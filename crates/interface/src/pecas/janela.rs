//! A janela modal (o `.dlg` das pranchetas, `Exportar.dc.html`) e a confirmação.
//!
//! Anatomia: cabeçalho de 50px (título 14px / 600 · spacer · ✕; `padding:0 10px 0 20px`, fio `--vidrob`
//! embaixo), corpo (`padding:18px 20px 20px`, rola acima de 90% da altura da tela) e rodapé opcional de 58px
//! (destrutivas à esquerda, spacer, primária à direita; `padding:0 14px 0 16px`, `gap:8px`, fio `--vidrob` em
//! cima, botões de 30px). 500px de largura (760px na [`Janela::larga`]), raio 10 (o raio único,
//! [`crate::pintura::RAIO_PADRAO`]), no vidro das camadas flutuantes ([`crate::pecas::campos::forma_vidro_fosco`]:
//! `--flut` com o reflexo de cima, borda `--vidrob`, realce `--vidroluz` e a sombra `--sh`, num lugar reservado
//! antes do conteúdo), e o véu (`--veu`) por cima da tela atrás.
//!
//! Fechar: o ✕, o Esc e o clique no fundo **não fecham sozinhos**: viram [`RespostaJanela::pediu_fechar`] e a
//! tela decide (o "pode fechar?" da exportação em andamento). Um clique no fundo nos 400ms depois de abrir não
//! conta (o segundo clique de um duplo clique que abriu a janela). No quadro em que abre, o foco vai para o
//! primeiro campo de digitar do corpo ([`crate::pecas::campos::Entrada`], `AreaTexto`, `Select`); sem campo,
//! para o botão primário do rodapé.
//!
//! ```ignore
//! if let Some(j) = &mut self.janela_nova {
//!     let r = Janela::new("nova", "Nova pessoa")
//!         .rodape_direita(|ui| { if ui.add(Botao::primario("Criar")).clicked() { criar = true; } })
//!         .mostrar(ctx, |ui| { campo(ui, "Nome", None, |ui| ui.add(Entrada::new(&mut j.nome))); });
//!     if r.pediu_fechar { self.janela_nova = None; }
//! }
//! ```

use egui::{Align, Context, Id, Key, Layout, Modal, Modifiers, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, vec2};

use crate::icones::Icone;
use crate::pecas::arranjo::linha_com_direita;
use crate::pecas::botao::{ALTURA_JANELA, Botao, TipoBotao, com_altura};
use crate::pecas::campos::{foco_inicial_pendente, forma_vidro_fosco, pedir_foco_inicial};
use crate::pecas::cartao::RAIO_CARD;
use crate::pecas::texto::{Trecho, paragrafo_rico};
use crate::tema::paleta_ctx;

/// Largura da janela (`.dlg{width:500px}`) e a da larga.
pub const LARGURA_JANELA: f32 = 500.0;
pub const LARGURA_JANELA_LARGA: f32 = 760.0;
/// Altura do cabeçalho (`.dlg header{height:50px}`, com o fio de baixo).
pub const ALTURA_CABECALHO: f32 = 50.0;
/// Altura do rodapé (`.dlg footer{height:58px}`, com o fio de cima).
pub const ALTURA_RODAPE: f32 = 58.0;

type Conteudo<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// O que a janela devolve a cada quadro.
#[derive(Debug, Clone)]
pub struct RespostaJanela<R> {
    /// O que o corpo devolveu.
    pub inner: R,
    /// A pessoa pediu para fechar (✕, Esc ou clique no fundo). A tela decide se fecha.
    pub pediu_fechar: bool,
    /// Enter num campo de uma linha do corpo ([`crate::pecas::campos::Entrada`], sem Shift/Ctrl/Alt): o submit
    /// implícito do `<form>` da janela web (o `aoEnviar`: "Abrir" no Pedir o caminho, "Renomear", "Salvar" /
    /// "Incluir linha" na janela da linha). Enter numa área de texto, num select ou escolhendo uma sugestão não
    /// conta. A tela faz o mesmo que o botão primário.
    pub enviou: bool,
    /// O quadro em que a janela abriu (o foco inicial foi pedido nele).
    pub abriu_agora: bool,
    /// Onde a janela ficou.
    pub rect: Rect,
}

fn id_envio() -> Id {
    Id::new("databox::janela::enviou")
}

/// Marca o envio do formulário da janela que está desenhando o corpo (chamado pela [`crate::pecas::campos::Entrada`]
/// quando o Enter sai do campo).
pub(crate) fn marcar_envio(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(id_envio(), true));
}

fn tomar_envio(ctx: &Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(id_envio())).unwrap_or(false)
}

