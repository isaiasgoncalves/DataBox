//! O ciclo da janela: um quadro inteiro ([`Editor::desenhar`]: as respostas que chegaram, a casca, o documento
//! da aba ativa, as janelas e, no fim, as ações), o diagnóstico de repintura e o `eframe::App`.

use egui::{Align, Id, Key, Layout, Modifiers, Rect, Ui, UiBuilder};
use egui::{ViewportCommand, pos2, vec2};
use estado::{Acao, Anatomia, ChaveAba, Contexto, IdAba, Ofertas, Rolagem, Rota, Verbo};
use interface::fontes;
use interface::pecas::abas_documento::ALTURA_FAIXA;
use interface::pecas::avisos::aviso_texto;
use interface::pecas::barra_status::ALTURA_STATUS;
use interface::pecas::barra_titulo::ALTURA_TITULO;
use interface::pecas::cartao::Cartao;
use interface::pecas::divisoria::{Divisoria, LadoDoPainel};
use interface::pecas::documento::{Documento as DocumentoPeca, Pagina};
use interface::pecas::ferramentas::ALTURA_BARRA;
use interface::pecas::toast::desenhar_toast;
use interface::pintura;
use interface::tema;

use crate::casca;
use crate::casca::docas::{Lado, Lugar, Painel};
use crate::casca::ferramentas::{Barra, EstadoPainel, PedidoBarra};
use crate::casca::menus::{MenuTabela, Menus, PedidoMenu};
use crate::casca::sobre::Sobre;
use crate::documento::Documento;
use crate::tela_arquivo::PedidoArquivo;

use super::abrir::{ESPERA_LONGA, ESPERA_PARA_MOSTRAR};
use super::paineis::{Medidas, PainelLateral, destino_do_arrasto, rotulo_da_largura};
use super::{ESPACO, Editor, Situacao, terminar_edicao};

/// O espaço entre a faixa de 30px do alto e a barra de ferramentas (`header.vidro{margin-top:6px}`).
const MARGEM_BARRA: f32 = 6.0;
/// O quanto a página rola no End, até o fim (o Home vai ao começo pelo `ao_topo`): mais que qualquer página, e finito
/// (o `scroll_with_delta` não aceita o infinito).
const ROLAR_ATE_O_FIM: f32 = -1.0e7;

