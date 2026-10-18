//! O documento da aba ativa (o `.doc` de `Main.dc.html`, `Dados.dc.html` e `Consulta.dc.html`): a moldura (fundo
//! `--c`, borda de 1px `--vidrob`, raio [`RAIO_PADRAO`]) e, de cima para baixo, por dentro da borda e de ponta a
//! ponta:
//!
//! - os avisos do topo ([`Documento::aviso`], o `.aviso` da prancheta 6: a cor do estado a 12% e o fio de baixo a
//!   35%, sem caixa);
//! - a faixa de contexto de 42px ([`ALTURA_FAIXA`], [`Documento::faixa`]): `padding:0 10px 0 14px`, `gap:8px`, o
//!   degradê do cabeçalho (`--sup-meio` → `--bt2`) com as riscas e o fio `--btb` embaixo;
//! - o conteúdo: com [`Rolagem::Pagina`], uma página que rola ([`Documento::pagina`], `padding:18px 20px 20px`
//!   debaixo da faixa, 20px sem ela; [`de_ponta_a_ponta`] tira o `padding` de uma parte dela, como a lista de colunas
//!   da Estrutura); com [`Rolagem::Propria`], o retângulo inteiro para quem rola por dentro ([`Documento::propria`]);
//! - o rodapé de 34px ([`ALTURA_RODAPE`], [`Documento::rodape`]): `padding:0 8px 0 14px`, `gap:8px`, fundo `--p`,
//!   o fio `--b` em cima e o texto em `--t2`.
//!
//! As partes são desenhadas na ordem em que são chamadas (a ordem do Tab e da árvore de acessibilidade): avisos,
//! faixa, conteúdo e rodapé. O lugar do rodapé fica reservado desde o começo ([`Documento::abrir`]).
//!
//! ```ignore
//! let mut d = Documento::abrir(ui, rect, None, true);
//! d.aviso(ui, TomAviso::Atencao, |ui, cores| { /* a frase e o link */ });
//! d.faixa(ui, |ui| { ui.label("Filtros"); });
//! d.pagina(ui, Pagina::new(("doc", id)), |ui| { /* o conteúdo */ });
//! d.rodape(ui, |ui| { ui.label("Linhas 1–41 de 41"); });
//! ```

use egui::{
    Align, CornerRadius, Id, Layout, Rangef, Rect, ScrollArea, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2,
};

use crate::pecas::abas_documento::pintar_documento;
use crate::pecas::avisos::{Aviso, CoresAviso, TomAviso, aviso_do_documento};
use crate::pecas::texto::{RespostaTexto, Trecho};
use crate::pintura::{self, Preenchimento, RAIO_PADRAO};
use crate::tema::paleta;

/// A altura da faixa de contexto (`.faixa{height:42px}`, com o fio de baixo).
pub const ALTURA_FAIXA: f32 = 42.0;
/// A altura do rodapé (`.rodape{height:34px}`, com o fio de cima).
pub const ALTURA_RODAPE: f32 = 34.0;
/// O `padding` da página: 20px dos lados e embaixo; em cima, 18px debaixo da faixa e 20px sem ela.
pub const RECUO_PAGINA: f32 = 20.0;
pub const RECUO_PAGINA_COM_FAIXA: f32 = 18.0;
/// `.faixa{padding:0 10px 0 14px}` e `.rodape{padding:0 8px 0 14px}`, com `gap:8px`.
const RECUO_ESQUERDA: f32 = 14.0;
const RECUO_DIREITA_FAIXA: f32 = 10.0;
const RECUO_DIREITA_RODAPE: f32 = 8.0;
const GAP: f32 = 8.0;

/// Quem rola o conteúdo do documento.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Rolagem {
    /// A casca rola o conteúdo como uma página (com o PageUp, o PageDown, o Espaço, o Home e o End quando nada tem o
    /// foco).
    #[default]
    Pagina,
    /// O conteúdo recebe o retângulo inteiro e rola por dentro (a grade dos Dados, o editor da Consulta): a casca não
    /// intercepta as teclas de rolagem.
    Propria,
}

