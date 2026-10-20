//! Aba Estrutura: a
//! faixa "Colunas · 7 colunas"; a lista de colunas de ponta a ponta (#, o nome que seleciona e abre "Alterar a
//! coluna", a pastilha do tipo, as regras em pills com o "+N" das que não cabem, o padrão e as ações Subir, Descer e
//! Excluir a coluna em botões de ícone); as listas Chaves estrangeiras e Restrições da tabela; e as Propriedades
//! ("Renomear a tabela" no fim da seção Tabela, a coluna selecionada com "Alterar a coluna" e a zona "Excluir a
//! tabela", recolhida). Na barra, a Nova coluna (o "+" fica desligado, D2 c), o Excluir da coluna selecionada e a
//! busca "Buscar colunas". Mais as janelas de coluna e de renomear.

use std::cell::Cell;

use banco::ColunaInfo;
use ddl::{ColunaDef, Gerada, Referencia};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Id, Margin, Sense, Ui, WidgetInfo, WidgetType, vec2};
use estado::{Aba, Acao, Contexto, Oferta, Ofertas, PedidoTabela, Promessa, Reacao, Rota, TabelaAberta, Verbo};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_numero, plural, sem_acento};
use interface::icones::{self, Icone, TipoColuna};
use interface::pecas::avisos::{TomAviso, aviso_texto, aviso_tom, vazio};
use interface::pecas::botao::{Botao, LADO_PEQUENO, TipoBotao};
use interface::pecas::campos::{
    Check, Entrada, EntradaSugestoes, Select, campo, campo_com_dica, campos_duas_colunas, enter_no_campo,
};
use interface::pecas::detalhes::Detalhes;
use interface::pecas::documento::de_ponta_a_ponta;
use interface::pecas::janela::{Confirmacao, Janela, confirmar};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::rotulos::{TipoPill, etiqueta_mais, pill, tag, tamanho_pill};
use interface::pecas::tabela::{ColunaTabela, Tabela, moldura_lista};
use interface::pecas::texto::{Trecho, galley_cortado, galley_quebrado, mostrar_paragrafo, nome_abre, texto};
use interface::tema::paleta;
use servico::{OperacaoEsquema, RespostaEsquema, TabelaCompleta};

use crate::apoio::{
    ACOES_FK, COLACOES, MSG_ESTRUTURA_ALTERADA, TIPOS, assinatura, codigo_quebravel, fluxo_medido, largura_check,
    largura_tag, motivo_sem_edicao, nova_instancia, opcoes_select, painel_rebaixado, pode_editar, texto_referencia,
    titulo_da_faixa, titulo_lista,
};
use crate::mudanca::{Montagem, Mudanca};

/// Como uma janela da aba terminou o quadro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FimJanela {
    Aberta,
    Fechada,
    /// A mudança foi aplicada (a janela fechou; quem a abriu recarrega).
    Concluida,
}

/// A frase do toast de uma resposta do serviço (o `avisoDeGravacao(r, r.descricao || "Estrutura alterada.")`).
pub(crate) fn toast_da_resposta(egui: &egui::Context, ctx: &mut Contexto<'_>, r: &RespostaEsquema) {
    let frase = if r.descricao.is_empty() { MSG_ESTRUTURA_ALTERADA } else { r.descricao.as_str() };
    let msg = ctx.aviso_de_gravacao(r.transacao_aberta, frase);
    interface::pecas::toast::toast_concluido(egui, msg);
}

/// Depois de mudar a estrutura (o `ctx.recarregar()` das abas): a casca relê a tabela e os objetos.
pub(crate) fn pedir_recarga(ctx: &mut Contexto<'_>, tabela: &mut TabelaAberta) {
    tabela.recarregar = true;
    ctx.emitir(Acao::RecarregarObjetos);
}

/// A ColunaDef do analisador para a coluna (o `definicaoDe`: o nome sem diferenciar maiúsculas).
fn definicao_de<'a>(completa: &'a TabelaCompleta, nome: &str) -> Option<&'a ColunaDef> {
    let baixo = nome.to_lowercase();
    completa.definicoes.as_ref()?.iter().find(|d| d.nome.to_lowercase() == baixo)
}

/// O ícone sólido da tabela (o cabeçalho das janelas de coluna e de renomear).
pub(crate) fn icone_da_tabela(tabela: &TabelaAberta) -> Icone {
    let info = tabela.info();
    estado::icone_da_tabela(info.tipo, info.sem_rowid)
}

/// A pastilha de uma coluna: a chave primária, a estrangeira e o tipo declarado.
pub(crate) fn pastilha_da_coluna(col: &ColunaInfo) -> TipoColuna {
    TipoColuna::do_declarado(&col.tipo, col.pk > 0, col.fk.is_some())
}

/// Uma ação pedida numa linha da lista de colunas.
enum AcaoLinha {
    /// O clique na linha: seleciona.
    Selecionar(String),
    /// O clique no nome: seleciona e abre "Alterar a coluna".
    Abrir(String),
    Mover(usize, bool),
    Excluir(String),
}

/// A aba Estrutura de uma tabela.
pub struct AbaEstrutura {
    janela_coluna: Option<JanelaColuna>,
    janela_renomear: Option<JanelaRenomear>,
    confirmacao: Option<Confirmacao>,
    /// A mudança que a confirmação aberta vai mandar.
    pendente: Option<OperacaoEsquema>,
    /// A mudança direta (mover, excluir coluna) a caminho.
    tarefa: Option<Promessa<RespostaEsquema>>,
    /// O erro de uma mudança direta, no alto da lista de colunas.
    erro_colunas: Option<String>,
    /// O erro acabou de chegar: a página rola até ele no próximo desenho.
    rolar_erro_colunas: bool,
    /// O que foi digitado na zona "Excluir a tabela".
    confirmar_nome: String,
    excluindo_tabela: Option<Promessa<RespostaEsquema>>,
    erro_zona: Option<String>,
    /// O número desta aba (a zona "Excluir a tabela" nasce fechada em cada aba nova).
    instancia: u64,
    /// O texto da busca da barra ("Buscar colunas"): filtra a lista pelo nome, sem acento nem caixa.
    busca: String,
    /// A coluna selecionada (o `tr.sel`), pelo nome: a das Propriedades e do Excluir da barra.
    selecionada: Option<String>,
    chaves_busca: ChavesBusca,
}

/// O nome de cada coluna sem acento nem caixa (a chave da busca da barra), guardado com o resumo dos nomes de que
/// saiu: refeito só quando as colunas mudam, não a cada quadro.
#[derive(Default)]
struct ChavesBusca {
    assinatura: u64,
    chaves: Vec<String>,
}

impl ChavesBusca {
    fn de(&mut self, colunas: &[ColunaInfo]) -> &[String] {
        let a = assinatura(colunas.iter().map(|c| c.nome.as_str()));
        if a != self.assinatura || self.chaves.len() != colunas.len() {
            self.chaves = colunas.iter().map(|c| sem_acento(&c.nome)).collect();
            self.assinatura = a;
        }
        &self.chaves
    }
}

