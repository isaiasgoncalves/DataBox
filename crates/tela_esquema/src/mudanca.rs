//! A janela de mudança de estrutura: o "SQL que será executado" com a
//! prévia calculada pelo serviço 300 ms depois de cada mudança (`servico.esquema(…, previa: true)` numa tarefa
//! de fundo; a resposta de uma prévia que já foi trocada por outra é descartada), o aviso de erro no alto da
//! janela e o "Aplicar" (`previa: false`).
//!
//! Cada janela (coluna, renomear, novo índice, nova tabela) guarda uma [`Mudanca`] e, a cada quadro:
//! 1. chama [`Mudanca::passo`] com a montagem do pedido (só usada quando a prévia vence) e recebe a resposta
//!    do "Aplicar" quando ele termina bem (o toast já foi dado);
//! 2. desenha o corpo: [`Mudanca::desenhar_aviso`], os campos e [`Mudanca::desenhar_previa`];
//! 3. com uma mudança nos campos, [`Mudanca::agendar`]; com o botão primário (ou o Enter num campo),
//!    [`Mudanca::aplicar`].

use egui::{Id, Ui};
use estado::{Contexto, Promessa, Reacao};
use interface::pecas::avisos::{Aviso, TomAviso, aviso_texto};
use interface::pecas::texto::{Trecho, texto_rico};
use interface::pecas::texto_pre::TextoPre;
use servico::{OperacaoEsquema, RespostaEsquema};

use crate::apoio::{ESPACO_ENTRE_BLOCOS, MSG_ESTRUTURA_ALTERADA, titulo_previa};

/// Espera entre a última mudança nos campos e a prévia (o `debounce(prever, 300)`).
pub(crate) const ESPERA_PREVIA: f64 = 0.3;
/// A altura máxima da prévia do SQL quando o aviso comprido da reconstrução vem embaixo dela.
const ALTURA_PREVIA_COM_AVISO: f32 = 180.0;

/// O que a janela montou a partir dos campos.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Montagem {
    /// Ainda falta o essencial (o `null`): a prévia mostra a frase do que falta.
    Falta,
    /// O pedido pronto.
    Pedido(Box<OperacaoEsquema>),
    /// Uma frase para a pessoa (o `throw new Error(…)`).
    Erro(String),
}

impl Montagem {
    /// O pedido pronto.
    pub(crate) fn pedido(op: OperacaoEsquema) -> Montagem {
        Montagem::Pedido(Box::new(op))
    }
}

/// O que fica abaixo da prévia (o `.slot-previa`).
#[derive(Debug, Clone, PartialEq)]
enum Slot {
    Vazio,
    /// A descrição e os avisos do plano, um por linha.
    Plano(Vec<String>),
    Erro(String),
}

/// O estado da prévia e do "Aplicar" de uma janela de mudança de estrutura.
pub(crate) struct Mudanca {
    id: Id,
    faltando: &'static str,
    /// O texto do `pre.previa-sql`.
    pub(crate) texto: String,
    /// `.desatualizada`: uma prévia nova está a caminho (60% de opacidade).
    pub(crate) desatualizada: bool,
    slot: Slot,
    previa: Option<Promessa<RespostaEsquema>>,
    agendada: Option<f64>,
    prever_ja: bool,
    aplicando: Option<Promessa<RespostaEsquema>>,
    /// O aviso no alto da janela (`j.aviso`): texto e se é de perigo.
    pub(crate) aviso: Option<(String, bool)>,
    /// O aviso acabou de mudar: no próximo desenho, o corpo da janela rola até ele (o
    /// `avisoEl.scrollIntoView({block:"nearest"})`; quem rolou o corpo até a prévia não fica sem retorno).
    rolar_aviso: bool,
    /// A frase de erro que o alto da janela já mostra (depois de um "Aplicar" recusado): a prévia não a repete.
    erro_na_janela: Option<String>,
}

impl Mudanca {
    /// Uma janela nova: a prévia é pedida no primeiro quadro (o `j.prever()` do fim de `janelaMudanca`).
    pub(crate) fn nova(id: Id, faltando: &'static str) -> Mudanca {
        Mudanca {
            id,
            faltando,
            texto: format!("-- {faltando}"),
            desatualizada: false,
            slot: Slot::Vazio,
            previa: None,
            agendada: None,
            prever_ja: true,
            aplicando: None,
            aviso: None,
            rolar_aviso: false,
            erro_na_janela: None,
        }
    }

