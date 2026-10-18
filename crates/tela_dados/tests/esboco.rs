//! Fumaça do apoio de testes: a aba Dados desenha a primeira página de clientes num palco com o banco de exemplo.

use estado::apoio_testes::{Ambiente, capturar, esperar, palco};
use interface::tema::Tema;
use tela_dados::AbaDados;

#[test]
fn esboco_desenha_no_palco() {
    let amb = Ambiente::exemplo("esboco_dados");
    let tabela = amb.tabela("clientes");
    assert_eq!(tabela.info().linhas, Some(60));
    let aba = AbaDados::nova(&tabela);
    let mut h = palco(&amb, (aba, tabela), Tema::Claro, egui::vec2(1200.0, 900.0), |ui, ctx, (aba, tabela)| {
        aba.ui(ui, ctx, tabela)
    });
    esperar(&mut h, |h| {
        let (aba, tabela) = &h.state().tela;
        aba.retrato(tabela).linhas == 60
    });
    let png = capturar(&mut h, "esboco-dados.png");
    assert!(png.exists());
    let _ = std::fs::remove_file(png);
}
