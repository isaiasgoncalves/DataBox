//! A tela de uma tabela ou visão (`#/t/:nome/:aba`, `#/v/:nome/:aba`): carrega a
//! tabela pelo serviço e entrega a parte da aba (Dados, Estrutura, Índices e gatilhos, Definição SQL) ao crate dela
//! com a [`TabelaAberta`], pelo contrato [`ParteTabela`]: a faixa, o conteúdo e o rodapé da parte no documento da
//! casca, o Atualizar e o Exportar ("Exportar clientes") junto das ofertas da parte, e nas Propriedades a seção
//! "Tabela" antes das da parte e o "Arquivo" curto depois. A tabela que não existe mais vira o card "A tabela X não
//! existe mais neste banco." com o link para a Visão geral.

use banco::{Contagem, TipoObjeto};
use egui::Ui;
use estado::{
    Aba, Acao, BuscaDaBarra, Contexto, Estado, EventoBusca, Oferta, Ofertas, ParteTabela, PedidoJanelaExportar,
    Promessa, Reacao, Rota, TabelaAberta, Verbo,
};
use interface::formato::formatar_bytes;
use interface::formato::plural;
use interface::pecas::avisos::{aviso_texto, carregando, vazio};
use interface::pecas::cartao::Cartao;
use interface::pecas::navegacao::{Cabecalho, Passo};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::link;
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

/// A aba ativa, do crate dela.
enum AbaAtiva {
    Dados(Box<AbaDados>),
    Estrutura(Box<AbaEstrutura>),
    Indices(Box<AbaIndices>),
    Definicao(Box<AbaDefinicao>),
}

impl AbaAtiva {
    fn nova(aba: Aba, tabela: &TabelaAberta) -> AbaAtiva {
        match aba {
            Aba::Dados => AbaAtiva::Dados(Box::new(AbaDados::nova(tabela))),
            Aba::Estrutura => AbaAtiva::Estrutura(Box::new(AbaEstrutura::nova(tabela))),
            Aba::Indices => AbaAtiva::Indices(Box::new(AbaIndices::nova(tabela))),
            Aba::Definicao => AbaAtiva::Definicao(Box::new(AbaDefinicao::nova(tabela))),
        }
    }

    /// A parte, pelo contrato delas.
    fn parte(&mut self) -> &mut dyn ParteTabela {
        match self {
            AbaAtiva::Dados(a) => a.as_mut(),
            AbaAtiva::Estrutura(a) => a.as_mut(),
            AbaAtiva::Indices(a) => a.as_mut(),
            AbaAtiva::Definicao(a) => a.as_mut(),
        }
    }

    fn parte_ref(&self) -> &dyn ParteTabela {
        match self {
            AbaAtiva::Dados(a) => a.as_ref(),
            AbaAtiva::Estrutura(a) => a.as_ref(),
            AbaAtiva::Indices(a) => a.as_ref(),
            AbaAtiva::Definicao(a) => a.as_ref(),
        }
    }
}

