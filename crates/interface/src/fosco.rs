//! O vidro fosco das camadas que se sobrepõem (o `backdrop-filter: blur(24px) saturate(150%)` do `.flut` das
//! pranchetas): menus e submenus, popovers, a lista dos selects, as janelas e o aviso flutuante. As dicas (as do
//! próprio egui, `Order::Tooltip`, e o balão de ajuda) ficam de fora: uma foto de tela cheia a cada dica custa mais
//! que a dica vale, e elas ficam no vidro liso ([`crate::pecas::campos::forma_vidro_flutuante`]: o `--flut` a 96%).
//!
//! O egui não desfoca o que está atrás de uma camada; o fosco faz isso com a imagem da própria janela. Quando aparece
//! uma camada de vidro sem um fundo que sirva, ela fica fora da pintura enquanto o eframe tira a foto da janela
//! (`ViewportCommand::Screenshot`: no glow e no wgpu, a foto é do quadro seguinte ao do pedido e chega no outro; o
//! `egui_kittest` fotografa o próprio quadro do pedido). A foto vira o fundo desfocado ([`fundo_desfocado`]): reduzida
//! a um texel por [`PONTOS_POR_TEXEL`]² pontos (a média de 2×2 pixels de cada bloco), desfocada por três caixas
//! seguidas (a gaussiana de [`DESFOQUE`] pontos do CSS), saturada a [`SATURACAO`] e enviada uma vez como textura
//! linear (uns 5 ms no build otimizado, em 1440×900 ou 2880×1800, fora a leitura da tela pelo eframe). O vidro
//! ([`crate::pecas::campos::forma_vidro_fosco`]) pinta a textura embaixo do `--flut`, nas coordenadas da tela
//! ([`uv_em`]); os 78% (escuro) e 80% (claro) do `--flut` só valem com essa textura embaixo.
//!
//! O fundo serve enquanto alguma camada estiver aberta: a camada que aparece depois usa o mesmo se nenhuma camada
//! aberta estiver embaixo dela (o submenu ao lado do menu) e ele tiver menos de 2 s; se estiver (a lista do select
//! sobre a janela), pede uma foto nova com as de baixo à vista. Fechou tudo, o último fundo, tirado sem camada
//! nenhuma à vista, fica mais [`IDADE_MAX_REUSO`] (o menu que fecha e o seguinte que abre logo depois não pedem outra
//! foto); os demais somem. A troca de tema refaz a foto na hora; a mudança de tamanho da janela (ou de escala),
//! quando ela para de mudar. Com a camada aberta, o que muda atrás dela não entra no fundo (ele fica como estava na
//! foto).
//!
//! Só vale com o fosco ligado ([`ligar`], no criador do app); sem ele (os testes de tela), o vidro é o `--flut` liso
//! a 96% (a mesma cor). Se a foto não chegar em [`ESPERA_MAX`] (janela minimizada), ou vier duas vezes de um tamanho
//! que não é o da janela, a camada aparece com o vidro liso. Repinta só enquanto espera a foto (dois quadros, em
//! geral, e nunca além da [`ESPERA_MAX`]).

use std::sync::Arc;
use std::time::Instant;

use egui::epaint::{ClippedShape, RectShape};
use egui::layers::{PaintList, ShapeIdx};
use egui::{
    Color32, ColorImage, Context, Event, LayerId, Order, Painter, Pos2, Rect, Shape, TextureHandle, TextureId,
    TextureOptions, Theme, Ui, UserData, Vec2, ViewportCommand, ViewportId, pos2, vec2,
};

/// O desvio da gaussiana, em pontos (o `blur(24px)` do `--desfoque`).
pub const DESFOQUE: f32 = 24.0;
/// A saturação depois do desfoque (o `saturate(150%)`).
pub const SATURACAO: f32 = 1.5;
/// Quantos pontos (de lado) cada texel do fundo cobre: a foto é reduzida antes de desfocar.
pub const PONTOS_POR_TEXEL: f32 = 4.0;
/// Quanto esperar pela foto (em segundos) antes de mostrar a camada com o vidro liso.
pub const ESPERA_MAX: f64 = 0.5;
/// Quantas fotos pedir, no máximo, para as mesmas camadas quando elas vêm de um tamanho que não é o da janela: depois
/// disso, o vidro liso.
const TENTATIVAS_MAX: u8 = 2;
/// Depois de a janela mudar de tamanho (ou de escala), quanto esperar parada antes de fotografar de novo.
const ESPERA_TAMANHO: f64 = 0.25;
/// Até que idade um fundo serve para uma camada que acabou de aparecer.
const IDADE_MAX_REUSO: f64 = 2.0;

/// Liga o fosco neste contexto (uma vez, no criador do app; chamar de novo não faz nada). `aviso` recebe as
/// linhas para o registro: a da primeira foto (o tempo até chegar e o do desfoque) e a de uma foto que não chegou.
pub fn ligar(ctx: &Context, aviso: Option<fn(&str)>) {
    ctx.add_plugin(Fosco { aviso, ..Fosco::default() });
}

/// O fosco está ligado neste contexto?
pub fn ligado(ctx: &Context) -> bool {
    ctx.plugin_opt::<Fosco>().is_some()
}

/// Como está o fosco agora (para os testes e o diagnóstico).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Resumo {
    /// Fundos desfocados guardados.
    pub fundos: usize,
    /// Camadas abertas com fundo.
    pub com_fundo: usize,
    /// Camadas abertas com o vidro liso (a foto não chegou, ou a janela ainda está mudando de tamanho).
    pub lisas: usize,
    /// Camadas fora da pintura, esperando a foto.
    pub esperando: usize,
    /// Fotos desfocadas desde que o fosco ligou.
    pub fotos: u32,
    /// Quanto o último desfoque levou (redução, desfoque, saturação e envio da textura), em ms.
    pub ultimo_ms: f32,
}

