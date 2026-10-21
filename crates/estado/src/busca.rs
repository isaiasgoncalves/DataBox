//! A busca da barra nos arquivos grandes (o CSV, a planilha do Excel e o texto): uma tarefa percorre o arquivo inteiro
//! e vai entregando os achados ([`Coletor`]); a tela lê o que já chegou a cada quadro ([`BuscaEmFundo`]), vai ao
//! primeiro achado depois da célula (ou do cursor) de onde a busca começou, anda com o Enter e o Shift+Enter (com
//! volta) e diz na faixa "3 de 41" ("41+" enquanto conta). Trocar o termo troca a busca: a de antes é cancelada ao
//! ser solta (a leitura substituída, como a da grade do banco).
//!
//! ```ignore
//! // a cada tecla (o EventoBusca::Mudou):
//! self.busca = Some(BuscaEmFundo::nova(egui, origem, true, move |coletor| {
//!     tabela.achar_todos(&termo, &|| coletor.parar(), &mut |l, c| coletor.achou((l, c)))
//! }));
//! // a cada quadro:
//! if let Some(alvo) = self.busca.as_mut().and_then(|b| b.passo(egui)) { ir_para(alvo) }
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use comum::Resultado;
use egui::{Color32, Ui};
use interface::formato::{formatar_numero, plural};

use crate::promessa::Promessa;

/// Quantos achados a busca guarda (16 MB de posições, no máximo): depois disso, ela só conta, e o Enter anda entre os
/// guardados.
pub const LIMITE_DE_ACHADOS: usize = 1_000_000;
/// De quanto em quanto tempo a tarefa entrega o que achou (e a tela olha de novo enquanto ela conta).
const INTERVALO_DE_ENTREGA: Duration = Duration::from_millis(80);
/// A frase da busca que terminou sem achar nada.
pub const NENHUM_RESULTADO: &str = "Nenhum resultado";
/// A frase da busca que ainda não achou nada.
pub const BUSCANDO: &str = "Buscando…";

/// O que a tarefa já entregou.
#[derive(Debug)]
struct Parcial<P> {
    itens: Vec<P>,
    /// Quantos achou (também os que passaram do [`LIMITE_DE_ACHADOS`]).
    total: u64,
    terminou: bool,
}

/// O lado da tarefa: recebe cada achado e o entrega à tela de tempos em tempos.
pub struct Coletor<P> {
    caixa: Arc<Mutex<Parcial<P>>>,
    parar: Arc<AtomicBool>,
    novos: Vec<P>,
    /// Os achados guardados (os entregues e os novos) e os contados desde a última entrega.
    guardados: usize,
    contados: u64,
    ultima_entrega: Instant,
}

fn travar<P>(caixa: &Mutex<Parcial<P>>) -> MutexGuard<'_, Parcial<P>> {
    caixa.lock().unwrap_or_else(|e| e.into_inner())
}

impl<P> Coletor<P> {
    /// A busca foi cancelada (o termo mudou, a tela fechou): a tarefa deve parar.
    pub fn parar(&self) -> bool {
        self.parar.load(Ordering::Relaxed)
    }

    /// O mesmo que [`Coletor::parar`], solto do coletor (para a função que busca, enquanto o coletor recebe os achados).
    pub fn sinal_de_parar(&self) -> impl Fn() -> bool + Send + Sync + 'static {
        let parar = Arc::clone(&self.parar);
        move || parar.load(Ordering::Relaxed)
    }

    /// Mais um achado, na ordem em que a tela vai percorrê-los (veja [`BuscaEmFundo::nova`]). Devolve se a busca
    /// continua.
    pub fn achou(&mut self, achado: P) -> bool {
        self.contados += 1;
        if self.guardados < LIMITE_DE_ACHADOS {
            self.novos.push(achado);
            self.guardados += 1;
        }
        if self.ultima_entrega.elapsed() >= INTERVALO_DE_ENTREGA {
            self.entregar();
        }
        !self.parar()
    }

    fn entregar(&mut self) {
        let mut c = travar(&self.caixa);
        c.itens.append(&mut self.novos);
        c.total += self.contados;
        self.contados = 0;
        self.ultima_entrega = Instant::now();
    }
}

/// Uma busca rodando (ou terminada) no arquivo inteiro, com o achado da vez. `P` é a posição de um achado, na ordem
/// em que o Enter os percorre (no CSV, a linha da vista e a coluna; na planilha, a linha e a coluna; no texto, o
/// trecho).
pub struct BuscaEmFundo<P> {
    caixa: Arc<Mutex<Parcial<P>>>,
    parar: Arc<AtomicBool>,
    tarefa: Option<Promessa<()>>,
    /// Os achados chegam já na ordem final (sem ela, a tela só vai a um achado quando a busca termina).
    em_ordem: bool,
    /// De onde a busca começou, enquanto ela ainda não foi ao primeiro achado depois dele.
    origem: Option<P>,
    atual: Option<usize>,
    erro: Option<String>,
}

