//! As tarefas da casca: o estado que chega ao iniciar, as leituras e gravações em andamento ([`Tarefa`]) e o
//! que fazer quando cada uma termina ([`FimTarefa`]), também as respostas dos documentos que saíram.

use std::path::PathBuf;
use std::sync::Arc;

use banco::{BancoInfo, ObjetosInfo, ResultadoConsulta};
use comum::arquivos::{Listagem, Natureza};
use comum::{Codigo, Resultado};
use estado::{Acao, Contexto, FonteDeColunas, IdAba, Promessa, Rota};
use interface::pecas::toast::{toast_atencao, toast_erro};
use prefs::PrefsInfo;
use servico::{InfoArquivo, Servico};

use crate::visao_geral::{Execucao, Resultado as ResultadoDaTela, avisar_resultado};

use super::abrir::TrocaPendente;
use super::{
    Carga, DepoisDaPartida, Editor, Origem, Situacao, carregar_estado, erro_da_partida, erro_da_partida_arquivo,
    erro_da_partida_pasta,
};

/// Uma leitura da casca em andamento (com o id do banco para que foi pedida).
pub(super) enum Tarefa {
    Objetos(String, Promessa<ObjetosInfo>),
    /// A leitura do BancoInfo, com o número dela (ver [`Editor::pedir_banco`]).
    Banco(String, u64, Promessa<BancoInfo>),
    Prefs(Promessa<(PrefsInfo, Option<PathBuf>)>),
    /// As preferências que voltam de uma gravação; `limpou_historico`: a gravação limpou o histórico de consultas,
    /// que só vale reler quando ela terminar.
    Tema {
        promessa: Promessa<PrefsInfo>,
        limpou_historico: bool,
    },
    /// A gravação das larguras dos painéis (uma por vez, ver [`Editor::guardar_larguras`]).
    Larguras(Promessa<PrefsInfo>),
    Diretorio(Promessa<()>),
    /// A execução da Consulta SQL que ficou correndo quando a pessoa saiu da tela (com o id do banco).
    Consulta(String, Promessa<ResultadoConsulta>),
    /// A manutenção ou a cópia de segurança da Visão geral que ficou correndo quando a aba saiu (com o id do banco).
    VisaoGeral(String, Execucao),
    /// O "Mostrar na pasta" do toast da cópia de segurança e do Explorador, e o "Abrir no programa padrão" da ficha do
    /// arquivo (o Explorador de Arquivos, ou o programa, abre numa tarefa).
    Revelar(Promessa<()>),
    /// A leitura de uma pasta da árvore (etapa 4): a chave da pasta aberta quando foi pedida (a leitura de outra
    /// pasta é descartada) e a chave da pasta lida (a raiz ou uma subpasta).
    Pasta {
        raiz: String,
        no: String,
        /// O [`Editor::geracao_pasta`] de quando foi pedida: a leitura pedida antes de um Fechar pasta (ou da troca
        /// de pasta) não vale para a pasta aberta depois, mesmo que seja a mesma.
        geracao: u64,
        promessa: Promessa<Listagem>,
    },
    /// Abrir uma pasta (Abrir pasta, um recente, o caminho digitado, a linha de comando, soltar na janela): a
    /// listagem da raiz e as preferências de depois (o recente novo).
    AbrirPasta {
        origem: Origem,
        caminho: PathBuf,
        /// O [`Editor::geracao_pasta`] de quando foi pedida: um Fechar pasta no meio a desfaz.
        geracao: u64,
        promessa: Promessa<(Listagem, PrefsInfo)>,
    },
    /// O que um caminho é (`Acao::AbrirCaminho`), para virar a abertura da pasta, do banco ou do arquivo.
    Alvo {
        origem: Origem,
        caminho: PathBuf,
        somente_leitura: bool,
        promessa: Promessa<Natureza>,
    },
    /// Registrar o arquivo de dados que vai abrir (o recente), antes de abrir a ficha dele.
    Arquivo {
        origem: Origem,
        caminho: PathBuf,
        /// O [`Editor::geracao_pasta`] de quando foi pedido, se o arquivo é da pasta aberta: a ficha de um arquivo da
        /// pasta que fechou nesse meio-tempo não abre (o Fechar pasta fecha as fichas de dentro dela).
        da_pasta: Option<u64>,
        promessa: Promessa<(InfoArquivo, PrefsInfo)>,
    },
    /// Uma gravação sem resposta para a tela (apagar a pasta aberta das preferências no Fechar pasta).
    Gravar(Promessa<()>),
    /// O que foi solto na janela: quantos itens vieram e o escolhido (a primeira pasta, ou o primeiro item) com o que
    /// ele é.
    Soltar {
        quantos: usize,
        promessa: Promessa<(PathBuf, Resultado<Natureza>)>,
    },
}

