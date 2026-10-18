//! As divisórias dos painéis laterais (o Explorador ou o Histórico à esquerda, as Propriedades à direita): arrastar muda
//! a largura ao vivo dentro dos limites (200 pt, 45% da janela e o documento de 560 pt), o duplo clique volta à largura
//! padrão, as setas mudam 16 pt por vez, a escolha vai para as preferências ao fim do gesto (a chave `"larguras_paineis"`) e
//! volta na próxima abertura, e a janela estreita encolhe os painéis antes de esconder as Propriedades. As capturas
//! vão para `capturas/rust/paineis-redimensionados-*.png`.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;

use apoio::*;
use editor::{Editor, Partida};
use egui::accesskit::{Action, ActionRequest, Role};
use egui::{Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::{Ambiente, pasta_capturas};
use interface::tema::Tema;
use prefs::LarguraPaineis;

const EXPLORADOR: &str = "Largura do Explorador";
const PROPRIEDADES: &str = "Largura das Propriedades";

/// O editor deste ambiente neste tamanho, com as larguras guardadas nas preferências dele (como o executável faz).
fn harness_com_prefs(amb: &Ambiente, tema: Tema, tamanho: egui::Vec2) -> H {
    let editor =
        Editor::novo(Arc::clone(&amb.servico), tema, Partida::default()).com_larguras(amb.servico.prefs().paineis());
    harness_editor(editor, tema, tamanho)
}

/// O vão (a alça) de uma divisória.
fn vao(h: &H, rotulo: &str) -> Rect {
    rect_de(h, Role::Splitter, rotulo)
}

fn desenhadas(h: &H) -> (f32, f32) {
    h.state().larguras_desenhadas().expect("um quadro desenhado")
}

fn mover(pos: Pos2) -> egui::Event {
    egui::Event::PointerMoved(pos)
}

fn botao(pos: Pos2, apertado: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: apertado, modifiers: Modifiers::NONE }
}

/// Manda os eventos, um por quadro a 10 ms um do outro (o passo do harness é de 0,25 s e o duplo clique tem de caber
/// em 0,3 s), e dá mais três quadros no passo normal.
fn mandar(h: &mut H, eventos: &[egui::Event]) {
    let inicio = h.ctx.input(|i| i.time);
    for (n, e) in eventos.iter().enumerate() {
        h.input_mut().time = Some(inicio + 0.01 * (n + 1) as f64);
        h.event(e.clone());
        h.step();
    }
    h.input_mut().time = None;
    h.run_steps(3);
}

/// Aperta o botão em `de`, anda até `para` em quatro passos e fica lá com o botão apertado.
fn arrastar_ate(h: &mut H, de: Pos2, para: Pos2) {
    h.hover_at(de);
    h.run_steps(2);
    h.event(botao(de, true));
    h.run_steps(1);
    for t in 1..=4 {
        h.hover_at(de + (para - de) * (t as f32 / 4.0));
        h.run_steps(2);
    }
}

fn soltar(h: &mut H, pos: Pos2) {
    h.event(botao(pos, false));
    h.run_steps(3);
}

/// Arrasta a alça `rotulo` `dx` pontos para a direita (para a esquerda, negativo) e solta; espera a gravação.
fn arrastar(h: &mut H, rotulo: &str, dx: f32) {
    let de = vao(h, rotulo).center();
    arrastar_ate(h, de, de + vec2(dx, 0.0));
    soltar(h, de + vec2(dx, 0.0));
    assentar(h);
    h.remove_cursor();
    h.run_steps(2);
}

fn duplo_clique(h: &mut H, rotulo: &str) {
    let p = vao(h, rotulo).center();
    mandar(h, &[mover(p), botao(p, true), botao(p, false), botao(p, true), botao(p, false)]);
    assentar(h);
}

fn texto_das_prefs(amb: &Ambiente) -> String {
    std::fs::read_to_string(amb.pasta().join("preferencias.json")).unwrap_or_default()
}

