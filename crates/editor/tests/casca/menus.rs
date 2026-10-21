//! Os menus Arquivo, Editar e Exibir (com o tema e os painéis), o menu sem banco e a janela "Atalhos do teclado", com
//! o foco que volta ao menu quando ela fecha.

use super::*;

#[test]
fn menu_arquivo_tema_e_atalhos() {
    let amb = Ambiente::exemplo("menu");
    let mut h = harness_com(&amb, Tema::Sistema, TAMANHO, Partida::default(), |_| None);
    assert_eq!(interface::tema::tema_aplicado(&h.ctx), Tema::Sistema);
    abrir_menu_arquivo(&mut h);
    assert!(menu_aberto(&h.ctx));
    // os itens da prancheta, sem reticências; o Fechar pasta (sem pasta aberta) e o Salvar (nenhuma aba o oferece)
    // desligados
    for (item, esperado) in [
        ("Abrir pasta", true),
        ("Abrir arquivo", true),
        ("Novo banco", true),
        ("Salvar", false),
        ("Fechar aba", true),
        ("Fechar banco", true),
        ("Fechar pasta", false),
    ] {
        assert_eq!(item_ligado(&h, item), esperado, "{item}");
    }
    assert!(tem_texto_exato(&h, "Abertos recentemente"));
    assert!(h.query(by().role(Role::Button).label("Abrir outro banco…")).is_none());
    capturar(&mut h, "menu-arquivo-aberto-claro.png");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!menu_aberto(&h.ctx), "Esc fecha o menu");

    // Exibir › Tema: vale na tela e nas preferências, fecha o menu, e a borda do Windows segue o tema do app (o
    // `SetTheme` que o egui manda a cada mudança)
    for (rotulo, tema, prefs_tema, da_janela) in [
        ("Escuro", Tema::Escuro, prefs::Tema::Escuro, egui::SystemTheme::Dark),
        ("Claro", Tema::Claro, prefs::Tema::Claro, egui::SystemTheme::Light),
        (comum::app::SEGUIR_O_SISTEMA, Tema::Sistema, prefs::Tema::Sistema, egui::SystemTheme::SystemDefault),
    ] {
        abrir_menu(&mut h, "Exibir");
        abrir_submenu(&mut h, "Tema");
        let enviados = clicar_no(&mut h, Role::RadioButton, rotulo);
        assert_eq!(h.state().tema(), tema);
        assert_eq!(interface::tema::tema_aplicado(&h.ctx), tema);
        assert!(enviados.contains(&ViewportCommand::SetTheme(da_janela)), "{rotulo}: {enviados:?}");
        assert!(!menu_aberto(&h.ctx), "escolher o tema fecha o menu");
        assentar(&mut h);
        assert_eq!(amb.servico.prefs().tema(), prefs_tema, "gravado nas preferências");
    }

    // Ajuda › Atalhos do teclado
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    assert!(!menu_aberto(&h.ctx), "clicar num item fecha o menu");
    h.run_steps(2);
    assert!(h.state().janela_aberta());
    assert!(
        tem_texto(&h, "Fechar a janela ou o menu aberto, ou interromper a consulta")
            && tem_texto(&h, "Na grade de dados")
    );
    assert!(tem_texto(&h, "Excluir as linhas marcadas (pergunta antes)"));
    assert!(tem_texto(&h, "Voltar e avançar entre as abas"));
    // os atalhos da casca nova (Ctrl+W, Ctrl+Tab, F5, Ctrl+B e Ctrl+Alt+B; a tecla é pintada, o leitor de tela
    // ouve a frase)
    for texto in [
        "Fechar aba",
        "Ir para a aba seguinte e para a anterior",
        "Atualizar a aba (a Visão geral ou a tabela)",
        "Mostrar ou esconder o Explorador",
        "Mostrar ou esconder as Propriedades",
    ] {
        assert!(tem_texto_exato(&h, texto), "{texto}: {:?}", textos(&h));
    }
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!h.state().janela_aberta(), "Esc fecha a janela de atalhos");
}

