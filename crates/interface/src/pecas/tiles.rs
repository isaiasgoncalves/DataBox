//! Os números da Visão geral (as pranchetas, `Main.dc.html`): grade automática de colunas de 150px no mínimo
//! (12px entre elas); cada caixa tem borda 1px `--b2`, raio 10, fundo `--c` e `padding:12px 16px`, com o número em
//! 24px / 350 (`line-height:1.1`) em `--t`, o sufixo ("KB", "há 5 dias") em 12px / 600 `--t3` na linha de base do
//! número, e o rótulo 4px abaixo, em 10.5px / 600, caixa alta, `.06em`, `--t3`. A caixa clicável ganha no hover a
//! borda `--acc` e o halo de 12px em `--brilho` (o `.atalho:hover`).

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{CursorIcon, Galley, Rect, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::botao::pintor_de_fora;
use crate::pecas::texto::{formato, galley, linha_de_base, y_na_linha};
use crate::pintura;
use crate::tema::{duracao_transicao, paleta};

/// Raio da caixa (o raio único, [`pintura::RAIO_PADRAO`]).
const RAIO: f32 = pintura::RAIO_PADRAO;
/// `padding:12px 16px` mais a borda de 1px.
const RECUO_X: f32 = 17.0;
const RECUO_Y: f32 = 13.0;
/// O número: 24px numa linha de 26,4px (`line-height:1.1`).
const TAMANHO_NUMERO: f32 = 24.0;
const LINHA_NUMERO: f32 = 26.4;
/// O rótulo: 10.5px numa linha de 14,7px, 4px abaixo do número.
const TAMANHO_ROTULO: f32 = 10.5;
const LINHA_ROTULO: f32 = 14.7;
const ESPACO_ROTULO: f32 = 4.0;
/// Altura de uma caixa.
const ALTURA: f32 = 2.0 * RECUO_Y + LINHA_NUMERO + ESPACO_ROTULO + LINHA_ROTULO;

/// Um número da Visão geral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile<'a> {
    /// O número já formatado ("12", "1,2", "5.000", "—").
    pub numero: &'a str,
    /// Sufixo depois do número ("KB", "há 5 dias"), em `--t3`; vazio = nenhum.
    pub sufixo: &'a str,
    /// O rótulo ("Tabelas", "Linhas (contadas)").
    pub rotulo: &'a str,
    /// Clicável (abre a lista correspondente).
    pub clicavel: bool,
}

impl<'a> Tile<'a> {
    pub fn new(numero: &'a str, rotulo: &'a str) -> Self {
        Tile { numero, sufixo: "", rotulo, clicavel: false }
    }

    pub fn sufixo(mut self, sufixo: &'a str) -> Self {
        self.sufixo = sufixo;
        self
    }

    pub fn clicavel(mut self, sim: bool) -> Self {
        self.clicavel = sim;
        self
    }
}

/// O rótulo em caixa alta: 10.5px / 600, `letter-spacing:.06em`.
fn galley_rotulo(ui: &Ui, texto: &str, cor: egui::Color32) -> Arc<Galley> {
    let mut f = formato(fontes::fonte(TAMANHO_ROTULO, Peso::Medio), cor);
    f.extra_letter_spacing = 0.06 * TAMANHO_ROTULO;
    ui.fonts_mut(|fs| fs.layout_job(LayoutJob::single_section(texto.to_uppercase(), f)))
}