impl AbaEstrutura {
    pub fn nova(_tabela: &TabelaAberta) -> AbaEstrutura {
        AbaEstrutura {
            janela_coluna: None,
            janela_renomear: None,
            confirmacao: None,
            pendente: None,
            tarefa: None,
            erro_colunas: None,
            rolar_erro_colunas: false,
            confirmar_nome: String::new(),
            excluindo_tabela: None,
            erro_zona: None,
            instancia: nova_instancia(),
            busca: String::new(),
            selecionada: None,
            chaves_busca: ChavesBusca::default(),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        estado::desenhar_parte_empilhada(self, ui, ctx, tabela);
    }

    /// A coluna selecionada, se ela ainda existe na tabela.
    fn coluna_selecionada<'t>(&self, tabela: &'t TabelaAberta) -> Option<&'t ColunaInfo> {
        let nome = self.selecionada.as_deref()?;
        tabela.info().colunas.iter().find(|c| c.nome == nome)
    }

    /// A coluna pode ser excluída (não é chave primária e a tabela tem outras).
    fn pode_excluir(tabela: &TabelaAberta, col: &ColunaInfo) -> bool {
        col.pk == 0 && tabela.info().colunas.len() > 1
    }

    /// O conteúdo da parte (o `ParteTabela::conteudo`), no lugar dela no documento.
    fn conteudo_da_parte(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let egui = ui.ctx().clone();
        self.receber(&egui, ctx, tabela);
        let somente_leitura = ctx.estado.somente_leitura();
        let editavel = pode_editar(tabela.info(), somente_leitura);

        let mut acao = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(msg) = &self.erro_colunas {
                let r = aviso_texto(ui, msg, true);
                if std::mem::take(&mut self.rolar_erro_colunas) {
                    ui.scroll_to_rect(r.rect.expand2(vec2(0.0, 18.0)), None);
                }
                ui.add_space(ESPACO_LISTAS);
            }
            let colunas = &tabela.info().colunas;
            // as colunas que a busca da barra acha (pelo nome, sem acento nem caixa), na ordem da tabela; as chaves
            // dos nomes só são refeitas quando as colunas mudam
            let termo = sem_acento(self.busca.trim());
            let visiveis: Vec<usize> = if termo.is_empty() {
                (0..colunas.len()).collect()
            } else {
                let chaves = self.chaves_busca.de(colunas);
                (0..colunas.len()).filter(|&i| chaves.get(i).is_some_and(|c| c.contains(&termo))).collect()
            };
            if visiveis.is_empty() {
                let texto = if colunas.is_empty() { VAZIO_COLUNAS } else { VAZIO_BUSCA_COLUNAS };
                vazio(ui, texto, true);
            } else if ui.available_width() < LARGURA_LISTA_LARGA {
                // a doca estreita (as Propriedades, vista Colunas): a lista compacta
                let selecionada = self.selecionada.as_deref();
                lista_colunas_compacta(ui, tabela, &visiveis, editavel, selecionada, &mut acao);
            } else {
                let selecionada = self.selecionada.as_deref();
                de_ponta_a_ponta(ui, |ui| lista_colunas(ui, tabela, &visiveis, editavel, selecionada, &mut acao));
            }
            if !editavel {
                ui.add_space(ESPACO_LISTAS);
                let motivo = motivo_sem_edicao(&tabela.completa, somente_leitura);
                aviso_tom(ui, &[Trecho::Normal(&motivo)], TomAviso::Informacao);
            }
            if !tabela.info().chaves_estrangeiras.is_empty() {
                ui.add_space(ESPACO_LISTAS);
                lista_chaves(ui, tabela);
            }
            if let Some(restricoes) = tabela.completa.restricoes.as_ref().filter(|r| !r.is_empty()) {
                ui.add_space(ESPACO_LISTAS);
                lista_restricoes(ui, restricoes);
            }
        });

        if let Some(a) = acao {
            self.pedir(&egui, ctx, tabela, a, editavel);
        }

        if let Some(sim) = confirmar(&egui, &mut self.confirmacao) {
            let op = self.pendente.take();
            if let (true, Some(op)) = (sim, op) {
                self.tarefa = Some(ctx.tarefa(&egui, move |servico, id| servico.esquema(id, &op, false)));
            }
        }

        if let Some(j) = &mut self.janela_coluna {
            match j.mostrar(&egui, ctx) {
                FimJanela::Aberta => {}
                FimJanela::Fechada => self.janela_coluna = None,
                FimJanela::Concluida => {
                    self.janela_coluna = None;
                    pedir_recarga(ctx, tabela);
                }
            }
        }
        if let Some(j) = &mut self.janela_renomear {
            match j.mostrar(&egui, ctx) {
                FimJanela::Aberta => {}
                FimJanela::Fechada => self.janela_renomear = None,
                FimJanela::Concluida => {
                    let novo = j.novo.trim().to_string();
                    self.janela_renomear = None;
                    ctx.emitir(Acao::RecarregarObjetos);
                    ctx.navegar(Rota::objeto_na_aba(novo, false, Aba::Estrutura));
                }
            }
        }
    }

    /// Recebe as mudanças diretas que terminaram.
    fn receber(&mut self, egui: &egui::Context, ctx: &mut Contexto<'_>, tabela: &mut TabelaAberta) {
        if let Some(r) = self.tarefa.as_mut().and_then(Promessa::tomar) {
            self.tarefa = None;
            match r {
                Ok(r) => {
                    toast_da_resposta(egui, ctx, &r);
                    pedir_recarga(ctx, tabela);
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        self.erro_colunas = Some(e.mensagem);
                        self.rolar_erro_colunas = true;
                    }
                }
            }
        }
        if let Some(r) = self.excluindo_tabela.as_mut().and_then(Promessa::tomar) {
            self.excluindo_tabela = None;
            match r {
                Ok(r) => {
                    toast_da_resposta(egui, ctx, &r);
                    ctx.emitir(Acao::RecarregarObjetos);
                    ctx.navegar(Rota::Banco);
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        self.erro_zona = Some(e.mensagem);
                    }
                }
            }
        }
    }

    /// Seleciona a linha, abre a janela de coluna ou a confirmação de mover/excluir.
    fn pedir(
        &mut self,
        egui: &egui::Context,
        ctx: &mut Contexto<'_>,
        tabela: &TabelaAberta,
        acao: AcaoLinha,
        editavel: bool,
    ) {
        let nome_tabela = &tabela.nome;
        match acao {
            AcaoLinha::Selecionar(nome) => self.selecionada = Some(nome),
            AcaoLinha::Abrir(nome) => {
                if editavel {
                    self.janela_coluna = Some(JanelaColuna::nova(egui, ctx, tabela, Some(&nome)));
                }
                self.selecionada = Some(nome);
            }
            _ if self.tarefa.is_some() => {}
            AcaoLinha::Mover(i, para_cima) => {
                let mut ordem: Vec<String> = tabela.info().colunas.iter().map(|c| c.nome.clone()).collect();
                if i >= ordem.len() || (para_cima && i == 0) || (!para_cima && i + 1 >= ordem.len()) {
                    return;
                }
                let c = ordem.remove(i);
                let frase = format!(
                    "Mover a coluna {c} para {} reconstrói a tabela {nome_tabela}: os dados são copiados para uma \
                     tabela nova com a nova ordem. Continuar?",
                    if para_cima { "cima" } else { "baixo" }
                );
                ordem.insert(if para_cima { i - 1 } else { i + 1 }, c);
                self.pendente = Some(OperacaoEsquema::ReordenarColunas { tabela: nome_tabela.clone(), ordem });
                self.confirmacao =
                    Some(Confirmacao::new(frase).titulo("Mudar a ordem das colunas").rotulo("Reconstruir a tabela"));
            }
            AcaoLinha::Excluir(nome) => {
                let frase = frase_excluir_coluna(&nome, nome_tabela);
                self.pendente = Some(OperacaoEsquema::ExcluirColuna { tabela: nome_tabela.clone(), coluna: nome });
                self.confirmacao =
                    Some(Confirmacao::new(frase).titulo("Excluir coluna").rotulo("Excluir").perigo(true));
            }
        }
    }

    /// A seção "Coluna" das Propriedades: o tipo com a pastilha, as regras, o padrão, a ordenação, a referência e
    /// "Alterar a coluna".
    fn secao_coluna(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &TabelaAberta, editavel: bool) {
        let Some(col) = self.coluna_selecionada(tabela) else { return };
        let def = definicao_de(&tabela.completa, &col.nome);
        let mut alterar = false;
        let _ = Secao::new("propriedades::coluna", "Coluna").extra(&col.nome).mostrar(ui, |ui| {
            ficha(ui, |d| linhas_da_coluna(d, col, def));
            if editavel {
                acoes_da_secao(ui, |ui| {
                    alterar = ui.add(Botao::new("Alterar a coluna").lista().com_icone(Icone::Editar)).clicked();
                });
            }
        });
        if alterar {
            let egui = ui.ctx().clone();
            let nome = col.nome.clone();
            self.janela_coluna = Some(JanelaColuna::nova(&egui, ctx, tabela, Some(&nome)));
            egui.request_repaint();
        }
    }

    /// A zona "Excluir a tabela" das Propriedades (recolhida, em `--perigo`): a consequência, o nome digitado para
    /// confirmar e o botão.
    fn secao_excluir_tabela(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &TabelaAberta) {
        let info = tabela.info();
        let quantas = match info.linhas {
            None => "todas as linhas dela".to_string(),
            Some(1) => "a única linha dela".to_string(),
            Some(n) => format!("as {} linhas", formatar_numero(n)),
        };
        let mut excluir = false;
        let ocupado = self.excluindo_tabela.is_some();
        let confirmar_nome = &mut self.confirmar_nome;
        let erro = self.erro_zona.as_deref();
        // a aba nova (refeita a cada recarga e a cada volta à aba) traz a zona fechada (PADROES §4.11)
        let secao = Secao::new(("propriedades::excluir_tabela", self.instancia), "Excluir a tabela");
        let _ = secao.perigo(true).recolhida(true).mostrar(ui, |ui| {
            corpo_da_secao(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let p = paleta(ui);
                let frase = format!(
                    "A tabela e {quantas} somem do arquivo, com os índices e gatilhos dela. Não há como desfazer."
                );
                let g = galley_quebrado(
                    ui,
                    &frase,
                    fontes::fonte(12.0, Peso::Normal),
                    p.estudio.t2,
                    ui.available_width(),
                    1.4,
                );
                mostrar_paragrafo(ui, &g, &frase);
                let pode = !ocupado && *confirmar_nome == info.nome;
                let resp = campo(ui, ROTULO_CONFIRMAR, None, |ui| {
                    Entrada::new(confirmar_nome)
                        .dica(&info.nome)
                        .rotulo(ROTULO_CONFIRMAR)
                        .id_salt(("confirmar_nome", &info.nome))
                        .mostrar(ui)
                        .response
                        .response
                });
                if enter_no_campo(ui, &resp) && pode {
                    excluir = true;
                }
                if ui.add(Botao::perigo("Excluir tabela").habilitado(pode).ocupado(ocupado)).clicked() {
                    excluir = true;
                }
                if let Some(msg) = erro {
                    aviso_texto(ui, msg, true);
                }
            });
        });
        if excluir && self.excluindo_tabela.is_none() && self.confirmar_nome == info.nome {
            self.erro_zona = None;
            let op = OperacaoEsquema::ExcluirTabela { tabela: info.nome.clone() };
            let egui = ui.ctx().clone();
            self.excluindo_tabela = Some(ctx.tarefa(&egui, move |servico, id| servico.esquema(id, &op, false)));
        }
    }

    #[cfg(test)]
    pub(crate) fn janela_coluna(&self) -> Option<&JanelaColuna> {
        self.janela_coluna.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn janela_renomear(&self) -> Option<&JanelaRenomear> {
        self.janela_renomear.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn selecionada(&self) -> Option<&str> {
        self.selecionada.as_deref()
    }
}

/// O rótulo do campo que confirma a exclusão da tabela.
const ROTULO_CONFIRMAR: &str = "Digite o nome da tabela para confirmar";
/// O espaço entre as listas do documento (`gap:18px`, o da página de `Main.dc.html`).
const ESPACO_LISTAS: f32 = 18.0;

/// O corpo da zona "Excluir a tabela" com o `padding:2px 14px 14px` da ficha (`.det`).
fn corpo_da_secao(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE.inner_margin(Margin { left: 14, right: 14, top: 2, bottom: 14 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        conteudo(ui);
    });
}

/// As ações de uma seção das Propriedades (`padding:0 8px 10px`, os `Botao::lista` da largura toda).
pub(crate) fn acoes_da_secao(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE.inner_margin(Margin { left: 8, right: 8, top: 0, bottom: 10 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        conteudo(ui);
    });
}

/// As linhas da seção "Coluna": Tipo (a pastilha e o tipo), Regras, Padrão, Ordenação e Referência.
fn linhas_da_coluna(d: &mut Detalhes<'_>, col: &ColunaInfo, def: Option<&ColunaDef>) {
    // `dd.tipo-d{display:flex;align-items:center;gap:7px}` na linha de 17px da ficha
    d.linha("Tipo", |ui| {
        em_linha_da_ficha(ui, |ui| desenhar_tipo(ui, col));
    });
    let itens = itens_regras(d.ui(), col, def);
    d.linha("Regras", |ui| {
        if itens.is_empty() {
            texto(ui, "—", 12.0, paleta(ui).estudio.t3);
        } else {
            // as pills quebram linha na coluna do valor, 4px entre elas
            let larguras: Vec<f32> = itens.iter().map(|it| it.largura).collect();
            // a tag da referência (24,5px) pede a linha mais alta
            let altura = if itens.iter().any(|it| it.tipo.is_none()) { ALTURA_TAG } else { ALTURA_LINHA_FICHA };
            fluxo_medido(ui, &larguras, altura, 4.0, 4.0, |ui, i| itens[i].desenhar(ui));
        }
    });
    let padrao = col.padrao.as_deref().filter(|s| !s.is_empty());
    d.linha_texto("Padrão", padrao, "—");
    let colacao = def.and_then(|d| d.colacao.as_deref());
    d.linha_texto("Ordenação", colacao, "nenhuma");
    let referencia = col.fk.as_ref().map(|fk| texto_referencia(&fk.tabela, std::slice::from_ref(&fk.coluna)));
    d.linha_texto("Referência", referencia.as_deref(), "—");
}

/// A linha da ficha (`.det`: 12px × 1,4 = 17px) e a da `.tag` (24,5px, arredondada para cima).
const ALTURA_LINHA_FICHA: f32 = 17.0;
const ALTURA_TAG: f32 = 25.0;

/// Um valor da ficha em linha, centrado na linha de 17px.
fn em_linha_da_ficha(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
    let largura = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(largura, ALTURA_LINHA_FICHA),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(vec2(largura, ALTURA_LINHA_FICHA));
            conteudo(ui);
        },
    );
}

