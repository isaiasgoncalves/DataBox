//! A grade da aba Dados montada sobre o [`NucleoGrade`] do `interface`: a coluna da caixa de marcar (o `td.selc`
//! de 32px, sem fio) e a do número da linha (fixas e fora da navegação), as colunas de dados com o `th` que ordena,
//! as células desenhadas pelo `interface::grade`, o abridor da linha, a edição no lugar (`Entrada::compacta`) e o
//! verde do `obsSalvo`.
//!
//! O [`Pintor`] é o [`DesenhoGrade`] de um quadro: lê a página e devolve o que a pessoa fez (ordenar, abrir,
//! marcar, terminar a edição) em campos de saída, que a aba aplica depois de desenhar.

use banco::Direcao;
use comum::Celula;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{
    Align, Color32, CursorIcon, Event, EventFilter, FocusDirection, Key, Layout, Painter, Pos2, Rect, Sense, Ui,
    UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};
use interface::fontes::{self, Peso};
use interface::grade::{
    ALTURA_CABECALHO_COM_TIPO_ABRE, ALTURA_LINHA, CabecalhoVisivel, CelulaVisivel, DesenhoGrade, ESPACO_SETA_ORDEM,
    LADO_SETA_ORDEM, LARGURA_COLUNA_MARCAR, LARGURA_MAX_COLUNA, LARGURA_MIN_COLUNA, LARGURA_PASTILHA_TH, Navegacao,
    NucleoGrade, Ordem, Th, dica_celula, largura_cabecalho, largura_natural, padding_coluna, pintar_cabecalho_coluna,
    pintar_celula,
};
use interface::pecas::campos::{ALTURA_CAMPO_PEQUENO, Check, DURACAO_SALVO, Entrada, pintar_salvo};
use interface::pecas::texto::nome_abre_tamanho;
use interface::tema::Paleta;

use crate::modelo::{Dados, Edicao, FimEdicao, Marcadas};
use crate::regras::distribuir_larguras;

/// Largura da coluna da caixa de marcar (`.grade td.selc{width:32px;padding:0;text-align:center;border-right:0}`):
/// a caixa de 16px no meio.
const LARGURA_MARCA: f32 = LARGURA_COLUNA_MARCAR;
/// A caixa de marcar (`.check` de 16px).
const LADO_CAIXA: f32 = 16.0;
/// `padding-right` da coluna do número da linha.
const PADDING_ABRIR: f32 = 8.0;
/// O `td.obs` da edição no lugar: 6px dos lados e o campo compacto de 26px no meio da célula de 27px (a linha de
/// 28px sem o fio de baixo); campo de 140px no mínimo.
const PADDING_OBS_X: f32 = 6.0;
const LARGURA_MIN_OBS: f32 = 140.0;
/// `tbody` com a página nova a caminho (`aria-busy`: opacidade .55).
const OPACIDADE_OCUPADA: f32 = 0.55;

/// A paleta das células de uma linha: na marcada (fundo `--sel`), o terciário do NULL, do "vazio" e do número
/// passa a `--t2` (`.grade tr.mar td.lin, .grade tr.mar td.nulo{color:var(--t2)}`: o `--t3` cairia abaixo de
/// 4,5:1 sobre a seleção; revisão de 15/10/2026, item 1). O texto comum da linha não muda.
pub(crate) fn paleta_da_linha(p: &Paleta, marcada: bool) -> Paleta {
    let mut copia = *p;
    if marcada {
        copia.faint = copia.estudio.t2;
        copia.estudio.t3 = copia.estudio.t2;
    }
    copia
}

