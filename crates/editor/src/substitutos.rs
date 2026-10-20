//! Substitutos das telas dos outros crates (tela_dados, tela_esquema, tela_consulta, tela_exportar), com a
//! mesma API do §4.8 de ARQUITETURA.md e os mesmos traits (`estado::ParteTabela` nas partes, `estado::Documento` na
//! Consulta SQL): usados quando o recurso `telas` está desligado, para a casca ser desenvolvida e testada sem
//! depender do estado daqueles crates. São cópias dos esboços.

use estado::{
    Acao, Contexto, Estado, Oferta, Ofertas, ParteTabela, PedidoJanelaExportar, Promessa, TabelaAberta, Verbo,
};
use interface::pecas::botao::Botao;
use interface::pecas::janela::Janela;

/// A aba Dados de uma tabela ou visão.
pub struct AbaDados {
    nome: String,
}

impl AbaDados {
    pub fn nova(tabela: &TabelaAberta) -> AbaDados {
        AbaDados { nome: tabela.nome.clone() }
    }
}

impl ParteTabela for AbaDados {
    fn conteudo(&mut self, ui: &mut egui::Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {
        ui.label(format!("Dados de {} (a fazer)", self.nome));
    }
}

/// A tabela pode mudar de estrutura (o substituto: a tabela comum, com o banco gravando).
pub fn pode_editar(info: &banco::TabelaInfo, somente_leitura: bool) -> bool {
    info.esquema_editavel && !somente_leitura
}

pub struct AbaEstrutura;
pub struct AbaIndices;
pub struct AbaDefinicao;

impl AbaEstrutura {
    pub fn nova(_tabela: &TabelaAberta) -> AbaEstrutura {
        AbaEstrutura
    }
}

impl ParteTabela for AbaEstrutura {
    fn conteudo(&mut self, ui: &mut egui::Ui, _ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        ui.label(format!("Estrutura de {} (a fazer)", tabela.nome));
    }
}

impl AbaIndices {
    pub fn nova(_tabela: &TabelaAberta) -> AbaIndices {
        AbaIndices
    }
}

impl ParteTabela for AbaIndices {
    fn conteudo(&mut self, ui: &mut egui::Ui, _ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        ui.label(format!("Índices de {} (a fazer)", tabela.nome));
    }
}

impl AbaDefinicao {
    pub fn nova(_tabela: &TabelaAberta) -> AbaDefinicao {
        AbaDefinicao
    }
}

impl ParteTabela for AbaDefinicao {
    fn conteudo(&mut self, ui: &mut egui::Ui, _ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        ui.label(format!("Definição de {} (a fazer)", tabela.nome));
    }
}

/// Janela "Nova tabela".
pub struct JanelaNovaTabela;

impl JanelaNovaTabela {
    pub fn nova() -> JanelaNovaTabela {
        JanelaNovaTabela
    }
    /// Desenha a janela; false quando ela fechou.
    pub fn mostrar(&mut self, egui: &egui::Context, _ctx: &mut Contexto) -> bool {
        let r = Janela::new("nova-tabela", "Nova tabela").mostrar(egui, |ui| {
            ui.label("(a fazer)");
        });
        !r.pediu_fechar
    }
}

/// Janela "Novo índice". O substituto cria de verdade um índice `idx_<tabela>_<coluna>` nas colunas pedidas
/// pelo botão "Criar índice" e, como a de verdade, emite `RecarregarObjetos` e fecha (para os testes da casca
/// conferirem que a tabela aberta é relida).
pub struct JanelaNovoIndice {
    tabela: String,
    colunas: Vec<String>,
    criando: Option<Promessa<()>>,
}

impl JanelaNovoIndice {
    pub fn nova(tabela: String, colunas: Vec<String>) -> JanelaNovoIndice {
        JanelaNovoIndice { tabela, colunas, criando: None }
    }
    /// Desenha a janela; false quando ela fechou.
    pub fn mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto) -> bool {
        if let Some(r) = self.criando.as_mut().and_then(Promessa::tomar) {
            self.criando = None;
            match r {
                Ok(()) => {
                    ctx.emitir(Acao::RecarregarObjetos);
                    return false;
                }
                Err(e) => {
                    let _ = ctx.reagir(&e);
                }
            }
        }
        let mut criar = false;
        let r = Janela::new("novo-indice", "Novo índice")
            .rodape_direita(|ui| {
                if ui.add(Botao::primario("Criar índice")).clicked() {
                    criar = true;
                }
            })
            .mostrar(egui, |ui| {
                ui.label(format!("Tabela {} (a fazer)", self.tabela));
            });
        if criar && self.criando.is_none() {
            let tabela = self.tabela.clone();
            let colunas = self.colunas.clone();
            self.criando = Some(ctx.tarefa(egui, move |s, id| {
                let nome = format!("idx_{tabela}_{}", colunas.join("_"));
                let lista: Vec<String> = colunas.iter().map(|c| comum::q(c)).collect();
                let sql = format!("CREATE INDEX {} ON {} ({})", comum::q(&nome), comum::q(&tabela), lista.join(", "));
                s.sql(id, &sql, 1)?;
                Ok(())
            }));
        }
        !r.pediu_fechar
    }
}

