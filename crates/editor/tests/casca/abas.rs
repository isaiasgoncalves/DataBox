//! As abas de documento: as partes da tabela e o Início, reabrir foca, fechar pelo ×, pelo meio e pelo Ctrl+W,
//! o Ctrl+Tab e o histórico único, a rolagem de cada aba, as várias Consultas SQL e a transação entre elas, as abas e o
//! banco (fechar, reaberto por fora) e as abas e a estrutura (renomear, excluir, reler, a janela Novo índice).

use super::*;

#[test]
fn abas_do_inicio_ao_banco_e_as_partes_da_tabela() {
    let amb = Ambiente::vazio("abas_partes");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let mut h = harness(&amb);
    // o Início sozinho, sem ×: nem o Ctrl+W o fecha
    assert_eq!(abas(&h), [("Início".to_string(), true)]);
    assert!(h.query_by_label("Fechar a aba Início").is_none());
    let desligadas = ["Nova consulta", "Nova consulta SQL"]
        .iter()
        .all(|&r| h.query_all(by().role(Role::Button).label(r)).all(|n| n.accesskit_node().is_disabled()));
    assert!(desligadas, "sem banco, sem Nova consulta (nem na barra, nem no \"+\")");
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert_eq!(abas(&h), [("Início".to_string(), true)]);

    // abrir o banco troca o Início pela Visão geral
    digitar(&mut h, CAMPO_DO_INICIO, &caminho.display().to_string());
    clicar_botao(&mut h, "Abrir");
    esperar(&mut h, "o banco abrir", |h| h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(abas(&h), [("exemplo.db · Visão geral".to_string(), true)]);

    // a tabela pelo Explorador e outra parte dela (o "Colunas" da Visão geral): uma aba só, por objeto (revisão de
    // UI de 19/10/2026), com a parte à vista nas Propriedades
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    h.state_mut().emitir(estado::Acao::Navegar(Rota::objeto_na_aba("clientes", false, Aba::Estrutura)));
    assentar(&mut h);
    let (rotulos, ativa) = rotulos_das_abas(&h);
    assert_eq!(rotulos, s(&["exemplo.db · Visão geral", "clientes"]));
    assert_eq!(ativa, "clientes");
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false), "o endereço da aba fica o dos Dados");
    assert_eq!(h.state().vista_da_tabela(), Some("Colunas"));
    capturar(&mut h, "casca-abas-claro.png");
}

/// Reabrir a mesma parte foca a aba que já está aberta, com a busca que ela tinha.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn reabrir_a_mesma_parte_foca_a_aba_com_a_busca() {
    let amb = Ambiente::exemplo("abas_mesma_parte");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    digitar(&mut h, "Buscar em clientes", "Recife");
    esperar(&mut h, "a busca no endereço", |h| h.state().rota().dados().is_some_and(|d| d.busca == "Recife"));
    let com_busca = h.state().rota().clone();
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assentar(&mut h);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &com_busca, "a busca ficou");
    assert_eq!(abas(&h).len(), 2);
}

#[test]
fn fechar_abas_pelo_x_pelo_meio_e_pelo_ctrl_w() {
    let amb = Ambiente::exemplo("abas_fechar");
    let mut h = harness(&amb);
    for t in ["clientes", "pedidos", "produtos"] {
        h.state_mut().ir(Rota::objeto(t, false));
        assentar(&mut h);
    }
    // Ctrl+W fecha a ativa e volta à usada antes dela
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert_eq!(rotulos_das_abas(&h).1, "pedidos");
    // o AltGr do ABNT2 (Ctrl+Alt) não fecha
    h.key_press_modifiers(Modifiers { alt: true, ..CTRL }, Key::W);
    h.run_steps(3);
    assert_eq!(abas(&h).len(), 3);
    // o clique do meio e o ×
    h.get(by().role(Role::Tab).label("pedidos")).click_button(PointerButton::Middle);
    h.run_steps(3);
    fechar_aba(&mut h, "clientes");
    assert_eq!(abas(&h), [("exemplo.db · Visão geral".to_string(), true)]);
    // fechar a última abre o Início, que sozinho não fecha
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert_eq!(abas(&h), [("Início".to_string(), true)]);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert!(h.state().estado().tem_banco(), "o banco continua aberto");
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert_eq!(abas(&h).len(), 1);
}

