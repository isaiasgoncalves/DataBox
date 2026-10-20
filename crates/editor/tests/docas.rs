//! As docas e as janelas próprias dos painéis e os menus Banco e Tabela (revisão de UI de 19/10/2026): arrastar a aba
//! de um painel para a outra doca, o "Destacar em janela", o "Prender de volta" e o "Fechar" da janela própria, o lugar
//! guardado nas preferências (`"lugares_paineis"`), as vistas das Propriedades da tabela e os itens dos menus.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;

use apoio::*;
use editor::{Editor, Partida};
use egui::accesskit::Role;
use egui::{Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::{Queryable, by};
use estado::Rota;
use estado::apoio_testes::Ambiente;
use interface::tema::Tema;
use prefs::{LugarPainel, LugaresPaineis};

fn botao(pos: Pos2, apertado: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: apertado, modifiers: Modifiers::NONE }
}

/// Clica no ponto (o botão que tem um rótulo repetido em outro lugar).
fn clicar_em(h: &mut H, pos: Pos2) {
    h.hover_at(pos);
    h.run_steps(1);
    h.event(botao(pos, true));
    h.event(botao(pos, false));
    h.run_steps(2);
}

/// Arrasta de `de` até `para` em quatro passos e solta lá.
fn arrastar_e_soltar(h: &mut H, de: Pos2, para: Pos2) {
    h.hover_at(de);
    h.run_steps(2);
    h.event(botao(de, true));
    h.run_steps(1);
    for t in 1..=4 {
        h.hover_at(de + (para - de) * (t as f32 / 4.0));
        h.run_steps(2);
    }
    h.event(botao(para, false));
    h.run_steps(3);
    assentar(h);
    h.remove_cursor();
    h.run_steps(2);
}

/// O botão com este rótulo mais à direita (o "Destacar em janela" da doca da direita).
fn mais_a_direita(h: &H, rotulo: &str) -> egui::Rect {
    h.query_all(by().role(Role::Button).label(rotulo))
        .map(|n| n.rect())
        .max_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap_or_else(|| panic!("o botão {rotulo}"))
}

/// Espera o lugar guardado nas preferências.
fn esperar_lugares(h: &mut H, amb: &Ambiente, esperados: LugaresPaineis) {
    let servico = Arc::clone(&amb.servico);
    esperar(h, "o lugar nas preferências", move |_| servico.prefs().lugares() == esperados);
}

#[test]
fn arrastar_a_aba_do_historico_para_a_doca_da_direita() {
    let amb = Ambiente::exemplo("docas_arrastar");
    let mut h = harness(&amb);
    assert_eq!(h.state().lugar_do_painel("Histórico"), Some("esquerda"));
    let de = rect_de(&h, Role::Tab, "Histórico").center();
    let doc = documento(&h);
    // solta sobre a doca da direita (à direita do documento)
    let para = egui::pos2(doc.right() + 60.0, doc.center().y);
    arrastar_e_soltar(&mut h, de, para);
    assert_eq!(h.state().lugar_do_painel("Histórico"), Some("direita"));
    assert!(painel_a_vista(&h, "Histórico"), "o painel movido fica à vista na doca nova");
    // a doca da direita mostra as abas com o nome; a da esquerda fica só com o Explorador
    assert!(aba_do_painel_ativa(&h, "Histórico"));
    assert!(existe(&h, Role::Tab, "Propriedades · exemplo.db"));
    assert!(painel_a_vista(&h, "Explorador"));
    esperar_lugares(
        &mut h,
        &amb,
        LugaresPaineis {
            explorador: Some(LugarPainel::Esquerda),
            historico: Some(LugarPainel::Direita),
            propriedades: Some(LugarPainel::Direita),
        },
    );
    capturar(&mut h, "docas-historico-a-direita-claro.png");
    // soltar no meio do documento não muda nada
    let de = rect_de(&h, Role::Tab, "Histórico").center();
    arrastar_e_soltar(&mut h, de, doc.center());
    assert_eq!(h.state().lugar_do_painel("Histórico"), Some("direita"));
}

