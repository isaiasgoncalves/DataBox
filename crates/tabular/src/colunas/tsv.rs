//! TSV como o Excel copia e cola: campos separados por tabulação, registros por quebra de linha; o campo que
//! tem tabulação, quebra de linha ou aspas vai entre aspas, com as aspas dobradas. O mesmo analisador do CSV
//! lê (aspas só valem no começo do campo, quebras dentro de aspas são texto).

use crate::csv::{Campo, decodificar_campo, proximo_registro};

const BOM: char = '\u{FEFF}';

/// Lê o texto da área de transferência como tabela: uma lista de linhas, cada uma com os seus campos (que
/// podem ser menos ou mais de uma linha para outra). A quebra de linha no fim não cria uma linha a mais (o
/// Excel termina o que copia com ela); uma linha em branco no meio é uma linha com um campo vazio (é o que o
/// Excel copia de uma célula vazia numa coluna só).
pub fn ler_tsv(texto: &str) -> Vec<Vec<String>> {
    let texto = texto.strip_prefix(BOM).unwrap_or(texto);
    let bytes = texto.as_bytes();
    let mut linhas = Vec::new();
    let mut campos: Vec<Campo> = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let Some(r) = proximo_registro(bytes, pos, b'\t', true, Some(&mut campos)) else {
            break;
        };
        // o texto veio de um &str: os campos são UTF-8 válido
        let cod = crate::csv::Codificacao::Utf8;
        linhas.push(campos.iter().map(|c| decodificar_campo(bytes, *c, cod)).collect());
        pos = r.fim;
    }
    linhas
}

fn precisa_de_aspas(campo: &str) -> bool {
    campo.bytes().any(|b| b == b'\t' || b == b'\n' || b == b'\r' || b == b'"')
}

fn escrever_campo(saida: &mut String, campo: &str) {
    if precisa_de_aspas(campo) {
        saida.push('"');
        for c in campo.chars() {
            if c == '"' {
                saida.push('"');
            }
            saida.push(c);
        }
        saida.push('"');
    } else {
        saida.push_str(campo);
    }
}

/// A tabela como o Excel a lê: tabulação entre os campos, `\r\n` depois de cada linha (inclusive a última).
pub fn escrever_tsv<L, C>(linhas: L) -> String
where
    L: IntoIterator,
    L::Item: IntoIterator<Item = C>,
    C: AsRef<str>,
{
    let mut saida = String::new();
    for linha in linhas {
        for (i, campo) in linha.into_iter().enumerate() {
            if i > 0 {
                saida.push('\t');
            }
            escrever_campo(&mut saida, campo.as_ref());
        }
        saida.push_str("\r\n");
    }
    saida
}

/// Uma coluna para a área de transferência: o `nome` (se tiver) na primeira linha e os valores embaixo. É o que
/// "Copiar coluna" leva.
pub fn escrever_coluna<S: AsRef<str>>(nome: Option<&str>, valores: &[S]) -> String {
    escrever_tsv(nome.into_iter().chain(valores.iter().map(|v| v.as_ref())).map(|c| [c]))
}

/// A largura da tabela: o maior número de campos de uma linha.
pub fn largura(linhas: &[Vec<String>]) -> usize {
    linhas.iter().map(Vec::len).max().unwrap_or(0)
}

