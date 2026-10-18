//! Avisos e estados do visual Estúdio: [`aviso`] (o `.aviso` âmbar das pranchetas; com `perigo`, o `.erro`
//! vermelho) e [`aviso_tom`] com os outros tons de [`TomAviso`] (concluído e informação), [`vazio`] (o que o vazio
//! significa, em `--t3`) e [`carregando`] (o ponto que pulsa na cor de destaque e o texto).
//!
//! O aviso é uma caixa com o sólido de estado à esquerda (24px: atenção, erro, concluído ou informação),
//! `gap:12px` e o texto centrado na altura dele (pranchetas 6, 12b e 15), fundo da cor do estado a 12% (`--aviso`; no erro, `--perigo` a 10%; no concluído, `--ok`; na
//! informação, o `--tint`) e fio de 1px a 35% (30% no erro), com o raio único de 10 ([`RAIO_AVISO`]). O texto é
//! 12px em `--t2`, com o `strong` em `--t` (no erro, em `--perigo`) e os links em `--acct` 600.
//!
//! Dentro de janelas, fichas e documentos (padrão de 16/10/2026), a caixa tem a largura do texto, até 560px
//! ([`LARGURA_MAX_AVISO`]), e fica centrada; num espaço mais estreito, ocupa a largura toda e o texto quebra
//! ([`caixa_do_aviso`]). Só a faixa do topo do documento vai de ponta a ponta ([`aviso_faixa`],
//! [`Aviso::ponta_a_ponta`]).
//!
//! O âmbar é só para o alerta de verdade (transação aberta, banco já aberto, linha sem chave, a consequência de
//! uma mudança): sucesso e resultado vão no concluído; prévia e informação neutra, na informação.

use std::time::Duration;

use egui::{Color32, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::ajuda::{IconeEstado, pintar_icone_estado};
use crate::pecas::botao::Botao;
use crate::pecas::texto::{
    CoresTexto, RespostaTexto, Trecho, galley, interagir_paragrafo, meia_entrelinha, paragrafo_rico_com, realces_links,
    sentido_selecionavel,
};
use crate::pintura;
use crate::tema::{movimento_reduzido, paleta};

/// Raio do aviso dentro de cartões e janelas: o raio único das superfícies, [pintura::RAIO_PADRAO] (decisão de
/// 16/10/2026, como nas pranchetas).
pub const RAIO_AVISO: f32 = pintura::RAIO_PADRAO;
/// O sólido de estado (`.sk.s24`) e o `gap:12px` até o texto (pranchetas 6, 12b e 15, revisão de 17/10/2026).
const LADO_ICONE: f32 = 24.0;
const ESPACO_ICONE: f32 = 12.0;
/// A caixa (`.aviso-j`: `padding:9px 12px`), mais o fio de 1px.
const RECUO_X: f32 = 13.0;
const RECUO_Y: f32 = 10.0;
/// A faixa do topo do documento (`.aviso` e `.erro`: `padding:8px 14px`), sem o fio dos lados nem o de cima; embaixo
/// o fio de 1px.
const RECUO_X_FAIXA: f32 = 14.0;
const RECUO_TOPO_FAIXA: f32 = 8.0;
const RECUO_BASE_FAIXA: f32 = 9.0;
/// O texto do aviso: 12px numa linha de 17px (`line-height:1.4`).
const TAMANHO_TEXTO: f32 = 12.0;
const FATOR_LINHA: f32 = 1.4;

/// Ponto do `.carregando` (6px, na cor de destaque `--acc`), pulsando entre 22% e 70% em 1,2s (parado e cheio com
/// movimento reduzido). Pede repintura enquanto aparece (~30 quadros por segundo, só enquanto há algo carregando).
pub fn pintar_ponto_pulsante(ui: &Ui, centro: Pos2) {
    let p = paleta(ui);
    let opacidade = if movimento_reduzido(ui) {
        1.0
    } else {
        let t = ui.input(|i| i.time);
        let fase = (t % 1.2) / 1.2;
        let onda = 0.5 - 0.5 * (fase * std::f64::consts::TAU).cos();
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        (0.22 + 0.48 * onda) as f32
    };
    ui.painter().circle_filled(centro, 3.0, p.estudio.acc.gamma_multiply(opacidade));
}

/// O carregando (`span.carregando.hint`): o ponto pulsante e o texto ("Carregando", "contando") em 11.5px `--t3`.
pub fn carregando(ui: &mut Ui, texto: &str) -> Response {
    let p = paleta(ui);
    let cor = p.estudio.t3;
    let g = galley(ui, texto, fontes::fonte(11.5, Peso::Normal), cor);
    let tamanho = vec2(6.0 + 6.0 + g.size().x, g.size().y.max(6.0));
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
    if ui.is_rect_visible(rect) {
        pintar_ponto_pulsante(ui, pos2(rect.left() + 3.0, rect.center().y));
        ui.painter().galley(pos2(rect.left() + 12.0, rect.top()), g, cor);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, texto));
    resp
}