/// A pastilha de 16px de um tipo de coluna (o `.sk.s16.tipo`), sem nó para o leitor de tela (o tipo vem escrito ao
/// lado).
fn pastilha(ui: &mut Ui, tipo: TipoColuna) {
    let (rect, _) = ui.allocate_exact_size(vec2(LADO_PASTILHA, LADO_PASTILHA), Sense::hover());
    if ui.is_rect_visible(rect) {
        icones::pintar(ui.painter(), rect, Icone::Tipo(tipo), egui::Color32::WHITE);
    }
}

/// A pastilha das listas e das Propriedades (`.sk.s16`).
const LADO_PASTILHA: f32 = 16.0;

pub(crate) fn frase_excluir_coluna(coluna: &str, tabela: &str) -> String {
    format!(
        "Excluir a coluna {coluna} de {tabela}? Os valores dessa coluna somem do arquivo. Isso não pode ser desfeito."
    )
}

// ------------------------------------------------------------------ a lista de colunas

/// As larguras fixas da lista (`th` da prancheta 6, com o item 18 da revisão): # 44, Nome 150, Tipo 120, Padrão 150 e
/// as ações 96; as Regras ficam com o resto.
const LARGURA_NUMERO: f32 = 44.0;
const LARGURA_NOME: f32 = 150.0;
const LARGURA_TIPO: f32 = 120.0;
const LARGURA_PADRAO: f32 = 150.0;
/// A coluna Regras não fica abaixo de 170 (a primeira pill e o "+N" cabem); o Padrão cede primeiro, até 72.
const LARGURA_MIN_REGRAS: f32 = 170.0;
const LARGURA_MIN_PADRAO: f32 = 72.0;
const LARGURA_ACOES: f32 = 96.0;
/// As colunas da lista.
const COL_NUMERO: usize = 0;
const COL_NOME: usize = 1;
const COL_TIPO: usize = 2;
const COL_REGRAS: usize = 3;
const COL_PADRAO: usize = 4;
/// O espaço entre as pills das Regras (`gap:6px`).
const ESPACO_REGRAS: f32 = 6.0;
/// O espaço entre a pastilha e o tipo (`gap:7px`).
const ESPACO_PASTILHA: f32 = 7.0;

/// Um item das Regras: pill (`tipo`) ou a tag da referência (`None`), com a dica do `title` e a largura.
struct ItemRegra {
    texto: String,
    tipo: Option<TipoPill>,
    dica: Option<String>,
    largura: f32,
}

impl ItemRegra {
    fn desenhar(&self, ui: &mut Ui) {
        let r = match self.tipo {
            Some(tipo) => pill(ui, &self.texto, tipo),
            None => tag(ui, &self.texto),
        };
        if let Some(d) = &self.dica {
            r.on_hover_text(d);
        }
    }
}

/// As regras de uma coluna, medidas: a chave primária na cor de destaque, as outras neutras (a
/// gerada e a oculta apagadas) e a referência numa tag.
fn itens_regras(ui: &Ui, col: &ColunaInfo, def: Option<&ColunaDef>) -> Vec<ItemRegra> {
    let mut itens = Vec::new();
    let mut pill_ = |texto: String, tipo: TipoPill, dica: Option<String>| {
        let largura = tamanho_pill(ui, &texto).x;
        itens.push(ItemRegra { texto, tipo: Some(tipo), dica, largura });
    };
    if col.pk > 0 {
        pill_("chave primária".into(), TipoPill::Acc, None);
    }
    if col.autoincremento {
        pill_("autoincremento".into(), TipoPill::Neutra, None);
    }
    if col.nao_nulo {
        pill_("não aceita nulo".into(), TipoPill::Neutra, None);
    }
    if col.unica {
        pill_("único".into(), TipoPill::Neutra, None);
    }
    if col.gerada {
        let dica = def
            .and_then(|d| d.gerada.as_ref())
            .map(|g| format!("Calculada: {} ({})", g.expressao, if g.armazenada { "armazenada" } else { "virtual" }));
        pill_("gerada".into(), TipoPill::Mute, dica);
    }
    if let Some(v) = def.and_then(|d| d.verificacao.as_deref()) {
        pill_("verificação".into(), TipoPill::Neutra, Some(format!("CHECK ({v})")));
    }
    if let Some(c) = def.and_then(|d| d.colacao.as_deref()) {
        pill_(format!("ordenação {c}"), TipoPill::Neutra, Some(format!("COLLATE {c}")));
    }
    if col.oculta == 1 {
        let dica = "Coluna oculta da tabela virtual: existe para consultas especiais e não aparece na grade.";
        pill_("oculta".into(), TipoPill::Mute, Some(dica.into()));
    }
    if let Some(fk) = &col.fk {
        let texto = texto_referencia(&fk.tabela, std::slice::from_ref(&fk.coluna));
        let largura = largura_tag(ui, &texto);
        itens.push(ItemRegra { texto, tipo: None, dica: None, largura });
    }
    itens
}

/// Quantos itens das Regras cabem em `largura` (`flex-wrap:nowrap` com o "+N" das que não cabem, item 18): todos,
/// quando cabem; senão, o máximo que deixa lugar para o "+N" dos outros. `largura_mais(n)` é a do "+n".
pub(crate) fn regras_que_cabem(larguras: &[f32], largura: f32, largura_mais: impl Fn(usize) -> f32) -> usize {
    let ate = |k: usize| -> f32 { larguras[..k].iter().sum::<f32>() + ESPACO_REGRAS * k.saturating_sub(1) as f32 };
    let n = larguras.len();
    if ate(n) <= largura + 0.5 {
        return n;
    }
    (0..n)
        .rev()
        .find(|&k| ate(k) + if k > 0 { ESPACO_REGRAS } else { 0.0 } + largura_mais(n - k) <= largura + 0.5)
        .unwrap_or(0)
}

/// As Regras numa linha: os itens que cabem e o "+N" com os outros na dica.
fn desenhar_regras(ui: &mut Ui, itens: &[ItemRegra]) {
    let larguras: Vec<f32> = itens.iter().map(|it| it.largura).collect();
    let mais = |n: usize| tamanho_pill(ui, &format!("+{n}")).x;
    // sempre a primeira pill (e o "+N" dos outros), mesmo numa coluna apertada (revisão de 17/10/2026)
    let k = regras_que_cabem(&larguras, ui.available_width(), mais).max(1.min(itens.len()));
    ui.spacing_mut().item_spacing.x = ESPACO_REGRAS;
    for it in &itens[..k] {
        it.desenhar(ui);
    }
    if k < itens.len() {
        let resto: Vec<&str> = itens[k..].iter().map(|it| it.texto.as_str()).collect();
        etiqueta_mais(ui, itens.len() - k, &resto.join(" · "));
    }
}

/// O Padrão numa linha (`td.mono`, nowrap): o texto em `--t2`, peso 350, encurtado com "…" e com o texto inteiro na
/// dica quando não cabe; sem padrão, "—".
fn desenhar_padrao(ui: &mut Ui, padrao: Option<&str>) {
    let e = &paleta(ui).estudio;
    let texto = match padrao.filter(|s| !s.trim().is_empty()) {
        Some(p) => p.split_whitespace().collect::<Vec<_>>().join(" "),
        None => "—".to_string(),
    };
    let g = galley_cortado(ui, &texto, fontes::fonte(12.0, Peso::Leve), e.t2, ui.available_width());
    let cortado = g.elided;
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, e.t2);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &texto));
    if cortado {
        resp.on_hover_text(&texto);
    }
}

