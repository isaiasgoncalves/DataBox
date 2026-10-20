//! Gravação: inserir, atualizar, excluir, restrições e as regressões dos achados da revisão.

use comum::{Celula, Chave, Entrada, Valor};

use super::*;
use crate::erros::MSG_SALVAR;
use crate::gravacao::savepoint;
use crate::meta::Meta;
use crate::{Operador, ResultadoComando, TipoChave};

fn rv(c: &Celula) -> i64 {
    match c {
        Celula::Inteiro(i) => *i,
        outra => panic!("{outra:?}"),
    }
}

#[test]
fn inserir_cliente_com_padrao_no_id_e_na_data() {
    let ex = exemplo("inserir-cliente");
    let r = ex
        .banco
        .inserir(
            "clientes",
            &vals(&[
                ("id", Entrada::Padrao),
                ("nome", tx("Zélia Teste")),
                ("email", tx("zelia@example.test")),
                ("limite", tx("1500,50")),
                ("criado_em", Entrada::Padrao),
            ]),
        )
        .unwrap();
    assert_eq!(r.chave, Some(Chave::Rowid(N_CLIENTES as i64 + 1)));
    let linha = r.linha.unwrap();
    assert_eq!(linha[0], ci(N_CLIENTES as i64 + 1));
    assert_eq!(linha[1], ct("Zélia Teste"));
    assert_eq!(linha[3], ct("São Paulo")); // DEFAULT
    assert_eq!(linha[4], Celula::Real(1500.5));
    assert_eq!(linha[5], ci(1)); // DEFAULT 1
    let data = linha[6].texto();
    let formato = "dddd-dd-dd dd:dd:dd";
    assert!(
        data.len() == formato.len()
            && data.chars().zip(formato.chars()).all(|(c, f)| if f == 'd' { c.is_ascii_digit() } else { c == f }),
        "{data}"
    );
    assert_eq!(cru_int(&ex.caminho, "SELECT seq FROM sqlite_sequence WHERE name = 'clientes'"), N_CLIENTES as i64 + 1);
}

#[test]
fn inserir_produto_sem_rowid_ignora_gerada() {
    let ex = exemplo("inserir-produto");
    let foto: Vec<u8> = (0..40).collect();
    let r = ex
        .banco
        .inserir(
            "produtos",
            &vals(&[
                ("codigo", tx("P900")),
                ("descricao", tx("Clipe")),
                ("preco", tx("2,50")),
                ("estoque", tx("7")),
                ("foto", Entrada::Blob(foto.clone())),
                ("serie", Entrada::Inteiro(9007199254741999)),
                ("preco_centavos", Entrada::Inteiro(1)),
            ]),
        )
        .unwrap();
    assert_eq!(r.chave, Some(pk(&[("codigo", vt("P900"))])));
    assert_eq!(
        r.linha.unwrap(),
        vec![
            ct("P900"),
            ct("Clipe"),
            Celula::Real(2.5),
            ci(7),
            Celula::Blob { tamanho: 40, inicio: foto[..24].to_vec() },
            ci(9007199254741999),
            ci(250)
        ]
    );
    assert_eq!(
        cru(&ex.caminho, "SELECT foto, serie FROM produtos WHERE codigo = 'P900'", &[]),
        vec![vec![Valor::Blob(foto), Valor::Inteiro(9007199254741999)]]
    );
}

#[test]
fn inserir_blob_de_arquivo() {
    let ex = exemplo("blob-arquivo");
    let arquivo = ex.pasta.join("foto.bin");
    std::fs::write(&arquivo, [0u8; 500]).unwrap();
    let r = ex
        .banco
        .inserir(
            "produtos",
            &vals(&[("codigo", tx("P901")), ("descricao", tx("Com foto")), ("foto", Entrada::Arquivo(arquivo))]),
        )
        .unwrap();
    assert_eq!(r.linha.unwrap()[4], Celula::Blob { tamanho: 500, inicio: vec![0; 24] });
    assert_eq!(ex.banco.ler_blob("produtos", r.chave.as_ref().unwrap(), "foto").unwrap(), Some(vec![0u8; 500]));
}

#[test]
fn inserir_coluna_desconhecida() {
    let ex = exemplo("coluna-desconhecida");
    let e = erro(ex.banco.inserir("clientes", &vals(&[("nome", tx("X")), ("nao_existe", Entrada::Inteiro(1))])));
    assert_eq!(e.status, 400);
}

#[test]
fn inserir_sem_valores_usa_default_values() {
    let ex = exemplo("default-values");
    let r = ex.banco.inserir(TAB_ESPACO, &[]).unwrap();
    assert_eq!(r.linha, Some(vec![Celula::Nulo, Celula::Nulo]));
    assert_eq!(contar(&ex.banco, TAB_ESPACO), 5);
}

