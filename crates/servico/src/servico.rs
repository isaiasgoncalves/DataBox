//! O [`Servico`]: o banco aberto, as preferências e as regras de cada operação.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::Duration;

use banco::{
    AcaoManutencao, Banco, BancoInfo, Contagem, CopiaFeita, Filtro, Interrupcao, LinhaCompleta, LinhaGravada,
    ObjetosInfo, Pagina, PedidoPagina, ResultadoComando, ResultadoConsulta, ResultadoManutencao, TipoObjeto,
};
use comum::{Celula, Chave, Codigo, Entrada, ErroBanco, Resultado};
use prefs::{ComandoSql, Mudanca, MudancaExportacao, Preferencias, PrefsInfo, ResultadoSql, Tema};

use crate::arquivos::{ComandoArquivo, comando_abrir, comando_revelar};
use crate::caminhos::{self, arquivos_do_app, chave_destino, limpar_caminho};
use crate::operacoes::{OperacaoEsquema, montar_plano};
use crate::tipos::{AppInfo, BlobSalvo, EstadoApp, Gravado, RespostaEsquema, TabelaCompleta};
use crate::{
    MSG_BANCO_TROCADO, MSG_DESTINO_DO_APP, MSG_DESTINO_DO_BANCO, MSG_ESQUEMA_LEITURA, MSG_SEM_BANCO, NOME_APP,
    PRAZO_CONTAGEM_BANCO,
};

/// O que a tela pede, sem HTTP: um banco aberto por vez e as preferências. `Send + Sync`: use por `Arc`, de
/// threads de fundo (nunca da thread da interface: quase tudo toca no SQLite ou no disco).
pub struct Servico {
    pub(crate) prefs: Preferencias,
    /// O banco aberto (o `app.state.banco`); a trava só é presa para ler ou trocar o `Arc`.
    banco: Mutex<Option<Arc<Banco>>>,
    /// Uma troca de banco por vez (o `app.state.lock_banco`).
    troca: Mutex<()>,
    /// O registro do aplicativo, que um arquivo exportado não pode substituir.
    arquivo_log: PathBuf,
    /// Exportar o banco inteiro: se contar todas as tabelas passar disto, o total fica None.
    pub(crate) prazo_contagem_banco: Duration,
}

impl fmt::Debug for Servico {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Servico")
            .field("prefs", &self.prefs)
            .field("banco", &self.atual().map(|b| b.caminho().to_path_buf()))
            .finish_non_exhaustive()
    }
}

