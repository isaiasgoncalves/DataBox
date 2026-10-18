//! Os contadores da Visão geral numa dock (`.dock` de `Main.dc.html`, o CSS entre os comentários "contadores na
//! dock"; pedido de 17/10/2026): os ícones `-grande` de 48pt em pé numa prateleira, com o número e o rótulo embaixo.
//!
//! - **Colunas**: tantas colunas iguais quantos itens quando cada número e cada rótulo cabem na célula com 6pt de
//!   cada lado ([`colunas_da_dock`]: `largura / n − 12 ≥` o maior texto); senão metade por linha (3 + 3), em duas
//!   prateleiras separadas por [`ESPACO_PRATELEIRAS`]. Na prancheta, a regra é a largura da dock (700px, uma
//!   `@container`); aqui, o que decide é o conteúdo. A célula tem [`ALTURA_CELULA`].
//! - **Prateleira** (superfície fixa, sem desfoque): um retângulo por linha, da largura toda, de [`TOPO_PRATELEIRA`]
//!   até o fim da célula, raio [`RAIO_PADRAO`], com o degradê `--dock1 → --dock2`, o brilho branco nos 22pt de cima
//!   (`--docktopo`), o fio `--dockb`, a luz de 1pt em cima (`--dockluz`) e a sombra macia embaixo (o CSS tem
//!   `0 1px 2px` + `0 12px 24px −14px`; o egui não tem espalhamento negativo, então é uma sombra só,
//!   `0 6px 14px`).
//! - **Ícone**: o PNG `-grande` em [`LADO_ICONE`] (48, 72 ou 96px pela escala), centrado, com o topo no topo da
//!   célula descido pelo [`ItemDock::pe`], para todos ficarem em pé na mesma linha ([`LINHA_DO_PE`], 44,5pt):
//!   [`pe_do_icone`] tem o de cada desenho. Embaixo do pé, a sombra de contato (elipse de 36 × 9pt, preto a 55%, 28%
//!   no claro, até transparente) e o reflexo: o mesmo PNG espelhado, só os 8pt mais perto do pé, de branco a 20%
//!   (no pé) a nada.
//! - **Número e rótulo**, centrados: o número em 24pt [`Peso::Leve`] com o topo em 58pt (48 + 10), sem quebra, e a
//!   unidade ("KB") em 12/600 `--t3` na linha de base dele; o rótulo `.tit` (11/600, caixa alta, `--t3`) 3pt
//!   abaixo.
//! - **Parada:** nada se move no hover. A dica do
//!   item ([`ItemDock::dica`]) aparece no hover da célula.
//!
//! O leitor de tela ouve cada célula como "7 Tabelas", "500 KB Tamanho".
//!
//! ```ignore
//! dock(ui, &[
//!     ItemDock::new(Icone::TabelaGrande, "7", "Tabelas"),
//!     ItemDock::new(Icone::DiscoGrande, "500", "Tamanho").unidade("KB"),
//!     ItemDock::new(Icone::LinhasGrande, "5.772", "Linhas (contadas)").dica("Tabelas grandes não são contadas…"),
//! ]);
//! ```

use std::f32::consts::TAU;
use std::sync::Arc;

use egui::epaint::{Mesh, RectShape, Vertex};
use egui::{
    Color32, Galley, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType, pos2,
    vec2,
};

use crate::fontes::{self, Peso};
use crate::icones::{self, Icone};
use crate::pecas::botao::realce_curvo;
use crate::pecas::texto::{ALTURA_TIT, galley, galley_tit, linha_de_base, y_na_linha};
use crate::pintura::{self, RAIO_PADRAO};
use crate::tema::{Paleta, SombraCss, paleta, rgb, rgba};

/// A altura da célula (`.dk{height:108px}`).
pub const ALTURA_CELULA: f32 = 108.0;
/// O topo da prateleira na célula (`.dock-g::before{top:26px}`): ela tem 82pt de altura.
pub const TOPO_PRATELEIRA: f32 = 26.0;
/// O espaço entre as duas prateleiras quando a dock quebra (`row-gap:14px`).
pub const ESPACO_PRATELEIRAS: f32 = 14.0;
/// O lado do ícone (`.dk-ic{width:48px;height:48px}`).
pub const LADO_ICONE: f32 = 48.0;
/// A linha em que os ícones ficam em pé, do topo da célula (o centro da sombra de contato).
pub const LINHA_DO_PE: f32 = 44.5;
/// `.dk{padding:0 6px}`: o texto cabe na célula com 6pt de cada lado.
pub const RECUO_CELULA: f32 = 6.0;

