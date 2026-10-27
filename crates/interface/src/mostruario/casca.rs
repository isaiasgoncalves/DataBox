//! A casca nova do DataBox no mostruário: a janela inteira das pranchetas (`Main.dc.html` e `Dados.dc.html`),
//! montada só com as peças, com dados inventados — a barra de título com os menus, a barra de ferramentas, a barra
//! de ícones, o Explorador (ou o Histórico), as abas com o documento, as Propriedades e a barra de status. Os
//! cliques mexem só no mostruário (abrir e fechar nós e abas, trocar o tema, esconder painéis); os botões da janela
//! mostram um aviso.

use egui::{Align, CornerRadius, Layout, Rangef, Rect, Shape, Stroke, Ui, UiBuilder, pos2, vec2};

use super::{DesenhoDados, Mostruario, nucleo_dados};
use crate::fontes::{self, Peso};
use crate::grade::NucleoGrade;
use crate::icones::{self, Icone};
use crate::pecas::abas_documento::{ALTURA_FAIXA, AbaDocumento, FaixaAbas, pintar_documento};
use crate::pecas::arvore::{No, Seta, recuo};
use crate::pecas::avisos::vazio;
use crate::pecas::barra_status::{ALTURA_STATUS, ItemStatus, barra_status};
use crate::pecas::barra_titulo::{
    ALTURA_TITULO, BotaoJanela, LARGURA_BOTAO_JANELA, botao_janela_em, marca, titulo_centrado,
};
use crate::pecas::botao::Botao;
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::casca::Menu;
use crate::pecas::detalhes::{Detalhes, detalhes};
use crate::pecas::dock::{ItemDock, dock};
use crate::pecas::ferramentas::{ALTURA_BARRA, barra_ferramentas, busca_desligada, separador};
use crate::pecas::painel::{Painel, Secao, pintar_painel_fixo};
use crate::pecas::rotulos::{TipoPill, pill};
use crate::pecas::texto::galley;
use crate::pecas::texto_pre::TextoPre;
use crate::pecas::toast::toast;
use crate::pintura::{self, Preenchimento};
use crate::tema::{self, Tema, paleta};

/// As tabelas do banco de exemplo (`Main.dc.html`): nome, linhas e o ícone do tipo.
const TABELAS: [(&str, &str, Icone); 7] = [
    ("clientes", "60", Icone::Tabela),
    ("eventos", "5.000", Icone::Tabela),
    ("itens_pedido", "456", Icone::Tabela),
    ("notas", "2", Icone::TabelaVirtual),
    ("pedidos", "240", Icone::Tabela),
    ("produtos", "10", Icone::TabelaSemRowid),
    ("Tabela com espaço", "4", Icone::Tabela),
];

/// As partes de cada tabela no Explorador.
const PARTES: [(&str, Icone); 4] = [
    ("Dados", Icone::Dados),
    ("Estrutura", Icone::Estrutura),
    ("Índices e gatilhos", Icone::Indices),
    ("Definição SQL", Icone::Definicao),
];

const BANCO: &str = "exemplo.db";
const CAMINHO: &str = "C:\\dados\\exemplo.db";

/// Os contadores da dock da Visão geral (`Main.dc.html`), no singular com um.
fn itens_da_dock() -> [ItemDock; 6] {
    [
        ItemDock::new(Icone::TabelaGrande, "7", "Tabelas"),
        ItemDock::new(Icone::VisaoGrande, "1", "Visão"),
        ItemDock::new(Icone::IndiceGrande, "5", "Índices"),
        ItemDock::new(Icone::GatilhoGrande, "1", "Gatilho"),
        ItemDock::new(Icone::DiscoGrande, "500", "Tamanho").unidade("KB"),
        ItemDock::new(Icone::LinhasGrande, "5.772", "Linhas"),
    ]
}
const FRASE_DO_MODO: &str = "Você está editando exemplo.db. Tudo o que for salvo grava no arquivo na hora.";

/// Uma aba aberta no mostruário.
#[derive(Debug, Clone, PartialEq)]
struct Aba {
    icone: Icone,
    nome: String,
    sub: Option<&'static str>,
}

impl Aba {
    fn new(icone: Icone, nome: &str, sub: Option<&'static str>) -> Self {
        Aba { icone, nome: nome.to_owned(), sub }
    }
}

