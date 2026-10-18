//! Testes de ponta a ponta: pacotes montados à mão (partes XML mínimas dentro de um zip) e arquivos gravados pelo
//! próprio exportador do DataBox, relidos pelo leitor.

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use comum::{Codigo, Resultado, Valor as ValorBanco};
use exportar::{Acompanhamento, Formato as FormatoExp, Opcoes, exportar_linhas, exportar_varias};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::*;

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CABECA: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

// ---- apoio --------------------------------------------------------------------------------------------

/// Pasta temporária do teste, apagada no fim.
struct Tmp(PathBuf);

impl Tmp {
    fn nova(nome: &str) -> Tmp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "planilha-teste-{nome}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }

    fn arq(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
    }

    fn gravar(&self, nome: &str, dados: &[u8]) -> PathBuf {
        let p = self.arq(nome);
        fs::write(&p, dados).unwrap();
        p
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn zip_de(partes: &[(&str, &str)], metodo: CompressionMethod) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default().compression_method(metodo);
    for (nome, conteudo) in partes {
        z.start_file(*nome, opcoes).unwrap();
        z.write_all(conteudo.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn zip_deflate(partes: &[(&str, &str)]) -> Vec<u8> {
    zip_de(partes, CompressionMethod::Deflated)
}

/// `<worksheet><sheetData>…</sheetData></worksheet>`
fn folha_xml(dados: &str) -> String {
    format!("{CABECA}<worksheet xmlns=\"{NS}\"><sheetData>{dados}</sheetData></worksheet>")
}

struct Aba {
    nome: String,
    estado: &'static str,
    xml: String,
}

/// Monta um .xlsx mínimo, parte por parte.
struct Montador {
    abas: Vec<Aba>,
    textos: Option<String>,
    estilos: Option<String>,
    d1904: bool,
    ativa: Option<usize>,
}

impl Montador {
    fn novo() -> Montador {
        Montador { abas: Vec::new(), textos: None, estilos: None, d1904: false, ativa: None }
    }

    /// Uma aba com as `<row>` dadas.
    fn aba(mut self, nome: &str, linhas: &str) -> Montador {
        self.abas.push(Aba { nome: nome.to_string(), estado: "", xml: folha_xml(linhas) });
        self
    }

    fn aba_xml(mut self, nome: &str, xml: &str) -> Montador {
        self.abas.push(Aba { nome: nome.to_string(), estado: "", xml: xml.to_string() });
        self
    }

    fn estado(mut self, estado: &'static str) -> Montador {
        self.abas.last_mut().unwrap().estado = estado;
        self
    }

    fn textos(mut self, itens: &str) -> Montador {
        self.textos = Some(format!("{CABECA}<sst xmlns=\"{NS}\">{itens}</sst>"));
        self
    }

    fn estilos(mut self, numfmts: &[(u32, &str)], xfs: &[u32]) -> Montador {
        let mut s = format!("{CABECA}<styleSheet xmlns=\"{NS}\">");
        if !numfmts.is_empty() {
            s.push_str(&format!("<numFmts count=\"{}\">", numfmts.len()));
            for (id, codigo) in numfmts {
                let codigo = codigo.replace('&', "&amp;").replace('"', "&quot;");
                s.push_str(&format!("<numFmt numFmtId=\"{id}\" formatCode=\"{codigo}\"/>"));
            }
            s.push_str("</numFmts>");
        }
        s.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\"/></cellStyleXfs>");
        s.push_str(&format!("<cellXfs count=\"{}\">", xfs.len()));
        for id in xfs {
            s.push_str(&format!("<xf numFmtId=\"{id}\" fontId=\"0\" fillId=\"0\" borderId=\"0\"><alignment/></xf>"));
        }
        s.push_str("</cellXfs></styleSheet>");
        self.estilos = Some(s);
        self
    }

    fn d1904(mut self) -> Montador {
        self.d1904 = true;
        self
    }

    fn ativa(mut self, i: usize) -> Montador {
        self.ativa = Some(i);
        self
    }

    fn partes(&self) -> Vec<(String, String)> {
        let n = self.abas.len();
        let mut tipos = format!(
            "{CABECA}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>"
        );
        for i in 1..=n {
            tipos.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        }
        tipos.push_str("</Types>");
        let raiz = format!(
            "{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rId1\" Type=\"{NS_R}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
        );
        let mut livro = format!("{CABECA}<workbook xmlns=\"{NS}\" xmlns:r=\"{NS_R}\">");
        livro.push_str(&format!("<workbookPr{}/>", if self.d1904 { " date1904=\"1\"" } else { "" }));
        if let Some(a) = self.ativa {
            livro.push_str(&format!("<bookViews><workbookView activeTab=\"{a}\"/></bookViews>"));
        }
        livro.push_str("<sheets>");
        let mut rels =
            format!("{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        for (i, aba) in self.abas.iter().enumerate() {
            let i = i + 1;
            let estado = if aba.estado.is_empty() { String::new() } else { format!(" state=\"{}\"", aba.estado) };
            let nome = aba.nome.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
            livro.push_str(&format!("<sheet name=\"{nome}\" sheetId=\"{}\"{estado} r:id=\"rId{i}\"/>", i * 10));
            rels.push_str(&format!(
                "<Relationship Id=\"rId{i}\" Type=\"{NS_R}/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
            ));
        }
        livro.push_str("</sheets></workbook>");
        let mut extra = n + 1;
        if self.estilos.is_some() {
            rels.push_str(&format!("<Relationship Id=\"rId{extra}\" Type=\"{NS_R}/styles\" Target=\"styles.xml\"/>"));
            extra += 1;
        }
        if self.textos.is_some() {
            rels.push_str(&format!(
                "<Relationship Id=\"rId{extra}\" Type=\"{NS_R}/sharedStrings\" Target=\"sharedStrings.xml\"/>"
            ));
        }
        rels.push_str("</Relationships>");
        let mut partes = vec![
            ("[Content_Types].xml".to_string(), tipos),
            ("_rels/.rels".to_string(), raiz),
            ("xl/workbook.xml".to_string(), livro),
            ("xl/_rels/workbook.xml.rels".to_string(), rels),
        ];
        for (i, aba) in self.abas.iter().enumerate() {
            partes.push((format!("xl/worksheets/sheet{}.xml", i + 1), aba.xml.clone()));
        }
        if let Some(e) = &self.estilos {
            partes.push(("xl/styles.xml".to_string(), e.clone()));
        }
        if let Some(t) = &self.textos {
            partes.push(("xl/sharedStrings.xml".to_string(), t.clone()));
        }
        partes
    }

    fn bytes(&self) -> Vec<u8> {
        let partes = self.partes();
        let refs: Vec<(&str, &str)> = partes.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
        zip_deflate(&refs)
    }

    fn salvar(&self, tmp: &Tmp) -> PathBuf {
        tmp.gravar("livro.xlsx", &self.bytes())
    }
}

/// Abre um livro de uma aba só com as linhas dadas.
fn com_linhas(tmp: &Tmp, linhas: &str) -> Pasta {
    Pasta::abrir(Montador::novo().aba("Plan1", linhas).salvar(tmp)).unwrap()
}

fn tudo(p: &Pasta, folha: usize) -> Vec<Linha> {
    p.linhas(folha).unwrap().map(|l| l.unwrap()).collect()
}

fn texto(s: &str) -> Valor {
    Valor::Texto(s.to_string())
}

fn erro_de(r: Resultado<Pasta>) -> comum::ErroBanco {
    match r {
        Ok(_) => panic!("devia dar erro"),
        Err(e) => e,
    }
}

fn data(ano: i32, mes: u8, dia: u8) -> Data {
    Data { ano, mes, dia }
}

fn hora(h: u8, m: u8, s: u8) -> Hora {
    Hora { hora: h, minuto: m, segundo: s, milissegundo: 0 }
}

// ---- o que o exportador do DataBox grava ----------------------------------------------------------------

fn cols(nomes: &[&str]) -> Vec<String> {
    nomes.iter().map(|s| s.to_string()).collect()
}

/// Nome, colunas e linhas de uma fonte para o `exportar_varias`.
type FonteDeTeste = (String, Vec<String>, Vec<Resultado<Vec<ValorBanco>>>);

fn exportar_uma(tmp: &Tmp, colunas: &[&str], linhas: Vec<Vec<ValorBanco>>) -> PathBuf {
    let destino = tmp.arq("exportado.xlsx");
    let linhas: Vec<Resultado<Vec<ValorBanco>>> = linhas.into_iter().map(Ok).collect();
    exportar_linhas(
        &cols(colunas),
        linhas,
        FormatoExp::Xlsx,
        &destino,
        &Opcoes::default(),
        "dados",
        Acompanhamento::default(),
    )
    .unwrap();
    destino
}

#[test]
fn le_de_volta_o_que_o_exportador_grava() {
    let tmp = Tmp::nova("volta");
    let caminho = exportar_uma(
        &tmp,
        &["id", "nome", "preco", "obs", "vazio"],
        vec![
            vec![
                ValorBanco::Inteiro(1),
                ValorBanco::Texto("Ana".into()),
                ValorBanco::Real(10.5),
                ValorBanco::Nulo,
                ValorBanco::Nulo,
            ],
            vec![
                ValorBanco::Inteiro(-2),
                ValorBanco::Texto("João & Maria <3> \"aspas\" 'apóstrofo'".into()),
                ValorBanco::Real(0.1),
                ValorBanco::Texto("  espaços nas pontas  ".into()),
                ValorBanco::Texto(String::new()),
            ],
            vec![
                ValorBanco::Inteiro(3),
                ValorBanco::Texto("linha1\nlinha2\ttab".into()),
                ValorBanco::Real(-1234567.891),
                ValorBanco::Texto("日本語 ✓ 😀".into()),
                ValorBanco::Nulo,
            ],
        ],
    );
    let pasta = Pasta::abrir(&caminho).unwrap();
    assert_eq!(pasta.folhas().len(), 1);
    assert_eq!(pasta.folhas()[0].nome, "dados");
    assert_eq!(pasta.folhas()[0].estado, Estado::Visivel);
    assert_eq!(pasta.folhas()[0].tipo, TipoFolha::Planilha);
    assert!(pasta.folhas()[0].tamanho_xml > 100);
    assert_eq!(pasta.sistema_de_datas(), SistemaDatas::D1900);
    let linhas = tudo(&pasta, 0);
    assert_eq!(linhas.len(), 4);
    let v = |l: usize| linhas[l].valores(5);
    assert_eq!(v(0), [texto("id"), texto("nome"), texto("preco"), texto("obs"), texto("vazio")]);
    assert_eq!(v(1), [Valor::Numero(1.0), texto("Ana"), Valor::Numero(10.5), Valor::Vazio, Valor::Vazio]);
    assert_eq!(
        v(2),
        [
            Valor::Numero(-2.0),
            texto("João & Maria <3> \"aspas\" 'apóstrofo'"),
            Valor::Numero(0.1),
            texto("  espaços nas pontas  "),
            // texto vazio não vira célula no exportador
            Valor::Vazio,
        ]
    );
    assert_eq!(
        v(3),
        [
            Valor::Numero(3.0),
            texto("linha1\nlinha2\ttab"),
            Valor::Numero(-1234567.891),
            texto("日本語 ✓ 😀"),
            Valor::Vazio
        ]
    );
    // o cabeçalho do exportador usa o estilo 1 (negrito branco)
    assert_eq!(linhas[0].celula(0).unwrap().estilo, 1);
    assert_eq!(linhas[1].celula(0).unwrap().estilo, 0);
    // sem formato de data: nenhum valor virou data
    assert!(linhas.iter().flat_map(|l| &l.celulas).all(|c| !matches!(c.valor, Valor::Data(_) | Valor::DataHora(_))));
}

#[test]
fn texto_que_parece_formula_erro_ou_escape_continua_texto() {
    let tmp = Tmp::nova("parece");
    let textos =
        ["=SOMA(A1:A3)", "#N/A", "+1", "@x", "_x0041_ literal", "a_x005F_b", "TRUE", "2024-03-15", "00123", "1E5"];
    let linhas: Vec<Vec<ValorBanco>> = textos.iter().map(|t| vec![ValorBanco::Texto(t.to_string())]).collect();
    let pasta = Pasta::abrir(exportar_uma(&tmp, &["c"], linhas)).unwrap();
    let lidas = tudo(&pasta, 0);
    for (i, t) in textos.iter().enumerate() {
        let c = lidas[i + 1].celula(0).unwrap();
        assert_eq!(c.valor, texto(t), "{t}");
        assert_eq!(c.formula, None);
    }
}

#[test]
fn varias_planilhas_do_exportador_com_nomes_saneados_e_ordem() {
    let tmp = Tmp::nova("varias");
    let destino = tmp.arq("banco.xlsx");
    let fontes: Vec<FonteDeTeste> = vec![
        ("clientes".into(), cols(&["id"]), vec![Ok(vec![ValorBanco::Inteiro(1)]), Ok(vec![ValorBanco::Inteiro(2)])]),
        ("a/b:c?[d]".into(), cols(&["x", "y"]), vec![Ok(vec![ValorBanco::Texto("t".into()), ValorBanco::Real(2.5)])]),
        ("vazia".into(), cols(&["z"]), vec![]),
    ];
    exportar_varias(fontes, FormatoExp::Xlsx, &destino, &Opcoes::default(), None, Acompanhamento::default()).unwrap();
    let pasta = Pasta::abrir(&destino).unwrap();
    let nomes: Vec<&str> = pasta.folhas().iter().map(|f| f.nome.as_str()).collect();
    assert_eq!(nomes, ["clientes", "a_b_c__d_", "vazia"]);
    assert_eq!(pasta.primeira_planilha_visivel(), Some(0));
    assert_eq!(tudo(&pasta, 0).len(), 3);
    assert_eq!(tudo(&pasta, 1)[1].valores(2), [texto("t"), Valor::Numero(2.5)]);
    assert_eq!(tudo(&pasta, 2).len(), 1); // só o cabeçalho
}

#[test]
fn planilha_do_exportador_indexa_e_pagina() {
    let tmp = Tmp::nova("paginar-exp");
    let linhas: Vec<Vec<ValorBanco>> =
        (0..3000).map(|i| vec![ValorBanco::Inteiro(i), ValorBanco::Texto(format!("linha {i}"))]).collect();
    let pasta = Pasta::abrir(exportar_uma(&tmp, &["n", "t"], linhas)).unwrap();
    let mut aba = pasta.planilha(0).unwrap();
    assert_eq!(aba.meta().congelado, Some(Congelamento { linhas: 1, colunas: 0 }));
    assert_eq!(aba.meta().colunas.len(), 2);
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!((ind.primeira_linha, ind.ultima_linha, ind.colunas, ind.linhas_com_dados), (1, 3001, 2, 3001));
    assert_eq!(aba.tamanho(), (3001, 2));
    assert_eq!(aba.meta().filtro, Intervalo::de_texto("A1:B3001"));
    let p = aba.pagina(2001, 3).unwrap();
    assert_eq!(p.linhas.iter().map(|l| l.numero).collect::<Vec<_>>(), [2001, 2002, 2003]);
    assert_eq!(p.linhas[0].valores(2), [Valor::Numero(1999.0), texto("linha 1999")]); // a linha 1 é o cabeçalho
    assert_eq!(p.linhas[2].valor(1), &texto("linha 2001"));
}

// ---- partes do pacote -----------------------------------------------------------------------------------

#[test]
fn abas_nomes_ordem_e_estado() {
    let tmp = Tmp::nova("abas");
    let livro = Montador::novo()
        .aba("Resumo & Totais", "")
        .aba("Escondida", "")
        .estado("hidden")
        .aba("Secreta", "")
        .estado("veryHidden")
        .aba("Última \"1\"", "")
        .ativa(3);
    let pasta = Pasta::abrir(livro.salvar(&tmp)).unwrap();
    let f = pasta.folhas();
    assert_eq!(
        f.iter().map(|f| f.nome.as_str()).collect::<Vec<_>>(),
        ["Resumo & Totais", "Escondida", "Secreta", "Última \"1\""]
    );
    assert_eq!(
        f.iter().map(|f| f.estado).collect::<Vec<_>>(),
        [Estado::Visivel, Estado::Oculta, Estado::MuitoOculta, Estado::Visivel]
    );
    assert_eq!(f.iter().map(|f| f.id).collect::<Vec<_>>(), [10, 20, 30, 40]);
    assert_eq!(f[0].parte, "xl/worksheets/sheet1.xml");
    assert_eq!(pasta.aba_ativa(), 3);
    assert_eq!(pasta.primeira_planilha_visivel(), Some(3));
}

#[test]
fn aba_ativa_oculta_cai_na_primeira_visivel() {
    let tmp = Tmp::nova("ativa-oculta");
    let livro = Montador::novo().aba("A", "").estado("hidden").aba("B", "").aba("C", "").ativa(0);
    let pasta = Pasta::abrir(livro.salvar(&tmp)).unwrap();
    assert_eq!(pasta.primeira_planilha_visivel(), Some(1));
    let tudo_oculto = Montador::novo().aba("A", "").estado("hidden").salvar(&tmp);
    assert_eq!(Pasta::abrir(tudo_oculto).unwrap().primeira_planilha_visivel(), None);
}

#[test]
fn folha_de_grafico_nao_tem_celulas() {
    let tmp = Tmp::nova("grafico");
    let mut partes = Montador::novo().aba("Dados", "<row r=\"1\"><c r=\"A1\"><v>1</v></c></row>").partes();
    // acrescenta uma folha de gráfico no livro e nas relações
    for (nome, conteudo) in &mut partes {
        if nome == "xl/workbook.xml" {
            *conteudo =
                conteudo.replace("</sheets>", "<sheet name=\"Gráfico1\" sheetId=\"2\" r:id=\"rId9\"/></sheets>");
        }
        if nome == "xl/_rels/workbook.xml.rels" {
            *conteudo = conteudo.replace(
                "</Relationships>",
                &format!("<Relationship Id=\"rId9\" Type=\"{NS_R}/chartsheet\" Target=\"chartsheets/sheet1.xml\"/></Relationships>"),
            );
        }
    }
    partes.push(("xl/chartsheets/sheet1.xml".into(), format!("{CABECA}<chartsheet xmlns=\"{NS}\"/>")));
    let refs: Vec<(&str, &str)> = partes.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    let caminho = tmp.gravar("g.xlsx", &zip_deflate(&refs));
    let pasta = Pasta::abrir(caminho).unwrap();
    assert_eq!(pasta.folhas()[1].tipo, TipoFolha::Grafico);
    assert_eq!(pasta.folhas()[1].nome, "Gráfico1");
    let e = pasta.planilha(1).err().unwrap();
    assert!(e.mensagem.contains("Gráfico1") && e.mensagem.contains("gráfico"), "{e}");
    assert!(pasta.linhas(1).is_err());
    assert_eq!(pasta.planilha(5).err().unwrap().mensagem, "A aba 6 não existe: a pasta de trabalho tem 2.");
    assert!(pasta.planilha(0).is_ok());
}

#[test]
fn textos_compartilhados_com_trechos_formatados() {
    let tmp = Tmp::nova("sst");
    let itens = "<si><t>simples</t></si>\
        <si><r><rPr><b/><sz val=\"11\"/></rPr><t xml:space=\"preserve\">Olá </t></r><r><t>mundo</t></r></si>\
        <si><t>kanji</t><rPh sb=\"0\" eb=\"1\"><t>ｶﾝｼﾞ</t></rPh><phoneticPr fontId=\"1\"/></si>\
        <si><t/></si>\
        <si></si>\
        <si><t>a_x000D_b_x005F_x0041_</t></si>\
        <si><t><![CDATA[<tag> & cdata]]></t></si>\
        <si><t>com &amp; e &lt;b&gt; &#233;</t></si>\
        <si><r><t>x</t></r><r><rPr><i/></rPr><t> </t></r><r><t>y</t></r></si>";
    let linha = "<row r=\"1\">\
        <c r=\"A1\" t=\"s\"><v>0</v></c><c r=\"B1\" t=\"s\"><v>1</v></c><c r=\"C1\" t=\"s\"><v>2</v></c>\
        <c r=\"D1\" t=\"s\"><v>3</v></c><c r=\"E1\" t=\"s\"><v>4</v></c><c r=\"F1\" t=\"s\"><v>5</v></c>\
        <c r=\"G1\" t=\"s\"><v>6</v></c><c r=\"H1\" t=\"s\"><v>7</v></c><c r=\"I1\" t=\"s\"><v>8</v></c></row>";
    let pasta = Pasta::abrir(Montador::novo().aba("P", linha).textos(itens).salvar(&tmp)).unwrap();
    assert_eq!(pasta.textos_compartilhados(), 9);
    assert_eq!(pasta.texto_compartilhado(1), Some("Olá mundo"));
    assert_eq!(pasta.texto_compartilhado(99), None);
    let l = &tudo(&pasta, 0)[0];
    assert_eq!(
        l.valores(9),
        [
            texto("simples"),
            texto("Olá mundo"),
            texto("kanji"),
            texto(""),
            texto(""),
            texto("a\rb_x0041_"),
            texto("<tag> & cdata"),
            texto("com & e <b> é"),
            texto("x y"),
        ]
    );
}

#[test]
fn textos_compartilhados_acima_do_limite_viram_erro() {
    let tmp = Tmp::nova("sst-limite");
    // 40 frases de 10 bytes: 400 bytes de texto mais 40 fins de frase
    let itens = "<si><t>0123456789</t></si>".repeat(40);
    let arquivo = Montador::novo().aba("P", "").textos(&itens).salvar(&tmp);
    let e = erro_de(Pasta::abrir_com_limite(&arquivo, 300));
    assert_eq!(
        e.mensagem,
        "A planilha não pôde ser aberta: os textos dela ocupariam mais de 300 bytes na memória, o limite do DataBox.          Se o arquivo for de confiança, divida-o no Excel em pastas de trabalho menores."
    );
    assert_eq!(e.status, 400);
    assert!(e.detalhe.as_deref().is_some_and(|d| d.contains("sharedStrings.xml")), "{:?}", e.detalhe);
    // uma frase só, maior que o limite, para no meio dela
    let longa = format!("<si><t>{}</t></si>", "x".repeat(1000));
    let tmp_longa = Tmp::nova("sst-limite-longa");
    let arquivo = Montador::novo().aba("P", "").textos(&longa).salvar(&tmp_longa);
    assert!(erro_de(Pasta::abrir_com_limite(&arquivo, 300)).mensagem.contains("300 bytes"));
    // frases vazias também contam (o fim de cada uma)
    let vazias = "<si/>".repeat(100);
    let tmp_vazias = Tmp::nova("sst-limite-vazias");
    let arquivo = Montador::novo().aba("P", "").textos(&vazias).salvar(&tmp_vazias);
    assert!(erro_de(Pasta::abrir_com_limite(&arquivo, 300)).mensagem.contains("300 bytes"));
    // abaixo do limite abre, e o limite de verdade é em MB
    let pasta = Pasta::abrir_com_limite(&Montador::novo().aba("P", "").textos(&itens).salvar(&tmp), 1000).unwrap();
    assert_eq!(pasta.textos_compartilhados(), 40);
    assert!(erros::textos_grandes(pasta::LIMITE_TEXTOS).mensagem.contains("mais de 512 MB"));
}

#[test]
fn indice_de_texto_invalido_vira_celula_vazia() {
    let tmp = Tmp::nova("sst-ruim");
    let linha =
        "<row r=\"1\"><c r=\"A1\" t=\"s\"><v>7</v></c><c r=\"B1\" t=\"s\"><v>abc</v></c><c r=\"C1\" t=\"s\"/></row>";
    let pasta = Pasta::abrir(Montador::novo().aba("P", linha).textos("<si><t>x</t></si>").salvar(&tmp)).unwrap();
    assert_eq!(tudo(&pasta, 0)[0].valores(3), [Valor::Vazio, Valor::Vazio, Valor::Vazio]);
}

#[test]
fn sem_textos_nem_estilos_funciona() {
    let tmp = Tmp::nova("sem-partes");
    let pasta = com_linhas(&tmp, "<row r=\"1\"><c r=\"A1\"><v>5</v></c><c r=\"B1\" s=\"9\"><v>6</v></c></row>");
    assert_eq!(pasta.textos_compartilhados(), 0);
    assert_eq!(tudo(&pasta, 0)[0].valores(2), [Valor::Numero(5.0), Valor::Numero(6.0)]);
    assert_eq!(pasta.formato(9).categoria(), Categoria::Geral);
}

// ---- tipos de célula -------------------------------------------------------------------------------------

#[test]
fn tipos_de_celula() {
    let tmp = Tmp::nova("tipos");
    let linha = "<row r=\"1\">\
        <c r=\"A1\"><v>42</v></c>\
        <c r=\"B1\" t=\"n\"><v>-1.5e-3</v></c>\
        <c r=\"C1\" t=\"inlineStr\"><is><t>embutido</t></is></c>\
        <c r=\"D1\" t=\"inlineStr\"><is><r><t>ri</t></r><r><rPr><b/></rPr><t> co</t></r></is></c>\
        <c r=\"E1\" t=\"b\"><v>1</v></c>\
        <c r=\"F1\" t=\"b\"><v>0</v></c>\
        <c r=\"G1\" t=\"e\"><v>#DIV/0!</v></c>\
        <c r=\"H1\" t=\"str\"><f>A1&amp;\"x\"</f><v>42x</v></c>\
        <c r=\"I1\"><f>SUM(A1:A3)</f><v>6</v></c>\
        <c r=\"J1\"><f>NOW()</f></c>\
        <c r=\"K1\" s=\"3\"/>\
        <c r=\"L1\"><v>  7  </v></c>\
        <c r=\"M1\"><v>texto no lugar do número</v></c>\
        <c r=\"N1\" t=\"inlineStr\"><v>só v</v></c>\
        <c r=\"O1\" t=\"b\"><v>TRUE</v></c>\
        <c r=\"P1\"><v/></c>\
        </row>";
    let pasta = com_linhas(&tmp, linha);
    let l = &tudo(&pasta, 0)[0];
    assert_eq!(
        l.valores(16),
        [
            Valor::Numero(42.0),
            Valor::Numero(-0.0015),
            texto("embutido"),
            texto("ri co"),
            Valor::Booleano(true),
            Valor::Booleano(false),
            Valor::Erro("#DIV/0!".into()),
            texto("42x"),
            Valor::Numero(6.0),
            Valor::Vazio,
            Valor::Vazio,
            Valor::Numero(7.0),
            texto("texto no lugar do número"),
            texto("só v"),
            Valor::Booleano(true),
            Valor::Vazio,
        ]
    );
    // fórmula: o último valor e o texto
    assert_eq!(l.celula(7).unwrap().formula.as_deref(), Some("A1&\"x\""));
    assert_eq!(l.celula(8).unwrap().formula.as_deref(), Some("SUM(A1:A3)"));
    assert_eq!(l.celula(9).unwrap().formula.as_deref(), Some("NOW()"));
    assert_eq!(l.celula(0).unwrap().formula, None);
    // a célula só com estilo existe, vazia
    assert_eq!(l.celula(10).unwrap().estilo, 3);
    assert!(l.celula(10).unwrap().valor.vazio());
}

#[test]
fn formula_compartilhada_sem_texto() {
    let tmp = Tmp::nova("f-comp");
    let linhas = "<row r=\"1\"><c r=\"A1\"><f t=\"shared\" ref=\"A1:A3\" si=\"0\">B1*2</f><v>2</v></c></row>\
                  <row r=\"2\"><c r=\"A2\"><f t=\"shared\" si=\"0\"/><v>4</v></c></row>";
    let t = tudo(&com_linhas(&tmp, linhas), 0);
    assert_eq!(t[0].celula(0).unwrap().formula.as_deref(), Some("B1*2"));
    assert_eq!(t[1].celula(0).unwrap().formula, None);
    assert_eq!(t[1].valor(0), &Valor::Numero(4.0));
}

#[test]
fn erros_de_celula_comuns() {
    let tmp = Tmp::nova("erros");
    let erros = ["#NULL!", "#DIV/0!", "#VALUE!", "#REF!", "#NAME?", "#NUM!", "#N/A", "#GETTING_DATA"];
    let celulas: String = erros
        .iter()
        .enumerate()
        .map(|(i, e)| format!("<c r=\"{}1\" t=\"e\"><v>{e}</v></c>", nome_da_coluna(i as u32)))
        .collect();
    let t = tudo(&com_linhas(&tmp, &format!("<row r=\"1\">{celulas}</row>")), 0);
    for (i, e) in erros.iter().enumerate() {
        assert_eq!(t[0].valor(i as u32), &Valor::Erro(e.to_string()));
    }
}

// ---- datas e formatos -----------------------------------------------------------------------------------

fn livro_com_estilos(tmp: &Tmp, linha: &str, d1904: bool) -> Pasta {
    // 0 geral, 1 = 14 (data), 2 = personalizado data e hora, 3 = 20 (hora), 4 = 9 (percentual),
    // 5 = moeda personalizada, 6 = 46 (duração), 7 = 22 (data e hora embutido), 8 = 49 (texto)
    let mut m = Montador::novo()
        .aba("P", linha)
        .estilos(&[(164, "yyyy-mm-dd hh:mm"), (165, "\"R$\" #,##0.00")], &[0, 14, 164, 20, 9, 165, 46, 22, 49]);
    if d1904 {
        m = m.d1904();
    }
    Pasta::abrir(m.salvar(tmp)).unwrap()
}

#[test]
fn datas_horas_e_formatos_pelo_estilo() {
    let tmp = Tmp::nova("datas");
    let linha = "<row r=\"1\">\
        <c r=\"A1\" s=\"1\"><v>45000</v></c>\
        <c r=\"B1\" s=\"2\"><v>45000.5</v></c>\
        <c r=\"C1\" s=\"3\"><v>0.75</v></c>\
        <c r=\"D1\" s=\"4\"><v>0.256</v></c>\
        <c r=\"E1\" s=\"5\"><v>1234.5</v></c>\
        <c r=\"F1\" s=\"6\"><v>1.5</v></c>\
        <c r=\"G1\" s=\"7\"><v>45000.25</v></c>\
        <c r=\"H1\" s=\"0\"><v>45000</v></c>\
        <c r=\"I1\" s=\"8\"><v>45000</v></c>\
        <c r=\"J1\" s=\"1\"><v>-5</v></c>\
        <c r=\"K1\" s=\"1\" t=\"inlineStr\"><is><t>já é texto</t></is></c>\
        <c r=\"L1\" s=\"99\"><v>45000</v></c>\
        </row>";
    let pasta = livro_com_estilos(&tmp, linha, false);
    let l = &tudo(&pasta, 0)[0];
    assert_eq!(l.valor(0), &Valor::Data(data(2023, 3, 15)));
    assert_eq!(l.valor(1), &Valor::DataHora(Momento { data: data(2023, 3, 15), hora: hora(12, 0, 0) }));
    assert_eq!(l.valor(2), &Valor::Hora(hora(18, 0, 0)));
    assert_eq!(l.valor(3), &Valor::Numero(0.256));
    assert_eq!(l.valor(4), &Valor::Numero(1234.5));
    assert_eq!(l.valor(5), &Valor::Numero(1.5));
    assert_eq!(l.valor(6), &Valor::DataHora(Momento { data: data(2023, 3, 15), hora: hora(6, 0, 0) }));
    assert_eq!(l.valor(7), &Valor::Numero(45000.0)); // sem formato de data é número
    assert_eq!(l.valor(8), &Valor::Numero(45000.0)); // formato de texto: continua número
    assert_eq!(l.valor(9), &Valor::Numero(-5.0)); // data negativa não existe
    assert_eq!(l.valor(10), &texto("já é texto"));
    assert_eq!(l.valor(11), &Valor::Numero(45000.0)); // estilo desconhecido
    // as categorias
    assert_eq!(pasta.formato(1).categoria(), Categoria::Data);
    assert_eq!(pasta.formato(2).categoria(), Categoria::DataHora);
    assert_eq!(pasta.formato(3).categoria(), Categoria::Hora);
    assert_eq!(pasta.formato(4).categoria(), Categoria::Percentual);
    assert_eq!(pasta.formato(5).categoria(), Categoria::Numero);
    assert_eq!(pasta.formato(5).codigo(), "\"R$\" #,##0.00");
    assert_eq!(pasta.formato(6).categoria(), Categoria::Duracao);
    // como aparecem
    let mostra = |c: u32| pasta.exibir(l.celula(c).unwrap());
    assert_eq!(mostra(0), "15/03/2023");
    assert_eq!(mostra(1), "2023-03-15 12:00");
    assert_eq!(mostra(2), "18:00");
    assert_eq!(mostra(3), "26%");
    assert_eq!(mostra(4), "R$ 1.234,50");
    assert_eq!(mostra(5), "36:00:00");
    assert_eq!(mostra(6), "15/03/2023 6:00");
    assert_eq!(mostra(7), "45000");
    assert_eq!(mostra(9), "-5");
    assert_eq!(mostra(10), "já é texto");
}

#[test]
fn defeito_de_1900_e_sistema_1904() {
    let tmp = Tmp::nova("1900");
    let celulas = |series: &[&str]| -> String {
        let c: String = series
            .iter()
            .enumerate()
            .map(|(i, s)| format!("<c r=\"{}1\" s=\"1\"><v>{s}</v></c>", nome_da_coluna(i as u32)))
            .collect();
        format!("<row r=\"1\">{c}</row>")
    };
    let series = ["0", "1", "59", "60", "61", "25569"];
    let p1900 = livro_com_estilos(&tmp, &celulas(&series), false);
    let l = &tudo(&p1900, 0)[0];
    let ds: Vec<Valor> = l.valores(6);
    assert_eq!(
        ds,
        [
            Valor::Data(data(1900, 1, 0)),
            Valor::Data(data(1900, 1, 1)),
            Valor::Data(data(1900, 2, 28)),
            Valor::Data(data(1900, 2, 29)), // o dia que não existiu
            Valor::Data(data(1900, 3, 1)),
            Valor::Data(data(1970, 1, 1)),
        ]
    );
    assert_eq!(p1900.exibir(l.celula(3).unwrap()), "29/02/1900");
    let tmp2 = Tmp::nova("1904");
    let p1904 = livro_com_estilos(&tmp2, &celulas(&["0", "1", "1462", "43538"]), true);
    assert_eq!(p1904.sistema_de_datas(), SistemaDatas::D1904);
    assert_eq!(
        tudo(&p1904, 0)[0].valores(4),
        [
            Valor::Data(data(1904, 1, 1)),
            Valor::Data(data(1904, 1, 2)),
            Valor::Data(data(1908, 1, 2)),
            Valor::Data(data(2023, 3, 15)),
        ]
    );
}

#[test]
fn data_iso_das_celulas_t_d() {
    let tmp = Tmp::nova("iso");
    let linha = "<row r=\"1\">\
        <c r=\"A1\" t=\"d\"><v>2024-03-15</v></c>\
        <c r=\"B1\" t=\"d\"><v>2024-03-15T10:30:00</v></c>\
        <c r=\"C1\" t=\"d\"><v>2024-03-15T10:30:00.250Z</v></c>\
        <c r=\"D1\" t=\"d\"><v>ontem</v></c>\
        </row>";
    let v = tudo(&com_linhas(&tmp, linha), 0)[0].valores(4);
    assert_eq!(v[0], Valor::Data(data(2024, 3, 15)));
    assert_eq!(v[1], Valor::DataHora(Momento { data: data(2024, 3, 15), hora: hora(10, 30, 0) }));
    assert_eq!(
        v[2],
        Valor::DataHora(Momento {
            data: data(2024, 3, 15),
            hora: Hora { hora: 10, minuto: 30, segundo: 0, milissegundo: 250 }
        })
    );
    assert_eq!(v[3], texto("ontem"));
}

// ---- lacunas, referências e linhas --------------------------------------------------------------------

#[test]
fn referencias_com_lacunas_de_colunas_e_linhas() {
    let tmp = Tmp::nova("lacunas");
    let linhas = "<row r=\"2\"><c r=\"C2\"><v>1</v></c><c r=\"F2\"><v>2</v></c></row>\
                  <row r=\"5\"><c r=\"A5\"><v>3</v></c></row>\
                  <row r=\"7\" hidden=\"1\" ht=\"30.5\"><c r=\"AA7\" t=\"inlineStr\"><is><t>longe</t></is></c></row>";
    let pasta = com_linhas(&tmp, linhas);
    let t = tudo(&pasta, 0);
    assert_eq!(t.iter().map(|l| l.numero).collect::<Vec<_>>(), [2, 5, 7]);
    assert_eq!(t[0].celulas.iter().map(|c| c.coluna).collect::<Vec<_>>(), [2, 5]);
    assert_eq!(
        t[0].valores(7),
        [Valor::Vazio, Valor::Vazio, Valor::Numero(1.0), Valor::Vazio, Valor::Vazio, Valor::Numero(2.0), Valor::Vazio]
    );
    assert_eq!(t[2].celulas[0].coluna, 26);
    assert!(t[2].oculta && !t[0].oculta);
    assert_eq!(t[2].altura, Some(30.5));
    assert_eq!(t[0].altura, None);

    let mut aba = pasta.planilha(0).unwrap();
    aba.indexar(|_| true).unwrap();
    assert_eq!(aba.tamanho(), (7, 27));
    let p = aba.pagina(1, 100).unwrap();
    assert_eq!(p.linhas.len(), 7); // cortada na última linha
    assert_eq!(p.linhas.iter().map(|l| l.numero).collect::<Vec<_>>(), (1..=7).collect::<Vec<_>>());
    assert!(p.linhas[0].celulas.is_empty() && p.linhas[2].celulas.is_empty() && p.linhas[3].celulas.is_empty());
    assert_eq!(p.linhas[4].valor(0), &Valor::Numero(3.0));
    assert_eq!(p.linhas[1].valor(2), &Valor::Numero(1.0));
}

#[test]
fn linha_e_celula_sem_referencia_seguem_a_ordem() {
    let tmp = Tmp::nova("sem-r");
    let linhas = "<row><c><v>1</v></c><c><v>2</v></c></row>\
                  <row r=\"5\"><c r=\"C5\"><v>3</v></c><c><v>4</v></c></row>\
                  <row r=\"3\"><c r=\"A3\"><v>5</v></c></row>\
                  <row r=\"6\"><c r=\"B6\"><v>6</v></c><c r=\"A6\"><v>7</v></c></row>";
    let t = tudo(&com_linhas(&tmp, linhas), 0);
    // r ausente = a seguinte; r repetido ou para trás = a seguinte (as linhas só crescem)
    assert_eq!(t.iter().map(|l| l.numero).collect::<Vec<_>>(), [1, 5, 6, 7]);
    assert_eq!(t[0].valores(2), [Valor::Numero(1.0), Valor::Numero(2.0)]);
    assert_eq!(t[1].valores(4), [Valor::Vazio, Valor::Vazio, Valor::Numero(3.0), Valor::Numero(4.0)]);
    // colunas fora de ordem são reordenadas
    assert_eq!(t[3].celulas.iter().map(|c| c.coluna).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(t[3].valores(2), [Valor::Numero(7.0), Valor::Numero(6.0)]);
}

#[test]
fn linha_vazia_e_celulas_vazias() {
    let tmp = Tmp::nova("vazias");
    let linhas = "<row r=\"1\"/><row r=\"2\"></row><row r=\"3\"><c r=\"A3\"/><c r=\"B3\" s=\"1\"></c></row>";
    let t = tudo(&com_linhas(&tmp, linhas), 0);
    assert_eq!(t.len(), 3);
    assert!(t[0].celulas.is_empty() && t[1].celulas.is_empty());
    assert_eq!(t[2].celulas.len(), 2);
    assert!(t[2].sem_valores());
}

// ---- informações da aba ------------------------------------------------------------------------------

#[test]
fn dimensao_colunas_painel_mescladas_e_filtro() {
    let tmp = Tmp::nova("meta");
    let xml = format!(
        "{CABECA}<worksheet xmlns=\"{NS}\"><sheetPr/><dimension ref=\"A1:D10\"/>\
         <sheetViews><sheetView workbookViewId=\"0\"><pane xSplit=\"1\" ySplit=\"2\" topLeftCell=\"B3\" activePane=\"bottomRight\" state=\"frozen\"/></sheetView></sheetViews>\
         <sheetFormatPr defaultRowHeight=\"15\" defaultColWidth=\"9.5\"/>\
         <cols><col min=\"1\" max=\"1\" width=\"20.5\" customWidth=\"1\"/><col min=\"3\" max=\"4\" width=\"12\" hidden=\"1\"/></cols>\
         <sheetData><row r=\"1\"><c r=\"A1\"><v>1</v></c></row><row r=\"3\"><c r=\"D3\"><v>1</v></c></row></sheetData>\
         <autoFilter ref=\"A1:D3\"/>\
         <mergeCells count=\"2\"><mergeCell ref=\"A1:B2\"/><mergeCell ref=\"$C$5:D6\"/><mergeCell ref=\"lixo\"/></mergeCells>\
         </worksheet>"
    );
    let pasta = Pasta::abrir(Montador::novo().aba_xml("P", &xml).salvar(&tmp)).unwrap();
    let mut aba = pasta.planilha(0).unwrap();
    // antes de indexar: o que vem antes dos dados
    let m = aba.meta();
    assert_eq!(m.dimensao, Intervalo::de_texto("A1:D10"));
    assert_eq!(m.congelado, Some(Congelamento { linhas: 2, colunas: 1 }));
    assert_eq!(m.altura_padrao, Some(15.0));
    assert_eq!(m.largura_padrao, Some(9.5));
    assert_eq!(m.colunas.len(), 2);
    assert_eq!(m.largura_da_coluna(0), Some(20.5));
    assert_eq!(m.largura_da_coluna(1), Some(9.5));
    assert_eq!(m.largura_da_coluna(3), Some(12.0));
    assert!(m.colunas[1].oculta && !m.colunas[0].oculta);
    assert!(m.mescladas.is_empty(), "as mescladas ficam no fim do XML");
    assert_eq!(aba.tamanho(), (10, 4)); // pela dimensão declarada
    aba.indexar(|_| true).unwrap();
    assert_eq!(aba.tamanho(), (3, 4)); // pelo que existe
    let m = aba.meta();
    assert_eq!(m.mescladas, [Intervalo::de_texto("A1:B2").unwrap(), Intervalo::de_texto("C5:D6").unwrap()]);
    assert_eq!(m.mescla_em(2, 1), Intervalo::de_texto("A1:B2").as_ref());
    assert_eq!(m.mescla_em(5, 3), Intervalo::de_texto("C5:D6").as_ref());
    assert_eq!(m.mescla_em(3, 0), None);
    assert_eq!(m.filtro, Intervalo::de_texto("A1:D3"));
    assert_eq!(m.dimensao, Intervalo::de_texto("A1:D10"));
}

#[test]
fn painel_dividido_sem_congelar_nao_conta() {
    let tmp = Tmp::nova("pane");
    let xml = format!(
        "{CABECA}<worksheet xmlns=\"{NS}\"><sheetViews><sheetView><pane ySplit=\"3000\" state=\"split\"/></sheetView></sheetViews><sheetData/></worksheet>"
    );
    let pasta = Pasta::abrir(Montador::novo().aba_xml("P", &xml).salvar(&tmp)).unwrap();
    assert_eq!(pasta.planilha(0).unwrap().meta().congelado, None);
}

// ---- índice e paginação --------------------------------------------------------------------------------

/// `n` linhas com um número na A e um texto na B.
fn linhas_geradas(n: u32) -> String {
    (1..=n)
        .map(|i| {
            format!(
                "<row r=\"{i}\"><c r=\"A{i}\"><v>{i}</v></c><c r=\"B{i}\" t=\"inlineStr\"><is><t>linha {i}</t></is></c></row>"
            )
        })
        .collect()
}

fn conferir_pagina(p: &Pagina, primeira: u32, quantidade: u32) {
    assert_eq!(p.primeira_linha, primeira);
    assert_eq!(p.linhas.len() as u32, quantidade, "quantidade de linhas");
    for (k, l) in p.linhas.iter().enumerate() {
        let n = primeira + k as u32;
        assert_eq!(l.numero, n);
        assert_eq!(l.valor(0), &Valor::Numero(f64::from(n)), "linha {n}");
        assert_eq!(l.valor(1), &texto(&format!("linha {n}")), "linha {n}");
    }
}

#[test]
fn indice_marcos_e_paginas_em_qualquer_ponto() {
    let tmp = Tmp::nova("paginas");
    let pasta = com_linhas(&tmp, &linhas_geradas(1000));
    let mut aba = pasta.planilha(0).unwrap();
    assert!(aba.indice().is_none());
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!((ind.primeira_linha, ind.ultima_linha, ind.colunas), (1, 1000, 2));
    assert_eq!(ind.linhas_com_dados, 1000);
    assert_eq!(ind.marcos(), 8); // linhas 1, 129, 257 … 897
    assert!(ind.bytes_xml > 50_000);
    assert_eq!(aba.tamanho(), (1000, 2));
    for (primeira, qtd) in
        [(1, 10), (1, 128), (120, 20), (128, 2), (129, 1), (500, 300), (900, 100), (1000, 1), (950, 500)]
    {
        let esperado = qtd.min(1000 - primeira + 1);
        let p = aba.pagina(primeira, qtd).unwrap();
        conferir_pagina(&p, primeira, esperado);
    }
    // fora do fim
    assert!(aba.pagina(1001, 10).unwrap().linhas.is_empty());
    assert!(aba.pagina(5, 0).unwrap().linhas.is_empty());
    // linha 0 vale 1
    conferir_pagina(&aba.pagina(0, 3).unwrap(), 1, 3);
    // página bem maior que o cache
    conferir_pagina(&aba.pagina(1, 1000).unwrap(), 1, 1000);
}

#[test]
fn rolar_para_baixo_nao_reabre_o_fluxo() {
    let tmp = Tmp::nova("rolar");
    let pasta = com_linhas(&tmp, &linhas_geradas(2000));
    let mut aba = pasta.planilha(0).unwrap();
    aba.indexar(|_| true).unwrap();
    for k in 0..20u32 {
        conferir_pagina(&aba.pagina(1 + k * 100, 100).unwrap(), 1 + k * 100, 100);
    }
    assert_eq!(aba.aberturas_do_fluxo(), 1, "ler em sequência continua o mesmo fluxo");
    // saltar para a frente também
    conferir_pagina(&aba.pagina(1900, 50).unwrap(), 1900, 50);
    assert_eq!(aba.aberturas_do_fluxo(), 1);
}

#[test]
fn voltar_com_o_cache_pequeno_reabre_e_acerta() {
    let tmp = Tmp::nova("voltar");
    let pasta = com_linhas(&tmp, &linhas_geradas(1500));
    let mut aba = pasta.planilha(0).unwrap();
    aba.definir_cache(2);
    aba.indexar(|_| true).unwrap();
    conferir_pagina(&aba.pagina(1, 100).unwrap(), 1, 100);
    conferir_pagina(&aba.pagina(1400, 50).unwrap(), 1400, 50);
    conferir_pagina(&aba.pagina(700, 20).unwrap(), 700, 20);
    assert_eq!(aba.aberturas_do_fluxo(), 2, "voltar de 1400 para 700 precisou recomeçar");
    // 700 ainda está no cache: não lê de novo
    conferir_pagina(&aba.pagina(705, 5).unwrap(), 705, 5);
    assert_eq!(aba.aberturas_do_fluxo(), 2);
    aba.liberar();
    conferir_pagina(&aba.pagina(300, 10).unwrap(), 300, 10);
    assert_eq!(aba.aberturas_do_fluxo(), 3);
}

#[test]
fn paginar_sem_indexar_tambem_funciona() {
    let tmp = Tmp::nova("sem-indice");
    let pasta = com_linhas(&tmp, &linhas_geradas(600));
    let mut aba = pasta.planilha(0).unwrap();
    conferir_pagina(&aba.pagina(1, 50).unwrap(), 1, 50);
    conferir_pagina(&aba.pagina(300, 50).unwrap(), 300, 50);
    conferir_pagina(&aba.pagina(10, 10).unwrap(), 10, 10); // já estava no cache
    // sem índice a página tem sempre a quantidade pedida
    let p = aba.pagina(590, 30).unwrap();
    assert_eq!(p.linhas.len(), 30);
    assert_eq!(p.linhas[10].valor(0), &Valor::Numero(600.0));
    assert!(p.linhas[11].celulas.is_empty());
    // depois de indexar, o cache e o cursor recomeçam sem perder a correção
    aba.indexar(|_| true).unwrap();
    conferir_pagina(&aba.pagina(590, 30).unwrap(), 590, 11);
}

#[test]
fn numeracao_esparsa_pagina_com_linhas_vazias() {
    let tmp = Tmp::nova("esparsa");
    let linhas = "<row r=\"1\"><c r=\"A1\"><v>1</v></c></row>\
                  <row r=\"500\"><c r=\"A500\"><v>500</v></c></row>\
                  <row r=\"2000\"><c r=\"A2000\"><v>2000</v></c></row>";
    let pasta = com_linhas(&tmp, linhas);
    let mut aba = pasta.planilha(0).unwrap();
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!((ind.linhas_com_dados, ind.primeira_linha, ind.ultima_linha), (3, 1, 2000));
    let p = aba.pagina(495, 10).unwrap();
    assert_eq!(p.linhas.iter().map(|l| l.numero).collect::<Vec<_>>(), (495..=504).collect::<Vec<_>>());
    assert_eq!(p.linhas[5].valor(0), &Valor::Numero(500.0));
    assert_eq!(p.linhas.iter().filter(|l| !l.celulas.is_empty()).count(), 1);
    let p = aba.pagina(1990, 20).unwrap();
    assert_eq!(p.linhas.len(), 11);
    assert_eq!(p.linhas[10].valor(0), &Valor::Numero(2000.0));
    // dados que só começam longe do topo
    let tmp2 = Tmp::nova("esparsa-2");
    let pasta2 = com_linhas(&tmp2, "<row r=\"1000\"><c r=\"A1000\"><v>7</v></c></row>");
    let mut aba2 = pasta2.planilha(0).unwrap();
    aba2.indexar(|_| true).unwrap();
    let p = aba2.pagina(1, 5).unwrap();
    assert_eq!(p.linhas.len(), 5);
    assert!(p.linhas.iter().all(|l| l.celulas.is_empty()));
    assert_eq!(aba2.pagina(999, 3).unwrap().linhas[1].valor(0), &Valor::Numero(7.0));
}

#[test]
fn indice_de_linhas_fora_do_caminho_rapido() {
    // uma linha com 3.000 células (mais de 64 KB de XML: não cabe num bloco do leitor), comentário e CDATA no
    // meio de outra, células sem referência em outra, e vizinhas comuns
    let tmp = Tmp::nova("indice-dificil");
    let enorme: String = (0..3000u32)
        .map(|c| format!("<c r=\"{}2\" t=\"inlineStr\"><is><t>valor da célula {c}</t></is></c>", nome_da_coluna(c)))
        .collect();
    let linhas = format!(
        "<row r=\"1\"><c r=\"A1\"><v>1</v></c></row>\
         <row r=\"2\">{enorme}</row>\
         <row r=\"3\"><c r=\"A3\"><v>3</v></c><!-- </row><row r=\"99\"> --><c r=\"B3\" t=\"inlineStr\"><is><t><![CDATA[</row><row r=\"98\">]]></t></is></c></row>\
         <row r=\"4\"><c><v>4</v></c><c><v>5</v></c><c><v>6</v></c></row>\
         <row r=\"5\"/>\
         <row r=\"6\"><c r=\"C6\"><v>6</v></c></row>"
    );
    let pasta = com_linhas(&tmp, &linhas);
    let mut aba = pasta.planilha(0).unwrap();
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!((ind.primeira_linha, ind.ultima_linha, ind.linhas_com_dados), (1, 6, 6));
    assert_eq!(ind.colunas, 3000);
    let p = aba.pagina(1, 6).unwrap();
    assert_eq!(p.linhas[1].celulas.len(), 3000);
    assert_eq!(p.linhas[1].valor(2999), &texto("valor da célula 2999"));
    assert_eq!(p.linhas[2].valor(1), &texto("</row><row r=\"98\">"));
    assert_eq!(p.linhas[3].valores(3), [Valor::Numero(4.0), Valor::Numero(5.0), Valor::Numero(6.0)]);
    assert!(p.linhas[4].celulas.is_empty());
    assert_eq!(p.linhas[5].valor(2), &Valor::Numero(6.0));
}

#[test]
fn indice_com_prefixo_de_espaco_de_nomes_nas_linhas() {
    let tmp = Tmp::nova("indice-prefixo");
    let linhas: String = (1..=300)
        .map(|i| {
            format!("<x:row r=\"{i}\"><x:c r=\"A{i}\"><x:v>{i}</x:v></x:c><x:c r=\"D{i}\"><x:v>1</x:v></x:c></x:row>")
        })
        .collect();
    let xml = format!("{CABECA}<x:worksheet xmlns:x=\"{NS}\"><x:sheetData>{linhas}</x:sheetData></x:worksheet>");
    let pasta = Pasta::abrir(Montador::novo().aba_xml("P", &xml).salvar(&tmp)).unwrap();
    let mut aba = pasta.planilha(0).unwrap();
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!((ind.ultima_linha, ind.colunas, ind.linhas_com_dados), (300, 4, 300));
    let p = aba.pagina(250, 5).unwrap();
    assert_eq!(p.linhas[0].valor(0), &Valor::Numero(250.0));
    assert_eq!(p.linhas[4].valor(3), &Valor::Numero(1.0));
}

#[test]
fn aba_vazia() {
    let tmp = Tmp::nova("aba-vazia");
    let pasta = com_linhas(&tmp, "");
    let mut aba = pasta.planilha(0).unwrap();
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!((ind.linhas_com_dados, ind.ultima_linha, ind.colunas, ind.marcos()), (0, 0, 0, 0));
    assert_eq!(aba.tamanho(), (0, 0));
    assert!(aba.pagina(1, 10).unwrap().linhas.is_empty());
    assert_eq!(tudo(&pasta, 0).len(), 0);
}

#[test]
fn progresso_e_interrupcao_da_indexacao() {
    let tmp = Tmp::nova("progresso");
    let pasta = com_linhas(&tmp, &linhas_geradas(20_000));
    let mut aba = pasta.planilha(0).unwrap();
    let mut chamadas: Vec<Progresso> = Vec::new();
    aba.indexar(|p| {
        chamadas.push(*p);
        true
    })
    .unwrap();
    // 20.000 linhas = 2 avisos de 8.192 em 8.192, mais o final
    assert_eq!(chamadas.len(), 3);
    assert_eq!(chamadas[0].linhas, 8192);
    assert_eq!(chamadas[1].linhas, 16384);
    assert_eq!(chamadas[2].linhas, 20_000);
    assert!(chamadas[0].bytes_lidos < chamadas[1].bytes_lidos);
    assert_eq!(chamadas[2].bytes_lidos, chamadas[2].bytes_total);
    assert!(chamadas[0].bytes_total >= chamadas[0].bytes_lidos);

    let mut outra = pasta.planilha(0).unwrap();
    let e = outra.indexar(|p| p.linhas < 10_000).err().unwrap();
    assert_eq!(e.status, 409);
    assert!(e.tem_codigo(Codigo::Cancelado), "{e}");
    assert!(outra.indice().is_none());
    // depois de interrompida, a aba continua utilizável
    conferir_pagina(&outra.pagina(15_000, 10).unwrap(), 15_000, 10);
}

#[test]
fn xml_da_aba_malformado_aparece_ao_ler() {
    let tmp = Tmp::nova("malformado");
    let xml = format!(
        "{CABECA}<worksheet xmlns=\"{NS}\"><sheetData><row r=\"1\"><c r=\"A1\"><v>1</v></c></row><row r=\"2\"><c r=\"A2\"><v>"
    );
    let pasta = Pasta::abrir(Montador::novo().aba_xml("Quebrada", &xml).salvar(&tmp)).unwrap();
    let mut it = pasta.linhas(0).unwrap();
    assert_eq!(it.next().unwrap().unwrap().valor(0), &Valor::Numero(1.0));
    let e = it.next().unwrap().err().unwrap();
    assert!(e.mensagem.contains("xl/worksheets/sheet1.xml") && e.mensagem.contains("XML"), "{e}");
    assert!(e.detalhe.is_some());
    assert!(it.next().is_none());
    let mut aba = pasta.planilha(0).unwrap();
    assert!(aba.indexar(|_| true).is_err());
    assert!(aba.pagina(1, 10).is_err());
}

#[test]
fn leitura_continua_depois_de_erro_de_pagina() {
    let tmp = Tmp::nova("erro-pagina");
    // as 300 primeiras linhas são boas; o XML acaba no meio da 301
    let xml =
        format!("{CABECA}<worksheet xmlns=\"{NS}\"><sheetData>{}<row r=\"301\"><c r=\"A301\"><v>", linhas_geradas(300));
    let pasta = Pasta::abrir(Montador::novo().aba_xml("P", &xml).salvar(&tmp)).unwrap();
    let mut aba = pasta.planilha(0).unwrap();
    conferir_pagina(&aba.pagina(1, 100).unwrap(), 1, 100);
    assert!(aba.pagina(257, 60).is_err());
    // o cursor foi descartado; as páginas boas continuam saindo (do cache e do fluxo novo)
    conferir_pagina(&aba.pagina(129, 50).unwrap(), 129, 50);
}

// ---- variações de pacote ---------------------------------------------------------------------------------

#[test]
fn prefixos_de_espaco_de_nomes_nas_partes() {
    let tmp = Tmp::nova("prefixo");
    let folha = format!(
        "{CABECA}<x:worksheet xmlns:x=\"{NS}\"><x:sheetData><x:row r=\"1\"><x:c r=\"A1\" t=\"inlineStr\"><x:is><x:t>oi</x:t></x:is></x:c>\
         <x:c r=\"B1\"><x:v>5</x:v></x:c></x:row></x:sheetData></x:worksheet>"
    );
    let livro = format!(
        "{CABECA}<x:workbook xmlns:x=\"{NS}\" xmlns:r=\"{NS_R}\"><x:sheets><x:sheet name=\"Prefixada\" sheetId=\"1\" r:id=\"rId1\"/></x:sheets></x:workbook>"
    );
    let rels = format!(
        "{CABECA}<rel:Relationships xmlns:rel=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <rel:Relationship Id=\"rId1\" Type=\"{NS_R}/worksheet\" Target=\"worksheets/sheet1.xml\"/></rel:Relationships>"
    );
    let bytes = zip_deflate(&[
        ("[Content_Types].xml", "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>"),
        ("xl/workbook.xml", &livro),
        ("xl/_rels/workbook.xml.rels", &rels),
        ("xl/worksheets/sheet1.xml", &folha),
    ]);
    let pasta = Pasta::abrir(tmp.gravar("p.xlsx", &bytes)).unwrap();
    assert_eq!(pasta.folhas()[0].nome, "Prefixada");
    assert_eq!(tudo(&pasta, 0)[0].valores(2), [texto("oi"), Valor::Numero(5.0)]);
}

#[test]
fn formato_strict_com_outros_enderecos_de_espaco_de_nomes() {
    let tmp = Tmp::nova("strict");
    let ns = "http://purl.oclc.org/ooxml/spreadsheetml/main";
    let nsr = "http://purl.oclc.org/ooxml/officeDocument/relationships";
    let folha = format!(
        "{CABECA}<worksheet xmlns=\"{ns}\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"d\"><v>2024-03-15</v></c></row></sheetData></worksheet>"
    );
    let livro = format!(
        "{CABECA}<workbook xmlns=\"{ns}\" xmlns:r=\"{nsr}\"><workbookPr date1904=\"false\"/><sheets><sheet name=\"S\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
    );
    let rels = format!(
        "{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{nsr}/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>"
    );
    let raiz = format!(
        "{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{nsr}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
    );
    let bytes = zip_deflate(&[
        (
            "[Content_Types].xml",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>",
        ),
        ("_rels/.rels", &raiz),
        ("xl/workbook.xml", &livro),
        ("xl/_rels/workbook.xml.rels", &rels),
        ("xl/worksheets/sheet1.xml", &folha),
    ]);
    let pasta = Pasta::abrir(tmp.gravar("s.xlsx", &bytes)).unwrap();
    assert_eq!(pasta.sistema_de_datas(), SistemaDatas::D1900);
    assert_eq!(tudo(&pasta, 0)[0].valor(0), &Valor::Data(data(2024, 3, 15)));
}

#[test]
fn nomes_de_parte_com_maiusculas_barra_invertida_alvo_absoluto_e_percentual() {
    let tmp = Tmp::nova("nomes");
    let folha = folha_xml("<row r=\"1\"><c r=\"A1\"><v>9</v></c></row>");
    let livro = format!(
        "{CABECA}<workbook xmlns=\"{NS}\" xmlns:r=\"{NS_R}\"><sheets><sheet name=\"A\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"B\" sheetId=\"2\" r:id=\"rId2\"/></sheets></workbook>"
    );
    let rels = format!(
        "{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{NS_R}/worksheet\" Target=\"/xl/worksheets/Sheet1.xml\"/>\
         <Relationship Id=\"rId2\" Type=\"{NS_R}/worksheet\" Target=\"worksheets/Minha%20Aba.xml\"/></Relationships>"
    );
    let bytes = zip_deflate(&[
        ("[CONTENT_TYPES].XML", "<Types/>"),
        ("XL\\Workbook.xml", &livro),
        ("xl\\_rels\\workbook.xml.rels", &rels),
        ("XL\\Worksheets\\SHEET1.XML", &folha),
        ("xl/worksheets/Minha Aba.xml", &folha),
    ]);
    let pasta = Pasta::abrir(tmp.gravar("n.xlsx", &bytes)).unwrap();
    assert_eq!(pasta.folhas().len(), 2);
    assert_eq!(tudo(&pasta, 0)[0].valor(0), &Valor::Numero(9.0));
    assert_eq!(tudo(&pasta, 1)[0].valor(0), &Valor::Numero(9.0));
}

#[test]
fn livro_sem_relacoes_usa_as_folhas_na_ordem() {
    let tmp = Tmp::nova("sem-rels");
    let folha = |n: u32| folha_xml(&format!("<row r=\"1\"><c r=\"A1\"><v>{n}</v></c></row>"));
    let livro = format!(
        "{CABECA}<workbook xmlns=\"{NS}\"><sheets><sheet name=\"A\" sheetId=\"1\"/><sheet name=\"B\" sheetId=\"2\"/></sheets></workbook>"
    );
    let bytes = zip_deflate(&[
        ("xl/workbook.xml", &livro),
        ("xl/worksheets/sheet1.xml", &folha(1)),
        ("xl/worksheets/sheet2.xml", &folha(2)),
    ]);
    let pasta = Pasta::abrir(tmp.gravar("r.xlsx", &bytes)).unwrap();
    assert_eq!(tudo(&pasta, 0)[0].valor(0), &Valor::Numero(1.0));
    assert_eq!(tudo(&pasta, 1)[0].valor(0), &Valor::Numero(2.0));
}

#[test]
fn xlsm_so_pelo_tipo_de_conteudo() {
    let tmp = Tmp::nova("xlsm");
    let folha = folha_xml("<row r=\"1\"><c r=\"A1\"><v>3</v></c></row>");
    let livro = format!(
        "{CABECA}<workbook xmlns=\"{NS}\" xmlns:r=\"{NS_R}\"><sheets><sheet name=\"M\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
    );
    let rels = format!(
        "{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{NS_R}/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>"
    );
    let tipos = "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/xl/livro-principal.xml\" ContentType=\"application/vnd.ms-excel.sheet.macroEnabled.main+xml\"/></Types>";
    let bytes = zip_deflate(&[
        ("[Content_Types].xml", tipos),
        ("xl/livro-principal.xml", &livro),
        ("xl/_rels/livro-principal.xml.rels", &rels),
        ("xl/worksheets/sheet1.xml", &folha),
        ("xl/vbaProject.bin", "binario"),
    ]);
    let pasta = Pasta::abrir(tmp.gravar("m.xlsm", &bytes)).unwrap();
    assert_eq!(pasta.folhas()[0].nome, "M");
    assert_eq!(tudo(&pasta, 0)[0].valor(0), &Valor::Numero(3.0));
}

#[test]
fn partes_com_bom_utf8() {
    let tmp = Tmp::nova("bom");
    let mut partes =
        Montador::novo().aba("Çã", "<row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>é</t></is></c></row>").partes();
    for (_, c) in &mut partes {
        c.insert(0, '\u{FEFF}');
    }
    let refs: Vec<(&str, &str)> = partes.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    let pasta = Pasta::abrir(tmp.gravar("b.xlsx", &zip_deflate(&refs))).unwrap();
    assert_eq!(pasta.folhas()[0].nome, "Çã");
    assert_eq!(tudo(&pasta, 0)[0].valor(0), &texto("é"));
}

#[test]
fn xml_de_parte_em_utf16_da_erro_claro() {
    let tmp = Tmp::nova("utf16");
    let mut partes = Montador::novo().aba("A", "").partes();
    let utf16: Vec<u8> = "<workbook/>".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let mut bom = vec![0xFF, 0xFE];
    bom.extend(utf16);
    partes.retain(|(n, _)| n != "xl/workbook.xml");
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let o = SimpleFileOptions::default();
    for (n, c) in &partes {
        z.start_file(n.as_str(), o).unwrap();
        z.write_all(c.as_bytes()).unwrap();
    }
    z.start_file("xl/workbook.xml", o).unwrap();
    z.write_all(&bom).unwrap();
    let caminho = tmp.gravar("u.xlsx", &z.finish().unwrap().into_inner());
    let e = erro_de(Pasta::abrir(caminho));
    assert!(e.mensagem.contains("xl/workbook.xml"), "{e}");
    assert!(e.detalhe.unwrap().contains("UTF-16"));
}

// ---- arquivos que não são .xlsx ---------------------------------------------------------------------------

const OLE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

fn ole_com(nomes: &[&str], deslocamento: usize) -> Vec<u8> {
    let mut b = OLE.to_vec();
    b.resize(deslocamento, 0);
    for n in nomes {
        b.extend(n.encode_utf16().flat_map(|u| u.to_le_bytes()));
        b.extend([0u8; 16]);
    }
    b.resize(b.len() + 4096, 0);
    b
}

#[test]
fn arquivo_inexistente_e_vazio() {
    let tmp = Tmp::nova("inexiste");
    let e = erro_de(Pasta::abrir(tmp.arq("nao-existe.xlsx")));
    assert_eq!(e.status, 404);
    assert!(e.mensagem.starts_with("Não foi possível abrir a planilha. Arquivo ou pasta não encontrado"), "{e}");
    let e = erro_de(Pasta::abrir(tmp.gravar("vazio.xlsx", b"")));
    assert_eq!(e.mensagem, "O arquivo está vazio; não é uma planilha do Excel (.xlsx).");
}

#[test]
fn planilha_protegida_por_senha() {
    let tmp = Tmp::nova("senha");
    let bytes = ole_com(&["Root Entry", "EncryptionInfo", "EncryptedPackage"], 600);
    let e = erro_de(Pasta::abrir(tmp.gravar("p.xlsx", &bytes)));
    assert!(e.mensagem.starts_with("A planilha está protegida por senha"), "{e}");
    assert!(e.mensagem.contains("Criptografar com Senha"));
    // o nome do fluxo cortado entre dois blocos de leitura de 1 MB também é achado
    let bytes = ole_com(&["EncryptedPackage"], 1024 * 1024 - 10);
    let e = erro_de(Pasta::abrir(tmp.gravar("p2.xlsx", &bytes)));
    assert!(e.mensagem.starts_with("A planilha está protegida por senha"), "{e}");
    // e bem no fim de um arquivo grande
    let bytes = ole_com(&["EncryptedPackage"], 3 * 1024 * 1024 + 123);
    assert!(erro_de(Pasta::abrir(tmp.gravar("p3.xlsx", &bytes))).mensagem.starts_with("A planilha está protegida"));
}

#[test]
fn planilha_xls_antiga() {
    let tmp = Tmp::nova("xls");
    for nomes in [&["Root Entry", "Workbook"][..], &["Root Entry", "Book"]] {
        let e = erro_de(Pasta::abrir(tmp.gravar("a.xls", &ole_com(nomes, 700))));
        assert!(e.mensagem.starts_with("Este é o formato antigo do Excel (.xls"), "{e}");
        assert!(e.mensagem.contains("Salvar Como"));
    }
}

#[test]
fn outro_documento_ole_nao_e_planilha() {
    let tmp = Tmp::nova("ole-outro");
    let e = erro_de(Pasta::abrir(tmp.gravar("a.doc", &ole_com(&["Root Entry", "WordDocument"], 700))));
    assert!(e.mensagem.contains("documento do Office no formato antigo"), "{e}");
}

#[test]
fn texto_html_e_lixo_nao_sao_planilha() {
    let tmp = Tmp::nova("lixo");
    for (nome, dados) in [
        ("a.xlsx", &b"a;b;c\n1;2;3\n"[..]),
        ("b.xlsx", b"<html><body><table><tr><td>1</td></tr></table></body></html>"),
        ("c.xlsx", b"<?xml version=\"1.0\"?><Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\"/>"),
        ("d.xlsx", &[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]),
        ("e.xlsx", b"x"),
    ] {
        let e = erro_de(Pasta::abrir(tmp.gravar(nome, dados)));
        assert!(e.mensagem.starts_with("O arquivo não é uma planilha do Excel (.xlsx)"), "{nome}: {e}");
    }
}

#[test]
fn zip_cortado_ou_corrompido() {
    let tmp = Tmp::nova("corrompido");
    let bom = Montador::novo().aba("A", &linhas_geradas(50)).bytes();
    // cortado no meio: perdeu o diretório central
    let e = erro_de(Pasta::abrir(tmp.gravar("c.xlsx", &bom[..bom.len() / 2])));
    assert!(e.mensagem.starts_with("O arquivo está corrompido ou incompleto"), "{e}");
    assert!(e.detalhe.is_some());
    // só a assinatura
    let e = erro_de(Pasta::abrir(tmp.gravar("d.xlsx", b"PK\x03\x04")));
    assert!(e.mensagem.starts_with("O arquivo está corrompido"), "{e}");
    // sem o final do diretório central
    let e = erro_de(Pasta::abrir(tmp.gravar("e.xlsx", &bom[..bom.len() - 10])));
    assert!(e.mensagem.starts_with("O arquivo está corrompido"), "{e}");
}

#[test]
fn byte_trocado_numa_parte_falha_na_soma_de_verificacao() {
    let tmp = Tmp::nova("crc");
    let partes = Montador::novo()
        .aba(
            "Aba de teste",
            "<row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>conteudoUnicoParaAchar</t></is></c></row>",
        )
        .partes();
    let refs: Vec<(&str, &str)> = partes.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    let mut bytes = zip_de(&refs, CompressionMethod::Stored);
    // troca uma letra do texto da célula, sem mudar o tamanho (e sem quebrar o XML)
    let marca = b"conteudoUnicoParaAchar";
    let pos = bytes.windows(marca.len()).position(|j| j == marca).unwrap();
    bytes[pos] = b'C';
    let caminho = tmp.gravar("crc.xlsx", &bytes);
    let pasta = Pasta::abrir(&caminho).unwrap(); // as partes pequenas estão boas
    let mut it = pasta.linhas(0).unwrap();
    let e = it.find_map(Result::err).expect("devia falhar na soma de verificação");
    assert!(e.mensagem.starts_with("O arquivo está corrompido ou incompleto"), "{e}");
    assert!(e.detalhe.unwrap().contains("xl/worksheets/sheet1.xml"));
    // uma parte pequena com a soma errada: o erro vem ao abrir
    let mut bytes = zip_de(&refs, CompressionMethod::Stored);
    let marca = b"Aba de teste";
    let pos = bytes.windows(marca.len()).position(|j| j == marca).unwrap();
    bytes[pos] = b'a';
    let e = erro_de(Pasta::abrir(tmp.gravar("crc2.xlsx", &bytes)));
    assert!(e.mensagem.starts_with("O arquivo está corrompido"), "{e}");
}

#[test]
fn zips_de_outros_formatos() {
    let tmp = Tmp::nova("outros-zips");
    let docx = zip_deflate(&[
        (
            "[Content_Types].xml",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>",
        ),
        (
            "_rels/.rels",
            &format!(
                "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{NS_R}/officeDocument\" Target=\"word/document.xml\"/></Relationships>"
            ),
        ),
        ("word/document.xml", "<w:document/>"),
    ]);
    let e = erro_de(Pasta::abrir(tmp.gravar("a.xlsx", &docx)));
    assert_eq!(e.mensagem, "O arquivo é um documento do Word (.docx), não uma planilha do Excel (.xlsx).");

    let pptx = zip_deflate(&[("[Content_Types].xml", "<Types/>"), ("ppt/presentation.xml", "<p/>")]);
    let e = erro_de(Pasta::abrir(tmp.gravar("b.xlsx", &pptx)));
    assert!(e.mensagem.contains(".pptx"), "{e}");

    let ods = zip_de(
        &[
            ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
            ("content.xml", "<office:document-content/>"),
        ],
        CompressionMethod::Stored,
    );
    let e = erro_de(Pasta::abrir(tmp.gravar("c.xlsx", &ods)));
    assert!(e.mensagem.contains(".ods"), "{e}");

    let odt = zip_de(&[("mimetype", "application/vnd.oasis.opendocument.text")], CompressionMethod::Stored);
    assert!(erro_de(Pasta::abrir(tmp.gravar("d.xlsx", &odt))).mensagem.contains("OpenDocument"));

    let qualquer = zip_deflate(&[("leia-me.txt", "oi"), ("dados/a.csv", "1,2")]);
    let e = erro_de(Pasta::abrir(tmp.gravar("e.xlsx", &qualquer)));
    assert!(e.mensagem.starts_with("O arquivo é um pacote zip, mas não é uma pasta de trabalho do Excel"), "{e}");
}

#[test]
fn xlsb_binario() {
    let tmp = Tmp::nova("xlsb");
    let bytes = zip_deflate(&[
        (
            "[Content_Types].xml",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/xl/workbook.bin\" ContentType=\"application/vnd.ms-excel.sheet.binary.macroEnabled.main\"/></Types>",
        ),
        (
            "_rels/.rels",
            &format!(
                "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{NS_R}/officeDocument\" Target=\"xl/workbook.bin\"/></Relationships>"
            ),
        ),
        ("xl/workbook.bin", "binario"),
    ]);
    let e = erro_de(Pasta::abrir(tmp.gravar("a.xlsb", &bytes)));
    assert!(e.mensagem.starts_with("O formato binário do Excel (.xlsb) não é suportado"), "{e}");
    // sem os tipos de conteúdo, só pelo nome da parte
    let so_nome = zip_deflate(&[("xl/workbook.bin", "binario")]);
    assert!(erro_de(Pasta::abrir(tmp.gravar("b.xlsb", &so_nome))).mensagem.contains(".xlsb"));
}

#[test]
fn livro_sem_abas_ou_com_aba_sem_conteudo() {
    let tmp = Tmp::nova("sem-abas");
    let livro = format!("{CABECA}<workbook xmlns=\"{NS}\"><sheets/></workbook>");
    let e = erro_de(Pasta::abrir(tmp.gravar("a.xlsx", &zip_deflate(&[("xl/workbook.xml", &livro)]))));
    assert_eq!(e.mensagem, "A pasta de trabalho não tem nenhuma planilha.");

    // a aba aponta para uma parte que não está no zip
    let mut partes = Montador::novo().aba("Some", "").partes();
    partes.retain(|(n, _)| n != "xl/worksheets/sheet1.xml");
    let refs: Vec<(&str, &str)> = partes.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    let pasta = Pasta::abrir(tmp.gravar("b.xlsx", &zip_deflate(&refs))).unwrap();
    assert_eq!(pasta.folhas()[0].parte, "");
    let e = pasta.planilha(0).err().unwrap();
    assert_eq!(e.mensagem, "A planilha está corrompida: falta o conteúdo da aba «Some» no arquivo.");
    assert!(pasta.linhas(0).is_err());
}

#[test]
fn workbook_com_xml_malformado() {
    let tmp = Tmp::nova("wb-ruim");
    let bytes = zip_deflate(&[("xl/workbook.xml", "<workbook><sheets><sheet name=\"x\"")]);
    let e = erro_de(Pasta::abrir(tmp.gravar("a.xlsx", &bytes)));
    assert!(e.mensagem.contains("xl/workbook.xml") && e.mensagem.contains("não é um XML válido"), "{e}");
}

#[test]
fn mensagem_de_senha_para_entrada_do_zip_criptografada() {
    let e = erros::da_io(&std::io::Error::other("Password required to decrypt file"), "xl/workbook.xml");
    assert!(e.mensagem.starts_with("A planilha está protegida por senha"));
    let e = erros::do_zip(&zip::result::ZipError::UnsupportedArchive("Password required to decrypt file"), "x");
    assert!(e.mensagem.starts_with("A planilha está protegida por senha"));
    let e = erros::do_zip(&zip::result::ZipError::FileNotFound, "xl/styles.xml");
    assert!(e.mensagem.contains("xl/styles.xml"));
}

// ---- garantias gerais ------------------------------------------------------------------------------------

#[test]
fn tipos_podem_passar_entre_linhas_de_execucao() {
    fn envia<T: Send>() {}
    fn compartilha<T: Send + Sync>() {}
    compartilha::<Pasta>();
    envia::<Planilha>();
    envia::<Linhas>();
    compartilha::<Linha>();
    compartilha::<Pagina>();
    compartilha::<MetaFolha>();
    compartilha::<Formato>();
}

#[test]
fn varias_planilhas_lendo_ao_mesmo_tempo_da_mesma_pasta() {
    let tmp = Tmp::nova("threads");
    let pasta = com_linhas(&tmp, &linhas_geradas(1500));
    let juntas: Vec<_> = (0..4u32)
        .map(|k| {
            let pasta = pasta.clone();
            std::thread::spawn(move || {
                let mut aba = pasta.planilha(0).unwrap();
                aba.indexar(|_| true).unwrap();
                for passo in 0..10u32 {
                    let primeira = 1 + (k * 311 + passo * 97) % 1400;
                    conferir_pagina(&aba.pagina(primeira, 50).unwrap(), primeira, 50);
                }
            })
        })
        .collect();
    for j in juntas {
        j.join().unwrap();
    }
}

#[test]
fn soltar_o_fluxo_no_meio_nao_trava_nem_prende_o_arquivo() {
    let tmp = Tmp::nova("soltar");
    let pasta = com_linhas(&tmp, &linhas_geradas(30_000));
    let caminho = pasta.caminho().to_path_buf();
    {
        let mut it = pasta.linhas(0).unwrap();
        assert!(it.next().is_some());
        // solta o iterador com o resto do XML (alguns MB) por ler
    }
    {
        let mut aba = pasta.planilha(0).unwrap();
        conferir_pagina(&aba.pagina(1, 10).unwrap(), 1, 10);
    }
    drop(pasta);
    // nenhuma linha de execução ficou com o arquivo aberto: dá para apagar
    fs::remove_file(&caminho).unwrap();
}

#[test]
fn le_aba_grande_sem_carregar_o_xml_na_memoria() {
    // 100 mil linhas (~9 MB de XML) em fluxo: contagem e soma conferem
    let tmp = Tmp::nova("grande");
    let pasta = com_linhas(&tmp, &linhas_geradas(100_000));
    let (mut n, mut soma) = (0u64, 0.0);
    for l in pasta.linhas(0).unwrap() {
        let l = l.unwrap();
        n += 1;
        soma += l.valor(0).como_numero().unwrap();
    }
    assert_eq!(n, 100_000);
    assert_eq!(soma, 100_000.0 * 100_001.0 / 2.0);
    let mut aba = pasta.planilha(0).unwrap();
    let ind = aba.indexar(|_| true).unwrap().clone();
    assert_eq!(ind.ultima_linha, 100_000);
    conferir_pagina(&aba.pagina(99_990, 11).unwrap(), 99_990, 11);
    conferir_pagina(&aba.pagina(50_001, 100).unwrap(), 50_001, 100);
}

#[test]
fn exibir_celulas_do_exportador_com_numeros_e_textos() {
    let tmp = Tmp::nova("exibir");
    let pasta = Pasta::abrir(exportar_uma(
        &tmp,
        &["n", "t", "r"],
        vec![vec![ValorBanco::Inteiro(1_234_567), ValorBanco::Texto("x".into()), ValorBanco::Real(0.5)]],
    ))
    .unwrap();
    let l = &tudo(&pasta, 0)[1];
    let v: Vec<String> = l.celulas.iter().map(|c| pasta.exibir(c)).collect();
    assert_eq!(v, ["1234567", "x", "0,5"]);
}

// ---- desempenho (rodar com --ignored --nocapture) -----------------------------------------------------

/// 1 milhão de linhas × 10 colunas gravadas pelo exportador, depois lidas: índice, páginas e leitura em fluxo.
#[test]
#[ignore = "benchmark: milhões de células; rode com --release -- --ignored --nocapture"]
fn benchmark_um_milhao_de_linhas_por_dez_colunas() {
    let tmp = Tmp::nova("bench");
    let destino = tmp.arq("grande.xlsx");
    let colunas =
        cols(&["id", "nome", "valor", "quantidade", "categoria", "obs", "ativo", "taxa", "codigo", "descricao"]);
    let t0 = Instant::now();
    let linhas = (0..1_000_000i64).map(|i| -> Resultado<Vec<ValorBanco>> {
        Ok(vec![
            ValorBanco::Inteiro(i),
            ValorBanco::Texto(format!("Cliente número {i}")),
            ValorBanco::Real(i as f64 * 1.37),
            ValorBanco::Inteiro(i % 97),
            ValorBanco::Texto(["A", "B", "C", "D"][(i % 4) as usize].to_string()),
            ValorBanco::Texto(format!("observação da linha {i} com acentuação")),
            ValorBanco::Inteiro(i % 2),
            ValorBanco::Real((i % 1000) as f64 / 1000.0),
            ValorBanco::Texto(format!("COD-{:07}", i)),
            ValorBanco::Nulo,
        ])
    });
    exportar_linhas(
        &colunas,
        linhas,
        FormatoExp::Xlsx,
        &destino,
        &Opcoes::default(),
        "dados",
        Acompanhamento::default(),
    )
    .unwrap();
    let tamanho = fs::metadata(&destino).unwrap().len();
    println!("gravado pelo exportador em {:?}: {:.1} MB", t0.elapsed(), tamanho as f64 / 1e6);

    let t = Instant::now();
    let pasta = Pasta::abrir(&destino).unwrap();
    println!("Pasta::abrir: {:?}", t.elapsed());
    let t = Instant::now();
    let mut aba = pasta.planilha(0).unwrap();
    println!("planilha() (cabeçalho do XML): {:?}", t.elapsed());
    let t = Instant::now();
    let primeira = aba.pagina(1, 100).unwrap();
    println!("primeira página, ainda sem índice: {:?} ({} linhas)", t.elapsed(), primeira.linhas.len());

    let t = Instant::now();
    let ind = aba.indexar(|_| true).unwrap().clone();
    let indexar = t.elapsed();
    println!(
        "INDEXAR 1.000.001 linhas x 10 colunas: {:?} ({:.1} MB de XML, {} marcos, {:.0} MB/s de XML, {:.1} milhões de linhas/s)",
        indexar,
        ind.bytes_xml as f64 / 1e6,
        ind.marcos(),
        ind.bytes_xml as f64 / 1e6 / indexar.as_secs_f64(),
        ind.linhas_com_dados as f64 / 1e6 / indexar.as_secs_f64()
    );
    assert_eq!(ind.ultima_linha, 1_000_001);
    assert_eq!(aba.tamanho(), (1_000_001, 10));

    // para saber quem segura a indexação: só descomprimir e entregar os bytes, sem interpretar nada
    let t = Instant::now();
    let mut fluxo = crate::fluxo::Fluxo::abrir(destino.clone(), pasta.folhas()[0].parte.clone());
    let bytes = std::io::copy(&mut fluxo, &mut std::io::sink()).unwrap();
    println!("só descomprimir e entregar ({:.1} MB): {:?}", bytes as f64 / 1e6, t.elapsed());
    drop(fluxo);

    for (nome, primeira) in [("começo", 1u32), ("meio", 500_001), ("perto do fim", 999_900), ("começo de novo", 1)] {
        let t = Instant::now();
        let p = aba.pagina(primeira, 100).unwrap();
        println!("página no {nome} (linha {primeira}): {:?}", t.elapsed());
        assert_eq!(p.linhas.len(), 100.min(1_000_002 - primeira as usize));
    }
    let t = Instant::now();
    for k in 0..200u32 {
        aba.pagina(1 + k * 100, 100).unwrap();
    }
    println!("200 páginas seguidas de 100 linhas (rolagem para baixo, fluxo reaproveitado): {:?}", t.elapsed());
    println!("aberturas do fluxo no total: {}", aba.aberturas_do_fluxo());

    let t = Instant::now();
    let (mut n, mut texto_total) = (0u64, 0usize);
    for l in pasta.linhas(0).unwrap() {
        let l = l.unwrap();
        n += 1;
        texto_total += l.celulas.iter().filter_map(|c| c.valor.como_texto()).map(str::len).sum::<usize>();
    }
    let completo = t.elapsed();
    println!(
        "LER TUDO em fluxo (Pasta::linhas): {n} linhas em {completo:?} ({:.0} MB/s de XML), {texto_total} bytes de texto",
        ind.bytes_xml as f64 / 1e6 / completo.as_secs_f64()
    );
    assert_eq!(n, 1_000_001);
}
