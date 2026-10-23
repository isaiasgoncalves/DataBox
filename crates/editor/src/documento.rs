//! O documento de uma aba: as telas (Início, Visão geral, tabela ou visão, Consulta SQL, a ficha do arquivo de dados)
//! atrás de um enum só, dentro
//! das `estado::Abas` da casca. Desde a etapa 3, cada tela implementa o trait [`estado::Documento`] (as partes de
//! tabela, o [`estado::ParteTabela`]): elas dizem a própria anatomia, as ofertas, as Propriedades, o status e a
//! busca da barra, e o enum só despacha por [`Documento::doc`] e [`Documento::doc_ref`]. O que é do ciclo de vida
//! das abas fica aqui, porque pede os tipos concretos: `novo`, `serve_para`, `titulo`, `marcar_para_reler`,
//! `mostra_tabela`, `executando`, `esquecer_transacao`, `refeito` e `ao_fechar`.
//!
//! A casca só desenha o documento da aba ativa. Os das outras abas ficam parados, com o que já leram; a cada quadro
//! eles recebem só o [`Documento::em_segundo_plano`] (a Consulta SQL toma as respostas que chegam).
//!
//! # Os verbos da barra de ferramentas
//!
//! | Aba | Ligados | O resto |
//! |---|---|---|
//! | Início | nenhum | desligado |
//! | Visão geral | Novo ("Nova tabela", desligado em só leitura), Atualizar, Exportar ("Exportar o banco", desligado sem tabelas nem visões) | desligado |
//! | Tabela ou visão | Exportar ("Exportar clientes", depois de lida), Atualizar, mais o que a parte oferece | desligado |
//! | Consulta SQL | Executar (o principal) | desligado (cada card tem o seu "Exportar…") |
//! | Ficha do arquivo | nenhum | desligado |
//! | Editor de arquivo (CSV, texto; `tela_arquivo::TelaArquivo`) | o que a tela oferece (Salvar, Desfazer, Refazer, Atualizar…) | desligado |
//!
//! A Nova coluna tem o lugar dela na barra e fica desligada em todas as abas por enquanto: ela liga na Estrutura (e
//! o Novo desliga lá); no editor de CSV (etapa 5), ela insere a coluna depois da selecionada. Atualizar espera o que
//! a tela já está lendo ou executando (desligado enquanto isso).

use banco::ResultadoConsulta;
use estado::{Contexto, Estado, IdAba, Ofertas, Promessa, Rota, Verbo};

use crate::inicio::TelaInicio;
use crate::tabela::TelaTabela;
use crate::tela_arquivo::TelaArquivo;
use crate::telas::TelaConsulta;
use crate::visao_geral::{Execucao, TelaBanco};

/// O que uma aba mostra.
pub(crate) enum Documento {
    Inicio(Box<TelaInicio>),
    Banco(Box<TelaBanco>),
    Tabela(Box<TelaTabela>),
    Consulta(Box<TelaConsulta>),
    /// Um arquivo de dados (`Rota::Arquivo`): a ficha ou o editor do tipo ([`TelaArquivo`], decisão 26 da etapa 5).
    Arquivo(Box<TelaArquivo>),
}

impl Documento {
    /// O documento da rota, recém-criado: ele pede ao serviço o que precisa quando for desenhado. `erro_inicial` é
    /// o do banco da linha de comando que não abriu, e só o Início o mostra. A Consulta SQL ganha ids próprios, com
    /// o id da aba como sal (o texto, o cursor e o desfazer não passam de uma Consulta para outra).
    pub(crate) fn novo(id: IdAba, rota: &Rota, erro_inicial: Option<String>) -> Documento {
        match rota {
            Rota::Inicio => Documento::Inicio(Box::new(TelaInicio::nova(erro_inicial))),
            Rota::Arquivo { caminho } => Documento::Arquivo(Box::new(TelaArquivo::nova(caminho))),
            Rota::Banco => Documento::Banco(Box::new(TelaBanco::nova())),
            r @ (Rota::Tabela { .. } | Rota::Visao { .. }) => Documento::Tabela(Box::new(TelaTabela::nova(r.clone()))),
            Rota::Consulta { sql } => Documento::Consulta(Box::new(TelaConsulta::nova(sql.clone()).com_id(id.0))),
        }
    }

