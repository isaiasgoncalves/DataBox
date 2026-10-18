//! Campos do visual Estúdio (o `.campo` das pranchetas, `design/canvas/Main.dc.html`; as caixas de marcar e as
//! opções em `Dados.dc.html`):
//!
//! - [`campo`]: o rótulo no estilo `.tit` (11px / 600, caixa alta, `.06em`, `--t3`) com o "i" mini opcional, 6px e
//!   o controle; [`rotulo_campo`] é só o rótulo.
//! - [`Entrada`]: retângulo de 28px ([`ALTURA_CAMPO`]; 30px na busca, [`Entrada::busca`], com a lupa e o `.kbd` de
//!   [`Entrada::atalho`]) com os cantos de raio 10 ([`RAIO_CAMPO`], o raio único das superfícies), fundo `--f`,
//!   borda de 1px `--fb`, `padding:0 12px`, texto 12px, em baixo-relevo (sombra interna em cima, luz de 1px
//!   embaixo); no hover a borda puxa para o destaque; no foco, borda e anel de 1px no destaque e o halo de 14px em
//!   `--brilho`. [`Entrada::compacta`] é o campo da edição no lugar da grade, discreto: raio 4, transparente até o
//!   hover e com o anel azul no foco.
//! - [`EntradaSugestoes`] (o `input` com `datalist`) e [`AreaTexto`] (o mesmo baixo-relevo com o mesmo raio 10;
//!   [`AreaTexto::codigo`] é o editor de SQL, com as cores e a fonte do código, o recuo de dois espaços no Tab e,
//!   se pedidos, os números das linhas, as linhas marcadas e o sublinhado ondulado).
//! - [`Select`]: o `.campo` de escolher (o mesmo retângulo de raio 10, com a seta de traço em `--t3`); a lista abre
//!   no vidro dos menus ([`lista_opcoes`]: `.flut` com os itens `.mi`). [`pintar_vidro_flutuante`] e
//!   [`pintar_item_menu`] são esse vidro e esse item para outras peças.
//! - [`check`] e [`Check`]: caixas quadradas de 16px com cantos de 5px ([`RAIO_CAIXA`]) e o volume dos botões;
//!   marcadas, com o degradê do destaque e o ✓ (ou o traço do indeterminado) branco. [`radio`]: o círculo de 16px
//!   com o mesmo volume e o ponto branco.
//! - [`campos_duas_colunas`] (duas colunas com 12px entre elas) e [`pintar_salvo`] (o verde que a célula pisca
//!   depois de gravar).

use std::f32::consts::{FRAC_PI_2, PI};
use std::ops::Range;

use egui::epaint::{Mesh, PathShape, PathStroke, RectShape};
use egui::layers::ShapeIdx;
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::text_edit::TextEditState;
use egui::{
    Align, Color32, CursorIcon, Event, EventFilter, FontId, Galley, Id, Key, Layout, Margin, Modifiers, Painter, Popup,
    PopupCloseBehavior, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, UiBuilder,
    Widget, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::codigo::CoresCodigo;
use crate::fontes::{self, Peso};
use crate::icones::{Icone, TipoColuna};
use crate::pecas::ajuda::ajuda_mini;
use crate::pecas::botao::{EstadoVolume, Volume, forma_realce_curvo, pintar_volume, pintor_de_fora, realce_curvo};
use crate::pecas::texto::{galley, galley_cortado, galley_tit};
use crate::pintura::{self, Preenchimento};
use crate::tema::{Estudio, Paleta, SombraCss, duracao_transicao, movimento_reduzido_ctx, paleta, rgba};

/// Altura dos campos (`.campo{height:28px}`, a mesma dos botões).
pub const ALTURA_CAMPO: f32 = 28.0;

/// Altura do campo pequeno (`.campo.pq{height:26px}`), a única dos campos de 26: o [`Entrada::pequeno`], o
/// [`EntradaSugestoes::pequeno`] e o [`Select::pequeno`] (as linhas de coluna da Nova tabela, o "100 por página" dos
/// Dados), com a mesma moldura do campo de 28px, e o [`Entrada::compacta`] da edição no lugar da grade (a linha de
/// 16px, o `padding:4px` e 1px de borda: cabe na linha de 28px), com a moldura dele.
pub const ALTURA_CAMPO_PEQUENO: f32 = 26.0;

/// A largura mínima de um campo ([`Entrada`], [`AreaTexto`]) num lugar estreito demais.
const LARGURA_MINIMA_CAMPO: f32 = 24.0;

/// Altura da busca (`.busca{height:30px}`); ver [`Entrada::busca`].
pub const ALTURA_BUSCA: f32 = 30.0;

/// `padding:0 12px` do `.campo` mais a borda de 1px: onde o texto começa.
const RECUO_TEXTO: f32 = 13.0;

/// A lupa da busca (`.sil` de 14px).
const LADO_LUPA: f32 = 14.0;

/// A pastilha de tipo antes do texto (`.sk.s16` com `gap:8px`, o Tipo das janelas de coluna).
const LADO_PASTILHA: f32 = 16.0;
const GAP_PASTILHA: f32 = 8.0;

/// Raio dos campos ([`Entrada`], busca, [`Select`], [`EntradaSugestoes`] e [`AreaTexto`]: `border-radius:10px`):
/// o raio único das superfícies, [`pintura::RAIO_PADRAO`]. O [`Entrada::compacta`] tem 4.
pub const RAIO_CAMPO: f32 = pintura::RAIO_PADRAO;

/// Raio do campo compacto da edição no lugar ([`Entrada::compacta`]).
const RAIO_COMPACTO: f32 = 4.0;

/// Um campo com rótulo: o rótulo [`rotulo_campo`] (`.tit`), 6px (o `margin-bottom:6px` do "Ou digite o caminho" do
/// Início) e o controle. Ocupa a largura disponível.
pub fn campo<R>(ui: &mut Ui, rotulo: &str, ajuda: Option<&str>, controle: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        rotulo_campo(ui, rotulo, ajuda);
        controle(ui)
    })
    .inner
}

/// [`campo`] sem o "i": a explicação vai na dica do rótulo (`on_hover_text`), como nas janelas das pranchetas 12 (que não
/// têm o "i" ao lado dos rótulos).
pub fn campo_com_dica<R>(ui: &mut Ui, rotulo: &str, dica: &str, controle: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        rotulo_campo(ui, rotulo, None).on_hover_text(dica);
        controle(ui)
    })
    .inner
}

/// Só o rótulo de um campo, no estilo `.tit` (11px / 600, caixa alta, `.06em`, `--t3`), com o "i" mini opcional
/// a 6px (como o "Opções" da janela Exportar).
pub fn rotulo_campo(ui: &mut Ui, rotulo: &str, ajuda: Option<&str>) -> Response {
    crate::pecas::arranjo::linha(ui, 15.0, 6.0, |ui| {
        let p = paleta(ui);
        let g = galley_tit(ui, rotulo, p.faint);
        let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(rect.min, g, p.faint);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, rotulo));
        if let Some(a) = ajuda {
            ajuda_mini(ui, a);
        }
        resp
    })
}

/// Os campos de uma janela em duas colunas com 12px entre elas. `largo` ocupa a linha inteira.
pub fn campos_duas_colunas(ui: &mut Ui, conteudo: impl FnOnce(&mut Campos<'_>)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 12.0;
        let mut c = Campos { ui };
        conteudo(&mut c);
    });
}

/// Linhas do [`campos_duas_colunas`].
pub struct Campos<'u> {
    ui: &'u mut Ui,
}

impl Campos<'_> {
    /// Dois campos lado a lado.
    pub fn par(&mut self, a: impl FnOnce(&mut Ui), b: impl FnOnce(&mut Ui)) {
        crate::pecas::arranjo::duas_colunas(self.ui, 12.0, a, b);
    }

    /// Um campo só, na coluna da esquerda.
    pub fn um(&mut self, a: impl FnOnce(&mut Ui)) {
        crate::pecas::arranjo::duas_colunas(self.ui, 12.0, a, |_| {});
    }

    /// Um campo na linha inteira.
    pub fn largo(&mut self, a: impl FnOnce(&mut Ui)) {
        a(self.ui);
    }

    pub fn ui(&mut self) -> &mut Ui {
        self.ui
    }
}

// ------------------------------------------------------------------ moldura de campo

/// Como a moldura está, de 0 a 1 (as transições de 200ms do `.campo`: `box-shadow` e `border-color`).
#[derive(Debug, Clone, Copy, Default)]
struct EstadoCampo {
    hover: f32,
    foco: f32,
    /// O campo desligado fica com a borda `--fb` (a `--fbc` é só dos campos que aceitam texto).
    desligado: bool,
}

impl EstadoCampo {
    /// Anima o hover e o foco da peça `id` (desligado quando o `Ui` está desligado).
    fn animar(ui: &Ui, id: Id, hover: bool, foco: bool) -> Self {
        let tempo = duracao_transicao(ui);
        EstadoCampo {
            hover: ui.ctx().animate_bool_with_time(id.with("hover"), hover, tempo),
            foco: ui.ctx().animate_bool_with_time(id.with("foco"), foco, tempo),
            desligado: !ui.is_enabled(),
        }
    }

    /// Desligado também pela peça (o `habilitado(false)` do campo, num `Ui` ligado).
    fn desligado_se(mut self, desligado: bool) -> Self {
        self.desligado |= desligado;
        self
    }
}

/// Os lugares da moldura, reservados antes do texto: o do que fica por fora (luz embaixo, anel e halo do foco) e
/// o do fundo com a borda.
struct LugarMoldura {
    fora: ShapeIdx,
    fundo: ShapeIdx,
}

impl LugarMoldura {
    fn reservar(ui: &Ui) -> Self {
        LugarMoldura { fora: ui.painter().add(Shape::Noop), fundo: ui.painter().add(Shape::Noop) }
    }
}

/// A luz de 1px embaixo, por fora (`0 1px 0 cor`, a luz do baixo-relevo do `.campo`): a meia-lua de baixo que a
/// sombra deslocada 1px deixa aparecer, cheia no fundo e sumindo nas laterais.
fn forma_luz_embaixo(rect: Rect, r: f32, cor: Color32) -> Shape {
    if cor.a() == 0 || !rect.is_positive() {
        return Shape::Noop;
    }
    let rr = pintura::raio(rect, r);
    let dir = pos2(rect.right() - rr, rect.bottom() - rr);
    let esq = pos2(rect.left() + rr, rect.bottom() - rr);
    let mut pontos = Vec::with_capacity(36);
    if rr >= 1.0 {
        // o meio do fio fica 0,5px por fora do contorno
        let rl = rr + 0.5;
        let n = ((rl * FRAC_PI_2) / 1.5).ceil().clamp(3.0, 16.0) as usize;
        for i in 0..=n {
            let a = FRAC_PI_2 * i as f32 / n as f32;
            pontos.push(dir + vec2(a.cos(), a.sin()) * rl);
        }
        // num círculo os dois centros coincidem: não repetir o ponto de baixo
        for i in usize::from((dir.x - esq.x) < 0.01)..=n {
            let a = FRAC_PI_2 + FRAC_PI_2 * i as f32 / n as f32;
            pontos.push(esq + vec2(a.cos(), a.sin()) * rl);
        }
    } else {
        pontos.extend([pos2(rect.right(), rect.bottom() + 0.5), pos2(rect.left(), rect.bottom() + 0.5)]);
    }
    let traco = PathStroke::new_uv(1.0, move |_, pos| {
        let centro = if pos.x > dir.x {
            dir
        } else if pos.x < esq.x {
            esq
        } else {
            return cor;
        };
        let d = pos - centro;
        let comprimento = d.length();
        let f = if comprimento > 0.01 { (d.y / comprimento).clamp(0.0, 1.0) } else { 1.0 };
        cor.gamma_multiply(f)
    });
    PathShape::line(pontos, traco).into()
}

/// A cor da borda do `.campo` com `hover` e `foco` de 0 a 1: `--fbc` (3:1 contra o fundo; revisão de 15/10/2026,
/// item 2), puxando para o `fbc_hover` no hover e para o destaque no foco. O campo compacto da grade e os campos
/// desligados continuam com o `--fb`.
pub fn cor_borda_campo(es: &Estudio, hover: f32, foco: f32) -> Color32 {
    pintura::misturar(pintura::misturar(es.fbc, es.fbc_hover, hover), es.acc, foco)
}

/// Pinta o `.campo` em baixo-relevo: fundo `--f`, borda [`cor_borda_campo`], a luz de 1px embaixo (`0 1px 0
/// --vidroluz`, que some no foco), o anel de 1px e o halo de 14px do foco (`0 0 0 1px --acc, 0 0 14px --brilho`) e
/// a sombra interna em cima (`inset 0 1px 2px`). O fundo e o que fica por fora vão nos lugares reservados antes do
/// texto; a sombra interna (3px no alto, longe do texto) vai por cima.
fn pintar_moldura(ui: &Ui, lugar: &LugarMoldura, rect: Rect, r: f32, e: EstadoCampo) {
    pintar_moldura_com_fundo(ui, lugar, rect, r, e, paleta(ui).estudio.f);
}

