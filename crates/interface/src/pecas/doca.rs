//! As docas da casca (revisão de UI de 19/10/2026: a `.doca` de `Main.dc.html`, `Dados.dc.html` e `Consulta.dc.html`):
//! o lugar de um ou mais painéis (o Explorador, o Histórico, as Propriedades), com a fileira de abas `.abas-doca` no
//! lugar do cabeçalho fino de antes e a superfície do painel (`.vidro`) embaixo.
//!
//! - **A fileira** ([`ALTURA_ABAS`], a mesma das abas de documento): as abas encostadas embaixo, a primeira na quina
//!   da superfície (`padding-left:0`), com `gap:2px`; a ativa sobe na superfície com textura e forma uma superfície
//!   só com o painel, como a aba de documento (com a primeira ativa, a quina de cima à esquerda do painel fica reta).
//!   Na ponta direita, o lugar das ações do painel ([`RespostaDoca::acoes`], que o [`Painel`] encaixado ocupa com o
//!   fio `.sep-doca` antes delas) e o "Destacar em janela" (a silhueta das duas janelas, [`Icone::Destacar`]).
//! - **A forma das abas** ([`FormaAbas`]) é uma só por doca: todas só ícone (`.aba.so-ic`, a doca estreita da
//!   esquerda, com o nome na dica) ou todas com o nome e o sub (`Propriedades · clientes`, a doca larga da direita).
//!   Um painel preso numa doca adota a forma dela.
//! - **Arrastar uma aba** (prancheta 2b, `DadosArrastar.dc.html`): a aba vira uma etiqueta que segue o ponteiro
//!   ([`pintar_etiqueta_arrastada`]); a doca diz onde ela está e quando foi solta ([`Arrasto`]), e quem chama decide
//!   o destino (outra doca, uma janela própria) e acende o alvo ([`pintar_alvo`]). Esc cancela.
//! - Acessibilidade: a fileira é a `tablist` de [`Doca::new`] ("Painéis à esquerda"); cada aba é um `tab` com o nome
//!   do painel ("Explorador", "Propriedades · clientes").
//!
//! ```ignore
//! let abas = [AbaDoca::new(Icone::Explorador, "Explorador"), AbaDoca::new(Icone::Historico, "Histórico")];
//! let r = Doca::new("doca-esquerda", "Painéis à esquerda", &abas).ativa(0).forma(FormaAbas::SoIcone).mostrar(ui);
//! let mut corpo = ui.new_child(UiBuilder::new().max_rect(r.corpo));
//! Painel::new("explorador", "Explorador").encaixe(Encaixe::doca(r.acoes)).acoes(|ui| { /* … */ }).mostrar(&mut corpo, |ui| {});
//! ```
//!
//! [`Painel`]: crate::pecas::painel::Painel

use egui::accesskit::Role;
use egui::epaint::RectShape;
use egui::{
    Align, Color32, CornerRadius, CursorIcon, Id, Key, Layout, Order, Pos2, Rangef, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::abas_documento::cor_sub_aba;
use crate::pecas::botao::{Botao, LADO_CABECALHO, forma_realce_curvo, pintor_de_fora};
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::navegacao::{ALTURA_ABA, ESPACO_ABAS, RAIO_ABA, cantos_aba, pintar_aba_ativa};
use crate::pecas::texto::{galley, galley_cortado};
use crate::pintura;
use crate::tema::{Paleta, duracao_transicao, paleta};

/// Altura da fileira de abas da doca (`.abas-doca{height:35px}`, a mesma das abas de documento).
pub const ALTURA_ABAS: f32 = crate::pecas::navegacao::ALTURA_ABAS;
/// Lado do ícone da aba (`.sk.s16`).
const LADO_ICONE: f32 = 16.0;
/// `.aba.so-ic{padding:0 10px}` com a borda de 1px dos dois lados.
const LARGURA_SO_ICONE: f32 = 1.0 + 10.0 + LADO_ICONE + 10.0 + 1.0;
/// `gap:7px` entre o ícone, o nome e o sub.
const GAP: f32 = 7.0;
/// Na aba com nome: a borda, o `padding-left:12px`, o ícone e o `gap` até o nome.
const ANTES_DO_TEXTO: f32 = 1.0 + 12.0 + LADO_ICONE + GAP;
/// Na aba com nome (sem o ×): o `padding-right:8px` do `a`, o da aba e a borda.
const DEPOIS_DO_TEXTO: f32 = 8.0 + 6.0 + 1.0;
/// O texto mínimo de uma aba com nome encolhida (o resto vira "…").
const TEXTO_MINIMO: f32 = 30.0;
/// `.abas-doca{padding-right:2px}` e `.abas-doca .ib.pq{margin:0 0 4px 0}`: os botões a 2px da direita e 4px de
/// baixo.
const RECUO_DIREITA: f32 = 2.0;
const RECUO_BAIXO_BOTOES: f32 = 4.0;
/// `.sep-doca{width:1px;height:18px;margin:0 4px 8px}`.
pub const ALTURA_SEP: f32 = 18.0;
pub const MARGEM_SEP: f32 = 4.0;
pub const BAIXO_SEP: f32 = 8.0;

/// A forma das abas de uma doca (uma só por doca).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormaAbas {
    /// Só o ícone (`.aba.so-ic`), com o nome na dica: a doca estreita da esquerda.
    SoIcone,
    /// O ícone, o nome e o sub: a doca larga da direita.
    ComNome,
}

