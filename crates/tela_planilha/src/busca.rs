//! A busca da barra na planilha (Ctrl+F, "Buscar em Plan1"): na aba inteira, não só nas linhas que já chegaram, pelo
//! texto como a grade o mostra (`Planilha::achar_todos`, em fluxo, numa tarefa), sem diferenciar maiúsculas nem
//! acentos e fora das colunas ocultas. A célula achada vira a ativa e a grade rola até ela; Enter e Shift+Enter andam
//! (com volta); a faixa diz "3 de 41" ("41+" enquanto conta); Esc limpa e volta à célula de antes. Cada tecla troca a
//! busca (a de antes é cancelada), e trocar de planilha a refaz na nova.

use egui::Context;
use estado::EventoBusca;
use estado::busca::BuscaEmFundo;

use crate::{Corpo, TelaPlanilha};

/// Um achado: a linha do Excel (de 1) e a coluna do arquivo (de 0).
type Achado = (u32, u32);

/// O estado da busca da barra de uma planilha.
#[derive(Default)]
pub(crate) struct BuscaPlanilha {
    /// O texto do campo.
    pub texto: String,
    busca: Option<BuscaEmFundo<Achado>>,
    /// A célula ativa da grade de antes da busca (o Esc volta a ela).
    antes: Option<Option<(usize, usize)>>,
    /// A aba (na pasta) da busca rodando.
    folha: Option<usize>,
    /// O achado que a grade ainda não tem como mostrar (a linha passa das que ela conhece antes do índice): vai a ele
    /// quando ela crescer.
    pendente: Option<Achado>,
}

impl BuscaPlanilha {
    /// O que a faixa diz da busca ("3 de 41"), quando há uma.
    pub(crate) fn rotulo(&self) -> Option<String> {
        self.busca.as_ref().map(BuscaEmFundo::rotulo)
    }

    /// A busca ainda percorre a planilha.
    pub(crate) fn ocupada(&self) -> bool {
        self.busca.as_ref().is_some_and(|b| !b.terminou())
    }
}

impl TelaPlanilha {
    /// A dica (e o rótulo) da busca da barra: a planilha da tela, ou o arquivo.
    pub(crate) fn dica_da_busca(&self) -> String {
        format!("Buscar em {}", self.nome_da_folha().unwrap_or(&self.nome))
    }

    /// O que a faixa diz da busca ("3 de 41", "41+", "Nenhum resultado"), quando há uma (para os testes).
    #[doc(hidden)]
    pub fn rotulo_da_busca(&self) -> Option<String> {
        self.busca.rotulo()
    }

    /// A célula ativa da grade (linha e coluna da grade).
    fn selecao_da_grade(&self) -> Option<(usize, usize)> {
        self.grade().and_then(|g| g.nucleo.selecao())
    }

    /// O Enter, o Shift+Enter, a tecla no campo e o Esc da busca da barra.
    pub(crate) fn evento_da_busca(&mut self, evento: EventoBusca, egui: &Context) {
        match evento {
            EventoBusca::Mudou => {
                if self.busca.texto.trim().is_empty() {
                    self.busca.busca = None;
                    self.busca.antes = None;
                    return;
                }
                if self.busca.antes.is_none() {
                    self.busca.antes = Some(self.selecao_da_grade());
                }
                self.comecar_busca(egui);
            }
            EventoBusca::Enter | EventoBusca::Anterior => {
                let adiante = evento == EventoBusca::Enter;
                match self.busca.busca.as_mut() {
                    Some(b) => {
                        if let Some(alvo) = b.seguinte(adiante) {
                            self.ir_ao_achado(alvo);
                        }
                    }
                    None if !self.busca.texto.trim().is_empty() => {
                        if self.busca.antes.is_none() {
                            self.busca.antes = Some(self.selecao_da_grade());
                        }
                        self.comecar_busca(egui);
                    }
                    None => {}
                }
            }
            EventoBusca::Limpar => {
                self.busca.busca = None;
                self.busca.pendente = None;
                if let Some(antes) = self.busca.antes.take()
                    && let Some(g) = self.grade_mut()
                {
                    g.nucleo.selecionar(antes);
                }
            }
        }
        egui.request_repaint();
    }

    /// Começa (ou recomeça) a busca do texto do campo na planilha da tela, a partir da célula ativa.
    fn comecar_busca(&mut self, egui: &Context) {
        let Some(aba) = (match &self.corpo {
            Corpo::Pronta(p) => p.aba.as_ref(),
            _ => None,
        }) else {
            self.busca.busca = None;
            return;
        };
        let (pasta, folha) = (aba.sessao.pasta().clone(), aba.sessao.folha);
        let ocultas: Vec<(u32, u32)> =
            aba.sessao.meta.colunas.iter().filter(|c| c.oculta).map(|c| (c.primeira, c.ultima)).collect();
        let origem = aba.grade.celula_ativa().unwrap_or((1, 0));
        let termo = self.busca.texto.trim().to_owned();
        self.busca.folha = Some(folha);
        self.busca.pendente = None;
        self.busca.busca = Some(BuscaEmFundo::nova(egui, Some(origem), true, move |coletor| {
            let planilha = pasta.planilha(folha)?;
            let parar = coletor.sinal_de_parar();
            planilha.achar_todos(&termo, &parar, &mut |linha, coluna| {
                let oculta = ocultas.iter().any(|&(a, b)| (a..=b).contains(&coluna));
                oculta || coletor.achou((linha, coluna))
            })?;
            Ok(())
        }));
    }

    /// A cada quadro: refaz a busca se a planilha da tela mudou, e vai ao achado que chegou.
    pub(crate) fn passo_da_busca(&mut self, egui: &Context) {
        if self.busca.busca.is_none() {
            return;
        }
        if self.folha_ativa() != self.busca.folha {
            self.busca.antes = None;
            self.comecar_busca(egui);
        }
        if let Some(alvo) = self.busca.busca.as_mut().and_then(|b| b.passo(egui)).or(self.busca.pendente) {
            self.ir_ao_achado(alvo);
        }
    }

    /// A célula achada vira a ativa (a grade rola até ela); a que a grade ainda não alcança fica pendente.
    fn ir_ao_achado(&mut self, (linha, coluna): Achado) {
        let Some(g) = self.grade_mut() else { return };
        let Some(c) = g.coluna_da_grade(coluna) else { return };
        let celula = (linha.saturating_sub(1) as usize, c);
        if g.nucleo.selecao() != Some(celula) {
            g.limpar_selecao();
            g.nucleo.selecionar(Some(celula));
        }
        let alcancou = g.nucleo.selecao() == Some(celula);
        self.busca.pendente = (!alcancou).then_some((linha, coluna));
    }
}
