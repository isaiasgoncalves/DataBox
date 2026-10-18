//! A faixa das planilhas de uma pasta do Excel (prancheta 17, `.folhas`; decisões 3, 4 e 5 da etapa 6): a faixa própria
//! de [`ALTURA_FAIXA_FOLHAS`] (32px) entre a grade e o rodapé, com o fundo `Cabecalho` e as riscas, o fio `--btb` em
//! cima, `padding:0 8px` e `gap:4px`.
//!
//! - À esquerda, os dois `.ib.pq` redondos ‹ › ("Rolar as planilhas para a esquerda" / "para a direita"), sempre à
//!   vista e desligados quando não há para onde rolar; a roda do mouse também rola a faixa.
//! - Cada planilha é uma pílula de [`ALTURA_PILULA`] (24px, `padding:0 12px`, 12px): a **ativa** no volume neutro
//!   `--volneu` com a textura, o texto `--t` em 600; as **inativas** transparentes em `--t2`, no hover o fundo `--h` e o
//!   texto `--t`; a **oculta** (só aparece quando quem chama a põe na lista: a caixa "Mostrar planilhas ocultas" da
//!   tela) em `--t3` e itálico. Nome com mais de [`CARACTERES_MAXIMOS`] (22) caracteres corta com "…" ([`nome_curto`]),
//!   e a dica diz o nome inteiro ("Custos (oculta)"). Sem ×: a planilha é só leitura.
//! - À direita, só quando as pílulas não cabem, o botão de **lista** ("Planilhas", a seta para baixo): as planilhas no
//!   menu de sempre (o vidro fosco, [`LARGURA_LISTA`] de largura e até [`ALTURA_MAX_LISTA`] de altura, rolando por
//!   dentro), com a ativa marcada (✓) e as ocultas em `--t3` itálico com "oculta" ou "muito oculta" à direita.
//! - Acessibilidade e teclado: a faixa é o `tablist` "Planilhas"; cada pílula, o `tab` com `selected` e o rótulo
//!   "Vendas, planilha 2 de 4" (", oculta" / ", muito oculta"; [`rotulo_da_folha`]). Entrando na faixa pelo Tab, o foco
//!   vai à ativa; ← e → andam entre as pílulas (a que recebe o foco entra à vista); Enter ou Espaço ativam. O
//!   Ctrl+PageUp/PageDown de dentro da grade é da tela (a faixa não sabe dela).
//!
//! ```ignore
//! let folhas = [Folha::new("Resumo"), Folha::new("Vendas"), Folha::new("Custos").estado(EstadoDaFolha::Oculta)];
//! let r = FaixaDeFolhas::new("pasta", &folhas).ativa(Some(1)).mostrar(ui);
//! if let Some(i) = r.escolhida { /* trocar de planilha */ }
//! ```

use std::borrow::Cow;

use egui::accesskit::Role;
use egui::epaint::RectShape;
use egui::{
    Align, CursorIcon, EventFilter, Id, Key, Layout, Popup, PopupCloseBehavior, Rect, Sense, Stroke, Ui, UiBuilder,
    WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::Icone;
use crate::pecas::botao::{Botao, EstadoVolume, LADO_PEQUENO, Volume, pintar_volume, pintor_de_fora};
use crate::pecas::casca::painel_no_vidro_rolavel;
use crate::pecas::texto::galley;
use crate::pintura::{self, Preenchimento};
use crate::tema::{duracao_transicao, paleta};

/// Altura da faixa das planilhas (`.folhas{height:32px}`).
pub const ALTURA_FAIXA_FOLHAS: f32 = 32.0;
/// Altura de uma pílula (`.fp{height:24px}`).
pub const ALTURA_PILULA: f32 = 24.0;
/// Até quantos caracteres o nome aparece inteiro na pílula (o resto vira "…").
pub const CARACTERES_MAXIMOS: usize = 22;
/// A largura e a altura máxima da lista das planilhas (`.menu-ctx.folhas-m{width:236px;max-height:332px}`).
pub const LARGURA_LISTA: f32 = 236.0;
pub const ALTURA_MAX_LISTA: f32 = 332.0;
/// O `padding:0 8px` e o `gap:4px` da faixa.
const RECUO: f32 = 8.0;
const GAP: f32 = 4.0;
/// O `padding:0 12px` e a borda de 1px da pílula.
const PADDING_PILULA: f32 = 12.0 + 1.0;
/// O rótulo da faixa e do botão da lista.
pub const ROTULO_FAIXA: &str = "Planilhas";
pub const ROLAR_ESQUERDA: &str = "Rolar as planilhas para a esquerda";
pub const ROLAR_DIREITA: &str = "Rolar as planilhas para a direita";

/// Se a planilha aparece na lista de abas do Excel (o `planilha::Estado`, sem depender do crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstadoDaFolha {
    #[default]
    Visivel,
    /// Escondida pelo "Ocultar" do Excel.
    Oculta,
    /// Escondida por macro (`veryHidden`).
    MuitoOculta,
}

