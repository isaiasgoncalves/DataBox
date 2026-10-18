//! Tema do DataBox (visual Estúdio): as paletas clara e escura com os tokens das pranchetas (`design/canvas`; os
//! `color-mix(in oklab, …)` resolvidos para RGBA fixos por `design/ferramentas/tokens_oklab.py`), o [`Tema`]
//! escolhido (Sistema · Claro · Escuro) e [`aplicar`], que configura o `egui` (cores, espaçamentos, fontes,
//! estilos de texto) e passa a seguir o tema do sistema em "Sistema".
//!
//! As telas nunca usam cor solta: pegam a [`Paleta`] do tema em vigor com [`paleta`] (ou [`paleta_ctx`]). Os
//! campos básicos da paleta seguem os nomes `--k-*`; os tokens novos ficam em [`Paleta::estudio`].
//!
//! Tradução do CSS: 1px do CSS = 1 ponto do egui. O vidro das camadas que se sobrepõem (`--flut`: menus, popovers,
//! listas, janelas e o aviso flutuante) é o fosco de [`crate::fosco`], com a foto da janela desfocada embaixo do
//! `--flut` a 78% (escuro) e 80% (claro); sem a foto (as dicas, os testes de tela, a foto que não chegou), o `--flut`
//! liso a 96% ([`crate::pecas::campos::forma_vidro_flutuante`]).

use std::sync::{Arc, LazyLock};

use egui::epaint::FontColorTransferFunction;
use egui::style::{ScrollStyle, Selection, TextCursorStyle, WidgetVisuals, Widgets};
use egui::{
    Color32, Context, CornerRadius, CursorIcon, Margin, Shadow, Stroke, Style, TextStyle, Theme, ThemePreference, Ui,
    Visuals, vec2,
};

use crate::fontes::{self, Peso};

/// RGBA do CSS (`rgba(r,g,b,a)`) como cor do egui (pré-multiplicada), em contexto constante.
pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied_const(r, g, b, (a * 255.0 + 0.5) as u8)
}

/// `#RRGGBB`.
pub const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// O alfa do vidro sem o fundo desfocado embaixo (o fosco desligado, a foto que ainda não chegou, os testes de tela
/// e as dicas): o `--flut` na mesma cor, a 96%. Os 78% (escuro) e 80% (claro) só valem com a textura desfocada
/// embaixo; sem ela, o que está atrás aparece nítido através de um vidro tão ralo e o texto da camada perde a leitura.
pub const ALFA_VIDRO_LISO: f32 = 0.96;

/// A cor com o alfa dado (0 a 1), mantendo o RGB.
pub fn com_alfa(cor: Color32, alfa: f32) -> Color32 {
    let [r, g, b, _] = cor.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (alfa.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
}

/// Uma sombra do CSS (`0 <y>px <desfoque>px <espalhar>px <cor>`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SombraCss {
    pub y: f32,
    pub desfoque: f32,
    pub espalhar: f32,
    pub cor: Color32,
}

/// O anel "aqua" (`--k-aquaRing` / `--k-aquaRingStrong`): fios de 1px nas laterais, em cima e embaixo, uma
/// sombra curta embaixo e o brilho interno.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnelAqua {
    pub lados: Color32,
    pub topo: Color32,
    pub base: Color32,
    pub sombra: SombraCss,
    pub brilho: Color32,
}

/// Um degradê radial do fundo (`--k-bloom`): elipse de raios `raio_x` × `raio_y` centrada em
/// (`centro_x`, `centro_y`) da janela (frações), da `cor` ao transparente em `parada` do raio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bloom {
    pub centro_x: f32,
    pub centro_y: f32,
    pub raio_x: f32,
    pub raio_y: f32,
    pub parada: f32,
    pub cor: Color32,
}

/// Os tokens do visual Estúdio que não cabem nos campos básicos (os das pranchetas do DataBox, `design/canvas`), com os
/// `color-mix(in oklab, …)` resolvidos por `design/ferramentas/tokens_oklab.py` (destaque azul).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estudio {
    /// `--acc` (cor de destaque) e `--acct` (texto na cor de destaque)
    pub acc: Color32,
    pub acct: Color32,
    /// `--painel` (painéis fixos), `--c` (documento), `--p` (faixa e rodapé do documento)
    pub painel: Color32,
    pub doc: Color32,
    pub faixa: Color32,
    /// `--t`, `--t2`, `--t3`
    pub t: Color32,
    pub t2: Color32,
    pub t3: Color32,
    /// `--h` (hover leve), `--b`, `--b2` (fios), `--vidrob` (borda do vidro e dos painéis), `--vidroluz`
    pub h: Color32,
    pub b: Color32,
    pub b2: Color32,
    pub vidrob: Color32,
    pub vidroluz: Color32,
    /// `--f` (fundo do campo) e `--fb` (borda dos trilhos, das barras, das prévias e dos campos desligados)
    pub f: Color32,
    pub fb: Color32,
    /// `--fbc`, a borda dos campos de texto (3:1 contra o fundo; revisão de 15/10/2026, item 2), e o hover dela
    /// (`color-mix(in oklab, --acc 45%, --fbc)`)
    pub fbc: Color32,
    pub fbc_hover: Color32,
    /// A borda das caixas de marcar e das opções desmarcadas (3:1 contra o fundo; revisão, item 7)
    pub caixa_borda: Color32,
    /// `--bt1` → `--bt2` (volume neutro: botões, aba ativa, cabeçalhos) e o meio dos dois (`--sup-meio`)
    pub bt1: Color32,
    pub bt2: Color32,
    pub sup_meio: Color32,
    /// `--btb` (borda dos botões), `--btluz` (realce de 1px em cima)
    pub btb: Color32,
    pub btluz: Color32,
    /// `--btsomb` (duas sombras)
    pub btsomb: [SombraCss; 2],
    /// `--volpri` (topo, o destaque a 25%, base) e a borda do botão principal (destaque 65% com preto)
    pub pri_topo: Color32,
    pub pri_base: Color32,
    pub pri_borda: Color32,
    /// `--volrisco` (topo, meio, base) e a borda
    pub risco_topo: Color32,
    pub risco_meio: Color32,
    pub risco_base: Color32,
    pub risco_borda: Color32,
    /// `--brilho` (halo do hover), `--brilhof` (halo forte), o brilho fraco dos itens selecionados
    pub brilho: Color32,
    pub brilho_forte: Color32,
    pub brilho_item: Color32,
    /// `--sel`, `--selh`, `--tint`
    pub sel: Color32,
    pub selh: Color32,
    pub tint: Color32,
    /// borda da etiqueta de filtro (destaque a 45%); o campo no hover usa o [`Estudio::fbc_hover`]
    pub borda_chip: Color32,
    /// `--flut` (vidro dos menus, janelas e avisos flutuantes) e `--veu` (o fundo atrás da janela)
    pub flut: Color32,
    pub veu: Color32,
    /// `--sh` (sombra das camadas flutuantes) e a sombra curta dos painéis
    pub sh: SombraCss,
    pub sombra_painel: SombraCss,
    /// sombra interna do campo (`inset 0 1px 2px`)
    pub campo_inset: Color32,
    /// cor das riscas de 135° (`--textura`)
    pub riscas: Color32,
    /// `--perigo`, `--ok`, `--aviso`
    pub perigo: Color32,
    pub ok: Color32,
    pub aviso: Color32,
    /// O texto de ok em pílula (`.pill.ok`): no claro, `#0b6a41` (4,78:1; revisão, item 30); no escuro, o `--ok`
    pub ok_texto: Color32,
    /// sintaxe: `--kw` palavra-chave, `--nu` número, `--st` texto, `--chave`
    pub kw: Color32,
    pub nu: Color32,
    pub st: Color32,
    pub chave: Color32,
    /// cores dos arquivos no Explorador: `--csv`, `--xls`, `--json`, `--txt`
    pub csv: Color32,
    pub xls: Color32,
    pub json: Color32,
    pub txt: Color32,
}

