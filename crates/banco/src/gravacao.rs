//! Gravação de linhas pela grade: inserir, atualizar e excluir, tudo parametrizado e tudo ou nada.

use std::sync::atomic::Ordering;

use comum::{Chave, Entrada, ErroBanco, Resultado, Valor, q};
use rusqlite::{Connection, params_from_iter};

use crate::banco::Banco;
use crate::dados::ler_valores;
use crate::erros::{MSG_EXPORTANDO, MSG_SALVAR, MSG_SOMENTE_LEITURA, e_trava_sqlite, traduzir};
use crate::meta::{MODULOS_GRAVAVEIS, Meta, coluna_por_nome, expr_identidade, montar_chave, onde_chave};
use crate::tipos::{LinhaGravada, TipoChave, TipoObjeto};
use crate::valores::{Coagido, coagir};

/// Modo de `preparar_valores`: o valor padrão só vale ao inserir.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    Inserir,
    Atualizar,
}

/// Desfaz o SAVEPOINT se a gravação sair no meio (erro ou pânico) sem ter chegado ao RELEASE.
struct Desfazer<'a> {
    conn: &'a Connection,
    nome: &'a str,
    externa: bool,
    ativo: bool,
}

impl Drop for Desfazer<'_> {
    fn drop(&mut self) {
        if !self.ativo {
            return;
        }
        let _ = self.conn.execute_batch(&format!("ROLLBACK TO {}", self.nome));
        let _ = self.conn.execute_batch(&format!("RELEASE {}", self.nome));
        if !self.externa && !self.conn.is_autocommit() && self.conn.execute_batch("ROLLBACK").is_err() {
            log::warn!("Não foi possível desfazer o SAVEPOINT {}.", self.nome);
        }
    }
}

/// Tudo ou nada: SAVEPOINT, e RELEASE no fim; em QUALQUER erro (também os que não são do SQLite), ROLLBACK TO
/// e RELEASE.
///
/// SAVEPOINT em vez de BEGIN funciona também dentro de uma transação aberta na Consulta SQL. Um erro que
/// deixasse o SAVEPOINT aberto o transformaria numa transação implícita, e tudo o que se gravasse depois
/// ficaria fora do arquivo até ser desfeito ao fechar o banco.
pub(crate) fn savepoint<R>(
    conn: &Connection,
    nome: &str,
    erro_sqlite: impl Fn(rusqlite::Error) -> ErroBanco,
    f: impl FnOnce() -> Resultado<R>,
) -> Resultado<R> {
    let externa = !conn.is_autocommit();
    conn.execute_batch(&format!("SAVEPOINT {nome}")).map_err(&erro_sqlite)?;
    let mut desfazer = Desfazer { conn, nome, externa, ativo: true };
    let r = f()?;
    conn.execute_batch(&format!("RELEASE {nome}")).map_err(&erro_sqlite)?;
    desfazer.ativo = false;
    Ok(r)
}

fn chave_ambigua(n: usize, consequencia: &str) -> ErroBanco {
    ErroBanco::com_status(
        format!(
            "A chave da linha corresponde a {n} linhas, não a uma só; {consequencia}. Use a Consulta SQL para \
             mudar essas linhas."
        ),
        409,
    )
}

impl Banco {
    /// `traduzir_erro` para gravações; com uma exportação lendo o banco, a trava é dela, não de outro programa.
    pub(crate) fn erro_gravacao(&self, e: rusqlite::Error, contexto: Option<&str>) -> ErroBanco {
        let trava = e_trava_sqlite(&e);
        let erro = traduzir(e, contexto);
        if trava && self.exportacoes.load(Ordering::SeqCst) > 0 {
            let mensagem = match contexto {
                Some(c) => format!("{c} {MSG_EXPORTANDO}"),
                None => MSG_EXPORTANDO.to_string(),
            };
            let mut exportando = ErroBanco::com_status(mensagem, 409);
            exportando.detalhe = erro.detalhe;
            return exportando;
        }
        erro
    }

    fn exigir_gravacao(&self, meta: &Meta) -> Resultado<()> {
        if self.somente_leitura {
            return Err(ErroBanco::com_status(MSG_SOMENTE_LEITURA, 403));
        }
        if meta.tipo == TipoObjeto::View {
            return Err(ErroBanco::com_status(
                format!("{} é uma visão; os dados dela vêm de outras tabelas e não são editados aqui.", meta.nome),
                403,
            ));
        }
        if meta.sistema && !meta.nome.eq_ignore_ascii_case("sqlite_sequence") {
            return Err(ErroBanco::com_status(
                format!("{} é uma tabela interna do SQLite e não pode ser editada.", meta.nome),
                403,
            ));
        }
        if meta.tipo == TipoObjeto::Virtual && !meta.modulo.as_deref().is_some_and(|m| MODULOS_GRAVAVEIS.contains(&m)) {
            return Err(ErroBanco::com_status(
                format!(
                    "A tabela virtual {} (módulo {}) não aceita edição.",
                    meta.nome,
                    meta.modulo.as_deref().unwrap_or("desconhecido")
                ),
                403,
            ));
        }
        Ok(())
    }

