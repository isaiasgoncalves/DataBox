//! Objetos do banco e estrutura das tabelas.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use comum::{Afinidade, Chave};

use super::*;
use crate::{
    ColunaIndice, ColunaInfo, FkColuna, GatilhoResumo, IndiceResumo, LinhaPagina, OrigemIndice, TabelaResumo,
    TipoChave, TipoObjeto, VisaoResumo,
};

fn rowid_alias(alias: &str) -> Option<TipoChave> {
    Some(TipoChave::Rowid { alias: alias.into() })
}

#[test]
fn objetos_classifica_tabelas_visoes_e_sistema() {
    let ex = exemplo("objetos");
    let objs = ex.banco.objetos().unwrap();
    let tabelas: HashMap<&str, &TabelaResumo> = objs.tabelas.iter().map(|t| (t.nome.as_str(), t)).collect();
    let nomes: HashSet<&str> = tabelas.keys().copied().collect();
    assert_eq!(
        nomes,
        HashSet::from(["clientes", "produtos", "pedidos", "itens_pedido", "eventos", TAB_ESPACO, "notas"])
    );
    assert_eq!(
        *tabelas["clientes"],
        TabelaResumo {
            nome: "clientes".into(),
            tipo: TipoObjeto::Table,
            sem_rowid: false,
            linhas: Some(N_CLIENTES),
            colunas: Some(7)
        }
    );
    assert!(tabelas["produtos"].sem_rowid);
    assert_eq!(tabelas["produtos"].colunas, Some(7)); // a coluna gerada conta
    assert_eq!(tabelas["itens_pedido"].linhas, Some(456));
    assert_eq!(tabelas["eventos"].linhas, Some(N_EVENTOS));
    assert_eq!(tabelas["notas"].tipo, TipoObjeto::Virtual);
    assert_eq!(tabelas["notas"].colunas, Some(2)); // sem as colunas ocultas do FTS5
    assert_eq!(objs.visoes, vec![VisaoResumo { nome: "v_pedidos_por_cliente".into(), colunas: Some(4) }]);

    let sistema: HashMap<&str, _> = objs.sistema.iter().map(|s| (s.nome.as_str(), s)).collect();
    for nome in [
        "sqlite_sequence",
        "sqlite_schema",
        "notas_data",
        "notas_idx",
        "notas_content",
        "notas_docsize",
        "notas_config",
    ] {
        assert!(sistema.contains_key(nome), "{nome}");
    }
    assert_eq!(sistema["notas_data"].tipo, TipoObjeto::Shadow);
    assert_eq!(sistema["sqlite_sequence"].tipo, TipoObjeto::Table);
    assert_eq!(sistema["sqlite_sequence"].linhas, Some(1));

    let indices: HashMap<&str, &IndiceResumo> = objs.indices.iter().map(|i| (i.nome.as_str(), i)).collect();
    assert_eq!(
        *indices["idx_pedidos_cliente"],
        IndiceResumo { nome: "idx_pedidos_cliente".into(), tabela: "pedidos".into(), unico: false, automatico: false }
    );
    assert!(indices["idx_clientes_nome_cidade"].unico);
    assert!(indices["sqlite_autoindex_clientes_1"].automatico);
    assert!(indices["sqlite_autoindex_clientes_1"].unico);
    assert_eq!(objs.gatilhos, vec![GatilhoResumo { nome: "trg_pedido_cancelado".into(), tabela: "pedidos".into() }]);
}

#[test]
fn visao_quebrada_nao_derruba_a_lista() {
    let ex = exemplo("visao-quebrada");
    ex.banco.consultar("CREATE VIEW v_quebrada AS SELECT * FROM tabela_apagada", 1000).unwrap();
    let objs = ex.banco.objetos().unwrap();
    assert!(objs.visoes.contains(&VisaoResumo { nome: "v_quebrada".into(), colunas: None }));
    let e = erro(pag_r(&ex.banco, "v_quebrada", |_| {}));
    assert!(e.status == 400 && e.mensagem.contains("tabela_apagada"), "{e:?}");
    assert!(pag(&ex.banco, "sqlite_schema", |_| {}).total.unwrap() > 0);
}

