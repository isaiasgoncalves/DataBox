//! Renomear, adicionar, excluir e reordenar colunas.

use amostras::{N_CLIENTES, N_EVENTOS, N_PEDIDOS};
use comum::Valor;
use ddl::{ColunaDef, Gerada};

use super::*;
use crate::{
    plano_adicionar_coluna, plano_alterar_coluna, plano_criar_tabela, plano_criar_visao, plano_excluir_coluna,
    plano_excluir_indice, plano_reordenar_colunas,
};

fn observacao(conn: &Connection, id: i64) -> String {
    conn.query_row("SELECT observacao FROM pedidos WHERE id = ?", [id], |r| r.get(0)).unwrap()
}

// ---------------------------------------------------------------------------------------------------
// Renomear coluna

#[test]
fn renomear_coluna_citada_por_indice_e_gatilho() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { nome: "situacao".into(), ..definicao(conn, "pedidos", "status") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "pedidos", "status", &nova));
    assert!(!plano.reconstroi);
    assert_eq!(plano.comandos, textos(&["ALTER TABLE \"pedidos\" RENAME COLUMN \"status\" TO \"situacao\""]));
    assert!(sql_de(conn, "idx_pedidos_status_data").unwrap().contains("situacao"));
    let gatilho = sql_de(conn, "trg_pedido_cancelado").unwrap();
    assert!(gatilho.contains("situacao") && !gatilho.contains("status"));
    conn.execute("UPDATE pedidos SET situacao = 'cancelado' WHERE id = 2", []).unwrap();
    assert!(observacao(conn, 2).contains("cancelado"));
    assert_eq!(contar(conn, "pedidos"), N_PEDIDOS);
}

#[test]
fn renomear_coluna_citada_por_visao() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { nome: "nome_completo".into(), ..definicao(conn, "clientes", "nome") };
    aplicar(conn, || plano_alterar_coluna(conn, "clientes", "nome", &nova));
    assert!(sql_de(conn, "v_pedidos_por_cliente").unwrap().replace('"', "").contains("c.nome_completo"));
    assert!(sql_de(conn, "idx_clientes_nome_cidade").unwrap().contains("nome_completo"));
    assert_eq!(contar(conn, "v_pedidos_por_cliente"), N_CLIENTES);
}

#[test]
fn renomear_e_mudar_tipo_ao_mesmo_tempo() {
    let a = ambiente();
    let conn = &a.conn;
    let antes = valores(conn, "SELECT id, cliente_id, feito_em, status, observacao FROM pedidos ORDER BY id");
    let nova =
        ColunaDef { nome: "situacao".into(), tipo: "VARCHAR(20)".into(), ..definicao(conn, "pedidos", "status") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "pedidos", "status", &nova));
    assert!(plano.reconstroi);
    assert_eq!(
        plano.comandos[..2],
        textos(&["BEGIN", "ALTER TABLE \"pedidos\" RENAME COLUMN \"status\" TO \"situacao\""])[..]
    );
    assert!(plano.descricao.contains("nome status → situacao; tipo TEXT → VARCHAR(20)"));
    let depois = valores(conn, "SELECT id, cliente_id, feito_em, situacao, observacao FROM pedidos ORDER BY id");
    assert_eq!(depois, antes);
    assert_eq!(coluna_xinfo(conn, "pedidos", "situacao").tipo, "VARCHAR(20)");
    assert!(sql_de(conn, "idx_pedidos_status_data").unwrap().contains("situacao"));
    assert!(sql_de(conn, "trg_pedido_cancelado").unwrap().replace('"', "").contains("new.situacao"));
    // o CHECK também foi para o nome novo
    let falha = conn.execute("UPDATE pedidos SET situacao = 'perdido' WHERE id = 3", []).unwrap_err();
    assert!(
        matches!(falha, rusqlite::Error::SqliteFailure(ref f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation),
        "{falha}"
    );
    conn.execute("UPDATE pedidos SET situacao = 'cancelado' WHERE id = 3", []).unwrap();
    assert!(observacao(conn, 3).contains("cancelado"));
}

#[test]
fn renomear_coluna_para_nome_que_ja_existe() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { nome: "EMAIL".into(), ..definicao(conn, "clientes", "cidade") };
    falha_com(plano_alterar_coluna(conn, "clientes", "cidade", &nova), "já tem uma coluna");
}

// ---------------------------------------------------------------------------------------------------
// Adicionar, excluir e reordenar colunas