fn prefs_lidas(amb: &Ambiente) -> LarguraPaineis {
    prefs::Preferencias::abrir(Some(amb.pasta().join("preferencias.json"))).paineis()
}

fn valor(h: &H, rotulo: &str) -> (f64, f64, f64) {
    let no = h.get(by().role(Role::Splitter).label(rotulo));
    let n = no.accesskit_node();
    (
        n.numeric_value().expect("o valor"),
        n.min_numeric_value().expect("o mínimo"),
        n.max_numeric_value().expect("o máximo"),
    )
}

#[test]
fn as_divisorias_ficam_no_vao_com_nome_valor_e_limites() {
    let amb = Ambiente::exemplo("paineis_vao");
    let h = harness(&amb);
    // 8 + a barra de ícones (52) + 8 + o Explorador (258) + 8 e o documento até as Propriedades (272 + 8)
    assert_eq!(desenhadas(&h), (258.0, 272.0));
    let doc = documento(&h);
    assert_eq!((doc.left(), doc.right()), (334.0, 1152.0));
    let e = vao(&h, EXPLORADOR);
    let p = vao(&h, PROPRIEDADES);
    assert_eq!((e.left(), e.right()), (326.0, 334.0), "o vão entre o Explorador e o documento");
    assert_eq!((p.left(), p.right()), (1152.0, 1160.0), "o vão entre o documento e as Propriedades");
    assert_eq!(e.top(), doc.top() - 34.0, "do alto ao pé do corpo");
    // o valor em pontos, o mínimo de 200 e o máximo (o documento mínimo de 560 desconta o outro painel)
    assert_eq!(valor(&h, EXPLORADOR), (258.0, 200.0, 516.0));
    assert_eq!(valor(&h, PROPRIEDADES), (272.0, 200.0, 530.0));
    // o leitor de tela ouve o papel de divisória e o valor
    let no = h.get(by().role(Role::Splitter).label(EXPLORADOR));
    assert_eq!(no.accesskit_node().value().as_deref(), Some("258 pontos"));
    // nada foi gravado: sem arrastar, o arquivo é o de antes (a chave só nasce da primeira escolha)
    assert!(!texto_das_prefs(&amb).contains("larguras_paineis"));
    assert_eq!(prefs_lidas(&amb), LarguraPaineis::default());
}

