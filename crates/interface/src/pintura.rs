//! Pintura de baixo nível: degradês (em texturas pequenas criadas uma vez), a textura de riscas finas a 135°,
//! sombras externas e internas, halos, os anéis dos botões, o anel de foco e o fundo da página. As peças usam
//! estas funções; uma tela só precisa delas para desenhar algo que as peças não têm.

use std::sync::Arc;

use egui::epaint::{Mesh, PathShape, PathStroke, RectShape, Vertex, WHITE_UV};
use egui::{
    Color32, ColorImage, Context, CornerRadius, Id, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle,
    TextureId, TextureOptions, Vec2, pos2, vec2,
};

use crate::tema::{AnelAqua, CLARA, ESCURA, Paleta, SombraCss};

/// Como pintar o fundo de uma peça.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Preenchimento {
    /// Cor lisa.
    Liso(Color32),
    /// `--volneu` (`--bt1` → `--bt2`): botão neutro, item ativo, etiqueta neutra, aviso.
    Aqua,
    /// `--volpri` (destaque clareado → destaque a 50% → destaque escurecido): botão principal, caixa marcada.
    Primario,
    /// `--c` liso (documento, cartão).
    Superficie,
    /// `--volrisco` (vermelho): botão de perigo (Interromper, Excluir).
    Risco,
    /// `--bt1` → meio (aba ativa: a metade de cima da "superfície só" com a faixa embaixo).
    Aba,
    /// meio → `--bt2` (faixa sob a aba, cabeçalho de grade e de tabela, cabeçalho de painel).
    Cabecalho,
}

#[derive(Clone)]
struct Texturas {
    /// claro, escuro
    aqua: [TextureHandle; 2],
    primario: [TextureHandle; 2],
    superficie: [TextureHandle; 2],
    risco: [TextureHandle; 2],
    aba: [TextureHandle; 2],
    cabecalho: [TextureHandle; 2],
    /// rampa de alfa (transparente em cima → branco embaixo), para as sombras internas
    rampa: TextureHandle,
}

fn id_texturas() -> Id {
    Id::new("databox::pintura::texturas")
}

/// Degradê vertical com paradas `(posição 0..1, cor)`, numa textura de 1×64.
fn imagem_degrade(paradas: &[(f32, Color32)]) -> ColorImage {
    const ALTURA: usize = 64;
    let mut pixels = Vec::with_capacity(ALTURA);
    for i in 0..ALTURA {
        let t = (i as f32 + 0.5) / ALTURA as f32;
        pixels.push(cor_no_degrade(paradas, t));
    }
    ColorImage::new([1, ALTURA], pixels)
}

/// Cor de um degradê na posição `t` (interpolação linear das cores pré-multiplicadas, como o CSS).
pub fn cor_no_degrade(paradas: &[(f32, Color32)], t: f32) -> Color32 {
    let Some(&(p0, c0)) = paradas.first() else { return Color32::TRANSPARENT };
    if t <= p0 {
        return c0;
    }
    for par in paradas.windows(2) {
        let [(pa, ca), (pb, cb)] = [par[0], par[1]];
        if t <= pb {
            let f = if pb > pa { (t - pa) / (pb - pa) } else { 1.0 };
            return misturar(ca, cb, f);
        }
    }
    paradas.last().map_or(Color32::TRANSPARENT, |&(_, c)| c)
}

/// Mistura linear de duas cores pré-multiplicadas.
pub fn misturar(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let canal = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(canal(a.r(), b.r()), canal(a.g(), b.g()), canal(a.b(), b.b()), canal(a.a(), b.a()))
}

fn paradas_aqua(p: &Paleta) -> [(f32, Color32); 2] {
    [(0.0, p.estudio.bt1), (1.0, p.estudio.bt2)]
}

/// As paradas do `--volpri` (o botão principal, as caixas marcadas e a barra de progresso): o topo claro, o
/// destaque já a 25% da altura (era 50%: o branco do topo apagava as letras; revisão de 15/10/2026, item 6) e a
/// base escura.
pub fn paradas_primario(p: &Paleta) -> [(f32, Color32); 3] {
    [(0.0, p.estudio.pri_topo), (0.25, p.estudio.acc), (1.0, p.estudio.pri_base)]
}

fn paradas_superficie(p: &Paleta) -> [(f32, Color32); 2] {
    [(0.0, p.surface_topo), (1.0, p.surface_base)]
}

fn paradas_risco(p: &Paleta) -> [(f32, Color32); 3] {
    [(0.0, p.estudio.risco_topo), (0.5, p.estudio.risco_meio), (1.0, p.estudio.risco_base)]
}

fn paradas_aba(p: &Paleta) -> [(f32, Color32); 2] {
    [(0.0, p.estudio.bt1), (1.0, p.estudio.sup_meio)]
}

fn paradas_cabecalho(p: &Paleta) -> [(f32, Color32); 2] {
    [(0.0, p.estudio.sup_meio), (1.0, p.estudio.bt2)]
}

/// O passo das riscas na diagonal, em pixels físicos (uma risca de 1px a cada 4px).
const PASSO_RISCAS: f32 = 4.0;

/// A cor de uma risca: o alfa corrigido para a mesma tinta média do CSS (`repeating-linear-gradient(135deg, cor 0
/// 1px, transparent 1px 3px)`: 1 de cada 3px lá; 1 de cada 4px aqui).
fn cor_das_riscas(cor: Color32) -> Color32 {
    let [r, g, b, a] = cor.to_srgba_unmultiplied();
    let alfa = (f32::from(a) * 4.0 / 3.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, alfa)
}