/// O estado vazio: texto `--t3` de 12px em linhas de 18px, centrado, com `padding:22px 20px` (ou `6px 0` dentro
/// de um corpo de cartão: `compacto`).
pub fn vazio(ui: &mut Ui, texto: &str, compacto: bool) -> Response {
    let p = paleta(ui);
    let cor = p.estudio.t3;
    let (px, py) = if compacto { (0.0, 6.0) } else { (20.0, 22.0) };
    let largura = ui.available_width();
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let desloc = meia_entrelinha(ui, &fonte, 18.0);
    let mut f = crate::pecas::texto::formato(fonte, cor);
    f.line_height = Some(18.0);
    let mut job = egui::text::LayoutJob::single_section(texto.to_owned(), f);
    job.wrap.max_width = (largura - 2.0 * px).max(40.0);
    job.halign = egui::Align::Center;
    let g = ui.fonts_mut(|fs| fs.layout_job(job));
    let altura = g.size().y + 2.0 * py;
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
    if ui.is_rect_visible(rect) {
        // com `halign` centrado, o x do galley é o eixo de cada linha
        ui.painter().galley(pos2(rect.center().x, rect.top() + py + desloc), g, cor);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, texto));
    resp
}

/// O tom de um aviso: o sólido de estado, o fundo e o fio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TomAviso {
    /// O `.aviso` âmbar com o sólido de atenção: um alerta de verdade (transação aberta, banco já aberto, linha sem
    /// chave, a consequência de uma mudança).
    #[default]
    Atencao,
    /// O `.erro` vermelho com o sólido de erro.
    Erro,
    /// Concluído: `--ok` a 12%, o fio a 35% e o sólido de concluído (arquivo salvo, banco íntegro, comando
    /// executado, transação gravada).
    Concluido,
    /// Informação: `--tint`, o fio do destaque a 35% e o sólido de informação (a prévia do que vai acontecer, um
    /// resultado neutro).
    Informacao,
    /// A nota neutra (`.nota-j` da ficha só leitura): fundo `--h`, fio `--vidrob` e o cadeado ("… está aberto só
    /// para leitura. Nada é gravado no arquivo.").
    Nota,
}

impl TomAviso {
    /// O tom do `perigo` de [`aviso`] e de [`Aviso::new`]: o erro ou a atenção.
    pub fn de_perigo(perigo: bool) -> TomAviso {
        if perigo { TomAviso::Erro } else { TomAviso::Atencao }
    }
}

