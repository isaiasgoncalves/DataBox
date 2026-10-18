//! Texto SQL: limpar comentários e textos citados para procurar palavras, dividir um roteiro em comandos e
//! reconhecer o começo de um comando (sem expressões regulares, escrito à mão).

use crate::ffi::comando_completo;
use crate::valores::{e_espaco, strip};

/// Caractere de palavra do `\w` Unicode (letras e dígitos de qualquer escrita, e o "_").
fn e_palavra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn achar_byte(b: &[u8], alvo: u8, desde: usize) -> Option<usize> {
    b.get(desde..)?.iter().position(|&c| c == alvo).map(|p| desde + p)
}

fn achar_par(b: &[u8], alvo: &[u8; 2], desde: usize) -> Option<usize> {
    b.get(desde..)?.windows(2).position(|w| w == alvo).map(|p| desde + p)
}

/// Próximo `--`, `/*`, aspas, crase ou colchete a partir de `desde`.
fn proxima_marca(b: &[u8], desde: usize) -> Option<usize> {
    (desde..b.len()).find(|&p| match b.get(p) {
        Some(b'-') => b.get(p + 1) == Some(&b'-'),
        Some(b'/') => b.get(p + 1) == Some(&b'*'),
        Some(b'\'' | b'"' | b'`' | b'[') => true,
        _ => false,
    })
}

/// Troca comentários por espaço e strings/identificadores citados por " x ", para procurar palavras.
pub(crate) fn limpar_sql(sql: &str) -> String {
    let b = sql.as_bytes();
    let n = b.len();
    let mut saida = String::with_capacity(n);
    let mut i = 0;
    // Todas as marcas são ASCII: as posições achadas são sempre fronteiras de caractere.
    while i < n {
        let Some(p) = proxima_marca(b, i) else {
            saida.push_str(&sql[i..]);
            break;
        };
        saida.push_str(&sql[i..p]);
        match b.get(p).copied().unwrap_or_default() {
            b'-' => {
                i = achar_byte(b, b'\n', p).unwrap_or(n);
                saida.push(' ');
            }
            b'/' => {
                i = achar_par(b, b"*/", p + 2).map_or(n, |j| j + 2);
                saida.push(' ');
            }
            b'[' => {
                i = achar_byte(b, b']', p + 1).map_or(n, |j| j + 1);
                saida.push_str(" x ");
            }
            aspa => {
                let mut j = p + 1;
                loop {
                    match achar_byte(b, aspa, j) {
                        None => {
                            j = n;
                            break;
                        }
                        Some(k) if b.get(k + 1) == Some(&aspa) => j = k + 2,
                        Some(k) => {
                            j = k + 1;
                            break;
                        }
                    }
                }
                i = j;
                saida.push_str(" x ");
            }
        }
    }
    saida
}

fn casa_sem_caixa(texto: &str, pos: usize, palavra: &str) -> bool {
    texto.as_bytes().get(pos..pos + palavra.len()).is_some_and(|f| f.eq_ignore_ascii_case(palavra.as_bytes()))
}

/// Posição da próxima ocorrência de `palavra` (ASCII) como palavra inteira (`\bPALAVRA\b`, sem caixa).
fn achar_palavra(texto: &str, palavra: &str, desde: usize) -> Option<usize> {
    let resto = texto.get(desde..)?;
    resto.char_indices().map(|(i, _)| desde + i).find(|&p| {
        casa_sem_caixa(texto, p, palavra)
            && texto[..p].chars().next_back().is_none_or(|c| !e_palavra(c))
            && texto[p + palavra.len()..].chars().next().is_none_or(|c| !e_palavra(c))
    })
}

/// `re.search(rf"\b{palavra}\b", _limpar_sql(sql), re.I)`: a palavra aparece fora de textos e comentários?
pub(crate) fn tem_palavra(sql: Option<&str>, palavra: &str) -> bool {
    match sql {
        Some(sql) if !sql.is_empty() => achar_palavra(&limpar_sql(sql), palavra, 0).is_some(),
        _ => false,
    }
}

/// `re.match(r"\s*P1\s+P2…\b", limpo, re.I)`: o texto (já limpo) começa com essas palavras?
pub(crate) fn comeca_com(limpo: &str, palavras: &[&str]) -> bool {
    let mut resto = limpo.trim_start_matches(e_espaco);
    for (k, palavra) in palavras.iter().enumerate() {
        if k > 0 {
            let sem_espaco = resto.trim_start_matches(e_espaco);
            if sem_espaco.len() == resto.len() {
                return false;
            }
            resto = sem_espaco;
        }
        if !casa_sem_caixa(resto, 0, palavra) {
            return false;
        }
        resto = &resto[palavra.len()..];
    }
    resto.chars().next().is_none_or(|c| !e_palavra(c))
}

