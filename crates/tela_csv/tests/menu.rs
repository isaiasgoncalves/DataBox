//! O menu da célula na grade do CSV e o rodapé da seleção (0.2.3, `design/planos-0.2.3.md`, A.1 a A.3): o clique
//! direito dentro e fora do intervalo, o Shift+F10 e o Esc, os itens de escrita desligados com a dica, o Copiar como
//! lido de volta da área de transferência, o Filtrar criando a condição, o Marcar as linhas e as contas do rodapé (na
//! hora e, além da página, com o "Somando…").

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use apoio::*;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::Ambiente;
use interface::grade::{MOTIVO_PROXIMA_ETAPA, ResumoSelecao};
use interface::tema::Tema;
use tela_csv::{Ganchos, OperadorCsv};

fn celula(h: &H, linha: u64, coluna: usize) -> String {
    doc(h).pagina().and_then(|p| p.celula(linha, coluna)).unwrap_or_default().to_string()
}

fn clique_direito(h: &mut H, linha: usize, coluna: usize) {
    let pos = doc(h).centro_celula(linha, coluna).expect("a célula à vista");
    clicar_direito_em(h, pos);
}

fn menu_aberto(h: &H) -> Option<String> {
    h.query_by_role(Role::Menu).and_then(|m| m.accesskit_node().label())
}

/// Escolhe um item do menu da célula (no submenu "Copiar como", passa por ele antes). Devolve o que foi para a área de
/// transferência nos quadros seguintes.
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

/// As contas do rodapé da seleção (o texto inteiro; num rótulo, o AccessKit o guarda em value).
fn rodape_da_selecao(h: &H) -> Option<String> {
    h.query_by_label_contains("Contagem ").and_then(|n| n.accesskit_node().value())
}

fn grade_com_foco(h: &H) -> bool {
    let id = doc(h).id_grade().expect("a grade");
    h.ctx.memory(|m| m.has_focus(id))
}

/// As contas que o rodapé deve mostrar para as `colunas` das linhas `linhas` do `conteudo` (`;`, decimal `,`, com o
/// cabeçalho), todas numéricas.
fn contas(conteudo: &str, linhas: std::ops::Range<usize>, colunas: &[usize]) -> String {
    let mut r = ResumoSelecao::default();
    for linha in conteudo.lines().skip(1).skip(linhas.start).take(linhas.len()) {
        let campos: Vec<&str> = linha.split(';').collect();
        for &c in colunas {
            r.somar_do_csv(campos[c], true, ',');
        }
    }
    r.texto()
}

#[test]
fn menu_copia_nos_formatos_filtra_e_marca() {
    let amb = Ambiente::vazio("csv_menu_da_celula");
    let conteudo = vendas(30);
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", conteudo.clone()));
    // quantidade e valor, três linhas
    clicar_celula(&mut h, 0, 4);
    tecla(&mut h, Key::ArrowDown, Modifiers::SHIFT);
    tecla(&mut h, Key::ArrowDown, Modifiers::SHIFT);
    tecla(&mut h, Key::ArrowRight, Modifiers::SHIFT);
    assert_eq!(rodape_da_selecao(&h), Some(contas(&conteudo, 0..3, &[4, 5])), "as contas na hora");

    // o clique direito dentro do intervalo age nele
    clique_direito(&mut h, 1, 5);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da seleção"));
    for item in ["Colar", "Limpar", "Excluir as linhas"] {
        let n = item_do_menu(&h, item);
        assert!(n.accesskit_node().is_disabled(), "{item} desligado");
        assert_eq!(n.accesskit_node().description().as_deref(), Some(MOTIVO_PROXIMA_ETAPA));
    }
    assert!(h.query_by_label("Definir como NULL").is_none(), "o CSV não tem NULL");
    assert!(h.query_by_label("Abrir a ficha da linha").is_none());
    let texto = escolher(&mut h, "INSERT", true).expect("copiou");
    let primeira = format!("  ({}, {}),", celula(&h, 0, 4), celula(&h, 0, 5).replace(',', "."));
    assert!(texto.starts_with("INSERT INTO \"vendas_2025\" (\"quantidade\", \"valor\") VALUES\n"), "{texto}");
    assert!(texto.contains(&primeira), "os números do CSV como literais: {texto}");
    assert!(menu_aberto(&h).is_none(), "escolher fecha o menu");
    assert!(grade_com_foco(&h), "e devolve o foco à grade");

    // CSV com o separador do arquivo, e o Copiar com cabeçalho
    clique_direito(&mut h, 0, 4);
    let texto = escolher(&mut h, "CSV", true).expect("copiou");
    assert!(texto.starts_with(&format!("quantidade;valor\r\n{};", celula(&h, 0, 4))), "{texto}");
    clique_direito(&mut h, 0, 4);
    let texto = escolher(&mut h, "Copiar com cabeçalho", false).expect("copiou");
    assert!(texto.starts_with(&format!("quantidade\tvalor\r\n{}\t", celula(&h, 0, 4))), "{texto}");

    // fora do intervalo a célula vira a ativa; o Filtrar cria a condição e o chip
    clique_direito(&mut h, 5, 1);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da célula"));
    assert_eq!(doc(&h).celula_ativa(), Some((5, 1)));
    let regiao = celula(&h, 5, 1);
    escolher(&mut h, "Filtrar por este valor", false);
    esperar_ociosa(&mut h);
    let c = doc(&h).condicoes().first().cloned().expect("a condição");
    assert_eq!((c.coluna.as_str(), c.op, c.valor.as_str()), ("regiao", OperadorCsv::Igual, regiao.as_str()));
    let chip = format!("Editar o filtro regiao igual a {regiao}");
    assert!(h.query_by_role_and_label(Role::Button, &chip).is_some(), "o chip da condição");
    assert_eq!(doc(&h).linhas_filtradas(), Some(6), "uma região a cada cinco linhas");

    // Excluir este valor do filtro: "diferente de"
    clique_direito(&mut h, 0, 2);
    let vendedor = celula(&h, 0, 2);
    escolher(&mut h, "Excluir este valor do filtro", false);
    esperar_ociosa(&mut h);
    let c = doc(&h).condicoes().get(1).cloned().expect("a segunda condição");
    assert_eq!((c.op, c.valor.as_str()), (OperadorCsv::Diferente, vendedor.as_str()));

    // Marcar as linhas: as do intervalo, somando às marcadas
    clicar_celula(&mut h, 0, 0);
    tecla(&mut h, Key::ArrowDown, Modifiers::SHIFT);
    clique_direito(&mut h, 0, 0);
    escolher(&mut h, "Marcar as linhas", false);
    h.run_steps(2);
    assert_eq!(doc(&h).linhas_marcadas().len(), 2);

    // o Shift+F10 abre na ativa com o foco no primeiro item; o Esc fecha e devolve o foco
    clicar_celula(&mut h, 1, 3);
    tecla(&mut h, Key::F10, Modifiers::SHIFT);
    assert!(menu_aberto(&h).is_some(), "o Shift+F10 abre o menu da célula");
    assert!(item_do_menu(&h, "Copiar").accesskit_node().is_focused());
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    assert!(menu_aberto(&h).is_none() && grade_com_foco(&h));
    // com uma coluna escolhida, o Shift+F10 é o do cabeçalho
    clicar_cabecalho(&mut h, 3, Modifiers::NONE);
    tecla(&mut h, Key::F10, Modifiers::SHIFT);
    assert_eq!(menu_aberto(&h).as_deref(), Some("produto"), "o menu da coluna produto");
}

