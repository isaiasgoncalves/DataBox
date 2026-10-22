//! Texto do visual Estúdio: montagem de `Galley` (com caixa alta e espaçamento entre letras, que o egui não tem
//! como estilo) e as peças de texto simples: [`h2`] (20/600), [`h3`] (14/600, o `.dlg h2`), [`h4`], [`eyebrow`] e
//! [`rotulo_caps`] (os três são o `.tit`: 11/600, caixa alta, `.06em`, `--t3`), [`hint`], [`texto_cortado`],
//! [`forte`], [`link`] e [`nome_abre`] (o `.link`: `--acct` 600, `--t` no hover), [`codigo`] (a fonte da interface
//! em peso 350 sobre `--h`: a monoespaçada fica só nos editores) e [`texto_rico`] (trechos com `strong`, `code`,
//! links clicáveis no meio da frase e o "i" mini; selecionável e copiável, como o texto da página web).
//! [`galley_caminho`] corta um caminho no meio da pasta, com o nome do arquivo inteiro.

use std::ops::Range;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align, Color32, FontId, Galley, Id, Response, Sense, Ui, Widget, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pintura;
use crate::tema::paleta;

/// Formato de um trecho: fonte, cor, espaçamento entre letras (em pontos) e altura de linha.
pub fn formato(fonte: FontId, cor: Color32) -> TextFormat {
    TextFormat { font_id: fonte, color: cor, valign: Align::Center, ..TextFormat::default() }
}

/// O alto de um galley de uma linha centrado numa linha do CSS que começa em `topo` e tem `altura_linha` (o
/// navegador reparte a sobra em cima e embaixo; com o ajuste de entrelinha das fontes, centrar dá a mesma
/// linha de base).
pub fn y_na_linha(topo: f32, altura_linha: f32, g: &Galley) -> f32 {
    topo + (altura_linha - g.size().y) / 2.0
}

/// Ascendente e descendente da `hhea` da Segoe UI (2210 e 514 em 2048; a entrelinha é zero), em frações do corpo.
const ASCENDENTE: f32 = 2210.0 / 2048.0;
const DESCENDENTE: f32 = 514.0 / 2048.0;

/// O alto de um galley de uma linha da fonte da interface de `tamanho` para a linha de base dele cair em `base`
/// (para alinhar textos de tamanhos diferentes na mesma linha). Com o galley em mãos, prefira [`linha_de_base`].
pub fn y_para_linha_de_base(base: f32, tamanho: f32) -> f32 {
    // a linha de base do egui fica um ascendente abaixo do alto da linha
    base - ASCENDENTE * tamanho
}

/// A linha de base do CSS numa linha que começa em `topo` e tem `altura_linha`, para a fonte da interface de
/// `tamanho`.
pub fn linha_de_base_css(topo: f32, altura_linha: f32, tamanho: f32) -> f32 {
    // metade da linha + (ascendente − descendente) / 2
    topo + altura_linha / 2.0 + (ASCENDENTE - DESCENDENTE) / 2.0 * tamanho
}

/// A linha de base da primeira linha de um galley, medida do alto dele (vale para qualquer fonte, também as de
/// reserva): para pôr dois textos de tamanhos diferentes na mesma linha de base, como o sufixo dos números da
/// Visão geral.
pub fn linha_de_base(g: &Galley) -> f32 {
    g.rows
        .first()
        .and_then(|l| l.row.glyphs.first().map(|c| l.pos.y + c.pos.y))
        .unwrap_or(ASCENDENTE * g.size().y / (ASCENDENTE + DESCENDENTE))
}

