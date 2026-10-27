//! A tela de uma tabela ou visão (`#/t/:nome/:aba`, `#/v/:nome/:aba`): carrega a tabela pelo serviço e entrega as
//! partes (Dados, Estrutura, Índices e gatilhos, Definição SQL) ao crate delas com a [`TabelaAberta`], pelo contrato
//! [`ParteTabela`]. A tabela que não existe mais vira o card "A tabela X não existe mais neste banco." com o link
//! para a Visão geral.
//!
//! Revisão de UI de 19/10/2026 (uma aba por objeto): o **documento** é sempre a grade dos Dados (a faixa, o conteúdo
//! e o rodapé deles, o Atualizar e o Exportar ("Exportar clientes") junto das ofertas deles). As outras partes viram
//! **vistas das Propriedades** ([`Vista`]), no segmentado do topo do painel: Resumo (a seção "Tabela", as dos Dados e
//! o "Arquivo" curto), Colunas (a Estrutura), Índices (os Índices e gatilhos) e SQL (a Definição); a visão só tem
//! Resumo e SQL. Na doca o segmentado usa os rótulos curtos (sem espaço nem para eles, só os ícones, com o nome na
//! dica: [`Vista::icone`]); na janela própria (a forma larga, a partir de
//! [`LARGURA_FORMA_LARGA`]), por extenso. A rota com outra parte (`#/t/clientes/estrutura`, o "Colunas" da Visão
//! geral, o Renomear) abre a aba do objeto com aquela vista; o endereço da aba fica o dos Dados. A coluna escolhida
//! na vista Colunas acende na grade.

use banco::{Contagem, TipoObjeto};
use egui::Ui;
use estado::{
    Aba, Acao, BuscaDaBarra, Contexto, Estado, EventoBusca, Oferta, Ofertas, ParteTabela, PedidoJanelaExportar,
    PedidoTabela, Promessa, Reacao, Rota, TabelaAberta, Verbo,
};
use interface::formato::formatar_bytes;
use interface::formato::plural;
use interface::icones::Icone;
use interface::pecas::avisos::{aviso_texto, carregando, vazio};
use interface::pecas::cartao::Cartao;
use interface::pecas::navegacao::{Cabecalho, ParteSeletor, Passo, TamanhoTrilho, trilho_com_partes};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::link;
use interface::pecas::toast::toast_concluido;
use servico::TabelaCompleta;

use crate::recursos::nome_arquivo;
use crate::telas::{AbaDados, AbaDefinicao, AbaEstrutura, AbaIndices};

/// "60 linhas · 7 colunas" (+ " · sem rowid" / " · visão"); sem a contagem, só as colunas.
pub(crate) fn subtitulo(linhas: Option<u64>, colunas: usize, sem_rowid: bool, e_visao: bool) -> String {
    let mut partes = Vec::with_capacity(4);
    if let Some(n) = linhas {
        partes.push(plural(n, "linha", "linhas"));
    }
    partes.push(plural(colunas as u64, "coluna", "colunas"));
    if sem_rowid {
        partes.push("sem rowid".to_string());
    }
    if e_visao {
        partes.push("visão".to_string());
    }
    partes.join(" · ")
}

/// O pedido da janela Exportar da tabela (`TabelaAberta::pedido_exportacao`, com a busca, os filtros, a ordem e as
/// linhas marcadas da aba Dados), com o nome do arquivo limpo.
fn pedido_exportacao(tabela: &TabelaAberta) -> PedidoJanelaExportar {
    let mut pedido = tabela.pedido_exportacao();
    pedido.nome_sugerido = nome_arquivo(&tabela.nome);
    pedido
}

/// A partir desta largura, as Propriedades estão na forma larga (a janela própria): o segmentado por extenso e a
/// tabela de colunas da Estrutura no lugar da lista compacta.
pub(crate) const LARGURA_FORMA_LARGA: f32 = 560.0;

/// O que as Propriedades da tabela mostram (o segmentado `.seg` do topo do painel, prancheta 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Vista {
    /// A seção "Tabela", as dos Dados e o "Arquivo" curto.
    #[default]
    Resumo,
    /// A Estrutura.
    Colunas,
    /// Os Índices e gatilhos.
    Indices,
    /// A Definição SQL.
    Sql,
}

