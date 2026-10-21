//! Os menus da barra de título (o `nav` "Menus" das pranchetas: Arquivo, Editar, Exibir e Ajuda, com 2px entre
//! eles), sobre a peça `interface::pecas::casca::Menu`. Nenhum item tem reticências (design/README.md, "Textos").
//!
//! - **Arquivo** (300px; desde 20/10/2026 com as silhuetas de 14 da barra de ferramentas, como os menus Banco e
//!   Tabela, no lugar dos sólidos de 24 da prancheta Inicio: item de menu é ação): Abrir pasta (a pasta;
//!   Ctrl+Shift+O) · Abrir arquivo (a folha; Ctrl+O) · Novo banco (o cilindro com o "+") | Abertos recentemente ▸
//!   (sem silhueta por ora; os recentes que existem, menos o banco aberto e a pasta aberta, com o sólido de 24 de
//!   cada tipo, a pasta, o cilindro ou a folha, em linhas de 44px: são objetos; o clique abre pelo tipo; desligado
//!   sem nenhum) | Salvar (o disquete; Ctrl+S, o verbo da aba) · Fechar aba (Ctrl+W; desligado com o Início sozinho)
//!   · Fechar banco (com um banco aberto) · Fechar pasta (desligado sem pasta) | Preferências (sem silhueta por ora;
//!   Ctrl+,).
//! - **Editar**: os verbos da aba, das mesmas `Ofertas` da barra de ferramentas: Desfazer · Refazer | Novo · Nova
//!   coluna · Duplicar · Excluir | Buscar · Filtrar · Atualizar · Executar · Exportar. O item fica desligado quando a
//!   aba não oferece o verbo, e o texto é o rótulo da oferta ("Nova tabela"). O escolhido vai para o documento pelo
//!   mesmo caminho do clique na barra: a casca o executa antes de a aba se desenhar.
//! - **Exibir** (a prancheta Main, 262px): Início (Alt+Home; a aba Início, que o trilho abria até 19/10/2026) |
//!   Explorador (Ctrl+B), Propriedades (Ctrl+Alt+B) e Histórico de consultas, marcados quando à vista na doca ou na
//!   janela própria (as Propriedades seguem o botão da barra: desligadas quando sumiram por falta de largura e no
//!   Início, que não as tem), e a Barra de status (escondida por padrão desde 20/10/2026; o que ela mostrava está
//!   nas Propriedades e nos rodapés dos documentos) | Tema ▸ (210px: Seguir o Windows, Claro e Escuro, com o ponto no
//!   escolhido, e "Preferências de aparência", que abre as Preferências na seção Aparência).
//! - **Banco** (revisão de UI de 19/10/2026, prancheta 3; só com um banco aberto, entre Exibir e Ajuda, o
//!   `.menu-ctx` de 290px com as silhuetas): Visão geral · Nova consulta SQL · Nova tabela (desligada em só leitura) |
//!   Verificar integridade · Verificar chaves estrangeiras · Otimizar · Compactar (VACUUM) (com as silhuetas da
//!   Manutenção das Propriedades; os dois últimos só com o banco gravando) | Fazer cópia de segurança · Exportar o
//!   banco | Abrir só para leitura (ou "Abrir para gravação",
//!   no banco só leitura) · Fechar o banco. As manutenções e a cópia põem a Visão geral à vista, que as executa.
//! - **Tabela** (prancheta 2; só com uma tabela ou visão à vista, depois do Banco): as vistas das Propriedades
//!   Colunas (Alt+2), Índices e gatilhos (Alt+3) e Definição SQL (Alt+4; a visão só tem esta) | Abrir na Consulta
//!   SQL · Exportar a tabela · Copiar o nome | Propriedades em janela própria | Nova linha · Nova coluna · Novo
//!   índice | Renomear a tabela · Excluir a tabela (em `--perigo`). O que muda a estrutura fica desligado na visão,
//!   no banco só leitura e na tabela que o SQLite não deixa editar.
//! - **Ajuda**: Atalhos do teclado (a janela; quando ela fecha, o foco volta ao botão "Ajuda") e Sobre o DataBox.
//!
//! Clicar num item fecha o menu. O que a pessoa escolheu volta como [`PedidoMenu`] para a casca aplicar.

use banco::{AcaoManutencao, BancoInfo};
use egui::{Id, Ui};
use estado::{Acao, Ofertas, Parte, Rota, Verbo};
use interface::icones::Icone;
use interface::pecas::casca::{Menu, PainelMenu};
use interface::tema::Tema;

use super::RecenteMenu;
use super::docas::{Paineis, Painel};
use super::ferramentas::EstadoPainel;
use super::preferencias::Secao;

