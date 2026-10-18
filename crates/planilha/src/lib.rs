//! Leitura de planilhas do Excel (`.xlsx`, `.xlsm`), só leitura, sem biblioteca de fora: o crate `zip` abre o pacote e
//! um leitor XML escrito aqui (módulo `xml`) interpreta as partes em fluxo. É o núcleo da etapa 6 do
//! DataBox ("Excel só leitura, com as planilhas em abas embaixo"): não conhece tela nenhuma.
//!
//! # API
//!
//! ```text
//! Pasta::abrir(caminho) -> Resultado<Pasta>      (Clone barato; Send + Sync; lê só as partes pequenas)
//!   .folhas() -> &[Folha]        Folha { nome, id, estado: Visivel|Oculta|MuitoOculta,
//!                                        tipo: Planilha|Grafico|Dialogo|Macro, parte, tamanho_xml }
//!   .primeira_planilha_visivel() -> Option<usize>   (a aba ativa do arquivo, se for visível, senão a primeira)
//!   .aba_ativa()  .sistema_de_datas() -> D1900|D1904
//!   .formato(estilo) -> &Formato   .exibir(&Celula) -> String   (o texto como o Excel mostra, em pt-BR)
//!   .textos_compartilhados()  .texto_compartilhado(i)
//!   .planilha(folha) -> Resultado<Planilha>        (leitura paginada)
//!   .linhas(folha)   -> Resultado<Linhas>          (iterador em fluxo de Resultado<Linha>; não guarda nada)
//!
//! Planilha  (uma aba, para a interface paginar)
//!   .indexar(|&Progresso| -> bool) -> Resultado<&Indice>   uma passada sem copiar texto; false interrompe
//!   .tamanho() -> (linhas, colunas)     .meta() -> &MetaFolha     .indice() -> Option<&Indice>
//!   .pagina(primeira_linha, quantidade) -> Resultado<Pagina>     (linhas do Excel, a partir de 1, sem buracos)
//!   .definir_cache(blocos)  .liberar()  .aberturas_do_fluxo()
//!
//! Pagina { primeira_linha, linhas: Vec<Linha> }
//! Linha { numero, oculta, altura, celulas: Vec<Celula> }    .celula(col) .valor(col) .valores(n)
//! Celula { coluna (de 0), valor: Valor, estilo, formula: Option<String> }
//! Valor { Vazio, Texto, Numero(f64), Booleano, Erro(String), Data, Hora, DataHora }
//! MetaFolha { dimensao, mescladas, colunas, congelado, filtro, altura_padrao, largura_padrao }
//!   .mescla_em(linha, coluna)  .largura_da_coluna(coluna)
//! Intervalo { linha_inicial, coluna_inicial, linha_final, coluna_final }   "A1:C3" ↔ Intervalo::de_texto / Display
//! Formato  .categoria() -> Geral|Numero|Percentual|Cientifico|Fracao|Data|Hora|DataHora|Duracao|Texto
//!          .exibir(&Valor) / .exibir_com(&Valor, &Localidade)
//! ```
//!
//! # Como a interface usa
//!
//! ```text
//! let pasta = Pasta::abrir(&caminho)?;                       // rápido: nomes das abas, estilos, textos
//! let mut aba = pasta.planilha(pasta.primeira_planilha_visivel().unwrap_or(0))?;
//! let primeira = aba.pagina(1, 200)?;                        // já dá para mostrar, sem esperar o índice
//! aba.indexar(|p| { atualizar_barra(p); !cancelado() })?;    // numa linha de execução; depois vale a rolagem
//! let (linhas, colunas) = aba.tamanho();                     // tamanho da barra de rolagem
//! let pagina = aba.pagina(linha_do_topo, 60)?;               // cada página, sem reler o arquivo todo
//! for l in &pagina.linhas { for c in &l.celulas { pasta.exibir(c) } }
//! ```
//!
//! # O que lê
//!
//! - Pacote: `[Content_Types].xml`, `_rels/.rels`, `xl/workbook.xml` (nomes, ordem, `state`, `date1904`,
//!   `activeTab`) e `xl/_rels/workbook.xml.rels`, `xl/sharedStrings.xml` (com trechos formatados `<r>`; o
//!   fonético `<rPh>` fica de fora), `xl/styles.xml` (`numFmts` e `cellXfs`) e cada `xl/worksheets/sheetN.xml`
//!   sob demanda. Nomes de parte sem diferenciar maiúsculas e com `\`, alvos relativos e absolutos, `%20`.
//! - Células: texto compartilhado (`t="s"`), texto embutido (`inlineStr`), número, booleano (`b`), erro (`e`),
//!   texto de fórmula (`str`), data ISO (`d`, dos arquivos Strict), fórmula com o último valor calculado (o texto
//!   da fórmula vem em `Celula::formula`), célula só com estilo (vazia), referências `r="C12"` (as células que
//!   faltam são vazias) e `_xHHHH_` escapado no texto. Datas e horas pelo estilo (formato de data/hora → [`Valor::Data`],
//!   [`Valor::Hora`], [`Valor::DataHora`]), no sistema 1900 (com o dia fantasma 29/02/1900) e no 1904.
//! - Da aba: dimensão, colunas (largura, oculta), painel congelado, altura/largura padrão, células mescladas,
//!   autofiltro, linhas ocultas e altura.
//!
//! # O que não lê (por enquanto)
//!
//! `.xls` (97-2003), `.xlsb` e arquivos protegidos por senha dão erro com a explicação em português. Ignorados:
//! gráficos, imagens, tabelas dinâmicas, formatação condicional, validação, comentários, hiperlinks, fórmulas
//! compartilhadas sem texto (a célula traz o valor calculado), cores e fontes. Fórmulas não são recalculadas.
//! Colunas de notação científica de engenharia (`##0.0E+0`) saem como científico comum.
//!
//! # Desempenho e memória
//!
//! O XML nunca é carregado inteiro: uma linha de execução descomprime em blocos de 256 KB (canal de 1 MB) e
//! o leitor XML consome em blocos de 64 KB, sem alocar por evento. As frases compartilhadas ficam num bloco
//! só de memória, com teto de 512 MB (frases e fins; passou, a abertura para e dá erro em português). A indexação de uma aba grande é uma passada sem copiar texto; cada página lê só ~128
//! linhas ao redor do ponto. Veja `Planilha` para o modelo de índice e cache.
//!
//! # Erros
//!
//! Todos são [`comum::ErroBanco`] com a frase em português (400, ou 404/403 de arquivo) e o detalhe técnico à
//! parte. Interromper a indexação devolve 409 com `Codigo::Cancelado`.

#![forbid(unsafe_code)]
// Índice e fatia que estouram viram pânico; aqui eles passam por .get() ou iterador (ARQUITETURA.md §3).
// Os testes ficam de fora: neles, o pânico é a falha que se quer ver.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]

mod datas;
mod erros;
mod fluxo;
mod folha;
mod formato;
mod pasta;
mod planilha;
mod valores;
mod xml;

#[cfg(test)]
mod testes;

pub use datas::{SistemaDatas, serial_para_hora, serial_para_momento};
pub use folha::{Congelamento, InfoColuna, MetaFolha};
pub use formato::{Categoria, Formato, Localidade, formato_interno};
pub use pasta::{Estado, Folha, Linhas, Pasta, TipoFolha};
pub use planilha::{BLOCOS_NO_CACHE, Indice, LINHAS_POR_BLOCO, LINHAS_POR_MARCO, Planilha, Progresso};
pub use valores::{
    Celula, Data, Hora, Intervalo, Linha, Momento, Pagina, ULTIMA_COLUNA, Valor, indice_da_coluna, nome_da_coluna,
    referencia, referencia_a1,
};
