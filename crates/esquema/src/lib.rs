//! Planos de mudança de estrutura: criar, renomear e excluir tabelas, colunas, índices, visões e gatilhos.
//!
//! Cada `plano_*` só lê o banco e devolve um [`Plano`] (descrição, SQL, avisos); nada muda até
//! [`executar`]. Assim a interface mostra antes "o SQL que será executado", e a prévia é exatamente o que
//! [`executar`] roda — fora os PRAGMAs de sessão (`foreign_keys`, `legacy_alter_table`), que aparecem nos
//! avisos e são desfeitos no fim.
//!
//! O que o ALTER TABLE do SQLite faz sozinho (renomear tabela e coluna, adicionar e excluir coluna) vai
//! direto. O resto usa a reconstrução de 12 passos da documentação oficial
//! (<https://sqlite.org/lang_altertable.html#otheralter>): tabela nova `_novo_<nome>`, cópia das linhas
//! (com o rowid, quando ele é interno), sqlite_sequence preservada, troca de nomes e recriação de índices,
//! gatilhos e visões, tudo numa transação, com `PRAGMA foreign_keys = OFF` antes do BEGIN,
//! `legacy_alter_table = ON` quando preciso e `PRAGMA foreign_key_check` de cada tabela envolvida no fim
//! (que só desfaz tudo por violações **novas**: as que o banco já tinha ficam registradas no plano e num
//! aviso).
//!
//! Os planos validam a mudança com um **ensaio** (os comandos rodados e desfeitos) que deixa o arquivo byte a
//! byte igual: BEGIN … ROLLBACK sem transação aberta; SAVEPOINT … ROLLBACK TO … RELEASE dentro de uma
//! transação aberta pela pessoa (na Consulta SQL), que continua aberta.
//!
//! # API pública
//!
//! Todas as funções recebem uma conexão em autocommit (`&rusqlite::Connection`, a que `banco::Banco`
//! entrega por `com_conexao`) e devolvem `comum::Resultado<…>`; os erros são [`comum::ErroBanco`] com frase
//! inteira e `status` (400 regra, 403 interna/auxiliar, 404 não existe, 409 nome já usado ou transação aberta),
//! sem `codigo`.
//!
//! ```text
//! pub struct Plano { pub descricao: String, pub comandos: Vec<String>, pub reconstroi: bool,
//!                    pub avisos: Vec<String>, /* privados: PRAGMAs de sessão, violações que já havia */ }
//! impl Plano { pub fn novo(descricao, comandos: Vec<String>) -> Plano;  pub fn sql(&self) -> String }
//!     // sql(): comandos unidos por ";\n\n" e terminados em ";" ("" sem comandos). `==` compara só os campos
//!     // públicos. Plano é Send + Sync (dados simples).
//! pub fn executar(conn, &Plano) -> Resultado<()>
//!     // erro → ROLLBACK e "Não foi possível aplicar a mudança; nada foi alterado. …"; plano com BEGIN e
//!     // transação aberta → 409 "Há uma transação aberta (na Consulta SQL). …"; violação nova de chave
//!     // estrangeira → "A mudança deixaria 1 linha de modelos apontando para … Nada foi alterado."
//! pub struct Definicoes { pub colunas: Vec<ColunaDef>, pub restricoes: Vec<String>, pub sem_rowid: bool,
//!                         pub estrita: bool }
//! pub fn definicoes_tabela(conn, tabela: &str) -> Resultado<Definicoes>
//!
//! pub use ddl::{ColunaDef, Gerada, Referencia};   // os argumentos das funções abaixo, sem depender do ddl
//! pub fn plano_criar_tabela<S: AsRef<str>>(conn, nome: &str, colunas: &[ColunaDef], restricoes: &[S],
//!                                          sem_rowid: bool)   // sem restrições: &[] as &[&str]
//! pub fn plano_renomear_tabela(conn, tabela: &str, novo_nome: &str)
//! pub fn plano_excluir_tabela(conn, tabela: &str)
//! pub fn plano_adicionar_coluna(conn, tabela: &str, coluna: &ColunaDef)
//! pub fn plano_alterar_coluna(conn, tabela: &str, coluna: &str, nova: &ColunaDef)   // inclui renomear
//! pub fn plano_excluir_coluna(conn, tabela: &str, coluna: &str)
//! pub fn plano_reordenar_colunas<S: AsRef<str>>(conn, tabela: &str, ordem: &[S])
//! pub struct ColunaIndice { pub nome: String, pub desc: bool }   // ColunaIndice::nova("x"), From<&str>
//! pub fn plano_criar_indice(conn, nome: &str, tabela: &str, colunas: &[ColunaIndice], unico: bool,
//!                           onde: Option<&str>)
//! pub fn plano_excluir_indice(conn, nome: &str)
//! pub fn plano_criar_visao(conn, nome: &str, select_sql: &str)
//! pub fn plano_excluir_visao(conn, nome: &str)
//! pub fn plano_criar_gatilho(conn, sql: &str)          // o CREATE TRIGGER … BEGIN … END inteiro
//! pub fn plano_excluir_gatilho(conn, nome: &str)
//!     // todas → Resultado<Plano>
//! pub const AVISO_RECONSTRUCAO: &str;  pub const AVISO_PRAGMAS: &str;
//! ```
//!
//! Os campos do [`Plano`] são públicos e a prévia é `plano.sql()`. A [`ColunaDef`] que as funções recebem pode
//! vir com espaços nas pontas e textos vazios: elas a normalizam antes. Nomes de tabela, coluna, índice, visão e gatilho valem sem
//! diferenciar maiúsculas (o nome exato tem preferência), como no SQLite.
//!
//! **O plano é um retrato do banco no momento em que foi montado**: guarda as violações de chave estrangeira
//! que o banco já tinha (só as novas desfazem a mudança), o nome temporário `_novo_<tabela>` que estava livre,
//! a decisão de copiar o rowid e as contagens dos avisos. Por isso **não guarde o plano da prévia** para
//! executar quando a pessoa confirmar: monte-o de novo e execute em seguida, com a mesma conexão e sem soltar
//! a trava do banco no meio (é o que o `servico` faz: monta e executa na mesma chamada). Senão, uma linha que ficou órfã nesse meio-tempo (pela Consulta SQL, por exemplo) conta
//! como violação nova e a mudança é recusada, ou o nome temporário pode já estar em uso.
//!
//! **Para o `servico`**: nenhum erro do SQLite escapa cru. O erro da montagem de um plano (uma contagem, um
//! PRAGMA, o banco travado) já chega com "Não foi possível preparar a mudança de estrutura." na frente; não
//! anteponha de novo. `definicoes_tabela` devolve o erro sem ela. Os erros de [`executar`] já vêm completos.
//!
//! O SQL que a pessoa digita (SELECT da visão, condição do índice, CHECK, DEFAULT, complementos, nomes) é
//! tratado com cuidado: texto com o caractere NUL é recusado antes de chegar ao
//! SQLite ("… O SQLite recusou a operação: the query contains a null character."; o SQLite pararia de ler no
//! NUL e rodaria um comando cortado, diferente da prévia) e parâmetro (`?`, `:x`) que o SQLite aceita mas
//! ninguém liga dá "… Incorrect number of bindings supplied. The current statement uses 1, and there are 0
//! supplied..".
//!
//! # Detalhes
//!
//! - Tipos no lugar das checagens de formato: `plano_criar_indice` recebe [`ColunaIndice`],
//!   `plano_reordenar_colunas` uma lista de nomes e as colunas são [`ColunaDef`]. Nome ausente é `""` (a frase
//!   da coluna de índice inexistente diz "A coluna  não existe…").
//! - Em [`executar`], o erro dos PRAGMAs de sessão chega traduzido sem contexto.
//! - O `plano_criar_gatilho` e a regra "um comando por vez" usam uma cópia da máquina de estados do
//!   `sqlite3_complete` sobre o tokenizador do ddl (sem FFI): só o primeiro comando do texto é preparado e o
//!   que vier depois além de espaços e comentários dá "You can only execute one statement at a time.". Difere
//!   do SQLite só em casos patológicos (número colado em palavra-chave, `:END`, `\v` entre comandos).
//! - Se restaurar `foreign_keys`/`legacy_alter_table` no fim de [`executar`] falhar depois de um erro da
//!   mudança, volta o erro da mudança.
//! - A mensagem do SQLite vem sem o "`… in <sql> at offset N`" que o rusqlite junta.
//! - NUL no CREATE TRIGGER do `plano_criar_gatilho` dá "Não foi possível criar o gatilho. O SQLite recusou a
//!   operação: the query contains a null character." (400), como os outros textos com NUL.
//!
//! # Testes
//!
//! `src/testes/` cobre cada operação e confere 227 casos fixos: `casos_esperados.rs` guarda o plano ou o erro
//! esperado de cada caso, letra por letra, e `referencia.rs` roda os mesmos casos aqui (com o `setup` de cada
//! um). Os casos que divergirem aparecem juntos na falha de `planos_e_erros_iguais_aos_esperados`.

#![forbid(unsafe_code)]

mod colunas;
mod completo;
mod leitura;
mod objetos;
mod plano;
mod reconstrucao;
mod sql;
mod tabelas;

pub use colunas::{plano_adicionar_coluna, plano_alterar_coluna, plano_excluir_coluna, plano_reordenar_colunas};
/// As definições de coluna que as funções recebem (moram no crate ddl).
pub use ddl::{ColunaDef, Gerada, Referencia};
pub use leitura::{Definicoes, definicoes_tabela};
pub use objetos::{
    ColunaIndice, plano_criar_gatilho, plano_criar_indice, plano_criar_visao, plano_excluir_gatilho,
    plano_excluir_indice, plano_excluir_visao,
};
pub use plano::{AVISO_PRAGMAS, AVISO_RECONSTRUCAO, Plano, executar};
pub use tabelas::{plano_criar_tabela, plano_excluir_tabela, plano_renomear_tabela};

#[cfg(test)]
mod testes;