impl Vista {
    pub(crate) const DA_TABELA: [Vista; 4] = [Vista::Resumo, Vista::Colunas, Vista::Indices, Vista::Sql];
    pub(crate) const DA_VISAO: [Vista; 2] = [Vista::Resumo, Vista::Sql];

    /// A vista de uma parte da rota (os Dados são o Resumo: a parte já está no documento).
    pub(crate) fn da_parte(parte: Aba) -> Vista {
        match parte {
            Aba::Dados => Vista::Resumo,
            Aba::Estrutura => Vista::Colunas,
            Aba::Indices => Vista::Indices,
            Aba::Definicao => Vista::Sql,
        }
    }

    /// O ícone que fica no lugar do rótulo quando ele não cabe (a doca mais estreita, revisão de 20/10/2026): a ficha
    /// da tabela (ou da visão) no Resumo, as colunas, os índices e a folha SQL.
    pub(crate) fn icone(self, e_visao: bool) -> Icone {
        match self {
            Vista::Resumo if e_visao => Icone::Visao,
            Vista::Resumo => Icone::Tabela,
            Vista::Colunas => Icone::Estrutura,
            Vista::Indices => Icone::Indices,
            Vista::Sql => Icone::Definicao,
        }
    }

    /// O rótulo no segmentado: curto na doca; por extenso na forma larga (a janela própria, prancheta 6).
    pub(crate) fn rotulo(self, larga: bool) -> &'static str {
        match (self, larga) {
            (Vista::Resumo, _) => "Resumo",
            (Vista::Colunas, _) => "Colunas",
            (Vista::Indices, false) => "Índices",
            (Vista::Indices, true) => "Índices e gatilhos",
            (Vista::Sql, false) => "SQL",
            (Vista::Sql, true) => "Definição SQL",
        }
    }
}

/// A parte de uma vista das Propriedades, do crate dela.
enum ParteDaVista {
    Estrutura(Box<AbaEstrutura>),
    Indices(Box<AbaIndices>),
    Definicao(Box<AbaDefinicao>),
}

impl ParteDaVista {
    /// A parte da vista (o Resumo não tem: são as seções da tela).
    fn nova(vista: Vista, tabela: &TabelaAberta) -> Option<ParteDaVista> {
        match vista {
            Vista::Resumo => None,
            Vista::Colunas => Some(ParteDaVista::Estrutura(Box::new(AbaEstrutura::nova(tabela)))),
            Vista::Indices => Some(ParteDaVista::Indices(Box::new(AbaIndices::nova(tabela)))),
            Vista::Sql => Some(ParteDaVista::Definicao(Box::new(AbaDefinicao::nova(tabela)))),
        }
    }

    fn vista(&self) -> Vista {
        match self {
            ParteDaVista::Estrutura(_) => Vista::Colunas,
            ParteDaVista::Indices(_) => Vista::Indices,
            ParteDaVista::Definicao(_) => Vista::Sql,
        }
    }

    /// A parte, pelo contrato delas.
    fn parte(&mut self) -> &mut dyn ParteTabela {
        match self {
            ParteDaVista::Estrutura(a) => a.as_mut(),
            ParteDaVista::Indices(a) => a.as_mut(),
            ParteDaVista::Definicao(a) => a.as_mut(),
        }
    }

    fn parte_ref(&self) -> &dyn ParteTabela {
        match self {
            ParteDaVista::Estrutura(a) => a.as_ref(),
            ParteDaVista::Indices(a) => a.as_ref(),
            ParteDaVista::Definicao(a) => a.as_ref(),
        }
    }
}

struct Pronta {
    tabela: TabelaAberta,
    /// Os Dados: o documento.
    dados: Box<AbaDados>,
    /// A parte da vista das Propriedades (a Estrutura, os Índices, a Definição), feita quando a vista aparece.
    vista: Option<ParteDaVista>,
    /// "60 linhas · 7 colunas" (o status da barra).
    sub: String,
}

enum Situacao {
    Carregando,
    /// A leitura voltou um erro que a tela ignora (`BancoTrocado`: a casca já relê o estado e refaz a tela;
    /// `Substituido`). Mostra o "Lendo a tabela…" sem pedir de novo: pedir com o mesmo id antigo voltaria o
    /// mesmo erro, num laço de tarefas e quadros enquanto a casca ressincroniza.
    Esperando,
    Falha {
        mensagem: String,
        inexistente: bool,
    },
    Pronta(Box<Pronta>),
}