impl EstadoDaFolha {
    /// "oculta" ou "muito oculta" (a etiqueta da lista, da dica e do rótulo); nada para a visível.
    pub fn etiqueta(self) -> Option<&'static str> {
        match self {
            EstadoDaFolha::Visivel => None,
            EstadoDaFolha::Oculta => Some("oculta"),
            EstadoDaFolha::MuitoOculta => Some("muito oculta"),
        }
    }

    /// Não é a visível.
    pub fn escondida(self) -> bool {
        self != EstadoDaFolha::Visivel
    }
}

/// Uma planilha da faixa: o nome e o estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Folha<'a> {
    pub nome: &'a str,
    pub estado: EstadoDaFolha,
}

impl<'a> Folha<'a> {
    /// Uma planilha visível.
    pub fn new(nome: &'a str) -> Self {
        Folha { nome, estado: EstadoDaFolha::Visivel }
    }

    pub fn estado(mut self, estado: EstadoDaFolha) -> Self {
        self.estado = estado;
        self
    }
}

/// O nome na pílula: inteiro até [`CARACTERES_MAXIMOS`] caracteres; mais que isso, os 21 primeiros e "…" (22 no total).
pub fn nome_curto(nome: &str) -> Cow<'_, str> {
    if nome.chars().count() <= CARACTERES_MAXIMOS {
        Cow::Borrowed(nome)
    } else {
        let mut s: String = nome.chars().take(CARACTERES_MAXIMOS - 1).collect();
        s.push('…');
        Cow::Owned(s)
    }
}

/// O rótulo do leitor de tela: "Vendas, planilha 2 de 4" (`i` a partir de 0), com ", oculta" ou ", muito oculta".
pub fn rotulo_da_folha(folha: &Folha<'_>, i: usize, n: usize) -> String {
    let base = format!("{}, planilha {} de {n}", folha.nome, i + 1);
    match folha.estado.etiqueta() {
        Some(e) => format!("{base}, {e}"),
        None => base,
    }
}

/// A dica da pílula: o nome inteiro, com "(oculta)" ou "(muito oculta)".
pub fn dica_da_folha(folha: &Folha<'_>) -> String {
    match folha.estado.etiqueta() {
        Some(e) => format!("{} ({e})", folha.nome),
        None => folha.nome.to_owned(),
    }
}

/// O que aconteceu na faixa neste quadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RespostaFolhas {
    /// A planilha escolhida (o clique, o Enter ou o Espaço numa pílula, ou um item da lista).
    pub escolhida: Option<usize>,
    /// O botão da lista está à vista (as pílulas não cabem).
    pub lista: bool,
    /// A faixa inteira.
    pub rect: Rect,
}

/// A faixa das planilhas (veja o topo do módulo). Ocupa a largura disponível e [`ALTURA_FAIXA_FOLHAS`] de altura.
#[must_use = "chame .mostrar(ui)"]
pub struct FaixaDeFolhas<'a> {
    id: Id,
    folhas: &'a [Folha<'a>],
    ativa: Option<usize>,
}

/// O que a faixa guarda entre os quadros: o deslocamento da rolagem, a ativa do último quadro (a que passa a ser a
/// ativa entra à vista) e a pílula que o teclado pediu para trazer à vista.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Memoria {
    deslocamento: f32,
    ativa: Option<usize>,
    trazer: Option<usize>,
}