#[test]
fn destacar_em_janela_prender_de_volta_e_fechar() {
    let amb = Ambiente::exemplo("docas_janela");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    // o Destacar da doca da direita leva as Propriedades para a janela própria
    let destacar = mais_a_direita(&h, "Destacar em janela");
    clicar_em(&mut h, destacar.center());
    h.run_steps(3);
    assentar(&mut h);
    assert_eq!(h.state().lugar_do_painel("Propriedades"), Some("janela"));
    assert!(painel_a_vista(&h, "Propriedades") && h.state().propriedades_a_vista());
    assert!(existe(&h, Role::Button, "Prender de volta"), "{:?}", textos(&h));
    // sem a doca da direita, o documento vai até a borda
    assert_eq!(documento(&h).right(), TAMANHO.x - 8.0);
    capturar(&mut h, "docas-propriedades-em-janela-claro.png");
    // o Prender de volta põe na doca de onde ele saiu, à vista
    clicar_botao(&mut h, "Prender de volta");
    assentar(&mut h);
    assert_eq!(h.state().lugar_do_painel("Propriedades"), Some("direita"));
    assert!(painel_a_vista(&h, "Propriedades"));
    // de novo para a janela, pelo menu Tabela, e o Fechar dela o devolve escondido
    abrir_menu(&mut h, "Tabela");
    clicar_item(&mut h, "Propriedades em janela própria");
    assentar(&mut h);
    assert_eq!(h.state().lugar_do_painel("Propriedades"), Some("janela"));
    let prender = rect_de(&h, Role::Button, "Prender de volta");
    let fechar = h
        .query_all(by().role(Role::Button).label("Fechar"))
        .map(|n| n.rect())
        .find(|r| (r.center().y - prender.center().y).abs() < 1.0 && r.left() > prender.left())
        .expect("o Fechar da janela própria");
    clicar_em(&mut h, fechar.center());
    h.run_steps(3);
    assentar(&mut h);
    assert_eq!(h.state().lugar_do_painel("Propriedades"), Some("direita"));
    assert!(!painel_a_vista(&h, "Propriedades"), "volta escondido");
    esperar_lugares(
        &mut h,
        &amb,
        LugaresPaineis {
            explorador: Some(LugarPainel::Esquerda),
            historico: Some(LugarPainel::Esquerda),
            propriedades: Some(LugarPainel::Direita),
        },
    );
}

#[test]
fn o_lugar_guardado_volta_na_proxima_abertura() {
    let amb = Ambiente::exemplo("docas_lugar_guardado");
    let lugares = LugaresPaineis { explorador: Some(LugarPainel::Direita), ..Default::default() };
    amb.servico.prefs().atualizar(prefs::Mudanca::lugares(lugares)).expect("gravar");
    let editor = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_lugares(amb.servico.prefs().lugares());
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    assentar(&mut h);
    assert_eq!(h.state().lugar_do_painel("Explorador"), Some("direita"));
    // a esquerda fica com o Histórico, que vira a aba à vista dela
    assert!(painel_a_vista(&h, "Histórico"));
    assert!(existe(&h, Role::Tab, "Explorador"));
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Estrutura de verdade (tela_esquema), com a lista de colunas")]
fn menu_tabela_vistas_e_copiar_o_nome() {
    let amb = Ambiente::exemplo("menu_tabela");
    let mut h = harness(&amb);
    // sem tabela à vista, sem o menu Tabela; com banco, o Banco
    assert!(existe(&h, Role::Button, "Banco"));
    assert!(!existe(&h, Role::Button, "Tabela"));
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    for (item, vista) in [("Colunas", "Colunas"), ("Índices e gatilhos", "Índices"), ("Definição SQL", "SQL")] {
        abrir_menu(&mut h, "Tabela");
        clicar_item(&mut h, item);
        assentar(&mut h);
        assert_eq!(h.state().vista_da_tabela(), Some(vista), "{item}");
        assert!(h.state().propriedades_a_vista());
    }
    // a vista Colunas na doca: a lista compacta, com a coluna clicada selecionada
    vista_da_tabela(&mut h, 2);
    assert!(textos_das_propriedades(&h).iter().any(|t| t == "email"), "a lista compacta das colunas");
    capturar(&mut h, "propriedades-vista-colunas-claro.png");
    abrir_menu(&mut h, "Tabela");
    clicar_item(&mut h, "Copiar o nome");
    esperar_texto(&mut h, "Nome copiado.");
    // Abrir na Consulta SQL
    abrir_menu(&mut h, "Tabela");
    clicar_item(&mut h, "Abrir na Consulta SQL");
    assentar(&mut h);
    assert!(matches!(h.state().rota(), Rota::Consulta { sql: Some(s) } if s.contains("clientes")));
}

#[test]
fn menu_banco_poe_a_visao_geral_a_vista_e_verifica() {
    let amb = Ambiente::exemplo("menu_banco");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    abrir_menu(&mut h, "Banco");
    clicar_item(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
    esperar(&mut h, "a verificação terminar", |h| !h.state().ocupado());
    // o Visão geral do menu e o Fechar o banco
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    abrir_menu(&mut h, "Banco");
    clicar_item(&mut h, "Visão geral");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
    abrir_menu(&mut h, "Banco");
    clicar_item(&mut h, "Fechar o banco");
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert!(!existe(&h, Role::Button, "Banco"), "sem banco, sem o menu");
}

#[test]
fn todas_as_abas_foca_a_escolhida() {
    let amb = Ambiente::exemplo("todas_as_abas");
    let mut h = harness(&amb);
    for t in ["clientes", "pedidos"] {
        h.state_mut().ir(Rota::objeto(t, false));
        assentar(&mut h);
    }
    clicar_botao(&mut h, "Todas as abas");
    h.run_steps(2);
    clicar(&mut h, Role::RadioButton, "clientes");
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "clientes");
}
