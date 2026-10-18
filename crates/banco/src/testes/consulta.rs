//! Consulta livre e divisão de roteiros.

use std::thread;
use std::time::{Duration, Instant};

use comum::{Celula, Chave};

use super::*;
use crate::{FiltroExportacao, Interrupcao, MSG_INTERROMPIDA, ResultadoComando, ResultadoConsulta, dividir_comandos};

/// Um comando que levaria minutos sem a parada (mil milhões de voltas do WITH RECURSIVE): se o Interromper
/// falhar, o teste falha em vez de pendurar.
const CONSULTA_LONGA: &str =
    "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 1000000000) SELECT count(*) FROM c;";
/// Um comando de milissegundos, mas com muito mais que os 20.000 passos entre duas conferências do progress
/// handler: um handler esquecido ligado com a marca acesa o cortaria.
const CONSULTA_MEDIA: &str =
    "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 300000) SELECT count(*) FROM c;";

#[test]
fn dividir_comandos_do_roteiro() {
    assert_eq!(dividir_comandos("SELECT 1; SELECT 2"), ["SELECT 1;", "SELECT 2"]);
    assert_eq!(dividir_comandos("SELECT 'a;b'; SELECT \"c;d\" FROM t;"), ["SELECT 'a;b';", "SELECT \"c;d\" FROM t;"]);
    assert_eq!(
        dividir_comandos("-- comentário; ainda comentário\nSELECT 1; /* ; */ SELECT 2;"),
        ["-- comentário; ainda comentário\nSELECT 1;", "/* ; */ SELECT 2;"]
    );
    let gatilho = "CREATE TRIGGER t AFTER INSERT ON a BEGIN UPDATE a SET x = 1; DELETE FROM b; END;";
    assert_eq!(dividir_comandos(&format!("{gatilho}\nSELECT 3;")), [gatilho, "SELECT 3;"]);
    assert!(dividir_comandos(" ; ;; -- só comentário\n ").is_empty());
    assert!(dividir_comandos("").is_empty());
    assert_eq!(dividir_comandos("SELECT 1; -- fim"), ["SELECT 1;"]);
    assert_eq!(dividir_comandos("SELECT 1\0; SELECT 2"), ["SELECT 1\0; SELECT 2"]); // NUL não derruba nada
}

#[test]
fn consultar_varios_comandos_com_gatilho() {
    let ex = exemplo("varios-comandos");
    let sql = "
        CREATE TABLE registro (id INTEGER PRIMARY KEY, texto TEXT);
        CREATE TRIGGER trg_registro AFTER INSERT ON clientes
        BEGIN
            INSERT INTO registro (texto) VALUES ('novo; cliente ' || new.nome);
            UPDATE registro SET texto = texto || '!' WHERE id = last_insert_rowid();
        END;
        INSERT INTO clientes (nome) VALUES ('Gatilho Teste');
        SELECT texto FROM registro;
    ";
    let r = ex.banco.consultar(sql, 1000).unwrap();
    assert_eq!(
        r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(),
        ["alteracao", "alteracao", "alteracao", "linhas"]
    );
    let gatilho = r.resultados[1].comando();
    assert!(gatilho.starts_with("CREATE TRIGGER") && gatilho.ends_with("END;"));
    assert!(matches!(r.resultados[2], ResultadoComando::Alteracao { linhas_afetadas: Some(1), .. }));
    match &r.resultados[3] {
        ResultadoComando::Linhas { colunas, linhas, truncado, tempo_ms, .. } => {
            assert_eq!(colunas, &["texto"]);
            assert_eq!(linhas, &vec![vec![ct("novo; cliente Gatilho Teste!")]]);
            assert!(!truncado && *tempo_ms >= 0.0);
        }
        outro => panic!("{outro:?}"),
    }
    assert!(r.alterou_esquema && r.alterou_dados && !r.transacao_aberta);
    assert_eq!(cru_int(&ex.caminho, "SELECT count(*) FROM sqlite_schema WHERE name = 'trg_registro'"), 1);
}

