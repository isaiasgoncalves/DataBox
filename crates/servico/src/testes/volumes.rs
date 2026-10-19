//! Tabelas grandes, a parte do serviço: páginas e contagens (A, B, C) e a exportação acompanhada (D), com
//! threads de verdade. Os testes de banco e de exportar dessas seções estão nos crates banco e exportar.
//!
//! O `Progresso` é da tela: não há registro de tarefas nem validade.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use banco::{Filtro, FiltroExportacao, Operador, PedidoPagina};
use comum::{Codigo, Entrada};
use exportar::{Formato, Opcoes};

use super::leitura::{ler_csv, ler_xlsx};
use super::*;
use crate::{Fase, MSG_EXPORTACAO_CANCELADA, MSG_TAREFA_EM_ANDAMENTO, OrigemExportacao, Progresso, ProgressoInfo};

fn tabela(nome: &str) -> OrigemExportacao {
    OrigemExportacao::Tabela { nome: nome.into(), filtro: FiltroExportacao::default() }
}

/// Exporta substituindo, com ou sem progresso.
fn exportar(
    a: &Ambiente,
    progresso: Option<&Arc<Progresso>>,
    destino: &Path,
    origem: OrigemExportacao,
    formato: Formato,
) -> comum::Resultado<crate::ResumoExportacao> {
    a.exportar_com(origem, formato, destino, Opcoes::default(), true, progresso.cloned())
}

/// Mediana e máximo de uma lista de durações, em segundos.
fn mediana_e_maximo(tempos: &mut [f64]) -> (f64, f64) {
    tempos.sort_by(f64::total_cmp);
    (tempos[tempos.len() / 2], tempos[tempos.len() - 1])
}

/// Nenhuma leitura de exportação ficou aberta: uma gravação na tabela passa na
/// hora, sem esperar a trava do arquivo nem o 409 "Há uma exportação em andamento…".
fn gravacao_passa(a: &Ambiente) {
    let inicio = Instant::now();
    let r = a.servico.inserir(&a.id(), "grande", &[("t0".into(), Entrada::Texto("depois".into()))]).unwrap();
    let dt = inicio.elapsed();
    assert!(r.resultado.chave.is_some() && !r.transacao_aberta, "{r:?}");
    assert!(dt < Duration::from_secs(2), "a gravação esperou {dt:?}");
}

// ---- A, B, C: página sem contar, contagem com filtros, leitura substituída ---------------------------

#[test]
fn api_linhas_contar() {
    let a = ambiente("linhas-contar");
    let id = a.id();
    let sem = PedidoPagina { contar: false, por_pagina: 10, ..PedidoPagina::default() };
    let r = a.servico.linhas(&id, "eventos", &sem).unwrap();
    assert!(r.total.is_none() && r.paginas.is_none() && r.tem_mais && r.linhas.len() == 10);
    assert!(r.tempo_ms >= 0.0);
    let r = a.servico.linhas(&id, "eventos", &PedidoPagina { por_pagina: 10, ..PedidoPagina::default() }).unwrap();
    assert!(r.total == Some(5000) && r.paginas == Some(500) && r.tem_mais);
    let r = a.servico.linhas(&id, "clientes", &PedidoPagina { contar: true, ..PedidoPagina::default() }).unwrap();
    assert!(r.total == Some(N_CLIENTES) && !r.tem_mais);
}

#[test]
fn api_contagem_com_busca_e_filtros() {
    let a = ambiente("contagem-filtros");
    let id = a.id();
    let filtros = vec![Filtro::texto("tipo", Operador::Igual, "login")];
    let pedido = PedidoPagina { busca: Some("09-0".into()), filtros: filtros.clone(), ..PedidoPagina::default() };
    let total = a.servico.linhas(&id, "eventos", &pedido).unwrap().total.unwrap();
    assert_eq!(a.servico.contagem(&id, "eventos", Some("09-0"), &filtros, None).unwrap().linhas, total);
    assert_eq!(a.servico.contagem(&id, "eventos", Some(""), &[], None).unwrap().linhas, N_EVENTOS);
    erro_com(a.servico.contagem(&id, "fantasma", None, &[], None), 404);
}