/// Uma aba da doca: o ícone, o nome do painel e o sub (o objeto das Propriedades: "clientes").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbaDoca<'a> {
    pub icone: Icone,
    pub nome: &'a str,
    pub sub: Option<&'a str>,
}

impl<'a> AbaDoca<'a> {
    pub fn new(icone: Icone, nome: &'a str) -> Self {
        AbaDoca { icone, nome, sub: None }
    }

    /// O sub depois do nome (o objeto das Propriedades); na aba só ícone, ele entra na dica.
    #[allow(clippy::should_implement_trait)] // o nome é o do `.aba .sub` das pranchetas
    pub fn sub(mut self, sub: Option<&'a str>) -> Self {
        self.sub = sub;
        self
    }

    /// O nome para o leitor de tela e a dica: "Propriedades · clientes", ou só o nome.
    pub fn rotulo(&self) -> String {
        match self.sub {
            Some(s) => format!("{} · {s}", self.nome),
            None => self.nome.to_owned(),
        }
    }
}

/// Uma aba sendo arrastada para fora da fileira.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrasto {
    /// A aba (o índice na fileira).
    pub aba: usize,
    /// Onde está o ponteiro (pode estar fora da janela, enquanto o botão segue apertado).
    pub ponteiro: Pos2,
    /// O botão foi solto neste quadro: quem chama aplica o destino.
    pub solto: bool,
}

/// O que aconteceu na doca neste quadro e onde ficam as partes dela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RespostaDoca {
    /// A aba clicada (com o botão principal ou o teclado): quem chama a põe à vista.
    pub clicada: Option<usize>,
    /// O "Destacar em janela" foi clicado (a aba ativa vai para uma janela própria).
    pub destacar: bool,
    /// Uma aba sendo arrastada (ou solta neste quadro); `None` sem arrasto ou depois do Esc.
    pub arrasto: Option<Arrasto>,
    /// A fileira de abas.
    pub fileira: Rect,
    /// O lugar das ações do painel na fileira (os botões de 20px, da direita para a esquerda; ver
    /// [`crate::pecas::painel::Painel::encaixe`]).
    pub acoes: Rect,
    /// A superfície do painel embaixo da fileira.
    pub superficie: Rect,
    /// O espaço de dentro da superfície (sem a borda de 1px), onde o painel desenha o corpo.
    pub corpo: Rect,
}

/// Uma doca: a fileira de abas e a superfície. Ocupa o espaço que o `Ui` tem (quem chama dá a ele o retângulo da
/// doca inteira).
#[must_use = "chame .mostrar(ui)"]
pub struct Doca<'a> {
    id: Id,
    rotulo: &'a str,
    abas: &'a [AbaDoca<'a>],
    ativa: usize,
    forma: FormaAbas,
    destacar: bool,
}