impl<'a> FaixaDeFolhas<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, folhas: &'a [Folha<'a>]) -> Self {
        FaixaDeFolhas { id: Id::new(("databox::folhas", id_salt)), folhas, ativa: None }
    }

    /// A planilha ativa (índice na lista dada).
    pub fn ativa(mut self, ativa: Option<usize>) -> Self {
        self.ativa = ativa;
        self
    }

    /// O id da pílula `i` (para os testes e para quem quer pôr o foco nela).
    pub fn id_da_folha(&self, i: usize) -> Id {
        self.id.with(("folha", i))
    }

    pub fn mostrar(self, ui: &mut Ui) -> RespostaFolhas {
        let p = paleta(ui);
        let e = &p.estudio;
        let n = self.folhas.len();
        let (faixa, resp_faixa) =
            ui.allocate_exact_size(vec2(ui.available_width(), ALTURA_FAIXA_FOLHAS), Sense::hover());
        let mut resposta = RespostaFolhas { escolhida: None, lista: false, rect: faixa };
        resp_faixa.widget_info(|| WidgetInfo::labeled(WidgetType::Other, ui.is_enabled(), ROTULO_FAIXA));
        ui.ctx().accesskit_node_builder(resp_faixa.id, |no| no.set_role(Role::TabList));

        // o fundo `Cabecalho` com as riscas e o fio `--btb` em cima
        let pintor = ui.painter().with_clip_rect(faixa.intersect(ui.clip_rect()));
        pintura::preencher(&pintor, p, faixa, 0.0, Preenchimento::Cabecalho);
        pintura::riscas(&pintor, p, faixa, 0.0);
        pintor.hline(faixa.x_range(), faixa.top() + 0.5, Stroke::new(1.0, e.btb));

        // as medidas: cada pílula com o peso dela (a ativa em 600)
        let fonte_de = |i: usize| {
            let f = &self.folhas[i];
            let peso = if f.estado.escondida() {
                Peso::Italico
            } else if Some(i) == self.ativa {
                Peso::Medio
            } else {
                Peso::Normal
            };
            fontes::fonte(12.0, peso)
        };
        let galleys: Vec<_> = (0..n)
            .map(|i| galley(ui, &nome_curto(self.folhas[i].nome), fonte_de(i), egui::Color32::PLACEHOLDER))
            .collect();
        let larguras: Vec<f32> = galleys.iter().map(|g| (g.size().x + 2.0 * PADDING_PILULA).ceil()).collect();
        let inicios: Vec<f32> = larguras
            .iter()
            .scan(0.0, |x, l| {
                let inicio = *x;
                *x += l + GAP;
                Some(inicio)
            })
            .collect();
        let total = larguras.iter().sum::<f32>() + GAP * n.saturating_sub(1) as f32;

        // ‹ › à esquerda, a lista à direita só quando as pílulas não cabem
        let y = faixa.center().y;
        let x_seta = faixa.left() + RECUO;
        let rect_esq = Rect::from_min_size(pos2(x_seta, y - LADO_PEQUENO / 2.0), vec2(LADO_PEQUENO, LADO_PEQUENO));
        let rect_dir = rect_esq.translate(vec2(LADO_PEQUENO + GAP, 0.0));
        let inicio_rolo = rect_dir.right() + GAP;
        let fim = (faixa.right() - RECUO).max(inicio_rolo);
        let transborda = total > fim - inicio_rolo + 0.5;
        let fim_rolo = if transborda { (fim - LADO_PEQUENO - GAP).max(inicio_rolo) } else { fim };
        let area =
            Rect::from_min_max(pos2(inicio_rolo, y - ALTURA_PILULA / 2.0), pos2(fim_rolo, y + ALTURA_PILULA / 2.0));
        let excesso = (total - area.width()).max(0.0);
        resposta.lista = transborda;

        // o teclado: a pílula focada, as setas e a entrada pelo Tab
        let ids: Vec<Id> = (0..n).map(|i| self.id_da_folha(i)).collect();
        let id_memoria = self.id.with("memoria");
        let antes: Memoria = ui.ctx().data(|d| d.get_temp(id_memoria)).unwrap_or_default();
        let mut memoria = antes;
        let focada = ui.memory(|m| m.focused()).and_then(|f| ids.iter().position(|&id| id == f));
        if let Some(f) = focada {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(ids[f], EventFilter { horizontal_arrows: true, ..Default::default() })
            });
            let veio_de_fora = !ids.iter().any(|&id| ui.memory(|m| m.had_focus_last_frame(id)));
            let pelo_tab = ui.input(|i| i.key_pressed(Key::Tab));
            let (esq, dir) = ui.input(|i| (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight)));
            let destino = if veio_de_fora && pelo_tab && self.ativa.is_some_and(|a| a != f && a < n) {
                self.ativa
            } else if dir && f + 1 < n {
                Some(f + 1)
            } else if esq && f > 0 {
                Some(f - 1)
            } else {
                None
            };
            if let Some(j) = destino {
                ui.memory_mut(|m| m.request_focus(ids[j]));
                memoria.trazer = Some(j);
                ui.ctx().request_repaint();
            }
        }

        // a rolagem: a roda, as setas, a ativa que mudou e a pílula que o teclado trouxe
        let mut deslocamento = antes.deslocamento;
        if excesso > 0.0 && resp_faixa.contains_pointer() {
            let delta = ui.input(|i| i.smooth_scroll_delta);
            deslocamento -= delta.x + delta.y;
        }
        let trazer = |deslocamento: &mut f32, i: usize| {
            let (x0, x1) = (inicios[i], inicios[i] + larguras[i]);
            if x0 < *deslocamento {
                *deslocamento = x0;
            } else if x1 > *deslocamento + area.width() {
                *deslocamento = x1 - area.width();
            }
        };
        if self.ativa != antes.ativa
            && let Some(i) = self.ativa.filter(|&i| i < n)
        {
            trazer(&mut deslocamento, i);
        }
        if let Some(i) = antes.trazer.filter(|&i| i < n) {
            trazer(&mut deslocamento, i);
            memoria.trazer = memoria.trazer.filter(|&j| j != i);
        }
        deslocamento = deslocamento.clamp(0.0, excesso);

        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(self.id.with("setas"))
                .max_rect(faixa)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let pode_esq = deslocamento > 0.5;
        let pode_dir = deslocamento < excesso - 0.5;
        let mut botao_seta = |rect: Rect, icone: Icone, rotulo: &'static str, ligado: bool| -> bool {
            let mut ui_botao = filho.new_child(UiBuilder::new().max_rect(rect));
            ui_botao.add(Botao::icone(icone).pequeno().rotulo(rotulo).dica(rotulo).habilitado(ligado)).clicked()
        };
        if botao_seta(rect_esq, Icone::Voltar, ROLAR_ESQUERDA, pode_esq) {
            // até o começo da pílula que estava cortada à esquerda
            deslocamento = inicios.iter().rev().copied().find(|&x0| x0 < deslocamento - 0.5).unwrap_or(0.0);
        }
        if botao_seta(rect_dir, Icone::Avancar, ROLAR_DIREITA, pode_dir) {
            // até o fim da pílula que estava cortada à direita
            let borda = deslocamento + area.width();
            deslocamento = inicios
                .iter()
                .zip(&larguras)
                .map(|(x0, l)| x0 + l)
                .find(|&x1| x1 > borda + 0.5)
                .map_or(excesso, |x1| x1 - area.width());
        }
        deslocamento = deslocamento.clamp(0.0, excesso);
        memoria.deslocamento = deslocamento;
        memoria.ativa = self.ativa;
        if memoria != antes {
            ui.ctx().data_mut(|d| d.insert_temp(id_memoria, memoria));
        }

        // as pílulas, recortadas no rolo (as que estão fora continuam com o id, para o foco do teclado)
        let mut rolo = ui.new_child(UiBuilder::new().id_salt(self.id.with("rolo")).max_rect(area));
        rolo.set_clip_rect(area.intersect(ui.clip_rect()));
        let pintor = rolo.painter().clone();
        let pintor_anel = ui.painter().with_clip_rect(faixa.intersect(ui.clip_rect()));
        let tempo = duracao_transicao(ui);
        for (i, folha) in self.folhas.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(area.left() - deslocamento + inicios[i], area.top()),
                vec2(larguras[i], ALTURA_PILULA),
            );
            let resp = rolo.interact(r, ids[i], Sense::click());
            let e_ativa = Some(i) == self.ativa;
            if resp.clicked() {
                resposta.escolhida = Some(i);
            }
            let hover = ui.ctx().animate_bool_with_time(ids[i].with("hover"), resp.hovered() && !e_ativa, tempo);
            if rolo.is_rect_visible(r) {
                if e_ativa {
                    let fora = pintor_de_fora(&rolo, r);
                    let estado = EstadoVolume { presenca: 1.0, hover: 0.0, pressao: 0.0 };
                    pintar_volume(&pintor, &fora, p, r, f32::INFINITY, &Volume::neutro(p), estado);
                } else if hover > 0.0 {
                    pintor.add(RectShape::filled(r, pintura::canto(r, f32::INFINITY), e.h.gamma_multiply(hover)));
                }
                let cor = if folha.estado.escondida() {
                    e.t3
                } else if e_ativa {
                    e.t
                } else {
                    pintura::misturar(e.t2, e.t, hover)
                };
                let g = galley(ui, &nome_curto(folha.nome), fonte_de(i), cor);
                pintor.galley(pos2(r.left() + PADDING_PILULA, r.center().y - g.size().y / 2.0), g, cor);
            }
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&pintor_anel, p, r.intersect(area), f32::INFINITY);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let rotulo = rotulo_da_folha(folha, i, n);
            resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), e_ativa, &rotulo));
            ui.ctx().accesskit_node_builder(resp.id, |no| {
                no.set_role(Role::Tab);
                no.set_selected(e_ativa);
            });
            let _ = resp.on_hover_text(dica_da_folha(folha));
        }

        if transborda {
            let rect =
                Rect::from_min_size(pos2(fim - LADO_PEQUENO, y - LADO_PEQUENO / 2.0), vec2(LADO_PEQUENO, LADO_PEQUENO));
            if let Some(i) = self.lista(ui, rect) {
                resposta.escolhida = Some(i);
            }
        }
        resposta
    }

    /// O botão da lista e o menu das planilhas, por cima da faixa e alinhado à direita do botão. Devolve a escolhida.
    fn lista(&self, ui: &mut Ui, rect: Rect) -> Option<usize> {
        let id_painel = self.id.with("lista");
        let aberto = Popup::is_id_open(ui.ctx(), id_painel);
        let mut ui_botao = ui.new_child(UiBuilder::new().id_salt(self.id.with("botao_lista")).max_rect(rect));
        let botao =
            Botao::icone(Icone::SetaBaixo).pequeno().rotulo(ROTULO_FAIXA).dica(ROTULO_FAIXA).selecionado(aberto);
        let resp = ui_botao.add(botao);
        ui.ctx().accesskit_node_builder(resp.id, |no| no.set_expanded(aberto));
        let n = self.folhas.len();
        let mut escolhida = None;
        let popup = Popup::new(id_painel, ui.ctx().clone(), resp.rect, ui.layer_id())
            .open_memory(resp.clicked().then_some(egui::SetOpenCommand::Toggle))
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(egui::RectAlign::TOP_END)
            .gap(3.0)
            .width(LARGURA_LISTA)
            .frame(egui::Frame::NONE);
        let id_rolagem = self.id.with("lista_rolagem");
        popup.show(|ui| {
            let (_, fechar, _) = painel_no_vidro_rolavel(ui, id_rolagem, LARGURA_LISTA, ALTURA_MAX_LISTA, |m| {
                m.com_marcas();
                for (i, folha) in self.folhas.iter().enumerate() {
                    let marcada = Some(i) == self.ativa;
                    let r = if folha.estado.escondida() {
                        m.item_marcado_fraco(folha.nome, folha.estado.etiqueta(), marcada)
                    } else {
                        m.item_marcado(folha.nome, None, marcada)
                    };
                    if marcada && !aberto {
                        // a lista abre com a ativa à vista
                        r.scroll_to_me(Some(Align::Center));
                    }
                    if r.clicked() && i < n {
                        escolhida = Some(i);
                    }
                }
            });
            if fechar {
                Popup::close_id(ui.ctx(), id_painel);
            }
        });
        escolhida
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_curto_corta_em_22_caracteres() {
        assert_eq!(nome_curto("Vendas"), "Vendas");
        let exato = "a".repeat(CARACTERES_MAXIMOS);
        assert_eq!(nome_curto(&exato), exato.as_str());
        let longo = nome_curto("Lançamentos contábeis de outubro de 2026");
        assert_eq!(longo.chars().count(), CARACTERES_MAXIMOS);
        assert!(longo.ends_with('…') && longo.starts_with("Lançamentos contábeis"));
    }

    #[test]
    fn rotulo_e_dica_dizem_o_estado() {
        let v = Folha::new("Vendas");
        assert_eq!(rotulo_da_folha(&v, 1, 4), "Vendas, planilha 2 de 4");
        assert_eq!(dica_da_folha(&v), "Vendas");
        let c = Folha::new("Custos").estado(EstadoDaFolha::Oculta);
        assert_eq!(rotulo_da_folha(&c, 3, 4), "Custos, planilha 4 de 4, oculta");
        assert_eq!(dica_da_folha(&c), "Custos (oculta)");
        let m = Folha::new("Parâmetros").estado(EstadoDaFolha::MuitoOculta);
        assert_eq!(rotulo_da_folha(&m, 8, 12), "Parâmetros, planilha 9 de 12, muito oculta");
        assert!(!EstadoDaFolha::Visivel.escondida() && EstadoDaFolha::MuitoOculta.escondida());
    }
}