#[test]
fn api_leitura_substituida_pelo_grupo() {
    // o cabeçalho X-Editor-Leitura virou o `leitura` do pedido
    let _vez = exclusivo();
    let pasta = Pasta::nova("substituida");
    let caminho = tabela_grande(&pasta.join("lenta.db"), 150_000, "grande", 8);
    let a = ambiente_de(pasta, &caminho);
    let id = a.id();
    let termo = "98765432101";
    // A velha precisa estar lendo quando a nova chega. Numa máquina carregada (o runner do CI) ela pode ainda nem
    // ter começado ou já ter terminado; então tenta de novo, esperando mais a cada vez, até ver a substituição.
    let mut substituida = None;
    for tentativa in 1..=5u64 {
        let velha = thread::scope(|s| {
            let velha = s.spawn(|| {
                let pedido = PedidoPagina {
                    busca: Some(termo.into()),
                    leitura: Some("grade:grande".into()),
                    ..PedidoPagina::default()
                };
                a.servico.linhas(&id, "grande", &pedido)
            });
            thread::sleep(Duration::from_millis(50 * tentativa)); // a velha chega primeiro
            let nova = PedidoPagina {
                por_pagina: 5,
                contar: false,
                leitura: Some("grade:grande".into()),
                ..PedidoPagina::default()
            };
            let nova = a.servico.linhas(&id, "grande", &nova).unwrap();
            // a contagem é de outro grupo e ninguém a substitui
            let contagem = a.servico.contagem(&id, "grande", None, &[], Some("contagem:grande")).unwrap();
            assert_eq!(nova.linhas.len(), 5);
            assert_eq!(contagem.linhas, 150_000);
            velha.join().unwrap()
        });
        if let Err(e) = velha {
            substituida = Some(e);
            break;
        }
    }
    let e = substituida.expect("em 5 tentativas, a leitura velha nunca foi substituída pela nova");
    assert_eq!(e.status, 409);
    assert_eq!(e.mensagem, "Leitura substituída por outra mais nova.");
    assert!(e.tem_codigo(Codigo::Substituido));
}

// ---- D: exportação acompanhada -------------------------------------------------------------------------

#[test]
fn registro_de_exportacoes() {
    let p = Progresso::novo();
    assert_eq!(
        p.instantaneo(),
        ProgressoInfo {
            fase: Fase::Preparando,
            linhas: 0,
            total: None,
            tabela_atual: None,
            tabelas_feitas: 0,
            tabelas_total: 0
        }
    );
    let rodada = p.iniciar().unwrap();
    assert_eq!(erro_com(p.iniciar(), 409).mensagem, MSG_TAREFA_EM_ANDAMENTO);
    p.atualizar(|i| {
        i.fase = Fase::Gravando;
        i.linhas = 10;
        i.total = Some(20);
        i.tabelas_total = 1;
        i.tabela_atual = Some("t".into());
    });
    assert_eq!(
        p.instantaneo(),
        ProgressoInfo {
            fase: Fase::Gravando,
            linhas: 10,
            total: Some(20),
            tabela_atual: Some("t".into()),
            tabelas_feitas: 0,
            tabelas_total: 1
        }
    );
    p.cancelar();
    assert!(p.cancelado());
    p.terminar(rodada, Fase::Cancelada, |_| {});
    p.cancelar(); // já terminada: nada muda
    assert_eq!(p.instantaneo().fase, Fase::Cancelada);
    // usado de novo depois de terminado: recomeça do zero, sem o cancelamento da vez anterior
    let segunda = p.iniciar().unwrap();
    assert_ne!(segunda, rodada);
    assert!(!p.cancelado());
    assert_eq!(p.instantaneo().fase, Fase::Contando);
    // o que sobra da vez anterior não mexe na atual
    p.abandonar(rodada);
    p.terminar(rodada, Fase::Erro, |i| i.linhas = 99);
    assert_eq!(p.instantaneo(), ProgressoInfo { fase: Fase::Contando, ..ProgressoInfo::default() });
    p.cancelar();
    assert!(p.cancelado());

    let y = Progresso::novo();
    y.cancelar(); // antes de a exportação chegar
    assert_eq!(y.instantaneo().fase, Fase::Preparando);
    y.iniciar().unwrap();
    assert!(y.cancelado());

    let z = Progresso::novo();
    let vez = z.iniciar().unwrap();
    z.terminar(vez, Fase::Concluida, |_| {});
    z.cancelar(); // já terminada: não vale para a próxima
    let vez = z.iniciar().unwrap();
    assert!(!z.cancelado()); // a mesma tarefa depois de terminada começa de novo
    z.abandonar(vez); // a exportação saiu sem terminar (pânico): não fica rodando para sempre
    assert_eq!(z.instantaneo().fase, Fase::Erro);
    let vez = z.iniciar().unwrap();
    z.terminar(vez, Fase::Concluida, |_| {});
    z.abandonar(vez); // já terminada: nada muda
    assert_eq!(z.instantaneo().fase, Fase::Concluida);

    // recusada antes de começar (sem banco, banco trocado): termina com erro, a não ser que esteja rodando em
    // outra exportação
    let r = Progresso::novo();
    r.recusar();
    assert!(r.instantaneo().fase.terminou() && r.instantaneo().fase == Fase::Erro);
    let vez = r.iniciar().unwrap();
    r.recusar();
    assert_eq!(r.instantaneo().fase, Fase::Contando);
    r.terminar(vez, Fase::Concluida, |_| {});

    let textos: Vec<&str> = Fase::TODAS.iter().map(|f| f.como_str()).collect();
    assert_eq!(textos, ["contando", "gravando", "finalizando", "concluida", "cancelada", "erro", "desconhecida"]);
}

