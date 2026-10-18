//! Bloco de texto pré-formatado (o SQL de um gatilho, da definição, a prévia de uma mudança): a caixa das listas
//! das pranchetas (fundo `--p`, borda 1px `--b2`, raio 10, `padding:10px 14px`), texto em `--t` que quebra e mantém
//! as quebras, leitura em 70ch. A variante SQL ([`TextoPre::sql`]) é o `.cod` das pranchetas: a fonte do código
//! (Consolas 12.5px em linhas de 20px por padrão; a família e o tamanho das Preferências) com as cores do código
//! ([`crate::codigo`]), na largura toda e com rolagem vertical. Com as cores DataBox a caixa fica no `--p`; com as
//! outras, no fundo delas. O texto pode ser selecionado e copiado (Ctrl+C).

use egui::{FontId, Margin, Response, ScrollArea, Shape, Stroke, StrokeKind, TextEdit, Ui, vec2};

use crate::codigo::{self, CoresCodigo};
use crate::fontes::{self, Peso};
use crate::pecas::texto::meia_entrelinha;
use crate::pintura;
use crate::tema::paleta;

/// Raio da caixa (o das listas e dos números da Visão geral: o raio único, [`pintura::RAIO_PADRAO`]).
const RAIO: f32 = pintura::RAIO_PADRAO;
/// `padding:10px 14px` mais a borda de 1px.
const RECUO_X: f32 = 15.0;
const RECUO_Y: f32 = 11.0;

#[must_use = "chame .mostrar(ui)"]
pub struct TextoPre<'a> {
    texto: &'a str,
    sql: bool,
    altura_max: Option<f32>,
    largura_total: bool,
    apagado: bool,
    id_salt: Option<egui::Id>,
    cores: Option<CoresCodigo>,
}

impl<'a> TextoPre<'a> {
    /// Texto pré-formatado comum (12px na fonte da interface, linhas de 18px, até 70ch).
    pub fn new(texto: &'a str) -> Self {
        TextoPre {
            texto,
            sql: false,
            altura_max: None,
            largura_total: false,
            apagado: false,
            id_salt: None,
            cores: None,
        }
    }

    /// SQL (o `.cod`): a fonte do código (12.5px em linhas de 20px por padrão) com as cores do código, na largura
    /// toda.
    pub fn sql(texto: &'a str) -> Self {
        TextoPre { sql: true, largura_total: true, ..TextoPre::new(texto) }
    }

    /// Só no SQL: estas cores no lugar das da aparência em vigor (as amostras das Preferências, o mostruário).
    pub fn cores(mut self, cores: CoresCodigo) -> Self {
        self.cores = Some(cores);
        self
    }

    /// Altura máxima, com rolagem vertical (320px nos gatilhos, 260px na prévia da janela).
    pub fn altura_max(mut self, altura: f32) -> Self {
        self.altura_max = Some(altura);
        self
    }

    /// Ocupa a largura toda (sem o limite de 70ch).
    pub fn largura_total(mut self, sim: bool) -> Self {
        self.largura_total = sim;
        self
    }