#[test]
fn objetos_nao_conta_linhas_em_arquivo_grande() {
    let ex = exemplo("grande");
    ex.banco.ajustes.limite_contagem_bytes.store(1024, Ordering::Relaxed);
    let objs = ex.banco.objetos().unwrap();
    assert!(objs.tabelas.iter().all(|t| t.linhas.is_none()));
    assert!(objs.sistema.iter().all(|s| s.linhas.is_none()));
    assert_eq!(ex.banco.tabela_info("clientes").unwrap().linhas, None);
    assert_eq!(contar(&ex.banco, "clientes"), N_CLIENTES); // sob demanda continua contando
}

fn por_nome(colunas: &[ColunaInfo]) -> HashMap<&str, &ColunaInfo> {
    colunas.iter().map(|c| (c.nome.as_str(), c)).collect()
}

#[test]
fn tabela_info_clientes() {
    let ex = exemplo("info-clientes");
    let info = ex.banco.tabela_info("clientes").unwrap();
    assert_eq!(info.nome, "clientes");
    assert_eq!(info.tipo, TipoObjeto::Table);
    assert!(!info.sistema && !info.sem_rowid && !info.estrita);
    assert!(info.editavel && info.esquema_editavel);
    assert_eq!(info.chave, rowid_alias("rowid"));
    assert_eq!(info.linhas, Some(N_CLIENTES));
    assert!(info.sql.as_deref().unwrap().starts_with("CREATE TABLE clientes"));
    assert_eq!(
        info.colunas.iter().map(|c| c.nome.as_str()).collect::<Vec<_>>(),
        ["id", "nome", "email", "cidade", "limite", "ativo", "criado_em"]
    );
    let cols = por_nome(&info.colunas);
    assert_eq!(
        *cols["id"],
        ColunaInfo {
            cid: 0,
            nome: "id".into(),
            tipo: "INTEGER".into(),
            afinidade: Afinidade::Integer,
            nao_nulo: false,
            padrao: None,
            pk: 1,
            oculta: 0,
            gerada: false,
            autoincremento: true,
            rowid_alias: true,
            unica: false,
            fk: None,
        }
    );
    assert!(cols["email"].unica);
    assert!(!cols["nome"].unica); // o índice único (nome, cidade) tem duas colunas
    assert!(cols["nome"].nao_nulo);
    assert_eq!(cols["cidade"].padrao.as_deref(), Some("'São Paulo'"));
    assert_eq!(cols["limite"].afinidade, Afinidade::Real);
    assert_eq!(cols["criado_em"].padrao.as_deref(), Some("CURRENT_TIMESTAMP"));
    let indices: HashMap<&str, _> = info.indices.iter().map(|i| (i.nome.as_str(), i)).collect();
    assert_eq!(indices["sqlite_autoindex_clientes_1"].origem, OrigemIndice::U);
    assert_eq!(indices["sqlite_autoindex_clientes_1"].sql, None);
    let parcial = indices["idx_clientes_nome_cidade"];
    assert!(parcial.parcial && parcial.unico && parcial.origem == OrigemIndice::C);
    assert_eq!(
        parcial.colunas,
        vec![ColunaIndice { nome: "nome".into(), desc: false }, ColunaIndice { nome: "cidade".into(), desc: false }]
    );
    assert!(parcial.sql.as_deref().unwrap().starts_with("CREATE UNIQUE INDEX"));
    assert!(info.gatilhos.is_empty());
}

