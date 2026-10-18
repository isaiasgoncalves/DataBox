//! O menu da casca no visual Estúdio: o botão `.menu` (pílula de 24px, transparente; no hover e aberto, o volume
//! neutro com o halo de 14px) e o painel no vidro das camadas flutuantes (`.flut`, raio 12, `padding:5px`) com os
//! itens `.mi` de 28px, o atalho em `--t3` (`--t2` no item aceso, [`cor_atalho_menu`]) e os separadores `.msep`; no
//! [`PainelMenu`], os itens desligados
//! (`.mi.off`), os marcados e as opções com a coluna das marcas (`.ck`: o ✓ e o ponto em `--acct`), o item de duas
//! linhas dos recentes e o submenu (Exibir › Tema, Arquivo › Abertos recentemente). Com
//! [`PainelMenu::com_icones`], o `.menu-ic` do Arquivo: linhas de 36px com o sólido de 24 de cada item
//! ([`PainelMenu::icone`]).
//!
//! As outras peças da casca ficam em [`crate::pecas::barra_titulo`], [`crate::pecas::ferramentas`],
//! [`crate::pecas::abas_documento`], [`crate::pecas::arvore`], [`crate::pecas::painel`] e
//! [`crate::pecas::barra_status`]; a barra de ícones é o [`crate::pecas::botao::Botao::rail`]. A topbar e a lateral
//! da casca antiga saíram depois da junção com o `ajustes-raios`.

use egui::containers::menu::MenuState;
use egui::{
    Align, Color32, CursorIcon, EventFilter, FocusDirection, Id, Key, Layout, PointerButton, Popup, PopupCloseBehavior,
    Rect, Response, Sense, Shape, Stroke, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::botao::{EstadoVolume, Volume, pintar_volume, pintor_de_fora};
use crate::pecas::campos::{forma_vidro_fosco, pintar_item_menu};
use crate::pecas::texto::{galley, galley_cortado, galley_tit};
use crate::pintura;
use crate::tema::{Paleta, duracao_transicao, paleta};
// ------------------------------------------------------------------ menu

/// O que a seta do `summary.btn` antigo somava à largura do botão depois do texto (`gap:6px` + `margin-left:2px` +
/// a caixa de 5px). O `.menu` do Estúdio não tem seta; fica para quem desenha um gatilho com ela.
pub const LARGURA_SETA_MENU: f32 = 13.0;

/// Os três pontos (no centro dos traços de 1,5px) da seta de abrir de um gatilho: as bordas direita e de baixo de
/// uma caixa de 5px girada `graus` em volta do `centro` (45° = fechado, apontando para baixo; −135° = aberto). A
/// seta fica com ~7,5px de largura, como no navegador.
pub fn pontos_seta_menu(centro: egui::Pos2, graus: f32) -> Vec<egui::Pos2> {
    let (s, c) = graus.to_radians().sin_cos();
    // os traços ficam por dentro da caixa (5px com bordas de 1,5px): o meio deles a 1,75px do centro
    [vec2(1.75, -2.5), vec2(1.75, 1.75), vec2(-2.5, 1.75)]
        .into_iter()
        .map(|v| centro + vec2(v.x * c - v.y * s, v.x * s + v.y * c))
        .collect()
}

/// Altura do botão do menu (`.menu{height:24px}`).
pub const ALTURA_MENU: f32 = 24.0;
/// `padding:0 10px` do `.menu`, mais a borda de 1px.
const RECUO_MENU: f32 = 11.0;
/// Largura do painel do menu (o menu Arquivo da prancheta do Início, `width:286px`).
pub const LARGURA_PAINEL_MENU: f32 = 286.0;
/// A borda de 1px e o `padding:5px` do vidro.
const RECUO_PAINEL: f32 = 6.0;
/// Raio do vidro dos menus.
const RAIO_PAINEL: f32 = 12.0;
/// Altura de um item (`.mi{height:28px}`).
const ALTURA_ITEM_MENU: f32 = 28.0;

/// A cor do atalho do item (`.at`): `--t3`; no item aceso (hover, foco, submenu aberto: o fundo `--sel`), o `--t2`
/// (o terciário cairia abaixo de 4,5:1 sobre a seleção; revisão de 15/10/2026, item 1).
pub fn cor_atalho_menu(p: &Paleta, aceso: bool) -> Color32 {
    if aceso { p.estudio.t2 } else { p.estudio.t3 }
}

/// O volume do `.menu:hover` / `.menu.on`: o neutro sem a sombra interna de baixo nem o brilho, com o halo de 14px
/// em `--brilho` sem o anel.
fn volume_menu(p: &Paleta) -> Volume {
    let mut v = Volume::neutro(p).sem_sombra_baixo().halo(14.0, p.estudio.brilho, false);
    v.clarear_hover = 1.0;
    v
}

/// O menu: o botão `.menu` (pílula de 24px, `padding:0 10px`, 12px em `--t2`, transparente; no hover e aberto, o
/// texto em `--t` e o volume neutro com o halo de 14px em `--brilho`) e o painel no vidro das camadas flutuantes
/// (`.flut`: 286px, raio 12, `padding:5px`), 3px abaixo do botão e alinhado à esquerda dele (à direita quando não
/// cabe). Pelo teclado: com o menu aberto e o foco no botão, ↓ vai ao primeiro item ligado e ↑ ao último; nos itens,
/// ↑ e ↓ andam pelos ligados, com volta, sem sair do painel; o Esc fecha o menu e devolve o foco ao botão.
#[must_use = "chame .mostrar(ui, |m| …)"]
pub struct Menu<'a> {
    id: Id,
    rotulo: &'a str,
    largura: f32,
}

