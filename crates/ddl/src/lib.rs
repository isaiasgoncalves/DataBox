//! Tokenizador de SQL (dialeto do SQLite) e analisador de CREATE TABLE.
//!
//! O SQLite guarda em sqlite_schema o texto exato do CREATE TABLE, mas não oferece um jeito de ler as
//! restrições de cada coluna (CHECK, COLLATE, expressão gerada, ações da chave estrangeira...). Este crate
//! lê esse texto e o devolve em partes ([`DefTabela`], [`DefColuna`]) e faz o caminho de volta
//! ([`montar_coluna`], [`montar_create_table`]), que é o que a reconstrução de tabela do `esquema` usa.
//!
//! Regra do caminho de volta: o que não foi entendido (nome de CONSTRAINT, cláusula ON CONFLICT, PRIMARY KEY
//! DESC...) vai para `extra` literalmente e é devolvido no fim da coluna, para nada se perder. Tipo, extra,
//! complemento e sufixo voltam numa linha só ([`normalizar_espacos`]): o comentário de linha `-- x` vira
//! `/* x */`, senão engoliria o resto da restrição.
//!
//! Todo erro é [`comum::ErroBanco`] com status 400 e frase inteira. Nada entra em pânico
//! com texto malformado. Todos os tipos são dados simples (`Send + Sync`).
//!
//! # Posições
//!
//! `Token::inicio` e `Token::fim` são **bytes** do texto UTF-8 e caem
//! sempre em fronteira de caractere: `&sql[t.inicio..t.fim] == t.texto`, e `&sql[toks[0].inicio..toks[k].fim]`
//! é o trecho original de vários tokens. Para fatiar sem risco de pânico, use [`fatia`]. A posição das
//! mensagens de erro ("começa na posição 8") é em caracteres.
//!
//! # API pública
//!
//! Tokens:
//! - `enum TipoToken { Palavra, Ident, Texto, Numero, Op, Espaco, Comentario, Blob }` + `como_str()` →
//!   "palavra" | "ident" | "string" | "numero" | "op" | "espaco" | "comentario" | "blob"; [`TIPOS_TOKEN`].
//! - `struct Token { tipo: TipoToken, texto: String, inicio: usize, fim: usize }`, com `alto() -> String`
//!   (maiúsculas), `e(&["CREATE", "TABLE"]) -> bool` (palavra sem aspas igual a uma delas)
//!   e `e_op("(") -> bool`.
//! - `tokenizar(sql: &str) -> Resultado<Vec<Token>>`; `significativos(sql: &str) -> Resultado<Vec<Token>>`
//!   (sem espaços e comentários); `significativos_de(tokens: Vec<Token>) -> Vec<Token>`.
//! - `desfazer_aspas(texto: &str) -> String` (`"a""b"` → `a"b`; para um token, `desfazer_aspas(&t.texto)`).
//! - `fecha(toks: &[Token], i: usize) -> Resultado<usize>`: índice do ")" que fecha o "(" de `toks[i]`.
//! - Texto: `e_espaco(char)`, `aparar(&str) -> &str`, `colapsar_espacos(&str) -> String` (cada sequência de
//!   espaços vira um), `preenchido(&str) -> bool`, `normalizar_espacos(&str) -> String`,
//!   `fatia(sql, inicio, fim) -> &str`. O espaço é o White_Space do Unicode **mais 0x1C–0x1F**: `str::trim()`
//!   não serve no lugar de `aparar`. Quem depende do ddl (esquema, servico) usa estes em vez de copiar a regra; banco e prefs, que não dependem do ddl, têm cópias próprias iguais
//!   (`pub(crate)`). Juntar as três em `comum` fica com o dono do comum (o ddl passaria a só reexportar).
//!
//! Analisador:
//! - `analisar_create_table(sql: &str) -> Resultado<DefTabela>`; outro comando, tabela virtual ou
//!   `CREATE TABLE … AS SELECT` → erro.
//! - `struct DefTabela { nome, colunas: Vec<DefColuna>, restricoes: Vec<String>, sem_rowid, estrita, sufixo: String }`.
//! - `struct DefColuna { nome, tipo, texto, pk, pk_ordem: Option<String>, autoincremento, nao_nulo, unica,
//!   padrao, verificacao, colacao: Option<String>, referencia: Option<Referencia>, referencia_texto:
//!   Option<String>, gerada: Option<Gerada>, extra: Option<String> }` + `como_dict() -> ColunaDef`.
//!
//! Caminho de volta (ColunaDef):
//! - `struct ColunaDef { nome, tipo: String, nao_nulo, unica, pk, autoincremento: bool, padrao, verificacao,
//!   colacao: Option<String>, referencia: Option<Referencia>, gerada: Option<Gerada>, extra: Option<String> }`.
//! - `struct Referencia { tabela: String, coluna, ao_excluir, ao_atualizar, complemento: Option<String> }`
//!   (`complemento` = MATCH / DEFERRABLE).
//! - `struct Gerada { expressao: String, armazenada: bool }`. As três com Debug, Clone, PartialEq, Eq, Default.
//! - `montar_coluna(&ColunaDef) -> Resultado<String>`; `montar_create_table(nome: &str, colunas_sql: &[String],
//!   restricoes: &[String], sufixo: &str) -> Resultado<String>` (sufixo "" = nenhum);
//!   `validar_trecho(texto: &str, rotulo: &str) -> Resultado<String>`; [`ACOES_FK`].
//!
//! Nomes citados (conjuntos ordenados, sem aspas e em minúsculas):
//! - `citados(sql) -> Resultado<BTreeSet<String>>`, `colunas_da_expressao(expr) -> Resultado<BTreeSet<String>>`,
//!   `colunas_da_restricao(texto, tabela) -> Resultado<BTreeSet<String>>`,
//!   `colunas_gravadas_pelo_gatilho(sql) -> Resultado<BTreeMap<String, BTreeSet<String>>>`.
//!
//! # Detalhes do texto
//!
//! - Dígito é só `0`–`9` (como no SQLite): dígitos Unicode fazem parte de uma palavra, como qualquer
//!   caractere fora do ASCII.
//! - `colunas_gravadas_pelo_gatilho` com `UPDATE OR x` no fim do texto para de ler.
//! - Valor padrão terminando num comentário de linha: `montar_coluna` mantém o "\n" que `validar_trecho` põe
//!   depois dele, como no CHECK, na coluna gerada, no complemento da referência e no extra:
//!   `DEFAULT 'x' -- obs\n COLLATE NOCASE REFERENCES "p" …` e `DEFAULT (1 + 2 -- soma\n)`. Sem o "\n", o
//!   comentário engoliria em silêncio o resto da coluna (COLLATE, REFERENCES, GENERATED, extra) ou o ")" da
//!   expressão. Só muda o texto quando o padrão termina em `-- …`; a coluna pode então terminar em "\n".
//!
//! # Testes
//!
//! `src/testes.rs` confere o texto (tokens, análise, montagem e ida e volta) sem executar nada no SQLite;
//! `tests/ida_e_volta_no_sqlite.rs` executa a ida e volta no SQLite e compara os PRAGMAs do original e da
//! volta.

#![forbid(unsafe_code)]

mod analise;
mod montagem;
mod nomes;
mod tokens;

pub use analise::{DefColuna, DefTabela, analisar_create_table};
pub use montagem::{ACOES_FK, ColunaDef, Gerada, Referencia, montar_coluna, montar_create_table, validar_trecho};
pub use nomes::{citados, colunas_da_expressao, colunas_da_restricao, colunas_gravadas_pelo_gatilho};
pub use tokens::{
    TIPOS_TOKEN, TipoToken, Token, aparar, colapsar_espacos, desfazer_aspas, e_espaco, fatia, fecha,
    normalizar_espacos, preenchido, significativos, significativos_de, tokenizar,
};

#[cfg(test)]
mod testes;
