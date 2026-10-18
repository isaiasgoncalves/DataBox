//! Arranjos que o CSS do kit faz com flex/grid e o egui não tem pronto: linha com ações à direita na ordem
//! natural (o `.spacer` do kit), duas colunas iguais (`.campos`, `.grid.g2`) e fluxo que quebra linha
//! (`.demo-linha`, `.acoes-linha`).

use egui::{Align, Id, Layout, Rect, Ui, UiBuilder, pos2, vec2};

/// Uma linha de altura fixa: `esquerda` a partir da esquerda e `direita` encostado à direita, os dois na
/// ordem natural (da esquerda para a direita), centrados na vertical. A largura da direita vem do quadro
/// anterior; quando muda, o egui refaz a passada sem mostrar a errada (`request_discard`).
/// Devolve o retângulo do que foi desenhado (vazio, `Rect::NOTHING`, se nada foi).
pub fn linha_com_direita(
    ui: &mut Ui,
    id: Id,
    altura: f32,
    espaco: f32,
    esquerda: impl FnOnce(&mut Ui),
    direita: impl FnOnce(&mut Ui),
) -> Rect {
    let id_largura = id.with("largura_direita");
    let largura_antes: f32 = ui.ctx().data(|d| d.get_temp(id_largura)).unwrap_or(0.0);
    let topo = ui.cursor().min;
    let largura = ui.available_width();
    let linha = Rect::from_min_size(topo, vec2(largura, altura));

    let limite_esquerda =
        (linha.right() - largura_antes - if largura_antes > 0.0 { espaco } else { 0.0 }).max(linha.left());
    let rect_esq = Rect::from_min_max(linha.min, pos2(limite_esquerda, linha.bottom()));
    let mut ui_esq = ui.new_child(
        UiBuilder::new().id_salt(id.with("esq")).max_rect(rect_esq).layout(Layout::left_to_right(Align::Center)),
    );
    esquerda(&mut ui_esq);

    let rect_dir = Rect::from_min_max(pos2(linha.right() - largura_antes, linha.top()), linha.max);
    let mut ui_dir = ui.new_child(
        UiBuilder::new()
            .id_salt(id.with("dir"))
            .max_rect(rect_dir.with_max_x(linha.right() + 4000.0))
            .layout(Layout::left_to_right(Align::Center)),
    );
    direita(&mut ui_dir);
    let usada = if ui_dir.min_rect().is_positive() { ui_dir.min_rect().width() } else { 0.0 };
    if (usada - largura_antes).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id_largura, usada));
        ui.ctx().request_discard("largura das ações à direita");
    }
    ui.allocate_rect(linha.union(ui_esq.min_rect()), egui::Sense::hover());
    // só o que foi desenhado (sem a linha reservada)
    let mut conteudo = Rect::NOTHING;
    for r in [ui_esq.min_rect(), ui_dir.min_rect()] {
        if r.is_positive() {
            conteudo = conteudo.union(r);
        }
    }
    conteudo
}

/// Duas colunas de mesma largura com `espaco` entre elas (o `grid-template-columns:repeat(2,minmax(0,1fr))`),
/// alinhadas em cima. Devolve o que cada lado devolveu.
pub fn duas_colunas<A, B>(
    ui: &mut Ui,
    espaco: f32,
    a: impl FnOnce(&mut Ui) -> A,
    b: impl FnOnce(&mut Ui) -> B,
) -> (A, B) {
    let largura = ui.available_width();
    let coluna = ((largura - espaco) / 2.0).max(0.0);
    let topo = ui.cursor().min;
    let rect_a = Rect::from_min_size(topo, vec2(coluna, f32::INFINITY));
    let rect_b = Rect::from_min_size(pos2(topo.x + coluna + espaco, topo.y), vec2(coluna, f32::INFINITY));
    let id = ui.next_auto_id();
    let mut ui_a =
        ui.new_child(UiBuilder::new().id_salt(id.with("a")).max_rect(rect_a).layout(Layout::top_down(Align::Min)));
    let ra = a(&mut ui_a);
    let mut ui_b =
        ui.new_child(UiBuilder::new().id_salt(id.with("b")).max_rect(rect_b).layout(Layout::top_down(Align::Min)));
    let rb = b(&mut ui_b);
    let altura = ui_a.min_rect().height().max(ui_b.min_rect().height());
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, altura)), egui::Sense::hover());
    ui.skip_ahead_auto_ids(1);
    (ra, rb)
}

