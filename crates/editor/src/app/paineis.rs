//! Os painéis: as medidas do corpo com as docas, as divisórias que mudam as larguras (e a gravação delas), as
//! docas e as janelas próprias dos painéis, o arrasto de uma aba de painel e o lugar de cada um.

use std::sync::Arc;

use egui::{Align, Layout, Pos2, Rect, Ui, UiBuilder};
use egui::{pos2, vec2};
use estado::{Acao, Contexto, FonteDeColunas, IdAba, Rota};
use interface::pecas::divisoria::SaidaDivisoria;
use interface::pecas::doca::{AbaDoca, Arrasto, Doca, FormaAbas};
use interface::pecas::painel::Encaixe;
use interface::pintura;
use interface::tema;

use crate::casca::docas::{Lado, Lugar, Painel};
use crate::casca::ferramentas::EstadoPainel;
use crate::casca::larguras::{self, Corpo, Disposicao};

use super::tarefas::Tarefa;
use super::{ESPACO, Editor, Situacao};

/// O que a pessoa pediu na janela própria de um painel.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PedidoDaJanela {
    /// O × do sistema ou o "Fechar" do cabeçalho: o painel volta escondido para a doca.
    Fechar,
    /// O "Prender de volta" (None: na doca de onde ele saiu) ou a aba solta em cima de um lado da janela principal.
    Prender(Option<Lado>),
    /// A aba foi solta neste ponto da janela própria (quem chama vê se caiu na janela principal).
    Solta(Pos2),
}

/// O tamanho com que a janela própria de um painel abre: a das Propriedades na largura da forma larga (a tabela de
/// colunas, os 720px da prancheta 6); a do Explorador e a do Histórico, na de uma doca larga.
fn tamanho_da_janela(painel: Painel) -> (f32, f32) {
    match painel {
        Painel::Propriedades => (720.0, 480.0),
        Painel::Explorador => (320.0, 560.0),
        Painel::Historico => (360.0, 520.0),
    }
}

/// O rótulo da divisória de uma doca, pelo painel à vista nela ("Largura do Explorador").
pub(super) fn rotulo_da_largura(painel: Option<Painel>) -> &'static str {
    match painel {
        Some(Painel::Explorador) | None => "Largura do Explorador",
        Some(Painel::Historico) => "Largura do Histórico",
        Some(Painel::Propriedades) => "Largura das Propriedades",
    }
}

/// A faixa da beirada do corpo que recebe uma aba arrastada quando a doca daquele lado está escondida.
const FAIXA_DE_ALVO: f32 = 120.0;

/// Onde cai a aba de painel solta em `ponteiro` (prancheta 2b) e o que acender: fora da janela, a janela própria;
/// sobre uma doca (ou, com ela escondida, a faixa da beirada do corpo daquele lado), aquela doca; no meio, nada.
pub(super) fn destino_do_arrasto(
    ponteiro: Pos2,
    janela: Rect,
    esquerda: Option<Rect>,
    inicio_direita: Option<f32>,
    corpo: Rect,
) -> Option<(Lugar, Rect)> {
    if !janela.contains(ponteiro) {
        return Some((Lugar::Janela, janela.shrink(2.0)));
    }
    let zona_esquerda =
        esquerda.unwrap_or_else(|| Rect::from_min_max(corpo.min, pos2(corpo.left() + FAIXA_DE_ALVO, corpo.bottom())));
    let zona_direita = match inicio_direita {
        Some(x) => Rect::from_min_max(pos2(x + ESPACO, corpo.top()), corpo.max),
        None => Rect::from_min_max(pos2(corpo.right() - FAIXA_DE_ALVO, corpo.top()), corpo.max),
    };
    if zona_esquerda.contains(ponteiro) {
        Some((Lugar::Doca(Lado::Esquerda), zona_esquerda))
    } else if zona_direita.contains(ponteiro) {
        Some((Lugar::Doca(Lado::Direita), zona_direita))
    } else {
        None
    }
}

