//! O código: o realce de SQL e a aparência do código (as "cores do código", a família e o tamanho da fonte).
//!
//! - [`PALAVRAS_RESERVADAS`] (as 147 do SQLite, em ordem para a busca binária) e [`palavra_reservada`].
//! - [`trechos_sql`]: um léxico de SQL que divide o texto em trechos contíguos ([`ClasseSql`]): palavras-chave,
//!   textos (`'…'`, `X'…'`), identificadores citados (`"…"`, `` `…` ``, `[…]`), números (decimais, com expoente,
//!   hexadecimais), comentários (`--` até o fim da linha e `/* … */`), pontuação e operadores (`, ; ( ) . = < > ! + -
//!   * / % | & ~`) e o resto (nomes, parâmetros, espaços). Nunca perde um byte: os trechos juntos dão o texto.
//! - [`CoresCodigo`] (o espelho do `prefs::CoresCodigo`: DataBox, Alto contraste, Solarized escuro, Monokai) e
//!   [`CoresResolvidas`] `{ fundo, texto, kw, st, nu, pu }`, com os valores da prancheta 13 (`Preferencias.dc.html`);
//!   o DataBox segue o tema (`--c`, `--t`, `--kw`, `--st`, `--nu` e `--t3`).
//! - [`realcar_sql`]`(texto, cores, fonte) -> LayoutJob`: palavra-chave em `kw`, texto e identificador citado em
//!   `st`, número em `nu`, comentário e pontuação em `pu`, o resto em `texto`.
//! - [`AparenciaCodigo`] `{ cores, familia, tamanho }`, guardada no contexto: [`definir`] (também o tamanho dos
//!   estilos `Monospace` e `mono`), [`atual`], [`cores`] (as resolvidas no tema em vigor), [`fonte`] e
//!   [`altura_linha`] (a linha de 1,6, 20px em 12,5px). A família trocada mora em [`crate::fontes::trocar_mono`]:
//!   ler o arquivo da fonte é com quem chama (numa `Promessa`, fora da thread da interface).
//!
//! Quem usa: [`crate::pecas::campos::AreaTexto::codigo`] (o editor da Consulta SQL) e
//! [`crate::pecas::texto_pre::TextoPre::sql`] (o SQL das janelas, dos gatilhos e da definição).

use std::ops::Range;
use std::sync::Arc;

use egui::cache::{ComputerMut, FrameCache};
use egui::text::{ByteIndex, LayoutJob, LayoutSection, TextFormat};
use egui::{Color32, Context, FontId, TextStyle, Ui};

use crate::tema::{self, Paleta, rgb};

/// As palavras reservadas do SQLite, em ordem alfabética para a
/// busca binária.
pub const PALAVRAS_RESERVADAS: [&str; 147] = [
    "ABORT",
    "ACTION",
    "ADD",
    "AFTER",
    "ALL",
    "ALTER",
    "ALWAYS",
    "ANALYZE",
    "AND",
    "AS",
    "ASC",
    "ATTACH",
    "AUTOINCREMENT",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CASCADE",
    "CASE",
    "CAST",
    "CHECK",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "CONFLICT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATABASE",
    "DEFAULT",
    "DEFERRABLE",
    "DEFERRED",
    "DELETE",
    "DESC",
    "DETACH",
    "DISTINCT",
    "DO",
    "DROP",
    "EACH",
    "ELSE",
    "END",
    "ESCAPE",
    "EXCEPT",
    "EXCLUDE",
    "EXCLUSIVE",
    "EXISTS",
    "EXPLAIN",
    "FAIL",
    "FILTER",
    "FIRST",
    "FOLLOWING",
    "FOR",
    "FOREIGN",
    "FROM",
    "FULL",
    "GENERATED",
    "GLOB",
    "GROUP",
    "GROUPS",
    "HAVING",
    "IF",
    "IGNORE",
    "IMMEDIATE",
    "IN",
    "INDEX",
    "INDEXED",
    "INITIALLY",
    "INNER",
    "INSERT",
    "INSTEAD",
    "INTERSECT",
    "INTO",
    "IS",
    "ISNULL",
    "JOIN",
    "KEY",
    "LAST",
    "LEFT",
    "LIKE",
    "LIMIT",
    "MATCH",
    "MATERIALIZED",
    "NATURAL",
    "NO",
    "NOT",
    "NOTHING",
    "NOTNULL",
    "NULL",
    "NULLS",
    "OF",
    "OFFSET",
    "ON",
    "OR",
    "ORDER",
    "OTHERS",
    "OUTER",
    "OVER",
    "PARTITION",
    "PLAN",
    "PRAGMA",
    "PRECEDING",
    "PRIMARY",
    "QUERY",
    "RAISE",
    "RANGE",
    "RECURSIVE",
    "REFERENCES",
    "REGEXP",
    "REINDEX",
    "RELEASE",
    "RENAME",
    "REPLACE",
    "RESTRICT",
    "RETURNING",
    "RIGHT",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SAVEPOINT",
    "SELECT",
    "SET",
    "TABLE",
    "TEMP",
    "TEMPORARY",
    "THEN",
    "TIES",
    "TO",
    "TRANSACTION",
    "TRIGGER",
    "UNBOUNDED",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USING",
    "VACUUM",
    "VALUES",
    "VIEW",
    "VIRTUAL",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
    "WITHOUT",
];

/// A maior palavra reservada ("CURRENT_TIMESTAMP").
const MAIOR_PALAVRA: usize = 17;

/// A palavra é reservada no SQLite (sem diferença de caixa)? Sem alocar: compara em maiúsculas numa pilha.
pub fn palavra_reservada(nome: &str) -> bool {
    let bytes = nome.as_bytes();
    if bytes.is_empty() || bytes.len() > MAIOR_PALAVRA || !nome.is_ascii() {
        return false;
    }
    let mut maiusculas = [0u8; MAIOR_PALAVRA];
    for (destino, b) in maiusculas.iter_mut().zip(bytes) {
        *destino = b.to_ascii_uppercase();
    }
    let alvo = &maiusculas[..bytes.len()];
    PALAVRAS_RESERVADAS.binary_search_by(|p| p.as_bytes().cmp(alvo)).is_ok()
}

/// O nome pronto para o SQL: como está quando é um identificador simples
/// (letra ASCII ou `_`, depois letras, algarismos e `_`) que não é palavra reservada; senão entre aspas duplas, com
/// as aspas de dentro dobradas. É a única regra do app: `order` vira `"order"`.
pub fn citar(nome: &str) -> String {
    let mut chars = nome.chars();
    let simples = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if simples && !palavra_reservada(nome) { nome.to_string() } else { format!("\"{}\"", nome.replace('"', "\"\"")) }
}

// ---- léxico ----------------------------------------------------------------------------------------

