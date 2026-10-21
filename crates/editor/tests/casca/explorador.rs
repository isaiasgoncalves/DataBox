//! A barra de ícones, o Explorador, o Histórico de consultas, a barra de status, os recentes do menu Arquivo e a
//! navegação (voltar e avançar, as rotas sem banco).

use super::*;

#[test]
fn casca_explorador_e_status() {
    let amb = Ambiente::exemplo("casca_explorador");
    let mut h = harness(&amb);
    assert_eq!(h.state().titulo(), "exemplo.db · DataBox");
    // o Explorador: o banco (marcado: a Visão geral está à vista) com as Tabelas abertas; Visões e Sistema
    // fechados; o "+ Nova tabela" da lateral de antes saiu (fica o da Visão geral)
    assert!(ligado(&h, Role::Link, "exemplo.db"));
    for t in TABELAS {
        assert!(h.query(by().role(Role::Link).label(t)).is_some(), "{t} no Explorador");
    }
    assert!(h.query(by().role(Role::Link).label("v_pedidos_por_cliente")).is_none());
    assert!(h.query(by().role(Role::Link).label("+ Nova tabela")).is_none());
    clicar_link(&mut h, "Visões");
    assert!(h.query(by().role(Role::Link).label("v_pedidos_por_cliente")).is_some());
    // a barra de ícones: o Explorador ligado, o Início não
    assert!(painel_a_vista(&h, "Explorador") && !(h.state().rota() == &Rota::Inicio));
    // a barra de status: o banco, o modo (a frase da nota de antes vai na dica), o SQLite e a codificação; o
    // tamanho do arquivo não vai nela, nem na Visão geral (D1, opção B: ele fica nos cartões e nas Propriedades)
    assert!(existe(&h, Role::Label, "exemplo.db"));
    let sqlite = format!("SQLite {}", h.state().estado().app.sqlite_versao);
    let tamanho = interface::formato::formatar_bytes(h.state().estado().banco.as_ref().map_or(0, |b| b.tamanho));
    for item in [sqlite.as_str(), "UTF-8"] {
        assert!(existe(&h, Role::Label, item), "{item} na barra de status: {:?}", textos(&h));
    }
    assert!(textos_das_propriedades(&h).contains(&tamanho), "o tamanho fica nas Propriedades");
    let dica = ler_dica(&mut h, Role::Label, "Gravação imediata");
    assert!(dica.contains("Você está editando exemplo.db. Tudo o que for salvo grava no arquivo na hora."), "{dica:?}");
    // o banco no Explorador não tem mais o ponto "Aberto" (revisão de 15/10/2026, item 15)
    assert!(!existe(&h, Role::Image, "Transação aberta"));
    assert!(!dica_do_estado_do_banco(&mut h).contains("Aberto"), "nada de dica de ponto sobre o canto do nó");
    capturar(&mut h, "casca-visao-geral-claro.png");
    // numa tabela, o que ela diz de si, sem o tamanho do arquivo (que fica nas Propriedades: escondidas pelo
    // Ctrl+Alt+B, ele sai da barra de status e do painel; numa tela de página, vai com elas para o fim do documento,
    // a decisão 4)
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert!(existe(&h, Role::Label, "clientes · 60 linhas · 7 colunas"), "{:?}", textos(&h));
    // no fim da seção Arquivo, depois do caminho: no Windows o caminho temporário quebra em mais linhas (e quantas
    // depende do número do processo), e o tamanho pode cair abaixo da dobra do painel, que rola
    let doc = documento(&h);
    assert!(
        h.query_all(by().role(Role::Label).label(&tamanho)).any(|n| n.rect().left() > doc.right()),
        "o tamanho nas Propriedades da tabela: {:?}",
        textos_das_propriedades(&h)
    );
    h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::B);
    h.run_steps(2);
    assert!(!h.state().propriedades_a_vista());
    let doc = documento(&h);
    let fora_do_documento = h
        .query_all(by().role(Role::Label).label(&tamanho))
        .filter(|n| n.rect().left() < doc.left() || n.rect().right() > doc.right())
        .count();
    assert_eq!(fora_do_documento, 0, "o tamanho só no fim do documento, se estiver");
    h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::B);
    h.run_steps(2);
    assert!(h.state().propriedades_a_vista());
    // e na Consulta SQL, "SQL"
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.run_steps(2);
    assert!(existe(&h, Role::Label, "SQL"));
}

