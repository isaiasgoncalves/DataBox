//! O núcleo tabular do DataBox (etapa 5): ler e gravar CSV como o editor precisa, e o motor do "Trazer colunas".
//! Sem tela e sem biblioteca de fora: só `std` e o `comum` (os erros em português).
//!
//! # API
//!
//! ```text
//! tabular::csv                                    CSV
//!   Tabela                  o que o editor usa: o arquivo por páginas + as alterações por cima
//!     abrir(&caminho, &OpcoesAbertura) -> Resultado<Tabela>        abrir_com_progresso(.., &mut ao_progresso)
//!     linhas() -> u64 · colunas() -> usize · nomes() · colunas_virtuais() · formato() · caminho() · leitor()
//!     pagina(inicio, quantidade) -> Vec<Vec<String>>      uma célula por coluna, com as alterações
//!     celula(l, c) · colunas_de(&[c], ao_progresso)       as colunas pedidas de todas as linhas (chave, origem)
//!     editar_celula(l, c, &str) -> bool                   false: o texto já era esse, nada mudou
//!     inserir_linha(pos) · acrescentar_linhas(n) · excluir_linhas(&[l])
//!     inserir_coluna(pos, nome) · excluir_colunas(&[c]) · renomear_coluna(c, nome)
//!     preencher_coluna(c, &[Option<String>]) · inserir_coluna_com_valores(pos, nome, &[String])
//!     em_um_passo(|t| ..)     tudo o que a closure fizer vale um passo no desfazer; se falhar, nada fica
//!     desfazer() · refazer() · pode_desfazer() · pode_refazer() · alterada()
//!     alteracoes() -> usize                               os passos do desfazer desde a abertura ou o Salvar
//!     passos_a_refazer() -> usize                         os passos do refazer
//!     marcas_da_pagina(inicio, quantidade) -> Vec<Vec<bool>>   quais células da página têm alteração (sem E/S)
//!     definir_cabecalho(bool) · reabrir(&OpcoesAbertura, ..)     só sem alterações pendentes (409)
//!     salvar(ao_progresso) -> ResumoSalvar · salvar_como(&destino, &Formato, ao_progresso)
//!                             Err: o destino ficou como estava; trocado o arquivo, nada mais devolve erro.
//!                             Recusa (MSG_NAO_E_UTF8, 409) o arquivo que não é UTF-8 lido como UTF-8.
//!   Leitor                  o arquivo no disco, só leitura: formato detectado + índice dos registros
//!     abrir · abrir_com_progresso · registros(i, n) · linhas_de(i, n) · linha(i)
//!     varrer(ao_progresso, |i, &VistaLinha| Ok(continuar)) · colunas_de(&[c], ao_progresso)
//!     reabrir_para_leitura() (cursor próprio, mesmo índice: para varrer em outra thread) · mudou_no_disco()
//!     utf8_valido()           o arquivo todo é UTF-8 válido (visto na passada da abertura)
//!   Edicoes                 o modelo das alterações (células, linhas, colunas, desfazer), sem E/S
//!   percorrer_coluna(&mut leitor, &edicoes, c, ao_progresso, |valor| Ok(continuar))
//!                           uma coluna com as alterações, um valor por vez, sem juntá-la (as contas da coluna)
//!   Formato { codificacao, separador, fim_de_linha, aspas, cabecalho, decimal, termina_em_quebra }
//!   OpcoesAbertura          o que ficar None é detectado
//!   Codificacao { Utf8, Utf8ComBom, Windows1252 } · FimDeLinha { Crlf, Lf, Cr } · EstiloAspas { Minimas, NaoNumericos, Todas }
//!   detectar_formato / _separador / _fim_de_linha / _aspas / _decimal / _cabecalho / detectar_codificacao
//!   escrever_registro(&mut Vec<u8>, &[campos], &Formato) · registro_em_bytes(..)
//!
//! tabular::colunas                                copiar, colar e trazer colunas
//!   normalizar(valor, &OpcoesCasamento) -> Cow<str>
//!   indice_da_origem(&chaves, &opcoes) -> HashMap<String, Vec<usize>>
//!   casar(&destino, &origem, &opcoes) -> Casamento { pares, sem_par, repetidas }
//!     .total() · .com_par() · .percentual() · .linhas_da_origem(i) · .trazer(&coluna, ChavesRepetidas)
//!   sugerir_chaves(nomes_d, amostra_d, nomes_o, amostra_o, &opcoes) -> Option<(coluna_d, coluna_o)>
//!   ler_tsv(&str) -> Vec<Vec<String>> · escrever_tsv(linhas) -> String · escrever_coluna(nome, &valores)
//!   colunas_coladas(&linhas, OrigemDoTexto, decimal) · texto_tem_cabecalho(..)
//!   comparar_quantidades · posicionar(&valores, linhas, QuandoSobram) -> Posicionamento
//!   atualizar_coluna(&atual, &trazidos, ModoAtualizacao) -> Atualizacao
//!   nome_generico · nome_da_coluna_trazida · rotulo_da_origem
//!
//! tabular::numero                                 número e tipo de um texto de célula
//!   analisar_numero(&str, decimal) · canonico(&str) · e_data(&str) · classificar(&str, decimal) · tipo_da_coluna(..)
//! ```
//!
//! # Como o CSV é lido
//!
//! - **Codificação:** BOM → UTF-8 com BOM. Sem BOM, o arquivo **inteiro** é conferido durante a passada do índice:
//!   UTF-8 válido (inclusive só ASCII) → UTF-8; qualquer byte inválido, em qualquer lugar → Windows-1252
//!   (tabela própria em [`csv::codificacao`]; os cinco bytes sem uso do Windows-1252 voltam iguais). UTF-16 e
//!   arquivo com bytes nulos são recusados com a frase pronta. Pedir uma codificação em `OpcoesAbertura` vale
//!   mais que a detectada.
//! - **Separador:** de `; , \t |`, o que dá mais registros com o mesmo número de colunas (duas ou mais) nos
//!   primeiros 256 KB / 100 registros, lidos com as aspas; empate: o de mais colunas, depois a ordem acima. Sem
//!   candidato (uma coluna só), `;`. **Fim de linha:** o mais usado na amostra (sem nenhum: CRLF).
//!   **Aspas:** todos os campos com texto entre aspas → `Todas`; texto entre aspas e números soltos →
//!   `NaoNumericos`; senão `Minimas`. **Decimal:** `,` ou `.` pelos campos que só fazem sentido como número com
//!   ele; sem evidência, `,` (ou `.` se o separador é a vírgula).
//! - **Cabeçalho:** heurística (`detectar_cabecalho`): nome em texto sobre valores que são número, data ou
//!   booleano vota a favor; primeira linha do mesmo tipo dos valores de baixo, nome que se repete nos valores ou
//!   nomes repetidos votam contra; sem votos, é cabeçalho se tudo na primeira linha é texto não vazio. Número com
//!   zero à esquerda (`004718`) é texto.
//! - **Registros:** as regras do CSV comum, sem modo estrito (ver `csv::analise`): quebra `\r\n`, `\n` ou
//!   `\r` fora de aspas; `""` dentro de campo citado; aspa no meio de campo comum é aspa comum; linha em branco é
//!   um registro de um campo vazio; a quebra no fim do arquivo não cria registro; registros com mais ou menos
//!   campos que o primeiro são aceitos (a tabela tem as colunas do registro mais largo; `Indice::irregulares`
//!   conta os diferentes).
//! - **Índice:** uma passada pelo arquivo (uns 0,35 s para 1 milhão de linhas por 10 colunas, 121 MB) guarda a
//!   posição de um registro a cada 64. Ler uma página é um `seek` até o ponto mais próximo, pular no máximo 63
//!   registros e ler os pedidos (os bytes vêm de uma leitura só, até o próximo ponto). Se o arquivo mudou por
//!   fora (tamanho diferente), a leitura recusa com 409 em vez de ler lixo.
//!
//! # Como o CSV é gravado
//!
//! - As alterações ficam em [`csv::Edicoes`] até o Salvar. Salvar lê o arquivo de origem em fluxo e escreve num
//!   temporário ao lado do destino (`~nome.xxxxxxxx.tmp`), com `sync_all`, e troca por `std::fs::rename` (que no
//!   Windows substitui o destino), tentando de novo por uns 200 ms se outro programa estiver lendo o arquivo (o
//!   mesmo cuidado do `exportar` e das `prefs`). Em qualquer erro (inclusive o do `ao_progresso`, que cancela) o
//!   temporário é apagado e o arquivo fica como estava.
//! - **O que não mudou não é regravado:** a linha sem alteração é copiada byte a byte (aspas que sobram, quebra
//!   de linha própria, espaços), desde que o formato de saída escreva as linhas do mesmo jeito que o de entrada.
//!   Mudar célula, inserir ou excluir linha regrava só as linhas tocadas; renomear coluna regrava só o
//!   cabeçalho; inserir ou excluir coluna regrava todas.
//! - **O que mudou segue o formato do arquivo:** codificação (com ou sem BOM), separador, quebra de linha (a
//!   original de cada linha, se o arquivo mistura), estilo de aspas e a quebra (ou a falta dela) no fim do
//!   arquivo. Um registro de um campo vazio sai `""`, como o Exportar. Caractere que
//!   a codificação não tem (Windows-1252) é erro com a frase e a linha, e nada é gravado.
//! - **`salvar_como` com outro formato** converte o arquivo todo (decodifica e regrava cada linha).
//! - **A proteção de fórmula do Exportar** (`'` antes de `=`, `+`, `-`, `@`) é um dado: ao abrir um export com
//!   proteção, o apóstrofo aparece no texto da célula, e nada é acrescentado ou tirado na gravação.
//! - Depois de salvar, o índice do arquivo novo já vem pronto (as posições são as da gravação) e o desfazer
//!   recomeça: as alterações viraram o arquivo.
//!
//! # Trazer colunas
//!
//! - A chave é normalizada por [`colunas::normalizar`]: espaços nas pontas, caixa e, só com "Números pelo
//!   valor", a forma canônica do número (`007`, `7` e `7,0` viram `7`; `1.234,5` e `1234,5` também; sem passar
//!   por ponto flutuante). Chave vazia nunca casa.
//! - `casar` indexa a origem uma vez (uma tabela `HashMap` de `&str` normalizadas) e procura as chaves do destino,
//!   repartindo a busca entre as threads a partir de 100.000 linhas. Chave repetida na origem guarda todas as
//!   linhas; a escolha (a primeira, a última, ou todas juntas com `"; "`, pulando as vazias) só acontece em
//!   [`colunas::Casamento::trazer`], de modo que trocar o modo no medidor não refaz a conta.
//! - O que o recurso pede da Tabela: `colunas_de` lê só a chave e as colunas marcadas (numa passada, com
//!   progresso e cancelamento); `em_um_passo` + `inserir_coluna_com_valores` + `preencher_coluna` aplicam o
//!   resultado como uma alteração só.
//! - TSV como o Excel: campo com tabulação, quebra de linha ou aspas vai entre aspas (aspas dobradas); `\r\n`
//!   depois de cada linha, inclusive a última. A leitura usa o mesmo analisador do CSV.
//!
//! # Testes
//!
//! `cargo test -p tabular`: os de unidade ficam em cada módulo (`testes`), os de arquivo de verdade em
//! `csv/testes.rs`, e `tests/` tem a volta completa com o Exportar do DataBox e o Trazer colunas de ponta a
//! ponta. Os benchmarks estão marcados `#[ignore]`: `cargo test --release -p tabular -- --ignored --nocapture`.

#![forbid(unsafe_code)]
// Índice e fatia que estouram viram pânico; aqui eles passam por .get() ou iterador (ARQUITETURA.md §3).
// Os testes ficam de fora: neles, o pânico é a falha que se quer ver.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]

pub mod colunas;
pub mod csv;
pub mod numero;

#[cfg(test)]
mod testes_apoio;
