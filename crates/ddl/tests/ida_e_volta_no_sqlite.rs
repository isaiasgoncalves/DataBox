//! Ida e volta executada no SQLite.
//!
//! Os testes de src/testes.rs conferem o texto montado contra o esperado e analisando de novo com o mesmo
//! tokenizador; estes respondem à pergunta "o SQLite aceita e aplica?": comparam os PRAGMAs table_xinfo,
//! foreign_key_list, index_list e table_list do original e da volta, e repetem INSERTs e leituras de
//! sqlite_sequence e table_xinfo.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ddl::{ColunaDef, Referencia, analisar_create_table, montar_coluna, montar_create_table};
use rusqlite::Connection;
use rusqlite::types::Value;

/// `comum::q` (o ddl não o reexporta e este teste não depende de comum).
fn q(nome: &str) -> String {
    format!("\"{}\"", nome.replace('"', "\"\""))
}

/// Banco em memória sem verificar chaves estrangeiras (o SQLite do rusqlite `bundled` liga a verificação por
/// padrão).
fn em_memoria() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    conn
}

/// Pasta temporária só deste teste (os testes rodam em paralelo); apagada no Drop.
struct Pasta(PathBuf);

impl Pasta {
    fn nova(nome: &str) -> Pasta {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let caminho = std::env::temp_dir().join(format!("ddl-sqlite-{nome}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&caminho);
        std::fs::create_dir_all(&caminho).unwrap();
        Pasta(caminho)
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Linhas = Vec<Vec<Value>>;

fn linhas(conn: &Connection, sql: &str) -> Linhas {
    let mut comando = conn.prepare(sql).unwrap();
    let n = comando.column_count();
    comando
        .query_map([], |r| (0..n).map(|i| r.get::<_, Value>(i)).collect::<Result<Vec<_>, _>>())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn texto(v: &Value) -> &str {
    match v {
        Value::Text(s) => s,
        outro => panic!("esperava texto, veio {outro:?}"),
    }
}

/// Os PRAGMAs comparados: (table_xinfo, foreign_key_list, index_list sem os índices criados à parte e sem
/// a coluna seq, em ordem, e o tipo/ncol/wr/strict do table_list).
fn pragmas(conn: &Connection, tabela: &str) -> (Linhas, Linhas, Linhas, Linhas) {
    let xinfo = linhas(conn, &format!("PRAGMA table_xinfo({})", q(tabela)));
    let fks = linhas(conn, &format!("PRAGMA foreign_key_list({})", q(tabela)));
    let mut indices: Linhas = linhas(conn, &format!("PRAGMA index_list({})", q(tabela)))
        .into_iter()
        .filter(|r| texto(&r[3]) != "c")
        .map(|r| r[1..].to_vec())
        .collect();
    indices.sort_by_key(|r| format!("{r:?}"));
    let info = linhas(conn, "PRAGMA table_list")
        .into_iter()
        .filter(|r| texto(&r[1]) == tabela)
        .map(|r| r[2..6].to_vec())
        .collect();
    (xinfo, fks, indices, info)
}

fn ida_e_volta(sql: &str) -> String {
    let d = analisar_create_table(sql).unwrap();
    let colunas: Vec<String> = d.colunas.iter().map(|c| montar_coluna(&c.como_dict()).unwrap()).collect();
    montar_create_table(&d.nome, &colunas, &d.restricoes, &d.sufixo).unwrap()
}

#[test]
fn ida_e_volta_de_todas_as_tabelas_do_exemplo() {
    let pasta = Pasta::nova("exemplo");
    let caminho = amostras::criar_banco_exemplo(pasta.0.join("exemplo.db")).unwrap();
    let original = Connection::open(&caminho).unwrap();
    let comuns: BTreeSet<String> = linhas(&original, "PRAGMA table_list")
        .iter()
        .filter(|r| texto(&r[2]) == "table" && texto(&r[0]) == "main")
        .map(|r| texto(&r[1]).to_string())
        .collect();
    let tabelas: Vec<(String, String)> =
        linhas(&original, "SELECT name, sql FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
            .iter()
            .map(|r| (texto(&r[0]).to_string(), texto(&r[1]).to_string()))
            .filter(|(nome, _)| comuns.contains(nome))
            .collect();
    let nomes: BTreeSet<&str> = tabelas.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        ["clientes", "produtos", "pedidos", "itens_pedido", "eventos", "Tabela com espaço"]
            .iter()
            .all(|n| nomes.contains(n))
    );
    for (nome, sql) in &tabelas {
        let novo = em_memoria();
        novo.execute(&ida_e_volta(sql), []).unwrap();
        assert_eq!(pragmas(&novo, nome), pragmas(&original, nome), "{nome}");
    }
}

#[test]
fn ida_e_volta_das_formas_raras() {
    let citado = "\"raro \"\"x\"\"\"";
    let sql = format!(
        "CREATE TABLE {citado} (\n{}",
        r#"        id INTEGER PRIMARY KEY DESC,
        a VARCHAR(255) CONSTRAINT nn NOT NULL ON CONFLICT REPLACE DEFAULT '',
        b DOUBLE PRECISION DEFAULT -1.5 CHECK (b <> 0 /* nunca zero; */),
        c TEXT REFERENCES pai(id) ON DELETE SET NULL ON UPDATE CASCADE MATCH FULL DEFERRABLE INITIALLY DEFERRED,
        d TEXT GENERATED ALWAYS AS (a || 'x') STORED,
        e DEFAULT (datetime('now')) COLLATE NOCASE,
        f INT CONSTRAINT u UNIQUE,
        g TEXT CHECK (g <> 'a' -- comentário no fim
        ),
        CONSTRAINT ck CHECK (b > -10), UNIQUE (a, b), FOREIGN KEY (a, b) REFERENCES outra(x, y)
    )"#
    );
    let o = em_memoria();
    let n = em_memoria();
    o.execute(&sql, []).unwrap();
    n.execute(&ida_e_volta(&sql), []).unwrap();
    assert_eq!(pragmas(&n, "raro \"x\""), pragmas(&o, "raro \"x\""));
    // "INTEGER PRIMARY KEY DESC" não vira apelido do rowid: a peculiaridade tem de sobreviver
    let inserir = format!("INSERT INTO {citado} (id, a, b) VALUES ('texto', 'a', 1)");
    o.execute(&inserir, []).unwrap();
    n.execute(&inserir, []).unwrap();
    let id: String = n.query_row(&format!("SELECT id FROM {citado}"), [], |r| r.get(0)).unwrap();
    assert_eq!(id, "texto");
}

#[test]
fn ida_e_volta_com_comentario_de_linha_no_meio_de_restricao_tipo_e_sufixo() {
    // Antes, extra, tipo e sufixo eram postos numa linha só com o "--" dentro: o comentário engolia o
    // ON DELETE CASCADE, o AUTOINCREMENT, o (80) NOT NULL UNIQUE e o WITHOUT ROWID.
    let sql = "CREATE TABLE registro (
        id INTEGER CONSTRAINT pk_registro PRIMARY KEY -- chave
            AUTOINCREMENT,
        categoria_id INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) -- dono
            ON DELETE CASCADE,
        nome VARCHAR -- tamanho
            (80) NOT NULL UNIQUE,
        rotulo TEXT CONSTRAINT d DEFAULT 'dois  espaços' -- o texto não muda
            NOT NULL,
        ref INTEGER REFERENCES categoria (id) DEFERRABLE -- adiada
            INITIALLY DEFERRED
    )";
    let sufixo = "CREATE TABLE cfg (k TEXT PRIMARY KEY, v ANY) STRICT, -- sem rowid\nWITHOUT ROWID";
    let o = em_memoria();
    let n = em_memoria();
    for conn in [&o, &n] {
        conn.execute("CREATE TABLE categoria (id INTEGER PRIMARY KEY)", []).unwrap();
    }
    o.execute(sql, []).unwrap();
    let volta = ida_e_volta(sql);
    assert!(!volta.contains("--"));
    n.execute(&volta, []).unwrap();
    let (antes, depois) = (pragmas(&o, "registro"), pragmas(&n, "registro"));
    // o comentário do tipo vira /* */: compara o table_xinfo sem a coluna do tipo
    let sem_tipo =
        |xinfo: &Linhas| -> Linhas { xinfo.iter().map(|r| r[..2].iter().chain(&r[3..]).cloned().collect()).collect() };
    assert_eq!(sem_tipo(&depois.0), sem_tipo(&antes.0));
    assert_eq!((&depois.1, &depois.2, &depois.3), (&antes.1, &antes.2, &antes.3));
    assert_eq!(texto(&depois.0[2][2]), "VARCHAR /* tamanho */ (80)");
    let cascata = depois.1.iter().find(|fk| texto(&fk[3]) == "categoria_id").map(|fk| texto(&fk[6]).to_string());
    assert_eq!(cascata.as_deref(), Some("CASCADE"));
    n.execute("INSERT INTO registro (categoria_id, nome) VALUES (NULL, 'a')", []).unwrap();
    assert_eq!(linhas(&n, "SELECT name FROM sqlite_sequence"), [[Value::Text("registro".into())]]);
    let rotulo: String = n.query_row("SELECT rotulo FROM registro", [], |r| r.get(0)).unwrap();
    assert_eq!(rotulo, "dois  espaços");
    o.execute(sufixo, []).unwrap();
    n.execute(&ida_e_volta(sufixo), []).unwrap();
    assert_eq!(pragmas(&n, "cfg"), pragmas(&o, "cfg"));
}

#[test]
fn comentario_de_linha_numa_expressao_nao_engole_o_resto() {
    let sql = montar_coluna(&ColunaDef {
        nome: "c".into(),
        tipo: "INTEGER".into(),
        verificacao: Some("c > 0 -- positivo".into()),
        padrao: Some("1".into()),
        ..ColunaDef::default()
    })
    .unwrap();
    let conn = em_memoria();
    conn.execute(&montar_create_table("t", &[sql], &[], "").unwrap(), []).unwrap();
    assert_eq!(linhas(&conn, "PRAGMA table_xinfo(t)")[0][4], Value::Text("1".into()));
}

/// Um DEFAULT com comentário de linha não engole a colação, a chave estrangeira nem o ")" de uma expressão
/// (topo do lib.rs).
#[test]
fn padrao_com_comentario_de_linha_nao_engole_o_resto_da_coluna() {
    let a = montar_coluna(&ColunaDef {
        nome: "a".into(),
        tipo: "TEXT".into(),
        padrao: Some("'x' -- obs".into()),
        colacao: Some("NOCASE".into()),
        referencia: Some(Referencia {
            tabela: "p".into(),
            ao_excluir: Some("cascade".into()),
            ..Referencia::default()
        }),
        ..ColunaDef::default()
    })
    .unwrap();
    let b =
        montar_coluna(&ColunaDef { nome: "b".into(), padrao: Some("1 + 2 -- soma".into()), ..ColunaDef::default() })
            .unwrap();
    let conn = em_memoria();
    conn.execute("CREATE TABLE p (id TEXT PRIMARY KEY)", []).unwrap();
    conn.execute(&montar_create_table("t", &[a, b], &[], "").unwrap(), []).unwrap();
    let xinfo = linhas(&conn, "PRAGMA table_xinfo(t)");
    assert_eq!(xinfo[0][4], Value::Text("'x'".into()));
    let fks = linhas(&conn, "PRAGMA foreign_key_list(t)");
    assert_eq!(fks.len(), 1);
    assert_eq!((texto(&fks[0][2]), texto(&fks[0][3]), texto(&fks[0][6])), ("p", "a", "CASCADE"));
    conn.execute("INSERT INTO t DEFAULT VALUES", []).unwrap();
    conn.execute("INSERT INTO t (a) VALUES ('X')", []).unwrap();
    assert_eq!(
        linhas(&conn, "SELECT a, b FROM t ORDER BY rowid"),
        [[Value::Text("x".into()), Value::Integer(3)], [Value::Text("X".into()), Value::Integer(3)]]
    );
    // COLLATE NOCASE valeu: 'x' e 'X' são iguais na coluna
    let iguais: i64 = conn.query_row("SELECT count(*) FROM t WHERE a = 'x'", [], |r| r.get(0)).unwrap();
    assert_eq!(iguais, 2);
    // e o que o SQLite guardou volta com a colação e a referência
    let guardado: String = conn.query_row("SELECT sql FROM sqlite_schema WHERE name = 't'", [], |r| r.get(0)).unwrap();
    let d = analisar_create_table(&guardado).unwrap();
    assert_eq!(d.colunas[0].colacao.as_deref(), Some("NOCASE"));
    assert_eq!(d.colunas[0].referencia.as_ref().map(|r| r.tabela.as_str()), Some("p"));
}
