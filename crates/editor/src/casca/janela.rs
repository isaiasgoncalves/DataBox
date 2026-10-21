//! A barra de título integrada aos menus (o `.titulo` de `Main.dc.html`) e os controles da janela sem a decoração
//! do Windows.
//!
//! **A faixa** ([`faixa_titulo`], na camada da raiz): 30px na cor do fundo da janela, com o logo completo de
//! 16px no quadrado de 36×30, os menus ([`super::menus`]) e o título no centro da janela ("exemplo.db · DataBox"),
//! em `--t3`, cortado com "…" entre o fim dos menus e o começo dos botões e esmaecido com a janela inativa. O
//! título também vai ao sistema (`ViewportCommand::Title`, pela casca), para a barra de tarefas e o Alt+Tab.
//!
//! **Os controles** ([`controles_da_janela`], só com a barra própria) ficam numa camada `Order::Tooltip`, acima da
//! janela modal: a `Modal` do egui fica em `Foreground` e só bloqueia as camadas abaixo dela, e uma camada sem `Area`
//! é interativa e ordenada pela `Order`. Assim, com a Exportar ou uma confirmação aberta, ainda dá para mover,
//! minimizar e fechar (o fechar passa pela pergunta de sempre). O fundo da faixa e os menus ficam na raiz, sob o véu
//! da modal.
//!
//! - O espaço livre (o logo e o trecho entre o último menu e o primeiro botão; o título não pega clique) arrasta a
//!   janela: `StartDrag` no começo do arrasto, e não no aperto, para o clique e o duplo clique não entrarem no laço
//!   do sistema. O duplo clique maximiza ou restaura (também no logo, que não fecha a janela como o ícone do
//!   sistema). Ele nunca cobre os menus.
//! - Os três botões de 46×30 à direita: "Minimizar", "Maximizar" ou "Restaurar" (pelo `viewport().maximized`) e
//!   "Fechar a janela" (dica "Fechar"), que manda `ViewportCommand::Close`. O eframe o devolve como o pedido de
//!   fechar de sempre, o mesmo caminho do Alt+F4 (a pergunta da exportação e da transação aberta).
//! - As faixas de redimensionar, de [`ESPESSURA_BORDA`] por dentro da borda, só com a janela não maximizada, fora
//!   da tela cheia e sem um menu ou uma lista aberta ([`direcao_da_borda`]): o cursor de redimensionar no hover e o
//!   `BeginResize` no próprio aperto (o redimensionar não espera o limiar do arrasto). Elas caem no anel de 8px de
//!   fundo em volta dos painéis; em cima, sobre a faixa do título (os menus e o Fechar perdem os 5px de cima). As dos
//!   lados começam embaixo da faixa do título: o Fechar fica com os 5px da direita, e o canto de cima à direita é
//!   o da faixa de cima.
//! - O espaço livre e as faixas só têm clique e arrasto, sem o foco: não são paradas do Tab nem nós de
//!   acessibilidade (o Tab e as setas não caem em áreas sem nome).
//! - A cada quadro, as [`RegioesJanela`] vão para o `ctx.data` ([`regioes_da_janela`]): a porta para tratar o
//!   `WM_NCHITTEST` um dia.
//!
//! **O que se perde sem a decoração do Windows.** O winit 0.30 não trata o `WM_NCHITTEST`, e as duas saídas (o
//! winit como dependência direta, para o menu do sistema, e o subclassing da janela, para o Snap Layouts) ficaram
//! de fora da etapa 2 por decisão de 16/10/2026. Sem elas:
//! - o Snap Layouts não aparece ao passar o mouse no Maximizar (o Win+Z ainda deve abrir);
//! - o clique direito no título não abre o menu do sistema (o Alt+Espaço ainda deve abrir);
//! - as bordas invisíveis de ~8px por fora da janela somem: o alvo de redimensionar fica 5px para dentro.
//!
//! Continuam com o sistema: o Aero Snap ao arrastar, arrastar a janela maximizada para restaurar, Win+setas, Alt+F4
//! e a sombra (`drag_window` e `drag_resize_window` do winit entram no laço do próprio Windows).
//!
//! **No Linux** (desde 20/10/2026; antes, a barra do GNOME ficava em cima da faixa dos menus) vale a mesma barra: o
//! `drag_window` e o `drag_resize_window` do winit passam o arrasto ao compositor (o `xdg_toplevel.move`/`resize` no
//! Wayland, o `_NET_WM_MOVERESIZE` no X11), e o encaixe nas bordas da tela e o Super+setas continuam com ele. Perde-se
//! o menu da janela no clique direito do título e, no Wayland do GNOME, a sombra e os cantos arredondados (o Mutter
//! não desenha sombra em janela sem decoração); os botões seguem o desenho do DataBox, não o `button-layout` do
//! sistema. O `--moldura-do-sistema` volta à decoração do sistema.

