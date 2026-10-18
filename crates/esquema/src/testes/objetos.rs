//! Índices, visões e gatilhos.

use comum::Valor;

use super::*;
use crate::{
    ColunaIndice, plano_criar_gatilho, plano_criar_indice, plano_criar_visao, plano_excluir_gatilho,
    plano_excluir_indice, plano_excluir_visao,
};

#[test]
fn criar_e_excluir_indices_unico_desc_e_parcial() {
    let a = ambiente();
    let conn = &a.conn;
    let plano = aplicar(conn, || {
        plano_criar_indice(conn, "idx_produtos_descricao", "produtos", &[ColunaIndice::nova("descricao")], true, None)
    });
    assert_eq!(
        plano.comandos,
        textos(&["CREATE UNIQUE INDEX \"idx_produtos_descricao\" ON \"produtos\" (\"descricao\")"])
    );
    let colunas = [ColunaIndice::nova("tipo"), ColunaIndice { nome: "momento".into(), desc: true }];
    aplicar(conn, || plano_criar_indice(conn, "idx_eventos_momento", "eventos", &colunas, false, None));
    let plano = aplicar(conn, || {
        plano_criar_indice(conn, "idx_pedidos_pagos", "pedidos", &["feito_em".into()], false, Some("status = 'pago'"))
    });
    assert!(plano.descricao.contains("só nas linhas em que status = 'pago'"));
    let lista = valores(conn, "PRAGMA index_list(produtos)");
    let unico = lista.iter().find(|r| r[1] == texto("idx_produtos_descricao")).unwrap();
    assert_eq!(unico[2], Valor::Inteiro(1));
    let desc: Vec<(Valor, Valor)> = valores(conn, "PRAGMA index_xinfo(idx_eventos_momento)")
        .into_iter()
        .filter(|r| r[5] != Valor::Inteiro(0))
        .map(|r| (r[2].clone(), r[3].clone()))
        .collect();
    assert_eq!(desc, vec![(texto("tipo"), Valor::Inteiro(0)), (texto("momento"), Valor::Inteiro(1))]);
    let pedidos = valores(conn, "PRAGMA index_list(pedidos)");
    let parcial = pedidos.iter().find(|r| r[1] == texto("idx_pedidos_pagos")).unwrap();
    assert_eq!(parcial[4], Valor::Inteiro(1));
    for nome in ["idx_produtos_descricao", "idx_eventos_momento", "idx_pedidos_pagos"] {
        aplicar(conn, || plano_excluir_indice(conn, nome));
        assert!(sql_de(conn, nome).is_none());
    }
}

#[test]
fn criar_indice_valida() {
    let a = ambiente();
    let conn = &a.conn;
    let tipo = [ColunaIndice::nova("tipo")];
    falha_com(plano_criar_indice(conn, "idx_tipo", "eventos", &tipo, true, None), "valores repetidos");
    falha_com(
        plano_criar_indice(conn, "idx_x", "eventos", &[ColunaIndice::nova("inexistente")], false, None),
        "não existe",
    );
    falha_com(plano_criar_indice(conn, "CLIENTES", "eventos", &tipo, false, None), "Já existe uma tabela");
    falha_com(plano_criar_indice(conn, "idx_x", "eventos", &[], false, None), "ao menos uma coluna");
}

#[test]
fn excluir_indice_automatico_e_recusado() {
    let a = ambiente();
    let e = erro(plano_excluir_indice(&a.conn, "sqlite_autoindex_clientes_1"));
    assert_eq!(
        e.mensagem,
        "Esse índice vem de uma restrição UNIQUE ou da chave primária; ele sai junto quando a restrição sai."
    );
}

#[test]
fn criar_e_excluir_visao() {
    let a = ambiente();
    let conn = &a.conn;
    let plano = aplicar(conn, || {
        plano_criar_visao(conn, "v_eventos_por_tipo", "SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo;")
    });
    assert_eq!(
        plano.comandos,
        textos(&["CREATE VIEW \"v_eventos_por_tipo\" AS\nSELECT tipo, count(*) AS n FROM eventos GROUP BY tipo"])
    );
    assert_eq!(contar(conn, "v_eventos_por_tipo"), 4);
    aplicar(conn, || plano_excluir_visao(conn, "v_eventos_por_tipo"));
    assert!(sql_de(conn, "v_eventos_por_tipo").is_none());
}

