//! Colunas: adicionar, alterar (inclusive renomear), excluir e reordenar.

use std::collections::BTreeSet;

use comum::{ErroBanco, Resultado, afinidade, q};
use ddl::{
    ColunaDef, DefColuna, DefTabela, Gerada, Referencia, aparar, colapsar_espacos, colunas_da_expressao,
    colunas_da_restricao, colunas_gravadas_pelo_gatilho, desfazer_aspas, fatia, fecha, montar_coluna, preenchido,
    significativos,
};
use rusqlite::Connection;

use crate::leitura::{
    achar_coluna, chaves_estrangeiras, def_tabela, estado, estado_apos, tabelas_que_referenciam, xinfo,
};
use crate::plano::{Plano, ensaiar, ensaiar_com};
use crate::reconstrucao::reconstrucao;
use crate::sql::{NoPreparo, erro_de_objeto, inteiro, linhas, plural, primeira, texto, texto_opcional, traduzir};

// ---------------------------------------------------------------------------------------------------
// ColunaDef

/// Texto sem espaços nas pontas, ou None se ficar vazio.
fn texto_limpo(v: &Option<String>) -> Option<String> {
    v.as_deref().map(aparar).filter(|s| !s.is_empty()).map(str::to_string)
}

fn acao_limpa(v: &Option<String>) -> Option<String> {
    texto_limpo(v).map(|a| colapsar_espacos(&a).to_uppercase())
}

/// Cópia da ColunaDef com os textos aparados e os vazios como None, para comparar e montar.
pub(crate) fn normalizar_coluna(c: &ColunaDef) -> ColunaDef {
    let referencia = c.referencia.as_ref().filter(|r| preenchido(&r.tabela)).map(|r| Referencia {
        tabela: r.tabela.clone(),
        coluna: r.coluna.clone().filter(|col| preenchido(col)),
        ao_excluir: acao_limpa(&r.ao_excluir),
        ao_atualizar: acao_limpa(&r.ao_atualizar),
        complemento: texto_limpo(&r.complemento),
    });
    let gerada = c.gerada.as_ref().and_then(|g| {
        let expressao = aparar(&g.expressao);
        (!expressao.is_empty()).then(|| Gerada { expressao: expressao.to_string(), armazenada: g.armazenada })
    });
    ColunaDef {
        nome: c.nome.clone(),
        tipo: aparar(&colapsar_espacos(&c.tipo)).to_string(),
        nao_nulo: c.nao_nulo,
        unica: c.unica,
        pk: c.pk,
        autoincremento: c.autoincremento,
        padrao: texto_limpo(&c.padrao),
        verificacao: texto_limpo(&c.verificacao),
        colacao: texto_limpo(&c.colacao),
        referencia,
        gerada,
        extra: texto_limpo(&c.extra),
    }
}

fn formatar_ref(r: &Option<Referencia>) -> String {
    let Some(r) = r else { return "nenhuma".to_string() };
    let mut texto = r.tabela.clone();
    if let Some(coluna) = r.coluna.as_deref().filter(|c| !c.is_empty()) {
        texto.push_str(&format!(".{coluna}"));
    }
    if let Some(acao) = r.ao_excluir.as_deref().filter(|a| !a.is_empty()) {
        texto.push_str(&format!(" (ao excluir {acao})"));
    }
    if let Some(acao) = r.ao_atualizar.as_deref().filter(|a| !a.is_empty()) {
        texto.push_str(&format!(" (ao atualizar {acao})"));
    }
    texto
}

fn formatar_gerada(g: &Option<Gerada>) -> String {
    match g {
        None => "nenhuma".to_string(),
        Some(g) => format!("{} ({})", g.expressao, if g.armazenada { "STORED" } else { "VIRTUAL" }),
    }
}

