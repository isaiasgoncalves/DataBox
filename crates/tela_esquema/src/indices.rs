//! Aba Índices e gatilhos: a faixa "Índices · 3 índices"; a
//! lista dos índices de ponta a ponta (os automáticos, de chave primária e de UNIQUE, sem Excluir), com a seleção da
//! linha e o Excluir em ícone; os gatilhos depois, com o SQL nas cores do código, Copiar e Excluir; e a janela "Novo
//! índice" com a prévia do SQL (que a aba Dados também abre, pela casca, com a coluna da ordenação lenta já
//! marcada). Na barra, o Novo índice e o Excluir do índice selecionado; nas Propriedades, o "Índice" da linha
//! selecionada. A busca da barra fica desligada.

use banco::{IndiceInfo, OrigemIndice};
use egui::{Align, Id, Layout, Margin, Rect, Sense, Ui, UiBuilder, pos2, vec2};
use estado::{Contexto, Oferta, Ofertas, Promessa, Reacao, TabelaAberta, Verbo};
use interface::fontes::{self, Peso};
use interface::formato::plural;
use interface::icones::Icone;
use interface::pecas::ajuda::ajuda_mini;
use interface::pecas::arranjo::em_linha;
use interface::pecas::avisos::{TomAviso, aviso_texto, aviso_tom, carregando, vazio};
use interface::pecas::botao::{Botao, TipoBotao};
use interface::pecas::campos::{Check, Entrada, Select, campo, campos_duas_colunas, check, rotulo_campo};
use interface::pecas::documento::{RECUO_PAGINA, de_ponta_a_ponta};
use interface::pecas::janela::{Confirmacao, Janela, confirmar};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::rotulos::{TipoPill, pill};
use interface::pecas::tabela::{ColunaTabela, Tabela};
use interface::pecas::texto::{Trecho, codigo, forte, galley, galley_cortado, hint};
use interface::pecas::texto_pre::TextoPre;
use interface::tema::paleta;
use servico::{ColunaIndicePedida, OperacaoEsquema, RespostaEsquema};

use crate::apoio::{
    assinatura, codigo_quebravel, modelo_forte, motivo_sem_edicao, nome_sugerido, nova_instancia, painel_rebaixado,
    pode_editar, sql_gatilho, titulo_da_faixa, titulo_lista, titulo_lista_com_contagem,
};
use crate::estrutura::{FimJanela, pedir_recarga, toast_da_resposta};
use crate::mudanca::{Montagem, Mudanca};

/// Onde mostrar o erro de uma exclusão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card {
    Indices,
    Gatilhos,
}

/// Uma ação pedida na lista de índices ou na dos gatilhos.
enum AcaoIndice {
    /// O clique na linha de um índice: seleciona.
    Selecionar(String),
    ExcluirIndice(String),
    ExcluirGatilho(String),
}

/// A aba Índices e gatilhos de uma tabela.
pub struct AbaIndices {
    janela: Option<JanelaNovoIndice>,
    confirmacao: Option<Confirmacao>,
    pendente: Option<(Card, OperacaoEsquema)>,
    tarefa: Option<(Card, Promessa<RespostaEsquema>)>,
    erro_indices: Option<String>,
    erro_gatilhos: Option<String>,
    /// O erro de uma das listas acabou de chegar: a página rola até ele no próximo desenho.
    rolar_erro_indices: bool,
    rolar_erro_gatilhos: bool,
    /// O número desta aba (os modelos dos gatilhos nascem fechados em cada aba nova).
    instancia: u64,
    /// O SQL de cada gatilho como a lista mostra (refeito só quando os gatilhos mudam).
    sql_gatilhos: Vec<String>,
    assinatura_gatilhos: u64,
    /// O índice selecionado (o `tr.sel`), pelo nome: o das Propriedades e do Excluir da barra.
    selecionado: Option<String>,
}

/// "nome, cidade" / "status, feito_em DESC" (o `textoColunas`).
fn texto_colunas(ind: &IndiceInfo) -> String {
    let partes: Vec<String> =
        ind.colunas.iter().map(|c| if c.desc { format!("{} DESC", c.nome) } else { c.nome.clone() }).collect();
    partes.join(", ")
}

/// O texto e o tom da etiqueta de origem de um índice.
fn origem_do_indice(origem: OrigemIndice) -> (&'static str, TipoPill) {
    match origem {
        OrigemIndice::Pk => ("da chave primária", TipoPill::Mute),
        OrigemIndice::U => ("de restrição UNIQUE", TipoPill::Mute),
        OrigemIndice::C => ("criado", TipoPill::Neutra),
    }
}

