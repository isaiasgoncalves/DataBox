//! Leituras da interface: objetos do banco, estrutura de uma tabela, contagem e páginas da grade.

use std::collections::BTreeMap;
use std::time::Instant;

use comum::{Celula, Chave, ErroBanco, LIMITE_TEXTO, Resultado, Valor, q};
use rusqlite::{Connection, params_from_iter};

use crate::banco::{Banco, MAXIMO_PAGINA};
use crate::busca::{Filtro, montar_where};
use crate::erros::Contexto;
use crate::meta::{
    Meta, classificar, expr_identidade, indice_comeca_por, montar_chave, montar_ordem, nenhuma_linha, onde_chave,
    pagina_sql, select_sql,
};
use crate::sql::tem_palavra;
use crate::tipos::{
    ChaveEstrangeira, ColunaIndice, ColunaInfo, Contagem, FkColuna, GatilhoInfo, GatilhoResumo, IndiceInfo,
    IndiceResumo, LinhaCompleta, LinhaPagina, ObjetosInfo, OrigemIndice, Pagina, PedidoPagina, SistemaResumo,
    TabelaInfo, TabelaResumo, TipoObjeto, VisaoResumo,
};
use crate::valores::{contagem_de, inteiro_de, texto_de, texto_do_valor, texto_opcional};

/// Milissegundos com uma casa (arredondado a 0,1 ms).
pub(crate) fn tempo_ms(inicio: Instant) -> f64 {
    (inicio.elapsed().as_secs_f64() * 10_000.0).round() / 10.0
}

fn contar(conn: &Connection, nome: &str) -> rusqlite::Result<u64> {
    conn.query_row(&format!("SELECT count(*) FROM main.{}", q(nome)), [], |r| r.get_ref(0).map(contagem_de))
}

fn contar_ou_nada(conn: &Connection, nome: &str) -> Option<u64> {
    contar(conn, nome).ok()
}

fn colunas_visiveis(conn: &Connection, nome: &str) -> Option<u64> {
    conn.query_row("SELECT count(*) FROM pragma_table_xinfo(?, 'main') WHERE hidden != 1", [nome], |r| {
        r.get_ref(0).map(contagem_de)
    })
    .ok()
}

/// Lê todas as linhas de uma consulta como valores exatos.
pub(crate) fn ler_valores(conn: &Connection, sql: &str, params: &[Valor]) -> rusqlite::Result<Vec<Vec<Valor>>> {
    let mut stmt = conn.prepare(sql)?;
    let n = stmt.column_count();
    let mut linhas = stmt.query(params_from_iter(params))?;
    let mut saida = Vec::new();
    while let Some(r) = linhas.next()? {
        let mut valores = Vec::with_capacity(n);
        for i in 0..n {
            valores.push(Valor::de_ref(r.get_ref(i)?));
        }
        saida.push(valores);
    }
    Ok(saida)
}

/// A primeira linha (com identidade e células) de uma consulta, ou None.
fn ler_primeira(
    conn: &Connection,
    sql: &str,
    params: &[Valor],
    n_ident: usize,
    limite: Option<usize>,
) -> rusqlite::Result<Option<(Vec<Valor>, Vec<Celula>)>> {
    let mut stmt = conn.prepare(sql)?;
    let n = stmt.column_count();
    let mut linhas = stmt.query(params_from_iter(params))?;
    let Some(r) = linhas.next()? else {
        return Ok(None);
    };
    let mut ident = Vec::with_capacity(n_ident);
    let mut celulas = Vec::with_capacity(n.saturating_sub(n_ident));
    for i in 0..n {
        let v = r.get_ref(i)?;
        if i < n_ident {
            ident.push(Valor::de_ref(v));
        } else {
            celulas.push(Celula::de_ref(v, limite));
        }
    }
    Ok(Some((ident, celulas)))
}