/// O [`pintar_moldura`] com outro fundo (o do esquema das cores do código, no editor de SQL).
fn pintar_moldura_com_fundo(ui: &Ui, lugar: &LugarMoldura, rect: Rect, r: f32, e: EstadoCampo, fundo: Color32) {
    let p = paleta(ui);
    let es = &p.estudio;
    let cantos = pintura::canto(rect, r);
    let borda = if e.desligado { es.fb } else { cor_borda_campo(es, e.hover, e.foco) };
    ui.painter().set(
        lugar.fundo,
        Shape::Vec(vec![
            RectShape::filled(rect, cantos, fundo).into(),
            RectShape::stroke(rect, cantos, Stroke::new(1.0, borda), StrokeKind::Inside).into(),
        ]),
    );
    // `.est.claro .campo` troca a luz pela de .9
    let luz = if p.escuro { es.vidroluz } else { rgba(255, 255, 255, 0.9) };
    let mut fora = vec![forma_luz_embaixo(rect, r, luz.gamma_multiply(1.0 - e.foco))];
    if e.foco > 0.0 {
        let halo = SombraCss { y: 0.0, desfoque: 14.0, espalhar: 0.0, cor: es.brilho.gamma_multiply(e.foco) };
        fora.push(pintura::forma_sombra(rect, r, halo));
        let anel = Stroke::new(1.0, es.acc.gamma_multiply(e.foco));
        fora.push(RectShape::stroke(rect, cantos, anel, StrokeKind::Outside).into());
    }
    pintor_de_fora(ui, rect).set(lugar.fora, Shape::Vec(fora));
    // `inset 0 1px 2px`: `campo_inset` (.3 no escuro, .08 no claro); a regra do foco põe o .3 nos dois temas
    let sombra = pintura::misturar(es.campo_inset, rgba(0, 0, 0, 0.3), e.foco);
    pintura::sombra_interna_topo(ui.painter(), rect.shrink(1.0), r - 1.0, 3.0, sombra);
}

/// O `.campo.so-leitura`: fundo `--p`, borda `--fb`, sem a luz embaixo, sem a sombra interna e sem o foco.
fn pintar_moldura_so_leitura(ui: &Ui, lugar: &LugarMoldura, rect: Rect, r: f32) {
    let es = &paleta(ui).estudio;
    let cantos = pintura::canto(rect, r);
    ui.painter().set(
        lugar.fundo,
        Shape::Vec(vec![
            RectShape::filled(rect, cantos, es.faixa).into(),
            RectShape::stroke(rect, cantos, Stroke::new(1.0, es.fb), StrokeKind::Inside).into(),
        ]),
    );
}

fn fonte_campo(mono: bool, tamanho: Option<f32>) -> FontId {
    // o `.mono` do Estúdio é a fonte da interface em peso 350 (`.mono{font-family:inherit;font-weight:350}`)
    fontes::fonte(tamanho.unwrap_or(12.0), if mono { Peso::Leve } else { Peso::Normal })
}

/// O campo compacto da edição no lugar (raio 4): transparente até o hover (borda `--fb`); com o foco, fundo `--f`,
/// borda e anel de 1px no destaque, sem o halo (a célula ativa já tem o contorno dela).
fn pintar_moldura_compacta(ui: &Ui, lugar: &LugarMoldura, rect: Rect, e: EstadoCampo) {
    let es = &paleta(ui).estudio;
    let r = RAIO_COMPACTO;
    let borda = pintura::misturar(es.fb.gamma_multiply(e.hover), es.acc, e.foco);
    ui.painter().set(
        lugar.fundo,
        Shape::Vec(vec![
            Shape::rect_filled(rect, r, es.f.gamma_multiply(e.foco)),
            Shape::rect_stroke(rect, r, Stroke::new(1.0, borda), StrokeKind::Inside),
        ]),
    );
    let anel = Stroke::new(1.0, es.acc.gamma_multiply(e.foco));
    pintor_de_fora(ui, rect).set(lugar.fora, Shape::rect_stroke(rect, r, anel, StrokeKind::Outside));
}

/// Duração do verde que a célula pisca depois de gravar.
pub const DURACAO_SALVO: f64 = 1.4;

/// O verde que a célula (ou o campo) pisca depois de gravar: fundo `--ok` a 18% com o anel dos botões,
/// desbotando até transparente em 1,4s (`ease-out`), a partir de `desde` (o `ctx.input(|i| i.time)` da resposta do
/// servidor). Pede repintura enquanto pisca; devolve `false` quando acabou (com movimento reduzido não pisca).
pub fn pintar_salvo(painter: &Painter, p: &Paleta, rect: Rect, raio: f32, desde: f64) -> bool {
    let ctx = painter.ctx();
    if movimento_reduzido_ctx(ctx) {
        return false;
    }
    let t = ctx.input(|i| i.time) - desde;
    if !(0.0..DURACAO_SALVO).contains(&t) {
        return false;
    }
    // `ease-out` = cubic-bezier(0, 0, .58, 1); a curva fica bem perto de 1 − (1 − x)²
    let x = (t / DURACAO_SALVO) as f32;
    let progresso = 1.0 - (1.0 - x) * (1.0 - x);
    let resta = (1.0 - progresso).clamp(0.0, 1.0);
    painter.rect_filled(rect, raio, p.aqua_ok_bg.gamma_multiply(resta));
    let anel = &p.anel_aqua;
    let fio = |c: Color32| c.gamma_multiply(resta);
    let mut sombra = anel.sombra;
    sombra.cor = fio(sombra.cor);
    pintura::sombra(painter, rect, raio, sombra);
    pintura::contorno_segmentado(painter, rect, raio, fio(anel.topo), fio(anel.lados), fio(anel.base));
    ctx.request_repaint();
    true
}

/// Campo de texto de uma linha (o `.campo` com `input`). Use com
/// `ui.add(Entrada::new(&mut texto).dica("C:\\pasta\\arquivo.db"))`.
///
/// Numa [`crate::pecas::janela::Janela`], Enter no campo (sem Shift/Ctrl/Alt) é o envio do formulário:
/// [`crate::pecas::janela::RespostaJanela::enviou`].
#[must_use = "use ui.add(entrada)"]
pub struct Entrada<'a> {
    texto: &'a mut String,
    dica: Option<&'a str>,
    mono: bool,
    compacta: bool,
    busca: bool,
    cor: Option<Color32>,
    largura: Option<f32>,
    id_salt: Option<Id>,
    senha: bool,
    alinhar_direita: bool,
    rotulo_acessivel: Option<&'a str>,
    habilitado: bool,
    atalho: Option<&'a str>,
    pastilha: Option<TipoColuna>,
    somente_leitura: bool,
    pequeno: bool,
    fixo: bool,
    dica_italica: bool,
}

impl<'a> Entrada<'a> {
    pub fn new(texto: &'a mut String) -> Self {
        Entrada {
            texto,
            dica: None,
            mono: false,
            compacta: false,
            busca: false,
            cor: None,
            largura: None,
            id_salt: None,
            senha: false,
            alinhar_direita: false,
            rotulo_acessivel: None,
            habilitado: true,
            atalho: None,
            pastilha: None,
            somente_leitura: false,
            pequeno: false,
            fixo: false,
            dica_italica: false,
        }
    }

    /// O `.campo.pq`: 26px de altura ([`ALTURA_CAMPO_PEQUENO`]) com a moldura de sempre (as linhas de coluna da Nova
    /// tabela).
    pub fn pequeno(mut self) -> Self {
        self.pequeno = true;
        self
    }

    /// O campo que o "NULL" marcado trava (`.campo.nulo` da ficha da linha): não edita, mas fica com a aparência
    /// normal (sem o esmaecimento do [`Entrada::habilitado`]); com [`Entrada::dica_italica`], o "NULL" em itálico.
    pub fn fixo(mut self, sim: bool) -> Self {
        self.fixo = sim;
        self
    }

    /// A dica (o `placeholder`) em itálico `--t3`: o "NULL" e o "vazio" da ficha da linha (`.campo.nulo::placeholder`).
    pub fn dica_italica(mut self, sim: bool) -> Self {
        self.dica_italica = sim;
        self
    }

    /// A pastilha de tipo de 16px antes do texto, com `gap:8px` (o Tipo da janela de coluna e da Nova tabela).
    /// Quem chama a calcula a cada quadro, pelo texto ([`TipoColuna::do_declarado`]).
    pub fn pastilha(mut self, tipo: TipoColuna) -> Self {
        self.pastilha = Some(tipo);
        self
    }

    /// O `.campo.so-leitura` (a ficha só leitura, D9): o texto pode ser selecionado e copiado, mas não muda; fundo
    /// `--p`, borda `--fb`, texto `--t2`, sem o baixo-relevo nem o halo do foco, e sem esmaecer.
    pub fn somente_leitura(mut self, sim: bool) -> Self {
        self.somente_leitura = sim;
        self
    }

    /// `input:disabled`: 70% de opacidade, sem edição nem foco (o campo travado por "NULL" ou "Usar o padrão").
    pub fn habilitado(mut self, sim: bool) -> Self {
        self.habilitado = sim;
        self
    }

