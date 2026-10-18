//! Leitura em fluxo para a exportação (iteradores, contagem e parar()).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use comum::{Chave, Resultado, Valor};

use super::*;
use crate::{Banco, FiltroExportacao, Operador, Parar, ResultadoComando};

const MSG_SO_SELECT: &str = "Só dá para exportar o resultado de uma consulta SELECT.";

fn ler_tabela(b: &Banco, nome: &str, filtro: &FiltroExportacao) -> (Vec<String>, Vec<Vec<Valor>>) {
    b.iterar_tabela(nome, filtro, None, |colunas, linhas| {
        Ok((colunas.to_vec(), linhas.collect::<Resultado<Vec<_>>>()?))
    })
    .unwrap()
}

fn ler_consulta(b: &Banco, sql: &str) -> Resultado<(Vec<String>, Vec<Vec<Valor>>)> {
    b.iterar_consulta(sql, None, |colunas, linhas| Ok((colunas.to_vec(), linhas.collect::<Resultado<Vec<_>>>()?)))
}

fn com_chaves(chaves: Vec<Chave>) -> FiltroExportacao {
    FiltroExportacao { chaves: Some(chaves), ..Default::default() }
}

#[test]
fn iterar_tabela_valores_crus() {
    let ex = exemplo("iterar-crus");
    let (colunas, lista) = ler_tabela(&ex.banco, "produtos", &FiltroExportacao::default());
    assert_eq!(colunas, ["codigo", "descricao", "preco", "estoque", "foto", "serie", "preco_centavos"]);
    assert_eq!(lista.len() as u64, N_PRODUTOS);
    let por_codigo: std::collections::HashMap<String, &Vec<Valor>> =
        lista.iter().map(|l| (l[0].texto_exibicao(), l)).collect();
    assert!(matches!(por_codigo["P003"][4], Valor::Blob(_)));
    assert_eq!(por_codigo["P004"][5], Valor::Inteiro(9007199254740997));
}

#[test]
fn iterar_tabela_com_filtro_ordem_e_chaves() {
    let ex = exemplo("iterar-filtro");
    let b = &ex.banco;
    let filtros = vec![filtro("tipo", Operador::Igual, tx("erro"))];
    let filtro_ordem = FiltroExportacao {
        filtros: filtros.clone(),
        ordem: Some("valor".into()),
        direcao: crate::Direcao::Desc,
        ..Default::default()
    };
    let (colunas, lista) = ler_tabela(b, "eventos", &filtro_ordem);
    assert_eq!(colunas, ["momento", "tipo", "valor", "dados"]);
    assert!(lista.len() as u64 == N_EVENTOS / 4 && lista.iter().all(|r| r[1] == vt("erro")));
    let valores: Vec<f64> = lista
        .iter()
        .filter_map(|r| match r[2] {
            Valor::Real(v) => Some(v),
            _ => None,
        })
        .collect();
    let mut ordenados = valores.clone();
    ordenados.sort_by(|a, b| b.total_cmp(a));
    assert_eq!(valores, ordenados);

    let (_, linhas) = ler_tabela(b, "eventos", &com_chaves([5, 2, 9].map(Chave::Rowid).to_vec()));
    assert_eq!(
        linhas.iter().map(|r| r[3].clone()).collect::<Vec<_>>(),
        [
            vt("{\"origem\": \"teste\", \"n\": 1}"),
            vt("{\"origem\": \"teste\", \"n\": 4}"),
            vt("{\"origem\": \"teste\", \"n\": 8}")
        ]
    );
    let (_, linhas) =
        ler_tabela(b, "produtos", &com_chaves(vec![pk(&[("codigo", vt("P002"))]), pk(&[("codigo", vt("P010"))])]));
    assert_eq!(linhas.iter().map(|r| r[0].clone()).collect::<Vec<_>>(), [vt("P002"), vt("P010")]);
    let (_, linhas) = ler_tabela(b, "eventos", &com_chaves(vec![]));
    assert!(linhas.is_empty());
    let (_, linhas) = ler_tabela(
        b,
        "eventos",
        &FiltroExportacao { filtros, chaves: Some(vec![Chave::Rowid(4), Chave::Rowid(5)]), ..Default::default() },
    );
    assert_eq!(linhas.len(), 1); // rowid 4 é i=3 → "erro"; rowid 5 é "login"
}

