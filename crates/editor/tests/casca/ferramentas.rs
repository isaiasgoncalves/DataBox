//! A barra de ferramentas (a ordem fixa, os verbos de cada aba, o F5, o Executar) e as Propriedades de cada
//! aba com o botão do painel.

use super::*;

/// Os itens da barra de ferramentas na ordem das pranchetas, pelo rótulo de acessibilidade (o Novo pelo que a aba dá
/// a ele: "Nova tabela" na Visão geral).
const ORDEM_DA_BARRA: [&str; 19] = [
    "Voltar",
    "Avançar",
    "Abrir pasta",
    "Abrir arquivo",
    "Salvar",
    "Novo",
    "Nova coluna",
    "Trazer colunas",
    "Duplicar",
    "Excluir",
    "Desfazer",
    "Refazer",
    "Buscar",
    "Filtrar",
    "Atualizar",
    "Executar",
    "Nova consulta",
    "Exportar",
    "Painel de propriedades",
];

/// Onde começa cada item da barra, na ordem dela (o primeiro nó com o rótulo: a barra vem antes do resto). O Novo e a
/// busca levam o rótulo que a aba dá a eles ("Nova tabela", "Buscar tabelas").
fn posicoes_da_barra(h: &H, novo: &str, busca: &str) -> Vec<f32> {
    ORDEM_DA_BARRA
        .iter()
        .map(|&r| {
            let rotulo = match r {
                "Novo" => novo,
                "Buscar" => busca,
                _ => r,
            };
            h.query_all_by_label(rotulo).next().unwrap_or_else(|| panic!("{rotulo} na barra")).rect().left()
        })
        .collect()
}

#[test]
fn barra_de_ferramentas_na_ordem_e_no_mesmo_lugar_em_toda_aba() {
    let amb = Ambiente::exemplo("barra_ordem");
    let mut h = harness(&amb);
    // a Visão geral: o Novo é a "Nova tabela"
    let na_visao_geral = posicoes_da_barra(&h, "Nova tabela", "Buscar tabelas");
    assert!(na_visao_geral.windows(2).all(|p| p[0] < p[1]), "na ordem das pranchetas: {na_visao_geral:?}");
    assert!(botao_ligado(&h, "Nova tabela") && !botao_ligado(&h, "Nova coluna"));
    // a Nova coluna tem o lugar dela (logo depois do Novo), desligada, com o rótulo e a dica (que repete o rótulo:
    // com ela à vista, o texto aparece duas vezes)
    assert_eq!(quantos(&h, "Nova coluna"), 1);
    h.get(by().role(Role::Button).label("Nova coluna")).hover();
    h.run_steps(3);
    assert_eq!(quantos(&h, "Nova coluna"), 2, "a dica do botão desligado");
    h.remove_cursor();
    h.run_steps(2);
    capturar(&mut h, "barra-de-ferramentas-claro.png");
    // nenhum botão muda de lugar ao trocar de aba (a busca da barra muda de dica com a aba; sem ela, "Buscar")
    let telas = cfg!(feature = "telas");
    let mut abas = vec![
        ("tabela", Rota::objeto("clientes", false), if telas { "Buscar em clientes" } else { "Buscar" }),
        ("Início", Rota::Inicio, "Buscar"),
    ];
    abas.push(("consulta", Rota::consulta_com("SELECT 1"), if telas { "Buscar no editor" } else { "Buscar" }));
    for (nome, rota, busca) in abas {
        // nos Dados (etapa 3) o Novo é a "Nova linha"
        let novo = if telas && nome == "tabela" { "Nova linha" } else { "Novo" };
        h.state_mut().ir(rota);
        assentar(&mut h);
        assert_eq!(posicoes_da_barra(&h, novo, busca), na_visao_geral, "{nome}");
        assert!(!botao_ligado(&h, "Nova coluna"), "a Nova coluna fica desligada na etapa 2 ({nome})");
    }
}

