//! O despacho: as [`Acao`]s do quadro (`aplicar`), os itens do menu Tabela e os pedidos à Visão geral, os
//! verbos executados numa aba fora da barra (o Salvar da fila ao fechar) e os atalhos de teclado da casca.

use std::path::PathBuf;

use egui::Ui;
use estado::{Acao, ChaveAba, Contexto, IdAba, Ofertas, Rota, Verbo};
use interface::tema;

use crate::casca;
use crate::casca::docas::{Lugar, Painel};
use crate::casca::ferramentas::EstadoPainel;
use crate::casca::menus::ItemTabela;
use crate::casca::preferencias::Secao;
use crate::documento::Documento;
use crate::telas::{JanelaExportar, JanelaNovaTabela, JanelaNovoIndice};
use crate::visao_geral::PedidoDoMenu;

use super::abrir::Troca;
use super::preferencias::tema_da_interface;
use super::tarefas::Tarefa;
use super::{Editor, Origem, Situacao, terminar_edicao};

/// O toast do Tabela › Copiar o nome.
pub(crate) const MSG_NOME_COPIADO: &str = "Nome copiado.";

impl Editor {
    /// Um item do menu Tabela, para a tabela à vista (revisão de UI de 19/10/2026): as vistas e os pedidos à Estrutura
    /// põem as Propriedades à vista; o resto vira a ação de sempre.
    pub(super) fn item_da_tabela(&mut self, ctx: &egui::Context, item: ItemTabela) {
        let Documento::Tabela(t) = &mut self.abas.ativa_mut().conteudo else { return };
        let nome = t.nome().to_string();
        match item {
            ItemTabela::Vista(parte) => {
                t.mostrar_parte(parte);
                self.paineis.mostrar(Painel::Propriedades);
            }
            ItemTabela::NovaColuna | ItemTabela::Renomear | ItemTabela::Excluir => {
                t.pedir(match item {
                    ItemTabela::NovaColuna => estado::PedidoTabela::NovaColuna,
                    ItemTabela::Renomear => estado::PedidoTabela::Renomear,
                    _ => estado::PedidoTabela::Excluir,
                });
                self.paineis.mostrar(Painel::Propriedades);
            }
            ItemTabela::AbrirNaConsulta => {
                let [sql, _] = estado::consultas_da_tabela(&nome);
                self.pendentes.push((None, Acao::Navegar(Rota::consulta_com(sql))));
            }
            ItemTabela::Exportar => {
                if let Some(pedido) = t.pedido_exportacao() {
                    self.pendentes.push((None, Acao::AbrirExportacao(pedido)));
                }
            }
            ItemTabela::CopiarNome => {
                ctx.copy_text(nome);
                interface::pecas::toast::toast_concluido(ctx, MSG_NOME_COPIADO);
            }
            ItemTabela::PropriedadesEmJanela => self.mover_painel(ctx, Painel::Propriedades, Lugar::Janela),
            ItemTabela::NovoIndice => {
                self.pendentes.push((None, Acao::AbrirNovoIndice { tabela: nome, colunas: Vec::new() }));
            }
        }
        ctx.request_repaint();
    }

    /// Põe a Visão geral à vista e passa a ela o pedido do menu Banco (sem banco, nada).
    fn pedir_a_visao_geral(&mut self, pedido: PedidoDoMenu) {
        if !self.estado.tem_banco() {
            return;
        }
        self.navegar(Rota::Banco);
        if let Documento::Banco(t) = &mut self.abas.ativa_mut().conteudo {
            t.pedir(pedido);
        }
    }

