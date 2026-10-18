//! As regras do DataBox sobre o banco aberto, as preferências e os arquivos: a interface egui chama o
//! [`Servico`] direto, em threads de fundo, e recebe objetos tipados e erros `comum::ErroBanco` (com `status` e
//! `codigo`). Contrato: §4.6 do ARQUITETURA.md.
//!
//! # API pública
//!
//! ```text
//! pub const NOME_APP: &str = "DataBox";         pub fn versao() -> &'static str   (a do Cargo.toml)
//! pub fn arquivo_log() -> PathBuf               %LOCALAPPDATA%\DataBox\databox.log
//!
//! pub struct Servico                            Send + Sync; use por Arc, sempre fora da thread da interface
//! impl Servico {
//!   pub fn novo(prefs: Preferencias) -> Servico                 sem banco aberto; não toca no disco
//!   pub fn com_arquivo_log(self, PathBuf) -> Servico            o registro, se não for o arquivo_log() padrão
//!   pub fn prefs(&self) -> &Preferencias
//!   // aplicação, preferências e arquivos (§4.1)
//!   pub fn app_info(&self) -> AppInfo
//!   pub fn estado(&self) -> Resultado<EstadoApp>                EstadoApp { app, banco: Option<BancoInfo>, prefs }
//!   pub fn gravar_prefs(&self, tema: Option<Tema>, exportacao: Option<MudancaExportacao>) -> Resultado<PrefsInfo>
//!   pub fn gravar_preferencias(&self, mudanca: Mudanca) -> Resultado<PrefsInfo>   tema, aparência e exportação
//!   pub fn remover_recente(&self, caminho: &Path) -> Resultado<PrefsInfo>
//!   pub fn lembrar_diretorio(&self, pasta: &Path)               a pasta (ou o arquivo) do diálogo de abrir
//!   pub fn revelar(&self, caminho: &Path) -> Resultado<()>      explorer /select,"caminho"
//!   pub fn abrir_arquivo(&self, caminho: &Path) -> Resultado<()>   programa associado (explorer "caminho")
//!   // pasta e arquivos de dados (§4.6, etapa 4; os tipos Natureza, Listagem, Entrada e TipoDeArquivo são de
//!   // comum::arquivos)
//!   pub fn alvo_do_caminho(&self, caminho: &Path) -> Resultado<Natureza>   Pasta | Banco | Dados(tipo)
//!   pub fn abrir_pasta(&self, caminho: &Path) -> Resultado<Listagem>        valida, lê a raiz, recente Pasta, pasta_aberta
//!   pub fn listar_pasta(&self, caminho: &Path, anterior: Option<&Listagem>) -> Resultado<Listagem>   um nível
//!       (o abrir_pasta devolve a raiz resolvida, sem nomes 8.3 nem `\\?\`, como o caminho do banco aberto)
//!   pub fn fechar_pasta(&self)                                  apaga pasta_aberta; os recentes ficam
//!   pub fn registrar_arquivo(&self, caminho: &Path) -> Resultado<InfoArquivo>   recente Arquivo
//!   pub fn info_arquivo(&self, caminho: &Path) -> Resultado<InfoArquivo>   o mesmo, sem mexer nos recentes (a ficha)
//!   // banco (§4.2)
//!   pub fn abrir_banco(&self, caminho: &Path, somente_leitura: bool, descartar_transacao: bool)
//!       -> Resultado<BancoInfo>
//!   pub fn novo_banco(&self, caminho: &Path, descartar_transacao: bool) -> Resultado<BancoInfo>
//!   pub fn fechar_banco(&self, banco_id: &str, descartar_transacao: bool) -> Resultado<()>
//!   pub fn info_banco(&self, banco_id: &str) -> Resultado<BancoInfo>
//!   pub fn objetos(&self, banco_id: &str) -> Resultado<ObjetosInfo>
//!   pub fn manutencao(&self, banco_id: &str, acao: AcaoManutencao) -> Resultado<ResultadoManutencao>
//!   pub fn copia(&self, banco_id: &str, caminho: &Path, substituir: bool) -> Resultado<CopiaFeita>
//!   // tabela e dados (§4.3)
//!   pub fn tabela(&self, banco_id: &str, nome: &str) -> Resultado<TabelaCompleta>
//!   pub fn contagem(&self, banco_id: &str, nome: &str, busca: Option<&str>, filtros: &[Filtro],
//!                   leitura: Option<&str> /* Some("contagem:<tabela>") */) -> Resultado<Contagem>
//!   pub fn linhas(&self, banco_id: &str, nome: &str, pedido: &PedidoPagina) -> Resultado<Pagina>
//!   pub fn linha(&self, banco_id: &str, nome: &str, chave: &Chave) -> Resultado<LinhaCompleta>
//!   pub fn inserir(&self, banco_id: &str, nome: &str, valores: &[(String, Entrada)])
//!       -> Resultado<Gravado<LinhaGravada>>
//!   pub fn atualizar(&self, banco_id: &str, nome: &str, chave: &Chave, valores: &[(String, Entrada)])
//!       -> Resultado<Gravado<LinhaGravada>>
//!   pub fn excluir(&self, banco_id: &str, nome: &str, chaves: &[Chave]) -> Resultado<Gravado<usize>>
//!   pub fn salvar_blob(&self, banco_id: &str, nome: &str, chave: &Chave, coluna: &str, caminho: &Path,
//!                      substituir: bool) -> Resultado<BlobSalvo>
//!   // estrutura (§4.4)
//!   pub fn esquema(&self, banco_id: &str, operacao: &OperacaoEsquema, previa: bool) -> Resultado<RespostaEsquema>
//!   // consulta livre (§4.5)
//!   pub fn sql(&self, banco_id: &str, sql: &str, limite: usize) -> Resultado<ResultadoConsulta>
//!   pub fn interromper_sql(&self, banco_id: &str) -> Resultado<bool>   true se alcançou um sql() (em andamento ou
//!       esperando a vez)
//!   pub fn interromper_sql_detalhado(&self, banco_id: &str) -> Resultado<banco::Interrupcao>
//!   pub fn interromper_sql_do_aberto(&self) -> bool   o do banco aberto, sem o id (o fechar do aplicativo)
//!   pub fn sql_em_andamento(&self, banco_id: &str) -> Resultado<bool>   um sql() já pegou a conexão (para os testes)
//!   pub fn historico(&self, banco_id: &str) -> Resultado<Vec<ComandoSql>>       (ItemHistorico = ComandoSql)
//!   // exportação (§4.6)
//!   pub fn exportar(&self, banco_id: &str, pedido: &PedidoExportacao, progresso: Option<Arc<Progresso>>)
//!       -> Resultado<ResumoExportacao>
//! }
//!
//! pub struct AppInfo { nome: &'static str, versao: &'static str, sqlite_versao: &'static str }   (Copy)
//! pub struct EstadoApp { app: AppInfo, banco: Option<BancoInfo>, prefs: PrefsInfo }
//! pub struct TabelaCompleta { info: TabelaInfo, definicoes: Option<Vec<ColunaDef>>, restricoes: Option<Vec<String>>,
//!                             aviso_esquema: Option<String> }
//! pub struct Gravado<T> { resultado: T, transacao_aberta: bool }
//! pub struct BlobSalvo { caminho: PathBuf, bytes: u64 }
//! pub type RespostaManutencao = banco::ResultadoManutencao;   { acao, ok, mensagens, tamanho_antes, tamanho_depois }
//! pub enum OperacaoEsquema { CriarTabela { nome, colunas: Vec<ColunaDef>, restricoes: Vec<String>, sem_rowid },
//!     RenomearTabela { tabela, novo_nome }, ExcluirTabela { tabela }, AdicionarColuna { tabela, coluna: ColunaDef },
//!     AlterarColuna { tabela, coluna /* nome atual */, nova: ColunaDef }, ExcluirColuna { tabela, coluna },
//!     ReordenarColunas { tabela, ordem: Vec<String> }, CriarIndice { nome, tabela, colunas: Vec<ColunaIndicePedida>,
//!     unico, onde: Option<String> }, ExcluirIndice { nome }, CriarVisao { nome, sql /* o SELECT */ },
//!     ExcluirVisao { nome }, CriarGatilho { sql /* CREATE TRIGGER inteiro */ }, ExcluirGatilho { nome } }
//!     · como_str() → "criar_tabela"…
//! pub type ColunaIndicePedida = esquema::ColunaIndice;        { nome: String, desc: bool }
//! pub struct RespostaEsquema { previa, descricao, sql, comandos: Vec<String>, reconstroi, avisos: Vec<String>,
//!                              transacao_aberta }
//! pub struct PedidoExportacao { origem: OrigemExportacao, formato: Formato, caminho: PathBuf, opcoes: Opcoes,
//!                               substituir: bool }
//! pub enum OrigemExportacao { Tabela { nome, filtro: FiltroExportacao }, Consulta { sql },
//!                             Banco { tabelas: Option<Vec<String>> /* None = todas */ } }
//! pub struct ResumoExportacao { caminho /* o gravado de fato */, nome, formato, linhas, bytes, tabelas,
//!                               tempo_ms, avisos }
//! pub struct Progresso                          Send + Sync, por Arc; trava própria (nunca a do banco);
//!                                               um novo a cada chamada de exportar (ver "Exportação acompanhada")
//!   Progresso::novo() -> Arc<Progresso> · cancelar() · cancelado() -> bool · instantaneo() -> ProgressoInfo
//! pub struct ProgressoInfo { fase: Fase, linhas: u64, total: Option<u64>, tabela_atual: Option<String>,
//!                            tabelas_feitas: usize, tabelas_total: usize }   (Default)
//! pub enum Fase { Preparando, Contando, Gravando, Finalizando, Concluida, Cancelada, Erro }
//!     · como_str() → "desconhecida" (Preparando) | "contando" | … · terminou() · TODAS
//! pub struct InfoArquivo { caminho: PathBuf, nome: String, tipo: TipoDeArquivo, tamanho: u64,
//!                          modificado_em: Option<SystemTime> }
//! pub struct ComandoArquivo { programa: PathBuf, argumentos: OsString } · linha() -> String
//! pub fn comando_revelar(caminho: &Path) -> Resultado<ComandoArquivo>   (o que revelar() executa)
//! pub fn comando_abrir(caminho: &Path) -> Resultado<ComandoArquivo>     (o que abrir_arquivo() executa)
//! pub const MSG_SEM_BANCO, MSG_BANCO_TROCADO, MSG_ESQUEMA_LEITURA, MSG_DESTINO_DO_BANCO, MSG_DESTINO_DO_APP,
//!           MSG_EXPORTACAO_CANCELADA, MSG_TAREFA_EM_ANDAMENTO: &str;  PRAZO_CONTAGEM_BANCO: Duration (2 s)
//! ```
//!
//! Reexportados para a tela não precisar procurar: `Formato`, `Opcoes` (exportar), `FiltroExportacao`,
//! `ResultadoManutencao`, `AcaoManutencao`, `CopiaFeita` (banco), `ColunaDef` (ddl), `Tema`, `Mudanca`,
//! `MudancaExportacao`, `MudancaAparencia`, `Aparencia`, `CoresCodigo`, `PrefsInfo`, `ComandoSql` (prefs).
//!
//! # Regras que quem chama precisa saber
//!
//! - **`banco_id`** (o antigo cabeçalho `X-Editor-Banco`): todo método que age sobre o banco aberto recebe o
//!   `BancoInfo.id` que a tela tem. Outro banco aberto (ou nenhum, com um id não vazio) → 409
//!   `Codigo::BancoTrocado` com [`MSG_BANCO_TROCADO`] (releia `estado()` e redesenhe); sem banco e id `""` →
//!   409 [`MSG_SEM_BANCO`]. Nada é feito nesses casos. `fechar_banco` confere do mesmo jeito (a tela que ainda
//!   mostra o A não fecha o B, nem desfaz a transação dele), mas sem banco e id `""` não faz nada (Ok).
//!   `abrir_banco` e `novo_banco` não recebem id (como lá, abrir e novo não conferiam).
//! - **Transação aberta na Consulta SQL** (`BancoInfo.transacao_aberta`): `abrir_banco`, `novo_banco` e
//!   `fechar_banco` desfariam tudo desde o BEGIN → 409 `Codigo::TransacaoAberta` ("Há uma transação aberta na
//!   Consulta SQL; grave (COMMIT) ou descarte (ROLLBACK) antes de fechar o banco ou abrir outro. …"), a não
//!   ser com `descartar_transacao`. Nada é aberto nem criado antes dessa conferência. As gravações da grade e da
//!   Estrutura feitas dentro da transação respondem `transacao_aberta: true` ([`Gravado`],
//!   [`RespostaEsquema`]): só vão para o arquivo com o COMMIT.
//! - **Abrir.** O novo banco é aberto antes de fechar o atual: se ele não abrir (404 inexistente, 400 não é
//!   banco), o atual continua aberto. Caminhos com espaços ou aspas nas pontas são limpos; `~`, `~\…` e
//!   `~fulano\…` são expandidos como no `os.path.expanduser` (vale para todo caminho digitado: banco, cópia,
//!   BLOB, exportação, recentes, revelar, abrir arquivo); vazio → 400 "Informe o caminho do banco.". O aberto
//!   entra nos recentes.
//! - **Trocar de banco com uma operação longa no atual.** A conexão principal do banco é uma só: Consulta SQL
//!   demorada, manutenção (VACUUM…), cópia de segurança e a exportação **com transação aberta** (que lê por
//!   ela até o fim) a seguram enquanto rodam. `abrir_banco`, `novo_banco` e `fechar_banco` **sem**
//!   `descartar_transacao` precisam dela para saber se há transação: esperam a operação terminar (pode levar
//!   minutos numa exportação grande) e só então trocam ou respondem 409 `TransacaoAberta`, como lá. Mostre que
//!   está esperando e não enfileire outra troca igual. A espera não prende as outras trocas: a nova tentativa
//!   **com** `descartar_transacao` troca na hora; o banco novo vale para todos assim que a chamada volta e o
//!   anterior fecha sozinho quando a operação soltar a conexão (quem trocou espera por isso no máximo 0,5 s; a
//!   chamada que esperava recebe 409 `TransacaoAberta` ou `BancoTrocado`). Exportação sem transação lê por
//!   conexão própria e não segura nada: a de uma tabela vai até o fim mesmo depois da troca; a do banco inteiro
//!   abre a leitura de cada tabela ao chegar nela e para na seguinte com 409 "O banco já foi fechado. Abra-o de
//!   novo." (sem código; sem arquivo nem temporário).
//! - **Destinos de arquivo** (exportação, BLOB, cópia): o próprio banco e os `-journal/-wal/-shm` dele → 400
//!   [`MSG_DESTINO_DO_BANCO`]; preferências, a trava delas e o registro (`databox.log`, `.1`…`.9`) → 400
//!   [`MSG_DESTINO_DO_APP`]; arquivo que já existe sem `substituir` → 409 `Codigo::DestinoExiste` ("Já existe
//!   um arquivo com esse nome: x.csv. Confirme para substituí-lo ou escolha outro nome."). Quando o caminho
//!   veio do diálogo de salvar (que já perguntou), passe `substituir: true`. Na exportação, o arquivo conferido
//!   é o gravado de fato (o banco inteiro em CSV vira `.zip`; e se esse `.zip` já existe e não foi o
//!   escolhido, 409 `DestinoExiste` mesmo com `substituir`).
//! - **A pasta é uma noção da interface**: o serviço não guarda a pasta aberta, só lê, valida e lembra (recentes e
//!   `pasta_aberta` das preferências). Os cinco métodos de pasta só tocam no disco e nas preferências, nunca na
//!   conexão do banco aberto: respondem na hora com uma Consulta SQL ou uma exportação segurando o banco. Chame
//!   de uma tarefa de fundo (pasta de rede). `alvo_do_caminho` classifica o que a pessoa digitou, soltou ou
//!   escolheu: pasta, banco (os 16 primeiros bytes `SQLite format 3\0`, ou 0 byte com extensão de banco; a
//!   extensão sozinha mente: `.dat` pode ser banco, `Thumbs.db` não é) ou arquivo de dados (`.csv` `.tsv` `.xlsx`
//!   `.json` `.txt` `.md` `.log`); inexistente → 404 "O arquivo não existe (ou foi movido): x."; o resto → 400 "O
//!   DataBox ainda não abre este tipo de arquivo: x.pdf.". `abrir_pasta`: não existe → 404 "A pasta não existe (ou
//!   foi movida): x."; é arquivo → 400 "O caminho não é uma pasta: x."; só depois de ler a raiz entra nos recentes
//!   (tipo Pasta) e grava `pasta_aberta`. `listar_pasta` lê **um nível** (`comum::arquivos::Listagem`): pastas,
//!   bancos e arquivos de dados em ordem natural sem caixa, os ocultos do Windows, os `~$` do Office e as pastas
//!   de ferramentas fora (um ponto no começo do nome não esconde), atalhos de pasta seguidos (os quebrados saem),
//!   corte em 500 (`a_mais`), prazo de 5 s, 20.000 entradas lidas e 2.000 cabeçalhos lidos (o que passar marca
//!   `parcial`); com `anterior`, o cabeçalho dos bancos com o mesmo tamanho e a mesma data não é lido de novo. A
//!   entrada de uma pasta vem sem tamanho e sem data (o NTFS muda a data de uma subpasta, vista de cima, quando
//!   alguém a abre): a releitura sem mudança nenhuma devolve uma `Listagem` igual (`==`) à de antes, e a casca não
//!   repinta. `abrir_pasta` e `info_arquivo` devolvem o caminho resolvido como o do banco aberto; `listar_pasta`
//!   mantém o caminho que recebeu (um atalho de pasta continua o atalho). `registrar_arquivo` põe o arquivo de dados
//!   nos recentes (tipo Arquivo); `abrir_banco` e `novo_banco` seguem entrando como banco.
//! - **`tabela()`**: em tabela comum, junta `definicoes`/`restricoes` do crate esquema; se o analisador não
//!   entender a definição, só a estrutura fica bloqueada (`info.esquema_editavel` false e `aviso_esquema`).
//! - **`esquema(operacao, previa)`**: banco só leitura → 403 [`MSG_ESQUEMA_LEITURA`]. Com `previa: false` o
//!   plano é montado de novo e executado na mesma chamada, sem soltar a trava do banco (não guarde o plano da
//!   prévia).
//! - **`sql()`** registra o texto no histórico do banco quando nenhum comando deu erro.
//! - **`interromper_sql()`** (o Interromper; chame de outra thread que não a do `sql()`): nunca espera a trava
//!   do banco. Com um `sql()` de posse da conexão, devolve true e o comando em andamento volta como
//!   `ResultadoComando::Erro` com `banco::MSG_INTERROMPIDA` ("A consulta foi interrompida."); os seguintes não
//!   rodam e o roteiro não entra no histórico. Com nenhum `sql()` de posse da conexão mas um esperando uma
//!   operação longa soltá-la, devolve true também (`Interrupcao::NaFila` no `interromper_sql_detalhado`): a marca
//!   dele é ligada, sem interrupt, e ele sai interrompido, sem executar nada, quando pegar a conexão. false quando
//!   nenhum `sql()` existe: nada muda, e a próxima consulta roda normalmente.
//! - **`estado()`** lê os recentes no disco (até 2 s com unidade de rede fora): chame numa thread de fundo.
//! - **Diálogos (na interface, com rfd)**: as regras que a janela segue (pasta inicial que não existe → a
//!   padrão do Windows):
//!   1. **Salvar** (Exportar, Salvar BLOB, cópia de segurança): começa na pasta do banco aberto
//!      (`BancoInfo.pasta`); sem banco, em `prefs().ultimo_diretorio()`. Sem nome sugerido, "dados". O caminho
//!      escolhido vai com `substituir: true` (o diálogo já perguntou). A pasta escolhida aqui não é lembrada.
//!   2. **Abrir banco**: começa em `prefs().ultimo_diretorio()`; o arquivo escolhido vai para
//!      [`Servico::lembrar_diretorio`] (que guarda a pasta dele) antes de `abrir_banco`. Cancelado → nada.
//!   3. **Escolher arquivo** (conteúdo de um BLOB): título "Escolher arquivo" quando a tela não dá outro
//!      (ex.: "Conteúdo da coluna foto"); todos os arquivos (`*.*`).
//!
//! # Exportação acompanhada (§4.6)
//!
//! Sem `progresso`, nada é contado nem registrado. Com ele, a tela cria um **`Progresso::novo()` a cada chamada
//! de `exportar`**, inclusive na nova tentativa com `substituir` depois do 409 `DestinoExiste`, passa um clone do `Arc` e lê o outro a cada quadro. Reaproveitado, o andamento
//! começa mostrando o fim da vez anterior (`terminou()` verdadeiro) e um cancelamento feito antes de a
//! exportação nova chegar se perde.
//!
//! - **o fim é a volta de `Servico::exportar`** (Ok/Err). Ao voltar, o `Progresso` passado está terminado
//!   (`Concluida` | `Cancelada` | `Erro`), também quando a exportação foi recusada antes de começar (sem banco,
//!   banco trocado → `Erro`). A exceção é o 409 [`MSG_TAREFA_EM_ANDAMENTO`]: o `Progresso` é da outra
//!   exportação, que segue;
//! - fases: [`Fase::Preparando`] (a exportação ainda não começou) → `Contando` assim que ela começa (tabela:
//!   contagem com os mesmos busca/filtros/chaves, `tabela_atual` = a tabela; banco: soma das tabelas, ou
//!   `total` None se passar de [`PRAZO_CONTAGEM_BANCO`]; consulta: não conta e passa direto) → `Gravando`
//!   (`total`; `linhas` a cada ~1.000; no banco inteiro, `tabela_atual` e `tabelas_feitas`/`tabelas_total`)
//!   → `Finalizando` (`tabelas_feitas` = total) → `Concluida` (`linhas` = as do resumo) | `Cancelada` |
//!   `Erro` (as demais informações ficam como estavam). Se a thread da exportação entrar em pânico, o
//!   andamento termina em `Erro` (não fica "rodando");
//! - [`Progresso::cancelar`] interrompe no próximo aviso ou na hora, se a exportação espera o SQLite
//!   (contagem, ordenação), pelo progress handler da conexão dela. A exportação devolve 409
//!   `Codigo::Cancelado` [`MSG_EXPORTACAO_CANCELADA`]; o temporário é apagado e o destino fica como estava.
//!   Cancelar antes de a exportação começar fica anotado: ela termina cancelada sem gravar nada. Exceção: antes
//!   de cada leitura (a contagem, cada tabela; no banco inteiro sem lista, também `objetos()`, que conta as
//!   linhas), a exportação passa pela conexão principal do banco; se uma operação longa a segura (Consulta SQL
//!   demorada, VACUUM, cópia de segurança), o cancelamento só é atendido quando ela soltar (como lá; o
//!   andamento continua na fase em que estava, ex. `Contando`);
//! - o mesmo `Progresso` em duas exportações ao mesmo tempo → a segunda recebe 409
//!   [`MSG_TAREFA_EM_ANDAMENTO`] e a primeira segue;
//! - [`Progresso::instantaneo`] e `cancelar` usam só a trava do próprio `Progresso`: respondem em
//!   microssegundos durante uma exportação de 1 milhão de linhas, mesmo com a exportação segurando a conexão
//!   principal do banco (transação aberta);
//! - as opções usadas (as cinco de [`Opcoes`]) viram o padrão das preferências.
//!
//! # Detalhes
//!
//! - Os "destinos confirmados pelo diálogo" viram `substituir: true`.
//! - `abrir_arquivo` usa `explorer.exe "caminho"` (sem `unsafe` para o `ShellExecuteW`): um arquivo sem
//!   programa associado abre a janela "Abrir com" do Windows.
//! - O `Progresso` da exportação é da tela; a fase de antes de começar é `Preparando`. Recusada antes de
//!   começar (sem banco, banco trocado), a exportação deixa o `Progresso` em `Erro`.
//! - Trocar de banco com `descartar_transacao` não espera o anterior fechar. Sem descartar, `fechar_banco`
//!   confere a transação antes de prender a trava das trocas, como abrir e novo.
//! - O detalhe do 409 `BancoTrocado` diz `banco_id: '…'`; o 409 [`MSG_TAREFA_EM_ANDAMENTO`] vem sem detalhe.
//! - As opções de exportação são tipadas ([`Opcoes`]): as cinco viram padrão a cada exportação.
//!
//! # Testes
//!
//! `src/testes/`: cada operação do serviço (`api.rs`), a exportação acompanhada e as páginas e contagens em
//! tabelas grandes (`volumes.rs`, com threads de verdade e tempos medidos: ler o andamento durante 1 milhão de
//! linhas, cancelar no meio, na contagem e antes de começar, `Progresso` reaproveitado, troca de banco no meio
//! de uma exportação), a exportação do banco de exemplo (`exemplo.rs`), a montagem dos comandos de "Mostrar na
//! pasta" e "Abrir" (`arquivos.rs`), o `~` e as variáveis nos caminhos digitados (`caminhos.rs`) e trocas de banco
//! concorrentes e durante uma operação longa (`concorrencia.rs`), a leitura da pasta com pastas temporárias de
//! verdade (`pasta.rs`: atributo oculto por `attrib +h`, junções por `mklink /J`, limites reduzidos, o banco aberto
//! com a conexão presa e o tempo de 10.000 arquivos). Os arquivos
//! exportados são relidos por `leitura.rs` (DEFLATE/.zip, JSON, CSV e células do .xlsx escritos à mão: o crate
//! não tem zip nem serde_json). Os testes pesados e os que medem tempo rodam um de cada vez (`exclusivo()`).
//! As preferências de teste ficam sempre num arquivo da pasta temporária do teste.