#[test]
fn explorador_abre_o_objeto_marca_a_aba_e_recolhe() {
    let amb = Ambiente::exemplo("explorador");
    let mut h = harness(&amb);
    // a tabela é uma folha (revisão de UI de 19/10/2026): o clique abre a aba dela, sem filhos no Explorador
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "clientes");
    for parte in ["Dados", "Estrutura", "Índices e gatilhos", "Definição SQL"] {
        assert!(h.query(by().role(Role::Link).label(&format!("clientes · {parte}"))).is_none(), "{parte}");
    }
    assert!(ligado(&h, Role::Link, "clientes") && !ligado(&h, Role::Link, "exemplo.db"));
    capturar(&mut h, "explorador-expandido-claro.png");
    // a visão também é uma folha, e fica marcada com a aba dela à vista
    clicar_link(&mut h, "Visões");
    clicar_link(&mut h, "v_pedidos_por_cliente");
    assentar(&mut h);
    assert!(ligado(&h, Role::Link, "v_pedidos_por_cliente") && !ligado(&h, Role::Link, "clientes"));
    assert!(h.query(by().role(Role::Link).label("v_pedidos_por_cliente · Definição SQL")).is_none());
    // Recolher tudo fecha a árvore; outra aba à vista abre os pais dela de novo
    clicar_botao(&mut h, "Recolher tudo");
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none());
    clicar_aba(&mut h, "clientes");
    assentar(&mut h);
    assert!(ligado(&h, Role::Link, "clientes"));
    assert!(h.query(by().role(Role::Link).label("v_pedidos_por_cliente")).is_none(), "as Visões ficam fechadas");
    // o banco foca a Visão geral
    clicar_link(&mut h, "exemplo.db");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
    // Ctrl+B esconde e o Exibir mostra; o AltGr+B (Ctrl+Alt) não mexe
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(2);
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none());
    assert!(!painel_a_vista(&h, "Explorador"));
    h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::B);
    h.run_steps(2);
    assert!(!painel_a_vista(&h, "Explorador"));
    exibir(&mut h, "Explorador");
    assert!(painel_a_vista(&h, "Explorador"));
    assert!(h.query(by().role(Role::Link).label("clientes")).is_some());
}

#[test]
fn historico_vazio() {
    let amb = Ambiente::exemplo("historico_vazio");
    let mut h = harness(&amb);
    exibir(&mut h, "Histórico de consultas");
    assentar(&mut h);
    // N5: o relógio de 48px, o texto em `--t2` e a explicação em `--t3`
    assert!(tem_texto(&h, "Nenhuma consulta ainda."));
    assert!(tem_texto(&h, "As consultas executadas aparecem aqui."));
    capturar(&mut h, "historico-vazio-claro.png");
}

/// Dois cliques (o duplo clique do egui) no meio de `rect`.
fn duplo_clique(h: &mut H, rect: egui::Rect) {
    let p = rect.center();
    let _ =
        comandos_dos_eventos(h, &[mover(p), apertar(p, true), apertar(p, false), apertar(p, true), apertar(p, false)]);
}