    /// O `placeholder` (em `--t3`).
    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }

    /// O `input.mono` (o valor padrão e o CHECK da janela de coluna, a condição do novo índice). No Estúdio o
    /// `.mono` é a fonte da interface em peso 350 (`.mono{font-family:inherit;font-weight:350}`): a Consolas
    /// fica só dentro dos editores de SQL e JSON. Os outros campos de caminho e de número ficam sem a opção.
    pub fn monoespacada(mut self, sim: bool) -> Self {
        self.mono = sim;
        self
    }

    /// O campo da edição no lugar da grade: 26px ([`ALTURA_CAMPO_PEQUENO`]), 12px, `padding:4px 8px`, raio 4,
    /// texto `--t2` (`--t` com o foco); fundo e borda transparentes até o hover (borda `--fb`) e o foco (`--f`,
    /// borda e anel de 1px no destaque). Na aba Dados o texto é sempre `--t` (`.cor(p.fg)`) e o campo tem 140px
    /// no mínimo.
    pub fn compacta(mut self) -> Self {
        self.compacta = true;
        self
    }

    /// A busca da barra (`.busca`): o mesmo campo, com 30px de altura ([`ALTURA_BUSCA`]), o mesmo raio 10 e a lupa
    /// de 14px em `--t3` antes do texto (`gap:6px`).
    pub fn busca(mut self, sim: bool) -> Self {
        self.busca = sim;
        self
    }

    /// O atalho no fim do campo (o `.kbd` da busca: 10.5px em `--t3`, borda `--b2` de raio 4, 18px de altura).
    pub fn atalho(mut self, atalho: &'a str) -> Self {
        self.atalho = Some(atalho);
        self
    }

    /// Cor do texto (no lugar do `--t`, ou do `--t2` sem foco no [`Entrada::compacta`]).
    pub fn cor(mut self, cor: Color32) -> Self {
        self.cor = Some(cor);
        self
    }

    /// Largura fixa (o padrão é a largura disponível).
    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = Some(largura);
        self
    }

    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    pub fn senha(mut self, sim: bool) -> Self {
        self.senha = sim;
        self
    }

    /// Texto à direita (números na grade).
    pub fn alinhar_direita(mut self, sim: bool) -> Self {
        self.alinhar_direita = sim;
        self
    }

    /// Nome para o leitor de tela (o `aria-label`, quando não há rótulo visível).
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo_acessivel = Some(rotulo);
        self
    }

    /// Mostra e devolve a saída completa do `TextEdit` (cursor, galley).
    pub fn mostrar(self, ui: &mut Ui) -> egui::text_edit::TextEditOutput {
        if self.habilitado {
            self.desenhar(ui)
        } else {
            ui.scope(|ui| {
                ui.multiply_opacity(0.7);
                self.desenhar(ui)
            })
            .inner
        }
    }

    fn desenhar(self, ui: &mut Ui) -> egui::text_edit::TextEditOutput {
        let p = paleta(ui);
        // o `.kbd` no fim (o texto dele, a largura com o `padding:0 4px` e a borda)
        let kbd = self.atalho.map(|a| {
            let g = galley(ui, a, fontes::fonte(10.5, Peso::Normal), p.estudio.t3);
            let largura = (g.size().x + 2.0 * 4.0 + 2.0).ceil();
            (g, largura)
        });
        let (fonte, altura, margem) = if self.compacta {
            (fontes::fonte(12.0, Peso::Normal), ALTURA_CAMPO_PEQUENO, Margin::symmetric(9, 0))
        } else {
            let altura = if self.busca {
                ALTURA_BUSCA
            } else if self.pequeno {
                ALTURA_CAMPO_PEQUENO
            } else {
                ALTURA_CAMPO
            };
            let mut margem = Margin::symmetric(RECUO_TEXTO as i8, 0);
            if self.busca {
                // a lupa de 14px e o `gap:6px` antes do texto
                margem.left = (RECUO_TEXTO + LADO_LUPA + 6.0) as i8;
            } else if self.pastilha.is_some() {
                margem.left = (RECUO_TEXTO + LADO_PASTILHA + GAP_PASTILHA) as i8;
            }
            if let Some((_, largura)) = &kbd {
                margem.right = (RECUO_TEXTO + largura + 6.0).min(127.0) as i8;
            }
            (fonte_campo(self.mono, None), altura, margem)
        };
        let largura = self.largura.unwrap_or_else(|| ui.available_width()).max(LARGURA_MINIMA_CAMPO);
        let lugar = LugarMoldura::reservar(ui);
        let texto = self.texto;
        // o mesmo id que o `TextEdit` escolheria sozinho (para saber, antes de desenhar, se ele tem o foco)
        let id = match self.id_salt {
            Some(id) => id,
            None => {
                let id = ui.next_auto_id();
                ui.skip_ahead_auto_ids(1);
                id
            }
        };
        let focado_antes = ui.memory(|m| m.has_focus(id));
        let cor_texto = match (self.cor, self.compacta) {
            (Some(c), _) => c,
            (None, true) if !focado_antes => p.muted,
            (None, _) if self.somente_leitura => p.estudio.t2,
            _ => p.fg,
        };
        // só leitura: o `TextEdit` sobre um `&str` seleciona e copia, mas não muda o texto
        let mut so_ler: &str;
        let buffer: &mut dyn egui::TextBuffer = if self.somente_leitura {
            so_ler = texto.as_str();
            &mut so_ler
        } else {
            &mut *texto
        };
        let mut te = TextEdit::singleline(buffer)
            .font(fonte.clone())
            .text_color(cor_texto)
            .frame(egui::Frame::NONE.inner_margin(margem))
            .min_size(vec2(largura, altura))
            .desired_width(largura)
            .vertical_align(Align::Center)
            .horizontal_align(if self.alinhar_direita { Align::Max } else { Align::Min })
            .interactive(self.habilitado && !self.fixo)
            .password(self.senha);
        if let Some(d) = self.dica {
            let fonte_dica = if self.dica_italica { fontes::fonte(fonte.size, Peso::Italico) } else { fonte };
            te = te.hint_text(RichText::new(d).font(fonte_dica).color(p.faint));
        }
        let saida = te.id(id).show(ui);
        let resp = &saida.response.response;
        let estado =
            EstadoCampo::animar(ui, resp.id, self.habilitado && !self.fixo && resp.hovered(), resp.has_focus())
                .desligado_se(!self.habilitado);
        if self.compacta {
            pintar_moldura_compacta(ui, &lugar, resp.rect, estado);
        } else {
            if self.somente_leitura {
                pintar_moldura_so_leitura(ui, &lugar, resp.rect, RAIO_CAMPO);
            } else {
                pintar_moldura(ui, &lugar, resp.rect, RAIO_CAMPO, estado);
            }
            let es = &p.estudio;
            if self.busca {
                let lupa = Rect::from_min_size(
                    pos2(resp.rect.left() + RECUO_TEXTO, resp.rect.center().y - LADO_LUPA / 2.0),
                    vec2(LADO_LUPA, LADO_LUPA),
                );
                crate::icones::pintar(ui.painter(), lupa, crate::icones::Icone::Buscar, es.t3);
            } else if let Some(tipo) = self.pastilha {
                let caixa = Rect::from_min_size(
                    pos2(resp.rect.left() + RECUO_TEXTO, resp.rect.center().y - LADO_PASTILHA / 2.0),
                    vec2(LADO_PASTILHA, LADO_PASTILHA),
                );
                crate::icones::pintar(ui.painter(), caixa, Icone::Tipo(tipo), Color32::WHITE);
            }
            if let Some((g, largura)) = kbd {
                // `.kbd{line-height:16px}` + a borda: 18px
                let caixa = Rect::from_min_size(
                    pos2(resp.rect.right() - RECUO_TEXTO - largura, resp.rect.center().y - 9.0),
                    vec2(largura, 18.0),
                );
                ui.painter().rect_stroke(caixa, 4.0, Stroke::new(1.0, es.b2), StrokeKind::Inside);
                ui.painter().galley(caixa.center() - g.size() / 2.0, g, es.t3);
            }
        }
        if self.habilitado && !self.fixo {
            foco_inicial(ui, resp);
        }
        // Enter num campo de uma linha envia o formulário da janela (o submit implícito do `<form>`)
        if enter_no_campo(ui, resp) {
            crate::pecas::janela::marcar_envio(ui.ctx());
        }
        if let Some(r) = self.rotulo_acessivel {
            let dica = self.dica.unwrap_or("");
            let senha = self.senha;
            let habilitado = self.habilitado;
            resp.widget_info(|| {
                let valor = if senha { "" } else { texto.as_str() };
                let mut info = WidgetInfo::text_edit(habilitado, valor, valor, dica);
                info.label = Some(r.to_owned());
                info
            });
        }
        saida
    }
}

impl Widget for Entrada<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.mostrar(ui).response.response
    }
}

/// Área de texto (`textarea`): o baixo-relevo do `.campo` com o mesmo raio 10 ([`RAIO_CAMPO`]), `padding:8px 12px`,
/// `line-height:1.55`, 60px no mínimo.
#[must_use = "use ui.add(area)"]
pub struct AreaTexto<'a> {
    texto: &'a mut String,
    dica: Option<&'a str>,
    mono: bool,
    tamanho_fonte: Option<f32>,
    altura: f32,
    largura: Option<f32>,
    id_salt: Option<Id>,
    codigo: bool,
    habilitado: bool,
    numeros: bool,
    marcas: Vec<(Range<usize>, Color32)>,
    sublinhado: Option<Range<usize>>,
    destaque: Option<(Range<usize>, bool)>,
    plana: bool,
    cores: Option<CoresCodigo>,
    somente_leitura: bool,
    linguagem: crate::codigo::Linguagem,
    erro_na_linha: Option<usize>,
}

/// A calha dos números das linhas do editor de código (`width:46px`, `padding-right:12px`).
pub const LARGURA_CALHA: f32 = 46.0;
const RECUO_NUMERO: f32 = 12.0;
/// O editor plano ([`AreaTexto::plana`], `.cod{padding:0 14px}` dentro de um bloco com `padding-top:10px`): o recuo
/// do texto dos lados e em cima.
const RECUO_PLANA_X: f32 = 14.0;
const RECUO_PLANA_Y: f32 = 10.0;

impl<'a> AreaTexto<'a> {
    pub fn new(texto: &'a mut String) -> Self {
        AreaTexto {
            texto,
            dica: None,
            mono: false,
            tamanho_fonte: None,
            altura: 60.0,
            largura: None,
            id_salt: None,
            codigo: false,
            habilitado: true,
            numeros: false,
            marcas: Vec::new(),
            sublinhado: None,
            destaque: None,
            plana: false,
            cores: None,
            somente_leitura: false,
            linguagem: crate::codigo::Linguagem::Sql,
            erro_na_linha: None,
        }
    }

    /// Só no modo código: o editor plano, encostado no que está em volta (o editor da Consulta SQL dentro do
    /// documento, prancheta 3): sem a borda, o raio, a sombra e o anel do campo, com o fundo das cores do código e o
    /// texto a 14px dos lados e a 10px do alto (`.cod{padding:0 14px}`). A calha, quando há, encosta na esquerda.
    pub fn plana(mut self, sim: bool) -> Self {
        self.plana = sim;
        self
    }

    /// Só no modo código: o fundo da seleção (`--sel`) nos caracteres `trecho` (em caracteres, como o cursor do
    /// `TextEdit`), pintado por baixo do texto. É o que a busca achou: o egui reduz a seleção do editor ao cursor
    /// quando o desenha sem o foco (o foco fica na busca), e o trecho continua à vista assim. Com `rolar`, a
    /// rolagem do editor leva o trecho para a vista neste quadro.
    pub fn destacar(mut self, trecho: Range<usize>, rolar: bool) -> Self {
        self.destaque = Some((trecho, rolar));
        self
    }

    /// O `textarea.campo.so-leitura` (a ficha só leitura): o texto seleciona e copia, mas não muda; fundo `--p`,
    /// borda `--fb`, texto `--t2`, sem baixo-relevo e sem esmaecer.
    pub fn somente_leitura(mut self, sim: bool) -> Self {
        self.somente_leitura = sim;
        self
    }

    /// Só no modo código: a calha de [`LARGURA_CALHA`] à esquerda com os números das linhas (fundo `--p`, números em
    /// `--t3` na fonte do código, alinhados à direita com 12px de folga, fio `--b` à direita). Ela fica parada na
    /// rolagem horizontal (o texto passa por baixo dela) e acompanha a vertical.
    pub fn numeros_de_linha(mut self, sim: bool) -> Self {
        self.numeros = sim;
        self
    }

    /// Só no modo código: o fundo `cor` nas linhas `linhas` (contadas do 0, na largura toda do texto, por baixo
    /// dele): as do comando com erro, no `--perigo` a 9%. Pode ser chamado mais de uma vez.
    pub fn marcar_linhas(mut self, linhas: Range<usize>, cor: Color32) -> Self {
        self.marcas.push((linhas, cor));
        self
    }

    /// Só no modo código: o sublinhado ondulado em `--perigo` nos caracteres `trecho` (em caracteres, como o
    /// cursor do `TextEdit`): o nome que a mensagem de erro cita.
    pub fn sublinhar(mut self, trecho: Range<usize>) -> Self {
        self.sublinhado = Some(trecho);
        self
    }

    /// Só no modo código: a linguagem do realce ([`crate::codigo::Linguagem`]: SQL por padrão, JSON, ou texto sem
    /// cores). O cache do realce separa as três.
    pub fn linguagem(mut self, linguagem: crate::codigo::Linguagem) -> Self {
        self.linguagem = linguagem;
        self
    }

    /// Só no modo código: a linha `linha` (contada do 0) é a do erro; o número dela na calha sai em `--perigo` (o
    /// fundo e o sublinhado são do [`AreaTexto::marcar_linhas`] e do [`AreaTexto::sublinhar`]).
    pub fn erro_na_linha(mut self, linha: usize) -> Self {
        self.erro_na_linha = Some(linha);
        self
    }

    /// Só no modo código: estas cores no lugar das da aparência em vigor ([`crate::codigo::atual`]).
    pub fn cores(mut self, cores: CoresCodigo) -> Self {
        self.cores = Some(cores);
        self
    }

