//! "Mostrar na pasta" e "Abrir" de um arquivo. A montagem do comando fica separada da execução para ser testada sem abrir janela.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use comum::{ErroBanco, Resultado};

/// Um programa do Windows e a linha de argumentos dele, passada como está (sem as aspas automáticas do Rust,
/// que transformariam `/select,"C:\…"` em `"/select,\"C:\…\""`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoArquivo {
    pub programa: PathBuf,
    pub argumentos: OsString,
}

impl ComandoArquivo {
    /// A linha de comando inteira, como o Windows a recebe: `"C:\Windows\explorer.exe" /select,"C:\…"`.
    pub fn linha(&self) -> String {
        format!("\"{}\" {}", self.programa.display(), self.argumentos.to_string_lossy())
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
            Err(ErroBanco::com_status(contexto.to_string(), 501))
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
/// indicado."; inexistente → 404 "O arquivo não existe (ou foi movido): …"; fora do Windows → 501.
pub fn comando_revelar(caminho: &Path) -> Resultado<ComandoArquivo> {
    montar_revelar(std::env::var_os("SystemRoot").as_deref(), caminho)
}

pub(crate) fn montar_revelar(raiz_sistema: Option<&OsStr>, caminho: &Path) -> Resultado<ComandoArquivo> {
    let caminho = exigir_existente(caminho)?;
    if !cfg!(windows) {
        return Err(ErroBanco::com_status("Mostrar na pasta só funciona no Windows.", 501));
    }
    Ok(ComandoArquivo { programa: explorer(raiz_sistema), argumentos: entre_aspas("/select,", &caminho) })
}

/// O comando do "Abrir": o arquivo no programa associado à extensão (Excel para .xlsx…), pelo Explorador de
/// Arquivos (`explorer.exe "caminho"`, que abre como um duplo clique). Erros como em [`comando_revelar`].
pub fn comando_abrir(caminho: &Path) -> Resultado<ComandoArquivo> {
    montar_abrir(std::env::var_os("SystemRoot").as_deref(), caminho)
}

pub(crate) fn montar_abrir(raiz_sistema: Option<&OsStr>, caminho: &Path) -> Resultado<ComandoArquivo> {
    let caminho = exigir_existente(caminho)?;
    if !cfg!(windows) {
        return Err(ErroBanco::com_status("Abrir com o programa padrão só funciona no Windows.", 501));
    }
    Ok(ComandoArquivo { programa: explorer(raiz_sistema), argumentos: entre_aspas("", &caminho) })
}
