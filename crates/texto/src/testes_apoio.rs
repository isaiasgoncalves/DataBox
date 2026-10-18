//! Apoio dos testes: pasta temporária própria de cada teste, apagada no fim.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(crate) struct Pasta(PathBuf);

impl Pasta {
    pub fn nova(nome: &str) -> Pasta {
        static N: AtomicUsize = AtomicUsize::new(0);
        let caminho = std::env::temp_dir().join(format!(
            "texto-teste-{nome}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&caminho);
        fs::create_dir_all(&caminho).unwrap();
        Pasta(caminho)
    }

    pub fn join(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
    }

    pub fn caminho(&self) -> &Path {
        &self.0
    }

    /// Grava `bytes` num arquivo da pasta e devolve o caminho.
    pub fn arquivo(&self, nome: &str, bytes: &[u8]) -> PathBuf {
        let c = self.join(nome);
        fs::write(&c, bytes).unwrap();
        c
    }

    pub fn nomes(&self) -> Vec<String> {
        let mut nomes: Vec<String> =
            fs::read_dir(&self.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        nomes.sort();
        nomes
    }

    /// Nenhum temporário ("~….tmp") esquecido na pasta.
    pub fn sem_temporarios(&self) {
        let sobras: Vec<String> =
            self.nomes().into_iter().filter(|n| n.starts_with('~') || n.ends_with(".tmp")).collect();
        assert!(sobras.is_empty(), "temporários esquecidos: {sobras:?}");
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
