//! A transação aberta (o aviso, o COMMIT, a pergunta ao fechar, também minimizada), fechar o banco e o banco
//! trocado por outro lugar.

use super::*;

#[test]
fn transacao_aberta_avisa_fora_da_consulta_e_pergunta_ao_fechar() {
    let amb = Ambiente::exemplo("transacao");
    let caminho = amb.caminho_banco();
    let original = ler_texto(&caminho, "SELECT cidade FROM clientes WHERE id = 1");
    let r = amb
        .servico
        .sql(&amb.estado.banco_id(), "BEGIN; UPDATE clientes SET cidade = 'Teresina' WHERE id = 1;", 100)
        .expect("begin");
    assert!(r.transacao_aberta);
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    let dica = ler_dica(&mut h, Role::Label, "Transação aberta");
    assert_eq!(dica, frase_da_transacao("Consulta SQL"), "a dica do modo");
    capturar(&mut h, "aviso-transacao-claro.png");

    // na Consulta SQL o aviso da casca não aparece; o link leva até ela
    clicar_link(&mut h, "Ir para a Consulta SQL");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Consulta { sql: None });
    assert!(!tem_texto(&h, "Há uma transação aberta na Consulta SQL."));

    // fechar o banco pergunta (a Consulta 1, aberta pelo link, ficou com a transação; sem as telas de verdade, o
    // esboço da Consulta não a toma); desistir deixa o banco aberto
    let fechar = if cfg!(feature = "telas") {
        "Fechar o banco descarta a transação aberta na Consulta 1"
    } else {
        "Fechar o banco descarta a transação aberta na Consulta SQL"
    };
    abrir_menu_arquivo(&mut h);
    clicar_botao(&mut h, "Fechar banco");
    esperar_texto(&mut h, fechar);
    assert!(tem_texto(&h, "também o que foi salvo pelas abas Dados e Estrutura"));
    capturar(&mut h, "confirmar-transacao-claro.png");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!tem_texto(&h, "Fechar o banco descarta"));
    assert!(h.state().estado().tem_banco());

    abrir_menu_arquivo(&mut h);
    clicar_botao(&mut h, "Fechar banco");
    esperar_texto(&mut h, fechar);
    clicar_botao(&mut h, "Descartar e continuar");
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert_eq!(h.state().titulo(), "DataBox");
    assert_eq!(ler_texto(&caminho, "SELECT cidade FROM clientes WHERE id = 1"), original, "nada foi para o arquivo");
}

#[test]
fn aviso_de_transacao_escuro() {
    let amb = Ambiente::exemplo("transacao_escuro");
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    capturar(&mut h, "aviso-transacao-escuro.png");
}