    /// `textarea:disabled`: 70% de opacidade, sem edição nem foco.
    pub fn habilitado(mut self, sim: bool) -> Self {
        self.habilitado = sim;
        self
    }

    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }

    /// Monoespaçada (a Consolas do editor de SQL, `.cod`).
    pub fn mono(mut self, sim: bool) -> Self {
        self.mono = sim;
        self
    }

    /// Tamanho da fonte (12px na interface; 12.5px no editor de SQL, como o `.cod`).
    pub fn tamanho_fonte(mut self, tamanho: f32) -> Self {
        self.tamanho_fonte = Some(tamanho);
        self
    }

    /// Altura mínima (60px por padrão, 96px na janela da linha, 220px no editor de SQL).
    pub fn altura(mut self, altura: f32) -> Self {
        self.altura = altura;
        self
    }

    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = Some(largura);
        self
    }

    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    /// Modo código (o editor de SQL: `white-space:pre`, linha de 1.6): o texto não quebra e a área tem altura
    /// fixa (`altura`), rolando nas duas direções, com as cores do código ([`crate::codigo::realcar_sql`]) sobre o
    /// fundo do esquema e a fonte do código (a família e o tamanho das Preferências; o `tamanho_fonte` passa por cima
    /// do tamanho). O Tab fica no campo e faz o `recuar()` da Consulta SQL
    /// ([`recuar`]): dois espaços no cursor, ou o recuo das linhas selecionadas inteiras; Shift+Tab tira o recuo.
    /// Nunca insere `\t`. Depois de um Esc, o Tab seguinte volta a mover o foco (o teclado não fica preso), e o
    /// Esc não tira o foco do campo.
    pub fn codigo(mut self, sim: bool) -> Self {
        self.codigo = sim;
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> egui::text_edit::TextEditOutput {
        if self.habilitado {
            self.desenhar(ui)
        } else {
            ui.scope(|ui| {
                ui.multiply_opacity(0.7);
                self.desenhar(ui)
            })
            .inner
        }
    }

    fn desenhar(self, ui: &mut Ui) -> egui::text_edit::TextEditOutput {
        let p = paleta(ui);
        let codigo = self.codigo;
        let aparencia = crate::codigo::atual(ui.ctx());
        let fonte = if codigo {
            FontId::monospace(self.tamanho_fonte.unwrap_or(aparencia.tamanho))
        } else if self.mono {
            fontes::fonte_mono(self.tamanho_fonte.unwrap_or(12.0))
        } else {
            fontes::fonte(self.tamanho_fonte.unwrap_or(12.0), Peso::Normal)
        };
        let cores = self.cores.unwrap_or(aparencia.cores).resolver(p);
        let cor_texto = if codigo {
            cores.texto
        } else if self.somente_leitura {
            p.estudio.t2
        } else {
            p.fg
        };
        let calha = if codigo && self.numeros { LARGURA_CALHA } else { 0.0 };
        let plana = codigo && self.plana;
        let largura = self.largura.unwrap_or_else(|| ui.available_width()).max(LARGURA_MINIMA_CAMPO);
        let altura_linha_css = (fonte.size * if codigo { 1.6 } else { 1.55 }).round();
        let linha_egui = ui.fonts_mut(|f| f.row_height(&fonte));
        let lugar = LugarMoldura::reservar(ui);
        // as linhas marcadas vão por cima do fundo e por baixo do texto
        let lugar_marcas = ui.painter().add(Shape::Noop);
        // a sobra da entrelinha vai metade em cima (como no CSS) e metade embaixo de cada linha
        let extra = (altura_linha_css - linha_egui).max(0.0);
        // o plano não tem a borda de 1px: o texto fica a 14px dos lados e a 10px do alto
        let (recuo_x, recuo_y) = if plana { (RECUO_PLANA_X, RECUO_PLANA_Y) } else { (RECUO_TEXTO, 9.0) };
        let margem = Margin {
            left: (recuo_x + calha) as i8,
            right: recuo_x as i8,
            top: (recuo_y + extra / 2.0).round() as i8,
            bottom: (recuo_y - extra / 2.0).max(1.0).round() as i8,
        };
        let habilitado = self.habilitado;
        let altura = self.altura;
        let id = self.id_salt.unwrap_or_else(|| ui.next_auto_id());
        let destaque = if codigo { self.destaque.clone() } else { None };
        // modo código: o Tab recua (em vez do `\t` do egui) até um Esc o liberar para mover o foco
        let mut liberar_tab = false;
        let mut tab_passou = false;
        let mut recuou = false;
        if codigo && habilitado {
            let id_liberar = id.with("liberar_tab");
            liberar_tab = ui.ctx().data(|d| d.get_temp(id_liberar)).unwrap_or(false);
            if ui.memory(|m| m.has_focus(id)) {
                let mut recuos = Vec::new();
                ui.input_mut(|i| {
                    i.events.retain(|e| match e {
                        Event::Key { key: Key::Escape, pressed: true, .. } => {
                            liberar_tab = true;
                            true
                        }
                        Event::Key { key: Key::Tab, pressed: true, modifiers, .. }
                            if !modifiers.ctrl && !modifiers.alt && !modifiers.command =>
                        {
                            if liberar_tab {
                                // deixa o egui mover o foco (o filtro do quadro anterior já não prende o Tab)
                                liberar_tab = false;
                                tab_passou = true;
                                true
                            } else {
                                recuos.push(modifiers.shift);
                                false
                            }
                        }
                        Event::Key { pressed: true, .. } => {
                            liberar_tab = false;
                            true
                        }
                        Event::Text(t) => t != "\t",
                        _ => true,
                    })
                });
                for remover in recuos {
                    recuou |= aplicar_recuo(ui, id, self.texto, remover);
                }
            }
            ui.ctx().data_mut(|d| d.insert_temp(id_liberar, liberar_tab));
        }
        // só leitura: o TextEdit sobre um &str seleciona e copia, mas não muda o texto
        // (sem copiar o texto: o `&str` empresta o próprio `String`, e o buffer só é o `&mut String` fora da leitura)
        let so_leitura = self.somente_leitura;
        let mut so_ler: &str = self.texto.as_str();
        let buffer: &mut dyn egui::TextBuffer = if so_leitura { &mut so_ler } else { &mut *self.texto };
        let (mut saida, rect) = ui
            .scope(|ui| {
                ui.spacing_mut().extra_text_line_spacing = extra;
                if codigo {
                    // o cursor e o texto selecionado na cor do texto do esquema (o Monokai no tema claro)
                    ui.visuals_mut().text_cursor.stroke.color = cores.texto;
                    ui.visuals_mut().selection.stroke.color = cores.texto;
                }
                let fonte_layout = fonte.clone();
                let linguagem = self.linguagem;
                let mut realce = move |ui: &Ui, texto: &dyn egui::TextBuffer, _largura: f32| {
                    let job = crate::codigo::realce_em_cache(
                        ui.ctx(),
                        texto.as_str(),
                        linguagem,
                        &cores,
                        &fonte_layout,
                        linha_egui + extra,
                        None,
                    );
                    ui.fonts_mut(|f| f.layout_job(LayoutJob::clone(&job)))
                };
                let mut te = TextEdit::multiline(buffer)
                    .id(id)
                    .font(fonte.clone())
                    .text_color(cor_texto)
                    .frame(egui::Frame::NONE.inner_margin(margem))
                    .desired_rows(1)
                    .interactive(habilitado);
                if codigo {
                    // o Tab fica no campo (o recuo foi aplicado acima), salvo depois de um Esc; o Esc não tira o
                    // foco. No quadro em que o Tab liberado passa, o filtro também não o deixa virar `\t`.
                    te = te.event_filter(EventFilter {
                        tab: !(liberar_tab || tab_passou),
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    });
                }
                if let Some(d) = self.dica {
                    te = te.hint_text(RichText::new(d).font(fonte.clone()).color(p.faint));
                }
                if codigo {
                    te = te.layouter(&mut realce).desired_width(f32::INFINITY).min_size(vec2(largura, altura));
                    let rolagem = egui::ScrollArea::both()
                        .id_salt(id.with("rolagem"))
                        .max_height(altura)
                        .max_width(largura)
                        .auto_shrink([false, false]);
                    let s = rolagem.show(ui, |ui| {
                        let saida = te.show(ui);
                        // o trecho destacado vem para a vista (dentro da rolagem, para valer neste quadro)
                        if let Some((trecho, true)) = &destaque {
                            let rects = retangulos_do_trecho(&saida.galley, saida.galley_pos, trecho);
                            if let Some(r) = rects.into_iter().reduce(|a, b| a.union(b)) {
                                ui.scroll_to_rect(r.expand2(vec2(RECUO_TEXTO + calha, 0.0)), Some(egui::Align::Center));
                            }
                        }
                        saida
                    });
                    let rect = egui::Rect::from_min_size(s.inner_rect.min, vec2(largura, altura));
                    (s.inner, rect)
                } else {
                    let s = te.min_size(vec2(largura, altura)).desired_width(largura).show(ui);
                    let rect = s.response.response.rect;
                    (s, rect)
                }
            })
            .inner;
        if recuou {
            saida.response.response.mark_changed();
        }
        let resp = &saida.response.response;
        let estado =
            EstadoCampo::animar(ui, resp.id, habilitado && resp.hovered(), resp.has_focus()).desligado_se(!habilitado);
        let fundo = if codigo { cores.fundo } else { p.estudio.f };
        if plana {
            ui.painter().set(lugar.fundo, RectShape::filled(rect, 0, fundo));
        } else if so_leitura {
            pintar_moldura_so_leitura(ui, &lugar, rect, RAIO_CAMPO);
        } else {
            pintar_moldura_com_fundo(ui, &lugar, rect, RAIO_CAMPO, estado, fundo);
        }
        if codigo {
            let extras = ExtrasCodigo {
                rect,
                borda: if plana { 0.0 } else { 1.0 },
                calha,
                fonte: &fonte,
                altura_linha: linha_egui + extra,
                marcas: &self.marcas,
                sublinhado: self.sublinhado.as_ref(),
                destaque: destaque.as_ref().map(|(t, _)| t),
                erro_na_linha: self.erro_na_linha,
            };
            extras.pintar(ui, lugar_marcas, &saida.galley, saida.galley_pos);
        }
        if habilitado {
            foco_inicial(ui, resp);
        }
        saida
    }
}

/// O que o modo código pinta em volta do texto: as linhas marcadas (por baixo), a calha com os números e o
/// sublinhado ondulado (por cima).
struct ExtrasCodigo<'a> {
    /// O campo inteiro (com a borda).
    rect: Rect,
    /// A borda do campo (0 no [`AreaTexto::plana`]).
    borda: f32,
    calha: f32,
    fonte: &'a FontId,
    altura_linha: f32,
    marcas: &'a [(Range<usize>, Color32)],
    sublinhado: Option<&'a Range<usize>>,
    destaque: Option<&'a Range<usize>>,
    /// A linha (do 0) cujo número da calha sai em `--perigo`.
    erro_na_linha: Option<usize>,
}

/// Os retângulos (um por linha visual) dos caracteres `trecho` do galley em `pos`, na tela; uma linha vazia dentro do
/// trecho ganha a largura de um espaço, como a seleção do egui.
fn retangulos_do_trecho(galley: &Galley, pos: Pos2, trecho: &Range<usize>) -> Vec<Rect> {
    let mut rects = Vec::new();
    let mut inicio = 0usize;
    for row in &galley.rows {
        let n = row.char_count_excluding_newline().0;
        let fim_linha = inicio + n + usize::from(row.ends_with_newline);
        let (a, b) = (trecho.start.max(inicio), trecho.end.min(inicio + n));
        let base = pos + row.pos.to_vec2();
        let y = row.rect().translate(pos.to_vec2()).y_range();
        if a < b
            && let (Some(g0), Some(g1)) = (row.glyphs.get(a - inicio), row.glyphs.get(b - 1 - inicio))
        {
            let (x0, x1) = (base.x + g0.pos.x, base.x + g1.pos.x + g1.advance_width);
            rects.push(Rect::from_x_y_ranges(x0..=x1, y));
        } else if n == 0 && trecho.start <= inicio && trecho.end > inicio {
            let x0 = base.x;
            rects.push(Rect::from_x_y_ranges(x0..=x0 + row.height() * 0.3, y));
        }
        inicio = fim_linha;
    }
    rects
}

/// As linhas lógicas do galley (a 1ª linha de cada parágrafo; o modo código não quebra), com o retângulo na tela.
fn linhas_do_galley(galley: &Galley, pos: Pos2) -> Vec<(usize, Rect)> {
    let mut linhas = Vec::with_capacity(galley.rows.len());
    let mut nova = true;
    for row in &galley.rows {
        if nova {
            linhas.push((linhas.len(), row.rect().translate(pos.to_vec2())));
        }
        nova = row.ends_with_newline;
    }
    linhas
}

