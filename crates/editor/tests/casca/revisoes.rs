//! As correções das revisões da etapa 2.

use super::*;

/// Com a aba de Dados à vista, a Consulta escondida termina um INSERT: a grade à vista relê (antes, ficava com as 60
/// linhas de antes, com o banco já em 61).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn tabela_a_vista_rele_depois_da_consulta_escondida() {
    let amb = Ambiente::exemplo("rele_consulta_escondida");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    esperar_texto(&mut h, "Linhas 1–60 de 60");
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "INSERT INTO clientes (nome) VALUES ('Revisao');");
    // a pessoa volta à aba de Dados (já lida) enquanto a Consulta espera o outro programa
    clicar_aba(&mut h, "clientes");
    assert_eq!(rotulos_das_abas(&h).1, "clientes");
    outro.soltar();
    esperar(&mut h, "a grade à vista reler com a linha nova", |h| tem_texto(h, "Linhas 1–61 de 61"));
    assert_eq!(ler_texto(&amb.caminho_banco(), "SELECT count(*) FROM clientes"), "Inteiro(61)");
}

/// A releitura da aba à vista pedida pela Consulta escondida espera a célula em edição gravar: o valor digitado não
/// se perde, e a grade relê com ele e com a linha nova.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da grade e da Consulta SQL de verdade")]
fn celula_em_edicao_grava_antes_da_releitura_pedida_por_outra_aba() {
    let amb = Ambiente::exemplo("edicao_e_releitura");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    esperar_texto(&mut h, "Linhas 1–60 de 60");
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "INSERT INTO clientes (nome) VALUES ('Revisao');");
    clicar_aba(&mut h, "clientes");
    esperar(&mut h, "a grade", |h| h.query_by_label("Grade").is_some());
    // uma célula da coluna email em edição, no meio da grade
    let valor = "releitura@example.test";
    let grade = h.get_by_label("Grade").rect();
    let email = rect_de(&h, Role::Button, "email");
    let pos = egui::pos2(email.center().x, grade.center().y);
    for pressed in [true, false] {
        h.event(egui::Event::PointerMoved(pos));
        h.event(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run_steps(2);
    h.event(egui::Event::Text(valor.to_string()));
    h.run_steps(2);
    assert!(h.ctx.text_edit_focused(), "a célula em edição");
    // a Consulta escondida termina: a edição termina, a célula grava e só então a grade relê
    outro.soltar();
    esperar(&mut h, "a grade reler com a linha nova", |h| tem_texto(h, "Linhas 1–61 de 61"));
    assert!(estado::esperar_tarefas(std::time::Duration::from_secs(60)), "a gravação terminou");
    let sql = format!("SELECT count(*) FROM clientes WHERE email = '{valor}'");
    assert_eq!(ler_texto(&amb.caminho_banco(), &sql), "Inteiro(1)", "o valor digitado foi gravado");
}

/// O mesmo com a Consulta fechada no meio da execução: a resposta vai para a execução solta, que relê a ativa.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn tabela_a_vista_rele_depois_da_execucao_solta() {
    let amb = Ambiente::exemplo("rele_execucao_solta");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    esperar_texto(&mut h, "Linhas 1–60 de 60");
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "INSERT INTO clientes (nome) VALUES ('Revisao');");
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert_eq!(rotulos_das_abas(&h).1, "clientes", "a Consulta fechou executando");
    outro.soltar();
    esperar(&mut h, "a grade à vista reler com a linha nova", |h| tem_texto(h, "Linhas 1–61 de 61"));
}

/// DROP TABLE numa Consulta escondida com a aba de Dados da tabela à vista: a aba mostra o card de que ela não existe
/// mais (antes, as linhas ficavam para sempre).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn tabela_excluida_por_consulta_escondida_mostra_o_card() {
    let amb = Ambiente::exemplo("drop_escondido");
    amb.servico
        .sql(&amb.estado.banco_id(), "CREATE TABLE efemera (a); INSERT INTO efemera VALUES (1), (2), (3);", 10)
        .expect("criar");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("efemera", false));
    assentar(&mut h);
    esperar_texto(&mut h, "Linhas 1–3 de 3");
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "DROP TABLE efemera;");
    clicar_aba(&mut h, "efemera");
    outro.soltar();
    esperar_texto(&mut h, "A tabela efemera não existe mais neste banco.");
    assentar(&mut h);
    assert!(!tem_texto(&h, "Linhas 1–3 de 3"));
    assert!(h.state().estado().objetos.as_ref().is_some_and(|o| !o.tabelas.iter().any(|t| t.nome == "efemera")));
    assert_eq!(rotulos_das_abas(&h).1, "efemera", "a ativa fica, com o card");
}

