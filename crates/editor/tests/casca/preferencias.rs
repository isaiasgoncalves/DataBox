//! As Preferências (etapa 3, passo 18), com as capturas da prancheta 13.

use super::*;

/// Vai para uma seção da navegação.
fn ir_para_secao(h: &mut H, secao: &str) {
    clicar_link(h, secao);
    h.run_steps(2);
}

/// O Salvar da janela (o último: a barra de ferramentas tem um Salvar, desligado, por baixo).
fn salvar_da_janela<'h>(h: &'h H) -> egui_kittest::Node<'h> {
    h.query_all(by().role(Role::Button).label("Salvar")).last().expect("o Salvar da janela")
}

fn salvar_ligado(h: &H) -> bool {
    !salvar_da_janela(h).accesskit_node().is_disabled()
}

/// Escolhe uma opção de um select da janela: abre a lista e clica na opção.
fn escolher_opcao(h: &mut H, select: &str, opcao: &str) {
    clicar(h, Role::ComboBox, select);
    clicar_botao(h, opcao);
}

/// O texto do arquivo de preferências do ambiente.
fn prefs_texto(amb: &Ambiente) -> String {
    std::fs::read_to_string(amb.pasta().join("preferencias.json")).expect("o preferencias.json")
}

/// O que o arquivo de preferências do ambiente guarda, lido por uma instância nova (como a próxima abertura leria).
fn prefs_lidas(amb: &Ambiente) -> prefs::Preferencias {
    prefs::Preferencias::abrir(Some(amb.pasta().join("preferencias.json")))
}

#[test]
fn preferencias_abrem_pelo_menu_pelo_atalho_e_pelo_inicio() {
    let amb = Ambiente::exemplo("pref_abrir");
    let mut h = harness(&amb);
    assert!(!preferencias_abertas(&h));

    // Arquivo › Preferências (o item com a engrenagem dupla e o Ctrl+,), na seção Geral
    abrir_menu_arquivo(&mut h);
    assert!(item_ligado(&h, "Preferências"));
    clicar_item(&mut h, "Preferências");
    h.run_steps(3);
    assert_eq!(h.state().preferencias_aberta(), Some("Geral"));
    assert!(existe(&h, Role::Dialog, "Preferências"));
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!preferencias_abertas(&h), "Esc fecha");

    // Ctrl+,
    h.key_press_modifiers(CTRL, Key::Comma);
    h.run_steps(3);
    assert_eq!(h.state().preferencias_aberta(), Some("Geral"));
    // com a janela aberta (modal), o atalho não abre outra nem fecha esta
    h.key_press_modifiers(CTRL, Key::Comma);
    h.run_steps(2);
    assert!(preferencias_abertas(&h));
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!preferencias_abertas(&h));

    // o botão "Preferências" dos Painéis do Início (o do pé do trilho saiu na revisão de UI de 19/10/2026)
    ir_ao_inicio(&mut h);
    clicar_botao(&mut h, "Preferências");
    h.run_steps(3);
    assert_eq!(h.state().preferencias_aberta(), Some("Geral"));
    h.key_press(Key::Escape);
    h.run_steps(3);

    // Exibir › Tema › Preferências de aparência: direto na seção Aparência
    abrir_menu(&mut h, "Exibir");
    abrir_submenu(&mut h, "Tema");
    clicar_item(&mut h, "Preferências de aparência");
    h.run_steps(3);
    assert_eq!(h.state().preferencias_aberta(), Some("Aparência"));
    assert!(!menu_aberto(&h.ctx));
    assert!(ligado(&h, Role::Link, "Aparência"));
}