#[test]
fn historico_clique_seleciona_e_duplo_clique_abre() {
    let amb = Ambiente::exemplo("historico");
    let mut h = harness(&amb);
    exibir(&mut h, "Histórico de consultas");
    assentar(&mut h);
    // no lugar do Explorador
    assert!(painel_a_vista(&h, "Histórico") && !painel_a_vista(&h, "Explorador"));
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none());
    assert!(tem_texto(&h, "Nenhuma consulta ainda."));
    // dois comandos executados (o serviço os guarda no histórico do banco): a resposta de uma Consulta relê a lista
    let id = amb.estado.banco_id();
    amb.servico.sql(&id, "SELECT 1 AS um", 10).expect("um");
    amb.servico.sql(&id, "SELECT count(*)\n  FROM clientes", 10).expect("dois");
    h.state_mut().emitir(estado::Acao::NotarTransacao(false));
    let resumo = "SELECT count(*) FROM clientes";
    esperar(&mut h, "a lista nova", |h| h.query(by().role(Role::Button).label(resumo)).is_some());
    assert!(rect_de(&h, Role::Button, resumo).top() < rect_de(&h, Role::Button, "SELECT 1 AS um").top(), "o mais novo");
    // o clique seleciona (o `.hist.on`), com o SQL inteiro na dica, e não abre nada
    clicar_botao(&mut h, resumo);
    assentar(&mut h);
    assert!(ligado(&h, Role::Button, resumo) && !ligado(&h, Role::Button, "SELECT 1 AS um"));
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert_eq!(ler_dica(&mut h, Role::Button, resumo).lines().next(), Some("SELECT count(*)"));
    capturar(&mut h, "historico-claro.png");
    // o Enter com o item focado abre uma Consulta nova com o comando inteiro
    h.get(by().role(Role::Button).label(resumo)).focus();
    h.run_steps(1);
    h.key_press(Key::Enter);
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 1");
    assert_eq!(h.state().rota(), &Rota::consulta_com("SELECT count(*)\n  FROM clientes"));
    // o duplo clique também, numa Consulta nova (a 2)
    let r = rect_de(&h, Role::Button, "SELECT 1 AS um");
    duplo_clique(&mut h, r);
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 2");
    assert_eq!(h.state().rota(), &Rota::consulta_com("SELECT 1 AS um"));
    // clicar de novo esconde; o Explorador volta pelo botão dele
    exibir(&mut h, "Histórico de consultas");
    assert!(!painel_a_vista(&h, "Histórico") && !painel_a_vista(&h, "Explorador"));
    assert!(h.query(by().role(Role::Button).label("SELECT 1 AS um")).is_none());
    exibir(&mut h, "Explorador");
    assert!(h.query(by().role(Role::Link).label("clientes")).is_some());
    // sem banco
    exibir(&mut h, "Histórico de consultas");
    abrir_menu_arquivo(&mut h);
    clicar_botao(&mut h, "Fechar banco");
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert!(existe(&h, Role::Label, "Nenhum banco aberto."));
    assert!(h.query(by().role(Role::Button).label("SELECT 1 AS um")).is_none());
}

/// A consulta que falhou também entra no Histórico (0.2.0): é a que a pessoa mais quer recuperar. O erro vai na dica,
/// com a pílula "Erro" ou "Parou no K" embaixo do resumo.
#[test]
fn historico_guarda_a_consulta_que_falhou() {
    let amb = Ambiente::exemplo("historico_falhou");
    let mut h = harness(&amb);
    exibir(&mut h, "Histórico de consultas");
    assentar(&mut h);
    let id = amb.estado.banco_id();
    amb.servico.sql(&id, "SELECT * FROM fantasma", 10).expect("um");
    amb.servico.sql(&id, "SELECT 1; SELECT * FROM fantasma", 10).expect("dois");
    h.state_mut().emitir(estado::Acao::NotarTransacao(false));
    let roteiro = "SELECT 1; SELECT * FROM fantasma";
    esperar(&mut h, "a lista nova", |h| h.query(by().role(Role::Button).label(roteiro)).is_some());
    let dica = ler_dica(&mut h, Role::Button, "SELECT * FROM fantasma");
    assert!(dica.starts_with("SELECT * FROM fantasma\n\nErro: "), "{dica}");
    let dica = ler_dica(&mut h, Role::Button, roteiro);
    assert!(dica.starts_with("SELECT 1; SELECT * FROM fantasma\n\nParou no comando 2: "), "{dica}");
    // selecionado, a pílula de erro fica só no contorno (a captura mostra)
    clicar_botao(&mut h, roteiro);
    assentar(&mut h);
    assert!(ligado(&h, Role::Button, roteiro));
    capturar(&mut h, "historico-com-erro-claro.png");
}

