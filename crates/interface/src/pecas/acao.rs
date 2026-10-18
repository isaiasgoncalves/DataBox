//! O cartão de ação do Início (o `.acao` da prancheta 4, `Inicio.dc.html`): um botão grande de 220px com o ícone
//! redondo de 36px, o nome em 13px / 600 e, embaixo, o atalho numa etiqueta `.kbd`. É o volume neutro com as riscas
//! (`background:var(--textura),var(--volneu)`, raio 10, borda `--btb`), com o halo de 26px e o anel de 1px em
//! `--brilho` no hover; o primário ([`Acao::primaria`], `.acao.pri`) usa o degradê do botão principal, com o texto
//! branco, a sombra azul `0 4px 14px` e, no hover, o anel e o halo de 30px em `--brilhof`. O ícone é uma silhueta de
//! 20px no círculo (`--tint` e `--acct`; no primário, branco a 18% e branco).
//!
//! ```ignore
//! if ui.add(Acao::new("Abrir pasta", Icone::Pasta).atalho("Ctrl+Shift+O").primaria(true)).clicked() { … }
//! ```
//!
//! Todos têm a mesma altura ([`ALTURA_ACAO`], a do cartão com atalho: o `align-items:stretch` da fileira da
//! prancheta), tenham atalho ou não. É um botão como qualquer outro para o teclado: Tab, Enter e Espaço.

use egui::{
    Color32, CursorIcon, Rect, Response, Sense, Stroke, StrokeKind, Ui, Widget, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::botao::{EstadoVolume, Volume, pintar_volume, pintor_de_fora};
use crate::pecas::texto::{galley, galley_cortado};
use crate::pintura;
use crate::tema::{Paleta, SombraCss, duracao_transicao, paleta, rgba};

/// A largura do cartão (`.acao{width:220px}`), com a borda e o `padding:14px`.
pub const LARGURA_ACAO: f32 = 220.0;
/// A altura: a borda, o `padding:14px`, o nome de 17px, 4px e a etiqueta de 18px (39px de conteúdo), dos dois lados.
pub const ALTURA_ACAO: f32 = 1.0 + 14.0 + ALTURA_NOME + MARGEM_KBD + ALTURA_KBD + 14.0 + 1.0;
/// O raio do cartão (`border-radius:10px`).
pub const RAIO_ACAO: f32 = 10.0;
/// A borda de 1px e o `padding:14px`.
const RECUO: f32 = 15.0;
/// O círculo do ícone (`.ico{width:36px}`), a silhueta de 20px dentro dele e o `gap:12px` até o texto.
const LADO_CIRCULO: f32 = 36.0;
const LADO_SILHUETA: f32 = 20.0;
const GAP: f32 = 12.0;
/// A linha do nome (13px) e a etiqueta `.kbd` (`line-height:16px` e a borda, com `margin-top:4px`).
const ALTURA_NOME: f32 = 17.0;
const MARGEM_KBD: f32 = 4.0;
const ALTURA_KBD: f32 = 18.0;
/// Quanto os halos passam da peça (o de 30px vai a 3σ = 45px).
const ALCANCE_HALO: f32 = 48.0;

/// O volume do cartão neutro: o do `.btn` sem o `brightness` do hover, com o halo de 26px (`.acao:hover`).
fn volume_neutro(p: &Paleta) -> Volume {
    Volume { halo_desfoque: 26.0, clarear_hover: 1.0, ..Volume::neutro(p) }
}

/// O volume do `.acao.pri`: a sombra `0 4px 14px` do destaque a 35% (no lugar das duas do botão), que no hover dá
/// lugar ao anel e ao halo de 30px em `--brilhof`.
fn volume_primario(p: &Paleta) -> Volume {
    let sem = SombraCss { y: 0.0, desfoque: 0.0, espalhar: 0.0, cor: Color32::TRANSPARENT };
    let azul = SombraCss { y: 4.0, desfoque: 14.0, espalhar: 0.0, cor: p.anel_aqua_forte.sombra.cor };
    Volume {
        sombras: [azul, sem],
        halo_desfoque: 30.0,
        halo_cor: p.estudio.brilho_forte,
        clarear_hover: 1.0,
        ..Volume::primario(p)
    }
}

/// Um cartão de ação. Use com `ui.add(Acao::new("Abrir arquivo", Icone::Arquivo).atalho("Ctrl+O"))`.
#[must_use = "use ui.add(acao)"]
pub struct Acao<'a> {
    titulo: &'a str,
    icone: Icone,
    atalho: Option<&'a str>,
    primaria: bool,
    habilitado: bool,
}