struct Pronta {
    tabela: TabelaAberta,
    aba: AbaAtiva,
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
    /// A rota pedida (corrigida depois de carregar, se o objeto for do outro tipo ou a aba não existir nele).
    rota: Rota,
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
        TelaTabela { rota, carga: None, contagem: None, situacao: Situacao::Carregando, arquivo: None }
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
        let lista: &[Aba] = if e_visao { &Aba::DA_VISAO } else { &Aba::DA_TABELA };
        let pedida = self.rota.aba().unwrap_or_default();
        let aba = if lista.contains(&pedida) { pedida } else { Aba::Dados };
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
        let aba_ativa = AbaAtiva::nova(aba, &tabela);
        self.situacao = Situacao::Pronta(Box::new(Pronta { tabela, aba: aba_ativa, sub }));
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
                let Pronta { tabela, aba, .. } = p.as_mut();
                aba.parte().conteudo(ui, ctx, tabela);
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

/// A tela de tabela como documento da casca: junta Atualizar e Exportar ("Exportar clientes", depois de lida), que
/// valem para toda tabela, às ofertas da parte, e desenha a seção "Tabela" das Propriedades antes das da parte e o
/// "Arquivo" curto depois.
impl estado::Documento for TelaTabela {
    /// A da parte, depois de lida (antes, a página sem faixa nem rodapé, com o "Lendo a tabela…").
    fn anatomia(&self) -> estado::Anatomia {
        match &self.situacao {
            Situacao::Pronta(p) => p.aba.parte_ref().anatomia(),
            _ => estado::Anatomia::default(),
        }
    }

    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, aba, .. } = p.as_mut();
            aba.parte().faixa(ui, ctx, tabela);
        }
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ui(ui, ctx);
    }

    fn rodape(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, aba, .. } = p.as_mut();
            aba.parte().rodape(ui, ctx, tabela);
        }
    }

    fn ofertas(&self, estado: &Estado) -> Ofertas {
        let da_parte = match &self.situacao {
            Situacao::Pronta(p) => p.aba.parte_ref().ofertas(&p.tabela, estado),
            _ => Ofertas::default(),
        };
        let exportar =
            Oferta::ligada().ligada_se(self.tabela_aberta().is_some()).com_dica(format!("Exportar {}", self.nome()));
        da_parte
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
            // a tabela relê (com a página, a ordem e a busca da aba Dados) e a casca relê os objetos (as outras abas
            // da mesma tabela releem quando aparecerem)
            Verbo::Atualizar => {
                self.recarregar();
                ctx.emitir(Acao::RecarregarObjetos);
            }
            _ => {
                if let Situacao::Pronta(p) = &mut self.situacao {
                    let Pronta { tabela, aba, .. } = p.as_mut();
                    aba.parte().executar(verbo, egui, ctx, tabela);
                }
            }
        }
    }

    /// A busca da barra é a da parte ("Buscar em clientes", "Buscar colunas"); antes de a tabela ser lida, e nas
    /// partes sem busca (Índices e gatilhos, Definição SQL), desligada.
    fn busca(&mut self) -> Option<BuscaDaBarra<'_>> {
        match &mut self.situacao {
            Situacao::Pronta(p) => p.aba.parte().busca(),
            _ => None,
        }
    }

    fn evento_busca(&mut self, evento: EventoBusca, egui: &egui::Context, ctx: &mut Contexto) {
        if let Situacao::Pronta(p) = &mut self.situacao {
            let Pronta { tabela, aba, .. } = p.as_mut();
            aba.parte().evento_busca(evento, egui, ctx, tabela);
        }
    }

    fn tem_propriedades(&self) -> bool {
        true
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
        match &mut self.situacao {
            Situacao::Pronta(p) => {
                let Pronta { tabela, aba, .. } = p.as_mut();
                let (parte, lida) = (aba.parte(), &*tabela);
                estado::secao_tabela(ui, lida, |ui| parte.acoes_da_tabela(ui, ctx, lida));
                aba.parte().propriedades(ui, ctx, tabela);
            }
            // antes de ser lida, só o nome
            _ => {
                let _ = Secao::new("propriedades::tabela", "Tabela").primeira(true).mostrar(ui, |ui| {
                    ficha(ui, |d| d.linha_texto("Nome", Some(&nome), "—"));
                });
            }
        }
        let (caminho, tamanho) = self.arquivo.as_ref().map(|(_, c, t)| (c.as_str(), t.as_str())).unwrap_or_default();
        let _ = Secao::new("propriedades::arquivo_curto", "Arquivo").mostrar(ui, |ui| {
            ficha(ui, |d| {
                d.linha_texto("Caminho", Some(caminho), "—");
                d.linha_texto("Tamanho", Some(tamanho), "—");
            });
        });
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
            let Pronta { tabela, aba, .. } = p.as_mut();
            aba.parte().em_segundo_plano(egui, ctx, tabela);
        }
    }
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
        while h.state().tela.tabela_aberta().is_none() {
            assert!(inicio.elapsed() < std::time::Duration::from_secs(15), "a tabela não chegou");
            std::thread::sleep(std::time::Duration::from_millis(10));
            h.step();
        }
        assert!(h.query_by_label("Lendo a tabela…").is_some(), "no quadro da chegada, ainda o carregando");
        assert!(h.ctx.has_requested_repaint(), "e o quadro seguinte pedido");
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