/// A página que rola ([`Documento::pagina`]): o id da rolagem (cada aba guarda a sua) e o que o teclado pediu.
#[derive(Clone, Copy, Debug)]
pub struct Pagina {
    pub id: Id,
    /// Rolar tantos pontos (positivo sobe), do PageUp e do PageDown.
    pub rolar_por: Option<f32>,
    /// Voltar ao topo (o Home).
    pub ao_topo: bool,
}

impl Pagina {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Pagina {
        Pagina { id: Id::new(id_salt), rolar_por: None, ao_topo: false }
    }
}

/// O documento sendo desenhado: a moldura já pintada e o espaço que sobra entre o que já foi desenhado em cima e o
/// rodapé reservado embaixo.
pub struct Documento {
    rect: Rect,
    /// Por dentro da borda.
    dentro: Rect,
    /// Onde a próxima parte de cima começa.
    topo: f32,
    /// Onde o conteúdo termina (o começo do rodapé, ou o fim de dentro da borda).
    base: f32,
    com_faixa: bool,
    com_rodape: bool,
}

/// Os cantos de cima (`nw`, `ne`) com o raio de dentro da borda, para a parte que encosta no alto da moldura.
fn cantos_de_cima(sim: bool) -> CornerRadius {
    let r = if sim { (RAIO_PADRAO - 1.0) as u8 } else { 0 };
    CornerRadius { nw: r, ne: r, sw: 0, se: 0 }
}

impl Documento {
    /// Pinta a moldura em `rect` (com a junção com a aba ativa, ver `abas_documento::pintar_documento`) e reserva o
    /// rodapé embaixo, se houver.
    pub fn abrir(ui: &Ui, rect: Rect, juncao: Option<Rangef>, com_rodape: bool) -> Documento {
        pintar_documento(ui.painter(), paleta(ui), rect, juncao);
        let dentro = rect.shrink(1.0);
        let base = if com_rodape { dentro.bottom() - ALTURA_RODAPE } else { dentro.bottom() };
        Documento { rect, dentro, topo: dentro.top(), base, com_faixa: false, com_rodape }
    }

    /// A moldura inteira (com a borda).
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// O espaço que sobra para o conteúdo agora (entre o que já foi desenhado em cima e o rodapé).
    pub fn rect_conteudo(&self) -> Rect {
        Rect::from_min_max(pos2(self.dentro.left(), self.topo), pos2(self.dentro.right(), self.base.max(self.topo)))
    }

    fn encosta_no_alto(&self) -> bool {
        self.topo <= self.dentro.top() + 0.01
    }