/// Uma linha, sem quebrar.
pub fn galley(ui: &Ui, texto: &str, fonte: FontId, cor: Color32) -> Arc<Galley> {
    let job = LayoutJob::single_section(texto.to_owned(), formato(fonte, cor));
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Uma linha cortada com "…" se passar de `largura`.
pub fn galley_cortado(ui: &Ui, texto: &str, fonte: FontId, cor: Color32, largura: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(texto.to_owned(), formato(fonte, cor));
    job.wrap =
        TextWrapping {
            max_width: largura.max(0.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…')
        };
    job.break_on_newline = false;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// O caminho que cabe (`cabe` mede o texto): inteiro, se couber; senão, com o nome do arquivo inteiro e a pasta
/// cortada no meio com "…" ("C:\dados\…\2026\vendas.db", item 17 da revisão). `None` quando nem "…\nome" cabe, ou
/// quando não há pasta.
pub fn encurtar_caminho(caminho: &str, cabe: impl Fn(&str) -> bool) -> Option<String> {
    if cabe(caminho) {
        return Some(caminho.to_owned());
    }
    let corte = caminho.rfind(['\\', '/'])?;
    let (pasta, nome) = (&caminho[..corte], &caminho[corte..]);
    let letras: Vec<char> = pasta.chars().collect();
    let montar = |k: usize| -> String {
        let (inicio, fim) = (k.div_ceil(2), k / 2);
        let mut s: String = letras[..inicio].iter().collect();
        s.push('…');
        s.extend(&letras[letras.len() - fim..]);
        s.push_str(nome);
        s
    };
    if !cabe(&montar(0)) {
        return None;
    }
    // o maior número de letras da pasta que ainda cabe (a medida cresce com as letras)
    let (mut cabe_k, mut nao_cabe) = (0, letras.len());
    while nao_cabe - cabe_k > 1 {
        let meio = (cabe_k + nao_cabe) / 2;
        if cabe(&montar(meio)) {
            cabe_k = meio;
        } else {
            nao_cabe = meio;
        }
    }
    Some(montar(cabe_k))
}

/// Uma linha com o caminho que cabe em `largura` ([`encurtar_caminho`]: a reticência no meio da pasta e o nome do
/// arquivo inteiro); se nem o nome couber, ele é cortado no fim.
pub fn galley_caminho(ui: &Ui, caminho: &str, fonte: FontId, cor: Color32, largura: f32) -> Arc<Galley> {
    let cabe = |s: &str| galley(ui, s, fonte.clone(), cor).size().x <= largura + 0.5;
    match encurtar_caminho(caminho, cabe) {
        Some(texto) => galley(ui, &texto, fonte, cor),
        None => {
            let nome = caminho.rsplit(['\\', '/']).next().unwrap_or(caminho);
            galley_cortado(ui, nome, fonte, cor, largura)
        }
    }
}

/// Texto em várias linhas com a entrelinha do CSS. O egui põe o texto no alto de cada linha e a sobra toda
/// embaixo; o CSS divide a sobra ("half-leading"). [`Paragrafo::pintar`] desce o bloco a metade da sobra.
#[derive(Clone)]
pub struct Paragrafo {
    pub galley: Arc<Galley>,
    /// Quanto descer o galley para o texto ficar no meio de cada linha.
    pub deslocamento: f32,
    /// Num parágrafo de [`Trecho`]s ([`paragrafo_rico`]): os caracteres de cada trecho no texto do galley (para
    /// achar os links e os "i" depois de quebrar as linhas). Vazio nos outros.
    pub faixas: Vec<Range<usize>>,
}

impl Paragrafo {
    pub fn tamanho(&self) -> egui::Vec2 {
        self.galley.size()
    }

    pub fn pintar(&self, painter: &egui::Painter, pos: egui::Pos2, cor: Color32) {
        painter.galley(pos + vec2(0.0, self.deslocamento), Arc::clone(&self.galley), cor);
    }
}

/// Metade da sobra entre a altura de linha do CSS e a altura de linha da fonte no egui.
pub fn meia_entrelinha(ui: &Ui, fonte: &FontId, altura_linha: f32) -> f32 {
    let natural = ui.fonts_mut(|f| f.row_height(fonte));
    ((altura_linha - natural) / 2.0).max(0.0).round()
}

/// Texto que quebra em `largura`, com altura de linha = tamanho × `fator_linha` (o `line-height` do CSS).
pub fn galley_quebrado(ui: &Ui, texto: &str, fonte: FontId, cor: Color32, largura: f32, fator_linha: f32) -> Paragrafo {
    let altura_linha = (fonte.size * fator_linha).round();
    let deslocamento = meia_entrelinha(ui, &fonte, altura_linha);
    let mut f = formato(fonte, cor);
    f.line_height = Some(altura_linha);
    let mut job = LayoutJob::single_section(texto.to_owned(), f);
    job.wrap = TextWrapping { max_width: largura.max(1.0), ..TextWrapping::default() };
    Paragrafo { galley: ui.fonts_mut(|f| f.layout_job(job)), deslocamento, faixas: Vec::new() }
}

/// Um `LayoutJob` já montado (com `line_height` nas seções) como [`Paragrafo`]; `fonte_base` e `altura_linha`
/// dão o deslocamento.
pub fn paragrafo_de_job(ui: &Ui, job: LayoutJob, fonte_base: &FontId, altura_linha: f32) -> Paragrafo {
    let deslocamento = meia_entrelinha(ui, fonte_base, altura_linha);
    Paragrafo { galley: ui.fonts_mut(|f| f.layout_job(job)), deslocamento, faixas: Vec::new() }
}

/// Caixa alta com espaçamento (`text-transform:uppercase; letter-spacing:<em>em`) em peso 700 (o `.pill`). Os
/// títulos de seção e rótulos usam o [`galley_tit`].
pub fn galley_caps(ui: &Ui, texto: &str, tamanho: f32, espacamento_em: f32, cor: Color32) -> Arc<Galley> {
    let mut f = formato(fontes::fonte(tamanho, Peso::Pesado), cor);
    f.extra_letter_spacing = espacamento_em * tamanho;
    let job = LayoutJob::single_section(texto.to_uppercase(), f);
    ui.fonts_mut(|fs| fs.layout_job(job))
}

/// Tamanho do `.tit` (`font-size:11px`).
pub const TAMANHO_TIT: f32 = 11.0;
/// Altura da linha do `.tit` (11px × 1,4).
pub const ALTURA_TIT: f32 = 15.0;

/// O `.tit` das pranchetas (título de seção, rótulo de campo): 11px / 600, caixa alta, `letter-spacing:.06em`.
pub fn galley_tit(ui: &Ui, texto: &str, cor: Color32) -> Arc<Galley> {
    galley_tit_em(ui, texto, TAMANHO_TIT, cor)
}

/// O `.tit` em outro tamanho (o título do cabeçalho dos painéis, `.cab .tit{font-size:10.5px}`).
pub fn galley_tit_em(ui: &Ui, texto: &str, tamanho: f32, cor: Color32) -> Arc<Galley> {
    let mut f = formato(fontes::fonte(tamanho, Peso::Medio), cor);
    f.extra_letter_spacing = 0.06 * tamanho;
    let job = LayoutJob::single_section(texto.to_uppercase(), f);
    ui.fonts_mut(|fs| fs.layout_job(job))
}

/// Aloca o tamanho do galley e pinta (texto não interativo). `sense` decide se é clicável.
pub(crate) fn mostrar_galley(ui: &mut Ui, g: Arc<Galley>, sense: Sense) -> Response {
    let (rect, resp) = ui.allocate_exact_size(g.size(), sense);
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, Color32::PLACEHOLDER);
    }
    resp
}

fn rotulo_acessivel(resp: Response, ui: &Ui, texto: &str) -> Response {
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), texto));
    resp
}

