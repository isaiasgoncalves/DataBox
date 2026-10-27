//! Ficha em linhas (o `.det` das pranchetas, a lista de propriedades): uma grade de [`LARGURA_ROTULO`] (112px) +
//! o resto, com [`ESPACO_COLUNAS`] (10px) entre o rótulo e o valor e 7px entre as linhas, sem fios; rótulo em
//! `--t3` e valor em `--t`, os dois em 12px numa linha de 17px (`line-height:1.4`). O rótulo comprido quebra dentro
//! da coluna; o valor quebra em qualquer ponto (`overflow-wrap:anywhere`). O valor vazio vai em `--t3`.
//! [`Detalhes::titulo_grupo`] é o `.sec` que separa dois `.det` (o "Na grade de dados" da janela de atalhos).
//! [`Detalhes::linha_caminho`] é o Caminho, sempre empilhado (o rótulo em cima), com a peça [`crate::pecas::caminho`].

use egui::{Align, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::texto::{galley_quebrado, mostrar_paragrafo_selecionavel, y_na_linha};
use crate::tema::paleta;

/// Largura da coluna do rótulo (`.det{grid-template-columns:112px minmax(0,1fr)}`).
pub const LARGURA_ROTULO: f32 = 112.0;
/// Espaço entre o rótulo e o valor (`gap:7px 10px`, o segundo).
pub const ESPACO_COLUNAS: f32 = 10.0;
/// O que a coluna do valor precisa, no mínimo, antes de a do rótulo ceder lugar: uma pill de 108px ("AUTOINCREMENTO")
/// tem de caber.
const VALOR_MINIMO: f32 = 110.0;
/// A coluna do rótulo não encolhe abaixo disto; faltando espaço até aí, a ficha empilha (o rótulo em cima do valor).
const ROTULO_MINIMO: f32 = 80.0;
/// Espaço entre o rótulo e o valor de uma linha empilhada.
const ESPACO_EMPILHADO: f32 = 2.0;
/// Espaço entre as linhas (`gap:7px 10px`, o primeiro).
const ESPACO_LINHAS: f32 = 7.0;
/// `line-height:1.4` em 12px.
const ALTURA_LINHA: f32 = 17.0;
/// O `.sec` do título de grupo: 32px.
const ALTURA_SEC: f32 = 32.0;
/// `.det{padding:2px 14px 14px}`: o que vem antes e depois de um título de grupo.
const RECUO_ACIMA_SEC: f32 = 14.0;
const RECUO_ABAIXO_SEC: f32 = 2.0;

/// A largura da coluna do rótulo numa ficha de `largura`, ou `None` quando a ficha empilha. São os [`LARGURA_ROTULO`]
/// das pranchetas enquanto o valor tem [`VALOR_MINIMO`]; numa ficha mais estreita (as Propriedades arrastadas para
/// menos de ~260px) o rótulo cede o que o valor precisa, até [`ROTULO_MINIMO`], e abaixo disso o rótulo vai em cima
/// do valor, que fica com a largura toda (as pills e os botões não passam da borda). Com o painel de 272px e em
/// toda janela, continua 112.
pub fn largura_do_rotulo(largura: f32) -> Option<f32> {
    let rotulo = LARGURA_ROTULO.min(largura - ESPACO_COLUNAS - VALOR_MINIMO);
    (rotulo >= ROTULO_MINIMO).then_some(rotulo)
}

/// Desenha uma ficha; as linhas entram pelo [`Detalhes`].
pub fn detalhes(ui: &mut Ui, conteudo: impl FnOnce(&mut Detalhes<'_>)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut d = Detalhes { ui, linhas: 0, depois_de_titulo: false };
        conteudo(&mut d);
    });
}

/// As linhas de uma ficha.
pub struct Detalhes<'u> {
    ui: &'u mut Ui,
    linhas: usize,
    depois_de_titulo: bool,
}