#![forbid(unsafe_code)]

mod arquivos;
mod caminhos;
mod exportacao;
mod operacoes;
mod pasta;
mod progresso;
mod servico;
mod tipos;

#[cfg(test)]
mod testes;

use std::time::Duration;

pub use arquivos::{ComandoArquivo, comando_abrir, comando_revelar};
pub use caminhos::arquivo_log;
pub use exportacao::{OrigemExportacao, PedidoExportacao};
pub use operacoes::{ColunaIndicePedida, OperacaoEsquema};
pub use pasta::InfoArquivo;
pub use progresso::{Fase, Progresso, ProgressoInfo};
pub use servico::Servico;
pub use tipos::{AppInfo, BlobSalvo, EstadoApp, Gravado, RespostaEsquema, ResumoExportacao, TabelaCompleta};

pub use banco::{AcaoManutencao, CopiaFeita, FiltroExportacao, ResultadoManutencao};
pub use ddl::ColunaDef;
pub use exportar::{Formato, Opcoes};
pub use prefs::{Aparencia, ComandoSql, CoresCodigo, Mudanca, MudancaAparencia, MudancaExportacao, PrefsInfo, Tema};

/// A resposta de `manutencao` (`{acao, ok, mensagens, tamanho_antes, tamanho_depois}`).
pub type RespostaManutencao = ResultadoManutencao;
/// Um item do histórico da Consulta SQL (`{sql, quando}`).
pub type ItemHistorico = ComandoSql;