impl Editor {
    /// Desenha um quadro inteiro na janela (`ui` = a janela toda, sem margem). `frame` é a janela do eframe, dona
    /// dos diálogos do Windows (None nos testes).
    pub fn desenhar(&mut self, ui: &mut Ui, frame: Option<&eframe::Frame>) {
        let ctx = ui.ctx().clone();
        tema::aplicar(&ctx, self.tema);
        if !fontes::instaladas(&ctx) {
            ctx.request_repaint();
            return;
        }
        if !self.iniciado {
            self.iniciar(&ctx);
        }
        self.receber(&ctx);
        self.receber_fonte(&ctx);
        self.cumprir_acao_do_toast(&ctx);
        // a janela voltou a ter o foco (o Alt+Tab de volta, um clique nela): a pasta aberta relê, porque outro
        // programa pode ter criado ou apagado arquivos nela (sem vigia do sistema nem temporizador)
        let focada = ctx.input(|i| i.focused);
        if focada && !self.focada {
            self.releitura_automatica(&ctx);
        }
        self.focada = focada;
        self.soltar_na_janela(&ctx);
        // as ações do quadro, com a aba que as emitiu (None: a casca, as janelas)
        let mut acoes: Vec<(Option<IdAba>, Acao)> = std::mem::take(&mut self.pendentes);
        // o verbo de um atalho do quadro anterior (o F5 deste quadro fica para o próximo)
        let verbo_do_quadro = self.verbo_pendente.take();
        // a aba ativa que outra aba mudou no quadro anterior: relê no fim deste, depois de se desenhar
        let reler_ativa = self.reler_ativa.take();
        let da_casca = |acoes: &mut Vec<(Option<IdAba>, Acao)>, emitidas: Vec<Acao>| {
            acoes.extend(emitidas.into_iter().map(|a| (None, a)));
        };

        // as medidas, de cima para baixo: a barra de título de 30px, a barra de ferramentas (`margin:6px 8px 0`,
        // 44px) e o corpo (`padding:8px; gap:8px`), até a barra de status (`margin:0 8px 8px`; sem ela, até a margem
        // de baixo): a doca da esquerda, a seção do documento (a faixa de abas e o `.doc`) e a doca da direita
        let janela = ctx.content_rect();
        let topo = Rect::from_min_size(janela.min, vec2(janela.width(), ALTURA_TITULO));
        let rect_barra = Rect::from_min_size(
            pos2(janela.left() + ESPACO, topo.bottom() + MARGEM_BARRA),
            vec2(janela.width() - 2.0 * ESPACO, ALTURA_BARRA),
        );
        let altura_status = if self.barra_status { ALTURA_STATUS + ESPACO } else { 0.0 };
        let rect_status = Rect::from_min_max(
            pos2(janela.left() + ESPACO, janela.bottom() - ESPACO - ALTURA_STATUS),
            pos2(janela.right() - ESPACO, janela.bottom() - ESPACO),
        );
        let corpo = Rect::from_min_max(
            pos2(janela.left() + ESPACO, rect_barra.bottom() + ESPACO),
            pos2(janela.right() - ESPACO, janela.bottom() - ESPACO - altura_status),
        );
        let pronta = matches!(self.situacao, Situacao::Pronta);

        // os atalhos: o F5 só com o Atualizar da aba ligado e o Ctrl+Alt+B só com o botão das Propriedades ligado
        // (uma vez por quadro: o que o documento oferece não muda até ele se desenhar; as ofertas dos Dados clonavam
        // as chaves marcadas a cada chamada)
        let ofertas = if pronta { self.abas.ativa().conteudo.ofertas(&self.estado) } else { Ofertas::default() };
        let painel_agora = self.medidas(corpo).botao_do_painel;
        let verbo_do_atalho = self.atalhos_de_teclado(&ctx, ui, &ofertas, painel_agora);
        self.pedido_de_fechar(&ctx);
        // as origens de colunas das abas abertas (o "Trazer colunas"), uma `Arc` por aba, no `Contexto` de todas
        let fontes = self.fontes_de_colunas();
        self.documentos_em_segundo_plano(&ctx, frame, &fontes, &mut acoes);

        let mut raiz =
            ui.new_child(UiBuilder::new().id_salt("editor").max_rect(janela).layout(Layout::top_down(Align::Min)));
        raiz.set_clip_rect(janela);
        pintura::pintar_fundo(raiz.painter(), tema::paleta(&raiz), janela);
        let rota_ativa = self.abas.ativa().rota().clone();
        // o Ctrl+S, o Ctrl+Z e o Ctrl+Y deste quadro; o F5 do quadro anterior
        let mut verbo = verbo_do_atalho.or(verbo_do_quadro);

        // a barra de título, antes de tudo (os menus vêm antes dos painéis na árvore de acessibilidade e no Tab): o
        // verbo do Editar é executado no documento antes de ele se desenhar, como o da barra de ferramentas; os
        // painéis mudam na hora
        let so_o_inicio = self.abas.len() == 1 && self.abas.ativa().chave() == Some(&ChaveAba::Inicio);
        let menus = Menus {
            ofertas: &ofertas,
            tem_banco: self.estado.tem_banco(),
            tem_pasta: self.estado.tem_pasta(),
            pode_fechar_aba: pronta && !so_o_inicio,
            paineis: self.paineis,
            tem_propriedades: pronta && self.abas.ativa().conteudo.tem_propriedades(),
            propriedades: self.medidas(corpo).botao_do_painel,
            tema: self.tema,
            barra_status: self.barra_status,
            banco: self.estado.banco.as_ref(),
            na_visao_geral: pronta && matches!(self.abas.ativa().conteudo, Documento::Banco(_)),
            tabela: match (pronta, &self.abas.ativa().conteudo) {
                (true, Documento::Tabela(t)) => Some(MenuTabela {
                    nome: t.nome(),
                    e_visao: t.tabela_aberta().map_or(matches!(rota_ativa, Rota::Visao { .. }), |a| a.e_visao),
                    editavel: t.editavel(self.estado.somente_leitura()),
                }),
                _ => None,
            },
            arquivo: match (pronta, &self.abas.ativa().conteudo) {
                (true, Documento::Arquivo(t)) => Some(t.caminho()),
                _ => None,
            },
        };
        let mut ui_topo = raiz
            .new_child(UiBuilder::new().id_salt("barra-titulo").max_rect(topo).layout(Layout::top_down(Align::Min)));
        let faixa =
            self.casca.faixa_titulo(&mut ui_topo, &self.estado, self.versao, &self.titulo, self.barra_propria, &menus);
        match faixa.pedido {
            Some(PedidoMenu::Acao(a)) => acoes.push((None, a)),
            Some(PedidoMenu::Verbo(v)) => verbo = Some(v),
            Some(PedidoMenu::Painel(painel)) => self.paineis.alternar(painel),
            Some(PedidoMenu::BarraStatus) => self.alternar_barra_status(&ctx),
            Some(PedidoMenu::Tabela(item)) => self.item_da_tabela(&ctx, item),
            Some(PedidoMenu::Atalhos) => self.janelas.atalhos = true,
            Some(PedidoMenu::Sobre) => {
                self.janelas.sobre.get_or_insert_with(Sobre::default);
            }
            Some(PedidoMenu::Preferencias(secao)) => self.abrir_preferencias(&ctx, secao),
            Some(PedidoMenu::AbrirNoPrograma(caminho)) => self.abrir_no_programa(&ctx, caminho),
            None => {}
        }

        // a barra de ferramentas, antes das docas (é a ordem do Tab) e do documento: o verbo clicado (ou o
        // F5 do quadro anterior) é executado no documento antes de ele se desenhar, e a busca da barra é o texto que o
        // documento empresta (o que acontece nela também vai ao documento antes do conteúdo); os pedidos da casca
        // valem no quadro seguinte, depois de a edição em curso gravar. O botão das Propriedades é medido com os
        // painéis de agora (o clique numa aba de doca, logo depois, pede outro quadro)
        let barra = Barra {
            ofertas: &ofertas,
            pode_voltar: pronta && self.abas.pode_voltar(),
            pode_avancar: pronta && self.abas.pode_avancar(),
            tem_banco: self.estado.tem_banco(),
            painel: self.medidas(corpo).botao_do_painel,
            aba: self.abas.id_ativa().0,
        };
        let mut ui_barra = raiz.new_child(UiBuilder::new().id_salt("barra-ferramentas").max_rect(rect_barra));
        let busca = if pronta { self.abas.ativa_mut().conteudo.doc().busca() } else { None };
        let (pedido_barra, evento_busca) = casca::ferramentas::barra(&mut ui_barra, &barra, busca);
        match pedido_barra {
            Some(PedidoBarra::Acao(a)) => {
                self.pendentes.push((None, a));
                terminar_edicao(&ctx);
            }
            Some(PedidoBarra::Verbo(v)) => verbo = Some(v),
            Some(PedidoBarra::AlternarPropriedades) => {
                self.paineis.alternar(Painel::Propriedades);
                ctx.request_repaint();
            }
            None => {}
        }
        // o Buscar do menu Editar leva o foco à busca da barra, como o Ctrl+F
        if verbo == Some(Verbo::Buscar) {
            verbo = None;
            self.focar_busca(&ctx);
        }
        // (o Atualizar de uma aba com alterações não salvas: a própria tela pergunta antes, decisão 27 da etapa 5)
        if pronta && (verbo.is_some() || evento_busca.is_some()) {
            let id = self.abas.id_ativa();
            let (versao, trocando) = (self.versao, self.troca.is_some());
            let Editor { servico, estado, abas, .. } = self;
            let mut emitidas = Vec::new();
            let mut contexto =
                Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando).com_fontes(&fontes);
            let documento = &mut abas.ativa_mut().conteudo;
            if let Some(v) = verbo {
                documento.executar(v, &ctx, &mut contexto);
            }
            if let Some(e) = evento_busca {
                documento.doc().evento_busca(e, &ctx, &mut contexto);
                ctx.request_repaint();
            }
            acoes.extend(emitidas.into_iter().map(|a| (Some(id), a)));
        }