/// A condição de um índice parcial (o `WHERE`), quando o SQL dele a deixa ler.
fn condicao_do_indice(ind: &IndiceInfo) -> Option<String> {
    if ind.parcial { ind.sql.as_deref().and_then(crate::apoio::condicao_indice) } else { None }
}

/// O índice se exclui (só os criados, com o SQL à mão).
fn pode_excluir_indice(ind: &IndiceInfo) -> bool {
    ind.origem == OrigemIndice::C && ind.sql.is_some()
}

/// O rótulo e a dica do Excluir de cada linha de índice.
pub(crate) const ROTULO_EXCLUIR_INDICE: &str = "Excluir o índice";
/// O espaço entre as partes do documento (`gap:18px`, o da página de `Main.dc.html`).
const ESPACO_LISTAS: f32 = 18.0;
/// O recuo do texto da faixa, da lista e dos Gatilhos: 15px da borda (o `RECUO_ESQUERDA` de 14 da faixa e a moldura).
const RECUO_LISTA: f32 = 15.0;
/// A largura da coluna do Excluir da lista.
const LARGURA_ACOES: f32 = 56.0;

impl AbaIndices {
    pub fn nova(_tabela: &TabelaAberta) -> AbaIndices {
        AbaIndices {
            janela: None,
            confirmacao: None,
            pendente: None,
            tarefa: None,
            erro_indices: None,
            erro_gatilhos: None,
            rolar_erro_indices: false,
            rolar_erro_gatilhos: false,
            instancia: nova_instancia(),
            sql_gatilhos: Vec::new(),
            assinatura_gatilhos: 0,
            selecionado: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        estado::desenhar_parte_empilhada(self, ui, ctx, tabela);
    }

    /// O índice selecionado, se ele ainda existe na tabela.
    fn indice_selecionado<'t>(&self, tabela: &'t TabelaAberta) -> Option<&'t IndiceInfo> {
        let nome = self.selecionado.as_deref()?;
        tabela.info().indices.iter().find(|i| i.nome == nome)
    }

    /// A confirmação de excluir um índice criado.
    fn pedir_excluir_indice(&mut self, nome: String) {
        let frase = format!(
            "Excluir o índice {nome}? Os dados não mudam, mas as buscas que usavam o índice podem ficar mais lentas."
        );
        self.pendente = Some((Card::Indices, OperacaoEsquema::ExcluirIndice { nome }));
        self.confirmacao = Some(Confirmacao::new(frase).titulo("Excluir índice").rotulo("Excluir").perigo(true));
    }

    /// A confirmação de excluir um gatilho.
    fn pedir_excluir_gatilho(&mut self, nome: String) {
        let frase = format!(
            "Excluir o gatilho {nome}? Ele deixa de rodar nas próximas alterações da tabela. Isso não pode ser \
             desfeito."
        );
        self.pendente = Some((Card::Gatilhos, OperacaoEsquema::ExcluirGatilho { nome }));
        self.confirmacao = Some(Confirmacao::new(frase).titulo("Excluir gatilho").rotulo("Excluir").perigo(true));
    }

    /// O conteúdo da parte (o `ParteTabela::conteudo`), no lugar dela no documento.
    fn conteudo_da_parte(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let egui = ui.ctx().clone();
        if let Some((card, r)) = self.tarefa.as_mut().and_then(|(card, p)| p.tomar().map(|r| (*card, r))) {
            self.tarefa = None;
            match r {
                Ok(r) => {
                    toast_da_resposta(&egui, ctx, &r);
                    pedir_recarga(ctx, tabela);
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        match card {
                            Card::Indices => {
                                self.erro_indices = Some(e.mensagem);
                                self.rolar_erro_indices = true;
                            }
                            Card::Gatilhos => {
                                self.erro_gatilhos = Some(e.mensagem);
                                self.rolar_erro_gatilhos = true;
                            }
                        }
                    }
                }
            }
        }
        let gatilhos = &tabela.info().gatilhos;
        let a = assinatura(gatilhos.iter().map(|g| (&g.nome, &g.sql)));
        if a != self.assinatura_gatilhos || self.sql_gatilhos.len() != gatilhos.len() {
            self.sql_gatilhos = gatilhos.iter().map(|g| sql_gatilho(g.sql.as_deref())).collect();
            self.assinatura_gatilhos = a;
        }

        let somente_leitura = ctx.estado.somente_leitura();
        let editavel = pode_editar(tabela.info(), somente_leitura);
        let mut acao = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            self.lista_de_indices(ui, tabela, editavel, somente_leitura, &mut acao);
            ui.add_space(ESPACO_LISTAS);
            // o mesmo recuo (15px) da faixa e da lista: a página recua 21px (o padding de 20 e a moldura)
            let disponivel = ui.available_rect_before_wrap();
            let recuo = RECUO_PAGINA + 1.0 - RECUO_LISTA;
            let largo = Rect::from_min_max(
                pos2(disponivel.left() - recuo, disponivel.top()),
                pos2(disponivel.right() + recuo, disponivel.bottom()),
            );
            ui.scope_builder(UiBuilder::new().max_rect(largo), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                self.lista_de_gatilhos(ui, tabela, editavel, &mut acao);
            });
        });
        match acao {
            Some(AcaoIndice::Selecionar(nome)) => self.selecionado = Some(nome),
            Some(AcaoIndice::ExcluirIndice(nome)) if self.tarefa.is_none() => self.pedir_excluir_indice(nome),
            Some(AcaoIndice::ExcluirGatilho(nome)) if self.tarefa.is_none() => self.pedir_excluir_gatilho(nome),
            _ => {}
        }
        if let Some(sim) = confirmar(&egui, &mut self.confirmacao) {
            let pendente = self.pendente.take();
            if let (true, Some((card, op))) = (sim, pendente) {
                let p = ctx.tarefa(&egui, move |servico, id| servico.esquema(id, &op, false));
                self.tarefa = Some((card, p));
            }
        }
        if let Some(j) = &mut self.janela {
            match j.mostrar_na_aba(&egui, ctx) {
                FimJanela::Aberta => {}
                FimJanela::Fechada => self.janela = None,
                FimJanela::Concluida => {
                    self.janela = None;
                    pedir_recarga(ctx, tabela);
                }
            }
        }
    }

    /// A lista dos índices (de ponta a ponta, no alto da página), ou o vazio dela.
    fn lista_de_indices(
        &mut self,
        ui: &mut Ui,
        tabela: &TabelaAberta,
        editavel: bool,
        somente_leitura: bool,
        acao: &mut Option<AcaoIndice>,
    ) {
        if let Some(msg) = &self.erro_indices {
            let r = aviso_texto(ui, msg, true);
            if std::mem::take(&mut self.rolar_erro_indices) {
                ui.scroll_to_rect(r.rect.expand2(vec2(0.0, 18.0)), None);
            }
            ui.add_space(ESPACO_LISTAS);
        }
        if tabela.info().indices.is_empty() {
            vazio(ui, "Nenhum índice nesta tabela. Índices aceleram buscas e ordenações por uma coluna.", true);
        } else {
            let selecionado = self.selecionado.as_deref();
            de_ponta_a_ponta(ui, |ui| tabela_indices(ui, tabela, editavel, selecionado, acao));
        }
        if !editavel {
            ui.add_space(ESPACO_LISTAS);
            let motivo = motivo_sem_edicao(&tabela.completa, somente_leitura);
            aviso_tom(ui, &[Trecho::Normal(&motivo)], TomAviso::Informacao);
        }
    }

    /// Os gatilhos: o título 13/600 com a contagem, e um modelo de cada, com o SQL nas cores do código.
    fn lista_de_gatilhos(&mut self, ui: &mut Ui, tabela: &TabelaAberta, editavel: bool, acao: &mut Option<AcaoIndice>) {
        let gatilhos = &tabela.info().gatilhos;
        if gatilhos.is_empty() {
            titulo_lista(ui, "Gatilhos");
        } else {
            titulo_lista_com_contagem(ui, "Gatilhos", &plural(gatilhos.len(), "gatilho", "gatilhos"));
        }
        if let Some(msg) = &self.erro_gatilhos {
            let r = aviso_texto(ui, msg, true);
            if std::mem::take(&mut self.rolar_erro_gatilhos) {
                ui.scroll_to_rect(r.rect.expand2(vec2(0.0, 18.0)), None);
            }
            ui.add_space(ESPACO_LISTAS);
        }
        if gatilhos.is_empty() {
            vazio(ui, "Nenhum gatilho nesta tabela. Gatilhos são criados pela Consulta SQL com CREATE TRIGGER.", true);
            return;
        }
        let sql_gatilhos = &self.sql_gatilhos;
        let instancia = self.instancia;
        for (i, g) in gatilhos.iter().enumerate() {
            let sql = sql_gatilhos.get(i).map_or("", String::as_str);
            // a aba nova (refeita a cada recarga e a cada volta à aba) traz os modelos fechados
            let id = Id::new(("tela_esquema::gatilho", &tabela.nome, &g.nome, instancia));
            modelo_forte(ui, id, &g.nome, i == 0, |ui| {
                // o SQL em mono nas cores do código, largura toda, até 320px
                TextoPre::sql(sql).altura_max(320.0).id_salt(id.with("sql")).mostrar(ui);
                em_linha(ui, 8.0, |ui| {
                    if ui.add(Botao::new("Copiar")).clicked() {
                        ui.ctx().copy_text(sql.to_string());
                        interface::pecas::toast::toast_concluido(ui.ctx(), "Copiado.");
                    }
                    if editavel && ui.add(Botao::ghost_perigo("Excluir")).clicked() {
                        *acao = Some(AcaoIndice::ExcluirGatilho(g.nome.clone()));
                    }
                });
            });
        }
    }

    /// A seção "Índice" das Propriedades: a linha selecionada (Nome, Colunas, Único, Origem e Condição).
    fn secao_indice(&self, ui: &mut Ui, tabela: &TabelaAberta) {
        let Some(ind) = self.indice_selecionado(tabela) else { return };
        let colunas = texto_colunas(ind);
        let condicao = condicao_do_indice(ind);
        let (origem, _) = origem_do_indice(ind.origem);
        let _ = Secao::new("propriedades::indice", "Índice").mostrar(ui, |ui| {
            ficha(ui, |d| {
                d.linha_texto("Nome", Some(&ind.nome), "—");
                d.linha_texto("Colunas", Some(&colunas), "—");
                d.linha_texto("Único", Some(if ind.unico { "sim" } else { "não" }), "—");
                d.linha_texto("Origem", Some(origem), "—");
                // o índice parcial que o analisador não lê diz só "parcial"
                let condicao = condicao.as_deref().or(ind.parcial.then_some("parcial"));
                d.linha_texto("Condição", condicao, "—");
            });
        });
    }

    #[cfg(test)]
    pub(crate) fn janela(&self) -> Option<&JanelaNovoIndice> {
        self.janela.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn sql_gatilhos(&self) -> &[String] {
        &self.sql_gatilhos
    }

    #[cfg(test)]
    pub(crate) fn selecionado(&self) -> Option<&str> {
        self.selecionado.as_deref()
    }
}