impl Estudio {
    /// O `--flut` sem o fundo desfocado embaixo: a mesma cor a [`ALFA_VIDRO_LISO`] (96%). É o fundo de toda camada de
    /// vidro enquanto não há textura desfocada para pôr embaixo (e das dicas, que ficam sempre no vidro liso).
    pub fn flut_liso(&self) -> Color32 {
        com_alfa(self.flut, ALFA_VIDRO_LISO)
    }
}

/// Todas as cores do tema. Os nomes dos campos seguem os tokens `--k-*`; os valores são os do visual
/// Estúdio (o campo diz de que token vem). Os tokens novos ficam em
/// [`Paleta::estudio`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paleta {
    /// Tokens do visual Estúdio.
    pub estudio: Estudio,
    pub escuro: bool,
    /// `--k-bg`
    pub bg: Color32,
    /// Degradê linear de fundo do tema escuro (`#0E2B4A 0%, #0A2039 55%, #081A2E 100%`); no claro, `bg`.
    pub fundo_topo: Color32,
    pub fundo_meio: Color32,
    pub fundo_base: Color32,
    /// `--k-bloom` (dois ou três degradês radiais).
    pub blooms: [Option<Bloom>; 3],
    /// `--k-fg`, `--k-muted`, `--k-faint`
    pub fg: Color32,
    pub muted: Color32,
    pub faint: Color32,
    /// `--k-surface` (degradê vertical de cima para baixo; no escuro é liso).
    pub surface_topo: Color32,
    pub surface_base: Color32,
    /// `--k-surface2`, `--k-solid`, `--k-modal`, `--k-navBg`
    pub surface2: Color32,
    pub solid: Color32,
    pub modal: Color32,
    pub nav_bg: Color32,
    /// Fundo do `th` grudado na grade (`--k-surface2` sobre `--k-solid`).
    pub th_grudado: Color32,
    /// `--k-border`, `--k-borderStrong`, `--k-hairline`, `--k-headRule`, `--k-rowHover`, `--k-grid`
    pub border: Color32,
    pub border_strong: Color32,
    pub hairline: Color32,
    pub head_rule: Color32,
    pub row_hover: Color32,
    pub grid: Color32,
    /// `--k-shadow`, `--k-shadowLg`
    pub sombra: SombraCss,
    pub sombra_lg: SombraCss,
    /// `--k-spec` (brilho de 1px por dentro, em cima)
    pub spec: Color32,
    /// `--k-field`, `--k-fieldBorder`, cor de `--k-fieldInset`
    pub field: Color32,
    pub field_border: Color32,
    pub field_inset: Color32,
    /// `--k-focus` (anel de 3px)
    pub focus: Color32,
    /// `--k-gold`, `--k-goldText`, `--k-line`, `--k-dot`, `--k-sheen`
    pub gold: Color32,
    pub gold_text: Color32,
    pub line: Color32,
    pub dot: Color32,
    pub sheen: Color32,
    /// `--k-aquaFg` e o degradê de `--k-aquaBg` (topo, 20%–50%, base)
    pub aqua_fg: Color32,
    pub aqua_topo: Color32,
    pub aqua_meio: Color32,
    pub aqua_base: Color32,
    /// `--k-aquaRing`, `--k-aquaRingStrong`
    pub anel_aqua: AnelAqua,
    pub anel_aqua_forte: AnelAqua,
    /// `--k-aquaPrimaryBg` (topo, 22%–55%, base) e `--k-aquaPrimaryFg`
    pub primario_topo: Color32,
    pub primario_meio: Color32,
    pub primario_base: Color32,
    pub primario_fg: Color32,
    /// `--k-aquaGoldBg`, `--k-goldOnAqua`
    pub aqua_gold_bg: Color32,
    pub gold_on_aqua: Color32,
    /// `--k-aquaOkBg`, `--k-okFg`
    pub aqua_ok_bg: Color32,
    pub ok_fg: Color32,
    /// `--k-aquaWaitBg`, `--k-waitFg`
    pub aqua_wait_bg: Color32,
    pub wait_fg: Color32,
    /// `--k-aquaDangerBg`, `--k-dangerFg`
    pub aqua_danger_bg: Color32,
    pub danger_fg: Color32,
    /// `--k-aquaNavyBg`
    pub aqua_navy_bg: Color32,
    /// `--k-trackBg` e a cor de `--k-trackRing` (sombra interna de 1px em cima)
    pub track_bg: Color32,
    pub track_ring: Color32,
    /// `--k-barSoft`, `--k-barOk`
    pub bar_soft: Color32,
    pub bar_ok: Color32,
    /// Fundo atrás da janela (`dialog.janela::backdrop`).
    pub backdrop: Color32,
    /// Toast: fundo `--k-fg`, texto `--k-bg`.
    pub toast_bg: Color32,
    pub toast_fg: Color32,
    pub toast_sombra: SombraCss,
    /// Seleção de texto.
    pub selecao: Color32,
}