#[test]
fn arrastar_o_explorador_muda_a_largura_ao_vivo_e_grava_ao_soltar() {
    let amb = Ambiente::exemplo("paineis_arrastar_explorador");
    let mut h = harness(&amb);
    let de = vao(&h, EXPLORADOR).center();
    arrastar_ate(&mut h, de, de + vec2(40.0, 0.0));
    // ao vivo: o documento já anda com o ponteiro, e nada foi gravado ainda
    assert_eq!(desenhadas(&h), (298.0, 272.0));
    assert_eq!(documento(&h).left(), 374.0);
    assert_eq!(vao(&h, EXPLORADOR).center().x, 374.0 - 4.0);
    assert!(!texto_das_prefs(&amb).contains("larguras_paineis"), "gravar a cada quadro do arrasto seria demais");
    assert_eq!(h.state().larguras_escolhidas(), (298.0, 272.0));
    soltar(&mut h, de + vec2(40.0, 0.0));
    assentar(&mut h);
    // ao soltar, só a que mudou vai para o arquivo
    assert_eq!(prefs_lidas(&amb), LarguraPaineis { explorador: Some(298.0), propriedades: None });
    assert!(texto_das_prefs(&amb).contains(r#""explorador": 298"#), "{}", texto_das_prefs(&amb));
    // as Propriedades não se mexeram
    assert_eq!(documento(&h).right(), 1152.0);
}

#[test]
fn arrastar_as_propriedades_para_a_esquerda_alarga_o_painel() {
    let amb = Ambiente::exemplo("paineis_arrastar_propriedades");
    let mut h = harness(&amb);
    arrastar(&mut h, PROPRIEDADES, -40.0);
    assert_eq!(desenhadas(&h), (258.0, 312.0));
    // o documento termina 8px antes do painel, que continua encostado na borda direita (8px da janela)
    assert_eq!(documento(&h).right(), 1440.0 - 8.0 - 312.0 - 8.0);
    assert_eq!(vao(&h, PROPRIEDADES).right(), 1440.0 - 8.0 - 312.0);
    assert_eq!(prefs_lidas(&amb), LarguraPaineis { explorador: None, propriedades: Some(312.0) });
    // e para a direita estreita
    arrastar(&mut h, PROPRIEDADES, 100.0);
    assert_eq!(desenhadas(&h).1, 212.0);
    assert_eq!(prefs_lidas(&amb).propriedades, Some(212.0));
}

#[test]
fn a_largura_fica_entre_o_minimo_e_o_maximo() {
    let amb = Ambiente::exemplo("paineis_limites");
    let mut h = harness(&amb);
    // o máximo do Explorador a 1440: 788 (o que cabe sem o documento ficar abaixo de 560) menos as Propriedades
    arrastar(&mut h, EXPLORADOR, 2000.0);
    assert_eq!(desenhadas(&h), (516.0, 272.0));
    assert_eq!(documento(&h).width(), 1152.0 - (68.0 + 516.0 + 8.0), "o documento ficou com o mínimo de 560");
    assert_eq!(documento(&h).width(), 560.0);
    arrastar(&mut h, EXPLORADOR, -2000.0);
    assert_eq!(desenhadas(&h).0, 200.0);
    assert_eq!(prefs_lidas(&amb).explorador, Some(200.0));
    // as Propriedades: o mesmo, de 200 ao que sobra com o Explorador de 200 (788 − 200)
    arrastar(&mut h, PROPRIEDADES, -2000.0);
    assert_eq!(desenhadas(&h), (200.0, 588.0));
    assert_eq!(documento(&h).width(), 560.0);
    arrastar(&mut h, PROPRIEDADES, 2000.0);
    assert_eq!(desenhadas(&h), (200.0, 200.0));
    // a divisória diz o intervalo de agora: o máximo de um depende do outro
    assert_eq!(valor(&h, EXPLORADOR), (200.0, 200.0, 588.0));
    assert_eq!(valor(&h, PROPRIEDADES), (200.0, 200.0, 588.0));
    // numa janela larga o teto é de 45% da janela
    let amb = Ambiente::exemplo("paineis_limites_larga");
    let mut h = harness_com(&amb, Tema::Claro, vec2(2400.0, 900.0), Partida::default(), |_| None);
    arrastar(&mut h, EXPLORADOR, 3000.0);
    assert_eq!(desenhadas(&h).0, 1080.0, "45% de 2400");
}

#[test]
fn duplo_clique_volta_a_largura_padrao() {
    let amb = Ambiente::exemplo("paineis_duplo_clique");
    let mut h = harness(&amb);
    arrastar(&mut h, EXPLORADOR, 60.0);
    arrastar(&mut h, PROPRIEDADES, 50.0);
    assert_eq!(desenhadas(&h), (318.0, 222.0));
    assert_eq!(prefs_lidas(&amb), LarguraPaineis { explorador: Some(318.0), propriedades: Some(222.0) });
    duplo_clique(&mut h, EXPLORADOR);
    assert_eq!(desenhadas(&h), (258.0, 222.0));
    duplo_clique(&mut h, PROPRIEDADES);
    assert_eq!(desenhadas(&h), (258.0, 272.0));
    // a escolha de volta ao padrão também vai para o arquivo
    assert_eq!(prefs_lidas(&amb), LarguraPaineis { explorador: Some(258.0), propriedades: Some(272.0) });
    // um duplo clique com o painel já no padrão não grava nada de novo
    let antes = texto_das_prefs(&amb);
    duplo_clique(&mut h, EXPLORADOR);
    assert_eq!(texto_das_prefs(&amb), antes);
}

#[test]
fn setas_mudam_16_pontos_e_gravam() {
    let amb = Ambiente::exemplo("paineis_teclado");
    let mut h = harness(&amb);
    h.remove_cursor();
    h.get(by().role(Role::Splitter).label(EXPLORADOR)).focus();
    h.run_steps(2);
    // a barra anda para onde a seta aponta: → alarga o Explorador
    h.key_press(Key::ArrowRight);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).0, 274.0);
    h.key_press(Key::ArrowRight);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).0, 290.0);
    h.key_press(Key::ArrowLeft);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).0, 274.0);
    assert_eq!(documento(&h).left(), 68.0 + 274.0 + 8.0);
    assert_eq!(valor(&h, EXPLORADOR).0, 274.0);
    // o foco ficou na divisória (as setas não o levaram ao vizinho)
    assert!(h.get(by().role(Role::Splitter).label(EXPLORADOR)).is_focused());
    assentar_e_conferir(&mut h, &amb, Some(274.0), None);
    // Home e End vão ao mínimo e ao máximo; Enter volta ao padrão
    h.key_press(Key::Home);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).0, 200.0);
    h.key_press(Key::End);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).0, 516.0);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).0, 258.0);
    assentar_e_conferir(&mut h, &amb, Some(258.0), None);

    // as Propriedades: ← alarga (a barra anda para a esquerda), → estreita
    h.get(by().role(Role::Splitter).label(PROPRIEDADES)).focus();
    h.run_steps(2);
    h.key_press(Key::ArrowLeft);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).1, 288.0);
    h.key_press(Key::ArrowRight);
    h.key_press(Key::ArrowRight);
    h.run_steps(3);
    assert_eq!(desenhadas(&h).1, 256.0);
    assert_eq!(documento(&h).right(), 1440.0 - 8.0 - 256.0 - 8.0);
    assentar_e_conferir(&mut h, &amb, Some(258.0), Some(256.0));
    // no limite a seta não passa
    h.key_press(Key::Home);
    h.run_steps(3);
    for _ in 0..3 {
        h.key_press(Key::ArrowRight);
        h.run_steps(2);
    }
    assert_eq!(desenhadas(&h).1, 200.0);
}

