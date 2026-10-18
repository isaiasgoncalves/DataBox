//! Etiquetas do visual Estúdio. [`pill`] é **estado** (`.pill`: verificadas, só leitura, não encontrado); [`tag`]
//! é **coisa** (tabela referenciada, nome de tabela que entra no SQL); [`chip`] é a etiqueta em pílula com a
//! borda na cor de destaque (`.chip`: o "BLOB · 2,3 KB" da grade e da janela da linha) e [`chip_filtro`] é a
//! etiqueta de filtro com o × (`.chip b`, `.chip .x`, na faixa de contexto dos Dados). [`etiqueta_mais`] é o "+N"
//! das regras que não cabem, com a lista na dica.

use egui::text::LayoutJob;
use egui::{Color32, CursorIcon, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::botao::{pintar_x, pintor_de_fora, realce_curvo};
use crate::pecas::texto::{formato, galley, galley_caps};
use crate::pintura;
use crate::tema::{Paleta, SombraCss, duracao_transicao, paleta, rgba};

/// A cor da pill (`.pill`, `.pill.ok`, `.pill.ac`, e os estados de perigo, espera e apagado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TipoPill {
    /// `.pill`: `--t2` sobre `--h`.
    #[default]
    Neutra,
    /// `.pill.ok`: verde (verificadas, único, ativo).
    Ok,
    /// Perigo (inativo, reprovado): `--perigo` sobre ele a 10%.
    No,
    /// Espera (só leitura, bloqueado): `--aviso` sobre ele a 12%.
    Wait,
    /// `.pill.ac`: a cor de destaque (chave primária, admin): `--acct` sobre `--tint`.
    Acc,
    /// Apagada (sem rowid, virtual, gerada, desligadas): `--t3` sobre `--h`.
    Mute,
}

/// O texto e o fundo da pill. O verde do texto é o `ok_texto` (no claro, mais escuro que o `--ok`, para os 10px
/// passarem de 4,5:1 sobre o fundo verde).
pub fn cores_pill(p: &Paleta, tipo: TipoPill) -> (Color32, Color32) {
    let e = &p.estudio;
    match tipo {
        TipoPill::Neutra => (e.t2, e.h),
        TipoPill::Ok => (e.ok_texto, p.aqua_ok_bg),
        TipoPill::No => (p.danger_fg, p.aqua_danger_bg),
        TipoPill::Wait => (p.wait_fg, p.aqua_wait_bg),
        TipoPill::Acc => (e.acct, e.tint),
        TipoPill::Mute => (e.t3, e.h),
    }
}

/// `.pill`: 10px / 700, caixa alta, `letter-spacing:.05em`.
fn galley_pill(ui: &Ui, texto: &str, cor: Color32) -> std::sync::Arc<egui::Galley> {
    galley_caps(ui, texto, 10.0, 0.05, cor)
}

/// Pinta uma pill no retângulo (para quem desenha sem alocar, como a grade).
pub fn pintar_pill(ui: &Ui, rect: Rect, texto: &str, tipo: TipoPill) {
    let p = paleta(ui);
    let (cor, fundo) = cores_pill(p, tipo);
    let painter = ui.painter();
    painter.rect_filled(rect, pintura::canto(rect, f32::INFINITY), fundo);
    let g = galley_pill(ui, texto, cor);
    let pos = pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0);
    painter.galley(pos, g, cor);
}

/// Tamanho de uma pill com este texto (`padding:1px 7px`, linha de 14px: 16px de altura).
pub fn tamanho_pill(ui: &Ui, texto: &str) -> egui::Vec2 {
    let g = galley_pill(ui, texto, Color32::PLACEHOLDER);
    vec2((g.size().x + 14.0).ceil(), 16.0)
}

/// `span.pill`: 10px / 700 em caixa alta, `padding:1px 7px`, em pílula. Estado nunca só por cor: a pill sempre
/// tem texto.
pub fn pill(ui: &mut Ui, texto: &str, tipo: TipoPill) -> Response {
    let tamanho = tamanho_pill(ui, texto);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
    if ui.is_rect_visible(rect) {
        pintar_pill(ui, rect, texto, tipo);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, texto));
    resp
}

