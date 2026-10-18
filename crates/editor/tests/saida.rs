//! Fechar o aplicativo: o que o `main` faz entre a janela fechar e o processo sair.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use banco::{MSG_INTERROMPIDA, ResultadoComando};
use estado::apoio_testes::Ambiente;

/// C12: a janela fechou com uma Consulta SQL longa rodando. O `main` espera as tarefas de fundo por um prazo (5 s) e
/// sairia matando o comando no meio; com o `interromper_ao_sair`, o comando é cortado na hora e a tarefa termina.
#[test]
fn fechar_com_uma_consulta_rodando_a_interrompe_em_vez_de_esperar_o_prazo() {
    let amb = Ambiente::exemplo("saida_consulta_rodando");
    let id = amb.estado.banco_id();
    // levaria minutos sem o corte
    let infinita = "WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 2000000000) \
                    SELECT count(*) AS n FROM s;";
    // sem consulta nenhuma, nada a cortar
    assert!(!editor::interromper_ao_sair(&amb.servico));
    let servico = Arc::clone(&amb.servico);
    let consulta = std::thread::spawn(move || servico.sql(&id, infinita, 10));
    let inicio = Instant::now();
    while !editor::interromper_ao_sair(&amb.servico) {
        assert!(!consulta.is_finished(), "a consulta terminou sozinha");
        assert!(inicio.elapsed() < Duration::from_secs(15), "a consulta não começou");
        std::thread::sleep(Duration::from_millis(5));
    }
    let r = consulta.join().expect("a thread da consulta").expect("a resposta");
    assert!(inicio.elapsed() < Duration::from_secs(10), "cortada na hora, não pelo prazo: {:?}", inicio.elapsed());
    match &r.resultados[..] {
        [ResultadoComando::Erro { mensagem, .. }] => assert_eq!(mensagem, MSG_INTERROMPIDA),
        outro => panic!("esperava o comando interrompido: {outro:?}"),
    }
    // e a conexão fica livre: o banco responde logo
    assert!(!editor::interromper_ao_sair(&amb.servico));
    assert!(amb.servico.sql(&amb.estado.banco_id(), "SELECT 1", 10).is_ok());
}