impl ExtrasCodigo<'_> {
    fn pintar(&self, ui: &Ui, lugar_marcas: ShapeIdx, galley: &Galley, pos: Pos2) {
        let es = &paleta(ui).estudio;
        let dentro = self.rect.shrink(self.borda).intersect(ui.clip_rect());
        if !dentro.is_positive() {
            return;
        }
        let linhas = linhas_do_galley(galley, pos);
        let visiveis = || linhas.iter().filter(|(_, r)| r.max.y >= dentro.min.y && r.min.y <= dentro.max.y);
        let esquerda_texto = self.rect.left() + self.borda + self.calha;

        // as linhas marcadas, na largura toda do texto
        let mut marcas = Vec::new();
        for (faixa, cor) in self.marcas {
            for (_, r) in visiveis().filter(|(n, _)| faixa.contains(n)) {
                let linha = Rect::from_x_y_ranges(esquerda_texto..=dentro.right(), r.y_range()).intersect(dentro);
                if linha.is_positive() {
                    marcas.push(Shape::rect_filled(linha, 0.0, *cor));
                }
            }
        }
        // o trecho destacado, no fundo da seleção, por cima das linhas marcadas
        if let Some(trecho) = self.destaque {
            let area = Rect::from_x_y_ranges(esquerda_texto..=dentro.right(), dentro.y_range());
            for r in retangulos_do_trecho(galley, pos, trecho) {
                let r = r.intersect(area);
                if r.is_positive() {
                    marcas.push(Shape::rect_filled(r, 2.0, es.sel));
                }
            }
        }
        ui.painter().set(lugar_marcas, Shape::Vec(marcas));

        // o sublinhado ondulado (`text-decoration:underline wavy --perigo`), por cima do texto
        if let Some(trecho) = self.sublinhado {
            let pintor =
                ui.painter().with_clip_rect(Rect::from_x_y_ranges(esquerda_texto..=dentro.right(), dentro.y_range()));
            let mut inicio = 0usize;
            for row in &galley.rows {
                let n = row.char_count_excluding_newline().0;
                let (a, b) = (trecho.start.max(inicio), trecho.end.min(inicio + n));
                if a < b
                    && let (Some(g0), Some(g1)) = (row.glyphs.get(a - inicio), row.glyphs.get(b - 1 - inicio))
                {
                    let base = pos + row.pos.to_vec2();
                    let (x0, x1) = (base.x + g0.pos.x, base.x + g1.pos.x + g1.advance_width);
                    // `text-underline-offset:3px`: abaixo do `_` dos nomes
                    let y = base.y + g0.pos.y + 4.0;
                    pintor.add(Shape::line(onda(x0, x1, y), Stroke::new(1.0, es.perigo)));
                }
                inicio += n + usize::from(row.ends_with_newline);
            }
        }

        // a calha dos números, parada na rolagem horizontal (o texto passa por baixo dela)
        if self.calha > 0.0 {
            let b = self.borda;
            let calha = Rect::from_min_max(
                pos2(self.rect.left() + b, self.rect.top() + b),
                pos2(self.rect.left() + self.calha, self.rect.bottom() - b),
            );
            let raio = if b > 0.0 { (RAIO_CAMPO - 1.0).max(0.0) as u8 } else { 0 };
            let cantos = egui::CornerRadius { nw: raio, sw: raio, ne: 0, se: 0 };
            let pintor = ui.painter().with_clip_rect(calha.intersect(ui.clip_rect()));
            pintor.add(RectShape::filled(calha, cantos, es.faixa));
            pintor.vline(calha.right() - 0.5, calha.y_range(), Stroke::new(1.0, es.b));
            for (n, r) in visiveis() {
                let cor = if self.erro_na_linha == Some(*n) { es.perigo } else { es.t3 };
                let mut job = LayoutJob::simple_singleline((n + 1).to_string(), self.fonte.clone(), cor);
                for s in &mut job.sections {
                    s.format.line_height = Some(self.altura_linha);
                }
                let numero = ui.fonts_mut(|f| f.layout_job(job));
                let x = calha.right() - RECUO_NUMERO - numero.size().x;
                pintor.galley(pos2(x, r.top()), numero, cor);
            }
        }
    }
}

/// O zigue-zague do sublinhado ondulado de `x0` a `x1`, em volta de `y` (onda de 4px, 1px para cada lado).
fn onda(x0: f32, x1: f32, y: f32) -> Vec<Pos2> {
    let mut pontos = vec![pos2(x0, y)];
    let mut x = x0;
    let mut cima = true;
    while x < x1 {
        x = (x + 2.0).min(x1);
        pontos.push(pos2(x, if cima { y - 1.0 } else { y + 1.0 }));
        cima = !cima;
    }
    pontos
}

/// O recuo do Tab no editor de SQL.
const RECUO: &str = "  ";

/// O recuo da Consulta SQL sobre o texto e a seleção `inicio..fim` (em caracteres): Tab sem
/// seleção de várias linhas troca a seleção por dois espaços; Tab com várias linhas recua cada linha inteira
/// da seleção; Shift+Tab (`remover`) tira dois espaços (ou um espaço ou tab) do começo de cada linha. Devolve o
/// texto novo e a seleção nova, ou `None` quando nada muda.
pub fn recuar(texto: &str, inicio: usize, fim: usize, remover: bool) -> Option<(String, usize, usize)> {
    let chars: Vec<char> = texto.chars().collect();
    let n = chars.len();
    let s = inicio.min(fim).min(n);
    let e = inicio.max(fim).min(n);
    let varias_linhas = chars[s..e].contains(&'\n');
    if !remover && !varias_linhas {
        let mut novo: String = chars[..s].iter().collect();
        novo.push_str(RECUO);
        novo.extend(&chars[e..]);
        let pos = s + RECUO.len();
        return Some((novo, pos, pos));
    }
    let ini = chars[..s].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1);
    // seleção que termina logo depois de uma quebra de linha não leva a linha seguinte junto
    let fim_selecao = if e > s && chars[e - 1] == '\n' { e - 1 } else { e };
    let fim_bloco = chars[fim_selecao..].iter().position(|&c| c == '\n').map_or(n, |i| fim_selecao + i);
    let bloco: String = chars[ini..fim_bloco].iter().collect();
    let mut tirado_primeira = 0;
    let linhas: Vec<String> = bloco
        .split('\n')
        .enumerate()
        .map(|(i, linha)| {
            if !remover {
                return format!("{RECUO}{linha}");
            }
            let tirar = if linha.starts_with(RECUO) {
                RECUO.len()
            } else if linha.starts_with([' ', '\t']) {
                1
            } else {
                0
            };
            if i == 0 {
                tirado_primeira = tirar;
            }
            linha.chars().skip(tirar).collect()
        })
        .collect();
    let novo_bloco = linhas.join("\n");
    if novo_bloco == bloco {
        return None;
    }
    let mut novo: String = chars[..ini].iter().collect();
    novo.push_str(&novo_bloco);
    novo.extend(&chars[fim_bloco..]);
    if varias_linhas {
        Some((novo, ini, ini + novo_bloco.chars().count()))
    } else {
        let pos = ini.max(s.saturating_sub(tirado_primeira));
        Some((novo, pos, pos.max(e.saturating_sub(tirado_primeira))))
    }
}

/// Aplica o [`recuar`] no texto e na seleção guardada do `TextEdit` `id`. Devolve se o texto mudou.
fn aplicar_recuo(ui: &Ui, id: Id, texto: &mut String, remover: bool) -> bool {
    let mut estado = TextEditState::load(ui.ctx(), id).unwrap_or_default();
    let n = texto.chars().count();
    let (ini, fim) = estado.cursor.char_range().map_or((n, n), |r| {
        let [a, b] = r.sorted_cursors();
        (a.index.0, b.index.0)
    });
    let Some((novo, a, b)) = recuar(texto, ini, fim, remover) else { return false };
    *texto = novo;
    estado.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
    estado.store(ui.ctx(), id);
    true
}

/// Enter (sem Shift, Ctrl nem Alt) acabou de sair deste campo de uma linha? (O `TextEdit` de uma linha solta o
/// foco no Enter.) É o submit implícito do `<form>` da janela.
pub fn enter_no_campo(ui: &Ui, resposta: &Response) -> bool {
    resposta.lost_focus()
        && ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(e, Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                    if !modifiers.shift && !modifiers.ctrl && !modifiers.alt && !modifiers.command)
            })
        })
}

impl Widget for AreaTexto<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.mostrar(ui).response.response
    }
}

// ------------------------------------------------------------------ foco inicial da janela

fn id_foco_pendente() -> Id {
    Id::new("databox::campos::foco_inicial")
}

/// Marca que o próximo campo de digitar desenhado deve receber o foco (a janela faz isso no quadro em que
/// abre). `false` desmarca.
pub fn pedir_foco_inicial(ui: &Ui, sim: bool) {
    ui.ctx().data_mut(|d| d.insert_temp(id_foco_pendente(), sim));
}

/// O foco inicial pedido ainda não foi dado a nenhum campo?
pub fn foco_inicial_pendente(ui: &Ui) -> bool {
    ui.ctx().data(|d| d.get_temp::<bool>(id_foco_pendente())).unwrap_or(false)
}

fn foco_inicial(ui: &Ui, resp: &Response) {
    // no quadro em que a janela abre, a camada dela ainda não é a de cima e o `Ui` vem desabilitado; o foco é
    // pedido assim mesmo e vale a partir do quadro seguinte
    if foco_inicial_pendente(ui) {
        resp.request_focus();
        pedir_foco_inicial(ui, false);
    }
}

// ------------------------------------------------------------------ select

/// O `.campo` de escolher (o `button.campo` das pranchetas): o retângulo de raio 10 do campo ([`RAIO_CAMPO`]) com o
/// texto da opção escolhida e a seta de traço em `--t3`; a lista abre embaixo no vidro dos menus ([`lista_opcoes`],
/// com a opção escolhida destacada). Setas ↑/↓ trocam a opção com o campo focado; Enter, espaço ou clique abrem a
/// lista; Esc fecha.
#[must_use = "chame .mostrar(ui)"]
pub struct Select<'a, T: PartialEq + Clone> {
    id: Id,
    valor: &'a mut T,
    opcoes: &'a [(T, &'a str)],
    largura: Option<f32>,
    dica: Option<&'a str>,
    rotulo_acessivel: Option<&'a str>,
    tamanho_fonte: f32,
    altura: f32,
}

/// O que o `button.campo` soma ao texto: a borda e o `padding:0 12px` dos dois lados, os dois `gap:6px` (antes e
/// depois do espaçador) e a seta de 12px.
const LARGURA_EXTRA_SELECT: f32 = 2.0 * RECUO_TEXTO + 6.0 + 6.0 + 12.0;

impl<'a, T: PartialEq + Clone> Select<'a, T> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, valor: &'a mut T, opcoes: &'a [(T, &'a str)]) -> Self {
        Select {
            id: Id::new(("databox::select", id_salt)),
            valor,
            opcoes,
            largura: None,
            dica: None,
            rotulo_acessivel: None,
            tamanho_fonte: 12.0,
            altura: ALTURA_CAMPO,
        }
    }

    /// Outra altura (o padrão é [`ALTURA_CAMPO`]; o "100 por página" do rodapé dos Dados tem 26px).
    pub fn altura(mut self, altura: f32) -> Self {
        self.altura = altura;
        self
    }

    /// Largura fixa (o padrão é a largura disponível; a do maior texto é [`Select::largura_auto`]).
    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = Some(largura);
        self
    }

    /// Largura pelo maior texto das opções (o `width:auto`).
    pub fn largura_auto(mut self) -> Self {
        self.largura = Some(f32::NAN);
        self
    }

    /// Dica ao passar o mouse (o `title`).
    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }

    /// Nome para o leitor de tela (o `aria-label`).
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo_acessivel = Some(rotulo);
        self
    }

    /// Tamanho do texto (12px, o da interface, por padrão).
    pub fn tamanho_fonte(mut self, tamanho: f32) -> Self {
        self.tamanho_fonte = tamanho;
        self
    }

    /// O `.campo.pq`: 26px de altura ([`ALTURA_CAMPO_PEQUENO`]), o mesmo que `altura(ALTURA_CAMPO_PEQUENO)`.
    pub fn pequeno(self) -> Self {
        self.altura(ALTURA_CAMPO_PEQUENO)
    }

    /// Desenha; devolve a resposta do campo, com `changed()` quando a pessoa escolheu outra opção.
    pub fn mostrar(self, ui: &mut Ui) -> Response {
        let p = paleta(ui);
        let fonte = fontes::fonte(self.tamanho_fonte, Peso::Normal);
        let atual = self.opcoes.iter().position(|(v, _)| v == self.valor);
        let texto_atual = atual.and_then(|i| self.opcoes.get(i)).map_or("", |(_, t)| *t);
        let largura = match self.largura {
            Some(l) if l.is_nan() => {
                let maior = self
                    .opcoes
                    .iter()
                    .map(|(_, t)| galley(ui, t, fonte.clone(), Color32::PLACEHOLDER).size().x)
                    .fold(0.0_f32, f32::max);
                (maior + LARGURA_EXTRA_SELECT).ceil()
            }
            Some(l) => l,
            None => ui.available_width(),
        };
        let (rect, mut resp) = ui.allocate_exact_size(vec2(largura.max(40.0), self.altura), Sense::click());
        let popup_id = self.id.with("lista");
        let aberto = Popup::is_id_open(ui.ctx(), popup_id);

        // teclado com o campo focado e a lista fechada: setas trocam a opção, Enter/espaço abrem
        let mut abrir = resp.clicked();
        if resp.has_focus() && !aberto && !self.opcoes.is_empty() {
            let (cima, baixo, enter) = ui.input(|i| {
                (
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::Enter) || i.key_pressed(Key::Space),
                )
            });
            let i = atual.unwrap_or(0);
            let novo = if baixo {
                Some((i + 1).min(self.opcoes.len() - 1))
            } else if cima {
                Some(i.saturating_sub(1))
            } else {
                None
            };
            if let Some(n) = novo
                && Some(n) != atual
                && let Some((v, _)) = self.opcoes.get(n)
            {
                *self.valor = v.clone();
                resp.mark_changed();
            }
            abrir |= enter;
        }
        if resp.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(resp.id, egui::EventFilter { vertical_arrows: true, ..Default::default() })
            });
        }

        // o halo do foco vai a 3σ = 21px
        if ui.is_rect_visible(rect.expand(24.0)) {
            let lugar = LugarMoldura::reservar(ui);
            let estado = EstadoCampo::animar(ui, resp.id, resp.hovered() || aberto, resp.has_focus() || aberto);
            pintar_moldura(ui, &lugar, rect, RAIO_CAMPO, estado);
            let g =
                galley_cortado(ui, texto_atual, fonte.clone(), p.fg, (rect.width() - LARGURA_EXTRA_SELECT).max(10.0));
            ui.painter().galley(pos2(rect.left() + RECUO_TEXTO, rect.center().y - g.size().y / 2.0), g, p.fg);
            pintar_seta(ui.painter(), p, rect);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        let mut escolhida = None;
        let largura_lista = rect.width();
        let ancora = rect;
        let popup = Popup::new(popup_id, ui.ctx().clone(), ancora, ui.layer_id())
            .open_memory(abrir.then_some(egui::SetOpenCommand::Toggle))
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .gap(4.0)
            .width(largura_lista)
            .frame(egui::Frame::NONE);
        popup.show(|ui| {
            ui.set_min_width(largura_lista);
            escolhida = lista_opcoes(ui, self.opcoes, atual, &fonte, largura_lista);
        });
        if let Some(i) = escolhida {
            if Some(i) != atual
                && let Some((v, _)) = self.opcoes.get(i)
            {
                *self.valor = v.clone();
                resp.mark_changed();
            }
            Popup::close_id(ui.ctx(), popup_id);
            resp.request_focus();
        }
        foco_inicial(ui, &resp);
        let rotulo = self.rotulo_acessivel.unwrap_or(texto_atual);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), rotulo));
        match self.dica {
            Some(d) => resp.on_hover_text(d),
            None => resp,
        }
    }
}

