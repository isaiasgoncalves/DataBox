//! O registro (log) do aplicativo, em `servico::arquivo_log()` (`%LOCALAPPDATA%\DataBox\databox.log`),
//! com rotação: até 1.000.000 de bytes, duas cópias
//! (`databox.log.1`, `databox.log.2`), e a rotação que aguenta outra instância com o mesmo arquivo aberto : no Windows, renomear o arquivo falha enquanto outro processo o segura; aí a
//! rotação fica para daqui a [`ADIAR_ROTACAO`] e a mensagem vai para o arquivo atual (nada se perde).
//!
//! O gancho de pânico ([`instalar_gancho_de_panico`]) registra a mensagem, o lugar e o rastro antes de a thread
//! cair (o `catch_unwind` das tarefas de fundo continua valendo depois dele).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tamanho a partir do qual o arquivo roda.
pub const MAX_BYTES: u64 = 1_000_000;
/// Quantas cópias antigas ficam.
pub const COPIAS: usize = 2;
/// Quanto esperar para tentar de novo uma rotação que falhou.
pub const ADIAR_ROTACAO: Duration = Duration::from_secs(60);

/// Os crates do projeto: registram a partir de INFO; as dependências (wgpu, winit, eframe…) só os avisos. O
/// primeiro é o do executável, que leva o nome do binário (DataBox.exe).
const CRATES_DO_PROJETO: [&str; 16] = [
    "DataBox",
    "editor",
    "servico",
    "banco",
    "prefs",
    "estado",
    "exportar",
    "esquema",
    "ddl",
    "comum",
    "interface",
    "tela_dados",
    "tela_esquema",
    "tela_consulta",
    "tela_exportar",
    "amostras",
];

struct Estado {
    fluxo: Option<File>,
    adiada_ate: Option<Instant>,
}

/// Um arquivo de registro com rotação que não perde mensagens quando outra instância o segura aberto.
pub struct RegistroCompartilhado {
    arquivo: PathBuf,
    max_bytes: u64,
    copias: usize,
    adiar: Duration,
    estado: Mutex<Estado>,
}

impl RegistroCompartilhado {
    /// O arquivo só é aberto na primeira mensagem (duas instâncias abertas não disputam o
    /// arquivo antes de ter o que escrever).
    pub fn novo(arquivo: impl Into<PathBuf>, max_bytes: u64, copias: usize) -> RegistroCompartilhado {
        RegistroCompartilhado {
            arquivo: arquivo.into(),
            max_bytes,
            copias,
            adiar: ADIAR_ROTACAO,
            estado: Mutex::new(Estado { fluxo: None, adiada_ate: None }),
        }
    }

    /// Muda o prazo de uma rotação adiada (testes).
    pub fn com_adiamento(mut self, adiar: Duration) -> RegistroCompartilhado {
        self.adiar = adiar;
        self
    }

    pub fn arquivo(&self) -> &Path {
        &self.arquivo
    }

    fn copia(&self, n: usize) -> PathBuf {
        let mut nome = self.arquivo.clone().into_os_string();
        nome.push(format!(".{n}"));
        PathBuf::from(nome)
    }

    /// Grava uma linha (com o fim de linha), rodando o arquivo antes se ele passaria do limite.
    pub fn escrever(&self, linha: &str) -> std::io::Result<()> {
        let mut est = self.estado.lock().unwrap_or_else(|e| e.into_inner());
        let tamanho = linha.len() as u64 + 1;
        if self.deve_rodar(&est, tamanho) {
            self.rodar(&mut est);
        }
        if est.fluxo.is_none() {
            est.fluxo = Some(abrir_para_acrescentar(&self.arquivo)?);
        }
        let Some(fluxo) = est.fluxo.as_mut() else { return Ok(()) };
        let mut bytes = Vec::with_capacity(linha.len() + 1);
        bytes.extend_from_slice(linha.as_bytes());
        bytes.push(b'\n');
        fluxo.write_all(&bytes)?;
        fluxo.flush()
    }

