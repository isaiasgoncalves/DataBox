//! Acesso ao SQLite usado pelo crate: rodar um comando, ler linhas e traduzir erros.

use comum::{ErroBanco, Resultado, formatar_real, traduzir_sqlite};
use ddl::aparar;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, Params, Row, Rows, Statement};

/// Um erro de uso, com a frase dada, antes de o SQLite rodar nada.
fn erro_de_uso(frase: String) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE), Some(frase))
}

/// O erro para um texto com mais de um comando.
fn varios_comandos() -> rusqlite::Error {
    erro_de_uso("You can only execute one statement at a time.".to_string())
}

/// O erro para um texto com o caractere NUL. O SQLite para de ler no NUL: sem esta
/// recusa, o comando rodaria cortado ali (diferente da prévia).
fn caractere_nulo() -> rusqlite::Error {
    erro_de_uso("the query contains a null character".to_string())
}

/// Texto com NUL → erro; vale para todo SQL antes de preparar.
pub(crate) fn recusar_nulo(sql: &str) -> rusqlite::Result<()> {
    if sql.contains('\0') { Err(caractere_nulo()) } else { Ok(()) }
}

/// O rusqlite (com `modern_sqlite`) junta ao texto do SQLite o comando e a posição do erro ("… in SELEC x at
/// offset 0"); a mensagem fica só com o texto do SQLite. Vários comandos num texto só e parâmetros (`?`, `:x`)
/// sem valor viram frases próprias.
pub(crate) fn normalizar(e: rusqlite::Error) -> rusqlite::Error {
    match e {
        rusqlite::Error::SqlInputError { error, msg, .. } => rusqlite::Error::SqliteFailure(error, Some(msg)),
        rusqlite::Error::MultipleStatement => varios_comandos(),
        rusqlite::Error::InvalidParameterCount(dados, usados) => erro_de_uso(format!(
            "Incorrect number of bindings supplied. The current statement uses {usados}, and there are {dados} \
             supplied."
        )),
        outro => outro,
    }
}

/// Quem acompanha cada comando que o SQLite começa a rodar (o teste da prévia usa): recebe o texto do comando que o SQLite **preparou**, com os parâmetros
/// expandidos (`sqlite3_expanded_sql`), e não o texto que o crate pediu para rodar. O rusqlite deste projeto
/// não tem o recurso `trace`; o aviso sai logo antes do primeiro passo, que é quando o SQLite chamaria o
/// rastreador (depois de ligar os parâmetros: um comando recusado ali não aparece).
pub(crate) struct Observador<'a>(Option<&'a mut dyn FnMut(&str)>);

impl<'a> Observador<'a> {
    /// Sem ninguém acompanhando (o caso normal).
    pub fn nenhum() -> Observador<'a> {
        Observador(None)
    }

    #[cfg(test)]
    pub fn novo(observar: &'a mut dyn FnMut(&str)) -> Observador<'a> {
        Observador(Some(observar))
    }

    fn avisar(&mut self, linhas: &Rows<'_>) {
        if let Some(observar) = self.0.as_deref_mut()
            && let Some(sql) = linhas.as_ref().and_then(Statement::expanded_sql)
        {
            observar(&sql);
        }
    }
}

/// A frase posta na frente do erro do SQLite que escapa da montagem de um plano.
pub(crate) const CONTEXTO_PREPARO: &str = "Não foi possível preparar a mudança de estrutura.";

/// O erro do SQLite traduzido sem contexto (PRAGMAs de sessão do `executar`).
pub(crate) fn cru(e: rusqlite::Error) -> ErroBanco {
    traduzir_sqlite(&normalizar(e), None)
}

/// O erro do SQLite da montagem do plano, com [`CONTEXTO_PREPARO`] na frente.
pub(crate) fn no_preparo(e: rusqlite::Error) -> ErroBanco {
    traduzir_sqlite(&normalizar(e), Some(CONTEXTO_PREPARO))
}

/// Tira a frase de [`no_preparo`] (o `definicoes_tabela` é chamado pelo servidor sem ela).
pub(crate) fn sem_frase_de_preparo(mut e: ErroBanco) -> ErroBanco {
    if let Some(resto) = e.mensagem.strip_prefix(CONTEXTO_PREPARO).and_then(|r| r.strip_prefix(' ')) {
        e.mensagem = resto.to_string();
    }
    e
}

/// Atalho para traduzir o erro de uma chamada ao rusqlite na montagem de um plano.
pub(crate) trait NoPreparo<T> {
    fn no_preparo(self) -> Resultado<T>;
}

impl<T> NoPreparo<T> for rusqlite::Result<T> {
    fn no_preparo(self) -> Resultado<T> {
        self.map_err(no_preparo)
    }
}