#[test]
fn consultar_so_leitura_nao_altera() {
    let ex = exemplo("so-leitura");
    let r = ex.banco.consultar("SELECT * FROM clientes; PRAGMA table_info(clientes)", 1000).unwrap();
    assert!(!r.alterou_esquema && !r.alterou_dados);
    match &r.resultados[0] {
        ResultadoComando::Linhas { linhas, .. } => assert_eq!(linhas.len() as u64, N_CLIENTES),
        outro => panic!("{outro:?}"),
    }
    assert_eq!(r.resultados[1].tipo(), "linhas");
}

#[test]
fn consultar_limite_e_truncado() {
    let ex = exemplo("truncado");
    let b = &ex.banco;
    let r = b.consultar("SELECT * FROM eventos", 10).unwrap();
    assert!(matches!(&r.resultados[0], ResultadoComando::Linhas { linhas, truncado: true, .. } if linhas.len() == 10));
    let r = b.consultar("SELECT * FROM eventos LIMIT 10", 10).unwrap();
    assert!(matches!(&r.resultados[0], ResultadoComando::Linhas { truncado: false, .. }));
    let r = b
        .consultar("SELECT observacao, serie FROM pedidos, produtos WHERE pedidos.id = 10 AND codigo = 'P004'", 1000)
        .unwrap();
    match &r.resultados[0] {
        ResultadoComando::Linhas { linhas, .. } => assert_eq!(
            linhas,
            &vec![vec![
                Celula::TextoCortado { inicio: format!("Texto longo {}", "x".repeat(288)), tamanho: 712 },
                ci(9007199254740997)
            ]]
        ),
        outro => panic!("{outro:?}"),
    }
}

#[test]
fn consultar_para_no_primeiro_erro() {
    let ex = exemplo("primeiro-erro");
    let b = &ex.banco;
    let r =
        b.consultar("UPDATE eventos SET tipo = 'x' WHERE rowid = 1; SELEC errado; DELETE FROM eventos;", 1000).unwrap();
    assert_eq!(r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(), ["alteracao", "erro"]);
    match &r.resultados[1] {
        ResultadoComando::Erro { comando, mensagem, detalhe } => {
            assert_eq!(comando, "SELEC errado;");
            assert!(mensagem.contains("Erro no comando SQL"), "{mensagem}");
            assert_eq!(mensagem, "Erro no comando SQL: near \"SELEC\": syntax error.");
            assert!(detalhe.as_deref().is_some_and(|d| !d.is_empty()));
        }
        outro => panic!("{outro:?}"),
    }
    assert!(r.alterou_dados);
    assert_eq!(contar(b, "eventos"), N_EVENTOS); // o DELETE não rodou
    assert_eq!(cru(&ex.caminho, "SELECT tipo FROM eventos WHERE rowid = 1", &[]), vec![vec![vt("x")]]); // o anterior gravou
}

#[test]
fn consultar_transacao_aberta() {
    let ex = exemplo("transacao");
    let b = &ex.banco;
    let r = b.consultar("BEGIN; DELETE FROM eventos WHERE rowid <= 10;", 1000).unwrap();
    assert!(r.transacao_aberta);
    assert!(matches!(r.resultados[0], ResultadoComando::Alteracao { linhas_afetadas: None, .. }));
    assert!(matches!(r.resultados[1], ResultadoComando::Alteracao { linhas_afetadas: Some(10), .. }));
    // enquanto a transação está aberta, a exclusão pela grade usa SAVEPOINT e continua funcionando
    assert_eq!(b.excluir("eventos", &[Chave::Rowid(11)]).unwrap(), 1);
    let r = b.consultar("ROLLBACK", 1000).unwrap();
    assert!(!r.transacao_aberta);
    assert_eq!(contar(b, "eventos"), N_EVENTOS);
}

#[test]
fn consultar_vazio() {
    let ex = exemplo("consulta-vazia");
    let r = ex.banco.consultar("  -- nada ", 1000).unwrap();
    assert_eq!(
        r,
        ResultadoConsulta { resultados: vec![], alterou_esquema: false, alterou_dados: false, transacao_aberta: false }
    );
}

