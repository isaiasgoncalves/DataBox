//! A tabela simples (`.lista-t`, Main.dc.html, Inicio.dc.html e DadosClaro.dc.html): a das listas que não são a
//! grade de dados — Início › Abertos recentemente, Visão geral › Tabelas, Estrutura › Colunas e Chaves
//! estrangeiras, Índices.
//!
//! - `th`: 30px, 11.5px / 600 em `--t2` (sem caixa alta), `padding:0 12px`, fundo `Cabecalho` com as riscas (a
//!   regra `.faixa,.grade th,.lista-t th` do fim do CSS) e o fio `--btb` embaixo.
//! - `td`: 32px ([`ALTURA_TD`]; as listas do Início e da Estrutura usam 34 e 36px, [`Tabela::altura_linha`]),
//!   `padding:0 12px`, fio `--b` entre as linhas (a última sem), conteúdo centrado na vertical; conteúdo mais alto
//!   que a linha a estica.
//! - Hover da linha: `--selh`; `tr.sel`: `--sel` ([`Tabela::marcadas`], no lugar do hover; o clique na linha que a
//!   seleciona vem em [`RespostaTabela::linha_clicada`], e o clique duplo que abre em
//!   [`RespostaTabela::linha_dupla`], com [`Tabela::clicavel`]); `tr.inativo`: o texto
//!   em `--t3` ([`Tabela::inativas`]: como o conteúdo é livre, a célula toda fica na opacidade que leva o `--t` ao
//!   `--t3` sobre o documento; o `nome_abre` da linha usa `inativo = true`).
//! - `td.num` / `th.num` e `td.acoes` à direita ([`ColunaTabela::direita`]); a coluna que fica com a sobra da
//!   largura e encurta com "…" ([`LarguraColuna::Estica`], o `td.caminho-curto`: use
//!   [`crate::pecas::texto::texto_cortado`] nela); sem coluna que estica, a sobra se reparte entre as colunas
//!   automáticas na proporção da largura de cada uma (como o `table{width:100%}` do navegador). Mais largo que
//!   o card, a tabela rola na horizontal (o `.scroller`).
//!
//! O conteúdo de cada célula é livre (`|ui, linha, coluna|`): `nome_abre`, `texto`, `pill`, `tag`, `codigo`,
//! `Botao::ghost`… num `Ui` da largura da célula, em linha e centrado na vertical. As larguras automáticas e as
//! alturas das linhas são medidas no quadro anterior (o egui refaz a passada sem mostrar a errada quando mudam).
//!
//! ```ignore
//! let colunas = [ColunaTabela::new("Nome"), ColunaTabela::new("Linhas").direita(), ColunaTabela::new("").direita()];
//! Tabela::new("tabelas", &colunas, objetos.len()).mostrar(ui, |ui, l, c| match c {
//!     0 => { nome_abre(ui, &objetos[l].nome, false); }
//!     1 => { texto(ui, &formatar_numero(objetos[l].linhas), 12.0, p.fg); }
//!     _ => { ui.add(Botao::ghost("Estrutura")); }
//! });
//! ```

use egui::{Align, Id, Layout, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::grade::{Th, largura_cabecalho, padding_coluna, pintar_cabecalho_coluna, pintar_fundo_cabecalho};
use crate::tema::{Paleta, paleta};

/// Altura do `th` (`.lista-t th{height:30px}`, com o fio `--btb` embaixo).
pub const ALTURA_TH: f32 = 30.0;
/// Altura de uma linha (`.lista-t td{height:32px}`, com o fio de 1px embaixo; a última não tem o fio).
pub const ALTURA_TD: f32 = 32.0;
/// Altura mínima do conteúdo de uma linha: o `td` de 32px menos o fio.
pub const ALTURA_CONTEUDO_MIN: f32 = ALTURA_TD - 1.0;
/// Padding lateral das células e dos `th` (`.lista-t th, .lista-t td{padding:0 12px}`), igual em todas as colunas
/// (o que [`crate::grade::padding_coluna`] devolve).
pub const PADDING_TABELA: f32 = 12.0;
/// Largura mínima da coluna que estica (sem o padding).
pub const LARGURA_MIN_ESTICA: f32 = 60.0;

/// Como a coluna ganha largura.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LarguraColuna {
    /// A largura do conteúdo mais largo (e do `th`), mais a parte dela na sobra.
    #[default]
    Auto,
    /// Largura fixa, com o padding.
    Fixa(f32),
    /// Fica com a sobra da largura e encurta com "…" (o `td.caminho-curto`: `max-width:0; width:100%`).
    Estica,
}

