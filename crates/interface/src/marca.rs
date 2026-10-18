//! O logo do DataBox (a caixa de papelão com o banco e as ferramentas), embutido em PNG rasterizado de
//! `design/logo/databox.svg` por `design/ferramentas/rasterizar` (ver `assets/logo/`). É sempre o logo completo, em
//! qualquer tamanho (decisão de 17/10/2026: não há versão simplificada): o PNG de cada lado vem do SVG desenhado
//! direto naquele lado, e não de uma redução de um maior.
//!
//! Os lados cobrem o que o app pede: 16 pt na barra de título (16, 20, 24, 32 e 48 px a 100%, 125%, 150%, 200% e
//! 300%) e 60 pt na janela Sobre (60, 90 e 120 px a 100%, 125% a 150% e 200%); o 256 px é o maior, para o `.ico`.

use std::sync::Arc;

use egui::{ColorImage, Context, Id, Rect, Response, Sense, TextureHandle, TextureOptions, Ui, vec2};

/// (lado em pixels, PNG) do logo, do menor para o maior.
const LOGO: [(u32, &[u8]); 13] = [
    (16, include_bytes!("../assets/logo/databox-16.png")),
    (20, include_bytes!("../assets/logo/databox-20.png")),
    (24, include_bytes!("../assets/logo/databox-24.png")),
    (32, include_bytes!("../assets/logo/databox-32.png")),
    (40, include_bytes!("../assets/logo/databox-40.png")),
    (48, include_bytes!("../assets/logo/databox-48.png")),
    (60, include_bytes!("../assets/logo/databox-60.png")),
    (64, include_bytes!("../assets/logo/databox-64.png")),
    (90, include_bytes!("../assets/logo/databox-90.png")),
    (96, include_bytes!("../assets/logo/databox-96.png")),
    (120, include_bytes!("../assets/logo/databox-120.png")),
    (128, include_bytes!("../assets/logo/databox-128.png")),
    (256, include_bytes!("../assets/logo/databox-256.png")),
];

/// O lado (em pixels) do PNG do ícone da janela, que o sistema reduz para a barra de tarefas e o Alt+Tab.
const LADO_ICONE_JANELA: u32 = 48;

/// PNG → pixels RGBA (não pré-multiplicados), largura e altura. `None` se o PNG não abrir.
pub fn decodificar_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

/// O ícone da janela (o logo completo de 48px), pronto para `ViewportBuilder::with_icon`.
pub fn icone_janela() -> Option<egui::IconData> {
    let (_, bytes) = LOGO.iter().find(|&&(lado, _)| lado == LADO_ICONE_JANELA)?;
    let (rgba, width, height) = decodificar_png(bytes)?;
    Some(egui::IconData { rgba, width, height })
}

/// O PNG do maior logo (256 px): o ícone do atalho fora do Windows (o `.desktop` do Linux).
pub fn logo_png_maior() -> &'static [u8] {
    LOGO.last().map(|&(_, png)| png).unwrap_or_default()
}

/// O índice do PNG para um lado em pixels físicos: o menor que cubra o lado pedido (ou o maior que houver).
fn escolher(pixels: f32) -> usize {
    LOGO.iter().position(|&(l, _)| l as f32 >= pixels - 0.5).unwrap_or(LOGO.len() - 1)
}

#[derive(Clone, Default)]
struct Texturas {
    logo: Vec<Option<TextureHandle>>,
}

fn id_texturas() -> Id {
    Id::new("databox::marca::logos")
}

fn carregar(ctx: &Context, nome: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let (rgba, w, h) = decodificar_png(bytes)?;
    let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    let opcoes = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
    Some(ctx.load_texture(nome, img, opcoes))
}

/// A textura do logo para o lado pedido (carregada na primeira vez que é usada).
fn textura(ctx: &Context, lado_pontos: f32) -> Option<TextureHandle> {
    let i = escolher(lado_pontos * ctx.pixels_per_point());
    let mut t: Arc<Texturas> = ctx.data(|d| d.get_temp(id_texturas())).unwrap_or_default();
    if t.logo.get(i).is_none_or(Option::is_none) {
        let novo = Arc::make_mut(&mut t);
        novo.logo.resize(LOGO.len(), None);
        novo.logo[i] = carregar(ctx, &format!("databox-logo-{}", LOGO[i].0), LOGO[i].1);
        ctx.data_mut(|d| d.insert_temp(id_texturas(), Arc::clone(&t)));
    }
    t.logo.get(i).cloned().flatten()
}

/// Pinta o logo no retângulo (quadrado; sem alocar espaço).
pub fn pintar_logo(ui: &Ui, rect: Rect) {
    if let Some(t) = textura(ui.ctx(), rect.width().min(rect.height())) {
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(t.id(), rect, uv, egui::Color32::WHITE);
    }
}

/// O logo num quadrado de `lado` pontos (16 na barra de título, 60 no Início). Clicável.
pub fn logo(ui: &mut Ui, lado: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(lado, lado), Sense::click());
    if ui.is_rect_visible(rect) {
        pintar_logo(ui, rect);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, "DataBox"));
    resp
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pngs_do_logo_abrem_no_tamanho_do_nome() {
        for &(lado, bytes) in LOGO.iter() {
            let (rgba, w, h) = decodificar_png(bytes).expect("png");
            assert_eq!((w, h), (lado, lado));
            assert_eq!(rgba.len(), (lado * lado * 4) as usize);
            // fundo transparente nos cantos
            assert_eq!(rgba[3], 0, "canto de cima à esquerda opaco no de {lado}px");
            // o desenho está lá: há pixels opacos no meio
            let meio = ((lado / 2 * lado + lado / 2) * 4 + 3) as usize;
            assert!(rgba[meio] > 0, "meio transparente no de {lado}px");
        }
        assert!(LOGO.windows(2).all(|p| p[0].0 < p[1].0), "do menor para o maior");
        let icone = icone_janela().expect("ícone da janela");
        assert_eq!((icone.width, icone.height), (LADO_ICONE_JANELA, LADO_ICONE_JANELA));
        assert!(decodificar_png(b"nada").is_none());
    }

    #[test]
    fn escolhe_o_tamanho_pela_escala_sempre_do_logo_completo() {
        assert_eq!(LOGO[escolher(16.0)].0, 16, "16pt a 100%");
        assert_eq!(LOGO[escolher(20.0)].0, 20, "16pt a 125%");
        assert_eq!(LOGO[escolher(24.0)].0, 24, "16pt a 150%");
        assert_eq!(LOGO[escolher(28.0)].0, 32, "16pt a 175%");
        assert_eq!(LOGO[escolher(32.0)].0, 32, "16pt a 200%");
        assert_eq!(LOGO[escolher(48.0)].0, 48, "16pt a 300%");
        assert_eq!(LOGO[escolher(60.0)].0, 60);
        assert_eq!(LOGO[escolher(75.0)].0, 90, "60pt a 125%");
        assert_eq!(LOGO[escolher(120.0)].0, 120);
        assert_eq!(LOGO[escolher(600.0)].0, 256, "maior que todos: o maior");
    }
}
