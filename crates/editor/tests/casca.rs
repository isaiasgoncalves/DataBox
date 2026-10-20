//! A casca inteira no `egui_kittest`: Início, abrir por caminho digitado e pelos recentes, a barra de título (os botões da janela, o
//! arrasto, o duplo clique, as bordas, o título) e os menus Arquivo (com os Abertos recentemente), Editar, Exibir
//! (com o tema) e Ajuda, a barra de ferramentas (ordem fixa, verbos de cada aba, F5), as Propriedades, a barra de ícones, o
//! Explorador, o Histórico de consultas e a barra de status, navegação, voltar/avançar,
//! transação aberta, fechar banco, banco trocado por outro lugar, as abas de documento (várias partes e Consultas,
//! fechar, Ctrl+Tab, o histórico único, a transação entre as Consultas, as tabelas que mudam ou somem) e as capturas
//! em `capturas/rust/`, com os nomes das de referência.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::path::PathBuf;

use apoio::*;
use editor::{Partida, PedidoDialogo, menu_aberto};
use egui::accesskit::Role;
use egui::{Key, Modifiers, PointerButton, ViewportCommand, ViewportId};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Aba, Rota};
use interface::pecas::toast::{toast, toast_atual};
use interface::tema::Tema;

const TABELAS: [&str; 7] = ["clientes", "eventos", "itens_pedido", "notas", "pedidos", "produtos", "Tabela com espaço"];

fn comandos(h: &H) -> Vec<ViewportCommand> {
    h.output().viewport_output.get(&ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default()
}

/// Manda os eventos, um por quadro a 10ms um do outro (o passo do harness é de 0,25 s, e o duplo clique tem de
/// caber em 0,3 s), e junta os comandos da janela de todos esses quadros e de mais três, no passo normal (0,75 s
/// depois, um clique seguinte não forma um duplo nem um triplo com estes).
fn comandos_dos_eventos(h: &mut H, eventos: &[egui::Event]) -> Vec<ViewportCommand> {
    let mut todos = Vec::new();
    let inicio = h.ctx.input(|i| i.time);
    for (n, e) in eventos.iter().enumerate() {
        h.input_mut().time = Some(inicio + 0.01 * (n + 1) as f64);
        h.event(e.clone());
        h.step();
        todos.extend(comandos(h));
    }
    h.input_mut().time = None;
    for _ in 0..3 {
        h.step();
        todos.extend(comandos(h));
    }
    todos
}

fn mover(pos: egui::Pos2) -> egui::Event {
    egui::Event::PointerMoved(pos)
}

fn apertar(pos: egui::Pos2, apertado: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: apertado, modifiers: Modifiers::NONE }
}

/// Um clique com o botão principal em `pos`, com os comandos da janela que ele mandou.
fn clicar_em(h: &mut H, pos: egui::Pos2) -> Vec<ViewportCommand> {
    comandos_dos_eventos(h, &[mover(pos), apertar(pos, true), apertar(pos, false)])
}

/// O clique no meio do nó com este papel e rótulo, com os comandos da janela.
fn clicar_no(h: &mut H, papel: Role, rotulo: &str) -> Vec<ViewportCommand> {
    let centro = rect_de(h, papel, rotulo).center();
    clicar_em(h, centro)
}

/// O título com este texto na barra de título (os 30px de cima), se estiver lá.
fn titulo_na_barra(h: &H, texto: &str) -> Option<egui::Rect> {
    h.query_all(by().role(Role::Label).label(texto)).map(|n| n.rect()).find(|r| r.bottom() <= 30.0)
}

/// Um ponto do espaço livre da barra de título: entre o menu Ajuda e o título.
fn ponto_livre(h: &H) -> egui::Pos2 {
    let ajuda = botao_menu(h, "Ajuda").rect();
    egui::pos2(ajuda.right() + 30.0, 15.0)
}

/// A janela maximizada ou não, como o eframe diria.
fn maximizada(h: &mut H, sim: bool) {
    h.input_mut().viewports.get_mut(&ViewportId::ROOT).expect("viewport").maximized = Some(sim);
    h.run_steps(2);
}

/// Lê um texto do banco por um serviço à parte (só leitura).
fn ler_texto(caminho: &std::path::Path, sql: &str) -> String {
    let pasta = caminho.parent().expect("pasta").join(format!("leitura-{}", std::process::id()));
    std::fs::create_dir_all(&pasta).expect("pasta");
    let s = servico::Servico::novo(prefs::Preferencias::abrir(Some(pasta.join("p.json"))));
    let info = s.abrir_banco(caminho, true, false).expect("abrir só leitura");
    let r = s.sql(&info.id, sql, 10).expect("consulta");
    let texto = match r.resultados.first() {
        Some(banco::ResultadoComando::Linhas { linhas, .. }) => {
            linhas.first().and_then(|l| l.first()).map(|c| format!("{c:?}")).unwrap_or_default()
        }
        outro => panic!("{outro:?}"),
    };
    let _ = s.fechar_banco(&info.id, true);
    let _ = std::fs::remove_dir_all(&pasta);
    texto
}

// ---- Início ---------------------------------------------------------------------------------------------------

/// O campo do caminho do Início (pasta, banco ou arquivo de dados).
const CAMPO_DO_INICIO: &str = "Caminho da pasta ou do arquivo";

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

// ---- Visão geral ----------------------------------------------------------------------------------------------

/// Os contadores da dock do exemplo: as visões e os gatilhos no singular (o exemplo tem um de cada).
const DOCK_DO_EXEMPLO: [&str; 4] = ["7 Tabelas", "1 Visão", "1 Gatilho", "Tamanho"];

#[test]
fn visao_geral_faixa_cartoes_e_tabelas() {
    let amb = Ambiente::exemplo("visao_geral");
    let copia = amb.pasta().join("copias").join("exemplo (cópia).db");
    std::fs::create_dir_all(copia.parent().expect("pasta")).expect("pasta");
    let destino = copia.clone();
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, Partida::default(), move |p| match p {
        PedidoDialogo::CopiaDeSeguranca { nome_sugerido } => {
            assert_eq!(nome_sugerido, "exemplo (cópia).db");
            Some(destino.clone())
        }
        _ => None,
    });
    assert_eq!(h.state().rota(), &Rota::Banco);
    // a faixa: o nome e o caminho inteiro (no leitor de tela), logo abaixo da faixa de abas
    let aberto = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string()).unwrap_or_default();
    let caminho = h.query_all_by_label(&aberto).map(|n| n.rect()).min_by(|a, b| a.top().total_cmp(&b.top()));
    let doc = documento(&h);
    assert!(caminho.is_some_and(|r| r.top() > doc.top() && r.bottom() < doc.top() + 42.0), "na faixa: {caminho:?}");
    // os contadores na dock, no singular com um, e a lista de Tabelas (sem o "7 tabelas" do título)
    for c in DOCK_DO_EXEMPLO {
        assert!(tem_texto(&h, c), "{c} na dock: {:?}", textos(&h));
    }
    assert!(tem_texto_exato(&h, "Tabelas") && !tem_texto(&h, "7 tabelas · clique"));
    for t in TABELAS {
        assert!(h.query(by().role(Role::Button).label(t)).is_some(), "{t} na tabela da visão geral");
    }
    // o botão de cada linha abre a vista Colunas da tabela (revisão de UI de 19/10/2026)
    assert_eq!(h.query_all(by().role(Role::Button).label("Colunas")).count(), 7);
    assert!(tem_texto(&h, "virtual") && tem_texto(&h, "sem rowid"));
    // o cabeçalho de antes saiu: a Nova tabela e o Exportar são os da barra
    assert!(h.query(by().role(Role::Button).label("Exportar o banco")).is_none());
    assert!(h.query(by().role(Role::Button).label("+ Nova tabela")).is_none());
    assert!(botao_ligado(&h, "Nova tabela") && botao_ligado(&h, "Exportar"));
    assert!(tem_texto(&h, "verificadas") || tem_texto(&h, "desligadas"));
    capturar(&mut h, "banco-visao-geral-claro.png");

    clicar_botao(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert!(tem_texto(&h, "O banco passou na verificação de integridade."), "{:?}", textos(&h));
    capturar(&mut h, "banco-integridade-claro.png");

    // VACUUM pergunta antes
    clicar_botao(&mut h, "Compactar (VACUUM)");
    esperar_texto(&mut h, "Compactar reescreve o arquivo inteiro");
    capturar(&mut h, "confirmar-vacuum-claro.png");
    clicar_botao(&mut h, "Compactar");
    assentar(&mut h);
    assert!(tem_texto(&h, "De "), "o \"de X para Y\" do VACUUM");
    capturar(&mut h, "banco-vacuum-claro.png");

    // cópia de segurança pelo diálogo (falso)
    clicar_botao(&mut h, "Fazer cópia de segurança");
    assentar(&mut h);
    esperar_texto(&mut h, "Cópia salva: ");
    assert!(copia.is_file());
    // o resultado é o toast: o nome do arquivo e o tamanho, o "Mostrar na pasta" no próprio toast e o caminho inteiro
    // só na dica (nada de caixa no painel)
    let nome = copia.file_name().and_then(|n| n.to_str()).expect("nome").to_string();
    let texto = toast_atual(&h.ctx).expect("o toast da cópia");
    assert!(
        texto.starts_with(&format!("Cópia salva: {nome} · "))
            && !texto.contains(copia.parent().unwrap().to_str().unwrap()),
        "{texto}"
    );
    assert_eq!(interface::pecas::toast::rotulo_da_acao_do_toast(&h.ctx).as_deref(), Some("Mostrar na pasta"));
    let dica = interface::pecas::toast::dica_do_toast(&h.ctx).expect("a dica");
    assert!(
        std::path::Path::new(&dica).ends_with(std::path::Path::new("copias").join(nome)),
        "o caminho inteiro na dica: {dica}"
    );
    assert!(h.query(by().role(Role::Button).label("Mostrar na pasta")).is_some());
    assert!(!tem_texto(&h, "Cópia salva em"), "sem a caixa de antes");
    capturar(&mut h, "banco-copia-claro.png");

    // o nome abre a tabela; "Colunas" abre a tabela com a vista Colunas nas Propriedades
    h.get(by().role(Role::Button).label("pedidos")).click();
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::objeto("pedidos", false));
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    h.query_all(by().role(Role::Button).label("Colunas")).next().expect("Colunas").click();
    h.run_steps(2);
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));
    assert_eq!(h.state().vista_da_tabela(), Some("Colunas"));
}

