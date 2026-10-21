//! Os atalhos de teclado da casca: Ctrl+O (abrir um arquivo), Ctrl+Shift+O (abrir uma pasta), Alt+← e Alt+→ e os botões laterais do mouse (o
//! histórico único, que atravessa as abas), Ctrl+W (fechar a aba), Ctrl+Tab e Ctrl+Shift+Tab (a aba seguinte e a
//! anterior na faixa, com volta), Ctrl+Shift+PageUp e Ctrl+Shift+PageDown (a aba ativa uma casa para a esquerda ou
//! para a direita, sem volta), F5 (o Atualizar da aba, se ela o oferece; senão a tecla fica com quem tem o foco),
//! Alt+Home (a aba Início, como o Exibir › Início; o trilho que a abria saiu em 19/10/2026), Alt+1 a Alt+4 (as
//! vistas das Propriedades de uma tabela à vista: Resumo, Colunas, Índices e gatilhos, Definição SQL),
//! Ctrl+B (mostrar ou esconder o Explorador), Ctrl+Alt+B (as Propriedades, com o botão delas ligado) e Ctrl+F (a
//! busca da barra, quando a aba tem busca; também com um campo de texto ou a grade com o foco) e Ctrl+, (a janela
//! Preferências, com ou sem campo de texto focado). O Ctrl+Enter continua com a Consulta SQL. Os verbos dos
//! documentos editáveis (etapa 5), só quando a aba os oferece: Ctrl+S (o Salvar, também com um campo de texto
//! focado), Ctrl+Z (o Desfazer) e Ctrl+Y ou Ctrl+Shift+Z (o Refazer), estes dois só sem campo de texto focado (com o
//! editor de texto focado, o desfazer é o dele).
//!
//! Só com os modificadores exatos: o Ctrl+Shift+O abre a pasta, não o arquivo, e
//! o AltGr do teclado ABNT2, que chega como Ctrl+Alt, não fecha a aba, não abre os diálogos e não mexe no Explorador. O Ctrl+Alt+B fica
//! com o campo de texto que tem o foco (o AltGr+B pode escrever algo em outros teclados). Quem chama decide quando
//! valem (nunca com uma janela ou modal aberta).

use egui::{InputState, Key, KeyboardShortcut, Modifiers, PointerButton};

/// O que a pessoa apertou neste quadro (as teclas já foram consumidas).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Atalhos {
    pub abrir: bool,
    /// Ctrl+Shift+O: o diálogo Abrir pasta.
    pub abrir_pasta: bool,
    pub voltar: bool,
    pub avancar: bool,
    pub fechar_aba: bool,
    pub proxima: bool,
    pub anterior: bool,
    /// Ctrl+Shift+PageUp e Ctrl+Shift+PageDown: a aba ativa uma casa para a esquerda ou para a direita.
    pub mover_esquerda: bool,
    pub mover_direita: bool,
    pub atualizar: bool,
    /// Alt+Home: a aba Início.
    pub inicio: bool,
    /// Alt+1 a Alt+4 (só com uma tabela à vista): a vista das Propriedades, de 1 (Resumo) a 4 (Definição SQL).
    pub vista: Option<u8>,
    pub explorador: bool,
    pub propriedades: bool,
    /// Ctrl+F: o foco na busca da barra.
    pub buscar: bool,
    /// Ctrl+, : a janela Preferências.
    pub preferencias: bool,
    /// Ctrl+S: o Salvar da aba (também com um campo de texto focado).
    pub salvar: bool,
    /// Ctrl+Z: o Desfazer da aba (só sem campo de texto focado).
    pub desfazer: bool,
    /// Ctrl+Y e Ctrl+Shift+Z: o Refazer da aba (só sem campo de texto focado).
    pub refazer: bool,
}

impl Atalhos {
    /// Algum atalho foi apertado.
    pub fn algum(&self) -> bool {
        *self != Atalhos::default()
    }