#[test]
fn api_exportacao_com_tarefa_registra_progresso_e_total() {
    let a = ambiente("tarefa-total");
    let id = a.id();
    let filtros = vec![Filtro::texto("tipo", Operador::Igual, "login")];
    let esperado = a.servico.contagem(&id, "eventos", Some("09"), &filtros, None).unwrap().linhas;
    let filtro = FiltroExportacao { busca: Some("09".into()), filtros, ..FiltroExportacao::default() };
    let t1 = Progresso::novo();
    let origem = OrigemExportacao::Tabela { nome: "eventos".into(), filtro };
    let r = exportar(&a, Some(&t1), &a.join("a.csv"), origem, Formato::Csv).unwrap();
    assert_eq!(
        t1.instantaneo(),
        ProgressoInfo {
            fase: Fase::Concluida,
            linhas: r.linhas,
            total: Some(esperado),
            tabela_atual: Some("eventos".into()),
            tabelas_feitas: 1,
            tabelas_total: 1
        }
    );
    let t2 = Progresso::novo();
    exportar(&a, Some(&t2), &a.join("b.csv"), tabela("eventos"), Formato::Csv).unwrap();
    assert_eq!(t2.instantaneo().total, Some(5000));
    let t3 = Progresso::novo();
    let consulta = OrigemExportacao::Consulta { sql: "SELECT * FROM clientes".into() };
    exportar(&a, Some(&t3), &a.join("c.json"), consulta, Formato::Json).unwrap();
    let p = t3.instantaneo();
    assert!(p.total.is_none() && p.linhas == 60 && p.tabela_atual.is_none() && p.fase == Fase::Concluida, "{p:?}");
    let t4 = Progresso::novo();
    let banco = OrigemExportacao::Banco { tabelas: Some(vec!["clientes".into(), "eventos".into()]) };
    exportar(&a, Some(&t4), &a.join("d.xlsx"), banco, Formato::Xlsx).unwrap();
    let p = t4.instantaneo();
    assert!(p.total == Some(5060) && p.linhas == 5060 && p.tabelas_feitas == 2 && p.tabelas_total == 2, "{p:?}");
    // sem progresso, tudo como antes
    exportar(&a, None, &a.join("e.csv"), tabela("eventos"), Formato::Csv).unwrap();
}

#[test]
fn api_total_do_banco_fica_null_quando_contar_demora() {
    let mut a = ambiente("total-demora");
    a.servico.prazo_contagem_banco = Duration::ZERO;
    let p = Progresso::novo();
    let banco = OrigemExportacao::Banco { tabelas: Some(vec!["clientes".into()]) };
    exportar(&a, Some(&p), &a.join("a.json"), banco, Formato::Json).unwrap();
    assert_eq!(p.instantaneo().total, None);
    assert_eq!(p.instantaneo().fase, Fase::Concluida);
}

#[test]
fn api_cancelar_antes_de_comecar_e_desconhecida() {
    let a = ambiente("cancelar-antes");
    let p = Progresso::novo();
    p.cancelar();
    assert_eq!(p.instantaneo().fase, Fase::Preparando);
    let destino = a.join("x.csv");
    let e = erro_com(exportar(&a, Some(&p), &destino, tabela("eventos"), Formato::Csv), 409);
    assert_eq!(
        e,
        comum::ErroBanco {
            mensagem: MSG_EXPORTACAO_CANCELADA.into(),
            detalhe: None,
            status: 409,
            codigo: Some(Codigo::Cancelado)
        }
    );
    assert!(!destino.exists());
    assert_eq!(p.instantaneo().fase, Fase::Cancelada);
    a.pasta.sem_temporarios();
}