/// Cria as texturas dos degradês e da rampa (chamado por [`crate::tema::aplicar`]).
pub(crate) fn criar_texturas(ctx: &Context) {
    let carregar = |nome: &str, img: ColorImage| ctx.load_texture(nome, img, TextureOptions::LINEAR);
    let par = |nome: &str, f: &dyn Fn(&Paleta) -> ColorImage| -> [TextureHandle; 2] {
        [carregar(&format!("{nome}-claro"), f(&CLARA)), carregar(&format!("{nome}-escuro"), f(&ESCURA))]
    };
    let t = Texturas {
        aqua: par("databox-neutro", &|p| imagem_degrade(&paradas_aqua(p))),
        primario: par("databox-primario", &|p| imagem_degrade(&paradas_primario(p))),
        superficie: par("databox-superficie", &|p| imagem_degrade(&paradas_superficie(p))),
        risco: par("databox-risco", &|p| imagem_degrade(&paradas_risco(p))),
        aba: par("databox-aba", &|p| imagem_degrade(&paradas_aba(p))),
        cabecalho: par("databox-cabecalho", &|p| imagem_degrade(&paradas_cabecalho(p))),
        rampa: carregar("databox-rampa", imagem_degrade(&[(0.0, Color32::TRANSPARENT), (1.0, Color32::WHITE)])),
    };
    ctx.data_mut(|d| d.insert_temp(id_texturas(), Arc::new(t)));
}

fn texturas(ctx: &Context) -> Option<Arc<Texturas>> {
    ctx.data(|d| d.get_temp(id_texturas()))
}

fn textura(ctx: &Context, p: &Paleta, qual: Preenchimento) -> Option<TextureId> {
    let t = texturas(ctx)?;
    let i = usize::from(p.escuro);
    Some(match qual {
        Preenchimento::Aqua => t.aqua[i].id(),
        Preenchimento::Primario => t.primario[i].id(),
        Preenchimento::Superficie => t.superficie[i].id(),
        Preenchimento::Risco => t.risco[i].id(),
        Preenchimento::Aba => t.aba[i].id(),
        Preenchimento::Cabecalho => t.cabecalho[i].id(),
        Preenchimento::Liso(_) => return None,
    })
}

/// Cor média de um preenchimento (para quando a textura não existe ainda).
pub fn cor_media(p: &Paleta, qual: Preenchimento) -> Color32 {
    let e = &p.estudio;
    match qual {
        Preenchimento::Liso(c) => c,
        Preenchimento::Aqua => e.sup_meio,
        Preenchimento::Primario => e.acc,
        Preenchimento::Superficie => misturar(p.surface_topo, p.surface_base, 0.5),
        Preenchimento::Risco => e.risco_meio,
        Preenchimento::Aba => misturar(e.bt1, e.sup_meio, 0.5),
        Preenchimento::Cabecalho => misturar(e.sup_meio, e.bt2, 0.5),
    }
}

/// O raio único das superfícies e dos campos (`border-radius:10px`): documentos e cartões, painéis, janelas, abas,
/// campos de texto, busca, listas suspensas e áreas de texto. Trocar o valor aqui troca todos juntos. Botões, menus
/// do topo, etiquetas, seletores, a barra de progresso e o aviso flutuante (toast) são pílula (raio
/// `f32::INFINITY`); o vidro dos menus tem 12, os itens de menu e da árvore 6 e as caixas de marcar 5. O aviso
/// dentro de cartões e janelas e a barra de status usam este raio.
pub const RAIO_PADRAO: f32 = 10.0;

/// Raio de canto em pontos, limitado à metade do lado menor (o `999px` da pílula vira meia altura).
pub fn raio(rect: Rect, r: f32) -> f32 {
    r.min(rect.height() / 2.0).min(rect.width() / 2.0).max(0.0)
}

/// O `CornerRadius` do egui para um raio em pontos (limitado como em [`raio`]).
pub fn canto(rect: Rect, r: f32) -> CornerRadius {
    CornerRadius::same(raio(rect, r).round().clamp(0.0, 255.0) as u8)
}

/// As riscas finas de 135° (`--textura`) por cima de um fundo já pintado, presas à janela (como o
/// `background-attachment: fixed` das pranchetas: a aba ativa e a faixa embaixo dela ficam contínuas).
pub fn riscas(painter: &Painter, p: &Paleta, rect: Rect, r: f32) {
    riscas_com_canto(painter, p, rect, canto(rect, r));
}

/// [`riscas`] com um raio por canto (a aba ativa tem só os de cima arredondados).
///
/// As riscas são geometria (uma faixa de 1px a cada 4px na diagonal, cortada pelo contorno da peça), e não uma
/// textura em repetição: o renderizador de testes do egui (`egui_kittest`, com a filtragem previsível) ignora o
/// modo de repetição e a textura não aparecia nas capturas; assim as riscas saem iguais em qualquer renderizador.
pub fn riscas_com_canto(painter: &Painter, p: &Paleta, rect: Rect, cantos: CornerRadius) {
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let cor = cor_das_riscas(p.estudio.riscas);
    if cor.a() == 0 {
        return;
    }
    let ppp = painter.ctx().pixels_per_point();
    let (passo, espessura) = (PASSO_RISCAS / ppp, 1.0 / ppp);
    // a risca k é a faixa `k·passo + folga ≤ x + y < k·passo + folga + espessura` (a folga põe a borda longe do
    // centro dos pixels: uma risca de exatamente 1 pixel por linha)
    let folga = 0.25 / ppp;
    let contorno = contorno_arredondado(rect, cantos);
    let inicio = ((rect.min.x + rect.min.y - folga - espessura) / passo).floor() as i64;
    let fim = ((rect.max.x + rect.max.y - folga) / passo).ceil() as i64;
    let mut malha = Mesh::default();
    for k in inicio..=fim {
        let s0 = k as f32 * passo + folga;
        let faixa = recortar_faixa(&contorno, s0, s0 + espessura);
        if faixa.len() < 3 {
            continue;
        }
        let base = malha.vertices.len() as u32;
        for ponto in &faixa {
            malha.colored_vertex(*ponto, cor);
        }
        for i in 1..faixa.len() as u32 - 1 {
            malha.add_triangle(base, base + i, base + i + 1);
        }
    }
    if !malha.is_empty() {
        painter.add(Shape::mesh(malha));
    }
}