/// Uma coluna da [`Tabela`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColunaTabela<'a> {
    /// O `th` (vazio na coluna das ações).
    pub titulo: &'a str,
    pub largura: LarguraColuna,
    /// À direita (`td.num`, `td.acoes`).
    pub direita: bool,
}

impl<'a> ColunaTabela<'a> {
    pub fn new(titulo: &'a str) -> Self {
        ColunaTabela { titulo, largura: LarguraColuna::Auto, direita: false }
    }

    /// Alinhada à direita (números `td.num`, ações `td.acoes`).
    pub fn direita(mut self) -> Self {
        self.direita = true;
        self
    }

    /// Fica com a sobra da largura e encurta com "…".
    pub fn estica(mut self) -> Self {
        self.largura = LarguraColuna::Estica;
        self
    }

    /// Largura fixa (com o padding).
    pub fn fixa(mut self, largura: f32) -> Self {
        self.largura = LarguraColuna::Fixa(largura);
        self
    }
}

/// O que aconteceu na tabela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RespostaTabela {
    /// A linha sob o ponteiro (a do hover).
    pub linha_sob_ponteiro: Option<usize>,
    /// A linha clicada fora dos controles dela ([`Tabela::clicavel`]: o `tr` que seleciona).
    pub linha_clicada: Option<usize>,
    /// A linha com o clique duplo fora dos controles dela ([`Tabela::clicavel`]: o `tr` que abre, como na árvore).
    pub linha_dupla: Option<usize>,
    /// Onde a tabela ficou.
    pub rect: Rect,
}

#[derive(Clone, Default, PartialEq)]
struct Medidas {
    naturais: Vec<f32>,
    /// A altura do conteúdo de cada linha quando passa da mínima (0: cabe na linha).
    alturas: Vec<f32>,
}

type Predicado<'a> = Box<dyn Fn(usize) -> bool + 'a>;

/// A opacidade que leva o `--t` ao `--t3` sobre o documento: o `tr.inativo td{color:--t3}` para o conteúdo livre
/// das células (os textos em `--t` ficam em `--t3`; o resto apaga na mesma medida).
fn opacidade_inativa(p: &Paleta) -> f32 {
    let e = &p.estudio;
    let canal = |c: egui::Color32| f32::from(c.r());
    let (t, t3, fundo) = (canal(e.t), canal(e.t3), canal(e.doc));
    if (t - fundo).abs() < 1.0 { 0.6 } else { ((t3 - fundo) / (t - fundo)).clamp(0.3, 1.0) }
}

/// A tabela simples `.lista-t` (veja o topo do módulo).
#[must_use = "chame .mostrar(ui, |ui, linha, coluna| …)"]
pub struct Tabela<'a> {
    id: Id,
    colunas: &'a [ColunaTabela<'a>],
    linhas: usize,
    inativa: Option<Predicado<'a>>,
    marcada: Option<Predicado<'a>>,
    cabecalho: bool,
    altura_linha: f32,
    clicavel: bool,
    recuo_extra: f32,
}

