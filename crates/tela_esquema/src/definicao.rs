//! Aba Definição SQL: a faixa "Comando de criação" com o
//! "Copiar" na ponta, e o CREATE TABLE (ou CREATE VIEW) seguido dos CREATE INDEX e CREATE TRIGGER da tabela, nas cores
//! do código, preenchendo o documento (a rolagem é do próprio texto). Na barra, o Excluir da visão ("Excluir a visão
//! v", com confirmação); nas Propriedades, os comandos prontos para a Consulta SQL. A busca da barra fica desligada.

use banco::TipoObjeto;
use egui::{Align, Layout, Margin, Ui};
use estado::{Acao, Contexto, Oferta, Ofertas, Promessa, Reacao, Rota, TabelaAberta, Verbo};
use interface::pecas::avisos::{aviso_texto, vazio};
use interface::pecas::botao::Botao;
use interface::pecas::janela::{Confirmacao, confirmar};
use interface::pecas::texto_pre::TextoPre;
use servico::{OperacaoEsquema, RespostaEsquema};

use crate::apoio::{assinatura, script_definicao, titulo_da_faixa};
use crate::estrutura::toast_da_resposta;

/// O recuo do texto em volta do documento (o `padding` da faixa: 14px).
const RECUO_TEXTO: i8 = 14;
/// A altura mínima do texto quando o documento é baixo (a janela estreita e baixa).
const ALTURA_MINIMA: f32 = 120.0;

/// A aba Definição SQL de uma tabela ou visão.
pub struct AbaDefinicao {
    /// O script mostrado (refeito só quando a tabela muda).
    script: String,
    assinatura: u64,
    confirmacao: Option<Confirmacao>,
    tarefa: Option<Promessa<RespostaEsquema>>,
    erro: Option<String>,
}

fn assinatura_da(tabela: &TabelaAberta) -> u64 {
    let info = tabela.info();
    assinatura(
        std::iter::once((info.tipo.como_str(), info.sql.as_deref()))
            .chain(info.indices.iter().map(|i| ("i", i.sql.as_deref())))
            .chain(info.gatilhos.iter().map(|g| ("g", g.sql.as_deref()))),
    )
}

impl AbaDefinicao {
    pub fn nova(tabela: &TabelaAberta) -> AbaDefinicao {
        AbaDefinicao {
            script: script_definicao(tabela.info()),
            assinatura: assinatura_da(tabela),
            confirmacao: None,
            tarefa: None,
            erro: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        estado::desenhar_parte_empilhada(self, ui, ctx, tabela);
    }

    /// Refaz o script quando a tabela mudou.
    fn atualizar(&mut self, tabela: &TabelaAberta) {
        let a = assinatura_da(tabela);
        if a != self.assinatura {
            self.script = script_definicao(tabela.info());
            self.assinatura = a;
        }
    }

    /// O conteúdo da parte (o `ParteTabela::conteudo`): o texto no lugar dela no documento.
    fn conteudo_da_parte(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let egui = ui.ctx().clone();
        if let Some(r) = self.tarefa.as_mut().and_then(Promessa::tomar) {
            self.tarefa = None;
            match r {
                Ok(r) => {
                    toast_da_resposta(&egui, ctx, &r);
                    ctx.emitir(Acao::RecarregarObjetos);
                    ctx.navegar(Rota::Banco);
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        self.erro = Some(e.mensagem);
                    }
                }
            }
        }
        self.atualizar(tabela);

        let script = self.script.as_str();
        let erro = self.erro.as_deref();
        let id = ("definicao", &tabela.nome);
        egui::Frame::NONE.inner_margin(Margin::same(RECUO_TEXTO)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(msg) = erro {
                aviso_texto(ui, msg, true);
                ui.add_space(f32::from(RECUO_TEXTO));
            }
            if script.is_empty() {
                vazio(ui, "O SQLite não guarda o comando de criação deste objeto (é uma tabela interna).", true);
            } else {
                // o texto preenche o que sobra do documento e rola por dentro
                let altura = ui.available_height().max(ALTURA_MINIMA);
                TextoPre::sql(script).altura_max(altura).id_salt(id).mostrar(ui);
            }
        });

        if let Some(true) = confirmar(&egui, &mut self.confirmacao)
            && self.tarefa.is_none()
        {
            let op = OperacaoEsquema::ExcluirVisao { nome: tabela.nome.clone() };
            self.tarefa = Some(ctx.tarefa(&egui, move |servico, id| servico.esquema(id, &op, false)));
        }
    }

    /// O Excluir da barra numa visão: a confirmação com a frase de sempre.
    fn pedir_excluir_visao(&mut self, tabela: &TabelaAberta) {
        let frase = format!(
            "Excluir a visão {}? Os dados das tabelas não mudam; só a consulta salva some do arquivo. Isso não pode \
             ser desfeito.",
            tabela.nome
        );
        self.confirmacao = Some(Confirmacao::new(frase).titulo("Excluir visão").rotulo("Excluir").perigo(true));
    }

    /// O script mostrado (para os testes).
    #[cfg(test)]
    pub(crate) fn script(&self) -> &str {
        &self.script
    }
}

/// A Definição SQL como parte da tela de tabela: a faixa com o "Copiar", o Excluir da visão na barra e, nas
/// Propriedades, "Abrir na Consulta SQL".
impl estado::ParteTabela for AbaDefinicao {
    fn anatomia(&self) -> estado::Anatomia {
        estado::Anatomia::default().com_faixa().rolagem_propria()
    }

    fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.atualizar(tabela);
        titulo_da_faixa(ui, "Comando de criação", "");
        if self.script.is_empty() {
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(Botao::ghost("Copiar").pequeno()).clicked() {
                ui.ctx().copy_text(self.script.clone());
                interface::pecas::toast::toast_concluido(ui.ctx(), "Copiado.");
            }
        });
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.conteudo_da_parte(ui, ctx, tabela);
    }

    /// Só a visão se exclui aqui ("Excluir a visão v"), fora do banco só para leitura.
    fn ofertas(&self, tabela: &TabelaAberta, estado: &estado::Estado) -> Ofertas {
        if tabela.info().tipo != TipoObjeto::View {
            return Ofertas::default();
        }
        let livre = !estado.somente_leitura() && self.tarefa.is_none();
        Ofertas::default()
            .com(Verbo::Excluir, Oferta::ligada().ligada_se(livre).com_dica(format!("Excluir a visão {}", tabela.nome)))
    }

    fn executar(&mut self, verbo: Verbo, _egui: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let e_visao = tabela.info().tipo == TipoObjeto::View;
        if verbo == Verbo::Excluir && e_visao && !ctx.estado.somente_leitura() && self.tarefa.is_none() {
            self.pedir_excluir_visao(tabela);
        }
    }

    /// "Abrir na Consulta SQL": os dois comandos prontos, que levam o texto ao editor.
    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        estado::secao_abrir_na_consulta(ui, ctx, &tabela.nome);
    }

    fn ocupada(&self) -> bool {
        self.tarefa.is_some()
    }
}
