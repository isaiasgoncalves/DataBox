//! As peças internas (reconhecimento de texto sem regex, comando completo, texto citado, formatação de milhar)
//! e garantias do tipo Plano.

use ddl::{ColunaDef, Gerada, Referencia};

use super::*;
use crate::colunas::normalizar_coluna;
use crate::completo::instrucao_completa;
use crate::sql::{erro_de_objeto, milhar, plural, repr_texto};

#[test]
fn instrucao_completa_como_o_sqlite3_complete() {
    // respostas do sqlite3_complete do SQLite
    let casos = [
        ("SELECT 1", false),
        ("SELECT 1;", true),
        ("SELECT 1; -- x", true),
        ("SELECT 1 -- x;", false),
        ("SELECT ';'", false),
        ("SELECT ';';", true),
        ("/* ; */", false),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; END;", true),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1 END;", false),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; END", false),
        ("CREATE TEMP TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; end ;", true),
        ("create temporary trigger t after insert on x begin select 1; /* c */ END /* d */ ;", true),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 'END;'; END;", true),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT \"a;\"; END;", true),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT CASE WHEN 1 THEN 2 END; END;", true),
        ("EXPLAIN CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; END;", true),
        ("CREATE TABLE t (a);", true),
        ("CREATE VIEW v AS SELECT 1;", true),
        ("CREATE TRIGGER", false),
        ("CREATE TRIGGER;", false),
        ("", false),
        (";", true),
        ("  ;  ", true),
        ("SELECT 'abc", false),
        ("SELECT [a;", false),
        ("SELECT 1; /* aberto", false),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; END; SELECT 2;", true),
        ("CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; END; SELECT 2", false),
    ];
    for (sql, esperado) in casos {
        assert_eq!(instrucao_completa(sql), esperado, "{sql}");
    }
}

#[test]
fn erro_de_objeto_como_a_regex() {
    let m = erro_de_objeto("error in view v_tipos after drop column: no such column: tipo").unwrap();
    assert_eq!((m.tipo, m.nome, m.causa), ("view", "v_tipos", "no such column: tipo"));
    // sem caixa, nome com espaço e " after " dentro (o menor nome que casa), causa com quebra de linha
    let m = erro_de_objeto("ERROR IN Trigger meu gatilho AFTER RENAME: x\ny").unwrap();
    assert_eq!((m.tipo, m.nome, m.causa), ("trigger", "meu gatilho", "x\ny"));
    let m = erro_de_objeto("error in index a after b after rename: c").unwrap();
    assert_eq!(m.nome, "a after b");
    let m = erro_de_objeto("antes: error in table t after drop column: ç").unwrap();
    assert_eq!((m.tipo, m.nome, m.causa), ("table", "t", "ç"));
    assert!(erro_de_objeto("error in view v after drop column: ").is_none());
    assert!(erro_de_objeto("error in view  after rename: x").is_none());
    assert!(erro_de_objeto("error in coluna v after rename: x").is_none());
    assert!(erro_de_objeto("no such column: x").is_none());
    assert!(erro_de_objeto("ção error in view ã after rename: é").is_some_and(|m| m.nome == "ã"));
}

#[test]
fn milhar_e_plural() {
    assert_eq!(milhar(0), "0");
    assert_eq!(milhar(999), "999");
    assert_eq!(milhar(5000), "5.000");
    assert_eq!(milhar(1234567), "1.234.567");
    assert_eq!(milhar(-1234), "-1.234");
    assert_eq!(plural(1, "linha", "linhas"), "1 linha");
    assert_eq!(plural(0, "linha", "linhas"), "0 linhas");
    assert_eq!(plural(5000, "linha", "linhas"), "5.000 linhas");
}

#[test]
fn texto_citado() {
    assert_eq!(repr_texto("modelos"), "'modelos'");
    assert_eq!(repr_texto("d'água"), "\"d'água\"");
    assert_eq!(repr_texto("a'b\"c"), "'a\\'b\"c'");
    assert_eq!(repr_texto("x\\y\n"), "'x\\\\y\\n'");
}