#[test]
fn inicio_tem_os_botoes_dos_paineis() {
    let amb = Ambiente::exemplo("inicio_paineis");
    let mut h = harness(&amb);
    ir_ao_inicio(&mut h);
    // os atalhos do trilho que saiu: o Explorador (Ctrl+B) e o Histórico põem o painel à vista onde ele mora
    assert!(tem_texto(&h, "Painéis"));
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(2);
    assert!(!painel_a_vista(&h, "Explorador"));
    clicar_botao(&mut h, "Explorador");
    assentar(&mut h);
    assert!(painel_a_vista(&h, "Explorador"));
    clicar_botao(&mut h, "Histórico de consultas");
    assentar(&mut h);
    assert!(painel_a_vista(&h, "Histórico") && !painel_a_vista(&h, "Explorador"));
    // o Preferências abre a janela e não fica ligado
    let estado = |h: &H| {
        h.query_all(by().role(Role::Button).label("Preferências"))
            .map(|n| n.accesskit_node().toggled())
            .next()
            .flatten()
    };
    assert_eq!(estado(&h), None, "não é um botão de alternar");
    clicar_botao(&mut h, "Preferências");
    h.run_steps(3);
    assert!(preferencias_abertas(&h));
}

#[test]
fn navegacao_com_as_quatro_secoes_e_os_solidos_de_20() {
    use interface::icones::Icone;
    let amb = Ambiente::exemplo("pref_navegacao");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    // Tabelas fica fora enquanto não houver o que configurar nela
    let tops: Vec<f32> =
        ["Geral", "Aparência", "Consulta SQL", "Exportação"].iter().map(|s| rect_de(&h, Role::Link, s).top()).collect();
    assert!(tops.windows(2).all(|w| w[1] - w[0] == 34.0), "na ordem da prancheta, 32px com 2px entre eles: {tops:?}");
    for (icone, rotulo) in [
        (Icone::Geral, "Geral"),
        (Icone::Aparencia, "Aparência"),
        (Icone::Consulta, "Consulta SQL"),
        (Icone::Exportacao, "Exportação"),
    ] {
        let no = rect_de(&h, Role::Link, rotulo);
        assert_eq!(no.height(), 32.0, "{rotulo}");
        let r = icone_pintado_em(&h, icone, 20.0, no).unwrap_or_else(|| panic!("o sólido de 20 em {rotulo}"));
        assert_eq!(r.size(), egui::vec2(20.0, 20.0));
        assert_eq!(r.left() - no.left(), 9.0, "{rotulo}: padding-left de 9");
    }
    // só a seção visível é a atual
    for (secao, esperado) in [("Geral", true), ("Aparência", false), ("Consulta SQL", false), ("Exportação", false)]
    {
        assert_eq!(ligado(&h, Role::Link, secao), esperado, "{secao}");
    }
    ir_para_secao(&mut h, "Exportação");
    assert!(ligado(&h, Role::Link, "Exportação"));
    assert!(!ligado(&h, Role::Link, "Geral"));
}

#[test]
fn tema_vale_na_hora_e_cancelar_volta() {
    let amb = Ambiente::exemplo("pref_tema");
    let mut h = harness(&amb);
    assert_eq!(h.state().tema(), Tema::Claro);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    assert!(!salvar_ligado(&h), "nada mudou ainda");
    let enviados = clicar_no(&mut h, Role::Button, "Escuro");
    assert_eq!(h.state().tema(), Tema::Escuro, "vale na hora, com a janela aberta");
    assert_eq!(interface::tema::tema_aplicado(&h.ctx), Tema::Escuro);
    assert!(enviados.contains(&ViewportCommand::SetTheme(egui::SystemTheme::Dark)), "{enviados:?}");
    assert!(salvar_ligado(&h));
    assert_eq!(amb.servico.prefs().tema(), prefs::Tema::Sistema, "nada é gravado até o Salvar");
    // o clique em Cancelar volta ao tema da abertura, também na janela do Windows
    let enviados = clicar_no(&mut h, Role::Button, "Cancelar");
    assert!(!preferencias_abertas(&h));
    assert_eq!(h.state().tema(), Tema::Claro);
    assert_eq!(interface::tema::tema_aplicado(&h.ctx), Tema::Claro);
    assert!(enviados.contains(&ViewportCommand::SetTheme(egui::SystemTheme::Light)), "{enviados:?}");
    assentar(&mut h);
    assert_eq!(amb.servico.prefs().tema(), prefs::Tema::Sistema);
    assert!(!prefs_texto(&amb).contains("\"aparencia\""), "a chave aparencia só entra depois de escolher");
}