#[test]
fn ctrl_tab_da_a_volta_e_o_historico_pula_a_aba_fechada() {
    let amb = Ambiente::exemplo("abas_ctrl_tab");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    h.state_mut().ir(Rota::objeto("pedidos", false));
    assentar(&mut h);
    let ctrl_shift = Modifiers { shift: true, ..CTRL };
    for (teclas, esperada) in [
        (CTRL, "exemplo.db · Visão geral"),
        (CTRL, "clientes"),
        (ctrl_shift, "exemplo.db · Visão geral"),
        (ctrl_shift, "pedidos"),
    ] {
        h.key_press_modifiers(teclas, Key::Tab);
        h.run_steps(3);
        assert_eq!(rotulos_das_abas(&h).1, esperada);
    }
    // o histórico atravessa as abas: Alt+← volta à anterior
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run_steps(3);
    assert_eq!(rotulos_das_abas(&h).1, "exemplo.db · Visão geral");
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    h.run_steps(3);
    assert_eq!(rotulos_das_abas(&h).1, "pedidos");
    // fechada a de clientes, voltar não para nela
    h.state_mut().ir(Rota::objeto("clientes", false));
    h.state_mut().ir(Rota::objeto("pedidos", false));
    h.run_steps(2);
    fechar_aba(&mut h, "clientes");
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run_steps(3);
    assert_eq!(rotulos_das_abas(&h).1, "exemplo.db · Visão geral");
}

#[test]
fn cada_aba_guarda_a_propria_rolagem() {
    let amb = Ambiente::exemplo("abas_rolagem");
    // baixa o bastante para a lista de tabelas passar do fim da página (a Manutenção fica no painel, que não rola com
    // ela)
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1440.0, 480.0), Partida::default(), |_| None);
    let alvo = |h: &H| rect_de(h, Role::Button, "Tabela com espaço").top();
    let antes = alvo(&h);
    // nada tem o foco: o End rola o documento até o fim
    h.key_press(Key::End);
    let mut anterior = antes;
    esperar(&mut h, "a rolagem parar", |h| {
        let agora = alvo(h);
        let parou = agora < antes && agora == anterior;
        anterior = agora;
        parou
    });
    let rolado = alvo(&h);
    assert!(rolado < antes - 100.0, "{antes} → {rolado}");
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "60 linhas · 7 colunas"), "a aba nova começa no alto");
    h.state_mut().ir(Rota::Banco);
    h.run_steps(3);
    assert_eq!(alvo(&h), rolado, "a Visão geral volta rolada");
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa do editor da Consulta SQL de verdade (tela_consulta)")]
fn tres_consultas_com_textos_separados() {
    let amb = Ambiente::exemplo("tres_consultas");
    let mut h = harness(&amb);
    for n in 1..=3 {
        clicar_primeiro(&mut h, Role::Button, "Nova consulta");
        assentar(&mut h);
        assert_eq!(rotulos_das_abas(&h).1, format!("Consulta {n}"));
        digitar(&mut h, "Comando SQL", &format!("SELECT {n}"));
    }
    capturar(&mut h, "casca-consulta-3-claro.png");
    for n in [1, 2, 3, 1] {
        clicar_aba(&mut h, &format!("Consulta {n}"));
        assert_eq!(h.get_by_label("Comando SQL").value().as_deref(), Some(format!("SELECT {n}").as_str()));
    }
    // fechada a 2, a próxima é a 2 de novo, vazia
    fechar_aba(&mut h, "Consulta 2");
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 2");
    assert_eq!(h.get_by_label("Comando SQL").value().as_deref(), Some(""));
    let (rotulos, _) = rotulos_das_abas(&h);
    assert_eq!(rotulos, s(&["exemplo.db · Visão geral", "Consulta 1", "Consulta 3", "Consulta 2"]));
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn a_transacao_vale_para_todas_as_consultas() {
    let amb = Ambiente::exemplo("transacao_consultas");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    executar(&mut h, "BEGIN; UPDATE clientes SET cidade = 'Teresina' WHERE id = 1;");
    esperar(&mut h, "a transação aberta", |h| h.state().estado().transacao_aberta());
    assert!(h.query(by().role(Role::Button).label("Gravar (COMMIT)")).is_some());

    // a Consulta 2 nasce vendo a transação (a mesma conexão)
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    esperar(&mut h, "o card da transação na 2", |h| {
        h.query(by().role(Role::Button).label("Gravar (COMMIT)")).is_some()
    });
    // a aba de Dados mostra o aviso com o nome da Consulta que abriu a transação, e o "Ir para a Consulta 1" foca ela
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta 1."));
    let dica = ler_dica(&mut h, Role::Label, "Transação aberta");
    assert_eq!(dica, frase_da_transacao("Consulta 1"), "a dica do modo");
    clicar_link(&mut h, "Ir para a Consulta 1");
    h.run_steps(2);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 1");

    // o COMMIT na 2 tira o card da 1
    clicar_aba(&mut h, "Consulta 2");
    clicar_botao(&mut h, "Gravar (COMMIT)");
    esperar(&mut h, "a transação fechar", |h| !h.state().estado().transacao_aberta());
    assentar(&mut h);
    clicar_aba(&mut h, "Consulta 1");
    h.run_steps(2);
    assert!(h.query(by().role(Role::Button).label("Gravar (COMMIT)")).is_none(), "{:?}", textos(&h));
    assert_eq!(ler_texto(&amb.caminho_banco(), "SELECT cidade FROM clientes WHERE id = 1"), "Texto(\"Teresina\")");
}

/// Fechar a aba de uma Consulta que executa não perde a resposta: a casca a recebe e avisa a transação.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn fechar_uma_consulta_que_executa_ainda_avisa_a_transacao() {
    let amb = Ambiente::exemplo("fechar_consulta_executando");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    // o UPDATE espera outro programa soltar o banco: a aba fecha antes da resposta, por mais carregada que a máquina
    // esteja (antes, uma CTE de 6 milhões de linhas fazia a espera, e com a máquina carregada estourava os 15 s)
    let outro = OutroPrograma::prender(&amb.caminho_banco());
    executando(&mut h, "BEGIN; UPDATE clientes SET cidade = 'Teresina' WHERE id = 1;");
    h.key_press_modifiers(CTRL, Key::W);
    h.run_steps(3);
    assert_eq!(abas(&h), [("exemplo.db · Visão geral".to_string(), true)]);
    assert!(!h.state().estado().transacao_aberta());
    outro.soltar();
    esperar(&mut h, "a resposta da Consulta fechada", |h| h.state().estado().transacao_aberta());
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    amb.servico.sql(&amb.estado.banco_id(), "ROLLBACK;", 10).expect("rollback");
}