/// Frases do que muda na coluna, fora o nome.
fn diferencas(velha: &ColunaDef, nova: &ColunaDef) -> Vec<String> {
    let mut mud = Vec::new();
    if velha.tipo != nova.tipo {
        let ou_sem = |t: &str| if t.is_empty() { "sem tipo".to_string() } else { t.to_string() };
        mud.push(format!("tipo {} → {}", ou_sem(&velha.tipo), ou_sem(&nova.tipo)));
    }
    let chaves: [(bool, bool, &str, &str); 4] = [
        (velha.pk, nova.pk, "passa a ser chave primária", "deixa de ser chave primária"),
        (velha.autoincremento, nova.autoincremento, "ganha autoincremento", "perde o autoincremento"),
        (velha.nao_nulo, nova.nao_nulo, "passa a não aceitar nulo", "passa a aceitar nulo"),
        (velha.unica, nova.unica, "passa a não aceitar repetição (UNIQUE)", "passa a aceitar repetição"),
    ];
    for (antes, depois, liga, desliga) in chaves {
        if antes != depois {
            mud.push((if depois { liga } else { desliga }).to_string());
        }
    }
    let textos: [(&Option<String>, &Option<String>, &str, &str); 4] = [
        (&velha.padrao, &nova.padrao, "valor padrão", "nenhum"),
        (&velha.verificacao, &nova.verificacao, "verificação", "nenhuma"),
        (&velha.colacao, &nova.colacao, "colação", "nenhuma"),
        (&velha.extra, &nova.extra, "complemento", "nenhum"),
    ];
    for (antes, depois, rotulo, vazio) in textos {
        if antes != depois {
            let ou_vazio =
                |v: &Option<String>| v.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| vazio.to_string());
            mud.push(format!("{rotulo} {} → {}", ou_vazio(antes), ou_vazio(depois)));
        }
    }
    if velha.referencia != nova.referencia {
        mud.push(format!("referência {} → {}", formatar_ref(&velha.referencia), formatar_ref(&nova.referencia)));
    }
    if velha.gerada != nova.gerada {
        mud.push(format!("expressão gerada {} → {}", formatar_gerada(&velha.gerada), formatar_gerada(&nova.gerada)));
    }
    mud
}

/// Aplica em `alvo` só os campos que mudaram de `velha` para `nova`.
fn aplicar_mudancas(alvo: &mut ColunaDef, velha: &ColunaDef, nova: &ColunaDef) {
    macro_rules! campos {
        ($($campo:ident),*) => {
            $( if nova.$campo != velha.$campo { alvo.$campo = nova.$campo.clone(); } )*
        };
    }
    campos!(nome, tipo, nao_nulo, unica, pk, autoincremento, padrao, verificacao, colacao, referencia, gerada, extra);
}

fn tipo_inteiro(tipo: &str) -> bool {
    aparar(tipo).to_uppercase() == "INTEGER"
}

/// Se a definição tem uma coluna INTEGER PRIMARY KEY (que é o próprio rowid).
pub(crate) fn tem_alias_rowid(colunas: &[ColunaDef], restricoes: &[String], sem_rowid: bool) -> Resultado<bool> {
    if sem_rowid {
        return Ok(false);
    }
    for c in colunas {
        let extra = significativos(c.extra.as_deref().unwrap_or(""))?;
        let pk_extra = extra.iter().any(|t| t.e(&["PRIMARY"]));
        if c.pk || pk_extra {
            // "INTEGER PRIMARY KEY DESC" não é alias (peculiaridade do SQLite)
            let desc = extra.iter().any(|t| t.e(&["DESC"]));
            return Ok(tipo_inteiro(&c.tipo) && !desc);
        }
    }
    for r in restricoes {
        let toks = significativos(r)?;
        for k in 0..toks.len().saturating_sub(1) {
            if toks[k].e(&["PRIMARY"]) && toks[k + 1].e(&["KEY"]) && k + 2 < toks.len() && toks[k + 2].texto == "(" {
                let z = fecha(&toks, k + 2)?;
                let dentro = toks.get(k + 3..z).unwrap_or(&[]);
                let nomes: Vec<&ddl::Token> = dentro
                    .iter()
                    .filter(|t| {
                        matches!(t.tipo, ddl::TipoToken::Palavra | ddl::TipoToken::Ident | ddl::TipoToken::Texto)
                            && !t.e(&["ASC", "DESC", "COLLATE"])
                    })
                    .collect();
                if !dentro.iter().any(|t| t.texto == ",")
                    && let Some(primeiro) = nomes.first()
                {
                    let alvo = desfazer_aspas(&primeiro.texto).to_lowercase();
                    let c = colunas.iter().find(|c| c.nome.to_lowercase() == alvo);
                    return Ok(c.is_some_and(|c| tipo_inteiro(&c.tipo)));
                }
                return Ok(false);
            }
        }
    }
    Ok(false)
}

