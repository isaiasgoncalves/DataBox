//! Navegação do visual Estúdio (pranchetas em `design/canvas`): o seletor segmentado ([`trilho`] e
//! [`segmentado`], o `.seg`: trilho rebaixado em pílula com as partes de larguras iguais e o item escolhido em
//! volume, `aria-pressed`: formato da exportação, seletor de tema; o [`seletor`] de resultados da Consulta SQL, com
//! a nota em cada parte e a rolagem horizontal), as abas de documento ([`abas`], `.aba` /
//! `.aba.on`: a ativa sobe na superfície com textura e se junta ao que vem embaixo), a [`trilha`] (`.migalha`:
//! "Banco › Tabelas") e o [`Cabecalho`] da tela (trilha, título com o "i", `sub` só com dados e as ações à direita,
//! que descem de linha juntas).

use std::f32::consts::{FRAC_PI_2, PI};

use egui::epaint::{PathShape, PathStroke, RectShape};
use egui::{
    Align, Color32, CornerRadius, CursorIcon, Id, Layout, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::ajuda::ajuda;
use crate::pecas::botao::{EstadoVolume, Volume, pintar_volume, pintor_de_fora, realce_curvo};
use crate::pecas::texto::{galley, galley_cortado, galley_quebrado};
use crate::pintura::{self, Preenchimento};
use crate::tema::{Paleta, duracao_transicao, paleta};

/// Tamanho dos botões do seletor segmentado. Em todos, as partes têm a mesma largura (a da maior); nos cheios, o
/// trilho vai de ponta a ponta da linha, dividida em partes iguais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TamanhoTrilho {
    /// `.seg button{height:26px;padding:0 14px}`.
    #[default]
    Normal,
    /// O normal ocupando a linha inteira (o `.seg` de um formulário, `.blk .seg{display:grid}`): o Formato da
    /// janela Exportar.
    Cheio,
    /// Os 24px do `.seg` da faixa de resultados da Consulta (`height:24px;padding:0 12px`): o seletor de tema.
    Compacto,
    /// Compacto ocupando a largura toda (o seletor de tema no painel do menu Arquivo, `flex:1`).
    CompactoCheio,
    /// O `.pref-l .seg` das Preferências: o cheio com `padding:0 10px` nos botões (os três rótulos do tema não cabem
    /// com 14 na coluna de 372px do controle).
    CheioJusto,
}

impl TamanhoTrilho {
    /// (altura do botão, `padding` lateral).
    fn medidas(self) -> (f32, f32) {
        match self {
            TamanhoTrilho::Normal | TamanhoTrilho::Cheio => (26.0, 14.0),
            TamanhoTrilho::CheioJusto => (26.0, 10.0),
            TamanhoTrilho::Compacto | TamanhoTrilho::CompactoCheio => (24.0, 12.0),
        }
    }

    /// O trilho ocupa a linha inteira?
    fn cheio(self) -> bool {
        matches!(self, TamanhoTrilho::Cheio | TamanhoTrilho::CompactoCheio | TamanhoTrilho::CheioJusto)
    }
}

/// A borda de 1px e o `padding:2px` do `.seg`: onde os botões começam.
const RECUO_SEG: f32 = 3.0;

/// A largura de cada uma das `n` partes do trilho (`grid-auto-columns:1fr`): a da `maior`; no trilho cheio, a da
/// linha `disponivel` dividida em partes iguais, quando a maior cabe nela.
fn largura_das_partes(maior: f32, n: usize, disponivel: Option<f32>) -> f32 {
    match disponivel {
        // o trilho cheio nunca passa da linha: sem espaço, as partes dividem o que há e o texto corta (com "…")
        Some(d) if n > 0 => ((d - 2.0 * RECUO_SEG) / n as f32).max(MINIMO_DA_PARTE),
        _ => maior,
    }
}

/// A menor parte de um trilho cheio apertado (abaixo disto, ele passa da linha).
const MINIMO_DA_PARTE: f32 = 28.0;
/// O `padding` lateral que sobra numa parte apertada, com o texto cortado.
const PADDING_APERTADO: f32 = 4.0;

/// O `.seg button.on`: o volume neutro sem a sombra interna de baixo (`inset 0 1px 0 --btluz, --btsomb`), sem
/// halo e sem brilho (o `:hover` vem antes no CSS e perde para o `.on`).
fn volume_escolhido(p: &Paleta) -> Volume {
    let mut v = Volume::neutro(p).sem_sombra_baixo().halo(0.0, Color32::TRANSPARENT, false);
    v.clarear_hover = 1.0;
    v
}