impl Banco {
    /// ObjetosInfo (§4.2): tabelas do usuário, visões, tabelas do sistema, índices e gatilhos.
    pub fn objetos(&self) -> Resultado<ObjetosInfo> {
        self.lendo(|conn| {
            (|| {
                let cls = classificar(conn)?;
                let contar = self.pode_contar();
                let linhas = |nome: &str| if contar { contar_ou_nada(conn, nome) } else { None };
                let tabelas = cls
                    .tabelas
                    .iter()
                    .map(|t| TabelaResumo {
                        nome: t.nome.clone(),
                        tipo: t.tipo,
                        sem_rowid: t.wr,
                        linhas: linhas(&t.nome),
                        colunas: colunas_visiveis(conn, &t.nome),
                    })
                    .collect();
                let visoes = cls
                    .visoes
                    .iter()
                    .map(|t| VisaoResumo { nome: t.nome.clone(), colunas: colunas_visiveis(conn, &t.nome) })
                    .collect();
                let sistema = cls
                    .sistema
                    .iter()
                    .map(|t| SistemaResumo {
                        nome: t.nome.clone(),
                        tipo: t.tipo,
                        linhas: linhas(&t.nome),
                        colunas: colunas_visiveis(conn, &t.nome),
                    })
                    .collect();
                let mut indices = Vec::new();
                let brutos = {
                    let mut stmt = conn.prepare(
                        "SELECT name, tbl_name, sql FROM main.sqlite_schema WHERE type = 'index' ORDER BY name",
                    )?;
                    let mut r = stmt.query([])?;
                    let mut saida = Vec::new();
                    while let Some(l) = r.next()? {
                        saida.push((texto_de(l.get_ref(0)?), texto_de(l.get_ref(1)?), texto_opcional(l.get_ref(2)?)));
                    }
                    saida
                };
                for (nome, tabela, sql) in brutos {
                    let unico = unico_na_lista(conn, &tabela, &nome).unwrap_or(false);
                    indices.push(IndiceResumo { nome, tabela, unico, automatico: sql.is_none() });
                }
                let gatilhos = {
                    let mut stmt = conn.prepare(
                        "SELECT name, tbl_name FROM main.sqlite_schema WHERE type = 'trigger' ORDER BY name",
                    )?;
                    let mut r = stmt.query([])?;
                    let mut saida = Vec::new();
                    while let Some(l) = r.next()? {
                        saida.push(GatilhoResumo { nome: texto_de(l.get_ref(0)?), tabela: texto_de(l.get_ref(1)?) });
                    }
                    saida
                };
                Ok(ObjetosInfo { tabelas, visoes, sistema, indices, gatilhos })
            })()
            .ctx("Não foi possível listar os objetos do banco.")
        })
    }

    /// Linhas da tabela que passam na busca e nos filtros (mesma semântica de [`Banco::linhas`]).
    pub fn contar_linhas(
        &self,
        tabela: &str,
        busca: Option<&str>,
        filtros: &[Filtro],
        leitura: Option<&str>,
    ) -> Resultado<u64> {
        Ok(self.contagem(tabela, busca, filtros, leitura)?.linhas)
    }

    /// `{"linhas", "tempo_ms"}` da contagem da grade (§4.3); `leitura` como em [`PedidoPagina::leitura`]
    /// (`"contagem:<tabela>"`).
    pub fn contagem(
        &self,
        tabela: &str,
        busca: Option<&str>,
        filtros: &[Filtro],
        leitura: Option<&str>,
    ) -> Resultado<Contagem> {
        self.lendo_substituivel(leitura, |conn| {
            let inicio = Instant::now();
            let meta = self.meta(conn, tabela)?;
            let (where_sql, params) = montar_where(&meta, busca, filtros)?;
            let linhas = conn
                .query_row(
                    &format!("SELECT count(*) FROM main.{}{where_sql}", q(&meta.nome)),
                    params_from_iter(&params),
                    |r| r.get_ref(0).map(contagem_de),
                )
                .ctx("Não foi possível contar as linhas.")?;
            Ok(Contagem { linhas, tempo_ms: tempo_ms(inicio) })
        })
    }

    /// TabelaInfo (§4.3) sem `definicoes`/`restricoes` (o serviço junta a partir do crate esquema).
    pub fn tabela_info(&self, nome: &str) -> Resultado<TabelaInfo> {
        self.lendo(|conn| {
            let meta = self.meta(conn, nome)?;
            self.tabela_info_de(conn, &meta).ctx(&format!("Não foi possível ler a estrutura de {}.", meta.nome))
        })
    }

