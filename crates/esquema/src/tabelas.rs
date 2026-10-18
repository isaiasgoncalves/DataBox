//! Tabelas: criar, renomear e excluir.

use std::collections::BTreeSet;

use comum::{ErroBanco, Resultado, q};
use ddl::{ColunaDef, aparar, citados, montar_coluna, montar_create_table, preenchido};
use rusqlite::Connection;

use crate::colunas::{normalizar_coluna, tem_palavra};
use crate::leitura::{
    TIPOS_NOME, chaves_estrangeiras, exigir_tabela, nome_livre, nome_temporario, nomes_tabelas, xinfo,
};
use crate::plano::{Plano, ensaiar};
use crate::sql::{NoPreparo, inteiro, linhas, plural, texto};

/// Cria uma tabela. Várias colunas com `pk` formam uma chave primária composta (restrição de tabela);
/// `restricoes` são restrições de tabela como texto ("CHECK (de < ate)"; `&[String]` ou `&[&str]`; sem
/// nenhuma, `&[] as &[&str]`); `sem_rowid` → WITHOUT ROWID.
pub fn plano_criar_tabela<S: AsRef<str>>(
    conn: &Connection,
    nome: &str,
    colunas: &[ColunaDef],
    restricoes: &[S],
    sem_rowid: bool,
) -> Resultado<Plano> {
    let nome = nome_livre(conn, nome, "da tabela", &TIPOS_NOME)?;
    if colunas.is_empty() {
        return Err(ErroBanco::novo("A tabela precisa de ao menos uma coluna."));
    }
    let mut cols: Vec<ColunaDef> = colunas.iter().map(normalizar_coluna).collect();
    let mut vistos: BTreeSet<String> = BTreeSet::new();
    for c in &mut cols {
        if !preenchido(&c.nome) {
            return Err(ErroBanco::novo("Toda coluna precisa de um nome."));
        }
        c.nome = aparar(&c.nome).to_string();
        if !vistos.insert(c.nome.to_lowercase()) {
            return Err(ErroBanco::novo(format!(
                "A coluna {} aparece mais de uma vez (o SQLite não diferencia maiúsculas de minúsculas nos nomes).",
                c.nome
            )));
        }
    }
    let pks: Vec<usize> = cols.iter().enumerate().filter(|(_, c)| c.pk).map(|(k, _)| k).collect();
    for c in &cols {
        if c.autoincremento && !(c.pk && pks.len() == 1 && c.tipo.to_uppercase() == "INTEGER") {
            return Err(ErroBanco::novo(format!(
                "Autoincremento só vale numa chave primária INTEGER de uma coluna só (coluna {}).",
                c.nome
            )));
        }
        if c.autoincremento && sem_rowid {
            return Err(ErroBanco::novo("Tabela sem rowid (WITHOUT ROWID) não aceita autoincremento."));
        }
    }
    let mut restr: Vec<String> =
        restricoes.iter().map(AsRef::as_ref).filter(|r| preenchido(r)).map(|r| aparar(r).to_string()).collect();
    let mut avisos = Vec::new();
    if pks.len() > 1 {
        // Várias colunas marcadas como chave primária formam uma chave composta.
        let citadas: Vec<String> = pks.iter().map(|&k| q(&cols[k].nome)).collect();
        restr.insert(0, format!("PRIMARY KEY ({})", citadas.join(", ")));
        let nomes: Vec<String> = pks.iter().map(|&k| cols[k].nome.clone()).collect();
        for &k in &pks {
            cols[k].pk = false;
        }
        avisos.push(format!("As colunas {} formam juntas a chave primária.", nomes.join(", ")));
    }
    let mut tem_pk_tabela = false;
    for r in &restr {
        if tem_palavra(r, "PRIMARY")? {
            tem_pk_tabela = true;
            break;
        }
    }
    if sem_rowid && pks.is_empty() && !tem_pk_tabela {
        return Err(ErroBanco::novo("Uma tabela sem rowid (WITHOUT ROWID) precisa de chave primária."));
    }
    let colunas_sql = cols.iter().map(montar_coluna).collect::<Resultado<Vec<String>>>()?;
    let sql = montar_create_table(&nome, &colunas_sql, &restr, if sem_rowid { "WITHOUT ROWID" } else { "" })?;
    ensaiar(conn, std::slice::from_ref(&sql), "A definição da tabela não é válida.")?;
    let descricao = format!("Criar a tabela {nome} com {}.", plural(cols.len() as i64, "coluna", "colunas"));
    Ok(Plano::completo(descricao, vec![sql], false, avisos))
}

