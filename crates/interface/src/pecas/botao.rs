//! Botões do visual Estúdio (`.btn` e `.ib` das pranchetas, `design/canvas/Main.dc.html`): pílula de 28px
//! ([`ALTURA`]; 30px no rodapé das janelas, o `.dlg footer .btn`, com [`com_altura`]), `padding:0 15px`, texto
//! 12px / 600. Papéis: [`TipoBotao::Normal`] (`.btn`, neutro) navega, abre e adiciona; [`TipoBotao::Primario`]
//! (`.btn.pri`, o degradê da cor de destaque) é a ação principal (no máximo um por documento ou janela;
//! [`TipoBotao::Ouro`] virou sinônimo dele); [`TipoBotao::Ghost`] (`.btn.sutil`) fica em linhas e como
//! "Cancelar"; [`TipoBotao::Perigo`] (`.btn.risco`, vermelho) e [`TipoBotao::GhostPerigo`] (`.btn.sutil.perigo`)
//! removem; [`Botao::redondo`] é o botão só de ícone (`.ib`, círculo de 30px). [`Botao::icone`] é o `.ib` com um
//! [`Icone`] da casca; [`Botao::pequeno`] é o `.ib.pq` de 26px (e o `.btn.pq` de 24px), [`Botao::do_cabecalho`] o
//! `.cab .ib` de 20px dos cabeçalhos de painel, [`Botao::rail`] o botão da
//! barra de ícones (`.rail`, 36px, com a barrinha na cor de destaque quando ligado) e [`Botao::principal`] o
//! `.ib.pri`.
//!
//! O volume ([`Volume`], que as caixas de marcar, o seletor segmentado e os botões de menu também usam): degradê
//! de cima para baixo com as riscas de 135°, borda de 1px, realce claro em cima, sombra interna embaixo e sombra
//! projetada. Hover: brilho de 7% e o halo na cor de destaque (vermelho nos de perigo); pressionado: o botão
//! afunda (sem sombra projetada, sombra interna em cima, brilho de 95%); foco do teclado: o hover e o anel de
//! foco; desligado: 45% de opacidade (30% no redondo). [`Botao::ocupado`] mostra o ponto pulsante do
//! `.carregando` antes do texto (no botão de ícone, o arco girando no lugar do ícone, o `.ib.ocupado` da
//! prancheta 15) e não aceita clique. [`Botao::com_icone`] põe a silhueta de 14px antes do texto (`.btn .sil`) e
//! [`Botao::lista`] é o `.btn.sutil.lista` (largura toda, texto 400 à esquerda, a silhueta em `--t3`).

use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Duration;

use egui::epaint::{PathShape, PathStroke, RectShape};
use egui::{
    Color32, CursorIcon, Id, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Widget, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Familia, Icone};
use crate::pecas::avisos::pintar_ponto_pulsante;
use crate::pecas::texto::{galley, galley_cortado};
use crate::pintura::{self, Preenchimento};
use crate::tema::{Paleta, SombraCss, duracao_transicao, movimento_reduzido, paleta, rgba};

/// Altura dos botões (`.btn{height:28px}`).
pub const ALTURA: f32 = 28.0;

/// Altura dos botões no rodapé das janelas (`.dlg footer .btn`, 30px); ver [`com_altura`].
pub const ALTURA_JANELA: f32 = 30.0;

/// Lado do botão redondo (`.ib`, círculo de 30px).
pub const LADO_REDONDO: f32 = 30.0;

/// Lado do botão de ícone pequeno (`.ib.pq`: o "+" das abas, a paginação).
pub const LADO_PEQUENO: f32 = 26.0;

/// Lado do botão de ícone do cabeçalho dos painéis (`.cab .ib{width:20px;height:20px}`, com a silhueta de 12px:
/// Abrir pasta e Recolher tudo no Explorador; pedido de 16/10/2026).
pub const LADO_CABECALHO: f32 = 20.0;

/// Lado do botão da barra de ícones (`.rail`).
pub const LADO_RAIL: f32 = 36.0;
/// A barrinha do `.rail.on` (`width:3px;border-radius:3px`): uma pílula de 3px, com o raio da metade da largura.
const LARGURA_BARRA_RAIL: f32 = 3.0;
const RAIO_BARRA_RAIL: f32 = LARGURA_BARRA_RAIL / 2.0;

/// Altura do botão de texto pequeno (`.btn.pq{height:24px;padding:0 10px}`).
pub const ALTURA_PEQUENO: f32 = 24.0;

/// `padding:0 15px` do `.btn`.
const PADDING: f32 = 15.0;
/// `padding:0 10px` do `.btn.pq`.
const PADDING_PEQUENO: f32 = 10.0;
/// O padding do botão estreito (o "✕" de remover filtro, o de tirar coluna).
const PADDING_ESTREITO: f32 = 8.0;
/// `gap:6px` entre o ponto do ocupado e o texto.
const GAP: f32 = 6.0;
/// Quanto as sombras e o halo passam da peça (o halo de 24px vai a 3σ = 36px).
const ALCANCE_HALO: f32 = 40.0;

/// O papel do botão (a classe das pranchetas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TipoBotao {
    /// `.btn` (neutro)
    #[default]
    Normal,
    /// `.btn.pri`
    Primario,
    /// O antigo `.btn.gold`: no Estúdio é o mesmo `.btn.pri`.
    Ouro,
    /// `.btn.sutil`
    Ghost,
    /// `.btn.risco`
    Perigo,
    /// `.btn.sutil.perigo`
    GhostPerigo,
}

/// O tamanho do botão (`.pq` e `.rail` das pranchetas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tamanho {
    #[default]
    Normal,
    /// `.btn.pq` (24px, `padding:0 10px`) e `.ib.pq` (26px, ícone de 14px).
    Pequeno,
    /// `.cab .ib` (20px, ícone de 12px).
    Cabecalho,
    /// `.rail` (36px; o sólido de 22px, a silhueta de 18px em `--t3` até o hover).
    Rail,
}