/// A lista dos índices (`.lista-t` de ponta a ponta), com o selecionado em `tr.sel`.
fn tabela_indices(
    ui: &mut Ui,
    tabela: &TabelaAberta,
    editavel: bool,
    selecionado: Option<&str>,
    acao: &mut Option<AcaoIndice>,
) {
    let indices = &tabela.info().indices;
    let mut cabecalhos = vec![
        ColunaTabela::new("Nome"),
        ColunaTabela::new("Colunas"),
        ColunaTabela::new("Único"),
        ColunaTabela::new("Origem"),
        ColunaTabela::new("Condição"),
    ];
    if editavel {
        cabecalhos.push(ColunaTabela::new("").direita().fixa(LARGURA_ACOES));
    }
    let marcada = |l: usize| indices.get(l).is_some_and(|i| Some(i.nome.as_str()) == selecionado);
    // o texto da lista a 15px da borda, como o da faixa e o dos Gatilhos (a coluna tem 12px e a moldura, 1px)
    let r = Tabela::new(("indices", &tabela.nome), &cabecalhos, indices.len())
        .marcadas(marcada)
        .recuo_lateral(RECUO_LISTA - 13.0)
        .clicavel()
        .mostrar(ui, |ui, l, c| {
            let Some(ind) = indices.get(l) else { return };
            match c {
                0 => {
                    forte(ui, &ind.nome);
                }
                1 => {
                    codigo(ui, &texto_colunas(ind));
                }
                2 => {
                    if ind.unico {
                        pill(ui, "único", TipoPill::Ok);
                    } else {
                        hint(ui, "—");
                    }
                }
                3 => {
                    let (texto, tipo) = origem_do_indice(ind.origem);
                    pill(ui, texto, tipo);
                }
                4 => match condicao_do_indice(ind) {
                    // `.tabela-indices td:nth-child(5) code{max-width:280px}`
                    Some(cond) => {
                        codigo_quebravel(ui, &cond, 280.0);
                    }
                    None if ind.parcial => {
                        pill(ui, "parcial", TipoPill::Mute);
                    }
                    None => {
                        hint(ui, "—");
                    }
                },
                _ => {
                    // só os índices criados se excluem; os automáticos não têm o botão
                    if pode_excluir_indice(ind) {
                        let b = Botao::icone(Icone::Excluir).pequeno().tipo(TipoBotao::GhostPerigo);
                        if ui.add(b.rotulo(ROTULO_EXCLUIR_INDICE).dica(ROTULO_EXCLUIR_INDICE)).clicked() {
                            *acao = Some(AcaoIndice::ExcluirIndice(ind.nome.clone()));
                        }
                    }
                }
            }
        });
    if let Some(l) = r.linha_clicada
        && acao.is_none()
        && let Some(ind) = indices.get(l)
    {
        *acao = Some(AcaoIndice::Selecionar(ind.nome.clone()));
    }
}