pub(crate) fn tem_palavra(sql: &str, palavra: &str) -> Resultado<bool> {
    Ok(significativos(sql)?.iter().any(|t| t.e(&[palavra])))
}

fn padrao_nulo(c: &ColunaDef) -> bool {
    c.padrao.as_deref().is_none_or(|p| p.to_uppercase() == "NULL")
}

fn chaves_ligadas(conn: &Connection) -> Resultado<bool> {
    Ok(inteiro(conn, "PRAGMA foreign_keys", []).no_preparo()? != 0)
}

// ---------------------------------------------------------------------------------------------------
// Adicionar

/// Por que o ADD COLUMN nativo não serve para esta coluna (None = serve).
fn motivo_sem_add_nativo(conn: &Connection, c: &ColunaDef) -> Resultado<Option<&'static str>> {
    if c.pk {
        return Ok(Some("chave primária só entra reconstruindo a tabela"));
    }
    if c.unica {
        return Ok(Some("coluna sem repetição (UNIQUE) só entra reconstruindo a tabela"));
    }
    if c.gerada.as_ref().is_some_and(|g| g.armazenada) {
        return Ok(Some("coluna gerada armazenada (STORED) só entra reconstruindo a tabela"));
    }
    if let Some(padrao) = &c.padrao {
        let toks = significativos(padrao)?;
        if toks.first().is_some_and(|t| t.texto == "(" || t.e(&["CURRENT_TIME", "CURRENT_DATE", "CURRENT_TIMESTAMP"])) {
            return Ok(Some("valor padrão que não é constante só entra reconstruindo a tabela"));
        }
    }
    if c.nao_nulo && padrao_nulo(c) && c.gerada.is_none() {
        return Ok(Some("coluna sem nulo e sem valor padrão só entra reconstruindo a tabela"));
    }
    if c.referencia.is_some() && !padrao_nulo(c) && chaves_ligadas(conn)? {
        return Ok(Some("chave estrangeira com valor padrão só entra reconstruindo a tabela"));
    }
    Ok(None)
}

/// Adiciona uma coluna: ADD COLUMN nativo quando o SQLite aceita, senão reconstrução (chave primária,
/// UNIQUE, coluna gerada STORED, padrão não constante, NOT NULL sem padrão, chave estrangeira com padrão).
pub fn plano_adicionar_coluna(conn: &Connection, tabela: &str, coluna: &ColunaDef) -> Resultado<Plano> {
    let (nome_t, d) = def_tabela(conn, tabela)?;
    let mut c = normalizar_coluna(coluna);
    if !preenchido(&c.nome) {
        return Err(ErroBanco::novo("Toda coluna precisa de um nome."));
    }
    c.nome = aparar(&c.nome).to_string();
    let baixo = c.nome.to_lowercase();
    if d.colunas.iter().any(|x| x.nome.to_lowercase() == baixo) {
        return Err(ErroBanco::com_status(format!("A tabela {nome_t} já tem uma coluna chamada {}.", c.nome), 409));
    }
    if c.autoincremento {
        return Err(ErroBanco::novo(
            "Autoincremento só pode ser definido ao criar a tabela, na chave primária INTEGER.",
        ));
    }
    if c.nao_nulo
        && padrao_nulo(&c)
        && c.gerada.is_none()
        && inteiro(conn, &format!("SELECT EXISTS (SELECT 1 FROM {})", q(&nome_t)), []).no_preparo()? != 0
    {
        return Err(ErroBanco::novo(format!(
            "A coluna {} não aceita nulo, então precisa de um valor padrão: a tabela já tem linhas, e elas ficariam \
             sem valor.",
            c.nome
        )));
    }
    let trecho = montar_coluna(&c)?;
    let tipo = if c.tipo.is_empty() { "sem tipo" } else { c.tipo.as_str() };
    let descricao = format!("Adicionar a coluna {} ({tipo}) em {nome_t}", c.nome);
    let mut motivo = motivo_sem_add_nativo(conn, &c)?;
    if motivo.is_none() {
        let comando = format!("ALTER TABLE {} ADD COLUMN {trecho}", q(&nome_t));
        match ensaiar(conn, std::slice::from_ref(&comando), "Não foi possível adicionar a coluna.") {
            Ok(()) => {
                let mut avisos = Vec::new();
                if !padrao_nulo(&c) {
                    avisos.push(format!(
                        "As linhas que já existem recebem o valor padrão {}.",
                        c.padrao.as_deref().unwrap_or("")
                    ));
                }
                return Ok(Plano::completo(format!("{descricao}."), vec![comando], false, avisos));
            }
            Err(erro) => {
                if !erro.detalhe.as_deref().unwrap_or("").to_lowercase().starts_with("cannot add a") {
                    return Err(erro);
                }
                motivo = Some("o ADD COLUMN do SQLite recusou esta definição");
            }
        }
    }
    let st = estado(conn, &nome_t)?;
    let mut colunas: Vec<ColunaDef> = st.def.colunas.iter().map(DefColuna::como_dict).collect();
    colunas.push(c);
    let (restricoes, sufixo) = (st.def.restricoes.clone(), st.def.sufixo.clone());
    reconstrucao(
        conn,
        &nome_t,
        &st,
        &colunas,
        &restricoes,
        &sufixo,
        format!("{descricao}: {}. A tabela será reconstruída.", motivo.unwrap_or_default()),
        Vec::new(),
        Vec::new(),
    )
}