#[test]
fn visao_geral_escuro() {
    let amb = Ambiente::exemplo("visao_geral_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    assert!(tem_texto(&h, "7 Tabelas"));
    capturar(&mut h, "banco-visao-geral-escuro.png");
    // a 960, com o Explorador: as Propriedades vão para o fim do documento
    let amb = Ambiente::exemplo("visao_geral_960_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, egui::vec2(960.0, 600.0), Partida::default(), |_| None);
    assert!(!h.state().propriedades_a_vista());
    capturar(&mut h, "banco-visao-geral-960-escuro.png");
}

/// "1 Visão", "1 Gatilho": o rótulo do contador no singular com um, no plural com os outros (item 26).
#[test]
fn cartoes_no_singular_com_um() {
    let amb = Ambiente::exemplo("singular");
    let mut h = harness(&amb);
    for (um, varios) in [("1 Visão", "1 Visões"), ("1 Gatilho", "1 Gatilhos"), ("7 Tabelas", "7 Tabela ")] {
        assert!(tem_texto_exato(&h, um) && !tem_texto(&h, varios), "{um}: {:?}", textos(&h));
    }
    // uma visão a mais: "2 Visões"
    amb.servico.sql(&amb.estado.banco_id(), "CREATE VIEW v_dois AS SELECT 1;", 10).expect("criar a visão");
    h.state_mut().emitir(estado::Acao::RecarregarObjetos);
    h.state_mut().emitir(estado::Acao::RecarregarBanco);
    esperar(&mut h, "a visão nova", |h| tem_texto_exato(h, "2 Visões"));
}

#[test]
fn visao_geral_so_leitura_so_tem_verificacoes_e_copia() {
    let amb = Ambiente::vazio("so_leitura");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let partida = Partida { caminho: Some(caminho), somente_leitura: true };
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, partida, |_| None);
    // a pílula "só leitura" na faixa, logo depois do nome
    assert!(tem_texto(&h, "só leitura"));
    let pilula = h.get(by().label("só leitura")).rect();
    assert!(pilula.top() > documento(&h).top() && pilula.bottom() < documento(&h).top() + 42.0, "{pilula:?}");
    // a Manutenção nas Propriedades: só as verificações e a cópia
    let t = textos_das_propriedades(&h);
    for item in
        ["Seção Manutenção", "Verificar integridade", "Verificar chaves estrangeiras", "Fazer cópia de segurança"]
    {
        assert!(t.iter().any(|x| x == item), "{item} nas Propriedades: {t:?}");
    }
    assert!(h.query(by().role(Role::Button).label("Otimizar")).is_none());
    assert!(h.query(by().role(Role::Button).label("Compactar (VACUUM)")).is_none());
    assert!(h.query(by().role(Role::Button).label("+ Nova tabela")).is_none());
    assert!(h.query(by().role(Role::Link).label("+ Nova tabela")).is_none());
    let dica = ler_dica(&mut h, Role::Label, "Só leitura");
    assert!(dica.contains("exemplo.db está aberto só para leitura. Nada é gravado no arquivo."), "{dica:?}");
    capturar(&mut h, "banco-visao-geral-so-leitura-claro.png");

    // no menu Editar, a Nova tabela fica desligada, como na barra
    abrir_menu(&mut h, "Editar");
    let desligadas: Vec<bool> =
        h.query_all(by().role(Role::Button).label("Nova tabela")).map(|n| n.accesskit_node().is_disabled()).collect();
    assert_eq!(desligadas, [true, true], "o item do menu e o botão da barra");
}

/// A Manutenção nas Propriedades: os cinco `Botao::lista` da largura do painel, na ordem da prancheta, sem
/// reticência.
#[test]
fn manutencao_nas_propriedades() {
    let amb = Ambiente::exemplo("manutencao_propriedades");
    let mut h = harness(&amb);
    assert!(h.state().propriedades_a_vista());
    let t = textos_das_propriedades(&h);
    let ordem = [
        "Seção Arquivo",
        "Seção Manutenção",
        "Verificar integridade",
        "Verificar chaves estrangeiras",
        "Otimizar",
        "Compactar (VACUUM)",
        "Fazer cópia de segurança",
    ];
    let posicoes: Vec<usize> = ordem
        .iter()
        .map(|item| t.iter().position(|x| x == item).unwrap_or_else(|| panic!("{item} nas Propriedades: {t:?}")))
        .collect();
    assert!(posicoes.windows(2).all(|p| p[0] < p[1]), "na ordem: {t:?}");
    let doc = documento(&h);
    for rotulo in &ordem[2..] {
        let r = rect_de(&h, Role::Button, rotulo);
        // o painel tem 272px; o botão, a largura toda menos o `padding:0 8px` e a borda
        assert!(r.left() > doc.right() && (r.width() - (272.0 - 2.0 - 16.0)).abs() < 1.0, "{rotulo}: {r:?}");
    }
    assert!(!tem_texto(&h, "Fazer cópia de segurança…"));
    capturar(&mut h, "propriedades-visao-geral-manutencao-claro.png");
}

/// "Buscar tabelas" filtra a lista pelo nome, sem acento nem caixa; sem nenhuma, a frase do vazio.
#[test]
fn buscar_tabelas_filtra_a_lista() {
    let amb = Ambiente::exemplo("buscar_tabelas");
    let mut h = harness(&amb);
    let na_lista = |h: &H| -> Vec<&str> {
        TABELAS.iter().copied().filter(|t| h.query(by().role(Role::Button).label(t)).is_some()).collect()
    };
    assert_eq!(na_lista(&h), TABELAS);
    digitar(&mut h, "Buscar tabelas", "PEDIDO");
    assert_eq!(na_lista(&h), ["itens_pedido", "pedidos"]);
    // sem acento: "espaco" acha "Tabela com espaço"
    campo_da_busca(&h, "Buscar tabelas").focus();
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    campo_da_busca(&h, "Buscar tabelas").type_text("espaco");
    h.run_steps(2);
    assert_eq!(na_lista(&h), ["Tabela com espaço"]);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    campo_da_busca(&h, "Buscar tabelas").type_text("zzz");
    h.run_steps(2);
    assert!(na_lista(&h).is_empty());
    assert!(tem_texto(&h, "Nenhuma tabela corresponde à busca."), "{:?}", textos(&h));
    // o Esc limpa e a lista volta
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert_eq!(na_lista(&h), TABELAS);
}

/// O Copiar caminho do Arquivo copia o caminho inteiro e dá o toast "Caminho copiado.".
#[test]
fn copiar_caminho_copia_e_avisa() {
    let amb = Ambiente::exemplo("copiar_caminho");
    let mut h = harness(&amb);
    let caminho = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string()).unwrap_or_default();
    // o Caminho numa linha, com o botão à direita dele, nas Propriedades
    let botao = rect_de(&h, Role::Button, "Copiar caminho");
    assert!(botao.left() > documento(&h).right(), "{botao:?}");
    h.get(by().role(Role::Button).label("Copiar caminho")).click();
    let mut copiado = None;
    for _ in 0..3 {
        h.step();
        copiado = copiado.or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
        });
    }
    assert_eq!(copiado.as_deref(), Some(caminho.as_str()));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Caminho copiado."));
}

/// Decisão 4: com o painel escondido (Ctrl+Alt+B), as seções das Propriedades vão para o fim do documento, depois da
/// lista e do `.tit` "Propriedades"; de volta, ficam só no painel.
#[test]
fn sem_o_painel_as_secoes_vao_para_o_fim_do_documento() {
    let amb = Ambiente::exemplo("secoes_no_fim");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1440.0, 1300.0), Partida::default(), |_| None);
    let painel = Modifiers { alt: true, ..CTRL };
    h.key_press_modifiers(painel, Key::B);
    h.run_steps(3);
    assert!(!h.state().propriedades_a_vista());
    let doc = documento(&h);
    assert_eq!(doc.right(), 1440.0 - 8.0, "o documento fica com a largura");
    let ultima = rect_de(&h, Role::Button, "Tabela com espaço");
    let titulo = h.get(by().role(Role::Label).label("Propriedades")).rect();
    let arquivo = h.get_by_label("Seção Arquivo").rect();
    let manutencao = h.get_by_label("Seção Manutenção").rect();
    assert!(ultima.bottom() < titulo.top() && titulo.bottom() <= arquivo.top(), "{ultima:?} {titulo:?} {arquivo:?}");
    assert!(arquivo.top() < manutencao.top());
    for r in [arquivo, manutencao, rect_de(&h, Role::Button, "Verificar integridade")] {
        assert!(r.left() >= doc.left() && r.right() <= doc.right(), "dentro do documento: {r:?}");
    }
    // as ações funcionam no fim também
    clicar_botao(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert!(tem_texto(&h, "O banco passou na verificação de integridade."), "{:?}", textos(&h));
    capturar(&mut h, "banco-visao-geral-sem-painel-claro.png");
    // de volta ao painel: nada no fim
    h.key_press_modifiers(painel, Key::B);
    h.run_steps(3);
    assert!(h.state().propriedades_a_vista());
    assert!(h.get_by_label("Seção Arquivo").rect().left() > documento(&h).right());
    assert!(
        h.query(by().role(Role::Label).label("Propriedades")).is_none_or(|n| n.rect().left() > documento(&h).right())
    );
}

