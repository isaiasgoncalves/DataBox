//! Leveza: com o aplicativo parado (sem mouse, teclado nem tarefa), nenhuma tela pede repintura — nem com as
//! animações normais ligadas (os outros testes desligam as animações), nem com várias abas abertas (as escondidas
//! não são desenhadas; as Consultas escondidas só tomam respostas prontas), nem com o Histórico de consultas à
//! vista, vazio ou com comandos, nem com o Explorador numa parte à vista, com as Visões abertas, recolhido, com o
//! campo de filtro (mais de 12 tabelas) e rolado até o fim, nem com as Propriedades de cada aba, escondidas, à vista
//! ou sumidas por falta de largura (a 960px, no tema escuro), nem com a transação aberta (o modo na barra de status,
//! o aviso no documento e a pill das Propriedades), nem com um menu e o submenu dele abertos (Arquivo › Abertos
//! recentemente, Exibir › Tema) ou com as janelas da Ajuda (Atalhos do teclado, Sobre o DataBox), nem depois de o
//! ponteiro passar pela faixa de abas, pelo Explorador, pelo Histórico, pela barra de ícones, pela de ferramentas,
//! pelas Propriedades, pela de status e pela barra de título (os botões da janela, um menu, o espaço livre e a borda
//! de redimensionar). Uma repintura pedida a cada quadro faria o app gastar CPU e bateria sem ninguém mexer nele.
//! Desde a etapa 4, também com a pasta aberta: o Explorador com a raiz, uma subpasta e uma pasta vazia abertas,
//! depois do ponteiro e com o menu de contexto aberto; a aba de um arquivo de dados; o Início com os recentes de cada
//! tipo e o vazio dos três cartões; e a releitura da pasta na volta do foco da janela, que não deixa quadro pendente.

use std::sync::Arc;
use std::time::{Duration, Instant};

use editor::{Editor, Partida, menu_aberto};
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Acao, Parte, Rota};
use interface::fontes;
use interface::tema::Tema;

type H = Harness<'static, Editor>;

const TAMANHO: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// Roda quadros até tudo assentar e devolve as causas de repintura vistas nos últimos 20 quadros.
fn causas_parado(h: &mut Harness<'static, Editor>) -> Vec<String> {
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    let mut vistas = Vec::new();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
        if h.ctx.has_requested_repaint() {
            vistas.extend(
                h.ctx
                    .repaint_causes()
                    .iter()
                    // o cursor que pisca num campo com foco (o editor da Consulta SQL) agenda uma repintura por
                    // piscada, como qualquer campo de texto; isso não é repintura contínua
                    .filter(|c| !c.file.replace('\\', "/").ends_with("text_selection/visuals.rs"))
                    .map(|c| format!("{}:{} {}", c.file, c.line, c.reason)),
            );
        }
    }
    vistas
}

/// Confere que a tela parada não pede repintura; `nome` diz qual tela é, na mensagem.
fn conferir_parado(h: &mut H, nome: &str) {
    let causas = causas_parado(h);
    assert!(causas.is_empty(), "{nome}, parado, pediu repintura: {causas:#?}");
}

/// Um ambiente com uma cópia do banco de exemplo (ainda fechado: o editor o abre pela linha de comando).
fn ambiente_com_exemplo(nome: &str) -> Ambiente {
    let amb = Ambiente::vazio(nome);
    amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo.db");
    amb
}

/// O editor com o banco de exemplo do ambiente aberto (pela linha de comando), com as animações normais, neste
/// tema e neste tamanho de janela, já carregado.
fn editor_parado(amb: &Ambiente, tema: Tema, tamanho: egui::Vec2) -> H {
    let partida = Partida { caminho: Some(amb.pasta().join("exemplo.db")), somente_leitura: false };
    let editor = Editor::novo(Arc::clone(&amb.servico), tema, partida);
    let mut h = Harness::builder()
        .with_size(tamanho)
        .with_pixels_per_point(1.0)
        .with_max_steps(64)
        .wgpu()
        .build_ui_state(|ui, e: &mut Editor| e.desenhar(ui, None), editor);
    esperar(&mut h, "o banco abrir", |h| {
        fontes::do_sistema_carregadas(&h.ctx) && h.state().carregado() && !h.state().ocupado()
    });
    h
}

/// O editor com o banco de exemplo aberto (pela linha de comando), com as animações normais, já carregado.
fn harness_parado(nome: &str) -> (Ambiente, H) {
    let amb = ambiente_com_exemplo(nome);
    let h = editor_parado(&amb, Tema::Claro, TAMANHO);
    (amb, h)
}

/// Roda quadros até `condicao` valer (as tarefas terminam em outras threads); falha em 15 s.
fn esperar(h: &mut H, o_que: &str, mut condicao: impl FnMut(&H) -> bool) {
    let inicio = Instant::now();
    while !condicao(h) {
        assert!(inicio.elapsed() < Duration::from_secs(15), "esperei 15 s por {o_que}");
        std::thread::sleep(Duration::from_millis(10));
        h.step();
    }
    h.run_steps(2);
}

/// Clica no primeiro nó com este papel e rótulo (os menus e as barras vêm antes dos painéis na árvore) e tira o
/// ponteiro.
fn clicar(h: &mut H, papel: Role, rotulo: &str) {
    h.query_all(by().role(papel).label(rotulo)).next().unwrap_or_else(|| panic!("nenhum {papel:?} {rotulo:?}")).click();
    h.run_steps(2);
    h.remove_cursor();
    h.run_steps(1);
}

/// Passa o ponteiro sobre cada nó (alguns quadros em cada um, para as dicas e os halos) e tira o ponteiro no fim.
fn passar_e_sair(h: &mut H, nos: &[(Role, &str)]) {
    for (papel, rotulo) in nos {
        h.query_all(by().role(*papel).label(rotulo))
            .next()
            .unwrap_or_else(|| panic!("nenhum {papel:?} {rotulo:?}"))
            .hover();
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(20));
            h.step();
        }
    }
    h.remove_cursor();
    h.run_steps(1);
}

