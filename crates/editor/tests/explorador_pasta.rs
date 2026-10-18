//! O Explorador com a pasta (etapa 4, prancheta 1, a 4 sem nada e a 15 com o menu de contexto): a raiz e as
//! pastas que abrem sob demanda, o banco aberto no lugar dele e a segunda raiz, os arquivos de dados, o "mais N
//! itens", o carregando, a falha e o vazio, o menu de contexto de cada tipo, o teclado (as setas, o Shift+F10 e o
//! Ctrl+Shift+C), o Recolher tudo e o Explorador vazio. A pasta entra no estado pelo `Editor::simular_pasta`, montada
//! à mão com `Listagem::montar` (sem a varredura do disco), e o que o Explorador pede sai no
//! `Editor::acoes_do_explorador`; a casca também cumpre o pedido (passo 8: lê a pasta de verdade), por isso os testes
//! que trocam um nó à mão esperam a leitura dela terminar antes.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use apoio::*;
use comum::arquivos::{Entrada, LIMITE_POR_PASTA, Listagem, Natureza, TipoDeArquivo};
use editor::Editor;
use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Acao, NoDaPasta, PastaAberta, Rota};
use interface::icones::{Icone, TipoArquivo};
use interface::pecas::ajuda::IconeEstado;
use interface::pecas::toast::{icone_toast, toast_atual};
use interface::tema::Tema;

/// O caminho como o serviço guarda o do banco aberto: absoluto, com os nomes curtos (8.3) resolvidos (a pasta
/// temporária vem como `C:\Users\CONCEI~1\…`), sem o `\\?\`.
fn real(caminho: &Path) -> PathBuf {
    let c = std::fs::canonicalize(caminho).expect("canonicalize");
    let texto = c.to_string_lossy().into_owned();
    PathBuf::from(texto.strip_prefix(r"\\?\").unwrap_or(&texto))
}

/// A pasta de dados do ambiente, pelo caminho real.
fn dados(amb: &Ambiente) -> PathBuf {
    real(&amb.pasta_de_dados())
}

fn entrada(pasta: &Path, nome: &str, natureza: Natureza) -> Entrada {
    let caminho = pasta.join(nome);
    Entrada {
        nome: nome.to_string(),
        chave: prefs::chave_caminho(&caminho),
        caminho,
        natureza,
        tamanho: None,
        modificado: None,
    }
}

fn chave(caminho: &Path) -> String {
    prefs::chave_caminho(caminho)
}

/// Uma pasta lida, com as entradas e as ocultas.
fn pronta(pasta: &Path, entradas: Vec<Entrada>, ocultas: usize) -> NoDaPasta {
    pronta_com_limite(pasta, entradas, ocultas, LIMITE_POR_PASTA)
}

fn pronta_com_limite(pasta: &Path, entradas: Vec<Entrada>, ocultas: usize, limite: usize) -> NoDaPasta {
    NoDaPasta::Pronto(Arc::new(Listagem::montar(pasta.to_path_buf(), entradas, ocultas, false, limite)))
}

/// A pasta de dados do `Ambiente::com_pasta` como a varredura a leria: a subpasta `sub`, os quatro bancos e os quatro
/// arquivos de dados; o `Thumbs.db` e o `relatorio.pdf` ficam de fora (as duas ocultas). As subpastas ainda não foram
/// lidas.
fn pasta_de_dados(raiz: &Path) -> PastaAberta {
    let mut p = PastaAberta::nova(raiz.to_path_buf());
    let entradas = vec![
        entrada(raiz, "sub", Natureza::Pasta),
        entrada(raiz, "exemplo.db", Natureza::Banco),
        entrada(raiz, "estoque_2026.db", Natureza::Banco),
        entrada(raiz, "sem_extensao", Natureza::Banco),
        entrada(raiz, "copia.dat", Natureza::Banco),
        entrada(raiz, "clientes.csv", Natureza::Dados(TipoDeArquivo::Csv)),
        entrada(raiz, "planilha.xlsx", Natureza::Dados(TipoDeArquivo::Xlsx)),
        entrada(raiz, "dados.json", Natureza::Dados(TipoDeArquivo::Json)),
        entrada(raiz, "notas.txt", Natureza::Dados(TipoDeArquivo::Texto)),
    ];
    p.nos.insert(p.chave.clone(), pronta(raiz, entradas, 2));
    p
}

/// Põe a pasta no estado do editor (como a casca faria) e desenha.
fn simular(h: &mut H, pasta: Option<PastaAberta>) {
    let ctx = h.ctx.clone();
    h.state_mut().simular_pasta(&ctx, pasta);
    h.run_steps(2);
}

/// Troca o que a árvore sabe de uma pasta (a leitura que chegou).
fn com_no(h: &mut H, pasta: &Path, no: NoDaPasta) {
    let mut p = h.state().estado().pasta.clone().expect("a pasta aberta");
    p.nos.insert(chave(pasta), no);
    simular(h, Some(p));
}

/// O ambiente com a pasta de dados e o `exemplo.db` de dentro dela aberto.
fn ambiente_com_banco_na_pasta(nome: &str, itens: &[&str]) -> Ambiente {
    let mut amb = Ambiente::com_pasta(nome, itens);
    amb.servico.abrir_banco(&amb.pasta_de_dados().join("exemplo.db"), false, false).expect("abrir o exemplo.db");
    amb.recarregar();
    amb
}

/// As ações que o Explorador emitiu desde a última vez.
fn acoes(h: &mut H) -> Vec<Acao> {
    h.state_mut().acoes_do_explorador()
}

fn listar(caminho: &Path) -> Acao {
    Acao::ListarPasta { caminho: caminho.to_path_buf() }
}

/// Aperta os modificadores e manda o evento (a tecla C ou o `Event::Copy`), e devolve o texto copiado no quadro dele.
/// O harness roda um quadro por evento e guarda só a saída do último: o evento vai no fim, e os modificadores soltam
/// depois.
fn copiar_com(h: &mut H, modificadores: Modifiers, evento: egui::Event) -> Option<String> {
    h.event(egui::Event::ModifiersChanged(modificadores));
    h.event(evento);
    h.step();
    let texto = h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    });
    h.event(egui::Event::ModifiersChanged(Modifiers::default()));
    h.run_steps(2);
    texto
}