/// Decisão 3: o Atualizar pede à casca o BancoInfo e os objetos, sem refazer a Visão geral (a busca e o resultado da
/// manutenção ficam).
#[test]
fn atualizar_rele_banco_e_objetos_pela_casca() {
    let amb = Ambiente::exemplo("atualizar_visao_geral");
    let mut h = harness(&amb);
    clicar_botao(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert!(tem_texto(&h, "O banco passou na verificação de integridade."));
    digitar(&mut h, "Buscar tabelas", "a");
    // uma tabela nova por fora (sem a casca saber)
    amb.servico.sql(&amb.estado.banco_id(), "CREATE TABLE zebra (a);", 10).expect("criar");
    assert!(tem_texto(&h, "7 Tabelas") && h.query(by().role(Role::Button).label("zebra")).is_none());
    h.remove_cursor();
    clicar_primeiro(&mut h, Role::Button, "Atualizar");
    esperar(&mut h, "a lista nova", |h| h.query(by().role(Role::Button).label("zebra")).is_some());
    assert!(tem_texto(&h, "8 Tabelas"), "a dock relida: {:?}", textos(&h));
    assert!(existe(&h, Role::Link, "zebra"), "o Explorador recebe a mesma lista");
    assert_eq!(campo_da_busca(&h, "Buscar tabelas").value().as_deref(), Some("a"), "a busca fica");
}

// ---- barra de título, menus, tema e atalhos -------------------------------------------------------------------

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

/// O link das preferências no Sobre: `%APPDATA%\DataBox` no Windows; nos outros sistemas, a pasta de verdade.
fn rotulo_preferencias(amb: &Ambiente) -> String {
    let pasta = amb.servico.prefs().arquivo().parent().map(|p| p.display().to_string()).unwrap_or_default();
    if cfg!(windows) { r"%APPDATA%\DataBox".to_string() } else { pasta }
}

/// O link do arquivo de log no Sobre: `%LOCALAPPDATA%\DataBox\databox.log` no Windows; nos outros, o caminho real.
fn rotulo_log() -> String {
    if cfg!(windows) {
        r"%LOCALAPPDATA%\DataBox\databox.log".to_string()
    } else {
        servico::arquivo_log().display().to_string()
    }
}

#[test]
fn ajuda_sobre_o_databox() {
    let amb = Ambiente::exemplo("sobre");
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Sobre o DataBox");
    h.run_steps(2);
    assert!(h.state().janela_aberta());
    assert!(existe(&h, Role::Dialog, "Sobre o DataBox"));
    // o logo e o nome, a versão, a descrição e a ficha, sem Consolas
    let app = h.state().estado().app;
    let versao = format!("Versão {}", app.versao);
    assert_eq!(versao, "Versão 0.2.0", "a versão do aplicativo");
    let sqlite = format!("{}, embutido", app.sqlite_versao);
    let (preferencias, log) = (rotulo_preferencias(&amb), rotulo_log());
    for texto in [
        "DataBox",
        versao.as_str(),
        "Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.",
        "Compilação",
        "Renderizador",
        "OpenGL (glow)",
        "SQLite",
        sqlite.as_str(),
        "Sistema",
        "Licença",
        "MIT",
        "Preferências",
        preferencias.as_str(),
        "Arquivo de log",
        log.as_str(),
        "Desenvolvido por Isaías Gonçalves. Feito em Rust, com egui e SQLite.",
    ] {
        assert!(tem_texto_exato(&h, texto), "{texto} na janela Sobre: {:?}", textos(&h));
    }
    // o "Sistema" vem do `std::env::consts` e a "Compilação" é "data · hash" (ou "desconhecida")
    let sistema = if cfg!(windows) { "Windows · x64" } else { "" };
    if cfg!(all(windows, target_arch = "x86_64")) {
        assert!(tem_texto_exato(&h, sistema), "{:?}", textos(&h));
    }
    assert!(tem_texto(&h, " · ") || tem_texto_exato(&h, "desconhecida"));
    for botao in ["Licenças de terceiros", "Copiar informações", "Fechar"] {
        assert!(existe(&h, Role::Button, botao), "{botao}");
    }
    assert!(existe(&h, Role::Link, "MIT") && existe(&h, Role::Link, &preferencias));
    capturar(&mut h, "janela-sobre-claro.png");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    assert!(botao_menu(&h, "Ajuda").is_focused(), "o foco volta ao botão Ajuda");
}

/// Os dois temas e as duas larguras da janela Sobre, para comparar com a prancheta 13.
#[test]
fn capturas_do_sobre() {
    for (tema, tamanho, nome) in [
        (Tema::Escuro, TAMANHO, "sobre-escuro.png"),
        (Tema::Claro, TAMANHO, "sobre-claro.png"),
        (Tema::Escuro, egui::vec2(960.0, 700.0), "sobre-escuro-960.png"),
        (Tema::Claro, egui::vec2(960.0, 700.0), "sobre-claro-960.png"),
    ] {
        let amb = Ambiente::exemplo("sobre_capturas");
        let mut h = harness_com(&amb, tema, tamanho, Partida::default(), |_| None);
        abrir_menu(&mut h, "Ajuda");
        clicar_item(&mut h, "Sobre o DataBox");
        h.run_steps(3);
        capturar(&mut h, nome);
    }
}

fn abrir_o_sobre(h: &mut H) {
    abrir_menu(h, "Ajuda");
    clicar_item(h, "Sobre o DataBox");
    h.run_steps(2);
    assert!(existe(h, Role::Dialog, "Sobre o DataBox"));
}

/// O que foi copiado nos próximos quadros.
fn texto_copiado(h: &mut H) -> Option<String> {
    let mut copiado = None;
    for _ in 0..3 {
        h.step();
        copiado = copiado.or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
        });
    }
    copiado
}

#[test]
fn copiar_informacoes() {
    let amb = Ambiente::exemplo("sobre_copiar");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    h.get(by().role(Role::Button).label("Copiar informações")).click();
    let copiado = texto_copiado(&mut h).expect("a ficha foi copiada");
    let app = h.state().estado().app;
    let linhas: Vec<&str> = copiado.lines().collect();
    assert_eq!(linhas[0], format!("DataBox {}", app.versao));
    assert!(linhas[1].starts_with("Compilação: "), "{copiado}");
    for esperada in [
        "Renderizador: OpenGL (glow)".to_string(),
        format!("SQLite: {}, embutido", app.sqlite_versao),
        "Licença: MIT".to_string(),
        r"Preferências: %APPDATA%\DataBox".to_string(),
        r"Arquivo de log: %LOCALAPPDATA%\DataBox\databox.log".to_string(),
    ] {
        assert!(linhas.contains(&esperada.as_str()), "{esperada}: {copiado}");
    }
    assert!(linhas.iter().any(|l| l.starts_with("Sistema: ")));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Informações copiadas."));
    assert!(existe(&h, Role::Dialog, "Sobre o DataBox"), "o Sobre continua aberto");
}

#[test]
fn licenca_mostra_o_texto_mit() {
    let amb = Ambiente::exemplo("sobre_licenca");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    clicar_link(&mut h, "MIT");
    h.run_steps(2);
    assert!(existe(&h, Role::Dialog, "Licença") && existe(&h, Role::Dialog, "Sobre o DataBox"));
    assert!(tem_texto(&h, "MIT License"), "{:?}", textos(&h));
    assert!(tem_texto(&h, "Permission is hereby granted, free of charge"), "{:?}", textos(&h));
    capturar(&mut h, "sobre-licenca-claro.png");
    // o Esc fecha só a de cima
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!existe(&h, Role::Dialog, "Licença") && existe(&h, Role::Dialog, "Sobre o DataBox"));
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
}

#[test]
fn licencas_de_terceiros_lista_as_bibliotecas() {
    let amb = Ambiente::exemplo("sobre_terceiros");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    clicar_botao(&mut h, "Licenças de terceiros");
    h.run_steps(2);
    assert!(existe(&h, Role::Dialog, "Licenças de terceiros"));
    for texto in
        ["Bibliotecas do projeto", "egui", "eframe", "egui_extras", "rusqlite", "chrono", "rfd", "zip", "image"]
    {
        assert!(tem_texto_exato(&h, texto), "{texto}: {:?}", textos(&h));
    }
    // as versões de verdade vêm do Cargo.lock; o egui e o eframe são os da 0.36
    assert!(tem_texto_exato(&h, "0.36.2"));
    assert!(tem_texto_exato(&h, "MIT OR Apache-2.0"));
    assert!(!tem_texto_exato(&h, "egui_kittest"), "as de teste ficam de fora");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!existe(&h, Role::Dialog, "Licenças de terceiros") && existe(&h, Role::Dialog, "Sobre o DataBox"));
}

/// A lista que o build.rs fez: "nome\tversão\tlicença\tdireta".
const TERCEIROS: &str = include_str!(concat!(env!("OUT_DIR"), "/terceiros.txt"));

/// O texto de um nó: o rótulo ou, nos textos soltos, o valor.
fn texto_do_no(n: &egui_kittest::Node<'_>) -> String {
    n.accesskit_node().label().or_else(|| n.accesskit_node().value()).unwrap_or_default()
}

/// Os rótulos de texto da lista de terceiros que estão na árvore (nome, retângulo), do alto da lista para baixo.
fn linhas_da_lista(h: &H, topo: f32, base: f32) -> Vec<(String, egui::Rect)> {
    // só os textos da própria lista (atrás da janela há o Sobre e o resto do programa, também na árvore)
    let da_lista: std::collections::HashSet<&str> = TERCEIROS
        .lines()
        .flat_map(|l| l.split('\t').take(3))
        .chain(["Bibliotecas do projeto", "Dependências delas", "não informada"])
        .collect();
    let mut v: Vec<(String, egui::Rect)> = h
        .query_all_by(|n| n.role() == Role::Label && n.bounding_box().is_some())
        .map(|n| (texto_do_no(&n), n.rect()))
        .filter(|(t, r)| {
            da_lista.contains(t.as_str())
                && r.height() <= 24.0
                && r.width() > 0.0
                && r.center().y > topo
                && r.center().y < base
        })
        .collect();
    v.sort_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y).then(a.1.left().total_cmp(&b.1.left())));
    v
}

