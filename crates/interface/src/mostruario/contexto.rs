//! A página do menu de contexto e dos cartões de ação no mostruário (as pranchetas 15 e 4): o Explorador de exemplo
//! com a pasta, um CSV, um texto e um banco, cada um com o seu menu de contexto (clique direito ou Shift+F10 no item),
//! e a fileira dos cartões de ação do Início. As ações só mudam a linha "Última ação".

use egui::{Align, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use super::Mostruario;
use crate::icones::{Icone, TipoArquivo};
use crate::pecas::acao::Acao;
use crate::pecas::arranjo::fluxo;
use crate::pecas::arvore::{No, Seta, recuo};
use crate::pecas::cartao::Cartao;
use crate::pecas::casca::PainelMenu;
use crate::pecas::menu_contexto::MenuContexto;
use crate::pecas::painel::Painel;
use crate::pecas::texto::{h2, hint};
use crate::pecas::toast::toast_concluido;

/// O estado da página.
pub(super) struct EstadoContexto {
    ultima_acao: String,
}

impl EstadoContexto {
    pub(super) fn new() -> Self {
        EstadoContexto { ultima_acao: "nenhuma".into() }
    }
}

/// O que o item do Explorador de exemplo é: muda os itens do menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    /// A raiz da pasta aberta: Atualizar, Mostrar na pasta, Copiar caminho e, depois de um fio, Fechar pasta.
    Raiz,
    /// Um arquivo ou banco: Abrir, Mostrar na pasta, Copiar caminho.
    Arquivo,
}

const LARGURA_EXPLORADOR: f32 = 258.0;
const ALTURA_EXPLORADOR: f32 = 168.0;

/// Os itens do menu de um nó, devolvendo o nome da ação escolhida.
fn itens_do_menu(m: &mut PainelMenu<'_>, item: Item) -> Option<&'static str> {
    let mut acao = None;
    if item == Item::Raiz {
        if m.silhueta(Icone::Atualizar).item("Atualizar", Some("F5"), false).clicked() {
            acao = Some("Atualizar");
        }
    } else if m.item("Abrir", None, false).clicked() {
        acao = Some("Abrir");
    }
    if m.silhueta(Icone::MostrarNaPasta).item("Mostrar na pasta", None, false).clicked() {
        acao = Some("Mostrar na pasta");
    }
    if m.silhueta(Icone::CopiarCaminho).item("Copiar caminho", Some("Ctrl+Shift+C"), false).clicked() {
        acao = Some("Copiar caminho");
    }
    if item == Item::Raiz {
        m.separador();
        if m.item_perigo("Fechar pasta", None, false).clicked() {
            acao = Some("Fechar pasta");
        }
    }
    acao
}

impl Mostruario {
    /// Mostra a página do menu de contexto e dos cartões ([`Mostruario::pagina_contexto`]) no lugar das peças.
    pub fn mostrar_contexto(&mut self, sim: bool) {
        self.vista_contexto = sim;
    }

    /// A janela mostra a página do menu de contexto e dos cartões?
    pub fn contexto_a_vista(&self) -> bool {
        self.vista_contexto
    }

    /// O menu de contexto sobre o Explorador de exemplo (prancheta 15).
    pub fn pagina_contexto(&mut self, ui: &mut Ui) {
        let largura = ui.available_width().min(960.0);
        let topo = ui.cursor().min;
        let rect = Rect::from_min_size(pos2(topo.x + 40.0, topo.y + 30.0), vec2(largura - 80.0, f32::INFINITY));
        let mut ui =
            ui.new_child(UiBuilder::new().id_salt("contexto").max_rect(rect).layout(Layout::top_down(Align::Min)));
        h2(&mut ui, "Menu de contexto e cartões de ação");
        ui.add_space(12.0);
        self.card_menu_contexto(&mut ui);
        ui.add_space(16.0);
        self.card_acoes(&mut ui);
    }

    /// Os três cartões de ação do Início (prancheta 4): Abrir pasta (o primário), Abrir arquivo e Novo banco.
    fn card_acoes(&mut self, ui: &mut Ui) {
        Cartao::new("acoes")
            .titulo("Cartões de ação")
            .hint("o .acao do Início: 220px, ícone redondo de 36, atalho no .kbd; o primário usa o degradê")
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    let mut escolhida = None;
                    fluxo(ui, 12.0, |ui| {
                        for (acao, nome) in [
                            (
                                Acao::new("Abrir pasta", Icone::Pasta).atalho("Ctrl+Shift+O").primaria(true),
                                "Abrir pasta",
                            ),
                            (Acao::new("Abrir arquivo", Icone::Arquivo).atalho("Ctrl+O"), "Abrir arquivo"),
                            (Acao::new("Novo banco", Icone::NovoBanco), "Novo banco"),
                            (Acao::new("Desligado", Icone::Arquivo).atalho("Ctrl+O").habilitado(false), "Desligado"),
                        ] {
                            if ui.add(acao).clicked() {
                                escolhida = Some(nome);
                            }
                        }
                    });
                    if let Some(nome) = escolhida {
                        self.contexto.ultima_acao = format!("cartão {nome}");
                    }
                    ui.add_space(8.0);
                    hint(ui, &format!("Última ação: {}", self.contexto.ultima_acao));
                });
            });
    }

    fn card_menu_contexto(&mut self, ui: &mut Ui) {
        Cartao::new("menu-contexto")
            .titulo("Menu de contexto")
            .hint("clique direito ou Shift+F10 no item · .menu-ctx: 250px, vidro fosco, itens de 28px")
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    let topo = ui.cursor().min;
                    let explorador = Rect::from_min_size(topo, vec2(LARGURA_EXPLORADOR, ALTURA_EXPLORADOR));
                    let mut filho = ui.new_child(UiBuilder::new().id_salt("explorador").max_rect(explorador));
                    let estado = &mut self.contexto;
                    let ctx = ui.ctx().clone();
                    Painel::new("contexto::explorador", "Explorador").mostrar(&mut filho, |ui| {
                        ui.add_space(6.0);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let nos = [
                            ("dados", Icone::PastaSolida, 0, Seta::Aberta, Item::Raiz),
                            ("vendas_2025.csv", Icone::Folha(TipoArquivo::Csv), 1, Seta::Vazia, Item::Arquivo),
                            ("leia-me.txt", Icone::Folha(TipoArquivo::Txt), 1, Seta::Vazia, Item::Arquivo),
                            ("exemplo.db", Icone::Banco, 1, Seta::Fechada, Item::Arquivo),
                        ];
                        for (nome, icone, nivel, seta, item) in nos {
                            let no = No::new(nome).recuo(recuo(nivel)).seta(seta).icone(icone).mostrar(ui);
                            let escolhida = MenuContexto::new(("contexto", nome))
                                .rotulo(nome)
                                .mostrar(&no.resposta, |m| itens_do_menu(m, item))
                                .flatten();
                            if let Some(acao) = escolhida {
                                estado.ultima_acao = format!("{acao} em {nome}");
                                if acao == "Copiar caminho" {
                                    toast_concluido(&ctx, "Caminho copiado.");
                                }
                            }
                        }
                    });
                    ui.allocate_rect(explorador, Sense::hover());
                    ui.add_space(8.0);
                    hint(ui, &format!("Última ação: {}", self.contexto.ultima_acao));
                });
            });
    }
}