/// A tecla C apertada com estes modificadores.
fn tecla_c(modificadores: Modifiers) -> egui::Event {
    egui::Event::Key { key: Key::C, physical_key: None, pressed: true, repeat: false, modifiers: modificadores }
}

/// O texto copiado nos próximos quadros (o clique que copia já na fila).
fn copiado(h: &mut H) -> Option<String> {
    let mut texto = None;
    for _ in 0..3 {
        h.step();
        texto = texto.or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
        });
    }
    texto
}

/// Onde um ícone foi pintado, dentro de `dentro`, pelo PNG dele (como em casca.rs).
fn icone_pintado_em(h: &H, icone: Icone, lado: f32, dentro: egui::Rect) -> Option<egui::Rect> {
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

/// O recuo do nó (o `padding-left`): onde o ícone de 16 dele começa, menos a seta e o `gap` (16px).
fn recuo_do_no(h: &H, rotulo: &str, icone: Icone) -> f32 {
    let no = rect_de(h, Role::Link, rotulo);
    let r = icone_pintado_em(h, icone, 16.0, no).unwrap_or_else(|| panic!("o ícone {icone:?} em {rotulo}"));
    assert_eq!(r.width(), 16.0, "{rotulo}");
    r.left() - no.left() - 16.0
}

/// O rótulo do nó com o foco.
fn focado(h: &H) -> Option<String> {
    h.query_all_by(|n| n.is_focused()).next().and_then(|n| n.accesskit_node().label())
}

fn focar(h: &mut H, rotulo: &str) {
    h.get(by().role(Role::Link).label(rotulo)).focus();
    h.run_steps(2);
}

/// O menu de contexto com este nome está aberto.
fn menu_aberto_de(h: &H, nome: &str) -> bool {
    h.query(by().role(Role::Menu).label(nome)).is_some()
}

/// Os itens do menu de contexto aberto, de cima para baixo.
fn itens_do_menu(h: &H) -> Vec<String> {
    let mut itens: Vec<(f32, String)> = ["Abrir", "Atualizar", "Mostrar na pasta", "Copiar caminho", "Fechar pasta"]
        .iter()
        .filter_map(|r| {
            h.query_all_by_label(r)
                .find(|n| n.accesskit_node().role() == Role::Button && n.rect().width() > 150.0)
                .map(|n| (n.rect().top(), r.to_string()))
        })
        .collect();
    itens.sort_by(|a, b| a.0.total_cmp(&b.0));
    itens.into_iter().map(|(_, r)| r).collect()
}

/// A pasta de dados no editor, com o banco de dentro dela aberto (prancheta 1).
fn harness_da_pasta(amb: &Ambiente, tema: Tema) -> H {
    let mut h = harness_com(amb, tema, TAMANHO, editor::Partida::default(), |_| None);
    // a leitura de verdade da partida termina antes: chegando depois, ela trocaria a árvore montada à mão
    assentar(&mut h);
    simular(&mut h, Some(pasta_de_dados(&dados(amb))));
    acoes(&mut h);
    h
}

#[test]
fn arvore_da_pasta_com_banco_aberto_no_lugar() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_arvore", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let raiz = dados(&amb);
    assert_eq!(h.state().rota(), &Rota::Banco);
    // a raiz (aberta), o banco aberto no lugar dele, com as Tabelas, e os outros nós em ordem: as pastas, os bancos e
    // os arquivos de dados
    let ordem = [
        "dados, pasta",
        "sub, pasta",
        "copia.dat",
        "estoque_2026.db",
        "exemplo.db",
        "Tabelas",
        "clientes",
        "Visões",
        "Sistema",
        "sem_extensao",
        "clientes.csv",
        "dados.json",
        "notas.txt",
        "planilha.xlsx",
    ];
    let tops: Vec<f32> = ordem.iter().map(|r| rect_de(&h, Role::Link, r).top()).collect();
    assert!(tops.windows(2).all(|t| t[0] < t[1]), "{:?}", ordem.iter().zip(&tops).collect::<Vec<_>>());
    // o banco aberto é um só (nada de segunda raiz) e fica marcado com a Visão geral
    assert_eq!(h.query_all(by().role(Role::Link).label("exemplo.db")).count(), 1);
    assert!(ligado(&h, Role::Link, "exemplo.db"));
    // os níveis da prancheta 1: a pasta em 8, o banco em 20, os grupos em 34 e as tabelas em 48; os ícones por tipo
    assert_eq!(recuo_do_no(&h, "dados, pasta", Icone::PastaSolida), 8.0);
    assert_eq!(recuo_do_no(&h, "sub, pasta", Icone::PastaSolida), 20.0);
    assert_eq!(recuo_do_no(&h, "exemplo.db", Icone::Banco), 20.0);
    assert_eq!(recuo_do_no(&h, "estoque_2026.db", Icone::Banco), 20.0);
    assert_eq!(recuo_do_no(&h, "clientes", Icone::Tabela), 48.0);
    assert_eq!(recuo_do_no(&h, "clientes.csv", Icone::Folha(TipoArquivo::Csv)), 20.0);
    assert_eq!(recuo_do_no(&h, "planilha.xlsx", Icone::Folha(TipoArquivo::Xlsx)), 20.0);
    assert_eq!(recuo_do_no(&h, "dados.json", Icone::Folha(TipoArquivo::Json)), 20.0);
    assert_eq!(recuo_do_no(&h, "notas.txt", Icone::Folha(TipoArquivo::Txt)), 20.0);
    // a pasta e a seta: `expanded`; o arquivo de dados não abre
    let expandido = |h: &H, r: &str| h.get(by().role(Role::Link).label(r)).accesskit_node().data().is_expanded();
    assert_eq!(expandido(&h, "dados, pasta"), Some(true));
    assert_eq!(expandido(&h, "sub, pasta"), Some(false));
    assert_eq!(expandido(&h, "estoque_2026.db"), Some(false));
    assert_eq!(expandido(&h, "clientes.csv"), None);
    // a dica da pasta: o caminho e os arquivos que a árvore esconde; a do arquivo, o caminho
    let dica = ler_dica(&mut h, Role::Link, "dados, pasta");
    assert!(dica.contains(&raiz.display().to_string()), "{dica:?}");
    assert!(dica.contains("2 arquivos de outros tipos não aparecem."), "{dica:?}");
    let dica = ler_dica(&mut h, Role::Link, "clientes.csv");
    assert_eq!(dica, raiz.join("clientes.csv").display().to_string());
    // parado, nada foi pedido (a raiz já estava lida)
    assert!(acoes(&mut h).is_empty());
}