#[test]
fn licencas_de_terceiros_sem_cortes_e_rolam_ate_o_fim() {
    let ultima = TERCEIROS
        .lines()
        .rfind(|l| l.split('\t').nth(3) == Some("0"))
        .and_then(|l| l.split('\t').next())
        .expect("há dependências indiretas");
    for (tema, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let amb = Ambiente::exemplo("sobre_terceiros_rolagem");
        let mut h = harness_com(&amb, tema, TAMANHO, Partida::default(), |_| None);
        abrir_o_sobre(&mut h);
        clicar_botao(&mut h, "Licenças de terceiros");
        h.run_steps(3);
        // a janela tem 620 px de largura, centrada (o nó do diálogo não traz retângulo)
        let janela_direita = TAMANHO.x / 2.0 + 310.0;
        let titulo = h.get_by_label("Bibliotecas do projeto").rect();
        let (topo, base) = (titulo.top() - 1.0, titulo.top() + 381.0);
        // a lista inteira cabe na janela, sem uma coluna por cima da outra
        let confere = |h: &H, onde: &str| {
            let linhas = linhas_da_lista(h, topo, base);
            assert!(linhas.len() > 12, "{onde}: {linhas:?}");
            for (texto, r) in &linhas {
                assert!(r.right() <= janela_direita - 20.0, "{onde}: {texto} passa do corpo: {r:?} {janela_direita}");
            }
            for par in linhas.windows(2) {
                let ((a, ra), (b, rb)) = (&par[0], &par[1]);
                if (ra.center().y - rb.center().y).abs() < 1.0 {
                    assert!(ra.right() <= rb.left(), "{onde}: {a} invade {b}: {ra:?} {rb:?}");
                }
            }
        };
        confere(&h, "no alto");
        assert!(tem_texto_exato(&h, "Bibliotecas do projeto"));
        assert!(!tem_texto_exato(&h, ultima), "a última ainda não devia estar à vista");
        capturar(&mut h, &format!("sobre-terceiros-{sufixo}.png"));

        // a roda do mouse sobre a lista, até o fim
        let sobre_a_lista = titulo.left_top() + egui::vec2(60.0, 60.0);
        h.hover_at(sobre_a_lista);
        let mut dica_conferida = false;
        for passo in 0..120 {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -150.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            });
            h.run_steps(2);
            // em todo o caminho, nada passa da borda nem fica por cima do vizinho
            if passo % 6 == 0 {
                confere(&h, &format!("rolando, passo {passo}"));
            }
            // a licença cortada com "…" guarda o texto inteiro na dica
            if !dica_conferida {
                let cortada = linhas_da_lista(&h, topo, base).into_iter().find(|(t, r)| {
                    r.left() > 760.0 && t.len() > 40 && r.right() > janela_direita - 40.0 && r.center().y < base - 12.0
                });
                if let Some((texto, _)) = cortada {
                    // a rolagem é suavizada: espera parar e acha a linha de novo antes de pôr o ponteiro nela
                    h.run_steps(10);
                    let Some((_, r)) = linhas_da_lista(&h, topo, base).into_iter().find(|(t, _)| *t == texto) else {
                        continue;
                    };
                    h.hover_at(r.center());
                    h.run_steps(3);
                    let n = textos(&h).iter().filter(|t| **t == texto).count();
                    assert!(n >= 2, "a licença cortada ({texto}) devia ter dica com o texto inteiro: {n}");
                    dica_conferida = true;
                    h.hover_at(sobre_a_lista);
                    h.run_steps(2);
                }
            }
        }
        h.run_steps(3);
        if TERCEIROS.lines().any(|l| l.split('\t').nth(2).is_some_and(|x| x.len() > 45)) {
            assert!(dica_conferida, "nenhuma licença longa foi cortada à vista");
        }
        assert!(tem_texto_exato(&h, ultima), "a última ({ultima}) não foi desenhada: {:?}", textos(&h));
        let r = h
            .query_all_by(|n| n.label().as_deref() == Some(ultima) || n.value().as_deref() == Some(ultima))
            .map(|n| n.rect())
            .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
            .expect("a última");
        assert!(r.top() >= topo && r.bottom() <= base, "a última linha está à vista: {r:?} em {topo}..{base}");
        // sem espaço vazio embaixo: a última linha encosta no fim da área
        assert!(base - r.bottom() < 30.0, "linhas faltando no fim: sobram {} px", base - r.bottom());
        confere(&h, "no fim");
        capturar(&mut h, &format!("sobre-terceiros-fim-{sufixo}.png"));
    }
}

#[test]
fn links_das_pastas_revelam() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let amb = Ambiente::exemplo("sobre_links");
    let abertos: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let anotados = Rc::clone(&abertos);
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(|_| None)
        .com_abrir_de_teste(move |p| anotados.borrow_mut().push(p.to_path_buf()));
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    abrir_o_sobre(&mut h);
    clicar_link(&mut h, &rotulo_preferencias(&amb));
    clicar_link(&mut h, &rotulo_log());
    let pasta = amb.servico.prefs().arquivo().parent().map(std::path::Path::to_path_buf).expect("a pasta");
    assert_eq!(abertos.borrow().as_slice(), [pasta, servico::arquivo_log()]);
}

#[test]
fn arquivo_de_log_no_rotulo() {
    let amb = Ambiente::exemplo("sobre_log");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    // o rótulo é "Arquivo de log" (não "Registro"), e a dica do link leva o caminho de verdade
    assert!(tem_texto_exato(&h, "Arquivo de log") && !tem_texto_exato(&h, "Registro"));
    h.get(by().role(Role::Link).label(&rotulo_log())).hover();
    h.run_steps(4);
    let log = servico::arquivo_log().display().to_string();
    assert!(tem_texto(&h, &format!("Abrir o arquivo: {log}")), "{:?}", textos(&h));
}

#[test]
fn sobre_mostra_o_renderizador_que_abriu_a_janela() {
    let amb = Ambiente::exemplo("sobre_wgpu");
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(|_| None)
        .com_renderizador(editor::linha_comando::Renderizador::Wgpu);
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    abrir_o_sobre(&mut h);
    assert!(tem_texto_exato(&h, "wgpu") && !tem_texto_exato(&h, "OpenGL (glow)"));
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

#[test]
fn titulo_no_centro_e_esmaecido_com_a_janela_inativa() {
    let amb = Ambiente::exemplo("titulo_centro");
    let mut h = harness(&amb);
    let titulo = titulo_na_barra(&h, "exemplo.db · DataBox").expect("o título na barra");
    assert!((titulo.center().x - TAMANHO.x / 2.0).abs() <= 1.0, "no centro da janela: {titulo:?}");
    assert!(titulo.left() > botao_menu(&h, "Ajuda").rect().right(), "depois dos menus");
    // a tinta do título contra o fundo da faixa: menos com a janela inativa
    let tinta = |h: &mut H| {
        h.remove_cursor();
        h.run_steps(2);
        let imagem = renderizar(h);
        let fundo = *imagem.get_pixel((titulo.left() - 6.0) as u32, titulo.center().y as u32);
        let mut soma = 0u64;
        for x in titulo.left() as u32..titulo.right() as u32 {
            for y in titulo.top() as u32..titulo.bottom() as u32 {
                let p = imagem.get_pixel(x, y);
                soma += (0..3).map(|i| u64::from(p[i].abs_diff(fundo[i]))).sum::<u64>();
            }
        }
        soma
    };
    let ativa = tinta(&mut h);
    h.input_mut().viewports.get_mut(&ViewportId::ROOT).expect("viewport").focused = Some(false);
    let inativa = tinta(&mut h);
    assert!(ativa > 0 && inativa * 10 < ativa * 8, "esmaece com a janela inativa: {ativa} → {inativa}");
    capturar(&mut h, "barra-titulo-inativa-claro.png");
}

#[test]
fn barra_de_titulo_minimiza_maximiza_e_fecha() {
    let amb = Ambiente::exemplo("barra_titulo_botoes");
    let mut h = harness(&amb);
    assert!(clicar_no(&mut h, Role::Button, "Minimizar").contains(&ViewportCommand::Minimized(true)));
    assert!(clicar_no(&mut h, Role::Button, "Maximizar").contains(&ViewportCommand::Maximized(true)));
    let regioes = editor::regioes_da_janela(&h.ctx).expect("as regiões publicadas");
    assert_eq!((regioes.borda, regioes.livre.len()), (5.0, 2));
    assert_eq!(regioes.botoes[2].max, egui::pos2(TAMANHO.x, 30.0), "o Fechar no canto");

    // maximizada: o do meio vira Restaurar e a borda some
    maximizada(&mut h, true);
    assert!(h.query(by().role(Role::Button).label("Maximizar")).is_none());
    let enviados = clicar_no(&mut h, Role::Button, "Restaurar");
    assert!(enviados.contains(&ViewportCommand::Maximized(false)), "{enviados:?}");
    assert_eq!(editor::regioes_da_janela(&h.ctx).map(|r| r.borda), Some(0.0));
    let perto_da_borda = egui::pos2(2.0, 450.0);
    let enviados = clicar_em(&mut h, perto_da_borda);
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::BeginResize(_))), "{enviados:?}");
    capturar(&mut h, "barra-titulo-maximizada-claro.png");
    maximizada(&mut h, false);
    assert!(existe(&h, Role::Button, "Maximizar"));

    // Fechar: o pedido de sempre (sem transação, o eframe fecha)
    let enviados = clicar_no(&mut h, Role::Button, "Fechar a janela");
    assert!(enviados.contains(&ViewportCommand::Close), "{enviados:?}");
    let dica = ler_dica(&mut h, Role::Button, "Fechar a janela");
    assert_eq!(dica, "Fechar");
}

#[test]
fn espaco_livre_arrasta_e_maximiza() {
    let amb = Ambiente::exemplo("barra_titulo_arrasto");
    let mut h = harness(&amb);
    let livre = ponto_livre(&h);
    // um clique só não arrasta nem maximiza
    let enviados = clicar_em(&mut h, livre);
    assert!(!enviados.contains(&ViewportCommand::StartDrag), "{enviados:?}");
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::Maximized(_))), "{enviados:?}");
    // o duplo clique maximiza (também no logo); maximizada, restaura
    let duplo = |p: egui::Pos2| [mover(p), apertar(p, true), apertar(p, false), apertar(p, true), apertar(p, false)];
    let enviados = comandos_dos_eventos(&mut h, &duplo(livre));
    assert!(enviados.contains(&ViewportCommand::Maximized(true)), "{enviados:?}");
    let enviados = comandos_dos_eventos(&mut h, &duplo(egui::pos2(18.0, 15.0)));
    assert!(enviados.contains(&ViewportCommand::Maximized(true)), "o logo: {enviados:?}");
    maximizada(&mut h, true);
    let enviados = comandos_dos_eventos(&mut h, &duplo(livre));
    assert!(enviados.contains(&ViewportCommand::Maximized(false)), "{enviados:?}");
    maximizada(&mut h, false);
    // arrastar 20px pelo espaço livre: o laço de arrasto do sistema, no começo do arrasto
    let adiante = livre + egui::vec2(20.0, 0.0);
    let arrasto = [mover(livre), apertar(livre, true), mover(livre + egui::vec2(10.0, 0.0)), mover(adiante)];
    let enviados = comandos_dos_eventos(&mut h, &arrasto);
    assert_eq!(enviados.iter().filter(|c| **c == ViewportCommand::StartDrag).count(), 1, "{enviados:?}");
    comandos_dos_eventos(&mut h, &[apertar(adiante, false)]);
    // arrastar a partir de um menu não arrasta a janela
    let menu = botao_menu(&h, "Editar").rect().center();
    let arrasto = [mover(menu), apertar(menu, true), mover(menu + egui::vec2(20.0, 0.0)), apertar(menu, false)];
    let enviados = comandos_dos_eventos(&mut h, &arrasto);
    assert!(!enviados.contains(&ViewportCommand::StartDrag), "{enviados:?}");
    // e o clique no menu ainda o abre (a borda de cima não come o meio dele)
    h.remove_cursor();
    h.run_steps(2);
    abrir_menu(&mut h, "Editar");
    assert!(menu_aberto(&h.ctx));
}

