//! Tabelas grandes (partes A e B): página sem contagem, contagem com filtros, ordem
//! por coluna indexada, e a busca rápida conferida contra o LIKE/GLOB de antes.

use std::path::Path;
use std::time::Instant;

use comum::{Entrada, Valor};
use rusqlite::{Connection, params_from_iter};

use super::*;
use crate::busca::{CARACTERES_DE_NUMERO, escapar_like, glob_sem_caixa, pode_estar_num_numero, trecho_sem_caixa};
use crate::meta::{construir_meta, indice_comeca_por, pagina_sql};
use crate::{Direcao, Operador};

// ---------------------------------------------------------------------------------------------------
// A) Página sem contagem
// ---------------------------------------------------------------------------------------------------

#[test]
fn pagina_sem_contar_nao_conta_e_diz_se_ha_mais() {
    let ex = exemplo("sem-contar");
    let b = &ex.banco;
    let base = |p: &mut PedidoPagina| {
        (p.pagina, p.por_pagina, p.ordem, p.direcao) = (3, 100, Some("valor".into()), Direcao::Desc)
    };
    let com = pag(b, "eventos", base);
    let sem = pag(b, "eventos", |p| {
        base(p);
        p.contar = false;
    });
    assert!(sem.total.is_none() && sem.paginas.is_none());
    assert!(sem.linhas == com.linhas && sem.tem_mais && com.tem_mais);
    assert!(sem.tempo_ms >= 0.0 && com.tempo_ms >= 0.0);
    let ultima = pag(b, "eventos", |p| (p.pagina, p.por_pagina, p.contar) = (com.paginas.unwrap(), 100, false));
    assert!(!ultima.tem_mais && ultima.linhas.len() == 100); // 5.000 linhas: a última página é cheia
    let depois = pag(b, "eventos", |p| (p.pagina, p.por_pagina, p.contar) = (com.paginas.unwrap() + 1, 100, false));
    assert!(depois.linhas.is_empty() && !depois.tem_mais);
}

#[test]
fn pagina_sem_contar_traz_as_mesmas_linhas() {
    let ex = exemplo("mesmas-linhas");
    let b = &ex.banco;
    let casos: Vec<fn(&mut PedidoPagina)> = vec![
        |_| {},
        |p| p.ordem = Some("tipo".into()),
        |p| p.busca = Some("09-0".into()),
        |p| (p.ordem, p.busca) = (Some("valor".into()), Some("login".into())),
        |p| p.filtros = vec![filtro("valor", Operador::Maior, Entrada::Inteiro(0))],
    ];
    for kw in casos {
        for pagina in [1, 2, 7] {
            let com = pag(b, "eventos", |p| {
                (p.pagina, p.por_pagina) = (pagina, 50);
                kw(p);
            });
            let sem = pag(b, "eventos", |p| {
                (p.pagina, p.por_pagina, p.contar) = (pagina, 50, false);
                kw(p);
            });
            assert_eq!(sem.linhas, com.linhas);
            assert_eq!(sem.tem_mais, com.tem_mais);
            assert_eq!(com.tem_mais, pagina * 50 < com.total.unwrap());
        }
    }
}

#[test]
fn tem_mais_com_contagem() {
    let ex = exemplo("tem-mais");
    let b = &ex.banco;
    assert!(!pag(b, "clientes", |p| p.por_pagina = 1000).tem_mais);
    let r = pag(b, "clientes", |p| (p.por_pagina, p.pagina) = (10, 6)); // 60 clientes: a sexta página é a última
    assert!(!r.tem_mais && r.total == Some(60));
    assert!(pag(b, "clientes", |p| (p.por_pagina, p.pagina) = (10, 5)).tem_mais);
    for tabela in ["produtos", "itens_pedido", "v_pedidos_por_cliente", "notas"] {
        // PK, sem rowid, visão, FTS5
        let com = pag(b, tabela, |p| (p.por_pagina, p.pagina) = (3, 2));
        let sem = pag(b, tabela, |p| (p.por_pagina, p.pagina, p.contar) = (3, 2, false));
        assert!(sem.linhas == com.linhas && sem.tem_mais == com.tem_mais, "{tabela}");
    }
}