#[test]
fn consultar_com_parametro_sem_valor_explica() {
    let ex = exemplo("parametro");
    let r = ex.banco.consultar("SELECT 1; INSERT INTO eventos (tipo) VALUES (?)", 1000).unwrap();
    match &r.resultados[1] {
        ResultadoComando::Erro { mensagem, .. } => assert_eq!(
            mensagem,
            "O SQLite recusou a operação: Incorrect number of bindings supplied. The current statement uses 1, and \
             there are 0 supplied.."
        ),
        outro => panic!("{outro:?}"),
    }
    match &r.resultados[0] {
        ResultadoComando::Linhas { comando, colunas, linhas, .. } => {
            assert_eq!(
                (comando.as_str(), colunas.clone(), linhas.clone()),
                ("SELECT 1;", vec!["1".to_string()], vec![vec![ci(1)]])
            );
        }
        outro => panic!("{outro:?}"),
    }
}

#[test]
fn consultar_coluna_com_nome_em_utf8_invalido_nao_derruba() {
    // um nome de coluna gravado em Latin-1 por outro programa: o rusqlite entraria em pânico ao ler o nome
    let tmp = Pasta::nova("nome-latin1");
    let p = tmp.join("caso.db");
    {
        let conn = rusqlite::Connection::open(&p).unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (1);").unwrap();
        // troca o nome "a" por um byte Latin-1 (é) direto no esquema
        conn.execute_batch(
            "PRAGMA writable_schema = ON;\
             UPDATE sqlite_schema SET sql = CAST(x'435245415445205441424c45207420286ee920494e544547455229' AS TEXT) \
             WHERE name = 't';\
             PRAGMA writable_schema = OFF;",
        )
        .unwrap();
    }
    let b = Banco::abrir(&p, false, false).unwrap();
    let r = b.consultar("SELECT * FROM t", 1000).unwrap();
    match &r.resultados[0] {
        ResultadoComando::Linhas { colunas, linhas, .. } => {
            assert_eq!(colunas, &["n\u{fffd}"]);
            assert_eq!(linhas, &vec![vec![ci(1)]]);
        }
        outro => panic!("{outro:?}"),
    }
    let mut lidas = 0;
    b.iterar_consulta("SELECT * FROM t", None, |colunas, linhas| {
        assert_eq!(colunas, ["n\u{fffd}"]);
        lidas = linhas.count();
        Ok(())
    })
    .unwrap();
    assert_eq!(lidas, 1);
}

/// O `detalhe` do comando cortado pela parada (a mensagem é sempre a do DataBox).
fn detalhe_interrompido(r: &ResultadoComando, comando_cortado: &str) -> Option<String> {
    match r {
        ResultadoComando::Erro { comando, mensagem, detalhe } => {
            assert_eq!(comando, comando_cortado);
            assert_eq!(mensagem, MSG_INTERROMPIDA);
            detalhe.clone()
        }
        outro => panic!("esperava o comando interrompido, veio {outro:?}"),
    }
}

/// A marca e o progress handler saíram com a consulta: a mesma conexão roda até o fim um comando muito mais
/// longo que os passos entre duas conferências, e a exportação por ela (transação aberta) também.
fn conexao_sem_a_parada(b: &Banco) {
    assert!(b.parada_consulta.lock().is_none());
    assert!(b.consultas_na_fila.lock().is_empty());
    assert!(!b.interromper_consulta());
    let direto = b.com_conexao(|c| Ok(c.query_row(CONSULTA_MEDIA, [], |r| r.get::<_, i64>(0)))).unwrap();
    assert_eq!(direto.unwrap(), 300_000);
    b.com_conexao(|c| {
        c.execute_batch("BEGIN").unwrap();
        Ok(())
    })
    .unwrap();
    let exportadas = b
        .iterar_consulta(CONSULTA_MEDIA.trim_end_matches(';'), None, |_, linhas| linhas.collect::<Resultado<Vec<_>>>())
        .unwrap();
    assert_eq!(exportadas, vec![vec![Valor::Inteiro(300_000)]]);
    b.com_conexao(|c| {
        c.execute_batch("ROLLBACK").unwrap();
        Ok(())
    })
    .unwrap();
    let r = b.consultar("SELECT count(*) FROM eventos; SELECT 1", 1000).unwrap();
    assert_eq!(r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(), ["linhas", "linhas"]);
}

