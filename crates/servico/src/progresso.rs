//! Progresso e cancelamento de uma exportação acompanhada (§4.6). A tela cria um [`Progresso`], passa um `Arc` dele para
//! `Servico::exportar` e guarda outro para ler o andamento e cancelar.
//!
//! O andamento tem trava própria, pequena (nunca a do banco): ler ([`Progresso::instantaneo`]) ou cancelar
//! responde em microssegundos, mesmo com a exportação segurando a conexão principal do banco.
//!
//! Cada `iniciar` abre uma [`Rodada`]: o que
//! sobra de uma exportação (a guarda contra pânico) só mexe na rodada dela.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use comum::{Codigo, ErroBanco, Resultado};

use crate::{MSG_EXPORTACAO_CANCELADA, MSG_TAREFA_EM_ANDAMENTO};

/// Em que ponto está a exportação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Fase {
    /// Ainda não começou (a exportação não chegou, ou foi cancelada antes de chegar). A tela mostra "Preparando a exportação…".
    #[default]
    Preparando,
    /// Contando as linhas antes de gravar (tabela e banco inteiro).
    Contando,
    Gravando,
    /// As linhas acabaram e o arquivo está sendo fechado.
    Finalizando,
    Concluida,
    Cancelada,
    Erro,
}

impl Fase {
    pub const TODAS: [Fase; 7] = [
        Fase::Contando,
        Fase::Gravando,
        Fase::Finalizando,
        Fase::Concluida,
        Fase::Cancelada,
        Fase::Erro,
        Fase::Preparando,
    ];

    /// "contando" | "gravando" | "finalizando" | "concluida" | "cancelada" | "erro" | "desconhecida" (Preparando).
    pub fn como_str(self) -> &'static str {
        match self {
            Fase::Preparando => "desconhecida",
            Fase::Contando => "contando",
            Fase::Gravando => "gravando",
            Fase::Finalizando => "finalizando",
            Fase::Concluida => "concluida",
            Fase::Cancelada => "cancelada",
            Fase::Erro => "erro",
        }
    }

    /// Concluída, cancelada ou com erro: não muda mais.
    pub fn terminou(self) -> bool {
        matches!(self, Fase::Concluida | Fase::Cancelada | Fase::Erro)
    }
}

/// O andamento num instante.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProgressoInfo {
    pub fase: Fase,
    /// Linhas gravadas até ali, a cada ~1.000 (no fim, as do resumo).
    pub linhas: u64,
    /// Tabela: contagem com os mesmos busca/filtros/chaves; consulta: None; banco: soma das tabelas ou None
    /// se contar passou de `PRAZO_CONTAGEM_BANCO`.
    pub total: Option<u64>,
    /// A tabela sendo gravada (None na consulta).
    pub tabela_atual: Option<String>,
    /// 0/1 numa tabela ou consulta (1/1 ao finalizar); as do banco no banco inteiro.
    pub tabelas_feitas: usize,
    pub tabelas_total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Situacao {
    #[default]
    Nova,
    Rodando,
    Terminada,
}

#[derive(Debug, Default)]
struct Interno {
    info: ProgressoInfo,
    situacao: Situacao,
    /// Quantas vezes [`Progresso::iniciar`] começou uma exportação: identifica a vez atual (a `Tarefa` nova de
    /// cada `iniciar`), para o que sobra de uma vez anterior não mexer na seguinte.
    rodada: u64,
}

/// Uma vez (rodada) do [`Progresso`] numa exportação, devolvida por [`Progresso::iniciar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rodada(u64);

/// O andamento de uma exportação, para ler e cancelar de outra thread (use por `Arc`). Crie um novo
/// ([`Progresso::novo`]) a cada chamada de `Servico::exportar`, inclusive na nova tentativa com `substituir`:
/// reaproveitado, o andamento começa mostrando o fim da vez
/// anterior e um cancelamento feito antes de a exportação nova chegar se perde.
#[derive(Debug, Default)]
pub struct Progresso {
    /// Lido pelo progress handler do SQLite a cada poucos milhares de passos: atômico, sem trava.
    cancelado: AtomicBool,
    interno: Mutex<Interno>,
}

impl Progresso {
    /// Um andamento novo, na fase [`Fase::Preparando`].
    pub fn novo() -> Arc<Progresso> {
        Arc::new(Progresso::default())
    }

