//! Caminhos no estilo do Windows (`abspath`, `normcase`, `normpath`, `basename`, `dirname`), aparar espaços e
//! citar textos, para calcular sempre a mesma chave de um
//! caminho e mostrarem o mesmo nome e a mesma pasta.
//!
//! - `abspath(p)` é `GetFullPathNameW(normpath(p))`: aqui, [`std::path::absolute`] (que no Windows chama o
//!   `GetFullPathNameW`) sobre o `normpath`.
//! - `normcase(p)` é `LCMapStringEx(LOCALE_NAME_INVARIANT, LCMAP_LOWERCASE, p.replace("/", "\\"))`. A tabela
//!   do Windows é a minúscula simples do Unicode menos alguns caracteres que ela não conhece (cherokee,
//!   georgiano mtavruli, `İ`, `ẞ`, o sinal de Kelvin…): [`EXCECOES_MINUSCULA`], conferida caractere a
//!   caractere contra o Windows.

use std::path::Path;

/// Faixas (inclusivas) de caracteres que o `char::to_lowercase` muda e o `LCMapStringEx` do Windows deixa como
/// estão. Geradas percorrendo todos os caracteres no `LCMapStringEx`, mais as letras novas do Unicode 17
/// (U+A7CE, U+A7D2, U+A7D4, U+16EA0–U+16EB8) que o Rust já conhece e o Windows não.
const EXCECOES_MINUSCULA: &[(u32, u32)] = &[
    (0x0130, 0x0130),
    (0x01C5, 0x01C5),
    (0x01C8, 0x01C8),
    (0x01CB, 0x01CB),
    (0x01F2, 0x01F2),
    (0x037F, 0x037F),
    (0x03F4, 0x03F4),
    (0x0524, 0x0524),
    (0x0526, 0x0526),
    (0x0528, 0x0528),
    (0x052A, 0x052A),
    (0x052C, 0x052C),
    (0x052E, 0x052E),
    (0x10C7, 0x10C7),
    (0x10CD, 0x10CD),
    (0x13A0, 0x13F5),
    (0x1C89, 0x1C89),
    (0x1C90, 0x1CBA),
    (0x1CBD, 0x1CBF),
    (0x1E9E, 0x1E9E),
    (0x2126, 0x2126),
    (0x212A, 0x212B),
    (0x2C2F, 0x2C2F),
    (0x2C70, 0x2C70),
    (0x2C7E, 0x2C7F),
    (0x2CEB, 0x2CEB),
    (0x2CED, 0x2CED),
    (0x2CF2, 0x2CF2),
    (0xA660, 0xA660),
    (0xA698, 0xA698),
    (0xA69A, 0xA69A),
    (0xA78D, 0xA78D),
    (0xA790, 0xA790),
    (0xA792, 0xA792),
    (0xA796, 0xA796),
    (0xA798, 0xA798),
    (0xA79A, 0xA79A),
    (0xA79C, 0xA79C),
    (0xA79E, 0xA79E),
    (0xA7A0, 0xA7A0),
    (0xA7A2, 0xA7A2),
    (0xA7A4, 0xA7A4),
    (0xA7A6, 0xA7A6),
    (0xA7A8, 0xA7A8),
    (0xA7AA, 0xA7AE),
    (0xA7B0, 0xA7B4),
    (0xA7B6, 0xA7B6),
    (0xA7B8, 0xA7B8),
    (0xA7BA, 0xA7BA),
    (0xA7BC, 0xA7BC),
    (0xA7BE, 0xA7BE),
    (0xA7C0, 0xA7C0),
    (0xA7C2, 0xA7C2),
    (0xA7C4, 0xA7C7),
    (0xA7C9, 0xA7C9),
    (0xA7CB, 0xA7CC),
    (0xA7CE, 0xA7CE),
    (0xA7D0, 0xA7D0),
    (0xA7D2, 0xA7D2),
    (0xA7D4, 0xA7D4),
    (0xA7D6, 0xA7D6),
    (0xA7D8, 0xA7D8),
    (0xA7DA, 0xA7DA),
    (0xA7DC, 0xA7DC),
    (0xA7F5, 0xA7F5),
    (0x104B0, 0x104D3),
    (0x10570, 0x1057A),
    (0x1057C, 0x1058A),
    (0x1058C, 0x10592),
    (0x10594, 0x10595),
    (0x10C80, 0x10CB2),
    (0x10D50, 0x10D65),
    (0x118A0, 0x118BF),
    (0x16E40, 0x16E5F),
    (0x16EA0, 0x16EB8),
    (0x1E900, 0x1E921),
];

