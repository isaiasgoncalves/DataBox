//! Acesso ao arquivo SQLite: conexão, introspecção, páginas da grade com busca e
//! filtros, gravação de linhas, consulta livre, leitura em fluxo para a exportação, manutenção e cópia.
//! O contrato está no ARQUITETURA.md (§4.3); os erros são `ErroBanco` com `status` e `codigo` estáveis.
//!
//! # API
//!
//! ```text
//! Banco                                   Send + Sync; use por Arc. Conexão principal em autocommit atrás de uma
//!                                         trava reentrante (solta em ordem de chegada). Fecha no drop.
//!   abrir(caminho: &Path, somente_leitura: bool, criar: bool) -> Resultado<Banco>
//!       caminho resolvido (absoluto, links e nomes 8.3 resolvidos). Inexistente sem `criar` → 404; pasta → 400;
//!       não é banco → 400. Arquivo protegido contra gravação aberto para edição → fica só leitura, com o motivo.
//!       Só leitura com -journal quente → 409. foreign_keys ON; temp_store MEMORY até 1 GB; trusted_schema OFF
//!       (também nas conexões de leitura).
//!   caminho() -> &Path · nome() -> &str · id() -> &str (16 hex, desta abertura)
//!   somente_leitura() -> bool · motivo_somente_leitura() -> Option<&str>
//!   transacao_aberta() -> bool · exigir_sem_transacao(acao: &str) -> Resultado<()>   409 Codigo::TransacaoAberta
//!   e_arquivo_do_banco(caminho: &Path) -> bool          o próprio arquivo ou <arquivo>-journal/-wal/-shm
//!   fechar()                                            idempotente; usar depois → 409
//!   com_conexao(f: FnOnce(&Connection) -> Resultado<R>) -> Resultado<R>   para o crate esquema
//!   conexao_leitura() -> Resultado<Connection>          conexão NOVA mode=ro (quem abre fecha)
//!
//!   info() -> Resultado<BancoInfo>                      com a trava presa > 0,5 s, devolve o último que leu
//!   objetos() -> Resultado<ObjetosInfo>
//!   tabela_info(nome) -> Resultado<TabelaInfo>          sem definicoes/restricoes (o serviço junta do esquema)
//!   linhas(nome, &PedidoPagina) -> Resultado<Pagina>
//!   contagem(nome, busca: Option<&str>, filtros: &[Filtro], leitura: Option<&str>) -> Resultado<Contagem>
//!   contar_linhas(…mesmos…) -> Resultado<u64>
//!   linha(nome, &Chave) -> Resultado<LinhaCompleta>     texto inteiro; linha que sumiu → 404
//!   ler_blob(nome, &Chave, coluna) -> Resultado<Option<Vec<u8>>>   BLOB como está; NULL → None; resto em UTF-8
//!   inserir(nome, &[(String, Entrada)]) -> Resultado<LinhaGravada>
//!   atualizar(nome, &Chave, &[(String, Entrada)]) -> Resultado<LinhaGravada>   a chave pode mudar (PK editada)
//!   desfazer_celula(nome, &Chave, coluna, gravado: &Celula, antigo: &Celula) -> Resultado<LinhaGravada>
//!                                                       o Desfazer da grade: grava `antigo` só se a célula ainda
//!                                                       tem `gravado` (senão 409 MSG_CELULA_MUDOU)
//!   excluir(nome, &[Chave]) -> Resultado<usize>         tudo ou nada
//!   consultar(sql, limite: usize) -> Resultado<ResultadoConsulta>   comando a comando, para no primeiro erro
//!   interromper_consulta() -> bool                      corta o `consultar` em andamento ou que espera a vez;
//!                                                       false sem nenhum
//!   interromper_consulta_detalhado() -> Interrupcao     o mesmo, dizendo qual (Nenhuma, EmAndamento, NaFila)
//!   iterar_tabela(nome, &FiltroExportacao, parar: Option<Arc<Parar>>, consumir) -> Resultado<R>
//!   iterar_consulta(sql, parar: Option<Arc<Parar>>, consumir) -> Resultado<R>
//!       consumir: FnOnce(&[String] /* colunas */, &mut LinhasExportacao) -> Resultado<R>; o iterador (valores
//!       exatos, `Resultado<Vec<Valor>>`) só vive dentro dele.
//!   contar_para_exportar(nome, &FiltroExportacao, parar, prazo: Option<Duration>) -> Resultado<Option<u64>>
//!   manutencao(AcaoManutencao) -> Resultado<ResultadoManutencao>
//!   copia_de_seguranca(destino: &Path) -> Resultado<CopiaFeita>
//!
//! dividir_comandos(sql: &str) -> Vec<String>              comandos de topo (sqlite3_complete)
//! coagir(&Entrada, Afinidade) -> Resultado<Coagido>       Coagido::{Valor(Valor), Padrao, Manter}
//! Operador::{Contem, NaoContem, Igual, Diferente, Maior, MaiorIgual, Menor, MenorIgual, Comeca, Termina,
//!            Nulo, NaoNulo, Vazio, NaoVazio}  · TODOS · como_str() · de_texto(&str) -> Resultado (400)
//! Filtro { coluna: String, op: Operador, valor: Entrada }  · Filtro::texto(coluna, op, "digitado")
//! Direcao::{Asc, Desc} · como_str() · de_texto(&str) -> Resultado (400 "Direção de ordenação inválida: …")
//! PedidoPagina { pagina: u64 (1..=2^31), por_pagina: u64 (1..=1000), ordem: Option<String>, direcao,
//!                busca: Option<String>, filtros: Vec<Filtro>, contar: bool, leitura: Option<String> }  (Default)
//! FiltroExportacao { busca, filtros, ordem, direcao, chaves: Option<Vec<Chave>> }  (Default)
//! AcaoManutencao::{Integridade, Chaves, Vacuum, Otimizar, Analisar} · TODAS · como_str() · de_texto
//! Parar = dyn Fn() -> bool + Send + Sync          verdadeiro interrompe a leitura da exportação no SQLite
//! MSG_SUBSTITUIDA · MSG_INTERROMPIDA ("A consulta foi interrompida.") · MSG_CELULA_MUDOU
//! ESPERA_GRAVACAO_MS (5000) · ESPERA_LEITURA_MS (1500) · ESPERA_APOS_TRAVA_MS (100)
//! ESPERA_COPIA_MS (5000) · LIMITE_CONTAGEM_BYTES (256 MB) · LIMITE_TEMP_MEMORIA (1 GB) · LIMITE_TEXTO (300)
//! ```
//!
//! Objetos devolvidos (ausente → `Option`; caminho → `PathBuf`):
//! [`BancoInfo`] (com [`ContagemObjetos`]), [`ObjetosInfo`] ([`TabelaResumo`], [`VisaoResumo`],
//! [`SistemaResumo`], [`IndiceResumo`], [`GatilhoResumo`]), [`TabelaInfo`] ([`ColunaInfo`], [`FkColuna`],
//! [`ChaveEstrangeira`], [`IndiceInfo`], [`ColunaIndice`], [`GatilhoInfo`], [`TipoChave`], [`TipoObjeto`],
//! [`OrigemIndice`]), [`Pagina`] ([`LinhaPagina`]: `chave: Option<Chave>`, `valores: Vec<Celula>` cortadas em
//! `LIMITE_TEXTO`), [`Contagem`], [`LinhaCompleta`] (`valores: Vec<(String, Celula)>`, texto inteiro),
//! [`LinhaGravada`] (`chave`, `linha: Option<Vec<Celula>>`), [`ResultadoConsulta`] com
//! [`ResultadoComando`]`::{Linhas, Alteracao, Erro}`, [`ResultadoManutencao`], [`CopiaFeita`].
//!
//! # Regras que quem chama precisa saber
//!
//! - **Leitura substituída.** `PedidoPagina::leitura = Some("grade:<tabela>")` e
//!   `contagem(…, leitura: Some("contagem:<tabela>"))`: a leitura mais nova de um grupo abandona a mais
//!   velha, que devolve `ErroBanco` 409 com [`comum::Codigo::Substituido`] e a mensagem [`MSG_SUBSTITUIDA`]
//!   (a interface ignora).
//!   Se ela já executava, é interrompida no SQLite (o `detalhe` traz o erro dele). O interrupt nunca alcança
//!   gravação, Estrutura, Consulta SQL nem exportação. Sem grupo, nada muda.
//! - **Espera por arquivo travado** por outro programa: leituras 1,5 s (0,1 s nos 3 s seguintes a uma trava),
//!   gravações 5 s; depois, 409 "O banco está em uso por outro programa…". Uma gravação travada por uma
//!   exportação em andamento diz isso (409 "Há uma exportação em andamento…").
//! - **Gravação.** Só leitura, visão, tabela do sistema (exceto sqlite_sequence) ou virtual sem suporte → 403.
//!   [`comum::Entrada::Padrao`] deixa a coluna fora do INSERT (no UPDATE → 400); [`comum::Entrada::Manter`]
//!   fora do UPDATE; colunas geradas são ignoradas. Atualizar e excluir correm num SAVEPOINT desfeito em
//!   qualquer erro (funciona dentro de uma transação aberta na Consulta SQL); chave que casasse com mais de
//!   uma linha → 409 e nada é gravado; linha descartada sem erro (ON CONFLICT IGNORE, RAISE(IGNORE)) → 409.
//! - **Chaves.** [`comum::Chave::Rowid`] em tabela com rowid; [`comum::Chave::Pk`] (valores exatos, texto com
//!   UTF-8 inválido como [`comum::Valor::TextoBruto`]) em WITHOUT ROWID ou com os três apelidos do rowid
//!   tomados por colunas; linha com PK nula nessas tabelas fica sem chave (`None`) e uma chave com PK nula →
//!   400. Chave do tipo errado para a tabela → 400.
//! - **Exportação.** `iterar_*` e `contar_para_exportar` leem numa conexão nova, só de leitura; com transação
//!   aberta (na consulta, também tabela temporária, banco anexado ou PRAGMA) leem pela principal, com PRAGMA
//!   query_only e a trava presa até `consumir` voltar, para o arquivo sair igual ao que a tela mostra. Um erro
//!   no meio da leitura (inclusive a interrupção por `parar`) vem no iterador já traduzido, com a frase "Não
//!   foi possível ler os dados para exportar." (detalhe "interrupted" quando foi o `parar`); quem cancela
//!   sabe que cancelou e troca pelo `Codigo::Cancelado`. `iterar_consulta` recusa o que não é um único
//!   SELECT (400 "Só dá para exportar o resultado de uma consulta SELECT.", sem executar nada).
//! - **Cópia de segurança:** destino = o banco ou -journal/-wal/-shm → 400; pasta → 400; pasta inexistente →
//!   404; transação aberta → 409 `TransacaoAberta`; arquivo travado por fora → desiste depois de 5 s seguidos
//!   (409). Grava num temporário ao lado e troca no fim.
//! - `consultar`: `limite` 1..=100.000 linhas por comando (`truncado` diz se havia mais); `linhas_afetadas` só
//!   em INSERT/UPDATE/DELETE/REPLACE; `alterou_esquema` pela schema_version,
//!   `alterou_dados` pelo total_changes. Quem registra o histórico é o serviço.
//! - **Interromper a Consulta SQL** (só no DataBox): `interromper_consulta()`, de qualquer thread, nunca espera
//!   a trava do banco. Com um `consultar` de posse da conexão, liga a marca dele e chama o interrupt do SQLite
//!   (devolve true); o comando em andamento volta como `ResultadoComando::Erro` com [`MSG_INTERROMPIDA`] ("A
//!   consulta foi interrompida.", `detalhe` com o erro do SQLite quando foi cortado no meio, `None` quando a
//!   marca chegou antes de ele começar), os seguintes não rodam e os anteriores ficam como estão (gravados, se
//!   não havia transação aberta; um BEGIN aberto pode ter sido desfeito pelo SQLite, e `transacao_aberta` diz).
//!   Sem nenhum `consultar` de posse da conexão, mas com um esperando a vez (a conexão está com uma gravação, uma
//!   exportação, um VACUUM…), só a marca do mais antigo dos que esperam é ligada, sem interrupt (a conexão não é
//!   dele): devolve true ([`Interrupcao::NaFila`]), e esse `consultar`, assim que pegar a conexão, sai com o primeiro
//!   comando interrompido sem executar nada. Sem nenhum `consultar`, devolve false e não faz nada.
//!   A marca e o progress handler só existem dentro do `consultar`, e o interrupt só sai com a marca presente,
//!   sob a trava dela: nunca alcança a próxima operação na conexão (grade, Estrutura, exportação, consulta).
//!
//! Único código `unsafe` (`#![deny(unsafe_code)]` aqui, liberado só no módulo `ffi`, com `// SAFETY:`):
//! `sqlite3_complete` (o rusqlite não o expõe) e a leitura dos nomes de coluna de uma consulta livre, que no
//! rusqlite entraria em pânico com um nome em UTF-8 inválido gravado por outro programa.
#![deny(unsafe_code)]

