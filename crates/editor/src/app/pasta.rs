//! A pasta aberta (etapa 4): abrir, ler a raiz e as subpastas numa tarefa, reler (a volta do foco, o fim de uma
//! troca ou de uma janela, o Atualizar) e fechar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use comum::Resultado;
use comum::arquivos::{Listagem, Natureza};
use estado::{NoDaPasta, PastaAberta};

use crate::documento::Documento;

use super::tarefas::Tarefa;
use super::{Editor, INTERVALO_RELEITURA, Origem};

impl Editor {
    /// Há uma leitura de pasta em andamento (a raiz, uma subpasta ou a abertura de uma pasta).
    pub(super) fn lendo_pasta(&self) -> bool {
        self.tarefas.iter().any(|t| matches!(t, Tarefa::Pasta { .. } | Tarefa::AbrirPasta { .. }))
    }

    /// Abre uma pasta numa tarefa (`Servico::abrir_pasta`: confere, lê a raiz, entra nos recentes e fica como a
    /// pasta aberta nas preferências). A pasta aberta de antes continua à vista até a nova chegar; uma abertura de
    /// cada vez (outra pedida no meio é ignorada).
    pub(super) fn abrir_pasta(&mut self, ctx: &egui::Context, caminho: PathBuf, origem: Origem) {
        if !self.carregado || self.tarefas.iter().any(|t| matches!(t, Tarefa::AbrirPasta { .. })) {
            if origem == Origem::Janela
                && let Some(j) = self.janelas.caminho.as_mut()
            {
                j.desocupar();
            }
            return;
        }
        let c = caminho.clone();
        let promessa = self.tarefa(ctx, move |s, _| {
            let listagem = s.abrir_pasta(&c)?;
            Ok((listagem, s.prefs().resumo()))
        });
        let geracao = self.geracao_pasta;
        self.tarefas.push(Tarefa::AbrirPasta { origem, caminho, geracao, promessa });
    }

    /// A pasta que abriu: vira a pasta aberta, com a raiz já lida. A mesma pasta de novo só troca a raiz (as
    /// subpastas lidas ficam).
    pub(super) fn por_a_pasta(&mut self, listagem: Listagem) {
        let chave = prefs::chave_caminho(&listagem.pasta);
        match self.estado.pasta.as_mut() {
            Some(p) if p.chave == chave => {
                p.nos.insert(chave, NoDaPasta::Pronto(Arc::new(listagem)));
            }
            _ => {
                // outra pasta: as leituras da de antes que ainda correm não valem
                self.geracao_pasta += 1;
                let mut p = PastaAberta::nova(listagem.pasta.clone());
                p.nos.insert(chave, NoDaPasta::Pronto(Arc::new(listagem)));
                self.estado.pasta = Some(p);
            }
        }
    }

    /// A pasta que estava aberta na sessão anterior (as preferências): volta com a raiz lendo, e a leitura é pedida
    /// agora. A que sumiu fica no Explorador com "Não foi possível ler a pasta." e o "Tentar de novo" (a chave fica).
    pub(super) fn restaurar_pasta(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        self.geracao_pasta += 1;
        self.estado.pasta = Some(PastaAberta::nova(caminho.clone()));
        self.mudou_estado(ctx);
        self.listar_pasta(ctx, caminho);
    }

    /// Lê uma pasta da árvore (a raiz ou uma subpasta da pasta aberta) numa tarefa. A pasta ainda não lida (ou que
    /// falhou) mostra "Carregando…" enquanto isso; a já lida continua à vista, e a leitura nova reaproveita os
    /// cabeçalhos dela. Uma leitura da mesma pasta já em andamento não é pedida de novo.
    pub(super) fn listar_pasta(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        let no = prefs::chave_caminho(&caminho);
        let Some(p) = self.estado.pasta.as_mut() else { return };
        let raiz = p.chave.clone();
        let em_andamento =
            self.tarefas.iter().any(|t| matches!(t, Tarefa::Pasta { raiz: r, no: n, .. } if *r == raiz && *n == no));
        if em_andamento {
            return;
        }
        let anterior = match p.nos.get(&no) {
            Some(NoDaPasta::Pronto(l)) => Some(Arc::clone(l)),
            _ => None,
        };
        let marcar = anterior.is_none() && p.nos.get(&no) != Some(&NoDaPasta::Lendo);
        if marcar {
            p.nos.insert(no.clone(), NoDaPasta::Lendo);
        }
        self.leituras_de_pasta += 1;
        let promessa = self.tarefa(ctx, move |s, _| s.listar_pasta(&caminho, anterior.as_deref()));
        let geracao = self.geracao_pasta;
        self.tarefas.push(Tarefa::Pasta { raiz, no, geracao, promessa });
        if marcar {
            self.mudou_estado(ctx);
        }
    }