/// O tipo com a pastilha (`gap:7px`), ou "sem tipo" em `--t3`.
fn desenhar_tipo(ui: &mut Ui, col: &ColunaInfo) {
    ui.spacing_mut().item_spacing.x = ESPACO_PASTILHA;
    pastilha(ui, pastilha_da_coluna(col));
    let e = &paleta(ui).estudio;
    let (t, cor) = if col.tipo.is_empty() { ("sem tipo", e.t3) } else { (col.tipo.as_str(), e.t) };
    let g = galley_cortado(ui, t, fontes::fonte(12.0, Peso::Normal), cor, ui.available_width());
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, cor);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, t));
}

/// As ações de uma linha (`td` de 96px à direita): Subir e Descer (com a dica de que reconstroem a tabela) e Excluir a
/// coluna (`.ib.pq.perigo`). O que não vale na linha (subir a primeira, descer a última, excluir a chave primária ou a
/// única coluna) só guarda o lugar, sem botão.
fn desenhar_acoes(ui: &mut Ui, tabela: &TabelaAberta, i: usize, acao: &mut Option<AcaoLinha>) {
    let colunas = &tabela.info().colunas;
    let Some(col) = colunas.get(i) else { return };
    let lugar = |ui: &mut Ui| {
        ui.allocate_exact_size(vec2(LADO_PEQUENO, LADO_PEQUENO), Sense::hover());
    };
    // da direita para a esquerda (a célula é alinhada à direita)
    ui.spacing_mut().item_spacing.x = 0.0;
    if AbaEstrutura::pode_excluir(tabela, col) {
        let b = Botao::icone(Icone::Excluir).pequeno().tipo(TipoBotao::GhostPerigo);
        let rotulo = rotulo_da_acao("Excluir", &col.nome);
        if ui.add(b.rotulo(&rotulo).dica(ROTULO_EXCLUIR_LINHA)).clicked() {
            *acao = Some(AcaoLinha::Excluir(col.nome.clone()));
        }
    } else {
        lugar(ui);
    }
    if i + 1 < colunas.len() {
        let rotulo = rotulo_da_acao("Descer", &col.nome);
        if ui.add(Botao::icone(Icone::Descer).pequeno().rotulo(&rotulo).dica(DICA_DESCER)).clicked() {
            *acao = Some(AcaoLinha::Mover(i, false));
        }
    } else {
        lugar(ui);
    }
    if i > 0 {
        let rotulo = rotulo_da_acao("Subir", &col.nome);
        if ui.add(Botao::icone(Icone::Subir).pequeno().rotulo(&rotulo).dica(DICA_SUBIR)).clicked() {
            *acao = Some(AcaoLinha::Mover(i, true));
        }
    } else {
        lugar(ui);
    }
}

/// A dica do Excluir de cada linha (o `aria-label` da prancheta, que o leitor de tela ouve com o nome da coluna,
/// [`rotulo_da_acao`]).
pub(crate) const ROTULO_EXCLUIR_LINHA: &str = "Excluir a coluna";

/// O rótulo de uma ação da linha para o leitor de tela, com a coluna: "Subir a coluna email", "Descer a coluna
/// email", "Excluir a coluna email" (com muitas linhas, só o verbo não diria qual).
pub(crate) fn rotulo_da_acao(verbo: &str, coluna: &str) -> String {
    format!("{verbo} a coluna {coluna}")
}
pub(crate) const DICA_SUBIR: &str = "Subir (reconstrói a tabela)";
pub(crate) const DICA_DESCER: &str = "Descer (reconstrói a tabela)";

/// A largura da coluna Padrão numa lista de `disponivel` pontos: os 150 de sempre, e menos quando a janela aperta (as
/// Regras nunca ficam abaixo de [`LARGURA_MIN_REGRAS`]: o Padrão cede primeiro, até [`LARGURA_MIN_PADRAO`]). Sem as
/// ações (`editavel` falso), a coluna delas não conta.
pub(crate) fn largura_do_padrao(disponivel: f32, editavel: bool) -> f32 {
    let fixas = LARGURA_NUMERO + LARGURA_NOME + LARGURA_TIPO + if editavel { LARGURA_ACOES } else { 0.0 };
    // a moldura da lista: 1px de cada lado
    (disponivel - 2.0 - fixas - LARGURA_MIN_REGRAS).clamp(LARGURA_MIN_PADRAO, LARGURA_PADRAO)
}

/// A lista de colunas (`.lista-t` da prancheta 6), só com as `visiveis` (os índices em `info.colunas`: a busca da
/// barra), com a selecionada em `tr.sel`.
fn lista_colunas(
    ui: &mut Ui,
    tabela: &TabelaAberta,
    visiveis: &[usize],
    editavel: bool,
    selecionada: Option<&str>,
    acao: &mut Option<AcaoLinha>,
) {
    let colunas = &tabela.info().colunas;
    let largura_padrao = largura_do_padrao(ui.available_width(), editavel);
    let mut cabecalhos = vec![
        ColunaTabela::new("#").direita().fixa(LARGURA_NUMERO),
        ColunaTabela::new("Nome").fixa(LARGURA_NOME),
        ColunaTabela::new("Tipo").fixa(LARGURA_TIPO),
        ColunaTabela::new("Regras").estica(),
        ColunaTabela::new("Padrão").fixa(largura_padrao),
    ];
    if editavel {
        cabecalhos.push(ColunaTabela::new("").direita().fixa(LARGURA_ACOES));
    }
    let marcada =
        |l: usize| visiveis.get(l).and_then(|&i| colunas.get(i)).is_some_and(|c| Some(c.nome.as_str()) == selecionada);
    let r = Tabela::new(("colunas", &tabela.nome), &cabecalhos, visiveis.len()).marcadas(marcada).clicavel().mostrar(
        ui,
        |ui, l, c| {
            let Some(&i) = visiveis.get(l) else { return };
            let Some(col) = colunas.get(i) else { return };
            let e = &paleta(ui).estudio;
            match c {
                COL_NUMERO => {
                    texto(ui, &(i + 1).to_string(), 12.0, e.t3);
                }
                // o nome que abre "Alterar a coluna" (`a.link`); sem edição, só o nome em 600 (a linha seleciona)
                COL_NOME if editavel => {
                    let r = nome_abre(ui, &col.nome, false);
                    if r.clicked() {
                        *acao = Some(AcaoLinha::Abrir(col.nome.clone()));
                    }
                    let _ = r.on_hover_text(format!("Alterar a coluna {}", col.nome));
                }
                COL_NOME => {
                    let g = galley_cortado(ui, &col.nome, fontes::fonte(12.0, Peso::Medio), e.t, ui.available_width());
                    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                    ui.painter().galley(rect.min, g, e.t);
                    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &col.nome));
                }
                COL_TIPO => desenhar_tipo(ui, col),
                COL_REGRAS => {
                    let itens = itens_regras(ui, col, definicao_de(&tabela.completa, &col.nome));
                    desenhar_regras(ui, &itens);
                }
                COL_PADRAO => desenhar_padrao(ui, col.padrao.as_deref()),
                _ => desenhar_acoes(ui, tabela, i, acao),
            }
        },
    );
    if let Some(l) = r.linha_clicada
        && acao.is_none()
        && let Some(col) = visiveis.get(l).and_then(|&i| colunas.get(i))
    {
        *acao = Some(AcaoLinha::Selecionar(col.nome.clone()));
    }
}

/// A partir desta largura, as colunas vão na tabela (`.lista-t`: o documento largo, a janela própria das
/// Propriedades); abaixo, na lista compacta da doca ([`lista_colunas_compacta`], revisão de UI de 19/10/2026).
pub const LARGURA_LISTA_LARGA: f32 = 560.0;
/// `.cp-lista{padding:0 6px}` e `.cp-col{padding:6px 8px;border-radius:6px;gap:2px 8px}`.
const RECUO_LISTA_COMPACTA: f32 = 6.0;
const RECUO_ITEM_COMPACTO: [f32; 2] = [8.0, 6.0];
const RAIO_ITEM_COMPACTO: f32 = 6.0;
/// A pastilha de 16px e o `gap:8px` até o nome: o recuo da segunda linha (as regras) e das ações.
const RECUO_SEGUNDA_LINHA: f32 = 16.0 + 8.0;