/// O número: 24pt numa linha de 26,4pt (`line-height:1.1`), com o topo 10pt abaixo do ícone.
const TAMANHO_NUMERO: f32 = 24.0;
const LINHA_NUMERO: f32 = 26.4;
const TOPO_NUMERO: f32 = LADO_ICONE + 10.0;
/// A unidade (`font-size:12px;font-weight:600`) depois de um espaço do número (o espaço de 24pt da Segoe UI).
const TAMANHO_UNIDADE: f32 = 12.0;
const ESPACO_UNIDADE: f32 = 0.274 * TAMANHO_NUMERO;
/// `.dk .tit{margin-top:3px}`.
const ESPACO_ROTULO: f32 = 3.0;
/// O brilho branco de cima da prateleira (`linear-gradient(to bottom, --docktopo, transparent 22px)`).
const ALTURA_BRILHO: f32 = 22.0;
/// A sombra de contato (`.dk-ic::before{left:6px;right:6px;top:40px;height:9px}`) e os pontos da borda da malha.
const TAMANHO_CONTATO: Vec2 = vec2(36.0, 9.0);
const PONTOS_CONTATO: usize = 24;
/// O reflexo: os 8pt do desenho mais perto do pé, de branco a 20% a nada.
const ALTURA_REFLEXO: f32 = 8.0;
const ALFA_REFLEXO: f32 = 0.2;

/// As cores da dock (`--dock1`, `--dock2`, `--docktopo`, `--dockb`, `--dockluz`, `--docksomb`, `--contato`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoresDock {
    pub dock1: Color32,
    pub dock2: Color32,
    pub topo: Color32,
    pub borda: Color32,
    pub luz: Color32,
    pub sombra: SombraCss,
    pub contato: Color32,
}

const DOCK_ESCURO: CoresDock = CoresDock {
    dock1: rgb(0x2A, 0x2B, 0x30),
    dock2: rgb(0x21, 0x22, 0x26),
    topo: rgba(255, 255, 255, 0.035),
    borda: rgba(255, 255, 255, 0.09),
    luz: rgba(255, 255, 255, 0.08),
    sombra: SombraCss { y: 6.0, desfoque: 14.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.35) },
    contato: rgba(0, 0, 0, 0.55),
};

const DOCK_CLARO: CoresDock = CoresDock {
    dock1: rgb(0xFB, 0xFB, 0xFC),
    dock2: rgb(0xEE, 0xF0, 0xF3),
    topo: rgba(255, 255, 255, 0.8),
    borda: rgba(0, 0, 0, 0.1),
    luz: rgba(255, 255, 255, 0.95),
    sombra: SombraCss { y: 6.0, desfoque: 14.0, espalhar: 0.0, cor: rgba(0, 0, 0, 0.12) },
    contato: rgba(0, 0, 0, 0.28),
};

/// As cores da dock no tema da paleta.
pub fn cores_dock(p: &Paleta) -> &'static CoresDock {
    if p.escuro { &DOCK_ESCURO } else { &DOCK_CLARO }
}

/// Quanto o desenho `-grande` desce para ficar em pé na [`LINHA_DO_PE`] (`.dk-ic.pe4` etc.): a ficha 4, a visão,
/// o índice e o gatilho 2, as linhas 1 e o disco 0 (o desenho que já toca a linha).
pub fn pe_do_icone(icone: Icone) -> f32 {
    match icone {
        Icone::TabelaGrande => 4.0,
        Icone::VisaoGrande | Icone::IndiceGrande | Icone::GatilhoGrande => 2.0,
        Icone::LinhasGrande => 1.0,
        _ => 0.0,
    }
}

/// Um contador da dock.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemDock {
    pub icone: Icone,
    /// Quanto o ícone desce para ficar em pé na linha (o [`pe_do_icone`] dele, no [`ItemDock::new`]).
    pub pe: f32,
    /// O número já formatado ("7", "5.772", "500").
    pub numero: String,
    /// A unidade depois do número ("KB", "MB").
    pub unidade: Option<String>,
    /// O rótulo, no singular com um ("1 Visão") e no plural com os outros ("7 Tabelas").
    pub rotulo: String,
    /// A dica no hover da célula.
    pub dica: Option<String>,
}

