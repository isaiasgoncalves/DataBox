//! Tabelas e o nome com espaço/coluna com aspas em todas as operações.

use amostras::{N_CLIENTES, N_EVENTOS};
use comum::Valor;
use ddl::{ColunaDef, Referencia};

use super::*;
use crate::{
    ColunaIndice, plano_adicionar_coluna, plano_alterar_coluna, plano_criar_indice, plano_criar_tabela,
    plano_criar_visao, plano_excluir_coluna, plano_excluir_indice, plano_excluir_tabela, plano_renomear_tabela,
    plano_reordenar_colunas,
};

#[test]
fn criar_tabela() {
    let a = ambiente();
    let conn = &a.conn;
    let colunas = [
        ColunaDef { pk: true, autoincremento: true, ..col("id", "INTEGER") },
        ColunaDef { nao_nulo: true, padrao: Some("'sem título'".into()), ..col("titulo", "TEXT") },
        ColunaDef {
            referencia: Some(Referencia {
                tabela: "clientes".into(),
                coluna: Some("id".into()),
                ao_excluir: Some("SET NULL".into()),
                ..Referencia::default()
            }),
            ..col("cliente_id", "INTEGER")
        },
    ];
    let plano = aplicar(conn, || plano_criar_tabela(conn, "tarefas", &colunas, SEM_RESTRICOES, false));
    assert_eq!(plano.descricao, "Criar a tabela tarefas com 3 colunas.");
    conn.execute("INSERT INTO tarefas (cliente_id) VALUES (1)", []).unwrap();
    assert_eq!(valores(conn, "SELECT id, titulo FROM tarefas"), vec![vec![Valor::Inteiro(1), texto("sem título")]]);
    assert_eq!(valores(conn, "PRAGMA foreign_key_list(tarefas)")[0][2], texto("clientes"));
}