/// O seletor segmentado (`.seg`): trilho em pílula com fundo `--f`, borda `--fb`, a sombra interna em cima
/// (`inset 0 1px 2px --b2`) e `padding:2px`; as partes têm todas a largura da maior (o `grid-auto-columns:1fr`) e,
/// nos tamanhos cheios ([`TamanhoTrilho::Cheio`], [`TamanhoTrilho::CompactoCheio`]), dividem a linha inteira; os
/// botões são transparentes em `--t2` (no hover, `--t` e o halo de 14px em `--brilho`) e o escolhido sobe com o
/// volume neutro e o texto em 600. Devolve o índice clicado (se houve clique) e a resposta do trilho inteiro.
/// `pressionado` diz ao leitor de tela que os itens são botões de alternar (`aria-pressed`); sem ele, são itens de
/// escolha.
pub fn trilho(
    ui: &mut Ui,
    id: Id,
    rotulos: &[&str],
    ativo: Option<usize>,
    tamanho: TamanhoTrilho,
    pressionado: bool,
) -> (Option<usize>, Response) {
    let partes: Vec<ParteSeletor<'_>> = rotulos.iter().map(|t| ParteSeletor::new(t)).collect();
    trilho_com_partes(ui, id, &partes, ativo, tamanho, pressionado)
}

/// O [`trilho`] com partes montadas por quem chama (o [`ParteSeletor::icone`] do Formato da janela Exportar, o
/// `.seg button` com o arquivo de 16px e `gap:6px` até o rótulo).
pub fn trilho_com_partes(
    ui: &mut Ui,
    id: Id,
    partes: &[ParteSeletor<'_>],
    ativo: Option<usize>,
    tamanho: TamanhoTrilho,
    pressionado: bool,
) -> (Option<usize>, Response) {
    let tipo = if pressionado { WidgetType::Button } else { WidgetType::SelectableLabel };
    desenhar_trilho(ui, id, partes, ativo, tamanho, tipo, false)
}

/// Uma parte do [`seletor`]: o texto (600 no escolhido), a nota depois dele (`.seg .n`: 400 em `--t3`, a 6px), o
/// texto em `--perigo` (`.seg .erro-t`: o comando que deu erro) e o ícone antes do texto (`.seg button
/// {display:inline-flex;gap:6px}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParteSeletor<'a> {
    pub texto: &'a str,
    pub nota: Option<&'a str>,
    pub perigo: bool,
    pub icone: Option<Icone>,
    /// O ícone que fica no lugar do texto quando ele não cabe ([`ParteSeletor::reserva`]).
    pub reserva: Option<Icone>,
}

/// O lado do ícone da parte (`.sk.s16`).
const LADO_ICONE_PARTE: f32 = 16.0;

impl<'a> ParteSeletor<'a> {
    pub fn new(texto: &'a str) -> Self {
        ParteSeletor { texto, nota: None, perigo: false, icone: None, reserva: None }
    }

    /// O ícone de 16px antes do texto, com 6px até ele (os arquivos XLSX, CSV e JSON do Formato).
    pub fn icone(mut self, icone: Icone) -> Self {
        self.icone = Some(icone);
        self
    }

    /// O ícone de 16px que fica sozinho no lugar do texto quando o trilho cheio não tem espaço para ele (as vistas
    /// das Propriedades numa doca estreita, revisão de 20/10/2026). Só vale quando todas as partes têm o seu: o nome
    /// vai para a dica, e o leitor de tela ouve o mesmo de sempre.
    pub fn reserva(mut self, icone: Icone) -> Self {
        self.reserva = Some(icone);
        self
    }

    pub fn nota(mut self, nota: &'a str) -> Self {
        self.nota = Some(nota);
        self
    }

    pub fn perigo(mut self, sim: bool) -> Self {
        self.perigo = sim;
        self
    }

    /// O rótulo para o leitor de tela: "Comando 1 · 8 linhas".
    pub fn rotulo(&self) -> String {
        match self.nota {
            Some(n) => format!("{} · {}", self.texto, n),
            None => self.texto.to_string(),
        }
    }
}

/// O `gap:6px` entre o texto e a nota da parte.
const GAP_NOTA: f32 = 6.0;