/// O que cada trecho do SQL é.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClasseSql {
    /// Nomes, parâmetros (`:x`, `?1`, `@x`, `$x`), espaços: a cor do texto.
    Comum,
    /// Uma das [`PALAVRAS_RESERVADAS`].
    PalavraChave,
    /// `'texto'` e o BLOB `X'0A1B'`.
    Texto,
    /// O identificador citado: `"nome"`, `` `nome` `` ou `[nome]` (nunca é palavra-chave).
    Citado,
    Numero,
    /// `-- até o fim da linha` e `/* … */`.
    Comentario,
    /// `, ; ( ) .` e os operadores.
    Pontuacao,
}

/// Um trecho contíguo do texto, em bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrechoSql {
    pub bytes: Range<usize>,
    pub classe: ClasseSql,
}

fn comeca_nome(c: char) -> bool {
    c.is_alphabetic() || c == '_' || !c.is_ascii()
}

fn continua_nome(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || !c.is_ascii()
}

fn pontuacao(c: char) -> bool {
    matches!(c, ',' | ';' | '(' | ')' | '.' | '=' | '<' | '>' | '!' | '+' | '-' | '*' | '/' | '%' | '|' | '&' | '~')
}

/// O fim (em bytes) do trecho entre `fecho`s que começa em `inicio` (na abertura), com o fecho dobrado como
/// escape (`''`, `""`); sem o fecho, vai até o fim do texto.
fn fim_citado(texto: &str, inicio: usize, abertura_len: usize, fecho: char, dobra: bool) -> usize {
    let mut i = inicio + abertura_len;
    let resto = texto.as_bytes();
    let f = fecho as u8;
    while i < resto.len() {
        if resto[i] == f {
            if dobra && resto.get(i + 1) == Some(&f) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    resto.len()
}

/// O fim (em bytes) do número que começa em `inicio` (um algarismo, ou um ponto seguido de algarismo).
fn fim_numero(b: &[u8], inicio: usize) -> usize {
    let mut i = inicio;
    if b[i] == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X')) && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit) {
        i += 2;
        while b.get(i).is_some_and(|c| c.is_ascii_hexdigit() || *c == b'_') {
            i += 1;
        }
        return i;
    }
    while b.get(i).is_some_and(|c| c.is_ascii_digit() || *c == b'_') {
        i += 1;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(|c| c.is_ascii_digit() || *c == b'_') {
            i += 1;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if b.get(j).is_some_and(u8::is_ascii_digit) {
            i = j;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
    }
    i
}

/// Divide o SQL em trechos contíguos (os vizinhos da mesma classe se juntam). Os trechos juntos dão o texto inteiro;
/// um texto vazio não tem trecho.
pub fn trechos_sql(texto: &str) -> Vec<TrechoSql> {
    let b = texto.as_bytes();
    let mut trechos: Vec<TrechoSql> = Vec::new();
    let juntar = |trechos: &mut Vec<TrechoSql>, bytes: Range<usize>, classe: ClasseSql| {
        if bytes.is_empty() {
            return;
        }
        match trechos.last_mut() {
            Some(ultimo) if ultimo.classe == classe && ultimo.bytes.end == bytes.start => ultimo.bytes.end = bytes.end,
            _ => trechos.push(TrechoSql { bytes, classe }),
        }
    };
    let mut i = 0;
    while i < b.len() {
        let c = texto[i..].chars().next().unwrap_or('\0');
        let proximo = b.get(i + 1).copied();
        let (fim, classe) = match c {
            '-' if proximo == Some(b'-') => {
                let fim = texto[i..].find('\n').map_or(b.len(), |n| i + n);
                (fim, ClasseSql::Comentario)
            }
            '/' if proximo == Some(b'*') => {
                let fim = texto[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2);
                (fim, ClasseSql::Comentario)
            }
            '\'' => (fim_citado(texto, i, 1, '\'', true), ClasseSql::Texto),
            'x' | 'X' if proximo == Some(b'\'') => (fim_citado(texto, i, 2, '\'', true), ClasseSql::Texto),
            '"' => (fim_citado(texto, i, 1, '"', true), ClasseSql::Citado),
            '`' => (fim_citado(texto, i, 1, '`', true), ClasseSql::Citado),
            '[' => (fim_citado(texto, i, 1, ']', false), ClasseSql::Citado),
            '0'..='9' => (fim_numero(b, i), ClasseSql::Numero),
            '.' if proximo.is_some_and(|d| d.is_ascii_digit()) => (fim_numero(b, i), ClasseSql::Numero),
            c if comeca_nome(c) => {
                let fim = texto[i..].char_indices().find(|&(_, c)| !continua_nome(c)).map_or(b.len(), |(n, _)| i + n);
                let classe = if palavra_reservada(&texto[i..fim]) { ClasseSql::PalavraChave } else { ClasseSql::Comum };
                (fim, classe)
            }
            // parâmetros: o nome vai junto, sem virar palavra-chave (`:limit`, `@from`)
            ':' | '@' | '$' | '?' => {
                let fim =
                    texto[i + 1..].char_indices().find(|&(_, c)| !continua_nome(c)).map_or(b.len(), |(n, _)| i + 1 + n);
                (fim, ClasseSql::Comum)
            }
            c if pontuacao(c) => (i + 1, ClasseSql::Pontuacao),
            c => (i + c.len_utf8(), ClasseSql::Comum),
        };
        // nunca para no meio de um caractere nem fica parado
        let fim = fim.max(i + c.len_utf8()).min(b.len());
        juntar(&mut trechos, i..fim, classe);
        i = fim;
    }
    trechos
}

// ---- léxico de JSON --------------------------------------------------------------------------------

/// O que cada trecho do JSON é.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClasseJson {
    /// Espaços, quebras e tudo que não é JSON (o texto solto de um documento inválido): a cor do texto.
    Comum,
    /// Uma cadeia seguida de `:` (o nome do campo): a cor `chave`.
    Chave,
    /// Uma cadeia que é valor (ou que não fecha): a cor `st`.
    Texto,
    Numero,
    /// `true`, `false` e `null`: a cor `kw`.
    Literal,
    /// `{ } [ ] , :`: a cor `pu`.
    Pontuacao,
}

/// Um trecho contíguo do JSON, em bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrechoJson {
    pub bytes: Range<usize>,
    pub classe: ClasseJson,
}

/// O fim (em bytes) da cadeia que abre em `inicio` (na aspa) e se ela fechou: vai até a aspa de fechamento, com `\`
/// escapando o caractere seguinte, e **nunca atravessa a linha** (uma cadeia sem fecho para antes da quebra). É o que
/// deixa o léxico linha a linha igual ao do texto todo.
fn fim_cadeia(b: &[u8], inicio: usize) -> (usize, bool) {
    let mut i = inicio + 1;
    while i < b.len() {
        match b[i] {
            b'"' => return (i + 1, true),
            b'\n' => return (i, false),
            b'\\' if b.get(i + 1).is_some_and(|c| *c != b'\n') => i += 2,
            _ => i += 1,
        }
    }
    (b.len(), false)
}

/// O fim (em bytes) do número JSON que começa em `inicio` (um `-` seguido de algarismo, ou um algarismo): parte
/// inteira, fração e expoente, sem exigir que o número esteja certo (`01` e `1.` também são um número só aqui).
fn fim_numero_json(b: &[u8], inicio: usize) -> usize {
    let mut i = inicio;
    if b[i] == b'-' {
        i += 1;
    }
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if b.get(j).is_some_and(u8::is_ascii_digit) {
            i = j;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
    }
    i
}

/// Depois de `fim`, só espaços e tabulações até um `:` na **mesma linha**? (É o que faz da cadeia uma chave.)
fn segue_dois_pontos(b: &[u8], fim: usize) -> bool {
    let mut i = fim;
    while matches!(b.get(i), Some(b' ' | b'\t' | b'\r')) {
        i += 1;
    }
    b.get(i) == Some(&b':')
}

/// Divide o JSON em trechos contíguos (os vizinhos da mesma classe se juntam). Os trechos juntos dão o texto inteiro;
/// um texto vazio não tem trecho. Vale também para o JSON inválido: uma cadeia seguida de `:` é chave, o que não é
/// JSON fica `Comum`. Uma cadeia nunca atravessa a linha, então lexar linha a linha dá o mesmo que lexar tudo
/// (o leitor de arquivo grande depende disso); só o `:` precisa estar na mesma linha da cadeia.
pub fn trechos_json(texto: &str) -> Vec<TrechoJson> {
    let b = texto.as_bytes();
    let mut trechos: Vec<TrechoJson> = Vec::new();
    let mut juntar = |bytes: Range<usize>, classe: ClasseJson| {
        if bytes.is_empty() {
            return;
        }
        match trechos.last_mut() {
            Some(ultimo) if ultimo.classe == classe && ultimo.bytes.end == bytes.start => ultimo.bytes.end = bytes.end,
            _ => trechos.push(TrechoJson { bytes, classe }),
        }
    };
    let mut i = 0;
    while i < b.len() {
        let c = texto[i..].chars().next().unwrap_or('\0');
        let (fim, classe) = match c {
            '"' => {
                let (fim, fechou) = fim_cadeia(b, i);
                (fim, if fechou && segue_dois_pontos(b, fim) { ClasseJson::Chave } else { ClasseJson::Texto })
            }
            '{' | '}' | '[' | ']' | ',' | ':' => (i + 1, ClasseJson::Pontuacao),
            '0'..='9' => (fim_numero_json(b, i), ClasseJson::Numero),
            '-' if b.get(i + 1).is_some_and(u8::is_ascii_digit) => (fim_numero_json(b, i), ClasseJson::Numero),
            c if c.is_ascii_alphabetic() => {
                let fim = texto[i..]
                    .char_indices()
                    .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_'))
                    .map_or(b.len(), |(n, _)| i + n);
                let classe = if matches!(&texto[i..fim], "true" | "false" | "null") {
                    ClasseJson::Literal
                } else {
                    ClasseJson::Comum
                };
                (fim, classe)
            }
            c => (i + c.len_utf8(), ClasseJson::Comum),
        };
        let fim = fim.max(i + c.len_utf8()).min(b.len());
        juntar(i..fim, classe);
        i = fim;
    }
    trechos
}

// ---- cores -----------------------------------------------------------------------------------------

/// A linguagem de um texto de código: decide o léxico do realce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Linguagem {
    /// O SQL: [`trechos_sql`].
    #[default]
    Sql,
    /// O JSON: [`trechos_json`].
    Json,
    /// Texto simples: sem realce (uma cor só, a do texto).
    Texto,
}