/// A lista compacta de colunas (`.cp-lista` e `.cp-col` de `Dados.dc.html`, a vista Colunas das Propriedades na doca):
/// cada coluna em duas linhas, a pastilha do tipo, o nome (o `a.link` que abre "Alterar a coluna"; sem edição, só o
/// nome) e o tipo em `--t3` à direita; embaixo, as regras e o padrão. A coluna clicada fica selecionada (`.sel`, o
/// fundo `--sel`) e ganha Subir, Descer e Excluir numa terceira linha; no hover, o fundo `--h`.
fn lista_colunas_compacta(
    ui: &mut Ui,
    tabela: &TabelaAberta,
    visiveis: &[usize],
    editavel: bool,
    selecionada: Option<&str>,
    acao: &mut Option<AcaoLinha>,
) {
    let colunas = &tabela.info().colunas;
    let e = paleta(ui).estudio;
    let largura = ui.available_width();
    for &i in visiveis {
        let Some(col) = colunas.get(i) else { continue };
        let sel = selecionada == Some(col.nome.as_str());
        let id = ui.id().with(("cp-col", &col.nome));
        let topo = ui.cursor().min;
        let x0 = ui.max_rect().left() + RECUO_LISTA_COMPACTA;
        let x1 = ui.max_rect().right() - RECUO_LISTA_COMPACTA;
        // a linha inteira seleciona (registrada antes dos filhos, que ficam por cima: o nome e as ações)
        let altura_antes: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(44.0);
        let fila = egui::Rect::from_min_max(egui::pos2(x0, topo.y), egui::pos2(x1, topo.y + altura_antes));
        let resp = ui.interact(fila, id, Sense::click());
        let fundo = ui.painter().add(egui::Shape::Noop);
        let [rx, ry] = RECUO_ITEM_COMPACTO;
        let dentro = egui::Rect::from_min_max(egui::pos2(x0 + rx, topo.y + ry), egui::pos2(x1 - rx, f32::INFINITY));
        let mut filho = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("cp-col-dentro", &col.nome))
                .max_rect(dentro)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        filho.spacing_mut().item_spacing.y = 2.0;
        filho.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            pastilha(ui, pastilha_da_coluna(col));
            // o tipo à direita (`.ti`: 11px em `--t3`), medido antes para o nome saber até onde ir
            let (t, cor) = if col.tipo.is_empty() { ("sem tipo", e.t3) } else { (col.tipo.as_str(), e.t3) };
            let g_tipo = galley_cortado(ui, t, fontes::fonte(11.0, Peso::Normal), cor, ui.available_width() / 2.0);
            let largura_nome = (ui.available_width() - g_tipo.size().x - 8.0).max(24.0);
            ui.allocate_ui_with_layout(
                vec2(largura_nome, 18.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if editavel {
                        let r = nome_abre(ui, &col.nome, false);
                        if r.clicked() {
                            *acao = Some(AcaoLinha::Abrir(col.nome.clone()));
                        }
                        let _ = r.on_hover_text(format!("Alterar a coluna {}", col.nome));
                    } else {
                        let g = galley_cortado(ui, &col.nome, fontes::fonte(12.0, Peso::Medio), e.t, largura_nome);
                        let (rect, r) = ui.allocate_exact_size(g.size(), Sense::hover());
                        ui.painter().galley(rect.min, g, e.t);
                        r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &col.nome));
                    }
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, r) = ui.allocate_exact_size(g_tipo.size(), Sense::hover());
                ui.painter().galley(rect.min, g_tipo, cor);
                r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, t));
            });
        });
        // as regras e o padrão (`.rg`, 11px em `--t3`), embaixo do nome
        let itens = itens_regras(&filho, col, definicao_de(&tabela.completa, &col.nome));
        let padrao = col.padrao.as_deref().filter(|p| !p.trim().is_empty());
        if !itens.is_empty() || padrao.is_some() {
            filho.horizontal(|ui| {
                ui.add_space(RECUO_SEGUNDA_LINHA - 8.0);
                if !itens.is_empty() {
                    desenhar_regras(ui, &itens);
                }
                if padrao.is_some() {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    desenhar_padrao(ui, padrao);
                }
            });
        }
        if sel && editavel {
            filho.horizontal(|ui| {
                ui.add_space(RECUO_SEGUNDA_LINHA - 6.0);
                ui.allocate_ui_with_layout(
                    vec2(3.0 * LADO_PEQUENO, LADO_PEQUENO),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| desenhar_acoes(ui, tabela, i, acao),
                );
            });
        }
        let altura = (filho.min_rect().height() + 2.0 * ry).ceil();
        if (altura - altura_antes).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(id, altura));
            ui.ctx().request_repaint();
        }
        let item = egui::Rect::from_min_max(egui::pos2(x0, topo.y), egui::pos2(x1, topo.y + altura));
        let cor = if sel {
            Some(e.sel)
        } else if resp.hovered() {
            Some(e.h)
        } else {
            None
        };
        if let Some(c) = cor {
            ui.painter().set(fundo, egui::epaint::RectShape::filled(item, RAIO_ITEM_COMPACTO, c));
        }
        if resp.has_focus() && interface::pintura::foco_visivel(ui.ctx()) {
            interface::pintura::anel_foco(ui.painter(), paleta(ui), item, RAIO_ITEM_COMPACTO);
        }
        resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, sel, &col.nome));
        if resp.clicked() && acao.is_none() {
            *acao = Some(AcaoLinha::Selecionar(col.nome.clone()));
        }
        ui.allocate_space(vec2(largura, altura));
    }
}

/// O vazio da lista sem colunas visíveis.
const VAZIO_COLUNAS: &str = "A tabela não tem colunas visíveis.";
/// O vazio quando a busca da barra não acha nenhuma.
pub const VAZIO_BUSCA_COLUNAS: &str = "Nenhuma coluna corresponde à busca.";
/// A dica (e o rótulo) da busca da barra na Estrutura.
pub const DICA_BUSCA_COLUNAS: &str = "Buscar colunas";

/// A lista Chaves estrangeiras (título 13/600 e a moldura `--b2`, como a de Tabelas da Visão geral).
fn lista_chaves(ui: &mut Ui, tabela: &TabelaAberta) {
    let fks = &tabela.info().chaves_estrangeiras;
    let cabecalhos = [
        ColunaTabela::new("Colunas"),
        ColunaTabela::new("Referência"),
        ColunaTabela::new("Ao excluir"),
        ColunaTabela::new("Ao atualizar"),
    ];
    titulo_lista(ui, "Chaves estrangeiras");
    moldura_lista(ui, |ui| {
        Tabela::new(("chaves", &tabela.nome), &cabecalhos, fks.len()).mostrar(ui, |ui, l, col| {
            let Some(fk) = fks.get(l) else { return };
            match col {
                0 => {
                    texto(ui, &fk.colunas.join(", "), 12.0, paleta(ui).estudio.t);
                }
                1 => {
                    tag(ui, &texto_referencia(&fk.tabela, &fk.colunas_ref));
                }
                2 => acao_fk(ui, &fk.ao_excluir),
                _ => acao_fk(ui, &fk.ao_atualizar),
            }
        });
    });
}

/// Ao excluir / Ao atualizar: o NO ACTION (o padrão) fica apagado, dito em português com o SQL entre parênteses
/// ("nenhuma ação (NO ACTION)").
fn acao_fk(ui: &mut Ui, acao: &str) {
    let e = &paleta(ui).estudio;
    if !acao.is_empty() && acao != "NO ACTION" {
        texto(ui, acao, 12.0, e.t);
    } else {
        texto(ui, "nenhuma ação (NO ACTION)", 12.0, e.t3);
    }
}

/// A lista Restrições da tabela: uma linha por restrição, no `code` que quebra.
fn lista_restricoes(ui: &mut Ui, restricoes: &[String]) {
    titulo_lista(ui, "Restrições da tabela");
    let cabecalhos = [ColunaTabela::new("Restrição")];
    moldura_lista(ui, |ui| {
        Tabela::new("restricoes", &cabecalhos, restricoes.len()).mostrar(ui, |ui, l, _| {
            if let Some(r) = restricoes.get(l) {
                codigo_quebravel(ui, r, ui.available_width());
            }
        });
    });
}

/// A Estrutura como parte da tela de tabela: a faixa "Colunas", a busca da barra ("Buscar colunas", que filtra a
/// lista na hora), a Nova coluna e o Excluir da coluna selecionada na barra, e as Propriedades.
impl estado::ParteTabela for AbaEstrutura {
    fn anatomia(&self) -> estado::Anatomia {
        estado::Anatomia::default().com_faixa()
    }

    /// "Colunas · 7 colunas" e, na ponta direita, a Nova coluna (o `.ib.pq` da `.sec` de `Dados.dc.html`), com a
    /// estrutura editável.
    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let n = tabela.info().colunas.len();
        titulo_da_faixa(ui, "Colunas", &plural(n, "coluna", "colunas"));
        if !pode_editar(tabela.info(), ctx.estado.somente_leitura()) {
            return;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let botao = Botao::icone(Icone::NovaColuna).pequeno().rotulo("Nova coluna").dica("Nova coluna");
            if ui.add(botao).clicked() {
                let egui = ui.ctx().clone();
                self.janela_coluna = Some(JanelaColuna::nova(&egui, ctx, tabela, None));
                egui.request_repaint();
            }
        });
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.conteudo_da_parte(ui, ctx, tabela);
    }

    /// D2 (c): a Nova coluna liga e o Novo fica desligado; o Excluir é o da coluna selecionada ("Excluir a coluna
    /// email"), desligado na chave primária e na tabela de uma coluna só.
    fn ofertas(&self, tabela: &TabelaAberta, estado: &estado::Estado) -> Ofertas {
        let editavel = pode_editar(tabela.info(), estado.somente_leitura());
        let livre = editavel && self.tarefa.is_none();
        let excluir = match self.coluna_selecionada(tabela) {
            Some(col) => Oferta::ligada()
                .ligada_se(livre && AbaEstrutura::pode_excluir(tabela, col))
                .com_dica(format!("Excluir a coluna {}", col.nome)),
            None => Oferta::default(),
        };
        Ofertas::default()
            .com(Verbo::NovaColuna, Oferta::ligada().ligada_se(editavel))
            .com(Verbo::Excluir, excluir)
            .ligar(Verbo::Buscar)
    }

    fn executar(&mut self, verbo: Verbo, egui: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let editavel = pode_editar(tabela.info(), ctx.estado.somente_leitura());
        match verbo {
            Verbo::NovaColuna if editavel => {
                self.janela_coluna = Some(JanelaColuna::nova(egui, ctx, tabela, None));
            }
            Verbo::Excluir if editavel => {
                if let Some(col) = self.coluna_selecionada(tabela).filter(|c| AbaEstrutura::pode_excluir(tabela, c)) {
                    let nome = col.nome.clone();
                    self.pedir(egui, ctx, tabela, AcaoLinha::Excluir(nome), editavel);
                }
            }
            _ => {}
        }
    }

    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        Some(estado::BuscaDaBarra { texto: &mut self.busca, dica: DICA_BUSCA_COLUNAS.into() })
    }

    /// "Renomear a tabela", no fim da seção Tabela.
    fn acoes_da_tabela(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &TabelaAberta) {
        if !pode_editar(tabela.info(), ctx.estado.somente_leitura()) {
            return;
        }
        acoes_da_secao(ui, |ui| {
            if ui.add(Botao::new("Renomear a tabela").lista().com_icone(Icone::Editar)).clicked() {
                self.janela_renomear = Some(JanelaRenomear::nova(&tabela.nome, icone_da_tabela(tabela)));
                ui.ctx().request_repaint();
            }
        });
    }

    /// A coluna selecionada e a zona "Excluir a tabela".
    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let editavel = pode_editar(tabela.info(), ctx.estado.somente_leitura());
        self.secao_coluna(ui, ctx, tabela, editavel);
        if editavel {
            self.secao_excluir_tabela(ui, ctx, tabela);
        }
    }

    fn ocupada(&self) -> bool {
        self.tarefa.is_some() || self.excluindo_tabela.is_some()
    }

    /// O menu Tabela: a Nova coluna e o Renomear abrem as janelas deles; o Excluir abre a zona "Excluir a tabela"
    /// (recolhida nas Propriedades), que pede o nome digitado.
    fn pedido(&mut self, pedido: PedidoTabela, egui: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        if !pode_editar(tabela.info(), ctx.estado.somente_leitura()) {
            return;
        }
        match pedido {
            PedidoTabela::NovaColuna => self.janela_coluna = Some(JanelaColuna::nova(egui, ctx, tabela, None)),
            PedidoTabela::Renomear => {
                self.janela_renomear = Some(JanelaRenomear::nova(&tabela.nome, icone_da_tabela(tabela)));
            }
            PedidoTabela::Excluir => Secao::abrir(egui, ("propriedades::excluir_tabela", self.instancia)),
        }
        egui.request_repaint();
    }

    fn coluna_escolhida(&self) -> Option<&str> {
        self.selecionada.as_deref()
    }
}