/// O aviso (`.aviso`, `perigo = false`) ou o erro (`.erro`): o sólido de estado, o texto de 12px e, no fim da
/// frase, os links. O conteúdo vem em trechos (`Trecho::Forte` para o `<strong>`, `Trecho::Link` para o `a`,
/// `Trecho::Ajuda` para o "i" mini). O texto pode ser selecionado e copiado (a mensagem de erro do SQLite, o
/// comando, o caminho). A caixa tem a largura do texto, até [`LARGURA_MAX_AVISO`], e fica centrada (a faixa de ponta
/// a ponta do topo do documento é a [`aviso_faixa`]). Devolve a resposta do bloco e o link clicado.
pub fn aviso(ui: &mut Ui, trechos: &[Trecho<'_>], perigo: bool) -> RespostaTexto {
    aviso_tom(ui, trechos, TomAviso::de_perigo(perigo))
}

/// [`aviso`] em qualquer [`TomAviso`] (o concluído e a informação além da atenção e do erro).
pub fn aviso_tom(ui: &mut Ui, trechos: &[Trecho<'_>], tom: TomAviso) -> RespostaTexto {
    aviso_com(ui, trechos, Aviso::com_tom(tom))
}

/// [`aviso_tom`] na faixa de ponta a ponta do topo do documento (o aviso de transação), em vez da caixa centrada.
pub fn aviso_faixa(ui: &mut Ui, trechos: &[Trecho<'_>], tom: TomAviso) -> RespostaTexto {
    aviso_com(ui, trechos, Aviso::com_tom(tom).ponta_a_ponta(true))
}

/// [`aviso_tom`] como o `.aviso` do topo do documento ([`Aviso::faixa_do_documento`], com os `cantos` do fundo): a
/// peça `pecas::documento` usa este.
pub fn aviso_do_documento(
    ui: &mut Ui,
    trechos: &[Trecho<'_>],
    tom: TomAviso,
    cantos: egui::CornerRadius,
) -> RespostaTexto {
    aviso_com(ui, trechos, Aviso::com_tom(tom).faixa_do_documento(cantos))
}

/// O conteúdo de um aviso com a frase em trechos e, na ponta direita da linha, um link (`a` com
/// `white-space:nowrap`, 12px / 500, sublinhado, `--acct` → `--t` no hover), como o "Ir para a Consulta 1" do aviso de
/// transação (prancheta 6). A frase quebra na largura que sobra e fica centrada na altura do link. Para o
/// [`Aviso::mostrar`] e o `aviso` do documento; devolve se o link foi clicado.
pub fn texto_com_link_a_direita(ui: &mut Ui, trechos: &[Trecho<'_>], link: &str, cores: CoresAviso) -> bool {
    let p = paleta(ui);
    let c = CoresTexto::new(p, cores.texto, cores.forte);
    let id = ui.id().with("databox::aviso::texto_com_link");
    let fonte = fontes::fonte(TAMANHO_TEXTO, Peso::Medio);
    let realce = ui.ctx().data(|d| d.get_temp::<bool>(id.with("realce"))).unwrap_or(false);
    let g_link = galley(ui, link, fonte, if realce { c.link_realce } else { c.link });
    let largura = ui.available_width();
    let largura_texto = (largura - g_link.size().x - ESPACO_ICONE).max(40.0);
    let realces = realces_links(ui, id);
    let g = paragrafo_rico_com(ui, trechos, TAMANHO_TEXTO, &c, largura_texto, FATOR_LINHA, &realces);
    let altura = g.tamanho().y.max(g_link.size().y);
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, altura), sentido_selecionavel());
    let topo_texto = rect.top() + (altura - g.tamanho().y) / 2.0;
    interagir_paragrafo(ui, id, &resp, &g, pos2(rect.left(), topo_texto), trechos, TAMANHO_TEXTO, true);
    let r_link = Rect::from_min_size(
        pos2(rect.right() - g_link.size().x, rect.top() + (altura - g_link.size().y) / 2.0),
        g_link.size(),
    );
    let r = ui.interact(r_link, id.with("link"), Sense::click());
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if r.hovered() != realce {
        ui.ctx().data_mut(|d| d.insert_temp(id.with("realce"), r.hovered()));
        ui.ctx().request_repaint();
    }
    let cor = if realce { c.link_realce } else { c.link };
    // o sublinhado, 2px abaixo da linha de base
    let y = r_link.bottom() - 3.0;
    ui.painter().hline(r_link.x_range(), y, Stroke::new(1.0, cor));
    ui.painter().galley(r_link.min, g_link, cor);
    if r.has_focus() && pintura::foco_visivel(ui.ctx()) {
        pintura::anel_foco(ui.painter(), p, r_link, 6.0);
    }
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Link, ui.is_enabled(), link));
    r.clicked()
}

fn aviso_com(ui: &mut Ui, trechos: &[Trecho<'_>], mut peca: Aviso) -> RespostaTexto {
    // a largura do texto numa linha só (o `max-content` do CSS): quando ele quebra, a caixa fica com a largura
    // máxima, como o `width:fit-content` faz no navegador; as cores não mudam a medida
    let p = paleta(ui);
    let c = CoresTexto::new(p, p.estudio.t2, p.estudio.t);
    let natural = paragrafo_rico_com(ui, trechos, TAMANHO_TEXTO, &c, f32::INFINITY, FATOR_LINHA, &[]).tamanho().x;
    peca.natural = Some(natural);
    let mut link = None;
    let resposta = peca.mostrar(ui, |ui, cores| {
        let id = ui.id().with("databox::aviso::texto");
        let p = paleta(ui);
        let c = CoresTexto::new(p, cores.texto, cores.forte);
        let realces = realces_links(ui, id);
        let g = paragrafo_rico_com(ui, trechos, TAMANHO_TEXTO, &c, ui.available_width(), FATOR_LINHA, &realces);
        let (rect, resp) = ui.allocate_exact_size(g.tamanho(), sentido_selecionavel());
        link = interagir_paragrafo(ui, id, &resp, &g, rect.min, trechos, TAMANHO_TEXTO, true);
    });
    RespostaTexto { resposta, link }
}

/// Atalho de [`aviso`] para uma frase só.
pub fn aviso_texto(ui: &mut Ui, texto: &str, perigo: bool) -> Response {
    aviso(ui, &[Trecho::Normal(texto)], perigo).resposta
}

/// O papel de uma ação do aviso compacto ([`aviso_com_acoes`]): o botão `.btn.pri.pq`, `.btn.pq` ou `.btn.sutil.pq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PapelAcao {
    /// A ação principal ("Colar 1.180").
    Principal,
    /// A alternativa ("Acrescentar 24 linhas").
    Neutra,
    /// A saída ("Cancelar"; a dica costuma ser a tecla: Esc).
    Sutil,
}

/// Uma ação do aviso compacto: o rótulo, o papel e a dica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcaoAviso<'a> {
    pub rotulo: &'a str,
    pub papel: PapelAcao,
    pub dica: Option<&'a str>,
}