/// As capturas da prancheta 1: a pasta `dados` com o banco aberto e os arquivos de dados.
#[test]
fn capturas_do_explorador_com_a_pasta() {
    let itens = ["vendas_2025.csv", "relatorio_q3.xlsx", "config.json", "leia-me.txt"];
    for (tema, nome) in [(Tema::Claro, "explorador-pasta-claro.png"), (Tema::Escuro, "explorador-pasta-escuro.png")] {
        let amb = ambiente_com_banco_na_pasta("explorador_pasta_captura", &itens);
        let mut h = harness_com(&amb, tema, TAMANHO, editor::Partida::default(), |_| None);
        let raiz = dados(&amb);
        let mut p = PastaAberta::nova(raiz.clone());
        let entradas = vec![
            entrada(&raiz, "exemplo.db", Natureza::Banco),
            entrada(&raiz, "estoque_2026.db", Natureza::Banco),
            entrada(&raiz, "vendas_2025.csv", Natureza::Dados(TipoDeArquivo::Csv)),
            entrada(&raiz, "relatorio_q3.xlsx", Natureza::Dados(TipoDeArquivo::Xlsx)),
            entrada(&raiz, "config.json", Natureza::Dados(TipoDeArquivo::Json)),
            entrada(&raiz, "leia-me.txt", Natureza::Dados(TipoDeArquivo::Texto)),
        ];
        p.nos.insert(p.chave.clone(), pronta(&raiz, entradas, 0));
        simular(&mut h, Some(p));
        assentar(&mut h);
        assert!(existe(&h, Role::Link, "leia-me.txt") && existe(&h, Role::Link, "clientes"));
        capturar(&mut h, nome);
    }
}

#[test]
fn clique_na_pasta_abre_e_lista_uma_vez() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_lista", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let sub = dados(&amb).join("sub");
    // a primeira vez que abre, pede a leitura e mostra o carregando
    clicar_link(&mut h, "sub, pasta");
    assert_eq!(acoes(&mut h), [listar(&sub)]);
    // (a leitura de verdade da casca pode já ter chegado)
    assert!(tem_texto(&h, "Carregando…") || existe(&h, Role::Link, "interno.csv"));
    // fechar e abrir de novo antes de a leitura chegar não pede outra
    clicar_link(&mut h, "sub, pasta");
    clicar_link(&mut h, "sub, pasta");
    assert!(acoes(&mut h).is_empty());
    assentar(&mut h);
    // a casca marca a leitura e depois a entrega
    com_no(&mut h, &sub, NoDaPasta::Lendo);
    assert!(tem_texto(&h, "Carregando…"));
    com_no(&mut h, &sub, pronta(&sub, vec![entrada(&sub, "interno.csv", Natureza::Dados(TipoDeArquivo::Csv))], 0));
    assert!(existe(&h, Role::Link, "interno.csv") && !tem_texto(&h, "Carregando…"));
    assert_eq!(recuo_do_no(&h, "interno.csv", Icone::Folha(TipoArquivo::Csv)), 34.0, "um nível abaixo");
    // já lida: fechar e abrir não lê de novo
    clicar_link(&mut h, "sub, pasta");
    assert!(!existe(&h, Role::Link, "interno.csv"));
    clicar_link(&mut h, "sub, pasta");
    assert!(existe(&h, Role::Link, "interno.csv"));
    assert!(acoes(&mut h).is_empty(), "lida uma vez só");
    // a raiz também fecha e abre pelo clique
    clicar_link(&mut h, "dados, pasta");
    assert!(!existe(&h, Role::Link, "clientes.csv") && !existe(&h, Role::Link, "exemplo.db"));
    clicar_link(&mut h, "dados, pasta");
    assert!(existe(&h, Role::Link, "clientes.csv"));
}