/// Os Índices e gatilhos como parte da tela de tabela: a faixa "Índices", o Novo índice e o Excluir do índice
/// selecionado na barra, e as Propriedades ("Índice"). Sem busca.
impl estado::ParteTabela for AbaIndices {
    fn anatomia(&self) -> estado::Anatomia {
        estado::Anatomia::default().com_faixa()
    }

    fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let n = tabela.info().indices.len();
        titulo_da_faixa(ui, "Índices", &plural(n, "índice", "índices"));
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.conteudo_da_parte(ui, ctx, tabela);
    }

    /// O Novo é o "Novo índice"; o Excluir é o do índice selecionado ("Excluir o índice idx_x"), ligado só nos
    /// criados.
    fn ofertas(&self, tabela: &TabelaAberta, estado: &estado::Estado) -> Ofertas {
        let editavel = pode_editar(tabela.info(), estado.somente_leitura());
        let livre = editavel && self.tarefa.is_none();
        let excluir = match self.indice_selecionado(tabela) {
            Some(ind) => Oferta::ligada()
                .ligada_se(livre && pode_excluir_indice(ind))
                .com_dica(format!("Excluir o índice {}", ind.nome)),
            None => Oferta::default(),
        };
        Ofertas::default()
            .com(Verbo::Novo, Oferta::ligada().ligada_se(editavel).com_rotulo("Novo índice").com_dica("Novo índice"))
            .com(Verbo::Excluir, excluir)
    }

    fn executar(&mut self, verbo: Verbo, _egui: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let editavel = pode_editar(tabela.info(), ctx.estado.somente_leitura());
        match verbo {
            Verbo::Novo if editavel => self.janela = Some(JanelaNovoIndice::da_tabela(tabela)),
            Verbo::Excluir if editavel && self.tarefa.is_none() => {
                if let Some(ind) = self.indice_selecionado(tabela).filter(|i| pode_excluir_indice(i)) {
                    let nome = ind.nome.clone();
                    self.pedir_excluir_indice(nome);
                }
            }
            _ => {}
        }
    }

    fn propriedades(&mut self, ui: &mut Ui, _ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.secao_indice(ui, tabela);
    }

    fn ocupada(&self) -> bool {
        self.tarefa.is_some()
    }
}

