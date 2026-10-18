//! Mostruário da interface: desenha todas as peças nos dois temas e grava as imagens.
//!
//! ```text
//! .\cargo.ps1 run -p interface --example mostruario --target-dir target\ag-interface '--' [pasta]
//! ```
//!
//! Sem pasta, grava em `capturas/rust/` na raiz do projeto. O crate `interface` não depende do `eframe` (o
//! executável é o crate `editor`), então o exemplo renderiza sem janela, com o mesmo wgpu da janela de verdade.
//! Para ver o mostruário numa janela, o `editor` (ou qualquer app eframe) chama, a cada quadro:
//!
//! ```ignore
//! impl eframe::App for App {
//!     fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
//!         self.mostruario.ui(ui); // interface::mostruario::Mostruario::new(Tema::Sistema)
//!     }
//! }
//! ```
//!
//! Lá, o seletor "Sistema · Claro · Escuro" do cabeçalho (e o do menu Arquivo) alterna o tema na hora, e o "Ver a
//! casca" do cartão dos ícones troca a página pela casca nova inteira (Ajuda › Mostruário das peças volta).

use std::path::PathBuf;

use egui::UiBuilder;
use egui_kittest::Harness;
use interface::mostruario::Mostruario;
use interface::tema::{self, Tema};
use interface::{fontes, pintura};

/// O que a imagem mostra.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pagina {
    Pecas,
    Janela,
    Casca,
}

fn renderizar(tema_escolhido: Tema, pagina: Pagina, destino: &PathBuf) -> Result<(), String> {
    let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
    let altura = if pagina == Pagina::Pecas { 3900.0 } else { 900.0 };
    let mut mostruario = Mostruario::new(tema_escolhido);
    match pagina {
        Pagina::Pecas => {}
        Pagina::Janela => mostruario.abrir_janela(),
        Pagina::Casca => mostruario.mostrar_casca(true),
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, altura))
        .with_pixels_per_point(1.0)
        .with_theme(tema_egui)
        .with_max_steps(12)
        .wgpu()
        .build_ui_state(
            |ui, m: &mut Mostruario| {
                let ctx = ui.ctx().clone();
                tema::aplicar(&ctx, m.tema());
                tema::definir_movimento_reduzido(&ctx, true);
                if !fontes::instaladas(&ctx) {
                    ctx.request_repaint();
                    return;
                }
                let tela = ctx.content_rect();
                let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                pagina.set_clip_rect(tela);
                pintura::pintar_fundo(pagina.painter(), tema::paleta(&pagina), tela);
                if m.casca_a_vista() {
                    m.pagina_casca(&mut pagina);
                } else {
                    m.pagina(&mut pagina);
                }
                m.sobreposicoes(&mut pagina);
            },
            mostruario,
        );
    harness.run_steps(6);
    let imagem = harness.render()?;
    imagem.save(destino).map_err(|e| format!("não foi possível gravar {}: {e}", destino.display()))
}

fn main() {
    let pasta = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("capturas").join("rust")
    });
    if let Err(e) = std::fs::create_dir_all(&pasta) {
        eprintln!("Não foi possível criar a pasta {}: {e}", pasta.display());
        std::process::exit(1);
    }
    let pedidos = [
        (Tema::Claro, Pagina::Pecas, "mostruario-claro.png"),
        (Tema::Escuro, Pagina::Pecas, "mostruario-escuro.png"),
        (Tema::Claro, Pagina::Janela, "mostruario-janela-claro.png"),
        (Tema::Escuro, Pagina::Janela, "mostruario-janela-escuro.png"),
        (Tema::Claro, Pagina::Casca, "mostruario-casca-claro.png"),
        (Tema::Escuro, Pagina::Casca, "mostruario-casca-escuro.png"),
    ];
    for (tema_escolhido, pagina, nome) in pedidos {
        let destino = pasta.join(nome);
        match renderizar(tema_escolhido, pagina, &destino) {
            Ok(()) => println!("{}", destino.display()),
            Err(e) => {
                eprintln!("{nome}: {e}");
                std::process::exit(1);
            }
        }
    }
}