/// Título de página (`h2`): 20px / 600, `--t`.
pub fn h2(ui: &mut Ui, texto: &str) -> Response {
    let p = paleta(ui);
    let g = galley(ui, texto, fontes::fonte(20.0, Peso::Medio), p.estudio.t);
    let r = mostrar_galley(ui, g, Sense::hover());
    rotulo_acessivel(r, ui, texto)
}

/// Título de janela e de documento (`h3`, o `.dlg h2` das pranchetas): 14px / 600, `--t`.
pub fn h3(ui: &mut Ui, texto: &str) -> Response {
    let p = paleta(ui);
    let g = galley(ui, texto, fontes::fonte(14.0, Peso::Medio), p.estudio.t);
    let r = mostrar_galley(ui, g, Sense::hover());
    rotulo_acessivel(r, ui, texto)
}

/// O título de uma lista no corpo do documento: 13px / 600, `--t` (o `h2` de "Tabelas" na Visão geral, as listas da
/// Estrutura e dos Índices, o título da faixa das partes). O espaço até a lista fica com quem chama.
pub fn titulo_lista(ui: &mut Ui, titulo: &str) -> Response {
    let p = paleta(ui);
    let g = galley(ui, titulo, fontes::fonte(TAMANHO_TITULO_LISTA, Peso::Medio), p.estudio.t);
    let r = mostrar_galley(ui, g, Sense::hover());
    rotulo_acessivel(r, ui, titulo)
}

/// O tamanho do [`titulo_lista`].
pub const TAMANHO_TITULO_LISTA: f32 = 13.0;

/// O `.tit` alocado numa linha de [`ALTURA_TIT`].
fn mostrar_tit(ui: &mut Ui, texto: &str) -> Response {
    let p = paleta(ui);
    let g = galley_tit(ui, texto, p.estudio.t3);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x, ALTURA_TIT), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(pos2(rect.left(), y_na_linha(rect.top(), ALTURA_TIT, &g)), g, p.estudio.t3);
    }
    rotulo_acessivel(resp, ui, texto)
}

/// Subtítulo dentro de um corpo ("SQL que será executado"): o `.tit` (11px / 600, caixa alta, `.06em`, `--t3`).
/// A margem fica por conta de quem chama.
pub fn h4(ui: &mut Ui, texto: &str) -> Response {
    mostrar_tit(ui, texto)
}

/// O antigo `.eyebrow`: no Estúdio é o mesmo `.tit` de [`h4`] (sem o ouro).
pub fn eyebrow(ui: &mut Ui, texto: &str) -> Response {
    mostrar_tit(ui, texto)
}