    fn tabela_info_de(&self, conn: &Connection, meta: &Meta) -> rusqlite::Result<TabelaInfo> {
        let nome = &meta.nome;
        let tipo = meta.tipo;
        let mut indices = Vec::new();
        if tipo != TipoObjeto::View {
            let lista = {
                let mut stmt =
                    conn.prepare("SELECT name, \"unique\", origin, partial FROM pragma_index_list(?, 'main')")?;
                let mut r = stmt.query([nome])?;
                let mut saida = Vec::new();
                while let Some(l) = r.next()? {
                    saida.push((
                        texto_de(l.get_ref(0)?),
                        inteiro_de(l.get_ref(1)?) != 0,
                        texto_de(l.get_ref(2)?),
                        inteiro_de(l.get_ref(3)?) != 0,
                    ));
                }
                saida
            };
            for (nome_idx, unico, origem, parcial) in lista {
                let mut colunas = Vec::new();
                let mut stmt =
                    conn.prepare("SELECT cid, name, \"desc\", \"key\" FROM pragma_index_xinfo(?, 'main')")?;
                let mut r = stmt.query([&nome_idx])?;
                while let Some(l) = r.next()? {
                    if inteiro_de(l.get_ref(3)?) == 0 {
                        continue;
                    }
                    let nome_col = match inteiro_de(l.get_ref(0)?) {
                        -2 => "(expressão)".to_string(),
                        -1 => "rowid".to_string(),
                        _ => texto_de(l.get_ref(1)?),
                    };
                    colunas.push(ColunaIndice { nome: nome_col, desc: inteiro_de(l.get_ref(2)?) != 0 });
                }
                drop(r);
                drop(stmt);
                let sql = conn
                    .query_row(
                        "SELECT sql FROM main.sqlite_schema WHERE type = 'index' AND name = ?",
                        [&nome_idx],
                        |r| r.get_ref(0).map(texto_opcional),
                    )
                    .or_else(nenhuma_linha)?;
                indices.push(IndiceInfo {
                    nome: nome_idx,
                    unico,
                    origem: OrigemIndice::de_texto(&origem),
                    parcial,
                    colunas,
                    sql,
                });
            }
            indices.sort_by_cached_key(|i| crate::meta::dobrar_caixa(&i.nome));
        }

        let mut fks: BTreeMap<i64, ChaveEstrangeira> = BTreeMap::new();
        if matches!(tipo, TipoObjeto::Table | TipoObjeto::Shadow) {
            let mut stmt = conn.prepare(
                "SELECT id, \"table\", \"from\", \"to\", on_update, on_delete FROM pragma_foreign_key_list(?, 'main')",
            )?;
            let mut r = stmt.query([nome])?;
            while let Some(l) = r.next()? {
                let id = inteiro_de(l.get_ref(0)?);
                let fk = fks.entry(id).or_insert_with(|| ChaveEstrangeira {
                    id,
                    colunas: Vec::new(),
                    tabela: String::new(),
                    colunas_ref: Vec::new(),
                    ao_excluir: String::new(),
                    ao_atualizar: String::new(),
                });
                if fk.colunas.is_empty() {
                    fk.tabela = texto_de(l.get_ref(1)?);
                    fk.ao_atualizar = texto_de(l.get_ref(4)?);
                    fk.ao_excluir = texto_de(l.get_ref(5)?);
                }
                fk.colunas.push(texto_de(l.get_ref(2)?));
                fk.colunas_ref.push(texto_opcional(l.get_ref(3)?));
            }
            drop(r);
            drop(stmt);
            for fk in fks.values_mut() {
                if fk.colunas_ref.iter().any(Option::is_none) {
                    // REFERENCES tabela sem coluna: aponta para a chave primária da tabela referenciada
                    let pk_ref = chave_primaria_de(conn, &fk.tabela).unwrap_or_default();
                    for (i, c) in fk.colunas_ref.iter_mut().enumerate() {
                        if c.is_none() {
                            *c = pk_ref.get(i).cloned();
                        }
                    }
                }
            }
        }
        let chaves_estrangeiras: Vec<ChaveEstrangeira> = fks.into_values().collect();

        let tem_rowid = tipo == TipoObjeto::Table && !meta.wr;
        let indice_pk = indices.iter().any(|i| i.origem == OrigemIndice::Pk);
        let mut alias_rowid = None;
        if tem_rowid
            && let [pk] = meta.pks.as_slice()
            && !indice_pk
            && let Some(col) = meta.colunas.iter().find(|c| &c.nome == pk)
            && col.tipo.trim().eq_ignore_ascii_case("INTEGER")
        {
            alias_rowid = Some(col.nome.clone());
        }
        let autoinc = alias_rowid.is_some() && tem_palavra(meta.sql.as_deref(), "AUTOINCREMENT");
        let unicas: Vec<&str> = indices
            .iter()
            .filter(|i| i.unico && i.origem == OrigemIndice::U && i.colunas.len() == 1)
            .filter_map(|i| i.colunas.first().map(|c| c.nome.as_str()))
            .collect();

        let colunas = meta
            .colunas
            .iter()
            .map(|c| {
                let fk = chaves_estrangeiras.iter().find(|f| f.colunas.contains(&c.nome)).map(|f| {
                    let pos = f.colunas.iter().position(|x| x == &c.nome).unwrap_or(0);
                    FkColuna {
                        tabela: f.tabela.clone(),
                        coluna: f.colunas_ref.get(pos).cloned().flatten(),
                        ao_excluir: f.ao_excluir.clone(),
                        ao_atualizar: f.ao_atualizar.clone(),
                    }
                });
                let e_alias = alias_rowid.as_deref() == Some(c.nome.as_str());
                ColunaInfo {
                    cid: c.cid,
                    nome: c.nome.clone(),
                    tipo: c.tipo.clone(),
                    afinidade: c.afinidade,
                    nao_nulo: c.nao_nulo,
                    padrao: c.padrao.clone(),
                    pk: c.pk,
                    oculta: c.oculta,
                    gerada: c.gerada,
                    autoincremento: e_alias && autoinc,
                    rowid_alias: e_alias,
                    unica: unicas.contains(&c.nome.as_str()),
                    fk,
                }
            })
            .collect();

        let gatilhos = {
            let mut stmt = conn.prepare(
                "SELECT name, sql FROM main.sqlite_schema WHERE type = 'trigger' AND tbl_name = ? COLLATE NOCASE \
                 ORDER BY name",
            )?;
            let mut r = stmt.query([nome])?;
            let mut saida = Vec::new();
            while let Some(l) = r.next()? {
                saida.push(GatilhoInfo { nome: texto_de(l.get_ref(0)?), sql: texto_opcional(l.get_ref(1)?) });
            }
            saida
        };
        let esquema_editavel = !(matches!(tipo, TipoObjeto::View | TipoObjeto::Virtual | TipoObjeto::Shadow)
            || meta.sistema
            || self.somente_leitura);
        let linhas = if self.pode_contar() { contar_ou_nada(conn, nome) } else { None };
        Ok(TabelaInfo {
            nome: nome.clone(),
            tipo,
            sistema: meta.sistema,
            sem_rowid: meta.wr,
            estrita: meta.strict,
            editavel: meta.gravavel,
            esquema_editavel,
            chave: meta.chave.clone(),
            colunas,
            chaves_estrangeiras,
            indices,
            gatilhos,
            sql: meta.sql.clone(),
            linhas,
        })
    }

