//! O extrator dos textos: confere o código com os catálogos `.po` de `crates/idioma/idiomas/`.
//!
//! ```text
//! cargo run -p idioma --bin extrator -- [--so-avisar] [--detalhes] [--pot] [--raiz <pasta>]
//! ```
//!
//! (no Windows, `./cargo.ps1 run -p idioma --bin extrator -- …`)
//!
//! 1. Lista as chaves do `t!`, do `tn!` e do `t_estatico!` nos `src/` dos crates (fora o próprio `idioma` e o código
//!    de teste: `testes.rs`, as pastas `testes/` e `tests/`, os itens com `#[cfg(test)]`).
//! 2. Compara com cada `.po`. No `en.po`, **falha** com a chave que falta (ou está sem tradução, ou `fuzzy`), com o
//!    plural que não bate (`tn!` no código e entrada simples no `.po`, ou o contrário) e com os `{nome}` da tradução
//!    que a chave não tem; nos outros idiomas, só avisa (o texto cai no inglês). As entradas do `.po` que o código não
//!    usa mais só contam no resumo (`--detalhes` lista).
//! 3. Lista os literais soltos dos crates de tela (`interface`, `estado`, `tela_*`, `editor`): texto entre aspas com
//!    espaço e acento fora das três macros, das mensagens de registro e de pânico (`log::…`, `panic!`, `assert!`…) e
//!    dos atributos. Cada um falha, menos os da lista de exceções (`ferramentas/idioma-excecoes.txt`). Com
//!    `--so-avisar`, os literais soltos só avisam (a varredura das telas, a I3, ainda não aconteceu); o que falta no
//!    `en.po` continua falhando.
//!
//! `--pot` escreve na saída todas as chaves do código no formato `.po` (com `#: arquivo:linha`), para começar um
//! idioma novo ou atualizar um catálogo no Poedit. Sai com 1 quando há falha, 0 quando não.
//!
//! Só usa a biblioteca padrão e o analisador do `.po` do próprio crate `idioma` (`idioma::po`); é um binário do crate
//! (`crates/idioma/Cargo.toml`), então o `cargo test -p idioma` roda os testes daqui também.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use idioma::po::{self, Catalogo, Traducao};

/// Os crates de tela: onde todo texto para a pessoa já devia passar pelo `t!`.
fn e_crate_de_tela(nome: &str) -> bool {
    matches!(nome, "interface" | "estado" | "editor") || nome.starts_with("tela_")
}

/// As macros cujos textos não são para a pessoa ler na tela (o registro, os pânicos e as conferências).
const MACROS_DE_FORA: &[&str] = &[
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "log",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "unreachable",
    "todo",
    "unimplemented",
    "println",
    "eprintln",
    "print",
    "eprint",
    "include_str",
    "include_bytes",
    "env",
    "concat",
];

const ACENTOS: &str = "áàâãäéèêëíìîïóòôõöúùûüçñÁÀÂÃÄÉÈÊËÍÌÎÏÓÒÔÕÖÚÙÛÜÇÑ";

// ---------------------------------------------------------------------- o léxico do Rust (o pedaço que importa)

#[derive(Debug, Clone, PartialEq)]
enum Ficha {
    Ident(String),
    Pontuacao(char),
    /// Um texto (`"…"` ou `r#"…"#`) já sem os escapes, com a linha.
    Texto(String, usize),
    /// Número, caractere, rótulo, texto de bytes: nada que interesse.
    Outro,
}

