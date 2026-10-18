//! definicoes_tabela, o próprio Plano e a alteração de coluna (reconstrução).

use amostras::{N_CLIENTES, N_ITENS_PEDIDO, N_PEDIDOS, N_PRODUTOS};
use comum::Valor;
use ddl::ColunaDef;

use super::*;
use crate::plano::executar_observado;
use crate::sql::Observador;
use crate::{
    ColunaIndice, plano_adicionar_coluna, plano_alterar_coluna, plano_criar_indice, plano_criar_tabela,
    plano_criar_visao, plano_excluir_coluna, plano_renomear_tabela, plano_reordenar_colunas,
};

const MARCAS: &str = "
    CREATE TABLE marcas (id INTEGER PRIMARY KEY, sigla TEXT UNIQUE COLLATE NOCASE);
    CREATE TABLE modelos (id INTEGER PRIMARY KEY, sigla TEXT REFERENCES marcas (sigla));
    INSERT INTO marcas VALUES (1, 'ABC');
    INSERT INTO modelos VALUES (1, 'abc');
";

// ---------------------------------------------------------------------------------------------------
// definicoes_tabela e o próprio Plano

#[test]
fn definicoes_de_clientes() {
    let a = ambiente();
    let conn = &a.conn;
    let d = definicoes_tabela(conn, "clientes").unwrap();
    assert!(!d.sem_rowid && !d.estrita && d.restricoes.is_empty());
    let id =
        ColunaDef { nome: "id".into(), tipo: "INTEGER".into(), pk: true, autoincremento: true, ..ColunaDef::default() };
    assert_eq!(d.colunas[0], id);
    assert!(definicoes_tabela(conn, "produtos").unwrap().sem_rowid);
    assert_eq!(definicoes_tabela(conn, "CLIENTES").unwrap().colunas[1].nome, "nome");
}

#[test]
fn definicoes_recusa_o_que_nao_e_tabela_editavel() {
    let a = ambiente();
    for (nome, trecho) in [
        ("v_pedidos_por_cliente", "visão"),
        ("notas", "virtual"),
        ("notas_data", "auxiliar"),
        ("sqlite_sequence", "interna"),
        ("nao_existe", "não existe"),
    ] {
        falha_com(definicoes_tabela(&a.conn, nome), trecho);
    }
}

#[test]
fn sql_do_plano() {
    let p = Plano::novo("x", textos(&["BEGIN", "SELECT 1", "COMMIT"]));
    assert_eq!(p.sql(), "BEGIN;\n\nSELECT 1;\n\nCOMMIT;");
    assert_eq!(Plano::novo("nada", Vec::new()).sql(), "");
}

/// Executa o plano anotando cada comando que o SQLite preparou e começou a rodar (o texto do próprio SQLite, do SQLite).
fn executar_rastreado(conn: &Connection, plano: &Plano) -> (comum::Resultado<()>, Vec<String>) {
    let mut rodados: Vec<String> = Vec::new();
    let mut anotar = |s: &str| rodados.push(s.to_string());
    let resultado = executar_observado(conn, plano, &mut Observador::novo(&mut anotar));
    (resultado, rodados)
}

/// Os comandos rastreados sem os PRAGMAs de sessão (lidos, ligados e restaurados por `executar`).
fn sem_sessao(rodados: &[String]) -> (Vec<String>, Vec<String>) {
    rodados.iter().cloned().partition(|s| {
        let alto = s.to_uppercase();
        !(alto.starts_with("PRAGMA FOREIGN_KEYS") || alto.starts_with("PRAGMA LEGACY_ALTER_TABLE"))
    })
}

#[test]
fn previa_mostra_exatamente_o_que_executar_roda() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { tipo: "VARCHAR(1000)".into(), ..definicao(conn, "pedidos", "observacao") };
    let plano = previa_sem_efeito(conn, || plano_alterar_coluna(conn, "pedidos", "observacao", &nova));
    let (resultado, rodados) = executar_rastreado(conn, &plano);
    resultado.unwrap();
    let (resto, sessao) = sem_sessao(&rodados);
    assert_eq!(resto, plano.comandos);
    assert!(sessao.iter().any(|s| s.contains("OFF")) && sessao.iter().any(|s| s.contains("legacy_alter_table = ON")));
    assert!(plano.avisos.iter().any(|a| a.contains("foreign_keys = OFF")));
    assert!(!plano.comandos.iter().any(|c| {
        let alto = c.to_uppercase();
        alto.starts_with("PRAGMA FOREIGN_KEYS") || alto.starts_with("PRAGMA LEGACY")
    }));
}