fn assentar_e_conferir(h: &mut H, amb: &Ambiente, explorador: Option<f32>, propriedades: Option<f32>) {
    assentar(h);
    let lidas = prefs_lidas(amb);
    assert_eq!(lidas.explorador, explorador);
    assert_eq!(lidas.propriedades, propriedades);
}

/// Pede ao nó `rotulo` uma ação do AccessKit (o que um leitor de tela faz).
fn pedir_acao(h: &mut H, rotulo: &str, acao: Action) {
    let (target_node, target_tree) = h.get(by().role(Role::Splitter).label(rotulo)).accesskit_node().locate();
    h.event(egui::Event::AccessKitActionRequest(ActionRequest { action: acao, target_node, target_tree, data: None }));
    h.run_steps(3);
}

#[test]
fn o_leitor_de_tela_aumenta_e_diminui() {
    let amb = Ambiente::exemplo("paineis_leitor");
    let mut h = harness(&amb);
    let no = h.get(by().role(Role::Splitter).label(EXPLORADOR));
    assert!(no.accesskit_node().data().supports_action(Action::Increment));
    assert!(no.accesskit_node().data().supports_action(Action::Decrement));
    // Aumentar e Diminuir são passos de 16 pontos no painel (não na barra), sem precisar do foco
    pedir_acao(&mut h, EXPLORADOR, Action::Increment);
    assert_eq!(desenhadas(&h).0, 274.0);
    pedir_acao(&mut h, EXPLORADOR, Action::Decrement);
    pedir_acao(&mut h, EXPLORADOR, Action::Decrement);
    assert_eq!(desenhadas(&h).0, 242.0);
    pedir_acao(&mut h, PROPRIEDADES, Action::Increment);
    assert_eq!(desenhadas(&h).1, 288.0);
    assentar(&mut h);
    assert_eq!(prefs_lidas(&amb), LarguraPaineis { explorador: Some(242.0), propriedades: Some(288.0) });
    // no mínimo, a ação de diminuir deixa de ser oferecida
    arrastar(&mut h, EXPLORADOR, -1000.0);
    let no = h.get(by().role(Role::Splitter).label(EXPLORADOR));
    assert!(!no.accesskit_node().data().supports_action(Action::Decrement));
    assert!(no.accesskit_node().data().supports_action(Action::Increment));
}