#[test]
fn esc_e_x_voltam_ao_estado_da_abertura() {
    use interface::codigo;
    for por in ["Esc", "×", "clique no fundo"] {
        let amb = Ambiente::exemplo("pref_esc");
        let mut h = harness(&amb);
        abrir_preferencias(&mut h);
        ir_para_secao(&mut h, "Aparência");
        clicar_botao(&mut h, "Escuro");
        clicar(&mut h, Role::RadioButton, "Monokai");
        escolher_opcao(&mut h, "Tamanho da fonte do código", "16 px");
        assert_eq!(h.state().tema(), Tema::Escuro);
        assert_eq!(codigo::atual(&h.ctx).cores, codigo::CoresCodigo::Monokai);
        assert_eq!(codigo::atual(&h.ctx).tamanho, 16.0);
        match por {
            "Esc" => h.key_press(Key::Escape),
            "×" => clicar_botao(&mut h, "Fechar"),
            _ => {
                // o fundo da janela (a tela por trás dela), passados os 400 ms em que o clique do duplo clique não conta
                clicar_em(&mut h, egui::pos2(20.0, 400.0));
            }
        }
        h.run_steps(3);
        assert!(!preferencias_abertas(&h), "{por}");
        assert_eq!(h.state().tema(), Tema::Claro, "{por}");
        let a = codigo::atual(&h.ctx);
        assert_eq!((a.cores, a.tamanho), (codigo::CoresCodigo::DataBox, 12.5), "{por}");
        assert_eq!(amb.servico.prefs().aparencia(), prefs::Aparencia::default(), "{por}: nada gravado");
    }
}

#[test]
fn salvar_so_liga_quando_algo_muda() {
    let amb = Ambiente::exemplo("pref_salvar_liga");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    assert!(!salvar_ligado(&h));
    // a dica diz por que está desligado
    h.remove_cursor();
    h.run_steps(2);
    let antes = textos(&h);
    salvar_da_janela(&h).hover();
    h.run_steps(3);
    let novos: Vec<String> = textos(&h).into_iter().filter(|t| !antes.contains(t)).collect();
    assert!(novos.iter().any(|t| t == "Nenhuma mudança para salvar"), "{novos:?}");
    h.remove_cursor();
    h.run_steps(2);
    clicar_botao(&mut h, "Escuro");
    assert!(salvar_ligado(&h));
    clicar_botao(&mut h, "Claro");
    assert!(!salvar_ligado(&h), "de volta ao que valia na abertura");
    clicar(&mut h, Role::RadioButton, "Solarized");
    assert!(salvar_ligado(&h));
    clicar(&mut h, Role::RadioButton, "DataBox");
    assert!(!salvar_ligado(&h));
    ir_para_secao(&mut h, "Exportação");
    clicar(&mut h, Role::CheckBox, "JSON indentado (mais fácil de ler)");
    assert!(salvar_ligado(&h));
    clicar(&mut h, Role::CheckBox, "JSON indentado (mais fácil de ler)");
    assert!(!salvar_ligado(&h));
}

#[test]
fn salvar_grava_e_fecha() {
    use interface::codigo;
    let amb = Ambiente::exemplo("pref_salvar");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    clicar_botao(&mut h, "Escuro");
    clicar(&mut h, Role::RadioButton, "Monokai");
    escolher_opcao(&mut h, "Tamanho da fonte do código", "14 px");
    clicar_ligado(&mut h, "Salvar");
    h.run_steps(2);
    assert!(!preferencias_abertas(&h), "Salvar fecha");
    assentar(&mut h);
    // o que o arquivo guarda (relido da pasta do ambiente) e o que a casca mantém
    let lidas = prefs_lidas(&amb);
    assert_eq!(lidas.tema(), prefs::Tema::Escuro);
    let a = lidas.aparencia();
    assert_eq!((a.cores_codigo, a.fonte_codigo, a.tamanho_codigo), (prefs::CoresCodigo::Monokai, None, 14.0));
    let texto = prefs_texto(&amb);
    assert!(texto.contains("\"cores_codigo\": \"monokai\"") && texto.contains("\"fonte_codigo\": null"), "{texto}");
    let p = &h.state().estado().prefs;
    assert_eq!(
        (p.tema, p.aparencia.cores_codigo, p.aparencia.tamanho_codigo),
        (prefs::Tema::Escuro, prefs::CoresCodigo::Monokai, 14.0)
    );
    // continua valendo na tela
    assert_eq!(h.state().tema(), Tema::Escuro);
    assert_eq!(codigo::atual(&h.ctx).tamanho, 14.0);
    // reaberta, o retrato é o de agora: nada a salvar
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    assert!(!salvar_ligado(&h));
}