#[test]
fn banco_fechado_abre_pelo_clique() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_abre_banco", &[]);
    let raiz = dados(&amb);
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, editor::Partida::default(), |_| None);
    let mut p = pasta_de_dados(&raiz);
    // o Thumbs.db como se a varredura o tomasse por banco (tem a extensão e não é SQLite): o erro vai para o toast
    let mut entradas = p.listagem(&p.chave).expect("a raiz").entradas.clone();
    entradas.push(entrada(&raiz, "Thumbs.db", Natureza::Banco));
    p.nos.insert(p.chave.clone(), pronta(&raiz, entradas, 1));
    simular(&mut h, Some(p.clone()));
    acoes(&mut h);
    // com a transação aberta: a pergunta de hoje; desistir deixa o banco como estava
    clicar_link(&mut h, "estoque_2026.db");
    assert_eq!(acoes(&mut h), [Acao::AbrirBanco { caminho: raiz.join("estoque_2026.db"), somente_leitura: false }]);
    esperar_texto(&mut h, "Abrir outro banco descarta a transação aberta");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert_eq!(h.state().estado().banco.as_ref().map(|b| b.nome.as_str()), Some("exemplo.db"));
    // de novo, e descartando: o banco troca e fica no lugar dele na árvore (a pasta continua)
    clicar_link(&mut h, "estoque_2026.db");
    esperar_texto(&mut h, "Abrir outro banco descarta a transação aberta");
    clicar_botao(&mut h, "Descartar e continuar");
    esperar(&mut h, "o estoque abrir", |h| {
        h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "estoque_2026.db")
    });
    assentar(&mut h);
    assert!(h.state().estado().tem_pasta(), "a pasta não é do banco");
    assert!(existe(&h, Role::Link, "Tabelas") && ligado(&h, Role::Link, "estoque_2026.db"));
    assert_eq!(h.query_all(by().role(Role::Link).label("estoque_2026.db")).count(), 1);
    assert_eq!(recuo_do_no(&h, "estoque_2026.db", Icone::Banco), 20.0);
    // o que não é banco: o erro no toast, e o aberto continua (a troca releu a pasta do disco, onde o Thumbs.db fica
    // de fora: a árvore montada à mão volta)
    simular(&mut h, Some(p));
    acoes(&mut h);
    clicar_link(&mut h, "Thumbs.db");
    esperar(&mut h, "o toast do erro", |h| icone_toast(&h.ctx) == Some(IconeEstado::Erro));
    assert!(toast_atual(&h.ctx).is_some());
    assert_eq!(h.state().estado().banco.as_ref().map(|b| b.nome.as_str()), Some("estoque_2026.db"));
}

#[test]
fn clique_no_arquivo_navega_para_a_ficha() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_arquivo", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let csv = dados(&amb).join("clientes.csv");
    clicar_link(&mut h, "clientes.csv");
    assert_eq!(acoes(&mut h), [Acao::AbrirArquivoDeDados { caminho: csv.clone() }]);
    // Enter com o foco no nó é o clique
    focar(&mut h, "notas.txt");
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(acoes(&mut h), [Acao::AbrirArquivoDeDados { caminho: dados(&amb).join("notas.txt") }]);
    // com a ficha à vista (a rota, que o passo 10 desenha), o nó dela fica marcado e o banco não
    // (antes, o notas.txt termina de abrir: o registro dele numa tarefa navega até ele quando volta)
    let notas = Rota::arquivo(dados(&amb).join("notas.txt"));
    esperar(&mut h, "o notas.txt aberto", move |h| *h.state().rota() == notas && !h.state().ocupado());
    h.state_mut().ir(Rota::arquivo(csv));
    h.run_steps(3);
    assert!(ligado(&h, Role::Link, "clientes.csv"));
    assert!(!ligado(&h, Role::Link, "notas.txt") && !ligado(&h, Role::Link, "exemplo.db"));
}

#[test]
fn nos_com_o_mesmo_nome_em_pastas_diferentes_nao_dividem_id() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_ids", &["a/leia-me.txt", "b/leia-me.txt"]);
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, editor::Partida::default(), |_| None);
    let raiz = dados(&amb);
    let (a, b) = (raiz.join("a"), raiz.join("b"));
    let mut p = PastaAberta::nova(raiz.clone());
    p.nos.insert(
        p.chave.clone(),
        pronta(&raiz, vec![entrada(&raiz, "a", Natureza::Pasta), entrada(&raiz, "b", Natureza::Pasta)], 0),
    );
    for pasta in [&a, &b] {
        p.nos.insert(
            chave(pasta),
            pronta(pasta, vec![entrada(pasta, "leia-me.txt", Natureza::Dados(TipoDeArquivo::Texto))], 0),
        );
    }
    simular(&mut h, Some(p));
    clicar_link(&mut h, "a, pasta");
    clicar_link(&mut h, "b, pasta");
    acoes(&mut h);
    let nos: Vec<_> =
        h.query_all(by().role(Role::Link).label("leia-me.txt")).map(|n| (n.accesskit_node().id(), n.rect())).collect();
    assert_eq!(nos.len(), 2);
    assert_ne!(nos[0].0, nos[1].0, "cada um com o seu id");
    // o clique no de baixo é dele (sem o id dividido, o egui daria o clique ao primeiro)
    let (de_cima, de_baixo) = if nos[0].1.top() < nos[1].1.top() { (nos[0].1, nos[1].1) } else { (nos[1].1, nos[0].1) };
    assert!(de_cima.top() < de_baixo.top());
    h.query_all(by().role(Role::Link).label("leia-me.txt")).find(|n| n.rect() == de_baixo).expect("o de baixo").click();
    h.run_steps(2);
    assert_eq!(acoes(&mut h), [Acao::AbrirArquivoDeDados { caminho: b.join("leia-me.txt") }]);
}

/// A leitura que parou antes do fim (o prazo de 5 s, as 20.000 entradas ou os 2.000 cabeçalhos: `Listagem::parcial`)
/// avisa na dica da pasta, depois das ocultas; a pasta lida inteira não tem o aviso.
#[test]
fn pasta_lida_pela_metade_avisa_na_dica() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_parcial", &[]);
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, editor::Partida::default(), |_| None);
    let raiz = dados(&amb);
    let mut p = PastaAberta::nova(raiz.clone());
    let entradas = vec![entrada(&raiz, "a.csv", Natureza::Dados(TipoDeArquivo::Csv))];
    let parcial = Listagem::montar(raiz.clone(), entradas.clone(), 1, true, LIMITE_POR_PASTA);
    p.nos.insert(p.chave.clone(), NoDaPasta::Pronto(Arc::new(parcial)));
    simular(&mut h, Some(p.clone()));
    let dica = ler_dica(&mut h, Role::Link, "dados, pasta");
    assert_eq!(
        dica,
        format!(
            "{}\n1 arquivo de outro tipo não aparece.\nA leitura parou antes do fim: alguns itens podem não aparecer.",
            raiz.display()
        )
    );
    p.nos.insert(p.chave.clone(), pronta(&raiz, entradas, 0));
    simular(&mut h, Some(p));
    assert_eq!(ler_dica(&mut h, Role::Link, "dados, pasta"), raiz.display().to_string());
}