/// A seta do select: o chevron `M6 9l6 6 6-6` numa caixa de 12px em `--t3` (o traço de 1,6 da grade de 24), no fim
/// do `padding` da direita.
fn pintar_seta(painter: &Painter, p: &Paleta, campo: Rect) {
    let caixa = Rect::from_min_size(pos2(campo.right() - RECUO_TEXTO - 12.0, campo.center().y - 6.0), vec2(12.0, 12.0));
    let k = caixa.width() / 24.0;
    let ponto = |x: f32, y: f32| caixa.min + vec2(x * k, y * k);
    let traco = PathStroke::new(1.6 * k, p.faint);
    painter.add(PathShape::line(vec![ponto(6.0, 9.0), ponto(12.0, 15.0), ponto(18.0, 9.0)], traco));
}

// ------------------------------------------------------------------ vidro e lista

/// Raio do vidro dos menus e da lista (`border-radius:12px`).
const RAIO_LISTA: f32 = 12.0;
/// A borda de 1px e o `padding:5px` do vidro dos menus.
const RECUO_LISTA: f32 = 6.0;
/// Altura de um item (`.mi{height:28px}`).
const ALTURA_ITEM: f32 = 28.0;
/// A lista rola acima desta altura.
const ALTURA_MAX_LISTA: f32 = 320.0;

/// O reflexo de cima do vidro (`linear-gradient(to bottom, rgba(255,255,255,.05), transparent 140px)`), recortado
/// pelos cantos: o contorno arredondado cortado nos 140px, em leque, com a cor de cada vértice pela altura (dentro
/// do corte o degradê é linear, então a malha é exata).
fn forma_reflexo(rect: Rect, raio: f32) -> Shape {
    const ALCANCE: f32 = 140.0;
    let topo = rgba(255, 255, 255, 0.05);
    let rr = pintura::raio(rect, raio);
    let limite = rect.top() + ALCANCE;
    let mut contorno = Vec::with_capacity(40);
    let cantos = [
        (pos2(rect.left() + rr, rect.top() + rr), PI),
        (pos2(rect.right() - rr, rect.top() + rr), PI + FRAC_PI_2),
        (pos2(rect.right() - rr, rect.bottom() - rr), 0.0),
        (pos2(rect.left() + rr, rect.bottom() - rr), FRAC_PI_2),
    ];
    let passos = if rr >= 1.0 { 8 } else { 0 };
    for (centro, a0) in cantos {
        for i in 0..=passos {
            let a = a0 + FRAC_PI_2 * i as f32 / passos.max(1) as f32;
            contorno.push(centro + vec2(a.cos(), a.sin()) * rr);
        }
    }
    // o corte em `limite` (o contorno é convexo; o que fica embaixo é transparente)
    let mut cortado: Vec<Pos2> = Vec::with_capacity(contorno.len() + 2);
    for (i, &a) in contorno.iter().enumerate() {
        let b = contorno[(i + 1) % contorno.len()];
        if a.y <= limite {
            cortado.push(a);
        }
        if (a.y <= limite) != (b.y <= limite) {
            let t = (limite - a.y) / (b.y - a.y);
            cortado.push(pos2(a.x + (b.x - a.x) * t, limite));
        }
    }
    if cortado.len() < 3 {
        return Shape::Noop;
    }
    let mut m = Mesh::default();
    for &p in &cortado {
        m.colored_vertex(p, topo.gamma_multiply((1.0 - (p.y - rect.top()) / ALCANCE).clamp(0.0, 1.0)));
    }
    for i in 1..cortado.len() as u32 - 1 {
        m.add_triangle(0, i, i + 1);
    }
    Shape::mesh(m)
}

/// O vidro das camadas flutuantes (`.flut`: menus, a lista do select, janelas e avisos que flutuam): a sombra
/// `--sh` por fora, o fundo com o reflexo de cima, a borda de 1px `--vidrob` e o realce `inset 0 1px 0
/// --vidroluz`, que acompanha os cantos. O fundo é o `--flut` translúcido (78% no escuro, 80% no claro): toda camada
/// que se sobrepõe a outra deixa ver o que está atrás, como na barra de menus das pranchetas (pedido de 17/10/2026).
/// Com o fosco ligado ([`crate::fosco`]), o que está atrás vem desfocado e saturado embaixo do `--flut` (o
/// `backdrop-filter` do navegador); sem a textura embaixo (o fosco desligado, a foto que ainda não chegou), o fundo é o
/// mesmo `--flut` a 96% ([`crate::tema::ALFA_VIDRO_LISO`]): a 78/80% o que está atrás apareceria nítido, sem desfoque.
pub fn pintar_vidro_flutuante(painter: &Painter, p: &Paleta, rect: Rect, raio: f32) {
    painter.add(forma_vidro_fosco(painter, p, rect, raio));
}

/// O mesmo que [`pintar_vidro_flutuante`], devolvendo a forma: o vidro de uma camada que só tem tamanho depois do
/// conteúdo (a janela, o painel do menu) vai num lugar reservado antes dele (`painter.add(Shape::Noop)` e, depois
/// de medir, `painter.set`). O `painter` é o da camada (para o fundo desfocado do [`crate::fosco`]).
pub fn forma_vidro_fosco(painter: &Painter, p: &Paleta, rect: Rect, raio: f32) -> Shape {
    forma_vidro(p, rect, raio, crate::fosco::fundo_para(painter, rect))
}

/// O vidro liso, sempre sem o fundo desfocado (o `--flut` a 96%): o das dicas e dos balões, que não levam a foto da
/// janela (uma foto de tela cheia a cada dica custaria mais que a dica vale), com o mesmo fundo que o de
/// [`forma_vidro_fosco`] tem enquanto a textura não existe.
pub fn forma_vidro_flutuante(p: &Paleta, rect: Rect, raio: f32) -> Shape {
    forma_vidro(p, rect, raio, None)
}

fn forma_vidro(p: &Paleta, rect: Rect, raio: f32, fundo: Option<(egui::TextureId, Rect)>) -> Shape {
    let es = &p.estudio;
    let cantos = pintura::canto(rect, raio);
    let mut formas = vec![pintura::forma_sombra(rect, raio, es.sh)];
    // os 78/80% do `--flut` só com a textura desfocada embaixo; sem ela, a mesma cor a 96% (V2)
    let preenchimento = if let Some((textura, uv)) = fundo {
        // o que está atrás, desfocado, embaixo do `--flut`
        formas.push(RectShape::filled(rect, cantos, Color32::WHITE).with_texture(textura, uv).into());
        es.flut
    } else {
        es.flut_liso()
    };
    formas.extend([
        RectShape::filled(rect, cantos, preenchimento).into(),
        forma_reflexo(rect, raio),
        RectShape::stroke(rect, cantos, Stroke::new(1.0, es.vidrob), StrokeKind::Inside).into(),
        forma_realce_curvo(rect.shrink(1.0), raio - 1.0, es.vidroluz),
    ]);
    Shape::Vec(formas)
}

/// Um item de menu em hover ou escolhido (`.mi:hover`, `.mi.on`, raio 6): fundo `--sel`, o realce de 1px em cima
/// (`inset 0 1px 0 rgba(255,255,255,.08)`) e o halo de 12px em `--brilho`, que pode ir até `clip` (o vidro não
/// corta o halo).
pub fn pintar_item_menu(painter: &Painter, clip: Rect, p: &Paleta, rect: Rect) {
    let es = &p.estudio;
    let mut fora = painter.clone();
    fora.set_clip_rect(clip);
    pintura::halo(&fora, rect, 6.0, 12.0, es.brilho);
    painter.rect_filled(rect, 6.0, es.sel);
    realce_curvo(painter, rect, 6.0, rgba(255, 255, 255, 0.08));
}

/// A lista de opções no vidro dos menus ([`pintar_vidro_flutuante`]: raio 12, `padding:5px`, itens `.mi` de 28px
/// com raio 6 e `padding:0 10px 0 12px`; rola acima de 320px). O item em hover e o escolhido têm o mesmo visual
/// ([`pintar_item_menu`]: `--sel`, realce de 1px e brilho), e o destaque segue o ponteiro, como no select do
/// navegador: a opção `atual` fica destacada enquanto o ponteiro não está na lista. É a lista do [`Select`] e das
/// [`EntradaSugestoes`]; ponha-a num `Popup` com `Frame::NONE` e `largura` igual à do gatilho. Devolve o índice clicado.
pub fn lista_opcoes<T>(
    ui: &mut Ui,
    opcoes: &[(T, &str)],
    atual: Option<usize>,
    fonte: &FontId,
    largura: f32,
) -> Option<usize> {
    lista(ui, opcoes, atual, None, fonte, largura)
}

/// A lista de [`lista_opcoes`]: `marcada` é a opção escolhida (destacada enquanto o ponteiro está fora da lista e
/// não há `destaque`); `destaque` é o item escolhido pelo teclado (`.mi.on`).
fn lista<T>(
    ui: &mut Ui,
    opcoes: &[(T, &str)],
    marcada: Option<usize>,
    destaque: Option<usize>,
    fonte: &FontId,
    largura: f32,
) -> Option<usize> {
    let p = paleta(ui);
    let mut escolhida = None;
    let topo = ui.cursor().min;
    let altura = ALTURA_MAX_LISTA.min(opcoes.len() as f32 * ALTURA_ITEM + 2.0 * RECUO_LISTA);
    let rect = Rect::from_min_size(topo, vec2(largura, altura));
    pintar_vidro_flutuante(ui.painter(), p, rect, RAIO_LISTA);
    let clip_halo = ui.clip_rect();
    let interno = rect.shrink(RECUO_LISTA);
    // os itens cobrem o interno inteiro: com o ponteiro ali, o destaque é o do hover
    let marcada_acesa = marcada.filter(|_| destaque.is_none() && !ui.rect_contains_pointer(interno));
    let mut filho = ui.new_child(UiBuilder::new().max_rect(interno).layout(Layout::top_down(Align::Min)));
    filho.spacing_mut().item_spacing.y = 0.0;
    egui::ScrollArea::vertical().max_height(altura - 2.0 * RECUO_LISTA).auto_shrink([false, true]).show_rows(
        &mut filho,
        ALTURA_ITEM,
        opcoes.len(),
        |ui, faixa| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in faixa {
                let Some((_, texto)) = opcoes.get(i) else { continue };
                // o `show_rows` desenha uma linha a mais depois da última à vista; inteira fora da vista, ela só
                // ocupa o lugar (sem clique e fora da árvore de acessibilidade)
                let tamanho = vec2(ui.available_width(), ALTURA_ITEM);
                if !ui.clip_rect().intersect(Rect::from_min_size(ui.cursor().min, tamanho)).is_positive() {
                    ui.allocate_space(tamanho);
                    continue;
                }
                let (r, resp) = ui.allocate_exact_size(tamanho, Sense::click());
                if resp.hovered() || Some(i) == destaque || Some(i) == marcada_acesa {
                    pintar_item_menu(ui.painter(), clip_halo, p, r);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                // `padding:0 10px 0 12px`
                let g = galley_cortado(ui, texto, fonte.clone(), p.fg, r.width() - 12.0 - 10.0);
                ui.painter().galley(pos2(r.left() + 12.0, r.center().y - g.size().y / 2.0), g, p.fg);
                let escolhido = Some(i) == marcada || Some(i) == destaque;
                resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, escolhido, *texto));
                if resp.clicked() {
                    escolhida = Some(i);
                }
            }
        },
    );
    ui.allocate_rect(rect, Sense::hover());
    escolhida
}