/// A tela de tabela ou visão.
pub(crate) struct TelaTabela {
    /// A rota pedida (corrigida depois de carregar para a parte Dados, e para o tipo certo do objeto).
    rota: Rota,
    /// A vista das Propriedades (a parte que a rota pediu, no começo).
    vista: Vista,
    /// Um pedido do menu Tabela à espera da Estrutura (que só existe com a vista Colunas à vista).
    pedido: Option<PedidoTabela>,
    carga: Option<Promessa<TabelaCompleta>>,
    contagem: Option<Promessa<Contagem>>,
    situacao: Situacao,
    /// O caminho e o tamanho do banco na seção "Arquivo" das Propriedades, com a versão do estado em que foram feitos.
    arquivo: Option<(u64, String, String)>,
}

/// "A tabela X não existe mais neste banco." vale para 404 e para as mensagens que dizem "não existe".
fn e_inexistente(e: &comum::ErroBanco) -> bool {
    e.status == 404 || e.mensagem.to_lowercase().contains("não existe")
}

impl TelaTabela {
    pub fn nova(rota: Rota) -> TelaTabela {
        let vista = Vista::da_parte(rota.aba().unwrap_or_default());
        TelaTabela {
            rota,
            vista,
            pedido: None,
            carga: None,
            contagem: None,
            situacao: Situacao::Carregando,
            arquivo: None,
        }
    }

    /// A vista das Propriedades agora.
    pub(crate) fn vista(&self) -> Vista {
        self.vista
    }

    /// Põe à vista nas Propriedades a parte pedida (a rota com outra parte, o menu Tabela); a visão só tem o Resumo e
    /// o SQL.
    pub(crate) fn mostrar_parte(&mut self, parte: Aba) {
        let vista = Vista::da_parte(parte);
        let e_visao = match &self.situacao {
            Situacao::Pronta(p) => p.tabela.e_visao,
            _ => matches!(self.rota, Rota::Visao { .. }),
        };
        let vistas: &[Vista] = if e_visao { &Vista::DA_VISAO } else { &Vista::DA_TABELA };
        if vistas.contains(&vista) {
            self.vista = vista;
        }
    }

    /// Um pedido do menu Tabela (Nova coluna, Renomear, Excluir a tabela): vai para a Estrutura, que fica à vista nas
    /// Propriedades (vista Colunas) e o atende no próximo quadro.
    pub(crate) fn pedir(&mut self, pedido: PedidoTabela) {
        self.vista = Vista::Colunas;
        self.pedido = Some(pedido);
    }

    /// A tabela pode mudar de estrutura (é tabela, o banco grava e o SQLite deixa editar).
    pub(crate) fn editavel(&self, somente_leitura: bool) -> bool {
        self.tabela_aberta().is_some_and(|t| !t.e_visao && crate::telas::pode_editar(t.info(), somente_leitura))
    }

    /// Há tarefas desta tela rodando (a da aba não conta).
    pub fn ocupada(&self) -> bool {
        self.carga.is_some() || self.contagem.is_some()
    }

    /// A tabela carregada (quando a tela está pronta).
    pub fn tabela_aberta(&self) -> Option<&TabelaAberta> {
        match &self.situacao {
            Situacao::Pronta(p) => Some(&p.tabela),
            _ => None,
        }
    }

    /// "60 linhas · 7 colunas" da tabela carregada (o subtítulo do cabeçalho, com a contagem que chegou depois).
    pub fn sub(&self) -> Option<&str> {
        match &self.situacao {
            Situacao::Pronta(p) => Some(&p.sub),
            _ => None,
        }
    }

    /// Relê a tabela e refaz a aba sem reler os objetos, que quem pediu já releu: a janela Novo índice da casca, ao criar o índice. A leitura começa no próximo quadro.
    pub fn recarregar(&mut self) {
        if let Situacao::Pronta(p) = &self.situacao {
            // a rota com o estado atual da aba Dados (página, ordem, busca), como no `tabela.recarregar`
            self.rota = p.tabela.rota.clone();
        }
        self.carga = None;
        self.contagem = None;
        self.situacao = Situacao::Carregando;
    }

    /// O nome da tabela ou visão pedida (o da rota, antes mesmo de ela ser lida).
    pub fn nome(&self) -> &str {
        self.rota.nome_objeto().unwrap_or_default()
    }