#[test]
fn bordas_redimensionam_a_janela() {
    let amb = Ambiente::exemplo("barra_titulo_bordas");
    let mut h = harness(&amb);
    for (pos, cursor, direcao) in [
        (egui::pos2(2.0, 450.0), egui::CursorIcon::ResizeWest, egui::ResizeDirection::West),
        (egui::pos2(1438.0, 898.0), egui::CursorIcon::ResizeSouthEast, egui::ResizeDirection::SouthEast),
        (egui::pos2(720.0, 2.0), egui::CursorIcon::ResizeNorth, egui::ResizeDirection::North),
        (egui::pos2(1439.0, 1.0), egui::CursorIcon::ResizeNorthEast, egui::ResizeDirection::NorthEast),
    ] {
        h.hover_at(pos);
        h.run_steps(2);
        assert_eq!(h.output().platform_output.cursor_icon, cursor, "o cursor em {pos:?}");
        // o redimensionar começa no aperto, sem esperar o arrasto
        let enviados = comandos_dos_eventos(&mut h, &[apertar(pos, true)]);
        assert!(enviados.contains(&ViewportCommand::BeginResize(direcao)), "{pos:?}: {enviados:?}");
        comandos_dos_eventos(&mut h, &[apertar(pos, false)]);
        h.remove_cursor();
        h.run_steps(2);
    }
    // a 6px da borda, já é o anel do fundo: nada
    h.hover_at(egui::pos2(6.0, 450.0));
    h.run_steps(2);
    assert_ne!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeWest);
}

#[test]
fn controles_da_janela_respondem_com_a_janela_modal_aberta() {
    let amb = Ambiente::exemplo("barra_titulo_modal");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    assert!(h.state().exportacao_aberta().is_some(), "a janela Exportar aberta");
    // os botões da janela ficam por cima do véu da modal
    assert!(clicar_no(&mut h, Role::Button, "Minimizar").contains(&ViewportCommand::Minimized(true)));
    let p = ponto_livre(&h);
    let arrasto = [mover(p), apertar(p, true), mover(p + egui::vec2(10.0, 0.0)), mover(p + egui::vec2(20.0, 0.0))];
    let enviados = comandos_dos_eventos(&mut h, &arrasto);
    assert!(enviados.contains(&ViewportCommand::StartDrag), "arrastar com a modal aberta: {enviados:?}");
    h.remove_cursor();
    h.run_steps(2);
    assert!(h.state().exportacao_aberta().is_some(), "a janela continua aberta");
    // os menus ficam sob o véu
    clicar_primeiro(&mut h, Role::Button, "Arquivo");
    assert!(!menu_aberto(&h.ctx));
}

#[test]
fn sem_a_barra_propria_a_faixa_nao_tem_os_controles() {
    let amb = Ambiente::exemplo("sem_barra_propria");
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_barra_propria(false);
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    for rotulo in ["Minimizar", "Maximizar", "Fechar a janela"] {
        assert!(h.query(by().role(Role::Button).label(rotulo)).is_none(), "{rotulo}");
    }
    assert!(editor::regioes_da_janela(&h.ctx).is_none());
    assert!(titulo_na_barra(&h, "exemplo.db · DataBox").is_some());
    let enviados = clicar_em(&mut h, egui::pos2(2.0, 450.0));
    assert!(!enviados.iter().any(|c| matches!(c, ViewportCommand::BeginResize(_))), "{enviados:?}");
    abrir_menu_arquivo(&mut h);
    assert!(menu_aberto(&h.ctx), "os menus continuam");
}

#[test]
fn parada_a_tela_nao_repinta_sem_parar() {
    let amb = Ambiente::exemplo("parada");
    let mut h = harness(&amb);
    for rota in [Rota::Banco, Rota::Inicio, Rota::objeto("clientes", false)] {
        h.state_mut().ir(rota);
        assentar(&mut h);
        h.try_run().expect("a tela para de pedir quadros");
        let espera = h.output().viewport_output.get(&ViewportId::ROOT).map(|v| v.repaint_delay);
        assert!(espera.is_some_and(|d| d >= std::time::Duration::from_millis(100)), "{espera:?}");
    }
}

#[test]
fn toast_captura() {
    let amb = Ambiente::exemplo("toast");
    let mut h = harness(&amb);
    toast(&h.ctx, "Linha salva.");
    h.run_steps(3);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Linha salva."));
    capturar(&mut h, "toast-claro.png");
}

// ---- barra de ícones, Explorador, Histórico, barra de status e navegação ---------------------------------------

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
    assert!(textos_das_propriedades(&h).contains(&tamanho));
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

// ---- abas de documento ----------------------------------------------------------------------------------------

fn s(textos: &[&str]) -> Vec<String> {
    textos.iter().map(|t| t.to_string()).collect()
}

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

// ---- várias Consultas SQL -------------------------------------------------------------------------------------

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

/// Executa o texto do editor da Consulta à vista (Ctrl+Enter) e espera a resposta.
fn executar(h: &mut H, sql: &str) {
    let campo = h.get_by_label("Comando SQL");
    campo.focus();
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    h.get_by_label("Comando SQL").type_text(sql);
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::Enter);
    h.run_steps(2);
    assentar(h);
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

// ---- abas e o banco -------------------------------------------------------------------------------------------

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

// ---- abas e a estrutura ---------------------------------------------------------------------------------------

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

// ---- transação aberta, fechar banco, banco trocado ------------------------------------------------------------

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

// ---- diálogos (com o gancho dos testes) -----------------------------------------------------------------------

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

// ---- barra de ferramentas e Propriedades ----------------------------------------------------------------------

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

// ---- janelas estreitas -----------------------------------------------------------------------------------------

/// Onde ficam as ações dos recentes com este rótulo ("Remover " … " da lista", com o nome do arquivo no meio).
fn acoes_dos_recentes(h: &H, comeco: &str, fim: &str) -> Vec<egui::Rect> {
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with(comeco) && l.ends_with(fim))).map(|n| n.rect()).collect()
}

/// "dd/mm/aaaa hh:mm" (o espaço do meio pode ser o sem quebra)
fn e_data(texto: &str) -> bool {
    let c: Vec<char> = texto.chars().collect();
    c.len() == 16 && c[2] == '/' && c[5] == '/' && c[10].is_whitespace() && c[13] == ':'
}

/// As ações dos recentes cabem no card, a data fica numa linha só e a pasta continua visível.
/// `pasta_minima`: a largura que a pasta (encurtada) ainda tem, no mínimo.
fn conferir_recentes(h: &H, direita: f32, pasta_minima: f32) {
    let botoes: Vec<egui::Rect> = acoes_dos_recentes(h, "Mostrar ", " na pasta")
        .into_iter()
        .chain(acoes_dos_recentes(h, "Remover ", " da lista"))
        .collect();
    assert!(!botoes.is_empty());
    for r in &botoes {
        assert!(r.right() <= direita + 0.5, "a ação {r:?} passa da borda do card ({direita})");
    }
    let datas: Vec<egui::Rect> =
        h.query_all_by(|n| n.label().or_else(|| n.value()).is_some_and(|l| e_data(&l))).map(|n| n.rect()).collect();
    assert!(!datas.is_empty(), "{:?}", textos(h));
    for d in &datas {
        assert!(d.height() < 25.0 && d.right() <= direita + 0.5, "a data numa linha só, dentro do card: {d:?}");
    }
    for r in &h.state().estado().prefs.recentes {
        let pasta = h.query_all_by_label(&r.pasta).next().map(|n| n.rect());
        assert!(pasta.is_some_and(|p| p.width() >= pasta_minima), "a pasta de {} aparece: {pasta:?}", r.nome);
    }
}

/// Põe estes bancos (criados agora) nos recentes, com o aberto no topo, e espera a tela mostrar `total`.
fn mais_recentes(h: &mut H, amb: &Ambiente, nomes: &[&str], total: usize) {
    for nome in nomes {
        let c = amb.pasta().join(nome);
        criar_banco(&c, "CREATE TABLE t (id INTEGER PRIMARY KEY);");
        amb.servico.prefs().adicionar_recente(&c);
    }
    amb.servico.prefs().adicionar_recente(&amb.estado.banco.as_ref().expect("banco").caminho);
    h.state_mut().emitir(estado::Acao::RecarregarPrefs);
    esperar(h, "os recentes", |h| h.state().estado().prefs.recentes.len() == total);
    assentar(h);
}

fn layout_em_janela_estreita(largura: f32, altura: f32, sufixo: &str) {
    let amb = Ambiente::exemplo(&format!("estreita_{sufixo}"));
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(largura, altura), Partida::default(), |_| None);
    assert_eq!(h.state().rota(), &Rota::Banco);

    // o corpo: 8 + a doca da esquerda (258) + 8 e o documento até 8px da borda; a da direita (as Propriedades, 272 +
    // 8), nem encolhidas as duas até 200, deixaria o documento com menos de 560px e some, com o botão desligado e a
    // dica de alargar
    let doc = documento(&h);
    assert_eq!((doc.left(), doc.right()), (8.0 + 258.0 + 8.0, largura - 8.0), "{doc:?}");
    assert!(!h.state().propriedades_a_vista());
    assert!(!botao_ligado(&h, "Painel de propriedades"));
    let dica = ler_dica(&mut h, Role::Button, "Painel de propriedades");
    assert!(dica.contains("Painel de propriedades (alargue a janela para ver)"), "{dica:?}");
    let direita = direita_do_card(&h);
    assert_eq!(direita, largura - 8.0 - 1.0 - 20.0 - 1.0);
    // a barra de ferramentas inteira cabe, com o Painel de propriedades encostado à direita
    let painel = rect_de(&h, Role::Button, "Painel de propriedades");
    assert!(painel.right() <= largura - 8.0 - 8.0 + 0.5, "{painel:?}");
    assert!(botao_da_barra(&h, "Exportar").rect().right() < painel.left());

    // Visão geral: a lista cabe na moldura, sem rolar na horizontal, e as seções das Propriedades vão para o fim do
    // documento, depois da lista, na largura dele
    for b in h.query_all_by_label("Colunas") {
        assert!(b.rect().right() <= direita + 0.5, "{:?}", b.rect());
    }
    let ultima = rect_de(&h, Role::Button, "Tabela com espaço");
    assert!(ultima.right() <= direita, "o nome inteiro à vista: {ultima:?}");
    let arquivo = h.get_by_label("Seção Arquivo").rect();
    let manutencao = h.get_by_label("Seção Manutenção").rect();
    assert!(ultima.bottom() < arquivo.top() && arquivo.top() < manutencao.top(), "{ultima:?} {arquivo:?}");
    assert!(arquivo.left() >= doc.left() && arquivo.right() <= doc.right(), "{arquivo:?}");
    assert_eq!(h.query_all_by_label("Verificar integridade").count(), 1);
    capturar(&mut h, &format!("banco-visao-geral-{sufixo}-claro.png"));
    // rolada até o fim: a Manutenção à vista
    h.key_press(Key::End);
    h.run_steps(4);
    capturar(&mut h, &format!("banco-visao-geral-{sufixo}-fim-claro.png"));
    h.key_press(Key::Home);
    h.run_steps(4);

    // Início: as ações dos recentes cabem no card e a data não quebra (só o exemplo.db nos recentes)
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    conferir_recentes(&h, direita, 30.0);
    capturar(&mut h, &format!("inicio-com-recentes-{sufixo}-claro.png"));

    // com nomes mais longos (o caso de "Remover da li" cortado): a pasta encurta até o próprio padding, e as
    // ações continuam dentro do card
    mais_recentes(&mut h, &amb, &["financeiro.sqlite", "estoque_2026.db"], 3);
    conferir_recentes(&h, direita, if largura < 1000.0 { 0.0 } else { 20.0 });
    capturar(&mut h, &format!("inicio-com-3-recentes-{sufixo}-claro.png"));

    // sem o Explorador, as Propriedades cabem ao lado da Visão geral (o documento passa dos 560px); de volta, somem
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(3);
    assert!(h.state().propriedades_a_vista());
    let doc = documento(&h);
    assert_eq!((doc.left(), doc.right()), (8.0, largura - 8.0 - 272.0 - 8.0), "{doc:?}");
    assert!(doc.width() >= 560.0);
    assert!(ligado(&h, Role::Button, "Painel de propriedades"));
    capturar(&mut h, &format!("casca-{sufixo}-propriedades-claro.png"));
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(3);
    assert!(!h.state().propriedades_a_vista());
}