    fn preparar_valores(
        &self,
        meta: &Meta,
        valores: &[(String, Entrada)],
        modo: Modo,
    ) -> Resultado<(Vec<String>, Vec<Valor>)> {
        let mut cols: Vec<String> = Vec::new();
        let mut params: Vec<Valor> = Vec::new();
        for (nome_col, entrada) in valores {
            let col = coluna_por_nome(&meta.colunas, nome_col)
                .ok_or_else(|| ErroBanco::novo(format!("A coluna não existe em {}: {nome_col}.", meta.nome)))?;
            if col.gerada || col.oculta == 1 {
                continue; // colunas geradas (e ocultas de tabela virtual) nunca são gravadas diretamente
            }
            let v = match coagir(entrada, col.afinidade)? {
                Coagido::Manter => continue,
                Coagido::Padrao if modo == Modo::Atualizar => {
                    return Err(ErroBanco::novo(format!(
                        "O valor padrão só vale ao inserir uma linha; escolha um valor para {} ou mantenha o atual.",
                        col.nome
                    )));
                }
                Coagido::Padrao => continue,
                Coagido::Valor(v) => v,
            };
            match cols.iter().position(|c| c == &col.nome) {
                Some(i) => {
                    if let Some(p) = params.get_mut(i) {
                        *p = v;
                    }
                }
                None => {
                    cols.push(col.nome.clone());
                    params.push(v);
                }
            }
        }
        Ok((cols, params))
    }

    fn usa_returning(meta: &Meta) -> bool {
        meta.tipo != TipoObjeto::Virtual && meta.chave.is_some()
    }

    fn resposta_gravacao(&self, conn: &Connection, meta: &Meta, chave: Option<Chave>) -> Resultado<LinhaGravada> {
        let Some(chave) = chave else {
            return Ok(LinhaGravada { chave: None, linha: None });
        };
        match self.ler_linha(conn, meta, &chave, Some(comum::LIMITE_TEXTO))? {
            // um gatilho pode ter apagado a linha logo depois de gravá-la
            None => Ok(LinhaGravada { chave: Some(chave), linha: None }),
            Some((ident, celulas)) => Ok(LinhaGravada { chave: montar_chave(meta, &ident), linha: Some(celulas) }),
        }
    }

    /// Insere uma linha e a devolve relida do banco (com DEFAULT e gatilhos aplicados).
    ///
    /// [`Entrada::Padrao`] deixa a coluna fora do INSERT; colunas geradas são ignoradas. Uma linha descartada
    /// sem erro (ON CONFLICT IGNORE, RAISE(IGNORE) num gatilho) → 409 explicando.
    pub fn inserir(&self, nome: &str, valores: &[(String, Entrada)]) -> Resultado<LinhaGravada> {
        self.com_conexao(|conn| {
            let meta = self.meta(conn, nome)?;
            self.exigir_gravacao(&meta)?;
            let (cols, params) = self.preparar_valores(&meta, valores, Modo::Inserir)?;
            let alvo = format!("main.{}", q(&meta.nome));
            let sql = if cols.is_empty() {
                format!("INSERT INTO {alvo} DEFAULT VALUES")
            } else {
                let nomes = cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ");
                let marcas = vec!["?"; cols.len()].join(", ");
                format!("INSERT INTO {alvo} ({nomes}) VALUES ({marcas})")
            };
            let erro = |e: rusqlite::Error| self.erro_gravacao(e, Some(MSG_SALVAR));
            let chave = if Banco::usa_returning(&meta) {
                // lê todas as linhas: o comando precisa terminar para o autocommit gravar e soltar o arquivo
                let sql = format!("{sql} RETURNING {}", expr_identidade(&meta).join(", "));
                let rs = ler_valores(conn, &sql, &params).map_err(erro)?;
                let Some(primeira) = rs.first() else {
                    // UNIQUE … ON CONFLICT IGNORE ou gatilho com RAISE(IGNORE) descartou a linha sem erro
                    return Err(ErroBanco::com_status(
                        format!(
                            "{MSG_SALVAR} O banco ignorou a linha nova: uma restrição com ON CONFLICT IGNORE (valor \
                             repetido) ou um gatilho da tabela descartou o comando. Nada foi gravado."
                        ),
                        409,
                    ));
                };
                montar_chave(&meta, primeira)
            } else {
                conn.prepare(&sql).and_then(|mut s| s.execute(params_from_iter(&params))).map_err(erro)?;
                match meta.chave {
                    Some(TipoChave::Rowid { .. }) => Some(Chave::Rowid(conn.last_insert_rowid())),
                    _ => None,
                }
            };
            self.resposta_gravacao(conn, &meta, chave)
        })
    }