/// As larguras das colunas para a largura disponível (`naturais` já com o padding).
pub fn distribuir_larguras(colunas: &[ColunaTabela<'_>], naturais: &[f32], disponivel: f32) -> Vec<f32> {
    let n = colunas.len();
    let mut larguras: Vec<f32> = (0..n)
        .map(|c| match colunas.get(c).map(|col| col.largura) {
            Some(LarguraColuna::Fixa(w)) => w,
            Some(LarguraColuna::Estica) => {
                let (e, d) = padding_coluna(c, n);
                LARGURA_MIN_ESTICA + e + d
            }
            _ => naturais.get(c).copied().unwrap_or(0.0),
        })
        .collect();
    let total: f32 = larguras.iter().sum();
    let sobra = disponivel - total;
    if sobra <= 0.0 {
        return larguras;
    }
    let esticam: Vec<usize> = (0..n).filter(|&c| colunas[c].largura == LarguraColuna::Estica).collect();
    if !esticam.is_empty() {
        let parte = sobra / esticam.len() as f32;
        for c in esticam {
            larguras[c] += parte;
        }
        return larguras;
    }
    let automaticas: Vec<usize> = (0..n).filter(|&c| colunas[c].largura == LarguraColuna::Auto).collect();
    let base: f32 = automaticas.iter().map(|&c| larguras[c]).sum();
    if base > 0.0 {
        for c in automaticas {
            larguras[c] += sobra * larguras[c] / base;
        }
    }
    larguras
}

impl<'a> Tabela<'a> {
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        colunas: &'a [ColunaTabela<'a>],
        linhas: usize,
    ) -> Self {
        Tabela {
            id: Id::new(("databox::tabela", id_salt)),
            colunas,
            linhas,
            inativa: None,
            marcada: None,
            cabecalho: true,
            altura_linha: ALTURA_TD,
            clicavel: false,
            recuo_extra: 0.0,
        }
    }

    /// Mais recuo nas pontas: soma `extra` ao padding da esquerda da primeira coluna e ao da direita da última (os
    /// Índices alinham o texto da lista com o resto da página, a 15px da borda).
    pub fn recuo_lateral(mut self, extra: f32) -> Self {
        self.recuo_extra = extra.max(0.0);
        self
    }

    /// O clique na linha (fora dos botões e links das células, que ficam por cima) vira
    /// [`RespostaTabela::linha_clicada`]: a seleção de uma linha (`tr.sel`, com [`Tabela::marcadas`]) da Estrutura e
    /// dos Índices. A linha não é parada do Tab: o teclado chega pelos controles dela.
    pub fn clicavel(mut self) -> Self {
        self.clicavel = true;
        self
    }

    /// `tr.inativo`: o texto da linha em `--t3` (o arquivo que não existe mais nos recentes).
    pub fn inativas(mut self, f: impl Fn(usize) -> bool + 'a) -> Self {
        self.inativa = Some(Box::new(f));
        self
    }

    /// `tr.sel`: o fundo `--sel` nas linhas marcadas (no lugar do `--selh` do hover).
    pub fn marcadas(mut self, f: impl Fn(usize) -> bool + 'a) -> Self {
        self.marcada = Some(Box::new(f));
        self
    }

    /// Sem a linha dos `th`.
    pub fn sem_cabecalho(mut self) -> Self {
        self.cabecalho = false;
        self
    }

    /// A altura mínima das linhas, com o fio (padrão [`ALTURA_TD`], 32px; a lista do Início usa 34px e as da
    /// Estrutura, 36px).
    pub fn altura_linha(mut self, altura: f32) -> Self {
        self.altura_linha = altura.max(2.0);
        self
    }

    /// Desenha a tabela na largura disponível; `celula(ui, linha, coluna)` põe o conteúdo de cada célula.
    pub fn mostrar(self, ui: &mut Ui, mut celula: impl FnMut(&mut Ui, usize, usize)) -> RespostaTabela {
        let p = paleta(ui);
        let id = self.id;
        let id_medidas = id.with("medidas");
        let antes: Medidas = ui.ctx().data(|d| d.get_temp(id_medidas)).unwrap_or_default();
        let n = self.colunas.len();
        let extra = self.recuo_extra;
        let padding_coluna = |c: usize, n: usize| {
            let (e, d) = padding_coluna(c, n);
            (e + if c == 0 { extra } else { 0.0 }, d + if c + 1 == n { extra } else { 0.0 })
        };
        let disponivel = ui.available_width();
        let naturais: Vec<f32> = self
            .colunas
            .iter()
            .enumerate()
            .map(|(c, col)| {
                let (e, d) = padding_coluna(c, n);
                let th = if col.titulo.is_empty() { 0.0 } else { largura_cabecalho(ui.painter(), col.titulo, None) };
                (th.max(antes.naturais.get(c).copied().unwrap_or(0.0)) + e + d).ceil()
            })
            .collect();
        let larguras = distribuir_larguras(self.colunas, &naturais, disponivel);
        let largura_tabela: f32 = larguras.iter().sum::<f32>().max(disponivel);
        let mut medidas = Medidas { naturais: vec![0.0; n], alturas: Vec::with_capacity(self.linhas) };
        let mut sob_ponteiro = None;
        let mut clicada = None;
        let mut dupla = None;

        let saida = ScrollArea::horizontal().id_salt(id.with("rolagem")).auto_shrink([false, true]).show(ui, |ui| {
            let topo = ui.cursor().min;
            let x0 = topo.x;
            let mut xs = Vec::with_capacity(n + 1);
            let mut x = x0;
            for l in &larguras {
                xs.push(x);
                x += l;
            }
            xs.push(x);
            let mut y = topo.y;
            if self.cabecalho {
                let cab = Rect::from_min_size(pos2(x0, y), vec2(largura_tabela, ALTURA_TH));
                pintar_fundo_cabecalho(ui.painter(), p, cab);
                for (c, col) in self.colunas.iter().enumerate() {
                    if col.titulo.is_empty() {
                        continue;
                    }
                    let (e, d) = padding_coluna(c, n);
                    // o texto centrado nos 29px acima do fio
                    let conteudo = Rect::from_min_max(
                        pos2(xs[c] + e, y),
                        pos2((xs[c + 1] - d).max(xs[c] + e), y + ALTURA_TH - 1.0),
                    );
                    let th = Th { numerica: col.direita, ..Th::new(col.titulo) };
                    let recorte = ui.painter().with_clip_rect(
                        Rect::from_min_max(pos2(xs[c], y), pos2(xs[c + 1], y + ALTURA_TH)).intersect(ui.clip_rect()),
                    );
                    pintar_cabecalho_coluna(&recorte, p, conteudo, &th);
                }
                y += ALTURA_TH;
            }
            let opacidade = opacidade_inativa(p);
            for l in 0..self.linhas {
                let ultima = l + 1 == self.linhas;
                // `.lista-t tr:last-child td{border-bottom:0}`
                let fio = if ultima { 0.0 } else { 1.0 };
                let minima = self.altura_linha - fio;
                let altura_conteudo = antes.alturas.get(l).copied().unwrap_or(0.0).max(minima);
                let altura = altura_conteudo + fio;
                let faixa = Rect::from_min_size(pos2(x0, y), vec2(largura_tabela, altura));
                let sob = ui.rect_contains_pointer(faixa);
                if sob {
                    sob_ponteiro = Some(l);
                }
                // antes das células: os controles delas ficam por cima e ganham o clique
                if self.clicavel {
                    let r = ui.interact(faixa, id.with(("linha", l)), Sense::CLICK);
                    if r.clicked() {
                        clicada = Some(l);
                    }
                    if r.double_clicked() {
                        dupla = Some(l);
                    }
                }
                if self.marcada.as_ref().is_some_and(|f| f(l)) {
                    ui.painter().rect_filled(faixa, 0.0, p.estudio.sel);
                } else if sob {
                    ui.painter().rect_filled(faixa, 0.0, p.estudio.selh);
                }
                if !ultima {
                    ui.painter().hline(faixa.x_range(), faixa.bottom() - 0.5, Stroke::new(1.0, p.estudio.b));
                }
                let inativa = self.inativa.as_ref().is_some_and(|f| f(l));
                let mut maior: f32 = 0.0;
                for (c, col) in self.colunas.iter().enumerate() {
                    let (e, d) = padding_coluna(c, n);
                    let dentro = Rect::from_min_max(
                        pos2(xs[c] + e, y),
                        pos2((xs[c + 1] - d).max(xs[c] + e), y + altura_conteudo),
                    );
                    let layout = if col.direita {
                        Layout::right_to_left(Align::Center)
                    } else {
                        Layout::left_to_right(Align::Center)
                    };
                    let mut filho =
                        ui.new_child(UiBuilder::new().id_salt(id.with((l, c))).max_rect(dentro).layout(layout));
                    let recorte = Rect::from_min_max(pos2(xs[c], y), pos2(xs[c + 1], y + altura));
                    filho.set_clip_rect(recorte.intersect(ui.clip_rect()));
                    filho.spacing_mut().item_spacing.x = 4.0;
                    if inativa {
                        filho.multiply_opacity(opacidade);
                    }
                    celula(&mut filho, l, c);
                    let usado = filho.min_rect();
                    if usado.is_positive() {
                        maior = maior.max(usado.height());
                        if let Some(m) = medidas.naturais.get_mut(c) {
                            *m = m.max(usado.width());
                        }
                    }
                }
                // só o que passa da mínima conta (o layout centrado ocupa a altura toda que recebe: guardar a
                // altura usada prenderia a linha na altura de quando era a última, sem o fio)
                medidas.alturas.push(if maior > minima + 0.5 { maior } else { 0.0 });
                y += altura;
            }
            let rect = Rect::from_min_max(topo, pos2(x0 + largura_tabela, y));
            ui.allocate_rect(rect, Sense::hover());
            rect
        });

        // a coluna que estica não mede: ela fica com a sobra
        for (c, col) in self.colunas.iter().enumerate() {
            if col.largura == LarguraColuna::Estica
                && let Some(m) = medidas.naturais.get_mut(c)
            {
                *m = 0.0;
            }
        }
        let mudou = medidas.alturas.len() != antes.alturas.len()
            || medidas.naturais.len() != antes.naturais.len()
            || medidas.alturas.iter().zip(&antes.alturas).any(|(a, b)| (a - b).abs() > 0.5)
            || medidas.naturais.iter().zip(&antes.naturais).any(|(a, b)| (a - b).abs() > 0.5);
        if mudou {
            ui.ctx().data_mut(|d| d.insert_temp(id_medidas, medidas));
            ui.ctx().request_discard("medidas da tabela");
        }
        RespostaTabela {
            linha_sob_ponteiro: sob_ponteiro,
            linha_clicada: clicada,
            linha_dupla: dupla,
            rect: saida.inner,
        }
    }
}

