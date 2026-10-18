//! Testes do tokenizador, do analisador e da montagem, sem executar nada no SQLite:
//! - o sqlite_schema do `amostras/dados/exemplo.db` é lido direto do arquivo ([`ler_sqlite_schema`], pelo
//!   formato documentado do SQLite), e o SQL das tabelas está copiado aqui e conferido contra ele;
//! - a "ida e volta" é conferida contra o texto esperado para cada CREATE TABLE e analisando de novo o que
//!   voltou.
//!
//! A parte que executa no SQLite (os PRAGMAs do original e da volta, os INSERTs, o sqlite_sequence) está em
//! `tests/ida_e_volta_no_sqlite.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;

fn tipos_textos(sql: &str) -> Vec<(&'static str, String)> {
    tokenizar(sql)
        .unwrap()
        .into_iter()
        .filter(|t| t.tipo != TipoToken::Espaco)
        .map(|t| (t.tipo.como_str(), t.texto))
        .collect()
}

fn pares(lista: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    lista.iter().map(|(a, b)| (*a, b.to_string())).collect()
}

fn conjunto(nomes: &[&str]) -> BTreeSet<String> {
    nomes.iter().map(|n| n.to_string()).collect()
}

// ---------------------------------------------------------------------------------------------------
// Tokenizador

#[test]
fn tokens_cobrem_o_texto_original_com_posicoes_exatas() {
    let sql = "SELECT \"a\"\"b\", 'x;y''z', [col 1], `c``d` -- fim; de linha\n\
               /* bloco ; com 'aspas' */ FROM t WHERE v <> 1e5 AND w != .5 OR h = 0x1F; ";
    let tokens = tokenizar(sql).unwrap();
    assert_eq!(tokens.iter().map(|t| t.texto.as_str()).collect::<String>(), sql);
    for t in &tokens {
        assert_eq!(&sql[t.inicio..t.fim], t.texto);
    }
}

#[test]
fn casos_dificeis_do_tokenizador() {
    let sql = "SELECT \"a\"\"b\", 'x;y''z', [col 1], `c``d` -- fim; de linha\n\
               /* bloco ; */ FROM t WHERE v <> 1e5 AND w != .5 OR h = 0x1F OR k = 3.25E-2 OR b = X'00ff';";
    assert_eq!(
        tipos_textos(sql),
        pares(&[
            ("palavra", "SELECT"),
            ("ident", "\"a\"\"b\""),
            ("op", ","),
            ("string", "'x;y''z'"),
            ("op", ","),
            ("ident", "[col 1]"),
            ("op", ","),
            ("ident", "`c``d`"),
            ("comentario", "-- fim; de linha"),
            ("comentario", "/* bloco ; */"),
            ("palavra", "FROM"),
            ("palavra", "t"),
            ("palavra", "WHERE"),
            ("palavra", "v"),
            ("op", "<>"),
            ("numero", "1e5"),
            ("palavra", "AND"),
            ("palavra", "w"),
            ("op", "!="),
            ("numero", ".5"),
            ("palavra", "OR"),
            ("palavra", "h"),
            ("op", "="),
            ("numero", "0x1F"),
            ("palavra", "OR"),
            ("palavra", "k"),
            ("op", "="),
            ("numero", "3.25E-2"),
            ("palavra", "OR"),
            ("palavra", "b"),
            ("op", "="),
            ("blob", "X'00ff'"),
            ("op", ";"),
        ])
    );
}

#[test]
fn operadores_de_dois_caracteres() {
    let ops: Vec<String> = tokenizar("a||b<=c>=d<>e!=f==g<<h>>i->j->>k")
        .unwrap()
        .into_iter()
        .filter(|t| t.tipo == TipoToken::Op)
        .map(|t| t.texto)
        .collect();
    assert_eq!(ops, ["||", "<=", ">=", "<>", "!=", "==", "<<", ">>", "->", "->>"]);
}

#[test]
fn ponto_e_virgula_dentro_de_texto_e_comentario_nao_divide() {
    let sql = "INSERT INTO t VALUES ('a;b'); -- ; comentário\n/* ; */ SELECT 1";
    let pontos: Vec<Token> = tokenizar(sql).unwrap().into_iter().filter(|t| t.texto == ";").collect();
    assert!(pontos.len() == 1 && pontos[0].tipo == TipoToken::Op);
}

#[test]
fn palavras_com_acento_e_parametros() {
    assert_eq!(
        tipos_textos("número = :valor AND x = ?1"),
        pares(&[
            ("palavra", "número"),
            ("op", "="),
            ("palavra", ":valor"),
            ("palavra", "AND"),
            ("palavra", "x"),
            ("op", "="),
            ("op", "?1"),
        ])
    );
}

#[test]
fn trecho_sem_fechamento_e_erro() {
    for sql in ["SELECT 'aberto", "SELECT \"nome", "SELECT [x", "SELECT /* sem fim", "X'00"] {
        assert!(tokenizar(sql).is_err(), "{sql}");
    }
}

#[test]
fn desfazer_aspas() {
    for (texto, nome) in
        [("\"a\"\"b\"", "a\"b"), ("`c``d`", "c`d"), ("[col 1]", "col 1"), ("'x''y'", "x'y"), ("simples", "simples")]
    {
        assert_eq!(super::desfazer_aspas(texto), nome);
    }
}

// ---------------------------------------------------------------------------------------------------
// Analisador