#[test]
fn verbos_de_cada_aba_e_os_da_casca() {
    let amb = Ambiente::exemplo("barra_verbos");
    let mut h = harness(&amb);
    // Visão geral: Nova tabela, Atualizar e Exportar; o resto desligado
    for (rotulo, esperado) in [
        ("Nova tabela", true),
        ("Atualizar", true),
        ("Exportar", true),
        ("Salvar", false),
        ("Duplicar", false),
        ("Excluir", false),
        ("Executar", false),
        ("Abrir pasta", true),
        ("Abrir arquivo", true),
        ("Nova consulta", true),
        ("Voltar", false),
        ("Avançar", false),
    ] {
        assert_eq!(botao_ligado(&h, rotulo), esperado, "{rotulo} na Visão geral");
    }
    let dica = ler_dica(&mut h, Role::Button, "Exportar");
    assert!(dica.contains("Exportar o banco"), "{dica:?}");
    // os desligados não respondem
    for rotulo in ["Salvar", "Duplicar", "Executar"] {
        clicar_primeiro(&mut h, Role::Button, rotulo);
        assert!(!h.state().janela_aberta(), "{rotulo}");
    }
    assert_eq!(abas(&h).len(), 1);
    // Exportar abre a janela do banco inteiro
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    let origem = h.state().exportacao_aberta().map(|p| (p.origem.clone(), p.nome_sugerido.clone()));
    assert_eq!(origem, Some((estado::OrigemJanelaExportar::Banco, "exemplo".to_string())));
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    // Nova tabela abre a janela
    clicar_botao(&mut h, "Nova tabela");
    assert!(h.state().janela_aberta(), "a janela Nova tabela");
    h.key_press(Key::Escape);
    h.run_steps(3);

    // a tabela: Exportar "Exportar clientes" (a tabela inteira, sem busca) e Atualizar; os Dados ligam o Novo como
    // "Nova linha" (o substituto, sem as telas, o deixa como "Novo", desligado)
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    let telas = cfg!(feature = "telas");
    let novo = if telas { "Nova linha" } else { "Novo" };
    assert!(botao_ligado(&h, "Exportar") && botao_ligado(&h, "Atualizar") && botao_ligado(&h, novo) == telas);
    assert!(botao_ligado(&h, "Voltar") && !botao_ligado(&h, "Avançar"));
    let dica = ler_dica(&mut h, Role::Button, "Exportar");
    assert!(dica.contains("Exportar clientes"), "{dica:?}");
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    match h.state().exportacao_aberta().map(|p| &p.origem) {
        Some(estado::OrigemJanelaExportar::Tabela { nome, .. }) => assert_eq!(nome, "clientes"),
        outra => panic!("a janela da tabela: {outra:?}"),
    }
    h.key_press(Key::Escape);
    h.run_steps(3);
    // Voltar e Avançar da barra: o histórico único
    clicar_primeiro(&mut h, Role::Button, "Voltar");
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert!(botao_ligado(&h, "Avançar"));
    clicar_primeiro(&mut h, Role::Button, "Avançar");
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));

    // a Consulta SQL: só o Executar (Exportar fica em cada card); a Nova consulta da barra abre outra, vazia, com o
    // Executar desligado até haver texto (a sugestão da Consulta vazia põe o SQL no editor; o substituto não tem)
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 1");
    if telas {
        assert!(!botao_ligado(&h, "Executar"));
        clicar_primeiro(&mut h, Role::Link, "SELECT * FROM clientes LIMIT 100");
        assentar(&mut h);
    }
    assert!(botao_ligado(&h, "Executar") && !botao_ligado(&h, "Exportar") && !botao_ligado(&h, "Atualizar"));

    // o Início: nada da aba
    ir_ao_inicio(&mut h);
    assentar(&mut h);
    for rotulo in ["Novo", "Atualizar", "Exportar", "Executar"] {
        assert!(!botao_ligado(&h, rotulo), "{rotulo} no Início");
    }
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    assert!(!h.state().janela_aberta());

    // Abrir arquivo da barra: o diálogo (que aqui cancela) e nada muda
    clicar_primeiro(&mut h, Role::Button, "Abrir arquivo");
    h.run_steps(3);
    assert!(!h.state().trocando() && h.state().estado().tem_banco());
}

