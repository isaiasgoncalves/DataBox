//! Consulta livre (a Consulta SQL): um roteiro executado comando a comando, parando no primeiro erro.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use comum::{Celula, ErroBanco, LIMITE_TEXTO, Resultado};
use rusqlite::Connection;

use crate::banco::{Banco, PASSOS_ENTRE_CONFERENCIAS};
use crate::dados::tempo_ms;
use crate::erros::{MSG_INTERROMPIDA, traduzir};
use crate::ffi::nomes_das_colunas;
use crate::sql::{dividir_comandos, e_dml};
use crate::tipos::{ResultadoComando, ResultadoConsulta};

/// O erro de comando com `?` sem valores.
pub(crate) fn erro_parametros(n: usize) -> ErroBanco {
    let original =
        format!("Incorrect number of bindings supplied. The current statement uses {n}, and there are 0 supplied.");
    ErroBanco::novo(format!("O SQLite recusou a operação: {original}.")).detalhe(original)
}

/// Erro de um comando: o do SQLite ainda sem tradução (para o aviso de exportação em andamento) ou um pronto.
enum Falha {
    Sqlite(rusqlite::Error),
    Pronta(ErroBanco),
}

impl From<rusqlite::Error> for Falha {
    fn from(e: rusqlite::Error) -> Self {
        Falha::Sqlite(e)
    }
}

fn executar(conn: &Connection, comando: &str, limite: usize, inicio: Instant) -> Result<ResultadoComando, Falha> {
    let mut stmt = conn.prepare(comando)?;
    if stmt.parameter_count() > 0 {
        return Err(Falha::Pronta(erro_parametros(stmt.parameter_count())));
    }
    let n = stmt.column_count();
    if n == 0 {
        let mudadas = stmt.execute([])?;
        return Ok(ResultadoComando::Alteracao {
            comando: comando.to_string(),
            linhas_afetadas: e_dml(comando).then_some(mudadas as u64),
            tempo_ms: tempo_ms(inicio),
        });
    }
    let colunas =
        nomes_das_colunas(conn, comando).unwrap_or_else(|| (0..n).map(|i| format!("coluna {}", i + 1)).collect());
    let mut linhas = Vec::new();
    let mut truncado = false;
    let mut brutas = stmt.query([])?;
    while let Some(r) = brutas.next()? {
        if linhas.len() == limite {
            truncado = true;
            break;
        }
        let mut valores = Vec::with_capacity(n);
        for i in 0..n {
            valores.push(Celula::de_ref(r.get_ref(i)?, Some(LIMITE_TEXTO)));
        }
        linhas.push(valores);
    }
    Ok(ResultadoComando::Linhas { comando: comando.to_string(), colunas, linhas, truncado, tempo_ms: tempo_ms(inicio) })
}

/// A parada da Consulta SQL ligada: tira o progress handler e a marca ao sair (também em erro ou pânico), sob
/// a trava da marca, antes de o `com_conexao` soltar a conexão. Depois disso o `interromper_consulta` não acha
/// a marca e não chama o interrupt: ele nunca alcança a próxima operação na conexão.
struct Parada<'a> {
    banco: &'a Banco,
    conn: &'a Connection,
    marca: Arc<AtomicBool>,
}

impl Parada<'_> {
    fn pedida(&self) -> bool {
        self.marca.load(Ordering::SeqCst)
    }
}

impl Drop for Parada<'_> {
    fn drop(&mut self) {
        let mut parada = self.banco.parada_consulta.lock();
        let _ = self.conn.progress_handler(0, None::<fn() -> bool>);
        *parada = None;
    }
}

/// O lugar de um `consultar` na fila da conexão: a marca fica registrada em `consultas_na_fila` desde antes de ele
/// pedir a conexão até ele pegá-la (a marca passa para a `parada_consulta`) ou desistir (erro antes do corpo).
struct Vez<'a> {
    banco: &'a Banco,
    marca: Arc<AtomicBool>,
}

impl<'a> Vez<'a> {
    fn entrar(banco: &'a Banco, marca: &Arc<AtomicBool>) -> Vez<'a> {
        banco.consultas_na_fila.lock().push(Arc::clone(marca));
        Vez { banco, marca: Arc::clone(marca) }
    }
}

impl Drop for Vez<'_> {
    fn drop(&mut self) {
        self.banco.consultas_na_fila.lock().retain(|m| !Arc::ptr_eq(m, &self.marca));
    }
}

/// O que o `interromper_consulta_detalhado` alcançou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupcao {
    /// Nenhum `consultar` em andamento nem esperando a vez: nada foi feito.
    Nenhuma,
    /// O `consultar` que tem a conexão: a marca foi ligada e o interrupt do SQLite, chamado.
    EmAndamento,
    /// Nenhum tem a conexão, mas um espera a vez dela (com uma gravação, uma exportação, um VACUUM): só a marca dele
    /// foi ligada, sem interrupt (a conexão não é dele). Ele sai interrompido, sem executar nada, quando a pegar.
    NaFila,
}

/// O comando cortado pela parada: a frase do DataBox; `detalhe` com o erro que o SQLite deu, quando ele foi
/// cortado no meio (sem detalhe, a parada chegou antes de ele começar).
fn interrompido(comando: &str, detalhe: Option<String>) -> ResultadoComando {
    ResultadoComando::Erro { comando: comando.to_string(), mensagem: MSG_INTERROMPIDA.to_string(), detalhe }
}