#[test]
fn adicionar_coluna_not_null_default_0_nativo() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { nao_nulo: true, padrao: Some("0".into()), ..col("peso", "INTEGER") };
    let plano = aplicar(conn, || plano_adicionar_coluna(conn, "eventos", &nova));
    assert!(!plano.reconstroi);
    assert_eq!(plano.comandos, textos(&["ALTER TABLE \"eventos\" ADD COLUMN \"peso\" INTEGER NOT NULL DEFAULT 0"]));
    assert_eq!(inteiro(conn, "SELECT count(*) FROM eventos WHERE peso = 0"), N_EVENTOS);
}

#[test]
fn adicionar_coluna_unique_reconstroi() {
    let a = ambiente();
    let conn = &a.conn;
    let antes = valores(conn, "SELECT * FROM clientes ORDER BY id");
    let seq = inteiro(conn, "SELECT seq FROM sqlite_sequence WHERE name = 'clientes'");
    let nova = ColunaDef { unica: true, ..col("documento", "TEXT") };
    let plano = aplicar(conn, || plano_adicionar_coluna(conn, "clientes", &nova));
    assert!(plano.reconstroi && plano.descricao.contains("A tabela será reconstruída."));
    let esperado: Vec<Vec<Valor>> = antes
        .into_iter()
        .map(|mut r| {
            r.push(Valor::Nulo);
            r
        })
        .collect();
    assert_eq!(valores(conn, "SELECT * FROM clientes ORDER BY id"), esperado);
    assert_eq!(nomes_colunas(conn, "clientes").last().map(String::as_str), Some("documento"));
    let unicos = valores(conn, "PRAGMA index_list(clientes)").into_iter().filter(|r| r[3] == texto("u")).count();
    assert_eq!(unicos, 2);
    assert_eq!(inteiro(conn, "SELECT seq FROM sqlite_sequence WHERE name = 'clientes'"), seq);
    assert_eq!(contar(conn, "v_pedidos_por_cliente"), N_CLIENTES);
}

#[test]
fn adicionar_coluna_com_padrao_nao_constante_reconstroi() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { padrao: Some("CURRENT_TIMESTAMP".into()), ..col("registrado_em", "TEXT") };
    let plano = aplicar(conn, || plano_adicionar_coluna(conn, "eventos", &nova));
    assert!(plano.reconstroi);
    assert_eq!(inteiro(conn, "SELECT count(*) FROM eventos WHERE registrado_em IS NOT NULL"), N_EVENTOS);
    // eventos não tem INTEGER PRIMARY KEY: o rowid de cada linha é mantido
    assert_eq!(
        valores(conn, "SELECT min(rowid), max(rowid) FROM eventos"),
        vec![vec![Valor::Inteiro(1), Valor::Inteiro(N_EVENTOS)]]
    );
}

#[test]
fn adicionar_coluna_not_null_sem_padrao_em_tabela_com_linhas() {
    let a = ambiente();
    let conn = &a.conn;
    let sem_padrao = ColunaDef { nao_nulo: true, ..col("x", "TEXT") };
    falha_com(plano_adicionar_coluna(conn, "eventos", &sem_padrao), "precisa de um valor padrão");
    falha_com(plano_adicionar_coluna(conn, "eventos", &col("TIPO", "TEXT")), "já tem uma coluna");
}

#[test]
fn excluir_coluna_livre_nativo() {
    let a = ambiente();
    let conn = &a.conn;
    let antes = valores(conn, "SELECT rowid, momento, tipo, valor FROM eventos ORDER BY rowid");
    let plano = aplicar(conn, || plano_excluir_coluna(conn, "eventos", "dados"));
    assert!(!plano.reconstroi && plano.comandos == textos(&["ALTER TABLE \"eventos\" DROP COLUMN \"dados\""]));
    assert!(plano.avisos.contains(&"Colunas removidas perdem os dados.".to_string()));
    assert_eq!(nomes_colunas(conn, "eventos"), textos(&["momento", "tipo", "valor"]));
    assert_eq!(valores(conn, "SELECT rowid, momento, tipo, valor FROM eventos ORDER BY rowid"), antes);
}

#[test]
fn excluir_coluna_indexada_nomeia_o_indice() {
    let a = ambiente();
    let e = erro(plano_excluir_coluna(&a.conn, "pedidos", "cliente_id"));
    assert_eq!(e.mensagem, "A coluna cliente_id é usada pelo índice idx_pedidos_cliente. Exclua o índice antes.");
}

