//! O cartão no visual Estúdio: um "documento" (o `.doc` das pranchetas) — fundo `--c`, borda 1px `--vidrob`, raio
//! 10 ([`crate::pintura::RAIO_PADRAO`]) e só a sombra curta dos painéis. O título vira a faixa `.sec` (32px; 42px
//! com ações, a altura da `.faixa`): título 12px / 600 em `--t`, o `hint` em `--t3`, conteúdo livre (`extra`) e as
//! ações à direita, separada do corpo por um fio `--vidrob`. Corpos (`corpo` 18/20, `corpo_flush` sem padding,
//! `corpo_avisos` 14/20; corpos seguidos ganham o fio `--vidrob` entre eles) e o rodapé `.rodape` (34px, fundo
//! `--p`, fio `--b` em cima, `padding:0 8px 0 14px`, texto `--t2`), que no fim do cartão acompanha os cantos de
//! baixo dele.
//!
//! ```ignore
//! Cartao::new("tabelas").titulo("Tabelas").hint("7 tabelas")
//!     .acoes(|ui| { ui.add(Botao::new("Nova tabela")); })
//!     .mostrar(ui, |c| {
//!         c.corpo_flush(|ui| { /* tabela */ });
//!         c.rodape(|ui| { hint(ui, "Ordem alfabética"); });
//!     });
//! ```
//! Cartões empilhados ficam a 16px um do outro: use [`espaco_entre_cards`].

use std::hash::Hash;

use egui::layers::ShapeIdx;
use egui::{
    Align, CornerRadius, Id, InnerResponse, Layout, Margin, Rect, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, vec2,
};

use crate::fontes::{self, Peso};
use crate::pecas::arranjo::linha_com_direita;
use crate::pecas::texto::galley;
use crate::pintura;
use crate::tema::paleta;

/// Raio do cartão, do documento e da janela (`border-radius:10px`): o raio único, [`pintura::RAIO_PADRAO`].
pub const RAIO_CARD: f32 = pintura::RAIO_PADRAO;
/// Distância entre cartões empilhados.
pub const ESPACO_CARDS: f32 = 16.0;
/// Altura da faixa do título (`.sec{height:32px}`) e a dela com ações (a `.faixa`, 42px), sem o fio de baixo.
pub const ALTURA_TITULO: f32 = 32.0;
pub const ALTURA_TITULO_ACOES: f32 = 42.0;
/// Altura do rodapé (`.rodape{height:34px}`, com o fio de cima).
pub const ALTURA_RODAPE: f32 = 34.0;

/// Adiciona o espaço que falta para dois cartões empilhados ficarem a 16px.
pub fn espaco_entre_cards(ui: &mut Ui) {
    let ja = ui.spacing().item_spacing.y;
    ui.add_space((ESPACO_CARDS - ja).max(0.0));
}

type Acoes<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// Construtor do cartão.
#[must_use = "chame .mostrar(ui, |c| …)"]
pub struct Cartao<'a> {
    id: Id,
    titulo: Option<&'a str>,
    dica_titulo: Option<&'a str>,
    hint: Option<&'a str>,
    extra: Option<Acoes<'a>>,
    acoes: Option<Acoes<'a>>,
    esticar: bool,
}

impl<'a> Cartao<'a> {
    pub fn new(id_salt: impl Hash + std::fmt::Debug) -> Self {
        Cartao {
            id: Id::new(("databox::cartao", id_salt)),
            titulo: None,
            dica_titulo: None,
            hint: None,
            extra: None,
            acoes: None,
            esticar: false,
        }
    }

    /// O título da faixa `.sec` (só o nome; a explicação vai no "i").
    pub fn titulo(mut self, titulo: &'a str) -> Self {
        self.titulo = Some(titulo);
        self
    }

    /// Dica do título (o `title`; a Consulta SQL põe ali o comando).
    pub fn dica_titulo(mut self, dica: &'a str) -> Self {
        self.dica_titulo = Some(dica);
        self
    }

    /// O texto em `--t3` ao lado do título (a contagem: "3 itens").
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    /// Conteúdo livre depois do hint, ainda à esquerda (o "contando" com o ponto pulsante, o "i" mini).
    pub fn extra(mut self, extra: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.extra = Some(Box::new(extra));
        self
    }