#[test]
fn normalizar_coluna_apara_e_esvazia() {
    let c = ColunaDef {
        nome: " a ".into(),
        tipo: "  VARCHAR (10)\n ".into(),
        padrao: Some("  ".into()),
        verificacao: Some(" a > 0 ".into()),
        colacao: Some("".into()),
        referencia: Some(Referencia {
            tabela: " t ".into(),
            coluna: Some(" ".into()),
            ao_excluir: Some(" set \t null ".into()),
            ao_atualizar: None,
            complemento: Some(" ".into()),
        }),
        gerada: Some(Gerada { expressao: " ".into(), armazenada: true }),
        extra: Some(" x ".into()),
        ..ColunaDef::default()
    };
    let n = normalizar_coluna(&c);
    assert_eq!(n.nome, " a "); // o nome é aparado por quem chama
    assert_eq!(n.tipo, "VARCHAR (10)");
    assert_eq!(n.padrao, None);
    assert_eq!(n.verificacao.as_deref(), Some("a > 0"));
    assert_eq!(n.colacao, None);
    let r = n.referencia.unwrap();
    assert_eq!(
        (r.tabela.as_str(), r.coluna, r.ao_excluir.as_deref(), r.complemento),
        (" t ", None, Some("SET NULL"), None)
    );
    assert_eq!(n.gerada, None);
    assert_eq!(n.extra.as_deref(), Some("x"));
    let sem_tabela = ColunaDef { referencia: Some(Referencia { tabela: " ".into(), ..Referencia::default() }), ..c };
    assert_eq!(normalizar_coluna(&sem_tabela).referencia, None);
}

#[test]
fn plano_compara_so_os_campos_publicos_e_e_send_sync() {
    fn exige_send_sync<T: Send + Sync>() {}
    exige_send_sync::<Plano>();
    exige_send_sync::<crate::Definicoes>();
    let a = ambiente();
    let conn = &a.conn;
    let nova = ColunaDef { tipo: "VARCHAR(9)".into(), ..definicao(conn, "pedidos", "observacao") };
    let plano = crate::plano_alterar_coluna(conn, "pedidos", "observacao", &nova).unwrap();
    let copia = Plano {
        descricao: plano.descricao.clone(),
        comandos: plano.comandos.clone(),
        reconstroi: true,
        avisos: plano.avisos.clone(),
        ..Plano::default()
    };
    assert_eq!(plano, copia);
    assert!(!plano.sessao.is_empty() && !plano.ja_havia.is_empty());
}

#[test]
fn erro_de_varios_comandos_tem_a_frase() {
    // o "..." duplo é de propósito (a frase do SQLite já termina em ponto)
    let a = ambiente();
    for sql in ["SELECT 1; SELECT 2", "SELECT 1; garbage here", "SELECT 1;;", "SELECT 1; -- x\n /* y */ ; "] {
        let e = erro(crate::plano::ensaiar(&a.conn, &textos(&[sql]), "Contexto."));
        assert_eq!(e.mensagem, "Contexto. O SQLite recusou a operação: You can only execute one statement at a time..");
        assert_eq!(e.detalhe.as_deref(), Some("You can only execute one statement at a time."));
    }
    // o erro do primeiro comando vem antes (só ele é preparado)
    let e = erro(crate::plano::ensaiar(&a.conn, &textos(&["SELECT * FROM nao_existe; SELECT 2"]), "Contexto."));
    assert_eq!(e.mensagem, "Contexto. A tabela não existe: nao_existe.");
    // um comando só, com ";", espaços e comentários no fim, passa
    let gatilho = "CREATE TRIGGER tg AFTER INSERT ON eventos BEGIN SELECT 1; SELECT 2; END; -- fim\n";
    for sql in ["SELECT 1; -- x\n /* y */ ", "SELECT 1", gatilho] {
        crate::plano::ensaiar(&a.conn, &textos(&[sql]), "Contexto.").unwrap();
    }
}