/// A grade de uma aba: o núcleo (rolagem, larguras, célula ativa) e as larguras naturais das colunas.
pub(crate) struct GradeDados {
    pub nucleo: NucleoGrade,
    /// Larguras pelo conteúdo (calculadas quando a página chega); o que sobra do card é repartido entre as de dados.
    naturais: Vec<f32>,
    /// As larguras que a aba deu ao núcleo no último quadro (para notar quando a pessoa arrasta uma borda).
    aplicadas: Vec<f32>,
    /// A pessoa arrastou uma borda: as larguras ficam como ela deixou.
    manual: bool,
    pub tem_marca: bool,
    pub tem_abrir: bool,
    /// Medir as colunas no próximo quadro (página nova).
    medir: bool,
    /// O canto do conteúdo da grade (sem rolagem) no último quadro: os testes acham as células por ele.
    pub origem: Option<Pos2>,
    /// O retângulo visível do corpo (abaixo do cabeçalho) no último quadro.
    pub visivel: Option<Rect>,
}

impl GradeDados {
    pub fn nova(nome: &str, tem_marca: bool, tem_abrir: bool, n_dados: usize) -> GradeDados {
        let primeira = usize::from(tem_marca) + usize::from(tem_abrir);
        let n = primeira + n_dados;
        let mut fixas = Vec::new();
        if tem_marca {
            fixas.push(0);
        }
        if tem_abrir {
            fixas.push(usize::from(tem_marca));
        }
        let mut nucleo = NucleoGrade::new(("tela_dados", nome), 0, vec![LARGURA_MIN_COLUNA; n])
            .com_navegacao(Navegacao::Dados)
            .com_altura_cabecalho(ALTURA_CABECALHO_COM_TIPO_ABRE)
            .com_navegaveis(primeira..n)
            .com_fixas(&fixas);
        if tem_marca {
            // `td.selc`: sem padding e sem o fio da direita
            nucleo = nucleo.com_coluna_marcar(0);
        }
        GradeDados {
            nucleo,
            naturais: Vec::new(),
            aplicadas: Vec::new(),
            manual: false,
            tem_marca,
            tem_abrir,
            medir: true,
            origem: None,
            visivel: None,
        }
    }

    /// O índice, no núcleo, da primeira coluna de dados.
    pub fn primeira(&self) -> usize {
        usize::from(self.tem_marca) + usize::from(self.tem_abrir)
    }

    /// Uma página nova: o número de linhas e, no próximo quadro, as larguras pelo conteúdo dela.
    pub fn nova_pagina(&mut self, n_linhas: usize) {
        self.nucleo.definir_linhas(n_linhas);
        self.medir = true;
    }

    /// Mede as colunas (se a página mudou) e reparte a largura disponível (a tabela de largura mínima 100%).
    pub fn ajustar_larguras(
        &mut self,
        ui: &Ui,
        p: &Paleta,
        dados: &Dados,
        cabecalho_abrir: Option<&str>,
        ordem: Option<&str>,
    ) {
        if self.medir {
            self.medir = false;
            self.naturais =
                medir_colunas(ui.painter(), p, dados, self.tem_marca, self.tem_abrir, cabecalho_abrir, ordem);
            self.manual = false;
        }
        if self.naturais.len() != self.nucleo.n_colunas() {
            return;
        }
        if !self.manual && !self.aplicadas.is_empty() && self.nucleo.larguras() != self.aplicadas.as_slice() {
            // a pessoa arrastou a borda de um `th` no quadro anterior
            self.manual = true;
        }
        if self.manual {
            return;
        }
        let mut novas = std::mem::take(&mut self.aplicadas);
        distribuir_larguras(&self.naturais, self.primeira(), ui.available_width(), &mut novas);
        if novas.as_slice() != self.nucleo.larguras() {
            self.nucleo.definir_larguras(novas.clone());
        }
        self.aplicadas = novas;
    }

    /// O centro de uma célula (linha, coluna de dados) no último quadro, se estiver à vista (para os testes).
    pub fn centro_celula(&self, linha: usize, coluna: usize) -> Option<Pos2> {
        let origem = self.origem?;
        let c = self.primeira() + coluna;
        let larguras = self.nucleo.larguras();
        let x0: f32 = larguras.iter().take(c).sum();
        let w = *larguras.get(c)?;
        let y = origem.y + self.nucleo.altura_do_cabecalho() + linha as f32 * ALTURA_LINHA + ALTURA_LINHA / 2.0;
        let pos = pos2(origem.x + x0 + w / 2.0, y);
        self.visivel.filter(|v| v.contains(pos)).map(|_| pos)
    }