/// O estado da casca do mostruário.
pub(super) struct EstadoCasca {
    explorador: bool,
    historico: bool,
    propriedades: bool,
    /// Os nós abertos da árvore (pelo rótulo).
    abertos: Vec<String>,
    abas: Vec<Aba>,
    ativa: usize,
    grade: NucleoGrade,
    marcadas: [bool; 5],
}

impl EstadoCasca {
    pub(super) fn new() -> Self {
        EstadoCasca {
            explorador: true,
            historico: false,
            propriedades: true,
            abertos: ["exemplo.db", "Tabelas", "clientes"].map(str::to_owned).to_vec(),
            abas: vec![
                Aba::new(Icone::Banco, BANCO, Some("Visão geral")),
                Aba::new(Icone::Tabela, "clientes", Some("Dados")),
                Aba::new(Icone::Tabela, "clientes", Some("Estrutura")),
                Aba::new(Icone::Consulta, "Consulta 1", None),
            ],
            ativa: 1,
            grade: nucleo_dados("mostruario-casca-dados"),
            marcadas: [false, true, false, true, false],
        }
    }

    fn aba(&self) -> &Aba {
        &self.abas[self.ativa.min(self.abas.len() - 1)]
    }

    /// Foca a aba igual ou abre uma nova no fim.
    fn abrir(&mut self, aba: Aba) {
        self.ativa = match self.abas.iter().position(|a| *a == aba) {
            Some(i) => i,
            None => {
                self.abas.push(aba);
                self.abas.len() - 1
            }
        };
    }

    /// Abre a próxima "Consulta N".
    fn nova_consulta(&mut self) {
        let n = self.abas.iter().filter(|a| a.icone == Icone::Consulta).count() + 1;
        self.abrir(Aba::new(Icone::Consulta, &format!("Consulta {n}"), None));
    }

    fn fechar(&mut self, i: usize) {
        if i < self.abas.len() {
            self.abas.remove(i);
        }
        if self.abas.is_empty() {
            self.abas.push(Aba::new(Icone::Casa, "Início", None));
        }
        if self.ativa >= i && self.ativa > 0 {
            self.ativa -= 1;
        }
        self.ativa = self.ativa.min(self.abas.len() - 1);
    }

    fn aberto(&self, chave: &str) -> bool {
        self.abertos.iter().any(|a| a == chave)
    }

    fn alternar(&mut self, chave: &str) {
        match self.abertos.iter().position(|a| a == chave) {
            Some(i) => {
                self.abertos.remove(i);
            }
            None => self.abertos.push(chave.to_owned()),
        }
    }

    fn seta(&self, chave: &str) -> Seta {
        if self.aberto(chave) { Seta::Aberta } else { Seta::Fechada }
    }
}

impl Mostruario {
    /// A casca inteira na janela (o `ui` ocupa a janela toda; quem chama já pintou o fundo).
    pub fn pagina_casca(&mut self, ui: &mut Ui) {
        let tela = ui.max_rect();
        self.barra_de_titulo(ui, tela);
        let barra = Rect::from_min_size(
            pos2(tela.left() + 8.0, tela.top() + ALTURA_TITULO + 6.0),
            vec2(tela.width() - 16.0, ALTURA_BARRA),
        );
        let mut ui_barra = ui.new_child(UiBuilder::new().id_salt("casca::ferramentas").max_rect(barra));
        self.ferramentas(&mut ui_barra);

        // o corpo: `padding:8px;gap:8px`, a barra de status embaixo com `margin:0 8px 8px`
        let topo_status = tela.bottom() - 8.0 - ALTURA_STATUS;
        let corpo = Rect::from_min_max(
            pos2(tela.left() + 8.0, barra.bottom() + 8.0),
            pos2(tela.right() - 8.0, topo_status - 8.0),
        );
        let rail = Rect::from_min_size(corpo.min, vec2(52.0, corpo.height()));
        self.barra_de_icones(ui, rail);
        let mut esquerda = rail.right() + 8.0;
        if self.casca.explorador || self.casca.historico {
            let r = Rect::from_min_size(pos2(esquerda, corpo.top()), vec2(258.0, corpo.height()));
            let mut ui_painel = ui.new_child(UiBuilder::new().id_salt("casca::esquerdo").max_rect(r));
            if self.casca.historico {
                self.historico(&mut ui_painel);
            } else {
                self.explorador(&mut ui_painel);
            }
            esquerda = r.right() + 8.0;
        }
        // as Propriedades somem quando o documento ficaria com menos de 560px (`flex:999 1 560px`)
        let com_propriedades = self.casca.propriedades && corpo.right() - 272.0 - 8.0 - esquerda >= 560.0;
        let direita = if com_propriedades { corpo.right() - 272.0 - 8.0 } else { corpo.right() };
        self.documento(ui, Rect::from_min_max(pos2(esquerda, corpo.top()), pos2(direita, corpo.bottom())));
        if com_propriedades {
            let r = Rect::from_min_max(pos2(direita + 8.0, corpo.top()), corpo.max);
            let mut ui_prop = ui.new_child(UiBuilder::new().id_salt("casca::propriedades").max_rect(r));
            self.propriedades(&mut ui_prop);
        }
        let r_status =
            Rect::from_min_size(pos2(tela.left() + 8.0, topo_status), vec2(tela.width() - 16.0, ALTURA_STATUS));
        let mut ui_status = ui.new_child(UiBuilder::new().id_salt("casca::status").max_rect(r_status));
        self.status(&mut ui_status);
        ui.advance_cursor_after_rect(tela);
    }

