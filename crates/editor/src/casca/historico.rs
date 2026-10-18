//! O Histórico de consultas (o `aside` "Histórico" de `Consulta.dc.html`), que a barra de ícones põe no lugar do
//! Explorador: os comandos já executados no banco aberto (`servico.historico`, o mais recente primeiro), cada um com
//! o resumo de uma linha da Consulta SQL (`resumo_comando`, em até duas linhas, semilight) e, na dica, o SQL inteiro
//! e a data (item 20). N5 da prancheta 15: o clique seleciona (`.hist.on`); o duplo clique ou o Enter
//! abrem uma Consulta nova com aquele comando.
//!
//! A lista é lida numa tarefa quando o painel aparece e de novo quando ela fica velha à vista (a casca a marca a
//! cada resposta de uma Consulta, que é quando um comando entra no histórico). Escondido, o painel solta a leitura
//! que estava correndo. Vazio: o relógio de 48px, "Nenhuma consulta ainda." e, em `--t3`, "As consultas executadas
//! aparecem aqui."; sem banco: "Nenhum banco aberto.".

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, CursorIcon, Galley, Key, Rect, Response, ScrollArea, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use estado::{Acao, Contexto, Promessa, Reacao};
use interface::fontes::{self, Peso};
use interface::formato::formatar_data;
use interface::icones::Icone;
use interface::pecas::arvore::RAIO_NO;
use interface::pecas::avisos::{aviso_texto, carregando};
use interface::pecas::botao::{pintor_de_fora, realce_curvo};
use interface::pecas::painel::Painel;
use interface::pecas::texto::formato;
use interface::pintura;
use interface::tema::{duracao_transicao, paleta, rgba};
use prefs::ComandoSql;

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

/// Um comando da lista, com os textos já prontos.
struct Item {
    sql: String,
    resumo: String,
    /// A dica: o SQL inteiro e a data.
    dica: String,
}

impl Item {
    fn de(c: ComandoSql) -> Item {
        let data = c.quando.as_deref().map(formatar_data).unwrap_or_default();
        let dica = if data.is_empty() { c.sql.clone() } else { format!("{}\n\n{data}", c.sql) };
        Item { resumo: resumo_comando(&c.sql), dica, sql: c.sql }
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
                    self.itens = Some(lista.into_iter().map(Item::de).collect());
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
    pub(crate) fn mostrar(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if !ctx.estado.tem_banco() {
            *self = Historico::default();
            Painel::new("historico", "Histórico").mostrar(ui, |ui| {
                vazio_centrado(ui, "Nenhum banco aberto.", None, None);
            });
            return;
        }
        self.receber(ui, ctx);
        let mut escolhido = None;
        Painel::new("historico", "Histórico").mostrar(ui, |ui| {
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

/// Um comando (`.hist`: `margin:2px 6px`, `padding:7px 10px`, o raio do nó da árvore, o texto em `--t2`; no
/// hover, o fundo `--h` e o texto `--t`; selecionado, o `.hist.on`: `--sel`, o realce de 1px e o brilho fraco do
/// destaque, com o texto em `--t`). O SQL inteiro e a data vão na dica. O rótulo para o leitor de tela é o resumo,
/// com o `selected`.
fn item_historico(ui: &mut Ui, i: usize, item: &Item, on: bool) -> Response {
    let p = paleta(ui);
    let e = p.estudio;
    let largura = ui.available_width();
    let largura_texto = largura - 2.0 * (MARGEM_X + RECUO_X);
    let g = galley_resumo(ui, &item.resumo, Color32::PLACEHOLDER, largura_texto);
    let altura = RECUO_Y * 2.0 + g.size().y;
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
        painter.galley(pos2(rect.left() + RECUO_X, rect.top() + RECUO_Y), g, cor);
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

    #[test]
    fn item_com_resumo_de_uma_linha_e_a_data() {
        let c = ComandoSql {
            sql: "SELECT *\n  FROM clientes\n WHERE id = 1".into(),
            quando: Some("2026-10-15T09:05:00".into()),
        };
        let item = Item::de(c);
        assert_eq!(item.resumo, "SELECT * FROM clientes WHERE id = 1");
        assert_eq!(item.sql, "SELECT *\n  FROM clientes\n WHERE id = 1", "o comando inteiro vai para a Consulta");
        // a dica: o SQL inteiro e a data
        assert!(item.dica.starts_with("SELECT *\n  FROM clientes\n WHERE id = 1\n\n15/10/2026"), "{}", item.dica);
        let sem_data = Item::de(ComandoSql { sql: "BEGIN".into(), quando: None });
        assert_eq!(sem_data.dica, "BEGIN");
    }
}
