//! Zona de risco (excluir a tabela e o que ela contém), **fechada por padrão**, no vermelho `--perigo` das
//! pranchetas. O resumo é uma linha com a seta das sanfonas (o chevron de 10px de [`crate::pecas::modelo`]) e o
//! título em `--perigo` 12px / 600, com o `.hint` em `--t3`; aberta, a seta aponta para baixo e o corpo ganha o
//! fundo de perigo (`--perigo` a 10%, o do `.erro`).

use egui::{
    Align, CornerRadius, CursorIcon, Id, Layout, Rect, Response, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2,
    vec2,
};

use crate::fontes::{self, Peso};
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::modelo::{LADO_SETA, pintar_seta_sanfona};
use crate::pecas::texto::galley;
use crate::pintura;
use crate::tema::paleta;

#[must_use = "chame .mostrar(ui, |ui| …)"]
pub struct ZonaRisco<'a> {
    id: Id,
    titulo: &'a str,
    hint: Option<&'a str>,
    padding_resumo: f32,
    fio_acima: bool,
}

impl<'a> ZonaRisco<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, titulo: &'a str) -> Self {
        ZonaRisco {
            id: Id::new(("databox::zona_risco", id_salt)),
            titulo,
            hint: None,
            padding_resumo: 18.0,
            fio_acima: false,
        }
    }

    /// O `.hint` do resumo ("excluir este item e tudo o que ele contém").
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    /// No fim do cartão da tabela: fio `--vidrob` em cima, resumo com 13px de padding vertical e, aberta, o fundo
    /// com os cantos de baixo do cartão (o egui só recorta por retângulo).
    pub fn no_card_tabela(mut self) -> Self {
        self.fio_acima = true;
        self.padding_resumo = 13.0;
        self
    }

    /// Está aberta? (o estado fica guardado no contexto)
    pub fn aberta(&self, ui: &Ui) -> bool {
        ui.ctx().data(|d| d.get_temp::<bool>(self.id)).unwrap_or(false)
    }

    /// Desenha o resumo e, aberta, o corpo. Devolve a resposta do resumo (clique abre/fecha).
    pub fn mostrar(self, ui: &mut Ui, corpo: impl FnOnce(&mut Ui)) -> Response {
        let p = paleta(ui);
        let e = &p.estudio;
        let mut aberta = self.aberta(ui);
        let largura = ui.available_width();
        let topo = ui.cursor().min;
        if self.fio_acima {
            ui.painter().hline(topo.x..=topo.x + largura, topo.y + 0.5, egui::Stroke::new(1.0, e.vidrob));
        }
        // resumo: seta, título e hint numa linha de 17px (12px × 1,4)
        let gt = galley(ui, self.titulo, fontes::fonte(12.0, Peso::Medio), e.perigo);
        let gh = self.hint.map(|h| galley(ui, h, fontes::fonte(11.5, Peso::Normal), e.t3));
        let altura_linha = 17.0_f32;
        let altura = altura_linha + 2.0 * self.padding_resumo;
        let rect = Rect::from_min_size(topo, vec2(largura, altura));
        let resp = ui.allocate_rect(rect, Sense::click());
        if resp.clicked() {
            aberta = !aberta;
            ui.ctx().data_mut(|d| d.insert_temp(self.id, aberta));
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let meio = rect.top() + self.padding_resumo + altura_linha * 0.5;
            let mut x = rect.left() + 20.0;
            pintar_seta_sanfona(painter, pos2(x + LADO_SETA / 2.0, meio), aberta, e.perigo);
            x += LADO_SETA + 8.0;
            let topo_titulo = meio - gt.size().y / 2.0;
            let base = topo_titulo + crate::pecas::texto::linha_de_base(&gt);
            painter.galley(pos2(x, topo_titulo), gt.clone(), e.perigo);
            x += gt.size().x + 10.0;
            if let Some(gh) = gh {
                // na linha de base do título
                let y = base - crate::pecas::texto::linha_de_base(&gh);
                painter.galley(pos2(x, y), gh, e.t3);
            }
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(painter, p, rect.shrink(4.0), 6.0);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, aberta, self.titulo));
        if aberta {
            let inicio = ui.cursor().min;
            let lugar = ui.painter().add(egui::Shape::Noop);
            let interno =
                Rect::from_min_size(inicio + vec2(20.0, 18.0), vec2((largura - 40.0).max(0.0), f32::INFINITY));
            let mut filho = ui.new_child(
                UiBuilder::new().id_salt(self.id.with("corpo")).max_rect(interno).layout(Layout::top_down(Align::Min)),
            );
            filho.spacing_mut().item_spacing = vec2(8.0, 8.0);
            corpo(&mut filho);
            let fundo = Rect::from_min_size(inicio, vec2(largura, filho.min_rect().height() + 36.0));
            // no fim do cartão, os cantos de baixo acompanham os dele (por dentro da borda de 1px)
            let raio = if self.fio_acima { (RAIO_CARD - 1.0) as u8 } else { 0 };
            let cantos = CornerRadius { nw: 0, ne: 0, sw: raio, se: raio };
            ui.painter().set(lugar, egui::Shape::rect_filled(fundo, cantos, p.aqua_danger_bg));
            ui.allocate_rect(fundo, Sense::hover());
        }
        resp
    }
}