/// Lado da silhueta antes do texto (`.btn .sil{width:14px}`).
const LADO_ICONE_TEXTO: f32 = 14.0;

/// Um botão. Use com `ui.add(Botao::primario("Salvar"))`.
#[must_use = "use ui.add(botao)"]
pub struct Botao<'a> {
    texto: &'a str,
    icone: Option<Icone>,
    /// A silhueta antes do texto ([`Botao::com_icone`]).
    icone_texto: Option<Icone>,
    /// O `.btn.lista` ([`Botao::lista`]).
    lista: bool,
    tipo: TipoBotao,
    redondo: bool,
    tamanho: Tamanho,
    estreito: bool,
    ocupado: bool,
    /// O ocupado com o arco girando ([`Botao::com_arco`]) no lugar do ponto.
    arco: bool,
    habilitado: bool,
    selecionado: bool,
    /// É um botão de alternar (chamaram [`Botao::selecionado`]): o leitor de tela ouve o estado.
    alternar: bool,
    dica: Option<&'a str>,
    rotulo_acessivel: Option<&'a str>,
    largura_min: f32,
}

impl<'a> Botao<'a> {
    /// `.btn`: o neutro (degradê `--bt1` → `--bt2`).
    pub fn new(texto: &'a str) -> Self {
        Botao {
            texto,
            icone: None,
            icone_texto: None,
            lista: false,
            tipo: TipoBotao::Normal,
            redondo: false,
            tamanho: Tamanho::Normal,
            estreito: false,
            ocupado: false,
            arco: false,
            habilitado: true,
            selecionado: false,
            alternar: false,
            dica: None,
            rotulo_acessivel: None,
            largura_min: 0.0,
        }
    }

    /// `.ib` com um ícone da casca (16px; 14px no [`Botao::pequeno`], 12px no [`Botao::do_cabecalho`]; no
    /// [`Botao::rail`], o sólido de 22px ou a silhueta de 18px). O leitor de tela ouve [`Icone::nome`] até
    /// [`Botao::rotulo`] dizer outro ("Nova tabela" no lugar de "Novo").
    pub fn icone(icone: Icone) -> Self {
        let mut b = Botao::redondo(icone.nome());
        b.icone = Some(icone);
        b
    }

    /// A silhueta de 14px antes do texto, com `gap:6px` (`.btn .sil`): na cor do texto, ou em `--t3` no
    /// [`Botao::lista`].
    pub fn com_icone(mut self, icone: Icone) -> Self {
        self.icone_texto = Some(icone);
        self
    }

    /// O `.btn.sutil.lista` (as ações da Manutenção e das Propriedades: "Verificar integridade", "Renomear a
    /// tabela"): 28px, a largura toda, o texto 400 à esquerda (cortado com "…" se não couber) e a silhueta do
    /// [`Botao::com_icone`] em `--t3`. Vira o sutil ([`TipoBotao::Ghost`]); um [`Botao::tipo`] depois troca.
    pub fn lista(mut self) -> Self {
        self.lista = true;
        self.tipo = TipoBotao::Ghost;
        self
    }

    /// O tamanho pequeno: o `.ib.pq` de 26px (ícone de 14px) ou, num botão de texto, o `.btn.pq` de 24px com
    /// `padding:0 10px`.
    pub fn pequeno(mut self) -> Self {
        self.tamanho = Tamanho::Pequeno;
        self
    }

    /// O botão de ícone do cabeçalho dos painéis (`.cab .ib`): círculo de 20px com o ícone de 12px.
    pub fn do_cabecalho(mut self) -> Self {
        self.tamanho = Tamanho::Cabecalho;
        self
    }

    /// O botão da barra de ícones (`.rail`): círculo de 36px com o ícone de 18px em `--t3`; no hover, o volume
    /// neutro com o halo de 16px; ligado ([`Botao::selecionado`]), o volume com a sombra interna, sem halo, e a
    /// barrinha de 3px na cor de destaque 7px à esquerda do botão (com o brilho de 8px).
    pub fn rail(mut self) -> Self {
        self.redondo = true;
        self.tamanho = Tamanho::Rail;
        self
    }

    /// `.ib.pri`: o botão de ícone no degradê da cor de destaque (o Executar da Consulta).
    pub fn principal(self) -> Self {
        self.tipo(TipoBotao::Primario)
    }

    /// `.btn.pri`: a ação principal, no degradê da cor de destaque com texto branco.
    pub fn primario(texto: &'a str) -> Self {
        Botao::new(texto).tipo(TipoBotao::Primario)
    }

    /// O antigo `.btn.gold` (gerar arquivo): no Estúdio é o mesmo [`Botao::primario`].
    pub fn ouro(texto: &'a str) -> Self {
        Botao::new(texto).tipo(TipoBotao::Ouro)
    }

    /// `.btn.sutil`: transparente, texto `--t2`; ganha o volume neutro no hover.
    pub fn ghost(texto: &'a str) -> Self {
        Botao::new(texto).tipo(TipoBotao::Ghost)
    }

    /// `.btn.risco`: a ação destrutiva (Excluir, Interromper), em vermelho.
    pub fn perigo(texto: &'a str) -> Self {
        Botao::new(texto).tipo(TipoBotao::Perigo)
    }

    /// `.btn.sutil.perigo`: destrutiva em linha de tabela ou no rodapé da janela (texto `--perigo`).
    pub fn ghost_perigo(texto: &'a str) -> Self {
        Botao::new(texto).tipo(TipoBotao::GhostPerigo)
    }

    /// `.ib`: círculo de 30px só com o ícone, transparente até o hover. "‹", "›", "✕"/"×" e "+" viram os
    /// ícones das pranchetas (as setas de voltar e avançar, o × e o + de traço); outro texto sai em 13px / 600.
    pub fn redondo(texto: &'a str) -> Self {
        let mut b = Botao::new(texto);
        b.redondo = true;
        b
    }

