//! Os menus da barra de título (o `nav` "Menus" das pranchetas: Arquivo, Editar, Exibir e Ajuda, com 2px entre
//! eles), sobre a peça `interface::pecas::casca::Menu`. Nenhum item tem reticências (design/README.md, "Textos").
//!
//! - **Arquivo** (a prancheta Inicio, o `.menu-ic` de 300px: linhas de 36px com os sólidos de 24):
//!   Abrir pasta (a pasta; Ctrl+Shift+O) · Abrir arquivo (a folha; Ctrl+O) · Novo banco (o cilindro com o
//!   "+") | Abertos recentemente ▸ (o relógio; os recentes que existem, menos o banco aberto e a pasta aberta, com o
//!   ícone de 24 de cada tipo, a pasta, o cilindro ou a folha, em linhas de 44px; o clique abre pelo tipo; desligado
//!   sem nenhum) | Salvar (o disquete; Ctrl+S, o verbo da aba) · Fechar aba (Ctrl+W; desligado com o Início sozinho)
//!   · Fechar banco (com um banco aberto) · Fechar pasta (desligado sem pasta) | Preferências (a engrenagem dupla;
//!   Ctrl+,).
//! - **Editar**: os verbos da aba, das mesmas `Ofertas` da barra de ferramentas: Desfazer · Refazer | Novo · Nova
//!   coluna · Duplicar · Excluir | Buscar · Filtrar · Atualizar · Executar · Exportar. O item fica desligado quando a
//!   aba não oferece o verbo, e o texto é o rótulo da oferta ("Nova tabela"). O escolhido vai para o documento pelo
//!   mesmo caminho do clique na barra: a casca o executa antes de a aba se desenhar.
//! - **Exibir** (a prancheta Main, 262px): Explorador (Ctrl+B), Propriedades (Ctrl+Alt+B) e Histórico de consultas,
//!   marcados quando à vista (as Propriedades seguem o botão da barra: desligadas quando sumiram por falta de
//!   largura e no Início, que não as tem) | Tema ▸ (210px: Seguir o Windows, Claro e Escuro, com o ponto no
//!   escolhido, e "Preferências de aparência", que abre as Preferências na seção Aparência).
//! - **Ajuda**: Atalhos do teclado (a janela; quando ela fecha, o foco volta ao botão "Ajuda") e Sobre o DataBox.
//!
//! Clicar num item fecha o menu. O que a pessoa escolheu volta como [`PedidoMenu`] para a casca aplicar.

use egui::{Id, Ui};
use estado::{Acao, Ofertas, Verbo};
use interface::icones::Icone;
use interface::pecas::casca::{Menu, PainelMenu};
use interface::tema::Tema;

use super::RecenteMenu;
use super::ferramentas::EstadoPainel;
use super::icones::{Paineis, PainelEsquerdo};
use super::preferencias::Secao;

/// Os ids dos quatro menus, na ordem da barra.
const IDS: [&str; 4] = ["arquivo", "editar", "exibir", "ajuda"];
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
    [(Tema::Sistema, "Seguir o Windows"), (Tema::Claro, "Claro"), (Tema::Escuro, "Escuro")];

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
    /// O estado do botão Painel de propriedades da barra (a marca e o desligado do item Propriedades).
    pub propriedades: EstadoPainel,
    pub tema: Tema,
}

/// O que a pessoa escolheu num menu.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PedidoMenu {
    /// Um pedido à casca (abrir, criar, um recente, fechar a aba ou o banco, o tema).
    Acao(Acao),
    /// Um verbo da aba (Editar, Salvar), para o documento executar antes de se desenhar.
    Verbo(Verbo),
    /// Mostrar ou esconder o Explorador, as Propriedades ou o Histórico de consultas.
    Explorador,
    Propriedades,
    Historico,
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
        p.com_icones();
        if p.icone(Icone::PastaSolida).item("Abrir pasta", Some("Ctrl+Shift+O"), false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::EscolherEAbrirPasta));
        }
        if p.icone(Icone::DocumentoSolido).item("Abrir arquivo", Some("Ctrl+O"), false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::EscolherEAbrir { somente_leitura: false }));
        }
        if p.icone(Icone::NovoBancoSolido).item("Novo banco", None, false).clicked() {
            pedido = Some(PedidoMenu::Acao(Acao::NovoBanco));
        }
        p.separador();
        if recentes.is_empty() {
            p.icone(Icone::Recentes).item_desligado("Abertos recentemente", None);
        } else {
            p.icone(Icone::Recentes).submenu("Abertos recentemente", LARGURA_RECENTES, |p| {
                p.com_icones();
                for r in recentes {
                    if p.item_duplo(Some(r.icone), &r.nome, &r.pasta).clicked() {
                        pedido = Some(PedidoMenu::Acao(r.acao.clone()));
                    }
                }
            });
        }
        p.separador();
        p.icone(Icone::DisqueteSolido);
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
        if p.icone(Icone::Preferencias).item("Preferências", Some("Ctrl+,"), false).clicked() {
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
        let esquerdo = m.paineis.esquerdo;
        if p.item_marcado("Explorador", Some("Ctrl+B"), esquerdo == Some(PainelEsquerdo::Explorador)).clicked() {
            pedido = Some(PedidoMenu::Explorador);
        }
        let propriedades = match m.propriedades {
            EstadoPainel::AVista => Some(true),
            EstadoPainel::Escondido => Some(false),
            EstadoPainel::SemLargura | EstadoPainel::SemPainel => None,
        };
        match propriedades {
            Some(marcado) => {
                if p.item_marcado("Propriedades", Some("Ctrl+Alt+B"), marcado).clicked() {
                    pedido = Some(PedidoMenu::Propriedades);
                }
            }
            None => {
                p.item_desligado("Propriedades", Some("Ctrl+Alt+B"));
            }
        }
        if p.item_marcado("Histórico de consultas", None, esquerdo == Some(PainelEsquerdo::Historico)).clicked() {
            pedido = Some(PedidoMenu::Historico);
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

    let (ajuda, _) = Menu::new(IDS[3], "Ajuda").mostrar(ui, |p| {
        if p.item("Atalhos do teclado", None, false).clicked() {
            pedido = Some(PedidoMenu::Atalhos);
        }
        if p.item(&format!("Sobre o {}", comum::app::NOME), None, false).clicked() {
            pedido = Some(PedidoMenu::Sobre);
        }
    });
    SaidaMenus { pedido, id_ajuda: ajuda.id }
}

/// Algum dos quatro menus está aberto?
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
        assert_eq!(rotulos, ["Seguir o Windows", "Claro", "Escuro"]);
        assert_eq!(TEMAS.map(|(t, _)| t), [Tema::Sistema, Tema::Claro, Tema::Escuro]);
    }
}
