//! A busca da barra no CSV (Ctrl+F, "Buscar em vendas.csv"): no arquivo inteiro, não só na página em mãos, e só nas
//! linhas que o filtro mostra, na ordem da vista. A tarefa tira a cópia da tabela para buscar (um leitor próprio e as
//! alterações de agora: `Tabela::copia_para_buscar`), percorre o arquivo pelo índice (`achar_todos`) e entrega cada
//! célula achada como (linha da vista, coluna); a célula achada vira a ativa e a grade rola até ela. Enter e
//! Shift+Enter andam, a faixa diz "3 de 41" ("41+" enquanto conta), Esc limpa e volta à célula de antes. Cada tecla
//! troca a busca (a de antes é cancelada), e uma alteração ou outra vista (filtro, ordem) a refaz.

use std::sync::Arc;

use estado::EventoBusca;
use estado::busca::BuscaEmFundo;

use crate::colunas::ColunaDaGrade;
use crate::sessao::travar;
use crate::tela::TelaCsv;

/// Um achado: a linha da vista e a coluna da tabela.
type Achado = (u64, usize);

/// O estado da busca da barra de um CSV.
#[derive(Default)]
pub(crate) struct BuscaCsv {
    /// O texto do campo.
    pub texto: String,
    busca: Option<BuscaEmFundo<Achado>>,
    /// A célula ativa de antes da busca (o Esc volta a ela).
    antes: Option<Option<(usize, usize)>>,
    /// O que a busca rodando viu: a versão da tabela e a vista.
    versao: u64,
    vista: Option<Arc<Vec<u64>>>,
}

impl BuscaCsv {
    /// O que a faixa diz da busca ("3 de 41"), quando há uma.
    pub(crate) fn rotulo(&self) -> Option<String> {
        self.busca.as_ref().map(BuscaEmFundo::rotulo)
    }

    /// A busca ainda percorre o arquivo.
    pub(crate) fn ocupada(&self) -> bool {
        self.busca.as_ref().is_some_and(|b| !b.terminou())
    }
}

impl TelaCsv {
    /// A dica (e o rótulo) da busca da barra.
    pub(crate) fn dica_da_busca(&self) -> String {
        format!("Buscar em {}", self.nome)
    }

    /// O que a faixa diz da busca ("3 de 41", "41+", "Nenhum resultado"), quando há uma (para os testes).
    #[doc(hidden)]
    pub fn rotulo_da_busca(&self) -> Option<String> {
        self.busca.rotulo()
    }

    /// O Enter, o Shift+Enter, a tecla no campo e o Esc da busca da barra.
    pub(crate) fn evento_da_busca(&mut self, evento: EventoBusca, ectx: &egui::Context) {
        match evento {
            EventoBusca::Mudou => {
                if self.busca.texto.trim().is_empty() {
                    self.busca.busca = None;
                    self.busca.antes = None;
                    return;
                }
                if self.busca.antes.is_none() {
                    self.busca.antes = Some(self.celula_ativa());
                }
                self.comecar_busca(ectx, true);
            }
            EventoBusca::Enter | EventoBusca::Anterior => {
                let adiante = evento == EventoBusca::Enter;
                let ativa = self.celula_ativa();
                match self.busca.busca.as_mut() {
                    Some(b) => {
                        // a busca refeita sem achado da vez parte da célula ativa
                        if let Some((l, c)) = ativa {
                            b.definir_origem((l as u64, c));
                        }
                        if let Some(alvo) = b.seguinte(adiante) {
                            self.ir_ao_achado(alvo);
                        }
                    }
                    None if !self.busca.texto.trim().is_empty() => {
                        if self.busca.antes.is_none() {
                            self.busca.antes = Some(self.celula_ativa());
                        }
                        self.comecar_busca(ectx, true);
                    }
                    None => {}
                }
            }
            EventoBusca::Limpar => {
                self.busca.busca = None;
                if let Some(antes) = self.busca.antes.take() {
                    self.selecionar(antes);
                }
            }
        }
        ectx.request_repaint();
    }

    /// Começa (ou recomeça) a busca do texto do campo na vista e na versão de agora; com `ir`, vai ao primeiro achado a
    /// partir da célula ativa.
    fn comecar_busca(&mut self, ectx: &egui::Context, ir: bool) {
        let Some(tabela) = self.tabela.clone() else { return };
        let termo = self.busca.texto.trim().to_owned();
        let vista = self.janela.vista.clone();
        let origem = ir.then(|| self.celula_ativa().map(|(l, c)| (l as u64, c)).unwrap_or((0, 0)));
        // sem vista, ou só filtrada, as linhas vêm na ordem dela; ordenada, a tarefa ordena os achados no fim
        let em_ordem = vista.as_ref().is_none_or(|v| v.windows(2).all(|par| par.first() < par.last()));
        self.busca.versao = self.versao;
        self.busca.vista = vista.clone();
        self.busca.busca = Some(BuscaEmFundo::nova(ectx, origem, em_ordem, move |coletor| {
            let copia = travar(&tabela).copia_para_buscar()?;
            // a posição na tabela de cada linha da vista, para achar a linha da vista de cada achado
            let indice: Option<Vec<(u64, u64)>> = vista.map(|v| {
                let mut pares: Vec<(u64, u64)> = v.iter().zip(0u64..).map(|(&p, i)| (p, i)).collect();
                pares.sort_unstable();
                pares
            });
            let parar = coletor.sinal_de_parar();
            copia.achar_todos(&termo, &parar, &mut |linha, coluna| {
                let na_vista = match &indice {
                    None => Some(linha),
                    Some(pares) => {
                        pares.binary_search_by_key(&linha, |&(p, _)| p).ok().and_then(|k| pares.get(k)).map(|&(_, i)| i)
                    }
                };
                na_vista.is_none_or(|l| coletor.achou((l, coluna)))
            })?;
            Ok(())
        }));
    }

    /// A cada quadro: refaz a busca se a tabela ou a vista mudaram (e só volta a ir a um achado se a célula ativa ainda
    /// era o da vez: quem editou outra célula não é puxado), e vai ao achado que chegou.
    pub(crate) fn passo_da_busca(&mut self, ectx: &egui::Context) {
        if self.busca.busca.is_none() {
            return;
        }
        let mesma_vista = match (&self.busca.vista, &self.janela.vista) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if self.busca.versao != self.versao || !mesma_vista {
            let ativa = self.celula_ativa().map(|(l, c)| (l as u64, c));
            let no_achado = ativa.is_some() && self.busca.busca.as_ref().and_then(BuscaEmFundo::atual) == ativa;
            self.comecar_busca(ectx, no_achado);
        }
        if let Some(alvo) = self.busca.busca.as_mut().and_then(|b| b.passo(ectx)) {
            self.ir_ao_achado(alvo);
        }
    }

    /// A célula achada vira a ativa (a grade rola até ela), na coluna da grade que mostra a coluna da tabela.
    fn ir_ao_achado(&mut self, (linha, coluna): Achado) {
        let Ok(linha) = usize::try_from(linha) else { return };
        let dado = self.mapa.iter().position(|c| *c == ColunaDaGrade::Dados(coluna)).unwrap_or(coluna);
        if let Some(g) = self.grade.as_mut() {
            g.nucleo.selecionar(Some((linha, dado + crate::grade::PRIMEIRA_DE_DADOS)));
        }
    }
}