/// As "cores do código" (o espelho do `prefs::CoresCodigo`, que o interface não conhece).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CoresCodigo {
    /// As do tema do DataBox, claro ou escuro.
    #[default]
    DataBox,
    AltoContraste,
    SolarizedEscuro,
    Monokai,
}

impl CoresCodigo {
    pub const TODAS: [CoresCodigo; 4] =
        [CoresCodigo::DataBox, CoresCodigo::AltoContraste, CoresCodigo::SolarizedEscuro, CoresCodigo::Monokai];

    /// O texto do arquivo de preferências: "databox", "alto_contraste", "solarized_escuro" ou "monokai".
    pub fn como_str(self) -> &'static str {
        match self {
            CoresCodigo::DataBox => "databox",
            CoresCodigo::AltoContraste => "alto_contraste",
            CoresCodigo::SolarizedEscuro => "solarized_escuro",
            CoresCodigo::Monokai => "monokai",
        }
    }

    /// O de [`CoresCodigo::como_str`]; outro texto é None.
    pub fn de_str(texto: &str) -> Option<CoresCodigo> {
        CoresCodigo::TODAS.into_iter().find(|c| c.como_str() == texto)
    }

    /// O nome da carta nas Preferências.
    pub fn rotulo(self) -> &'static str {
        match self {
            CoresCodigo::DataBox => "DataBox",
            CoresCodigo::AltoContraste => "Alto contraste",
            CoresCodigo::SolarizedEscuro => "Solarized",
            CoresCodigo::Monokai => "Monokai",
        }
    }

    /// O complemento da carta (o `small`): "segue o tema", "escuro" ou nada.
    pub fn nota(self) -> &'static str {
        match self {
            CoresCodigo::DataBox => "segue o tema",
            CoresCodigo::SolarizedEscuro => "escuro",
            CoresCodigo::AltoContraste | CoresCodigo::Monokai => "",
        }
    }

    /// As cores segue o tema (só a DataBox)?
    pub fn segue_o_tema(self) -> bool {
        self == CoresCodigo::DataBox
    }

    /// As seis cores na paleta do tema (só a DataBox depende dela).
    pub fn resolver(self, p: &Paleta) -> CoresResolvidas {
        let e = &p.estudio;
        match self {
            CoresCodigo::DataBox => {
                CoresResolvidas { fundo: e.doc, texto: e.t, kw: e.kw, st: e.st, nu: e.nu, pu: e.t3, chave: e.chave }
            }
            CoresCodigo::AltoContraste => CoresResolvidas {
                fundo: rgb(0x00, 0x00, 0x00),
                texto: rgb(0xFF, 0xFF, 0xFF),
                kw: rgb(0x66, 0xC2, 0xFF),
                st: rgb(0x8C, 0xFF, 0x8C),
                nu: rgb(0xFF, 0xD8, 0x66),
                pu: rgb(0xC8, 0xC8, 0xC8),
                chave: rgb(0xE0, 0xB0, 0xFF),
            },
            CoresCodigo::SolarizedEscuro => CoresResolvidas {
                fundo: rgb(0x00, 0x2B, 0x36),
                texto: rgb(0x93, 0xA1, 0xA1),
                kw: rgb(0x85, 0x99, 0x00),
                st: rgb(0x2A, 0xA1, 0x98),
                nu: rgb(0xD3, 0x36, 0x82),
                pu: rgb(0x65, 0x7B, 0x83),
                chave: rgb(0x84, 0x89, 0xD6),
            },
            CoresCodigo::Monokai => CoresResolvidas {
                fundo: rgb(0x27, 0x28, 0x22),
                texto: rgb(0xF8, 0xF8, 0xF2),
                kw: rgb(0xF9, 0x26, 0x72),
                st: rgb(0xE6, 0xDB, 0x74),
                nu: rgb(0xAE, 0x81, 0xFF),
                pu: rgb(0xA5, 0x9F, 0x85),
                chave: rgb(0xA6, 0xE2, 0x2E),
            },
        }
    }
}

