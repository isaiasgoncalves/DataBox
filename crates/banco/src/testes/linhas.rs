//! `linhas()`: páginas, ordem, busca, filtros e leitura de uma linha.

use std::time::Instant;

use comum::{Afinidade, Celula, Chave, Entrada, Valor};

use super::*;
use crate::{Direcao, Operador};

#[test]
fn linhas_pagina_padrao() {
    let ex = exemplo("pagina-padrao");
    let p = pag(&ex.banco, "clientes", |_| {});
    assert_eq!(p.colunas, ["id", "nome", "email", "cidade", "limite", "ativo", "criado_em"]);
    assert_eq!(p.tipos, ["INTEGER", "TEXT", "TEXT", "TEXT", "REAL", "INTEGER", "TEXT"]);
    use Afinidade::*;
    assert_eq!(p.afinidades, [Integer, Text, Text, Text, Real, Integer, Text]);
    assert_eq!((p.total, p.paginas), (Some(N_CLIENTES), Some(1)));
    assert_eq!((p.pagina, p.por_pagina), (1, 100));
    assert_eq!((p.ordem.as_deref(), p.direcao), (None, Direcao::Asc));
    assert_eq!(p.linhas.len() as u64, N_CLIENTES);
    let primeira = &p.linhas[0];
    assert_eq!(primeira.chave, Some(Chave::Rowid(1)));
    assert_eq!(primeira.valores[0], ci(1));
    let nome = primeira.valores[1].texto();
    let esperado = format!("{}.1@example.test", nome.split(' ').next().unwrap().to_lowercase());
    assert_eq!(primeira.valores[2], ct(&esperado));
}

#[test]
fn linhas_paginacao_e_limites() {
    let ex = exemplo("paginacao");
    let b = &ex.banco;
    let p = pag(b, "eventos", |x| (x.pagina, x.por_pagina) = (3, 1000));
    assert_eq!((p.total, p.paginas), (Some(N_EVENTOS), Some(5)));
    assert_eq!(p.linhas[0].chave, Some(Chave::Rowid(2001)));
    assert_eq!(pag(b, "eventos", |x| x.por_pagina = 5000).por_pagina, 1000);
    let um = pag(b, "eventos", |x| x.por_pagina = 0);
    assert!(um.por_pagina == 1 && um.linhas.len() == 1 && um.paginas == Some(N_EVENTOS));
    assert_eq!(pag(b, "eventos", |x| x.pagina = 0).pagina, 1);
}

#[test]
fn linhas_pagina_alem_do_fim() {
    let ex = exemplo("alem-do-fim");
    let p = pag(&ex.banco, "clientes", |x| (x.pagina, x.por_pagina) = (9, 25));
    assert!(p.linhas.is_empty());
    assert_eq!((p.total, p.paginas, p.pagina), (Some(N_CLIENTES), Some(3), 9));
}

#[test]
fn linhas_tabela_vazia_tem_uma_pagina() {
    let ex = exemplo("vazia");
    let p = pag(&ex.banco, "clientes", |x| x.filtros = vec![filtro("id", Operador::Maior, Entrada::Inteiro(999))]);
    assert!(p.total == Some(0) && p.paginas == Some(1) && p.linhas.is_empty());
}

#[test]
fn linhas_ordem_com_desempate_estavel() {
    let ex = exemplo("desempate");
    let mut vistos: Vec<(String, i64)> = Vec::new();
    for pagina in 1..=10 {
        let p = pag(&ex.banco, "eventos", |x| {
            (x.pagina, x.por_pagina, x.ordem, x.direcao) = (pagina, 500, Some("tipo".into()), Direcao::Desc)
        });
        assert!(p.ordem.as_deref() == Some("tipo") && p.direcao == Direcao::Desc);
        vistos.extend(p.linhas.iter().map(|l| (l.valores[1].texto(), rowid(&l.chave))));
    }
    assert_eq!(vistos.len() as u64, N_EVENTOS);
    assert_eq!(vistos.iter().map(|(_, r)| *r).collect::<std::collections::HashSet<_>>().len() as u64, N_EVENTOS);
    let mut esperado = vistos.clone();
    esperado.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    assert_eq!(vistos, esperado);
}

