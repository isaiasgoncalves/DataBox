//! Introspecção de uma tabela ou visão e a identidade das linhas.

use comum::{Afinidade, Chave, ErroBanco, Resultado, Valor, afinidade, q};
use rusqlite::Connection;

use crate::erros::{Contexto, MSG_SEM_IDENTIDADE, detalhe_curto, e_trava_sqlite};
use crate::sql::{comeca_com, limpar_sql, modulo_da_tabela_virtual, tem_palavra};
use crate::tipos::{Direcao, TipoChave, TipoObjeto};
use crate::valores::{inteiro_de, texto_de, texto_opcional};

/// Módulos de tabela virtual que aceitam INSERT/UPDATE/DELETE; os demais (dbstat, csv etc.) ficam só leitura.
pub(crate) const MODULOS_GRAVAVEIS: [&str; 6] = ["fts3", "fts4", "fts5", "rtree", "rtree_i32", "geopoly"];

/// Dobra a caixa (o bastante para nomes de tabela e coluna).
pub(crate) fn dobrar_caixa(s: &str) -> String {
    s.to_lowercase().replace('ß', "ss").replace('ς', "σ")
}

/// Um objeto do esquema main, como o PRAGMA table_list o descreve.
#[derive(Debug, Clone)]
pub(crate) struct Bruta {
    pub(crate) nome: String,
    pub(crate) tipo: TipoObjeto,
    pub(crate) wr: bool,
    pub(crate) strict: bool,
}

/// Uma coluna do PRAGMA table_xinfo, com a afinidade.
#[derive(Debug, Clone)]
pub(crate) struct ColunaMeta {
    pub(crate) cid: i64,
    pub(crate) nome: String,
    pub(crate) tipo: String,
    pub(crate) afinidade: Afinidade,
    pub(crate) nao_nulo: bool,
    pub(crate) padrao: Option<String>,
    pub(crate) pk: i64,
    pub(crate) oculta: i64,
    pub(crate) gerada: bool,
}

/// Tudo o que leitura e gravação precisam saber de uma tabela ou visão.
#[derive(Debug, Clone)]
pub(crate) struct Meta {
    pub(crate) nome: String,
    pub(crate) tipo: TipoObjeto,
    pub(crate) wr: bool,
    pub(crate) strict: bool,
    pub(crate) sql: Option<String>,
    pub(crate) colunas: Vec<ColunaMeta>,
    pub(crate) visiveis: Vec<ColunaMeta>,
    pub(crate) pks: Vec<String>,
    pub(crate) chave: Option<TipoChave>,
    pub(crate) sistema: bool,
    pub(crate) modulo: Option<String>,
    pub(crate) gravavel: bool,
}

/// `fetchone()` que não achou nada: None em vez de erro.
pub(crate) fn nenhuma_linha<T>(e: rusqlite::Error) -> rusqlite::Result<Option<T>> {
    match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        outro => Err(outro),
    }
}

pub(crate) fn e_sistema(nome: &str, tipo: TipoObjeto) -> bool {
    tipo == TipoObjeto::Shadow || nome.to_lowercase().starts_with("sqlite_")
}

/// Objetos do esquema main via PRAGMA table_list: nome, tipo, WITHOUT ROWID e STRICT.
pub(crate) fn tabelas_brutas(conn: &Connection) -> rusqlite::Result<Vec<Bruta>> {
    let lista = (|| {
        let mut stmt = conn.prepare("SELECT name, type, wr, strict FROM pragma_table_list WHERE schema = 'main'")?;
        let mut linhas = stmt.query([])?;
        let mut saida = Vec::new();
        while let Some(r) = linhas.next()? {
            saida.push(Bruta {
                nome: texto_de(r.get_ref(0)?),
                tipo: TipoObjeto::de_texto(&texto_de(r.get_ref(1)?)),
                wr: inteiro_de(r.get_ref(2)?) != 0,
                strict: inteiro_de(r.get_ref(3)?) != 0,
            });
        }
        Ok(saida)
    })();
    match lista {
        Ok(saida) => Ok(saida),
        // arquivo travado: o caminho alternativo esperaria de novo pela mesma trava
        Err(e) if e_trava_sqlite(&e) => Err(e),
        // SQLite sem o PRAGMA ou módulo virtual ausente: classificação aproximada pelo sqlite_schema
        Err(_) => {
            let mut stmt =
                conn.prepare("SELECT name, type, sql FROM main.sqlite_schema WHERE type IN ('table', 'view')")?;
            let mut linhas = stmt.query([])?;
            let mut saida = Vec::new();
            while let Some(r) = linhas.next()? {
                let sql = texto_opcional(r.get_ref(2)?);
                let mut tipo = TipoObjeto::de_texto(&texto_de(r.get_ref(1)?));
                if tipo == TipoObjeto::Table
                    && comeca_com(&limpar_sql(sql.as_deref().unwrap_or("")), &["CREATE", "VIRTUAL"])
                {
                    tipo = TipoObjeto::Virtual;
                }
                let wr = tem_palavra(sql.as_deref(), "ROWID") && tem_palavra(sql.as_deref(), "WITHOUT");
                saida.push(Bruta { nome: texto_de(r.get_ref(0)?), tipo, wr, strict: false });
            }
            saida.push(Bruta { nome: "sqlite_schema".into(), tipo: TipoObjeto::Table, wr: false, strict: false });
            Ok(saida)
        }
    }
}