impl<'a> Menu<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, rotulo: &'a str) -> Self {
        Menu { id: Id::new(("databox::menu", id_salt)), rotulo, largura: LARGURA_PAINEL_MENU }
    }

    /// A largura do painel (o padrão é [`LARGURA_PAINEL_MENU`]; o Exibir das pranchetas tem 262px).
    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = largura;
        self
    }

    /// O menu está aberto?
    pub fn aberto(&self, ui: &Ui) -> bool {
        Popup::is_id_open(ui.ctx(), self.id.with("painel"))
    }

    /// Fecha o menu (clicar num item fecha; os botões de tema não).
    pub fn fechar(ui: &Ui, id_salt: impl std::hash::Hash + std::fmt::Debug) {
        Popup::close_id(ui.ctx(), Id::new(("databox::menu", id_salt)).with("painel"));
    }

    pub fn mostrar<R>(self, ui: &mut Ui, conteudo: impl FnOnce(&mut PainelMenu<'_>) -> R) -> (Response, Option<R>) {
        let p = paleta(ui);
        let e = &p.estudio;
        let id_painel = self.id.with("painel");
        let aberto = Popup::is_id_open(ui.ctx(), id_painel);
        let fonte = fontes::fonte(12.0, Peso::Normal);
        let largura = (galley(ui, self.rotulo, fonte.clone(), Color32::PLACEHOLDER).size().x + 2.0 * RECUO_MENU).ceil();
        let (rect, resp) = ui.allocate_exact_size(vec2(largura, ALTURA_MENU), Sense::click());
        let foco = resp.has_focus() && pintura::foco_visivel(ui.ctx());
        let realce = ui.ctx().animate_bool_with_time(
            resp.id.with("realce"),
            resp.hovered() || aberto || foco,
            duracao_transicao(ui),
        );
        if ui.is_rect_visible(rect.expand(24.0)) {
            let painter = ui.painter();
            let fora = pintor_de_fora(ui, rect);
            let estado = EstadoVolume { presenca: realce, hover: realce, pressao: 0.0 };
            pintar_volume(painter, &fora, p, rect, f32::INFINITY, &volume_menu(p), estado);
            let cor = pintura::misturar(e.t2, e.t, realce);
            let g = galley(ui, self.rotulo, fonte, cor);
            painter.galley(rect.center() - g.size() / 2.0, g, cor);
            if foco {
                pintura::anel_foco(&fora, p, rect, f32::INFINITY);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, self.rotulo));

        // o painel fica 2px à esquerda do botão (`left:34px` com o botão em 36px) e 3px abaixo dele
        let popup = Popup::new(id_painel, ui.ctx().clone(), rect.expand2(vec2(2.0, 0.0)), ui.layer_id())
            .open_memory(resp.clicked().then_some(egui::SetOpenCommand::Toggle))
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(egui::RectAlign::BOTTOM_START)
            .gap(3.0)
            .width(self.largura)
            .frame(egui::Frame::NONE);
        let mut resultado = None;
        let mut itens = Vec::new();
        let largura_painel = self.largura;
        popup.show(|ui| {
            let (r, fechar, ligados) = painel_no_vidro(ui, largura_painel, conteudo);
            resultado = Some(r);
            itens = ligados;
            if fechar {
                Popup::close_id(ui.ctx(), id_painel);
            }
        });
        let aberto_agora = Popup::is_id_open(ui.ctx(), id_painel);
        // com o menu aberto e o foco no botão, ↓ vai ao primeiro item ligado e ↑ ao último (o egui levaria o foco ao
        // que está mais perto, embaixo do painel)
        if aberto_agora && resp.has_focus() {
            let (cima, baixo) = ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
            let destino = if baixo {
                itens.first()
            } else if cima {
                itens.last()
            } else {
                None
            };
            if let Some(&id) = destino {
                ui.memory_mut(|m| {
                    m.request_focus(id);
                    m.move_focus(FocusDirection::None);
                });
            }
        }
        // o Esc fecha o menu e tira o foco de quem o tinha (o item): ele volta ao botão do menu
        if aberto && !aberto_agora && ui.input(|i| i.key_pressed(Key::Escape)) {
            resp.request_focus();
        }
        (resp, resultado)
    }
}