#[test]
fn linhas_sem_ordem_desc_inverte_a_identidade() {
    let ex = exemplo("desc");
    let p = pag(&ex.banco, "clientes", |x| (x.por_pagina, x.direcao) = (3, Direcao::Desc));
    assert_eq!(p.linhas.iter().map(|l| rowid(&l.chave)).collect::<Vec<_>>(), [60, 59, 58]);
}

#[test]
fn linhas_ordem_invalida_da_400() {
    let ex = exemplo("ordem-invalida");
    let b = &ex.banco;
    assert_eq!(erro(pag_r(b, "clientes", |x| x.ordem = Some("coluna_que_nao_existe".into()))).status, 400);
    assert_eq!(erro(pag_r(b, "clientes", |x| x.ordem = Some("id; DROP TABLE clientes".into()))).status, 400);
    let e = erro(Direcao::de_texto("lateral"));
    assert_eq!(e.status, 400);
    assert_eq!(e.mensagem, "Direção de ordenação inválida: lateral. Use asc ou desc.");
    assert_eq!(Direcao::de_texto("DESC").unwrap(), Direcao::Desc);
    assert_eq!(contar(b, "clientes"), N_CLIENTES);
}

#[test]
fn linhas_ordem_em_coluna_citada() {
    let ex = exemplo("ordem-citada");
    let p = pag(&ex.banco, TAB_ESPACO, |x| x.ordem = Some(COL_CITADA.into()));
    assert_eq!(p.colunas, [COL_CITADA, "número"]);
    assert_eq!(
        p.linhas.iter().map(|l| l.valores[0].clone()).collect::<Vec<_>>(),
        [Celula::Nulo, ct(""), ct("com \"aspas\""), ct("olá, mundo")]
    );
}

#[test]
fn linhas_busca_em_todas_as_colunas() {
    let ex = exemplo("busca");
    let b = &ex.banco;
    assert_eq!(pag(b, "clientes", |x| x.busca = Some("@example.test".into())).total, Some(N_CLIENTES));
    assert_eq!(pag(b, "clientes", |x| x.busca = Some("clientes.1@".into())).total, Some(0)); // nenhum e-mail tem isso
    assert_eq!(pag(b, TAB_ESPACO, |x| x.busca = Some("\"aspas\"".into())).total, Some(1));
    assert_eq!(pag(b, "clientes", |x| x.busca = Some(String::new())).total, Some(N_CLIENTES));
}

#[test]
fn linhas_busca_trata_curinga_como_literal() {
    let ex = exemplo("curinga");
    let b = &ex.banco;
    for texto in ["100% certo", "100 certo", "a_b", "axb", "barra \\ invertida"] {
        b.inserir(TAB_ESPACO, &vals(&[(COL_CITADA, tx(texto))])).unwrap();
    }
    let primeira = |busca: &str| {
        pag(b, TAB_ESPACO, |x| x.busca = Some(busca.into()))
            .linhas
            .iter()
            .map(|l| l.valores[0].texto())
            .collect::<Vec<_>>()
    };
    assert_eq!(primeira("%"), ["100% certo"]);
    assert_eq!(primeira("_"), ["a_b"]);
    assert_eq!(primeira("\\"), ["barra \\ invertida"]);
    assert_eq!(pag(b, TAB_ESPACO, |x| x.busca = Some("0% c".into())).total, Some(1));
}

fn contar_cru(caminho: &std::path::Path, onde: &str) -> u64 {
    cru_int(caminho, &format!("SELECT count(*) FROM eventos WHERE {onde}")) as u64
}

