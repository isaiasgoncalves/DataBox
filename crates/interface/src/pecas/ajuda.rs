//! O "i" com balão de ajuda: a explicação da tela, do card ou do campo mora aqui, não sob o título. O ícone é o
//! sólido de informação das pranchetas (o PNG `estados/informacao`), num quadrado de 16px ao lado do título ou de
//! 14px ([`ajuda_mini`], ao lado de rótulos, como o "Opções" da janela Exportar). O balão é o vidro das camadas
//! flutuantes (`.flut`), abre no hover ou no foco, fica aberto enquanto o mouse está nele (fecha 140ms depois de
//! sair), mede até 440px (320px no mini) e vira para cima quando não cabe embaixo. Papel `tooltip`.
//!
//! [`pintar_icone_estado`] pinta os sólidos de estado ([`IconeEstado`], o [`crate::icones::EstadoIcone`]: atenção,
//! erro, concluído, informação, ajuda, bloqueado) para outras peças (os avisos e o toast).

use egui::{
    Area, Color32, CursorIcon, Id, Order, Painter, Pos2, Rect, Response, Sense, Ui, Vec2, WidgetInfo, WidgetType, pos2,
    vec2,
};

use crate::icones::{self, Icone};
use crate::pecas::botao::pintor_de_fora;
use crate::pecas::campos::forma_vidro_flutuante;
use crate::pecas::texto::{Trecho, paragrafo_rico};
use crate::pintura;
use crate::tema::{duracao_transicao, paleta};

// ------------------------------------------------------------------ ícones de estado

/// Os sólidos de estado das pranchetas (`Icones.dc.html`, grupo "Estados e avisos"): o mesmo enum dos ícones.
pub use crate::icones::EstadoIcone as IconeEstado;

/// Pinta um sólido de estado no quadrado `rect` (16px ao lado do texto, 14px ao lado dos rótulos, 18px no aviso),
/// pelo PNG do tamanho certo para a escala da tela ([`icones::pintar`] com o [`Icone::Estado`]).
pub fn pintar_icone_estado(painter: &Painter, rect: Rect, icone: IconeEstado) {
    icones::pintar(painter, rect, Icone::Estado(icone), Color32::WHITE);
}

// ------------------------------------------------------------------ o "i" e o balão

#[derive(Clone, Copy, Default)]
struct EstadoBalao {
    aberto: bool,
    saiu_em: Option<f64>,
    rect: Option<Rect>,
}

/// O "i" de 16px ao lado do título, com o balão.
pub fn ajuda(ui: &mut Ui, texto: &str) -> Response {
    Ajuda::new(&[Trecho::Normal(texto)]).mostrar(ui)
}

/// O "i" de 14px ao lado de um rótulo de campo ou título de seção.
pub fn ajuda_mini(ui: &mut Ui, texto: &str) -> Response {
    Ajuda::new(&[Trecho::Normal(texto)]).mini(true).mostrar(ui)
}

/// Balão com trechos (negrito, código).
#[must_use = "chame .mostrar(ui)"]
pub struct Ajuda<'a> {
    trechos: &'a [Trecho<'a>],
    mini: bool,
    rotulo: &'a str,
}

/// Raio do balão (o do vidro dos menus).
const RAIO_BALAO: f32 = 12.0;