#[test]
fn previa_mostra_exatamente_o_que_executar_roda_em_outros_planos() {
    // O mesmo, com o SQL que a pessoa digita indo para o comando: SELECT da visão, condição do índice,
    // DEFAULT, renomear só mudando maiúsculas (BEGIN … COMMIT) e criar tabela com restrição.
    let a = ambiente();
    let conn = &a.conn;
    let fabricas: Vec<Box<dyn Fn() -> comum::Resultado<Plano> + '_>> = vec![
        Box::new(|| {
            plano_criar_visao(conn, "v_n", "SELECT tipo, count(*) AS n FROM eventos -- por tipo\nGROUP BY tipo")
        }),
        Box::new(|| {
            plano_criar_indice(
                conn,
                "ix_p",
                "pedidos",
                &[ColunaIndice::nova("feito_em")],
                false,
                Some("status = 'pago'"),
            )
        }),
        Box::new(|| {
            let peso = ColunaDef { nao_nulo: true, padrao: Some("0".into()), ..col("peso", "INTEGER") };
            plano_adicionar_coluna(conn, "eventos", &peso)
        }),
        Box::new(|| plano_renomear_tabela(conn, "eventos", "Eventos")),
        Box::new(|| {
            plano_criar_tabela(
                conn,
                "faixas",
                &[col("de", "INTEGER"), col("ate", "INTEGER")],
                &["CHECK (de < ate)"],
                false,
            )
        }),
    ];
    for fabrica in &fabricas {
        let plano = previa_sem_efeito(conn, fabrica);
        let (resultado, rodados) = executar_rastreado(conn, &plano);
        resultado.unwrap_or_else(|e| panic!("{}: {e}", plano.descricao));
        assert_eq!(sem_sessao(&rodados).0, plano.comandos, "{}", plano.descricao);
        confere_sessao_restaurada(conn);
    }
}

#[test]
fn rastreio_mostra_o_texto_que_o_sqlite_preparou() {
    // O rastreio vem do comando preparado (sqlite3_expanded_sql), não do texto que o crate mandou: num plano
    // com "SELECT 1; -- fim", o SQLite prepara só "SELECT 1;" (o comentário fica de fora).
    let a = ambiente();
    let (resultado, rodados) = executar_rastreado(&a.conn, &Plano::novo("x", textos(&["SELECT 1; -- fim"])));
    resultado.unwrap();
    assert_eq!(sem_sessao(&rodados).0, vec!["SELECT 1;".to_string()]);
}

#[test]
fn comando_com_nul_ou_parametro_nao_roda_cortado() {
    // O SQLite para de ler no NUL: sem a recusa, o CREATE VIEW rodaria só até "eventos" (e traria as 5.000
    // linhas) e o rastreio, que antes vinha do texto do plano, não mostraria a diferença.
    let a = ambiente();
    let conn = &a.conn;
    let antes = retrato(conn);
    for (comando, trecho) in [
        ("CREATE VIEW \"v_n\" AS\nSELECT * FROM eventos\0 WHERE 0", "the query contains a null character"),
        (
            "CREATE INDEX \"idx_n\" ON \"eventos\" (\"tipo\") WHERE tipo = 'a'\0 AND 0",
            "the query contains a null character",
        ),
        ("SELECT ?", "Incorrect number of bindings supplied. The current statement uses 1, and there are 0 supplied."),
    ] {
        let (resultado, rodados) = executar_rastreado(conn, &Plano::novo("x", textos(&[comando])));
        let e = falha_com(resultado, trecho);
        assert!(e.mensagem.starts_with("Não foi possível aplicar a mudança; nada foi alterado. O SQLite recusou"));
        assert!(sem_sessao(&rodados).0.is_empty(), "{rodados:?}");
        assert_eq!(retrato(conn), antes);
        confere_sessao_restaurada(conn);
    }
}

// ---------------------------------------------------------------------------------------------------
// Alterar coluna (reconstrução)

fn como_real(v: &Valor) -> f64 {
    match v {
        Valor::Inteiro(i) => *i as f64,
        Valor::Real(r) => *r,
        outro => panic!("preço não numérico: {outro:?}"),
    }
}