#[test]
fn excluir_coluna_com_impedimento() {
    let a = ambiente();
    let conn = &a.conn;
    for (tabela, coluna, trecho) in [
        ("clientes", "id", "chave primária"),
        ("itens_pedido", "produto", "chave primária"),
        ("clientes", "email", "UNIQUE"),
        ("clientes", "nome", "índice idx_clientes_nome_cidade"),
        ("pedidos", "feito_em", "índice idx_pedidos_status_data"),
        ("produtos", "preco", "coluna gerada preco_centavos"),
        ("pedidos", "observacao", "gatilho trg_pedido_cancelado"),
    ] {
        let antes = retrato(conn);
        falha_com(plano_excluir_coluna(conn, tabela, coluna), trecho);
        assert_eq!(retrato(conn), antes);
    }
}

#[test]
fn excluir_coluna_com_chave_estrangeira_e_visao() {
    let a = ambiente();
    let conn = &a.conn;
    executar(conn, &crate::plano_excluir_indice(conn, "idx_pedidos_cliente").unwrap()).unwrap();
    falha_com(plano_excluir_coluna(conn, "pedidos", "cliente_id"), "chave estrangeira para clientes");
    let visao = plano_criar_visao(conn, "v_tipos", "SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo").unwrap();
    executar(conn, &visao).unwrap();
    let e = erro(plano_excluir_coluna(conn, "eventos", "tipo"));
    assert_eq!(e.mensagem, "A coluna tipo é usada pela visão v_tipos. Exclua a visão antes.");
}

#[test]
fn excluir_coluna_citada_em_restricao_da_tabela_ou_em_check_de_outra() {
    let a = ambiente();
    let conn = &a.conn;
    let colunas = [
        col("de", "INTEGER"),
        col("ate", "INTEGER"),
        ColunaDef { verificacao: Some("passo <= ate".into()), ..col("passo", "INTEGER") },
        col("nota", "TEXT"),
    ];
    executar(conn, &plano_criar_tabela(conn, "faixas", &colunas, &textos(&["CHECK (de < ate)"]), false).unwrap())
        .unwrap();
    falha_com(plano_excluir_coluna(conn, "faixas", "de"), "restrição da tabela «CHECK (de < ate)»");
    falha_com(plano_excluir_coluna(conn, "faixas", "ate"), "restrição da tabela");
    executar(conn, &plano_excluir_coluna(conn, "faixas", "nota").unwrap()).unwrap();
    assert_eq!(nomes_colunas(conn, "faixas"), textos(&["de", "ate", "passo"]));
}

#[test]
fn excluir_coluna_gravada_por_gatilho_de_outra_tabela_e_recusada() {
    // O DROP COLUMN do SQLite aceita (ele não confere a lista do INSERT nem o SET do UPDATE), e depois
    // todo INSERT em pedidos quebrava com "table auditoria has no column named mensagem".
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(
        "
        CREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER, mensagem TEXT, visto INTEGER);
        CREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN
            INSERT INTO auditoria (pedido, mensagem) VALUES (new.id, 'criado');
        END;
        CREATE TRIGGER tg_pedidos_visto AFTER UPDATE ON pedidos BEGIN
            UPDATE auditoria SET visto = 1 WHERE pedido = new.id;
        END;
    ",
    )
    .unwrap();
    let antes = retrato(conn);
    let e = erro(plano_excluir_coluna(conn, "auditoria", "mensagem"));
    assert_eq!(
        e.mensagem,
        "A coluna mensagem é usada pelo gatilho tg_pedidos_auditoria (de pedidos). Exclua o gatilho antes."
    );
    falha_com(plano_excluir_coluna(conn, "auditoria", "visto"), "gatilho tg_pedidos_visto (de pedidos)");
    assert_eq!(retrato(conn), antes);
}