/// O [`Resumo`] do fosco, se ele estiver ligado.
pub fn resumo(ctx: &Context) -> Option<Resumo> {
    ctx.with_plugin(|f: &mut Fosco| Resumo {
        fundos: f.fundos.len(),
        com_fundo: f.camadas.iter().filter(|c| matches!(c.destino, Destino::Fundo(_))).count(),
        lisas: f.camadas.iter().filter(|c| c.destino == Destino::Liso).count(),
        esperando: f.pedido.as_ref().map_or(0, |p| p.escondidas.len()),
        fotos: f.fotos,
        ultimo_ms: f.ultimo_ms,
    })
}

/// O fundo desfocado para um vidro em `rect` (pontos da tela) na camada do `painter`: a textura e as coordenadas
/// dela que cobrem o `rect`. Marca a camada como de vidro nesta passada. `None` sem o fosco ligado, enquanto a foto
/// não chegou ou com o vidro liso.
pub(crate) fn fundo_para(painter: &Painter, rect: Rect) -> Option<(TextureId, Rect)> {
    let camada = painter.layer_id();
    painter
        .ctx()
        .with_plugin(|f: &mut Fosco| {
            f.marcar(camada, rect);
            f.fundo_da(camada).map(|fundo| (fundo.textura.id(), uv_em(fundo.cobre, rect)))
        })
        .flatten()
}

/// As coordenadas de textura de `rect` num fundo que cobre `cobre` (os dois em pontos da tela).
pub fn uv_em(cobre: Rect, rect: Rect) -> Rect {
    let w = cobre.width().max(f32::EPSILON);
    let h = cobre.height().max(f32::EPSILON);
    let uv = |p: Pos2| pos2((p.x - cobre.min.x) / w, (p.y - cobre.min.y) / h);
    Rect::from_min_max(uv(rect.min), uv(rect.max))
}

// ------------------------------------------------------------------ o desfoque

/// O fundo desfocado de uma foto da janela com `ppp` pixels por ponto: a imagem pequena (um texel por
/// [`PONTOS_POR_TEXEL`]² pontos, desfocada com [`DESFOQUE`] pontos e saturada a [`SATURACAO`]) e o retângulo da
/// tela, em pontos, que ela cobre (do canto de cima à esquerda; um pouco maior que a tela quando os blocos da borda
/// ficam incompletos).
pub fn fundo_desfocado(foto: &ColorImage, ppp: f32) -> (ColorImage, Rect) {
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let fator = (PONTOS_POR_TEXEL * ppp).round().max(1.0) as usize;
    let pequena = reduzir_amostrando(foto, fator, fator / 2);
    let mut desfocada = desfocar(&pequena, DESFOQUE * ppp / fator as f32);
    saturar(&mut desfocada, SATURACAO);
    let [w, h] = desfocada.size;
    let cobre = Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32) * fator as f32 / ppp);
    (desfocada, cobre)
}

/// Reduz a imagem pela média dos blocos de `fator`×`fator` pixels (os blocos da borda direita e da de baixo, que
/// ficam incompletos, pela média do que têm). O alfa sai 255: a foto da janela é opaca.
pub fn reduzir(img: &ColorImage, fator: usize) -> ColorImage {
    reduzir_amostrando(img, fator, 1)
}

/// O [`reduzir`] lendo um pixel a cada `passo` nas linhas e nas colunas de cada bloco (a média de uma grade do
/// bloco). O fundo lê 2×2 pixels por bloco (`passo` = metade do bloco): a leitura fica do tamanho do fundo, não da
/// tela, e depois do desfoque de 6 texels não se vê a diferença.
pub fn reduzir_amostrando(img: &ColorImage, fator: usize, passo: usize) -> ColorImage {
    let fator = fator.max(1);
    let passo = passo.clamp(1, fator);
    let [w, h] = img.size;
    let (rw, rh) = (w.div_ceil(fator), h.div_ceil(fator));
    let mut somas = vec![[0_u32; 3]; rw];
    let mut saida = Vec::with_capacity(rw * rh);
    for by in 0..rh {
        somas.fill([0; 3]);
        let (y0, y1) = (by * fator, ((by + 1) * fator).min(h));
        for y in (y0..y1).step_by(passo) {
            for (soma, bloco) in somas.iter_mut().zip(img.pixels[y * w..(y + 1) * w].chunks(fator)) {
                for c in bloco.iter().step_by(passo) {
                    soma[0] += u32::from(c.r());
                    soma[1] += u32::from(c.g());
                    soma[2] += u32::from(c.b());
                }
            }
        }
        let alto = (y1 - y0).div_ceil(passo) as u32;
        for (bx, soma) in somas.iter().enumerate() {
            let largo = (((bx + 1) * fator).min(w) - bx * fator).div_ceil(passo) as u32;
            let n = (largo * alto).max(1);
            let media = |s: u32| ((s + n / 2) / n) as u8;
            saida.push(Color32::from_rgb(media(soma[0]), media(soma[1]), media(soma[2])));
        }
    }
    ColorImage::new([rw, rh], saida)
}

/// As larguras (ímpares) de `n` caixas seguidas que, juntas, fazem uma gaussiana de desvio `sigma` (o método de
/// Kutskir: as caixas de largura `l` e `l + 2` misturadas para a variância bater).
pub fn larguras_caixa(sigma: f32, n: usize) -> Vec<usize> {
    let n = n.max(1);
    let s2 = (sigma.max(0.0) as f64).powi(2);
    let ideal = (12.0 * s2 / n as f64 + 1.0).sqrt();
    let mut l = ideal.floor() as i64;
    if l % 2 == 0 {
        l -= 1;
    }
    let l = l.max(1);
    let lf = l as f64;
    let nf = n as f64;
    let m = ((12.0 * s2 - nf * lf * lf - 4.0 * nf * lf - 3.0 * nf) / (-4.0 * lf - 4.0)).round().clamp(0.0, nf) as usize;
    (0..n).map(|i| if i < m { l as usize } else { l as usize + 2 }).collect()
}

