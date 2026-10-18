//! Arquivos de texto do DataBox (etapa 5): ler e gravar TXT, MD, LOG e JSON como o editor de texto precisa.
//! Puro: sem tela e sem biblioteca de fora, só `std`, o `comum` (os erros em português) e o `tabular` (a
//! codificação e a tabela do Windows-1252 que o CSV já usa).
//!
//! # API
//!
//! ```text
//! texto::limites                                  os limites de tamanho
//!   LIMITE_EDICAO (512 KiB, 18/10/2026) · LIMITE_LEITURA (32 MiB) · LIMITE_ESTRUTURA (8 MiB)
//!   max_undos(tamanho) -> usize                   clamp(32 MiB / tamanho, 10, 100)
//!   tamanho_em_texto(bytes) -> String             "812 bytes", "6,2 MB"
//!
//! texto::arquivo                                  carregar e gravar um arquivo de texto
//!   carregar(&caminho) -> Resultado<Carregado>    codificação, fim de linha dominante, texto só com \n
//!   decodificar(&bytes, modificado)               o mesmo, para quem já tem os bytes
//!   gravar(&caminho, &texto, &Carregado) -> Resultado<ResumoGravado>   atômico; devolve o formato do arquivo
//!   codificar(&texto, codificacao, fim_de_linha) -> Resultado<Vec<u8>>
//!   gravar_atomico(&caminho, &bytes)              temporário + sync_all + rename (insiste 200 ms); pelo link simbólico,
//!                                                 grava o arquivo para onde ele aponta
//!   mudou_no_disco(&caminho, &Carregado) -> bool  o tamanho ou a data não são os de quando foi lido
//!   Carregado { texto, codificacao, fim_de_linha, fim_misto, termina_em_quebra, bytes, modificado }
//!     .registrar_gravacao(&resumo, &texto) · .rotulo_da_codificacao() · .rotulo_do_fim_de_linha()
//!   Codificacao · FimDeLinha                      as do `tabular::csv`, reexportadas
//!
//! texto::posicao                                  linha, coluna, byte e índice de caractere
//!   linha_e_coluna(&texto, byte) -> (linha, coluna)           de 1, em caracteres
//!   byte_da_linha_e_coluna(..) · caractere_do_byte(..) · byte_do_caractere(..) · contar_linhas(..)
//!
//! texto::json                                     o analisador de JSON próprio (a lista completa está em `json`)
//!   analisar(&texto) -> Analise { arvore, erro }  estrito (RFC 8259), sem recursão, com a posição de cada valor
//!   validar(&texto) -> Option<ErroJson>           só o veredito, sem montar a estrutura
//!   formatar(&texto, Recuo) -> Result<String, ErroJson>        só espaços e quebras mudam
//!   Arvore: caminho_em(byte) · filhos(id) · pagina_de_filhos(id, a_partir) · nome(id, &texto) · contagem(id)
//! ```
//!
//! # Como o texto é lido e gravado
//!
//! - **Codificação:** BOM de UTF-8 → UTF-8 com BOM; UTF-8 válido do começo ao fim (inclusive só ASCII) → UTF-8;
//!   qualquer byte inválido → Windows-1252 (tabela própria do `tabular`). UTF-16 (com BOM, ou sem ele quando os
//!   zeros se repetem de dois em dois) recebe a frase "O arquivo está em UTF-16…"; outros bytes nulos, "O arquivo
//!   não parece de texto…". Acima de 32 MiB a leitura é recusada com a frase do tamanho.
//! - **Fim de linha:** o texto carregado só tem `\n`; `\r\n` e `\r` viram `\n`. O fim de linha dominante (empate:
//!   CRLF, depois LF; sem nenhuma quebra: CRLF) é guardado e **volta na gravação**; arquivo que misturava fins de
//!   linha é regravado todo com o dominante (`fim_misto`, a ficha mostra "CRLF (misto)").
//! - **Quebra no fim:** o texto carregado traz a quebra final se o arquivo tinha, e a gravação grava o texto como
//!   ele está, então não acrescenta nem tira a quebra sozinha. O BOM volta como estava.
//! - **Gravação atômica:** temporário `~nome.xxxxxxxx.tmp` na pasta do destino, `sync_all`, `rename` por cima
//!   (que no Windows substitui o destino), tentando de novo por uns 200 ms se outro programa estiver lendo o
//!   arquivo. Em qualquer erro o temporário é apagado e o arquivo fica como estava. Caractere que o Windows-1252
//!   não tem recusa a gravação, com a frase e a linha, antes de tocar no disco.
//!
//! # Testes
//!
//! `cargo test -p texto`. Os de arquivo usam uma pasta temporária por teste.

#![forbid(unsafe_code)]

pub mod arquivo;
pub mod json;
pub mod limites;
pub mod posicao;

#[cfg(test)]
mod testes_apoio;

pub use arquivo::{Carregado, ResumoGravado, carregar, gravar, gravar_atomico, mudou_no_disco};
pub use tabular::csv::{Codificacao, FimDeLinha};