    /// A aba de um arquivo de dados com um editor dado no lugar da tela do tipo (o documento de mentira dos testes
    /// da casca: `Editor::com_editor_de_teste`).
    pub(crate) fn de_arquivo_com(caminho: &std::path::Path, editor: Box<dyn estado::Documento>) -> Documento {
        Documento::Arquivo(Box::new(TelaArquivo::com_editor(caminho, editor)))
    }

    /// O documento tem alterações não salvas (o ponto da aba, a fila de perguntas ao fechar).
    pub(crate) fn alterado(&self) -> bool {
        self.doc_ref().alterado()
    }

    /// O editor de arquivo tem uma pergunta própria aberta (o "Salvar mesmo assim"): a fila ao fechar espera.
    pub(crate) fn pergunta_aberta(&self) -> bool {
        matches!(self, Documento::Arquivo(t) if t.pergunta_aberta())
    }

    /// A tela, pelo contrato delas.
    pub(crate) fn doc(&mut self) -> &mut dyn estado::Documento {
        match self {
            Documento::Inicio(t) => t.as_mut(),
            Documento::Banco(t) => t.as_mut(),
            Documento::Tabela(t) => t.as_mut(),
            Documento::Consulta(t) => t.as_mut(),
            Documento::Arquivo(t) => t.doc(),
        }
    }

    /// A tela, pelo contrato delas, só para ler.
    pub(crate) fn doc_ref(&self) -> &dyn estado::Documento {
        match self {
            Documento::Inicio(t) => t.as_ref(),
            Documento::Banco(t) => t.as_ref(),
            Documento::Tabela(t) => t.as_ref(),
            Documento::Consulta(t) => t.as_ref(),
            Documento::Arquivo(t) => t.doc_ref(),
        }
    }

    /// A faixa, o rodapé e a rolagem que o documento pede.
    pub(crate) fn anatomia(&self) -> estado::Anatomia {
        self.doc_ref().anatomia()
    }

    /// O documento serve para esta rota (a aba pode trocar de endereço sem ser refeita): o Início, a Visão geral e a
    /// Consulta SQL só para as rotas deles; a tela de tabela, para qualquer tabela ou visão (ela mesma corrige o tipo
    /// e a parte).
    pub(crate) fn serve_para(&self, rota: &Rota) -> bool {
        matches!(
            (self, rota),
            (Documento::Inicio(_), Rota::Inicio)
                | (Documento::Banco(_), Rota::Banco)
                | (Documento::Tabela(_), Rota::Tabela { .. } | Rota::Visao { .. })
                | (Documento::Consulta(_), Rota::Consulta { .. })
                | (Documento::Arquivo(_), Rota::Arquivo { .. })
        )
    }

    /// O nome e o sub da aba na faixa: ("Início", None), ("vendas.db", Some("Visão geral")), ("clientes",
    /// Some("Dados")), ("Consulta 2", None). `rota` e `numero` são os da aba (`AbaAberta::rota` e
    /// `numero_consulta`).
    pub(crate) fn titulo(&self, rota: &Rota, numero: Option<u32>, estado: &Estado) -> (String, Option<String>) {
        match self {
            Documento::Inicio(_) => ("Início".into(), None),
            Documento::Banco(_) => match &estado.banco {
                Some(b) => (b.nome.clone(), Some("Visão geral".into())),
                None => ("Visão geral".into(), None),
            },
            // uma aba por objeto (revisão de UI de 19/10/2026): só o nome, sem a parte
            Documento::Tabela(_) => (rota.nome_objeto().unwrap_or_default().to_string(), None),
            Documento::Consulta(_) => match numero {
                Some(n) => (format!("Consulta {n}"), None),
                None => ("Consulta".into(), None),
            },
            Documento::Arquivo(t) => (t.nome().to_string(), None),
        }
    }