// ------------------------------------------------------------------ janela Novo índice

/// Uma coluna da lista do novo índice.
#[derive(Debug, Clone)]
pub(crate) struct LinhaIndice {
    pub(crate) nome: String,
    pub(crate) marcada: bool,
    pub(crate) desc: bool,
    /// "1ª", "2ª"… (vazio quando não escolhida).
    pub(crate) ordem: String,
}

/// Os campos da janela Novo índice.
pub(crate) struct CamposIndice {
    id: Id,
    tabela: String,
    /// None enquanto as colunas da tabela não chegam.
    pub(crate) linhas: Option<Vec<LinhaIndice>>,
    /// Índices em `linhas`, na ordem em que foram marcadas.
    pub(crate) escolhidas: Vec<usize>,
    pub(crate) nome: String,
    pub(crate) nome_editado: bool,
    pub(crate) unico: bool,
    pub(crate) onde: String,
}

/// A janela "Novo índice": colunas na ordem da tabela, marcadas na ordem de escolha, ASC/DESC por coluna,
/// único e condição. Aberta pelo "+ Novo índice" da aba ou pela casca (a sugestão de índice da aba Dados).
pub struct JanelaNovoIndice {
    /// As colunas que nascem marcadas, nessa ordem.
    iniciais: Vec<String>,
    carga: Option<Promessa<Vec<String>>>,
    pub(crate) campos: CamposIndice,
    pub(crate) mudanca: Mudanca,
    /// O índice foi criado (a janela fechou pelo "Criar índice").
    criado: bool,
}