/// O contorno convexo de um retângulo de cantos arredondados (arcos de três passos), no sentido horário da tela.
fn contorno_arredondado(rect: Rect, cantos: CornerRadius) -> Vec<Pos2> {
    use std::f32::consts::FRAC_PI_2;
    let limite = rect.width().min(rect.height()) / 2.0;
    let raio = |r: u8| f32::from(r).min(limite);
    // (centro do arco, ângulo inicial, raio)
    let arcos = [
        (pos2(rect.left() + raio(cantos.nw), rect.top() + raio(cantos.nw)), 2.0 * FRAC_PI_2, raio(cantos.nw)),
        (pos2(rect.right() - raio(cantos.ne), rect.top() + raio(cantos.ne)), 3.0 * FRAC_PI_2, raio(cantos.ne)),
        (pos2(rect.right() - raio(cantos.se), rect.bottom() - raio(cantos.se)), 0.0, raio(cantos.se)),
        (pos2(rect.left() + raio(cantos.sw), rect.bottom() - raio(cantos.sw)), FRAC_PI_2, raio(cantos.sw)),
    ];
    let mut pontos = Vec::with_capacity(16);
    for (centro, inicio, r) in arcos {
        if r < 0.5 {
            pontos.push(centro);
            continue;
        }
        for i in 0..=3 {
            let a = inicio + FRAC_PI_2 * i as f32 / 3.0;
            pontos.push(centro + vec2(a.cos(), a.sin()) * r);
        }
    }
    pontos
}

/// A parte de um polígono convexo onde `s0 ≤ x + y ≤ s1` (Sutherland–Hodgman contra os dois semiplanos).
fn recortar_faixa(poligono: &[Pos2], s0: f32, s1: f32) -> Vec<Pos2> {
    let recortar = |entrada: &[Pos2], dentro: &dyn Fn(Pos2) -> f32| -> Vec<Pos2> {
        let mut saida = Vec::with_capacity(entrada.len() + 2);
        for (i, &a) in entrada.iter().enumerate() {
            let b = entrada[(i + 1) % entrada.len()];
            let (da, db) = (dentro(a), dentro(b));
            if da >= 0.0 {
                saida.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                saida.push(a + (b - a) * (da / (da - db)));
            }
        }
        saida
    };
    let acima = recortar(poligono, &|q| q.x + q.y - s0);
    if acima.len() < 3 {
        return Vec::new();
    }
    recortar(&acima, &|q| s1 - q.x - q.y)
}

/// Sombra interna embaixo (`inset 0 -<altura>px <altura+1>px cor`, o volume dos botões e das caixas): uma rampa da
/// cor, de transparente até `altura` pontos acima da base, recortada pelos cantos da peça.
pub fn sombra_interna_baixo(painter: &Painter, rect: Rect, r: f32, altura: f32, cor: Color32) {
    let Some(t) = texturas(painter.ctx()) else { return };
    if cor.a() == 0 || rect.height() <= 0.0 || altura <= 0.0 {
        return;
    }
    let inicio = 1.0 - rect.height() / altura;
    let uv = Rect::from_min_max(pos2(0.0, inicio), pos2(1.0, 1.0));
    painter.add(RectShape::filled(rect, canto(rect, r), cor).with_texture(t.rampa.id(), uv));
}

/// Sombra interna em cima (`inset 0 1px 2px cor`, o baixo-relevo dos campos e do trilho da barra de progresso):
/// a rampa invertida, da cor no topo até transparente `altura` pontos abaixo.
pub fn sombra_interna_topo(painter: &Painter, rect: Rect, r: f32, altura: f32, cor: Color32) {
    let Some(t) = texturas(painter.ctx()) else { return };
    if cor.a() == 0 || rect.height() <= 0.0 || altura <= 0.0 {
        return;
    }
    let fim = rect.height() / altura;
    // a rampa vai de transparente (0) a cheia (1); de cabeça para baixo, a cheia fica no topo
    let uv = Rect::from_min_max(pos2(0.0, 1.0), pos2(1.0, 1.0 - fim));
    painter.add(RectShape::filled(rect, canto(rect, r), cor).with_texture(t.rampa.id(), uv));
}

/// Halo em volta da peça (`0 0 <desfoque>px cor`: o brilho do hover e do foco), só por fora.
pub fn halo(painter: &Painter, rect: Rect, r: f32, desfoque: f32, cor: Color32) {
    sombra(painter, rect, r, SombraCss { y: 0.0, desfoque, espalhar: 0.0, cor });
}

/// Pinta o fundo de uma peça com raio `r` (use `f32::INFINITY` para pill).
pub fn preencher(painter: &Painter, p: &Paleta, rect: Rect, r: f32, qual: Preenchimento) {
    let forma = RectShape::filled(rect, canto(rect, r), Color32::WHITE);
    match textura(painter.ctx(), p, qual) {
        Some(id) => {
            painter.add(forma.with_texture(id, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))));
        }
        None => {
            painter.add(RectShape::filled(rect, canto(rect, r), cor_media(p, qual)));
        }
    }
}

/// O mesmo que [`preencher`], devolvendo a forma (para pintar depois num lugar reservado).
pub fn forma_preenchida(ctx: &Context, p: &Paleta, rect: Rect, r: f32, qual: Preenchimento) -> Shape {
    match textura(ctx, p, qual) {
        Some(id) => RectShape::filled(rect, canto(rect, r), Color32::WHITE)
            .with_texture(id, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))
            .into(),
        None => RectShape::filled(rect, canto(rect, r), cor_media(p, qual)).into(),
    }
}