#[test]
fn nova_tabela_desligada_em_so_leitura() {
    let amb = Ambiente::vazio("barra_so_leitura");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let partida = Partida { caminho: Some(caminho), somente_leitura: true };
    let h = harness_com(&amb, Tema::Claro, TAMANHO, partida, |_| None);
    assert!(!botao_ligado(&h, "Nova tabela"), "só leitura: sem Nova tabela");
    assert!(botao_ligado(&h, "Exportar") && botao_ligado(&h, "Atualizar"));
}

#[test]
fn atualizar_e_f5_releem_a_aba() {
    let amb = Ambiente::exemplo("barra_atualizar");
    let id = amb.estado.banco_id();
    let mut h = harness(&amb);
    // uma tabela criada por fora: a Visão geral atualizada relê os objetos (o Explorador também a mostra)
    amb.servico.sql(&id, "CREATE TABLE criada_por_fora (a);", 10).expect("criar");
    assert!(h.query(by().role(Role::Link).label("criada_por_fora")).is_none());
    clicar_primeiro(&mut h, Role::Button, "Atualizar");
    esperar(&mut h, "a tabela nova", |h| h.query(by().role(Role::Link).label("criada_por_fora")).is_some());

    // uma coluna criada por fora: o F5 relê a tabela à vista
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    amb.servico.sql(&id, "ALTER TABLE clientes ADD COLUMN apelido TEXT;", 10).expect("coluna");
    assert!(h.state().tabela_aberta().is_some_and(|t| t.info().colunas.iter().all(|c| c.nome != "apelido")));
    h.key_press(Key::F5);
    esperar(&mut h, "a tabela relida pelo F5", |h| {
        h.state().tabela_aberta().is_some_and(|t| t.info().colunas.iter().any(|c| c.nome == "apelido"))
    });
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false), "a mesma aba");

    // no Início o F5 não faz nada
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    h.key_press(Key::F5);
    h.run_steps(3);
    assert_eq!(h.state().rota(), &Rota::Inicio);
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn executar_da_barra_roda_a_consulta() {
    let amb = Ambiente::exemplo("barra_executar");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    digitar(&mut h, "Comando SQL", "SELECT 41 + 1 AS resposta_da_barra");
    // o Executar da barra (o da tela saiu): o seletor com o Comando 1
    assert!(!tem_texto(&h, "Comando 1 · 1 linha"));
    clicar_primeiro(&mut h, Role::Button, "Executar");
    esperar_texto(&mut h, "Comando 1 · 1 linha");
    assentar(&mut h);
    assert!(tem_texto(&h, "1 linha · "), "{:?}", textos(&h));
    capturar(&mut h, "casca-consulta-barra-claro.png");
}

