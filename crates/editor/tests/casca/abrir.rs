//! Abrir: o Início, o caminho digitado, a linha de comando, os recentes, os diálogos (com o gancho dos testes), a
//! janela "Pedir o caminho" sem o diálogo do Windows e as trocas de banco.

use super::*;

#[test]
fn inicio_sem_banco() {
    let amb = Ambiente::vazio("inicio_sem_banco");
    let mut h = harness(&amb);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert_eq!(h.state().titulo(), "DataBox");
    // o título no meio da barra de título, entre os menus e os botões da janela
    assert!(titulo_na_barra(&h, "DataBox").is_some(), "{:?}", textos(&h));
    for menu in ["Arquivo", "Editar", "Exibir", "Ajuda", "Minimizar", "Maximizar", "Fechar a janela"] {
        assert!(existe(&h, Role::Button, menu), "{menu}");
    }
    // o Início da prancheta 4: os três cartões, o caminho e os recentes (vazios)
    assert!(existe(&h, Role::Button, "Abrir pasta") && existe(&h, Role::Button, "Novo banco"));
    assert!(existe(&h, Role::TextInput, "Caminho da pasta ou do arquivo") && tem_texto(&h, "Abertos recentemente"));
    assert!(tem_texto(&h, "Nenhum item aberto ainda. As pastas, os bancos e os arquivos que você abrir aparecem aqui"));
    // o Início e o Explorador à vista na doca da esquerda, o Explorador vazio e a barra de status sem banco (o
    // trilho saiu na revisão de UI de 19/10/2026)
    assert!((h.state().rota() == &Rota::Inicio) && painel_a_vista(&h, "Explorador"));
    assert!(!painel_a_vista(&h, "Histórico"));
    // o Explorador sem pasta nem banco (prancheta 4): o "Abrir pasta" no meio dele
    assert!(tem_texto_exato(&h, "Nenhuma pasta aberta."));
    assert!(tem_texto(&h, "Abra uma pasta para ver os bancos, planilhas e textos que ela tem."));
    // o "Abrir arquivo" da barra de ferramentas e o do Início (sem reticências)
    assert_eq!(h.query_all(by().role(Role::Button).label("Abrir arquivo")).count(), 2);
    assert!(existe(&h, Role::Label, "Nenhuma pasta aberta"), "a barra de status");
    assert!(!existe(&h, Role::Label, "Gravação imediata"));
    assert!(h.query(by().role(Role::Button).label("Banco")).is_none());
    assert!(h.query(by().role(Role::Button).label("Consulta SQL")).is_none());
    assert!(h.query(by().role(Role::Link).label("Visão geral")).is_none());
    capturar(&mut h, "inicio-sem-banco-claro.png");
}