    /// Ocupa a altura toda do lugar (os cartões lado a lado de [`crate::pecas::arranjo::colunas_iguais`], que
    /// ficam da mesma altura).
    pub fn esticar(mut self) -> Self {
        self.esticar = true;
        self
    }

    /// As ações à direita da faixa do título, na ordem em que aparecem.
    pub fn acoes(mut self, acoes: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.acoes = Some(Box::new(acoes));
        self
    }

    /// Desenha o cartão na largura disponível. O conteúdo recebe um [`CorpoCartao`] para os corpos e o rodapé.
    pub fn mostrar<R>(self, ui: &mut Ui, conteudo: impl FnOnce(&mut CorpoCartao<'_>) -> R) -> InnerResponse<R> {
        let p = paleta(ui);
        let e = &p.estudio;
        let largura = ui.available_width();
        let topo = ui.cursor().min;
        let lugar_sombra = ui.painter().add(Shape::Noop);
        let lugar_fundo = ui.painter().add(Shape::Noop);

        // o conteúdo fica dentro da borda de 1px
        let max = Rect::from_min_size(topo + vec2(1.0, 1.0), vec2((largura - 2.0).max(0.0), f32::INFINITY));
        let mut filho =
            ui.new_child(UiBuilder::new().id_salt(self.id).max_rect(max).layout(Layout::top_down(Align::Min)));
        filho.spacing_mut().item_spacing.y = 0.0;
        // overflow:hidden na largura do cartão
        let corte = filho
            .clip_rect()
            .intersect(Rect::from_x_y_ranges(topo.x + 1.0..=topo.x + largura - 1.0, filho.clip_rect().y_range()));
        filho.set_clip_rect(corte);

        let tem_cabecalho =
            self.titulo.is_some() || self.hint.is_some() || self.acoes.is_some() || self.extra.is_some();
        if tem_cabecalho {
            cabecalho(&mut filho, self.id, self.titulo, self.dica_titulo, self.hint, self.extra, self.acoes);
        }
        let mut corpo = CorpoCartao { ui: &mut filho, secoes: 0, id: self.id, rodapes: Vec::new() };
        let inner = conteudo(&mut corpo);
        let rodapes = std::mem::take(&mut corpo.rodapes);
        let mut altura = (filho.min_rect().bottom() + 1.0 - topo.y).max(2.0);
        if self.esticar {
            ui.ctx().data_mut(|d| d.insert_temp(crate::pecas::arranjo::id_altura_natural(ui), altura));
            let disponivel = ui.max_rect().bottom() - topo.y;
            if disponivel.is_finite() {
                altura = altura.max(disponivel);
            }
        }
        let rect = Rect::from_min_size(topo, vec2(largura, altura));
        let resp = ui.allocate_rect(rect, Sense::hover());

        ui.painter().set(lugar_sombra, pintura::forma_sombra(rect, RAIO_CARD, e.sombra_painel));
        ui.painter().set(lugar_fundo, Shape::rect_filled(rect, RAIO_CARD, e.doc));
        // o rodapé que fecha o cartão leva os cantos de baixo dele (por dentro da borda)
        let raio_dentro = (RAIO_CARD - 1.0) as u8;
        for (lugar, r) in rodapes {
            let ultimo = r.bottom() >= rect.bottom() - 1.5;
            let raio = if ultimo { raio_dentro } else { 0 };
            let cantos = CornerRadius { nw: 0, ne: 0, sw: raio, se: raio };
            ui.painter().set(lugar, Shape::rect_filled(r, cantos, e.faixa));
        }
        ui.painter().rect_stroke(rect, RAIO_CARD, Stroke::new(1.0, e.vidrob), StrokeKind::Inside);
        InnerResponse::new(inner, resp)
    }
}

fn cabecalho<'a>(
    ui: &mut Ui,
    id: Id,
    titulo: Option<&'a str>,
    dica_titulo: Option<&'a str>,
    hint: Option<&'a str>,
    extra: Option<Acoes<'a>>,
    acoes: Option<Acoes<'a>>,
) {
    let e = &paleta(ui).estudio;
    let com_acoes = acoes.is_some();
    let altura = if com_acoes { ALTURA_TITULO_ACOES } else { ALTURA_TITULO };
    let inicio = ui.cursor().min;
    let largura = ui.available_width();
    // `.sec{padding:0 12px}`; com ações, os 10px da direita da `.faixa`
    let (esquerda, direita) = (12.0, if com_acoes { 10.0 } else { 12.0 });
    let interno =
        Rect::from_min_size(inicio + vec2(esquerda, 0.0), vec2((largura - esquerda - direita).max(0.0), altura));
    let mut linha =
        ui.new_child(UiBuilder::new().id_salt(id.with("cab")).max_rect(interno).layout(Layout::top_down(Align::Min)));
    linha_com_direita(
        &mut linha,
        id.with("acoes"),
        altura,
        12.0,
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if let Some(t) = titulo {
                let g = galley(ui, t, fontes::fonte(12.0, Peso::Medio), e.t);
                let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(rect.min, g, e.t);
                let resp = match dica_titulo {
                    Some(d) => resp.on_hover_text(d),
                    None => resp,
                };
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, t));
            }
            if let Some(h) = hint {
                let g = galley(ui, h, fontes::fonte(12.0, Peso::Normal), e.t3);
                let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(rect.min, g, e.t3);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, h));
            }
            if let Some(x) = extra {
                x(ui);
            }
        },
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if let Some(a) = acoes {
                a(ui);
            }
        },
    );
    // a faixa e o fio `--vidrob` embaixo
    let fim = Rect::from_min_size(inicio, vec2(largura, altura + 1.0));
    ui.allocate_rect(fim, Sense::hover());
    ui.painter().hline(fim.x_range(), fim.bottom() - 0.5, Stroke::new(1.0, e.vidrob));
}

