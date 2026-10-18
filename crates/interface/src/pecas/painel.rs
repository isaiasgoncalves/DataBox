//! Os painéis fixos da casca (o `.vidro` das regras do fim de `Main.dc.html`: a barra de ferramentas, a barra de
//! ícones, o Explorador, o Histórico, as Propriedades e a barra de status): fundo `--painel`, borda `--vidrob`, o
//! realce `inset 0 1px 0 --vidroluz` e a sombra curta (`0 1px 3px`), raio [`RAIO_CARD`].
//!
//! - [`pintar_painel_fixo`] / [`forma_painel_fixo`]: só a superfície.
//! - [`Painel`]: a superfície com o cabeçalho `.cab`, uma faixa de 23px ([`ALTURA_CAB`], 0,6 dos 38 de antes,
//!   pedido de 16/10/2026) que não vai de borda a borda: ela tem a largura da linha selecionada da árvore (o
//!   `margin:0 6px` do `.no`, [`MARGEM_NO`], por dentro da borda do painel; pedido de 17/10/2026) e fica à mesma
//!   distância do alto (`.cab{margin:6px 6px 0}`), com os quatro cantos de [`RAIO_NO`]: o degradê `--bt1` →
//!   `--bt2` com as riscas, o realce `--btluz` e o fio `--btb` de 1px em volta toda; o título `.tit` de 10,5px em
//!   `--t2` a 12px da esquerda da faixa e as ações de 20px, [`Botao::do_cabecalho`], a 4px da direita dela) e o
//!   corpo embaixo da faixa, recortado no painel.
//!
//! [`Botao::do_cabecalho`]: crate::pecas::botao::Botao::do_cabecalho
//! - [`Secao`]: a seção `.sec` das Propriedades (32px, [`ALTURA_SECAO`]: a seta de 12px em `--t3`, o título em
//!   600 e o extra em `--t2`, com o fio `--vidrob` em cima a partir da segunda), que abre e fecha no clique. O
//!   rótulo para o leitor de tela é "Seção <título>" ("Seção Arquivo"): o título sozinho repetiria o de um menu.
//! - [`ficha`]: a ficha `.det` dentro de uma seção aberta (`padding:2px 14px 14px`).
//!
//! ```ignore
//! Painel::new("explorador", "Explorador")
//!     .acoes(|ui| { ui.add(Botao::icone(Icone::Recolher).do_cabecalho()); })
//!     .mostrar(ui, |ui| { /* a árvore */ });
//! ```

use egui::epaint::RectShape;
use egui::{
    Align, CursorIcon, Id, Layout, Painter, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
    WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::arvore::{MARGEM_NO, RAIO_NO};
use crate::pecas::botao::forma_realce_curvo;
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::texto::{galley, galley_cortado, galley_tit_em};
use crate::pintura::{self, Preenchimento};
use crate::tema::{Paleta, paleta};

/// Altura da faixa do cabeçalho do painel (`.cab{height:23px}`, com o fio de 1px em cima e embaixo; 0,6 dos 38 de
/// antes, pedido de 16/10/2026).
pub const ALTURA_CAB: f32 = 23.0;
/// A borda de 1px da superfície do painel.
const BORDA: f32 = 1.0;
/// O recuo da faixa `.cab` dentro do painel, nos lados e em cima: a borda mais a `margin:0 6px` da linha da árvore
/// ([`MARGEM_NO`]), para a faixa e a linha selecionada terem as mesmas bordas (`.cab{margin:6px 6px 0}` dentro
/// da borda). Pedido de 17/10/2026.
pub const RECUO_CAB: f32 = BORDA + MARGEM_NO;
/// O título do `.cab` (`.cab .tit{font-size:10.5px}`).
pub const TAMANHO_TITULO_CAB: f32 = 10.5;
/// `.cab{padding:0 4px 0 12px}`: o título a 12px da esquerda da faixa e as ações a 4px da direita dela.
const RECUO_TITULO_CAB: f32 = 12.0;
const RECUO_ACOES_CAB: f32 = 4.0;

/// A cor do título do `.cab`: o terciário cairia abaixo de 4,5:1 sobre o relevo do cabeçalho, então é o `--t2`
/// (revisão de 15/10/2026, item 1).
pub fn cor_titulo_cab(p: &Paleta) -> egui::Color32 {
    p.estudio.t2
}
/// Altura da seção das Propriedades (`.sec{height:32px}`).
pub const ALTURA_SECAO: f32 = 32.0;

/// A superfície do painel fixo, devolvendo a forma (para pintar num lugar reservado antes do conteúdo).
pub fn forma_painel_fixo(p: &Paleta, rect: Rect, raio: f32) -> Shape {
    let e = &p.estudio;
    let cantos = pintura::canto(rect, raio);
    Shape::Vec(vec![
        pintura::forma_sombra(rect, raio, e.sombra_painel),
        RectShape::filled(rect, cantos, e.painel).into(),
        RectShape::stroke(rect, cantos, Stroke::new(1.0, e.vidrob), StrokeKind::Inside).into(),
        forma_realce_curvo(rect.shrink(1.0), raio - 1.0, e.vidroluz),
    ])
}

/// Pinta a superfície do painel fixo (raio [`RAIO_CARD`] nos painéis e também na barra de status).
pub fn pintar_painel_fixo(painter: &Painter, p: &Paleta, rect: Rect, raio: f32) {
    painter.add(forma_painel_fixo(p, rect, raio));
}

/// Pinta a faixa do `.cab` no retângulo dela: o degradê neutro com as riscas nos quatro cantos de [`RAIO_NO`], o
/// fio `--btb` de 1px em volta toda (por dentro) e o realce `--btluz` por dentro do fio.
fn pintar_cab(painter: &Painter, p: &Paleta, rect: Rect) {
    let e = &p.estudio;
    let cantos = pintura::canto(rect, RAIO_NO);
    let mut fundo = pintura::forma_preenchida(painter.ctx(), p, rect, 0.0, Preenchimento::Aqua);
    if let Shape::Rect(f) = &mut fundo {
        f.corner_radius = cantos;
    }
    painter.add(fundo);
    pintura::riscas_com_canto(painter, p, rect, cantos);
    painter.add(RectShape::stroke(rect, cantos, Stroke::new(1.0, e.btb), StrokeKind::Inside));
    painter.add(forma_realce_curvo(rect.shrink(1.0), RAIO_NO - 1.0, e.btluz));
}

type Acoes<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// Um painel fixo com o cabeçalho `.cab` (o Explorador, o Histórico, as Propriedades). Ocupa o espaço que o `Ui`
/// tem (quem chama dá a ele o retângulo do painel).
#[must_use = "chame .mostrar(ui, |ui| …)"]
pub struct Painel<'a> {
    id: Id,
    titulo: &'a str,
    acoes: Option<Acoes<'a>>,
}