/// O seletor de resultados (o `.seg role=tablist` da faixa de resultados da Consulta SQL, prancheta 3): o trilho
/// compacto (24px, `padding:0 12px`) com as partes iguais, cada uma com a nota ("Comando 1 · 8 linhas") e o texto em
/// `--perigo` no comando com erro. Uma parte escolhida sempre; quando não cabe na largura, rola na horizontal (a
/// barra fina do egui, por cima) e a rolagem acompanha a escolhida quando ela muda. Leitor de tela: `tablist` com
/// `tab`s. Devolve o índice clicado.
pub fn seletor(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    rotulo: &str,
    partes: &[ParteSeletor<'_>],
    ativo: usize,
) -> Option<usize> {
    let id = Id::new(("databox::seletor", id_salt));
    let id_ativo = id.with("ativo");
    let mudou = ui.ctx().data(|d| d.get_temp::<usize>(id_ativo)) != Some(ativo);
    let altura = TamanhoTrilho::Compacto.medidas().0 + 2.0 * RECUO_SEG;
    let saida = egui::ScrollArea::horizontal()
        .id_salt(id.with("rolagem"))
        .auto_shrink([false, true])
        .max_height(altura)
        .show(ui, |ui| {
            let (clicado, resp) = desenhar_trilho(
                ui,
                id,
                partes,
                Some(ativo),
                TamanhoTrilho::Compacto,
                WidgetType::SelectableLabel,
                true,
            );
            if mudou && partes.len() > 1 {
                let largura = (resp.rect.width() - 2.0 * RECUO_SEG) / partes.len() as f32;
                let x = resp.rect.left() + RECUO_SEG + largura * ativo as f32;
                let r = Rect::from_min_size(pos2(x, resp.rect.top()), vec2(largura, resp.rect.height()));
                ui.scroll_to_rect(r, None);
            }
            ui.ctx().accesskit_node_builder(resp.id, |no| {
                no.set_role(egui::accesskit::Role::TabList);
                no.set_label(rotulo);
            });
            clicado
        });
    if mudou {
        ui.ctx().data_mut(|d| d.insert_temp(id_ativo, ativo));
    }
    saida.inner
}

/// O trilho com as partes (o [`trilho`] e o [`seletor`]). `tipo` é o papel de cada item para o leitor de tela;
/// com `abas`, cada item é um `tab` (o [`seletor`]).
fn desenhar_trilho(
    ui: &mut Ui,
    id: Id,
    partes: &[ParteSeletor<'_>],
    ativo: Option<usize>,
    tamanho: TamanhoTrilho,
    tipo: WidgetType,
    abas: bool,
) -> (Option<usize>, Response) {
    let p = paleta(ui);
    let e = &p.estudio;
    let (altura_item, pad_x) = tamanho.medidas();
    let peso_de = |i: usize| if Some(i) == ativo { Peso::Medio } else { Peso::Normal };
    let fonte_nota = fontes::fonte(12.0, Peso::Normal);
    // o ícone e a nota, o que vem além do texto (cada um com o `gap:6px`)
    let largura_nota = |ui: &Ui, parte: &ParteSeletor<'_>| {
        let icone = if parte.icone.is_some() { LADO_ICONE_PARTE + GAP_NOTA } else { 0.0 };
        icone + parte.nota.map_or(0.0, |n| GAP_NOTA + galley(ui, n, fonte_nota.clone(), Color32::PLACEHOLDER).size().x)
    };
    // a maior parte: o texto medido em 600 (o peso do escolhido, para o trilho não mudar de largura quando a
    // escolha muda), a nota, o `padding` e a borda de 1px
    let maior = partes
        .iter()
        .map(|parte| {
            let g = galley(ui, parte.texto, fontes::fonte(12.0, Peso::Medio), Color32::PLACEHOLDER);
            (g.size().x + largura_nota(ui, parte) + 2.0 * pad_x + 2.0).ceil()
        })
        .fold(0.0_f32, f32::max);
    let disponivel = Some(ui.available_width()).filter(|d| tamanho.cheio() && d.is_finite());
    let largura = largura_das_partes(maior, partes.len(), disponivel);
    let total = largura * partes.len() as f32 + 2.0 * RECUO_SEG;
    // sem espaço para o texto de alguma parte (o mesmo corte de baixo), todas mostram só o ícone de reserva
    let so_icones = !partes.is_empty()
        && partes.iter().all(|parte| parte.reserva.is_some())
        && partes.iter().any(|parte| {
            let g = galley(ui, parte.texto, fontes::fonte(12.0, Peso::Medio), Color32::PLACEHOLDER);
            g.size().x > largura - 2.0 - 2.0 * PADDING_APERTADO - largura_nota(ui, parte)
        });
    let (rect, resp) = ui.allocate_exact_size(vec2(total, altura_item + 2.0 * RECUO_SEG), Sense::hover());
    let visivel = ui.is_rect_visible(rect.expand(20.0));
    if visivel {
        let painter = ui.painter();
        painter.add(RectShape::filled(rect, pintura::canto(rect, f32::INFINITY), e.f));
        pintura::sombra_interna_topo(painter, rect.shrink(1.0), f32::INFINITY, 3.0, e.b2);
        painter.add(RectShape::stroke(
            rect,
            pintura::canto(rect, f32::INFINITY),
            Stroke::new(1.0, e.fb),
            egui::StrokeKind::Inside,
        ));
    }
    let mut clicado = None;
    let mut x = rect.left() + RECUO_SEG;
    let tempo = duracao_transicao(ui);
    for (i, parte) in partes.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + RECUO_SEG), vec2(largura, altura_item));
        x += largura;
        let item = ui.interact(r, id.with(i), Sense::click());
        let e_ativo = Some(i) == ativo;
        if item.clicked() {
            clicado = Some(i);
        }
        let hover = ui.ctx().animate_bool_with_time(item.id.with("hover"), item.hovered() && !e_ativo, tempo);
        if visivel && ui.is_rect_visible(r.expand(20.0)) {
            let painter = ui.painter();
            let fora = pintor_de_fora(ui, r);
            if e_ativo {
                let estado = EstadoVolume { presenca: 1.0, hover: 0.0, pressao: 0.0 };
                pintar_volume(painter, &fora, p, r, f32::INFINITY, &volume_escolhido(p), estado);
            } else if hover > 0.0 {
                // `.seg button:hover{box-shadow:0 0 14px --brilho}`: só o halo, sem fundo
                pintura::halo(&fora, r, f32::INFINITY, 14.0, e.brilho.gamma_multiply(hover));
            }
            let cor = if parte.perigo {
                e.perigo
            } else if e_ativo {
                e.t
            } else {
                pintura::misturar(e.t2, e.t, hover)
            };
            if let Some(icone) = parte.reserva.filter(|_| so_icones) {
                // sem espaço para o texto: só o ícone, com o nome na dica
                let caixa = Rect::from_center_size(r.center(), vec2(LADO_ICONE_PARTE, LADO_ICONE_PARTE));
                icones::pintar(painter, caixa, icone, cor);
                if item.hovered() {
                    item.clone().on_hover_text(parte.rotulo());
                }
            } else {
                // a parte apertada (o trilho cheio numa doca estreita) corta o texto, com o nome inteiro na dica
                let cabe = (largura - 2.0 - 2.0 * PADDING_APERTADO - largura_nota(ui, parte)).max(8.0);
                let g = galley_cortado(ui, parte.texto, fontes::fonte(12.0, peso_de(i)), cor, cabe);
                if g.elided && item.hovered() {
                    item.clone().on_hover_text(parte.texto);
                }
                let gn = parte.nota.map(|n| galley(ui, n, fonte_nota.clone(), e.t3));
                let largura_icone = if parte.icone.is_some() { LADO_ICONE_PARTE + GAP_NOTA } else { 0.0 };
                let total_texto = largura_icone + g.size().x + gn.as_ref().map_or(0.0, |gn| GAP_NOTA + gn.size().x);
                let inicio = r.center().x - total_texto / 2.0;
                if let Some(icone) = parte.icone {
                    let caixa = Rect::from_min_size(
                        pos2(inicio, r.center().y - LADO_ICONE_PARTE / 2.0),
                        vec2(LADO_ICONE_PARTE, LADO_ICONE_PARTE),
                    );
                    icones::pintar(painter, caixa, icone, cor);
                }
                let x0 = inicio + largura_icone;
                let largura_texto = g.size().x;
                painter.galley(pos2(x0, r.center().y - g.size().y / 2.0), g, cor);
                if let Some(gn) = gn {
                    let pos = pos2(x0 + largura_texto + GAP_NOTA, r.center().y - gn.size().y / 2.0);
                    painter.galley(pos, gn, e.t3);
                }
            }
            if item.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&fora, p, r, f32::INFINITY);
            }
        }
        if item.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let rotulo = parte.rotulo();
        item.widget_info(|| WidgetInfo::selected(tipo, ui.is_enabled(), e_ativo, &rotulo));
        if abas {
            ui.ctx().accesskit_node_builder(item.id, |no| {
                no.set_role(egui::accesskit::Role::Tab);
                no.set_selected(e_ativo);
            });
        }
    }
    (clicado, resp)
}

