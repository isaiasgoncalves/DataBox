//! Os filtros das grades, iguais na aba Dados (a faixa de contexto, prancheta 2) e no CSV (a `.barra-filtros` no alto
//! do documento, prancheta 7; revisão de UI de 19/10/2026; o CSV não tem faixa desde a 0.2.3):
//!
//! - [`filtros`]: "Filtros" (o `.tit` em `--t3`), uma etiqueta por condição ([`chip_filtro`]: o clique abre a condição
//!   no popover, o × a tira), "+ Condição" e, à direita, "Limpar filtros" (só com condição);
//!   [`filtros_com_direita`] põe mais alguma coisa na ponta direita (a contagem da busca do CSV, "3 de 41"). A
//!   contagem das linhas não mora aqui: fica no rodapé da grade, "Linhas 1–24 de 236 · filtradas de 1.180" (U3 da
//!   revisão de UI de 22/10/2026; até a 0.2.3, "236 de 1.180 linhas" na ponta direita desta barra);
//! - [`barra_filtros`]: a `.barra-filtros` de 38px ([`ALTURA_BARRA_FILTROS`], `padding:0 10px 0 14px`, `gap:8px` e o
//!   fio `--b` embaixo), para quem não põe os filtros na faixa de contexto;
//! - [`popover_condicao`]: o popover de vidro da condição (sem prancheta): Coluna, Operador, Valor (fora nos
//!   operadores sem valor), Cancelar e Aplicar; Enter no valor aplica, Esc, Cancelar e o clique fora desistem.
//!
//! A tela guarda as condições e diz o que fazer com a [`RespostaFiltros`]; os operadores são os dela (os 14 do banco
//! na aba Dados, os do CSV no CSV).

use std::hash::Hash;

use egui::{Align, Id, Layout, Rect, Sense, Stroke, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};

use crate::icones::Icone;
use crate::pecas::botao::{Botao, LADO_PEQUENO};
use crate::pecas::campos::{Entrada, Select, campo, enter_no_campo};
use crate::pecas::popover::Popover;
use crate::pecas::rotulos::chip_filtro;
use crate::pecas::texto::galley_tit;
use crate::tema::paleta;

/// A contagem do rodapé à espera do total (a tabela grande com busca ou filtro, a vista do CSV em cálculo).
pub const CONTANDO: &str = "contando…";
/// A contagem que falhou (o motivo vai na dica).
pub const LINHAS_NAO_CONTADAS: &str = "linhas não contadas";
/// A altura da `.barra-filtros` (`height:38px`, com o fio de baixo).
pub const ALTURA_BARRA_FILTROS: f32 = 38.0;
/// A largura do popover da condição.
pub const LARGURA_CONDICAO: f32 = 300.0;
/// `.barra-filtros{padding:0 10px 0 14px; gap:8px}`.
const RECUO_ESQUERDA: f32 = 14.0;
const RECUO_DIREITA: f32 = 10.0;
const GAP: f32 = 8.0;

/// Uma condição aplicada, como a etiqueta a mostra: "ativo igual a 1" (a coluna e o valor em 600).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtiquetaFiltro<'a> {
    /// O id da condição na tela (volta em [`RespostaFiltros::abrir`] e [`RespostaFiltros::tirar`]).
    pub id: u64,
    pub coluna: &'a str,
    /// O rótulo do operador ("igual a").
    pub operador: &'a str,
    /// O valor; `None` nos operadores sem valor ("é nulo").
    pub valor: Option<&'a str>,
}

/// O que a pessoa fez nos filtros neste quadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RespostaFiltros {
    /// Abrir o popover: numa condição nova (`None`) ou na de uma etiqueta, preso embaixo do retângulo.
    pub abrir: Option<(Option<u64>, Rect)>,
    /// O × de uma etiqueta.
    pub tirar: Option<u64>,
    /// "Limpar filtros".
    pub limpar: bool,
    /// Onde ficou o "+ Condição" (o verbo Filtrar abre o popover preso nele).
    pub nova_condicao: Rect,
}

/// "Filtros", as etiquetas, "+ Condição" e, à direita, "Limpar filtros" (só com etiqueta), num `Ui` em linha (a faixa
/// de contexto ou a [`barra_filtros`]).
pub fn filtros(ui: &mut Ui, etiquetas: &[EtiquetaFiltro<'_>]) -> RespostaFiltros {
    filtros_com_direita(ui, etiquetas, |_| {})
}

/// Os [`filtros`] com mais alguma coisa na ponta direita, depois do "Limpar filtros" (a contagem da busca da barra do
/// CSV, "3 de 41", que não tem faixa de contexto): `direita` desenha num `Ui` da direita para a esquerda.
pub fn filtros_com_direita(
    ui: &mut Ui,
    etiquetas: &[EtiquetaFiltro<'_>],
    direita: impl FnOnce(&mut Ui),
) -> RespostaFiltros {
    let p = paleta(ui);
    let t3 = p.estudio.t3;
    let g = galley_tit(ui, "Filtros", t3);
    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, t3);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, "Filtros"));
    let mut abrir = None;
    let mut tirar = None;
    for e in etiquetas {
        let mut trechos = vec![(e.coluna, true), (e.operador, false)];
        if let Some(v) = e.valor {
            trechos.push((v, true));
        }
        let r = chip_filtro(ui, &trechos, "Tirar o filtro");
        if r.fechar {
            tirar = Some(e.id);
        } else if r.abrir {
            abrir = Some((Some(e.id), r.resposta.rect));
        }
    }
    let b = ui.add(
        Botao::ghost("Condição")
            .com_icone(Icone::Mais)
            .pequeno()
            .rotulo("Nova condição")
            .dica("Nova condição de filtro"),
    );
    if b.clicked() {
        abrir = Some((None, b.rect));
    }
    let mut limpar = false;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        direita(ui);
        if !etiquetas.is_empty() && ui.add(Botao::ghost("Limpar filtros").pequeno()).clicked() {
            limpar = true;
        }
    });
    RespostaFiltros { abrir, tirar, limpar, nova_condicao: b.rect }
}