// ---------------------------------------------------------------------------------------------------
// Alterar

/// Muda uma coluna. Só o nome → RENAME COLUMN nativo; outra mudança → reconstrução (com o RENAME COLUMN
/// antes, na mesma transação, se o nome também muda). Nada muda → plano sem comandos.
pub fn plano_alterar_coluna(conn: &Connection, tabela: &str, coluna: &str, nova: &ColunaDef) -> Resultado<Plano> {
    let (nome_t, d) = def_tabela(conn, tabela)?;
    let (pos, atual) = achar_coluna(&d, coluna)?;
    let velha = normalizar_coluna(&atual.como_dict());
    let mut nova = normalizar_coluna(nova);
    if !preenchido(&nova.nome) {
        return Err(ErroBanco::novo("Toda coluna precisa de um nome."));
    }
    nova.nome =
        if aparar(&nova.nome) == aparar(&velha.nome) { velha.nome.clone() } else { aparar(&nova.nome).to_string() };
    let renomeia = nova.nome != velha.nome;
    let baixo = nova.nome.to_lowercase();
    if renomeia && d.colunas.iter().enumerate().any(|(k, x)| k != pos && x.nome.to_lowercase() == baixo) {
        return Err(ErroBanco::com_status(format!("A tabela {nome_t} já tem uma coluna chamada {}.", nova.nome), 409));
    }
    let mudancas = diferencas(&velha, &nova);
    if !renomeia && mudancas.is_empty() {
        return Ok(Plano::completo(format!("Nada muda na coluna {}.", velha.nome), Vec::new(), false, Vec::new()));
    }
    let renomear = format!("ALTER TABLE {} RENAME COLUMN {} TO {}", q(&nome_t), q(&velha.nome), q(&nova.nome));
    if mudancas.is_empty() {
        ensaiar(conn, std::slice::from_ref(&renomear), "Não foi possível renomear a coluna.")?;
        return Ok(Plano::completo(
            format!("Renomear a coluna {} de {nome_t} para {}.", velha.nome, nova.nome),
            vec![renomear],
            false,
            vec!["Índices, gatilhos e visões que citam a coluna passam a usar o nome novo.".to_string()],
        ));
    }

    if nova.autoincremento && !(nova.pk && nova.tipo.to_uppercase() == "INTEGER") {
        return Err(ErroBanco::novo("Autoincremento só vale numa chave primária INTEGER."));
    }
    let mut avisos = Vec::new();
    let col = q(&velha.nome);
    let tabela_q = q(&nome_t);
    if nova.nao_nulo && !velha.nao_nulo && nova.gerada.is_none() {
        let nulos = inteiro(conn, &format!("SELECT count(*) FROM {tabela_q} WHERE {col} IS NULL"), []).no_preparo()?;
        if nulos != 0 {
            avisos.push(format!(
                "Há {} com {} nulo; linhas com valor nulo impedem NOT NULL, e a mudança vai falhar enquanto elas \
                 existirem.",
                plural(nulos, "linha", "linhas"),
                velha.nome
            ));
        }
    }
    if (nova.unica && !velha.unica) || (nova.pk && !velha.pk) {
        let sql = format!(
            "SELECT count(*) FROM (SELECT 1 FROM {tabela_q} WHERE {col} IS NOT NULL GROUP BY {col} HAVING count(*) > 1)"
        );
        let repetidos = inteiro(conn, &sql, []).no_preparo()?;
        if repetidos != 0 {
            avisos.push(format!(
                "Há {} em {}; a mudança vai falhar enquanto houver repetição.",
                plural(repetidos, "valor repetido", "valores repetidos"),
                velha.nome
            ));
        }
    }
    let afinidade_nova = afinidade(Some(&nova.tipo));
    if afinidade(Some(&velha.tipo)) != afinidade_nova && nova.gerada.is_none() {
        avisos.push(format!(
            "Os valores são copiados e convertidos pela afinidade do tipo novo ({}).",
            afinidade_nova.como_str()
        ));
    }
    if nova.gerada.is_some() && velha.gerada.is_none() {
        avisos.push("A coluna passa a ser calculada: os valores gravados nela se perdem.".to_string());
    }
    if nova.pk != velha.pk && !tabelas_que_referenciam(conn, &nome_t)?.is_empty() {
        avisos.push(
            "Outras tabelas têm chave estrangeira para esta; se alguma ficar sem correspondência, a verificação no \
             fim desfaz tudo."
                .to_string(),
        );
    }

    let antes = if renomeia { vec![renomear] } else { Vec::new() };
    let st = estado_apos(conn, &nome_t, &antes)?;
    // Parte da coluna como ficou depois do RENAME COLUMN (que reescreve o CHECK e a expressão gerada com o
    // nome novo) e aplica só os campos que a pessoa mudou.
    let Some(depois) = st.def.colunas.get(pos) else {
        return Err(ErroBanco::com_status(format!("A coluna {coluna} não existe em {nome_t}."), 404));
    };
    let mut alvo = normalizar_coluna(&depois.como_dict());
    aplicar_mudancas(&mut alvo, &velha, &nova);
    let colunas: Vec<ColunaDef> =
        st.def.colunas.iter().enumerate().map(|(k, x)| if k == pos { alvo.clone() } else { x.como_dict() }).collect();
    let mut frases = if renomeia { vec![format!("nome {} → {}", velha.nome, nova.nome)] } else { Vec::new() };
    frases.extend(mudancas);
    let (restricoes, sufixo) = (st.def.restricoes.clone(), st.def.sufixo.clone());
    reconstrucao(
        conn,
        &nome_t,
        &st,
        &colunas,
        &restricoes,
        &sufixo,
        format!("Mudar a coluna {} de {nome_t}: {}. A tabela será reconstruída.", velha.nome, frases.join("; ")),
        avisos,
        antes,
    )
}