    /// O que a aba diz de si na barra de status: "clientes · 60 linhas · 7 colunas" na tabela ou visão (depois de
    /// lida), "SQL" na Consulta SQL; nada no Início e na Visão geral.
    pub(crate) fn status(&self, estado: &Estado) -> Option<String> {
        self.doc_ref().status(estado)
    }

    /// O que a aba oferece de cada verbo da barra de ferramentas agora (a tabela no topo do módulo). Os textos são
    /// fixos ou levam o nome da tabela; nada vai ao serviço.
    pub(crate) fn ofertas(&self, estado: &Estado) -> Ofertas {
        self.doc_ref().ofertas(estado)
    }

    /// Executa o verbo (um clique na barra de ferramentas, um atalho), antes de a aba se desenhar no quadro; o que
    /// ele pede à casca vai pelo `ctx`. Um verbo que a aba não oferece agora não faz nada.
    pub(crate) fn executar(&mut self, verbo: Verbo, egui: &egui::Context, ctx: &mut Contexto) {
        if !self.ofertas(ctx.estado).ligado(verbo) {
            return;
        }
        self.doc().executar(verbo, egui, ctx);
        egui.request_repaint();
    }

    /// O documento tem o painel de Propriedades: o Início não tem (o painel não é desenhado e o documento fica com a
    /// largura dele).
    pub(crate) fn tem_propriedades(&self) -> bool {
        self.doc_ref().tem_propriedades()
    }

    /// Há tarefas do documento rodando (para os testes esperarem). A execução da Consulta SQL não conta: ela pode
    /// demorar quanto o comando quiser, e a casca não espera por ela.
    pub(crate) fn ocupada(&self) -> bool {
        self.doc_ref().ocupada()
    }