    /// Um "Aplicar" está a caminho (o `j.ocupado(true)`).
    pub(crate) fn ocupada(&self) -> bool {
        self.aplicando.is_some()
    }

    /// Agenda a prévia para daqui a 300 ms (cada mudança adia a anterior).
    pub(crate) fn agendar(&mut self, ctx: &egui::Context) {
        let agora = ctx.input(|i| i.time);
        self.agendada = Some(agora + ESPERA_PREVIA);
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(ESPERA_PREVIA));
    }

    /// Pede a prévia no próximo quadro, sem esperar (o `j.prever()`).
    pub(crate) fn prever_ja(&mut self, ctx: &egui::Context) {
        self.agendada = None;
        self.prever_ja = true;
        ctx.request_repaint();
    }

    /// Mostra um aviso no alto da janela (o `j.aviso`); o corpo rola até ele no próximo desenho.
    pub(crate) fn avisar(&mut self, mensagem: impl Into<String>, perigo: bool) {
        self.aviso = Some((mensagem.into(), perigo));
        self.rolar_aviso = true;
    }

    fn mostrar_faltando(&mut self) {
        self.desatualizada = false;
        self.texto = format!("-- {}", self.faltando);
        self.slot = Slot::Vazio;
    }

    fn erro_na_previa(&mut self, mensagem: String) {
        self.slot =
            if self.erro_na_janela.as_deref() == Some(mensagem.as_str()) { Slot::Vazio } else { Slot::Erro(mensagem) };
    }

    /// Erro de um "Aplicar": no alto da janela, uma vez só (a prévia, recalculada, não repete a frase).
    fn erro_ao_aplicar(&mut self, ctx: &egui::Context, mensagem: String) {
        self.erro_na_janela = Some(mensagem.clone());
        self.avisar(mensagem, true);
        self.prever_ja(ctx);
    }

    fn prever(&mut self, egui: &egui::Context, ctx: &Contexto<'_>, montagem: Montagem) {
        match montagem {
            Montagem::Erro(msg) => {
                self.previa = None;
                self.mostrar_faltando();
                self.erro_na_previa(msg);
            }
            Montagem::Falta => {
                self.previa = None;
                self.mostrar_faltando();
            }
            Montagem::Pedido(op) => {
                self.desatualizada = true;
                // a prévia anterior, se ainda estiver a caminho, é descartada com a promessa dela
                self.previa = Some(ctx.tarefa(egui, move |servico, id| servico.esquema(id, &op, true)));
            }
        }
    }

    /// O trabalho de cada quadro: recebe a prévia e o "Aplicar" que chegaram e dispara a prévia vencida.
    /// `montar` só é chamado quando a prévia vai ser pedida. Devolve a resposta do "Aplicar" que deu certo (o
    /// toast com a descrição já foi dado; a janela fecha e faz o `aoConcluir`).
    pub(crate) fn passo(
        &mut self,
        egui: &egui::Context,
        ctx: &mut Contexto<'_>,
        montar: impl FnOnce() -> Montagem,
    ) -> Option<RespostaEsquema> {
        if let Some(r) = self.previa.as_mut().and_then(Promessa::tomar) {
            self.previa = None;
            self.desatualizada = false;
            match r {
                Ok(r) => {
                    // um comando por bloco, sem a linha em branco entre eles (o `Plano::sql()` junta com ";\n\n")
                    self.texto = if r.sql.is_empty() {
                        "-- Nada muda: nenhum comando a executar.".to_string()
                    } else {
                        r.sql.replace(";\n\n", ";\n")
                    };
                    let mut linhas = Vec::with_capacity(1 + r.avisos.len());
                    linhas.push(r.descricao);
                    linhas.extend(r.avisos);
                    self.slot = Slot::Plano(linhas);
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        self.texto = "-- A prévia não pôde ser calculada.".to_string();
                        self.erro_na_previa(e.mensagem);
                    }
                }
            }
        }

        let mut concluida = None;
        if let Some(r) = self.aplicando.as_mut().and_then(Promessa::tomar) {
            self.aplicando = None;
            match r {
                Ok(r) => {
                    let frase = if r.descricao.is_empty() { MSG_ESTRUTURA_ALTERADA } else { r.descricao.as_str() };
                    let msg = ctx.aviso_de_gravacao(r.transacao_aberta, frase);
                    interface::pecas::toast::toast_concluido(egui, msg);
                    concluida = Some(r);
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        self.erro_ao_aplicar(egui, e.mensagem);
                    }
                }
            }
        }
        if concluida.is_some() {
            return concluida;
        }

        let agora = egui.input(|i| i.time);
        let vencida = self.agendada.is_some_and(|t| agora >= t);
        if self.prever_ja || vencida {
            self.prever_ja = false;
            self.agendada = None;
            self.prever(egui, ctx, montar());
        } else if let Some(t) = self.agendada {
            egui.request_repaint_after(std::time::Duration::from_secs_f64((t - agora).max(0.0)));
        }
        None
    }

    /// O "Aplicar": cancela a prévia agendada e a que está a caminho; com o pedido
    /// pronto, manda `previa: false`.
    pub(crate) fn aplicar(&mut self, egui: &egui::Context, ctx: &Contexto<'_>, montagem: Montagem) {
        if self.ocupada() {
            return;
        }
        self.agendada = None;
        self.previa = None;
        self.desatualizada = false;
        match montagem {
            Montagem::Erro(msg) => self.erro_ao_aplicar(egui, msg),
            Montagem::Falta => {
                self.avisar(self.faltando, true);
                egui.request_repaint();
            }
            Montagem::Pedido(op) => {
                self.aviso = None;
                self.erro_na_janela = None;
                self.aplicando = Some(ctx.tarefa(egui, move |servico, id| servico.esquema(id, &op, false)));
            }
        }
    }

    /// O aviso no alto do corpo da janela (o `j.aviso`), quando há. Novo, o corpo rola o mínimo para ele aparecer
    /// (com o corpo rolado até a prévia, o erro de um "Aplicar" ficaria fora da vista).
    pub(crate) fn desenhar_aviso(&mut self, ui: &mut Ui) {
        if let Some((msg, perigo)) = &self.aviso {
            let r = aviso_texto(ui, msg, *perigo);
            if std::mem::take(&mut self.rolar_aviso) {
                ui.scroll_to_rect(r.rect, None);
            }
        }
    }

    /// O `h4` "SQL que será executado", o `pre.previa-sql` e o aviso com a descrição e os avisos do plano (ou
    /// o erro da prévia). A descrição sozinha vai no aviso de informação; com os avisos do plano (a tabela
    /// reconstruída, o que muda junto), no âmbar.
    pub(crate) fn desenhar_previa(&self, ui: &mut Ui) {
        titulo_previa(ui, "SQL que será executado");
        ui.add_space(8.0);
        // com o aviso comprido da reconstrução embaixo, a prévia fica menor para o aviso inteiro caber na janela sem
        // corte (a 900px de altura)
        let com_aviso_longo = matches!(&self.slot, Slot::Plano(linhas) if linhas.len() > 1);
        TextoPre::sql(&self.texto)
            .altura_max(if com_aviso_longo { ALTURA_PREVIA_COM_AVISO } else { 260.0 })
            .apagado(self.desatualizada)
            .id_salt(self.id.with("previa"))
            .mostrar(ui);
        match &self.slot {
            Slot::Vazio => {}
            Slot::Plano(linhas) => {
                ui.add_space(ESPACO_ENTRE_BLOCOS);
                let tom = if linhas.len() > 1 { TomAviso::Atencao } else { TomAviso::Informacao };
                // a largura do corpo e o sólido no alto, mesmo com vários parágrafos
                Aviso::com_tom(tom).cheia(true).icone_no_topo(linhas.len() > 1).mostrar(ui, |ui, cores| {
                    // `.slot-previa .notice > div + div{margin-top:4px}`
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for l in linhas {
                        texto_rico(ui, &[Trecho::Normal(l)], 12.5, cores.texto, cores.forte);
                    }
                });
            }
            Slot::Erro(msg) => {
                ui.add_space(ESPACO_ENTRE_BLOCOS);
                aviso_texto(ui, msg, true);
            }
        }
    }

    /// O texto do aviso abaixo da prévia (para os testes).
    #[cfg(test)]
    pub(crate) fn texto_do_slot(&self) -> Option<(String, bool)> {
        match &self.slot {
            Slot::Vazio => None,
            Slot::Plano(l) => Some((l.join("\n"), false)),
            Slot::Erro(m) => Some((m.clone(), true)),
        }
    }

    /// A prévia está a caminho ou agendada (para os testes).
    #[cfg(test)]
    pub(crate) fn previa_pendente(&self) -> bool {
        self.previa.is_some() || self.agendada.is_some() || self.prever_ja || self.desatualizada
    }
}
