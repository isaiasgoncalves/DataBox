//! O aviso flutuante (o toast): o vidro das camadas flutuantes (`.flut` das pranchetas: fundo `--flut` opaco,
//! borda `--vidrob`, realce em cima e a sombra `--sh`; no claro, borda e sombra mais firmes) em pílula, fixo no
//! rodapé, centrado, 24px acima da borda (e acima do limite que quem chama dá: no editor, a borda de baixo do
//! cartão do documento, com a barra de status à vista), com o sólido de estado e o texto
//! em 12px `--t`; `role=status`. Some sozinho em 2,6s; entra e sai em 180ms subindo 14px.
//!
//! Qualquer tela chama [`toast_concluido`] (o que foi feito: "Linha salva.", "Copiado."), [`toast_erro`] (o que
//! falhou), [`toast_atencao`] (o que não foi feito ou ficou pela metade) ou [`toast`] (a informação neutra, com o
//! "i"); a casca chama [`desenhar_toast`] uma vez por quadro, depois de tudo. [`toast_concluido_com_acao`] põe um botão
//! pequeno dentro da pílula (`.toast.com-acao`: "Cópia salva: x.db · 119 MB" com "Mostrar na pasta"; o toast fica
//! [`DURACAO_TOAST_COM_ACAO`] e traz o texto da dica, o caminho inteiro): o clique sai em [`acao_do_toast`], que a casca
//! consulta a cada quadro, e o toast some; [`toast_concluido_com_acao_por`] dá à ação outro prazo.

use std::time::Duration;

use egui::epaint::RectShape;
use egui::{Area, Color32, Context, Id, Order, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::pecas::ajuda::{IconeEstado, pintar_icone_estado};
use crate::pecas::botao::{ALTURA_PEQUENO, Botao};
use crate::pecas::campos::pintar_vidro_flutuante;
use crate::pintura;
use crate::tema::{SombraCss, movimento_reduzido_ctx, paleta_ctx};

/// Quanto tempo o toast fica na tela.
pub const DURACAO_TOAST: f64 = 2.6;
const TRANSICAO: f64 = 0.18;

/// Altura do toast: a linha de 12px × 1,4 no `padding:9px 16px` da pílula, com a borda de 1px (o `.toast` das
/// pranchetas 8 e 15: 36,8px, arredondados).
const ALTURA_TOAST: f32 = 37.0;
/// Raio do vidro: pílula, como os botões e as etiquetas (decisão de 16/10/2026; o vidro dos menus fica com 12).
pub const RAIO_TOAST: f32 = f32::INFINITY;
/// A distância até a borda de baixo do cartão (ou da janela, sem limite): 24px (revisão de 17/10/2026; eram 22).
const MARGEM_BAIXO: f32 = 24.0;
/// No tema claro a sombra e a borda do vidro ficam mais firmes no toast (a pílula é pequena e o `--vidrob` de 8%
/// quase some no fundo claro).
const SOMBRA_TOAST_CLARO: SombraCss =
    SombraCss { y: 3.0, desfoque: 12.0, espalhar: 0.0, cor: Color32::from_rgba_premultiplied(0, 0, 0, 56) };
const BORDA_TOAST_CLARO: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 56);
/// `padding:9px 16px` dos dois lados da pílula, dentro da borda de 1px (o 12px da esquerda era do vidro de cantos de
/// 12, de antes da pílula).
const RECUO_ESQUERDA: f32 = 17.0;
const RECUO_DIREITA: f32 = 17.0;
/// O sólido de estado (16px) e o `gap:8px` até o texto.
const LADO_ICONE: f32 = 16.0;
const ESPACO_ICONE: f32 = 8.0;

/// Quanto tempo o toast com ação fica na tela (a pessoa precisa de tempo para ler e chegar ao botão).
pub const DURACAO_TOAST_COM_ACAO: f64 = 7.0;
/// `.toast.com-acao{padding:5px 6px 5px 16px;gap:10px}`: o espaço entre o texto e o botão, e o do botão até a borda
/// (6px mais o fio de 1px).
const ESPACO_ACAO: f32 = 10.0;
const RECUO_DIREITA_ACAO: f32 = 7.0;
/// O `.btn.pq` da ação: `padding:0 10px` e a borda de 1px, no texto de 12px.
const PADDING_ACAO: f32 = 10.0;