#[test]
fn excluir_coluna_sem_falso_positivo_de_palavra_chave_ou_homonima() {
    // "key" aparecia na restrição «PRIMARY KEY (id)» e total de pedidos "era usada" pelo gatilho que
    // grava clientes.total; o SQLite exclui as duas sem quebrar nada.
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(
        "
        CREATE TABLE configs (id INTEGER NOT NULL, \"key\" VARCHAR(50), valor TEXT, total REAL,
            PRIMARY KEY (id), FOREIGN KEY (id) REFERENCES clientes (id));
        CREATE TABLE somas (id INTEGER PRIMARY KEY, total REAL);
        CREATE TRIGGER tg_soma AFTER INSERT ON configs BEGIN
            UPDATE somas SET total = coalesce(total, 0) + 1 WHERE id = new.id;
        END;
        INSERT INTO somas VALUES (1, 0);
    ",
    )
    .unwrap();
    aplicar(conn, || plano_excluir_coluna(conn, "configs", "key"));
    aplicar(conn, || plano_excluir_coluna(conn, "configs", "total"));
    assert_eq!(nomes_colunas(conn, "configs"), textos(&["id", "valor"]));
    conn.execute("INSERT INTO configs VALUES (1, 'x')", []).unwrap();
    assert_eq!(valores(conn, "SELECT total FROM somas"), vec![vec![Valor::Real(1.0)]]);
    // o uso de verdade (o SET do gatilho grava em somas.total) continua recusado
    falha_com(plano_excluir_coluna(conn, "somas", "total"), "gatilho tg_soma");
}

#[test]
fn reordenar_colunas() {
    let a = ambiente();
    let conn = &a.conn;
    let consulta = "SELECT rowid, momento, tipo, valor, dados FROM eventos ORDER BY rowid";
    let antes = valores(conn, consulta);
    let ordem = ["dados", "tipo", "momento", "valor"];
    let plano = aplicar(conn, || plano_reordenar_colunas(conn, "eventos", &ordem));
    assert!(plano.reconstroi);
    assert_eq!(nomes_colunas(conn, "eventos"), textos(&ordem));
    assert_eq!(valores(conn, consulta), antes);
}

#[test]
fn reordenar_tabela_sem_rowid_com_coluna_gerada() {
    let a = ambiente();
    let conn = &a.conn;
    let antes = valores(conn, "SELECT * FROM produtos ORDER BY codigo");
    let ordem = ["preco_centavos", "serie", "foto", "estoque", "preco", "descricao", "codigo"];
    aplicar(conn, || plano_reordenar_colunas(conn, "produtos", &ordem));
    assert_eq!(nomes_colunas(conn, "produtos"), textos(&ordem));
    let depois = valores(
        conn,
        "SELECT codigo, descricao, preco, estoque, foto, serie, preco_centavos FROM produtos ORDER BY codigo",
    );
    assert_eq!(depois, antes);
}

#[test]
fn reordenar_valida_a_lista() {
    let a = ambiente();
    let conn = &a.conn;
    falha_com(plano_reordenar_colunas(conn, "eventos", &["dados", "tipo", "momento"]), "mesmas colunas");
    falha_com(plano_reordenar_colunas(conn, "eventos", &["dados", "tipo", "momento", "momento"]), "mesmas colunas");
    assert!(
        plano_reordenar_colunas(conn, "eventos", &["momento", "tipo", "valor", "dados"]).unwrap().comandos.is_empty()
    );
}

// ---------------------------------------------------------------------------------------------------
// Casos a mais

#[test]
fn alterar_coluna_gerada_e_rowid_preservado() {
    // Passar uma coluna a ser calculada avisa que os valores somem e mantém o rowid interno de eventos.
    let a = ambiente();
    let conn = &a.conn;
    let antes = valores(conn, "SELECT rowid, dados FROM eventos ORDER BY rowid");
    let gerada = Gerada { expressao: "length(dados)".into(), armazenada: false };
    let nova = ColunaDef { gerada: Some(gerada), ..definicao(conn, "eventos", "valor") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "eventos", "valor", &nova));
    assert!(plano.avisos.iter().any(|a| a.contains("passa a ser calculada")));
    assert_eq!(valores(conn, "SELECT rowid, dados FROM eventos ORDER BY rowid"), antes);
    assert_eq!(inteiro(conn, "SELECT count(*) FROM eventos WHERE valor = length(dados)"), N_EVENTOS);
}

#[test]
fn plano_sem_begin_roda_dentro_da_transacao_da_pessoa() {
    // Plano sem BEGIN roda dentro da transação aberta pela pessoa, que continua aberta e decide no fim.
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch("BEGIN").unwrap();
    let plano = plano_excluir_indice(conn, "idx_pedidos_cliente").unwrap();
    executar(conn, &plano).unwrap();
    assert!(!conn.is_autocommit());
    assert!(sql_de(conn, "idx_pedidos_cliente").is_none());
    conn.execute_batch("ROLLBACK").unwrap();
    assert!(sql_de(conn, "idx_pedidos_cliente").is_some());
}