    /// O × de fechar janela (o `.ib` do cabeçalho do `.dlg`).
    pub fn fechar() -> Self {
        Botao::redondo("✕").dica("Fechar (Esc)").rotulo("Fechar")
    }

    pub fn tipo(mut self, tipo: TipoBotao) -> Self {
        self.tipo = tipo;
        self
    }

    /// Padding lateral de 8px em vez de 15px (o "✕" e os botões de remover filtro).
    pub fn estreito(mut self, sim: bool) -> Self {
        self.estreito = sim;
        self
    }

    /// Ocupado: ponto pulsante antes do texto e sem clique (o `.carregando` do botão de exportar). No botão de
    /// ícone (o Executar da Consulta executando, N3), o arco girando no lugar do ícone, a 80% de opacidade
    /// (`.ib.ocupado`); o leitor de tela ouve o botão desligado.
    pub fn ocupado(mut self, sim: bool) -> Self {
        self.ocupado = sim;
        self
    }

    /// No ocupado de um botão de texto, o arco girando de 14px (o `.ic` com `.gira` do "Gerando arquivo" da janela
    /// Exportar) no lugar do ponto pulsante.
    pub fn com_arco(mut self, sim: bool) -> Self {
        self.arco = sim;
        self
    }

    /// `disabled`: 45% de opacidade (30% no redondo, o `.ib:disabled`) e sem clique.
    pub fn habilitado(mut self, sim: bool) -> Self {
        self.habilitado = sim;
        self
    }

    /// Estado "ligado" de um botão de alternar (`aria-pressed`): o sutil e o redondo ficam com o volume neutro
    /// sem o halo (o `.ib.on`); os outros, como no hover. O leitor de tela ouve o estado (ligado ou não).
    pub fn selecionado(mut self, sim: bool) -> Self {
        self.selecionado = sim;
        self.alternar = true;
        self
    }

    /// Dica ao passar o mouse (o `title`).
    pub fn dica(mut self, texto: &'a str) -> Self {
        self.dica = Some(texto);
        self
    }

    /// Nome para o leitor de tela quando o texto é um ícone (o `aria-label`).
    pub fn rotulo(mut self, texto: &'a str) -> Self {
        self.rotulo_acessivel = Some(texto);
        self
    }

    /// Largura mínima.
    pub fn largura_min(mut self, largura: f32) -> Self {
        self.largura_min = largura;
        self
    }
}

// ------------------------------------------------------------------ altura

fn id_altura() -> Id {
    Id::new("databox::botao::altura")
}

/// Desenha `conteudo` com os botões de texto numa outra altura: o rodapé das janelas usa [`ALTURA_JANELA`]
/// (`.dlg footer .btn`). O redondo continua com 30px.
pub fn com_altura<R>(ui: &mut Ui, altura: f32, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let antes: Option<f32> = ctx.data(|d| d.get_temp(id_altura()));
    ctx.data_mut(|d| d.insert_temp(id_altura(), altura));
    let r = conteudo(ui);
    ctx.data_mut(|d| match antes {
        Some(a) => {
            d.insert_temp(id_altura(), a);
        }
        None => {
            d.remove_temp::<f32>(id_altura());
        }
    });
    r
}

/// A altura dos botões de texto neste ponto do quadro ([`ALTURA`], ou a de [`com_altura`]).
pub fn altura_atual(ui: &Ui) -> f32 {
    ui.ctx().data(|d| d.get_temp(id_altura())).unwrap_or(ALTURA)
}

// ------------------------------------------------------------------ volume

const SEM_SOMBRA: SombraCss = SombraCss { y: 0.0, desfoque: 0.0, espalhar: 0.0, cor: Color32::TRANSPARENT };

/// O acabamento com volume do `.btn` (`background:var(--textura),var(--vol…)` com as sombras dele). Os campos
/// seguem a ordem do `box-shadow` do CSS; as caixas de marcar, o `.seg button.on`, o `.menu` e o `.rail` montam
/// o deles a partir de [`Volume::neutro`] / [`Volume::primario`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    /// O degradê (`--volneu` = [`Preenchimento::Aqua`], `--volpri`, `--volrisco`); as riscas vão por cima.
    pub fundo: Preenchimento,
    /// `border:1px solid` (por dentro da peça).
    pub borda: Color32,
    /// `inset 0 1px 0` (o realce de 1px em cima, que acompanha a curva).
    pub realce: Color32,
    /// `inset 0 -2px 3px` (transparente = sem).
    pub sombra_baixo: Color32,
    /// As sombras projetadas (cor transparente = sem).
    pub sombras: [SombraCss; 2],
    /// O halo do hover (`0 0 <halo_desfoque>px halo_cor`), com o anel `0 0 0 1px halo_cor` quando `anel`.
    pub halo_desfoque: f32,
    pub halo_cor: Color32,
    pub anel: bool,
    /// No hover, as sombras projetadas dão lugar ao halo (`.btn.pri:hover`, `.btn.risco:hover`).
    pub sombras_somem_no_hover: bool,
    /// O `filter:brightness()` do hover (1 = sem).
    pub clarear_hover: f32,
}

impl Volume {
    /// `.btn`: `--volneu`, borda `--btb`, realce `--btluz`, `inset 0 -2px 3px rgba(0,0,0,.18)` e `--btsomb`; no
    /// hover, `brightness(1.07)` com o anel de 1px e o halo de 18px em `--brilho`.
    pub fn neutro(p: &Paleta) -> Self {
        let e = &p.estudio;
        Volume {
            fundo: Preenchimento::Aqua,
            borda: e.btb,
            realce: e.btluz,
            sombra_baixo: rgba(0, 0, 0, 0.18),
            sombras: e.btsomb,
            halo_desfoque: 18.0,
            halo_cor: e.brilho,
            anel: true,
            sombras_somem_no_hover: false,
            clarear_hover: 1.07,
        }
    }

