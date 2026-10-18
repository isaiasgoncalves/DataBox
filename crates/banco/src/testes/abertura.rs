//! Abertura, informações do banco e fechamento.

use std::fs;
use std::path::{Component, Path, PathBuf};

use comum::{Chave, Codigo, Valor};

use super::*;
use crate::caminhos::com_sufixo;
use crate::{Banco, ContagemObjetos};

#[test]
fn abre_em_pasta_com_espaco_e_acentos() {
    let tmp = Pasta::nova("acentos");
    let pasta = tmp.join("pasta com espaço e ação");
    fs::create_dir(&pasta).unwrap();
    let caminho = amostras::criar_banco_exemplo_sem_fts(pasta.join("vendas #1.db")).unwrap();
    {
        let b = Banco::abrir(&caminho, false, false).unwrap();
        assert_eq!(b.caminho(), canonico(&caminho));
        assert_eq!(b.nome(), "vendas #1.db");
        assert_eq!(contar(&b, "clientes"), N_CLIENTES);
        b.inserir(TAB_ESPACO, &vals(&[(COL_CITADA, tx("gravado")), ("número", tx("9"))])).unwrap();
    }
    assert_eq!(
        cru(
            &caminho,
            "SELECT \"número\" FROM \"Tabela com espaço\" WHERE \"coluna \"\"citada\"\"\" = ?",
            &[vt("gravado")]
        ),
        vec![vec![Valor::Inteiro(9)]]
    );
    let b = Banco::abrir(&caminho, true, false).unwrap();
    assert_eq!(contar(&b, "pedidos"), N_PEDIDOS);
    let info = b.info().unwrap();
    assert_eq!(info.pasta, canonico(&pasta));
    assert!(info.somente_leitura);
}

/// Caminho de `de` até `para` com "..".
fn relativo(de: &Path, para: &Path) -> PathBuf {
    let a: Vec<Component<'_>> = de.components().collect();
    let b: Vec<Component<'_>> = para.components().collect();
    let iguais = |x: &Component<'_>, y: &Component<'_>| {
        x.as_os_str().to_string_lossy().to_lowercase() == y.as_os_str().to_string_lossy().to_lowercase()
    };
    let comum = a.iter().zip(&b).take_while(|(x, y)| iguais(x, y)).count();
    let mut saida = PathBuf::new();
    for _ in comum..a.len() {
        saida.push("..");
    }
    for c in &b[comum..] {
        saida.push(c.as_os_str());
    }
    saida
}

#[test]
fn caminho_relativo_e_resolvido() {
    let tmp = Pasta::nova("relativo");
    let caminho = caminho_exemplo(&tmp);
    let atual = canonico(&std::env::current_dir().unwrap());
    let rel = relativo(&atual, &canonico(&caminho));
    assert!(rel.is_relative(), "{rel:?}");
    let b = Banco::abrir(&rel, false, false).unwrap();
    assert_eq!(b.caminho(), canonico(&caminho));
    assert!(b.caminho().is_absolute());
}

#[test]
fn info_tem_todos_os_campos() {
    let ex = exemplo("info");
    let info = ex.banco.info().unwrap();
    assert_eq!(info.id, ex.banco.id());
    assert_eq!(info.id.len(), 16);
    assert!(info.id.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!info.transacao_aberta && info.motivo_somente_leitura.is_none());
    assert_eq!(info.caminho, canonico(&ex.caminho));
    assert_eq!(info.nome, "exemplo.db");
    assert_eq!(info.tamanho, fs::metadata(&ex.caminho).unwrap().len());
    let data = info.modificado_em.clone().unwrap();
    let formato = "dddd-dd-ddTdd:dd:dd";
    assert_eq!(data.len(), formato.len(), "{data}");
    assert!(
        data.chars().zip(formato.chars()).all(|(c, f)| if f == 'd' { c.is_ascii_digit() } else { c == f }),
        "{data}"
    );
    assert_eq!(info.sqlite_versao, rusqlite::version());
    assert_eq!(info.codificacao, "UTF-8");
    assert!(info.tamanho_pagina > 0 && info.paginas > 0);
    assert_eq!((info.paginas * info.tamanho_pagina) as u64, info.tamanho);
    assert_eq!(info.modo_diario, "delete");
    assert_eq!(info.versao_usuario, 0);
    assert!(info.chaves_estrangeiras);
    assert!(!info.somente_leitura);
    // tabelas: clientes, produtos, pedidos, itens_pedido, eventos, Tabela com espaço, notas
    // índices: três criados + os automáticos de clientes.email (UNIQUE) e da PK de produtos (WITHOUT ROWID)
    assert_eq!(info.contagem, ContagemObjetos { tabelas: 7, visoes: 1, indices: 5, gatilhos: 1 });
}