/// A `.barra-filtros` de 38px no alto de `ui` (de ponta a ponta, o fio `--b` embaixo), com `conteudo` num `Ui` em
/// linha, centrado na altura, com 8px entre os itens. O `ui` avança até o fim dela.
pub fn barra_filtros<R>(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let p = paleta(ui);
    let topo = ui.cursor().top();
    let rect = Rect::from_min_size(pos2(ui.max_rect().left(), topo), vec2(ui.max_rect().width(), ALTURA_BARRA_FILTROS));
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.estudio.b));
    let linha = Rect::from_min_max(
        pos2(rect.left() + RECUO_ESQUERDA, rect.top()),
        pos2(rect.right() - RECUO_DIREITA, rect.bottom() - 1.0),
    );
    let mut filho = ui.new_child(
        UiBuilder::new().id_salt("databox::barra_filtros").max_rect(linha).layout(Layout::left_to_right(Align::Center)),
    );
    filho.set_clip_rect(linha.intersect(ui.clip_rect()));
    filho.spacing_mut().item_spacing.x = GAP;
    let r = conteudo(&mut filho);
    ui.advance_cursor_after_rect(rect);
    r
}

/// A condição aberta no popover: a nova (`alvo` None) ou a de uma etiqueta, com a coluna (o índice na lista de
/// colunas), o operador e o valor em edição.
#[derive(Debug, Clone, PartialEq)]
pub struct CondicaoEmEdicao<O> {
    pub alvo: Option<u64>,
    pub coluna: usize,
    pub op: O,
    pub valor: String,
    /// Onde o popover fica preso (a etiqueta ou o "+ Condição").
    pub ancora: Rect,
    /// Pôr o foco no campo do valor no próximo quadro.
    pub focar: bool,
}

/// Como o popover terminou neste quadro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FimCondicao {
    /// Aplicar (o botão ou Enter no valor): a tela troca a condição da etiqueta ou acrescenta a nova.
    Aplicar,
    /// Cancelar, Esc ou o clique fora.
    Desistir,
}

/// O popover da condição `c` (com o id `id`, o mesmo a cada quadro): Coluna (`colunas`), Operador (`operadores`),
/// Valor (fora quando `sem_valor` diz que o operador não leva), Cancelar e Aplicar (desligado sem valor, com a dica
/// "Escreva um valor para aplicar"). Trocar para um operador com valor põe o foco no campo.
pub fn popover_condicao<O: PartialEq + Clone>(
    ectx: &egui::Context,
    id: impl Hash + std::fmt::Debug,
    c: &mut CondicaoEmEdicao<O>,
    colunas: &[(usize, &str)],
    operadores: &[(O, &str)],
    sem_valor: impl Fn(&O) -> bool,
) -> Option<FimCondicao> {
    let id = Id::new(("databox::filtros", id));
    let (mut aplicar, mut cancelar) = (false, false);
    let r = Popover::new(("condicao", id), c.ancora).largura(LARGURA_CONDICAO).rotulo("Condição de filtro").mostrar(
        ectx,
        |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            campo(ui, "Coluna", None, |ui| {
                Select::new(("coluna_condicao", id), &mut c.coluna, colunas).rotulo("Coluna da condição").mostrar(ui);
            });
            let mudou = campo(ui, "Operador", None, |ui| {
                Select::new(("op_condicao", id), &mut c.op, operadores)
                    .rotulo("Operador da condição")
                    .mostrar(ui)
                    .changed()
            });
            let com_valor = !sem_valor(&c.op);
            if mudou && com_valor {
                c.focar = true;
            }
            let pode = !com_valor || !c.valor.is_empty();
            if com_valor {
                campo(ui, "Valor", None, |ui| {
                    let r = Entrada::new(&mut c.valor)
                        .dica("valor a procurar")
                        .rotulo("Valor da condição")
                        .id_salt(("valor_condicao", id))
                        .mostrar(ui)
                        .response
                        .response;
                    if std::mem::take(&mut c.focar) {
                        r.request_focus();
                    }
                    if enter_no_campo(ui, &r) && !c.valor.is_empty() {
                        aplicar = true;
                    }
                });
            }
            let largura = ui.available_width();
            ui.allocate_ui_with_layout(vec2(largura, LADO_PEQUENO), Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let b = ui.add(Botao::primario("Aplicar").pequeno().habilitado(pode));
                if b.on_disabled_hover_text("Escreva um valor para aplicar").clicked() {
                    aplicar = true;
                }
                if ui.add(Botao::ghost("Cancelar").pequeno()).clicked() {
                    cancelar = true;
                }
            });
        },
    );
    if aplicar {
        Some(FimCondicao::Aplicar)
    } else if r.fechar || cancelar {
        Some(FimCondicao::Desistir)
    } else {
        None
    }
}