/// Sombra externa do CSS (`0 y desfoque espalhar cor`) de um retângulo de raio `r`, pintada **só fora** da
/// peça, como o `box-shadow` externo (que nunca aparece embaixo da caixa: com o fundo translúcido do tema
/// escuro, uma sombra cheia por baixo escureceria a peça). É uma malha em anéis em volta do contorno, com a
/// gaussiana do CSS (σ = desfoque/2) calculada em cada vértice e um furo exatamente no contorno.
///
/// As malhas ficam guardadas por tamanho, raio e geometria da sombra, com a intensidade (0 a 1) de cada vértice; a
/// cor entra na cópia de cada chamada. Só a posição e a cor mudam de um quadro para o outro: uma transição de hover
/// ou de foco (a cor do halo muda a cada quadro) não cria malha nova.
pub fn forma_sombra(rect: Rect, r: f32, s: SombraCss) -> Shape {
    if s.cor.a() == 0 || !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Shape::Noop;
    }
    let chave = ChaveSombra {
        tamanho: [rect.width().to_bits(), rect.height().to_bits()],
        raio: raio(rect, r).to_bits(),
        sombra: [s.y.to_bits(), s.desfoque.to_bits(), s.espalhar.to_bits()],
    };
    let guardada = CACHE_SOMBRAS.with(|c| c.borrow_mut().buscar(chave, || malha_sombra(rect.size(), r, s)));
    if guardada.malha.is_empty() {
        return Shape::Noop;
    }
    let mut m = guardada.malha.clone();
    for (v, &forca) in m.vertices.iter_mut().zip(&guardada.forca) {
        v.color = s.cor.gamma_multiply(forca);
    }
    m.translate(rect.min.to_vec2());
    Shape::mesh(m)
}

pub fn sombra(painter: &Painter, rect: Rect, r: f32, s: SombraCss) {
    painter.add(forma_sombra(rect, r, s));
}

/// O que identifica a malha de uma sombra (a cor fica de fora: entra na cópia).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ChaveSombra {
    tamanho: [u32; 2],
    raio: u32,
    sombra: [u32; 3],
}

/// A malha de uma sombra com o canto de cima à esquerda na origem e a intensidade de cada vértice (a cor dos
/// vértices da malha é ignorada).
struct MalhaSombra {
    malha: Mesh,
    forca: Vec<f32>,
}

/// Quantas malhas de sombra ficam guardadas.
const LIMITE_CACHE_SOMBRAS: usize = 512;

/// As malhas guardadas, com o último uso de cada uma. Cheio, o cache solta a metade usada há mais tempo: as sombras
/// paradas da tela, pedidas a cada quadro, ficam (uma largura animada, como a da barra de progresso, gera uma
/// malha por quadro, mas não tira as outras do cache).
#[derive(Default)]
struct CacheSombras {
    malhas: std::collections::HashMap<ChaveSombra, (Arc<MalhaSombra>, u64)>,
    relogio: u64,
}

impl CacheSombras {
    fn buscar(&mut self, chave: ChaveSombra, criar: impl FnOnce() -> MalhaSombra) -> Arc<MalhaSombra> {
        self.relogio += 1;
        let agora = self.relogio;
        if let Some((m, uso)) = self.malhas.get_mut(&chave) {
            *uso = agora;
            return Arc::clone(m);
        }
        if self.malhas.len() >= LIMITE_CACHE_SOMBRAS {
            let mut usos: Vec<u64> = self.malhas.values().map(|(_, u)| *u).collect();
            let meio = usos.len() / 2;
            let corte = *usos.select_nth_unstable(meio).1;
            self.malhas.retain(|_, (_, u)| *u >= corte);
        }
        let m = Arc::new(criar());
        self.malhas.insert(chave, (Arc::clone(&m), agora));
        m
    }
}

thread_local! {
    static CACHE_SOMBRAS: std::cell::RefCell<CacheSombras> = std::cell::RefCell::new(CacheSombras::default());
}

/// `erf` com erro abaixo de 5·10⁻⁴ (a aproximação de Abramowitz e Stegun que o Evan Wallace usa nas sombras).
fn erf(x: f32) -> f32 {
    let s = x.signum();
    let a = x.abs();
    let t = 1.0 + (0.278_393 + (0.230_389 + 0.078_108 * a * a) * a) * a;
    let t2 = t * t;
    s - s / (t2 * t2)
}

/// Fração de uma gaussiana 1D (σ) de um segmento horizontal na altura `y` de um retângulo arredondado
/// centrado na origem, vista do ponto `x`.
fn faixa_x(x: f32, y: f32, sigma: f32, canto: f32, meia: Vec2) -> f32 {
    let delta = (meia.y - canto - y.abs()).min(0.0);
    let curva = meia.x - canto + (canto * canto - delta * delta).max(0.0).sqrt();
    let k = std::f32::consts::FRAC_1_SQRT_2 / sigma;
    0.5 * (erf((x + curva) * k) - erf((x - curva) * k))
}

/// Intensidade (0 a 1) da sombra gaussiana de um retângulo arredondado (`centro`, `meia` = metade do tamanho,
/// `canto` = raio) no ponto `p`: integral exata em x e numérica em y (o método do Evan Wallace).
fn intensidade(p: Pos2, centro: Pos2, meia: Vec2, canto: f32, sigma: f32) -> f32 {
    const AMOSTRAS: usize = 8;
    let px = p.x - centro.x;
    let py = p.y - centro.y;
    let (baixo, alto) = (py - meia.y, py + meia.y);
    let inicio = (-3.0 * sigma).clamp(baixo, alto);
    let fim = (3.0 * sigma).clamp(baixo, alto);
    let passo = (fim - inicio) / AMOSTRAS as f32;
    let norma = 1.0 / ((std::f32::consts::TAU).sqrt() * sigma);
    let mut y = inicio + passo * 0.5;
    let mut valor = 0.0;
    for _ in 0..AMOSTRAS {
        let gauss = (-(y * y) / (2.0 * sigma * sigma)).exp() * norma;
        valor += faixa_x(px, py - y, sigma, canto, meia) * gauss * passo;
        y += passo;
    }
    valor.clamp(0.0, 1.0)
}