/// Em tabela STRICT, ANY guarda o valor como veio (sem afinidade nenhuma); fora dela, ANY é NUMERIC.
fn afinidade_da_coluna(tipo_declarado: &str, tabela: &Bruta) -> Afinidade {
    if tabela.strict && tipo_declarado.trim().eq_ignore_ascii_case("ANY") {
        return Afinidade::Blob;
    }
    afinidade(Some(tipo_declarado))
}

/// A coluna pelo nome exato; senão, sem diferença de caixa.
pub(crate) fn coluna_por_nome<'a>(lista: &'a [ColunaMeta], nome: &str) -> Option<&'a ColunaMeta> {
    lista.iter().find(|c| c.nome == nome).or_else(|| {
        let alvo = dobrar_caixa(nome);
        lista.iter().find(|c| dobrar_caixa(&c.nome) == alvo)
    })
}

/// Tudo o que leitura e gravação precisam saber de uma tabela ou visão (`_meta`).
pub(crate) fn construir_meta(conn: &Connection, nome: &str, somente_leitura: bool) -> Resultado<Meta> {
    if nome.is_empty() {
        return Err(ErroBanco::com_status("Informe o nome da tabela.", 400));
    }
    let brutas = tabelas_brutas(conn).ctx("Não foi possível ler a lista de tabelas.")?;
    let alvo = dobrar_caixa(nome);
    let achada = brutas
        .iter()
        .find(|t| t.nome == nome)
        .or_else(|| brutas.iter().find(|t| dobrar_caixa(&t.nome) == alvo))
        .ok_or_else(|| ErroBanco::com_status(format!("A tabela ou visão não existe: {nome}."), 404))?;
    let nome = achada.nome.clone();
    let tipo = achada.tipo;
    let colunas = (|| {
        let mut stmt = conn.prepare(
            "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden FROM pragma_table_xinfo(?, 'main')",
        )?;
        let mut linhas = stmt.query([&nome])?;
        let mut saida = Vec::new();
        while let Some(r) = linhas.next()? {
            let tipo_declarado = texto_de(r.get_ref(2)?);
            let oculta = inteiro_de(r.get_ref(6)?);
            saida.push(ColunaMeta {
                cid: inteiro_de(r.get_ref(0)?),
                nome: texto_de(r.get_ref(1)?),
                afinidade: afinidade_da_coluna(&tipo_declarado, achada),
                tipo: tipo_declarado,
                nao_nulo: inteiro_de(r.get_ref(3)?) != 0,
                padrao: texto_opcional(r.get_ref(4)?),
                pk: inteiro_de(r.get_ref(5)?),
                oculta,
                gerada: oculta == 2 || oculta == 3,
            });
        }
        Ok(saida)
    })()
    .ctx(&format!("Não foi possível ler as colunas de {nome}."))?;
    let visiveis: Vec<ColunaMeta> = colunas.iter().filter(|c| c.oculta != 1).cloned().collect();
    let sql = conn
        .query_row("SELECT sql FROM main.sqlite_schema WHERE type IN ('table', 'view') AND name = ?", [&nome], |r| {
            r.get_ref(0).map(texto_opcional)
        })
        .or_else(nenhuma_linha)
        .sem_ctx()?;
    let mut com_pk: Vec<&ColunaMeta> = colunas.iter().filter(|c| c.pk != 0).collect();
    com_pk.sort_by_key(|c| c.pk);
    let pks: Vec<String> = com_pk.into_iter().map(|c| c.nome.clone()).collect();

    let chave_pk = || (!pks.is_empty()).then(|| TipoChave::Pk { colunas: pks.clone() });
    let chave = if tipo == TipoObjeto::View {
        None
    } else if achada.wr {
        chave_pk()
    } else {
        let nomes: Vec<String> = colunas.iter().map(|c| c.nome.to_lowercase()).collect();
        match ["rowid", "oid", "_rowid_"].into_iter().find(|a| !nomes.iter().any(|n| n == a)) {
            Some(alias) => Some(TipoChave::Rowid { alias: alias.to_string() }),
            None => chave_pk(),
        }
    };

    let modulo = if tipo == TipoObjeto::Virtual {
        modulo_da_tabela_virtual(&limpar_sql(sql.as_deref().unwrap_or("")))
    } else {
        None
    };
    let sistema = e_sistema(&nome, tipo);
    let gravavel = if tipo == TipoObjeto::View || somente_leitura || chave.is_none() {
        false
    } else if sistema {
        nome.eq_ignore_ascii_case("sqlite_sequence")
    } else if tipo == TipoObjeto::Virtual {
        modulo.as_deref().is_some_and(|m| MODULOS_GRAVAVEIS.contains(&m))
    } else {
        true
    };
    Ok(Meta {
        nome,
        tipo,
        wr: achada.wr,
        strict: achada.strict,
        sql,
        colunas,
        visiveis,
        pks,
        chave,
        sistema,
        modulo,
        gravavel,
    })
}

