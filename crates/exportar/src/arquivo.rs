//! Arquivos: conferência do destino, temporário ao lado dele, troca no fim e o arquivo por baixo do zip.

use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use comum::{ErroBanco, Resultado, traduzir_io};

/// Contexto dos erros de disco ao gravar (e ao trocar o destino pelo temporário).
pub(crate) const CONTEXTO_GRAVAR: &str = "Não foi possível gravar o arquivo exportado.";
/// Contexto dos erros ao criar o temporário.
pub(crate) const CONTEXTO_CRIAR: &str = "Não foi possível criar o arquivo exportado.";

/// Erro de disco → [`ErroBanco`] com a frase de contexto antes. No Windows, arquivo travado por outro
/// programa (violação de compartilhamento ou de trava) é tratado como falta de permissão.
pub(crate) fn erro_io(e: &io::Error, contexto: &str) -> ErroBanco {
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return ErroBanco {
            mensagem: "Sem permissão para acessar o arquivo. Ele pode estar aberto em outro programa.".to_string(),
            detalhe: Some(e.to_string()),
            status: 403,
            codigo: None,
        }
        .com_contexto(contexto);
    }
    traduzir_io(e, None, Some(contexto))
}

/// Erro do crate `zip` → [`ErroBanco`] (os de disco como [`erro_io`]).
pub(crate) fn erro_zip(e: zip::result::ZipError) -> ErroBanco {
    match e {
        zip::result::ZipError::Io(io) => erro_io(&io, CONTEXTO_GRAVAR),
        outro => traduzir_io(&io::Error::other(outro.to_string()), None, Some(CONTEXTO_GRAVAR)),
    }
}

/// O `_conferir_destino`: caminho absoluto (com "~" expandido), pasta existente e que não seja uma pasta.
pub(crate) fn conferir_destino(destino: &Path) -> Resultado<PathBuf> {
    if destino.as_os_str().to_string_lossy().trim().is_empty() {
        return Err(ErroBanco::novo("Informe o caminho do arquivo de destino."));
    }
    let expandido = expandir_usuario(destino);
    let absoluto = std::path::absolute(&expandido).map_err(|e| traduzir_io(&e, Some(&expandido), None))?;
    // sem a barra no fim
    let caminho: PathBuf = absoluto.components().collect();
    let pasta = caminho.parent().map_or_else(|| caminho.clone(), Path::to_path_buf);
    if !pasta.is_dir() {
        return Err(ErroBanco::com_status(
            format!("A pasta de destino não existe: {}. Escolha outra pasta ou crie esta antes.", pasta.display()),
            404,
        ));
    }
    if caminho.is_dir() {
        return Err(ErroBanco::novo(format!(
            "O destino escolhido é uma pasta, não um arquivo: {}.",
            caminho.display()
        )));
    }
    Ok(caminho)
}

/// "~" e "~\…" (ou "~/…") → pasta da pessoa, como o `os.path.expanduser`.
fn expandir_usuario(caminho: &Path) -> PathBuf {
    let texto = caminho.to_string_lossy();
    let resto = match texto.strip_prefix('~') {
        Some(r) if r.is_empty() || r.starts_with(['/', '\\']) => r.trim_start_matches(['/', '\\']).to_string(),
        _ => return caminho.to_path_buf(),
    };
    let casa = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let pasta = std::env::var_os("HOMEPATH")?;
            let mut p = PathBuf::from(drive);
            p.push(pasta);
            Some(p)
        })
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from));
    match casa {
        Some(c) if resto.is_empty() => c,
        Some(c) => c.join(resto),
        None => caminho.to_path_buf(),
    }
}

/// Sufixo de nome com 8 letras, números ou "_" (como os do `tempfile.mkstemp`).
fn sufixo_aleatorio() -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let agora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut v = RandomState::new().hash_one((std::process::id(), n, agora));
    const ALFABETO: &[u8; 37] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut s = String::with_capacity(8);
    for _ in 0..8 {
        s.push(char::from(ALFABETO[(v % 37) as usize]));
        v /= 37;
    }
    s
}

/// Cria o temporário na mesma pasta do destino ("~saida.xlsx.k3j9x0ab.tmp"), aberto para gravar.
pub(crate) fn criar_temporario(destino: &Path) -> Resultado<(PathBuf, File)> {
    let pasta = destino.parent().unwrap_or(destino);
    let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    criar_unico(pasta, &format!("~{nome}."), ".tmp", false).map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            ErroBanco::com_status(format!("Sem permissão para gravar na pasta {}.", pasta.display()), 403)
                .detalhe(e.to_string())
        } else {
            erro_io(&e, CONTEXTO_CRIAR)
        }
    })
}

/// Começo do nome dos arquivos de trabalho.
pub(crate) const PREFIXO_TRABALHO: &str = "DataBox-exportar-";