#[test]
fn commit_tira_o_aviso_da_transacao() {
    let amb = Ambiente::exemplo("commit");
    let id = amb.estado.banco_id();
    amb.servico.sql(&id, "BEGIN;", 10).expect("begin");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    let dica = ler_dica(&mut h, Role::Label, "Transação aberta");
    assert!(dica.contains(&frase_da_transacao("Consulta SQL")), "{dica:?}");

    // o "Gravar (COMMIT)" da Consulta SQL: a resposta diz que a transação fechou e a tela avisa a casca
    let r = amb.servico.sql(&id, "COMMIT;", 10).expect("commit");
    assert!(!r.transacao_aberta);
    h.state_mut().emitir(estado::Acao::NotarTransacao(false));
    assentar(&mut h);
    assert!(!tem_texto(&h, "Há uma transação aberta na Consulta SQL."), "o aviso some");
    assert!(!existe(&h, Role::Label, "Transação aberta"));
    let dica = ler_dica(&mut h, Role::Label, "Gravação imediata");
    assert!(dica.contains("Tudo o que for salvo grava no arquivo na hora."), "o modo volta: {dica:?}");

    // e a visão geral (que relê o BancoInfo) também não mostra o aviso
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    assert!(!h.state().estado().transacao_aberta());
    assert!(!tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    assert!(existe(&h, Role::Label, "Gravação imediata"));
}

#[test]
fn fechar_a_janela_com_transacao_aberta_pergunta_antes() {
    let amb = Ambiente::exemplo("fechar_janela");
    let mut h = harness(&amb);
    let pedir_fechar = |h: &mut H| {
        h.input_mut().viewports.get_mut(&ViewportId::ROOT).expect("viewport").events.push(egui::ViewportEvent::Close);
        h.step();
    };
    // sem transação, fecha sem perguntar
    pedir_fechar(&mut h);
    assert!(!comandos(&h).contains(&ViewportCommand::CancelClose));
    assert!(!h.state().janela_aberta());

    // o BEGIN feito por fora: a casca relê o BancoInfo (voltar à aba da Visão geral só a foca, sem reler)
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(estado::Acao::RecarregarBanco);
    assentar(&mut h);
    assert!(h.state().estado().transacao_aberta());
    pedir_fechar(&mut h);
    assert!(comandos(&h).contains(&ViewportCommand::CancelClose), "a janela não fecha sozinha");
    esperar_texto(&mut h, "Fechar o DataBox descarta a transação aberta na Consulta SQL");
    // desistir deixa a janela aberta; o "Fechar a janela" da barra de título faz o mesmo caminho (o eframe devolve
    // o Close como o pedido de fechar de sempre), e a pergunta aparece de novo
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    let enviados = clicar_no(&mut h, Role::Button, "Fechar a janela");
    assert!(enviados.contains(&ViewportCommand::Close), "{enviados:?}");
    pedir_fechar(&mut h);
    assert!(comandos(&h).contains(&ViewportCommand::CancelClose));
    esperar_texto(&mut h, "Fechar o DataBox descarta a transação aberta na Consulta SQL");
    h.get(by().role(Role::Button).label("Descartar e fechar")).click();
    let mut fechou = false;
    for _ in 0..4 {
        h.step();
        fechou |= comandos(&h).contains(&ViewportCommand::Close);
    }
    assert!(fechou, "confirmado, a janela fecha");
}

#[test]
fn fechar_com_a_janela_minimizada_pergunta_antes() {
    let amb = Ambiente::exemplo("fechar_minimizada");
    let mut h = harness(&amb);
    // minimizada, o eframe não desenha quadro e só chama o `App::logic` (pelo `run_logic`)
    let pedir_fechar_minimizada = |h: &mut H| {
        let mut raw = egui::RawInput::default();
        let info = raw.viewports.entry(ViewportId::ROOT).or_default();
        info.minimized = Some(true);
        info.events.push(egui::ViewportEvent::Close);
        let ctx = h.ctx.clone();
        let saida = ctx.run_logic(&raw, |c| h.state_mut().logica(c));
        saida.viewport_commands.get(&ViewportId::ROOT).cloned().unwrap_or_default()
    };
    // sem transação, fecha sem perguntar
    assert!(!pedir_fechar_minimizada(&mut h).contains(&ViewportCommand::CancelClose));
    h.run_steps(2);
    assert!(!h.state().janela_aberta());

    // o BEGIN feito por fora: a casca relê o BancoInfo
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(estado::Acao::RecarregarBanco);
    assentar(&mut h);
    assert!(h.state().estado().transacao_aberta());
    let restaurar = pedir_fechar_minimizada(&mut h);
    assert!(restaurar.contains(&ViewportCommand::CancelClose), "a janela não fecha sozinha: {restaurar:?}");
    assert!(restaurar.contains(&ViewportCommand::Minimized(false)), "e volta à vista: {restaurar:?}");
    assert!(restaurar.contains(&ViewportCommand::Focus));

    // o primeiro quadro da janela restaurada faz a pergunta de sempre
    esperar_texto(&mut h, "Fechar o DataBox descarta a transação aberta na Consulta SQL");
    h.get(by().role(Role::Button).label("Descartar e fechar")).click();
    let mut fechou = false;
    for _ in 0..4 {
        h.step();
        fechou |= comandos(&h).contains(&ViewportCommand::Close);
    }
    assert!(fechou, "confirmado, a janela fecha");
}

#[test]
fn fechar_banco_volta_ao_inicio() {
    let amb = Ambiente::exemplo("fechar_banco");
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    clicar_botao(&mut h, "Fechar banco");
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert_eq!(h.state().titulo(), "DataBox");
    assert!(tem_texto_exato(&h, "Nenhuma pasta aberta."), "o Explorador vazio");
    assert!(h.query(by().role(Role::Link).label("exemplo.db")).is_none());
    assert!(existe(&h, Role::Label, "Nenhuma pasta aberta"), "a barra de status");
    assert!(amb.servico.estado().expect("estado").banco.is_none());
    // voltar não leva a uma tela de banco
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::Inicio);
}

#[test]
fn banco_trocado_por_outro_lugar_ressincroniza() {
    let amb = Ambiente::exemplo("banco_trocado");
    let outro = amb.pasta().join("outro.db");
    criar_banco(
        &outro,
        "CREATE TABLE clientes (id INTEGER PRIMARY KEY, nome TEXT); INSERT INTO clientes VALUES (1, 'Intocado');",
    );
    let mut h = harness(&amb);
    // outro lugar abre outro banco por baixo desta tela
    amb.servico.abrir_banco(&outro, false, false).expect("abrir o outro");
    h.state_mut().ir(Rota::objeto("clientes", false));
    esperar(&mut h, "o toast do banco trocado", |h| {
        toast_atual(&h.ctx).is_some_and(|t| t.contains("O banco aberto mudou"))
    });
    esperar(&mut h, "a casca ressincronizar", |h| {
        h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "outro.db")
    });
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco, "outro arquivo: a visão geral dele");
    assert_eq!(h.state().titulo(), "outro.db · DataBox");
    assert!(!h.state().janela_aberta());

    // o mesmo arquivo reaberto por fora: a tela atual volta
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    // a página da aba Dados (que o `ocupado()` não conta) chega antes de o banco ser reaberto: sem isso, ela podia
    // voltar `BancoTrocado` depois, no meio do clique em "Exportar…", e a ressincronização fechava a janela antes da
    // conferência
    if cfg!(feature = "telas") {
        esperar_texto(&mut h, "Linhas 1–1 de 1");
    }
    let id = h.state().estado().banco_id();
    amb.servico.fechar_banco(&id, true).expect("fechar");
    amb.servico.abrir_banco(&outro, false, false).expect("reabrir");
    clicar_ligado(&mut h, "Exportar");
    assert!(h.state().janela_aberta(), "a janela Exportar aberta");
    // um pedido com o id velho (a releitura dos objetos) volta `BancoTrocado`
    h.state_mut().emitir(estado::Acao::RecarregarObjetos);
    esperar(&mut h, "ressincronizar no mesmo arquivo", |h| h.state().estado().banco_id() != id);
    assentar(&mut h);
    assert!(!h.state().janela_aberta(), "a ressincronização fecha as janelas");
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));

    // e sem banco nenhum: o Início
    let id = h.state().estado().banco_id();
    amb.servico.fechar_banco(&id, true).expect("fechar");
    h.state_mut().emitir(estado::Acao::RecarregarObjetos);
    esperar(&mut h, "ressincronizar sem banco", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
}