#[test]
fn mais_n_itens_nao_tem_foco() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_limite", &[]);
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, editor::Partida::default(), |_| None);
    let raiz = dados(&amb);
    let mut p = PastaAberta::nova(raiz.clone());
    let entradas = ["a.csv", "b.csv", "c.csv", "d.csv", "e.csv"]
        .iter()
        .map(|n| entrada(&raiz, n, Natureza::Dados(TipoDeArquivo::Csv)))
        .collect();
    // o limite reduzido: 3 à vista e 2 a mais
    p.nos.insert(p.chave.clone(), pronta_com_limite(&raiz, entradas, 0, 3));
    simular(&mut h, Some(p));
    assert!(existe(&h, Role::Link, "c.csv") && !existe(&h, Role::Link, "d.csv"));
    let mais = rect_de(&h, Role::Label, "mais 2 itens");
    assert!(mais.top() >= rect_de(&h, Role::Link, "c.csv").bottom(), "depois das entradas");
    let dica = ler_dica(&mut h, Role::Label, "mais 2 itens");
    assert_eq!(dica, "Só os primeiros 3 itens aparecem. Use Mostrar na pasta para ver o resto.");
    // o Tab passa do último nó para o que vem depois do Explorador, sem parar na linha
    focar(&mut h, "c.csv");
    h.key_press(Key::Tab);
    h.run_steps(2);
    let depois = focado(&h);
    assert!(depois.is_some() && depois.as_deref() != Some("mais 2 itens"), "{depois:?}");
    // o ↓ também não para nela
    focar(&mut h, "c.csv");
    h.key_press(Key::ArrowDown);
    h.run_steps(2);
    assert_ne!(focado(&h).as_deref(), Some("mais 2 itens"));
}

#[test]
fn pasta_vazia_carregando_e_falha_com_tentar_de_novo() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_estados", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let sub = dados(&amb).join("sub");
    clicar_link(&mut h, "sub, pasta");
    acoes(&mut h);
    // a leitura de verdade que a casca pediu termina antes de o teste trocar o nó à mão
    assentar(&mut h);
    com_no(&mut h, &sub, NoDaPasta::Lendo);
    assert!(tem_texto(&h, "Carregando…"));
    // a falha: a frase, o motivo e o Tentar de novo, que pede a leitura de novo
    com_no(&mut h, &sub, NoDaPasta::Falhou("Acesso negado.".into()));
    assert!(tem_texto(&h, "Não foi possível ler a pasta.") && tem_texto(&h, "Acesso negado."));
    assert!(!tem_texto(&h, "Carregando…"));
    clicar_botao(&mut h, "Tentar de novo");
    assert_eq!(acoes(&mut h), [listar(&sub)]);
    assentar(&mut h);
    // vazia
    com_no(&mut h, &sub, pronta(&sub, vec![], 3));
    assert!(tem_texto_exato(&h, "Nenhum arquivo que o DataBox abra."));
    assert!(!tem_texto(&h, "Não foi possível ler a pasta."));
    // a raiz que falhou (a pasta sumiu): o mesmo aviso no lugar da árvore
    let raiz = dados(&amb);
    com_no(&mut h, &raiz, NoDaPasta::Falhou("A pasta não existe (ou foi movida).".into()));
    assert!(tem_texto(&h, "Não foi possível ler a pasta.") && tem_texto(&h, "A pasta não existe (ou foi movida)."));
    // o banco aberto, que ficou sem lugar, vira a segunda raiz
    assert!(existe(&h, Role::Link, "exemplo.db"));
    clicar_botao(&mut h, "Tentar de novo");
    assert_eq!(acoes(&mut h), [listar(&raiz)]);
}

#[test]
fn banco_fora_da_pasta_vira_a_segunda_raiz_no_alto() {
    let mut amb = Ambiente::com_pasta("explorador_pasta_fora", &[]);
    let fora = amostras::criar_banco_exemplo(amb.pasta().join("fora.db")).expect("fora.db");
    amb.servico.abrir_banco(&fora, false, false).expect("abrir o fora.db");
    amb.recarregar();
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    // no alto, igual à árvore sem pasta (banco em 8, tabelas em 34); a pasta depois
    let banco = rect_de(&h, Role::Link, "fora.db");
    assert!(banco.top() < rect_de(&h, Role::Link, "dados, pasta").top());
    assert!(rect_de(&h, Role::Link, "Tabelas").top() < rect_de(&h, Role::Link, "dados, pasta").top());
    assert_eq!(recuo_do_no(&h, "fora.db", Icone::Banco), 8.0);
    assert_eq!(recuo_do_no(&h, "clientes", Icone::Tabela), 34.0);
    assert_eq!(recuo_do_no(&h, "dados, pasta", Icone::PastaSolida), 8.0);
    // o exemplo.db da pasta é um banco fechado
    assert!(!ligado(&h, Role::Link, "exemplo.db") && existe(&h, Role::Link, "exemplo.db"));
    capturar(&mut h, "explorador-pasta-segunda-raiz-claro.png");
}

