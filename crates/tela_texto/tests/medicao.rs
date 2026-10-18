//! A medição do `TextEdit` do egui com texto grande (0,5, 1 e 2 MiB), a base dos limites de
//! `texto::limites`.
//!
//! `#[ignore]`: roda com `.\cargo.ps1 test -p tela_texto --test medicao '--' --ignored --nocapture`. Desenha o mesmo campo
//! que o editor usa (o `AreaTexto` em modo código, plano, com a calha dos números) num palco, e conta o tempo de
//! cada quadro (de CPU: a lógica, o layout do texto e a pintura em formas, sem a GPU) com o texto parado, digitando uma
//! letra por quadro e com o Ctrl+Z. As dependências do dev profile são otimizadas (`opt-level = 2`), então o egui é medido
//! como roda no executável; o que está abaixo de uns 50 ms por tecla se sente como instantâneo.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use egui::{Event, Id, Key, Modifiers};
use estado::apoio_testes::{Ambiente, palco};
use interface::codigo::Linguagem;
use interface::pecas::campos::AreaTexto;
use interface::tema::Tema;

mod apoio;

/// Um texto de linhas de uns 80 caracteres com `bytes` bytes (JSON-ish para a versão colorida).
fn texto_de(bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 100);
    let mut i = 0;
    while s.len() < bytes {
        s.push_str(&format!(
            "  \"campo_{i}\": \"um valor de texto bem comprido para encher a linha do arquivo {i}\",\n"
        ));
        i += 1;
    }
    s
}

fn medir(linguagem: Linguagem, bytes: usize) -> (Duration, Duration, Duration, usize) {
    let amb = Ambiente::vazio("medicao_text_edit");
    let id = Id::new("medicao::campo");
    let mut h = {
        let _vez = apoio::vez();
        palco(&amb, texto_de(bytes), Tema::Escuro, egui::vec2(1440.0, 900.0), move |ui, _ctx, texto| {
            let rect = ui.available_rect_before_wrap();
            let filho = &mut ui.new_child(egui::UiBuilder::new().max_rect(rect));
            AreaTexto::new(texto)
                .codigo(true)
                .plana(true)
                .numeros_de_linha(true)
                .linguagem(linguagem)
                .altura(rect.height())
                .largura(rect.width())
                .id_salt("medicao::campo")
                .mostrar(filho);
        })
    };
    h.ctx.memory_mut(|m| m.request_focus(id));
    h.run_steps(4);
    let tamanho = h.state().tela.len();
    let parado = {
        let t = Instant::now();
        for _ in 0..10 {
            h.step();
        }
        t.elapsed() / 10
    };
    let digitando = {
        let t = Instant::now();
        for _ in 0..10 {
            h.event(Event::Text("a".into()));
            h.step();
        }
        t.elapsed() / 10
    };
    let desfazendo = {
        h.event(Event::Key {
            key: Key::Z,
            physical_key: Some(Key::Z),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        });
        let t = Instant::now();
        h.step();
        t.elapsed()
    };
    let _vez = apoio::vez();
    drop(h);
    (parado, digitando, desfazendo, tamanho)
}

#[test]
#[ignore = "medição: roda com --ignored --nocapture"]
fn text_edit_com_0_5_1_e_2_mib() {
    println!("{:<8} {:<8} {:>12} {:>12} {:>12}", "tamanho", "cores", "parado", "digitando", "ctrl+z");
    for (rotulo, bytes) in [("0,5 MiB", 512 * 1024), ("1 MiB", 1024 * 1024), ("2 MiB", 2 * 1024 * 1024)] {
        for (nome, linguagem) in [("sem", Linguagem::Texto), ("json", Linguagem::Json)] {
            let (parado, digitando, desfazendo, n) = medir(linguagem, bytes);
            println!(
                "{rotulo:<8} {nome:<8} {:>9.1} ms {:>9.1} ms {:>9.1} ms   ({n} bytes)",
                parado.as_secs_f64() * 1000.0,
                digitando.as_secs_f64() * 1000.0,
                desfazendo.as_secs_f64() * 1000.0
            );
        }
    }
}