#[derive(Clone, Copy)]
struct EstadoJanela {
    aberta_em: f64,
    ultima_passada: u64,
}

#[must_use = "chame .mostrar(ctx, |ui| …)"]
pub struct Janela<'a> {
    id: Id,
    titulo: &'a str,
    larga: bool,
    largura: Option<f32>,
    icone: Option<Icone>,
    logo: bool,
    sub: Option<&'a str>,
    dica_fechar: Option<&'a str>,
    corpo_cru: bool,
    esquerda: Option<Conteudo<'a>>,
    direita: Option<Conteudo<'a>>,
}

/// O ícone do cabeçalho (`header .sk.s20`) e a margem dele até o título (`h2{margin-left:10px}`).
pub const LADO_ICONE_JANELA: f32 = 20.0;
const ESPACO_ICONE_JANELA: f32 = 10.0;
/// O espaço entre o título e o sub (`margin-left:8px`).
const ESPACO_SUB: f32 = 8.0;

impl<'a> Janela<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, titulo: &'a str) -> Self {
        Janela {
            id: Id::new(("databox::janela", id_salt)),
            titulo,
            larga: false,
            largura: None,
            icone: None,
            logo: false,
            sub: None,
            dica_fechar: None,
            corpo_cru: false,
            esquerda: None,
            direita: None,
        }
    }

    /// A dica do × do cabeçalho no lugar de "Fechar (Esc)" (o `title` do × das Preferências diz que o fechar volta
    /// ao que estava quando a janela abriu).
    pub fn dica_fechar(mut self, dica: &'a str) -> Self {
        self.dica_fechar = Some(dica);
        self
    }

    /// O corpo sem o `padding:18px 20px 20px` e sem o espaço entre os blocos: a tela desenha as bordas dela (a grade
    /// 184 / 1fr das Preferências, com a navegação encostada na borda da janela). A rolagem continua.
    pub fn corpo_cru(mut self, sim: bool) -> Self {
        self.corpo_cru = sim;
        self
    }

    /// A janela larga: 760px.
    pub fn larga(mut self, sim: bool) -> Self {
        self.larga = sim;
        self
    }

    /// Outra largura (440 no Sobre); vale no lugar da [`Janela::larga`].
    pub fn largura(mut self, largura: f32) -> Self {
        self.largura = Some(largura);
        self
    }

    /// O sólido de 20px antes do título, com 10px até ele (a ficha de "Linha 12", o cilindro de "Exportar o banco").
    pub fn icone(mut self, icone: Icone) -> Self {
        self.icone = Some(icone);
        self
    }

    /// O logo completo de 20px no lugar do ícone ([`crate::marca::pintar_logo`]: o "Sobre o DataBox").
    pub fn logo(mut self, sim: bool) -> Self {
        self.logo = sim;
        self
    }

    /// O texto em `--t3` depois do título, a 8px dele ("clientes" em "Alterar a coluna cidade").
    #[allow(clippy::should_implement_trait)] // o nome é o do sub da aba e do cabeçalho das pranchetas
    pub fn sub(mut self, sub: &'a str) -> Self {
        self.sub = Some(sub);
        self
    }

    /// Ações destrutivas, à esquerda do rodapé ([`Botao::ghost_perigo`] "Excluir linha", [`Botao::perigo`]
    /// "Interromper").
    pub fn rodape_esquerda(mut self, conteudo: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.esquerda = Some(Box::new(conteudo));
        self
    }

    /// Ações à direita do rodapé (a primária por último).
    pub fn rodape_direita(mut self, conteudo: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.direita = Some(Box::new(conteudo));
        self
    }

    pub fn mostrar<R>(self, ctx: &Context, corpo: impl FnOnce(&mut Ui) -> R) -> RespostaJanela<R> {
        let p = paleta_ctx(ctx);
        let agora = ctx.input(|i| i.time);
        let passada = ctx.cumulative_pass_nr();
        let anterior: Option<EstadoJanela> = ctx.data(|d| d.get_temp(self.id));
        let abriu_agora = anterior.is_none_or(|e| e.ultima_passada + 1 < passada);
        let aberta_em = match anterior {
            Some(e) if !abriu_agora => e.aberta_em,
            _ => agora,
        };
        ctx.data_mut(|d| d.insert_temp(self.id, EstadoJanela { aberta_em, ultima_passada: passada }));
        // o foco que a janela dá ao abrir não mostra o anel (nem no ✕): o anel só vem depois de uma tecla apertada
        // com a janela já aberta (o `:focus-visible` do navegador nem conta a tecla que abriu a janela)
        if agora - aberta_em < 1e-6 {
            crate::pintura::esquecer_teclado(ctx);
        }

        let tela = ctx.content_rect();
        let largura = self
            .largura
            .unwrap_or(if self.larga { LARGURA_JANELA_LARGA } else { LARGURA_JANELA })
            .min(tela.width() * 0.94);
        let altura_max = tela.height() * 0.9;
        let mut fechar_x = false;
        let mut enviou = false;
        let tem_rodape = self.esquerda.is_some() || self.direita.is_some();
        let titulo = self.titulo;
        let (icone, sub, logo) = (self.icone, self.sub, self.logo);
        let (dica_fechar, corpo_cru) = (self.dica_fechar, self.corpo_cru);
        let id = self.id;

        // o véu (`--veu`) cobre a tela atrás da janela
        let modal =
            Modal::new(id.with("modal")).backdrop_color(p.estudio.veu).frame(egui::Frame::NONE).show(ctx, |ui| {
                ui.set_width(largura);
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                let e = &p.estudio;
                let externo = ui.cursor().min;
                let lugar_vidro = ui.painter().add(egui::Shape::Noop);
                // o conteúdo fica dentro da borda de 1px
                let topo = externo + vec2(1.0, 1.0);
                let largura_total = largura;
                let largura = largura_total - 2.0;
                let fio = Stroke::new(1.0, e.vidrob);

                // cabeçalho: `padding:0 10px 0 20px`, 49px e o fio de baixo
                let altura_cab = ALTURA_CABECALHO - 1.0;
                let cab = Rect::from_min_size(topo + vec2(20.0, 0.0), vec2(largura - 20.0 - 10.0, altura_cab));
                let mut ui_cab = ui.new_child(
                    UiBuilder::new().id_salt(id.with("cab")).max_rect(cab).layout(Layout::top_down(Align::Min)),
                );
                linha_com_direita(
                    &mut ui_cab,
                    id.with("cab_linha"),
                    altura_cab,
                    12.0,
                    |ui| {
                        if icone.is_some() || logo {
                            let (r, _) =
                                ui.allocate_exact_size(vec2(LADO_ICONE_JANELA, LADO_ICONE_JANELA), Sense::hover());
                            match icone {
                                Some(icone) => crate::icones::pintar(ui.painter(), r, icone, e.t2),
                                None => crate::marca::pintar_logo(ui, r),
                            }
                            ui.add_space(ESPACO_ICONE_JANELA - ui.spacing().item_spacing.x);
                        }
                        // `.dlg h2`: 14px / 600
                        let r = crate::pecas::texto::h3(ui, titulo);
                        ui.ctx().accesskit_node_builder(r.id, |n| n.set_role(egui::accesskit::Role::Heading));
                        if let Some(sub) = sub {
                            ui.add_space(ESPACO_SUB - ui.spacing().item_spacing.x);
                            crate::pecas::texto::texto(ui, sub, 12.0, e.t3);
                        }
                    },
                    |ui| {
                        let fechar = match dica_fechar {
                            Some(d) => Botao::fechar().dica(d),
                            None => Botao::fechar(),
                        };
                        if ui.add(fechar).clicked() {
                            fechar_x = true;
                        }
                    },
                );
                let fim_cab = topo.y + ALTURA_CABECALHO;
                ui.painter().hline(topo.x..=topo.x + largura, fim_cab - 0.5, fio);

                // corpo: `padding:18px 20px 20px`
                if abriu_agora {
                    pedir_foco_inicial(ui, true);
                }
                let altura_rodape = if tem_rodape { ALTURA_RODAPE } else { 0.0 };
                let max_corpo = (altura_max - 2.0 - ALTURA_CABECALHO - altura_rodape).max(80.0);
                let rect_corpo = Rect::from_min_size(egui::pos2(topo.x, fim_cab), vec2(largura, max_corpo));
                let mut ui_corpo = ui.new_child(
                    UiBuilder::new()
                        .id_salt(id.with("corpo"))
                        .max_rect(rect_corpo)
                        .layout(Layout::top_down(Align::Min)),
                );
                // só o Enter dos campos deste corpo conta como envio
                tomar_envio(ctx);
                let saida = ScrollArea::vertical()
                    .id_salt(id.with("rolagem"))
                    .max_height(max_corpo)
                    .auto_shrink([false, true])
                    .show(&mut ui_corpo, |ui| {
                        let margem = if corpo_cru {
                            egui::Margin::ZERO
                        } else {
                            egui::Margin { left: 20, right: 20, top: 18, bottom: 20 }
                        };
                        egui::Frame::NONE
                            .inner_margin(margem)
                            .show(ui, |ui| {
                                ui.set_width(largura - f32::from(margem.left + margem.right));
                                ui.spacing_mut().item_spacing =
                                    if corpo_cru { vec2(0.0, 0.0) } else { vec2(8.0, 12.0) };
                                // o aviso no alto do corpo ganha 14px embaixo
                                crate::pecas::avisos::marcar_corpo(ui);
                                corpo(ui)
                            })
                            .inner
                    });
                enviou = tomar_envio(ctx);
                let inner = saida.inner;
                let fim_corpo = fim_cab + saida.content_size.y.min(max_corpo).max(0.0);
                let mut fim = fim_corpo;

                // rodapé: o fio de cima, 57px, `padding:0 14px 0 16px`, `gap:8px` e os botões de 30px
                if tem_rodape {
                    let foco_no_rodape = foco_inicial_pendente(ui);
                    pedir_foco_inicial(ui, false);
                    definir_foco_botao(ui, foco_no_rodape);
                    ui.painter().hline(topo.x..=topo.x + largura, fim_corpo + 0.5, fio);
                    let altura_linha = ALTURA_RODAPE - 1.0;
                    let rect_rod = Rect::from_min_size(
                        egui::pos2(topo.x + 16.0, fim_corpo + 1.0),
                        vec2(largura - 16.0 - 14.0, altura_linha),
                    );
                    let mut ui_rod = ui.new_child(
                        UiBuilder::new()
                            .id_salt(id.with("rodape"))
                            .max_rect(rect_rod)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    let esquerda = self.esquerda;
                    let direita = self.direita;
                    com_altura(&mut ui_rod, ALTURA_JANELA, |ui_rod| {
                        linha_com_direita(
                            ui_rod,
                            id.with("rod_linha"),
                            altura_linha,
                            8.0,
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                if let Some(e) = esquerda {
                                    e(ui);
                                }
                            },
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                if let Some(d) = direita {
                                    d(ui);
                                }
                            },
                        );
                    });
                    definir_foco_botao(ui, false);
                    fim = fim_corpo + ALTURA_RODAPE;
                } else {
                    pedir_foco_inicial(ui, false);
                }

                let rect = Rect::from_min_max(externo, egui::pos2(externo.x + largura_total, fim + 1.0));
                ui.allocate_rect(rect, Sense::hover());
                // o vidro das camadas flutuantes no lugar reservado antes do conteúdo: fica embaixo dele
                ui.painter().set(lugar_vidro, forma_vidro_fosco(ui.painter(), p, rect, RAIO_CARD));
                (inner, rect)
            });

        let (inner, rect) = modal.inner;
        let clique_fundo = modal.backdrop_response.clicked() && agora - aberta_em > 0.4;
        let esc = modal.is_top_modal
            && !modal.any_popup_open
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        ctx.accesskit_node_builder(modal.response.id, |n| {
            n.set_role(egui::accesskit::Role::Dialog);
            n.set_modal();
            n.set_label(titulo);
        });
        RespostaJanela { inner, pediu_fechar: fechar_x || clique_fundo || esc, enviou, abriu_agora, rect }
    }
}