/// As cores de um esquema já resolvidas: o fundo da área, o texto comum, `kw` (palavra-chave; `true`, `false` e `null`
/// do JSON), `st` (texto e identificador citado), `nu` (número), `pu` (pontuação e comentário) e `chave` (o nome dos
/// campos do JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoresResolvidas {
    pub fundo: Color32,
    pub texto: Color32,
    pub kw: Color32,
    pub st: Color32,
    pub nu: Color32,
    pub pu: Color32,
    pub chave: Color32,
}

impl CoresResolvidas {
    /// A cor de uma classe do léxico de SQL.
    pub fn da_classe(&self, classe: ClasseSql) -> Color32 {
        match classe {
            ClasseSql::Comum => self.texto,
            ClasseSql::PalavraChave => self.kw,
            ClasseSql::Texto | ClasseSql::Citado => self.st,
            ClasseSql::Numero => self.nu,
            ClasseSql::Comentario | ClasseSql::Pontuacao => self.pu,
        }
    }

    /// A cor de uma classe do léxico de JSON.
    pub fn da_classe_json(&self, classe: ClasseJson) -> Color32 {
        match classe {
            ClasseJson::Comum => self.texto,
            ClasseJson::Chave => self.chave,
            ClasseJson::Texto => self.st,
            ClasseJson::Numero => self.nu,
            ClasseJson::Literal => self.kw,
            ClasseJson::Pontuacao => self.pu,
        }
    }
}

/// O texto com as cores, sem quebra (`white-space:pre`), numa fonte só. Os trechos são os de [`trechos_sql`]; o
/// texto vazio fica com uma seção vazia (para a linha ter a altura da fonte).
pub fn realcar_sql(texto: &str, cores: &CoresResolvidas, fonte: FontId) -> LayoutJob {
    realcar(texto, Linguagem::Sql, cores, fonte)
}

/// O mesmo para o JSON, com os trechos de [`trechos_json`]: chave em `chave`, texto em `st`, número em `nu`,
/// `true`/`false`/`null` em `kw`, pontuação em `pu`.
pub fn realcar_json(texto: &str, cores: &CoresResolvidas, fonte: FontId) -> LayoutJob {
    realcar(texto, Linguagem::Json, cores, fonte)
}

/// O texto com as cores da `linguagem` (sem quebra, numa fonte só); [`Linguagem::Texto`] fica numa cor só.
pub fn realcar(texto: &str, linguagem: Linguagem, cores: &CoresResolvidas, fonte: FontId) -> LayoutJob {
    montar_job(texto, &cores_dos_trechos(texto, linguagem, cores), cores, fonte)
}

/// Os trechos do texto já com a cor de cada um (vazio: nenhum trecho, o texto todo na cor do texto).
fn cores_dos_trechos(texto: &str, linguagem: Linguagem, cores: &CoresResolvidas) -> Vec<(Range<usize>, Color32)> {
    match linguagem {
        Linguagem::Sql => trechos_sql(texto).into_iter().map(|t| (t.bytes, cores.da_classe(t.classe))).collect(),
        Linguagem::Json => trechos_json(texto).into_iter().map(|t| (t.bytes, cores.da_classe_json(t.classe))).collect(),
        Linguagem::Texto => Vec::new(),
    }
}

fn montar_job(texto: &str, trechos: &[(Range<usize>, Color32)], cores: &CoresResolvidas, fonte: FontId) -> LayoutJob {
    let mut job = LayoutJob { text: texto.to_owned(), keep_trailing_whitespace: true, ..Default::default() };
    job.wrap.max_width = f32::INFINITY;
    let secao = |bytes: Range<usize>, cor: Color32| LayoutSection {
        leading_space: 0.0,
        byte_range: ByteIndex(bytes.start)..ByteIndex(bytes.end),
        format: TextFormat::simple(fonte.clone(), cor),
    };
    if trechos.is_empty() {
        job.sections.push(secao(0..texto.len(), cores.texto));
    } else {
        job.sections.extend(trechos.iter().map(|(bytes, cor)| secao(bytes.clone(), *cor)));
    }
    job
}

/// O tamanho (em bytes) a partir do qual o texto não leva realce: o léxico e as dezenas de milhares de seções do
/// `LayoutJob` custariam mais que o quadro. O texto continua no editor, só que numa cor.
pub const LIMITE_REALCE: usize = 200_000;

/// Tudo de que o [`LayoutJob`] do realce depende: o texto (pelo conteúdo), a linguagem, as cores, a fonte, a altura
/// da linha e a largura de quebra (em bits: `None` = não quebra). Dois quadros com a mesma chave usam o mesmo job.
#[derive(Clone, Copy, Hash)]
struct ChaveRealce<'a> {
    texto: &'a str,
    linguagem: Linguagem,
    cores: CoresResolvidas,
    fonte: &'a FontId,
    altura: u32,
    quebra: Option<u32>,
}

/// O cache do realce: o job pronto (o léxico e as seções incluídos) por [`ChaveRealce`], que dura enquanto o quadro
/// seguinte o pedir (o texto do editor muda pouco entre um quadro e outro; refazer as seções a cada quadro era O(n)).
#[derive(Default)]
struct Realce;

impl<'a> ComputerMut<ChaveRealce<'a>, Arc<LayoutJob>> for Realce {
    fn compute(&mut self, chave: ChaveRealce<'a>) -> Arc<LayoutJob> {
        let trechos = if chave.texto.len() > LIMITE_REALCE {
            Vec::new()
        } else {
            cores_dos_trechos(chave.texto, chave.linguagem, &chave.cores)
        };
        let mut job = montar_job(chave.texto, &trechos, &chave.cores, chave.fonte.clone());
        let altura = f32::from_bits(chave.altura);
        for s in &mut job.sections {
            s.format.line_height = Some(altura);
        }
        if let Some(largura) = chave.quebra {
            job.wrap.max_width = f32::from_bits(largura);
        }
        Arc::new(job)
    }
}

