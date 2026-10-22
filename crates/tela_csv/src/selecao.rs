//! O menu da célula na grade do CSV (0.2.3, `design/planos-0.2.3.md`, A.1 e A.2) e o rodapé da seleção (A.3).
//!
//! - **Copiar, Copiar com cabeçalho, Copiar como:** o intervalo, na hora se as linhas estão na página em mãos; senão
//!   numa tarefa da fila, que lê as que faltam (o [`Trabalho::LerIntervalo`] leva o [`ComoCopiar`]). Os números vão
//!   pelo tipo detectado da coluna, com o decimal do arquivo; o CSV do "Copiar como" usa o separador do arquivo e o
//!   `INSERT` o nome dele, sem a extensão.
//! - **Filtrar por este valor / Excluir este valor do filtro:** uma condição "igual a" ou "diferente de" pela célula
//!   ativa; a vazia vira "é vazio" e "não é vazio".
//! - **Marcar as linhas:** as do intervalo, somando às marcadas.
//! - Os itens de escrita (Colar, Limpar, Excluir as linhas) aparecem desligados até a próxima etapa da 0.2.3.
//! - **O rodapé da seleção:** as contas do intervalo, na hora se ele está na página; senão numa tarefa com o
//!   [`Controle`] (cancelada quando a seleção muda), com o "Somando…" enquanto ela roda.

use std::path::Path;
use std::sync::Arc;

use comum::Codigo;
use egui::{Align, Layout, Ui};
use estado::Promessa;
use interface::grade::{
    AcaoCelula, Intervalo, OfertasCelula, ResumoSelecao, frase_do_intervalo_copiado, menu_da_celula, rodape_da_selecao,
    rodape_somando,
};
use interface::pecas::toast::{toast_atencao, toast_concluido};
use tabular::colunas::{OpcoesCopia, ValorCopia, escrever_como, escrever_tsv, escrever_tsv_com_cabecalho};

use crate::colunas::ColunaDaGrade;
use crate::filtro::OperadorCsv;
use crate::sessao::{self, Controle, LIMITE_DA_COPIA, Trabalho};
use crate::tela::TelaCsv;
use crate::textos;

/// Como levar o intervalo para a área de transferência: a ação de copiar e o que ela precisa saber das colunas.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ComoCopiar {
    /// [`AcaoCelula::Copiar`], [`AcaoCelula::CopiarComCabecalho`] ou [`AcaoCelula::CopiarComo`].
    pub acao: AcaoCelula,
    /// O nome e se é numérica (o tipo detectado), de cada coluna do intervalo.
    pub nomes: Vec<String>,
    pub numericas: Vec<bool>,
    pub decimal: char,
    pub separador: char,
    /// A tabela do `INSERT`: o nome do arquivo sem a extensão.
    pub tabela: String,
}

impl ComoCopiar {
    /// O texto das `linhas` (uma por linha do intervalo, um texto por coluna). O Copiar de uma célula só leva o texto
    /// dela, sem o fim de linha do TSV.
    pub(crate) fn escrever(&self, linhas: &[Vec<String>]) -> String {
        match self.acao {
            AcaoCelula::CopiarComCabecalho => escrever_tsv_com_cabecalho(&self.nomes, linhas),
            AcaoCelula::CopiarComo(formato) => {
                let valores: Vec<Vec<ValorCopia<'_>>> = linhas
                    .iter()
                    .map(|l| {
                        l.iter()
                            .enumerate()
                            .map(|(j, t)| ValorCopia::do_csv(t, self.numericas.get(j) == Some(&true), self.decimal))
                            .collect()
                    })
                    .collect();
                let opcoes = OpcoesCopia { separador: self.separador, tabela: &self.tabela };
                escrever_como(formato, &self.nomes, &valores, &opcoes)
            }
            _ => match linhas {
                [linha] if linha.len() == 1 => linha[0].clone(),
                _ => escrever_tsv(linhas),
            },
        }
    }
}

/// O que as contas do rodapé valem: o intervalo, a versão da tabela e a vista (pela identidade dela).
#[derive(Debug, Clone, PartialEq)]
struct ChaveSoma {
    intervalo: Intervalo,
    versao: u64,
    vista: Option<usize>,
}

/// As contas do rodapé da seleção.
enum EstadoSoma {
    Pronta(ResumoSelecao),
    Somando { promessa: Promessa<ResumoSelecao>, controle: Arc<Controle> },
    Falhou,
}