const ESTUDIO_CLARO: Estudio = Estudio {
    acc: rgb(0x2C, 0x6F, 0xDC),
    acct: rgb(0x21, 0x58, 0xB1),
    painel: rgb(0xF4, 0xF5, 0xF7),
    doc: rgb(0xFF, 0xFF, 0xFF),
    faixa: rgb(0xF6, 0xF7, 0xF9),
    t: rgb(0x1B, 0x1C, 0x1F),
    t2: rgb(0x45, 0x47, 0x4D),
    t3: rgb(0x62, 0x65, 0x6C),
    h: rgba(0, 0, 0, 0.05),
    b: rgba(0, 0, 0, 0.07),
    b2: rgba(0, 0, 0, 0.12),
    vidrob: rgba(0, 0, 0, 0.08),
    vidroluz: rgba(255, 255, 255, 0.8),
    f: rgb(0xFF, 0xFF, 0xFF),
    fb: rgba(0, 0, 0, 0.16),
    fbc: rgba(0, 0, 0, 0.46),
    fbc_hover: rgba(19, 58, 120, 0.703),
    caixa_borda: rgba(0, 0, 0, 0.5),
    bt1: rgb(0xFF, 0xFF, 0xFF),
    bt2: rgb(0xE6, 0xE7, 0xEA),
    sup_meio: rgb(0xF2, 0xF3, 0xF4),
    btb: rgba(0, 0, 0, 0.18),
    btluz: rgba(255, 255, 255, 0.95),
    btsomb: [
        SombraCss { y: 1.0, desfoque: 1.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.12) },
        SombraCss { y: 3.0, desfoque: 8.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.08) },
    ],
    pri_topo: rgb(0x68, 0x99, 0xE9),
    pri_base: rgb(0x1E, 0x51, 0xA3),
    pri_borda: rgb(0x14, 0x3B, 0x7B),
    risco_topo: rgb(0xDE, 0x6A, 0x5E),
    risco_meio: rgb(0xC0, 0x39, 0x2B),
    risco_base: rgb(0xA3, 0x2F, 0x23),
    risco_borda: rgb(0x7F, 0x22, 0x19),
    brilho: rgba(44, 111, 220, 0.45),
    brilho_forte: rgba(44, 111, 220, 0.62),
    brilho_item: rgba(44, 111, 220, 0.22),
    sel: rgba(44, 111, 220, 0.17),
    selh: rgba(44, 111, 220, 0.07),
    tint: rgba(44, 111, 220, 0.11),
    borda_chip: rgba(44, 111, 220, 0.45),
    flut: rgba(255, 255, 255, 0.8),
    veu: rgba(20, 22, 28, 0.22),
    sh: SombraCss { y: 18.0, desfoque: 50.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.16) },
    sombra_painel: SombraCss { y: 1.0, desfoque: 3.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.08) },
    campo_inset: rgba(0, 0, 0, 0.08),
    riscas: rgba(0, 0, 0, 0.02),
    perigo: rgb(0xC4, 0x26, 0x2E),
    ok: rgb(0x0F, 0x7A, 0x4C),
    aviso: rgb(0x8F, 0x54, 0x00),
    ok_texto: rgb(0x0B, 0x6A, 0x41),
    kw: rgb(0x1F, 0x5F, 0xBF),
    nu: rgb(0xA3, 0x52, 0x00),
    st: rgb(0x0F, 0x7A, 0x4C),
    chave: rgb(0x7A, 0x3F, 0xC4),
    csv: rgb(0x11, 0x80, 0x4F),
    xls: rgb(0x16, 0x73, 0x3F),
    json: rgb(0x9A, 0x64, 0x00),
    txt: rgb(0x5D, 0x60, 0x68),
};