impl<'a> Doca<'a> {
    /// `rotulo` é o nome da `tablist` ("Painéis à esquerda", "Painéis à direita").
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, rotulo: &'a str, abas: &'a [AbaDoca<'a>]) -> Self {
        Doca {
            id: Id::new(("databox::doca", id_salt)),
            rotulo,
            abas,
            ativa: 0,
            forma: FormaAbas::ComNome,
            destacar: true,
        }
    }

    /// A aba à vista (o índice; fora da fileira, a primeira).
    pub fn ativa(mut self, ativa: usize) -> Self {
        self.ativa = ativa;
        self
    }

    pub fn forma(mut self, forma: FormaAbas) -> Self {
        self.forma = forma;
        self
    }

    /// Com o botão "Destacar em janela" na ponta da fileira (o padrão).
    pub fn destacar(mut self, sim: bool) -> Self {
        self.destacar = sim;
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> RespostaDoca {
        let p = paleta(ui);
        let e = &p.estudio;
        let rect = ui.available_rect_before_wrap();
        let fileira = Rect::from_min_size(rect.min, vec2(rect.width(), ALTURA_ABAS));
        // a superfície começa 1px antes do fim da fileira (`margin-bottom:-1px`): a aba ativa e ela formam uma peça só
        let superficie = Rect::from_min_max(pos2(rect.left(), fileira.bottom() - 1.0), rect.max);
        let ativa = if self.ativa < self.abas.len() { self.ativa } else { 0 };
        let mut resposta = RespostaDoca {
            clicada: None,
            destacar: false,
            arrasto: None,
            fileira,
            acoes: Rect::NOTHING,
            superficie,
            corpo: superficie.shrink(1.0),
        };

        // o Destacar, na ponta direita (`padding-right:2px`, `margin-bottom:4px`)
        let base_botoes = fileira.bottom() - RECUO_BAIXO_BOTOES;
        let mut direita = fileira.right() - RECUO_DIREITA;
        if self.destacar {
            let r = Rect::from_min_max(
                pos2(direita - LADO_CABECALHO, base_botoes - LADO_CABECALHO),
                pos2(direita, base_botoes),
            );
            let mut filho = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("destacar"))
                    .max_rect(r)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let botao = Botao::icone(Icone::Destacar)
                .do_cabecalho()
                .rotulo("Destacar em janela")
                .dica("Destacar em janela (ou arraste a aba para fora)");
            resposta.destacar = filho.add(botao).clicked();
            direita = r.left() - ESPACO_ABAS;
        }

        // as larguras das abas: só ícone, fixas; com nome, o texto encolhe por igual quando falta espaço
        let fonte = fontes::fonte(12.0, Peso::Normal);
        let larguras: Vec<f32> = match self.forma {
            FormaAbas::SoIcone => vec![LARGURA_SO_ICONE; self.abas.len()],
            FormaAbas::ComNome => {
                let naturais: Vec<f32> = self
                    .abas
                    .iter()
                    .map(|a| {
                        let nome = galley(ui, a.nome, fonte.clone(), Color32::PLACEHOLDER).size().x;
                        let sub =
                            a.sub.map_or(0.0, |s| GAP + galley(ui, s, fonte.clone(), Color32::PLACEHOLDER).size().x);
                        nome + sub
                    })
                    .collect();
                let fixas = vec![ANTES_DO_TEXTO + DEPOIS_DO_TEXTO; self.abas.len()];
                let disponivel = direita - fileira.left();
                let textos = crate::pecas::abas_documento::larguras_texto(
                    &naturais,
                    &fixas,
                    ESPACO_ABAS,
                    disponivel,
                    TEXTO_MINIMO,
                );
                fixas.iter().zip(&textos).map(|(f, t)| (f + t).ceil()).collect()
            }
        };

        // a superfície antes das abas (a aba ativa pinta por cima do fio de cima dela)
        let mut x = fileira.left();
        let mut rects = Vec::with_capacity(self.abas.len());
        for l in &larguras {
            rects.push(Rect::from_min_size(pos2(x, fileira.bottom() - ALTURA_ABA), vec2(*l, ALTURA_ABA)));
            x += l + ESPACO_ABAS;
        }
        let fim_das_abas = rects.last().map_or(fileira.left(), |r| r.right());
        let juncao = rects.get(ativa).map(|r| Rangef::new(r.left(), r.right()));
        pintar_superficie(ui.painter(), p, superficie, juncao);

        let tempo = duracao_transicao(ui);
        let id_arrasto = self.id.with("arrasto");
        let mut cancelado: bool = ui.ctx().data(|d| d.get_temp(id_arrasto)).unwrap_or(false);
        let mut algum_arrasto = false;
        for (i, (aba, r)) in self.abas.iter().zip(&rects).enumerate() {
            let visivel = r.intersect(Rect::from_x_y_ranges(fileira.left()..=direita, fileira.y_range()));
            if !visivel.is_positive() {
                continue;
            }
            let e_ativa = i == ativa;
            let rotulo = aba.rotulo();
            let resp = ui.interact(visivel, self.id.with(("aba", i)), Sense::click_and_drag());
            if resp.clicked() {
                resposta.clicada = Some(i);
            }
            if resp.dragged() || resp.drag_stopped() {
                algum_arrasto = true;
            }
            if let Some(a) = ler_arrasto(ui, &resp, &mut cancelado, i) {
                resposta.arrasto = Some(a);
            }

            let hover =
                ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.contains_pointer() && !e_ativa, tempo);
            let pintor = ui.painter().with_clip_rect(visivel);
            if e_ativa {
                pintar_aba_ativa(&pintor, p, *r);
            } else if hover > 0.0 {
                pintor.add(RectShape::filled(*r, cantos_aba(), e.h.gamma_multiply(hover)));
            }
            let cor = if e_ativa { e.t } else { pintura::misturar(e.t2, e.t, hover) };
            match self.forma {
                FormaAbas::SoIcone => {
                    let icone = Rect::from_center_size(r.center(), vec2(LADO_ICONE, LADO_ICONE));
                    icones::pintar(&pintor, icone, aba.icone, cor);
                }
                FormaAbas::ComNome => pintar_nome(ui, &pintor, *r, aba, cor, e_ativa),
            }
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&pintor_de_fora(ui, *r), p, *r, RAIO_ABA);
            }
            if self.forma == FormaAbas::SoIcone {
                resp.clone().on_hover_text(&rotulo);
            }
            resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), e_ativa, &rotulo));
            ui.ctx().accesskit_node_builder(resp.id, |no| {
                no.set_role(Role::Tab);
                no.set_selected(e_ativa);
            });
        }
        // o Esc vale até o botão ser solto; o próximo arrasto começa limpo
        let cancelado = cancelado && algum_arrasto && !ui.input(|i| i.pointer.any_released());
        ui.ctx().data_mut(|d| d.insert_temp(id_arrasto, cancelado));
        if let Some(a) = resposta.arrasto
            && let Some(aba) = self.abas.get(a.aba)
            && !a.solto
        {
            pintar_etiqueta_arrastada(ui, self.id.with("etiqueta"), aba, a.ponteiro);
        }

        let fileira_resp = ui.interact(fileira, self.id.with("fileira"), Sense::hover());
        fileira_resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, self.rotulo));
        ui.ctx().accesskit_node_builder(fileira_resp.id, |no| no.set_role(Role::TabList));

        resposta.acoes = Rect::from_min_max(
            pos2((fim_das_abas + ESPACO_ABAS).min(direita), base_botoes - LADO_CABECALHO),
            pos2(direita, base_botoes),
        );
        ui.allocate_rect(rect, Sense::hover());
        resposta
    }
}