/// Renomeia uma tabela (inclusive virtual). Só mudar maiúsculas passa por um nome intermediário, numa
/// transação (o SQLite recusa o RENAME direto).
pub fn plano_renomear_tabela(conn: &Connection, tabela: &str, novo_nome: &str) -> Resultado<Plano> {
    let (atual, _) = exigir_tabela(conn, tabela, true)?;
    if !preenchido(novo_nome) {
        return Err(ErroBanco::novo("Informe o nome novo da tabela."));
    }
    let mut novo = aparar(novo_nome).to_string();
    if novo == atual {
        return Err(ErroBanco::novo("O nome novo é igual ao atual."));
    }
    let comandos = if novo.to_lowercase() == atual.to_lowercase() {
        // O SQLite recusa renomear só mudando maiúsculas: passa por um nome intermediário.
        let meio = nome_temporario(conn, &atual)?;
        vec![
            "BEGIN".to_string(),
            format!("ALTER TABLE {} RENAME TO {}", q(&atual), q(&meio)),
            format!("ALTER TABLE {} RENAME TO {}", q(&meio), q(&novo)),
            "COMMIT".to_string(),
        ]
    } else {
        novo = nome_livre(conn, &novo, "da tabela", &TIPOS_NOME)?;
        vec![format!("ALTER TABLE {} RENAME TO {}", q(&atual), q(&novo))]
    };
    let ensaio: Vec<String> = comandos.iter().filter(|c| *c != "BEGIN" && *c != "COMMIT").cloned().collect();
    ensaiar(conn, &ensaio, "Não foi possível renomear a tabela.")?;
    Ok(Plano::completo(
        format!("Renomear a tabela {atual} para {novo}."),
        comandos,
        false,
        vec![
            "Chaves estrangeiras, índices, gatilhos e visões que citam a tabela passam a usar o nome novo.".to_string(),
        ],
    ))
}

/// Exclui uma tabela (inclusive virtual), avisando do que sai junto (índices, gatilhos), do que acontece com
/// as tabelas que têm chave estrangeira para ela e das visões e gatilhos de outras tabelas que a citam.
pub fn plano_excluir_tabela(conn: &Connection, tabela: &str) -> Resultado<Plano> {
    let (nome, _) = exigir_tabela(conn, tabela, true)?;
    let n_linhas = inteiro(conn, &format!("SELECT count(*) FROM {}", q(&nome)), []).no_preparo()?;
    let mut avisos = vec!["A exclusão não pode ser desfeita.".to_string()];
    let ligados = linhas(
        conn,
        "SELECT type, name FROM sqlite_schema WHERE tbl_name = ? AND type IN ('index', 'trigger') AND sql IS NOT NULL",
        [&nome],
        |r| texto(r, 0),
    )
    .no_preparo()?;
    let n_ind = ligados.iter().filter(|t| *t == "index").count() as i64;
    let n_gat = ligados.iter().filter(|t| *t == "trigger").count() as i64;
    if n_ind != 0 || n_gat != 0 {
        avisos.push(format!(
            "Saem junto {} e {}.",
            plural(n_ind, "índice", "índices"),
            plural(n_gat, "gatilho", "gatilhos")
        ));
    }
    avisos.extend(avisos_chaves_para(conn, &nome)?);
    let baixo = nome.to_lowercase();
    let mut visoes = Vec::new();
    for (n, s) in linhas(conn, "SELECT name, sql FROM sqlite_schema WHERE type = 'view'", [], |r| {
        Ok((texto(r, 0)?, texto(r, 1)?))
    })
    .no_preparo()?
    {
        if citados(&s)?.contains(&baixo) {
            visoes.push(n);
        }
    }
    if !visoes.is_empty() {
        avisos.push(format!("Estas visões citam a tabela e deixarão de funcionar: {}.", visoes.join(", ")));
    }
    let mut gatilhos = Vec::new();
    for (n, t, s) in linhas(
        conn,
        "SELECT name, tbl_name, sql FROM sqlite_schema WHERE type = 'trigger' AND tbl_name <> ? COLLATE NOCASE \
         ORDER BY rowid",
        [&nome],
        |r| Ok((texto(r, 0)?, texto(r, 1)?, texto(r, 2)?)),
    )
    .no_preparo()?
    {
        if citados(&s)?.contains(&baixo) {
            gatilhos.push(format!("{n} (de {t})"));
        }
    }
    if !gatilhos.is_empty() {
        avisos.push(format!(
            "Estes gatilhos de outras tabelas citam {nome} e passarão a falhar (e a impedir a gravação que os \
             dispara): {}.",
            gatilhos.join(", ")
        ));
    }
    Ok(Plano::completo(
        format!("Excluir a tabela {nome} e as {} dela.", plural(n_linhas, "linha", "linhas")),
        vec![format!("DROP TABLE {}", q(&nome))],
        false,
        avisos,
    ))
}

/// Uma chave estrangeira (possivelmente de várias colunas) de uma tabela filha para a que vai ser excluída.
struct Grupo {
    filha: String,
    id: i64,
    de: Vec<String>,
    para: Vec<Option<String>>,
    acao: String,
}