    /// O centro da caixa de marcar de uma linha (ou a do cabeçalho, com `None`).
    pub fn centro_marca(&self, linha: Option<usize>) -> Option<Pos2> {
        let origem = self.origem?;
        if !self.tem_marca {
            return None;
        }
        // a caixa no meio da coluna
        let x = origem.x + LARGURA_MARCA / 2.0;
        let alt = self.nucleo.altura_do_cabecalho();
        let y = match linha {
            Some(l) => origem.y + alt + l as f32 * ALTURA_LINHA + (ALTURA_LINHA - 1.0) / 2.0,
            // a do cabeçalho, no meio do `th` (sem o fio de baixo)
            None => origem.y + (alt - 1.0) / 2.0,
        };
        Some(pos2(x, y))
    }
}

/// As larguras pelo conteúdo: caixa de marcar, número da linha e cada coluna de dados (cabeçalho e as primeiras
/// linhas), até 320px.
fn medir_colunas(
    painter: &Painter,
    p: &Paleta,
    dados: &Dados,
    tem_marca: bool,
    tem_abrir: bool,
    cabecalho_abrir: Option<&str>,
    ordem: Option<&str>,
) -> Vec<f32> {
    let primeira = usize::from(tem_marca) + usize::from(tem_abrir);
    let n = primeira + dados.colunas.len();
    let mut larguras = Vec::with_capacity(n);
    let largura_abre = |texto: &str| -> f32 {
        let job = egui::text::LayoutJob::single_section(
            texto.to_owned(),
            interface::pecas::texto::formato(fontes::fonte(12.0, Peso::Pesado), Color32::PLACEHOLDER),
        );
        painter.layout_job(job).size().x
    };
    if tem_marca {
        larguras.push(LARGURA_MARCA);
    }
    if tem_abrir {
        let c = usize::from(tem_marca);
        let (esq, _) = padding_coluna(c, n);
        let mut maior = cabecalho_abrir.map_or(0.0, |t| largura_cabecalho(painter, t, None));
        for rotulo in dados.rotulos.iter().take(200) {
            maior = maior.max(largura_abre(rotulo));
        }
        larguras.push((esq + maior + PADDING_ABRIR).ceil());
    }
    for (i, col) in dados.colunas.iter().enumerate() {
        let (esq, dir) = padding_coluna(primeira + i, n);
        let limite = LARGURA_MAX_COLUNA - esq - dir;
        // a pastilha antes do nome, e a seta da ordem ao lado dele (5px + a `.seta-ord` de 9)
        let seta = if ordem == Some(col.nome.as_str()) { ESPACO_SETA_ORDEM + LADO_SETA_ORDEM } else { 0.0 };
        let mut maior = LARGURA_PASTILHA_TH + largura_cabecalho(painter, &col.nome, None) + seta;
        for (linha, rotulo) in dados.pagina.linhas.iter().zip(&dados.rotulos).take(200) {
            if maior >= limite {
                break;
            }
            if let Some(cel) = linha.valores.get(i) {
                let w = if col.abre { largura_abre(rotulo) } else { largura_natural(painter, p, cel) };
                maior = maior.max(w);
            }
        }
        larguras.push((maior.min(limite) + esq + dir).ceil().max(LARGURA_MIN_COLUNA));
    }
    larguras
}

/// Uma célula gravando agora: o texto que a pessoa digitou fica nela até a resposta.
#[derive(Debug, Clone)]
pub(crate) struct Pendente {
    pub geracao: u64,
    pub linha: usize,
    pub coluna: usize,
    pub texto: String,
}

