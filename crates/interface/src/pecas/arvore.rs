//! A árvore do Explorador (o `.no` de `Main.dc.html` e `Dados.dc.html`): nós de 24px ([`ALTURA_NO`]) com
//! `margin:0 6px`, raio 6 ([`RAIO_NO`]), o recuo do nível ([`recuo`]), a seta de abrir de 10px em `--t3` (ou o
//! espaço dela, `.vz`), o ícone (16px nos sólidos, 14px nas silhuetas), o nome em 12px `--t2` e a contagem à
//! direita (`.q`: 11px `--t3`, algarismos tabulares), a silhueta de 13px dela ([`No::icone_a_direita`]) ou o
//! ícone de estado de 16px ([`No::estado`]: o Atenção do banco com a transação aberta).
//!
//! - Hover: o fundo `--h` e o texto `--t`. Ativo (`.no.on`): o fundo `--sel`, o realce de 1px e o brilho fraco do
//!   destaque, como os itens da lateral; a contagem passa a `--t2` ([`cor_contagem_no`]).
//! - O clique no nó é a resposta ([`RespostaNo::resposta`]); o clique na seta só abre ou fecha
//!   ([`RespostaNo::alternou`]). Com o foco, → abre e ← fecha, e o foco fica no nó (as setas dos lados ficam com
//!   ele). A seta não é uma parada do Tab: o nó já é.
//! - Acessibilidade: papel `link` com `selected` (o nó ativo) e `expanded` (os que abrem); o rótulo é o nome, ou
//!   [`No::rotulo`] quando o nome se repete ("clientes · Estrutura" no nó que mostra só "Estrutura"). O rótulo
//!   também é o id do nó (o foco e o hover não pulam quando a árvore abre ou fecha): ele não pode se repetir no
//!   mesmo `Ui`.
//!
//! ```ignore
//! let r = No::new("clientes").recuo(recuo(2)).seta(Seta::Fechada).icone(Icone::Tabela).contagem("60").mostrar(ui);
//! if r.alternou { /* abre ou fecha */ }
//! if r.resposta.clicked() { /* abre a aba */ }
//! ```

use egui::{
    Color32, CursorIcon, EventFilter, FocusDirection, Key, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2,
    vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Familia, Icone};
use crate::pecas::botao::{pintor_de_fora, realce_curvo};
use crate::pecas::texto::{galley, galley_cortado};
use crate::pintura;
use crate::tema::{Paleta, duracao_transicao, paleta, rgba};

/// Altura de um nó (`.no{height:24px}`).
pub const ALTURA_NO: f32 = 24.0;
/// `margin:0 6px` do nó dentro do painel.
pub const MARGEM_NO: f32 = 6.0;
/// Raio do nó (o dos itens das listas dos painéis da esquerda, como o `.hist` do Histórico).
pub const RAIO_NO: f32 = 6.0;
/// A seta (`.seta`, 10px) e o espaço dela (`.vz`).
const LADO_SETA: f32 = 10.0;
/// `gap:6px` entre a seta, o ícone e o nome.
const GAP: f32 = 6.0;
/// O nó da navegação das Preferências ([`No::lateral`], `.pref-nav .no`): 32px, `margin:0 8px`, `gap:10px`,
/// `padding:0 12px 0 9px` e o sólido de 20px.
pub const ALTURA_NO_LATERAL: f32 = 32.0;
const MARGEM_NO_LATERAL: f32 = 8.0;
const GAP_LATERAL: f32 = 10.0;
const RECUO_LATERAL: f32 = 9.0;
const RECUO_DIREITA_LATERAL: f32 = 12.0;
const LADO_ICONE_LATERAL: f32 = 20.0;
/// `padding-right:10px`.
const RECUO_DIREITA: f32 = 10.0;
/// O ícone de estado à direita ([`No::estado`], `.q .sk.s16`).
const LADO_ESTADO: f32 = 16.0;
/// A silhueta à direita ([`No::icone_a_direita`], `.q .sil{width:13px}`).
const LADO_ICONE_DIREITA: f32 = 13.0;

