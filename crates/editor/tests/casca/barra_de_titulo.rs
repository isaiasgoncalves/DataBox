//! A barra de título: o título no centro, os botões da janela, o arrasto e o duplo clique no espaço livre, as
//! bordas, os controles com a janela modal aberta, a faixa sem a barra própria; e a tela parada e o toast.

use super::*;

/// Um ponto do espaço livre da barra de título: entre o menu Ajuda e o título.
fn ponto_livre(h: &H) -> egui::Pos2 {
    let ajuda = botao_menu(h, "Ajuda").rect();
    egui::pos2(ajuda.right() + 30.0, 15.0)
}

/// A janela maximizada ou não, como o eframe diria.
fn maximizada(h: &mut H, sim: bool) {
    h.input_mut().viewports.get_mut(&ViewportId::ROOT).expect("viewport").maximized = Some(sim);
    h.run_steps(2);
}

#[test]
fn titulo_no_centro_e_esmaecido_com_a_janela_inativa() {
    let amb = Ambiente::exemplo("titulo_centro");
    let mut h = harness(&amb);
    let titulo = titulo_na_barra(&h, "exemplo.db · DataBox").expect("o título na barra");
    assert!((titulo.center().x - TAMANHO.x / 2.0).abs() <= 1.0, "no centro da janela: {titulo:?}");
    assert!(titulo.left() > botao_menu(&h, "Ajuda").rect().right(), "depois dos menus");
    // a tinta do título contra o fundo da faixa: menos com a janela inativa
    let tinta = |h: &mut H| {
        h.remove_cursor();
        h.run_steps(2);
        let imagem = renderizar(h);
        let fundo = *imagem.get_pixel((titulo.left() - 6.0) as u32, titulo.center().y as u32);
        let mut soma = 0u64;
        for x in titulo.left() as u32..titulo.right() as u32 {
            for y in titulo.top() as u32..titulo.bottom() as u32 {
                let p = imagem.get_pixel(x, y);
                soma += (0..3).map(|i| u64::from(p[i].abs_diff(fundo[i]))).sum::<u64>();
            }
        }
        soma
    };
    let ativa = tinta(&mut h);
    h.input_mut().viewports.get_mut(&ViewportId::ROOT).expect("viewport").focused = Some(false);
    let inativa = tinta(&mut h);
    assert!(ativa > 0 && inativa * 10 < ativa * 8, "esmaece com a janela inativa: {ativa} → {inativa}");
    capturar(&mut h, "barra-titulo-inativa-claro.png");
}

#[test]
fn barra_de_titulo_minimiza_maximiza_e_fecha() {
    let amb = Ambiente::exemplo("barra_titulo_botoes");
    let mut h = harness(&amb);
    assert!(clicar_no(&mut h, Role::Button, "Minimizar").contains(&ViewportCommand::Minimized(true)));
    assert!(clicar_no(&mut h, Role::Button, "Maximizar").contains(&ViewportCommand::Maximized(true)));
    let regioes = editor::regioes_da_janela(&h.ctx).expect("as regiões publicadas");
    assert_eq!((regioes.borda, regioes.livre.len()), (5.0, 2));
    assert_eq!(regioes.botoes[2].max, egui::pos2(TAMANHO.x, 30.0), "o Fechar no canto");

    // maximizada: o do meio vira Restaurar e a borda some
    maximizada(&mut h, true);
    assert!(h.query(by().role(Role::Button).label("Maximizar")).is_none());
    let enviados = clicar_no(&mut h, Role::Button, "Restaurar");
    assert!(enviados.contains(&ViewportCommand::Maximized(false)), "{enviados:?}");
    assert_eq!(editor::regioes_da_janela(&h.ctx).map(|r| r.borda), Some(0.0));
    let perto_da_borda = egui::pos2(2.0, 450.0);
    let enviados = clicar_em(&mut h, perto_da_borda);
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::BeginResize(_))), "{enviados:?}");
    capturar(&mut h, "barra-titulo-maximizada-claro.png");
    maximizada(&mut h, false);
    assert!(existe(&h, Role::Button, "Maximizar"));

    // Fechar: o pedido de sempre (sem transação, o eframe fecha)
    let enviados = clicar_no(&mut h, Role::Button, "Fechar a janela");
    assert!(enviados.contains(&ViewportCommand::Close), "{enviados:?}");
    let dica = ler_dica(&mut h, Role::Button, "Fechar a janela");
    assert_eq!(dica, "Fechar");
}

