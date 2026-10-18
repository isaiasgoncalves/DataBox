//! Leitura em fluxo para a exportação: uma tabela (com busca, filtros, ordem e linhas marcadas) ou um SELECT.
//!
//! Lê numa conexão nova, só de leitura, para não segurar a interface; quando a sessão da conexão principal
//! enxerga algo que o arquivo ainda não tem (transação aberta na Consulta SQL; numa consulta, também tabela
//! temporária, banco anexado ou PRAGMA), lê pela conexão principal, com PRAGMA query_only ligado e segurando
//! a trava até o fim, para o arquivo sair igual ao que a tela mostra.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use comum::{Chave, ErroBanco, Resultado, Valor, q};
use rusqlite::{Connection, Rows, params_from_iter};

use crate::banco::{Banco, LIMITE_PARAMETROS, LOTE_LEITURA, PASSOS_ENTRE_CONFERENCIAS};
use crate::busca::{Filtro, montar_where};
use crate::consulta::erro_parametros;
use crate::erros::{Contexto, MSG_SO_SELECT, erro_fechado, traduzir};
use crate::ffi::nomes_das_colunas;
use crate::meta::{Meta, montar_ordem, onde_chave, select_sql};
use crate::sql::{comeca_com, dividir_comandos, limpar_sql};
use crate::tipos::{Direcao, TipoChave};

/// Quem pode mandar parar uma leitura de exportação: verdadeiro interrompe o comando em andamento (pelo
/// progress handler da conexão da exportação, conferido pela própria thread que lê), também no meio de uma
/// ordenação ou contagem longa.
pub type Parar = dyn Fn() -> bool + Send + Sync;

/// As linhas que a exportação lê, uma a uma: valores exatos, na ordem das colunas. Um erro do SQLite no meio
/// (inclusive a interrupção pedida por [`Parar`]) vem como `Err` já traduzido, com a frase "Não foi possível
/// ler os dados para exportar.", e o iterador termina.
pub type LinhasExportacao<'a> = dyn Iterator<Item = Resultado<Vec<Valor>>> + 'a;

/// O recorte de uma tabela para exportar (a `origem` `{"tipo": "tabela", …}`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FiltroExportacao {
    pub busca: Option<String>,
    pub filtros: Vec<Filtro>,
    pub ordem: Option<String>,
    pub direcao: Direcao,
    /// Só as linhas marcadas (Some(vec![]) não exporta nenhuma).
    pub chaves: Option<Vec<Chave>>,
}

const CONTEXTO_LEITURA: &str = "Não foi possível ler os dados para exportar.";

/// Lê as linhas em lotes.
struct LeitorLinhas<'s> {
    linhas: Rows<'s>,
    n: usize,
    lote: VecDeque<Vec<Valor>>,
    acabou: bool,
}

impl<'s> LeitorLinhas<'s> {
    fn novo(linhas: Rows<'s>, n: usize) -> Self {
        LeitorLinhas { linhas, n, lote: VecDeque::new(), acabou: false }
    }

    fn encher(&mut self, quantas: usize) -> rusqlite::Result<()> {
        for _ in 0..quantas {
            match self.linhas.next()? {
                Some(r) => {
                    let mut valores = Vec::with_capacity(self.n);
                    for i in 0..self.n {
                        valores.push(Valor::de_ref(r.get_ref(i)?));
                    }
                    self.lote.push_back(valores);
                }
                None => {
                    self.acabou = true;
                    break;
                }
            }
        }
        Ok(())
    }

    /// O primeiro passo do comando, antes de entregar o iterador: um erro aqui ainda é
    /// "não foi possível ler a tabela" ou "executar a consulta".
    fn preparar(&mut self) -> rusqlite::Result<()> {
        self.encher(1)
    }
}

