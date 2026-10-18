//! O popover: uma camada de vidro fosco (`.flut`, [`forma_vidro_fosco`]) presa embaixo de um elemento da tela (a
//! etiqueta de filtro ou o "+ Condição" da faixa dos Dados), com o conteúdo livre de quem chama (campos e botões).
//! Fica por cima do documento (na camada das janelas, abaixo das listas de opções), dentro da janela, e fecha com o
//! Esc ou com um clique fora dele; uma
//! lista aberta por um campo de dentro (a do [`crate::pecas::campos::Select`]) fecha antes, sem fechar o popover. O
//! clique na âncora não conta como fora: quem a desenha decide (abrir outra condição, fechar).
//!
//! ```ignore
//! let r = Popover::new(("condicao", id), rect_da_etiqueta).largura(300.0).mostrar(ctx, |ui| { /* os campos */ });
//! if r.fechar { /* desistiu */ }
//! ```

use egui::{Area, Id, Key, Order, Popup, Rect, Shape, Ui, vec2};

use crate::pecas::campos::forma_vidro_fosco;
use crate::tema::paleta;

/// Raio do vidro do popover (o dos menus, `border-radius:12px`).
pub const RAIO_POPOVER: f32 = 12.0;
/// O `padding:14px` de dentro.
pub const RECUO_POPOVER: f32 = 14.0;
/// A distância entre a âncora e o popover.
pub const DISTANCIA_POPOVER: f32 = 6.0;
/// Largura padrão.
pub const LARGURA_POPOVER: f32 = 300.0;

/// Um popover preso embaixo de `ancora` (à esquerda dela).
#[must_use = "chame .mostrar(ctx, |ui| …)"]
pub struct Popover {
    id: Id,
    ancora: Rect,
    largura: f32,
    rotulo: Option<String>,
}

/// O que o [`Popover::mostrar`] devolve.
pub struct RespostaPopover<R> {
    /// O que o conteúdo devolveu.
    pub inner: R,
    /// O vidro inteiro, na tela.
    pub rect: Rect,
    /// A pessoa desistiu: Esc (sem uma lista de dentro aberta) ou um clique fora do popover e da âncora, a partir do
    /// segundo quadro (o clique que abriu não fecha).
    pub fechar: bool,
}

impl Popover {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, ancora: Rect) -> Self {
        Popover { id: Id::new(("databox::popover", id_salt)), ancora, largura: LARGURA_POPOVER, rotulo: None }
    }

    /// A largura do vidro (com o recuo de dentro).
    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = largura;
        self
    }

    /// O nome para o leitor de tela (o `aria-label` do `role=dialog`: "Condição de filtro").
    pub fn rotulo(mut self, rotulo: impl Into<String>) -> Self {
        self.rotulo = Some(rotulo.into());
        self
    }

    /// O id da camada (o mesmo a cada quadro para o mesmo `id_salt`).
    pub fn id(&self) -> Id {
        self.id
    }

    /// Desenha o vidro e o conteúdo (de cima para baixo, com a largura de dentro) e diz se a pessoa desistiu.
    pub fn mostrar<R>(self, ctx: &egui::Context, conteudo: impl FnOnce(&mut Ui) -> R) -> RespostaPopover<R> {
        let Popover { id, ancora, largura, rotulo } = self;
        // aberto já no quadro anterior: só então um clique fora fecha (o que abriu não conta)
        let ja_estava = ctx.read_response(id).is_some();
        // uma lista aberta por um campo de dentro (o select) fecha primeiro, com o mesmo clique ou o mesmo Esc
        let lista_aberta = Popup::is_any_open(ctx);
        // na camada das janelas (`Order::Middle`): acima do documento e das abas, e abaixo das listas que os campos
        // de dentro abrem (`Order::Foreground`), que assim recebem o clique primeiro
        let resposta = Area::new(id)
            .order(Order::Middle)
            .fixed_pos(ancora.left_bottom() + vec2(0.0, DISTANCIA_POPOVER))
            .constrain(true)
            .show(ctx, |ui| {
                let fundo = ui.painter().add(Shape::Noop);
                let quadro = egui::Frame::NONE.inner_margin(RECUO_POPOVER).show(ui, |ui| {
                    ui.set_width((largura - 2.0 * RECUO_POPOVER).max(0.0));
                    conteudo(ui)
                });
                let rect = quadro.response.rect;
                ui.painter().set(fundo, forma_vidro_fosco(ui.painter(), paleta(ui), rect, RAIO_POPOVER));
                (quadro.inner, rect)
            });
        if let Some(rotulo) = rotulo {
            ctx.accesskit_node_builder(resposta.response.id, |n| {
                n.set_role(egui::accesskit::Role::Dialog);
                n.set_label(rotulo);
            });
        }
        let (inner, rect) = resposta.inner;
        let esc = !lista_aberta && ctx.input(|i| i.key_pressed(Key::Escape));
        let clique_fora = ja_estava
            && !lista_aberta
            && ctx.input(|i| {
                i.pointer.any_click()
                    && i.pointer.interact_pos().is_some_and(|pos| !rect.contains(pos) && !ancora.contains(pos))
            });
        RespostaPopover { inner, rect, fechar: esc || clique_fora }
    }
}
