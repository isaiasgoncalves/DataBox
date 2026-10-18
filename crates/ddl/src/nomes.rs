//! Nomes que um comando, uma expressão, uma restrição ou um gatilho citam (para saber o que depende de
//! uma tabela ou coluna antes de mudá-la).

use std::collections::{BTreeMap, BTreeSet};

use comum::Resultado;

use crate::tokens::{TipoToken, Token, desfazer_aspas, e_nome, fecha, significativos};

/// Nomes (sem aspas, em minúsculas) de todas as palavras e identificadores do comando.
pub fn citados(sql: &str) -> Resultado<BTreeSet<String>> {
    Ok(significativos(sql)?
        .iter()
        .filter(|t| matches!(t.tipo, TipoToken::Palavra | TipoToken::Ident))
        .map(|t| desfazer_aspas(&t.texto).to_lowercase())
        .collect())
}

/// Palavras-chave que aparecem dentro de expressões (CHECK, coluna gerada, índice) e não são nomes de coluna.
const PALAVRAS_EXPRESSAO: [&str; 39] = [
    "AND",
    "OR",
    "NOT",
    "NULL",
    "IS",
    "IN",
    "BETWEEN",
    "LIKE",
    "GLOB",
    "REGEXP",
    "MATCH",
    "ESCAPE",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "CAST",
    "AS",
    "COLLATE",
    "EXISTS",
    "DISTINCT",
    "TRUE",
    "FALSE",
    "ISNULL",
    "NOTNULL",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "ASC",
    "DESC",
    "SELECT",
    "FROM",
    "WHERE",
    "RAISE",
    "ABORT",
    "FAIL",
    "IGNORE",
    "ROLLBACK",
];

/// Nomes de coluna (sem aspas, em minúsculas) que uma expressão cita: fora palavras-chave, nomes de função
/// (seguidos de "("), qualificadores ("t." de t.coluna), colações e o tipo de um CAST.
fn nomes_na_expressao(toks: &[Token]) -> BTreeSet<String> {
    let mut nomes = BTreeSet::new();
    for (k, t) in toks.iter().enumerate() {
        if !matches!(t.tipo, TipoToken::Palavra | TipoToken::Ident) {
            continue;
        }
        let antes = k.checked_sub(1).and_then(|a| toks.get(a));
        let depois = toks.get(k + 1);
        if antes.is_some_and(|a| a.e(&["COLLATE", "AS"])) {
            continue;
        }
        if depois.is_some_and(|d| d.e_op("(") || d.e_op(".")) {
            continue;
        }
        if t.tipo == TipoToken::Palavra && (t.e(&PALAVRAS_EXPRESSAO) || t.texto.starts_with([':', '@', '$'])) {
            continue;
        }
        nomes.insert(desfazer_aspas(&t.texto).to_lowercase());
    }
    nomes
}

/// Nomes de coluna que uma expressão (CHECK, coluna gerada, índice) cita, sem as palavras-chave dela.
pub fn colunas_da_expressao(expr: &str) -> Resultado<BTreeSet<String>> {
    Ok(nomes_na_expressao(&significativos(expr)?))
}

/// Colunas de `tabela` que uma restrição de tabela usa (PRIMARY KEY (…), UNIQUE (…), CHECK (…),
/// FOREIGN KEY (…)). Todo uso de coluna fica entre parênteses; o que está fora (palavras-chave, nome da
/// CONSTRAINT, tabela referenciada) não conta, nem a lista de REFERENCES outra (…) de outra tabela.
pub fn colunas_da_restricao(texto: &str, tabela: &str) -> Resultado<BTreeSet<String>> {
    let toks = significativos(texto)?;
    let tabela = tabela.to_lowercase();
    let mut nomes = BTreeSet::new();
    let mut i = 0;
    while let Some(t) = toks.get(i) {
        if t.e_op("(") {
            let z = fecha(&toks, i)?;
            let de_outra = i >= 2
                && toks.get(i - 2).is_some_and(|r| r.e(&["REFERENCES"]))
                && toks.get(i - 1).is_some_and(|nome| desfazer_aspas(&nome.texto).to_lowercase() != tabela);
            if !de_outra {
                nomes.extend(nomes_na_expressao(toks.get(i + 1..z).unwrap_or(&[])));
            }
            i = z + 1;
        } else {
            i += 1;
        }
    }
    Ok(nomes)
}

const FIM_DO_SET: [&str; 5] = ["WHERE", "FROM", "RETURNING", "ORDER", "LIMIT"];