#[test]
fn erros_de_restricao_traduzidos() {
    let ex = exemplo("restricoes");
    let b = &ex.banco;
    let email_do_1 = b.linha("clientes", &Chave::Rowid(1)).unwrap().valor("email").unwrap().texto();
    type Caso<'a> = (&'a str, Vec<(String, Entrada)>, &'a str);
    let casos: Vec<Caso<'_>> = vec![
        ("clientes", vals(&[("nome", tx("Repetido")), ("email", tx(&email_do_1))]), "não aceita repetição"),
        ("clientes", vals(&[("nome", Entrada::Nulo)]), "não aceita valor nulo"),
        ("clientes", vals(&[("nome", tx("Negativo")), ("limite", tx("-1"))]), "regra de verificação"),
        (
            "pedidos",
            vals(&[("cliente_id", Entrada::Inteiro(9999)), ("feito_em", tx("2026-09-22 10:00:00"))]),
            "chave estrangeira",
        ),
        (
            "pedidos",
            vals(&[
                ("cliente_id", Entrada::Inteiro(1)),
                ("feito_em", tx("2026-09-22 10:00:00")),
                ("status", tx("perdido")),
            ]),
            "regra de verificação",
        ),
    ];
    for (tabela, valores, trecho) in casos {
        let e = erro(b.inserir(tabela, &valores));
        assert_eq!(e.status, 400);
        assert!(e.mensagem.starts_with("Não foi possível salvar a linha."), "{}", e.mensagem);
        assert!(e.mensagem.contains(trecho), "{}", e.mensagem);
        assert!(e.detalhe.is_some_and(|d| !d.is_empty()));
    }
}

#[test]
fn atualizar_com_gatilho_relê_a_linha() {
    let ex = exemplo("gatilho");
    let r = ex.banco.atualizar("pedidos", &Chave::Rowid(3), &vals(&[("status", tx("cancelado"))])).unwrap();
    assert_eq!(r.chave, Some(Chave::Rowid(3)));
    let linha = r.linha.unwrap();
    assert_eq!(linha[3], ct("cancelado"));
    assert!(linha[4].texto().ends_with("cancelado")); // escrito pelo gatilho trg_pedido_cancelado
}

#[test]
fn atualizar_mudando_a_chave() {
    let ex = exemplo("mudando-chave");
    let b = &ex.banco;
    let novo = b
        .inserir("pedidos", &vals(&[("cliente_id", Entrada::Inteiro(2)), ("feito_em", tx("2026-09-22 09:00:00"))]))
        .unwrap();
    assert_eq!(novo.chave, Some(Chave::Rowid(N_PEDIDOS as i64 + 1)));
    let chave = novo.chave.unwrap();
    let r = b.atualizar("pedidos", &chave, &vals(&[("id", tx("5007"))])).unwrap();
    assert!(r.chave == Some(Chave::Rowid(5007)) && r.linha.unwrap()[0] == ci(5007));
    assert_eq!(erro(b.linha("pedidos", &chave)).status, 404);
    // pedido com itens: a FK de itens_pedido (sem ON UPDATE) impede trocar o id
    let e = erro(b.atualizar("pedidos", &Chave::Rowid(7), &vals(&[("id", Entrada::Inteiro(6007))])));
    assert!(e.mensagem.contains("chave estrangeira"), "{}", e.mensagem);

    b.inserir("produtos", &vals(&[("codigo", tx("P950")), ("descricao", tx("Provisório"))])).unwrap();
    let r = b
        .atualizar(
            "produtos",
            &pk(&[("codigo", vt("P950"))]),
            &vals(&[("codigo", tx("P951")), ("preco", tx("3")), ("preco_centavos", Entrada::Inteiro(9))]),
        )
        .unwrap();
    assert_eq!(r.chave, Some(pk(&[("codigo", vt("P951"))])));
    let linha = r.linha.unwrap();
    assert!(linha[0] == ct("P951") && linha[6] == ci(300));
}

#[test]
fn atualizar_padrao_e_erro_manter_pula() {
    let ex = exemplo("padrao-manter");
    let b = &ex.banco;
    assert_eq!(erro(b.atualizar("clientes", &Chave::Rowid(1), &vals(&[("cidade", Entrada::Padrao)]))).status, 400);
    let antes = b.linha("clientes", &Chave::Rowid(1)).unwrap();
    let r = b
        .atualizar("clientes", &Chave::Rowid(1), &vals(&[("cidade", Entrada::Manter), ("nome", tx("Nome Trocado"))]))
        .unwrap();
    let linha = r.linha.unwrap();
    assert!(linha[1] == ct("Nome Trocado") && Some(&linha[3]) == antes.valor("cidade"));
    let r = b.atualizar("clientes", &Chave::Rowid(1), &vals(&[("cidade", Entrada::Manter)])).unwrap();
    assert_eq!(r.chave, Some(Chave::Rowid(1)));
}