/// (nome, sql em sqlite_schema do banco de exemplo, o texto esperado da ida e volta).
const TABELAS_DO_EXEMPLO: [(&str, &str, &str); 6] = [
    (
        "clientes",
        "CREATE TABLE clientes (\n    id        INTEGER PRIMARY KEY AUTOINCREMENT,\n    nome      TEXT NOT NULL,\n    \
         email     TEXT UNIQUE,\n    cidade    TEXT DEFAULT 'São Paulo',\n    limite    REAL CHECK (limite >= 0),\n    \
         ativo     INTEGER NOT NULL DEFAULT 1,\n    criado_em TEXT DEFAULT CURRENT_TIMESTAMP\n)",
        "CREATE TABLE \"clientes\" (\n    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n    \"nome\" TEXT NOT NULL,\n    \
         \"email\" TEXT UNIQUE,\n    \"cidade\" TEXT DEFAULT 'São Paulo',\n    \"limite\" REAL CHECK (limite >= 0),\n    \
         \"ativo\" INTEGER NOT NULL DEFAULT 1,\n    \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n)",
    ),
    (
        "produtos",
        "CREATE TABLE produtos (\n    codigo     TEXT PRIMARY KEY,\n    descricao  TEXT NOT NULL COLLATE NOCASE,\n    \
         preco      NUMERIC NOT NULL DEFAULT 0,\n    estoque    INTEGER DEFAULT 0,\n    foto       BLOB,\n    \
         serie      INTEGER,\n    preco_centavos INTEGER GENERATED ALWAYS AS (CAST(preco * 100 AS INTEGER)) VIRTUAL\n) \
         WITHOUT ROWID",
        "CREATE TABLE \"produtos\" (\n    \"codigo\" TEXT PRIMARY KEY,\n    \"descricao\" TEXT NOT NULL COLLATE NOCASE,\n    \
         \"preco\" NUMERIC NOT NULL DEFAULT 0,\n    \"estoque\" INTEGER DEFAULT 0,\n    \"foto\" BLOB,\n    \
         \"serie\" INTEGER,\n    \"preco_centavos\" INTEGER GENERATED ALWAYS AS (CAST(preco * 100 AS INTEGER)) VIRTUAL\n) \
         WITHOUT ROWID",
    ),
    (
        "pedidos",
        "CREATE TABLE pedidos (\n    id         INTEGER PRIMARY KEY,\n    \
         cliente_id INTEGER NOT NULL REFERENCES clientes(id) ON DELETE CASCADE,\n    feito_em   TEXT NOT NULL,\n    \
         status     TEXT NOT NULL DEFAULT 'aberto' CHECK (status IN ('aberto', 'pago', 'cancelado')),\n    \
         observacao TEXT\n)",
        "CREATE TABLE \"pedidos\" (\n    \"id\" INTEGER PRIMARY KEY,\n    \
         \"cliente_id\" INTEGER NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE,\n    \
         \"feito_em\" TEXT NOT NULL,\n    \
         \"status\" TEXT NOT NULL CHECK (status IN ('aberto', 'pago', 'cancelado')) DEFAULT 'aberto',\n    \
         \"observacao\" TEXT\n)",
    ),
    (
        "itens_pedido",
        "CREATE TABLE itens_pedido (\n    pedido_id  INTEGER NOT NULL REFERENCES pedidos(id) ON DELETE CASCADE,\n    \
         produto    TEXT NOT NULL REFERENCES produtos(codigo),\n    quantidade INTEGER NOT NULL DEFAULT 1,\n    \
         preco      NUMERIC,\n    PRIMARY KEY (pedido_id, produto)\n)",
        "CREATE TABLE \"itens_pedido\" (\n    \
         \"pedido_id\" INTEGER NOT NULL REFERENCES \"pedidos\" (\"id\") ON DELETE CASCADE,\n    \
         \"produto\" TEXT NOT NULL REFERENCES \"produtos\" (\"codigo\"),\n    \
         \"quantidade\" INTEGER NOT NULL DEFAULT 1,\n    \"preco\" NUMERIC,\n    PRIMARY KEY (pedido_id, produto)\n)",
    ),
    (
        "eventos",
        "CREATE TABLE eventos (\n    momento TEXT,\n    tipo    TEXT,\n    valor   REAL,\n    dados   TEXT\n)",
        "CREATE TABLE \"eventos\" (\n    \"momento\" TEXT,\n    \"tipo\" TEXT,\n    \"valor\" REAL,\n    \"dados\" TEXT\n)",
    ),
    (
        "Tabela com espaço",
        "CREATE TABLE \"Tabela com espaço\" (\n    \"coluna \"\"citada\"\"\" TEXT,\n    \"número\" INTEGER\n)",
        "CREATE TABLE \"Tabela com espaço\" (\n    \"coluna \"\"citada\"\"\" TEXT,\n    \"número\" INTEGER\n)",
    ),
];

fn sql_tabela(nome: &str) -> &'static str {
    TABELAS_DO_EXEMPLO.iter().find(|(n, _, _)| *n == nome).map(|(_, sql, _)| *sql).unwrap()
}

/// Inteiro de tamanho variável do formato do SQLite: (valor, bytes lidos).
fn varint(b: &[u8]) -> (u64, usize) {
    let mut v = 0u64;
    for (i, &byte) in b.iter().take(8).enumerate() {
        v = (v << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return (v, i + 1);
        }
    }
    ((v << 8) | u64::from(b[8]), 9)
}

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

fn be16(b: &[u8]) -> usize {
    usize::from(u16::from_be_bytes([b[0], b[1]]))
}

/// (type, name, sql) de cada linha do sqlite_schema de um arquivo SQLite, lido sem o SQLite (o crate não
/// tem rusqlite), pelo formato de sqlite.org/fileformat.html. Basta o caso do banco de exemplo: o
/// sqlite_schema inteiro numa folha só (a página 1) e nenhum registro transbordando para outra página; fora
/// disso o teste para com a mensagem.
fn ler_sqlite_schema(bytes: &[u8]) -> Vec<(String, String, Option<String>)> {
    assert_eq!(&bytes[..16], b"SQLite format 3\0");
    assert_eq!(be32(&bytes[56..]), 1, "o texto do banco de exemplo é UTF-8");
    let tamanho = match be16(&bytes[16..]) {
        1 => 65536,
        n => n,
    };
    let util = tamanho - usize::from(bytes[20]);
    let p = &bytes[..tamanho];
    let h = 100; // a página 1 começa com o cabeçalho do arquivo
    assert_eq!(p[h], 0x0d, "o sqlite_schema do exemplo passou a ocupar mais de uma página: o leitor precisa crescer");
    (0..be16(&p[h + 3..]))
        .map(|k| {
            let pos = be16(&p[h + 8 + 2 * k..]);
            let (total, a) = varint(&p[pos..]);
            let (_rowid, b) = varint(&p[pos + a..]);
            let (total, inicio) = (total as usize, pos + a + b);
            assert!(total <= util - 35, "registro do sqlite_schema maior que a página: o leitor precisa crescer");
            registro_do_esquema(&p[inicio..inicio + total])
        })
        .collect()
}

/// Colunas type, name e sql (0, 1 e 4) de um registro do sqlite_schema.
fn registro_do_esquema(carga: &[u8]) -> (String, String, Option<String>) {
    let (tamanho_cabecalho, mut pos_cab) = varint(carga);
    let mut pos_dado = tamanho_cabecalho as usize;
    let mut colunas: Vec<Option<String>> = Vec::new();
    while pos_cab < tamanho_cabecalho as usize {
        let (serial, a) = varint(&carga[pos_cab..]);
        pos_cab += a;
        let tamanho = match serial {
            0 | 8 | 9 => 0,
            1..=4 => serial as usize,
            5 => 6,
            6 | 7 => 8,
            n if n >= 12 => ((n - 12) / 2) as usize,
            n => panic!("tipo de coluna {n} no sqlite_schema"),
        };
        let dado = &carga[pos_dado..pos_dado + tamanho];
        pos_dado += tamanho;
        colunas.push((serial >= 13 && serial % 2 == 1).then(|| String::from_utf8(dado.to_vec()).unwrap()));
    }
    (colunas[0].clone().unwrap(), colunas[1].clone().unwrap(), colunas[4].clone())
}

/// (nome, sql) das tabelas comuns do banco de exemplo, lidas do arquivo: as que `PRAGMA table_list` dá (tipo "table" em main, sem as tabelas virtuais e as de sombra delas) e
/// `name NOT LIKE 'sqlite_%'`.
fn tabelas_do_arquivo_de_exemplo() -> Vec<(String, String)> {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR")).join("../amostras/dados/exemplo.db");
    let esquema = ler_sqlite_schema(&std::fs::read(&caminho).unwrap());
    let virtuais: Vec<String> = esquema
        .iter()
        .filter(|(tipo, _, sql)| tipo == "table" && sql.as_deref().is_some_and(|s| s.starts_with("CREATE VIRTUAL")))
        .map(|(_, nome, _)| nome.to_lowercase())
        .collect();
    // tabela de sombra do FTS5: <virtual>_config, _content, _data, _docsize ou _idx
    let de_sombra = |nome: &str| {
        let nome = nome.to_lowercase();
        virtuais.iter().any(|v| {
            nome.strip_prefix(&format!("{v}_"))
                .is_some_and(|resto| ["config", "content", "data", "docsize", "idx"].contains(&resto))
        })
    };
    esquema
        .into_iter()
        .filter(|(tipo, nome, _)| {
            tipo == "table"
                && !nome.to_lowercase().starts_with("sqlite_")
                && !virtuais.contains(&nome.to_lowercase())
                && !de_sombra(nome)
        })
        .map(|(_, nome, sql)| (nome, sql.unwrap()))
        .collect()
}

