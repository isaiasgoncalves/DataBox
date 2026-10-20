//! O Histórico de consultas (o `aside` "Histórico" de `Consulta.dc.html`), que a barra de ícones põe no lugar do
//! Explorador: os comandos já executados no banco aberto (`servico.historico`, o mais recente primeiro), inclusive
//! os que falharam, cada um com o resumo de uma linha da Consulta SQL (`resumo_comando`, em até duas linhas,
//! semilight) e, embaixo (`.hist .meta`, prancheta 3 da revisão de 19/10/2026), a pílula com o resultado ("9
//! linhas", "3 alteradas", "Parou no 3", "Erro") e o horário; na dica, o SQL inteiro, o erro e a data (item 20). N5 da prancheta 15: o clique seleciona (`.hist.on`); o duplo clique ou o Enter
//! abrem uma Consulta nova com aquele comando.
//!
//! A lista é lida numa tarefa quando o painel aparece e de novo quando ela fica velha à vista (a casca a marca a
//! cada resposta de uma Consulta, que é quando um comando entra no histórico). Escondido, o painel solta a leitura
//! que estava correndo. Vazio: o relógio de 48px, "Nenhuma consulta ainda." e, em `--t3`, "As consultas executadas
//! aparecem aqui."; sem banco: "Nenhum banco aberto.".

use std::sync::Arc;

use chrono::NaiveDate;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, CursorIcon, Galley, Key, Rect, Response, ScrollArea, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use estado::{Acao, Contexto, Promessa, Reacao};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_data, formatar_numero, plural};
use interface::icones::Icone;
use interface::pecas::arvore::RAIO_NO;
use interface::pecas::avisos::{aviso_texto, carregando};
use interface::pecas::botao::{pintor_de_fora, realce_curvo};
use interface::pecas::painel::{Encaixe, Painel};
use interface::pecas::rotulos::{TipoPill, pintar_pill, pintar_pill_contorno, tamanho_pill};
use interface::pecas::texto::formato;
use interface::pintura;
use interface::tema::{duracao_transicao, paleta, rgba};
use prefs::{ComandoSql, ResultadoSql};

use super::explorador::{vazio_centrado, vazio_centrado_com};
use crate::telas::resumo_comando;

/// `margin:2px 6px` e `padding:7px 10px` do `.hist`.
const MARGEM_X: f32 = 6.0;
const MARGEM_Y: f32 = 2.0;
const RECUO_X: f32 = 10.0;
const RECUO_Y: f32 = 7.0;

/// O Histórico vazio (N5).
pub(crate) const VAZIO: &str = "Nenhuma consulta ainda.";
pub(crate) const VAZIO_EXPLICACAO: &str = "As consultas executadas aparecem aqui.";

/// `.hist .meta`: `margin-top:4px`, `gap:6px`, 11px em `--t3`.
const ESPACO_META: f32 = 4.0;
const VAO_META: f32 = 6.0;
const TAMANHO_META: f32 = 11.0;

/// Um comando da lista, com os textos já prontos.
struct Item {
    sql: String,
    resumo: String,
    /// A pílula do resultado (None nos comandos guardados antes da 0.2.0, sem resultado).
    pilula: Option<(String, TipoPill)>,
    /// Ao lado da pílula: "3 comandos · 10:42", "ontem · 18:20" ("" sem data nem roteiro).
    meta: String,
    /// A dica: o SQL inteiro, o erro e a data.
    dica: String,
}

impl Item {
    /// `hoje`: a data local de agora (o horário de hoje sai só com a hora; o de ontem, com "ontem").
    fn de(c: ComandoSql, hoje: NaiveDate) -> Item {
        let pilula = c.resultado.as_ref().map(pilula);
        let quando = c.quando.as_deref().map(|q| quando_curto(q, hoje)).unwrap_or_default();
        let meta = match c.comandos.filter(|n| *n > 1) {
            Some(n) if quando.is_empty() => format!("{n} comandos"),
            Some(n) => format!("{n} comandos · {quando}"),
            None => quando,
        };
        let mut dica = c.sql.clone();
        match &c.resultado {
            Some(ResultadoSql::Erro { numero, mensagem }) if !mensagem.is_empty() => {
                let onde = if *numero > 1 { format!("Parou no comando {numero}") } else { "Erro".to_string() };
                dica.push_str(&format!("\n\n{onde}: {mensagem}"));
            }
            Some(ResultadoSql::Interrompida { numero }) if *numero > 1 => {
                dica.push_str(&format!("\n\nInterrompida no comando {numero}."));
            }
            _ => {}
        }
        if let Some(data) = c.quando.as_deref().map(formatar_data) {
            dica.push_str(&format!("\n\n{data}"));
        }
        Item { resumo: resumo_comando(&c.sql), pilula, meta, dica, sql: c.sql }
    }
}