// ---------------------------------------------------------------------------------------------------
// Excluir

/// Frase dizendo o que impede excluir a coluna e o que remover antes (None = nada impede).
fn impedimento_exclusao(
    conn: &Connection,
    tabela: &str,
    d: &DefTabela,
    pos_col: usize,
    col: &DefColuna,
) -> Resultado<Option<String>> {
    let nc = &col.nome;
    let baixo = col.nome.to_lowercase();
    if d.colunas.len() == 1 {
        return Ok(Some(format!(
            "A coluna {nc} é a única de {tabela}; uma tabela precisa de ao menos uma coluna. Se for o caso, exclua a \
             tabela."
        )));
    }
    let na_pk = xinfo(conn, tabela)?.iter().rev().find(|r| r.nome.to_lowercase() == baixo).is_some_and(|r| r.pk != 0);
    if na_pk {
        return Ok(Some(format!("A coluna {nc} faz parte da chave primária de {tabela} e não pode ser excluída.")));
    }
    let indices = linhas(conn, &format!("PRAGMA index_list({})", q(tabela)), [], |r| Ok((texto(r, 1)?, texto(r, 3)?)))
        .no_preparo()?;
    for (nome_i, origem) in indices {
        let mut nomes: BTreeSet<String> = linhas(conn, &format!("PRAGMA index_xinfo({})", q(&nome_i)), [], |r| {
            Ok((texto(r, 2)?.to_lowercase(), crate::sql::inteiro_em(r, 5)?))
        })
        .no_preparo()?
        .into_iter()
        .filter(|(_, chave)| *chave != 0)
        .map(|(nome, _)| nome)
        .collect();
        // a chave primária de uma tabela WITHOUT ROWID aparece no index_list mas não em sqlite_schema
        let sql_i = primeira(conn, "SELECT sql FROM sqlite_schema WHERE name = ?", [&nome_i], |r| texto_opcional(r, 0))
            .no_preparo()?
            .flatten();
        if let Some(sql_i) = sql_i.filter(|s| !s.is_empty()) {
            let toks = significativos(&sql_i)?;
            if let Some(abre) = toks.iter().find(|t| t.texto == "(") {
                nomes.extend(colunas_da_expressao(fatia(&sql_i, abre.inicio, sql_i.len()))?);
            }
        }
        if !nomes.contains(&baixo) {
            continue;
        }
        if origem == "c" {
            return Ok(Some(format!("A coluna {nc} é usada pelo índice {nome_i}. Exclua o índice antes.")));
        }
        if origem == "u" && col.unica {
            return Ok(Some(format!(
                "A coluna {nc} não aceita repetição (UNIQUE). Tire essa regra da coluna antes (isso reconstrói a \
                 tabela)."
            )));
        }
        if origem == "u" {
            return Ok(Some(format!(
                "A coluna {nc} faz parte de uma restrição UNIQUE da tabela ({nome_i}). Tire a restrição antes."
            )));
        }
    }
    for fk in chaves_estrangeiras(conn, tabela).no_preparo()? {
        if fk.de.to_lowercase() == baixo {
            return Ok(Some(format!(
                "A coluna {nc} é uma chave estrangeira para {}. Tire a referência antes.",
                fk.tabela
            )));
        }
    }
    for (filha, alvo, _) in tabelas_que_referenciam(conn, tabela)? {
        if alvo.unwrap_or_default().to_lowercase() == baixo {
            return Ok(Some(format!(
                "A coluna {nc} é referenciada pela chave estrangeira de {filha}. Tire a referência de {filha} antes."
            )));
        }
    }
    for r in &d.restricoes {
        if colunas_da_restricao(r, tabela)?.contains(&baixo) {
            return Ok(Some(format!("A coluna {nc} aparece na restrição da tabela «{r}». Tire a restrição antes.")));
        }
    }
    for (k, outra) in d.colunas.iter().enumerate() {
        if k == pos_col {
            continue;
        }
        if let Some(g) = &outra.gerada
            && colunas_da_expressao(&g.expressao)?.contains(&baixo)
        {
            return Ok(Some(format!(
                "A coluna gerada {} depende de {nc}. Exclua-a ou mude a expressão dela antes.",
                outra.nome
            )));
        }
        if let Some(v) = &outra.verificacao
            && colunas_da_expressao(v)?.contains(&baixo)
        {
            return Ok(Some(format!("A regra de verificação da coluna {} usa {nc}. Mude a regra antes.", outra.nome)));
        }
    }
    // Gatilhos de qualquer tabela: os que gravam na coluna (INSERT INTO t (…), UPDATE t SET …, UPDATE OF …)
    // passam no DROP COLUMN do SQLite e quebram depois; os que só a leem, o ensaio do DROP COLUMN recusa.
    let gatilhos =
        linhas(conn, "SELECT name, tbl_name, sql FROM sqlite_schema WHERE type = 'trigger' ORDER BY rowid", [], |r| {
            Ok((texto(r, 0)?, texto(r, 1)?, texto(r, 2)?))
        })
        .no_preparo()?;
    let tabela_baixo = tabela.to_lowercase();
    for (nome_g, tbl_g, sql_g) in gatilhos {
        let gravadas = colunas_gravadas_pelo_gatilho(&sql_g)?;
        if gravadas.get(&tabela_baixo).is_some_and(|cols| cols.contains(&baixo)) {
            let de = if tbl_g.to_lowercase() == tabela_baixo { String::new() } else { format!(" (de {tbl_g})") };
            return Ok(Some(format!("A coluna {nc} é usada pelo gatilho {nome_g}{de}. Exclua o gatilho antes.")));
        }
    }
    Ok(None)
}