pub(crate) struct SomaSelecao {
    chave: ChaveSoma,
    estado: EstadoSoma,
}

impl Drop for SomaSelecao {
    fn drop(&mut self) {
        // a seleção mudou (ou a tela fechou): a leitura de fundo para
        if let EstadoSoma::Somando { controle, .. } = &self.estado {
            controle.cancelar();
        }
    }
}

/// O nome do arquivo sem a extensão (a tabela do `INSERT`).
fn nome_da_tabela(nome: &str) -> String {
    Path::new(nome).file_stem().and_then(|s| s.to_str()).unwrap_or(nome).to_string()
}

impl TelaCsv {
    /// A coluna da tabela de cada coluna da grade do intervalo (None: a provisória, que vai vazia).
    fn colunas_do_intervalo(&self, iv: Intervalo) -> Vec<Option<usize>> {
        let Some(g) = self.grade.as_ref() else { return Vec::new() };
        iv.colunas()
            .map(|c| match g.dado(c).map(|d| self.mapa.get(d)) {
                Some(Some(ColunaDaGrade::Dados(t))) => Some(*t),
                Some(None) => g.dado(c),
                _ => None,
            })
            .collect()
    }

    /// A posição na tabela de cada linha do intervalo (sem filtro nem ordem, a vista é a própria tabela).
    fn posicoes_do_intervalo(&self, iv: Intervalo) -> Vec<u64> {
        let vista = self.janela.vista.as_ref();
        iv.linhas()
            .filter_map(|l| match vista {
                Some(v) => v.get(l).copied(),
                None => Some(l as u64),
            })
            .collect()
    }

    /// O que a `acao` de copiar precisa saber das colunas do intervalo.
    fn como_copiar(&self, acao: AcaoCelula, colunas: &[Option<usize>]) -> ComoCopiar {
        let r = self.retrato.as_ref();
        let nomes = colunas.iter().map(|t| t.and_then(|t| r?.nomes.get(t).cloned()).unwrap_or_default()).collect();
        let numericas =
            colunas.iter().map(|t| t.and_then(|t| r?.tipos.get(t).copied()).is_some_and(textos::numerica)).collect();
        ComoCopiar {
            acao,
            nomes,
            numericas,
            decimal: r.map_or(',', |r| r.formato.decimal),
            separador: r.map_or(';', |r| r.formato.separador),
            tabela: nome_da_tabela(&self.nome),
        }
    }

    /// Copia o intervalo (linhas da vista, colunas da grade) como a `acao` pede: na hora, se as linhas estão na página
    /// em mãos; senão numa tarefa, que lê as que faltam.
    pub(crate) fn copiar_intervalo_como(&mut self, ectx: &egui::Context, acao: AcaoCelula, iv: Intervalo) {
        if iv.celulas() > LIMITE_DA_COPIA {
            toast_atencao(ectx, textos::grande_demais_para_copiar(iv.celulas()));
            return;
        }
        let colunas = self.colunas_do_intervalo(iv);
        let como = self.como_copiar(acao, &colunas);
        let pagina = self.pagina.as_ref();
        if let Some(p) = pagina.filter(|p| p.contem(iv.linha0 as u64) && p.contem(iv.linha1 as u64)) {
            let linhas: Vec<Vec<String>> = iv
                .linhas()
                .map(|l| {
                    colunas
                        .iter()
                        .map(|t| t.and_then(|t| p.celula(l as u64, t)).unwrap_or_default().to_string())
                        .collect()
                })
                .collect();
            ectx.copy_text(como.escrever(&linhas));
            toast_concluido(ectx, frase_do_intervalo_copiado(iv));
            return;
        }
        let posicoes = self.posicoes_do_intervalo(iv);
        self.enfileirar(Trabalho::LerIntervalo { posicoes, colunas, como });
    }

    /// O que o menu oferece no CSV, com o intervalo `iv` e a célula ativa (linha da vista, coluna da tabela).
    fn ofertas_da_celula(&self, iv: Intervalo) -> OfertasCelula {
        let o = OfertasCelula::de_leitura(iv).com_escrita_por_vir(&[
            AcaoCelula::Colar,
            AcaoCelula::Limpar,
            AcaoCelula::ExcluirLinhas,
        ]);
        // a coluna provisória (o colar como nova coluna, ainda sem nome) não filtra
        let o = if self.celula_ativa().is_some() {
            o.ligar(AcaoCelula::FiltrarPorValor).ligar(AcaoCelula::ExcluirValorDoFiltro)
        } else {
            o
        };
        o.ligar(AcaoCelula::MarcarLinhas)
    }