// ------------------------------------------------------------------ abas de documento

/// Altura da faixa das abas (o `div[role=tablist]` de 35px: as abas de 34px encostadas embaixo).
pub const ALTURA_ABAS: f32 = 35.0;
/// Altura de uma aba (`.aba{height:34px}`).
pub(crate) const ALTURA_ABA: f32 = 34.0;
/// Raio dos cantos de cima da aba (`border-radius:10px 10px 0 0`: o raio único, [`pintura::RAIO_PADRAO`]).
pub(crate) const RAIO_ABA: f32 = pintura::RAIO_PADRAO;
/// `padding:0 10px` da faixa das abas.
pub(crate) const RECUO_ABAS: f32 = 10.0;
/// `gap:2px` entre as abas.
pub(crate) const ESPACO_ABAS: f32 = 2.0;
/// `.aba{padding:0 6px 0 12px}` e o `padding-right:8px` do `a` das abas sem o × (a do Início), mais a borda de
/// 1px dos dois lados: o que a aba soma ao texto.
const SOBRA_ABA: f32 = 1.0 + 12.0 + 8.0 + 6.0 + 1.0;

/// Os cantos da aba: só os de cima arredondados.
pub(crate) fn cantos_aba() -> CornerRadius {
    let r = RAIO_ABA as u8;
    CornerRadius { nw: r, ne: r, sw: 0, se: 0 }
}