#[test]
fn cada_operador_de_filtro() {
    let ex = exemplo("operadores");
    let casos: [(&str, Operador, Entrada, &str); 14] = [
        ("tipo", Operador::Contem, tx("og"), "tipo LIKE '%og%'"),
        ("tipo", Operador::NaoContem, tx("og"), "tipo IS NULL OR tipo NOT LIKE '%og%'"),
        ("tipo", Operador::Igual, tx("erro"), "tipo = 'erro'"),
        ("tipo", Operador::Diferente, tx("erro"), "tipo IS NOT 'erro'"),
        ("valor", Operador::Maior, tx("10"), "valor > 10"),
        ("valor", Operador::MaiorIgual, tx("10,5"), "valor >= 10.5"),
        ("valor", Operador::Menor, Entrada::Inteiro(-20), "valor < -20"),
        ("valor", Operador::MenorIgual, tx("-20.25"), "valor <= -20.25"),
        ("momento", Operador::Comeca, tx("2026-09-01T"), "momento LIKE '2026-09-01T%'"),
        ("dados", Operador::Termina, tx("7}"), "dados LIKE '%7}'"),
        ("valor", Operador::Nulo, Entrada::Nulo, "valor IS NULL"),
        ("valor", Operador::NaoNulo, Entrada::Nulo, "valor IS NOT NULL"),
        ("valor", Operador::Vazio, Entrada::Nulo, "valor IS NULL OR valor = ''"),
        ("valor", Operador::NaoVazio, Entrada::Nulo, "valor IS NOT NULL AND valor <> ''"),
    ];
    for (coluna, op, valor, onde) in casos {
        let esperado = contar_cru(&ex.caminho, onde);
        assert!(0 < esperado && esperado < N_EVENTOS, "{onde}");
        let p = pag(&ex.banco, "eventos", |x| {
            x.filtros = vec![filtro(coluna, op, valor.clone())];
            x.por_pagina = 10;
        });
        assert_eq!(p.total, Some(esperado), "{op:?}");
        assert_eq!(p.paginas, Some(esperado.div_ceil(10)), "{op:?}");
    }
}

#[test]
fn operadores_de_filtro_sao_os_do_contrato() {
    let nomes: Vec<&str> = Operador::TODOS.iter().map(|o| o.como_str()).collect();
    assert_eq!(
        nomes,
        [
            "contem",
            "nao_contem",
            "igual",
            "diferente",
            "maior",
            "maior_igual",
            "menor",
            "menor_igual",
            "comeca",
            "termina",
            "nulo",
            "nao_nulo",
            "vazio",
            "nao_vazio"
        ]
    );
}

#[test]
fn filtros_vazio_e_nulo_em_texto() {
    let ex = exemplo("vazio-nulo");
    let f = |op| pag(&ex.banco, TAB_ESPACO, |x| x.filtros = vec![filtro(COL_CITADA, op, Entrada::Nulo)]).total.unwrap();
    assert_eq!((f(Operador::Vazio), f(Operador::NaoVazio), f(Operador::Nulo), f(Operador::NaoNulo)), (2, 2, 1, 3));
}

#[test]
fn filtros_combinados_com_and_e_busca() {
    let ex = exemplo("combinados");
    let filtros =
        vec![filtro("tipo", Operador::Igual, tx("login")), filtro("valor", Operador::Maior, Entrada::Inteiro(0))];
    let esperado = contar_cru(&ex.caminho, "tipo = 'login' AND valor > 0");
    assert_eq!(pag(&ex.banco, "eventos", |x| x.filtros = filtros.clone()).total, Some(esperado));
    let com_busca = pag(&ex.banco, "eventos", |x| {
        x.filtros = filtros.clone();
        x.busca = Some("09-02".into());
    });
    assert_eq!(
        com_busca.total,
        Some(contar_cru(&ex.caminho, "tipo = 'login' AND valor > 0 AND momento LIKE '%09-02%'"))
    );
}

#[test]
fn filtro_coage_pela_afinidade() {
    let ex = exemplo("coage");
    // "3" em coluna INTEGER vira número; igual a número acha a linha
    let p = pag(&ex.banco, TAB_ESPACO, |x| x.filtros = vec![filtro("número", Operador::Igual, tx("3"))]);
    assert_eq!(p.total, Some(1));
    let p = pag(&ex.banco, "clientes", |x| x.filtros = vec![filtro("id", Operador::MaiorIgual, Entrada::Inteiro(59))]);
    assert_eq!(p.total, Some(2));
}