#[test]
fn leitor_do_esquema_ve_o_banco_de_exemplo_inteiro() {
    // o mesmo que `SELECT type, name, length(sql) FROM sqlite_schema` devolve
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR")).join("../amostras/dados/exemplo.db");
    let esquema = ler_sqlite_schema(&std::fs::read(&caminho).unwrap());
    let lido: Vec<(&str, &str, Option<usize>)> = esquema
        .iter()
        .map(|(tipo, nome, sql)| (tipo.as_str(), nome.as_str(), sql.as_ref().map(|s| s.chars().count())))
        .collect();
    assert_eq!(
        lido,
        [
            ("table", "clientes", Some(297)),
            ("index", "sqlite_autoindex_clientes_1", None),
            ("table", "sqlite_sequence", Some(38)),
            ("table", "produtos", Some(325)),
            ("table", "pedidos", Some(282)),
            ("table", "itens_pedido", Some(265)),
            ("index", "sqlite_autoindex_itens_pedido_1", None),
            ("table", "eventos", Some(95)),
            ("table", "Tabela com espaço", Some(87)),
            ("index", "idx_pedidos_cliente", Some(56)),
            ("index", "idx_pedidos_status_data", Some(71)),
            ("index", "idx_clientes_nome_cidade", Some(87)),
            ("view", "v_pedidos_por_cliente", Some(192)),
            ("trigger", "trg_pedido_cancelado", Some(217)),
            ("table", "notas", Some(52)),
            ("table", "notas_data", Some(61)),
            ("table", "notas_idx", Some(83)),
            ("table", "notas_content", Some(60)),
            ("table", "notas_docsize", Some(61)),
            ("table", "notas_config", Some(59)),
        ]
    );
    // e as tabelas da ida e volta (PRAGMA table_list: tipo "table", sem as de sombra)
    let comuns: Vec<String> = tabelas_do_arquivo_de_exemplo().into_iter().map(|(nome, _)| nome).collect();
    assert_eq!(comuns, ["clientes", "produtos", "pedidos", "itens_pedido", "eventos", "Tabela com espaço"]);
}

fn por_nome(d: &DefTabela) -> BTreeMap<String, DefColuna> {
    d.colunas.iter().map(|c| (c.nome.clone(), c.clone())).collect()
}

#[test]
fn analisar_clientes() {
    let d = analisar_create_table(sql_tabela("clientes")).unwrap();
    assert!(d.nome == "clientes" && !d.sem_rowid && !d.estrita && d.restricoes.is_empty());
    let c = por_nome(&d);
    assert_eq!(
        d.colunas.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(),
        ["id", "nome", "email", "cidade", "limite", "ativo", "criado_em"]
    );
    assert!(c["id"].tipo == "INTEGER" && c["id"].pk && c["id"].autoincremento);
    assert!(c["nome"].nao_nulo && !c["nome"].unica);
    assert!(c["email"].unica);
    assert_eq!(c["cidade"].padrao.as_deref(), Some("'São Paulo'"));
    assert_eq!(c["limite"].verificacao.as_deref(), Some("limite >= 0"));
    assert!(c["ativo"].nao_nulo && c["ativo"].padrao.as_deref() == Some("1"));
    assert_eq!(c["criado_em"].padrao.as_deref(), Some("CURRENT_TIMESTAMP"));
    assert!(d.colunas.iter().all(|x| x.extra.is_none()));
    assert_eq!(c["cidade"].texto, "cidade TEXT DEFAULT 'São Paulo'");
}

#[test]
fn analisar_produtos_sem_rowid_com_coluna_gerada() {
    let d = analisar_create_table(sql_tabela("produtos")).unwrap();
    assert!(d.sem_rowid && d.sufixo == "WITHOUT ROWID");
    let c = por_nome(&d);
    assert!(c["codigo"].pk && !c["codigo"].autoincremento);
    assert!(c["descricao"].colacao.as_deref() == Some("NOCASE") && c["descricao"].nao_nulo);
    assert!(c["preco"].tipo == "NUMERIC" && c["preco"].padrao.as_deref() == Some("0"));
    assert_eq!(
        c["preco_centavos"].gerada,
        Some(Gerada { expressao: "CAST(preco * 100 AS INTEGER)".into(), armazenada: false })
    );
    assert_eq!(c["preco_centavos"].tipo, "INTEGER");
}

#[test]
fn analisar_pedidos_e_itens() {
    let d = analisar_create_table(sql_tabela("pedidos")).unwrap();
    let c = por_nome(&d);
    let r = c["cliente_id"].referencia.clone().unwrap();
    assert_eq!(r.tabela, "clientes");
    assert_eq!(r.coluna.as_deref(), Some("id"));
    assert_eq!(r.ao_excluir.as_deref(), Some("CASCADE"));
    assert_eq!(r.ao_atualizar, None);
    assert_eq!(c["status"].padrao.as_deref(), Some("'aberto'"));
    assert_eq!(c["status"].verificacao.as_deref(), Some("status IN ('aberto', 'pago', 'cancelado')"));
    let itens = analisar_create_table(sql_tabela("itens_pedido")).unwrap();
    assert_eq!(itens.restricoes, ["PRIMARY KEY (pedido_id, produto)"]);
    assert_eq!(
        itens.colunas.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(),
        ["pedido_id", "produto", "quantidade", "preco"]
    );
}

#[test]
fn analisar_nomes_com_aspas() {
    let d = analisar_create_table(sql_tabela("Tabela com espaço")).unwrap();
    assert_eq!(d.nome, "Tabela com espaço");
    assert_eq!(
        d.colunas.iter().map(|x| (x.nome.as_str(), x.tipo.as_str())).collect::<Vec<_>>(),
        [("coluna \"citada\"", "TEXT"), ("número", "INTEGER")]
    );
}

#[test]
fn analisar_formas_raras() {
    let sql = r#"create temp table if not exists main."x y" (
        a VARCHAR(255) CONSTRAINT nn NOT NULL ON CONFLICT REPLACE,
        b DOUBLE   PRECISION DEFAULT -1.5,
        c UNSIGNED BIG INT PRIMARY KEY DESC,
        d DECIMAL(10, 2) REFERENCES "t 2"(id) ON UPDATE SET NULL MATCH SIMPLE DEFERRABLE INITIALLY DEFERRED,
        e TEXT AS (a || 'x, y') STORED,
        f DEFAULT (datetime('now')) COLLATE "RTRIM",
        g UNIQUE ON CONFLICT IGNORE NULL,
        -- comentário no meio, com vírgula
        CONSTRAINT ck CHECK (b > 0), UNIQUE (a, b)
    ) strict, WITHOUT ROWID;"#;
    let d = analisar_create_table(sql).unwrap();
    assert!(d.nome == "x y" && d.sem_rowid && d.estrita);
    let c = por_nome(&d);
    assert!(c["a"].tipo == "VARCHAR(255)" && !c["a"].nao_nulo);
    assert_eq!(c["a"].extra.as_deref(), Some("CONSTRAINT nn NOT NULL ON CONFLICT REPLACE"));
    assert!(c["b"].tipo == "DOUBLE PRECISION" && c["b"].padrao.as_deref() == Some("-1.5"));
    assert!(c["c"].tipo == "UNSIGNED BIG INT" && !c["c"].pk && c["c"].extra.as_deref() == Some("PRIMARY KEY DESC"));
    let r = c["d"].referencia.clone().unwrap();
    assert_eq!(c["d"].tipo, "DECIMAL(10, 2)");
    assert_eq!(
        (r.tabela.as_str(), r.coluna.as_deref(), r.ao_atualizar.as_deref(), r.ao_excluir.as_deref()),
        ("t 2", Some("id"), Some("SET NULL"), None)
    );
    assert_eq!(r.complemento.as_deref(), Some("MATCH SIMPLE DEFERRABLE INITIALLY DEFERRED"));
    assert_eq!(c["e"].gerada, Some(Gerada { expressao: "a || 'x, y'".into(), armazenada: true }));
    assert!(
        c["f"].tipo.is_empty()
            && c["f"].padrao.as_deref() == Some("(datetime('now'))")
            && c["f"].colacao.as_deref() == Some("RTRIM")
    );
    assert!(c["g"].extra.as_deref() == Some("UNIQUE ON CONFLICT IGNORE") && !c["g"].unica);
    assert_eq!(d.restricoes, ["CONSTRAINT ck CHECK (b > 0)", "UNIQUE (a, b)"]);
    // o resto
    assert_eq!(d.sufixo, "strict, WITHOUT ROWID");
    assert_eq!(
        c["d"].referencia_texto.as_deref(),
        Some("REFERENCES \"t 2\"(id) ON UPDATE SET NULL MATCH SIMPLE DEFERRABLE INITIALLY DEFERRED")
    );
    assert_eq!(
        c["d"].texto,
        "d DECIMAL(10, 2) REFERENCES \"t 2\"(id) ON UPDATE SET NULL MATCH SIMPLE DEFERRABLE INITIALLY DEFERRED"
    );
    assert_eq!(c["e"].texto, "e TEXT AS (a || 'x, y') STORED");
    assert_eq!(c["a"].pk_ordem, None);
}