/// Onde ficam o painel da esquerda, a seção do documento (a faixa de abas e o `.doc`) e as Propriedades, e o estado
/// do botão Painel de propriedades (o mesmo no menu Exibir).
pub(super) struct Medidas {
    /// A doca da esquerda, se está à vista.
    pub(super) esquerda: Option<Rect>,
    pub(super) secao: Rect,
    /// Onde a seção do documento termina (a doca da direita começa `ESPACO` depois).
    pub(super) direita: f32,
    /// A doca da direita está à vista (a pessoa a quer, ela tem aba e cabe).
    pub(super) com_direita: bool,
    pub(super) botao_do_painel: EstadoPainel,
    /// As larguras dos painéis neste quadro e até onde as divisórias os levam.
    pub(super) disposicao: Disposicao,
}

/// Qual das docas a divisória redimensiona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PainelLateral {
    Esquerdo,
    Direito,
}

impl Editor {
    /// As larguras do corpo com as docas que a pessoa quer agora, nas larguras dela ([`larguras::dispor`]): numa
    /// janela estreita as docas encolhem até [`larguras::MINIMA`] e, se o documento ainda ficaria com menos de 560px
    /// (`flex:999 1 560px`), a da direita some (sem mudar a escolha dela), como no Início quando ela só tem as
    /// Propriedades.
    pub(super) fn medidas(&self, corpo: Rect) -> Medidas {
        let pronta = matches!(self.situacao, Situacao::Pronta);
        let tem_propriedades = pronta && self.abas.ativa().conteudo.tem_propriedades();
        let largura = Corpo { janela: corpo.width() + 2.0 * ESPACO, largura: corpo.width(), espaco: ESPACO };
        let com_esquerda = self.paineis.doca_a_vista(Lado::Esquerda, tem_propriedades);
        let quer_direita = self.paineis.doca_a_vista(Lado::Direita, tem_propriedades);
        let disposicao = larguras::dispor(&largura, self.larguras, com_esquerda, quer_direita);
        let esquerda = com_esquerda.then(|| Rect::from_min_size(corpo.min, vec2(disposicao.esquerda, corpo.height())));
        let inicio = esquerda.map_or(corpo.left(), |r| r.right() + ESPACO);
        let com_direita = disposicao.com_direita;
        let direita = if com_direita { corpo.right() - disposicao.direita - ESPACO } else { corpo.right() };
        let secao = Rect::from_min_max(pos2(inicio, corpo.top()), pos2(direita.max(inicio), corpo.bottom()));
        let botao_do_painel = if !tem_propriedades {
            // o Início (e o quadro antes de o estado chegar): o botão desligado, sem mudar a escolha da pessoa
            EstadoPainel::SemPainel
        } else if !self.paineis.a_vista(Painel::Propriedades, true) {
            EstadoPainel::Escondido
        } else if self.paineis.lugar(Painel::Propriedades) == Lugar::Doca(Lado::Direita) && !com_direita {
            EstadoPainel::SemLargura
        } else {
            EstadoPainel::AVista
        };
        Medidas { esquerda, secao, direita, com_direita, botao_do_painel, disposicao }
    }

    /// A divisória de uma doca acabou de pedir `largura` (o arrasto, as setas, o duplo clique). O que a pessoa vê
    /// passa a ser a escolha dela: a outra doca, se está à vista, guarda a largura com que foi desenhada. Quando o
    /// gesto termina, a escolha nova vai para as preferências (só o que mudou, numa tarefa).
    pub(super) fn largura_pedida(
        &mut self,
        ctx: &egui::Context,
        painel: PainelLateral,
        saida: &SaidaDivisoria,
        d: &Disposicao,
        com_esquerda: bool,
    ) {
        if saida.mudou {
            match painel {
                PainelLateral::Esquerdo => {
                    self.larguras.esquerda = saida.largura;
                    if d.com_direita {
                        self.larguras.direita = d.direita;
                    }
                }
                PainelLateral::Direito => {
                    self.larguras.direita = saida.largura;
                    if com_esquerda {
                        self.larguras.esquerda = d.esquerda;
                    }
                }
            }
            // a doca e o documento deste quadro já foram medidos: o quadro seguinte os desenha na largura nova
            ctx.request_repaint();
        }
        if saida.concluiu {
            self.guardar_larguras(ctx);
        }
    }

