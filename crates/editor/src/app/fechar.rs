//! Fechar: a fila de perguntas ao fechar abas alteradas ([`FilaDeSalvar`]: Não salvar · Cancelar · Salvar, uma
//! aba de cada vez) e as perguntas antes de fechar a janela (a exportação em andamento, a transação aberta),
//! com o pedido que chega com a janela minimizada ou encoberta ([`Editor::logica`]) e a confirmação aberta.

use std::collections::{HashSet, VecDeque};

use comum::app::NOME;
use egui::ViewportCommand;
use estado::{IdAba, Verbo};
use interface::formato::formatar_numero;
use interface::pecas::janela::{Confirmacao, Decisao, confirmar_decisao};
use interface::pecas::toast::toast_erro;

use crate::documento::Documento;
use crate::telas::JanelaExportar;

use super::tarefas::Tarefa;
use super::{Confirmado, Editor, Origem, frase_descartar};

/// Para onde a fila de perguntas ao fechar vai quando acaba.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Destino {
    /// O ×, o clique do meio, o Ctrl+W, o Arquivo › Fechar aba.
    Aba(IdAba),
    /// O Fechar pasta (fecha as abas de arquivo de dentro dela), com a chave da pasta de quando a fila começou: se
    /// outra pasta abriu no meio, nada fecha.
    Pasta(String),
    /// O pedido de fechar a janela: depois dos arquivos, as perguntas da exportação e da transação que já existiam.
    Janela,
}

/// A fila de perguntas ao fechar: as abas alteradas que vão fechar, na ordem da faixa. Uma de cada vez,
/// a aba vai à frente e a pergunta "Salvar as alterações em {nome}?" sai (Não salvar · Cancelar · Salvar). Salvar
/// roda o verbo Salvar do documento e espera a gravação terminar (`salvando`); se ela falha (o erro de
/// `tomar_erro_de_gravacao`), o toast com o erro e tudo para, sem fechar nada. Cancelar (e o Esc) também para tudo.
/// Quando a fila acaba, o [`Destino`], depois de conferir de novo: a aba alterada no meio (enquanto uma gravação era
/// esperada) entra na fila antes de fechar. Se a pergunta aberta some sem resposta (outra confirmação tomou o lugar
/// dela, o banco trocado por fora), a fila para: nada fecha sem a pessoa responder.
pub(super) struct FilaDeSalvar {
    restantes: VecDeque<IdAba>,
    destino: Destino,
    /// A aba cuja gravação a fila espera terminar.
    esperando: Option<IdAba>,
    /// A aba da pergunta aberta agora (até a resposta).
    perguntando: Option<IdAba>,
    /// As abas já respondidas (Não salvar, ou Salvar que gravou): não se pergunta de novo.
    respondidas: HashSet<IdAba>,
}

/// O botão da segunda saída da pergunta ao fechar (prancheta 15, N2).
pub(crate) const NAO_SALVAR: &str = "Não salvar";

/// O título da pergunta da fila ao fechar: "Salvar as alterações em vendas_2025.csv?".
pub(crate) fn titulo_salvar_alteracoes(nome: &str) -> String {
    format!("Salvar as alterações em {nome}?")
}

/// A frase da pergunta: "Há 1 alteração não salva. Se não salvar, ela se perde.", "Há 3 alterações não salvas. Se
/// não salvar, elas se perdem." ou, sem a contagem (o texto e o JSON), "Há alterações não salvas. Se não salvar, elas
/// se perdem.".
pub(crate) fn frase_salvar_alteracoes(alteracoes: Option<usize>) -> String {
    match alteracoes {
        Some(1) => "Há 1 alteração não salva. Se não salvar, ela se perde.".to_string(),
        Some(n) if n > 1 => format!("Há {} alterações não salvas. Se não salvar, elas se perdem.", formatar_numero(n)),
        _ => "Há alterações não salvas. Se não salvar, elas se perdem.".to_string(),
    }
}

impl Editor {
    /// As abas alteradas (`estado::Documento::alterado`) entre as que `que` escolhe, na ordem da faixa.
    fn abas_alteradas(&self, que: impl Fn(&estado::AbaAberta<Documento>) -> bool) -> VecDeque<IdAba> {
        self.abas.iter().filter(|a| que(a) && a.conteudo.alterado()).map(|a| a.id()).collect()
    }

    /// Começa a fila de perguntas sobre estas abas alteradas, com o destino de quando acabar; false (nada começa)
    /// sem nenhuma.
    fn comecar_fila(&mut self, ctx: &egui::Context, restantes: VecDeque<IdAba>, destino: Destino) -> bool {
        if restantes.is_empty() {
            return false;
        }
        self.fila =
            Some(FilaDeSalvar { restantes, destino, esperando: None, perguntando: None, respondidas: HashSet::new() });
        ctx.request_repaint();
        true
    }