#[test]
fn fechar_o_banco_fecha_todas_as_abas() {
    let amb = Ambiente::exemplo("abas_fechar_banco");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: Some("SELECT 2".into()) });
    h.run_steps(2);
    assentar(&mut h);
    assert_eq!(abas(&h).len(), 4);
    abrir_menu_arquivo(&mut h);
    clicar_botao(&mut h, "Fechar banco");
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert_eq!(abas(&h), [("Início".to_string(), true)]);
}

#[test]
fn banco_reaberto_por_fora_no_mesmo_arquivo_mantem_as_abas() {
    let amb = Ambiente::exemplo("abas_mesmo_arquivo");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: Some("SELECT 42".into()) });
    h.run_steps(2);
    assentar(&mut h);
    // o mesmo arquivo reaberto por fora: o próximo pedido com o id velho volta `BancoTrocado`
    let id = h.state().estado().banco_id();
    amb.servico.fechar_banco(&id, true).expect("fechar");
    amb.servico.abrir_banco(&amb.caminho_banco(), false, false).expect("reabrir");
    h.state_mut().ir(Rota::objeto("pedidos", false));
    esperar(&mut h, "ressincronizar", |h| h.state().estado().banco_id() != id);
    assentar(&mut h);
    let (rotulos, ativa) = rotulos_das_abas(&h);
    assert_eq!(rotulos, s(&["exemplo.db · Visão geral", "clientes", "Consulta 1", "pedidos"]), "as abas ficam");
    assert_eq!(ativa, "pedidos");
    // cada documento foi refeito: a Consulta com o texto, a tabela relida com o id novo
    clicar_aba(&mut h, "Consulta 1");
    assentar(&mut h);
    assert!(tem_texto(&h, "SELECT 42"), "{:?}", textos(&h));
    clicar_aba(&mut h, "clientes");
    assentar(&mut h);
    assert!(h.state().tabela_aberta().is_some_and(|t| t.nome == "clientes"));
    assert_eq!(h.state().estado().banco_id(), amb.servico.estado().expect("estado").banco.expect("banco").id);
}