    /// Um aviso de ponta a ponta no alto (o `.aviso` da prancheta 6: a transação aberta, a troca de banco que
    /// demora), com o conteúdo livre de [`Aviso::mostrar`]. Vários avisos ficam um embaixo do outro.
    pub fn aviso<R>(&mut self, ui: &mut Ui, tom: TomAviso, conteudo: impl FnOnce(&mut Ui, CoresAviso) -> R) -> R {
        let r = Rect::from_min_max(pos2(self.dentro.left(), self.topo), pos2(self.dentro.right(), self.base));
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(("databox::documento::aviso", self.topo as i32))
                .max_rect(r)
                .layout(Layout::top_down(Align::Min)),
        );
        filho.set_clip_rect(r.intersect(ui.clip_rect()));
        let (resp, saida) = Aviso::com_tom(tom)
            .faixa_do_documento(cantos_de_cima(self.encosta_no_alto()))
            .mostrar_com(&mut filho, conteudo);
        self.topo = resp.rect.bottom().min(self.base);
        saida
    }

    /// Um aviso de ponta a ponta no alto com a frase em trechos (o `strong`, os links): o [`aviso_do_documento`].
    /// Devolve a resposta e o link clicado.
    pub fn aviso_texto(&mut self, ui: &mut Ui, tom: TomAviso, trechos: &[Trecho<'_>]) -> RespostaTexto {
        let r = Rect::from_min_max(pos2(self.dentro.left(), self.topo), pos2(self.dentro.right(), self.base));
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(("databox::documento::aviso", self.topo as i32))
                .max_rect(r)
                .layout(Layout::top_down(Align::Min)),
        );
        filho.set_clip_rect(r.intersect(ui.clip_rect()));
        let resposta = aviso_do_documento(&mut filho, trechos, tom, cantos_de_cima(self.encosta_no_alto()));
        self.topo = resposta.resposta.rect.bottom().min(self.base);
        resposta
    }

    /// A faixa de contexto de 42px, com `conteudo` num `Ui` em linha, centrado na altura.
    pub fn faixa<R>(&mut self, ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        let p = paleta(ui);
        let rect = Rect::from_min_size(pos2(self.dentro.left(), self.topo), vec2(self.dentro.width(), ALTURA_FAIXA));
        let cantos = cantos_de_cima(self.encosta_no_alto());
        let painter = ui.painter();
        let mut fundo = pintura::forma_preenchida(ui.ctx(), p, rect, 0.0, Preenchimento::Cabecalho);
        if let Shape::Rect(f) = &mut fundo {
            f.corner_radius = cantos;
        }
        painter.add(fundo);
        pintura::riscas_com_canto(painter, p, rect, cantos);
        painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.estudio.btb));
        let linha = Rect::from_min_max(
            pos2(rect.left() + RECUO_ESQUERDA, rect.top()),
            pos2(rect.right() - RECUO_DIREITA_FAIXA, rect.bottom() - 1.0),
        );
        self.topo = rect.bottom();
        self.com_faixa = true;
        em_linha(ui, linha, "databox::documento::faixa", conteudo)
    }

    /// O conteúdo numa página que rola, com o `padding` das pranchetas (18px em cima debaixo da faixa, 20px sem ela, e
    /// 20px dos lados e embaixo). `conteudo` recebe a coluna de cima para baixo, sem espaço entre os itens.
    pub fn pagina<R>(&mut self, ui: &mut Ui, pagina: Pagina, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        let area = self.rect_conteudo();
        let topo_recuo = if self.com_faixa { RECUO_PAGINA_COM_FAIXA } else { RECUO_PAGINA };
        let mut ui_pagina = ui.new_child(
            UiBuilder::new().id_salt("databox::documento::pagina").max_rect(area).layout(Layout::top_down(Align::Min)),
        );
        ui_pagina.set_clip_rect(area.intersect(ui.clip_rect()));
        // a calha de 8px da barra: quando a página rola, as listas de ponta a ponta não passam por baixo dela
        ui_pagina.spacing_mut().scroll.floating_allocated_width = crate::grade::CALHA_ROLAGEM;
        let mut rolagem = ScrollArea::vertical().id_salt(pagina.id).auto_shrink([false, false]);
        if pagina.ao_topo {
            rolagem = rolagem.vertical_scroll_offset(0.0);
        }
        let saida = rolagem.show(&mut ui_pagina, |ui| {
            if let Some(d) = pagina.rolar_por {
                ui.scroll_with_delta(vec2(0.0, d));
            }
            let inicio = ui.cursor().min;
            // a largura que a rolagem deixa (menos a calha, quando há barra)
            let direita = ui.max_rect().right().min(area.right());
            let largura = (direita - area.left() - 2.0 * RECUO_PAGINA).max(0.0);
            let coluna =
                Rect::from_min_size(pos2(inicio.x + RECUO_PAGINA, inicio.y + topo_recuo), vec2(largura, f32::INFINITY));
            let mut ui_coluna =
                ui.new_child(UiBuilder::new().id_salt("coluna").max_rect(coluna).layout(Layout::top_down(Align::Min)));
            ui_coluna.spacing_mut().item_spacing.y = 0.0;
            let atual = PaginaAtual { coluna: coluna.min, esquerda: area.left(), direita, topo: inicio.y };
            ui.ctx().data_mut(|d| d.insert_temp(id_pagina_atual(), atual));
            let r = conteudo(&mut ui_coluna);
            ui.ctx().data_mut(|d| d.remove_temp::<PaginaAtual>(id_pagina_atual()));
            let mut fim = ui_coluna.min_rect().bottom().max(inicio.y) + RECUO_PAGINA;
            // um conteúdo que enche a altura toda (a grade de rolagem própria) deixaria o fim no infinito, e a área
            // de rolagem guardaria uma altura sem fim: sem fim de página, a coluna termina onde começou
            if !fim.is_finite() {
                fim = inicio.y;
            }
            ui.allocate_rect(Rect::from_min_max(inicio, pos2(direita, fim)), Sense::hover());
            r
        });
        self.topo = self.base;
        saida.inner
    }

    /// O conteúdo que rola por dentro: o retângulo inteiro que sobra, sem `padding`.
    pub fn propria<R>(&mut self, ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        let area = self.rect_conteudo();
        let mut filho = ui.new_child(
            UiBuilder::new().id_salt("databox::documento::propria").max_rect(area).layout(Layout::top_down(Align::Min)),
        );
        filho.set_clip_rect(area.intersect(ui.clip_rect()));
        let r = conteudo(&mut filho);
        self.topo = self.base;
        r
    }

    /// O rodapé de 34px no lugar reservado em [`Documento::abrir`], com `conteudo` num `Ui` em linha, centrado na
    /// altura (o texto em `--t2`). Sem o rodapé reservado, não desenha nada.
    pub fn rodape<R>(&mut self, ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if !self.com_rodape {
            return None;
        }
        let p = paleta(ui);
        let rect = Rect::from_min_max(pos2(self.dentro.left(), self.base), self.dentro.max);
        let r = (RAIO_PADRAO - 1.0) as u8;
        let cantos = CornerRadius { nw: 0, ne: 0, sw: r, se: r };
        ui.painter().rect_filled(rect, cantos, p.estudio.faixa);
        ui.painter().hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, p.estudio.b));
        let linha = Rect::from_min_max(
            pos2(rect.left() + RECUO_ESQUERDA, rect.top() + 1.0),
            pos2(rect.right() - RECUO_DIREITA_RODAPE, rect.bottom()),
        );
        Some(em_linha(ui, linha, "databox::documento::rodape", |ui| {
            ui.visuals_mut().override_text_color = Some(p.estudio.t2);
            conteudo(ui)
        }))
    }
}

