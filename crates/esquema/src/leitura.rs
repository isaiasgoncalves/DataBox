//! Leitura do esquema: o objeto pelo nome, a tabela editável, nomes livres e o estado que a reconstrução
//! precisa saber.

use comum::{ErroBanco, Resultado, q};
use ddl::{ColunaDef, DefColuna, DefTabela, analisar_create_table, aparar, citados, preenchido, significativos};
use rusqlite::Connection;
use rusqlite::types::ToSql;

use crate::plano::desfeito_no_fim;
use crate::sql::{
    NoPreparo, inteiro_em, linhas, primeira, rodar, sem_frase_de_preparo, texto, texto_opcional, traduzir,
};

/// Uma linha de sqlite_schema: (type, name, tbl_name, sql).
#[derive(Debug, Clone)]
pub(crate) struct Objeto {
    pub tipo: String,
    pub nome: String,
    pub tabela: String,
    pub sql: Option<String>,
}

fn ler_objeto(r: &rusqlite::Row<'_>) -> rusqlite::Result<Objeto> {
    Ok(Objeto { tipo: texto(r, 0)?, nome: texto(r, 1)?, tabela: texto(r, 2)?, sql: texto_opcional(r, 3)? })
}

/// "?,?,?" com uma marca por item.
fn marcas(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// O objeto de sqlite_schema com esse nome e um desses tipos: nome exato primeiro, depois sem caixa.
pub(crate) fn objeto(conn: &Connection, nome: &str, tipos: &[&str]) -> Resultado<Option<Objeto>> {
    for comparacao in ["name = ?", "name = ? COLLATE NOCASE"] {
        let sql = format!(
            "SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE {comparacao} AND type IN ({})",
            marcas(tipos.len())
        );
        let mut params: Vec<&dyn ToSql> = vec![&nome];
        params.extend(tipos.iter().map(|t| t as &dyn ToSql));
        if let Some(obj) = primeira(conn, &sql, params.as_slice(), ler_objeto).no_preparo()? {
            return Ok(Some(obj));
        }
    }
    Ok(None)
}

fn e_virtual(sql: &str) -> Resultado<bool> {
    let toks = significativos(sql)?;
    Ok(toks.len() > 1 && toks[0].e(&["CREATE"]) && toks[1].e(&["VIRTUAL"]))
}

/// Nome real e SQL de uma tabela cuja estrutura pode ser mudada; senão [`ErroBanco`] explicando por quê.
pub(crate) fn exigir_tabela(conn: &Connection, tabela: &str, virtual_ok: bool) -> Resultado<(String, String)> {
    if !preenchido(tabela) {
        return Err(ErroBanco::novo("Informe o nome da tabela."));
    }
    let Some(obj) = objeto(conn, tabela, &["table", "view"])? else {
        return Err(ErroBanco::com_status(format!("A tabela {tabela} não existe."), 404));
    };
    let nome = obj.nome;
    if obj.tipo == "view" {
        return Err(ErroBanco::novo(format!(
            "{nome} é uma visão: a estrutura dela é o próprio SELECT. Para mudá-la, exclua a visão e crie de novo."
        )));
    }
    if nome.to_lowercase().starts_with("sqlite_") {
        return Err(ErroBanco::com_status(
            format!("{nome} é uma tabela interna do SQLite; a estrutura dela não pode ser mudada."),
            403,
        ));
    }
    let tipo_lista =
        primeira(conn, &format!("PRAGMA main.table_list({})", q(&nome)), [], |r| texto(r, 2)).no_preparo()?;
    if tipo_lista.as_deref() == Some("shadow") {
        return Err(ErroBanco::com_status(
            format!(
                "{nome} é uma tabela auxiliar de uma tabela virtual; mudar a estrutura dela estragaria a tabela \
                 virtual."
            ),
            403,
        ));
    }
    let sql = obj.sql.unwrap_or_default();
    if !virtual_ok && e_virtual(&sql)? {
        return Err(ErroBanco::novo(format!(
            "{nome} é uma tabela virtual: as colunas vêm do módulo que a criou e não podem ser editadas."
        )));
    }
    Ok((nome, sql))
}

pub(crate) fn def_tabela(conn: &Connection, tabela: &str) -> Resultado<(String, DefTabela)> {
    let (nome, sql) = exigir_tabela(conn, tabela, false)?;
    Ok((nome, analisar_create_table(&sql)?))
}

/// As definições de uma tabela editável.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Definicoes {
    pub colunas: Vec<ColunaDef>,
    /// Restrições de tabela como estão no texto ("PRIMARY KEY (pedido_id, produto)").
    pub restricoes: Vec<String>,
    pub sem_rowid: bool,
    pub estrita: bool,
}