// ------------------------------------------------------------------ janela de coluna

const AJUDA_TIPO: &str = "Escolha da lista ou digite outro. O SQLite aceita qualquer nome de tipo; o que vale é a \
                          afinidade (INTEGER, TEXT, REAL, NUMERIC ou BLOB) que ele deduz do nome.";
const AJUDA_PADRAO: &str = "Expressão SQL: 0, 'texto' entre aspas simples, CURRENT_TIMESTAMP…";
const AJUDA_VERIFICACAO: &str = "Condição que todo valor precisa cumprir, como preco >= 0. Vazio: sem verificação.";
const AJUDA_COLACAO: &str = "Como os textos são comparados: NOCASE ignora maiúsculas, RTRIM ignora espaços no fim, \
                             BINARY compara byte a byte.";
const AJUDA_EXPRESSAO: &str = "A coluna é gerada: o valor vem desta expressão e não é digitado.";
const AJUDA_REFERENCIA: &str = "Chave estrangeira: cada valor precisa existir na tabela escolhida.";
const AJUDA_COLUNA_REF: &str = "Sem escolher, a referência aponta para a chave primária da outra tabela.";
const AJUDA_EXTRA: &str = "Trechos da definição que o editor não separa em campos. Ficam como estão, a menos que você \
                           os mude.";
const AJUDA_AUTO: &str = "Só vale numa chave primária do tipo INTEGER: o número de cada linha nova nunca repete um que \
                          já foi usado, nem de linhas excluídas.";

/// Os campos da janela de coluna.
pub(crate) struct CamposColuna {
    id: Id,
    tabela: String,
    /// O nome da coluna sendo alterada (None: nova coluna).
    nome_atual: Option<String>,
    colunas_tabela: Vec<String>,
    pub(crate) nome: String,
    pub(crate) tipo: String,
    pub(crate) padrao: String,
    pub(crate) verificacao: String,
    colacoes: Vec<String>,
    pub(crate) colacao: usize,
    tabelas_ref: Vec<String>,
    pub(crate) ref_tabela: usize,
    pub(crate) colunas_ref: Vec<String>,
    pub(crate) ref_coluna: usize,
    /// A coluna referenciada escolhida (o `colunaPedida`): continua escolhida enquanto a lista carrega.
    coluna_pedida: String,
    carga_ref: Option<(String, String, Promessa<Vec<String>>)>,
    acoes_excluir: Vec<String>,
    pub(crate) ref_excluir: usize,
    acoes_atualizar: Vec<String>,
    pub(crate) ref_atualizar: usize,
    complemento: Option<String>,
    pub(crate) nao_nulo: bool,
    pub(crate) unica: bool,
    pub(crate) pk: bool,
    pub(crate) auto: bool,
    gerada: Option<(String, bool)>,
    colacao_gerada: Option<String>,
    extra: Option<String>,
}

/// A janela de coluna: nova ou alterar uma coluna existente.
pub(crate) struct JanelaColuna {
    titulo: String,
    /// O sólido da tabela no cabeçalho (a ficha anil da tabela sem rowid, D6).
    icone: Icone,
    pode_excluir: bool,
    pub(crate) campos: CamposColuna,
    pub(crate) mudanca: Mudanca,
    confirmacao: Option<Confirmacao>,
    excluindo: Option<Promessa<RespostaEsquema>>,
}

fn rotuladas<'a>(valores: &'a [String], vazio: &'a str) -> Vec<(usize, &'a str)> {
    valores.iter().enumerate().map(|(i, v)| (i, if v.is_empty() { vazio } else { v.as_str() })).collect()
}

impl JanelaColuna {
    /// Abre a janela para `nome_atual` (alterar, pré-preenchida pela ColunaDef) ou para uma coluna nova.
    pub(crate) fn nova(
        egui: &egui::Context,
        ctx: &Contexto<'_>,
        tabela: &TabelaAberta,
        nome_atual: Option<&str>,
    ) -> JanelaColuna {
        let info = tabela.info();
        let pragma = nome_atual.and_then(|n| info.colunas.iter().find(|c| c.nome == n));
        let def = match nome_atual {
            Some(n) => definicao_de(&tabela.completa, n).cloned().unwrap_or_else(|| ColunaDef {
                nome: n.to_string(),
                tipo: pragma.map(|p| p.tipo.clone()).unwrap_or_default(),
                nao_nulo: pragma.is_some_and(|p| p.nao_nulo),
                unica: pragma.is_some_and(|p| p.unica),
                pk: pragma.is_some_and(|p| p.pk > 0),
                autoincremento: pragma.is_some_and(|p| p.autoincremento),
                padrao: pragma.and_then(|p| p.padrao.clone()),
                ..ColunaDef::default()
            }),
            None => ColunaDef { tipo: "TEXT".into(), ..ColunaDef::default() },
        };
        let id = Id::new(("tela_esquema::coluna", &tabela.nome, nome_atual));
        let referencia = def.referencia.clone();
        let (colacoes, colacao) = opcoes_select(&COLACOES, def.colacao.as_deref(), true);
        let nomes_tabelas: Vec<&str> = ctx
            .estado
            .objetos
            .as_ref()
            .map(|o| o.tabelas.iter().filter(|t| t.tipo == banco::TipoObjeto::Table).map(|t| t.nome.as_str()).collect())
            .unwrap_or_default();
        let (tabelas_ref, ref_tabela) =
            opcoes_select(&nomes_tabelas, referencia.as_ref().map(|r| r.tabela.as_str()), true);
        let coluna_pedida = referencia.as_ref().and_then(|r| r.coluna.clone()).unwrap_or_default();
        let mut colunas_ref = vec![String::new()];
        if !coluna_pedida.is_empty() {
            colunas_ref.push(coluna_pedida.clone());
        }
        let ref_coluna = colunas_ref.len() - 1;
        let (acoes_excluir, ref_excluir) =
            opcoes_select(&ACOES_FK, referencia.as_ref().and_then(|r| r.ao_excluir.as_deref()), true);
        let (acoes_atualizar, ref_atualizar) =
            opcoes_select(&ACOES_FK, referencia.as_ref().and_then(|r| r.ao_atualizar.as_deref()), true);
        let titulo = match nome_atual {
            Some(n) => format!("Alterar a coluna {n}"),
            None => "Nova coluna".to_string(),
        };
        let pode_excluir = nome_atual.is_some() && !pragma.is_some_and(|p| p.pk > 0) && info.colunas.len() > 1;
        let mut campos = CamposColuna {
            id,
            tabela: tabela.nome.clone(),
            nome_atual: nome_atual.map(str::to_string),
            colunas_tabela: info.colunas.iter().map(|c| c.nome.clone()).collect(),
            nome: def.nome.clone(),
            tipo: def.tipo.clone(),
            padrao: def.padrao.clone().unwrap_or_default(),
            verificacao: def.verificacao.clone().unwrap_or_default(),
            colacoes,
            colacao,
            tabelas_ref,
            ref_tabela,
            colunas_ref,
            ref_coluna,
            coluna_pedida,
            carga_ref: None,
            acoes_excluir,
            ref_excluir,
            acoes_atualizar,
            ref_atualizar,
            complemento: referencia.and_then(|r| r.complemento),
            nao_nulo: def.nao_nulo,
            unica: def.unica,
            pk: def.pk,
            auto: def.autoincremento,
            gerada: def.gerada.as_ref().map(|g| (g.expressao.clone(), g.armazenada)),
            colacao_gerada: def.colacao.clone(),
            extra: def.extra.clone().filter(|e| !e.is_empty()),
        };
        campos.atualizar_auto();
        if campos.ref_tabela != 0 {
            campos.carregar_colunas_ref(egui, ctx);
        }
        JanelaColuna {
            titulo,
            icone: icone_da_tabela(tabela),
            pode_excluir,
            campos,
            mudanca: Mudanca::nova(id, "Digite o nome da coluna para ver o SQL."),
            confirmacao: None,
            excluindo: None,
        }
    }