impl Iterator for LeitorLinhas<'_> {
    type Item = Resultado<Vec<Valor>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.lote.is_empty()
            && !self.acabou
            && let Err(e) = self.encher(LOTE_LEITURA)
        {
            // o lote em que o erro aconteceu não é entregue
            self.acabou = true;
            self.lote.clear();
            return Some(Err(traduzir(e, Some(CONTEXTO_LEITURA))));
        }
        self.lote.pop_front().map(Ok)
    }
}

/// Volta a conexão principal ao normal ao fim da exportação (também em erro ou pânico).
struct RestaurarPrincipal<'a> {
    conn: &'a Connection,
}

impl Drop for RestaurarPrincipal<'_> {
    fn drop(&mut self) {
        let _ = self.conn.progress_handler(0, None::<fn() -> bool>);
        if let Err(e) = self.conn.execute_batch("PRAGMA query_only = OFF") {
            log::warn!("Não foi possível religar a gravação depois de exportar: {e}");
        }
    }
}

/// Conexão própria da exportação: fecha e só então desconta do contador.
struct LeituraPropria<'a> {
    conn: Option<Connection>,
    contador: &'a AtomicUsize,
}

impl Drop for LeituraPropria<'_> {
    fn drop(&mut self) {
        drop(self.conn.take());
        self.contador.fetch_sub(1, Ordering::SeqCst);
    }
}

fn ligar_parar(conn: &Connection, parar: Option<Arc<Parar>>) -> Resultado<()> {
    if let Some(p) = parar {
        conn.progress_handler(PASSOS_ENTRE_CONFERENCIAS, Some(move || p())).sem_ctx()?;
    }
    Ok(())
}

/// A conexão principal enxerga algo que uma conexão nova não veria?
///
/// Transação aberta na Consulta SQL (mudanças ainda fora do arquivo) e, para uma consulta livre, tabela
/// temporária, banco anexado (ATTACH) ou PRAGMA, que vale para a sessão.
fn sessao_propria(conn: &Connection, sql: Option<&str>) -> bool {
    if !conn.is_autocommit() {
        return true;
    }
    let Some(sql) = sql else {
        return false;
    };
    if comeca_com(&limpar_sql(sql), &["PRAGMA"]) {
        return true;
    }
    let temporarias = conn.query_row("SELECT count(*) FROM temp.sqlite_schema", [], |r| r.get::<_, i64>(0));
    if temporarias.is_ok_and(|n| n > 0) {
        return true;
    }
    (|| {
        let mut stmt = conn.prepare("SELECT name FROM pragma_database_list")?;
        let mut r = stmt.query([])?;
        while let Some(l) = r.next()? {
            let nome = crate::valores::texto_de(l.get_ref(0)?);
            if nome != "main" && nome != "temp" {
                return Ok(true);
            }
        }
        Ok::<_, rusqlite::Error>(false)
    })()
    .unwrap_or(false)
}

impl Banco {
    /// Roda `f` na conexão de exportação: a principal (segurando a trava até o fim, com PRAGMA query_only) ou
    /// uma nova, só de leitura. `parar` vira o progress handler dessa conexão.
    fn ler_para_exportar<R>(
        &self,
        principal: bool,
        parar: Option<Arc<Parar>>,
        f: impl FnOnce(&Connection) -> Resultado<R>,
    ) -> Resultado<R> {
        if principal {
            let guarda = self.conn.lock();
            let celula = guarda.try_borrow().map_err(|_| erro_fechado())?;
            let conn = celula.as_ref().ok_or_else(erro_fechado)?;
            conn.execute_batch("PRAGMA query_only = ON").sem_ctx()?;
            let _restaurar = RestaurarPrincipal { conn };
            ligar_parar(conn, parar)?;
            return f(conn);
        }
        let conn = self.conexao_leitura()?;
        self.exportacoes.fetch_add(1, Ordering::SeqCst);
        let propria = LeituraPropria { conn: Some(conn), contador: &self.exportacoes };
        let conn = propria.conn.as_ref().ok_or_else(erro_fechado)?;
        ligar_parar(conn, parar)?;
        f(conn)
    }