/// Abre um menu da barra de título e, se vier, o submenu dele (o ponteiro fica sobre o item do submenu, como a mão
/// de quem o abriu).
fn abrir_menu(h: &mut H, menu: &str, submenu: Option<&str>) {
    h.get(by().role(Role::Button).label(menu)).click();
    h.run_steps(2);
    assert!(menu_aberto(&h.ctx), "o menu {menu} abriu");
    if let Some(item) = submenu {
        h.get(by().role(Role::Button).label(item)).hover();
        h.run_steps(3);
    }
}

/// Esc fecha o menu ou a janela aberta.
fn esc(h: &mut H) {
    h.key_press(Key::Escape);
    h.run_steps(3);
}

/// Um passo do teste: abrir ou focar a aba de uma rota, uma ação da casca ou o clique num botão (o ponteiro sai
/// depois).
enum Passo {
    Ir(Rota),
    Emitir(Acao),
    Clicar(&'static str),
}

#[test]
fn parado_nao_pede_repintura() {
    let (_amb, mut h) = harness_parado("parado_nao_repinta");
    let passos = [
        ("visão geral", None),
        ("dados de clientes", Some(Passo::Ir(Rota::objeto("clientes", false)))),
        ("estrutura de clientes", Some(Passo::Ir(Rota::objeto_na_aba("clientes", false, estado::Parte::Estrutura)))),
        ("consulta SQL", Some(Passo::Ir(Rota::consulta_com("SELECT 1")))),
        ("segunda consulta SQL", Some(Passo::Emitir(Acao::NovaConsulta { sql: Some("SELECT 2".into()) }))),
        ("início", Some(Passo::Ir(Rota::Inicio))),
        ("visão geral com várias abas abertas", Some(Passo::Ir(Rota::Banco))),
        ("dados de clientes de novo", Some(Passo::Ir(Rota::objeto("clientes", false)))),
        ("histórico de consultas à vista", Some(Passo::Clicar("Histórico de consultas"))),
        ("sem o painel da esquerda", Some(Passo::Clicar("Histórico de consultas"))),
        ("explorador de volta", Some(Passo::Clicar("Explorador"))),
        ("propriedades escondidas", Some(Passo::Clicar("Painel de propriedades"))),
        ("propriedades à vista", Some(Passo::Clicar("Painel de propriedades"))),
    ];
    for (nome, passo) in passos {
        match passo {
            Some(Passo::Ir(r)) => h.state_mut().ir(r),
            Some(Passo::Emitir(a)) => h.state_mut().emitir(a),
            Some(Passo::Clicar(rotulo)) => {
                h.get(by().role(Role::Button).label(rotulo)).click();
                h.step();
                h.remove_cursor();
            }
            None => {}
        }
        let causas = causas_parado(&mut h);
        assert!(causas.is_empty(), "a tela {nome} parada pediu repintura: {causas:#?}");
    }
    assert_eq!(h.state().abas_abertas().len(), 6);
}

/// N3: a Consulta executando conta o tempo no rodapé, que pede um quadro a cada 250 ms, e o arco do Executar ocupado
/// gira pedindo um a cada ~33 ms: o ritmo real da tela executando é o do arco (~30 quadros por segundo), bem mais
/// frequente que o do rodapé, e só então; com o Interromper, a resposta chega e o app volta a ficar parado, sem
/// repintura nenhuma.
#[test]
#[cfg(feature = "telas")]
fn executando_repinta_no_ritmo_do_arco_e_para_no_fim() {
    let (_amb, mut h) = harness_parado("executando_repinta");
    let infinita = "WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 2000000000) \
                    SELECT count(*) AS n FROM s;";
    h.state_mut().ir(Rota::consulta_com(infinita));
    assert!(causas_parado(&mut h).is_empty(), "a Consulta parada, antes de executar");
    let executando = |h: &H| h.query_all(by().label_contains("Executando · ")).next().is_some();
    h.get(by().role(Role::Button).label("Executar")).click();
    h.remove_cursor();
    esperar(&mut h, "o rodapé executando", |h| executando(h));
    // durante a execução há repintura pedida, mas não a cada quadro sem motivo: depois de assentar, só as duas causas
    // dos dois relógios da tela
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    let mut causas = std::collections::BTreeSet::new();
    let (mut com_repintura, mut quadros_do_arco) = (0, 0);
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
        if h.ctx.has_requested_repaint() {
            let neste: Vec<String> = h
                .ctx
                .repaint_causes()
                .iter()
                .map(|c| format!("{}:{} {}", c.file.replace('\\', "/"), c.line, c.reason))
                .collect();
            com_repintura += 1;
            quadros_do_arco += usize::from(neste.iter().any(|c| c.contains("pecas/botao.rs")));
            causas.extend(neste);
        }
    }
    // só duas causas, as dos dois relógios da tela: o rodapé (250 ms) e o arco do Executar ocupado (33 ms)
    let arquivos: Vec<String> = causas.iter().map(|c| c.replace('\\', "/")).collect();
    assert!(arquivos.iter().any(|c| c.starts_with("crates/tela_consulta/src/lib.rs")), "o rodapé: {causas:#?}");
    assert!(arquivos.iter().any(|c| c.starts_with("crates/interface/src/pecas/botao.rs")), "o arco: {causas:#?}");
    assert!(
        arquivos.iter().all(|c| c.starts_with("crates/tela_consulta/src/lib.rs")
            || c.starts_with("crates/interface/src/pecas/botao.rs")),
        "{causas:#?}"
    );
    // o ritmo real: o arco renova o pedido de 33 ms em todo quadro desenhado (o do rodapé, de 250 ms, perde para
    // ele), então a tela executando repinta a ~30 quadros por segundo, não a 4
    assert!(
        com_repintura >= 50 && quadros_do_arco == com_repintura,
        "o arco em {quadros_do_arco} de {com_repintura} quadros com repintura pedida"
    );
    // o Interromper encerra, e o app para
    h.get(by().role(Role::Button).label("Interromper")).click();
    h.remove_cursor();
    esperar(&mut h, "a execução terminar", |h| !executando(h));
    assert!(h.query_all(by().label_contains("A consulta foi interrompida.")).next().is_some());
    conferir_parado(&mut h, "a Consulta interrompida");
}