/// A borda da aba ativa (`border:1px solid --vidrob; border-bottom:0`): os lados e o alto com os cantos de cima,
/// no meio do pixel de dentro.
fn contorno_aba(rect: Rect) -> Vec<Pos2> {
    let linha = Rect::from_min_max(rect.min + vec2(0.5, 0.5), pos2(rect.right() - 0.5, rect.bottom()));
    let r = (RAIO_ABA - 0.5).min(linha.width() / 2.0).max(0.0);
    let mut pontos = Vec::with_capacity(28);
    pontos.push(linha.left_bottom());
    let passos = 10;
    let arco = |pontos: &mut Vec<Pos2>, centro: Pos2, a0: f32| {
        for j in 0..=passos {
            let a = a0 + FRAC_PI_2 * j as f32 / passos as f32;
            pontos.push(centro + vec2(a.cos(), a.sin()) * r);
        }
    };
    arco(&mut pontos, pos2(linha.left() + r, linha.top() + r), PI);
    arco(&mut pontos, pos2(linha.right() - r, linha.top() + r), PI + FRAC_PI_2);
    pontos.push(linha.right_bottom());
    pontos
}

/// Pinta a aba ativa (`.aba.on`): o degradê `--bt1` → meio ([`Preenchimento::Aba`], a metade de cima da
/// "superfície só" com a faixa do documento embaixo), as riscas presas à janela, o realce `inset 0 1px 0 --btluz`
/// e a borda `--vidrob` sem o lado de baixo.
pub(crate) fn pintar_aba_ativa(painter: &Painter, p: &Paleta, rect: Rect) {
    let mut fundo = pintura::forma_preenchida(painter.ctx(), p, rect, 0.0, Preenchimento::Aba);
    if let Shape::Rect(r) = &mut fundo {
        r.corner_radius = cantos_aba();
    }
    painter.add(fundo);
    pintura::riscas_com_canto(painter, p, rect, cantos_aba());
    let dentro = Rect::from_min_max(rect.min + vec2(1.0, 1.0), pos2(rect.right() - 1.0, rect.bottom()));
    realce_curvo(painter, dentro, RAIO_ABA - 1.0, p.estudio.btluz);
    painter.add(PathShape::line(contorno_aba(rect), PathStroke::new(1.0, p.estudio.vidrob)));
}

/// As abas de documento (`.aba`, as regras do fim de `Main.dc.html`): itens `(chave, rótulo)`; devolve a chave
/// clicada. Cada aba tem 34px, texto 12px e os cantos de cima com raio 10; as inativas são transparentes em `--t2`
/// (no hover, `--h` e `--t`) e a ativa sobe na superfície com textura ([`Preenchimento::Aba`]) com o texto em
/// `--t`. Embaixo corre o fio `--vidrob` da borda do documento, que a aba ativa cobre (o `margin-bottom:-1px` da
/// faixa das abas). Ocupa a largura disponível ([`ALTURA_ABAS`] de altura). Cada aba é um endereço: quem chama
/// troca a rota.
pub fn abas<K: PartialEq + Copy>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    itens: &[(K, &str)],
    ativa: K,
) -> Option<K> {
    let p = paleta(ui);
    let e = &p.estudio;
    let id = Id::new(("databox::abas", id_salt));
    let (faixa, _) = ui.allocate_exact_size(vec2(ui.available_width(), ALTURA_ABAS), Sense::hover());
    let visivel = ui.is_rect_visible(faixa.expand(8.0));
    if visivel {
        ui.painter().hline(faixa.x_range(), faixa.bottom() - 0.5, Stroke::new(1.0, e.vidrob));
    }
    let tempo = duracao_transicao(ui);
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let mut clicada = None;
    let mut x = faixa.left() + RECUO_ABAS;
    for (i, (chave, rotulo)) in itens.iter().enumerate() {
        let g = galley(ui, rotulo, fonte.clone(), Color32::PLACEHOLDER);
        let largura = (g.size().x + SOBRA_ABA).ceil();
        let r = Rect::from_min_size(pos2(x, faixa.bottom() - ALTURA_ABA), vec2(largura, ALTURA_ABA));
        x += largura + ESPACO_ABAS;
        let item = ui.interact(r, id.with(i), Sense::click());
        let e_ativa = *chave == ativa;
        if item.clicked() {
            clicada = Some(*chave);
        }
        let hover = ui.ctx().animate_bool_with_time(item.id.with("hover"), item.hovered() && !e_ativa, tempo);
        if visivel {
            let painter = ui.painter();
            if e_ativa {
                pintar_aba_ativa(painter, p, r);
            } else if hover > 0.0 {
                painter.add(RectShape::filled(r, cantos_aba(), e.h.gamma_multiply(hover)));
            }
            let cor = if e_ativa { e.t } else { pintura::misturar(e.t2, e.t, hover) };
            painter.galley(pos2(r.left() + 13.0, r.center().y - g.size().y / 2.0), g, cor);
            if item.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&pintor_de_fora(ui, r), p, r, RAIO_ABA);
            }
        }
        if item.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        item.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), e_ativa, *rotulo));
    }
    clicada
}