/// Rótulo em caixa alta (rótulo de seção, de grupo): o `.tit` de [`h4`]. O `espacamento_em` do kit antigo (`.12`,
/// `.10`) fica sem efeito: o `.tit` tem `.06em`.
pub fn rotulo_caps(ui: &mut Ui, texto: &str, _espacamento_em: f32) -> Response {
    mostrar_tit(ui, texto)
}

/// Texto auxiliar (`.hint`): 11.5px, `--t3`, numa linha.
pub fn hint(ui: &mut Ui, texto: &str) -> Response {
    hint_tamanho(ui, texto, 11.5)
}

/// `.hint` com outro tamanho (11px nos rodapés).
pub fn hint_tamanho(ui: &mut Ui, texto: &str, tamanho: f32) -> Response {
    let p = paleta(ui);
    let g = galley(ui, texto, fontes::fonte(tamanho, Peso::Normal), p.faint);
    let r = mostrar_galley(ui, g, Sense::hover());
    rotulo_acessivel(r, ui, texto)
}

/// Texto comum (400) no tamanho e na cor da paleta pedidos, numa linha.
pub fn texto(ui: &mut Ui, texto: &str, tamanho: f32, cor: Color32) -> Response {
    let g = galley(ui, texto, fontes::fonte(tamanho, Peso::Normal), cor);
    let r = mostrar_galley(ui, g, Sense::hover());
    rotulo_acessivel(r, ui, texto)
}

/// Parágrafo que quebra na largura disponível (`line-height:1.5`).
pub fn paragrafo(ui: &mut Ui, conteudo: &str, tamanho: f32, cor: Color32) -> Response {
    let largura = ui.available_width();
    let g = galley_quebrado(ui, conteudo, fontes::fonte(tamanho, Peso::Normal), cor, largura, 1.5);
    mostrar_paragrafo(ui, &g, conteudo)
}

/// Aloca e pinta um [`Paragrafo`].
pub fn mostrar_paragrafo(ui: &mut Ui, g: &Paragrafo, rotulo: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(g.tamanho(), Sense::hover());
    if ui.is_rect_visible(rect) {
        g.pintar(ui.painter(), rect.min, Color32::PLACEHOLDER);
    }
    rotulo_acessivel(resp, ui, rotulo)
}

/// `strong` (700) na cor do texto.
pub fn forte(ui: &mut Ui, texto: &str) -> Response {
    let p = paleta(ui);
    let g = galley(ui, texto, fontes::fonte(13.0, Peso::Pesado), p.estudio.t);
    let r = mostrar_galley(ui, g, Sense::hover());
    rotulo_acessivel(r, ui, texto)
}

/// O texto clicável do `.link` (`color:--acct; font-weight:600`, sem sublinhado; `--t` no hover), com o anel de
/// foco do teclado. `cor` é a cor fora do hover.
fn texto_clicavel(ui: &mut Ui, texto: &str, tamanho: f32, cor: Color32, tipo: WidgetType) -> Response {
    let p = paleta(ui);
    let id = ui.next_auto_id();
    let hover = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let cor = if hover { p.estudio.t } else { cor };
    let g = galley(ui, texto, fontes::fonte(tamanho, Peso::Medio), cor);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::click());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, cor);
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(ui.painter(), p, rect, 4.0);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.widget_info(|| WidgetInfo::labeled(tipo, ui.is_enabled(), texto));
    resp
}

/// Link de texto (`.link`, `a`): 12px / 600 em `--acct`, sem sublinhado; no hover vira `--t`.
pub fn link(ui: &mut Ui, texto: &str) -> Response {
    link_tamanho(ui, texto, 12.0)
}

/// [`link`] em outro tamanho.
pub fn link_tamanho(ui: &mut Ui, texto: &str, tamanho: f32) -> Response {
    let cor = paleta(ui).estudio.acct;
    texto_clicavel(ui, texto, tamanho, cor, WidgetType::Link)
}

/// O atalho de SQL das Propriedades (`a.atalho` de `Dados.dc.html`: "SELECT * FROM clientes LIMIT 100"): a largura
/// toda, `padding:7px 10px`, raio [`pintura::RAIO_PADRAO`], fundo `--h`, borda de 1px `--vidrob` e o texto 12px em
/// `--acct` numa linha (cortado com "…", com o texto inteiro na dica); no hover, a borda `--acc` e o halo de 12px em
/// `--brilho` (`.atalho:hover`). É um link para o leitor de tela.
pub fn atalho(ui: &mut Ui, texto: &str) -> Response {
    let largura = ui.available_width();
    atalho_na_largura(ui, texto, largura)
}