    /// Chegou a leitura de uma pasta: vai para `estado.pasta.nos`, a não ser que a pasta aberta tenha mudado nesse
    /// meio-tempo. Se nada mudou (a releitura do foco), o estado fica como está e nada repinta.
    pub(super) fn chegou_a_pasta(&mut self, ctx: &egui::Context, raiz: &str, no: String, r: Resultado<Listagem>) {
        let Some(p) = self.estado.pasta.as_mut() else { return };
        if p.chave != raiz {
            return;
        }
        let novo = match r {
            Ok(l) => NoDaPasta::Pronto(Arc::new(l)),
            Err(e) => {
                log::warn!("Não foi possível ler a pasta: {e}");
                NoDaPasta::Falhou(e.mensagem)
            }
        };
        if p.nos.get(&no) == Some(&novo) {
            return;
        }
        p.nos.insert(no, novo);
        self.mudou_estado(ctx);
    }

    /// O `Acao::AtualizarPasta`: relê a raiz e as subpastas já lidas (o caminho delas sai da listagem de quem as
    /// contém). Sem pasta aberta, nada.
    pub(super) fn atualizar_pasta(&mut self, ctx: &egui::Context) {
        let Some(p) = self.estado.pasta.as_ref() else { return };
        let subpastas: HashMap<&str, &Path> = p
            .nos
            .values()
            .filter_map(|no| match no {
                NoDaPasta::Pronto(l) => Some(l),
                _ => None,
            })
            .flat_map(|l| l.entradas.iter().filter(|e| e.natureza == Natureza::Pasta))
            .map(|e| (e.chave.as_str(), e.caminho.as_path()))
            .collect();
        let mut caminhos: Vec<PathBuf> = p
            .nos
            .iter()
            // as que falharam também (uma falha passageira da rede não fica até a pessoa abrir a pasta de novo)
            .filter(|(chave, no)| **chave != p.chave && matches!(no, NoDaPasta::Pronto(_) | NoDaPasta::Falhou(_)))
            .filter_map(|(chave, _)| subpastas.get(chave.as_str()).map(|c| c.to_path_buf()))
            .collect();
        caminhos.sort();
        caminhos.insert(0, p.caminho.clone());
        for c in caminhos {
            self.listar_pasta(ctx, c);
        }
    }

    /// A releitura de quando o próprio app pode ter criado arquivos na pasta (o fim de uma troca de banco, o Novo
    /// banco; o fim de uma janela, a Exportar; a cópia de segurança): sem o espaçamento da volta do foco; com uma
    /// leitura em andamento, fica para quando ela terminar ([`Editor::reler_pendente`]), sem se perder.
    pub(super) fn reler_a_pasta(&mut self, ctx: &egui::Context) {
        if self.estado.pasta.is_none() {
            return;
        }
        if self.lendo_pasta() {
            self.reler_pendente = true;
            return;
        }
        self.reler_pendente = false;
        self.atualizar_pasta(ctx);
    }

    /// A releitura da volta do foco da janela: no máximo uma a cada [`INTERVALO_RELEITURA`] e nunca com uma leitura
    /// em andamento.
    pub(super) fn releitura_automatica(&mut self, ctx: &egui::Context) {
        if self.estado.pasta.is_none() || self.lendo_pasta() {
            return;
        }
        let agora = ctx.input(|i| i.time);
        if self.ultima_releitura.is_some_and(|antes| agora - antes < INTERVALO_RELEITURA) {
            return;
        }
        self.ultima_releitura = Some(agora);
        self.atualizar_pasta(ctx);
    }

    /// O Fechar pasta: tira a árvore, fecha as abas de arquivo de dados de dentro da pasta (as de fora ficam) e apaga
    /// a pasta aberta das preferências numa tarefa. O banco continua aberto; os recentes ficam.
    pub(super) fn fechar_pasta(&mut self, ctx: &egui::Context) {
        let Some(p) = self.estado.pasta.take() else { return };
        // uma abertura de pasta ainda a caminho não reabre a árvore depois deste fechar, e as leituras pedidas antes
        // dele não valem para a pasta aberta depois
        self.geracao_pasta += 1;
        self.reler_pendente = false;
        let dentro = |a: &estado::AbaAberta<Documento>| a.rota().caminho_arquivo().is_some_and(|c| p.contem(c));
        let fechadas = self.abas.fechar_onde(dentro, |id, r| Documento::novo(id, r, None));
        let id_banco = self.estado.banco_id();
        for aba in fechadas {
            self.soltar(aba.conteudo, &id_banco);
        }
        let promessa = self.tarefa(ctx, |s, _| {
            s.fechar_pasta();
            Ok(())
        });
        self.tarefas.push(Tarefa::Gravar(promessa));
        self.mudou_estado(ctx);
    }
}