#[test]
fn tabela_info_pedidos_fk_indices_e_gatilho() {
    let ex = exemplo("info-pedidos");
    let info = ex.banco.tabela_info("pedidos").unwrap();
    let cols = por_nome(&info.colunas);
    assert!(cols["id"].rowid_alias);
    assert!(!cols["id"].autoincremento);
    assert_eq!(
        cols["cliente_id"].fk,
        Some(FkColuna {
            tabela: "clientes".into(),
            coluna: Some("id".into()),
            ao_excluir: "CASCADE".into(),
            ao_atualizar: "NO ACTION".into()
        })
    );
    assert_eq!(info.chaves_estrangeiras.len(), 1);
    let fk = &info.chaves_estrangeiras[0];
    assert_eq!((fk.id, fk.colunas.clone(), fk.tabela.as_str()), (0, vec!["cliente_id".to_string()], "clientes"));
    assert_eq!(fk.colunas_ref, vec![Some("id".to_string())]);
    assert_eq!((fk.ao_excluir.as_str(), fk.ao_atualizar.as_str()), ("CASCADE", "NO ACTION"));
    let indices: HashMap<&str, _> = info.indices.iter().map(|i| (i.nome.as_str(), i)).collect();
    assert_eq!(
        indices["idx_pedidos_status_data"].colunas,
        vec![ColunaIndice { nome: "status".into(), desc: false }, ColunaIndice { nome: "feito_em".into(), desc: true }]
    );
    assert!(!indices["idx_pedidos_cliente"].parcial);
    assert_eq!(info.gatilhos.iter().map(|g| g.nome.as_str()).collect::<Vec<_>>(), ["trg_pedido_cancelado"]);
    assert!(info.gatilhos[0].sql.as_deref().unwrap().contains("BEGIN"));
}

#[test]
fn tabela_info_produtos_sem_rowid_e_gerada() {
    let ex = exemplo("info-produtos");
    let info = ex.banco.tabela_info("produtos").unwrap();
    assert!(info.sem_rowid);
    assert_eq!(info.chave, Some(TipoChave::Pk { colunas: vec!["codigo".into()] }));
    let cols = por_nome(&info.colunas);
    assert!(cols["preco_centavos"].gerada);
    assert_eq!(cols["preco_centavos"].oculta, 2);
    assert!(!cols["codigo"].rowid_alias);
    assert_eq!(cols["foto"].afinidade, Afinidade::Blob);
    assert_eq!(cols["preco"].afinidade, Afinidade::Numeric);
    assert_eq!(info.indices.iter().map(|i| i.origem).collect::<Vec<_>>(), [OrigemIndice::Pk]);
}

#[test]
fn tabela_info_itens_pedido_pk_composta() {
    let ex = exemplo("info-itens");
    let info = ex.banco.tabela_info("itens_pedido").unwrap();
    // itens_pedido tem rowid (não é WITHOUT ROWID): a identidade é o rowid, mesmo com PK composta
    assert_eq!(info.chave, rowid_alias("rowid"));
    let cols = por_nome(&info.colunas);
    assert!(cols["pedido_id"].pk == 1 && cols["produto"].pk == 2);
    assert!(!cols["pedido_id"].rowid_alias);
    assert_eq!(cols["produto"].fk.as_ref().unwrap().tabela, "produtos");
    assert_eq!(info.chaves_estrangeiras.len(), 2);
}

#[test]
fn tabela_info_visao_virtual_sombra_e_sistema() {
    let ex = exemplo("info-tipos");
    let b = &ex.banco;
    let visao = b.tabela_info("v_pedidos_por_cliente").unwrap();
    assert!(visao.tipo == TipoObjeto::View && visao.chave.is_none());
    assert!(!visao.editavel && !visao.esquema_editavel);
    assert_eq!(visao.linhas, Some(N_CLIENTES));

    let notas = b.tabela_info("notas").unwrap();
    assert!(notas.tipo == TipoObjeto::Virtual && !notas.sistema);
    assert!(notas.editavel && !notas.esquema_editavel);
    assert_eq!(notas.chave, rowid_alias("rowid"));
    assert_eq!(notas.colunas.iter().map(|c| c.oculta).collect::<Vec<_>>(), [0, 0, 1, 1]);

    let sombra = b.tabela_info("notas_data").unwrap();
    assert!(sombra.tipo == TipoObjeto::Shadow && sombra.sistema);
    assert!(!sombra.editavel && !sombra.esquema_editavel);

    let seq = b.tabela_info("sqlite_sequence").unwrap();
    assert!(seq.sistema && seq.editavel && !seq.esquema_editavel);

    let esquema = b.tabela_info("sqlite_schema").unwrap();
    assert!(esquema.sistema && !esquema.editavel);
}