#[test]
fn create_table_as_select_nao_e_editavel() {
    let erro = analisar_create_table("CREATE TABLE copia AS SELECT * FROM clientes").unwrap_err();
    assert!(erro.mensagem.contains("AS SELECT"), "{}", erro.mensagem);
}

#[test]
fn outros_comandos_sao_recusados() {
    for sql in
        ["CREATE VIRTUAL TABLE n USING fts5(a)", "CREATE VIEW v AS SELECT 1", "CREATE TABLE t", "CREATE TABLE t ()"]
    {
        assert!(analisar_create_table(sql).is_err(), "{sql}");
    }
}

// ---------------------------------------------------------------------------------------------------
// Ida e volta

fn ida_e_volta(sql: &str) -> String {
    let d = analisar_create_table(sql).unwrap();
    let colunas: Vec<String> = d.colunas.iter().map(|c| montar_coluna(&c.como_dict()).unwrap()).collect();
    montar_create_table(&d.nome, &colunas, &d.restricoes, &d.sufixo).unwrap()
}

/// O que voltou, analisado de novo, tem a mesma definição e é ponto fixo.
fn conferir_definicao(original: &str, volta: &str) {
    let antes = analisar_create_table(original).unwrap();
    let depois = analisar_create_table(volta).unwrap();
    assert_eq!(depois.nome, antes.nome);
    assert_eq!(
        depois.colunas.iter().map(DefColuna::como_dict).collect::<Vec<_>>(),
        antes.colunas.iter().map(DefColuna::como_dict).collect::<Vec<_>>()
    );
    assert_eq!(depois.restricoes, antes.restricoes);
    assert_eq!((depois.sem_rowid, depois.estrita, &depois.sufixo), (antes.sem_rowid, antes.estrita, &antes.sufixo));
    assert_eq!(ida_e_volta(volta), volta);
}

#[test]
fn ida_e_volta_de_todas_as_tabelas_do_exemplo() {
    // percorre as tabelas do arquivo, e não a lista daqui
    let tabelas = tabelas_do_arquivo_de_exemplo();
    let nomes: BTreeSet<&str> = tabelas.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        ["clientes", "produtos", "pedidos", "itens_pedido", "eventos", "Tabela com espaço"]
            .iter()
            .all(|n| nomes.contains(n))
    );
    for (nome, sql) in &tabelas {
        // tabela nova no exemplo pede aqui o texto esperado para ela
        let (_, copia, esperado) = TABELAS_DO_EXEMPLO
            .iter()
            .find(|(n, _, _)| n == nome)
            .unwrap_or_else(|| panic!("falta em TABELAS_DO_EXEMPLO a tabela {nome} do banco de exemplo"));
        assert_eq!(copia, sql, "a cópia do SQL de {nome} difere do sqlite_schema do exemplo");
        let volta = ida_e_volta(sql);
        assert_eq!(volta, *esperado, "{nome}");
        conferir_definicao(sql, &volta);
    }
    assert_eq!(tabelas.len(), TABELAS_DO_EXEMPLO.len());
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
    let volta = ida_e_volta(&sql);
    assert_eq!(
        volta,
        "CREATE TABLE \"raro \"\"x\"\"\" (\n    \"id\" INTEGER PRIMARY KEY DESC,\n    \
         \"a\" VARCHAR(255) DEFAULT '' CONSTRAINT nn NOT NULL ON CONFLICT REPLACE,\n    \
         \"b\" DOUBLE PRECISION CHECK (b <> 0 /* nunca zero; */) DEFAULT -1.5,\n    \
         \"c\" TEXT REFERENCES \"pai\" (\"id\") ON DELETE SET NULL ON UPDATE CASCADE MATCH FULL DEFERRABLE INITIALLY DEFERRED,\n    \
         \"d\" TEXT GENERATED ALWAYS AS (a || 'x') STORED,\n    \"e\" DEFAULT (datetime('now')) COLLATE NOCASE,\n    \
         \"f\" INT CONSTRAINT u UNIQUE,\n    \"g\" TEXT CHECK (g <> 'a' -- comentário no fim\n),\n    \
         CONSTRAINT ck CHECK (b > -10),\n    UNIQUE (a, b),\n    FOREIGN KEY (a, b) REFERENCES outra(x, y)\n)"
    );
    conferir_definicao(&sql, &volta);
    // "INTEGER PRIMARY KEY DESC" não vira apelido do rowid: a peculiaridade tem de sobreviver
    let id = &analisar_create_table(&volta).unwrap().colunas[0];
    assert!(!id.pk && id.extra.as_deref() == Some("PRIMARY KEY DESC"));
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
    let volta = ida_e_volta(sql);
    assert!(!volta.contains("--"));
    assert_eq!(
        volta,
        "CREATE TABLE \"registro\" (\n    \
         \"id\" INTEGER CONSTRAINT pk_registro PRIMARY KEY /* chave */ AUTOINCREMENT,\n    \
         \"categoria_id\" INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) /* dono */ ON DELETE CASCADE,\n    \
         \"nome\" VARCHAR /* tamanho */ (80) NOT NULL UNIQUE,\n    \
         \"rotulo\" TEXT NOT NULL CONSTRAINT d DEFAULT 'dois  espaços',\n    \
         \"ref\" INTEGER REFERENCES \"categoria\" (\"id\") DEFERRABLE /* adiada */ INITIALLY DEFERRED\n)"
    );
    conferir_definicao(sql, &volta);
    let depois = analisar_create_table(&volta).unwrap();
    assert_eq!(depois.colunas[2].tipo, "VARCHAR /* tamanho */ (80)");
    // nada ficou dentro de comentário: as palavras chegam ao SQLite
    let palavras: Vec<String> = significativos(&volta).unwrap().iter().map(Token::alto).collect();
    for p in ["AUTOINCREMENT", "CASCADE", "UNIQUE", "INITIALLY", "DEFERRED"] {
        assert!(palavras.iter().any(|x| x == p), "{p}");
    }
    assert!(palavras.windows(3).any(|w| w == ["ON", "DELETE", "CASCADE"]));
    let rotulo = &depois.colunas[3];
    assert!(rotulo.extra.as_deref() == Some("CONSTRAINT d DEFAULT 'dois  espaços'") && rotulo.nao_nulo);

    let volta = ida_e_volta(sufixo);
    assert_eq!(
        volta,
        "CREATE TABLE \"cfg\" (\n    \"k\" TEXT PRIMARY KEY,\n    \"v\" ANY\n) STRICT, /* sem rowid */ WITHOUT ROWID"
    );
    conferir_definicao(sufixo, &volta);
    let cfg = analisar_create_table(&volta).unwrap();
    assert!(cfg.estrita && cfg.sem_rowid);
}