/// Distâncias ao longo de um lado reto de comprimento `l`: densas perto das pontas (onde a sombra muda) e uma
/// no meio. Não inclui as pontas.
fn pontos_do_lado(l: f32, zona: f32, passo: f32) -> Vec<f32> {
    let mut d = Vec::new();
    if l <= 0.5 {
        return d;
    }
    let zona = zona.min(l / 2.0);
    let mut t = passo;
    while t < zona - 0.25 {
        d.push(t);
        t += passo;
    }
    let meio = l / 2.0;
    if d.last().is_none_or(|&u| meio - u > 0.25) {
        d.push(meio);
    }
    let espelho: Vec<f32> = d.iter().rev().filter(|&&u| l - u > meio + 0.25).map(|&u| l - u).collect();
    d.extend(espelho);
    d
}

/// A malha da sombra de uma peça de tamanho `tamanho` com o canto de cima à esquerda na origem, e a intensidade de
/// cada vértice (a cor da sombra fica de fora).
fn malha_sombra(tamanho: Vec2, r: f32, s: SombraCss) -> MalhaSombra {
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut m = Mesh::default();
    let mut forcas = Vec::new();
    let peca = Rect::from_min_size(Pos2::ZERO, tamanho);
    let rr = raio(peca, r);
    let sigma = (s.desfoque / 2.0).max(0.25);
    let alvo = peca.translate(vec2(0.0, s.y)).expand(s.espalhar);
    if alvo.width() <= 0.0 || alvo.height() <= 0.0 {
        return MalhaSombra { malha: m, forca: forcas };
    }
    let canto_alvo = raio(alvo, rr + s.espalhar);
    let (centro_alvo, meia_alvo) = (alvo.center(), alvo.size() / 2.0);
    // até onde a sombra vai além do contorno da peça
    let alcance = s.y.abs() + s.espalhar.max(0.0) + 3.0 * sigma;
    if alcance <= 0.0 {
        return MalhaSombra { malha: m, forca: forcas };
    }
    let passo = (0.5 * sigma).max(0.75);
    let aneis = ((alcance / passo).ceil() as usize).clamp(2, 24);
    let por_canto = (((rr + alcance) * FRAC_PI_2 / 3.0).ceil() as usize).clamp(2, 12);
    let zona = 3.5 * sigma + s.y.abs() + s.espalhar.max(0.0);
    let lado_h = pontos_do_lado(tamanho.x - 2.0 * rr, zona, passo.max(1.0));
    let lado_v = pontos_do_lado(tamanho.y - 2.0 * rr, zona, passo.max(1.0));
    // os cantos (centro dos arcos, ângulo inicial) e o lado reto que vem depois de cada um, no sentido horário
    let cantos = [
        (pos2(rr, rr), PI, vec2(1.0, 0.0), &lado_h),
        (pos2(tamanho.x - rr, rr), PI + FRAC_PI_2, vec2(0.0, 1.0), &lado_v),
        (pos2(tamanho.x - rr, tamanho.y - rr), 0.0, vec2(-1.0, 0.0), &lado_h),
        (pos2(rr, tamanho.y - rr), FRAC_PI_2, vec2(0.0, -1.0), &lado_v),
    ];
    let mut contorno = Vec::new();
    for &(centro, a0, direcao, lado) in &cantos {
        for j in 0..por_canto {
            let a = a0 + FRAC_PI_2 * j as f32 / (por_canto - 1) as f32;
            contorno.push((centro, vec2(a.cos(), a.sin())));
        }
        // o lado reto até o próximo canto, empurrado para fora pela normal do fim do arco
        let normal = vec2((a0 + FRAC_PI_2).cos(), (a0 + FRAC_PI_2).sin());
        for &d in lado.iter() {
            contorno.push((centro + direcao * d, normal));
        }
    }
    let n = contorno.len();
    m.vertices.reserve(n * (aneis + 1));
    m.indices.reserve(n * aneis * 6);
    forcas.reserve(n * (aneis + 1));
    for anel in 0..=aneis {
        let e = alcance * anel as f32 / aneis as f32;
        for &(base, normal) in &contorno {
            let p = base + normal * (rr + e);
            let forca = if anel == aneis { 0.0 } else { intensidade(p, centro_alvo, meia_alvo, canto_alvo, sigma) };
            m.vertices.push(Vertex { pos: p, uv: WHITE_UV, color: Color32::TRANSPARENT });
            forcas.push(forca);
        }
    }
    for anel in 0..aneis {
        for i in 0..n {
            let a = (anel * n + i) as u32;
            let b = (anel * n + (i + 1) % n) as u32;
            let c = ((anel + 1) * n + i) as u32;
            let d = ((anel + 1) * n + (i + 1) % n) as u32;
            m.add_triangle(a, b, c);
            m.add_triangle(b, d, c);
        }
    }
    MalhaSombra { malha: m, forca: forcas }
}

/// Pontos de um arco (centro, raio, de `a0` a `a1` em radianos, sentido horário na tela).
fn arco(saida: &mut Vec<Pos2>, centro: Pos2, r: f32, a0: f32, a1: f32) {
    let passos = ((r * (a1 - a0).abs()) / 2.0).ceil().clamp(2.0, 24.0) as usize;
    for i in 0..=passos {
        let a = a0 + (a1 - a0) * i as f32 / passos as f32;
        saida.push(centro + vec2(a.cos(), a.sin()) * r);
    }
}