/// `{tabela: {colunas}}` (sem aspas, em minúsculas) que um CREATE TRIGGER cita como alvo: a lista do
/// UPDATE OF do cabeçalho, as listas de INSERT INTO t (…) e as colunas à esquerda do "=" no SET de
/// UPDATE t e de ON CONFLICT … DO UPDATE.
///
/// São os usos que o ALTER TABLE … DROP COLUMN do SQLite não confere (ele aceita, e o gatilho quebra na
/// próxima gravação); os outros (new.x, SELECT x, WHERE x…) ele mesmo recusa.
pub fn colunas_gravadas_pelo_gatilho(sql: &str) -> Resultado<BTreeMap<String, BTreeSet<String>>> {
    let toks = significativos(sql)?;
    let n = toks.len();
    let mut gravadas: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Nome da tabela em toks[k] (pulando "esquema.") e o índice logo depois dele; None se acabou o texto.
    let nome_em = |k: usize| -> Option<(String, usize)> {
        let k = if k + 2 < n && toks.get(k + 1).is_some_and(|t| t.texto == ".") { k + 2 } else { k };
        toks.get(k).map(|t| (desfazer_aspas(&t.texto).to_lowercase(), k + 1))
    };
    let anotar = |gravadas: &mut BTreeMap<String, BTreeSet<String>>, tabela: &str, nomes: &[Token]| {
        gravadas
            .entry(tabela.to_string())
            .or_default()
            .extend(nomes.iter().filter(|t| e_nome(t)).map(|t| desfazer_aspas(&t.texto).to_lowercase()));
    };

    let corpo = toks.iter().position(|t| t.e(&["BEGIN"])).unwrap_or(n);
    // cabeçalho: … UPDATE OF a, b ON tabela …
    for k in 0..corpo {
        if toks.get(k).is_some_and(|t| t.e(&["UPDATE"]))
            && k + 1 < corpo
            && toks.get(k + 1).is_some_and(|t| t.e(&["OF"]))
        {
            let fim = (k + 2..corpo).find(|&j| toks.get(j).is_some_and(|t| t.e(&["ON"])));
            if let Some(fim) = fim
                && fim + 1 < corpo
                && let Some((tabela, _)) = nome_em(fim + 1)
            {
                anotar(&mut gravadas, &tabela, toks.get(k + 2..fim).unwrap_or(&[]));
            }
            break;
        }
    }

    // tabela do INSERT em curso, alvo de um ON CONFLICT … DO UPDATE SET
    let mut inserida: Option<String> = None;
    let mut k = corpo + 1;
    while let Some(t) = toks.get(k) {
        if t.e(&["INTO"]) && toks.get(k + 1).is_some_and(e_nome) {
            let Some((tabela, depois)) = nome_em(k + 1) else { break };
            k = depois;
            if toks.get(k).is_some_and(|u| u.texto == "(") {
                let z = fecha(&toks, k)?;
                anotar(&mut gravadas, &tabela, toks.get(k + 1..z).unwrap_or(&[]));
                k = z + 1;
            }
            inserida = Some(tabela);
            continue;
        }
        if t.e(&["UPDATE"]) && k + 1 < n {
            let alvo;
            if k > 0 && toks.get(k - 1).is_some_and(|d| d.e(&["DO"])) {
                alvo = inserida.clone();
                k += 1;
            } else {
                let mut j = k + 1;
                if j + 1 < n && toks.get(j).is_some_and(|o| o.e(&["OR"])) {
                    j += 2;
                }
                // "UPDATE OR x" no fim do texto: acaba a leitura.
                let Some((tabela, depois)) = nome_em(j) else { break };
                alvo = Some(tabela);
                k = depois;
            }
            let Some(alvo) = alvo else { continue };
            if !toks.get(k).is_some_and(|s| s.e(&["SET"])) {
                continue;
            }
            k += 1;
            let mut inicio_item = true;
            let mut prof: i64 = 0;
            let mut casos: i64 = 0;
            while let Some(u) = toks.get(k) {
                if prof == 0 && casos == 0 && (u.texto == ";" || u.e(&FIM_DO_SET)) {
                    break;
                }
                if inicio_item {
                    if u.texto == "(" {
                        let z = fecha(&toks, k)?;
                        anotar(&mut gravadas, &alvo, toks.get(k + 1..z).unwrap_or(&[]));
                        k = z + 1;
                    } else {
                        anotar(&mut gravadas, &alvo, std::slice::from_ref(u));
                        k += 1;
                    }
                    inicio_item = false;
                    continue;
                }
                if u.texto == "(" {
                    prof += 1;
                } else if u.texto == ")" {
                    prof -= 1;
                } else if u.e(&["CASE"]) {
                    casos += 1;
                } else if u.e(&["END"]) && casos != 0 {
                    casos -= 1;
                } else if u.texto == "," && prof == 0 && casos == 0 {
                    inicio_item = true;
                }
                k += 1;
            }
            continue;
        }
        if t.texto == ";" {
            inserida = None;
        }
        k += 1;
    }
    Ok(gravadas)
}