    /// Algum atalho que tira o documento de vista, abre o diálogo ou o refaz (a edição em curso grava antes):
    /// todos, menos o Ctrl+Shift+PageUp e PageDown, que só mudam a aba de lugar, o Ctrl+B e o Ctrl+Alt+B, que só
    /// mostram e escondem os painéis, o Ctrl+F, que só leva o foco à
    /// busca da barra (a célula em edição grava quando perde o foco), e os verbos do documento (Ctrl+S, Ctrl+Z,
    /// Ctrl+Y), que vão a ele no mesmo quadro sem tirar o foco de ninguém.
    pub fn sai_do_documento(&self) -> bool {
        Atalhos {
            mover_esquerda: false,
            mover_direita: false,
            explorador: false,
            propriedades: false,
            buscar: false,
            salvar: false,
            desfazer: false,
            refazer: false,
            ..*self
        }
        .algum()
    }

    /// O verbo do documento que o atalho pede (Ctrl+S, Ctrl+Z, Ctrl+Y), executado no mesmo quadro.
    pub fn verbo(&self) -> Option<estado::Verbo> {
        use estado::Verbo;
        [(self.salvar, Verbo::Salvar), (self.desfazer, Verbo::Desfazer), (self.refazer, Verbo::Refazer)]
            .into_iter()
            .find_map(|(pedido, v)| pedido.then_some(v))
    }
}

/// O que vale neste quadro, além dos atalhos de sempre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Valem {
    /// Um campo de texto tem o foco: o Ctrl+Alt+B fica com ele.
    pub campo_de_texto: bool,
    /// Uma tabela ou visão à vista: o Alt+1 a Alt+4 escolhem a vista das Propriedades.
    pub tabela: bool,
    /// A aba oferece o Atualizar: sem ele, o F5 fica com quem tem o foco (nada sai do campo).
    pub atualizar: bool,
    /// O botão Painel de propriedades está ligado: sem ele (o Início, a janela estreita), o Ctrl+Alt+B não faz nada.
    pub propriedades: bool,
    /// A aba tem a busca da barra: sem ela (o Início, os Índices, a Definição), o Ctrl+F fica com quem tem o foco.
    pub buscar: bool,
    /// A aba oferece o Salvar, o Desfazer e o Refazer (sem eles, a tecla fica com quem tem o foco).
    pub salvar: bool,
    pub desfazer: bool,
    pub refazer: bool,
}

impl Default for Valem {
    /// Tudo vale, sem campo de texto focado.
    fn default() -> Self {
        Valem {
            campo_de_texto: false,
            tabela: false,
            atualizar: true,
            propriedades: true,
            buscar: true,
            salvar: true,
            desfazer: true,
            refazer: true,
        }
    }
}

/// Consome a tecla com estes modificadores e nenhum outro (o `consume_shortcut` do egui aceita Shift e Alt a mais).
fn consumir(i: &mut InputState, m: Modifiers, k: Key) -> bool {
    i.modifiers.matches_exact(m) && i.consume_shortcut(&KeyboardShortcut::new(m, k))
}