/// Contorno de 1px por fora de um retângulo arredondado, com cores diferentes no topo, nas laterais e na base
/// (o que as sombras `-1px 0 0`, `1px 0 0`, `0 -1px 0` e `0 1px 0` do anel aqua desenham).
pub fn contorno_segmentado(painter: &Painter, rect: Rect, r: f32, topo: Color32, lados: Color32, base: Color32) {
    let fora = rect.expand(0.5);
    let r = raio(fora, r + 0.5);
    let (x0, y0, x1, y1) = (fora.left(), fora.top(), fora.right(), fora.bottom());
    use std::f32::consts::{FRAC_PI_2, PI};
    if topo.a() > 0 && x1 - r > x0 + r {
        painter.add(Shape::line_segment([pos2(x0 + r, y0), pos2(x1 - r, y0)], Stroke::new(1.0, topo)));
    }
    if base.a() > 0 && x1 - r > x0 + r {
        painter.add(Shape::line_segment([pos2(x0 + r, y1), pos2(x1 - r, y1)], Stroke::new(1.0, base)));
    }
    if lados.a() > 0 {
        let mut dir = Vec::with_capacity(40);
        arco(&mut dir, pos2(x1 - r, y0 + r), r, -FRAC_PI_2, 0.0);
        arco(&mut dir, pos2(x1 - r, y1 - r), r, 0.0, FRAC_PI_2);
        painter.add(PathShape::line(dir, PathStroke::new(1.0, lados)));
        let mut esq = Vec::with_capacity(40);
        arco(&mut esq, pos2(x0 + r, y1 - r), r, FRAC_PI_2, PI);
        arco(&mut esq, pos2(x0 + r, y0 + r), r, PI, PI + FRAC_PI_2);
        painter.add(PathShape::line(esq, PathStroke::new(1.0, lados)));
    }
}

/// O anel aqua completo: sombra curta embaixo e o contorno segmentado.
pub fn anel_aqua(painter: &Painter, rect: Rect, r: f32, anel: &AnelAqua) {
    sombra(painter, rect, r, anel.sombra);
    contorno_segmentado(painter, rect, r, anel.topo, anel.lados, anel.base);
}

/// Só a sombra do anel aqua (para pintar antes do fundo).
pub fn sombra_anel(painter: &Painter, rect: Rect, r: f32, anel: &AnelAqua) {
    sombra(painter, rect, r, anel.sombra);
}

/// Só os fios do anel aqua (para pintar depois do fundo).
pub fn fios_anel(painter: &Painter, rect: Rect, r: f32, anel: &AnelAqua) {
    contorno_segmentado(painter, rect, r, anel.topo, anel.lados, anel.base);
}

/// O trilho rebaixado (`--k-trackRing`: sombra interna de 1px em cima): um fio interno e o topo mais escuro.
pub fn anel_rebaixado(painter: &Painter, rect: Rect, r: f32, cor: Color32) {
    if cor.a() == 0 {
        return;
    }
    painter.add(RectShape::stroke(
        rect,
        canto(rect, r),
        Stroke::new(1.0, cor.gamma_multiply(0.45)),
        StrokeKind::Inside,
    ));
    let rr = raio(rect, r);
    let dentro = rect.shrink(0.5);
    if dentro.width() > 2.0 * rr {
        let mut topo = Vec::with_capacity(40);
        use std::f32::consts::{FRAC_PI_2, PI};
        let rr2 = (rr - 0.5).max(0.0);
        arco(&mut topo, pos2(dentro.left() + rr2, dentro.top() + rr2), rr2, PI + FRAC_PI_2 * 0.5, PI + FRAC_PI_2);
        topo.push(pos2(dentro.right() - rr2, dentro.top()));
        arco(&mut topo, pos2(dentro.right() - rr2, dentro.top() + rr2), rr2, -FRAC_PI_2, -FRAC_PI_2 * 0.5);
        painter.add(PathShape::line(topo, PathStroke::new(1.0, cor.gamma_multiply(0.55))));
    }
}

/// Brilho especular (`--k-spec`): fio de 1px por dentro, no topo de um card ou janela.
pub fn brilho_topo(painter: &Painter, rect: Rect, r: f32, cor: Color32) {
    if cor.a() == 0 {
        return;
    }
    let rr = raio(rect, r);
    let y = rect.top() + 1.5;
    painter.add(Shape::line_segment([pos2(rect.left() + rr, y), pos2(rect.right() - rr, y)], Stroke::new(1.0, cor)));
}

#[derive(Clone, Copy, Default)]
struct ModoTeclado {
    passada: u64,
    teclado: bool,
}

/// O foco deve aparecer? (o `:focus-visible` do navegador: o anel só aparece quando a pessoa está usando o
/// teclado; depois de um clique, o foco dado por código fica sem anel). Vale para botões, links, abas,
/// caixas de seleção; campos de texto mostram o foco sempre.
pub fn foco_visivel(ctx: &Context) -> bool {
    let id = Id::new("databox::pintura::modo_teclado");
    let passada = ctx.cumulative_pass_nr();
    let antes: ModoTeclado = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    if antes.passada == passada {
        return antes.teclado;
    }
    let mut teclado = antes.teclado;
    ctx.input(|i| {
        for e in &i.events {
            match e {
                egui::Event::PointerButton { pressed: true, .. } => teclado = false,
                egui::Event::Key { pressed: true, .. } => teclado = true,
                _ => {}
            }
        }
    });
    ctx.data_mut(|d| d.insert_temp(id, ModoTeclado { passada, teclado }));
    teclado
}

/// Esquece o teclado na passada atual: o foco dado por código nesta passada (o foco inicial de uma janela que acabou
/// de abrir) não ganha o anel, mesmo que a tecla que abriu a janela ainda esteja nos eventos. Teclas apertadas nos
/// quadros seguintes voltam a ligar o anel.
pub fn esquecer_teclado(ctx: &Context) {
    let id = Id::new("databox::pintura::modo_teclado");
    let passada = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(id, ModoTeclado { passada, teclado: false }));
}

/// Anel de foco do teclado (`:focus-visible`: 2px de ouro, 2px afastado).
pub fn anel_foco(painter: &Painter, p: &Paleta, rect: Rect, r: f32) {
    let fora = rect.expand(3.0);
    painter.add(RectShape::stroke(fora, canto(fora, r + 3.0), Stroke::new(2.0, p.gold), StrokeKind::Inside));
}