#[test]
fn a_escolha_volta_na_proxima_abertura() {
    let amb = Ambiente::exemplo("paineis_persistencia");
    {
        let mut h = harness(&amb);
        arrastar(&mut h, EXPLORADOR, 70.0);
        arrastar(&mut h, PROPRIEDADES, 30.0);
        assert_eq!(desenhadas(&h), (328.0, 242.0));
    }
    assert_eq!(prefs_lidas(&amb), LarguraPaineis { explorador: Some(328.0), propriedades: Some(242.0) });
    // outra instância lê o arquivo (como o executável) e já abre assim
    let h = harness_com_prefs(&amb, Tema::Claro, TAMANHO);
    assert_eq!(desenhadas(&h), (328.0, 242.0));
    assert_eq!(h.state().larguras_escolhidas(), (328.0, 242.0));
    assert_eq!(documento(&h).left(), 68.0 + 328.0 + 8.0);
    assert_eq!(valor(&h, EXPLORADOR).0, 328.0);
    // sem a chave, as larguras padrão (e o arquivo continua sem ela)
    let amb = Ambiente::exemplo("paineis_sem_chave");
    amb.servico.prefs().atualizar(prefs::Mudanca::tema(prefs::Tema::Escuro)).expect("tema");
    let h = harness_com_prefs(&amb, Tema::Claro, TAMANHO);
    assert_eq!(desenhadas(&h), (258.0, 272.0));
    assert!(!texto_das_prefs(&amb).contains("larguras_paineis"));
    // uma largura que o arquivo traz fora da faixa vale a padrão; uma dentro da faixa mas fora do que a janela
    // comporta é limitada na hora de desenhar, sem mexer na escolha
    let amb = Ambiente::exemplo("paineis_estranhas");
    amb.servico
        .prefs()
        .atualizar(prefs::Mudanca::paineis(prefs::MudancaLarguraPaineis {
            explorador: Some(1900.0),
            propriedades: Some(130.0),
        }))
        .expect("larguras");
    let h = harness_com_prefs(&amb, Tema::Claro, TAMANHO);
    assert_eq!(h.state().larguras_escolhidas(), (1900.0, 130.0));
    // 1900 vira o teto de 45% (648) e encolhe para o documento ficar com 560 (788 − 200); 130 vira o mínimo
    assert_eq!(desenhadas(&h), (588.0, 200.0));
}