/// O texto e a cor da pílula: "9 linhas" (ok), "3 alteradas", "Executado", "Parou no 3" e "Erro" (perigo),
/// "Interrompida" (espera).
fn pilula(r: &ResultadoSql) -> (String, TipoPill) {
    match r {
        ResultadoSql::Linhas { quantidade, mais: false } => (plural(*quantidade, "linha", "linhas"), TipoPill::Ok),
        ResultadoSql::Linhas { quantidade, mais: true } => {
            (format!("{}+ linhas", formatar_numero(*quantidade)), TipoPill::Ok)
        }
        ResultadoSql::Alteradas(n) => (plural(*n, "alterada", "alteradas"), TipoPill::Neutra),
        ResultadoSql::Executado => ("Executado".to_string(), TipoPill::Neutra),
        ResultadoSql::Erro { numero, .. } if *numero > 1 => (format!("Parou no {numero}"), TipoPill::No),
        ResultadoSql::Erro { .. } => ("Erro".to_string(), TipoPill::No),
        ResultadoSql::Interrompida { .. } => ("Interrompida".to_string(), TipoPill::Wait),
    }
}

/// O horário curto da meta: hoje, só a hora ("10:42"); ontem, "ontem · 18:20"; antes, a data e a hora
/// ("15/10/2026 09:05"). Texto que não é data volta como o [`formatar_data`] o deixa.
fn quando_curto(iso: &str, hoje: NaiveDate) -> String {
    let Ok(data) = chrono::NaiveDateTime::parse_from_str(iso.get(..19).unwrap_or(iso), "%Y-%m-%dT%H:%M:%S") else {
        return formatar_data(iso);
    };
    let hora = data.format("%H:%M");
    if data.date() == hoje {
        hora.to_string()
    } else if hoje.pred_opt() == Some(data.date()) {
        format!("ontem · {hora}")
    } else {
        formatar_data(iso)
    }
}

/// O estado do Histórico entre os quadros.
#[derive(Default)]
pub(crate) struct Historico {
    /// O id do banco da lista.
    banco: String,
    /// A lista lida (None antes da primeira leitura deste banco).
    itens: Option<Vec<Item>>,
    pedido: Option<Promessa<Vec<ComandoSql>>>,
    /// A lista precisa ser lida de novo (o painel apareceu, ou uma Consulta executou algo).
    velha: bool,
    erro: Option<String>,
    /// O comando selecionado (pelo texto: a lista é relida a cada execução).
    selecionado: Option<String>,
}

impl Historico {
    /// Uma Consulta respondeu: um comando pode ter entrado no histórico. À vista, a lista é relida.
    pub(crate) fn marcar_velha(&mut self) {
        self.velha = true;
    }

    /// O painel não está à vista: a leitura em andamento é solta, e a lista é relida quando ele aparecer.
    pub(crate) fn esconder(&mut self) {
        self.pedido = None;
        self.velha = true;
    }

    /// Há uma leitura rodando.
    pub(crate) fn ocupado(&self) -> bool {
        self.pedido.is_some()
    }

    /// A lista precisa ser lida de novo (para os testes).
    #[cfg(test)]
    pub(crate) fn velha(&self) -> bool {
        self.velha
    }

    /// Nos testes: o estado depois de a primeira leitura deste banco falhar (o aviso, nenhuma lista, nada pedido).
    pub(crate) fn simular_falha(&mut self, banco: String, mensagem: &str) {
        *self = Historico { banco, erro: Some(mensagem.to_string()), ..Historico::default() };
    }