fn e_sep(b: u8) -> bool {
    b == b'\\' || b == b'/'
}

/// O caminho exibido: absoluto e normalizado, com a caixa como veio.
pub fn caminho_exibido(caminho: impl AsRef<Path>) -> String {
    abspath(&caminho.as_ref().to_string_lossy())
}

/// A chave de comparação (`normcase(abspath(caminho))`): sem diferença de caixa nem
/// de barra. É a chave do histórico de SQL no arquivo de preferências.
pub fn chave_caminho(caminho: impl AsRef<Path>) -> String {
    normcase(&caminho_exibido(caminho))
}

/// Absoluto e normalizado, no estilo do Windows.
pub(crate) fn abspath(texto: &str) -> String {
    let normal = normpath(texto);
    match std::path::absolute(Path::new(&normal)) {
        Ok(absoluto) => absoluto.to_string_lossy().into_owned(),
        Err(_) => abspath_reserva(texto),
    }
}

/// O caminho de reserva do `ntpath.abspath` quando o `GetFullPathNameW` recusa (um NUL no meio, por exemplo).
fn abspath_reserva(texto: &str) -> String {
    let caminho = if isabs(texto) {
        texto.to_string()
    } else {
        let (drive, root, resto) = splitroot(texto);
        if !drive.is_empty() || !root.is_empty() {
            match std::path::absolute(Path::new(&format!("{drive}{root}"))) {
                Ok(base) => juntar(&base.to_string_lossy(), resto),
                Err(_) => format!("{drive}\\{resto}"),
            }
        } else {
            match std::env::current_dir() {
                Ok(cwd) => juntar(&cwd.to_string_lossy(), texto),
                Err(_) => texto.to_string(),
            }
        }
    };
    normpath(&caminho)
}

/// `ntpath.join(base, relativo)` para um `relativo` sem unidade nem raiz.
fn juntar(base: &str, relativo: &str) -> String {
    if base.is_empty() || base.as_bytes().last().is_some_and(|b| e_sep(*b) || *b == b':') {
        format!("{base}{relativo}")
    } else {
        format!("{base}\\{relativo}")
    }
}

/// `ntpath.isabs`: UNC, dispositivo ou unidade com raiz.
fn isabs(texto: &str) -> bool {
    let inicio: String = texto.chars().take(3).map(|c| if c == '/' { '\\' } else { c }).collect();
    inicio.starts_with("\\\\") || inicio.chars().skip(1).take(2).eq(":\\".chars())
}

/// `ntpath.splitroot`: (unidade, raiz, resto), fatias do texto original.
pub(crate) fn splitroot(p: &str) -> (&str, &str, &str) {
    let b = p.as_bytes();
    if b.first().is_some_and(|c| e_sep(*c)) {
        if b.get(1).is_some_and(|c| e_sep(*c)) {
            // UNC (\\servidor\compartilhamento, \\?\UNC\servidor\compartilhamento) ou dispositivo (\\.\x, \\?\x)
            let prefixo_unc = b"\\\\?\\UNC\\";
            let e_unc = b.len() >= 8
                && b.iter().zip(prefixo_unc.iter()).all(|(x, y)| {
                    let x = if *x == b'/' { b'\\' } else { x.to_ascii_uppercase() };
                    x == *y
                });
            let inicio = if e_unc { 8 } else { 2 };
            let achar = |de: usize| b.iter().skip(de).position(|c| e_sep(*c)).map(|i| i + de);
            let Some(indice) = achar(inicio) else {
                return (p, "", "");
            };
            let Some(indice2) = achar(indice + 1) else {
                return (p, "", "");
            };
            // separadores são ASCII: os índices caem em fronteira de caractere
            return (
                p.get(..indice2).unwrap_or(p),
                p.get(indice2..indice2 + 1).unwrap_or(""),
                p.get(indice2 + 1..).unwrap_or(""),
            );
        }
        return ("", p.get(..1).unwrap_or(""), p.get(1..).unwrap_or(""));
    }
    let mut chars = p.char_indices();
    let _primeiro = chars.next();
    if let Some((i1, ':')) = chars.next() {
        let fim_drive = i1 + 1;
        let drive = p.get(..fim_drive).unwrap_or("");
        if b.get(fim_drive).is_some_and(|c| e_sep(*c)) {
            return (drive, p.get(fim_drive..fim_drive + 1).unwrap_or(""), p.get(fim_drive + 1..).unwrap_or(""));
        }
        return (drive, "", p.get(fim_drive..).unwrap_or(""));
    }
    ("", "", p)
}