#[test]
fn menu_arquivo_abre_os_recentes() {
    let amb = Ambiente::exemplo("menu_recentes");
    let estoque = amb.pasta().join("estoque_2026.db");
    criar_banco(&estoque, "CREATE TABLE itens (id INTEGER PRIMARY KEY);");
    amb.servico.prefs().adicionar_recente(&estoque);
    amb.servico.prefs().adicionar_recente(&amb.estado.banco.as_ref().expect("banco").caminho);
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    h.get(by().role(Role::Button).label("Abertos recentemente")).hover();
    h.run_steps(3);
    // o banco aberto não entra
    assert!(h.query(by().role(Role::Button).label("estoque_2026.db")).is_some(), "{:?}", textos(&h));
    assert!(h.query(by().role(Role::Button).label("exemplo.db")).is_none());
    capturar(&mut h, "menu-arquivo-recentes-claro.png");
    h.get(by().role(Role::Button).label("estoque_2026.db")).click();
    esperar(&mut h, "abrir o estoque", |h| {
        h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "estoque_2026.db")
    });
    assentar(&mut h);
    assert!(!menu_aberto(&h.ctx));
    assert_eq!(h.state().rota(), &Rota::Banco);

    // sem recentes além do aberto, o item fica desligado
    let amb = Ambiente::exemplo("menu_sem_recentes");
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    assert!(h.get(by().role(Role::Button).label("Abertos recentemente")).accesskit_node().is_disabled());
}

#[test]
fn navega_para_tabela_troca_de_aba_volta_e_avanca() {
    let amb = Ambiente::exemplo("navegacao");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));
    assert!(tem_texto(&h, "60 linhas · 7 colunas"));
    assert!(botao_ligado(&h, "Exportar"), "o Exportar da barra");
    assert!(ligado(&h, Role::Link, "clientes"), "o objeto à vista marcado no Explorador");
    assert_eq!(h.state().vista_da_tabela(), Some("Resumo"));

    // as partes viram vistas das Propriedades (revisão de UI de 19/10/2026): Alt+2 a Alt+4 e Alt+1, sem trocar de aba
    for (n, vista) in [(2, "Colunas"), (3, "Índices"), (4, "SQL"), (1, "Resumo")] {
        vista_da_tabela(&mut h, n);
        assert_eq!(h.state().vista_da_tabela(), Some(vista));
        assert_eq!(h.state().rota(), &Rota::objeto("clientes", false), "a aba é a mesma: {vista}");
    }
    // a rota de uma parte (o "Colunas" da Visão geral) abre a aba do objeto com a vista
    h.state_mut().emitir(estado::Acao::Navegar(Rota::objeto_na_aba("clientes", false, Aba::Estrutura)));
    assentar(&mut h);
    assert_eq!(h.state().vista_da_tabela(), Some("Colunas"));
    assert_eq!(rotulos_das_abas(&h).0, s(&["exemplo.db · Visão geral", "clientes"]));

    // Alt+← e Alt+→, entre as abas
    clicar_link(&mut h, "pedidos");
    assentar(&mut h);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    assentar(&mut h);
    assert_eq!(h.state().rota().nome_objeto(), Some("clientes"));
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("pedidos", false));

    // os botões laterais do mouse
    for (botao, esperado) in [(PointerButton::Extra1, "clientes"), (PointerButton::Extra2, "pedidos")] {
        let pos = egui::pos2(800.0, 600.0);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos, button: botao, pressed, modifiers: Modifiers::NONE });
        }
        h.step();
        assentar(&mut h);
        assert_eq!(h.state().rota().nome_objeto(), Some(esperado));
    }

    // a aba da Visão geral (a trilha saiu com o cabeçalho dos Dados)
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);

    // uma Consulta SQL pelo "+" da faixa e o Início pelo Alt+Home (o trilho saiu)
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assert_eq!(h.state().rota(), &Rota::Consulta { sql: None });
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 1");
    ir_ao_inicio(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::Consulta { sql: None });
}

