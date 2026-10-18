//! Concorrência: arquivo travado por outro programa, info() durante uma leitura longa e a leitura substituída
//! (com threads de verdade).

use std::sync::Arc;
use std::sync::Mutex as Trava;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use comum::{Chave, Codigo, ErroBanco};

use super::*;
use crate::{Banco, FiltroExportacao, MSG_INTERROMPIDA, MSG_SUBSTITUIDA, PedidoPagina, ResultadoComando};

fn e_substituida(e: &ErroBanco) -> bool {
    e.status == 409 && e.codigo == Some(Codigo::Substituido)
}

fn com_leitura(grupo: &str, ajustar: impl FnOnce(&mut PedidoPagina)) -> PedidoPagina {
    pedido(|p| {
        p.leitura = Some(grupo.into());
        ajustar(p);
    })
}

#[test]
fn banco_travado_por_outro_programa_responde_logo() {
    let _exclusivo = exclusivo();
    // Com outro programa em BEGIN EXCLUSIVE, cada pedido esperava 5 s por comando, em fila (14 a 42 s).
    let ex = exemplo("travado");
    let b = &ex.banco;
    let outro = rusqlite::Connection::open(&ex.caminho).unwrap();
    outro.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let inicio = Instant::now();
    assert_eq!(erro(pag_r(b, "clientes", |_| {})).status, 409);
    let primeiro = inicio.elapsed();
    let inicio = Instant::now();
    assert_eq!(erro(b.info()).status, 409);
    assert_eq!(erro(b.objetos()).status, 409);
    let fila = inicio.elapsed();
    outro.execute_batch("ROLLBACK").unwrap();
    assert!(primeiro.as_secs_f64() < 3.5, "{primeiro:?}");
    assert!(fila.as_secs_f64() < 1.5, "{fila:?}");
}

#[test]
fn info_nao_espera_uma_leitura_longa() {
    let _exclusivo = exclusivo();
    // Com a trava presa por uma busca demorada, o estado esperava junto e a interface congelava.
    let ex = exemplo("info-longa");
    let b = &ex.banco;
    let primeira = b.info().unwrap();
    let (preso_tx, preso_rx) = mpsc::channel();
    let (soltar_tx, soltar_rx) = mpsc::channel::<()>();
    thread::scope(|s| {
        s.spawn(move || {
            b.com_conexao(|_| {
                preso_tx.send(()).unwrap();
                let _ = soltar_rx.recv_timeout(Duration::from_secs(10));
                Ok(())
            })
            .unwrap();
        });
        preso_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let inicio = Instant::now();
        assert_eq!(b.info().unwrap(), primeira);
        assert!(inicio.elapsed() < Duration::from_secs(2));
        soltar_tx.send(()).unwrap();
    });
}

#[test]
fn leitura_na_fila_desiste_quando_chega_outra_mais_nova() {
    let ex = exemplo("fila");
    let b = &ex.banco;
    thread::scope(|s| {
        let (velha, nova) = b
            .com_conexao(|_| {
                let velha = s.spawn(|| b.linhas("clientes", &com_leitura("grade:clientes", |_| {})));
                esperar(|| b.leituras.lock().grupos.contains_key("grade:clientes"), "a leitura velha na fila");
                let primeira = b.leituras.lock().grupos.get("grade:clientes").cloned().unwrap();
                let nova = s.spawn(|| {
                    b.linhas("clientes", &com_leitura("grade:clientes", |p| (p.pagina, p.por_pagina) = (2, 10)))
                });
                esperar(
                    || !b.leituras.lock().grupos.get("grade:clientes").is_some_and(|g| Arc::ptr_eq(g, &primeira)),
                    "a leitura nova na fila",
                );
                Ok((velha, nova))
            })
            .unwrap();
        let velha = velha.join().unwrap();
        let nova = nova.join().unwrap();
        let e = erro(velha);
        assert!(e_substituida(&e), "{e:?}");
        assert_eq!(e.mensagem, MSG_SUBSTITUIDA);
        assert_eq!(e.to_string(), MSG_SUBSTITUIDA);
        assert_eq!(nova.unwrap().pagina, 2);
    });
    let estado = b.leituras.lock();
    assert!(estado.grupos.is_empty() && estado.em_curso.is_none());
}