    /// Recebe a leitura que chegou e pede outra se a lista é de outro banco ou está velha.
    fn receber(&mut self, ui: &Ui, ctx: &mut Contexto) {
        let id = ctx.banco_id();
        if id != self.banco {
            *self = Historico { banco: id, velha: true, ..Historico::default() };
        }
        if let Some(r) = self.pedido.as_mut().and_then(Promessa::tomar) {
            self.pedido = None;
            match r {
                Ok(lista) => {
                    let hoje = chrono::Local::now().date_naive();
                    self.itens = Some(lista.into_iter().map(|c| Item::de(c, hoje)).collect());
                    self.erro = None;
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        self.erro = Some(e.mensagem);
                    }
                }
            }
        }
        if self.velha && self.pedido.is_none() {
            self.velha = false;
            self.pedido = Some(ctx.tarefa(ui.ctx(), |s, id| s.historico(id)));
        }
    }

    /// Desenha o painel no espaço de `ui`.
    pub(crate) fn mostrar(&mut self, ui: &mut Ui, lugar_acoes: Encaixe, ctx: &mut Contexto) {
        if !ctx.estado.tem_banco() {
            *self = Historico::default();
            Painel::new("historico", "Histórico").encaixe(lugar_acoes).mostrar(ui, |ui| {
                vazio_centrado(ui, "Nenhum banco aberto.", None, None);
            });
            return;
        }
        self.receber(ui, ctx);
        let mut escolhido = None;
        Painel::new("historico", "Histórico").encaixe(lugar_acoes).mostrar(ui, |ui| {
            if let Some(erro) = &self.erro {
                egui::Frame::NONE.inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    aviso_texto(ui, erro, true);
                });
            }
            match &self.itens {
                // o ponto pulsante é de quem espera uma leitura: depois do erro, sem pedido correndo, fica só o aviso
                None if self.pedido.is_some() => {
                    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                        carregando(ui, "Carregando…");
                    });
                }
                None => {}
                Some(itens) if itens.is_empty() => {
                    vazio_centrado_com(ui, Icone::Historico, VAZIO, Some(VAZIO_EXPLICACAO), None);
                }
                Some(itens) => {
                    let selecionado = &mut self.selecionado;
                    ScrollArea::vertical().id_salt("historico-rolagem").auto_shrink([false, false]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        // `padding:6px 0 8px`
                        ui.add_space(6.0);
                        for (i, item) in itens.iter().enumerate() {
                            let on = selecionado.as_deref() == Some(item.sql.as_str());
                            let r = item_historico(ui, i, item, on);
                            if r.clicked() || r.double_clicked() {
                                *selecionado = Some(item.sql.clone());
                                r.request_focus();
                            }
                            let enter = r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if r.double_clicked() || enter {
                                escolhido = Some(item.sql.clone());
                            }
                        }
                        ui.add_space(8.0);
                    });
                }
            }
        });
        if let Some(sql) = escolhido {
            ctx.emitir(Acao::NovaConsulta { sql: Some(sql) });
        }
    }
}

/// O resumo em até duas linhas, cortado com "…".
fn galley_resumo(ui: &Ui, texto: &str, cor: Color32, largura: f32) -> Arc<Galley> {
    let mut f = formato(fontes::fonte(12.0, Peso::Leve), cor);
    // `line-height:1.45`
    f.line_height = Some((12.0_f32 * 1.45).round());
    let mut job = LayoutJob::single_section(texto.to_string(), f);
    job.wrap =
        TextWrapping {
            max_width: largura.max(1.0), max_rows: 2, break_anywhere: false, overflow_character: Some('…')
        };
    ui.fonts_mut(|f| f.layout_job(job))
}