#[test]
fn faixa_de_abas_parada_nao_repinta() {
    let (_amb, mut h) = harness_parado("faixa_parada_nao_repinta");
    h.state_mut().ir(Rota::objeto("clientes", false));
    h.state_mut().emitir(Acao::NovaConsulta { sql: None });
    let causas = causas_parado(&mut h);
    assert!(causas.is_empty(), "com três abas: {causas:#?}");
    // o ponteiro passa sobre uma aba, sobre o × de outra, sobre o "+" ("Nova consulta SQL"; o "Nova consulta" é o da
    // barra de ferramentas), sobre um nó do Explorador, um botão da barra de ícones, o modo na barra de status (que
    // tem dica), os botões da barra de ferramentas (um ligado, um desligado com dica, o Painel) e uma seção das
    // Propriedades, e sai
    for (papel, rotulo, ultimo) in [
        (Role::Tab, "clientes · Dados", false),
        (Role::Button, "Fechar a aba exemplo.db · Visão geral", false),
        (Role::Button, "Nova consulta SQL", true),
        (Role::Link, "pedidos", false),
        (Role::Button, "Histórico de consultas", false),
        (Role::Label, "Gravação imediata", false),
        (Role::Button, "Nova consulta", false),
        (Role::Button, "Nova coluna", false),
        (Role::Button, "Painel de propriedades", false),
        (Role::Button, "Seção Execução", false),
    ] {
        {
            let mut nos = h.query_all(by().role(papel).label(rotulo));
            let no = if ultimo { nos.last() } else { nos.next() };
            no.unwrap_or_else(|| panic!("{rotulo}")).hover();
        }
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(20));
            h.step();
        }
    }
    h.remove_cursor();
    let causas = causas_parado(&mut h);
    assert!(causas.is_empty(), "a faixa parada depois do hover pediu repintura: {causas:#?}");
}

#[test]
fn barra_de_titulo_parada_nao_repinta() {
    let (_amb, mut h) = harness_parado("barra_titulo_parada_nao_repinta");
    let causas = causas_parado(&mut h);
    assert!(causas.is_empty(), "a barra de título parada: {causas:#?}");
    // o ponteiro passa sobre os botões da janela, sobre um menu, pelo espaço livre (com o título) e pela borda de
    // redimensionar, e sai
    let mut pontos: Vec<egui::Pos2> = ["Fechar a janela", "Maximizar", "Minimizar", "Exibir"]
        .iter()
        .map(|rotulo| h.get(by().role(Role::Button).label(rotulo)).rect().center())
        .collect();
    pontos.extend([egui::pos2(600.0, 15.0), egui::pos2(2.0, 450.0), egui::pos2(1438.0, 898.0)]);
    for p in pontos {
        h.hover_at(p);
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(20));
            h.step();
        }
    }
    h.remove_cursor();
    let causas = causas_parado(&mut h);
    assert!(causas.is_empty(), "a barra de título parada depois do hover pediu repintura: {causas:#?}");
}

#[test]
fn divisorias_dos_paineis_paradas_nao_repintam() {
    let (_amb, mut h) = harness_parado("divisorias_paradas");
    conferir_parado(&mut h, "as divisórias paradas");
    // o ponteiro passa pelas duas divisórias (o fio aparece e some, com a transição normal) e sai
    for rotulo in ["Largura do Explorador", "Largura das Propriedades"] {
        h.get(by().role(Role::Splitter).label(rotulo)).hover();
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(20));
            h.step();
        }
    }
    h.remove_cursor();
    conferir_parado(&mut h, "as divisórias depois do ponteiro");
    // arrastar o Explorador e soltar: depois do gesto (e da gravação das preferências), tudo parado outra vez
    let de = h.get(by().role(Role::Splitter).label("Largura do Explorador")).rect().center();
    let botao = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(de);
    h.run_steps(2);
    h.event(botao(de, true));
    h.run_steps(1);
    for dx in [10.0, 20.0, 30.0] {
        h.hover_at(de + egui::vec2(dx, 0.0));
        h.run_steps(2);
    }
    h.event(botao(de + egui::vec2(30.0, 0.0), false));
    h.run_steps(2);
    h.remove_cursor();
    esperar(&mut h, "as preferências gravarem", |h| !h.state().ocupado());
    conferir_parado(&mut h, "as divisórias depois de arrastar");
    // o foco do teclado na divisória (o fio firme) e um passo com a seta
    h.get(by().role(Role::Splitter).label("Largura das Propriedades")).focus();
    h.key_press(Key::ArrowLeft);
    h.run_steps(2);
    esperar(&mut h, "as preferências gravarem", |h| !h.state().ocupado());
    conferir_parado(&mut h, "a divisória com o foco do teclado");
}