/// O Desfazer da grade (prancheta 15): o valor de antes volta só se a célula ainda tem o que a edição gravou; NULL e
/// número voltam com o tipo de antes; só leitura, texto cortado e linha que sumiu não gravam nada.
#[test]
fn desfazer_celula_so_com_o_valor_gravado() {
    let ex = exemplo("desfazer-celula");
    let b = &ex.banco;
    let um = Chave::Rowid(1);
    let antes = b.linha("clientes", &um).unwrap().valor("cidade").cloned().unwrap();
    let gravado = b.atualizar("clientes", &um, &vals(&[("cidade", tx("Natal"))])).unwrap().linha.unwrap()[3].clone();
    assert_eq!(gravado, ct("Natal"));

    // outro comando mudou a célula depois: 409 e ela fica como está
    b.atualizar("clientes", &um, &vals(&[("cidade", tx("Recife"))])).unwrap();
    let e = erro(b.desfazer_celula("clientes", &um, "cidade", &gravado, &antes));
    assert_eq!((e.status, e.mensagem.as_str()), (409, crate::MSG_CELULA_MUDOU));
    assert_eq!(b.linha("clientes", &um).unwrap().valor("cidade"), Some(&ct("Recife")));

    // com o valor gravado na célula, ele volta
    b.atualizar("clientes", &um, &vals(&[("cidade", tx("Natal"))])).unwrap();
    let r = b.desfazer_celula("clientes", &um, "cidade", &gravado, &antes).unwrap();
    assert_eq!((r.chave, r.linha.unwrap()[3].clone()), (Some(um.clone()), antes.clone()));

    // NULL e número voltam como eram (o texto "154,03" não passa pela coerção da digitação)
    let r = b.atualizar("clientes", &um, &vals(&[("cidade", Entrada::Nulo), ("limite", tx("7"))])).unwrap();
    let linha = r.linha.unwrap();
    b.desfazer_celula("clientes", &um, "cidade", &linha[3], &antes).unwrap();
    b.desfazer_celula("clientes", &um, "limite", &linha[4], &Celula::Real(154.03)).unwrap();
    let r = b.desfazer_celula("clientes", &um, "email", &ct("lucas.1@example.test"), &Celula::Nulo).unwrap();
    let linha = r.linha.unwrap();
    assert_eq!((&linha[2], &linha[3], &linha[4]), (&Celula::Nulo, &antes, &Celula::Real(154.03)));

    // a PK editada volta, e a chave da resposta é a de antes
    let novo = b.inserir("clientes", &vals(&[("nome", tx("Provisório"))])).unwrap().chave.unwrap();
    let id = rowid(&Some(novo.clone()));
    let r = b.atualizar("clientes", &novo, &vals(&[("id", Entrada::Inteiro(9001))])).unwrap();
    let r = b.desfazer_celula("clientes", &r.chave.unwrap(), "id", &ci(9001), &ci(id)).unwrap();
    assert_eq!(r.chave, Some(novo));

    let cortado = Celula::TextoCortado { inicio: "x".into(), tamanho: 900 };
    assert_eq!(erro(b.desfazer_celula("clientes", &um, "nome", &ct("y"), &cortado)).status, 400);
    assert_eq!(erro(b.desfazer_celula("clientes", &Chave::Rowid(9999), "cidade", &ct("y"), &antes)).status, 404);
    let ro = exemplo_ro("desfazer-ro");
    assert_eq!(erro(ro.banco.desfazer_celula("clientes", &um, "cidade", &antes, &ct("x"))).status, 403);
}

#[test]
fn atualizar_linha_que_sumiu() {
    let ex = exemplo("sumiu");
    assert_eq!(erro(ex.banco.atualizar("clientes", &Chave::Rowid(9999), &vals(&[("nome", tx("X"))]))).status, 404);
}

#[test]
fn atualizar_quebrando_fk() {
    let ex = exemplo("quebrando-fk");
    let e = erro(ex.banco.atualizar("produtos", &pk(&[("codigo", vt("P001"))]), &vals(&[("codigo", tx("P777"))])));
    assert!(e.mensagem.contains("chave estrangeira"));
}

#[test]
fn editar_itens_pedido_pk_composta_com_rowid() {
    let ex = exemplo("itens");
    let b = &ex.banco;
    let p = pag(b, "itens_pedido", |x| x.por_pagina = 2);
    let chave = p.linhas[0].chave.clone().unwrap();
    assert!(matches!(chave, Chave::Rowid(_)));
    let r = b.atualizar("itens_pedido", &chave, &vals(&[("quantidade", tx("42"))])).unwrap();
    assert_eq!(r.linha.unwrap()[2], ci(42));
    let duplicado = &p.linhas[1].valores;
    let e = erro(b.inserir(
        "itens_pedido",
        &vals(&[("pedido_id", Entrada::Inteiro(rv(&duplicado[0]))), ("produto", tx(&duplicado[1].texto()))]),
    ));
    assert!(e.mensagem.contains("não aceita repetição"), "{}", e.mensagem);
    assert_eq!(b.excluir("itens_pedido", &[chave, p.linhas[1].chave.clone().unwrap()]).unwrap(), 2);
    assert_eq!(contar(b, "itens_pedido"), 454);
}