/// A ação de um toast (o botão pequeno dentro da pílula, como o "Marcar as linhas sem par" da prancheta 16): o
/// rótulo e a `carga` que [`acao_do_toast`] devolve quando a pessoa clica (um caminho, um nome: o que quem chamou
/// precisa para cumprir a ação, que pode vir quadros depois e de outra tela).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcaoToast {
    pub rotulo: String,
    pub carga: String,
}

#[derive(Clone)]
struct EstadoToast {
    mensagem: String,
    icone: IconeEstado,
    desde: f64,
    acao: Option<AcaoToast>,
    /// O texto da dica da pílula (o caminho inteiro de um arquivo, por exemplo).
    dica: Option<String>,
    /// Quanto tempo fica na tela (sem contar a saída).
    duracao: f64,
}

impl EstadoToast {
    fn duracao(&self) -> f64 {
        self.duracao
    }
}

fn id_toast() -> Id {
    Id::new("databox::toast")
}

fn id_acao_clicada() -> Id {
    id_toast().with("acao_clicada")
}

fn mostrar(ctx: &Context, mensagem: String, icone: IconeEstado) {
    mostrar_com(ctx, mensagem, icone, None, None, DURACAO_TOAST);
}

fn mostrar_com(
    ctx: &Context,
    mensagem: String,
    icone: IconeEstado,
    acao: Option<AcaoToast>,
    dica: Option<String>,
    duracao: f64,
) {
    let desde = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(id_toast(), EstadoToast { mensagem, icone, desde, acao, dica, duracao }));
    ctx.request_repaint();
}

/// O toast com o sólido de concluído e uma ação no próprio toast ("Cópia salva: x.db · 119 MB" com "Mostrar na
/// pasta"); `dica` é o texto que aparece ao pôr o ponteiro na pílula (o caminho inteiro). O clique na ação sai em
/// [`acao_do_toast`], e o toast some.
pub fn toast_concluido_com_acao(ctx: &Context, mensagem: impl Into<String>, dica: Option<String>, acao: AcaoToast) {
    mostrar_com(ctx, mensagem.into(), IconeEstado::Concluido, Some(acao), dica, DURACAO_TOAST_COM_ACAO);
}

/// O [`toast_concluido_com_acao`] por outro tempo: a ação que só vale por um prazo (o "Desfazer · Ctrl+Z" da
/// gravação no banco fica 6 s, prancheta 15).
pub fn toast_concluido_com_acao_por(ctx: &Context, mensagem: impl Into<String>, acao: AcaoToast, duracao: f64) {
    mostrar_com(ctx, mensagem.into(), IconeEstado::Concluido, Some(acao), None, duracao);
}

/// A `carga` da ação do toast que a pessoa clicou desde a última chamada (e só ela: a chamada a consome). A casca
/// chama a cada quadro.
pub fn acao_do_toast(ctx: &Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(id_acao_clicada()))
}

/// Devolve uma carga tirada com [`acao_do_toast`] que não era de quem tirou (a casca devolve as dos documentos,
/// que as leem no quadro seguinte).
pub fn devolver_acao_do_toast(ctx: &Context, carga: String) {
    ctx.data_mut(|d| d.insert_temp(id_acao_clicada(), carga));
}

/// O rótulo da ação do toast visível agora (para testes).
pub fn rotulo_da_acao_do_toast(ctx: &Context) -> Option<String> {
    let agora = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<EstadoToast>(id_toast()))
        .filter(|e| agora - e.desde < e.duracao() + TRANSICAO)
        .and_then(|e| e.acao.map(|a| a.rotulo))
}

/// A dica da pílula do toast visível agora (para testes).
pub fn dica_do_toast(ctx: &Context) -> Option<String> {
    let agora = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<EstadoToast>(id_toast()))
        .filter(|e| agora - e.desde < e.duracao() + TRANSICAO)
        .and_then(|e| e.dica)
}

/// Mostra `mensagem` no toast com o sólido de informação, o "i" (troca a anterior e reinicia os 2,6s): a
/// mensagem neutra ("Nenhuma alteração para salvar.", "Escreva um comando para executar.").
pub fn toast(ctx: &Context, mensagem: impl Into<String>) {
    mostrar(ctx, mensagem.into(), IconeEstado::Informacao);
}