    pub(crate) fn mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto<'_>) -> FimJanela {
        let JanelaColuna { titulo, icone, pode_excluir, campos, mudanca, confirmacao, excluindo } = self;
        if campos.receber_colunas_ref() {
            mudanca.agendar(egui);
        }
        if let Some(r) = excluindo.as_mut().and_then(Promessa::tomar) {
            *excluindo = None;
            match r {
                Ok(r) => {
                    toast_da_resposta(egui, ctx, &r);
                    return FimJanela::Concluida;
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        mudanca.avisar(e.mensagem, true);
                    }
                }
            }
        }
        if mudanca.passo(egui, ctx, || campos.montar()).is_some() {
            return FimJanela::Concluida;
        }

        let ocupado = mudanca.ocupada() || excluindo.is_some();
        let mut aplicar = false;
        let mut excluir = false;
        let mut ref_mudou = false;
        let pode_excluir = *pode_excluir;
        let mut cancelar = false;
        let tabela = campos.tabela.clone();
        let r = Janela::new(("tela_esquema::janela_coluna", campos.id), titulo.as_str())
            .icone(*icone)
            .sub(&tabela)
            .rodape_esquerda(|ui| {
                if pode_excluir && ui.add(Botao::ghost_perigo("Excluir coluna").habilitado(!ocupado)).clicked() {
                    excluir = true;
                }
            })
            .rodape_direita(|ui| {
                cancelar = botao_cancelar(ui, ocupado);
                if ui.add(Botao::primario("Aplicar").habilitado(!ocupado)).clicked() {
                    aplicar = true;
                }
            })
            .mostrar(egui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                mudanca.desenhar_aviso(ui);
                let (mudou, mudou_ref) = campos.desenhar(ui);
                ref_mudou = mudou_ref;
                mudanca.desenhar_previa(ui);
                mudou
            });

        if ref_mudou {
            campos.coluna_pedida.clear();
            campos.carregar_colunas_ref(egui, ctx);
        }
        if r.inner {
            mudanca.agendar(egui);
        }
        if excluir && !ocupado {
            let frase = frase_excluir_coluna(campos.nome_atual.as_deref().unwrap_or(""), &campos.tabela);
            *confirmacao = Some(Confirmacao::new(frase).titulo("Excluir coluna").rotulo("Excluir").perigo(true));
        }
        if let Some(true) = confirmar(egui, confirmacao)
            && excluindo.is_none()
            && let Some(atual) = campos.nome_atual.clone()
        {
            mudanca.aviso = None;
            let op = OperacaoEsquema::ExcluirColuna { tabela: campos.tabela.clone(), coluna: atual };
            *excluindo = Some(ctx.tarefa(egui, move |servico, id| servico.esquema(id, &op, false)));
        }
        if (aplicar || r.enviou) && !ocupado {
            mudanca.aplicar(egui, ctx, campos.montar());
        }
        if (r.pediu_fechar || cancelar) && !ocupado && confirmacao.is_none() {
            return FimJanela::Fechada;
        }
        FimJanela::Aberta
    }
}

impl CamposColuna {
    /// Autoincremento só com INTEGER e chave primária (o `atualizarAuto`). Devolve se pode.
    fn atualizar_auto(&mut self) -> bool {
        let pode = self.tipo.trim().eq_ignore_ascii_case("INTEGER") && self.pk;
        if !pode {
            self.auto = false;
        }
        pode
    }

    fn tabela_ref(&self) -> &str {
        self.tabelas_ref.get(self.ref_tabela).map_or("", String::as_str)
    }

    #[cfg(test)]
    pub(crate) fn ref_tabela_nome(&self) -> &str {
        self.tabela_ref()
    }

    #[cfg(test)]
    pub(crate) fn ref_coluna_nome(&self) -> &str {
        self.colunas_ref.get(self.ref_coluna).map_or("", String::as_str)
    }

    #[cfg(test)]
    pub(crate) fn ref_excluir_nome(&self) -> &str {
        self.acoes_excluir.get(self.ref_excluir).map_or("", String::as_str)
    }

    /// Carrega as colunas da tabela referenciada (o `carregarColunasRef`): enquanto a lista não chega, ficam
    /// "A chave primária" e a coluna já escolhida.
    fn carregar_colunas_ref(&mut self, egui: &egui::Context, ctx: &Contexto<'_>) {
        let t = self.tabela_ref().to_string();
        self.carga_ref = None;
        if t.is_empty() {
            return;
        }
        let pedida = self.coluna_pedida.clone();
        self.colunas_ref = vec![String::new()];
        if !pedida.is_empty() {
            self.colunas_ref.push(pedida.clone());
        }
        self.ref_coluna = self.colunas_ref.len() - 1;
        if t == self.tabela {
            let nomes = self.colunas_tabela.clone();
            self.aplicar_colunas_ref(&pedida, nomes);
            return;
        }
        let nome = t.clone();
        let promessa = ctx.tarefa(egui, move |servico, id| {
            servico.tabela(id, &nome).map(|c| c.info.colunas.into_iter().map(|c| c.nome).collect())
        });
        self.carga_ref = Some((t, pedida, promessa));
    }

    /// Recebe a lista de colunas da tabela referenciada; devolve se ela chegou (para agendar a prévia).
    fn receber_colunas_ref(&mut self) -> bool {
        let Some((t, pedida, p)) = &mut self.carga_ref else { return false };
        let Some(r) = p.tomar() else { return false };
        let (t, pedida) = (std::mem::take(t), std::mem::take(pedida));
        self.carga_ref = None;
        if self.tabela_ref() != t {
            return false;
        }
        // sem a lista, fica a coluna que já estava escolhida
        self.aplicar_colunas_ref(&pedida, r.unwrap_or_default());
        true
    }

    fn aplicar_colunas_ref(&mut self, pedida: &str, mut nomes: Vec<String>) {
        if !pedida.is_empty() && !nomes.iter().any(|n| n.to_lowercase() == pedida.to_lowercase()) {
            nomes.push(pedida.to_string());
        }
        self.colunas_ref = std::iter::once(String::new()).chain(nomes).collect();
        self.ref_coluna = if pedida.is_empty() {
            0
        } else {
            self.colunas_ref
                .iter()
                .position(|n| !n.is_empty() && n.to_lowercase() == pedida.to_lowercase())
                .unwrap_or(0)
        };
    }

    /// O pedido (o `montar()` da janela de coluna).
    pub(crate) fn montar(&self) -> Montagem {
        let n = self.nome.trim();
        if n.is_empty() {
            return Montagem::Falta;
        }
        let ref_t = self.tabela_ref();
        let valor = |v: &[String], i: usize| v.get(i).filter(|s| !s.is_empty()).cloned();
        let referencia = (!ref_t.is_empty()).then(|| Referencia {
            tabela: ref_t.to_string(),
            coluna: valor(&self.colunas_ref, self.ref_coluna),
            ao_excluir: valor(&self.acoes_excluir, self.ref_excluir),
            ao_atualizar: valor(&self.acoes_atualizar, self.ref_atualizar),
            complemento: self.complemento.clone(),
        });
        let gerada = self.gerada.is_some();
        let nao_vazio = |s: &str| Some(s.trim()).filter(|s| !s.is_empty()).map(str::to_string);
        let pode_auto = self.tipo.trim().eq_ignore_ascii_case("INTEGER") && self.pk;
        let coluna = ColunaDef {
            nome: n.to_string(),
            tipo: self.tipo.trim().to_string(),
            nao_nulo: self.nao_nulo,
            unica: self.unica,
            pk: self.pk,
            autoincremento: self.auto && pode_auto,
            padrao: if gerada { None } else { nao_vazio(&self.padrao) },
            verificacao: nao_vazio(&self.verificacao),
            colacao: if gerada { self.colacao_gerada.clone() } else { valor(&self.colacoes, self.colacao) },
            referencia,
            gerada: self.gerada.as_ref().map(|(expressao, armazenada)| Gerada {
                expressao: expressao.trim().to_string(),
                armazenada: *armazenada,
            }),
            extra: self.extra.as_deref().and_then(nao_vazio),
        };
        Montagem::pedido(match &self.nome_atual {
            Some(atual) => {
                OperacaoEsquema::AlterarColuna { tabela: self.tabela.clone(), coluna: atual.clone(), nova: coluna }
            }
            None => OperacaoEsquema::AdicionarColuna { tabela: self.tabela.clone(), coluna },
        })
    }

