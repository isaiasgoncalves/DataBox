//! Os formatos do "Copiar como" do menu da célula (0.2.3): CSV, JSON, Markdown, `INSERT`, a lista para `IN (…)` e o
//! `pd.DataFrame` do pandas, além do TSV com o cabeçalho do "Copiar com cabeçalho". Sem tela: recebem os nomes das
//! colunas e as linhas de [`ValorCopia`] (o NULL, o número e o texto já separados por quem sabe o tipo: o banco pela
//! célula, o CSV e o Excel pelo tipo detectado da coluna, [`ValorCopia::do_csv`]).
//!
//! O TSV e o CSV terminam as linhas em `\r\n` (o Excel e o RFC 4180); os de código, em `\n`.

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write as _;

use crate::numero::literal_numerico;

/// Quantas linhas vão em cada `INSERT` do [`escrever_insert`] (o SQLite aceita bem mais; 500 mantém o comando legível
/// e longe do limite de tamanho de comando dos outros bancos).
pub const LINHAS_POR_INSERT: usize = 500;

/// Uma célula para os formatos de cópia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValorCopia<'a> {
    /// O NULL do banco (no CSV, a célula vazia de uma coluna numérica): `null`, `NULL`, `None`; vazio no CSV e no
    /// Markdown.
    Nulo,
    /// Um número: o `texto` como a grade mostra (vai no CSV e no Markdown: `13.741,03` num CSV em português) e o
    /// `literal` que o código lê (vai no JSON, no SQL e no Python: `13741.03`, [`literal_numerico`]).
    Numero {
        texto: Cow<'a, str>,
        literal: Cow<'a, str>,
    },
    Texto(Cow<'a, str>),
}

impl<'a> ValorCopia<'a> {
    /// Um texto.
    pub fn texto(texto: impl Into<Cow<'a, str>>) -> Self {
        ValorCopia::Texto(texto.into())
    }

    /// Um número já escrito como literal (`17051`, `-0.5`): o texto é o próprio literal.
    pub fn numero(literal: impl Into<Cow<'a, str>>) -> Self {
        let literal = literal.into();
        ValorCopia::Numero { texto: literal.clone(), literal }
    }

    /// A célula de um CSV (ou de uma planilha): numa coluna `numerica` (o tipo detectado é inteiro ou decimal), o
    /// número lido com o `decimal` do arquivo e a célula vazia como [`ValorCopia::Nulo`]; o resto é texto (um valor
    /// que não se lê como número fica texto, com as aspas).
    pub fn do_csv(texto: &'a str, numerica: bool, decimal: char) -> Self {
        if numerica {
            if texto.trim().is_empty() {
                return ValorCopia::Nulo;
            }
            if let Some(literal) = literal_numerico(texto, decimal) {
                return ValorCopia::Numero { texto: Cow::Borrowed(texto), literal: Cow::Owned(literal) };
            }
        }
        ValorCopia::Texto(Cow::Borrowed(texto))
    }

    /// O texto como a grade mostra (o NULL é vazio): o que vai no TSV, no CSV e no Markdown.
    pub fn como_texto(&self) -> &str {
        match self {
            ValorCopia::Nulo => "",
            ValorCopia::Numero { texto, .. } => texto,
            ValorCopia::Texto(t) => t,
        }
    }
}

/// O formato do "Copiar como".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatoCopia {
    Csv,
    Json,
    Markdown,
    Insert,
    ListaIn,
    Pandas,
}

impl FormatoCopia {
    /// Na ordem do submenu.
    pub const TODOS: [FormatoCopia; 6] = [
        FormatoCopia::Csv,
        FormatoCopia::Json,
        FormatoCopia::Markdown,
        FormatoCopia::Insert,
        FormatoCopia::ListaIn,
        FormatoCopia::Pandas,
    ];

    /// O nome no submenu "Copiar como".
    pub fn rotulo(self) -> &'static str {
        match self {
            FormatoCopia::Csv => "CSV",
            FormatoCopia::Json => "JSON",
            FormatoCopia::Markdown => "Markdown",
            FormatoCopia::Insert => "INSERT",
            FormatoCopia::ListaIn => "Lista para IN (…)",
            FormatoCopia::Pandas => "pandas",
        }
    }
}