#[test]
fn arquivo_inexistente_da_404() {
    let tmp = Pasta::nova("inexistente");
    let e = erro(Banco::abrir(&tmp.join("nao_existe.db"), false, false));
    assert_eq!(e.status, 404);
    assert!(!tmp.join("nao_existe.db").exists());
}

#[test]
fn pasta_no_lugar_do_arquivo() {
    let tmp = Pasta::nova("pasta");
    let e = erro(Banco::abrir(&tmp.caminho, false, false));
    assert_eq!(e.status, 400);
    assert!(e.mensagem.contains("pasta"));
}

#[test]
fn arquivo_que_nao_e_banco() {
    let tmp = Pasta::nova("falso");
    let falso = tmp.join("planilha.db");
    fs::write(&falso, "isto nao e um banco sqlite ".repeat(100)).unwrap();
    let e = erro(Banco::abrir(&falso, false, false));
    assert_eq!(e.status, 400);
    assert!(e.mensagem.contains("não é um banco SQLite"), "{}", e.mensagem);
    assert!(e.detalhe.is_some_and(|d| !d.is_empty()));
}

#[test]
fn arquivo_vazio_e_banco_valido() {
    let tmp = Pasta::nova("zero");
    let vazio = tmp.join("zero.db");
    fs::write(&vazio, b"").unwrap();
    let b = Banco::abrir(&vazio, false, false).unwrap();
    let objs = b.objetos().unwrap();
    assert!(objs.tabelas.is_empty() && objs.visoes.is_empty() && objs.indices.is_empty());
    assert_eq!(objs.sistema.iter().map(|s| s.nome.as_str()).collect::<Vec<_>>(), ["sqlite_schema"]);
    assert_eq!(b.info().unwrap().contagem, ContagemObjetos::default());
}

#[test]
fn banco_vazio_da_fixture() {
    let tmp = Pasta::nova("vazio");
    let caminho = amostras::criar_banco_vazio(tmp.join("vazio.db")).unwrap();
    let b = Banco::abrir(&caminho, false, false).unwrap();
    assert!(b.objetos().unwrap().tabelas.is_empty());
}

#[test]
fn criar_banco_novo() {
    let tmp = Pasta::nova("novo");
    let novo = tmp.join("novo.db");
    {
        let b = Banco::abrir(&novo, false, true).unwrap();
        assert!(novo.exists());
        let r = b.consultar("CREATE TABLE t (a); INSERT INTO t VALUES (1);", 1000).unwrap();
        assert_eq!(r.resultados.iter().map(|x| x.tipo()).collect::<Vec<_>>(), ["alteracao", "alteracao"]);
    }
    assert_eq!(cru(&novo, "SELECT a FROM t", &[]), vec![vec![Valor::Inteiro(1)]]);
}

#[test]
fn criar_em_pasta_inexistente() {
    let tmp = Pasta::nova("sem-pasta");
    let e = erro(Banco::abrir(&tmp.join("nao").join("existe.db"), false, true));
    assert_eq!(e.status, 404);
}

#[test]
fn conexao_configurada() {
    let ex = exemplo("configurada");
    ex.banco
        .com_conexao(|conn| {
            assert!(conn.is_autocommit());
            let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
            assert_eq!(fk, 1);
            // a trava é reentrante: dentro dela, o banco continua usável
            assert!(!ex.banco.transacao_aberta());
            assert_eq!(contar(&ex.banco, "clientes"), N_CLIENTES);
            Ok(())
        })
        .unwrap();
}

#[test]
fn fechar_e_usar_depois() {
    let ex = exemplo("fechar");
    ex.banco.fechar();
    ex.banco.fechar(); // fechar de novo não é erro
    let e = erro(ex.banco.objetos());
    assert_eq!(e.status, 409);
}

