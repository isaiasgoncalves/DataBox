//! CSV: detectar o formato, ler por páginas, editar e gravar de volta como estava.
//!
//! ```text
//! Tabela (o que o editor usa)
//!   ├─ Leitor    o arquivo no disco: formato detectado, índice dos registros, páginas por posição
//!   └─ Edicoes   o que a pessoa mudou, à parte do arquivo: células, linhas, colunas, desfazer
//! gravacao       lê o arquivo, aplica as Edicoes e grava num temporário; troca no fim
//! ```
//!
//! - [`codificacao`]: UTF-8, UTF-8 com BOM e Windows-1252 (tabela própria).
//! - [`detectar_formato`] e suas partes: separador, fim de linha, aspas, decimal, cabeçalho.
//! - [`Leitor`]: uma passada monta o índice; depois cada página custa um `seek` e a leitura de poucas
//!   dezenas de registros. `abrir_com_progresso` aceita acompanhar e cancelar.
//! - [`Tabela`]: o `Leitor` com as [`Edicoes`] por cima e as operações do editor.
//! - [`percorrer_coluna`]: os valores de uma coluna, com as alterações, um por vez (as contas da coluna).
//! - [`escrever_registro`]: o registro como bytes, com aspas e codificação do [`Formato`].

mod analise;
mod arquivos;
pub mod codificacao;
mod deteccao;
mod edicoes;
mod escrita;
mod fluxo;
mod formato;
mod gravacao;
mod indice;
mod leitor;
mod tabela;

#[cfg(test)]
mod testes;

pub(crate) use analise::{Campo, decodificar_campo, proximo_registro};

pub use codificacao::{BOM_UTF8, Codificacao, detectar_codificacao};
pub use deteccao::{
    REGISTROS_DA_AMOSTRA, detectar_aspas, detectar_cabecalho, detectar_decimal, detectar_fim_de_linha,
    detectar_formato, detectar_separador,
};
pub use edicoes::{ChaveLinha, ColunaVirtual, Edicoes, IdColuna, Trecho};
pub use escrita::{escrever_registro, registro_em_bytes};
pub use fluxo::percorrer_coluna;
pub use formato::{EstiloAspas, FimDeLinha, Formato, OpcoesAbertura, SEPARADORES, decimal_valido, separador_valido};
pub use gravacao::{MSG_NAO_E_UTF8, ResumoSalvar};
pub use indice::{Indice, PASSO_INDICE};
pub use leitor::{Leitor, TAMANHO_DA_AMOSTRA, VistaLinha};
pub use tabela::Tabela;