fn id_foco_botao() -> Id {
    Id::new("databox::janela::foco_botao")
}

fn definir_foco_botao(ui: &Ui, sim: bool) {
    ui.ctx().data_mut(|d| d.insert_temp(id_foco_botao(), sim));
}

/// O primeiro botão primário do rodapé deve pegar o foco inicial? (usado pelo [`Botao`]).
pub(crate) fn tomar_foco_botao(ui: &Ui, tipo: TipoBotao) -> bool {
    matches!(tipo, TipoBotao::Primario | TipoBotao::Ouro) && tomar_foco_rodape(ui)
}

/// O foco inicial da janela ainda está à espera de um botão do rodapé? Se sim, passa a ser de quem pergunta.
pub fn tomar_foco_rodape(ui: &Ui) -> bool {
    let pendente = ui.ctx().data(|d| d.get_temp::<bool>(id_foco_botao())).unwrap_or(false);
    if pendente {
        definir_foco_botao(ui, false);
    }
    pendente
}

// ------------------------------------------------------------------ confirmar

/// Uma pergunta antes de uma ação com consequência (a [`Janela`] de confirmação).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmacao {
    /// Título curto ("Excluir linhas"). O padrão do JavaScript é "Confirmar".
    pub titulo: String,
    /// A frase com a consequência.
    pub frase: String,
    /// O texto do botão da ação ("Excluir", "Substituir"). O padrão é "Continuar".
    pub rotulo: String,
    /// Ação destrutiva: o botão é o `.btn.risco` (vermelho); senão o `.btn.pri`.
    pub perigo: bool,
    /// A segunda saída, à esquerda do rodapé em [`Botao::ghost_perigo`] ("Não salvar" do "Salvar as alterações em
    /// vendas_2025.csv?", prancheta 15): descarta e segue, sem fazer a ação.
    pub descartar: Option<String>,
}