#[test]
fn banco_dentro_de_pasta_fechada_abre_os_pais() {
    let mut amb = Ambiente::com_pasta("explorador_pasta_pais", &[]);
    let fundo = amostras::criar_banco_exemplo(amb.pasta_de_dados().join("sub").join("fundo.db")).expect("fundo.db");
    amb.servico.abrir_banco(&fundo, false, false).expect("abrir o fundo.db");
    amb.recarregar();
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, editor::Partida::default(), |_| None);
    let raiz = dados(&amb);
    let sub = raiz.join("sub");
    assentar(&mut h);
    simular(&mut h, Some(pasta_de_dados(&raiz)));
    // a subpasta abre sozinha e pede a leitura; enquanto ela não chega, o banco não vira segunda raiz
    assert_eq!(h.get(by().role(Role::Link).label("sub, pasta")).accesskit_node().data().is_expanded(), Some(true));
    assert_eq!(acoes(&mut h), [listar(&sub)]);
    // a leitura de verdade que a casca pediu pode chegar a qualquer quadro (numa máquina carregada, já nos dois de
    // cima): ela termina antes de o teste pôr a subpasta em leitura à mão
    assentar(&mut h);
    com_no(&mut h, &sub, NoDaPasta::Lendo);
    assert!(!existe(&h, Role::Link, "fundo.db"));
    com_no(&mut h, &sub, pronta(&sub, vec![entrada(&sub, "fundo.db", Natureza::Banco)], 0));
    // no lugar dele, dois níveis abaixo da raiz, com a subárvore
    assert_eq!(h.query_all(by().role(Role::Link).label("fundo.db")).count(), 1);
    assert_eq!(recuo_do_no(&h, "fundo.db", Icone::Banco), 34.0);
    assert!(rect_de(&h, Role::Link, "fundo.db").top() > rect_de(&h, Role::Link, "sub, pasta").top());
    assert_eq!(recuo_do_no(&h, "clientes", Icone::Tabela), 62.0);
    // fechada pela pessoa, a subpasta fica fechada (e o banco não sobe para a raiz de cima)
    clicar_link(&mut h, "sub, pasta");
    assert!(!existe(&h, Role::Link, "fundo.db"));
    h.run_steps(3);
    assert!(!existe(&h, Role::Link, "fundo.db"));
}

#[test]
fn recolher_tudo_deixa_a_raiz_aberta() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_recolher", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let sub = dados(&amb).join("sub");
    com_no(&mut h, &sub, pronta(&sub, vec![entrada(&sub, "interno.csv", Natureza::Dados(TipoDeArquivo::Csv))], 0));
    clicar_link(&mut h, "sub, pasta");
    assert!(existe(&h, Role::Link, "interno.csv") && existe(&h, Role::Link, "clientes"));
    clicar_botao(&mut h, "Recolher tudo");
    // a raiz continua aberta, com o que ela tem; a subpasta e o banco recolhem
    assert_eq!(h.get(by().role(Role::Link).label("dados, pasta")).accesskit_node().data().is_expanded(), Some(true));
    assert!(existe(&h, Role::Link, "sub, pasta") && existe(&h, Role::Link, "exemplo.db"));
    assert!(!existe(&h, Role::Link, "interno.csv") && !existe(&h, Role::Link, "Tabelas"));
    assert!(acoes(&mut h).is_empty());
}

#[test]
fn seta_direita_e_esquerda_abrem_e_fecham_a_pasta() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_setas", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let sub = dados(&amb).join("sub");
    focar(&mut h, "sub, pasta");
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(h.get(by().role(Role::Link).label("sub, pasta")).accesskit_node().data().is_expanded(), Some(true));
    assert_eq!(acoes(&mut h), [listar(&sub)]);
    assert_eq!(focado(&h).as_deref(), Some("sub, pasta"), "o foco fica no nó");
    h.key_press(Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(h.get(by().role(Role::Link).label("sub, pasta")).accesskit_node().data().is_expanded(), Some(false));
    assert_eq!(focado(&h).as_deref(), Some("sub, pasta"));
    // Espaço também é o clique: abre
    h.key_press(Key::Space);
    h.run_steps(2);
    assert_eq!(h.get(by().role(Role::Link).label("sub, pasta")).accesskit_node().data().is_expanded(), Some(true));
    // ↓ desce pela árvore, de nó em nó
    h.key_press(Key::ArrowDown);
    h.run_steps(2);
    assert!(focado(&h).is_some_and(|f| f != "sub, pasta"), "{:?}", focado(&h));
}

/// O → num banco fechado da pasta não troca de banco (a troca fecharia as abas do aberto): só o clique, o Enter e o
/// Abrir do menu.
#[test]
fn seta_direita_no_banco_fechado_nao_troca_de_banco() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_seta_banco", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    focar(&mut h, "estoque_2026.db");
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    assert!(acoes(&mut h).is_empty());
    h.key_press(Key::Enter);
    h.run_steps(2);
    let estoque = dados(&amb).join("estoque_2026.db");
    assert_eq!(acoes(&mut h), [Acao::AbrirBanco { caminho: estoque, somente_leitura: false }]);
}

/// O clique duplo numa pasta (o gesto do Explorador do Windows) abre e deixa aberta: o segundo clique não a fecha.
#[test]
fn clique_duplo_na_pasta_deixa_aberta() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_clique_duplo", &[]);
    // quadros de 0,05 s: os dois cliques ficam dentro do tempo do clique duplo do egui (0,3 s)
    let editor = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, editor::Partida::default()).com_dialogo(|_| None);
    let mut h = harness_editor_com_passo(editor, Tema::Claro, TAMANHO, 0.05);
    simular(&mut h, Some(pasta_de_dados(&dados(&amb))));
    acoes(&mut h);
    let expandida = |h: &H| h.get(by().role(Role::Link).label("sub, pasta")).accesskit_node().data().is_expanded();
    h.get(by().role(Role::Link).label("sub, pasta")).click();
    h.step();
    h.get(by().role(Role::Link).label("sub, pasta")).click();
    h.run_steps(2);
    assert_eq!(expandida(&h), Some(true));
    // um clique depois (fora do tempo do clique duplo) fecha
    h.run_steps(10);
    h.get(by().role(Role::Link).label("sub, pasta")).click();
    h.run_steps(2);
    assert_eq!(expandida(&h), Some(false));
}

/// Fechar a pasta esquece o que era dela: a mesma pasta aberta de novo volta ao começo, com a raiz aberta (mesmo
/// depois de um Recolher tudo sem pasta) e os pais do banco aberto abertos outra vez.
#[test]
fn a_mesma_pasta_aberta_de_novo_volta_ao_comeco() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_de_novo", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let raiz = dados(&amb);
    let aberta = |h: &H| h.get(by().role(Role::Link).label("dados, pasta")).accesskit_node().data().is_expanded();
    assert_eq!(aberta(&h), Some(true));
    // fecha a pasta (o banco continua) e recolhe tudo
    simular(&mut h, None);
    clicar_botao(&mut h, "Recolher tudo");
    simular(&mut h, Some(pasta_de_dados(&raiz)));
    assert_eq!(aberta(&h), Some(true), "a raiz abre sozinha");
    assert!(existe(&h, Role::Link, "sub, pasta") && existe(&h, Role::Link, "exemplo.db"), "o que a raiz tem");
}

