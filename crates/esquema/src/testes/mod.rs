//! Testes de cada operação, os casos fixos (`referencia.rs`, com `casos_esperados.rs`) e os das peças internas
//! (`unidades.rs`).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use comum::{ErroBanco, Resultado, Valor, q};
use ddl::ColunaDef;
use rusqlite::Connection;

use crate::{Plano, definicoes_tabela, executar};

mod casos_esperados;
mod colunas;
mod objetos;
mod plano;
mod referencia;
mod tabelas;
mod unidades;

pub(crate) const TABELA_ESPACO: &str = "Tabela com espaço";
pub(crate) const COLUNA_ASPAS: &str = "coluna \"citada\"";
/// As `restricoes` vazias de `plano_criar_tabela` (o `&[]` sozinho não diz o tipo).
pub(crate) const SEM_RESTRICOES: &[&str] = &[];

/// Pasta temporária própria do teste, apagada no fim.
pub(crate) struct Pasta(PathBuf);

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Banco de exemplo novo numa pasta só dele e a conexão como a do Banco: autocommit e chaves estrangeiras
/// ligadas. Os campos caem na ordem: conexão, depois a pasta.
pub(crate) struct Ambiente {
    pub conn: Connection,
    pub caminho: PathBuf,
    _pasta: Pasta,
}

