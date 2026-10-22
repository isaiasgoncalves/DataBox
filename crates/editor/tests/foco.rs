//! O foco do teclado na casca (as correções das revisões da etapa 2): o Tab só para em peças com nome e segue a
//! ordem da tela (a barra de título, a barra de ferramentas, a barra de ícones, os painéis e o documento), as setas
//! do menu aberto ficam no painel dele e as do Explorador ficam na árvore, e o Esc do menu devolve o foco ao botão. O
//! Shift+F10 na grade abre o menu da célula com o foco no primeiro item, e o Esc devolve o foco à grade.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;

/// O nó com o foco: o papel, o rótulo e onde ele está. None quando nada tem o foco ou o foco está num widget sem nó
/// de acessibilidade.
fn focado(h: &H) -> Option<(Role, Option<String>, egui::Rect)> {
    h.query_all_by(|n| n.is_focused()).next().map(|n| {
        let a = n.accesskit_node();
        (a.role(), a.label(), n.rect())
    })
}

/// O rótulo do nó com o foco; falha se o foco está num widget sem nome (uma área invisível que entrou no Tab).
fn rotulo_focado(h: &H, depois_de: &str) -> String {
    let id = h.ctx.memory(|m| m.focused());
    match focado(h) {
        Some((_, Some(rotulo), _)) if !rotulo.trim().is_empty() => rotulo,
        outro => panic!("depois de {depois_de}, o foco está num widget sem nome: {id:?} {outro:?}"),
    }
}

/// Aperta o Tab até o foco dar a volta e devolve os rótulos, na ordem (cada parada tem de ter nome).
fn paradas_do_tab(h: &mut H, maximo: usize) -> Vec<String> {
    let mut paradas: Vec<String> = Vec::new();
    for i in 0..maximo {
        h.key_press(Key::Tab);
        h.run_steps(1);
        let rotulo = rotulo_focado(h, &format!("{} Tabs ({paradas:?})", i + 1));
        let id = h.ctx.memory(|m| m.focused());
        if i > 0 && paradas.first() == Some(&rotulo) && id.is_some() {
            return paradas;
        }
        paradas.push(rotulo);
    }
    panic!("o Tab não deu a volta em {maximo} paradas: {paradas:?}");
}

#[test]
fn tab_so_para_em_pecas_com_nome_e_na_ordem_da_casca() {
    // antes: o espaço livre da barra de título, as quatro faixas de redimensionar e a seta de cada nó do Explorador
    // eram paradas do Tab sem nome, e a barra de ícones vinha antes da barra de ferramentas
    let amb = Ambiente::exemplo("foco_tab_casca");
    let mut h = harness(&amb);
    h.remove_cursor();
    h.run_steps(2);
    let paradas = paradas_do_tab(&mut h, 200);
    let posicao = |rotulo: &str| {
        paradas.iter().position(|p| p == rotulo).unwrap_or_else(|| panic!("{rotulo} não está no Tab: {paradas:?}"))
    };
    // a barra de título, a barra de ferramentas, a doca da esquerda (as abas dela e o Explorador), nesta ordem
    assert_eq!(paradas.first().map(String::as_str), Some("Arquivo"));
    assert!(posicao("Ajuda") < posicao("Abrir arquivo"), "{paradas:?}");
    assert!(posicao("Painel de propriedades") < posicao("Explorador"), "a barra de ferramentas antes da doca");
    assert!(posicao("Explorador") < posicao("Histórico") && posicao("Histórico") < posicao("exemplo.db"));
    // na árvore, um nó depois do outro (sem a seta no meio)
    assert_eq!(posicao("Tabelas"), posicao("exemplo.db") + 1, "{paradas:?}");
    assert_eq!(posicao("clientes"), posicao("Tabelas") + 1, "{paradas:?}");
    // os botões da janela no fim, sem as áreas de arrastar e de redimensionar
    let fim = &paradas[paradas.len() - 3..];
    assert_eq!(fim, ["Minimizar", "Maximizar", "Fechar a janela"], "{paradas:?}");
}

#[test]
fn tab_com_a_exportar_aberta_fica_na_janela_e_nos_botoes_da_janela() {
    let amb = Ambiente::exemplo("foco_tab_modal");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    h.remove_cursor();
    h.run_steps(3);
    assert!(h.state().exportacao_aberta().is_some());
    let paradas = paradas_do_tab(&mut h, 60);
    // nada de baixo do véu (os menus, as barras, o Explorador) e nada sem nome
    for de_baixo in ["Arquivo", "Editar", "Exibir", "Abrir arquivo", "Explorador", "exemplo.db"] {
        assert!(!paradas.iter().any(|p| p == de_baixo), "{de_baixo} sob o véu entrou no Tab: {paradas:?}");
    }
    // os botões da janela continuam (ficam acima da modal)
    for botao in ["Minimizar", "Maximizar", "Fechar a janela"] {
        assert!(paradas.iter().any(|p| p == botao), "{botao}: {paradas:?}");
    }
}

