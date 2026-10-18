//! Barra de progresso do visual Estúdio (`.barra` e `.barra>div` das pranchetas): trilho rebaixado de 8px em
//! pílula (fundo `--f`, borda `--fb`, `inset 0 1px 2px rgba(0,0,0,.35)`) e o preenchimento com o volume do botão
//! principal (`--volpri` com as riscas, realce `inset 0 1px 0 rgba(255,255,255,.35)` e o halo de 10px em
//! `--brilhof`, que o `overflow:hidden` do trilho corta). Com total, a largura anda com transição de 320ms; sem
//! total (consulta, banco que não se conta) um trecho de 32% vai e volta devagar (1,6s); com movimento reduzido
//! fica parado e cheio a 45%.

use std::time::Duration;

use egui::epaint::RectShape;
use egui::{Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::pecas::botao::realce_curvo;
use crate::pintura::{self, Preenchimento};
use crate::tema::{Paleta, movimento_reduzido, paleta, rgba};

/// Altura da barra (`.barra{height:8px}`).
pub const ALTURA_BARRA: f32 = 8.0;

/// `cubic-bezier(.22,.61,.36,1)` (`--cr-ease-out`) em `t`.
pub fn ease_out(t: f32) -> f32 {
    // resolve x(s) = t por bisseção e devolve y(s)
    let (x1, y1, x2, y2) = (0.22_f32, 0.61_f32, 0.36_f32, 1.0_f32);
    let bez = |a: f32, b: f32, s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s;
    let t = t.clamp(0.0, 1.0);
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let meio = (lo + hi) / 2.0;
        if bez(x1, x2, meio) < t {
            lo = meio;
        } else {
            hi = meio;
        }
    }
    bez(y1, y2, (lo + hi) / 2.0)
}

/// O preenchimento (`.barra>div`) em `cheio`, cortado pelo trilho em `corte`: o halo, o degradê do destaque com
/// as riscas e o realce de cima.
fn pintar_cheio(corte: &Painter, p: &Paleta, cheio: Rect) {
    if cheio.width() <= 0.0 {
        return;
    }
    pintura::halo(corte, cheio, f32::INFINITY, 10.0, p.estudio.brilho_forte);
    pintura::preencher(corte, p, cheio, f32::INFINITY, Preenchimento::Primario);
    pintura::riscas(corte, p, cheio, f32::INFINITY);
    realce_curvo(corte, cheio, f32::INFINITY, rgba(255, 255, 255, 0.35));
}

/// A barra na largura disponível (no mínimo 130px). `fracao`: `Some(0.0..=1.0)` com total, `None` indeterminada.
pub fn barra_progresso(ui: &mut Ui, fracao: Option<f32>) -> Response {
    let p = paleta(ui);
    let e = &p.estudio;
    let largura = ui.available_width().max(130.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, ALTURA_BARRA), Sense::hover());
    let reduzido = movimento_reduzido(ui);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let cantos = pintura::canto(rect, f32::INFINITY);
        // o trilho: fundo, a sombra interna em cima (dentro da borda) e a borda
        painter.add(RectShape::filled(rect, cantos, e.f));
        let dentro = rect.shrink(1.0);
        pintura::sombra_interna_topo(painter, dentro, f32::INFINITY, 3.0, rgba(0, 0, 0, 0.35));
        // o preenchimento ocupa a altura de dentro e é cortado por ela (`overflow:hidden`)
        let corte = painter.with_clip_rect(dentro.intersect(painter.clip_rect()));
        match fracao {
            Some(f) => {
                let alvo = f.clamp(0.0, 1.0);
                let atual =
                    if reduzido { alvo } else { ui.ctx().animate_value_with_time(resp.id.with("fracao"), alvo, 0.32) };
                if atual > 0.0 {
                    let cheio = Rect::from_min_size(dentro.min, vec2(dentro.width() * atual, dentro.height()));
                    pintar_cheio(&corte, p, cheio);
                }
            }
            None if reduzido => {
                let mut fraco = corte.clone();
                fraco.multiply_opacity(0.45);
                pintar_cheio(&fraco, p, dentro);
            }
            None => {
                let t = ui.input(|i| i.time) as f32;
                let ciclo = (t % 3.2) / 1.6; // ida e volta (alternate)
                let fase = if ciclo <= 1.0 { ciclo } else { 2.0 - ciclo };
                let desloc = -0.20 + 2.52 * ease_out(fase);
                let trecho = dentro.width() * 0.32;
                let x = dentro.left() + trecho * desloc;
                let cheio = Rect::from_min_max(pos2(x, dentro.top()), pos2(x + trecho, dentro.bottom()));
                pintar_cheio(&corte, p, cheio);
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
        }
        painter.add(RectShape::stroke(rect, cantos, Stroke::new(1.0, e.fb), StrokeKind::Inside));
    }
    resp.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::ProgressIndicator);
        info.value = fracao.map(|f| f64::from(f) * 100.0);
        info
    });
    resp
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ease_out_vai_de_zero_a_um() {
        assert!(ease_out(0.0).abs() < 1e-3);
        assert!((ease_out(1.0) - 1.0).abs() < 1e-3);
        assert!(ease_out(0.5) > 0.5, "ease-out anda mais no começo");
    }
}