/// Lê o arrasto da aba `aba` (a resposta dela, com `Sense::click_and_drag`): o egui só diz que ela está sendo
/// arrastada depois de o ponteiro andar um pouco (abaixo disso é um clique), e o Esc cancela até o botão ser solto.
fn ler_arrasto(ui: &Ui, resp: &egui::Response, cancelado: &mut bool, aba: usize) -> Option<Arrasto> {
    if !(resp.dragged() || resp.drag_stopped()) {
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        return None;
    }
    if resp.dragged() && ui.input(|inp| inp.key_pressed(Key::Escape)) {
        *cancelado = true;
    }
    let ponteiro = ui.input(|inp| inp.pointer.latest_pos().or(inp.pointer.interact_pos()))?;
    if *cancelado {
        return None;
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    Some(Arrasto { aba, ponteiro, solto: resp.drag_stopped() })
}

/// O ícone, o nome e o sub de uma aba com nome no retângulo dela.
fn pintar_nome(ui: &Ui, pintor: &egui::Painter, r: Rect, aba: &AbaDoca<'_>, cor: Color32, ativa: bool) {
    let p = paleta(ui);
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let x_icone = r.left() + 1.0 + 12.0;
    let icone = Rect::from_min_size(pos2(x_icone, r.center().y - LADO_ICONE / 2.0), vec2(LADO_ICONE, LADO_ICONE));
    icones::pintar(pintor, icone, aba.icone, cor);
    let x_texto = x_icone + LADO_ICONE + GAP;
    let espaco = r.right() - DEPOIS_DO_TEXTO - x_texto;
    let g_nome = galley_cortado(ui, aba.nome, fonte.clone(), cor, espaco);
    let largura_nome = g_nome.size().x;
    pintor.galley(pos2(x_texto, r.center().y - g_nome.size().y / 2.0), g_nome, cor);
    if let Some(s) = aba.sub {
        let resta = espaco - largura_nome - GAP;
        if resta >= 16.0 {
            let cor_sub = cor_sub_aba(p, ativa);
            let g = galley_cortado(ui, s, fonte, cor_sub, resta);
            pintor.galley(pos2(x_texto + largura_nome + GAP, r.center().y - g.size().y / 2.0), g, cor_sub);
        }
    }
}

/// A aba de um painel na janela própria dele (prancheta 6: o cabeçalho da janela com a aba ativa encostada embaixo,
/// `padding-left:12px`): o ícone, o nome e o sub, que se arrasta de volta para a janela principal. `base` é o canto
/// de baixo à esquerda da aba (o fim do cabeçalho). Devolve a largura usada e o arrasto, se há um.
pub fn aba_da_janela(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    aba: &AbaDoca<'_>,
    base: Pos2,
) -> (f32, Option<Arrasto>) {
    let p = paleta(ui);
    let id = Id::new(("databox::aba_da_janela", id_salt));
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let mut texto = galley(ui, aba.nome, fonte.clone(), Color32::PLACEHOLDER).size().x;
    if let Some(s) = aba.sub {
        texto += GAP + galley(ui, s, fonte, Color32::PLACEHOLDER).size().x;
    }
    let largura = (ANTES_DO_TEXTO + texto + DEPOIS_DO_TEXTO).ceil();
    let r = Rect::from_min_size(pos2(base.x, base.y - ALTURA_ABA), vec2(largura, ALTURA_ABA));
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let id_cancelado = id.with("cancelado");
    let mut cancelado: bool = ui.ctx().data(|d| d.get_temp(id_cancelado)).unwrap_or(false);
    let arrasto = ler_arrasto(ui, &resp, &mut cancelado, 0);
    let cancelado = cancelado && (resp.dragged() || resp.drag_stopped()) && !ui.input(|i| i.pointer.any_released());
    ui.ctx().data_mut(|d| d.insert_temp(id_cancelado, cancelado));
    pintar_aba_ativa(ui.painter(), p, r);
    pintar_nome(ui, ui.painter(), r, aba, p.estudio.t, true);
    if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
        pintura::anel_foco(&pintor_de_fora(ui, r), p, r, RAIO_ABA);
    }
    let rotulo = aba.rotulo();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), true, &rotulo));
    ui.ctx().accesskit_node_builder(resp.id, |no| {
        no.set_role(Role::Tab);
        no.set_selected(true);
    });
    (largura, arrasto)
}