/// Desfoca a imagem com uma gaussiana de desvio `sigma` (em pixels dela), por três caixas seguidas nas linhas e nas
/// colunas; na borda, o pixel da borda se repete. O alfa sai 255.
pub fn desfocar(img: &ColorImage, sigma: f32) -> ColorImage {
    let [w, h] = img.size;
    // um plano por canal: as caixas andam em fatias simples (rápido também no build de desenvolvimento)
    let mut planos: [Vec<f32>; 3] = [0, 1, 2].map(|c| img.pixels.iter().map(|p| f32::from(p.to_array()[c])).collect());
    if sigma > 0.0 && w > 0 && h > 0 {
        let larguras = larguras_caixa(sigma, 3);
        let mut acolchoada = Vec::with_capacity(w + larguras.iter().max().copied().unwrap_or(1) + 1);
        let mut saida = vec![0.0_f32; w * h];
        let mut soma = vec![0.0_f32; w];
        for plano in &mut planos {
            for &largura in &larguras {
                let raio = largura / 2;
                if raio == 0 {
                    continue;
                }
                for fila in plano.chunks_exact_mut(w) {
                    caixa(fila, raio, &mut acolchoada);
                }
                caixa_vertical(plano, &mut saida, &mut soma, w, raio);
            }
        }
    }
    let [r, g, b] = &planos;
    let pixels = r.iter().zip(g).zip(b).map(|((r, g), b)| Color32::from_rgb(canal(*r), canal(*g), canal(*b))).collect();
    ColorImage::new([w, h], pixels)
}

/// A média móvel de `2·raio + 1` valores da `linha`, no lugar, repetindo os das pontas (`acolchoada` é só o
/// espaço de trabalho: a linha com as pontas repetidas).
fn caixa(linha: &mut [f32], raio: usize, acolchoada: &mut Vec<f32>) {
    let n = linha.len();
    if n == 0 {
        return;
    }
    acolchoada.clear();
    acolchoada.extend(std::iter::repeat_n(linha[0], raio));
    acolchoada.extend_from_slice(linha);
    acolchoada.extend(std::iter::repeat_n(linha[n - 1], raio + 1));
    let largura = 2 * raio + 1;
    let peso = 1.0 / largura as f32;
    let mut soma: f32 = acolchoada[..largura].iter().sum();
    for ((alvo, entra), sai) in linha.iter_mut().zip(&acolchoada[largura..]).zip(acolchoada.iter()) {
        *alvo = soma * peso;
        soma += entra - sai;
    }
}

/// A [`caixa`] nas colunas do `plano` (linhas de `w`), linha a linha: a soma de cada coluna anda junto com as outras
/// (`soma`, uma por coluna), e o resultado vai para `saida` antes de voltar ao plano.
fn caixa_vertical(plano: &mut [f32], saida: &mut [f32], soma: &mut [f32], w: usize, raio: usize) {
    let h = plano.len() / w;
    if h == 0 {
        return;
    }
    let linha = |y: isize| {
        let y = y.clamp(0, h as isize - 1) as usize;
        &plano[y * w..(y + 1) * w]
    };
    let r = raio as isize;
    let peso = 1.0 / (2 * raio + 1) as f32;
    soma.fill(0.0);
    for k in -r..=r {
        for (s, v) in soma.iter_mut().zip(linha(k)) {
            *s += v;
        }
    }
    for (y, fila) in saida.chunks_exact_mut(w).enumerate() {
        let (entra, sai) = (linha(y as isize + r + 1), linha(y as isize - r));
        for (((alvo, s), e), d) in fila.iter_mut().zip(soma.iter_mut()).zip(entra).zip(sai) {
            *alvo = *s * peso;
            *s += e - d;
        }
    }
    plano.copy_from_slice(saida);
}

/// O canal arredondado (sem o `f32::round`, que no Windows GNU é uma chamada à libm e pesava mais que o desfoque).
fn canal(v: f32) -> u8 {
    (v.clamp(0.0, 255.0) + 0.5) as u8
}