/// `re.search(r"\bUSING\s+([A-Za-z_][A-Za-z0-9_]*)", limpo, re.I)`, em minúsculas: o módulo de uma tabela
/// virtual.
pub(crate) fn modulo_da_tabela_virtual(limpo: &str) -> Option<String> {
    let mut desde = 0;
    while let Some(p) = achar_palavra(limpo, "USING", desde) {
        let depois = &limpo[p + 5..];
        let sem_espaco = depois.trim_start_matches(e_espaco);
        if sem_espaco.len() < depois.len() {
            let nome: String = sem_espaco
                .chars()
                .enumerate()
                .take_while(|&(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
                .map(|(_, c)| c)
                .collect();
            if !nome.is_empty() {
                return Some(nome.to_lowercase());
            }
        }
        desde = p + 5;
    }
    None
}

fn anexar_comando(comandos: &mut Vec<String>, trecho: &str) {
    let texto = strip(trecho);
    if !strip(&limpar_sql(texto).replace(';', "")).is_empty() {
        comandos.push(texto.to_string());
    }
}

/// Divide um roteiro SQL em comandos de topo.
///
/// Acumula o texto até cada ";" e só corta quando o `sqlite3_complete` diz que o trecho fecha um comando:
/// assim ";" dentro de string, comentário ou do BEGIN…END de um gatilho não corta nada. Comandos vazios (só
/// ";" ou só comentário) são descartados.
pub fn dividir_comandos(sql: &str) -> Vec<String> {
    let mut comandos = Vec::new();
    let mut inicio = 0;
    let mut busca = 0;
    while let Some(rel) = sql.get(busca..).and_then(|s| s.find(';')) {
        let pos = busca + rel;
        let trecho = &sql[inicio..=pos];
        if comando_completo(trecho) {
            anexar_comando(&mut comandos, trecho);
            inicio = pos + 1;
        }
        busca = pos + 1;
    }
    anexar_comando(&mut comandos, &sql[inicio..]);
    comandos
}

/// O comando é INSERT, UPDATE, DELETE ou REPLACE (depois de espaços e comentários)? Só esses dizem quantas linhas mudaram; os outros dão -1 (aqui, None).
pub(crate) fn e_dml(sql: &str) -> bool {
    let b = sql.as_bytes();
    let mut i = 0;
    loop {
        match b.get(i) {
            None => return false,
            Some(b' ' | b'\t' | b'\x0c' | b'\n' | b'\r') => i += 1,
            Some(b'-') if b.get(i + 1) == Some(&b'-') => match achar_byte(b, b'\n', i + 2) {
                Some(j) => i = j + 1,
                None => return false,
            },
            Some(b'/') if b.get(i + 1) == Some(&b'*') => match achar_par(b, b"*/", i + 2) {
                Some(j) => i = j + 2,
                None => return false,
            },
            Some(_) => break,
        }
    }
    let resto = b.get(i..).unwrap_or_default();
    let comeca = |p: &[u8]| resto.get(..p.len()).is_some_and(|f| f.eq_ignore_ascii_case(p));
    comeca(b"insert") || comeca(b"update") || comeca(b"delete") || comeca(b"replace")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limpar_sql_troca_textos_e_comentarios() {
        assert_eq!(limpar_sql("a 'b;c' -- x\nd /* e */ f [g] \"h\"\"i\" `j`"), "a  x   \nd   f  x   x   x ");
        assert_eq!(limpar_sql("'sem fim"), " x ");
        assert_eq!(limpar_sql("ação 'é' fim"), "ação  x  fim");
    }

    #[test]
    fn palavras_fora_de_texto_e_comentario() {
        assert!(tem_palavra(Some("id integer primary key autoincrement"), "AUTOINCREMENT"));
        assert!(!tem_palavra(Some("x DEFAULT 'AUTOINCREMENT' /* AUTOINCREMENT */"), "AUTOINCREMENT"));
        assert!(!tem_palavra(Some("AUTOINCREMENTO"), "AUTOINCREMENT"));
        assert!(!tem_palavra(Some("çAUTOINCREMENT"), "AUTOINCREMENT"));
        assert!(comeca_com("  create\n virtual table x using fts5(a)", &["CREATE", "VIRTUAL"]));
        assert!(!comeca_com("CREATE VIRTUALX", &["CREATE", "VIRTUAL"]));
        assert!(!comeca_com("CREATEVIRTUAL", &["CREATE", "VIRTUAL"]));
        assert!(comeca_com(" PRAGMA foreign_keys", &["PRAGMA"]));
        assert_eq!(modulo_da_tabela_virtual("CREATE VIRTUAL TABLE n USING FTS5(a, b)").as_deref(), Some("fts5"));
        assert_eq!(modulo_da_tabela_virtual("using; USING  rtree_i32 (x)").as_deref(), Some("rtree_i32"));
        assert_eq!(modulo_da_tabela_virtual("USING 9x"), None);
    }

    #[test]
    fn dml_conta_linhas_afetadas() {
        for s in
            ["INSERT INTO t VALUES (1)", "/* c */ DELETE FROM t", "-- x\nINSERT INTO t", "REPLACE INTO t", "  update t"]
        {
            assert!(e_dml(s), "{s}");
        }
        for s in ["WITH x AS (SELECT 1) INSERT INTO t SELECT 1", "BEGIN", "PRAGMA x", "-- só comentário", "/* aberto"]
        {
            assert!(!e_dml(s), "{s}");
        }
    }
}
