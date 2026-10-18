//! Trava entre processos no `preferencias.json.lock`.
//!
//! O [`File::try_lock`] do Rust usa `LockFileEx` exclusivo sobre o arquivo inteiro: uma instância exclui a outra.
//! O arquivo `.lock` fica, vazio, ao lado.
//!
//! **A vez.** Quem espera tenta de [`INTERVALO`] em [`INTERVALO`], e quem segura a trava solta e trava de novo em
//! microssegundos quando grava várias mudanças seguidas: sem mais nada, quem espera quase nunca acerta o intervalo e
//! fica de fora até a outra instância acabar tudo (ou até o prazo, quando grava sem a trava). Por isso quem espera
//! "bate na porta" a cada tentativa, mudando a data de modificação do `.lock` (a trava de faixa do Windows não
//! impede mudar a data); quem segura confere a data ao soltar ([`TravaEntreProcessos::soltar`]) e, se alguém bateu,
//! espera [`CORTESIA`] antes de travar de novo. Sem ninguém esperando, nada muda nem atrasa.

use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

/// O intervalo entre as tentativas de quem espera a trava.
const INTERVALO: Duration = Duration::from_millis(10);
/// Quanto quem acabou de soltar a trava espera antes de travar de novo, quando outra instância bateu enquanto ela
/// estava presa: algumas tentativas de quem espera, com folga para a máquina carregada.
pub(crate) const CORTESIA: Duration = Duration::from_millis(50);

/// Segura a trava enquanto existir; solta ao sair de escopo (e o sistema solta se o processo morrer).
pub(crate) struct TravaEntreProcessos {
    arquivo: Option<File>,
    travado: bool,
    /// A data de modificação do `.lock` logo depois de travar: mudou até soltar = alguém bateu.
    marca: Option<SystemTime>,
}

/// A data de modificação do arquivo aberto (None se o sistema não disser).
fn modificado(arquivo: &File) -> Option<SystemTime> {
    arquivo.metadata().and_then(|m| m.modified()).ok()
}

impl TravaEntreProcessos {
    /// Espera a trava por até `tempo_maximo`; sem ela (arquivo inacessível ou outra instância presa), segue
    /// sem trava.
    pub(crate) fn adquirir(caminho: &Path, tempo_maximo: Duration) -> TravaEntreProcessos {
        let aberto = (|| {
            if let Some(pasta) = caminho.parent() {
                fs::create_dir_all(pasta)?;
            }
            OpenOptions::new().read(true).write(true).create(true).truncate(false).open(caminho)
        })();
        let arquivo = match aberto {
            Ok(a) => a,
            Err(e) => {
                log::warn!("Sem a trava das preferências ({}): {}", caminho.display(), e);
                return TravaEntreProcessos { arquivo: None, travado: false, marca: None };
            }
        };
        let fim = Instant::now() + tempo_maximo;
        loop {
            if arquivo.try_lock().is_ok() {
                let marca = modificado(&arquivo);
                return TravaEntreProcessos { arquivo: Some(arquivo), travado: true, marca };
            }
            // bate na porta: quem segura a trava vê a data mudar ao soltar e cede a vez
            let _ = arquivo.set_modified(SystemTime::now());
            if Instant::now() > fim {
                log::warn!(
                    "A trava das preferências não veio em {:.0} s; gravando sem ela.",
                    tempo_maximo.as_secs_f64()
                );
                return TravaEntreProcessos { arquivo: Some(arquivo), travado: false, marca: None };
            }
            thread::sleep(INTERVALO);
        }
    }

    #[cfg(test)]
    pub(crate) fn travado(&self) -> bool {
        self.travado
    }

    /// Solta a trava e diz se outra instância bateu enquanto ela estava presa (e então quem soltou deve esperar
    /// [`CORTESIA`] antes de travar de novo).
    pub(crate) fn soltar(self) -> bool {
        match (&self.arquivo, self.marca) {
            (Some(arquivo), Some(marca)) if self.travado => modificado(arquivo).is_some_and(|agora| agora != marca),
            _ => false,
        }
        // o Drop solta a trava
    }
}

impl Drop for TravaEntreProcessos {
    fn drop(&mut self) {
        if let Some(arquivo) = &self.arquivo
            && self.travado
        {
            let _ = arquivo.unlock();
        }
    }
}
