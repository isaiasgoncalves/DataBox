//! Casos fixos: cada caso de `casos_esperados.rs` roda aqui num banco de exemplo novo, com o mesmo `setup`, e o plano (descrição, comandos, reconstrói,
//! avisos) ou o erro (mensagem e status) tem de ser idêntico, letra por letra.

use comum::Resultado;
use ddl::{ColunaDef, Gerada, Referencia};

use super::casos_esperados::{CASOS, Esperado};
use super::*;
use crate::*;

const ESPACO: &str = TABELA_ESPACO;
const ASPAS: &str = COLUNA_ASPAS;
const GATILHO_EVENTO: &str = "CREATE TRIGGER trg_evento_erro AFTER INSERT ON eventos WHEN new.tipo = 'erro'\n\
    BEGIN\n    UPDATE eventos SET dados = 'marcado; revisar' WHERE rowid = new.rowid;\nEND;";

/// O `plano_alterar_coluna` com a definição atual da coluna e as mudanças aplicadas por `mudar`.
fn alterar(conn: &Connection, tabela: &str, coluna: &str, mudar: impl FnOnce(&mut ColunaDef)) -> Resultado<Plano> {
    let mut nova = definicao(conn, tabela, coluna);
    mudar(&mut nova);
    plano_alterar_coluna(conn, tabela, coluna, &nova)
}

fn so_nome(nome: &str) -> ColunaDef {
    col(nome, "")
}

fn indice(nome: &str, desc: bool) -> ColunaIndice {
    ColunaIndice { nome: nome.into(), desc }
}

fn referencia(tabela: &str, coluna: Option<&str>, ao_excluir: Option<&str>, ao_atualizar: Option<&str>) -> Referencia {
    Referencia {
        tabela: tabela.into(),
        coluna: coluna.map(Into::into),
        ao_excluir: ao_excluir.map(Into::into),
        ao_atualizar: ao_atualizar.map(Into::into),
        complemento: None,
    }
}

/// Executa o plano (os casos "executar_*"): Ok(None) quando deu certo.
fn feito(conn: &Connection, plano: Resultado<Plano>) -> Resultado<Option<Plano>> {
    executar(conn, &plano?)?;
    Ok(None)
}