#[test]
fn shift_f10_abre_o_menu_de_contexto() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_shift_f10", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    focar(&mut h, "clientes.csv");
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(3);
    assert!(menu_aberto_de(&h, "clientes.csv"), "{:?}", textos(&h));
    // arquivo: Abrir, Mostrar na pasta, Copiar caminho (com o atalho)
    assert_eq!(itens_do_menu(&h), ["Abrir", "Mostrar na pasta", "Copiar caminho"]);
    // o foco vai ao primeiro item; Enter abre o arquivo e o menu fecha
    assert_eq!(focado(&h).as_deref(), Some("Abrir"));
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert!(!menu_aberto_de(&h, "clientes.csv"));
    assert_eq!(acoes(&mut h), [Acao::AbrirArquivoDeDados { caminho: dados(&amb).join("clientes.csv") }]);
    // o da pasta: Atualizar no lugar do Abrir; o Esc fecha e devolve o foco ao nó
    focar(&mut h, "sub, pasta");
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(3);
    assert_eq!(itens_do_menu(&h), ["Atualizar", "Mostrar na pasta", "Copiar caminho"]);
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!menu_aberto_de(&h, "sub"));
    assert_eq!(focado(&h).as_deref(), Some("sub, pasta"));
    // o do banco (o aberto também): Abrir foca a Visão geral
    focar(&mut h, "estoque_2026.db");
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(3);
    assert_eq!(itens_do_menu(&h), ["Abrir", "Mostrar na pasta", "Copiar caminho"]);
    h.key_press(Key::Escape);
    h.run_steps(3);
}

#[test]
fn ctrl_shift_c_copia_o_caminho_com_o_foco_no_no() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_ctrl_shift_c", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let raiz = dados(&amb);
    let ctrl_shift = Modifiers { shift: true, ..CTRL };
    focar(&mut h, "dados.json");
    assert_eq!(
        copiar_com(&mut h, ctrl_shift, tecla_c(ctrl_shift)),
        Some(raiz.join("dados.json").display().to_string())
    );
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Caminho copiado."));
    // a pasta e o banco aberto também
    focar(&mut h, "sub, pasta");
    assert_eq!(copiar_com(&mut h, ctrl_shift, tecla_c(ctrl_shift)), Some(raiz.join("sub").display().to_string()));
    focar(&mut h, "exemplo.db");
    let banco = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string());
    assert_eq!(copiar_com(&mut h, ctrl_shift, tecla_c(ctrl_shift)), banco);
    // como o egui-winit entrega no Windows: o `Event::Copy` (sem a tecla) com o Ctrl+Shift apertados
    focar(&mut h, "notas.txt");
    assert_eq!(copiar_com(&mut h, ctrl_shift, egui::Event::Copy), Some(raiz.join("notas.txt").display().to_string()));
    // só com os modificadores exatos: o AltGr (Ctrl+Alt) e o Ctrl+C sozinho não copiam o caminho; sem o foco num
    // nó, também não
    focar(&mut h, "dados.json");
    let altgr = Modifiers { shift: true, alt: true, ..CTRL };
    assert_eq!(copiar_com(&mut h, altgr, tecla_c(altgr)), None);
    assert_eq!(copiar_com(&mut h, altgr, egui::Event::Copy), None);
    assert_eq!(copiar_com(&mut h, CTRL, tecla_c(CTRL)), None);
    h.ctx.memory_mut(|m| m.surrender_focus(m.focused().expect("o foco no nó")));
    h.run_steps(2);
    assert_eq!(copiar_com(&mut h, ctrl_shift, tecla_c(ctrl_shift)), None);
}

#[test]
fn menu_de_contexto_da_raiz_tem_fechar_pasta() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_menu_raiz", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    h.get(by().role(Role::Link).label("dados, pasta")).click_secondary();
    h.run_steps(3);
    assert!(menu_aberto_de(&h, "dados"));
    assert_eq!(itens_do_menu(&h), ["Atualizar", "Mostrar na pasta", "Copiar caminho", "Fechar pasta"]);
    // o menu da prancheta 15: 250 de largura e itens de 28
    let item = item_de_menu(&h, "Atualizar").rect();
    assert_eq!(item.height(), 28.0);
    let menu = h.get(by().role(Role::Menu).label("dados")).rect();
    assert_eq!(menu.width(), 250.0);
    capturar(&mut h, "explorador-menu-de-contexto-claro.png");
    clicar_item(&mut h, "Atualizar");
    assert_eq!(acoes(&mut h), [Acao::AtualizarPasta]);
    // o Atualizar de uma subpasta relê só ela
    let sub = dados(&amb).join("sub");
    h.get(by().role(Role::Link).label("sub, pasta")).click_secondary();
    h.run_steps(3);
    clicar_item(&mut h, "Atualizar");
    assert_eq!(acoes(&mut h), [listar(&sub)]);
    // o Fechar pasta (a casca tira a pasta: passo 8)
    h.get(by().role(Role::Link).label("dados, pasta")).click_secondary();
    h.run_steps(3);
    clicar_item(&mut h, "Fechar pasta");
    assert_eq!(acoes(&mut h), [Acao::FecharPasta]);
    assentar(&mut h);
    assert!(!h.state().estado().tem_pasta());
}

#[test]
fn menu_de_contexto_escuro() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_menu_escuro", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Escuro);
    h.get(by().role(Role::Link).label("clientes.csv")).click_secondary();
    h.run_steps(3);
    assert!(menu_aberto_de(&h, "clientes.csv"));
    capturar(&mut h, "explorador-menu-de-contexto-escuro.png");
}