    /// Desenha o `.campos` da janela; devolve (algo mudou, a tabela da referência mudou).
    fn desenhar(&mut self, ui: &mut Ui) -> (bool, bool) {
        let mudou = Cell::new(false);
        let ref_mudou = Cell::new(false);
        let marcar = |r: &egui::Response| {
            if r.changed() {
                mudou.set(true);
            }
        };
        let id = self.id;
        let gerada = self.gerada.is_some();
        let CamposColuna {
            nome,
            tipo,
            padrao,
            verificacao,
            colacoes,
            colacao,
            tabelas_ref,
            ref_tabela,
            colunas_ref,
            ref_coluna,
            coluna_pedida,
            acoes_excluir,
            ref_excluir,
            acoes_atualizar,
            ref_atualizar,
            nao_nulo,
            unica,
            pk,
            auto,
            gerada: campo_gerada,
            extra,
            ..
        } = self;
        campos_duas_colunas(ui, |c| {
            c.par(
                |ui| {
                    campo(ui, "Nome", None, |ui| {
                        marcar(&ui.add(Entrada::new(nome).rotulo("Nome").id_salt(id.with("nome"))))
                    });
                },
                |ui| {
                    campo_com_dica(ui, "Tipo", AJUDA_TIPO, |ui| {
                        // a pastilha de 16px acompanha o tipo digitado (a decisão 17, só pelo texto do tipo)
                        let pastilha = TipoColuna::do_declarado(tipo, false, false);
                        marcar(
                            &EntradaSugestoes::new(id.with("tipo"), tipo, &TIPOS)
                                .pastilha(pastilha)
                                .monoespacada(true)
                                .rotulo("Tipo")
                                .mostrar(ui),
                        );
                    });
                },
            );
            if !gerada {
                c.par(
                    |ui| {
                        campo_com_dica(ui, "Valor padrão", AJUDA_PADRAO, |ui| {
                            marcar(
                                &ui.add(Entrada::new(padrao).monoespacada(true).dica("nenhum").rotulo("Valor padrão")),
                            );
                        });
                    },
                    |ui| campo_verificacao(ui, verificacao, &marcar),
                );
                c.par(
                    |ui| {
                        campo_com_dica(ui, ROTULO_ORDENACAO, AJUDA_COLACAO, |ui| {
                            let opcoes = rotuladas(colacoes, "nenhuma");
                            marcar(
                                &Select::new(id.with("colacao"), colacao, &opcoes).rotulo(ROTULO_ORDENACAO).mostrar(ui),
                            );
                        });
                    },
                    |ui| campo_referencia(ui, id, tabelas_ref, ref_tabela, &marcar, &ref_mudou),
                );
            } else {
                c.largo(|ui| campo_verificacao(ui, verificacao, &marcar));
                if let Some((expressao, armazenada)) = campo_gerada {
                    c.largo(|ui| {
                        campo_com_dica(ui, "Expressão calculada", AJUDA_EXPRESSAO, |ui| {
                            marcar(&ui.add(Entrada::new(expressao).monoespacada(true).rotulo("Expressão calculada")));
                        });
                    });
                    c.largo(|ui| marcar(&Check::new(armazenada, "Armazenada (STORED)").mostrar(ui)));
                }
            }
            if gerada {
                c.largo(|ui| campo_referencia(ui, id, tabelas_ref, ref_tabela, &marcar, &ref_mudou));
            }
            if *ref_tabela != 0 {
                c.largo(|ui| {
                    painel_rebaixado(ui, Margin::same(12), |ui| {
                        campos_duas_colunas(ui, |c| {
                            c.largo(|ui| {
                                campo_com_dica(ui, "Coluna referenciada", AJUDA_COLUNA_REF, |ui| {
                                    let opcoes = rotuladas(colunas_ref, "A chave primária");
                                    let r = Select::new(id.with("ref_coluna"), ref_coluna, &opcoes)
                                        .rotulo("Coluna referenciada")
                                        .mostrar(ui);
                                    if r.changed() {
                                        *coluna_pedida = colunas_ref.get(*ref_coluna).cloned().unwrap_or_default();
                                    }
                                    marcar(&r);
                                });
                            });
                            c.par(
                                |ui| {
                                    campo(ui, "Ao excluir", None, |ui| {
                                        let opcoes = rotuladas(acoes_excluir, "Padrão (NO ACTION)");
                                        marcar(
                                            &Select::new(id.with("ao_excluir"), ref_excluir, &opcoes)
                                                .rotulo("Ao excluir")
                                                .mostrar(ui),
                                        );
                                    });
                                },
                                |ui| {
                                    campo(ui, "Ao atualizar", None, |ui| {
                                        let opcoes = rotuladas(acoes_atualizar, "Padrão (NO ACTION)");
                                        marcar(
                                            &Select::new(id.with("ao_atualizar"), ref_atualizar, &opcoes)
                                                .rotulo("Ao atualizar")
                                                .mostrar(ui),
                                        );
                                    });
                                },
                            );
                        });
                    });
                });
            }
            if let Some(extra) = extra {
                c.largo(|ui| {
                    campo_com_dica(ui, "Outras restrições", AJUDA_EXTRA, |ui| {
                        marcar(&ui.add(Entrada::new(extra).monoespacada(true).rotulo("Outras restrições")));
                    });
                });
            }
            c.largo(|ui| {
                // `.checks-coluna`: flex que quebra linha, `gap:10px 18px`; 16px depois dos campos (14px da grade e 2px)
                ui.add_space(2.0);
                let larguras = [
                    largura_check(ui, "Não aceita nulo", 12.0),
                    largura_check(ui, "Único", 12.0),
                    largura_check(ui, "Chave primária", 12.0),
                    largura_check(ui, "Autoincremento", 12.0) + 6.0 + 14.0,
                ];
                fluxo_medido(ui, &larguras, 19.0, 18.0, 10.0, |ui, i| match i {
                    0 => marcar(&Check::new(nao_nulo, "Não aceita nulo").mostrar(ui)),
                    1 => marcar(&Check::new(unica, "Único").mostrar(ui)),
                    2 => marcar(&Check::new(pk, "Chave primária").mostrar(ui)),
                    _ => {
                        let pode = tipo.trim().eq_ignore_ascii_case("INTEGER") && *pk;
                        if !pode {
                            *auto = false;
                        }
                        ui.add_enabled_ui(pode, |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            // a explicação vai na dica (a prancheta 12b não tem o "i")
                            let r = Check::new(auto, "Autoincremento").mostrar(ui);
                            r.clone().on_hover_text(AJUDA_AUTO).on_disabled_hover_text(AJUDA_AUTO);
                            marcar(&r);
                        });
                    }
                });
            });
        });
        (mudou.get(), ref_mudou.get())
    }
}

/// O rótulo do Select de colação (a prancheta 12b: "Ordenação (COLLATE)", item 21).
const ROTULO_ORDENACAO: &str = "Ordenação (COLLATE)";

/// O campo Referência: a tabela que a coluna referencia ("nenhuma" sem chave estrangeira, como a prancheta 12b).
fn campo_referencia(
    ui: &mut Ui,
    id: Id,
    tabelas_ref: &[String],
    ref_tabela: &mut usize,
    marcar: &impl Fn(&egui::Response),
    ref_mudou: &Cell<bool>,
) {
    campo_com_dica(ui, "Referência", AJUDA_REFERENCIA, |ui| {
        let opcoes = rotuladas(tabelas_ref, "nenhuma");
        let r = Select::new(id.with("ref_tabela"), ref_tabela, &opcoes).rotulo("Referência").mostrar(ui);
        if r.changed() {
            ref_mudou.set(true);
        }
        marcar(&r);
    });
}

fn campo_verificacao(ui: &mut Ui, verificacao: &mut String, marcar: &impl Fn(&egui::Response)) {
    campo_com_dica(ui, "Verificação (CHECK)", AJUDA_VERIFICACAO, |ui| {
        marcar(&ui.add(Entrada::new(verificacao).monoespacada(true).dica("nenhuma").rotulo("Verificação (CHECK)")));
    });
}

// ------------------------------------------------------------------ janela de renomear

/// A janela "Renomear tabela".
pub(crate) struct JanelaRenomear {
    tabela: String,
    /// O sólido da tabela no cabeçalho (a ficha anil da tabela sem rowid, D6).
    icone: Icone,
    pub(crate) novo: String,
    pub(crate) mudanca: Mudanca,
    id_campo: Id,
    primeiro_quadro: bool,
}

impl JanelaRenomear {
    pub(crate) fn nova(tabela: &str, icone: Icone) -> JanelaRenomear {
        let id = Id::new(("tela_esquema::renomear", tabela));
        JanelaRenomear {
            tabela: tabela.to_string(),
            icone,
            novo: tabela.to_string(),
            mudanca: Mudanca::nova(id, "Digite o nome novo da tabela para ver o SQL."),
            id_campo: id.with("novo_nome"),
            primeiro_quadro: true,
        }
    }

    pub(crate) fn mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto<'_>) -> FimJanela {
        let JanelaRenomear { tabela, icone, novo, mudanca, id_campo, primeiro_quadro } = self;
        let montar = |novo: &str| montar_renomear(tabela, novo);
        if mudanca.passo(egui, ctx, || montar(novo)).is_some() {
            return FimJanela::Concluida;
        }
        // o `TextEdit` da `Entrada` tem o id `Id::new(id_salt)`
        let id_texto = Id::new(*id_campo);
        if *primeiro_quadro && egui.memory(|m| m.has_focus(id_texto)) {
            // `entrada.select()`: o nome atual nasce selecionado (no quadro em que o campo já tem o foco: sem
            // ele, o egui desfaz a seleção)
            *primeiro_quadro = false;
            let mut estado = TextEditState::load(egui, id_texto).unwrap_or_default();
            estado.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(novo.chars().count()))));
            estado.store(egui, id_texto);
        }
        let ocupado = mudanca.ocupada();
        let mut aplicar = false;
        let mut cancelar = false;
        let id = *id_campo;
        let r = Janela::new(("tela_esquema::renomear", tabela.as_str()), "Renomear tabela")
            .icone(*icone)
            .sub(tabela.as_str())
            .rodape_direita(|ui| {
                cancelar = botao_cancelar(ui, ocupado);
                if ui.add(Botao::primario("Renomear").habilitado(!ocupado)).clicked() {
                    aplicar = true;
                }
            })
            .mostrar(egui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                mudanca.desenhar_aviso(ui);
                let mut mudou = false;
                campos_duas_colunas(ui, |c| {
                    c.largo(|ui| {
                        campo(
                            ui,
                            "Novo nome",
                            Some("Índices, gatilhos e visões que citam a tabela passam a usar o nome novo."),
                            |ui| {
                                mudou = ui.add(Entrada::new(novo).rotulo("Novo nome").id_salt(id)).changed();
                            },
                        );
                    });
                });
                mudanca.desenhar_previa(ui);
                mudou
            });
        if r.inner {
            mudanca.agendar(egui);
        }
        if (aplicar || r.enviou) && !ocupado {
            mudanca.aplicar(egui, ctx, montar(novo));
        }
        if (r.pediu_fechar || cancelar) && !ocupado {
            return FimJanela::Fechada;
        }
        FimJanela::Aberta
    }
}

/// O "Cancelar" (`Botao::ghost`) do rodapé das janelas de edição, antes do primário: fecha como o ✕ e o Esc.
pub(crate) fn botao_cancelar(ui: &mut Ui, ocupado: bool) -> bool {
    ui.add(Botao::ghost("Cancelar").habilitado(!ocupado)).clicked()
}

/// O pedido de "Renomear tabela": nada enquanto o nome novo está vazio ou igual ao atual.
fn montar_renomear(tabela: &str, novo: &str) -> Montagem {
    let n = novo.trim();
    if n.is_empty() || n == tabela {
        return Montagem::Falta;
    }
    Montagem::pedido(OperacaoEsquema::RenomearTabela { tabela: tabela.to_string(), novo_nome: n.to_string() })
}