#[test]
fn pagina_sem_contar_para_cedo_numa_tabela_grande() {
    let _exclusivo = exclusivo();
    let tmp = Pasta::nova("para-cedo");
    let p = tabela_grande(&tmp.join("g.db"), 200_000, "grande", 2);
    let b = Banco::abrir(&p, false, false).unwrap();
    let inicio = Instant::now();
    let com = pag(&b, "grande", |x| x.busca = Some("texto 1".into()));
    let tempo_com = inicio.elapsed();
    let inicio = Instant::now();
    let sem = pag(&b, "grande", |x| (x.busca, x.contar) = (Some("texto 1".into()), false));
    let tempo_sem = inicio.elapsed();
    assert!(sem.linhas == com.linhas && sem.tem_mais && com.total.unwrap() > 1000);
    assert!(tempo_sem < tempo_com / 3, "{tempo_sem:?} {tempo_com:?}");
}

#[test]
fn ordem_por_coluna_com_indice_conta_a_parte_e_traz_o_mesmo() {
    // Sem filtro e com um índice na coluna de ordem, a página não usa count(*) OVER (): o índice vale.
    let tmp = Pasta::nova("indice");
    let p = tabela_grande(&tmp.join("idx.db"), 3_000, "grande", 2);
    let b = Banco::abrir(&p, false, false).unwrap();
    let mut antes = Vec::new();
    for d in [Direcao::Asc, Direcao::Desc] {
        for pg in [1, 2, 75, 76] {
            antes.push((
                (d, pg),
                pag(&b, "grande", |x| (x.pagina, x.por_pagina, x.ordem, x.direcao) = (pg, 40, Some("t1".into()), d)),
            ));
        }
    }
    b.consultar("CREATE INDEX idx_parcial ON grande (t1) WHERE n0 > 500000", 1000).unwrap();
    b.com_conexao(|c| {
        assert!(!indice_comeca_por(c, "grande", "t1").unwrap()); // parcial não entrega a ordem da tabela toda
        Ok(())
    })
    .unwrap();
    b.consultar("CREATE INDEX idx_t1 ON grande (t1, n0)", 1000).unwrap();
    b.com_conexao(|c| {
        assert!(indice_comeca_por(c, "grande", "T1").unwrap() && !indice_comeca_por(c, "grande", "n0").unwrap());
        let meta = construir_meta(c, "grande", false).unwrap();
        let (sql, com_total) = pagina_sql(&meta, "", " ORDER BY \"t1\" ASC, rowid ASC", Some("t1"), true, true);
        assert!(!sql.contains("OVER") && !com_total);
        let (sql, com_total) =
            pagina_sql(&meta, " WHERE n0 > 1", " ORDER BY \"t1\" ASC, rowid ASC", Some("t1"), true, true);
        assert!(sql.contains("OVER") && com_total); // com filtro, contar na mesma passada continua melhor
        Ok(())
    })
    .unwrap();
    for ((d, pg), esperado) in antes {
        let r = pag(&b, "grande", |x| (x.pagina, x.por_pagina, x.ordem, x.direcao) = (pg, 40, Some("t1".into()), d));
        assert_eq!(r.linhas, esperado.linhas);
        assert!(r.total == esperado.total && r.total == Some(3_000));
        assert!(r.paginas == esperado.paginas && r.tem_mais == esperado.tem_mais);
    }
}

// ---------------------------------------------------------------------------------------------------
// B) Contagem com busca e filtros
// ---------------------------------------------------------------------------------------------------

#[test]
fn contagem_com_busca_e_filtros() {
    let ex = exemplo("contagem");
    let b = &ex.banco;
    let filtros =
        vec![filtro("tipo", Operador::Igual, tx("login")), filtro("valor", Operador::Maior, Entrada::Inteiro(0))];
    let esperado = pag(b, "eventos", |p| (p.filtros, p.busca) = (filtros.clone(), Some("09-0".into()))).total.unwrap();
    let c = b.contagem("eventos", Some("09-0"), &filtros, None).unwrap();
    assert!(c.linhas == esperado && 0 < esperado && esperado < 5000 && c.tempo_ms >= 0.0);
    assert_eq!(
        b.contar_linhas("eventos", None, &filtros, None).unwrap(),
        pag(b, "eventos", |p| p.filtros = filtros.clone()).total.unwrap()
    );
    assert_eq!(contar(b, "eventos"), 5000);
    assert_eq!(b.contar_linhas("eventos", Some(""), &[], None).unwrap(), 5000);
    assert_eq!(erro(b.contagem("fantasma", None, &[], None)).status, 404);
}

// ---------------------------------------------------------------------------------------------------
// Busca mais rápida, mesmo resultado
// ---------------------------------------------------------------------------------------------------