#[test]
fn inicio_sem_banco_escuro() {
    let amb = Ambiente::vazio("inicio_sem_banco_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    assert!(tem_texto(&h, "Abertos recentemente"));
    capturar(&mut h, "inicio-sem-banco-escuro.png");
}

#[test]
fn abrir_por_caminho_digitado() {
    let amb = Ambiente::vazio("abrir_digitado");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let mut h = harness(&amb);
    digitar(&mut h, CAMPO_DO_INICIO, &format!("  \"{}\"  ", caminho.display()));
    clicar_botao(&mut h, "Abrir");
    esperar(&mut h, "o banco abrir", |h| h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert_eq!(h.state().titulo(), "exemplo.db · DataBox");
    let aberto = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string()).unwrap_or_default();
    assert!(aberto.ends_with("exemplo.db") && tem_texto(&h, &aberto), "o caminho no p.sub da visão geral");
    // entrou nos recentes
    let recentes = &h.state().estado().prefs.recentes;
    assert!(recentes.iter().any(|r| r.nome == "exemplo.db"), "{recentes:?}");
    // o banco no Explorador, com a Visão geral marcada, e a Nova consulta ligada (na barra e no "+" da faixa)
    assert!(ligado(&h, Role::Link, "exemplo.db"));
    assert!(!(h.state().rota() == &Rota::Inicio));
    let novas: Vec<bool> = ["Nova consulta", "Nova consulta SQL"]
        .iter()
        .flat_map(|&r| h.query_all(by().role(Role::Button).label(r)).map(|n| n.accesskit_node().is_disabled()))
        .collect();
    assert_eq!(novas, [false, false]);
}

#[test]
fn caminho_vazio_e_inexistente_mostram_o_erro_no_card() {
    let amb = Ambiente::vazio("caminho_erro");
    let mut h = harness(&amb);
    clicar_botao(&mut h, "Abrir");
    esperar_texto(&mut h, "Digite o caminho completo da pasta ou do arquivo.");
    let fantasma = amb.pasta().join("nao_existe.db");
    digitar(&mut h, CAMPO_DO_INICIO, &fantasma.display().to_string());
    clicar_botao(&mut h, "Abrir");
    // "O arquivo do banco não existe…" (abrindo como banco) ou "O arquivo não existe (ou foi movido)…" (a
    // classificação do caminho, passo 8)
    esperar_texto(&mut h, "não existe");
    assert!(!tem_texto(&h, "Digite o caminho completo"), "o erro anterior sai");
    assert!(!h.state().estado().tem_banco());
    assert_eq!(h.state().rota(), &Rota::Inicio);
    capturar(&mut h, "inicio-erro-caminho-claro.png");
    // Enter no campo também abre
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    h.get_by_label(CAMPO_DO_INICIO).focus();
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    digitar(&mut h, CAMPO_DO_INICIO, &caminho.display().to_string());
    let valor = h.get_by_label(CAMPO_DO_INICIO).value();
    assert_eq!(valor.as_deref(), Some(caminho.display().to_string().as_str()), "{:?}", textos(&h));
    h.key_press(Key::Enter);
    esperar(&mut h, "o banco abrir pelo Enter", |h| h.state().estado().tem_banco());
}

#[test]
fn banco_da_linha_de_comando() {
    let amb = Ambiente::vazio("linha_comando");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let partida = Partida { caminho: Some(caminho), somente_leitura: true };
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, partida, |_| None);
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert_eq!(h.state().titulo(), "exemplo.db · DataBox");
    assert!(h.state().estado().somente_leitura());
    let dica = ler_dica(&mut h, Role::Label, "Só leitura");
    assert!(dica.contains("está aberto só para leitura"), "a dica do modo na barra de status: {dica:?}");
    drop(h);

    // o que não abre: o aplicativo abre sem ele e diz por quê no Início, com as linhas em branco
    let amb = Ambiente::vazio("linha_comando_ruim");
    let ruim = amb.pasta().join("estragado.db");
    std::fs::write(&ruim, b"isto nao e um banco sqlite, so texto qualquer para o teste").expect("gravar");
    let partida = Partida { caminho: Some(ruim.clone()), somente_leitura: false };
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, partida, |_| None);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert!(!h.state().estado().tem_banco());
    assert!(tem_texto(&h, &format!("Não foi possível abrir o banco {}.", ruim.display())), "{:?}", textos(&h));
    assert!(tem_texto(&h, "O arquivo não é um banco SQLite"), "{:?}", textos(&h));
    assert!(tem_texto(&h, "O DataBox abre sem ele."), "{:?}", textos(&h));
    capturar(&mut h, "inicio-erro-claro.png");
}