    /// O menu da célula: chamado todo quadro, depois da grade (abre no clique direito numa célula e no Shift+F10).
    pub(crate) fn menu_da_celula(&mut self, ui: &Ui) {
        let Some(iv) = self.grade.as_ref().and_then(|g| g.nucleo.intervalo()) else {
            // sem célula ativa o menu não abre; chamado assim mesmo, para fechar o que estiver aberto
            if let Some(g) = self.grade.as_mut() {
                menu_da_celula(ui, &mut g.nucleo, &OfertasCelula::default());
            }
            return;
        };
        let ofertas = self.ofertas_da_celula(iv);
        let Some(g) = self.grade.as_mut() else { return };
        let Some(acao) = menu_da_celula(ui, &mut g.nucleo, &ofertas) else { return };
        let ectx = ui.ctx().clone();
        match acao {
            AcaoCelula::Copiar | AcaoCelula::CopiarComCabecalho | AcaoCelula::CopiarComo(_) => {
                self.copiar_intervalo_como(&ectx, acao, iv);
            }
            AcaoCelula::FiltrarPorValor | AcaoCelula::ExcluirValorDoFiltro => {
                self.filtrar_pela_ativa(acao == AcaoCelula::ExcluirValorDoFiltro);
            }
            AcaoCelula::MarcarLinhas => {
                let posicoes = self.posicoes_do_intervalo(iv);
                self.marcadas.extend(posicoes);
            }
            _ => {}
        }
    }

    /// O "Filtrar por este valor" (ou, com `excluir`, o "Excluir este valor do filtro"): uma condição "igual a" ou
    /// "diferente de" com o texto da célula ativa; a vazia vira "é vazio" ou "não é vazio".
    fn filtrar_pela_ativa(&mut self, excluir: bool) {
        let Some((l, c)) = self.celula_ativa() else { return };
        let (Some(valor), Some(nome)) = (
            self.pagina.as_ref().and_then(|p| p.celula(l as u64, c)).map(str::to_string),
            self.retrato.as_ref().and_then(|r| r.nomes.get(c)).cloned(),
        ) else {
            return;
        };
        let op = match (valor.trim().is_empty(), excluir) {
            (true, false) => OperadorCsv::Vazio,
            (true, true) => OperadorCsv::NaoVazio,
            (false, false) => OperadorCsv::Igual,
            (false, true) => OperadorCsv::Diferente,
        };
        self.filtrar(&nome, op, &valor);
    }

    /// As contas do rodapé da seleção, com mais de uma célula selecionada: na hora, se o intervalo está na página;
    /// senão numa tarefa (None em `Some(None)`: somando). Uma seleção nova cancela a conta da anterior.
    fn resumo_da_selecao(&mut self, ectx: &egui::Context) -> Option<Option<ResumoSelecao>> {
        let Some(iv) = self.grade.as_ref().and_then(|g| g.nucleo.intervalo()).filter(Intervalo::varias) else {
            self.soma = None;
            return None;
        };
        let chave = ChaveSoma {
            intervalo: iv,
            versao: self.versao,
            vista: self.janela.vista.as_ref().map(|v| Arc::as_ptr(v) as usize),
        };
        if self.soma.as_ref().is_none_or(|s| s.chave != chave) {
            self.soma = self.somar(ectx, iv).map(|estado| SomaSelecao { chave, estado });
        }
        let s = self.soma.as_mut()?;
        if let EstadoSoma::Somando { promessa, .. } = &mut s.estado
            && let Some(r) = promessa.tomar()
        {
            s.estado = match r {
                Ok(r) => EstadoSoma::Pronta(r),
                Err(e) => {
                    if !e.tem_codigo(Codigo::Cancelado) {
                        log::warn!("As contas da seleção falharam: {e}");
                    }
                    EstadoSoma::Falhou
                }
            };
        }
        match &s.estado {
            EstadoSoma::Pronta(r) => Some(Some(*r)),
            EstadoSoma::Somando { .. } => Some(None),
            EstadoSoma::Falhou => None,
        }
    }