impl JanelaNovoIndice {
    /// A janela da tabela `tabela`, com `colunas` já marcadas (nessa ordem). As colunas da tabela são lidas
    /// pelo serviço no primeiro quadro.
    pub fn nova(tabela: String, colunas: Vec<String>) -> JanelaNovoIndice {
        let id = Id::new(("tela_esquema::novo_indice", &tabela));
        JanelaNovoIndice {
            iniciais: colunas,
            carga: None,
            campos: CamposIndice {
                id,
                nome: nome_sugerido::<&str>(&tabela, &[]),
                tabela,
                linhas: None,
                escolhidas: Vec::new(),
                nome_editado: false,
                unico: false,
                onde: String::new(),
            },
            mudanca: Mudanca::nova(id, "Marque ao menos uma coluna para ver o SQL."),
            criado: false,
        }
    }

    /// A janela do "+ Novo índice" da aba, com as colunas da tabela já carregadas.
    pub(crate) fn da_tabela(tabela: &TabelaAberta) -> JanelaNovoIndice {
        let mut j = JanelaNovoIndice::nova(tabela.nome.clone(), Vec::new());
        j.campos.definir_colunas(tabela.info().colunas.iter().map(|c| c.nome.clone()).collect(), &[]);
        j
    }

    /// Desenha a janela; false quando ela fechou (pelo ✕, Esc, clique no fundo ou depois de criar o índice).
    /// Criado o índice, a janela emite `Acao::RecarregarObjetos` e [`JanelaNovoIndice::criou`] passa a dizer
    /// sim: a casca então relê a tabela aberta, se for a [`JanelaNovoIndice::tabela`] (a dica de
    /// ordenação lenta da aba Dados some).
    pub fn mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto) -> bool {
        match self.mostrar_na_aba(egui, ctx) {
            FimJanela::Aberta => true,
            FimJanela::Fechada => false,
            FimJanela::Concluida => {
                ctx.emitir(estado::Acao::RecarregarObjetos);
                false
            }
        }
    }

    /// O índice foi criado: a janela fechou pelo "Criar índice" (e não pelo ✕, pelo Esc ou pelo fundo). Quem a
    /// abriu relê a tabela.
    pub fn criou(&self) -> bool {
        self.criado
    }

    /// A tabela do índice.
    pub fn tabela(&self) -> &str {
        &self.campos.tabela
    }

    pub(crate) fn mostrar_na_aba(&mut self, egui: &egui::Context, ctx: &mut Contexto<'_>) -> FimJanela {
        let JanelaNovoIndice { iniciais, carga, campos, mudanca, criado } = self;
        if campos.linhas.is_none() && carga.is_none() {
            let nome = campos.tabela.clone();
            *carga = Some(ctx.tarefa(egui, move |servico, id| {
                servico.tabela(id, &nome).map(|c| c.info.colunas.into_iter().map(|c| c.nome).collect())
            }));
        }
        if let Some(r) = carga.as_mut().and_then(Promessa::tomar) {
            *carga = None;
            match r {
                Ok(nomes) => {
                    campos.definir_colunas(nomes, iniciais);
                    mudanca.prever_ja(egui);
                }
                Err(e) => {
                    // sem as colunas, a lista fica vazia (e o "Criar índice" diz o que falta)
                    campos.linhas = Some(Vec::new());
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        mudanca.avisar(e.mensagem, true);
                    }
                }
            }
        }
        if mudanca.passo(egui, ctx, || campos.montar()).is_some() {
            *criado = true;
            return FimJanela::Concluida;
        }

        let ocupado = mudanca.ocupada();
        let mut aplicar = false;
        let r = Janela::new(("tela_esquema::janela_indice", campos.id), "Novo índice")
            .rodape_direita(|ui| {
                if ui.add(Botao::primario("Criar índice").habilitado(!ocupado)).clicked() {
                    aplicar = true;
                }
            })
            .mostrar(egui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                mudanca.desenhar_aviso(ui);
                let mudou = campos.desenhar(ui);
                mudanca.desenhar_previa(ui);
                mudou
            });
        if r.inner {
            mudanca.agendar(egui);
        }
        if (aplicar || r.enviou) && !ocupado {
            mudanca.aplicar(egui, ctx, campos.montar());
        }
        if r.pediu_fechar && !ocupado {
            return FimJanela::Fechada;
        }
        FimJanela::Aberta
    }
}