/// A cor da contagem do nó (`.q`): `--t3`; no nó ativo, sobre a seleção, o `--t2` (o terciário cairia abaixo de
/// 4,5:1; revisão de 15/10/2026, item 1).
pub fn cor_contagem_no(p: &Paleta, ativo: bool) -> Color32 {
    if ativo { p.estudio.t2 } else { p.estudio.t3 }
}

/// O recuo de cada nível da árvore (o `padding-left` dos nós): 8 no banco, 20 nos grupos (Tabelas, Visões,
/// Sistema), 34 nos objetos e 48 nas partes da tabela; daí em diante, mais 14 por nível.
pub fn recuo(nivel: usize) -> f32 {
    match nivel {
        0 => 8.0,
        n => 20.0 + 14.0 * (n - 1) as f32,
    }
}

/// A seta de abrir do nó.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Seta {
    /// Sem seta (e sem o espaço dela).
    #[default]
    Nenhuma,
    /// O espaço da seta, vazio (`.vz`: alinha com os irmãos que abrem).
    Vazia,
    /// Fechada (`M9 6l6 6-6 6`).
    Fechada,
    /// Aberta (`M6 9l6 6 6-6`).
    Aberta,
}

/// O que aconteceu com o nó.
#[derive(Debug, Clone)]
pub struct RespostaNo {
    /// O nó inteiro (o clique abre o que ele representa).
    pub resposta: Response,
    /// A seta foi clicada (ou → / ← com o foco): quem chama abre ou fecha.
    pub alternou: bool,
}

/// Um nó da árvore.
#[must_use = "chame .mostrar(ui)"]
pub struct No<'a> {
    texto: &'a str,
    rotulo: Option<&'a str>,
    recuo: f32,
    seta: Seta,
    icone: Option<Icone>,
    contagem: Option<&'a str>,
    icone_direita: Option<Icone>,
    estado: Option<(Icone, &'a str)>,
    dica_estado: Option<&'a str>,
    forte: bool,
    ativo: bool,
    lateral: bool,
    dica: Option<&'a str>,
}

impl<'a> No<'a> {
    pub fn new(texto: &'a str) -> Self {
        No {
            texto,
            rotulo: None,
            recuo: recuo(0),
            seta: Seta::Nenhuma,
            icone: None,
            contagem: None,
            icone_direita: None,
            estado: None,
            dica_estado: None,
            forte: false,
            ativo: false,
            lateral: false,
            dica: None,
        }
    }