/// Satura as cores como o `saturate()` do CSS (a matriz do `feColorMatrix type="saturate"`): `s` = 1 deixa como
/// está, 0 é cinza, acima de 1 satura.
pub fn saturar(img: &mut ColorImage, s: f32) {
    let m = [
        [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
    ];
    for c in &mut img.pixels {
        let v = [f32::from(c.r()), f32::from(c.g()), f32::from(c.b())];
        let k = |l: [f32; 3]| canal(l[0] * v[0] + l[1] * v[1] + l[2] * v[2]);
        *c = Color32::from_rgba_premultiplied(k(m[0]), k(m[1]), k(m[2]), c.a());
    }
}

// ------------------------------------------------------------------ as camadas

/// O que vai no `UserData` da foto pedida: a geração do pedido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Marca(u64);

struct Fundo {
    id: u64,
    textura: TextureHandle,
    cobre: Rect,
    /// As camadas que estavam à vista na foto.
    base: Vec<LayerId>,
    em: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Destino {
    Fundo(u64),
    Liso,
}

struct Camada {
    id: LayerId,
    rect: Rect,
    destino: Destino,
}

struct Pedido {
    geracao: u64,
    base: Vec<LayerId>,
    /// As camadas fora da pintura até a foto chegar.
    escondidas: Vec<LayerId>,
    /// Desde quando elas esperam (o prazo da [`ESPERA_MAX`]; um pedido refeito mantém este instante).
    desde: f64,
    /// Quantos pedidos de foto já foram feitos para elas, contando o atual (no máximo [`TENTATIVAS_MAX`]).
    tentativas: u8,
    pedido_em: Instant,
}

/// Uma camada de vidro desta passada: onde está o vidro (a união dos vidros dela) e, nas dicas do egui, onde estão
/// os quadros `window_fill` (o índice na lista, dentro de um `Shape::Vec` o índice nele, e a opacidade).
struct Vista {
    id: LayerId,
    rect: Rect,
    quadros: Vec<(usize, Option<usize>, f32)>,
}

#[derive(Default)]
struct Fosco {
    aviso: Option<fn(&str)>,
    geracao: u64,
    fundos: Vec<Fundo>,
    camadas: Vec<Camada>,
    pedido: Option<Pedido>,
    /// Os vidros pintados com [`fundo_para`] nesta passada.
    marcadas: Vec<(LayerId, Rect)>,
    tema: Option<Theme>,
    tela: Option<(Vec2, f32)>,
    /// Não fotografar antes disto (a janela mudou de tamanho).
    quieto_em: f64,
    fotos: u32,
    ultimo_ms: f32,
    avisou_falta: bool,
}

impl Fosco {
    fn marcar(&mut self, camada: LayerId, rect: Rect) {
        match self.marcadas.iter_mut().find(|(c, _)| *c == camada) {
            Some((_, r)) => *r = r.union(rect),
            None => self.marcadas.push((camada, rect)),
        }
    }

    fn fundo_da(&self, camada: LayerId) -> Option<&Fundo> {
        let Destino::Fundo(id) = self.camadas.iter().find(|c| c.id == camada)?.destino else { return None };
        self.fundos.iter().find(|f| f.id == id)
    }

    /// Esquece os fundos e as camadas: as abertas viram novas e ganham uma foto nova.
    fn invalidar(&mut self) {
        self.fundos.clear();
        self.camadas.clear();
        self.pedido = None;
    }

    /// Pede a foto da janela. `desde` é quando as camadas escondidas começaram a esperar (o prazo da [`ESPERA_MAX`] é
    /// dele, não do pedido: um pedido refeito não renova a espera) e `tentativas`, quantos pedidos já foram feitos
    /// para elas contando este.
    fn pedir_foto(&mut self, ctx: &Context, base: Vec<LayerId>, escondidas: Vec<LayerId>, desde: f64, tentativas: u8) {
        self.geracao += 1;
        let geracao = self.geracao;
        self.pedido = Some(Pedido { geracao, base, escondidas, desde, tentativas, pedido_em: Instant::now() });
        ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(Marca(geracao))));
    }

    fn receber(&mut self, ctx: &Context, foto: &ColorImage, tela: Vec2, ppp: f32, agora: f64) {
        let Some(pedido) = self.pedido.take() else { return };
        let esperado = [(tela.x * ppp).round() as i64, (tela.y * ppp).round() as i64];
        if (foto.size[0] as i64 - esperado[0]).abs() > 2 || (foto.size[1] as i64 - esperado[1]).abs() > 2 {
            // a janela mudou de tamanho entre o pedido e a foto: pede outra, uma vez (com o prazo original); a
            // segunda que vem do tamanho errado, ou a espera que passou do prazo, deixa as camadas no vidro liso
            if pedido.tentativas < TENTATIVAS_MAX && agora - pedido.desde <= ESPERA_MAX {
                self.pedir_foto(ctx, pedido.base, pedido.escondidas, pedido.desde, pedido.tentativas + 1);
            } else {
                self.desistir(pedido.escondidas, "a foto da janela veio do tamanho errado");
            }
            return;
        }
        let chegou_ms = pedido.pedido_em.elapsed().as_secs_f32() * 1000.0;
        let inicio = Instant::now();
        let (imagem, cobre) = fundo_desfocado(foto, ppp);
        let texels = imagem.size;
        let textura = ctx.load_texture(format!("databox::fosco::{}", pedido.geracao), imagem, TextureOptions::LINEAR);
        self.ultimo_ms = inicio.elapsed().as_secs_f32() * 1000.0;
        self.fotos += 1;
        if self.fotos == 1
            && let Some(aviso) = self.aviso
        {
            aviso(&format!(
                "Vidro fosco: a foto da janela ({}×{} px) chegou {chegou_ms:.0} ms depois do pedido e foi desfocada em \
                 {:.1} ms ({}×{} texels, desfoque de {DESFOQUE} pt).",
                foto.size[0], foto.size[1], self.ultimo_ms, texels[0], texels[1],
            ));
        }
        self.fundos.push(Fundo { id: pedido.geracao, textura, cobre, base: pedido.base, em: agora });
        for id in pedido.escondidas {
            self.camadas.push(Camada { id, rect: Rect::NOTHING, destino: Destino::Fundo(pedido.geracao) });
        }
    }

    /// As camadas escondidas esperando a foto ficam com o vidro liso (a foto não vem, ou veio errada): o fosco avisa
    /// uma vez no registro.
    fn desistir(&mut self, escondidas: Vec<LayerId>, motivo: &str) {
        for id in escondidas {
            self.camadas.push(Camada { id, rect: Rect::NOTHING, destino: Destino::Liso });
        }
        if !self.avisou_falta
            && let Some(aviso) = self.aviso
        {
            aviso(&format!("Vidro fosco: {motivo}; a camada aberta fica com o vidro liso."));
        }
        self.avisou_falta = true;
    }

    /// As camadas de vidro desta passada: as marcadas por [`fundo_para`] e as de quadros do próprio egui (um quadro
    /// com o `window_fill` e o `window_stroke`, com ou sem a opacidade da entrada) numa camada do meio ou de cima;
    /// as dicas (`Order::Tooltip`) ficam de fora: são vidro liso, sem foto.
    fn vistas(&mut self, ctx: &Context) -> Vec<Vista> {
        let mut vistas: Vec<Vista> =
            self.marcadas.drain(..).map(|(id, rect)| Vista { id, rect, quadros: Vec::new() }).collect();
        let estilo = ctx.global_style();
        let (fundo, borda) = (estilo.visuals.window_fill, estilo.visuals.window_stroke.color);
        let mut candidatas: Vec<LayerId> = ctx.memory(|m| {
            m.areas()
                .visible_layer_ids()
                .into_iter()
                .filter(|l| matches!(l.order, Order::Middle | Order::Foreground))
                .collect()
        });
        candidatas.retain(|l| !vistas.iter().any(|v| v.id == *l));
        candidatas.sort_by_key(|l| (l.order, l.id.value()));
        for id in candidatas {
            let quadros = ctx.graphics(|g| g.get(id).map(|lista| quadros_do_egui(lista, fundo, borda)));
            let Some(quadros) = quadros.filter(|q| !q.is_empty()) else { continue };
            let rect = quadros.iter().fold(Rect::NOTHING, |r, q| r.union(q.2));
            vistas.push(Vista { id, rect, quadros: quadros.into_iter().map(|(i, j, _, k)| (i, j, k)).collect() });
        }
        vistas
    }

    fn fim_da_passada(&mut self, ctx: &Context) {
        let agora = ctx.input(|i| i.time);
        let vistas = self.vistas(ctx);
        let abertas: Vec<LayerId> = vistas.iter().map(|v| v.id).collect();

        // as que fecharam
        self.camadas.retain(|c| abertas.contains(&c.id));
        if let Some(p) = &mut self.pedido {
            p.escondidas.retain(|l| abertas.contains(l));
            if p.escondidas.is_empty() {
                self.pedido = None;
            }
        }
        // sem camada aberta, o último fundo fica por [`IDADE_MAX_REUSO`] (o menu que fecha e outro que abre logo em
        // seguida não pedem outra foto de tela cheia), se foi tirado sem camada nenhuma à vista (senão a foto
        // mostraria a que acabou de fechar); com camadas, só os que não dependem de uma que fechou
        if abertas.is_empty() {
            self.fundos.retain(|f| f.base.is_empty() && agora - f.em <= IDADE_MAX_REUSO);
        } else {
            self.fundos.retain(|f| f.base.iter().all(|b| abertas.contains(b)));
        }
        for c in &mut self.camadas {
            if let Some(v) = vistas.iter().find(|v| v.id == c.id) {
                c.rect = v.rect;
            }
        }

        // as que apareceram
        for v in &vistas {
            let conhecida = self.camadas.iter().any(|c| c.id == v.id)
                || self.pedido.as_ref().is_some_and(|p| p.escondidas.contains(&v.id));
            if conhecida {
                continue;
            }
            if agora < self.quieto_em {
                // a janela está mudando de tamanho: o vidro liso, e a foto quando ela parar
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(self.quieto_em - agora + 0.01));
                continue;
            }
            let embaixo: Vec<LayerId> =
                self.camadas.iter().filter(|c| cobre_de_fato(c.rect, v.rect)).map(|c| c.id).collect();
            let serve = self
                .fundos
                .iter()
                .find(|f| agora - f.em <= IDADE_MAX_REUSO && embaixo.iter().all(|e| f.base.contains(e)))
                .map(|f| f.id);
            if let Some(id) = serve {
                self.camadas.push(Camada { id: v.id, rect: v.rect, destino: Destino::Fundo(id) });
            } else if let Some(p) = &mut self.pedido {
                p.escondidas.push(v.id);
            } else {
                let base = self.camadas.iter().map(|c| c.id).collect();
                self.pedir_foto(ctx, base, vec![v.id], agora, 1);
            }
        }

        // a foto que não chega (janela minimizada): o vidro liso
        if self.pedido.as_ref().is_some_and(|p| agora - p.desde > ESPERA_MAX)
            && let Some(p) = self.pedido.take()
        {
            for id in p.escondidas {
                let rect = vistas.iter().find(|v| v.id == id).map_or(Rect::NOTHING, |v| v.rect);
                self.camadas.push(Camada { id, rect, destino: Destino::Liso });
            }
            if !self.avisou_falta
                && let Some(aviso) = self.aviso
            {
                aviso(&format!(
                    "Vidro fosco: a foto da janela não chegou em {:.0} ms; a camada aberta fica com o vidro liso.",
                    ESPERA_MAX * 1000.0
                ));
            }
            self.avisou_falta = true;
        }

        // fora da pintura enquanto a foto não chega (para não sair nela)
        if let Some(p) = &self.pedido {
            ctx.graphics_mut(|g| {
                for id in &p.escondidas {
                    if let Some(lista) = g.get_mut(*id) {
                        esconder(lista);
                    }
                }
            });
            ctx.request_repaint();
        }

        // as dicas do egui com fundo: a textura embaixo do quadro delas
        for v in vistas.iter().filter(|v| !v.quadros.is_empty()) {
            let Some(fundo) = self.fundo_da(v.id) else { continue };
            let (textura, cobre) = (fundo.textura.id(), fundo.cobre);
            ctx.graphics_mut(|g| {
                let Some(lista) = g.get_mut(v.id) else { return };
                for &(i, j, k) in &v.quadros {
                    lista.mutate_shape(ShapeIdx(i), |cs| forrar(cs, j, k, textura, cobre));
                }
            });
        }
    }
}