fn acima_de_2_53(v: &Valor) -> bool {
    matches!(v, Valor::Inteiro(i) if *i > 1 << 53)
}

#[test]
fn mudar_tipo_de_produtos_preco() {
    let a = ambiente();
    let conn = &a.conn;
    let consulta =
        "SELECT codigo, descricao, preco, estoque, foto, serie, preco_centavos FROM produtos ORDER BY codigo";
    let antes = valores(conn, consulta);
    let fotos: Vec<Valor> = antes.iter().map(|r| r[4].clone()).filter(|v| !v.e_nulo()).collect();
    let grandes: Vec<Valor> = antes.iter().map(|r| r[5].clone()).filter(acima_de_2_53).collect();
    assert!(!fotos.is_empty() && !grandes.is_empty());
    let nova = ColunaDef { tipo: "REAL".into(), ..definicao(conn, "produtos", "preco") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "produtos", "preco", &nova));
    assert!(plano.reconstroi);
    assert_eq!(plano.descricao, "Mudar a coluna preco de produtos: tipo NUMERIC → REAL. A tabela será reconstruída.");
    assert!(plano.avisos.iter().any(|a| a.contains("afinidade")));

    let depois = valores(conn, consulta);
    assert_eq!(depois.len() as i64, N_PRODUTOS);
    for (a, d) in antes.iter().zip(&depois) {
        for i in [0, 1, 3, 4, 5, 6] {
            assert_eq!(a[i], d[i]);
        }
        assert_eq!(d[2], Valor::Real(como_real(&a[2])));
    }
    assert_eq!(depois.iter().map(|r| r[5].clone()).filter(acima_de_2_53).collect::<Vec<_>>(), grandes);
    assert!(depois.iter().all(|r| matches!(r[5], Valor::Inteiro(_))));
    assert_eq!(depois.iter().map(|r| r[4].clone()).filter(|v| !v.e_nulo()).collect::<Vec<_>>(), fotos);
    let preco = coluna_xinfo(conn, "produtos", "preco");
    assert!(preco.tipo == "REAL" && preco.notnull == 1 && preco.padrao.as_deref() == Some("0"));
    assert_eq!(coluna_xinfo(conn, "produtos", "preco_centavos").hidden, 2); // coluna gerada virtual continua gerada
    let lista = valores(conn, "PRAGMA table_list");
    let produtos = lista.iter().find(|r| r[1] == texto("produtos")).unwrap();
    assert_eq!(produtos[4], Valor::Inteiro(1)); // WITHOUT ROWID
    assert_eq!(contar(conn, "itens_pedido"), N_ITENS_PEDIDO);
    assert!(sql_de(conn, "produtos").unwrap().contains("COLLATE NOCASE"));
}

#[test]
fn tornar_cidade_not_null_funciona() {
    let a = ambiente();
    let conn = &a.conn;
    let seq = inteiro(conn, "SELECT seq FROM sqlite_sequence WHERE name = 'clientes'");
    let antes = valores(conn, "SELECT * FROM clientes ORDER BY id");
    let nova = ColunaDef { nao_nulo: true, ..definicao(conn, "clientes", "cidade") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "clientes", "cidade", &nova));
    assert!(plano.reconstroi && plano.descricao.contains("passa a não aceitar nulo"));
    assert_eq!(valores(conn, "SELECT * FROM clientes ORDER BY id"), antes);
    let cidade = coluna_xinfo(conn, "clientes", "cidade");
    assert!(cidade.notnull == 1 && cidade.padrao.as_deref() == Some("'São Paulo'"));
    assert_eq!(inteiro(conn, "SELECT seq FROM sqlite_sequence WHERE name = 'clientes'"), seq);
    // visão que cita clientes, índice parcial e o índice do UNIQUE continuam de pé
    assert_eq!(contar(conn, "v_pedidos_por_cliente"), N_CLIENTES);
    assert!(sql_de(conn, "idx_clientes_nome_cidade").unwrap().contains("WHERE ativo = 1"));
    let nomes = indices(conn, "clientes");
    assert!(nomes.contains("idx_clientes_nome_cidade") && nomes.contains("sqlite_autoindex_clientes_1"));
    assert!(contar(conn, "pedidos") == N_PEDIDOS && contar(conn, "itens_pedido") == N_ITENS_PEDIDO);
}