/// As setas ↑ e ↓ com o foco num item ligado do painel: o anterior e o seguinte, com volta (o primeiro depois do
/// último). O item focado trava as setas verticais, para o egui não levar o foco ao que está embaixo do menu (como o
/// `Select`); no primeiro quadro com o foco, a navegação que o egui já pediu é desfeita.
fn andar_pelos_itens(ui: &Ui, itens: &[Id]) {
    let Some(focado) = ui.memory(|m| m.focused()) else { return };
    let Some(atual) = itens.iter().position(|&id| id == focado) else { return };
    let (cima, baixo) = ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
    let n = itens.len();
    let destino = if baixo {
        Some((atual + 1) % n)
    } else if cima {
        Some((atual + n - 1) % n)
    } else {
        None
    };
    ui.memory_mut(|m| {
        m.set_focus_lock_filter(focado, EventFilter { vertical_arrows: true, ..Default::default() });
        if let Some(i) = destino {
            m.request_focus(itens[i]);
            m.move_focus(FocusDirection::None);
        }
    });
}

/// O painel no vidro das camadas flutuantes (`.flut`, raio 12, `padding:5px`) com os itens um embaixo do outro: o
/// vidro vai num lugar reservado antes deles e recebe o tamanho depois de medir. Trata as setas ↑ e ↓ entre os itens
/// ligados ([`andar_pelos_itens`]). Devolve o que o conteúdo devolveu, se algum item pediu para fechar e os ids dos
/// itens ligados, na ordem.
pub(crate) fn painel_no_vidro<R>(
    ui: &mut Ui,
    largura: f32,
    conteudo: impl FnOnce(&mut PainelMenu<'_>) -> R,
) -> (R, bool, Vec<Id>) {
    let p = paleta(ui);
    let topo = ui.cursor().min;
    let lugar_vidro = ui.painter().add(Shape::Noop);
    let largura_dentro = largura - 2.0 * RECUO_PAINEL;
    let interno = Rect::from_min_size(topo + vec2(RECUO_PAINEL, RECUO_PAINEL), vec2(largura_dentro, f32::INFINITY));
    let mut filho = ui.new_child(UiBuilder::new().max_rect(interno).layout(Layout::top_down_justified(Align::Min)));
    filho.spacing_mut().item_spacing.y = 0.0;
    filho.set_min_width(largura_dentro);
    let clip_halo = ui.clip_rect();
    let mut painel = PainelMenu {
        ui: &mut filho,
        fechar: false,
        clip_halo,
        marcas: false,
        icones: false,
        silhuetas: false,
        perigo: false,
        fraco: false,
        proximo_icone: None,
        ligados: Vec::new(),
    };
    let r = conteudo(&mut painel);
    let PainelMenu { fechar, ligados, .. } = painel;
    andar_pelos_itens(ui, &ligados);
    let altura = (filho.min_rect().height() + 2.0 * RECUO_PAINEL).round();
    let vidro = Rect::from_min_size(topo, vec2(largura, altura));
    ui.allocate_rect(vidro, Sense::hover());
    ui.painter().set(lugar_vidro, forma_vidro_fosco(ui.painter(), p, vidro, RAIO_PAINEL));
    (r, fechar, ligados)
}

/// O [`painel_no_vidro`] com no máximo `altura_max` de altura (o vidro inteiro): os itens rolam por dentro dele (a lista
/// das planilhas, `.menu-ctx.folhas-m{max-height:332px}`).
pub(crate) fn painel_no_vidro_rolavel<R>(
    ui: &mut Ui,
    id_salt: Id,
    largura: f32,
    altura_max: f32,
    conteudo: impl FnOnce(&mut PainelMenu<'_>) -> R,
) -> (R, bool, Vec<Id>) {
    let p = paleta(ui);
    let topo = ui.cursor().min;
    let lugar_vidro = ui.painter().add(Shape::Noop);
    let largura_dentro = largura - 2.0 * RECUO_PAINEL;
    let altura_dentro = (altura_max - 2.0 * RECUO_PAINEL).max(ALTURA_ITEM_MENU);
    let interno = Rect::from_min_size(topo + vec2(RECUO_PAINEL, RECUO_PAINEL), vec2(largura_dentro, altura_dentro));
    let mut filho = ui.new_child(UiBuilder::new().max_rect(interno).layout(Layout::top_down_justified(Align::Min)));
    let clip_halo = ui.clip_rect();
    let saida = egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .max_height(altura_dentro)
        .auto_shrink([false, true])
        .show(&mut filho, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_min_width(ui.available_width());
            let mut painel = PainelMenu {
                ui,
                fechar: false,
                clip_halo,
                marcas: false,
                icones: false,
                silhuetas: false,
                perigo: false,
                fraco: false,
                proximo_icone: None,
                ligados: Vec::new(),
            };
            let r = conteudo(&mut painel);
            let PainelMenu { fechar, ligados, .. } = painel;
            (r, fechar, ligados)
        });
    let (r, fechar, ligados) = saida.inner;
    andar_pelos_itens(ui, &ligados);
    let altura = (saida.inner_rect.height() + 2.0 * RECUO_PAINEL).round();
    let vidro = Rect::from_min_size(topo, vec2(largura, altura));
    ui.allocate_rect(vidro, Sense::hover());
    ui.painter().set(lugar_vidro, forma_vidro_fosco(ui.painter(), p, vidro, RAIO_PAINEL));
    (r, fechar, ligados)
}

/// A coluna das marcas dos itens (`.mi .ck`: 14px) e o `gap:10px` até o texto.
const COLUNA_MARCA: f32 = 14.0 + 10.0;
/// Altura do item de duas linhas (os recentes do Arquivo, `style="height:40px"`; 44px no menu com ícones).
const ALTURA_ITEM_DUPLO: f32 = 40.0;
const ALTURA_ITEM_DUPLO_ICONES: f32 = 44.0;
/// O menu com ícones (`.menu-ic`, o Arquivo): itens de 36px, `padding-left:8px`, a coluna do
/// ícone de 24px (`.mi-ic`, com o sólido de 24) e `gap:12px` até o texto.
pub const ALTURA_ITEM_MENU_ICONES: f32 = 36.0;
pub const LADO_ICONE_MENU: f32 = 24.0;
const RECUO_ICONES: f32 = 8.0;
const GAP_ICONES: f32 = 12.0;

/// O que vai na coluna das marcas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marca {
    Nenhuma,
    /// O ✓ (`menuitemcheckbox` marcado).
    Visto,
    /// O ponto (`menuitemradio` escolhido).
    Ponto,
}