impl egui::Plugin for Fosco {
    fn debug_name(&self) -> &'static str {
        "databox::fosco"
    }

    fn on_begin_pass(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.marcadas.clear();
        if ctx.viewport_id() != ViewportId::ROOT {
            return;
        }
        let agora = ctx.input(|i| i.time);
        let tema = ctx.theme();
        let tela = (ctx.viewport_rect().size(), ctx.pixels_per_point());
        if self.tema.is_some_and(|t| t != tema) {
            self.invalidar();
        }
        if self.tela.is_some_and(|t| t != tela) {
            self.invalidar();
            self.quieto_em = agora + ESPERA_TAMANHO;
        }
        self.tema = Some(tema);
        self.tela = Some(tela);

        let fotos: Vec<(u64, Arc<ColorImage>)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    Event::Screenshot { viewport_id, user_data, image } if *viewport_id == ViewportId::ROOT => {
                        let marca = user_data.data.as_ref()?.downcast_ref::<Marca>()?;
                        Some((marca.0, Arc::clone(image)))
                    }
                    _ => None,
                })
                .collect()
        });
        for (geracao, foto) in fotos {
            if self.pedido.as_ref().is_some_and(|p| p.geracao == geracao) {
                self.receber(&ctx, &foto, tela.0, tela.1, agora);
            }
        }
    }

    fn on_end_pass(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if ctx.viewport_id() != ViewportId::ROOT || ctx.will_discard() {
            self.marcadas.clear();
            return;
        }
        self.fim_da_passada(&ctx);
    }
}

