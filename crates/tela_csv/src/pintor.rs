//! O [`Pintor`]: o [`DesenhoGrade`] de um quadro da grade do CSV. Lê o [`Retrato`] e a [`Pagina`] (cópias; a grade
//! nunca espera a tabela) e devolve, em campos de saída, o que a pessoa fez (marcar, terminar a edição, o "+") e que
//! linhas ficaram à vista (a tela pede a página que falta).

use std::collections::BTreeSet;

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{
    Align, Color32, Event, EventFilter, FocusDirection, Id, Key, Layout, Pos2, Rect, Response, Sense, Ui, UiBuilder,
    WidgetInfo, WidgetType, pos2, vec2,
};
use interface::grade::{
    ALTURA_LINHA, CabecalhoVisivel, CelulaVisivel, DesenhoGrade, Ordem, Th, pintar_cabecalho_coluna,
    pintar_celula_alterada, pintar_numero_linha, pintar_texto_de_celula, rect_seta_ordem,
};
use interface::icones::Icone;
use interface::pecas::botao::{Botao, LADO_PEQUENO};
use interface::pecas::campos::{ALTURA_CAMPO_PEQUENO, Check, Entrada};
use interface::tema::Paleta;

use crate::colunas::{AlvoCabecalho, CabecalhoEmEdicao, ColunaDaGrade, FimCabecalho, id_do_th};
use crate::grade::{COLUNA_MARCAR, COLUNA_NUMERO, PRIMEIRA_DE_DADOS};
use crate::sessao::{Pagina, Retrato};
use crate::textos;

/// `padding-right` da coluna do número da linha (`td.lin{padding:0 8px}`).
const PADDING_NUMERO: f32 = 8.0;
/// A caixa de marcar (`.check` de 16px) no meio do `td.selc`.
const LADO_CAIXA: f32 = 16.0;
/// O `td.obs` da edição no lugar: 6px dos lados, o campo compacto de 26px no meio da célula, 140px no mínimo.
const PADDING_OBS_X: f32 = 6.0;
const LARGURA_MIN_OBS: f32 = 140.0;
/// A célula gravando: o texto que a pessoa digitou fica nela, um pouco apagado, até a resposta.
const OPACIDADE_PENDENTE: f32 = 0.7;
/// O `.th-edit` do cabeçalho em edição: `height:22px; padding:0 6px; border-radius:4px`, a 4px das bordas do `th`.
const ALTURA_TH_EDIT: f32 = 22.0;
const PADDING_TH_EDIT: f32 = 6.0;
const RAIO_TH_EDIT: f32 = 4.0;
const RECUO_TH_EDIT: f32 = 4.0;
/// O alvo do clique em volta da seta da ordem no `th`.
const LADO_SETA: f32 = 16.0;

/// Uma edição no lugar (linha da vista, coluna de dados), com o texto do campo.
#[derive(Debug, Clone)]
pub(crate) struct Edicao {
    pub linha: usize,
    pub coluna: usize,
    /// A posição da linha na tabela (o que a alteração grava).
    pub posicao: u64,
    pub texto: String,
    pub original: String,
    pub numerica: bool,
    pub rotulo: String,
    /// O campo já apareceu com o foco (só então perder o foco grava).
    pub focada: bool,
    pub pedir_foco: bool,
}

impl Edicao {
    pub(crate) fn salt(&self) -> (&'static str, usize, usize) {
        ("tela_csv::edicao", self.linha, self.coluna)
    }

    pub(crate) fn id(&self) -> Id {
        Id::new(self.salt())
    }
}

/// Como uma edição no lugar terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FimEdicao {
    /// Enter (`foco`: a grade volta a ter o foco) ou o campo perdeu o foco.
    Salvar { foco: bool },
    /// Esc.
    Cancelar,
    /// Tab / Shift+Tab: grava e vai para a célula seguinte (ou anterior).
    Tab { voltando: bool },
}

/// Uma célula gravando (o texto que a pessoa digitou, até a resposta da tarefa `seq`).
#[derive(Debug, Clone)]
pub(crate) struct Pendente {
    pub seq: u64,
    pub linha: usize,
    pub coluna: usize,
    pub texto: String,
}