    /// A cada quadro, com a aba fora de vista: a Consulta SQL toma as respostas que chegaram (a sondagem, o
    /// Histórico, a execução), com as ações delas (`NotarTransacao`, `RecarregarObjetos`), sem desenhar nada; a
    /// tela de tabela passa à parte dela (os Dados gravam e dão o toast na hora); a Visão geral dá o toast da
    /// manutenção ou da cópia que terminou. O Início espera a aba voltar.
    pub(crate) fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        self.doc().em_segundo_plano(egui, ctx);
    }

    /// Outra aba mudou o banco (a estrutura de uma tabela, tabelas novas, uma execução da Consulta SQL): o documento
    /// relê quando a aba aparecer. A tela de tabela guarda a página, a ordem e a busca (`TelaTabela::recarregar`); a
    /// Visão geral pede de novo o banco e os objetos, sem perder a manutenção ou a cópia em andamento nem o resultado
    /// à vista (`TelaBanco::reler`). Os outros não leem nada do banco por conta própria.
    pub(crate) fn marcar_para_reler(&mut self) {
        match self {
            Documento::Tabela(t) => t.recarregar(),
            Documento::Banco(t) => t.reler(),
            Documento::Arquivo(t) => t.reler(),
            Documento::Inicio(_) | Documento::Consulta(_) => {}
        }
    }

    /// A tela de tabela já leu a tabela (as linhas, a estrutura): o que ela mostra pode ter ficado velho.
    pub(crate) fn mostra_tabela(&self) -> bool {
        matches!(self, Documento::Tabela(t) if t.tabela_aberta().is_some())
    }

    /// A Consulta SQL executa (também com a resposta já pronta, ainda não tomada por um quadro).
    pub(crate) fn executando(&self) -> bool {
        matches!(self, Documento::Consulta(t) if t.executando())
    }

    /// Outra aba mudou a transação (todas as Consultas usam a mesma conexão): a Consulta SQL volta a seguir o
    /// `BancoInfo` do estado.
    pub(crate) fn esquecer_transacao(&mut self) {
        if let Documento::Consulta(t) = self {
            t.esquecer_transacao();
        }
    }

    /// O documento refeito para o banco reaberto por fora no mesmo arquivo (os pedidos dele levavam o id de antes):
    /// a tela de tabela e a Visão geral pela rota da aba (só leem quando aparecerem); a Consulta SQL com o texto do
    /// editor (os resultados se perdem). O Início fica como está (None).
    pub(crate) fn refeito(&self, id: IdAba, rota: &Rota) -> Option<Documento> {
        match self {
            // o Início e a ficha não são do banco: ficam
            Documento::Inicio(_) | Documento::Arquivo(_) => None,
            Documento::Consulta(t) => {
                let texto = (!t.texto().is_empty()).then(|| t.texto().to_string());
                Some(Documento::Consulta(Box::new(TelaConsulta::nova(texto).com_id(id.0))))
            }
            Documento::Banco(_) | Documento::Tabela(_) => Some(Documento::novo(id, rota, None)),
        }
    }

    /// O documento vai sair (a aba fechou ou a casca o refaz): a execução da Consulta SQL que ainda corre, para a
    /// casca receber a resposta no lugar dele. A transação e as contagens do Explorador ainda valem.
    pub(crate) fn ao_fechar(&mut self) -> Option<Promessa<ResultadoConsulta>> {
        match self {
            Documento::Consulta(t) if t.executando() => t.soltar_execucao(),
            _ => None,
        }
    }

    /// O documento vai sair: a manutenção ou a cópia de segurança da Visão geral que ainda corre, para a casca
    /// esperar por ela no lugar da tela e avisar o resultado num toast, em vez de o perder com a aba.
    pub(crate) fn execucao_ao_fechar(&mut self) -> Option<Execucao> {
        match self {
            Documento::Banco(t) => t.soltar_execucao(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use estado::Parte;
    use estado::apoio_testes::Ambiente;
    use estado::{Acao, Anatomia};

    use crate::telas::DICA_EXECUTAR;
    use crate::visao_geral::DICA_EXPORTAR_BANCO;

    fn titulo(rota: Rota, numero: Option<u32>, estado: &Estado) -> (String, Option<String>) {
        Documento::novo(IdAba(1), &rota, None).titulo(&rota, numero, estado)
    }

    fn par(nome: &str, sub: Option<&str>) -> (String, Option<String>) {
        (nome.to_string(), sub.map(str::to_string))
    }

    #[test]
    fn cada_rota_vira_o_seu_documento() {
        let tipo = |rota: Rota| match Documento::novo(IdAba(7), &rota, None) {
            Documento::Inicio(_) => "inicio",
            Documento::Banco(_) => "banco",
            Documento::Tabela(_) => "tabela",
            Documento::Consulta(_) => "consulta",
            Documento::Arquivo(_) => "arquivo",
        };
        assert_eq!(tipo(Rota::Inicio), "inicio");
        assert_eq!(tipo(Rota::arquivo(r"C:\dados\vendas.csv")), "arquivo");
        assert_eq!(tipo(Rota::Banco), "banco");
        assert_eq!(tipo(Rota::objeto("clientes", false)), "tabela");
        assert_eq!(tipo(Rota::objeto("v", true)), "tabela");
        assert_eq!(tipo(Rota::consulta_com("SELECT 1")), "consulta");
        // recém-criados, nenhum está ocupado nem tem execução para soltar; a barra de status só ouve a Consulta (a
        // tabela fala depois de lida)
        let amb = Ambiente::vazio("documento_rotas");
        for (rota, status) in [
            (Rota::Inicio, None),
            (Rota::Banco, None),
            (Rota::objeto("clientes", false), None),
            (Rota::Consulta { sql: None }, Some("SQL")),
        ] {
            let mut d = Documento::novo(IdAba(7), &rota, None);
            assert!(!d.ocupada());
            assert!(d.ao_fechar().is_none());
            assert_eq!(d.status(&amb.estado).as_deref(), status);
        }
    }

    #[test]
    fn serve_para_a_rota_e_refeito_pela_rota() {
        let tabela = Rota::objeto("clientes", false);
        let d = Documento::novo(IdAba(3), &tabela, None);
        assert!(d.serve_para(&tabela) && d.serve_para(&Rota::objeto_na_aba("v", true, Parte::Definicao)));
        assert!(!d.serve_para(&Rota::Banco) && !d.serve_para(&Rota::Inicio));
        let inicio = Documento::novo(IdAba(1), &Rota::Inicio, None);
        assert!(inicio.serve_para(&Rota::Inicio) && !inicio.serve_para(&Rota::Banco));
        // a ficha não é do banco: fica como está quando ele é reaberto
        let a_csv = estado::apoio_testes::abs("dados/a.csv");
        let ficha = Documento::novo(IdAba(5), &Rota::arquivo(&a_csv), None);
        assert!(ficha.serve_para(&Rota::arquivo(&a_csv)) && !ficha.serve_para(&Rota::Inicio));
        assert!(ficha.refeito(IdAba(5), &Rota::arquivo(&a_csv)).is_none());
        assert_eq!(ficha.titulo(&Rota::arquivo(&a_csv), None, &Ambiente::vazio("titulo_ficha").estado).0, "a.csv");
        let c = Documento::novo(IdAba(4), &Rota::consulta_com("SELECT 42"), None);
        assert!(c.serve_para(&Rota::Consulta { sql: None }) && !c.serve_para(&tabela));
        // o banco reaberto por fora: o Início fica; a Consulta volta com o texto; a tabela, pela rota
        assert!(inicio.refeito(IdAba(1), &Rota::Inicio).is_none());
        match c.refeito(IdAba(4), &Rota::Consulta { sql: None }) {
            Some(Documento::Consulta(t)) => assert_eq!(t.texto(), "SELECT 42"),
            _ => panic!("a Consulta refeita"),
        }
        assert!(matches!(d.refeito(IdAba(3), &tabela), Some(Documento::Tabela(_))));
        assert!(matches!(
            Documento::novo(IdAba(2), &Rota::Banco, None).refeito(IdAba(2), &Rota::Banco),
            Some(Documento::Banco(_))
        ));
    }

    fn ligados(o: &Ofertas) -> Vec<Verbo> {
        o.iter().filter(|(_, oferta)| oferta.ligado).map(|(v, _)| v).collect()
    }

    #[test]
    fn ofertas_de_cada_documento() {
        let amb = Ambiente::exemplo("documento_ofertas");
        let estado = &amb.estado;
        // o Início não oferece nada e não tem Propriedades
        let inicio = Documento::novo(IdAba(1), &Rota::Inicio, None);
        assert_eq!(inicio.ofertas(estado), Ofertas::default());
        assert!(!inicio.tem_propriedades());
        // a Visão geral: Nova tabela, Buscar (as tabelas), Atualizar e Exportar o banco
        let banco = Documento::novo(IdAba(2), &Rota::Banco, None);
        let o = banco.ofertas(estado);
        assert_eq!(ligados(&o), [Verbo::Novo, Verbo::Buscar, Verbo::Atualizar, Verbo::Exportar]);
        assert_eq!((o.rotulo(Verbo::Novo), o.dica(Verbo::Novo)), ("Nova tabela".into(), "Nova tabela".into()));
        assert_eq!(o.dica(Verbo::Exportar), DICA_EXPORTAR_BANCO);
        assert_eq!(o.dica(Verbo::Atualizar), "Atualizar (F5)");
        assert!(banco.tem_propriedades());
        // só leitura, sem a Nova tabela; sem tabelas nem visões, sem Exportar
        let mut leitura = estado.clone();
        if let Some(b) = leitura.banco.as_mut() {
            b.somente_leitura = true;
        }
        assert_eq!(ligados(&banco.ofertas(&leitura)), [Verbo::Buscar, Verbo::Atualizar, Verbo::Exportar]);
        let mut vazio = estado.clone();
        if let Some(o) = vazio.objetos.as_mut() {
            o.tabelas.clear();
            o.visoes.clear();
        }
        assert!(!banco.ofertas(&vazio).ligado(Verbo::Exportar));
        // a tabela: Atualizar e Exportar "Exportar clientes", este só depois de lida
        let tabela = Documento::novo(IdAba(3), &Rota::objeto("clientes", false), None);
        let o = tabela.ofertas(estado);
        assert_eq!(ligados(&o), [Verbo::Atualizar]);
        assert_eq!(o.dica(Verbo::Exportar), "Exportar clientes");
        assert_eq!(o.rotulo(Verbo::Novo), "Novo");
        // a Consulta SQL com texto (vazia, o Executar desliga): o Executar, principal, com a dica do trecho selecionado
        let consulta = Documento::novo(IdAba(4), &Rota::Consulta { sql: Some("SELECT 1".into()) }, None);
        let o = consulta.ofertas(estado);
        // a Consulta de verdade busca no editor; o substituto não tem busca
        let esperados: &[Verbo] =
            if cfg!(feature = "telas") { &[Verbo::Buscar, Verbo::Executar] } else { &[Verbo::Executar] };
        assert_eq!(ligados(&o), esperados);
        assert!(o.get(Verbo::Executar).principal && !o.get(Verbo::Atualizar).principal);
        assert_eq!(o.dica(Verbo::Executar), DICA_EXECUTAR);
        // a Nova coluna tem o lugar dela, desligada em todas as abas na etapa 2
        for d in [&inicio, &banco, &tabela, &consulta] {
            let o = d.ofertas(estado);
            assert!(!o.ligado(Verbo::NovaColuna));
            assert_eq!(
                (o.rotulo(Verbo::NovaColuna), o.dica(Verbo::NovaColuna)),
                ("Nova coluna".into(), "Nova coluna".into())
            );
        }
    }

    #[test]
    fn executar_pede_a_casca_o_que_cada_verbo_faz() {
        let amb = Ambiente::exemplo("documento_executar");
        let egui = egui::Context::default();
        let executar = |d: &mut Documento, v: Verbo| {
            let mut acoes = Vec::new();
            d.executar(v, &egui, &mut Contexto::novo(&amb.servico, &amb.estado, None, &mut acoes));
            acoes
        };
        let mut banco = Documento::novo(IdAba(2), &Rota::Banco, None);
        assert!(matches!(executar(&mut banco, Verbo::Novo).as_slice(), [Acao::AbrirNovaTabela]));
        match executar(&mut banco, Verbo::Exportar).as_slice() {
            [Acao::AbrirExportacao(p)] => {
                assert_eq!((&p.origem, p.nome_sugerido.as_str()), (&estado::OrigemJanelaExportar::Banco, "exemplo"));
            }
            outra => panic!("o pedido da janela Exportar: {outra:?}"),
        }
        // o Atualizar pede à casca o BancoInfo e os objetos na hora, sem refazer a Visão geral
        assert!(matches!(
            executar(&mut banco, Verbo::Atualizar).as_slice(),
            [Acao::RecarregarBanco, Acao::RecarregarObjetos]
        ));
        // os desligados não fazem nada
        for v in [Verbo::Salvar, Verbo::NovaColuna, Verbo::Executar] {
            assert!(executar(&mut banco, v).is_empty(), "{v:?}");
        }
        // a tabela ainda não lida: o Atualizar relê e a casca relê os objetos; o Exportar espera a leitura
        let mut tabela = Documento::novo(IdAba(3), &Rota::objeto("clientes", false), None);
        assert!(executar(&mut tabela, Verbo::Exportar).is_empty());
        assert!(matches!(executar(&mut tabela, Verbo::Atualizar).as_slice(), [Acao::RecarregarObjetos]));
        // o Início não faz nada
        let mut inicio = Documento::novo(IdAba(1), &Rota::Inicio, None);
        for v in Verbo::TODOS {
            assert!(executar(&mut inicio, v).is_empty(), "{v:?}");
        }
    }

    #[test]
    fn titulo_de_cada_documento() {
        let amb = Ambiente::exemplo("documento_titulo");
        let estado = &amb.estado;
        let banco = estado.banco.as_ref().map(|b| b.nome.clone()).unwrap_or_default();
        assert!(!banco.is_empty());
        assert_eq!(titulo(Rota::Inicio, None, estado), par("Início", None));
        assert_eq!(titulo(Rota::Banco, None, estado), par(&banco, Some("Visão geral")));
        // uma aba por objeto (revisão de UI de 19/10/2026): o título é só o objeto, em qualquer parte
        assert_eq!(titulo(Rota::objeto("clientes", false), None, estado), par("clientes", None));
        let estrutura = Rota::objeto_na_aba("clientes", false, Parte::Estrutura);
        assert_eq!(titulo(estrutura, None, estado), par("clientes", None));
        let definicao = Rota::objeto_na_aba("v", true, Parte::Definicao);
        assert_eq!(titulo(definicao, None, estado), par("v", None));
        assert_eq!(titulo(Rota::Consulta { sql: None }, Some(2), estado), par("Consulta 2", None));
        // sem banco (a casca não deixa chegar aqui), a Visão geral fica sem o sub
        let mut sem_banco = estado.clone();
        sem_banco.banco = None;
        assert_eq!(titulo(Rota::Banco, None, &sem_banco), par("Visão geral", None));
    }

    #[test]
    fn cada_documento_diz_a_anatomia() {
        // a Visão geral: na página, sem faixa (U4 da revisão de UI de 22/10/2026) nem rodapé
        let banco = Documento::novo(IdAba(5), &Rota::Banco, None);
        assert_eq!(banco.doc_ref().anatomia(), Anatomia::default());
        // o Início e a tabela, por enquanto, são a página sem faixa nem rodapé (passos 10 e 11)
        for rota in [Rota::Inicio, Rota::objeto("clientes", false)] {
            let d = Documento::novo(IdAba(5), &rota, None);
            assert_eq!(d.doc_ref().anatomia(), Anatomia::default(), "{rota:?}");
        }
        // a Consulta SQL (passo 13): faixa, rodapé e rolagem própria
        let consulta = Documento::novo(IdAba(5), &Rota::Consulta { sql: None }, None);
        assert_eq!(consulta.anatomia(), Anatomia::default().com_faixa().com_rodape().rolagem_propria());
    }

    #[test]
    fn ofertas_vem_das_telas() {
        // o enum só despacha: as ofertas, as Propriedades e o status são os das telas
        let amb = Ambiente::exemplo("ofertas_das_telas");
        let estado = &amb.estado;
        for rota in [Rota::Inicio, Rota::Banco, Rota::objeto("clientes", false), Rota::Consulta { sql: None }] {
            let d = Documento::novo(IdAba(6), &rota, None);
            assert_eq!(d.ofertas(estado), d.doc_ref().ofertas(estado), "{rota:?}");
            assert_eq!(d.tem_propriedades(), d.doc_ref().tem_propriedades());
            assert_eq!(d.status(estado), d.doc_ref().status(estado));
        }
        let banco = estado::Documento::ofertas(&TelaBanco::nova(), estado);
        assert_eq!(banco.rotulo(Verbo::Novo), "Nova tabela");
        let consulta = estado::Documento::ofertas(&TelaConsulta::nova(None), estado);
        assert!(consulta.get(Verbo::Executar).principal);
        let tabela = estado::Documento::ofertas(&TelaTabela::nova(Rota::objeto("pedidos", false)), estado);
        assert_eq!(tabela.dica(Verbo::Exportar), "Exportar pedidos");
    }
}