#[test]
fn rodape_soma_alem_da_pagina_com_o_somando() {
    let amb = Ambiente::vazio("csv_rodape_somando");
    let segurar = Arc::new(AtomicBool::new(true));
    let s2 = Arc::clone(&segurar);
    let ganchos = Ganchos {
        somar: Some(Arc::new(move || {
            let inicio = Instant::now();
            while s2.load(Ordering::SeqCst) && inicio.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(5));
            }
        })),
        ..Ganchos::default()
    };
    let conteudo = vendas(3000);
    let caminho = arquivo(&amb, "vendas_2025.csv", conteudo.clone());
    let mut h = palco(&amb, caminho, Tema::Claro, TAMANHO, ganchos);
    esperar_ociosa(&mut h);
    // da quantidade até o fim (Ctrl+Shift+End): a página não cobre, a conta vai para uma tarefa
    clicar_celula(&mut h, 0, 4);
    tecla(&mut h, Key::End, Modifiers::COMMAND | Modifiers::SHIFT);
    assert!(h.query_by_label(interface::grade::SOMANDO).is_some(), "o Somando… enquanto a conta roda");
    assert_eq!(rodape_da_selecao(&h), None);
    segurar.store(false, Ordering::SeqCst);
    let esperado = contas(&conteudo, 0..3000, &[4, 5]);
    let inicio = Instant::now();
    while rodape_da_selecao(&h).as_deref() != Some(esperado.as_str()) {
        assert!(inicio.elapsed() < Duration::from_secs(20), "as contas não chegaram: {:?}", rodape_da_selecao(&h));
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(esperado.starts_with("Contagem 6.000 · Soma "), "{esperado}");
    // uma célula só (o Ctrl+Home desfaz o intervalo): o rodapé de sempre
    tecla(&mut h, Key::Home, Modifiers::COMMAND);
    assert_eq!(rodape_da_selecao(&h), None);
    assert!(h.query_by_label(interface::grade::SOMANDO).is_none());
}

#[test]
fn ctrl_shift_c_copia_com_o_cabecalho() {
    let amb = Ambiente::vazio("csv_ctrl_shift_c");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_celula(&mut h, 1, 1);
    tecla(&mut h, Key::ArrowRight, Modifiers::SHIFT);
    h.input_mut().events.push(Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND | Modifiers::SHIFT,
    });
    h.step();
    let esperado = format!("regiao\tvendedor\r\n{}\t{}\r\n", celula(&h, 1, 1), celula(&h, 1, 2));
    assert_eq!(area_de_transferencia(&mut h), Some(esperado));
}

#[test]
fn capturas_do_menu_da_celula() {
    let amb = Ambiente::vazio("csv_menu_capturas");
    for (tema, nome) in [(Tema::Claro, "csv-menu-da-celula-claro.png"), (Tema::Escuro, "csv-menu-da-celula-escuro.png")]
    {
        let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(30)), tema, TAMANHO);
        clicar_celula(&mut h, 2, 4);
        tecla(&mut h, Key::ArrowDown, Modifiers::SHIFT);
        tecla(&mut h, Key::ArrowDown, Modifiers::SHIFT);
        tecla(&mut h, Key::ArrowRight, Modifiers::SHIFT);
        clique_direito(&mut h, 3, 5);
        h.get_by_label("Copiar como").hover();
        h.run_steps(4);
        capturar(&mut h, nome);
    }
}
