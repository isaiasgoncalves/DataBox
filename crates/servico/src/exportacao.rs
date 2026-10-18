//! Exportação acompanhada (§4.6): a exportação, o acompanhamento e a contagem das tabelas, sobre o `banco::Banco::iterar_*` e o `exportar::Escritor`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use banco::{Banco, FiltroExportacao, LinhasExportacao, Parar};
use comum::{ErroBanco, Resultado};
use exportar::{Acompanhamento, Escritor, Formato, Meta, Opcoes, Resumo, erro_de_leitura};
use prefs::MudancaExportacao;

use crate::progresso::{Fase, Progresso, ProgressoInfo, Rodada, erro_cancelada};
use crate::tipos::ResumoExportacao;
use crate::{Servico, caminhos};

/// De onde vêm as linhas exportadas (a `origem` de `POST /api/exportar`).
#[derive(Debug, Clone, PartialEq)]
pub enum OrigemExportacao {
    /// Uma tabela ou visão, com a busca, os filtros, a ordem e as linhas marcadas da grade.
    Tabela { nome: String, filtro: FiltroExportacao },
    /// O resultado de um único SELECT.
    Consulta { sql: String },
    /// Várias tabelas num arquivo só; None = todas as do banco (as de `ObjetosInfo.tabelas`).
    Banco { tabelas: Option<Vec<String>> },
}

/// `POST /api/exportar` sem a `tarefa` (o acompanhamento vai à parte, num [`Progresso`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoExportacao {
    pub origem: OrigemExportacao,
    pub formato: Formato,
    /// O caminho escolhido (no banco inteiro em CSV, o .zip ao lado é o gravado de fato).
    pub caminho: PathBuf,
    pub opcoes: Opcoes,
    /// Substituir o arquivo que já existe (a pessoa confirmou, ou o caminho veio do diálogo de salvar).
    pub substituir: bool,
}

/// Termina com erro a rodada do andamento que ainda estiver rodando quando sai de cena (só a desta
/// exportação: se a tela já começou outra com o mesmo `Progresso`, aquela fica como está).
struct Abandono<'a>(&'a Progresso, Rodada);

impl Drop for Abandono<'_> {
    fn drop(&mut self) {
        self.0.abandonar(self.1);
    }
}

/// Liga a exportação ao [`Progresso`] dela. Sem progresso, nada é contado nem
/// registrado: a exportação corre sem acompanhamento.
struct Acompanhar {
    progresso: Option<Arc<Progresso>>,
}

impl Acompanhar {
    fn ativo(&self) -> bool {
        self.progresso.is_some()
    }

    /// Cancelada → 409 `Codigo::Cancelado`.
    fn conferir(&self) -> Resultado<()> {
        self.progresso.as_ref().map_or(Ok(()), |p| p.conferir())
    }

    /// Muda a fase (e o que mais for preciso) e confere o cancelamento.
    fn fase(&self, fase: Fase, ajustar: impl FnOnce(&mut ProgressoInfo)) -> Resultado<()> {
        if let Some(p) = &self.progresso {
            p.atualizar(|info| {
                info.fase = fase;
                ajustar(info);
            });
        }
        self.conferir()
    }

    /// O `parar()` das leituras do banco: verdadeiro interrompe o SQLite na hora.
    fn parar(&self) -> Option<Arc<Parar>> {
        self.progresso.clone().map(|p| Arc::new(move || p.cancelado()) as Arc<Parar>)
    }

    /// Os callbacks do Escritor: linhas a cada ~1.000, a tabela atual (só no banco inteiro) e a finalização.
    fn acompanhamento(&self, com_tabela: bool) -> Acompanhamento {
        let Some(p) = &self.progresso else {
            return Acompanhamento::default();
        };
        let (a, b, c) = (Arc::clone(p), Arc::clone(p), Arc::clone(p));
        let acomp = Acompanhamento::default()
            .ao_progresso(move |linhas| {
                a.atualizar(|info| info.linhas = linhas);
                a.conferir()
            })
            .ao_finalizar(move || {
                b.atualizar(|info| {
                    info.fase = Fase::Finalizando;
                    info.tabelas_feitas = info.tabelas_total;
                });
                b.conferir()
            });
        if !com_tabela {
            return acomp;
        }
        acomp.ao_tabela(move |nome, feitas| {
            c.atualizar(|info| {
                info.tabela_atual = Some(nome.to_string());
                info.tabelas_feitas = feitas;
            });
            c.conferir()
        })
    }
}