    /// Executa um verbo no documento da aba `id` fora do caminho da barra (o Salvar da fila ao fechar); o que ele pede
    /// à casca vale no fim do próximo quadro, marcado com a aba.
    pub(super) fn executar_na_aba(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        id: IdAba,
        verbo: Verbo,
    ) {
        let fontes = self.fontes_de_colunas();
        let (versao, trocando) = (self.versao, self.troca.is_some());
        let Editor { servico, estado, abas, pendentes, .. } = self;
        let Some(aba) = abas.get_mut(id) else { return };
        let mut emitidas = Vec::new();
        let mut contexto =
            Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando).com_fontes(&fontes);
        aba.conteudo.executar(verbo, ctx, &mut contexto);
        pendentes.extend(emitidas.into_iter().map(|a| (Some(id), a)));
        ctx.request_repaint();
    }

    /// Quem pediu a troca de banco: o Início (pela marca da ação: o erro volta ao card "Abrir um banco") ou a casca.
    fn origem_da_troca(&self, origem: Option<IdAba>) -> Origem {
        let do_inicio = origem.and_then(|id| self.abas.get(id)).is_some_and(|a| a.chave() == Some(&ChaveAba::Inicio));
        if do_inicio { Origem::Inicio } else { Origem::Casca }
    }

    /// Aplica as ações do quadro, cada uma com a aba que a emitiu (None: a casca, as janelas, os testes).
    pub(super) fn aplicar(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        acoes: Vec<(Option<IdAba>, Acao)>,
    ) {
        for (origem, acao) in acoes {
            match acao {
                Acao::Navegar(rota) => self.navegar(rota),
                // o endereço vai para a aba que o emitiu, mesmo que ela tenha saído de vista no mesmo quadro (o
                // adiamento da busca ou do texto da Consulta vencendo no quadro do clique em outra aba)
                Acao::Atualizar(rota) => self.trocar_endereco(origem, rota, true),
                Acao::Substituir(rota) => self.trocar_endereco(origem, rota, false),
                Acao::Voltar => {
                    self.abas.voltar();
                }
                Acao::Avancar => {
                    self.abas.avancar();
                }
                Acao::NovaConsulta { sql } => self.nova_consulta(sql),
                Acao::FocarAba(id) => {
                    self.abas.focar(id);
                }
                Acao::MoverAba { id, para } => {
                    self.abas.mover(id, para);
                }
                // a aba alterada pergunta antes (a fila de perguntas ao fechar, etapa 5)
                Acao::FecharAba(id) => {
                    let id = id.unwrap_or_else(|| self.abas.id_ativa());
                    self.pedir_fechar_aba(ctx, id);
                }
                Acao::AbrirBanco { caminho, somente_leitura } => {
                    let de = self.origem_da_troca(origem);
                    self.iniciar_troca(ctx, Troca::Abrir { caminho, somente_leitura }, false, de);
                }
                Acao::EscolherEAbrir { somente_leitura } => {
                    let de = self.origem_da_troca(origem);
                    self.escolher_e_abrir(ctx, frame, somente_leitura, de)
                }
                Acao::NovoBanco => {
                    let de = self.origem_da_troca(origem);
                    self.novo_banco(ctx, frame, de)
                }
                Acao::FecharBanco => {
                    if self.estado.tem_banco() {
                        self.iniciar_troca(ctx, Troca::Fechar, false, Origem::Casca);
                    }
                }
                Acao::RecarregarObjetos => {
                    if self.estado.tem_banco() {
                        let id = self.estado.banco_id();
                        // o "Tentar de novo" do Explorador: volta ao ponto pulsante enquanto a leitura corre
                        if self.falha_objetos.take().is_some() {
                            self.mudou_estado(ctx);
                        }
                        self.tarefas.push(Tarefa::Objetos(id, self.tarefa(ctx, |s, id| s.objetos(id))));
                        // a Visão geral pede ao abrir (e no Atualizar) só para mostrar a lista de agora: nada mudou no
                        // banco, e as outras abas não releem
                        let da_visao_geral = origem
                            .and_then(|id| self.abas.get(id))
                            .is_some_and(|a| matches!(a.conteudo, Documento::Banco(_)));
                        if !da_visao_geral {
                            self.marcar_para_reler(ctx, origem);
                        }
                    }
                }
                Acao::RecarregarBanco => {
                    if self.estado.tem_banco() {
                        self.pedir_banco(ctx);
                    }
                }
                Acao::RecarregarPrefs => {
                    let p = self
                        .tarefa(ctx, |s, _| Ok((s.prefs().resumo(), s.prefs().ultimo_diretorio().map(PathBuf::from))));
                    self.tarefas.push(Tarefa::Prefs(p));
                }
                Acao::NotarTransacao(aberta) => {
                    // toda resposta de uma Consulta (também a da execução solta, sem aba) avisa a transação: um
                    // comando pode ter entrado no histórico
                    let da_consulta = origem.is_none_or(|id| {
                        self.abas.get(id).is_some_and(|a| matches!(a.conteudo, Documento::Consulta(_)))
                    });
                    if da_consulta {
                        self.casca.historico.marcar_velha();
                    }
                    self.notar_transacao(ctx, origem, aberta)
                }
                Acao::BancoTrocado => self.ressincronizar(ctx),
                Acao::LembrarDiretorio(pasta) => {
                    self.estado.ultimo_diretorio = Some(pasta.clone());
                    let p = self.tarefa(ctx, move |s, _| {
                        s.lembrar_diretorio(&pasta);
                        Ok(())
                    });
                    self.tarefas.push(Tarefa::Diretorio(p));
                }
                Acao::TrocarTema(t) => {
                    self.tema = tema_da_interface(t);
                    tema::aplicar(ctx, self.tema);
                    self.estado.prefs.tema = t;
                    self.mudou_estado(ctx);
                    let promessa = self.tarefa(ctx, move |s, _| s.gravar_prefs(Some(t), None));
                    self.tarefas.push(Tarefa::Tema { promessa, limpou_historico: false });
                }
                Acao::MostrarPainel(painel) => {
                    self.paineis.mostrar(Painel::from(painel));
                    ctx.request_repaint();
                }
                Acao::DestacarPainel(painel) => self.mover_painel(ctx, Painel::from(painel), Lugar::Janela),
                Acao::AbrirPreferencias => self.abrir_preferencias(ctx, Secao::Geral),
                Acao::ManutencaoDoBanco(m) => self.pedir_a_visao_geral(PedidoDoMenu::Manutencao(m)),
                Acao::CopiaDeSeguranca => self.pedir_a_visao_geral(PedidoDoMenu::Copia),
                Acao::AbrirExportacao(pedido) => {
                    self.janelas.pedido_exportar = Some(pedido.clone());
                    self.janelas.exportar = Some(JanelaExportar::nova(pedido));
                }
                Acao::AbrirNovaTabela => self.janelas.nova_tabela = Some(JanelaNovaTabela::nova()),
                Acao::AbrirNovoIndice { tabela, colunas } => {
                    self.janelas.tabela_do_indice = tabela.clone();
                    self.janelas.novo_indice = Some(JanelaNovoIndice::nova(tabela, colunas))
                }
                Acao::EscolherCopia { nome_sugerido } => self.escolher_copia(ctx, frame, nome_sugerido),
                // a pasta e os arquivos de dados (etapa 4): o erro volta ao Início quando a ação veio dele
                Acao::AbrirCaminho { caminho, somente_leitura } => {
                    let de = self.origem_da_troca(origem);
                    self.abrir_caminho(ctx, caminho, somente_leitura, de);
                }
                Acao::EscolherEAbrirPasta => {
                    let de = self.origem_da_troca(origem);
                    self.escolher_e_abrir_pasta(ctx, frame, de);
                }
                Acao::AbrirPasta { caminho } => {
                    let de = self.origem_da_troca(origem);
                    self.abrir_pasta(ctx, caminho, de);
                }
                Acao::AbrirArquivoDeDados { caminho } => {
                    let de = self.origem_da_troca(origem);
                    self.abrir_arquivo_de_dados(ctx, caminho, de);
                }
                Acao::ListarPasta { caminho } => self.listar_pasta(ctx, caminho),
                Acao::AtualizarPasta => self.atualizar_pasta(ctx),
                Acao::FecharPasta => self.pedir_fechar_pasta(ctx),
            }
        }
    }

    /// Os atalhos da casca ([`casca::atalhos`]): Ctrl+O, Ctrl+Shift+O, Alt+←/→ e os botões laterais do mouse, Ctrl+W, Ctrl+Tab,
    /// Ctrl+Shift+Tab, F5, Ctrl+B, Ctrl+Alt+B e Ctrl+F. Não valem com uma janela ou modal aberta. O F5 só vale com o
    /// Atualizar ligado na aba (`ofertas`), o Ctrl+F, com o Buscar, e o Ctrl+Alt+B, com o botão Painel de propriedades
    /// ligado (`painel`): fora disso, as teclas ficam com quem tem o foco. O Ctrl+B e o Ctrl+Alt+B mostram ou escondem
    /// o Explorador e as Propriedades na hora, sem tirar o foco de ninguém (o Ctrl+Alt+B fica com o campo de texto
    /// focado), e o Ctrl+F leva o foco à busca da barra, com o texto dela selecionado. Nos outros, a ação (e o
    /// Atualizar do F5) fica para o quadro seguinte e a edição de texto em curso termina agora: a célula da grade em
    /// edição vê o campo perder o foco neste quadro e grava antes de a aba sair de vista ou ser relida.
    pub(super) fn atalhos_de_teclado(
        &mut self,
        ctx: &egui::Context,
        ui: &Ui,
        ofertas: &Ofertas,
        painel: EstadoPainel,
    ) -> Option<Verbo> {
        // uma janela aberta tem prioridade
        let modal = self.janela_aberta() || ctx.memory(|m| m.top_modal_layer().is_some());
        if modal {
            return None;
        }
        let valem = casca::atalhos::Valem {
            campo_de_texto: ctx.text_edit_focused(),
            tabela: matches!(self.situacao, Situacao::Pronta)
                && matches!(self.abas.ativa().conteudo, Documento::Tabela(_)),
            atualizar: ofertas.ligado(Verbo::Atualizar),
            propriedades: painel.ligado(),
            buscar: ofertas.ligado(Verbo::Buscar),
            salvar: ofertas.ligado(Verbo::Salvar),
            desfazer: ofertas.ligado(Verbo::Desfazer),
            refazer: ofertas.ligado(Verbo::Refazer),
        };
        let a = casca::atalhos::ler(ctx, valem);
        if a.explorador {
            self.paineis.alternar(Painel::Explorador);
        }
        if a.propriedades {
            self.paineis.alternar(Painel::Propriedades);
        }
        if a.buscar {
            self.focar_busca(ctx);
        }
        // Ctrl+S, Ctrl+Z e Ctrl+Y: o verbo vai ao documento neste quadro, antes de ele se desenhar
        let verbo = a.verbo();
        if !a.sai_do_documento() {
            return verbo;
        }
        if a.atualizar {
            self.verbo_pendente = Some(Verbo::Atualizar);
        }
        if a.inicio {
            self.pendentes.push((None, Acao::Navegar(Rota::Inicio)));
        }
        if let Some(n) = a.vista {
            let parte = estado::Parte::DA_TABELA[usize::from(n - 1)];
            self.item_da_tabela(ctx, ItemTabela::Vista(parte));
        }
        if a.abrir {
            casca::menus::fechar_todos(ui);
            self.pendentes.push((None, Acao::EscolherEAbrir { somente_leitura: false }));
        }
        if a.abrir_pasta {
            casca::menus::fechar_todos(ui);
            self.pendentes.push((None, Acao::EscolherEAbrirPasta));
        }
        if a.preferencias {
            casca::menus::fechar_todos(ui);
            self.abrir_preferencias(ctx, Secao::Geral);
        }
        if a.voltar {
            self.pendentes.push((None, Acao::Voltar));
        }
        if a.avancar {
            self.pendentes.push((None, Acao::Avancar));
        }
        if a.fechar_aba {
            self.pendentes.push((None, Acao::FecharAba(Some(self.abas.id_ativa()))));
        }
        // a aba seguinte e a anterior na faixa, com volta
        for (pedida, seguinte) in [(a.proxima, true), (a.anterior, false)] {
            if let Some(id) = self.abas.vizinha(seguinte).filter(|_| pedida) {
                self.pendentes.push((None, Acao::FocarAba(id)));
            }
        }
        // a ativa uma casa para a esquerda ou para a direita, sem volta
        let id = self.abas.id_ativa();
        if let Some(i) = self.abas.posicao_na_faixa(id) {
            let para = match (a.mover_esquerda, a.mover_direita) {
                (true, _) => i.checked_sub(1),
                (_, true) => (i + 1 < self.abas.len()).then_some(i + 1),
                _ => None,
            };
            if let Some(para) = para {
                self.pendentes.push((None, Acao::MoverAba { id, para }));
            }
        }
        terminar_edicao(ctx);
        verbo
    }

    /// O foco na busca da barra da aba ativa, com o texto dela selecionado (o Ctrl+F e o Buscar do menu Editar).
    pub(super) fn focar_busca(&mut self, ctx: &egui::Context) {
        if !matches!(self.situacao, Situacao::Pronta) {
            return;
        }
        let aba = self.abas.id_ativa().0;
        if let Some(b) = self.abas.ativa_mut().conteudo.doc().busca() {
            estado::focar_busca_da_barra(ctx, aba, b.texto);
            ctx.request_repaint();
        }
    }
}