impl<'a> AcaoAviso<'a> {
    pub fn principal(rotulo: &'a str) -> Self {
        AcaoAviso { rotulo, papel: PapelAcao::Principal, dica: None }
    }

    pub fn neutra(rotulo: &'a str) -> Self {
        AcaoAviso { rotulo, papel: PapelAcao::Neutra, dica: None }
    }

    pub fn sutil(rotulo: &'a str) -> Self {
        AcaoAviso { rotulo, papel: PapelAcao::Sutil, dica: None }
    }

    /// A dica (o `title`) do botão.
    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }
}

/// O mais que o aviso compacto leva: três ações (a prancheta 16: Colar 1.180 · Acrescentar 24 linhas · Cancelar).
pub const MAXIMO_ACOES: usize = 3;

/// O que aconteceu no aviso com ações.
#[derive(Debug, Clone)]
pub struct RespostaAcoes {
    /// A caixa do aviso.
    pub resposta: Response,
    /// O índice (em `acoes`) do botão clicado.
    pub clicada: Option<usize>,
    /// O link clicado no texto, se houver.
    pub link: Option<usize>,
}

/// O aviso compacto de topo de documento com ações (`.aviso-j` com `.corpo` e `.acoes`, prancheta 16: "A coluna colada
/// tem 1.204 valores e o arquivo, 1.180 linhas." com Colar 1.180 · Acrescentar 24 linhas · Cancelar): a caixa com a
/// largura do conteúdo, centrada, o texto e embaixo dele, com 8px, a linha de botões pequenos (`.btn.pq`, `gap:6px`).
/// Leva até [`MAXIMO_ACOES`] ações (as outras são ignoradas). O texto pode ser selecionado e copiado.
pub fn aviso_com_acoes(ui: &mut Ui, trechos: &[Trecho<'_>], tom: TomAviso, acoes: &[AcaoAviso<'_>]) -> RespostaAcoes {
    debug_assert!(acoes.len() <= MAXIMO_ACOES, "o aviso compacto leva até {MAXIMO_ACOES} ações");
    let mut clicada = None;
    let mut link = None;
    let resposta = Aviso::com_tom(tom).mostrar(ui, |ui, cores| {
        let id = ui.id().with("databox::aviso::texto");
        let p = paleta(ui);
        let c = CoresTexto::new(p, cores.texto, cores.forte);
        let realces = realces_links(ui, id);
        let g = paragrafo_rico_com(ui, trechos, TAMANHO_TEXTO, &c, ui.available_width(), FATOR_LINHA, &realces);
        let (rect, resp) = ui.allocate_exact_size(g.tamanho(), sentido_selecionavel());
        link = interagir_paragrafo(ui, id, &resp, &g, rect.min, trechos, TAMANHO_TEXTO, true);
        if acoes.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (i, a) in acoes.iter().take(MAXIMO_ACOES).enumerate() {
                let botao = match a.papel {
                    PapelAcao::Principal => Botao::primario(a.rotulo),
                    PapelAcao::Neutra => Botao::new(a.rotulo),
                    PapelAcao::Sutil => Botao::ghost(a.rotulo),
                }
                .pequeno();
                let botao = if let Some(d) = a.dica { botao.dica(d) } else { botao };
                if ui.add(botao).clicked() {
                    clicada = Some(i);
                }
            }
        });
    });
    RespostaAcoes { resposta, clicada, link }
}