impl<'a> Painel<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, titulo: &'a str) -> Self {
        Painel { id: Id::new(("databox::painel", id_salt)), titulo, acoes: None }
    }

    /// As ações à direita do título (os [`Botao::do_cabecalho`] de 20px com 2px entre eles: Abrir pasta, Recolher
    /// tudo).
    ///
    /// [`Botao::do_cabecalho`]: crate::pecas::botao::Botao::do_cabecalho
    pub fn acoes(mut self, acoes: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.acoes = Some(Box::new(acoes));
        self
    }

    /// Desenha o painel e o corpo (`corpo` recebe o `Ui` do espaço embaixo do cabeçalho, recortado no painel).
    pub fn mostrar<R>(self, ui: &mut Ui, corpo: impl FnOnce(&mut Ui) -> R) -> R {
        let p = paleta(ui);
        let rect = ui.available_rect_before_wrap();
        pintar_painel_fixo(ui.painter(), p, rect, RAIO_CARD);
        // a faixa tem a largura da linha da árvore: o mesmo recuo dos lados e, em cima, o mesmo valor
        let cab = Rect::from_min_max(
            rect.min + vec2(RECUO_CAB, RECUO_CAB),
            pos2(rect.right() - RECUO_CAB, rect.top() + RECUO_CAB + ALTURA_CAB),
        );
        pintar_cab(ui.painter(), p, cab);
        // `.cab{padding:0 4px 0 12px;gap:2px}`: o título à esquerda, as ações à direita
        let linha = cab;
        let mut largura_acoes = 0.0;
        if let Some(acoes) = self.acoes {
            let dentro = Rect::from_min_max(
                pos2(linha.left() + RECUO_TITULO_CAB, linha.top()),
                pos2(linha.right() - RECUO_ACOES_CAB, linha.bottom()),
            );
            let mut ui_acoes = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("acoes"))
                    .max_rect(dentro)
                    .layout(Layout::right_to_left(Align::Center)),
            );
            ui_acoes.spacing_mut().item_spacing.x = 2.0;
            acoes(&mut ui_acoes);
            if ui_acoes.min_rect().is_positive() {
                largura_acoes = ui_acoes.min_rect().width() + 8.0;
            }
        }
        let cor = cor_titulo_cab(p);
        let g = galley_tit_em(ui, self.titulo, TAMANHO_TITULO_CAB, cor);
        let pos = pos2(linha.left() + RECUO_TITULO_CAB, linha.center().y - g.size().y / 2.0);
        let rect_titulo = Rect::from_min_size(pos, g.size());
        let limite = linha.right() - RECUO_ACOES_CAB - largura_acoes;
        ui.painter().with_clip_rect(linha.with_max_x(limite)).galley(pos, g, cor);
        let resp_titulo = ui.interact(rect_titulo, self.id.with("titulo"), Sense::hover());
        resp_titulo.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, self.titulo));

        let rect_corpo = Rect::from_min_max(pos2(rect.left() + BORDA, cab.bottom()), rect.max - vec2(BORDA, BORDA));
        let mut ui_corpo = ui.new_child(
            UiBuilder::new().id_salt(self.id.with("corpo")).max_rect(rect_corpo).layout(Layout::top_down(Align::Min)),
        );
        ui_corpo.set_clip_rect(rect_corpo.intersect(ui.clip_rect()));
        let r = corpo(&mut ui_corpo);
        ui.allocate_rect(rect, Sense::hover());
        r
    }
}

