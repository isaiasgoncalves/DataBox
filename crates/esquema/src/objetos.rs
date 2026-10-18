//! Índices, visões e gatilhos.

use std::collections::{BTreeSet, HashMap};

use comum::{ErroBanco, Resultado, q, traduzir_sqlite};
use ddl::{Token, aparar, desfazer_aspas, fatia, preenchido, significativos, validar_trecho};
use rusqlite::Connection;

use crate::completo::instrucao_completa;
use crate::leitura::{TIPOS_NOME, exigir_tabela, nome_livre, objeto, xinfo};
use crate::plano::{Plano, ensaiar, ensaiar_com};
use crate::sql::{NoPreparo, linhas, recusar_nulo, rodar, texto, traduzir};

/// Uma coluna de um índice novo: o nome e se a ordem é decrescente.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColunaIndice {
    pub nome: String,
    pub desc: bool,
}

impl ColunaIndice {
    /// Coluna em ordem crescente.
    pub fn nova(nome: impl Into<String>) -> ColunaIndice {
        ColunaIndice { nome: nome.into(), desc: false }
    }
}

impl From<&str> for ColunaIndice {
    fn from(nome: &str) -> Self {
        ColunaIndice::nova(nome)
    }
}

/// Cria um índice (único com `unico`; parcial com `onde`, a condição do WHERE).
pub fn plano_criar_indice(
    conn: &Connection,
    nome: &str,
    tabela: &str,
    colunas: &[ColunaIndice],
    unico: bool,
    onde: Option<&str>,
) -> Resultado<Plano> {
    let nome = nome_livre(conn, nome, "do índice", &TIPOS_NOME)?;
    let (nome_t, _) = exigir_tabela(conn, tabela, false)?;
    let reais: HashMap<String, String> =
        xinfo(conn, &nome_t)?.into_iter().map(|r| (r.nome.to_lowercase(), r.nome)).collect();
    if colunas.is_empty() {
        return Err(ErroBanco::novo("Escolha ao menos uma coluna para o índice."));
    }
    let mut partes = Vec::new();
    let mut legivel = Vec::new();
    let mut vistas: BTreeSet<String> = BTreeSet::new();
    for c in colunas {
        let pedido = &c.nome;
        let real = match reais.get(&pedido.to_lowercase()) {
            Some(real) if preenchido(pedido) => real,
            _ => return Err(ErroBanco::novo(format!("A coluna {pedido} não existe em {nome_t}."))),
        };
        if !vistas.insert(real.to_lowercase()) {
            return Err(ErroBanco::novo(format!("A coluna {real} aparece mais de uma vez no índice.")));
        }
        let ordem = if c.desc { " DESC" } else { "" };
        partes.push(format!("{}{ordem}", q(real)));
        legivel.push(format!("{real}{ordem}"));
    }
    let mut comando = format!(
        "CREATE {}INDEX {} ON {} ({})",
        if unico { "UNIQUE " } else { "" },
        q(&nome),
        q(&nome_t),
        partes.join(", ")
    );
    let mut descricao =
        format!("Criar o índice {}{nome} em {nome_t} ({})", if unico { "único " } else { "" }, legivel.join(", "));
    if let Some(onde) = onde.filter(|o| preenchido(o)) {
        let condicao = validar_trecho(onde, "A condição do índice")?;
        comando.push_str(&format!(" WHERE {condicao}"));
        descricao.push_str(&format!(", só nas linhas em que {}", aparar(&condicao)));
    }
    let legivel_txt = legivel.join(", ");
    let tradutor = move |e: &rusqlite::Error, contexto: &str| -> ErroBanco {
        let original = e.to_string();
        if unico && original.to_lowercase().contains("unique constraint failed") {
            return ErroBanco::novo(format!(
                "Não é possível criar o índice único: há linhas com valores repetidos em {legivel_txt}."
            ))
            .detalhe(original);
        }
        traduzir(e, contexto)
    };
    ensaiar_com(conn, std::slice::from_ref(&comando), "Não foi possível criar o índice.", &tradutor)?;
    let mut avisos =
        vec!["O índice acelera buscas e ordenações por essas colunas; o arquivo fica um pouco maior.".to_string()];
    if unico {
        avisos.push("Com o índice único, o banco passa a recusar valores repetidos nessas colunas.".to_string());
    }
    Ok(Plano::completo(format!("{descricao}."), vec![comando], false, avisos))
}