fn id_corpo_atual() -> Id {
    Id::new("databox::avisos::corpo_atual")
}

/// Marca `ui` como o corpo de um cartão ou de uma janela: um aviso que é o primeiro item dele ganha
/// [`ESPACO_APOS_AVISO`] embaixo. O [`crate::pecas::cartao::CorpoCartao`] e a [`crate::pecas::janela::Janela`] já
/// marcam os seus.
pub fn marcar_corpo(ui: &Ui) {
    ui.ctx().data_mut(|d| d.insert_temp(id_corpo_atual(), ui.id()));
}

/// Espaço embaixo do aviso que abre um corpo de cartão ou de janela (com o espaço de 8px entre avisos seguidos,
/// as margens se juntam em 14px).
pub const ESPACO_APOS_AVISO: f32 = 14.0;

/// `ui` é um corpo marcado e nada foi desenhado nele ainda?
fn inicio_de_corpo(ui: &Ui) -> bool {
    ui.ctx().data(|d| d.get_temp::<Id>(id_corpo_atual())) == Some(ui.id())
        && ui.cursor().min.y <= ui.max_rect().min.y + 0.01
}

/// As cores do texto dentro de um aviso (para conteúdo próprio em [`Aviso::mostrar`]).
#[derive(Debug, Clone, Copy)]
pub struct CoresAviso {
    /// Texto comum (`--t2`).
    pub texto: Color32,
    /// `strong` (`--t`; no erro, `--perigo`).
    pub forte: Color32,
}