#[test]
fn colunas_da_restricao_so_conta_os_nomes_entre_parenteses() {
    let c = |texto: &str, tabela: &str| colunas_da_restricao(texto, tabela).unwrap();
    assert_eq!(c("PRIMARY KEY (id)", "t"), conjunto(&["id"]));
    assert_eq!(c("CONSTRAINT key_pk PRIMARY KEY (id ASC)", "t"), conjunto(&["id"]));
    assert_eq!(c("FOREIGN KEY(dono) REFERENCES outra (key) ON DELETE NO ACTION", "t"), conjunto(&["dono"]));
    assert_eq!(c("FOREIGN KEY (pai) REFERENCES t (key)", "t"), conjunto(&["pai", "key"]));
    assert_eq!(
        c("CHECK (length(nome) > 0 AND \"key\" IS NOT NULL AND x IN (1, 2))", "t"),
        conjunto(&["nome", "key", "x"])
    );
    assert_eq!(c("UNIQUE (a COLLATE NOCASE, b)", "t"), conjunto(&["a", "b"]));
}

#[test]
fn colunas_gravadas_pelo_gatilho() {
    let sql = r#"CREATE TRIGGER g AFTER UPDATE OF status, "valor" ON pedidos WHEN new.total > 0
    BEGIN
        INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'x');
        INSERT OR REPLACE INTO "Log" (quando) SELECT CASE WHEN a THEN 1 ELSE 2 END FROM t;
        UPDATE OR IGNORE clientes SET total = total + new.total, (a, b) = (1, CASE WHEN x THEN 2 END),
            c = (SELECT max(d) FROM e) WHERE id = new.cliente_id;
        INSERT INTO resumo (dia) VALUES (1) ON CONFLICT (dia) DO UPDATE SET n = n + 1;
        DELETE FROM outra WHERE total = 0;
    END"#;
    let esperado: BTreeMap<String, BTreeSet<String>> = [
        ("pedidos", conjunto(&["status", "valor"])),
        ("auditoria", conjunto(&["pedido", "mensagem"])),
        ("log", conjunto(&["quando"])),
        ("clientes", conjunto(&["total", "a", "b", "c"])),
        ("resumo", conjunto(&["dia", "n"])),
    ]
    .into_iter()
    .map(|(t, c)| (t.to_string(), c))
    .collect();
    assert_eq!(super::colunas_gravadas_pelo_gatilho(sql).unwrap(), esperado);
}

fn coluna(nome: &str) -> ColunaDef {
    ColunaDef { nome: nome.into(), ..ColunaDef::default() }
}

#[test]
fn montar_coluna_padroes_e_expressoes() {
    let m = |c: ColunaDef| montar_coluna(&c).unwrap();
    assert_eq!(
        m(ColunaDef { tipo: "TEXT".into(), padrao: Some("'x'".into()), ..coluna("a\"b") }),
        "\"a\"\"b\" TEXT DEFAULT 'x'"
    );
    assert_eq!(
        m(ColunaDef { tipo: "REAL".into(), padrao: Some("-1.5".into()), ..coluna("n") }),
        "\"n\" REAL DEFAULT -1.5"
    );
    assert_eq!(
        m(ColunaDef { padrao: Some("datetime('now')".into()), ..coluna("d") }),
        "\"d\" DEFAULT (datetime('now'))"
    );
    assert_eq!(
        m(ColunaDef { padrao: Some("CURRENT_TIMESTAMP".into()), ..coluna("d") }),
        "\"d\" DEFAULT CURRENT_TIMESTAMP"
    );
    assert_eq!(
        m(ColunaDef { tipo: "INTEGER".into(), pk: true, autoincremento: true, ..coluna("id") }),
        "\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"
    );
    let sql = m(ColunaDef {
        tipo: "INTEGER".into(),
        nao_nulo: true,
        unica: true,
        verificacao: Some("c > 0".into()),
        colacao: Some("NOCASE".into()),
        referencia: Some(Referencia {
            tabela: "Outra tabela".into(),
            coluna: Some("id".into()),
            ao_excluir: Some("cascade".into()),
            ..Referencia::default()
        }),
        ..coluna("c")
    });
    assert_eq!(
        sql,
        "\"c\" INTEGER NOT NULL UNIQUE CHECK (c > 0) COLLATE NOCASE REFERENCES \"Outra tabela\" (\"id\") ON DELETE CASCADE"
    );
    assert_eq!(
        m(ColunaDef {
            tipo: "INTEGER".into(),
            gerada: Some(Gerada { expressao: "a + 1".into(), armazenada: true }),
            ..coluna("g")
        }),
        "\"g\" INTEGER GENERATED ALWAYS AS (a + 1) STORED"
    );
}

#[test]
fn comentario_de_linha_numa_expressao_nao_engole_o_resto() {
    let sql = montar_coluna(&ColunaDef {
        tipo: "INTEGER".into(),
        verificacao: Some("c > 0 -- positivo".into()),
        padrao: Some("1".into()),
        ..coluna("c")
    })
    .unwrap();
    let criar = montar_create_table("t", &[sql], &[], "").unwrap();
    assert_eq!(criar, "CREATE TABLE \"t\" (\n    \"c\" INTEGER CHECK (c > 0 -- positivo\n) DEFAULT 1\n)");
    let d = analisar_create_table(&criar).unwrap();
    assert_eq!(d.colunas[0].padrao.as_deref(), Some("1"));
}

