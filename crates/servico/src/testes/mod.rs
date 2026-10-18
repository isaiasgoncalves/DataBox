//! Testes do serviço sobre o banco de exemplo do crate `amostras` (os números conferidos são os dele).

mod api;
mod arquivos;
mod caminhos;
mod concorrencia;
mod exemplo;
mod leitura;
mod pasta;
mod volumes;

use std::cell::RefCell;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use comum::{ErroBanco, Resultado, Valor};
use exportar::{Formato, Opcoes};
use prefs::Preferencias;
use rusqlite::Connection;

use crate::{OrigemExportacao, PedidoExportacao, Progresso, ResumoExportacao, Servico};

thread_local! {
    /// A falha forçada de `esquema.definicoes_tabela` (por thread: os testes rodam em paralelo).
    static FALHA_DEFINICOES: RefCell<Option<ErroBanco>> = const { RefCell::new(None) };
}

pub(crate) fn falha_nas_definicoes() -> Option<ErroBanco> {
    FALHA_DEFINICOES.with(|f| f.borrow().clone())
}

pub(crate) fn fazer_definicoes_falharem(erro: Option<ErroBanco>) {
    FALHA_DEFINICOES.with(|f| *f.borrow_mut() = erro);
}

pub(crate) const N_CLIENTES: u64 = amostras::N_CLIENTES as u64;
pub(crate) const N_PRODUTOS: u64 = amostras::N_PRODUTOS as u64;
pub(crate) const N_PEDIDOS: u64 = amostras::N_PEDIDOS as u64;
pub(crate) const N_EVENTOS: u64 = amostras::N_EVENTOS as u64;

pub(crate) const TABELAS_EXEMPLO: [&str; 7] =
    ["clientes", "produtos", "pedidos", "itens_pedido", "eventos", "Tabela com espaço", "notas"];

/// Pasta temporária única por teste (os testes rodam em paralelo), com o caminho resolvido (sem nomes curtos
/// 8.3), apagada no fim.
pub(crate) struct Pasta {
    pub(crate) caminho: PathBuf,
}

