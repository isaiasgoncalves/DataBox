//! Testes do crate, sobre o banco de exemplo do crate `amostras` (os números conferidos são os dele).

mod abertura;
mod concorrencia;
mod consulta;
mod exportacao;
mod gravacao;
mod linhas;
mod manutencao;
mod objetos;
mod valores;
mod volumes;

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use comum::{Celula, Chave, Entrada, ErroBanco, Resultado, Valor};
use rusqlite::{Connection, params_from_iter};

use crate::{Banco, Filtro, Operador, Pagina, PedidoPagina};

pub(crate) const TAB_ESPACO: &str = "Tabela com espaço";
pub(crate) const COL_CITADA: &str = "coluna \"citada\"";

pub(crate) const N_CLIENTES: u64 = amostras::N_CLIENTES as u64;
pub(crate) const N_PRODUTOS: u64 = amostras::N_PRODUTOS as u64;
pub(crate) const N_PEDIDOS: u64 = amostras::N_PEDIDOS as u64;
pub(crate) const N_EVENTOS: u64 = amostras::N_EVENTOS as u64;

/// Pasta temporária única por teste (os testes rodam em paralelo), apagada no fim.
pub(crate) struct Pasta {
    pub(crate) caminho: PathBuf,
}

impl Pasta {
    pub(crate) fn nova(rotulo: &str) -> Pasta {
        static N: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let caminho = std::env::temp_dir().join(format!(
            "banco-{rotulo}-{}-{}-{nanos}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&caminho).unwrap();
        Pasta { caminho }
    }

    pub(crate) fn join(&self, nome: &str) -> PathBuf {
        self.caminho.join(nome)
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

/// O banco de exemplo numa pasta própria (os campos caem nesta ordem: o banco fecha antes de a pasta sumir).
pub(crate) struct Exemplo {
    pub(crate) banco: Banco,
    pub(crate) caminho: PathBuf,
    pub(crate) pasta: Pasta,
}

pub(crate) fn caminho_exemplo(pasta: &Pasta) -> PathBuf {
    amostras::criar_banco_exemplo(pasta.join("exemplo.db")).unwrap()
}

pub(crate) fn exemplo(rotulo: &str) -> Exemplo {
    let pasta = Pasta::nova(rotulo);
    let caminho = caminho_exemplo(&pasta);
    Exemplo { banco: Banco::abrir(&caminho, false, false).unwrap(), caminho, pasta }
}

pub(crate) fn exemplo_ro(rotulo: &str) -> Exemplo {
    let pasta = Pasta::nova(rotulo);
    let caminho = caminho_exemplo(&pasta);
    Exemplo { banco: Banco::abrir(&caminho, true, false).unwrap(), caminho, pasta }
}

/// Caminho absoluto canônico, sem o prefixo `\\?\`.
pub(crate) fn canonico(p: &Path) -> PathBuf {
    let c = fs::canonicalize(p).unwrap();
    let texto = c.to_string_lossy().into_owned();
    PathBuf::from(texto.strip_prefix(r"\\?\").unwrap_or(&texto))
}

/// Consulta direta, sem passar pelo Banco, para conferir o que ficou gravado no arquivo.
pub(crate) fn cru(caminho: &Path, sql: &str, params: &[Valor]) -> Vec<Vec<Valor>> {
    let conn = Connection::open(caminho).unwrap();
    let mut stmt = conn.prepare(sql).unwrap();
    let n = stmt.column_count();
    let mut linhas = stmt.query(params_from_iter(params)).unwrap();
    let mut saida = Vec::new();
    while let Some(r) = linhas.next().unwrap() {
        saida.push((0..n).map(|i| Valor::de_ref(r.get_ref(i).unwrap())).collect());
    }
    saida
}

/// Primeira coluna da primeira linha de `cru`, como inteiro.
pub(crate) fn cru_int(caminho: &Path, sql: &str) -> i64 {
    match cru(caminho, sql, &[]).first().and_then(|l| l.first()) {
        Some(Valor::Inteiro(n)) => *n,
        outro => panic!("esperava inteiro, veio {outro:?}"),
    }
}

/// Banco novo com o roteiro.
pub(crate) fn banco_de(pasta: &Pasta, sql: &str, nome: &str) -> PathBuf {
    let caminho = pasta.join(nome);
    let conn = Connection::open(&caminho).unwrap();
    conn.execute_batch(sql).unwrap();
    caminho
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

/// O erro de um resultado que precisava falhar.
pub(crate) fn erro<T: Debug>(r: Resultado<T>) -> ErroBanco {
    match r {
        Ok(v) => panic!("esperava erro, veio {v:?}"),
        Err(e) => e,
    }
}

pub(crate) fn pedido(ajustar: impl FnOnce(&mut PedidoPagina)) -> PedidoPagina {
    let mut p = PedidoPagina::default();
    ajustar(&mut p);
    p
}

/// `banco.linhas(nome, **kw)`.
pub(crate) fn pag(b: &Banco, nome: &str, ajustar: impl FnOnce(&mut PedidoPagina)) -> Pagina {
    b.linhas(nome, &pedido(ajustar)).unwrap()
}

pub(crate) fn pag_r(b: &Banco, nome: &str, ajustar: impl FnOnce(&mut PedidoPagina)) -> Resultado<Pagina> {
    b.linhas(nome, &pedido(ajustar))
}

pub(crate) fn contar(b: &Banco, nome: &str) -> u64 {
    b.contar_linhas(nome, None, &[], None).unwrap()
}

pub(crate) fn filtro(coluna: &str, op: Operador, valor: Entrada) -> Filtro {
    Filtro::novo(coluna, op, valor)
}

pub(crate) fn tx(s: &str) -> Entrada {
    Entrada::Texto(s.to_string())
}

pub(crate) fn vals(pares: &[(&str, Entrada)]) -> Vec<(String, Entrada)> {
    pares.iter().map(|(c, e)| (c.to_string(), e.clone())).collect()
}

pub(crate) fn ct(s: &str) -> Celula {
    Celula::Texto(s.to_string())
}

pub(crate) fn ci(n: i64) -> Celula {
    Celula::Inteiro(n)
}

pub(crate) fn vt(s: &str) -> Valor {
    Valor::Texto(s.to_string())
}

pub(crate) fn pk(pares: &[(&str, Valor)]) -> Chave {
    Chave::Pk(pares.iter().map(|(c, v)| (c.to_string(), v.clone())).collect())
}

pub(crate) fn rowid(c: &Option<Chave>) -> i64 {
    match c {
        Some(Chave::Rowid(i)) => *i,
        outra => panic!("esperava rowid, veio {outra:?}"),
    }
}

/// Os valores da primeira coluna de cada linha da página, como inteiros.
pub(crate) fn ids(p: &Pagina) -> Vec<i64> {
    p.linhas
        .iter()
        .map(|l| match l.valores.first() {
            Some(Celula::Inteiro(i)) => *i,
            outro => panic!("esperava inteiro, veio {outro:?}"),
        })
        .collect()
}

/// Os testes pesados (tabelas de 150 mil a 1 milhão de linhas, estresse com threads) e os que medem tempo
/// rodam um de cada vez: em paralelo, um disputaria a máquina com o outro e as medidas perderiam o sentido.
pub(crate) fn exclusivo() -> std::sync::MutexGuard<'static, ()> {
    static PESADO: Mutex<()> = Mutex::new(());
    PESADO.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn esperar(condicao: impl Fn() -> bool, rotulo: &str) {
    let fim = Instant::now() + Duration::from_secs(20);
    while !condicao() {
        assert!(Instant::now() < fim, "Esperei demais por {rotulo}.");
        thread::sleep(Duration::from_millis(5));
    }
}

/// Registro (log) dos testes: guarda as mensagens para conferir avisos.
struct Registro;

static MENSAGENS: Mutex<Vec<String>> = Mutex::new(Vec::new());

impl log::Log for Registro {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, registro: &log::Record<'_>) {
        MENSAGENS.lock().unwrap_or_else(|e| e.into_inner()).push(registro.args().to_string());
    }

    fn flush(&self) {}
}

static REGISTRO: Registro = Registro;

pub(crate) fn instalar_registro() {
    static UMA_VEZ: Once = Once::new();
    UMA_VEZ.call_once(|| {
        let _ = log::set_logger(&REGISTRO).map(|()| log::set_max_level(log::LevelFilter::Info));
    });
}

pub(crate) fn mensagens_registradas() -> Vec<String> {
    MENSAGENS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