#[test]
fn paineis_parados_nao_repintam() {
    let (amb, mut h) = harness_parado("paineis_parados");
    let id = h.state().estado().banco_id();
    for sql in ["SELECT 1 AS um", "SELECT count(*) FROM clientes"] {
        amb.servico.sql(&id, sql, 10).expect(sql);
    }

    // o Histórico de consultas com dois comandos, e o ponteiro sobre um deles
    clicar(&mut h, Role::Button, "Histórico de consultas");
    esperar(&mut h, "a lista do Histórico", |h| h.query(by().role(Role::Button).label("SELECT 1 AS um")).is_some());
    conferir_parado(&mut h, "o Histórico com comandos");
    passar_e_sair(&mut h, &[(Role::Button, "SELECT 1 AS um"), (Role::Button, "SELECT count(*) FROM clientes")]);
    conferir_parado(&mut h, "o Histórico depois do ponteiro");

    // o Explorador com a Estrutura de clientes à vista (a tabela aberta e a parte marcada) e as Visões abertas; o
    // ponteiro numa parte, numa visão e no Recolher tudo; depois, recolhido
    clicar(&mut h, Role::Button, "Explorador");
    h.state_mut().ir(Rota::objeto_na_aba("clientes", false, Parte::Estrutura));
    esperar(&mut h, "a Estrutura de clientes", |h| {
        !h.state().ocupado() && h.query(by().role(Role::Link).label("clientes · Estrutura")).is_some()
    });
    clicar(&mut h, Role::Link, "Visões");
    conferir_parado(&mut h, "o Explorador com uma parte à vista e as Visões abertas");
    passar_e_sair(
        &mut h,
        &[
            (Role::Link, "clientes · Índices e gatilhos"),
            (Role::Link, "v_pedidos_por_cliente"),
            (Role::Button, "Recolher tudo"),
        ],
    );
    conferir_parado(&mut h, "o Explorador depois do ponteiro");
    clicar(&mut h, Role::Button, "Recolher tudo");
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none(), "a árvore recolhida");
    conferir_parado(&mut h, "o Explorador recolhido");

    // a transação aberta: o modo na barra de status, o aviso no alto do documento, as Propriedades da Consulta com
    // a pill "aberta" e as da Visão geral
    amb.servico.sql(&id, "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(Acao::NotarTransacao(true));
    esperar(&mut h, "a transação no estado", |h| h.state().estado().transacao_aberta() && !h.state().ocupado());
    assert!(h.query(by().role(Role::Label).label("Transação aberta")).is_some(), "o modo na barra de status");
    conferir_parado(&mut h, "a Estrutura com a transação aberta");
    h.state_mut().ir(Rota::Consulta { sql: None });
    esperar(&mut h, "a Consulta", |h| !h.state().ocupado());
    conferir_parado(&mut h, "a Consulta com a transação aberta");
    h.state_mut().ir(Rota::Banco);
    esperar(&mut h, "a Visão geral", |h| !h.state().ocupado());
    conferir_parado(&mut h, "a Visão geral com a transação aberta");
    // (com as telas, a Consulta 1 abriu uma aba e viu a transação: o aviso cita ela; o ícone Atenção do Explorador
    // tem dica)
    let ir = if cfg!(feature = "telas") { "Ir para a Consulta 1" } else { "Ir para a Consulta SQL" };
    passar_e_sair(&mut h, &[(Role::Label, "Transação aberta"), (Role::Image, "Transação aberta"), (Role::Link, ir)]);
    conferir_parado(&mut h, "a transação aberta depois do ponteiro");
    amb.servico.sql(&id, "ROLLBACK;", 10).expect("rollback");
}

/// C2: a leitura dos objetos que falha não deixa o Explorador em "Carregando…" para sempre (o ponto pulsante pede
/// ~30 quadros por segundo): fica o motivo e o "Tentar de novo", parado; o botão refaz a leitura.
#[test]
fn explorador_com_a_leitura_dos_objetos_falhada_nao_gira_nem_repinta() {
    let (_amb, mut h) = harness_parado("explorador_falhou");
    let ctx = h.ctx.clone();
    h.state_mut().simular_falha_dos_objetos(&ctx, "O disco não respondeu.");
    h.run_steps(3);
    assert!(h.query_by_label("Não foi possível ler as tabelas.").is_some(), "a frase do erro");
    assert!(h.query_by_label("O disco não respondeu.").is_some(), "o motivo");
    assert!(h.query_by_label("Carregando…").is_none(), "sem o ponto pulsante");
    conferir_parado(&mut h, "o Explorador com a leitura dos objetos falhada");
    clicar(&mut h, Role::Button, "Tentar de novo");
    esperar(&mut h, "a lista voltar", |h| h.query(by().role(Role::Link).label("clientes")).is_some());
    assert!(h.query_by_label("Não foi possível ler as tabelas.").is_none());
    conferir_parado(&mut h, "o Explorador depois de tentar de novo");
}

/// C3: o erro da leitura do Histórico (sem lista) fica sozinho: sem o "Carregando…" eterno do lado do aviso.
#[test]
fn historico_com_a_leitura_falhada_mostra_so_o_aviso_e_nao_repinta() {
    let (_amb, mut h) = harness_parado("historico_falhou");
    clicar(&mut h, Role::Button, "Histórico de consultas");
    esperar(&mut h, "a leitura do Histórico", |h| !h.state().ocupado());
    h.state_mut().simular_falha_do_historico("Não foi possível ler o histórico.");
    h.run_steps(3);
    assert!(h.query(by().label_contains("Não foi possível ler o histórico.")).is_some(), "o aviso");
    assert!(h.query_by_label("Carregando…").is_none(), "sem o ponto pulsante");
    conferir_parado(&mut h, "o Histórico com a leitura falhada");
}

#[test]
fn janela_estreita_escura_e_explorador_com_filtro_parados_nao_repintam() {
    let amb = ambiente_com_exemplo("estreita_parada");
    let mut h = editor_parado(&amb, Tema::Escuro, egui::vec2(960.0, 600.0));
    // a 960px as Propriedades somem por falta de largura, e o botão delas fica desligado, com a dica de alargar
    assert!(!h.state().propriedades_a_vista());
    conferir_parado(&mut h, "a Visão geral escura a 960px");
    passar_e_sair(&mut h, &[(Role::Button, "Painel de propriedades"), (Role::Link, "clientes")]);
    conferir_parado(&mut h, "a janela estreita depois do ponteiro");
    h.state_mut().ir(Rota::objeto("clientes", false));
    esperar(&mut h, "a aba de Dados", |h| !h.state().ocupado());
    conferir_parado(&mut h, "a aba de Dados escura a 960px");

    // mais de 12 tabelas: o campo "Filtrar tabelas" no Explorador, com um filtro digitado (o cursor que pisca no
    // campo não conta) e, sem ele, a árvore com o Sistema aberto e rolada até o fim
    let id = h.state().estado().banco_id();
    let sql: String = (0..12).map(|i| format!("CREATE TABLE extra_{i:02} (a); ")).collect();
    amb.servico.sql(&id, &sql, 10).expect("criar tabelas");
    h.state_mut().emitir(Acao::RecarregarObjetos);
    esperar(&mut h, "o campo de filtro", |h| h.query_by_label("Filtrar tabelas").is_some() && !h.state().ocupado());
    conferir_parado(&mut h, "o Explorador com o campo de filtro");
    let campo = h.get_by_label("Filtrar tabelas");
    campo.focus();
    campo.type_text("_1");
    h.run_steps(2);
    assert!(h.query(by().role(Role::Link).label("extra_10")).is_some());
    assert!(h.query(by().role(Role::Link).label("clientes")).is_none(), "o filtro vale");
    conferir_parado(&mut h, "o Explorador filtrado");
    h.key_press(Key::Backspace);
    h.key_press(Key::Backspace);
    h.run_steps(2);
    h.get(by().role(Role::Link).label("Sistema")).scroll_to_me();
    h.run_steps(3);
    clicar(&mut h, Role::Link, "Sistema");
    h.get(by().role(Role::Link).label("sqlite_sequence")).scroll_to_me();
    h.run_steps(5);
    conferir_parado(&mut h, "o Explorador rolado até o fim");
}