/// O que o DROP TABLE faz com as tabelas que têm chave estrangeira para `nome`.
///
/// Com as chaves estrangeiras ligadas, o DROP TABLE faz antes um DELETE implícito, que dispara as ações de
/// cada chave: CASCADE exclui as linhas ligadas, SET NULL e SET DEFAULT trocam o valor delas, e NO ACTION e
/// RESTRICT fazem a exclusão falhar se houver linha ligada. Desligadas, nada disso acontece e as linhas
/// ligadas ficam órfãs.
fn avisos_chaves_para(conn: &Connection, nome: &str) -> Resultado<Vec<String>> {
    let ligadas = inteiro(conn, "PRAGMA foreign_keys", []).no_preparo()? != 0;
    let mut na_pk: Vec<_> = xinfo(conn, nome)?.into_iter().filter(|r| r.pk != 0).collect();
    na_pk.sort_by_key(|r| r.pk);
    let pk_pai: Vec<String> = na_pk.into_iter().map(|r| r.nome).collect();
    let baixo = nome.to_lowercase();
    let mut grupos: Vec<Grupo> = Vec::new();
    for filha in nomes_tabelas(conn)? {
        if filha == nome || filha.to_lowercase().starts_with("sqlite_") {
            continue;
        }
        // tabela virtual cujo módulo não está carregado: pula
        let Ok(fks) = chaves_estrangeiras(conn, &filha) else { continue };
        for fk in fks {
            if fk.tabela.to_lowercase() != baixo {
                continue;
            }
            let k = match grupos.iter().position(|g| g.filha == filha && g.id == fk.id) {
                Some(k) => k,
                None => {
                    grupos.push(Grupo {
                        filha: filha.clone(),
                        id: fk.id,
                        de: Vec::new(),
                        para: Vec::new(),
                        acao: fk.ao_excluir.clone(),
                    });
                    grupos.len() - 1
                }
            };
            grupos[k].de.push(fk.de);
            grupos[k].para.push(fk.para);
        }
    }
    let mut avisos = Vec::new();
    let mut sem_destino: Vec<String> = Vec::new();
    for g in &grupos {
        let filha = &g.filha;
        let acao = if g.acao.is_empty() { "NO ACTION".to_string() } else { g.acao.to_uppercase() };
        let para: Vec<String> = if g.para.iter().all(|p| p.as_deref().is_some_and(|s| !s.is_empty())) {
            g.para.iter().map(|p| p.clone().unwrap_or_default()).collect()
        } else {
            pk_pai.clone()
        };
        let n: Option<i64> = if para.len() != g.de.len() {
            None // chave sem correspondência
        } else {
            let lista: Vec<String> = g.de.iter().map(|c| q(c)).collect();
            let alvo: Vec<String> = para.iter().map(|c| q(c)).collect();
            let sql = format!(
                "SELECT count(*) FROM {} WHERE ({}) IN (SELECT {} FROM {})",
                q(filha),
                lista.join(", "),
                alvo.join(", "),
                q(nome)
            );
            inteiro(conn, &sql, []).ok()
        };
        let colunas = g.de.join(", ");
        let ligadas_txt = match n {
            None => "as linhas ligadas".to_string(),
            Some(n) => plural(n, "linha ligada", "linhas ligadas"),
        };
        let um = n == Some(1);
        if !ligadas {
            if n != Some(0) {
                avisos.push(format!(
                    "A tabela {filha} tem chave estrangeira para {nome}, mas a verificação de chaves estrangeiras está \
                     desligada: {ligadas_txt} de {filha} {} apontando para linhas que não existem.",
                    if um { "fica" } else { "ficam" }
                ));
            }
            continue;
        }
        sem_destino.push(filha.clone());
        if n == Some(0) {
            continue;
        }
        match acao.as_str() {
            "CASCADE" => avisos.push(format!(
                "A tabela {filha} tem chave estrangeira para {nome} com ON DELETE CASCADE: {ligadas_txt} de {filha} \
                 também {}.",
                if um { "será excluída" } else { "serão excluídas" }
            )),
            "SET NULL" => avisos.push(format!(
                "A tabela {filha} tem chave estrangeira para {nome} com ON DELETE SET NULL: {colunas} fica nulo em \
                 {ligadas_txt} de {filha}."
            )),
            "SET DEFAULT" => avisos.push(format!(
                "A tabela {filha} tem chave estrangeira para {nome} com ON DELETE SET DEFAULT: {colunas} volta ao valor \
                 padrão em {ligadas_txt} de {filha} (se o padrão não for nulo, a exclusão falha, porque ele deixa de ter \
                 para onde apontar)."
            )),
            _ => {
                let hoje = match n {
                    None => ".".to_string(),
                    Some(n) => format!(" (hoje, {}).", plural(n, "linha", "linhas")),
                };
                avisos.push(format!(
                    "A tabela {filha} tem chave estrangeira para {nome} ({acao}): a exclusão falha enquanto houver \
                     linhas de {filha} apontando para ela{hoje}"
                ));
            }
        }
    }
    let mut vistas: Vec<&String> = Vec::new();
    for filha in &sem_destino {
        if vistas.contains(&filha) {
            continue;
        }
        vistas.push(filha);
        avisos.push(format!(
            "Depois da exclusão, a chave estrangeira de {filha} fica apontando para uma tabela que não existe: com as \
             chaves estrangeiras ligadas, gravar em {filha} passa a falhar até a referência ser tirada."
        ));
    }
    Ok(avisos)
}