impl<P: Ord + Copy + Send + 'static> BuscaEmFundo<P> {
    /// Começa a busca numa tarefa: `buscar` percorre o arquivo e entrega cada achado ao [`Coletor`] (parando quando
    /// ele pede). `origem` é de onde a busca começa (a célula ativa, o cursor): a tela vai ao primeiro achado a
    /// partir dele, ou ao primeiro de todos. Com `em_ordem`, os achados chegam na ordem de `P` e a tela vai ao
    /// primeiro assim que ele chega; sem ela (o CSV ordenado por uma coluna), a tarefa os ordena no fim.
    pub fn nova(
        egui: &egui::Context,
        origem: Option<P>,
        em_ordem: bool,
        buscar: impl FnOnce(&mut Coletor<P>) -> Resultado<()> + Send + 'static,
    ) -> BuscaEmFundo<P> {
        let caixa = Arc::new(Mutex::new(Parcial { itens: Vec::new(), total: 0, terminou: false }));
        let parar = Arc::new(AtomicBool::new(false));
        let mut coletor = Coletor {
            caixa: Arc::clone(&caixa),
            parar: Arc::clone(&parar),
            novos: Vec::new(),
            guardados: 0,
            contados: 0,
            ultima_entrega: Instant::now(),
        };
        let tarefa = Promessa::rodar(egui, move || {
            let r = buscar(&mut coletor);
            coletor.entregar();
            if r.is_ok() && !coletor.parar() {
                let mut c = travar(&coletor.caixa);
                if !em_ordem {
                    c.itens.sort_unstable();
                }
                c.terminou = true;
            }
            r
        });
        BuscaEmFundo { caixa, parar, tarefa: Some(tarefa), em_ordem, origem, atual: None, erro: None }
    }

    /// A cada quadro: toma o fim da tarefa e, enquanto a busca não foi ao primeiro achado, vai a ele quando der.
    /// Devolve o achado para onde a tela deve ir agora (a célula ativa, o trecho pintado). Enquanto a tarefa conta,
    /// pede outro quadro daqui a pouco (a contagem da faixa anda).
    pub fn passo(&mut self, egui: &egui::Context) -> Option<P> {
        if let Some(r) = self.tarefa.as_mut().and_then(Promessa::tomar) {
            self.tarefa = None;
            if let Err(e) = r {
                self.erro = Some(e.mensagem);
            }
        }
        if self.tarefa.is_some() {
            egui.request_repaint_after(INTERVALO_DE_ENTREGA);
        }
        let origem = self.origem?;
        let c = travar(&self.caixa);
        let escolhido = if self.em_ordem || c.terminou {
            let i = c.itens.partition_point(|p| *p < origem);
            if i < c.itens.len() {
                Some(i)
            } else if c.terminou {
                (!c.itens.is_empty()).then_some(0)
            } else {
                None
            }
        } else {
            None
        };
        let terminou = c.terminou || self.erro.is_some();
        let alvo = escolhido.and_then(|i| c.itens.get(i).copied());
        drop(c);
        if escolhido.is_some() || terminou {
            self.origem = None;
            self.atual = escolhido;
        }
        alvo
    }

    /// O Enter (`adiante`) e o Shift+Enter: o achado seguinte ou o anterior, com volta (enquanto a busca conta, a
    /// volta espera o fim dela). Antes de ir ao primeiro, vai a ele se já se sabe qual é. Devolve para onde ir.
    pub fn seguinte(&mut self, adiante: bool) -> Option<P> {
        let c = travar(&self.caixa);
        let n = c.itens.len();
        let i = match self.atual {
            None => {
                let origem = self.origem?;
                if !(self.em_ordem || c.terminou) {
                    return None;
                }
                let i = c.itens.partition_point(|p| *p < origem);
                if i < n {
                    i
                } else if c.terminou && n > 0 {
                    0
                } else {
                    return None;
                }
            }
            Some(i) if adiante && i + 1 < n => i + 1,
            Some(_) if adiante && c.terminou => 0,
            Some(i) if !adiante && i > 0 => i - 1,
            Some(_) if !adiante && c.terminou && n > 0 => n - 1,
            Some(_) => return None,
        };
        let alvo = c.itens.get(i).copied();
        drop(c);
        if alvo.is_some() {
            self.atual = Some(i);
            self.origem = None;
        }
        alvo
    }

    /// De onde o próximo Enter parte, enquanto a busca não tem achado da vez (a que recomeçou sem ir a nenhum, porque
    /// o texto mudou debaixo dela).
    pub fn definir_origem(&mut self, origem: P) {
        if self.atual.is_none() {
            self.origem = Some(origem);
        }
    }

    /// O achado da vez.
    pub fn atual(&self) -> Option<P> {
        let i = self.atual?;
        travar(&self.caixa).itens.get(i).copied()
    }

    /// A tarefa terminou (ou falhou).
    pub fn terminou(&self) -> bool {
        self.tarefa.is_none()
    }

    /// Quantos achados há (até agora, enquanto conta).
    pub fn total(&self) -> u64 {
        travar(&self.caixa).total
    }

    /// O que a faixa diz: "3 de 41", "3 de 41+" e "41+" enquanto conta, "Buscando…", "Nenhum resultado", "41
    /// resultados" (sem achado da vez: a busca refeita debaixo de quem saiu do achado) ou o erro.
    pub fn rotulo(&self) -> String {
        if let Some(e) = &self.erro {
            return e.clone();
        }
        let c = travar(&self.caixa);
        let total = usize::try_from(c.total).unwrap_or(usize::MAX);
        let mais = if c.terminou { "" } else { "+" };
        match self.atual {
            Some(i) => format!("{} de {}{mais}", formatar_numero(i + 1), formatar_numero(total)),
            None if total == 0 && c.terminou => NENHUM_RESULTADO.to_owned(),
            None if total == 0 => BUSCANDO.to_owned(),
            None if c.terminou => plural(total, "resultado", "resultados"),
            None => format!("{}+", formatar_numero(total)),
        }
    }
}