    /// `.btn.pri`: `--volpri`, borda do destaque a 65% com preto, realce `rgba(255,255,255,.38)`, sombra interna
    /// a .22 e as sombras `0 1px 1px rgba(0,0,0,.4), 0 4px 12px` do destaque a 35%; no hover, o anel e o halo de
    /// 24px em `--brilhof` no lugar delas.
    pub fn primario(p: &Paleta) -> Self {
        let e = &p.estudio;
        Volume {
            fundo: Preenchimento::Primario,
            borda: e.pri_borda,
            realce: rgba(255, 255, 255, 0.38),
            sombra_baixo: rgba(0, 0, 0, 0.22),
            sombras: [
                SombraCss { y: 1.0, desfoque: 1.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.4) },
                p.anel_aqua_forte.sombra,
            ],
            halo_desfoque: 24.0,
            halo_cor: e.brilho_forte,
            anel: true,
            sombras_somem_no_hover: true,
            clarear_hover: 1.07,
        }
    }

    /// `.btn.risco`: `--volrisco`, borda `#7f2219`, realce `rgba(255,255,255,.32)`, sombra `0 1px 1px
    /// rgba(0,0,0,.4)`; no hover, o anel e o halo de 22px vermelhos (`rgba(232,96,80,.55)`).
    pub fn risco(p: &Paleta) -> Self {
        let e = &p.estudio;
        Volume {
            fundo: Preenchimento::Risco,
            borda: e.risco_borda,
            realce: rgba(255, 255, 255, 0.32),
            sombra_baixo: rgba(0, 0, 0, 0.22),
            sombras: [SombraCss { y: 1.0, desfoque: 1.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.4) }, SEM_SOMBRA],
            halo_desfoque: 22.0,
            halo_cor: rgba(232, 96, 80, 0.55),
            anel: true,
            sombras_somem_no_hover: true,
            clarear_hover: 1.07,
        }
    }

    /// Sem a sombra interna embaixo (o `.btn.sutil:hover`, o `.ib:hover`).
    pub fn sem_sombra_baixo(mut self) -> Self {
        self.sombra_baixo = Color32::TRANSPARENT;
        self
    }

    /// Outro halo de hover (`0 0 <desfoque>px cor`, com ou sem o anel de 1px).
    pub fn halo(mut self, desfoque: f32, cor: Color32, anel: bool) -> Self {
        self.halo_desfoque = desfoque;
        self.halo_cor = cor;
        self.anel = anel;
        self
    }
}

/// Como a peça está a cada quadro (os três de 0 a 1, das transições de 200ms).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EstadoVolume {
    /// Quanto do volume aparece: o sutil e o redondo só o mostram no hover ou ligados; os outros, sempre (1).
    pub presenca: f32,
    /// Hover (ou foco do teclado): o brilho, o halo e o anel.
    pub hover: f32,
    /// Pressionado: afunda.
    pub pressao: f32,
}

/// O pintor das sombras e halos de uma peça: com o recorte da janela quando a peça está inteira à vista (o
/// halo passa da peça, como no navegador), senão o do `Ui` (cortada pela rolagem, não brilha por cima do resto).
pub fn pintor_de_fora(ui: &Ui, rect: Rect) -> Painter {
    let mut pintor = ui.painter().clone();
    if ui.clip_rect().contains_rect(rect) {
        pintor.set_clip_rect(ui.ctx().content_rect());
    }
    pintor
}

/// Pinta o volume no retângulo (raio `f32::INFINITY` = pílula ou círculo), na ordem do CSS: sombras projetadas
/// e halo (com `fora`, ver [`pintor_de_fora`]), degradê com o brilho do hover, riscas, sombra interna embaixo
/// (ou em cima, afundado), realce curvo e borda. O texto fica por conta de quem chama.
pub fn pintar_volume(
    painter: &Painter,
    fora: &Painter,
    p: &Paleta,
    rect: Rect,
    raio: f32,
    v: &Volume,
    e: EstadoVolume,
) {
    let presenca = e.presenca.clamp(0.0, 1.0);
    if presenca <= 0.0 || !rect.is_positive() {
        return;
    }
    let hover = e.hover.clamp(0.0, 1.0);
    let pressao = e.pressao.clamp(0.0, 1.0);
    let mut pv = painter.clone();
    pv.multiply_opacity(presenca);
    let mut pf = fora.clone();
    pf.multiply_opacity(presenca);

    // sombras projetadas: somem ao afundar (e no hover dos que as trocam pelo halo)
    let mut forca = 1.0 - pressao;
    if v.sombras_somem_no_hover {
        forca *= 1.0 - hover;
    }
    for s in v.sombras {
        if s.cor.a() > 0 && forca > 0.0 {
            pintura::sombra(&pf, rect, raio, SombraCss { cor: s.cor.gamma_multiply(forca), ..s });
        }
    }
    // halo e anel do hover
    let forca_halo = hover * (1.0 - pressao);
    if forca_halo > 0.0 && v.halo_cor.a() > 0 {
        let cor = v.halo_cor.gamma_multiply(forca_halo);
        if v.halo_desfoque > 0.0 {
            pintura::halo(&pf, rect, raio, v.halo_desfoque, cor);
        }
        if v.anel {
            pf.add(RectShape::stroke(rect, pintura::canto(rect, raio), Stroke::new(1.0, cor), StrokeKind::Outside));
        }
    }

    // o degradê, com o `brightness()` do hover (1.07) e do afundado (.95)
    let brilho = (1.0 + (v.clarear_hover - 1.0) * hover) * (1.0 - 0.05 * pressao);
    let mut forma = pintura::forma_preenchida(painter.ctx(), p, rect, raio, v.fundo);
    if brilho < 1.0
        && let Shape::Rect(r) = &mut forma
    {
        r.fill = pintura::clarear(r.fill, brilho);
    }
    pv.add(forma);
    pintura::riscas(&pv, p, rect, raio);
    if brilho > 1.0 {
        // clarear um degradê: soma (luz aditiva, alfa zero) a fração que falta da cor média
        let m = pintura::cor_media(p, v.fundo);
        let k = brilho - 1.0;
        let canal = |c: u8| (f32::from(c) * k).round().clamp(0.0, 255.0) as u8;
        let luz = Color32::from_rgba_premultiplied(canal(m.r()), canal(m.g()), canal(m.b()), 0);
        pv.add(RectShape::filled(rect, pintura::canto(rect, raio), luz));
    }
    let dentro = rect.shrink(1.0);
    let raio_dentro = raio - 1.0;
    pintura::sombra_interna_baixo(&pv, dentro, raio_dentro, 4.0, v.sombra_baixo.gamma_multiply(1.0 - pressao));
    // `.btn:active{box-shadow:inset 0 2px 5px rgba(0,0,0,.4)}`
    pintura::sombra_interna_topo(&pv, dentro, raio_dentro, 5.0, rgba(0, 0, 0, 0.4).gamma_multiply(pressao));
    realce_curvo(&pv, dentro, raio_dentro, v.realce.gamma_multiply(1.0 - pressao));
    pv.add(RectShape::stroke(rect, pintura::canto(rect, raio), Stroke::new(1.0, v.borda), StrokeKind::Inside));
}