/// A etiqueta "+N" das regras que não couberam na linha (item 18 da revisão): a [`pill`] neutra com "+N" e a lista
/// inteira na dica (`dica`); o leitor de tela ouve "+N" com a lista.
pub fn etiqueta_mais(ui: &mut Ui, n: usize, dica: &str) -> Response {
    let texto = format!("+{n}");
    let tamanho = tamanho_pill(ui, &texto);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
    if ui.is_rect_visible(rect) {
        pintar_pill(ui, rect, &texto, TipoPill::Neutra);
    }
    let rotulo = format!("{texto}: {dica}");
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &rotulo));
    resp.on_hover_text(dica)
}

/// Como [`pill`], clicável: no hover ganha mais uma camada de `--h` e o halo de 10px em `--brilho`.
pub fn pill_clicavel(ui: &mut Ui, texto: &str, tipo: TipoPill) -> Response {
    let tamanho = tamanho_pill(ui, texto);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), duracao_transicao(ui));
    if ui.is_rect_visible(rect.expand(16.0)) {
        let p = paleta(ui);
        if hover > 0.0 {
            pintura::halo(&pintor_de_fora(ui, rect), rect, f32::INFINITY, 10.0, p.estudio.brilho.gamma_multiply(hover));
        }
        pintar_pill(ui, rect, texto, tipo);
        if hover > 0.0 {
            ui.painter().rect_filled(rect, pintura::canto(rect, f32::INFINITY), p.estudio.h.gamma_multiply(hover));
        }
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(ui.painter(), p, rect, f32::INFINITY);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, texto));
    resp
}

/// Forma da tag (`.tag`, `.tag.dash` para "+ vincular").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoTag {
    /// Na tinta do destaque: `--acct` sobre `--tint`.
    #[default]
    Ouro,
    /// Tracejada, transparente (`.tag.dash`): o tracejado em `--fb`, na cor de destaque no hover.
    Tracejada,
}

/// Raio da tag (não é pílula: é "coisa", não estado).
const RAIO_TAG: f32 = 7.0;

fn pintar_tag(ui: &Ui, rect: Rect, texto: &str, tipo: TipoTag, hover: f32, pressao: f32) {
    let p = paleta(ui);
    let e = &p.estudio;
    let painter = ui.painter();
    let cor_texto = match tipo {
        TipoTag::Ouro => {
            // hover como o item de menu (`.mi:hover`): `--sel`, o realce e o halo de 12px em `--brilho`
            if hover > 0.0 {
                let halo = e.brilho.gamma_multiply(hover * (1.0 - pressao));
                pintura::halo(&pintor_de_fora(ui, rect), rect, RAIO_TAG, 12.0, halo);
            }
            painter.rect_filled(rect, RAIO_TAG, pintura::misturar(e.tint, e.sel, hover));
            pintura::sombra_interna_topo(painter, rect, RAIO_TAG, 4.0, rgba(0, 0, 0, 0.25).gamma_multiply(pressao));
            realce_curvo(painter, rect, RAIO_TAG, rgba(255, 255, 255, 0.08).gamma_multiply(hover));
            e.acct
        }
        TipoTag::Tracejada => {
            let borda = pintura::misturar(e.fb, e.acc, hover);
            let r = rect.shrink(0.5);
            let pontos = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            for par in pontos.windows(2) {
                painter.extend(egui::Shape::dashed_line(&[par[0], par[1]], Stroke::new(1.0, borda), 3.0, 2.0));
            }
            pintura::misturar(e.t2, e.t, hover)
        }
    };
    let g = galley(ui, texto, fontes::fonte(11.0, Peso::Medio), cor_texto);
    painter.galley(pos2(rect.left() + 11.0, rect.center().y - g.size().y / 2.0), g, cor_texto);
}

fn tamanho_tag(ui: &Ui, texto: &str) -> egui::Vec2 {
    let g = galley(ui, texto, fontes::fonte(11.0, Peso::Medio), Color32::PLACEHOLDER);
    // padding 4px 11px, linha de 16,5px (11px × 1,5)
    vec2((g.size().x + 22.0).ceil(), 24.5)
}

/// `span.tag`: raio 7px, 11px / 600, `--acct` sobre `--tint`.
pub fn tag(ui: &mut Ui, texto: &str) -> Response {
    let tamanho = tamanho_tag(ui, texto);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
    if ui.is_rect_visible(rect) {
        pintar_tag(ui, rect, texto, TipoTag::Ouro, 0.0, 0.0);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, texto));
    resp
}

