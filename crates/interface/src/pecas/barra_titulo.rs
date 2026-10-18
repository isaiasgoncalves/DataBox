//! A barra de título integrada aos menus (o `.titulo` de `Main.dc.html`): uma faixa de 30px ([`ALTURA_TITULO`])
//! na cor do fundo da janela, com o logo completo de 16px num quadrado de 36×30 ([`marca`]), os menus, o
//! título centrado em `--t3` ([`titulo_centrado`]) e os botões minimizar, maximizar (ou restaurar) e fechar, de
//! 46×30 ([`botao_janela`]).
//!
//! As peças só desenham e respondem ao clique: quem chama manda os comandos ao sistema (`ViewportCommand`) e cuida
//! do arrasto e das bordas. Os glifos dos botões são de traço, 10px com 1px, alinhados ao pixel; no hover, o fundo
//! `--h` (o Fechar fica vermelho, `#c42b1c`, com o glifo branco). Com a janela inativa, o título e os glifos
//! esmaecem.
//!
//! ```ignore
//! let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
//! let ativa = ctx.input(|i| i.viewport().focused != Some(false));
//! if botao_janela(ui, BotaoJanela::Fechar, ativa).clicked() { ctx.send_viewport_cmd(ViewportCommand::Close) }
//! ```

use egui::{
    Color32, CursorIcon, Id, Painter, Pos2, Rangef, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::pecas::texto::galley_cortado;
use crate::pintura;
use crate::tema::{duracao_transicao, paleta, rgb};

/// Altura da barra de título (`.titulo{height:30px}`).
pub const ALTURA_TITULO: f32 = 30.0;
/// Largura de cada botão da janela.
pub const LARGURA_BOTAO_JANELA: f32 = 46.0;
/// O quadrado do logo (`.titulo .marca`: 36×30).
pub const LARGURA_MARCA: f32 = 36.0;
/// O logo completo de 16px.
const LADO_LOGO: f32 = 16.0;
/// O vermelho do Fechar no hover (o do Windows 11).
pub const VERMELHO_FECHAR: Color32 = rgb(0xC4, 0x2B, 0x1C);
/// O espaço mínimo entre o título e o que vem antes e depois dele.
const FOLGA_TITULO: f32 = 12.0;
/// Os botões da janela são retos (encostam nas bordas dela).
const RAIO_BOTAO_JANELA: f32 = 0.0;
/// O anel de foco fica por dentro do botão (por fora, na borda da janela, ele sumiria): o `anel_foco` afasta 3px,
/// então o anel fica a 2px da borda do botão.
const RECUO_ANEL_FOCO: f32 = 5.0;

/// Os três botões da janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotaoJanela {
    Minimizar,
    Maximizar,
    /// O maximizar com a janela já maximizada.
    Restaurar,
    Fechar,
}

impl BotaoJanela {
    /// O nome para o leitor de tela (único na janela: o × das abas e o das janelas têm outros).
    pub fn rotulo(self) -> &'static str {
        match self {
            BotaoJanela::Minimizar => "Minimizar",
            BotaoJanela::Maximizar => "Maximizar",
            BotaoJanela::Restaurar => "Restaurar",
            BotaoJanela::Fechar => "Fechar a janela",
        }
    }

    /// A dica.
    pub fn dica(self) -> &'static str {
        match self {
            BotaoJanela::Fechar => "Fechar",
            outro => outro.rotulo(),
        }
    }
}

/// O logo completo de 16px no quadrado de 36×30 (sem clique: o espaço é de arrastar a janela).
pub fn marca(ui: &mut Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(LARGURA_MARCA, ALTURA_TITULO), Sense::hover());
    if ui.is_rect_visible(rect) {
        crate::marca::pintar_logo(ui, Rect::from_center_size(rect.center(), vec2(LADO_LOGO, LADO_LOGO)));
    }
    resp
}

/// Um botão da janela de 46×30, alocado no `ui` (da esquerda para a direita).
pub fn botao_janela(ui: &mut Ui, botao: BotaoJanela, janela_ativa: bool) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(LARGURA_BOTAO_JANELA, ALTURA_TITULO), Sense::hover());
    let id = ui.id().with(("databox::botao_janela", botao.rotulo()));
    botao_janela_em(ui, id, rect, botao, janela_ativa)
}