#[test]
fn janela_de_atalhos_captura() {
    let amb = Ambiente::exemplo("atalhos");
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    h.run_steps(3);
    assert!(tem_texto(&h, "Na grade de dados"));
    capturar(&mut h, "janela-atalhos-claro.png");
}

#[test]
fn menu_sem_banco() {
    let amb = Ambiente::vazio("menu_sem_banco");
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    assert!(item_ligado(&h, "Abrir arquivo") && item_ligado(&h, "Novo banco"));
    assert!(!item_ligado(&h, "Fechar aba"), "o Início sozinho não fecha");
    assert!(!item_ligado(&h, "Abertos recentemente"), "sem recentes");
    assert!(h.query(by().role(Role::Button).label("Fechar banco")).is_none());
    assert!(h.query(by().role(Role::Button).label("Abrir banco…")).is_none());
    capturar(&mut h, "menu-sem-banco-claro.png");
}

#[test]
fn menu_arquivo_escuro() {
    let amb = Ambiente::exemplo("menu_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    abrir_menu_arquivo(&mut h);
    assert!(menu_aberto(&h.ctx));
    capturar(&mut h, "menu-arquivo-aberto-escuro.png");
}

#[test]
fn menu_exibir_tema_escuro() {
    let amb = Ambiente::exemplo("menu_exibir_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    abrir_menu(&mut h, "Exibir");
    // o Explorador e as Propriedades à vista, marcados; o Histórico não
    assert!(ligado(&h, Role::CheckBox, "Explorador") && ligado(&h, Role::CheckBox, "Propriedades"));
    assert!(!ligado(&h, Role::CheckBox, "Histórico de consultas"));
    abrir_submenu(&mut h, "Tema");
    let escolhidos: Vec<bool> =
        [comum::app::SEGUIR_O_SISTEMA, "Claro", "Escuro"].map(|t| ligado(&h, Role::RadioButton, t)).into();
    assert_eq!(escolhidos, [false, false, true], "o ponto no tema escolhido");
    capturar(&mut h, "menu-exibir-tema-escuro.png");
}

#[test]
fn menu_exibir_mostra_e_esconde_os_paineis() {
    let amb = Ambiente::exemplo("menu_exibir");
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Exibir");
    clicar(&mut h, Role::CheckBox, "Explorador");
    assert!(!menu_aberto(&h.ctx), "o item fecha o menu");
    assert!(!painel_a_vista(&h, "Explorador"), "a doca mostra o mesmo");
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none());
    abrir_menu(&mut h, "Exibir");
    clicar(&mut h, Role::CheckBox, "Histórico de consultas");
    assert!(painel_a_vista(&h, "Histórico"));
    esperar_texto(&mut h, "Nenhuma consulta ainda.");
    abrir_menu(&mut h, "Exibir");
    assert!(ligado(&h, Role::CheckBox, "Histórico de consultas") && !ligado(&h, Role::CheckBox, "Explorador"));
    clicar(&mut h, Role::CheckBox, "Propriedades");
    assert!(!h.state().propriedades_a_vista());
    assert!(!ligado(&h, Role::Button, "Painel de propriedades"));
    abrir_menu(&mut h, "Exibir");
    clicar(&mut h, Role::CheckBox, "Propriedades");
    assert!(h.state().propriedades_a_vista());

    // numa janela estreita as Propriedades somem e o item fica desligado, como o botão da barra
    let amb = Ambiente::exemplo("menu_exibir_estreita");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(980.0, 700.0), Partida::default(), |_| None);
    assert!(!h.state().propriedades_a_vista());
    abrir_menu(&mut h, "Exibir");
    assert!(!item_ligado(&h, "Propriedades"));
}

