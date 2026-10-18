//! O [`Banco`]: um arquivo SQLite aberto, com a conexão principal atrás de uma trava reentrante.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use comum::{Codigo, ErroBanco, Resultado, traduzir_io};
use parking_lot::{Mutex, ReentrantMutex, ReentrantMutexGuard};
use rusqlite::{Connection, InterruptHandle, OpenFlags};

use crate::caminhos::{com_sufixo, formatar_data, mesmo_arquivo, normcase, resolver, tamanho_arquivo, uri_de};
use crate::erros::{Contexto, MSG_JOURNAL_QUENTE, MSG_PROTEGIDO, e_trava, erro_fechado, msg_transacao, traduzir};
use crate::leitura::Leitura;
use crate::meta::{Meta, classificar, construir_meta};
use crate::tipos::{BancoInfo, ContagemObjetos};
use crate::valores::{inteiro_de, texto_de};

/// Caracteres de texto na grade antes de cortar.
pub use comum::LIMITE_TEXTO;
/// Acima disso (bytes do arquivo), objetos() e tabela_info() não contam linhas (`linhas` None).
pub const LIMITE_CONTAGEM_BYTES: u64 = 256 * 1024 * 1024;
/// Até esse tamanho de arquivo, ordenações e tabelas temporárias da conexão ficam em memória.
pub const LIMITE_TEMP_MEMORIA: u64 = 1024 * 1024 * 1024;
/// Espera por um arquivo travado por outro programa (busy_timeout, em ms): gravação espera mais; leitura
/// desiste logo, para a tela não ficar pendurada; logo depois de uma trava, a espera cai a quase nada, para
/// os pedidos que estavam na fila responderem na hora em vez de esperar cada um a sua vez.
pub const ESPERA_GRAVACAO_MS: u64 = 5000;
pub const ESPERA_LEITURA_MS: u64 = 1500;
pub const ESPERA_APOS_TRAVA_MS: u64 = 100;
pub(crate) const JANELA_APOS_TRAVA: Duration = Duration::from_secs(3);
/// Quanto info() espera a trava antes de responder com o último BancoInfo que leu.
pub(crate) const ESPERA_INFO: Duration = Duration::from_millis(500);
/// Cópia de segurança: quanto tempo seguido ela aceita ficar esperando o arquivo travado por outro programa.
pub const ESPERA_COPIA_MS: u64 = 5000;
pub(crate) const PAGINAS_POR_PASSO_COPIA: i32 = 1024;
/// Instruções da máquina virtual do SQLite entre duas consultas a quem pode mandar parar uma leitura
/// (progress handler): perto de um milissegundo de trabalho, sem pesar na leitura.
pub(crate) const PASSOS_ENTRE_CONFERENCIAS: i32 = 20_000;
pub(crate) const LIMITE_PARAMETROS: usize = 30_000;
pub(crate) const LOTE_LEITURA: usize = 500;
/// Com até 1000 linhas por página, o OFFSET cabe folgado nos 64 bits do SQLite.
pub(crate) const MAXIMO_PAGINA: u64 = 1 << 31;

pub(crate) fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Os números que os testes trocam (por banco aberto).
pub(crate) struct Ajustes {
    pub(crate) espera_gravacao_ms: AtomicU64,
    pub(crate) espera_copia_ms: AtomicU64,
    pub(crate) limite_contagem_bytes: AtomicU64,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            espera_gravacao_ms: AtomicU64::new(ESPERA_GRAVACAO_MS),
            espera_copia_ms: AtomicU64::new(ESPERA_COPIA_MS),
            limite_contagem_bytes: AtomicU64::new(LIMITE_CONTAGEM_BYTES),
        }
    }
}

/// As leituras substituíveis (ver `leitura.rs`): a mais nova de cada grupo e a que está executando
/// agora na conexão principal (a única que um interrupt pode alcançar).
#[derive(Default)]
pub(crate) struct EstadoLeituras {
    pub(crate) em_curso: Option<Arc<Leitura>>,
    pub(crate) grupos: HashMap<String, Arc<Leitura>>,
}