/// O que vai à direita do item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direita<'t> {
    Nada,
    /// O atalho (`.at`: 11px, `--t3`).
    Atalho(&'t str),
    /// A seta do submenu (12px, `--t3`).
    Seta,
}

/// O conteúdo do painel do menu.
pub struct PainelMenu<'u> {
    ui: &'u mut Ui,
    fechar: bool,
    /// Até onde vai o halo dos itens (o vidro não corta).
    clip_halo: Rect,
    /// Os itens reservam a coluna das marcas ([`PainelMenu::com_marcas`]).
    marcas: bool,
    /// O menu com ícones ([`PainelMenu::com_icones`]).
    icones: bool,
    /// O menu das silhuetas de 14px ([`PainelMenu::com_silhuetas`]).
    silhuetas: bool,
    /// O item que está sendo pintado é o `.mi.perigo` ([`PainelMenu::item_perigo`]).
    perigo: bool,
    /// O item que está sendo pintado é o esmaecido ([`PainelMenu::item_marcado_fraco`]).
    fraco: bool,
    /// O ícone do próximo item ([`PainelMenu::icone`] e [`PainelMenu::silhueta`]).
    proximo_icone: Option<Icone>,
    /// Os ids dos itens ligados, na ordem (as setas ↑ e ↓ andam por eles).
    ligados: Vec<Id>,
}

