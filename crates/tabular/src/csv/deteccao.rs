//! A detecção do formato de um CSV pelos primeiros bytes: separador, fim de linha, estilo das aspas, decimal e
//! se a primeira linha é o cabeçalho. Tudo é heurística sobre uma amostra (os primeiros 256 KB, no máximo 100
//! registros), e a pessoa corrige nas Propriedades quando erra.

use super::analise::{Campo, decodificar_campo, proximo_registro, registros_do_trecho};
use super::codificacao::{Codificacao, detectar_codificacao};
use super::formato::{EstiloAspas, FimDeLinha, Formato, SEPARADORES};
use crate::numero::{TipoDetectado, classificar, tipo_da_coluna};

/// Quantos registros da amostra a detecção olha.
pub const REGISTROS_DA_AMOSTRA: usize = 100;

/// Um registro da amostra: onde estão os campos e a quebra de linha que o fecha (se tiver).
struct Amostrado {
    campos: Vec<Campo>,
    terminador: Option<FimDeLinha>,
}

/// Os primeiros registros do trecho. O registro cortado no fim de um trecho truncado fica de fora.
fn amostrar(bytes: &[u8], sep: u8, truncado: bool) -> Vec<Amostrado> {
    let mut saida = Vec::new();
    let mut pos = 0;
    while saida.len() < REGISTROS_DA_AMOSTRA && pos < bytes.len() {
        let mut campos = Vec::new();
        let Some(r) = proximo_registro(bytes, pos, sep, !truncado, Some(&mut campos)) else {
            break;
        };
        let terminador = bytes.get(r.fim_conteudo..r.fim).and_then(FimDeLinha::de_bytes);
        saida.push(Amostrado { campos, terminador });
        pos = r.fim;
    }
    saida
}

/// O separador cujas colunas mais se repetem de registro em registro (e que separa algo: duas colunas ou
/// mais). No empate, o de mais colunas e, depois, a ordem de [`SEPARADORES`]. Sem nenhum candidato (uma coluna
/// só), `;`, o do Exportar.
pub fn detectar_separador(bytes: &[u8], truncado: bool) -> char {
    // (frequência da contagem mais comum, registros, colunas dessa contagem)
    let mut melhor: Option<(char, usize, usize, usize)> = None;
    for sep in SEPARADORES {
        let amostra = amostrar(bytes, u8::try_from(u32::from(sep)).unwrap_or(b';'), truncado);
        if amostra.is_empty() {
            continue;
        }
        let mut contagens: Vec<(usize, usize)> = Vec::new();
        for a in &amostra {
            match contagens.iter_mut().find(|(n, _)| *n == a.campos.len()) {
                Some((_, vezes)) => *vezes += 1,
                None => contagens.push((a.campos.len(), 1)),
            }
        }
        // a contagem mais comum; no empate, a de mais colunas
        let Some(&(colunas, vezes)) = contagens.iter().max_by_key(|(n, v)| (*v, *n)) else {
            continue;
        };
        if colunas < 2 {
            continue;
        }
        let total = amostra.len();
        let melhor_que_o_atual = match melhor {
            None => true,
            Some((_, v0, t0, c0)) => {
                // vezes/total contra v0/t0, sem dividir
                let a = vezes * t0;
                let b = v0 * total;
                a > b || (a == b && colunas > c0)
            }
        };
        if melhor_que_o_atual {
            melhor = Some((sep, vezes, total, colunas));
        }
    }
    melhor.map_or(';', |(sep, ..)| sep)
}