    /// O pedido da janela Exportar da tabela carregada (o "Exportar" do cabeçalho e o Exportar da barra de
    /// ferramentas); None antes de ela ser lida.
    pub fn pedido_exportacao(&self) -> Option<PedidoJanelaExportar> {
        self.tabela_aberta().map(pedido_exportacao)
    }

    fn carregar(&mut self, ui: &Ui, ctx: &Contexto) {
        let nome = self.nome().to_string();
        self.carga = Some(ctx.tarefa(ui.ctx(), move |s, id| s.tabela(id, &nome)));
    }

    fn receber(&mut self, ui: &Ui, ctx: &mut Contexto) {
        if let Some(r) = self.carga.as_mut().and_then(Promessa::tomar) {
            self.carga = None;
            match r {
                Ok(completa) => self.montar(ui, ctx, completa),
                Err(e) => {
                    self.situacao = if ctx.reagir(&e) == Reacao::Mostrar {
                        Situacao::Falha { inexistente: e_inexistente(&e), mensagem: e.mensagem }
                    } else {
                        Situacao::Esperando
                    };
                }
            }
        }
        if let Some(r) = self.contagem.as_mut().and_then(Promessa::tomar) {
            self.contagem = None;
            // sem a contagem, o subtítulo fica só com as colunas
            if let (Ok(c), Situacao::Pronta(p)) = (r, &mut self.situacao) {
                p.tabela.completa.info.linhas = Some(c.linhas);
                let info = p.tabela.info();
                p.sub = subtitulo(Some(c.linhas), info.colunas.len(), info.sem_rowid, p.tabela.e_visao);
            }
        }
    }

