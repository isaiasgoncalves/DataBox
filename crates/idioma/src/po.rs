//! O analisador do `.po`, no subconjunto que o DataBox usa (o que o Poedit, o Weblate e o Crowdin gravam para um
//! projeto sem contexto):
//!
//! ```text
//! # comentário do tradutor          (qualquer linha que começa com #: #. #: #, #| também)
//! #, fuzzy                          (a entrada não vale: o tradutor ainda não confirmou)
//! msgid ""                          (o cabeçalho: Language e Plural-Forms)
//! msgstr ""
//! "Language: en\n"
//! "Plural-Forms: nplurals=2; plural=(n != 1);\n"
//!
//! msgid "Abrir pasta"
//! msgstr "Open folder"
//!
//! msgid "{n} linha"
//! msgid_plural "{n} linhas"
//! msgstr[0] "{n} row"
//! msgstr[1] "{n} rows"
//! ```
//!
//! - O texto pode continuar nas linhas seguintes (`"…"` sozinho na linha); as partes se juntam.
//! - Escapes aceitos: `\n`, `\t`, `\r`, `\"`, `\\`. Qualquer outro é erro.
//! - `msgstr` vazio (ou um `msgstr[n]` vazio) é entrada sem tradução: fica de fora e o texto cai no português.
//! - `#~` (entrada obsoleta) e `#, fuzzy` ficam de fora.
//! - `msgctxt` não é usado pelo projeto e é recusado (o código não tem como pedir um contexto).
//! - `msgid` repetido é erro (o `msgfmt` também recusa).
//! - O `Plural-Forms` é a expressão do gettext (a do C: `?:`, `||`, `&&`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `+`,
//!   `-`, `*`, `/`, `%`, `!`, parênteses, `n` e inteiros). Sem ele, vale a do inglês e do português:
//!   `nplurals=2; plural=(n != 1);`.
//!
//! Este arquivo só usa a biblioteca padrão; o extrator (`ferramentas/idioma.rs`) usa o mesmo analisador.

use std::collections::HashMap;
use std::fmt;

/// Uma tradução do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Traducao {
    /// `msgid` → `msgstr`.
    Simples(String),
    /// `msgid` + `msgid_plural` → `msgstr[0..nplurals]`.
    Plural {
        /// O `msgid_plural` (o plural em português, para o extrator comparar com o `tn!`).
        plural_original: String,
        formas: Vec<String>,
    },
}

/// Um `.po` lido.
#[derive(Debug, Clone)]
pub struct Catalogo {
    /// O `Language:` do cabeçalho, se houver.
    pub idioma: Option<String>,
    /// Quantas formas de plural o idioma tem (o `nplurals`).
    pub formas_plural: usize,
    regra: Expr,
    /// O `msgid` → a tradução (só as entradas traduzidas e confirmadas).
    pub traducoes: HashMap<String, Traducao>,
    /// Os `msgid` com entrada no arquivo, mesmo sem tradução ou `fuzzy`, na ordem do arquivo (o extrator lista as que
    /// faltam traduzir).
    pub chaves: Vec<String>,
}

impl Default for Catalogo {
    fn default() -> Catalogo {
        Catalogo {
            idioma: None,
            formas_plural: 2,
            regra: Expr::regra_padrao(),
            traducoes: HashMap::new(),
            chaves: Vec::new(),
        }
    }
}

impl Catalogo {
    /// A forma de plural para `n` (o índice do `msgstr[i]`), sempre menor que `formas_plural`.
    pub fn forma(&self, n: u64) -> usize {
        let i = self.regra.avaliar(n);
        usize::try_from(i).unwrap_or(usize::MAX).min(self.formas_plural.saturating_sub(1))
    }

    /// A tradução de `msgid` sem plural; None se não há (ou se a entrada é de plural).
    pub fn simples(&self, msgid: &str) -> Option<&str> {
        match self.traducoes.get(msgid)? {
            Traducao::Simples(t) => Some(t),
            Traducao::Plural { .. } => None,
        }
    }