/// Uma célula que acabou de gravar (o verde do `obsSalvo`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Piscada {
    pub geracao: u64,
    pub linha: usize,
    pub coluna: usize,
    pub desde: f64,
}

/// O desenho de um quadro da grade: entradas (a página, o que está marcado, a edição) e saídas (o que a pessoa
/// fez).
pub(crate) struct Pintor<'a> {
    pub p: &'a Paleta,
    pub dados: &'a Dados,
    pub primeira: usize,
    pub tem_marca: bool,
    pub tem_abrir: bool,
    pub editavel: bool,
    pub marcadas: &'a Marcadas,
    pub edicao: Option<&'a mut Edicao>,
    pub pendentes: &'a [Pendente],
    pub piscadas: &'a [Piscada],
    pub ordem: Option<(&'a str, Direcao)>,
    pub ocupada: bool,
    pub titulo_abre: &'a str,
    /// O `th` da coluna do número da linha: o nome do rowid ("rowid"), ou nada (tabela sem rowid).
    pub cabecalho_abrir: Option<&'a str>,
    pub todas_marcadas: bool,
    pub algumas_marcadas: bool,
    /// A célula ativa está na ponta da grade para o Tab (última; com Shift, a primeira).
    pub ponta_ida: bool,
    pub ponta_volta: bool,
    pub agora: f64,
    /// As larguras do núcleo neste quadro e a altura do cabeçalho (para achar o canto do conteúdo).
    pub larguras: &'a [f32],
    pub altura_cabecalho: f32,
    /// A coluna escolhida nas Propriedades, acesa na grade (`th.selcol` e `td.selcol`).
    pub destacada: Option<&'a str>,
    // ---- saídas
    pub ordenar: Option<usize>,
    pub abrir: Option<usize>,
    pub marcar: Option<(usize, bool)>,
    pub marcar_pagina: Option<bool>,
    pub fim_edicao: Option<FimEdicao>,
    pub edicao_desenhada: bool,
    pub origem: Option<Pos2>,
    pub visivel: Option<Rect>,
}