/// Normaliza `.`, `..`, barras repetidas e `/` → `\`.
pub(crate) fn normpath(caminho: &str) -> String {
    let caminho = caminho.replace('/', "\\");
    let (drive, root, resto) = splitroot(&caminho);
    let prefixo = format!("{drive}{root}");
    let mut partes: Vec<&str> = Vec::new();
    for parte in resto.split('\\') {
        match parte {
            "" | "." => {}
            ".." => {
                if partes.last().is_some_and(|p| *p != "..") {
                    partes.pop();
                } else if partes.is_empty() && !root.is_empty() {
                    // ".." logo depois da raiz não sobe além dela
                } else {
                    partes.push("..");
                }
            }
            outra => partes.push(outra),
        }
    }
    if prefixo.is_empty() && partes.is_empty() {
        return ".".to_string();
    }
    format!("{prefixo}{}", partes.join("\\"))
}

/// Um caractere em minúscula como o `LCMapStringEx` invariante do Windows.
pub(crate) fn minuscula_windows(c: char) -> char {
    if c == '/' {
        return '\\';
    }
    let cp = u32::from(c);
    let excecao = EXCECOES_MINUSCULA
        .binary_search_by(|(ini, fim)| {
            if *fim < cp {
                std::cmp::Ordering::Less
            } else if *ini > cp {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok();
    if excecao {
        return c;
    }
    let mut minusculas = c.to_lowercase();
    match (minusculas.next(), minusculas.next()) {
        (Some(m), None) => m,
        _ => c,
    }
}

/// Minúsculas (tabela do Windows) e `/` → `\`.
pub(crate) fn normcase(texto: &str) -> String {
    texto.chars().map(minuscula_windows).collect()
}

/// `ntpath.split`: (cabeça, cauda).
fn dividir(p: &str) -> (String, &str) {
    let (drive, root, resto) = splitroot(p);
    let b = resto.as_bytes();
    let mut i = b.len();
    while i > 0 && b.get(i - 1).is_some_and(|c| !e_sep(*c)) {
        i -= 1;
    }
    let cabeca = resto.get(..i).unwrap_or("");
    let cauda = resto.get(i..).unwrap_or("");
    (format!("{drive}{root}{}", cabeca.trim_end_matches(['\\', '/'])), cauda)
}

/// O último componente do caminho.
pub(crate) fn nome_base(p: &str) -> &str {
    dividir(p).1
}

/// O caminho sem o último componente.
pub(crate) fn pasta_de(p: &str) -> String {
    dividir(p).0
}

/// Espaço: o `White_Space` do Unicode mais os separadores de informação 0x1C–0x1F.
pub(crate) fn e_espaco(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Tira [`e_espaco`] das pontas.
pub(crate) fn tirar_espacos(texto: &str) -> &str {
    texto.trim_matches(e_espaco)
}

/// Caractere que vai como está no texto citado (controles, separadores e formatação comuns).
fn imprimivel(c: char) -> bool {
    let cp = u32::from(c);
    if c == ' ' {
        return true;
    }
    if c.is_control() || c.is_whitespace() {
        return false;
    }
    let formatacao = matches!(cp,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x180E | 0x200B..=0x200F | 0x202A..=0x202E
        | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x1BCA0..=0x1BCA3
        | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F);
    let uso_privado = matches!(cp, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD);
    !formatacao && !uso_privado
}

/// O texto citado (aspas e escapes), para o detalhe técnico dos erros ("csv_separador='|'").
pub(crate) fn citar(texto: &str) -> String {
    let aspa = if texto.contains('\'') && !texto.contains('"') { '"' } else { '\'' };
    let mut saida = String::with_capacity(texto.len() + 2);
    saida.push(aspa);
    for c in texto.chars() {
        match c {
            '\\' => saida.push_str("\\\\"),
            '\t' => saida.push_str("\\t"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            c if c == aspa => {
                saida.push('\\');
                saida.push(c);
            }
            c if imprimivel(c) => saida.push(c),
            c => {
                let cp = u32::from(c);
                if cp <= 0xFF {
                    saida.push_str(&format!("\\x{cp:02x}"));
                } else if cp <= 0xFFFF {
                    saida.push_str(&format!("\\u{cp:04x}"));
                } else {
                    saida.push_str(&format!("\\U{cp:08x}"));
                }
            }
        }
    }
    saida.push(aspa);
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normpath_normaliza() {
        assert_eq!(normpath("C:/a/./b/../c//d.db"), "C:\\a\\c\\d.db");
        assert_eq!(normpath("C:\\..\\x"), "C:\\x");
        assert_eq!(normpath("..\\..\\x"), "..\\..\\x");
        assert_eq!(normpath("a\\.."), ".");
        assert_eq!(normpath(""), ".");
        assert_eq!(normpath("\\\\servidor\\pasta\\..\\x"), "\\\\servidor\\pasta\\x");
        assert_eq!(normpath("C:rel\\..\\x"), "C:x");
    }

    #[test]
    fn splitroot_separa_unidade_e_raiz() {
        assert_eq!(splitroot("C:\\a\\b"), ("C:", "\\", "a\\b"));
        assert_eq!(splitroot("C:a"), ("C:", "", "a"));
        assert_eq!(splitroot("\\a"), ("", "\\", "a"));
        assert_eq!(splitroot("//srv/share/x"), ("//srv/share", "/", "x"));
        assert_eq!(splitroot("\\\\?\\UNC\\srv\\share\\x"), ("\\\\?\\UNC\\srv\\share", "\\", "x"));
        assert_eq!(splitroot("\\\\srv"), ("\\\\srv", "", ""));
        assert_eq!(splitroot("Á:x"), ("Á:", "", "x"));
        assert_eq!(splitroot("ação"), ("", "", "ação"));
    }

    #[test]
    fn nome_e_pasta() {
        assert_eq!(nome_base("C:\\dados\\vendas.db"), "vendas.db");
        assert_eq!(pasta_de("C:\\dados\\vendas.db"), "C:\\dados");
        assert_eq!(pasta_de("C:\\vendas.db"), "C:\\");
        assert_eq!(pasta_de("\\\\srv\\share\\x.db"), "\\\\srv\\share\\");
        assert_eq!(nome_base("C:/a/ação.db"), "ação.db");
        assert_eq!(pasta_de("C:/a//ação.db"), "C:/a");
        assert_eq!(nome_base("C:\\a\\"), "");
    }

    #[test]
    fn normcase_com_a_tabela_do_windows() {
        assert_eq!(normcase("C:/Dados/AÇÃO.DB"), "c:\\dados\\ação.db");
        assert_eq!(normcase("İ"), "İ");
        assert_eq!(normcase("ẞ\u{212A}"), "ẞ\u{212A}");
        assert_eq!(normcase("ΣΑΣ"), "σασ");
        assert_eq!(normcase("\u{10400}"), "\u{10428}");
        assert_eq!(normcase("\u{A7CE}\u{16EA0}\u{13A0}"), "\u{A7CE}\u{16EA0}\u{13A0}"); // o Windows não conhece
        assert_eq!(normcase(""), "");
    }

    #[test]
    fn caminho_relativo_fica_absoluto() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(caminho_exibido("x.db"), cwd.join("x.db").to_string_lossy());
        assert_eq!(caminho_exibido("C:/A/../B/c.db"), "C:\\B\\c.db");
        assert_eq!(chave_caminho("C:/A/../B/C.db"), "c:\\b\\c.db");
    }

    #[test]
    fn aparar_e_citar() {
        assert_eq!(tirar_espacos("\u{1c} SELECT 1 \n\u{a0}"), "SELECT 1");
        assert_eq!(citar("|"), "'|'");
        assert_eq!(citar("\t"), "'\\t'");
        assert_eq!(citar("it's"), "\"it's\"");
        assert_eq!(citar("a'\"b"), "'a\\'\"b'");
        assert_eq!(citar("\u{1}\u{a0}ç"), "'\\x01\\xa0ç'");
    }
}