    fn travar(&self) -> MutexGuard<'_, Interno> {
        self.interno.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Pede para a exportação parar: ela para no próximo aviso (a cada ~1.000 linhas, ao trocar de tabela, ao
    /// finalizar) ou na hora, se está esperando o SQLite (contagem, ordenação). Antes de a exportação chegar,
    /// fica anotado e ela termina cancelada sem gravar nada; depois de terminada, não muda nada.
    pub fn cancelar(&self) {
        let interno = self.travar();
        if interno.situacao != Situacao::Terminada {
            self.cancelado.store(true, Ordering::SeqCst);
        }
    }

    /// O cancelamento foi pedido (para a exportação em andamento ou a que vai chegar).
    pub fn cancelado(&self) -> bool {
        self.cancelado.load(Ordering::SeqCst)
    }

    /// Uma cópia do andamento agora.
    pub fn instantaneo(&self) -> ProgressoInfo {
        self.travar().info.clone()
    }

    /// Começa uma exportação com este andamento (o `RegistroExportacoes.iniciar`): já rodando → 409
    /// "Já há uma exportação em andamento com essa tarefa."; terminado antes → recomeça do zero (sem o
    /// cancelamento da vez anterior). A [`Rodada`] devolvida identifica esta vez em `terminar` e `abandonar`.
    pub(crate) fn iniciar(&self) -> Resultado<Rodada> {
        let mut interno = self.travar();
        match interno.situacao {
            Situacao::Rodando => return Err(ErroBanco::com_status(MSG_TAREFA_EM_ANDAMENTO, 409)),
            Situacao::Terminada => self.cancelado.store(false, Ordering::SeqCst),
            Situacao::Nova => {}
        }
        interno.situacao = Situacao::Rodando;
        interno.rodada += 1;
        // começa contando
        interno.info = ProgressoInfo { fase: Fase::Contando, ..ProgressoInfo::default() };
        Ok(Rodada(interno.rodada))
    }

    pub(crate) fn atualizar(&self, ajustar: impl FnOnce(&mut ProgressoInfo)) {
        ajustar(&mut self.travar().info);
    }

    /// Termina a `rodada` na fase dada (concluída, cancelada ou erro), com os ajustes finais. Se outra vez já
    /// começou (não deveria: só começa depois de esta terminar), não mexe nela.
    pub(crate) fn terminar(&self, rodada: Rodada, fase: Fase, ajustar: impl FnOnce(&mut ProgressoInfo)) {
        let mut interno = self.travar();
        if Rodada(interno.rodada) != rodada {
            return;
        }
        ajustar(&mut interno.info);
        interno.info.fase = fase;
        interno.situacao = Situacao::Terminada;
    }

    /// A `rodada` ainda rodando (a exportação saiu sem terminar, num pânico) → termina com erro. Uma rodada
    /// já terminada, ou uma vez seguinte que já começou com este andamento, fica como está.
    pub(crate) fn abandonar(&self, rodada: Rodada) {
        let mut interno = self.travar();
        if Rodada(interno.rodada) == rodada && interno.situacao == Situacao::Rodando {
            interno.info.fase = Fase::Erro;
            interno.situacao = Situacao::Terminada;
        }
    }

    /// A exportação foi recusada antes de começar (nenhum banco aberto, banco trocado): termina com erro, para
    /// todo `Servico::exportar` que recebe um andamento o deixar terminado ao voltar. Rodando em outra
    /// exportação → não é deste pedido: fica como está.
    pub(crate) fn recusar(&self) {
        let mut interno = self.travar();
        if interno.situacao != Situacao::Rodando {
            interno.info.fase = Fase::Erro;
            interno.situacao = Situacao::Terminada;
        }
    }

    /// Cancelada → o 409 `Codigo::Cancelado` que interrompe a exportação.
    pub(crate) fn conferir(&self) -> Resultado<()> {
        if self.cancelado() { Err(erro_cancelada()) } else { Ok(()) }
    }
}

/// 409 "Exportação interrompida; nenhum arquivo foi gravado." com `Codigo::Cancelado`.
pub(crate) fn erro_cancelada() -> ErroBanco {
    ErroBanco::com_status(MSG_EXPORTACAO_CANCELADA, 409).codigo(Codigo::Cancelado)
}
