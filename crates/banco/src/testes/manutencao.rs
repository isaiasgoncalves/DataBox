//! Manutenção e cópia de segurança.

use std::fs;
use std::sync::atomic::Ordering;
use std::time::Instant;

use comum::Codigo;

use super::*;
use crate::AcaoManutencao;
use crate::caminhos::com_sufixo;

#[test]
fn manutencao() {
    for acao in AcaoManutencao::TODAS {
        let ex = exemplo(acao.como_str());
        let r = ex.banco.manutencao(acao).unwrap();
        assert!(r.acao == acao && r.ok, "{r:?}");
        assert!(!r.mensagens.is_empty());
        assert!(r.tamanho_antes > 0 && r.tamanho_depois > 0);
    }
}

#[test]
fn manutencao_chaves_quebradas_e_vacuum_reduz() {
    let ex = exemplo("chaves-quebradas");
    let b = &ex.banco;
    b.consultar(
        "PRAGMA foreign_keys = OFF; INSERT INTO pedidos (cliente_id, feito_em) VALUES (9999, 'x'); PRAGMA foreign_keys = ON;",
        1000,
    )
    .unwrap();
    let r = b.manutencao(AcaoManutencao::Chaves).unwrap();
    assert!(!r.ok && r.mensagens.len() == 1 && r.mensagens[0].contains("pedidos"), "{r:?}");
    assert_eq!(
        r.mensagens[0],
        format!("pedidos: a linha {} aponta para clientes, onde o valor não existe.", N_PEDIDOS + 1)
    );
    b.consultar("DELETE FROM eventos", 1000).unwrap();
    let r = b.manutencao(AcaoManutencao::Vacuum).unwrap();
    assert!(r.tamanho_depois < r.tamanho_antes);
    let e = erro(AcaoManutencao::de_texto("formatar"));
    assert_eq!(e.status, 400);
    assert_eq!(e.mensagem, "Ação de manutenção desconhecida: formatar.");
}

#[test]
fn vacuum_com_transacao_aberta() {
    let ex = exemplo("vacuum-transacao");
    ex.banco.consultar("BEGIN", 1000).unwrap();
    assert_eq!(erro(ex.banco.manutencao(AcaoManutencao::Vacuum)).status, 409);
    ex.banco.consultar("ROLLBACK", 1000).unwrap();
}

#[test]
fn copia_de_seguranca() {
    let ex = exemplo("copia");
    let pasta = ex.pasta.join("cópias de segurança");
    fs::create_dir(&pasta).unwrap();
    let destino = pasta.join("exemplo cópia.db");
    let r = ex.banco.copia_de_seguranca(&destino).unwrap();
    assert_eq!(r.caminho, canonico(&destino));
    assert_eq!(r.tamanho, fs::metadata(&destino).unwrap().len());
    assert_eq!(cru_int(&destino, "SELECT count(*) FROM eventos") as u64, N_EVENTOS);
    let nomes: Vec<String> =
        fs::read_dir(&pasta).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(nomes, ["exemplo cópia.db"]); // sem temporário esquecido
    fs::write(&destino, b"lixo que vai ser sobrescrito").unwrap();
    ex.banco.copia_de_seguranca(&destino).unwrap();
    assert_eq!(cru_int(&destino, "SELECT count(*) FROM clientes") as u64, N_CLIENTES);
}

#[test]
fn copia_de_seguranca_nao_aceita_o_proprio_arquivo() {
    let ex = exemplo("copia-propria");
    let b = &ex.banco;
    let pai = ex.caminho.parent().unwrap();
    assert_eq!(erro(b.copia_de_seguranca(&ex.caminho)).status, 400);
    assert_eq!(erro(b.copia_de_seguranca(&pai.join(".").join("exemplo.db"))).status, 400);
    assert_eq!(erro(b.copia_de_seguranca(&pai.join("nao existe").join("x.db"))).status, 404);
    assert_eq!(contar(b, "clientes"), N_CLIENTES);
}

#[test]
fn copia_de_seguranca_com_transacao_aberta_e_recusada_na_hora() {
    let _exclusivo = exclusivo();
    let ex = exemplo("copia-transacao");
    let b = &ex.banco;
    b.consultar("BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;", 1000).unwrap();
    let inicio = Instant::now();
    let e = erro(b.copia_de_seguranca(&ex.pasta.join("copia.db")));
    assert!(e.status == 409 && e.codigo == Some(Codigo::TransacaoAberta));
    assert!(inicio.elapsed().as_secs_f64() < 1.0);
    assert!(!ex.pasta.join("copia.db").exists());
    assert!(
        fs::read_dir(&ex.pasta.caminho).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".copia.db."))
    );
    b.consultar("ROLLBACK", 1000).unwrap();
}

#[test]
fn copia_de_seguranca_com_arquivo_travado_por_fora_desiste() {
    let _exclusivo = exclusivo();
    let ex = exemplo("copia-travada");
    let b = &ex.banco;
    b.ajustes.espera_copia_ms.store(500, Ordering::Relaxed);
    let outro = rusqlite::Connection::open(&ex.caminho).unwrap();
    outro.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let inicio = Instant::now();
    let e = erro(b.copia_de_seguranca(&ex.pasta.join("copia.db")));
    let decorrido = inicio.elapsed();
    outro.execute_batch("ROLLBACK").unwrap();
    drop(outro);
    assert!(e.status == 409 && decorrido.as_secs_f64() < 8.0, "{e:?} {decorrido:?}");
    assert!(
        fs::read_dir(&ex.pasta.caminho).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".copia.db."))
    );
    assert_eq!(b.info().unwrap().nome, "exemplo.db"); // a trava foi solta
}

#[test]
fn arquivos_do_banco_journal_wal_shm() {
    let ex = exemplo("journal-wal-shm");
    let b = &ex.banco;
    assert!(b.e_arquivo_do_banco(&ex.caminho));
    for sufixo in ["-journal", "-wal", "-shm", "-JOURNAL"] {
        assert!(b.e_arquivo_do_banco(&com_sufixo(&ex.caminho, sufixo)), "{sufixo}");
    }
    assert!(!b.e_arquivo_do_banco(&ex.pasta.join("outro.db")));
    assert_eq!(erro(b.copia_de_seguranca(&com_sufixo(&ex.caminho, "-journal"))).status, 400);
}