#[test]
fn propriedades_de_cada_aba_e_o_botao_do_painel() {
    let amb = Ambiente::exemplo("propriedades");
    let mut h = harness(&amb);
    let caminho = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string()).unwrap_or_default();
    // a Visão geral: a ficha "Arquivo" e a "Manutenção"
    assert!(h.state().propriedades_a_vista());
    assert!(ligado(&h, Role::Button, "Painel de propriedades"));
    let t = textos_das_propriedades(&h);
    // as seções têm rótulo próprio ("Seção Arquivo"): o menu Arquivo é o único "Arquivo"
    for item in ["Propriedades · exemplo.db", "Seção Arquivo", caminho.as_str(), "UTF-8", "Seção Manutenção"] {
        assert!(t.iter().any(|x| x == item), "{item} nas Propriedades: {t:?}");
    }
    assert_eq!(h.query_all(by().role(Role::Button).label("Arquivo")).count(), 1, "o rótulo do menu não se repete");
    capturar(&mut h, "propriedades-visao-geral-claro.png");

    // a tabela: a ficha dela e o Arquivo curto
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    let t = textos_das_propriedades(&h);
    for item in ["Seção Tabela", "clientes", "tabela", "60", "7", "não", "Seção Arquivo", caminho.as_str()] {
        assert!(t.iter().any(|x| x == item), "{item} nas Propriedades: {t:?}");
    }
    capturar(&mut h, "propriedades-tabela-claro.png");
    // a visão: "Visão", sem índices nem estrita
    h.state_mut().ir(Rota::objeto("v_pedidos_por_cliente", true));
    assentar(&mut h);
    let t = textos_das_propriedades(&h);
    assert!(t.iter().any(|x| x == "Seção Visão") && t.iter().any(|x| x == "visão"), "{t:?}");
    assert!(!t.iter().any(|x| x == "Seção Tabela"), "{t:?}");

    // a Consulta SQL: a Execução, com a transação, que segue a da conexão (e as Tabelas, na de verdade)
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.run_steps(2);
    assentar(&mut h);
    let t = textos_das_propriedades(&h);
    for item in ["Seção Execução", "nenhuma aberta"] {
        assert!(t.iter().any(|x| x == item), "{item} nas Propriedades: {t:?}");
    }
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(estado::Acao::NotarTransacao(true));
    assentar(&mut h);
    assert!(textos_das_propriedades(&h).iter().any(|x| x == "aberta"));
    amb.servico.sql(&amb.estado.banco_id(), "ROLLBACK;", 10).expect("rollback");
    h.state_mut().emitir(estado::Acao::NotarTransacao(false));
    assentar(&mut h);

    // o botão da barra esconde e mostra; o Ctrl+Alt+B também (mas não com um campo de texto focado)
    clicar_botao(&mut h, "Painel de propriedades");
    assert!(!h.state().propriedades_a_vista() && !ligado(&h, Role::Button, "Painel de propriedades"));
    assert!(textos_das_propriedades(&h).is_empty());
    h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::B);
    h.run_steps(2);
    assert!(h.state().propriedades_a_vista() && ligado(&h, Role::Button, "Painel de propriedades"));
    if cfg!(feature = "telas") {
        h.get_by_label("Comando SQL").focus();
        h.run_steps(1);
        h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::B);
        h.run_steps(2);
        assert!(h.state().propriedades_a_vista(), "o Ctrl+Alt+B fica com o editor de SQL");
    }

    // o Início não tem Propriedades: o painel não aparece e o botão fica desligado, sem a marca
    ir_ao_inicio(&mut h);
    assentar(&mut h);
    assert!(!h.state().propriedades_a_vista());
    assert!(!ligado(&h, Role::Button, "Painel de propriedades") && !botao_ligado(&h, "Painel de propriedades"));
    assert_eq!(documento(&h).right(), TAMANHO.x - 8.0, "o documento fica com a largura");
    // antes, o botão, o item do Exibir e o Ctrl+Alt+B trocavam a escolha no Início, sem nada mudar na tela: agora
    // não fazem nada, e a escolha da pessoa fica (o painel volta na Visão geral)
    abrir_menu(&mut h, "Exibir");
    assert!(!item_ligado(&h, "Propriedades"), "o item do Exibir desligado no Início");
    h.key_press(Key::Escape);
    h.run_steps(2);
    for (i, quem) in ["o botão", "o Ctrl+Alt+B"].into_iter().enumerate() {
        ir_ao_inicio(&mut h);
        assentar(&mut h);
        if i == 0 {
            clicar_botao(&mut h, "Painel de propriedades");
        } else {
            h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::B);
            h.run_steps(2);
        }
        h.state_mut().ir(Rota::Banco);
        assentar(&mut h);
        assert!(h.state().propriedades_a_vista(), "{quem} no Início não muda a escolha");
    }
}
