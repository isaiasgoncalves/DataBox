//! Tokenizador de SQL (dialeto do SQLite) e as peças de texto que o resto do crate usa.
//!
//! Posições (`Token::inicio`/`Token::fim`) são em **bytes** do texto UTF-8 e caem sempre numa fronteira de
//! caractere: `&sql[t.inicio..t.fim] == t.texto` vale para todo token, com ou sem acento. A única posição em
//! caracteres é a das mensagens de erro ("começa na posição 8").

use comum::{ErroBanco, Resultado};

/// Tipo de um token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoToken {
    /// Palavra sem aspas: palavra-chave, nome, função ou parâmetro nomeado (`:x`, `@x`, `$x`).
    Palavra,
    /// Nome citado: `"a"`, `` `a` `` ou `[a]`.
    Ident,
    /// Texto entre aspas simples (`'a'`); "string" no `como_str`.
    Texto,
    Numero,
    /// Operador ou pontuação (`(`, `,`, `||`, `->>`, `?1`…).
    Op,
    Espaco,
    /// `-- até o fim da linha` ou `/* bloco */`.
    Comentario,
    /// `X'00ff'`.
    Blob,
}

impl TipoToken {
    /// O texto do tipo: "palavra", "ident", "string", "numero", "op", "espaco",
    /// "comentario" ou "blob".
    pub fn como_str(self) -> &'static str {
        match self {
            TipoToken::Palavra => "palavra",
            TipoToken::Ident => "ident",
            TipoToken::Texto => "string",
            TipoToken::Numero => "numero",
            TipoToken::Op => "op",
            TipoToken::Espaco => "espaco",
            TipoToken::Comentario => "comentario",
            TipoToken::Blob => "blob",
        }
    }
}

/// Todos os tipos, em ordem fixa.
pub const TIPOS_TOKEN: [TipoToken; 8] = [
    TipoToken::Palavra,
    TipoToken::Ident,
    TipoToken::Texto,
    TipoToken::Numero,
    TipoToken::Op,
    TipoToken::Espaco,
    TipoToken::Comentario,
    TipoToken::Blob,
];

const OPERADORES_3: [&str; 1] = ["->>"];
const OPERADORES_2: [&str; 9] = ["||", "<=", ">=", "<>", "!=", "==", "<<", ">>", "->"];

/// Um pedaço do SQL. `texto == &sql[inicio..fim]` (posições em bytes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub tipo: TipoToken,
    pub texto: String,
    pub inicio: usize,
    pub fim: usize,
}

impl Token {
    /// Texto em maiúsculas (Unicode, como o `str.upper()`), para comparar palavras-chave.
    pub fn alto(&self) -> String {
        self.texto.to_uppercase()
    }

    /// Verdadeiro se o token é uma palavra (sem aspas) cujo texto em maiúsculas é igual a uma das dadas
    /// (passe as palavras em maiúsculas).
    pub fn e(&self, palavras: &[&str]) -> bool {
        if self.tipo != TipoToken::Palavra {
            return false;
        }
        if self.texto.is_ascii() {
            palavras.iter().any(|p| {
                p.len() == self.texto.len()
                    && self.texto.bytes().zip(p.bytes()).all(|(a, b)| a.to_ascii_uppercase() == b)
            })
        } else {
            // Fora do ASCII o maiúsculo pode mudar de tamanho e até virar palavra-chave ("ın" → "IN").
            let alto = self.alto();
            palavras.iter().any(|p| alto == *p)
        }
    }

    /// Operador com exatamente este texto (`tipo == Op`).
    pub fn e_op(&self, op: &str) -> bool {
        self.tipo == TipoToken::Op && self.texto == op
    }
}

// ---------------------------------------------------------------------------------------------------
// Texto