/// Exclui um índice criado por CREATE INDEX (o de uma restrição UNIQUE ou da chave primária é recusado).
pub fn plano_excluir_indice(conn: &Connection, nome: &str) -> Resultado<Plano> {
    if !preenchido(nome) {
        return Err(ErroBanco::novo("Informe o nome do índice."));
    }
    let Some(obj) = objeto(conn, nome, &["index"])? else {
        return Err(ErroBanco::com_status(format!("O índice {nome} não existe."), 404));
    };
    if obj.nome.to_lowercase().starts_with("sqlite_autoindex_") || obj.sql.as_deref().is_none_or(str::is_empty) {
        return Err(ErroBanco::novo(
            "Esse índice vem de uma restrição UNIQUE ou da chave primária; ele sai junto quando a restrição sai.",
        ));
    }
    Ok(Plano::completo(
        format!("Excluir o índice {} de {}.", obj.nome, obj.tabela),
        vec![format!("DROP INDEX {}", q(&obj.nome))],
        false,
        vec!["Os dados não mudam; só as buscas que usavam o índice podem ficar mais lentas.".to_string()],
    ))
}

/// Os tokens sem os ";" do fim.
fn sem_ponto_e_virgula_final(mut toks: Vec<Token>) -> Vec<Token> {
    while toks.last().is_some_and(|t| t.texto == ";") {
        toks.pop();
    }
    toks
}

fn select_unico(sql: &str) -> Resultado<String> {
    if !preenchido(sql) {
        return Err(ErroBanco::novo("Escreva o SELECT da visão."));
    }
    let toks = sem_ponto_e_virgula_final(significativos(sql)?);
    let (Some(primeiro), Some(ultimo)) = (toks.first(), toks.last()) else {
        return Err(ErroBanco::novo("A visão precisa de um único comando SELECT."));
    };
    if !primeiro.e(&["SELECT", "WITH", "VALUES"]) || toks.iter().any(|t| t.texto == ";") {
        return Err(ErroBanco::novo("A visão precisa de um único comando SELECT."));
    }
    Ok(fatia(sql, primeiro.inicio, ultimo.fim).to_string())
}

/// Cria uma visão a partir de um único SELECT (WITH e VALUES também valem; ";" no fim é tirado).
pub fn plano_criar_visao(conn: &Connection, nome: &str, select_sql: &str) -> Resultado<Plano> {
    let nome = nome_livre(conn, nome, "da visão", &TIPOS_NOME)?;
    let corpo = select_unico(select_sql)?;
    // prepara sem rodar: acusa tabela ou coluna que não existe
    rodar(conn, &format!("EXPLAIN {corpo}"))
        .map_err(|e| traduzir_sqlite(&e, Some("O SELECT da visão não é válido.")))?;
    let comando = format!("CREATE VIEW {} AS\n{corpo}", q(&nome));
    ensaiar(conn, std::slice::from_ref(&comando), "Não foi possível criar a visão.")?;
    Ok(Plano::completo(
        format!("Criar a visão {nome}."),
        vec![comando],
        false,
        vec!["A visão guarda só a consulta; os dados continuam nas tabelas.".to_string()],
    ))
}