#[test]
fn filtro_invalido() {
    let ex = exemplo("filtro-invalido");
    let e =
        erro(pag_r(&ex.banco, "clientes", |x| x.filtros = vec![filtro("xx", Operador::Igual, Entrada::Inteiro(1))]));
    assert_eq!(e.status, 400);
    assert_eq!(e.mensagem, "A coluna do filtro não existe em clientes: xx.");
    assert_eq!(erro(Operador::de_texto("parecido")).status, 400);
}

#[test]
fn linhas_produtos_sem_rowid_blob_inteiro_grande_e_gerada() {
    let ex = exemplo("produtos");
    let p = pag(&ex.banco, "produtos", |_| {});
    assert_eq!(p.colunas, ["codigo", "descricao", "preco", "estoque", "foto", "serie", "preco_centavos"]);
    assert_eq!(p.total, Some(N_PRODUTOS));
    let por_codigo: std::collections::HashMap<String, Vec<Celula>> = p
        .linhas
        .iter()
        .map(|l| match &l.chave {
            Some(Chave::Pk(pk)) => (pk[0].1.texto_exibicao(), l.valores.clone()),
            outra => panic!("{outra:?}"),
        })
        .collect();
    let esperados: std::collections::HashSet<String> = (1..=10).map(|n| format!("P{n:03}")).collect();
    assert_eq!(por_codigo.keys().cloned().collect::<std::collections::HashSet<_>>(), esperados);
    assert_eq!(p.linhas[0].chave, Some(pk(&[("codigo", vt("P001"))])));
    assert_eq!(por_codigo["P004"][5], ci(9007199254740993 + 4));
    assert_eq!(por_codigo["P001"][5], ci(1));
    match &por_codigo["P003"][4] {
        Celula::Blob { tamanho, inicio } => assert!(*tamanho == 64 + 3 * 16 && inicio.len() == 24),
        outra => panic!("{outra:?}"),
    }
    assert_eq!(por_codigo["P001"][4], Celula::Nulo);
    let preco = match por_codigo["P001"][2] {
        Celula::Real(r) => r,
        Celula::Inteiro(i) => i as f64,
        ref outra => panic!("{outra:?}"),
    };
    assert_eq!(por_codigo["P001"][6], ci((preco * 100.0) as i64));
}

#[test]
fn linhas_visao_sem_chave() {
    let ex = exemplo("visao");
    let p = pag(&ex.banco, "v_pedidos_por_cliente", |x| {
        (x.por_pagina, x.ordem, x.direcao) = (10, Some("pedidos".into()), Direcao::Desc)
    });
    assert_eq!(p.colunas, ["id", "nome", "pedidos", "ultimo"]);
    assert_eq!(p.total, Some(N_CLIENTES));
    assert!(p.linhas.iter().all(|l| l.chave.is_none()));
}

#[test]
fn linhas_fts5() {
    let ex = exemplo("fts5");
    let p = pag(&ex.banco, "notas", |x| x.busca = Some("trimestre".into()));
    assert_eq!(p.colunas, ["titulo", "corpo"]);
    assert_eq!(p.total, Some(1));
    assert_eq!(p.linhas[0].chave, Some(Chave::Rowid(1)));
}

#[test]
fn texto_longo_truncado_na_grade_e_inteiro_em_linha() {
    let ex = exemplo("texto-longo");
    let b = &ex.banco;
    let p = pag(b, "pedidos", |x| x.filtros = vec![filtro("id", Operador::Igual, Entrada::Inteiro(10))]);
    let obs = &p.linhas[0].valores[4];
    assert_eq!(*obs, Celula::TextoCortado { inicio: format!("Texto longo {}", "x".repeat(288)), tamanho: 712 });
    let completa = b.linha("pedidos", p.linhas[0].chave.as_ref().unwrap()).unwrap();
    assert_eq!(completa.chave, Some(Chave::Rowid(10)));
    assert_eq!(completa.valor("observacao"), Some(&ct(&format!("Texto longo {}", "x".repeat(700)))));
    assert_eq!(completa.valor("observacao").unwrap().texto().chars().count(), 712);
    assert_eq!(b.linha("pedidos", &Chave::Rowid(5)).unwrap().valor("observacao"), Some(&ct("Entregar na portaria")));
}