/// O halo de foco dos campos (`--k-focus`: 3px de ouro translúcido em volta).
pub fn halo_foco(painter: &Painter, p: &Paleta, rect: Rect, r: f32) {
    let fora = rect.expand(3.0);
    painter.add(RectShape::stroke(fora, canto(fora, r + 3.0), Stroke::new(3.0, p.focus), StrokeKind::Inside));
}

/// Clareia uma cor (o `filter: brightness()` do hover).
pub fn clarear(c: Color32, fator: f32) -> Color32 {
    let canal = |x: u8| (f32::from(x) * fator).round().clamp(0.0, f32::from(c.a())) as u8;
    Color32::from_rgba_premultiplied(canal(c.r()), canal(c.g()), canal(c.b()), c.a())
}

// ------------------------------------------------------------------ fundo da página

#[derive(Clone)]
struct CacheFundo {
    chave: [u32; 5],
    malhas: Vec<Arc<Mesh>>,
}

fn id_fundo() -> Id {
    Id::new("databox::pintura::fundo")
}

fn malha_degrade_vertical(rect: Rect, paradas: &[(f32, Color32)]) -> Mesh {
    let mut m = Mesh::default();
    for (i, &(t, cor)) in paradas.iter().enumerate() {
        let y = rect.top() + rect.height() * t;
        m.colored_vertex(pos2(rect.left(), y), cor);
        m.colored_vertex(pos2(rect.right(), y), cor);
        if i > 0 {
            let b = (2 * i) as u32;
            m.add_triangle(b - 2, b - 1, b);
            m.add_triangle(b - 1, b + 1, b);
        }
    }
    m
}

/// Elipse em degradê da cor ao transparente (um `radial-gradient(rx ry at x y, cor, transparent parada)`).
fn malha_bloom(janela: Rect, b: &crate::tema::Bloom) -> Mesh {
    const SEGMENTOS: usize = 64;
    const ANEIS: usize = 8;
    let centro = pos2(janela.left() + janela.width() * b.centro_x, janela.top() + janela.height() * b.centro_y);
    let (rx, ry) = (b.raio_x * b.parada, b.raio_y * b.parada);
    let mut m = Mesh::default();
    m.vertices.reserve(1 + SEGMENTOS * ANEIS);
    m.vertices.push(Vertex { pos: centro, uv: WHITE_UV, color: b.cor });
    for anel in 1..=ANEIS {
        let t = anel as f32 / ANEIS as f32;
        let cor = b.cor.gamma_multiply(1.0 - t);
        for s in 0..SEGMENTOS {
            let a = s as f32 / SEGMENTOS as f32 * std::f32::consts::TAU;
            m.vertices.push(Vertex {
                pos: centro + vec2(a.cos() * rx * t, a.sin() * ry * t),
                uv: WHITE_UV,
                color: cor,
            });
        }
    }
    let idx = |anel: usize, s: usize| -> u32 { (1 + (anel - 1) * SEGMENTOS + s % SEGMENTOS) as u32 };
    for s in 0..SEGMENTOS {
        m.add_triangle(0, idx(1, s), idx(1, s + 1));
    }
    for anel in 2..=ANEIS {
        for s in 0..SEGMENTOS {
            let (a, b2, c, d) = (idx(anel - 1, s), idx(anel - 1, s + 1), idx(anel, s), idx(anel, s + 1));
            m.add_triangle(a, c, b2);
            m.add_triangle(b2, c, d);
        }
    }
    m
}