/// O "\n" que o validar_trecho põe depois do comentário fica (topo do lib.rs): sem ele,
/// `DEFAULT 'x' -- obs COLLATE NOCASE REFERENCES "p" …` perderia em silêncio a colação e a chave estrangeira (o SQLite e a nova análise só viam o DEFAULT).
#[test]
fn padrao_com_comentario_de_linha_nao_engole_o_resto_da_coluna() {
    let c = ColunaDef {
        tipo: "TEXT".into(),
        padrao: Some("'x' -- obs".into()),
        colacao: Some("NOCASE".into()),
        referencia: Some(Referencia {
            tabela: "p".into(),
            ao_excluir: Some("cascade".into()),
            ..Referencia::default()
        }),
        extra: Some("CONSTRAINT u UNIQUE".into()),
        ..coluna("a")
    };
    let sql = montar_coluna(&c).unwrap();
    assert_eq!(
        sql,
        "\"a\" TEXT DEFAULT 'x' -- obs\n COLLATE NOCASE REFERENCES \"p\" ON DELETE CASCADE CONSTRAINT u UNIQUE"
    );
    let criar = montar_create_table("t", &[sql, "\"b\"".into()], &[], "").unwrap();
    let d = analisar_create_table(&criar).unwrap();
    let a = d.colunas[0].como_dict();
    assert_eq!(a.padrao.as_deref(), Some("'x'"));
    assert_eq!(a.colacao.as_deref(), Some("NOCASE"));
    assert_eq!(
        a.referencia,
        Some(Referencia { tabela: "p".into(), ao_excluir: Some("CASCADE".into()), ..Referencia::default() })
    );
    assert_eq!(a.extra.as_deref(), Some("CONSTRAINT u UNIQUE"));
    assert_eq!(d.colunas.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(), ["a", "b"]);

    // expressão: o ")" que a montagem acrescenta não pode cair dentro do comentário
    let m = |padrao: &str| montar_coluna(&ColunaDef { padrao: Some(padrao.into()), ..coluna("a") }).unwrap();
    assert_eq!(m("1 + 2 -- soma"), "\"a\" DEFAULT (1 + 2 -- soma\n)");
    assert_eq!(m("  (1) -- um  "), "\"a\" DEFAULT (1) -- um\n");
    assert_eq!(m("- 1 -- menos"), "\"a\" DEFAULT - 1 -- menos\n");
    assert_eq!(m("1 /* bloco */"), "\"a\" DEFAULT 1 /* bloco */");
    let d = analisar_create_table(&montar_create_table("t", &[m("1 + 2 -- soma")], &[], "").unwrap()).unwrap();
    assert_eq!(d.colunas[0].padrao.as_deref(), Some("(1 + 2 -- soma\n)"));
    // e a ida e volta é ponto fixo
    let volta = montar_create_table("t", &[montar_coluna(&d.colunas[0].como_dict()).unwrap()], &[], "").unwrap();
    assert_eq!(volta, "CREATE TABLE \"t\" (\n    \"a\" DEFAULT (1 + 2 -- soma\n)\n)");
}

#[test]
fn montar_coluna_recusa_definicao_invalida() {
    let casos = [
        ColunaDef { tipo: "TEXT".into(), ..coluna("") },
        ColunaDef { tipo: "INTEGER NOT NULL".into(), ..coluna("a") },
        ColunaDef { tipo: "TEXT); DROP TABLE x; --".into(), ..coluna("a") },
        ColunaDef { verificacao: Some("1); DROP TABLE clientes; --".into()), ..coluna("a") },
        ColunaDef { verificacao: Some("a > (1".into()), ..coluna("a") },
        ColunaDef { padrao: Some("1)".into()), ..coluna("a") },
        ColunaDef {
            referencia: Some(Referencia {
                tabela: "t".into(),
                ao_excluir: Some("APAGAR".into()),
                ..Referencia::default()
            }),
            ..coluna("a")
        },
        ColunaDef { autoincremento: true, ..coluna("a") },
    ];
    for c in casos {
        assert!(montar_coluna(&c).is_err(), "{c:?}");
    }
}

// ---------------------------------------------------------------------------------------------------
// Frases dos erros, montagem e texto acentuado, caso a caso

#[test]
fn posicoes_em_bytes_com_texto_acentuado() {
    let sql = "SELECT ação, \"Ê ç\", 'São Paulo' -- comentário é\n/* ü */ FROM [tabela ñ] WHERE número <> 1;";
    let tokens = tokenizar(sql).unwrap();
    assert_eq!(tokens.iter().map(|t| t.texto.as_str()).collect::<String>(), sql);
    for t in &tokens {
        assert!(sql.is_char_boundary(t.inicio) && sql.is_char_boundary(t.fim));
        assert_eq!(&sql[t.inicio..t.fim], t.texto);
        assert_eq!(fatia(sql, t.inicio, t.fim), t.texto);
    }
    let acao = tokens.iter().find(|t| t.texto == "ação").unwrap();
    assert_eq!((acao.inicio, acao.fim), (7, 13)); // bytes: "ç" e "ã" têm dois cada
    assert_eq!(fatia(sql, 8, 9), ""); // fora de fronteira de caractere: vazio, sem pânico
    assert_eq!(fatia(sql, 5, 500), "");
}

#[test]
fn erros_do_tokenizador_com_as_frases() {
    let casos = [
        ("SELECT 'aberto", "O comando SQL tem um texto entre aspas simples sem fechamento (começa na posição 8)."),
        ("SELECT \"nome", "O comando SQL tem um nome entre aspas duplas sem fechamento (começa na posição 8)."),
        ("SELECT [x", "O comando SQL tem um nome entre colchetes sem fechamento."),
        ("SELECT /* sem fim", "O comando SQL tem um comentário /* sem fechamento."),
        ("X'00", "O comando SQL tem um BLOB (X'…') sem fechamento (começa na posição 2)."),
        ("SELECT 'ação', 'x", "O comando SQL tem um texto entre aspas simples sem fechamento (começa na posição 16)."),
        ("SELECT ação, `x", "O comando SQL tem um nome entre crases sem fechamento (começa na posição 14)."),
        ("número = x'0", "O comando SQL tem um BLOB (X'…') sem fechamento (começa na posição 11)."),
    ];
    for (sql, frase) in casos {
        let erro = tokenizar(sql).unwrap_err();
        assert_eq!(erro.mensagem, frase, "{sql}");
        assert_eq!(erro.status, 400);
    }
}

#[test]
fn erros_do_analisador_com_as_frases() {
    let casos = [
        (
            "CREATE VIRTUAL TABLE n USING fts5(a)",
            "Tabela virtual não tem estrutura editável: as colunas vêm do módulo que a criou.",
        ),
        ("CREATE VIEW v AS SELECT 1", "Esperava TABLE no CREATE TABLE, mas veio VIEW."),
        ("CREATE TABLE t", "O CREATE TABLE não tem a lista de colunas entre parênteses."),
        ("CREATE TABLE t ()", "O CREATE TABLE não tem colunas."),
        (
            "CREATE TABLE copia AS SELECT * FROM clientes",
            "Esta tabela foi definida por uma consulta (CREATE TABLE … AS SELECT); a estrutura dela não pode ser editada aqui.",
        ),
        ("", "Esperava CREATE no CREATE TABLE, mas veio o fim do texto."),
        ("CREATE TABLE", "O CREATE TABLE não tem o nome da tabela."),
        ("CREATE TABLE t (a, b", "O comando CREATE TABLE tem parênteses sem fechamento."),
        ("create table if not exists", "O CREATE TABLE não tem o nome da tabela."),
        ("CREATE TABLE t (CHECK (a > 0))", "O CREATE TABLE não tem colunas."),
    ];
    for (sql, frase) in casos {
        assert_eq!(analisar_create_table(sql).unwrap_err().mensagem, frase, "{sql}");
    }
}