/// Um arquivo SQLite aberto para leitura e edição.
///
/// Uma conexão só, em autocommit (cada gravação vai para o arquivo na hora), atrás de uma trava reentrante:
/// várias threads usam o mesmo `Banco` por `Arc`, uma de cada vez na conexão.
/// A exportação lê por conexões novas, só de leitura, para não segurar a interface.
pub struct Banco {
    pub(crate) caminho: PathBuf,
    pub(crate) nome: String,
    pub(crate) id: String,
    pub(crate) somente_leitura: bool,
    pub(crate) motivo_somente_leitura: Option<String>,
    pub(crate) conn: ReentrantMutex<RefCell<Option<Connection>>>,
    /// Interrompe o comando em andamento na conexão principal (só pelo protocolo da leitura substituída e pelo
    /// `interromper_consulta`, cada um sob a sua trava pequena).
    pub(crate) interruptor: InterruptHandle,
    /// A marca da Consulta SQL em andamento: presente só enquanto o `consultar` tem a conexão (posta e tirada
    /// dentro do `com_conexao`); o `interromper_consulta` liga a marca e chama o interrupt segurando esta trava.
    pub(crate) parada_consulta: Mutex<Option<Arc<AtomicBool>>>,
    /// As marcas das Consultas SQL que ainda esperam a vez na conexão (a mais antiga primeiro), postas antes de o
    /// `consultar` pedir a conexão e tiradas quando ele a pega (a marca passa para a `parada_consulta`) ou desiste.
    /// Sempre travada DEPOIS da `parada_consulta`, nunca antes.
    pub(crate) consultas_na_fila: Mutex<Vec<Arc<AtomicBool>>>,
    pub(crate) travado_em: Mutex<Option<Instant>>,
    pub(crate) ultima_info: Mutex<Option<BancoInfo>>,
    /// Exportações lendo por conexões próprias agora (uma gravação travada por elas diz isso).
    pub(crate) exportacoes: AtomicUsize,
    /// Trava pequena da leitura substituída: protege a marca `em_curso`.
    pub(crate) leituras: Mutex<EstadoLeituras>,
    pub(crate) ajustes: Ajustes,
    /// Só nos testes: troca a descrição da tabela .
    #[cfg(test)]
    pub(crate) ajuste_meta: Mutex<Option<fn(&mut Meta)>>,
}

fn novo_id() -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let mut h = RandomState::new().build_hasher();
    h.write_u64(CONTADOR.fetch_add(1, Ordering::Relaxed));
    h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default());
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

/// Conexão nova pela URI (`mode=ro` ou `mode=rw`), com a espera de gravação.
pub(crate) fn conectar(caminho: &Path, modo: &str) -> Resultado<Connection> {
    let base = if modo == "ro" { OpenFlags::SQLITE_OPEN_READ_ONLY } else { OpenFlags::SQLITE_OPEN_READ_WRITE };
    let flags = base | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(uri_de(caminho, modo), flags).ctx("Não foi possível abrir o banco.")?;
    conn.busy_timeout(ms(ESPERA_GRAVACAO_MS)).ctx("Não foi possível abrir o banco.")?;
    Ok(conn)
}

/// Dá para gravar no arquivo? O SQLite abre em modo rw um arquivo protegido e cai para leitura sem avisar.
fn da_para_gravar(conn: &Connection, caminho: &Path) -> bool {
    let gravavel = fs::metadata(caminho).map(|m| !m.permissions().readonly()).unwrap_or(false);
    if !gravavel {
        return false;
    }
    // travado por outro programa: não espera, grava depois
    let _ = conn.busy_timeout(Duration::ZERO);
    // pega a trava de gravação sem gravar nada
    let r = conn.execute_batch("BEGIN IMMEDIATE").and_then(|()| conn.execute_batch("ROLLBACK"));
    let _ = conn.busy_timeout(ms(ESPERA_GRAVACAO_MS));
    match r {
        Ok(()) => true,
        Err(e) => {
            let texto = e.to_string().to_lowercase();
            !(texto.contains("readonly") || texto.contains("read-only"))
        }
    }
}

