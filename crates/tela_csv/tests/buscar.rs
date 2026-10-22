//! A busca da barra (Ctrl+F) no CSV: no arquivo inteiro, não só na página em mãos; a célula achada vira a ativa e a
//! grade rola até ela; Enter e Shift+Enter andam; a faixa diz "3 de 41"; Esc volta à célula de antes; com filtro, só
//! nas linhas que ele mostra.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use estado::apoio_testes::Ambiente;
use estado::{Documento, Verbo};
use tela_csv::OperadorCsv;

/// 1.927 linhas de `n;cidade;grupo`: "São Luís" (ou "SAO LUIS") nas linhas 3, 50, 97… (de 47 em 47: 41 linhas), o
/// grupo "a" nas linhas pares e "b" nas ímpares.
fn cidades() -> String {
    let mut s = String::from("n;cidade;grupo\n");
    for i in 0..1927 {
        let cidade = match (i % 47 == 3, (i / 47) % 2 == 0) {
            (true, true) => "São Luís",
            (true, false) => "SAO LUIS",
            _ => "Recife",
        };
        s.push_str(&format!("{i};{cidade};{}\n", if i % 2 == 0 { "a" } else { "b" }));
    }
    s
}

/// Digita `termo` na busca da barra e espera a busca terminar.
fn buscar(h: &mut H, termo: &str) {
    let campo = h.get_by_label("Buscar em cidades.csv");
    campo.focus();
    campo.type_text(termo);
    h.run_steps(2);
    esperar_ociosa(h);
}

fn rotulo(h: &H) -> Option<String> {
    doc(h).rotulo_da_busca()
}

#[test]
fn busca_no_arquivo_inteiro_anda_e_volta_a_celula_de_antes() {
    let amb = Ambiente::vazio("buscar_csv");
    let mut h = abrir(&amb, arquivo(&amb, "cidades.csv", cidades()));
    assert!(doc(&h).ofertas(&h.state().estado).ligado(Verbo::Buscar), "o Ctrl+F vale no CSV");
    doc_mut(&mut h).selecionar(Some((51, 0)));
    h.run_steps(2);
    buscar(&mut h, "sao luis");
    // o primeiro achado depois da célula ativa (a linha 52): a 98, o terceiro dos 41
    assert_eq!(rotulo(&h).as_deref(), Some("3 de 41"));
    // a contagem na ponta direita da barra dos filtros (o CSV não tem a faixa)
    let barra = h.get_by_label("Filtros").rect().center().y;
    assert!((h.get_by_label("3 de 41").rect().center().y - barra).abs() < 4.0, "a contagem na barra dos filtros");
    assert_eq!(doc(&h).celula_ativa(), Some((97, 1)));
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!((doc(&h).celula_ativa(), rotulo(&h).as_deref()), (Some((144, 1)), Some("4 de 41")));
    h.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    h.run_steps(2);
    assert_eq!((doc(&h).celula_ativa(), rotulo(&h).as_deref()), (Some((3, 1)), Some("1 de 41")));
    // antes do primeiro, o último: a grade rola até ele e lê a página dele
    h.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    esperar_ociosa(&mut h);
    assert_eq!((doc(&h).celula_ativa(), rotulo(&h).as_deref()), (Some((1883, 1)), Some("41 de 41")));
    assert!(doc(&h).centro_celula(1883, 1).is_some(), "a grade rolou até a célula achada");
    assert!(doc(&h).pagina().is_some_and(|p| p.contem(1883)));
    assert_eq!(doc(&h).pagina().unwrap().celula(1883, 1), Some("São Luís"));
    // e depois do último, o primeiro
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(doc(&h).celula_ativa(), Some((3, 1)));
    // o Esc limpa e volta à célula de antes da busca
    h.key_press(Key::Escape);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).celula_ativa(), Some((51, 0)));
    assert_eq!(rotulo(&h), None);
    assert!(h.query_by_label("1 de 41").is_none());
}

#[test]
fn busca_so_nas_linhas_do_filtro_e_sem_nada_diz_nenhum() {
    let amb = Ambiente::vazio("buscar_csv_filtro");
    let mut h = abrir(&amb, arquivo(&amb, "cidades.csv", cidades()));
    doc_mut(&mut h).filtrar("grupo", OperadorCsv::Igual, "b");
    esperar_ociosa(&mut h);
    buscar(&mut h, "SÃO");
    // só as ímpares: 3, 97, 191… (21 das 41); a linha 3 do arquivo é a segunda da vista
    assert_eq!(rotulo(&h).as_deref(), Some("1 de 21"));
    assert_eq!(doc(&h).celula_ativa(), Some((1, 1)));
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(doc(&h).celula_ativa(), Some((48, 1)), "a linha 97 do arquivo, na vista");
    let campo = h.get_by_label("Buscar em cidades.csv");
    campo.focus();
    campo.type_text("x");
    h.run_steps(2);
    esperar_ociosa(&mut h);
    assert_eq!(rotulo(&h).as_deref(), Some("Nenhum resultado"));
}

#[test]
fn a_alteracao_pendente_entra_na_busca() {
    let amb = Ambiente::vazio("buscar_csv_alteracao");
    let mut h = abrir(&amb, arquivo(&amb, "cidades.csv", cidades()));
    buscar(&mut h, "olinda");
    assert_eq!(rotulo(&h).as_deref(), Some("Nenhum resultado"));
    // a célula alterada (ainda não salva) refaz a busca e passa a ser achada
    clicar_celula(&mut h, 5, 1);
    tecla(&mut h, Key::F2, Modifiers::NONE);
    tecla(&mut h, Key::A, Modifiers::COMMAND);
    digitar(&mut h, "Olinda");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert!(doc(&h).alterado());
    // a busca refeita conta a célula, mas não puxa a célula ativa de quem estava editando; o Enter parte dela
    assert_eq!(rotulo(&h).as_deref(), Some("1 resultado"));
    clicar_celula(&mut h, 2, 0);
    h.get_by_label("Buscar em cidades.csv").focus();
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(rotulo(&h).as_deref(), Some("1 de 1"));
    assert_eq!(doc(&h).celula_ativa(), Some((5, 1)));
}