impl<'a> Acao<'a> {
    /// O `.acao` neutro, com o nome e a silhueta do círculo.
    pub fn new(titulo: &'a str, icone: Icone) -> Self {
        Acao { titulo, icone, atalho: None, primaria: false, habilitado: true }
    }

    /// O atalho na etiqueta `.kbd` embaixo do nome.
    pub fn atalho(mut self, atalho: &'a str) -> Self {
        self.atalho = Some(atalho);
        self
    }

    /// O `.acao.pri`: o degradê do botão principal (no máximo um por tela).
    pub fn primaria(mut self, sim: bool) -> Self {
        self.primaria = sim;
        self
    }

    /// Desligado: 45% de opacidade, sem hover nem clique.
    pub fn habilitado(mut self, sim: bool) -> Self {
        self.habilitado = sim;
        self
    }
}

impl Widget for Acao<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let p = paleta(ui);
        let e = &p.estudio;
        let sense = if self.habilitado { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(vec2(LARGURA_ACAO, ALTURA_ACAO), sense);
        let foco = self.habilitado && resp.has_focus() && pintura::foco_visivel(ui.ctx());
        let hover = ui.ctx().animate_bool_with_time(
            resp.id.with("hover"),
            self.habilitado && (resp.hovered() || foco),
            duracao_transicao(ui),
        );
        if self.habilitado && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if ui.is_rect_visible(rect.expand(ALCANCE_HALO)) {
            let mut painter = ui.painter().clone();
            let mut fora = pintor_de_fora(ui, rect);
            if !self.habilitado {
                painter.multiply_opacity(0.45);
                fora.multiply_opacity(0.45);
            }
            let volume = if self.primaria { volume_primario(p) } else { volume_neutro(p) };
            let estado = EstadoVolume { presenca: 1.0, hover, pressao: 0.0 };
            pintar_volume(&painter, &fora, p, rect, RAIO_ACAO, &volume, estado);

            // o círculo do ícone: `--tint` e `--acct`; no primário, branco a 18% e branco
            let (fundo_circulo, cor_icone, cor_texto, borda_kbd) = if self.primaria {
                (rgba(255, 255, 255, 0.18), Color32::WHITE, Color32::WHITE, rgba(255, 255, 255, 0.4))
            } else {
                (e.tint, e.acct, e.t, e.b2)
            };
            let centro = pos2(rect.left() + RECUO + LADO_CIRCULO / 2.0, rect.center().y);
            painter.circle_filled(centro, LADO_CIRCULO / 2.0, fundo_circulo);
            icones::pintar(
                &painter,
                Rect::from_center_size(centro, vec2(LADO_SILHUETA, LADO_SILHUETA)),
                self.icone,
                cor_icone,
            );

            // o nome e a etiqueta, centrados na altura do cartão (`align-items:center`)
            let x = rect.left() + RECUO + LADO_CIRCULO + GAP;
            let disponivel = (rect.right() - RECUO - x).max(0.0);
            let bloco = if self.atalho.is_some() { ALTURA_NOME + MARGEM_KBD + ALTURA_KBD } else { ALTURA_NOME };
            let topo = rect.center().y - bloco / 2.0;
            let g = galley_cortado(ui, self.titulo, fontes::fonte(13.0, Peso::Medio), cor_texto, disponivel);
            painter.galley(pos2(x, topo + (ALTURA_NOME - g.size().y) / 2.0), g, cor_texto);
            if let Some(atalho) = self.atalho {
                // `.kbd{font-size:10.5px;padding:0 4px;border:1px solid;border-radius:4px}`; no primário, branco
                let cor_kbd = if self.primaria { Color32::WHITE } else { e.t3 };
                let g = galley(ui, atalho, fontes::fonte(10.5, Peso::Normal), cor_kbd);
                let largura = (g.size().x + 2.0 * 4.0 + 2.0).ceil().min(disponivel);
                let caixa = Rect::from_min_size(pos2(x, topo + ALTURA_NOME + MARGEM_KBD), vec2(largura, ALTURA_KBD));
                painter.rect_stroke(caixa, 4.0, Stroke::new(1.0, borda_kbd), StrokeKind::Inside);
                painter.galley(caixa.center() - g.size() / 2.0, g, cor_kbd);
            }
            if foco {
                pintura::anel_foco(&fora, p, rect, RAIO_ACAO);
            }
        }
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, self.habilitado, self.titulo));
        resp
    }
}