#[test]
fn recentes_abrem_removem_e_mostram_o_que_sumiu() {
    let amb = Ambiente::exemplo("recentes");
    let estoque = amb.pasta().join("estoque_2026.db");
    let financeiro = amb.pasta().join("financeiro.sqlite");
    let antigo = amb.pasta().join("arquivo_antigo.db");
    for (c, sql) in [
        (&antigo, "CREATE TABLE a (x);"),
        (&financeiro, "CREATE TABLE contas (id INTEGER PRIMARY KEY, nome TEXT);"),
        (&estoque, "CREATE TABLE itens (id INTEGER PRIMARY KEY, nome TEXT);"),
    ] {
        criar_banco(c, sql);
        amb.servico.prefs().adicionar_recente(c);
    }
    // o aberto volta ao topo (pelo caminho com que o serviço o guardou)
    amb.servico.prefs().adicionar_recente(&amb.estado.banco.as_ref().expect("banco").caminho);
    std::fs::remove_file(&antigo).expect("apagar o antigo");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    assert!(tem_texto(&h, "Abertos recentemente"), "{:?}", textos(&h));
    assert!(tem_texto(&h, "exemplo.db está aberto. Abrir outro fecha este."));
    assert!(tem_texto(&h, "não encontrado"), "pill do arquivo que sumiu");
    // o leitor de tela ouve o verbo com o arquivo ("Remover vendas.db da lista"); a dica é só o verbo
    assert_eq!(acoes_dos_recentes(&h, "Remover ", " da lista").len(), 4);
    assert_eq!(acoes_dos_recentes(&h, "Mostrar ", " na pasta").len(), 3, "o que sumiu não tem Mostrar na pasta");
    assert!(h.query_by_label("Remover arquivo_antigo.db da lista").is_some());
    assert!(h.query_by_label("Mostrar arquivo_antigo.db na pasta").is_none());
    capturar(&mut h, "inicio-com-recentes-claro.png");

    // o nome do que sumiu é um `strong` que não abre: nem botão, nem mãozinha
    assert!(h.query(by().role(Role::Button).label("arquivo_antigo.db")).is_none());
    h.get(by().role(Role::Label).label("arquivo_antigo.db")).hover();
    h.run_steps(2);
    assert_ne!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand,
        "o nome que sumiu não parece clicável"
    );
    h.get(by().role(Role::Label).label("arquivo_antigo.db")).click();
    h.run_steps(3);
    assert!(h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "exemplo.db"));

    // a dica da pasta é o caminho completo do arquivo (o `title = r.caminho`), mesmo sem encurtar
    let r_antigo = h.state().estado().prefs.recentes.iter().find(|r| r.nome == "arquivo_antigo.db").cloned();
    let r_antigo = r_antigo.expect("o antigo nos recentes");
    h.query_all_by_label(&r_antigo.pasta).last().expect("a pasta do antigo").hover();
    h.run_steps(3);
    assert!(tem_texto(&h, &r_antigo.caminho), "a dica com o caminho: {:?}", textos(&h));
    h.remove_cursor();
    h.run_steps(2);

    // remover da lista (a última linha é a do antigo)
    let ultimo = h.query_all_by_label("Remover arquivo_antigo.db da lista").last().expect("botão");
    ultimo.click();
    esperar(&mut h, "o recente sair", |h| h.state().estado().prefs.recentes.len() == 3);
    assert!(!tem_texto(&h, "não encontrado"));

    // clicar no nome abre aquele banco e vai para a visão geral
    h.get(by().role(Role::Button).label("estoque_2026.db")).click();
    esperar(&mut h, "abrir o estoque", |h| {
        h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "estoque_2026.db")
    });
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert_eq!(h.state().titulo(), "estoque_2026.db · DataBox");
}

#[test]
fn ir_para_o_banco_pelo_aviso_do_inicio() {
    let amb = Ambiente::exemplo("aviso_inicio");
    let mut h = harness(&amb);
    ir_ao_inicio(&mut h);
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    clicar_link(&mut h, "Ir para o banco");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
}

#[test]
fn ctrl_o_abre_pelo_dialogo_e_lembra_a_pasta() {
    let amb = Ambiente::vazio("ctrl_o");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let escolhido = caminho.clone();
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, Partida::default(), move |p| {
        (p == &PedidoDialogo::AbrirBanco).then(|| escolhido.clone())
    });
    h.key_press_modifiers(CTRL, Key::O);
    esperar(&mut h, "abrir pelo Ctrl+O", |h| h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
    let ultimo = amb.servico.prefs().ultimo_diretorio().map(PathBuf::from);
    assert_eq!(ultimo.as_deref(), caminho.parent());
}

#[test]
fn novo_banco_pelo_menu() {
    let amb = Ambiente::exemplo("novo_banco");
    let novo = amb.pasta().join("novo.db");
    let destino = novo.clone();
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, Partida::default(), move |p| {
        (p == &PedidoDialogo::NovoBanco).then(|| destino.clone())
    });
    abrir_menu_arquivo(&mut h);
    clicar_item(&mut h, "Novo banco");
    esperar(&mut h, "criar o banco", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "novo.db"));
    assentar(&mut h);
    assert!(novo.is_file());
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert!(tem_texto(&h, "Nenhuma tabela ainda."));
    assert!(tem_texto(
        &h,
        "O banco não tem tabelas. Use o botão Nova tabela da barra de ferramentas ou a Consulta SQL para criar a primeira."
    ));
}

#[test]
fn abrir_arquivo_do_inicio_cancelado_nao_faz_nada() {
    let amb = Ambiente::vazio("cancelado");
    let mut h = harness(&amb);
    // o do Início é o último na árvore (depois da barra de ferramentas e do Explorador vazio)
    h.query_all(by().role(Role::Button).label("Abrir arquivo")).last().expect("o botão do Início").click();
    h.run_steps(3);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert!(!h.state().estado().tem_banco());
}