/// Seletor segmentado (`.seg` com `button[aria-pressed]`, ver [`trilho`]): formato da exportação, tema. Troca
/// `valor` e devolve `true` quando mudou.
pub fn segmentado<K: PartialEq + Copy>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    itens: &[(K, &str)],
    valor: &mut K,
    tamanho: TamanhoTrilho,
) -> bool {
    let rotulos: Vec<&str> = itens.iter().map(|(_, r)| *r).collect();
    let ativo = itens.iter().position(|(k, _)| k == valor);
    let (clicado, _) = trilho(ui, Id::new(("databox::segmentado", id_salt)), &rotulos, ativo, tamanho, true);
    match clicado.and_then(|i| itens.get(i)) {
        Some((k, _)) if k != valor => {
            *valor = *k;
            true
        }
        _ => false,
    }
}

// ------------------------------------------------------------------ trilha

/// Um passo da [`trilha`]: o rótulo e se é link (o último costuma não ser).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Passo<'a> {
    pub rotulo: &'a str,
    pub link: bool,
}

/// Altura da linha da [`trilha`].
const ALTURA_TRILHA: f32 = 17.0;
/// O chevron do `.migalha .ic` (11px) com o `gap:4px` dos dois lados.
const LARGURA_SEPARADOR: f32 = 4.0 + 11.0 + 4.0;

/// O chevron `M9 6l6 6-6 6` (o separador do `.migalha`) numa caixa de 11px centrada em `centro`, com o traço de
/// 1,6 da grade de 24.
fn pintar_chevron(painter: &Painter, centro: Pos2, cor: Color32) {
    let k = 11.0 / 24.0;
    let ponto = |x: f32, y: f32| centro + vec2((x - 12.0) * k, (y - 12.0) * k);
    let pontos = vec![ponto(9.0, 6.0), ponto(15.0, 12.0), ponto(9.0, 18.0)];
    painter.add(PathShape::line(pontos, PathStroke::new(1.6 * k, cor)));
}

/// A trilha "onde estou" (`.migalha`): 12px em `--t2`, os passos separados pelo chevron de 11px em `--t3` com 4px
/// de cada lado; os links ficam `--t` no hover. O lugar atual é o título que vem embaixo (no [`Cabecalho`]), então
/// nenhum passo sai em negrito. Devolve o índice do link clicado.
pub fn trilha(ui: &mut Ui, passos: &[Passo<'_>]) -> Option<usize> {
    let p = paleta(ui);
    let e = &p.estudio;
    let mut clicado = None;
    crate::pecas::arranjo::linha(ui, ALTURA_TRILHA, 0.0, |ui| {
        for (i, passo) in passos.iter().enumerate() {
            if i > 0 {
                let (r, _) = ui.allocate_exact_size(vec2(LARGURA_SEPARADOR, ALTURA_TRILHA), Sense::hover());
                pintar_chevron(ui.painter(), r.center(), e.t3);
            }
            if passo.link {
                let id = ui.next_auto_id();
                let hover = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
                let cor = if hover { e.t } else { e.t2 };
                let g = galley(ui, passo.rotulo, fontes::fonte(12.0, Peso::Normal), cor);
                let (r, resp) = ui.allocate_exact_size(vec2(g.size().x, ALTURA_TRILHA), Sense::click());
                ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, cor);
                if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                    pintura::anel_foco(ui.painter(), p, r, 4.0);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Link, true, passo.rotulo));
                if resp.clicked() {
                    clicado = Some(i);
                }
            } else {
                let g = galley(ui, passo.rotulo, fontes::fonte(12.0, Peso::Normal), e.t2);
                let (r, resp) = ui.allocate_exact_size(vec2(g.size().x, ALTURA_TRILHA), Sense::hover());
                ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, e.t2);
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, passo.rotulo));
            }
        }
    });
    clicado
}