/// Lê e consome os atalhos da casca deste quadro, com o que [`Valem`] agora (as teclas dos que não valem não são
/// consumidas).
pub(crate) fn ler(ctx: &egui::Context, valem: Valem) -> Atalhos {
    ctx.input_mut(|i| {
        let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        Atalhos {
            abrir: consumir(i, Modifiers::COMMAND, Key::O),
            abrir_pasta: consumir(i, ctrl_shift, Key::O),
            voltar: consumir(i, Modifiers::ALT, Key::ArrowLeft) || i.pointer.button_pressed(PointerButton::Extra1),
            avancar: consumir(i, Modifiers::ALT, Key::ArrowRight) || i.pointer.button_pressed(PointerButton::Extra2),
            fechar_aba: consumir(i, Modifiers::COMMAND, Key::W),
            proxima: consumir(i, Modifiers::COMMAND, Key::Tab),
            anterior: consumir(i, ctrl_shift, Key::Tab),
            mover_esquerda: consumir(i, ctrl_shift, Key::PageUp),
            mover_direita: consumir(i, ctrl_shift, Key::PageDown),
            atualizar: valem.atualizar && consumir(i, Modifiers::NONE, Key::F5),
            inicio: consumir(i, Modifiers::ALT, Key::Home),
            vista: if valem.tabela {
                [Key::Num1, Key::Num2, Key::Num3, Key::Num4]
                    .into_iter()
                    .zip(1u8..)
                    .find_map(|(k, n)| consumir(i, Modifiers::ALT, k).then_some(n))
            } else {
                None
            },
            explorador: consumir(i, Modifiers::COMMAND, Key::B),
            propriedades: valem.propriedades && !valem.campo_de_texto && consumir(i, ctrl_alt, Key::B),
            buscar: valem.buscar && consumir(i, Modifiers::COMMAND, Key::F),
            preferencias: consumir(i, Modifiers::COMMAND, Key::Comma),
            // o Ctrl+S vale com o campo de texto focado (decisão 9 da etapa 5); o Ctrl+Z e o Ctrl+Y, só sem ele: com
            // o `TextEdit` (ou o editor de célula) focado, o desfazer é o do egui
            salvar: valem.salvar && consumir(i, Modifiers::COMMAND, Key::S),
            desfazer: valem.desfazer && !valem.campo_de_texto && consumir(i, Modifiers::COMMAND, Key::Z),
            refazer: valem.refazer
                && !valem.campo_de_texto
                && (consumir(i, Modifiers::COMMAND, Key::Y) || consumir(i, ctrl_shift, Key::Z)),
        }
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O Ctrl do Windows como o egui-winit entrega: `ctrl` e `command` juntos.
    const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

    /// Os atalhos lidos de um quadro com esta tecla apertada (com os modificadores no estado do teclado também).
    fn com_tecla_e_campo(modificadores: Modifiers, tecla: Key, campo_de_texto: bool) -> Atalhos {
        com_tecla_valendo(modificadores, tecla, Valem { campo_de_texto, ..Valem::default() })
    }

    fn com_tecla_valendo(modificadores: Modifiers, tecla: Key, valem: Valem) -> Atalhos {
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::ModifiersChanged(modificadores));
        raw.events.push(egui::Event::Key {
            key: tecla,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: modificadores,
        });
        let mut lidos = Atalhos::default();
        let saida = ctx.run_ui(raw, |ui| lidos = ler(ui.ctx(), valem));
        saida.drop_without_applying_deltas();
        lidos
    }

    fn com_tecla(modificadores: Modifiers, tecla: Key) -> Atalhos {
        com_tecla_e_campo(modificadores, tecla, false)
    }

    #[test]
    fn cada_atalho_com_os_modificadores_exatos() {
        assert_eq!(com_tecla(CTRL, Key::O), Atalhos { abrir: true, ..Default::default() });
        assert_eq!(com_tecla(CTRL, Key::W), Atalhos { fechar_aba: true, ..Default::default() });
        assert_eq!(com_tecla(CTRL, Key::Tab), Atalhos { proxima: true, ..Default::default() });
        let ctrl_shift = Modifiers { shift: true, ..CTRL };
        assert_eq!(com_tecla(ctrl_shift, Key::Tab), Atalhos { anterior: true, ..Default::default() });
        assert_eq!(com_tecla(ctrl_shift, Key::PageUp), Atalhos { mover_esquerda: true, ..Default::default() });
        assert_eq!(com_tecla(ctrl_shift, Key::PageDown), Atalhos { mover_direita: true, ..Default::default() });
        assert!(!com_tecla(ctrl_shift, Key::PageDown).sai_do_documento(), "mudar a aba de lugar não a tira de vista");
        assert_eq!(com_tecla(Modifiers::ALT, Key::ArrowLeft), Atalhos { voltar: true, ..Default::default() });
        assert_eq!(com_tecla(Modifiers::ALT, Key::ArrowRight), Atalhos { avancar: true, ..Default::default() });
        let explorador = com_tecla(CTRL, Key::B);
        assert_eq!(explorador, Atalhos { explorador: true, ..Default::default() });
        assert!(!explorador.sai_do_documento(), "o Ctrl+B não tira a edição em curso do campo");
        assert!(com_tecla(CTRL, Key::W).sai_do_documento());
        // o AltGr do ABNT2 chega como Ctrl+Alt: não fecha a aba, não abre nem mexe no Explorador; o Shift a mais não
        // abre
        let altgr = Modifiers { alt: true, ..CTRL };
        assert!(!com_tecla(altgr, Key::W).algum());
        assert!(!com_tecla(altgr, Key::O).algum());
        // o Ctrl+Shift+O abre a pasta (não o arquivo); o AltGr com o Shift não abre nada
        let pasta = com_tecla(ctrl_shift, Key::O);
        assert_eq!(pasta, Atalhos { abrir_pasta: true, ..Default::default() });
        assert!(pasta.sai_do_documento(), "o diálogo é modal: a edição em curso grava antes");
        assert!(!com_tecla(Modifiers { shift: true, ..altgr }, Key::O).algum());
        assert!(!com_tecla(Modifiers::NONE, Key::Tab).algum(), "o Tab sozinho é do foco");
    }

    #[test]
    fn f5_atualiza_e_ctrl_alt_b_mostra_as_propriedades() {
        let f5 = com_tecla(Modifiers::NONE, Key::F5);
        assert_eq!(f5, Atalhos { atualizar: true, ..Default::default() });
        assert!(f5.sai_do_documento(), "o F5 relê a aba: a edição em curso grava antes");
        assert!(!com_tecla(CTRL, Key::F5).algum(), "Ctrl+F5 não é o atalho");
        // o Ctrl+Alt+B (o AltGr+B do ABNT2) mostra ou esconde as Propriedades, sem tirar a edição do campo; com um
        // campo de texto focado, fica com ele (e o Ctrl+B, sem o Alt, continua sendo o do Explorador)
        let ctrl_alt = Modifiers { alt: true, ..CTRL };
        let propriedades = com_tecla(ctrl_alt, Key::B);
        assert_eq!(propriedades, Atalhos { propriedades: true, ..Default::default() });
        assert!(!propriedades.sai_do_documento());
        assert!(!com_tecla_e_campo(ctrl_alt, Key::B, true).algum());
        assert_eq!(com_tecla_e_campo(CTRL, Key::B, true), Atalhos { explorador: true, ..Default::default() });
    }

    #[test]
    fn ctrl_f_vai_para_a_busca_so_com_a_busca() {
        let buscar = com_tecla(CTRL, Key::F);
        assert_eq!(buscar, Atalhos { buscar: true, ..Default::default() });
        assert!(!buscar.sai_do_documento(), "o Ctrl+F só leva o foco à busca");
        // também com um campo de texto focado (o editor da Consulta); sem a busca na aba, não é atalho
        assert!(com_tecla_e_campo(CTRL, Key::F, true).buscar);
        let sem_busca = Valem { buscar: false, ..Valem::default() };
        assert!(!com_tecla_valendo(CTRL, Key::F, sem_busca).algum());
        // o AltGr+F e o Ctrl+Shift+F não são o atalho
        assert!(!com_tecla(Modifiers { alt: true, ..CTRL }, Key::F).algum());
        assert!(!com_tecla(Modifiers { shift: true, ..CTRL }, Key::F).algum());
    }

    #[test]
    fn ctrl_virgula_abre_as_preferencias() {
        let a = com_tecla(CTRL, Key::Comma);
        assert_eq!(a, Atalhos { preferencias: true, ..Default::default() });
        assert!(a.sai_do_documento(), "a janela é modal: a edição em curso grava antes");
        // também com um campo de texto focado (o editor da Consulta)
        assert!(com_tecla_e_campo(CTRL, Key::Comma, true).preferencias);
        // o AltGr+, e o Ctrl+Shift+, não são o atalho
        assert!(!com_tecla(Modifiers { alt: true, ..CTRL }, Key::Comma).algum());
        assert!(!com_tecla(Modifiers { shift: true, ..CTRL }, Key::Comma).algum());
    }

    #[test]
    fn ctrl_s_salva_mesmo_com_campo_focado() {
        let a = com_tecla(CTRL, Key::S);
        assert_eq!(a, Atalhos { salvar: true, ..Default::default() });
        assert_eq!(a.verbo(), Some(estado::Verbo::Salvar));
        assert!(!a.sai_do_documento(), "o Salvar vai ao documento no mesmo quadro, sem tirar o foco do campo");
        // com o campo de texto focado também (o editor de texto, a célula do CSV)
        assert!(com_tecla_e_campo(CTRL, Key::S, true).salvar);
        // sem o Salvar na aba, a tecla fica com quem tem o foco; o AltGr+S e o Ctrl+Shift+S não são o atalho
        assert!(!com_tecla_valendo(CTRL, Key::S, Valem { salvar: false, ..Valem::default() }).algum());
        assert!(!com_tecla(Modifiers { alt: true, ..CTRL }, Key::S).algum());
        assert!(!com_tecla(Modifiers { shift: true, ..CTRL }, Key::S).algum());
    }

    #[test]
    fn ctrl_z_e_ctrl_y_so_fora_de_campo_de_texto() {
        let z = com_tecla(CTRL, Key::Z);
        assert_eq!(z, Atalhos { desfazer: true, ..Default::default() });
        assert_eq!(z.verbo(), Some(estado::Verbo::Desfazer));
        assert!(!z.sai_do_documento());
        let y = com_tecla(CTRL, Key::Y);
        assert_eq!(y, Atalhos { refazer: true, ..Default::default() });
        assert_eq!(y.verbo(), Some(estado::Verbo::Refazer));
        // o Ctrl+Shift+Z também refaz
        assert_eq!(
            com_tecla(Modifiers { shift: true, ..CTRL }, Key::Z),
            Atalhos { refazer: true, ..Default::default() }
        );
        // com um campo de texto focado, ficam com ele (o desfazer do egui)
        assert!(!com_tecla_e_campo(CTRL, Key::Z, true).algum());
        assert!(!com_tecla_e_campo(CTRL, Key::Y, true).algum());
        assert!(!com_tecla_e_campo(Modifiers { shift: true, ..CTRL }, Key::Z, true).algum());
        // sem a oferta da aba, não são atalho; o AltGr+Z não é o Ctrl+Z
        let sem = Valem { desfazer: false, refazer: false, ..Valem::default() };
        assert!(!com_tecla_valendo(CTRL, Key::Z, sem).algum() && !com_tecla_valendo(CTRL, Key::Y, sem).algum());
        assert!(!com_tecla(Modifiers { alt: true, ..CTRL }, Key::Z).algum());
        assert_eq!(Atalhos::default().verbo(), None);
    }

    #[test]
    fn f5_sem_atualizar_e_ctrl_alt_b_sem_o_botao_nao_valem() {
        // sem o Atualizar na aba, o F5 não é atalho (fica com quem tem o foco, e a edição não termina)
        let sem_atualizar = Valem { atualizar: false, ..Valem::default() };
        assert!(!com_tecla_valendo(Modifiers::NONE, Key::F5, sem_atualizar).algum());
        // com o botão Painel de propriedades desligado (o Início, a janela estreita), o Ctrl+Alt+B não faz nada
        let ctrl_alt = Modifiers { alt: true, ..CTRL };
        let sem_painel = Valem { propriedades: false, ..Valem::default() };
        assert!(!com_tecla_valendo(ctrl_alt, Key::B, sem_painel).algum());
        // os outros continuam
        assert_eq!(com_tecla_valendo(CTRL, Key::B, sem_painel), Atalhos { explorador: true, ..Default::default() });
    }

    #[test]
    fn alt_home_vai_ao_inicio() {
        let alt = Modifiers { alt: true, ..Modifiers::NONE };
        let a = com_tecla(alt, Key::Home);
        assert_eq!(a, Atalhos { inicio: true, ..Default::default() });
        assert!(a.sai_do_documento(), "a edição em curso grava antes de a aba sair de vista");
        // sem o Alt, o Home fica com quem tem o foco
        assert_eq!(com_tecla(Modifiers::NONE, Key::Home), Atalhos::default());
    }

    #[test]
    fn alt_com_numero_so_com_tabela() {
        let alt = Modifiers { alt: true, ..Modifiers::NONE };
        let com_tabela = Valem { tabela: true, ..Valem::default() };
        assert_eq!(com_tecla_valendo(alt, Key::Num2, com_tabela), Atalhos { vista: Some(2), ..Default::default() });
        assert_eq!(com_tecla_valendo(alt, Key::Num4, com_tabela).vista, Some(4));
        assert_eq!(com_tecla_valendo(alt, Key::Num2, Valem::default()), Atalhos::default());
    }
}