const ESTUDIO_ESCURO: Estudio = Estudio {
    acc: rgb(0x2C, 0x6F, 0xDC),
    acct: rgb(0x85, 0xAD, 0xEF),
    painel: rgb(0x1E, 0x1F, 0x22),
    doc: rgb(0x19, 0x1A, 0x1D),
    faixa: rgb(0x21, 0x22, 0x26),
    t: rgb(0xEC, 0xEC, 0xEE),
    t2: rgb(0xB9, 0xBB, 0xC0),
    t3: rgb(0x95, 0x97, 0xA0),
    h: rgba(255, 255, 255, 0.07),
    b: rgba(255, 255, 255, 0.07),
    b2: rgba(255, 255, 255, 0.12),
    vidrob: rgba(255, 255, 255, 0.09),
    vidroluz: rgba(255, 255, 255, 0.06),
    f: rgb(0x14, 0x15, 0x18),
    fb: rgba(255, 255, 255, 0.14),
    fbc: rgba(255, 255, 255, 0.40),
    fbc_hover: rgba(114, 160, 235, 0.67),
    caixa_borda: rgba(255, 255, 255, 0.35),
    bt1: rgb(0x3B, 0x3C, 0x40),
    bt2: rgb(0x26, 0x27, 0x2A),
    sup_meio: rgb(0x30, 0x31, 0x35),
    btb: rgba(0, 0, 0, 0.6),
    btluz: rgba(255, 255, 255, 0.13),
    btsomb: [
        SombraCss { y: 1.0, desfoque: 1.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.5) },
        SombraCss { y: 3.0, desfoque: 8.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.3) },
    ],
    pri_topo: rgb(0x68, 0x99, 0xE9),
    pri_base: rgb(0x1E, 0x51, 0xA3),
    pri_borda: rgb(0x14, 0x3B, 0x7B),
    risco_topo: rgb(0xDE, 0x6A, 0x5E),
    risco_meio: rgb(0xC0, 0x39, 0x2B),
    risco_base: rgb(0xA3, 0x2F, 0x23),
    risco_borda: rgb(0x7F, 0x22, 0x19),
    brilho: rgba(44, 111, 220, 0.6),
    brilho_forte: rgba(44, 111, 220, 0.78),
    brilho_item: rgba(44, 111, 220, 0.22),
    sel: rgba(44, 111, 220, 0.3),
    selh: rgba(44, 111, 220, 0.12),
    tint: rgba(44, 111, 220, 0.2),
    borda_chip: rgba(44, 111, 220, 0.45),
    flut: rgba(30, 31, 35, 0.78),
    veu: rgba(6, 7, 9, 0.45),
    sh: SombraCss { y: 18.0, desfoque: 50.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.55) },
    sombra_painel: SombraCss { y: 1.0, desfoque: 3.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.2) },
    campo_inset: rgba(0, 0, 0, 0.3),
    riscas: rgba(255, 255, 255, 0.022),
    perigo: rgb(0xFF, 0x8A, 0x80),
    ok: rgb(0x5F, 0xD0, 0x9A),
    aviso: rgb(0xF2, 0xB6, 0x5E),
    ok_texto: rgb(0x5F, 0xD0, 0x9A),
    kw: rgb(0x8A, 0xB4, 0xF8),
    nu: rgb(0xF2, 0xB6, 0x5E),
    st: rgb(0x7F, 0xD1, 0xA8),
    chave: rgb(0xC9, 0xA0, 0xFF),
    csv: rgb(0x4C, 0xC3, 0x8A),
    xls: rgb(0x3F, 0xAE, 0x6A),
    json: rgb(0xE8, 0xB0, 0x4B),
    txt: rgb(0xA8, 0xAB, 0xB3),
};

/// O anel dos botões neutros (`border:1px solid --btb`, `inset 0 1px 0 --btluz`, a primeira sombra de `--btsomb`).
const fn anel_botao(e: &Estudio) -> AnelAqua {
    AnelAqua { lados: e.btb, topo: e.btb, base: e.btb, sombra: e.btsomb[0], brilho: e.btluz }
}

/// Tema claro (`.est.claro` das pranchetas).
pub const CLARA: Paleta = Paleta {
    estudio: ESTUDIO_CLARO,
    escuro: false,
    bg: rgb(0xE3, 0xE5, 0xE9),
    fundo_topo: rgb(0xE3, 0xE5, 0xE9),
    fundo_meio: rgb(0xE3, 0xE5, 0xE9),
    fundo_base: rgb(0xE3, 0xE5, 0xE9),
    blooms: [None, None, None],
    fg: ESTUDIO_CLARO.t,
    muted: ESTUDIO_CLARO.t2,
    faint: ESTUDIO_CLARO.t3,
    surface_topo: ESTUDIO_CLARO.doc,
    surface_base: ESTUDIO_CLARO.doc,
    surface2: ESTUDIO_CLARO.faixa,
    solid: ESTUDIO_CLARO.doc,
    modal: ESTUDIO_CLARO.flut,
    nav_bg: ESTUDIO_CLARO.painel,
    th_grudado: ESTUDIO_CLARO.sup_meio,
    border: ESTUDIO_CLARO.vidrob,
    border_strong: ESTUDIO_CLARO.b2,
    hairline: ESTUDIO_CLARO.b,
    head_rule: ESTUDIO_CLARO.b2,
    row_hover: ESTUDIO_CLARO.selh,
    grid: ESTUDIO_CLARO.b,
    sombra: ESTUDIO_CLARO.sombra_painel,
    sombra_lg: ESTUDIO_CLARO.sh,
    spec: ESTUDIO_CLARO.vidroluz,
    field: ESTUDIO_CLARO.f,
    field_border: ESTUDIO_CLARO.fb,
    field_inset: ESTUDIO_CLARO.campo_inset,
    focus: ESTUDIO_CLARO.brilho,
    gold: ESTUDIO_CLARO.acc,
    gold_text: ESTUDIO_CLARO.acct,
    line: ESTUDIO_CLARO.acc,
    dot: ESTUDIO_CLARO.acc,
    sheen: ESTUDIO_CLARO.btluz,
    aqua_fg: ESTUDIO_CLARO.t,
    aqua_topo: ESTUDIO_CLARO.bt1,
    aqua_meio: ESTUDIO_CLARO.sup_meio,
    aqua_base: ESTUDIO_CLARO.bt2,
    anel_aqua: anel_botao(&ESTUDIO_CLARO),
    anel_aqua_forte: AnelAqua {
        lados: ESTUDIO_CLARO.pri_borda,
        topo: ESTUDIO_CLARO.pri_borda,
        base: ESTUDIO_CLARO.pri_borda,
        sombra: SombraCss { y: 4.0, desfoque: 12.0, espalhar: 0.0, cor: rgba(44, 111, 220, 0.35) },
        brilho: rgba(255, 255, 255, 0.38),
    },
    primario_topo: ESTUDIO_CLARO.pri_topo,
    primario_meio: ESTUDIO_CLARO.acc,
    primario_base: ESTUDIO_CLARO.pri_base,
    primario_fg: rgb(0xFF, 0xFF, 0xFF),
    aqua_gold_bg: ESTUDIO_CLARO.tint,
    gold_on_aqua: ESTUDIO_CLARO.acct,
    aqua_ok_bg: rgba(15, 122, 76, 0.18),
    ok_fg: ESTUDIO_CLARO.ok,
    aqua_wait_bg: rgba(143, 84, 0, 0.12),
    wait_fg: ESTUDIO_CLARO.aviso,
    aqua_danger_bg: rgba(196, 38, 46, 0.10),
    danger_fg: ESTUDIO_CLARO.perigo,
    aqua_navy_bg: ESTUDIO_CLARO.h,
    track_bg: ESTUDIO_CLARO.f,
    track_ring: rgba(0, 0, 0, 0.15),
    bar_soft: rgba(44, 111, 220, 0.45),
    bar_ok: rgba(15, 122, 76, 0.65),
    backdrop: ESTUDIO_CLARO.veu,
    toast_bg: rgb(0xFC, 0xFC, 0xFD),
    toast_fg: ESTUDIO_CLARO.t,
    toast_sombra: ESTUDIO_CLARO.sh,
    selecao: ESTUDIO_CLARO.sel,
};