#[test]
fn montagem_caso_a_caso() {
    let r = |tabela: &str| Referencia { tabela: tabela.into(), ..Referencia::default() };
    let casos: Vec<(ColunaDef, Result<&str, &str>)> = vec![
        (ColunaDef { tipo: "TEXT".into(), ..coluna("") }, Err("Toda coluna precisa de um nome.")),
        (
            ColunaDef { tipo: "INTEGER NOT NULL".into(), ..coluna("a") },
            Err("O tipo «INTEGER NOT NULL» não pode ter restrições (NOT); use os campos da coluna para isso."),
        ),
        (
            ColunaDef { tipo: "TEXT); DROP TABLE x; --".into(), ..coluna("a") },
            Err("O tipo «TEXT); DROP TABLE x;» não é válido."),
        ),
        (
            ColunaDef { verificacao: Some("1); DROP TABLE clientes; --".into()), ..coluna("a") },
            Err("A regra de verificação tem um parêntese fechando sem abrir: 1); DROP TABLE clientes; --"),
        ),
        (
            ColunaDef { verificacao: Some("a > (1".into()), ..coluna("a") },
            Err("A regra de verificação tem parênteses sem fechamento: a > (1"),
        ),
        (
            ColunaDef { padrao: Some("1)".into()), ..coluna("a") },
            Err("O valor padrão tem um parêntese fechando sem abrir: 1)"),
        ),
        (
            ColunaDef { referencia: Some(Referencia { ao_excluir: Some("APAGAR".into()), ..r("t") }), ..coluna("a") },
            Err(
                "Ação inválida em Ao excluir: APAGAR. Use uma destas: SET NULL, SET DEFAULT, CASCADE, RESTRICT, NO ACTION.",
            ),
        ),
        (
            ColunaDef { autoincremento: true, ..coluna("a") },
            Err("Autoincremento só vale para a chave primária (coluna a)."),
        ),
        (ColunaDef { tipo: "VARCHAR('x')".into(), ..coluna("a") }, Err("O tipo «VARCHAR('x')» não é válido.")),
        (ColunaDef { tipo: "INT * 2".into(), ..coluna("a") }, Err("O tipo «INT * 2» não é válido.")),
        (ColunaDef { verificacao: Some("   ".into()), ..coluna("a") }, Ok("\"a\"")),
        (ColunaDef { colacao: Some("minha colação".into()), ..coluna("a") }, Ok("\"a\" COLLATE \"minha colação\"")),
        (ColunaDef { colacao: Some("  NOCASE ".into()), ..coluna("a") }, Ok("\"a\" COLLATE NOCASE")),
        (
            ColunaDef {
                referencia: Some(Referencia { ao_atualizar: Some("  no   action ".into()), ..r("t") }),
                ..coluna("a")
            },
            Ok("\"a\" REFERENCES \"t\" ON UPDATE NO ACTION"),
        ),
        // o "\n" fica depois do comentário; ver
        // padrao_com_comentario_de_linha_nao_engole_o_resto_da_coluna
        (ColunaDef { padrao: Some("1 -- um".into()), ..coluna("a") }, Ok("\"a\" DEFAULT 1 -- um\n")),
        (ColunaDef { padrao: Some("(1) + (2)".into()), ..coluna("a") }, Ok("\"a\" DEFAULT ((1) + (2))")),
        (ColunaDef { padrao: Some("x'00'".into()), ..coluna("a") }, Ok("\"a\" DEFAULT x'00'")),
        (ColunaDef { padrao: Some("+ 5".into()), ..coluna("a") }, Ok("\"a\" DEFAULT + 5")),
        (ColunaDef { tipo: "  DOUBLE \n PRECISION ".into(), ..coluna("a") }, Ok("\"a\" DOUBLE PRECISION")),
        (ColunaDef { extra: Some("CONSTRAINT x".into()), ..coluna("a") }, Ok("\"a\" CONSTRAINT x")),
        (ColunaDef { tipo: "x'00'".into(), ..coluna("a") }, Err("O tipo «x'00'» não é válido.")),
        (
            ColunaDef {
                referencia: Some(Referencia { complemento: Some("MATCH FULL".into()), ..r("t") }),
                ..coluna("a")
            },
            Ok("\"a\" REFERENCES \"t\" MATCH FULL"),
        ),
        (
            ColunaDef { gerada: Some(Gerada { expressao: "b * 2".into(), armazenada: false }), ..coluna("a") },
            Ok("\"a\" GENERATED ALWAYS AS (b * 2) VIRTUAL"),
        ),
        (
            ColunaDef { verificacao: Some("a > 0;".into()), ..coluna("a") },
            Err("A regra de verificação não pode ter ponto e vírgula: a > 0;"),
        ),
        (
            ColunaDef { verificacao: Some("a > 0)".into()), ..coluna("a") },
            Err("A regra de verificação tem um parêntese fechando sem abrir: a > 0)"),
        ),
        (
            ColunaDef { tipo: "TEXT".into(), padrao: Some("'é'".into()), ..coluna("ação") },
            Ok("\"ação\" TEXT DEFAULT 'é'"),
        ),
        // referência sem tabela e gerada sem expressão valem como ausentes
        (ColunaDef { referencia: Some(r("  ")), gerada: Some(Gerada::default()), ..coluna("a") }, Ok("\"a\"")),
    ];
    for (c, esperado) in casos {
        let obtido = montar_coluna(&c);
        match esperado {
            Ok(sql) => assert_eq!(obtido.as_deref(), Ok(sql), "{c:?}"),
            Err(frase) => assert_eq!(obtido.map_err(|e| e.mensagem), Err(frase.to_string()), "{c:?}"),
        }
    }
}

#[test]
fn montar_create_table_caso_a_caso() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let m = |nome: &str, colunas: &[&str], restricoes: &[&str], sufixo: &str| {
        montar_create_table(nome, &s(colunas), &s(restricoes), sufixo).map_err(|e| e.mensagem)
    };
    assert_eq!(m("", &["a"], &[], ""), Err("A tabela precisa de um nome.".to_string()));
    assert_eq!(m("  ", &["a"], &[], ""), Err("A tabela precisa de um nome.".to_string()));
    assert_eq!(m("t", &[], &[], ""), Err("A tabela precisa de ao menos uma coluna.".to_string()));
    assert_eq!(
        m("t", &["\"a\" -- x", "\"b\""], &["UNIQUE (a) -- y", "CHECK (b)"], " STRICT -- z"),
        Ok("CREATE TABLE \"t\" (\n    \"a\" -- x\n    ,\n    \"b\",\n    UNIQUE (a) -- y\n    ,\n    CHECK (b)\n) STRICT -- z"
            .to_string())
    );
    assert_eq!(
        m("t", &["\"a\""], &["CHECK (a"], ""),
        Err("A restrição da tabela tem parênteses sem fechamento: CHECK (a".to_string())
    );
    assert_eq!(
        m("t", &["\"a\""], &[], "WITHOUT ROWID; DROP"),
        Err("O complemento da tabela não pode ter ponto e vírgula: WITHOUT ROWID; DROP".to_string())
    );
    assert_eq!(validar_trecho("  ", "A condição do índice").unwrap_err().mensagem, "A condição do índice está vazio.");
}

#[test]
fn normalizar_espacos_caso_a_caso() {
    let casos = [
        ("a  \n b", "a b"),
        ("VARCHAR -- tamanho\n (80)", "VARCHAR /* tamanho */ (80)"),
        ("'a  b'  c", "'a  b' c"),
        ("x -- */ y\n z", "x /* * / y */ z"),
        ("a /* b\n  c */ d", "a /* b c */ d"),
        ("a -- \n b", "a b"),
        ("'aberto  x", "'aberto x"),
        ("  \u{1c} a \u{a0} b\u{3000} ", "a b"),
    ];
    for (texto, esperado) in casos {
        assert_eq!(normalizar_espacos(texto), esperado, "{texto:?}");
    }
    assert_eq!(aparar("\u{1f} x \u{85}"), "x");
    assert_eq!(colapsar_espacos(" a \t\n b "), " a b ");
}

