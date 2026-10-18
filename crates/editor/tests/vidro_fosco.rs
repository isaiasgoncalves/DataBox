//! O vidro fosco ([`interface::fosco`]) na casca inteira: o `egui_kittest` atende o `ViewportCommand::Screenshot`
//! com a renderização do próprio quadro (o eframe, com a do quadro seguinte), então o caminho é o mesmo do app. As
//! capturas `vidro-fosco-{escuro,claro}.png` (em `capturas/rust/`) mostram um menu sobre a Visão geral, para ver se o
//! texto de trás some e o do menu continua nítido.

mod apoio;

use apoio::*;
use editor::Partida;
use estado::apoio_testes::Ambiente;
use interface::fosco::{self, Resumo};
use interface::tema::Tema;

fn resumo(h: &H) -> Resumo {
    fosco::resumo(&h.ctx).expect("o fosco ligado")
}

/// A Visão geral do banco de exemplo, com o fosco ligado e nenhuma camada aberta.
fn visao_geral(nome: &str, tema: Tema) -> (Ambiente, H) {
    let amb = Ambiente::exemplo(nome);
    let mut h = harness_com(&amb, tema, TAMANHO, Partida::default(), |_| None);
    fosco::ligar(&h.ctx, None);
    assentar(&mut h);
    assert!(fosco::ligado(&h.ctx));
    assert_eq!(rotulos_das_abas(&h).1, "exemplo.db · Visão geral");
    assert_eq!(resumo(&h), Resumo::default(), "sem camada aberta, nada de foto");
    (amb, h)
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "a cena precisa das telas de verdade")]
fn o_menu_arquivo_sobre_a_visao_geral_escura() {
    let (_amb, mut h) = visao_geral("fosco_escuro", Tema::Escuro);
    abrir_menu_arquivo(&mut h);
    h.run_steps(2);
    let r = resumo(&h);
    assert_eq!((r.fotos, r.fundos, r.com_fundo, r.esperando, r.lisas), (1, 1, 1, 0, 0), "{r:?}");
    // fechado o menu, a foto (tirada sem camada nenhuma à vista) fica um pouco, para o menu seguinte (C10): nenhuma
    // camada com fundo, e abrir outro logo em seguida não pede outra foto de tela cheia (o passo do harness é de
    // 0,25 s e o fundo serve por 2 s: poucos quadros entre um e outro)
    h.key_press(egui::Key::Escape);
    h.run_steps(1);
    assert!(!editor::menu_aberto(&h.ctx), "o menu fechou");
    let r = resumo(&h);
    assert_eq!((r.fundos, r.com_fundo), (1, 0), "{r:?}");
    abrir_menu_arquivo(&mut h);
    h.run_steps(1);
    assert!(editor::menu_aberto(&h.ctx));
    let r = resumo(&h);
    assert_eq!((r.fotos, r.fundos, r.com_fundo, r.esperando), (1, 1, 1, 0), "o mesmo fundo serviu: {r:?}");
    capturar(&mut h, "vidro-fosco-escuro.png");
    assert_eq!(resumo(&h).fotos, 1, "com o menu parado, nenhuma foto a mais");

    // fechado de vez, passados os 2 s o fundo guardado também some
    h.key_press(egui::Key::Escape);
    h.run_steps(12);
    assert!(!editor::menu_aberto(&h.ctx));
    assert_eq!(resumo(&h).fundos, 0, "sem camada, o fundo velho vai embora");
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "a cena precisa das telas de verdade")]
fn o_exibir_tema_sobre_a_visao_geral_clara() {
    let (_amb, mut h) = visao_geral("fosco_claro", Tema::Claro);
    abrir_menu(&mut h, "Exibir");
    abrir_submenu(&mut h, "Tema");
    h.run_steps(2);
    let r = resumo(&h);
    assert_eq!((r.fotos, r.com_fundo, r.esperando), (1, 2, 0), "{r:?}");
    capturar(&mut h, "vidro-fosco-claro.png");

    // o tema que muda com o menu aberto (o do Windows, no "Sistema") refaz a foto: o fundo era o do tema claro
    h.ctx.set_theme(egui::ThemePreference::Dark);
    h.run_steps(3);
    assert!(editor::menu_aberto(&h.ctx));
    let r = resumo(&h);
    assert_eq!((r.fotos, r.fundos, r.com_fundo, r.esperando), (2, 1, 2, 0), "{r:?}");
}