use egui::{
    Align, CursorIcon, Id, LayerId, Layout, Order, PointerButton, Pos2, Rangef, Rect, ResizeDirection, Sense, Ui,
    UiBuilder, ViewportCommand, pos2,
};
use interface::pecas::barra_titulo::{BotaoJanela, LARGURA_BOTAO_JANELA, botao_janela_em, marca, titulo_centrado};

use super::RecenteMenu;
use super::menus::{self, ESPACO_MENUS, Menus, PedidoMenu};

/// A espessura das faixas de redimensionar, por dentro da borda da janela.
pub const ESPESSURA_BORDA: f32 = 5.0;
/// Até onde, a partir da quina, a faixa de redimensionar vale como canto (só o trecho da faixa, nunca um quadrado).
pub const CANTO_BORDA: f32 = 12.0;

/// As regiões da barra própria no último quadro, em pontos (a porta para um `WM_NCHITTEST` próprio).
#[derive(Debug, Clone, PartialEq)]
pub struct RegioesJanela {
    /// O espaço livre, que arrasta a janela: o logo e o trecho entre os menus e os botões.
    pub livre: Vec<Rect>,
    /// Minimizar, Maximizar (ou Restaurar) e Fechar.
    pub botoes: [Rect; 3],
    /// A espessura das faixas de redimensionar; 0 com a janela maximizada ou em tela cheia.
    pub borda: f32,
}

fn id_regioes() -> Id {
    Id::new("databox::regioes_janela")
}

/// As regiões da barra própria publicadas no último quadro (None sem a barra própria).
pub fn regioes_da_janela(ctx: &egui::Context) -> Option<RegioesJanela> {
    ctx.data(|d| d.get_temp(id_regioes()))
}

/// Em que faixa de redimensionar cai `pos`: as faixas de `espessura` por dentro de cada lado de `janela`, com o canto
/// no trecho da faixa a até `canto` da quina. Fora delas (e fora da janela), None.
pub fn direcao_da_borda(pos: Pos2, janela: Rect, espessura: f32, canto: f32) -> Option<ResizeDirection> {
    if espessura <= 0.0 || !janela.contains(pos) {
        return None;
    }
    let oeste = pos.x - janela.left() < espessura;
    let leste = janela.right() - pos.x < espessura;
    let norte = pos.y - janela.top() < espessura;
    let sul = janela.bottom() - pos.y < espessura;
    let lado = oeste || leste;
    let topo_ou_base = norte || sul;
    if !lado && !topo_ou_base {
        return None;
    }
    // ao longo de uma faixa, perto da quina, vale o canto: Some(true) = oeste / norte, Some(false) = leste / sul
    let horizontal = if oeste || (topo_ou_base && pos.x - janela.left() < canto) {
        Some(true)
    } else if leste || (topo_ou_base && janela.right() - pos.x < canto) {
        Some(false)
    } else {
        None
    };
    let vertical = if norte || (lado && pos.y - janela.top() < canto) {
        Some(true)
    } else if sul || (lado && janela.bottom() - pos.y < canto) {
        Some(false)
    } else {
        None
    };
    match (vertical, horizontal) {
        (Some(true), Some(true)) => Some(ResizeDirection::NorthWest),
        (Some(true), Some(false)) => Some(ResizeDirection::NorthEast),
        (Some(false), Some(true)) => Some(ResizeDirection::SouthWest),
        (Some(false), Some(false)) => Some(ResizeDirection::SouthEast),
        (Some(true), None) => Some(ResizeDirection::North),
        (Some(false), None) => Some(ResizeDirection::South),
        (None, Some(true)) => Some(ResizeDirection::West),
        (None, Some(false)) => Some(ResizeDirection::East),
        (None, None) => None,
    }
}

