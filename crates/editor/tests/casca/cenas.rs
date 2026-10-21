//! As cenas das pranchetas (as capturas do fechamento da etapa 2).

use super::*;

/// Os recentes da prancheta do Início, criados agora: o exemplo.db no topo, depois o financeiro.sqlite e o
/// arquivo_antigo.db, que some do disco (a linha inativa com a pill "não encontrado").
fn recentes_da_prancheta(amb: &Ambiente) {
    let antigo = amb.pasta().join("arquivo_antigo.db");
    let financeiro = amb.pasta().join("financeiro.sqlite");
    criar_banco(&antigo, "CREATE TABLE t (id INTEGER PRIMARY KEY);");
    criar_banco(&financeiro, "CREATE TABLE lancamentos (id INTEGER PRIMARY KEY, valor REAL);");
    let exemplo = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo.db");
    for caminho in [&antigo, &financeiro, &exemplo] {
        amb.servico.prefs().adicionar_recente(caminho);
    }
    std::fs::remove_file(&antigo).expect("apagar o antigo");
}

/// O Início sem banco, com os recentes da prancheta.
fn cena_do_inicio(nome: &str, tema: Tema) -> (Ambiente, H) {
    let amb = Ambiente::vazio(nome);
    recentes_da_prancheta(&amb);
    let h = harness_com(&amb, tema, TAMANHO, Partida::default(), |_| None);
    assert_eq!(abas(&h), [("Início".to_string(), true)]);
    for nome in ["exemplo.db", "financeiro.sqlite", "arquivo_antigo.db"] {
        assert!(tem_texto(&h, nome), "{nome} nos recentes: {:?}", textos(&h));
    }
    assert!(tem_texto(&h, "não encontrado"));
    (amb, h)
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_do_inicio_claro() {
    let (_amb, mut h) = cena_do_inicio("cena_inicio_claro", Tema::Claro);
    capturar(&mut h, "casca-inicio-claro.png");
}

/// A prancheta 4: o Início escuro com o menu Arquivo e os Abertos recentemente abertos.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_do_inicio_escuro_com_o_menu_arquivo() {
    let (_amb, mut h) = cena_do_inicio("cena_inicio_escuro", Tema::Escuro);
    abrir_menu_arquivo(&mut h);
    abrir_submenu(&mut h, "Abertos recentemente");
    // os que existem, sem o que sumiu (o nome nos recentes do Início e no submenu)
    let botoes = |rotulo: &str| h.query_all(by().role(Role::Button).label(rotulo)).count();
    assert_eq!(botoes("financeiro.sqlite"), 2, "{:?}", textos(&h));
    assert_eq!(botoes("arquivo_antigo.db"), 0);
    capturar(&mut h, "casca-inicio-escuro.png");
}

/// A prancheta 1: a Visão geral escura com três abas abertas e o Exibir › Tema.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_da_visao_geral_com_tres_abas_e_o_tema() {
    let amb = Ambiente::exemplo("cena_visao_geral");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.run_steps(2);
    assentar(&mut h);
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assentar(&mut h);
    let ativa = "exemplo.db · Visão geral".to_string();
    assert_eq!(rotulos_das_abas(&h), (s(&["exemplo.db · Visão geral", "clientes", "Consulta 1"]), ativa));
    abrir_menu(&mut h, "Exibir");
    abrir_submenu(&mut h, "Tema");
    assert!(ligado(&h, Role::RadioButton, "Escuro"));
    capturar(&mut h, "casca-visao-geral-escuro.png");
}

/// A prancheta 2: a aba clientes escura, com o filtro "ativo igual a 1" e a vista Colunas nas Propriedades, entre a
/// Visão geral e uma Consulta.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_dos_dados_com_filtro_entre_as_abas() {
    let amb = Ambiente::exemplo("cena_dados");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    let filtro = estado::FiltroDigitado { coluna: "ativo".into(), op: banco::Operador::Igual, valor: "1".into() };
    let filtrada = Rota::objeto("clientes", false)
        .com_dados(estado::ConsultaDados { filtros: vec![filtro], ..Default::default() });
    h.state_mut().ir(filtrada.clone());
    assentar(&mut h);
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.run_steps(2);
    assentar(&mut h);
    clicar_aba(&mut h, "clientes");
    assentar(&mut h);
    vista_da_tabela(&mut h, 2);
    let (rotulos, ativa) = rotulos_das_abas(&h);
    assert_eq!(rotulos, s(&["exemplo.db · Visão geral", "clientes", "Consulta 1"]));
    assert_eq!(ativa, "clientes");
    assert_eq!(h.state().rota(), &filtrada, "o filtro ficou na aba");
    assert_eq!(h.state().vista_da_tabela(), Some("Colunas"));
    assert!(ligado(&h, Role::Link, "clientes"));
    assert!(existe(&h, Role::Label, "clientes · 60 linhas · 7 colunas"), "{:?}", textos(&h));
    capturar(&mut h, "casca-dados-abas-escuro.png");
    capturar_para_o_readme(&mut h, "dados-sqlite-escuro.png");
}