#[test]
fn iterar_nao_trava_a_leitura_e_solta_o_arquivo_no_fim() {
    let _exclusivo = exclusivo();
    let ex = exemplo("iterar-solta");
    let b = &ex.banco;
    b.iterar_tabela("eventos", &FiltroExportacao::default(), None, |_, linhas| {
        let primeira = linhas.next().unwrap().unwrap();
        assert_eq!(primeira[1], vt("login"));
        // a interface continua lendo pela conexão principal enquanto a exportação anda
        assert_eq!(pag(b, "clientes", |x| x.por_pagina = 5).total, Some(N_CLIENTES));
        Ok(())
    })
    .unwrap();
    // terminado o consumo, a conexão de leitura some e a gravação não espera nada
    let inicio = Instant::now();
    b.atualizar("eventos", &Chave::Rowid(4000), &vals(&[("tipo", tx("depois da exportação"))])).unwrap();
    assert!(inicio.elapsed() < Duration::from_secs(2));

    b.iterar_tabela("clientes", &FiltroExportacao::default(), None, |_, _| Ok(())).unwrap(); // sem iterar
    let inicio = Instant::now();
    b.atualizar("eventos", &Chave::Rowid(4001), &vals(&[("tipo", tx("depois do descarte"))])).unwrap();
    assert!(inicio.elapsed() < Duration::from_secs(2));

    let (_, todas) = ler_tabela(b, "eventos", &FiltroExportacao::default());
    assert_eq!(todas.len() as u64, N_EVENTOS);
    b.atualizar("eventos", &Chave::Rowid(4002), &vals(&[("tipo", tx("depois de ler tudo"))])).unwrap();
}

#[test]
fn iterar_consulta() {
    let ex = exemplo("iterar-consulta");
    let (colunas, linhas) = ler_consulta(&ex.banco, "SELECT id, nome FROM clientes WHERE id <= 3 ORDER BY id").unwrap();
    assert_eq!(colunas, ["id", "nome"]);
    assert_eq!(linhas.iter().map(|r| r[0].clone()).collect::<Vec<_>>(), [1, 2, 3].map(Valor::Inteiro));
    let (colunas, linhas) =
        ler_consulta(&ex.banco, "-- com comentário\nWITH x AS (SELECT 1 AS um) SELECT * FROM x;").unwrap();
    assert_eq!(colunas, ["um"]);
    assert_eq!(linhas, vec![vec![Valor::Inteiro(1)]]);
}

#[test]
fn iterar_consulta_recusa_o_que_nao_e_leitura() {
    let ex = exemplo("iterar-recusa");
    let b = &ex.banco;
    assert_eq!(erro(ler_consulta(b, "DELETE FROM clientes")).mensagem, MSG_SO_SELECT);
    assert_eq!(erro(ler_consulta(b, "UPDATE clientes SET nome = 'x' RETURNING id")).mensagem, MSG_SO_SELECT);
    assert_eq!(erro(ler_consulta(b, "SELECT 1; SELECT 2")).status, 400);
    assert_eq!(erro(ler_consulta(b, "SELECT * FROM nao_existe")).status, 400);
    assert_eq!(erro(ler_consulta(b, "  -- nada")).mensagem, "Nenhuma consulta para exportar.");
    assert_eq!(contar(b, "clientes"), N_CLIENTES);
}

#[test]
fn conexao_leitura_e_nova_e_so_leitura() {
    let ex = exemplo("conexao-leitura");
    let conn = ex.banco.conexao_leitura().unwrap();
    assert!(conn.is_readonly("main").unwrap());
    assert!(conn.execute("DELETE FROM clientes", []).is_err());
    // a principal continua sem transação e gravando
    ex.banco
        .com_conexao(|principal| {
            assert!(!principal.is_readonly("main").unwrap());
            Ok(())
        })
        .unwrap();
    drop(conn);
    assert_eq!(contar(&ex.banco, "clientes"), N_CLIENTES);
}