#[test]
fn janela_estreita_encolhe_os_paineis_antes_de_esconder_as_propriedades() {
    // com as larguras padrão as Propriedades cabem até 1052 de janela (antes sumiam abaixo de 1182): os painéis
    // encolhem e o documento fica com 560
    let amb = Ambiente::exemplo("paineis_estreita_1100");
    let mut h = harness_com(&amb, Tema::Claro, vec2(1100.0, 700.0), Partida::default(), |_| None);
    assert!(h.state().propriedades_a_vista());
    let (e, p) = desenhadas(&h);
    assert!((200.0..258.0).contains(&e) && (200.0..272.0).contains(&p), "{e} {p}");
    let doc = documento(&h);
    assert_eq!((doc.left(), doc.right()), (68.0 + e + 8.0, 1100.0 - 8.0 - p - 8.0));
    assert!(doc.width() >= 560.0, "{doc:?}");
    // a escolha da pessoa não mudou
    assert_eq!(h.state().larguras_escolhidas(), (258.0, 272.0));
    assert_eq!(valor(&h, EXPLORADOR).0, f64::from(e));
    assert_eq!(valor(&h, PROPRIEDADES).0, f64::from(p));
    capturar(&mut h, "paineis-estreitos-1100-claro.png");

    // no mínimo exato os dois painéis ficam de 200
    let amb = Ambiente::exemplo("paineis_estreita_1052");
    let h = harness_com(&amb, Tema::Claro, vec2(1052.0, 700.0), Partida::default(), |_| None);
    assert!(h.state().propriedades_a_vista());
    assert_eq!(desenhadas(&h), (200.0, 200.0));
    assert_eq!(documento(&h).width(), 560.0);
    // um pixel menos e as Propriedades somem, o Explorador volta ao que a pessoa pediu e as seções vão para o fim
    // do documento (a decisão 4)
    let amb = Ambiente::exemplo("paineis_estreita_1051");
    let h = harness_com(&amb, Tema::Claro, vec2(1051.0, 700.0), Partida::default(), |_| None);
    assert!(!h.state().propriedades_a_vista());
    assert_eq!(desenhadas(&h).0, 258.0);
    assert!(h.query(by().role(Role::Splitter).label(PROPRIEDADES)).is_none(), "sem painel, sem divisória");
    assert!(h.query_by_label("Seção Arquivo").is_some(), "as seções no fim do documento");
    assert_eq!(documento(&h).right(), 1051.0 - 8.0);

    // Propriedades mais estreitas deixam o Explorador inteiro por mais tempo (a 1110 só cabem os dois como estão)
    let amb = Ambiente::exemplo("paineis_estreita_estreitas");
    amb.servico
        .prefs()
        .atualizar(prefs::Mudanca::paineis(prefs::MudancaLarguraPaineis {
            explorador: Some(258.0),
            propriedades: Some(200.0),
        }))
        .expect("larguras");
    let h = harness_com_prefs(&amb, Tema::Claro, vec2(1110.0, 700.0));
    assert_eq!(desenhadas(&h), (258.0, 200.0));
    let amb = Ambiente::exemplo("paineis_estreita_padrao_1110");
    let h = harness_com(&amb, Tema::Claro, vec2(1110.0, 700.0), Partida::default(), |_| None);
    assert!(desenhadas(&h).0 < 258.0, "as padrão já encolhem aí");
}

#[test]
fn arrastar_numa_janela_apertada_acompanha_o_ponteiro() {
    let amb = Ambiente::exemplo("paineis_apertada");
    let mut h = harness_com(&amb, Tema::Claro, vec2(1100.0, 700.0), Partida::default(), |_| None);
    let (e0, p0) = desenhadas(&h);
    // só cabe um ponto a mais, pelo documento mínimo
    arrastar(&mut h, EXPLORADOR, 10.0);
    let (e1, p1) = desenhadas(&h);
    assert!(e1 <= e0 + 2.0 && p1 == p0, "{e0} {p0} -> {e1} {p1}");
    // encolher vale, e o outro painel guarda o que se via (a pessoa mexeu no que via)
    arrastar(&mut h, EXPLORADOR, -30.0);
    assert_eq!(desenhadas(&h).0, 200.0);
    assert_eq!(h.state().larguras_escolhidas().1, p0);
    assert_eq!(documento(&h).width(), 1100.0 - 16.0 - 52.0 - 24.0 - 200.0 - p0);
}