    /// A forma de `msgid` (o singular em português) para `n`; None se não há tradução de plural.
    pub fn plural(&self, msgid: &str, n: u64) -> Option<&str> {
        match self.traducoes.get(msgid)? {
            Traducao::Plural { formas, .. } => formas.get(self.forma(n)).map(String::as_str),
            Traducao::Simples(_) => None,
        }
    }
}

/// Um problema no `.po`, com a linha (a partir de 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroPo {
    pub linha: usize,
    pub mensagem: String,
}

impl fmt::Display for ErroPo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "linha {}: {}", self.linha, self.mensagem)
    }
}

impl std::error::Error for ErroPo {}

fn erro(linha: usize, mensagem: impl Into<String>) -> ErroPo {
    ErroPo { linha, mensagem: mensagem.into() }
}

/// O campo que as linhas de continuação (`"…"`) estendem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Campo {
    Id,
    IdPlural,
    Str(usize),
}

/// Uma entrada sendo lida.
#[derive(Debug, Default)]
struct Entrada {
    linha: usize,
    fuzzy: bool,
    id: Option<String>,
    id_plural: Option<String>,
    /// `msgstr` (índice 0) ou `msgstr[i]`.
    strs: Vec<(usize, String)>,
    /// Viu `msgstr` sem índice.
    str_simples: bool,
    campo: Option<Campo>,
}

impl Entrada {
    fn vazia(&self) -> bool {
        self.id.is_none() && self.id_plural.is_none() && self.strs.is_empty()
    }

    fn texto_do_campo(&mut self, campo: Campo) -> Option<&mut String> {
        match campo {
            Campo::Id => self.id.as_mut(),
            Campo::IdPlural => self.id_plural.as_mut(),
            Campo::Str(i) => self.strs.iter_mut().find(|(j, _)| *j == i).map(|(_, s)| s),
        }
    }
}

/// Lê um `.po` inteiro.
pub fn ler(texto: &str) -> Result<Catalogo, ErroPo> {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let mut cat = Catalogo::default();
    let mut vistas: HashMap<String, usize> = HashMap::new();
    let mut atual = Entrada::default();
    let mut cabecalho_lido = false;
    for (i, bruta) in texto.lines().enumerate() {
        let n = i + 1;
        let linha = bruta.trim();
        if linha.is_empty() {
            fechar(&mut atual, &mut cat, &mut vistas, &mut cabecalho_lido)?;
            continue;
        }
        if let Some(resto) = linha.strip_prefix('#') {
            if resto.starts_with('~') {
                // obsoleta: só comentário
                continue;
            }
            if atual.campo.is_some_and(|c| c != Campo::Id) {
                // um comentário depois do msgstr começa a próxima entrada
                fechar(&mut atual, &mut cat, &mut vistas, &mut cabecalho_lido)?;
            }
            if let Some(bandeiras) = resto.strip_prefix(',')
                && bandeiras.split(',').any(|b| b.trim() == "fuzzy")
            {
                atual.fuzzy = true;
            }
            continue;
        }
        if linha.starts_with('"') {
            let campo = atual.campo.ok_or_else(|| erro(n, "texto entre aspas fora de um msgid ou msgstr"))?;
            let parte = texto_entre_aspas(linha, n)?;
            atual.texto_do_campo(campo).ok_or_else(|| erro(n, "texto entre aspas fora de lugar"))?.push_str(&parte);
            continue;
        }
        let (palavra, resto) = linha.split_once(|c: char| c.is_whitespace()).unwrap_or((linha, ""));
        let valor = texto_entre_aspas(resto.trim(), n)?;
        match palavra {
            "msgctxt" => return Err(erro(n, "msgctxt não é usado pelo DataBox")),
            "msgid" => {
                if !atual.vazia() {
                    fechar(&mut atual, &mut cat, &mut vistas, &mut cabecalho_lido)?;
                }
                atual.linha = n;
                atual.id = Some(valor);
                atual.campo = Some(Campo::Id);
            }
            "msgid_plural" => {
                if atual.id.is_none() || atual.id_plural.is_some() || !atual.strs.is_empty() {
                    return Err(erro(n, "msgid_plural fora de lugar (vem logo depois do msgid)"));
                }
                atual.id_plural = Some(valor);
                atual.campo = Some(Campo::IdPlural);
            }
            "msgstr" => {
                if atual.id.is_none() || atual.str_simples || !atual.strs.is_empty() {
                    return Err(erro(n, "msgstr fora de lugar"));
                }
                if atual.id_plural.is_some() {
                    return Err(erro(n, "entrada com msgid_plural usa msgstr[0], msgstr[1]…"));
                }
                atual.str_simples = true;
                atual.strs.push((0, valor));
                atual.campo = Some(Campo::Str(0));
            }
            outra => {
                let indice = outra
                    .strip_prefix("msgstr[")
                    .and_then(|r| r.strip_suffix(']'))
                    .ok_or_else(|| erro(n, format!("palavra desconhecida: {outra}")))?;
                let indice: usize = indice.parse().map_err(|_| erro(n, format!("índice inválido: {outra}")))?;
                if atual.id_plural.is_none() {
                    return Err(erro(n, "msgstr[n] só vale numa entrada com msgid_plural"));
                }
                if indice != atual.strs.len() {
                    return Err(erro(n, format!("esperava msgstr[{}]", atual.strs.len())));
                }
                atual.strs.push((indice, valor));
                atual.campo = Some(Campo::Str(indice));
            }
        }
    }
    fechar(&mut atual, &mut cat, &mut vistas, &mut cabecalho_lido)?;
    // as entradas de plural precisam de todas as formas do idioma (o cabeçalho pode vir depois delas só em teoria;
    // por isso a conferência é no fim)
    for (id, t) in &cat.traducoes {
        if let Traducao::Plural { formas, .. } = t
            && formas.len() != cat.formas_plural
        {
            let linha = vistas.get(id).copied().unwrap_or(0);
            return Err(erro(
                linha,
                format!("o plural tem {} formas, e o Plural-Forms pede {}", formas.len(), cat.formas_plural),
            ));
        }
    }
    Ok(cat)
}