/// Tema escuro (`.est` das pranchetas).
pub const ESCURA: Paleta = Paleta {
    estudio: ESTUDIO_ESCURO,
    escuro: true,
    bg: rgb(0x11, 0x12, 0x14),
    fundo_topo: rgb(0x11, 0x12, 0x14),
    fundo_meio: rgb(0x11, 0x12, 0x14),
    fundo_base: rgb(0x11, 0x12, 0x14),
    blooms: [None, None, None],
    fg: ESTUDIO_ESCURO.t,
    muted: ESTUDIO_ESCURO.t2,
    faint: ESTUDIO_ESCURO.t3,
    surface_topo: ESTUDIO_ESCURO.doc,
    surface_base: ESTUDIO_ESCURO.doc,
    surface2: ESTUDIO_ESCURO.faixa,
    solid: ESTUDIO_ESCURO.doc,
    modal: ESTUDIO_ESCURO.flut,
    nav_bg: ESTUDIO_ESCURO.painel,
    th_grudado: ESTUDIO_ESCURO.sup_meio,
    border: ESTUDIO_ESCURO.vidrob,
    border_strong: ESTUDIO_ESCURO.b2,
    hairline: ESTUDIO_ESCURO.b,
    head_rule: ESTUDIO_ESCURO.b2,
    row_hover: ESTUDIO_ESCURO.selh,
    grid: ESTUDIO_ESCURO.b,
    sombra: ESTUDIO_ESCURO.sombra_painel,
    sombra_lg: ESTUDIO_ESCURO.sh,
    spec: ESTUDIO_ESCURO.vidroluz,
    field: ESTUDIO_ESCURO.f,
    field_border: ESTUDIO_ESCURO.fb,
    field_inset: ESTUDIO_ESCURO.campo_inset,
    focus: ESTUDIO_ESCURO.brilho,
    gold: ESTUDIO_ESCURO.acc,
    gold_text: ESTUDIO_ESCURO.acct,
    line: ESTUDIO_ESCURO.acc,
    dot: ESTUDIO_ESCURO.acc,
    sheen: ESTUDIO_ESCURO.btluz,
    aqua_fg: ESTUDIO_ESCURO.t,
    aqua_topo: ESTUDIO_ESCURO.bt1,
    aqua_meio: ESTUDIO_ESCURO.sup_meio,
    aqua_base: ESTUDIO_ESCURO.bt2,
    anel_aqua: anel_botao(&ESTUDIO_ESCURO),
    anel_aqua_forte: AnelAqua {
        lados: ESTUDIO_ESCURO.pri_borda,
        topo: ESTUDIO_ESCURO.pri_borda,
        base: ESTUDIO_ESCURO.pri_borda,
        sombra: SombraCss { y: 4.0, desfoque: 12.0, espalhar: 0.0, cor: rgba(44, 111, 220, 0.35) },
        brilho: rgba(255, 255, 255, 0.38),
    },
    primario_topo: ESTUDIO_ESCURO.pri_topo,
    primario_meio: ESTUDIO_ESCURO.acc,
    primario_base: ESTUDIO_ESCURO.pri_base,
    primario_fg: rgb(0xFF, 0xFF, 0xFF),
    aqua_gold_bg: ESTUDIO_ESCURO.tint,
    gold_on_aqua: ESTUDIO_ESCURO.acct,
    aqua_ok_bg: rgba(95, 208, 154, 0.18),
    ok_fg: ESTUDIO_ESCURO.ok,
    aqua_wait_bg: rgba(242, 182, 94, 0.12),
    wait_fg: ESTUDIO_ESCURO.aviso,
    aqua_danger_bg: rgba(255, 138, 128, 0.10),
    danger_fg: ESTUDIO_ESCURO.perigo,
    aqua_navy_bg: ESTUDIO_ESCURO.h,
    track_bg: ESTUDIO_ESCURO.f,
    track_ring: rgba(0, 0, 0, 0.35),
    bar_soft: rgba(44, 111, 220, 0.45),
    bar_ok: rgba(95, 208, 154, 0.65),
    backdrop: ESTUDIO_ESCURO.veu,
    toast_bg: rgb(0x1C, 0x1D, 0x21),
    toast_fg: ESTUDIO_ESCURO.t,
    toast_sombra: ESTUDIO_ESCURO.sh,
    selecao: ESTUDIO_ESCURO.sel,
};

/// A paleta do tema em vigor neste `Ui` (claro ou escuro, já resolvido o "Sistema").
pub fn paleta(ui: &Ui) -> &'static Paleta {
    if ui.visuals().dark_mode { &ESCURA } else { &CLARA }
}

/// A paleta do tema em vigor no contexto.
pub fn paleta_ctx(ctx: &Context) -> &'static Paleta {
    match ctx.theme() {
        Theme::Dark => &ESCURA,
        Theme::Light => &CLARA,
    }
}

/// O tema escolhido pela pessoa. Os textos são os do arquivo de preferências (`"system" | "light" | "dark"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tema {
    #[default]
    Sistema,
    Claro,
    Escuro,
}

