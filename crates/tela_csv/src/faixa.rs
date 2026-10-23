//! A barra dos filtros do CSV (prancheta 7, revisão de UI de 19/10/2026), como a da aba Dados, no alto do documento (o
//! CSV não tem mais a faixa desde a 0.2.3, item F: o formato foi para o rodapé): a `.barra-filtros` com as peças de
//! `interface::pecas::filtros` ("Filtros", uma etiqueta por condição, "+ Condição" e, filtrando, "Limpar filtros"; na
//! ponta direita, a contagem da busca da barra, "3 de 41") e o popover da condição. A contagem das linhas fica no
//! rodapé ("Linhas 1–24 de 236 · filtradas de 1.180", U3 da revisão de UI de 22/10/2026). E a vista: pedida numa tarefa quando as condições, a ordem ou o documento mudam.

use std::sync::Arc;

use egui::Ui;
use estado::Promessa;
use interface::formato::plural;
use interface::pecas::filtros::{
    CondicaoEmEdicao, EtiquetaFiltro, FimCondicao, barra_filtros, filtros_com_direita, popover_condicao,
};

use crate::filtro::{CalculoVista, ChaveVista, Condicao, OPERADORES, OperadorCsv, Ordem, calcular_vista};
use crate::sessao::{Controle, Trabalho};
use crate::tela::TelaCsv;

/// A condição aberta no popover: a nova (`alvo` None) ou a de uma etiqueta.
pub(crate) type EdicaoCondicao = CondicaoEmEdicao<OperadorCsv>;

impl TelaCsv {
    /// Há filtro ou ordem: a ordem da tela não é a do arquivo.
    pub fn filtrando(&self) -> bool {
        !self.filtros.is_empty() || self.ordem.is_some()
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

    // ------------------------------------------------------------------ a faixa, os filtros e o popover

    /// A `.barra-filtros` no alto do conteúdo: "Filtros", as etiquetas, "+ Condição" e, filtrando, "Limpar filtros";
    /// e o popover da condição.
    pub(crate) fn desenhar_filtros(&mut self, ui: &mut Ui) {
        let etiquetas: Vec<EtiquetaFiltro<'_>> = self
            .filtros
            .iter()
            .map(|f| EtiquetaFiltro {
                id: f.id,
                coluna: &f.coluna,
                operador: f.op.rotulo(),
                valor: (!f.op.sem_valor()).then_some(f.valor.as_str()),
            })
            .collect();
        // a contagem da busca da barra ("3 de 41") na ponta direita: o CSV não tem mais a faixa
        let busca = self.busca.rotulo();
        let r = barra_filtros(ui, |ui| {
            filtros_com_direita(ui, &etiquetas, |ui| {
                if let Some(rotulo) = &busca {
                    let cor = interface::tema::paleta(ui).estudio.t2;
                    estado::busca::rotulo_na_faixa(ui, rotulo, cor);
                }
            })
        });
        let mut abrir = r.abrir;
        if std::mem::take(&mut self.abrir_condicao_pendente) {
            abrir = Some((None, r.nova_condicao));
        }
        if let Some(id) = r.tirar {
            self.filtros.retain(|f| f.id != id);
            if self.condicao.as_ref().is_some_and(|c| c.alvo == Some(id)) {
                self.condicao = None;
            }
        }
        if r.limpar {
            self.filtros.clear();
            self.condicao = None;
        }
        let nomes = self.retrato.as_ref().map(|r| r.nomes.clone()).unwrap_or_default();
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

    /// O popover da condição (`interface::pecas::filtros::popover_condicao`, o da aba Dados com os operadores do
    /// CSV).
    fn desenhar_condicao(&mut self, ectx: &egui::Context, nomes: &[String]) {
        let Some(c) = self.condicao.as_mut() else { return };
        let colunas: Vec<(usize, &str)> = nomes.iter().map(String::as_str).enumerate().collect();
        let id = ("tela_csv::condicao", self.id.as_str());
        match popover_condicao(ectx, id, c, &colunas, &OPERADORES, |o| o.sem_valor()) {
            Some(FimCondicao::Aplicar) => {
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
            }
            Some(FimCondicao::Desistir) => self.condicao = None,
            None => {}
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