/// O toast com o sólido de concluído, o ✓: o que foi feito ("Linha salva.", "Copiado.", "Tabela criada.").
pub fn toast_concluido(ctx: &Context, mensagem: impl Into<String>) {
    mostrar(ctx, mensagem.into(), IconeEstado::Concluido);
}

/// O toast com o sólido de atenção: o que não foi feito ou ficou pela metade ("O banco aberto mudou…; nada foi
/// feito.").
pub fn toast_atencao(ctx: &Context, mensagem: impl Into<String>) {
    mostrar(ctx, mensagem.into(), IconeEstado::Atencao);
}

/// O toast com o sólido de erro, o ×: o que falhou (a mensagem do erro de uma tarefa).
pub fn toast_erro(ctx: &Context, mensagem: impl Into<String>) {
    mostrar(ctx, mensagem.into(), IconeEstado::Erro);
}

/// O sólido do toast visível agora (para testes).
pub fn icone_toast(ctx: &Context) -> Option<IconeEstado> {
    let agora = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<EstadoToast>(id_toast()))
        .filter(|e| agora - e.desde < e.duracao() + TRANSICAO)
        .map(|e| e.icone)
}

/// A mensagem do toast visível agora (para testes e para o leitor de tela).
pub fn toast_atual(ctx: &Context) -> Option<String> {
    let agora = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<EstadoToast>(id_toast()))
        .filter(|e| agora - e.desde < e.duracao() + TRANSICAO)
        .map(|e| e.mensagem)
}

/// Onde o toast visível agora ficou no último quadro (para testes).
pub fn rect_toast(ctx: &Context) -> Option<Rect> {
    toast_atual(ctx)?;
    ctx.data(|d| d.get_temp::<Rect>(id_toast().with("rect")))
}

/// Opacidade do toast `t` segundos depois de aparecer: entra e sai em 180ms (com movimento reduzido, aparece e
/// some de uma vez, sem a subida de 14px, que acompanha a opacidade).
pub fn opacidade_toast(t: f64, reduzido: bool) -> f32 {
    opacidade_toast_em(t, DURACAO_TOAST, reduzido)
}

/// O [`opacidade_toast`] para um toast que dura `duracao` segundos (o com ação dura mais).
fn opacidade_toast_em(t: f64, duracao: f64, reduzido: bool) -> f32 {
    let entrada = if reduzido { 1.0 } else { (t / TRANSICAO).clamp(0.0, 1.0) };
    let saida =
        if t > duracao { if reduzido { 0.0 } else { 1.0 - ((t - duracao) / TRANSICAO).clamp(0.0, 1.0) } } else { 1.0 };
    entrada.min(saida) as f32
}