    fn montar(&mut self, ui: &Ui, ctx: &mut Contexto, completa: TabelaCompleta) {
        let e_visao = completa.info.tipo == TipoObjeto::View;
        let nome = self.nome().to_string();
        // o documento é sempre a parte Dados; a parte que a rota pediu já virou a vista das Propriedades
        let pedida = self.rota.aba().unwrap_or_default();
        let aba = Aba::Dados;
        let vistas: &[Vista] = if e_visao { &Vista::DA_VISAO } else { &Vista::DA_TABELA };
        if !vistas.contains(&self.vista) {
            self.vista = Vista::Resumo;
        }
        let mesmo_tipo = matches!(self.rota, Rota::Visao { .. }) == e_visao;
        if !mesmo_tipo || aba != pedida {
            // o endereço muda sem entrar no histórico; a query só fica na mesma aba
            let mut certa = Rota::objeto_na_aba(nome.clone(), e_visao, aba);
            if aba == pedida
                && let Some(d) = self.rota.dados()
            {
                certa = certa.com_dados(d.clone());
            }
            self.rota = certa.clone();
            ctx.emitir(Acao::Substituir(certa));
        }
        let info = &completa.info;
        let sub = subtitulo(info.linhas, info.colunas.len(), info.sem_rowid, e_visao);
        if info.linhas.is_none() && !e_visao {
            // a contagem de tabela grande não vem na TabelaInfo; ela chega depois, sem segurar a tela
            let n = nome.clone();
            self.contagem = Some(ctx.tarefa(ui.ctx(), move |s, id| s.contagem(id, &n, None, &[], None)));
        }
        let tabela = TabelaAberta::nova(nome, e_visao, completa, self.rota.clone());
        let dados = Box::new(AbaDados::nova(&tabela));
        let vista = ParteDaVista::nova(self.vista, &tabela);
        self.situacao = Situacao::Pronta(Box::new(Pronta { tabela, dados, vista, sub }));
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if self.carga.is_none() && matches!(self.situacao, Situacao::Carregando) {
            self.carregar(ui, ctx);
        }
        let pronta_antes = matches!(self.situacao, Situacao::Pronta(_));
        self.receber(ui, ctx);
        // A anatomia que a casca leu antes do `conteudo` era a da tela ainda sem parte (página que rola); a parte que
        // acabou de ser lida (Dados, Estrutura…) é de rolagem própria e enche a altura toda, o que numa página de
        // rolagem dá uma altura infinita. Neste quadro, ainda o "Lendo a tabela…"; o seguinte já nasce com a anatomia
        // da parte.
        let acabou_de_ler = !pronta_antes && matches!(self.situacao, Situacao::Pronta(_));
        let nome = self.nome().to_string();
        let e_visao = matches!(self.rota, Rota::Visao { .. });
        let mut recarregar: Option<Rota> = None;
        match &mut self.situacao {
            Situacao::Carregando | Situacao::Esperando => {
                carregando(ui, "Lendo a tabela…");
            }
            Situacao::Pronta(_) if acabou_de_ler => {
                carregando(ui, "Lendo a tabela…");
                ui.ctx().request_repaint();
            }
            Situacao::Falha { mensagem, inexistente } => {
                let grupo = if e_visao { "Visões" } else { "Tabelas" };
                let passos = [Passo { rotulo: "Banco", link: true }, Passo { rotulo: grupo, link: false }];
                let r = Cabecalho::new(("tabela", &nome), &nome).trilha(&passos).mostrar(ui);
                let mut ir_banco = r.trilha_clicada == Some(0);
                Cartao::new(("falha-tabela", &nome)).mostrar(ui, |c| {
                    c.corpo(|ui| {
                        if *inexistente {
                            let tipo = if e_visao { "visão" } else { "tabela" };
                            vazio(ui, &format!("A {tipo} {nome} não existe mais neste banco."), true);
                            // `p.centro{margin:0}` logo embaixo do `.empty`, numa linha de 19,5px (13px × 1,5)
                            ui.add_space(-ui.spacing().item_spacing.y);
                            let largura = ui.available_width();
                            ui.allocate_ui_with_layout(
                                egui::vec2(largura, 19.5),
                                egui::Layout::top_down(egui::Align::Center).with_main_align(egui::Align::Center),
                                |ui| {
                                    ui.set_min_size(egui::vec2(largura, 19.5));
                                    if link(ui, "Ir para a visão geral do banco").clicked() {
                                        ir_banco = true;
                                    }
                                },
                            );
                        } else {
                            aviso_texto(ui, mensagem, true);
                        }
                    });
                });
                if ir_banco {
                    ctx.navegar(Rota::Banco);
                }
            }
            // a parte no formato de documento: a casca desenha a faixa e o rodapé dela, e aqui vem só o conteúdo (as
            // partes ficam nas abas da casca e no Explorador; o título, o Exportar e a ajuda, na barra e nas dicas)
            Situacao::Pronta(p) => {
                let Pronta { tabela, dados, vista, .. } = p.as_mut();
                // a coluna escolhida na vista Colunas acende na grade
                let escolhida = vista
                    .as_ref()
                    .filter(|v| v.vista() == Vista::Colunas)
                    .and_then(|v| v.parte_ref().coluna_escolhida());
                dados.destacar_coluna(escolhida);
                dados.conteudo(ui, ctx, tabela);
                if tabela.recarregar {
                    tabela.recarregar = false;
                    recarregar = Some(tabela.rota.clone());
                }
            }
        }
        if let Some(rota) = recarregar {
            // a estrutura mudou: relê a tabela e os objetos e refaz a aba
            self.rota = rota;
            ctx.emitir(Acao::RecarregarObjetos);
            self.situacao = Situacao::Carregando;
            self.carregar(ui, ctx);
        }
    }
}

