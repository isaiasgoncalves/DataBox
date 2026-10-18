//! As riscas de 135° aparecem de verdade no render (e não só numa textura em repetição, que o renderizador de testes
//! do egui, com a filtragem previsível, ignorava): uma risca de 1px a cada 4px na diagonal, também na peça de
//! cantos arredondados, e nada fora do contorno.

use egui::{CornerRadius, Rect, pos2, vec2};
use egui_kittest::Harness;
use interface::fontes;
use interface::pintura::{self, Preenchimento};
use interface::tema::{self, Tema};

/// Um renderizador wgpu de cada vez neste arquivo (dois ao mesmo tempo, sob carga, derrubam o processo).
static UM_DE_CADA_VEZ: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn renderizar(
    tema_escolhido: Tema,
    desenhar: impl Fn(&egui::Painter, &interface::tema::Paleta) + 'static,
) -> image::RgbaImage {
    let _trava = UM_DE_CADA_VEZ.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut h = Harness::builder().with_size(vec2(160.0, 90.0)).wgpu().build_ui(move |ui| {
        let ctx = ui.ctx().clone();
        tema::aplicar(&ctx, tema_escolhido);
        if !fontes::instaladas(&ctx) {
            ctx.request_repaint();
            return;
        }
        desenhar(ui.painter(), tema::paleta(ui));
    });
    h.run_steps(4);
    h.render().expect("renderizar com wgpu")
}

/// Quantos pixels de `area` diferem do fundo liso `fundo`.
fn diferentes(img: &image::RgbaImage, area: Rect, fundo: [u8; 4]) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    for y in area.min.y as u32..area.max.y as u32 {
        for x in area.min.x as u32..area.max.x as u32 {
            if img.get_pixel(x, y).0 != fundo {
                v.push((x, y));
            }
        }
    }
    v
}

#[test]
fn riscas_aparecem_uma_a_cada_quatro_pixels_na_diagonal() {
    for tema_escolhido in [Tema::Escuro, Tema::Claro] {
        let area = Rect::from_min_max(pos2(20.0, 20.0), pos2(140.0, 60.0));
        let img = renderizar(tema_escolhido, move |painter, p| {
            pintura::preencher(painter, p, area, 0.0, Preenchimento::Liso(p.estudio.sup_meio));
            pintura::riscas(painter, p, area, 0.0);
        });
        // o fundo liso é a cor mais comum da área; as riscas mudam pixels em toda linha
        let mut contagem = std::collections::HashMap::<[u8; 4], usize>::new();
        for y in 20..60 {
            for x in 20..140 {
                *contagem.entry(img.get_pixel(x, y).0).or_default() += 1;
            }
        }
        let fundo = *contagem.iter().max_by_key(|(_, n)| **n).map(|(c, _)| c).expect("pixels");
        let riscados = diferentes(&img, area, fundo);
        let total = (area.width() * area.height()) as usize;
        assert!(
            riscados.len() * 5 > total && riscados.len() * 3 < total,
            "{tema_escolhido:?}: {} de {total} pixels riscados (era para ser cerca de um quarto)",
            riscados.len()
        );
        // todos na mesma classe da diagonal: x + y com o mesmo resto na divisão por 4
        let classe = (riscados[0].0 + riscados[0].1) % 4;
        assert!(riscados.iter().all(|(x, y)| (x + y) % 4 == classe), "{tema_escolhido:?}: as riscas não são diagonais");
    }
}

#[test]
fn riscas_respeitam_os_cantos_arredondados() {
    let area = Rect::from_min_max(pos2(20.0, 20.0), pos2(140.0, 60.0));
    let img = renderizar(Tema::Escuro, move |painter, p| {
        pintura::riscas_com_canto(painter, p, area, CornerRadius::same(14));
    });
    // sem fundo nenhum pintado, só as riscas: o canto de cima à esquerda (fora do arco de 14) fica vazio
    let canto = Rect::from_min_max(pos2(20.0, 20.0), pos2(23.0, 23.0));
    let fundo = img.get_pixel(5, 5).0;
    assert!(diferentes(&img, canto, fundo).is_empty(), "risca fora do canto arredondado");
    assert!(!diferentes(&img, area.shrink(16.0), fundo).is_empty(), "sem riscas no miolo");
}