#[test]
fn interromper_para_a_consulta_longa() {
    let ex = exemplo("interromper-longa");
    let b = &ex.banco;
    let roteiro = format!("{CONSULTA_LONGA} SELECT 2;");
    // O pedido pode chegar antes de o comando começar no SQLite (a marca conferida antes dele, sem detalhe) ou
    // com ele rodando (o interrupt e o progress handler, com o erro do SQLite no detalhe): as duas formas têm a
    // mesma frase. Repete até ver o corte no meio do comando.
    let mut no_meio = false;
    for _ in 0..10 {
        let (r, demora) = thread::scope(|s| {
            let longa = s.spawn(|| b.consultar(&roteiro, 1000));
            esperar(|| b.parada_consulta.lock().is_some() || longa.is_finished(), "a consulta começar");
            thread::sleep(Duration::from_millis(30));
            let pedido = Instant::now();
            assert!(b.interromper_consulta());
            let r = longa.join().unwrap().unwrap();
            (r, pedido.elapsed())
        });
        assert!(demora < Duration::from_secs(10), "{demora:?}");
        assert_eq!(r.resultados.len(), 1, "o SELECT 2 não roda: {:?}", r.resultados);
        assert!(!r.alterou_dados && !r.alterou_esquema && !r.transacao_aberta);
        if let Some(d) = detalhe_interrompido(&r.resultados[0], CONSULTA_LONGA) {
            assert_eq!(d, "interrupted");
            no_meio = true;
            break;
        }
    }
    assert!(no_meio, "nenhuma vez o comando foi cortado no meio");
    conexao_sem_a_parada(b);
}

#[test]
fn interromper_no_meio_do_roteiro_nao_roda_o_resto() {
    let ex = exemplo("interromper-roteiro");
    let b = &ex.banco;
    let roteiro =
        format!("INSERT INTO eventos (tipo) VALUES ('antes da parada'); {CONSULTA_LONGA} DELETE FROM eventos;");
    let r = thread::scope(|s| {
        let longa = s.spawn(|| b.consultar(&roteiro, 1000));
        // o INSERT já está no arquivo: o comando longo é o que roda agora (ou o próximo a começar)
        esperar(
            || {
                longa.is_finished()
                    || cru_int(&ex.caminho, "SELECT count(*) FROM eventos WHERE tipo = 'antes da parada'") == 1
            },
            "o primeiro comando gravar",
        );
        assert!(b.interromper_consulta());
        longa.join().unwrap().unwrap()
    });
    assert_eq!(r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(), ["alteracao", "erro"]);
    assert!(matches!(r.resultados[0], ResultadoComando::Alteracao { linhas_afetadas: Some(1), .. }));
    let detalhe = detalhe_interrompido(&r.resultados[1], CONSULTA_LONGA);
    assert!(detalhe.is_none() || detalhe.as_deref() == Some("interrupted"), "{detalhe:?}");
    assert!(r.alterou_dados && !r.transacao_aberta);
    // o anterior gravou, o DELETE não rodou
    assert_eq!(contar(b, "eventos"), N_EVENTOS + 1);
    assert_eq!(cru_int(&ex.caminho, "SELECT count(*) FROM eventos"), (N_EVENTOS + 1) as i64);
    conexao_sem_a_parada(b);
}