/// Começa a edição: o foco no campo e o texto todo selecionado (ou o cursor no fim, quando ela começou com uma tecla).
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

/// O desenho de um quadro da grade.
pub(crate) struct Pintor<'a> {
    pub p: &'a Paleta,
    pub retrato: &'a Retrato,
    pub pagina: Option<&'a Pagina>,
    pub n_dados: usize,
    pub larguras: &'a [f32],
    pub altura_cabecalho: f32,
    /// As linhas marcadas (posições na tabela).
    pub marcadas: &'a BTreeSet<u64>,
    /// A caixa do cabeçalho: todas as linhas da vista marcadas, ou só algumas (o traço).
    pub todas_marcadas: bool,
    pub algumas_marcadas: bool,
    /// A grade aceita edição (não está gravando).
    pub editavel: bool,
    pub edicao: Option<&'a mut Edicao>,
    pub pendentes: &'a [Pendente],
    /// A célula ativa está na ponta da grade para o Tab (a última; com Shift, a primeira).
    pub ponta_ida: bool,
    pub ponta_volta: bool,
    /// As colunas da grade (com a provisória) e os valores da provisória (a coluna colada), por posição na tabela.
    pub mapa: &'a [ColunaDaGrade],
    pub provisoria: Option<&'a [String]>,
    /// As colunas selecionadas (da tabela).
    pub selecionadas: &'a BTreeSet<usize>,
    pub cabecalho_edicao: Option<&'a mut CabecalhoEmEdicao>,
    /// O id do documento (os ids dos `th`).
    pub id_doc: &'a str,
    /// A ordem de agora (a coluna da tabela e se é decrescente): a seta em `--acct` no `th`.
    pub ordem: Option<(usize, bool)>,
    // ---- saídas
    /// A seta da ordem de uma coluna (da tabela) foi clicada.
    pub ordenar: Option<usize>,
    /// O clique no `th` de uma coluna da tabela, com Ctrl e Shift.
    pub clique_th: Option<(usize, bool, bool)>,
    /// O clique direito no `th` de uma coluna da tabela (o menu).
    pub menu_th: Option<usize>,
    /// As respostas dos `th` à vista (o dono do menu).
    pub ths: Vec<(usize, Response)>,
    pub fim_cabecalho: Option<FimCabecalho>,
    pub cabecalho_desenhado: bool,
    /// As linhas da vista que apareceram (a primeira e a última), para pedir a página que falta.
    pub visiveis: Option<(u64, u64)>,
    /// O "+" do fim do cabeçalho foi clicado (Nova coluna no fim).
    pub nova_no_fim: bool,
    /// A caixa de uma linha (da vista) mudou.
    pub marcar: Option<(usize, bool)>,
    /// A caixa do cabeçalho mudou.
    pub marcar_todas: Option<bool>,
    pub fim_edicao: Option<FimEdicao>,
    pub edicao_desenhada: bool,
    pub origem: Option<Pos2>,
    pub visivel: Option<Rect>,
}

