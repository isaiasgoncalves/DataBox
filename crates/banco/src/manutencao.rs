//! Manutenção do arquivo (integridade, chaves estrangeiras, VACUUM, otimizar, ANALYZE) e cópia de segurança.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use comum::{ErroBanco, Resultado, traduzir_io};
use rusqlite::Connection;
use rusqlite::backup::{Backup, StepResult};

use crate::banco::{Banco, ESPERA_APOS_TRAVA_MS, PAGINAS_POR_PASSO_COPIA, ms};
use crate::caminhos::{resolver, tamanho_arquivo};
use crate::erros::{Contexto, MSG_SOMENTE_LEITURA};
use crate::valores::{texto_de, texto_opcional};

/// Ação de manutenção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcaoManutencao {
    Integridade,
    Chaves,
    Vacuum,
    Otimizar,
    Analisar,
}

impl AcaoManutencao {
    pub const TODAS: [AcaoManutencao; 5] = [
        AcaoManutencao::Integridade,
        AcaoManutencao::Chaves,
        AcaoManutencao::Vacuum,
        AcaoManutencao::Otimizar,
        AcaoManutencao::Analisar,
    ];

    /// "integridade" | "chaves" | "vacuum" | "otimizar" | "analisar".
    pub fn como_str(self) -> &'static str {
        match self {
            AcaoManutencao::Integridade => "integridade",
            AcaoManutencao::Chaves => "chaves",
            AcaoManutencao::Vacuum => "vacuum",
            AcaoManutencao::Otimizar => "otimizar",
            AcaoManutencao::Analisar => "analisar",
        }
    }

    /// Pelo texto; outro → 400 "Ação de manutenção desconhecida: x.".
    pub fn de_texto(texto: &str) -> Resultado<AcaoManutencao> {
        AcaoManutencao::TODAS
            .into_iter()
            .find(|a| a.como_str() == texto)
            .ok_or_else(|| ErroBanco::novo(format!("Ação de manutenção desconhecida: {texto}.")))
    }

    fn grava(self) -> bool {
        matches!(self, AcaoManutencao::Vacuum | AcaoManutencao::Otimizar | AcaoManutencao::Analisar)
    }
}

/// `{"acao", "ok", "mensagens", "tamanho_antes", "tamanho_depois"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoManutencao {
    pub acao: AcaoManutencao,
    pub ok: bool,
    pub mensagens: Vec<String>,
    pub tamanho_antes: u64,
    pub tamanho_depois: u64,
}

/// `{"caminho", "tamanho"}` da cópia de segurança.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiaFeita {
    pub caminho: PathBuf,
    pub tamanho: u64,
}

fn coluna_de_texto(conn: &Connection, sql: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let mut r = stmt.query([])?;
    let mut saida = Vec::new();
    while let Some(l) = r.next()? {
        saida.push(texto_de(l.get_ref(0)?));
    }
    Ok(saida)
}

/// Erro de cópia por arquivo travado por outro programa, depois de `ESPERA_COPIA` seguidos.
fn copia_travada() -> ErroBanco {
    ErroBanco::com_status(
        "Não foi possível fazer a cópia de segurança. O banco está em uso por outro programa. Feche-o lá e tente \
         de novo.",
        409,
    )
    .detalhe("database is locked")
}

/// Cópia em passos: 1024 páginas por passo e 250 ms de pausa quando o
/// arquivo está travado, desistindo depois de `espera` seguidos assim.
fn copiar(origem: &Connection, destino: &mut Connection, espera: Duration) -> Resultado<()> {
    let contexto = "Não foi possível fazer a cópia de segurança.";
    let copia = Backup::new(origem, destino).ctx(contexto)?;
    let mut travado_desde: Option<Instant> = None;
    loop {
        match copia.step(PAGINAS_POR_PASSO_COPIA).ctx(contexto)? {
            StepResult::Done => return Ok(()),
            StepResult::Busy | StepResult::Locked => {
                match travado_desde {
                    None => travado_desde = Some(Instant::now()),
                    Some(desde) if desde.elapsed() > espera => return Err(copia_travada()),
                    Some(_) => {}
                }
                thread::sleep(Duration::from_millis(250));
            }
            _ => travado_desde = None,
        }
    }
}