/// O cursor de cada direção.
fn cursor(direcao: ResizeDirection) -> CursorIcon {
    match direcao {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

/// Onde ficam os três botões (46×30 cada, encostados no canto de cima à direita).
pub(crate) fn retangulos_dos_botoes(janela: Rect) -> [Rect; 3] {
    let altura = interface::pecas::barra_titulo::ALTURA_TITULO;
    let botao = |i: f32| {
        let x = janela.right() - (3.0 - i) * LARGURA_BOTAO_JANELA;
        Rect::from_min_max(pos2(x, janela.top()), pos2(x + LARGURA_BOTAO_JANELA, janela.top() + altura))
    };
    [botao(0.0), botao(1.0), botao(2.0)]
}

/// O trecho da faixa em que o título pode ficar: do fim dos menus ao começo dos botões (sem a barra própria, até o
/// fim da faixa).
pub(crate) fn trecho_do_titulo(faixa: Rect, fim_menus: f32, barra_propria: bool) -> Rangef {
    let fim = if barra_propria { faixa.right() - 3.0 * LARGURA_BOTAO_JANELA } else { faixa.right() };
    Rangef::new(fim_menus, fim.max(fim_menus))
}

/// O que a faixa devolve.
pub(crate) struct SaidaFaixa {
    pub pedido: Option<PedidoMenu>,
    /// O botão "Ajuda".
    pub id_ajuda: Id,
    /// O espaço livre: o logo e o trecho entre os menus e os botões (o do título).
    pub logo: Rect,
    pub livre: Rect,
}

/// A faixa de 30px no espaço de `ui` (a largura da janela): o logo, os menus e o título no centro. Não desenha os
/// botões da janela (ver [`controles_da_janela`]), mas reserva o lugar deles quando `barra_propria`.
pub(crate) fn faixa_titulo(
    ui: &mut Ui,
    titulo: &str,
    barra_propria: bool,
    m: &Menus<'_>,
    recentes: &[RecenteMenu],
) -> SaidaFaixa {
    let faixa = ui.max_rect();
    let mut linha = ui.new_child(
        UiBuilder::new().id_salt("faixa-titulo").max_rect(faixa).layout(Layout::left_to_right(Align::Center)),
    );
    linha.spacing_mut().item_spacing.x = 0.0;
    let logo = marca(&mut linha).rect;
    linha.spacing_mut().item_spacing.x = ESPACO_MENUS;
    let saida = menus::barra(&mut linha, m, recentes);
    let fim_menus = linha.min_rect().right();
    let trecho = trecho_do_titulo(faixa, fim_menus, barra_propria);
    let ativa = ui.ctx().input(|i| i.viewport().focused != Some(false));
    titulo_centrado(ui, faixa, trecho, titulo, ativa);
    let livre = Rect::from_x_y_ranges(trecho, faixa.y_range());
    SaidaFaixa { pedido: saida.pedido, id_ajuda: saida.id_ajuda, logo, livre }
}

/// Os controles da janela sem decoração, na camada `Order::Tooltip` (acima da modal), sobre a janela inteira:
/// arrastar e maximizar pelo espaço livre (`logo` e `livre`, os da [`faixa_titulo`] deste quadro), os três botões e
/// as faixas de redimensionar. Publica as [`RegioesJanela`]. Chamado no fim do quadro, depois das janelas globais.
pub(crate) fn controles_da_janela(ui: &mut Ui, logo: Rect, livre: Rect) {
    let ctx = ui.ctx().clone();
    let janela = ctx.content_rect();
    let (maximizada, tela_cheia, ativa) = ctx.input(|i| {
        let v = i.viewport();
        (v.maximized.unwrap_or(false), v.fullscreen.unwrap_or(false), v.focused != Some(false))
    });
    let camada = LayerId::new(Order::Tooltip, Id::new("databox::janela"));
    let ui = ui.new_child(UiBuilder::new().id_salt("controles-da-janela").layer_id(camada).max_rect(janela));

    // o espaço livre: arrastar e o duplo clique (sem o foco: não é uma parada do Tab nem um nó de acessibilidade)
    let livres: Vec<Rect> = [logo, livre].into_iter().filter(|r| r.is_positive()).collect();
    for (i, r) in livres.iter().enumerate() {
        let resp = ui.interact(*r, ui.id().with(("livre", i)), Sense::CLICK | Sense::DRAG);
        if resp.drag_started_by(PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if resp.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximizada));
        }
    }

    // os três botões (maximizada, a janela encosta nas bordas da tela e eles também)
    let botoes = retangulos_dos_botoes(janela);
    let do_meio = if maximizada { BotaoJanela::Restaurar } else { BotaoJanela::Maximizar };
    for (i, botao) in [BotaoJanela::Minimizar, do_meio, BotaoJanela::Fechar].into_iter().enumerate() {
        if botao_janela_em(&ui, ui.id().with(("botao", i)), botoes[i], botao, ativa).clicked() {
            ctx.send_viewport_cmd(match botao {
                BotaoJanela::Minimizar => ViewportCommand::Minimized(true),
                BotaoJanela::Maximizar => ViewportCommand::Maximized(true),
                BotaoJanela::Restaurar => ViewportCommand::Maximized(false),
                BotaoJanela::Fechar => ViewportCommand::Close,
            });
        }
    }

    // as faixas de redimensionar, por último: ficam por cima dos botões e do espaço livre. Com um menu ou uma lista
    // aberta, elas saem: ficariam por cima do que encosta na borda da janela
    let borda = if maximizada || tela_cheia { 0.0 } else { ESPESSURA_BORDA };
    if borda > 0.0 && !egui::Popup::is_any_open(&ctx) {
        bordas(&ui, janela, borda);
    }
    let regioes = RegioesJanela { livre: livres, botoes, borda };
    ctx.data_mut(|d| d.insert_temp(id_regioes(), regioes));
}