/// O que alguns formatos pedem além das células.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcoesCopia<'a> {
    /// O separador do CSV (o do arquivo, no CSV; a vírgula nas outras grades).
    pub separador: char,
    /// A tabela do `INSERT` (no CSV e no Excel, o nome do arquivo ou da planilha).
    pub tabela: &'a str,
}

impl Default for OpcoesCopia<'_> {
    fn default() -> Self {
        OpcoesCopia { separador: ',', tabela: "tabela" }
    }
}

/// As células no `formato`, com os `nomes` das colunas. A lista para `IN (…)` usa só a primeira coluna (o menu só a
/// oferece com uma).
pub fn escrever_como<S: AsRef<str>>(
    formato: FormatoCopia,
    nomes: &[S],
    linhas: &[Vec<ValorCopia<'_>>],
    opcoes: &OpcoesCopia<'_>,
) -> String {
    match formato {
        FormatoCopia::Csv => escrever_csv(nomes, linhas, opcoes.separador),
        FormatoCopia::Json => escrever_json(nomes, linhas),
        FormatoCopia::Markdown => escrever_markdown(nomes, linhas),
        FormatoCopia::Insert => escrever_insert(opcoes.tabela, nomes, linhas),
        FormatoCopia::ListaIn => escrever_lista_in(linhas.iter().filter_map(|l| l.first())),
        FormatoCopia::Pandas => escrever_pandas(nomes, linhas),
    }
}

/// O TSV do [`super::escrever_tsv`] com os `nomes` das colunas na primeira linha (o "Copiar com cabeçalho").
pub fn escrever_tsv_com_cabecalho<S, L, C>(nomes: &[S], linhas: L) -> String
where
    S: AsRef<str>,
    L: IntoIterator,
    L::Item: IntoIterator<Item = C>,
    C: AsRef<str>,
{
    let mut saida = super::escrever_tsv([nomes.iter().map(AsRef::as_ref)]);
    saida.push_str(&super::escrever_tsv(linhas));
    saida
}

// ------------------------------------------------------------------ CSV

/// CSV pelo RFC 4180: o `separador` entre os campos, `\r\n` depois de cada linha (inclusive a do cabeçalho e a
/// última), o campo que tem o separador, aspas ou quebra de linha entre aspas, com as aspas dobradas. O número vai como
/// a grade mostra (`13.741,03` com o separador `;` fica sem aspas; com `,`, entre aspas).
pub fn escrever_csv<S: AsRef<str>>(nomes: &[S], linhas: &[Vec<ValorCopia<'_>>], separador: char) -> String {
    let mut saida = String::new();
    linha_csv(&mut saida, nomes.iter().map(AsRef::as_ref), separador);
    for linha in linhas {
        linha_csv(&mut saida, linha.iter().map(ValorCopia::como_texto), separador);
    }
    saida
}

fn linha_csv<'c>(saida: &mut String, campos: impl Iterator<Item = &'c str>, separador: char) {
    for (i, campo) in campos.enumerate() {
        if i > 0 {
            saida.push(separador);
        }
        if campo.contains([separador, '"', '\n', '\r']) {
            saida.push('"');
            saida.push_str(&campo.replace('"', "\"\""));
            saida.push('"');
        } else {
            saida.push_str(campo);
        }
    }
    saida.push_str("\r\n");
}

// ------------------------------------------------------------------ JSON

/// Uma lista de objetos, um por linha: `[{"nome": "Bruno", "limite": 13741.03}, …]`, com os números como números, o
/// NULL como `null` e o texto com os escapes do JSON. Sem linhas, `[]`.
pub fn escrever_json<S: AsRef<str>>(nomes: &[S], linhas: &[Vec<ValorCopia<'_>>]) -> String {
    if linhas.is_empty() {
        return "[]\n".to_string();
    }
    let mut saida = String::from("[\n");
    for (i, linha) in linhas.iter().enumerate() {
        saida.push_str("  {");
        for (j, (nome, valor)) in nomes.iter().zip(linha).enumerate() {
            if j > 0 {
                saida.push_str(", ");
            }
            texto_json(&mut saida, nome.as_ref());
            saida.push_str(": ");
            match valor {
                ValorCopia::Nulo => saida.push_str("null"),
                ValorCopia::Numero { literal, .. } => saida.push_str(literal),
                ValorCopia::Texto(t) => texto_json(&mut saida, t),
            }
        }
        saida.push('}');
        if i + 1 < linhas.len() {
            saida.push(',');
        }
        saida.push('\n');
    }
    saida.push_str("]\n");
    saida
}

fn texto_json(saida: &mut String, texto: &str) {
    saida.push('"');
    for c in texto.chars() {
        match c {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(saida, "\\u{:04x}", c as u32);
            }
            c => saida.push(c),
        }
    }
    saida.push('"');
}

// ------------------------------------------------------------------ Markdown

/// Uma tabela do Markdown (a do GitHub): o cabeçalho, o fio (`---:` na coluna em que todo valor é número, alinhada à
/// direita) e uma linha por linha. A barra vira `\|`, a quebra de linha vira `<br>` e o NULL fica vazio.
pub fn escrever_markdown<S: AsRef<str>>(nomes: &[S], linhas: &[Vec<ValorCopia<'_>>]) -> String {
    let mut saida = String::new();
    linha_markdown(&mut saida, nomes.iter().map(AsRef::as_ref));
    saida.push('|');
    for j in 0..nomes.len() {
        let valores = linhas.iter().filter_map(|l| l.get(j)).filter(|v| !matches!(v, ValorCopia::Nulo));
        let mut numerica = false;
        let mut so_numeros = true;
        for v in valores {
            numerica = true;
            so_numeros &= matches!(v, ValorCopia::Numero { .. });
        }
        saida.push_str(if numerica && so_numeros { " ---: |" } else { " --- |" });
    }
    saida.push('\n');
    for linha in linhas {
        linha_markdown(&mut saida, linha.iter().map(ValorCopia::como_texto));
    }
    saida
}

fn linha_markdown<'c>(saida: &mut String, campos: impl Iterator<Item = &'c str>) {
    saida.push('|');
    for campo in campos {
        saida.push(' ');
        let mut anterior_cr = false;
        for c in campo.chars() {
            match c {
                '|' => saida.push_str("\\|"),
                '\r' => saida.push_str("<br>"),
                '\n' if anterior_cr => {}
                '\n' => saida.push_str("<br>"),
                c => saida.push(c),
            }
            anterior_cr = c == '\r';
        }
        saida.push_str(" |");
    }
    saida.push('\n');
}

// ------------------------------------------------------------------ SQL

/// O `INSERT` das linhas na `tabela`: `INSERT INTO "clientes" ("nome", "email") VALUES` com uma linha por linha
/// embaixo, um comando para até [`LINHAS_POR_INSERT`] linhas. Os nomes vão citados com o [`comum::q`]; o texto entre
/// apóstrofos, com os apóstrofos dobrados; o NULL como `NULL`. Sem linhas, vazio.
pub fn escrever_insert<S: AsRef<str>>(tabela: &str, nomes: &[S], linhas: &[Vec<ValorCopia<'_>>]) -> String {
    let mut saida = String::new();
    let colunas: Vec<String> = nomes.iter().map(|n| comum::q(n.as_ref())).collect();
    let cabeca = format!("INSERT INTO {} ({}) VALUES\n", comum::q(tabela), colunas.join(", "));
    for bloco in linhas.chunks(LINHAS_POR_INSERT) {
        saida.push_str(&cabeca);
        for (i, linha) in bloco.iter().enumerate() {
            saida.push_str("  (");
            for (j, valor) in linha.iter().enumerate() {
                if j > 0 {
                    saida.push_str(", ");
                }
                valor_sql(&mut saida, valor);
            }
            saida.push_str(if i + 1 < bloco.len() { "),\n" } else { ");\n" });
        }
    }
    saida
}

fn valor_sql(saida: &mut String, valor: &ValorCopia<'_>) {
    match valor {
        ValorCopia::Nulo => saida.push_str("NULL"),
        ValorCopia::Numero { literal, .. } => saida.push_str(literal),
        ValorCopia::Texto(t) => {
            saida.push('\'');
            saida.push_str(&t.replace('\'', "''"));
            saida.push('\'');
        }
    }
}

/// A lista para colar depois de um `IN`: `('Recife', 'Curitiba')`, os valores de uma coluna sem os repetidos (fica o
/// primeiro, na ordem da grade), os números sem aspas e sem o NULL (que o `IN` nunca acha). Sem valores, `()`.
pub fn escrever_lista_in<'v, 'a: 'v>(valores: impl IntoIterator<Item = &'v ValorCopia<'a>>) -> String {
    let mut vistos: HashSet<(bool, &str)> = HashSet::new();
    let mut saida = String::from("(");
    for valor in valores {
        let chave = match valor {
            ValorCopia::Nulo => continue,
            ValorCopia::Numero { literal, .. } => (true, literal.as_ref()),
            ValorCopia::Texto(t) => (false, t.as_ref()),
        };
        if !vistos.insert(chave) {
            continue;
        }
        if vistos.len() > 1 {
            saida.push_str(", ");
        }
        valor_sql(&mut saida, valor);
    }
    saida.push(')');
    saida
}

// ------------------------------------------------------------------ pandas

/// O `pd.DataFrame({...})` para colar num notebook: uma lista por coluna, com o nome dela; os números como números, o
/// NULL como `None` e o texto entre aspas duplas com os escapes do Python.
pub fn escrever_pandas<S: AsRef<str>>(nomes: &[S], linhas: &[Vec<ValorCopia<'_>>]) -> String {
    let mut saida = String::from("pd.DataFrame({\n");
    for (j, nome) in nomes.iter().enumerate() {
        saida.push_str("    ");
        texto_python(&mut saida, nome.as_ref());
        saida.push_str(": [");
        for (i, linha) in linhas.iter().enumerate() {
            if i > 0 {
                saida.push_str(", ");
            }
            match linha.get(j).unwrap_or(&ValorCopia::Nulo) {
                ValorCopia::Nulo => saida.push_str("None"),
                ValorCopia::Numero { literal, .. } => saida.push_str(literal),
                ValorCopia::Texto(t) => texto_python(&mut saida, t),
            }
        }
        saida.push_str("],\n");
    }
    saida.push_str("})\n");
    saida
}

fn texto_python(saida: &mut String, texto: &str) {
    saida.push('"');
    for c in texto.chars() {
        match c {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(saida, "\\x{:02x}", c as u32);
            }
            c => saida.push(c),
        }
    }
    saida.push('"');
}

#[cfg(test)]
mod testes {
    use super::*;

    fn t(s: &str) -> ValorCopia<'_> {
        ValorCopia::texto(s)
    }

    fn n(s: &str) -> ValorCopia<'_> {
        ValorCopia::numero(s)
    }

    /// Duas linhas de clientes, com aspas, quebra de linha e NULL.
    fn clientes() -> (Vec<&'static str>, Vec<Vec<ValorCopia<'static>>>) {
        (
            vec!["nome", "limite", "obs"],
            vec![
                vec![t("Bruno"), n("13741.03"), t("diz \"oi\"")],
                vec![t("Ana | Bia"), ValorCopia::Nulo, t("linha 1\nlinha 2")],
            ],
        )
    }

    #[test]
    fn valores_do_csv_pelo_tipo_da_coluna() {
        assert_eq!(
            ValorCopia::do_csv("13.741,03", true, ','),
            ValorCopia::Numero { texto: "13.741,03".into(), literal: "13741.03".into() }
        );
        assert_eq!(ValorCopia::do_csv("", true, ','), ValorCopia::Nulo, "vazio numa coluna numérica");
        assert_eq!(ValorCopia::do_csv("  ", true, ','), ValorCopia::Nulo);
        assert_eq!(ValorCopia::do_csv("n/d", true, ','), t("n/d"), "o que não é número fica texto");
        assert_eq!(ValorCopia::do_csv("13,5", false, ','), t("13,5"), "coluna de texto: tudo texto");
        assert_eq!(ValorCopia::do_csv("", false, ','), t(""));
        assert_eq!(ValorCopia::do_csv("1,234.5", true, '.').como_texto(), "1,234.5");
        assert_eq!(n("7").como_texto(), "7");
        assert_eq!(ValorCopia::Nulo.como_texto(), "");
    }

    #[test]
    fn tsv_com_cabecalho() {
        assert_eq!(
            escrever_tsv_com_cabecalho(&["nome", "obs"], [["Ana", "l1\nl2"]]),
            "nome\tobs\r\nAna\t\"l1\nl2\"\r\n"
        );
        assert_eq!(escrever_tsv_com_cabecalho(&["a"], Vec::<Vec<String>>::new()), "a\r\n");
    }

    #[test]
    fn csv_pelo_rfc_4180() {
        let (nomes, linhas) = clientes();
        assert_eq!(
            escrever_csv(&nomes, &linhas, ','),
            "nome,limite,obs\r\nBruno,13741.03,\"diz \"\"oi\"\"\"\r\nAna | Bia,,\"linha 1\nlinha 2\"\r\n"
        );
        // o número como a grade mostra: com a vírgula decimal, aspas só se o separador é a vírgula
        let pt = vec![vec![ValorCopia::do_csv("13.741,03", true, ','), t("a;b")]];
        assert_eq!(escrever_csv(&["limite", "x"], &pt, ';'), "limite;x\r\n13.741,03;\"a;b\"\r\n");
        assert_eq!(escrever_csv(&["limite", "x"], &pt, ','), "limite,x\r\n\"13.741,03\",a;b\r\n");
        assert_eq!(escrever_csv(&["tab"], &[vec![t("a\tb")]], '\t'), "tab\r\n\"a\tb\"\r\n");
        assert_eq!(escrever_csv(&["cr"], &[vec![t("a\rb")]], ','), "cr\r\n\"a\rb\"\r\n");
    }

    #[test]
    fn json_com_numeros_e_null() {
        let (nomes, linhas) = clientes();
        assert_eq!(
            escrever_json(&nomes, &linhas),
            "[\n  {\"nome\": \"Bruno\", \"limite\": 13741.03, \"obs\": \"diz \\\"oi\\\"\"},\n  \
             {\"nome\": \"Ana | Bia\", \"limite\": null, \"obs\": \"linha 1\\nlinha 2\"}\n]\n"
        );
        let pt = vec![vec![ValorCopia::do_csv("-1.234,50", true, ',')]];
        assert_eq!(escrever_json(&["v"], &pt), "[\n  {\"v\": -1234.50}\n]\n", "decimal com vírgula do CSV");
        assert_eq!(escrever_json(&["a\\b"], &[vec![t("\u{1}\r\t")]]), "[\n  {\"a\\\\b\": \"\\u0001\\r\\t\"}\n]\n");
        assert_eq!(escrever_json(&["v"], &[]), "[]\n");
    }

    #[test]
    fn markdown_com_barras_escapadas() {
        let (nomes, linhas) = clientes();
        assert_eq!(
            escrever_markdown(&nomes, &linhas),
            "| nome | limite | obs |\n| --- | ---: | --- |\n| Bruno | 13741.03 | diz \"oi\" |\n\
             | Ana \\| Bia |  | linha 1<br>linha 2 |\n"
        );
        assert_eq!(escrever_markdown(&["x"], &[vec![t("a\r\nb")]]), "| x |\n| --- |\n| a<br>b |\n");
        let pt = vec![vec![ValorCopia::do_csv("13.741,03", true, ',')]];
        assert_eq!(escrever_markdown(&["v"], &pt), "| v |\n| ---: |\n| 13.741,03 |\n");
        assert_eq!(escrever_markdown(&["v"], &[vec![ValorCopia::Nulo]]), "| v |\n| --- |\n|  |\n", "só NULL: texto");
    }

    #[test]
    fn insert_com_nomes_citados() {
        let (nomes, linhas) = clientes();
        assert_eq!(
            escrever_insert("clientes", &nomes, &linhas),
            "INSERT INTO \"clientes\" (\"nome\", \"limite\", \"obs\") VALUES\n  ('Bruno', 13741.03, 'diz \"oi\"'),\n  \
             ('Ana | Bia', NULL, 'linha 1\nlinha 2');\n"
        );
        assert_eq!(
            escrever_insert("a\"b", &["it's"], &[vec![t("d'Ávila")]]),
            "INSERT INTO \"a\"\"b\" (\"it's\") VALUES\n  ('d''Ávila');\n"
        );
        assert_eq!(escrever_insert("t", &["x"], &[]), "");
    }

    #[test]
    fn insert_de_500_em_500() {
        let linhas: Vec<Vec<ValorCopia<'_>>> = (0..1001).map(|i| vec![ValorCopia::numero(i.to_string())]).collect();
        let sql = escrever_insert("t", &["n"], &linhas);
        assert_eq!(sql.matches("INSERT INTO").count(), 3);
        assert_eq!(sql.matches(";\n").count(), 3);
        assert!(sql.contains("  (499);\nINSERT INTO \"t\" (\"n\") VALUES\n  (500),\n"));
        assert!(sql.ends_with("VALUES\n  (1000);\n"));
    }

    #[test]
    fn lista_in_sem_repetidos() {
        let valores = [t("Recife"), t("Curitiba"), t("Recife"), ValorCopia::Nulo, t("d'Ávila")];
        assert_eq!(escrever_lista_in(&valores), "('Recife', 'Curitiba', 'd''Ávila')");
        let numeros = [n("1"), n("2.5"), n("1"), t("1")];
        assert_eq!(escrever_lista_in(&numeros), "(1, 2.5, '1')", "o número 1 e o texto '1' são diferentes");
        let pt: Vec<ValorCopia<'_>> =
            ["1.000,5", "", "1.000,5"].iter().map(|s| ValorCopia::do_csv(s, true, ',')).collect();
        assert_eq!(escrever_lista_in(&pt), "(1000.5)");
        assert_eq!(escrever_lista_in(&[ValorCopia::Nulo]), "()");
        let (nomes, linhas) = clientes();
        assert_eq!(
            escrever_como(FormatoCopia::ListaIn, &nomes, &linhas, &OpcoesCopia::default()),
            "('Bruno', 'Ana | Bia')",
            "só a primeira coluna"
        );
    }

    #[test]
    fn pandas_com_none() {
        let (nomes, linhas) = clientes();
        assert_eq!(
            escrever_pandas(&nomes, &linhas),
            "pd.DataFrame({\n    \"nome\": [\"Bruno\", \"Ana | Bia\"],\n    \"limite\": [13741.03, None],\n    \
             \"obs\": [\"diz \\\"oi\\\"\", \"linha 1\\nlinha 2\"],\n})\n"
        );
        assert_eq!(
            escrever_pandas(&["a"], &[vec![t("c:\\x\u{0}")]]),
            "pd.DataFrame({\n    \"a\": [\"c:\\\\x\\x00\"],\n})\n"
        );
        assert_eq!(escrever_pandas(&["a"], &[]), "pd.DataFrame({\n    \"a\": [],\n})\n");
    }

    #[test]
    fn copiar_como_escolhe_o_formato() {
        let (nomes, linhas) = clientes();
        let opcoes = OpcoesCopia { separador: ';', tabela: "clientes" };
        assert_eq!(escrever_como(FormatoCopia::Csv, &nomes, &linhas, &opcoes), escrever_csv(&nomes, &linhas, ';'));
        assert_eq!(
            escrever_como(FormatoCopia::Insert, &nomes, &linhas, &opcoes),
            escrever_insert("clientes", &nomes, &linhas)
        );
        assert_eq!(
            FormatoCopia::TODOS.map(FormatoCopia::rotulo).join(" · "),
            "CSV · JSON · Markdown · INSERT · Lista para IN (…) · pandas"
        );
    }
}