impl Tema {
    pub fn como_str(self) -> &'static str {
        match self {
            Tema::Sistema => "system",
            Tema::Claro => "light",
            Tema::Escuro => "dark",
        }
    }

    /// `"system" | "light" | "dark"`; qualquer outro texto é `None`.
    pub fn de_str(s: &str) -> Option<Tema> {
        match s {
            "system" => Some(Tema::Sistema),
            "light" => Some(Tema::Claro),
            "dark" => Some(Tema::Escuro),
            _ => None,
        }
    }

    /// Rótulo do seletor de tema ("Sistema", "Claro", "Escuro").
    pub fn rotulo(self) -> &'static str {
        match self {
            Tema::Sistema => "Sistema",
            Tema::Claro => "Claro",
            Tema::Escuro => "Escuro",
        }
    }

    /// Dica do botão do seletor (o `title` do mostruário).
    pub fn dica(self) -> &'static str {
        match self {
            Tema::Sistema => "Seguir o tema do sistema",
            Tema::Claro => "Tema claro",
            Tema::Escuro => "Tema escuro",
        }
    }

    pub const TODOS: [Tema; 3] = [Tema::Sistema, Tema::Claro, Tema::Escuro];

    fn preferencia(self) -> ThemePreference {
        match self {
            Tema::Sistema => ThemePreference::System,
            Tema::Claro => ThemePreference::Light,
            Tema::Escuro => ThemePreference::Dark,
        }
    }
}

/// Nomes dos estilos de texto (`TextStyle::Name`). Os do egui também são configurados:
/// `Body` = corpo, `Button` = botão, `Small` = hint, `Monospace` = mono, `Heading` = h2.
pub mod estilos {
    /// Título de página: 20px / 600.
    pub const H2: &str = "h2";
    /// Título de seção, de documento e de janela: 14px / 600.
    pub const H3: &str = "h3";
    /// Corpo e células: 12px / 400.
    pub const CORPO: &str = "corpo";
    /// Texto auxiliar: 11px / 400 (`--t3`).
    pub const HINT: &str = "hint";
    /// Código, SQL e JSON (só dentro dos editores): 12.5px na Consolas.
    pub const MONO: &str = "mono";
    /// Título de seção em caixa alta (`.tit`), rótulo de campo, `h4`: 11px / 600 (com espaçamento; ver
    /// `pecas::rotulo_caps`).
    pub const EYEBROW: &str = "eyebrow";
    /// Botão: 12px / 600.
    pub const BOTAO: &str = "botao";
    /// Campo de texto e select: 12px / 400.
    pub const CAMPO: &str = "campo";
    /// Número grande (os totais da Visão geral): 24px / 350.
    pub const NUMERO: &str = "numero";
}

static ESTILO_H2: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::H2)));
static ESTILO_H3: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::H3)));
static ESTILO_CORPO: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::CORPO)));
static ESTILO_HINT: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::HINT)));
static ESTILO_MONO: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::MONO)));
static ESTILO_EYEBROW: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::EYEBROW)));
static ESTILO_BOTAO: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::BOTAO)));
static ESTILO_CAMPO: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::CAMPO)));
static ESTILO_NUMERO: LazyLock<TextStyle> = LazyLock::new(|| TextStyle::Name(Arc::from(estilos::NUMERO)));

/// `TextStyle` de um dos [`estilos`] (sem alocar: os nomes são criados uma vez só).
pub fn estilo(nome: &str) -> TextStyle {
    match nome {
        estilos::H2 => ESTILO_H2.clone(),
        estilos::H3 => ESTILO_H3.clone(),
        estilos::HINT => ESTILO_HINT.clone(),
        estilos::MONO => ESTILO_MONO.clone(),
        estilos::EYEBROW => ESTILO_EYEBROW.clone(),
        estilos::BOTAO => ESTILO_BOTAO.clone(),
        estilos::CAMPO => ESTILO_CAMPO.clone(),
        estilos::NUMERO => ESTILO_NUMERO.clone(),
        _ => ESTILO_CORPO.clone(),
    }
}

fn estilos_de_texto() -> std::collections::BTreeMap<TextStyle, egui::FontId> {
    let mut m = std::collections::BTreeMap::new();
    m.insert(TextStyle::Body, fontes::fonte(12.0, Peso::Normal));
    m.insert(TextStyle::Button, fontes::fonte(12.0, Peso::Medio));
    m.insert(TextStyle::Small, fontes::fonte(11.0, Peso::Normal));
    m.insert(TextStyle::Monospace, fontes::fonte_mono(12.5));
    m.insert(TextStyle::Heading, fontes::fonte(20.0, Peso::Medio));
    m.insert(ESTILO_H2.clone(), fontes::fonte(20.0, Peso::Medio));
    m.insert(ESTILO_H3.clone(), fontes::fonte(14.0, Peso::Medio));
    m.insert(ESTILO_CORPO.clone(), fontes::fonte(12.0, Peso::Normal));
    m.insert(ESTILO_HINT.clone(), fontes::fonte(11.0, Peso::Normal));
    m.insert(ESTILO_MONO.clone(), fontes::fonte_mono(12.5));
    m.insert(ESTILO_EYEBROW.clone(), fontes::fonte(11.0, Peso::Medio));
    m.insert(ESTILO_BOTAO.clone(), fontes::fonte(12.0, Peso::Medio));
    m.insert(ESTILO_CAMPO.clone(), fontes::fonte(12.0, Peso::Normal));
    m.insert(ESTILO_NUMERO.clone(), fontes::fonte(24.0, Peso::Leve));
    m
}

fn sombra_egui(s: SombraCss) -> Shadow {
    Shadow {
        offset: [0, s.y.round() as i8],
        blur: (s.desfoque.min(255.0)) as u8,
        spread: s.espalhar.max(0.0) as u8,
        color: s.cor,
    }
}

fn widget(fundo: Color32, fundo_fraco: Color32, borda: Color32, texto: Color32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: fundo,
        weak_bg_fill: fundo_fraco,
        bg_stroke: Stroke::new(1.0, borda),
        corner_radius: CornerRadius::same(6),
        fg_stroke: Stroke::new(1.0, texto),
        expansion: 0.0,
    }
}