impl Pintor<'_> {
    fn idx_abrir(&self) -> Option<usize> {
        self.tem_abrir.then_some(usize::from(self.tem_marca))
    }

    fn pendente(&self, linha: usize, coluna: usize) -> Option<&str> {
        self.pendentes
            .iter()
            .rev()
            .find(|g| g.geracao == self.dados.geracao && g.linha == linha && g.coluna == coluna)
            .map(|g| g.texto.as_str())
    }

    /// `.grade tr.mar td{background:var(--sel)}`: o fundo da linha marcada vai em cada célula, sem o fio de baixo
    /// (no navegador o fio da tabela fica fora do fundo da célula e continua claro entre duas linhas marcadas);
    /// cobre também o hover da linha (`--selh`), como o fundo do `td` cobre o do `tr`.
    /// A linha `l` está marcada?
    fn linha_marcada(&self, l: usize) -> bool {
        self.dados.chaves.get(l).and_then(|t| t.as_deref()).is_some_and(|t| self.marcadas.contem(t))
    }

    fn fundo_marcada(&self, ui: &Ui, c: &CelulaVisivel) {
        if !self.linha_marcada(c.linha) {
            return;
        }
        let mut r = c.celula;
        if c.linha + 1 == self.dados.n_linhas() {
            // a última linha não tem fio embaixo
            r.max.y += 1.0;
        }
        if c.coluna + 1 == self.larguras.len() {
            // colunas estreitadas à mão podem não encher a grade: o fundo vai até a borda, como o do hover
            r.max.x = r.max.x.max(ui.clip_rect().right());
        }
        let mut pintor = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
        if self.ocupada {
            pintor.multiply_opacity(OPACIDADE_OCUPADA);
        }
        pintor.rect_filled(r, 0.0, self.p.estudio.sel);
    }

    fn piscada(&self, linha: usize, coluna: usize) -> Option<f64> {
        self.piscadas
            .iter()
            .rev()
            .find(|g| g.geracao == self.dados.geracao && g.linha == linha && g.coluna == coluna)
            .map(|g| g.desde)
            .filter(|d| self.agora - d < DURACAO_SALVO)
    }

    /// Um filho do `ui` da grade no retângulo, com a opacidade da página a caminho.
    fn filho(&self, ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, rect: Rect, layout: Layout) -> Ui {
        let mut filho = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect).layout(layout));
        if self.ocupada {
            filho.multiply_opacity(OPACIDADE_OCUPADA);
        }
        filho
    }

    fn caixa_de_marcar(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        let l = c.linha;
        let Some(texto) = self.dados.chaves.get(l) else { return };
        let marcada = texto.as_deref().is_some_and(|t| self.marcadas.contem(t));
        // `td.selc{text-align:center}`: a caixa no meio da célula
        let caixa = Rect::from_center_size(c.celula.center(), vec2(LADO_CAIXA, LADO_CAIXA));
        let mut filho = self.filho(ui, ("tela_dados::marca", l), caixa, Layout::left_to_right(Align::Center));
        // dentro do recorte de quem desenha a grade também (o documento que rola por dentro na casca)
        filho.set_clip_rect(c.recorte.intersect(ui.clip_rect()));
        // linha sem chave (PK nula) não tem como ser marcada: a caixa fica desabilitada, sem sumir
        if texto.is_none() {
            filho.disable();
        }
        let mut m = marcada;
        let r = Check::new(&mut m, "").mostrar(&mut filho);
        if r.changed() {
            self.marcar = Some((l, m));
        }
        // o rótulo acessível (montado só quando a acessibilidade pede)
        let habilitada = texto.is_some();
        let rotulo = self.dados.rotulo(l);
        r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, habilitada, m, format!("Marcar a linha {rotulo}")));
    }

    fn abridor(&mut self, ui: &mut Ui, c: &CelulaVisivel, direita: f32) {
        let l = c.linha;
        let rect = Rect::from_min_max(pos2(c.celula.left(), c.celula.top()), pos2(direita, c.celula.bottom()));
        let mut filho = self.filho(ui, ("tela_dados::abre", l, c.coluna), rect, Layout::right_to_left(Align::Center));
        filho.set_clip_rect(c.recorte.intersect(ui.clip_rect()));
        let r = nome_abre_tamanho(&mut filho, self.dados.rotulo(l), false, 12.0);
        if r.clicked() {
            self.abrir = Some(l);
        }
        if r.hovered() {
            r.on_hover_text(self.titulo_abre);
        }
    }

    fn campo_de_edicao(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        let (ponta_ida, ponta_volta) = (self.ponta_ida, self.ponta_volta);
        let p = self.p;
        let Some(ed) = self.edicao.as_deref_mut() else { return };
        self.edicao_desenhada = true;
        let id = ed.id();
        if ed.pedir_foco {
            // o foco antes de desenhar o campo: sem ele, o `TextEdit` desfaz a seleção do texto todo
            ed.pedir_foco = false;
            ui.memory_mut(|m| m.request_focus(id));
        }
        let tinha_foco = ed.focada || ui.memory(|m| m.has_focus(id));
        let mut fim = None;
        let mut sair_voltando = None;
        if tinha_foco {
            // as teclas da edição saem dos eventos antes de o campo vê-las (o `keydown` com `stopPropagation`)
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                        if fim.is_none() && !modifiers.ctrl && !modifiers.alt =>
                    {
                        fim = Some(FimEdicao::Salvar { foco: true });
                        false
                    }
                    Event::Key { key: Key::Escape, pressed: true, .. } if fim.is_none() => {
                        fim = Some(FimEdicao::Cancelar);
                        false
                    }
                    Event::Key { key: Key::Tab, pressed: true, modifiers, .. }
                        if fim.is_none() && !modifiers.ctrl && !modifiers.alt =>
                    {
                        let voltando = modifiers.shift;
                        if (voltando && ponta_volta) || (!voltando && ponta_ida) {
                            // na ponta da grade o Tab segue a página; a célula grava quando o foco sai
                            fim = Some(FimEdicao::Salvar { foco: false });
                            sair_voltando = Some(voltando);
                            true
                        } else {
                            fim = Some(FimEdicao::Tab { voltando });
                            false
                        }
                    }
                    _ => true,
                })
            });
            if let Some(voltando) = sair_voltando {
                let direcao = if voltando { FocusDirection::Previous } else { FocusDirection::Next };
                ui.memory_mut(|m| m.move_focus(direcao));
            }
        }
        let largura = (c.celula.width() - 2.0 * PADDING_OBS_X).max(LARGURA_MIN_OBS);
        // o campo de 26px no meio da célula (sem passar para a linha de baixo nem cobrir o fio)
        let rect = Rect::from_min_size(
            pos2(c.celula.left() + PADDING_OBS_X, c.celula.center().y - ALTURA_CAMPO_PEQUENO / 2.0),
            vec2(largura, ALTURA_CAMPO_PEQUENO),
        );
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(("tela_dados::obs", ed.linha, ed.coluna))
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        // o texto começa todo selecionado: legível sobre o fundo da seleção
        texto_selecionado_legivel(&mut filho);
        // o `td.obs` tem `overflow:visible`: o campo pode passar da célula (mas não da grade)
        let salt = ed.salt();
        let saida = Entrada::new(&mut ed.texto)
            .compacta()
            .cor(p.fg)
            .alinhar_direita(ed.numerica)
            .largura(largura)
            .id_salt(salt)
            .rotulo(&ed.rotulo)
            .mostrar(&mut filho);
        let resp = saida.response.response;
        if resp.has_focus() {
            ed.focada = true;
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true },
                )
            });
        } else if ed.focada && fim.is_none() {
            // perdeu o foco (clique em outro lugar): grava
            fim = Some(FimEdicao::Salvar { foco: false });
        }
        if fim.is_some() {
            self.fim_edicao = fim;
        }
    }
}