/// A página que está sendo desenhada ([`Documento::pagina`]): o canto da coluna com o `padding`, os lados de dentro da
/// moldura e o alto da página (para o [`de_ponta_a_ponta`]).
#[derive(Clone, Copy, Debug, Default)]
struct PaginaAtual {
    coluna: egui::Pos2,
    esquerda: f32,
    direita: f32,
    topo: f32,
}

fn id_pagina_atual() -> Id {
    Id::new("databox::documento::pagina_atual")
}

/// Na coluna da [`Documento::pagina`]: o conteúdo de ponta a ponta, sem o `padding` dos lados e, no alto da página,
/// sem o de cima (a lista de colunas da Estrutura, que encosta na faixa e nas bordas: prancheta 6). O que vem depois
/// continua na coluna, com o `padding`. Fora da página (os palcos das telas, que empilham as partes), desenha no
/// lugar, como um `scope`.
pub fn de_ponta_a_ponta<R>(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let pagina: Option<PaginaAtual> = ui.ctx().data(|d| d.get_temp(id_pagina_atual()));
    let na_coluna = pagina.filter(|p| (ui.max_rect().left() - p.coluna.x).abs() < 0.5);
    let Some(p) = na_coluna else { return ui.scope(conteudo).inner };
    let cursor = ui.cursor().min;
    let no_alto = (cursor.y - p.coluna.y).abs() < 0.5;
    let topo = if no_alto { p.topo } else { cursor.y };
    let rect = Rect::from_min_max(pos2(p.esquerda, topo), pos2(p.direita, f32::INFINITY));
    let mut filho = ui.new_child(
        UiBuilder::new()
            .id_salt("databox::documento::ponta_a_ponta")
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    let r = conteudo(&mut filho);
    let usado = filho.min_rect();
    let mut fim = if usado.is_positive() { usado.bottom() } else { topo };
    // o que enche a altura toda não tem fim a que a coluna possa avançar
    if !fim.is_finite() {
        fim = cursor.y;
    }
    // a coluna avança até o fim do que foi desenhado, nos lados dela
    ui.advance_cursor_after_rect(Rect::from_min_max(
        pos2(ui.max_rect().left(), cursor.y),
        pos2(ui.max_rect().right(), fim.max(cursor.y)),
    ));
    r
}

/// Um `Ui` em linha no retângulo, centrado na altura, com 8px entre os itens.
fn em_linha<R>(ui: &mut Ui, rect: Rect, id: &'static str, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let mut filho =
        ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    filho.set_clip_rect(rect.intersect(ui.clip_rect()));
    filho.spacing_mut().item_spacing.x = GAP;
    conteudo(&mut filho)
}