/// O [`atalho`] com a largura do texto (até a disponível), para uma fileira que quebra (`display:flex;
/// flex-wrap:wrap; gap:8px`): as sugestões da Consulta vazia (prancheta 15, "Para começar").
pub fn atalho_justo(ui: &mut Ui, texto: &str) -> Response {
    let natural = galley(ui, texto, fontes::fonte(12.0, Peso::Normal), paleta(ui).estudio.acct).size().x;
    let largura = (natural + 2.0 * (10.0 + 1.0)).ceil().min(ui.available_width());
    atalho_na_largura(ui, texto, largura)
}

fn atalho_na_largura(ui: &mut Ui, texto: &str, largura: f32) -> Response {
    let p = paleta(ui);
    let e = &p.estudio;
    // `padding:7px 10px` e a borda de 1px
    let (recuo_x, recuo_y) = (10.0 + 1.0, 7.0 + 1.0);
    let g = galley_cortado(ui, texto, fontes::fonte(12.0, Peso::Normal), e.acct, (largura - 2.0 * recuo_x).max(1.0));
    let cortado = g.size().x + 0.5 < galley(ui, texto, fontes::fonte(12.0, Peso::Normal), e.acct).size().x;
    let altura = (g.size().y + 2.0 * recuo_y).ceil();
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, altura), Sense::click());
    let hover =
        ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), crate::tema::duracao_transicao(ui));
    if ui.is_rect_visible(rect.expand(16.0)) {
        let raio = pintura::RAIO_PADRAO;
        if hover > 0.0 {
            let fora = crate::pecas::botao::pintor_de_fora(ui, rect);
            pintura::halo(&fora, rect, raio, 12.0, e.brilho.gamma_multiply(hover));
        }
        let painter = ui.painter();
        painter.rect_filled(rect, raio, e.h);
        let borda = pintura::misturar(e.vidrob, e.acc, hover);
        painter.rect_stroke(rect, raio, egui::Stroke::new(1.0, borda), egui::StrokeKind::Inside);
        painter.galley(pos2(rect.left() + recuo_x, rect.center().y - g.size().y / 2.0), g, e.acct);
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(painter, p, rect, raio);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Link, ui.is_enabled(), texto));
    if cortado { resp.on_hover_text(texto) } else { resp }
}

/// O nome que abre a entidade (o `a.link` das listas): 12px / 600 em `--acct`, `--t` no hover. `inativo` usa
/// `--t3` (a linha `tr.inativo`).
pub fn nome_abre(ui: &mut Ui, texto: &str, inativo: bool) -> Response {
    nome_abre_tamanho(ui, texto, inativo, 12.0)
}

/// [`nome_abre`] num tamanho dado.
pub fn nome_abre_tamanho(ui: &mut Ui, texto: &str, inativo: bool, tamanho: f32) -> Response {
    let e = &paleta(ui).estudio;
    let cor = if inativo { e.t3 } else { e.acct };
    texto_clicavel(ui, texto, tamanho, cor, WidgetType::Button)
}

/// `code` em linha (tipos, nomes de colunas, trechos de SQL): a fonte da interface em peso 350 (o `.mono` das
/// pranchetas; a monoespaçada fica só dentro dos editores), 12px `--t`, sobre `--h` com raio 4 e `padding:1px 6px`.
pub fn codigo(ui: &mut Ui, texto: &str) -> Response {
    let p = paleta(ui);
    let g = galley(ui, texto, fontes::fonte(12.0, Peso::Leve), p.estudio.t);
    let tamanho = g.size() + vec2(12.0, 2.0);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 4.0, p.estudio.h);
        ui.painter().galley(pos2(rect.left() + 6.0, rect.top() + 1.0), g, p.estudio.t);
    }
    rotulo_acessivel(resp, ui, texto)
}