/// O realce de 1px em cima (`inset 0 1px 0 cor`) de uma peça arredondada, por dentro de `rect` (a caixa sem a
/// borda): a meia-lua que acompanha a curva, cheia no alto e sumindo nas laterais (onde o `inset` desce 1px
/// e não aparece). [`pintura::brilho_topo`] só faz o trecho reto, que num círculo não existe.
pub fn realce_curvo(painter: &Painter, rect: Rect, r: f32, cor: Color32) {
    painter.add(forma_realce_curvo(rect, r, cor));
}

/// O mesmo que [`realce_curvo`], devolvendo a forma (para pintar depois num lugar reservado).
pub fn forma_realce_curvo(rect: Rect, r: f32, cor: Color32) -> Shape {
    if cor.a() == 0 || rect.width() < 2.0 || rect.height() < 2.0 {
        return Shape::Noop;
    }
    let linha = rect.shrink(0.5);
    let rr = pintura::raio(linha, r - 0.5);
    let esq = pos2(linha.left() + rr, linha.top() + rr);
    let dir = pos2(linha.right() - rr, linha.top() + rr);
    let mut pontos = Vec::with_capacity(36);
    if rr >= 1.0 {
        let n = ((rr * FRAC_PI_2) / 1.5).ceil().clamp(3.0, 16.0) as usize;
        let arco = |pontos: &mut Vec<Pos2>, centro: Pos2, a0: f32, desde: usize| {
            for i in desde..=n {
                let a = a0 + FRAC_PI_2 * i as f32 / n as f32;
                pontos.push(centro + vec2(a.cos(), a.sin()) * rr);
            }
        };
        arco(&mut pontos, esq, PI, 0);
        // num círculo os dois centros coincidem: não repetir o ponto do alto
        arco(&mut pontos, dir, PI + FRAC_PI_2, usize::from((dir.x - esq.x) < 0.01));
    } else {
        pontos.extend([linha.left_top(), linha.right_top()]);
    }
    let traco = PathStroke::new_uv(1.0, move |_, pos| {
        let centro = if pos.x < esq.x {
            esq
        } else if pos.x > dir.x {
            dir
        } else {
            return cor;
        };
        let d = pos - centro;
        let comprimento = d.length();
        let f = if comprimento > 0.01 { (-d.y / comprimento).clamp(0.0, 1.0) } else { 1.0 };
        cor.gamma_multiply(f)
    });
    PathShape::line(pontos, traco).into()
}

// ------------------------------------------------------------------ ícones de traço

/// Os textos que o botão desenha como ícone das pranchetas: "✕"/"×" (o × de 14px), "‹" e "›" (as setas de voltar
/// e avançar) e "+".
fn glifo(texto: &str) -> Option<Icone> {
    match texto {
        "✕" | "×" => Some(Icone::Fechar),
        "‹" => Some(Icone::Voltar),
        "›" => Some(Icone::Avancar),
        "+" => Some(Icone::Mais),
        _ => None,
    }
}

/// Lado da caixa do ícone no botão: 16px no `.ib` (14px no ×), 14px no `.ib.pq`, 12px no `.cab .ib`; no `.rail`,
/// 22px nos sólidos (o trilho B) e 18px nas silhuetas.
fn lado_icone(icone: Icone, tamanho: Tamanho) -> f32 {
    match (tamanho, icone) {
        (Tamanho::Normal, Icone::Fechar) => 14.0,
        (Tamanho::Normal, _) => 16.0,
        (Tamanho::Pequeno, _) => 14.0,
        (Tamanho::Cabecalho, _) => 12.0,
        (Tamanho::Rail, i) if i.familia() == Familia::Solido => 22.0,
        (Tamanho::Rail, _) => 18.0,
    }
}

/// O giro do arco de carregamento agora (uma volta por segundo), pedindo o quadro seguinte; parado com o movimento
/// reduzido.
fn giro_do_arco(ui: &Ui) -> f32 {
    if movimento_reduzido(ui) {
        return 0.0;
    }
    ui.ctx().request_repaint_after(Duration::from_millis(33));
    (ui.input(|i| i.time) % 1.0) as f32
}

/// O que o ocupado de um botão de texto toma antes do texto: o ponto de 6px ou o arco de 14px, com `gap:6px`.
fn largura_do_ocupado(ocupado: bool, arco: bool) -> f32 {
    match (ocupado, arco) {
        (false, _) => 0.0,
        (true, false) => 6.0 + GAP,
        (true, true) => LADO_ICONE_TEXTO + GAP,
    }
}

/// O × de traço (o `.ic` `M6 6l12 12M18 6 6 18` na grade de 24) centrado em `centro`, numa caixa de `lado`.
pub(crate) fn pintar_x(painter: &Painter, centro: Pos2, lado: f32, cor: Color32) {
    icones::pintar(painter, Rect::from_center_size(centro, vec2(lado, lado)), Icone::Fechar, cor);
}

// ------------------------------------------------------------------ pintura do botão