/// A ficha (`.det`, rótulo de 112px + o valor) dentro de uma [`Secao`] aberta: `padding:2px 14px 14px`.
pub fn ficha(ui: &mut Ui, conteudo: impl FnOnce(&mut crate::pecas::detalhes::Detalhes<'_>)) {
    let r = Rect::from_min_max(ui.cursor().min + vec2(14.0, 2.0), pos2(ui.max_rect().right() - 14.0, f32::INFINITY));
    let mut dentro = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
    crate::pecas::detalhes::detalhes(&mut dentro, conteudo);
    let altura = dentro.min_rect().height() + 2.0 + 14.0;
    ui.allocate_space(vec2(ui.available_width(), altura));
}

/// Uma seção das Propriedades (`.sec`), que abre e fecha no clique (aberta no começo).
#[must_use = "chame .mostrar(ui, |ui| …)"]
pub struct Secao<'a> {
    id: Id,
    titulo: &'a str,
    extra: Option<&'a str>,
    primeira: bool,
    perigo: bool,
    recolhida: bool,
}

impl<'a> Secao<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, titulo: &'a str) -> Self {
        Secao {
            id: Id::new(("databox::secao", id_salt)),
            titulo,
            extra: None,
            primeira: false,
            perigo: false,
            recolhida: false,
        }
    }

    /// O título e a seta em `--perigo` (o "Excluir a tabela" da Estrutura, prancheta 6).
    pub fn perigo(mut self, sim: bool) -> Self {
        self.perigo = sim;
        self
    }

    /// Nasce fechada (a zona de risco: o `details` fechado). Para ela nascer fechada de novo numa tela refeita, ponha
    /// no `id_salt` algo que muda com a tela.
    pub fn recolhida(mut self, sim: bool) -> Self {
        self.recolhida = sim;
        self
    }

    /// O texto depois do título, em 400 `--t2` (o nome da coluna em "Coluna email").
    pub fn extra(mut self, extra: &'a str) -> Self {
        self.extra = Some(extra);
        self
    }

    /// A primeira seção do painel, logo depois do `.cab`: sem o fio de cima.
    pub fn primeira(mut self, sim: bool) -> Self {
        self.primeira = sim;
        self
    }

    /// Está aberta?
    pub fn aberta(&self, ui: &Ui) -> bool {
        ui.ctx().data(|d| d.get_temp::<bool>(self.id)).unwrap_or(!self.recolhida)
    }

    /// O título e, aberta, o corpo. Devolve a resposta do título e o que o corpo devolveu.
    pub fn mostrar<R>(self, ui: &mut Ui, corpo: impl FnOnce(&mut Ui) -> R) -> (Response, Option<R>) {
        let p = paleta(ui);
        let e = &p.estudio;
        let aberta = self.aberta(ui);
        let largura = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(largura, ALTURA_SECAO), Sense::click());
        if resp.clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(self.id, !aberta));
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            if !self.primeira {
                painter.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, e.vidrob));
            }
            // `padding:0 12px;gap:6px`, a seta de 12px em `--t3`
            let seta = Rect::from_min_size(pos2(rect.left() + 12.0, rect.center().y - 6.0), vec2(12.0, 12.0));
            let icone = if aberta { Icone::SetaBaixo } else { Icone::SetaDireita };
            let (cor_seta, cor_titulo) = if self.perigo { (e.perigo, e.perigo) } else { (e.t3, e.t) };
            icones::pintar(painter, seta, icone, cor_seta);
            let x = seta.right() + 6.0;
            let g =
                galley_cortado(ui, self.titulo, fontes::fonte(12.0, Peso::Medio), cor_titulo, rect.right() - 12.0 - x);
            let fim = x + g.size().x;
            painter.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, cor_titulo);
            if let Some(extra) = self.extra {
                let ge = galley(ui, extra, fontes::fonte(12.0, Peso::Normal), e.t2);
                painter.with_clip_rect(rect).galley(pos2(fim + 6.0, rect.center().y - ge.size().y / 2.0), ge, e.t2);
            }
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(painter, p, rect.shrink(3.0), RAIO_NO);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let rotulo = format!("Seção {}", self.titulo);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::CollapsingHeader, ui.is_enabled(), &rotulo));
        ui.ctx().accesskit_node_builder(resp.id, |no| no.set_expanded(aberta));
        let r = aberta.then(|| corpo(ui));
        (resp, r)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn painel_fixo_tem_sombra_fundo_borda_e_realce() {
        use crate::tema::{CLARA, ESCURA};
        for p in [&CLARA, &ESCURA] {
            let Shape::Vec(formas) =
                forma_painel_fixo(p, Rect::from_min_size(pos2(0.0, 0.0), vec2(258.0, 400.0)), RAIO_CARD)
            else {
                panic!("o painel é uma lista de formas");
            };
            assert_eq!(formas.len(), 4);
            let Shape::Rect(fundo) = &formas[1] else { panic!("o fundo é um retângulo") };
            assert_eq!(fundo.fill, p.estudio.painel);
        }
    }
}