/// Roda um comando até o fim (um comando sem resultado), descartando as
/// linhas que ele devolver. O erro sai normalizado.
///
/// Texto com NUL é recusado antes de tudo ("the query contains a null
/// character"); só o primeiro comando do texto é preparado (o erro dele vem primeiro) e qualquer coisa além de
/// espaços e comentários depois dele é recusada com "You can only execute one statement at a time." (o
/// rusqlite prepararia o resto e devolveria o erro de sintaxe dele); parâmetro sem valor dá "Incorrect number
/// of bindings supplied. …".
pub(crate) fn rodar(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
    rodar_observado(conn, sql, &mut Observador::nenhum())
}

/// [`rodar`] avisando o [`Observador`] do comando que o SQLite preparou.
pub(crate) fn rodar_observado(conn: &Connection, sql: &str, observador: &mut Observador<'_>) -> rusqlite::Result<()> {
    recusar_nulo(sql)?;
    let (fim, resto) = crate::completo::primeiro_comando(sql);
    let mut comando = conn.prepare(sql.get(..fim).unwrap_or(sql)).map_err(normalizar)?;
    if resto {
        return Err(varios_comandos());
    }
    let mut linhas = comando.query([]).map_err(normalizar)?;
    observador.avisar(&linhas);
    while linhas.next().map_err(normalizar)?.is_some() {}
    Ok(())
}

/// Todas as linhas de uma consulta, cada uma lida por `ler`. O erro sai normalizado.
pub(crate) fn linhas<T, P: Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    ler: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    linhas_observadas(conn, sql, params, &mut Observador::nenhum(), ler)
}

/// [`linhas`] avisando o [`Observador`] do comando que o SQLite preparou.
pub(crate) fn linhas_observadas<T, P: Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    observador: &mut Observador<'_>,
    mut ler: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    recusar_nulo(sql)?;
    let mut comando = conn.prepare(sql).map_err(normalizar)?;
    let mut resultado = comando.query(params).map_err(normalizar)?;
    observador.avisar(&resultado);
    let mut saida = Vec::new();
    while let Some(linha) = resultado.next().map_err(normalizar)? {
        saida.push(ler(linha)?);
    }
    Ok(saida)
}

/// A primeira linha de uma consulta (o `fetchone()`), ou None.
pub(crate) fn primeira<T, P: Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    ler: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Option<T>> {
    primeira_observada(conn, sql, params, &mut Observador::nenhum(), ler)
}

fn primeira_observada<T, P: Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    observador: &mut Observador<'_>,
    ler: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Option<T>> {
    recusar_nulo(sql)?;
    let mut comando = conn.prepare(sql).map_err(normalizar)?;
    let mut resultado = comando.query(params).map_err(normalizar)?;
    observador.avisar(&resultado);
    match resultado.next().map_err(normalizar)? {
        Some(linha) => Ok(Some(ler(linha)?)),
        None => Ok(None),
    }
}

/// O inteiro da primeira coluna da primeira linha (`fetchone()[0]`); 0 se não vier linha ou vier NULL.
pub(crate) fn inteiro<P: Params>(conn: &Connection, sql: &str, params: P) -> rusqlite::Result<i64> {
    inteiro_observado(conn, sql, params, &mut Observador::nenhum())
}

/// [`inteiro`] avisando o [`Observador`] do comando que o SQLite preparou.
pub(crate) fn inteiro_observado<P: Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    observador: &mut Observador<'_>,
) -> rusqlite::Result<i64> {
    Ok(primeira_observada(conn, sql, params, observador, |r| inteiro_em(r, 0))?.unwrap_or(0))
}

/// Texto de uma coluna, sem falhar com UTF-8 inválido (vira "�"); NULL → None.
pub(crate) fn texto_opcional(linha: &Row<'_>, i: usize) -> rusqlite::Result<Option<String>> {
    Ok(match linha.get_ref(i)? {
        ValueRef::Null => None,
        ValueRef::Integer(n) => Some(n.to_string()),
        ValueRef::Real(r) => Some(formatar_real(r)),
        ValueRef::Text(b) | ValueRef::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
    })
}

/// Texto de uma coluna; NULL → "".
pub(crate) fn texto(linha: &Row<'_>, i: usize) -> rusqlite::Result<String> {
    Ok(texto_opcional(linha, i)?.unwrap_or_default())
}

/// Inteiro de uma coluna; NULL ou outro tipo → 0.
pub(crate) fn inteiro_em(linha: &Row<'_>, i: usize) -> rusqlite::Result<i64> {
    Ok(match linha.get_ref(i)? {
        ValueRef::Integer(n) => n,
        _ => 0,
    })
}