impl ItemDock {
    pub fn new(icone: Icone, numero: impl Into<String>, rotulo: impl Into<String>) -> ItemDock {
        ItemDock {
            icone,
            pe: pe_do_icone(icone),
            numero: numero.into(),
            unidade: None,
            rotulo: rotulo.into(),
            dica: None,
        }
    }

    pub fn unidade(mut self, unidade: impl Into<String>) -> ItemDock {
        self.unidade = Some(unidade.into());
        self
    }

    pub fn dica(mut self, dica: impl Into<String>) -> ItemDock {
        self.dica = Some(dica.into());
        self
    }

    pub fn pe(mut self, pe: f32) -> ItemDock {
        self.pe = pe;
        self
    }

    /// "7 Tabelas", "500 KB Tamanho".
    pub fn rotulo_acessivel(&self) -> String {
        match &self.unidade {
            Some(u) => format!("{} {u} {}", self.numero, self.rotulo),
            None => format!("{} {}", self.numero, self.rotulo),
        }
    }
}

/// Quantas colunas a dock de `n` itens tem na `largura`, com o `maior` texto (número com a unidade, ou rótulo):
/// todos numa linha quando ele cabe na célula com [`RECUO_CELULA`] de cada lado; senão, metade por linha.
pub fn colunas_da_dock(largura: f32, maior: f32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    if largura / n as f32 - 2.0 * RECUO_CELULA >= maior { n } else { n.div_ceil(2) }
}

/// O que a dock desenhou: as colunas e a resposta de cada célula (com a dica do item).
pub struct RespostaDock {
    pub colunas: usize,
    pub celulas: Vec<Response>,
}

/// Os textos de um item já montados.
struct Textos {
    numero: Arc<Galley>,
    unidade: Option<Arc<Galley>>,
    rotulo: Arc<Galley>,
}

impl Textos {
    fn largura_numero(&self) -> f32 {
        self.numero.size().x + self.unidade.as_ref().map_or(0.0, |u| ESPACO_UNIDADE + u.size().x)
    }

    fn maior(&self) -> f32 {
        self.largura_numero().max(self.rotulo.size().x)
    }
}

/// A dock na largura disponível (veja o topo do módulo).
pub fn dock(ui: &mut Ui, itens: &[ItemDock]) -> RespostaDock {
    let p = paleta(ui);
    let e = &p.estudio;
    let cores = cores_dock(p);
    let textos: Vec<Textos> = itens
        .iter()
        .map(|item| Textos {
            numero: galley(ui, &item.numero, fontes::fonte(TAMANHO_NUMERO, Peso::Leve), e.t),
            unidade: item.unidade.as_ref().map(|u| galley(ui, u, fontes::fonte(TAMANHO_UNIDADE, Peso::Medio), e.t3)),
            rotulo: galley_tit(ui, &item.rotulo, e.t3),
        })
        .collect();
    let maior = textos.iter().map(Textos::maior).fold(0.0, f32::max);
    let largura = ui.available_width();
    let colunas = colunas_da_dock(largura, maior, itens.len());
    if colunas == 0 {
        return RespostaDock { colunas, celulas: Vec::new() };
    }
    let linhas = itens.len().div_ceil(colunas);
    let altura = linhas as f32 * ALTURA_CELULA + (linhas as f32 - 1.0) * ESPACO_PRATELEIRAS;
    let base_id = ui.next_auto_id();
    let (total, _) = ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
    let largura_celula = largura / colunas as f32;
    let topo_da_linha = |l: usize| total.top() + l as f32 * (ALTURA_CELULA + ESPACO_PRATELEIRAS);
    if ui.is_rect_visible(total.expand(cores.sombra.desfoque * 2.0)) {
        for l in 0..linhas {
            let y = topo_da_linha(l);
            let prateleira =
                Rect::from_min_max(pos2(total.left(), y + TOPO_PRATELEIRA), pos2(total.right(), y + ALTURA_CELULA));
            pintar_prateleira(ui.painter(), cores, prateleira);
        }
    }
    let mut celulas = Vec::with_capacity(itens.len());
    for (i, (item, t)) in itens.iter().zip(&textos).enumerate() {
        let (l, c) = (i / colunas, i % colunas);
        let celula = Rect::from_min_size(
            pos2(total.left() + c as f32 * largura_celula, topo_da_linha(l)),
            vec2(largura_celula, ALTURA_CELULA),
        );
        let mut resp = ui.interact(celula, base_id.with(i), Sense::hover());
        if ui.is_rect_visible(celula) {
            pintar_item(ui, cores, celula, item, t);
        }
        let rotulo = item.rotulo_acessivel();
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &rotulo));
        if let Some(dica) = &item.dica {
            resp = resp.on_hover_text(dica);
        }
        celulas.push(resp);
    }
    RespostaDock { colunas, celulas }
}

