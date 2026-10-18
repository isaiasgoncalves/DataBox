//! Peças pequenas usadas por todos os crates do DataBox.
//!
//! - [`ErroBanco`]: erro com mensagem em português pronta para a tela e o texto técnico à parte.
//! - [`q`]: cita um identificador SQL. Nunca monte SQL com nome sem passar por aqui; valores vão sempre como
//!   parâmetro (`?`), nunca no texto.
//! - [`afinidade`]: a afinidade de tipo que o SQLite dá a um tipo declarado.
//! - [`traduzir_sqlite`] / [`traduzir_io`]: erro do SQLite ou do disco → [`ErroBanco`] com frase inteira.
//! - Os valores que circulam entre os crates : [`Valor`] (o que está
//!   no arquivo), [`Celula`] (o que a grade mostra), [`Chave`] (a identidade de uma linha) e [`Entrada`] (o
//!   que a pessoa manda gravar).
//! - [`app`]: o nome do aplicativo (`NOME`, `PASTA`) e as pastas por pessoa
//!   (`pasta_pessoal`, `pasta_de_configuracao` para as preferências, `pasta_de_estado` para o registro, e
//!   `base_ou`, a regra "variável de ambiente ou a pasta pessoal").
//! - [`arquivos`]: o que um caminho é (pasta, banco SQLite ou arquivo de dados), puro e sem tocar no disco: a
//!   tabela de extensões, o palpite pelo nome e tamanho, o cabeçalho do SQLite, a ordem natural dos nomes, a
//!   regra dos ocultos e os tipos `Entrada` e `Listagem` da árvore de pastas (o tamanho e a data só de arquivo).
//!   Quem varre a pasta é o `servico`.
//!   (O `arquivos::Entrada` é uma linha da árvore; não confundir com a [`Entrada`] de valores.)

pub mod app;
pub mod arquivos;
mod erro;
mod valores;

pub use erro::{Codigo, ErroBanco, Resultado, traduzir_io, traduzir_sqlite};
pub use valores::{Afinidade, Celula, Chave, Entrada, LIMITE_TEXTO, Valor, afinidade, formatar_real, q};