/// As partes de dentro do cartão. Cada corpo depois do primeiro ganha um fio `--vidrob` em cima (o primeiro já
/// tem o fio de baixo da faixa do título).
pub struct CorpoCartao<'u> {
    ui: &'u mut Ui,
    secoes: usize,
    id: Id,
    /// O lugar do fundo de cada rodapé e onde ele ficou (o cartão pinta no fim, com os cantos se for o último).
    rodapes: Vec<(ShapeIdx, Rect)>,
}

impl CorpoCartao<'_> {
    fn secao<R>(&mut self, margem: Margin, fio_acima: bool, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        let e = &paleta(self.ui).estudio;
        let largura = self.ui.available_width();
        let inicio = self.ui.cursor().min;
        if fio_acima && self.secoes > 0 {
            // o fio da faixa do título já separa o primeiro corpo
            self.ui.painter().hline(inicio.x..=inicio.x + largura, inicio.y + 0.5, Stroke::new(1.0, e.vidrob));
        }
        let interno = Rect::from_min_size(
            inicio + vec2(f32::from(margem.left), f32::from(margem.top)),
            vec2((largura - f32::from(margem.left) - f32::from(margem.right)).max(0.0), f32::INFINITY),
        );
        let n = self.secoes;
        let mut filho = self.ui.new_child(
            UiBuilder::new().id_salt(self.id.with(("secao", n))).max_rect(interno).layout(Layout::top_down(Align::Min)),
        );
        filho.spacing_mut().item_spacing = vec2(8.0, 8.0);
        // o aviso que abre um corpo ganha 14px embaixo (o `livre` não é um corpo)
        if fio_acima {
            crate::pecas::avisos::marcar_corpo(&filho);
        }
        let r = conteudo(&mut filho);
        let altura = filho.min_rect().height().max(0.0) + f32::from(margem.top) + f32::from(margem.bottom);
        self.ui.allocate_rect(Rect::from_min_size(inicio, vec2(largura, altura)), Sense::hover());
        self.secoes += 1;
        r
    }

    /// Corpo com `padding:18px 20px`.
    pub fn corpo<R>(&mut self, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        self.secao(Margin::symmetric(20, 18), true, conteudo)
    }

    /// Corpo sem padding (tabela de borda a borda).
    pub fn corpo_flush<R>(&mut self, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        self.secao(Margin::ZERO, true, conteudo)
    }

    /// Corpo com `padding:14px 20px` (o cartão que é só um aviso).
    pub fn corpo_avisos<R>(&mut self, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        self.secao(Margin::symmetric(20, 14), true, conteudo)
    }

    /// Um corpo com padding próprio (com fio acima, se não for o primeiro).
    pub fn corpo_com_margem<R>(&mut self, margem: Margin, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        self.secao(margem, true, conteudo)
    }

    /// Conteúdo sem padding nem fio (a zona de risco, as sanfonas).
    pub fn livre<R>(&mut self, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        self.secao(Margin::ZERO, false, conteudo)
    }

    /// O rodapé (`.rodape`): fio `--b` em cima, 34px, fundo `--p`, `padding:0 8px 0 14px`, itens em linha
    /// (centrados na vertical) com 8px entre eles. O texto do rodapé vai em `--t2` (`texto(ui, …, 12.0,
    /// p.estudio.t2)`) ou `--t3` (`hint`).
    pub fn rodape(&mut self, conteudo: impl FnOnce(&mut Ui)) {
        self.rodape_dividido(conteudo, |_| {});
    }

    /// Rodapé com uma parte à esquerda e outra encostada à direita (o rodapé da grade: "Linhas 1–100 de 5.000"
    /// · spacer · por página, ‹, "Página 1 de 50", ›).
    pub fn rodape_dividido(&mut self, esquerda: impl FnOnce(&mut Ui), direita: impl FnOnce(&mut Ui)) {
        let e = &paleta(self.ui).estudio;
        let largura = self.ui.available_width();
        let inicio = self.ui.cursor().min;
        let lugar_fundo = self.ui.painter().add(Shape::Noop);
        self.ui.painter().hline(inicio.x..=inicio.x + largura, inicio.y + 0.5, Stroke::new(1.0, e.b));
        // a linha tem 33px (34 com o fio); um conteúdo mais alto (medido no quadro anterior) a estica
        let n = self.secoes;
        let minima = ALTURA_RODAPE - 1.0;
        let id_altura = self.id.with(("rodape_altura", n));
        let altura_antes: f32 =
            self.ui.ctx().data(|d| d.get_temp::<f32>(id_altura)).unwrap_or(minima).clamp(minima, 200.0);
        let (recuo_esq, recuo_dir) = (14.0, 8.0);
        let interno = Rect::from_min_size(
            inicio + vec2(recuo_esq, 1.0),
            vec2((largura - recuo_esq - recuo_dir).max(0.0), altura_antes),
        );
        let mut filho = self.ui.new_child(
            UiBuilder::new()
                .id_salt(self.id.with(("rodape", n)))
                .max_rect(interno)
                .layout(Layout::top_down(Align::Min)),
        );
        let usado = linha_com_direita(
            &mut filho,
            self.id.with(("rodape_linha", n)),
            altura_antes,
            8.0,
            |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                esquerda(ui);
            },
            |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                direita(ui);
            },
        );
        // os itens ficam centrados na linha: a altura usada é duas vezes a distância do meio até a ponta mais longe
        let meio = interno.center().y;
        let medida = if usado.is_positive() {
            (2.0 * (meio - usado.top()).max(usado.bottom() - meio)).max(minima)
        } else {
            minima
        };
        if (medida - altura_antes).abs() > 0.5 {
            self.ui.ctx().data_mut(|d| d.insert_temp(id_altura, medida));
            self.ui.ctx().request_discard("altura do rodapé do cartão");
        }
        let rect = Rect::from_min_size(inicio, vec2(largura, altura_antes + 1.0));
        self.ui.allocate_rect(rect, Sense::hover());
        // o fundo fica embaixo do fio e do conteúdo; o cartão decide os cantos no fim
        self.rodapes.push((lugar_fundo, rect));
        self.secoes += 1;
    }

    /// O `Ui` do cartão, sem padding (para quem precisa de algo fora do padrão).
    pub fn ui(&mut self) -> &mut Ui {
        self.ui
    }
}
