//! A barra de ferramentas universal (o `header.vidro` com o `role=toolbar` de `Main.dc.html`): o painel fixo de 46px
//! ([`ALTURA_BARRA`]: a linha de 44px e a borda de 1px) com `padding:0 8px` e 2px entre os botões, os separadores `.sep` ([`separador`]) e a busca de
//! 250px ([`busca_desligada`], a [`Entrada::busca`] com a lupa e o `.kbd` Ctrl+F). Os botões são os
//! [`crate::pecas::botao::Botao::icone`] de 30px; cada aba diz o que cada verbo faz e desliga o que não se aplica.
//!
//! ```ignore
//! barra_ferramentas(ui, |ui| {
//!     ui.add(Botao::icone(Icone::Voltar).dica("Voltar (Alt+←)"));
//!     separador(ui);
//!     busca_desligada(ui);
//! }, |ui| {
//!     ui.add(Botao::icone(Icone::Painel).selecionado(true));
//! });
//! ```

use egui::{Id, Rect, Response, Sense, Stroke, Ui, vec2};

use crate::pecas::arranjo::linha_com_direita;
use crate::pecas::campos::Entrada;
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::painel::pintar_painel_fixo;
use crate::tema::paleta;

/// A linha dos botões da barra de ferramentas (`role=toolbar` com `height:44px`).
pub const ALTURA_LINHA_BARRA: f32 = 44.0;
/// Altura da barra de ferramentas: o `header.vidro` com a linha de 44px e a borda de 1px em cima e embaixo (46px
/// nas pranchetas, medido no Chrome; o app tinha 44, revisão de 17/10/2026).
pub const ALTURA_BARRA: f32 = ALTURA_LINHA_BARRA + 2.0;
/// Largura da busca (`.busca{width:250px}`).
pub const LARGURA_BUSCA: f32 = 250.0;
/// `padding:0 8px`.
const RECUO: f32 = 8.0;
/// `gap:2px` entre os botões.
const GAP: f32 = 2.0;

/// A barra inteira na largura disponível: o painel fixo de [`ALTURA_BARRA`], `esquerda` a partir da esquerda e
/// `direita` encostado à direita (o Painel de propriedades depois do `flex:1`), com 2px entre os itens. Devolve o
/// retângulo da barra.
pub fn barra_ferramentas(ui: &mut Ui, esquerda: impl FnOnce(&mut Ui), direita: impl FnOnce(&mut Ui)) -> Rect {
    let p = paleta(ui);
    let topo = ui.cursor().min;
    let rect = Rect::from_min_size(topo, vec2(ui.available_width(), ALTURA_BARRA));
    pintar_painel_fixo(ui.painter(), p, rect, RAIO_CARD);
    // a linha fica dentro da borda de 1px
    let dentro = rect.shrink2(vec2(RECUO + 1.0, 1.0));
    let mut filho = ui.new_child(egui::UiBuilder::new().id_salt("databox::barra_ferramentas").max_rect(dentro));
    linha_com_direita(
        &mut filho,
        Id::new("databox::barra_ferramentas::linha"),
        ALTURA_LINHA_BARRA,
        GAP,
        |ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            esquerda(ui);
        },
        |ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            direita(ui);
        },
    );
    ui.allocate_rect(rect, Sense::hover());
    rect
}

/// O separador `.sep`: o fio de 1×22px em `--vidrob`, com `margin:0 6px`.
pub fn separador(ui: &mut Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(1.0 + 2.0 * 6.0, 22.0), Sense::hover());
    let e = &paleta(ui).estudio;
    ui.painter().vline(rect.center().x.floor() + 0.5, rect.y_range(), Stroke::new(1.0, e.vidrob));
    resp
}

/// A busca da barra desligada (a aba sem busca: o Início, os Índices e gatilhos, a Definição SQL): a
/// [`Entrada::busca`] de 250×30 com a lupa, o texto "Buscar" e o `.kbd` Ctrl+F. O leitor de tela ouve "Buscar". A
/// ligada é a `estado::busca_da_barra`, sobre o texto do documento.
pub fn busca_desligada(ui: &mut Ui) -> Response {
    let mut vazio = String::new();
    let entrada = Entrada::new(&mut vazio)
        .busca(true)
        .atalho("Ctrl+F")
        .dica("Buscar")
        .rotulo("Buscar")
        .largura(LARGURA_BUSCA)
        .id_salt("databox::busca_da_barra")
        .habilitado(false);
    ui.add(entrada)
}