#[test]
fn menus_e_janelas_da_ajuda_parados_nao_repintam() {
    let amb = ambiente_com_exemplo("menus_parados");
    // um recente além do aberto, para o submenu "Abertos recentemente" ter um item
    let outro = amostras::criar_banco_exemplo(amb.pasta().join("outro.db")).expect("outro.db");
    amb.servico.prefs().adicionar_recente(&outro);
    let mut h = editor_parado(&amb, Tema::Claro, TAMANHO);

    abrir_menu(&mut h, "Arquivo", Some("Abertos recentemente"));
    assert!(h.query(by().role(Role::Button).label("outro.db")).is_some(), "o submenu com o outro banco");
    conferir_parado(&mut h, "o menu Arquivo com os Abertos recentemente");
    assert!(h.query(by().role(Role::Button).label("outro.db")).is_some(), "o submenu continua aberto");
    esc(&mut h);
    assert!(!menu_aberto(&h.ctx));

    abrir_menu(&mut h, "Exibir", Some("Tema"));
    assert!(h.query(by().role(Role::RadioButton).label("Seguir o Windows")).is_some(), "o submenu do Tema");
    conferir_parado(&mut h, "o menu Exibir com o Tema");
    assert!(h.query(by().role(Role::RadioButton).label("Seguir o Windows")).is_some(), "o submenu continua aberto");
    esc(&mut h);

    abrir_menu(&mut h, "Editar", None);
    conferir_parado(&mut h, "o menu Editar");
    assert!(menu_aberto(&h.ctx), "o menu continua aberto");
    esc(&mut h);

    for janela in ["Atalhos do teclado", "Sobre o DataBox"] {
        abrir_menu(&mut h, "Ajuda", None);
        clicar(&mut h, Role::Button, janela);
        assert!(h.state().janela_aberta(), "a janela {janela}");
        conferir_parado(&mut h, &format!("a janela {janela}"));
        esc(&mut h);
        assert!(!h.state().janela_aberta(), "Esc fecha a janela {janela}");
        conferir_parado(&mut h, &format!("a casca depois da janela {janela}"));
    }
}

// ---------------------------------------------------------------------------------------------- a pasta (etapa 4)

/// A raiz da pasta aberta já foi lida.
fn raiz_lida(h: &H) -> bool {
    h.state().estado().pasta.as_ref().is_some_and(|p| p.listagem(&p.chave).is_some())
}