/// A quebra de linha mais usada nos registros da amostra; sem nenhuma (uma linha só, sem quebra), CRLF.
pub fn detectar_fim_de_linha(bytes: &[u8], sep: char, truncado: bool) -> FimDeLinha {
    let amostra = amostrar(bytes, u8::try_from(u32::from(sep)).unwrap_or(b';'), truncado);
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    for a in &amostra {
        match a.terminador {
            Some(FimDeLinha::Crlf) => crlf += 1,
            Some(FimDeLinha::Lf) => lf += 1,
            Some(FimDeLinha::Cr) => cr += 1,
            None => {}
        }
    }
    if crlf == 0 && lf == 0 && cr == 0 {
        return FimDeLinha::Crlf;
    }
    if crlf >= lf && crlf >= cr {
        FimDeLinha::Crlf
    } else if lf >= cr {
        FimDeLinha::Lf
    } else {
        FimDeLinha::Cr
    }
}

/// O estilo das aspas da amostra: todos os campos com texto entre aspas → `Todas`; os de texto entre aspas e
/// os números soltos → `NaoNumericos`; senão, `Minimas`.
pub fn detectar_aspas(bytes: &[u8], sep: char, truncado: bool, cod: Codificacao, decimal: char) -> EstiloAspas {
    let amostra = amostrar(bytes, u8::try_from(u32::from(sep)).unwrap_or(b';'), truncado);
    let (mut todos_citados, mut texto_citado_numero_solto) = (true, true);
    let (mut algum_citado, mut algum_numero_solto) = (false, false);
    for a in &amostra {
        for c in &a.campos {
            let texto = decodificar_campo(bytes, *c, cod);
            if texto.is_empty() {
                continue;
            }
            let numero = matches!(classificar(&texto, decimal), TipoDetectado::Inteiro | TipoDetectado::Decimal);
            if c.citado {
                algum_citado = true;
            } else {
                todos_citados = false;
                if numero {
                    algum_numero_solto = true;
                }
            }
            // no estilo "não numéricos", o texto vem entre aspas e o número, solto
            if c.citado == numero {
                texto_citado_numero_solto = false;
            }
        }
    }
    if !algum_citado {
        EstiloAspas::Minimas
    } else if todos_citados {
        EstiloAspas::Todas
    } else if texto_citado_numero_solto && algum_numero_solto {
        EstiloAspas::NaoNumericos
    } else {
        EstiloAspas::Minimas
    }
}

/// O decimal que os números da amostra usam: `,` ou `.`, o que tiver mais campos que só fazem sentido como
/// número com ele (`12,5`, `1.234,56`). Sem evidência, `,` — a não ser que o separador seja a vírgula.
pub fn detectar_decimal(registros: &[Vec<String>], sep: char) -> char {
    let (mut virgula, mut ponto) = (0usize, 0usize);
    for campo in registros.iter().flatten() {
        let t = campo.trim();
        if t.bytes().any(|b| !(b.is_ascii_digit() || b == b'.' || b == b',' || b == b'-' || b == b'+')) {
            continue;
        }
        if crate::numero::analisar_numero(t, ',').is_some() && t.contains(',') {
            virgula += 1;
        }
        if crate::numero::analisar_numero(t, '.').is_some() && t.contains('.') {
            ponto += 1;
        }
    }
    if virgula > ponto || (virgula == ponto && sep != ',') { ',' } else { '.' }
}