/// Expressões da identidade da linha, para selecionar e ordenar (desempate estável).
pub(crate) fn expr_identidade(meta: &Meta) -> Vec<String> {
    match &meta.chave {
        None => Vec::new(),
        // rowid/oid/_rowid_ sem aspas: entre aspas viraria texto se não achar a coluna
        Some(TipoChave::Rowid { alias }) => vec![alias.clone()],
        Some(TipoChave::Pk { colunas }) => colunas.iter().map(|c| q(c)).collect(),
    }
}

/// A chave da linha a partir dos valores da identidade (lidos exatos, com [`Valor::TextoBruto`] para texto
/// em UTF-8 inválido). PK nula em tabela com rowid: a linha fica sem identidade.
pub(crate) fn montar_chave(meta: &Meta, valores: &[Valor]) -> Option<Chave> {
    match meta.chave.as_ref()? {
        TipoChave::Rowid { .. } => match valores.first() {
            Some(Valor::Inteiro(i)) => Some(Chave::Rowid(*i)),
            _ => None,
        },
        TipoChave::Pk { colunas } => {
            let mut pk = Vec::with_capacity(colunas.len());
            for (coluna, valor) in colunas.iter().zip(valores) {
                if valor.e_nulo() && !meta.wr {
                    // várias linhas podem ter a PK nula, e "coluna IS NULL" pegaria todas
                    return None;
                }
                pk.push((coluna.clone(), valor.clone()));
            }
            Some(Chave::Pk(pk))
        }
    }
}