    /// Pagina (§4.3): uma página de linhas com a identidade de cada uma e os totais do filtro.
    ///
    /// Com `contar: false` não há contagem (`total` e `paginas` None): a página vem com uma linha a mais, só
    /// para saber se há outra depois (`tem_mais`), e a consulta pode parar cedo. Com `leitura`, uma leitura
    /// mais nova do mesmo grupo abandona esta (ErroBanco 409 `Substituido`).
    pub fn linhas(&self, nome: &str, pedido: &PedidoPagina) -> Resultado<Pagina> {
        let pagina = pedido.pagina.clamp(1, MAXIMO_PAGINA);
        let por_pagina = pedido.por_pagina.clamp(1, 1000);
        let contar = pedido.contar;
        self.lendo_substituivel(pedido.leitura.as_deref(), |conn| {
            let inicio = Instant::now();
            let meta = self.meta(conn, nome)?;
            let (where_sql, params) = montar_where(&meta, pedido.busca.as_deref(), &pedido.filtros)?;
            let (ordem_sql, nome_ordem) = montar_ordem(&meta, pedido.ordem.as_deref(), pedido.direcao)?;
            let n_ident = expr_identidade(&meta).len();
            let deslocamento = (pagina - 1) * por_pagina;
            let (linhas, total, tem_mais) = (|| {
                let indexada = contar
                    && nome_ordem.is_some()
                    && where_sql.is_empty()
                    && meta.tipo != TipoObjeto::View
                    && indice_comeca_por(conn, &meta.nome, nome_ordem.as_deref().unwrap_or_default())?;
                let (sql, com_total) =
                    pagina_sql(&meta, &where_sql, &ordem_sql, nome_ordem.as_deref(), contar, indexada);
                let pedir = if contar { por_pagina } else { por_pagina + 1 };
                let mut todos = params.clone();
                todos.push(Valor::Inteiro(i64::try_from(pedir).unwrap_or(i64::MAX)));
                todos.push(Valor::Inteiro(i64::try_from(deslocamento).unwrap_or(i64::MAX)));
                let mut stmt = conn.prepare(&sql)?;
                let n_colunas = stmt.column_count();
                let n_celulas = n_colunas.saturating_sub(n_ident + usize::from(com_total));
                let mut brutas = stmt.query(params_from_iter(&todos))?;
                let mut linhas = Vec::new();
                let mut total_lido = None;
                while let Some(r) = brutas.next()? {
                    let mut ident = Vec::with_capacity(n_ident);
                    for i in 0..n_ident {
                        ident.push(Valor::de_ref(r.get_ref(i)?));
                    }
                    let mut valores = Vec::with_capacity(n_celulas);
                    for i in n_ident..n_ident + n_celulas {
                        valores.push(Celula::de_ref(r.get_ref(i)?, Some(LIMITE_TEXTO)));
                    }
                    if com_total && total_lido.is_none() {
                        total_lido = Some(contagem_de(r.get_ref(n_ident + n_celulas)?));
                    }
                    linhas.push(LinhaPagina { chave: montar_chave(&meta, &ident), valores });
                }
                drop(brutas);
                drop(stmt);
                if !contar {
                    let tem_mais = linhas.len() as u64 > por_pagina;
                    linhas.truncate(usize::try_from(por_pagina).unwrap_or(usize::MAX));
                    return Ok((linhas, None, tem_mais));
                }
                let mut total = total_lido;
                if total.is_none() && deslocamento == 0 && (linhas.len() as u64) < por_pagina {
                    // a primeira página já trouxe tudo: contar de novo varreria a tabela à toa
                    total = Some(linhas.len() as u64);
                }
                let total = match total {
                    Some(t) => t,
                    None => conn.query_row(
                        &format!("SELECT count(*) FROM main.{}{where_sql}", q(&meta.nome)),
                        params_from_iter(&params),
                        |r| r.get_ref(0).map(contagem_de),
                    )?,
                };
                let tem_mais = deslocamento + (linhas.len() as u64) < total;
                Ok((linhas, Some(total), tem_mais))
            })()
            .ctx("Não foi possível ler as linhas.")?;
            Ok(Pagina {
                colunas: meta.visiveis.iter().map(|c| c.nome.clone()).collect(),
                tipos: meta.visiveis.iter().map(|c| c.tipo.clone()).collect(),
                afinidades: meta.visiveis.iter().map(|c| c.afinidade).collect(),
                linhas,
                total,
                pagina,
                por_pagina,
                paginas: total.map(|t| t.div_ceil(por_pagina).max(1)),
                tem_mais,
                ordem: nome_ordem,
                direcao: pedido.direcao,
                tempo_ms: tempo_ms(inicio),
            })
        })
    }