/// O visual de um botão: o volume e as cores do texto.
struct Aparencia {
    volume: Volume,
    texto: Color32,
    texto_hover: Color32,
    /// Transparente fora do hover (`.btn.sutil`, `.ib`).
    sutil: bool,
}

fn aparencia(p: &Paleta, tipo: TipoBotao, redondo: bool) -> Aparencia {
    let e = &p.estudio;
    let branco = Color32::WHITE;
    if redondo {
        // `.ib:hover`: o volume neutro sem a sombra interna, halo de 14px e sem o brilho
        let mut ib = Volume::neutro(p).sem_sombra_baixo().halo(14.0, e.brilho, false);
        ib.clarear_hover = 1.0;
        return match tipo {
            TipoBotao::Normal | TipoBotao::Ghost => {
                Aparencia { volume: ib, texto: e.t2, texto_hover: e.t, sutil: true }
            }
            // `.ib.pri`: sempre no degradê do destaque, `0 2px 8px` do destaque a 35%; hover com o halo de 20px
            TipoBotao::Primario | TipoBotao::Ouro => {
                let mut v = Volume::primario(p).sem_sombra_baixo().halo(20.0, e.brilho_forte, true);
                v.sombras = [SombraCss { y: 2.0, desfoque: 8.0, ..p.anel_aqua_forte.sombra }, SEM_SOMBRA];
                v.clarear_hover = 1.0;
                Aparencia { volume: v, texto: branco, texto_hover: branco, sutil: false }
            }
            TipoBotao::Perigo => {
                let mut v = Volume::risco(p).sem_sombra_baixo();
                v.clarear_hover = 1.0;
                Aparencia { volume: v, texto: branco, texto_hover: branco, sutil: false }
            }
            // `.ib.perigo:hover`: o ícone em `--perigo` e o halo vermelho
            TipoBotao::GhostPerigo => Aparencia {
                volume: ib.halo(14.0, e.perigo.gamma_multiply(0.5), false),
                texto: e.t2,
                texto_hover: e.perigo,
                sutil: true,
            },
        };
    }
    // `.btn.sutil:hover`: o volume neutro sem a sombra interna e o halo de 16px sem o anel
    let sutil = Volume::neutro(p).sem_sombra_baixo().halo(16.0, e.brilho, false);
    match tipo {
        TipoBotao::Normal => Aparencia { volume: Volume::neutro(p), texto: e.t, texto_hover: e.t, sutil: false },
        TipoBotao::Primario | TipoBotao::Ouro => {
            Aparencia { volume: Volume::primario(p), texto: branco, texto_hover: branco, sutil: false }
        }
        TipoBotao::Perigo => Aparencia { volume: Volume::risco(p), texto: branco, texto_hover: branco, sutil: false },
        TipoBotao::Ghost => Aparencia { volume: sutil, texto: e.t2, texto_hover: e.t, sutil: true },
        // `.btn.sutil.perigo`: o texto fica vermelho também no hover, e o halo é o vermelho a 50%
        TipoBotao::GhostPerigo => Aparencia {
            volume: sutil.halo(16.0, e.perigo.gamma_multiply(0.5), false),
            texto: e.perigo,
            texto_hover: e.perigo,
            sutil: true,
        },
    }
}

/// O `.rail`: transparente em `--t3`; no hover, o volume neutro sem a sombra interna e o halo de 16px; ligado, o
/// volume com a sombra interna (`inset 0 -2px 3px`) e sem halo (o `.rail.on` vem depois do `:hover` no CSS).
fn aparencia_rail(p: &Paleta, ligado: bool) -> Aparencia {
    let e = &p.estudio;
    let mut v = Volume::neutro(p).halo(16.0, e.brilho, false);
    v.clarear_hover = 1.0;
    if !ligado {
        v = v.sem_sombra_baixo();
    }
    Aparencia { volume: v, texto: e.t3, texto_hover: e.t, sutil: true }
}

/// O que o botão precisa saber para se pintar num quadro.
struct Quadro<'t> {
    texto: &'t str,
    icone: Option<Icone>,
    icone_texto: Option<Icone>,
    lista: bool,
    tipo: TipoBotao,
    redondo: bool,
    tamanho: Tamanho,
    hover: f32,
    pressao: f32,
    foco: bool,
    ocupado: bool,
    arco: bool,
    ligado: bool,
}

/// O ponto pulsante sobre o degradê do destaque ou do vermelho: branco (o `--dot` sumiria no azul).
fn pintar_ponto_claro(ui: &Ui, painter: &Painter, centro: Pos2) {
    let opacidade = if movimento_reduzido(ui) {
        1.0
    } else {
        // a mesma pulsação do `.carregando` (22% a 70% em 1,2s)
        let fase = (ui.input(|i| i.time) % 1.2) / 1.2;
        let onda = 0.5 - 0.5 * (fase * std::f64::consts::TAU).cos();
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        (0.22 + 0.48 * onda) as f32
    };
    painter.circle_filled(centro, 3.0, Color32::WHITE.gamma_multiply(opacidade));
}

