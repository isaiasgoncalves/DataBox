//! "Mostrar na pasta" e "Abrir" de um arquivo. A montagem do comando fica separada da execução para ser testada sem abrir janela.
//!
//! No Windows, pelo Explorador de Arquivos (`explorer.exe`); no Linux (e nos outros Unix), pelo `xdg-open`, que
//! abre o arquivo no programa associado e a pasta no gerenciador de arquivos da área de trabalho (sem selecionar o
//! arquivo: o `xdg-open` não tem como).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use comum::{ErroBanco, Resultado};

/// Um programa e os argumentos dele. No Windows, a linha de argumentos passada como está (sem as aspas automáticas
/// do Rust, que transformariam `/select,"C:\…"` em `"/select,\"C:\…\""`); fora dele, um argumento só (o caminho).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoArquivo {
    pub programa: PathBuf,
    pub argumentos: OsString,
}

impl ComandoArquivo {
    /// A linha de comando inteira, como o Windows a recebe: `"C:\Windows\explorer.exe" /select,"C:\…"`; fora
    /// dele, o programa e o caminho entre aspas: `xdg-open "/home/…"`.
    pub fn linha(&self) -> String {
        if cfg!(windows) {
            format!("\"{}\" {}", self.programa.display(), self.argumentos.to_string_lossy())
        } else {
            format!("{} \"{}\"", self.programa.display(), self.argumentos.to_string_lossy())
        }
    }

    /// Inicia o programa sem esperar por ele.
    pub(crate) fn executar(&self, contexto: &str) -> Resultado<()> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new(&self.programa)
                .raw_arg(&self.argumentos)
                .spawn()
                .map(drop)
                .map_err(|e| comum::traduzir_io(&e, Some(&self.programa), Some(contexto)))
        }
        #[cfg(not(windows))]
        {
            let mut filho = std::process::Command::new(&self.programa)
                .arg(&self.argumentos)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        ErroBanco::com_status(
                            format!(
                                "{contexto} O programa {} não foi encontrado: instale o pacote xdg-utils.",
                                self.programa.display()
                            ),
                            501,
                        )
                    } else {
                        comum::traduzir_io(&e, Some(&self.programa), Some(contexto))
                    }
                })?;
            // o xdg-open sai logo; esperar por ele numa thread à parte evita o processo zumbi
            std::thread::spawn(move || filho.wait());
            Ok(())
        }
    }
}

/// O `_exigir_existente`: caminho absoluto de um arquivo (ou pasta) que existe.
fn exigir_existente(caminho: &Path) -> Resultado<PathBuf> {
    if !ddl::preenchido(&caminho.as_os_str().to_string_lossy()) {
        return Err(ErroBanco::novo("Nenhum arquivo foi indicado."));
    }
    let absoluto = std::path::absolute(caminho).unwrap_or_else(|_| caminho.to_path_buf());
    if !absoluto.exists() {
        return Err(ErroBanco::com_status(
            format!("O arquivo não existe (ou foi movido): {}.", absoluto.display()),
            404,
        ));
    }
    Ok(absoluto)
}

/// `%SystemRoot%\explorer.exe` (sem a variável, `C:\Windows`).
fn explorer(raiz_sistema: Option<&OsStr>) -> PathBuf {
    let raiz = raiz_sistema.filter(|r| !r.is_empty()).unwrap_or(OsStr::new(r"C:\Windows"));
    Path::new(raiz).join("explorer.exe")
}

/// `"<caminho>"`: caminho do Windows não pode conter aspas, então elas bastam para espaços e acentos.
fn entre_aspas(antes: &str, caminho: &Path) -> OsString {
    let mut argumento = OsString::from(antes);
    argumento.push("\"");
    argumento.push(caminho.as_os_str());
    argumento.push("\"");
    argumento
}

/// O comando do "Mostrar na pasta": o Explorador de Arquivos na pasta do arquivo, com ele selecionado
/// (`explorer.exe /select,"caminho"`, a única forma documentada). Vazio → 400 "Nenhum arquivo foi
/// indicado."; inexistente → 404 "O arquivo não existe (ou foi movido): …". Fora do Windows, a pasta do arquivo
/// pelo `xdg-open` ([`montar_revelar_xdg`]).
pub fn comando_revelar(caminho: &Path) -> Resultado<ComandoArquivo> {
    if cfg!(windows) {
        montar_revelar(std::env::var_os("SystemRoot").as_deref(), caminho)
    } else {
        montar_revelar_xdg(caminho)
    }
}

pub(crate) fn montar_revelar(raiz_sistema: Option<&OsStr>, caminho: &Path) -> Resultado<ComandoArquivo> {
    let caminho = exigir_existente(caminho)?;
    Ok(ComandoArquivo { programa: explorer(raiz_sistema), argumentos: entre_aspas("/select,", &caminho) })
}

/// O programa que abre arquivos e pastas fora do Windows.
const XDG_OPEN: &str = "xdg-open";

/// O "Mostrar na pasta" fora do Windows: `xdg-open` na pasta que contém o arquivo (ou a pasta, se for uma).
pub(crate) fn montar_revelar_xdg(caminho: &Path) -> Resultado<ComandoArquivo> {
    let caminho = exigir_existente(caminho)?;
    let pasta = if caminho.is_dir() {
        caminho.clone()
    } else {
        caminho.parent().map(Path::to_path_buf).unwrap_or(caminho.clone())
    };
    Ok(ComandoArquivo { programa: PathBuf::from(XDG_OPEN), argumentos: pasta.into_os_string() })
}

/// O comando do "Abrir": o arquivo no programa associado à extensão (Excel para .xlsx…), pelo Explorador de
/// Arquivos (`explorer.exe "caminho"`, que abre como um duplo clique). Erros como em [`comando_revelar`].
/// Fora do Windows, `xdg-open "caminho"` ([`montar_abrir_xdg`]).
pub fn comando_abrir(caminho: &Path) -> Resultado<ComandoArquivo> {
    if cfg!(windows) {
        montar_abrir(std::env::var_os("SystemRoot").as_deref(), caminho)
    } else {
        montar_abrir_xdg(caminho)
    }
}

pub(crate) fn montar_abrir(raiz_sistema: Option<&OsStr>, caminho: &Path) -> Resultado<ComandoArquivo> {
    let caminho = exigir_existente(caminho)?;
    Ok(ComandoArquivo { programa: explorer(raiz_sistema), argumentos: entre_aspas("", &caminho) })
}

/// O "Abrir" fora do Windows: `xdg-open "caminho"`, que usa o programa associado ao tipo do arquivo.
pub(crate) fn montar_abrir_xdg(caminho: &Path) -> Resultado<ComandoArquivo> {
    let caminho = exigir_existente(caminho)?;
    Ok(ComandoArquivo { programa: PathBuf::from(XDG_OPEN), argumentos: caminho.into_os_string() })
}