    /// As abas que o destino fecha (a da aba, as de arquivo de dentro da pasta, todas).
    fn abas_do_destino(&self, destino: &Destino) -> VecDeque<IdAba> {
        match destino {
            Destino::Aba(id) => self.abas_alteradas(|a| a.id() == *id),
            Destino::Pasta(chave) => match self.estado.pasta.as_ref().filter(|p| p.chave == *chave) {
                Some(pasta) => self.abas_alteradas(|a| a.rota().caminho_arquivo().is_some_and(|c| pasta.contem(c))),
                None => VecDeque::new(),
            },
            Destino::Janela => self.abas_alteradas(|_| true),
        }
    }

    /// O `FecharAba`: a aba alterada passa pela fila de perguntas; a outra fecha. Com a fila andando, outro pedido
    /// de fechar espera (é ignorado: a pergunta é modal, e só a gravação esperada deixa clicar).
    pub(super) fn pedir_fechar_aba(&mut self, ctx: &egui::Context, id: IdAba) {
        if self.fila.is_some() {
            return;
        }
        let alteradas = self.abas_alteradas(|a| a.id() == id);
        if !self.comecar_fila(ctx, alteradas, Destino::Aba(id)) {
            self.fechar_aba(id);
        }
    }

    /// O `FecharPasta`: uma pergunta por arquivo alterado de dentro da pasta, e depois o fechar; sem nenhum, fecha.
    pub(super) fn pedir_fechar_pasta(&mut self, ctx: &egui::Context) {
        if self.fila.is_some() {
            return;
        }
        let Some(chave) = self.estado.pasta.as_ref().map(|p| p.chave.clone()) else { return };
        let destino = Destino::Pasta(chave);
        let alteradas = self.abas_do_destino(&destino);
        if !self.comecar_fila(ctx, alteradas, destino) {
            self.fechar_pasta(ctx);
        }
    }

    /// O pedido de fechar a janela com abas alteradas: a fila de todas, antes das perguntas da exportação e da
    /// transação. True quando a fila começou (ou já andava): quem chama cancela o fechar. Com uma fila de outro
    /// destino andando (fechar uma aba, a pasta), ela passa a ser a de fechar a janela: no fim, as alteradas que
    /// faltam entram nela e a janela fecha.
    fn fila_ao_fechar_a_janela(&mut self, ctx: &egui::Context) -> bool {
        if let Some(f) = self.fila.as_mut() {
            f.destino = Destino::Janela;
            ctx.request_repaint();
            return true;
        }
        let alteradas = self.abas_alteradas(|_| true);
        self.comecar_fila(ctx, alteradas, Destino::Janela)
    }