/// A moldura das listas no corpo de um documento (`border:1px solid --b2; border-radius:10px; overflow:hidden`: a
/// de Tabelas da Visão geral, as da Estrutura): o conteúdo (uma [`Tabela`], um vazio) por dentro da borda, na
/// largura disponível, com o que passa dos cantos arredondados coberto pelo fundo do documento (`--c`).
pub fn moldura_lista<R>(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let p = paleta(ui);
    let largura = ui.available_width();
    let topo = ui.cursor().min;
    let dentro = Rect::from_min_size(topo + vec2(1.0, 1.0), vec2((largura - 2.0).max(0.0), f32::INFINITY));
    let mut filho = ui.new_child(UiBuilder::new().max_rect(dentro).layout(Layout::top_down(Align::Min)));
    let r = conteudo(&mut filho);
    let rect = Rect::from_min_size(topo, vec2(largura, filho.min_rect().height().max(0.0) + 2.0));
    ui.allocate_rect(rect, Sense::hover());
    let painter = ui.painter();
    painter.add(cantos_por_fora(rect.shrink(1.0), RAIO_MOLDURA - 1.0, p.estudio.doc));
    let cantos = crate::pintura::canto(rect, RAIO_MOLDURA);
    painter.rect_stroke(rect, cantos, Stroke::new(1.0, p.estudio.b2), egui::StrokeKind::Inside);
    r
}