impl Pintor<'_> {
    /// A coluna da grade que está na coluna `c` do núcleo.
    fn dado(&self, c: usize) -> Option<ColunaDaGrade> {
        c.checked_sub(PRIMEIRA_DE_DADOS).and_then(|g| self.mapa.get(g)).copied()
    }

    fn numerica(&self, coluna: ColunaDaGrade) -> bool {
        match coluna {
            ColunaDaGrade::Dados(d) => self.retrato.tipos.get(d).copied().is_some_and(textos::numerica),
            ColunaDaGrade::Provisoria => false,
        }
    }

    /// O cabeçalho desta coluna está em edição (a provisória, ou a coluna que se renomeia)?
    fn cabecalho_em_edicao(&self, coluna: ColunaDaGrade) -> bool {
        match (coluna, self.cabecalho_edicao.as_deref().map(|e| &e.alvo)) {
            (ColunaDaGrade::Provisoria, Some(_)) => true,
            (ColunaDaGrade::Dados(c), Some(AlvoCabecalho::Renomear { coluna })) => c == *coluna,
            _ => false,
        }
    }

    /// O campo do cabeçalho em edição (o `.th-edit` da prancheta 16: 22px, raio 4, o fio e o brilho do destaque, o
    /// nome em 600; Enter confirma, Esc é do alvo, perder o foco confirma).
    fn campo_do_cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        let p = self.p;
        let Some(ed) = self.cabecalho_edicao.as_deref_mut() else { return };
        self.cabecalho_desenhado = true;
        let id = ed.id();
        // até o campo ganhar o foco (o botão que abriu a edição pode segurá-lo um quadro)
        if ed.pedir_foco || !ed.focada {
            ed.pedir_foco = false;
            ui.memory_mut(|m| m.request_focus(id));
        }
        let mut fim = None;
        if ed.focada || ui.memory(|m| m.has_focus(id)) {
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Key { key: Key::Enter, pressed: true, .. } if fim.is_none() => {
                        fim = Some(FimCabecalho::Confirmar);
                        false
                    }
                    Event::Key { key: Key::Escape, pressed: true, .. } if fim.is_none() => {
                        fim = Some(FimCabecalho::Esc);
                        false
                    }
                    _ => true,
                })
            });
        }
        let caixa = Rect::from_min_max(
            pos2(h.th.left() + RECUO_TH_EDIT, h.conteudo.center().y - ALTURA_TH_EDIT / 2.0),
            pos2(h.th.right() - 1.0 - RECUO_TH_EDIT, h.conteudo.center().y + ALTURA_TH_EDIT / 2.0),
        );
        let pintor = ui.painter().with_clip_rect(h.recorte.intersect(ui.clip_rect()));
        interface::pintura::halo(&pintor, caixa, RAIO_TH_EDIT, 10.0, p.estudio.brilho);
        pintor.rect_filled(caixa, RAIO_TH_EDIT, p.estudio.f);
        pintor.rect_stroke(caixa, RAIO_TH_EDIT, egui::Stroke::new(2.0, p.estudio.acc), egui::StrokeKind::Inside);
        let dentro = caixa.shrink2(vec2(PADDING_TH_EDIT, 0.0));
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt("tela_csv::th_edicao_ui")
                .max_rect(dentro)
                .layout(Layout::left_to_right(Align::Center)),
        );
        filho.set_clip_rect(caixa.intersect(ui.clip_rect()));
        // `.th-edit .sel-txt`: a seleção no destaque a 55% com o texto branco
        filho.visuals_mut().selection.bg_fill = p.estudio.acc.gamma_multiply(0.55);
        filho.visuals_mut().selection.stroke.color = Color32::WHITE;
        let resp = filho.add(
            egui::TextEdit::singleline(&mut ed.nome)
                .id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .font(interface::fontes::fonte(11.5, interface::fontes::Peso::Medio))
                .text_color(p.estudio.t)
                .desired_width(dentro.width()),
        );
        let rotulo = "Nome da coluna".to_string();
        resp.widget_info(|| WidgetInfo { label: Some(rotulo.clone()), ..WidgetInfo::text_edit(true, "", "", "") });
        if resp.has_focus() {
            ed.focada = true;
            ui.memory_mut(|m| m.set_focus_lock_filter(id, EventFilter { escape: true, ..EventFilter::default() }));
        } else if ed.focada && fim.is_none() {
            fim = Some(FimCabecalho::Confirmar);
        }
        if fim.is_some() {
            self.fim_cabecalho = fim;
        }
    }

    fn notar_visivel(&mut self, linha: u64) {
        self.visiveis = Some(match self.visiveis {
            None => (linha, linha),
            Some((a, b)) => (a.min(linha), b.max(linha)),
        });
    }

    /// A linha (da vista) está marcada?
    fn marcada(&self, l: usize) -> bool {
        self.pagina.and_then(|p| p.posicao(l as u64)).is_some_and(|pos| self.marcadas.contains(&pos))
    }

    fn pendente(&self, linha: usize, coluna: usize) -> Option<&str> {
        self.pendentes.iter().rev().find(|g| g.linha == linha && g.coluna == coluna).map(|g| g.texto.as_str())
    }

    fn caixa_de_marcar(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        let l = c.linha;
        let Some(posicao) = self.pagina.and_then(|p| p.posicao(l as u64)) else { return };
        let caixa = Rect::from_center_size(c.celula.center(), vec2(LADO_CAIXA, LADO_CAIXA));
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(("tela_csv::marca", l))
                .max_rect(caixa)
                .layout(Layout::left_to_right(Align::Center)),
        );
        filho.set_clip_rect(c.recorte.intersect(ui.clip_rect()));
        let mut m = self.marcadas.contains(&posicao);
        let r = Check::new(&mut m, "").mostrar(&mut filho);
        if r.changed() {
            self.marcar = Some((l, m));
        }
        let numero = posicao + 1;
        r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, m, format!("Marcar a linha {numero}")));
    }

    fn campo_de_edicao(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        let (ponta_ida, ponta_volta) = (self.ponta_ida, self.ponta_volta);
        let fg = self.p.fg;
        let Some(ed) = self.edicao.as_deref_mut() else { return };
        self.edicao_desenhada = true;
        let id = ed.id();
        if ed.pedir_foco {
            ed.pedir_foco = false;
            ui.memory_mut(|m| m.request_focus(id));
        }
        let tinha_foco = ed.focada || ui.memory(|m| m.has_focus(id));
        let mut fim = None;
        let mut sair_voltando = None;
        if tinha_foco {
            // as teclas da edição saem dos eventos antes de o campo vê-las
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
        let rect = Rect::from_min_size(
            pos2(c.celula.left() + PADDING_OBS_X, c.celula.center().y - ALTURA_CAMPO_PEQUENO / 2.0),
            vec2(largura, ALTURA_CAMPO_PEQUENO),
        );
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(("tela_csv::obs", ed.linha, ed.coluna))
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        if filho.visuals().selection.stroke.color != fg {
            filho.visuals_mut().selection.stroke.color = fg;
        }
        let salt = ed.salt();
        let saida = Entrada::new(&mut ed.texto)
            .compacta()
            .cor(fg)
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

    /// A seta da ordem no `th` (o clique ordena: crescente → decrescente → sem ordem): a `.seta-ord` de 9px logo
    /// depois do nome, dos dois lados do alinhamento (U7 da revisão de UI de 22/10/2026; até a 0.2.3, na outra ponta
    /// do `th`), em `--acct` com a ordem e, sem ordem, em `--t3` com o ponteiro no `th` (`--t` em cima dela). Devolve a
    /// ordem e a cor da seta a desenhar, ou `None` sem ordem e sem o ponteiro.
    fn seta_da_ordem(&self, ui: &Ui, h: &CabecalhoVisivel, d: usize) -> Option<(Ordem, Option<Color32>)> {
        let ordenada = self.ordem.filter(|(c, _)| *c == d).map(|(_, desc)| desc);
        let sobre = ui.rect_contains_pointer(h.recorte);
        let ordem = match ordenada {
            Some(true) => Ordem::Decrescente,
            Some(false) => Ordem::Crescente,
            None if sobre => Ordem::Crescente,
            None => return None,
        };
        let cor = ordenada.is_none().then_some(self.p.estudio.t3);
        Some((ordem, cor))
    }

    /// O alvo do clique da seta (16px em volta dela) e a dica: "Ordenar por regiao (decrescente)". Devolve se o
    /// ponteiro está em cima dele.
    fn alvo_da_seta(&mut self, ui: &mut Ui, h: &CabecalhoVisivel, d: usize, nome: &str, seta: Rect) -> bool {
        let r = Rect::from_center_size(seta.center(), vec2(LADO_SETA, LADO_SETA));
        let resp = ui.interact(r.intersect(h.recorte), Id::new(("tela_csv::seta", self.id_doc, d)), Sense::click());
        let proxima = match self.ordem.filter(|(c, _)| *c == d).map(|(_, desc)| desc) {
            None => "crescente",
            Some(false) => "decrescente",
            Some(true) => "sem ordem",
        };
        let rotulo = format!("Ordenar por {nome}");
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &rotulo));
        if resp.clicked() {
            self.ordenar = Some(d);
        }
        // a seta fica no meio do `th`, junto do nome: o clique direito nela é o do `th` (o menu da coluna, com ela
        // como dono neste quadro)
        if resp.secondary_clicked() {
            self.menu_th = Some(d);
            if let Some(th) = self.ths.iter_mut().find(|(c, _)| *c == d) {
                th.1 = resp.clone();
            }
        }
        let em_cima = resp.hovered();
        if em_cima {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            resp.on_hover_text(format!("Ordenar por {nome} ({proxima})"));
        }
        em_cima
    }

    /// A paleta das células de uma linha marcada: o terciário do número passa a `--t2` (contraste sobre `--sel`).
    fn paleta_da_linha(&self, marcada: bool) -> Paleta {
        let mut p = *self.p;
        if marcada {
            p.estudio.t3 = p.estudio.t2;
        }
        p
    }
}

