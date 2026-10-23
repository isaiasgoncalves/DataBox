//! A busca da barra (Ctrl+F) no texto e no leitor de arquivos grandes: no texto inteiro; o trecho achado fica
//! pintado e à vista; Enter e Shift+Enter andam; o rodapé diz "3 de 41"; Esc volta ao cursor (ou à linha) de antes.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use estado::{Documento, Verbo};
use tela_texto::Modo;

/// 205 linhas: "São Luís" (ou "SAO LUIS") nas linhas 3, 8, 13… (de 5 em 5: 41 linhas) e "Recife" nas outras.
fn cidades() -> String {
    let mut s = String::new();
    for i in 0..205 {
        let cidade = match (i % 5 == 3, i % 2 == 0) {
            (true, true) => "São Luís",
            (true, false) => "SAO LUIS",
            _ => "Recife",
        };
        s.push_str(&format!("linha {i}: {cidade}\n"));
    }
    s
}

fn rotulo(t: &H) -> Option<String> {
    t.state().tela.doc.rotulo_da_busca()
}

/// O cursor do editor (em caracteres).
fn cursor(t: &H) -> Option<usize> {
    let id = t.state().tela.doc.id_do_campo();
    egui::text_edit::TextEditState::load(&t.ctx, id).and_then(|e| e.cursor.char_range()).map(|r| r.primary.index.0)
}

/// O trecho achado, como texto.
fn achado(t: &H) -> Option<String> {
    let r = t.state().tela.doc.trecho_achado()?;
    Some(texto(t)[r].to_string())
}

/// O byte onde começa a linha `i` (de 0) do texto.
fn byte_da_linha(texto: &str, i: usize) -> usize {
    texto.split_inclusive('\n').take(i).map(str::len).sum()
}

/// Digita `termo` na busca da barra e espera a busca terminar.
fn buscar(t: &mut Teste, rotulo_do_campo: &str, termo: &str) {
    let campo = t.get_by_label(rotulo_do_campo);
    campo.focus();
    campo.type_text(termo);
    t.run_steps(2);
    esperar_ate(t, |h| !h.state().tela.doc.ocupada());
    t.run_steps(2);
}

#[test]
fn busca_no_editor_anda_e_volta_ao_cursor_de_antes() {
    let conteudo = cidades();
    let mut t = abrir("buscar_texto", "notas.txt", conteudo.as_bytes());
    assert_eq!(t.state().tela.doc.modo(), Modo::Editor);
    assert!(t.state().tela.doc.ofertas(&t.state().estado).ligado(Verbo::Buscar), "o Ctrl+F vale no texto");
    // o cursor no começo da linha 12
    let antes = conteudo[..byte_da_linha(&conteudo, 12)].chars().count();
    let id = t.state().tela.doc.id_do_campo();
    let mut estado = egui::text_edit::TextEditState::load(&t.ctx, id).unwrap_or_default();
    estado.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(antes))));
    estado.store(&t.ctx, id);
    t.run_steps(2);
    buscar(&mut t, "Buscar em notas.txt", "sao luis");
    // o primeiro achado depois do cursor: o da linha 13, o terceiro dos 41
    assert_eq!(rotulo(&t).as_deref(), Some("3 de 41"));
    assert!(t.query_by_label("3 de 41").is_some(), "a contagem no rodapé");
    let inicio = byte_da_linha(&conteudo, 13) + "linha 13: ".len();
    assert_eq!(t.state().tela.doc.trecho_achado(), Some(inicio..inicio + "SAO LUIS".len()));
    assert_eq!(achado(&t).as_deref(), Some("SAO LUIS"));
    // o cursor vai para o fim do trecho achado
    assert_eq!(cursor(&t), Some(conteudo[..inicio + "SAO LUIS".len()].chars().count()));
    t.key_press(Key::Enter);
    t.run_steps(2);
    assert_eq!((achado(&t).as_deref(), rotulo(&t).as_deref()), (Some("São Luís"), Some("4 de 41")));
    for _ in 0..4 {
        t.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
        t.run_steps(2);
    }
    // antes do primeiro, o último (a linha 203)
    assert_eq!(rotulo(&t).as_deref(), Some("41 de 41"));
    let ultimo = byte_da_linha(&conteudo, 203) + "linha 203: ".len();
    assert_eq!(t.state().tela.doc.trecho_achado().map(|r| r.start), Some(ultimo));
    t.key_press(Key::Enter);
    t.run_steps(2);
    assert_eq!(rotulo(&t).as_deref(), Some("1 de 41"));
    // o Esc limpa e volta ao cursor de antes da busca
    t.key_press(Key::Escape);
    t.run_steps(2);
    assert_eq!(cursor(&t), Some(antes));
    assert_eq!(rotulo(&t), None);
    assert_eq!(t.state().tela.doc.trecho_achado(), None);
}

#[test]
fn busca_no_leitor_de_arquivo_grande_rola_ate_o_achado() {
    let mut s = String::new();
    for i in 0..14_000 {
        s.push_str(&format!("linha {i}: um texto comprido o bastante para encher o arquivo e passar do limite\n"));
    }
    let mut t = abrir("buscar_leitor", "grande.log", s.as_bytes());
    assert_eq!(t.state().tela.doc.modo(), Modo::Leitor);
    assert!(t.state().tela.doc.ofertas(&t.state().estado).ligado(Verbo::Buscar));
    buscar(&mut t, "Buscar em grande.log", "TEXTO COMPRIDO");
    assert_eq!(rotulo(&t).as_deref(), Some("1 de 14.000"));
    // antes do primeiro, o último: o leitor rola até a linha dele
    t.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    t.run_steps(3);
    assert_eq!(rotulo(&t).as_deref(), Some("14.000 de 14.000"));
    assert!(t.state().tela.doc.linhas_desenhadas().contains(&13_999), "{:?}", t.state().tela.doc.linhas_desenhadas());
    let campo = t.get_by_label("Buscar em grande.log");
    campo.focus();
    campo.type_text("x");
    t.run_steps(2);
    esperar_ate(&mut t, |h| !h.state().tela.doc.ocupada());
    assert_eq!(rotulo(&t).as_deref(), Some("Nenhum resultado"));
    // o Esc volta à linha de antes da busca (a primeira)
    t.key_press(Key::Escape);
    t.run_steps(3);
    assert_eq!(t.state().tela.doc.linhas_desenhadas().start, 0);
    // uma ocorrência só, no meio do arquivo
    buscar(&mut t, "Buscar em grande.log", "linha 9876:");
    assert_eq!(rotulo(&t).as_deref(), Some("1 de 1"));
    t.run_steps(2);
    assert!(t.state().tela.doc.linhas_desenhadas().contains(&9876));
    let r = t.state().tela.doc.trecho_achado().unwrap();
    assert_eq!(&s[r], "linha 9876:");
}