#[test]
fn erro_cru_do_sqlite_leva_a_frase_do_servidor() {
    // O erro da montagem do plano vem com "Não foi possível preparar a mudança de estrutura."; o
    // definicoes_tabela, sem essa frase.
    let a = ambiente();
    let outra = Connection::open(&a.caminho).unwrap();
    outra.execute_batch("BEGIN EXCLUSIVE").unwrap();
    a.conn.busy_timeout(std::time::Duration::ZERO).unwrap();
    let travado = "O banco está em uso por outro programa. Feche-o lá e tente de novo.";
    let e = erro(crate::plano_excluir_tabela(&a.conn, "eventos"));
    assert_eq!(e.mensagem, format!("Não foi possível preparar a mudança de estrutura. {travado}"));
    assert_eq!(e.status, 409);
    let e = erro(definicoes_tabela(&a.conn, "eventos"));
    assert_eq!((e.mensagem.as_str(), e.status), (travado, 409));
    outra.execute_batch("ROLLBACK").unwrap();
    assert!(crate::plano_excluir_tabela(&a.conn, "eventos").is_ok());
}

#[test]
fn primeiro_comando_como_o_sqlite_separa() {
    use crate::completo::primeiro_comando;
    assert_eq!(primeiro_comando("SELECT 1"), (8, false));
    assert_eq!(primeiro_comando("SELECT 1; "), (9, false));
    assert_eq!(primeiro_comando("SELECT ';'; SELECT 2"), (11, true));
    let gatilho = "CREATE TRIGGER t AFTER INSERT ON x BEGIN SELECT 1; END; SELECT 2";
    assert_eq!(primeiro_comando(gatilho), (gatilho.find("END;").unwrap() + 4, true));
    assert_eq!(primeiro_comando("SELECT 'aberto"), (14, false));
}

#[test]
fn nul_e_parametro_tem_as_frases() {
    let a = ambiente();
    let conn = &a.conn;
    // A causa: o rusqlite passa o comprimento e o SQLite para de ler no NUL, sem erro (o resto vira "cauda"
    // vazia). Por isso todo SQL passa antes por `recusar_nulo`.
    let cortado = conn.prepare("SELECT count(*) FROM eventos\0 WHERE 0").unwrap();
    assert_eq!(cortado.expanded_sql().as_deref(), Some("SELECT count(*) FROM eventos"));
    drop(cortado);
    let nul = "the query contains a null character";
    for sql in ["SELECT 1\0", "\0", "SELECT 1; \0 SELECT 2"] {
        let e = erro(crate::plano::ensaiar(conn, &textos(&[sql]), "Contexto."));
        assert_eq!(e.mensagem, format!("Contexto. O SQLite recusou a operação: {nul}."));
        assert_eq!(e.detalhe.as_deref(), Some(nul));
    }
    let e = crate::sql::linhas(conn, "SELECT ?1\0", [1], |r| crate::sql::inteiro_em(r, 0)).unwrap_err();
    assert_eq!(e.to_string(), nul);
    let e = crate::sql::inteiro(conn, "PRAGMA foreign_keys\0 = OFF", []).unwrap_err();
    assert_eq!(e.to_string(), nul);
    // Parâmetro que o SQLite aceita no prepare e ninguém liga: a frase do sqlite3 (com o ponto duplo).
    for (sql, n) in [("SELECT ?", 1), ("SELECT :x, @y, $z", 3), ("SELECT ?5", 5)] {
        let e = erro(crate::plano::ensaiar(conn, &textos(&[sql]), "Contexto."));
        assert_eq!(
            e.mensagem,
            format!(
                "Contexto. O SQLite recusou a operação: Incorrect number of bindings supplied. The current statement \
                 uses {n}, and there are 0 supplied.."
            )
        );
    }
}

#[test]
fn mensagem_do_sqlite_sem_o_trecho_do_rusqlite() {
    let a = ambiente();
    let e = erro(crate::plano::ensaiar(&a.conn, &textos(&["SELECT xyz FROM eventos"]), "Contexto."));
    assert_eq!(e.mensagem, "Contexto. A coluna não existe: xyz.");
    assert_eq!(e.detalhe.as_deref(), Some("no such column: xyz"));
}
