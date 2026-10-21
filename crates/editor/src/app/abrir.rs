//! Abrir e trocar: a pasta, o banco e o arquivo de dados pelo caminho (o que ele é sai do serviço numa tarefa),
//! pelos diálogos (ou, sem eles, pela janela "Pedir o caminho"), soltos na janela; a troca de banco com o card
//! da troca que demora; a ressincronização quando outro lugar trocou o banco.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use comum::arquivos::{Natureza, e_extensao_de_banco};
use comum::{Codigo, ErroBanco, Resultado};
use estado::{ChaveAba, Promessa, Rota};
use interface::pecas::janela::Confirmacao;
use interface::pecas::toast::{toast_atencao, toast_erro};

use crate::documento::Documento;
use crate::pedir_caminho::{PedirCaminho, Proposito, SaidaCaminho};
use crate::recursos::{PedidoDialogo, RespostaDialogo};

use super::tarefas::Tarefa;
use super::{Carga, Confirmado, Editor, Origem, carregar_estado, frase_descartar};

/// Uma troca de banco mais rápida que isto não mostra nada (sem piscar o card).
pub(super) const ESPERA_PARA_MOSTRAR: f64 = 0.3;
/// Depois disto, o card da troca diz que ela espera o banco atual terminar o que está fazendo.
pub(super) const ESPERA_LONGA: f64 = 2.0;

/// Uma troca de banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Troca {
    Abrir { caminho: PathBuf, somente_leitura: bool },
    Novo { caminho: PathBuf },
    Fechar,
}

impl Troca {
    /// O começo da frase da confirmação de descartar a transação.
    fn acao(&self) -> &'static str {
        match self {
            Troca::Abrir { .. } => "Abrir outro banco",
            Troca::Novo { .. } => "Criar um banco novo",
            Troca::Fechar => "Fechar o banco",
        }
    }

    /// O card da troca que está demorando: logo no começo e depois de [`ESPERA_LONGA`].
    pub(super) fn em_andamento(&self, longa: bool) -> &'static str {
        match (self, longa) {
            (Troca::Abrir { .. }, false) => "Abrindo o banco…",
            (Troca::Novo { .. }, false) => "Criando o banco…",
            (Troca::Fechar, false) => "Fechando o banco…",
            (Troca::Abrir { .. }, true) => MSG_ABRINDO_ESPERANDO,
            (Troca::Novo { .. }, true) => MSG_CRIANDO_ESPERANDO,
            (Troca::Fechar, true) => MSG_FECHANDO_ESPERANDO,
        }
    }
}

/// Os textos do card da troca que passou de [`ESPERA_LONGA`] (a conexão está presa numa operação longa).
pub(crate) const MSG_ABRINDO_ESPERANDO: &str =
    "Abrindo o banco. Aguardando o banco atual terminar a operação em andamento.";
pub(crate) const MSG_CRIANDO_ESPERANDO: &str =
    "Criando o banco. Aguardando o banco atual terminar a operação em andamento.";
pub(crate) const MSG_FECHANDO_ESPERANDO: &str = "Fechando o banco. Aguardando a operação em andamento terminar.";

/// O caminho tem uma extensão de banco (`.db`, `.sqlite`…): pedido pelo nome, ele abre como banco mesmo quando a
/// classificação não o reconhece, e o erro do SQLite diz por quê ("O arquivo não é um banco SQLite…").
pub(super) fn tem_extensao_de_banco(caminho: &Path) -> bool {
    caminho.extension().and_then(|e| e.to_str()).is_some_and(e_extensao_de_banco)
}

pub(super) struct TrocaPendente {
    pub(super) troca: Troca,
    pub(super) origem: Origem,
    pub(super) promessa: Promessa<Carga>,
    /// Quando começou (o `input.time`), para o card da troca que demora.
    pub(super) desde: f64,
}

impl Editor {
    /// O `Acao::AbrirCaminho`: o serviço diz o que o caminho é (numa tarefa), e ele abre como pasta, banco ou arquivo
    /// de dados ([`FimTarefa::Alvo`]).
    pub(super) fn abrir_caminho(
        &mut self,
        ctx: &egui::Context,
        caminho: PathBuf,
        somente_leitura: bool,
        origem: Origem,
    ) {
        if !self.carregado {
            return;
        }
        let c = caminho.clone();
        let promessa = self.tarefa(ctx, move |s, _| s.alvo_do_caminho(&c));
        self.tarefas.push(Tarefa::Alvo { origem, caminho, somente_leitura, promessa });
    }

