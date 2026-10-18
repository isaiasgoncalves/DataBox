//! A faixa dos filtros do CSV (a da aba Dados, prancheta 16 b): "Filtros", uma etiqueta por condição (o clique abre a
//! condição no popover, o × a tira), "+ Condição" e, à direita, "236 de 1.180 linhas" (ou "contando…") e "Limpar
//! filtros". E a vista: pedida numa tarefa quando as condições, a ordem ou o documento mudam.

use std::sync::Arc;

use egui::{Align, Layout, Rect, Sense, Ui, vec2};
use estado::{Contexto, Promessa};
use interface::formato::{formatar_numero, plural};
use interface::icones::Icone;
use interface::pecas::avisos::carregando;
use interface::pecas::botao::{Botao, LADO_PEQUENO};
use interface::pecas::campos::{Entrada as CampoTexto, Select, campo, enter_no_campo};
use interface::pecas::popover::Popover;
use interface::pecas::rotulos::chip_filtro;
use interface::pecas::texto::{galley_tit, texto as texto_simples};
use interface::tema::paleta;

use crate::filtro::{CalculoVista, ChaveVista, Condicao, OPERADORES, OperadorCsv, Ordem, calcular_vista};
use crate::sessao::{Controle, Trabalho};
use crate::tela::TelaCsv;

/// A largura do popover da condição (a da aba Dados).
const LARGURA_CONDICAO: f32 = 300.0;

/// A condição aberta no popover: a nova (`alvo` None) ou a de uma etiqueta.
#[derive(Debug, Clone)]
pub(crate) struct EdicaoCondicao {
    pub alvo: Option<u64>,
    pub coluna: usize,
    pub op: OperadorCsv,
    pub valor: String,
    pub ancora: Rect,
    pub focar: bool,
}

impl TelaCsv {
    /// Há filtro ou ordem: a ordem da tela não é a do arquivo.
    pub fn filtrando(&self) -> bool {
        !self.filtros.is_empty() || self.ordem.is_some()
    }

    /// A faixa aparece com condição, com o popover aberto ou pedido.
    pub(crate) fn com_faixa(&self) -> bool {
        !self.filtros.is_empty() || self.condicao.is_some() || self.abrir_condicao_pendente
    }

    /// As condições aplicadas.
    pub fn condicoes(&self) -> &[Condicao] {
        &self.filtros
    }

    /// A ordem pela seta do cabeçalho.
    pub fn ordem(&self) -> Option<&Ordem> {
        self.ordem.as_ref()
    }

    /// Aplica uma condição (os testes e o popover).
    pub fn filtrar(&mut self, coluna: &str, op: OperadorCsv, valor: &str) {
        self.proximo_filtro += 1;
        let valor = if op.sem_valor() { String::new() } else { valor.to_string() };
        self.filtros.push(Condicao { id: self.proximo_filtro, coluna: coluna.to_string(), op, valor });
    }

    /// A seta do cabeçalho: crescente → decrescente → sem ordem.
    pub fn ordenar(&mut self, coluna: &str) {
        self.ordem = match self.ordem.take() {
            Some(o) if o.coluna == coluna && !o.decrescente => Some(Ordem { coluna: o.coluna, decrescente: true }),
            Some(o) if o.coluna == coluna => None,
            _ => Some(Ordem { coluna: coluna.to_string(), decrescente: false }),
        };
    }

    /// O número de linhas da vista (com filtro), se ela já foi calculada.
    pub fn linhas_filtradas(&self) -> Option<u64> {
        self.janela.vista.as_ref().map(|v| v.len() as u64)
    }

    // ------------------------------------------------------------------ a vista