#[test]
fn sem_rowid_com_pk_composta() {
    let ex = exemplo("pk-composta");
    let b = &ex.banco;
    b.consultar(
        "CREATE TABLE lotes (pedido_id INTEGER, produto TEXT, qtd INTEGER, PRIMARY KEY (pedido_id, produto)) WITHOUT ROWID;
         INSERT INTO lotes VALUES (7, 'P003', 1), (7, 'P004', 2), (8, 'P003', 3);",
        1000,
    )
    .unwrap();
    let info = b.tabela_info("lotes").unwrap();
    assert_eq!(info.chave, Some(TipoChave::Pk { colunas: vec!["pedido_id".into(), "produto".into()] }));
    let p = pag(b, "lotes", |x| (x.ordem, x.direcao) = (Some("qtd".into()), crate::Direcao::Desc));
    let chave = |pedido: i64, produto: &str| pk(&[("pedido_id", Valor::Inteiro(pedido)), ("produto", vt(produto))]);
    assert_eq!(
        p.linhas.iter().map(|l| l.chave.clone().unwrap()).collect::<Vec<_>>(),
        [chave(8, "P003"), chave(7, "P004"), chave(7, "P003")]
    );
    let linha = b.linha("lotes", &chave(7, "P003")).unwrap();
    assert_eq!(
        linha.valores,
        vec![("pedido_id".to_string(), ci(7)), ("produto".to_string(), ct("P003")), ("qtd".to_string(), ci(1))]
    );
    let r = b.atualizar("lotes", &chave(7, "P003"), &vals(&[("produto", tx("P009")), ("qtd", tx("5"))])).unwrap();
    assert_eq!(r.chave, Some(chave(7, "P009")));
    assert_eq!(r.linha, Some(vec![ci(7), ct("P009"), ci(5)]));
    assert_eq!(b.excluir("lotes", &[r.chave.unwrap(), chave(8, "P003")]).unwrap(), 2);
    assert_eq!(
        cru(&ex.caminho, "SELECT * FROM lotes", &[]),
        vec![vec![Valor::Inteiro(7), vt("P004"), Valor::Inteiro(2)]]
    );
}

#[test]
fn excluir_varias_numa_transacao() {
    let ex = exemplo("excluir-varias");
    let b = &ex.banco;
    let chaves = [1, 2, 3, 99999].map(Chave::Rowid);
    assert_eq!(b.excluir("eventos", &chaves).unwrap(), 3);
    assert_eq!(contar(b, "eventos"), N_EVENTOS - 3);
    assert_eq!(b.excluir("eventos", &[]).unwrap(), 0);
}

#[test]
fn excluir_e_tudo_ou_nada() {
    let ex = exemplo("tudo-ou-nada");
    let b = &ex.banco;
    b.inserir("produtos", &vals(&[("codigo", tx("P960")), ("descricao", tx("Sem itens"))])).unwrap();
    let e = erro(b.excluir("produtos", &[pk(&[("codigo", vt("P960"))]), pk(&[("codigo", vt("P001"))])]));
    assert!(e.mensagem.contains("chave estrangeira"));
    assert_eq!(cru_int(&ex.caminho, "SELECT count(*) FROM produtos WHERE codigo = 'P960'"), 1);
    b.com_conexao(|conn| {
        assert!(conn.is_autocommit());
        Ok(())
    })
    .unwrap();
}

#[test]
fn excluir_em_cascata() {
    let ex = exemplo("cascata");
    let pedidos_do_1 = cru_int(&ex.caminho, "SELECT count(*) FROM pedidos WHERE cliente_id = 1") as u64;
    assert_eq!(ex.banco.excluir("clientes", &[Chave::Rowid(1)]).unwrap(), 1);
    assert_eq!(contar(&ex.banco, "pedidos"), N_PEDIDOS - pedidos_do_1);
}

#[test]
fn nomes_com_espaco_e_aspas_gravacao() {
    let ex = exemplo("nomes-gravacao");
    let b = &ex.banco;
    let r = b.inserir(TAB_ESPACO, &vals(&[(COL_CITADA, tx("valor \"com\" aspas")), ("número", tx("  15 "))])).unwrap();
    assert_eq!(r.linha, Some(vec![ct("valor \"com\" aspas"), ci(15)]));
    let r = b.atualizar(TAB_ESPACO, r.chave.as_ref().unwrap(), &vals(&[(COL_CITADA, tx("trocado"))])).unwrap();
    assert_eq!(r.linha, Some(vec![ct("trocado"), ci(15)]));
    let p = pag(b, TAB_ESPACO, |x| x.filtros = vec![filtro(COL_CITADA, Operador::Igual, tx("trocado"))]);
    assert_eq!(p.total, Some(1));
    assert_eq!(b.excluir(TAB_ESPACO, &[r.chave.unwrap()]).unwrap(), 1);
}

#[test]
fn fts5_editavel() {
    let ex = exemplo("fts5-editavel");
    let b = &ex.banco;
    let r = b
        .inserir("notas", &vals(&[("titulo", tx("Nova nota")), ("corpo", tx("Conteúdo pesquisável sobre orçamento."))]))
        .unwrap();
    assert_eq!(r.chave, Some(Chave::Rowid(3)));
    assert_eq!(r.linha, Some(vec![ct("Nova nota"), ct("Conteúdo pesquisável sobre orçamento.")]));
    let r = b.atualizar("notas", r.chave.as_ref().unwrap(), &vals(&[("titulo", tx("Nota revista"))])).unwrap();
    assert_eq!(r.linha.unwrap()[0], ct("Nota revista"));
    assert_eq!(pag(b, "notas", |x| x.busca = Some("orçamento".into())).total, Some(1));
    assert_eq!(b.excluir("notas", &[r.chave.unwrap()]).unwrap(), 1);
}