    /// O `shouldRollover`: o arquivo (com o que a outra instância escreveu) mais a mensagem chega ao limite, e
    /// não há rotação adiada.
    fn deve_rodar(&self, est: &Estado, tamanho: u64) -> bool {
        if self.max_bytes == 0 {
            return false;
        }
        if est.adiada_ate.is_some_and(|ate| Instant::now() < ate) {
            return false;
        }
        let atual = std::fs::metadata(&self.arquivo).map(|m| m.len()).unwrap_or(0);
        atual + tamanho >= self.max_bytes
    }

    /// O `doRollover`: .1 → .2, arquivo → .1. Se algo falha (outra instância segura o arquivo), a rotação
    /// fica para depois e o registro segue no arquivo atual.
    fn rodar(&self, est: &mut Estado) {
        est.fluxo = None;
        if self.copias == 0 {
            if std::fs::OpenOptions::new().write(true).truncate(true).open(&self.arquivo).is_err() {
                est.adiada_ate = Some(Instant::now() + self.adiar);
            }
            return;
        }
        let resultado = (|| -> std::io::Result<()> {
            for n in (1..self.copias).rev() {
                let de = self.copia(n);
                let para = self.copia(n + 1);
                if de.exists() {
                    if para.exists() {
                        std::fs::remove_file(&para)?;
                    }
                    std::fs::rename(&de, &para)?;
                }
            }
            let para = self.copia(1);
            if para.exists() {
                std::fs::remove_file(&para)?;
            }
            std::fs::rename(&self.arquivo, &para)
        })();
        if resultado.is_err() {
            est.adiada_ate = Some(Instant::now() + self.adiar);
        }
    }
}

fn abrir_para_acrescentar(arquivo: &Path) -> std::io::Result<File> {
    if let Some(pasta) = arquivo.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(pasta)?;
    }
    OpenOptions::new().create(true).append(true).open(arquivo)
}

/// O nome do nível no registro ("DEBUG", "INFO", "WARNING", "ERROR").
fn nome_nivel(nivel: log::Level) -> &'static str {
    match nivel {
        log::Level::Error => "ERROR",
        log::Level::Warn => "WARNING",
        log::Level::Info => "INFO",
        log::Level::Debug => "DEBUG",
        log::Level::Trace => "TRACE",
    }
}

/// Uma linha do registro: `2026-10-08 14:03:05,123 INFO [main] editor::app: mensagem`.
pub fn formatar_linha(
    quando: chrono::DateTime<chrono::Local>,
    nivel: log::Level,
    thread: &str,
    alvo: &str,
    mensagem: &str,
) -> String {
    format!("{} {} [{thread}] {alvo}: {mensagem}", quando.format("%Y-%m-%d %H:%M:%S,%3f"), nome_nivel(nivel))
}

/// O nível mínimo registrado para um alvo (`crate::modulo`).
pub fn nivel_do_alvo(alvo: &str) -> log::LevelFilter {
    let primeiro = alvo.split("::").next().unwrap_or(alvo);
    if CRATES_DO_PROJETO.contains(&primeiro) { log::LevelFilter::Info } else { log::LevelFilter::Warn }
}

/// O `log::Log` do aplicativo.
pub struct Registro {
    saida: RegistroCompartilhado,
}

impl log::Log for Registro {
    fn enabled(&self, metadados: &log::Metadata<'_>) -> bool {
        metadados.level() <= nivel_do_alvo(metadados.target())
    }

    fn log(&self, registro: &log::Record<'_>) {
        if !self.enabled(registro.metadata()) {
            return;
        }
        let atual = std::thread::current();
        let thread = atual.name().unwrap_or("sem-nome");
        let linha = formatar_linha(
            chrono::Local::now(),
            registro.level(),
            thread,
            registro.target(),
            &registro.args().to_string(),
        );
        // sem onde avisar (não há console): uma mensagem que não pôde ser gravada se perde em silêncio
        let _ = self.saida.escrever(&linha);
    }

    fn flush(&self) {}
}