/// Os ids dos menus, na ordem da barra (o Banco e a Tabela só aparecem com banco aberto e tabela à vista).
const IDS: [&str; 6] = ["arquivo", "editar", "exibir", "banco", "tabela", "ajuda"];
/// Largura do Banco e da Tabela (`.menu-ctx` de 290px, `Consulta.dc.html` e `Dados.dc.html`).
const LARGURA_BANCO: f32 = 290.0;
/// O espaço entre os menus (`nav{gap:2px}`).
pub(crate) const ESPACO_MENUS: f32 = 2.0;
/// Largura do painel do Exibir (`width:262px`) e a do submenu Tema (`width:210px`).
const LARGURA_EXIBIR: f32 = 262.0;
const LARGURA_TEMA: f32 = 210.0;
/// Largura do Arquivo e do submenu "Abertos recentemente" (`Inicio.dc.html`, o `.menu-ic` de 300px e o de 310px).
const LARGURA_ARQUIVO: f32 = 300.0;
const LARGURA_RECENTES: f32 = 310.0;

/// Os grupos do Editar, separados por fios.
const GRUPOS_EDITAR: [&[Verbo]; 3] = [
    &[Verbo::Desfazer, Verbo::Refazer],
    &[Verbo::Novo, Verbo::NovaColuna, Verbo::Duplicar, Verbo::Excluir],
    &[Verbo::Buscar, Verbo::Filtrar, Verbo::Atualizar, Verbo::Executar, Verbo::Exportar],
];

/// As opções do Exibir › Tema.
pub(crate) const TEMAS: [(Tema, &str); 3] =
    [(Tema::Sistema, comum::app::SEGUIR_O_SISTEMA), (Tema::Claro, "Claro"), (Tema::Escuro, "Escuro")];

/// O que os menus precisam saber a cada quadro.
pub(crate) struct Menus<'a> {
    /// O que a aba ativa oferece de cada verbo (os mesmos da barra de ferramentas).
    pub ofertas: &'a Ofertas,
    pub tem_banco: bool,
    /// Há uma pasta aberta (o Fechar pasta liga).
    pub tem_pasta: bool,
    /// A aba ativa pode fechar (o Início sozinho não fecha).
    pub pode_fechar_aba: bool,
    pub paineis: Paineis,
    /// O documento à vista tem Propriedades (as abas das docas contam com elas).
    pub tem_propriedades: bool,
    /// O estado do botão Painel de propriedades da barra (a marca e o desligado do item Propriedades).
    pub propriedades: EstadoPainel,
    pub tema: Tema,
    /// A barra de status à vista (a marca do Exibir › Barra de status).
    pub barra_status: bool,
    /// O banco aberto (com ele, o menu Banco).
    pub banco: Option<&'a BancoInfo>,
    /// A tabela ou visão à vista (com ela, o menu Tabela).
    pub tabela: Option<MenuTabela<'a>>,
}

/// O que o menu Tabela precisa saber da tabela à vista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuTabela<'a> {
    pub nome: &'a str,
    pub e_visao: bool,
    /// A estrutura pode mudar (tabela, banco gravando, e o SQLite deixa).
    pub editavel: bool,
}

/// Um item do menu Tabela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemTabela {
    /// Uma vista das Propriedades (a parte da tabela).
    Vista(Parte),
    AbrirNaConsulta,
    Exportar,
    CopiarNome,
    PropriedadesEmJanela,
    NovaColuna,
    NovoIndice,
    Renomear,
    Excluir,
}

/// O que a pessoa escolheu num menu.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PedidoMenu {
    /// Um pedido à casca (abrir, criar, um recente, fechar a aba ou o banco, o tema).
    Acao(Acao),
    /// Um verbo da aba (Editar, Salvar), para o documento executar antes de se desenhar.
    Verbo(Verbo),
    /// Mostrar ou esconder o Explorador, as Propriedades ou o Histórico de consultas.
    Painel(Painel),
    /// Mostrar ou esconder a barra de status.
    BarraStatus,
    /// Um item do menu Tabela, para a tabela à vista.
    Tabela(ItemTabela),
    /// A janela "Atalhos do teclado" e a "Sobre o DataBox".
    Atalhos,
    Sobre,
    /// A janela Preferências, já na seção (Arquivo › Preferências: Geral; Exibir › Tema › Preferências de aparência).
    Preferencias(Secao),
}

/// O que a barra de menus devolve.
pub(crate) struct SaidaMenus {
    pub pedido: Option<PedidoMenu>,
    /// O botão "Ajuda" (o foco volta para ele quando as janelas do menu fecham).
    pub id_ajuda: Id,
}