/// Colunas ([`ColunaDef`]), restrições de tabela, WITHOUT ROWID e STRICT de uma tabela editável. Visão,
/// tabela virtual, auxiliar de tabela virtual, interna do SQLite ou que não existe → [`ErroBanco`].
pub fn definicoes_tabela(conn: &Connection, tabela: &str) -> Resultado<Definicoes> {
    let (_, d) = def_tabela(conn, tabela).map_err(sem_frase_de_preparo)?;
    Ok(Definicoes {
        colunas: d.colunas.iter().map(DefColuna::como_dict).collect(),
        restricoes: d.restricoes,
        sem_rowid: d.sem_rowid,
        estrita: d.estrita,
    })
}

/// "uma tabela", "uma visão"… para a frase de nome já usado.
fn artigo_tipo(tipo: &str) -> &'static str {
    match tipo {
        "table" => "uma tabela",
        "view" => "uma visão",
        "index" => "um índice",
        "trigger" => "um gatilho",
        _ => "um objeto",
    }
}

/// Nome novo sem espaços nas pontas; vazio, reservado ou já usado → [`ErroBanco`].
pub(crate) fn nome_livre(conn: &Connection, nome: &str, rotulo: &str, tipos: &[&str]) -> Resultado<String> {
    if !preenchido(nome) {
        return Err(ErroBanco::novo(format!("Informe o nome {rotulo}.")));
    }
    let nome = aparar(nome);
    if nome.to_lowercase().starts_with("sqlite_") {
        return Err(ErroBanco::novo("Nomes que começam com sqlite_ são reservados pelo SQLite."));
    }
    let sql = format!(
        "SELECT type, name FROM sqlite_schema WHERE type IN ({}) AND name = ? COLLATE NOCASE",
        marcas(tipos.len())
    );
    let mut params: Vec<&dyn ToSql> = tipos.iter().map(|t| t as &dyn ToSql).collect();
    params.push(&nome);
    let existe = primeira(conn, &sql, params.as_slice(), |r| Ok((texto(r, 0)?, texto(r, 1)?))).no_preparo()?;
    if let Some((tipo, real)) = existe {
        return Err(ErroBanco::com_status(
            format!(
                "Já existe {} chamado {real} no banco (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
                artigo_tipo(&tipo)
            ),
            409,
        ));
    }
    Ok(nome.to_string())
}

/// Os tipos que um nome novo de tabela, visão ou índice não pode repetir.
pub(crate) const TIPOS_NOME: [&str; 3] = ["table", "view", "index"];

/// "_novo_<tabela>" (ou "_novo_<tabela>_2"…) que ainda não existe.
pub(crate) fn nome_temporario(conn: &Connection, tabela: &str) -> Resultado<String> {
    let base = format!("_novo_{tabela}");
    let mut nome = base.clone();
    let mut k = 2;
    while primeira(conn, "SELECT 1 FROM sqlite_schema WHERE name = ? COLLATE NOCASE", [&nome], |_| Ok(()))
        .no_preparo()?
        .is_some()
    {
        nome = format!("{base}_{k}");
        k += 1;
    }
    Ok(nome)
}

/// Uma linha de `PRAGMA table_xinfo`: (cid, name, type, notnull, dflt_value, pk, hidden), só o que se usa.
#[derive(Debug, Clone)]
pub(crate) struct ColunaXinfo {
    pub nome: String,
    /// Posição na chave primária (0 = fora dela).
    pub pk: i64,
}

pub(crate) fn xinfo(conn: &Connection, tabela: &str) -> Resultado<Vec<ColunaXinfo>> {
    linhas(conn, &format!("PRAGMA table_xinfo({})", q(tabela)), [], |r| {
        Ok(ColunaXinfo { nome: texto(r, 1)?, pk: inteiro_em(r, 5)? })
    })
    .no_preparo()
}

/// O que a reconstrução precisa saber da tabela e do que depende dela, na ordem de criação.
#[derive(Debug, Clone)]
pub(crate) struct Estado {
    pub sql: String,
    pub def: DefTabela,
    pub xinfo: Vec<ColunaXinfo>,
    /// (nome, sql) dos índices criados por CREATE INDEX.
    pub indices: Vec<(String, String)>,
    pub gatilhos: Vec<(String, String)>,
    /// Visões que citam a tabela.
    pub visoes: Vec<(String, String)>,
    /// Gatilhos dessas visões (INSTEAD OF), que saem com elas.
    pub gatilhos_visoes: Vec<(String, String)>,
}