fn valores_misturados() -> Vec<Valor> {
    let t = |s: &str| Valor::Texto(s.to_string());
    let mut v = vec![Valor::Nulo];
    for s in [
        "",
        "Ana",
        "ANA PAULA",
        "São Paulo",
        "SÃO PAULO 999",
        "joão",
        "JOÃO",
        "élida",
        "100% certo",
        "a_b",
        "barra \\ invertida",
        "asterisco * e ? e [x]",
        "ab\0cd",
        "Inf",
        "-inf texto",
        "1,5",
        "12345",
        "e+20",
        "straße",
        "İstanbul",
        "ıi",
        "Kelvin K",
        "ſ longo",
    ] {
        v.push(t(s));
    }
    v.extend([0, 7, -12, 12345, 9007199254740993, i64::MIN].map(Valor::Inteiro));
    v.extend([1.5, -2.25e-07, 1e20, 12345.0, 0.1, f64::INFINITY, f64::NEG_INFINITY, 1e308, 5e-324].map(Valor::Real));
    for b in [&b"\x00binario"[..], b"Paulo", b"12345", b"", b"\xff\xfe"] {
        v.push(Valor::Blob(b.to_vec()));
    }
    v
}

const TERMOS: [&str; 47] = [
    "a",
    "PAULO",
    "paulo",
    "ão",
    "SÃO",
    "são paulo 9",
    "1",
    "12",
    "12345",
    "1.5",
    "-2",
    "e+20",
    "e-07",
    "inf",
    "INF",
    "nf",
    "Inf",
    "%",
    "_",
    "\\",
    "*",
    "?",
    "[",
    "[x]",
    "cd",
    "binario",
    "0.1",
    " ",
    "1,5",
    "ß",
    "SS",
    "İ",
    "ı",
    "I",
    "K",
    "k",
    "ſ",
    "S",
    "9007199254740993",
    "-9223",
    "5e-324",
    "e+308",
    "b\0c",
    "ab",
    "ANA P",
    "a p",
    "  ",
];

fn banco_misturado(caminho: &Path) {
    let valores = valores_misturados();
    let n_valores = valores.len();
    let conn = Connection::open(caminho).unwrap();
    conn.execute_batch("CREATE TABLE m (id INTEGER PRIMARY KEY, a, b TEXT, c INTEGER, d REAL, e BLOB)").unwrap();
    let mut n = 0;
    for (i, v) in valores.iter().enumerate() {
        for deslocamento in 0..5 {
            // cada valor aparece em todas as colunas, junto de outros
            n += 1;
            let mut linha = vec![v.clone()];
            linha.extend((1..5).map(|k| valores[(i + k * 7 + deslocamento) % n_valores].clone()));
            linha.rotate_left(deslocamento);
            let mut params = vec![Valor::Inteiro(n)];
            params.extend(linha);
            conn.execute("INSERT INTO m VALUES (?, ?, ?, ?, ?, ?)", params_from_iter(&params)).unwrap();
        }
    }
    conn.execute("INSERT INTO m (id, a) VALUES (?, CAST(x'4a6fe36f' AS TEXT))", [n + 1]).unwrap(); // UTF-8 inválido
}

fn ids_antigos(caminho: &Path, onde: &str, params: &[Valor]) -> Vec<i64> {
    cru(caminho, &format!("SELECT id FROM m WHERE {onde} ORDER BY id"), params)
        .into_iter()
        .map(|l| match l[0] {
            Valor::Inteiro(i) => i,
            _ => panic!(),
        })
        .collect()
}

/// Como a busca e o filtro de texto eram antes das peneiras (a referência do resultado certo).
fn condicao_antiga(coluna: &str, termo: &str, antes: bool, depois: bool) -> (String, Vec<Valor>) {
    let texto = format!("CAST({coluna} AS TEXT)");
    if termo.is_ascii() {
        let padrao =
            format!("{}{}{}", if antes { "%" } else { "" }, escapar_like(termo), if depois { "%" } else { "" });
        return (format!("{texto} LIKE ? ESCAPE '\\'"), vec![Valor::Texto(padrao)]);
    }
    (format!("{texto} GLOB ?"), vec![Valor::Texto(glob_sem_caixa(termo, antes, depois))])
}

fn chaves_achadas(p: &crate::Pagina) -> Vec<i64> {
    p.linhas.iter().map(|l| rowid(&l.chave)).collect()
}

#[test]
fn busca_rapida_acha_o_mesmo_que_antes() {
    let tmp = Pasta::nova("busca-rapida");
    let p = tmp.join("m.db");
    banco_misturado(&p);
    let b = Banco::abrir(&p, false, false).unwrap();
    for termo in TERMOS {
        let mut partes = Vec::new();
        let mut params = Vec::new();
        for c in ["id", "a", "b", "c", "d", "e"] {
            let (sql, ps) = condicao_antiga(&format!("\"{c}\""), termo, true, true);
            partes.push(sql);
            params.extend(ps);
        }
        let esperado = ids_antigos(&p, &partes.join(" OR "), &params);
        let achados = chaves_achadas(&pag(&b, "m", |x| (x.busca, x.por_pagina) = (Some(termo.into()), 1000)));
        assert_eq!(achados, esperado, "{termo:?}");
        assert_eq!(b.contar_linhas("m", Some(termo), &[], None).unwrap(), esperado.len() as u64, "{termo:?}");
    }
}