/// O caminho como o serviço guarda o da pasta aberta: absoluto, com os nomes curtos (8.3) resolvidos (a pasta
/// temporária vem como `C:\Users\CONCEI~1\…`), sem o `\\?\`.
fn real(caminho: &std::path::Path) -> std::path::PathBuf {
    let c = std::fs::canonicalize(caminho).expect("canonicalize");
    let texto = c.to_string_lossy().into_owned();
    std::path::PathBuf::from(texto.strip_prefix(r"\\?\").unwrap_or(&texto))
}

/// A pasta desta chave já foi lida (está pronta no estado).
fn pasta_lida(h: &H, caminho: &std::path::Path) -> bool {
    let chave = prefs::chave_caminho(caminho);
    h.state().estado().pasta.as_ref().is_some_and(|p| p.listagem(&chave).is_some())
}

/// O editor com a pasta de dados do `Ambiente::com_pasta` aberta pela linha de comando (como `databox.exe C:\dados`)
/// e, com `banco`, o `exemplo.db` de dentro dela aberto, com as animações normais, neste tema; a raiz já lida.
fn editor_da_pasta(nome: &str, tema: Tema, banco: bool) -> (Ambiente, H) {
    let amb = Ambiente::com_pasta(nome, &[]);
    let partida = Partida { caminho: Some(amb.pasta_de_dados()), somente_leitura: false };
    let editor = Editor::novo(Arc::clone(&amb.servico), tema, partida);
    let tema_egui = if tema == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
    let mut h = Harness::builder()
        .with_size(TAMANHO)
        .with_pixels_per_point(1.0)
        .with_theme(tema_egui)
        .with_max_steps(64)
        .wgpu()
        .build_ui_state(|ui, e: &mut Editor| e.desenhar(ui, None), editor);
    esperar(&mut h, "a pasta abrir", |h| {
        fontes::do_sistema_carregadas(&h.ctx) && h.state().carregado() && !h.state().ocupado() && raiz_lida(h)
    });
    if banco {
        let caminho = amb.pasta_de_dados().join("exemplo.db");
        h.state_mut().emitir(Acao::AbrirBanco { caminho, somente_leitura: false });
        esperar(&mut h, "o banco abrir", |h| h.state().estado().tem_banco() && !h.state().ocupado());
    }
    (amb, h)
}

/// A janela perde e recupera o foco (o Alt+Tab de ida e volta): a volta relê a pasta aberta.
fn alternar_o_foco(h: &mut H) {
    h.input_mut().focused = false;
    h.step();
    h.input_mut().focused = true;
    h.step();
}

/// Etapa 4: o Explorador com a pasta aberta (a raiz, o banco aberto no lugar dele, os arquivos de dados), com uma
/// subpasta e uma pasta vazia abertas (a leitura pendente gira o ponto do "Carregando…"; pronta, para), depois do
/// ponteiro pelos nós e com o menu de contexto aberto: tudo parado, sem repintura.
#[test]
fn explorador_com_a_pasta_parado_nao_repinta() {
    let (amb, mut h) = editor_da_pasta("explorador_pasta_parado", Tema::Claro, true);
    assert!(h.query(by().role(Role::Link).label("dados, pasta")).is_some(), "a raiz da pasta");
    conferir_parado(&mut h, "a pasta com o banco aberto no lugar");

    // a subpasta e, dentro dela, a vazia: a leitura de cada uma termina e a árvore para
    let sub = real(&amb.pasta_de_dados()).join("sub");
    clicar(&mut h, Role::Link, "sub, pasta");
    esperar(&mut h, "a subpasta", |h| pasta_lida(h, &sub) && !h.state().ocupado());
    assert!(h.query(by().role(Role::Link).label("interno.csv")).is_some(), "o arquivo da subpasta");
    conferir_parado(&mut h, "a subpasta aberta");
    clicar(&mut h, Role::Link, "vazia, pasta");
    esperar(&mut h, "a pasta vazia", |h| pasta_lida(h, &sub.join("vazia")) && !h.state().ocupado());
    assert!(h.query_by_label("Nenhum arquivo que o DataBox abra.").is_some(), "o vazio da pasta");
    conferir_parado(&mut h, "a pasta vazia aberta");

    // o ponteiro pela raiz (com a dica das ocultas), por uma pasta, por um banco fechado e por um arquivo de dados
    passar_e_sair(
        &mut h,
        &[
            (Role::Link, "dados, pasta"),
            (Role::Link, "sub, pasta"),
            (Role::Link, "estoque_2026.db"),
            (Role::Link, "clientes.csv"),
        ],
    );
    conferir_parado(&mut h, "o Explorador com a pasta depois do ponteiro");

    // o menu de contexto aberto (o vidro fosco, sem o ponteiro em cima) não repinta, e continua aberto
    h.get(by().role(Role::Link).label("clientes.csv")).click_secondary();
    h.run_steps(3);
    h.remove_cursor();
    assert!(h.query(by().role(Role::Menu).label("clientes.csv")).is_some(), "o menu de contexto abriu");
    conferir_parado(&mut h, "o menu de contexto aberto");
    assert!(h.query(by().role(Role::Menu).label("clientes.csv")).is_some(), "o menu continua aberto");
    esc(&mut h);
    assert!(h.query(by().role(Role::Menu).label("clientes.csv")).is_none(), "Esc fecha o menu");
    conferir_parado(&mut h, "o Explorador depois do menu de contexto");
}

/// Etapa 4: a aba de um arquivo de dados (a ficha) e o Início com os recentes de cada tipo, no tema escuro; e o
/// Explorador vazio (sem pasta e sem banco) com o Início dos três cartões: parados, sem repintura.
#[test]
fn ficha_do_arquivo_e_inicio_com_a_pasta_parados_nao_repintam() {
    let (_amb, mut h) = editor_da_pasta("ficha_parada", Tema::Escuro, true);
    // o `planilha.xlsx` da pasta de teste não é um pacote de verdade: a planilha (etapa 6) mostra o erro com os botões
    // da ficha (sem o recurso `telas`, a própria ficha)
    clicar(&mut h, Role::Link, "planilha.xlsx");
    esperar(&mut h, "a ficha do arquivo", |h| matches!(h.state().rota(), Rota::Arquivo { .. }) && !h.state().ocupado());
    conferir_parado(&mut h, "a ficha do arquivo");
    passar_e_sair(&mut h, &[(Role::Button, "Abrir no programa padrão"), (Role::Button, "Mostrar na pasta")]);
    conferir_parado(&mut h, "a ficha do arquivo depois do ponteiro");
    // a tela de CSV aberta e parada
    #[cfg(feature = "telas")]
    {
        clicar(&mut h, Role::Link, "clientes.csv");
        let pronto = |h: &H| h.query_by_label("2 linhas · 2 colunas").is_some() && !h.state().ocupado();
        esperar(&mut h, "o CSV na grade", pronto);
        conferir_parado(&mut h, "a tela de CSV");
    }
    // o Início com a pasta, o banco e o arquivo nos recentes
    h.state_mut().ir(Rota::Inicio);
    esperar(&mut h, "o Início", |h| !h.state().ocupado());
    conferir_parado(&mut h, "o Início com os recentes de cada tipo");

    // sem nada aberto: o Explorador vazio ("Nenhuma pasta aberta.") e os cartões de ação do Início
    let amb = Ambiente::vazio("inicio_vazio_parado");
    let editor = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
    let mut h = Harness::builder()
        .with_size(TAMANHO)
        .with_pixels_per_point(1.0)
        .with_max_steps(64)
        .wgpu()
        .build_ui_state(|ui, e: &mut Editor| e.desenhar(ui, None), editor);
    esperar(&mut h, "o Início", |h| {
        fontes::do_sistema_carregadas(&h.ctx) && h.state().carregado() && !h.state().ocupado()
    });
    assert!(h.query_by_label("Nenhuma pasta aberta.").is_some(), "o Explorador vazio");
    conferir_parado(&mut h, "o Explorador vazio e o Início");
    let cartoes: Vec<egui::Pos2> =
        h.query_all(by().role(Role::Button).label("Abrir pasta")).map(|n| n.rect().center()).collect();
    assert!(!cartoes.is_empty(), "o cartão e o botão Abrir pasta");
    for p in cartoes {
        h.hover_at(p);
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(20));
            h.step();
        }
    }
    h.remove_cursor();
    h.run_steps(1);
    conferir_parado(&mut h, "o Início depois do ponteiro nos cartões");
}

/// Etapa 4, decisão 6: a volta do foco da janela relê a pasta aberta (a raiz e a subpasta aberta), mas a leitura que
/// não achou nada novo não troca o estado (a mesma listagem fica, o mesmo `Arc`), e o app volta a ficar parado: a
/// releitura não vira repintura contínua. A segunda volta logo em seguida (antes dos 2 s) não relê nada.
#[test]
fn volta_do_foco_rele_a_pasta_e_nao_deixa_repintura() {
    let (amb, mut h) = editor_da_pasta("foco_parado", Tema::Claro, false);
    let sub = real(&amb.pasta_de_dados()).join("sub");
    clicar(&mut h, Role::Link, "sub, pasta");
    esperar(&mut h, "a subpasta", |h| pasta_lida(h, &sub) && !h.state().ocupado());
    conferir_parado(&mut h, "a pasta com a subpasta aberta");
    let listagens = |h: &H| {
        let p = h.state().estado().pasta.clone().expect("a pasta aberta");
        let raiz = match p.nos.get(&p.chave) {
            Some(estado::NoDaPasta::Pronto(l)) => Arc::clone(l),
            outro => panic!("a raiz: {outro:?}"),
        };
        let sub = match p.nos.get(&prefs::chave_caminho(&sub)) {
            Some(estado::NoDaPasta::Pronto(l)) => Arc::clone(l),
            outro => panic!("a subpasta: {outro:?}"),
        };
        (raiz, sub)
    };
    let (raiz_antes, sub_antes) = listagens(&h);
    let antes = h.state().leituras_de_pasta();

    alternar_o_foco(&mut h);
    assert_eq!(h.state().leituras_de_pasta(), antes + 2, "a raiz e a subpasta aberta relidas");
    // a segunda volta logo depois: a leitura ainda corre ou não passaram os 2 s
    alternar_o_foco(&mut h);
    assert_eq!(h.state().leituras_de_pasta(), antes + 2, "nada relido antes dos 2 s");
    esperar(&mut h, "as releituras terminarem", |h| !h.state().ocupado());
    let (raiz_depois, sub_depois) = listagens(&h);
    assert!(Arc::ptr_eq(&raiz_antes, &raiz_depois), "a raiz igual não troca o estado");
    assert!(Arc::ptr_eq(&sub_antes, &sub_depois), "a subpasta igual não troca o estado");
    conferir_parado(&mut h, "a pasta depois da volta do foco");
    assert_eq!(h.state().leituras_de_pasta(), antes + 2, "parado, a pasta não é relida sozinha");
}