#[test]
fn gravar_em_visao_e_sistema_da_403() {
    let ex = exemplo("visao-403");
    let b = &ex.banco;
    assert_eq!(erro(b.inserir("v_pedidos_por_cliente", &vals(&[("nome", tx("X"))]))).status, 403);
    assert_eq!(erro(b.excluir("notas_data", &[Chave::Rowid(1)])).status, 403);
    assert_eq!(erro(b.atualizar("sqlite_schema", &Chave::Rowid(1), &vals(&[("name", tx("x"))]))).status, 403);
    let r = b.atualizar("sqlite_sequence", &Chave::Rowid(1), &vals(&[("seq", Entrada::Inteiro(500))])).unwrap();
    assert_eq!(r.linha, Some(vec![ct("clientes"), ci(500)]));
}

#[test]
fn somente_leitura_recusa_gravar() {
    let ex = exemplo_ro("ro-gravar");
    let b = &ex.banco;
    assert_eq!(erro(b.inserir("clientes", &vals(&[("nome", tx("X"))]))).status, 403);
    assert_eq!(erro(b.atualizar("clientes", &Chave::Rowid(1), &vals(&[("nome", tx("X"))]))).status, 403);
    assert_eq!(erro(b.excluir("clientes", &[Chave::Rowid(1)])).status, 403);
    assert_eq!(erro(b.manutencao(crate::AcaoManutencao::Vacuum)).status, 403);
    let r = b.consultar("DELETE FROM clientes", 1000).unwrap();
    match &r.resultados[0] {
        crate::ResultadoComando::Erro { mensagem, .. } => assert!(mensagem.contains("só para leitura"), "{mensagem}"),
        outro => panic!("{outro:?}"),
    }
    assert_eq!(contar(b, "clientes"), N_CLIENTES);
    assert!(b.manutencao(crate::AcaoManutencao::Integridade).unwrap().ok);
}

// ---------------------------------------------------------------------------------------------------
// Regressões dos achados da revisão
// ---------------------------------------------------------------------------------------------------

#[test]
fn pk_nula_com_rowid_sombreado_nao_tem_identidade() {
    // rowid, oid e _rowid_ são colunas: a identidade vira a PK, que aceita NULL; "codigo IS NULL" pegava todas
    let tmp = Pasta::nova("pk-nula");
    let p = banco_de(
        &tmp,
        "CREATE TABLE a (rowid TEXT, oid TEXT, _rowid_ TEXT, codigo TEXT PRIMARY KEY, v TEXT);
         INSERT INTO a VALUES ('r1','o1','x1', NULL, 'primeira'), ('r2','o2','x2', NULL, 'segunda'),
                              ('r3','o3','x3','C3','terceira');",
        "caso.db",
    );
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        let chaves: Vec<_> = pag(&b, "a", |_| {}).linhas.into_iter().map(|l| l.chave).collect();
        assert_eq!(chaves, [None, None, Some(pk(&[("codigo", vt("C3"))]))]);
        let nula = pk(&[("codigo", Valor::Nulo)]);
        assert_eq!(erro(b.atualizar("a", &nula, &vals(&[("v", tx("EDITADA"))]))).status, 400);
        assert_eq!(erro(b.excluir("a", &[nula])).status, 400);
        let r = b.atualizar("a", &pk(&[("codigo", vt("C3"))]), &vals(&[("v", tx("ok"))])).unwrap();
        assert_eq!(r.linha.unwrap().last(), Some(&ct("ok")));
        assert!(!b.transacao_aberta());
    }
    assert_eq!(
        cru(&p, "SELECT codigo, v FROM a", &[]),
        vec![vec![Valor::Nulo, vt("primeira")], vec![Valor::Nulo, vt("segunda")], vec![vt("C3"), vt("ok")]]
    );
}

#[test]
fn gravacao_que_atingiria_mais_de_uma_linha_e_desfeita() {
    // Proteção geral: se a chave casar com mais de uma linha, nada é salvo nem excluído.
    let tmp = Pasta::nova("mais-de-uma");
    let p =
        banco_de(&tmp, "CREATE TABLE t (codigo TEXT, v TEXT); INSERT INTO t VALUES ('C1','a'), ('C1','b');", "caso.db");
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        fn pk_sem_unicidade(m: &mut Meta) {
            m.chave = Some(TipoChave::Pk { colunas: vec!["codigo".into()] });
            m.wr = true;
        }
        *b.ajuste_meta.lock() = Some(pk_sem_unicidade);
        let chave = pk(&[("codigo", vt("C1"))]);
        assert_eq!(erro(b.atualizar("t", &chave, &vals(&[("v", tx("x"))]))).status, 409);
        assert_eq!(erro(b.excluir("t", &[chave])).status, 409);
        assert!(!b.transacao_aberta());
    }
    assert_eq!(cru(&p, "SELECT v FROM t ORDER BY v", &[]), vec![vec![vt("a")], vec![vt("b")]]);
}

