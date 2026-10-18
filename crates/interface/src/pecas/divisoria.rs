//! A divisória de um painel lateral (o Explorador e as Propriedades da casca): a alça que muda a largura do painel no
//! vão de 8px entre ele e o documento. Em repouso é invisível (nenhuma barra nova na tela); com o ponteiro em cima,
//! durante o arrasto e com o foco do teclado ela mostra um fio de 2px na cor de destaque (`--acc`) no meio do vão,
//! que aparece e some com a transição de [`duracao_transicao`] (nenhum repaint contínuo: parada, a peça não pede
//! quadro).
//!
//! - **Mouse:** o cursor vira [`CursorIcon::ResizeHorizontal`]; arrastar muda a largura ao vivo (em pontos inteiros,
//!   dentro de `minima` e `maxima`); o duplo clique volta à largura `padrao`. [`SaidaDivisoria::concluiu`] diz que o
//!   gesto terminou (soltou o botão, o duplo clique, um passo do teclado): é a hora de guardar a largura, não a cada
//!   quadro do arrasto.
//! - **Teclado:** a divisória recebe o foco (Tab) e as setas movem a barra [`PASSO_TECLADO`] pontos por vez, para
//!   onde a seta aponta (→ alarga o painel da esquerda; ← alarga o da direita); Home e End vão ao mínimo e ao
//!   máximo; Enter volta à largura padrão.
//! - **Acessibilidade:** o papel `Splitter` do AccessKit, com o rótulo ("Largura do Explorador"), o valor em pontos,
//!   o mínimo e o máximo, e as ações Aumentar e Diminuir.
//!
//! Quem desenha os painéis decide as larguras (e o que fazer com a janela estreita): a peça só devolve a largura
//! pedida, já dentro dos limites que recebeu.
//!
//! ```ignore
//! let saida = Divisoria::new("explorador", "Largura do Explorador", vao, LadoDoPainel::Esquerda,
//!     LimitesLargura { atual: 258.0, minima: 200.0, maxima: 600.0, padrao: 258.0 }).mostrar(ui);
//! if saida.mudou { larguras.explorador = saida.largura; }
//! if saida.concluiu { guardar(larguras); }
//! ```

use egui::accesskit::{Action, Orientation, Role};
use egui::{CursorIcon, EventFilter, Id, Key, Modifiers, Pos2, Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::pintura;
use crate::tema::{duracao_transicao, paleta};

/// Quanto cada toque nas setas muda a largura, em pontos.
pub const PASSO_TECLADO: f32 = 16.0;
/// A espessura do fio que aparece no vão.
pub const ESPESSURA_FIO: f32 = 2.0;
/// Quanto o fio fica mais curto que o vão, em cima e embaixo (o fio não encosta nas quinas dos painéis).
const RECUO_FIO: f32 = 6.0;
/// A opacidade do fio com o ponteiro em cima; no arrasto e no foco do teclado ele fica mais firme.
const OPACIDADE_HOVER: f32 = 0.38;
const OPACIDADE_FIRME: f32 = 0.9;

/// De que lado do vão fica o painel que a divisória redimensiona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadoDoPainel {
    /// O painel está à esquerda do vão (o Explorador): arrastar para a direita o alarga.
    Esquerda,
    /// O painel está à direita do vão (as Propriedades): arrastar para a esquerda o alarga.
    Direita,
}

impl LadoDoPainel {
    /// +1 se mover a barra para a direita alarga o painel, −1 se estreita.
    fn sinal(self) -> f32 {
        match self {
            LadoDoPainel::Esquerda => 1.0,
            LadoDoPainel::Direita => -1.0,
        }
    }
}

/// A largura de agora do painel e os limites dela, em pontos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitesLargura {
    pub atual: f32,
    pub minima: f32,
    /// Se vier menor que a mínima, vale a mínima.
    pub maxima: f32,
    /// A largura a que o duplo clique (e o Enter) volta.
    pub padrao: f32,
}

impl LimitesLargura {
    /// O mínimo e o máximo em pontos inteiros (o máximo nunca fica abaixo do mínimo).
    pub fn inteiros(&self) -> (f32, f32) {
        let minima = self.minima.ceil();
        (minima, self.maxima.floor().max(minima))
    }