    /// Pede a vista que as condições e a ordem de agora pedem (a velha em cálculo é cancelada, e a resposta dela
    /// descartada), ou a tira.
    pub(crate) fn pedir_vista(&mut self, ectx: &egui::Context) {
        let Some(r) = self.retrato.as_ref() else { return };
        if !self.filtrando() {
            if self.janela.vista.is_some() {
                self.trocar_vista(None);
            }
            self.calculo_vista = None;
            return;
        }
        let chave = ChaveVista { condicoes: self.filtros.clone(), ordem: self.ordem.clone(), versao: self.versao };
        if self.vista_de.as_ref() == Some(&chave) || self.calculo_vista.as_ref().is_some_and(|c| c.chave == chave) {
            return;
        }
        let Some(tabela) = self.tabela.clone() else { return };
        let (condicoes, ordem, nomes, decimal) =
            (self.filtros.clone(), self.ordem.clone(), r.nomes.clone(), r.formato.decimal);
        let controle = Controle::novo();
        let c2 = Arc::clone(&controle);
        let promessa =
            Promessa::rodar(ectx, move || calcular_vista(&tabela, &condicoes, ordem.as_ref(), &nomes, decimal, &c2));
        // a vista velha em cálculo é cancelada ao ser trocada (o `Drop` do `CalculoVista`)
        self.calculo_vista = Some(CalculoVista { chave, promessa, controle });
    }

    /// A vista que chegou, se ainda é a pedida.
    pub(crate) fn receber_vista(&mut self) {
        let Some(c) = self.calculo_vista.as_mut() else { return };
        let Some(r) = c.promessa.tomar() else { return };
        let Some(c) = self.calculo_vista.take() else { return };
        let atual = ChaveVista { condicoes: self.filtros.clone(), ordem: self.ordem.clone(), versao: self.versao };
        if let Ok(posicoes) = r
            && c.chave.condicoes == atual.condicoes
            && c.chave.ordem == atual.ordem
        {
            self.vista_de = Some(c.chave.clone());
            self.trocar_vista(Some(Arc::new(posicoes)));
        }
    }

    /// Troca a vista da grade: a página em mãos não vale mais, e a primeira página da vista nova é pedida.
    fn trocar_vista(&mut self, vista: Option<Arc<Vec<u64>>>) {
        if vista.is_none() {
            self.vista_de = None;
        }
        self.janela.vista = vista;
        self.janela.inicio = 0;
        self.pagina = None;
        if let Some(g) = self.grade.as_mut() {
            g.nucleo.selecionar(None);
            g.medir_de_novo();
        }
        self.enfileirar(Trabalho::Pagina);
    }

    // ------------------------------------------------------------------ a faixa e o popover