/// Quanto engrossar o texto (o expoente do `FontColorTransferFunction::Gamma`: alfa = cobertura^γ) no tema
/// claro. O Edge (DirectWrite) desenha a Segoe UI com um pouco mais de tinta que a cobertura geométrica pura do
/// egui; o valor aproxima as pranchetas do canvas vistas no Edge.
pub const GAMA_TEXTO_CLARO: f32 = 0.8;
/// O mesmo para o tema escuro: texto claro sobre fundo escuro pede um pouco mais de tinta. O atlas de glifos é
/// um só por tema, e o mesmo γ vale para o texto branco dos botões na cor de destaque.
pub const GAMA_TEXTO_ESCURO: f32 = 0.6;

/// O `Visuals` do egui para uma paleta (vale para os widgets do próprio egui que aparecerem: barras de
/// rolagem, dicas, separadores, seleção de texto).
pub fn visuais(p: &Paleta) -> Visuals {
    let mut v = if p.escuro { Visuals::dark() } else { Visuals::light() };
    v.dark_mode = p.escuro;
    v.text_options.color_transfer_function =
        FontColorTransferFunction::Gamma(if p.escuro { GAMA_TEXTO_ESCURO } else { GAMA_TEXTO_CLARO });
    v.override_text_color = None;
    v.weak_text_color = Some(p.faint);
    v.hyperlink_color = p.gold_text;
    v.faint_bg_color = p.row_hover;
    v.extreme_bg_color = p.field;
    v.text_edit_bg_color = Some(p.field);
    v.code_bg_color = p.track_bg;
    v.warn_fg_color = p.wait_fg;
    v.error_fg_color = p.danger_fg;
    // o raio único das superfícies (janelas, cartões, painéis)
    v.window_corner_radius = CornerRadius::same(crate::pintura::RAIO_PADRAO as u8);
    v.window_shadow = sombra_egui(p.sombra_lg);
    // as dicas e camadas do próprio egui no vidro liso das nossas (`--flut` a 96%): dica não leva foto de tela cheia
    v.window_fill = p.estudio.flut_liso();
    v.window_stroke = Stroke::new(1.0, p.estudio.vidrob);
    v.window_highlight_topmost = false;
    v.menu_corner_radius = CornerRadius::same(12);
    v.panel_fill = p.bg;
    v.popup_shadow = sombra_egui(p.sombra_lg);
    // O texto selecionado mantém a cor do texto sobre o fundo da seleção (`--sel`), como no navegador.
    v.selection = Selection { bg_fill: p.selecao, stroke: Stroke::new(1.0, p.fg) };
    v.text_cursor = TextCursorStyle { stroke: Stroke::new(1.5, p.fg), ..TextCursorStyle::default() };
    v.interact_cursor = Some(CursorIcon::PointingHand);
    v.button_frame = true;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;
    v.striped = false;
    v.image_loading_spinners = false;
    v.disabled_alpha = 0.45;
    v.widgets = Widgets {
        noninteractive: widget(p.solid, Color32::TRANSPARENT, p.hairline, p.fg),
        inactive: widget(p.border_strong, p.aqua_meio, p.field_border, p.fg),
        hovered: widget(p.faint, p.aqua_meio, p.border_strong, p.fg),
        active: widget(p.muted, p.aqua_meio, p.gold, p.fg),
        open: widget(p.faint, p.aqua_meio, p.border_strong, p.fg),
    };
    v
}

/// O `Style` completo para uma paleta: visuais, estilos de texto e espaçamentos das pranchetas.
pub fn estilo_completo(p: &Paleta) -> Style {
    let mut s = Style { visuals: visuais(p), text_styles: estilos_de_texto(), ..Style::default() };
    s.spacing.item_spacing = vec2(8.0, 8.0);
    s.spacing.button_padding = vec2(15.0, 5.0);
    s.spacing.interact_size = vec2(28.0, 28.0);
    s.spacing.menu_margin = Margin::same(5);
    s.spacing.window_margin = Margin::same(0);
    s.spacing.text_edit_width = 280.0;
    s.spacing.tooltip_width = 440.0;
    s.spacing.icon_width = 16.0;
    s.spacing.icon_width_inner = 10.0;
    s.spacing.icon_spacing = 8.0;
    s.spacing.combo_height = 320.0;
    s.spacing.indent = 18.0;
    let mut rolagem = ScrollStyle::thin();
    rolagem.bar_width = 8.0;
    rolagem.floating_width = 6.0;
    rolagem.floating_allocated_width = 0.0;
    rolagem.content_margin = Margin::ZERO;
    rolagem.dormant_background_opacity = 0.0;
    rolagem.active_background_opacity = 0.0;
    rolagem.interact_background_opacity = 0.0;
    rolagem.dormant_handle_opacity = 1.0;
    rolagem.active_handle_opacity = 1.0;
    rolagem.interact_handle_opacity = 1.0;
    rolagem.fade.strength = 0.0;
    s.spacing.scroll = rolagem;
    s.animation_time = 0.2;
    s.explanation_tooltips = false;
    s.url_in_tooltip = false;
    s
}

/// Chave do que já foi aplicado neste contexto (para `aplicar` poder ser chamado a cada quadro sem custo).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Aplicado {
    tema: Tema,
}

fn id_aplicado() -> egui::Id {
    egui::Id::new("databox::tema::aplicado")
}