/// Fecha a entrada lida e a guarda no catálogo.
fn fechar(
    atual: &mut Entrada,
    cat: &mut Catalogo,
    vistas: &mut HashMap<String, usize>,
    cabecalho_lido: &mut bool,
) -> Result<(), ErroPo> {
    let e = std::mem::take(atual);
    if e.vazia() {
        return Ok(());
    }
    let Some(id) = e.id else {
        return Err(erro(e.linha, "entrada sem msgid"));
    };
    if e.strs.is_empty() {
        return Err(erro(e.linha, "entrada sem msgstr"));
    }
    if id.is_empty() {
        if *cabecalho_lido {
            return Err(erro(e.linha, "cabeçalho (msgid \"\") repetido"));
        }
        *cabecalho_lido = true;
        let texto = e.strs.into_iter().next().map(|(_, s)| s).unwrap_or_default();
        return ler_cabecalho(&texto, e.linha, cat);
    }
    if let Some(&antes) = vistas.get(&id) {
        return Err(erro(e.linha, format!("msgid repetido (já na linha {antes}): {id:?}")));
    }
    vistas.insert(id.clone(), e.linha);
    cat.chaves.push(id.clone());
    if e.fuzzy {
        return Ok(());
    }
    let traducao = match e.id_plural {
        None => {
            let s = e.strs.into_iter().next().map(|(_, s)| s).unwrap_or_default();
            if s.is_empty() {
                return Ok(());
            }
            Traducao::Simples(s)
        }
        Some(plural_original) => {
            let formas: Vec<String> = e.strs.into_iter().map(|(_, s)| s).collect();
            if formas.iter().any(String::is_empty) {
                return Ok(());
            }
            Traducao::Plural { plural_original, formas }
        }
    };
    cat.traducoes.insert(id, traducao);
    Ok(())
}

/// O cabeçalho: `Language:` e `Plural-Forms:`; o resto (autores, datas, `Content-Type`) não importa.
fn ler_cabecalho(texto: &str, linha: usize, cat: &mut Catalogo) -> Result<(), ErroPo> {
    for item in texto.lines() {
        let Some((chave, valor)) = item.split_once(':') else { continue };
        let valor = valor.trim();
        match chave.trim() {
            "Language" if !valor.is_empty() => cat.idioma = Some(valor.to_string()),
            "Plural-Forms" => {
                let (n, regra) = ler_plural_forms(valor).map_err(|m| erro(linha, format!("Plural-Forms: {m}")))?;
                cat.formas_plural = n;
                cat.regra = regra;
            }
            _ => {}
        }
    }
    Ok(())
}