#[test]
fn api_erro_na_exportacao_marca_a_fase() {
    let a = ambiente("erro-fase");
    let p = Progresso::novo();
    erro_com(exportar(&a, Some(&p), &a.join("a.csv"), tabela("fantasma"), Formato::Csv), 404);
    assert_eq!(p.instantaneo().fase, Fase::Erro);
}

#[test]
fn csv_exportado_com_tarefa_e_igual_ao_sem() {
    let a = ambiente("igual");
    let (x, y) = (a.join("a.csv"), a.join("b.csv"));
    exportar(&a, Some(&Progresso::novo()), &x, tabela("eventos"), Formato::Csv).unwrap();
    exportar(&a, None, &y, tabela("eventos"), Formato::Csv).unwrap();
    assert_eq!(fs::read(&x).unwrap(), fs::read(&y).unwrap());
    assert_eq!(ler_csv(&x, ';').len(), 5001);
}

#[test]
fn progresso_e_rotas_respondem_durante_a_exportacao() {
    let _vez = exclusivo();
    let pasta = Pasta::nova("respondem");
    let caminho = tabela_grande(&pasta.join("exp.db"), 200_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let destino = a.join("saida.xlsx");
    let p = Progresso::novo();
    let outra = Progresso::novo();
    let filtro = FiltroExportacao {
        filtros: vec![Filtro::texto("id", Operador::MenorIgual, "25000")],
        ..FiltroExportacao::default()
    };
    let (mut fases, mut linhas, mut t_progresso, mut t_cancelar) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (resposta, final_) = thread::scope(|s| {
        let origem = OrigemExportacao::Tabela { nome: "grande".into(), filtro };
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &destino, origem, Formato::Xlsx));
        while !exportacao.is_finished() {
            let inicio = Instant::now();
            let info = p.instantaneo();
            t_progresso.push(inicio.elapsed().as_secs_f64());
            if info.fase == Fase::Gravando {
                assert!(info.total == Some(25_000) && info.tabela_atual.as_deref() == Some("grande"), "{info:?}");
            }
            fases.push(info.fase);
            linhas.push(info.linhas);
            let inicio = Instant::now();
            outra.cancelar();
            t_cancelar.push(inicio.elapsed().as_secs_f64());
            thread::sleep(Duration::from_millis(2));
        }
        (exportacao.join().unwrap(), p.instantaneo())
    });
    resposta.unwrap();
    assert!(final_.fase == Fase::Concluida && final_.linhas == 25_000 && final_.tabelas_feitas == 1, "{final_:?}");
    assert!(fases.contains(&Fase::Gravando));
    assert!(linhas.windows(2).all(|j| j[0] <= j[1]));
    let maior = *linhas.iter().max().unwrap();
    assert!(maior > 0 && maior <= 25_000);
    let mut distintos = linhas.clone();
    distintos.dedup();
    assert!(distintos.len() > 3, "o progresso andou aos poucos: {distintos:?}");
    for (rota, tempos) in [("progresso", &mut t_progresso), ("cancelar", &mut t_cancelar)] {
        let (mediana, maximo) = mediana_e_maximo(tempos);
        assert!(mediana < 0.001 && maximo < 0.05, "{rota}: mediana {mediana} s, máximo {maximo} s");
    }
    assert_eq!(ler_xlsx(&destino)[0].linhas(), 25_001);
}

#[test]
fn progresso_lido_rapido_durante_um_milhao_de_linhas() {
    // a leitura do andamento usa só a trava do Progresso: microssegundos, enquanto 1 milhão de linhas são gravadas
    let _vez = exclusivo();
    let pasta = Pasta::nova("milhao");
    let caminho = tabela_grande(&pasta.join("milhao.db"), 1_000_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let destino = a.join("milhao.csv");
    let p = Progresso::novo();
    let mut tempos = Vec::new();
    let mut lidas = 0usize;
    let resumo = thread::scope(|s| {
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &destino, tabela("grande"), Formato::Csv));
        while !exportacao.is_finished() {
            let inicio = Instant::now();
            let info = p.instantaneo();
            tempos.push(inicio.elapsed().as_secs_f64());
            if info.fase == Fase::Gravando && info.linhas > 0 {
                lidas += 1;
            }
            thread::sleep(Duration::from_micros(500));
        }
        exportacao.join().unwrap()
    });
    let resumo = resumo.unwrap();
    assert_eq!(resumo.linhas, 1_000_000);
    assert_eq!(resumo.bytes, fs::metadata(&destino).unwrap().len());
    let final_ = p.instantaneo();
    assert!(final_.fase == Fase::Concluida && final_.linhas == 1_000_000 && final_.total == Some(1_000_000));
    assert!(lidas > 10, "o andamento foi lido só {lidas} vezes durante a gravação");
    let (mediana, maximo) = mediana_e_maximo(&mut tempos);
    // teto folgado: na prática, a mediana fica em poucos microssegundos
    assert!(mediana < 0.000_5 && maximo < 0.05, "{} leituras: mediana {mediana} s, máximo {maximo} s", tempos.len());
}