// ------------------------------------------------------------------ input com datalist

#[derive(Clone, Copy, Default)]
struct EstadoSugestoes {
    aberta: bool,
    destaque: Option<usize>,
    rect_lista: Option<Rect>,
}

/// Campo de texto com sugestões (o `input` com `datalist` do Tipo na janela de coluna e na Nova tabela: INTEGER,
/// TEXT, REAL…). A lista abre embaixo ao clicar no campo, ao digitar ou com ↓, mostra as sugestões que contêm o
/// que foi digitado (sem diferenciar maiúsculas), ↑/↓ escolhem, Enter ou clique trocam o texto pela sugestão (e
/// esse Enter não envia a janela), Esc fecha só a lista. O texto continua livre: as sugestões não limitam.
#[must_use = "chame .mostrar(ui)"]
pub struct EntradaSugestoes<'a> {
    id: Id,
    texto: &'a mut String,
    sugestoes: &'a [&'a str],
    dica: Option<&'a str>,
    largura: Option<f32>,
    rotulo_acessivel: Option<&'a str>,
    mono: bool,
    pastilha: Option<TipoColuna>,
    pequeno: bool,
}

impl<'a> EntradaSugestoes<'a> {
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        texto: &'a mut String,
        sugestoes: &'a [&'a str],
    ) -> Self {
        EntradaSugestoes {
            id: Id::new(("databox::sugestoes", id_salt)),
            texto,
            sugestoes,
            dica: None,
            largura: None,
            rotulo_acessivel: None,
            mono: false,
            pastilha: None,
            pequeno: false,
        }
    }

    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }

    /// A pastilha do tipo antes do texto ([`Entrada::pastilha`]): o Tipo da janela de coluna e da Nova tabela.
    pub fn pastilha(mut self, tipo: TipoColuna) -> Self {
        self.pastilha = Some(tipo);
        self
    }

    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = Some(largura);
        self
    }

    /// O `.campo.pq` ([`Entrada::pequeno`]).
    pub fn pequeno(mut self) -> Self {
        self.pequeno = true;
        self
    }

    /// Nome para o leitor de tela (o `aria-label`).
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo_acessivel = Some(rotulo);
        self
    }

    /// O `input.mono` ([`Entrada::monoespacada`]).
    pub fn monoespacada(mut self, sim: bool) -> Self {
        self.mono = sim;
        self
    }

    /// Id do campo de texto (o que recebe o foco).
    pub fn id_campo(&self) -> Id {
        self.id.with("campo")
    }

    /// As sugestões que contêm `texto` (sem diferenciar maiúsculas), na ordem dada.
    pub fn filtrar(sugestoes: &[&str], texto: &str) -> Vec<usize> {
        let alvo = texto.trim().to_lowercase();
        (0..sugestoes.len()).filter(|&i| sugestoes.get(i).is_some_and(|s| s.to_lowercase().contains(&alvo))).collect()
    }

    /// Desenha; a resposta do campo tem `changed()` quando o texto mudou (digitando ou escolhendo uma sugestão).
    pub fn mostrar(self, ui: &mut Ui) -> Response {
        let id_campo = self.id_campo();
        let id_estado = self.id;
        let mut estado: EstadoSugestoes = ui.ctx().data(|d| d.get_temp(id_estado)).unwrap_or_default();
        let filtradas = Self::filtrar(self.sugestoes, self.texto);
        let focado = ui.memory(|m| m.has_focus(id_campo));
        let mut escolhida: Option<usize> = None;

        // Esc com a lista aberta fecha só a lista (o egui já tirou o foco do campo no começo do quadro)
        if estado.aberta && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            estado.aberta = false;
            estado.destaque = None;
            ui.memory_mut(|m| m.request_focus(id_campo));
        }
        if focado {
            let n = filtradas.len();
            let mut destaque = estado.destaque.filter(|&d| d < n);
            let mut aberta = estado.aberta;
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Key { key: Key::ArrowDown, pressed: true, modifiers, .. } if !modifiers.any() && n > 0 => {
                        destaque = Some(if aberta { destaque.map_or(0, |d| (d + 1).min(n - 1)) } else { 0 });
                        aberta = true;
                        false
                    }
                    Event::Key { key: Key::ArrowUp, pressed: true, modifiers, .. }
                        if !modifiers.any() && aberta && n > 0 =>
                    {
                        destaque = Some(destaque.map_or(0, |d| d.saturating_sub(1)));
                        false
                    }
                    Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                        if !modifiers.any() && aberta && destaque.is_some() =>
                    {
                        escolhida = destaque.and_then(|d| filtradas.get(d).copied());
                        false
                    }
                    _ => true,
                })
            });
            estado.aberta = aberta;
            estado.destaque = destaque;
        }

        let texto = self.texto;
        let mut resp = Entrada {
            id_salt: Some(id_campo),
            dica: self.dica,
            largura: self.largura,
            rotulo_acessivel: self.rotulo_acessivel,
            mono: self.mono,
            pastilha: self.pastilha,
            pequeno: self.pequeno,
            ..Entrada::new(&mut *texto)
        }
        .mostrar(ui)
        .response
        .response;
        if resp.changed() {
            estado.aberta = true;
            estado.destaque = None;
        }
        if resp.clicked() {
            estado.aberta = true;
        }
        // o clique numa sugestão tira o foco do campo antes de chegar à lista: a lista fica enquanto o ponteiro
        // está nela
        let ponteiro_na_lista =
            estado.rect_lista.zip(ui.ctx().pointer_latest_pos()).is_some_and(|(r, p)| r.contains(p));
        if !resp.has_focus() && !ponteiro_na_lista {
            estado.aberta = false;
            estado.destaque = None;
        }

        // a lista, no vidro dos menus, embaixo do campo
        let filtradas = Self::filtrar(self.sugestoes, texto);
        estado.rect_lista = None;
        if estado.aberta && !filtradas.is_empty() && escolhida.is_none() {
            let fonte = fonte_campo(self.mono, None);
            let opcoes: Vec<(usize, &str)> =
                filtradas.iter().filter_map(|&i| self.sugestoes.get(i).map(|s| (i, *s))).collect();
            let largura = resp.rect.width();
            let destaque = estado.destaque.filter(|&d| d < opcoes.len());
            let popup = Popup::new(id_estado.with("lista"), ui.ctx().clone(), resp.rect, ui.layer_id())
                .open(true)
                .gap(4.0)
                .width(largura)
                .frame(egui::Frame::NONE);
            if let Some(r) = popup.show(|ui| {
                ui.set_min_width(largura);
                lista(ui, &opcoes, None, destaque, &fonte, largura)
            }) {
                estado.rect_lista = Some(r.response.rect);
                if let Some(i) = r.inner {
                    escolhida = opcoes.get(i).map(|(original, _)| *original);
                }
            }
        }
        if let Some(s) = escolhida.and_then(|i| self.sugestoes.get(i)) {
            // o texto vira a sugestão, com o cursor no fim
            *texto = (*s).to_owned();
            let n = texto.chars().count();
            let mut estado_te = TextEditState::load(ui.ctx(), id_campo).unwrap_or_default();
            estado_te.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(n))));
            estado_te.store(ui.ctx(), id_campo);
            estado.aberta = false;
            estado.destaque = None;
            resp.mark_changed();
            ui.memory_mut(|m| m.request_focus(id_campo));
        }
        ui.ctx().data_mut(|d| d.insert_temp(id_estado, estado));
        resp
    }
}

// ------------------------------------------------------------------ check e radio

/// Lado das caixas de marcar e diâmetro das opções (`width:16px;height:16px`).
const LADO_CAIXA: f32 = 16.0;
/// Raio dos cantos da caixa de marcar (`.est input[type=checkbox]{border-radius:5px}`); a opção
/// (`[type=radio]`) continua redonda.
pub const RAIO_CAIXA: f32 = 5.0;
/// `gap:8px` entre a caixa e o texto (`.op`).
const GAP_CAIXA: f32 = 8.0;

const SEM_SOMBRA: SombraCss = SombraCss { y: 0.0, desfoque: 0.0, espalhar: 0.0, cor: Color32::TRANSPARENT };

/// O volume das caixas de marcar e das opções (`.est input[type=checkbox]`, `[type=radio]`). Desmarcada: o neutro
/// (`--volneu` com as riscas, borda `--btb`, realce `--btluz`) sem a sombra interna, com `0 1px 2px
/// rgba(0,0,0,.3)`; marcada: o degradê do destaque com a borda `pri_borda`, o realce `rgba(255,255,255,.35)` e
/// `0 1px 4px` do destaque a 35%. No hover, `brightness(1.08)`, o anel de 1px e o halo no lugar das sombras: 12px
/// em `--brilho`; na marcada, 14px em `--brilhof`.
fn volume_caixa(p: &Paleta, marcada: bool) -> Volume {
    let e = &p.estudio;
    if marcada {
        Volume {
            fundo: Preenchimento::Primario,
            borda: e.pri_borda,
            realce: rgba(255, 255, 255, 0.35),
            sombra_baixo: Color32::TRANSPARENT,
            sombras: [
                SombraCss { y: 1.0, desfoque: 4.0, espalhar: 0.0, cor: p.anel_aqua_forte.sombra.cor },
                SEM_SOMBRA,
            ],
            halo_desfoque: 14.0,
            halo_cor: e.brilho_forte,
            anel: true,
            sombras_somem_no_hover: true,
            clarear_hover: 1.08,
        }
    } else {
        // a borda própria das desmarcadas (`caixa_borda`, 3:1 contra o fundo; revisão de 15/10/2026, item 7)
        Volume {
            borda: e.caixa_borda,
            sombras: [SombraCss { y: 1.0, desfoque: 2.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.3) }, SEM_SOMBRA],
            sombras_somem_no_hover: true,
            clarear_hover: 1.08,
            ..Volume::neutro(p).sem_sombra_baixo().halo(12.0, e.brilho, true)
        }
    }
}

/// O que vai dentro da caixa marcada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marca {
    /// O ✓ (o `::before` de 8×4px com as bordas de 2px à esquerda e embaixo, `translateY(-1px) rotate(-45deg)`).
    Visto,
    /// O traço do indeterminado.
    Traco,
    /// O ponto de 6px da opção.
    Ponto,
}

impl Marca {
    /// O raio da caixa: a de marcar é quadrada com cantos de 5px ([`RAIO_CAIXA`]); a opção é um círculo.
    fn raio(self) -> f32 {
        match self {
            Marca::Visto | Marca::Traco => RAIO_CAIXA,
            Marca::Ponto => f32::INFINITY,
        }
    }
}