    /// Anda a fila (no fim do quadro, antes da confirmação): espera a gravação pedida, faz a próxima pergunta (com a
    /// aba dela à frente) ou, vazia, cumpre o destino.
    pub(super) fn andar_fila(&mut self, ctx: &egui::Context) {
        let ativa = self.abas.id_ativa();
        let Some(fila) = self.fila.as_mut() else { return };
        if let Some(id) = fila.esperando {
            if let Some(aba) = self.abas.get_mut(id) {
                // a tela perguntou algo no meio (o "Salvar mesmo assim" do arquivo que mudou no disco): espera a
                // resposta, com a aba à vista para a janela dela desenhar; fechada a pergunta com o documento ainda
                // alterado (abaixo), a pessoa cancelou
                if aba.conteudo.pergunta_aberta() {
                    if id != ativa {
                        self.abas.focar(id);
                        ctx.request_repaint();
                    }
                    return;
                }
                let doc = aba.conteudo.doc();
                if doc.salvando() {
                    // a resposta da gravação acorda a interface (a `Promessa` da tela)
                    return;
                }
                let erro = doc.tomar_erro_de_gravacao();
                let ainda_alterado = doc.alterado();
                fila.esperando = None;
                if let Some(e) = erro {
                    toast_erro(ctx, e);
                    self.fila = None;
                    return;
                }
                if ainda_alterado {
                    // o documento não gravou (o Salvar não estava ligado, a gravação foi cancelada): nada fecha
                    self.fila = None;
                    return;
                }
                fila.respondidas.insert(id);
            }
            // a aba que gravava fechou no meio: não há o que esperar
            fila.esperando = None;
        }
        // a pergunta desta fila sumiu sem resposta (outra confirmação tomou o lugar dela, o banco trocado por fora
        // fechou tudo): a fila para, e nada fecha sem a pessoa responder
        if let Some(id) = fila.perguntando {
            let ainda =
                self.confirmacao.is_some() && matches!(self.confirmado, Some(Confirmado::SalvarAlterado(p)) if p == id);
            if !ainda {
                self.fila = None;
                return;
            }
        }
        // uma pergunta (a desta fila ou outra) está aberta: espera a resposta
        if self.confirmacao.is_some() {
            return;
        }
        while let Some(id) = fila.restantes.pop_front() {
            if fila.respondidas.contains(&id) {
                continue;
            }
            let Some(aba) = self.abas.get(id).filter(|a| a.conteudo.alterado()) else { continue };
            let (nome, _) = aba.conteudo.titulo(aba.rota(), aba.numero_consulta(), &self.estado);
            let alteracoes = aba.conteudo.doc_ref().alteracoes();
            self.confirmacao = Some(
                Confirmacao::new(frase_salvar_alteracoes(alteracoes))
                    .titulo(titulo_salvar_alteracoes(&nome))
                    .rotulo("Salvar")
                    .descartar(NAO_SALVAR),
            );
            self.confirmado = Some(Confirmado::SalvarAlterado(id));
            fila.perguntando = Some(id);
            // a aba da pergunta vai à frente
            self.abas.focar(id);
            ctx.request_repaint();
            return;
        }
        // antes de fechar, de novo: uma aba do destino alterada no meio (enquanto uma gravação era esperada) pergunta
        // também
        let destino = fila.destino.clone();
        let faltam: VecDeque<IdAba> = self
            .abas_do_destino(&destino)
            .into_iter()
            .filter(|id| self.fila.as_ref().is_some_and(|f| !f.respondidas.contains(id)))
            .collect();
        if !faltam.is_empty() {
            if let Some(f) = self.fila.as_mut() {
                f.restantes = faltam;
            }
            ctx.request_repaint();
            return;
        }
        let Some(FilaDeSalvar { destino, .. }) = self.fila.take() else { return };
        match destino {
            Destino::Aba(id) => self.fechar_aba(id),
            Destino::Pasta(chave) => {
                // outra pasta abriu enquanto a fila andava: a fila era da de antes, e nada fecha
                if self.estado.pasta.as_ref().is_some_and(|p| p.chave == chave) {
                    self.fechar_pasta(ctx);
                }
            }
            Destino::Janela => {
                let (exportando, transacao) = (self.exportando(), self.estado.transacao_aberta());
                if exportando || transacao {
                    self.perguntar_ao_fechar(exportando, transacao);
                } else {
                    self.pode_fechar = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint();
    }

    fn exportando(&self) -> bool {
        self.janelas.exportar.as_ref().is_some_and(JanelaExportar::exportando)
    }

    /// Há o que perguntar ao fechar ou o que ainda pode mudar a resposta: uma aba com alterações não salvas (etapa 5),
    /// uma exportação em andamento, a transação aberta, uma Consulta que executa (também com a resposta já pronta,
    /// que só um quadro toma: um BEGIN que chegou com a janela minimizada), a execução solta de uma Consulta fechada
    /// ou a fila de perguntas ao fechar andando.
    fn algo_a_conferir_ao_fechar(&self) -> bool {
        self.fila.is_some()
            || self.abas.iter().any(|a| a.conteudo.alterado())
            || self.exportando()
            || self.estado.transacao_aberta()
            || self.abas.iter().any(|a| a.conteudo.executando())
            || self.tarefas.iter().any(|t| matches!(t, Tarefa::Consulta(..)))
    }

    /// O corpo do `eframe::App::logic`. Com a janela minimizada ou encoberta, o eframe não desenha quadro e só
    /// chama o `logic`: um pedido de fechar que chega assim (pela barra de tarefas) fecharia sem a pergunta do
    /// `pedido_de_fechar`, e o estado da casca pode estar atrasado (as respostas que chegaram esperam um quadro).
    /// Com algo a conferir ([`Editor::algo_a_conferir_ao_fechar`]), cancela o fechar, restaura a janela e deixa a
    /// decisão para o primeiro quadro dela. Com a janela à vista não faz nada: quem pergunta é o quadro.
    #[doc(hidden)]
    pub fn logica(&mut self, ctx: &egui::Context) {
        let (fechar, escondida) =
            ctx.input(|i| (i.viewport().close_requested(), i.viewport().visible() == Some(false)));
        if !fechar || !escondida || self.pode_fechar || !self.algo_a_conferir_ao_fechar() {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.fechar_pendente = true;
        ctx.request_repaint();
    }

    /// Fechar a janela com uma exportação em andamento ou com a transação aberta pergunta antes. O pedido que chegou com a janela minimizada ou encoberta já foi cancelado pelo
    /// [`Editor::logica`], que a restaurou: ele é decidido no fim do primeiro quadro, por
    /// [`Editor::decidir_fechar_pendente`].
    pub(super) fn pedido_de_fechar(&mut self, ctx: &egui::Context) {
        let pedido = ctx.input(|i| i.viewport().close_requested());
        if self.pode_fechar || !pedido {
            return;
        }
        // um pedido novo vale pelo que estava pendente
        self.fechar_pendente = false;
        // os arquivos alterados primeiro (etapa 5): a fila pergunta um por um e, no fim, faz as perguntas de baixo
        if self.fila_ao_fechar_a_janela(ctx) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            return;
        }
        let exportando = self.exportando();
        let transacao = self.estado.transacao_aberta();
        if !exportando && !transacao {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        self.perguntar_ao_fechar(exportando, transacao);
    }

    /// O pedido de fechar que chegou com a janela minimizada ou encoberta, no fim do primeiro quadro da janela
    /// restaurada (depois do `aplicar`): as respostas que esperavam um quadro (o BEGIN de uma Consulta, a execução
    /// solta) já acertaram a transação. Com o motivo, a pergunta; sem ele (a troca de banco terminou, a Consulta não
    /// abriu transação), fecha, como a pessoa pediu.
    pub(super) fn decidir_fechar_pendente(&mut self, ctx: &egui::Context) {
        if !std::mem::take(&mut self.fechar_pendente) || self.pode_fechar {
            return;
        }
        if self.fila_ao_fechar_a_janela(ctx) {
            return;
        }
        let exportando = self.exportando();
        let transacao = self.estado.transacao_aberta();
        if !exportando && !transacao {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        self.perguntar_ao_fechar(exportando, transacao);
        ctx.request_repaint();
    }

    /// A pergunta antes de fechar: a da exportação (e, depois dela, a da transação) ou a da transação.
    fn perguntar_ao_fechar(&mut self, exportando: bool, transacao: bool) {
        if self.confirmacao.is_some() {
            return;
        }
        if exportando {
            // título próprio: a confirmação da janela Exportar ("Interromper a exportação") tem o id do título e
            // pode estar aberta ao mesmo tempo
            self.confirmacao = Some(
                Confirmacao::new(format!(
                    "Há uma exportação em andamento. Fechar o {NOME} a interrompe; nenhum arquivo é gravado."
                ))
                .titulo(format!("Fechar o {NOME}"))
                .rotulo("Interromper e fechar")
                .perigo(true),
            );
            self.confirmado = Some(Confirmado::FecharApp { falta_transacao: transacao });
        } else {
            self.perguntar_fechar_com_transacao();
        }
    }

    fn perguntar_fechar_com_transacao(&mut self) {
        self.confirmacao = Some(
            Confirmacao::new(frase_descartar(&format!("Fechar o {NOME}"), &self.nome_da_consulta_da_transacao()))
                .titulo("Transação aberta")
                .rotulo("Descartar e fechar")
                .perigo(true),
        );
        self.confirmado = Some(Confirmado::FecharApp { falta_transacao: false });
    }

    pub(super) fn mostrar_confirmacao(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>) {
        let Some(decisao) = confirmar_decisao(ctx, &mut self.confirmacao) else { return };
        let Some(confirmado) = self.confirmado.take() else { return };
        // as perguntas dos arquivos (etapa 5)
        match confirmado {
            Confirmado::SalvarAlterado(id) => {
                if let Some(f) = self.fila.as_mut() {
                    f.perguntando = None;
                    if decisao == Decisao::Descartou {
                        f.respondidas.insert(id);
                    }
                }
                match decisao {
                    // grava e espera: a fila anda quando a gravação terminar
                    Decisao::Confirmou => {
                        self.executar_na_aba(ctx, frame, id, Verbo::Salvar);
                        if let Some(f) = self.fila.as_mut() {
                            f.esperando = Some(id);
                        }
                    }
                    // segue sem gravar: a aba fecha com o destino
                    Decisao::Descartou => {}
                    Decisao::Cancelou => self.fila = None,
                }
                ctx.request_repaint();
                return;
            }
            Confirmado::Troca(..) | Confirmado::FecharApp { .. } => {}
        }
        // as outras não têm a segunda saída: só a ação vale
        let resposta = decisao == Decisao::Confirmou;
        if !resposta {
            // desistir da troca pedida pela janela "Pedir o caminho" fecha a janela também (lá, o `executar`
            // terminava sem erro e a janela fechava)
            if let Confirmado::Troca(_, Origem::Janela) = confirmado {
                self.janelas.caminho = None;
            }
            return;
        }
        match confirmado {
            Confirmado::Troca(troca, origem) => {
                self.iniciar_troca(ctx, troca, true, origem);
            }
            Confirmado::FecharApp { falta_transacao: true } => {
                self.janelas.exportar = None;
                self.perguntar_fechar_com_transacao();
            }
            Confirmado::FecharApp { falta_transacao: false } => {
                self.janelas.exportar = None;
                self.pode_fechar = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            // já tratadas acima
            Confirmado::SalvarAlterado(_) => {}
        }
    }
}