/// Pinta a superfície do painel (o `.vidro`: sombra, `--painel`, borda `--vidrob` e o realce) com a aba ativa
/// encaixada em cima: na `juncao` o fio de cima some, e com a junção encostada na quina esquerda o canto de cima à
/// esquerda fica reto (`border-top-left-radius:0`).
pub fn pintar_superficie(painter: &egui::Painter, p: &Paleta, rect: Rect, juncao: Option<Rangef>) {
    let e = &p.estudio;
    let mut cantos = pintura::canto(rect, RAIO_CARD);
    if juncao.is_some_and(|j| j.min <= rect.left() + 0.5) {
        cantos.nw = 0;
    }
    painter.add(Shape::Vec(vec![
        pintura::forma_sombra(rect, RAIO_CARD, e.sombra_painel),
        RectShape::filled(rect, cantos, e.painel).into(),
        RectShape::stroke(rect, cantos, Stroke::new(1.0, e.vidrob), StrokeKind::Inside).into(),
        forma_realce_curvo(rect.shrink(1.0), RAIO_CARD - 1.0, e.vidroluz),
    ]));
    if let Some(j) = juncao {
        // por dentro das bordas laterais da aba, na cor do pé dela (o meio do degradê da aba ativa), cobrindo o fio e
        // o realce
        let x = Rangef::new(j.min + 1.0, j.max - 1.0);
        if x.span() > 0.0 {
            let faixa = Rect::from_x_y_ranges(x, Rangef::new(rect.top(), rect.top() + 2.0));
            painter.rect_filled(faixa, CornerRadius::ZERO, e.sup_meio);
        }
    }
}