/// " coluna IS ? AND …" que localiza a linha da chave, com os parâmetros.
pub(crate) fn onde_chave(meta: &Meta, chave: &Chave) -> Resultado<(String, Vec<Valor>)> {
    let Some(tipo) = &meta.chave else {
        return Err(ErroBanco::novo(format!(
            "{} não tem identidade de linha; não dá para localizar uma linha específica.",
            meta.nome
        )));
    };
    let detalhe = || detalhe_curto(&format!("{chave:?}"));
    match (tipo, chave) {
        (TipoChave::Rowid { alias }, Chave::Rowid(i)) => Ok((format!("{alias} = ?"), vec![Valor::Inteiro(*i)])),
        (TipoChave::Rowid { .. }, Chave::Pk(_)) => {
            Err(ErroBanco::novo("A chave da linha não tem o rowid.").detalhe(detalhe()))
        }
        (TipoChave::Pk { .. }, Chave::Rowid(_)) => {
            Err(ErroBanco::novo("A chave da linha não tem todas as colunas da chave primária.").detalhe(detalhe()))
        }
        (TipoChave::Pk { colunas }, Chave::Pk(valores)) => {
            let mut partes = Vec::with_capacity(colunas.len());
            let mut params = Vec::with_capacity(colunas.len());
            for c in colunas {
                let Some((_, valor)) = valores.iter().find(|(n, _)| n == c) else {
                    return Err(ErroBanco::novo("A chave da linha não tem todas as colunas da chave primária.")
                        .detalhe(detalhe()));
                };
                if valor.e_nulo() && !meta.wr {
                    return Err(ErroBanco::novo(MSG_SEM_IDENTIDADE).detalhe(detalhe()));
                }
                if let Valor::TextoBruto(bytes) = valor {
                    partes.push(format!("{} IS CAST(? AS TEXT)", q(c)));
                    params.push(Valor::Blob(bytes.clone()));
                } else {
                    partes.push(format!("{} IS ?", q(c)));
                    params.push(valor.clone());
                }
            }
            Ok((partes.join(" AND "), params))
        }
    }
}

/// `SELECT <identidade>, <colunas visíveis> FROM main.<tabela>`.
pub(crate) fn select_sql(meta: &Meta, com_identidade: bool) -> String {
    let mut cols = if com_identidade { expr_identidade(meta) } else { Vec::new() };
    cols.extend(meta.visiveis.iter().map(|c| q(&c.nome)));
    if cols.is_empty() {
        cols.push("NULL".into());
    }
    format!("SELECT {} FROM main.{}", cols.join(", "), q(&meta.nome))
}

/// " ORDER BY …" (com o desempate pela identidade) e o nome real da coluna de ordem.
pub(crate) fn montar_ordem(meta: &Meta, ordem: Option<&str>, direcao: Direcao) -> Resultado<(String, Option<String>)> {
    let mut termos = Vec::new();
    let mut nome_ordem = None;
    if let Some(ordem) = ordem.filter(|o| !o.is_empty()) {
        let col = coluna_por_nome(&meta.visiveis, ordem)
            .ok_or_else(|| ErroBanco::novo(format!("A coluna de ordenação não existe em {}: {ordem}.", meta.nome)))?;
        termos.push(format!("{} {}", q(&col.nome), direcao.sql()));
        nome_ordem = Some(col.nome.clone());
    }
    // Desempate pela identidade deixa a paginação estável; sem coluna de ordem, a direção vale para ela.
    let sentido = if nome_ordem.is_some() { "ASC" } else { direcao.sql() };
    termos.extend(expr_identidade(meta).into_iter().map(|e| format!("{e} {sentido}")));
    let sql = if termos.is_empty() { String::new() } else { format!(" ORDER BY {}", termos.join(", ")) };
    Ok((sql, nome_ordem))
}