#[test]
fn layout_em_janela_estreita_960() {
    layout_em_janela_estreita(960.0, 600.0, "960");
}

#[test]
fn layout_em_janela_estreita_990() {
    layout_em_janela_estreita(990.0, 700.0, "990");
}

/// A 1024px (sem o trilho, desde a revisão de UI de 19/10/2026), as duas docas cabem encolhidas, na proporção do que
/// cada uma tem acima de 200, e o documento fica com pelo menos 560px.
#[test]
fn layout_em_janela_de_1024_encolhe_as_docas() {
    let amb = Ambiente::exemplo("estreita_1024");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1024.0, 700.0), Partida::default(), |_| None);
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert!(h.state().propriedades_a_vista());
    assert!(botao_ligado(&h, "Painel de propriedades"));
    let (esquerda, direita) = h.state().larguras_desenhadas().expect("as larguras");
    assert!(esquerda < 258.0 && direita < 272.0 && esquerda >= 200.0 && direita >= 200.0, "{esquerda} {direita}");
    let doc = documento(&h);
    assert_eq!(doc.left(), 8.0 + esquerda + 8.0, "{doc:?}");
    assert_eq!(doc.right(), 1024.0 - 8.0 - direita - 8.0, "{doc:?}");
    assert!(doc.width() >= 560.0, "{doc:?}");
    capturar(&mut h, "casca-1024-docas-claro.png");
}

#[test]
fn recentes_com_o_que_sumiu_cabem_em_1024() {
    let amb = Ambiente::exemplo("estreita_sumiu");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1024.0, 700.0), Partida::default(), |_| None);
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    // o que não existe mais leva a pill "não encontrado" à coluna Tamanho
    mais_recentes(&mut h, &amb, &["arquivo_antigo.db", "financeiro.sqlite", "estoque_2026.db"], 4);
    std::fs::remove_file(amb.pasta().join("arquivo_antigo.db")).expect("apagar o antigo");
    h.state_mut().emitir(estado::Acao::RecarregarPrefs);
    esperar(&mut h, "o antigo some", |h| h.state().estado().prefs.recentes.iter().any(|r| !r.existe));
    assentar(&mut h);
    // o Início não tem Propriedades: o documento vai até 8px da borda
    assert_eq!(documento(&h).right(), 1024.0 - 8.0);
    conferir_recentes(&h, direita_do_card(&h), 0.0);
    capturar(&mut h, "inicio-com-4-recentes-1024-claro.png");
}

/// A cópia de segurança com um caminho comprido: o resultado é o toast ("Cópia salva: x.db · 119 MB", sem a pasta),
/// com "Mostrar na pasta" nele e o caminho inteiro só na dica; nada vai para o painel. O clique na ação revela o
/// arquivo (aqui, o gancho dos testes no lugar do Explorador de Arquivos) e o toast sai.
#[test]
fn copia_de_seguranca_vira_toast_com_a_acao_e_o_caminho_na_dica() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let amb = Ambiente::exemplo("copia_longa");
    let pasta = amb.pasta().join("CopiasDeSegurancaDoFinanceiro2026").join("subpasta_com_nome_comprido");
    std::fs::create_dir_all(&pasta).expect("pasta");
    let destino = pasta.join("copia_de_seguranca_do_exemplo.db");
    let escolhido = destino.clone();
    let reveladas: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let anotadas = Rc::clone(&reveladas);
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(move |p| matches!(p, PedidoDialogo::CopiaDeSeguranca { .. }).then(|| escolhido.clone()))
        .com_abrir_de_teste(move |p| anotadas.borrow_mut().push(p.to_path_buf()));
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    clicar_botao(&mut h, "Fazer cópia de segurança");
    assentar(&mut h);
    assert!(destino.is_file());
    let texto = toast_atual(&h.ctx).expect("o toast da cópia");
    let nome = "copia_de_seguranca_do_exemplo.db";
    assert!(texto.starts_with(&format!("Cópia salva: {nome} · ")), "{texto}");
    assert!(!texto.contains("CopiasDeSeguranca"), "só o nome do arquivo no toast: {texto}");
    // (o serviço devolve o caminho por extenso, e o do teste pode estar na forma curta 8.3 da pasta temporária)
    let dica = interface::pecas::toast::dica_do_toast(&h.ctx).expect("a dica");
    let fim = ["CopiasDeSegurancaDoFinanceiro2026", "subpasta_com_nome_comprido", "copia_de_seguranca_do_exemplo.db"];
    assert!(
        std::path::Path::new(&dica).ends_with(fim.iter().collect::<PathBuf>()),
        "o caminho inteiro na dica: {dica}"
    );
    // nada de caixa no painel: nem o caminho, nem o botão dentro do painel de Propriedades
    assert!(!tem_texto(&h, "Cópia salva em"));
    assert!(!textos_das_propriedades(&h).iter().any(|t| t.contains("Cópia salva") || t == "Mostrar na pasta"));
    // o toast cabe na janela, centrado
    let r = interface::pecas::toast::rect_toast(&h.ctx).expect("onde ficou o toast");
    assert!(r.left() > 0.0 && r.right() < TAMANHO.x, "{r:?}");
    capturar(&mut h, "banco-copia-toast-claro.png");
    // o clique em "Mostrar na pasta" revela o arquivo e o toast sai
    h.get(by().role(Role::Button).label("Mostrar na pasta")).click();
    h.run_steps(3);
    let reveladas = reveladas.borrow();
    assert_eq!(reveladas.len(), 1);
    assert_eq!(reveladas[0].display().to_string(), dica, "o caminho da dica é o que se revela");
    assert!(toast_atual(&h.ctx).is_none(), "cumprida a ação, o toast sai");
}

// ---- sem o diálogo do Windows: a janela "Pedir o caminho" ------------------------------------------------------

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

// ---- trocas de banco ------------------------------------------------------------------------------------------

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

// ---- janela Novo índice da casca ------------------------------------------------------------------------------

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

// ---- as cenas das pranchetas ----------------------------------------------------------------------------------
//
// As capturas do fechamento da etapa 2 montam, com o banco de exemplo, as cenas das pranchetas do canvas, para a
// comparação a olho: o Início com os recentes e o menu Arquivo (prancheta 4), a Visão geral com três abas e o
// Exibir › Tema (1), a aba de Dados com o filtro (2), a Consulta com o Histórico (3), a Estrutura com a transação
// aberta (6) e a janela de 960px. O que as pranchetas têm e a etapa 2 não (a pasta aberta no Explorador, os
// ícones rasterizados, as telas no formato de documento) fica como está. Com os substitutos (sem o recurso
// `telas`), as cenas não fazem sentido e não sobrescrevem as capturas.

/// Passa o ponteiro sobre o canto do banco no Explorador onde fica o ícone de estado (16px, a 10px da direita do nó) e
/// devolve a dica que apareceu; tira o ponteiro no fim.
fn dica_do_estado_do_banco(h: &mut H) -> String {
    h.remove_cursor();
    h.run_steps(6);
    let antes = textos(h);
    let no = rect_de(h, Role::Link, "exemplo.db");
    h.hover_at(egui::pos2(no.right() - 10.0 - 8.0, no.center().y));
    h.run_steps(3);
    let mut depois = textos(h);
    for t in &antes {
        if let Some(i) = depois.iter().position(|d| d == t) {
            depois.remove(i);
        }
    }
    h.remove_cursor();
    h.run_steps(2);
    depois.join("\n")
}

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

// ---- as correções das revisões da etapa 2 ---------------------------------------------------------------------

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

/// Outro programa com o banco preso para gravação (`BEGIN IMMEDIATE` numa conexão à parte): os comandos que gravam
/// esperam por ele (até os 5 s do busy_timeout) até [`OutroPrograma::soltar`]. Deixa uma Consulta executando pelo
/// tempo que o teste quiser, sem depender da velocidade da máquina.
struct OutroPrograma {
    servico: servico::Servico,
    id: String,
    pasta: PathBuf,
}

