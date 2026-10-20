//! As abas de documento da casca (o `div[role=tablist]` e as `.aba` de `Main.dc.html` e `Dados.dc.html`): a faixa
//! de 35px ([`ALTURA_FAIXA`]) com as abas de 34px encostadas embaixo, cada uma com o ícone de 16px, o nome, o sub em
//! `--t3` e o × redondo de 20px; depois da última, o `.ib.pq` "+" (Nova consulta) e, na ponta direita, o "Todas as
//! abas" ([`FaixaAbas::todas`], a seta que abre o menu com todas as abas abertas).
//!
//! - A primeira aba começa na quina do documento (`padding-left:0`, revisão de UI de 19/10/2026): com ela ativa, o
//!   canto de cima à esquerda do documento fica reto ([`pintar_documento`]).
//!
//! - A aba ativa sobe na superfície com textura (o degradê `--bt1` → meio, as riscas e o realce) e forma uma
//!   superfície só com o documento embaixo, sem barrinha colorida: [`RespostaAbas::juncao`] diz onde ela está, e
//!   [`pintar_documento`] pinta o `.doc` sem o fio de cima naquele trecho.
//! - As inativas são transparentes em `--t2`; no hover, o fundo `--h` e o texto `--t`. O × fica em `--t3` (no
//!   hover, o círculo `--h` e o `--t`). O clique do meio também fecha.
//! - A aba com alteração não salva ([`AbaDocumento::alterada`]) mostra o ponto de [`LADO_PONTO`] em `--acct` (`.aba
//!   .mod`) no lugar do ×; o × volta com o ponteiro na aba ou com o foco nela, na mesma posição (a largura da aba não
//!   muda entre um e outro, ao contrário do CSS, que troca 21px por 24px).
//! - Sem espaço, os textos encolhem por igual com "…" (as abas curtas ficam inteiras) até [`TEXTO_MINIMO`]; daí
//!   em diante a faixa rola com a roda, e a aba que passa a ser a ativa entra à vista.
//! - Acessibilidade: cada aba tem o papel `tab`, `selected` e o rótulo "nome · sub" ([`AbaDocumento::rotulo`]); o ×
//!   é o botão "Fechar a aba nome · sub" (rótulo único), com a dica "Fechar aba (Ctrl+W)".
//!
//! ```ignore
//! let abas = [AbaDocumento::new(Icone::Banco, "exemplo.db").sub("Visão geral"), AbaDocumento::new(Icone::Tabela, "clientes").sub("Dados")];
//! let r = FaixaAbas::new("abas", &abas).ativa(Some(1)).nova(true).mostrar(ui);
//! pintar_documento(ui.painter(), paleta(ui), rect_do_documento, r.juncao);
//! ```