/// Configura o contexto no tema pedido: os dois estilos (claro e escuro, com as cores, os espaçamentos e os
/// estilos de texto), as fontes, as texturas dos degradês e das riscas e a preferência de tema do egui (em
/// [`Tema::Sistema`] o egui segue o sistema sozinho).
///
/// Fontes: na primeira chamada, as do sistema (Segoe UI, Consolas, Segoe UI Symbol) são lidas do disco na hora
/// ([`crate::fontes::instalar`], ~5 MB): é a única leitura de disco na thread da interface, e acontece antes do
/// primeiro quadro (no criador do app), para o primeiro quadro já sair com a fonte certa.
///
/// Chame uma vez antes do primeiro quadro (no criador do app do eframe) e de novo quando a pessoa trocar o
/// tema. Chamar a cada quadro também serve: sem mudança, não faz nada. As fontes novas só valem a partir da
/// passada seguinte (regra do egui); use [`crate::fontes::instaladas`] se precisar saber.
pub fn aplicar(ctx: &Context, tema: Tema) {
    let antes: Option<Aplicado> = ctx.data(|d| d.get_temp(id_aplicado()));
    if antes == Some(Aplicado { tema }) {
        return;
    }
    if antes.is_none() {
        fontes::instalar(ctx);
        crate::pintura::criar_texturas(ctx);
    }
    ctx.set_style_of(Theme::Light, estilo_completo(&CLARA));
    ctx.set_style_of(Theme::Dark, estilo_completo(&ESCURA));
    // o tamanho do código escolhido nas Preferências (o `estilo_completo` traz o padrão, 12,5px)
    let tamanho_codigo = crate::codigo::atual(ctx).tamanho;
    if tamanho_codigo != crate::codigo::TAMANHO_PADRAO {
        crate::codigo::aplicar_tamanho(ctx, tamanho_codigo);
    }
    ctx.set_theme(tema.preferencia());
    ctx.data_mut(|d| d.insert_temp(id_aplicado(), Aplicado { tema }));
}

/// O tema aplicado por último neste contexto ([`Tema::Sistema`] se nada foi aplicado).
pub fn tema_aplicado(ctx: &Context) -> Tema {
    ctx.data(|d| d.get_temp::<Aplicado>(id_aplicado())).map_or(Tema::Sistema, |a| a.tema)
}

fn id_movimento() -> egui::Id {
    egui::Id::new("databox::tema::movimento_reduzido")
}

/// Liga ou desliga o "movimento reduzido" (o `prefers-reduced-motion` do CSS): sem transições, o carregando
/// fica parado e a barra indeterminada fica cheia a 45%.
pub fn definir_movimento_reduzido(ctx: &Context, reduzido: bool) {
    ctx.data_mut(|d| d.insert_temp(id_movimento(), reduzido));
}

/// Movimento reduzido ligado? Também vale quando o estilo não tem animação (`animation_time` zero, como nos
/// testes com `egui_kittest`).
pub fn movimento_reduzido(ui: &Ui) -> bool {
    ui.style().animation_time <= 0.0 || movimento_reduzido_pedido(ui.ctx())
}

/// [`movimento_reduzido`] para quem só tem o contexto (o toast, o "salvo" da célula): o mesmo indicador de
/// [`definir_movimento_reduzido`], ou o estilo global sem animação.
pub fn movimento_reduzido_ctx(ctx: &Context) -> bool {
    ctx.global_style().animation_time <= 0.0 || movimento_reduzido_pedido(ctx)
}

fn movimento_reduzido_pedido(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(id_movimento())).unwrap_or(false)
}

/// Duração das transições (200ms; zero com movimento reduzido).
pub fn duracao_transicao(ui: &Ui) -> f32 {
    if movimento_reduzido(ui) { 0.0 } else { 0.2 }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn rgba_arredonda_o_alfa_como_o_css() {
        assert_eq!(rgba(255, 255, 255, 0.85).a(), 217);
        assert_eq!(rgba(19, 60, 99, 0.16).a(), 41);
        assert_eq!(rgba(0, 0, 0, 0.0), Color32::TRANSPARENT);
        assert_eq!(rgba(19, 60, 99, 1.0), rgb(19, 60, 99));
    }

    #[test]
    fn tokens_principais() {
        // os das pranchetas (`.est` e `.est.claro`)
        assert_eq!(ESCURA.bg, rgb(0x11, 0x12, 0x14));
        assert_eq!(CLARA.bg, rgb(0xE3, 0xE5, 0xE9));
        assert_eq!((ESCURA.fg, CLARA.fg), (rgb(0xEC, 0xEC, 0xEE), rgb(0x1B, 0x1C, 0x1F)));
        // o destaque é o mesmo azul nos dois temas; o texto nele muda
        assert_eq!(ESCURA.estudio.acc, CLARA.estudio.acc);
        assert_eq!(ESCURA.primario_meio, ESCURA.estudio.acc);
        assert_eq!(ESCURA.primario_fg, rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(ESCURA.gold_text, rgb(0x85, 0xAD, 0xEF));
        assert_eq!(CLARA.gold_text, rgb(0x21, 0x58, 0xB1));
        // fundo liso, sem degradês radiais
        assert!(CLARA.blooms.iter().chain(ESCURA.blooms.iter()).all(Option::is_none));
        assert_eq!((CLARA.escuro, ESCURA.escuro), (false, true));
    }

    #[test]
    fn tema_ida_e_volta_pelo_texto() {
        for t in Tema::TODOS {
            assert_eq!(Tema::de_str(t.como_str()), Some(t));
        }
        assert_eq!(Tema::de_str("azul"), None);
        assert_eq!(Tema::default(), Tema::Sistema);
    }

    #[test]
    fn estilo_tem_todos_os_estilos_de_texto() {
        let s = estilo_completo(&CLARA);
        for nome in [
            estilos::H2,
            estilos::H3,
            estilos::CORPO,
            estilos::HINT,
            estilos::MONO,
            estilos::EYEBROW,
            estilos::BOTAO,
            estilos::CAMPO,
            estilos::NUMERO,
        ] {
            assert!(s.text_styles.contains_key(&estilo(nome)), "{nome}");
        }
        assert!(!s.visuals.dark_mode);
        assert!(estilo_completo(&ESCURA).visuals.dark_mode);
        assert_eq!(s.text_styles[&estilo(estilos::H2)].size, 20.0);
        assert_eq!(s.text_styles[&estilo(estilos::CORPO)].size, 12.0);
    }
}