#[test]
fn criar_tabela_com_chave_composta_e_sem_rowid() {
    let a = ambiente();
    let conn = &a.conn;
    let colunas = [ColunaDef { pk: true, ..col("a", "TEXT") }, ColunaDef { pk: true, ..col("b", "INTEGER") }];
    let plano = aplicar(conn, || plano_criar_tabela(conn, "pares", &colunas, SEM_RESTRICOES, true));
    assert!(plano.comandos[0].contains("PRIMARY KEY (\"a\", \"b\")") && plano.comandos[0].ends_with(") WITHOUT ROWID"));
    assert_eq!(xinfo(conn, "pares").iter().map(|r| r.pk).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn criar_tabela_valida() {
    let a = ambiente();
    let a_col = || vec![col("a", "")];
    let casos: Vec<(&str, Vec<ColunaDef>, &str)> = vec![
        ("", a_col(), "Informe o nome"),
        ("Clientes", a_col(), "Já existe uma tabela"),
        ("sqlite_minha", a_col(), "reservados"),
        ("nova", Vec::new(), "ao menos uma coluna"),
        ("nova", vec![col("a", ""), col("A", "")], "mais de uma vez"),
        ("nova", vec![col(" ", "")], "precisa de um nome"),
        ("nova", vec![ColunaDef { pk: true, autoincremento: true, ..col("a", "TEXT") }], "Autoincremento"),
        ("nova", vec![ColunaDef { autoincremento: true, ..col("a", "INTEGER") }], "Autoincremento"),
    ];
    for (nome, colunas, trecho) in casos {
        falha_com(plano_criar_tabela(&a.conn, nome, &colunas, SEM_RESTRICOES, false), trecho);
    }
}

#[test]
fn criar_tabela_sem_rowid_exige_chave() {
    let a = ambiente();
    falha_com(plano_criar_tabela(&a.conn, "nova", &[col("a", "")], SEM_RESTRICOES, true), "precisa de chave primária");
}

#[test]
fn renomear_tabela_atualiza_fks_e_visao() {
    let a = ambiente();
    let conn = &a.conn;
    let plano = aplicar(conn, || plano_renomear_tabela(conn, "clientes", "contatos"));
    assert_eq!(plano.comandos, textos(&["ALTER TABLE \"clientes\" RENAME TO \"contatos\""]));
    assert_eq!(valores(conn, "PRAGMA foreign_key_list(pedidos)")[0][2], texto("contatos"));
    assert!(sql_de(conn, "v_pedidos_por_cliente").unwrap().contains("contatos"));
    assert_eq!(contar(conn, "v_pedidos_por_cliente"), N_CLIENTES);
    assert_eq!(valores(conn, "SELECT name FROM sqlite_sequence"), vec![vec![texto("contatos")]]);
    falha_com(plano_renomear_tabela(conn, "contatos", "PEDIDOS"), "Já existe");
}

#[test]
fn renomear_tabela_so_mudando_maiusculas() {
    let a = ambiente();
    let conn = &a.conn;
    let plano = aplicar(conn, || plano_renomear_tabela(conn, "eventos", "Eventos"));
    assert!(plano.comandos[0] == "BEGIN" && plano.comandos.last().map(String::as_str) == Some("COMMIT"));
    assert_eq!(valores(conn, "SELECT name FROM sqlite_schema WHERE name = 'Eventos'"), vec![vec![texto("Eventos")]]);
    assert_eq!(contar(conn, "Eventos"), N_EVENTOS);
}

#[test]
fn excluir_tabela() {
    let a = ambiente();
    let conn = &a.conn;
    let plano = aplicar(conn, || plano_excluir_tabela(conn, "eventos"));
    assert_eq!(plano.comandos, textos(&["DROP TABLE \"eventos\""]));
    assert_eq!(plano.descricao, "Excluir a tabela eventos e as 5.000 linhas dela.");
    assert!(sql_de(conn, "eventos").is_none());
    let plano = plano_excluir_tabela(conn, "pedidos").unwrap();
    assert!(plano.avisos.iter().any(|a| a.contains("itens_pedido") && a.contains("CASCADE")));
    assert!(plano.avisos.iter().any(|a| a.contains("v_pedidos_por_cliente")));
}

#[test]
fn excluir_tabela_com_filha_set_null_avisa_que_a_chave_fica_nula() {
    // Antes o aviso dizia que a exclusão falharia; ela passa e zera a chave em todas as linhas da filha.
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(
        "
        CREATE TABLE autores (id INTEGER PRIMARY KEY, nome TEXT);
        CREATE TABLE textos (id INTEGER PRIMARY KEY, titulo TEXT,
                             autor_id INTEGER REFERENCES autores (id) ON DELETE SET NULL);
        CREATE TABLE resenhas (id INTEGER PRIMARY KEY, autor_id INTEGER REFERENCES autores (id));
        INSERT INTO autores VALUES (1, 'Ana Exemplo'), (2, 'Bruno Exemplo');
        INSERT INTO textos VALUES (1, 'A', 1), (2, 'B', 2), (3, 'C', 1), (4, 'D', NULL);
    ",
    )
    .unwrap();
    let plano = previa_sem_efeito(conn, || plano_excluir_tabela(conn, "autores"));
    let set_null = "A tabela textos tem chave estrangeira para autores com ON DELETE SET NULL: autor_id fica nulo em \
                    3 linhas ligadas de textos.";
    assert!(plano.avisos.contains(&set_null.to_string()));
    assert!(!plano.avisos.iter().any(|a| a.contains("a exclusão falha") && a.contains("textos")));
    assert!(!plano.avisos.iter().any(|a| a.starts_with("A tabela resenhas"))); // nenhuma linha de resenhas aponta
    assert!(plano.avisos.iter().any(|a| a.starts_with("Depois da exclusão, a chave estrangeira de resenhas")));
    executar(conn, &plano).unwrap();
    assert_eq!(valores(conn, "SELECT autor_id FROM textos"), vec![vec![Valor::Nulo]; 4]);
    // com linha ligada e NO ACTION, aí sim a exclusão falha
    conn.execute_batch(
        "
        CREATE TABLE editoras (id INTEGER PRIMARY KEY);
        CREATE TABLE livros (id INTEGER PRIMARY KEY, editora_id INTEGER REFERENCES editoras (id));
        INSERT INTO editoras VALUES (1); INSERT INTO livros VALUES (1, 1), (2, 1);
    ",
    )
    .unwrap();
    let plano = plano_excluir_tabela(conn, "editoras").unwrap();
    assert!(plano.avisos.iter().any(|a| a.contains("a exclusão falha") && a.contains("(hoje, 2 linhas)")));
    erro(executar(conn, &plano));
}

#[test]
fn excluir_tabela_avisa_dos_gatilhos_de_outras_tabelas_que_a_citam() {
    let a = ambiente();
    let conn = &a.conn;
    conn.execute_batch(
        "
        CREATE TABLE auditoria (id INTEGER PRIMARY KEY, pedido INTEGER);
        CREATE TRIGGER tg_pedidos_auditoria AFTER INSERT ON pedidos BEGIN
            INSERT INTO auditoria (pedido) VALUES (new.id);
        END;
        CREATE TRIGGER tg_auditoria_propria AFTER INSERT ON auditoria BEGIN SELECT 1; END;
    ",
    )
    .unwrap();
    let plano = plano_excluir_tabela(conn, "auditoria").unwrap();
    let aviso = "Estes gatilhos de outras tabelas citam auditoria e passarão a falhar (e a impedir a gravação que os \
                 dispara): tg_pedidos_auditoria (de pedidos).";
    assert!(plano.avisos.contains(&aviso.to_string()));
}

// ---------------------------------------------------------------------------------------------------
// Nome com espaço e coluna com aspas, em todas as operações

#[test]
fn tabela_com_espaco_e_coluna_com_aspas_em_todas_as_operacoes() {
    let a = ambiente();
    let conn = &a.conn;
    let t = TABELA_ESPACO;
    let linhas = |tabela: &str, colunas: &[&str]| {
        let lista: Vec<String> = colunas.iter().map(|c| q(c)).collect();
        valores(conn, &format!("SELECT {} FROM {} ORDER BY rowid", lista.join(", "), q(tabela)))
    };

    let originais = linhas(t, &[COLUNA_ASPAS, "número"]);
    assert_eq!(
        originais,
        vec![
            vec![texto("olá, mundo"), Valor::Inteiro(1)],
            vec![texto("com \"aspas\""), Valor::Inteiro(2)],
            vec![texto(""), Valor::Inteiro(3)],
            vec![Valor::Nulo, Valor::Nulo],
        ]
    );

    // mudar o tipo (reconstrução)
    let nova = ColunaDef { tipo: "VARCHAR(50)".into(), ..definicao(conn, t, COLUNA_ASPAS) };
    assert!(aplicar(conn, || plano_alterar_coluna(conn, t, COLUNA_ASPAS, &nova)).reconstroi);
    assert_eq!(linhas(t, &[COLUNA_ASPAS, "número"]), originais);

    // renomear a coluna (nativo) para outro nome com aspas
    let outra = "outra \"coluna\", com vírgula";
    let nova = ColunaDef { nome: outra.into(), ..definicao(conn, t, COLUNA_ASPAS) };
    assert!(!aplicar(conn, || plano_alterar_coluna(conn, t, COLUNA_ASPAS, &nova)).reconstroi);
    assert_eq!(linhas(t, &[outra, "número"]), originais);

    // adicionar coluna nativa e com reconstrução
    let mais = ColunaDef { padrao: Some("'x'".into()), ..col("mais 'uma'", "TEXT") };
    aplicar(conn, || plano_adicionar_coluna(conn, t, &mais));
    let unica = ColunaDef { unica: true, ..col("única [sim]", "INTEGER") };
    aplicar(conn, || plano_adicionar_coluna(conn, t, &unica));
    let esperado: Vec<Vec<Valor>> = originais
        .iter()
        .map(|r| {
            let mut r = r.clone();
            r.extend([texto("x"), Valor::Nulo]);
            r
        })
        .collect();
    assert_eq!(linhas(t, &[outra, "número", "mais 'uma'", "única [sim]"]), esperado);

    // índice sobre coluna com aspas
    let indice = [ColunaIndice { nome: outra.into(), desc: true }];
    aplicar(conn, || plano_criar_indice(conn, "idx \"espaço\"", t, &indice, false, None));
    assert!(indices(conn, t).contains("idx \"espaço\""));

    // excluir coluna com índice → erro que nomeia o índice; sem o índice → nativo
    falha_com(plano_excluir_coluna(conn, t, outra), "índice idx \"espaço\"");
    aplicar(conn, || plano_excluir_indice(conn, "idx \"espaço\""));

    // reordenar (reconstrução) mantém o rowid e os valores
    let ordem = ["única [sim]", "número", outra, "mais 'uma'"];
    aplicar(conn, || plano_reordenar_colunas(conn, t, &ordem));
    assert_eq!(nomes_colunas(conn, t), textos(&ordem));
    assert_eq!(linhas(t, &[outra, "número"]), originais);

    aplicar(conn, || plano_excluir_coluna(conn, t, outra));
    assert_eq!(nomes_colunas(conn, t), textos(&["única [sim]", "número", "mais 'uma'"]));

    // visão sobre a tabela, que sobrevive à reconstrução
    aplicar(conn, || plano_criar_visao(conn, "v espaço", &format!("SELECT * FROM {}", q(t))));
    let nova = ColunaDef { tipo: "REAL".into(), ..definicao(conn, t, "número") };
    aplicar(conn, || plano_alterar_coluna(conn, t, "número", &nova));
    assert_eq!(contar(conn, "v espaço"), 4);

    // renomear e excluir a tabela
    let novo_nome = "Outra \"tabela\", com vírgula";
    aplicar(conn, || plano_renomear_tabela(conn, t, novo_nome));
    assert_eq!(contar(conn, novo_nome), 4);
    aplicar(conn, || plano_excluir_tabela(conn, novo_nome));
    assert!(sql_de(conn, novo_nome).is_none());

    // criar tabela com nomes citados
    let colunas = [ColunaDef { pk: true, ..col("id \"x\"", "INTEGER") }, col("valor; texto", "TEXT")];
    aplicar(conn, || plano_criar_tabela(conn, "nova \"tabela\"", &colunas, SEM_RESTRICOES, false));
    conn.execute("INSERT INTO \"nova \"\"tabela\"\"\" (\"valor; texto\") VALUES ('a')", []).unwrap();
    assert_eq!(contar(conn, "nova \"tabela\""), 1);
}