/// C7: o Interromper com a consulta ainda esperando a vez (a conexão com outra operação) liga a marca dela, sem
/// chamar o interrupt (a conexão não é dela): ao pegar a conexão, ela sai interrompida sem executar nada, e a
/// operação que segurava a conexão vai até o fim.
#[test]
fn interromper_com_a_consulta_esperando_a_vez() {
    let ex = exemplo("interromper-na-fila");
    let b = &ex.banco;
    let antes = contar(b, "eventos");
    let (preso_tx, preso_rx) = std::sync::mpsc::channel::<()>();
    let (solta_tx, solta_rx) = std::sync::mpsc::channel::<()>();
    let r = thread::scope(|s| {
        // quem segura a conexão: uma operação longa de outro tipo (aqui, parada até o teste soltar)
        let dono = s.spawn(move || {
            b.com_conexao(|c| {
                preso_tx.send(()).unwrap();
                solta_rx.recv_timeout(Duration::from_secs(20)).unwrap();
                Ok(c.query_row(CONSULTA_MEDIA, [], |r| r.get::<_, i64>(0)))
            })
        });
        preso_rx.recv_timeout(Duration::from_secs(20)).unwrap();
        // sem consulta nenhuma, nada a interromper (e o interrupt não sai: o dono não é uma consulta)
        assert_eq!(b.interromper_consulta_detalhado(), Interrupcao::Nenhuma);
        let espera = s.spawn(|| b.consultar("INSERT INTO eventos (tipo) VALUES ('nao devia rodar'); SELECT 1;", 100));
        esperar(|| b.consultas_esperando() == 1, "a consulta entrar na fila");
        assert_eq!(b.interromper_consulta_detalhado(), Interrupcao::NaFila);
        assert!(b.interromper_consulta(), "true também para a que espera");
        solta_tx.send(()).unwrap();
        assert_eq!(dono.join().unwrap().unwrap().unwrap(), 300_000, "o interrupt não alcançou o dono da conexão");
        espera.join().unwrap().unwrap()
    });
    assert_eq!(r.resultados.len(), 1, "{:?}", r.resultados);
    assert!(detalhe_interrompido(&r.resultados[0], "INSERT INTO eventos (tipo) VALUES ('nao devia rodar');").is_none());
    assert!(!r.alterou_dados && !r.alterou_esquema && !r.transacao_aberta);
    assert_eq!(contar(b, "eventos"), antes, "o INSERT não rodou");
    assert_eq!(b.consultas_esperando(), 0);
    conexao_sem_a_parada(b);
    assert_eq!(b.interromper_consulta_detalhado(), Interrupcao::Nenhuma);
}

#[test]
fn interromper_sem_consulta_nao_afeta_a_proxima() {
    let ex = exemplo("interromper-sem-consulta");
    let b = &ex.banco;
    assert!(!b.interromper_consulta());
    // nem com outra operação de posse da conexão: o interrupt não sai, e ela vai até o fim
    thread::scope(|s| {
        let (entrou_tx, entrou_rx) = std::sync::mpsc::channel();
        let outra = s.spawn(move || {
            b.com_conexao(|c| {
                entrou_tx.send(()).unwrap();
                Ok(c.query_row(CONSULTA_MEDIA, [], |r| r.get::<_, i64>(0)))
            })
        });
        entrou_rx.recv_timeout(Duration::from_secs(20)).unwrap();
        while !outra.is_finished() {
            assert!(!b.interromper_consulta());
            thread::yield_now();
        }
        assert_eq!(outra.join().unwrap().unwrap().unwrap(), 300_000);
    });
    let r = b.consultar(&format!("SELECT count(*) FROM eventos; {CONSULTA_MEDIA}"), 1000).unwrap();
    assert_eq!(r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(), ["linhas", "linhas"]);
    match &r.resultados[1] {
        ResultadoComando::Linhas { linhas, .. } => assert_eq!(linhas, &vec![vec![ci(300_000)]]),
        outro => panic!("{outro:?}"),
    }
    conexao_sem_a_parada(b);
    // a grade e a exportação de uma tabela também
    b.inserir("eventos", &vals(&[("tipo", tx("depois"))])).unwrap();
    let n = b.iterar_tabela("eventos", &FiltroExportacao::default(), None, |_, linhas| Ok(linhas.count())).unwrap();
    assert_eq!(n as u64, N_EVENTOS + 1);
}
