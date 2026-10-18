//! A tabela (ou visão) aberta numa tela `Rota::Tabela`/`Rota::Visao`: o que a casca carrega e entrega a cada
//! aba.
//!
//! A casca (crate editor) carrega a [`TabelaCompleta`] pelo serviço, desenha o cabeçalho (trilha, h2, sub,
//! "Exportar…") e as abas, e chama a aba ativa com `&mut TabelaAberta`. A aba:
//! - lê `info` e `rota` (a aba Dados tira dela página, ordem, busca e filtros);
//! - pede `recarregar = true` depois de mudar a estrutura (a casca relê a tabela e os objetos e refaz as abas);
//! - informa as linhas marcadas (`selecao`) e o total já contado da busca e dos filtros
//!   (`definir_contagem_filtrada`), que o "Exportar…" do cabeçalho usa sem contar de novo.

use banco::TabelaInfo;
use comum::Chave;
use servico::TabelaCompleta;

use crate::acao::{FiltroAtual, OrigemJanelaExportar, PedidoJanelaExportar};
use crate::rota::{Aba, FiltroDigitado, Rota};

/// A tabela da tela atual, compartilhada entre a casca e as abas.
#[derive(Debug, Clone)]
pub struct TabelaAberta {
    pub nome: String,
    pub e_visao: bool,
    pub completa: TabelaCompleta,
    /// A rota atual (aba e estado da aba Dados). A aba Dados a atualiza e emite `Acao::Atualizar`.
    pub rota: Rota,
    /// A aba pede que a casca releia a tabela e os objetos (depois de mudar a estrutura).
    pub recarregar: bool,
    /// Chaves das linhas marcadas na aba Dados.
    pub selecao: Vec<Chave>,
    contagem_filtrada: Option<(String, Vec<FiltroDigitado>, u64)>,
}

impl TabelaAberta {
    pub fn nova(nome: impl Into<String>, e_visao: bool, completa: TabelaCompleta, rota: Rota) -> TabelaAberta {
        TabelaAberta {
            nome: nome.into(),
            e_visao,
            completa,
            rota,
            recarregar: false,
            selecao: Vec::new(),
            contagem_filtrada: None,
        }
    }

    pub fn info(&self) -> &TabelaInfo {
        &self.completa.info
    }

    pub fn aba(&self) -> Aba {
        self.rota.aba().unwrap_or_default()
    }

    /// Guarda quantas linhas a busca e os filtros atuais acham.
    pub fn definir_contagem_filtrada(&mut self, busca: &str, filtros: &[FiltroDigitado], linhas: u64) {
        self.contagem_filtrada = Some((busca.to_string(), filtros.to_vec(), linhas));
    }

    /// O total já contado para esta busca e estes filtros, se houver.
    pub fn contagem_filtrada(&self, busca: &str, filtros: &[FiltroDigitado]) -> Option<u64> {
        match &self.contagem_filtrada {
            Some((b, f, n)) if b == busca && f.as_slice() == filtros => Some(*n),
            _ => None,
        }
    }

    /// O pedido da janela Exportar para o Exportar da barra de ferramentas: na aba Dados, com a busca, os filtros,
    /// a ordem e as linhas marcadas (a ordem vai mesmo sem busca nem filtro; a janela só oferece "Só o que a busca…" quando há busca ou filtro); nas outras
    /// abas, só a tabela inteira.
    pub fn pedido_exportacao(&self) -> PedidoJanelaExportar {
        let mut filtro = None;
        let mut marcadas = Vec::new();
        if self.aba() == Aba::Dados
            && let Some(d) = self.rota.dados()
        {
            if d.tem_busca_ou_filtro() || d.ordem.is_some() {
                filtro = Some(FiltroAtual {
                    busca: d.busca_opt().map(str::to_string),
                    filtros: d.filtros_banco(),
                    ordem: d.ordem.clone(),
                    direcao: d.direcao,
                    linhas: self.contagem_filtrada(&d.busca, &d.filtros),
                });
            }
            marcadas = self.selecao.clone();
        }
        PedidoJanelaExportar {
            origem: OrigemJanelaExportar::Tabela {
                nome: self.nome.clone(),
                total: self.info().linhas,
                filtro,
                marcadas,
            },
            nome_sugerido: self.nome.clone(),
        }
    }
}