/// Desenha o toast (se houver) por cima de tudo, 22px acima da borda de baixo da janela e nunca abaixo de `limite`
/// (o `y` onde a borda de baixo dele pode chegar: o editor passa o alto da barra de status menos o espaço entre os
/// painéis; sem nada embaixo, `f32::INFINITY`). Pede repintura só enquanto anima e no fim dos 2,6s.
pub fn desenhar_toast(ctx: &Context, limite: f32) {
    let Some(e) = ctx.data(|d| d.get_temp::<EstadoToast>(id_toast())) else { return };
    let agora = ctx.input(|i| i.time);
    let t = agora - e.desde;
    let duracao = e.duracao();
    if t >= duracao + TRANSICAO {
        ctx.data_mut(|d| d.remove::<EstadoToast>(id_toast()));
        return;
    }
    // o mesmo indicador das outras peças (o `prefers-reduced-motion` que o editor repassa do Windows)
    let opacidade = opacidade_toast_em(t, duracao, movimento_reduzido_ctx(ctx));
    if opacidade < 1.0 && opacidade > 0.0 {
        ctx.request_repaint();
    } else if t < duracao {
        ctx.request_repaint_after(Duration::from_secs_f64(duracao - t));
    }
    let p = paleta_ctx(ctx);
    let cor = p.estudio.t;
    let tela = ctx.content_rect();
    let desce = 14.0 * (1.0 - opacidade);
    let job = egui::text::LayoutJob::single_section(
        e.mensagem.clone(),
        crate::pecas::texto::formato(fontes::fonte(12.0, Peso::Normal), cor),
    );
    let g = ctx.fonts_mut(|f| f.layout_job(job));
    // com ação, o botão pequeno (`.btn.pq`) vai depois do texto, 10px adiante, e a pílula fecha 6px depois dele
    let largura_acao = e.acao.as_ref().map(|a| {
        let job = egui::text::LayoutJob::single_section(
            a.rotulo.clone(),
            crate::pecas::texto::formato(fontes::fonte(12.0, Peso::Medio), cor),
        );
        let gb = ctx.fonts_mut(|f| f.layout_job(job));
        (gb.size().x + 2.0 * PADDING_ACAO + 2.0).ceil()
    });
    let largura = match largura_acao {
        Some(b) => {
            (RECUO_ESQUERDA + LADO_ICONE + ESPACO_ICONE + g.size().x + ESPACO_ACAO + b + RECUO_DIREITA_ACAO).ceil()
        }
        None => (RECUO_ESQUERDA + LADO_ICONE + ESPACO_ICONE + g.size().x + RECUO_DIREITA).ceil(),
    };
    let tamanho = vec2(largura, ALTURA_TOAST);
    let baixo = tela.bottom().min(limite) - MARGEM_BAIXO;
    let rect = Rect::from_center_size(pos2(tela.center().x, baixo - tamanho.y / 2.0 + desce), tamanho);
    ctx.data_mut(|d| d.insert_temp(id_toast().with("rect"), rect));
    // só com ação ou dica a pílula recebe o ponteiro; sem elas, o clique passa para o que está embaixo
    let interativo = e.acao.is_some() || e.dica.is_some();
    Area::new(id_toast().with("area")).order(Order::Tooltip).interactable(interativo).fixed_pos(rect.min).show(
        ctx,
        |ui| {
            let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::hover());
            let mut painter = ui.painter().clone();
            painter.multiply_opacity(opacidade);
            if !p.escuro {
                // no claro, a sombra `--sh` e a borda `--vidrob` (8%) somem num cartão pequeno: mais firmes
                pintura::sombra(&painter, rect, RAIO_TOAST, SOMBRA_TOAST_CLARO);
            }
            pintar_vidro_flutuante(&painter, p, rect, RAIO_TOAST);
            if !p.escuro {
                painter.add(RectShape::stroke(
                    rect,
                    pintura::canto(rect, RAIO_TOAST),
                    Stroke::new(1.0, BORDA_TOAST_CLARO),
                    StrokeKind::Inside,
                ));
            }
            let icone = Rect::from_min_size(
                pos2(rect.left() + RECUO_ESQUERDA, rect.center().y - LADO_ICONE / 2.0),
                Vec2::splat(LADO_ICONE),
            );
            pintar_icone_estado(&painter, icone, e.icone);
            painter.galley(pos2(icone.right() + ESPACO_ICONE, rect.center().y - g.size().y / 2.0), g, cor);
            let mensagem = e.mensagem.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &mensagem));
            ui.ctx().accesskit_node_builder(resp.id, |n| {
                n.set_role(egui::accesskit::Role::Status);
                n.set_live(egui::accesskit::Live::Polite);
            });
            if let (Some(acao), Some(largura)) = (&e.acao, largura_acao) {
                let botao = Rect::from_min_size(
                    pos2(rect.right() - RECUO_DIREITA_ACAO - largura, rect.center().y - ALTURA_PEQUENO / 2.0),
                    vec2(largura, ALTURA_PEQUENO),
                );
                ui.multiply_opacity(opacidade);
                if ui.put(botao, Botao::new(&acao.rotulo).pequeno()).clicked() {
                    ctx.data_mut(|d| {
                        d.insert_temp(id_acao_clicada(), acao.carga.clone());
                        // cumprida a ação, o toast sai
                        d.remove::<EstadoToast>(id_toast());
                    });
                    ctx.request_repaint();
                }
            }
            if let Some(dica) = &e.dica {
                resp.on_hover_text(dica);
            }
        },
    );
}