/// A meta ao lado da pílula, numa linha só, cortada com "…".
fn galley_meta(ui: &Ui, texto: &str, cor: Color32, largura: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(texto.to_string(), formato(fontes::fonte(TAMANHO_META, Peso::Normal), cor));
    job.wrap =
        TextWrapping {
            max_width: largura.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…')
        };
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Um comando (`.hist`: `margin:2px 6px`, `padding:7px 10px`, o raio do nó da árvore, o texto em `--t2`; no
/// hover, o fundo `--h` e o texto `--t`; selecionado, o `.hist.on`: `--sel`, o realce de 1px e o brilho fraco do
/// destaque, com o texto em `--t`). Embaixo, a `.meta`: a pílula do resultado e o horário em `--t3`; no item
/// selecionado, a pílula de erro vira só contorno (`.hist.on .pill.erro`), para não sumir no `--sel`. O SQL
/// inteiro, o erro e a data vão na dica. O rótulo para o leitor de tela é o resumo, com o `selected`.
fn item_historico(ui: &mut Ui, i: usize, item: &Item, on: bool) -> Response {
    let p = paleta(ui);
    let e = p.estudio;
    let largura = ui.available_width();
    let largura_texto = largura - 2.0 * (MARGEM_X + RECUO_X);
    let g = galley_resumo(ui, &item.resumo, Color32::PLACEHOLDER, largura_texto);
    let pilula = item.pilula.as_ref().map(|(texto, tipo)| (texto, *tipo, tamanho_pill(ui, texto)));
    let altura_meta = match (&pilula, item.meta.is_empty()) {
        (Some((_, _, tamanho)), _) => tamanho.y,
        (None, false) => (TAMANHO_META * 1.45).round(),
        (None, true) => 0.0,
    };
    let altura_meta = if altura_meta > 0.0 { ESPACO_META + altura_meta } else { 0.0 };
    let altura = RECUO_Y * 2.0 + g.size().y + altura_meta;
    let (linha, _) = ui.allocate_exact_size(vec2(largura, altura + 2.0 * MARGEM_Y), Sense::hover());
    let rect = Rect::from_min_max(
        pos2(linha.left() + MARGEM_X, linha.top() + MARGEM_Y),
        pos2(linha.right() - MARGEM_X, linha.bottom() - MARGEM_Y),
    );
    let resp = ui.interact(rect, ui.id().with(("historico::item", i)), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.contains_pointer(), duracao_transicao(ui));
    let cor = if on { e.t } else { pintura::misturar(e.t2, e.t, hover) };
    if ui.is_rect_visible(rect.expand(10.0)) {
        let painter = ui.painter();
        if on {
            // `.hist.on`: `--sel`, `inset 0 1px 0 rgba(255,255,255,.07)` e `0 0 10px` do destaque a 22%
            pintura::halo(&pintor_de_fora(ui, rect), rect, RAIO_NO, 10.0, e.brilho_item);
            painter.rect_filled(rect, RAIO_NO, e.sel);
            realce_curvo(painter, rect, RAIO_NO, rgba(255, 255, 255, 0.07));
        } else if hover > 0.0 {
            painter.rect_filled(rect, RAIO_NO, e.h.gamma_multiply(hover));
        }
        let topo_meta = rect.top() + RECUO_Y + g.size().y + ESPACO_META;
        painter.galley(pos2(rect.left() + RECUO_X, rect.top() + RECUO_Y), g, cor);
        if altura_meta > 0.0 {
            let mut x = rect.left() + RECUO_X;
            let centro = topo_meta + (altura_meta - ESPACO_META) / 2.0;
            if let Some((texto, tipo, tamanho)) = pilula {
                let r = Rect::from_min_size(pos2(x, centro - tamanho.y / 2.0), tamanho);
                if on && tipo == TipoPill::No {
                    pintar_pill_contorno(ui, r, texto, tipo);
                } else {
                    pintar_pill(ui, r, texto, tipo);
                }
                x += tamanho.x + VAO_META;
            }
            if !item.meta.is_empty() {
                let gm = galley_meta(ui, &item.meta, e.t3, rect.right() - RECUO_X - x);
                painter.galley(pos2(x, centro - gm.size().y / 2.0), gm, e.t3);
            }
        }
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(&pintor_de_fora(ui, rect), p, rect, RAIO_NO);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, on, &item.resumo));
    resp.on_hover_text(&item.dica)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn dia(texto: &str) -> NaiveDate {
        NaiveDate::parse_from_str(texto, "%Y-%m-%d").unwrap()
    }

    fn comando(sql: &str, quando: Option<&str>, resultado: Option<ResultadoSql>, comandos: Option<u32>) -> ComandoSql {
        ComandoSql { sql: sql.into(), quando: quando.map(str::to_string), resultado, comandos }
    }

    #[test]
    fn item_com_resumo_de_uma_linha_e_a_data() {
        let c = comando("SELECT *\n  FROM clientes\n WHERE id = 1", Some("2026-10-15T09:05:00"), None, None);
        let item = Item::de(c, dia("2026-10-19"));
        assert_eq!(item.resumo, "SELECT * FROM clientes WHERE id = 1");
        assert_eq!(item.sql, "SELECT *\n  FROM clientes\n WHERE id = 1", "o comando inteiro vai para a Consulta");
        // a dica: o SQL inteiro e a data
        assert!(item.dica.starts_with("SELECT *\n  FROM clientes\n WHERE id = 1\n\n15/10/2026"), "{}", item.dica);
        // guardado antes da 0.2.0: sem pílula, só o horário
        assert_eq!(item.pilula, None);
        assert_eq!(item.meta, "15/10/2026 09:05");
        let sem_data = Item::de(comando("BEGIN", None, None, None), dia("2026-10-19"));
        assert_eq!((sem_data.dica.as_str(), sem_data.meta.as_str()), ("BEGIN", ""));
    }

    #[test]
    fn pilula_de_cada_resultado() {
        let textos = |r: ResultadoSql| pilula(&r);
        assert_eq!(textos(ResultadoSql::Linhas { quantidade: 9, mais: false }), ("9 linhas".into(), TipoPill::Ok));
        assert_eq!(textos(ResultadoSql::Linhas { quantidade: 1, mais: false }), ("1 linha".into(), TipoPill::Ok));
        assert_eq!(
            textos(ResultadoSql::Linhas { quantidade: 1000, mais: true }),
            ("1.000+ linhas".into(), TipoPill::Ok)
        );
        assert_eq!(textos(ResultadoSql::Alteradas(3)), ("3 alteradas".into(), TipoPill::Neutra));
        assert_eq!(textos(ResultadoSql::Alteradas(1)), ("1 alterada".into(), TipoPill::Neutra));
        assert_eq!(textos(ResultadoSql::Executado), ("Executado".into(), TipoPill::Neutra));
        assert_eq!(textos(ResultadoSql::erro(1, "A tabela não existe: x.")), ("Erro".into(), TipoPill::No));
        assert_eq!(textos(ResultadoSql::erro(3, "A tabela não existe: x.")), ("Parou no 3".into(), TipoPill::No));
        assert_eq!(textos(ResultadoSql::Interrompida { numero: 1 }), ("Interrompida".into(), TipoPill::Wait));
    }

    #[test]
    fn item_que_falhou_leva_o_erro_na_dica_e_os_comandos_na_meta() {
        let hoje = dia("2026-10-19");
        let c = comando(
            "SELECT 1; SELECT 2; SELECT * FROM x",
            Some("2026-10-19T10:42:07"),
            Some(ResultadoSql::erro(3, "A tabela não existe: x.")),
            Some(3),
        );
        let item = Item::de(c, hoje);
        assert_eq!(item.pilula, Some(("Parou no 3".into(), TipoPill::No)));
        assert_eq!(item.meta, "3 comandos · 10:42");
        assert_eq!(
            item.dica,
            "SELECT 1; SELECT 2; SELECT * FROM x\n\nParou no comando 3: A tabela não existe: x.\n\n19/10/2026 10:42"
        );

        let um = comando(
            "SELECT * FROM x",
            Some("2026-10-18T18:20:00"),
            Some(ResultadoSql::erro(1, "Sem tabela.")),
            Some(1),
        );
        let item = Item::de(um, hoje);
        assert_eq!(item.pilula, Some(("Erro".into(), TipoPill::No)));
        assert_eq!(item.meta, "ontem · 18:20", "um comando só não diz quantos");
        assert!(item.dica.contains("\n\nErro: Sem tabela.\n\n"), "{}", item.dica);
    }

    #[test]
    fn horario_curto() {
        let hoje = dia("2026-10-19");
        assert_eq!(quando_curto("2026-10-19T10:42:07", hoje), "10:42");
        assert_eq!(quando_curto("2026-10-18T18:20:00", hoje), "ontem · 18:20");
        assert_eq!(quando_curto("2026-10-15T09:05:00", hoje), "15/10/2026 09:05");
        assert_eq!(quando_curto("2026-01-01T00:00:00", dia("2026-01-02")), "ontem · 00:00");
        assert_eq!(quando_curto("não é data", hoje), "não é data");
    }
}