        // as larguras, com as docas que a pessoa quer agora
        let Medidas { esquerda: rect_esquerda, secao, direita, com_direita, botao_do_painel, disposicao } =
            self.medidas(corpo);
        let tem_propriedades = pronta && self.abas.ativa().conteudo.tem_propriedades();
        self.propriedades_a_vista = match self.paineis.lugar(Painel::Propriedades) {
            Lugar::Doca(Lado::Direita) => com_direita && self.paineis.a_vista(Painel::Propriedades, tem_propriedades),
            _ => self.paineis.a_vista(Painel::Propriedades, tem_propriedades),
        };
        self.disposicao = Some(disposicao);
        // o documento tem Propriedades, mas o painel não está à vista (escondido ou sem largura): nas telas de
        // rolagem de página, as seções vão para o fim do documento (decisão 4, `estado::propriedades_no_fim`)
        let propriedades_no_fim = matches!(botao_do_painel, EstadoPainel::Escondido | EstadoPainel::SemLargura);

        // a doca da esquerda, antes do documento (é a ordem do Tab)
        let mut arrasto = None;
        if let Some(r) = rect_esquerda {
            arrasto = self.doca(&mut raiz, Lado::Esquerda, r, frame, &fontes, &rota_ativa, &mut acoes);
            // a divisória no vão entre a doca e o documento (logo depois dela, na ordem do Tab)
            let rotulo = rotulo_da_largura(self.paineis.ativo(Lado::Esquerda, tem_propriedades));
            let vao = Rect::from_min_max(pos2(r.right(), r.top()), pos2(r.right() + ESPACO, r.bottom()));
            let saida =
                Divisoria::new("painel-esquerdo", rotulo, vao, LadoDoPainel::Esquerda, disposicao.limites_esquerda())
                    .mostrar(&mut raiz);
            self.largura_pedida(&ctx, PainelLateral::Esquerdo, &saida, &disposicao, true);
        }
        if !self.paineis.a_vista(Painel::Historico, tem_propriedades) {
            self.casca.historico.esconder();
        }