impl OutroPrograma {
    fn prender(caminho: &std::path::Path) -> OutroPrograma {
        let pasta = caminho.parent().expect("pasta").join(format!("outro-programa-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let servico = servico::Servico::novo(prefs::Preferencias::abrir(Some(pasta.join("p.json"))));
        let id = servico.abrir_banco(caminho, false, false).expect("abrir").id;
        servico.sql(&id, "BEGIN IMMEDIATE;", 10).expect("prender o banco");
        OutroPrograma { servico, id, pasta }
    }

    /// Solta o banco: o comando que esperava segue.
    fn soltar(self) {}
}

impl Drop for OutroPrograma {
    fn drop(&mut self) {
        let _ = self.servico.sql(&self.id, "ROLLBACK;", 10);
        let _ = self.servico.fechar_banco(&self.id, true);
        let _ = std::fs::remove_dir_all(&self.pasta);
    }
}

/// Digita o comando na Consulta à vista e executa: ele fica esperando o [`OutroPrograma`] soltar o banco.
fn executando(h: &mut H, sql: &str) {
    digitar(h, "Comando SQL", sql);
    h.key_press_modifiers(CTRL, Key::Enter);
    h.run_steps(2);
    // o rodapé da Consulta conta o tempo
    esperar_texto(h, "Executando");
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

// ---- busca da barra -------------------------------------------------------------------------------------------

/// O campo da busca da barra pelo rótulo (a dica da aba: "Buscar tabelas", "Buscar em clientes"; "Buscar" desligada).
fn campo_da_busca<'h>(h: &'h H, rotulo: &'h str) -> egui_kittest::Node<'h> {
    h.get(by().role(Role::TextInput).label(rotulo))
}

#[test]
fn ctrl_f_vai_para_a_busca_da_barra() {
    let amb = Ambiente::exemplo("ctrl_f_busca");
    let mut h = harness(&amb);
    // a Visão geral: "Buscar tabelas"
    h.key_press_modifiers(CTRL, Key::F);
    h.run_steps(2);
    assert!(campo_da_busca(&h, "Buscar tabelas").is_focused(), "o Ctrl+F leva o foco à busca da barra");
    h.key_press(Key::Escape);
    h.run_steps(2);
    // o Buscar do menu Editar faz o mesmo
    abrir_menu(&mut h, "Editar");
    clicar_item(&mut h, "Buscar");
    h.run_steps(2);
    assert!(campo_da_busca(&h, "Buscar tabelas").is_focused(), "o Buscar do menu");
    // no Início, sem busca, o Ctrl+F não faz nada e a busca fica desligada
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    h.key_press_modifiers(CTRL, Key::F);
    h.run_steps(2);
    let busca = campo_da_busca(&h, "Buscar");
    assert!(busca.accesskit_node().is_disabled() && !busca.is_focused());
    if cfg!(feature = "telas") {
        // nos Dados, "Buscar em clientes", também com a grade em foco (o texto fica selecionado)
        h.state_mut().ir(Rota::objeto("clientes", false));
        assentar(&mut h);
        h.key_press_modifiers(CTRL, Key::F);
        h.run_steps(2);
        assert!(campo_da_busca(&h, "Buscar em clientes").is_focused());
    }
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn busca_da_barra_filtra_os_dados_e_vai_para_o_endereco() {
    let amb = Ambiente::exemplo("busca_dados");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    digitar(&mut h, "Buscar em clientes", "Recife");
    esperar(&mut h, "a busca no endereço", |h| h.state().rota().dados().is_some_and(|d| d.busca == "Recife"));
    assentar(&mut h);
    assert!(h.state().tabela_aberta().is_some_and(|t| t.rota.dados().is_some_and(|d| d.busca == "Recife")));
    // o Esc com texto limpa e aplica na hora
    campo_da_busca(&h, "Buscar em clientes").focus();
    h.run_steps(1);
    h.key_press(Key::Escape);
    esperar(&mut h, "a busca limpa", |h| h.state().rota().dados().is_some_and(|d| d.busca.is_empty()));
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some(""));
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn cada_aba_guarda_a_propria_busca() {
    let amb = Ambiente::exemplo("busca_por_aba");
    let mut h = harness(&amb);
    // a Visão geral filtra a lista de tabelas pelo nome, sem acento nem caixa
    digitar(&mut h, "Buscar tabelas", "PRODU");
    h.run_steps(2);
    assert!(h.query(by().role(Role::Button).label("produtos")).is_some());
    assert!(h.query(by().role(Role::Button).label("clientes")).is_none(), "{:?}", textos(&h));
    // os Dados têm a busca deles
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some(""));
    digitar(&mut h, "Buscar em clientes", "Recife");
    esperar(&mut h, "a busca no endereço", |h| h.state().rota().dados().is_some_and(|d| d.busca == "Recife"));
    // de volta à Visão geral, a dela continua lá; e a dos Dados também
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assentar(&mut h);
    assert_eq!(campo_da_busca(&h, "Buscar tabelas").value().as_deref(), Some("PRODU"));
    clicar_aba(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some("Recife"));
    // a vista Colunas das Propriedades não troca a busca da barra: continua a dos Dados
    vista_da_tabela(&mut h, 2);
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some("Recife"));
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn busca_desligada_no_inicio_e_ligada_na_tabela_em_qualquer_vista() {
    let amb = Ambiente::exemplo("busca_desligada");
    let mut h = harness(&amb);
    let ligada = |h: &mut H, rotulo: &str| {
        let desligada = campo_da_busca(h, rotulo).accesskit_node().is_disabled();
        abrir_menu(h, "Editar");
        let no_menu = item_ligado(h, "Buscar");
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert_eq!(!desligada, no_menu, "a barra e o menu dizem o mesmo");
        !desligada
    };
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    assert!(!ligada(&mut h, "Buscar"), "a busca desligada no Início");
    // a tabela é sempre a grade dos Dados (revisão de UI de 19/10/2026): a busca fica, em qualquer vista das
    // Propriedades
    for (rota, rotulo) in [
        (Rota::objeto_na_aba("v_pedidos_por_cliente", true, Aba::Definicao), "Buscar em v_pedidos_por_cliente"),
        (Rota::objeto_na_aba("clientes", false, Aba::Indices), "Buscar em clientes"),
    ] {
        h.state_mut().ir(rota.clone());
        assentar(&mut h);
        assert!(ligada(&mut h, rotulo), "a busca ligada em {rota:?}");
    }
}

// ---- os ícones em PNG na casca (etapa 3, passo 8) ----------------------------------------------------------------

/// Onde o PNG do ícone de `lado` pontos foi pintado dentro de `dentro` no último quadro (a malha com a textura dele).
fn icone_pintado_em(h: &H, icone: interface::icones::Icone, lado: f32, dentro: egui::Rect) -> Option<egui::Rect> {
    fn juntar(forma: &egui::Shape, textura: egui::TextureId, achados: &mut Vec<egui::Rect>) {
        match forma {
            egui::Shape::Vec(formas) => formas.iter().for_each(|f| juntar(f, textura, achados)),
            egui::Shape::Mesh(m) if m.texture_id == textura => achados.push(m.calc_bounds()),
            _ => {}
        }
    }
    let textura = interface::icones::textura_carregada(&h.ctx, icone, lado)?;
    let mut achados = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, textura, &mut achados);
    }
    achados.into_iter().find(|r| dentro.expand(0.5).contains_rect(*r))
}

#[test]
fn docas_com_os_solidos_e_o_explorador_on() {
    use interface::icones::Icone;
    let amb = Ambiente::exemplo("docas_solidos");
    let mut h = harness(&amb);
    assentar(&mut h);
    assert!(painel_a_vista(&h, "Explorador"), "o Explorador à vista, a aba ativa da doca");
    assert!(!painel_a_vista(&h, "Histórico"));
    // a doca da esquerda (revisão de UI de 19/10/2026): as abas só ícone (`.aba.so-ic`, 38 × 34) com os sólidos de 16
    for (icone, rotulo) in [(Icone::Explorador, "Explorador"), (Icone::Historico, "Histórico")] {
        let aba = rect_de(&h, Role::Tab, rotulo);
        assert_eq!(aba.size(), egui::vec2(38.0, 34.0), "{rotulo}");
        let r = icone_pintado_em(&h, icone, 16.0, aba).unwrap_or_else(|| panic!("o sólido de 16 em {rotulo}"));
        assert!((r.center() - aba.center()).length() <= 0.75, "{rotulo}: {r:?} em {aba:?}");
    }
    assert!(aba_do_painel_ativa(&h, "Explorador") && !aba_do_painel_ativa(&h, "Histórico"));
    // a da direita, com o nome e o objeto
    assert!(aba_do_painel_ativa(&h, "Propriedades · exemplo.db"));
    // o Explorador pelo tipo do objeto: o banco, a tabela sem rowid (produtos) e a virtual (notas)
    let explorador = h.get(by().role(Role::Link).label("produtos")).rect();
    assert!(icone_pintado_em(&h, Icone::TabelaSemRowid, 16.0, explorador.expand2(egui::vec2(60.0, 0.0))).is_some());
    let notas = h.get(by().role(Role::Link).label("notas")).rect();
    assert!(icone_pintado_em(&h, Icone::TabelaVirtual, 16.0, notas.expand2(egui::vec2(60.0, 0.0))).is_some());
    assert!(interface::icones::textura_carregada(&h.ctx, Icone::Banco, 16.0).is_some());
    // a aba da tabela sem rowid leva a mesma ficha
    clicar_link(&mut h, "produtos");
    assentar(&mut h);
    let aba = h.query_all(by().role(Role::Tab)).find(|n| n.accesskit_node().label().is_some_and(|l| l == "produtos"));
    let aba = aba.expect("a aba de produtos").rect();
    assert!(icone_pintado_em(&h, Icone::TabelaSemRowid, 16.0, aba).is_some(), "a ficha anil na aba");
}

#[test]
fn menu_arquivo_com_icones_de_24() {
    use interface::icones::Icone;
    let amb = Ambiente::exemplo("menu_icones");
    let estoque = amb.pasta().join("estoque_2026.db");
    criar_banco(&estoque, "CREATE TABLE itens (id INTEGER PRIMARY KEY);");
    amb.servico.prefs().adicionar_recente(&estoque);
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    // `.menu-ic`: as linhas de 36 e os sólidos de 24 com `padding-left:8px`
    for rotulo in ["Abrir pasta", "Abrir arquivo", "Novo banco", "Abertos recentemente", "Salvar", "Fechar aba"] {
        assert_eq!(item_de_menu(&h, rotulo).rect().height(), 36.0, "{rotulo}");
    }
    for (icone, rotulo) in [
        (Icone::PastaSolida, "Abrir pasta"),
        (Icone::DocumentoSolido, "Abrir arquivo"),
        (Icone::NovoBancoSolido, "Novo banco"),
        (Icone::Recentes, "Abertos recentemente"),
    ] {
        let item = item_de_menu(&h, rotulo).rect();
        let r = icone_pintado_em(&h, icone, 24.0, item).unwrap_or_else(|| panic!("o sólido de 24 em {rotulo}"));
        assert_eq!((r.width(), r.left() - item.left()), (24.0, 8.0), "{rotulo}");
    }
    // o desligado (Salvar) em cinza: outra textura, a de cor nem é carregada
    assert!(interface::icones::textura_carregada(&h.ctx, Icone::DisqueteSolido, 24.0).is_none());
    // os recentes: o cilindro de 24 em linhas de 44
    abrir_submenu(&mut h, "Abertos recentemente");
    let recente = item_de_menu(&h, "estoque_2026.db").rect();
    assert_eq!(recente.height(), 44.0);
    let r = icone_pintado_em(&h, Icone::Banco, 24.0, recente).expect("o cilindro de 24 no recente");
    assert_eq!(r.left() - recente.left(), 8.0);
    capturar(&mut h, "menu-arquivo-icones-claro.png");
}

// ---- as Preferências (etapa 3, passo 18) ---------------------------------------------------------------------------

/// Arquivo › Preferências.
fn abrir_preferencias(h: &mut H) {
    abrir_menu_arquivo(h);
    clicar_item(h, "Preferências");
    h.run_steps(3);
    // as famílias de fonte do código chegam de uma tarefa (a thread da interface não toca no disco)
    assentar(h);
}

fn preferencias_abertas(h: &H) -> bool {
    h.state().preferencias_aberta().is_some()
}

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

// ---- textos e acessibilidade que cruzam as telas (etapa 3, passo 20) -------------------------------------------

/// Os botões da árvore de acessibilidade cujo nome termina em reticências (o "…" ou os três pontos): o nome de um
/// botão diz o que ele faz; a reticência de "Abrir arquivo…" prometia um diálogo que o clique já abre sem ela.
fn botoes_com_reticencias(h: &H) -> Vec<String> {
    h.query_all(by().role(Role::Button))
        .filter_map(|n| n.accesskit_node().label())
        .filter(|rotulo| {
            let rotulo = rotulo.trim_end();
            rotulo.ends_with('…') || rotulo.ends_with("...")
        })
        .collect()
}

/// Falha com o lugar e os botões, se algum botão à vista termina em reticências.
fn sem_reticencias(h: &H, onde: &str) {
    let achados = botoes_com_reticencias(h);
    assert!(achados.is_empty(), "botões com reticências {onde}: {achados:?}");
}

/// A frase do aviso de transação aberta e da dica do modo na barra de status, com o nome da Consulta.
fn frase_da_transacao(consulta: &str) -> String {
    format!(
        "Há uma transação aberta na {consulta}. O que for salvo agora só vai para o arquivo depois do COMMIT; fechar o \
         banco descarta tudo desde o BEGIN."
    )
}

/// Fecha a janela aberta com o Esc.
fn fechar_janela(h: &mut H) {
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa das telas de verdade")]
fn nenhum_botao_com_reticencias() {
    let amb = Ambiente::exemplo("sem_reticencias");
    let mut h = harness(&amb);
    // o Início, com banco aberto e sem
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    sem_reticencias(&h, "no Início");
    for rotulo in ["Abrir arquivo", "Novo banco"] {
        assert!(existe(&h, Role::Button, rotulo), "{rotulo} no Início: {:?}", textos(&h));
    }
    // a Visão geral e as quatro vistas de uma tabela
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    sem_reticencias(&h, "na Visão geral");
    for (parte, nome) in [
        (Aba::Dados, "Dados"),
        (Aba::Estrutura, "Estrutura"),
        (Aba::Indices, "Índices e gatilhos"),
        (Aba::Definicao, "Definição SQL"),
    ] {
        h.state_mut().ir(Rota::objeto_na_aba("clientes", false, parte));
        assentar(&mut h);
        sem_reticencias(&h, &format!("na vista {nome}"));
    }
    // a Consulta SQL, com um resultado e o Histórico
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    executar(&mut h, "SELECT id, nome FROM clientes LIMIT 3");
    sem_reticencias(&h, "na Consulta SQL");
    // os menus
    for menu in ["Arquivo", "Editar", "Exibir", "Banco", "Ajuda"] {
        abrir_menu(&mut h, menu);
        sem_reticencias(&h, &format!("no menu {menu}"));
        h.key_press(Key::Escape);
        h.run_steps(3);
    }
    // as janelas: a barra (Exportar, Nova linha, Nova coluna), a Nova tabela, os Atalhos, o Sobre e as Preferências
    h.state_mut().ir(Rota::objeto_na_aba("clientes", false, Aba::Dados));
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    assentar(&mut h);
    assert!(h.state().janela_aberta(), "a janela Exportar");
    sem_reticencias(&h, "na janela Exportar");
    fechar_janela(&mut h);
    // o menu Tabela (com a tabela à vista) e a Nova coluna dele: um diálogo da Estrutura, nas Propriedades (não conta
    // em `janela_aberta`)
    abrir_menu(&mut h, "Tabela");
    sem_reticencias(&h, "no menu Tabela");
    clicar_item(&mut h, "Nova coluna");
    esperar(&mut h, "o diálogo Nova coluna", |h| existe(h, Role::Dialog, "Nova coluna"));
    sem_reticencias(&h, "na janela Nova coluna");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!existe(&h, Role::Dialog, "Nova coluna"));
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova tabela");
    assentar(&mut h);
    assert!(h.state().janela_aberta(), "a janela Nova tabela");
    sem_reticencias(&h, "na janela Nova tabela");
    fechar_janela(&mut h);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    assert!(h.state().janela_aberta());
    sem_reticencias(&h, "na janela Atalhos do teclado");
    fechar_janela(&mut h);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Sobre o DataBox");
    assert!(h.state().janela_aberta());
    sem_reticencias(&h, "na janela Sobre");
    clicar_botao(&mut h, "Licenças de terceiros");
    sem_reticencias(&h, "na janela Licenças de terceiros");
    h.key_press(Key::Escape);
    h.run_steps(3);
    if h.state().janela_aberta() {
        fechar_janela(&mut h);
    }
    abrir_preferencias(&mut h);
    assert!(preferencias_abertas(&h));
    sem_reticencias(&h, "nas Preferências");
    fechar_janela(&mut h);
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn aviso_de_transacao_cita_a_consulta() {
    let amb = Ambiente::exemplo("aviso_cita_consulta");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    // a transação abre na Consulta 2 (a que está à vista)
    executar(&mut h, "BEGIN; UPDATE clientes SET cidade = 'Teresina' WHERE id = 1;");
    esperar(&mut h, "a transação aberta", |h| h.state().estado().transacao_aberta());
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta 2."), "{:?}", textos(&h));
    assert!(!tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    assert!(existe(&h, Role::Link, "Ir para a Consulta 2"));
    assert!(!existe(&h, Role::Link, "Ir para a Consulta SQL"));
    clicar_link(&mut h, "Ir para a Consulta 2");
    h.run_steps(2);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 2", "o link foca a Consulta da transação");
}

#[test]
fn aviso_de_transacao_sem_consulta_diz_consulta_sql() {
    let amb = Ambiente::exemplo("aviso_sem_consulta");
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    assert!(existe(&h, Role::Link, "Ir para a Consulta SQL"));
}

#[test]
fn dica_do_status_igual_ao_aviso() {
    let amb = Ambiente::exemplo("dica_igual_ao_aviso");
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    // o aviso: o começo em destaque e o resto; a dica do modo é as duas partes numa frase só
    let frase = frase_da_transacao("Consulta SQL");
    let (inicio, resto) = frase.split_once(". ").expect("duas partes");
    assert!(tem_texto(&h, &format!("{inicio}.")) && tem_texto(&h, resto), "{:?}", textos(&h));
    assert!(ler_dica(&mut h, Role::Label, "Transação aberta").contains(&frase));
}

#[test]
fn explorador_sem_ponto_verde_e_com_atencao_na_transacao() {
    let amb = Ambiente::exemplo("explorador_atencao");
    let id = amb.estado.banco_id();
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    // aberto sem transação: nenhum ponto nem ícone de estado no nó do banco (o "Aberto" saiu)
    assert!(!existe(&h, Role::Image, "Transação aberta"));
    assert!(!dica_do_estado_do_banco(&mut h).contains("Aberto"));
    // com a transação aberta: o ícone Atenção de 16px, com o rótulo "Transação aberta" e, na dica, o aviso inteiro
    amb.servico.sql(&id, "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(estado::Acao::NotarTransacao(true));
    assentar(&mut h);
    let no = rect_de(&h, Role::Link, "exemplo.db");
    assert!(no.contains_rect(rect_de(&h, Role::Image, "Transação aberta")), "o ícone fica dentro do nó do banco");
    let atencao = interface::icones::Icone::Estado(interface::icones::EstadoIcone::Atencao);
    let pintado = icone_pintado_em(&h, atencao, 16.0, no).expect("o ícone Atenção pintado no nó do banco");
    assert_eq!((pintado.width(), pintado.height()), (16.0, 16.0));
    assert_eq!(dica_do_estado_do_banco(&mut h), frase_da_transacao("Consulta SQL"));
    // o COMMIT tira o ícone
    amb.servico.sql(&id, "COMMIT;", 10).expect("commit");
    h.state_mut().emitir(estado::Acao::NotarTransacao(false));
    assentar(&mut h);
    assert!(!existe(&h, Role::Image, "Transação aberta"));
}

#[test]
fn botao_marcado_diz_que_esta_pressionado() {
    use egui::accesskit::Toggled;
    let amb = Ambiente::exemplo("botao_pressionado");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    let estado_de = |h: &H, rotulo: &str| h.get(by().role(Role::Button).label(rotulo)).accesskit_node().toggled();
    // o botão Painel de propriedades da barra de ferramentas (`.ib.on`): pressionado com as Propriedades à vista
    assert_eq!(estado_de(&h, "Painel de propriedades"), Some(Toggled::True));
    clicar_botao(&mut h, "Painel de propriedades");
    assert_eq!(estado_de(&h, "Painel de propriedades"), Some(Toggled::False), "escondido: não pressionado");
    clicar_botao(&mut h, "Painel de propriedades");
    assert_eq!(estado_de(&h, "Painel de propriedades"), Some(Toggled::True));
}

#[test]
fn abas_das_docas_dizem_qual_esta_a_vista() {
    let amb = Ambiente::exemplo("docas_a_vista");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    // a Visão geral à vista, com o Explorador: a aba dele é a escolhida; a do Histórico não
    assert!(aba_do_painel_ativa(&h, "Explorador") && !aba_do_painel_ativa(&h, "Histórico"));
    // o clique na aba do Histórico o põe à vista na mesma doca
    clicar_aba_do_painel(&mut h, "Histórico");
    assentar(&mut h);
    assert!(aba_do_painel_ativa(&h, "Histórico") && !aba_do_painel_ativa(&h, "Explorador"));
    assert!(painel_a_vista(&h, "Histórico") && !painel_a_vista(&h, "Explorador"));
    // pelo Exibir, de volta ao Explorador
    exibir(&mut h, "Explorador");
    assert!(aba_do_painel_ativa(&h, "Explorador"));
}