use egui::accesskit::Role;
use egui::epaint::RectShape;
use egui::{
    Align, Color32, CursorIcon, Id, Layout, Painter, PointerButton, Rangef, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::botao::{Botao, LADO_PEQUENO, pintor_de_fora};
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::menu_contexto::MenuContexto;
use crate::pecas::navegacao::{
    ALTURA_ABA, ALTURA_ABAS, ESPACO_ABAS, RAIO_ABA, RECUO_ABAS, cantos_aba, pintar_aba_ativa,
};
use crate::pecas::texto::{galley, galley_cortado};
use crate::pintura;
use crate::tema::{Paleta, duracao_transicao, paleta};

/// Altura da faixa das abas (as abas de 34px encostadas embaixo; o documento começa 1px antes do fim dela).
pub const ALTURA_FAIXA: f32 = ALTURA_ABAS;
/// O texto mínimo de uma aba encolhida (o resto vira "…").
pub const TEXTO_MINIMO: f32 = 30.0;
/// Lado do ícone da aba (`.sk.s16`).
const LADO_ICONE: f32 = 16.0;
/// Lado do × (`.aba .x`).
const LADO_X: f32 = 20.0;
/// Lado do ponto da aba alterada (`.aba .mod{width:7px;height:7px}`), no centro do lugar do ×: a largura da aba não
/// muda entre o ponto e o ×.
pub const LADO_PONTO: f32 = 7.0;
/// `gap:7px` entre o ícone, o nome e o sub.
const GAP: f32 = 7.0;
/// A borda de 1px, o `padding-left:12px`, o ícone e o `gap` até o nome.
const ANTES_DO_TEXTO: f32 = 1.0 + 12.0 + LADO_ICONE + GAP;
/// O × (`margin-left:4px`, 20px), o `padding-right:6px` e a borda.
const DEPOIS_COM_X: f32 = 4.0 + LADO_X + 6.0 + 1.0;
/// Sem o × (o Início sozinho): o `padding-right:8px` do `a`, o da aba e a borda.
const DEPOIS_SEM_X: f32 = 8.0 + 6.0 + 1.0;
/// O "+" (`.ib.pq` com `margin-left:4px`) e o `gap:2px` antes dele.
const LARGURA_NOVA: f32 = ESPACO_ABAS + 4.0 + LADO_PEQUENO;
/// O sub só aparece com pelo menos isto de espaço.
const SUB_MINIMO: f32 = 16.0;

/// A cor do sub da aba ("Visão geral", "Dados"): `--t3`; na aba ativa, sobre o relevo, o `--t2` (o terciário cairia
/// abaixo de 4,5:1; revisão de 15/10/2026, item 1).
pub fn cor_sub_aba(p: &Paleta, ativa: bool) -> egui::Color32 {
    if ativa { p.estudio.t2 } else { p.estudio.t3 }
}

/// Uma aba: o ícone, o nome, o sub (em `--t3`; na ativa, [`cor_sub_aba`]), se tem o × e a chave dela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbaDocumento<'a> {
    pub icone: Icone,
    pub nome: &'a str,
    pub sub: Option<&'a str>,
    pub fechavel: bool,
    /// O que identifica a aba entre os quadros (o id da aba na casca); sem ela, a posição na faixa. Com a chave,
    /// o foco do teclado e o hover ficam na aba certa quando outra antes dela fecha.
    pub chave: Option<u64>,
    /// Tem alteração não salva: o ponto de 7px em `--acct` (`.aba .mod`) no lugar do ×, que volta no hover e no foco.
    pub alterada: bool,
}

impl<'a> AbaDocumento<'a> {
    /// Uma aba fechável, sem sub ("Consulta 2").
    pub fn new(icone: Icone, nome: &'a str) -> Self {
        AbaDocumento { icone, nome, sub: None, fechavel: true, chave: None, alterada: false }
    }

    /// A aba tem alteração não salva (o ponto no lugar do ×; o leitor de tela ouve ", alterada").
    pub fn alterada(mut self, sim: bool) -> Self {
        self.alterada = sim;
        self
    }

    /// A chave estável da aba (ver [`AbaDocumento::chave`]).
    pub fn com_chave(mut self, chave: u64) -> Self {
        self.chave = Some(chave);
        self
    }

    /// O sub depois do nome ("Dados", "Visão geral").
    #[allow(clippy::should_implement_trait)] // o nome é o do `.aba .sub` das pranchetas
    pub fn sub(mut self, sub: &'a str) -> Self {
        self.sub = Some(sub);
        self
    }

    /// Com ou sem o × (o Início sozinho não fecha).
    pub fn fechavel(mut self, sim: bool) -> Self {
        self.fechavel = sim;
        self
    }

    /// O nome para o leitor de tela: "clientes · Dados", ou só o nome sem o sub; a aba com alteração não salva ganha
    /// ", alterada" ("vendas_2025.csv, alterada").
    pub fn rotulo(&self) -> String {
        let base = match self.sub {
            Some(s) => format!("{} · {s}", self.nome),
            None => self.nome.to_owned(),
        };
        if self.alterada { format!("{base}, alterada") } else { base }
    }
}

/// O que aconteceu na faixa neste quadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RespostaAbas {
    /// A aba clicada (com o botão principal ou o teclado): quem chama a foca.
    pub clicada: Option<usize>,
    /// A aba a fechar (o × ou o clique do meio).
    pub fechar: Option<usize>,
    /// O "+" foi clicado.
    pub nova: bool,
    /// Uma aba foi escolhida no menu "Todas as abas" (o índice; quem chama a foca, como [`RespostaAbas::clicada`]).
    pub escolhida: Option<usize>,
    /// A faixa inteira.
    pub rect: Rect,
    /// O trecho da aba ativa (à vista) sobre o documento, para o [`pintar_documento`].
    pub juncao: Option<Rangef>,
}