#[test]
fn linha_aceita_chave_em_json_e_some_404() {
    let ex = exemplo("linha-404");
    let b = &ex.banco;
    assert_eq!(b.linha("clientes", &Chave::Rowid(2)).unwrap().valor("id"), Some(&ci(2)));
    assert_eq!(erro(b.linha("clientes", &Chave::Rowid(9999))).status, 404);
    assert_eq!(erro(b.linha("clientes", &pk(&[("id", Valor::Inteiro(1))]))).status, 400);
    assert_eq!(erro(b.linha("produtos", &pk(&[]))).status, 400);
    assert_eq!(erro(b.linha("v_pedidos_por_cliente", &Chave::Rowid(1))).status, 400);
}

#[test]
fn ler_blob() {
    let ex = exemplo("ler-blob");
    let b = &ex.banco;
    let chave = pk(&[("codigo", vt("P003"))]);
    let dados = b.ler_blob("produtos", &chave, "foto").unwrap().unwrap();
    assert_eq!(dados.len(), 64 + 3 * 16);
    assert_eq!(b.ler_blob("produtos", &pk(&[("codigo", vt("P001"))]), "foto").unwrap(), None);
    assert_eq!(b.ler_blob("produtos", &chave, "descricao").unwrap(), Some("Grampeador".as_bytes().to_vec()));
    assert_eq!(erro(b.ler_blob("produtos", &chave, "nao_existe")).status, 404);
}

#[test]
fn busca_e_filtros_ignoram_maiusculas_acentuadas() {
    let tmp = Pasta::nova("acentuadas");
    let p = banco_de(
        &tmp,
        "CREATE TABLE pessoas (id INTEGER PRIMARY KEY, nome TEXT, cidade TEXT);\
         INSERT INTO pessoas VALUES (1, 'JOÃO DA SILVA', 'SÃO PAULO'), (2, 'joão souza', 'Recife'),\
         (3, 'Élida Ramos', 'Brasília'), (4, 'élida costa', 'BRASÍLIA'), (5, 'Ana', 'Natal'),\
         (6, CAST(x'4a6fe36f' AS TEXT), 'Latin-1');",
        "caso.db",
    );
    let b = Banco::abrir(&p, false, false).unwrap();
    let busca = |t: &str| ids(&pag(&b, "pessoas", |x| x.busca = Some(t.into())));
    let filtrar = |op, t: &str| ids(&pag(&b, "pessoas", |x| x.filtros = vec![filtro("nome", op, tx(t))]));
    assert_eq!(busca("joão"), [1, 2]);
    assert_eq!(busca("JOÃO"), [1, 2]);
    assert_eq!(busca("são paulo"), [1]);
    assert_eq!(busca("ÉLIDA"), [3, 4]);
    assert_eq!(busca("brasília"), [3, 4]);
    assert_eq!(busca("ana"), [5]); // só ASCII: caminho de sempre
    assert_eq!(filtrar(Operador::Contem, "JOÃO"), [1, 2]);
    assert_eq!(filtrar(Operador::Comeca, "élida"), [3, 4]);
    assert_eq!(filtrar(Operador::NaoContem, "ÉLIDA"), [1, 2, 5, 6]);
    assert_eq!(busca("ã"), [1, 2]); // o texto Latin-1 inválido não derruba a busca
}

#[test]
fn filtros_que_nao_sao_lista_e_pagina_absurda() {
    let ex = exemplo("pagina-absurda");
    let r = pag(&ex.banco, "clientes", |x| x.pagina = 10_000_000_000_000_000_000);
    assert!(r.linhas.is_empty() && r.total == Some(N_CLIENTES));
}