    /// A faixa: "Filtros", as etiquetas, "+ Condição", a contagem e "Limpar filtros".
    pub(crate) fn desenhar_faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let p = paleta(ui);
        let nomes = self.retrato.as_ref().map(|r| r.nomes.clone()).unwrap_or_default();
        let g = galley_tit(ui, "Filtros", p.estudio.t3);
        let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(r.min, g, p.estudio.t3);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Filtros"));
        let mut abrir: Option<(Option<u64>, Rect)> = None;
        let mut tirar = None;
        for f in &self.filtros {
            let mut trechos = vec![(f.coluna.as_str(), true), (f.op.rotulo(), false)];
            if !f.op.sem_valor() {
                trechos.push((f.valor.as_str(), true));
            }
            let r = chip_filtro(ui, &trechos, "Tirar o filtro");
            if r.fechar {
                tirar = Some(f.id);
            } else if r.abrir {
                abrir = Some((Some(f.id), r.resposta.rect));
            }
        }
        let b = ui.add(
            Botao::ghost("Condição")
                .com_icone(Icone::Mais)
                .pequeno()
                .rotulo("Nova condição")
                .dica("Nova condição de filtro"),
        );
        if b.clicked() || std::mem::take(&mut self.abrir_condicao_pendente) {
            abrir = Some((None, b.rect));
        }
        let total = self.retrato.as_ref().map_or(0, |r| r.linhas);
        let filtradas = self.linhas_filtradas();
        let contando = self.calculo_vista.is_some();
        let tem_filtros = !self.filtros.is_empty();
        let mut limpar = false;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if tem_filtros && ui.add(Botao::ghost("Limpar filtros").pequeno()).clicked() {
                limpar = true;
            }
            let t3 = p.estudio.t3;
            match filtradas {
                _ if contando => {
                    carregando(ui, "contando…");
                }
                Some(n) if tem_filtros => {
                    texto_simples(
                        ui,
                        &format!("{} de {}", formatar_numero(n), plural(total, "linha", "linhas")),
                        12.0,
                        t3,
                    );
                }
                _ => {
                    texto_simples(ui, &plural(total, "linha", "linhas"), 12.0, t3);
                }
            }
        });
        if let Some(id) = tirar {
            self.filtros.retain(|f| f.id != id);
            if self.condicao.as_ref().is_some_and(|c| c.alvo == Some(id)) {
                self.condicao = None;
            }
        }
        if limpar {
            self.filtros.clear();
            self.condicao = None;
        }
        if let Some((alvo, ancora)) = abrir {
            let existente = alvo.and_then(|id| self.filtros.iter().find(|f| f.id == id));
            let (coluna, op, valor) = match existente {
                Some(f) => (nomes.iter().position(|n| *n == f.coluna).unwrap_or(0), f.op, f.valor.clone()),
                None => (self.coluna_em_foco().unwrap_or(0), OperadorCsv::Contem, String::new()),
            };
            self.condicao = Some(EdicaoCondicao { alvo, coluna, op, valor, ancora, focar: !op.sem_valor() });
        }
        let ectx = ui.ctx().clone();
        self.desenhar_condicao(&ectx, &nomes);
    }

    /// O popover da condição: Coluna, Operador, Valor (fora nos operadores sem valor), Cancelar e Aplicar; Enter no
    /// valor aplica, Esc e o clique fora desistem.
    fn desenhar_condicao(&mut self, ectx: &egui::Context, nomes: &[String]) {
        let Some(c) = self.condicao.as_mut() else { return };
        let colunas: Vec<(usize, &str)> = nomes.iter().map(String::as_str).enumerate().collect();
        let (mut aplicar, mut cancelar) = (false, false);
        let id = self.id.clone();
        let r = Popover::new(("tela_csv::condicao", &id), c.ancora)
            .largura(LARGURA_CONDICAO)
            .rotulo("Condição de filtro")
            .mostrar(ectx, |ui| {
                ui.spacing_mut().item_spacing.y = 12.0;
                campo(ui, "Coluna", None, |ui| {
                    Select::new(("tela_csv::coluna_condicao", &id), &mut c.coluna, &colunas)
                        .rotulo("Coluna da condição")
                        .mostrar(ui);
                });
                let mudou = campo(ui, "Operador", None, |ui| {
                    Select::new(("tela_csv::op_condicao", &id), &mut c.op, &OPERADORES)
                        .rotulo("Operador da condição")
                        .mostrar(ui)
                        .changed()
                });
                if mudou && !c.op.sem_valor() {
                    c.focar = true;
                }
                let pode = c.op.sem_valor() || !c.valor.is_empty();
                if !c.op.sem_valor() {
                    campo(ui, "Valor", None, |ui| {
                        let r = CampoTexto::new(&mut c.valor)
                            .dica("valor a procurar")
                            .rotulo("Valor da condição")
                            .id_salt(("tela_csv::valor_condicao", &id))
                            .mostrar(ui)
                            .response
                            .response;
                        if std::mem::take(&mut c.focar) {
                            r.request_focus();
                        }
                        if enter_no_campo(ui, &r) && !c.valor.is_empty() {
                            aplicar = true;
                        }
                    });
                }
                let largura = ui.available_width();
                ui.allocate_ui_with_layout(vec2(largura, LADO_PEQUENO), Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let b = ui.add(Botao::primario("Aplicar").pequeno().habilitado(pode));
                    if b.on_disabled_hover_text("Escreva um valor para aplicar").clicked() {
                        aplicar = true;
                    }
                    if ui.add(Botao::ghost("Cancelar").pequeno()).clicked() {
                        cancelar = true;
                    }
                });
            });
        if aplicar {
            let Some(c) = self.condicao.take() else { return };
            let Some(coluna) = nomes.get(c.coluna).cloned() else { return };
            let valor = if c.op.sem_valor() { String::new() } else { c.valor };
            match c.alvo.and_then(|id| self.filtros.iter_mut().find(|f| f.id == id)) {
                Some(f) => {
                    f.coluna = coluna;
                    f.op = c.op;
                    f.valor = valor;
                }
                None => self.filtrar(&coluna, c.op, &valor),
            }
        } else if r.fechar || cancelar {
            self.condicao = None;
        }
    }

    /// A dica do Filtrar da barra: "Filtrar", "Filtrar (1 condição)".
    pub(crate) fn dica_filtrar(&self) -> String {
        match self.filtros.len() {
            0 => "Filtrar".to_string(),
            n => format!("Filtrar ({})", plural(n as u64, "condição", "condições")),
        }
    }
}