/// Um trecho de [`texto_rico`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trecho<'a> {
    /// Texto na cor normal do bloco.
    Normal(&'a str),
    /// `strong`: 700, na cor forte do bloco.
    Forte(&'a str),
    /// `code`: a fonte da interface em peso 350 (no tamanho do bloco) sobre `--h`.
    Codigo(&'a str),
    /// `.hint` (11.5px, `--t3`).
    Fraco(&'a str),
    /// Quebra de linha (`<br>`).
    Quebra,
    /// Link de texto (`a`) no meio do parágrafo: 600 em `--acct`, sem sublinhado, `--t` no hover; clicável e
    /// focável. Em [`texto_rico`] e [`crate::pecas::avisos::aviso`], o índice dele em `trechos` volta em
    /// [`RespostaTexto::link`] (o "Ir para a Consulta SQL" do aviso de transação, o "Ir para o banco" do Início).
    /// Num parágrafo só pintado, é só texto.
    Link(&'a str),
    /// O "i" mini (14px, 5px depois do trecho anterior) com o balão destes trechos: o "Mensagem original do
    /// SQLite" no fim da frase do erro da Consulta SQL. Só em [`texto_rico`] e no aviso.
    Ajuda(&'a [Trecho<'a>]),
}

/// As cores de um parágrafo de [`Trecho`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoresTexto {
    /// Texto comum.
    pub cor: Color32,
    /// `strong` e `code`.
    pub forte: Color32,
    /// Link (`a`).
    pub link: Color32,
    /// Link sob o ponteiro (`a:hover{color:--t}`).
    pub link_realce: Color32,
    /// Fundo do `code`.
    pub fundo_codigo: Color32,
    /// `.hint`.
    pub fraco: Color32,
}

impl CoresTexto {
    /// Com os links e os trechos fracos da paleta (`--acct` → `--t` no hover, `--t3`, o `code` sobre `--h`).
    pub fn new(p: &crate::tema::Paleta, cor: Color32, forte: Color32) -> Self {
        let e = &p.estudio;
        CoresTexto { cor, forte, link: e.acct, link_realce: e.t, fundo_codigo: e.h, fraco: e.t3 }
    }
}

/// Largura reservada para o "i" mini no texto (5px de margem + 14px).
const LARGURA_AJUDA_MINI: f32 = 19.0;

/// Monta o parágrafo e devolve também os caracteres de cada trecho. `realces` são os índices dos links sob o
/// ponteiro (em `--t`); `espaco` é a largura do espaço sem quebra na fonte do bloco.
fn montar_rico(
    trechos: &[Trecho<'_>],
    tamanho: f32,
    cores: &CoresTexto,
    largura: f32,
    fator_linha: f32,
    realces: &[usize],
    espaco: f32,
) -> (LayoutJob, Vec<Range<usize>>) {
    let altura_linha = (tamanho * fator_linha).round();
    let mut job = LayoutJob {
        wrap: TextWrapping { max_width: largura.max(1.0), ..TextWrapping::default() },
        ..LayoutJob::default()
    };
    let base = |fonte: FontId, cor: Color32| TextFormat {
        font_id: fonte,
        color: cor,
        line_height: Some(altura_linha),
        valign: Align::Center,
        ..TextFormat::default()
    };
    let mut faixas = Vec::with_capacity(trechos.len());
    let mut n = 0;
    for (i, t) in trechos.iter().enumerate() {
        let inicio = n;
        let mut anexar = |job: &mut LayoutJob, s: &str, f: TextFormat| {
            n += s.chars().count();
            job.append(s, 0.0, f);
        };
        match *t {
            Trecho::Normal(s) => anexar(&mut job, s, base(fontes::fonte(tamanho, Peso::Normal), cores.cor)),
            Trecho::Forte(s) => anexar(&mut job, s, base(fontes::fonte(tamanho, Peso::Pesado), cores.forte)),
            Trecho::Codigo(s) => {
                let mut f = base(fontes::fonte(tamanho, Peso::Leve), cores.forte);
                f.background = cores.fundo_codigo;
                f.expand_bg = 1.5;
                anexar(&mut job, s, f);
            }
            Trecho::Fraco(s) => anexar(&mut job, s, base(fontes::fonte(11.5, Peso::Normal), cores.fraco)),
            Trecho::Quebra => anexar(&mut job, "\n", base(fontes::fonte(tamanho, Peso::Normal), cores.cor)),
            Trecho::Link(s) => {
                let cor = if realces.contains(&i) { cores.link_realce } else { cores.link };
                anexar(&mut job, s, base(fontes::fonte(tamanho, Peso::Medio), cor));
            }
            Trecho::Ajuda(_) => {
                // um espaço sem quebra alargado guarda o lugar do "i" (que é desenhado por cima, depois)
                let mut f = base(fontes::fonte(tamanho, Peso::Normal), Color32::TRANSPARENT);
                f.extra_letter_spacing = (LARGURA_AJUDA_MINI - espaco).max(0.0);
                anexar(&mut job, "\u{a0}", f);
            }
        }
        faixas.push(inicio..n);
    }
    (job, faixas)
}

/// Monta um parágrafo com trechos (o `notice` com `<strong>`, por exemplo) que quebra em `largura`.
pub fn job_rico(
    trechos: &[Trecho<'_>],
    tamanho: f32,
    cor: Color32,
    cor_forte: Color32,
    fundo_codigo: Color32,
    faint: Color32,
    largura: f32,
) -> LayoutJob {
    let cores = CoresTexto {
        cor,
        forte: cor_forte,
        link: crate::tema::CLARA.estudio.acct,
        link_realce: cor_forte,
        fundo_codigo,
        fraco: faint,
    };
    // o espaço da Segoe UI: 562 de 2048
    montar_rico(trechos, tamanho, &cores, largura, 1.5, &[], 0.274 * tamanho).0
}

/// [`job_rico`] pronto para pintar, com a entrelinha do CSS (1.5, ou `fator_linha`).
pub fn paragrafo_rico(
    ui: &Ui,
    trechos: &[Trecho<'_>],
    tamanho: f32,
    cor: Color32,
    cor_forte: Color32,
    largura: f32,
    fator_linha: f32,
) -> Paragrafo {
    let cores = CoresTexto::new(paleta(ui), cor, cor_forte);
    paragrafo_rico_com(ui, trechos, tamanho, &cores, largura, fator_linha, &[])
}

/// [`paragrafo_rico`] com todas as cores e os links em realce (`realces`, índices em `trechos`).
pub fn paragrafo_rico_com(
    ui: &Ui,
    trechos: &[Trecho<'_>],
    tamanho: f32,
    cores: &CoresTexto,
    largura: f32,
    fator_linha: f32,
    realces: &[usize],
) -> Paragrafo {
    let fonte = fontes::fonte(tamanho, Peso::Normal);
    let espaco = ui.fonts_mut(|f| f.glyph_width(&fonte, '\u{a0}'));
    let (job, faixas) = montar_rico(trechos, tamanho, cores, largura, fator_linha, realces, espaco);
    let altura_linha = (tamanho * fator_linha).round();
    let mut g = paragrafo_de_job(ui, job, &fonte, altura_linha);
    g.faixas = faixas;
    g
}

/// Os retângulos (um por linha quebrada) dos caracteres `faixa` de um galley, com a linha de base de cada um,
/// em coordenadas do galley.
pub fn retangulos_da_faixa(galley: &Galley, faixa: &Range<usize>) -> Vec<(egui::Rect, f32)> {
    let mut saida = Vec::new();
    let mut indice = 0;
    for linha in &galley.rows {
        let n = linha.row.glyphs.len();
        let (ini, fim) = (faixa.start.max(indice), faixa.end.min(indice + n));
        if ini < fim
            && let (Some(g0), Some(g1)) = (linha.row.glyphs.get(ini - indice), linha.row.glyphs.get(fim - 1 - indice))
        {
            let r = linha.rect();
            let x0 = linha.pos.x + g0.pos.x;
            let x1 = linha.pos.x + g1.max_x();
            saida.push((egui::Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom())), linha.pos.y + g0.pos.y));
        }
        indice += n + usize::from(linha.ends_with_newline);
    }
    saida
}

/// O que um parágrafo de [`Trecho`]s devolve.
#[derive(Debug, Clone)]
pub struct RespostaTexto {
    /// A resposta do bloco de texto.
    pub resposta: Response,
    /// O índice (em `trechos`) do [`Trecho::Link`] clicado neste quadro.
    pub link: Option<usize>,
}

/// Mostra um parágrafo de trechos já montado (de [`paragrafo_rico_com`]) no lugar `pos`, que quem chama já
/// reservou: o texto selecionável e copiável (`selecionavel`: arrastar seleciona, Ctrl+C copia, como o texto da
/// página web), os [`Trecho::Link`] clicáveis e os [`Trecho::Ajuda`] com o "i" mini. `id` guarda o realce dos
/// links de um quadro para o outro (passe o mesmo `id` a [`realces_links`] ao montar o parágrafo). O tamanho do
/// texto (`_tamanho`) servia ao sublinhado do link do kit antigo e fica na assinatura.
#[allow(clippy::too_many_arguments)]
pub fn interagir_paragrafo(
    ui: &mut Ui,
    id: Id,
    resposta: &Response,
    g: &Paragrafo,
    pos: egui::Pos2,
    trechos: &[Trecho<'_>],
    _tamanho: f32,
    selecionavel: bool,
) -> Option<usize> {
    let p = paleta(ui);
    let origem = pos + vec2(0.0, g.deslocamento);
    if selecionavel {
        egui::text_selection::LabelSelectionState::label_text_selection(
            ui,
            resposta,
            origem,
            Arc::clone(&g.galley),
            p.fg,
            egui::Stroke::NONE,
        );
    } else {
        ui.painter().galley(origem, Arc::clone(&g.galley), p.fg);
    }
    let mut clicado = None;
    let mut realces = Vec::new();
    for (i, t) in trechos.iter().enumerate() {
        let Some(faixa) = g.faixas.get(i) else { continue };
        match t {
            Trecho::Link(rotulo) => {
                // o `a` das pranchetas não tem sublinhado: a cor (`--acct` → `--t`) e o peso bastam
                for (k, (r, _)) in retangulos_da_faixa(&g.galley, faixa).into_iter().enumerate() {
                    let r = r.translate(origem.to_vec2());
                    let resp = ui.interact(r, id.with(("link", i, k)), Sense::click());
                    if resp.hovered() {
                        realces.push(i);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        clicado = Some(i);
                    }
                    if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
                        pintura::anel_foco(ui.painter(), p, r, 6.0);
                    }
                    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Link, ui.is_enabled(), *rotulo));
                }
            }
            Trecho::Ajuda(balao) => {
                if let Some((r, _)) = retangulos_da_faixa(&g.galley, faixa).into_iter().next() {
                    let r = r.translate(origem.to_vec2());
                    // o egui põe o espaçamento extra antes do glifo: o lugar guardado termina no fim do espaço sem
                    // quebra, e o "i" de 14px fica encostado nesse fim (5px depois do trecho anterior)
                    let icone = egui::Rect::from_min_size(pos2(r.right() - 14.0, r.center().y - 7.0), vec2(14.0, 14.0));
                    let mut filho = ui.new_child(egui::UiBuilder::new().id_salt(id.with(("ajuda", i))).max_rect(icone));
                    crate::pecas::ajuda::Ajuda::new(balao).mini(true).mostrar(&mut filho);
                }
            }
            _ => {}
        }
    }
    let antes = realces_links(ui, id);
    if antes != realces {
        ui.ctx().data_mut(|d| d.insert_temp(id.with("realces"), realces));
        ui.ctx().request_repaint();
    }
    clicado
}

/// Os links sob o ponteiro no quadro anterior (para montar o parágrafo com a cor do hover).
pub fn realces_links(ui: &Ui, id: Id) -> Vec<usize> {
    ui.ctx().data(|d| d.get_temp::<Vec<usize>>(id.with("realces"))).unwrap_or_default()
}

/// Sentido de um bloco de texto selecionável: hover e arrastar (sem foco pelo Tab, como o `Label` do egui).
pub fn sentido_selecionavel() -> Sense {
    let mut s = Sense::click_and_drag();
    s -= Sense::FOCUSABLE;
    s | Sense::hover()
}

/// Parágrafo com trechos (normais, fortes, de código, links e o "i" mini), quebrando na largura disponível;
/// o texto pode ser selecionado e copiado.
pub fn texto_rico(
    ui: &mut Ui,
    trechos: &[Trecho<'_>],
    tamanho: f32,
    cor: Color32,
    cor_forte: Color32,
) -> RespostaTexto {
    let id = ui.next_auto_id();
    let cores = CoresTexto::new(paleta(ui), cor, cor_forte);
    let realces = realces_links(ui, id);
    let g = paragrafo_rico_com(ui, trechos, tamanho, &cores, ui.available_width(), 1.5, &realces);
    let (rect, resposta) = ui.allocate_exact_size(g.tamanho(), sentido_selecionavel());
    let link = interagir_paragrafo(ui, id, &resposta, &g, rect.min, trechos, tamanho, true);
    let texto_inteiro = g.galley.text().replace('\u{a0}', " ");
    let resposta = rotulo_acessivel(resposta, ui, texto_inteiro.trim_end());
    RespostaTexto { resposta, link }
}

/// Aloca e pinta um [`Paragrafo`] com o texto selecionável e copiável (o caminho no `p.sub`, o valor da ficha).
pub fn mostrar_paragrafo_selecionavel(ui: &mut Ui, g: &Paragrafo, rotulo: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(g.tamanho(), sentido_selecionavel());
    if ui.is_rect_visible(rect) {
        egui::text_selection::LabelSelectionState::label_text_selection(
            ui,
            &resp,
            rect.min + vec2(0.0, g.deslocamento),
            Arc::clone(&g.galley),
            paleta(ui).fg,
            egui::Stroke::NONE,
        );
    }
    rotulo_acessivel(resp, ui, rotulo)
}

/// Uma linha que encurta com "…" na largura disponível (a pasta dos recentes: `td.caminho-curto`), com a dica do
/// texto inteiro quando encurtou.
pub fn texto_cortado(ui: &mut Ui, texto: &str, tamanho: f32, cor: Color32) -> Response {
    let largura = ui.available_width();
    let g = galley_cortado(ui, texto, fontes::fonte(tamanho, Peso::Normal), cor, largura);
    let cortado = g.elided;
    let r = mostrar_galley(ui, g, Sense::hover());
    let r = rotulo_acessivel(r, ui, texto);
    if cortado { r.on_hover_text(texto) } else { r }
}

/// Peça de texto pronta como `Widget` (para `ui.add`): rótulo comum de 12px em `--t`.
pub struct Rotulo<'a>(pub &'a str);

impl Widget for Rotulo<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let p = paleta(ui);
        texto(ui, self.0, 12.0, p.estudio.t)
    }
}