/// O texto entre aspas simples, para o detalhe técnico.
fn repr(texto: &str) -> String {
    format!("'{}'", texto.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn travar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A ação que a transação aberta impede (o texto do 409 `TransacaoAberta`).
const ACAO_TROCA: &str = "fechar o banco ou abrir outro";

/// Quanto quem troca de banco espera o anterior fechar; passado isso, ele fecha sozinho quando a operação que
/// segura a conexão principal dele terminar.
pub(crate) const ESPERA_FECHAR_ANTERIOR: Duration = Duration::from_millis(500);

/// 409 `Codigo::BancoTrocado` para quem mandou `banco_id`.
fn erro_trocado(banco_id: &str) -> ErroBanco {
    ErroBanco::com_status(MSG_BANCO_TROCADO, 409)
        .detalhe(format!("banco_id: {}", repr(banco_id)))
        .codigo(Codigo::BancoTrocado)
}

/// A conferência do banco da tela: o id que a tela tem precisa ser o do banco aberto, ou
/// `""` sem banco nenhum; senão, 409 `Codigo::BancoTrocado`.
fn conferir_id(atual: Option<&Banco>, banco_id: &str) -> Resultado<()> {
    match atual {
        None if banco_id.is_empty() => Ok(()),
        Some(banco) if banco.id() == banco_id => Ok(()),
        _ => Err(erro_trocado(banco_id)),
    }
}

/// Fecha o banco que saiu numa thread à parte e espera até `espera` por ela: uma operação longa na conexão
/// principal dele (exportação com transação aberta, Consulta SQL, VACUUM, cópia de segurança) segura o
/// fechamento até terminar, e quem trocou de banco não fica esperando junto.
fn fechar_anterior(banco: Arc<Banco>, espera: Duration) {
    let (aviso, fechado) = mpsc::channel::<()>();
    let para_thread = Arc::clone(&banco);
    let iniciada = thread::Builder::new().name("fechar banco anterior".into()).spawn(move || {
        para_thread.fechar();
        let _ = aviso.send(());
    });
    match iniciada {
        Ok(_) => {
            if fechado.recv_timeout(espera).is_err() {
                log::info!(
                    "O banco {} vai ser fechado quando a operação em andamento nele terminar.",
                    banco.caminho().display()
                );
            }
        }
        Err(e) => {
            log::warn!("Sem thread para fechar o banco anterior ({e}); fechando nesta.");
            banco.fechar();
        }
    }
}

impl Servico {
    /// O serviço sem banco aberto. Não toca no disco.
    pub fn novo(prefs: Preferencias) -> Servico {
        Servico {
            prefs,
            banco: Mutex::new(None),
            troca: Mutex::new(()),
            arquivo_log: caminhos::arquivo_log(),
            prazo_contagem_banco: PRAZO_CONTAGEM_BANCO,
        }
    }

    /// O registro do aplicativo (para recusá-lo como destino), quando não é o padrão [`crate::arquivo_log`].
    pub fn com_arquivo_log(mut self, arquivo_log: PathBuf) -> Servico {
        self.arquivo_log = arquivo_log;
        self
    }

    pub fn prefs(&self) -> &Preferencias {
        &self.prefs
    }

    /// O banco aberto agora, se houver.
    pub(crate) fn atual(&self) -> Option<Arc<Banco>> {
        travar(&self.banco).clone()
    }

    /// O banco aberto, conferido pelo `banco_id`: sem banco → 409 "Nenhum banco
    /// aberto."; outro banco (ou nenhum, para quem ainda mostra um) → 409 `Codigo::BancoTrocado`.
    pub(crate) fn banco_aberto(&self, banco_id: &str) -> Resultado<Arc<Banco>> {
        match self.atual() {
            None if banco_id.is_empty() => Err(ErroBanco::com_status(MSG_SEM_BANCO, 409)),
            None => Err(erro_trocado(banco_id)),
            Some(banco) if banco.id() == banco_id => Ok(banco),
            Some(_) => Err(erro_trocado(banco_id)),
        }
    }

    /// Põe o banco novo no lugar e fecha o anterior (um banco por vez).
    ///
    /// Fechar a conexão desfaz uma transação aberta na Consulta SQL, e com ela tudo o que foi salvo desde o
    /// BEGIN (também pela grade e pela Estrutura). Sem `descartar_transacao`, a troca é recusada (409).
    /// `esperado` (o `banco_id` de quem pede para fechar) é conferido aqui, com a troca presa: outra troca não
    /// passa entre conferir e trocar.
    ///
    /// O anterior fecha numa thread à parte: quem pediu a troca espera no máximo [`ESPERA_FECHAR_ANTERIOR`]
    /// (uma operação longa na conexão principal dele, como uma exportação com transação aberta, só solta a
    /// conexão no fim). O banco novo já vale para todos assim que a troca é feita.
    fn trocar_banco(
        &self,
        novo: Option<Arc<Banco>>,
        descartar_transacao: bool,
        esperado: Option<&str>,
    ) -> Resultado<()> {
        let troca = travar(&self.troca);
        let anterior = self.atual();
        let mesmo = matches!((&anterior, &novo), (Some(a), Some(n)) if Arc::ptr_eq(a, n));
        let conferido =
            esperado.map_or(Ok(()), |id| conferir_id(anterior.as_deref(), id)).and_then(|()| match &anterior {
                Some(a) if !mesmo && !descartar_transacao => a.exigir_sem_transacao(ACAO_TROCA),
                _ => Ok(()),
            });
        if let Err(e) = conferido {
            drop(troca);
            if let Some(n) = &novo {
                n.fechar();
            }
            return Err(e);
        }
        *travar(&self.banco) = novo;
        drop(troca);
        if let Some(a) = anterior
            && !mesmo
        {
            fechar_anterior(a, ESPERA_FECHAR_ANTERIOR);
        }
        Ok(())
    }

    /// Recusa gravar um arquivo (exportação, BLOB, cópia) onde isso estragaria algo.
    ///
    /// - o próprio banco aberto e os -journal/-wal/-shm dele (400);
    /// - os arquivos do aplicativo: preferências, a trava delas e o registro (400);
    /// - um arquivo que já existe, sem `substituir` (409 `Codigo::DestinoExiste`).
    pub(crate) fn conferir_destino(&self, destino: &Path, substituir: bool, banco: Option<&Banco>) -> Resultado<()> {
        let texto = destino.display().to_string();
        if banco.is_some_and(|b| b.e_arquivo_do_banco(destino)) {
            return Err(ErroBanco::novo(MSG_DESTINO_DO_BANCO).detalhe(texto));
        }
        let do_app: HashSet<String> = arquivos_do_app(&self.prefs, &self.arquivo_log);
        if do_app.contains(&chave_destino(destino)) {
            return Err(ErroBanco::novo(MSG_DESTINO_DO_APP).detalhe(texto));
        }
        if !substituir && destino.is_file() {
            let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Err(ErroBanco::com_status(
                format!(
                    "Já existe um arquivo com esse nome: {nome}. Confirme para substituí-lo ou escolha outro nome."
                ),
                409,
            )
            .detalhe(texto)
            .codigo(Codigo::DestinoExiste));
        }
        Ok(())
    }

    // ---- aplicação, preferências e arquivos (§4.1) -------------------------------------------------

    pub fn app_info(&self) -> AppInfo {
        AppInfo { nome: NOME_APP, versao: crate::versao(), sqlite_versao: rusqlite::version() }
    }

    /// `GET /api/estado`: aplicativo, banco aberto (BancoInfo) e preferências (lê os recentes no disco).
    pub fn estado(&self) -> Resultado<EstadoApp> {
        let banco = match self.atual() {
            None => None,
            Some(b) => match b.info() {
                Ok(info) => Some(info),
                // trocado (e o anterior fechado) entre pegar o Arc e ler: vale o atual
                Err(e) => match self.atual() {
                    None => None,
                    Some(agora) if !Arc::ptr_eq(&agora, &b) => Some(agora.info()?),
                    Some(_) => return Err(e),
                },
            },
        };
        Ok(EstadoApp { app: self.app_info(), banco, prefs: self.prefs.resumo() })
    }

    /// `POST /api/prefs`: muda tema e/ou opções de exportação (None não mexe). Separador inválido → 400.
    pub fn gravar_prefs(&self, tema: Option<Tema>, exportacao: Option<MudancaExportacao>) -> Resultado<PrefsInfo> {
        self.prefs.atualizar(Mudanca { tema, exportacao, ..Mudanca::default() })?;
        Ok(self.prefs.resumo())
    }

    /// O Salvar das Preferências: tema, aparência do código e exportação de uma vez (None em cada campo não mexe),
    /// com as regras de `Preferencias::atualizar`. Devolve o resumo depois da gravação; valor inválido → 400, sem
    /// gravar nada.
    pub fn gravar_preferencias(&self, mudanca: Mudanca) -> Resultado<PrefsInfo> {
        self.prefs.atualizar(mudanca)?;
        Ok(self.prefs.resumo())
    }

    /// `POST /api/prefs/recentes/remover`. Caminho vazio → 400 "Informe o caminho do arquivo.".
    pub fn remover_recente(&self, caminho: &Path) -> Resultado<PrefsInfo> {
        let caminho = limpar_caminho(caminho, "o caminho do arquivo")?;
        self.prefs.remover_recente(&caminho);
        Ok(self.prefs.resumo())
    }

    /// A última pasta usada num diálogo de abrir (o `ultimo_diretorio` das preferências), para o próximo
    /// começar nela. Aceita a pasta ou o arquivo escolhido (que vira a pasta dele). Erro só vai para o registro.
    pub fn lembrar_diretorio(&self, pasta: &Path) {
        if let Err(e) = self.prefs.atualizar(Mudanca::ultimo_diretorio(Some(pasta.to_path_buf()))) {
            log::info!("Última pasta não guardada ({}): {e}", pasta.display());
        }
    }

    /// `POST /api/arquivo/revelar`: o Explorador de Arquivos na pasta do arquivo, com ele selecionado.
    /// Vazio → 400; inexistente → 404 "O arquivo não existe (ou foi movido): …".
    pub fn revelar(&self, caminho: &Path) -> Resultado<()> {
        self.preparar_revelar(caminho)?.executar(if cfg!(windows) {
            "Não foi possível abrir o Explorador de Arquivos."
        } else {
            "Não foi possível abrir o gerenciador de arquivos."
        })
    }

    /// `POST /api/arquivo/abrir`: o arquivo no programa associado. Erros como em
    /// [`Servico::revelar`].
    pub fn abrir_arquivo(&self, caminho: &Path) -> Resultado<()> {
        self.preparar_abrir(caminho)?.executar("Não foi possível abrir o arquivo.")
    }

    /// O comando que [`Servico::revelar`] executa (caminho limpo e conferido).
    pub(crate) fn preparar_revelar(&self, caminho: &Path) -> Resultado<ComandoArquivo> {
        comando_revelar(&limpar_caminho(caminho, "o caminho do arquivo")?)
    }

    /// O comando que [`Servico::abrir_arquivo`] executa (caminho limpo e conferido).
    pub(crate) fn preparar_abrir(&self, caminho: &Path) -> Resultado<ComandoArquivo> {
        comando_abrir(&limpar_caminho(caminho, "o caminho do arquivo")?)
    }

    // ---- banco (§4.2) ------------------------------------------------------------------------------

    /// `POST /api/banco/abrir`: abre o arquivo e fecha o anterior; entra nos recentes. Transação aberta no
    /// atual → 409 `TransacaoAberta` (a não ser com `descartar_transacao`). Se o novo não abrir, o atual
    /// continua aberto.
    pub fn abrir_banco(
        &self,
        caminho: &Path,
        somente_leitura: bool,
        descartar_transacao: bool,
    ) -> Resultado<BancoInfo> {
        let caminho = limpar_caminho(caminho, "o caminho do banco")?;
        if let Some(atual) = self.atual()
            && !descartar_transacao
        {
            atual.exigir_sem_transacao(ACAO_TROCA)?; // antes de abrir o novo à toa
        }
        // abre o novo antes de fechar o atual: se o arquivo não abrir, o banco atual continua aberto
        let novo = Arc::new(Banco::abrir(&caminho, somente_leitura, false)?);
        self.instalar(novo, descartar_transacao)
    }

    /// `POST /api/banco/novo`: cria um arquivo vazio e o abre (como [`Servico::abrir_banco`]). Pasta → 400;
    /// pasta que não existe → 404; arquivo existente e não vazio → 409.
    pub fn novo_banco(&self, caminho: &Path, descartar_transacao: bool) -> Resultado<BancoInfo> {
        if let Some(atual) = self.atual()
            && !descartar_transacao
        {
            atual.exigir_sem_transacao(ACAO_TROCA)?;
        }
        let caminho = limpar_caminho(caminho, "o caminho do banco novo")?;
        if caminho.is_dir() {
            return Err(ErroBanco::novo(format!(
                "O caminho é uma pasta; informe também o nome do arquivo: {}.",
                caminho.display()
            )));
        }
        let pasta = caminho.parent().unwrap_or(Path::new(""));
        if !pasta.is_dir() {
            return Err(ErroBanco::com_status(
                format!("A pasta não existe: {}. Escolha outra pasta ou crie esta antes.", pasta.display()),
                404,
            ));
        }
        if std::fs::metadata(&caminho).is_ok_and(|m| m.len() > 0) {
            return Err(ErroBanco::com_status(
                format!(
                    "Já existe um arquivo com esse nome e ele não está vazio: {}. Escolha outro nome ou abra o arquivo \
                     existente.",
                    caminho.display()
                ),
                409,
            ));
        }
        let novo = Arc::new(Banco::abrir(&caminho, false, true)?);
        self.instalar(novo, descartar_transacao)
    }

    /// Lê o BancoInfo do banco recém-aberto, põe no lugar do atual e guarda nos recentes.
    fn instalar(&self, novo: Arc<Banco>, descartar_transacao: bool) -> Resultado<BancoInfo> {
        let info = match novo.info() {
            Ok(info) => info,
            Err(e) => {
                novo.fechar();
                return Err(e);
            }
        };
        self.trocar_banco(Some(Arc::clone(&novo)), descartar_transacao, None)?;
        self.prefs.adicionar_recente(novo.caminho());
        Ok(info)
    }

    /// `POST /api/banco/fechar`, com o `banco_id` que a tela tem: outro banco aberto,
    /// ou nenhum com um id não vazio → 409 `Codigo::BancoTrocado` (nada é fechado); sem banco e id `""` → não
    /// faz nada. Transação aberta → 409 `TransacaoAberta` (a não ser com `descartar_transacao`, que a desfaz).
    pub fn fechar_banco(&self, banco_id: &str, descartar_transacao: bool) -> Resultado<()> {
        let atual = self.atual();
        conferir_id(atual.as_deref(), banco_id)?;
        if let Some(atual) = atual
            && !descartar_transacao
        {
            // fora da trava das trocas: esperar aqui (uma operação longa na conexão principal) não impede outra
            // troca, como a nova tentativa descartando a transação
            atual.exigir_sem_transacao(ACAO_TROCA)?;
        }
        self.trocar_banco(None, descartar_transacao, Some(banco_id))
    }

    /// `GET /api/banco`.
    pub fn info_banco(&self, banco_id: &str) -> Resultado<BancoInfo> {
        self.banco_aberto(banco_id)?.info()
    }

    /// `GET /api/banco/objetos`.
    pub fn objetos(&self, banco_id: &str) -> Resultado<ObjetosInfo> {
        self.banco_aberto(banco_id)?.objetos()
    }

    /// `POST /api/banco/manutencao` → `{acao, ok, mensagens, tamanho_antes, tamanho_depois}`.
    pub fn manutencao(&self, banco_id: &str, acao: AcaoManutencao) -> Resultado<ResultadoManutencao> {
        self.banco_aberto(banco_id)?.manutencao(acao)
    }

    /// `POST /api/banco/copia`: cópia de segurança; destinos conferidos (ver a documentação do crate).
    pub fn copia(&self, banco_id: &str, caminho: &Path, substituir: bool) -> Resultado<CopiaFeita> {
        let banco = self.banco_aberto(banco_id)?;
        let destino = limpar_caminho(caminho, "o caminho da cópia")?;
        self.conferir_destino(&destino, substituir, Some(&banco))?;
        banco.copia_de_seguranca(&destino)
    }

    // ---- tabela e dados (§4.3) ---------------------------------------------------------------------

    /// `GET /api/tabela`: TabelaInfo com as definições do esquema. Uma definição que o analisador não entende
    /// só bloqueia a estrutura (`esquema_editavel` false e `aviso_esquema`); os dados continuam editáveis.
    pub fn tabela(&self, banco_id: &str, nome: &str) -> Resultado<TabelaCompleta> {
        let banco = self.banco_aberto(banco_id)?;
        let mut info = banco.tabela_info(nome)?;
        let (mut definicoes, mut restricoes, mut aviso_esquema) = (None, None, None);
        if info.tipo == TipoObjeto::Table {
            match banco.com_conexao(|conn| ler_definicoes(conn, &info.nome)) {
                Ok(defs) => {
                    definicoes = Some(defs.colunas);
                    restricoes = Some(defs.restricoes);
                }
                Err(e) => {
                    log::debug!("Definição de {} não analisada: {e}", info.nome);
                    if info.esquema_editavel {
                        info.esquema_editavel = false;
                        aviso_esquema = Some(
                            ddl::aparar(&format!(
                                "Não foi possível entender a definição desta tabela, então a estrutura não pode ser \
                                 mudada por aqui (use a Consulta SQL). {}",
                                e.mensagem
                            ))
                            .to_string(),
                        );
                    }
                }
            }
        }
        Ok(TabelaCompleta { info, definicoes, restricoes, aviso_esquema })
    }

    /// `GET /api/tabela/contagem`. `leitura: Some("contagem:<tabela>")` liga a leitura substituída.
    pub fn contagem(
        &self,
        banco_id: &str,
        nome: &str,
        busca: Option<&str>,
        filtros: &[Filtro],
        leitura: Option<&str>,
    ) -> Resultado<Contagem> {
        self.banco_aberto(banco_id)?.contagem(nome, busca, filtros, leitura)
    }

    /// `GET /api/tabela/linhas`.
    pub fn linhas(&self, banco_id: &str, nome: &str, pedido: &PedidoPagina) -> Resultado<Pagina> {
        self.banco_aberto(banco_id)?.linhas(nome, pedido)
    }

    /// `GET /api/tabela/linha`: a linha inteira (texto sem cortar).
    pub fn linha(&self, banco_id: &str, nome: &str, chave: &Chave) -> Resultado<LinhaCompleta> {
        self.banco_aberto(banco_id)?.linha(nome, chave)
    }

    /// `POST /api/tabela/linhas`.
    pub fn inserir(
        &self,
        banco_id: &str,
        nome: &str,
        valores: &[(String, Entrada)],
    ) -> Resultado<Gravado<LinhaGravada>> {
        let banco = self.banco_aberto(banco_id)?;
        let resultado = banco.inserir(nome, valores)?;
        Ok(Gravado { resultado, transacao_aberta: banco.transacao_aberta() })
    }

    /// `PATCH /api/tabela/linhas`.
    pub fn atualizar(
        &self,
        banco_id: &str,
        nome: &str,
        chave: &Chave,
        valores: &[(String, Entrada)],
    ) -> Resultado<Gravado<LinhaGravada>> {
        let banco = self.banco_aberto(banco_id)?;
        let resultado = banco.atualizar(nome, chave, valores)?;
        Ok(Gravado { resultado, transacao_aberta: banco.transacao_aberta() })
    }

    /// O Desfazer da edição de uma célula na grade: grava `antigo` de volta só se a célula ainda tem `gravado` (ver
    /// [`banco::Banco::desfazer_celula`]: 409 [`banco::MSG_CELULA_MUDOU`] se ela mudou depois). Numa transação
    /// aberta na Consulta SQL o UPDATE entra nela, como a edição.
    pub fn desfazer_celula(
        &self,
        banco_id: &str,
        nome: &str,
        chave: &Chave,
        coluna: &str,
        gravado: &Celula,
        antigo: &Celula,
    ) -> Resultado<Gravado<LinhaGravada>> {
        let banco = self.banco_aberto(banco_id)?;
        let resultado = banco.desfazer_celula(nome, chave, coluna, gravado, antigo)?;
        Ok(Gravado { resultado, transacao_aberta: banco.transacao_aberta() })
    }

    /// `DELETE /api/tabela/linhas` → quantas linhas saíram.
    pub fn excluir(&self, banco_id: &str, nome: &str, chaves: &[Chave]) -> Resultado<Gravado<usize>> {
        let banco = self.banco_aberto(banco_id)?;
        let resultado = banco.excluir(nome, chaves)?;
        Ok(Gravado { resultado, transacao_aberta: banco.transacao_aberta() })
    }

    /// `POST /api/tabela/blob/salvar`: o conteúdo da célula num arquivo. NULL → 400 "A célula está vazia
    /// (NULL); não há conteúdo para salvar."; destinos conferidos (ver a documentação do crate).
    pub fn salvar_blob(
        &self,
        banco_id: &str,
        nome: &str,
        chave: &Chave,
        coluna: &str,
        caminho: &Path,
        substituir: bool,
    ) -> Resultado<BlobSalvo> {
        let banco = self.banco_aberto(banco_id)?;
        let destino = limpar_caminho(caminho, "o caminho do arquivo")?;
        let Some(dados) = banco.ler_blob(nome, chave, coluna)? else {
            return Err(ErroBanco::novo("A célula está vazia (NULL); não há conteúdo para salvar."));
        };
        self.conferir_destino(&destino, substituir, Some(&banco))?;
        caminhos::gravar_arquivo(&destino, &dados)?;
        Ok(BlobSalvo { caminho: destino, bytes: dados.len() as u64 })
    }

    // ---- estrutura (§4.4) --------------------------------------------------------------------------

    /// `POST /api/esquema`. `previa` true: só o plano (nada muda). false: monta o plano de novo e executa na
    /// mesma chamada, sem soltar a trava do banco no meio. Banco só leitura → 403.
    pub fn esquema(&self, banco_id: &str, operacao: &OperacaoEsquema, previa: bool) -> Resultado<RespostaEsquema> {
        let banco = self.banco_aberto(banco_id)?;
        if banco.somente_leitura() {
            return Err(ErroBanco::com_status(MSG_ESQUEMA_LEITURA, 403));
        }
        let plano = banco.com_conexao(|conn| {
            let plano = montar_plano(conn, operacao)?;
            if !previa {
                esquema::executar(conn, &plano)?;
            }
            Ok(plano)
        })?;
        let transacao_aberta = !previa && banco.transacao_aberta();
        Ok(RespostaEsquema {
            previa,
            sql: plano.sql(),
            descricao: plano.descricao,
            comandos: plano.comandos,
            reconstroi: plano.reconstroi,
            avisos: plano.avisos,
            transacao_aberta,
        })
    }

    // ---- consulta livre (§4.5) ---------------------------------------------------------------------

    /// `POST /api/sql`: roda o roteiro comando a comando (para no primeiro erro). Com ao menos um comando, o texto
    /// entra no histórico do banco com o que deu, inclusive quando falhou ou foi interrompido (a consulta que
    /// falhou é a que a pessoa mais quer recuperar).
    pub fn sql(&self, banco_id: &str, sql: &str, limite: usize) -> Resultado<ResultadoConsulta> {
        let banco = self.banco_aberto(banco_id)?;
        let resultado = banco.consultar(sql, limite)?;
        if let Some(ultimo) = resultado.resultados.last() {
            let comandos = u32::try_from(resultado.resultados.len()).unwrap_or(u32::MAX);
            self.prefs.registrar_sql(
                banco.caminho(),
                sql,
                Some(resultado_do_historico(ultimo, comandos)),
                Some(comandos),
            );
        }
        Ok(resultado)
    }

    /// O Interromper da Consulta SQL (só no DataBox): corta o `sql()` em andamento no banco aberto, ou, se nenhum
    /// tem a conexão e um espera a vez dela, marca esse para sair interrompido sem executar. Ok(true) se alcançou
    /// um; Ok(false) se nenhum havia (já terminou). Só passa pelo `banco_aberto` (a trava pequena do banco aberto,
    /// nunca a da conexão): responde na hora mesmo com a consulta segurando o banco. Banco trocado → 409
    /// `Codigo::BancoTrocado`, como os demais.
    pub fn interromper_sql(&self, banco_id: &str) -> Resultado<bool> {
        Ok(self.interromper_sql_detalhado(banco_id)? != Interrupcao::Nenhuma)
    }

    /// Um `sql()` já pegou a conexão do banco aberto e está rodando o roteiro (não conta o que ainda espera a vez).
    /// Só passa pelo `banco_aberto`: responde na hora. Serve para os testes saberem que o Interromper vai alcançar
    /// um comando de verdade, e não a espera.
    pub fn sql_em_andamento(&self, banco_id: &str) -> Resultado<bool> {
        Ok(self.banco_aberto(banco_id)?.consulta_em_andamento())
    }

    /// Interrompe a Consulta SQL do banco aberto, seja qual for (o fechar do aplicativo, que não tem o id de banco
    /// nenhum à mão). true se havia uma, em andamento ou esperando a vez; sem banco aberto, false.
    pub fn interromper_sql_do_aberto(&self) -> bool {
        self.atual().is_some_and(|banco| banco.interromper_consulta())
    }

    /// O [`Servico::interromper_sql`] dizendo o que alcançou: a consulta em andamento, uma que ainda espera a vez
    /// (a tela avisa que ela só sai quando o banco ficar livre) ou nenhuma.
    pub fn interromper_sql_detalhado(&self, banco_id: &str) -> Resultado<Interrupcao> {
        Ok(self.banco_aberto(banco_id)?.interromper_consulta_detalhado())
    }

    /// `GET /api/sql/historico`: os comandos já executados nesse banco, o mais recente primeiro.
    pub fn historico(&self, banco_id: &str) -> Resultado<Vec<ComandoSql>> {
        let banco = self.banco_aberto(banco_id)?;
        Ok(self.prefs.historico_sql(banco.caminho()))
    }
}

/// A pílula do Histórico: o que o último comando do roteiro deu (`numero`: a posição dele, do 1; o roteiro para no
/// primeiro erro, que é sempre o último).
fn resultado_do_historico(ultimo: &ResultadoComando, numero: u32) -> ResultadoSql {
    match ultimo {
        ResultadoComando::Linhas { linhas, truncado, .. } => {
            ResultadoSql::Linhas { quantidade: linhas.len() as u64, mais: *truncado }
        }
        ResultadoComando::Alteracao { linhas_afetadas: Some(n), .. } => ResultadoSql::Alteradas(*n),
        ResultadoComando::Alteracao { linhas_afetadas: None, .. } => ResultadoSql::Executado,
        ResultadoComando::Erro { mensagem, .. } if mensagem == banco::MSG_INTERROMPIDA => {
            ResultadoSql::Interrompida { numero }
        }
        ResultadoComando::Erro { mensagem, .. } => ResultadoSql::erro(numero, mensagem),
    }
}

/// `esquema::definicoes_tabela`, que os testes fazem falhar.
fn ler_definicoes(conn: &rusqlite::Connection, nome: &str) -> Resultado<esquema::Definicoes> {
    #[cfg(test)]
    if let Some(erro) = crate::testes::falha_nas_definicoes() {
        return Err(erro);
    }
    esquema::definicoes_tabela(conn, nome)
}
