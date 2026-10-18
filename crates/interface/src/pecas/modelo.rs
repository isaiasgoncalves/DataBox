//! O trecho expansível (`details.modelo`): um gatilho por item na aba Índices e gatilhos. Fio `--b` em cima (o
//! primeiro sem fio e sem o padding de cima), `padding:12px 0`; o `summary` em linha (12px `--t`, `gap:8px`) com a
//! seta da árvore das pranchetas (o `.no .seta`: chevron de 10px em `--t3`, para a direita fechado e para baixo
//! aberto); aberto, o conteúdo vem 8px abaixo. O estado aberto fica guardado no contexto.
//!
//! ```ignore
//! for (i, g) in gatilhos.iter().enumerate() {
//!     Modelo::new(("gatilho", &g.nome), &g.nome).primeiro(i == 0).mostrar(ui, |ui| {
//!         TextoPre::sql(&g.sql).mostrar(ui);
//!         em_linha(ui, 8.0, |ui| { ui.add(Botao::new("Copiar")); ui.add(Botao::ghost_perigo("Excluir")); });
//!     });
//! }
//! ```

use egui::epaint::{PathShape, PathStroke};
use egui::{Color32, CursorIcon, Id, Painter, Pos2, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::texto::galley_cortado;
use crate::pintura;
use crate::tema::paleta;

/// Lado da seta (`.no .seta{width:10px;height:10px}`).
pub(crate) const LADO_SETA: f32 = 10.0;

/// A seta das sanfonas (o `.no .seta` da árvore): o chevron `M9 6l6 6-6 6` (fechado, para a direita) ou
/// `M6 9l6 6 6-6` (aberto, para baixo) da grade de 24, numa caixa de 10px centrada em `centro`.
pub(crate) fn pintar_seta_sanfona(painter: &Painter, centro: Pos2, aberto: bool, cor: Color32) {
    // o `stroke-width:1.6` da grade de 24, com um mínimo legível
    let traco = PathStroke::new((1.6 * LADO_SETA / 24.0).max(1.0), cor);
    painter.add(PathShape::line(pontos_seta(centro, aberto).to_vec(), traco));
}

/// Os três pontos do chevron (o do meio é a ponta).
fn pontos_seta(centro: Pos2, aberto: bool) -> [Pos2; 3] {
    let k = LADO_SETA / 24.0;
    let ponto = |x: f32, y: f32| centro + vec2((x - 12.0) * k, (y - 12.0) * k);
    if aberto {
        [ponto(6.0, 9.0), ponto(12.0, 15.0), ponto(18.0, 9.0)]
    } else {
        [ponto(9.0, 6.0), ponto(15.0, 12.0), ponto(9.0, 18.0)]
    }
}

/// O que o [`Modelo`] devolve.
#[derive(Debug)]
pub struct RespostaModelo<R> {
    /// A resposta do `summary` (com `clicked()` quando alternou).
    pub resposta: Response,
    /// Aberto depois deste quadro.
    pub aberto: bool,
    /// O que o conteúdo devolveu (só aberto).
    pub inner: Option<R>,
}

/// Um `details.modelo`.
#[must_use = "chame .mostrar(ui, |ui| …)"]
pub struct Modelo<'a> {
    id: Id,
    resumo: &'a str,
    primeiro: bool,
    aberto_inicial: bool,
}

impl<'a> Modelo<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, resumo: &'a str) -> Self {
        Modelo { id: Id::new(("databox::modelo", id_salt)), resumo, primeiro: false, aberto_inicial: false }
    }

    /// O primeiro da lista (`.modelo:first-of-type`): sem o fio e sem o padding de cima.
    pub fn primeiro(mut self, sim: bool) -> Self {
        self.primeiro = sim;
        self
    }

    /// Nasce aberto (o padrão é fechado, como o `details`).
    pub fn aberto_inicial(mut self, sim: bool) -> Self {
        self.aberto_inicial = sim;
        self
    }

    /// Está aberto? (o estado guardado de um `Modelo` com este `id_salt`)
    pub fn esta_aberto(ui: &Ui, id_salt: impl std::hash::Hash + std::fmt::Debug) -> bool {
        ui.ctx().data(|d| d.get_temp::<bool>(Id::new(("databox::modelo", id_salt)))).unwrap_or(false)
    }

    pub fn mostrar<R>(self, ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> RespostaModelo<R> {
        let id = self.id;
        let mut aberto: bool = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(self.aberto_inicial);
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let p = paleta(ui);
            let e = &p.estudio;
            let largura = ui.available_width();
            if !self.primeiro {
                let topo = ui.cursor().min;
                ui.painter().hline(topo.x..=topo.x + largura, topo.y + 0.5, egui::Stroke::new(1.0, e.b));
                ui.add_space(13.0);
            }
            // o `summary`: a seta (10px) e o texto (12px) numa linha de 17px, 8px entre eles
            let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 17.0), Sense::click());
            if resposta.clicked() {
                aberto = !aberto;
                ui.ctx().data_mut(|d| d.insert_temp(id, aberto));
            }
            let x_texto = rect.left() + LADO_SETA + 8.0;
            let gt = galley_cortado(
                ui,
                self.resumo,
                fontes::fonte(12.0, Peso::Normal),
                e.t,
                (rect.right() - x_texto).max(10.0),
            );
            pintar_seta_sanfona(ui.painter(), pos2(rect.left() + LADO_SETA / 2.0, rect.center().y), aberto, e.t3);
            let largura_texto = gt.size().x;
            ui.painter().galley(pos2(x_texto, rect.center().y - gt.size().y / 2.0), gt, e.t);
            if resposta.has_focus() && pintura::foco_visivel(ui.ctx()) {
                let alvo = egui::Rect::from_min_max(rect.min, pos2(x_texto + largura_texto, rect.max.y));
                pintura::anel_foco(ui.painter(), p, alvo, 4.0);
            }
            if resposta.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let resumo = self.resumo;
            resposta.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, aberto, resumo));
            let inner = if aberto {
                ui.add_space(8.0);
                Some(
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        conteudo(ui)
                    })
                    .inner,
                )
            } else {
                None
            };
            ui.add_space(12.0);
            RespostaModelo { resposta, aberto, inner }
        })
        .inner
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn seta_aponta_para_a_direita_fechada_e_para_baixo_aberta() {
        let c = pos2(50.0, 50.0);
        // a ponta do chevron (o ponto do meio) fica à direita fechado e embaixo aberto, dentro da caixa de 10px
        let [a, ponta, b] = pontos_seta(c, false);
        assert!(ponta.x > c.x && (ponta.y - c.y).abs() < 0.01 && a.x < ponta.x && b.x < ponta.x);
        let [a, ponta, b] = pontos_seta(c, true);
        assert!(ponta.y > c.y && (ponta.x - c.x).abs() < 0.01 && a.y < ponta.y && b.y < ponta.y);
        for p in pontos_seta(c, true).iter().chain(pontos_seta(c, false).iter()) {
            assert!((p.x - c.x).abs() <= LADO_SETA / 2.0 && (p.y - c.y).abs() <= LADO_SETA / 2.0);
        }
    }
}