    /// A barra de título com os menus, no alto de `tela` (também no alto da página das peças).
    pub(super) fn barra_de_titulo(&mut self, ui: &mut Ui, tela: Rect) {
        let faixa = Rect::from_min_size(tela.min, vec2(tela.width(), ALTURA_TITULO));
        let mut linha = ui.new_child(
            UiBuilder::new().id_salt("casca::titulo").max_rect(faixa).layout(Layout::left_to_right(Align::Center)),
        );
        linha.spacing_mut().item_spacing.x = 0.0;
        marca(&mut linha);
        linha.spacing_mut().item_spacing.x = 2.0;
        self.menus(&mut linha);
        let fim_menus = linha.min_rect().right();
        let x_botoes = faixa.right() - 3.0 * LARGURA_BOTAO_JANELA;
        titulo_centrado(ui, faixa, Rangef::new(fim_menus, x_botoes), "exemplo.db · DataBox", true);
        let botoes = [
            (BotaoJanela::Minimizar, "Minimizaria a janela."),
            (BotaoJanela::Maximizar, "Maximizaria a janela."),
            (BotaoJanela::Fechar, "Fecharia o DataBox."),
        ];
        for (i, (botao, aviso)) in botoes.into_iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(x_botoes + LARGURA_BOTAO_JANELA * i as f32, faixa.top()),
                vec2(LARGURA_BOTAO_JANELA, ALTURA_TITULO),
            );
            if botao_janela_em(ui, ui.id().with(("casca::janela", i)), r, botao, true).clicked() {
                toast(ui.ctx(), aviso);
            }
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        // o Arquivo com os sólidos de 24 e as linhas de 36 (`.menu-ic`)
        let _ = Menu::new("casca::arquivo", "Arquivo").largura(300.0).mostrar(ui, |m| {
            m.com_icones();
            m.icone(Icone::PastaSolida).item_desligado("Abrir pasta", Some("Ctrl+Shift+O"));
            m.icone(Icone::DocumentoSolido).item("Abrir arquivo", Some("Ctrl+O"), false);
            m.icone(Icone::NovoBancoSolido).item("Novo banco", None, false);
            m.separador();
            m.icone(Icone::Recentes).submenu("Abertos recentemente", 310.0, |m| {
                m.com_icones();
                m.item_duplo(Some(Icone::Banco), BANCO, "C:\\dados");
                m.item_duplo(Some(Icone::BancoSoLeitura), "financeiro.sqlite", "D:\\relatorios");
                m.item_duplo(Some(Icone::Banco), "estoque_2026.db", "C:\\dados\\arquivo\\2026");
            });
            m.separador();
            m.icone(Icone::DisqueteSolido).item_desligado("Salvar", Some("Ctrl+S"));
            m.item("Fechar aba", Some("Ctrl+W"), false);
            m.item("Fechar banco", None, false);
            m.item_desligado("Fechar pasta", None);
        });
        let _ = Menu::new("casca::editar", "Editar").mostrar(ui, |m| {
            m.item_desligado("Desfazer", Some("Ctrl+Z"));
            m.item_desligado("Refazer", Some("Ctrl+Y"));
            m.separador();
            m.item("Nova tabela", None, false);
            m.item_desligado("Duplicar", None);
            m.item_desligado("Excluir", None);
            m.separador();
            m.item_desligado("Buscar", Some("Ctrl+F"));
            m.item_desligado("Filtrar", None);
            m.item("Atualizar", Some("F5"), false);
            m.item_desligado("Executar", Some("Ctrl+Enter"));
            m.item("Exportar", None, false);
        });
        let estado = &mut self.casca;
        let mut tema = self.tema;
        let _ = Menu::new("casca::exibir", "Exibir").largura(262.0).mostrar(ui, |m| {
            m.com_marcas();
            if m.item_marcado("Explorador", Some("Ctrl+B"), estado.explorador && !estado.historico).clicked() {
                estado.explorador = estado.historico || !estado.explorador;
                estado.historico = false;
            }
            if m.item_marcado("Propriedades", Some("Ctrl+Alt+B"), estado.propriedades).clicked() {
                estado.propriedades = !estado.propriedades;
            }
            if m.item_marcado("Histórico de consultas", None, estado.historico).clicked() {
                estado.historico = !estado.historico;
            }
            m.separador();
            m.submenu("Tema", 210.0, |m| {
                for (t, nome) in
                    [(Tema::Sistema, comum::app::SEGUIR_O_SISTEMA), (Tema::Claro, "Claro"), (Tema::Escuro, "Escuro")]
                {
                    if m.item_opcao(nome, tema == t).clicked() {
                        tema = t;
                    }
                }
            });
        });
        if tema != self.tema {
            self.tema = tema;
            tema::aplicar(ui.ctx(), tema);
        }
        let mut voltar = false;
        let _ = Menu::new("casca::ajuda", "Ajuda").mostrar(ui, |m| {
            m.item("Atalhos do teclado", None, false);
            m.item("Sobre o DataBox", None, false);
            voltar = m.item("Mostruário das peças", None, true).clicked();
        });
        if voltar {
            self.vista_casca = false;
        }
    }

    /// A barra de ferramentas universal (também na página das peças).
    pub(super) fn ferramentas(&mut self, ui: &mut Ui) {
        let aba = self.casca.aba().clone();
        let propriedades = self.casca.propriedades;
        let (mut nova, mut alternar_painel) = (false, false);
        let visao_geral = aba.sub == Some("Visão geral");
        let consulta = aba.icone == Icone::Consulta;
        let inicio = aba.icone == Icone::Casa;
        let exportar = if visao_geral { "Exportar o banco".to_owned() } else { format!("Exportar {}", aba.nome) };
        barra_ferramentas(
            ui,
            |ui| {
                ui.add(Botao::icone(Icone::Voltar).dica("Voltar (Alt+←)"));
                ui.add(Botao::icone(Icone::Avancar).dica("Avançar (Alt+→)").habilitado(false));
                separador(ui);
                ui.add(Botao::icone(Icone::Pasta).dica("Abrir pasta").habilitado(false));
                ui.add(Botao::icone(Icone::Arquivo).dica("Abrir arquivo (Ctrl+O)"));
                ui.add(Botao::icone(Icone::Salvar).dica("Salvar (Ctrl+S)").habilitado(false));
                separador(ui);
                let novo = Botao::icone(Icone::Novo).rotulo("Nova tabela").dica("Nova tabela");
                ui.add(novo.habilitado(visao_geral));
                // o lugar fixo da Nova coluna, logo depois do Novo (desligado na etapa 2)
                ui.add(Botao::icone(Icone::NovaColuna).dica("Nova coluna").habilitado(false));
                ui.add(Botao::icone(Icone::Duplicar).dica("Duplicar").habilitado(false));
                ui.add(Botao::icone(Icone::Excluir).dica("Excluir").habilitado(false));
                ui.add(Botao::icone(Icone::Desfazer).dica("Desfazer (Ctrl+Z)").habilitado(false));
                ui.add(Botao::icone(Icone::Refazer).dica("Refazer (Ctrl+Y)").habilitado(false));
                separador(ui);
                busca_desligada(ui);
                ui.add(Botao::icone(Icone::Filtrar).dica("Filtrar").habilitado(false));
                ui.add(Botao::icone(Icone::Atualizar).dica("Atualizar (F5)").habilitado(!inicio && !consulta));
                separador(ui);
                // o principal (`.ib.pri`) só na Consulta; nas outras abas, o `.ib` desligado
                let executar = Botao::icone(Icone::Executar)
                    .dica("Executar (Ctrl+Enter); com texto selecionado, executa só o trecho");
                ui.add(if consulta { executar.principal() } else { executar.habilitado(false) });
                nova = ui.add(Botao::icone(Icone::NovaConsulta).dica("Nova consulta SQL")).clicked();
                ui.add(Botao::icone(Icone::Exportar).dica(&exportar).habilitado(!inicio && !consulta));
            },
            |ui| {
                let painel = Botao::icone(Icone::Painel).selecionado(propriedades).dica("Painel de propriedades");
                alternar_painel = ui.add(painel).clicked();
            },
        );
        if nova {
            self.casca.nova_consulta();
        }
        if alternar_painel {
            self.casca.propriedades = !propriedades;
        }
    }

    fn barra_de_icones(&mut self, ui: &mut Ui, rect: Rect) {
        pintar_painel_fixo(ui.painter(), paleta(ui), rect, RAIO_CARD);
        let dentro = Rect::from_min_max(pos2(rect.left(), rect.top() + 8.0), pos2(rect.right(), rect.bottom() - 8.0));
        let mut ui_rail = ui.new_child(
            UiBuilder::new().id_salt("casca::icones").max_rect(dentro).layout(Layout::top_down(Align::Center)),
        );
        ui_rail.spacing_mut().item_spacing.y = 6.0;
        let estado = &mut self.casca;
        let inicio = estado.aba().icone == Icone::Casa;
        if ui_rail.add(Botao::icone(Icone::Inicio).rail().selecionado(inicio).dica("Início")).clicked() {
            estado.abrir(Aba::new(Icone::Casa, "Início", None));
        }
        let explorador = estado.explorador && !estado.historico;
        if ui_rail.add(Botao::icone(Icone::Explorador).rail().selecionado(explorador).dica("Explorador")).clicked() {
            estado.explorador = !explorador;
            estado.historico = false;
        }
        let historico =
            Botao::icone(Icone::Historico).rail().selecionado(estado.historico).dica("Histórico de consultas");
        if ui_rail.add(historico).clicked() {
            estado.historico = !estado.historico;
        }
    }

    /// O Explorador com o banco de exemplo (também à esquerda da página das peças).
    pub(super) fn explorador(&mut self, ui: &mut Ui) {
        let estado = &mut self.casca;
        let ativa = estado.aba().clone();
        let ativo = |nome: &str, sub: &str| ativa.nome == nome && ativa.sub == Some(sub);
        let mut recolher = false;
        let mut alternar: Option<String> = None;
        let mut abrir: Option<Aba> = None;
        Painel::new("casca::explorador", "Explorador")
            .acoes(|ui| {
                recolher = ui.add(Botao::icone(Icone::Recolher).do_cabecalho().dica("Recolher tudo")).clicked();
                ui.add(Botao::icone(Icone::Pasta).do_cabecalho().rotulo("Abrir pasta no Explorador").habilitado(false));
            })
            .mostrar(ui, |ui| {
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let banco = No::new(BANCO)
                    .recuo(recuo(0))
                    .seta(estado.seta(BANCO))
                    .icone(Icone::Banco)
                    .forte()
                    .dica(CAMINHO)
                    .ativo(ativo(BANCO, "Visão geral"))
                    .mostrar(ui);
                if banco.alternou {
                    alternar = Some(BANCO.to_owned());
                } else if banco.resposta.clicked() {
                    abrir = Some(Aba::new(Icone::Banco, BANCO, Some("Visão geral")));
                }
                if !estado.aberto(BANCO) {
                    return;
                }
                let grupos: [(&str, &str); 3] = [("Tabelas", "7"), ("Visões", "1"), ("Sistema", "2")];
                for (grupo, contagem) in grupos {
                    let r = No::new(grupo).recuo(recuo(1)).seta(estado.seta(grupo)).contagem(contagem).mostrar(ui);
                    if r.alternou || r.resposta.clicked() {
                        alternar = Some(grupo.to_owned());
                    }
                    if !estado.aberto(grupo) {
                        continue;
                    }
                    let objetos: Vec<(&str, &str, Icone)> = match grupo {
                        "Tabelas" => TABELAS.to_vec(),
                        "Visões" => vec![("v_pedidos_por_cliente", "—", Icone::Visao)],
                        _ => vec![("sqlite_sequence", "3", Icone::Sistema), ("sqlite_stat1", "5", Icone::Sistema)],
                    };
                    for (nome, linhas, icone) in objetos {
                        let tem_partes = icone != Icone::Sistema;
                        let seta = if tem_partes { estado.seta(nome) } else { Seta::Vazia };
                        let r = No::new(nome).recuo(recuo(2)).seta(seta).icone(icone).contagem(linhas).mostrar(ui);
                        if r.alternou {
                            alternar = Some(nome.to_owned());
                        } else if r.resposta.clicked() {
                            if tem_partes && !estado.aberto(nome) {
                                alternar = Some(nome.to_owned());
                            }
                            abrir = Some(Aba::new(icone, nome, Some("Dados")));
                        }
                        if !(tem_partes && estado.aberto(nome)) {
                            continue;
                        }
                        // a visão só tem Dados e Definição SQL
                        let partes = if icone == Icone::Visao { vec![PARTES[0], PARTES[3]] } else { PARTES.to_vec() };
                        for (parte, icone_parte) in partes {
                            let rotulo = format!("{nome} · {parte}");
                            let r = No::new(parte)
                                .rotulo(&rotulo)
                                .recuo(recuo(3))
                                .seta(Seta::Vazia)
                                .icone(icone_parte)
                                .ativo(ativo(nome, parte))
                                .mostrar(ui);
                            if r.resposta.clicked() {
                                abrir = Some(Aba::new(icone, nome, Some(parte)));
                            }
                        }
                    }
                }
            });
        if recolher {
            estado.abertos.clear();
        }
        if let Some(chave) = alternar {
            estado.alternar(&chave);
        }
        if let Some(aba) = abrir {
            estado.abrir(aba);
        }
    }

    fn historico(&mut self, ui: &mut Ui) {
        let comandos = [
            "SELECT * FROM \"clientes\" WHERE \"cidade\" = 'Recife'",
            "SELECT \"cidade\", COUNT(*) FROM \"clientes\" GROUP BY \"cidade\"",
            "UPDATE \"pedidos\" SET \"total\" = 0 WHERE \"id\" = 7",
            "BEGIN",
        ];
        let estado = &mut self.casca;
        let mut abrir = None;
        Painel::new("casca::historico", "Histórico").mostrar(ui, |ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, sql) in comandos.iter().enumerate() {
                let rotulo = format!("Comando {}", i + 1);
                if No::new(sql).rotulo(&rotulo).icone(Icone::Consulta).mostrar(ui).resposta.clicked() {
                    abrir = Some(i);
                }
            }
        });
        if abrir.is_some() {
            estado.nova_consulta();
        }
    }

    fn documento(&mut self, ui: &mut Ui, secao: Rect) {
        let p = paleta(ui);
        let e = &p.estudio;
        let estado = &mut self.casca;
        let faixa = Rect::from_min_size(secao.min, vec2(secao.width(), ALTURA_FAIXA));
        let mut ui_faixa = ui.new_child(UiBuilder::new().id_salt("casca::abas").max_rect(faixa));
        let sozinha = estado.abas.len() == 1;
        let abas: Vec<AbaDocumento<'_>> = estado
            .abas
            .iter()
            .map(|a| {
                let aba = AbaDocumento::new(a.icone, &a.nome).fechavel(!(sozinha && a.icone == Icone::Casa));
                match a.sub {
                    Some(s) => aba.sub(s),
                    None => aba,
                }
            })
            .collect();
        let r = FaixaAbas::new("casca", &abas).ativa(Some(estado.ativa)).nova(true).mostrar(&mut ui_faixa);
        drop(abas);
        let doc = Rect::from_min_max(pos2(secao.left(), faixa.bottom() - 1.0), secao.max);
        pintar_documento(ui.painter(), p, doc, r.juncao);

        let aba = estado.aba().clone();
        let tem_faixa = aba.icone != Icone::Casa;
        let mut topo = doc.top() + 1.0;
        if tem_faixa {
            // a faixa de contexto do documento (`.faixa`: 42px, o degradê meio → `--bt2` com as riscas, o fio
            // `--btb` embaixo), que forma uma superfície só com a aba ativa
            let r_faixa = Rect::from_min_max(pos2(doc.left() + 1.0, topo), pos2(doc.right() - 1.0, topo + 42.0));
            let raio = (RAIO_CARD - 1.0) as u8;
            let cantos = CornerRadius { nw: raio, ne: raio, sw: 0, se: 0 };
            let mut fundo = pintura::forma_preenchida(ui.ctx(), p, r_faixa, 0.0, Preenchimento::Cabecalho);
            if let Shape::Rect(f) = &mut fundo {
                f.corner_radius = cantos;
            }
            ui.painter().add(fundo);
            pintura::riscas_com_canto(ui.painter(), p, r_faixa, cantos);
            ui.painter().hline(r_faixa.x_range(), r_faixa.bottom() - 0.5, Stroke::new(1.0, e.btb));
            let mut x = r_faixa.left() + 14.0;
            icones::pintar(
                ui.painter(),
                Rect::from_min_size(pos2(x, r_faixa.center().y - 8.0), vec2(16.0, 16.0)),
                aba.icone,
                e.t2,
            );
            x += 16.0 + 8.0;
            let g = galley(ui, &aba.nome, fontes::fonte(13.0, Peso::Medio), e.t);
            let largura = g.size().x;
            ui.painter().galley(pos2(x, r_faixa.center().y - g.size().y / 2.0), g, e.t);
            x += largura + 8.0;
            let gc = galley(ui, CAMINHO, fontes::fonte(12.0, Peso::Leve), e.t3);
            ui.painter().galley(pos2(x, r_faixa.center().y - gc.size().y / 2.0), gc, e.t3);
            topo = r_faixa.bottom();
        }
        let corpo =
            Rect::from_min_max(pos2(doc.left() + 20.0, topo + 18.0), pos2(doc.right() - 20.0, doc.bottom() - 20.0));
        let mut ui_corpo = ui.new_child(
            UiBuilder::new()
                .id_salt(("casca::corpo", estado.ativa))
                .max_rect(corpo)
                .layout(Layout::top_down(Align::Min)),
        );
        ui_corpo.set_clip_rect(corpo.intersect(ui.clip_rect()));
        match (aba.icone, aba.sub) {
            (Icone::Consulta, _) => {
                TextoPre::sql(
                    "SELECT \"cidade\", COUNT(*) AS \"clientes\"\n  FROM \"clientes\"\n GROUP BY \"cidade\";",
                )
                .mostrar(&mut ui_corpo);
            }
            (_, Some("Visão geral")) => {
                dock(&mut ui_corpo, &itens_da_dock());
            }
            (_, Some("Dados")) => {
                let mut desenho = DesenhoDados { p, marcadas: &mut estado.marcadas };
                estado.grade.mostrar(&mut ui_corpo, corpo.height().min(260.0), &mut desenho);
            }
            (Icone::Casa, _) => {
                vazio(&mut ui_corpo, "O Início do DataBox: os recentes e o abrir por caminho.", false);
            }
            (_, parte) => {
                let texto = format!("A parte {} de {} fica aqui.", parte.unwrap_or("—"), aba.nome);
                vazio(&mut ui_corpo, &texto, false);
            }
        }

        if let Some(i) = r.clicada {
            estado.ativa = i;
        }
        if let Some(i) = r.fechar {
            estado.fechar(i);
        }
        if r.nova {
            estado.nova_consulta();
        }
    }

    fn propriedades(&mut self, ui: &mut Ui) {
        let aba = self.casca.aba().clone();
        Painel::new("casca::propriedades", "Propriedades").mostrar(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            match (aba.icone, aba.sub) {
                (Icone::Casa, _) => {}
                (Icone::Consulta, _) => {
                    let _ = Secao::new("casca::banco", "Banco").primeira(true).mostrar(ui, |ui| {
                        ficha(ui, |d| {
                            d.linha_texto("Nome", Some(BANCO), "—");
                            let _ = d.linha_caminho("Caminho", CAMINHO);
                        });
                    });
                    let _ = Secao::new("casca::transacao", "Transação").mostrar(ui, |ui| {
                        ficha(ui, |d| {
                            d.linha("Situação", |ui| {
                                pill(ui, "nenhuma", TipoPill::Neutra);
                            });
                        });
                    });
                }
                (_, Some("Visão geral")) => {
                    let _ = Secao::new("casca::arquivo", "Arquivo").primeira(true).mostrar(ui, |ui| {
                        ficha(ui, |d| {
                            let _ = d.linha_caminho("Caminho", CAMINHO);
                            d.linha_texto("Tamanho", Some("500 KB"), "—");
                            d.linha_texto("Modificado em", Some("13/10/2026 18:00"), "—");
                            d.linha_texto("Versão do SQLite", Some("3.53.2"), "—");
                            d.linha_texto("Codificação", Some("UTF-8"), "—");
                            d.linha_texto("Tamanho de página", Some("4,0 KB"), "—");
                            d.linha_texto("Modo de diário", Some("delete"), "—");
                            d.linha_texto("Versão do usuário", Some("0"), "—");
                            d.linha("Chaves estrangeiras", |ui| {
                                pill(ui, "verificadas", TipoPill::Ok);
                            });
                        });
                    });
                }
                _ => {
                    let _ = Secao::new("casca::tabela", "Tabela").primeira(true).mostrar(ui, |ui| {
                        ficha(ui, |d| {
                            d.linha_texto("Nome", Some(&aba.nome), "—");
                            d.linha_texto("Tipo", Some("tabela"), "—");
                            d.linha_texto("Linhas", Some("60"), "—");
                            d.linha_texto("Colunas", Some("7"), "—");
                            d.linha_texto("Índices", Some("1"), "—");
                            d.linha_texto("Gatilhos", Some("0"), "—");
                            d.linha_texto("Estrita", Some("não"), "—");
                        });
                    });
                    let _ = Secao::new("casca::arquivo_curto", "Arquivo").mostrar(ui, |ui| {
                        ficha(ui, |d| {
                            let _ = d.linha_caminho("Caminho", CAMINHO);
                            d.linha_texto("Tamanho", Some("500 KB"), "—");
                        });
                    });
                    // a zona de risco da Estrutura: a seção de perigo, recolhida
                    let zona = Secao::new("casca::excluir_tabela", "Excluir a tabela").perigo(true).recolhida(true);
                    let _ = zona.mostrar(ui, |_| {});
                }
            }
        });
    }

    fn status(&mut self, ui: &mut Ui) {
        let e = paleta(ui).estudio;
        let aba = self.casca.aba().clone();
        let resumo = match (aba.icone, aba.sub) {
            (Icone::Consulta, _) => Some("SQL".to_owned()),
            (Icone::Casa, _) | (_, Some("Visão geral")) => None,
            _ => Some(format!("{} · 60 linhas · 7 colunas", aba.nome)),
        };
        // sem o tamanho do arquivo, também na Visão geral (D1, opção B): ele está nos cartões e nas Propriedades
        let mut direita: Vec<ItemStatus<'_>> = resumo.iter().map(|r| ItemStatus::new(r)).collect();
        direita.extend([ItemStatus::new("SQLite 3.53.2"), ItemStatus::new("UTF-8")]);
        let esquerda = [ItemStatus::new(BANCO).ponto(e.ok), ItemStatus::new("Gravação imediata").dica(FRASE_DO_MODO)];
        barra_status(ui, &esquerda, &direita);
    }
}

/// A ficha (`.det`) dentro de uma seção das Propriedades: `padding:2px 14px 14px`.
fn ficha(ui: &mut Ui, conteudo: impl FnOnce(&mut Detalhes<'_>)) {
    let r = Rect::from_min_max(ui.cursor().min + vec2(14.0, 2.0), pos2(ui.max_rect().right() - 14.0, f32::INFINITY));
    let mut dentro = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
    detalhes(&mut dentro, conteudo);
    let altura = dentro.min_rect().height() + 2.0 + 14.0;
    ui.allocate_space(vec2(ui.available_width(), altura));
}
