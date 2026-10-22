//! O menu da célula na grade da planilha e o rodapé da seleção (0.2.3, `design/planos-0.2.3.md`, A.1 a A.3): o clique
//! direito dentro e fora do intervalo, o Shift+F10 (o da célula, e o do cabeçalho com uma coluna escolhida), o filtro
//! desligado com a dica, o Copiar como lido de volta da área de transferência, o Ctrl+Shift+C e as contas do rodapé
//! (das linhas que já chegaram e, além delas, numa tarefa).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::{NodeT, Queryable};
use interface::grade::ResumoSelecao;
use interface::tema::Tema;
use tela_planilha::MSG_SEM_FILTRO;

fn abrir_relatorio(nome: &str, tema: Tema) -> Teste {
    let mut t = abrir_com(&format!("pl_menu_{nome}"), "relatorio_q3.xlsx", &relatorio_q3(60), tema, TAMANHO, |t| t);
    esperar_o_indice(&mut t);
    t.run_steps(3);
    t
}

/// O centro do texto pintado `texto` (o primeiro).
fn centro_de(h: &H, texto: &str) -> Pos2 {
    h.output()
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == texto => Some(t.pos + egui::vec2(t.galley.size().x / 2.0, 7.0)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{texto} não foi pintado"))
}

fn botao(h: &mut H, pos: Pos2, button: PointerButton) {
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

fn tecla(h: &mut H, key: Key, modifiers: Modifiers) {
    h.event(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run_steps(2);
}

fn menu_aberto(h: &H) -> Option<String> {
    h.query_by_role(Role::Menu).and_then(|m| m.accesskit_node().label())
}

/// Escolhe um item do menu (no submenu "Copiar como", passa por ele antes) e devolve o que foi copiado.
fn escolher(h: &mut H, item: &str, submenu: bool) -> Option<String> {
    if submenu {
        h.get_by_label("Copiar como").hover();
        h.run_steps(3);
    }
    h.get_by_label(item).click();
    let mut copiado = None;
    for _ in 0..4 {
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

fn rodape_da_selecao(h: &H) -> Option<String> {
    h.query_by_label_contains("Contagem ").and_then(|n| n.accesskit_node().value())
}

/// A receita da linha `n` do Excel no `relatorio_q3` (a coluna D).
fn receita(n: u32) -> f64 {
    let k = (n - 2) as usize;
    ((2200 + (k * 7919) % 11_800) * 100 + (k * 37) % 100) as f64 / 100.0
}

#[test]
fn menu_copia_nos_formatos_e_o_filtro_desligado() {
    let mut t = abrir_relatorio("formatos", Tema::Claro);
    let pos = centro_de(&t, "R$ 10.119,37");
    botao(&mut t, pos, PointerButton::Primary);
    tecla(&mut t, Key::ArrowDown, Modifiers::SHIFT);
    tecla(&mut t, Key::ArrowDown, Modifiers::SHIFT);
    let mut esperado = ResumoSelecao::default();
    for n in 3..=5 {
        let r = receita(n);
        let casas = format!("{r}").split_once('.').map_or(0, |(_, f)| f.len() as u8);
        esperado.somar_numero(r, casas);
    }
    assert_eq!(rodape_da_selecao(&t), Some(esperado.texto()), "os números do Excel entram nas contas");

    // dentro do intervalo
    botao(&mut t, pos, PointerButton::Secondary);
    assert_eq!(menu_aberto(&t).as_deref(), Some("Menu da seleção"));
    for item in ["Filtrar por este valor", "Excluir este valor do filtro"] {
        let n = t.get_by_label(item);
        assert!(n.accesskit_node().is_disabled(), "{item} desligado");
        assert_eq!(n.accesskit_node().description().as_deref(), Some(MSG_SEM_FILTRO));
    }
    for item in ["Colar", "Limpar", "Marcar as linhas", "Excluir as linhas", "Abrir a ficha da linha"] {
        assert!(t.query_by_label(item).is_none(), "a planilha não tem {item}");
    }
    let json = escolher(&mut t, "JSON", true).expect("copiou");
    let linhas: Vec<String> = (3..=5).map(|n| format!("  {{\"D\": {}}}", receita(n))).collect();
    assert_eq!(json, format!("[\n{}\n]\n", linhas.join(",\n")), "a letra é o nome; o número vai como número");

    botao(&mut t, pos, PointerButton::Secondary);
    let insert = escolher(&mut t, "INSERT", true).expect("copiou");
    assert!(insert.starts_with("INSERT INTO \"Vendas\" (\"D\") VALUES\n"), "o nome da planilha: {insert}");
    botao(&mut t, pos, PointerButton::Secondary);
    let com_cabecalho = escolher(&mut t, "Copiar com cabeçalho", false).expect("copiou");
    assert_eq!(com_cabecalho.lines().count(), 4, "a letra e as três linhas: {com_cabecalho:?}");
    assert!(com_cabecalho.starts_with("D\r\n10119,37\r\n"), "{com_cabecalho:?}");

    // fora do intervalo, a célula vira a ativa
    let fora = centro_de(&t, "Receita");
    botao(&mut t, fora, PointerButton::Secondary);
    assert_eq!(menu_aberto(&t).as_deref(), Some("Menu da célula"));
    assert_eq!(tela(&t).grade().and_then(|g| g.celula_ativa()), Some((2, 3)));
    tecla(&mut t, Key::Escape, Modifiers::NONE);
    assert!(menu_aberto(&t).is_none());

    // o Shift+F10 abre o da célula; com uma coluna escolhida, o do cabeçalho
    tecla(&mut t, Key::F10, Modifiers::SHIFT);
    assert_eq!(menu_aberto(&t).as_deref(), Some("Menu da célula"));
    assert!(t.get_by_label("Copiar").accesskit_node().is_focused(), "o foco no primeiro item");
    tecla(&mut t, Key::Escape, Modifiers::NONE);
    let d = centro_de(&t, "D");
    botao(&mut t, d, PointerButton::Primary);
    tecla(&mut t, Key::F10, Modifiers::SHIFT);
    assert_eq!(menu_aberto(&t).as_deref(), Some("Coluna"), "o menu do cabeçalho");
}

#[test]
fn ctrl_shift_c_copia_com_a_letra() {
    let mut t = abrir_relatorio("ctrl_shift_c", Tema::Claro);
    let pos = centro_de(&t, "R$ 10.119,37");
    botao(&mut t, pos, PointerButton::Primary);
    tecla(&mut t, Key::ArrowRight, Modifiers::SHIFT);
    t.event(Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND | Modifiers::SHIFT,
    });
    t.step();
    let copiado = t.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    });
    assert!(copiado.is_some_and(|c| c.starts_with("D\tE\r\n10119,37\t")));
}

#[test]
fn rodape_soma_as_linhas_que_ainda_nao_chegaram() {
    let bytes = Montador::novo().aba("Dados", &linhas_simples(5000)).bytes();
    let mut t = abrir_com("pl_menu_soma", "dados.xlsx", &bytes, Tema::Claro, TAMANHO, |t| t);
    esperar_o_indice(&mut t);
    t.run_steps(3);
    let pos = centro_de(&t, "1");
    botao(&mut t, pos, PointerButton::Primary);
    // até o fim: a coluna A (os números de 1 a 5.000) e a B (textos)
    tecla(&mut t, Key::End, Modifiers::COMMAND | Modifiers::SHIFT);
    let soma = 5000 * 5001 / 2;
    let esperado = format!(
        "Contagem 10.000 · Soma {} · Média 2.500,5 · Mín. 1 · Máx. 5.000",
        interface::formato::formatar_numero(soma)
    );
    esperar_ate(&mut t, |h| rodape_da_selecao(h).as_deref() == Some(esperado.as_str()));
}

#[test]
fn capturas_do_menu_da_celula() {
    for (tema, nome, sal) in [
        (Tema::Claro, "planilha-menu-da-celula-claro.png", "captura_claro"),
        (Tema::Escuro, "planilha-menu-da-celula-escuro.png", "captura_escuro"),
    ] {
        let mut t = abrir_relatorio(sal, tema);
        let pos = centro_de(&t, "R$ 10.119,37");
        botao(&mut t, pos, PointerButton::Primary);
        tecla(&mut t, Key::ArrowDown, Modifiers::SHIFT);
        tecla(&mut t, Key::ArrowDown, Modifiers::SHIFT);
        botao(&mut t, pos, PointerButton::Secondary);
        t.get_by_label("Filtrar por este valor").hover();
        t.run_steps(30);
        capturar(&mut t, nome);
    }
}

/// O rodapé sem a faixa (0.2.3, item F): da janela larga à estreita, as contas da seleção encolhem primeiro; depois
/// saem os itens da esquerda (o chip "Somente leitura" antes da contagem de linhas), e a planilha com a célula ativa
/// fica até o fim.
#[test]
fn rodape_corta_as_contas_antes_e_a_migalha_por_ultimo() {
    let rotulo_no_rodape =
        |h: &H, rotulo: &str, altura: f32| h.query_all_by_label(rotulo).any(|n| n.rect().top() > altura - 40.0);
    let pintado = |h: &H, inicio: &str| {
        h.output()
            .shapes
            .iter()
            .any(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text().starts_with(inicio)))
    };
    // (as contas inteiras, o chip, a contagem de linhas, a planilha e a célula)
    let mut estados = Vec::new();
    for largura in (560..=1240).rev().step_by(40) {
        let tamanho = egui::vec2(largura as f32, 700.0);
        let nome = format!("pl_menu_rodape_{largura}");
        let mut t = abrir_com(&nome, "relatorio_q3.xlsx", &relatorio_q3(60), Tema::Claro, tamanho, |t| t);
        esperar_o_indice(&mut t);
        t.run_steps(3);
        tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((2, 4)));
        t.run_steps(2);
        tela_mut(&mut t).grade_mut().expect("a grade").nucleo.estender((4, 4));
        t.run_steps(3);
        let contas = rodape_da_selecao(&t).expect("as contas");
        assert!(rotulo_no_rodape(&t, "Vendas", 700.0) && rotulo_no_rodape(&t, "D3", 700.0), "a migalha ({largura})");
        estados.push((
            pintado(&t, &contas),
            rotulo_no_rodape(&t, "Somente leitura", 700.0),
            rotulo_no_rodape(&t, "· 60 linhas · 6 colunas", 700.0),
        ));
    }
    assert_eq!(estados.first(), Some(&(true, true, true)), "larga: tudo cabe");
    assert!(estados.contains(&(false, true, true)), "as contas encolhem primeiro: {estados:?}");
    assert!(estados.iter().any(|e| !e.1), "o chip sai quando falta espaço: {estados:?}");
    assert!(!estados.iter().any(|e| e.0 && !e.1), "as contas encolhem antes de o chip sair: {estados:?}");
    assert!(!estados.iter().any(|e| e.1 && !e.2), "o chip sai antes da contagem: {estados:?}");
}