impl PainelMenu<'_> {
    /// O separador `.msep`: fio de 1px `--vidrob` com `margin:5px 6px`.
    pub fn separador(&mut self) {
        let e = &paleta(self.ui).estudio;
        self.ui.add_space(5.0);
        let (rect, _) = self.ui.allocate_exact_size(vec2(self.ui.available_width(), 1.0), Sense::hover());
        self.ui.painter().hline(rect.left() + 6.0..=rect.right() - 6.0, rect.center().y, Stroke::new(1.0, e.vidrob));
        self.ui.add_space(5.0);
    }

    /// Daqui em diante, todos os itens reservam a coluna das marcas (`.ck`, 14px e o `gap:10px`), para alinhar com
    /// os [`PainelMenu::item_marcado`] e [`PainelMenu::item_opcao`] (o menu Exibir).
    pub fn com_marcas(&mut self) {
        self.marcas = true;
    }

    /// Daqui em diante, o menu com ícones (`.menu-ic`, o Arquivo da prancheta Inicio): itens de 36px
    /// ([`ALTURA_ITEM_MENU_ICONES`]) com a coluna do ícone de 24px ([`LADO_ICONE_MENU`]) e `gap:12px`, e os itens de
    /// duas linhas com 44px e o ícone de 24. Os itens sem [`PainelMenu::icone`] deixam a coluna vazia, alinhados.
    pub fn com_icones(&mut self) {
        self.icones = true;
    }

    /// O sólido do próximo item (no [`PainelMenu::com_icones`]): `p.icone(Icone::PastaSolida).item(…)`. No item
    /// desligado, o ícone fica a 45% e com o `grayscale(.7)` (`.mi.off .mi-ic`).
    pub fn icone(&mut self, icone: Icone) -> &mut Self {
        self.proximo_icone = Some(icone);
        self
    }

    /// Daqui em diante, o menu das silhuetas (o `.menu-ctx` do menu de contexto): cada item reserva a coluna de 14px
    /// da silhueta (`.mi .sil`, em `--t2`) e o `gap:10px` até o texto, tenha ele a [`PainelMenu::silhueta`] ou não
    /// (o "Abrir" da prancheta 15 leva só o espaço).
    pub fn com_silhuetas(&mut self) {
        self.silhuetas = true;
    }

    /// A silhueta do próximo item (no [`PainelMenu::com_silhuetas`]): `p.silhueta(Icone::CopiarCaminho).item(…)`.
    pub fn silhueta(&mut self, icone: Icone) -> &mut Self {
        self.proximo_icone = Some(icone);
        self
    }

    /// O item de perigo (`.mi.perigo`: o texto e a silhueta em `--perigo`), como [`PainelMenu::item`].
    pub fn item_perigo(&mut self, texto: &str, atalho: Option<&str>, separado: bool) -> Response {
        self.perigo = true;
        let resposta = self.item(texto, atalho, separado);
        self.perigo = false;
        resposta
    }

    fn altura_item(&self) -> f32 {
        if self.icones { ALTURA_ITEM_MENU_ICONES } else { ALTURA_ITEM_MENU }
    }

    /// O cabeçalho do painel: o nome em 12px / 600 e o `hint` embaixo em 11px `--t3` (`padding:6px 12px 7px`),
    /// com o separador `.msep` antes dos itens.
    pub fn cabecalho(&mut self, nome: &str, hint: &str) {
        let e = &paleta(self.ui).estudio;
        let largura = self.ui.available_width();
        let gn = galley_cortado(self.ui, nome, fontes::fonte(12.0, Peso::Medio), e.t, largura - 24.0);
        let gh = crate::pecas::texto::galley_quebrado(
            self.ui,
            hint,
            fontes::fonte(11.0, Peso::Normal),
            e.t3,
            largura - 24.0,
            1.45,
        );
        let altura = 6.0 + 17.0 + 1.0 + gh.tamanho().y + 7.0;
        let (rect, resp) = self.ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
        self.ui.painter().galley(pos2(rect.left() + 12.0, rect.top() + 6.0 + (17.0 - gn.size().y) / 2.0), gn, e.t);
        gh.pintar(self.ui.painter(), pos2(rect.left() + 12.0, rect.top() + 6.0 + 17.0 + 1.0), e.t3);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, nome));
        self.separador();
    }

    fn alocar(&mut self, altura: f32, habilitado: bool) -> (Rect, Response) {
        let largura = self.ui.available_width();
        let sense = if habilitado { Sense::click() } else { Sense::hover() };
        let (rect, resp) = self.ui.allocate_exact_size(vec2(largura, altura), sense);
        if habilitado {
            self.ligados.push(resp.id);
        }
        (rect, resp)
    }

    /// Pinta um `.mi` (28px, raio 6, `padding:0 10px 0 12px`, 12px em `--t`; no hover, no foco e aberto, o fundo
    /// `--sel` com o realce e o halo de 12px; desligado, `.mi.off`: `--t3` a 60%).
    #[allow(clippy::too_many_arguments)]
    fn pintar_item(
        &self,
        rect: Rect,
        resp: &Response,
        texto: &str,
        direita: Direita<'_>,
        marca: Marca,
        coluna: bool,
        habilitado: bool,
        aberto: bool,
        icone: Option<Icone>,
    ) {
        let p = paleta(self.ui);
        let e = &p.estudio;
        let painter = self.ui.painter();
        let foco = habilitado && resp.has_focus() && pintura::foco_visivel(self.ui.ctx());
        let aceso = habilitado && (resp.hovered() || foco || aberto);
        if aceso {
            pintar_item_menu(painter, self.clip_halo, p, rect);
        }
        if habilitado && resp.hovered() {
            self.ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let (mut cor, cor_fraca) =
            if habilitado { (e.t, e.t3) } else { (e.t3.gamma_multiply(0.6), e.t3.gamma_multiply(0.6)) };
        if self.perigo && habilitado {
            cor = e.perigo;
        }
        if self.fraco && habilitado {
            cor = e.t3;
        }
        let mut x = rect.left() + if self.icones { RECUO_ICONES } else { 12.0 };
        if self.silhuetas {
            if let Some(icone) = icone {
                let caixa = Rect::from_min_size(pos2(x, rect.center().y - 7.0), vec2(14.0, 14.0));
                // `.mi .sil{color:var(--t2)}`, `.mi.perigo .sil{color:var(--perigo)}` e `.mi.off{opacity:.6}`
                let mut pintor = painter.clone();
                if !habilitado {
                    pintor.multiply_opacity(0.6);
                }
                icones::pintar(&pintor, caixa, icone, if self.perigo { e.perigo } else { e.t2 });
            }
            x += COLUNA_MARCA;
        }
        if self.icones {
            if let Some(icone) = icone {
                let caixa = Rect::from_min_size(
                    pos2(x, rect.center().y - LADO_ICONE_MENU / 2.0),
                    vec2(LADO_ICONE_MENU, LADO_ICONE_MENU),
                );
                if habilitado {
                    icones::pintar(painter, caixa, icone, e.t2);
                } else {
                    // `.mi.off{opacity:.6}` e `.mi.off .mi-ic{opacity:.45;filter:grayscale(.7)}`
                    let mut apagado = painter.clone();
                    apagado.multiply_opacity(0.6 * 0.45);
                    icones::pintar_apagado(&apagado, caixa, icone, 0.7);
                }
            }
            x += LADO_ICONE_MENU + GAP_ICONES;
        }
        if coluna {
            let caixa = Rect::from_min_size(pos2(x, rect.center().y - 7.0), vec2(14.0, 14.0));
            match marca {
                Marca::Visto => icones::pintar(painter, caixa, Icone::Marcado, e.acct),
                // `circle r=4` na grade de 24 do `.ck`
                Marca::Ponto => {
                    painter.circle_filled(caixa.center(), 4.0 * 14.0 / 24.0, e.acct);
                }
                Marca::Nenhuma => {}
            }
            x += COLUNA_MARCA;
        }
        let largura_direita = match direita {
            Direita::Nada => 0.0,
            Direita::Atalho(a) => {
                let cor_at = if habilitado { cor_atalho_menu(p, aceso) } else { cor_fraca };
                let ga = galley(self.ui, a, fontes::fonte(11.0, Peso::Normal), cor_at);
                let largura = ga.size().x;
                painter.galley(pos2(rect.right() - 10.0 - largura, rect.center().y - ga.size().y / 2.0), ga, cor_at);
                largura + 10.0
            }
            Direita::Seta => {
                let seta =
                    Rect::from_min_size(pos2(rect.right() - 10.0 - 12.0, rect.center().y - 6.0), vec2(12.0, 12.0));
                icones::pintar(painter, seta, Icone::SetaDireita, cor_fraca);
                12.0 + 10.0
            }
        };
        let g = galley_cortado(
            self.ui,
            texto,
            fontes::fonte(12.0, if self.fraco { Peso::Italico } else { Peso::Normal }),
            cor,
            rect.right() - 10.0 - largura_direita - x,
        );
        painter.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, cor);
    }

    /// Um item (`.mi`: 28px, raio 6, `padding:0 10px 0 12px`, 12px em `--t`; no hover, o fundo `--sel` com o
    /// realce de 1px e o halo de 12px em `--brilho`) com o atalho à direita (`.at`: 11px, `--t3`). `separado` põe o
    /// `.msep` acima. Clicar fecha o menu.
    pub fn item(&mut self, texto: &str, atalho: Option<&str>, separado: bool) -> Response {
        if separado {
            self.separador();
        }
        let icone = self.proximo_icone.take();
        let (rect, resp) = self.alocar(self.altura_item(), true);
        let direita = atalho.map_or(Direita::Nada, Direita::Atalho);
        self.pintar_item(rect, &resp, texto, direita, Marca::Nenhuma, self.marcas, true, false, icone);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, texto));
        if resp.clicked() {
            self.fechar = true;
        }
        resp
    }

    /// Um item desligado (`.mi.off`: `--t3` a 60%, sem hover nem clique), com o atalho.
    pub fn item_desligado(&mut self, texto: &str, atalho: Option<&str>) -> Response {
        let icone = self.proximo_icone.take();
        let (rect, resp) = self.alocar(self.altura_item(), false);
        let direita = atalho.map_or(Direita::Nada, Direita::Atalho);
        self.pintar_item(rect, &resp, texto, direita, Marca::Nenhuma, self.marcas, false, false, icone);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, false, texto));
        resp
    }

    /// Um item de ligar e desligar (`menuitemcheckbox`): o ✓ em `--acct` na coluna das marcas quando `marcado`.
    /// Clicar fecha o menu; quem chama troca o estado.
    pub fn item_marcado(&mut self, texto: &str, atalho: Option<&str>, marcado: bool) -> Response {
        let icone = self.proximo_icone.take();
        let (rect, resp) = self.alocar(self.altura_item(), true);
        let marca = if marcado { Marca::Visto } else { Marca::Nenhuma };
        let direita = atalho.map_or(Direita::Nada, Direita::Atalho);
        self.pintar_item(rect, &resp, texto, direita, marca, true, true, false, icone);
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, marcado, texto));
        if resp.clicked() {
            self.fechar = true;
        }
        resp
    }

    /// O [`PainelMenu::item_marcado`] esmaecido (`.mi.oc`: o texto em `--t3` e itálico; o atalho fica como é): a planilha
    /// oculta na lista das planilhas, com "oculta" à direita.
    pub fn item_marcado_fraco(&mut self, texto: &str, atalho: Option<&str>, marcado: bool) -> Response {
        self.fraco = true;
        let resposta = self.item_marcado(texto, atalho, marcado);
        self.fraco = false;
        resposta
    }

    /// Uma opção de um grupo (`menuitemradio`, o Tema): o ponto em `--acct` na coluna das marcas quando escolhida.
    /// Clicar fecha o menu; quem chama troca a escolha.
    pub fn item_opcao(&mut self, texto: &str, escolhida: bool) -> Response {
        let icone = self.proximo_icone.take();
        let (rect, resp) = self.alocar(self.altura_item(), true);
        let marca = if escolhida { Marca::Ponto } else { Marca::Nenhuma };
        self.pintar_item(rect, &resp, texto, Direita::Nada, marca, true, true, false, icone);
        resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, escolhida, texto));
        if resp.clicked() {
            self.fechar = true;
        }
        resp
    }

    /// Um item de duas linhas de 40px (os Abertos recentemente): o ícone de 16px, o nome em 12px `--t` e o `sub`
    /// embaixo (o caminho: 10.5px / 350 em `--t3`), os dois cortados com "…". No [`PainelMenu::com_icones`], 44px com
    /// o ícone de 24 (`.sk.s24`). Clicar fecha o menu.
    pub fn item_duplo(&mut self, icone: Option<Icone>, nome: &str, sub: &str) -> Response {
        let altura = if self.icones { ALTURA_ITEM_DUPLO_ICONES } else { ALTURA_ITEM_DUPLO };
        let (rect, resp) = self.alocar(altura, true);
        let p = paleta(self.ui);
        let e = &p.estudio;
        let painter = self.ui.painter();
        let foco = resp.has_focus() && pintura::foco_visivel(self.ui.ctx());
        if resp.hovered() || foco {
            pintar_item_menu(painter, self.clip_halo, p, rect);
        }
        if resp.hovered() {
            self.ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let (recuo, lado, gap) =
            if self.icones { (RECUO_ICONES, LADO_ICONE_MENU, GAP_ICONES) } else { (12.0, 16.0, 10.0) };
        let mut x = rect.left() + recuo;
        if let Some(icone) = icone {
            icones::pintar(
                painter,
                Rect::from_min_size(pos2(x, rect.center().y - lado / 2.0), vec2(lado, lado)),
                icone,
                e.t2,
            );
            x += lado + gap;
        }
        let largura = (rect.right() - 10.0 - x).max(0.0);
        let gn = galley_cortado(self.ui, nome, fontes::fonte(12.0, Peso::Normal), e.t, largura);
        let gs = galley_cortado(self.ui, sub, fontes::fonte(10.5, Peso::Leve), e.t3, largura);
        let y = rect.center().y - (gn.size().y + gs.size().y) / 2.0;
        let altura_nome = gn.size().y;
        painter.galley(pos2(x, y), gn, e.t);
        painter.galley(pos2(x, y + altura_nome), gs, e.t3);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, nome));
        if resp.clicked() {
            self.fechar = true;
        }
        resp
    }

    /// Um submenu (`aria-haspopup`: Exibir › Tema, Arquivo › Abertos recentemente): o item com a seta de 12px à
    /// direita, que abre com o ponteiro em cima (ou no clique, ou no Enter) um painel de `largura` no mesmo vidro, 2px
    /// à direita dele e com o primeiro item na altura dele (`.mi.on` enquanto aberto). Ir para outro item fecha;
    /// clicar num item do submenu fecha o menu inteiro, e clicar fora dos dois também. Devolve o que o conteúdo
    /// devolveu, com o submenu aberto.
    pub fn submenu<R>(
        &mut self,
        texto: &str,
        largura: f32,
        conteudo: impl FnOnce(&mut PainelMenu<'_>) -> R,
    ) -> Option<R> {
        let ctx = self.ui.ctx().clone();
        let icone = self.proximo_icone.take();
        let (rect, resp) = self.alocar(self.altura_item(), true);
        let id_sub = resp.id.with("submenu");
        // o estado fica no do egui (o `MenuState` da raiz do menu): com o submenu aberto, o menu de cima não fecha
        // no clique dentro dele
        let (aberto_antes, id_raiz) = MenuState::from_ui(self.ui, |estado, pilha| (estado.open_item, pilha.id));
        let mut aberto = aberto_antes == Some(id_sub);
        let sobre = ctx.pointer_hover_pos().is_some_and(|pos| rect.contains(pos));
        let pelo_teclado = resp.clicked() && !resp.clicked_by(PointerButton::Primary);
        let mut mudar = None;
        if aberto && pelo_teclado {
            mudar = Some(false);
            aberto = false;
        } else if !aberto && (resp.clicked() || (sobre && aberto_antes.is_none())) {
            mudar = Some(true);
            aberto = true;
        }
        self.pintar_item(rect, &resp, texto, Direita::Seta, Marca::Nenhuma, self.marcas, true, aberto, icone);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, texto));
        ctx.accesskit_node_builder(resp.id, |no| no.set_expanded(aberto));
        let mut resultado = None;
        if aberto {
            // o vidro começa 6px acima do item (a borda e o `padding:5px`), e o primeiro item fica na altura dele
            let ancora = rect.expand2(vec2(0.0, RECUO_PAINEL));
            let saida = Popup::new(id_sub, ctx.clone(), ancora, self.ui.layer_id())
                .open(true)
                .align(egui::RectAlign::RIGHT_START)
                .gap(2.0)
                .width(largura)
                .frame(egui::Frame::NONE)
                .close_behavior(PopupCloseBehavior::IgnoreClicks)
                .show(|ui| painel_no_vidro(ui, largura, conteudo));
            if let Some(saida) = saida {
                let (r, fechar, _) = saida.inner;
                resultado = Some(r);
                let raiz = ctx.read_response(id_raiz);
                let clicou_fora = ctx.input(|i| i.pointer.any_click())
                    && saida.response.clicked_elsewhere()
                    && raiz.as_ref().is_none_or(|r| r.clicked_elsewhere());
                if fechar || clicou_fora {
                    self.fechar = true;
                    mudar = Some(false);
                }
                // o ponteiro foi para outro item do menu (sem estar a caminho do submenu): fecha
                let a_caminho = ctx.input(|i| i.pointer.is_moving_towards_rect(&saida.response.rect));
                if a_caminho {
                    ctx.request_repaint();
                }
                let no_menu = ctx.pointer_hover_pos().is_some_and(|pos| {
                    raiz.as_ref().is_some_and(|r| r.rect.contains(pos) && ctx.layer_id_at(pos) == Some(r.layer_id))
                });
                if !sobre && !saida.response.contains_pointer() && !a_caminho && no_menu {
                    mudar = Some(false);
                }
            }
        }
        if let Some(abrir) = mudar {
            MenuState::from_id(&ctx, id_raiz, |estado| estado.open_item = abrir.then_some(id_sub));
        }
        resultado
    }

    /// Título de um bloco do painel (o "Aparência"): o separador `.msep` e o rótulo `.tit` (11px / 600, caixa alta,
    /// `--t3`) com `padding:4px 12px 6px`.
    pub fn titulo(&mut self, texto: &str) {
        self.separador();
        let e = &paleta(self.ui).estudio;
        self.ui.add_space(4.0);
        let g = galley_tit(self.ui, texto, e.t3);
        let (rect, resp) = self.ui.allocate_exact_size(vec2(self.ui.available_width(), 15.0), Sense::hover());
        self.ui.painter().galley(pos2(rect.left() + 12.0, rect.top() + (15.0 - g.size().y) / 2.0), g, e.t3);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, texto));
        self.ui.add_space(6.0);
    }

    /// Conteúdo livre (o seletor de tema, que não fecha o menu).
    pub fn livre<R>(&mut self, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
        conteudo(self.ui)
    }

    /// Fecha o menu depois deste quadro.
    pub fn fechar(&mut self) {
        self.fechar = true;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn seta_do_menu_tem_7_e_meio_px_e_aponta_para_baixo_fechada() {
        let c = pos2(100.0, 50.0);
        let fechada = pontos_seta_menu(c, 45.0);
        let xs: Vec<f32> = fechada.iter().map(|p| p.x).collect();
        let largura = xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min);
        // ~6px entre os meios dos traços + 1,5px do traço: os ~7,5px medidos no navegador
        assert!((largura + 1.5 - 7.5).abs() < 0.1, "{largura}");
        // a ponta (o canto da caixa) fica embaixo, centrada
        assert!((fechada[1].x - c.x).abs() < 0.01 && fechada[1].y > c.y + 2.0);
        // aberta (−135°): a ponta em cima
        let aberta = pontos_seta_menu(c, -135.0);
        assert!((aberta[1].x - c.x).abs() < 0.01 && aberta[1].y < c.y - 2.0);
    }

    #[test]
    fn volume_do_menu_e_o_do_css() {
        use crate::tema::{CLARA, ESCURA};
        for p in [&CLARA, &ESCURA] {
            // `.menu:hover`: `inset 0 1px 0 --btluz, --btsomb, 0 0 14px --brilho` (sem `inset 0 -2px 3px`, sem o
            // anel de 1px e sem `brightness`)
            let v = volume_menu(p);
            assert_eq!((v.realce, v.sombras, v.borda), (p.estudio.btluz, p.estudio.btsomb, p.estudio.btb));
            assert_eq!((v.halo_desfoque, v.halo_cor, v.anel), (14.0, p.estudio.brilho, false));
            assert_eq!((v.sombra_baixo, v.clarear_hover), (Color32::TRANSPARENT, 1.0));
        }
    }
}