#[test]
fn setas_no_explorador_ficam_na_arvore() {
    let amb = Ambiente::exemplo("foco_setas_arvore");
    let mut h = harness(&amb);
    h.remove_cursor();
    h.run_steps(2);
    // o Tab até o nó Tabelas do Explorador (aberto no começo)
    let mut achou = false;
    for _ in 0..80 {
        h.key_press(Key::Tab);
        h.run_steps(1);
        if focado(&h).is_some_and(|(papel, rotulo, r)| {
            papel == Role::Link && rotulo.as_deref() == Some("Tabelas") && r.right() < 330.0
        }) {
            achou = true;
            break;
        }
    }
    assert!(achou, "o Tab não chegou ao nó Tabelas");
    // ← fecha o nó e o foco fica nele (antes, ia para o vizinho)
    h.key_press(Key::ArrowLeft);
    h.run_steps(2);
    assert!(!existe(&h, Role::Link, "clientes"), "o ← fechou o nó");
    assert_eq!(rotulo_focado(&h, "o ←"), "Tabelas");
    // → abre e o foco também fica (antes, ia para o "i" do documento, à direita)
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    assert!(existe(&h, Role::Link, "clientes"), "o → abriu o nó");
    assert_eq!(rotulo_focado(&h, "o →"), "Tabelas");
    // ↓ desce pela árvore e, no fim dela, não cai numa faixa de redimensionar sem nome
    for i in 0..12 {
        h.key_press(Key::ArrowDown);
        h.run_steps(1);
        rotulo_focado(&h, &format!("{} ↓", i + 1));
    }
}

#[test]
fn setas_do_menu_ficam_no_painel_e_o_esc_devolve_o_foco() {
    let amb = Ambiente::exemplo("foco_menu");
    let mut h = harness(&amb);
    h.remove_cursor();
    h.run_steps(2);
    h.key_press(Key::Tab);
    h.run_steps(1);
    assert_eq!(rotulo_focado(&h, "o primeiro Tab"), "Arquivo");
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(editor::menu_aberto(&h.ctx));
    // os itens ligados do painel, de cima para baixo
    let mut ligados: Vec<(f32, String)> = h
        .query_all_by(|n| matches!(n.role(), Role::Button | Role::CheckBox) && !n.is_disabled())
        .filter(|n| n.rect().width() > 150.0 && n.rect().top() > 30.0 && n.rect().left() < 330.0)
        .filter_map(|n| Some((n.rect().top(), n.accesskit_node().label()?)))
        .collect();
    ligados.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ligados: Vec<String> = ligados.into_iter().map(|(_, r)| r).collect();
    // o Abrir pasta ligou na etapa 4 (passo 11)
    assert!(ligados.len() >= 3 && ligados[0] == "Abrir pasta", "{ligados:?}");
    // ↓ do botão vai ao primeiro e anda pelos ligados, com volta (antes, o terceiro ↓ caía no Explorador, embaixo)
    for (i, esperado) in ligados.iter().chain(ligados.first()).enumerate() {
        h.key_press(Key::ArrowDown);
        h.run_steps(1);
        assert!(editor::menu_aberto(&h.ctx), "o menu fechou no {}º ↓", i + 1);
        assert_eq!(&rotulo_focado(&h, &format!("{} ↓", i + 1)), esperado, "{ligados:?}");
    }
    // ↑ volta, também com volta
    h.key_press(Key::ArrowUp);
    h.run_steps(1);
    assert_eq!(rotulo_focado(&h, "o ↑"), ligados[ligados.len() - 1]);
    // o Esc fecha o menu e devolve o foco ao botão (antes, o foco sumia)
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!editor::menu_aberto(&h.ctx));
    assert_eq!(rotulo_focado(&h, "o Esc"), "Arquivo");
    assert!(botao_menu(&h, "Arquivo").is_focused());
    h.key_press(Key::Tab);
    h.run_steps(1);
    assert_eq!(rotulo_focado(&h, "o Tab depois do Esc"), "Editar");
    // com o menu aberto pelo clique, o ↑ do botão vai ao último
    clicar_botao(&mut h, "Exibir");
    assert!(editor::menu_aberto(&h.ctx));
    botao_menu(&h, "Exibir").focus();
    h.run_steps(1);
    h.key_press(Key::ArrowUp);
    h.run_steps(1);
    assert_eq!(rotulo_focado(&h, "o ↑ no Exibir"), "Tema");
    assert!(h.get(by().role(Role::Button).label("Tema")).is_focused());
}

#[cfg(feature = "telas")]
#[test]
fn shift_f10_na_grade_abre_o_menu_da_celula_e_o_esc_volta_a_ela() {
    let amb = Ambiente::exemplo("foco_menu_da_celula");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    let grade = h.get(by().label("Grade")).rect();
    // o corpo da grade, depois da caixa de marcar (32)
    let pos = egui::pos2(grade.left() + 32.0 + 60.0, grade.top() + 14.0);
    h.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        let button = egui::PointerButton::Primary;
        h.event(egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(2);
    assert_eq!(rotulo_focado(&h, "o clique na célula"), "Grade");
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    h.run_steps(3);
    assert!(h.query_by_role(Role::Menu).is_some(), "o Shift+F10 abre o menu da célula");
    assert_eq!(rotulo_focado(&h, "o Shift+F10"), "Copiar", "o foco no primeiro item");
    // as setas andam pelos itens, sem sair do menu
    h.key_press(Key::ArrowDown);
    h.run_steps(1);
    assert_eq!(rotulo_focado(&h, "o ↓"), "Copiar com cabeçalho");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.query_by_role(Role::Menu).is_none(), "o Esc fecha");
    assert_eq!(rotulo_focado(&h, "o Esc"), "Grade", "e devolve o foco à grade");
}