        // a troca de banco que está demorando: o card no alto do conteúdo (e um quadro quando o texto muda)
        let texto_troca = self.troca.as_ref().and_then(|t| {
            let decorrido = ctx.input(|i| i.time) - t.desde;
            if decorrido < ESPERA_PARA_MOSTRAR {
                return None;
            }
            if decorrido < ESPERA_LONGA {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(ESPERA_LONGA - decorrido));
            }
            Some(t.troca.em_andamento(decorrido >= ESPERA_LONGA))
        });

        // a faixa de abas no alto da seção (só com o documento à vista); o que se pede nela vale no quadro seguinte,
        // depois de a edição em curso gravar. O documento (`.doc`) começa 1px antes do fim dela e forma uma superfície
        // só com a aba ativa
        let mut juncao = None;
        let topo_documento = if pronta {
            let r = Rect::from_min_size(secao.min, vec2(secao.width(), ALTURA_FAIXA));
            let mut ui_faixa = raiz.new_child(UiBuilder::new().id_salt("faixa-abas").max_rect(r));
            let (pedido, j) = casca::abas::faixa(&mut ui_faixa, &self.abas, &self.estado);
            if let Some(a) = pedido {
                self.pendentes.push((None, a));
                terminar_edicao(&ctx);
            }
            juncao = j;
            r.bottom() - 1.0
        } else {
            secao.top()
        };
        let rect_documento = Rect::from_min_max(pos2(secao.left(), topo_documento), secao.max);
        self.rect_documento = Some(rect_documento);
        let anatomia = if pronta { self.abas.ativa().conteudo.anatomia() } else { Anatomia::default() };

        // o documento (a peça `.doc`): os avisos do topo (a troca que demora, a transação aberta fora das Consultas), a
        // faixa, o conteúdo e o rodapé que ele pede, por dentro da borda (a roda em cima dos painéis é deles)
        let id_ativa = self.abas.id_ativa();
        let consulta_da_transacao = self.nome_da_consulta_da_transacao();
        let mut do_documento = Vec::new();
        let mut ir_para_a_consulta = false;
        {
            // A página rola pelo teclado como a do navegador (o ScrollArea do egui não faz isso), quando nada tem o
            // foco: um campo de texto, a grade ou um botão focado ficam com as teclas. O documento que rola por
            // dentro (`Rolagem::Propria`) fica com elas.
            let pagina_da_casca = anatomia.rolagem == Rolagem::Pagina;
            let teclado = if !pagina_da_casca
                || self.janela_aberta()
                || ctx.memory(|m| m.focused().is_some() || m.top_modal_layer().is_some())
            {
                None
            } else {
                ctx.input_mut(|i| {
                    let shift = i.modifiers.shift;
                    if i.consume_key(Modifiers::NONE, Key::PageDown)
                        || (!shift && i.consume_key(Modifiers::NONE, Key::Space))
                    {
                        Some(-0.9)
                    } else if i.consume_key(Modifiers::NONE, Key::PageUp)
                        || (shift && i.consume_key(Modifiers::SHIFT, Key::Space))
                    {
                        Some(0.9)
                    } else if i.consume_key(Modifiers::NONE, Key::Home) {
                        Some(f32::INFINITY)
                    } else if i.consume_key(Modifiers::NONE, Key::End) {
                        Some(f32::NEG_INFINITY)
                    } else {
                        None
                    }
                })
            };
            let Editor { servico, estado, situacao, abas, troca, versao, .. } = self;
            let trocando = troca.is_some();
            let versao = *versao;
            let na_consulta = matches!(rota_ativa, Rota::Consulta { .. });
            let mut doc = DocumentoPeca::abrir(&raiz, rect_documento, juncao, anatomia.rodape);
            if let Some(texto) = texto_troca {
                casca::aviso_troca(&mut raiz, &mut doc, texto);
            }
            if estado.transacao_aberta() && !na_consulta {
                ir_para_a_consulta = casca::aviso_transacao(&mut raiz, &mut doc, &consulta_da_transacao);
            }
            // cada aba guarda a própria rolagem
            let mut pagina = Pagina::new(("doc", id_ativa));
            pagina.ao_topo = teclado == Some(f32::INFINITY);
            pagina.rolar_por = match teclado {
                Some(f) if f.is_finite() => Some(f * doc.rect_conteudo().height()),
                Some(f) if f == f32::NEG_INFINITY => Some(ROLAR_ATE_O_FIM),
                _ => None,
            };
            let mut contexto = Contexto::novo(servico, estado, frame, &mut do_documento)
                .com_casca(versao, trocando)
                .com_fontes(&fontes);
            match situacao {
                Situacao::Carregando => {}
                Situacao::Falha(m) => doc.pagina(&mut raiz, pagina, |ui| {
                    Cartao::new("falha-inicio").titulo("Não foi possível iniciar").mostrar(ui, |c| {
                        c.corpo(|ui| {
                            aviso_texto(ui, m, true);
                        });
                    });
                }),
                Situacao::Pronta => {
                    let tela = abas.ativa_mut().conteudo.doc();
                    // os avisos do documento (a transação da Consulta SQL), antes da faixa
                    tela.avisos(&mut doc, &mut raiz, &mut contexto);
                    if anatomia.faixa {
                        doc.faixa(&mut raiz, |ui| tela.faixa(ui, &mut contexto));
                    }
                    match anatomia.rolagem {
                        Rolagem::Pagina => doc.pagina(&mut raiz, pagina, |ui| {
                            tela.conteudo(ui, &mut contexto);
                            // A anatomia foi lida antes do conteúdo: a tela de tabela que acabou de ser lida
                            // (Carregando → Pronta, dentro do `conteudo`) vira a parte de rolagem própria, que enche a
                            // altura toda e deixa o cursor do `ScrollArea` no infinito. Nesse quadro não há fim de
                            // página onde pôr as Propriedades (o próximo quadro já nasce com a anatomia certa).
                            let ainda_pagina = tela.anatomia().rolagem == Rolagem::Pagina;
                            if !ainda_pagina {
                                ui.ctx().request_repaint();
                            }
                            if propriedades_no_fim && ainda_pagina {
                                estado::propriedades_no_fim(ui, |ui| tela.propriedades(ui, &mut contexto));
                            }
                        }),
                        Rolagem::Propria => doc.propria(&mut raiz, |ui| tela.conteudo(ui, &mut contexto)),
                    }
                    let _ = doc.rodape(&mut raiz, |ui| tela.rodape(ui, &mut contexto));
                }
            }
        }
        if ir_para_a_consulta {
            acoes.push((None, Acao::Navegar(Rota::Consulta { sql: None })));
        }
        acoes.extend(do_documento.into_iter().map(|a| (Some(id_ativa), a)));
        // os botões da ficha do arquivo de dados: o programa padrão e o Explorador de Arquivos, numa tarefa
        if let Some(Documento::Arquivo(t)) = pronta.then(|| &mut self.abas.ativa_mut().conteudo)
            && let Some(pedido) = t.tomar_pedido()
        {
            match pedido {
                PedidoArquivo::AbrirNoPadrao(caminho) => self.abrir_no_programa(&ctx, caminho),
                PedidoArquivo::MostrarNaPasta(caminho) => self.revelar(&ctx, caminho),
                PedidoArquivo::TrazerColunas(rotulo) => self.trazer_colunas_de(&ctx, rotulo),
            }
        }

        // a doca da direita, depois de o documento se desenhar neste quadro (as Propriedades da tabela já lida); o que
        // o painel pede vem marcado com a aba
        if com_direita {
            let r = Rect::from_min_max(pos2(direita + ESPACO, corpo.top()), corpo.max);
            // a divisória no vão entre o documento e a doca (antes dela, na ordem do Tab)
            let vao = Rect::from_min_max(pos2(direita, r.top()), pos2(r.left(), r.bottom()));
            let rotulo = rotulo_da_largura(self.paineis.ativo(Lado::Direita, tem_propriedades));
            let saida =
                Divisoria::new("propriedades", rotulo, vao, LadoDoPainel::Direita, disposicao.limites_direita())
                    .mostrar(&mut raiz);
            self.largura_pedida(&ctx, PainelLateral::Direito, &saida, &disposicao, rect_esquerda.is_some());
            if let Some(a) = self.doca(&mut raiz, Lado::Direita, r, frame, &fontes, &rota_ativa, &mut acoes) {
                arrasto = Some(a);
            }
        }
        // a aba de painel arrastada: acende o destino e, solta, muda o painel de lugar
        if let Some((painel, a)) = arrasto {
            let destino = destino_do_arrasto(a.ponteiro, janela, rect_esquerda, com_direita.then_some(direita), corpo);
            if let Some((lugar, alvo)) = destino {
                if a.solto {
                    if lugar == Lugar::Janela {
                        // a janela abre onde a aba foi solta (o ponteiro, na tela)
                        let tela =
                            ctx.input(|i| i.viewport().inner_rect).map_or(a.ponteiro, |r| r.min + a.ponteiro.to_vec2());
                        self.posicao_da_janela = Some((painel, tela));
                    }
                    self.mover_painel(&ctx, painel, lugar);
                } else {
                    interface::pecas::doca::pintar_alvo(&raiz, Id::new("alvo-da-doca"), alvo);
                }
            }
            ctx.request_repaint();
        }

        // as janelas próprias dos painéis (depois do documento: as Propriedades da tabela já lida)
        self.janelas_dos_paineis(&ctx, frame, &fontes, &rota_ativa, &mut acoes, janela);

        // a barra de status, com o que o documento à vista diz de si (depois de ele se desenhar neste quadro)
        let do_status = self.documento().and_then(|d| d.status(&self.estado));
        // a aba de arquivo que diz o próprio status (os editores da etapa 5): a barra do arquivo, no lugar da do banco
        let do_arquivo = self.documento().and_then(|d| d.doc_ref().status_do_arquivo(&self.estado));
        if self.barra_status {
            let mut ui_status = raiz.new_child(UiBuilder::new().id_salt("barra-status").max_rect(rect_status));
            let consulta = self.nome_da_consulta_da_transacao();
            let (estado, versao) = (&self.estado, self.versao);
            self.casca.status(&mut ui_status, estado, versao, do_status.as_deref(), &consulta, do_arquivo.as_ref());
        }

        // a aba que outra aba mudou no quadro anterior relê agora (a célula em edição já gravou neste quadro)
        self.reler_a_ativa(&ctx, reler_ativa);

        // janelas globais, por cima de tudo
        let mut indice_criado = false;
        let mut caminho_digitado = None;
        let mut das_janelas = Vec::new();
        {
            let (versao, trocando) = (self.versao, self.troca.is_some());
            let Editor { servico, estado, janelas, casca, renderizador, abrir_de_teste, .. } = self;
            {
                let mut contexto = Contexto::novo(servico, estado, frame, &mut das_janelas).com_casca(versao, trocando);
                if let Some(j) = janelas.sobre.as_mut()
                    && !j.mostrar(&ctx, &mut contexto, *renderizador, abrir_de_teste)
                {
                    janelas.sobre = None;
                    casca.focar_menu();
                }
                if let Some(j) = janelas.exportar.as_mut()
                    && !j.mostrar(&ctx, &mut contexto)
                {
                    janelas.exportar = None;
                }
                if let Some(j) = janelas.nova_tabela.as_mut()
                    && !j.mostrar(&ctx, &mut contexto)
                {
                    janelas.nova_tabela = None;
                }
            }
            if let Some(j) = janelas.novo_indice.as_mut() {
                // o que a janela emite fica à parte para saber se ela criou o índice (`RecarregarObjetos`)
                let mut emitidas = Vec::new();
                let mut contexto = Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando);
                let aberta = j.mostrar(&ctx, &mut contexto);
                janelas.indice_criado |= emitidas.iter().any(|a| matches!(a, Acao::RecarregarObjetos));
                das_janelas.extend(emitidas);
                if !aberta {
                    janelas.novo_indice = None;
                    indice_criado = std::mem::take(&mut janelas.indice_criado);
                }
            }
            if janelas.atalhos && !casca::janela_atalhos(&ctx) {
                janelas.atalhos = false;
                casca.focar_menu();
            }
            if let Some(j) = janelas.caminho.as_mut() {
                caminho_digitado = j.mostrar(&ctx);
            }
        }
        self.mostrar_preferencias(&ctx);
        da_casca(&mut acoes, das_janelas);
        if indice_criado {
            // criado o índice, as abas da tabela releem (a ativa na hora) e a dica de ordenação lenta some
            let tabela = std::mem::take(&mut self.janelas.tabela_do_indice);
            for aba in self.abas.iter_mut() {
                if matches!(aba.conteudo, Documento::Tabela(_)) && aba.rota().nome_objeto() == Some(tabela.as_str()) {
                    aba.conteudo.marcar_para_reler();
                }
            }
            ctx.request_repaint();
        }
        if let Some(s) = caminho_digitado {
            self.caminho_digitado(&ctx, s);
        }
        // a fila de perguntas ao fechar abas alteradas: a gravação esperada, a próxima pergunta ou o destino
        self.andar_fila(&ctx);
        self.mostrar_confirmacao(&ctx, frame);
        // o toast fica acima da barra de status, com o espaço de sempre entre os painéis; sem ela, acima do rodapé
        // do documento (a paginação e o "Linhas 1–60 de 60" continuam à vista)
        let limite_toast = if self.barra_status {
            rect_status.top() - ESPACO
        } else {
            corpo.bottom() - interface::pecas::documento::ALTURA_RODAPE - ESPACO
        };
        desenhar_toast(&ctx, limite_toast);
        // os controles da janela sem decoração, por cima de tudo (também da janela modal)
        if self.barra_propria {
            casca::janela::controles_da_janela(&mut raiz, faixa.logo, faixa.livre);
        }

        self.aplicar(&ctx, frame, acoes);
        // o fechar que chegou com a janela minimizada, com o que este quadro tomou (a transação já acertada)
        self.decidir_fechar_pendente(&ctx);
        // uma janela global fechou (a Exportar e a cópia criam arquivos): a pasta aberta relê
        let com_janela = self.janelas.alguma();
        if self.com_janela && !com_janela {
            self.reler_a_pasta(&ctx);
        }
        // a releitura que esperava uma leitura em andamento terminar
        if self.reler_pendente && !self.lendo_pasta() {
            self.reler_a_pasta(&ctx);
        }
        self.com_janela = com_janela;
        let janela_aberta = self.janela_aberta();
        self.casca.devolver_foco(&ctx, janela_aberta);
        if self.titulo_enviado.as_deref() != Some(self.titulo.as_str()) {
            ctx.send_viewport_cmd(ViewportCommand::Title(self.titulo.clone()));
            self.titulo_enviado = Some(self.titulo.clone());
        }
    }
}

