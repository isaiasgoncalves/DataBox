//! O menu de contexto (o `.menu-ctx` da prancheta 15, `Estados.dc.html`): o painel no vidro fosco das camadas
//! flutuantes (`.flut`, 250px, raio 12, `padding:5px`) com os itens `.mi` de 28px, a silhueta de 14px em `--t2`
//! antes do texto (o item sem silhueta guarda o espaço dela), o atalho em `.at`, os separadores `.msep` e o item de
//! perigo (`.mi.perigo`). É o mesmo [`PainelMenu`] dos menus da barra: [`PainelMenu::item`],
//! [`PainelMenu::item_perigo`], [`PainelMenu::item_desligado`], [`PainelMenu::separador`] e [`PainelMenu::silhueta`].
//!
//! Abre no clique direito do `dono` (o menu nasce com o canto de cima à esquerda no ponteiro) e com o Shift+F10
//! quando o `dono` tem o foco (embaixo dele, à esquerda; o foco vai ao primeiro item). Pelo teclado, ↑ e ↓ andam pelos
//! itens ligados, com volta (sem item com o foco, ↓ vai ao primeiro e ↑ ao último); Enter ativa; o Esc fecha e devolve
//! o foco ao `dono`, e ativar um item pelo teclado também. Perto da borda da janela o egui vira o menu para o outro
//! lado.
//!
//! ```ignore
//! MenuContexto::new(("no", chave)).rotulo(nome).mostrar(&resposta_do_no, |m| {
//!     m.item("Abrir", None, false);
//!     if m.silhueta(Icone::CopiarCaminho).item("Copiar caminho", Some("Ctrl+Shift+C"), false).clicked() { … }
//! });
//! ```

use egui::accesskit::Role;
use egui::{
    Context, Event, FocusDirection, Frame, Id, Key, Modifiers, Popup, PopupCloseBehavior, Pos2, RectAlign, Response,
    SetOpenCommand, vec2,
};

use crate::pecas::casca::{PainelMenu, painel_no_vidro};

/// A largura do menu (`.menu-ctx{width:250px}`), com a borda e o `padding:5px`.
pub const LARGURA_MENU_CONTEXTO: f32 = 250.0;
/// Pelo teclado, o menu abre embaixo do `dono`, este tanto à direita da borda esquerda dele.
const RECUO_TECLADO: f32 = 8.0;

/// O que o menu guarda entre os quadros.
#[derive(Clone, Copy, Default)]
struct Estado {
    /// Onde o canto de cima à esquerda do menu fica.
    pos: Pos2,
    /// Aberto pelo teclado: o foco vai ao primeiro item quando o painel já está à vista.
    pedir_foco: bool,
    /// Um item tinha o foco no quadro anterior.
    foco_em_item: bool,
}

/// O menu de contexto de um `dono`. Chame [`MenuContexto::mostrar`] todo quadro, logo depois de desenhar o dono.
#[must_use = "chame .mostrar(&dono, |m| …)"]
pub struct MenuContexto<'a> {
    id: Id,
    rotulo: &'a str,
}

/// Consome o Shift+F10 só com os modificadores exatos: o `consume_key` aceitaria também o Alt+Shift+F10 e o
/// Ctrl+Shift+F10.
fn consumir_shift_f10(ctx: &Context) -> bool {
    ctx.input_mut(|i| {
        let achado = i.events.iter().position(|e| {
            matches!(e, Event::Key { key: Key::F10, pressed: true, modifiers, .. }
                if modifiers.matches_exact(Modifiers::SHIFT))
        });
        achado.map(|p| i.events.remove(p)).is_some()
    })
}