/// Um verbo da aba como item: ligado se a aba o oferece, com o rótulo dela e o atalho. Devolve se foi clicado.
fn item_do_verbo(p: &mut PainelMenu<'_>, ofertas: &Ofertas, verbo: Verbo) -> bool {
    let rotulo = ofertas.rotulo(verbo);
    if ofertas.ligado(verbo) {
        p.item(&rotulo, verbo.atalho(), false).clicked()
    } else {
        p.item_desligado(&rotulo, verbo.atalho());
        false
    }
}

/// Os quatro menus, da esquerda para a direita, no `ui` (que deve estar num layout da esquerda para a direita, com
/// o espaço entre os itens já em [`ESPACO_MENUS`]).
pub(crate) fn barra(ui: &mut Ui, m: &Menus<'_>, recentes: &[RecenteMenu]) -> SaidaMenus {
    let mut pedido = None;
    let _ = Menu::new(IDS[0], "Arquivo").largura(LARGURA_ARQUIVO).mostrar(ui, |p| {
        // as silhuetas da barra de ferramentas (revisão de 20/10/2026: item de menu é ação, como nos outros menus;
        // os sólidos ficam para os objetos, como os arquivos dos Abertos recentemente)
        p.com_silhuetas();
        if p.silhueta(Icone::Pasta).item("Abrir pasta", Some("Ctrl+Shift+O"), false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::EscolherEAbrirPasta));
        }
        if p.silhueta(Icone::Arquivo).item("Abrir arquivo", Some("Ctrl+O"), false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::EscolherEAbrir { somente_leitura: false }));
        }
        if p.silhueta(Icone::NovoBanco).item("Novo banco", None, false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::NovoBanco));
        }
        p.separador();
        if recentes.is_empty() {
            p.item_desligado("Abertos recentemente", None);
        } else {
            p.submenu("Abertos recentemente", LARGURA_RECENTES, |p| {
                p.com_icones();
                for r in recentes {
                    if p.item_duplo(Some(r.icone), &r.nome, &r.pasta).clicked() {
                        pedido = Some(PedidoMenu::Acao(r.acao.clone()));
                    }
                }
            });
        }
        p.separador();
        p.silhueta(Icone::Salvar);
        if item_do_verbo(p, m.ofertas, Verbo::Salvar) {
            pedido = Some(PedidoMenu::Verbo(Verbo::Salvar));
        }
        if !m.pode_fechar_aba {
            p.item_desligado("Fechar aba", Some("Ctrl+W"));
        } else if p.item("Fechar aba", Some("Ctrl+W"), false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::FecharAba(None)));
        }
        if m.tem_banco && p.item("Fechar banco", None, false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::FecharBanco));
        }
        if !m.tem_pasta {
            p.item_desligado("Fechar pasta", None);
        } else if p.item("Fechar pasta", None, false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::FecharPasta));
        }
        p.separador();
        if p.item("Preferências", Some("Ctrl+,"), false).clicked() {
            pedido = Some(PedidoMenu::Preferencias(Secao::Geral));
        }
    });

    let _ = Menu::new(IDS[1], "Editar").mostrar(ui, |p| {
        for (i, grupo) in GRUPOS_EDITAR.iter().enumerate() {
            if i > 0 {
                p.separador();
            }
            for &verbo in *grupo {
                if item_do_verbo(p, m.ofertas, verbo) {
                    pedido = Some(PedidoMenu::Verbo(verbo));
                }
            }
        }
    });

    let _ = Menu::new(IDS[2], "Exibir").largura(LARGURA_EXIBIR).mostrar(ui, |p| {
        p.com_marcas();
        if p.item("Início", Some("Alt+Home"), false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::Navegar(Rota::Inicio)));
        }
        p.separador();
        let a_vista = |painel| m.paineis.a_vista(painel, m.tem_propriedades);
        if p.item_marcado("Explorador", Some("Ctrl+B"), a_vista(Painel::Explorador)).clicked() {
            pedido = Some(PedidoMenu::Painel(Painel::Explorador));
        }
        let propriedades = match m.propriedades {
            EstadoPainel::AVista => Some(true),
            EstadoPainel::Escondido => Some(false),
            EstadoPainel::SemLargura | EstadoPainel::SemPainel => None,
        };
        match propriedades {
            Some(marcado) => {
                if p.item_marcado("Propriedades", Some("Ctrl+Alt+B"), marcado).clicked() {
                    pedido = Some(PedidoMenu::Painel(Painel::Propriedades));
                }
            }
            None => {
                p.item_desligado("Propriedades", Some("Ctrl+Alt+B"));
            }
        }
        if p.item_marcado("Histórico de consultas", None, a_vista(Painel::Historico)).clicked() {
            pedido = Some(PedidoMenu::Painel(Painel::Historico));
        }
        if p.item_marcado("Barra de status", None, m.barra_status).clicked() {
            pedido = Some(PedidoMenu::BarraStatus);
        }
        p.separador();
        p.submenu("Tema", LARGURA_TEMA, |p| {
            p.com_marcas();
            for (tema, rotulo) in TEMAS {
                if p.item_opcao(rotulo, m.tema == tema).clicked() && m.tema != tema {
                    pedido = Some(PedidoMenu::Acao(Acao::TrocarTema(crate::app::tema_das_prefs(tema))));
                }
            }
            p.separador();
            if p.item("Preferências de aparência", None, false).clicked() {
                pedido = Some(PedidoMenu::Preferencias(Secao::Aparencia));
            }
        });
    });

    if let Some(b) = m.banco {
        let somente_leitura = b.somente_leitura;
        let _ = Menu::new(IDS[3], "Banco").largura(LARGURA_BANCO).mostrar(ui, |p| {
            p.com_silhuetas();
            if p.item("Visão geral", None, false).clicked() {
                pedido = Some(PedidoMenu::Acao(Acao::Navegar(Rota::Banco)));
            }
            if p.silhueta(Icone::NovaConsulta).item("Nova consulta SQL", None, false).clicked() {
                pedido = Some(PedidoMenu::Acao(Acao::NovaConsulta { sql: None }));
            }
            p.silhueta(Icone::Novo);
            if somente_leitura {
                p.item_desligado("Nova tabela", None);
            } else if p.item("Nova tabela", None, false).clicked() {
                pedido = Some(PedidoMenu::Acao(Acao::AbrirNovaTabela));
            }
            p.separador();
            // as silhuetas da seção Manutenção das Propriedades
            let manutencoes = [
                (AcaoManutencao::Integridade, Icone::VerificarIntegridade, "Verificar integridade", false),
                (AcaoManutencao::Chaves, Icone::VerificarChaves, "Verificar chaves estrangeiras", false),
                (AcaoManutencao::Otimizar, Icone::Otimizar, "Otimizar", true),
                (AcaoManutencao::Vacuum, Icone::Compactar, "Compactar (VACUUM)", true),
            ];
            for (acao, icone, rotulo, escreve) in manutencoes {
                p.silhueta(icone);
                if escreve && somente_leitura {
                    p.item_desligado(rotulo, None);
                } else if p.item(rotulo, None, false).clicked() {
                    pedido = Some(PedidoMenu::Acao(Acao::ManutencaoDoBanco(acao)));
                }
            }
            p.separador();
            if p.silhueta(Icone::CopiaSeguranca).item("Fazer cópia de segurança", None, false).clicked() {
                pedido = Some(PedidoMenu::Acao(Acao::CopiaDeSeguranca));
            }
            if p.silhueta(Icone::Exportar).item("Exportar o banco", None, false).clicked() {
                pedido = Some(PedidoMenu::Acao(Acao::AbrirExportacao(crate::visao_geral::pedido_exportacao(b))));
            }
            p.separador();
            let (rotulo, so_leitura) =
                if somente_leitura { ("Abrir para gravação", false) } else { ("Abrir só para leitura", true) };
            if p.item(rotulo, None, false).clicked() {
                let caminho = b.caminho.clone();
                pedido = Some(PedidoMenu::Acao(Acao::AbrirBanco { caminho, somente_leitura: so_leitura }));
            }
            if p.item("Fechar o banco", None, false).clicked() {
                pedido = Some(PedidoMenu::Acao(Acao::FecharBanco));
            }
        });
    }

    if let Some(t) = m.tabela {
        let _ = Menu::new(IDS[4], "Tabela").largura(LARGURA_BANCO).mostrar(ui, |p| {
            p.com_silhuetas();
            let vistas: &[(Parte, &str, &str)] = if t.e_visao {
                &[(Parte::Definicao, "Definição SQL", "Alt+4")]
            } else {
                &[
                    (Parte::Estrutura, "Colunas", "Alt+2"),
                    (Parte::Indices, "Índices e gatilhos", "Alt+3"),
                    (Parte::Definicao, "Definição SQL", "Alt+4"),
                ]
            };
            for &(parte, rotulo, atalho) in vistas {
                if parte == Parte::Estrutura {
                    p.silhueta(Icone::Editar);
                }
                if p.item(rotulo, Some(atalho), false).clicked() {
                    pedido = Some(PedidoMenu::Tabela(ItemTabela::Vista(parte)));
                }
            }
            p.separador();
            let itens = [
                (Some(Icone::NovaConsulta), "Abrir na Consulta SQL", ItemTabela::AbrirNaConsulta),
                (Some(Icone::Exportar), "Exportar a tabela", ItemTabela::Exportar),
                (Some(Icone::CopiarCaminho), "Copiar o nome", ItemTabela::CopiarNome),
            ];
            for (icone, rotulo, item) in itens {
                if let Some(i) = icone {
                    p.silhueta(i);
                }
                if p.item(rotulo, None, false).clicked() {
                    pedido = Some(PedidoMenu::Tabela(item));
                }
            }
            p.separador();
            if p.silhueta(Icone::Destacar).item("Propriedades em janela própria", None, false).clicked() {
                pedido = Some(PedidoMenu::Tabela(ItemTabela::PropriedadesEmJanela));
            }
            p.separador();
            p.silhueta(Icone::Novo);
            if item_do_verbo(p, m.ofertas, Verbo::Novo) {
                pedido = Some(PedidoMenu::Verbo(Verbo::Novo));
            }
            let estrutura = [
                (Some(Icone::NovaColuna), "Nova coluna", ItemTabela::NovaColuna),
                (None, "Novo índice", ItemTabela::NovoIndice),
            ];
            for (icone, rotulo, item) in estrutura {
                if let Some(i) = icone {
                    p.silhueta(i);
                }
                if !t.editavel {
                    p.item_desligado(rotulo, None);
                } else if p.item(rotulo, None, false).clicked() {
                    pedido = Some(PedidoMenu::Tabela(item));
                }
            }
            p.separador();
            p.silhueta(Icone::Editar);
            if !t.editavel {
                p.item_desligado("Renomear a tabela", None);
            } else if p.item("Renomear a tabela", None, false).clicked() {
                pedido = Some(PedidoMenu::Tabela(ItemTabela::Renomear));
            }
            p.silhueta(Icone::Excluir);
            if !t.editavel {
                p.item_desligado("Excluir a tabela", None);
            } else if p.item_perigo("Excluir a tabela", None, false).clicked() {
                pedido = Some(PedidoMenu::Tabela(ItemTabela::Excluir));
            }
        });
    }

    let (ajuda, _) = Menu::new(IDS[5], "Ajuda").mostrar(ui, |p| {
        if p.item("Atalhos do teclado", None, false).clicked() {
            pedido = Some(PedidoMenu::Atalhos);
        }
        if p.item(&format!("Sobre o {}", comum::app::NOME), None, false).clicked() {
            pedido = Some(PedidoMenu::Sobre);
        }
    });
    SaidaMenus { pedido, id_ajuda: ajuda.id }
}