    /// O caminho classificado abre pelo que ele é: a pasta, a ficha do arquivo de dados ou o banco.
    pub(super) fn abrir_pelo_alvo(
        &mut self,
        ctx: &egui::Context,
        caminho: PathBuf,
        somente_leitura: bool,
        origem: Origem,
        r: Resultado<Natureza>,
    ) {
        match r {
            Ok(Natureza::Pasta) => self.abrir_pasta(ctx, caminho, origem),
            Ok(Natureza::Dados(_)) => self.abrir_arquivo_de_dados(ctx, caminho, origem),
            Ok(Natureza::Banco) => self.abrir_banco_de(ctx, caminho, somente_leitura, origem),
            // um banco pedido pelo nome que não abre: o erro do SQLite (ou o do arquivo que não existe) diz por quê
            Err(_) if tem_extensao_de_banco(&caminho) => self.abrir_banco_de(ctx, caminho, somente_leitura, origem),
            Err(e) => self.avisar_erro(ctx, origem, e.mensagem),
        }
    }

    /// Soltar arquivos ou pastas na janela (decisão 18, sem o vidro "Solte para abrir", que não tem prancheta): abre
    /// um item só, a primeira pasta ou, sem pasta, o primeiro que o app abre, como o caminho digitado (`AbrirCaminho`); com mais de
    /// um, o aviso "Só um item abre por vez; abri dados.". Com uma janela ou um modal aberto, nada abre. O que cada
    /// item é sai do serviço numa tarefa (a thread da interface não toca no disco).
    pub(super) fn soltar_na_janela(&mut self, ctx: &egui::Context) {
        let soltos: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|c| !c.as_os_str().is_empty()).collect()
        });
        if soltos.is_empty() || !self.carregado || self.janela_aberta() {
            return;
        }
        if ctx.memory(|m| m.top_modal_layer().is_some()) {
            return;
        }
        let quantos = soltos.len();
        let servico = Arc::clone(&self.servico);
        let promessa = Promessa::rodar(ctx, move || {
            let alvos: Vec<(PathBuf, Resultado<Natureza>)> =
                soltos.into_iter().map(|c| (c.clone(), servico.alvo_do_caminho(&c))).collect();
            // a primeira pasta; sem pasta, o primeiro que o app abre (um PDF solto junto não esconde o CSV); sem
            // nenhum, o primeiro, com o erro dele
            let i = alvos
                .iter()
                .position(|(_, r)| matches!(r, Ok(Natureza::Pasta)))
                .or_else(|| alvos.iter().position(|(_, r)| r.is_ok()))
                .unwrap_or(0);
            alvos.into_iter().nth(i).ok_or_else(|| ErroBanco::novo("Nada foi solto na janela."))
        });
        self.tarefas.push(Tarefa::Soltar { quantos, promessa });
    }

    /// O `Acao::AbrirArquivoDeDados`: registra o recente (numa tarefa) e abre a ficha do arquivo (`Rota::Arquivo`),
    /// ou foca a que já está aberta.
    pub(super) fn abrir_arquivo_de_dados(&mut self, ctx: &egui::Context, caminho: PathBuf, origem: Origem) {
        let c = caminho.clone();
        let promessa = self.tarefa(ctx, move |s, _| {
            let info = s.registrar_arquivo(&c)?;
            Ok((info, s.prefs().resumo()))
        });
        let da_pasta = self.estado.pasta.as_ref().filter(|p| p.contem(&caminho)).map(|_| self.geracao_pasta);
        self.tarefas.push(Tarefa::Arquivo { origem, caminho, da_pasta, promessa });
    }

    /// Abre um banco (a troca de sempre); a janela "Pedir o caminho" volta a aceitar o botão se a troca não começou.
    fn abrir_banco_de(&mut self, ctx: &egui::Context, caminho: PathBuf, somente_leitura: bool, origem: Origem) {
        if !self.iniciar_troca(ctx, Troca::Abrir { caminho, somente_leitura }, false, origem)
            && origem == Origem::Janela
            && let Some(j) = self.janelas.caminho.as_mut()
        {
            j.desocupar();
        }
    }

    /// O diálogo "Abrir pasta" (ou, sem ele, a janela "Pedir o caminho" com o propósito Pasta). A pasta de cima da
    /// escolhida vira a última usada, para o próximo diálogo abrir ao lado dela.
    pub(super) fn escolher_e_abrir_pasta(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        origem: Origem,
    ) {
        if !self.carregado {
            return;
        }
        let pasta = self.estado.ultimo_diretorio.clone();
        let escolha = match self.dialogos.abrir_pasta(frame, pasta) {
            RespostaDialogo::Escolhido(escolha) => escolha,
            RespostaDialogo::Cancelado => return,
            RespostaDialogo::Indisponivel => {
                self.janelas.caminho = Some(PedirCaminho::pasta());
                return;
            }
        };
        if let Some(de_cima) = escolha.pasta {
            self.estado.ultimo_diretorio = Some(de_cima.clone());
            let p = self.tarefa(ctx, move |s, _| {
                s.lembrar_diretorio(&de_cima);
                Ok(())
            });
            self.tarefas.push(Tarefa::Diretorio(p));
        }
        self.abrir_pasta(ctx, escolha.caminho, origem);
    }

    /// Começa uma troca de banco; false quando já há uma em andamento (ou o estado ainda não chegou).
    pub(super) fn iniciar_troca(&mut self, ctx: &egui::Context, troca: Troca, descartar: bool, origem: Origem) -> bool {
        // uma troca de cada vez: não enfileira outra igual enquanto a primeira espera (o card no alto do
        // conteúdo mostra que ela está esperando)
        if self.troca.is_some() || !self.carregado {
            return false;
        }
        let servico = Arc::clone(&self.servico);
        let id = self.estado.banco_id();
        let t = troca.clone();
        let promessa = Promessa::rodar(ctx, move || {
            match &t {
                Troca::Abrir { caminho, somente_leitura } => {
                    servico.abrir_banco(caminho, *somente_leitura, descartar)?;
                }
                Troca::Novo { caminho } => {
                    servico.novo_banco(caminho, descartar)?;
                }
                Troca::Fechar => servico.fechar_banco(&id, descartar)?,
            }
            let (estado, aviso) = carregar_estado(&servico)?;
            Ok(Carga::de(estado, aviso))
        });
        let desde = ctx.input(|i| i.time);
        self.troca = Some(TrocaPendente { troca, origem, promessa, desde });
        // o card da troca aparece se ela demorar (a promessa acorda a interface quando terminar)
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(ESPERA_PARA_MOSTRAR));
        true
    }

    pub(super) fn fim_da_troca(&mut self, ctx: &egui::Context, troca: Troca, origem: Origem, r: Resultado<Carga>) {
        match r {
            Ok(carga) => {
                self.janelas.fechar_todas();
                // uma releitura pedida durante a troca (um pedido que voltou `BancoTrocado` antes de ela
                // terminar) chegaria com o estado de antes e puxaria a tela de volta: a troca já trouxe o novo
                if self.ressincronizando.is_some() {
                    self.carga = None;
                    self.ressincronizando = None;
                }
                // a execução da Consulta SQL que ainda corre foi pedida ao banco de antes: a resposta dela não vale
                // para o novo
                let id_antes = self.estado.banco_id();
                // etapa 4, passo 9: a pasta não é do banco e continua aberta (o passo 8 a lê e a relê)
                let pasta = self.estado.pasta.take();
                self.estado = carga.estado;
                self.estado.pasta = pasta;
                self.falha_objetos = carga.aviso.clone();
                self.mudou_estado(ctx);
                if let Some(a) = carga.aviso {
                    toast_erro(ctx, a);
                }
                // as rotas antigas eram de outro banco
                let rota = if self.estado.tem_banco() { Rota::Banco } else { Rota::Inicio };
                self.reiniciar_abas(rota, &id_antes);
                // o Novo banco cria um arquivo na pasta: a árvore relê
                self.reler_a_pasta(ctx);
            }
            Err(e) if e.tem_codigo(Codigo::TransacaoAberta) => {
                self.confirmacao = Some(
                    Confirmacao::new(frase_descartar(troca.acao(), &self.nome_da_consulta_da_transacao()))
                        .titulo("Transação aberta")
                        .rotulo("Descartar e continuar")
                        .perigo(true),
                );
                self.confirmado = Some(Confirmado::Troca(troca, origem));
            }
            Err(e) if e.tem_codigo(Codigo::BancoTrocado) => self.ressincronizar(ctx),
            // o erro vai para quem pediu: a janela "Pedir o caminho" ou o Início (mesmo fora de vista); o resto é
            // toast
            Err(e) => self.avisar_erro(ctx, origem, e.mensagem),
        }
    }

    /// Outro lugar trocou o banco (um pedido voltou `BancoTrocado`): relê o estado, fecha as janelas e redesenha.
    /// Durante uma troca pedida aqui, não faz nada: os pedidos com o id antigo voltam `BancoTrocado` porque o
    /// serviço já trocou, e a própria troca traz o estado novo (sem o toast de "outra janela").
    pub(super) fn ressincronizar(&mut self, ctx: &egui::Context) {
        if self.carga.is_some() || self.troca.is_some() {
            return;
        }
        let anterior = self.estado.banco.as_ref().map(|b| b.caminho.clone());
        self.janelas.fechar_todas();
        self.confirmacao = None;
        self.confirmado = None;
        toast_atencao(ctx, servico::MSG_BANCO_TROCADO);
        let servico = Arc::clone(&self.servico);
        self.ressincronizando = Some(anterior);
        self.carga = Some(Promessa::rodar(ctx, move || {
            let (estado, aviso) = carregar_estado(&servico)?;
            Ok(Carga::de(estado, aviso))
        }));
    }

    pub(super) fn escolher_e_abrir(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        so_leitura: bool,
        origem: Origem,
    ) {
        if self.troca.is_some() || !self.carregado {
            return;
        }
        let pasta = self.estado.ultimo_diretorio.clone();
        let escolha = match self.dialogos.abrir_banco(frame, pasta) {
            RespostaDialogo::Escolhido(escolha) => escolha,
            RespostaDialogo::Cancelado => return,
            RespostaDialogo::Indisponivel => {
                self.janelas.caminho = Some(PedirCaminho::abrir(so_leitura));
                return;
            }
        };
        // a pasta do arquivo escolhido vira a última usada
        if let Some(p) = &escolha.pasta {
            self.estado.ultimo_diretorio = Some(p.clone());
        }
        let caminho = escolha.caminho.clone();
        let servico = Arc::clone(&self.servico);
        self.tarefas.push(Tarefa::Diretorio(Promessa::rodar(ctx, move || {
            servico.lembrar_diretorio(&caminho);
            Ok(())
        })));
        // o "Abrir arquivo" mostra também os arquivos de dados: o escolhido abre pelo que ele é
        self.abrir_caminho(ctx, escolha.caminho, so_leitura, origem);
    }

    pub(super) fn novo_banco(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, origem: Origem) {
        if self.troca.is_some() || !self.carregado {
            return;
        }
        let pasta_banco = self.estado.banco.as_ref().map(|b| b.pasta.clone());
        let pasta = pasta_banco.clone().or_else(|| self.estado.ultimo_diretorio.clone());
        match self.dialogos.salvar_banco(frame, pasta, PedidoDialogo::NovoBanco, "novo.db") {
            RespostaDialogo::Escolhido(escolha) => {
                self.iniciar_troca(ctx, Troca::Novo { caminho: escolha.caminho }, false, origem);
            }
            RespostaDialogo::Cancelado => {}
            RespostaDialogo::Indisponivel => {
                self.janelas.caminho = Some(PedirCaminho::novo(pasta_banco.as_deref()));
            }
        }
    }

    /// O que a janela "Pedir o caminho" devolveu.
    pub(super) fn caminho_digitado(&mut self, ctx: &egui::Context, saida: SaidaCaminho) {
        let Some(proposito) = self.janelas.caminho.as_ref().map(|j| j.proposito) else { return };
        let caminho = match saida {
            SaidaCaminho::Fechou => {
                self.janelas.caminho = None;
                return;
            }
            SaidaCaminho::Enviou(caminho) => caminho,
        };
        let troca = match proposito {
            // o caminho digitado abre pelo que ele é, como o do Início; o erro aparece na janela
            Proposito::Abrir { somente_leitura } => {
                self.abrir_caminho(ctx, caminho, somente_leitura, Origem::Janela);
                return;
            }
            Proposito::Pasta => {
                self.abrir_pasta(ctx, caminho, Origem::Janela);
                return;
            }
            Proposito::Novo => Troca::Novo { caminho },
            Proposito::Copia => {
                // o caminho volta à Visão geral, que faz a cópia (e pergunta antes de substituir); a janela é modal,
                // e nada fecha nem troca a aba no meio
                self.janelas.caminho = None;
                if let Some(Documento::Banco(t)) = self.documento_da_chave(&ChaveAba::VisaoGeral) {
                    t.copiar_para(caminho, false);
                    ctx.request_repaint();
                }
                return;
            }
        };
        if !self.iniciar_troca(ctx, troca, false, Origem::Janela)
            && let Some(j) = self.janelas.caminho.as_mut()
        {
            j.desocupar();
        }
    }

    /// A cópia de segurança pedida pela Visão geral (`Acao::EscolherCopia`): o diálogo "Salvar banco SQLite" na pasta
    /// do banco; o caminho escolhido volta à Visão geral (o Windows já perguntou antes de substituir). Sem o diálogo
    /// nativo, a janela "Pedir o caminho" ("Salvar como"), cujo caminho volta pelo [`Editor::caminho_digitado`].
    pub(super) fn escolher_copia(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, nome: String) {
        let Some(pasta) = self.estado.banco.as_ref().map(|b| b.pasta.clone()) else { return };
        let pedido = PedidoDialogo::CopiaDeSeguranca { nome_sugerido: nome.clone() };
        match self.dialogos.salvar_banco(frame, Some(pasta.clone()), pedido, &nome) {
            RespostaDialogo::Escolhido(escolha) => {
                if let Some(Documento::Banco(t)) = self.documento_da_chave(&ChaveAba::VisaoGeral) {
                    t.copiar_para(escolha.caminho, true);
                    ctx.request_repaint();
                }
            }
            RespostaDialogo::Cancelado => {}
            RespostaDialogo::Indisponivel => self.janelas.caminho = Some(PedirCaminho::copia(&pasta, &nome)),
        }
    }
}