impl<'a> MenuContexto<'a> {
    /// `id_salt` separa o menu de um dono do de outro (o caminho do nó, no Explorador).
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        MenuContexto { id: Id::new(("databox::menu_contexto", id_salt)), rotulo: "" }
    }

    /// O nome do menu para o leitor de tela (o `aria-label` da prancheta: o nome do arquivo).
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo = rotulo;
        self
    }

    fn id_painel(&self) -> Id {
        self.id.with("painel")
    }

    /// O menu deste `id_salt` está aberto?
    pub fn aberto(ctx: &Context, id_salt: impl std::hash::Hash + std::fmt::Debug) -> bool {
        Popup::is_id_open(ctx, MenuContexto::new(id_salt).id_painel())
    }

    /// Fecha o menu deste `id_salt` (clicar num item já fecha).
    pub fn fechar(ctx: &Context, id_salt: impl std::hash::Hash + std::fmt::Debug) {
        Popup::close_id(ctx, MenuContexto::new(id_salt).id_painel());
    }

    /// Desenha o menu aberto, se estiver, e devolve o que o `conteudo` devolveu.
    pub fn mostrar<R>(self, dono: &Response, conteudo: impl FnOnce(&mut PainelMenu<'_>) -> R) -> Option<R> {
        let ctx = dono.ctx.clone();
        let id_painel = self.id_painel();
        let id_estado = self.id.with("estado");
        let anterior: Estado = ctx.data(|d| d.get_temp(id_estado)).unwrap_or_default();
        let aberto_antes = Popup::is_id_open(&ctx, id_painel);

        let mut estado = anterior;
        let mut abrir = false;
        if dono.secondary_clicked() {
            estado.pos = ctx
                .input(|i| i.pointer.interact_pos())
                .or_else(|| dono.interact_pointer_pos())
                .unwrap_or_else(|| dono.rect.left_bottom());
            estado.pedir_foco = false;
            abrir = true;
        } else if dono.has_focus() && consumir_shift_f10(&ctx) {
            estado.pos = dono.rect.left_bottom() + vec2(RECUO_TECLADO, 0.0);
            estado.pedir_foco = true;
            abrir = true;
        }

        let mut resultado = None;
        let mut itens = Vec::new();
        let saida = Popup::new(id_painel, ctx.clone(), estado.pos, dono.layer_id)
            .open_memory(abrir.then_some(SetOpenCommand::Bool(true)))
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(RectAlign::BOTTOM_START)
            .gap(0.0)
            .width(LARGURA_MENU_CONTEXTO)
            .frame(Frame::NONE)
            .show(|ui| {
                let (r, fechar, ligados) = painel_no_vidro(ui, LARGURA_MENU_CONTEXTO, |m| {
                    m.com_silhuetas();
                    conteudo(m)
                });
                resultado = Some(r);
                itens = ligados;
                if fechar {
                    Popup::close_id(ui.ctx(), id_painel);
                }
            });
        if let Some(saida) = &saida {
            ctx.accesskit_node_builder(saida.response.id, |no| {
                no.set_role(Role::Menu);
                if !self.rotulo.is_empty() {
                    no.set_label(self.rotulo);
                }
            });
        }

        let aberto_agora = Popup::is_id_open(&ctx, id_painel);
        if aberto_agora && !itens.is_empty() {
            let focado = ctx.memory(|m| m.focused());
            let em_item = focado.is_some_and(|f| itens.contains(&f));
            // o painel só recebe o foco depois do quadro de medida (o primeiro com ele aberto)
            let destino = if aberto_antes && estado.pedir_foco {
                estado.pedir_foco = false;
                itens.first()
            } else if aberto_antes && !em_item {
                let (cima, baixo) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
                if baixo {
                    itens.first()
                } else if cima {
                    itens.last()
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(&id) = destino {
                ctx.memory_mut(|m| {
                    m.request_focus(id);
                    m.move_focus(FocusDirection::None);
                });
            }
            estado.foco_em_item = em_item;
        }
        if aberto_antes && !aberto_agora {
            // o Esc e o Enter num item devolvem o foco ao dono; o clique em outro lugar deixa o foco onde ele foi
            let esc = ctx.input(|i| i.key_pressed(Key::Escape));
            if esc || (anterior.foco_em_item && !ctx.input(|i| i.pointer.any_click())) {
                dono.request_focus();
            }
            estado.foco_em_item = false;
            estado.pedir_foco = false;
        }
        if abrir || aberto_agora || aberto_antes {
            ctx.data_mut(|d| d.insert_temp(id_estado, estado));
        }
        resultado
    }
}