pub(crate) fn estado(conn: &Connection, tabela: &str) -> Resultado<Estado> {
    let todos = linhas(conn, "SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY rowid", [], ler_objeto)
        .no_preparo()?;
    let baixo = tabela.to_lowercase();
    let mut sql_tabela: Option<String> = None;
    let (mut indices, mut gatilhos, mut visoes, mut gatilhos_visoes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for o in &todos {
        if o.tipo == "table" && o.nome == tabela {
            sql_tabela.clone_from(&o.sql);
        } else if o.tipo == "index"
            && o.tabela.to_lowercase() == baixo
            && let Some(sql) = o.sql.as_ref().filter(|s| !s.is_empty())
        {
            indices.push((o.nome.clone(), sql.clone()));
        } else if o.tipo == "trigger"
            && o.tabela.to_lowercase() == baixo
            && let Some(sql) = &o.sql
        {
            gatilhos.push((o.nome.clone(), sql.clone()));
        } else if o.tipo == "view"
            && citados(o.sql.as_deref().unwrap_or(""))?.contains(&baixo)
            && let Some(sql) = &o.sql
        {
            visoes.push((o.nome.clone(), sql.clone()));
        }
    }
    let nomes_visoes: Vec<String> = visoes.iter().map(|(n, _)| n.to_lowercase()).collect();
    for o in &todos {
        if o.tipo == "trigger"
            && nomes_visoes.contains(&o.tabela.to_lowercase())
            && let Some(sql) = &o.sql
        {
            gatilhos_visoes.push((o.nome.clone(), sql.clone()));
        }
    }
    let sql = sql_tabela.unwrap_or_default();
    Ok(Estado {
        def: analisar_create_table(&sql)?,
        sql,
        xinfo: xinfo(conn, tabela)?,
        indices,
        gatilhos,
        visoes,
        gatilhos_visoes,
    })
}

/// [`estado`] como ficaria depois dos comandos `antes` (rodados numa transação e desfeitos).
pub(crate) fn estado_apos(conn: &Connection, tabela: &str, antes: &[String]) -> Resultado<Estado> {
    if antes.is_empty() {
        return estado(conn, tabela);
    }
    desfeito_no_fim(conn, "_editor_previa", || {
        for c in antes {
            rodar(conn, c).map_err(|e| traduzir(&e, "Não foi possível preparar a mudança."))?;
        }
        estado(conn, tabela)
    })
}

/// Posição e definição da coluna: nome exato primeiro, depois sem caixa.
pub(crate) fn achar_coluna<'a>(d: &'a DefTabela, coluna: &str) -> Resultado<(usize, &'a DefColuna)> {
    if !preenchido(coluna) {
        return Err(ErroBanco::novo("Informe o nome da coluna."));
    }
    if let Some(achada) = d.colunas.iter().enumerate().find(|(_, c)| c.nome == coluna) {
        return Ok(achada);
    }
    let baixo = coluna.to_lowercase();
    if let Some(achada) = d.colunas.iter().enumerate().find(|(_, c)| c.nome.to_lowercase() == baixo) {
        return Ok(achada);
    }
    Err(ErroBanco::com_status(format!("A coluna {coluna} não existe em {}.", d.nome), 404))
}

/// Uma linha de `PRAGMA foreign_key_list`: (id, seq, table, from, to, on_update, on_delete, match).
#[derive(Debug, Clone)]
pub(crate) struct ChaveEstrangeira {
    pub id: i64,
    pub tabela: String,
    pub de: String,
    /// NULL quando a referência aponta para a chave primária sem dizer a coluna.
    pub para: Option<String>,
    pub ao_excluir: String,
}

/// As chaves estrangeiras de uma tabela; erro do SQLite (tabela virtual cujo módulo não está carregado) sobe.
pub(crate) fn chaves_estrangeiras(conn: &Connection, tabela: &str) -> rusqlite::Result<Vec<ChaveEstrangeira>> {
    linhas(conn, &format!("PRAGMA foreign_key_list({})", q(tabela)), [], |r| {
        Ok(ChaveEstrangeira {
            id: inteiro_em(r, 0)?,
            tabela: texto(r, 2)?,
            de: texto(r, 3)?,
            para: texto_opcional(r, 4)?,
            ao_excluir: texto(r, 6)?,
        })
    })
}

/// Nomes das tabelas, na ordem de criação.
pub(crate) fn nomes_tabelas(conn: &Connection) -> Resultado<Vec<String>> {
    linhas(conn, "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY rowid", [], |r| texto(r, 0)).no_preparo()
}

/// (tabela filha, coluna referenciada ou None, ação ao excluir) das chaves estrangeiras que apontam para
/// `tabela`, fora ela mesma.
pub(crate) fn tabelas_que_referenciam(
    conn: &Connection,
    tabela: &str,
) -> Resultado<Vec<(String, Option<String>, String)>> {
    let baixo = tabela.to_lowercase();
    let mut achadas = Vec::new();
    for nome in nomes_tabelas(conn)? {
        if nome == tabela || nome.to_lowercase().starts_with("sqlite_") {
            continue;
        }
        // tabela virtual cujo módulo não está carregado: pula
        let Ok(fks) = chaves_estrangeiras(conn, &nome) else { continue };
        for fk in fks {
            if fk.tabela.to_lowercase() == baixo {
                achadas.push((nome.clone(), fk.para, fk.ao_excluir));
            }
        }
    }
    Ok(achadas)
}