/// Desenha os números na largura disponível; devolve o índice do clicado.
pub fn tiles(ui: &mut Ui, itens: &[Tile<'_>]) -> Option<usize> {
    if itens.is_empty() {
        return None;
    }
    let p = paleta(ui);
    let e = &p.estudio;
    let largura = ui.available_width();
    let espaco = 12.0;
    // auto-fit: quantas colunas de 150px cabem; com menos itens, os que há se esticam
    let cabem = (((largura + espaco) / (150.0 + espaco)).floor() as usize).max(1);
    let colunas = cabem.min(itens.len());
    let largura_tile = ((largura - espaco * (colunas as f32 - 1.0)) / colunas as f32).max(0.0);
    let linhas = itens.len().div_ceil(colunas);
    let topo = ui.cursor().min;
    let total = Rect::from_min_size(topo, vec2(largura, linhas as f32 * ALTURA + (linhas as f32 - 1.0) * espaco));
    let base_id = ui.next_auto_id();
    ui.allocate_rect(total, Sense::hover());
    let mut clicado = None;
    let tempo = duracao_transicao(ui);
    for (i, t) in itens.iter().enumerate() {
        let (l, c) = (i / colunas, i % colunas);
        let rect = Rect::from_min_size(
            pos2(topo.x + c as f32 * (largura_tile + espaco), topo.y + l as f32 * (ALTURA + espaco)),
            vec2(largura_tile, ALTURA),
        );
        let sense = if t.clicavel { Sense::click() } else { Sense::hover() };
        let resp = ui.interact(rect, base_id.with(i), sense);
        let foco = t.clicavel && resp.has_focus() && pintura::foco_visivel(ui.ctx());
        let hover = if t.clicavel {
            ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered() || foco, tempo)
        } else {
            0.0
        };
        if resp.clicked() {
            clicado = Some(i);
        }
        if ui.is_rect_visible(rect.expand(18.0)) {
            let painter = ui.painter();
            if hover > 0.0 {
                // `.atalho:hover{border-color:--acc; box-shadow:0 0 12px --brilho}`
                pintura::halo(&pintor_de_fora(ui, rect), rect, RAIO, 12.0, e.brilho.gamma_multiply(hover));
            }
            painter.rect_filled(rect, RAIO, e.doc);
            let borda = pintura::misturar(e.b2, e.acc, hover);
            painter.rect_stroke(rect, RAIO, Stroke::new(1.0, borda), StrokeKind::Inside);
            let x = rect.left() + RECUO_X;
            let topo_numero = rect.top() + RECUO_Y;
            let gn = galley(ui, t.numero, fontes::fonte(TAMANHO_NUMERO, Peso::Leve), e.t);
            let y_numero = y_na_linha(topo_numero, LINHA_NUMERO, &gn);
            let base = y_numero + linha_de_base(&gn);
            let largura_numero = gn.size().x;
            painter.galley(pos2(x, y_numero), gn, e.t);
            if !t.sufixo.is_empty() {
                // o `span` de 12px / 600 em `--t3`, na linha de base do número, depois de um espaço
                let gs = galley(ui, t.sufixo, fontes::fonte(12.0, Peso::Medio), e.t3);
                let espaco_texto = 0.274 * TAMANHO_NUMERO;
                let y = base - linha_de_base(&gs);
                painter.galley(pos2(x + largura_numero + espaco_texto, y), gs, e.t3);
            }
            let gr = galley_rotulo(ui, t.rotulo, e.t3);
            let topo_rotulo = topo_numero + LINHA_NUMERO + ESPACO_ROTULO;
            painter.galley(pos2(x, y_na_linha(topo_rotulo, LINHA_ROTULO, &gr)), gr, e.t3);
            if foco {
                pintura::anel_foco(painter, p, rect, RAIO);
            }
        }
        if t.clicavel && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let tipo = if t.clicavel { WidgetType::Button } else { WidgetType::Label };
        resp.widget_info(|| WidgetInfo::labeled(tipo, true, format!("{} {} {}", t.numero, t.sufixo, t.rotulo)));
    }
    clicado
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caixa_tem_a_altura_das_pranchetas() {
        // `padding:12px 16px` + a borda, número de 26,4px, 4px e o rótulo de 14,7px
        assert!((ALTURA - (2.0 * 13.0 + 26.4 + 4.0 + 14.7)).abs() < 1e-4);
    }
}
