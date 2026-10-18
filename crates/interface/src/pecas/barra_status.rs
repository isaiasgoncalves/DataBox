//! A barra de status da casca (o `footer.vidro` de `Main.dc.html`, `Dados.dc.html` e `DadosClaro.dc.html`): o
//! painel fixo de 26px ([`ALTURA_STATUS`]) com o raio único de 10 ([`RAIO_STATUS`]), `padding:0 14px`, `gap:14px`,
//! 11px em `--t2`. À esquerda, o banco (com o ponto de estado de 7px) e o modo; à direita, o que a aba diz de si e
//! do arquivo.
//!
//! Cada item pode ter o ponto, outra cor e peso (o "Transação aberta" em `--aviso` 600), a dica (a frase inteira,
//! que o leitor de tela e os testes leem passando o ponteiro) e um rótulo curto. Sem espaço, os da esquerda
//! encolhem com "…"; os da direita ficam inteiros enquanto sobrar o primeiro da esquerda inteiro (o ponto e o nome
//! do banco): daí em diante, o primeiro da direita (o do documento) encolhe com "…". Nada passa da barra.
//!
//! ```ignore
//! barra_status(ui, &[ItemStatus::new("exemplo.db").ponto(e.ok), ItemStatus::new("Gravação imediata").dica(frase)],
//!     &[ItemStatus::new("SQLite 3.53.2"), ItemStatus::new("UTF-8")]);
//! ```

use egui::{Color32, Id, Rect, Response, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::painel::pintar_painel_fixo;
use crate::pecas::texto::galley_cortado;
use crate::pintura;
use crate::tema::paleta;

/// Altura da barra de status (`height:26px`).
pub const ALTURA_STATUS: f32 = 26.0;
/// Raio da barra de status: o raio único das superfícies (`border-radius:10px`; com os 13 da meia altura ela virava
/// pílula; revisão de 15/10/2026, item 5).
pub const RAIO_STATUS: f32 = pintura::RAIO_PADRAO;
/// `padding:0 14px` e `gap:14px`.
const RECUO: f32 = 14.0;
/// O ponto de estado (7px) e o `gap:6px` até o texto.
const LADO_PONTO: f32 = 7.0;
const GAP_PONTO: f32 = 6.0;

/// Um item da barra de status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemStatus<'a> {
    pub texto: &'a str,
    pub ponto: Option<Color32>,
    pub cor: Option<Color32>,
    pub forte: bool,
    pub dica: Option<&'a str>,
    pub rotulo: Option<&'a str>,
}

impl<'a> ItemStatus<'a> {
    pub fn new(texto: &'a str) -> Self {
        ItemStatus { texto, ponto: None, cor: None, forte: false, dica: None, rotulo: None }
    }

    /// O ponto de 7px antes do texto (`--ok` com o banco aberto, `--aviso` com a transação, `--t3` sem banco).
    pub fn ponto(mut self, cor: Color32) -> Self {
        self.ponto = Some(cor);
        self
    }

    /// O texto em outra cor (no lugar do `--t2`).
    pub fn cor(mut self, cor: Color32) -> Self {
        self.cor = Some(cor);
        self
    }

    /// O texto em 600.
    pub fn forte(mut self) -> Self {
        self.forte = true;
        self
    }

    /// A dica (o `title`): a frase inteira do modo.
    pub fn dica(mut self, dica: &'a str) -> Self {
        self.dica = Some(dica);
        self
    }

    /// O nome para o leitor de tela, quando não é o texto.
    pub fn rotulo(mut self, rotulo: &'a str) -> Self {
        self.rotulo = Some(rotulo);
        self
    }
}

/// O que a barra desenhou: as respostas dos itens (na ordem dada) e o retângulo dela.
#[derive(Debug, Clone)]
pub struct RespostaStatus {
    pub esquerda: Vec<Response>,
    pub direita: Vec<Response>,
    pub rect: Rect,
}