fn pintar(ui: &Ui, rect: Rect, q: &Quadro<'_>) {
    let p = paleta(ui);
    let rail = q.tamanho == Tamanho::Rail;
    let ap = if rail { aparencia_rail(p, q.ligado) } else { aparencia(p, q.tipo, q.redondo) };
    let mut painter = ui.painter().clone();
    if q.redondo && !ui.is_enabled() {
        // `.ib:disabled{opacity:.3}` (o `Ui` desligado já aplicou os 45%)
        painter.multiply_opacity(0.3 / ui.visuals().disabled_alpha().max(0.3));
    } else if q.redondo && q.ocupado {
        // `.ib.ocupado{opacity:.8}`
        painter.multiply_opacity(0.8);
    }
    let mut fora = pintor_de_fora(ui, rect);
    fora.set_opacity(painter.opacity());
    let hover = if q.foco { 1.0 } else { q.hover };
    let presenca = if ap.sutil { hover.max(q.pressao).max(if q.ligado { 1.0 } else { 0.0 }) } else { 1.0 };
    // o `.rail.on` não tem o halo do hover
    let hover_volume = if rail && q.ligado { 0.0 } else { hover };
    let estado = EstadoVolume { presenca, hover: hover_volume, pressao: q.pressao };
    pintar_volume(&painter, &fora, p, rect, f32::INFINITY, &ap.volume, estado);
    if rail && q.ligado {
        // `.rail.on::before{left:-8px;top:9px;bottom:9px;width:3px;border-radius:3px;background:--acc;
        // box-shadow:0 0 8px --brilhof}` (contados por dentro da borda de 1px)
        let barra = Rect::from_min_max(
            pos2(rect.left() - 7.0, rect.top() + 10.0),
            pos2(rect.left() - 7.0 + LARGURA_BARRA_RAIL, rect.bottom() - 10.0),
        );
        pintura::halo(&fora, barra, RAIO_BARRA_RAIL, 8.0, p.estudio.brilho_forte);
        fora.rect_filled(barra, RAIO_BARRA_RAIL, p.estudio.acc);
    }

    let cor = pintura::misturar(ap.texto, ap.texto_hover, hover.max(if q.ligado { 1.0 } else { 0.0 }));
    if let Some(icone) = q.icone.or_else(|| glifo(q.texto)) {
        let lado = lado_icone(icone, q.tamanho);
        let caixa = Rect::from_center_size(rect.center(), vec2(lado, lado));
        if q.ocupado {
            // `.ib .gira`: o arco no lugar do ícone
            icones::pintar_arco(&painter, caixa, cor, giro_do_arco(ui));
        } else {
            icones::pintar(&painter, caixa, icone, cor);
        }
    } else {
        let tamanho = if q.redondo { 13.0 } else { 12.0 };
        let peso = if q.lista { Peso::Normal } else { Peso::Medio };
        let largura_ponto = largura_do_ocupado(q.ocupado, q.arco);
        let largura_icone = if q.icone_texto.is_some() { LADO_ICONE_TEXTO + GAP } else { 0.0 };
        let fonte = fontes::fonte(tamanho, peso);
        let g = if q.lista {
            let cabe = rect.width() - 2.0 * (PADDING + 1.0) - largura_ponto - largura_icone;
            galley_cortado(ui, q.texto, fonte, cor, cabe)
        } else {
            galley(ui, q.texto, fonte, cor)
        };
        let x = if q.lista {
            rect.left() + 1.0 + PADDING
        } else {
            rect.center().x - (g.size().x + largura_ponto + largura_icone) / 2.0
        };
        if q.ocupado && q.arco {
            let caixa = Rect::from_min_size(
                pos2(x, rect.center().y - LADO_ICONE_TEXTO / 2.0),
                vec2(LADO_ICONE_TEXTO, LADO_ICONE_TEXTO),
            );
            icones::pintar_arco(&painter, caixa, cor, giro_do_arco(ui));
        } else if q.ocupado {
            let centro = pos2(x + 3.0, rect.center().y);
            if ap.texto == Color32::WHITE {
                pintar_ponto_claro(ui, &painter, centro);
            } else {
                pintar_ponto_pulsante(ui, centro);
            }
        }
        if let Some(icone) = q.icone_texto {
            // `.btn.lista .ic{color:var(--t3)}`
            let cor_icone = if q.lista { p.estudio.t3 } else { cor };
            let caixa = Rect::from_min_size(
                pos2(x + largura_ponto, rect.center().y - LADO_ICONE_TEXTO / 2.0),
                vec2(LADO_ICONE_TEXTO, LADO_ICONE_TEXTO),
            );
            icones::pintar(&painter, caixa, icone, cor_icone);
        }
        painter.galley(pos2(x + largura_ponto + largura_icone, rect.center().y - g.size().y / 2.0), g, cor);
    }
    if q.foco {
        pintura::anel_foco(&fora, p, rect, f32::INFINITY);
    }
}

/// Pinta o botão no retângulo (para peças que desenham botões sem alocar). `hover` e `pressao` vão de 0 a 1;
/// `foco` é o foco do teclado (desenha o hover e o anel de foco).
#[allow(clippy::too_many_arguments)]
pub fn pintar_botao(
    ui: &Ui,
    rect: Rect,
    texto: &str,
    tipo: TipoBotao,
    redondo: bool,
    hover: f32,
    pressao: f32,
    foco: bool,
    ocupado: bool,
) {
    let q = Quadro {
        texto,
        icone: None,
        icone_texto: None,
        lista: false,
        tipo,
        redondo,
        tamanho: Tamanho::Normal,
        hover,
        pressao,
        foco,
        ocupado,
        arco: false,
        ligado: false,
    };
    pintar(ui, rect, &q);
}