/// Pinta o fundo da página (`--k-bg` e `--k-bloom`) no retângulo da janela. As malhas ficam guardadas: sem
/// mudar de tamanho ou de tema, nada é recalculado.
pub fn pintar_fundo(painter: &Painter, p: &Paleta, janela: Rect) {
    let chave = [
        janela.min.x.to_bits(),
        janela.min.y.to_bits(),
        janela.max.x.to_bits(),
        janela.max.y.to_bits(),
        u32::from(p.escuro),
    ];
    let ctx = painter.ctx();
    let guardado: Option<CacheFundo> = ctx.data(|d| d.get_temp(id_fundo()));
    let malhas = match guardado {
        Some(c) if c.chave == chave => c.malhas,
        _ => {
            let mut malhas = Vec::with_capacity(4);
            malhas.push(Arc::new(malha_degrade_vertical(
                janela,
                &[(0.0, p.fundo_topo), (0.55, p.fundo_meio), (1.0, p.fundo_base)],
            )));
            for b in p.blooms.iter().flatten() {
                malhas.push(Arc::new(malha_bloom(janela, b)));
            }
            ctx.data_mut(|d| d.insert_temp(id_fundo(), CacheFundo { chave, malhas: malhas.clone() }));
            malhas
        }
    };
    painter.rect_filled(janela, 0.0, p.bg);
    for m in malhas {
        painter.add(Shape::Mesh(m));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn degrade_nas_paradas() {
        let preto = Color32::from_rgb(0, 0, 0);
        let branco = Color32::from_rgb(255, 255, 255);
        let paradas = [(0.0, preto), (0.5, branco), (1.0, preto)];
        assert_eq!(cor_no_degrade(&paradas, 0.0), preto);
        assert_eq!(cor_no_degrade(&paradas, 0.5), branco);
        assert_eq!(cor_no_degrade(&paradas, 0.25), Color32::from_rgb(128, 128, 128));
        assert_eq!(cor_no_degrade(&paradas, 2.0), preto);
        assert_eq!(cor_no_degrade(&[], 0.3), Color32::TRANSPARENT);
    }

    #[test]
    fn raio_do_pill_e_meia_altura() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 30.0));
        assert_eq!(raio(r, f32::INFINITY), 15.0);
        assert_eq!(raio(r, 8.0), 8.0);
    }

    /// Distância com sinal de `p` ao retângulo arredondado (negativa dentro).
    fn distancia(p: Pos2, rect: Rect, rr: f32) -> f32 {
        let q = (p - rect.center()).abs() - (rect.size() / 2.0 - vec2(rr, rr));
        q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - rr
    }

    #[test]
    fn sombra_fica_so_fora_da_peca() {
        let peca = Rect::from_min_size(pos2(10.0, 20.0), vec2(120.0, 30.0));
        let casos = [
            (f32::INFINITY, CLARA.anel_aqua.sombra),
            (f32::INFINITY, ESCURA.anel_aqua.sombra),
            (14.0, ESCURA.sombra),
            (12.0, CLARA.sombra_lg),
            (f32::INFINITY, CLARA.toast_sombra),
            (0.0, CLARA.anel_aqua_forte.sombra),
        ];
        for (r, s) in casos {
            let Shape::Mesh(m) = forma_sombra(peca, r, s) else { panic!("a sombra é uma malha") };
            assert!(m.is_valid());
            let rr = raio(peca, r);
            for v in &m.vertices {
                let d = distancia(v.pos, peca, rr);
                assert!(d > -0.01, "vértice dentro da peça ({d}) com raio {r}: {:?}", v.pos);
            }
            // o anel de fora some
            let ultimo = m.vertices.last().map(|v| v.color);
            assert_eq!(ultimo, Some(Color32::TRANSPARENT));
        }
    }

    #[test]
    fn sombra_desce_com_o_deslocamento() {
        // `0 2px 6px`: logo abaixo da peça a sombra é forte; logo acima, mais fraca; longe, nada
        let peca = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 30.0));
        let s = SombraCss { y: 2.0, desfoque: 6.0, espalhar: 0.0, cor: Color32::BLACK };
        let (centro, meia) = (peca.translate(vec2(0.0, 2.0)).center(), vec2(100.0, 15.0));
        let embaixo = intensidade(pos2(100.0, 30.5), centro, meia, 15.0, 3.0);
        let em_cima = intensidade(pos2(100.0, -0.5), centro, meia, 15.0, 3.0);
        let longe = intensidade(pos2(100.0, 45.0), centro, meia, 15.0, 3.0);
        assert!(embaixo > 0.65 && embaixo < 0.85, "{embaixo}");
        assert!(em_cima < embaixo && em_cima > 0.1, "{em_cima}");
        assert!(longe < 0.01, "{longe}");
        // e a malha guardada é a mesma para a mesma peça em outro lugar
        let a = forma_sombra(peca, f32::INFINITY, s);
        let b = forma_sombra(peca.translate(vec2(50.0, 7.0)), f32::INFINITY, s);
        match (a, b) {
            (Shape::Mesh(a), Shape::Mesh(b)) => {
                assert_eq!(a.vertices.len(), b.vertices.len());
                assert_eq!(a.vertices[0].pos + vec2(50.0, 7.0), b.vertices[0].pos);
            }
            _ => panic!("malhas"),
        }
    }

    #[test]
    fn cor_animada_nao_cria_malha_nova() {
        // um halo de hover que acende em 12 quadros: a mesma malha, com a cor de cada quadro
        let peca = Rect::from_min_size(pos2(0.0, 0.0), vec2(90.0, 28.0));
        let antes = CACHE_SOMBRAS.with(|c| c.borrow().malhas.len());
        let mut cores = Vec::new();
        for i in 1..=12 {
            let cor = CLARA.estudio.brilho.gamma_multiply(i as f32 / 12.0);
            let Shape::Mesh(m) =
                forma_sombra(peca, f32::INFINITY, SombraCss { y: 0.0, desfoque: 14.0, espalhar: 0.0, cor })
            else {
                panic!("a sombra é uma malha")
            };
            cores.push(m.vertices.iter().map(|v| v.color.a()).max().unwrap_or(0));
        }
        assert_eq!(CACHE_SOMBRAS.with(|c| c.borrow().malhas.len()), antes + 1);
        assert!(cores.windows(2).all(|par| par[0] <= par[1]), "o halo acende: {cores:?}");
        assert!(cores[11] > cores[0]);
    }

    #[test]
    fn cache_cheio_guarda_as_sombras_em_uso() {
        let parada = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 200.0));
        let s = SombraCss { y: 18.0, desfoque: 50.0, espalhar: 0.0, cor: Color32::BLACK };
        let _ = forma_sombra(parada, 14.0, s);
        // uma largura animada: uma malha por quadro, e a sombra parada pedida a cada quadro
        for i in 0..(2 * LIMITE_CACHE_SOMBRAS) {
            let barra = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0 + i as f32 * 0.5, 6.0));
            let _ = forma_sombra(barra, f32::INFINITY, SombraCss { y: 0.0, desfoque: 10.0, espalhar: 0.0, cor: s.cor });
            let _ = forma_sombra(parada, 14.0, s);
        }
        let chave = ChaveSombra {
            tamanho: [parada.width().to_bits(), parada.height().to_bits()],
            raio: 14.0_f32.to_bits(),
            sombra: [s.y.to_bits(), s.desfoque.to_bits(), s.espalhar.to_bits()],
        };
        CACHE_SOMBRAS.with(|c| {
            let c = c.borrow();
            assert!(c.malhas.len() <= LIMITE_CACHE_SOMBRAS);
            assert!(c.malhas.contains_key(&chave), "a sombra parada continua guardada");
        });
    }

    #[test]
    fn malha_do_bloom_e_valida() {
        let janela = Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0));
        for b in CLARA.blooms.iter().chain(ESCURA.blooms.iter()).flatten() {
            assert!(malha_bloom(janela, b).is_valid());
        }
        assert!(
            malha_degrade_vertical(janela, &[(0.0, Color32::RED), (0.5, Color32::RED), (1.0, Color32::RED)]).is_valid()
        );
    }
}