/// O raio da moldura das listas (o raio único).
const RAIO_MOLDURA: f32 = crate::pintura::RAIO_PADRAO;

/// O que fica fora dos cantos arredondados de raio `r` em `rect`, pintado com `cor` (o `overflow:hidden` de uma
/// caixa arredondada sobre conteúdo retangular): em cada canto, um leque do vértice do retângulo até o arco.
fn cantos_por_fora(rect: Rect, r: f32, cor: egui::Color32) -> egui::Shape {
    use std::f32::consts::FRAC_PI_2;
    const PASSOS: usize = 8;
    let r = crate::pintura::raio(rect, r);
    if r <= 0.0 {
        return egui::Shape::Noop;
    }
    let mut m = egui::epaint::Mesh::default();
    // (o vértice, o centro do arco, o ângulo inicial)
    let cantos = [
        (rect.left_top(), pos2(rect.left() + r, rect.top() + r), 2.0 * FRAC_PI_2),
        (rect.right_top(), pos2(rect.right() - r, rect.top() + r), 3.0 * FRAC_PI_2),
        (rect.right_bottom(), pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (rect.left_bottom(), pos2(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
    ];
    for (vertice, centro, inicio) in cantos {
        let base = m.vertices.len() as u32;
        m.colored_vertex(vertice, cor);
        for k in 0..=PASSOS {
            let a = inicio + FRAC_PI_2 * k as f32 / PASSOS as f32;
            m.colored_vertex(centro + vec2(a.cos(), a.sin()) * r, cor);
        }
        for k in 0..PASSOS as u32 {
            m.add_triangle(base, base + 1 + k, base + 2 + k);
        }
    }
    egui::Shape::mesh(m)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cantos_por_fora_cobrem_so_os_cantos() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let egui::Shape::Mesh(m) = cantos_por_fora(rect, 9.0, egui::Color32::RED) else { panic!("uma malha") };
        let area: f32 = m
            .indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [0, 1, 2].map(|i| m.vertices[t[i] as usize].pos);
                ((b - a).x * (c - a).y - (b - a).y * (c - a).x).abs() / 2.0
            })
            .sum();
        // quatro vezes r² − πr²/4, com o arco em 8 cordas (cada uma entra no máximo 0,2px no círculo)
        let esperada = 4.0 * (81.0 - std::f32::consts::PI * 81.0 / 4.0);
        assert!(area >= esperada && area - esperada < 2.0, "{area} ≠ {esperada}");
    }

    #[test]
    fn sobra_vai_para_a_coluna_que_estica() {
        let colunas = [ColunaTabela::new("Nome"), ColunaTabela::new("Pasta").estica(), ColunaTabela::new("").direita()];
        let l = distribuir_larguras(&colunas, &[120.0, 999.0, 200.0], 1000.0);
        assert_eq!(l[0], 120.0);
        assert_eq!(l[2], 200.0);
        assert_eq!(l.iter().sum::<f32>(), 1000.0, "a pasta fica com o resto");
        // sem espaço, a que estica fica no mínimo, com o `padding:0 12px` do `.lista-t td`
        let l = distribuir_larguras(&colunas, &[600.0, 0.0, 600.0], 1000.0);
        assert_eq!(l[1], LARGURA_MIN_ESTICA + 12.0 + 12.0);
    }

    #[test]
    fn inativa_leva_o_texto_ao_t3() {
        use crate::tema::{CLARA, ESCURA};
        for p in [&CLARA, &ESCURA] {
            let e = &p.estudio;
            let o = opacidade_inativa(p);
            let r = f32::from(e.doc.r()) + (f32::from(e.t.r()) - f32::from(e.doc.r())) * o;
            assert!((r - f32::from(e.t3.r())).abs() < 1.0, "o --t a {o} sobre o documento: {r}");
        }
    }

    #[test]
    fn sem_coluna_que_estica_a_sobra_se_reparte_na_proporcao() {
        let colunas =
            [ColunaTabela::new("Nome"), ColunaTabela::new("Linhas").direita(), ColunaTabela::new("").fixa(90.0)];
        let l = distribuir_larguras(&colunas, &[200.0, 100.0, 0.0], 690.0);
        assert_eq!(l[2], 90.0, "a fixa não muda");
        assert!((l[0] - 400.0).abs() < 0.01 && (l[1] - 200.0).abs() < 0.01, "{l:?}");
        // mais largo que o espaço: fica como está (a tabela rola)
        let l = distribuir_larguras(&colunas, &[500.0, 400.0, 0.0], 690.0);
        assert_eq!(l, vec![500.0, 400.0, 90.0]);
    }
}