#[test]
fn progresso_responde_com_a_conexao_principal_presa() {
    // com transação aberta, a exportação lê pela conexão principal e segura a trava do banco até o fim; o
    // andamento e o cancelamento não esperam por ela
    let _vez = exclusivo();
    let pasta = Pasta::nova("principal");
    let caminho = tabela_grande(&pasta.join("exp.db"), 300_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let id = a.id();
    a.servico.sql(&id, "BEGIN; UPDATE grande SET t0 = 'na transação' WHERE id = 1;", 1000).unwrap();
    let destino = a.join("saida.json");
    let p = Progresso::novo();
    let gravando = AtomicBool::new(false);
    let (resposta, tempos, dt_cancelar) = thread::scope(|s| {
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &destino, tabela("grande"), Formato::Json));
        esperar(|| p.instantaneo().linhas >= 3000, "a exportação gravando");
        gravando.store(true, Ordering::SeqCst);
        let mut tempos = Vec::new();
        for _ in 0..200 {
            let inicio = Instant::now();
            let _ = p.instantaneo();
            tempos.push(inicio.elapsed().as_secs_f64());
        }
        let inicio = Instant::now();
        p.cancelar();
        let dt = inicio.elapsed().as_secs_f64();
        (exportacao.join().unwrap(), tempos, dt)
    });
    assert!(gravando.load(Ordering::SeqCst));
    let e = erro_com(resposta, 409);
    assert!(e.tem_codigo(Codigo::Cancelado));
    let (mediana, maximo) = mediana_e_maximo(&mut tempos.clone());
    assert!(mediana < 0.000_5 && maximo < 0.05 && dt_cancelar < 0.05, "{mediana} {maximo} {dt_cancelar}");
    assert!(!destino.exists());
    // a conexão principal voltou ao normal: a transação continua e grava
    a.servico.sql(&id, "UPDATE grande SET t0 = 'depois' WHERE id = 2; ROLLBACK", 1000).unwrap();
    assert!(!a.servico.info_banco(&id).unwrap().transacao_aberta);
    a.pasta.sem_temporarios();
}