    /// Prévia desatualizada: 60% de opacidade.
    pub fn apagado(mut self, sim: bool) -> Self {
        self.apagado = sim;
        self
    }

    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(egui::Id::new(salt));
        self
    }

    pub fn mostrar(self, ui: &mut Ui) -> Response {
        let p = paleta(ui);
        let e = &p.estudio;
        // `.cod{font-size:12.5px;line-height:20px}` (o tamanho das Preferências, na linha de 1,6); o texto comum fica
        // em 12px / 1.5
        let aparencia = codigo::atual(ui.ctx());
        let (fonte, altura_linha) = if self.sql {
            (FontId::monospace(aparencia.tamanho), codigo::altura_linha(aparencia.tamanho))
        } else {
            (fontes::fonte(12.0, Peso::Normal), 18.0)
        };
        let cores = self.cores.unwrap_or(aparencia.cores);
        let resolvidas = cores.resolver(p);
        let (cor_texto, cor_fundo) = if !self.sql {
            (e.t, e.faixa)
        } else if cores.segue_o_tema() {
            (resolvidas.texto, e.faixa)
        } else {
            (resolvidas.texto, resolvidas.fundo)
        };
        let linha_natural = ui.fonts_mut(|f| f.row_height(&fonte));
        let meia = meia_entrelinha(ui, &fonte, altura_linha);
        // `max-width:70ch` (com `box-sizing:border-box`, o padding e a borda entram nos 70ch)
        let ch = ui.fonts_mut(|f| f.glyph_width(&fonte, '0'));
        let largura = if self.largura_total { ui.available_width() } else { ui.available_width().min(70.0 * ch) };
        let lugar = ui.painter().add(Shape::Noop);
        let id = self.id_salt.unwrap_or_else(|| ui.next_auto_id());
        let mut texto = self.texto;
        let resposta = ui
            .scope(|ui| {
                if self.apagado {
                    ui.multiply_opacity(0.6);
                }
                let extra = (altura_linha - linha_natural).max(0.0);
                ui.spacing_mut().extra_text_line_spacing = extra;
                if self.sql {
                    // o cursor e o texto selecionado na cor do texto do esquema (o Monokai no tema claro)
                    ui.visuals_mut().text_cursor.stroke.color = cor_texto;
                    ui.visuals_mut().selection.stroke.color = cor_texto;
                }
                let sql = self.sql;
                let fonte_realce = fonte.clone();
                let mut realce = move |ui: &Ui, texto: &dyn egui::TextBuffer, largura: f32| {
                    let job = codigo::realce_em_cache(
                        ui.ctx(),
                        texto.as_str(),
                        codigo::Linguagem::Sql,
                        &resolvidas,
                        &fonte_realce,
                        linha_natural + extra,
                        Some(largura),
                    );
                    ui.fonts_mut(|f| f.layout_job(egui::text::LayoutJob::clone(&job)))
                };
                let mut editor = |ui: &mut Ui, texto: &mut &str| {
                    let mut te = TextEdit::multiline(texto);
                    if sql {
                        te = te.layouter(&mut realce);
                    }
                    te.id(id)
                        .font(fonte.clone())
                        .text_color(cor_texto)
                        .frame(egui::Frame::NONE.inner_margin(Margin {
                            left: RECUO_X as i8,
                            right: RECUO_X as i8,
                            top: (RECUO_Y + meia) as i8,
                            bottom: (RECUO_Y - meia).max(0.0) as i8,
                        }))
                        .desired_width(largura)
                        .desired_rows(1)
                        .show(ui)
                        .response
                        .response
                };
                match self.altura_max {
                    Some(h) => {
                        let saida = ScrollArea::vertical()
                            .id_salt(id.with("rolagem"))
                            .max_height(h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| editor(ui, &mut texto));
                        // a área encolhe até o conteúdo quando ele é menor que a altura máxima
                        let r = egui::Rect::from_min_size(
                            saida.inner_rect.min,
                            vec2(saida.inner_rect.width(), saida.content_size.y.min(h)),
                        );
                        (saida.inner, r)
                    }
                    None => {
                        let r = editor(ui, &mut texto);
                        let rect = r.rect;
                        (r, rect)
                    }
                }
            })
            .inner;
        let (resp, rect) = resposta;
        let rect = egui::Rect::from_min_size(rect.min, vec2(largura, rect.height()));
        let opacidade = if self.apagado { 0.6 } else { 1.0 };
        ui.painter().set(
            lugar,
            Shape::Vec(vec![
                Shape::rect_filled(rect, RAIO, cor_fundo.gamma_multiply(opacidade)),
                Shape::rect_stroke(rect, RAIO, Stroke::new(1.0, e.b2.gamma_multiply(opacidade)), StrokeKind::Inside),
            ]),
        );
        resp
    }
}