/// O que a pessoa decidiu numa confirmação ([`confirmar_decisao`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decisao {
    /// O botão da ação.
    Confirmou,
    /// A segunda saída ([`Confirmacao::descartar`]).
    Descartou,
    /// "Cancelar", ✕, Esc ou o clique no fundo: nada acontece.
    Cancelou,
}

impl Confirmacao {
    /// Com os padrões: título "Confirmar", botão "Continuar", sem perigo.
    pub fn new(frase: impl Into<String>) -> Self {
        Confirmacao {
            titulo: "Confirmar".into(),
            frase: frase.into(),
            rotulo: "Continuar".into(),
            perigo: false,
            descartar: None,
        }
    }

    /// A segunda saída à esquerda do rodapé ("Não salvar"), em vermelho de texto.
    pub fn descartar(mut self, rotulo: impl Into<String>) -> Self {
        self.descartar = Some(rotulo.into());
        self
    }

    pub fn titulo(mut self, titulo: impl Into<String>) -> Self {
        self.titulo = titulo.into();
        self
    }

    pub fn rotulo(mut self, rotulo: impl Into<String>) -> Self {
        self.rotulo = rotulo.into();
        self
    }

    pub fn perigo(mut self, sim: bool) -> Self {
        self.perigo = sim;
        self
    }
}