/// O texto selecionado de um campo na cor do texto (`--t`), sobre o fundo da seleção (`--sel`), como a seleção do
/// sistema no navegador. O tema do `interface` já faz isso; a regra fica aqui para um estilo que pinte o texto
/// selecionado de outra cor. Vale para o `ui` dado e os filhos dele.
pub(crate) fn texto_selecionado_legivel(ui: &mut Ui) {
    let fg = interface::tema::paleta(ui).fg;
    if ui.visuals().selection.stroke.color != fg {
        ui.visuals_mut().selection.stroke.color = fg;
    }
}

/// Começa a edição: o foco no campo e o texto todo selecionado (ou o cursor no fim, quando a edição começou com
/// uma tecla).
pub(crate) fn preparar_edicao(ctx: &egui::Context, ed: &Edicao, selecionar_tudo: bool) {
    let id = ed.id();
    let n = ed.texto.chars().count();
    let mut estado = TextEditState::load(ctx, id).unwrap_or_default();
    let faixa = if selecionar_tudo {
        CCursorRange::two(CCursor::new(0), CCursor::new(n))
    } else {
        CCursorRange::one(CCursor::new(n))
    };
    estado.cursor.set_char_range(Some(faixa));
    estado.store(ctx, id);
}

impl DesenhoGrade for Pintor<'_> {
    /// A coluna escolhida nas Propriedades acende (o fundo `--sel` no `th` e o `--tint` nas células).
    fn th_selecionada(&mut self, coluna: usize) -> bool {
        let Some(nome) = self.destacada else { return false };
        coluna.checked_sub(self.primeira).and_then(|dc| self.dados.colunas.get(dc)).is_some_and(|c| c.nome == nome)
    }

    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        if self.origem.is_none() {
            // o canto do conteúdo, desfeita a rolagem: a primeira célula visível diz onde ele está
            let x0: f32 = self.larguras.iter().take(c.coluna).sum();
            self.origem = Some(pos2(
                c.celula.left() - x0,
                c.celula.top() - self.altura_cabecalho - c.linha as f32 * ALTURA_LINHA,
            ));
            self.visivel = Some(ui.clip_rect());
        }
        self.fundo_marcada(ui, c);
        if self.tem_marca && c.coluna == 0 {
            self.caixa_de_marcar(ui, c);
            return;
        }
        if self.idx_abrir() == Some(c.coluna) {
            self.abridor(ui, c, c.celula.right() - PADDING_ABRIR);
            return;
        }
        let Some(dc) = c.coluna.checked_sub(self.primeira) else { return };
        let l = c.linha;
        let Some(col) = self.dados.colunas.get(dc) else { return };
        if let Some(desde) = self.piscada(l, dc) {
            let pintor = ui.painter().with_clip_rect(c.recorte);
            pintar_salvo(&pintor, self.p, c.celula, 0.0, desde);
        }
        let editando =
            self.edicao.as_deref().is_some_and(|e| e.geracao == self.dados.geracao && e.linha == l && e.coluna == dc);
        if editando {
            self.campo_de_edicao(ui, c);
            return;
        }
        if let Some(texto) = self.pendente(l, dc) {
            let mut pintor = ui.painter().with_clip_rect(c.recorte);
            pintor.multiply_opacity(0.7);
            let cel = Celula::Texto(texto.to_owned());
            pintar_celula(&pintor, &paleta_da_linha(self.p, self.linha_marcada(l)), c.conteudo, &cel, col.numerica);
            return;
        }
        if col.abre {
            self.abridor(ui, c, c.conteudo.right());
            return;
        }
        let Some(valor) = self.dados.celula(l, dc) else { return };
        let mut pintor = ui.painter().with_clip_rect(c.recorte);
        if self.ocupada {
            pintor.multiply_opacity(OPACIDADE_OCUPADA);
        }
        let mut p = paleta_da_linha(self.p, self.linha_marcada(l));
        if col.gerada {
            // `td.gerada`: o texto em `--k-muted`
            p.fg = p.muted;
        }
        pintar_celula(&pintor, &p, c.conteudo, valor, col.numerica);
    }

    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        let pintor = ui.painter().with_clip_rect(h.recorte);
        if self.tem_marca && h.coluna == 0 {
            if !self.editavel || self.dados.n_linhas() == 0 {
                return;
            }
            // a caixa no meio do `th.selc` de 30px (sem o fio de baixo)
            let caixa = Rect::from_center_size(h.conteudo.center(), vec2(LADO_CAIXA, LADO_CAIXA));
            let mut filho = ui.new_child(
                UiBuilder::new()
                    .id_salt("tela_dados::marca_pagina")
                    .max_rect(caixa)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            filho.set_clip_rect(h.recorte.intersect(ui.clip_rect()));
            let mut m = self.todas_marcadas;
            let parte = self.algumas_marcadas && !self.todas_marcadas;
            let r = Check::new(&mut m, "").indeterminado(parte).mostrar(&mut filho);
            if r.changed() {
                self.marcar_pagina = Some(m);
            }
            let marcada = self.todas_marcadas;
            // parte da página marcada: estado misto (`selected: None` numa caixa vira `Toggled::Mixed`)
            r.widget_info(|| WidgetInfo {
                selected: (!parte).then_some(marcada),
                ..WidgetInfo::labeled(WidgetType::Checkbox, true, "Marcar todas as linhas desta página")
            });
            if r.hovered() {
                r.on_hover_text("Marcar todas as linhas desta página");
            }
            return;
        }
        if self.idx_abrir() == Some(h.coluna) {
            if let Some(alias) = self.cabecalho_abrir {
                let conteudo =
                    Rect::from_min_max(h.conteudo.min, pos2(h.th.right() - PADDING_ABRIR, h.conteudo.bottom()));
                let th = Th { numerica: true, ..Th::new(alias) };
                pintar_cabecalho_coluna(&pintor, self.p, conteudo, &th);
            }
            return;
        }
        let Some(dc) = h.coluna.checked_sub(self.primeira) else { return };
        let Some(col) = self.dados.colunas.get(dc) else { return };
        let ordenada = self.ordem.filter(|(o, _)| *o == col.nome).map(|(_, d)| d);
        let seta = ordenada.map(|d| if d == Direcao::Desc { Ordem::Decrescente } else { Ordem::Crescente });
        // a pastilha de 16 (o tipo por extenso na dica dela) e o `button.th-abre`: o nome (e a seta), na altura toda
        // do `th`; numa coluna numérica os dois vão juntos à direita, como o `pintar_cabecalho_coluna` os desenha
        let largura_seta = if seta.is_some() { ESPACO_SETA_ORDEM + LADO_SETA_ORDEM } else { 0.0 };
        let largura_nome = largura_cabecalho(ui.painter(), &col.nome, None) + largura_seta;
        let x0 = if col.numerica {
            (h.conteudo.right() - LARGURA_PASTILHA_TH - largura_nome).max(h.conteudo.left())
        } else {
            h.conteudo.left()
        };
        let x_nome = x0 + LARGURA_PASTILHA_TH;
        let fim_nome = (x_nome + largura_nome).min(h.conteudo.right());
        let alvo = Rect::from_min_max(pos2(x_nome, h.conteudo.top()), pos2(fim_nome, h.conteudo.bottom()))
            .intersect(h.recorte);
        let r = ui.interact(alvo, ui.id().with(("tela_dados::th", dc)), Sense::click());
        let th = Th {
            numerica: col.numerica,
            ordem: seta,
            clicavel: true,
            realce: r.hovered(),
            pastilha: Some(col.pastilha),
            selecionada: self.destacada == Some(col.nome.as_str()),
            ..Th::new(&col.nome)
        };
        if let Some(rect_pastilha) = pintar_cabecalho_coluna(&pintor, self.p, h.conteudo, &th) {
            let rp =
                ui.interact(rect_pastilha.intersect(h.recorte), ui.id().with(("tela_dados::tipo", dc)), Sense::hover());
            let tipo = col.rotulo_tipo.as_str();
            rp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, tipo));
            if rp.hovered() {
                rp.on_hover_text(tipo);
            }
        }
        if r.clicked() {
            self.ordenar = Some(dc);
        }
        let nome = col.nome.as_str();
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, nome));
        if r.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            let proxima = match ordenada {
                None => "crescente",
                Some(Direcao::Asc) => "decrescente",
                Some(Direcao::Desc) => "sem ordem",
            };
            r.on_hover_text(format!("Ordenar por {nome} ({proxima})"));
        }
    }

    fn dica(&mut self, linha: usize, coluna: usize) -> Option<String> {
        let dc = coluna.checked_sub(self.primeira)?;
        let col = self.dados.colunas.get(dc)?;
        if col.abre {
            return None;
        }
        match self.dados.celula(linha, dc)? {
            // números não têm `title` na aba Dados (só texto, o "vazio", o "…" e o chip)
            Celula::Inteiro(_) | Celula::Real(_) | Celula::Nulo => None,
            outra => dica_celula(outra),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use interface::tema::{CLARA, ESCURA};

    #[test]
    fn linha_marcada_tira_o_terciario_do_null_e_do_numero() {
        for p in [&CLARA, &ESCURA] {
            let m = paleta_da_linha(p, true);
            assert_eq!((m.faint, m.estudio.t3), (p.estudio.t2, p.estudio.t2));
            // o texto comum da linha fica como estava
            assert_eq!((m.fg, m.muted), (p.fg, p.muted));
            assert_eq!(paleta_da_linha(p, false), *p);
        }
    }
}