/// Liga o registro em `arquivo` (com rotação) e o gancho de pânico. Devolve `false` se já havia um registro.
pub fn iniciar(arquivo: PathBuf) -> bool {
    let registro = Registro { saida: RegistroCompartilhado::novo(arquivo, MAX_BYTES, COPIAS) };
    if log::set_boxed_logger(Box::new(registro)).is_err() {
        return false;
    }
    log::set_max_level(log::LevelFilter::Info);
    instalar_gancho_de_panico();
    true
}

/// Registra todo pânico (mensagem, lugar, thread e rastro) antes de a thread cair.
pub fn instalar_gancho_de_panico() {
    std::panic::set_hook(Box::new(|info| {
        let mensagem = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "pânico sem mensagem".into());
        let lugar = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let atual = std::thread::current();
        let thread = atual.name().unwrap_or("sem-nome");
        let rastro = std::backtrace::Backtrace::force_capture();
        log::error!("Pânico na thread {thread} em {lugar}: {mensagem}\n{rastro}");
    }));
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("editor-registro-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("pasta");
        p
    }

    fn todos(pasta: &Path) -> String {
        let mut texto = String::new();
        for e in std::fs::read_dir(pasta).expect("ler pasta").flatten() {
            if e.file_name().to_string_lossy().starts_with("editor.log") {
                texto.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
            }
        }
        texto
    }

    /// Só no Windows: o arquivo aberto sem FILE_SHARE_DELETE impede o renomear (fora dele, renomear não falha).
    #[cfg(windows)]
    #[test]
    fn registro_continua_com_outra_instancia_segurando_o_arquivo() {
        use std::os::windows::fs::OpenOptionsExt;
        let p = pasta("compartilhado");
        let arquivo = p.join("editor.log");
        std::fs::write(&arquivo, format!("{}\n", "x".repeat(990))).expect("gravar");
        // a outra instância segura o arquivo sem FILE_SHARE_DELETE: renomear falha
        let outra = OpenOptions::new().append(true).share_mode(0x1 | 0x2).open(&arquivo).expect("abrir");
        let registro = RegistroCompartilhado::novo(&arquivo, 1000, 2);
        for i in 0..5 {
            registro.escrever(&format!("mensagem {i} que precisa aparecer")).expect("escrever");
        }
        drop(outra);
        let texto = todos(&p);
        for i in 0..5 {
            assert!(texto.contains(&format!("mensagem {i} que precisa aparecer")), "{texto}");
        }
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn roda_e_guarda_duas_copias() {
        let p = pasta("rotacao");
        let arquivo = p.join("editor.log");
        let registro = RegistroCompartilhado::novo(&arquivo, 100, 2).com_adiamento(Duration::ZERO);
        for i in 0..20 {
            registro.escrever(&format!("linha {i:02} de registro com algum texto")).expect("escrever");
        }
        assert!(arquivo.exists() && p.join("editor.log.1").exists() && p.join("editor.log.2").exists());
        assert!(!p.join("editor.log.3").exists());
        assert!(std::fs::metadata(&arquivo).expect("meta").len() < 100);
        // a última mensagem fica no arquivo atual
        assert!(std::fs::read_to_string(&arquivo).expect("ler").contains("linha 19"));
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn formato_da_linha() {
        use chrono::TimeZone;
        let quando = chrono::Local.with_ymd_and_hms(2026, 10, 8, 14, 3, 5).single().expect("data")
            + chrono::Duration::milliseconds(123);
        assert_eq!(
            formatar_linha(quando, log::Level::Warn, "main", "editor::app", "Algo"),
            "2026-10-08 14:03:05,123 WARNING [main] editor::app: Algo"
        );
    }

    #[test]
    fn dependencias_so_registram_avisos() {
        assert_eq!(nivel_do_alvo("servico::servico"), log::LevelFilter::Info);
        assert_eq!(nivel_do_alvo("DataBox"), log::LevelFilter::Info);
        assert_eq!(nivel_do_alvo("wgpu_core::device"), log::LevelFilter::Warn);
        assert_eq!(nivel_do_alvo("eframe"), log::LevelFilter::Warn);
    }
}