/// Exclui uma coluna com o DROP COLUMN nativo. O que impede (chave primária, índice, UNIQUE, chave
/// estrangeira, restrição da tabela, coluna gerada, CHECK de outra coluna, gatilho que grava nela, visão)
/// vira [`ErroBanco`] dizendo o que remover antes.
pub fn plano_excluir_coluna(conn: &Connection, tabela: &str, coluna: &str) -> Resultado<Plano> {
    let (nome_t, d) = def_tabela(conn, tabela)?;
    let (pos, col) = achar_coluna(&d, coluna)?;
    if let Some(motivo) = impedimento_exclusao(conn, &nome_t, &d, pos, col)? {
        return Err(ErroBanco::novo(motivo));
    }
    let comando = format!("ALTER TABLE {} DROP COLUMN {}", q(&nome_t), q(&col.nome));
    let nome_col = col.nome.clone();
    let tradutor = move |e: &rusqlite::Error, contexto: &str| -> ErroBanco {
        let original = e.to_string();
        if let Some(m) = erro_de_objeto(&original) {
            let (artigo, objeto) = match m.tipo {
                "view" => ("pela visão", "a visão"),
                "trigger" => ("pelo gatilho", "o gatilho"),
                "index" => ("pelo índice", "o índice"),
                _ => return traduzir(e, contexto),
            };
            let mensagem = format!("A coluna {nome_col} é usada {artigo} {}. Exclua {objeto} antes.", m.nome);
            return ErroBanco::novo(mensagem).detalhe(original);
        }
        traduzir(e, contexto)
    };
    ensaiar_com(conn, std::slice::from_ref(&comando), "Não foi possível excluir a coluna.", &tradutor)?;
    Ok(Plano::completo(
        format!("Excluir a coluna {} de {nome_t}.", col.nome),
        vec![comando],
        false,
        vec!["Colunas removidas perdem os dados.".to_string()],
    ))
}