/// As variáveis de ambiente que ligam o [`diagnosticar_repintura`]: a do DataBox e a do nome antigo, que continua
/// valendo.
const VARIAVEIS_DIAGNOSTICO: [&str; 2] = ["DATABOX_DIAGNOSTICO_REPINTURA", "EDITOR_DIAGNOSTICO_REPINTURA"];

/// Com a variável de ambiente `DATABOX_DIAGNOSTICO_REPINTURA` (ou a antiga, `EDITOR_DIAGNOSTICO_REPINTURA`)
/// definida, registra no log, a cada segundo com quadros, quantos quadros houve e quem pediu repintura (o app
/// parado não deve pintar nada).
fn diagnosticar_repintura(ctx: &egui::Context) {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    /// (início da janela de 1 s, quadros, causas e eventos contados)
    type Conta = Option<(f64, u32, BTreeMap<String, u32>)>;
    static LIGADO: OnceLock<bool> = OnceLock::new();
    static CONTA: Mutex<Conta> = Mutex::new(None);
    if !*LIGADO.get_or_init(|| VARIAVEIS_DIAGNOSTICO.iter().any(|v| std::env::var_os(v).is_some())) {
        return;
    }
    let agora = ctx.input(|i| i.time);
    let mut conta = CONTA.lock().unwrap_or_else(|e| e.into_inner());
    let (inicio, quadros, causas) = conta.get_or_insert_with(|| (agora, 0, BTreeMap::new()));
    *quadros += 1;
    for c in ctx.repaint_causes() {
        *causas.entry(format!("{}:{} {}", c.file, c.line, c.reason)).or_default() += 1;
    }
    ctx.input(|i| {
        for e in &i.raw.events {
            let nome: String = format!("{e:?}").chars().take_while(|c| c.is_alphanumeric()).collect();
            *causas.entry(format!("evento {nome}")).or_default() += 1;
        }
    });
    if agora - *inicio >= 1.0 {
        log::info!("repintura: {quadros} quadros em {:.1} s; causas: {causas:?}", agora - *inicio);
        *conta = None;
    }
}

impl eframe::App for Editor {
    /// Antes de cada `ui` e, com a janela minimizada ou encoberta, no lugar dele (ver [`Editor::logica`]).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.logica(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.desenhar(ui, Some(frame));
        diagnosticar_repintura(ui.ctx());
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}