/// `n` colunas de mesma largura (o `.grid.g2/.g3/.g4`), alinhadas em cima; o conteúdo recebe o índice da
/// coluna (um `FnMut` só, para poder usar o mesmo estado em todas).
pub fn colunas(ui: &mut Ui, n: usize, espaco: f32, mut conteudo: impl FnMut(&mut Ui, usize)) {
    let n = n.max(1);
    let largura = ui.available_width();
    let coluna = ((largura - espaco * (n as f32 - 1.0)) / n as f32).max(0.0);
    let topo = ui.cursor().min;
    let id = ui.next_auto_id();
    let mut altura: f32 = 0.0;
    for i in 0..n {
        let rect =
            Rect::from_min_size(pos2(topo.x + i as f32 * (coluna + espaco), topo.y), vec2(coluna, f32::INFINITY));
        let mut filho =
            ui.new_child(UiBuilder::new().id_salt(id.with(i)).max_rect(rect).layout(Layout::top_down(Align::Min)));
        conteudo(&mut filho, i);
        altura = altura.max(filho.min_rect().height());
    }
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, altura)), egui::Sense::hover());
    ui.skip_ahead_auto_ids(1);
}

/// Chave onde um conteúdo esticável (o [`crate::pecas::cartao::Cartao::esticar`]) anota a altura natural dele.
pub(crate) fn id_altura_natural(ui: &Ui) -> Id {
    ui.id().with("databox::altura_natural")
}

/// Como [`colunas`], com as colunas da mesma altura (o `align-items:stretch` do `.grid`): cada coluna recebe a
/// altura da mais alta, medida no quadro anterior. Um card com [`crate::pecas::cartao::Cartao::esticar`] ocupa
/// essa altura toda.
pub fn colunas_iguais(ui: &mut Ui, n: usize, espaco: f32, mut conteudo: impl FnMut(&mut Ui, usize)) {
    let n = n.max(1);
    let largura = ui.available_width();
    let coluna = ((largura - espaco * (n as f32 - 1.0)) / n as f32).max(0.0);
    let topo = ui.cursor().min;
    let id = ui.next_auto_id();
    let id_altura = id.with("altura");
    let altura_antes: Option<f32> = ui.ctx().data(|d| d.get_temp(id_altura));
    let mut maior_natural: f32 = 0.0;
    for i in 0..n {
        let altura = altura_antes.unwrap_or(f32::INFINITY);
        let rect = Rect::from_min_size(pos2(topo.x + i as f32 * (coluna + espaco), topo.y), vec2(coluna, altura));
        let mut filho =
            ui.new_child(UiBuilder::new().id_salt(id.with(i)).max_rect(rect).layout(Layout::top_down(Align::Min)));
        conteudo(&mut filho, i);
        let natural: Option<f32> = ui.ctx().data(|d| d.get_temp(id_altura_natural(&filho)));
        maior_natural = maior_natural.max(natural.unwrap_or_else(|| filho.min_rect().height()));
    }
    if altura_antes.is_none_or(|a| (a - maior_natural).abs() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(id_altura, maior_natural));
        ui.ctx().request_discard("altura das colunas");
    }
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, maior_natural)), egui::Sense::hover());
    ui.skip_ahead_auto_ids(1);
}

/// Itens lado a lado que quebram linha, com `espaco` entre eles nas duas direções (o `display:flex;
/// flex-wrap:wrap; gap` do kit), centrados na vertical em cada linha.
pub fn fluxo<R>(ui: &mut Ui, espaco: f32, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(espaco, espaco);
        conteudo(ui)
    })
    .inner
}

/// Itens lado a lado numa linha de altura fixa (`altura`), centrados na vertical, com `espaco` entre eles. A
/// linha ocupa só a largura do que foi desenhado. (O `ui.horizontal` do egui reserva a altura de um botão,
/// 30px, mesmo para uma linha de texto pequeno.)
pub fn linha<R>(ui: &mut Ui, altura: f32, espaco: f32, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), altura));
    let id = ui.next_auto_id();
    let mut filho =
        ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    filho.spacing_mut().item_spacing.x = espaco;
    let r = conteudo(&mut filho);
    let direita = if filho.min_rect().is_positive() { filho.min_rect().right() } else { rect.left() };
    ui.allocate_rect(Rect::from_min_max(rect.min, pos2(direita.max(rect.left()), rect.bottom())), egui::Sense::hover());
    ui.skip_ahead_auto_ids(1);
    r
}

/// Itens lado a lado sem quebrar linha numa linha de 30px (a altura dos botões), com `espaco` entre eles.
pub fn em_linha<R>(ui: &mut Ui, espaco: f32, conteudo: impl FnOnce(&mut Ui) -> R) -> R {
    linha(ui, 30.0, espaco, conteudo)
}