/// O fio `.sep-doca` antes das ações do painel na fileira: 1×18px em `--vidrob`, a 8px de baixo; `direita` é onde
/// começa a primeira ação.
pub fn pintar_sep(painter: &egui::Painter, p: &Paleta, direita: f32, base: f32) {
    let x = direita - MARGEM_SEP - 0.5;
    let y = Rangef::new(base - BAIXO_SEP - ALTURA_SEP, base - BAIXO_SEP);
    painter.vline(x, y, Stroke::new(1.0, p.estudio.vidrob));
}

/// A etiqueta da aba arrastada (prancheta 2b): a aba ativa solta, com o ícone e o nome, presa ao ponteiro por cima
/// de tudo.
pub fn pintar_etiqueta_arrastada(ui: &Ui, id: Id, aba: &AbaDoca<'_>, ponteiro: Pos2) {
    let p = paleta(ui);
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let g = galley(ui, &aba.rotulo(), fonte, p.estudio.t);
    let largura = ANTES_DO_TEXTO + g.size().x + DEPOIS_DO_TEXTO;
    let r = Rect::from_min_size(ponteiro + vec2(-18.0, -ALTURA_ABA / 2.0), vec2(largura, ALTURA_ABA));
    let pintor = ui.ctx().layer_painter(egui::LayerId::new(Order::Tooltip, id));
    pintor.add(pintura::forma_sombra(r, RAIO_ABA, p.estudio.sombra_painel));
    pintor.add(RectShape::filled(r, RAIO_ABA, p.estudio.painel));
    pintar_aba_ativa(&pintor, p, r);
    pintor.rect_stroke(r, RAIO_ABA, Stroke::new(1.0, p.estudio.vidrob), StrokeKind::Inside);
    let icone =
        Rect::from_min_size(pos2(r.left() + 13.0, r.center().y - LADO_ICONE / 2.0), vec2(LADO_ICONE, LADO_ICONE));
    icones::pintar(&pintor, icone, aba.icone, p.estudio.t);
    pintor.galley(pos2(icone.right() + GAP, r.center().y - g.size().y / 2.0), g, p.estudio.t);
}

/// Acende o lugar onde a aba arrastada vai cair (uma doca, a janela inteira): o fundo `--acc` translúcido e o
/// contorno `--acc`, por cima de tudo.
pub fn pintar_alvo(ui: &Ui, id: Id, rect: Rect) {
    let p = paleta(ui);
    let pintor = ui.ctx().layer_painter(egui::LayerId::new(Order::Foreground, id));
    pintor.rect_filled(rect, RAIO_CARD, p.estudio.acc.gamma_multiply(0.12));
    pintor.rect_stroke(rect, RAIO_CARD, Stroke::new(2.0, p.estudio.acc), StrokeKind::Inside);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn rotulo_da_aba_da_doca() {
        assert_eq!(AbaDoca::new(Icone::Explorador, "Explorador").rotulo(), "Explorador");
        let a = AbaDoca::new(Icone::Painel, "Propriedades").sub(Some("clientes"));
        assert_eq!(a.rotulo(), "Propriedades · clientes");
    }

    #[test]
    fn aba_so_icone_tem_38px() {
        // `.aba.so-ic{padding:0 10px}` com o ícone de 16 e a borda de 1px dos dois lados
        assert_eq!(LARGURA_SO_ICONE, 38.0);
    }
}