/// O Idioma (P7): ao lado do Tema, vale na hora (o rótulo da linha já sai no idioma novo), o Cancelar volta ao
/// sistema e o Salvar grava a chave `"idioma"`. O idioma desta thread fica isolado do global (`com_idioma`): os
/// outros testes do binário continuam em português.
#[test]
fn idioma_vale_na_hora_cancelar_volta_e_salvar_grava() {
    use idioma::{Escolha, INGLES, PORTUGUES};
    idioma::com_idioma(PORTUGUES, || {
        let amb = Ambiente::exemplo("pref_idioma");
        let mut h = harness(&amb);
        abrir_preferencias(&mut h);
        ir_para_secao(&mut h, "Aparência");
        // logo depois do Tema, com o idioma que o sistema dá entre parênteses
        assert!(rect_de(&h, Role::ComboBox, "Idioma").top() > rect_de(&h, Role::Button, "Escuro").bottom());
        let sistema = format!("Seguir o sistema ({})", idioma::do_sistema().nome_curto());
        assert!(tem_texto(&h, &sistema), "{sistema}");
        assert!(tem_texto(&h, "Menus, janelas e mensagens; os números e as datas seguem o idioma."));

        escolher_opcao(&mut h, "Idioma", "English");
        h.run_steps(2);
        assert_eq!(idioma::atual(), INGLES, "vale na hora");
        assert!(existe(&h, Role::ComboBox, "Language"), "o rótulo do t! já em inglês");
        assert!(salvar_ligado(&h));
        assert_eq!(amb.servico.prefs().idioma(), Escolha::Sistema, "nada é gravado até o Salvar");
        clicar_botao(&mut h, "Cancelar");
        h.run_steps(3);
        assert!(!preferencias_abertas(&h));
        assert_eq!(idioma::atual(), idioma::do_sistema(), "o Cancelar volta a seguir o sistema");
        assentar(&mut h);
        assert!(!prefs_texto(&amb).contains("\"idioma\""), "sem a chave enquanto a pessoa não salva");

        // de novo, agora com o Português (Brasil) e o Salvar
        idioma::definir(PORTUGUES);
        abrir_preferencias(&mut h);
        ir_para_secao(&mut h, "Aparência");
        escolher_opcao(&mut h, "Idioma", "Português (Brasil)");
        assert_eq!(idioma::atual(), PORTUGUES);
        clicar_ligado(&mut h, "Salvar");
        assentar(&mut h);
        assert!(!preferencias_abertas(&h));
        assert_eq!(prefs_lidas(&amb).idioma(), Escolha::Idioma(PORTUGUES));
        assert!(prefs_texto(&amb).contains("\"idioma\": \"pt-BR\""), "{}", prefs_texto(&amb));
        assert_eq!(h.state().estado().prefs.idioma, Escolha::Idioma(PORTUGUES));
        // reaberta, o retrato é o de agora; o Restaurar o padrão volta a seguir o sistema
        abrir_preferencias(&mut h);
        ir_para_secao(&mut h, "Aparência");
        assert!(!salvar_ligado(&h));
        clicar_botao(&mut h, "Restaurar o padrão");
        h.run_steps(2);
        assert_eq!(idioma::atual(), idioma::do_sistema());
        h.key_press(Key::Escape);
        h.run_steps(3);
        assert_eq!(idioma::atual(), PORTUGUES, "o Esc volta ao que valia na abertura");
    });
}