    /// A linha da chave: (identidade, células) com o texto cortado em `limite`.
    pub(crate) fn ler_linha(
        &self,
        conn: &Connection,
        meta: &Meta,
        chave: &Chave,
        limite: Option<usize>,
    ) -> Resultado<Option<(Vec<Valor>, Vec<Celula>)>> {
        let (where_sql, params) = onde_chave(meta, chave)?;
        let sql = format!("{} WHERE {where_sql}", select_sql(meta, true));
        ler_primeira(conn, &sql, &params, expr_identidade(meta).len(), limite).ctx("Não foi possível ler a linha.")
    }

    pub(crate) fn linha_sumiu() -> ErroBanco {
        ErroBanco::com_status(
            "A linha não foi encontrada; ela pode ter sido excluída ou alterada por outro programa.",
            404,
        )
    }

    /// Uma linha inteira, com o texto sem cortar (BLOB continua resumido; o conteúdo sai por
    /// [`Banco::ler_blob`]).
    pub fn linha(&self, nome: &str, chave: &Chave) -> Resultado<LinhaCompleta> {
        let (meta, lida) = self.lendo(|conn| {
            let meta = self.meta(conn, nome)?;
            let lida = self.ler_linha(conn, &meta, chave, None)?;
            Ok((meta, lida))
        })?;
        let (ident, celulas) = lida.ok_or_else(Banco::linha_sumiu)?;
        Ok(LinhaCompleta {
            chave: montar_chave(&meta, &ident),
            valores: meta.visiveis.iter().map(|c| c.nome.clone()).zip(celulas).collect(),
        })
    }