impl Banco {
    /// Abre o arquivo (caminho resolvido para absoluto). Sem `criar`, arquivo inexistente → 404; com `criar`,
    /// cria um arquivo vazio (0 bytes já é um banco válido). Pasta → 400; arquivo que não é banco → 400.
    /// Arquivo protegido contra gravação aberto para edição fica só leitura, com o motivo em
    /// [`Banco::motivo_somente_leitura`]. Só leitura com um -journal quente ao lado → 409 explicando.
    pub fn abrir(caminho: &Path, somente_leitura: bool, criar: bool) -> Resultado<Banco> {
        let p =
            resolver(caminho).map_err(|e| traduzir_io(&e, Some(caminho), Some("Não foi possível abrir o banco.")))?;
        let nome = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if p.is_dir() {
            return Err(ErroBanco::com_status(
                format!("O caminho é uma pasta, não um arquivo de banco: {}.", p.display()),
                400,
            ));
        }
        if !p.exists() {
            if !criar {
                return Err(ErroBanco::com_status(format!("O arquivo do banco não existe: {}.", p.display()), 404));
            }
            // arquivo de 0 bytes já é um banco SQLite válido
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&p)
                .map_err(|e| traduzir_io(&e, Some(&p), Some("Não foi possível criar o banco.")))?;
        }

        let conn = conectar(&p, if somente_leitura { "ro" } else { "rw" })?;
        let preparo = (|| {
            conn.query_row("SELECT count(*) FROM main.sqlite_schema", [], |_| Ok(()))?;
            conn.execute_batch("PRAGMA foreign_keys = ON")?;
            if tamanho_arquivo(&p) <= LIMITE_TEMP_MEMORIA {
                // Ordenar a grade de uma tabela grande (subconsulta estreita) com o sorter em arquivos
                // temporários levava 17 s; em memória, 2 s. Só até um tamanho de banco que cabe folgado.
                // (Tem de ser antes de qualquer tabela temporária: mudar temp_store apaga as que existem.)
                conn.execute_batch("PRAGMA temp_store = MEMORY")?;
            }
            Ok(())
        })();
        if let Err(e) = preparo {
            drop(conn);
            let erro = traduzir(e, Some("Não foi possível abrir o banco."));
            if somente_leitura && erro.status == 403 && com_sufixo(&p, "-journal").exists() {
                let mut quente = ErroBanco::com_status(
                    format!("Não foi possível abrir o banco só para leitura. {MSG_JOURNAL_QUENTE}"),
                    409,
                );
                quente.detalhe = erro.detalhe;
                return Err(quente);
            }
            return Err(erro);
        }