pub(crate) fn ambiente() -> Ambiente {
    static CONTADOR: AtomicUsize = AtomicUsize::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
    let pasta = std::env::temp_dir().join(format!("esquema-teste-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    let caminho = amostras::criar_banco_exemplo(pasta.join("exemplo.db")).expect("banco de exemplo");
    let conn = Connection::open(&caminho).expect("abrir o banco");
    conn.execute_batch("PRAGMA foreign_keys = ON").expect("foreign_keys");
    Ambiente { conn, caminho, _pasta: Pasta(pasta) }
}

/// Todas as linhas de uma consulta.
pub(crate) fn valores(conn: &Connection, sql: &str) -> Vec<Vec<Valor>> {
    let mut comando = conn.prepare(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    let n = comando.column_count();
    let linhas = comando
        .query_map([], |r| (0..n).map(|i| r.get::<_, Valor>(i)).collect::<rusqlite::Result<Vec<Valor>>>())
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
    linhas.collect::<rusqlite::Result<Vec<_>>>().unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// O inteiro da primeira coluna da primeira linha.
pub(crate) fn inteiro(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

pub(crate) fn texto(v: &str) -> Valor {
    Valor::Texto(v.to_string())
}

/// (objetos de sqlite_schema, linhas de cada tabela), em ordem.
pub(crate) type Retrato = (Vec<Vec<Valor>>, Vec<(String, Vec<String>)>);

/// Esquema inteiro + todas as linhas de todas as tabelas (sqlite_sequence inclusive).
pub(crate) fn retrato(conn: &Connection) -> Retrato {
    let mut objetos = valores(conn, "SELECT type, name, tbl_name, coalesce(sql, '') FROM sqlite_schema");
    objetos.sort_by_key(|r| format!("{r:?}"));
    let mut dados = Vec::new();
    for linha in valores(conn, "SELECT name FROM sqlite_schema WHERE type = 'table'") {
        let nome = linha[0].texto_exibicao();
        let mut linhas: Vec<String> =
            valores(conn, &format!("SELECT * FROM {}", q(&nome))).iter().map(|r| format!("{r:?}")).collect();
        linhas.sort();
        dados.push((nome, linhas));
    }
    dados.sort();
    (objetos, dados)
}

/// Monta o plano e confere que isso não mudou nada no banco.
pub(crate) fn previa_sem_efeito(conn: &Connection, fabrica: impl FnOnce() -> Resultado<Plano>) -> Plano {
    let antes = retrato(conn);
    let plano = fabrica().unwrap_or_else(|e| panic!("o plano falhou: {e}"));
    assert_eq!(retrato(conn), antes);
    assert!(conn.is_autocommit());
    plano
}

pub(crate) fn aplicar(conn: &Connection, fabrica: impl FnOnce() -> Resultado<Plano>) -> Plano {
    let plano = previa_sem_efeito(conn, fabrica);
    executar(conn, &plano).unwrap_or_else(|e| panic!("executar falhou: {e}"));
    confere_sessao_restaurada(conn);
    plano
}

pub(crate) fn confere_sessao_restaurada(conn: &Connection) {
    assert_eq!(inteiro(conn, "PRAGMA foreign_keys"), 1);
    assert_eq!(inteiro(conn, "PRAGMA legacy_alter_table"), 0);
    assert!(conn.is_autocommit());
    assert!(valores(conn, "PRAGMA foreign_key_check").is_empty());
    assert_eq!(valores(conn, "PRAGMA integrity_check"), vec![vec![texto("ok")]]);
    assert!(valores(conn, "SELECT name FROM sqlite_schema WHERE name LIKE '\\_novo\\_%' ESCAPE '\\'").is_empty());
}

pub(crate) fn definicao(conn: &Connection, tabela: &str, coluna: &str) -> ColunaDef {
    definicoes_tabela(conn, tabela)
        .unwrap_or_else(|e| panic!("{e}"))
        .colunas
        .into_iter()
        .find(|c| c.nome == coluna)
        .unwrap_or_else(|| panic!("coluna {coluna} não achada em {tabela}"))
}

/// Uma linha de `PRAGMA table_xinfo`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Xinfo {
    pub nome: String,
    pub tipo: String,
    pub notnull: i64,
    pub padrao: Option<String>,
    pub pk: i64,
    pub hidden: i64,
}

pub(crate) fn xinfo(conn: &Connection, tabela: &str) -> Vec<Xinfo> {
    let mut comando = conn.prepare(&format!("PRAGMA table_xinfo({})", q(tabela))).unwrap();
    comando
        .query_map([], |r| {
            Ok(Xinfo {
                nome: r.get(1)?,
                tipo: r.get(2)?,
                notnull: r.get(3)?,
                padrao: r.get(4)?,
                pk: r.get(5)?,
                hidden: r.get(6)?,
            })
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

pub(crate) fn nomes_colunas(conn: &Connection, tabela: &str) -> Vec<String> {
    xinfo(conn, tabela).into_iter().map(|r| r.nome).collect()
}

pub(crate) fn coluna_xinfo(conn: &Connection, tabela: &str, coluna: &str) -> Xinfo {
    xinfo(conn, tabela).into_iter().find(|r| r.nome == coluna).unwrap()
}

pub(crate) fn sql_de(conn: &Connection, nome: &str) -> Option<String> {
    conn.query_row("SELECT sql FROM sqlite_schema WHERE name = ?", [nome], |r| r.get(0)).ok()
}

pub(crate) fn contar(conn: &Connection, tabela: &str) -> i64 {
    inteiro(conn, &format!("SELECT count(*) FROM {}", q(tabela)))
}

/// Nomes dos índices de uma tabela.
pub(crate) fn indices(conn: &Connection, tabela: &str) -> BTreeSet<String> {
    valores(conn, &format!("PRAGMA index_list({})", q(tabela))).iter().map(|r| r[1].texto_exibicao()).collect()
}

/// O erro de um resultado que tinha de falhar.
pub(crate) fn erro<T: std::fmt::Debug>(r: Resultado<T>) -> ErroBanco {
    match r {
        Ok(v) => panic!("esperava erro, veio {v:?}"),
        Err(e) => e,
    }
}

/// Falhou, e o texto do erro (mensagem e detalhe) contém o trecho.
pub(crate) fn falha_com<T: std::fmt::Debug>(r: Resultado<T>, trecho: &str) -> ErroBanco {
    let e = erro(r);
    assert!(e.to_string().contains(trecho), "esperava «{trecho}» em «{e}»");
    e
}

/// ColunaDef só com nome e tipo.
pub(crate) fn col(nome: &str, tipo: &str) -> ColunaDef {
    ColunaDef { nome: nome.to_string(), tipo: tipo.to_string(), ..ColunaDef::default() }
}

pub(crate) fn textos(itens: &[&str]) -> Vec<String> {
    itens.iter().map(|s| s.to_string()).collect()
}