#[test]
fn janelas_globais_abrem_pelas_acoes() {
    let amb = Ambiente::exemplo("janelas_globais");
    let mut h = harness(&amb);
    // o Exportar e a Nova tabela da barra (o cabeçalho da Visão geral de antes saiu)
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    assert!(h.state().janela_aberta(), "a janela Exportar");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    clicar_primeiro(&mut h, Role::Button, "Nova tabela");
    assert!(h.state().janela_aberta(), "a janela Nova tabela");
    // Ctrl+B não mexe no Explorador com uma janela aberta
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(2);
    assert!(h.state().janela_aberta());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    assert!(painel_a_vista(&h, "Explorador"));
}

/// O campo da janela "Pedir o caminho" (o rótulo do `.field` também se chama "Caminho do arquivo").
fn campo_caminho(h: &H) -> egui_kittest::Node<'_> {
    h.get(by().role(Role::TextInput).label("Caminho do arquivo"))
}

/// Troca o texto do campo da janela "Pedir o caminho".
fn digitar_caminho(h: &mut H, texto: &str) {
    campo_caminho(h).focus();
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    campo_caminho(h).type_text(texto);
    h.run_steps(2);
}

#[test]
fn pedir_o_caminho_sem_dialogo_nativo() {
    let amb = Ambiente::exemplo("pedir_caminho");
    let outro = amb.pasta().join("outro.db");
    criar_banco(&outro, "CREATE TABLE t (a);");
    let existente = amb.pasta().join("existente.db");
    std::fs::write(&existente, b"conteudo que nao pode sumir sem perguntar").expect("gravar");
    let mut h = harness_sem_dialogo(&amb, egui::vec2(1440.0, 1391.0));

    // Ctrl+O: "Abrir arquivo", com o campo vazio (e o foco nele)
    h.key_press_modifiers(CTRL, Key::O);
    h.run_steps(3);
    assert!(h.state().janela_aberta());
    assert!(tem_texto(&h, "Abrir arquivo"));
    assert_eq!(campo_caminho(&h).value().as_deref(), Some(""));
    assert!(campo_caminho(&h).is_focused());
    capturar(&mut h, "pedir-caminho-claro.png");

    // vazio: o aviso na janela
    clicar_botao(&mut h, "Abrir");
    esperar_texto(&mut h, "Digite o caminho do arquivo.");
    // inexistente: o erro do serviço na janela, que continua aberta
    digitar_caminho(&mut h, &amb.pasta().join("nao_existe.db").display().to_string());
    clicar_botao(&mut h, "Abrir");
    esperar_texto(&mut h, "O arquivo do banco não existe");
    assert!(h.state().janela_aberta());
    assert!(!tem_texto(&h, "Digite o caminho do arquivo."));
    capturar(&mut h, "pedir-caminho-erro-claro.png");
    // o certo (com as aspas do "Copiar como caminho"), com Enter: abre e a janela fecha
    digitar_caminho(&mut h, &format!("\"{}\"", outro.display()));
    h.key_press(Key::Enter);
    esperar(&mut h, "abrir o outro", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "outro.db"));
    assentar(&mut h);
    assert!(!h.state().janela_aberta());
    assert_eq!(h.state().rota(), &Rota::Banco);

    // Arquivo › Novo banco: a janela "Novo banco" com novo.db na pasta do banco aberto
    abrir_menu_arquivo(&mut h);
    clicar_item(&mut h, "Novo banco");
    h.run_steps(2);
    assert!(tem_texto(&h, "Novo banco"));
    let pasta = h.state().estado().banco.as_ref().map(|b| b.pasta.display().to_string()).unwrap_or_default();
    let sugerido = format!("{pasta}{}novo.db", std::path::MAIN_SEPARATOR);
    assert_eq!(campo_caminho(&h).value().as_deref(), Some(sugerido.as_str()));
    clicar_botao(&mut h, "Criar");
    esperar(&mut h, "criar o novo", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "novo.db"));
    assentar(&mut h);
    assert!(amb.pasta().join("novo.db").is_file());
    assert!(!h.state().janela_aberta());

    // a cópia de segurança: "Salvar como"; um arquivo que já existe pergunta antes de substituir
    clicar_botao(&mut h, "Fazer cópia de segurança");
    h.run_steps(2);
    assert!(tem_texto(&h, "Salvar como"));
    let sugerido = format!("{pasta}{}novo (cópia).db", std::path::MAIN_SEPARATOR);
    assert_eq!(campo_caminho(&h).value().as_deref(), Some(sugerido.as_str()));
    digitar_caminho(&mut h, &existente.display().to_string());
    // o "Salvar" da janela (o da barra de ferramentas, por trás, está desligado)
    clicar_ligado(&mut h, "Salvar");
    esperar_texto(&mut h, "Já existe existente.db. Substituir? O conteúdo atual do arquivo se perde.");
    capturar(&mut h, "confirmar-substituir-claro.png");
    // desistir: nada muda
    h.key_press(Key::Escape);
    assentar(&mut h);
    assert_eq!(std::fs::read(&existente).expect("ler"), b"conteudo que nao pode sumir sem perguntar");
    assert!(!tem_texto(&h, "Cópia salva"));
    // de novo, e substituir
    clicar_botao(&mut h, "Fazer cópia de segurança");
    h.run_steps(2);
    digitar_caminho(&mut h, &existente.display().to_string());
    h.key_press(Key::Enter);
    esperar_texto(&mut h, "Já existe existente.db.");
    clicar_botao(&mut h, "Substituir");
    esperar_texto(&mut h, "Cópia salva: existente.db");
    assert!(std::fs::read(&existente).expect("ler").starts_with(b"SQLite format 3"));
}

