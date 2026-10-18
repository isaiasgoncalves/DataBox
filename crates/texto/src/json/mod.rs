//! JSON próprio do editor: analisar com posições, a estrutura sob demanda, o erro com a
//! frase em português e o Formatar pelos símbolos.
//!
//! Nem o `serde_json` nem o `prefs::json` servem ao editor: o primeiro perde a ordem das chaves, dá erro em inglês e
//! não guarda posição; o segundo é tolerante (aceita `NaN`, descarta nível fundo).
//! Este é estrito (RFC 8259), sem recursão (a pilha é um `Vec`), e guarda onde cada valor começa e termina.
//!
//! ```text
//! texto::json
//!   analisar(&texto) -> Analise { arvore: Option<Arvore>, erro: Option<ErroJson> }
//!   validar(&texto) -> Option<ErroJson>            só o veredito, sem montar a estrutura (arquivo grande)
//!   ErroJson { linha, coluna, byte, motivo }        Display: "JSON inválido na linha 14, coluna 7: …"
//!   Arvore                                          a estrutura: uma arena de nós na ordem do documento
//!     raiz() · no(id) · pai(id) · filhos(id) · ordem(id) · total()
//!     caminho_em(byte) -> Vec<NoId>                 a migalha: do mais externo ao mais interno, sem recursão
//!     nome(id, &texto) · trecho(id, &texto) · valor_de_texto(id, &texto)
//!     pagina_de_filhos(id, a_partir) -> PaginaDeFilhos { nos, restantes }   no máximo 200 por vez
//!     contagem(id) -> Option<String>                "5 chaves", "1 item"
//!   No { tipo, inicio, fim, chave, filhos }  ·  Tipo { Objeto, Lista, Texto, Numero, Inteiro, Booleano, Nulo }
//!   formatar(&texto, Recuo) -> Result<String, ErroJson>        só espaços e quebras mudam
//!   detectar_recuo(&texto) -> Recuo · recuo_do_texto(&texto) -> Option<Recuo> · Recuo { Espacos(n), Tabulacao }
//!   desescapar(&interno) · mais_itens_em_texto(n) · milhares(n) · FILHOS_POR_PAGINA
//! ```
//!
//! # O que é aceito
//!
//! A gramática da RFC 8259, e nada mais: sem comentários, sem `NaN`/`Infinity`, sem aspas simples, sem vírgula
//! sobrando, sem zero à esquerda (`01`), sem `1.` nem `.5`, sem quebra de linha dentro de cadeia, sem caractere de
//! controle solto. Chave repetida vale (as duas ficam na estrutura). Um BOM no começo é pulado. Qualquer valor pode ser
//! a raiz (`3`, `"x"`, `null`).
//!
//! # O erro
//!
//! Aponta o começo do símbolo que estraga (linha e coluna de 1, a coluna em caracteres) com uma das 16 frases da tabela
//! "Textos novos" do plano e mais uma, a do caractere de controle solto (a tabulação dentro de uma cadeia, que o
//! `json.loads` também recusa e que a tabela não cobria). O documento que acaba antes da hora aponta o fim do último
//! símbolo. Com erro, não há estrutura: a tela guarda a última válida.
//!
//! # Memória e tempo
//!
//! Cada nó custa uns 64 bytes e a análise de 8 MiB leva uns décimos de segundo (os números medidos estão no
//! relatório da etapa e em `cargo test --release -p texto -- --ignored --nocapture`). Acima de
//! [`crate::limites::LIMITE_ESTRUTURA`] use [`validar`], que não guarda nó nenhum.

mod analise;
mod arvore;
mod erro;
mod formatar;

pub use analise::{Analise, analisar, validar};
pub use arvore::{
    Arvore, FILHOS_POR_PAGINA, Filhos, No, NoId, PaginaDeFilhos, Tipo, desescapar, mais_itens_em_texto, milhares,
};
pub use erro::ErroJson;
pub use formatar::{NIVEL_MAXIMO_DO_RECUO, Recuo, detectar_recuo, formatar, recuo_do_texto};

#[cfg(test)]
mod testes;