    /// Uma doca no retângulo dela (revisão de UI de 19/10/2026): a fileira de abas (só ícone à esquerda, com o nome à
    /// direita), o painel à vista no corpo e o que a pessoa fez nela. O clique numa aba a põe à vista; o "Destacar em
    /// janela" leva o painel à vista para a janela própria. Devolve a aba arrastada, se há uma, com o painel dela.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn doca(
        &mut self,
        raiz: &mut Ui,
        lado: Lado,
        rect: Rect,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) -> Option<(Painel, Arrasto)> {
        let ctx = raiz.ctx().clone();
        let tem_propriedades = self.abas.ativa().conteudo.tem_propriedades();
        let paineis = self.paineis.abas(lado, tem_propriedades);
        let ativo = self.paineis.ativo(lado, tem_propriedades)?;
        // a aba das Propriedades leva o objeto do documento à vista ("Propriedades · clientes")
        let objeto = {
            let a = self.abas.ativa();
            a.conteudo.titulo(a.rota(), a.numero_consulta(), &self.estado).0
        };
        let itens: Vec<AbaDoca<'_>> = paineis
            .iter()
            .map(|&p| AbaDoca::new(p.icone(), p.nome()).sub((p == Painel::Propriedades).then_some(objeto.as_str())))
            .collect();
        let forma = match lado {
            Lado::Esquerda => FormaAbas::SoIcone,
            Lado::Direita => FormaAbas::ComNome,
        };
        let indice = paineis.iter().position(|&p| p == ativo).unwrap_or(0);
        let mut ui_doca = raiz
            .new_child(UiBuilder::new().id_salt(("doca", lado)).max_rect(rect).layout(Layout::top_down(Align::Min)));
        let r = Doca::new(("doca", lado), lado.rotulo(), &itens).ativa(indice).forma(forma).mostrar(&mut ui_doca);
        if let Some(p) = r.clicada.and_then(|i| paineis.get(i)) {
            self.paineis.focar(*p);
            ctx.request_repaint();
        }
        let arrasto = r.arrasto.and_then(|a| paineis.get(a.aba).map(|p| (*p, a)));
        let mut ui_corpo = ui_doca.new_child(
            UiBuilder::new().id_salt(("doca-corpo", lado)).max_rect(r.corpo).layout(Layout::top_down(Align::Min)),
        );
        ui_corpo.set_clip_rect(r.corpo.intersect(raiz.clip_rect()));
        self.conteudo_do_painel(&mut ui_corpo, ativo, Encaixe::doca(r.acoes), frame, fontes, rota, acoes);
        if r.destacar {
            self.mover_painel(&ctx, ativo, Lugar::Janela);
        }
        arrasto
    }

    /// O corpo de um painel (o Explorador, o Histórico, as Propriedades do documento à vista) no espaço de `ui`, com
    /// as ações dele em `lugar_acoes` (a fileira da doca, ou o rodapé da janela própria).
    #[allow(clippy::too_many_arguments)]
    fn conteudo_do_painel(
        &mut self,
        ui: &mut Ui,
        painel: Painel,
        lugar_acoes: Encaixe,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) {
        let ctx = ui.ctx().clone();
        let (versao, trocando) = (self.versao, self.troca.is_some());
        match painel {
            Painel::Explorador => {
                let mut emitidas = Vec::new();
                let consulta = self.nome_da_consulta_da_transacao();
                let (estado, falha) = (&self.estado, self.falha_objetos.as_deref());
                self.casca.explorador(ui, lugar_acoes, estado, versao, rota, falha, &consulta, &mut emitidas);
                acoes.extend(emitidas.into_iter().map(|a| (None, a)));
                // etapa 4, passo 9: o "Mostrar na pasta" do menu de contexto do Explorador
                if let Some(caminho) = self.casca.revelar_do_explorador() {
                    self.revelar(&ctx, caminho);
                }
            }
            Painel::Historico => {
                let mut emitidas = Vec::new();
                let Editor { servico, estado, casca, .. } = self;
                let mut contexto = Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando);
                casca.historico.mostrar(ui, lugar_acoes, &mut contexto);
                acoes.extend(emitidas.into_iter().map(|a| (None, a)));
            }
            Painel::Propriedades => {
                let id_ativa = self.abas.id_ativa();
                let mut das_propriedades = Vec::new();
                let Editor { servico, estado, casca, abas, .. } = self;
                let mut contexto = Contexto::novo(servico, estado, frame, &mut das_propriedades)
                    .com_casca(versao, trocando)
                    .com_fontes(fontes);
                casca.propriedades(ui, lugar_acoes, &mut abas.ativa_mut().conteudo, id_ativa, &mut contexto);
                acoes.extend(das_propriedades.into_iter().map(|a| (Some(id_ativa), a)));
            }
        }
    }

    /// As janelas próprias dos painéis (revisão de UI de 19/10/2026, prancheta 6): uma janela do sistema, não modal,
    /// para cada painel que mora nela com a janela aberta (as Propriedades, só quando o documento à vista as tem). O
    /// fechar da janela (o × do sistema ou o "Fechar" do cabeçalho) devolve o painel escondido para a doca; o "Prender
    /// de volta" e a aba arrastada até a janela principal o prendem na doca, à vista.
    pub(super) fn janelas_dos_paineis(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
        principal: Rect,
    ) {
        let tem_propriedades =
            matches!(self.situacao, Situacao::Pronta) && self.abas.ativa().conteudo.tem_propriedades();
        let raiz = ctx.input(|i| i.viewport().inner_rect);
        for painel in Painel::TODOS {
            if !self.paineis.janela_aberta(painel) || (painel == Painel::Propriedades && !tem_propriedades) {
                continue;
            }
            let (largura, altura) = tamanho_da_janela(painel);
            let mut construtor = egui::ViewportBuilder::default()
                .with_title(format!("{} · {}", painel.nome(), comum::app::NOME))
                .with_inner_size([largura, altura])
                .with_min_inner_size([260.0, 220.0]);
            if let Some(icone) = interface::marca::icone_janela() {
                construtor = construtor.with_icon(Arc::new(icone));
            }
            if let Some((p, pos)) = self.posicao_da_janela
                && p == painel
            {
                construtor = construtor.with_position(pos);
                self.posicao_da_janela = None;
            }
            let id = egui::ViewportId::from_hash_of(("janela-do-painel", painel));
            let mut pedido = None;
            ctx.show_viewport_immediate(id, construtor, |ui, _| {
                pedido = self.janela_do_painel(ui, painel, frame, fontes, rota, acoes);
                if ui.input(|i| i.viewport().close_requested()) {
                    pedido = Some(PedidoDaJanela::Fechar);
                }
                // a aba solta fora da janela própria: em cima da janela principal, prende na doca daquele lado
                if let Some(PedidoDaJanela::Solta(ponteiro)) = pedido {
                    let tela = ui.input(|i| i.viewport().inner_rect).map_or(ponteiro, |r| r.min + ponteiro.to_vec2());
                    let alvo = raiz.unwrap_or(principal);
                    pedido = alvo.contains(tela).then(|| {
                        let lado = if tela.x < alvo.center().x { Lado::Esquerda } else { Lado::Direita };
                        PedidoDaJanela::Prender(Some(lado))
                    });
                }
            });
            match pedido {
                Some(PedidoDaJanela::Fechar) => {
                    let mudou = self.paineis.fechar_janela(painel);
                    self.depois_de_mover(ctx, mudou);
                }
                Some(PedidoDaJanela::Prender(Some(lado))) => self.mover_painel(ctx, painel, Lugar::Doca(lado)),
                Some(PedidoDaJanela::Prender(None)) => self.prender_painel(ctx, painel),
                Some(PedidoDaJanela::Solta(_)) | None => {}
            }
        }
    }

    /// O conteúdo da janela própria de um painel (prancheta 6): o cabeçalho de 50px com a aba do painel encostada
    /// embaixo (arrastá-la de volta prende), o "Prender de volta" (a silhueta espelhada) e o "Fechar"; o corpo do
    /// painel e, no Explorador, o rodapé de 58px com as ações dele.
    #[allow(clippy::too_many_arguments)]
    fn janela_do_painel(
        &mut self,
        ui: &mut Ui,
        painel: Painel,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) -> Option<PedidoDaJanela> {
        use interface::pecas::janela::{ALTURA_CABECALHO, ALTURA_RODAPE};
        let p = tema::paleta(ui);
        let e = &p.estudio;
        let rect = ui.available_rect_before_wrap();
        pintura::pintar_fundo(ui.painter(), p, rect);
        let cab = Rect::from_min_size(rect.min, vec2(rect.width(), ALTURA_CABECALHO));
        let com_rodape = painel == Painel::Explorador;
        let rodape = Rect::from_min_max(pos2(rect.left(), rect.bottom() - ALTURA_RODAPE), rect.max);
        let corpo = Rect::from_min_max(
            pos2(rect.left(), cab.bottom()),
            pos2(rect.right(), if com_rodape { rodape.top() } else { rect.bottom() }),
        );
        ui.painter().rect_filled(corpo, 0.0, e.painel);
        ui.painter().hline(cab.x_range(), cab.bottom() - 0.5, egui::Stroke::new(1.0, e.vidrob));
        if com_rodape {
            ui.painter().hline(rodape.x_range(), rodape.top() + 0.5, egui::Stroke::new(1.0, e.vidrob));
        }
        let mut pedido = None;
        // a aba (`padding:0 10px 0 12px`), com o objeto das Propriedades
        let objeto = {
            let a = self.abas.ativa();
            a.conteudo.titulo(a.rota(), a.numero_consulta(), &self.estado).0
        };
        let aba = AbaDoca::new(painel.icone(), painel.nome())
            .sub((painel == Painel::Propriedades).then_some(objeto.as_str()));
        let (_, arrasto) = interface::pecas::doca::aba_da_janela(
            ui,
            ("janela-do-painel", painel),
            &aba,
            pos2(cab.left() + 12.0, cab.bottom()),
        );
        if let Some(a) = arrasto
            && a.solto
        {
            pedido = Some(PedidoDaJanela::Solta(a.ponteiro));
        }
        // o Prender de volta e o Fechar, à direita (`gap:2px`)
        let dica_prender = match self.paineis.origem(painel) {
            Lado::Esquerda => "Prender de volta na doca da esquerda",
            Lado::Direita => "Prender de volta na doca da direita",
        };
        let botoes =
            Rect::from_min_max(pos2(cab.right() - 10.0 - 62.0, cab.top()), pos2(cab.right() - 10.0, cab.bottom()));
        let mut ui_botoes = ui.new_child(
            UiBuilder::new()
                .id_salt(("janela-do-painel-botoes", painel))
                .max_rect(botoes)
                .layout(Layout::right_to_left(Align::Center)),
        );
        ui_botoes.spacing_mut().item_spacing.x = 2.0;
        let fechar = interface::pecas::botao::Botao::icone(interface::icones::Icone::Fechar)
            .rotulo("Fechar")
            .dica("Fechar (o painel volta escondido para a doca)");
        if ui_botoes.add(fechar).clicked() {
            pedido = Some(PedidoDaJanela::Fechar);
        }
        let prender = interface::pecas::botao::Botao::icone(interface::icones::Icone::Prender)
            .rotulo("Prender de volta")
            .dica(dica_prender);
        if ui_botoes.add(prender).clicked() {
            pedido = Some(PedidoDaJanela::Prender(None));
        }
        let lugar_acoes = if com_rodape {
            // as ações no rodapé (`padding:0 14px 0 16px`), com os botões de 20px no meio da altura
            let meio = rodape.center().y;
            Rect::from_min_max(pos2(rodape.left() + 16.0, meio - 10.0), pos2(rodape.right() - 14.0, meio + 10.0))
        } else {
            Rect::NOTHING
        };
        let mut ui_corpo = ui.new_child(
            UiBuilder::new()
                .id_salt(("janela-do-painel-corpo", painel))
                .max_rect(corpo)
                .layout(Layout::top_down(Align::Min)),
        );
        ui_corpo.set_clip_rect(corpo);
        self.conteudo_do_painel(&mut ui_corpo, painel, Encaixe::janela(lugar_acoes), frame, fontes, rota, acoes);
        ui.allocate_rect(rect, egui::Sense::hover());
        pedido
    }

    /// Muda um painel de lugar (uma doca, a janela própria) e grava o lugar novo nas preferências, numa tarefa.
    pub(super) fn mover_painel(&mut self, ctx: &egui::Context, painel: Painel, destino: Lugar) {
        let mudou = self.paineis.mover(painel, destino);
        self.depois_de_mover(ctx, mudou);
    }

    /// Prende de volta, na doca de onde saiu, o painel da janela própria.
    fn prender_painel(&mut self, ctx: &egui::Context, painel: Painel) {
        let mudou = self.paineis.prender(painel);
        self.depois_de_mover(ctx, mudou);
    }

    /// O Exibir › Barra de status: mostra ou esconde a barra e grava a escolha nas preferências, numa tarefa.
    pub(super) fn alternar_barra_status(&mut self, ctx: &egui::Context) {
        self.barra_status = !self.barra_status;
        let mudanca = prefs::Mudanca::barra_status(self.barra_status);
        let promessa = self.tarefa(ctx, move |s, _| s.gravar_preferencias(mudanca).map(|_| ()));
        self.tarefas.push(Tarefa::Gravar(promessa));
        ctx.request_repaint();
    }

    fn depois_de_mover(&mut self, ctx: &egui::Context, mudou: bool) {
        if mudou {
            let mudanca = prefs::Mudanca::lugares(self.paineis.para_prefs());
            let promessa = self.tarefa(ctx, move |s, _| s.gravar_preferencias(mudanca).map(|_| ()));
            self.tarefas.push(Tarefa::Gravar(promessa));
        }
        ctx.request_repaint();
    }

    /// Grava nas preferências as larguras escolhidas que ainda não estão lá (só as que mudaram), numa tarefa. Uma
    /// gravação por vez: duas em voo, de threads diferentes, podiam terminar fora de ordem e deixar no arquivo a
    /// largura mais velha (o dedo na seta, uma tecla por quadro); o que a pessoa mexeu enquanto uma corria sai quando
    /// ela termina.
    pub(super) fn guardar_larguras(&mut self, ctx: &egui::Context) {
        if self.gravando_larguras || self.larguras == self.larguras_guardadas {
            return;
        }
        let (agora, antes) = (self.larguras, self.larguras_guardadas);
        let mudanca = prefs::Mudanca::paineis(prefs::MudancaLarguraPaineis {
            explorador: (agora.esquerda != antes.esquerda).then_some(agora.esquerda),
            propriedades: (agora.direita != antes.direita).then_some(agora.direita),
        });
        self.larguras_guardadas = agora;
        self.gravando_larguras = true;
        self.tarefas.push(Tarefa::Larguras(self.tarefa(ctx, move |s, _| s.gravar_preferencias(mudanca))));
    }
}