/// A prateleira de uma linha: a sombra macia por fora, o degradê com o brilho de cima, o fio e a luz de 1pt.
fn pintar_prateleira(painter: &Painter, cores: &CoresDock, rect: Rect) {
    let cantos = pintura::canto(rect, RAIO_PADRAO);
    pintura::sombra(painter, rect, RAIO_PADRAO, cores.sombra);
    painter.add(RectShape::filled(rect, cantos, cores.dock2));
    // `--dock1` em cima, sumindo até embaixo, sobre o `--dock2`: o degradê dos dois
    pintura::sombra_interna_topo(painter, rect, RAIO_PADRAO, rect.height(), cores.dock1);
    pintura::sombra_interna_topo(painter, rect, RAIO_PADRAO, ALTURA_BRILHO, cores.topo);
    realce_curvo(painter, rect.shrink(1.0), RAIO_PADRAO - 1.0, cores.luz);
    painter.add(RectShape::stroke(rect, cantos, Stroke::new(1.0, cores.borda), StrokeKind::Inside));
}

/// Uma célula: a sombra de contato, o reflexo, o ícone, o número e o rótulo.
fn pintar_item(ui: &Ui, cores: &CoresDock, celula: Rect, item: &ItemDock, t: &Textos) {
    let e = &paleta(ui).estudio;
    let painter = ui.painter().with_clip_rect(celula.intersect(ui.clip_rect()));
    let cx = celula.center().x;
    painter.add(malha_contato(pos2(cx, celula.top() + LINHA_DO_PE), TAMANHO_CONTATO, cores.contato));
    // o ícone e o reflexo, descidos pelo pé de cada desenho
    let topo_icone = celula.top() + item.pe;
    let quadrado = Rect::from_min_size(pos2(cx - LADO_ICONE / 2.0, topo_icone), Vec2::splat(LADO_ICONE));
    if let Some((textura, lado_png)) = icones::textura_do_icone(ui.ctx(), item.icone, LADO_ICONE) {
        let destino = icones::retangulo_na_tela(quadrado.center(), LADO_ICONE, lado_png, ui.ctx().pixels_per_point());
        painter.add(malha_reflexo(textura, destino, item.pe));
    }
    icones::pintar(&painter, quadrado, item.icone, Color32::WHITE);
    // o número (com a unidade na linha de base dele) e o rótulo, centrados
    let topo_numero = celula.top() + TOPO_NUMERO;
    let x = cx - t.largura_numero() / 2.0;
    let y_numero = y_na_linha(topo_numero, LINHA_NUMERO, &t.numero);
    let base = y_numero + linha_de_base(&t.numero);
    let largura_numero = t.numero.size().x;
    painter.galley(pos2(x, y_numero), Arc::clone(&t.numero), e.t);
    if let Some(u) = &t.unidade {
        painter.galley(pos2(x + largura_numero + ESPACO_UNIDADE, base - linha_de_base(u)), Arc::clone(u), e.t3);
    }
    let topo_rotulo = topo_numero + LINHA_NUMERO + ESPACO_ROTULO;
    let r = &t.rotulo;
    painter.galley(pos2(cx - r.size().x / 2.0, y_na_linha(topo_rotulo, ALTURA_TIT, r)), Arc::clone(r), e.t3);
}