/// As fichas de um arquivo, sem comentários.
fn fichas(fonte: &str) -> Vec<Ficha> {
    let c: Vec<char> = fonte.chars().collect();
    let mut saida = Vec::new();
    let mut i = 0;
    let mut linha = 1;
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            linha += 1;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        // comentários
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut fundo = 1;
            i += 2;
            while i < c.len() && fundo > 0 {
                if c[i] == '\n' {
                    linha += 1;
                }
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    fundo += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    fundo -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // textos crus: r"…", r#"…"#, br"…", cr"…"
        let prefixo_cru = match (ch, c.get(i + 1), c.get(i + 2)) {
            ('r', Some('"' | '#'), _) => Some((1, true)),
            ('b' | 'c', Some('r'), Some('"' | '#')) => Some((2, false)),
            _ => None,
        };
        if let Some((pulo, e_texto)) = prefixo_cru {
            let mut j = i + pulo;
            let mut cerquilhas = 0;
            while c.get(j) == Some(&'#') {
                cerquilhas += 1;
                j += 1;
            }
            if c.get(j) == Some(&'"') {
                let inicio = linha;
                j += 1;
                let mut valor = String::new();
                while let Some(&x) = c.get(j) {
                    if x == '"' && (1..=cerquilhas).all(|k| c.get(j + k) == Some(&'#')) {
                        j += 1 + cerquilhas;
                        break;
                    }
                    if x == '\n' {
                        linha += 1;
                    }
                    valor.push(x);
                    j += 1;
                }
                saida.push(if e_texto { Ficha::Texto(valor, inicio) } else { Ficha::Outro });
                i = j;
                continue;
            }
        }
        // textos: "…", b"…", c"…"
        let (e_aspas, pulo, e_texto) = match (ch, c.get(i + 1)) {
            ('"', _) => (true, 0, true),
            ('b' | 'c', Some('"')) => (true, 1, false),
            _ => (false, 0, false),
        };
        if e_aspas {
            let inicio = linha;
            let mut j = i + pulo + 1;
            let mut valor = String::new();
            while let Some(&x) = c.get(j) {
                match x {
                    '"' => {
                        j += 1;
                        break;
                    }
                    '\\' => {
                        j += 1;
                        let Some(&e) = c.get(j) else { break };
                        j += 1;
                        match e {
                            'n' => valor.push('\n'),
                            't' => valor.push('\t'),
                            'r' => valor.push('\r'),
                            '0' => valor.push('\0'),
                            '\\' | '"' | '\'' => valor.push(e),
                            'x' => {
                                let hex: String = c[j..(j + 2).min(c.len())].iter().collect();
                                j += 2;
                                if let Some(v) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                    valor.push(v);
                                }
                            }
                            'u' => {
                                // \u{…}
                                let fim = c[j..].iter().position(|&x| x == '}').map_or(c.len(), |p| j + p);
                                let hex: String = c[(j + 1).min(fim)..fim].iter().collect();
                                j = fim + 1;
                                if let Some(v) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                    valor.push(v);
                                }
                            }
                            '\n' => {
                                // a continuação: some a quebra e o espaço do começo da próxima linha
                                linha += 1;
                                while c.get(j).is_some_and(|x| x.is_whitespace()) {
                                    if c[j] == '\n' {
                                        linha += 1;
                                    }
                                    j += 1;
                                }
                            }
                            outro => valor.push(outro),
                        }
                    }
                    '\n' => {
                        linha += 1;
                        valor.push('\n');
                        j += 1;
                    }
                    x => {
                        valor.push(x);
                        j += 1;
                    }
                }
            }
            saida.push(if e_texto { Ficha::Texto(valor, inicio) } else { Ficha::Outro });
            i = j;
            continue;
        }
        // caractere ('a', '\n', '\u{e9}') ou rótulo ('a)
        if ch == '\'' {
            if c.get(i + 1) == Some(&'\\') {
                // o caractere escapado pode ser a própria aspa ('\'')
                let mut j = i + 3;
                while j < c.len() && c[j] != '\'' {
                    j += 1;
                }
                i = j + 1;
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                    i += 1;
                }
            }
            saida.push(Ficha::Outro);
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let inicio = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            saida.push(Ficha::Ident(c[inicio..i].iter().collect()));
            continue;
        }
        if ch.is_ascii_digit() {
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '.') {
                // "1..2": o ponto duplo não é do número
                if c[i] == '.' && c.get(i + 1) == Some(&'.') {
                    break;
                }
                i += 1;
            }
            saida.push(Ficha::Outro);
            continue;
        }
        saida.push(Ficha::Pontuacao(ch));
        i += 1;
    }
    saida
}

// ---------------------------------------------------------------------- o que se tira das fichas

/// Uma chave achada no código.
#[derive(Debug, Clone)]
struct Chave {
    /// None no `t!`; o plural em português no `tn!`.
    plural: Option<String>,
    /// Onde aparece ("crates/x/src/a.rs:12").
    lugares: Vec<String>,
}

