//! Leitura substituída (cada grupo de leitura só tem uma viva: a mais nova cancela a anterior).
//!
//! Cada grupo (`"grade:<tabela>"`, `"contagem:<tabela>"`) guarda a leitura mais nova. Ao chegar outra, a anterior
//! é abandonada: se ainda espera a trava do banco, desiste ao pegá-la; se já executa, é interrompida no
//! SQLite. Protocolo sem corrida: a trava pequena `leituras` protege a marca `em_curso`, posta pela thread da
//! leitura (já com a trava do banco) logo antes de executar e tirada logo depois, antes de soltar a trava do
//! banco; o interrupt só é chamado segurando a trava pequena e com a marca da leitura abandonada presente.
//! Assim ele nunca alcança uma gravação, a Estrutura, a Consulta SQL ou uma exportação. Um progress handler
//! da própria leitura cobre o instante entre pôr a marca e o SQLite começar o comando (em que o SQLite
//! zeraria o pedido de interrupção).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use comum::{ErroBanco, Resultado};
use rusqlite::Connection;

use crate::banco::{Banco, PASSOS_ENTRE_CONFERENCIAS};
use crate::erros::{erro_substituida, traduzir};

/// Um pedido de leitura de um grupo: fica abandonado quando chega outro mais novo.
#[derive(Debug)]
pub(crate) struct Leitura {
    pub(crate) grupo: String,
    abandonada: AtomicBool,
}

impl Leitura {
    pub(crate) fn abandonada(&self) -> bool {
        self.abandonada.load(Ordering::SeqCst)
    }
}

/// Tira a marca e o progress handler mesmo se a leitura sair no meio (erro ou pânico).
struct Marca<'a> {
    banco: &'a Banco,
    conn: &'a Connection,
}

impl Drop for Marca<'_> {
    fn drop(&mut self) {
        let _ = self.conn.progress_handler(0, None::<fn() -> bool>);
        self.banco.leituras.lock().em_curso = None;
    }
}

impl Banco {
    /// Anota a leitura como a mais nova do grupo e abandona a anterior dele.
    ///
    /// A anterior que já executa no SQLite é interrompida agora; a que ainda espera a trava desiste ao pegá-la.
    pub(crate) fn registrar_leitura(&self, grupo: Option<&str>) -> Option<Arc<Leitura>> {
        let grupo = grupo.filter(|g| !g.is_empty())?;
        let nova = Arc::new(Leitura { grupo: grupo.to_string(), abandonada: AtomicBool::new(false) });
        let mut estado = self.leituras.lock();
        if let Some(anterior) = estado.grupos.insert(grupo.to_string(), Arc::clone(&nova)) {
            anterior.abandonada.store(true, Ordering::SeqCst);
            if estado.em_curso.as_ref().is_some_and(|e| Arc::ptr_eq(e, &anterior)) {
                self.interruptor.interrupt();
            }
        }
        Some(nova)
    }

    pub(crate) fn soltar_leitura(&self, leitura: Option<&Arc<Leitura>>) {
        let Some(leitura) = leitura else {
            return;
        };
        let mut estado = self.leituras.lock();
        if estado.grupos.get(&leitura.grupo).is_some_and(|g| Arc::ptr_eq(g, leitura)) {
            estado.grupos.remove(&leitura.grupo);
        }
    }

    /// Como `lendo`; com `grupo`, outra leitura mais nova do mesmo grupo abandona esta (409 `Substituido`).
    pub(crate) fn lendo_substituivel<R>(
        &self,
        grupo: Option<&str>,
        f: impl FnOnce(&Connection) -> Resultado<R>,
    ) -> Resultado<R> {
        let leitura = self.registrar_leitura(grupo);
        let r = self.lendo(|conn| match &leitura {
            None => f(conn),
            Some(l) => self.interrompivel(conn, l, f),
        });
        self.soltar_leitura(leitura.as_ref());
        r
    }

    /// Marca a leitura como interrompível enquanto ela executa (a trava do banco já está presa).
    ///
    /// A marca e a conferência de "abandonada" andam juntas sob a trava pequena: ou esta leitura vê que foi
    /// abandonada e desiste antes de executar, ou a mais nova vê a marca e a interrompe. A marca sai antes de
    /// a trava do banco ser solta: nenhum comando de outra pessoa roda com ela presente. A abandonada devolve
    /// 409 `Substituido` também quando terminou o comando mas ainda não respondeu; interrompida no meio de um
    /// comando, o `detalhe` traz o erro que o SQLite deu.
    pub(crate) fn interrompivel<R>(
        &self,
        conn: &Connection,
        leitura: &Arc<Leitura>,
        f: impl FnOnce(&Connection) -> Resultado<R>,
    ) -> Resultado<R> {
        {
            let mut estado = self.leituras.lock();
            if leitura.abandonada() {
                return Err(erro_substituida());
            }
            estado.em_curso = Some(Arc::clone(leitura));
        }
        let r = {
            let _marca = Marca { banco: self, conn };
            let vigia = Arc::clone(leitura);
            match conn.progress_handler(PASSOS_ENTRE_CONFERENCIAS, Some(move || vigia.abandonada())) {
                Ok(()) => f(conn),
                Err(e) => Err(traduzir(e, None)),
            }
        };
        match r {
            Err(e) if leitura.abandonada() => Err(substituida_por_erro(&e)),
            Err(e) => Err(e),
            Ok(_) if leitura.abandonada() => Err(erro_substituida()),
            Ok(v) => Ok(v),
        }
    }
}

fn substituida_por_erro(original: &ErroBanco) -> ErroBanco {
    erro_substituida().detalhe(original.detalhe.clone().unwrap_or_else(|| original.mensagem.clone()))
}