impl Widget for Botao<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        if !self.habilitado && ui.is_enabled() {
            return ui.add_enabled(false, Botao { habilitado: true, ..self });
        }
        let (largura, altura) = if self.redondo {
            let lado = match self.tamanho {
                Tamanho::Normal => LADO_REDONDO,
                Tamanho::Pequeno => LADO_PEQUENO,
                Tamanho::Cabecalho => LADO_CABECALHO,
                Tamanho::Rail => LADO_RAIL,
            };
            (lado, lado)
        } else {
            // a largura sai sempre do texto, também quando ele vira ícone (o "✕" do filtro): as telas medem o
            // botão com a mesma conta
            let peso = if self.lista { Peso::Normal } else { Peso::Medio };
            let conteudo = galley(ui, self.texto, fontes::fonte(12.0, peso), Color32::PLACEHOLDER).size().x;
            let padding = match (self.tamanho, self.estreito) {
                (Tamanho::Pequeno, _) => PADDING_PEQUENO,
                (_, true) => PADDING_ESTREITO,
                _ => PADDING,
            };
            let altura = if self.tamanho == Tamanho::Pequeno { ALTURA_PEQUENO } else { altura_atual(ui) };
            let largura_ponto = largura_do_ocupado(self.ocupado, self.arco);
            let largura_icone = if self.icone_texto.is_some() { LADO_ICONE_TEXTO + GAP } else { 0.0 };
            // `padding:0 15px` + a borda de 1px dos dois lados; a lista ocupa a largura toda
            let largura = (conteudo + largura_ponto + largura_icone + 2.0 * padding + 2.0).ceil().max(self.largura_min);
            let largura = if self.lista { ui.available_width().max(self.largura_min) } else { largura };
            (largura, altura)
        };
        let sense = if self.ocupado { Sense::hover() } else { Sense::click() };
        let (rect, resp) = ui.allocate_exact_size(vec2(largura, altura), sense);
        let interativo = ui.is_enabled() && !self.ocupado;
        if !self.ocupado && crate::pecas::janela::tomar_foco_botao(ui, self.tipo) {
            resp.request_focus();
        }
        let foco = interativo && resp.has_focus() && pintura::foco_visivel(ui.ctx());
        // ligado: o sutil e o redondo ganham o volume (`.ib.on`); os outros ficam como no hover
        let ligado_como_hover =
            self.selecionado && !self.redondo && !matches!(self.tipo, TipoBotao::Ghost | TipoBotao::GhostPerigo);
        let tempo = duracao_transicao(ui);
        let hover_alvo = interativo && resp.hovered() || ligado_como_hover;
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), hover_alvo, tempo);
        let pressao = ui.ctx().animate_bool_with_time(
            resp.id.with("pressao"),
            interativo && resp.is_pointer_button_down_on(),
            tempo,
        );
        if ui.is_rect_visible(rect.expand(ALCANCE_HALO)) {
            let q = Quadro {
                texto: self.texto,
                icone: self.icone,
                icone_texto: self.icone_texto,
                lista: self.lista,
                tipo: self.tipo,
                redondo: self.redondo,
                tamanho: self.tamanho,
                hover,
                pressao,
                foco,
                ocupado: self.ocupado,
                arco: self.arco,
                ligado: self.selecionado && !ligado_como_hover,
            };
            pintar(ui, rect, &q);
        }
        if interativo && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        } else if self.ocupado && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Progress);
        }
        let rotulo = self.rotulo_acessivel.unwrap_or(self.texto);
        let habilitado = ui.is_enabled() && !self.ocupado;
        if self.alternar {
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, habilitado, self.selecionado, rotulo));
        } else {
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, habilitado, rotulo));
        }
        match self.dica {
            Some(d) => resp.on_hover_text(d),
            None => resp,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn glifos_das_pranchetas() {
        assert_eq!(glifo("✕"), Some(Icone::Fechar));
        assert_eq!(glifo("×"), Some(Icone::Fechar));
        assert_eq!(glifo("‹"), Some(Icone::Voltar));
        assert_eq!(glifo("›"), Some(Icone::Avancar));
        assert_eq!(glifo("+"), Some(Icone::Mais));
        assert_eq!(glifo("+ Nova tabela"), None);
        // `.ib .sil` 16px (o × 14px), `.ib.pq .sil` 14px, `.cab .ib .sil` 12px, `.rail .sil` 18px e o sólido do
        // trilho B de 22px
        assert_eq!(lado_icone(Icone::Fechar, Tamanho::Normal), 14.0);
        assert_eq!(lado_icone(Icone::Salvar, Tamanho::Normal), 16.0);
        assert_eq!(lado_icone(Icone::Mais, Tamanho::Pequeno), 14.0);
        assert_eq!(lado_icone(Icone::Recolher, Tamanho::Cabecalho), 12.0);
        assert_eq!(lado_icone(Icone::Inicio, Tamanho::Rail), 22.0);
        assert_eq!(lado_icone(Icone::Casa, Tamanho::Rail), 18.0);
    }

    #[test]
    fn volumes_seguem_o_css() {
        use crate::tema::{CLARA, ESCURA};
        for p in [&CLARA, &ESCURA] {
            let n = Volume::neutro(p);
            assert_eq!((n.borda, n.realce, n.sombras), (p.estudio.btb, p.estudio.btluz, p.estudio.btsomb));
            assert_eq!((n.halo_desfoque, n.anel, n.clarear_hover), (18.0, true, 1.07));
            let pri = Volume::primario(p);
            assert_eq!(
                (pri.fundo, pri.borda, pri.halo_cor),
                (Preenchimento::Primario, p.estudio.pri_borda, p.estudio.brilho_forte)
            );
            assert!(pri.sombras_somem_no_hover);
            assert_eq!(Volume::risco(p).borda, p.estudio.risco_borda);
            assert_eq!(n.sem_sombra_baixo().sombra_baixo, Color32::TRANSPARENT);
        }
    }

    #[test]
    fn ouro_e_o_mesmo_que_o_primario() {
        use crate::tema::ESCURA;
        let (a, b) = (aparencia(&ESCURA, TipoBotao::Ouro, false), aparencia(&ESCURA, TipoBotao::Primario, false));
        assert_eq!((a.volume, a.texto), (b.volume, b.texto));
    }

    #[test]
    fn rail_segue_o_css() {
        use crate::tema::{CLARA, ESCURA};
        for p in [&CLARA, &ESCURA] {
            // `.rail:hover`: `inset 0 1px 0 --btluz, --btsomb, 0 0 16px --brilho`, texto `--t3` → `--t`
            let hover = aparencia_rail(p, false);
            assert_eq!((hover.volume.halo_desfoque, hover.volume.anel), (16.0, false));
            assert_eq!(hover.volume.sombra_baixo, Color32::TRANSPARENT);
            assert_eq!((hover.texto, hover.texto_hover, hover.sutil), (p.estudio.t3, p.estudio.t, true));
            // `.rail.on`: com o `inset 0 -2px 3px rgba(0,0,0,.18)`
            assert_eq!(aparencia_rail(p, true).volume.sombra_baixo, rgba(0, 0, 0, 0.18));
        }
    }
}