#[derive(Debug, Default)]
struct Achados {
    chaves: BTreeMap<String, Chave>,
    /// (lugar, texto) dos literais soltos dos crates de tela.
    soltos: Vec<(String, String)>,
    /// Problemas de uso das macros (uma chave que não é literal, um tn! com forma estranha).
    problemas: Vec<String>,
}

fn e_pontuacao(f: Option<&Ficha>, c: char) -> bool {
    matches!(f, Some(Ficha::Pontuacao(x)) if *x == c)
}

fn par_de(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

/// O índice logo depois do grupo que abre em `i` (`(`, `[` ou `{`, com o fecho correspondente).
fn depois_do_grupo(f: &[Ficha], i: usize) -> usize {
    let mut fundo = 0usize;
    let mut j = i;
    while j < f.len() {
        match &f[j] {
            Ficha::Pontuacao('(' | '[' | '{') => fundo += 1,
            Ficha::Pontuacao(')' | ']' | '}') => {
                fundo = fundo.saturating_sub(1);
                if fundo == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    f.len()
}

/// Um atributo `#[cfg(test)]` (ou `#[cfg(any(test, …))]`, sem `not`) entre `i` e `fim`.
fn e_cfg_de_teste(f: &[Ficha]) -> bool {
    let tem = |n: &str| f.iter().any(|x| matches!(x, Ficha::Ident(s) if s == n));
    tem("cfg") && tem("test") && !tem("not")
}

/// Pula o item depois de um atributo: até o `;` de fora dos grupos ou até o fim do primeiro `{…}`.
fn depois_do_item(f: &[Ficha], mut i: usize) -> usize {
    // outros atributos do mesmo item
    while e_pontuacao(f.get(i), '#') && e_pontuacao(f.get(i + 1), '[') {
        i = depois_do_grupo(f, i + 1);
    }
    while i < f.len() {
        match &f[i] {
            Ficha::Pontuacao(';') => return i + 1,
            Ficha::Pontuacao('{') => return depois_do_grupo(f, i),
            Ficha::Pontuacao('(' | '[') => i = depois_do_grupo(f, i),
            _ => i += 1,
        }
    }
    f.len()
}

fn lugar(arquivo: &str, linha: usize) -> String {
    format!("{arquivo}:{linha}")
}

fn texto_solto(s: &str) -> bool {
    s.contains(' ') && s.chars().any(|c| ACENTOS.contains(c))
}

fn ler_arquivo(f: &[Ficha], arquivo: &str, de_tela: bool, achados: &mut Achados) {
    let mut i = 0;
    while i < f.len() {
        match &f[i] {
            // atributos: #[…] e #![…]; o item de um #[cfg(test)] fica de fora inteiro
            Ficha::Pontuacao('#') => {
                let abre = if e_pontuacao(f.get(i + 1), '!') { i + 2 } else { i + 1 };
                if e_pontuacao(f.get(abre), '[') {
                    let fim = depois_do_grupo(f, abre);
                    i = if abre == i + 1 && e_cfg_de_teste(&f[abre..fim]) { depois_do_item(f, fim) } else { fim };
                    continue;
                }
                i += 1;
            }
            Ficha::Ident(nome) if e_pontuacao(f.get(i + 1), '!') && f.get(i + 2).and_then(abre_grupo).is_some() => {
                let abre = i + 2;
                let fim = depois_do_grupo(f, abre);
                match nome.as_str() {
                    "t" | "t_estatico" => {
                        match f.get(abre + 1) {
                            Some(Ficha::Texto(s, l)) => anotar(achados, s, None, &lugar(arquivo, *l)),
                            _ => achados.problemas.push(format!(
                                "{}: {nome}! sem um texto entre aspas como chave",
                                lugar(arquivo, linha_perto(f, i))
                            )),
                        }
                        // os argumentos podem ter outros t! dentro
                        if let Some(virgula) = (abre + 2..fim).find(|&k| e_pontuacao(f.get(k), ',')) {
                            ler_arquivo(&f[virgula..fim.saturating_sub(1)], arquivo, de_tela, achados);
                        }
                        i = fim;
                    }
                    "tn" => {
                        // a contagem vai até a primeira vírgula de fora dos grupos
                        let mut k = abre + 1;
                        while k < fim && !e_pontuacao(f.get(k), ',') {
                            k = if f.get(k).and_then(abre_grupo).is_some() { depois_do_grupo(f, k) } else { k + 1 };
                        }
                        match (f.get(k + 1), f.get(k + 2), f.get(k + 3)) {
                            (Some(Ficha::Texto(um, l)), Some(Ficha::Pontuacao(',')), Some(Ficha::Texto(varios, _))) => {
                                anotar(achados, um, Some(varios), &lugar(arquivo, *l));
                            }
                            _ => achados.problemas.push(format!(
                                "{}: tn! sem os dois textos entre aspas depois da contagem",
                                lugar(arquivo, linha_perto(f, i))
                            )),
                        }
                        ler_arquivo(&f[abre + 1..k.min(fim)], arquivo, de_tela, achados);
                        if let Some(virgula) = (k + 4..fim).find(|&x| e_pontuacao(f.get(x), ',')) {
                            ler_arquivo(&f[virgula..fim.saturating_sub(1)], arquivo, de_tela, achados);
                        }
                        i = fim;
                    }
                    n if MACROS_DE_FORA.contains(&n) => i = fim,
                    _ => i += 2,
                }
            }
            Ficha::Texto(s, l) => {
                if de_tela && texto_solto(s) {
                    achados.soltos.push((lugar(arquivo, *l), s.clone()));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
}

fn abre_grupo(f: &Ficha) -> Option<char> {
    match f {
        Ficha::Pontuacao(c) => par_de(*c),
        _ => None,
    }
}

/// A linha do texto mais perto de `i` (para as mensagens de problema).
fn linha_perto(f: &[Ficha], i: usize) -> usize {
    f[i..]
        .iter()
        .chain(f[..i].iter().rev())
        .find_map(|x| if let Ficha::Texto(_, l) = x { Some(*l) } else { None })
        .unwrap_or(0)
}

fn anotar(achados: &mut Achados, chave: &str, plural: Option<&String>, onde: &str) {
    let entrada = achados
        .chaves
        .entry(chave.to_string())
        .or_insert_with(|| Chave { plural: plural.cloned(), lugares: Vec::new() });
    if entrada.plural.as_ref() != plural {
        achados.problemas.push(format!(
            "{onde}: a chave {chave:?} aparece com plural diferente em {}",
            entrada.lugares.first().map_or("outro lugar", String::as_str)
        ));
    }
    entrada.lugares.push(onde.to_string());
}

// ---------------------------------------------------------------------- os arquivos

/// Os `.rs` de `pasta`, fora o código de teste, em ordem.
fn arquivos_rs(pasta: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(lista) = fs::read_dir(pasta) else { return };
    let mut caminhos: Vec<PathBuf> = lista.filter_map(|e| e.ok().map(|e| e.path())).collect();
    caminhos.sort();
    for p in caminhos {
        let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if !matches!(nome, "testes" | "tests" | "apoio_testes") {
                arquivos_rs(&p, saida);
            }
        } else if p.extension().is_some_and(|e| e == "rs") && !matches!(nome, "testes.rs" | "apoio_testes.rs") {
            saida.push(p);
        }
    }
}

fn relativo(raiz: &Path, p: &Path) -> String {
    p.strip_prefix(raiz).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// As exceções dos literais soltos: "arquivo" (o arquivo inteiro) ou "arquivo: texto" (aquele literal); `*` no
/// lugar do arquivo vale para todos.
fn ler_excecoes(texto: &str) -> Vec<(String, Option<String>)> {
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| match l.split_once(": ") {
            Some((a, t)) => (a.trim().to_string(), Some(t.to_string())),
            None => (l.to_string(), None),
        })
        .collect()
}

fn excecao(excecoes: &[(String, Option<String>)], onde: &str, texto: &str) -> bool {
    let arquivo = onde.rsplit_once(':').map_or(onde, |(a, _)| a);
    excecoes
        .iter()
        .any(|(a, t)| (a == "*" || a == arquivo) && t.as_deref().is_none_or(|t| t == texto.replace('\n', "\\n")))
}

// ---------------------------------------------------------------------- a conferência com o .po

/// Os problemas de uma chave num catálogo (vazio = tudo certo).
fn conferir(chave: &str, info: &Chave, cat: &Catalogo) -> Option<String> {
    let permitidos: BTreeSet<&str> = match &info.plural {
        None => idioma::nomes(chave).into_iter().collect(),
        Some(p) => idioma::nomes(chave).into_iter().chain(idioma::nomes(p)).chain(["n"]).collect(),
    };
    match (cat.traducoes.get(chave), &info.plural) {
        (None, _) if cat.chaves.iter().any(|c| c == chave) => Some("sem tradução (vazia ou fuzzy)".into()),
        (None, _) => Some("falta".into()),
        (Some(Traducao::Simples(_)), Some(_)) => Some("o código usa tn! e o .po tem entrada sem plural".into()),
        (Some(Traducao::Plural { .. }), None) => Some("o código usa t! e o .po tem entrada de plural".into()),
        (Some(Traducao::Simples(t)), None) => {
            let usados: BTreeSet<&str> = idioma::nomes(t).into_iter().collect();
            (usados != permitidos)
                .then(|| format!("os {{nomes}} da tradução ({usados:?}) não são os da chave ({permitidos:?})"))
        }
        (Some(Traducao::Plural { plural_original, formas }), Some(p)) => {
            if plural_original != p {
                return Some(format!("o msgid_plural é {plural_original:?}, e o código diz {p:?}"));
            }
            formas
                .iter()
                .flat_map(|f| idioma::nomes(f))
                .find(|n| !permitidos.contains(n))
                .map(|n| format!("a tradução usa {{{n}}}, que a chave não tem"))
        }
    }
}

struct Opcoes {
    so_avisar: bool,
    detalhes: bool,
    pot: bool,
    raiz: PathBuf,
}

fn ler_opcoes() -> Result<Opcoes, String> {
    let mut o = Opcoes {
        so_avisar: false,
        detalhes: false,
        pot: false,
        raiz: Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--so-avisar" => o.so_avisar = true,
            "--detalhes" => o.detalhes = true,
            "--pot" => o.pot = true,
            "--raiz" => o.raiz = PathBuf::from(args.next().ok_or("--raiz precisa de uma pasta")?),
            "-h" | "--help" | "--ajuda" => {
                return Err("uso: extrator [--so-avisar] [--detalhes] [--pot] [--raiz <pasta>]".into());
            }
            outra => return Err(format!("opção desconhecida: {outra}")),
        }
    }
    Ok(o)
}

/// Mostra até `limite` linhas de uma lista (todas com `--detalhes`).
fn listar(linhas: &[String], detalhes: bool, limite: usize) {
    let n = if detalhes { linhas.len() } else { linhas.len().min(limite) };
    for l in &linhas[..n] {
        println!("  {l}");
    }
    if n < linhas.len() {
        println!("  … e mais {} (--detalhes mostra todos)", linhas.len() - n);
    }
}

fn main() -> ExitCode {
    let o = match ler_opcoes() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let raiz = o.raiz.canonicalize().unwrap_or(o.raiz.clone());
    let mut achados = Achados::default();
    let mut crates: Vec<PathBuf> = fs::read_dir(raiz.join("crates"))
        .map(|l| l.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    crates.sort();
    for pasta in crates {
        let nome = pasta.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if nome == "idioma" {
            // os exemplos e os testes do próprio crate não são textos do app
            continue;
        }
        let mut arquivos = Vec::new();
        arquivos_rs(&pasta.join("src"), &mut arquivos);
        for a in arquivos {
            let Ok(fonte) = fs::read_to_string(&a) else { continue };
            ler_arquivo(&fichas(&fonte), &relativo(&raiz, &a), e_crate_de_tela(&nome), &mut achados);
        }
    }

    if o.pot {
        println!(
            "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\"Plural-Forms: nplurals=2; plural=(n != 1);\\n\""
        );
        for (chave, info) in &achados.chaves {
            println!();
            for l in &info.lugares {
                println!("#: {l}");
            }
            println!("msgid {}", po::escapar(chave));
            match &info.plural {
                None => println!("msgstr \"\""),
                Some(p) => println!("msgid_plural {}\nmsgstr[0] \"\"\nmsgstr[1] \"\"", po::escapar(p)),
            }
        }
        return ExitCode::SUCCESS;
    }

    let mut falhou = false;
    println!("Textos do código: {} chaves no t!/tn!/t_estatico!.", achados.chaves.len());
    if !achados.problemas.is_empty() {
        falhou = true;
        println!("ERRO: {} usos das macros que o extrator não entende:", achados.problemas.len());
        listar(&achados.problemas, true, 0);
    }

    // os catálogos
    let pasta_po = raiz.join("crates/idioma/idiomas");
    let mut pos: Vec<PathBuf> = fs::read_dir(&pasta_po)
        .map(|l| {
            l.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "po")).collect()
        })
        .unwrap_or_default();
    pos.sort();
    if !pos.iter().any(|p| p.file_stem().is_some_and(|s| s == "en")) {
        println!("ERRO: não achei o {}", relativo(&raiz, &pasta_po.join("en.po")));
        falhou = true;
    }
    for p in &pos {
        let codigo = p.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string();
        let obrigatorio = codigo == "en";
        let nome = relativo(&raiz, p);
        let cat =
            match fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| po::ler(&t).map_err(|e| e.to_string()))
            {
                Ok(c) => c,
                Err(e) => {
                    println!("ERRO: {nome}: {e}");
                    falhou = true;
                    continue;
                }
            };
        let faltas: Vec<String> = achados
            .chaves
            .iter()
            .filter_map(|(chave, info)| {
                conferir(chave, info, &cat)
                    .map(|m| format!("{}: {chave:?} — {m}", info.lugares.first().map_or("", String::as_str)))
            })
            .collect();
        let orfas: Vec<String> =
            cat.chaves.iter().filter(|c| !achados.chaves.contains_key(*c)).map(|c| format!("{c:?}")).collect();
        if faltas.is_empty() {
            println!("{nome}: todas as chaves do código traduzidas.");
        } else if obrigatorio {
            falhou = true;
            println!("ERRO: {nome}: {} chaves do código com problema:", faltas.len());
            listar(&faltas, true, 0);
        } else {
            println!("aviso: {nome}: {} chaves sem tradução (caem no inglês):", faltas.len());
            listar(&faltas, o.detalhes, 20);
        }
        if !orfas.is_empty() {
            println!(
                "{nome}: {} entradas que o código não usa (textos de teste do crate idioma ou chaves velhas).",
                orfas.len()
            );
            if o.detalhes {
                listar(&orfas, true, 0);
            }
        }
    }

    // os literais soltos das telas
    let excecoes = ler_excecoes(&fs::read_to_string(raiz.join("ferramentas/idioma-excecoes.txt")).unwrap_or_default());
    let soltos: Vec<String> = achados
        .soltos
        .iter()
        .filter(|(onde, texto)| !excecao(&excecoes, onde, texto))
        .map(|(onde, texto)| format!("{onde}: {texto:?}"))
        .collect();
    if soltos.is_empty() {
        println!("Crates de tela: nenhum texto com espaço e acento fora do t!.");
    } else if o.so_avisar {
        println!(
            "aviso: {} textos com espaço e acento fora do t! nos crates de tela (a varredura da I3):",
            soltos.len()
        );
        listar(&soltos, o.detalhes, 15);
    } else {
        falhou = true;
        println!("ERRO: {} textos com espaço e acento fora do t! nos crates de tela:", soltos.len());
        listar(&soltos, o.detalhes, 50);
        println!(
            "  Passe o texto pelo t! (ou t_estatico!) ou, se não é para a pessoa ler, ponha em ferramentas/idioma-excecoes.txt."
        );
    }

    if falhou { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn achar(fonte: &str, de_tela: bool) -> Achados {
        let mut a = Achados::default();
        ler_arquivo(&fichas(fonte), "a.rs", de_tela, &mut a);
        a
    }

    #[test]
    fn acha_as_chaves_das_tres_macros() {
        let fonte = r##"
            const SALVAR: &str = t_estatico!("Salvar");
            fn f(n: usize, x: &str) {
                let _ = t!("Abrir pasta");
                let _ = idioma::t!("Gravado em {tabela}", tabela = t!("vendas e compras"));
                let _ = tn!(contar(n, (1, 2)), "{n} linha", "{n} linhas");
                let _ = tn!(n, "Uma em {x}", "{n} em {x}", x = x);
                let _ = t!(r#"Com "aspas" cruas"#);
                let _ = t!("Duas \
                            linhas\n e \"aspas\" \u{e9}");
            }
        "##;
        let a = achar(fonte, false);
        let chaves: Vec<&str> = a.chaves.keys().map(String::as_str).collect();
        assert_eq!(
            chaves,
            [
                "Abrir pasta",
                "Com \"aspas\" cruas",
                "Duas linhas\n e \"aspas\" é",
                "Gravado em {tabela}",
                "Salvar",
                "Uma em {x}",
                "vendas e compras",
                "{n} linha"
            ]
        );
        assert_eq!(a.chaves["{n} linha"].plural.as_deref(), Some("{n} linhas"));
        assert_eq!(a.chaves["Abrir pasta"].plural, None);
        assert_eq!(a.chaves["Abrir pasta"].lugares, ["a.rs:4"]);
        assert!(a.problemas.is_empty(), "{:?}", a.problemas);
    }

    #[test]
    fn literais_soltos_e_o_que_fica_de_fora() {
        let fonte = r#"
            //! "Comentário com ação" não conta
            /* nem "este bloco com ação" */
            #[doc = "atributo com ação"]
            fn f() {
                let a = "Texto solto com ação";
                let b = "SemEspaçoNenhum";
                let c = "sem acento nenhum";
                log::warn!("registro com ação {x}");
                panic!("pânico com ação");
                assert!(true, "conferência com ação");
                let d = 'ç';
                let e: &'static str = "outro solto, à toa";
                let _ = format!("formato com ação {a}");
            }
            #[cfg(test)]
            mod testes {
                fn g() { let _ = "teste com ação"; }
            }
            #[cfg(test)]
            fn h() { let _ = "outro teste com ação"; }
            fn depois() { let _ = "depois dos testes, com ação"; }
        "#;
        let a = achar(fonte, true);
        let soltos: Vec<&str> = a.soltos.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(
            soltos,
            ["Texto solto com ação", "outro solto, à toa", "formato com ação {a}", "depois dos testes, com ação"]
        );
        assert_eq!(a.soltos[0].0, "a.rs:6");
        // fora dos crates de tela, nada
        assert!(achar(fonte, false).soltos.is_empty());
    }

    #[test]
    fn uso_que_o_extrator_nao_entende_e_plural_diferente() {
        let a = achar(r#"fn f(x: &str) { t!(x); tn!(2, "um"); t!("a"); tn!(2, "a", "as"); }"#, false);
        assert_eq!(a.problemas.len(), 3, "{:?}", a.problemas);
    }

    #[test]
    fn excecoes_por_arquivo_e_por_texto() {
        let ex = ler_excecoes("# comentário\n\ncrates/a.rs\ncrates/b.rs: só este\n*: em todo lugar\n");
        assert!(excecao(&ex, "crates/a.rs:3", "qualquer um"));
        assert!(excecao(&ex, "crates/b.rs:9", "só este"));
        assert!(!excecao(&ex, "crates/b.rs:9", "outro"));
        assert!(excecao(&ex, "crates/c.rs:1", "em todo lugar"));
        assert!(!excecao(&ex, "crates/c.rs:1", "nada"));
    }

    #[test]
    fn conferencia_com_o_po() {
        let cat = po::ler(
            "msgid \"Abrir\"\nmsgstr \"Open\"\n\nmsgid \"Em {t}\"\nmsgstr \"In {x}\"\n\n\
             msgid \"{n} linha\"\nmsgid_plural \"{n} linhas\"\nmsgstr[0] \"{n} row\"\nmsgstr[1] \"{n} rows\"\n\n\
             msgid \"Vazia\"\nmsgstr \"\"\n",
        )
        .unwrap();
        let simples = Chave { plural: None, lugares: vec![] };
        let plural = |p: &str| Chave { plural: Some(p.into()), lugares: vec![] };
        assert_eq!(conferir("Abrir", &simples, &cat), None);
        assert_eq!(conferir("Falta", &simples, &cat).as_deref(), Some("falta"));
        assert!(conferir("Vazia", &simples, &cat).is_some_and(|m| m.contains("sem tradução")));
        assert!(conferir("Em {t}", &simples, &cat).is_some_and(|m| m.contains("nomes")));
        assert_eq!(conferir("{n} linha", &plural("{n} linhas"), &cat), None);
        assert!(conferir("{n} linha", &plural("{n} linhas!"), &cat).is_some_and(|m| m.contains("msgid_plural")));
        assert!(conferir("{n} linha", &simples, &cat).is_some_and(|m| m.contains("t!")));
        assert!(conferir("Abrir", &plural("Abrirs"), &cat).is_some_and(|m| m.contains("tn!")));
    }
}