/// A dica do próprio egui (o `window_fill`) fica no vidro liso, a 96%: nenhuma foto de tela cheia a cada dica (C10).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "a cena precisa das telas de verdade")]
fn a_dica_do_egui_fica_no_vidro_liso_sem_foto() {
    use egui_kittest::kittest::{Queryable, by};
    let (_amb, mut h) = visao_geral("fosco_dica", Tema::Escuro);
    h.get(by().role(egui::accesskit::Role::Label).label("Gravação imediata")).hover();
    h.run_steps(3);
    let r = resumo(&h);
    assert_eq!(r, Resumo::default(), "a dica não pede foto nem leva fundo: {r:?}");
    // com o ponteiro em cima (a dica some sem ele)
    let imagem = renderizar(&mut h);
    imagem.save(estado::apoio_testes::pasta_capturas().join("vidro-fosco-dica-escuro.png")).expect("gravar png");
    h.remove_cursor();
    h.run_steps(3);
    assert_eq!(resumo(&h), Resumo::default(), "a dica sumiu sem deixar nada");
}

/// A janela (Atalhos do teclado) com o fosco ligado: o modal e o vidro da janela com o fundo desfocado embaixo.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "a cena precisa das telas de verdade")]
fn a_janela_atalhos_no_fosco_escuro_e_claro() {
    for (tema, nome) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let (_amb, mut h) = visao_geral(&format!("fosco_janela_{nome}"), tema);
        abrir_menu(&mut h, "Ajuda");
        clicar_item(&mut h, "Atalhos do teclado");
        h.run_steps(3);
        assert!(h.state().janela_aberta(), "a janela abriu");
        let r = resumo(&h);
        assert!(r.fotos >= 1 && r.com_fundo >= 1 && r.esperando == 0 && r.lisas == 0, "{r:?}");
        capturar(&mut h, &format!("vidro-fosco-janela-{nome}.png"));
    }
}

/// O popover (a condição de filtro dos Dados, preso embaixo do "+ Condição") com o fosco ligado.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "a cena precisa das telas de verdade")]
fn o_popover_da_condicao_no_fosco_escuro_e_claro() {
    for (tema, nome) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let (_amb, mut h) = visao_geral(&format!("fosco_popover_{nome}"), tema);
        h.state_mut().ir(estado::Rota::objeto("clientes", false));
        assentar(&mut h);
        esperar_texto(&mut h, "Linhas 1–60 de 60");
        assert_eq!(resumo(&h), Resumo::default(), "sem camada aberta");
        clicar_botao(&mut h, "Nova condição");
        h.run_steps(3);
        let r = resumo(&h);
        assert!(r.fotos >= 1 && r.com_fundo >= 1 && r.esperando == 0 && r.lisas == 0, "{r:?}");
        capturar(&mut h, &format!("vidro-fosco-popover-{nome}.png"));
    }
}

/// O aviso flutuante (toast) com o fosco ligado, sobre a Visão geral.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "a cena precisa das telas de verdade")]
fn o_toast_no_fosco_escuro_e_claro() {
    for (tema, nome) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let (_amb, mut h) = visao_geral(&format!("fosco_toast_{nome}"), tema);
        interface::pecas::toast::toast_concluido(&h.ctx, "Copiado.");
        h.run_steps(3);
        let r = resumo(&h);
        assert!(r.fotos >= 1 && r.com_fundo >= 1 && r.esperando == 0 && r.lisas == 0, "{r:?}");
        assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("Copiado."));
        capturar(&mut h, &format!("vidro-fosco-toast-{nome}.png"));
    }
}