#[test]
fn leitura_em_execucao_e_interrompida() {
    let _exclusivo = exclusivo();
    let tmp = Pasta::nova("interrompida");
    let p = tabela_grande(&tmp.join("lenta.db"), 150_000, "grande", 8);
    let termo = "98765432101"; // cabe num número: converte todos os reais, e não acha nada
    let b = Banco::abrir(&p, false, false).unwrap();
    let inicio = Instant::now();
    pag(&b, "grande", |x| x.busca = Some(termo.into()));
    let completa = inicio.elapsed();
    thread::scope(|s| {
        let velha = s.spawn(|| {
            let r = b.linhas("grande", &com_leitura("grade:grande", |x| x.busca = Some(termo.into())));
            (r, Instant::now())
        });
        esperar(|| b.leituras.lock().em_curso.is_some(), "a leitura velha executando");
        thread::sleep(completa / 10);
        let pedido = Instant::now();
        let nova =
            b.linhas("grande", &com_leitura("grade:grande", |x| (x.por_pagina, x.contar) = (10, false))).unwrap();
        let (r, fim) = velha.join().unwrap();
        let e = erro(r);
        assert!(e_substituida(&e), "{e:?}");
        assert_eq!(nova.linhas.len(), 10);
        assert!(fim - pedido < completa / 2, "{:?} {completa:?}", fim - pedido);
    });
    assert!(b.leituras.lock().em_curso.is_none());
}

#[test]
fn sem_grupo_ou_de_outro_grupo_nada_e_substituido() {
    let ex = exemplo("outro-grupo");
    let b = &ex.banco;
    thread::scope(|s| {
        let pedidos = b
            .com_conexao(|_| {
                let pedidos = vec![
                    s.spawn(|| b.linhas("eventos", &PedidoPagina::default()).map(|_| ())),
                    s.spawn(|| b.linhas("eventos", &PedidoPagina::default()).map(|_| ())),
                    s.spawn(|| b.linhas("eventos", &com_leitura("grade:eventos", |_| {})).map(|_| ())),
                    s.spawn(|| b.contagem("eventos", None, &[], Some("contagem:eventos")).map(|_| ())),
                    s.spawn(|| b.linhas("clientes", &com_leitura("grade:clientes", |_| {})).map(|_| ())),
                ];
                esperar(|| b.leituras.lock().grupos.len() == 3, "as leituras na fila");
                Ok(pedidos)
            })
            .unwrap();
        for p in pedidos {
            p.join().unwrap().unwrap();
        }
    });
}

#[test]
fn substituida_depois_de_executar_responde_409() {
    // A mais nova chega quando a velha já terminou o comando, mas ainda não respondeu: a velha é descartada.
    let ex = exemplo("depois-de-executar");
    let b = &ex.banco;
    let leitura = b.registrar_leitura(Some("grade:x")).unwrap();
    let r = b.lendo(|conn| {
        b.interrompivel(conn, &leitura, |c| {
            c.query_row("SELECT 1", [], |_| Ok(())).unwrap();
            b.registrar_leitura(Some("grade:x"));
            Ok(())
        })
    });
    assert!(e_substituida(&erro(r)));
    assert!(b.leituras.lock().em_curso.is_none());
}