impl CamposIndice {
    /// As colunas da tabela, com `iniciais` já marcadas nessa ordem.
    fn definir_colunas(&mut self, nomes: Vec<String>, iniciais: &[String]) {
        let mut linhas: Vec<LinhaIndice> = nomes
            .into_iter()
            .map(|nome| LinhaIndice { nome, marcada: false, desc: false, ordem: String::new() })
            .collect();
        self.escolhidas.clear();
        for c in iniciais {
            if let Some(i) = linhas.iter().position(|l| &l.nome == c)
                && !self.escolhidas.contains(&i)
            {
                linhas[i].marcada = true;
                self.escolhidas.push(i);
            }
        }
        self.linhas = Some(linhas);
        if !self.escolhidas.is_empty() {
            self.numerar();
        }
    }

    /// Numera a ordem de escolha e refaz o nome sugerido (se a pessoa não digitou outro).
    fn numerar(&mut self) {
        let Some(linhas) = &mut self.linhas else { return };
        for (i, l) in linhas.iter_mut().enumerate() {
            l.ordem = match self.escolhidas.iter().position(|&e| e == i) {
                Some(k) => format!("{}ª", k + 1),
                None => String::new(),
            };
        }
        if !self.nome_editado {
            let nomes: Vec<&str> =
                self.escolhidas.iter().filter_map(|&i| linhas.get(i).map(|l| l.nome.as_str())).collect();
            self.nome = nome_sugerido(&self.tabela, &nomes);
        }
    }

    pub(crate) fn montar(&self) -> Montagem {
        let Some(linhas) = &self.linhas else { return Montagem::Falta };
        if self.escolhidas.is_empty() {
            return Montagem::Falta;
        }
        let colunas = self
            .escolhidas
            .iter()
            .filter_map(|&i| linhas.get(i))
            .map(|l| ColunaIndicePedida { nome: l.nome.clone(), desc: l.desc })
            .collect();
        let onde = self.onde.trim();
        Montagem::pedido(OperacaoEsquema::CriarIndice {
            nome: self.nome.trim().to_string(),
            tabela: self.tabela.clone(),
            colunas,
            unico: self.unico,
            onde: (!onde.is_empty()).then(|| onde.to_string()),
        })
    }

    /// Os campos: Nome, a lista de colunas, Único e a condição. Devolve se algo mudou.
    fn desenhar(&mut self, ui: &mut Ui) -> bool {
        let id = self.id;
        let mut mudou = false;
        let mut alternada: Option<usize> = None;
        let mut nome_mudou = false;
        {
            let CamposIndice { nome, linhas, unico, onde, .. } = self;
            campos_duas_colunas(ui, |c| {
                c.largo(|ui| {
                    campo(
                        ui,
                        "Nome",
                        Some(
                            "Sugerido pelas colunas escolhidas; pode trocar. O nome não pode repetir o de outra \
                             tabela, visão ou índice.",
                        ),
                        |ui| {
                            nome_mudou = ui
                                .add(Entrada::new(nome).monoespacada(true).rotulo("Nome").id_salt(id.with("nome")))
                                .changed();
                        },
                    );
                });
                c.largo(|ui| {
                    // o `div.field` das colunas: o rótulo, 6px e a lista
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        rotulo_campo(
                            ui,
                            "Colunas",
                            Some(
                                "Marque na ordem em que o índice deve ordenar: a primeira marcada é a que mais pesa. \
                                 O número ao lado mostra a ordem.",
                            ),
                        );
                        let (alt, dir) = lista_colunas(ui, id, linhas.as_deref_mut());
                        alternada = alt;
                        mudou |= dir;
                    });
                });
                c.largo(|ui| {
                    interface::pecas::arranjo::linha(ui, 19.0, 6.0, |ui| {
                        mudou |= check(ui, unico, "Único").changed();
                        ajuda_mini(
                            ui,
                            "O banco passa a recusar linhas com a mesma combinação de valores nessas colunas.",
                        );
                    });
                });
                c.largo(|ui| {
                    campo(
                        ui,
                        "Condição (WHERE)",
                        Some("Opcional: o índice só cobre as linhas que cumprem a condição (índice parcial)."),
                        |ui| {
                            mudou |= ui
                                .add(
                                    Entrada::new(onde)
                                        .monoespacada(true)
                                        .dica("opcional, como ativo = 1")
                                        .rotulo("Condição (WHERE)"),
                                )
                                .changed();
                        },
                    );
                });
            });
        }
        if nome_mudou {
            self.nome_editado = !self.nome.trim().is_empty();
            mudou = true;
        }
        if let Some(i) = alternada {
            let marcada = self.linhas.as_ref().and_then(|l| l.get(i)).is_some_and(|l| l.marcada);
            let pos = self.escolhidas.iter().position(|&e| e == i);
            match (marcada, pos) {
                (true, None) => self.escolhidas.push(i),
                (false, Some(k)) => {
                    self.escolhidas.remove(k);
                }
                _ => {}
            }
            self.numerar();
            mudou = true;
        }
        mudou
    }
}