/// A Visão geral escondida no meio de uma manutenção não perde o resultado quando outra aba muda o banco (antes, o
/// `RecarregarObjetos` da Consulta a refazia e a verificação se perdia) e o avisa na hora, num toast (C8: o resultado
/// nunca foi um aviso do painel, que só existia com a aba à vista).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn visao_geral_escondida_avisa_o_resultado_da_manutencao_num_toast() {
    use interface::pecas::toast::toast_atual;
    let amb = Ambiente::exemplo("manutencao_toast_escondida");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "INSERT INTO clientes (nome) VALUES ('Revisao');");
    // na Visão geral, a verificação de integridade (espera a Consulta soltar a conexão); depois, outra aba
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    clicar_botao(&mut h, "Verificar integridade");
    clicar_aba(&mut h, "clientes");
    outro.soltar();
    esperar(&mut h, "o toast da verificação", |h| {
        toast_atual(&h.ctx).is_some_and(|t| t.contains("O banco passou na verificação de integridade."))
    });
    assert_eq!(rotulos_das_abas(&h).1, "clientes", "a Visão geral continuou escondida");
    esperar(&mut h, "a grade reler com a linha nova", |h| tem_texto(h, "Linhas 1–61 de 61"));
    // de volta à Visão geral, o resultado não está no painel (era o aviso de antes): só o toast, que já passou
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assert!(!textos_das_propriedades(&h).iter().any(|t| t.contains("passou na verificação")));
}

/// C8: a manutenção que corria quando a Visão geral foi fechada termina do mesmo jeito, com o toast (antes, a
/// resposta se perdia com a tela).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn manutencao_que_corria_quando_a_visao_geral_fechou_avisa_num_toast() {
    use interface::pecas::toast::toast_atual;
    let amb = Ambiente::exemplo("manutencao_toast_fechada");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "INSERT INTO clientes (nome) VALUES ('Revisao');");
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    clicar_botao(&mut h, "Verificar integridade");
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert!(
        !rotulos_das_abas(&h).0.iter().any(|a| a.contains("Visão geral")),
        "a Visão geral fechou: {:?}",
        rotulos_das_abas(&h)
    );
    outro.soltar();
    esperar(&mut h, "o toast da verificação", |h| {
        toast_atual(&h.ctx).is_some_and(|t| t.contains("O banco passou na verificação de integridade."))
    });
}

/// Fechar pela barra de tarefas com a janela minimizada, logo depois de um BEGIN cuja resposta ainda não foi tomada
/// (sem quadro, ninguém a toma): a janela volta e pergunta (antes, fechava sem perguntar com a transação aberta).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn fechar_minimizada_com_a_resposta_do_begin_ainda_nao_recebida() {
    let amb = Ambiente::exemplo("minimizada_begin");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    digitar(&mut h, "Comando SQL", "BEGIN; UPDATE clientes SET cidade = 'Revisao' WHERE id = 1;");
    let antes = h.ctx.cumulative_pass_nr();
    h.key_press_modifiers(CTRL, Key::Enter);
    h.run_steps(1);
    let passadas = h.ctx.cumulative_pass_nr() - antes;
    // a janela foi minimizada: sem quadro, a resposta chega mas ninguém a toma
    assert!(estado::esperar_tarefas(std::time::Duration::from_secs(60)));
    if passadas > 1 && h.state().estado().transacao_aberta() {
        // o egui repetiu a passada no mesmo quadro (descarte de layout, que depende das fontes do sistema) e a
        // segunda já tomou a resposta: o cenário (minimizada antes de tomar) não aconteceu nesta máquina
        amb.servico.sql(&amb.estado.banco_id(), "ROLLBACK;", 10).expect("rollback");
        return;
    }
    assert!(!h.state().estado().transacao_aberta(), "a casca ainda não sabe da transação");
    let mut raw = egui::RawInput::default();
    let info = raw.viewports.entry(ViewportId::ROOT).or_default();
    info.minimized = Some(true);
    info.events.push(egui::ViewportEvent::Close);
    let ctx = h.ctx.clone();
    let saida = ctx.run_logic(&raw, |c| h.state_mut().logica(c));
    let cmds = saida.viewport_commands.get(&ViewportId::ROOT).cloned().unwrap_or_default();
    assert!(cmds.contains(&ViewportCommand::CancelClose), "fecharia sem perguntar com a transação aberta: {cmds:?}");
    // a janela restaurada: o quadro toma a resposta e só então decide (pergunta da transação, sem fechar)
    let mut enviados = Vec::new();
    for _ in 0..4 {
        h.step();
        enviados.extend(comandos(&h));
    }
    assert!(h.state().estado().transacao_aberta());
    assert!(!enviados.contains(&ViewportCommand::Close), "{enviados:?}");
    assert!(h.state().janela_aberta(), "a pergunta da transação");
    assert!(tem_texto(&h, "Fechar o DataBox descarta a transação aberta"), "{:?}", textos(&h));
    amb.servico.sql(&amb.estado.banco_id(), "ROLLBACK;", 10).expect("rollback");
}