/// A largura máxima do aviso em caixa (`max-width:min(100%,560px)`, padrão de 16/10/2026).
pub const LARGURA_MAX_AVISO: f32 = 560.0;

/// A largura e o recuo da caixa do aviso num espaço de `disponivel` pontos, com o conteúdo de `conteudo` pontos (o
/// texto já quebrado na largura que sobra): a caixa tem a largura do conteúdo com os recuos, até
/// [`LARGURA_MAX_AVISO`] e nunca mais que o espaço, e fica centrada (`width:fit-content; margin-inline:auto`). Com
/// `ponta_a_ponta` (a faixa do topo do documento), ocupa o espaço todo.
pub fn caixa_do_aviso(disponivel: f32, conteudo: f32, ponta_a_ponta: bool) -> (f32, f32) {
    if ponta_a_ponta {
        return (disponivel, 0.0);
    }
    let maximo = disponivel.min(LARGURA_MAX_AVISO);
    let largura = (conteudo + RECUO_X + LADO_ICONE + ESPACO_ICONE + RECUO_X).ceil().min(maximo);
    (largura, ((disponivel - largura) / 2.0).max(0.0).round())
}

/// Aviso com conteúdo livre (botões na linha de ações, a lista de avisos da exportação…).
pub struct Aviso {
    tom: TomAviso,
    ponta_a_ponta: bool,
    /// A faixa do topo do documento: sem caixa, com o fio só embaixo e estes cantos (os de cima da moldura, quando
    /// ela encosta no alto dela).
    faixa: Option<egui::CornerRadius>,
    /// A largura do conteúdo numa linha só, quando quem chama sabe (o texto dos [`aviso`]).
    natural: Option<f32>,
    /// A caixa sempre com a largura máxima (até [`LARGURA_MAX_AVISO`]), e não a do conteúdo: os avisos de uma janela
    /// que aparecem um de cada vez ficam com a mesma largura e a mesma coluna do sólido.
    cheia: bool,
    /// O sólido no alto da caixa (no padding), e não centrado: o aviso de várias linhas com vários parágrafos.
    icone_no_topo: bool,
}

impl Aviso {
    /// O aviso âmbar ou, com `perigo`, o erro.
    pub fn new(perigo: bool) -> Self {
        Aviso::com_tom(TomAviso::de_perigo(perigo))
    }

    /// O aviso em qualquer [`TomAviso`].
    pub fn com_tom(tom: TomAviso) -> Self {
        Aviso { tom, ponta_a_ponta: false, faixa: None, natural: None, cheia: false, icone_no_topo: false }
    }

    /// O sólido no alto da caixa, alinhado ao padding, em vez de centrado na altura: o aviso comprido de vários
    /// parágrafos ("A tabela será reconstruída…") não deixa o sólido solto no meio do texto.
    pub fn icone_no_topo(mut self, sim: bool) -> Self {
        self.icone_no_topo = sim;
        self
    }

    /// A caixa com a largura máxima do espaço (até [`LARGURA_MAX_AVISO`]) em vez da largura do conteúdo: os avisos de
    /// corpo do Exportar (erro, "Arquivo salvo") ficam com a mesma largura e o sólido na mesma coluna.
    pub fn cheia(mut self, sim: bool) -> Self {
        self.cheia = sim;
        self
    }

    /// A faixa de ponta a ponta (o aviso do topo do documento): sem isto, o aviso é uma caixa com a largura do
    /// texto, até [`LARGURA_MAX_AVISO`], centrada.
    pub fn ponta_a_ponta(mut self, sim: bool) -> Self {
        self.ponta_a_ponta = sim;
        self
    }

    /// O `.aviso` do topo do documento (prancheta 6), por dentro da moldura dele: de ponta a ponta, sem a caixa (o
    /// fio de 1px só embaixo, `padding:8px 14px`), com `cantos` no fundo (os de cima da moldura, quando a faixa
    /// encosta no alto dela; a peça `pecas::documento` cuida disso).
    pub fn faixa_do_documento(mut self, cantos: egui::CornerRadius) -> Self {
        self.ponta_a_ponta = true;
        self.faixa = Some(cantos);
        self
    }