/// Algum dos menus está aberto?
pub fn algum_aberto(ctx: &egui::Context) -> bool {
    IDS.iter().any(|id| egui::Popup::is_id_open(ctx, Id::new(("databox::menu", *id)).with("painel")))
}

/// Fecha o menu que estiver aberto (o Ctrl+O fecha o menu antes de abrir o diálogo).
pub(crate) fn fechar_todos(ui: &Ui) {
    for id in IDS {
        Menu::fechar(ui, id);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn editar_tem_todos_os_verbos_menos_o_salvar() {
        let verbos: Vec<Verbo> = GRUPOS_EDITAR.iter().flat_map(|g| g.iter().copied()).collect();
        // fora do Editar: o Salvar (fica no Arquivo), o Interromper (só existe na barra, enquanto a Consulta
        // executa) e o Trazer colunas (só o editor de CSV: a barra e o menu do cabeçalho; etapa 5)
        assert_eq!(verbos.len(), Verbo::TODOS.len() - 3);
        assert!(!verbos.contains(&Verbo::Interromper) && !verbos.contains(&Verbo::TrazerColunas));
        assert!(!verbos.contains(&Verbo::Salvar), "o Salvar fica no Arquivo");
        // a Nova coluna logo depois do Novo, como na barra
        let novo = verbos.iter().position(|&v| v == Verbo::Novo).expect("Novo");
        assert_eq!(verbos[novo + 1], Verbo::NovaColuna);
    }

    #[test]
    fn temas_na_ordem_do_exibir() {
        let rotulos: Vec<&str> = TEMAS.iter().map(|(_, r)| *r).collect();
        assert_eq!(rotulos, [comum::app::SEGUIR_O_SISTEMA, "Claro", "Escuro"]);
        assert_eq!(TEMAS.map(|(t, _)| t), [Tema::Sistema, Tema::Claro, Tema::Escuro]);
    }
}