    /// O conteúdo inteiro de uma célula: BLOB como está, NULL → None, o resto como texto UTF-8.
    pub fn ler_blob(&self, nome: &str, chave: &Chave, coluna: &str) -> Resultado<Option<Vec<u8>>> {
        let valor = self.lendo(|conn| {
            let meta = self.meta(conn, nome)?;
            let col = crate::meta::coluna_por_nome(&meta.visiveis, coluna).ok_or_else(|| {
                ErroBanco::com_status(format!("A coluna não existe em {}: {coluna}.", meta.nome), 404)
            })?;
            let (where_sql, params) = onde_chave(&meta, chave)?;
            let sql = format!("SELECT {} FROM main.{} WHERE {where_sql}", q(&col.nome), q(&meta.nome));
            conn.query_row(&sql, params_from_iter(&params), |r| r.get_ref(0).map(Valor::de_ref))
                .map(Some)
                .or_else(nenhuma_linha)
                .ctx("Não foi possível ler o conteúdo da coluna.")
        })?;
        match valor.ok_or_else(Banco::linha_sumiu)? {
            Valor::Nulo => Ok(None),
            Valor::Blob(b) => Ok(Some(b)),
            outro => Ok(Some(texto_do_valor(&outro).into_bytes())),
        }
    }
}

/// A coluna é única segundo o PRAGMA index_list da tabela?
fn unico_na_lista(conn: &Connection, tabela: &str, indice: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare("SELECT name, \"unique\" FROM pragma_index_list(?, 'main')")?;
    let mut r = stmt.query([tabela])?;
    while let Some(l) = r.next()? {
        if texto_de(l.get_ref(0)?) == indice {
            return Ok(inteiro_de(l.get_ref(1)?) != 0);
        }
    }
    Ok(false)
}

/// Colunas da chave primária de uma tabela, na ordem da chave.
fn chave_primaria_de(conn: &Connection, tabela: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name, pk FROM pragma_table_info(?, 'main') WHERE pk > 0")?;
    let mut r = stmt.query([tabela])?;
    let mut cols = Vec::new();
    while let Some(l) = r.next()? {
        cols.push((inteiro_de(l.get_ref(1)?), texto_de(l.get_ref(0)?)));
    }
    cols.sort_by_key(|(pk, _)| *pk);
    Ok(cols.into_iter().map(|(_, nome)| nome).collect())
}