#[test]
fn cancelar_exportacao_em_andamento() {
    let _vez = exclusivo();
    let pasta = Pasta::nova("cancelar");
    let caminho = tabela_grande(&pasta.join("exp.db"), 200_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let destino = a.join("saida.csv");
    fs::write(&destino, "o que já estava aqui").unwrap();
    let p = Progresso::novo();
    let (resposta, fim, pedido, dt) = thread::scope(|s| {
        let exportacao = s.spawn(|| {
            let r = exportar(&a, Some(&p), &destino, tabela("grande"), Formato::Csv);
            (r, Instant::now())
        });
        esperar(|| p.instantaneo().linhas >= 2000, "a exportação gravando");
        let inicio = Instant::now();
        p.cancelar();
        let dt = inicio.elapsed().as_secs_f64();
        let pedido = Instant::now();
        let (r, fim) = exportacao.join().unwrap();
        (r, fim, pedido, dt)
    });
    let progresso = p.instantaneo();
    p.cancelar(); // já terminada
    assert!(dt < 0.05, "{dt}");
    let e = erro_com(resposta, 409);
    assert!(e.mensagem == MSG_EXPORTACAO_CANCELADA && e.tem_codigo(Codigo::Cancelado));
    assert!(fim.saturating_duration_since(pedido) < Duration::from_secs(1));
    assert!(progresso.fase == Fase::Cancelada && (2000..200_000).contains(&progresso.linhas), "{progresso:?}");
    assert_eq!(fs::read_to_string(&destino).unwrap(), "o que já estava aqui");
    assert_eq!(p.instantaneo().fase, Fase::Cancelada);
    a.pasta.sem_temporarios();
    gravacao_passa(&a);
}

#[test]
fn cancelar_durante_a_contagem() {
    let _vez = exclusivo();
    let pasta = Pasta::nova("cancelar-contagem");
    let caminho = tabela_grande(&pasta.join("lenta.db"), 150_000, "grande", 8);
    let a = ambiente_de(pasta, &caminho);
    let termo = "98765432101";
    let filtro = FiltroExportacao { busca: Some(termo.into()), ..FiltroExportacao::default() };
    let banco = a.servico.atual().unwrap();
    let destino = a.join("saida.json");
    // sem o cancelamento no meio da contagem, a exportação esperaria pelo menos os 9/10 que faltam dela; com ele,
    // volta logo. A máquina carregada (várias compilações ao mesmo tempo) atrasa a volta de uma tentativa: vale a
    // melhor de três, cada uma com a contagem inteira medida logo antes, na mesma carga
    let mut vistas = Vec::new();
    for _ in 0..3 {
        let inicio = Instant::now();
        assert_eq!(banco.contar_para_exportar("grande", &filtro, None, None).unwrap(), Some(0));
        let completa = inicio.elapsed();
        let p = Progresso::novo();
        let (resposta, fim, pedido) = thread::scope(|s| {
            let origem = OrigemExportacao::Tabela { nome: "grande".into(), filtro: filtro.clone() };
            let exportacao = s.spawn(|| {
                let r = exportar(&a, Some(&p), &destino, origem, Formato::Json);
                (r, Instant::now())
            });
            esperar(|| p.instantaneo().fase == Fase::Contando, "a contagem");
            thread::sleep(completa / 10);
            p.cancelar();
            let pedido = Instant::now();
            let (r, fim) = exportacao.join().unwrap();
            (r, fim, pedido)
        });
        assert!(erro_com(resposta, 409).tem_codigo(Codigo::Cancelado));
        assert!(!destino.exists());
        a.pasta.sem_temporarios();
        let espera = fim.saturating_duration_since(pedido);
        vistas.push((espera, completa));
        if espera < (completa * 3 / 5).max(Duration::from_millis(300)) {
            gravacao_passa(&a);
            return;
        }
    }
    panic!("o cancelamento esperou a contagem (espera, contagem inteira): {vistas:?}");
}

#[test]
fn mesma_tarefa_em_duas_exportacoes_ao_mesmo_tempo() {
    let _vez = exclusivo();
    let pasta = Pasta::nova("mesma-tarefa");
    let caminho = tabela_grande(&pasta.join("exp.db"), 300_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let p = Progresso::novo();
    let segunda = a.join("segunda.csv");
    let (primeira, outra) = thread::scope(|s| {
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &a.join("primeira.csv"), tabela("grande"), Formato::Csv));
        esperar(|| p.instantaneo().linhas > 0, "a primeira exportação gravando");
        let outra = exportar(&a, Some(&p), &segunda, tabela("grande"), Formato::Csv);
        let andamento = p.instantaneo();
        assert!(andamento.fase == Fase::Gravando || andamento.fase == Fase::Finalizando, "{andamento:?}");
        p.cancelar();
        (exportacao.join().unwrap(), outra)
    });
    let e = erro_com(outra, 409);
    assert_eq!(e.mensagem, MSG_TAREFA_EM_ANDAMENTO);
    assert!(!segunda.exists());
    // a primeira seguiu com o mesmo andamento e foi ela que o cancelamento parou
    assert!(erro_com(primeira, 409).tem_codigo(Codigo::Cancelado));
    assert_eq!(p.instantaneo().fase, Fase::Cancelada);
}

// ---- regressões das revisões: Progresso reaproveitado, recusado antes de começar, troca de banco ----------