#[test]
fn savepoint_desfaz_em_qualquer_erro() {
    let ex = exemplo("savepoint");
    let b = &ex.banco;
    b.com_conexao(|conn| {
        let r: comum::Resultado<()> = savepoint(
            conn,
            "teste",
            |e| crate::erros::traduzir(e, None),
            || {
                conn.execute_batch("DELETE FROM eventos").unwrap();
                Err(comum::ErroBanco::novo("qualquer"))
            },
        );
        assert_eq!(r.unwrap_err().mensagem, "qualquer");
        assert!(conn.is_autocommit());
        // um pânico no meio também desfaz
        let panico = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: comum::Resultado<()> = savepoint(
                conn,
                "teste",
                |e| crate::erros::traduzir(e, None),
                || {
                    conn.execute_batch("DELETE FROM eventos").unwrap();
                    panic!("qualquer");
                },
            );
        }));
        assert!(panico.is_err());
        assert!(conn.is_autocommit());
        Ok(())
    })
    .unwrap();
    assert_eq!(contar(b, "eventos"), N_EVENTOS);
}

#[test]
fn chave_real_grande_que_volta_do_navegador_como_inteiro() {
    // 1e19 não pode virar o inteiro 10000000000000000000: a chave guarda o REAL exato.
    let tmp = Pasta::nova("real-grande");
    let p = banco_de(
        &tmp,
        "CREATE TABLE e (k REAL PRIMARY KEY, v TEXT) WITHOUT ROWID;\
         INSERT INTO e VALUES (1e19, 'dezenove'), (1e20, 'vinte'), (1.5, 'pequeno');",
        "caso.db",
    );
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        for valor in [1e19, 1e20] {
            let chave = pk(&[("k", Valor::Real(valor))]);
            assert_eq!(b.linha("e", &chave).unwrap().valor("k"), Some(&Celula::Real(valor)));
            assert_eq!(b.atualizar("e", &chave, &vals(&[("v", tx("EDIT"))])).unwrap().linha.unwrap()[1], ct("EDIT"));
        }
        assert_eq!(b.excluir("e", &[pk(&[("k", Valor::Real(1e19))])]).unwrap(), 1);
        assert!(!b.transacao_aberta());
    }
    assert_eq!(
        cru(&p, "SELECT k, v FROM e ORDER BY k", &[]),
        vec![vec![Valor::Real(1.5), vt("pequeno")], vec![Valor::Real(1e20), vt("EDIT")]]
    );
}

#[test]
fn pk_texto_com_utf8_invalido() {
    // Texto gravado em Latin-1 por outro programa: a grade mostra "�", e a chave precisa dos bytes originais.
    let tmp = Pasta::nova("utf8-invalido");
    let p = banco_de(
        &tmp,
        "CREATE TABLE cidades (nome TEXT PRIMARY KEY, uf TEXT) WITHOUT ROWID;
         INSERT INTO cidades VALUES (CAST(x'53e36f205061756c6f' AS TEXT), 'SP'), ('Recife', 'PE'), ('Açaí \u{fffd}', 'PA');",
        "caso.db",
    );
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        let por_uf: std::collections::HashMap<String, Chave> =
            pag(&b, "cidades", |_| {}).linhas.into_iter().map(|l| (l.valores[1].texto(), l.chave.unwrap())).collect();
        assert_eq!(por_uf["PE"], pk(&[("nome", vt("Recife"))]));
        assert_eq!(por_uf["PA"], pk(&[("nome", vt("Açaí \u{fffd}"))])); // "�" de verdade no texto: chave comum
        let sp = por_uf["SP"].clone();
        assert_eq!(sp, pk(&[("nome", Valor::TextoBruto(b"S\xe3o Paulo".to_vec()))]));
        assert_eq!(b.linha("cidades", &sp).unwrap().valor("uf"), Some(&ct("SP")));
        assert_eq!(b.atualizar("cidades", &sp, &vals(&[("uf", tx("RJ"))])).unwrap().linha.unwrap()[1], ct("RJ"));
        assert_eq!(b.linha("cidades", &por_uf["PA"]).unwrap().valor("uf"), Some(&ct("PA")));
        assert_eq!(b.excluir("cidades", &[sp]).unwrap(), 1);
    }
    assert!(cru(&p, "SELECT hex(nome) FROM cidades WHERE uf IN ('SP', 'RJ')", &[]).is_empty());
}

#[test]
fn coluna_any_de_tabela_strict_guarda_o_texto_como_digitado() {
    let tmp = Pasta::nova("strict");
    let p = banco_de(
        &tmp,
        "CREATE TABLE s (id INTEGER PRIMARY KEY, v ANY) STRICT; INSERT INTO s VALUES (1, 'x'), (2, 'y');",
        "caso.db",
    );
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        assert_eq!(b.tabela_info("s").unwrap().colunas[1].afinidade, comum::Afinidade::Blob);
        b.atualizar("s", &Chave::Rowid(1), &vals(&[("v", tx("007"))])).unwrap();
        b.atualizar("s", &Chave::Rowid(2), &vals(&[("v", tx("1.50"))])).unwrap();
    }
    assert_eq!(
        cru(&p, "SELECT typeof(v), v FROM s ORDER BY id", &[]),
        vec![vec![vt("text"), vt("007")], vec![vt("text"), vt("1.50")]]
    );
}

