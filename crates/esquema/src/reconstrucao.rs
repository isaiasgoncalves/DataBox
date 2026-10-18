//! A reconstrução de 12 passos da documentação do SQLite (<https://sqlite.org/lang_altertable.html#otheralter>):
//! tabela nova `_novo_<nome>`, cópia das linhas, troca de nomes e recriação de índices, gatilhos e visões, tudo
//! numa transação, com as chaves estrangeiras conferidas no fim.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use comum::{Resultado, q};
use ddl::{ColunaDef, montar_coluna, montar_create_table};
use rusqlite::Connection;

use crate::colunas::{tem_alias_rowid, tem_palavra};
use crate::leitura::{Estado, nome_temporario, tabelas_que_referenciam};
use crate::plano::{AVISO_PRAGMAS, AVISO_RECONSTRUCAO, Plano, e_verificacao_fk, ensaiar, partes_violacao, violacoes};
use crate::sql::lit;

/// Plano dos 12 passos. `st` é o estado da tabela depois dos comandos `antes` (que entram logo depois do
/// BEGIN, na mesma transação).
#[allow(clippy::too_many_arguments)]
pub(crate) fn reconstrucao(
    conn: &Connection,
    tabela: &str,
    st: &Estado,
    colunas: &[ColunaDef],
    restricoes: &[String],
    sufixo: &str,
    descricao: String,
    avisos: Vec<String>,
    antes: Vec<String>,
) -> Resultado<Plano> {
    let temp = nome_temporario(conn, tabela)?;
    let colunas_sql = colunas.iter().map(montar_coluna).collect::<Resultado<Vec<String>>>()?;
    let criar = montar_create_table(&temp, &colunas_sql, restricoes, sufixo)?;
    let mut ensaio = antes.clone();
    ensaio.push(criar.clone());
    ensaiar(conn, &ensaio, "A nova definição da tabela não é válida.")?;

    let velhas: HashMap<String, &str> = st.xinfo.iter().map(|r| (r.nome.to_lowercase(), r.nome.as_str())).collect();
    let mut destino: Vec<String> = Vec::new();
    let mut origem: Vec<String> = Vec::new();
    for c in colunas {
        if c.gerada.is_some() {
            continue;
        }
        if let Some(fonte) = velhas.get(&c.nome.to_lowercase()) {
            destino.push(q(&c.nome));
            origem.push(q(fonte));
        }
    }
    let sem_rowid_novo = tem_palavra(sufixo, "WITHOUT")? && tem_palavra(sufixo, "ROWID")?;
    if !destino.is_empty()
        && !st.def.sem_rowid
        && !sem_rowid_novo
        && !tem_alias_rowid(colunas, restricoes, sem_rowid_novo)?
    {
        // Sem INTEGER PRIMARY KEY o rowid é interno; copiá-lo mantém a identidade de cada linha.
        let mut usados: BTreeSet<String> = velhas.keys().cloned().collect();
        usados.extend(colunas.iter().map(|c| c.nome.to_lowercase()));
        if let Some(alias) = ["rowid", "oid", "_rowid_"].into_iter().find(|a| !usados.contains(*a)) {
            destino.insert(0, alias.to_string());
            origem.insert(0, alias.to_string());
        }
    }

    let mut comandos = vec!["BEGIN".to_string()];
    comandos.extend(antes);
    comandos.push(criar.clone());
    let mut sessao = BTreeMap::new();
    sessao.insert(0, vec!["PRAGMA foreign_keys = OFF".to_string()]);
    sessao.insert(comandos.len() - 1, vec!["PRAGMA legacy_alter_table = ON".to_string()]);
    if !destino.is_empty() {
        comandos.push(format!(
            "INSERT INTO {} ({})\n    SELECT {} FROM {}",
            q(&temp),
            destino.join(", "),
            origem.join(", "),
            q(tabela)
        ));
    }
    if tem_palavra(&st.sql, "AUTOINCREMENT")? && tem_palavra(&criar, "AUTOINCREMENT")? {
        // O DROP TABLE apaga a linha da tabela em sqlite_sequence e o RENAME leva a da nova; copiar antes
        // preserva o contador mesmo quando as últimas linhas já tinham sido excluídas.
        comandos.push(format!("DELETE FROM sqlite_sequence WHERE name = {}", lit(&temp)));
        comandos.push(format!(
            "INSERT INTO sqlite_sequence (name, seq) SELECT {}, seq FROM sqlite_sequence WHERE name = {}",
            lit(&temp),
            lit(tabela)
        ));
    }
    comandos.extend(st.visoes.iter().map(|(n, _)| format!("DROP VIEW {}", q(n))));
    comandos.push(format!("DROP TABLE {}", q(tabela)));
    comandos.push(format!("ALTER TABLE {} RENAME TO {}", q(&temp), q(tabela)));
    for grupo in [&st.indices, &st.gatilhos, &st.visoes, &st.gatilhos_visoes] {
        comandos.extend(grupo.iter().map(|(_, sql)| sql.clone()));
    }
    let mut verificadas = vec![tabela.to_string()];
    for (filha, _, _) in tabelas_que_referenciam(conn, tabela)? {
        if !verificadas[1..].contains(&filha) {
            verificadas.push(filha);
        }
    }
    comandos.extend(verificadas.iter().map(|t| format!("PRAGMA foreign_key_check({})", q(t))));
    comandos.push("COMMIT".to_string());

    let novas: BTreeSet<String> = colunas.iter().map(|c| c.nome.to_lowercase()).collect();
    let perdidas: Vec<&str> =
        st.xinfo.iter().filter(|r| !novas.contains(&r.nome.to_lowercase())).map(|r| r.nome.as_str()).collect();
    let mut lista = vec![AVISO_RECONSTRUCAO.to_string()];
    lista.extend(avisos);
    if !perdidas.is_empty() {
        lista.push(format!("Colunas removidas perdem os dados: {}.", perdidas.join(", ")));
    }
    // Linhas órfãs que o banco já tinha (gravadas com as chaves estrangeiras desligadas, por exemplo) não são
    // culpa da mudança: ficam registradas aqui e a verificação no fim só recusa as novas.
    let ja_havia: Vec<(String, Vec<_>)> =
        comandos.iter().filter(|c| e_verificacao_fk(c)).map(|c| (c.clone(), violacoes(conn, c))).collect();
    let todas: Vec<_> = ja_havia.iter().flat_map(|(_, v)| v.iter().cloned()).collect();
    if !todas.is_empty() {
        lista.push(format!(
            "O banco já tem {}. Elas ficam como estão: a verificação no fim da reconstrução só recusa problemas novos.",
            partes_violacao(&todas).join(" e ")
        ));
    }
    lista.push(AVISO_PRAGMAS.to_string());
    let mut plano = Plano::completo(descricao, comandos, true, lista);
    plano.sessao = sessao;
    plano.ja_havia = ja_havia;
    Ok(plano)
}