/// A `.lista-indice`: uma linha por coluna da tabela (caixa + nome, a ordem de escolha "1ª" e o ASC/DESC da
/// escolhida). Devolve a linha cuja caixa mudou e se alguma direção mudou.
fn lista_colunas(ui: &mut Ui, id: Id, linhas: Option<&mut [LinhaIndice]>) -> (Option<usize>, bool) {
    let mut alternada = None;
    let mut direcao_mudou = false;
    painel_rebaixado(ui, Margin::symmetric(12, 6), |ui| {
        let Some(linhas) = linhas else {
            interface::pecas::arranjo::linha(ui, 34.0, 0.0, |ui| {
                carregando(ui, "Lendo a tabela…");
            });
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt(id.with("lista"))
            .max_height(260.0 - 12.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let p = paleta(ui);
                for (i, l) in linhas.iter_mut().enumerate() {
                    // `.coluna-indice`: grid minmax(0,1fr) 28px 170px, 8px entre eles, 34px no mínimo
                    let largura = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(vec2(largura, 34.0), Sense::hover());
                    let x_select = rect.right() - 170.0;
                    let x_ordem = x_select - 8.0 - 28.0;
                    let rect_check =
                        Rect::from_min_max(rect.min, pos2((x_ordem - 8.0).max(rect.left()), rect.bottom()));
                    let mut ui_check = ui.new_child(
                        UiBuilder::new()
                            .id_salt(id.with(("check", i)))
                            .max_rect(rect_check)
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    ui_check.spacing_mut().item_spacing.x = 8.0;
                    let antes = l.marcada;
                    let resp_caixa = Check::new(&mut l.marcada, "").mostrar(&mut ui_check);
                    // o nome (800 quando escolhida) faz parte do `label.check`: clicar nele também marca
                    let peso = if l.marcada { Peso::Pesado } else { Peso::Normal };
                    let livre = (rect_check.right() - ui_check.cursor().min.x).max(10.0);
                    let g = galley_cortado(&ui_check, &l.nome, fontes::fonte(12.5, peso), p.fg, livre);
                    let (rect_nome, resp_nome) = ui_check.allocate_exact_size(vec2(g.size().x, 19.0), Sense::click());
                    ui_check.painter().galley(pos2(rect_nome.left(), rect_nome.center().y - g.size().y / 2.0), g, p.fg);
                    if resp_nome.hovered() {
                        ui_check.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp_nome.clicked() {
                        l.marcada = !l.marcada;
                    }
                    let marcada = l.marcada;
                    let nome = l.nome.as_str();
                    resp_caixa
                        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, marcada, nome));
                    if l.marcada != antes {
                        alternada = Some(i);
                    }
                    if !l.ordem.is_empty() {
                        // `.ordem-escolha`: hint em ouro, 800, à direita da coluna de 28px
                        let go = galley(ui, &l.ordem, fontes::fonte(11.5, Peso::Pesado), p.gold_text);
                        let pos = pos2(x_ordem + 28.0 - go.size().x, rect.center().y - go.size().y / 2.0);
                        ui.painter().galley(pos, go, p.gold_text);
                        let rect_select =
                            Rect::from_min_size(pos2(x_select, rect.center().y - 15.0), vec2(170.0, 30.0));
                        let mut ui_select =
                            ui.new_child(UiBuilder::new().id_salt(id.with(("direcao", i))).max_rect(rect_select));
                        let rotulo = format!("Ordem de {}", l.nome);
                        let opcoes = [(false, "ASC (crescente)"), (true, "DESC (decrescente)")];
                        if Select::new(id.with(("direcao", i)), &mut l.desc, &opcoes)
                            .largura(170.0)
                            .rotulo(&rotulo)
                            .mostrar(&mut ui_select)
                            .changed()
                        {
                            direcao_mudou = true;
                        }
                    }
                }
            });
    });
    (alternada, direcao_mudou)
}