/// A coluna `j` da tabela (vazia onde a linha é mais curta).
pub fn coluna_do_texto(linhas: &[Vec<String>], j: usize) -> Vec<String> {
    linhas.iter().map(|l| l.get(j).cloned().unwrap_or_default()).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn t(linhas: &[&[&str]]) -> Vec<Vec<String>> {
        linhas.iter().map(|l| l.iter().map(|c| c.to_string()).collect()).collect()
    }

    #[test]
    fn le_o_que_o_excel_copia() {
        assert_eq!(ler_tsv("a\tb\r\n1\t2\r\n"), t(&[&["a", "b"], &["1", "2"]]));
        assert_eq!(ler_tsv("a\tb\n1\t2\n"), t(&[&["a", "b"], &["1", "2"]]));
        assert_eq!(ler_tsv("a\tb\r\n1\t2"), t(&[&["a", "b"], &["1", "2"]]), "sem quebra no fim");
        assert_eq!(ler_tsv(""), Vec::<Vec<String>>::new());
        assert_eq!(ler_tsv("x"), t(&[&["x"]]));
    }

    #[test]
    fn quebra_de_linha_dentro_da_celula() {
        let excel = "nome\tobs\r\nAna\t\"linha 1\nlinha 2\"\r\nBia\tok\r\n";
        assert_eq!(ler_tsv(excel), t(&[&["nome", "obs"], &["Ana", "linha 1\nlinha 2"], &["Bia", "ok"]]));
        let crlf_dentro = "\"a\r\nb\"\tc\r\n";
        assert_eq!(ler_tsv(crlf_dentro), t(&[&["a\r\nb", "c"]]));
    }

    #[test]
    fn aspas_dentro_da_celula() {
        assert_eq!(ler_tsv("\"ele disse \"\"oi\"\"\"\tx\r\n"), t(&[&["ele disse \"oi\"", "x"]]));
        assert_eq!(ler_tsv("5\" cano\tx\r\n"), t(&[&["5\" cano", "x"]]), "aspa no meio é aspa comum");
        assert_eq!(ler_tsv("\"tab\there\"\tx\r\n"), t(&[&["tab\there", "x"]]));
    }

    #[test]
    fn celulas_vazias_e_linhas_em_branco() {
        assert_eq!(ler_tsv("a\t\tc\r\n"), t(&[&["a", "", "c"]]));
        assert_eq!(ler_tsv("a\t\r\n"), t(&[&["a", ""]]));
        assert_eq!(ler_tsv("a\r\n\r\nb\r\n"), t(&[&["a"], &[""], &["b"]]), "célula vazia no meio de uma coluna");
        assert_eq!(ler_tsv("a\r\n\r\n"), t(&[&["a"], &[""]]), "célula vazia no fim de uma coluna");
        assert_eq!(ler_tsv("\r\n"), t(&[&[""]]));
    }

    #[test]
    fn linhas_irregulares_e_bom() {
        assert_eq!(ler_tsv("a\tb\tc\r\nd\r\n"), t(&[&["a", "b", "c"], &["d"]]));
        assert_eq!(ler_tsv("\u{FEFF}a\tb\r\n"), t(&[&["a", "b"]]));
        assert_eq!(largura(&ler_tsv("a\tb\tc\r\nd\r\n")), 3);
        assert_eq!(largura(&[]), 0);
    }

    #[test]
    fn acentos_e_unicode() {
        assert_eq!(ler_tsv("ação\t日本\t😀\r\n"), t(&[&["ação", "日本", "😀"]]));
    }

    #[test]
    fn escreve_como_o_excel() {
        assert_eq!(escrever_tsv(t(&[&["a", "b"], &["1", "2"]])), "a\tb\r\n1\t2\r\n");
        assert_eq!(escrever_tsv(t(&[&["a\tb", "c"]])), "\"a\tb\"\tc\r\n");
        assert_eq!(escrever_tsv(t(&[&["l1\nl2"]])), "\"l1\nl2\"\r\n");
        assert_eq!(escrever_tsv(t(&[&["l1\r\nl2"]])), "\"l1\r\nl2\"\r\n");
        assert_eq!(escrever_tsv(t(&[&["diz \"oi\""]])), "\"diz \"\"oi\"\"\"\r\n");
        assert_eq!(
            escrever_tsv(t(&[&["com espaço ", " e ;"]])),
            "com espaço \t e ;\r\n",
            "espaços e ; não pedem aspas"
        );
        assert_eq!(escrever_tsv(t(&[&[""]])), "\r\n");
        assert_eq!(escrever_tsv(Vec::<Vec<String>>::new()), "");
        assert_eq!(escrever_tsv([["x", "y"]]), "x\ty\r\n", "aceita arrays de &str");
    }

    #[test]
    fn escreve_e_le_de_volta_qualquer_coisa() {
        let tabela = t(&[
            &["nome", "obs", "preço"],
            &["Ana", "linha 1\nlinha 2", "12,90"],
            &["", "", ""],
            &["com\ttab", "aspas \"x\" e \"\"y\"\"", "\"\""],
            &["\"começa com aspa", "termina com aspa\"", "  espaços  "],
            &["crlf\r\ndentro", "cr\rsó", "ação 日本 😀"],
        ]);
        assert_eq!(ler_tsv(&escrever_tsv(&tabela)), tabela);
    }

    #[test]
    fn coluna_para_a_area_de_transferencia() {
        assert_eq!(escrever_coluna(Some("preco"), &["12,90", "4,50"]), "preco\r\n12,90\r\n4,50\r\n");
        assert_eq!(escrever_coluna(None, &["a", "b"]), "a\r\nb\r\n");
        assert_eq!(escrever_coluna(Some("x"), &["l1\nl2"]), "x\r\n\"l1\nl2\"\r\n");
        assert_eq!(escrever_coluna::<&str>(None, &[]), "");
        let lido = ler_tsv(&escrever_coluna(Some("preco"), &["12,90", "", "4,50"]));
        assert_eq!(coluna_do_texto(&lido, 0), ["preco", "12,90", "", "4,50"]);
        assert_eq!(coluna_do_texto(&t(&[&["a", "b"], &["c"]]), 1), ["b", ""]);
    }
}