#[test]
fn salvar_so_o_tema_nao_cria_a_chave_aparencia() {
    let amb = Ambiente::exemplo("pref_so_tema");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    clicar_botao(&mut h, "Escuro");
    clicar_ligado(&mut h, "Salvar");
    assentar(&mut h);
    assert_eq!(prefs_lidas(&amb).tema(), prefs::Tema::Escuro);
    let texto = prefs_texto(&amb);
    assert!(!texto.contains("\"aparencia\""), "sem a chave enquanto a pessoa não escolhe: {texto}");
}

#[test]
fn exportacao_salva_so_o_que_mudou() {
    let amb = Ambiente::exemplo("pref_exportacao");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Exportação");
    // as cinco opções, com os textos da janela Exportar (com o formato na frente quando só vale para um)
    for rotulo in ["Separador", "Decimal"] {
        assert!(existe(&h, Role::ComboBox, rotulo), "{rotulo}");
    }
    for rotulo in [
        "CSV: primeira linha com os nomes das colunas",
        "Proteger contra fórmulas do Excel",
        "JSON indentado (mais fácil de ler)",
    ] {
        assert!(ligado(&h, Role::CheckBox, rotulo), "{rotulo} começa marcada");
    }
    escolher_opcao(&mut h, "Separador", "Tabulação");
    clicar(&mut h, Role::CheckBox, "JSON indentado (mais fácil de ler)");
    assert!(salvar_ligado(&h));
    clicar_ligado(&mut h, "Salvar");
    assentar(&mut h);
    let e = prefs_lidas(&amb).exportacao();
    assert_eq!((e.csv_separador, e.csv_decimal, e.json_indentado), ('\t', ',', false));
    assert_eq!((e.cabecalho, e.csv_proteger_formulas), (None, None), "o que não mudou não é escolhido por ninguém");
    assert!(!prefs_texto(&amb).contains("\"aparencia\""));
    let e = h.state().estado().prefs.exportacao;
    assert_eq!((e.csv_separador, e.json_indentado), ('\t', false), "a janela Exportar lê daqui");
}

#[test]
fn restaurar_so_a_secao_visivel() {
    let amb = Ambiente::exemplo("pref_restaurar");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    clicar_botao(&mut h, "Escuro");
    clicar(&mut h, Role::RadioButton, "Alto contraste");
    escolher_opcao(&mut h, "Tamanho da fonte do código", "16 px");
    ir_para_secao(&mut h, "Exportação");
    clicar(&mut h, Role::CheckBox, "Proteger contra fórmulas do Excel");
    // na Exportação, o "Restaurar o padrão" é da Exportação: a Aparência fica como está
    assert!(
        ler_dica(&mut h, Role::Button, "Restaurar o padrão").contains("Restaura o padrão só desta seção (Exportação)")
    );
    clicar_botao(&mut h, "Restaurar o padrão");
    assert!(ligado(&h, Role::CheckBox, "Proteger contra fórmulas do Excel"));
    assert_eq!(h.state().tema(), Tema::Escuro);
    assert_eq!(interface::codigo::atual(&h.ctx).tamanho, 16.0);
    // na Aparência, volta o padrão dela: seguir o Windows, cores DataBox, 12,5 px
    ir_para_secao(&mut h, "Aparência");
    assert!(
        ler_dica(&mut h, Role::Button, "Restaurar o padrão").contains("Restaura o padrão só desta seção (Aparência)")
    );
    clicar_botao(&mut h, "Restaurar o padrão");
    assert_eq!(h.state().tema(), Tema::Sistema);
    let a = interface::codigo::atual(&h.ctx);
    assert_eq!((a.cores, a.tamanho), (interface::codigo::CoresCodigo::DataBox, 12.5));
    // nas seções sem padrão (Geral, Consulta SQL), o botão nem aparece
    ir_para_secao(&mut h, "Geral");
    assert!(h.query(by().role(Role::Button).label("Restaurar o padrão")).is_none());
    ir_para_secao(&mut h, "Consulta SQL");
    assert!(h.query(by().role(Role::Button).label("Restaurar o padrão")).is_none());
}

