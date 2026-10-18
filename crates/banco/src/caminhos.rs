//! Caminhos de arquivo: resolução (com `~`), comparação de arquivos e a URI do SQLite.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// `~` no começo do caminho vira a pasta da pessoa (`expanduser`).
fn expandir_usuario(caminho: &Path) -> PathBuf {
    let mut partes = caminho.components();
    if let Some(Component::Normal(primeira)) = partes.next()
        && primeira == "~"
        && let Some(casa) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
    {
        return PathBuf::from(casa).join(partes.as_path());
    }
    caminho.to_path_buf()
}

/// Tira o prefixo `\\?\` que o `canonicalize` do Windows põe (`\\?\UNC\srv\x` → `\\srv\x`).
fn sem_prefixo_estendido(caminho: PathBuf) -> PathBuf {
    let texto = caminho.as_os_str().to_string_lossy();
    if let Some(resto) = texto.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{resto}"));
    }
    if let Some(resto) = texto.strip_prefix(r"\\?\")
        && resto.as_bytes().get(1) == Some(&b':')
    {
        return PathBuf::from(resto);
    }
    caminho
}

/// Caminho absoluto, com os links e os nomes curtos (8.3) resolvidos na parte que existe; o resto do caminho
/// (um arquivo que ainda vai ser criado) é anexado como veio. O `~` no começo vira a pasta pessoal.
pub(crate) fn resolver(caminho: &Path) -> io::Result<PathBuf> {
    // o caminho vazio é a pasta atual
    let caminho = if caminho.as_os_str().is_empty() { Path::new(".") } else { caminho };
    let absoluto = std::path::absolute(expandir_usuario(caminho))?;
    let mut base = absoluto.clone();
    let mut resto: Vec<OsString> = Vec::new();
    loop {
        if let Ok(real) = fs::canonicalize(&base) {
            let mut saida = sem_prefixo_estendido(real);
            for parte in resto.iter().rev() {
                saida.push(parte);
            }
            return Ok(saida);
        }
        match (base.parent(), base.file_name()) {
            (Some(pai), Some(nome)) => {
                resto.push(nome.to_os_string());
                base = pai.to_path_buf();
            }
            _ => return Ok(absoluto),
        }
    }
}

/// `os.path.normcase`: no Windows, sem diferença de caixa e com "\" como separador.
pub(crate) fn normcase(caminho: &Path) -> String {
    let texto = caminho.as_os_str().to_string_lossy();
    if cfg!(windows) { texto.replace('/', "\\").to_lowercase() } else { texto.into_owned() }
}

/// Os dois caminhos são o mesmo arquivo? (`os.path.samefile` quando os dois existem.)
pub(crate) fn mesmo_arquivo(a: &Path, b: &Path) -> bool {
    if a.exists()
        && b.exists()
        && let (Ok(x), Ok(y)) = (fs::canonicalize(a), fs::canonicalize(b))
    {
        return normcase(&x) == normcase(&y);
    }
    normcase(a) == normcase(b)
}

/// `<caminho><sufixo>` ("-journal", "-wal", "-shm").
pub(crate) fn com_sufixo(caminho: &Path, sufixo: &str) -> PathBuf {
    let mut texto = caminho.as_os_str().to_os_string();
    texto.push(sufixo);
    PathBuf::from(texto)
}

pub(crate) fn tamanho_arquivo(caminho: &Path) -> u64 {
    fs::metadata(caminho).map(|m| m.len()).unwrap_or(0)
}

/// Data local como o `datetime.isoformat()` sem frações: "2026-09-22T10:31:00".
pub(crate) fn formatar_data(quando: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(quando).format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// URI do SQLite para o arquivo (`file:///C:/pasta/x.db?mode=rw`), com os bytes fora do conjunto seguro em
/// %XX (espaço, "#", "?", "%" e acentos). Caminho de rede (`\\servidor\pasta`) vira `file:////servidor/pasta`:
/// o SQLite só aceita autoridade vazia.
pub(crate) fn uri_de(caminho: &Path, modo: &str) -> String {
    let texto = caminho.as_os_str().to_string_lossy().replace('\\', "/");
    let mut uri = String::with_capacity(texto.len() * 3 + 20);
    uri.push_str("file:");
    uri.push_str(if texto.starts_with('/') { "//" } else { "///" });
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            uri.push(char::from(b));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(b >> 4)]));
            uri.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
    }
    uri.push_str("?mode=");
    uri.push_str(modo);
    uri
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn uri_do_sqlite() {
        assert_eq!(
            uri_de(Path::new(r"C:\Pasta com espaço\vendas #1.db"), "ro"),
            "file:///C:/Pasta%20com%20espa%C3%A7o/vendas%20%231.db?mode=ro"
        );
        assert_eq!(uri_de(Path::new(r"\\servidor\pasta\x.db"), "rw"), "file:////servidor/pasta/x.db?mode=rw");
    }

    #[test]
    fn prefixo_estendido() {
        assert_eq!(sem_prefixo_estendido(PathBuf::from(r"\\?\C:\a\b.db")), PathBuf::from(r"C:\a\b.db"));
        assert_eq!(sem_prefixo_estendido(PathBuf::from(r"\\?\UNC\srv\x\b.db")), PathBuf::from(r"\\srv\x\b.db"));
    }
}