/// A primeira linha parece o cabeçalho? Os registros são os primeiros do arquivo, decodificados.
///
/// Cada coluna dá um voto a favor (nome em texto sobre valores que são número, data ou booleano) ou contra
/// (a primeira linha tem o mesmo tipo dos valores de baixo; ou, em coluna de texto, o "nome" também aparece
/// nos valores). Nomes repetidos na primeira linha votam contra. Sem votos, é cabeçalho se todas as células
/// da primeira linha são texto não vazio, e um arquivo de uma linha só também.
pub fn detectar_cabecalho(registros: &[Vec<String>], decimal: char) -> bool {
    let Some(primeira) = registros.first() else {
        return false;
    };
    if primeira.is_empty() {
        return false;
    }
    let resto = registros.get(1..).unwrap_or_default();
    let tipos: Vec<TipoDetectado> = primeira.iter().map(|c| classificar(c, decimal)).collect();
    let tudo_texto = tipos.iter().all(|t| *t == TipoDetectado::Texto);
    if resto.is_empty() {
        return tudo_texto;
    }
    let (mut a_favor, mut contra) = (0i32, 0i32);
    for (j, (celula, tipo)) in primeira.iter().zip(&tipos).enumerate() {
        let valores: Vec<&str> = resto.iter().filter_map(|r| r.get(j)).map(String::as_str).collect();
        let dominante = tipo_da_coluna(valores.iter().copied(), decimal);
        match (*tipo, dominante) {
            (TipoDetectado::Vazio, _) | (_, TipoDetectado::Vazio) => {}
            (TipoDetectado::Texto, d) if d.e_tipado() => a_favor += 2,
            (t, TipoDetectado::Texto) if t.e_tipado() => contra += 1,
            (t, d) if t.e_tipado() && d.e_tipado() => contra += 2,
            (TipoDetectado::Texto, TipoDetectado::Texto)
                if valores.iter().any(|v| v.trim().eq_ignore_ascii_case(celula.trim())) =>
            {
                contra += 1;
            }
            _ => {}
        }
    }
    let nao_vazias: Vec<String> =
        primeira.iter().filter(|c| !c.trim().is_empty()).map(|c| c.trim().to_lowercase()).collect();
    let mut unicas = nao_vazias.clone();
    unicas.sort();
    unicas.dedup();
    if unicas.len() != nao_vazias.len() {
        contra += 1;
    }
    if a_favor != contra {
        return a_favor > contra;
    }
    tudo_texto
}