#[test]
fn cores_do_codigo_mudam_a_consulta_na_hora() {
    use interface::codigo;
    let amb = Ambiente::exemplo("pref_cores");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Consulta { sql: None });
    assentar(&mut h);
    assert_eq!(codigo::atual(&h.ctx).cores, codigo::CoresCodigo::DataBox);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    // as quatro cartas, com a DataBox escolhida
    for (rotulo, escolhida) in [("DataBox", true), ("Alto contraste", false), ("Solarized", false), ("Monokai", false)]
    {
        assert_eq!(ligado(&h, Role::RadioButton, rotulo), escolhida, "{rotulo}");
    }
    clicar(&mut h, Role::RadioButton, "Solarized");
    assert_eq!(codigo::atual(&h.ctx).cores, codigo::CoresCodigo::SolarizedEscuro);
    assert!(ligado(&h, Role::RadioButton, "Solarized"));
    assert!(!ligado(&h, Role::RadioButton, "DataBox"));
    clicar_botao(&mut h, "Cancelar");
    assert_eq!(codigo::atual(&h.ctx).cores, codigo::CoresCodigo::DataBox, "Cancelar volta");
}

#[test]
fn fonte_do_codigo_troca_e_volta() {
    use interface::{codigo, fontes};
    let outras: Vec<&str> = fontes::familias_mono_disponiveis()
        .into_iter()
        .map(|f| f.nome)
        .filter(|n| *n != fontes::FAMILIA_MONO_PADRAO)
        .collect();
    let Some(&outra) = outras.first() else {
        eprintln!("sem outra família monoespaçada nesta máquina: nada a trocar");
        return;
    };
    let amb = Ambiente::exemplo("pref_fonte");
    let mut h = harness(&amb);
    assert_eq!(fontes::mono_escolhida(&h.ctx), None);
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    escolher_opcao(&mut h, "Família da fonte do código", outra);
    assentar(&mut h);
    assert_eq!(fontes::mono_escolhida(&h.ctx).as_deref(), Some(outra), "a família vale na hora");
    assert_eq!(codigo::atual(&h.ctx).familia.as_deref(), Some(outra));
    assert!(salvar_ligado(&h));
    // voltar à padrão pelo próprio select desliga o Salvar
    escolher_opcao(
        &mut h,
        "Família da fonte do código",
        &format!("Padrão do sistema ({})", fontes::FAMILIA_MONO_PADRAO),
    );
    assentar(&mut h);
    assert_eq!(fontes::mono_escolhida(&h.ctx), None);
    assert!(!salvar_ligado(&h));
    // e o Cancelar também volta a que valia
    escolher_opcao(&mut h, "Família da fonte do código", outra);
    assentar(&mut h);
    clicar_botao(&mut h, "Cancelar");
    assentar(&mut h);
    assert_eq!(fontes::mono_escolhida(&h.ctx), None, "Cancelar volta à Consolas");
    assert_eq!(codigo::atual(&h.ctx).familia, None);
    // Salvar guarda o nome da família
    abrir_preferencias(&mut h);
    ir_para_secao(&mut h, "Aparência");
    escolher_opcao(&mut h, "Família da fonte do código", outra);
    assentar(&mut h);
    clicar_ligado(&mut h, "Salvar");
    assentar(&mut h);
    assert_eq!(prefs_lidas(&amb).aparencia().fonte_codigo.as_deref(), Some(outra));
    assert_eq!(fontes::mono_escolhida(&h.ctx).as_deref(), Some(outra));
}