/// A sombra de contato: uma elipse em leque, com a cor no centro e transparente na borda (o
/// `radial-gradient(closest-side, --contato, transparent)`).
fn malha_contato(centro: Pos2, tamanho: Vec2, cor: Color32) -> Mesh {
    let mut m = Mesh::default();
    m.colored_vertex(centro, cor);
    for k in 0..PONTOS_CONTATO {
        let a = TAU * k as f32 / PONTOS_CONTATO as f32;
        m.colored_vertex(centro + vec2(a.cos() * tamanho.x / 2.0, a.sin() * tamanho.y / 2.0), Color32::TRANSPARENT);
    }
    for k in 0..PONTOS_CONTATO as u32 {
        m.add_triangle(0, 1 + k, 1 + (k + 1) % PONTOS_CONTATO as u32);
    }
    m
}

/// O reflexo do ícone pintado em `destino`, descido `pe`: os [`ALTURA_REFLEXO`] pontos do desenho logo acima do pé,
/// espelhados logo abaixo dele (a uv invertida na vertical), de branco a 20% no pé a nada embaixo (o
/// `-webkit-box-reflect` com a máscara da prancheta).
fn malha_reflexo(textura: egui::TextureId, destino: Rect, pe: f32) -> Mesh {
    let escala = destino.height() / LADO_ICONE;
    let pe_no_desenho = LINHA_DO_PE - pe;
    let y_pe = destino.top() + pe_no_desenho * escala;
    let y_fim = y_pe + ALTURA_REFLEXO * escala;
    let (v_pe, v_fim) = (pe_no_desenho / LADO_ICONE, (pe_no_desenho - ALTURA_REFLEXO) / LADO_ICONE);
    let cheio = Color32::WHITE.gamma_multiply(ALFA_REFLEXO);
    let mut m = Mesh::with_texture(textura);
    let vertice = |x: f32, y: f32, u: f32, v: f32, cor: Color32| Vertex { pos: pos2(x, y), uv: pos2(u, v), color: cor };
    m.vertices.extend([
        vertice(destino.left(), y_pe, 0.0, v_pe, cheio),
        vertice(destino.right(), y_pe, 1.0, v_pe, cheio),
        vertice(destino.right(), y_fim, 1.0, v_fim, Color32::TRANSPARENT),
        vertice(destino.left(), y_fim, 0.0, v_fim, Color32::TRANSPARENT),
    ]);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    m
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn colunas_pelo_conteudo() {
        // 776pt: células de 129pt, 117 por dentro
        assert_eq!(colunas_da_dock(776.0, 117.0, 6), 6);
        assert_eq!(colunas_da_dock(776.0, 117.5, 6), 3);
        assert_eq!(colunas_da_dock(300.0, 10.0, 0), 0);
        assert_eq!(colunas_da_dock(100.0, 80.0, 5), 3);
    }

    #[test]
    fn todos_em_pe_na_mesma_linha() {
        for (icone, pe) in [
            (Icone::TabelaGrande, 4.0),
            (Icone::VisaoGrande, 2.0),
            (Icone::IndiceGrande, 2.0),
            (Icone::GatilhoGrande, 2.0),
            (Icone::DiscoGrande, 0.0),
            (Icone::LinhasGrande, 1.0),
        ] {
            assert_eq!(ItemDock::new(icone, "1", "x").pe, pe, "{icone:?}");
        }
        let item = ItemDock::new(Icone::DiscoGrande, "500", "Tamanho").unidade("KB");
        assert_eq!(item.rotulo_acessivel(), "500 KB Tamanho");
    }

    #[test]
    fn reflexo_espelha_os_8pt_acima_do_pe() {
        let destino = Rect::from_min_size(pos2(10.0, 4.0), Vec2::splat(LADO_ICONE));
        let m = malha_reflexo(egui::TextureId::Managed(1), destino, 4.0);
        // o pé em 4 + 40,5 = 44,5; o reflexo vai até 52,5, com a uv de 40,5/48 a 32,5/48
        assert_eq!((m.vertices[0].pos.y, m.vertices[2].pos.y), (44.5, 52.5));
        assert!((m.vertices[0].uv.y - 40.5 / 48.0).abs() < 1e-6 && (m.vertices[3].uv.y - 32.5 / 48.0).abs() < 1e-6);
        assert_eq!(m.vertices[3].color, Color32::TRANSPARENT);
    }
}