#[test]
fn coluna_sem_tipo_com_numeros_edicao_e_filtros() {
    let tmp = Pasta::nova("sem-tipo");
    let p = banco_de(
        &tmp,
        "CREATE TABLE u (id INTEGER PRIMARY KEY, qtd, obs);\
         INSERT INTO u VALUES (1, 5, 'a'), (2, 10, 'b'), (3, 7, 'c'), (4, '10', 'texto');",
        "caso.db",
    );
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        b.atualizar("u", &Chave::Rowid(1), &vals(&[("qtd", tx("6"))])).unwrap(); // a grade sempre manda o texto digitado
        let ids_de = |op, valor: &str| ids(&pag(&b, "u", |x| x.filtros = vec![filtro("qtd", op, tx(valor))]));
        assert_eq!(ids_de(Operador::Igual, "10"), [2, 4]); // o número 10 e o texto '10'
        assert_eq!(ids_de(Operador::Diferente, "10"), [1, 3]);
        assert_eq!(ids_de(Operador::Maior, "6"), [2, 3, 4]); // o texto '10' é maior que qualquer número, como no SQLite
        assert_eq!(ids_de(Operador::Menor, "7"), [1]);
        let ordem: Vec<Celula> =
            pag(&b, "u", |x| x.ordem = Some("qtd".into())).linhas.into_iter().map(|l| l.valores[1].clone()).collect();
        assert_eq!(ordem, [ci(6), ci(7), ci(10), ct("10")]);
    }
    assert_eq!(
        cru(&p, "SELECT typeof(qtd), qtd FROM u WHERE id = 1", &[]),
        vec![vec![vt("integer"), Valor::Inteiro(6)]]
    );
}

#[test]
fn on_conflict_ignore_e_raise_ignore_explicam_o_que_houve() {
    let tmp = Pasta::nova("ignore");
    let p = banco_de(
        &tmp,
        "CREATE TABLE contatos (id INTEGER PRIMARY KEY, email TEXT UNIQUE ON CONFLICT IGNORE, nome TEXT);
         INSERT INTO contatos VALUES (1, 'ana@example.test', 'Ana'), (2, 'bia@example.test', 'Bia');
         CREATE TABLE log (id INTEGER PRIMARY KEY, msg TEXT);
         CREATE TRIGGER so_maiusculas BEFORE INSERT ON log WHEN new.msg <> upper(new.msg) BEGIN SELECT RAISE(IGNORE); END;",
        "caso.db",
    );
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        let e = erro(b.inserir("contatos", &vals(&[("email", tx("ana@example.test")), ("nome", tx("Outra"))])));
        assert!(e.status == 409 && e.mensagem.contains("ignorou a linha nova"), "{e:?}");
        let e = erro(b.atualizar("contatos", &Chave::Rowid(2), &vals(&[("email", tx("ana@example.test"))])));
        assert!(e.status == 409 && e.mensagem.contains("continua como estava"), "{e:?}");
        assert_eq!(erro(b.atualizar("contatos", &Chave::Rowid(99), &vals(&[("nome", tx("x"))]))).status, 404);
        assert_eq!(erro(b.inserir("log", &vals(&[("msg", tx("minúsculas"))]))).status, 409);
        assert!(!b.transacao_aberta());
    }
    assert_eq!(
        cru(&p, "SELECT * FROM contatos WHERE id = 2", &[]),
        vec![vec![Valor::Inteiro(2), vt("bia@example.test"), vt("Bia")]]
    );
}