#[test]
fn exportar_com_transacao_aberta_le_o_que_a_tela_mostra() {
    let ex = exemplo("exportar-transacao");
    let b = &ex.banco;
    b.consultar(
        "BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;\
         INSERT INTO clientes (id, nome, email) VALUES (999, 'Linha Nova', 'nova@example.test');",
        1000,
    )
    .unwrap();
    let (colunas, todas) = ler_tabela(b, "clientes", &FiltroExportacao::default());
    assert_eq!(todas.len() as u64, N_CLIENTES + 1);
    let cidade = colunas.iter().position(|c| c == "cidade").unwrap();
    assert_eq!(todas[0][cidade], vt("Natal"));
    let (_, so_a_nova) = ler_tabela(b, "clientes", &com_chaves(vec![Chave::Rowid(999)]));
    assert_eq!(so_a_nova.iter().map(|r| r[0].clone()).collect::<Vec<_>>(), [Valor::Inteiro(999)]);
    let (_, consulta) = ler_consulta(b, "SELECT cidade FROM clientes WHERE id = 1").unwrap();
    assert_eq!(consulta, vec![vec![vt("Natal")]]);
    assert_eq!(erro(ler_consulta(b, "DELETE FROM clientes WHERE id = 999")).status, 400);
    // a conexão volta a gravar depois da exportação
    b.consultar("ROLLBACK", 1000).unwrap();
    b.atualizar("clientes", &Chave::Rowid(2), &vals(&[("cidade", tx("Recife"))])).unwrap();
    assert!(!b.transacao_aberta());
}

#[test]
fn exportar_consulta_com_tabela_temporaria_e_pragma_da_sessao() {
    let ex = exemplo("exportar-temporaria");
    let b = &ex.banco;
    let r = b
        .consultar(
            "CREATE TEMP TABLE resumo AS SELECT cidade, count(*) AS n FROM clientes GROUP BY cidade; SELECT * FROM resumo",
            1000,
        )
        .unwrap();
    let esperado = match r.resultados.last() {
        Some(ResultadoComando::Linhas { linhas, .. }) => linhas.len(),
        outro => panic!("{outro:?}"),
    };
    let (colunas, linhas) = ler_consulta(b, "SELECT * FROM resumo").unwrap();
    assert_eq!(colunas, ["cidade", "n"]);
    assert_eq!(linhas.len(), esperado);
    let (_, fk) = ler_consulta(b, "PRAGMA foreign_keys").unwrap();
    assert_eq!(fk, vec![vec![Valor::Inteiro(1)]]);
    b.atualizar("clientes", &Chave::Rowid(1), &vals(&[("cidade", tx("Natal"))])).unwrap(); // a principal ficou livre
}

#[test]
fn edicao_durante_exportacao_explica_que_a_trava_e_da_exportacao() {
    let ex = exemplo("edicao-exportando");
    let b = &ex.banco;
    b.ajustes.espera_gravacao_ms.store(200, Ordering::Relaxed);
    b.com_conexao(|conn| {
        conn.busy_timeout(Duration::from_millis(200)).unwrap();
        Ok(())
    })
    .unwrap();
    // conexão de leitura com o SHARED do arquivo
    let e = b
        .iterar_tabela("eventos", &FiltroExportacao::default(), None, |_, linhas| {
            linhas.next().unwrap().unwrap();
            Ok(erro(b.atualizar("clientes", &Chave::Rowid(1), &vals(&[("cidade", tx("Natal"))]))))
        })
        .unwrap();
    assert!(e.status == 409 && e.mensagem.contains("exportação em andamento"), "{e:?}");
    assert_eq!(b.exportacoes.load(Ordering::SeqCst), 0);
    b.atualizar("clientes", &Chave::Rowid(1), &vals(&[("cidade", tx("Natal"))])).unwrap();
}