/// As quatro faixas de redimensionar (em cima, embaixo, à esquerda e à direita). As dos lados começam embaixo da
/// barra de título: o Fechar fica inteiro, e o canto de cima à direita continua na faixa de cima.
pub(crate) fn faixas_da_borda(janela: Rect, espessura: f32) -> [Rect; 4] {
    let abaixo_do_titulo = janela.top() + interface::pecas::barra_titulo::ALTURA_TITULO;
    [
        Rect::from_min_max(janela.min, pos2(janela.right(), janela.top() + espessura)),
        Rect::from_min_max(pos2(janela.left(), janela.bottom() - espessura), janela.max),
        Rect::from_min_max(pos2(janela.left(), abaixo_do_titulo), pos2(janela.left() + espessura, janela.bottom())),
        Rect::from_min_max(pos2(janela.right() - espessura, abaixo_do_titulo), janela.max),
    ]
}

/// As [`faixas_da_borda`]: o cursor no hover e o `BeginResize` no aperto. Sem o foco: não são paradas do Tab nem
/// nós de acessibilidade.
fn bordas(ui: &Ui, janela: Rect, espessura: f32) {
    let ctx = ui.ctx();
    for (i, r) in faixas_da_borda(janela, espessura).into_iter().enumerate() {
        let resp = ui.interact(r, ui.id().with(("borda", i)), Sense::DRAG);
        let apertada = resp.is_pointer_button_down_on();
        if !resp.hovered() && !apertada {
            continue;
        }
        let pos = resp.interact_pointer_pos().or_else(|| resp.hover_pos());
        let Some(direcao) = pos.and_then(|p| direcao_da_borda(p, janela, espessura, CANTO_BORDA)) else { continue };
        ctx.set_cursor_icon(cursor(direcao));
        if apertada && ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direcao));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use egui::vec2;

    const JANELA: Rect = Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1440.0, y: 900.0 } };

    fn em(x: f32, y: f32) -> Option<ResizeDirection> {
        direcao_da_borda(pos2(x, y), JANELA, ESPESSURA_BORDA, CANTO_BORDA)
    }

    #[test]
    fn direcao_da_borda_nos_lados_e_nos_cantos() {
        use ResizeDirection::*;
        // os lados
        assert_eq!(em(2.0, 450.0), Some(West));
        assert_eq!(em(1438.0, 450.0), Some(East));
        assert_eq!(em(720.0, 2.0), Some(North));
        assert_eq!(em(720.0, 898.0), Some(South));
        // os quatro cantos, na quina
        assert_eq!(em(1.0, 1.0), Some(NorthWest));
        assert_eq!(em(1439.0, 1.0), Some(NorthEast));
        assert_eq!(em(1.0, 899.0), Some(SouthWest));
        assert_eq!(em(1439.0, 899.0), Some(SouthEast));
        // o canto vale ao longo da faixa até 12px da quina, nunca num quadrado de 12×12
        assert_eq!(em(10.0, 2.0), Some(NorthWest));
        assert_eq!(em(2.0, 10.0), Some(NorthWest));
        assert_eq!(em(13.0, 2.0), Some(North));
        assert_eq!(em(1438.0, 14.0), Some(East));
        assert_eq!(em(8.0, 8.0), None, "dentro do quadrado de 12px mas fora das faixas");
        // dentro e fora
        assert_eq!(em(720.0, 450.0), None);
        assert_eq!(em(5.0, 450.0), None, "a faixa tem 5px");
        assert_eq!(em(-1.0, 450.0), None);
        assert_eq!(direcao_da_borda(pos2(2.0, 450.0), JANELA, 0.0, CANTO_BORDA), None, "sem borda");
    }

    #[test]
    fn na_quina_do_fechar_so_os_5px_de_cima_redimensionam() {
        let [_, _, fechar] = retangulos_dos_botoes(JANELA);
        assert_eq!(fechar, Rect::from_min_size(pos2(1394.0, 0.0), vec2(46.0, 30.0)));
        // a quina do Fechar é o canto de cima à direita
        assert_eq!(em(fechar.right() - 1.0, fechar.top() + 1.0), Some(ResizeDirection::NorthEast));
        // nos 5px de cima, o norte (perto da quina, o canto); embaixo deles, o meio do botão é dele
        assert_eq!(em(fechar.center().x, 3.0), Some(ResizeDirection::North));
        assert_eq!(em(fechar.center().x, fechar.center().y), None);
        assert_eq!(em(fechar.left() + 1.0, 6.0), None);
    }

    #[test]
    fn os_lados_comecam_embaixo_da_barra_de_titulo() {
        // antes, a faixa da direita ia do alto da janela e comia os 5px da direita do Fechar
        let faixas = faixas_da_borda(JANELA, ESPESSURA_BORDA);
        let em_alguma = |x: f32, y: f32| faixas.iter().any(|r| r.contains(pos2(x, y)));
        let [_, _, fechar] = retangulos_dos_botoes(JANELA);
        // a direita do Fechar, embaixo dos 5px de cima, é dele
        assert!(!em_alguma(fechar.right() - 2.0, fechar.center().y));
        assert!(!em_alguma(fechar.right() - 2.0, fechar.bottom() - 1.0));
        // e a esquerda do logo também é do espaço livre
        assert!(!em_alguma(2.0, 15.0));
        // o canto de cima à direita continua na faixa de cima, e os lados valem logo embaixo da barra de título
        assert!(em_alguma(fechar.right() - 1.0, 1.0));
        assert_eq!(em(fechar.right() - 1.0, 1.0), Some(ResizeDirection::NorthEast));
        assert!(em_alguma(JANELA.right() - 2.0, fechar.bottom() + 1.0));
        assert!(em_alguma(2.0, 31.0));
        assert!(em_alguma(720.0, JANELA.bottom() - 1.0));
    }

    #[test]
    fn o_titulo_fica_entre_os_menus_e_os_botoes() {
        let faixa = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 30.0));
        assert_eq!(trecho_do_titulo(faixa, 260.0, true), Rangef::new(260.0, 1440.0 - 138.0));
        // sem a barra própria, até o fim
        assert_eq!(trecho_do_titulo(faixa, 260.0, false), Rangef::new(260.0, 1440.0));
        // menus que passam do começo dos botões: o trecho fica vazio, sem inverter
        assert_eq!(trecho_do_titulo(faixa, 1350.0, true).span(), 0.0);
    }
}