    /// A largura `valor` dentro dos limites, em pontos inteiros.
    pub fn limitar(&self, valor: f32) -> f32 {
        let (minima, maxima) = self.inteiros();
        valor.round().clamp(minima, maxima)
    }
}

/// O que a divisória fez neste quadro.
#[derive(Debug, Clone)]
pub struct SaidaDivisoria {
    pub resposta: Response,
    /// A largura que a pessoa pede agora (a `atual` quando nada mudou), dentro dos limites e em pontos inteiros.
    pub largura: f32,
    /// A largura mudou neste quadro (arrasto, tecla, duplo clique).
    pub mudou: bool,
    /// O gesto terminou neste quadro (soltou o botão, o duplo clique, um passo ou um Home/End/Enter): guarde a
    /// largura agora.
    pub concluiu: bool,
}

/// A divisória de um painel lateral (ver o módulo).
#[must_use = "chame .mostrar(ui)"]
pub struct Divisoria<'a> {
    id: Id,
    rotulo: &'a str,
    vao: Rect,
    lado: LadoDoPainel,
    limites: LimitesLargura,
}

impl<'a> Divisoria<'a> {
    /// `vao` é o retângulo entre o painel e o documento (a alça ocupa ele inteiro); `rotulo`, o nome para o leitor
    /// de tela ("Largura do Explorador").
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        rotulo: &'a str,
        vao: Rect,
        lado: LadoDoPainel,
        limites: LimitesLargura,
    ) -> Self {
        Divisoria { id: Id::new(("databox::divisoria", id_salt)), rotulo, vao, lado, limites }
    }

    pub fn mostrar(self, ui: &mut Ui) -> SaidaDivisoria {
        let l = self.limites;
        let sinal = self.lado.sinal();
        let ctx = ui.ctx().clone();
        let resp = ui.interact(self.vao, self.id, Sense::click_and_drag());
        let id_inicio = self.id.with("inicio");
        let mut nova = l.limitar(l.atual);
        let mut concluiu = false;

        // o mouse: arrasta pela distância desde o clique (a largura de quando começou, mais o quanto andou) e volta
        // ao padrão no duplo clique
        if resp.drag_started() {
            ctx.data_mut(|d| d.insert_temp(id_inicio, nova));
        }
        if resp.dragged() {
            let inicio = ctx.data(|d| d.get_temp::<f32>(id_inicio)).unwrap_or(nova);
            if let Some(delta) = resp.total_drag_delta() {
                nova = l.limitar(inicio + sinal * delta.x);
            }
        }
        if resp.drag_stopped() {
            ctx.data_mut(|d| d.remove_temp::<f32>(id_inicio));
            concluiu = true;
        }
        if resp.double_clicked() {
            nova = l.limitar(l.padrao);
            concluiu = true;
        }

        // o leitor de tela: Aumentar e Diminuir são um passo, com ou sem o foco
        let (mais, menos) = ui.input(|i| {
            (
                i.num_accesskit_action_requests(resp.id, Action::Increment),
                i.num_accesskit_action_requests(resp.id, Action::Decrement),
            )
        });
        let mut passos = mais as f32 - menos as f32;
        // o teclado: as setas movem a barra; o foco segura as setas horizontais para o egui não levar o foco ao
        // vizinho
        if resp.has_focus() {
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(self.id, EventFilter { horizontal_arrows: true, ..Default::default() });
            });
            let (direita, esquerda, inicio, fim) = ui.input(|i| {
                (
                    i.num_presses(Key::ArrowRight),
                    i.num_presses(Key::ArrowLeft),
                    i.key_pressed(Key::Home),
                    i.key_pressed(Key::End),
                )
            });
            // a barra anda para onde a seta aponta; o painel alarga quando a barra anda para longe dele
            passos += sinal * (direita as f32 - esquerda as f32);
            if inicio || fim {
                nova = if inicio { l.inteiros().0 } else { l.inteiros().1 };
                concluiu = true;
            }
            // o Enter (o clique do teclado) volta ao padrão, como o duplo clique
            if resp.clicked() && !resp.clicked_by(egui::PointerButton::Primary) {
                nova = l.limitar(l.padrao);
                concluiu = true;
            }
            // o Home e o End são desta peça enquanto ela tem o foco (a rolagem da página não os leva)
            ui.input_mut(|i| {
                for tecla in [Key::Home, Key::End] {
                    i.consume_key(Modifiers::NONE, tecla);
                }
            });
        }
        if passos != 0.0 {
            nova = l.limitar(nova + passos * PASSO_TECLADO);
            concluiu = true;
        }
        // depois do mouse a divisória larga o foco: com ele, a rolagem da página pelo teclado ficaria parada e as
        // setas continuariam sendo dela
        if resp.clicked_by(egui::PointerButton::Primary) || resp.drag_stopped() {
            resp.surrender_focus();
        }

        // a leitura para o AccessKit: o valor é a largura de agora (a de antes deste quadro, que é a que está na tela)
        resp.widget_info(|| WidgetInfo::slider(true, f64::from(l.atual), self.rotulo));
        let (minima, maxima) = l.inteiros();
        ctx.accesskit_node_builder(resp.id, |no| {
            no.set_role(Role::Splitter);
            no.set_orientation(Orientation::Vertical);
            no.set_min_numeric_value(f64::from(minima));
            no.set_max_numeric_value(f64::from(maxima));
            no.set_numeric_value_step(f64::from(PASSO_TECLADO));
            no.set_value(format!("{} pontos", l.atual.round()));
            if l.atual < maxima {
                no.add_action(Action::Increment);
            }
            if l.atual > minima {
                no.add_action(Action::Decrement);
            }
        });

        // o cursor e o fio no meio do vão
        let arrastando = resp.dragged();
        if resp.hovered() || arrastando {
            ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        let foco = resp.has_focus() && pintura::foco_visivel(&ctx);
        let tempo = duracao_transicao(ui);
        let presenca =
            ctx.animate_bool_with_time(self.id.with("presenca"), resp.hovered() || arrastando || foco, tempo);
        let firmeza = ctx.animate_bool_with_time(self.id.with("firmeza"), arrastando || foco, tempo);
        if presenca > 0.0 && ui.is_rect_visible(self.vao) {
            let cor = paleta(ui).estudio.acc;
            let opacidade = presenca * (OPACIDADE_HOVER + (OPACIDADE_FIRME - OPACIDADE_HOVER) * firmeza);
            ui.painter().rect_filled(fio(self.vao), ESPESSURA_FIO / 2.0, cor.gamma_multiply(opacidade));
        }

        SaidaDivisoria { resposta: resp, largura: nova, mudou: nova != l.limitar(l.atual), concluiu }
    }
}

