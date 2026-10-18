//! Tarefa de fundo: a thread da interface nunca toca no SQLite nem no disco.
//!
//! ```ignore
//! // ao pedir (ex.: ao abrir a tela):
//! self.carga = Some(Promessa::rodar(ui.ctx(), move || servico.tabela(&id, &nome)));
//! // a cada quadro:
//! if let Some(r) = self.carga.as_mut().and_then(Promessa::tomar) {
//!     self.carga = None;
//!     match r { Ok(info) => …, Err(e) => … }
//! }
//! ```
//!
//! Ao terminar, a tarefa pede uma repintura: a tela não precisa repintar sem parar enquanto espera. Um pânico
//! dentro da tarefa vira `ErroBanco` 500 com a frase de erro inesperado do servidor, em vez de deixar a tela
//! esperando para sempre.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use comum::{ErroBanco, Resultado};

/// Frase de erro inesperado.
pub const MSG_INESPERADO: &str = "Erro inesperado. Tente de novo; se continuar, feche e abra o DataBox.";

/// Tarefas de fundo rodando agora (em todo o processo).
static EM_ANDAMENTO: AtomicUsize = AtomicUsize::new(0);

/// Quantas tarefas de fundo estão rodando agora.
pub fn tarefas_em_andamento() -> usize {
    EM_ANDAMENTO.load(Ordering::SeqCst)
}

/// Espera as tarefas de fundo terminarem, até `prazo`; true se terminaram. O executável chama ao fechar a
/// janela: uma gravação em voo vai até o fim, e uma exportação interrompida apaga o temporário dela.
pub fn esperar_tarefas(prazo: Duration) -> bool {
    let inicio = Instant::now();
    while tarefas_em_andamento() > 0 {
        if inicio.elapsed() >= prazo {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

/// Desconta a tarefa ao sair da thread, até num pânico.
struct Contada;

impl Drop for Contada {
    fn drop(&mut self) {
        EM_ANDAMENTO.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Resultado que chega de uma thread de fundo.
pub struct Promessa<T> {
    caixa: Arc<Mutex<Option<Resultado<T>>>>,
    entregue: bool,
}

impl<T: Send + 'static> Promessa<T> {
    /// Roda `f` numa thread nova e acorda a interface quando terminar.
    pub fn rodar(ctx: &egui::Context, f: impl FnOnce() -> Resultado<T> + Send + 'static) -> Promessa<T> {
        let caixa = Arc::new(Mutex::new(None));
        let destino = Arc::clone(&caixa);
        let ctx = ctx.clone();
        EM_ANDAMENTO.fetch_add(1, Ordering::SeqCst);
        let criada = std::thread::Builder::new().name("editor-tarefa".into()).spawn(move || {
            let _contada = Contada;
            let resultado = catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|panico| {
                let texto = panico
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panico.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "pânico sem mensagem".into());
                log::error!("Pânico numa tarefa de fundo: {texto}");
                Err(ErroBanco::com_status(MSG_INESPERADO, 500).detalhe(texto))
            });
            *destino.lock().unwrap_or_else(|e| e.into_inner()) = Some(resultado);
            ctx.request_repaint();
        });
        if let Err(e) = criada {
            EM_ANDAMENTO.fetch_sub(1, Ordering::SeqCst);
            *caixa.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(Err(ErroBanco::com_status(MSG_INESPERADO, 500).detalhe(e.to_string())));
        }
        Promessa { caixa, entregue: false }
    }

    /// Uma promessa já resolvida (útil em testes e quando o resultado já é conhecido).
    pub fn pronta(resultado: Resultado<T>) -> Promessa<T> {
        Promessa { caixa: Arc::new(Mutex::new(Some(resultado))), entregue: false }
    }
}

impl<T> Promessa<T> {
    /// O resultado, uma única vez, quando a tarefa terminar; `None` enquanto ela roda (e depois de entregue).
    pub fn tomar(&mut self) -> Option<Resultado<T>> {
        if self.entregue {
            return None;
        }
        let r = self.caixa.lock().unwrap_or_else(|e| e.into_inner()).take();
        if r.is_some() {
            self.entregue = true;
        }
        r
    }

    /// A tarefa ainda não entregou o resultado.
    pub fn pendente(&self) -> bool {
        !self.entregue
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::time::{Duration, Instant};

    fn esperar<T: Send + 'static>(p: &mut Promessa<T>) -> Resultado<T> {
        let inicio = Instant::now();
        loop {
            if let Some(r) = p.tomar() {
                return r;
            }
            assert!(inicio.elapsed() < Duration::from_secs(10), "a tarefa não terminou");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn entrega_uma_vez() {
        let ctx = egui::Context::default();
        let mut p = Promessa::rodar(&ctx, || Ok(41 + 1));
        assert_eq!(esperar(&mut p), Ok(42));
        assert!(p.tomar().is_none());
        assert!(!p.pendente());
    }

    #[test]
    fn panico_vira_erro_500() {
        let ctx = egui::Context::default();
        let mut p: Promessa<()> = Promessa::rodar(&ctx, || panic!("explodiu"));
        let e = esperar(&mut p).unwrap_err();
        assert_eq!(e.status, 500);
        assert_eq!(e.mensagem, MSG_INESPERADO);
        assert_eq!(e.detalhe.as_deref(), Some("explodiu"));
    }
}