#[test]
fn gravacao_concorrente_nunca_e_interrompida() {
    let _exclusivo = exclusivo();
    // Leitores do mesmo grupo se substituindo sem parar enquanto gravações, Estrutura e Consulta SQL rodam.
    let tmp = Pasta::nova("estresse");
    let p = tabela_grande(&tmp.join("estresse.db"), 3_000, "grande", 2);
    let n_rodadas: i64 = 400;
    {
        let b = Banco::abrir(&p, false, false).unwrap();
        // Só para o teste andar: sem esperar o disco a cada gravação (o fsync levava ~7 ms por gravação no
        // Windows). Não muda nada do que está sendo conferido: quem disputa a conexão e quem é interrompido.
        b.com_conexao(|c| {
            c.execute_batch("PRAGMA synchronous = OFF").unwrap();
            Ok(())
        })
        .unwrap();
        let parar = AtomicBool::new(false);
        let (ok, substituida, interrompida) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
        let erros_leitura: Trava<Vec<String>> = Trava::new(Vec::new());
        let erros_gravacao: Trava<Vec<String>> = Trava::new(Vec::new());
        thread::scope(|s| {
            let leitor = |grupo: &'static str, ajustar: fn(&mut PedidoPagina)| {
                let (b, parar, ok, substituida, interrompida, erros) =
                    (&b, &parar, &ok, &substituida, &interrompida, &erros_leitura);
                move || {
                    while !parar.load(Ordering::SeqCst) {
                        match b.linhas("grande", &com_leitura(grupo, ajustar)) {
                            Ok(_) => {
                                ok.fetch_add(1, Ordering::SeqCst);
                            }
                            Err(e) if e_substituida(&e) => {
                                substituida.fetch_add(1, Ordering::SeqCst);
                                if e.detalhe.is_some() {
                                    // interrompida no meio do comando, não só na fila
                                    interrompida.fetch_add(1, Ordering::SeqCst);
                                }
                            }
                            Err(e) => erros.lock().unwrap().push(format!("{e:?}")),
                        }
                    }
                }
            };
            let leitores = vec![
                s.spawn(leitor("grade:grande", |p| p.busca = Some("7".into()))),
                s.spawn(leitor("grade:grande", |p| p.busca = Some("texto 9".into()))),
                s.spawn(leitor("grade:grande", |p| p.ordem = Some("t1".into()))),
                s.spawn(leitor("grade:grande", |p| (p.contar, p.busca) = (false, Some("1".into())))),
                s.spawn(leitor("contagem:grande", |_| {})),
                s.spawn(leitor("contagem:grande", |p| p.busca = Some("3".into()))),
            ];
            for i in 0..n_rodadas {
                let rodada = || -> Result<(), String> {
                    let fmt = |e: ErroBanco| format!("{e:?}");
                    b.atualizar("grande", &Chave::Rowid(i % 50 + 1), &vals(&[("t0", tx(&format!("editado {i}")))]))
                        .map_err(fmt)?;
                    b.inserir("grande", &vals(&[("t0", tx(&format!("novo {i}"))), ("n0", comum::Entrada::Inteiro(i))]))
                        .map_err(fmt)?;
                    let sem_erro = |r: crate::ResultadoConsulta| {
                        let erros: Vec<_> =
                            r.resultados.into_iter().filter(|x| matches!(x, ResultadoComando::Erro { .. })).collect();
                        if erros.is_empty() { Ok(()) } else { Err(format!("{erros:?}")) }
                    };
                    sem_erro(
                        b.consultar("UPDATE grande SET n1 = n1 + 1 WHERE id = 1; SELECT count(*) FROM grande", 1000)
                            .map_err(fmt)?,
                    )?;
                    if i % 20 == 0 {
                        sem_erro(
                            b.consultar(
                                &format!(
                                    "CREATE INDEX idx_estresse_{i} ON grande (t{}); DROP INDEX idx_estresse_{i}",
                                    i % 5
                                ),
                                1000,
                            )
                            .map_err(fmt)?,
                        )?;
                    }
                    if i % 50 == 0 {
                        let n = b
                            .iterar_tabela("grande", &FiltroExportacao::default(), None, |_, linhas| Ok(linhas.count()))
                            .map_err(fmt)?;
                        if n < 3_000 {
                            return Err(format!("exportou {n} linhas"));
                        }
                    }
                    Ok(())
                };
                if let Err(e) = rodada() {
                    erros_gravacao.lock().unwrap().push(e);
                }
            }
            parar.store(true, Ordering::SeqCst);
            for t in leitores {
                t.join().unwrap();
            }
        });
        let erros = erros_gravacao.lock().unwrap();
        assert!(erros.is_empty(), "{:?}", &erros[..erros.len().min(3)]);
        let erros = erros_leitura.lock().unwrap();
        assert!(erros.is_empty(), "{:?}", &erros[..erros.len().min(3)]);
        let (ok, substituida, interrompida) = (ok.into_inner(), substituida.into_inner(), interrompida.into_inner());
        // Quantas leituras terminam ou são interrompidas no meio depende de como o sistema reparte as threads (numa
        // máquina de 2 núcleos, ou com --test-threads=1, pode não haver nenhuma): só a disputa é certa. A interrupção
        // no meio do comando tem o teste dela, `leitura_em_execucao_e_interrompida`.
        eprintln!("ok {ok}, substituída {substituida}, interrompida {interrompida}");
        assert!(substituida > 0, "ok {ok}, substituída {substituida}, interrompida {interrompida}");
        let estado = b.leituras.lock();
        assert!(estado.em_curso.is_none() && estado.grupos.is_empty());
        drop(estado);
        assert_eq!(contar(&b, "grande"), 3_000 + n_rodadas as u64);
    }
    // n1 = (i * 104730) % 1000000 com i = 1, mais uma unidade por rodada
    assert_eq!(cru_int(&p, "SELECT n1 FROM grande WHERE id = 1"), 104_730 + n_rodadas);
    assert_eq!(cru_int(&p, "SELECT count(*) FROM grande WHERE t0 LIKE 'novo %'"), n_rodadas);
    assert_eq!(
        cru(&p, "SELECT t0 FROM grande WHERE id = 50", &[]),
        vec![vec![vt(&format!("editado {}", n_rodadas - 1))]]
    );
    assert_eq!(cru(&p, "PRAGMA integrity_check", &[]), vec![vec![vt("ok")]]);
}