/// A camada em `baixo` fica mesmo embaixo da que está em `cima`? Uma beirada de até [`BEIRADA`] pontos não conta: o
/// submenu passa 4px por cima da borda do menu (o `padding:5px` dos dois e o `gap` de 2px), e ali o fundo de baixo
/// sem o menu não se distingue.
fn cobre_de_fato(baixo: Rect, cima: Rect) -> bool {
    let comum = baixo.intersect(cima);
    comum.width() > BEIRADA && comum.height() > BEIRADA
}

/// Ver [`cobre_de_fato`].
const BEIRADA: f32 = 8.0;

/// Os quadros das dicas do egui numa lista de formas: um retângulo sem textura com o `fundo` e a `borda` (os dois
/// na mesma proporção, a da opacidade da entrada), no alto da lista ou dentro de um `Shape::Vec` dela. Cada um com
/// o índice, o índice dentro do `Shape::Vec`, o retângulo e a opacidade.
fn quadros_do_egui(lista: &PaintList, fundo: Color32, borda: Color32) -> Vec<(usize, Option<usize>, Rect, f32)> {
    let mut achados = Vec::new();
    for (i, cs) in lista.all_entries().enumerate() {
        match &cs.shape {
            Shape::Rect(r) => {
                if let Some(k) = quadro_de_vidro(r, fundo, borda) {
                    achados.push((i, None, r.rect, k));
                }
            }
            Shape::Vec(formas) => {
                for (j, f) in formas.iter().enumerate() {
                    if let Shape::Rect(r) = f
                        && let Some(k) = quadro_de_vidro(r, fundo, borda)
                    {
                        achados.push((i, Some(j), r.rect, k));
                    }
                }
            }
            _ => {}
        }
    }
    achados
}

/// O retângulo é o quadro de uma dica do egui (o `fundo` e a `borda` vezes a mesma opacidade)? Devolve a opacidade.
fn quadro_de_vidro(r: &RectShape, fundo: Color32, borda: Color32) -> Option<f32> {
    if r.brush.is_some() || fundo.a() == 0 || r.fill.a() == 0 || r.stroke.width <= 0.0 {
        return None;
    }
    let k = f32::from(r.fill.a()) / f32::from(fundo.a());
    (k <= 1.0 + 1e-3 && proporcional(r.fill, fundo, k) && proporcional(r.stroke.color, borda, k)).then_some(k.min(1.0))
}

/// `cor` é `base` vezes `k` (com a tolerância do arredondamento)?
fn proporcional(cor: Color32, base: Color32, k: f32) -> bool {
    cor.to_array().iter().zip(base.to_array()).all(|(&c, b)| (f32::from(c) - f32::from(b) * k).abs() <= 2.5)
}

/// Põe a textura do fundo embaixo do quadro (a forma `j` dentro do `Shape::Vec`, ou a própria forma).
fn forrar(cs: &mut ClippedShape, j: Option<usize>, opacidade: f32, textura: TextureId, cobre: Rect) {
    let alvo = match (j, &mut cs.shape) {
        (None, s) => s,
        (Some(j), Shape::Vec(formas)) => match formas.get_mut(j) {
            Some(s) => s,
            None => return,
        },
        _ => return,
    };
    let Shape::Rect(quadro) = alvo else { return };
    let forro = RectShape::filled(quadro.rect, quadro.corner_radius, Color32::WHITE.gamma_multiply(opacidade))
        .with_texture(textura, uv_em(cobre, quadro.rect));
    let original = Shape::Rect(quadro.clone());
    *alvo = Shape::Vec(vec![forro.into(), original]);
}