/// A barra de status na largura disponível ([`ALTURA_STATUS`] de altura).
pub fn barra_status(ui: &mut Ui, esquerda: &[ItemStatus<'_>], direita: &[ItemStatus<'_>]) -> RespostaStatus {
    let p = paleta(ui);
    let e = &p.estudio;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ALTURA_STATUS), Sense::hover());
    pintar_painel_fixo(ui.painter(), p, rect, RAIO_STATUS);
    let id = ui.id().with("databox::barra_status");
    let fonte = |item: &ItemStatus<'_>| fontes::fonte(11.0, if item.forte { Peso::Medio } else { Peso::Normal });
    let largura_ponto = |item: &ItemStatus<'_>| if item.ponto.is_some() { LADO_PONTO + GAP_PONTO } else { 0.0 };
    let mut resposta = RespostaStatus { esquerda: Vec::new(), direita: Vec::new(), rect };
    // os itens num Ui cortado na barra: nada passa dela (nem o texto da direita, numa janela estreita demais)
    let mut itens = ui.new_child(UiBuilder::new().max_rect(rect));
    itens.set_clip_rect(rect.intersect(ui.clip_rect()));
    let ui = &itens;

    // a direita, inteira enquanto couber: com o mínimo da esquerda (o primeiro item, o ponto e o nome do banco) e
    // os recuos, o que passar sai do primeiro item da direita (o do documento), cortado com "…"
    let larguras: Vec<f32> = direita
        .iter()
        .map(|item| {
            let g = galley_cortado(ui, item.texto, fonte(item), Color32::PLACEHOLDER, f32::INFINITY);
            largura_ponto(item) + g.size().x
        })
        .collect();
    let minimo_esquerda = esquerda.first().map_or(0.0, |item| {
        let g = galley_cortado(ui, item.texto, fonte(item), Color32::PLACEHOLDER, f32::INFINITY);
        largura_ponto(item) + g.size().x + RECUO
    });
    let total_direita = larguras.iter().sum::<f32>() + RECUO * larguras.len().saturating_sub(1) as f32;
    let cabe_na_direita = (rect.width() - 2.0 * RECUO - minimo_esquerda).max(0.0);
    let sobra = (total_direita - cabe_na_direita).max(0.0);
    let mut x = rect.right() - RECUO;
    let mut direita_desenhada = Vec::with_capacity(direita.len());
    for (i, item) in direita.iter().enumerate().rev() {
        let cor = item.cor.unwrap_or(e.t2);
        let maximo =
            if i == 0 && sobra > 0.0 { (larguras[0] - sobra - largura_ponto(item)).max(0.0) } else { f32::INFINITY };
        let g = galley_cortado(ui, item.texto, fonte(item), cor, maximo);
        let largura = largura_ponto(item) + g.size().x;
        let r = Rect::from_min_size(pos2(x - largura, rect.top()), vec2(largura, rect.height()));
        x = r.left() - RECUO;
        direita_desenhada.push((i, desenhar(ui, id.with(("direita", i)), r, item, g, cor)));
    }
    direita_desenhada.sort_by_key(|(i, _)| *i);
    resposta.direita = direita_desenhada.into_iter().map(|(_, r)| r).collect();

    // a esquerda, com o que sobra
    let limite = x;
    let mut x = rect.left() + RECUO;
    for (i, item) in esquerda.iter().enumerate() {
        let cor = item.cor.unwrap_or(e.t2);
        let espaco = (limite - x - largura_ponto(item)).max(0.0);
        let g = galley_cortado(ui, item.texto, fonte(item), cor, espaco);
        let largura = largura_ponto(item) + g.size().x;
        let r = Rect::from_min_size(pos2(x, rect.top()), vec2(largura, rect.height()));
        x = r.right() + RECUO;
        resposta.esquerda.push(desenhar(ui, id.with(("esquerda", i)), r, item, g, cor));
    }
    resposta
}

fn desenhar(
    ui: &Ui,
    id: Id,
    r: Rect,
    item: &ItemStatus<'_>,
    g: std::sync::Arc<egui::Galley>,
    cor: Color32,
) -> Response {
    let mut x = r.left();
    if let Some(ponto) = item.ponto {
        ui.painter().circle_filled(pos2(x + LADO_PONTO / 2.0, r.center().y), LADO_PONTO / 2.0, ponto);
        x += LADO_PONTO + GAP_PONTO;
    }
    ui.painter().galley(pos2(x, r.center().y - g.size().y / 2.0), g, cor);
    let resp = ui.interact(r, id, Sense::hover());
    let rotulo = item.rotulo.unwrap_or(item.texto);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, rotulo));
    match item.dica {
        Some(d) => resp.on_hover_text(d),
        None => resp,
    }
}