/// A dica do Executar da barra de ferramentas (a mesma da tela_consulta).
pub const DICA_EXECUTAR: &str = "Executar (Ctrl+Enter); com texto selecionado, executa só o trecho";

/// A Consulta SQL.
pub struct TelaConsulta {
    sql: String,
}

/// A Consulta SQL como documento, com a anatomia da de verdade (faixa, rodapé e rolagem própria): o Executar
/// principal (que não executa nada), o Exportar desligado, "SQL" no status e a seção "Execução" nas Propriedades.
impl estado::Documento for TelaConsulta {
    fn anatomia(&self) -> estado::Anatomia {
        estado::Anatomia::default().com_faixa().com_rodape().rolagem_propria()
    }

    fn faixa(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        if let Some(b) = &ctx.estado.banco {
            ui.label(b.nome.as_str());
        }
    }

    fn conteudo(&mut self, ui: &mut egui::Ui, _ctx: &mut Contexto) {
        ui.label(format!("Consulta SQL (a fazer): {}", self.sql));
    }

    fn ofertas(&self, _estado: &Estado) -> Ofertas {
        Ofertas::default()
            .com(
                Verbo::Executar,
                Oferta::ligada().ligada_se(!self.executando()).como_principal().com_dica(DICA_EXECUTAR),
            )
            .com(Verbo::Exportar, Oferta::default().com_dica("Exportar o resultado"))
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    fn propriedades(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        use interface::pecas::painel::{Secao, ficha};
        let aberta = ctx.estado.transacao_aberta();
        let _ = Secao::new("propriedades::execucao", "Execução").primeira(true).mostrar(ui, |ui| {
            ficha(ui, |d| d.linha_texto("Transação", Some(if aberta { "aberta" } else { "nenhuma aberta" }), "—"));
        });
    }

    fn status(&self, _estado: &Estado) -> Option<String> {
        Some("SQL".into())
    }
}

impl TelaConsulta {
    pub fn nova(sql: Option<String>) -> TelaConsulta {
        TelaConsulta { sql: sql.unwrap_or_default() }
    }

    pub fn executando(&self) -> bool {
        false
    }

    pub fn soltar_execucao(&mut self) -> Option<estado::Promessa<banco::ResultadoConsulta>> {
        None
    }

    /// Os ids próprios da aba (o substituto não tem ids a separar).
    pub fn com_id(self, _sal: u64) -> Self {
        self
    }

    /// O texto do editor (a casca o guarda ao refazer a Consulta depois de o banco ser reaberto por fora).
    pub fn texto(&self) -> &str {
        &self.sql
    }

    /// Outra Consulta mudou a transação (o substituto não a guarda).
    pub fn esquecer_transacao(&mut self) {}
}

/// O comando numa linha só (o Histórico de consultas da casca): os espaços juntados, cortado com "…" depois de 72
/// caracteres, como o `resumo_comando` da tela_consulta.
pub fn resumo_comando(sql: &str) -> String {
    const MAX: usize = 72;
    let linha = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if linha.chars().count() > MAX {
        let mut curto: String = linha.chars().take(MAX - 1).collect();
        curto.push('…');
        curto
    } else {
        linha
    }
}

/// A janela Exportar.
pub struct JanelaExportar {
    pedido: PedidoJanelaExportar,
}

impl JanelaExportar {
    pub fn nova(pedido: PedidoJanelaExportar) -> JanelaExportar {
        JanelaExportar { pedido }
    }

    /// Desenha a janela; false quando ela fechou.
    pub fn mostrar(&mut self, egui: &egui::Context, _ctx: &mut Contexto) -> bool {
        let titulo = format!("Exportar {}", self.pedido.nome_sugerido);
        let r = Janela::new("exportar", &titulo).mostrar(egui, |ui| {
            ui.label("(a fazer)");
        });
        !r.pediu_fechar
    }

    /// Há uma exportação em andamento.
    pub fn exportando(&self) -> bool {
        false
    }
}