mod banco;
mod busca;
mod caminhos;
mod consulta;
mod dados;
mod erros;
mod exportacao;
mod ffi;
mod gravacao;
mod leitura;
mod manutencao;
mod meta;
mod sql;
mod tipos;
mod valores;

#[cfg(test)]
mod testes;

pub use banco::{
    Banco, ESPERA_APOS_TRAVA_MS, ESPERA_COPIA_MS, ESPERA_GRAVACAO_MS, ESPERA_LEITURA_MS, LIMITE_CONTAGEM_BYTES,
    LIMITE_TEMP_MEMORIA, LIMITE_TEXTO,
};
pub use busca::{Filtro, Operador};
pub use consulta::Interrupcao;
pub use erros::{MSG_CELULA_MUDOU, MSG_INTERROMPIDA, MSG_SUBSTITUIDA};
pub use exportacao::{FiltroExportacao, LinhasExportacao, Parar};
pub use manutencao::{AcaoManutencao, CopiaFeita, ResultadoManutencao};
pub use sql::dividir_comandos;
pub use tipos::{
    BancoInfo, ChaveEstrangeira, ColunaIndice, ColunaInfo, Contagem, ContagemObjetos, Direcao, FkColuna, GatilhoInfo,
    GatilhoResumo, IndiceInfo, IndiceResumo, LinhaCompleta, LinhaGravada, LinhaPagina, ObjetosInfo, OrigemIndice,
    Pagina, PedidoPagina, ResultadoComando, ResultadoConsulta, SistemaResumo, TabelaInfo, TabelaResumo, TipoChave,
    TipoObjeto, VisaoResumo,
};
pub use valores::{Coagido, coagir};

/// `Banco` é usado por `Arc` de várias threads (§3 item 6 de ARQUITETURA.md).
const _: () = {
    const fn exige_send_sync<T: Send + Sync>() {}
    exige_send_sync::<Banco>();
};