    /// Atualiza uma linha; se a chave primária mudou, a resposta traz a chave nova.
    ///
    /// [`Entrada::Manter`] deixa a coluna fora do UPDATE; [`Entrada::Padrao`] → 400. Roda num SAVEPOINT: a
    /// chave que casasse com mais de uma linha não grava nada (409). Linha que sumiu → 404; alteração
    /// descartada sem erro (ON CONFLICT IGNORE, RAISE(IGNORE)) → 409.
    pub fn atualizar(&self, nome: &str, chave: &Chave, valores: &[(String, Entrada)]) -> Resultado<LinhaGravada> {
        self.com_conexao(|conn| {
            let meta = self.meta(conn, nome)?;
            self.exigir_gravacao(&meta)?;
            let (where_sql, wparams) = onde_chave(&meta, chave)?;
            let (cols, params) = self.preparar_valores(&meta, valores, Modo::Atualizar)?;
            if cols.is_empty() {
                let (ident, _) = self.ler_linha(conn, &meta, chave, Some(0))?.ok_or_else(Banco::linha_sumiu)?;
                return self.resposta_gravacao(conn, &meta, montar_chave(&meta, &ident));
            }
            let sets = cols.iter().map(|c| format!("{} = ?", q(c))).collect::<Vec<_>>().join(", ");
            let sql = format!("UPDATE main.{} SET {sets} WHERE {where_sql}", q(&meta.nome));
            let mut todos = params;
            todos.extend(wparams);
            let erro = |e: rusqlite::Error| self.erro_gravacao(e, Some(MSG_SALVAR));
            let (afetadas, rs) = savepoint(conn, "editor_atualizar", erro, || {
                let (afetadas, rs) = if Banco::usa_returning(&meta) {
                    let sql = format!("{sql} RETURNING {}", expr_identidade(&meta).join(", "));
                    let rs = ler_valores(conn, &sql, &todos).map_err(erro)?;
                    (rs.len(), Some(rs))
                } else {
                    let n = conn.prepare(&sql).and_then(|mut s| s.execute(params_from_iter(&todos))).map_err(erro)?;
                    (n, None)
                };
                if afetadas > 1 {
                    return Err(chave_ambigua(afetadas, "nada foi salvo"));
                }
                Ok((afetadas, rs))
            })?;
            if afetadas == 0 {
                if self.ler_linha(conn, &meta, chave, Some(0))?.is_some() {
                    return Err(ErroBanco::com_status(
                        format!(
                            "{MSG_SALVAR} O banco ignorou a alteração: uma restrição com ON CONFLICT IGNORE (valor \
                             repetido) ou um gatilho da tabela descartou o comando. A linha continua como estava."
                        ),
                        409,
                    ));
                }
                return Err(Banco::linha_sumiu());
            }
            let nova = match rs.as_deref().and_then(<[_]>::first) {
                Some(primeira) => montar_chave(&meta, primeira),
                None => Some(chave.clone()),
            };
            self.resposta_gravacao(conn, &meta, nova)
        })
    }

    /// Exclui várias linhas numa transação só (tudo ou nada); devolve quantas saíram. Chave que casasse com
    /// mais de uma linha → 409 e nada é excluído.
    pub fn excluir(&self, nome: &str, chaves: &[Chave]) -> Resultado<usize> {
        self.com_conexao(|conn| {
            let meta = self.meta(conn, nome)?;
            self.exigir_gravacao(&meta)?;
            let condicoes = chaves.iter().map(|c| onde_chave(&meta, c)).collect::<Resultado<Vec<_>>>()?;
            if condicoes.is_empty() {
                return Ok(0);
            }
            let alvo = format!("main.{}", q(&meta.nome));
            let erro = |e: rusqlite::Error| self.erro_gravacao(e, Some("Não foi possível excluir as linhas."));
            savepoint(conn, "editor_excluir", erro, || {
                let mut removidas = 0;
                for (where_sql, params) in &condicoes {
                    let n = conn
                        .prepare(&format!("DELETE FROM {alvo} WHERE {where_sql}"))
                        .and_then(|mut s| s.execute(params_from_iter(params)))
                        .map_err(erro)?;
                    if n > 1 {
                        return Err(chave_ambigua(n, "nenhuma linha foi excluída"));
                    }
                    removidas += n;
                }
                Ok(removidas)
            })
        })
    }
}