/// Pinta a caixa (`marcada` e `hover` de 0 a 1; quadrada de cantos de 5px ou, na opção, redonda): o volume neutro,
/// o do destaque por cima conforme marca, e a marca branca.
fn pintar_caixa(ui: &Ui, caixa: Rect, marcada: f32, hover: f32, marca: Marca) {
    let p = paleta(ui);
    let fora = pintor_de_fora(ui, caixa);
    let painter = ui.painter();
    let estado = |presenca: f32| EstadoVolume { presenca, hover, pressao: 0.0 };
    let raio = marca.raio();
    if marcada < 1.0 {
        pintar_volume(painter, &fora, p, caixa, raio, &volume_caixa(p, false), estado(1.0));
    }
    if marcada <= 0.0 {
        return;
    }
    pintar_volume(painter, &fora, p, caixa, raio, &volume_caixa(p, true), estado(marcada));
    let branco = Color32::WHITE.gamma_multiply(marcada);
    let c = caixa.center();
    match marca {
        Marca::Visto => {
            // o "L" de 10×6px (8×4 mais as bordas de 2px) girado −45° e subido 1px, com o meio dos traços em
            // (−4, −3) → (−4, 2) → (5, 2) antes de girar
            let pontos = vec![c + vec2(-4.95, -0.29), c + vec2(-1.41, 3.24), c + vec2(4.95, -3.12)];
            painter.add(PathShape::line(pontos, PathStroke::new(2.0, branco)));
        }
        Marca::Traco => {
            painter.line_segment([c + vec2(-4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(2.0, branco));
        }
        Marca::Ponto => {
            painter.circle_filled(c, 3.0, branco);
        }
    }
}

/// Caixa de marcar com o texto ao lado (o `label.op` com o `input[type=checkbox]`): quadrada de 16px com cantos de
/// 5px, `gap:8px`, 12px em `--t`.
pub fn check(ui: &mut Ui, marcado: &mut bool, texto: &str) -> Response {
    Check::new(marcado, texto).mostrar(ui)
}

/// Caixa de marcar com estado indeterminado (o "marcar a página" da grade).
#[must_use = "chame .mostrar(ui)"]
pub struct Check<'a> {
    marcado: &'a mut bool,
    texto: &'a str,
    indeterminado: bool,
    tamanho_texto: f32,
    cor_texto: Option<Color32>,
    rotulo_acessivel: Option<&'a str>,
    icone: Option<Icone>,
}

/// O lado do sólido entre a caixa e o texto (a ficha da tabela na lista do Exportar, `.sk.s16` com `gap:8px`).
const LADO_ICONE_CHECK: f32 = 16.0;

impl<'a> Check<'a> {
    pub fn new(marcado: &'a mut bool, texto: &'a str) -> Self {
        Check {
            marcado,
            texto,
            indeterminado: false,
            tamanho_texto: 12.0,
            cor_texto: None,
            rotulo_acessivel: None,
            icone: None,
        }
    }

    /// O ícone de 16px entre a caixa e o texto, com `gap:8px` de cada lado (a ficha de cada tabela da lista do
    /// Exportar o banco).
    pub fn icone(mut self, icone: Icone) -> Self {
        self.icone = Some(icone);
        self
    }

    /// Indeterminado: o degradê do destaque com um traço branco no lugar do ✓.
    pub fn indeterminado(mut self, sim: bool) -> Self {
        self.indeterminado = sim;
        self
    }

    /// Tamanho do texto (12px, o da interface, por padrão).
    pub fn tamanho_texto(mut self, tamanho: f32) -> Self {
        self.tamanho_texto = tamanho;
        self
    }

    /// Cor do texto (`--t2` nos checks "NULL" da janela da linha).
    pub fn cor_texto(mut self, cor: Color32) -> Self {
        self.cor_texto = Some(cor);
        self
    }

    /// O nome para o leitor de tela, quando o texto visível não basta (o `aria-label` "email é NULL" da ficha da linha,
    /// com o "NULL" na tela).
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo_acessivel = Some(rotulo);
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> Response {
        let p = paleta(ui);
        let cor_texto = self.cor_texto.unwrap_or(p.fg);
        let g = (!self.texto.is_empty())
            .then(|| galley(ui, self.texto, fontes::fonte(self.tamanho_texto, Peso::Normal), cor_texto));
        let largura_icone = if self.icone.is_some() { GAP_CAIXA + LADO_ICONE_CHECK } else { 0.0 };
        let largura = LADO_CAIXA + largura_icone + g.as_ref().map_or(0.0, |g| GAP_CAIXA + g.size().x);
        let altura = g.as_ref().map_or(LADO_CAIXA, |g| g.size().y.max(18.0));
        let (rect, mut resp) = ui.allocate_exact_size(vec2(largura.ceil(), altura), Sense::click());
        if resp.clicked() {
            *self.marcado = !*self.marcado;
            resp.mark_changed();
        }
        let caixa =
            Rect::from_min_size(pos2(rect.left(), rect.center().y - LADO_CAIXA / 2.0), vec2(LADO_CAIXA, LADO_CAIXA));
        // o halo de 14px vai a 3σ = 21px
        if ui.is_rect_visible(rect.expand(24.0)) {
            let tempo = duracao_transicao(ui);
            let hover =
                ui.ctx().animate_bool_with_time(resp.id.with("hover"), ui.is_enabled() && resp.hovered(), tempo);
            let marca = ui.ctx().animate_bool_with_time(
                resp.id.with("marca"),
                *self.marcado || self.indeterminado,
                tempo * 0.5,
            );
            let simbolo = if self.indeterminado { Marca::Traco } else { Marca::Visto };
            pintar_caixa(ui, caixa, marca, hover, simbolo);
            if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                pintura::anel_foco(&pintor_de_fora(ui, caixa), p, caixa, simbolo.raio());
            }
            if let Some(icone) = self.icone {
                let lugar = Rect::from_min_size(
                    pos2(caixa.right() + GAP_CAIXA, rect.center().y - LADO_ICONE_CHECK / 2.0),
                    vec2(LADO_ICONE_CHECK, LADO_ICONE_CHECK),
                );
                crate::icones::pintar(ui.painter(), lugar, icone, cor_texto);
            }
            if let Some(g) = g {
                let x = caixa.right() + largura_icone + GAP_CAIXA;
                ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, cor_texto);
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let marcado = *self.marcado;
        resp.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Checkbox,
                ui.is_enabled(),
                marcado,
                self.rotulo_acessivel.unwrap_or(self.texto),
            )
        });
        resp
    }
}

/// Botão de opção (o `label.op` com o `input[type=radio]`: círculo de 16px; escolhido, com o degradê do destaque e
/// o ponto branco de 6px) com o texto ao lado (12px, `gap:8px`). Devolve a resposta com `changed()` quando passou
/// a ser o escolhido.
pub fn radio(ui: &mut Ui, escolhido: bool, texto: &str) -> Response {
    let p = paleta(ui);
    let g = galley(ui, texto, fontes::fonte(12.0, Peso::Normal), p.fg);
    let largura = LADO_CAIXA + GAP_CAIXA + g.size().x;
    let altura = g.size().y.max(18.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(largura.ceil(), altura), Sense::click());
    if resp.clicked() && !escolhido {
        resp.mark_changed();
    }
    let caixa =
        Rect::from_min_size(pos2(rect.left(), rect.center().y - LADO_CAIXA / 2.0), vec2(LADO_CAIXA, LADO_CAIXA));
    if ui.is_rect_visible(rect.expand(24.0)) {
        let tempo = duracao_transicao(ui);
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), ui.is_enabled() && resp.hovered(), tempo);
        let marca = ui.ctx().animate_bool_with_time(resp.id.with("marca"), escolhido, tempo * 0.5);
        pintar_caixa(ui, caixa, marca, hover, Marca::Ponto);
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(&pintor_de_fora(ui, caixa), p, caixa, Marca::Ponto.raio());
        }
        ui.painter().galley(pos2(caixa.right() + GAP_CAIXA, rect.center().y - g.size().y / 2.0), g, p.fg);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, ui.is_enabled(), escolhido, texto));
    resp
}

/// Monta um `LayoutJob` de uma linha com a fonte do campo (útil para medir textos de opções).
pub fn job_campo(texto: &str, cor: Color32) -> LayoutJob {
    LayoutJob::single_section(texto.to_owned(), crate::pecas::texto::formato(fonte_campo(false, None), cor))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tab_poe_dois_espacos_no_cursor() {
        assert_eq!(recuar("SELECT", 6, 6, false), Some(("SELECT  ".into(), 8, 8)));
        // a seleção numa linha só vira os dois espaços
        assert_eq!(recuar("SELECT *", 7, 8, false), Some(("SELECT   ".into(), 9, 9)));
        // acentos: as posições são em caracteres
        assert_eq!(recuar("ação", 1, 1, false), Some(("a  ção".into(), 3, 3)));
    }

    #[test]
    fn tab_com_varias_linhas_recua_as_linhas_inteiras() {
        assert_eq!(recuar("a\nb", 0, 3, false), Some(("  a\n  b".into(), 0, 7)));
        // a seleção que termina logo depois de uma quebra não leva a linha seguinte
        assert_eq!(recuar("a\nb\nc", 0, 2, false), Some(("  a\nb\nc".into(), 0, 3)));
        // do meio da primeira linha ao meio da segunda: as duas linhas inteiras
        assert_eq!(recuar("xa\nyb\nz", 1, 4, false), Some(("  xa\n  yb\nz".into(), 0, 9)));
    }

    #[test]
    fn shift_tab_tira_o_recuo() {
        assert_eq!(recuar("  a\n  b", 0, 7, true), Some(("a\nb".into(), 0, 3)));
        assert_eq!(recuar("  abc", 4, 4, true), Some(("abc".into(), 2, 2)));
        assert_eq!(recuar(" x", 1, 1, true), Some(("x".into(), 0, 0)));
        assert_eq!(recuar("\tx", 2, 2, true), Some(("x".into(), 1, 1)));
        // o cursor no recuo não passa para a linha de cima
        assert_eq!(recuar("a\n  b", 3, 3, true), Some(("a\nb".into(), 2, 2)));
        assert_eq!(recuar("abc", 1, 1, true), None, "sem recuo, nada muda");
    }

    #[test]
    fn sugestoes_filtram_sem_diferenciar_maiusculas() {
        let tipos = ["INTEGER", "TEXT", "REAL", "NUMERIC", "BLOB", "DATE", "DATETIME", "BOOLEAN", "VARCHAR(255)"];
        assert_eq!(EntradaSugestoes::filtrar(&tipos, ""), (0..tipos.len()).collect::<Vec<_>>());
        assert_eq!(EntradaSugestoes::filtrar(&tipos, "te"), vec![0, 1, 5, 6]);
        assert_eq!(EntradaSugestoes::filtrar(&tipos, "  real "), vec![2]);
        assert!(EntradaSugestoes::filtrar(&tipos, "xyz").is_empty());
    }

    #[test]
    fn caixas_seguem_o_css() {
        use crate::tema::{CLARA, ESCURA};
        for p in [&CLARA, &ESCURA] {
            let d = volume_caixa(p, false);
            assert_eq!((d.fundo, d.borda, d.realce), (Preenchimento::Aqua, p.estudio.caixa_borda, p.estudio.btluz));
            assert_eq!(d.sombra_baixo, Color32::TRANSPARENT, "sem a sombra interna do botão");
            assert_eq!((d.halo_desfoque, d.halo_cor, d.clarear_hover), (12.0, p.estudio.brilho, 1.08));
            let m = volume_caixa(p, true);
            assert_eq!((m.fundo, m.borda), (Preenchimento::Primario, p.estudio.pri_borda));
            assert_eq!((m.halo_desfoque, m.halo_cor), (14.0, p.estudio.brilho_forte));
            assert!(d.sombras_somem_no_hover && m.sombras_somem_no_hover);
        }
        // a caixa de marcar é quadrada com cantos de 5px (também com o traço do indeterminado); a opção, redonda
        let caixa = Rect::from_min_size(pos2(0.0, 0.0), vec2(LADO_CAIXA, LADO_CAIXA));
        assert_eq!(pintura::canto(caixa, Marca::Visto.raio()), egui::CornerRadius::same(5));
        assert_eq!(pintura::canto(caixa, Marca::Traco.raio()), egui::CornerRadius::same(5));
        assert_eq!(pintura::canto(caixa, Marca::Ponto.raio()), egui::CornerRadius::same(8));
    }

    #[test]
    fn caixa_desmarcada_com_borda_propria() {
        use crate::tema::{CLARA, ESCURA, rgba};
        // `.est input:not(:checked){border-color:rgba(255,255,255,.35)}` e `.est.claro …{rgba(0,0,0,.5)}`: a borda
        // passa de 3:1 contra o fundo; a marcada fica com a borda do destaque
        assert_eq!(volume_caixa(&ESCURA, false).borda, rgba(255, 255, 255, 0.35));
        assert_eq!(volume_caixa(&CLARA, false).borda, rgba(0, 0, 0, 0.5));
        for p in [&CLARA, &ESCURA] {
            assert_ne!(volume_caixa(p, false).borda, p.estudio.btb, "não é mais a borda dos botões");
            assert_eq!(volume_caixa(p, true).borda, p.estudio.pri_borda);
        }
    }

    #[test]
    fn campos_tem_o_raio_unico_das_superficies() {
        // os campos deixam de ser pílula: o retângulo de cantos de 10px das superfícies, em qualquer altura
        assert_eq!(RAIO_CAMPO, pintura::RAIO_PADRAO);
        for altura in [ALTURA_CAMPO, ALTURA_BUSCA, 60.0] {
            let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, altura));
            assert_eq!(pintura::canto(r, RAIO_CAMPO), egui::CornerRadius::same(10), "{altura}px");
        }
    }

    #[test]
    fn reflexo_do_vidro_para_nos_140px() {
        let alto = Rect::from_min_size(pos2(10.0, 20.0), vec2(200.0, 320.0));
        let Shape::Mesh(m) = forma_reflexo(alto, 12.0) else { panic!("o reflexo é uma malha") };
        assert!(m.is_valid());
        assert!(m.vertices.iter().all(|v| v.pos.y <= 20.0 + 140.0 + 0.01), "cortado nos 140px");
        assert!(m.vertices.iter().all(|v| v.pos.x >= 10.0 - 0.01 && v.pos.x <= 210.0 + 0.01));
        // num vidro baixo, o contorno inteiro
        let baixo = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 96.0));
        let Shape::Mesh(m) = forma_reflexo(baixo, 12.0) else { panic!("o reflexo é uma malha") };
        assert!(m.vertices.iter().any(|v| (v.pos.y - 96.0).abs() < 0.01));
    }
}