/// `nplurals=2; plural=(n != 1);` → (2, a expressão).
fn ler_plural_forms(valor: &str) -> Result<(usize, Expr), String> {
    let mut formas = None;
    let mut regra = None;
    for parte in valor.split(';') {
        let Some((chave, v)) = parte.split_once('=') else { continue };
        match chave.trim() {
            "nplurals" => {
                let n: usize = v.trim().parse().map_err(|_| format!("nplurals inválido: {}", v.trim()))?;
                if n == 0 {
                    return Err("nplurals precisa ser pelo menos 1".into());
                }
                formas = Some(n);
            }
            "plural" => regra = Some(Expr::ler(v.trim())?),
            _ => {}
        }
    }
    Ok((formas.ok_or("falta o nplurals")?, regra.ok_or("falta o plural")?))
}

/// O texto entre aspas de uma linha (`"…"`), com os escapes resolvidos.
fn texto_entre_aspas(trecho: &str, linha: usize) -> Result<String, ErroPo> {
    let dentro = trecho
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .filter(|_| trecho.len() >= 2)
        .ok_or_else(|| erro(linha, format!("esperava um texto entre aspas: {trecho}")))?;
    let mut saida = String::with_capacity(dentro.len());
    let mut chars = dentro.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') => saida.push('\n'),
                Some('t') => saida.push('\t'),
                Some('r') => saida.push('\r'),
                Some('"') => saida.push('"'),
                Some('\\') => saida.push('\\'),
                Some(outro) => return Err(erro(linha, format!("escape desconhecido: \\{outro}"))),
                None => return Err(erro(linha, "barra invertida no fim do texto")),
            },
            '"' => return Err(erro(linha, "aspas sem a barra invertida dentro do texto")),
            c => saida.push(c),
        }
    }
    Ok(saida)
}