// ------------------------------------------------------------------------------- os editores (etapas 5 e 6)

/// O editor sem banco nem pasta, com as animações normais, neste tema, já carregado.
#[cfg(feature = "telas")]
fn editor_vazio(nome: &str, tema: Tema) -> (Ambiente, H) {
    let amb = Ambiente::vazio(nome);
    let editor = Editor::novo(Arc::clone(&amb.servico), tema, Partida::default());
    let tema_egui = if tema == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
    let mut h = Harness::builder()
        .with_size(TAMANHO)
        .with_pixels_per_point(1.0)
        .with_theme(tema_egui)
        .with_max_steps(64)
        .wgpu()
        .build_ui_state(|ui, e: &mut Editor| e.desenhar(ui, None), editor);
    esperar(&mut h, "o editor", |h| {
        fontes::do_sistema_carregadas(&h.ctx) && h.state().carregado() && !h.state().ocupado()
    });
    (amb, h)
}

/// O centro do primeiro texto pintado igual a `texto` (uma célula, uma letra de coluna, um cabeçalho).
#[cfg(feature = "telas")]
fn centro_do_texto(h: &H, texto: &str) -> egui::Pos2 {
    h.output()
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == texto => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
        .unwrap_or_else(|| panic!("o texto {texto:?} pintado"))
}

#[cfg(feature = "telas")]
fn clique_direito(h: &mut H, pos: egui::Pos2) {
    use egui::{Event, Modifiers, PointerButton};
    h.hover_at(pos);
    h.event(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
    h.event(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    h.remove_cursor();
    h.run_steps(1);
}

/// Um `.xlsx` de verdade (o exportador do DataBox) com uma planilha por nome, cada uma com `n` linhas.
#[cfg(feature = "telas")]
fn xlsx(destino: &std::path::Path, planilhas: &[&str], n: i64) {
    use comum::{Resultado, Valor};
    let colunas = vec!["id".to_string(), "regiao".to_string(), "valor".to_string()];
    let fontes = planilhas.iter().map(|nome| {
        let linhas: Vec<Resultado<Vec<Valor>>> = (1..=n)
            .map(|i| {
                Ok(vec![Valor::Inteiro(i), Valor::Texto(format!("região {}", i % 7)), Valor::Real(i as f64 * 1.5)])
            })
            .collect();
        (nome.to_string(), colunas.clone(), linhas)
    });
    exportar::exportar_varias(
        fontes,
        exportar::Formato::Xlsx,
        destino,
        &exportar::Opcoes::default(),
        None,
        exportar::Acompanhamento::default(),
    )
    .expect("exportar o xlsx");
}

/// Etapa 5, passo 15: o editor de CSV parado, com uma célula em edição (o cursor que pisca não conta), com o menu do
/// cabeçalho aberto e com a janela Trazer colunas aberta (depois de calcular o casamento); o editor de JSON parado,
/// válido e com o erro (a validação de 250 ms não fica se repetindo); o leitor de um JSON acima do limite de edição;
/// e o de texto. Nenhum pede repintura parado.
#[cfg(feature = "telas")]
#[test]
fn editores_de_csv_e_de_texto_parados_nao_repintam() {
    let (amb, mut h) = editor_vazio("editores_parados", Tema::Escuro);
    let pasta = amb.pasta().join("arquivos");
    std::fs::create_dir_all(&pasta).expect("a pasta");
    let estoque = pasta.join("estoque_2026.csv");
    std::fs::write(&estoque, "sku;regiao;quantidade\n1;Sul;10\n2;Norte;20\n3;Leste;5\n").expect("o csv");
    let precos = pasta.join("precos.csv");
    std::fs::write(&precos, "sku;preco\n1;9,90\n2;19,90\n").expect("o outro csv");

    // a origem do Trazer colunas numa outra aba
    h.state_mut().ir(Rota::arquivo(&precos));
    esperar(&mut h, "o precos.csv", |h| h.query_by_label("2 linhas · 2 colunas").is_some() && !h.state().ocupado());
    h.state_mut().ir(Rota::arquivo(&estoque));
    esperar(&mut h, "o CSV na grade", |h| h.query_by_label("3 linhas · 3 colunas").is_some() && !h.state().ocupado());
    conferir_parado(&mut h, "o editor de CSV");

    // uma célula em edição (F2 na ativa)
    let norte = centro_do_texto(&h, "Norte");
    h.hover_at(norte);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: norte,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    h.remove_cursor();
    h.key_press(Key::F2);
    h.run_steps(3);
    assert!(h.query_all(by().role(Role::TextInput).label("regiao, linha 2")).next().is_some(), "a célula em edição");
    conferir_parado(&mut h, "o CSV com uma célula em edição");
    esc(&mut h);
    conferir_parado(&mut h, "o CSV depois da edição");

    // o menu do cabeçalho
    h.get(by().role(Role::Button).label("regiao")).click_secondary();
    h.run_steps(3);
    h.remove_cursor();
    h.run_steps(1);
    assert!(h.query_by_label("Colar com correspondência").is_some(), "o menu do cabeçalho abriu");
    conferir_parado(&mut h, "o menu do cabeçalho do CSV");
    esc(&mut h);

    // a janela Trazer colunas, depois de ler as colunas e casar as linhas
    clicar(&mut h, Role::Button, "Trazer colunas");
    esperar(&mut h, "a janela Trazer colunas", |h| {
        h.query_by_label("para estoque_2026.csv").is_some() && !h.state().ocupado()
    });
    conferir_parado(&mut h, "a janela Trazer colunas");
    esc(&mut h);
    conferir_parado(&mut h, "o CSV depois da janela Trazer colunas");

    // o JSON válido e com o erro, e o texto
    let config = pasta.join("config.json");
    std::fs::write(&config, "{\n  \"nome\": \"DataBox\",\n  \"versao\": 1\n}\n").expect("o json");
    h.state_mut().ir(Rota::arquivo(&config));
    esperar(&mut h, "o JSON validado", |h| h.query_by_label("JSON válido").is_some() && !h.state().ocupado());
    conferir_parado(&mut h, "o editor de JSON válido");
    let quebrado = pasta.join("quebrado.json");
    std::fs::write(&quebrado, "{\n  \"nome\": \"DataBox\"\n  \"versao\": 1\n}\n").expect("o json com erro");
    h.state_mut().ir(Rota::arquivo(&quebrado));
    let com_erro =
        |h: &H| h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with("JSON inválido"))).next().is_some();
    esperar(&mut h, "o erro do JSON", |h| com_erro(h) && !h.state().ocupado());
    conferir_parado(&mut h, "o editor de JSON com o erro");
    let notas = pasta.join("notas.txt");
    std::fs::write(&notas, "primeira linha\nsegunda linha\n").expect("o txt");
    h.state_mut().ir(Rota::arquivo(&notas));
    esperar(&mut h, "o texto", |h| matches!(h.state().rota(), Rota::Arquivo { .. }) && !h.state().ocupado());
    conferir_parado(&mut h, "o editor de texto");

    // o leitor: um JSON acima do limite de edição abre só para leitura
    let grande = pasta.join("grande.json");
    let mut texto = String::from("[\n");
    // acima dos 512 KiB do `texto::limites::LIMITE_EDICAO`
    while texto.len() < 600 * 1024 {
        texto.push_str("  {\"id\": 1, \"nome\": \"uma linha comprida o bastante para encher o arquivo\"},\n");
    }
    texto.push_str("  {}\n]\n");
    std::fs::write(&grande, texto).expect("o json grande");
    h.state_mut().ir(Rota::arquivo(&grande));
    let no_leitor = |h: &H| {
        h.query_all_by(|n| n.label().or(n.value()).is_some_and(|l| l.contains("abre só para leitura"))).next().is_some()
            && h.query_by_label("JSON válido").is_some()
            && !h.state().ocupado()
    };
    esperar(&mut h, "o leitor", no_leitor);
    conferir_parado(&mut h, "o leitor do JSON grande");
}

