//! Volumes grandes na aba Dados: a página que aparece
//! antes da contagem numa tabela grande com busca, a leitura substituída ignorada ao digitar rápido e as dicas de
//! página lenta (índice para ordenar, filtro no lugar da busca).
//!
//! A lentidão vem dos ganchos de teste da aba: a contagem espera um sinal do teste e o
//! `tempo_ms` da página é trocado por um valor alto. A leitura substituída usa uma tabela grande de verdade.

mod apoio;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use apoio::*;
use banco::PedidoPagina;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use estado::apoio_testes::Ambiente;
use estado::{Acao, ConsultaDados, FiltroDigitado};
use interface::tema::Tema;
use tela_dados::Ganchos;

/// Um sinal que o teste solta; solto também ao sair, para nenhuma thread ficar presa.
#[derive(Default)]
struct Liberar(Mutex<bool>, Condvar);

impl Liberar {
    /// Espera o sinal (no máximo 30 s; um despertar espúrio volta a esperar).
    fn esperar(&self) {
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut feito = self.0.lock().unwrap();
        while !*feito {
            let agora = std::time::Instant::now();
            if agora >= limite {
                break;
            }
            feito = self.1.wait_timeout(feito, limite - agora).unwrap().0;
        }
    }

    fn soltar(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_all();
    }
}

/// (busca, grupo da leitura) de uma contagem pedida.
type PedidoContagem = (Option<String>, Option<String>);

struct SoltarAoSair(Arc<Liberar>);

impl Drop for SoltarAoSair {
    fn drop(&mut self) {
        self.0.soltar();
    }
}

#[test]
fn tabela_grande_com_busca_mostra_a_pagina_antes_da_contagem() {
    let amb = Ambiente::exemplo("volumes_grande_busca");
    let liberar = Arc::new(Liberar::default());
    let _guarda = SoltarAoSair(Arc::clone(&liberar));
    let paginas: Arc<Mutex<Vec<PedidoPagina>>> = Arc::default();
    let contagens: Arc<Mutex<Vec<PedidoContagem>>> = Arc::default();
    let ganchos = {
        let (paginas, contagens, liberar) = (Arc::clone(&paginas), Arc::clone(&contagens), Arc::clone(&liberar));
        Ganchos {
            pagina: Some(Arc::new(move |pedido, _| paginas.lock().unwrap().push(pedido.clone()))),
            contagem: Some(Arc::new(move |busca, leitura| {
                contagens.lock().unwrap().push((busca.map(str::to_string), leitura.map(str::to_string)));
                liberar.esperar();
            })),
            ..Ganchos::default()
        }
    };
    // toda tabela passa por grande: a TabelaInfo vem sem contagem
    let mut tabela = amb.tabela("eventos");
    tabela.completa.info.linhas = None;
    let mut h = abrir_com(&amb, tabela, Tema::Claro, ganchos);
    assert_eq!(retrato(&h).contagem_linhas, "Linhas 1–100 de 5.000", "sem busca, conta como antes");

    let id_busca = campo_busca();
    preencher(&mut h, id_busca, "login");
    // a grade aparece logo, com a contagem ainda por vir
    esperar_que(&mut h, |r| !r.carregando && r.contagem_linhas.ends_with("de contando…"));
    let r = retrato(&h);
    assert_eq!(r.linhas, 100);
    assert_eq!(r.contagem_linhas, "Linhas 1–100 de contando…");
    assert_eq!(r.pagina_atual, "Página 1");
    assert_eq!(r.faixa, "contando…");
    assert!(r.proxima_habilitada, "o › segue o tem_mais");
    afastar_mouse(&mut h);
    capturar(&mut h, "dados-contando-claro.png");
    assert_eq!(ultima_rota(&h).map(|c| c.busca), Some("login".into()));
    let ultima = paginas.lock().unwrap().last().cloned().expect("pedido");
    assert_eq!(ultima.busca.as_deref(), Some("login"));
    assert!(!ultima.contar, "contar=false");
    assert_eq!(ultima.leitura.as_deref(), Some("grade:eventos"));
    let pedidos = contagens.lock().unwrap().clone();
    let buscas: Vec<_> = pedidos.iter().filter(|(b, _)| b.as_deref() == Some("login")).collect();
    assert_eq!(buscas.last().and_then(|(_, l)| l.clone()).as_deref(), Some("contagem:eventos"));

    // o "›" anda pelo tem_mais, ainda sem o total
    clicar_botao(&mut h, "Próxima página");
    esperar_que(&mut h, |r| r.contagem_linhas == "Linhas 101–200 de contando…" && !r.carregando);
    assert_eq!(retrato(&h).pagina_atual, "Página 2");

    liberar.soltar();
    let id = amb.estado.banco_id();
    let n = amb.servico.contagem(&id, "eventos", Some("login"), &[], None).expect("contagem").linhas;
    let total = interface::formato::formatar_numero(n);
    esperar_que(&mut h, |r| r.contagem_linhas == format!("Linhas 101–200 de {total}"));
    let r = retrato(&h);
    assert_eq!(r.pagina_atual, format!("Página 2 de {}", n.div_ceil(100)));
    assert_eq!(r.faixa, format!("{total} linhas"), "sem o total da tabela, só as achadas");
    // a contagem já conhecida vale para as outras páginas da mesma busca: a página vem sem contar e sem pedir
    // outra contagem
    let antes = contagens.lock().unwrap().iter().filter(|(b, _)| b.as_deref() == Some("login")).count();
    clicar_botao(&mut h, "Página anterior");
    esperar_que(&mut h, |r| r.contagem_linhas == format!("Linhas 1–100 de {total}") && !r.carregando);
    assert!(!paginas.lock().unwrap().last().expect("pedido").contar);
    assert_eq!(contagens.lock().unwrap().iter().filter(|(b, _)| b.as_deref() == Some("login")).count(), antes);
    // o "Exportar…" já sabe o total da busca
    match h.state().tela.doc.tabela.pedido_exportacao().origem {
        estado::OrigemJanelaExportar::Tabela { filtro: Some(f), .. } => assert_eq!(f.linhas, Some(n)),
        outra => panic!("{outra:?}"),
    }
}

