//! A barra de ícones (o `nav.vidro` "Áreas" das pranchetas, o trilho B): o painel fixo de 52px com os botões
//! `.rail` de 36px e os sólidos de 22px (a casa nova, a gaveta de fichário e o relógio com a seta), um embaixo do
//! outro (`padding:8px 0; gap:6px`), ligados com a barrinha na cor de destaque:
//!
//! - **Início**: foca a aba Início ou abre uma (ligado com ela à vista);
//! - **Explorador**: mostra ou esconde o Explorador (ligado com ele à vista; Ctrl+B);
//! - **Histórico de consultas**: põe o Histórico no lugar do Explorador; de novo, esconde;
//! - **Preferências**, no pé da barra (a engrenagem dupla; "Preferências (Ctrl+,)"): abre a janela, e por isso nunca
//!   fica ligado.
//!
//! O que está à vista ([`Paineis`], também as Propriedades da direita) fica só na sessão: as preferências não mudam
//! de formato.

use egui::accesskit::AriaCurrent;
use egui::{Align, Layout, Rect, Ui, UiBuilder, pos2, vec2};
use interface::icones::Icone;
use interface::pecas::botao::{Botao, LADO_RAIL};
use interface::pecas::cartao::RAIO_CARD;
use interface::pecas::painel::pintar_painel_fixo;
use interface::tema::paleta;

/// O painel da esquerda à vista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PainelEsquerdo {
    Explorador,
    Historico,
}

/// Os painéis que a pessoa quer à vista (o Explorador e as Propriedades no começo). As Propriedades ainda somem
/// sozinhas no Início e com o documento estreito demais, sem mudar esta escolha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Paineis {
    pub esquerdo: Option<PainelEsquerdo>,
    pub propriedades: bool,
}

impl Default for Paineis {
    fn default() -> Self {
        Paineis { esquerdo: Some(PainelEsquerdo::Explorador), propriedades: true }
    }
}

impl Paineis {
    /// As Propriedades (o botão da barra de ferramentas e o Ctrl+Alt+B).
    pub(crate) fn alternar_propriedades(&mut self) {
        self.propriedades = !self.propriedades;
    }

    /// O Explorador (o botão e o Ctrl+B): à vista, some; senão, aparece (no lugar do Histórico).
    pub(crate) fn alternar_explorador(&mut self) {
        self.alternar(PainelEsquerdo::Explorador);
    }

    /// O Histórico de consultas: à vista, some; senão, aparece no lugar do Explorador.
    pub(crate) fn alternar_historico(&mut self) {
        self.alternar(PainelEsquerdo::Historico);
    }

    fn alternar(&mut self, painel: PainelEsquerdo) {
        self.esquerdo = if self.esquerdo == Some(painel) { None } else { Some(painel) };
    }
}

/// O botão da barra que foi clicado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PedidoIcones {
    Inicio,
    Explorador,
    Historico,
    /// O botão do pé: abre as Preferências.
    Preferencias,
}

/// A dica do botão do pé ("Preferências (Ctrl+,)"); o leitor de tela ouve só "Preferências".
const DICA_PREFERENCIAS: &str = "Preferências (Ctrl+,)";

/// Desenha a barra em `rect`; `inicio` diz se a aba Início está à vista.
pub(crate) fn barra(ui: &mut Ui, rect: Rect, inicio: bool, paineis: Paineis) -> Option<PedidoIcones> {
    pintar_painel_fixo(ui.painter(), paleta(ui), rect, RAIO_CARD);
    let dentro = Rect::from_min_max(pos2(rect.left(), rect.top() + 8.0), pos2(rect.right(), rect.bottom() - 8.0));
    let mut ui_barra = ui.new_child(
        UiBuilder::new().id_salt("barra-de-icones").max_rect(dentro).layout(Layout::top_down(Align::Center)),
    );
    ui_barra.spacing_mut().item_spacing.y = 6.0;
    let botoes = [
        (PedidoIcones::Inicio, Icone::Inicio, inicio),
        (PedidoIcones::Explorador, Icone::Explorador, paineis.esquerdo == Some(PainelEsquerdo::Explorador)),
        (PedidoIcones::Historico, Icone::Historico, paineis.esquerdo == Some(PainelEsquerdo::Historico)),
    ];
    let mut pedido = None;
    for (qual, icone, ligado) in botoes {
        // a dica é o próprio nome (o `title` das pranchetas)
        let r = ui_barra.add(Botao::icone(icone).rail().selecionado(ligado).dica(icone.nome()));
        // o que está à vista diz ao leitor de tela que é o atual (`aria-current` do `.rail.on`): a página, no Início; o
        // painel, no Explorador e no Histórico. O estado de alternar (`aria-pressed`) já vem do botão.
        if ligado {
            let atual = if qual == PedidoIcones::Inicio { AriaCurrent::Page } else { AriaCurrent::True };
            ui.ctx().accesskit_node_builder(r.id, |no| no.set_aria_current(atual));
        }
        if r.clicked() {
            pedido = Some(qual);
        }
    }
    // Preferências no pé (o `<span style="flex:1">` da prancheta empurra o botão para baixo). Ele abre uma janela:
    // nunca fica ligado (item 29 da revisão), então não é um botão de alternar
    let pe = Rect::from_min_size(
        pos2(dentro.left(), (dentro.bottom() - LADO_RAIL).max(dentro.top())),
        vec2(dentro.width(), LADO_RAIL),
    );
    let mut ui_pe = ui
        .new_child(UiBuilder::new().id_salt("barra-de-icones-pe").max_rect(pe).layout(Layout::top_down(Align::Center)));
    if ui_pe.add(Botao::icone(Icone::Preferencias).rail().dica(DICA_PREFERENCIAS)).clicked() {
        pedido = Some(PedidoIcones::Preferencias);
    }
    pedido
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn explorador_e_historico_se_revezam() {
        let mut p = Paineis::default();
        assert_eq!(p.esquerdo, Some(PainelEsquerdo::Explorador));
        p.alternar_explorador();
        assert_eq!(p.esquerdo, None);
        p.alternar_explorador();
        assert_eq!(p.esquerdo, Some(PainelEsquerdo::Explorador));
        // o Histórico toma o lugar do Explorador, e o Explorador o dele
        p.alternar_historico();
        assert_eq!(p.esquerdo, Some(PainelEsquerdo::Historico));
        p.alternar_explorador();
        assert_eq!(p.esquerdo, Some(PainelEsquerdo::Explorador));
        p.alternar_historico();
        p.alternar_historico();
        assert_eq!(p.esquerdo, None);
        // as Propriedades vão e voltam sozinhas, sem mexer no painel da esquerda
        assert!(p.propriedades);
        p.alternar_propriedades();
        assert!(!p.propriedades && p.esquerdo.is_none());
        p.alternar_propriedades();
        assert!(p.propriedades);
    }
}