/// O F5 numa aba sem o Atualizar (a Consulta SQL) fica com o campo: antes, ele virava o atalho e tirava o foco do
/// editor de SQL.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn f5_sem_atualizar_nao_tira_o_foco_do_campo() {
    let amb = Ambiente::exemplo("f5_sem_atualizar");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    assert!(!botao_ligado(&h, "Atualizar"), "a Consulta não oferece o Atualizar");
    digitar(&mut h, "Comando SQL", "SELECT 1");
    assert!(h.get_by_label("Comando SQL").is_focused());
    h.key_press(Key::F5);
    h.run_steps(3);
    assert!(h.get_by_label("Comando SQL").is_focused(), "o F5 tirou o foco do editor de SQL");
    // com o Atualizar (a Visão geral), o F5 continua sendo o atalho
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assentar(&mut h);
    assert!(botao_ligado(&h, "Atualizar"));
    h.key_press(Key::F5);
    h.run_steps(2);
    esperar(&mut h, "a Visão geral reler", |h| !h.state().ocupado());
}

/// O toast fica acima da barra de status, com o espaço de sempre entre os painéis (antes, ele a cobria).
#[test]
fn toast_fica_acima_da_barra_de_status() {
    let amb = Ambiente::exemplo("toast_acima_do_status");
    let mut h = harness(&amb);
    toast(&h.ctx, "Linha salva.");
    h.run_steps(3);
    let toast = interface::pecas::toast::rect_toast(&h.ctx).expect("o toast à vista");
    let status = h.get(by().role(Role::Label).label("Gravação imediata")).rect();
    assert_eq!(status.height(), interface::pecas::barra_status::ALTURA_STATUS, "o item ocupa a altura da barra");
    assert!(toast.bottom() <= status.top() - 8.0 + 0.5, "o toast {toast:?} cobre a barra de status {status:?}");
    assert!((toast.center().x - TAMANHO.x / 2.0).abs() <= 1.0, "centrado");
}

/// A direita do Fechar é dele: a faixa da direita começa embaixo da barra de título (antes, ela comia os 5px da
/// direita do botão, e o clique ali começava a redimensionar).
#[test]
fn a_direita_do_fechar_fecha_a_janela() {
    let amb = Ambiente::exemplo("direita_do_fechar");
    let mut h = harness(&amb);
    let fechar = rect_de(&h, Role::Button, "Fechar a janela");
    let ponto = egui::pos2(fechar.right() - 2.0, fechar.center().y);
    h.hover_at(ponto);
    h.run_steps(2);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::Default, "o cursor do botão");
    let enviados = clicar_em(&mut h, ponto);
    assert!(enviados.contains(&ViewportCommand::Close), "{enviados:?}");
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::BeginResize(_))), "{enviados:?}");
    // o canto de cima à direita continua redimensionando, e a direita logo embaixo da barra também
    for (pos, direcao) in [
        (egui::pos2(TAMANHO.x - 1.0, 1.0), egui::ResizeDirection::NorthEast),
        (egui::pos2(TAMANHO.x - 2.0, 31.0), egui::ResizeDirection::East),
    ] {
        let enviados = comandos_dos_eventos(&mut h, &[mover(pos), apertar(pos, true)]);
        assert!(enviados.contains(&ViewportCommand::BeginResize(direcao)), "{pos:?}: {enviados:?}");
        comandos_dos_eventos(&mut h, &[apertar(pos, false)]);
    }
}