/// O [`realcar`] com a altura de linha e a largura de quebra (`None` = não quebra), no cache do contexto: o job só é
/// refeito quando o texto, a linguagem, as cores, a fonte, a altura ou a quebra mudam (o SQL e o JSON do mesmo texto
/// são jobs diferentes). Acima de [`LIMITE_REALCE`] bytes, sem realce.
pub(crate) fn realce_em_cache(
    ctx: &Context,
    texto: &str,
    linguagem: Linguagem,
    cores: &CoresResolvidas,
    fonte: &FontId,
    altura_da_linha: f32,
    quebra: Option<f32>,
) -> Arc<LayoutJob> {
    let chave = ChaveRealce {
        texto,
        linguagem,
        cores: *cores,
        fonte,
        altura: altura_da_linha.to_bits(),
        quebra: quebra.map(f32::to_bits),
    };
    ctx.memory_mut(|m| m.caches.cache::<FrameCache<Arc<LayoutJob>, Realce>>().get(chave).clone())
}

// ---- aparência no contexto -------------------------------------------------------------------------

/// O tamanho padrão da fonte do código (item 16 da revisão), o mesmo do `prefs::TAMANHO_CODIGO_PADRAO`.
pub const TAMANHO_PADRAO: f32 = 12.5;

/// A aparência do código em vigor: as cores, a família (None = a padrão, Consolas) e o tamanho em px.
#[derive(Debug, Clone, PartialEq)]
pub struct AparenciaCodigo {
    pub cores: CoresCodigo,
    pub familia: Option<String>,
    pub tamanho: f32,
}

impl Default for AparenciaCodigo {
    fn default() -> Self {
        AparenciaCodigo { cores: CoresCodigo::DataBox, familia: None, tamanho: TAMANHO_PADRAO }
    }
}

fn id_aparencia() -> egui::Id {
    egui::Id::new("databox::codigo::aparencia")
}

/// Passa a valer `aparencia` no contexto (na hora: o próximo desenho já usa as cores e o tamanho), com o tamanho
/// também nos estilos `Monospace` e `mono` dos dois temas. Não troca a família: isso é com
/// [`crate::fontes::trocar_mono`] / [`crate::fontes::restaurar_mono`], depois de ler o arquivo.
pub fn definir(ctx: &Context, aparencia: AparenciaCodigo) {
    let tamanho = aparencia.tamanho;
    ctx.data_mut(|d| d.insert_temp(id_aparencia(), aparencia));
    aplicar_tamanho(ctx, tamanho);
}

/// O tamanho do código nos estilos de texto (o [`tema::aplicar`] chama de novo ao trocar o tema).
pub(crate) fn aplicar_tamanho(ctx: &Context, tamanho: f32) {
    let mono = tema::estilo(tema::estilos::MONO);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(tamanho));
        s.text_styles.insert(mono.clone(), FontId::monospace(tamanho));
    });
}

/// A aparência em vigor (a padrão, se nada foi definido).
pub fn atual(ctx: &Context) -> AparenciaCodigo {
    ctx.data(|d| d.get_temp::<AparenciaCodigo>(id_aparencia())).unwrap_or_default()
}

/// As cores em vigor, resolvidas no tema do `ui`.
pub fn cores(ui: &Ui) -> CoresResolvidas {
    atual(ui.ctx()).cores.resolver(tema::paleta(ui))
}

/// A fonte do código em vigor (a família monoespaçada, no tamanho escolhido).
pub fn fonte(ctx: &Context) -> FontId {
    FontId::monospace(atual(ctx).tamanho)
}