/// Arquivo de trabalho (o CSV de cada tabela antes de entrar no .zip) na `pasta` dada ou, sem ela, na pasta
/// temporária do sistema.
///
/// Some sozinho: no
/// Windows, é aberto com FILE_FLAG_DELETE_ON_CLOSE, e o próprio sistema o apaga quando o handle fecha, mesmo
/// que o processo morra no meio da exportação sem passar por nenhum `Drop` (a janela fechada com a exportação
/// rodando numa thread de fundo, um travamento, o Gerenciador de Tarefas). Fora do Windows, o nome é apagado
/// logo depois de criado (o arquivo continua aberto). Quem usa ainda chama [`remover`] no fim, de reserva.
pub(crate) fn criar_temporario_de_trabalho(pasta: Option<&Path>) -> Resultado<(PathBuf, File)> {
    let pasta = pasta.map_or_else(std::env::temp_dir, Path::to_path_buf);
    let (caminho, arquivo) =
        criar_unico(&pasta, PREFIXO_TRABALHO, ".tmp", true).map_err(|e| erro_io(&e, CONTEXTO_CRIAR))?;
    #[cfg(not(windows))]
    remover(&caminho);
    Ok((caminho, arquivo))
}

fn criar_unico(pasta: &Path, prefixo: &str, sufixo: &str, apagar_ao_fechar: bool) -> io::Result<(PathBuf, File)> {
    let mut ultimo = None;
    for _ in 0..100 {
        let caminho = pasta.join(format!("{prefixo}{}{sufixo}", sufixo_aleatorio()));
        let mut opcoes = OpenOptions::new();
        opcoes.read(true).write(true).create_new(true);
        if apagar_ao_fechar {
            marcar_apagar_ao_fechar(&mut opcoes);
        }
        match opcoes.open(&caminho) {
            Ok(arquivo) => return Ok((caminho, arquivo)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => ultimo = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(ultimo.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)))
}

/// No Windows: o sistema apaga o arquivo quando o último handle dele fecha.
#[cfg(windows)]
fn marcar_apagar_ao_fechar(opcoes: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    opcoes.custom_flags(FILE_FLAG_DELETE_ON_CLOSE).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
}

/// Fora do Windows, quem cria apaga o nome logo depois (ver [`criar_temporario_de_trabalho`]).
#[cfg(not(windows))]
fn marcar_apagar_ao_fechar(_opcoes: &mut OpenOptions) {}

/// Apaga sem reclamar (o arquivo pode já não existir).
pub(crate) fn remover(caminho: &Path) {
    let _ = fs::remove_file(caminho);
}

/// O arquivo por baixo do `ZipWriter`.
///
/// Depois de [`ArquivoZip::parar`] (ou do primeiro erro de disco) ele não grava mais nada: finge gravar e
/// posicionar, com a posição que o arquivo teria. Assim o `ZipWriter`, que tenta fechar o pacote quando é
/// descartado, termina em silêncio num arquivo que vai ser apagado.
pub(crate) struct ArquivoZip {
    arquivo: File,
    parado: Arc<AtomicBool>,
    posicao: u64,
    fim: u64,
}

impl ArquivoZip {
    /// O arquivo e a chave que o para (quem a guarda para o arquivo sem precisar dele).
    pub fn novo(arquivo: File) -> (ArquivoZip, Arc<AtomicBool>) {
        let parado = Arc::new(AtomicBool::new(false));
        (ArquivoZip { arquivo, parado: Arc::clone(&parado), posicao: 0, fim: 0 }, parado)
    }

    fn parado(&self) -> bool {
        self.parado.load(Ordering::Relaxed)
    }

    fn parar(&self) {
        self.parado.store(true, Ordering::Relaxed);
    }

    fn avancar(&mut self, n: u64) {
        self.posicao = self.posicao.saturating_add(n);
        self.fim = self.fim.max(self.posicao);
    }

    /// Fecha o arquivo depois do `ZipWriter::finish`. Parado no meio é erro (o pacote não está inteiro).
    pub fn terminar(mut self) -> io::Result<()> {
        if self.parado() {
            return Err(io::Error::other("a gravação do arquivo compactado foi interrompida"));
        }
        self.arquivo.flush()
    }
}

impl Write for ArquivoZip {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.parado() {
            self.avancar(buf.len() as u64);
            return Ok(buf.len());
        }
        match self.arquivo.write(buf) {
            Ok(n) => {
                self.avancar(n as u64);
                Ok(n)
            }
            Err(e) => {
                self.parar();
                Err(e)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.parado() {
            return Ok(());
        }
        let r = self.arquivo.flush();
        if r.is_err() {
            self.parar();
        }
        r
    }
}

impl Seek for ArquivoZip {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if self.parado() {
            let nova = match pos {
                SeekFrom::Start(n) => Some(n),
                SeekFrom::Current(d) => self.posicao.checked_add_signed(d),
                SeekFrom::End(d) => self.fim.checked_add_signed(d),
            }
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
            self.posicao = nova;
            self.fim = self.fim.max(nova);
            return Ok(nova);
        }
        match self.arquivo.seek(pos) {
            Ok(p) => {
                self.posicao = p;
                self.fim = self.fim.max(p);
                Ok(p)
            }
            Err(e) => {
                self.parar();
                Err(e)
            }
        }
    }
}