#[test]
fn renomear_ou_excluir_fecha_as_abas_da_tabela_fora_de_vista() {
    let amb = Ambiente::exemplo("abas_sumidas");
    let id = amb.estado.banco_id();
    amb.servico
        .sql(&id, "CREATE TABLE velha (a); CREATE TABLE excluida (a); CREATE TABLE ativa (a);", 10)
        .expect("criar");
    let mut h = harness(&amb);
    for rota in [Rota::objeto("velha", false), Rota::objeto("excluida", false), Rota::objeto("ativa", false)] {
        h.state_mut().ir(rota);
        assentar(&mut h);
    }
    assert_eq!(abas(&h).len(), 4);
    // renomeada uma, excluídas duas (uma delas na aba à vista): a lista nova de objetos fecha as abas delas, menos a
    // ativa, que mostra que a tabela não existe mais
    amb.servico
        .sql(&id, "ALTER TABLE velha RENAME TO nova; DROP TABLE excluida; DROP TABLE ativa;", 10)
        .expect("mudar");
    h.state_mut().emitir(estado::Acao::RecarregarObjetos);
    esperar(&mut h, "as abas fecharem", |h| h.state().abas_abertas().len() == 2);
    let (rotulos, ativa) = rotulos_das_abas(&h);
    assert_eq!(rotulos, s(&["exemplo.db · Visão geral", "ativa"]));
    assert_eq!(ativa, "ativa");
}

#[test]
fn mudar_a_estrutura_rele_as_outras_abas_da_tabela() {
    let amb = Ambiente::exemplo("abas_mesma_tabela");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    let clientes = h.state().id_aba_ativa();
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.run_steps(2);
    assentar(&mut h);
    let consulta = h.state().id_aba_ativa();
    assert_ne!(clientes, consulta);
    // outra aba muda a tabela (como um ALTER TABLE na Consulta) e avisa a casca
    amb.servico.sql(&amb.estado.banco_id(), "ALTER TABLE clientes ADD COLUMN apelido TEXT;", 10).expect("coluna");
    h.state_mut().emitir_de(consulta, estado::Acao::RecarregarObjetos);
    h.run_steps(2);
    assentar(&mut h);
    // a da tabela relê quando aparece
    clicar_aba(&mut h, "clientes");
    esperar(&mut h, "a tabela relida", |h| {
        h.state().tabela_aberta().is_some_and(|t| t.info().colunas.iter().any(|c| c.nome == "apelido"))
    });
}

/// Trocar de aba com uma célula da grade em edição (pelo Ctrl+Tab ou pelo clique na faixa): o valor é gravado antes
/// de a aba Dados sair de vista.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da grade de verdade (tela_dados), que o substituto não tem")]
fn trocar_de_aba_com_celula_em_edicao_grava_antes() {
    let amb = Ambiente::exemplo("edicao_e_troca_de_aba");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    esperar_texto(&mut h, "Linhas 1–60 de 60");
    esperar(&mut h, "a grade", |h| h.query_by_label("Grade").is_some());
    for (i, trocar) in ["ctrl+tab", "clique na aba"].into_iter().enumerate() {
        let valor = format!("troca.{i}@example.test");
        // a coluna email (texto), pelo `th` dela, no meio da grade
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
        h.event(egui::Event::Text(valor.clone()));
        h.run_steps(2);
        match trocar {
            "ctrl+tab" => h.key_press_modifiers(CTRL, Key::Tab),
            _ => h.get(by().role(Role::Tab).label("exemplo.db · Visão geral")).click(),
        }
        h.run_steps(3);
        assert_eq!(rotulos_das_abas(&h).1, "exemplo.db · Visão geral", "{trocar}");
        assert!(estado::esperar_tarefas(std::time::Duration::from_secs(10)), "a gravação terminou");
        let sql = format!("SELECT count(*) FROM clientes WHERE email = '{valor}'");
        assert_eq!(ler_texto(&amb.caminho_banco(), &sql), "Inteiro(1)", "o valor foi gravado ({trocar})");
        clicar_aba(&mut h, "clientes");
        assentar(&mut h);
        h.remove_cursor();
        h.run_steps(2);
    }
}