/// A prancheta 3: a Consulta 2 escura com o roteiro de três comandos (o último com erro) e o Histórico à esquerda.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_da_consulta_com_o_historico() {
    let amb = Ambiente::exemplo("cena_consulta");
    // comandos já executados neste banco, para o Histórico
    let id = amb.estado.banco_id();
    for sql in [
        "SELECT tipo, count(*) FROM eventos GROUP BY tipo",
        "PRAGMA integrity_check",
        "SELECT count(*) FROM eventos",
        "SELECT * FROM v_pedidos_por_cliente LIMIT 100",
    ] {
        amb.servico.sql(&id, sql, 10).expect(sql);
    }
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    executar(&mut h, "SELECT cidade, count(*) AS clientes FROM clientes GROUP BY cidade ORDER BY 2 DESC;");
    esperar_texto(&mut h, "Comando 1 · ");
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    executar(
        &mut h,
        "SELECT id, nome, email, cidade, limite FROM clientes ORDER BY id LIMIT 8;\n\
         UPDATE clientes SET cidade = cidade WHERE id <= 3;\n\
         SELECT * FROM tabela_que_nao_existe;",
    );
    esperar_texto(&mut h, "A execução parou neste comando");
    exibir(&mut h, "Histórico de consultas");
    assentar(&mut h);
    let (rotulos, ativa) = rotulos_das_abas(&h);
    assert_eq!(rotulos, s(&["exemplo.db · Visão geral", "clientes", "Consulta 1", "Consulta 2"]));
    assert_eq!(ativa, "Consulta 2");
    assert!(h.query(by().role(Role::Button).label("PRAGMA integrity_check")).is_some(), "{:?}", textos(&h));
    capturar(&mut h, "casca-consulta-2-escuro.png");
    capturar_para_o_readme(&mut h, "consulta-sql-escuro.png");
}

/// A prancheta 6: clientes clara, com a vista Colunas e a transação aberta (o aviso no documento e o modo na barra
/// de status).
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_da_estrutura_com_a_transacao_aberta() {
    let amb = Ambiente::exemplo("cena_transacao");
    let r = amb
        .servico
        .sql(&amb.estado.banco_id(), "BEGIN; UPDATE clientes SET cidade = 'Teresina' WHERE id = 1;", 10)
        .expect("begin");
    assert!(r.transacao_aberta);
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto_na_aba("clientes", false, Aba::Estrutura));
    assentar(&mut h);
    h.state_mut().emitir(estado::Acao::NovaConsulta { sql: None });
    h.run_steps(2);
    assentar(&mut h);
    clicar_aba(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(rotulos_das_abas(&h).1, "clientes");
    assert_eq!(h.state().vista_da_tabela(), Some("Colunas"));
    // a Consulta 1 viu a transação aberta por fora (o BEGIN veio pelo serviço): é a dela
    esperar_texto(&mut h, "Há uma transação aberta na Consulta 1.");
    assert!(existe(&h, Role::Label, "Transação aberta"));
    assert!(ligado(&h, Role::Link, "clientes"));
    // o banco no Explorador também avisa, com o ícone Atenção (a dica dele se soma ao modo da barra de status)
    assert!(existe(&h, Role::Image, "Transação aberta"));
    assert_eq!(dica_do_estado_do_banco(&mut h), frase_da_transacao("Consulta 1"), "a dica é o aviso inteiro");
    capturar(&mut h, "casca-transacao-status-claro.png");
}

/// A casca na janela mínima (960×600) com a aba de Dados: as Propriedades somem por falta de largura.
#[test]
#[cfg_attr(not(feature = "telas"), ignore = "as cenas das pranchetas precisam das telas de verdade")]
fn cena_da_janela_de_960() {
    let amb = Ambiente::exemplo("cena_960");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(960.0, 600.0), Partida::default(), |_| None);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(!h.state().propriedades_a_vista());
    assert_eq!(documento(&h).right(), 960.0 - 8.0);
    capturar(&mut h, "casca-960-claro.png");
}