/// Etapa 6, passo 9: a planilha parada; depois de o índice de uma planilha maior terminar (a barra "Lendo a
/// planilha…" para e a rolagem fica com a altura final); a faixa das planilhas com muitas abas (as setas ‹ ›, o
/// ponteiro nas pílulas, a lista aberta); e o menu do cabeçalho aberto. Nenhum pede repintura parado.
#[cfg(feature = "telas")]
#[test]
fn planilha_parada_nao_repinta() {
    let (amb, mut h) = editor_vazio("planilha_parada", Tema::Claro);
    let pasta = amb.pasta().join("planilhas");
    std::fs::create_dir_all(&pasta).expect("a pasta");
    let nomes: Vec<String> = (1..=14).map(|i| format!("Lançamentos do mês {i:02}")).collect();
    let nomes: Vec<&str> = nomes.iter().map(String::as_str).collect();
    let relatorio = pasta.join("relatorio_q3.xlsx");
    xlsx(&relatorio, &nomes, 20_000);
    h.state_mut().ir(Rota::arquivo(&relatorio));
    let aba = "Lançamentos do mês 01, planilha 1 de 14";
    esperar(&mut h, "a planilha abrir", |h| h.query(by().role(Role::Tab).label(aba)).is_some());
    // o índice da planilha de 20.000 linhas (e o cabeçalho) termina, e com ele a barra; o rodapé fica com a contagem
    // final
    esperar(&mut h, "o índice terminar", |h| {
        let lendo = h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with("Lendo a planilha"))).next().is_some();
        let contado = h
            .query_all_by(|n| n.label().or(n.value()).is_some_and(|l| l.starts_with("20.001 linhas")))
            .next()
            .is_some();
        !h.state().ocupado() && !lendo && contado
    });
    conferir_parado(&mut h, "a planilha depois do índice");

    // a faixa das planilhas: o ponteiro nas pílulas e nas setas, a seta da direita, a lista
    passar_e_sair(
        &mut h,
        &[
            (Role::Tab, aba),
            (Role::Tab, "Lançamentos do mês 02, planilha 2 de 14"),
            (Role::Button, "Rolar as planilhas para a direita"),
        ],
    );
    conferir_parado(&mut h, "a faixa das planilhas depois do ponteiro");
    clicar(&mut h, Role::Button, "Rolar as planilhas para a direita");
    conferir_parado(&mut h, "a faixa das planilhas rolada");
    clicar(&mut h, Role::Tab, "Lançamentos do mês 02, planilha 2 de 14");
    esperar(&mut h, "a segunda planilha", |h| !h.state().ocupado());
    conferir_parado(&mut h, "a segunda planilha");
    clicar(&mut h, Role::Button, "Planilhas");
    assert!(menu_aberto(&h.ctx) || h.query_by_label("Lançamentos do mês 14").is_some(), "a lista das planilhas");
    conferir_parado(&mut h, "a lista das planilhas aberta");
    esc(&mut h);

    // o menu do cabeçalho
    let b = centro_do_texto(&h, "B");
    clique_direito(&mut h, b);
    assert!(h.query_by_label("Copiar coluna").is_some(), "o menu do cabeçalho da planilha");
    conferir_parado(&mut h, "o menu do cabeçalho da planilha");
    esc(&mut h);
    conferir_parado(&mut h, "a planilha depois do menu");
}
