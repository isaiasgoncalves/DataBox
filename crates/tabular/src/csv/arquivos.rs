//! Arquivos: erros de disco com a frase para a pessoa, o temporário ao lado do destino e a troca no fim. É o
//! mesmo cuidado do `exportar` (temporário "~nome.xxxxxxxx.tmp" na pasta do destino, `std::fs::rename` por
//! cima, que no Windows substitui o destino) e das `prefs` (tentar a troca de novo por uns instantes quando
//! outro programa está lendo o arquivo).

use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use comum::{ErroBanco, Resultado, traduzir_io};

pub(crate) const CONTEXTO_LER: &str = "Não foi possível ler o arquivo.";
pub(crate) const CONTEXTO_GRAVAR: &str = "Não foi possível gravar o arquivo.";

/// Quantas vezes se tenta trocar o arquivo, e o intervalo entre as tentativas.
const TENTATIVAS_DA_TROCA: u32 = 4;
const ESPERA_DA_TROCA: Duration = Duration::from_millis(50);

/// Erro de disco → [`ErroBanco`] com a frase de contexto antes. No Windows, arquivo travado por outro programa
/// (violação de compartilhamento ou de trava) é tratado como falta de permissão.
pub(crate) fn erro_io(e: &io::Error, caminho: Option<&Path>, contexto: &str) -> ErroBanco {
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return ErroBanco::com_status(
            "Sem permissão para acessar o arquivo. Ele pode estar aberto em outro programa.",
            403,
        )
        .detalhe(e.to_string())
        .com_contexto(contexto);
    }
    traduzir_io(e, caminho, Some(contexto))
}

/// Sufixo de nome com 8 letras, números ou "_".
fn sufixo_aleatorio() -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let agora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut v = RandomState::new().hash_one((std::process::id(), n, agora));
    const ALFABETO: &[u8; 37] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut s = String::with_capacity(8);
    for _ in 0..8 {
        s.push(char::from(ALFABETO.get((v % 37) as usize).copied().unwrap_or(b'x')));
        v /= 37;
    }
    s
}

/// Confere o destino: a pasta existe, o destino não é uma pasta, e o arquivo, se já existe, aceita gravação.
pub(crate) fn conferir_destino(destino: &Path) -> Resultado<()> {
    let pasta = destino.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !pasta.is_dir() {
        return Err(ErroBanco::com_status(
            format!("A pasta de destino não existe: {}. Escolha outra pasta ou crie esta antes.", pasta.display()),
            404,
        ));
    }
    if destino.is_dir() {
        return Err(ErroBanco::novo(format!(
            "O destino escolhido é uma pasta, não um arquivo: {}.",
            destino.display()
        )));
    }
    if let Ok(meta) = fs::metadata(destino)
        && meta.permissions().readonly()
    {
        return Err(ErroBanco::com_status(
            format!(
                "O arquivo {} é somente leitura; tire essa marca nas propriedades dele ou salve com outro nome.",
                destino.display()
            ),
            403,
        ));
    }
    Ok(())
}

/// Cria o temporário na mesma pasta do destino ("~saida.csv.k3j9x0ab.tmp"), aberto para gravar.
pub(crate) fn criar_temporario(destino: &Path) -> Resultado<(PathBuf, File)> {
    let pasta = destino.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut ultimo = None;
    for _ in 0..100 {
        let caminho = pasta.join(format!("~{nome}.{}.tmp", sufixo_aleatorio()));
        match OpenOptions::new().write(true).create_new(true).open(&caminho) {
            Ok(arquivo) => return Ok((caminho, arquivo)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => ultimo = Some(e),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                return Err(ErroBanco::com_status(
                    format!("Sem permissão para gravar na pasta {}.", pasta.display()),
                    403,
                )
                .detalhe(e.to_string()));
            }
            Err(e) => return Err(erro_io(&e, Some(&caminho), CONTEXTO_GRAVAR)),
        }
    }
    let e = ultimo.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists));
    Err(erro_io(&e, None, CONTEXTO_GRAVAR))
}

fn e_permissao(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// Põe o temporário no lugar do destino. No Windows a troca falha enquanto outro processo lê o destino, e
/// costuma passar em milissegundos: tenta mais algumas vezes antes de desistir.
pub(crate) fn trocar(temporario: &Path, destino: &Path) -> io::Result<()> {
    let mut tentativa = 0;
    loop {
        match fs::rename(temporario, destino) {
            Ok(()) => return Ok(()),
            Err(e) if tentativa < TENTATIVAS_DA_TROCA && e_permissao(&e) => {
                tentativa += 1;
                thread::sleep(ESPERA_DA_TROCA);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Apaga sem reclamar (o arquivo pode já não existir).
pub(crate) fn remover(caminho: &Path) {
    let _ = fs::remove_file(caminho);
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::testes_apoio::Pasta;

    #[test]
    fn temporario_fica_na_pasta_do_destino_com_nome_unico() {
        let pasta = Pasta::nova("temp");
        let destino = pasta.join("dados.csv");
        let (a, _fa) = criar_temporario(&destino).unwrap();
        let (b, _fb) = criar_temporario(&destino).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.parent(), destino.parent());
        let nome = a.file_name().unwrap().to_string_lossy().into_owned();
        assert!(nome.starts_with("~dados.csv.") && nome.ends_with(".tmp"), "{nome}");
    }

    #[test]
    fn troca_substitui_e_remover_nao_reclama() {
        let pasta = Pasta::nova("troca");
        let destino = pasta.join("a.csv");
        std::fs::write(&destino, b"velho").unwrap();
        let (tmp, mut f) = criar_temporario(&destino).unwrap();
        io::Write::write_all(&mut f, b"novo").unwrap();
        drop(f);
        trocar(&tmp, &destino).unwrap();
        assert_eq!(std::fs::read(&destino).unwrap(), b"novo");
        assert!(!tmp.exists());
        remover(&tmp);
    }

    #[test]
    fn conferencias_do_destino() {
        let pasta = Pasta::nova("destino");
        assert!(conferir_destino(&pasta.join("novo.csv")).is_ok());
        let e = conferir_destino(&pasta.join("nao_existe").join("x.csv")).unwrap_err();
        assert_eq!(e.status, 404);
        let sub = pasta.join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert!(conferir_destino(&sub).unwrap_err().mensagem.contains("é uma pasta"));
        let ro = pasta.join("ro.csv");
        std::fs::write(&ro, b"x").unwrap();
        let mut p = std::fs::metadata(&ro).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&ro, p).unwrap();
        let e = conferir_destino(&ro).unwrap_err();
        assert_eq!(e.status, 403);
        assert!(e.mensagem.contains("somente leitura"));
        let mut p = std::fs::metadata(&ro).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(&ro, p).unwrap();
    }

    #[test]
    fn erro_de_arquivo_inexistente_tem_frase() {
        let e = erro_io(&io::Error::from(io::ErrorKind::NotFound), Some(Path::new("C:\\x\\a.csv")), CONTEXTO_LER);
        assert_eq!(e.status, 404);
        assert!(
            e.mensagem.starts_with("Não foi possível ler o arquivo. Arquivo ou pasta não encontrado: C:\\x\\a.csv")
        );
    }
}