impl DesenhoGrade for Pintor<'_> {
    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        if self.origem.is_none() {
            let x0: f32 = self.larguras.iter().take(c.coluna).sum();
            self.origem = Some(pos2(
                c.celula.left() - x0,
                c.celula.top() - self.altura_cabecalho - c.linha as f32 * ALTURA_LINHA,
            ));
            self.visivel = Some(ui.clip_rect());
        }
        let l = c.linha as u64;
        self.notar_visivel(l);
        if c.coluna == COLUNA_MARCAR {
            self.caixa_de_marcar(ui, c);
            return;
        }
        let pintor = ui.painter().with_clip_rect(c.recorte);
        let marcada = self.marcada(c.linha);
        if c.coluna == COLUNA_NUMERO {
            // o número da linha é a posição na tabela (com filtro ou ordem, a da linha na vista não serve)
            if let Some(n) = self.pagina.and_then(|p| p.posicao(l)) {
                let conteudo =
                    Rect::from_min_max(c.celula.min, pos2(c.celula.right() - 1.0 - PADDING_NUMERO, c.celula.bottom()));
                let p = self.paleta_da_linha(marcada);
                pintar_numero_linha(&pintor, &p, conteudo, &interface::formato::formatar_numero(n + 1));
            }
            return;
        }
        let Some(coluna) = self.dado(c.coluna) else { return };
        let Some(pagina) = self.pagina else { return };
        let ColunaDaGrade::Dados(d) = coluna else {
            // a provisória: os valores colados (por posição na tabela) ou nada (a Nova coluna)
            let valor = pagina.posicao(l).and_then(|p| self.provisoria?.get(usize::try_from(p).ok()?));
            if let Some(texto) = valor {
                pintar_texto_de_celula(&pintor, self.p, c.conteudo, texto, false);
            }
            return;
        };
        let editando = self.edicao.as_deref().is_some_and(|e| e.linha == c.linha && e.coluna == d);
        if editando {
            self.campo_de_edicao(ui, c);
            return;
        }
        if let Some(texto) = self.pendente(c.linha, d) {
            let mut fraco = pintor.clone();
            fraco.multiply_opacity(OPACIDADE_PENDENTE);
            pintar_celula_alterada(&fraco, self.p, c.celula);
            pintar_texto_de_celula(&fraco, self.p, c.conteudo, texto, self.numerica(coluna));
            return;
        }
        if pagina.alterada(l, d) {
            pintar_celula_alterada(&pintor, self.p, c.celula);
        }
        if let Some(texto) = pagina.celula(l, d) {
            pintar_texto_de_celula(&pintor, self.p, c.conteudo, texto, self.numerica(coluna));
        }
    }

    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        let pintor = ui.painter().with_clip_rect(h.recorte);
        if h.coluna == COLUNA_MARCAR {
            if self.retrato.linhas == 0 {
                return;
            }
            let caixa = Rect::from_center_size(h.conteudo.center(), vec2(LADO_CAIXA, LADO_CAIXA));
            let mut filho = ui.new_child(
                UiBuilder::new()
                    .id_salt("tela_csv::marca_todas")
                    .max_rect(caixa)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            filho.set_clip_rect(h.recorte.intersect(ui.clip_rect()));
            let mut m = self.todas_marcadas;
            let parte = self.algumas_marcadas && !self.todas_marcadas;
            let r = Check::new(&mut m, "").indeterminado(parte).mostrar(&mut filho);
            if r.changed() {
                self.marcar_todas = Some(m);
            }
            let marcada = self.todas_marcadas;
            r.widget_info(|| WidgetInfo {
                selected: (!parte).then_some(marcada),
                ..WidgetInfo::labeled(WidgetType::Checkbox, true, "Marcar todas as linhas")
            });
            if r.hovered() {
                r.on_hover_text("Marcar todas as linhas");
            }
            return;
        }
        if h.coluna == COLUNA_NUMERO {
            let conteudo = Rect::from_min_max(h.th.min, pos2(h.th.right() - 1.0 - PADDING_NUMERO, h.conteudo.bottom()));
            let th = Th { numerica: true, ..Th::new("#") };
            let mut p = *self.p;
            p.estudio.t2 = p.estudio.t3;
            pintar_cabecalho_coluna(&pintor, &p, conteudo, &th);
            return;
        }
        if h.coluna == PRIMEIRA_DE_DADOS + self.n_dados {
            // `th.nova-col`: o `.ib.pq` "+" (Nova coluna no fim)
            let r = Rect::from_center_size(h.th.center() - vec2(0.5, 0.5), vec2(LADO_PEQUENO, LADO_PEQUENO));
            let mut filho = ui.new_child(
                UiBuilder::new()
                    .id_salt("tela_csv::nova_no_fim")
                    .max_rect(r)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            filho.set_clip_rect(h.recorte.intersect(ui.clip_rect()));
            let b = Botao::icone(Icone::Mais).pequeno().rotulo("Nova coluna no fim").dica("Nova coluna no fim");
            if filho.add(b.habilitado(self.editavel)).clicked() {
                self.nova_no_fim = true;
            }
            return;
        }
        let Some(coluna) = self.dado(h.coluna) else { return };
        if self.cabecalho_em_edicao(coluna) {
            self.campo_do_cabecalho(ui, h);
            return;
        }
        let ColunaDaGrade::Dados(d) = coluna else { return };
        // o `th` inteiro é a área do clique (seleciona; Ctrl e Shift somam) e do clique direito (o menu)
        let resp = ui.interact(h.recorte, id_do_th(self.id_doc, d), Sense::click());
        if resp.clicked() {
            let m = ui.input(|i| i.modifiers);
            self.clique_th = Some((d, m.ctrl || m.command, m.shift));
        }
        if resp.secondary_clicked() {
            self.menu_th = Some(d);
        }
        let nome = self.retrato.nomes.get(d).map_or("", String::as_str);
        let selecionada = self.selecionadas.contains(&d);
        let rotulo = nome.to_string();
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selecionada, &rotulo));
        self.ths.push((d, resp));
        let tipo = self.retrato.tipos.get(d).copied().unwrap_or(tabular::numero::TipoDetectado::Texto);
        let numerica = self.numerica(coluna);
        let seta = self.seta_da_ordem(ui, h, d);
        let mut th = Th {
            numerica,
            pastilha: Some(textos::pastilha(tipo)),
            selecionada,
            ordem: seta.map(|(o, _)| o),
            cor_seta: seta.and_then(|(_, c)| c),
            ..Th::new(nome)
        };
        // a seta logo depois do nome: o alvo antes de pintar, para a cor do hover já valer neste quadro
        if let Some(r) = rect_seta_ordem(&pintor, h.conteudo, &th)
            && self.alvo_da_seta(ui, h, d, nome, r)
            && th.cor_seta.is_some()
        {
            th.cor_seta = Some(self.p.estudio.t);
        }
        if let Some(rp) = pintar_cabecalho_coluna(&pintor, self.p, h.conteudo, &th) {
            let resp = ui.interact(rp.intersect(h.recorte), ui.id().with(("tela_csv::tipo", d)), Sense::hover());
            let dica = textos::dica_do_tipo(tipo);
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &dica));
            if resp.hovered() {
                resp.on_hover_text(dica);
            }
        }
    }

    fn fundo_linha(&mut self, linha: usize) -> Option<Color32> {
        self.marcada(linha).then_some(self.p.estudio.sel)
    }

    fn dica(&mut self, linha: usize, coluna: usize) -> Option<String> {
        let ColunaDaGrade::Dados(d) = self.dado(coluna)? else { return None };
        self.pagina?.celula(linha as u64, d).filter(|t| !t.is_empty()).map(str::to_string)
    }

    fn th_selecionada(&mut self, coluna: usize) -> bool {
        matches!(self.dado(coluna), Some(ColunaDaGrade::Dados(d)) if self.selecionadas.contains(&d))
    }
}