    /// O nome para o leitor de tela, quando o texto se repete na árvore ("clientes · Estrutura").
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo = Some(rotulo);
        self
    }

    /// O `padding-left` (ver [`recuo`]).
    pub fn recuo(mut self, recuo: f32) -> Self {
        self.recuo = recuo;
        self
    }

    pub fn seta(mut self, seta: Seta) -> Self {
        self.seta = seta;
        self
    }

    pub fn icone(mut self, icone: Icone) -> Self {
        self.icone = Some(icone);
        self
    }

    /// A contagem à direita (`.q`: "60", "5.000", "—").
    pub fn contagem(mut self, contagem: &'a str) -> Self {
        self.contagem = Some(contagem);
        self
    }

    /// A silhueta de 13px à direita, no lugar da contagem (`.q .sil`, na cor dela): o ↵ das Tabelas da Consulta
    /// SQL, que dizem que o clique insere o nome no editor.
    pub fn icone_a_direita(mut self, icone: Icone) -> Self {
        self.icone_direita = Some(icone);
        self
    }

    /// O ícone de estado de 16px à direita (no lugar da contagem: o Atenção do banco com a transação aberta, que a
    /// revisão de 15/10/2026 pôs no lugar do ponto de cor), com a dica e o rótulo dele (`role="img"`: "Transação
    /// aberta"); com o ponteiro sobre ele, só essa dica aparece, sem a do nó. O banco aberto sem aviso não leva nada.
    pub fn estado(mut self, icone: Icone, dica: &'a str) -> Self {
        self.estado = Some((icone, dica));
        self
    }

    /// Uma dica mais longa para o ícone de estado, no lugar do rótulo dele (o aviso inteiro da transação aberta); o
    /// leitor de tela continua ouvindo o rótulo curto do [`No::estado`].
    pub fn dica_do_estado(mut self, dica: &'a str) -> Self {
        self.dica_estado = Some(dica);
        self
    }

    /// O nome em 600 (o banco aberto).
    pub fn forte(mut self) -> Self {
        self.forte = true;
        self
    }

    /// O nó da aba à vista (`.no.on`).
    pub fn ativo(mut self, sim: bool) -> Self {
        self.ativo = sim;
        self
    }

    /// A dica do nó inteiro (o caminho do banco).
    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }

    /// O nó da navegação das Preferências (`.pref-nav .no`): 32px de altura ([`ALTURA_NO_LATERAL`]) com
    /// `margin:0 8px`, `padding:0 12px 0 9px` e `gap:10px`, o sólido de 20px e, no ativo, o nome em 600.
    pub fn lateral(mut self) -> Self {
        self.lateral = true;
        self.recuo = RECUO_LATERAL;
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> RespostaNo {
        let p = paleta(ui);
        let e = &p.estudio;
        let (altura_no, margem, gap) = if self.lateral {
            (ALTURA_NO_LATERAL, MARGEM_NO_LATERAL, GAP_LATERAL)
        } else {
            (ALTURA_NO, MARGEM_NO, GAP)
        };
        let recuo_direita = if self.lateral { RECUO_DIREITA_LATERAL } else { RECUO_DIREITA };
        let topo = ui.cursor().min;
        let largura = ui.available_width();
        let (linha, _) = ui.allocate_exact_size(vec2(largura, altura_no), Sense::hover());
        let rect =
            Rect::from_min_max(pos2(linha.left() + margem, topo.y), pos2(linha.right() - margem, linha.bottom()));
        let id = ui.id().with(("databox::no", self.rotulo.unwrap_or(self.texto)));
        let resp = ui.interact(rect, id, Sense::click());
        let mut x = rect.left() + self.recuo;

        // a seta: um alvo de clique próprio, por cima do nó
        let mut alternou = false;
        let seta_rect = Rect::from_min_size(pos2(x, rect.center().y - LADO_SETA / 2.0), vec2(LADO_SETA, LADO_SETA));
        let abre = matches!(self.seta, Seta::Fechada | Seta::Aberta);
        if abre {
            let alvo = Rect::from_min_max(pos2(x - 4.0, rect.top()), pos2(x + LADO_SETA + GAP / 2.0, rect.bottom()));
            // só o clique: a seta não é uma parada do Tab (o nó já é, e o → e o ← dele abrem e fecham)
            let resp_seta = ui.interact(alvo, id.with("seta"), Sense::CLICK);
            alternou = resp_seta.clicked();
            if resp_seta.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.has_focus() {
                let (direita, esquerda) = ui.input(|i| (i.key_pressed(Key::ArrowRight), i.key_pressed(Key::ArrowLeft)));
                alternou |= (direita && self.seta == Seta::Fechada) || (esquerda && self.seta == Seta::Aberta);
                // o → e o ← são do nó: o foco fica nele, sem o egui o levar ao vizinho da direita ou da esquerda (a
                // trava vale a partir do segundo quadro com o foco; no primeiro, a navegação já pedida é desfeita)
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(resp.id, EventFilter { horizontal_arrows: true, ..Default::default() });
                    if direita || esquerda {
                        m.move_focus(FocusDirection::None);
                    }
                });
            }
        }
        if self.seta != Seta::Nenhuma {
            x += LADO_SETA + GAP;
        }

        let hover = ui.ctx().animate_bool_with_time(id.with("hover"), resp.contains_pointer(), duracao_transicao(ui));
        // o ponteiro sobre o ícone de estado: só a dica dele aparece (como o `title` de dentro, no HTML)
        let mut sobre_o_estado = false;
        if ui.is_rect_visible(rect.expand(10.0)) {
            let painter = ui.painter();
            if self.ativo {
                // `.no.on`: `--sel`, `inset 0 1px 0 rgba(255,255,255,.07)` e `0 0 10px` do destaque a 22%
                pintura::halo(&pintor_de_fora(ui, rect), rect, RAIO_NO, 10.0, e.brilho_item);
                painter.rect_filled(rect, RAIO_NO, e.sel);
                realce_curvo(painter, rect, RAIO_NO, rgba(255, 255, 255, 0.07));
            } else if hover > 0.0 {
                painter.rect_filled(rect, RAIO_NO, e.h.gamma_multiply(hover));
            }
            let cor = if self.ativo { e.t } else { pintura::misturar(e.t2, e.t, hover) };
            match self.seta {
                Seta::Fechada => icones::pintar(painter, seta_rect, Icone::SetaDireita, e.t3),
                Seta::Aberta => icones::pintar(painter, seta_rect, Icone::SetaBaixo, e.t3),
                Seta::Nenhuma | Seta::Vazia => {}
            }
            if let Some(icone) = self.icone {
                // os sólidos em 16px (`.sk.s16`; 20px na navegação das Preferências), as silhuetas em 14px e em
                // `--t3` (`.no .ic`)
                let lado = match (self.lateral, icone.familia()) {
                    (true, _) => LADO_ICONE_LATERAL,
                    (false, Familia::Silhueta) => 14.0,
                    (false, _) => 16.0,
                };
                let r = Rect::from_min_size(pos2(x, rect.center().y - lado / 2.0), vec2(lado, lado));
                icones::pintar(painter, r, icone, e.t3);
                x += lado + gap;
            }
            // à direita: a contagem, a silhueta ou o ícone de estado
            let cor_q = cor_contagem_no(p, self.ativo);
            let gc = self.contagem.map(|c| galley(ui, c, fontes::fonte(11.0, Peso::Normal), cor_q));
            let mut direita = rect.right() - recuo_direita;
            if let Some(gc) = gc {
                let pos = pos2(direita - gc.size().x, rect.center().y - gc.size().y / 2.0);
                direita -= gc.size().x + GAP;
                painter.galley(pos, gc, cor_q);
            }
            if let Some(icone) = self.icone_direita {
                let caixa = Rect::from_min_size(
                    pos2(direita - LADO_ICONE_DIREITA, rect.center().y - LADO_ICONE_DIREITA / 2.0),
                    vec2(LADO_ICONE_DIREITA, LADO_ICONE_DIREITA),
                );
                icones::pintar(painter, caixa, icone, cor_q);
                direita -= LADO_ICONE_DIREITA + GAP;
            }
            if let Some((icone, dica)) = self.estado {
                let caixa = Rect::from_min_size(
                    pos2(direita - LADO_ESTADO, rect.center().y - LADO_ESTADO / 2.0),
                    vec2(LADO_ESTADO, LADO_ESTADO),
                );
                icones::pintar(painter, caixa, icone, cor_q);
                let alvo = caixa.expand2(vec2(3.0, (ALTURA_NO - LADO_ESTADO) / 2.0));
                // a dica do ícone ("Transação aberta"); só hover: o clique continua no nó
                let resp_estado = ui.interact(alvo, id.with("estado"), Sense::hover());
                resp_estado.widget_info(|| WidgetInfo::labeled(WidgetType::Image, ui.is_enabled(), dica));
                sobre_o_estado = resp_estado.hovered();
                resp_estado.on_hover_text(self.dica_estado.unwrap_or(dica));
                direita -= LADO_ESTADO + GAP;
            }
            let peso = if self.forte || (self.lateral && self.ativo) { Peso::Medio } else { Peso::Normal };
            let g = galley_cortado(ui, self.texto, fontes::fonte(12.0, peso), cor, (direita - x).max(0.0));
            painter.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, cor);
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&pintor_de_fora(ui, rect), p, rect, RAIO_NO);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let rotulo = self.rotulo.unwrap_or(self.texto);
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Link, ui.is_enabled(), self.ativo, rotulo));
        if abre {
            ui.ctx().accesskit_node_builder(resp.id, |no| no.set_expanded(self.seta == Seta::Aberta));
        }
        let resposta = match self.dica {
            Some(d) if !sobre_o_estado => resp.on_hover_text(d),
            _ => resp,
        };
        RespostaNo { resposta, alternou }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recuos_das_pranchetas() {
        assert_eq!([recuo(0), recuo(1), recuo(2), recuo(3), recuo(4)], [8.0, 20.0, 34.0, 48.0, 62.0]);
    }
}