/// Mostra a confirmação guardada em `estado` (se houver). Devolve `Some(true)` pelo botão da ação,
/// `Some(false)` quando a pessoa desiste ("Cancelar", ✕, Esc, clique no fundo); nos dois casos `estado` volta a
/// `None`. Enquanto a janela está aberta devolve `None`. O foco nasce no botão da ação. Com a segunda saída
/// ([`Confirmacao::descartar`]) ela também vale `Some(false)`; quem a usa chama [`confirmar_decisao`].
pub fn confirmar(ctx: &Context, estado: &mut Option<Confirmacao>) -> Option<bool> {
    confirmar_decisao(ctx, estado).map(|d| d == Decisao::Confirmou)
}

/// [`confirmar`] que diz qual das três saídas a pessoa escolheu: a ação, a segunda saída ([`Confirmacao::descartar`],
/// "Não salvar") ou desistir ("Cancelar", ✕, Esc, clique no fundo).
pub fn confirmar_decisao(ctx: &Context, estado: &mut Option<Confirmacao>) -> Option<Decisao> {
    let c = estado.as_ref()?;
    let (mut aceitou, mut descartou, mut cancelou) = (false, false, false);
    let mut janela = Janela::new(("confirmar", &c.titulo), &c.titulo);
    if let Some(rotulo) = &c.descartar {
        janela = janela.rodape_esquerda(|ui| {
            if ui.add(Botao::ghost_perigo(rotulo)).clicked() {
                descartou = true;
            }
        });
    }
    let r = janela
        .rodape_direita(|ui| {
            // "Cancelar" antes da ação, com o mesmo efeito do ✕
            if ui.add(Botao::ghost("Cancelar")).clicked() {
                cancelou = true;
            }
            let tipo = if c.perigo { TipoBotao::Perigo } else { TipoBotao::Primario };
            let resp = ui.add(Botao::new(&c.rotulo).tipo(tipo));
            // o foco nasce no botão da ação (também quando ele é o vermelho)
            if tomar_foco_rodape(ui) {
                resp.request_focus();
            }
            if resp.clicked() {
                aceitou = true;
            }
        })
        .mostrar(ctx, |ui| {
            // a frase: 13px / 1.5 em `--t` (o parágrafo do Início), até 62ch
            let e = &crate::tema::paleta(ui).estudio;
            let largura = ui.available_width().min(62.0 * 7.2);
            let g = paragrafo_rico(ui, &[Trecho::Normal(&c.frase)], 13.0, e.t, e.t, largura, 1.5);
            crate::pecas::texto::mostrar_paragrafo(ui, &g, &c.frase);
        });
    let resultado = if aceitou {
        Some(Decisao::Confirmou)
    } else if descartou {
        Some(Decisao::Descartou)
    } else if cancelou || r.pediu_fechar {
        Some(Decisao::Cancelou)
    } else {
        None
    };
    if resultado.is_some() {
        *estado = None;
    }
    resultado
}