#[test]
fn menu_editar_espelha_a_barra() {
    let amb = Ambiente::exemplo("menu_editar");
    let mut h = harness(&amb);
    // a Visão geral: os mesmos verbos da barra, com o rótulo da aba ("Nova tabela")
    abrir_menu(&mut h, "Editar");
    for (item, esperado) in [
        ("Desfazer", false),
        ("Refazer", false),
        ("Nova tabela", true),
        ("Nova coluna", false),
        ("Duplicar", false),
        ("Excluir", false),
        ("Buscar", true),
        ("Filtrar", false),
        ("Atualizar", true),
        ("Executar", false),
        ("Exportar", true),
    ] {
        assert_eq!(item_ligado(&h, item), esperado, "{item} no Editar da Visão geral");
    }
    assert!(h.query(by().role(Role::Button).label("Novo")).is_none(), "o Novo leva o rótulo da aba");
    capturar(&mut h, "menu-editar-claro.png");
    // o item executa como o botão: a janela Nova tabela
    clicar_item(&mut h, "Nova tabela");
    assert!(!menu_aberto(&h.ctx));
    assert!(h.state().janela_aberta(), "a janela Nova tabela");
    h.key_press(Key::Escape);
    h.run_steps(3);
    // o Exportar com a origem da aba
    abrir_menu(&mut h, "Editar");
    clicar_item(&mut h, "Exportar");
    let origem = h.state().exportacao_aberta().map(|p| p.origem.clone());
    assert_eq!(origem, Some(estado::OrigemJanelaExportar::Banco));
    h.key_press(Key::Escape);
    h.run_steps(3);

    // a tabela: Atualizar e Exportar; os Dados (etapa 3) ligam o Novo como "Nova linha" (o substituto, sem as
    // telas, o deixa como "Novo", desligado)
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    abrir_menu(&mut h, "Editar");
    let telas = cfg!(feature = "telas");
    let novo = if telas { "Nova linha" } else { "Novo" };
    assert!(item_ligado(&h, "Atualizar") && item_ligado(&h, "Exportar") && item_ligado(&h, novo) == telas);
    h.key_press(Key::Escape);
    h.run_steps(2);

    // a Consulta SQL: o Executar, que roda o comando como o Ctrl+Enter
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: Some("SELECT 41 + 1 AS resposta_do_menu".into()) });
    h.run_steps(2);
    assentar(&mut h);
    abrir_menu(&mut h, "Editar");
    assert!(item_ligado(&h, "Executar") && !item_ligado(&h, "Exportar"));
    if cfg!(feature = "telas") {
        clicar_item(&mut h, "Executar");
        esperar_texto(&mut h, "Comando 1 · 1 linha");
    }
}

#[test]
fn fechar_aba_e_fechar_banco_pelo_menu_arquivo() {
    let amb = Ambiente::exemplo("menu_fechar_aba");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).0, s(&["exemplo.db · Visão geral", "clientes"]));
    abrir_menu_arquivo(&mut h);
    clicar_item(&mut h, "Fechar aba");
    h.run_steps(2);
    assert_eq!(rotulos_das_abas(&h), (s(&["exemplo.db · Visão geral"]), "exemplo.db · Visão geral".to_string()));
    // o Fechar banco fica entre o Fechar aba e o Fechar pasta
    abrir_menu_arquivo(&mut h);
    let y = |h: &H, rotulo: &str| item_de_menu(h, rotulo).rect().top();
    assert!(y(&h, "Fechar aba") < y(&h, "Fechar banco") && y(&h, "Fechar banco") < y(&h, "Fechar pasta"));
    clicar_item(&mut h, "Fechar banco");
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
}

#[test]
fn fechar_atalhos_devolve_o_foco_ao_menu() {
    let amb = Ambiente::exemplo("foco_atalhos");
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    h.run_steps(2);
    assert!(h.state().janela_aberta());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    assert!(botao_menu(&h, "Ajuda").is_focused(), "o foco volta ao botão Ajuda");
}