#[test]
fn historico_e_explorador_dividem_a_largura() {
    let amb = Ambiente::exemplo("paineis_historico");
    let mut h = harness(&amb);
    arrastar(&mut h, EXPLORADOR, 50.0);
    assert_eq!(desenhadas(&h).0, 308.0);
    // o Histórico toma o lugar do Explorador, na mesma largura, com a divisória dele
    clicar_botao(&mut h, "Histórico de consultas");
    assentar(&mut h);
    assert_eq!(desenhadas(&h).0, 308.0);
    assert!(h.query(by().role(Role::Splitter).label(EXPLORADOR)).is_none());
    assert_eq!(valor(&h, "Largura do Histórico"), (308.0, 200.0, 516.0));
    assert_eq!(
        valor(&h, PROPRIEDADES),
        (272.0, 200.0, 480.0),
        "o máximo das Propriedades desconta o painel da esquerda"
    );
    assert_eq!(documento(&h).left(), 68.0 + 308.0 + 8.0);
    // sem o painel da esquerda, não há divisória nem largura reservada
    clicar_botao(&mut h, "Histórico de consultas");
    assert!(h.query(by().role(Role::Splitter).label("Largura do Histórico")).is_none());
    assert!(h.query(by().role(Role::Splitter).label(EXPLORADOR)).is_none());
    assert_eq!(documento(&h).left(), 68.0);
}

#[test]
fn o_tab_passa_pelas_divisorias_na_ordem_da_tela() {
    let amb = Ambiente::exemplo("paineis_tab");
    let mut h = harness(&amb);
    h.remove_cursor();
    h.run_steps(2);
    let mut paradas: Vec<String> = Vec::new();
    for _ in 0..120 {
        h.key_press(Key::Tab);
        h.run_steps(1);
        let rotulo =
            h.query_all_by(|n| n.is_focused()).next().and_then(|n| n.accesskit_node().label()).unwrap_or_default();
        let fim = rotulo == PROPRIEDADES;
        paradas.push(rotulo);
        if fim {
            break;
        }
    }
    let posicao =
        |r: &str| paradas.iter().position(|p| p == r).unwrap_or_else(|| panic!("{r} não está no Tab: {paradas:?}"));
    // a divisória do Explorador vem depois da árvore dele e antes do documento; a das Propriedades, antes delas
    assert!(posicao("clientes") < posicao(EXPLORADOR), "{paradas:?}");
    assert!(posicao(EXPLORADOR) < posicao(PROPRIEDADES), "{paradas:?}");
    assert!(paradas.iter().all(|p| !p.trim().is_empty()), "toda parada tem nome: {paradas:?}");
}

#[test]
fn o_conteudo_dos_paineis_acompanha_a_largura() {
    use estado::{Parte, Rota};
    let amb = Ambiente::exemplo("paineis_conteudo");
    let mut h = harness(&amb);
    // o Explorador no mínimo e as Propriedades no mínimo: nada passa da borda do painel, em cada tela
    arrastar(&mut h, EXPLORADOR, -500.0);
    arrastar(&mut h, PROPRIEDADES, 500.0);
    assert_eq!(desenhadas(&h), (200.0, 200.0));
    conferir_conteudo_dos_paineis(&h, "a Visão geral");
    for (rota, tela) in [
        (Rota::objeto("clientes", false), "os Dados de clientes"),
        (Rota::objeto_na_aba("clientes", false, Parte::Estrutura), "a Estrutura de clientes"),
        (Rota::Consulta { sql: None }, "a Consulta SQL"),
    ] {
        h.state_mut().ir(rota);
        assentar(&mut h);
        h.remove_cursor();
        h.run_steps(3);
        assert_eq!(desenhadas(&h), (200.0, 200.0), "{tela}");
        conferir_conteudo_dos_paineis(&h, tela);
        if tela == "os Dados de clientes" {
            capturar(&mut h, "paineis-minimos-claro.png");
        }
    }
}