/// O nome do aplicativo ("DataBox"), o de `comum::app`.
pub const NOME_APP: &str = comum::app::NOME;

pub const MSG_SEM_BANCO: &str = "Nenhum banco aberto.";
pub const MSG_ESQUEMA_LEITURA: &str = "O banco foi aberto só para leitura; a estrutura dele não pode ser mudada.";
pub const MSG_BANCO_TROCADO: &str = "O banco aberto mudou (outra janela abriu ou fechou um banco); nada foi feito. \
                                     A tela vai ser atualizada com o banco atual.";
pub const MSG_DESTINO_DO_BANCO: &str = "Esse arquivo pertence ao banco aberto (é ele mesmo ou um dos arquivos \
                                        -journal, -wal e -shm que o SQLite cria ao lado dele); escolha outro nome.";
pub const MSG_DESTINO_DO_APP: &str =
    "Esse arquivo é do próprio DataBox (preferências ou arquivo de log); escolha outro nome.";
pub const MSG_EXPORTACAO_CANCELADA: &str = "Exportação interrompida; nenhum arquivo foi gravado.";
pub const MSG_TAREFA_EM_ANDAMENTO: &str = "Já há uma exportação em andamento com essa tarefa.";

/// Exportar o banco inteiro: se contar todas as tabelas escolhidas passar disto, o total fica None.
pub const PRAZO_CONTAGEM_BANCO: Duration = Duration::from_secs(2);

/// A versão do aplicativo (a do Cargo.toml do workspace).
pub fn versao() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// `Servico` e `Progresso` são usados por `Arc` de várias threads (§3 item 6 de ARQUITETURA.md).
const _: () = {
    const fn exige_send_sync<T: Send + Sync>() {}
    exige_send_sync::<Servico>();
    exige_send_sync::<Progresso>();
};