/// O resultado de uma [`Tarefa`].
pub(super) enum FimTarefa {
    Objetos(String, Resultado<ObjetosInfo>),
    Banco(String, u64, Resultado<BancoInfo>),
    Prefs(Resultado<(PrefsInfo, Option<PathBuf>)>),
    Tema(Resultado<PrefsInfo>, bool),
    Larguras(Resultado<PrefsInfo>),
    Consulta(String, Resultado<ResultadoConsulta>),
    VisaoGeral(String, Resultado<(ResultadoDaTela, bool)>),
    Revelar(Resultado<()>),
    Pasta { raiz: String, no: String, geracao: u64, r: Resultado<Listagem> },
    AbrirPasta { origem: Origem, caminho: PathBuf, geracao: u64, r: Resultado<(Listagem, PrefsInfo)> },
    Alvo { origem: Origem, caminho: PathBuf, somente_leitura: bool, r: Resultado<Natureza> },
    Arquivo { origem: Origem, caminho: PathBuf, da_pasta: Option<u64>, r: Resultado<(InfoArquivo, PrefsInfo)> },
    Soltar { quantos: usize, caminho: PathBuf, r: Resultado<Natureza> },
    Nada,
}

impl Editor {
    pub(super) fn iniciar(&mut self, ctx: &egui::Context) {
        self.iniciado = true;
        let partida = self.partida.take().unwrap_or_default();
        let servico = Arc::clone(&self.servico);
        self.carga = Some(Promessa::rodar(ctx, move || {
            let mut erro_partida = None;
            let mut depois = None;
            if let Some(caminho) = &partida.caminho {
                match servico.alvo_do_caminho(caminho) {
                    // a pasta e o arquivo de dados abrem depois de o estado chegar (a casca os trata)
                    Ok(Natureza::Pasta) => depois = Some(DepoisDaPartida::Pasta(caminho.clone())),
                    Ok(Natureza::Dados(_)) => depois = Some(DepoisDaPartida::Arquivo(caminho.clone())),
                    // o banco, e o que não se sabe o que é: abre como banco, e o erro do SQLite diz por quê
                    Ok(Natureza::Banco) | Err(_) => {
                        if let Err(e) = servico.abrir_banco(caminho, partida.somente_leitura, false) {
                            log::warn!("Não foi possível abrir {} ao iniciar: {}", caminho.display(), e.mensagem);
                            erro_partida = Some(erro_da_partida(caminho, &e.mensagem));
                        }
                    }
                }
            }
            // a pasta que estava aberta volta (nunca o banco); a leitura dela é pedida sem esperar
            let pasta_aberta = servico.prefs().pasta_aberta().map(PathBuf::from);
            let (estado, aviso) = carregar_estado(&servico)?;
            Ok(Carga { estado, aviso, erro_partida, pasta_aberta, depois })
        }));
    }