    /// Descrição, WHERE e se a leitura vai pela conexão principal.
    fn preparar_tabela(&self, nome: &str, filtro: &FiltroExportacao) -> Resultado<(Meta, String, Vec<Valor>, bool)> {
        let (meta, principal) = self.com_conexao(|conn| Ok((self.meta(conn, nome)?, sessao_propria(conn, None))))?;
        let (mut where_sql, mut params) = montar_where(&meta, filtro.busca.as_deref(), &filtro.filtros)?;
        if let Some(chaves) = &filtro.chaves {
            (where_sql, params) = restringir_chaves(&meta, chaves, where_sql, params)?;
        }
        Ok((meta, where_sql, params, principal))
    }

    /// Lê a tabela para exportar: `consumir` recebe as colunas visíveis e o iterador das linhas (valores
    /// exatos). O iterador só vive dentro de `consumir` (a trava e o comando preparado ficam vivos enquanto ele
    /// roda). `parar` verdadeiro interrompe a leitura (o iterador devolve `Err`).
    pub fn iterar_tabela<R>(
        &self,
        nome: &str,
        filtro: &FiltroExportacao,
        parar: Option<Arc<Parar>>,
        consumir: impl FnOnce(&[String], &mut LinhasExportacao<'_>) -> Resultado<R>,
    ) -> Resultado<R> {
        let (meta, where_sql, params, principal) = self.preparar_tabela(nome, filtro)?;
        let (ordem_sql, _) = montar_ordem(&meta, filtro.ordem.as_deref(), filtro.direcao)?;
        let sql = format!("{}{where_sql}{ordem_sql}", select_sql(&meta, false));
        let colunas: Vec<String> = meta.visiveis.iter().map(|c| c.nome.clone()).collect();
        self.ler_para_exportar(principal, parar, |conn| {
            let contexto = "Não foi possível ler a tabela para exportar.";
            let mut stmt = conn.prepare(&sql).ctx(contexto)?;
            let n = stmt.column_count();
            let linhas = stmt.query(params_from_iter(&params)).ctx(contexto)?;
            let mut leitor = LeitorLinhas::novo(linhas, n);
            leitor.preparar().ctx(contexto)?;
            consumir(&colunas, &mut leitor)
        })
    }

    /// Quantas linhas [`Banco::iterar_tabela`] entregaria com o mesmo filtro, para o progresso da exportação.
    ///
    /// Conta como ela lê, sem segurar a interface. Sem busca, filtros nem chaves é o count(*) da tabela, que o
    /// SQLite faz sem olhar as linhas. `Ok(None)` quando não deu para contar: `parar()` ficou verdadeiro, passou
    /// de `prazo` ou o SQLite recusou (tabela virtual sem contagem, por exemplo). Tabela que não existe ou
    /// filtro inválido → `Err`, como em `iterar_tabela`.
    pub fn contar_para_exportar(
        &self,
        nome: &str,
        filtro: &FiltroExportacao,
        parar: Option<Arc<Parar>>,
        prazo: Option<Duration>,
    ) -> Resultado<Option<u64>> {
        let (meta, where_sql, params, principal) = self.preparar_tabela(nome, filtro)?;
        let limite = prazo.map(|p| Instant::now() + p);
        let desistir: Arc<Parar> =
            Arc::new(move || parar.as_ref().is_some_and(|p| p()) || limite.is_some_and(|l| Instant::now() > l));
        self.ler_para_exportar(principal, Some(desistir), |conn| {
            let sql = format!("SELECT count(*) FROM main.{}{where_sql}", q(&meta.nome));
            Ok(conn
                .query_row(&sql, params_from_iter(&params), |r| r.get::<_, i64>(0))
                .ok()
                .map(|n| u64::try_from(n).unwrap_or(0)))
        })
    }

    /// Resultado de um único SELECT para exportar; o que grava é recusado ("Só dá para exportar o resultado
    /// de uma consulta SELECT."). `consumir` e `parar` como em [`Banco::iterar_tabela`]; as colunas são as do
    /// resultado.
    pub fn iterar_consulta<R>(
        &self,
        sql: &str,
        parar: Option<Arc<Parar>>,
        consumir: impl FnOnce(&[String], &mut LinhasExportacao<'_>) -> Resultado<R>,
    ) -> Resultado<R> {
        let comandos = dividir_comandos(sql);
        let [comando] = comandos.as_slice() else {
            return Err(ErroBanco::novo(if comandos.is_empty() {
                "Nenhuma consulta para exportar."
            } else {
                "Para exportar, a consulta precisa ser um único comando SELECT."
            }));
        };
        let principal = self.com_conexao(|conn| Ok(sessao_propria(conn, Some(comando))))?;
        self.ler_para_exportar(principal, parar, |conn| {
            let recusa = |e: rusqlite::Error| {
                let texto = e.to_string().to_lowercase();
                if texto.contains("readonly") || texto.contains("read-only") {
                    ErroBanco::novo(MSG_SO_SELECT).detalhe(e.to_string())
                } else {
                    traduzir(e, Some("Não foi possível executar a consulta."))
                }
            };
            let mut stmt = conn.prepare(comando).map_err(recusa)?;
            let n = stmt.column_count();
            if n == 0 {
                // nada é executado: um comando sem resultado só pode gravar ou mudar a sessão
                return Err(ErroBanco::novo(MSG_SO_SELECT));
            }
            if stmt.parameter_count() > 0 {
                return Err(
                    erro_parametros(stmt.parameter_count()).com_contexto("Não foi possível executar a consulta.")
                );
            }
            let colunas = nomes_das_colunas(conn, comando)
                .unwrap_or_else(|| (0..n).map(|i| format!("coluna {}", i + 1)).collect());
            let linhas = stmt.query([]).map_err(recusa)?;
            let mut leitor = LeitorLinhas::novo(linhas, n);
            leitor.preparar().map_err(recusa)?;
            consumir(&colunas, &mut leitor)
        })
    }
}

/// Junta ao WHERE a condição "só as linhas marcadas".
fn restringir_chaves(
    meta: &Meta,
    chaves: &[Chave],
    where_sql: String,
    params: Vec<Valor>,
) -> Resultado<(String, Vec<Valor>)> {
    let Some(tipo) = &meta.chave else {
        return Err(ErroBanco::novo(format!("{} não tem identidade de linha; exporte sem marcar linhas.", meta.nome)));
    };
    let (condicao, mais) = if chaves.is_empty() {
        ("0".to_string(), Vec::new())
    } else if let TipoChave::Rowid { alias } = tipo {
        // json_each aceita qualquer quantidade de rowids num parâmetro só
        let mut ids = Vec::with_capacity(chaves.len());
        for chave in chaves {
            let (_, p) = onde_chave(meta, chave)?;
            if let Some(Valor::Inteiro(i)) = p.first() {
                ids.push(i.to_string());
            }
        }
        (format!("{alias} IN (SELECT value FROM json_each(?))"), vec![Valor::Texto(format!("[{}]", ids.join(", ")))])
    } else {
        let n_colunas = match tipo {
            TipoChave::Pk { colunas } => colunas.len(),
            TipoChave::Rowid { .. } => 1,
        };
        if chaves.len() * n_colunas > LIMITE_PARAMETROS {
            return Err(ErroBanco::novo(
                "Linhas marcadas demais para exportar de uma vez; use um filtro em vez de marcar.",
            ));
        }
        let mut partes = Vec::with_capacity(chaves.len());
        let mut ps = Vec::new();
        for chave in chaves {
            let (w, p) = onde_chave(meta, chave)?;
            partes.push(format!("({w})"));
            ps.extend(p);
        }
        (partes.join(" OR "), ps)
    };
    if where_sql.is_empty() {
        return Ok((format!(" WHERE ({condicao})"), mais));
    }
    let mut todos = params;
    todos.extend(mais);
    Ok((format!("{where_sql} AND ({condicao})"), todos))
}
