//! O Plano, a execução dele e os ensaios (comandos rodados numa transação e desfeitos).

use std::collections::{BTreeMap, HashMap};

use comum::{ErroBanco, Resultado};
use ddl::aparar;
use rusqlite::Connection;

use crate::sql::{
    NoPreparo, Observador, cru, inteiro_em, inteiro_observado, inteiro_opcional, linhas_observadas, plural, repr_texto,
    rodar, rodar_observado, texto, traduzir,
};

/// Avisos fixos da reconstrução.
pub const AVISO_RECONSTRUCAO: &str = "A tabela será reconstruída: os dados são copiados para uma tabela nova com a \
     definição alterada, e os índices, gatilhos e visões ligados a ela são recriados.";
pub const AVISO_PRAGMAS: &str = "Durante a reconstrução as chaves estrangeiras ficam desligadas (PRAGMA \
     foreign_keys = OFF) e o ALTER TABLE usa o modo antigo (PRAGMA legacy_alter_table = ON); os dois voltam como \
     estavam no fim.";

const MSG_TRANSACAO: &str = "Há uma transação aberta (na Consulta SQL). Grave com COMMIT ou descarte com ROLLBACK \
     antes de mudar a estrutura.";
const CONTEXTO_EXECUTAR: &str = "Não foi possível aplicar a mudança; nada foi alterado.";

/// Uma linha devolvida por `PRAGMA foreign_key_check`: (tabela, rowid, pai, fk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Violacao {
    pub tabela: String,
    /// NULL numa tabela WITHOUT ROWID.
    pub rowid: Option<i64>,
    pub pai: String,
    pub fk: i64,
}

impl Violacao {
    /// A tupla citada: `('modelos', 1, 'marcas', 0)`.
    fn repr(&self) -> String {
        let rowid = self.rowid.map_or_else(|| "None".to_string(), |r| r.to_string());
        format!("({}, {rowid}, {}, {})", repr_texto(&self.tabela), repr_texto(&self.pai), self.fk)
    }
}

/// O que uma mudança de estrutura vai fazer: descrição, os comandos SQL e os avisos. Nada muda no banco até
/// [`executar`]. A prévia ([`Plano::sql`]) é exatamente o que [`executar`] roda, fora os PRAGMAs de sessão
/// (`foreign_keys`, `legacy_alter_table`), que aparecem nos avisos e são desfeitos no fim.
///
/// É um retrato do banco no momento em que foi montado: para mudar de fato, monte o plano de novo logo antes
/// de [`executar`] (não guarde o da prévia; ver [`executar`]).
///
/// A comparação (`==`) olha só os campos públicos.
#[derive(Debug, Clone, Default)]
pub struct Plano {
    pub descricao: String,
    pub comandos: Vec<String>,
    /// Verdadeiro na reconstrução de 12 passos (tabela nova, cópia, troca de nomes).
    pub reconstroi: bool,
    pub avisos: Vec<String>,
    /// PRAGMAs de sessão a rodar antes do comando de índice i; ficam fora da prévia (vão nos avisos).
    pub(crate) sessao: BTreeMap<usize, Vec<String>>,
    /// Violações de chave estrangeira que o banco já tinha quando o plano foi montado, por comando
    /// `PRAGMA foreign_key_check`: [`executar`] só desfaz tudo se aparecerem outras.
    pub(crate) ja_havia: Vec<(String, Vec<Violacao>)>,
}

impl PartialEq for Plano {
    fn eq(&self, outro: &Self) -> bool {
        self.descricao == outro.descricao
            && self.comandos == outro.comandos
            && self.reconstroi == outro.reconstroi
            && self.avisos == outro.avisos
    }
}

impl Plano {
    /// Plano com descrição e comandos, sem avisos e sem reconstrução.
    pub fn novo(descricao: impl Into<String>, comandos: Vec<String>) -> Plano {
        Plano { descricao: descricao.into(), comandos, ..Plano::default() }
    }

    pub(crate) fn completo(descricao: String, comandos: Vec<String>, reconstroi: bool, avisos: Vec<String>) -> Plano {
        Plano { descricao, comandos, reconstroi, avisos, ..Plano::default() }
    }

    /// O SQL da prévia: os comandos separados por ";\n\n" e terminados em ";" ("" sem comandos).
    pub fn sql(&self) -> String {
        if self.comandos.is_empty() { String::new() } else { format!("{};", self.comandos.join(";\n\n")) }
    }

    fn violacoes_anteriores(&self, comando: &str) -> &[Violacao] {
        self.ja_havia.iter().find(|(c, _)| c == comando).map_or(&[], |(_, v)| v.as_slice())
    }
}

// ---------------------------------------------------------------------------------------------------
// Ensaios