/// Com um menu aberto encostado na borda da janela, a faixa de redimensionar não fica por cima dele (antes, o cursor
/// e o aperto na beira do menu eram da borda).
#[test]
fn borda_nao_fica_por_cima_de_um_menu_aberto() {
    let amb = Ambiente::exemplo("borda_e_menu");
    // a janela da largura em que o painel do Ajuda (286px, 2px antes do botão) encosta na borda da direita
    let ajuda = {
        let h = harness(&amb);
        botao_menu(&h, "Ajuda").rect()
    };
    let largura = (ajuda.left() - 2.0 + interface::pecas::casca::LARGURA_PAINEL_MENU).round();
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(largura, 700.0), Partida::default(), |_| None);
    abrir_menu(&mut h, "Ajuda");
    assert!(menu_aberto(&h.ctx));
    let item = item_de_menu(&h, "Atalhos do teclado").rect();
    let beira = egui::pos2(largura - 2.0, item.center().y);
    assert!(item.right() + 6.0 >= largura - 0.5, "o painel encosta na borda: {item:?} em {largura}");
    h.hover_at(beira);
    h.run_steps(2);
    assert_ne!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeEast, "a borda por cima do menu");
    let enviados = comandos_dos_eventos(&mut h, &[apertar(beira, true), apertar(beira, false)]);
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::BeginResize(_))), "{enviados:?}");
    // fechado o menu, a borda volta
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!menu_aberto(&h.ctx));
    h.hover_at(egui::pos2(largura - 2.0, 400.0));
    h.run_steps(2);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeEast);
}

/// N3 pela casca: o Interromper tem o lugar dele na barra (depois do Executar), desligado até a Consulta executar,
/// e só então liga (o Executar fica desligado e ocupado); o clique corta o comando; no fim, a barra volta ao que era.
/// Nenhum botão da barra muda de lugar no meio disso (C31).
#[cfg(feature = "telas")]
#[test]
fn interromper_pela_barra_da_casca() {
    let amb = Ambiente::exemplo("interromper_barra");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    // o primeiro de cada nome é o da barra de ferramentas (ela vem antes dos painéis na árvore)
    let lugar = |h: &H, rotulo: &str| h.query_all(by().role(Role::Button).label(rotulo)).next().expect(rotulo).rect();
    let parado = (lugar(&h, "Interromper"), lugar(&h, "Nova consulta"), lugar(&h, "Exportar"));
    assert!(
        h.get(by().role(Role::Button).label("Interromper")).accesskit_node().is_disabled(),
        "desligado enquanto nada executa"
    );
    // desligado, ele é cinza como qualquer botão de ícone desligado, e a dica diz quando ele vale
    assert_eq!(ler_dica(&mut h, Role::Button, "Interromper"), "Interromper (só durante uma execução)");
    digitar(
        &mut h,
        "Comando SQL",
        "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 2000000000) SELECT count(*) FROM c;",
    );
    clicar_primeiro(&mut h, Role::Button, "Executar");
    esperar_texto(&mut h, "Executando · ");
    let executar = h.query_all(by().role(Role::Button).label("Executar")).next().expect("o Executar");
    assert!(executar.accesskit_node().is_disabled(), "o Executar fica desligado enquanto executa");
    let executar = executar.rect();
    let interromper = h.get(by().role(Role::Button).label("Interromper")).rect();
    assert!(interromper.left() >= executar.right() - 1.0, "o Interromper vem depois do Executar");
    assert!(
        !h.get(by().role(Role::Button).label("Interromper")).accesskit_node().is_disabled(),
        "ligado enquanto executa"
    );
    let executando = (lugar(&h, "Interromper"), lugar(&h, "Nova consulta"), lugar(&h, "Exportar"));
    assert_eq!(executando, parado, "nenhum botão da barra muda de lugar quando o Interromper liga");
    capturar(&mut h, "casca-consulta-executando-claro.png");
    // o Esc interrompe da mesma forma que o botão; aqui, o botão. Um pedido que chega antes de a execução pegar a
    // conexão não alcança nada (a máquina carregada faz isso acontecer), e a pessoa clicaria de novo
    let inicio = std::time::Instant::now();
    while !tem_texto(&h, "A consulta foi interrompida.") {
        assert!(inicio.elapsed() < std::time::Duration::from_secs(15), "a execução não foi interrompida");
        if let Some(b) = h.query_all(by().role(Role::Button).label("Interromper")).next() {
            b.click();
        }
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assentar(&mut h);
    assert!(
        h.get(by().role(Role::Button).label("Interromper")).accesskit_node().is_disabled(),
        "volta a desligado, no mesmo lugar"
    );
    assert_eq!(lugar(&h, "Interromper"), parado.0);
    assert!(!tem_texto(&h, "Executando · "));
    assert!(
        !h.query_all(by().role(Role::Button).label("Executar"))
            .next()
            .expect("o Executar")
            .accesskit_node()
            .is_disabled()
    );
}