#[test]
fn tornar_limite_not_null_falha_e_nada_muda() {
    let a = ambiente();
    let conn = &a.conn;
    let nulos = inteiro(conn, "SELECT count(*) FROM clientes WHERE limite IS NULL");
    assert!(nulos > 0);
    let nova = ColunaDef { nao_nulo: true, ..definicao(conn, "clientes", "limite") };
    let plano = previa_sem_efeito(conn, || plano_alterar_coluna(conn, "clientes", "limite", &nova));
    let trecho = format!("Há {nulos} linhas com limite nulo");
    assert!(plano.avisos.iter().any(|a| a.contains(&trecho) && a.contains("impedem NOT NULL")));
    let antes = retrato(conn);
    let e = erro(executar(conn, &plano));
    assert!(e.mensagem.contains("limite") && e.mensagem.contains("nulo"));
    assert_eq!(retrato(conn), antes);
    confere_sessao_restaurada(conn);
}

#[test]
fn alterar_clientes_preserva_autoincremento_e_sqlite_sequence() {
    let a = ambiente();
    let conn = &a.conn;
    // contador acima do maior id: como se as últimas linhas tivessem sido excluídas
    conn.execute("UPDATE sqlite_sequence SET seq = 500 WHERE name = 'clientes'", []).unwrap();
    let nova = ColunaDef { tipo: "VARCHAR(200)".into(), ..definicao(conn, "clientes", "email") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "clientes", "email", &nova));
    assert!(plano.comandos.iter().any(|c| c.contains("sqlite_sequence")));
    assert!(sql_de(conn, "clientes").unwrap().contains("AUTOINCREMENT"));
    assert_eq!(
        valores(conn, "SELECT name, seq FROM sqlite_sequence"),
        vec![vec![texto("clientes"), Valor::Inteiro(500)]]
    );
    conn.execute("INSERT INTO clientes (nome, email) VALUES ('Cliente Novo', 'novo@example.test')", []).unwrap();
    assert_eq!(inteiro(conn, "SELECT max(id) FROM clientes"), 501);
}

#[test]
fn alterar_pedidos_preserva_gatilho_indices_e_fks_de_itens() {
    let a = ambiente();
    let conn = &a.conn;
    let antes_pedidos = valores(conn, "SELECT * FROM pedidos ORDER BY id");
    let antes_itens = valores(conn, "SELECT * FROM itens_pedido ORDER BY pedido_id, produto");
    let fks_itens = valores(conn, "PRAGMA foreign_key_list(itens_pedido)");
    let fks_pedidos = valores(conn, "PRAGMA foreign_key_list(pedidos)");
    let nomes_indices = ["idx_pedidos_cliente", "idx_pedidos_status_data"];
    let sql_indices: Vec<Option<String>> = nomes_indices.iter().map(|n| sql_de(conn, n)).collect();
    let sql_gatilho = sql_de(conn, "trg_pedido_cancelado");
    let nova = ColunaDef { tipo: "VARCHAR(1000)".into(), ..definicao(conn, "pedidos", "observacao") };
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "pedidos", "observacao", &nova));
    assert!(plano.comandos.contains(&"PRAGMA foreign_key_check(\"itens_pedido\")".to_string()));

    assert_eq!(valores(conn, "SELECT * FROM pedidos ORDER BY id"), antes_pedidos);
    assert_eq!(valores(conn, "SELECT * FROM itens_pedido ORDER BY pedido_id, produto"), antes_itens);
    assert_eq!(valores(conn, "PRAGMA foreign_key_list(itens_pedido)"), fks_itens);
    assert_eq!(valores(conn, "PRAGMA foreign_key_list(pedidos)"), fks_pedidos);
    assert_eq!(nomes_indices.iter().map(|n| sql_de(conn, n)).collect::<Vec<_>>(), sql_indices);
    let desc: Vec<(Valor, Valor)> = valores(conn, "PRAGMA index_xinfo(idx_pedidos_status_data)")
        .into_iter()
        .filter(|r| r[5] != Valor::Inteiro(0))
        .map(|r| (r[2].clone(), r[3].clone()))
        .collect();
    assert_eq!(desc, vec![(texto("status"), Valor::Inteiro(0)), (texto("feito_em"), Valor::Inteiro(1))]);
    assert_eq!(sql_de(conn, "trg_pedido_cancelado"), sql_gatilho);
    assert_eq!(coluna_xinfo(conn, "pedidos", "observacao").tipo, "VARCHAR(1000)");
    // o gatilho funciona
    conn.execute("UPDATE pedidos SET status = 'cancelado' WHERE id = 1", []).unwrap();
    let obs: String = conn.query_row("SELECT observacao FROM pedidos WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert!(obs.contains("cancelado"));
    // e o ON DELETE CASCADE de itens_pedido → pedidos também
    let n_itens_1 = inteiro(conn, "SELECT count(*) FROM itens_pedido WHERE pedido_id = 1");
    conn.execute("DELETE FROM pedidos WHERE id = 1", []).unwrap();
    assert_eq!(contar(conn, "itens_pedido"), N_ITENS_PEDIDO - n_itens_1);
}