/// Alt+← com uma célula da grade em edição: o valor é gravado antes de a aba Dados sair da tela, e Ctrl+Shift+O (modificador a mais) não abre o diálogo.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da grade de verdade (tela_dados), que o substituto não tem")]
fn alt_seta_com_celula_em_edicao_grava_antes_de_sair() {
    let amb = Ambiente::exemplo("edicao_e_alt_seta");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    esperar_texto(&mut h, "Linhas 1–60 de 60");
    esperar(&mut h, "a grade", |h| h.query_by_label("Grade").is_some());
    // a coluna email (texto), pelo `th` dela, no meio da grade: o clique ativa a célula de alguma linha e dá o foco
    // à grade
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
    // digitar na célula ativa começa a edição, como numa planilha
    h.event(egui::Event::Text("alt.seta@example.test".into()));
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run_steps(3);
    assert_eq!(h.state().rota(), &Rota::Banco, "Alt+← volta à visão geral");
    assert!(estado::esperar_tarefas(std::time::Duration::from_secs(10)), "a gravação terminou");
    assert_eq!(
        ler_texto(&amb.caminho_banco(), "SELECT count(*) FROM clientes WHERE email = 'alt.seta@example.test'"),
        "Inteiro(1)",
        "o valor digitado foi gravado"
    );

    // Ctrl+Shift+O não é o atalho de abrir arquivo: é o da pasta (o diálogo cancela aqui)
    let shift_ctrl = Modifiers { shift: true, ..CTRL };
    h.key_press_modifiers(shift_ctrl, Key::O);
    h.run_steps(3);
    assert!(!h.state().trocando(), "nenhuma troca de banco começou");
}

#[test]
fn visao_tem_so_dados_e_definicao() {
    let amb = Ambiente::exemplo("visao");
    let mut h = harness(&amb);
    // as Visões começam fechadas no Explorador
    clicar_link(&mut h, "Visões");
    clicar_link(&mut h, "v_pedidos_por_cliente");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("v_pedidos_por_cliente", true));
    // a visão é uma folha no Explorador; nas Propriedades, só o Resumo e o SQL (o Alt+2 não vale)
    assert!(existe(&h, Role::Link, "v_pedidos_por_cliente"));
    assert!(existe(&h, Role::Button, "Resumo") && existe(&h, Role::Button, "SQL"));
    assert!(!existe(&h, Role::Button, "Colunas"));
    vista_da_tabela(&mut h, 2);
    assert_eq!(h.state().vista_da_tabela(), Some("Resumo"));
    vista_da_tabela(&mut h, 4);
    assert_eq!(h.state().vista_da_tabela(), Some("SQL"));
    assert!(tem_texto(&h, "· visão"), "o status diz visão");
    assert!(tem_texto(&h, "Visões"));
    capturar(&mut h, "visao-dados-claro.png");

    // pedir a visão como tabela (ou numa aba que ela não tem) corrige o endereço sem entrar no histórico
    h.state_mut().ir(Rota::objeto_na_aba("v_pedidos_por_cliente", false, Aba::Estrutura));
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("v_pedidos_por_cliente", true));
}

#[test]
fn tabela_inexistente_tem_link_para_a_visao_geral() {
    let amb = Ambiente::exemplo("inexistente");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("fantasma", false));
    esperar_texto(&mut h, "A tabela fantasma não existe mais neste banco.");
    // o link logo embaixo do `.empty` (o `p.centro{margin:0}`), centrado
    let vazio = h.get_by_label("A tabela fantasma não existe mais neste banco.").rect();
    let link = rect_de(&h, Role::Link, "Ir para a visão geral do banco");
    assert!(link.top() - vazio.bottom() < 4.0, "sem o espaço entre os itens do corpo: {vazio:?} {link:?}");
    assert!((link.center().x - vazio.center().x).abs() < 1.0, "centrado: {vazio:?} {link:?}");
    capturar(&mut h, "tabela-inexistente-claro.png");
    clicar_link(&mut h, "Ir para a visão geral do banco");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
}

#[test]
fn sem_banco_toda_rota_volta_ao_inicio() {
    let amb = Ambiente::vazio("sem_banco_rotas");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    h.state_mut().ir(Rota::Consulta { sql: None });
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: Some("SELECT 1".into()) });
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::Inicio, "a Nova consulta também precisa de banco");
}