/// `button.tag` (as tabelas da Consulta SQL) ou `button.tag.dash` ("+ vincular"): clicável; no hover, `--sel`
/// com o realce e o halo de 12px (a tracejada fica na cor de destaque); no clique, afunda.
pub fn tag_clicavel(ui: &mut Ui, texto: &str, tipo: TipoTag) -> Response {
    let tamanho = tamanho_tag(ui, texto);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::click());
    let tempo = duracao_transicao(ui);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), tempo);
    let pressao = ui.ctx().animate_bool_with_time(resp.id.with("pressao"), resp.is_pointer_button_down_on(), tempo);
    if ui.is_rect_visible(rect.expand(20.0)) {
        pintar_tag(ui, rect, texto, tipo, hover, pressao);
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(ui.painter(), paleta(ui), rect, RAIO_TAG);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), texto));
    resp
}

// ------------------------------------------------------------------ chip

/// Altura do chip (`.chip{height:24px}`).
pub const ALTURA_CHIP: f32 = 24.0;

/// `0 1px 3px rgba(0,0,0,.25)` do `.chip`.
const SOMBRA_CHIP: SombraCss = SombraCss { y: 1.0, desfoque: 3.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.25) };

/// O fundo do chip: `--tint` com a borda do destaque a 45%, o realce `inset 0 1px 0 rgba(255,255,255,.08)` e a
/// sombra `0 1px 3px rgba(0,0,0,.25)`; o apagado (`mute`) é `--h` com a borda `--b2`, sem sombra.
fn pintar_fundo_chip(painter: &egui::Painter, p: &Paleta, rect: Rect, mute: bool) {
    let e = &p.estudio;
    let r = f32::INFINITY;
    let canto = pintura::canto(rect, r);
    if mute {
        painter.rect_filled(rect, canto, e.h);
        painter.rect_stroke(rect, canto, Stroke::new(1.0, e.b2), StrokeKind::Inside);
    } else {
        pintura::sombra(painter, rect, r, SOMBRA_CHIP);
        painter.rect_filled(rect, canto, e.tint);
        realce_curvo(painter, rect.shrink(1.0), r, rgba(255, 255, 255, 0.08));
        painter.rect_stroke(rect, canto, Stroke::new(1.0, e.borda_chip), StrokeKind::Inside);
    }
}

/// Pinta um chip (`.chip`: 24px, pílula, 12px em `--t2`, `padding:0 10px`; `mute` = apagado, `--t3`) no
/// retângulo.
pub fn pintar_chip(ui: &Ui, rect: Rect, texto: &str, mute: bool) {
    pintar_chip_em(ui.painter(), paleta(ui), rect, texto, mute);
}

/// [`pintar_chip`] com um `Painter` (sem `Ui`; a grade usa).
pub fn pintar_chip_em(painter: &egui::Painter, p: &Paleta, rect: Rect, texto: &str, mute: bool) {
    pintar_fundo_chip(painter, p, rect, mute);
    let cor = if mute { p.estudio.t3 } else { p.estudio.t2 };
    let job = LayoutJob::single_section(texto.to_owned(), formato(fontes::fonte(12.0, Peso::Normal), cor));
    let g = painter.layout_job(job);
    painter.galley(pos2(rect.left() + 11.0, rect.center().y - g.size().y / 2.0), g, cor);
}

/// Tamanho de um chip com este texto (24px de altura; `padding:0 10px` e a borda de 1px).
pub fn tamanho_chip(ui: &Ui, texto: &str) -> egui::Vec2 {
    tamanho_chip_em(ui.painter(), texto)
}

/// [`tamanho_chip`] com um `Painter`.
pub fn tamanho_chip_em(painter: &egui::Painter, texto: &str) -> egui::Vec2 {
    let job =
        LayoutJob::single_section(texto.to_owned(), formato(fontes::fonte(12.0, Peso::Normal), Color32::PLACEHOLDER));
    let g = painter.layout_job(job);
    vec2((g.size().x + 22.0).ceil(), ALTURA_CHIP)
}

/// `span.chip` (ou o apagado, `mute`).
pub fn chip(ui: &mut Ui, texto: &str, mute: bool) -> Response {
    let tamanho = tamanho_chip(ui, texto);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
    if ui.is_rect_visible(rect.expand(6.0)) {
        pintar_chip(ui, rect, texto, mute);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, texto));
    resp
}