/// Bloco cujas mudanças são sempre desfeitas, sem tocar no arquivo.
///
/// Fora de uma transação usa BEGIN … ROLLBACK. Um SAVEPOINT solto não serviria: o RELEASE final faz COMMIT
/// de uma transação de escrita vazia, e o SQLite regrava o contador de mudanças do cabeçalho (e a data de
/// modificação do arquivo muda) a cada prévia. Dentro de uma transação aberta pela pessoa (na Consulta
/// SQL), SAVEPOINT … ROLLBACK TO … RELEASE, que só desfaz o que o bloco fez.
pub(crate) fn desfeito_no_fim<T>(conn: &Connection, nome: &str, bloco: impl FnOnce() -> Resultado<T>) -> Resultado<T> {
    let propria = conn.is_autocommit();
    if propria {
        rodar(conn, "BEGIN").no_preparo()?;
    } else {
        rodar(conn, &format!("SAVEPOINT {nome}")).no_preparo()?;
    }
    let resultado = bloco();
    let desfazer = if propria {
        if conn.is_autocommit() { Ok(()) } else { rodar(conn, "ROLLBACK") }
    } else {
        rodar(conn, &format!("ROLLBACK TO {nome}")).and_then(|()| rodar(conn, &format!("RELEASE {nome}")))
    };
    if let Err(e) = desfazer {
        log::warn!("esquema: não foi possível desfazer o ensaio ({e})");
    }
    resultado
}

/// Tradutor de erro de um ensaio (o `tradutor` opcional de `_ensaiar`).
pub(crate) type Tradutor<'a> = &'a dyn Fn(&rusqlite::Error, &str) -> ErroBanco;

/// Roda os comandos numa transação e desfaz: o SQLite valida a mudança sem que nada fique gravado.
pub(crate) fn ensaiar(conn: &Connection, comandos: &[String], contexto: &str) -> Resultado<()> {
    ensaiar_com(conn, comandos, contexto, &traduzir)
}

pub(crate) fn ensaiar_com(
    conn: &Connection,
    comandos: &[String],
    contexto: &str,
    tradutor: Tradutor<'_>,
) -> Resultado<()> {
    desfeito_no_fim(conn, "_editor_ensaio", || {
        for c in comandos {
            rodar(conn, c).map_err(|e| tradutor(&e, contexto))?;
        }
        Ok(())
    })
}

// ---------------------------------------------------------------------------------------------------
// Verificação das chaves estrangeiras

pub(crate) fn e_verificacao_fk(comando: &str) -> bool {
    comando.to_uppercase().starts_with("PRAGMA FOREIGN_KEY_CHECK")
}

fn ler_violacoes(conn: &Connection, comando: &str, observador: &mut Observador<'_>) -> rusqlite::Result<Vec<Violacao>> {
    linhas_observadas(conn, comando, [], observador, |r| {
        Ok(Violacao { tabela: texto(r, 0)?, rowid: inteiro_opcional(r, 1)?, pai: texto(r, 2)?, fk: inteiro_em(r, 3)? })
    })
}

/// O que um `PRAGMA foreign_key_check` devolve agora (erro → nenhuma).
pub(crate) fn violacoes(conn: &Connection, comando: &str) -> Vec<Violacao> {
    ler_violacoes(conn, comando, &mut Observador::nenhum()).unwrap_or_default()
}

/// As violações de `agora` que não estavam em `ja_havia`. Compara por (tabela, rowid, pai): o número da chave
/// estrangeira (fk) muda quando a reconstrução reordena as colunas.
fn violacoes_novas(ja_havia: &[Violacao], agora: Vec<Violacao>) -> Vec<Violacao> {
    let chave = |v: &Violacao| (v.tabela.clone(), v.rowid, v.pai.clone());
    let mut resta: HashMap<(String, Option<i64>, String), usize> = HashMap::new();
    for v in ja_havia {
        *resta.entry(chave(v)).or_default() += 1;
    }
    let mut novas = Vec::new();
    for v in agora {
        match resta.get_mut(&chave(&v)) {
            Some(n) if *n > 0 => *n -= 1,
            _ => novas.push(v),
        }
    }
    novas
}

/// "1 linha de modelos apontando para linhas que não existem em marcas", uma frase por (tabela, pai), na
/// ordem em que aparecem.
pub(crate) fn partes_violacao(violacoes: &[Violacao]) -> Vec<String> {
    let mut por_tabela: Vec<((&str, &str), i64)> = Vec::new();
    for v in violacoes {
        let chave = (v.tabela.as_str(), v.pai.as_str());
        match por_tabela.iter_mut().find(|(c, _)| *c == chave) {
            Some((_, n)) => *n += 1,
            None => por_tabela.push((chave, 1)),
        }
    }
    por_tabela
        .into_iter()
        .map(|((t, p), n)| {
            format!("{} de {t} apontando para linhas que não existem em {p}", plural(n, "linha", "linhas"))
        })
        .collect()
}