/// O fio no meio do vão, mais curto que ele em cima e embaixo, alinhado ao pixel.
fn fio(vao: Rect) -> Rect {
    let centro: Pos2 = pos2(vao.center().x.round(), vao.center().y);
    Rect::from_center_size(centro, vec2(ESPESSURA_FIO, (vao.height() - 2.0 * RECUO_FIO).max(0.0)))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limites_em_pontos_inteiros_e_com_o_maximo_acima_do_minimo() {
        let l = LimitesLargura { atual: 258.0, minima: 200.4, maxima: 500.9, padrao: 258.0 };
        assert_eq!(l.inteiros(), (201.0, 500.0));
        assert_eq!(l.limitar(10.0), 201.0);
        assert_eq!(l.limitar(999.0), 500.0);
        assert_eq!(l.limitar(300.4), 300.0);
        // um máximo abaixo do mínimo (janela minúscula) fica no mínimo
        let apertado = LimitesLargura { atual: 258.0, minima: 200.0, maxima: 150.0, padrao: 258.0 };
        assert_eq!(apertado.inteiros(), (200.0, 200.0));
        assert_eq!(apertado.limitar(400.0), 200.0);
    }

    #[test]
    fn o_fio_fica_no_meio_do_vao_e_alinhado() {
        let vao = Rect::from_min_max(pos2(318.0, 100.0), pos2(326.0, 700.0));
        let f = fio(vao);
        assert_eq!((f.left(), f.right()), (321.0, 323.0));
        assert_eq!((f.top(), f.bottom()), (106.0, 694.0));
    }

    #[test]
    fn o_sinal_do_lado() {
        assert_eq!(LadoDoPainel::Esquerda.sinal(), 1.0);
        assert_eq!(LadoDoPainel::Direita.sinal(), -1.0);
    }
}
