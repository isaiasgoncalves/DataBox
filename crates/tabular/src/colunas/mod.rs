//! Copiar, colar e trazer colunas, sem tela: só a lógica.
//!
//! - [`casar`]: o motor do "Trazer colunas" (normalizar a chave, indexar a origem, casar as linhas do destino,
//!   escolher entre chaves repetidas, sugerir as colunas-chave).
//! - [`ler_tsv`] e [`escrever_tsv`]: o texto com tabulações da área de transferência, com as aspas do Excel.
//! - Os formatos do "Copiar como" do menu da célula ([`escrever_como`]): [`escrever_csv`], [`escrever_json`],
//!   [`escrever_markdown`], [`escrever_insert`], [`escrever_lista_in`] e [`escrever_pandas`], sobre as células de
//!   [`ValorCopia`]; e o [`escrever_tsv_com_cabecalho`] do "Copiar com cabeçalho".
//! - A cola: o cabeçalho do texto colado, a cola por posição com as quantidades diferentes, atualizar uma
//!   coluna que existe e os nomes das colunas novas.

mod casar;
mod colagem;
mod formatos;
mod tsv;

pub use casar::{
    AMOSTRA_DA_SUGESTAO, Casamento, ChavesRepetidas, Linhas, OpcoesCasamento, Repetida, SEPARADOR_DE_REPETIDAS, casar,
    indice_da_origem, normalizar, sugerir_chaves,
};
pub use colagem::{
    Atualizacao, ColunaColada, ModoAtualizacao, OrigemDoTexto, Posicionamento, QuandoSobram, Quantidades,
    atualizar_coluna, colunas_coladas, comparar_quantidades, nome_da_coluna_trazida, nome_generico, posicionar,
    rotulo_da_origem, texto_tem_cabecalho,
};
pub use formatos::{
    FormatoCopia, LINHAS_POR_INSERT, OpcoesCopia, ValorCopia, escrever_como, escrever_csv, escrever_insert,
    escrever_json, escrever_lista_in, escrever_markdown, escrever_pandas, escrever_tsv_com_cabecalho,
};
pub use tsv::{coluna_do_texto, escrever_coluna, escrever_tsv, largura, ler_tsv};