fn frase_violacao(violacoes: &[Violacao]) -> String {
    format!("A mudança deixaria {}. Nada foi alterado.", partes_violacao(violacoes).join(" e "))
}

// ---------------------------------------------------------------------------------------------------
// Execução

enum Falha {
    Sqlite(rusqlite::Error),
    Banco(ErroBanco),
}

impl From<rusqlite::Error> for Falha {
    fn from(e: rusqlite::Error) -> Self {
        Falha::Sqlite(e)
    }
}

/// Roda o plano. Qualquer erro → ROLLBACK (o banco fica como estava) e [`ErroBanco`] em português.
///
/// Um plano com BEGIN (reconstrução, renomear só mudando maiúsculas) com uma transação da pessoa aberta é
/// recusado (409). No fim, fora de transação, `foreign_keys` e `legacy_alter_table` voltam como estavam.
///
/// O plano é um retrato do banco no momento em que foi montado (as violações de chave estrangeira que já
/// havia, o nome temporário `_novo_<tabela>`, se o rowid é copiado, as contagens dos avisos). Monte-o de novo
/// logo antes de executar, na mesma vez em que segura a conexão (como o `servico` faz); não
/// guarde o plano da prévia para executar quando a pessoa confirmar.
pub fn executar(conn: &Connection, plano: &Plano) -> Resultado<()> {
    executar_observado(conn, plano, &mut Observador::nenhum())
}

/// [`executar`] avisando o [`Observador`] de cada comando que o SQLite preparou e começou a rodar, na ordem e
/// com o texto que o próprio SQLite tem.
pub(crate) fn executar_observado(conn: &Connection, plano: &Plano, observador: &mut Observador<'_>) -> Resultado<()> {
    if plano.comandos.is_empty() {
        return Ok(());
    }
    let transacional = plano.comandos.iter().any(|c| aparar(c).to_uppercase() == "BEGIN");
    if transacional && !conn.is_autocommit() {
        return Err(ErroBanco::com_status(MSG_TRANSACAO, 409));
    }
    let chaves = inteiro_observado(conn, "PRAGMA foreign_keys", [], observador).map_err(cru)?;
    let legado = inteiro_observado(conn, "PRAGMA legacy_alter_table", [], observador).map_err(cru)?;

    let resultado = rodar_comandos(conn, plano, observador);
    if resultado.is_err()
        && transacional
        && !conn.is_autocommit()
        && let Err(e) = rodar_observado(conn, "ROLLBACK", observador)
    {
        log::warn!("esquema: ROLLBACK depois do erro falhou ({e})");
    }
    let erro = match resultado {
        Ok(()) => None,
        Err(Falha::Sqlite(e)) => Some(traduzir(&e, CONTEXTO_EXECUTAR)),
        Err(Falha::Banco(e)) => Some(e),
    };

    // sempre, com erro ou sem
    if conn.is_autocommit() {
        let legado_sql = format!("PRAGMA legacy_alter_table = {}", i64::from(legado != 0));
        let chaves_sql = format!("PRAGMA foreign_keys = {}", i64::from(chaves != 0));
        for sql in [legado_sql, chaves_sql] {
            if let Err(e) = rodar_observado(conn, &sql, observador) {
                // O erro da mudança, se houve, é o que interessa à pessoa.
                if erro.is_none() {
                    return Err(cru(e));
                }
                log::warn!("esquema: não foi possível restaurar {sql} ({e})");
            }
        }
    }
    match erro {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn rodar_comandos(conn: &Connection, plano: &Plano, observador: &mut Observador<'_>) -> Result<(), Falha> {
    for (i, comando) in plano.comandos.iter().enumerate() {
        for pragma in plano.sessao.get(&i).map_or(&[][..], |v| v.as_slice()) {
            rodar_observado(conn, pragma, observador)?;
        }
        if e_verificacao_fk(comando) {
            let agora = ler_violacoes(conn, comando, observador)?;
            let novas = violacoes_novas(plano.violacoes_anteriores(comando), agora);
            if !novas.is_empty() {
                let primeiras: Vec<String> = novas.iter().take(5).map(Violacao::repr).collect();
                let detalhe = format!("{comando}: [{}]", primeiras.join(", "));
                return Err(Falha::Banco(ErroBanco::novo(frase_violacao(&novas)).detalhe(detalhe)));
            }
        } else {
            rodar_observado(conn, comando, observador)?;
        }
    }
    Ok(())
}