#[test]
fn ordenar_pagina_funda_pela_subconsulta_estreita() {
    let ex = exemplo("subconsulta");
    let b = &ex.banco;
    let todas: Vec<Celula> =
        pag(b, "eventos", |x| (x.ordem, x.direcao, x.por_pagina) = (Some("valor".into()), Direcao::Desc, 1000))
            .linhas
            .into_iter()
            .map(|l| l.valores[0].clone())
            .collect();
    let esperado: Vec<Celula> =
        cru(&ex.caminho, "SELECT momento FROM eventos ORDER BY valor DESC, rowid ASC LIMIT 1000", &[])
            .into_iter()
            .map(|l| Celula::de_valor(&l[0], Some(comum::LIMITE_TEXTO)))
            .collect();
    assert_eq!(todas, esperado);
    let pagina = pag(b, "eventos", |x| (x.ordem, x.pagina, x.por_pagina) = (Some("tipo".into()), 37, 100));
    let esperado: Vec<i64> =
        cru(&ex.caminho, "SELECT rowid FROM eventos ORDER BY tipo, rowid LIMIT 100 OFFSET 3600", &[])
            .into_iter()
            .map(|l| match l[0] {
                Valor::Inteiro(i) => i,
                _ => panic!(),
            })
            .collect();
    assert_eq!(pagina.linhas.iter().map(|l| rowid(&l.chave)).collect::<Vec<_>>(), esperado);
    assert_eq!(pagina.total, Some(N_EVENTOS));
    let p = pag(b, "itens_pedido", |x| (x.ordem, x.pagina, x.por_pagina) = (Some("quantidade".into()), 2, 50));
    assert_eq!(p.linhas.len(), 50);
}

#[test]
fn ordenar_e_paginar_tabela_grande_e_rapido() {
    let _exclusivo = exclusivo();
    let tmp = Pasta::nova("grande-rapido");
    let p = tmp.join("grande.db");
    {
        let conn = rusqlite::Connection::open(&p).unwrap();
        let colunas = (0..20).map(|i| format!("c{i:02} TEXT")).collect::<Vec<_>>().join(", ");
        conn.execute_batch(&format!("CREATE TABLE g (id INTEGER PRIMARY KEY, {colunas})")).unwrap();
        let texto = format!("'{}'", "x".repeat(60));
        let resto = vec![texto.as_str(); 19].join(", ");
        conn.execute_batch(&format!(
            "WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM s WHERE i < 199999) \
             INSERT INTO g SELECT NULL, printf('%06d', (i * 7919) % 200000), {resto} FROM s"
        ))
        .unwrap();
    }
    let b = Banco::abrir(&p, false, false).unwrap();
    let inicio = Instant::now();
    let r =
        pag(&b, "g", |x| (x.ordem, x.direcao, x.pagina, x.por_pagina) = (Some("c00".into()), Direcao::Desc, 1500, 100));
    let decorrido = inicio.elapsed();
    assert!(r.linhas.len() == 100 && r.total == Some(200_000));
    assert!(decorrido.as_secs_f64() < 3.0, "{decorrido:?}");
}

#[test]
fn busca_com_acento_e_caracteres_do_glob() {
    let tmp = Pasta::nova("glob");
    let p = banco_de(
        &tmp,
        "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);\
         INSERT INTO t VALUES (1, 'AÇÃO [urgente]*'), (2, 'ação?'), (3, 'acao');",
        "caso.db",
    );
    let b = Banco::abrir(&p, false, false).unwrap();
    let busca = |t: &str| ids(&pag(&b, "t", |x| x.busca = Some(t.into())));
    assert_eq!(busca("ação"), [1, 2]);
    assert_eq!(busca("ÇÃO [URGENTE]*"), [1]);
    assert_eq!(busca("ação?"), [2]);
    assert_eq!(ids(&pag(&b, "t", |x| x.filtros = vec![filtro("v", Operador::Termina, tx("ção?"))])), [2]);
    assert_eq!(ids(&pag(&b, "t", |x| x.filtros = vec![filtro("v", Operador::Comeca, tx("AÇ"))])), [1, 2]);
    assert_eq!(
        ids(&pag(&b, "t", |x| (x.busca, x.ordem, x.direcao) = (Some("ação".into()), Some("v".into()), Direcao::Desc))),
        [2, 1]
    );
}