/// Um botão da janela num retângulo dado (sem alocar: os controles da janela ficam numa camada própria). O
/// retângulo pode passar dos 46×30 (com a janela maximizada, ele vai até a borda da tela); o glifo fica no meio.
pub fn botao_janela_em(ui: &Ui, id: Id, rect: Rect, botao: BotaoJanela, janela_ativa: bool) -> Response {
    let p = paleta(ui);
    let e = &p.estudio;
    let resp = ui.interact(rect, id, Sense::click());
    let tempo = duracao_transicao(ui);
    let hover = ui.ctx().animate_bool_with_time(id.with("hover"), resp.hovered(), tempo);
    let pressao = resp.is_pointer_button_down_on();
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fechar = botao == BotaoJanela::Fechar;
        let fundo = match (fechar, pressao) {
            (true, false) => VERMELHO_FECHAR,
            (true, true) => pintura::clarear(VERMELHO_FECHAR, 0.85),
            (false, false) => e.h,
            (false, true) => e.b2,
        };
        if hover > 0.0 {
            painter.rect_filled(rect, RAIO_BOTAO_JANELA, fundo.gamma_multiply(hover));
        }
        let base = if janela_ativa { e.t2 } else { e.t3 };
        let cor =
            if fechar { pintura::misturar(base, Color32::WHITE, hover) } else { pintura::misturar(base, e.t, hover) };
        pintar_glifo(painter, rect.center(), botao, cor);
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(painter, p, rect.shrink(RECUO_ANEL_FOCO), RAIO_BOTAO_JANELA);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Default);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), botao.rotulo()));
    resp.on_hover_text(botao.dica())
}

/// O glifo de 10px com traço de 1px, centrado no pixel inteiro mais perto de `centro` (o traço fica nítido).
fn pintar_glifo(painter: &Painter, centro: Pos2, botao: BotaoJanela, cor: Color32) {
    let c = pos2(centro.x.round(), centro.y.round());
    let traco = Stroke::new(1.0, cor);
    match botao {
        BotaoJanela::Minimizar => {
            painter.hline(c.x - 5.0..=c.x + 5.0, c.y + 0.5, traco);
        }
        BotaoJanela::Maximizar => {
            painter.rect_stroke(
                Rect::from_min_max(c - vec2(5.0, 5.0), c + vec2(5.0, 5.0)),
                1.0,
                traco,
                StrokeKind::Inside,
            );
        }
        BotaoJanela::Restaurar => {
            // a janela da frente (8×8, embaixo à esquerda) e o canto da de trás
            painter.rect_stroke(
                Rect::from_min_max(c + vec2(-5.0, -3.0), c + vec2(3.0, 5.0)),
                1.0,
                traco,
                StrokeKind::Inside,
            );
            let atras = vec![
                c + vec2(-2.5, -3.0),
                c + vec2(-2.5, -4.5),
                c + vec2(4.5, -4.5),
                c + vec2(4.5, 2.5),
                c + vec2(3.0, 2.5),
            ];
            painter.add(egui::Shape::line(atras, traco));
        }
        BotaoJanela::Fechar => {
            painter.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], traco);
            painter.line_segment([c + vec2(5.0, -5.0), c + vec2(-5.0, 5.0)], traco);
        }
    }
}

/// O título da janela no centro da faixa (o `.nome`: `left:50%`, em `--t3`), cortado com "…" para caber em
/// `livre` (do fim dos menus ao começo dos botões, com 12px de folga) e deslocado para dentro dele quando o centro
/// não dá. Esmaece com a janela inativa. Não aloca nem pega clique (o espaço é de arrastar a janela); devolve onde
/// o texto ficou.
pub fn titulo_centrado(ui: &Ui, faixa: Rect, livre: Rangef, texto: &str, janela_ativa: bool) -> Rect {
    let e = &paleta(ui).estudio;
    let livre = Rangef::new(livre.min + FOLGA_TITULO, livre.max - FOLGA_TITULO);
    let cor = if janela_ativa { e.t3 } else { e.t3.gamma_multiply(0.6) };
    let g = galley_cortado(ui, texto, fontes::fonte(12.0, Peso::Normal), cor, livre.span().max(0.0));
    let x = posicao_titulo(g.size().x, faixa.center().x, livre);
    let rect = Rect::from_min_size(pos2(x, faixa.center().y - g.size().y / 2.0), g.size());
    if livre.span() > 0.0 {
        ui.painter().galley(rect.min, g, cor);
    }
    let resp = ui.interact(rect, ui.id().with("databox::titulo_da_janela"), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, texto));
    rect
}

/// Onde o título começa: centrado em `centro`, mas sem sair de `livre` (do lado que sobra, ele encosta).
pub(crate) fn posicao_titulo(largura: f32, centro: f32, livre: Rangef) -> f32 {
    (centro - largura / 2.0).min(livre.max - largura).max(livre.min)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn titulo_fica_no_centro_e_encosta_quando_nao_cabe() {
        let livre = Rangef::new(300.0, 1300.0);
        // cabe no centro da janela (720)
        assert_eq!(posicao_titulo(200.0, 720.0, livre), 620.0);
        // menus compridos: o centro cairia em cima deles, o título encosta no fim deles
        assert_eq!(posicao_titulo(200.0, 350.0, livre), 300.0);
        // perto dos botões: encosta neles
        assert_eq!(posicao_titulo(200.0, 1250.0, livre), 1100.0);
    }

    #[test]
    fn rotulos_dos_botoes() {
        assert_eq!(BotaoJanela::Fechar.rotulo(), "Fechar a janela");
        assert_eq!(BotaoJanela::Fechar.dica(), "Fechar");
        assert_eq!(BotaoJanela::Restaurar.dica(), "Restaurar");
    }
}