/// Algum índice não parcial da tabela começa pela coluna (ele entrega a ordem pronta)?
pub(crate) fn indice_comeca_por(conn: &Connection, tabela: &str, coluna: &str) -> rusqlite::Result<bool> {
    let indices = {
        let mut stmt = conn.prepare("SELECT name, partial FROM pragma_index_list(?, 'main')")?;
        let mut linhas = stmt.query([tabela])?;
        let mut saida = Vec::new();
        while let Some(r) = linhas.next()? {
            saida.push((texto_de(r.get_ref(0)?), inteiro_de(r.get_ref(1)?) != 0));
        }
        saida
    };
    let alvo = dobrar_caixa(coluna);
    for (nome_indice, parcial) in indices {
        if parcial {
            continue;
        }
        let primeira = conn
            .query_row("SELECT name FROM pragma_index_info(?, 'main') WHERE seqno = 0", [&nome_indice], |r| {
                r.get_ref(0).map(texto_opcional)
            })
            .or_else(nenhuma_linha)?;
        if primeira.is_some_and(|p| dobrar_caixa(&p) == alvo) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// SELECT de uma página (parâmetros: os do WHERE, LIMIT e OFFSET) e se a última coluna é o total.
///
/// Ordenado por uma coluna, o SQLite ordena a tabela inteira; com as linhas largas (todas as colunas),
/// uma página funda de uma tabela grande levava mais de um minuto. Com ordem ou filtro, a página sai
/// então de uma subconsulta estreita (só a identidade, com o total do filtro contado na mesma passada
/// por count(*) OVER ()), e as colunas são buscadas só para as linhas da página.
///
/// Sem contar, nada obriga a passar pela tabela inteira: só com filtro (sem ordem), a consulta direta
/// para assim que junta as linhas pedidas; com ordem, a subconsulta estreita continua, sem o total.
/// O mesmo sem filtro e com um índice que começa pela coluna de ordem (`indexada`): o count(*) OVER ()
/// obrigaria a ler e ordenar a tabela inteira apesar do índice, e quem chama conta à parte.
pub(crate) fn pagina_sql(
    meta: &Meta,
    where_sql: &str,
    ordem_sql: &str,
    nome_ordem: Option<&str>,
    contar: bool,
    indexada: bool,
) -> (String, bool) {
    let ident = expr_identidade(meta);
    // PK de tabela com rowid pode ser NULL, e NULL nunca casa com IN: aí fica a consulta direta.
    let pk_nula = matches!(meta.chave, Some(TipoChave::Pk { .. })) && !meta.wr;
    let mut exprs = ident.clone();
    exprs.extend(meta.visiveis.iter().map(|c| q(&c.nome)));
    let tabela = format!("main.{}", q(&meta.nome));
    let estreita = if contar { !where_sql.is_empty() || nome_ordem.is_some() } else { nome_ordem.is_some() };
    let alvo = match ident.as_slice() {
        [] => None,
        [um] => Some(um.clone()),
        varias => Some(format!("({})", varias.join(", "))),
    };
    let Some(alvo) = alvo.filter(|_| !pk_nula && estreita) else {
        let lista = if exprs.is_empty() { "NULL".to_string() } else { exprs.join(", ") };
        return (format!("SELECT {lista} FROM {tabela}{where_sql}{ordem_sql} LIMIT ? OFFSET ?"), false);
    };
    let apelidos: Vec<String> = (0..ident.len()).map(|i| format!("editor_id{i}")).collect();
    let selecao = ident.iter().zip(&apelidos).map(|(e, a)| format!("{e} AS {a}")).collect::<Vec<_>>().join(", ");
    let lista = exprs.join(", ");
    let apelidos = apelidos.join(", ");
    if !contar || (indexada && where_sql.is_empty()) {
        return (
            format!(
                "WITH editor_pagina AS MATERIALIZED (SELECT {selecao} FROM {tabela}{where_sql}{ordem_sql} \
                 LIMIT ? OFFSET ?) SELECT {lista} FROM {tabela} \
                 WHERE {alvo} IN (SELECT {apelidos} FROM editor_pagina){ordem_sql}"
            ),
            false,
        );
    }
    (
        format!(
            "WITH editor_pagina AS MATERIALIZED (SELECT {selecao}, count(*) OVER () AS editor_total \
             FROM {tabela}{where_sql}{ordem_sql} LIMIT ? OFFSET ?) \
             SELECT {lista}, (SELECT editor_total FROM editor_pagina LIMIT 1) FROM {tabela} \
             WHERE {alvo} IN (SELECT {apelidos} FROM editor_pagina){ordem_sql}"
        ),
        true,
    )
}

/// Tabelas do usuário, visões e tabelas do sistema, cada grupo em ordem de nome sem caixa.
pub(crate) struct Classificadas {
    pub(crate) tabelas: Vec<Bruta>,
    pub(crate) visoes: Vec<Bruta>,
    pub(crate) sistema: Vec<Bruta>,
}

pub(crate) fn classificar(conn: &Connection) -> rusqlite::Result<Classificadas> {
    let mut cls = Classificadas { tabelas: Vec::new(), visoes: Vec::new(), sistema: Vec::new() };
    for t in tabelas_brutas(conn)? {
        if t.tipo == TipoObjeto::View {
            cls.visoes.push(t);
        } else if e_sistema(&t.nome, t.tipo) {
            cls.sistema.push(t);
        } else {
            cls.tabelas.push(t);
        }
    }
    for grupo in [&mut cls.tabelas, &mut cls.visoes, &mut cls.sistema] {
        grupo.sort_by_cached_key(|t| dobrar_caixa(&t.nome));
    }
    Ok(cls)
}
