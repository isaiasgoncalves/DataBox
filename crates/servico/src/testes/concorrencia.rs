//! O serviço por `Arc` em várias threads (§3 item 6 de ARQUITETURA.md): trocas de banco no meio de
//! leituras e gravações não travam, não entram em pânico e nunca gravam no banco errado (o `banco_trocado` de
//! lá, com threads de verdade).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use banco::{PedidoPagina, ResultadoComando};
use comum::{Codigo, Entrada, Valor};
use prefs::ResultadoSql;

use super::*;
use crate::servico::ESPERA_FECHAR_ANTERIOR;

/// O id e o nome do banco aberto agora ("" sem banco).
fn atual(a: &Ambiente) -> (String, String) {
    a.servico.atual().map(|b| (b.id().to_string(), b.nome().to_string())).unwrap_or_default()
}

#[test]
fn trocas_de_banco_concorrentes_nao_travam_nem_gravam_no_banco_errado() {
    let _vez = exclusivo();
    let a = ambiente("concorrencia");
    let outro = amostras::criar_banco_exemplo(a.join("outro.db")).unwrap();
    let fim = AtomicBool::new(false);
    let (gravadas, lidas, recusadas) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0));
    let conferir_recusa = |e: comum::ErroBanco| {
        // banco trocado, fechado no meio ou nenhum aberto: sempre 409, nunca outra coisa
        assert_eq!(e.status, 409, "{e:?}");
        assert!(e.tem_codigo(Codigo::BancoTrocado) || e.codigo.is_none(), "código inesperado: {e:?}");
        recusadas.fetch_add(1, Ordering::Relaxed);
    };
    thread::scope(|s| {
        for _ in 0..3 {
            s.spawn(|| {
                while !fim.load(Ordering::Relaxed) {
                    let (id, nome) = atual(&a);
                    let valores = vec![("tipo".to_string(), Entrada::Texto(format!("marca:{nome}")))];
                    match a.servico.inserir(&id, "eventos", &valores) {
                        Ok(_) => {
                            gravadas.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => conferir_recusa(e),
                    }
                }
            });
        }
        for _ in 0..3 {
            s.spawn(|| {
                while !fim.load(Ordering::Relaxed) {
                    let (id, _) = atual(&a);
                    let pedido = PedidoPagina { por_pagina: 50, ..PedidoPagina::default() };
                    match a.servico.linhas(&id, "clientes", &pedido) {
                        Ok(p) => {
                            assert_eq!(p.total, Some(N_CLIENTES));
                            lidas.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => conferir_recusa(e),
                    }
                    if let Err(e) = a.servico.info_banco(&id) {
                        conferir_recusa(e);
                    }
                }
            });
        }
        for i in 0..24 {
            let caminho = if i % 2 == 0 { &outro } else { &a.exemplo };
            if i % 8 == 7 {
                a.servico.fechar_banco(&a.id(), false).unwrap();
                thread::sleep(std::time::Duration::from_millis(5));
            }
            a.servico.abrir_banco(caminho, false, false).unwrap();
            thread::sleep(std::time::Duration::from_millis(10));
        }
        fim.store(true, Ordering::Relaxed);
    });
    a.servico.fechar_banco(&a.id(), false).unwrap();
    assert!(gravadas.load(Ordering::Relaxed) > 0 && lidas.load(Ordering::Relaxed) > 0);
    assert!(recusadas.load(Ordering::Relaxed) > 0, "nenhuma troca aconteceu no meio de um pedido");
    // cada gravação foi para o banco que a thread viu, nunca para o outro
    for (arquivo, nome) in [(&a.exemplo, "exemplo.db"), (&outro, "outro.db")] {
        let marcas = cru(arquivo, "SELECT DISTINCT tipo FROM eventos WHERE tipo LIKE 'marca:%'");
        assert!(!marcas.is_empty(), "{nome}");
        assert_eq!(marcas, [[Valor::Texto(format!("marca:{nome}"))]], "{nome}");
    }
}

#[test]
fn troca_de_banco_nao_espera_a_operacao_longa_no_anterior() {
    // Uma operação longa na conexão principal (exportação com transação aberta, Consulta SQL, VACUUM, cópia)
    // segura a trava do banco até o fim; aqui ela é simulada segurando a trava até o teste mandar soltar.
    let _vez = exclusivo();
    let a = ambiente("troca-longa");
    let outro = amostras::criar_banco_vazio(a.join("outro.db")).unwrap();
    let id = a.id();
    a.servico.sql(&id, "BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;", 1000).unwrap();
    let anterior = a.servico.atual().unwrap();
    let (entrou, dentro) = mpsc::channel::<()>();
    let (soltar, solta) = mpsc::channel::<()>();
    thread::scope(|s| {
        let segura = Arc::clone(&anterior);
        let longa = s.spawn(move || {
            segura.com_conexao(|_| {
                entrou.send(()).unwrap();
                // prazo: se a troca voltasse a esperar o anterior fechar, o teste falha em vez de travar
                let _ = solta.recv_timeout(Duration::from_secs(10));
                Ok(())
            })
        });
        dentro.recv().unwrap();
        // sem descartar, a conferência da transação espera a operação terminar (precisa da conexão)...
        let sem_descartar = s.spawn(|| a.servico.fechar_banco(&id, false));
        thread::sleep(Duration::from_millis(150));
        assert!(!sem_descartar.is_finished());
        // ...mas não prende as trocas: a nova tentativa, descartando a transação, troca na hora e não espera o
        // anterior fechar (ele fecha quando a operação soltar a conexão)
        let inicio = Instant::now();
        let info = a.servico.abrir_banco(&outro, false, true).unwrap();
        let dt = inicio.elapsed();
        assert!(dt < ESPERA_FECHAR_ANTERIOR + Duration::from_secs(1), "abrir esperou {dt:?}");
        assert_eq!(a.id(), info.id);
        assert_eq!(a.servico.info_banco(&info.id).unwrap().nome, "outro.db");
        assert!(!longa.is_finished());
        soltar.send(()).unwrap();
        longa.join().unwrap().unwrap();
        // quem esperava recebe um 409: a transação ainda aberta, ou o banco que ele mostrava já trocado
        let e = erro_com(sem_descartar.join().unwrap(), 409);
        assert!(e.tem_codigo(Codigo::TransacaoAberta) || e.tem_codigo(Codigo::BancoTrocado), "{e:?}");
    });
    // o anterior fechou assim que a operação soltou a conexão, desfazendo a transação (a pessoa escolheu)
    esperar(|| anterior.objetos().is_err(), "o banco anterior fechar");
    assert_eq!(erro(anterior.objetos()).status, 409);
    assert_ne!(cru(&a.exemplo, "SELECT cidade FROM clientes WHERE id = 1"), [[Valor::Texto("Natal".into())]]);
    assert_eq!(a.servico.info_banco(&a.id()).unwrap().nome, "outro.db");
}

/// C7 pelo serviço: o Interromper com o `sql()` ainda esperando a vez (uma operação longa de outro tipo segura a
/// conexão): responde na hora com `NaFila`, sem cortar a operação dela, e o `sql()` sai interrompido sem executar.
#[test]
fn interromper_sql_com_o_sql_esperando_a_vez() {
    use banco::Interrupcao;
    let a = ambiente("interromper-sql-na-fila");
    let id = a.id();
    let aberto = a.servico.atual().unwrap();
    let (entrou, dentro) = mpsc::channel::<()>();
    let (soltar, solta) = mpsc::channel::<()>();
    let r = thread::scope(|s| {
        let dona = s.spawn(move || {
            aberto.com_conexao(|_| {
                entrou.send(()).unwrap();
                let _ = solta.recv_timeout(Duration::from_secs(10));
                Ok(())
            })
        });
        dentro.recv().unwrap();
        assert!(!a.servico.sql_em_andamento(&id).unwrap());
        assert_eq!(a.servico.interromper_sql_detalhado(&id).unwrap(), Interrupcao::Nenhuma);
        let espera = s.spawn(|| a.servico.sql(&id, "INSERT INTO eventos (tipo) VALUES ('nao devia rodar');", 10));
        esperar(
            || a.servico.interromper_sql_detalhado(&id).unwrap() == Interrupcao::NaFila || espera.is_finished(),
            "o sql() esperar a vez",
        );
        assert!(!espera.is_finished(), "a conexão está com a operação longa");
        assert!(!a.servico.sql_em_andamento(&id).unwrap(), "esperar a vez não é estar rodando");
        soltar.send(()).unwrap();
        dona.join().unwrap().unwrap();
        espera.join().unwrap().unwrap()
    });
    match &r.resultados[..] {
        [ResultadoComando::Erro { mensagem, .. }] => assert_eq!(mensagem, banco::MSG_INTERROMPIDA),
        outro => panic!("{outro:?}"),
    }
    // o comando não rodou; o roteiro entra no histórico como interrompido no primeiro
    let itens = a.servico.historico(&id).unwrap();
    assert_eq!(itens.len(), 1);
    assert_eq!(itens[0].resultado, Some(ResultadoSql::Interrompida { numero: 1 }));
    let r = a.servico.sql(&id, "SELECT count(*) FROM eventos WHERE tipo = 'nao devia rodar'", 10).unwrap();
    assert!(
        matches!(&r.resultados[..], [ResultadoComando::Linhas { linhas, .. }] if linhas == &vec![vec![comum::Celula::Inteiro(0)]]),
        "{r:?}"
    );
}

#[test]
fn interromper_sql_nao_espera_a_trava() {
    let a = ambiente("interromper-sql");
    let id = a.id();
    // banco trocado: 409 como os demais, sem fazer nada
    assert!(erro_com(a.servico.interromper_sql("0123456789abcdef"), 409).tem_codigo(Codigo::BancoTrocado));
    // uma operação longa (que não é consulta) segura a conexão: responde na hora, sem cortar nada
    let aberto = a.servico.atual().unwrap();
    let (entrou, dentro) = mpsc::channel::<()>();
    let (soltar, solta) = mpsc::channel::<()>();
    thread::scope(|s| {
        let longa = s.spawn(move || {
            aberto.com_conexao(|_| {
                entrou.send(()).unwrap();
                // prazo: se o Interromper esperasse a trava, o teste falha em vez de travar
                let _ = solta.recv_timeout(Duration::from_secs(10));
                Ok(())
            })
        });
        dentro.recv().unwrap();
        let inicio = Instant::now();
        assert!(!a.servico.interromper_sql(&id).unwrap());
        assert!(inicio.elapsed() < Duration::from_secs(5), "{:?}", inicio.elapsed());
        assert!(!longa.is_finished());
        soltar.send(()).unwrap();
        longa.join().unwrap().unwrap();
    });
    // uma consulta longa de verdade (minutos sem a parada): cortada, sem rodar o resto nem entrar no histórico
    let longa = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 1000000000) \
                 SELECT count(*) FROM c;";
    let r = thread::scope(|s| {
        let consulta = s.spawn(|| a.servico.sql(&id, &format!("{longa} SELECT 2;"), 1000));
        esperar(|| consulta.is_finished() || a.servico.interromper_sql(&id).unwrap(), "a consulta começar");
        consulta.join().unwrap().unwrap()
    });
    match &r.resultados[..] {
        [ResultadoComando::Erro { comando, mensagem, .. }] => {
            assert_eq!(comando, longa);
            assert_eq!(mensagem, banco::MSG_INTERROMPIDA);
        }
        outro => panic!("{outro:?}"),
    }
    let itens = a.servico.historico(&id).unwrap();
    assert_eq!(itens.len(), 1);
    assert_eq!(itens[0].resultado, Some(ResultadoSql::Interrompida { numero: 1 }), "entra como interrompida");
    // a próxima roda normalmente e entra no histórico
    let r = a.servico.sql(&id, "SELECT count(*) FROM clientes", 1000).unwrap();
    assert!(matches!(&r.resultados[..], [ResultadoComando::Linhas { .. }]), "{r:?}");
    assert!(!a.servico.interromper_sql(&id).unwrap());
    assert_eq!(a.servico.historico(&id).unwrap().len(), 2);
}