#[test]
fn explorador_com_muitas_tabelas_filtra_e_o_filtro_nao_passa_para_outro_banco() {
    let amb = Ambiente::exemplo("muitas_tabelas");
    let longo = "t_150_tabela_de_nome_razoavelmente_comprido_para_o_explorador";
    let mut sql: String = (0..12).map(|i| format!("CREATE TABLE extra_{i:02} (a); ")).collect();
    sql.push_str(&format!("CREATE TABLE {longo} (a); INSERT INTO {longo} VALUES (1), (2), (3);"));
    let r = amb.servico.sql(&amb.estado.banco_id(), &sql, 10).expect("criar tabelas");
    assert!(!r.resultados.iter().any(|c| matches!(c, banco::ResultadoComando::Erro { .. })));
    let pequeno = amb.pasta().join("pequeno.db");
    criar_banco(&pequeno, "CREATE TABLE um (a); CREATE TABLE dois (a); CREATE TABLE tres (a);");

    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("extra_03", false));
    assentar(&mut h);
    assert!(h.query_by_label("Filtrar tabelas").is_some(), "o campo de filtro aparece acima de 12 tabelas");
    assert!(h.query(by().role(Role::Link).label(longo)).is_some());
    capturar(&mut h, "explorador-com-muitas-tabelas-claro.png");

    digitar(&mut h, "Filtrar tabelas", "_1");
    for (nome, visivel) in
        [("extra_10", true), ("extra_11", true), (longo, true), ("clientes", false), ("extra_03", false)]
    {
        assert_eq!(h.query(by().role(Role::Link).label(nome)).is_some(), visivel, "{nome}");
    }
    capturar(&mut h, "explorador-filtrado-claro.png");

    // sem filtro, com o Sistema aberto e rolado até o fim
    h.get_by_label("Filtrar tabelas").focus();
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::A);
    h.key_press(Key::Backspace);
    h.run_steps(3);
    assert!(h.query(by().role(Role::Link).label("clientes")).is_some());
    h.get(by().role(Role::Link).label("Sistema")).scroll_to_me();
    h.run_steps(3);
    clicar_link(&mut h, "Sistema");
    h.get(by().role(Role::Link).label("sqlite_sequence")).scroll_to_me();
    h.run_steps(5);
    capturar(&mut h, "explorador-rolado-ate-o-fim-claro.png");

    // outro banco, com 3 tabelas: sem campo e todas aparecem
    digitar(&mut h, "Filtrar tabelas", "zzz");
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none());
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    digitar(&mut h, CAMPO_DO_INICIO, &pequeno.display().to_string());
    clicar_botao(&mut h, "Abrir");
    esperar(&mut h, "abrir o pequeno", |h| h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "pequeno.db"));
    assentar(&mut h);
    assert!(h.query_by_label("Filtrar tabelas").is_none());
    for t in ["um", "dois", "tres"] {
        assert!(h.query(by().role(Role::Link).label(t)).is_some(), "{t}");
    }
}

#[test]
fn criar_indice_pela_janela_da_casca_rele_a_tabela() {
    let amb = Ambiente::exemplo("novo_indice");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    let tem_indice = |h: &H| {
        h.state().tabela_aberta().is_some_and(|t| t.info().indices.iter().any(|i| i.nome == "idx_clientes_cidade"))
    };
    assert!(h.state().tabela_aberta().is_some() && !tem_indice(&h));

    // fechar sem criar não relê nada
    h.state_mut().emitir(estado::Acao::AbrirNovoIndice { tabela: "clientes".into(), colunas: vec!["cidade".into()] });
    esperar(&mut h, "a janela abrir", |h| h.state().janela_aberta());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    assert!(!h.state().ocupado() && h.state().tabela_aberta().is_some());

    // criar: a janela fecha e a tabela aberta é relida (a dica de ordenação lenta da aba Dados some)
    h.state_mut().emitir(estado::Acao::AbrirNovoIndice { tabela: "clientes".into(), colunas: vec!["cidade".into()] });
    esperar(&mut h, "a janela abrir", |h| h.state().janela_aberta());
    // a janela de verdade lê as colunas da tabela e calcula a prévia (tarefas da janela, que o `ocupado()` da
    // casca não conta) antes de deixar criar. A prévia que chega aumenta o corpo: no quadro em que chega, o
    // rodapé desce (a janela só se recentra no quadro seguinte) e um clique no lugar de antes cai no corpo. Por
    // isso o clique espera a prévia chegar e a janela parar no lugar (um tempo fixo deixava a corrida no ar). O
    // substituto (sem o recurso `telas`) não tem prévia
    if cfg!(feature = "telas") {
        esperar_texto(&mut h, "CREATE INDEX");
    }
    esperar_parado(&mut h, Role::Button, "Criar índice");
    clicar_botao(&mut h, "Criar índice");
    for _ in 0..100 {
        if !h.state().janela_aberta() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(!h.state().janela_aberta(), "a janela fecha ao criar: {:?}", textos(&h));
    esperar(&mut h, "a tabela relida com o índice", tem_indice);
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));
}