#[test]
fn troca_demorada_mostra_que_esta_esperando() {
    let amb = Ambiente::exemplo("troca_demorada");
    let outro = amb.pasta().join("outro.db");
    criar_banco(&outro, "CREATE TABLE t (a);");
    let mut h = harness(&amb);
    // uma consulta demorada segura a conexão do banco atual; abrir outro espera por ela
    let servico = std::sync::Arc::clone(&amb.servico);
    let id = amb.estado.banco_id();
    let inicio = std::time::Instant::now();
    let consulta = std::thread::spawn(move || {
        servico.sql(
            &id,
            // ~1,5 s sozinha nesta máquina: o card aparece depois de 0,3 s
            "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 6000000) SELECT count(*) FROM c;",
            10,
        )
    });
    std::thread::sleep(std::time::Duration::from_millis(150));
    h.state_mut().emitir(estado::Acao::AbrirBanco { caminho: outro.clone(), somente_leitura: false });
    esperar_texto(&mut h, "Abrindo o banco…");
    assert!(h.state().trocando());
    capturar(&mut h, "troca-esperando-claro.png");
    esperar(&mut h, "o outro abrir", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "outro.db"));
    assentar(&mut h);
    assert!(!tem_texto(&h, "Abrindo o banco…"), "o card some");
    let r = consulta.join().expect("thread da consulta");
    eprintln!("a consulta levou {:?}: {:?}", inicio.elapsed(), r.as_ref().map(|r| r.resultados.len()));
}

#[test]
fn troca_pedida_aqui_nao_avisa_banco_trocado() {
    let amb = Ambiente::exemplo("troca_propria");
    let outro = amb.pasta().join("outro.db");
    criar_banco(&outro, "CREATE TABLE clientes (id INTEGER PRIMARY KEY);");
    let mut h = harness(&amb);
    // a troca começa e, no mesmo quadro, um pedido com o id antigo volta `BancoTrocado`
    h.state_mut().emitir(estado::Acao::AbrirBanco { caminho: outro.clone(), somente_leitura: false });
    h.state_mut().emitir(estado::Acao::BancoTrocado);
    esperar(&mut h, "abrir o outro", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "outro.db"));
    assentar(&mut h);
    assert!(
        toast_atual(&h.ctx).is_none_or(|t| !t.contains("O banco aberto mudou")),
        "sem o toast de outra janela: {:?}",
        toast_atual(&h.ctx)
    );
    assert_eq!(h.state().rota(), &Rota::Banco);
    // nada puxa a pessoa de volta para a visão geral
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));
}