/// Espaço: o White_Space do Unicode e
/// também os separadores ASCII 0x1C–0x1F.
pub fn e_espaco(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Tira [`e_espaco`] das pontas.
pub fn aparar(texto: &str) -> &str {
    texto.trim_matches(e_espaco)
}

/// Tira [`e_espaco`] do fim.
pub(crate) fn aparar_fim(texto: &str) -> &str {
    texto.trim_end_matches(e_espaco)
}

/// `re.sub(r"\s+", " ", texto)`: cada sequência de espaços vira um espaço (sem aparar as pontas).
pub fn colapsar_espacos(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut em_espaco = false;
    for ch in texto.chars() {
        if e_espaco(ch) {
            if !em_espaco {
                saida.push(' ');
                em_espaco = true;
            }
        } else {
            saida.push(ch);
            em_espaco = false;
        }
    }
    saida
}

/// Verdadeiro se o texto tem algo além de espaços.
pub fn preenchido(texto: &str) -> bool {
    !aparar(texto).is_empty()
}

/// `&sql[inicio..fim]` sem risco de pânico: fora do texto ou fora de fronteira de caractere devolve "".
/// Use com as posições dos tokens (`fatia(sql, toks[0].inicio, toks[k].fim)`, `fatia(sql, t.inicio, sql.len())`).
pub fn fatia(sql: &str, inicio: usize, fim: usize) -> &str {
    sql.get(inicio..fim).unwrap_or("")
}

/// Posição em caracteres (contando de 0) do byte `i`: a das mensagens de erro.
fn posicao_em_caracteres(sql: &str, i: usize) -> usize {
    sql.get(..i).map_or(0, |s| s.chars().count())
}

// ---------------------------------------------------------------------------------------------------
// Tokenizador

/// Letra para um identificador sem aspas. O SQLite aceita qualquer caractere fora do ASCII.
fn e_letra(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch as u32 >= 0x80
}

fn e_letra_ou_digito(ch: char) -> bool {
    e_letra(ch) || ch.is_ascii_digit() || ch == '$'
}

fn char_em(sql: &str, i: usize) -> Option<char> {
    sql.get(i..).and_then(|resto| resto.chars().next())
}

fn byte_em(sql: &str, i: usize) -> Option<u8> {
    sql.as_bytes().get(i).copied()
}

fn digito_em(sql: &str, i: usize) -> bool {
    byte_em(sql, i).is_some_and(|b| b.is_ascii_digit())
}

/// Posição logo depois da aspa que fecha o trecho aberto no byte `i`; aspa dobrada é escape.
fn fechar_aspas(sql: &str, i: usize, aspa: u8, rotulo: &str) -> Resultado<usize> {
    let mut j = i + 1;
    while let Some(b) = byte_em(sql, j) {
        if b == aspa {
            if byte_em(sql, j + 1) == Some(aspa) {
                j += 2;
                continue;
            }
            return Ok(j + 1);
        }
        j += 1;
    }
    Err(ErroBanco::novo(format!(
        "O comando SQL tem {rotulo} sem fechamento (começa na posição {}).",
        posicao_em_caracteres(sql, i) + 1
    )))
}

fn fim_numero(sql: &str, i: usize) -> usize {
    if byte_em(sql, i) == Some(b'0') && matches!(byte_em(sql, i + 1), Some(b'x' | b'X')) {
        let mut j = i + 2;
        while byte_em(sql, j).is_some_and(|b| b.is_ascii_hexdigit() || b == b'_') {
            j += 1;
        }
        return j;
    }
    let mut j = i;
    while byte_em(sql, j).is_some_and(|b| b.is_ascii_digit() || (b == b'_' && digito_em(sql, j + 1))) {
        j += 1;
    }
    if byte_em(sql, j) == Some(b'.') {
        j += 1;
        while digito_em(sql, j) {
            j += 1;
        }
    }
    if matches!(byte_em(sql, j), Some(b'e' | b'E')) {
        let mut k = j + 1;
        if matches!(byte_em(sql, k), Some(b'+' | b'-')) {
            k += 1;
        }
        if digito_em(sql, k) {
            while digito_em(sql, k) {
                k += 1;
            }
            j = k;
        }
    }
    j
}

/// Fim (em bytes) da sequência que começa em `i` e segue enquanto `continua` aceitar o caractere.
fn avancar_enquanto(sql: &str, mut j: usize, continua: impl Fn(char) -> bool) -> usize {
    while let Some(ch) = char_em(sql, j) {
        if !continua(ch) {
            break;
        }
        j += ch.len_utf8();
    }
    j
}

/// Divide o SQL em tokens, com a posição (em bytes) de cada um no texto original
/// (`&sql[t.inicio..t.fim] == t.texto`).
///
/// Texto entre aspas, identificador citado ou comentário `/*` sem fechamento → [`ErroBanco`].
pub fn tokenizar(sql: &str) -> Resultado<Vec<Token>> {
    let mut tokens = Vec::new();
    let n = sql.len();
    let mut i = 0;
    while let Some(ch) = char_em(sql, i) {
        let depois = i + ch.len_utf8();
        let (tipo, j) = if e_espaco(ch) {
            (TipoToken::Espaco, avancar_enquanto(sql, depois, e_espaco))
        } else if ch == '-' && byte_em(sql, i + 1) == Some(b'-') {
            let j = fatia(sql, i, n).find('\n').map_or(n, |p| i + p);
            (TipoToken::Comentario, j)
        } else if ch == '/' && byte_em(sql, i + 1) == Some(b'*') {
            match fatia(sql, i + 2, n).find("*/") {
                Some(p) => (TipoToken::Comentario, i + 2 + p + 2),
                None => return Err(ErroBanco::novo("O comando SQL tem um comentário /* sem fechamento.")),
            }
        } else if (ch == 'x' || ch == 'X') && byte_em(sql, i + 1) == Some(b'\'') {
            (TipoToken::Blob, fechar_aspas(sql, i + 1, b'\'', "um BLOB (X'…')")?)
        } else if ch == '\'' {
            (TipoToken::Texto, fechar_aspas(sql, i, b'\'', "um texto entre aspas simples")?)
        } else if ch == '"' {
            (TipoToken::Ident, fechar_aspas(sql, i, b'"', "um nome entre aspas duplas")?)
        } else if ch == '`' {
            (TipoToken::Ident, fechar_aspas(sql, i, b'`', "um nome entre crases")?)
        } else if ch == '[' {
            match fatia(sql, i + 1, n).find(']') {
                Some(p) => (TipoToken::Ident, i + 1 + p + 1),
                None => return Err(ErroBanco::novo("O comando SQL tem um nome entre colchetes sem fechamento.")),
            }
        } else if ch.is_ascii_digit() || (ch == '.' && digito_em(sql, i + 1)) {
            (TipoToken::Numero, fim_numero(sql, i))
        } else if e_letra(ch) {
            (TipoToken::Palavra, avancar_enquanto(sql, depois, e_letra_ou_digito))
        } else if matches!(ch, ':' | '@' | '$') && char_em(sql, depois).is_some_and(e_letra_ou_digito) {
            // parâmetro nomeado
            (TipoToken::Palavra, avancar_enquanto(sql, depois, e_letra_ou_digito))
        } else if ch == '?' {
            (TipoToken::Op, avancar_enquanto(sql, depois, |c| c.is_ascii_digit()))
        } else {
            let resto = fatia(sql, i, n);
            let achou = OPERADORES_3.iter().chain(OPERADORES_2.iter()).find(|o| resto.starts_with(**o));
            (TipoToken::Op, achou.map_or(depois, |o| i + o.len()))
        };
        tokens.push(Token { tipo, texto: fatia(sql, i, j).to_string(), inicio: i, fim: j });
        i = j;
    }
    Ok(tokens)
}

/// Tokens sem espaços e comentários.
pub fn significativos(sql: &str) -> Resultado<Vec<Token>> {
    Ok(significativos_de(tokenizar(sql)?))
}

/// [`significativos`] de uma lista já tokenizada.
pub fn significativos_de(tokens: Vec<Token>) -> Vec<Token> {
    tokens.into_iter().filter(|t| !matches!(t.tipo, TipoToken::Espaco | TipoToken::Comentario)).collect()
}

/// Nome sem aspas: `"a""b"` → `a"b`; `` `x` `` → `x`; `[x]` → `x`; `'x'` → `x`; palavra fica como está.
/// Para um token, passe `&t.texto`.
pub fn desfazer_aspas(texto: &str) -> String {
    let mut chars = texto.chars();
    let (Some(a), Some(z)) = (chars.next(), chars.next_back()) else {
        return texto.to_string();
    };
    let meio = chars.as_str();
    if a == '[' && z == ']' {
        return meio.to_string();
    }
    if a == z && matches!(a, '"' | '`' | '\'') {
        return meio.replace(&format!("{a}{a}"), &a.to_string());
    }
    texto.to_string()
}

/// Nome de tabela ou coluna: palavra, identificador citado ou texto entre aspas simples.
pub(crate) fn e_nome(t: &Token) -> bool {
    matches!(t.tipo, TipoToken::Palavra | TipoToken::Ident | TipoToken::Texto)
}

/// Índice do `)` que fecha o `(` em `toks[i]`.
pub fn fecha(toks: &[Token], i: usize) -> Resultado<usize> {
    let mut prof: i64 = 0;
    for (j, t) in toks.iter().enumerate().skip(i) {
        if t.e_op("(") {
            prof += 1;
        } else if t.e_op(")") {
            prof -= 1;
            if prof == 0 {
                return Ok(j);
            }
        }
    }
    Err(ErroBanco::novo("O comando CREATE TABLE tem parênteses sem fechamento."))
}

/// Texto original de `toks[i]` até `toks[j]` (inclusive).
pub(crate) fn trecho<'a>(sql: &'a str, toks: &[Token], i: usize, j: usize) -> &'a str {
    match (toks.get(i), toks.get(j)) {
        (Some(a), Some(z)) => fatia(sql, a.inicio, z.fim),
        _ => "",
    }
}