#[test]
fn limpar_recentes_e_historico_valem_no_salvar() {
    let amb = Ambiente::exemplo("pref_limpar");
    amb.servico.prefs().registrar_sql(amb.caminho_banco(), "SELECT 1", None, None);
    // um recente além do banco aberto (que o menu não repete)
    let outro = amb.pasta().join("outro.db");
    criar_banco(&outro, "CREATE TABLE t (id INTEGER PRIMARY KEY);");
    amb.servico.prefs().adicionar_recente(&outro);
    let mut h = harness(&amb);
    assert!(!amb.servico.prefs().recentes().is_empty());
    abrir_menu_arquivo(&mut h);
    assert!(item_ligado(&h, "Abertos recentemente"), "antes de limpar, o menu tem o outro banco");
    h.key_press(Key::Escape);
    h.run_steps(2);
    abrir_preferencias(&mut h);
    // Geral: a lista de Abertos recentemente
    assert!(tem_texto(&h, "banco na lista") || tem_texto(&h, "bancos na lista"), "{:?}", textos(&h));
    clicar_botao(&mut h, "Limpar a lista");
    assert!(tem_texto(&h, "A lista será limpa ao salvar."));
    assert!(salvar_ligado(&h));
    assert!(!amb.servico.prefs().recentes().is_empty(), "nada some antes do Salvar");
    clicar_botao(&mut h, "Manter a lista");
    assert!(!salvar_ligado(&h));
    clicar_botao(&mut h, "Limpar a lista");
    // Consulta SQL: o histórico
    ir_para_secao(&mut h, "Consulta SQL");
    assert!(salvar_ligado(&h), "a marca da outra seção continua");
    clicar_botao(&mut h, "Limpar o histórico");
    assert!(tem_texto(&h, "O histórico será limpo ao salvar."));
    clicar_ligado(&mut h, "Salvar");
    assentar(&mut h);
    assert!(amb.servico.prefs().recentes().is_empty());
    assert!(amb.servico.prefs().historico_sql(amb.caminho_banco()).is_empty());
    assert!(h.state().estado().prefs.recentes.is_empty(), "a casca relê a lista");
    abrir_menu_arquivo(&mut h);
    assert!(!item_ligado(&h, "Abertos recentemente"), "sem recentes no menu");
}

#[test]
fn cancelar_desfaz_a_limpeza_marcada() {
    let amb = Ambiente::exemplo("pref_limpar_cancelar");
    let mut h = harness(&amb);
    abrir_preferencias(&mut h);
    clicar_botao(&mut h, "Limpar a lista");
    clicar_botao(&mut h, "Cancelar");
    assentar(&mut h);
    assert!(!amb.servico.prefs().recentes().is_empty());
    abrir_preferencias(&mut h);
    assert!(!salvar_ligado(&h), "a janela nova começa sem a marca");
}

// As capturas das Preferências (prancheta 13): cada seção nos dois temas, mais a janela estreita.
fn captura_de_preferencias(tema: Tema, sufixo: &str, tamanho: egui::Vec2, secoes: &[(&str, &str)]) {
    let amb = Ambiente::exemplo(&format!("pref_captura_{sufixo}"));
    let mut h = harness_com(&amb, tema, tamanho, Partida::default(), |_| None);
    abrir_preferencias(&mut h);
    for (secao, nome) in secoes {
        ir_para_secao(&mut h, secao);
        capturar(&mut h, &format!("preferencias-{nome}-{sufixo}.png"));
    }
}

#[test]
fn preferencias_capturas_claro() {
    captura_de_preferencias(
        Tema::Claro,
        "claro",
        TAMANHO,
        &[
            ("Geral", "geral"),
            ("Aparência", "aparencia"),
            ("Consulta SQL", "consulta-sql"),
            ("Exportação", "exportacao"),
        ],
    );
}

#[test]
fn preferencias_capturas_escuro() {
    captura_de_preferencias(
        Tema::Escuro,
        "escuro",
        TAMANHO,
        &[
            ("Geral", "geral"),
            ("Aparência", "aparencia"),
            ("Consulta SQL", "consulta-sql"),
            ("Exportação", "exportacao"),
        ],
    );
}

#[test]
fn preferencias_numa_janela_estreita() {
    captura_de_preferencias(Tema::Claro, "claro-960", egui::vec2(960.0, 600.0), &[("Aparência", "aparencia")]);
}