#[test]
fn tabela_info_nome_com_espaco_e_aspas() {
    let ex = exemplo("info-espaco");
    let info = ex.banco.tabela_info(TAB_ESPACO).unwrap();
    assert_eq!(info.colunas.iter().map(|c| c.nome.as_str()).collect::<Vec<_>>(), [COL_CITADA, "número"]);
    assert_eq!(info.linhas, Some(4));
}

#[test]
fn tabela_info_nome_sem_diferenciar_caixa() {
    let ex = exemplo("info-caixa");
    assert_eq!(ex.banco.tabela_info("CLIENTES").unwrap().nome, "clientes");
}

#[test]
fn tabela_inexistente() {
    let ex = exemplo("inexistente");
    assert_eq!(erro(ex.banco.tabela_info("nao_existe")).status, 404);
}

#[test]
fn somente_leitura_marca_nada_editavel() {
    let ex = exemplo_ro("ro-info");
    let info = ex.banco.tabela_info("clientes").unwrap();
    assert!(!info.editavel && !info.esquema_editavel);
}

#[test]
fn autoincremento_so_como_palavra_fora_de_string_e_comentario() {
    let ex = exemplo("autoinc");
    let b = &ex.banco;
    b.consultar(
        r#"
        CREATE TABLE t_falso (id INTEGER PRIMARY KEY /* AUTOINCREMENT */, obs TEXT DEFAULT 'AUTOINCREMENT',
                              "AUTOINCREMENT" TEXT); -- AUTOINCREMENT
        CREATE TABLE t_verdadeiro (id integer primary key autoincrement, obs TEXT);
        CREATE TABLE t_bigint (id BIGINT PRIMARY KEY, obs TEXT);
        CREATE TABLE t_desc (id INTEGER PRIMARY KEY DESC, obs TEXT);
    "#,
        1000,
    )
    .unwrap();
    let falso = b.tabela_info("t_falso").unwrap();
    let id = por_nome(&falso.colunas)["id"].clone();
    assert!(id.rowid_alias && !id.autoincremento);
    let verdadeiro = b.tabela_info("t_verdadeiro").unwrap();
    let id = por_nome(&verdadeiro.colunas)["id"].clone();
    assert!(id.rowid_alias && id.autoincremento);
    assert!(!b.tabela_info("t_bigint").unwrap().colunas[0].rowid_alias);
    assert!(!b.tabela_info("t_desc").unwrap().colunas[0].rowid_alias); // peculiaridade documentada do SQLite
}

#[test]
fn identidade_quando_rowid_esta_sombreado() {
    let ex = exemplo("sombreado");
    let b = &ex.banco;
    b.consultar(
        "
        CREATE TABLE s1 (rowid TEXT, v TEXT);
        CREATE TABLE s2 (rowid TEXT, oid TEXT, v TEXT);
        CREATE TABLE s3 (rowid TEXT, oid TEXT, _rowid_ TEXT, v TEXT PRIMARY KEY);
        CREATE TABLE s4 (rowid TEXT, oid TEXT, _rowid_ TEXT);
        INSERT INTO s1 VALUES ('texto', 'a');
    ",
        1000,
    )
    .unwrap();
    assert_eq!(b.tabela_info("s1").unwrap().chave, rowid_alias("oid"));
    assert_eq!(b.tabela_info("s2").unwrap().chave, rowid_alias("_rowid_"));
    assert_eq!(b.tabela_info("s3").unwrap().chave, Some(TipoChave::Pk { colunas: vec!["v".into()] }));
    let s4 = b.tabela_info("s4").unwrap();
    assert!(s4.chave.is_none() && !s4.editavel);
    let p = pag(b, "s1", |_| {});
    assert_eq!(p.linhas, vec![LinhaPagina { chave: Some(Chave::Rowid(1)), valores: vec![ct("texto"), ct("a")] }]);
}