fn rodar(nome: &str, c: &Connection) -> Resultado<Option<Plano>> {
    let plano = match nome {
        // --- alterar coluna
        "alterar_pedidos_observacao_varchar" => {
            alterar(c, "pedidos", "observacao", |n| n.tipo = "VARCHAR(1000)".into())
        }
        "alterar_produtos_preco_real" => alterar(c, "produtos", "preco", |n| n.tipo = "REAL".into()),
        "alterar_clientes_cidade_nao_nulo" => alterar(c, "clientes", "cidade", |n| n.nao_nulo = true),
        "alterar_clientes_limite_nao_nulo" => alterar(c, "clientes", "limite", |n| n.nao_nulo = true),
        "alterar_clientes_email_sem_unica" => alterar(c, "clientes", "email", |n| n.unica = false),
        "alterar_clientes_nome_unica" => alterar(c, "clientes", "nome", |n| n.unica = true),
        "alterar_pedidos_status_renomear_e_tipo" => alterar(c, "pedidos", "status", |n| {
            n.nome = "situacao".into();
            n.tipo = "VARCHAR(20)".into();
        }),
        "alterar_pedidos_status_renomear" => alterar(c, "pedidos", "status", |n| n.nome = "  situacao  ".into()),
        "alterar_clientes_nome_nada_muda" => alterar(c, "clientes", "nome", |_| {}),
        "alterar_clientes_nome_so_espacos" => alterar(c, "clientes", "nome", |n| {
            n.nome = " nome ".into();
            n.tipo = " TEXT ".into();
        }),
        "alterar_clientes_id_sem_pk" => alterar(c, "clientes", "id", |n| n.pk = false),
        "alterar_clientes_id_sem_pk_nem_auto" => alterar(c, "clientes", "id", |n| {
            n.pk = false;
            n.autoincremento = false;
        }),
        "alterar_eventos_valor_gerada" => alterar(c, "eventos", "valor", |n| {
            n.gerada = Some(Gerada { expressao: " length(dados) ".into(), armazenada: false });
        }),
        "alterar_produtos_centavos_sem_gerada" => alterar(c, "produtos", "preco_centavos", |n| n.gerada = None),
        "alterar_itens_quantidade_check" => {
            alterar(c, "itens_pedido", "quantidade", |n| n.verificacao = Some("quantidade > 0".into()))
        }
        "alterar_clientes_cidade_padrao_colacao" => alterar(c, "clientes", "cidade", |n| {
            n.padrao = Some(" 'Recife' ".into());
            n.colacao = Some("NOCASE".into());
            n.extra = Some(String::new());
        }),
        "alterar_pedidos_cliente_ref" => alterar(c, "pedidos", "cliente_id", |n| {
            n.referencia = Some(referencia("clientes", Some("id"), Some("set   null"), Some("cascade")));
        }),
        "alterar_pedidos_cliente_sem_ref" => alterar(c, "pedidos", "cliente_id", |n| n.referencia = None),
        "alterar_eventos_tipo_pk" => alterar(c, "eventos", "tipo", |n| n.pk = true),
        "alterar_eventos_tipo_sem_tipo" => alterar(c, "eventos", "tipo", |n| n.tipo = String::new()),
        "alterar_coluna_aspas_varchar" => alterar(c, ESPACO, ASPAS, |n| n.tipo = "VARCHAR(50)".into()),
        "alterar_marcas_sigla_sem_colacao" => alterar(c, "marcas", "sigla", |n| n.colacao = None),
        "alterar_com_orfa" => alterar(c, "pedidos", "observacao", |n| n.tipo = "VARCHAR(10)".into()),
        "alterar_registro_nota" => alterar(c, "registro", "nota", |n| n.tipo = "VARCHAR(80)".into()),
        "alterar_t_desc_v" => alterar(c, "t_desc", "v", |n| n.tipo = "VARCHAR(9)".into()),
        "alterar_t_comp_b" => alterar(c, "t_comp", "b", |n| n.tipo = "VARCHAR(9)".into()),
        "alterar_t_oid_v" => alterar(c, "t_oid", "v", |n| n.tipo = "VARCHAR(9)".into()),
        "alterar_auditoria_mensagem" => alterar(c, "auditoria", "mensagem", |n| n.tipo = "VARCHAR(9)".into()),
        "alterar_erro_coluna_existe" => alterar(c, "clientes", "cidade", |n| n.nome = "EMAIL".into()),
        "alterar_erro_auto" => alterar(c, "eventos", "tipo", |n| n.autoincremento = true),
        "alterar_erro_nome_vazio" => alterar(c, "clientes", "cidade", |n| n.nome = "  ".into()),
        "alterar_erro_coluna_nao_existe" => plano_alterar_coluna(c, "clientes", "xyz", &so_nome("a")),
        "alterar_erro_tabela_nao_existe" => plano_alterar_coluna(c, "xyz", "a", &so_nome("a")),
        "alterar_erro_tabela_vazia" => plano_alterar_coluna(c, "", "a", &so_nome("a")),
        "alterar_erro_coluna_vazia" => plano_alterar_coluna(c, "clientes", " ", &so_nome("a")),
        "alterar_erro_tipo_invalido" => alterar(c, "clientes", "cidade", |n| n.tipo = "TEXT NOT NULL".into()),
        "alterar_erro_acao_invalida" => alterar(c, "pedidos", "cliente_id", |n| {
            n.referencia = Some(referencia("clientes", None, Some("apagar"), None));
        }),
        "alterar_erro_definicao_invalida" => {
            alterar(c, "clientes", "cidade", |n| n.verificacao = Some("cidade <> 'x' (".into()))
        }
        "alterar_erro_tabela_shadow" => plano_alterar_coluna(c, "notas_data", "id", &so_nome("x")),
        // --- executar
        "executar_limite_nao_nulo" => return feito(c, alterar(c, "clientes", "limite", |n| n.nao_nulo = true)),
        "executar_marcas_sigla" => return feito(c, alterar(c, "marcas", "sigla", |n| n.colacao = None)),
        "executar_transacao_aberta" => {
            let plano = alterar(c, "eventos", "tipo", |n| n.nao_nulo = true)?;
            c.execute_batch("BEGIN").unwrap();
            let r = executar(c, &plano);
            c.execute_batch("ROLLBACK").unwrap();
            r?;
            return Ok(None);
        }
        "executar_clientes_cidade" => return feito(c, alterar(c, "clientes", "cidade", |n| n.nao_nulo = true)),
        "executar_excluir_editoras" => return feito(c, plano_excluir_tabela(c, "editoras")),
        // --- adicionar
        "adicionar_eventos_peso" => plano_adicionar_coluna(
            c,
            "eventos",
            &ColunaDef { nao_nulo: true, padrao: Some("0".into()), ..col("peso", "INTEGER") },
        ),
        "adicionar_eventos_obs" => plano_adicionar_coluna(
            c,
            "eventos",
            &ColunaDef { padrao: Some("'nenhuma'".into()), ..col(" obs ", "TEXT") },
        ),
        "adicionar_eventos_sem_tipo" => plano_adicionar_coluna(c, "eventos", &so_nome("x")),
        "adicionar_clientes_documento" => {
            plano_adicionar_coluna(c, "clientes", &ColunaDef { unica: true, ..col("documento", "TEXT") })
        }
        "adicionar_eventos_registrado" => plano_adicionar_coluna(
            c,
            "eventos",
            &ColunaDef { padrao: Some("CURRENT_TIMESTAMP".into()), ..col("registrado_em", "TEXT") },
        ),
        "adicionar_eventos_expressao" => plano_adicionar_coluna(
            c,
            "eventos",
            &ColunaDef { padrao: Some("(date('now'))".into()), ..col("dia", "TEXT") },
        ),
        "adicionar_eventos_pk" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { pk: true, ..col("codigo", "INTEGER") })
        }
        "adicionar_eventos_stored" | "adicionar_eventos_virtual" => {
            let gerada = Gerada { expressao: "length(dados)".into(), armazenada: nome == "adicionar_eventos_stored" };
            plano_adicionar_coluna(c, "eventos", &ColunaDef { gerada: Some(gerada), ..col("tam", "INTEGER") })
        }
        "adicionar_pedidos_fk_padrao" => plano_adicionar_coluna(
            c,
            "pedidos",
            &ColunaDef {
                padrao: Some("1".into()),
                referencia: Some(referencia("clientes", Some("id"), None, None)),
                ..col("outro", "INTEGER")
            },
        ),
        "adicionar_pedidos_fk" => plano_adicionar_coluna(
            c,
            "pedidos",
            &ColunaDef {
                referencia: Some(referencia("clientes", Some("id"), Some("cascade"), None)),
                ..col("outro", "INTEGER")
            },
        ),
        "adicionar_vazia_nao_nulo" => {
            plano_adicionar_coluna(c, "vazia", &ColunaDef { nao_nulo: true, ..col("b", "TEXT") })
        }
        "adicionar_espaco_unica" => {
            plano_adicionar_coluna(c, ESPACO, &ColunaDef { unica: true, ..col("única [sim]", "INTEGER") })
        }
        "adicionar_erro_nao_nulo" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { nao_nulo: true, ..col("x", "TEXT") })
        }
        "adicionar_erro_existe" => plano_adicionar_coluna(c, "eventos", &col("TIPO", "TEXT")),
        "adicionar_erro_auto" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { pk: true, autoincremento: true, ..col("x", "INTEGER") })
        }
        "adicionar_erro_sem_nome" => plano_adicionar_coluna(c, "eventos", &so_nome("")),
        "adicionar_erro_visao" => plano_adicionar_coluna(c, "v_pedidos_por_cliente", &so_nome("x")),
        "adicionar_erro_virtual" => plano_adicionar_coluna(c, "notas", &so_nome("x")),
        "adicionar_erro_interna" => plano_adicionar_coluna(c, "sqlite_sequence", &so_nome("x")),
        "adicionar_erro_padrao" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { padrao: Some("1; DROP TABLE x".into()), ..so_nome("x") })
        }
        // --- excluir coluna
        "excluir_eventos_dados" => plano_excluir_coluna(c, "eventos", "dados"),
        "excluir_erro_clientes_id" => plano_excluir_coluna(c, "clientes", "id"),
        "excluir_erro_itens_produto" => plano_excluir_coluna(c, "itens_pedido", "produto"),
        "excluir_erro_clientes_email" => plano_excluir_coluna(c, "clientes", "email"),
        "excluir_erro_clientes_nome" => plano_excluir_coluna(c, "clientes", "nome"),
        "excluir_erro_pedidos_feito_em" => plano_excluir_coluna(c, "pedidos", "feito_em"),
        "excluir_erro_produtos_preco" => plano_excluir_coluna(c, "produtos", "preco"),
        "excluir_erro_pedidos_observacao" => plano_excluir_coluna(c, "pedidos", "observacao"),
        "excluir_erro_pedidos_cliente_id" => plano_excluir_coluna(c, "pedidos", "cliente_id"),
        "excluir_erro_clientes_limite" => plano_excluir_coluna(c, "clientes", "limite"),
        "excluir_erro_clientes_ativo" => plano_excluir_coluna(c, "clientes", "ativo"),
        "excluir_erro_pedidos_status" => plano_excluir_coluna(c, "pedidos", "status"),
        "excluir_erro_auditoria_mensagem" => plano_excluir_coluna(c, "auditoria", "mensagem"),
        "excluir_erro_auditoria_visto" => plano_excluir_coluna(c, "auditoria", "visto"),
        "excluir_erro_eventos_tipo_visao" => plano_excluir_coluna(c, "eventos", "tipo"),
        "excluir_erro_unica_coluna" => plano_excluir_coluna(c, "t_unica", "so"),
        "excluir_erro_referenciada" => plano_excluir_coluna(c, "marcas", "sigla"),
        "excluir_erro_fk" => plano_excluir_coluna(c, "pedidos", "cliente_id"),
        "excluir_erro_restricao" => plano_excluir_coluna(c, "faixas", "de"),
        "excluir_erro_check_outra" => plano_excluir_coluna(c, "faixas", "ate"),
        "excluir_erro_unique_tabela" => plano_excluir_coluna(c, "u", "a"),
        "excluir_erro_gatilho_le" => plano_excluir_coluna(c, "g", "b"),
        "excluir_erro_coluna_nao_existe" => plano_excluir_coluna(c, "eventos", "xyz"),
        // --- reordenar
        "reordenar_eventos" => plano_reordenar_colunas(c, "eventos", &["dados", "tipo", "momento", "valor"]),
        "reordenar_produtos" => plano_reordenar_colunas(
            c,
            "produtos",
            &["preco_centavos", "serie", "foto", "estoque", "preco", "descricao", "codigo"],
        ),
        "reordenar_itens" => {
            plano_reordenar_colunas(c, "itens_pedido", &["PRODUTO", "pedido_id", "quantidade", "preco"])
        }
        "reordenar_clientes" => {
            plano_reordenar_colunas(c, "clientes", &["criado_em", "ativo", "limite", "cidade", "email", "nome", "id"])
        }
        "reordenar_pedidos" => {
            plano_reordenar_colunas(c, "pedidos", &["observacao", "status", "feito_em", "cliente_id", "id"])
        }
        "reordenar_auditoria" => plano_reordenar_colunas(c, "auditoria", &["visto", "mensagem", "pedido", "id"]),
        "reordenar_registro" => plano_reordenar_colunas(c, "registro", &["nota", "categoria_id", "id"]),
        "reordenar_espaco" => plano_reordenar_colunas(c, ESPACO, &["número", ASPAS]),
        "reordenar_mesma_ordem" => plano_reordenar_colunas(c, "eventos", &["MOMENTO", "tipo", "valor", "dados"]),
        "reordenar_erro_faltando" => plano_reordenar_colunas(c, "eventos", &["dados", "tipo", "momento"]),
        "reordenar_erro_repetida" => plano_reordenar_colunas(c, "eventos", &["dados", "tipo", "momento", "momento"]),
        // --- tabelas
        "criar_tarefas" => plano_criar_tabela(
            c,
            " tarefas ",
            &[
                ColunaDef { pk: true, autoincremento: true, ..col("id", "INTEGER") },
                ColunaDef { nao_nulo: true, padrao: Some("'sem título'".into()), ..col("titulo", "TEXT") },
                ColunaDef {
                    referencia: Some(referencia("clientes", Some("id"), Some("SET NULL"), None)),
                    ..col("cliente_id", "INTEGER")
                },
            ],
            SEM_RESTRICOES,
            false,
        ),
        "criar_pares" => plano_criar_tabela(
            c,
            "pares",
            &[ColunaDef { pk: true, ..col("a", "TEXT") }, ColunaDef { pk: true, ..col("b", "INTEGER") }],
            SEM_RESTRICOES,
            true,
        ),
        "criar_faixas" => plano_criar_tabela(
            c,
            "faixas",
            &[
                col("de", "INTEGER"),
                col("ate", "INTEGER"),
                ColunaDef { verificacao: Some("passo <= ate".into()), ..col("passo", "INTEGER") },
                ColunaDef { colacao: Some("NOCASE".into()), ..col("nota", "TEXT") },
            ],
            &textos(&[" CHECK (de < ate) ", "", "UNIQUE (de, ate)"]),
            false,
        ),
        "criar_uma" => plano_criar_tabela(c, "uma", &[so_nome("a")], SEM_RESTRICOES, false),
        "criar_sem_rowid_com_restricao" => {
            plano_criar_tabela(c, "sr", &[col("a", "TEXT")], &textos(&["PRIMARY KEY (a)"]), true)
        }
        "criar_nomes_citados" => plano_criar_tabela(
            c,
            "nova \"tabela\"",
            &[ColunaDef { pk: true, ..col("id \"x\"", "INTEGER") }, col("valor; texto", "TEXT")],
            SEM_RESTRICOES,
            false,
        ),
        "criar_erro_sem_nome" => plano_criar_tabela(c, "", &[so_nome("a")], SEM_RESTRICOES, false),
        "criar_erro_existe" => plano_criar_tabela(c, "Clientes", &[so_nome("a")], SEM_RESTRICOES, false),
        "criar_erro_existe_indice" => {
            plano_criar_tabela(c, "IDX_PEDIDOS_CLIENTE", &[so_nome("a")], SEM_RESTRICOES, false)
        }
        "criar_erro_existe_visao" => {
            plano_criar_tabela(c, "v_pedidos_por_cliente", &[so_nome("a")], SEM_RESTRICOES, false)
        }
        "criar_erro_reservado" => plano_criar_tabela(c, "sqlite_minha", &[so_nome("a")], SEM_RESTRICOES, false),
        "criar_erro_sem_colunas" => plano_criar_tabela(c, "nova", &[], SEM_RESTRICOES, false),
        "criar_erro_repetida" => plano_criar_tabela(c, "nova", &[so_nome("a"), so_nome("A")], SEM_RESTRICOES, false),
        "criar_erro_coluna_sem_nome" => plano_criar_tabela(c, "nova", &[so_nome(" ")], SEM_RESTRICOES, false),
        "criar_erro_auto_texto" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { pk: true, autoincremento: true, ..col("a", "TEXT") }],
            SEM_RESTRICOES,
            false,
        ),
        "criar_erro_auto_sem_pk" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { autoincremento: true, ..col("a", "INTEGER") }],
            SEM_RESTRICOES,
            false,
        ),
        "criar_erro_auto_sem_rowid" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { pk: true, autoincremento: true, ..col("a", "INTEGER") }],
            SEM_RESTRICOES,
            true,
        ),
        "criar_erro_sem_rowid_sem_chave" => plano_criar_tabela(c, "nova", &[so_nome("a")], SEM_RESTRICOES, true),
        "criar_erro_ref_invalida" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { verificacao: Some("a >".into()), ..so_nome("a") }],
            SEM_RESTRICOES,
            false,
        ),
        "renomear_clientes" => plano_renomear_tabela(c, "clientes", " contatos "),
        "renomear_eventos_maiusculas" => plano_renomear_tabela(c, "eventos", "Eventos"),
        "renomear_notas_virtual" => plano_renomear_tabela(c, "notas", "anotacoes"),
        "renomear_erro_existe" => plano_renomear_tabela(c, "clientes", "PEDIDOS"),
        "renomear_erro_igual" => plano_renomear_tabela(c, "clientes", "clientes"),
        "renomear_erro_vazio" => plano_renomear_tabela(c, "clientes", ""),
        "renomear_erro_visao" => plano_renomear_tabela(c, "v_pedidos_por_cliente", "x"),
        "renomear_erro_reservado" => plano_renomear_tabela(c, "clientes", "sqlite_x"),
        "excluir_tabela_eventos" => plano_excluir_tabela(c, "eventos"),
        "excluir_tabela_pedidos" => plano_excluir_tabela(c, "pedidos"),
        "excluir_tabela_clientes" => plano_excluir_tabela(c, "clientes"),
        "excluir_tabela_produtos" => plano_excluir_tabela(c, "produtos"),
        "excluir_tabela_notas" => plano_excluir_tabela(c, "notas"),
        "excluir_tabela_autores" | "excluir_tabela_autores_sem_fk" => plano_excluir_tabela(c, "autores"),
        "excluir_tabela_pares" => plano_excluir_tabela(c, "pares"),
        "excluir_tabela_auditoria" => plano_excluir_tabela(c, "auditoria"),
        "excluir_tabela_erro_nao_existe" => plano_excluir_tabela(c, "xyz"),
        "excluir_tabela_erro_interna" => plano_excluir_tabela(c, "sqlite_sequence"),
        "excluir_tabela_erro_shadow" => plano_excluir_tabela(c, "notas_data"),
        // --- índices, visões, gatilhos
        "indice_unico" => {
            plano_criar_indice(c, "idx_produtos_descricao", "produtos", &[indice("descricao", false)], true, None)
        }
        "indice_desc" => plano_criar_indice(
            c,
            "idx_eventos_momento",
            "EVENTOS",
            &[indice("tipo", false), indice("MOMENTO", true)],
            false,
            None,
        ),
        "indice_parcial" => plano_criar_indice(
            c,
            "idx_pedidos_pagos",
            "pedidos",
            &["feito_em".into()],
            false,
            Some(" status = 'pago' "),
        ),
        "indice_espaco" => plano_criar_indice(c, "idx \"espaço\"", ESPACO, &[indice(ASPAS, true)], false, None),
        "indice_erro_repetidos" => {
            plano_criar_indice(c, "idx_tipo", "eventos", &[indice("tipo", false), indice("momento", true)], true, None)
        }
        "indice_erro_repetido_um" => plano_criar_indice(c, "idx_tipo", "eventos", &[indice("tipo", false)], true, None),
        "indice_erro_coluna" => plano_criar_indice(c, "idx_x", "eventos", &[indice("inexistente", false)], false, None),
        "indice_erro_coluna_vazia" => plano_criar_indice(c, "idx_x", "eventos", &[indice("", false)], false, None),
        "indice_erro_duas_vezes" => {
            plano_criar_indice(c, "idx_x", "eventos", &[indice("tipo", false), indice("TIPO", false)], false, None)
        }
        "indice_erro_nome" => plano_criar_indice(c, "CLIENTES", "eventos", &[indice("tipo", false)], false, None),
        "indice_erro_sem_colunas" => plano_criar_indice(c, "idx_x", "eventos", &[], false, None),
        "indice_erro_tabela" => plano_criar_indice(c, "idx_x", "nao_existe", &[indice("a", false)], false, None),
        "indice_erro_onde" => plano_criar_indice(
            c,
            "idx_x",
            "eventos",
            &[indice("tipo", false)],
            false,
            Some("tipo = 'a'; DROP TABLE eventos"),
        ),
        "indice_erro_onde_coluna" => {
            plano_criar_indice(c, "idx_x", "eventos", &[indice("tipo", false)], false, Some("xyz = 1"))
        }
        "excluir_indice" => plano_excluir_indice(c, "IDX_PEDIDOS_CLIENTE"),
        "excluir_indice_erro_auto" => plano_excluir_indice(c, "sqlite_autoindex_clientes_1"),
        "excluir_indice_erro_nao_existe" => plano_excluir_indice(c, "xyz"),
        "excluir_indice_erro_vazio" => plano_excluir_indice(c, ""),
        "visao_criar" => {
            plano_criar_visao(c, "v_eventos_por_tipo", "  SELECT tipo, count(*) AS n FROM eventos GROUP BY tipo; ;")
        }
        "visao_criar_with" => plano_criar_visao(c, "v_w", "WITH x AS (SELECT 1 AS a) SELECT a FROM x"),
        "visao_erro_delete" => plano_criar_visao(c, "v_x", "DELETE FROM eventos"),
        "visao_erro_dois" => plano_criar_visao(c, "v_x", "SELECT 1; SELECT 2"),
        "visao_erro_tabela" => plano_criar_visao(c, "v_x", "SELECT * FROM tabela_que_nao_existe"),
        "visao_erro_coluna" => plano_criar_visao(c, "v_x", "SELECT xyz FROM eventos"),
        "visao_erro_sintaxe" => plano_criar_visao(c, "v_x", "SELECT FROM"),
        "visao_erro_vazio" => plano_criar_visao(c, "v_x", ""),
        "visao_erro_nome" => plano_criar_visao(c, "V_PEDIDOS_POR_CLIENTE", "SELECT 1"),
        "visao_excluir" => plano_excluir_visao(c, "v_pedidos_por_cliente"),
        "visao_excluir_com_gatilho" => plano_excluir_visao(c, "V_AUDITORIA"),
        "visao_excluir_erro" => plano_excluir_visao(c, "xyz"),
        "visao_excluir_erro_vazio" => plano_excluir_visao(c, ""),
        "gatilho_criar" => plano_criar_gatilho(c, GATILHO_EVENTO),
        "gatilho_criar_temp" => plano_criar_gatilho(
            c,
            "CREATE TEMP TRIGGER IF NOT EXISTS main.\"tg x\" AFTER DELETE ON eventos BEGIN SELECT 1; END",
        ),
        "gatilho_erro_vazio" => plano_criar_gatilho(c, "  "),
        "gatilho_erro_outro" => plano_criar_gatilho(c, "CREATE TABLE x (a)"),
        "gatilho_erro_incompleto" => {
            plano_criar_gatilho(c, "CREATE TRIGGER t1 AFTER INSERT ON eventos BEGIN SELECT 1 END")
        }
        "gatilho_erro_existe" => {
            plano_criar_gatilho(c, "CREATE TRIGGER TRG_PEDIDO_CANCELADO AFTER INSERT ON eventos BEGIN SELECT 1; END")
        }
        "gatilho_erro_tabela" => {
            plano_criar_gatilho(c, "CREATE TRIGGER t2 AFTER INSERT ON nao_existe BEGIN SELECT 1; END")
        }
        "gatilho_erro_reservado" => {
            plano_criar_gatilho(c, "CREATE TRIGGER sqlite_t AFTER INSERT ON eventos BEGIN SELECT 1; END")
        }
        "gatilho_excluir" => plano_excluir_gatilho(c, "TRG_PEDIDO_CANCELADO"),
        "gatilho_excluir_erro" => plano_excluir_gatilho(c, "xyz"),
        "gatilho_excluir_erro_vazio" => plano_excluir_gatilho(c, ""),
        // --- NUL no texto digitado (recusado: "the query contains a null character")
        "visao_erro_nul" => plano_criar_visao(c, "v_n", "SELECT * FROM eventos\0 WHERE 0"),
        "indice_erro_onde_nul" => {
            plano_criar_indice(c, "idx_n", "eventos", &["tipo".into()], false, Some("tipo = 'a'\0 AND 0"))
        }
        "adicionar_erro_extra_nul" => plano_adicionar_coluna(
            c,
            "eventos",
            &ColunaDef { extra: Some("COLLATE NOCASE\0 NOT NULL".into()), ..col("x", "TEXT") },
        ),
        "adicionar_erro_complemento_nul" => {
            let ref_ = Referencia {
                complemento: Some("DEFERRABLE\0 INITIALLY DEFERRED".into()),
                ..referencia("clientes", Some("id"), None, None)
            };
            plano_adicionar_coluna(c, "pedidos", &ColunaDef { referencia: Some(ref_), ..col("outro", "INTEGER") })
        }
        "adicionar_erro_padrao_nul" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { padrao: Some("'a'\0".into()), ..col("x", "TEXT") })
        }
        "adicionar_erro_nome_nul" => plano_adicionar_coluna(c, "eventos", &col("x\0y", "TEXT")),
        "adicionar_erro_tipo_nul" => plano_adicionar_coluna(c, "eventos", &col("x", "TEXT\0")),
        "adicionar_erro_colacao_nul" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { colacao: Some("NOCASE\0".into()), ..col("x", "TEXT") })
        }
        "adicionar_erro_gerada_nul" => {
            let gerada = Gerada { expressao: "length(dados)\0".into(), armazenada: false };
            plano_adicionar_coluna(c, "eventos", &ColunaDef { gerada: Some(gerada), ..col("g", "INTEGER") })
        }
        "adicionar_erro_ref_tabela_nul" => plano_adicionar_coluna(
            c,
            "pedidos",
            &ColunaDef { referencia: Some(referencia("clientes\0", Some("id"), None, None)), ..col("o", "INTEGER") },
        ),
        "criar_erro_check_nul" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { verificacao: Some("a > 0\0".into()), ..so_nome("a") }],
            SEM_RESTRICOES,
            false,
        ),
        "criar_erro_restricao_nul" => plano_criar_tabela(c, "nova", &[so_nome("a")], &["CHECK (a > 0)\0"], false),
        "criar_erro_nome_nul" => plano_criar_tabela(c, "t\0x", &[so_nome("a")], SEM_RESTRICOES, false),
        "criar_erro_coluna_nome_nul" => plano_criar_tabela(c, "nova", &[so_nome("a\0b")], SEM_RESTRICOES, false),
        "criar_erro_padrao_nul" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { padrao: Some("1\0".into()), ..so_nome("a") }],
            SEM_RESTRICOES,
            false,
        ),
        "renomear_erro_nul" => plano_renomear_tabela(c, "clientes", "c\0x"),
        "renomear_erro_origem_nul" => plano_renomear_tabela(c, "clientes\0x", "y"),
        "excluir_tabela_erro_nul" => plano_excluir_tabela(c, "eventos\0"),
        "alterar_erro_check_nul" => alterar(c, "eventos", "tipo", |n| n.verificacao = Some("tipo <> ''\0".into())),
        "alterar_erro_check_nul_cidade" => {
            alterar(c, "clientes", "cidade", |n| n.verificacao = Some("cidade <> ''\0 OR 1".into()))
        }
        "alterar_erro_nome_nul" => alterar(c, "eventos", "tipo", |n| n.nome = "ti\0po".into()),
        "alterar_erro_tipo_nul" => alterar(c, "eventos", "tipo", |n| n.tipo = "TEXT\0".into()),
        "alterar_erro_coluna_arg_nul" => plano_alterar_coluna(c, "eventos", "tipo\0", &so_nome("x")),
        "excluir_erro_coluna_nul" => plano_excluir_coluna(c, "eventos", "dados\0"),
        "excluir_erro_tabela_nul" => plano_excluir_coluna(c, "eventos\0", "dados"),
        "reordenar_erro_nul" => plano_reordenar_colunas(c, "eventos", &["dados\0", "tipo", "momento", "valor"]),
        "indice_erro_nome_nul" => plano_criar_indice(c, "idx\0x", "eventos", &["tipo".into()], false, None),
        "indice_erro_coluna_nul" => plano_criar_indice(c, "idx_x", "eventos", &["tipo\0".into()], false, None),
        "indice_erro_tabela_nul" => plano_criar_indice(c, "idx_x", "eventos\0", &["tipo".into()], false, None),
        "excluir_indice_erro_nul" => plano_excluir_indice(c, "idx_pedidos_cliente\0"),
        "visao_erro_nome_nul" => plano_criar_visao(c, "v\0x", "SELECT 1"),
        "visao_excluir_erro_nul" => plano_excluir_visao(c, "v_pedidos_por_cliente\0"),
        "gatilho_excluir_erro_nul" => plano_excluir_gatilho(c, "trg_pedido_cancelado\0"),
        // --- parâmetros (?, :x) no texto digitado: o prepare aceita, a ligação recusa
        "visao_erro_param" => plano_criar_visao(c, "v_p", "SELECT ?"),
        "visao_erro_param_nome" => plano_criar_visao(c, "v_p", "SELECT :x FROM eventos"),
        "visao_erro_param_dois" => plano_criar_visao(c, "v_p", "SELECT ?, ?3"),
        "adicionar_erro_gerada_param" => {
            let gerada = Gerada { expressao: "?".into(), armazenada: false };
            plano_adicionar_coluna(c, "eventos", &ColunaDef { gerada: Some(gerada), ..col("g", "INTEGER") })
        }
        "adicionar_erro_padrao_param" => {
            plano_adicionar_coluna(c, "eventos", &ColunaDef { padrao: Some("?".into()), ..col("x", "TEXT") })
        }
        "criar_erro_check_param" => plano_criar_tabela(
            c,
            "nova",
            &[ColunaDef { verificacao: Some("a > ?".into()), ..so_nome("a") }],
            SEM_RESTRICOES,
            false,
        ),
        "indice_erro_onde_param" => {
            plano_criar_indice(c, "idx_p", "eventos", &["tipo".into()], false, Some("tipo = ?"))
        }
        "gatilho_erro_param" => {
            plano_criar_gatilho(c, "CREATE TRIGGER t_p AFTER INSERT ON eventos BEGIN SELECT ?; END")
        }
        // --- definicoes (todas erro)
        "definicoes_visao"
        | "definicoes_virtual"
        | "definicoes_shadow"
        | "definicoes_interna"
        | "definicoes_nao_existe"
        | "definicoes_nul" => {
            let tabela = match nome {
                "definicoes_visao" => "v_pedidos_por_cliente",
                "definicoes_virtual" => "notas",
                "definicoes_shadow" => "notas_data",
                "definicoes_interna" => "sqlite_sequence",
                "definicoes_nul" => "eventos\0",
                _ => "nao_existe",
            };
            definicoes_tabela(c, tabela)?;
            panic!("{nome}: definicoes_tabela devia falhar");
        }
        outro => panic!("caso sem correspondente em Rust: {outro}"),
    };
    plano.map(Some)
}