    /// Começa as contas do intervalo: da página, se ela o cobre; senão uma tarefa que passa pelas colunas do arquivo.
    fn somar(&self, ectx: &egui::Context, iv: Intervalo) -> Option<EstadoSoma> {
        let r = self.retrato.as_ref()?;
        let colunas = self.colunas_do_intervalo(iv);
        let numericas: Vec<bool> =
            colunas.iter().map(|t| t.and_then(|t| r.tipos.get(t).copied()).is_some_and(textos::numerica)).collect();
        let decimal = r.formato.decimal;
        if let Some(p) = self.pagina.as_ref().filter(|p| p.contem(iv.linha0 as u64) && p.contem(iv.linha1 as u64)) {
            let mut resumo = ResumoSelecao::default();
            for l in iv.linhas() {
                for (t, &numerica) in colunas.iter().zip(&numericas) {
                    if let Some(texto) = t.and_then(|t| p.celula(l as u64, t)) {
                        resumo.somar_do_csv(texto, numerica, decimal);
                    }
                }
            }
            return Some(EstadoSoma::Pronta(resumo));
        }
        let tabela = self.tabela.clone()?;
        let posicoes = self.posicoes_do_intervalo(iv);
        let controle = Controle::novo();
        let na_tarefa = Arc::clone(&controle);
        let gancho = self.ganchos.somar.clone();
        let promessa = Promessa::rodar(ectx, move || {
            if let Some(g) = gancho {
                g();
            }
            let mut inst = sessao::instantaneo(&tabela)?;
            let total = usize::try_from(inst.linhas()).unwrap_or(usize::MAX);
            let mut no_intervalo = vec![false; total];
            let mut ultima = 0;
            for p in posicoes {
                if let Some(m) = usize::try_from(p).ok().and_then(|p| no_intervalo.get_mut(p)) {
                    *m = true;
                    ultima = ultima.max(p as usize);
                }
            }
            let mut resumo = ResumoSelecao::default();
            for (t, numerica) in colunas.into_iter().zip(numericas) {
                let Some(t) = t else { continue };
                let mut i = 0usize;
                inst.percorrer_coluna(t, &mut |f, n| na_tarefa.avisar(f, n), |valor| {
                    if no_intervalo.get(i) == Some(&true) {
                        resumo.somar_do_csv(valor, numerica, decimal);
                    }
                    i += 1;
                    // depois da última linha do intervalo, a passada para
                    Ok(i <= ultima)
                })?;
            }
            Ok(resumo)
        });
        Some(EstadoSoma::Somando { promessa, controle })
    }

    /// O lado direito do rodapé com mais de uma célula selecionada: as contas, ou "Somando…".
    pub(crate) fn rodape_da_selecao(&mut self, ui: &mut Ui) {
        let Some(resumo) = self.resumo_da_selecao(ui.ctx()) else { return };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match resumo {
            Some(r) => rodape_da_selecao(ui, &r),
            None => rodape_somando(ui),
        });
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use interface::grade::FormatoCopia;

    fn como(acao: AcaoCelula) -> ComoCopiar {
        ComoCopiar {
            acao,
            nomes: vec!["nome".into(), "limite".into()],
            numericas: vec![false, true],
            decimal: ',',
            separador: ';',
            tabela: "clientes".into(),
        }
    }

    #[test]
    fn escrever_nos_formatos() {
        let linhas = vec![vec!["Ana".to_string(), "10,5".to_string()], vec!["Bia".to_string(), String::new()]];
        assert_eq!(como(AcaoCelula::Copiar).escrever(&linhas), "Ana\t10,5\r\nBia\t\r\n");
        assert_eq!(como(AcaoCelula::Copiar).escrever(&[vec!["Ana".to_string()]]), "Ana", "uma célula: o texto");
        assert_eq!(como(AcaoCelula::CopiarComCabecalho).escrever(&linhas), "nome\tlimite\r\nAna\t10,5\r\nBia\t\r\n");
        assert_eq!(
            como(AcaoCelula::CopiarComo(FormatoCopia::Insert)).escrever(&linhas),
            "INSERT INTO \"clientes\" (\"nome\", \"limite\") VALUES\n  ('Ana', 10.5),\n  ('Bia', NULL);\n"
        );
        assert_eq!(
            como(AcaoCelula::CopiarComo(FormatoCopia::Csv)).escrever(&linhas),
            "nome;limite\r\nAna;10,5\r\nBia;\r\n"
        );
    }

    #[test]
    fn tabela_do_insert_sem_a_extensao() {
        assert_eq!(nome_da_tabela("clientes.csv"), "clientes");
        assert_eq!(nome_da_tabela("vendas 2026.tsv"), "vendas 2026");
        assert_eq!(nome_da_tabela("sem_extensao"), "sem_extensao");
    }
}