/// O que o [`chip_filtro`] devolve.
#[derive(Debug, Clone)]
pub struct RespostaChip {
    /// O chip inteiro.
    pub resposta: Response,
    /// O × foi clicado (a tela tira o filtro).
    pub fechar: bool,
    /// O corpo da etiqueta foi clicado (fora do ×): a tela abre a condição para editar.
    pub abrir: bool,
}

/// A etiqueta de filtro (`<span class="chip"><b>ativo</b> igual a <b>1</b><button class="x">`): os trechos
/// `(texto, forte)` — os fortes em `--t` 600, os outros em `--t2` — e o × redondo de 18px (`.chip .x`, hover
/// `--h` com o ícone em `--t`). `padding:0 3px 0 10px`; no `inline-flex` do chip cada trecho é um item, com
/// `gap:5px` entre eles (os espaços das pontas não contam). `rotulo_x` é o verbo do × para o leitor de tela
/// ("Tirar o filtro", que ele ouve seguido da condição: "Tirar o filtro ativo igual a 1"). O corpo é um botão
/// ("Editar o filtro ativo igual a 1") (a tela abre a condição para editar, [`RespostaChip::abrir`]): no hover
/// ganha o `--h` por cima e o halo de 10px em `--brilho`, como o item em hover do README.
pub fn chip_filtro(ui: &mut Ui, trechos: &[(&str, bool)], rotulo_x: &str) -> RespostaChip {
    let p = paleta(ui);
    let e = &p.estudio;
    let mut job = LayoutJob::default();
    for (i, &(texto, forte)) in trechos.iter().enumerate() {
        let (peso, cor) = if forte { (Peso::Medio, e.t) } else { (Peso::Normal, e.t2) };
        let gap = if i == 0 { 0.0 } else { 5.0 };
        job.append(texto.trim(), gap, formato(fontes::fonte(12.0, peso), cor));
    }
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let largura = (1.0 + 10.0 + g.size().x + 5.0 + 18.0 + 3.0 + 1.0).ceil();
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, ALTURA_CHIP), Sense::click());
    let rect_x = Rect::from_center_size(pos2(rect.right() - 1.0 - 3.0 - 9.0, rect.center().y), vec2(18.0, 18.0));
    let x = ui.interact(rect_x, resp.id.with("x"), Sense::click());
    let tempo = duracao_transicao(ui);
    let hover_x = ui.ctx().animate_bool_with_time(x.id.with("hover"), x.hovered(), tempo);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered() && !x.hovered(), tempo);
    if ui.is_rect_visible(rect.expand(16.0)) {
        if hover > 0.0 {
            pintura::halo(&pintor_de_fora(ui, rect), rect, f32::INFINITY, 10.0, e.brilho.gamma_multiply(hover));
        }
        let painter = ui.painter();
        pintar_fundo_chip(painter, p, rect, false);
        if hover > 0.0 {
            painter.rect_filled(rect, pintura::canto(rect, f32::INFINITY), e.h.gamma_multiply(hover));
        }
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(painter, p, rect, f32::INFINITY);
        }
        painter.galley(pos2(rect.left() + 11.0, rect.center().y - g.size().y / 2.0), g, e.t2);
        if hover_x > 0.0 {
            painter.circle_filled(rect_x.center(), 9.0, e.h.gamma_multiply(hover_x));
        }
        pintar_x(painter, rect_x.center(), 11.0, pintura::misturar(e.t2, e.t, hover_x));
        if x.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(painter, p, rect_x, f32::INFINITY);
        }
    }
    if x.hovered() || resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    // o leitor de tela ouve o que cada botão faz com qual filtro: "Editar o filtro ativo igual a 1" e "Tirar o
    // filtro ativo igual a 1" (com vários filtros, só o verbo não diria qual)
    let texto = trechos.iter().map(|(t, _)| t.trim()).collect::<Vec<_>>().join(" ");
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), format!("Editar o filtro {texto}")));
    x.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), format!("{rotulo_x} {texto}")));
    let fechar = x.clicked();
    let abrir = resp.clicked() && !fechar;
    RespostaChip { resposta: resp, fechar, abrir }
}