/// A altura da linha do código (`line-height:1.6`: 20px em 12,5px).
pub fn altura_linha(tamanho: f32) -> f32 {
    (tamanho * 1.6).round()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn classes(texto: &str) -> Vec<(&str, ClasseSql)> {
        trechos_sql(texto).into_iter().map(|t| (&texto[t.bytes], t.classe)).collect()
    }

    #[test]
    fn palavras_reservadas_em_ordem_e_sem_caixa() {
        let mut ordenadas = PALAVRAS_RESERVADAS;
        ordenadas.sort_unstable();
        assert_eq!(ordenadas, PALAVRAS_RESERVADAS, "a busca binária precisa da lista em ordem");
        assert_eq!(PALAVRAS_RESERVADAS.iter().map(|p| p.len()).max(), Some(MAIOR_PALAVRA));
        for p in ["select", "Select", "CURRENT_TIMESTAMP", "without"] {
            assert!(palavra_reservada(p), "{p}");
        }
        for p in ["", "selecta", "clientes", "seléct", "CURRENT_TIMESTAMPS"] {
            assert!(!palavra_reservada(p), "{p}");
        }
    }

    /// C5: o job do realce vem do cache (o mesmo `Arc`) enquanto o texto, as cores, a fonte, a altura e a quebra são os
    /// mesmos; qualquer um que mude refaz; acima do limite, sem realce (uma seção só, na cor do texto).
    #[test]
    fn o_realce_vem_do_cache_e_refaz_so_quando_algo_muda() {
        let ctx = Context::default();
        let cores = CoresCodigo::Monokai.resolver(tema::paleta_ctx(&ctx));
        let fonte = FontId::monospace(12.5);
        let texto = "SELECT nome FROM clientes WHERE id = 1;";
        let a = realce_em_cache(&ctx, texto, Linguagem::Sql, &cores, &fonte, 20.0, None);
        let b = realce_em_cache(&ctx, texto, Linguagem::Sql, &cores, &fonte, 20.0, None);
        assert!(Arc::ptr_eq(&a, &b), "o mesmo job, sem refazer");
        assert!(a.sections.len() > 1 && a.sections.iter().all(|s| s.format.line_height == Some(20.0)));
        // o texto, as cores, a fonte, a altura e a quebra: cada um refaz
        let depois = [
            realce_em_cache(&ctx, "SELECT 2;", Linguagem::Sql, &cores, &fonte, 20.0, None),
            realce_em_cache(
                &ctx,
                texto,
                Linguagem::Sql,
                &CoresCodigo::AltoContraste.resolver(tema::paleta_ctx(&ctx)),
                &fonte,
                20.0,
                None,
            ),
            realce_em_cache(&ctx, texto, Linguagem::Sql, &cores, &FontId::monospace(11.0), 20.0, None),
            realce_em_cache(&ctx, texto, Linguagem::Sql, &cores, &fonte, 24.0, None),
            realce_em_cache(&ctx, texto, Linguagem::Sql, &cores, &fonte, 20.0, Some(300.0)),
        ];
        assert!(depois.iter().all(|j| !Arc::ptr_eq(&a, j)));
        assert_eq!(depois[4].wrap.max_width, 300.0);
        // o texto enorme não leva realce (nem o léxico dele)
        let enorme = "SELECT 1; ".repeat(LIMITE_REALCE / 10 + 10);
        assert!(enorme.len() > LIMITE_REALCE);
        let j = realce_em_cache(&ctx, &enorme, Linguagem::Sql, &cores, &fonte, 20.0, None);
        assert_eq!(j.sections.len(), 1);
        assert_eq!(j.text.len(), enorme.len());
        assert_eq!(j.sections[0].format.color, cores.texto);
    }

    #[test]
    fn citar_so_quando_precisa_e_sempre_a_palavra_reservada() {
        assert_eq!(citar("clientes"), "clientes");
        assert_eq!(citar("_x1"), "_x1");
        assert_eq!(citar("v_pedidos_por_cliente"), "v_pedidos_por_cliente");
        assert_eq!(citar("Tabela com espaço"), "\"Tabela com espaço\"");
        assert_eq!(citar("order"), "\"order\"");
        assert_eq!(citar("Select"), "\"Select\"");
        assert_eq!(citar("group"), "\"group\"");
        assert_eq!(citar("1a"), "\"1a\"");
        assert_eq!(citar("2026"), "\"2026\"");
        assert_eq!(citar("ação"), "\"ação\"");
        assert_eq!(citar("a\"b"), "\"a\"\"b\"");
        assert_eq!(citar("diz \"oi\""), "\"diz \"\"oi\"\"\"");
        assert_eq!(citar(""), "\"\"");
    }

    #[test]
    fn realce_sql_separa_palavras_textos_numeros_e_comentarios() {
        use ClasseSql::*;
        assert_eq!(
            classes("SELECT nome, limite\n  FROM \"clientes\"\n WHERE limite > 1000;"),
            [
                ("SELECT", PalavraChave),
                (" nome", Comum),
                (",", Pontuacao),
                (" limite\n  ", Comum),
                ("FROM", PalavraChave),
                (" ", Comum),
                ("\"clientes\"", Citado),
                ("\n ", Comum),
                ("WHERE", PalavraChave),
                (" limite ", Comum),
                (">", Pontuacao),
                (" ", Comum),
                ("1000", Numero),
                (";", Pontuacao),
            ]
        );
        assert_eq!(
            classes("-- a 'linha'\nselect 'it''s', x'0A', 1.5e-3, .5, 0x1F /* fim */"),
            [
                ("-- a 'linha'", Comentario),
                ("\n", Comum),
                ("select", PalavraChave),
                (" ", Comum),
                ("'it''s'", Texto),
                (",", Pontuacao),
                (" ", Comum),
                ("x'0A'", Texto),
                (",", Pontuacao),
                (" ", Comum),
                ("1.5e-3", Numero),
                (",", Pontuacao),
                (" ", Comum),
                (".5", Numero),
                (",", Pontuacao),
                (" ", Comum),
                ("0x1F", Numero),
                (" ", Comum),
                ("/* fim */", Comentario),
            ]
        );
        // nomes com algarismos não viram número; parâmetros ficam com o nome
        assert_eq!(
            classes("t1.c2 = :limit"),
            [("t1", Comum), (".", Pontuacao), ("c2 ", Comum), ("=", Pontuacao), (" :limit", Comum),]
        );
        // o que não fecha vai até o fim
        assert_eq!(classes("'aberto"), [("'aberto", Texto)]);
        assert_eq!(classes("/* sem fim"), [("/* sem fim", Comentario)]);
        assert!(trechos_sql("").is_empty());
    }

    #[test]
    fn realce_sql_nao_perde_nenhum_byte() {
        let amostras = [
            "SELECT \"a\tb\", 'ç' -- \u{1}\u{7f}\u{2028}\n  FROM t",
            "INSERT INTO [São Paulo] VALUES ('ação', X'FF', 1e, 2e+, 0x, -.5);",
            "ção`não fecha",
            "\"\"\"\"''''[[]]`` 😀 /**/ -- \r\n\r\n",
            "a-b-c--d",
            "?1 @x $y :z ?",
        ];
        let cores = CoresCodigo::Monokai.resolver(&tema::ESCURA);
        for texto in amostras {
            let trechos = trechos_sql(texto);
            let mut fim = 0;
            for t in &trechos {
                assert_eq!(t.bytes.start, fim, "{texto:?}: buraco antes de {t:?}");
                assert!(t.bytes.end > t.bytes.start);
                fim = t.bytes.end;
            }
            assert_eq!(fim, texto.len(), "{texto:?}");
            let job = realcar_sql(texto, &cores, FontId::monospace(12.5));
            let juntas: String =
                job.sections.iter().map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0]).collect();
            assert_eq!(juntas, texto);
            assert_eq!(job.text, texto);
        }
        // o vazio fica com uma seção vazia, na cor do texto
        let vazio = realcar_sql("", &cores, FontId::monospace(12.5));
        assert_eq!(vazio.sections.len(), 1);
        assert_eq!(vazio.sections[0].format.color, cores.texto);
    }

    #[test]
    fn identificador_citado_nao_e_palavra_chave() {
        use ClasseSql::*;
        assert_eq!(classes("\"select\""), [("\"select\"", Citado)]);
        assert_eq!(classes("`from`"), [("`from`", Citado)]);
        assert_eq!(classes("[where]"), [("[where]", Citado)]);
        assert_eq!(classes("\"a\"\"b\" x"), [("\"a\"\"b\"", Citado), (" x", Comum)]);
        // e não vira texto: tem a cor `st`, como na prancheta, mas a classe é outra
        let c = CoresCodigo::DataBox.resolver(&tema::CLARA);
        assert_eq!(c.da_classe(Citado), c.st);
        assert_eq!(c.da_classe(PalavraChave), c.kw);
        assert_ne!(c.kw, c.st);
    }

    #[test]
    fn cores_das_quatro_opcoes_como_na_prancheta_13() {
        // as amostras da prancheta 13 (`.cc-p`), no tema escuro
        let tabela = [
            (CoresCodigo::DataBox, [0x191a1d, 0xececee, 0x8ab4f8, 0x7fd1a8, 0xf2b65e, 0x9597a0]),
            (CoresCodigo::AltoContraste, [0x000000, 0xffffff, 0x66c2ff, 0x8cff8c, 0xffd866, 0xc8c8c8]),
            (CoresCodigo::SolarizedEscuro, [0x002b36, 0x93a1a1, 0x859900, 0x2aa198, 0xd33682, 0x657b83]),
            (CoresCodigo::Monokai, [0x272822, 0xf8f8f2, 0xf92672, 0xe6db74, 0xae81ff, 0xa59f85]),
        ];
        let hex = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        for (cores, [fundo, texto, kw, st, nu, pu]) in tabela {
            let r = cores.resolver(&tema::ESCURA);
            assert_eq!(
                r,
                CoresResolvidas {
                    fundo: hex(fundo),
                    texto: hex(texto),
                    kw: hex(kw),
                    st: hex(st),
                    nu: hex(nu),
                    pu: hex(pu),
                    chave: r.chave,
                },
                "{cores:?}"
            );
        }
        assert_eq!(CoresCodigo::TODAS.map(CoresCodigo::rotulo), ["DataBox", "Alto contraste", "Solarized", "Monokai"]);
        assert_eq!(CoresCodigo::TODAS.map(CoresCodigo::nota), ["segue o tema", "", "escuro", ""]);
        for c in CoresCodigo::TODAS {
            assert_eq!(CoresCodigo::de_str(c.como_str()), Some(c));
        }
        assert_eq!(CoresCodigo::de_str("Monokai"), None);
    }

    #[test]
    fn databox_segue_o_tema() {
        let (claro, escuro) =
            (CoresCodigo::DataBox.resolver(&tema::CLARA), CoresCodigo::DataBox.resolver(&tema::ESCURA));
        let e = &tema::CLARA.estudio;
        assert_eq!(
            claro,
            CoresResolvidas { fundo: e.doc, texto: e.t, kw: e.kw, st: e.st, nu: e.nu, pu: e.t3, chave: e.chave }
        );
        assert_ne!(claro, escuro);
        assert!(CoresCodigo::DataBox.segue_o_tema());
        // as outras são as mesmas nos dois temas
        for c in [CoresCodigo::AltoContraste, CoresCodigo::SolarizedEscuro, CoresCodigo::Monokai] {
            assert_eq!(c.resolver(&tema::CLARA), c.resolver(&tema::ESCURA));
            assert!(!c.segue_o_tema());
        }
    }

    #[test]
    fn aparencia_fica_no_contexto_e_muda_o_estilo_mono() {
        let ctx = Context::default();
        assert_eq!(atual(&ctx), AparenciaCodigo::default());
        assert_eq!(altura_linha(TAMANHO_PADRAO), 20.0);
        definir(
            &ctx,
            AparenciaCodigo { cores: CoresCodigo::Monokai, familia: Some("Cascadia Mono".into()), tamanho: 14.0 },
        );
        assert_eq!(atual(&ctx).cores, CoresCodigo::Monokai);
        assert_eq!(fonte(&ctx), FontId::monospace(14.0));
        for tema_egui in [egui::Theme::Light, egui::Theme::Dark] {
            let estilo = ctx.style_of(tema_egui);
            assert_eq!(estilo.text_styles[&TextStyle::Monospace], FontId::monospace(14.0));
            assert_eq!(estilo.text_styles[&tema::estilo(tema::estilos::MONO)], FontId::monospace(14.0));
        }
    }

    // ---- JSON ----

    fn classes_json(texto: &str) -> Vec<(&str, ClasseJson)> {
        trechos_json(texto).into_iter().map(|t| (&texto[t.bytes], t.classe)).collect()
    }

    /// O exemplo da prancheta 8: chave, texto, número, literal e pontuação; uma cadeia seguida de `:` (mesmo com
    /// espaços) é chave, não texto. E nenhum byte se perde.
    #[test]
    fn trechos_json_nao_perdem_byte_e_marcam_chave() {
        use ClasseJson::*;
        assert_eq!(
            classes_json("{\n  \"versao\": 3,\n  \"tema\" : \"escuro\",\n  \"ok\": [true, null, -1.5e+3]\n}"),
            [
                ("{", Pontuacao),
                ("\n  ", Comum),
                ("\"versao\"", Chave),
                (":", Pontuacao),
                (" ", Comum),
                ("3", Numero),
                (",", Pontuacao),
                ("\n  ", Comum),
                ("\"tema\"", Chave),
                (" ", Comum),
                (":", Pontuacao),
                (" ", Comum),
                ("\"escuro\"", Texto),
                (",", Pontuacao),
                ("\n  ", Comum),
                ("\"ok\"", Chave),
                (":", Pontuacao),
                (" ", Comum),
                ("[", Pontuacao),
                ("true", Literal),
                (",", Pontuacao),
                (" ", Comum),
                ("null", Literal),
                (",", Pontuacao),
                (" ", Comum),
                ("-1.5e+3", Numero),
                ("]", Pontuacao),
                ("\n", Comum),
                ("}", Pontuacao),
            ]
        );
        let cores = CoresCodigo::Monokai.resolver(&tema::ESCURA);
        let amostras = [
            "{\"a\": \"ç\\\"b\", \"😀\": [1, 2.5E-3, false]}",
            "[\"aberta\n, 1]",
            "\"\\",
            "\"a\\\nb\"",
            "{\"a\" \t\r: 1}",
            "-",
            "--1",
            "1.e5 .5 01 0x1F",
            "ação: né 😀 /* x */ // y",
        ];
        for texto in amostras {
            let trechos = trechos_json(texto);
            let mut fim = 0;
            for t in &trechos {
                assert_eq!(t.bytes.start, fim, "{texto:?}: buraco antes de {t:?}");
                assert!(t.bytes.end > t.bytes.start);
                assert!(texto.is_char_boundary(t.bytes.start) && texto.is_char_boundary(t.bytes.end));
                fim = t.bytes.end;
            }
            assert_eq!(fim, texto.len(), "{texto:?}");
            let job = realcar_json(texto, &cores, FontId::monospace(12.5));
            let juntas: String =
                job.sections.iter().map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0]).collect();
            assert_eq!(juntas, texto);
        }
        // o vazio não tem trecho e o job fica com uma seção vazia, na cor do texto
        assert!(trechos_json("").is_empty());
        let vazio = realcar_json("", &cores, FontId::monospace(12.5));
        assert_eq!((vazio.sections.len(), vazio.sections[0].format.color), (1, cores.texto));
    }

    /// O documento quebrado também recebe as cores (o editor mostra o JSON inválido enquanto a pessoa digita): sem a
    /// vírgula, com aspas simples, comentário e literal incompleto, a chave continua chave.
    #[test]
    fn trechos_json_valem_para_json_invalido() {
        use ClasseJson::*;
        assert_eq!(
            classes_json("{\"a\": 1\n \"b\": 'x', // c\n \"d\": tru"),
            [
                ("{", Pontuacao),
                ("\"a\"", Chave),
                (":", Pontuacao),
                (" ", Comum),
                ("1", Numero),
                ("\n ", Comum),
                ("\"b\"", Chave),
                (":", Pontuacao),
                (" 'x'", Comum),
                (",", Pontuacao),
                (" // c\n ", Comum),
                ("\"d\"", Chave),
                (":", Pontuacao),
                (" tru", Comum),
            ]
        );
        // cadeia que não fecha: vai até a quebra e é texto (nunca chave); a linha de baixo recomeça limpa
        assert_eq!(
            classes_json("\"aberta: 1\n\"b\": 2"),
            [("\"aberta: 1", Texto), ("\n", Comum), ("\"b\"", Chave), (":", Pontuacao), (" ", Comum), ("2", Numero)]
        );
        // o `:` precisa estar na mesma linha da cadeia
        assert_eq!(classes_json("\"a\"\n: 1")[0], ("\"a\"", Texto));
        // palavras que não são literais não ganham a cor deles
        assert_eq!(classes_json("True nulls"), [("True nulls", Comum)]);
        assert_eq!(classes_json("false"), [("false", Literal)]);
    }

    /// O leitor de arquivo grande lexa linha a linha: o resultado (depois de juntar os vizinhos iguais) é o mesmo
    /// do texto todo, com cadeia que não fecha, `:` na linha de baixo, `\r\n` e barra no fim da linha.
    #[test]
    fn trechos_json_por_linha_dao_o_mesmo_que_o_texto_todo() {
        let amostras = [
            "{\n  \"a\": 1,\n  \"b\": [true, \"x\"]\n}\n",
            "{\r\n  \"a\"\r\n  : 1,\r\n  \"b\" : \"aberta\r\n}",
            "[\"a\\\n\"b\": 2, \"c\\",
            "\"x\"\n:\n\"y\"\n\n\n  ",
            "ação 😀 \"é\": -0.5e3\n\"fim",
        ];
        for texto in amostras {
            let todo = trechos_json(texto);
            let mut por_linha: Vec<TrechoJson> = Vec::new();
            let mut base = 0;
            for linha in texto.split_inclusive('\n') {
                for t in trechos_json(linha) {
                    let (bytes, classe) = (t.bytes.start + base..t.bytes.end + base, t.classe);
                    match por_linha.last_mut() {
                        Some(u) if u.classe == classe && u.bytes.end == bytes.start => u.bytes.end = bytes.end,
                        _ => por_linha.push(TrechoJson { bytes, classe }),
                    }
                }
                base += linha.len();
            }
            assert_eq!(por_linha, todo, "{texto:?}");
        }
    }

    /// Cada classe na sua cor: `chave`, `st`, `nu`, `kw` (true/false/null), `pu` e o texto.
    #[test]
    fn json_colore_chave_texto_numero_literal_e_pontuacao() {
        let texto = "{\"nome\": \"Produção\", \"n\": 3, \"ok\": true, \"x\": null, \"sobra\": ?}";
        for carta in CoresCodigo::TODAS {
            let cores = carta.resolver(&tema::ESCURA);
            let job = realcar_json(texto, &cores, FontId::monospace(12.5));
            let cor_de = |trecho: &str| {
                let b = texto.find(trecho).unwrap();
                job.sections.iter().find(|s| s.byte_range.start.0 <= b && b < s.byte_range.end.0).unwrap().format.color
            };
            assert_eq!(cor_de("\"nome\""), cores.chave, "{carta:?}");
            assert_eq!(cor_de("\"Produção\""), cores.st, "{carta:?}");
            assert_eq!(cor_de("3"), cores.nu, "{carta:?}");
            assert_eq!(cor_de("true"), cores.kw, "{carta:?}");
            assert_eq!(cor_de("null"), cores.kw, "{carta:?}");
            assert_eq!(cor_de("{"), cores.pu, "{carta:?}");
            assert_eq!(cor_de("?"), cores.texto, "{carta:?}");
        }
        // o DataBox usa a `--chave` do tema, claro e escuro
        assert_eq!(CoresCodigo::DataBox.resolver(&tema::ESCURA).chave, tema::ESCURA.estudio.chave);
        assert_eq!(CoresCodigo::DataBox.resolver(&tema::CLARA).chave, tema::CLARA.estudio.chave);
        assert_ne!(tema::ESCURA.estudio.chave, tema::CLARA.estudio.chave);
    }

    /// O contraste WCAG entre duas cores opacas.
    fn contraste(a: Color32, b: Color32) -> f64 {
        let luz = |c: Color32| {
            let canal = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * canal(c.r()) + 0.7152 * canal(c.g()) + 0.0722 * canal(c.b())
        };
        let (x, y) = (luz(a), luz(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// As quatro cartas têm a cor da chave (a das Preferências no plano: Alto contraste `#E0B0FF`, Monokai
    /// `#A6E22E`, DataBox a `--chave` do tema; o Solarized sobe do `#6C71C4`, a violeta do esquema, que dá só 3,4:1,
    /// para `#8489D6`), distinta das outras cores da carta e com 4,5:1 sobre o fundo dela.
    #[test]
    fn cada_carta_tem_cor_de_chave_com_contraste() {
        let hex = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        assert_eq!(CoresCodigo::AltoContraste.resolver(&tema::ESCURA).chave, hex(0xE0B0FF));
        assert_eq!(CoresCodigo::SolarizedEscuro.resolver(&tema::ESCURA).chave, hex(0x8489D6));
        assert_eq!(CoresCodigo::Monokai.resolver(&tema::ESCURA).chave, hex(0xA6E22E));
        assert_eq!(CoresCodigo::DataBox.resolver(&tema::ESCURA).chave, hex(0xC9A0FF));
        assert_eq!(CoresCodigo::DataBox.resolver(&tema::CLARA).chave, hex(0x7A3FC4));
        for paleta in [&tema::ESCURA, &tema::CLARA] {
            for carta in CoresCodigo::TODAS {
                let c = carta.resolver(paleta);
                let razao = contraste(c.chave, c.fundo);
                assert!(razao >= 4.5, "{carta:?}: a chave {:?} sobre {:?} tem só {razao:.2}:1", c.chave, c.fundo);
                for (nome, outra) in [("kw", c.kw), ("st", c.st), ("nu", c.nu), ("texto", c.texto)] {
                    assert_ne!(c.chave, outra, "{carta:?}: a chave repete a cor {nome}");
                }
            }
        }
    }

    /// O mesmo texto em SQL, em JSON e como texto simples são jobs diferentes no cache (e o JSON acima do limite
    /// fica sem cores como o SQL).
    #[test]
    fn realce_em_cache_separa_sql_de_json() {
        let ctx = Context::default();
        let cores = CoresCodigo::Monokai.resolver(tema::paleta_ctx(&ctx));
        let fonte = FontId::monospace(12.5);
        let texto = "{\"a\": 1}";
        let sql = realce_em_cache(&ctx, texto, Linguagem::Sql, &cores, &fonte, 20.0, None);
        let json = realce_em_cache(&ctx, texto, Linguagem::Json, &cores, &fonte, 20.0, None);
        let simples = realce_em_cache(&ctx, texto, Linguagem::Texto, &cores, &fonte, 20.0, None);
        assert!(!Arc::ptr_eq(&sql, &json) && !Arc::ptr_eq(&json, &simples) && !Arc::ptr_eq(&sql, &simples));
        assert!(Arc::ptr_eq(&json, &realce_em_cache(&ctx, texto, Linguagem::Json, &cores, &fonte, 20.0, None)));
        let cor_de_aspas = |job: &LayoutJob| {
            job.sections.iter().find(|s| job.text[s.byte_range.start.0..].starts_with('"')).map(|s| s.format.color)
        };
        assert_eq!(cor_de_aspas(&sql), Some(cores.st), "no SQL, \"a\" é identificador citado");
        assert_eq!(cor_de_aspas(&json), Some(cores.chave), "no JSON, \"a\" é chave");
        assert_eq!(simples.sections.len(), 1);
        assert_eq!(simples.sections[0].format.color, cores.texto);
        assert!(json.sections.iter().all(|s| s.format.line_height == Some(20.0)));
        let enorme = "{\"a\": 1}\n".repeat(LIMITE_REALCE / 9 + 10);
        let j = realce_em_cache(&ctx, &enorme, Linguagem::Json, &cores, &fonte, 20.0, None);
        assert_eq!((j.sections.len(), j.sections[0].format.color), (1, cores.texto));
    }
}