/// Regressão: um gatilho gravado no arquivo com RAISE(ABORT, 'İ UNIQUE constraint failed: é') dava pânico na
/// tradução do erro ('İ' ocupa 3 bytes em minúscula e 2 no original).
#[test]
fn restricao_com_texto_que_muda_de_tamanho_em_minuscula() {
    let tmp = Pasta::nova("restricao-unicode");
    let p = banco_de(
        &tmp,
        "CREATE TABLE t (v TEXT);
         CREATE TRIGGER t_ins BEFORE INSERT ON t BEGIN SELECT RAISE(ABORT, 'İ UNIQUE constraint failed: é'); END;
         CREATE TABLE u (a TEXT);
         INSERT INTO u (a) VALUES ('y');
         CREATE TRIGGER u_ins BEFORE INSERT ON u BEGIN SELECT RAISE(ABORT, 'İİ NOT NULL constraint failed: a'); END;
         CREATE TRIGGER u_upd BEFORE UPDATE ON u BEGIN SELECT RAISE(ABORT, 'İ UNIQUE constraint failed: é'); END;
         CREATE TRIGGER u_del BEFORE DELETE ON u BEGIN SELECT RAISE(ABORT, 'İİ NOT NULL constraint failed: a'); END;",
        "caso.db",
    );
    const UNICA: &str = "Já existe uma linha com esse valor em é; a coluna não aceita repetição.";
    const NULA: &str = "A coluna a não aceita valor nulo.";
    const DET_UNICA: &str = "İ UNIQUE constraint failed: é";
    const DET_NULA: &str = "İİ NOT NULL constraint failed: a";
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        for (sql, mensagem, detalhe) in
            [("INSERT INTO t (v) VALUES ('x')", UNICA, DET_UNICA), ("INSERT INTO u (a) VALUES ('x')", NULA, DET_NULA)]
        {
            let r = b.consultar(sql, 1000).unwrap();
            assert_eq!(
                r.resultados,
                vec![ResultadoComando::Erro {
                    comando: sql.to_string(),
                    mensagem: mensagem.to_string(),
                    detalhe: Some(detalhe.to_string()),
                }]
            );
        }
        let casos = [
            (erro(b.inserir("t", &vals(&[("v", tx("x"))]))), format!("{MSG_SALVAR} {UNICA}"), DET_UNICA),
            (erro(b.inserir("u", &vals(&[("a", tx("x"))]))), format!("{MSG_SALVAR} {NULA}"), DET_NULA),
            (
                erro(b.atualizar("u", &Chave::Rowid(1), &vals(&[("a", tx("z"))]))),
                format!("{MSG_SALVAR} {UNICA}"),
                DET_UNICA,
            ),
            (erro(b.excluir("u", &[Chave::Rowid(1)])), format!("Não foi possível excluir as linhas. {NULA}"), DET_NULA),
        ];
        for (e, mensagem, detalhe) in casos {
            assert_eq!((e.mensagem.as_str(), e.detalhe.as_deref(), e.status), (mensagem.as_str(), Some(detalhe), 400));
        }
        assert!(!b.transacao_aberta());
    }
    assert_eq!(cru(&p, "SELECT a FROM u", &[]), vec![vec![vt("y")]]);
    assert_eq!(cru_int(&p, "SELECT count(*) FROM t"), 0);
}

/// As colunas tiradas do texto de UNIQUE/NOT NULL, caso a caso: marcador sem diferenciar maiúsculas
/// (também 'İ' e 'ı' por 'i', 'ſ' por 's'), o resto até o fim do texto ou da quebra de linha final, os
/// espaços aparados e caracteres que mudam de tamanho em minúscula antes ou depois do marcador.
#[test]
fn colunas_da_mensagem_caso_a_caso() {
    let unica = |cols: &str| format!("Já existe uma linha com esse valor em {cols}; a coluna não aceita repetição.");
    let unica_sem = "Já existe uma linha com esse valor; a coluna não aceita repetição.".to_string();
    let casos: Vec<(&str, String)> = vec![
        ("İ UNIQUE constraint failed: é", unica("é")),
        ("İİ NOT NULL constraint failed: a", "A coluna a não aceita valor nulo.".to_string()),
        ("UNIQUE constraint failed: x.a\nb", unica_sem.clone()),
        ("UNIQUE constraint failed: x.a\n", unica("a")),
        ("UNIQUE constraint failed x; CONSTRAİNT FAİLED: t.b", unica("b")),
        ("UNIQUE constraint failed x; CONSTRAıNT FAıLED: t.b", unica("b")),
        ("UNIQUE constraint failed x; ConStRaint failed: t.b", unica("b")),
        ("\u{212A} UNIQUE constraint failed: a", unica("a")),
        ("UNIQUE constraint failed: \u{1c} a.b \u{1c}", unica("b")),
        ("UNIQUE constraint failed: ", unica_sem.clone()),
        ("UNIQUE constraint failed: a.b, c", unica("b, c")),
        ("UNIQUE constraint failed: a\nUNIQUE constraint failed: b.c", unica("c")),
        ("NOT NULL constraint failed: ſ.x", "A coluna x não aceita valor nulo.".to_string()),
        ("UNIQUE constraint failed: a.b.c , d", unica("b.c, d")),
        ("UNIQUE constraint failed: ,", unica(", ")),
        ("unique constraint failed: Ⱥ.É", unica("É")),
        ("NOT NULL constraint failed", "Uma coluna obrigatória ficou sem valor.".to_string()),
    ];
    for (texto, esperada) in casos {
        let falha = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some(texto.to_string()),
        );
        let e = crate::erros::traduzir(falha, None);
        assert_eq!((e.mensagem.as_str(), e.detalhe.as_deref(), e.status), (esperada.as_str(), Some(texto), 400));
    }
    // o contexto antecede a frase, e as outras regras continuam com o comum
    let falha = |texto: &str| {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT), Some(texto.into()))
    };
    let e = crate::erros::traduzir(falha("NOT NULL constraint failed: clientes.nome"), Some(MSG_SALVAR));
    assert_eq!(e.mensagem, "Não foi possível salvar a linha. A coluna nome não aceita valor nulo.");
    let e = crate::erros::traduzir(falha("CHECK constraint failed: preco >= 0"), None);
    assert_eq!(e.mensagem, "O valor não passa na regra de verificação da tabela (preco >= 0).");
}