/// A faixa das abas. Ocupa a largura disponível e [`ALTURA_FAIXA`] de altura.
#[must_use = "chame .mostrar(ui)"]
pub struct FaixaAbas<'a> {
    id: Id,
    abas: &'a [AbaDocumento<'a>],
    ativa: Option<usize>,
    nova: Option<bool>,
    todas: bool,
}

impl<'a> FaixaAbas<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, abas: &'a [AbaDocumento<'a>]) -> Self {
        FaixaAbas { id: Id::new(("databox::abas_documento", id_salt)), abas, ativa: None, nova: None, todas: false }
    }

    /// A aba ativa.
    pub fn ativa(mut self, ativa: Option<usize>) -> Self {
        self.ativa = ativa;
        self
    }

    /// Mostra o "+" no fim (ligado ou desligado; sem chamar, não há "+").
    pub fn nova(mut self, habilitada: bool) -> Self {
        self.nova = Some(habilitada);
        self
    }

    /// Com o "Todas as abas" na ponta direita (`.ib.pq` com a seta de 14px, `margin:0 0 4px 0`): o menu com cada aba
    /// aberta ("nome · sub"), a ativa marcada.
    pub fn todas(mut self, sim: bool) -> Self {
        self.todas = sim;
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> RespostaAbas {
        let p = paleta(ui);
        let e = &p.estudio;
        let (faixa, faixa_resp) = ui.allocate_exact_size(vec2(ui.available_width(), ALTURA_FAIXA), Sense::hover());
        let mut resposta =
            RespostaAbas { clicada: None, fechar: None, nova: false, escolhida: None, rect: faixa, juncao: None };
        let fonte = fontes::fonte(12.0, Peso::Normal);
        let medir = |texto: &str| galley(ui, texto, fonte.clone(), Color32::PLACEHOLDER).size().x;
        let medidas: Vec<(f32, f32)> = self.abas.iter().map(|a| (medir(a.nome), a.sub.map_or(0.0, medir))).collect();
        let naturais: Vec<f32> = self
            .abas
            .iter()
            .zip(&medidas)
            .map(|(a, &(nome, sub))| if a.sub.is_some() { nome + GAP + sub } else { nome })
            .collect();
        let fixas: Vec<f32> =
            self.abas.iter().map(|a| ANTES_DO_TEXTO + if a.fechavel { DEPOIS_COM_X } else { DEPOIS_SEM_X }).collect();
        let reserva_nova = if self.nova.is_some() { LARGURA_NOVA } else { 0.0 };
        let reserva_todas = if self.todas { ESPACO_ABAS + LADO_PEQUENO } else { 0.0 };
        // a primeira aba na quina do documento (`padding-left:0`); à direita, o `padding-right:10px`
        let area = Rect::from_min_max(
            faixa.left_top(),
            pos2((faixa.right() - RECUO_ABAS - reserva_nova - reserva_todas).max(faixa.left()), faixa.bottom()),
        );
        let textos = larguras_texto(&naturais, &fixas, ESPACO_ABAS, area.width(), TEXTO_MINIMO);
        let larguras: Vec<f32> = fixas.iter().zip(&textos).map(|(f, t)| (f + t).ceil()).collect();
        let inicios: Vec<f32> = larguras
            .iter()
            .scan(0.0, |x, l| {
                let inicio = *x;
                *x += l + ESPACO_ABAS;
                Some(inicio)
            })
            .collect();
        let total = larguras.iter().sum::<f32>() + ESPACO_ABAS * larguras.len().saturating_sub(1) as f32;

        // a rolagem (só quando nem encolhidas as abas cabem)
        let id_rolagem = self.id.with("rolagem");
        let antes: (f32, Option<usize>) = ui.ctx().data(|d| d.get_temp(id_rolagem)).unwrap_or((0.0, None));
        let mut deslocamento = antes.0;
        let excesso = (total - area.width()).max(0.0);
        if excesso > 0.0 && faixa_resp.contains_pointer() {
            let delta = ui.input(|i| i.smooth_scroll_delta);
            deslocamento -= delta.x + delta.y;
        }
        if self.ativa != antes.1
            && let Some(i) = self.ativa.filter(|&i| i < larguras.len())
        {
            // a aba que passou a ser a ativa entra à vista
            let (x0, x1) = (inicios[i], inicios[i] + larguras[i]);
            if x0 < deslocamento {
                deslocamento = x0;
            } else if x1 > deslocamento + area.width() {
                deslocamento = x1 - area.width();
            }
        }
        deslocamento = deslocamento.clamp(0.0, excesso);
        if (deslocamento, self.ativa) != antes {
            ui.ctx().data_mut(|d| d.insert_temp(id_rolagem, (deslocamento, self.ativa)));
        }

        let pintor = ui.painter().with_clip_rect(area);
        let tempo = duracao_transicao(ui);
        for (i, aba) in self.abas.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(area.left() - deslocamento + inicios[i], faixa.bottom() - ALTURA_ABA),
                vec2(larguras[i], ALTURA_ABA),
            );
            let visivel = r.intersect(area);
            if !visivel.is_positive() {
                continue;
            }
            let e_ativa = Some(i) == self.ativa;
            let rotulo = aba.rotulo();
            let id_aba = match aba.chave {
                Some(c) => self.id.with(("chave", c)),
                None => self.id.with(("posicao", i)),
            };
            let resp = ui.interact(visivel, id_aba, Sense::click());
            let rect_x =
                Rect::from_center_size(pos2(r.right() - 1.0 - 6.0 - LADO_X / 2.0, r.center().y), vec2(LADO_X, LADO_X));
            // o × depois da aba: fica por cima dela
            let resp_x = (aba.fechavel && rect_x.intersect(area).is_positive())
                .then(|| ui.interact(rect_x.intersect(area), id_aba.with("fechar"), Sense::click()));
            if resp.clicked() {
                resposta.clicada = Some(i);
            }
            if aba.fechavel && resp.clicked_by(PointerButton::Middle) {
                resposta.fechar = Some(i);
            }
            if resp_x.as_ref().is_some_and(|x| x.clicked()) {
                resposta.fechar = Some(i);
            }

            // o hover vale também com o ponteiro no × (o `:hover` da aba inteira)
            let hover =
                ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.contains_pointer() && !e_ativa, tempo);
            if e_ativa {
                pintar_aba_ativa(&pintor, p, r);
                resposta.juncao = Some(Rangef::new(visivel.left(), visivel.right()));
            } else if hover > 0.0 {
                pintor.add(RectShape::filled(r, cantos_aba(), e.h.gamma_multiply(hover)));
            }
            let cor = if e_ativa { e.t } else { pintura::misturar(e.t2, e.t, hover) };
            let x_icone = r.left() + 1.0 + 12.0;
            let icone =
                Rect::from_min_size(pos2(x_icone, r.center().y - LADO_ICONE / 2.0), vec2(LADO_ICONE, LADO_ICONE));
            icones::pintar(&pintor, icone, aba.icone, cor);
            // o nome tem a preferência; o sub fica com o que sobra
            let espaco = textos[i];
            let (nome, sub) = medidas[i];
            let x_texto = x_icone + LADO_ICONE + GAP;
            let g_nome = if nome <= espaco + 0.5 {
                galley(ui, aba.nome, fonte.clone(), cor)
            } else {
                galley_cortado(ui, aba.nome, fonte.clone(), cor, espaco)
            };
            let largura_nome = g_nome.size().x;
            pintor.galley(pos2(x_texto, r.center().y - g_nome.size().y / 2.0), g_nome, cor);
            if let Some(s) = aba.sub {
                let resta = espaco - largura_nome - GAP;
                if resta >= SUB_MINIMO.min(sub) {
                    let cor_sub = cor_sub_aba(p, e_ativa);
                    let g_sub = if sub <= resta + 0.5 {
                        galley(ui, s, fonte.clone(), cor_sub)
                    } else {
                        galley_cortado(ui, s, fonte.clone(), cor_sub, resta)
                    };
                    let pos = pos2(x_texto + largura_nome + GAP, r.center().y - g_sub.size().y / 2.0);
                    pintor.galley(pos, g_sub, cor_sub);
                }
            }
            if let Some(rx) = resp_x {
                // `.aba .mod+.x{display:none}` e `.aba:hover .mod, .aba.foco .mod{display:none}`: a aba alterada mostra o
                // ponto no lugar do ×, e o × volta com o ponteiro na aba ou com o foco nela (ou no ×)
                let ponto = aba.alterada && !(resp.contains_pointer() || resp.has_focus() || rx.has_focus());
                let hover_x = ui.ctx().animate_bool_with_time(rx.id.with("hover"), rx.hovered(), tempo);
                if ponto {
                    pintor.circle_filled(rect_x.center(), LADO_PONTO / 2.0, e.acct);
                } else {
                    if hover_x > 0.0 {
                        pintor.circle_filled(rect_x.center(), LADO_X / 2.0, e.h.gamma_multiply(hover_x));
                    }
                    let cor_x = pintura::misturar(e.t3, e.t, hover_x);
                    icones::pintar(
                        &pintor,
                        Rect::from_center_size(rect_x.center(), vec2(12.0, 12.0)),
                        Icone::Fechar,
                        cor_x,
                    );
                }
                if rx.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if rx.has_focus() && pintura::foco_visivel(ui.ctx()) {
                    pintura::anel_foco(&pintor_de_fora(ui, rect_x), p, rect_x, LADO_X / 2.0);
                }
                let rotulo_x = format!("Fechar a aba {rotulo}");
                rx.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &rotulo_x));
                rx.on_hover_text("Fechar aba (Ctrl+W)");
            }
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&pintor_de_fora(ui, r), p, r, RAIO_ABA);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), e_ativa, &rotulo));
            ui.ctx().accesskit_node_builder(resp.id, |no| {
                no.set_role(Role::Tab);
                no.set_selected(e_ativa);
            });
        }

        if let Some(habilitada) = self.nova {
            // `margin:0 0 4px 4px`: logo depois da última aba, ou no fim da faixa quando ela rola
            let x = (area.left() - deslocamento + total).min(area.right()) + ESPACO_ABAS + 4.0;
            let rect =
                Rect::from_min_size(pos2(x, faixa.bottom() - 4.0 - LADO_PEQUENO), vec2(LADO_PEQUENO, LADO_PEQUENO));
            let mut filho = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("nova"))
                    .max_rect(rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let botao = Botao::icone(Icone::Mais).pequeno().rotulo("Nova consulta SQL").dica("Nova consulta SQL");
            resposta.nova = filho.add(botao.habilitado(habilitada)).clicked();
        }
        if self.todas {
            let x = faixa.right() - RECUO_ABAS - LADO_PEQUENO;
            let rect =
                Rect::from_min_size(pos2(x, faixa.bottom() - 4.0 - LADO_PEQUENO), vec2(LADO_PEQUENO, LADO_PEQUENO));
            let mut filho = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("todas"))
                    .max_rect(rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let botao =
                filho.add(Botao::icone(Icone::SetaBaixo).pequeno().rotulo("Todas as abas").dica("Todas as abas"));
            let rotulos: Vec<String> = self.abas.iter().map(AbaDocumento::rotulo).collect();
            let escolhida = MenuContexto::new(self.id.with("menu-todas")).rotulo("Todas as abas").no_clique().mostrar(
                &botao,
                |m| {
                    let mut escolhida = None;
                    m.com_marcas();
                    for (i, rotulo) in rotulos.iter().enumerate() {
                        if m.item_opcao(rotulo, Some(i) == self.ativa).clicked() {
                            escolhida = Some(i);
                        }
                    }
                    escolhida
                },
            );
            resposta.escolhida = escolhida.flatten();
        }
        resposta
    }
}