#[test]
fn uso_em_varias_threads() {
    let ex = exemplo("threads");
    let b = &ex.banco;
    let primeiros: Vec<i64> = std::thread::scope(|s| {
        let tarefas: Vec<_> = (0..8)
            .map(|t| {
                s.spawn(move || {
                    (1..=40u64)
                        .filter(|p| p % 8 == t)
                        .map(|p| (p, rowid(&pag(b, "eventos", |x| (x.pagina, x.por_pagina) = (p, 50)).linhas[0].chave)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut todos: Vec<(u64, i64)> = tarefas.into_iter().flat_map(|t| t.join().unwrap()).collect();
        todos.sort_unstable();
        todos.into_iter().map(|(_, r)| r).collect()
    });
    assert_eq!(primeiros, (0..40).map(|i| 1 + 50 * i).collect::<Vec<i64>>());
}

#[test]
#[allow(clippy::permissions_set_readonly_false)]
fn arquivo_com_atributo_somente_leitura_abre_so_para_leitura() {
    struct Restaurar(PathBuf);
    impl Drop for Restaurar {
        fn drop(&mut self) {
            if let Ok(m) = fs::metadata(&self.0) {
                let mut p = m.permissions();
                p.set_readonly(false);
                let _ = fs::set_permissions(&self.0, p);
            }
        }
    }
    let tmp = Pasta::nova("protegido");
    let p =
        banco_de(&tmp, "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT); INSERT INTO t VALUES (1, 'a');", "caso.db");
    {
        let restaurar = Restaurar(p.clone());
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(&p, perm).unwrap();
        let b = Banco::abrir(&p, false, false).unwrap();
        assert!(b.somente_leitura());
        assert!(b.info().unwrap().motivo_somente_leitura.unwrap().contains("protegido contra gravação"));
        let t = b.tabela_info("t").unwrap();
        assert!(!t.editavel && !t.esquema_editavel);
        assert_eq!(erro(b.atualizar("t", &Chave::Rowid(1), &vals(&[("v", tx("b"))]))).status, 403);
        drop(b);
        drop(restaurar);
    }
    let b = Banco::abrir(&p, false, false).unwrap();
    assert!(!b.somente_leitura() && b.info().unwrap().motivo_somente_leitura.is_none());
}

#[test]
fn journal_quente_so_leitura_explica_o_que_fazer() {
    let tmp = Pasta::nova("journal");
    let mut roteiro = String::from("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT);");
    for i in 1..=3000 {
        roteiro.push_str(&format!("INSERT INTO t VALUES ({i}, 'original');"));
    }
    let p = banco_de(&tmp, &roteiro, "caso.db");
    // Uma transação interrompida no meio (o processo que gravava morreu): com o cache de uma página, o UPDATE
    // já levou páginas novas ao arquivo e guardou as originais no -journal; a cópia dos dois é o que sobra.
    let quente = tmp.join("quente.db");
    {
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch("PRAGMA cache_size=1; BEGIN; UPDATE t SET v = 'meio ' || hex(randomblob(40));").unwrap();
        fs::write(&quente, fs::read(&p).unwrap()).unwrap();
        fs::write(com_sufixo(&quente, "-journal"), fs::read(com_sufixo(&p, "-journal")).unwrap()).unwrap();
        c.execute_batch("ROLLBACK").unwrap();
    }
    assert!(com_sufixo(&quente, "-journal").exists());
    let e = erro(Banco::abrir(&quente, true, false));
    assert_eq!(e.status, 409);
    assert!(e.mensagem.contains("transação interrompida") && e.mensagem.contains("para edição"), "{}", e.mensagem);
    let b = Banco::abrir(&quente, false, false).unwrap(); // para edição, o SQLite recupera
    assert_eq!(b.linha("t", &Chave::Rowid(1)).unwrap().valor("v"), Some(&ct("original")));
}

#[test]
fn fechar_com_transacao_aberta_avisa_no_registro() {
    instalar_registro();
    let ex = exemplo("registro");
    let b = &ex.banco;
    b.consultar("BEGIN; UPDATE clientes SET cidade = 'Natal' WHERE id = 1;", 1000).unwrap();
    assert!(b.transacao_aberta() && b.info().unwrap().transacao_aberta);
    let e = erro(b.exigir_sem_transacao("fechar o banco"));
    assert_eq!(e.status, 409);
    assert_eq!(e.codigo, Some(Codigo::TransacaoAberta));
    b.fechar();
    let caminho = b.caminho().display().to_string();
    assert!(
        mensagens_registradas().iter().any(|m| m.contains("transação aberta") && m.contains(&caminho)),
        "{:?}",
        mensagens_registradas()
    );
}

#[test]
fn esquema_do_arquivo_nao_e_confiavel() {
    let ex = exemplo("trusted-schema");
    let ligado = |c: &Connection| c.query_row("PRAGMA trusted_schema", [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(ex.banco.com_conexao(|c| Ok(ligado(c))).unwrap(), 0);
    assert_eq!(ligado(&ex.banco.conexao_leitura().unwrap()), 0);
    let ro = exemplo_ro("trusted-schema-ro");
    assert_eq!(ro.banco.com_conexao(|c| Ok(ligado(c))).unwrap(), 0);
}