#[test]
fn planos_e_erros_iguais_aos_esperados() {
    let mut diferencas: Vec<String> = Vec::new();
    for caso in CASOS {
        let a = ambiente();
        if !caso.setup.is_empty() {
            a.conn.execute_batch(caso.setup).unwrap_or_else(|e| panic!("{}: setup: {e}", caso.nome));
        }
        let obtido = rodar(caso.nome, &a.conn);
        let ok = match (&caso.esperado, &obtido) {
            (Esperado::Plano { descricao, comandos, reconstroi, avisos }, Ok(Some(p))) => {
                p.descricao == *descricao
                    && p.comandos == *comandos
                    && p.reconstroi == *reconstroi
                    && p.avisos == *avisos
            }
            (Esperado::Erro { mensagem, status }, Err(e)) => e.mensagem == *mensagem && e.status == *status,
            (Esperado::Feito, Ok(None)) => true,
            _ => false,
        };
        if !ok {
            let esperado = match &caso.esperado {
                Esperado::Plano { descricao, comandos, reconstroi, avisos } => {
                    format!("Plano {{ {descricao:?}, {comandos:#?}, {reconstroi}, {avisos:#?} }}")
                }
                Esperado::Erro { mensagem, status } => format!("Erro {{ {mensagem:?}, {status} }}"),
                Esperado::Feito => "Feito".to_string(),
            };
            let obtido = match &obtido {
                Ok(Some(p)) => {
                    format!("Plano {{ {:?}, {:#?}, {}, {:#?} }}", p.descricao, p.comandos, p.reconstroi, p.avisos)
                }
                Ok(None) => "Feito".to_string(),
                Err(e) => format!("Erro {{ {:?}, {} }} (detalhe {:?})", e.mensagem, e.status, e.detalhe),
            };
            diferencas.push(format!("── {}\n   esperado: {esperado}\n   obtido:   {obtido}", caso.nome));
        }
    }
    assert!(
        diferencas.is_empty(),
        "{} de {} casos diferentes:\n{}",
        diferencas.len(),
        CASOS.len(),
        diferencas.join("\n")
    );
}