    /// Desenha a caixa, o sólido de estado e o conteúdo; devolve a resposta do bloco (com papel `status`). A caixa
    /// tem a largura do conteúdo, até [`LARGURA_MAX_AVISO`], e fica centrada no espaço ([`caixa_do_aviso`]; num
    /// espaço mais estreito, ocupa a largura toda e o texto quebra). No conteúdo livre, a largura dele é a do quadro
    /// anterior: quando muda, o quadro é refeito na hora (`request_discard`), sem a caixa pular. O primeiro aviso de
    /// um corpo de cartão ou de janela deixa [`ESPACO_APOS_AVISO`] até o item seguinte.
    pub fn mostrar(self, ui: &mut Ui, conteudo: impl FnOnce(&mut Ui, CoresAviso)) -> Response {
        self.mostrar_com(ui, conteudo).0
    }

    /// O [`Aviso::mostrar`] devolvendo também o que o conteúdo devolveu.
    pub(crate) fn mostrar_com<R>(self, ui: &mut Ui, conteudo: impl FnOnce(&mut Ui, CoresAviso) -> R) -> (Response, R) {
        let p = paleta(ui);
        let e = &p.estudio;
        let primeiro = inicio_de_corpo(ui);
        // `.aviso`: `--aviso` a 12% e o fio a 35%; `.erro`: `--perigo` a 10% e o fio a 30%; o concluído e a
        // informação seguem o `.aviso` com `--ok` e com o destaque (o fundo é o `--tint`)
        let normal = CoresAviso { texto: e.t2, forte: e.t };
        let (cores, fundo, fio, icone) = match self.tom {
            TomAviso::Erro => (
                CoresAviso { texto: e.t2, forte: e.perigo },
                p.aqua_danger_bg,
                e.perigo.gamma_multiply(0.30),
                IconeEstado::Erro,
            ),
            TomAviso::Atencao => (normal, p.aqua_wait_bg, e.aviso.gamma_multiply(0.35), IconeEstado::Atencao),
            TomAviso::Concluido => {
                (normal, e.ok.gamma_multiply(0.12), e.ok.gamma_multiply(0.35), IconeEstado::Concluido)
            }
            TomAviso::Informacao => (normal, e.tint, e.acc.gamma_multiply(0.35), IconeEstado::Informacao),
            TomAviso::Nota => (normal, e.h, e.vidrob, IconeEstado::Bloqueado),
        };
        let lugar_fundo = ui.painter().add(egui::Shape::Noop);
        let disponivel = ui.available_width();
        let inicio = ui.cursor().min;
        // a faixa do documento não tem as bordas dos lados nem a de cima: 14px dos lados e 8px em cima; embaixo, os
        // 8px e o fio
        let (recuo_x, recuo_topo, recuo_base) = if self.faixa.is_some() {
            (RECUO_X_FAIXA, RECUO_TOPO_FAIXA, RECUO_BASE_FAIXA)
        } else {
            (RECUO_X, RECUO_Y, RECUO_Y)
        };
        let x_texto = recuo_x + LADO_ICONE + ESPACO_ICONE;
        // o texto quebra na largura que a caixa maior deixa; a caixa fica com a largura do texto numa linha só (a
        // dos `aviso`) ou, no conteúdo livre, com a que ele ocupou no quadro anterior (no primeiro, a maior)
        let maximo_texto = (disponivel.min(LARGURA_MAX_AVISO) - x_texto - recuo_x).max(10.0);
        let id_largura = ui.next_auto_id().with("databox::aviso::largura");
        let medida: Option<f32> = match self.natural {
            Some(n) => Some(n.min(maximo_texto)),
            None => ui.ctx().data(|d| d.get_temp(id_largura)),
        };
        let conteudo_da_caixa = if self.ponta_a_ponta {
            0.0
        } else if self.cheia {
            maximo_texto
        } else {
            medida.unwrap_or(maximo_texto)
        };
        let (largura, recuo) = caixa_do_aviso(disponivel, conteudo_da_caixa, self.ponta_a_ponta);
        let largura_texto = if self.ponta_a_ponta { (disponivel - x_texto - recuo_x).max(10.0) } else { maximo_texto };
        // o texto fica centrado na altura do sólido (`align-items:center`): a altura dele é a do quadro anterior (no
        // primeiro, uma linha de 17px); quando muda, o quadro seguinte já sai certo (sem `request_discard`: as passadas
        // de um quadro são poucas e as telas já gastam as delas nas medidas das tabelas e das larguras)
        let id_altura = id_largura.with("altura");
        let altura_conhecida: Option<f32> = ui.ctx().data(|d| d.get_temp(id_altura));
        let desce = ((LADO_ICONE - altura_conhecida.unwrap_or(17.0)) / 2.0).max(0.0);
        let interno =
            Rect::from_min_size(inicio + vec2(recuo + x_texto, recuo_topo + desce), vec2(largura_texto, f32::INFINITY));
        let mut filho =
            ui.new_child(egui::UiBuilder::new().max_rect(interno).layout(egui::Layout::top_down(egui::Align::Min)));
        filho.spacing_mut().item_spacing = vec2(8.0, 8.0);
        let saida = conteudo(&mut filho, cores);
        let altura_texto = filho.min_rect().height();
        if altura_conhecida.is_none_or(|a| (a - altura_texto).abs() > 0.5) {
            ui.ctx().data_mut(|d| d.insert_temp(id_altura, altura_texto));
            ui.ctx().request_repaint();
        }
        if !self.ponta_a_ponta && !self.cheia && self.natural.is_none() {
            let ocupada = filho.min_rect().width().min(maximo_texto);
            if medida.is_none_or(|m| (m - ocupada).abs() > 0.5) {
                ui.ctx().data_mut(|d| d.insert_temp(id_largura, ocupada));
                ui.ctx().request_discard("a caixa do aviso tem a largura do texto");
            }
        }
        // a linha tem a altura do mais alto: o texto ou o sólido
        let altura = altura_texto.max(LADO_ICONE) + recuo_topo + recuo_base;
        let rect = Rect::from_min_size(inicio + vec2(recuo, 0.0), vec2(largura, altura));
        let resp = ui.allocate_rect(rect, Sense::hover());
        match self.faixa {
            Some(cantos) => {
                ui.painter().set(lugar_fundo, egui::Shape::rect_filled(rect, cantos, fundo));
                ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, fio));
            }
            None => {
                ui.painter().set(lugar_fundo, egui::Shape::rect_filled(rect, RAIO_AVISO, fundo));
                ui.painter().rect_stroke(rect, RAIO_AVISO, Stroke::new(1.0, fio), StrokeKind::Inside);
            }
        }
        // o sólido fica centrado na faixa (sem contar o fio de baixo da faixa do documento); o `.erro` também
        // (`align-items:center`, prancheta 15)
        let meio = if self.faixa.is_some() { rect.center().y - 0.5 } else { rect.center().y };
        let y_icone = if self.icone_no_topo { rect.top() + recuo_topo } else { meio - LADO_ICONE / 2.0 };
        let caixa = Rect::from_min_size(pos2(rect.left() + recuo_x, y_icone), vec2(LADO_ICONE, LADO_ICONE));
        pintar_icone_estado(ui.painter(), caixa, icone);
        ui.ctx().accesskit_node_builder(resp.id, |n| n.set_role(egui::accesskit::Role::Status));
        if primeiro {
            ui.add_space((ESPACO_APOS_AVISO - ui.spacing().item_spacing.y).max(0.0));
        }
        (resp, saida)
    }
}