#[test]
fn espaco_livre_arrasta_e_maximiza() {
    let amb = Ambiente::exemplo("barra_titulo_arrasto");
    let mut h = harness(&amb);
    let livre = ponto_livre(&h);
    // um clique só não arrasta nem maximiza
    let enviados = clicar_em(&mut h, livre);
    assert!(!enviados.contains(&ViewportCommand::StartDrag), "{enviados:?}");
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::Maximized(_))), "{enviados:?}");
    // o duplo clique maximiza (também no logo); maximizada, restaura
    let duplo = |p: egui::Pos2| [mover(p), apertar(p, true), apertar(p, false), apertar(p, true), apertar(p, false)];
    let enviados = comandos_dos_eventos(&mut h, &duplo(livre));
    assert!(enviados.contains(&ViewportCommand::Maximized(true)), "{enviados:?}");
    let enviados = comandos_dos_eventos(&mut h, &duplo(egui::pos2(18.0, 15.0)));
    assert!(enviados.contains(&ViewportCommand::Maximized(true)), "o logo: {enviados:?}");
    maximizada(&mut h, true);
    let enviados = comandos_dos_eventos(&mut h, &duplo(livre));
    assert!(enviados.contains(&ViewportCommand::Maximized(false)), "{enviados:?}");
    maximizada(&mut h, false);
    // arrastar 20px pelo espaço livre: o laço de arrasto do sistema, no começo do arrasto
    let adiante = livre + egui::vec2(20.0, 0.0);
    let arrasto = [mover(livre), apertar(livre, true), mover(livre + egui::vec2(10.0, 0.0)), mover(adiante)];
    let enviados = comandos_dos_eventos(&mut h, &arrasto);
    assert_eq!(enviados.iter().filter(|c| **c == ViewportCommand::StartDrag).count(), 1, "{enviados:?}");
    comandos_dos_eventos(&mut h, &[apertar(adiante, false)]);
    // arrastar a partir de um menu não arrasta a janela
    let menu = botao_menu(&h, "Editar").rect().center();
    let arrasto = [mover(menu), apertar(menu, true), mover(menu + egui::vec2(20.0, 0.0)), apertar(menu, false)];
    let enviados = comandos_dos_eventos(&mut h, &arrasto);
    assert!(!enviados.contains(&ViewportCommand::StartDrag), "{enviados:?}");
    // e o clique no menu ainda o abre (a borda de cima não come o meio dele)
    h.remove_cursor();
    h.run_steps(2);
    abrir_menu(&mut h, "Editar");
    assert!(menu_aberto(&h.ctx));
}

#[test]
fn bordas_redimensionam_a_janela() {
    let amb = Ambiente::exemplo("barra_titulo_bordas");
    let mut h = harness(&amb);
    for (pos, cursor, direcao) in [
        (egui::pos2(2.0, 450.0), egui::CursorIcon::ResizeWest, egui::ResizeDirection::West),
        (egui::pos2(1438.0, 898.0), egui::CursorIcon::ResizeSouthEast, egui::ResizeDirection::SouthEast),
        (egui::pos2(720.0, 2.0), egui::CursorIcon::ResizeNorth, egui::ResizeDirection::North),
        (egui::pos2(1439.0, 1.0), egui::CursorIcon::ResizeNorthEast, egui::ResizeDirection::NorthEast),
    ] {
        h.hover_at(pos);
        h.run_steps(2);
        assert_eq!(h.output().platform_output.cursor_icon, cursor, "o cursor em {pos:?}");
        // o redimensionar começa no aperto, sem esperar o arrasto
        let enviados = comandos_dos_eventos(&mut h, &[apertar(pos, true)]);
        assert!(enviados.contains(&ViewportCommand::BeginResize(direcao)), "{pos:?}: {enviados:?}");
        comandos_dos_eventos(&mut h, &[apertar(pos, false)]);
        h.remove_cursor();
        h.run_steps(2);
    }
    // a 6px da borda, já é o anel do fundo: nada
    h.hover_at(egui::pos2(6.0, 450.0));
    h.run_steps(2);
    assert_ne!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeWest);
}

#[test]
fn controles_da_janela_respondem_com_a_janela_modal_aberta() {
    let amb = Ambiente::exemplo("barra_titulo_modal");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    assert!(h.state().exportacao_aberta().is_some(), "a janela Exportar aberta");
    // os botões da janela ficam por cima do véu da modal
    assert!(clicar_no(&mut h, Role::Button, "Minimizar").contains(&ViewportCommand::Minimized(true)));
    let p = ponto_livre(&h);
    let arrasto = [mover(p), apertar(p, true), mover(p + egui::vec2(10.0, 0.0)), mover(p + egui::vec2(20.0, 0.0))];
    let enviados = comandos_dos_eventos(&mut h, &arrasto);
    assert!(enviados.contains(&ViewportCommand::StartDrag), "arrastar com a modal aberta: {enviados:?}");
    h.remove_cursor();
    h.run_steps(2);
    assert!(h.state().exportacao_aberta().is_some(), "a janela continua aberta");
    // os menus ficam sob o véu
    clicar_primeiro(&mut h, Role::Button, "Arquivo");
    assert!(!menu_aberto(&h.ctx));
}

#[test]
fn sem_a_barra_propria_a_faixa_nao_tem_os_controles() {
    let amb = Ambiente::exemplo("sem_barra_propria");
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_barra_propria(false);
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    for rotulo in ["Minimizar", "Maximizar", "Fechar a janela"] {
        assert!(h.query(by().role(Role::Button).label(rotulo)).is_none(), "{rotulo}");
    }
    assert!(editor::regioes_da_janela(&h.ctx).is_none());
    assert!(titulo_na_barra(&h, "exemplo.db · DataBox").is_some());
    let enviados = clicar_em(&mut h, egui::pos2(2.0, 450.0));
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::BeginResize(_))), "{enviados:?}");
    abrir_menu_arquivo(&mut h);
    assert!(menu_aberto(&h.ctx), "os menus continuam");
}

#[test]
fn parada_a_tela_nao_repinta_sem_parar() {
    let amb = Ambiente::exemplo("parada");
    let mut h = harness(&amb);
    for rota in [Rota::Banco, Rota::Inicio, Rota::objeto("clientes", false)] {
        h.state_mut().ir(rota);
        assentar(&mut h);
        h.try_run().expect("a tela para de pedir quadros");
        let espera = h.output().viewport_output.get(&ViewportId::ROOT).map(|v| v.repaint_delay);
        assert!(espera.is_some_and(|d| d >= std::time::Duration::from_millis(100)), "{espera:?}");
    }
}

#[test]
fn toast_captura() {
    let amb = Ambiente::exemplo("toast");
    let mut h = harness(&amb);
    toast(&h.ctx, "Linha salva.");
    h.run_steps(3);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Linha salva."));
    capturar(&mut h, "toast-claro.png");
}