impl Detalhes<'_> {
    /// Uma linha com o valor desenhado por quem chama.
    pub fn linha<R>(&mut self, rotulo: &str, valor: impl FnOnce(&mut Ui) -> R) -> R {
        self.linha_com(rotulo, false, valor)
    }

    /// O Caminho de um arquivo ou banco ([`crate::pecas::caminho::caminho`], D3 da revisão de 26/10/2026): o rótulo
    /// em cima e o caminho na largura inteira da ficha, em até duas linhas, com o Copiar caminho na ponta. Devolve se o
    /// Copiar foi clicado (quem chama copia e dá o toast).
    pub fn linha_caminho(&mut self, rotulo: &str, caminho: &str) -> bool {
        self.linha_com(rotulo, true, |ui| crate::pecas::caminho::caminho(ui, caminho))
    }

    fn linha_com<R>(&mut self, rotulo: &str, empilhar: bool, valor: impl FnOnce(&mut Ui) -> R) -> R {
        let p = paleta(self.ui);
        let e = &p.estudio;
        let largura = self.ui.available_width();
        if self.depois_de_titulo {
            // o `.det` depois do `.sec` começa com o `padding-top:2px` dele
            self.ui.add_space(RECUO_ABAIXO_SEC);
            self.depois_de_titulo = false;
        } else if self.linhas > 0 {
            self.ui.add_space(ESPACO_LINHAS);
        }
        let topo = self.ui.cursor().min;
        // `dt`: quebra dentro da coluna de 112px (menos, numa ficha estreita; na estreitíssima, a ficha empilha)
        let colunas = if empilhar { None } else { largura_do_rotulo(largura) };
        let gr =
            galley_quebrado(self.ui, rotulo, fontes::fonte(12.0, Peso::Normal), e.t3, colunas.unwrap_or(largura), 1.4);
        let altura_rotulo = gr.tamanho().y.max(ALTURA_LINHA);
        gr.pintar(self.ui.painter(), topo, e.t3);
        let (x_valor, y_valor) = match colunas {
            Some(rotulo) => (topo.x + rotulo + ESPACO_COLUNAS, topo.y),
            None => (topo.x, topo.y + altura_rotulo + ESPACO_EMPILHADO),
        };
        let rect_valor =
            Rect::from_min_size(pos2(x_valor, y_valor), vec2((topo.x + largura - x_valor).max(0.0), f32::INFINITY));
        let n = self.linhas;
        let mut filho = self.ui.new_child(
            UiBuilder::new()
                .id_salt(("databox::detalhes", n))
                .max_rect(rect_valor)
                .layout(Layout::top_down(Align::Min)),
        );
        filho.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let r = valor(&mut filho);
        let fundo_valor =
            if filho.min_rect().is_positive() { filho.min_rect().bottom() } else { y_valor + ALTURA_LINHA };
        // em duas colunas o rótulo também conta na altura; empilhado, o valor já vem depois dele
        let fundo = if colunas.is_some() { fundo_valor.max(topo.y + altura_rotulo) } else { fundo_valor };
        self.ui.allocate_rect(Rect::from_min_max(topo, pos2(topo.x + largura, fundo)), Sense::hover());
        self.linhas += 1;
        r
    }

    /// Uma linha com texto (`dd`: quebra em qualquer ponto), que pode ser selecionado e copiado (o caminho do
    /// arquivo). `None` mostra o `vazio` em `--t3` ("não informado", "—").
    pub fn linha_texto(&mut self, rotulo: &str, valor: Option<&str>, vazio: &str) {
        self.linha(rotulo, |ui| {
            let e = &paleta(ui).estudio;
            let (texto, cor) = match valor {
                Some(v) => (v, e.t),
                None => (vazio, e.t3),
            };
            let g = galley_quebrado(ui, texto, fontes::fonte(12.0, Peso::Normal), cor, ui.available_width(), 1.4);
            mostrar_paragrafo_selecionavel(ui, &g, texto);
        });
    }

    /// Título de grupo dentro da ficha: o `.sec` entre dois `.det` (`.det+.sec{border-top:1px solid --vidrob}`).
    /// Depois de uma linha vêm os 14px de baixo do `.det` e o fio `--vidrob`; o título (12px / 600 em `--t`) fica
    /// numa faixa de 32px, e a linha seguinte começa 2px abaixo dela (o `padding-top` do `.det`).
    pub fn titulo_grupo(&mut self, texto: &str) {
        let p = paleta(self.ui);
        let e = &p.estudio;
        let largura = self.ui.available_width();
        let x = self.ui.cursor().min.x;
        if self.linhas > 0 || self.depois_de_titulo {
            self.ui.add_space(RECUO_ACIMA_SEC);
            let y = self.ui.cursor().min.y;
            self.ui.painter().hline(x..=x + largura, y + 0.5, egui::Stroke::new(1.0, e.vidrob));
            self.ui.add_space(1.0);
        }
        let g = crate::pecas::texto::galley(self.ui, texto, fontes::fonte(12.0, Peso::Medio), e.t);
        let (rect, resp) = self.ui.allocate_exact_size(vec2(largura, ALTURA_SEC), Sense::hover());
        self.ui.painter().galley(pos2(rect.left(), y_na_linha(rect.top(), ALTURA_SEC, &g)), g, e.t);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, texto));
        self.depois_de_titulo = true;
    }

    pub fn ui(&mut self) -> &mut Ui {
        self.ui
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_coluna_do_rotulo_so_cede_em_ficha_estreita_e_empilha_na_estreitissima() {
        // o painel de 272px (ficha de 242px) e qualquer janela: os 112px de sempre
        assert_eq!(largura_do_rotulo(242.0), Some(112.0));
        assert_eq!(largura_do_rotulo(232.0), Some(112.0));
        assert_eq!(largura_do_rotulo(500.0), Some(112.0));
        // as Propriedades arrastadas para ~250px: o valor ganha os 110px que precisa
        assert_eq!(largura_do_rotulo(220.0), Some(100.0));
        assert_eq!(largura_do_rotulo(220.0).map(|r| r + ESPACO_COLUNAS + VALOR_MINIMO), Some(220.0));
        assert_eq!(largura_do_rotulo(200.0), Some(ROTULO_MINIMO));
        // sem lugar nem para o rótulo de 80px, a ficha empilha (o painel de 200px tem uma ficha de 170px)
        assert_eq!(largura_do_rotulo(199.0), None);
        assert_eq!(largura_do_rotulo(170.0), None);
    }
}