#[test]
fn contar_para_exportar() {
    let ex = exemplo("contar-exportar");
    let b = &ex.banco;
    let filtros = vec![filtro("tipo", Operador::Igual, tx("login"))];
    assert_eq!(b.contar_para_exportar("eventos", &FiltroExportacao::default(), None, None).unwrap(), Some(N_EVENTOS));
    let com_busca = FiltroExportacao { busca: Some("09-0".into()), filtros: filtros.clone(), ..Default::default() };
    let esperado = pag(b, "eventos", |x| {
        x.busca = Some("09-0".into());
        x.filtros = filtros.clone();
    })
    .total;
    assert_eq!(b.contar_para_exportar("eventos", &com_busca, None, None).unwrap(), esperado);
    let chaves = com_chaves([3, 7, 999].map(Chave::Rowid).to_vec());
    assert_eq!(b.contar_para_exportar("clientes", &chaves, None, None).unwrap(), Some(2));
    let sempre: Arc<Parar> = Arc::new(|| true);
    let com_x = FiltroExportacao { busca: Some("x".into()), ..Default::default() };
    assert_eq!(b.contar_para_exportar("eventos", &com_x, Some(sempre), None).unwrap(), None);
    let ruim = FiltroExportacao { filtros: vec![filtro("nao_existe", Operador::Igual, tx("1"))], ..Default::default() };
    assert_eq!(erro(b.contar_para_exportar("eventos", &ruim, None, None)).status, 400);
    assert_eq!(b.exportacoes.load(Ordering::SeqCst), 0);
}

#[test]
fn contar_e_iterar_param_quando_mandam_parar() {
    let _exclusivo = exclusivo();
    let tmp = Pasta::nova("parar");
    let p = tabela_grande(&tmp.join("lenta.db"), 150_000, "grande", 8);
    let b = Banco::abrir(&p, false, false).unwrap();
    let termo = FiltroExportacao { busca: Some("98765432101".into()), ..Default::default() };
    let inicio = Instant::now();
    assert_eq!(b.contar_para_exportar("grande", &termo, None, None).unwrap(), Some(0));
    let completa = inicio.elapsed();
    assert_eq!(b.contar_para_exportar("grande", &termo, None, Some(completa / 10)).unwrap(), None);

    let pare = Arc::new(AtomicBool::new(false));
    let disparo = {
        let pare = Arc::clone(&pare);
        thread::spawn(move || {
            thread::sleep(completa / 10);
            pare.store(true, Ordering::SeqCst);
        })
    };
    let vigia = Arc::clone(&pare);
    let parar: Arc<Parar> = Arc::new(move || vigia.load(Ordering::SeqCst));
    let inicio = Instant::now();
    assert_eq!(b.contar_para_exportar("grande", &termo, Some(parar), None).unwrap(), None);
    assert!(inicio.elapsed() < completa / 2, "{:?} {completa:?}", inicio.elapsed());
    disparo.join().unwrap();

    let pare = Arc::new(AtomicBool::new(false));
    let vigia = Arc::clone(&pare);
    let parar: Arc<Parar> = Arc::new(move || vigia.load(Ordering::SeqCst));
    let ordem = FiltroExportacao { ordem: Some("t3".into()), ..Default::default() };
    let r = b.iterar_tabela("grande", &ordem, Some(parar), |_, linhas| {
        linhas.next().unwrap()?;
        pare.store(true, Ordering::SeqCst);
        for l in linhas {
            l?;
        }
        Ok(())
    });
    let e = erro(r);
    assert!(e.mensagem.starts_with("Não foi possível ler os dados para exportar."), "{e:?}");
    assert_eq!(e.detalhe.as_deref(), Some("interrupted"));
    assert_eq!(b.exportacoes.load(Ordering::SeqCst), 0);
    // a conexão principal (transação aberta) volta sem o progress handler e gravando
    b.consultar("BEGIN", 1000).unwrap();
    let sempre: Arc<Parar> = Arc::new(|| true);
    let r = b.iterar_tabela("grande", &FiltroExportacao::default(), Some(sempre), |_, linhas| {
        for l in linhas {
            l?;
        }
        Ok(())
    });
    assert!(r.is_err());
    b.consultar("COMMIT", 1000).unwrap();
    assert!(b.inserir("grande", &vals(&[("t0", tx("depois"))])).unwrap().chave.is_some());
}