// ------------------------------------------------------------------ cabeçalho

type Conteudo<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// O que aconteceu no cabeçalho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RespostaCabecalho {
    /// Índice do passo da trilha que foi clicado.
    pub trilha_clicada: Option<usize>,
}

/// Altura da linha do título do [`Cabecalho`] (14px / 600 com o "i" de 16px e as pílulas ao lado).
const ALTURA_TITULO: f32 = 20.0;

/// O título da tela: a [`trilha`] em cima (quando a tela é filha de outra), o título em 14px / 600 (`--t`) com o
/// "i" da ajuda, o `sub` só com dados (12px / 350 em `--t3`, o `.mono` do caminho na faixa do documento) e as ações
/// à direita.
#[must_use = "chame .mostrar(ui)"]
pub struct Cabecalho<'a> {
    id: Id,
    titulo: &'a str,
    ajuda: Option<&'a str>,
    sub: Option<&'a str>,
    trilha: &'a [Passo<'a>],
    apos: Option<Conteudo<'a>>,
    acoes: Option<Conteudo<'a>>,
}

impl<'a> Cabecalho<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, titulo: &'a str) -> Self {
        Cabecalho {
            id: Id::new(("databox::cabecalho", id_salt)),
            titulo,
            ajuda: None,
            sub: None,
            trilha: &[],
            apos: None,
            acoes: None,
        }
    }

    /// O texto do "i" ao lado do título.
    pub fn ajuda(mut self, texto: &'a str) -> Self {
        self.ajuda = Some(texto);
        self
    }

    /// `p.sub`: só dados (caminho, contagens); quebra em qualquer ponto.
    #[allow(clippy::should_implement_trait)] // `sub` é o subtítulo, não a subtração
    pub fn sub(mut self, texto: &'a str) -> Self {
        self.sub = Some(texto);
        self
    }

    /// A trilha acima do título (quando a tela é filha de outra).
    pub fn trilha(mut self, passos: &'a [Passo<'a>]) -> Self {
        self.trilha = passos;
        self
    }

    /// Conteúdo depois do "i" no título (a pill "só leitura").
    pub fn apos(mut self, conteudo: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.apos = Some(Box::new(conteudo));
        self
    }

    /// As ações à direita (num grupo só, que desce de linha inteiro e continua à direita).
    pub fn acoes(mut self, conteudo: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.acoes = Some(Box::new(conteudo));
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> RespostaCabecalho {
        let p = paleta(ui);
        let e = &p.estudio;
        let mut resposta = RespostaCabecalho::default();
        let id_medidas = self.id.with("medidas");
        // (largura natural do bloco do título, largura e altura das ações) do quadro anterior
        let (natural_antes, acoes_antes, altura_acoes_antes): (f32, f32, f32) =
            ui.ctx().data(|d| d.get_temp(id_medidas)).unwrap_or((0.0, 0.0, 30.0));
        let topo = ui.cursor().min + vec2(2.0, 0.0);
        let largura = (ui.available_width() - 4.0).max(0.0);
        let tem_acoes = self.acoes.is_some();
        let cabe = !tem_acoes || natural_antes + 12.0 + acoes_antes <= largura;
        let largura_titulo = if cabe && tem_acoes { (largura - acoes_antes - 12.0).max(0.0) } else { largura };

        // bloco do título
        let rect_titulo = Rect::from_min_size(topo, vec2(largura_titulo, f32::INFINITY));
        let mut ui_titulo = ui.new_child(
            UiBuilder::new().id_salt(self.id.with("titulo")).max_rect(rect_titulo).layout(Layout::top_down(Align::Min)),
        );
        ui_titulo.spacing_mut().item_spacing.y = 0.0;
        let mut natural: f32 = 0.0;
        if !self.trilha.is_empty() {
            resposta.trilha_clicada = trilha(&mut ui_titulo, self.trilha);
            natural = natural.max(ui_titulo.min_rect().width());
            ui_titulo.add_space(4.0);
        }
        crate::pecas::arranjo::linha(&mut ui_titulo, ALTURA_TITULO, 0.0, |ui| {
            let g = galley(ui, self.titulo, fontes::fonte(14.0, Peso::Medio), e.t);
            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x, ALTURA_TITULO), Sense::hover());
            ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, e.t);
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, self.titulo));
            if let Some(a) = self.ajuda {
                ui.add_space(8.0);
                ajuda(ui, a);
            }
            if let Some(apos) = self.apos {
                ui.add_space(8.0);
                apos(ui);
            }
        });
        natural = natural.max(ui_titulo.min_rect().width());
        if let Some(sub) = self.sub {
            ui_titulo.add_space(4.0);
            let fonte = fontes::fonte(12.0, Peso::Leve);
            let inteira = galley(&ui_titulo, sub, fonte.clone(), e.t3).size().x;
            natural = natural.max(inteira);
            let limite = largura_titulo.min(72.0 * 7.0);
            let g = galley_quebrado(&ui_titulo, sub, fonte, e.t3, limite, 1.5);
            // só dados (o caminho do banco): dá para selecionar e copiar
            crate::pecas::texto::mostrar_paragrafo_selecionavel(&mut ui_titulo, &g, sub);
        }
        let bloco = ui_titulo.min_rect();
        let mut usado = Rect::from_min_size(topo, vec2(largura, bloco.height()));

        // ações
        let mut medida_acoes = (0.0, 30.0);
        if let Some(acoes) = self.acoes {
            let y = if cabe { bloco.center().y - altura_acoes_antes / 2.0 } else { bloco.bottom() + 12.0 };
            let x = (topo.x + largura - acoes_antes).max(topo.x);
            let rect_acoes =
                Rect::from_min_max(pos2(x, y), pos2(topo.x + largura + 4000.0, y + altura_acoes_antes.max(30.0)));
            let mut ui_acoes = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("acoes"))
                    .max_rect(rect_acoes)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            ui_acoes.spacing_mut().item_spacing.x = 8.0;
            acoes(&mut ui_acoes);
            let r = ui_acoes.min_rect();
            medida_acoes = (if r.is_positive() { r.width() } else { 0.0 }, r.height().max(1.0));
            usado = usado.union(r.intersect(Rect::from_min_size(topo, vec2(largura, f32::INFINITY))));
        }
        let medidas = (natural, medida_acoes.0, medida_acoes.1);
        if (medidas.0 - natural_antes).abs() > 0.5
            || (medidas.1 - acoes_antes).abs() > 0.5
            || (medidas.2 - altura_acoes_antes).abs() > 0.5
        {
            ui.ctx().data_mut(|d| d.insert_temp(id_medidas, medidas));
            ui.ctx().request_discard("medidas do cabeçalho");
        }
        ui.allocate_rect(usado.expand2(vec2(2.0, 0.0)), Sense::hover());
        ui.add_space((16.0 - ui.spacing().item_spacing.y).max(0.0));
        resposta
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn medidas_do_seg() {
        // `.seg button{height:26px;padding:0 14px}`; o compacto, o da faixa da Consulta (24px, 0 12px)
        assert_eq!(TamanhoTrilho::Normal.medidas(), (26.0, 14.0));
        assert_eq!(TamanhoTrilho::Cheio.medidas(), (26.0, 14.0));
        assert_eq!(TamanhoTrilho::Compacto.medidas(), (24.0, 12.0));
        assert_eq!(TamanhoTrilho::CompactoCheio.medidas(), (24.0, 12.0));
        assert!(TamanhoTrilho::Cheio.cheio() && TamanhoTrilho::CompactoCheio.cheio());
        assert!(!TamanhoTrilho::Normal.cheio() && !TamanhoTrilho::Compacto.cheio());
    }

    #[test]
    fn partes_do_seg_sao_iguais_e_o_cheio_vai_de_ponta_a_ponta() {
        // sem ocupar a linha: todas com a largura da maior
        assert_eq!(largura_das_partes(106.0, 3, None), 106.0);
        // cheio: a linha de 460px menos a borda e o `padding:2px` dos dois lados, em três partes iguais
        assert_eq!(largura_das_partes(106.0, 3, Some(460.0)), (460.0 - 6.0) / 3.0);
        // a linha estreita demais: as partes dividem o que há (o texto corta com "…"), até o mínimo
        assert_eq!(largura_das_partes(106.0, 3, Some(200.0)), (200.0 - 6.0) / 3.0);
        assert_eq!(largura_das_partes(106.0, 3, Some(60.0)), MINIMO_DA_PARTE);
        assert_eq!(largura_das_partes(0.0, 0, Some(200.0)), 0.0);
    }

    #[test]
    fn contorno_da_aba_nao_fecha_embaixo() {
        let r = Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, ALTURA_ABA));
        let c = contorno_aba(r);
        // começa e termina embaixo, nos lados (sem o `border-bottom`)
        assert_eq!(c.first().map(|p| p.y), Some(r.bottom()));
        assert_eq!(c.last().map(|p| p.y), Some(r.bottom()));
        // o alto fica no meio do primeiro pixel
        let topo = c.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!((topo - (r.top() + 0.5)).abs() < 0.01, "{topo}");
        assert_eq!(cantos_aba(), CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 });
    }
}