impl Banco {
    /// Executa um roteiro SQL comando a comando e para no primeiro erro (§4.5). `limite` (1 a 100.000) é o
    /// máximo de linhas guardadas por comando que devolve linhas (`truncado` diz se havia mais).
    ///
    /// Enquanto roda, [`Banco::interromper_consulta`] corta o comando em andamento (ele volta como
    /// `ResultadoComando::Erro` com [`MSG_INTERROMPIDA`]) e os seguintes não rodam. Enquanto espera a vez na
    /// conexão, o pedido de parada também vale: ao pegá-la, ele sai com o primeiro comando interrompido, sem
    /// executar nada.
    pub fn consultar(&self, sql: &str, limite: usize) -> Resultado<ResultadoConsulta> {
        let limite = limite.clamp(1, 100_000);
        let comandos = dividir_comandos(sql);
        let marca = Arc::new(AtomicBool::new(false));
        let _vez = Vez::entrar(self, &marca);
        self.com_conexao(|conn| {
            let versao = || conn.query_row("PRAGMA main.schema_version", [], |r| r.get::<_, i64>(0)).ok();
            let versao_antes = versao();
            let mudancas_antes = conn.total_changes();
            let mut resultados = Vec::with_capacity(comandos.len());
            {
                let parada = self.ligar_parada(conn, &marca)?;
                for comando in &comandos {
                    if parada.pedida() {
                        resultados.push(interrompido(comando, None));
                        break;
                    }
                    match executar(conn, comando, limite, Instant::now()) {
                        Ok(r) => resultados.push(r),
                        Err(falha) => {
                            let erro = match falha {
                                Falha::Sqlite(e) => self.erro_gravacao(e, None),
                                Falha::Pronta(e) => e,
                            };
                            resultados.push(if parada.pedida() {
                                interrompido(comando, Some(erro.detalhe.unwrap_or(erro.mensagem)))
                            } else {
                                ResultadoComando::Erro {
                                    comando: comando.clone(),
                                    mensagem: erro.mensagem,
                                    detalhe: erro.detalhe,
                                }
                            });
                            break;
                        }
                    }
                }
            }
            let versao_depois = versao().or(versao_antes);
            Ok(ResultadoConsulta {
                resultados,
                alterou_esquema: versao_depois != versao_antes,
                alterou_dados: conn.total_changes() != mudancas_antes,
                transacao_aberta: !conn.is_autocommit(),
            })
        })
    }

    /// Passa a marca do `consultar` (já registrada na fila, e talvez já ligada por um pedido que chegou enquanto ele
    /// esperava) para a posse da conexão, com o progress handler que a confere (a conexão já está presa pelo
    /// `com_conexao`). O handler cobre o instante entre a marca ligar e o comando começar no SQLite, em que o
    /// interrupt se perderia (o SQLite zera o pedido ao começar um comando com nenhum outro rodando).
    fn ligar_parada<'a>(&'a self, conn: &'a Connection, marca: &Arc<AtomicBool>) -> Resultado<Parada<'a>> {
        // sempre nesta ordem: a `parada_consulta`, depois a fila
        let mut atual = self.parada_consulta.lock();
        let vigia = Arc::clone(marca);
        conn.progress_handler(PASSOS_ENTRE_CONFERENCIAS, Some(move || vigia.load(Ordering::SeqCst)))
            .map_err(|e| traduzir(e, None))?;
        self.consultas_na_fila.lock().retain(|m| !Arc::ptr_eq(m, marca));
        *atual = Some(Arc::clone(marca));
        Ok(Parada { banco: self, conn, marca: Arc::clone(marca) })
    }

    /// Interrompe a Consulta SQL (o botão Interromper): corta a que tem a conexão (liga a marca dela e chama o
    /// interrupt do SQLite) ou, se nenhuma a tem e uma espera a vez, liga a marca da mais antiga, que sai
    /// interrompida sem executar. Devolve false, sem fazer nada, sem nenhum `consultar`. Nunca espera a trava do
    /// banco, e o interrupt só sai com a marca presente na posse da conexão, segurando a trava dela: nunca alcança
    /// uma gravação da grade, a Estrutura, uma exportação nem a próxima consulta.
    pub fn interromper_consulta(&self) -> bool {
        self.interromper_consulta_detalhado() != Interrupcao::Nenhuma
    }

    /// O [`Banco::interromper_consulta`] dizendo o que alcançou: a consulta em andamento ou uma que ainda espera.
    pub fn interromper_consulta_detalhado(&self) -> Interrupcao {
        let parada = self.parada_consulta.lock();
        if let Some(marca) = parada.as_ref() {
            marca.store(true, Ordering::SeqCst);
            self.interruptor.interrupt();
            return Interrupcao::EmAndamento;
        }
        match self.consultas_na_fila.lock().first() {
            Some(marca) => {
                marca.store(true, Ordering::SeqCst);
                Interrupcao::NaFila
            }
            None => Interrupcao::Nenhuma,
        }
    }

    /// Um `consultar` está de posse da conexão agora (rodando os comandos do roteiro dele).
    pub fn consulta_em_andamento(&self) -> bool {
        self.parada_consulta.lock().is_some()
    }

    /// Quantas Consultas SQL esperam a vez na conexão (para os testes).
    #[cfg(test)]
    pub(crate) fn consultas_esperando(&self) -> usize {
        self.consultas_na_fila.lock().len()
    }
}