/// Nos painéis de 200px, quem começa dentro de um painel termina dentro dele e o título do cabeçalho segue a faixa.
fn conferir_conteudo_dos_paineis(h: &H, tela: &str) {
    let doc = documento(h);
    let explorador = Rect::from_min_max(pos2(68.0, doc.top() - 34.0), pos2(68.0 + 200.0, doc.bottom()));
    let propriedades = Rect::from_min_max(pos2(doc.right() + 8.0, doc.top() - 34.0), pos2(1432.0, doc.bottom()));
    let nos: Vec<(Role, String, Rect)> = h
        .query_all_by(|_| true)
        .filter_map(|n| {
            let a = n.accesskit_node();
            let texto = a.label().or_else(|| a.value())?;
            (!texto.trim().is_empty()).then(|| (a.role(), texto, n.rect()))
        })
        .collect();
    // quem começa dentro do painel termina dentro dele (o corpo vai do alto da faixa de abas ao pé do documento)
    let no_corpo = |r: &Rect| r.top() >= explorador.top() - 0.5 && r.bottom() <= explorador.bottom() + 0.5;
    let do_explorador: Vec<_> = nos
        .iter()
        .filter(|(_, _, r)| no_corpo(r) && r.left() >= explorador.left() - 0.5 && r.left() < explorador.right())
        .collect();
    assert!(do_explorador.len() > 3, "{tela}: a árvore está lá: {nos:?}");
    for (papel, texto, r) in &do_explorador {
        assert!(
            r.right() <= explorador.right() + 0.5,
            "{tela}: {papel:?} {texto:?} passa da borda do Explorador: {r:?}"
        );
    }
    let das_propriedades: Vec<_> =
        nos.iter().filter(|(_, _, r)| no_corpo(r) && r.left() >= propriedades.left() - 0.5).collect();
    assert!(das_propriedades.len() > 3, "{tela}: as Propriedades estão lá: {nos:?}");
    for (papel, texto, r) in &das_propriedades {
        assert!(
            r.right() <= propriedades.right() + 0.5,
            "{tela}: {papel:?} {texto:?} passa da borda das Propriedades: {r:?}"
        );
    }
    // o título do cabeçalho segue a faixa (7px da borda do painel e 12px até o título)
    let titulo = h
        .query_all(by().role(Role::Label).label("Propriedades"))
        .map(|n| n.rect())
        .find(|r| r.left() > doc.right())
        .expect("o título das Propriedades");
    assert!(
        titulo.left() >= propriedades.left() + 7.0 + 12.0 - 0.5 && titulo.right() <= propriedades.right(),
        "{tela}: {titulo:?}"
    );
}

/// Gera a captura com o ponteiro sobre uma divisória (o fio de destaque aparece).
fn capturar_com_ponteiro(h: &mut H, rotulo: &str, nome: &str) {
    let centro = vao(h, rotulo).center();
    h.hover_at(centro);
    h.run_steps(3);
    let imagem = renderizar(h);
    imagem.save(pasta_capturas().join(nome)).expect("gravar a captura");
}

fn preparar_para_captura(tema: Tema, nome: &str) -> H {
    let amb = Ambiente::exemplo(nome);
    let mut h = harness_com(&amb, tema, TAMANHO, Partida::default(), |_| None);
    h.state_mut().ir(estado::Rota::objeto("clientes", false));
    assentar(&mut h);
    arrastar(&mut h, EXPLORADOR, 82.0);
    arrastar(&mut h, PROPRIEDADES, -40.0);
    assert_eq!(desenhadas(&h), (340.0, 312.0));
    h
}

#[test]
fn captura_com_os_paineis_redimensionados() {
    for (tema, nome) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let mut h = preparar_para_captura(tema, &format!("paineis_captura_{nome}"));
        // o ponteiro na divisória do Explorador: o fio de 2px em destaque
        capturar_com_ponteiro(&mut h, EXPLORADOR, &format!("paineis-redimensionados-{nome}.png"));
    }
    // o foco do teclado na divisória das Propriedades: o fio firme
    let mut h = preparar_para_captura(Tema::Claro, "paineis_captura_foco");
    h.remove_cursor();
    h.get(by().role(Role::Splitter).label(PROPRIEDADES)).focus();
    h.run_steps(2);
    h.key_press(Key::ArrowLeft);
    h.run_steps(3);
    assert!(h.get(by().role(Role::Splitter).label(PROPRIEDADES)).is_focused());
    let imagem = renderizar(&mut h);
    imagem.save(pasta_capturas().join("paineis-redimensionados-foco-claro.png")).expect("gravar a captura");
}