        let mut motivo = None;
        let mut so_leitura = somente_leitura;
        if !so_leitura && !da_para_gravar(&conn, &p) {
            so_leitura = true;
            motivo = Some(MSG_PROTEGIDO.to_string());
        }
        let interruptor = conn.get_interrupt_handle();
        Ok(Banco {
            caminho: p,
            nome,
            id: novo_id(),
            somente_leitura: so_leitura,
            motivo_somente_leitura: motivo,
            conn: ReentrantMutex::new(RefCell::new(Some(conn))),
            interruptor,
            parada_consulta: Mutex::new(None),
            consultas_na_fila: Mutex::new(Vec::new()),
            travado_em: Mutex::new(None),
            ultima_info: Mutex::new(None),
            exportacoes: AtomicUsize::new(0),
            leituras: Mutex::new(EstadoLeituras::default()),
            ajustes: Ajustes::default(),
            #[cfg(test)]
            ajuste_meta: Mutex::new(None),
        })
    }

    /// Caminho absoluto do arquivo.
    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Nome do arquivo ("vendas.db").
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// Identidade desta abertura (16 caracteres hexadecimais, aleatória).
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn somente_leitura(&self) -> bool {
        self.somente_leitura
    }

    /// Por que ficou só leitura sem a pessoa pedir (arquivo protegido); None quando foi pedido ou dá para gravar.
    pub fn motivo_somente_leitura(&self) -> Option<&str> {
        self.motivo_somente_leitura.as_deref()
    }

    pub(crate) fn espera_gravacao(&self) -> Duration {
        ms(self.ajustes.espera_gravacao_ms.load(Ordering::Relaxed))
    }

    pub(crate) fn pode_contar(&self) -> bool {
        tamanho_arquivo(&self.caminho) <= self.ajustes.limite_contagem_bytes.load(Ordering::Relaxed)
    }

    /// Fecha a conexão. Uma transação ainda aberta é desfeita pelo SQLite (quem chama confere antes) e fica
    /// um aviso no registro. Fechar de novo não é erro; usar depois de fechar → 409.
    pub fn fechar(&self) {
        let guarda = self.conn.lock();
        let Ok(mut celula) = guarda.try_borrow_mut() else {
            log::warn!("O banco {} não foi fechado: a conexão ainda está em uso nesta thread.", self.caminho.display());
            return;
        };
        if let Some(conn) = celula.take() {
            if !conn.is_autocommit() {
                log::warn!(
                    "O banco {} foi fechado com uma transação aberta; o que mudou desde o BEGIN foi descartado.",
                    self.caminho.display()
                );
            }
            if let Err((_conn, e)) = conn.close() {
                log::warn!("Erro ao fechar o banco {}: {e}", self.caminho.display());
            }
        }
    }

    /// Entrega a conexão principal segurando a trava (para o crate esquema e o serviço). A
    /// trava é reentrante: dentro de `f` dá para chamar os outros métodos do banco.
    pub fn com_conexao<R>(&self, f: impl FnOnce(&Connection) -> Resultado<R>) -> Resultado<R> {
        let guarda = self.conn.lock();
        let r = match guarda.try_borrow() {
            Ok(celula) => match celula.as_ref() {
                Some(conn) => f(conn),
                None => Err(erro_fechado()),
            },
            Err(_) => Err(erro_fechado()),
        };
        // Solta em ordem de chegada: uma gravação da pessoa não fica esperando atrás de uma rajada de leituras
        // que pegariam a trava de volta antes de ela acordar.
        ReentrantMutexGuard::unlock_fair(guarda);
        r
    }

    /// True quando a Consulta SQL deixou um BEGIN aberto (ou um SAVEPOINT ficou pendurado).
    pub fn transacao_aberta(&self) -> bool {
        self.com_conexao(|conn| Ok(!conn.is_autocommit())).unwrap_or(false)
    }

    /// ErroBanco 409 (codigo `TransacaoAberta`) se houver transação aberta na conexão principal.
    pub fn exigir_sem_transacao(&self, acao: &str) -> Resultado<()> {
        if self.transacao_aberta() {
            return Err(ErroBanco::com_status(msg_transacao(acao), 409).codigo(Codigo::TransacaoAberta));
        }
        Ok(())
    }

    /// True se o caminho é o próprio arquivo do banco ou um dos que o SQLite cria ao lado dele
    /// (-journal, -wal, -shm). Gravar um arquivo exportado como `<banco>-journal` seria desastroso: o SQLite o
    /// trataria como uma transação interrompida e "desfaria" o banco com ele.
    pub fn e_arquivo_do_banco(&self, caminho: &Path) -> bool {
        let Ok(alvo) = resolver(caminho) else {
            return false;
        };
        if mesmo_arquivo(&alvo, &self.caminho) {
            return true;
        }
        let nome = normcase(&alvo);
        ["-journal", "-wal", "-shm"].iter().any(|s| nome == normcase(&com_sufixo(&self.caminho, s)))
    }

    /// Conexão NOVA, só de leitura, para exportar sem travar a interface. Quem abre fecha (drop).
    pub fn conexao_leitura(&self) -> Resultado<Connection> {
        let conn = conectar(&self.caminho, "ro")?;
        conn.query_row("SELECT count(*) FROM main.sqlite_schema", [], |_| Ok(()))
            .ctx("Não foi possível abrir o banco para leitura.")?;
        Ok(conn)
    }

    /// Descrição da tabela ou visão.
    pub(crate) fn meta(&self, conn: &Connection, nome: &str) -> Resultado<Meta> {
        #[allow(unused_mut)]
        let mut meta = construir_meta(conn, nome, self.somente_leitura)?;
        #[cfg(test)]
        if let Some(ajustar) = *self.ajuste_meta.lock() {
            ajustar(&mut meta);
        }
        Ok(meta)
    }

    /// Conexão principal para ler, segurando a trava, com espera curta por um arquivo travado.
    ///
    /// Um pedido de leitura roda vários comandos, e todos os pedidos fazem fila na mesma trava: com a espera
    /// de gravação (5 s) em cada comando, a tela ficaria quase um minuto pendurada antes de avisar que o
    /// banco está em uso. Depois de uma trava, os pedidos da fila desistem quase na hora.
    pub(crate) fn lendo<R>(&self, f: impl FnOnce(&Connection) -> Resultado<R>) -> Resultado<R> {
        self.com_conexao(|conn| {
            let recente = self.travado_em.lock().is_some_and(|t| t.elapsed() < JANELA_APOS_TRAVA);
            let _ = conn.busy_timeout(ms(if recente { ESPERA_APOS_TRAVA_MS } else { ESPERA_LEITURA_MS }));
            let r = f(conn);
            if let Err(e) = &r
                && e_trava(e)
            {
                *self.travado_em.lock() = Some(Instant::now());
            }
            let _ = conn.busy_timeout(self.espera_gravacao());
            r
        })
    }

    /// BancoInfo (§4.2).
    ///
    /// Com a trava presa por uma leitura longa (busca numa tabela enorme), devolve o último BancoInfo depois
    /// de 0,5 s em vez de deixar a interface inteira esperando junto.
    pub fn info(&self) -> Resultado<BancoInfo> {
        let guarda = match self.conn.try_lock_for(ESPERA_INFO) {
            Some(g) => g,
            None => {
                if let Some(ultima) = self.ultima_info.lock().clone() {
                    return Ok(ultima);
                }
                self.conn.lock()
            }
        };
        let dados = self.info_interno();
        drop(guarda);
        let dados = dados?;
        *self.ultima_info.lock() = Some(dados.clone());
        Ok(dados)
    }

    fn info_interno(&self) -> Resultado<BancoInfo> {
        struct Lido {
            transacao: bool,
            codificacao: String,
            tamanho_pagina: i64,
            paginas: i64,
            paginas_livres: i64,
            modo_diario: String,
            versao_usuario: i64,
            chaves_estrangeiras: bool,
            contagem: ContagemObjetos,
        }
        let lido = self.lendo(|conn| {
            let transacao = !conn.is_autocommit();
            (|| {
                let pragma =
                    |nome: &str| conn.query_row(&format!("PRAGMA main.{nome}"), [], |r| r.get_ref(0).map(texto_de));
                let pragma_int =
                    |nome: &str| conn.query_row(&format!("PRAGMA main.{nome}"), [], |r| r.get_ref(0).map(inteiro_de));
                let cls = classificar(conn)?;
                let contar = |sql: &str| conn.query_row(sql, [], |r| r.get_ref(0).map(inteiro_de));
                let indices = contar("SELECT count(*) FROM main.sqlite_schema WHERE type = 'index'")?;
                let gatilhos = contar("SELECT count(*) FROM main.sqlite_schema WHERE type = 'trigger'")?;
                Ok(Lido {
                    transacao,
                    codificacao: pragma("encoding")?,
                    tamanho_pagina: pragma_int("page_size")?,
                    paginas: pragma_int("page_count")?,
                    paginas_livres: pragma_int("freelist_count")?,
                    modo_diario: pragma("journal_mode")?.to_lowercase(),
                    versao_usuario: pragma_int("user_version")?,
                    chaves_estrangeiras: conn.query_row("PRAGMA foreign_keys", [], |r| r.get_ref(0).map(inteiro_de))?
                        != 0,
                    contagem: ContagemObjetos {
                        tabelas: cls.tabelas.len() as u64,
                        visoes: cls.visoes.len() as u64,
                        indices: u64::try_from(indices).unwrap_or(0),
                        gatilhos: u64::try_from(gatilhos).unwrap_or(0),
                    },
                })
            })()
            .ctx("Não foi possível ler as informações do banco.")
        })?;
        let (tamanho, modificado_em) = match fs::metadata(&self.caminho) {
            Ok(m) => (m.len(), m.modified().ok().map(formatar_data)),
            Err(_) => (0, None),
        };
        Ok(BancoInfo {
            id: self.id.clone(),
            caminho: self.caminho.clone(),
            nome: self.nome.clone(),
            pasta: self.caminho.parent().map(Path::to_path_buf).unwrap_or_default(),
            tamanho,
            modificado_em,
            somente_leitura: self.somente_leitura,
            motivo_somente_leitura: self.motivo_somente_leitura.clone(),
            transacao_aberta: lido.transacao,
            sqlite_versao: rusqlite::version().to_string(),
            codificacao: lido.codificacao,
            tamanho_pagina: lido.tamanho_pagina,
            paginas: lido.paginas,
            paginas_livres: lido.paginas_livres,
            modo_diario: lido.modo_diario,
            versao_usuario: lido.versao_usuario,
            chaves_estrangeiras: lido.chaves_estrangeiras,
            contagem: lido.contagem,
        })
    }
}

impl Drop for Banco {
    fn drop(&mut self) {
        self.fechar();
    }
}

impl std::fmt::Debug for Banco {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Banco")
            .field("caminho", &self.caminho)
            .field("id", &self.id)
            .field("somente_leitura", &self.somente_leitura)
            .finish_non_exhaustive()
    }
}