#[test]
fn copiar_caminho_copia_e_avisa() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_copiar", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    h.get(by().role(Role::Link).label("planilha.xlsx")).click_secondary();
    h.run_steps(3);
    // o texto copiado sai no quadro do clique: sem os passos do `clicar_item`
    item_de_menu(&h, "Copiar caminho").click();
    assert_eq!(copiado(&mut h), Some(dados(&amb).join("planilha.xlsx").display().to_string()));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Caminho copiado."));
    assert!(acoes(&mut h).is_empty(), "o Explorador mesmo cumpre");
}

#[test]
fn mostrar_na_pasta_chama_o_gancho() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_mostrar", &[]);
    let anotados: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let gancho = Rc::clone(&anotados);
    let editor = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, editor::Partida::default())
        .com_dialogo(|_| None)
        .com_abrir_de_teste(move |p| gancho.borrow_mut().push(p.to_path_buf()));
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    let raiz = dados(&amb);
    simular(&mut h, Some(pasta_de_dados(&raiz)));
    for (no, caminho) in
        [("notas.txt", raiz.join("notas.txt")), ("sub, pasta", raiz.join("sub")), ("dados, pasta", raiz.clone())]
    {
        h.get(by().role(Role::Link).label(no)).click_secondary();
        h.run_steps(3);
        clicar_item(&mut h, "Mostrar na pasta");
        h.run_steps(2);
        assert_eq!(anotados.borrow().last(), Some(&caminho), "{no}");
    }
    assert_eq!(anotados.borrow().len(), 3);
}

#[test]
fn explorador_vazio_pede_a_pasta() {
    let amb = Ambiente::vazio("explorador_pasta_vazio");
    let mut h = harness(&amb);
    // prancheta 4: a pasta de 48, as duas frases e o botão; o cabeçalho só com o título
    assert!(tem_texto_exato(&h, "Nenhuma pasta aberta."));
    assert!(tem_texto_exato(&h, "Abra uma pasta para ver os bancos, planilhas e textos que ela tem."));
    assert!(h.query(by().role(Role::Button).label("Recolher tudo")).is_none());
    // o do Explorador (o da barra de ferramentas, ligado desde o passo 11, vem antes)
    let botao = abrir_pasta_do_explorador(&h).rect();
    assert!(icone_pintado_em(&h, Icone::PastaSolida, 48.0, botao.expand2(egui::vec2(60.0, 120.0))).is_some());
    acoes(&mut h);
    abrir_pasta_do_explorador(&h).click();
    h.run_steps(2);
    assert_eq!(acoes(&mut h), [Acao::EscolherEAbrirPasta]);
    capturar(&mut h, "explorador-vazio-claro.png");
    // com a pasta e sem banco, o cabeçalho volta com os dois botões; o Abrir pasta dele também pede a pasta
    let amb = Ambiente::com_pasta("explorador_pasta_sem_banco", &[]);
    let mut h = harness(&amb);
    simular(&mut h, Some(pasta_de_dados(&dados(&amb))));
    assert!(existe(&h, Role::Button, "Recolher tudo") && existe(&h, Role::Link, "dados, pasta"));
    assert!(!tem_texto(&h, "Nenhuma pasta aberta."));
    acoes(&mut h);
    abrir_pasta_do_explorador(&h).click();
    h.run_steps(2);
    assert_eq!(acoes(&mut h), [Acao::EscolherEAbrirPasta]);
}

/// O "Abrir pasta" do Explorador (o do meio, vazio, ou o do cabeçalho): o que fica no painel da esquerda, embaixo da
/// barra de ferramentas (que tem o dela) e antes do documento (o cartão do Início também se chama assim).
fn abrir_pasta_do_explorador(h: &H) -> egui_kittest::Node<'_> {
    h.query_all(by().role(Role::Button).label("Abrir pasta"))
        .find(|n| n.rect().top() > 90.0 && n.rect().right() < 340.0)
        .expect("o Abrir pasta do Explorador")
}

#[test]
fn dica_do_abrir_pasta_do_cabecalho() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_dica", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let botao = h
        .query_all(by().role(Role::Button).label("Abrir pasta"))
        .find(|n| !n.accesskit_node().is_disabled())
        .expect("o Abrir pasta do cabeçalho");
    botao.hover();
    h.run_steps(3);
    assert!(tem_texto(&h, "Abrir pasta (Ctrl+Shift+O)"), "{:?}", textos(&h));
    h.remove_cursor();
    h.run_steps(2);
}

#[test]
fn explorador_com_a_pasta_parado_nao_repinta() {
    let amb = ambiente_com_banco_na_pasta("explorador_pasta_parado", &[]);
    let mut h = harness_da_pasta(&amb, Tema::Claro);
    let sub = dados(&amb).join("sub");
    com_no(&mut h, &sub, pronta(&sub, vec![entrada(&sub, "interno.csv", Natureza::Dados(TipoDeArquivo::Csv))], 0));
    clicar_link(&mut h, "sub, pasta");
    // o ponteiro passou pelos nós e saiu; depois, com o menu de contexto aberto
    for rotulo in ["dados, pasta", "interno.csv", "clientes.csv"] {
        h.get(by().role(Role::Link).label(rotulo)).hover();
        h.run_steps(3);
    }
    h.remove_cursor();
    for cena in ["a árvore", "o menu de contexto"] {
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(10));
            h.step();
        }
        let mut causas = Vec::new();
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(10));
            h.step();
            if h.ctx.has_requested_repaint() {
                causas.extend(h.ctx.repaint_causes().iter().map(|c| format!("{}:{} {}", c.file, c.line, c.reason)));
            }
        }
        assert!(causas.is_empty(), "{cena}, parado, pediu repintura: {causas:#?}");
        h.get(by().role(Role::Link).label("clientes.csv")).click_secondary();
        h.run_steps(3);
        h.remove_cursor();
    }
}