#[test]
fn progresso_reaproveitado_nao_e_encerrado_pela_exportacao_anterior() {
    // A tela deve criar um Progresso por exportação. Mesmo reaproveitado logo que a anterior diz "concluida",
    // o que sobra dela (ainda grava as opções nas preferências, o que espera até 3 s pela trava da outra
    // versão) não pode encerrar a seguinte com "erro" nem deixá-la sem poder ser interrompida.
    let _vez = exclusivo();
    let pasta = Pasta::nova("reaproveitado");
    let caminho = tabela_grande(&pasta.join("exp.db"), 200_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    // a outra versão segurando a trava das preferências
    let trava = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(a.servico.prefs().arquivo_trava())
        .unwrap();
    trava.try_lock().unwrap();
    let p = Progresso::novo();
    let filtro =
        FiltroExportacao { filtros: vec![Filtro::texto("id", Operador::MenorIgual, "100")], ..Default::default() };
    let pequena = OrigemExportacao::Tabela { nome: "grande".into(), filtro };
    let (primeira_csv, segunda_csv) = (a.join("primeira.csv"), a.join("segunda.csv"));
    thread::scope(|s| {
        let primeira = s.spawn(|| exportar(&a, Some(&p), &primeira_csv, pequena, Formato::Csv));
        esperar(|| p.instantaneo().fase == Fase::Concluida, "a primeira concluída");
        // a primeira ainda não voltou e a tela já dispara a seguinte com o mesmo andamento
        let segunda = s.spawn(|| exportar(&a, Some(&p), &segunda_csv, tabela("grande"), Formato::Csv));
        esperar(|| p.instantaneo().fase == Fase::Gravando && p.instantaneo().linhas >= 1000, "a segunda gravando");
        assert!(!primeira.is_finished(), "a primeira voltou antes de a segunda começar");
        trava.unlock().unwrap();
        assert_eq!(primeira.join().unwrap().unwrap().linhas, 100);
        let agora = p.instantaneo();
        assert!(agora.fase == Fase::Gravando || agora.fase == Fase::Finalizando, "{agora:?}");
        p.cancelar();
        assert!(p.cancelado());
        let e = erro_com(segunda.join().unwrap(), 409);
        assert!(e.tem_codigo(Codigo::Cancelado), "{e:?}");
    });
    assert_eq!(p.instantaneo().fase, Fase::Cancelada);
    assert!(primeira_csv.exists() && !segunda_csv.exists());
    a.pasta.sem_temporarios();
}

#[test]
fn nova_tentativa_com_substituir_usa_um_progresso_novo() {
    // o fluxo da janela Exportar: 409 destino_existe, a pessoa confirma e a tela refaz com substituir e um
    // Progresso novo
    let a = ambiente("nova-tentativa");
    let destino = a.join("x.csv");
    fs::write(&destino, "já existia").unwrap();
    let primeira = Progresso::novo();
    let e = erro_com(a.exportar(tabela("eventos"), Formato::Csv, &destino, Some(Arc::clone(&primeira))), 409);
    assert!(e.tem_codigo(Codigo::DestinoExiste), "{e:?}");
    assert_eq!(primeira.instantaneo().fase, Fase::Erro);
    let segunda = Progresso::novo();
    assert_eq!(exportar(&a, Some(&segunda), &destino, tabela("eventos"), Formato::Csv).unwrap().linhas, 5000);
    assert_eq!(
        segunda.instantaneo(),
        ProgressoInfo {
            fase: Fase::Concluida,
            linhas: 5000,
            total: Some(5000),
            tabela_atual: Some("eventos".into()),
            tabelas_feitas: 1,
            tabelas_total: 1
        }
    );
    assert_eq!(primeira.instantaneo().fase, Fase::Erro);
    // recusada antes de começar (banco trocado): o andamento termina com erro, não fica em "Preparando"
    let recusada = Progresso::novo();
    let pedido = PedidoExportacao {
        origem: tabela("eventos"),
        formato: Formato::Csv,
        caminho: a.join("y.csv"),
        opcoes: Opcoes::default(),
        substituir: true,
    };
    let e = erro_com(a.servico.exportar("outro banco", &pedido, Some(Arc::clone(&recusada))), 409);
    assert!(e.tem_codigo(Codigo::BancoTrocado), "{e:?}");
    assert_eq!(recusada.instantaneo().fase, Fase::Erro);
    assert!(!a.join("y.csv").exists());
}

#[test]
fn abrir_outro_banco_no_meio_da_exportacao_de_uma_tabela() {
    // a exportação de uma tabela lê pela conexão própria dela: a troca não espera e ela vai até o fim
    let _vez = exclusivo();
    let pasta = Pasta::nova("troca-tabela");
    let caminho = tabela_grande(&pasta.join("exp.db"), 200_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let outro = amostras::criar_banco_vazio(a.join("outro.db")).unwrap();
    let destino = a.join("saida.csv");
    let p = Progresso::novo();
    let (resposta, dt, na_troca, terminada) = thread::scope(|s| {
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &destino, tabela("grande"), Formato::Csv));
        esperar(|| p.instantaneo().linhas >= 1000, "a exportação gravando");
        let inicio = Instant::now();
        a.servico.abrir_banco(&outro, false, false).unwrap();
        let dt = inicio.elapsed();
        let na_troca = p.instantaneo();
        let terminada = exportacao.is_finished();
        (exportacao.join().unwrap(), dt, na_troca, terminada)
    });
    // a troca voltou com a exportação ainda no meio (lendo ou já trocando o temporário pelo arquivo): ela não esperou.
    // O teto do tempo é folgado: com a máquina carregada, abrir um banco pequeno passa às vezes de 1 s, e o que prova
    // que a troca não esperou é a exportação ainda correndo
    assert!(!terminada && matches!(na_troca.fase, Fase::Gravando | Fase::Finalizando), "{na_troca:?}");
    assert!(dt < Duration::from_secs(5), "abrir esperou {dt:?}");
    assert_eq!(resposta.unwrap().linhas, 200_000);
    assert_eq!(p.instantaneo().fase, Fase::Concluida);
    let csv = ler_csv(&destino, ';');
    assert_eq!(csv.len(), 200_001);
    assert_eq!(csv[200_000][0], "200000");
    assert_eq!(a.servico.info_banco(&a.id()).unwrap().nome, "outro.db");
    a.pasta.sem_temporarios();
}