#[test]
fn tabela_grande_com_filtro_conta_a_parte_e_completa_o_total() {
    let amb = Ambiente::exemplo("volumes_grande_filtro");
    let mut tabela = amb.tabela("eventos");
    tabela.completa.info.linhas = None;
    let filtro = FiltroDigitado { coluna: "tipo".into(), op: banco::Operador::Contem, valor: "login".into() };
    tabela.rota = tabela.rota.com_dados(ConsultaDados { filtros: vec![filtro.clone()], ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    esperar_que(&mut h, |r| r.faixa.ends_with(" linhas"));
    let id = amb.estado.banco_id();
    let n = amb.servico.contagem(&id, "eventos", None, &[filtro.para_banco()], None).expect("contagem").linhas;
    assert_eq!(retrato(&h).pagina_atual, format!("Página 1 de {}", n.div_ceil(100)));
}

#[test]
fn contagem_que_falha_deixa_as_linhas_nao_contadas() {
    let amb = Ambiente::exemplo("volumes_contagem_falha");
    // a contagem à parte da busca quebra (um erro inesperado do serviço)
    let ganchos = Ganchos {
        contagem: Some(Arc::new(|busca, _| {
            if busca.is_some() {
                panic!("contagem quebrada de propósito");
            }
        })),
        ..Ganchos::default()
    };
    let mut tabela = amb.tabela("eventos");
    tabela.completa.info.linhas = None;
    tabela.rota = tabela.rota.com_dados(ConsultaDados { busca: "login".into(), ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, ganchos);
    esperar_que(&mut h, |r| r.faixa == "linhas não contadas");
    let r = retrato(&h);
    assert_eq!(r.contagem_linhas, "Linhas 1–100 de …");
    assert_eq!(r.pagina_atual, "Página 1");
    assert!(r.proxima_habilitada, "o › continua seguindo o tem_mais");
    assert!(r.erro.is_none(), "a grade continua na tela");
    assert_eq!(r.linhas, 100);
}

#[test]
fn leitura_substituida_e_ignorada_ao_digitar_rapido() {
    let amb = Ambiente::exemplo("volumes_substituida");
    // uma tabela grande de verdade: cada busca leva um tempo no SQLite, e a letra seguinte chega antes
    executar(
        &amb,
        "CREATE TABLE grande AS WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 400000) \
         SELECT i AS id, 'nome ' || i AS nome, i % 97 AS grupo, hex(randomblob(12)) AS codigo FROM n;",
    );
    let abandonadas = Arc::new(AtomicUsize::new(0));
    let ganchos = {
        let a = Arc::clone(&abandonadas);
        Ganchos {
            abandonada: Some(Arc::new(move || {
                a.fetch_add(1, Ordering::SeqCst);
            })),
            ..Ganchos::default()
        }
    };
    let mut amb = amb;
    amb.recarregar();
    let mut h = abrir_com(&amb, amb.tabela("grande"), Tema::Claro, ganchos);
    let id_busca = campo_busca();
    h.ctx.memory_mut(|m| m.request_focus(id_busca));
    h.step();
    // cada letra espera os 300ms do adiamento (dois quadros de 250ms) e sai antes de a anterior terminar
    for letra in ["9", "9", "9", "9", "1"] {
        digitar(&mut h, letra);
    }
    esperar_que(&mut h, |r| !r.carregando && r.faixa.ends_with("linhas") && r.linhas > 0);
    let r = retrato(&h);
    assert_eq!(ultima_rota(&h).map(|c| c.busca), Some("99991".into()));
    assert!(r.erro.is_none(), "nada de erro na tela");
    assert!(toast(&h).is_none_or(|t| !t.contains("substituída")));
    assert!(abandonadas.load(Ordering::SeqCst) > 0, "as leituras velhas foram abandonadas pelas novas");
    let aba = &h.state().tela.doc.parte;
    assert!((0..4).any(|c| aba.texto_celula(0, c).is_some_and(|t| t.contains("99991"))), "a página da busca final");
}

/// A página com `tempo_ms` alto: ordenada, 4,6 s; com busca, 3,4 s.
fn paginas_lentas() -> Ganchos {
    Ganchos {
        pagina: Some(Arc::new(|pedido: &PedidoPagina, p: &mut banco::Pagina| {
            if pedido.ordem.is_some() {
                p.tempo_ms = 4600.0;
            } else if pedido.busca.is_some() {
                p.tempo_ms = 3400.0;
            }
        })),
        ..Ganchos::default()
    }
}

#[test]
fn ordenar_lento_sugere_indice_e_abre_a_janela_com_a_coluna() {
    let amb = Ambiente::exemplo("volumes_sugere_indice");
    let mut h = abrir_com(&amb, amb.tabela("clientes"), Tema::Claro, paginas_lentas());
    assert_eq!(retrato(&h).sugestao, None);

    clicar_botao(&mut h, "cidade");
    esperar_grade(&mut h);
    assert_eq!(
        retrato(&h).sugestao.as_deref(),
        Some("Ordenar por cidade levou 4,6 s. Um índice nessa coluna deixa a ordenação imediata.")
    );
    assert!(h.query_by_role_and_label(Role::Button, "Criar índice").is_some());
    afastar_mouse(&mut h);
    capturar(&mut h, "dados-sugestao-indice-claro.png");

    clicar_botao(&mut h, "Criar índice");
    assert!(h.state().acoes.iter().any(|a| matches!(a,
        Acao::AbrirNovoIndice { tabela, colunas } if tabela == "clientes" && colunas == &vec!["cidade".to_string()])));

    // com o índice criado (a janela Novo índice grava e a casca refaz a tela), a mesma ordenação não sugere nada
    let id = amb.estado.banco_id();
    let operacao = servico::OperacaoEsquema::CriarIndice {
        nome: "idx_clientes_cidade".into(),
        tabela: "clientes".into(),
        colunas: vec![servico::ColunaIndicePedida { nome: "cidade".into(), desc: false }],
        unico: false,
        onde: None,
    };
    amb.servico.esquema(&id, &operacao, false).expect("criar índice");
    let mut h = recarregar_com(&amb, h, paginas_lentas());
    assert_eq!(ultima_rota(&h).and_then(|c| c.ordem).as_deref(), Some("cidade"));
    assert_eq!(retrato(&h).sugestao, None);

    // a coluna que é o próprio rowid (id INTEGER PRIMARY KEY) nunca pede índice
    clicar_botao(&mut h, "id");
    esperar_grade(&mut h);
    assert_eq!(ultima_rota(&h).and_then(|c| c.ordem).as_deref(), Some("id"));
    assert_eq!(retrato(&h).sugestao, None);
}

#[test]
fn busca_lenta_sugere_filtro_por_coluna() {
    let amb = Ambiente::exemplo("volumes_busca_lenta");
    let mut tabela = amb.tabela("clientes");
    tabela.rota = tabela.rota.com_dados(ConsultaDados { busca: "Recife".into(), ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, paginas_lentas());
    assert_eq!(
        retrato(&h).sugestao.as_deref(),
        Some("A busca percorre todas as colunas; um filtro por coluna costuma ser mais rápido.")
    );
    assert!(h.query_by_role_and_label(Role::Button, "Criar índice").is_none());

    // com um filtro por coluna, a dica some
    condicao_pelo_popover(&mut h, "ativo", "igual a", Some("1"));
    esperar_que(&mut h, |r| !r.carregando && r.linhas > 0 && r.sugestao.is_none());
    let rota = ultima_rota(&h).expect("rota");
    assert_eq!(
        rota.filtros,
        vec![FiltroDigitado { coluna: "ativo".into(), op: banco::Operador::Igual, valor: "1".into() }]
    );
}