    pub(super) fn tarefa<T: Send + 'static>(
        &self,
        ctx: &egui::Context,
        f: impl FnOnce(&Servico, &str) -> Resultado<T> + Send + 'static,
    ) -> Promessa<T> {
        let servico = Arc::clone(&self.servico);
        let id = self.estado.banco_id();
        Promessa::rodar(ctx, move || f(&servico, &id))
    }

    pub(super) fn receber(&mut self, ctx: &egui::Context) {
        // o estado inicial ou a ressincronização
        if let Some(r) = self.carga.as_mut().and_then(Promessa::tomar) {
            self.carga = None;
            let ressincronizando = self.ressincronizando.take();
            match r {
                Ok(carga) => {
                    let anterior = ressincronizando.clone().flatten();
                    // a execução que o documento refeito solta foi pedida ao banco de antes
                    let id_antes = self.estado.banco_id();
                    // etapa 4, passo 9: a pasta não é do banco e continua aberta
                    let pasta = self.estado.pasta.take();
                    self.estado = carga.estado;
                    self.estado.pasta = pasta;
                    self.falha_objetos = carga.aviso.clone();
                    self.mudou_estado(ctx);
                    if let Some(a) = carga.aviso {
                        toast_erro(ctx, a);
                    }
                    if !self.carregado {
                        self.carregado = true;
                        self.erro_inicial = carga.erro_partida;
                        let rota = if self.estado.tem_banco() { Rota::Banco } else { Rota::Inicio };
                        self.reiniciar_abas(rota, &id_antes);
                        // a pasta da linha de comando vale pela que estava aberta; a que estava aberta volta (a
                        // leitura da raiz é pedida agora, sem esperar)
                        match carga.depois {
                            Some(DepoisDaPartida::Pasta(c)) => self.abrir_pasta(ctx, c, Origem::Partida),
                            outro => {
                                if let Some(p) = carga.pasta_aberta {
                                    self.restaurar_pasta(ctx, p);
                                }
                                if let Some(DepoisDaPartida::Arquivo(c)) = outro {
                                    self.abrir_arquivo_de_dados(ctx, c, Origem::Partida);
                                }
                            }
                        }
                    } else if ressincronizando.is_some() {
                        // uma janela aberta no meio da releitura (no mesmo quadro do pedido que voltou
                        // `BancoTrocado`, por exemplo) também era do estado de antes
                        self.janelas.fechar_todas();
                        // o mesmo arquivo: as abas ficam, com os documentos refeitos; outro: a visão geral dele;
                        // nenhum: o Início
                        let mesmo = match (&anterior, &self.estado.banco) {
                            (Some(a), Some(b)) => prefs::chave_caminho(a) == prefs::chave_caminho(&b.caminho),
                            _ => false,
                        };
                        if mesmo {
                            self.refazer_documentos(&id_antes);
                        } else {
                            let rota = if self.estado.tem_banco() { Rota::Banco } else { Rota::Inicio };
                            self.reiniciar_abas(rota, &id_antes);
                        }
                    }
                }
                Err(e) => {
                    if !self.carregado {
                        self.carregado = true;
                        self.situacao = Situacao::Falha(e.mensagem);
                    } else {
                        toast_erro(ctx, e.mensagem);
                    }
                }
            }
        }

        // a troca de banco
        if let Some(r) = self.troca.as_mut().and_then(|t| t.promessa.tomar())
            && let Some(TrocaPendente { troca, origem, .. }) = self.troca.take()
        {
            self.fim_da_troca(ctx, troca, origem, r);
        }

        // as leituras menores
        let mut i = 0;
        while i < self.tarefas.len() {
            let fim = match &mut self.tarefas[i] {
                Tarefa::Objetos(id, p) => p.tomar().map(|r| FimTarefa::Objetos(id.clone(), r)),
                Tarefa::Banco(id, n, p) => p.tomar().map(|r| FimTarefa::Banco(id.clone(), *n, r)),
                Tarefa::Prefs(p) => p.tomar().map(FimTarefa::Prefs),
                Tarefa::Tema { promessa, limpou_historico } => {
                    let limpou = *limpou_historico;
                    promessa.tomar().map(|r| FimTarefa::Tema(r, limpou))
                }
                Tarefa::Larguras(p) => p.tomar().map(FimTarefa::Larguras),
                Tarefa::Diretorio(p) => p.tomar().map(|_| FimTarefa::Nada),
                Tarefa::Consulta(id, p) => p.tomar().map(|r| FimTarefa::Consulta(id.clone(), r)),
                Tarefa::VisaoGeral(id, e) => e.tomar_solta().map(|r| FimTarefa::VisaoGeral(id.clone(), r)),
                Tarefa::Revelar(p) => p.tomar().map(FimTarefa::Revelar),
                Tarefa::Pasta { raiz, no, geracao, promessa } => promessa.tomar().map(|r| FimTarefa::Pasta {
                    raiz: raiz.clone(),
                    no: no.clone(),
                    geracao: *geracao,
                    r,
                }),
                Tarefa::AbrirPasta { origem, caminho, geracao, promessa } => promessa
                    .tomar()
                    .map(|r| FimTarefa::AbrirPasta { origem: *origem, caminho: caminho.clone(), geracao: *geracao, r }),
                Tarefa::Alvo { origem, caminho, somente_leitura, promessa } => promessa.tomar().map(|r| {
                    FimTarefa::Alvo { origem: *origem, caminho: caminho.clone(), somente_leitura: *somente_leitura, r }
                }),
                Tarefa::Arquivo { origem, caminho, da_pasta, promessa } => promessa
                    .tomar()
                    .map(|r| FimTarefa::Arquivo { origem: *origem, caminho: caminho.clone(), da_pasta: *da_pasta, r }),
                Tarefa::Gravar(p) => p.tomar().map(|_| FimTarefa::Nada),
                Tarefa::Soltar { quantos, promessa } => {
                    let quantos = *quantos;
                    promessa.tomar().map(|r| match r {
                        Ok((caminho, r)) => FimTarefa::Soltar { quantos, caminho, r },
                        Err(e) => FimTarefa::Soltar { quantos, caminho: PathBuf::new(), r: Err(e) },
                    })
                }
            };
            match fim {
                Some(fim) => {
                    self.tarefas.remove(i);
                    self.fim_da_tarefa(ctx, fim);
                }
                None => i += 1,
            }
        }
    }

    /// As abas fora de vista não são desenhadas; elas recebem só o `em_segundo_plano` do documento: as Consultas SQL
    /// escondidas tomam as respostas que chegam (a execução que segue com a aba escondida avisa a transação e os
    /// objetos na hora). Parado, nada pede quadro.
    pub(super) fn documentos_em_segundo_plano(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) {
        if !matches!(self.situacao, Situacao::Pronta) {
            return;
        }
        let ativa = self.abas.id_ativa();
        let (versao, trocando) = (self.versao, self.troca.is_some());
        let Editor { servico, estado, abas, .. } = self;
        for aba in abas.iter_mut().filter(|a| a.id() != ativa) {
            let mut emitidas = Vec::new();
            let mut contexto =
                Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando).com_fontes(fontes);
            aba.conteudo.em_segundo_plano(ctx, &mut contexto);
            let id = aba.id();
            acoes.extend(emitidas.into_iter().map(|a| (Some(id), a)));
        }
    }

    pub(super) fn fim_da_tarefa(&mut self, ctx: &egui::Context, fim: FimTarefa) {
        match fim {
            // a resposta de um pedido feito para um banco que já não é o aberto não vale mais (nem o erro)
            FimTarefa::Objetos(id, r) => match r {
                _ if id != self.estado.banco_id() => {}
                Ok(o) => {
                    self.falha_objetos = None;
                    self.trocar_objetos(ctx, o);
                    self.mudou_estado(ctx);
                }
                // releitura em silêncio: o Explorador fica
                // com a lista anterior; só o banco trocado ressincroniza. Sem lista nenhuma (a primeira leitura
                // falhou), o Explorador mostra o motivo e o "Tentar de novo": sem isso ficaria em "Carregando…" para
                // sempre, repintando
                Err(e) if e.tem_codigo(Codigo::BancoTrocado) => self.ressincronizar(ctx),
                Err(e) => {
                    log::warn!("Não foi possível reler os objetos do banco: {e}");
                    if self.estado.objetos.is_none() {
                        self.falha_objetos = Some(e.mensagem);
                        self.mudou_estado(ctx);
                    }
                }
            },
            FimTarefa::Banco(id, numero, r) => match r {
                _ if id != self.estado.banco_id() => {}
                // uma leitura mais velha que outra já aplicada (voltaram fora de ordem) não desfaz a mais nova
                Ok(_) if numero < self.banco_aplicada => {}
                Ok(mut b) if b.id == id => {
                    self.banco_aplicada = numero;
                    // pedida antes da última mudança de transação: o resto vale, a transação é a que a nota disse
                    if numero < self.banco_nota
                        && let Some(atual) = self.estado.banco.as_ref()
                    {
                        b.transacao_aberta = atual.transacao_aberta;
                    }
                    self.estado.banco = Some(b);
                    self.mudou_estado(ctx);
                }
                Ok(_) => {}
                Err(e) => self.erro_da_casca(ctx, &e),
            },
            FimTarefa::Prefs(r) => match r {
                Ok((prefs, ultimo)) => {
                    self.estado.prefs = prefs;
                    if ultimo.is_some() {
                        self.estado.ultimo_diretorio = ultimo;
                    }
                    self.mudou_estado(ctx);
                }
                Err(e) => self.erro_da_casca(ctx, &e),
            },
            FimTarefa::Tema(r, limpou_historico) => {
                // o histórico foi limpo no arquivo só agora: a lista à vista (ou a próxima leitura) é de antes
                if limpou_historico {
                    self.casca.historico.marcar_velha();
                    ctx.request_repaint();
                }
                match r {
                    Ok(prefs) => {
                        self.estado.prefs = prefs;
                        self.mudou_estado(ctx);
                    }
                    Err(e) => toast_erro(ctx, e.mensagem),
                }
            }
            // a manutenção ou a cópia que ficou correndo sem a aba: o resultado, num toast. O erro do banco que já
            // não é o aberto não vale; o sucesso (o arquivo da cópia existe) vale sempre
            FimTarefa::VisaoGeral(id, r) => match r {
                Ok((r, escreveu)) => {
                    avisar_resultado(ctx, &r);
                    if escreveu && id == self.estado.banco_id() {
                        self.pendentes.push((None, Acao::RecarregarBanco));
                    }
                    // a cópia de segurança criou um arquivo, talvez na pasta aberta: a árvore relê
                    if matches!(r, ResultadoDaTela::Copia { .. }) {
                        self.reler_a_pasta(ctx);
                    }
                    ctx.request_repaint();
                }
                Err(_) if id != self.estado.banco_id() => {}
                Err(e) => self.erro_da_casca(ctx, &e),
            },
            // a gravação das larguras terminou: se a pessoa mexeu de novo enquanto ela corria, a próxima sai agora
            FimTarefa::Larguras(r) => {
                self.gravando_larguras = false;
                match r {
                    Ok(prefs) => {
                        self.estado.prefs = prefs;
                        self.mudou_estado(ctx);
                        self.guardar_larguras(ctx);
                    }
                    Err(e) => toast_erro(ctx, e.mensagem),
                }
            }
            // a execução que ficou correndo: a transação e, se o comando mudou dados ou estrutura (ou fechou a
            // transação: um COMMIT ou um ROLLBACK), os objetos e as abas; uma falha não aparece fora da tela (lá também
            // não), só o banco trocado ressincroniza
            FimTarefa::Consulta(id, r) => match r {
                // a resposta é do banco de antes de uma troca: não vale para o aberto agora
                _ if id != self.estado.banco_id() => {}
                Ok(r) => {
                    let fechou_a_transacao = self.estado.transacao_aberta() && !r.transacao_aberta;
                    self.pendentes.push((None, Acao::NotarTransacao(r.transacao_aberta)));
                    if r.alterou_dados || r.alterou_esquema || fechou_a_transacao {
                        self.pendentes.push((None, Acao::RecarregarObjetos));
                    }
                    ctx.request_repaint();
                }
                Err(e) if e.tem_codigo(Codigo::BancoTrocado) => self.ressincronizar(ctx),
                Err(_) => {}
            },
            FimTarefa::Revelar(r) => {
                if let Err(e) = r {
                    self.erro_da_casca(ctx, &e);
                }
            }
            FimTarefa::Pasta { raiz, no, geracao, r } => {
                if geracao == self.geracao_pasta {
                    self.chegou_a_pasta(ctx, &raiz, no, r);
                }
            }
            // a pasta pedida antes de um Fechar pasta: o fechar vale (a pessoa fechou depois de pedir), e a chave
            // `pasta_aberta` que a abertura gravou sai de novo
            FimTarefa::AbrirPasta { origem, geracao, r: Ok(_), .. } if geracao != self.geracao_pasta => {
                if origem == Origem::Janela {
                    self.janelas.caminho = None;
                }
                let promessa = self.tarefa(ctx, |s, _| {
                    s.fechar_pasta();
                    Ok(())
                });
                self.tarefas.push(Tarefa::Gravar(promessa));
            }
            FimTarefa::AbrirPasta { origem, caminho, r, .. } => match r {
                Ok((listagem, prefs)) => {
                    self.estado.prefs = prefs;
                    self.por_a_pasta(listagem);
                    if origem == Origem::Janela {
                        self.janelas.caminho = None;
                    }
                    self.mudou_estado(ctx);
                }
                Err(e) => {
                    let mensagem = if origem == Origem::Partida {
                        erro_da_partida_pasta(&caminho, &e.mensagem)
                    } else {
                        e.mensagem
                    };
                    self.avisar_erro(ctx, origem, mensagem);
                }
            },
            FimTarefa::Alvo { origem, caminho, somente_leitura, r } => {
                self.abrir_pelo_alvo(ctx, caminho, somente_leitura, origem, r)
            }
            FimTarefa::Soltar { quantos, caminho, r } => {
                if quantos > 1 && r.is_ok() {
                    let nome = caminho
                        .file_name()
                        .map_or_else(|| caminho.display().to_string(), |n| n.to_string_lossy().into_owned());
                    toast_atencao(ctx, format!("Só um item abre por vez; abri {nome}."));
                }
                self.abrir_pelo_alvo(ctx, caminho, false, Origem::Casca, r)
            }
            FimTarefa::Arquivo { origem, caminho, da_pasta, r } => match r {
                Ok((info, prefs)) => {
                    self.estado.prefs = prefs;
                    if origem == Origem::Janela {
                        self.janelas.caminho = None;
                    }
                    self.mudou_estado(ctx);
                    // o arquivo era da pasta que fechou enquanto ele era registrado: a ficha não abre (o Fechar pasta
                    // fechou as de dentro dela); o recente fica
                    let pasta_fechou = da_pasta.is_some_and(|g| g != self.geracao_pasta)
                        && !self.estado.pasta.as_ref().is_some_and(|p| p.contem(&info.caminho));
                    if !pasta_fechou {
                        self.navegar(Rota::Arquivo { caminho: info.caminho });
                    }
                }
                Err(e) => {
                    let mensagem = if origem == Origem::Partida {
                        erro_da_partida_arquivo(&caminho, &e.mensagem)
                    } else {
                        e.mensagem
                    };
                    self.avisar_erro(ctx, origem, mensagem);
                }
            },
            FimTarefa::Nada => {}
        }
    }
}