#[test]
fn interromper_nao_alcanca_a_gravacao_da_grade() {
    let _exclusivo = exclusivo();
    // O Interromper apertado sem parar, de outra thread, enquanto consultas e gravações da grade se revezam na
    // conexão principal: só as consultas são cortadas, nunca a gravação que pega a conexão logo depois (nem
    // por um interrupt atrasado).
    let ex = exemplo("interromper-gravacao");
    let b = &ex.banco;
    // Só para o teste andar: sem esperar o disco a cada gravação (não muda quem é interrompido).
    b.com_conexao(|c| {
        c.execute_batch("PRAGMA synchronous = OFF").unwrap();
        Ok(())
    })
    .unwrap();
    let curta =
        "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 50000) SELECT count(*) FROM c;";
    let roteiro = format!("{curta} SELECT 1;");
    let fim = AtomicBool::new(false);
    let (mut rodadas, mut cortadas) = (0u64, 0u64);
    thread::scope(|s| {
        let apertador = s.spawn(|| {
            let mut acertos = 0u64;
            while !fim.load(Ordering::SeqCst) {
                if b.interromper_consulta() {
                    acertos += 1;
                }
                thread::yield_now();
            }
            acertos
        });
        let prazo = Instant::now() + Duration::from_secs(30);
        while rodadas < 200 || (cortadas < 20 && Instant::now() < prazo) {
            let r = b.consultar(&roteiro, 1000).unwrap();
            for c in &r.resultados {
                if let ResultadoComando::Erro { mensagem, .. } = c {
                    assert_eq!(mensagem, MSG_INTERROMPIDA, "{c:?}");
                    cortadas += 1;
                }
            }
            b.atualizar("eventos", &Chave::Rowid(1), &vals(&[("tipo", tx(&format!("rodada {rodadas}")))])).unwrap();
            b.inserir("eventos", &vals(&[("tipo", tx("inserida"))])).unwrap();
            let apagar = b.inserir("eventos", &vals(&[("tipo", tx("apagada"))])).unwrap().chave.unwrap();
            assert_eq!(b.excluir("eventos", &[apagar]).unwrap(), 1);
            rodadas += 1;
        }
        fim.store(true, Ordering::SeqCst);
        let acertos = apertador.join().unwrap();
        assert!(acertos > 0 && cortadas > 0, "acertos {acertos}, cortadas {cortadas} em {rodadas} rodadas");
    });
    assert!(b.parada_consulta.lock().is_none());
    assert_eq!(cru_int(&ex.caminho, "SELECT count(*) FROM eventos WHERE tipo = 'inserida'"), rodadas as i64);
    assert_eq!(cru_int(&ex.caminho, "SELECT count(*) FROM eventos WHERE tipo = 'apagada'"), 0);
    assert_eq!(
        cru(&ex.caminho, "SELECT tipo FROM eventos WHERE rowid = 1", &[]),
        vec![vec![vt(&format!("rodada {}", rodadas - 1))]]
    );
}
