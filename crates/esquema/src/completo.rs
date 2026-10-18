//! A máquina de estados do `sqlite3_complete` do SQLite sobre o
//! tokenizador do ddl e sem FFI: diz se um texto termina num comando completo e onde termina o primeiro
//! comando (para recusar um texto com mais de um).

use ddl::{TipoToken, Token, tokenizar};

#[derive(Clone, Copy)]
enum Tk {
    Semi = 0,
    Ws,
    Outro,
    Explain,
    Create,
    Temp,
    Trigger,
    End,
}

/// Estados: 0 inválido, 1 início, 2 normal, 3 EXPLAIN, 4 CREATE, 5 gatilho, 6 ";" no gatilho, 7 "END" no
/// gatilho (a tabela `trans` de complete.c).
const TRANS: [[u8; 8]; 8] = [
    // SEMI WS OUTRO EXPLAIN CREATE TEMP TRIGGER END
    [1, 0, 2, 3, 4, 2, 2, 2],
    [1, 1, 2, 3, 4, 2, 2, 2],
    [1, 2, 2, 2, 2, 2, 2, 2],
    [1, 3, 3, 2, 4, 2, 2, 2],
    [1, 4, 2, 2, 2, 4, 5, 2],
    [6, 5, 5, 5, 5, 5, 5, 5],
    [6, 6, 5, 5, 5, 5, 5, 7],
    [1, 7, 5, 5, 5, 5, 5, 5],
];

fn classe(t: &Token) -> Tk {
    match t.tipo {
        TipoToken::Espaco | TipoToken::Comentario => Tk::Ws,
        TipoToken::Op if t.texto == ";" => Tk::Semi,
        TipoToken::Palavra => {
            let p = t.texto.as_str();
            if p.eq_ignore_ascii_case("CREATE") {
                Tk::Create
            } else if p.eq_ignore_ascii_case("TRIGGER") {
                Tk::Trigger
            } else if p.eq_ignore_ascii_case("TEMP") || p.eq_ignore_ascii_case("TEMPORARY") {
                Tk::Temp
            } else if p.eq_ignore_ascii_case("END") {
                Tk::End
            } else if p.eq_ignore_ascii_case("EXPLAIN") {
                Tk::Explain
            } else {
                Tk::Outro
            }
        }
        _ => Tk::Outro,
    }
}

fn proximo(estado: usize, t: &Token) -> usize {
    usize::from(TRANS[estado][classe(t) as usize])
}

/// O texto termina num ";" que fecha um comando; dentro de um CREATE TRIGGER, só o ";" depois de "; END".
/// Texto, nome citado ou comentario `/*` sem fechamento → falso.
pub(crate) fn instrucao_completa(sql: &str) -> bool {
    let Ok(tokens) = tokenizar(sql) else { return false };
    tokens.iter().fold(0, proximo) == 1
}

/// Onde termina o primeiro comando (em bytes, depois do ";" que o fecha) e se depois dele vem algo além de
/// espaços e comentários. Texto que não tokeniza: o texto inteiro (o SQLite acusa o erro).
pub(crate) fn primeiro_comando(sql: &str) -> (usize, bool) {
    let Ok(tokens) = tokenizar(sql) else { return (sql.len(), false) };
    let mut estado = 0;
    for (k, t) in tokens.iter().enumerate() {
        estado = proximo(estado, t);
        if estado == 1 && t.e_op(";") {
            let resto = tokens[k + 1..].iter().any(|r| !matches!(r.tipo, TipoToken::Espaco | TipoToken::Comentario));
            return (t.fim, resto);
        }
    }
    (sql.len(), false)
}