/// O Início pede a troca por `Acao` (sem canal próprio): a casca reconhece a aba de origem pela marca da ação, e o
/// erro volta ao card "Abrir um banco"; o mesmo pedido sem aba (o menu, a barra) dá o toast.
#[test]
fn erro_do_inicio_volta_ao_card_pela_marca_da_acao() {
    let amb = Ambiente::vazio("erro_pela_marca");
    let mut h = harness(&amb);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    let inicio = h.state().id_aba_ativa();
    let um = amb.pasta().join("um.db");
    h.state_mut().emitir_de(inicio, estado::Acao::AbrirBanco { caminho: um.clone(), somente_leitura: false });
    // (o caminho inteiro quebra em linhas no aviso; o nome do arquivo fica numa só)
    esperar_texto(&mut h, "um.db");
    assentar(&mut h);
    assert!(toast_atual(&h.ctx).is_none(), "o erro foi para o card, sem toast: {:?}", toast_atual(&h.ctx));
    // sem aba: o toast, e o card continua com o erro dele
    let dois = amb.pasta().join("dois.db");
    h.state_mut().emitir(estado::Acao::AbrirBanco { caminho: dois.clone(), somente_leitura: false });
    esperar(&mut h, "o toast do erro", |h| toast_atual(&h.ctx).is_some_and(|t| t.contains("dois.db")));
    assert!(tem_texto(&h, "um.db"), "{:?}", textos(&h));
    assert_eq!(h.state().rota(), &Rota::Inicio);
}

/// Com uma troca de banco em andamento (o `Contexto::trocando`), os botões do Início ficam desligados.
#[test]
fn inicio_desligado_durante_a_troca() {
    let amb = Ambiente::exemplo("inicio_na_troca");
    let outro = amb.pasta().join("outro.db");
    criar_banco(&outro, "CREATE TABLE t (a);");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    // os botões do Início (o último de cada rótulo na árvore: a barra de ferramentas tem o "Abrir arquivo" dela)
    let desligados = |h: &H| -> Vec<bool> {
        ["Abrir arquivo", "Novo banco", "Abrir"]
            .iter()
            .filter_map(|r| h.query_all(by().role(Role::Button).label(r)).last())
            .map(|n| n.accesskit_node().is_disabled())
            .collect()
    };
    assert_eq!(desligados(&h), [false; 3]);
    // uma consulta demorada segura a conexão: a troca espera por ela
    let servico = std::sync::Arc::clone(&amb.servico);
    let id = amb.estado.banco_id();
    let consulta = std::thread::spawn(move || {
        servico.sql(
            &id,
            "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 6000000) SELECT count(*) FROM c;",
            10,
        )
    });
    std::thread::sleep(std::time::Duration::from_millis(150));
    h.state_mut().emitir(estado::Acao::AbrirBanco { caminho: outro.clone(), somente_leitura: false });
    esperar(&mut h, "a troca começar", |h| h.state().trocando());
    assert!(h.state().trocando(), "a troca ainda espera a consulta");
    assert_eq!(desligados(&h), [true; 3]);
    esperar(&mut h, "o outro abrir", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "outro.db"));
    assentar(&mut h);
    let _ = consulta.join();
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    assert_eq!(desligados(&h), [false; 3]);
}

/// A cópia de segurança: a Visão geral só pede (`EscolherCopia`); o diálogo é da casca, que devolve o caminho
/// escolhido à Visão geral (o Windows já perguntou antes de substituir: a tela não pergunta de novo).
#[test]
fn copia_pelo_dialogo_vem_pela_casca() {
    let amb = Ambiente::exemplo("copia_pela_casca");
    let destino = amb.pasta().join("copia pela casca.db");
    std::fs::write(&destino, b"o dialogo do Windows ja perguntou").expect("gravar");
    let pedidos = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let (anotados, escolhido) = (std::rc::Rc::clone(&pedidos), destino.clone());
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1440.0, 1391.0), Partida::default(), move |p| {
        anotados.borrow_mut().push(p.clone());
        matches!(p, PedidoDialogo::CopiaDeSeguranca { .. }).then(|| escolhido.clone())
    });
    clicar_botao(&mut h, "Fazer cópia de segurança");
    esperar_texto(&mut h, "Cópia salva: copia pela casca.db");
    assert_eq!(*pedidos.borrow(), [PedidoDialogo::CopiaDeSeguranca { nome_sugerido: "exemplo (cópia).db".into() }]);
    assert!(std::fs::read(&destino).expect("ler").starts_with(b"SQLite format 3"));
    assert!(!tem_texto(&h, "Já existe"));
    // pedida de fora (sem aba), com outro nome: o mesmo diálogo, e o caminho vai à Visão geral
    pedidos.borrow_mut().clear();
    h.state_mut().emitir(estado::Acao::EscolherCopia { nome_sugerido: "outra.db".into() });
    assentar(&mut h);
    assert_eq!(*pedidos.borrow(), [PedidoDialogo::CopiaDeSeguranca { nome_sugerido: "outra.db".into() }]);
    assert!(tem_texto(&h, "Cópia salva: copia pela casca.db"));
}