/// Inteiro de uma coluna que pode ser NULL (o rowid de uma violação de chave estrangeira).
pub(crate) fn inteiro_opcional(linha: &Row<'_>, i: usize) -> rusqlite::Result<Option<i64>> {
    Ok(match linha.get_ref(i)? {
        ValueRef::Integer(n) => Some(n),
        _ => None,
    })
}

// ---------------------------------------------------------------------------------------------------
// Tradução de erros

/// Um erro "error in view v after drop column: no such column: x" do ALTER TABLE.
pub(crate) struct ErroDeObjeto<'a> {
    /// "index", "trigger", "view" ou "table".
    pub tipo: &'static str,
    pub nome: &'a str,
    pub causa: &'a str,
}

const TIPOS_OBJETO: [&str; 4] = ["index", "trigger", "view", "table"];
const QUANDO: [&str; 2] = ["drop column", "rename"];

fn comeca_com(texto: &[u8], prefixo: &str) -> bool {
    texto.len() >= prefixo.len() && texto[..prefixo.len()].eq_ignore_ascii_case(prefixo.as_bytes())
}

/// Reconhece, sem regex:
/// `error in (index|trigger|view|table) (.+?) after (drop column|rename): (.+)$`, sem caixa e com `.`
/// casando quebra de linha. As partes fixas são ASCII, então toda posição achada é fronteira de caractere.
pub(crate) fn erro_de_objeto(msg: &str) -> Option<ErroDeObjeto<'_>> {
    let b = msg.as_bytes();
    for inicio in 0..b.len() {
        if !comeca_com(&b[inicio..], "error in ") {
            continue;
        }
        let p = inicio + "error in ".len();
        for tipo in TIPOS_OBJETO {
            if !comeca_com(&b[p..], tipo) || b.get(p + tipo.len()) != Some(&b' ') {
                continue;
            }
            let nome_ini = p + tipo.len() + 1;
            // (.+?): o menor nome (ao menos um caractere) seguido do resto que casa
            for nome_fim in nome_ini + 1..b.len() {
                if !comeca_com(&b[nome_fim..], " after ") {
                    continue;
                }
                let q = nome_fim + " after ".len();
                for quando in QUANDO {
                    let depois = q + quando.len();
                    if comeca_com(&b[q..], quando) && b[depois..].starts_with(b": ") && depois + 2 < b.len() {
                        let (Some(nome), Some(causa)) = (msg.get(nome_ini..nome_fim), msg.get(depois + 2..)) else {
                            continue;
                        };
                        return Some(ErroDeObjeto { tipo, nome, causa });
                    }
                }
            }
        }
    }
    None
}

/// "O índice", "O gatilho", "A visão", "A tabela".
fn objeto_maiusculo(tipo: &str) -> &'static str {
    match tipo {
        "index" => "O índice",
        "trigger" => "O gatilho",
        "view" => "A visão",
        _ => "A tabela",
    }
}

/// Tradução de um erro do SQLite: o ALTER TABLE que quebraria um índice, gatilho,
/// visão ou tabela vira a frase que nomeia o objeto; o resto, a tradução comum com o contexto.
pub(crate) fn traduzir(e: &rusqlite::Error, contexto: &str) -> ErroBanco {
    let original = e.to_string();
    if let Some(m) = erro_de_objeto(&original) {
        let mensagem =
            format!("{contexto} {} {} deixaria de funcionar ({}).", objeto_maiusculo(m.tipo), m.nome, aparar(m.causa));
        return ErroBanco::novo(mensagem).detalhe(original);
    }
    traduzir_sqlite(e, Some(contexto))
}

// ---------------------------------------------------------------------------------------------------
// Textos

/// `f"{n:,}".replace(",", ".")`: 5000 → "5.000".
pub(crate) fn milhar(n: i64) -> String {
    let digitos = n.unsigned_abs().to_string();
    let mut saida = String::with_capacity(digitos.len() + digitos.len() / 3 + 1);
    if n < 0 {
        saida.push('-');
    }
    for (k, ch) in digitos.chars().enumerate() {
        if k > 0 && (digitos.len() - k).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(ch);
    }
    saida
}

/// "1 linha", "5.000 linhas".
pub(crate) fn plural(n: i64, um: &str, varios: &str) -> String {
    format!("{} {}", milhar(n), if n == 1 { um } else { varios })
}

/// Literal de texto SQL: `'a''b'`.
pub(crate) fn lit(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "''"))
}

/// O texto entre aspas simples, com escapes (para o detalhe técnico das violações).
pub(crate) fn repr_texto(s: &str) -> String {
    let aspa = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut saida = String::with_capacity(s.len() + 2);
    saida.push(aspa);
    for ch in s.chars() {
        match ch {
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if c == aspa => {
                saida.push('\\');
                saida.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => saida.push_str(&format!("\\x{:02x}", c as u32)),
            c => saida.push(c),
        }
    }
    saida.push(aspa);
    saida
}