#[test]
fn abrir_outro_banco_no_meio_da_exportacao_do_banco_inteiro() {
    // cada tabela abre a leitura dela quando a exportação chega nela: a seguinte encontra o banco fechado e a
    // exportação para com o 409 (sem código), sem deixar o .zip nem temporário
    let _vez = exclusivo();
    let pasta = Pasta::nova("troca-banco-inteiro");
    let caminho = tabela_grande(&pasta.join("exp.db"), 200_000, "grande", 2);
    tabela_grande(&caminho, 10, "pequena", 0);
    let a = ambiente_de(pasta, &caminho);
    let outro = amostras::criar_banco_vazio(a.join("outro.db")).unwrap();
    let destino = a.join("tudo.csv");
    let p = Progresso::novo();
    let origem = OrigemExportacao::Banco { tabelas: Some(vec!["grande".into(), "pequena".into()]) };
    let (resposta, na_troca) = thread::scope(|s| {
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &destino, origem, Formato::Csv));
        esperar(|| p.instantaneo().linhas >= 1000, "a exportação gravando a primeira tabela");
        a.servico.abrir_banco(&outro, false, false).unwrap();
        let na_troca = p.instantaneo();
        (exportacao.join().unwrap(), na_troca)
    });
    assert!(na_troca.fase == Fase::Gravando && na_troca.tabela_atual.as_deref() == Some("grande"), "{na_troca:?}");
    let e = erro_com(resposta, 409);
    assert_eq!(e.mensagem, "O banco já foi fechado. Abra-o de novo.");
    assert!(e.codigo.is_none(), "{e:?}");
    assert_eq!(p.instantaneo().fase, Fase::Erro);
    assert!(!a.join("tudo.zip").exists() && !destino.exists());
    a.pasta.sem_temporarios();
}

#[test]
fn fechar_durante_exportacao_com_transacao_aberta() {
    // Com transação aberta, a exportação lê pela conexão principal e a segura até o fim. Fechar sem descartar
    // precisa dela para saber se há transação: espera a exportação e responde 409. Descartando, fecha.
    let _vez = exclusivo();
    let pasta = Pasta::nova("fechar-exportando");
    let caminho = tabela_grande(&pasta.join("exp.db"), 200_000, "grande", 2);
    let a = ambiente_de(pasta, &caminho);
    let id = a.id();
    a.servico.sql(&id, "BEGIN; UPDATE grande SET t0 = 'na transação' WHERE id = 1;", 1000).unwrap();
    let destino = a.join("saida.csv");
    let p = Progresso::novo();
    let (resposta, recusa, na_volta) = thread::scope(|s| {
        let exportacao = s.spawn(|| exportar(&a, Some(&p), &destino, tabela("grande"), Formato::Csv));
        esperar(|| p.instantaneo().linhas >= 3000, "a exportação gravando");
        let recusa = a.servico.fechar_banco(&id, false);
        let na_volta = p.instantaneo();
        (exportacao.join().unwrap(), recusa, na_volta)
    });
    let e = erro_com(recusa, 409);
    assert!(e.tem_codigo(Codigo::TransacaoAberta), "{e:?}");
    // só voltou depois de a exportação soltar a conexão (já tinha lido tudo)
    assert!(na_volta.fase == Fase::Finalizando || na_volta.fase == Fase::Concluida, "{na_volta:?}");
    assert_eq!(resposta.unwrap().linhas, 200_000);
    // o arquivo saiu igual ao que a tela mostra (com a transação)
    let csv = ler_csv(&destino, ';');
    assert_eq!((csv.len(), csv[1][0].as_str(), csv[1][1].as_str()), (200_001, "1", "na transação"));
    // a pessoa escolhe descartar: fecha e a mudança some do arquivo
    a.servico.fechar_banco(&id, true).unwrap();
    assert!(a.servico.atual().is_none());
    assert_eq!(cru(&caminho, "SELECT t0 FROM grande WHERE id = 1"), [[comum::Valor::Texto("texto 7919".into())]]);
    a.pasta.sem_temporarios();
}