impl Pasta {
    pub(crate) fn nova(rotulo: &str) -> Pasta {
        static N: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let caminho = std::env::temp_dir().join(format!(
            "servico-{rotulo}-{}-{}-{nanos}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&caminho).unwrap();
        Pasta { caminho: canonico(&caminho) }
    }

    pub(crate) fn join(&self, nome: &str) -> PathBuf {
        self.caminho.join(nome)
    }

    /// Nada de temporário de exportação ou de BLOB sobrando ("~…", ".…tmp").
    pub(crate) fn sem_temporarios(&self) {
        let sobras: Vec<String> = fs::read_dir(&self.caminho)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('~') || n.ends_with(".tmp"))
            .collect();
        assert!(sobras.is_empty(), "sobraram temporários: {sobras:?}");
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        for _ in 0..50 {
            if fs::remove_dir_all(&self.caminho).is_ok() || !self.caminho.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Caminho absoluto canônico, sem o prefixo `\\?\`.
pub(crate) fn canonico(p: &Path) -> PathBuf {
    let c = fs::canonicalize(p).unwrap();
    let texto = c.to_string_lossy().into_owned();
    PathBuf::from(texto.strip_prefix(r"\\?\").unwrap_or(&texto))
}

/// Preferências de teste num arquivo da pasta do teste (nunca as do usuário).
pub(crate) fn prefs_em(pasta: &Pasta) -> Preferencias {
    Preferencias::abrir(Some(pasta.join("p.json")))
}

/// Serviço com o banco de exemplo aberto. Os campos caem nesta ordem: o
/// serviço (e o banco) fecha antes de a pasta sumir.
pub(crate) struct Ambiente {
    pub(crate) servico: Servico,
    pub(crate) exemplo: PathBuf,
    pub(crate) pasta: Pasta,
}

impl Ambiente {
    /// O id do banco aberto (o que a tela manda).
    pub(crate) fn id(&self) -> String {
        self.servico.atual().map(|b| b.id().to_string()).unwrap_or_default()
    }

    pub(crate) fn join(&self, nome: &str) -> PathBuf {
        self.pasta.join(nome)
    }

    /// `/api/exportar` com as opções padrão e sem substituir.
    pub(crate) fn exportar(
        &self,
        origem: OrigemExportacao,
        formato: Formato,
        caminho: &Path,
        progresso: Option<Arc<Progresso>>,
    ) -> Resultado<ResumoExportacao> {
        self.exportar_com(origem, formato, caminho, Opcoes::default(), false, progresso)
    }

    pub(crate) fn exportar_com(
        &self,
        origem: OrigemExportacao,
        formato: Formato,
        caminho: &Path,
        opcoes: Opcoes,
        substituir: bool,
        progresso: Option<Arc<Progresso>>,
    ) -> Resultado<ResumoExportacao> {
        let pedido = PedidoExportacao { origem, formato, caminho: caminho.to_path_buf(), opcoes, substituir };
        self.servico.exportar(&self.id(), &pedido, progresso)
    }
}

fn ambiente_com(rotulo: &str, somente_leitura: bool) -> Ambiente {
    let pasta = Pasta::nova(rotulo);
    let exemplo = amostras::criar_banco_exemplo(pasta.join("exemplo.db")).unwrap();
    let servico = Servico::novo(prefs_em(&pasta));
    servico.abrir_banco(&exemplo, somente_leitura, false).unwrap();
    Ambiente { servico, exemplo, pasta }
}

pub(crate) fn ambiente(rotulo: &str) -> Ambiente {
    ambiente_com(rotulo, false)
}

/// O fixture `cliente_leitura`: o exemplo aberto só para leitura.
pub(crate) fn ambiente_leitura(rotulo: &str) -> Ambiente {
    ambiente_com(rotulo, true)
}

/// Serviço com um banco qualquer aberto (os `criar_app(banco_inicial=…)` dos testes de volume).
pub(crate) fn ambiente_de(pasta: Pasta, banco: &Path) -> Ambiente {
    let servico = Servico::novo(prefs_em(&pasta));
    servico.abrir_banco(banco, false, false).unwrap();
    Ambiente { servico, exemplo: banco.to_path_buf(), pasta }
}

/// O erro de um resultado que precisava falhar.
pub(crate) fn erro<T: Debug>(r: Resultado<T>) -> ErroBanco {
    match r {
        Ok(v) => panic!("esperava erro, veio {v:?}"),
        Err(e) => e,
    }
}

/// Falhou com esse status e com uma frase.
pub(crate) fn erro_com<T: Debug>(r: Resultado<T>, status: u16) -> ErroBanco {
    let e = erro(r);
    assert_eq!(e.status, status, "{e:?}");
    assert!(!e.mensagem.is_empty());
    e
}

/// Consulta direta, sem passar pelo serviço, para conferir o que ficou gravado no arquivo.
pub(crate) fn cru(caminho: &Path, sql: &str) -> Vec<Vec<Valor>> {
    let conn = Connection::open(caminho).unwrap();
    let mut stmt = conn.prepare(sql).unwrap();
    let n = stmt.column_count();
    let mut linhas = stmt.query([]).unwrap();
    let mut saida = Vec::new();
    while let Some(r) = linhas.next().unwrap() {
        saida.push((0..n).map(|i| Valor::de_ref(r.get_ref(i).unwrap())).collect());
    }
    saida
}

/// Tabela com n linhas: id, t0..t4 TEXT, n0..n2 INTEGER e `reais` colunas REAL (gerada pelo próprio SQLite).
pub(crate) fn tabela_grande(caminho: &Path, n: u64, nome: &str, reais: usize) -> PathBuf {
    let mut cols = vec!["id INTEGER PRIMARY KEY".to_string()];
    cols.extend((0..5).map(|i| format!("t{i} TEXT")));
    cols.extend((0..3).map(|i| format!("n{i} INTEGER")));
    cols.extend((0..reais).map(|i| format!("r{i} REAL")));
    let mut sel = vec!["i".to_string()];
    sel.extend((0..5).map(|k| format!("'texto ' || ((i * {}) % 100003)", 7919 + k)));
    sel.extend((0..3).map(|k| format!("(i * {}) % 1000000", 104729 + k)));
    sel.extend((0..reais).map(|k| format!("((i * {}) % 100000) / 7.0", 1299709 + k)));
    let conn = Connection::open(caminho).unwrap();
    conn.execute_batch(&format!("CREATE TABLE \"{nome}\" ({})", cols.join(", "))).unwrap();
    conn.execute_batch(&format!(
        "WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < {n}) \
         INSERT INTO \"{nome}\" SELECT {} FROM s",
        sel.join(", ")
    ))
    .unwrap();
    caminho.to_path_buf()
}

/// Os testes pesados (tabelas de 150 mil a 1 milhão de linhas) e os que medem tempo rodam um de cada vez: em
/// paralelo, um disputaria a máquina com o outro e as medidas perderiam o sentido.
pub(crate) fn exclusivo() -> MutexGuard<'static, ()> {
    static PESADO: Mutex<()> = Mutex::new(());
    PESADO.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn esperar(condicao: impl Fn() -> bool, rotulo: &str) {
    let fim = Instant::now() + Duration::from_secs(30);
    while !condicao() {
        assert!(Instant::now() < fim, "Esperei demais por {rotulo}.");
        thread::sleep(Duration::from_millis(2));
    }
}