/// Tira da pintura todas as formas da lista (a camada fica no lugar, sem nada à vista).
fn esconder(lista: &mut PaintList) {
    let n = lista.all_entries().len();
    for i in 0..n {
        lista.reset_shape(ShapeIdx(i));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn imagem(w: usize, h: usize, f: impl Fn(usize, usize) -> Color32) -> ColorImage {
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                px.push(f(x, y));
            }
        }
        ColorImage::new([w, h], px)
    }

    fn cinza(v: u8) -> Color32 {
        Color32::from_rgb(v, v, v)
    }

    #[test]
    fn reduzir_faz_a_media_dos_blocos_e_das_bordas_incompletas() {
        // 5×3 em blocos de 2: 3×2 texels; a coluna da direita tem 1px de largura, a linha de baixo 1px de altura
        let img = imagem(5, 3, |x, y| cinza((x * 10 + y * 100) as u8));
        let r = reduzir(&img, 2);
        assert_eq!(r.size, [3, 2]);
        let v = |x: usize, y: usize| r.pixels[y * 3 + x].r();
        assert_eq!(v(0, 0), 55); // (0 + 10 + 100 + 110) / 4
        assert_eq!(v(1, 0), 75); // (20 + 30 + 120 + 130) / 4
        assert_eq!(v(2, 0), 90); // (40 + 140) / 2
        assert_eq!(v(0, 1), 205); // (200 + 210) / 2
        assert_eq!(v(2, 1), 240); // só o (4, 2)
        assert!(r.pixels.iter().all(|c| c.a() == 255));
        assert_eq!(reduzir(&img, 1).pixels, img.pixels.iter().map(|c| cinza(c.r())).collect::<Vec<_>>());

        // amostrando a cada 2px: no bloco de 4×4, só as colunas e linhas 0 e 2 (e 1 de 1 na borda de 1px)
        let img = imagem(5, 4, |x, y| cinza((x * 10 + y * 50) as u8));
        let r = reduzir_amostrando(&img, 4, 2);
        assert_eq!(r.size, [2, 1]);
        assert_eq!(r.pixels[0].r(), 60); // (0 + 20 + 100 + 120) / 4
        assert_eq!(r.pixels[1].r(), 90); // (40 + 140) / 2
        assert_eq!(reduzir_amostrando(&img, 4, 1).pixels, reduzir(&img, 4).pixels);
    }

    #[test]
    fn as_caixas_somam_a_variancia_da_gaussiana() {
        for sigma in [1.5_f32, 4.0, 6.0, 8.0, 12.0] {
            let caixas = larguras_caixa(sigma, 3);
            assert_eq!(caixas.len(), 3);
            assert!(caixas.iter().all(|w| w % 2 == 1), "{caixas:?}");
            let variancia: f32 = caixas.iter().map(|&w| ((w * w) as f32 - 1.0) / 12.0).sum();
            assert!((variancia.sqrt() - sigma).abs() < 0.6, "σ {sigma}: {caixas:?} dá {}", variancia.sqrt());
        }
        assert_eq!(larguras_caixa(6.0, 3), [11, 11, 13]);
    }

    #[test]
    fn desfocar_mantem_o_liso_e_espalha_o_ponto_por_igual() {
        let liso = imagem(20, 12, |_, _| Color32::from_rgb(30, 31, 35));
        assert_eq!(desfocar(&liso, 6.0).pixels, liso.pixels);

        let ponto = imagem(61, 61, |x, y| if x == 30 && y == 30 { cinza(255) } else { cinza(0) });
        let d = desfocar(&ponto, 3.0);
        let v = |x: usize, y: usize| i32::from(d.pixels[y * 61 + x].r());
        // simétrico nos dois eixos e decrescente a partir do centro
        assert_eq!(v(27, 30), v(33, 30));
        assert_eq!(v(30, 27), v(30, 33));
        assert_eq!(v(27, 30), v(30, 27));
        assert!(v(30, 30) >= v(31, 30) && v(31, 30) >= v(33, 30) && v(33, 30) >= v(36, 30));
        assert_eq!(v(0, 0), 0);
    }

    #[test]
    fn desfocar_apaga_o_texto_que_estava_atras() {
        // um "texto": traços finos de 1px a cada 3px, em linhas de 12px (como o corpo de 12px), branco sobre escuro,
        // na escala do fundo (1 texel = 4 pontos, desfoque de 6 texels)
        let texto = imagem(120, 60, |x, y| if x % 3 == 0 && y % 12 < 9 { cinza(230) } else { cinza(30) });
        let pequena = reduzir(&texto, 1);
        let d = desfocar(&pequena, DESFOQUE / PONTOS_POR_TEXEL);
        let miolo: Vec<i32> = (20..100)
            .flat_map(|x| (15..45).map(move |y| (x, y)))
            .map(|(x, y)| i32::from(d.pixels[y * 120 + x].r()))
            .collect();
        let (min, max) = (*miolo.iter().min().unwrap(), *miolo.iter().max().unwrap());
        assert!(max - min <= 2, "sobrou contraste do texto: {min}..{max}");
    }

    #[test]
    fn saturar_como_o_css() {
        let mut img = imagem(3, 1, |x, _| match x {
            0 => cinza(128),
            1 => Color32::from_rgb(200, 100, 50),
            _ => Color32::from_rgb(30, 31, 35),
        });
        let antes = img.clone();
        saturar(&mut img, 1.0);
        assert_eq!(img.pixels, antes.pixels, "100% não muda nada");
        saturar(&mut img, SATURACAO);
        assert_eq!(img.pixels[0], cinza(128), "o cinza continua cinza");
        let c = img.pixels[1];
        assert!(c.r() > 200 && c.b() < 50, "a cor fica mais viva: {c:?}");
        // a luminância (0,213 / 0,715 / 0,072) quase não muda
        let lum = |c: Color32| 0.213 * f32::from(c.r()) + 0.715 * f32::from(c.g()) + 0.072 * f32::from(c.b());
        assert!((lum(c) - lum(antes.pixels[1])).abs() < 2.0);
    }

    #[test]
    fn as_coordenadas_do_vidro_seguem_a_tela() {
        let cobre = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let uv = uv_em(cobre, Rect::from_min_max(pos2(144.0, 90.0), pos2(288.0, 180.0)));
        assert!((uv.min - pos2(0.1, 0.1)).length() < 1e-6 && (uv.max - pos2(0.2, 0.2)).length() < 1e-6, "{uv:?}");
        assert_eq!(uv_em(cobre, cobre), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));

        // 1442×901 px a 1 px por ponto: blocos de 4 → 361×226 texels, que cobrem 1444×904 pontos
        let foto = ColorImage::filled([1442, 901], cinza(40));
        let (img, cobre) = fundo_desfocado(&foto, 1.0);
        assert_eq!(img.size, [361, 226]);
        assert_eq!(cobre, Rect::from_min_size(Pos2::ZERO, vec2(1444.0, 904.0)));
        // a 2 px por ponto, blocos de 8 px (os mesmos 4 pontos): 2880×1800 → 360×225, cobrindo 1440×900 pontos
        let (img, cobre) = fundo_desfocado(&ColorImage::filled([2880, 1800], cinza(40)), 2.0);
        assert_eq!(img.size, [360, 225]);
        assert_eq!(cobre, Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0)));
        // o texel do canto de baixo à direita: o centro dele em uv é o centro do bloco na tela
        let centro = uv_em(cobre, Rect::from_center_size(pos2(1438.0, 898.0), Vec2::ZERO)).min;
        assert!((centro.x - 359.5 / 360.0).abs() < 1e-6 && (centro.y - 224.5 / 225.0).abs() < 1e-6);
    }

    /// C4: uma foto de tamanho que não é o da janela não gera pedidos sem fim: uma segunda tentativa, com o prazo
    /// original, e depois o vidro liso.
    #[test]
    fn foto_do_tamanho_errado_tenta_de_novo_uma_vez_e_cai_no_vidro_liso() {
        let ctx = Context::default();
        let camada = LayerId::new(Order::Middle, egui::Id::new("camada"));
        let mut f = Fosco::default();
        f.pedir_foto(&ctx, vec![], vec![camada], 10.0, 1);
        let tela = vec2(100.0, 80.0);
        let errada = ColorImage::filled([300, 200], cinza(40));
        // a primeira, ainda no prazo: uma segunda tentativa, que não renova o prazo
        f.receber(&ctx, &errada, tela, 1.0, 10.1);
        let pedido = f.pedido.as_ref().expect("a segunda tentativa");
        assert_eq!((pedido.tentativas, pedido.desde), (2, 10.0));
        assert!(f.camadas.is_empty());
        // a segunda também veio errada: acabou, a camada fica no vidro liso e nenhuma foto vira fundo
        f.receber(&ctx, &errada, tela, 1.0, 10.2);
        assert!(f.pedido.is_none(), "nenhum pedido a mais");
        assert_eq!(f.camadas.len(), 1);
        assert!(f.camadas[0].id == camada && f.camadas[0].destino == Destino::Liso);
        assert!((f.fotos, f.fundos.len()) == (0, 0));
        // uma foto certa depois (de outro pedido) segue valendo
        f.pedir_foto(&ctx, vec![], vec![camada], 11.0, 1);
        f.receber(&ctx, &ColorImage::filled([100, 80], cinza(40)), tela, 1.0, 11.1);
        assert_eq!((f.fotos, f.fundos.len()), (1, 1));
    }

    /// C4: com o prazo da espera já gasto, a foto errada não pede outra: o vidro liso na hora.
    #[test]
    fn foto_errada_depois_do_prazo_nao_pede_outra() {
        let ctx = Context::default();
        let camada = LayerId::new(Order::Middle, egui::Id::new("camada"));
        let mut f = Fosco::default();
        f.pedir_foto(&ctx, vec![], vec![camada], 10.0, 1);
        f.receber(&ctx, &ColorImage::filled([300, 200], cinza(40)), vec2(100.0, 80.0), 1.0, 10.0 + ESPERA_MAX + 0.1);
        assert!(f.pedido.is_none());
        assert_eq!(f.camadas.iter().map(|c| c.destino).collect::<Vec<_>>(), [Destino::Liso]);
    }

    #[test]
    fn reconhece_o_quadro_das_dicas_do_egui() {
        let fundo = Color32::from_rgba_unmultiplied(30, 31, 35, 199);
        let borda = Color32::from_rgba_unmultiplied(255, 255, 255, 20);
        let quadro = |f: Color32, b: Color32| {
            RectShape::new(
                Rect::from_min_size(Pos2::ZERO, vec2(100.0, 30.0)),
                12,
                f,
                (1.0, b),
                egui::StrokeKind::Inside,
            )
        };
        assert_eq!(quadro_de_vidro(&quadro(fundo, borda), fundo, borda), Some(1.0));
        // entrando, com 40% de opacidade
        let k = quadro_de_vidro(&quadro(fundo.gamma_multiply(0.4), borda.gamma_multiply(0.4)), fundo, borda);
        assert!(k.is_some_and(|k| (k - 0.4).abs() < 0.01), "{k:?}");
        // outro fundo, sem borda ou com textura não é a dica
        assert_eq!(quadro_de_vidro(&quadro(Color32::from_rgb(30, 31, 35), borda), fundo, borda), None);
        let mut sem_borda = quadro(fundo, borda);
        sem_borda.stroke.width = 0.0;
        assert_eq!(quadro_de_vidro(&sem_borda, fundo, borda), None);
        let uv = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0));
        let com_textura = quadro(fundo, borda).with_texture(TextureId::default(), uv);
        assert_eq!(quadro_de_vidro(&com_textura, fundo, borda), None);
    }

    /// O custo do desfoque de uma foto da janela (1440×900 a 1 px por ponto e 2880×1800 a 2): rode com
    /// `cargo test -p interface --release --lib fosco::testes::custo -- --ignored --nocapture`.
    #[test]
    #[ignore = "medida de tempo; rode à mão em --release"]
    fn custo_do_desfoque() {
        for (w, h, ppp) in [(1440, 900, 1.0), (2880, 1800, 2.0)] {
            let foto =
                imagem(w, h, |x, y| if (x / 3 + y / 12) % 2 == 0 { cinza(230) } else { Color32::from_rgb(30, 31, 35) });
            let mut melhor = [f32::INFINITY; 4];
            for _ in 0..10 {
                let inicio = Instant::now();
                let r = fundo_desfocado(&foto, ppp);
                std::hint::black_box(r);
                melhor[0] = melhor[0].min(inicio.elapsed().as_secs_f32() * 1000.0);
                let fator = (PONTOS_POR_TEXEL * ppp).round() as usize;
                let inicio = Instant::now();
                let pequena = std::hint::black_box(reduzir_amostrando(&foto, fator, fator / 2));
                melhor[1] = melhor[1].min(inicio.elapsed().as_secs_f32() * 1000.0);
                let inicio = Instant::now();
                let mut d = std::hint::black_box(desfocar(&pequena, DESFOQUE * ppp / fator as f32));
                melhor[2] = melhor[2].min(inicio.elapsed().as_secs_f32() * 1000.0);
                let inicio = Instant::now();
                saturar(&mut d, SATURACAO);
                std::hint::black_box(d);
                melhor[3] = melhor[3].min(inicio.elapsed().as_secs_f32() * 1000.0);
            }
            let [total, reducao, desfoque, saturacao] = melhor;
            println!(
                "{w}×{h} a {ppp} px/pt: {total:.2} ms (redução {reducao:.2}, desfoque {desfoque:.2}, saturação \
                 {saturacao:.2})"
            );
        }
    }
}