/// Põe o trecho numa linha só: cada sequência de espaços e quebras de linha vira um espaço.
///
/// Só mexe no que está fora de textos e nomes citados (`'a  b'` continua igual), e troca o comentário de
/// linha `-- x` por `/* x */`: numa linha só ele engoliria o resto do trecho (o ON DELETE CASCADE depois de
/// `REFERENCES t (id) -- dono`, o AUTOINCREMENT depois de `PRIMARY KEY -- chave`). Texto que não tokeniza
/// (aspas sem fechamento) só tem os espaços colapsados.
pub fn normalizar_espacos(texto: &str) -> String {
    let Ok(tokens) = tokenizar(texto) else {
        return aparar(&colapsar_espacos(texto)).to_string();
    };
    let mut partes: Vec<String> = Vec::with_capacity(tokens.len());
    for t in tokens {
        let pedaco = match t.tipo {
            TipoToken::Comentario if t.texto.starts_with("--") => {
                let corpo = colapsar_espacos(t.texto.get(2..).unwrap_or(""));
                let corpo = aparar(&corpo).replace("*/", "* /");
                if corpo.is_empty() { " ".to_string() } else { format!("/* {corpo} */") }
            }
            TipoToken::Comentario => colapsar_espacos(&t.texto),
            TipoToken::Espaco => " ".to_string(),
            _ => t.texto,
        };
        if pedaco == " " && partes.last().is_none_or(|p| p == " ") {
            continue;
        }
        partes.push(pedaco);
    }
    aparar(&partes.concat()).to_string()
}