/// A tela de tabela como documento da casca: o documento é a parte Dados, com o Atualizar e o Exportar ("Exportar
/// clientes", depois de lida), que valem para toda tabela, junto das ofertas dela; as Propriedades têm o segmentado
/// das vistas ([`Vista`]) no topo.
impl estado::Documento for TelaTabela {
    /// A dos Dados, depois de lida (antes, a página sem faixa nem rodapé, com o "Lendo a tabela…").
    fn anatomia(&self) -> estado::Anatomia {
        match &self.situacao {
            Situacao::Pronta(p) => p.dados.anatomia(),
            _ => estado::Anatomia::default(),
        }
    }

    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, dados, .. } = p.as_mut();
            dados.faixa(ui, ctx, tabela);
        }
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ui(ui, ctx);
    }

    fn rodape(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, dados, .. } = p.as_mut();
            dados.rodape(ui, ctx, tabela);
        }
    }

    fn ofertas(&self, estado: &Estado) -> Ofertas {
        let dos_dados = match &self.situacao {
            Situacao::Pronta(p) => p.dados.ofertas(&p.tabela, estado),
            _ => Ofertas::default(),
        };
        let exportar =
            Oferta::ligada().ligada_se(self.tabela_aberta().is_some()).com_dica(format!("Exportar {}", self.nome()));
        dos_dados
            .com(Verbo::Atualizar, Oferta::ligada().ligada_se(!TelaTabela::ocupada(self)))
            .com(Verbo::Exportar, exportar)
    }

    fn executar(&mut self, verbo: Verbo, egui: &egui::Context, ctx: &mut Contexto) {
        match verbo {
            Verbo::Exportar => {
                if let Some(pedido) = self.pedido_exportacao() {
                    ctx.emitir(Acao::AbrirExportacao(pedido));
                }
            }
            // a tabela relê (com a página, a ordem e a busca dos Dados) e a casca relê os objetos (as outras abas
            // da mesma tabela releem quando aparecerem)
            Verbo::Atualizar => {
                self.recarregar();
                ctx.emitir(Acao::RecarregarObjetos);
            }
            _ => {
                if let Situacao::Pronta(p) = &mut self.situacao {
                    let Pronta { tabela, dados, .. } = p.as_mut();
                    dados.executar(verbo, egui, ctx, tabela);
                }
            }
        }
    }

    /// A busca da barra é a dos Dados ("Buscar em clientes"); antes de a tabela ser lida, desligada.
    fn busca(&mut self) -> Option<BuscaDaBarra<'_>> {
        match &mut self.situacao {
            Situacao::Pronta(p) => p.dados.busca(),
            _ => None,
        }
    }

    fn evento_busca(&mut self, evento: EventoBusca, egui: &egui::Context, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, dados, .. } = p.as_mut();
            dados.evento_busca(evento, egui, ctx, tabela);
        }
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    /// No Resumo, as seções rolam pela casca; nas outras vistas, o segmentado fica fixo e a vista rola por dentro.
    fn propriedades_rolam(&self) -> bool {
        self.vista == Vista::Resumo || !matches!(self.situacao, Situacao::Pronta(_))
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if self.arquivo.as_ref().is_none_or(|(v, ..)| *v != ctx.versao) {
            let (caminho, tamanho) = ctx
                .estado
                .banco
                .as_ref()
                .map(|b| (b.caminho.display().to_string(), formatar_bytes(b.tamanho)))
                .unwrap_or_default();
            self.arquivo = Some((ctx.versao, caminho, tamanho));
        }
        let nome = self.nome().to_string();
        let Situacao::Pronta(p) = &mut self.situacao else {
            // antes de ser lida, só o nome
            let _ = Secao::new("propriedades::tabela", "Tabela").primeira(true).mostrar(ui, |ui| {
                ficha(ui, |d| d.linha_texto("Nome", Some(&nome), "—"));
            });
            return;
        };
        // o segmentado das vistas (`.seg` de ponta a ponta, `padding:8px 10px 6px`)
        let vistas: &[Vista] = if p.tabela.e_visao { &Vista::DA_VISAO } else { &Vista::DA_TABELA };
        let larga = ui.available_width() >= LARGURA_FORMA_LARGA;
        let e_visao = p.tabela.e_visao;
        let partes: Vec<ParteSeletor<'_>> =
            vistas.iter().map(|v| ParteSeletor::new(v.rotulo(larga)).reserva(v.icone(e_visao))).collect();
        let atual = vistas.iter().position(|v| *v == self.vista);
        let mut escolhida = None;
        recuado(ui, [8.0, 10.0, 6.0], |ui| {
            let id = ui.id().with(("vistas-da-tabela", &nome));
            let (clicada, _) = trilho_com_partes(ui, id, &partes, atual, TamanhoTrilho::CompactoCheio, false);
            escolhida = clicada.and_then(|i| vistas.get(i).copied());
        });
        if let Some(v) = escolhida {
            self.vista = v;
            ui.ctx().request_repaint();
        }
        let vista = self.vista;
        let pedido = self.pedido.take();
        match vista {
            Vista::Resumo => {
                let Pronta { tabela, dados, .. } = p.as_mut();
                let (parte, lida) = (dados.as_mut(), &*tabela);
                estado::secao_tabela(ui, lida, |ui| parte.acoes_da_tabela(ui, ctx, lida));
                dados.propriedades(ui, ctx, tabela);
                let (caminho, tamanho) =
                    self.arquivo.as_ref().map(|(_, c, t)| (c.as_str(), t.as_str())).unwrap_or_default();
                let banco = ctx.estado.banco.as_ref();
                let mut copiar = false;
                let _ = Secao::new("propriedades::arquivo_curto", "Arquivo").mostrar(ui, |ui| {
                    ficha(ui, |d| {
                        copiar = d.linha_caminho("Caminho", caminho);
                        if let Some(b) = banco {
                            crate::casca::status::linha_da_gravacao(d, b);
                        }
                        d.linha_texto("Tamanho", Some(tamanho), "—");
                    });
                });
                if copiar {
                    ui.ctx().copy_text(caminho.to_string());
                    toast_concluido(ui.ctx(), crate::visao_geral::MSG_CAMINHO_COPIADO);
                }
            }
            _ => {
                let egui = ui.ctx().clone();
                // a parte da vista, feita agora se ainda não existe (ou era de outra vista)
                if p.vista.as_ref().map(ParteDaVista::vista) != Some(vista) {
                    p.vista = ParteDaVista::nova(vista, &p.tabela);
                }
                let Pronta { tabela, vista: parte_vista, .. } = p.as_mut();
                let Some(parte_vista) = parte_vista else { return };
                let parte = parte_vista.parte();
                if let Some(pedido) = pedido {
                    parte.pedido(pedido, &egui, ctx, tabela);
                }
                // a faixa da parte ("Colunas · 7 colunas", "Comando de criação" e o Copiar) como cabeçalho da vista
                let largura = ui.available_width();
                let (linha, _) =
                    ui.allocate_exact_size(egui::vec2(largura, ALTURA_CABECALHO_VISTA), egui::Sense::hover());
                let mut ui_faixa = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(linha.shrink2(egui::vec2(12.0, 0.0)))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                parte.faixa(&mut ui_faixa, ctx, tabela);
                if vista == Vista::Sql {
                    // a Definição rola por dentro, na altura que sobra
                    parte.conteudo(ui, ctx, tabela);
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt(("vista", vista as u8, &nome))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            recuado(ui, [0.0, 10.0, 10.0], |ui| parte.conteudo(ui, ctx, tabela));
                            parte.propriedades(ui, ctx, tabela);
                        });
                }
                if tabela.recarregar {
                    // a estrutura mudou pela vista: a tela relê no próximo quadro do documento
                    egui.request_repaint();
                }
            }
        }
    }

    /// "clientes · 60 linhas · 7 colunas", depois de lida.
    fn status(&self, _estado: &Estado) -> Option<String> {
        let nome = &self.tabela_aberta()?.nome;
        self.sub().map(|sub| format!("{nome} · {sub}"))
    }

    fn ocupada(&self) -> bool {
        TelaTabela::ocupada(self)
    }

    fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, dados, vista, .. } = p.as_mut();
            dados.em_segundo_plano(egui, ctx, tabela);
            if let Some(v) = vista {
                v.parte().em_segundo_plano(egui, ctx, tabela);
            }
        }
    }
}