impl<'a> Ajuda<'a> {
    pub fn new(trechos: &'a [Trecho<'a>]) -> Self {
        Ajuda { trechos, mini: false, rotulo: "Sobre esta tela" }
    }

    /// Variante mini (14px).
    pub fn mini(mut self, sim: bool) -> Self {
        self.mini = sim;
        if sim {
            self.rotulo = "Ajuda";
        }
        self
    }

    /// Nome do ícone para o leitor de tela (o `aria-label`).
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo = rotulo;
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> Response {
        let p = paleta(ui);
        let e = &p.estudio;
        let lado = if self.mini { 14.0 } else { 16.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(lado, lado), Sense::click());
        let id_estado = resp.id.with("balao");
        let mut estado: EstadoBalao = ui.ctx().data(|d| d.get_temp(id_estado)).unwrap_or_default();

        let agora = ui.input(|i| i.time);
        let ponteiro = ui.ctx().pointer_hover_pos();
        let sobre_balao = match (estado.rect, ponteiro) {
            // a "ponte" de 10px entre o ícone e o balão
            (Some(r), Some(pt)) if estado.aberto => r.expand2(vec2(0.0, 10.0)).contains(pt),
            _ => false,
        };
        let ativo = resp.hovered() || resp.has_focus() || sobre_balao;
        if ativo {
            estado.aberto = true;
            estado.saiu_em = None;
        } else if estado.aberto {
            let saiu = *estado.saiu_em.get_or_insert(agora);
            if agora - saiu >= 0.14 {
                estado.aberto = false;
                estado.saiu_em = None;
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(0.14 - (agora - saiu) + 0.005));
            }
        }

        // o ícone, com um halo fraco em `--brilho` no hover e no foco do teclado
        let foco = resp.has_focus() && pintura::foco_visivel(ui.ctx());
        let realce =
            ui.ctx().animate_bool_with_time(resp.id.with("realce"), resp.hovered() || foco, duracao_transicao(ui));
        if ui.is_rect_visible(rect.expand(12.0)) {
            let fora = pintor_de_fora(ui, rect);
            if realce > 0.0 {
                // o círculo do ícone (18 da grade de 48)
                let circulo = Rect::from_center_size(rect.center(), Vec2::splat(lado * 36.0 / 48.0));
                pintura::halo(&fora, circulo, f32::INFINITY, 10.0, e.brilho.gamma_multiply(realce));
            }
            pintar_icone_estado(ui.painter(), rect, IconeEstado::Informacao);
            if foco {
                pintura::anel_foco(&fora, p, rect, f32::INFINITY);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Help);
        }

        if estado.aberto {
            let tela = ui.ctx().content_rect();
            let largura_max = if self.mini { 320.0_f32 } else { 440.0_f32 }.min(tela.width() * 0.8);
            // `padding:9px 12px` dentro da borda de 1px; o balão fica da largura do texto, até o máximo
            let (pad_x, pad_y) = (12.0, 9.0);
            let g = paragrafo_rico(ui, self.trechos, 12.0, e.t, e.t, largura_max - 2.0 * pad_x - 2.0, 1.5);
            let tamanho =
                vec2((g.tamanho().x + 2.0 * pad_x + 2.0).ceil().min(largura_max), g.tamanho().y + 2.0 * pad_y + 2.0);
            let mut pos =
                Pos2::new(rect.left().max(tela.left() + 8.0).min(tela.right() - tamanho.x - 8.0), rect.bottom() + 8.0);
            if pos.y + tamanho.y > tela.bottom() - 8.0 && rect.top() - 8.0 - tamanho.y > tela.top() + 8.0 {
                pos.y = rect.top() - 8.0 - tamanho.y;
            }
            let area = Area::new(id_estado.with("area")).order(Order::Tooltip).fixed_pos(pos).interactable(true).show(
                ui.ctx(),
                |ui| {
                    let (r, _) = ui.allocate_exact_size(tamanho, Sense::hover());
                    let painter = ui.painter();
                    // o balão é uma dica: no vidro liso (96%), sem a foto de tela cheia do fosco
                    painter.add(forma_vidro_flutuante(p, r, RAIO_BALAO));
                    g.pintar(painter, pos2(r.left() + pad_x + 1.0, r.top() + pad_y + 1.0), e.t);
                },
            );
            estado.rect = Some(area.response.rect);
            ui.ctx().accesskit_node_builder(area.response.id, |n| n.set_role(egui::accesskit::Role::Tooltip));
        } else {
            estado.rect = None;
        }
        ui.ctx().data_mut(|d| d.insert_temp(id_estado, estado));
        let rotulo = self.rotulo;
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, rotulo));
        resp
    }
}

/// Id do estado do balão de um ícone (para testes).
pub fn id_balao(resposta_icone: &Response) -> Id {
    resposta_icone.id.with("balao")
}