/// Tudo o que a amostra diz do formato. `bytes` é o começo do arquivo, sem o BOM; `truncado` diz que o arquivo
/// é maior que o trecho; `bom` diz que o arquivo começava com o BOM.
pub fn detectar_formato(bytes: &[u8], truncado: bool, bom: bool) -> Formato {
    let codificacao = if bom { Codificacao::Utf8ComBom } else { detectar_codificacao(bytes, truncado) };
    let sep = detectar_separador(bytes, truncado);
    let sep_byte = u8::try_from(u32::from(sep)).unwrap_or(b';');
    let registros = registros_do_trecho(bytes, sep_byte, truncado, REGISTROS_DA_AMOSTRA, codificacao);
    let decimal = detectar_decimal(&registros, sep);
    Formato {
        codificacao,
        separador: sep,
        fim_de_linha: detectar_fim_de_linha(bytes, sep, truncado),
        aspas: detectar_aspas(bytes, sep, truncado, codificacao, decimal),
        cabecalho: detectar_cabecalho(&registros, decimal),
        decimal,
        termina_em_quebra: !truncado && (bytes.ends_with(b"\n") || bytes.ends_with(b"\r") || bytes.is_empty()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn sep(texto: &str) -> char {
        detectar_separador(texto.as_bytes(), false)
    }

    #[test]
    fn separador_de_cada_tipo() {
        assert_eq!(sep("a;b;c\n1;2;3\n4;5;6\n"), ';');
        assert_eq!(sep("a,b,c\n1,2,3\n4,5,6\n"), ',');
        assert_eq!(sep("a\tb\tc\n1\t2\t3\n"), '\t');
        assert_eq!(sep("a|b|c\n1|2|3\n"), '|');
    }

    #[test]
    fn separador_com_decimal_virgula_no_arquivo_de_ponto_e_virgula() {
        let t = "produto;preco\nCaderno;12,90\nCaneta;4,50\nMochila;129,90\n";
        assert_eq!(sep(t), ';');
    }

    #[test]
    fn separador_com_virgula_dentro_de_aspas() {
        let t = "nome,obs\n\"Silva, Ana\",ok\n\"Souza, Bia\",ok\n";
        assert_eq!(sep(t), ',');
        let t = "nome;obs\n\"a, b, c\";ok\n\"d, e, f\";ok\n";
        assert_eq!(sep(t), ';');
    }

    #[test]
    fn separador_sem_candidato_e_ponto_e_virgula() {
        assert_eq!(sep("so uma coluna\noutra linha\n"), ';');
        assert_eq!(sep(""), ';');
        assert_eq!(sep("a\n"), ';');
    }

    #[test]
    fn separador_desempata_por_consistencia() {
        // a vírgula aparece em todas as linhas mas com contagens diferentes; o ; é constante
        let t = "a;b;c\nx, y;1;2\nz;3,5;4\nw, v, u;5;6\n";
        assert_eq!(sep(t), ';');
    }

    #[test]
    fn separador_com_amostra_truncada_ignora_o_registro_cortado() {
        let t = "a;b;c\n1;2;3\n4;5";
        assert_eq!(detectar_separador(t.as_bytes(), true), ';');
    }

    #[test]
    fn fim_de_linha() {
        assert_eq!(detectar_fim_de_linha(b"a;b\r\nc;d\r\n", ';', false), FimDeLinha::Crlf);
        assert_eq!(detectar_fim_de_linha(b"a;b\nc;d\n", ';', false), FimDeLinha::Lf);
        assert_eq!(detectar_fim_de_linha(b"a;b\rc;d\r", ';', false), FimDeLinha::Cr);
        assert_eq!(detectar_fim_de_linha(b"a;b\nc;d\ne;f\r\n", ';', false), FimDeLinha::Lf, "a mais usada");
        assert_eq!(detectar_fim_de_linha(b"a;b", ';', false), FimDeLinha::Crlf, "sem quebra: o do Windows");
        assert_eq!(detectar_fim_de_linha(b"", ';', false), FimDeLinha::Crlf);
        assert_eq!(
            detectar_fim_de_linha(b"a;\"x\r\ny\"\nb;c\n", ';', false),
            FimDeLinha::Lf,
            "quebra dentro de aspas não conta"
        );
    }

    #[test]
    fn aspas() {
        let d = |t: &str| detectar_aspas(t.as_bytes(), ';', false, Codificacao::Utf8, ',');
        assert_eq!(d("a;b\n1;2\n"), EstiloAspas::Minimas);
        assert_eq!(d("a;\"b;c\"\n1;2\n"), EstiloAspas::Minimas, "aspas só onde precisa");
        assert_eq!(d("\"a\";\"b\"\n\"1\";\"2\"\n"), EstiloAspas::Todas);
        assert_eq!(d("\"a\";\"b\"\n1;2,5\n\"x\";3\n"), EstiloAspas::NaoNumericos);
        assert_eq!(d("\"a\";\"b\"\n\"x\";\"y\"\n"), EstiloAspas::Todas, "sem número, todas");
        assert_eq!(d("\"a\";b\n1;2\n"), EstiloAspas::Minimas);
        assert_eq!(d("\"a\";\"\"\n\"x\";\n"), EstiloAspas::Todas, "campos vazios não contam");
        assert_eq!(d(""), EstiloAspas::Minimas);
    }

    #[test]
    fn decimal() {
        let r = |t: &[&[&str]]| -> Vec<Vec<String>> {
            t.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
        };
        assert_eq!(detectar_decimal(&r(&[&["preco"], &["12,50"], &["3,2"]]), ';'), ',');
        assert_eq!(detectar_decimal(&r(&[&["preco"], &["12.50"], &["3.2"]]), ';'), '.');
        assert_eq!(detectar_decimal(&r(&[&["x"], &["1.234,56"], &["12.5"]]), ';'), ',', "empate: vale o padrão");
        assert_eq!(detectar_decimal(&r(&[&["x"], &["10"], &["20"]]), ';'), ',', "sem evidência, vírgula");
        assert_eq!(
            detectar_decimal(&r(&[&["x"], &["10"], &["20"]]), ','),
            '.',
            "sem evidência com separador vírgula: ponto"
        );
        assert_eq!(detectar_decimal(&r(&[&["x"], &["1,234.56"], &["7.5"]]), ','), '.');
        assert_eq!(detectar_decimal(&[], ';'), ',');
    }

    fn cab(linhas: &[&[&str]]) -> bool {
        let r: Vec<Vec<String>> = linhas.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect();
        detectar_cabecalho(&r, ',')
    }

    #[test]
    fn cabecalho_com_valores_numericos_embaixo() {
        assert!(cab(&[&["codigo", "preco"], &["1", "2,5"], &["2", "3,5"]]));
        assert!(cab(&[&["data", "regiao", "quantidade"], &["2025-01-02", "Sul", "1"], &["2025-01-03", "Norte", "8"]]));
    }

    #[test]
    fn sem_cabecalho_quando_a_primeira_linha_e_dado() {
        assert!(!cab(&[&["1", "2,5"], &["2", "3,5"], &["3", "4,5"]]));
        assert!(!cab(&[&["2025-01-02", "10"], &["2025-01-03", "20"]]));
        assert!(
            !cab(&[&["1", "Sul"], &["2", "Norte"]]),
            "uma coluna numérica com número na primeira linha pesa contra"
        );
    }

    #[test]
    fn so_texto_vale_cabecalho_se_os_nomes_nao_se_repetem() {
        assert!(cab(&[&["nome", "cidade"], &["Ana", "Recife"], &["Bia", "Natal"]]));
        assert!(!cab(&[&["Sul", "Norte"], &["Sul", "Norte"], &["Norte", "Sul"]]), "os 'nomes' repetem nos valores");
        assert!(!cab(&[&["a", "a"], &["b", "c"]]), "nomes repetidos na primeira linha");
    }

    #[test]
    fn cabecalho_em_casos_de_borda() {
        assert!(!cab(&[]));
        assert!(cab(&[&["nome", "cidade"]]), "só o cabeçalho (tabela vazia exportada)");
        assert!(!cab(&[&["1", "2"]]), "uma linha de números");
        assert!(!cab(&[&["nome", ""]]), "uma linha só com célula vazia");
        assert!(cab(&[&["", "preco"], &["a", "1"], &["b", "2"]]), "primeira célula vazia (índice de planilha)");
    }

    #[test]
    fn formato_completo_de_um_export_do_databox() {
        let t = "\u{FEFF}codigo;produto;preco\r\n004718;Caderno universitário;24,90\r\n004725;Caneta gel;4,50\r\n";
        let f = detectar_formato(&t.as_bytes()[3..], false, true);
        assert_eq!(f.codificacao, Codificacao::Utf8ComBom);
        assert_eq!(f.separador, ';');
        assert_eq!(f.fim_de_linha, FimDeLinha::Crlf);
        assert_eq!(f.aspas, EstiloAspas::Minimas);
        assert_eq!(f.decimal, ',');
        assert!(f.cabecalho && f.termina_em_quebra);
    }

    #[test]
    fn formato_de_um_csv_do_excel_em_windows_1252() {
        let mut b = b"nome;cidade;valor\r\nJos\xE9;S\xE3o Paulo;1.234,50\r\nAna;Bel\xE9m;99,90".to_vec();
        let f = detectar_formato(&b, false, false);
        assert_eq!(f.codificacao, Codificacao::Windows1252);
        assert_eq!(f.separador, ';');
        assert_eq!(f.decimal, ',');
        assert!(!f.termina_em_quebra);
        b.extend_from_slice(b"\r\n");
        assert!(detectar_formato(&b, false, false).termina_em_quebra);
    }

    #[test]
    fn formato_de_csv_unix_com_virgula_e_ponto() {
        let t = "id,name,price\n1,\"Smith, J\",12.5\n2,Lee,7.25\n";
        let f = detectar_formato(t.as_bytes(), false, false);
        assert_eq!((f.separador, f.decimal, f.fim_de_linha), (',', '.', FimDeLinha::Lf));
        assert_eq!(f.codificacao, Codificacao::Utf8);
    }
}