impl Banco {
    /// Verificações e otimizações do arquivo. VACUUM, otimizar e analisar num banco só leitura → 403; VACUUM
    /// com transação aberta → 409.
    pub fn manutencao(&self, acao: AcaoManutencao) -> Resultado<ResultadoManutencao> {
        if acao.grava() && self.somente_leitura {
            return Err(ErroBanco::com_status(MSG_SOMENTE_LEITURA, 403));
        }
        let tamanho_antes = tamanho_arquivo(&self.caminho);
        let contexto = "Não foi possível concluir a manutenção.";
        let (ok, mensagens) = self.com_conexao(|conn| match acao {
            AcaoManutencao::Integridade => {
                let resultado = coluna_de_texto(conn, "PRAGMA main.integrity_check").ctx(contexto)?;
                let ok = resultado == ["ok"];
                Ok((ok, if ok { vec!["O banco passou na verificação de integridade.".to_string()] } else { resultado }))
            }
            AcaoManutencao::Chaves => {
                let quebras = (|| {
                    let mut stmt = conn.prepare("PRAGMA main.foreign_key_check")?;
                    let mut r = stmt.query([])?;
                    let mut saida = Vec::new();
                    while let Some(l) = r.next()? {
                        let tabela = texto_de(l.get_ref(0)?);
                        let rowid = texto_opcional(l.get_ref(1)?).unwrap_or_else(|| "(sem rowid)".to_string());
                        let pai = texto_de(l.get_ref(2)?);
                        saida.push(format!("{tabela}: a linha {rowid} aponta para {pai}, onde o valor não existe."));
                    }
                    Ok(saida)
                })()
                .ctx(contexto)?;
                let ok = quebras.is_empty();
                Ok((ok, if ok { vec!["Nenhuma chave estrangeira quebrada.".to_string()] } else { quebras }))
            }
            AcaoManutencao::Vacuum => {
                if !conn.is_autocommit() {
                    return Err(ErroBanco::com_status(
                        "Há uma transação aberta na consulta SQL; confirme (COMMIT) ou desfaça (ROLLBACK) antes de \
                         compactar.",
                        409,
                    ));
                }
                conn.execute_batch("VACUUM").ctx(contexto)?;
                Ok((true, vec!["O banco foi compactado (VACUUM).".to_string()]))
            }
            AcaoManutencao::Otimizar => {
                conn.execute_batch("PRAGMA main.optimize").ctx(contexto)?;
                Ok((true, vec!["O SQLite revisou as estatísticas usadas nas consultas (PRAGMA optimize).".to_string()]))
            }
            AcaoManutencao::Analisar => {
                conn.execute_batch("ANALYZE main").ctx(contexto)?;
                Ok((true, vec!["As estatísticas dos índices foram recalculadas (ANALYZE).".to_string()]))
            }
        })?;
        Ok(ResultadoManutencao { acao, ok, mensagens, tamanho_antes, tamanho_depois: tamanho_arquivo(&self.caminho) })
    }

    /// Cópia consistente do banco (API de backup do SQLite) num arquivo novo, gravada num temporário ao lado e
    /// trocada no fim. Destino = o próprio banco ou -journal/-wal/-shm → 400; pasta → 400; pasta que não
    /// existe → 404; transação aberta → 409 `TransacaoAberta` (a cópia repetiria para sempre segurando a
    /// trava, e sairia sem as mudanças); arquivo travado por outro programa → desiste depois de 5 s seguidos
    /// (409).
    pub fn copia_de_seguranca(&self, destino: &Path) -> Resultado<CopiaFeita> {
        let contexto = "Não foi possível fazer a cópia de segurança.";
        let alvo = resolver(destino).map_err(|e| traduzir_io(&e, Some(destino), Some(contexto)))?;
        if self.e_arquivo_do_banco(&alvo) {
            return Err(ErroBanco::novo(
                "A cópia de segurança precisa ir para outro arquivo, não para o próprio banco (nem para os arquivos \
                 -journal, -wal e -shm que o SQLite cria ao lado dele).",
            ));
        }
        if alvo.is_dir() {
            return Err(ErroBanco::novo(format!(
                "O destino é uma pasta; informe o nome do arquivo: {}.",
                alvo.display()
            )));
        }
        let pasta = alvo.parent().map(Path::to_path_buf).unwrap_or_default();
        if !pasta.is_dir() {
            return Err(ErroBanco::com_status(format!("A pasta de destino não existe: {}.", pasta.display()), 404));
        }
        self.exigir_sem_transacao("fazer a cópia de segurança")?;
        let nome = alvo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let temporario = pasta.join(format!(".{nome}.{}.tmp", std::process::id()));
        let espera = ms(self.ajustes.espera_copia_ms.load(Ordering::Relaxed));
        let feito = self
            .com_conexao(|conn| {
                self.exigir_sem_transacao("fazer a cópia de segurança")?;
                let mut destino_conn = Connection::open(&temporario).ctx(contexto)?;
                // cada passo espera pouco pela trava; quem conta o tempo total é `copiar`
                let _ = conn.busy_timeout(ms(ESPERA_APOS_TRAVA_MS));
                let r = copiar(conn, &mut destino_conn, espera);
                drop(destino_conn);
                let _ = conn.busy_timeout(self.espera_gravacao());
                r
            })
            .and_then(|()| fs::rename(&temporario, &alvo).map_err(|e| traduzir_io(&e, Some(&alvo), Some(contexto))));
        if let Err(e) = feito {
            let _ = fs::remove_file(&temporario);
            return Err(e);
        }
        Ok(CopiaFeita { tamanho: tamanho_arquivo(&alvo), caminho: alvo })
    }
}