/// A altura do cabeçalho de uma vista das Propriedades (a faixa da parte, na altura da `.sec`).
const ALTURA_CABECALHO_VISTA: f32 = interface::pecas::painel::ALTURA_SECAO;

/// Um `Ui` com o recuo `[cima, lados, baixo]` (o `padding` de um bloco das Propriedades).
fn recuado(ui: &mut Ui, [cima, lados, baixo]: [f32; 3], conteudo: impl FnOnce(&mut Ui)) {
    let topo = ui.cursor().min;
    let r = egui::Rect::from_min_max(
        topo + egui::vec2(lados, cima),
        egui::pos2(ui.max_rect().right() - lados, f32::INFINITY),
    );
    let mut dentro = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
    conteudo(&mut dentro);
    let altura = dentro.min_rect().height().max(0.0) + cima + baixo;
    ui.allocate_space(egui::vec2(ui.available_width(), altura));
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn subtitulo_como_o_javascript() {
        assert_eq!(subtitulo(Some(60), 7, false, false), "60 linhas · 7 colunas");
        assert_eq!(subtitulo(Some(10), 7, true, false), "10 linhas · 7 colunas · sem rowid");
        assert_eq!(subtitulo(Some(1), 1, false, true), "1 linha · 1 coluna · visão");
        assert_eq!(subtitulo(None, 4, false, false), "4 colunas");
        assert_eq!(subtitulo(Some(5000), 4, false, false), "5.000 linhas · 4 colunas");
    }

    #[test]
    fn banco_trocado_nao_relanca_a_leitura() {
        use egui_kittest::kittest::Queryable;
        use estado::apoio_testes::{Ambiente, assentar, palco};
        let amb = Ambiente::exemplo("tabela_banco_trocado");
        // outro lugar fechou o banco: o id que a tela tem ficou velho e a leitura volta `BancoTrocado`
        amb.servico.fechar_banco(&amb.estado.banco_id(), true).expect("fechar");
        let tela = TelaTabela::nova(Rota::objeto("clientes", false));
        let mut h = palco(&amb, tela, interface::tema::Tema::Claro, egui::vec2(1200.0, 700.0), |ui, ctx, t| {
            t.ui(ui, ctx);
        });
        assentar(&mut h);
        assentar(&mut h);
        let trocados = h.state().acoes.iter().filter(|a| matches!(a, Acao::BancoTrocado)).count();
        assert_eq!(trocados, 1, "um aviso à casca, sem laço: {:?}", h.state().acoes);
        assert!(!h.state().tela.ocupada(), "a tela não pede de novo com o mesmo id");
        assert!(matches!(h.state().tela.situacao, Situacao::Esperando));
        assert!(h.query_by_label("Lendo a tabela…").is_some());
    }

    /// C6: no quadro em que a leitura chega, a parte (de rolagem própria, que enche a altura) não é desenhada ainda:
    /// dentro da página da casca ela deixaria o cursor no infinito. Fica o "Lendo a tabela…" e um quadro a pedir.
    #[test]
    fn o_quadro_em_que_a_tabela_fica_pronta_nao_desenha_a_parte() {
        use egui_kittest::kittest::Queryable;
        use estado::apoio_testes::{Ambiente, palco};
        let amb = Ambiente::exemplo("tabela_quadro_da_leitura");
        let tela = TelaTabela::nova(Rota::objeto("clientes", false));
        let mut h = palco(&amb, tela, interface::tema::Tema::Claro, egui::vec2(1200.0, 700.0), |ui, ctx, t| {
            t.ui(ui, ctx);
        });
        let inicio = std::time::Instant::now();
        let mut passadas = 0;
        while h.state().tela.tabela_aberta().is_none() {
            assert!(inicio.elapsed() < std::time::Duration::from_secs(15), "a tabela não chegou");
            std::thread::sleep(std::time::Duration::from_millis(10));
            let antes = h.ctx.cumulative_pass_nr();
            h.step();
            passadas = h.ctx.cumulative_pass_nr() - antes;
        }
        // Com uma passada no quadro da chegada, ela ainda mostra o carregando e pede o quadro seguinte. Se o egui
        // repetiu a passada no mesmo quadro (um descarte de layout, que depende das fontes do sistema), a segunda já
        // não é a da chegada e desenha a parte, com a anatomia lida de novo pela casca.
        if passadas == 1 {
            assert!(h.query_by_label("Lendo a tabela…").is_some(), "no quadro da chegada, ainda o carregando");
            assert!(h.ctx.has_requested_repaint(), "e o quadro seguinte pedido");
        }
        h.run_steps(3);
        assert!(h.query_by_label("Lendo a tabela…").is_none(), "depois, a parte");
    }

    #[test]
    fn recarregar_rele_com_a_rota_atual() {
        use estado::apoio_testes::{Ambiente, assentar, palco};
        let amb = Ambiente::exemplo("tabela_recarregar");
        let tela = TelaTabela::nova(Rota::objeto("clientes", false));
        let mut h = palco(&amb, tela, interface::tema::Tema::Claro, egui::vec2(1200.0, 700.0), |ui, ctx, t| {
            t.ui(ui, ctx);
        });
        assentar(&mut h);
        assert!(h.state().tela.tabela_aberta().is_some());
        amb.servico.sql(&amb.estado.banco_id(), "CREATE INDEX idx_teste ON clientes (cidade);", 1).expect("índice");
        h.state_mut().tela.recarregar();
        assert!(h.state().tela.tabela_aberta().is_none(), "volta a ler");
        assentar(&mut h);
        let indices = h.state().tela.tabela_aberta().map(|t| t.info().indices.len()).unwrap_or_default();
        assert!(
            h.state().tela.tabela_aberta().is_some_and(|t| t.info().indices.iter().any(|i| i.nome == "idx_teste")),
            "relida com o índice novo ({indices})"
        );
    }
}