impl<P> Drop for BuscaEmFundo<P> {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
    }
}

/// A contagem da busca na faixa do documento (12px em `cor`, com o mesmo texto para os leitores de tela).
pub fn rotulo_na_faixa(ui: &mut Ui, rotulo: &str, cor: Color32) {
    interface::pecas::texto::texto(ui, rotulo, 12.0, cor);
}

#[cfg(test)]
mod testes {
    use super::*;

    fn esperar<P: Ord + Copy + Send + 'static>(b: &mut BuscaEmFundo<P>, egui: &egui::Context) -> Option<P> {
        let inicio = Instant::now();
        loop {
            let alvo = b.passo(egui);
            if b.terminou() {
                return alvo.or_else(|| b.passo(egui));
            }
            assert!(inicio.elapsed() < Duration::from_secs(10), "a busca não terminou");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn com(itens: Vec<u32>) -> impl FnOnce(&mut Coletor<u32>) -> Resultado<()> + Send + 'static {
        move |c| {
            for i in itens {
                if !c.achou(i) {
                    break;
                }
            }
            Ok(())
        }
    }

    #[test]
    fn vai_ao_primeiro_depois_da_origem_e_anda_com_volta() {
        let egui = egui::Context::default();
        let mut b = BuscaEmFundo::nova(&egui, Some(25), true, com(vec![10, 20, 30, 40]));
        assert_eq!(esperar(&mut b, &egui), Some(30));
        assert_eq!(b.rotulo(), "3 de 4");
        assert_eq!(b.seguinte(true), Some(40));
        assert_eq!(b.seguinte(true), Some(10));
        assert_eq!(b.seguinte(false), Some(40));
        assert_eq!(b.rotulo(), "4 de 4");
        assert_eq!(b.atual(), Some(40));
        assert_eq!(b.total(), 4);
    }

    #[test]
    fn depois_do_ultimo_volta_ao_primeiro_e_sem_nada_diz_nenhum() {
        let egui = egui::Context::default();
        let mut b = BuscaEmFundo::nova(&egui, Some(99), true, com(vec![1, 2]));
        assert_eq!(esperar(&mut b, &egui), Some(1));
        // sem origem, a busca conta mas não vai a nenhum; o Enter parte da origem que a tela der
        let mut sem_origem = BuscaEmFundo::nova(&egui, None, true, com(vec![1, 2, 3]));
        assert_eq!(esperar(&mut sem_origem, &egui), None);
        assert_eq!(sem_origem.rotulo(), "3 resultados");
        sem_origem.definir_origem(2);
        assert_eq!(sem_origem.seguinte(true), Some(2));
        assert_eq!(sem_origem.rotulo(), "2 de 3");
        let mut vazia = BuscaEmFundo::nova(&egui, Some(0), true, com(vec![]));
        assert_eq!(esperar(&mut vazia, &egui), None);
        assert_eq!(vazia.rotulo(), NENHUM_RESULTADO);
        assert_eq!(vazia.seguinte(true), None);
    }

    #[test]
    fn fora_de_ordem_ordena_no_fim() {
        let egui = egui::Context::default();
        let mut b = BuscaEmFundo::nova(&egui, Some(0), false, com(vec![30, 10, 20]));
        assert_eq!(esperar(&mut b, &egui), Some(10));
        assert_eq!(b.seguinte(true), Some(20));
    }

    #[test]
    fn enquanto_conta_diz_mais_e_soltar_cancela() {
        let egui = egui::Context::default();
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let parou = Arc::new(AtomicBool::new(false));
        let p2 = Arc::clone(&parou);
        let mut b = BuscaEmFundo::nova(&egui, Some(0u32), true, move |c| {
            c.achou(1);
            c.entregar();
            let _ = rx.recv();
            while !c.parar() {
                std::thread::sleep(Duration::from_millis(1));
            }
            p2.store(true, Ordering::Relaxed);
            Ok(())
        });
        let inicio = Instant::now();
        while b.passo(&egui).is_none() {
            assert!(inicio.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(b.rotulo(), "1 de 1+");
        // a volta espera o fim da contagem
        assert_eq!(b.seguinte(true), None);
        assert!(!b.terminou());
        drop(b);
        let _ = tx.send(());
        let inicio = Instant::now();
        while !parou.load(Ordering::Relaxed) {
            assert!(inicio.elapsed() < Duration::from_secs(10), "a tarefa não parou");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