/// Exclui uma visão (os gatilhos INSTEAD OF dela saem junto, e o aviso diz quais).
pub fn plano_excluir_visao(conn: &Connection, nome: &str) -> Resultado<Plano> {
    if !preenchido(nome) {
        return Err(ErroBanco::novo("Informe o nome da visão."));
    }
    let Some(obj) = objeto(conn, nome, &["view"])? else {
        return Err(ErroBanco::com_status(format!("A visão {nome} não existe."), 404));
    };
    let real = obj.nome;
    let mut avisos = vec!["Os dados das tabelas não mudam; só a consulta salva some.".to_string()];
    let gatilhos =
        linhas(conn, "SELECT name FROM sqlite_schema WHERE type = 'trigger' AND tbl_name = ?", [&real], |r| {
            texto(r, 0)
        })
        .no_preparo()?;
    if !gatilhos.is_empty() {
        avisos.push(format!("Saem junto os gatilhos da visão: {}.", gatilhos.join(", ")));
    }
    Ok(Plano::completo(format!("Excluir a visão {real}."), vec![format!("DROP VIEW {}", q(&real))], false, avisos))
}

const CONTEXTO_GATILHO: &str = "Não foi possível criar o gatilho.";

/// Cria um gatilho a partir do CREATE TRIGGER completo.
pub fn plano_criar_gatilho(conn: &Connection, sql: &str) -> Resultado<Plano> {
    if !preenchido(sql) {
        return Err(ErroBanco::novo("Escreva o comando CREATE TRIGGER."));
    }
    let toks = sem_ponto_e_virgula_final(significativos(sql)?);
    let n = toks.len();
    let mut k = 1;
    if n > 1 && toks[1].e(&["TEMP", "TEMPORARY"]) {
        k = 2;
    }
    let cabecalho_ok = n > k && toks[0].e(&["CREATE"]) && toks[k].e(&["TRIGGER"]);
    if !cabecalho_ok || !toks[n - 1].e(&["END"]) {
        return Err(ErroBanco::novo("O gatilho precisa de um único comando CREATE TRIGGER … BEGIN … END."));
    }
    k += 1;
    if k + 2 < n && toks[k].e(&["IF"]) && toks[k + 1].e(&["NOT"]) && toks[k + 2].e(&["EXISTS"]) {
        k += 3;
    }
    if k >= n {
        return Err(ErroBanco::novo("O CREATE TRIGGER não tem o nome do gatilho."));
    }
    let token_nome = if k + 2 < n && toks[k + 1].texto == "." { &toks[k + 2] } else { &toks[k] };
    let nome = desfazer_aspas(&token_nome.texto);
    nome_livre(conn, &nome, "do gatilho", &["trigger"])?;
    let comando = fatia(sql, toks[0].inicio, toks[n - 1].fim).to_string();
    // Lá, o sqlite3.complete_statement levantava ValueError com NUL (erro inesperado); aqui, a frase dos
    // outros textos com NUL, no mesmo ponto.
    recusar_nulo(&comando).map_err(|e| traduzir(&e, CONTEXTO_GATILHO))?;
    if !instrucao_completa(&format!("{comando};")) {
        return Err(ErroBanco::novo("O comando CREATE TRIGGER está incompleto."));
    }
    ensaiar(conn, std::slice::from_ref(&comando), CONTEXTO_GATILHO)?;
    Ok(Plano::completo(
        format!("Criar o gatilho {nome}."),
        vec![comando],
        false,
        vec!["O gatilho roda sozinho a cada alteração que combina com ele.".to_string()],
    ))
}

/// Exclui um gatilho.
pub fn plano_excluir_gatilho(conn: &Connection, nome: &str) -> Resultado<Plano> {
    if !preenchido(nome) {
        return Err(ErroBanco::novo("Informe o nome do gatilho."));
    }
    let Some(obj) = objeto(conn, nome, &["trigger"])? else {
        return Err(ErroBanco::com_status(format!("O gatilho {nome} não existe."), 404));
    };
    Ok(Plano::completo(
        format!("Excluir o gatilho {} de {}.", obj.nome, obj.tabela),
        vec![format!("DROP TRIGGER {}", q(&obj.nome))],
        false,
        vec!["Os dados não mudam; o gatilho só deixa de rodar nas próximas alterações.".to_string()],
    ))
}