#[test]
fn filtros_de_texto_rapidos_acham_o_mesmo_que_antes() {
    let tmp = Pasta::nova("filtros-rapidos");
    let p = tmp.join("m.db");
    banco_misturado(&p);
    let b = Banco::abrir(&p, false, false).unwrap();
    for op in [Operador::Contem, Operador::NaoContem, Operador::Comeca, Operador::Termina] {
        for coluna in ["a", "b", "c", "d", "e"] {
            for termo in TERMOS {
                let (mut sql, params) =
                    condicao_antiga(&format!("\"{coluna}\""), termo, op != Operador::Comeca, op != Operador::Termina);
                if op == Operador::NaoContem {
                    sql = format!("(\"{coluna}\" IS NULL OR NOT {sql})");
                }
                let esperado = ids_antigos(&p, &sql, &params);
                let achados = chaves_achadas(&pag(&b, "m", |x| {
                    (x.filtros, x.por_pagina) = (vec![filtro(coluna, op, tx(termo))], 1000)
                }));
                assert_eq!(achados, esperado, "{op:?} {coluna} {termo:?}");
            }
        }
    }
}

#[test]
fn texto_de_numero_so_tem_os_caracteres_previstos() {
    let conn = Connection::open_in_memory().unwrap();
    let mut valores: Vec<Valor> = [0, -1, i64::MAX, i64::MIN].map(Valor::Inteiro).to_vec();
    valores.extend(
        [
            1.5,
            -0.0,
            1e308,
            -1e308,
            5e-324,
            1.0 / 3.0,
            1e20,
            1e-7,
            123456789.125,
            f64::INFINITY,
            f64::NEG_INFINITY,
            9.99999999999999e307,
            2.2250738585072014e-308,
        ]
        .map(Valor::Real),
    );
    for v in valores {
        let texto: String = conn.query_row("SELECT CAST(? AS TEXT)", [&v], |r| r.get(0)).unwrap();
        assert!(texto.chars().all(|c| CARACTERES_DE_NUMERO.contains(c)), "{v:?} {texto}");
    }
    assert!(pode_estar_num_numero("12345") && pode_estar_num_numero("-1.5E+20"));
    assert!(pode_estar_num_numero("INF") && pode_estar_num_numero("ınf")); // ı maiúsculo é I
    assert!(!pode_estar_num_numero("1,5") && !pode_estar_num_numero("paulo"));
    assert!(!pode_estar_num_numero("são") && !pode_estar_num_numero(" "));
    assert_eq!(trecho_sem_caixa("SÃO PAULO 999"), " 999");
    assert!(trecho_sem_caixa("12345") == "12345" && trecho_sem_caixa("7") == "7");
    assert!(trecho_sem_caixa("a1b").is_empty() && trecho_sem_caixa("12\x0034") == "12");
}

#[test]
fn tabela_de_um_milhao_de_linhas_pagina_sem_esperar() {
    let _exclusivo = exclusivo();
    // O caso que motivou o contar=false e a ordem pelo índice: 1 milhão de linhas, página 1 na hora.
    let tmp = Pasta::nova("milhao");
    let p = tabela_grande(&tmp.join("milhao.db"), 1_000_000, "grande", 2);
    let b = Banco::abrir(&p, false, false).unwrap();
    b.consultar("CREATE INDEX idx_t3 ON grande (t3)", 1000).unwrap();
    let inicio = Instant::now();
    let r = pag(&b, "grande", |x| (x.busca, x.contar) = (Some("texto 1".into()), false));
    assert!(r.tem_mais && r.linhas.len() == 100);
    assert!(inicio.elapsed().as_secs_f64() < 3.0, "{:?}", inicio.elapsed());
    let inicio = Instant::now();
    let r = pag(&b, "grande", |x| (x.ordem, x.pagina) = (Some("t3".into()), 2));
    assert!(r.total == Some(1_000_000) && r.linhas.len() == 100);
    assert!(inicio.elapsed().as_secs_f64() < 5.0, "{:?}", inicio.elapsed());
    assert_eq!(
        b.contar_para_exportar("grande", &crate::FiltroExportacao::default(), None, None).unwrap(),
        Some(1_000_000)
    );
}