impl Servico {
    /// `POST /api/exportar` (ver a documentação do crate).
    pub fn exportar(
        &self,
        banco_id: &str,
        pedido: &PedidoExportacao,
        progresso: Option<Arc<Progresso>>,
    ) -> Resultado<ResumoExportacao> {
        let banco = match self.banco_aberto(banco_id) {
            Ok(banco) => banco,
            Err(erro) => {
                // recusada antes de começar: o andamento não fica em Preparando para sempre
                if let Some(p) = &progresso {
                    p.recusar();
                }
                return Err(erro);
            }
        };
        let vez = match &progresso {
            Some(p) => Some((Arc::clone(p), p.iniciar()?)),
            None => None,
        };
        // se algo desandar sem passar pelos `terminar` abaixo (um pânico), o andamento não fica "rodando"
        let _guarda = vez.as_ref().map(|(p, rodada)| Abandono(p, *rodada));
        let acompanhar = Acompanhar { progresso };
        let resumo = match self.gravar_exportacao(&banco, pedido, &acompanhar) {
            Ok(resumo) => resumo,
            Err(erro) => {
                let Some((p, rodada)) = &vez else {
                    return Err(erro);
                };
                if p.cancelado() {
                    // interrompida em qualquer ponto (contagem, leitura, gravação, finalização): o Escritor já
                    // apagou o temporário e o destino ficou como estava
                    log::info!("Exportação cancelada: {erro}");
                    p.terminar(*rodada, Fase::Cancelada, |_| {});
                    return Err(erro_cancelada());
                }
                p.terminar(*rodada, Fase::Erro, |_| {});
                return Err(erro);
            }
        };
        if let Some((p, rodada)) = &vez {
            p.terminar(*rodada, Fase::Concluida, |info| {
                info.linhas = resumo.linhas;
                info.tabelas_feitas = info.tabelas_total;
            });
        }
        // as opções usadas viram o padrão da próxima exportação
        let o = &pedido.opcoes;
        let padrao = MudancaExportacao {
            cabecalho: Some(o.cabecalho),
            csv_separador: Some(o.csv_separador),
            csv_decimal: Some(o.csv_decimal),
            json_indentado: Some(o.json_indentado),
            csv_proteger_formulas: Some(o.csv_proteger_formulas),
        };
        if let Err(e) = self.prefs.atualizar(prefs::Mudanca::exportacao(padrao)) {
            // a exportação já deu certo; uma opção estranha só não vira padrão
            log::info!("Opções de exportação não guardadas como padrão: {padrao:?} ({e})");
        }
        Ok(ResumoExportacao {
            nome: resumo.nome(),
            caminho: resumo.caminho,
            formato: resumo.formato,
            linhas: resumo.linhas,
            bytes: resumo.bytes,
            tabelas: resumo.tabelas,
            tempo_ms: resumo.tempo_ms,
            avisos: resumo.avisos,
        })
    }