// ---------------------------------------------------------------------------------------------------
// Reordenar

/// Põe as colunas na ordem dada (os nomes, sem diferenciar maiúsculas; cada coluna exatamente uma vez).
/// Mesma ordem → plano sem comandos; senão, reconstrução.
pub fn plano_reordenar_colunas<S: AsRef<str>>(conn: &Connection, tabela: &str, ordem: &[S]) -> Resultado<Plano> {
    let (nome_t, d) = def_tabela(conn, tabela)?;
    let atuais: BTreeSet<String> = d.colunas.iter().map(|c| c.nome.to_lowercase()).collect();
    let pedidos: Vec<String> = ordem.iter().map(|o| o.as_ref().to_lowercase()).collect();
    let pedidos_unicos: BTreeSet<&String> = pedidos.iter().collect();
    if pedidos.len() != atuais.len()
        || pedidos_unicos.len() != atuais.len()
        || !pedidos.iter().all(|p| atuais.contains(p))
    {
        return Err(ErroBanco::novo(
            "A nova ordem precisa ter exatamente as mesmas colunas da tabela, cada uma uma vez.",
        ));
    }
    // a última coluna com o nome vale
    let achar = |cols: &[DefColuna], nome: &str| cols.iter().rev().find(|c| c.nome.to_lowercase() == nome).cloned();
    let novas: Vec<String> = pedidos.iter().filter_map(|p| achar(&d.colunas, p)).map(|c| c.nome).collect();
    if novas == d.colunas.iter().map(|c| c.nome.clone()).collect::<Vec<_>>() {
        return Ok(Plano::completo(
            format!("A ordem das colunas de {nome_t} não muda."),
            Vec::new(),
            false,
            Vec::new(),
        ));
    }
    let st = estado(conn, &nome_t)?;
    let colunas: Vec<ColunaDef> =
        pedidos.iter().filter_map(|p| achar(&st.def.colunas, p)).map(|c| c.como_dict()).collect();
    let nomes: Vec<&str> = colunas.iter().map(|c| c.nome.as_str()).collect();
    let (restricoes, sufixo) = (st.def.restricoes.clone(), st.def.sufixo.clone());
    reconstrucao(
        conn,
        &nome_t,
        &st,
        &colunas,
        &restricoes,
        &sufixo,
        format!("Reordenar as colunas de {nome_t}: {}. A tabela será reconstruída.", nomes.join(", ")),
        Vec::new(),
        Vec::new(),
    )
}