#[test]
fn reconstrucao_desfaz_tudo_se_a_chave_estrangeira_quebrar() {
    // A colação NOCASE do pai é a que a chave estrangeira usa: sem ela, 'abc' deixa de achar 'ABC'.
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(MARCAS).unwrap();
    let nova = ColunaDef { colacao: None, ..definicao(conn, "marcas", "sigla") };
    let plano = previa_sem_efeito(conn, || plano_alterar_coluna(conn, "marcas", "sigla", &nova));
    let antes = retrato(conn);
    falha_com(executar(conn, &plano), "1 linha de modelos apontando para linhas que não existem em marcas");
    assert_eq!(retrato(conn), antes);
    assert!(inteiro(conn, "PRAGMA foreign_keys") == 1 && conn.is_autocommit());
}

#[test]
fn violacao_que_ja_existia_nao_bloqueia_a_reconstrucao() {
    // Antes, a linha órfã gravada com as chaves desligadas fazia toda reconstrução de pedidos (e das
    // filhas) falhar com "A mudança deixaria…", sem nada na prévia.
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF; INSERT INTO itens_pedido VALUES (99999, 'P001', 1, 1.0); PRAGMA foreign_keys = ON;",
    )
    .unwrap();
    let orfas = valores(conn, "PRAGMA foreign_key_check");
    let nova = ColunaDef { tipo: "VARCHAR(10)".into(), ..definicao(conn, "pedidos", "observacao") };
    let plano = previa_sem_efeito(conn, || plano_alterar_coluna(conn, "pedidos", "observacao", &nova));
    assert!(
        plano
            .avisos
            .iter()
            .any(|a| a.contains("já tem 1 linha de itens_pedido apontando para linhas que não existem em pedidos"))
    );
    executar(conn, &plano).unwrap();
    assert_eq!(xinfo(conn, "pedidos")[4].tipo, "VARCHAR(10)");
    assert_eq!(valores(conn, "PRAGMA foreign_key_check"), orfas);
    // a mudança em outra tabela ligada também passa
    let nova = ColunaDef { tipo: "REAL".into(), ..definicao(conn, "itens_pedido", "preco") };
    executar(conn, &plano_alterar_coluna(conn, "itens_pedido", "preco", &nova).unwrap()).unwrap();
    assert_eq!(valores(conn, "PRAGMA foreign_key_check"), orfas);
    // mas uma violação nova continua desfazendo tudo, mesmo com a antiga lá
    conn.execute_batch(MARCAS).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = OFF; INSERT INTO modelos VALUES (2, 'XYZ'); PRAGMA foreign_keys = ON;")
        .unwrap();
    let nova = ColunaDef { colacao: None, ..definicao(conn, "marcas", "sigla") };
    let plano = plano_alterar_coluna(conn, "marcas", "sigla", &nova).unwrap();
    falha_com(executar(conn, &plano), "deixaria 1 linha de modelos");
}