#[test]
fn criar_visao_valida_o_select() {
    let a = ambiente();
    for (sql, trecho) in [
        ("DELETE FROM eventos", "único comando SELECT"),
        ("SELECT 1; SELECT 2", "único comando SELECT"),
        ("SELECT * FROM tabela_que_nao_existe", "não é válido"),
        ("", "Escreva"),
    ] {
        falha_com(plano_criar_visao(&a.conn, "v_x", sql), trecho);
    }
}

#[test]
fn criar_e_excluir_gatilho() {
    let a = ambiente();
    let conn = &a.conn;
    let sql = "CREATE TRIGGER trg_evento_erro AFTER INSERT ON eventos WHEN new.tipo = 'erro'\n\
               BEGIN\n    UPDATE eventos SET dados = 'marcado; revisar' WHERE rowid = new.rowid;\nEND;";
    aplicar(conn, || plano_criar_gatilho(conn, sql));
    conn.execute("INSERT INTO eventos (momento, tipo) VALUES ('2026-09-22T10:00:00', 'erro')", []).unwrap();
    assert_eq!(
        valores(conn, "SELECT dados FROM eventos ORDER BY rowid DESC LIMIT 1"),
        vec![vec![texto("marcado; revisar")]]
    );
    let plano = aplicar(conn, || plano_excluir_gatilho(conn, "trg_evento_erro"));
    assert_eq!(plano.comandos, textos(&["DROP TRIGGER \"trg_evento_erro\""]));
    aplicar(conn, || plano_excluir_gatilho(conn, "trg_pedido_cancelado"));
    assert_eq!(inteiro(conn, "SELECT count(*) FROM sqlite_schema WHERE type = 'trigger'"), 0);
    falha_com(plano_excluir_gatilho(conn, "trg_pedido_cancelado"), "não existe");
}

// ---------------------------------------------------------------------------------------------------
// NUL e parâmetros no SQL digitado

const NUL: &str = "O SQLite recusou a operação: the query contains a null character.";

#[test]
fn visao_e_indice_com_nul_sao_recusados_e_nada_e_criado() {
    // O SQLite para de ler no NUL: antes, a visão era criada como "SELECT * FROM eventos" (5.000 linhas em vez
    // de nenhuma) e o índice com "WHERE tipo = 'a'".
    let a = ambiente();
    let conn = &a.conn;
    let antes = retrato(conn);
    let e = erro(plano_criar_visao(conn, "v_n", "SELECT * FROM eventos\0 WHERE 0"));
    assert_eq!(e.mensagem, format!("O SELECT da visão não é válido. {NUL}"));
    let tipo = [ColunaIndice::nova("tipo")];
    let e = erro(plano_criar_indice(conn, "idx_n", "eventos", &tipo, false, Some("tipo = 'a'\0 AND 0")));
    assert_eq!(e.mensagem, format!("Não foi possível criar o índice. {NUL}"));
    assert_eq!(retrato(conn), antes);
    assert!(sql_de(conn, "v_n").is_none() && sql_de(conn, "idx_n").is_none());
    assert!(conn.is_autocommit());
}

#[test]
fn gatilho_com_nul_e_recusado() {
    // Lá, o sqlite3.complete_statement levantava ValueError (erro inesperado); aqui, a frase dos outros textos
    // com NUL, antes de conferir se o comando está completo (o NUL entre "; " e "END" o deixaria incompleto).
    let a = ambiente();
    let conn = &a.conn;
    for sql in [
        "CREATE TRIGGER t_n AFTER INSERT ON eventos BEGIN SELECT 1;\0 END",
        "CREATE TRIGGER t_n AFTER INSERT ON eventos BEGIN SELECT 1\0; END",
    ] {
        let e = erro(plano_criar_gatilho(conn, sql));
        assert_eq!(e.mensagem, format!("Não foi possível criar o gatilho. {NUL}"));
        assert_eq!(e.status, 400);
    }
    assert!(sql_de(conn, "t_n").is_none());
}

#[test]
fn visao_com_parametro_e_recusada() {
    let a = ambiente();
    for (sql, n) in [("SELECT ?", 1), ("SELECT :x FROM eventos", 1), ("SELECT ?, ?3", 3)] {
        let e = erro(plano_criar_visao(&a.conn, "v_p", sql));
        assert_eq!(
            e.mensagem,
            format!(
                "O SELECT da visão não é válido. O SQLite recusou a operação: Incorrect number of bindings supplied. \
                 The current statement uses {n}, and there are 0 supplied.."
            )
        );
    }
}