/// O texto escrito como o `.po` pede (aspas e escapes), para o extrator gravar entradas novas.
pub fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len() + 2);
    s.push('"');
    for c in texto.chars() {
        match c {
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            '\r' => s.push_str("\\r"),
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

// ---------------------------------------------------------------------- a expressão do Plural-Forms

/// A expressão do `plural=` (a sintaxe do C, com inteiros sem sinal como o gettext).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expr {
    N,
    Num(u64),
    Nao(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Se(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Ou,
    E,
    Igual,
    Diferente,
    Menor,
    MenorIgual,
    Maior,
    MaiorIgual,
    Mais,
    Menos,
    Vezes,
    Dividido,
    Resto,
}

impl Op {
    /// A precedência do C (maior = liga mais forte).
    fn precedencia(self) -> u8 {
        match self {
            Op::Ou => 1,
            Op::E => 2,
            Op::Igual | Op::Diferente => 3,
            Op::Menor | Op::MenorIgual | Op::Maior | Op::MaiorIgual => 4,
            Op::Mais | Op::Menos => 5,
            Op::Vezes | Op::Dividido | Op::Resto => 6,
        }
    }

    fn aplicar(self, a: u64, b: u64) -> u64 {
        let v = |x: bool| u64::from(x);
        match self {
            Op::Ou => v(a != 0 || b != 0),
            Op::E => v(a != 0 && b != 0),
            Op::Igual => v(a == b),
            Op::Diferente => v(a != b),
            Op::Menor => v(a < b),
            Op::MenorIgual => v(a <= b),
            Op::Maior => v(a > b),
            Op::MaiorIgual => v(a >= b),
            Op::Mais => a.wrapping_add(b),
            Op::Menos => a.wrapping_sub(b),
            Op::Vezes => a.wrapping_mul(b),
            Op::Dividido => a.checked_div(b).unwrap_or(0),
            Op::Resto => a.checked_rem(b).unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Simbolo {
    N,
    Num(u64),
    Op(Op),
    Nao,
    Pergunta,
    DoisPontos,
    Abre,
    Fecha,
}

impl Expr {
    /// `n != 1`: o inglês e o português.
    fn regra_padrao() -> Expr {
        Expr::Bin(Op::Diferente, Box::new(Expr::N), Box::new(Expr::Num(1)))
    }

    fn ler(texto: &str) -> Result<Expr, String> {
        let simbolos = simbolos(texto)?;
        let mut p = Leitor { simbolos: &simbolos, pos: 0 };
        let e = p.condicional()?;
        if p.pos != simbolos.len() {
            return Err(format!("sobrou texto na expressão: {texto}"));
        }
        Ok(e)
    }

    fn avaliar(&self, n: u64) -> u64 {
        match self {
            Expr::N => n,
            Expr::Num(v) => *v,
            Expr::Nao(e) => u64::from(e.avaliar(n) == 0),
            Expr::Bin(op, a, b) => op.aplicar(a.avaliar(n), b.avaliar(n)),
            Expr::Se(c, a, b) => {
                if c.avaliar(n) != 0 {
                    a.avaliar(n)
                } else {
                    b.avaliar(n)
                }
            }
        }
    }
}

fn simbolos(texto: &str) -> Result<Vec<Simbolo>, String> {
    let b = texto.as_bytes();
    let mut saida = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let par = b.get(i + 1).copied();
        let (s, passo) = match c {
            b' ' | b'\t' => {
                i += 1;
                continue;
            }
            b'n' => (Simbolo::N, 1),
            b'0'..=b'9' => {
                let fim = b[i..].iter().position(|d| !d.is_ascii_digit()).map_or(b.len(), |p| i + p);
                let v: u64 = texto[i..fim].parse().map_err(|_| format!("número grande demais: {}", &texto[i..fim]))?;
                (Simbolo::Num(v), fim - i)
            }
            b'|' if par == Some(b'|') => (Simbolo::Op(Op::Ou), 2),
            b'&' if par == Some(b'&') => (Simbolo::Op(Op::E), 2),
            b'=' if par == Some(b'=') => (Simbolo::Op(Op::Igual), 2),
            b'!' if par == Some(b'=') => (Simbolo::Op(Op::Diferente), 2),
            b'<' if par == Some(b'=') => (Simbolo::Op(Op::MenorIgual), 2),
            b'>' if par == Some(b'=') => (Simbolo::Op(Op::MaiorIgual), 2),
            b'!' => (Simbolo::Nao, 1),
            b'<' => (Simbolo::Op(Op::Menor), 1),
            b'>' => (Simbolo::Op(Op::Maior), 1),
            b'+' => (Simbolo::Op(Op::Mais), 1),
            b'-' => (Simbolo::Op(Op::Menos), 1),
            b'*' => (Simbolo::Op(Op::Vezes), 1),
            b'/' => (Simbolo::Op(Op::Dividido), 1),
            b'%' => (Simbolo::Op(Op::Resto), 1),
            b'?' => (Simbolo::Pergunta, 1),
            b':' => (Simbolo::DoisPontos, 1),
            b'(' => (Simbolo::Abre, 1),
            b')' => (Simbolo::Fecha, 1),
            _ => return Err(format!("símbolo inesperado na expressão: {}", &texto[i..])),
        };
        saida.push(s);
        i += passo;
    }
    Ok(saida)
}

/// Descida recursiva com precedência (o `?:` por fora, associativo à direita).
struct Leitor<'a> {
    simbolos: &'a [Simbolo],
    pos: usize,
}

impl Leitor<'_> {
    fn proximo(&self) -> Option<Simbolo> {
        self.simbolos.get(self.pos).copied()
    }

    fn condicional(&mut self) -> Result<Expr, String> {
        let c = self.binaria(1)?;
        if self.proximo() != Some(Simbolo::Pergunta) {
            return Ok(c);
        }
        self.pos += 1;
        let a = self.condicional()?;
        if self.proximo() != Some(Simbolo::DoisPontos) {
            return Err("faltou o ':' do '?'".into());
        }
        self.pos += 1;
        let b = self.condicional()?;
        Ok(Expr::Se(Box::new(c), Box::new(a), Box::new(b)))
    }

    fn binaria(&mut self, minima: u8) -> Result<Expr, String> {
        let mut esq = self.unaria()?;
        while let Some(Simbolo::Op(op)) = self.proximo() {
            if op.precedencia() < minima {
                break;
            }
            self.pos += 1;
            let dir = self.binaria(op.precedencia() + 1)?;
            esq = Expr::Bin(op, Box::new(esq), Box::new(dir));
        }
        Ok(esq)
    }

    fn unaria(&mut self) -> Result<Expr, String> {
        let s = self.proximo().ok_or("a expressão terminou antes da hora")?;
        self.pos += 1;
        match s {
            Simbolo::N => Ok(Expr::N),
            Simbolo::Num(v) => Ok(Expr::Num(v)),
            Simbolo::Nao => Ok(Expr::Nao(Box::new(self.unaria()?))),
            Simbolo::Abre => {
                let e = self.condicional()?;
                if self.proximo() != Some(Simbolo::Fecha) {
                    return Err("faltou fechar o parêntese".into());
                }
                self.pos += 1;
                Ok(e)
            }
            outro => Err(format!("símbolo fora de lugar na expressão: {outro:?}")),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const CABECALHO_EN: &str =
        "msgid \"\"\nmsgstr \"\"\n\"Language: en\\n\"\n\"Plural-Forms: nplurals=2; plural=(n != 1);\\n\"\n";

    #[test]
    fn le_simples_plural_e_cabecalho() {
        let po = format!(
            "{CABECALHO_EN}\n# comentário\n#: crates/x/src/a.rs:10\nmsgid \"Abrir pasta\"\nmsgstr \"Open folder\"\n\n\
             msgid \"{{n}} linha\"\nmsgid_plural \"{{n}} linhas\"\nmsgstr[0] \"{{n}} row\"\nmsgstr[1] \"{{n}} rows\"\n"
        );
        let cat = ler(&po).unwrap();
        assert_eq!(cat.idioma.as_deref(), Some("en"));
        assert_eq!(cat.formas_plural, 2);
        assert_eq!(cat.simples("Abrir pasta"), Some("Open folder"));
        assert_eq!(cat.plural("{n} linha", 1), Some("{n} row"));
        assert_eq!(cat.plural("{n} linha", 0), Some("{n} rows"));
        assert_eq!(cat.plural("{n} linha", 2), Some("{n} rows"));
        assert_eq!(cat.simples("{n} linha"), None, "a de plural não responde como simples");
        assert_eq!(cat.plural("Abrir pasta", 2), None);
        assert_eq!(cat.chaves, ["Abrir pasta", "{n} linha"]);
    }

    #[test]
    fn continuacao_e_escapes() {
        let po = "msgid \"\"\n\"Linha um\\n\"\n\"com \\\"aspas\\\" e \\\\ e\\ttab\"\nmsgstr \"ok\"\n";
        let cat = ler(po).unwrap();
        assert_eq!(cat.simples("Linha um\ncom \"aspas\" e \\ e\ttab"), Some("ok"));
        assert!(ler("msgid \"a\\q\"\nmsgstr \"b\"\n").unwrap_err().mensagem.contains("escape desconhecido"));
        assert!(ler("msgid \"a\" b\"\nmsgstr \"b\"\n").is_err());
        assert_eq!(escapar("a\n\"b\"\\\t"), "\"a\\n\\\"b\\\"\\\\\\t\"");
    }

    #[test]
    fn vazio_fuzzy_e_obsoleta_ficam_de_fora() {
        let po = "msgid \"Sem tradução\"\nmsgstr \"\"\n\n#, fuzzy\nmsgid \"Incerta\"\nmsgstr \"Unsure\"\n\n\
                  #~ msgid \"Velha\"\n#~ msgstr \"Old\"\n\nmsgid \"x {n}\"\nmsgid_plural \"x {n}s\"\nmsgstr[0] \"a\"\nmsgstr[1] \"\"\n";
        let cat = ler(po).unwrap();
        assert!(cat.traducoes.is_empty(), "{:?}", cat.traducoes);
        assert_eq!(cat.chaves, ["Sem tradução", "Incerta", "x {n}"]);
        let vazio = ler("").unwrap();
        assert!(vazio.traducoes.is_empty() && vazio.idioma.is_none());
        assert_eq!(ler("\u{feff}# só comentário\n").unwrap().formas_plural, 2);
    }

    #[test]
    fn msgid_repetido_e_erro() {
        let e = ler("msgid \"a\"\nmsgstr \"b\"\n\nmsgid \"a\"\nmsgstr \"c\"\n").unwrap_err();
        assert_eq!(e.linha, 4);
        assert!(e.mensagem.contains("repetido") && e.mensagem.contains("linha 1"), "{e}");
    }

    #[test]
    fn estrutura_errada_e_erro() {
        for (po, trecho) in [
            ("msgstr \"b\"\n", "fora de lugar"),
            ("msgid \"a\"\n", "sem msgstr"),
            ("msgid \"a\"\nmsgstr[0] \"b\"\n", "msgid_plural"),
            ("msgid \"a\"\nmsgid_plural \"as\"\nmsgstr \"b\"\n", "msgstr[0]"),
            ("msgid \"a\"\nmsgid_plural \"as\"\nmsgstr[1] \"b\"\n", "esperava msgstr[0]"),
            ("msgid \"a\"\nmsgid_plural \"as\"\nmsgstr[0] \"b\"\n", "2"),
            ("msgctxt \"menu\"\nmsgid \"a\"\nmsgstr \"b\"\n", "msgctxt"),
            ("\"solto\"\n", "fora de um msgid"),
            ("msgsrt \"b\"\n", "desconhecida"),
        ] {
            let e = ler(po).expect_err(po);
            assert!(e.mensagem.contains(trecho), "{po:?} → {e}");
        }
    }

    #[test]
    fn plural_forms_de_varios_idiomas() {
        let regra = |s: &str| ler_plural_forms(s).unwrap();
        let (n, pt) = regra("nplurals=2; plural=(n != 1);");
        assert_eq!(n, 2);
        assert_eq!([0, 1, 2, 1000].map(|v| pt.avaliar(v)), [1, 0, 1, 1]);
        // o pt_BR do gettext (0 no singular), o francês
        let (_, fr) = regra("nplurals=2; plural=(n > 1);");
        assert_eq!([0, 1, 2].map(|v| fr.avaliar(v)), [0, 0, 1]);
        // o russo: três formas, com ?: aninhado, %, && e ||
        let (n, ru) = regra(
            "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        );
        assert_eq!(n, 3);
        assert_eq!([1, 2, 5, 11, 21, 22, 25, 111, 112].map(|v| ru.avaliar(v)), [0, 1, 2, 2, 0, 1, 2, 2, 2]);
        // o japonês: uma forma só
        let (n, ja) = regra("nplurals=1; plural=0;");
        assert_eq!((n, ja.avaliar(7)), (1, 0));
        // ! e divisão por zero não caem
        assert_eq!(regra("nplurals=2; plural=!(n/0);").1.avaliar(3), 1);
        for ruim in [
            "nplurals=2;",
            "plural=n;",
            "nplurals=0; plural=0;",
            "nplurals=2; plural=(n;",
            "nplurals=2; plural=n ?? 1;",
        ] {
            assert!(ler_plural_forms(ruim).is_err(), "{ruim}");
        }
    }

    #[test]
    fn forma_fora_da_faixa_fica_na_ultima() {
        let po = "msgid \"\"\nmsgstr \"Plural-Forms: nplurals=2; plural=n;\\n\"\n\n\
                  msgid \"a\"\nmsgid_plural \"as\"\nmsgstr[0] \"um\"\nmsgstr[1] \"muitos\"\n";
        let cat = ler(po).unwrap();
        assert_eq!(cat.plural("a", 0), Some("um"));
        assert_eq!(cat.plural("a", 9), Some("muitos"));
        let ruim = "msgid \"\"\nmsgstr \"Plural-Forms: nplurals=x; plural=n;\\n\"\n";
        assert!(ler(ruim).unwrap_err().mensagem.starts_with("Plural-Forms"));
        assert!(ler(&format!("{CABECALHO_EN}\n{CABECALHO_EN}")).unwrap_err().mensagem.contains("repetido"));
    }
}