#[test]
fn reconstrucao_preserva_restricao_com_comentario_de_linha() {
    // O comentário "-- dono" ia parar na mesma linha do ON DELETE CASCADE e o engolia (idem AUTOINCREMENT).
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(
        "
        CREATE TABLE categoria (id INTEGER PRIMARY KEY, nome TEXT);
        CREATE TABLE registro (
          id INTEGER CONSTRAINT pk_registro PRIMARY KEY -- chave
              AUTOINCREMENT,
          categoria_id INTEGER CONSTRAINT fk_cat REFERENCES categoria (id) -- dono
              ON DELETE CASCADE,
          nota TEXT
        );
        INSERT INTO categoria VALUES (1, 'Raiz'), (2, 'Outra');
        INSERT INTO registro (categoria_id, nota) VALUES (1, 'a'), (1, 'b'), (2, 'c');
        DELETE FROM registro WHERE id = 3;
    ",
    )
    .unwrap();
    let fk = valores(conn, "PRAGMA foreign_key_list(registro)");
    aplicar(conn, || plano_reordenar_colunas(conn, "registro", &["nota", "categoria_id", "id"]));
    let nova = ColunaDef { tipo: "VARCHAR(80)".into(), ..definicao(conn, "registro", "nota") };
    aplicar(conn, || plano_alterar_coluna(conn, "registro", "nota", &nova));
    assert_eq!(valores(conn, "PRAGMA foreign_key_list(registro)"), fk);
    assert_eq!(valores(conn, "SELECT seq FROM sqlite_sequence WHERE name = 'registro'"), vec![vec![Valor::Inteiro(3)]]);
    conn.execute("DELETE FROM categoria WHERE id = 1", []).unwrap();
    assert_eq!(inteiro(conn, "SELECT count(*) FROM registro"), 0);
    conn.execute("INSERT INTO registro (categoria_id, nota) VALUES (2, 'nova')", []).unwrap();
    assert_eq!(inteiro(conn, "SELECT id FROM registro"), 4);
}

#[test]
fn previa_e_mudanca_que_falha_nao_tocam_no_arquivo() {
    // SAVEPOINT … RELEASE fora de transação regravava o contador de mudanças do cabeçalho a cada prévia.
    let a = ambiente();
    let conn = &a.conn;
    let conteudo = || std::fs::read(&a.caminho).unwrap();
    let original = conteudo();
    let invertidos: Vec<String> = nomes_colunas(conn, "clientes").into_iter().rev().collect();
    let nome2 = ColunaDef { nome: "nome2".into(), tipo: "VARCHAR".into(), ..definicao(conn, "clientes", "nome") };
    let fabricas: Vec<Box<dyn Fn() -> comum::Resultado<Plano> + '_>> = vec![
        Box::new(|| plano_reordenar_colunas(conn, "clientes", &invertidos)),
        Box::new(|| plano_criar_indice(conn, "ix_x", "clientes", &[ColunaIndice::nova("nome")], false, None)),
        Box::new(|| plano_renomear_tabela(conn, "clientes", "clientes2")),
        Box::new(|| plano_alterar_coluna(conn, "clientes", "nome", &nome2)),
        Box::new(|| plano_adicionar_coluna(conn, "clientes", &col("x", "TEXT"))),
        Box::new(|| plano_excluir_coluna(conn, "eventos", "dados")),
        Box::new(|| plano_criar_tabela(conn, "nova", &[col("a", "")], SEM_RESTRICOES, false)),
        Box::new(|| plano_criar_visao(conn, "v_x", "SELECT 1")),
    ];
    for fabrica in &fabricas {
        previa_sem_efeito(conn, fabrica);
        assert!(conteudo() == original);
    }
    let limite = ColunaDef { nao_nulo: true, ..definicao(conn, "clientes", "limite") };
    let plano = plano_alterar_coluna(conn, "clientes", "limite", &limite).unwrap();
    falha_com(executar(conn, &plano), "nada foi alterado");
    assert!(conteudo() == original);
    // com uma transação da pessoa aberta, o ensaio usa SAVEPOINT e não a encerra
    conn.execute_batch("BEGIN").unwrap();
    let dentro = plano_reordenar_colunas(conn, "clientes", &invertidos);
    let aberta = !conn.is_autocommit();
    conn.execute_batch("ROLLBACK").unwrap();
    dentro.unwrap();
    assert!(aberta);
    assert!(conteudo() == original);
}

#[test]
fn reconstrucao_com_transacao_aberta_e_recusada() {
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { nao_nulo: true, ..definicao(conn, "eventos", "tipo") };
    let plano = plano_alterar_coluna(conn, "eventos", "tipo", &nova).unwrap();
    conn.execute_batch("BEGIN").unwrap();
    let resultado = executar(conn, &plano);
    conn.execute_batch("ROLLBACK").unwrap();
    let e = falha_com(resultado, "transação aberta");
    assert_eq!(e.status, 409);
}

#[test]
fn nada_muda_devolve_plano_vazio() {
    let a = ambiente();
    let conn = &a.conn;
    let igual = definicao(conn, "clientes", "nome");
    let plano = aplicar(conn, || plano_alterar_coluna(conn, "clientes", "nome", &igual));
    assert!(plano.comandos.is_empty() && !plano.reconstroi);
}
