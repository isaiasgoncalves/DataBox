//! A busca da barra (Ctrl+F) na planilha: na aba inteira, não só nas linhas que já chegaram; a célula achada vira a
//! ativa e a grade rola até ela; Enter e Shift+Enter andam; a faixa diz "3 de 41"; Esc volta à célula de antes; as
//! colunas ocultas ficam de fora.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use estado::{Documento, Verbo};

/// 1.927 linhas: o número em A, "São Luís" (ou "SAO LUIS") em B nas linhas 3, 50, 97… (de 47 em 47: 41 linhas) e
/// "l{n}" nas outras; na coluna C, oculta, "sao luis" na linha 10.
fn cidades() -> Vec<u8> {
    let mut linhas = String::new();
    for i in 1..=1927u32 {
        let b = match (i % 47 == 3, (i / 47) % 2 == 0) {
            (true, true) => t("São Luís"),
            (true, false) => t("SAO LUIS"),
            _ => t(&format!("l{i}")),
        };
        let c = if i == 10 { t("sao luis") } else { C::Nada };
        linhas.push_str(&linha_xml(i, &[C::Num(f64::from(i)), b, c], false));
    }
    let extras = Extras { colunas: vec![(3, 3, 10.0, true)], ..Extras::default() };
    Montador::novo().aba_com("Plan1", &linhas, &extras).bytes()
}

fn rotulo(h: &H) -> Option<String> {
    tela(h).rotulo_da_busca()
}

fn ativa(h: &H) -> Option<(u32, u32)> {
    tela(h).grade().and_then(|g| g.celula_ativa())
}

/// Roda quadros até a busca terminar.
fn esperar_a_busca(t: &mut Teste) {
    esperar_ate(t, |h| !tela(h).ocupada());
    t.run_steps(2);
}

#[test]
fn busca_na_aba_inteira_anda_e_volta_a_celula_de_antes() {
    let mut t = abrir("pl_buscar", "cidades.xlsx", &cidades());
    esperar_o_indice(&mut t);
    t.run_steps(2);
    assert!(tela(&t).ofertas(&t.state().estado).ligado(Verbo::Buscar), "o Ctrl+F vale na planilha");
    tela_mut(&mut t).grade_mut().unwrap().nucleo.selecionar(Some((50, 1)));
    t.run_steps(2);
    let campo = t.get_by_label("Buscar em Plan1");
    campo.focus();
    campo.type_text("sao luis");
    t.run_steps(2);
    esperar_a_busca(&mut t);
    // o primeiro achado depois de A51: B97, o terceiro dos 41 (o C10 oculto não conta)
    assert_eq!(rotulo(&t).as_deref(), Some("3 de 41"));
    assert!(t.query_by_label("3 de 41").is_some(), "a contagem na faixa");
    assert_eq!(ativa(&t), Some((97, 1)));
    t.key_press(Key::Enter);
    t.run_steps(2);
    assert_eq!((ativa(&t), rotulo(&t).as_deref()), (Some((144, 1)), Some("4 de 41")));
    for _ in 0..3 {
        t.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
        t.run_steps(2);
    }
    assert_eq!((ativa(&t), rotulo(&t).as_deref()), (Some((3, 1)), Some("1 de 41")));
    // antes do primeiro, o último: a grade rola até ele e lê as linhas dele
    t.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    t.run_steps(2);
    assert_eq!((ativa(&t), rotulo(&t).as_deref()), (Some((1883, 1)), Some("41 de 41")));
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.linha(1883).is_some()));
    t.key_press(Key::Enter);
    t.run_steps(2);
    assert_eq!(ativa(&t), Some((3, 1)), "depois do último, o primeiro");
    // o Esc limpa e volta à célula de antes da busca
    t.key_press(Key::Escape);
    t.run_steps(2);
    assert_eq!(ativa(&t), Some((51, 0)));
    assert_eq!(rotulo(&t), None);
    // sem nada achado
    let campo = t.get_by_label("Buscar em Plan1");
    campo.focus();
    campo.type_text("olinda");
    t.run_steps(2);
    esperar_a_busca(&mut t);
    assert_eq!(rotulo(&t).as_deref(), Some("Nenhum resultado"));
    assert_eq!(ativa(&t), Some((51, 0)));
}

#[test]
fn achado_alem_das_linhas_conhecidas_espera_o_indice() {
    let mut t = abrir_com(
        "pl_buscar_indice",
        "cidades.xlsx",
        &cidades(),
        interface::tema::Tema::Claro,
        TAMANHO,
        tela_planilha::TelaPlanilha::com_indice_manual,
    );
    t.run_steps(2);
    let antes = tela(&t).grade().map(|g| g.nucleo.n_linhas()).unwrap();
    assert!(antes < 1500, "antes do índice, a grade só conhece {antes} linhas");
    let campo = t.get_by_label("Buscar em Plan1");
    campo.focus();
    campo.type_text("l1500");
    t.run_steps(2);
    esperar_a_busca(&mut t);
    // a busca já achou (no fluxo, sem o índice), mas a grade ainda não tem a linha
    assert_eq!(rotulo(&t).as_deref(), Some("1 de 1"));
    assert_ne!(ativa(&t), Some((1500, 1)));
    let egui = t.ctx.clone();
    tela_mut(&mut t).iniciar_indice(&egui);
    esperar_o_indice(&mut t);
    t.run_steps(3);
    assert_eq!(ativa(&t), Some((1500, 1)), "com o índice, a grade vai ao achado");
}