/// `e_espaco` é exatamente o White_Space do Unicode mais os separadores 0x1C–0x1F, conferido em todo o
/// Unicode.
/// banco e prefs têm cópias próprias desta regra (não dependem do ddl); este teste fixa a do ddl.
#[test]
fn e_espaco_em_todo_o_unicode() {
    let esperados: Vec<u32> = [0x9, 0xa, 0xb, 0xc, 0xd, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x85, 0xa0, 0x1680]
        .into_iter()
        .chain(0x2000..=0x200a)
        .chain([0x2028, 0x2029, 0x202f, 0x205f, 0x3000])
        .collect();
    assert_eq!(esperados.len(), 29);
    let rust: Vec<u32> =
        (0..=0x10ffff_u32).filter_map(char::from_u32).filter(|&c| e_espaco(c)).map(u32::from).collect();
    assert_eq!(rust, esperados);
    // e o trim() do Rust não serve: deixa os separadores 0x1C–0x1F
    assert_eq!("\u{1c}x\u{1f}".trim(), "\u{1c}x\u{1f}");
    assert_eq!(aparar("\u{1c}x\u{1f}"), "x");
    assert!(!preenchido("\u{1c}\u{1d}\u{1e}\u{1f} \u{3000}"));
}

#[test]
fn gatilhos_com_esquema_e_acento() {
    let g = |sql: &str| super::colunas_gravadas_pelo_gatilho(sql).unwrap();
    let um = |t: &str, c: &[&str]| (t.to_string(), conjunto(c));
    assert_eq!(
        g("CREATE TRIGGER x AFTER INSERT ON main.t BEGIN UPDATE main.\"Ação\" SET \"Ç\" = 1; END"),
        [um("ação", &["ç"])].into_iter().collect()
    );
    assert_eq!(
        g("CREATE TRIGGER x UPDATE OF a ON t BEGIN INSERT INTO s.u(b) VALUES(1); UPDATE v SET c=1 FROM w; END"),
        [um("t", &["a"]), um("u", &["b"]), um("v", &["c"])].into_iter().collect()
    );
    assert!(g("").is_empty());
}

#[test]
fn expressoes_caso_a_caso() {
    let e = |expr: &str| colunas_da_expressao(expr).unwrap();
    assert_eq!(e("CAST(preco * 100 AS INTEGER)"), conjunto(&["preco"]));
    assert_eq!(e("a COLLATE NOCASE > :p AND t.b = @x"), conjunto(&["a", "b"]));
    assert_eq!(e("ção + \"Ê\" + [x y]"), conjunto(&["x y", "ção", "ê"]));
    // "ın" em maiúsculas é "IN" (maiúscula Unicode completa): palavra-chave, não coluna
    assert_eq!(e("abs(ı) + ın"), conjunto(&["ı"]));
    assert_eq!(e("CASE WHEN x IS NOT NULL THEN y ELSE z END"), conjunto(&["x", "y", "z"]));
    assert_eq!(e(""), BTreeSet::new());
    assert_eq!(
        citados("SELECT c.Nome FROM \"Clientes\" c").unwrap(),
        conjunto(&["select", "c", "nome", "from", "clientes"])
    );
}

#[test]
fn token_e_compara_em_maiusculas() {
    let t = |texto: &str, tipo| Token { tipo, texto: texto.into(), inicio: 0, fim: texto.len() };
    assert!(t("create", TipoToken::Palavra).e(&["CREATE"]));
    assert!(!t("create", TipoToken::Palavra).e(&["create"]));
    assert!(!t("\"CREATE\"", TipoToken::Ident).e(&["CREATE"]));
    assert!(t("ın", TipoToken::Palavra).e(&["IN"]));
    assert!(!t("ação", TipoToken::Palavra).e(&["ACAO"]));
    assert_eq!(t("ação", TipoToken::Palavra).alto(), "AÇÃO");
    assert_eq!(
        TIPOS_TOKEN.map(TipoToken::como_str),
        ["palavra", "ident", "string", "numero", "op", "espaco", "comentario", "blob"]
    );
}

#[test]
fn como_dict_leva_o_complemento_so_quando_existe() {
    let d =
        analisar_create_table("CREATE TABLE t (a REFERENCES p(id) ON DELETE SET DEFAULT, b REFERENCES p MATCH FULL)")
            .unwrap();
    let a = d.colunas[0].como_dict();
    assert_eq!(
        a.referencia,
        Some(Referencia {
            tabela: "p".into(),
            coluna: Some("id".into()),
            ao_excluir: Some("SET DEFAULT".into()),
            ..Referencia::default()
        })
    );
    let b = d.colunas[1].como_dict();
    assert_eq!(b.referencia.unwrap().complemento.as_deref(), Some("MATCH FULL"));
}

#[test]
fn nada_entra_em_panico_com_texto_cortado() {
    let textos = [
        TABELAS_DO_EXEMPLO[0].1,
        TABELAS_DO_EXEMPLO[1].1,
        "create temp table if not exists main.\"x y\" (a VARCHAR(255) CONSTRAINT nn NOT NULL ON CONFLICT REPLACE, \
         d DECIMAL(10, 2) REFERENCES \"t 2\"(id) ON UPDATE SET NULL MATCH SIMPLE NOT DEFERRABLE INITIALLY DEFERRED, \
         e TEXT GENERATED ALWAYS AS (a || 'x, y') STORED, f GENERATED x, g CONSTRAINT, h PRIMARY KEY ASC ON CONFLICT \
         ABORT AUTOINCREMENT, i DEFAULT - 1, j COLLATE [ç ã], CONSTRAINT ck CHECK (b > 0)) strict, WITHOUT ROWID;",
        "CREATE TRIGGER g AFTER UPDATE OF status, \"valor\" ON main.pedidos BEGIN INSERT INTO s.\"Ação\" (é) VALUES (1) \
         ON CONFLICT (é) DO UPDATE SET (a, b) = (1, CASE WHEN x THEN 2 END), c = 1; UPDATE OR IGNORE main.t SET x = 1; \
         UPDATE OR",
        "SELECT ação, X'0a', 0x1F_2, 1_000.5e-3, ?12, :ç, @x, $y, a->>'b' -- fim\n/* ç */ [ñ] `ü` 'é''s'",
    ];
    for texto in textos {
        let cortes: Vec<usize> = (0..=texto.len()).filter(|&i| texto.is_char_boundary(i)).collect();
        for &corte in &cortes {
            for parte in [&texto[..corte], &texto[corte..]] {
                let _ = tokenizar(parte);
                let _ = analisar_create_table(parte);
                let _ = citados(parte);
                let _ = colunas_da_expressao(parte);
                let _ = colunas_da_restricao(parte, "t");
                let _ = super::colunas_gravadas_pelo_gatilho(parte);
                let _ = normalizar_espacos(parte);
                let _ = validar_trecho(parte, "O trecho");
                if let Ok(toks) = significativos(parte) {
                    for i in 0..toks.len() {
                        let _ = fecha(&toks, i);
                    }
                }
                let c = ColunaDef {
                    nome: parte.into(),
                    tipo: parte.into(),
                    padrao: Some(parte.into()),
                    verificacao: Some(parte.into()),
                    colacao: Some(parte.into()),
                    referencia: Some(Referencia {
                        tabela: parte.into(),
                        coluna: Some(parte.into()),
                        ao_excluir: Some(parte.into()),
                        ao_atualizar: None,
                        complemento: Some(parte.into()),
                    }),
                    gerada: Some(Gerada { expressao: parte.into(), armazenada: true }),
                    extra: Some(parte.into()),
                    ..ColunaDef::default()
                };
                let _ = montar_coluna(&c);
                let _ = montar_create_table(parte, &[parte.to_string()], &[parte.to_string()], parte);
            }
        }
    }
}

#[test]
fn tipos_podem_ir_entre_threads() {
    fn exige<T: Send + Sync>() {}
    exige::<Token>();
    exige::<DefTabela>();
    exige::<DefColuna>();
    exige::<ColunaDef>();
    exige::<Referencia>();
    exige::<Gerada>();
}