    fn gravar_exportacao(
        &self,
        banco: &Banco,
        pedido: &PedidoExportacao,
        acompanhar: &Acompanhar,
    ) -> Resultado<Resumo> {
        acompanhar.conferir()?; // cancelada antes de começar
        let destino = caminhos::limpar_caminho(&pedido.caminho, "o caminho do arquivo de destino")?;
        let varias = matches!(pedido.origem, OrigemExportacao::Banco { .. });
        // o arquivo que vai ser gravado de fato (o banco inteiro em CSV vira .zip)
        let final_ = exportar::destino_final(pedido.formato, &destino, varias)?;
        self.conferir_destino(&final_, pedido.substituir, Some(banco))?;
        let formato = pedido.formato;
        let opcoes = &pedido.opcoes;
        match &pedido.origem {
            OrigemExportacao::Tabela { nome, filtro } => {
                if acompanhar.ativo() {
                    // sem busca, filtro nem chaves, é o count(*) da tabela, que o SQLite faz sem ler as linhas
                    acompanhar.fase(Fase::Contando, |info| {
                        info.tabelas_total = 1;
                        info.tabela_atual = Some(nome.clone());
                    })?;
                    let total = banco.contar_para_exportar(nome, filtro, acompanhar.parar(), None)?;
                    acompanhar.fase(Fase::Gravando, |info| info.total = total)?;
                }
                banco.iterar_tabela(nome, filtro, acompanhar.parar(), |colunas, linhas| {
                    let escritor =
                        Escritor::novo(formato, &destino, opcoes, false, None, acompanhar.acompanhamento(false))?;
                    gravar_fonte(escritor, nome, colunas, linhas)
                })
            }
            OrigemExportacao::Consulta { sql } => {
                acompanhar.fase(Fase::Gravando, |info| info.tabelas_total = 1)?;
                banco.iterar_consulta(sql, acompanhar.parar(), |colunas, linhas| {
                    let escritor =
                        Escritor::novo(formato, &destino, opcoes, false, None, acompanhar.acompanhamento(false))?;
                    gravar_fonte(escritor, "consulta", colunas, linhas)
                })
            }
            OrigemExportacao::Banco { tabelas } => {
                let nomes = match tabelas {
                    None => {
                        let nomes: Vec<String> = banco.objetos()?.tabelas.into_iter().map(|t| t.nome).collect();
                        if nomes.is_empty() {
                            return Err(ErroBanco::novo(
                                "O banco não tem tabelas para exportar. Crie uma com Nova tabela, na barra de \
                                 ferramentas, ou pela Consulta SQL.",
                            ));
                        }
                        nomes
                    }
                    Some(pedidas) if pedidas.is_empty() => {
                        return Err(ErroBanco::novo("Escolha ao menos uma tabela para exportar."));
                    }
                    Some(pedidas) => pedidas.clone(),
                };
                if acompanhar.ativo() {
                    acompanhar.fase(Fase::Contando, |info| info.tabelas_total = nomes.len())?;
                    let total = contar_tabelas(banco, &nomes, acompanhar, self.prazo_contagem_banco)?;
                    acompanhar.fase(Fase::Gravando, |info| info.total = total)?;
                }
                let meta = Meta { banco: Some(banco.nome().to_string()), exportado_em: None };
                let mut escritor =
                    Escritor::novo(formato, &destino, opcoes, true, Some(meta), acompanhar.acompanhamento(true))?;
                // uma tabela de cada vez: cada leitura só abre quando a exportação chega na tabela dela
                for nome in &nomes {
                    banco.iterar_tabela(
                        nome,
                        &FiltroExportacao::default(),
                        acompanhar.parar(),
                        |colunas, linhas| {
                            escritor.comecar_tabela(nome, colunas)?;
                            for linha in linhas {
                                escritor.linha(&linha.map_err(erro_de_leitura)?)?;
                            }
                            escritor.terminar_tabela()
                        },
                    )?;
                }
                escritor.concluir()
            }
        }
    }
}

/// Grava a fonte única de uma tabela ou consulta e conclui.
fn gravar_fonte(
    mut escritor: Escritor,
    nome: &str,
    colunas: &[String],
    linhas: &mut LinhasExportacao<'_>,
) -> Resultado<Resumo> {
    escritor.comecar_tabela(nome, colunas)?;
    for linha in linhas {
        escritor.linha(&linha.map_err(erro_de_leitura)?)?;
    }
    escritor.concluir()
}

/// O `_contar_tabelas`: soma das linhas das tabelas escolhidas, ou None se alguma não der para contar dentro
/// do prazo (somando todas).
fn contar_tabelas(banco: &Banco, nomes: &[String], acompanhar: &Acompanhar, prazo: Duration) -> Resultado<Option<u64>> {
    let fim = Instant::now() + prazo;
    let mut soma = 0u64;
    for nome in nomes {
        let restante = fim.saturating_duration_since(Instant::now());
        let n = if restante > Duration::ZERO {
            banco.contar_para_exportar(nome, &FiltroExportacao::default(), acompanhar.parar(), Some(restante))?
        } else {
            None
        };
        acompanhar.conferir()?;
        match n {
            Some(n) => soma = soma.saturating_add(n),
            None => return Ok(None),
        }
    }
    Ok(Some(soma))
}