/// A largura do texto de cada aba para caberem em `disponivel`: as naturais, se couberem; senão, o texto de cada
/// uma encolhe até um teto comum (as curtas ficam inteiras), nunca abaixo de `minimo` (aí a faixa rola).
pub(crate) fn larguras_texto(naturais: &[f32], fixas: &[f32], espaco: f32, disponivel: f32, minimo: f32) -> Vec<f32> {
    let n = naturais.len();
    if n == 0 {
        return Vec::new();
    }
    let resto = disponivel - fixas.iter().sum::<f32>() - espaco * (n - 1) as f32;
    if naturais.iter().sum::<f32>() <= resto {
        return naturais.to_vec();
    }
    let mut ordenadas = naturais.to_vec();
    ordenadas.sort_by(f32::total_cmp);
    let mut sobra = resto;
    let mut teto = f32::INFINITY;
    for (i, &w) in ordenadas.iter().enumerate() {
        let restantes = (n - i) as f32;
        if w * restantes <= sobra {
            sobra -= w;
        } else {
            teto = sobra / restantes;
            break;
        }
    }
    let teto = teto.max(minimo);
    naturais.iter().map(|&w| w.min(teto)).collect()
}

/// Pinta o documento (`.doc`: fundo `--c`, borda `--vidrob`, raio [`RAIO_CARD`]) embaixo da faixa das abas. Na
/// `juncao` (a aba ativa, [`RespostaAbas::juncao`]) o fio de cima some e a aba continua no documento, como o
/// `margin-bottom:-1px` da faixa das pranchetas: o retângulo do documento começa 1px antes do fim da faixa.
pub fn pintar_documento(painter: &Painter, p: &Paleta, rect: Rect, juncao: Option<Rangef>) {
    let e = &p.estudio;
    // a aba ativa encostada na quina: o canto de cima à esquerda fica reto (`border-top-left-radius:0`)
    let mut cantos = pintura::canto(rect, RAIO_CARD);
    if juncao.is_some_and(|j| j.min <= rect.left() + 0.5) {
        cantos.nw = 0;
    }
    painter.rect_filled(rect, cantos, e.doc);
    painter.rect_stroke(rect, cantos, Stroke::new(1.0, e.vidrob), StrokeKind::Inside);
    if let Some(j) = juncao {
        // por dentro das bordas laterais da aba, na cor do pé dela (o meio do degradê da aba ativa)
        let x = Rangef::new(j.min + 1.0, j.max - 1.0);
        if x.span() > 0.0 {
            painter.rect_filled(Rect::from_x_y_ranges(x, Rangef::new(rect.top(), rect.top() + 1.0)), 0.0, e.sup_meio);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn abas_que_cabem_ficam_inteiras() {
        assert_eq!(larguras_texto(&[50.0, 80.0], &[60.0, 60.0], 2.0, 1000.0, 30.0), [50.0, 80.0]);
        assert!(larguras_texto(&[], &[], 2.0, 100.0, 30.0).is_empty());
    }

    #[test]
    fn sem_espaco_as_longas_encolhem_ate_o_mesmo_teto() {
        // 300 de texto disponíveis (500 − 3 × 60 − 2 × 10): a curta fica, as duas longas dividem o resto
        let t = larguras_texto(&[40.0, 200.0, 300.0], &[60.0; 3], 10.0, 500.0, 30.0);
        assert_eq!(t[0], 40.0);
        assert!((t[1] - 130.0).abs() < 1e-3 && (t[2] - 130.0).abs() < 1e-3, "{t:?}");
        assert!((t.iter().sum::<f32>() - 300.0).abs() < 1e-3);
        // apertado demais: o mínimo vale e a faixa rola
        assert_eq!(larguras_texto(&[200.0, 300.0], &[60.0; 2], 2.0, 150.0, 30.0), [30.0, 30.0]);
    }

    #[test]
    fn rotulo_da_aba() {
        assert_eq!(AbaDocumento::new(Icone::Tabela, "clientes").sub("Dados").rotulo(), "clientes · Dados");
        assert_eq!(AbaDocumento::new(Icone::Consulta, "Consulta 2").rotulo(), "Consulta 2");
        assert!(!AbaDocumento::new(Icone::Inicio, "Início").fechavel(false).fechavel);
        assert_eq!(AbaDocumento::new(Icone::Consulta, "Consulta 2").com_chave(7).chave, Some(7));
    }
}
