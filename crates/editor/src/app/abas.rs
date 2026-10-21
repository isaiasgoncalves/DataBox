//! As abas de documento: navegar (abre a aba da rota ou foca a aberta), recomeçar e refazer as abas numa troca
//! de banco, fechar uma aba, trocar o endereço dela (`Substituir`, `Atualizar`), as releituras pedidas por
//! outra aba (`RecarregarObjetos`), a lista de objetos nova e o estado da transação (`NotarTransacao`).

use std::collections::HashSet;
use std::sync::Arc;

use banco::ObjetosInfo;
use estado::{ChaveAba, FonteDeColunas, IdAba, Rota};

use crate::casca;
use crate::casca::docas::Painel;
use crate::documento::Documento;

use super::tarefas::Tarefa;
use super::{Editor, Situacao, terminar_edicao};

impl Editor {
    /// Sem banco, toda rota além do Início volta ao Início.
    fn rota_valida(&self, rota: Rota) -> Rota {
        if rota.exige_banco() && !self.estado.tem_banco() { Rota::Inicio } else { rota }
    }

    /// O erro do banco da linha de comando, só para o Início (o primeiro que aparecer o mostra).
    fn erro_para(&mut self, rota: &Rota) -> Option<String> {
        if matches!(rota, Rota::Inicio) { self.erro_inicial.take() } else { None }
    }

    /// O documento que sai: a execução da Consulta SQL que ainda corre continua numa tarefa da casca, com o id do
    /// banco para que foi pedida. A resposta ainda acerta a transação e os objetos, e é descartada se o banco
    /// mudou nesse meio-tempo.
    pub(super) fn soltar(&mut self, mut documento: Documento, id_banco: &str) {
        if let Some(p) = documento.ao_fechar() {
            self.tarefas.push(Tarefa::Consulta(id_banco.to_string(), p));
        }
        // a manutenção ou a cópia de segurança da Visão geral também termina sem a tela: o resultado vira um toast
        if let Some(e) = documento.execucao_ao_fechar() {
            self.tarefas.push(Tarefa::VisaoGeral(id_banco.to_string(), e));
        }
    }

    /// Recomeça as abas e o histórico numa rota: o estado que chegou ao iniciar e as trocas de banco (as rotas
    /// antigas eram de outro banco). As abas do banco fecham, inclusive as Consultas; as de arquivo de dados ficam
    /// (decisão 22 da etapa 4: não são do banco). As execuções que ainda correm seguem com `id_antes`, o id do banco
    /// de antes (a resposta é descartada na chegada).
    pub(super) fn reiniciar_abas(&mut self, rota: Rota, id_antes: &str) {
        let rota = self.rota_valida(rota);
        let erro = self.erro_para(&rota);
        let de_arquivo = |a: &estado::AbaAberta<Documento>| matches!(a.chave(), Some(ChaveAba::Arquivo(_)));
        let fechadas = self.abas.reiniciar_menos(rota, |id, r| Documento::novo(id, r, erro), de_arquivo);
        for aba in fechadas {
            self.soltar(aba.conteudo, id_antes);
        }
        self.consulta_da_transacao = None;
        self.situacao = Situacao::Pronta;
    }

    /// O banco foi reaberto por fora no mesmo arquivo: as abas e o histórico ficam (os ids delas não mudam), e cada
    /// documento é refeito pela rota da aba ([`Documento::refeito`]; o Início fica). Nenhum pedido com o id do banco
    /// de antes sobra para voltar `BancoTrocado` de novo: as execuções soltas seguem com `id_antes`.
    pub(super) fn refazer_documentos(&mut self, id_antes: &str) {
        let mut soltos = Vec::new();
        for aba in self.abas.iter_mut() {
            if let Some(novo) = aba.conteudo.refeito(aba.id(), aba.rota()) {
                soltos.push(std::mem::replace(&mut aba.conteudo, novo));
            }
        }
        for documento in soltos {
            self.soltar(documento, id_antes);
        }
        // a transação era da conexão de antes
        self.consulta_da_transacao = None;
        self.situacao = Situacao::Pronta;
    }

    /// Abre a aba da rota ou foca a que já está aberta, sem refazer o documento nem trocar o endereço dela. A
    /// Consulta SQL sem comando (o "Ir para a Consulta SQL" do aviso de transação) foca a que abriu a transação, depois
    /// a usada mais recentemente; sem nenhuma, abre uma.
    pub(super) fn navegar(&mut self, rota: Rota) {
        let rota = self.rota_valida(rota);
        if let Rota::Consulta { sql: None } = rota
            && let Some(id) = self.consulta_da_transacao.filter(|&id| self.abas.get(id).is_some())
        {
            self.abas.focar(id);
            return;
        }
        let mut erro = self.erro_para(&rota);
        let de_teste = self.editor_de_teste.clone();
        // a tabela pedida em outra parte (a Estrutura, os Índices, a Definição): a aba do objeto, com a parte à vista
        // nas Propriedades (revisão de UI de 19/10/2026)
        let parte = rota.aba().filter(|&p| p != estado::Parte::Dados);
        self.abas.abrir(rota, |id, r| {
            let editor = match (r, de_teste.as_deref()) {
                (Rota::Arquivo { caminho }, Some(f)) => f(caminho).map(|e| Documento::de_arquivo_com(caminho, e)),
                _ => None,
            };
            editor.unwrap_or_else(|| Documento::novo(id, r, erro.take()))
        });
        // o Início já aberto não leva o erro: ele espera o próximo
        if erro.is_some() {
            self.erro_inicial = erro;
        }
        if let Some(parte) = parte
            && let Documento::Tabela(t) = &mut self.abas.ativa_mut().conteudo
        {
            t.mostrar_parte(parte);
            self.paineis.mostrar(Painel::Propriedades);
        }
    }

    /// Uma Consulta SQL nova, sempre (o "+" da faixa de abas, um item do Histórico); sem banco, o Início.
    pub(super) fn nova_consulta(&mut self, sql: Option<String>) {
        if !self.estado.tem_banco() {
            self.navegar(Rota::Inicio);
            return;
        }
        self.abas.nova_consulta(sql, |id, r| Documento::novo(id, r, None));
    }

    /// As origens de colunas das abas abertas (o "Trazer colunas"), na ordem das abas: uma `Arc` por aba, montadas a
    /// cada quadro para o `Contexto::fontes`.
    pub(super) fn fontes_de_colunas(&self) -> Vec<Arc<dyn FonteDeColunas>> {
        if !matches!(self.situacao, Situacao::Pronta) {
            return Vec::new();
        }
        self.abas.iter().filter_map(|a| a.conteudo.doc_ref().fonte_de_colunas()).collect()
    }

    /// O "Trazer colunas desta planilha" (o menu do cabeçalho de uma planilha): foca o CSV ativo, ou o usado mais
    /// recentemente, com a janela Trazer colunas aberta e a planilha (`rotulo`, a `FonteDeColunas` dela) como origem.
    /// Sem CSV aberto não há o que fazer (o item do menu fica desligado com "Abra um CSV para trazer colunas.").
    pub(super) fn trazer_colunas_de(&mut self, ctx: &egui::Context, rotulo: String) {
        let destino =
            self.abas.mais_recente_onde(|a| matches!(&a.conteudo, Documento::Arquivo(t) if t.recebe_trazer()));
        let Some(id) = destino else {
            log::info!("Trazer colunas de {rotulo}: nenhum CSV aberto");
            return;
        };
        if let Some(Documento::Arquivo(t)) = self.abas.get_mut(id).map(|a| &mut a.conteudo)
            && t.trazer_colunas_de(rotulo)
        {
            self.abas.focar(id);
            ctx.request_repaint();
        }
    }

    /// Fecha a aba (a execução da Consulta SQL que ainda corre segue numa tarefa da casca). Fechar a última abre o
    /// Início; o Início sozinho não fecha.
    pub(super) fn fechar_aba(&mut self, id: IdAba) {
        if let Some(aba) = self.abas.fechar(id, |id, r| Documento::novo(id, r, None)) {
            let id_banco = self.estado.banco_id();
            self.soltar(aba.conteudo, &id_banco);
        }
    }

    /// O `Substituir` (`nova_entrada` false) ou o `Atualizar` de uma aba: só o endereço muda, sem refazer o
    /// documento. Vale para a aba de origem (sem ela, a ativa), se o documento dela servir para a rota; se a chave
    /// nova for a de outra aba aberta (uma visão pedida como tabela, que a tela corrigiu), as duas viram uma e a de
    /// origem fecha.
    pub(super) fn trocar_endereco(&mut self, origem: Option<IdAba>, rota: Rota, nova_entrada: bool) {
        if rota.exige_banco() && !self.estado.tem_banco() {
            return;
        }
        let id = origem.unwrap_or_else(|| self.abas.id_ativa());
        if !self.abas.get(id).is_some_and(|a| a.conteudo.serve_para(&rota)) {
            return;
        }
        let fundida = if nova_entrada { self.abas.atualizar(id, rota) } else { self.abas.substituir(id, rota) };
        if let Some(aba) = fundida {
            let id_banco = self.estado.banco_id();
            self.soltar(aba.conteudo, &id_banco);
        }
    }

    /// O `RecarregarObjetos`: a estrutura ou as tabelas mudaram. Vindo de uma aba de tabela (ela mesma já se relê),
    /// as outras abas da mesma tabela releem; vindo de outro lugar (a Consulta SQL, as janelas, a execução solta),
    /// todas as abas de tabela e a Visão geral, menos a de origem. As escondidas releem quando aparecerem; a ativa
    /// (quando não é a de origem: a Consulta escondida que terminou um INSERT, a execução solta) relê no fim do
    /// quadro seguinte ([`Editor::reler_a_ativa`]), depois de a célula em edição dela gravar: a edição termina agora.
    pub(super) fn marcar_para_reler(&mut self, ctx: &egui::Context, origem: Option<IdAba>) {
        let ativa = self.abas.id_ativa();
        let tabela_de_origem = origem
            .and_then(|id| self.abas.get(id))
            .filter(|a| matches!(a.conteudo, Documento::Tabela(_)))
            .and_then(|a| a.rota().nome_objeto().map(str::to_string));
        let mut marcar_a_ativa = false;
        for aba in self.abas.iter_mut() {
            let marcar = Some(aba.id()) != origem
                && match &tabela_de_origem {
                    Some(nome) => aba.rota().nome_objeto() == Some(nome.as_str()),
                    None => true,
                };
            if !marcar {
                continue;
            }
            if aba.id() == ativa {
                marcar_a_ativa = true;
            } else {
                aba.conteudo.marcar_para_reler();
            }
        }
        if marcar_a_ativa {
            self.pedir_releitura_da_ativa(ctx);
        }
    }

    /// A aba ativa relê no fim do quadro seguinte: a edição de texto em curso termina agora, e a célula da grade em
    /// edição grava no quadro seguinte, antes de a tela ser relida.
    fn pedir_releitura_da_ativa(&mut self, ctx: &egui::Context) {
        self.reler_ativa = Some(self.abas.id_ativa());
        terminar_edicao(ctx);
    }

    /// No fim do quadro, depois de o documento se desenhar: a aba que [`Editor::pedir_releitura_da_ativa`] marcou no
    /// quadro anterior relê (se ainda estiver aberta).
    pub(super) fn reler_a_ativa(&mut self, ctx: &egui::Context, pedida: Option<IdAba>) {
        if let Some(aba) = pedida.and_then(|id| self.abas.get_mut(id)) {
            aba.conteudo.marcar_para_reler();
            ctx.request_repaint();
        }
    }

    /// Chegou uma lista de objetos nova do mesmo banco: fecham as abas das tabelas e visões que sumiram dela
    /// (renomeadas ou excluídas), menos a ativa, que relê e mostra o card de que a tabela não existe mais.
    pub(super) fn trocar_objetos(&mut self, ctx: &egui::Context, objetos: ObjetosInfo) {
        let Some(antes) = self.estado.objetos.replace(objetos) else { return };
        let nomes = |o: &ObjetosInfo| -> HashSet<String> {
            let tabelas = o.tabelas.iter().map(|t| t.nome.clone());
            let visoes = o.visoes.iter().map(|v| v.nome.clone());
            tabelas.chain(visoes).chain(o.sistema.iter().map(|s| s.nome.clone())).collect()
        };
        let agora = self.estado.objetos.as_ref().map(nomes).unwrap_or_default();
        let sumidos: HashSet<String> = nomes(&antes).difference(&agora).cloned().collect();
        if sumidos.is_empty() {
            return;
        }
        let ativa = self.abas.id_ativa();
        let sumiu = |a: &estado::AbaAberta<Documento>| match a.chave() {
            Some(ChaveAba::Objeto { nome }) => sumidos.contains(nome),
            _ => false,
        };
        // a ativa que ainda mostra as linhas da tabela que sumiu relê (e mostra o card de inexistente)
        let ativa_sumiu = sumiu(self.abas.ativa()) && self.abas.ativa().conteudo.mostra_tabela();
        let fechadas = self.abas.fechar_onde(|a| a.id() != ativa && sumiu(a), |id, r| Documento::novo(id, r, None));
        let id_banco = self.estado.banco_id();
        for aba in fechadas {
            self.soltar(aba.conteudo, &id_banco);
        }
        if ativa_sumiu {
            self.pedir_releitura_da_ativa(ctx);
        }
    }

    /// O `NotarTransacao`: o estado da transação segundo a resposta de alguém. Quando muda, as outras Consultas
    /// esquecem o que a última resposta delas dizia (todas usam a mesma conexão) e o BancoInfo é relido. A Consulta
    /// que abre a transação (ou a primeira que a vê aberta) é a que o "Ir para a Consulta SQL" foca.
    pub(super) fn notar_transacao(&mut self, ctx: &egui::Context, origem: Option<IdAba>, aberta: bool) {
        let mudou = self.estado.banco.as_mut().is_some_and(|b| {
            let mudou = b.transacao_aberta != aberta;
            b.transacao_aberta = aberta;
            mudou
        });
        let consulta =
            origem.filter(|&id| self.abas.get(id).is_some_and(|a| matches!(a.conteudo, Documento::Consulta(_))));
        let conhecida = self.consulta_da_transacao.is_some_and(|id| self.abas.get(id).is_some());
        if !aberta {
            self.consulta_da_transacao = None;
        } else if consulta.is_some() && (mudou || !conhecida) {
            self.consulta_da_transacao = consulta;
        }
        if mudou {
            self.mudou_estado(ctx);
            for aba in self.abas.iter_mut().filter(|a| Some(a.id()) != origem) {
                aba.conteudo.esquecer_transacao();
            }
            // as leituras do BancoInfo pedidas até aqui viram da transação de antes
            self.banco_nota = self.leituras_banco;
            self.pedir_banco(ctx);
        }
    }

    /// Pede a leitura do BancoInfo (numerada: ver `leituras_banco`).
    pub(super) fn pedir_banco(&mut self, ctx: &egui::Context) {
        let id = self.estado.banco_id();
        let numero = self.leituras_banco;
        self.leituras_banco += 1;
        self.tarefas.push(Tarefa::Banco(id, numero, self.tarefa(ctx, |s, id| s.info_banco(id))));
    }

    /// O nome da Consulta que abriu a transação, como está na faixa de abas ("Consulta 2"); "Consulta SQL" quando
    /// nenhuma aba a abriu (o aviso e a dica do status citam este nome).
    pub(super) fn nome_da_consulta_da_transacao(&self) -> String {
        let numero = self.consulta_da_transacao.and_then(|id| self.abas.get(id)).and_then(|a| a.numero_consulta());
        casca::nome_da_consulta(numero)
    }

    /// A aba desta chave.
    pub(super) fn documento_da_chave(&mut self, chave: &ChaveAba) -> Option<&mut Documento> {
        let id = self.abas.achar(chave)?;
        self.abas.get_mut(id).map(|a| &mut a.conteudo)
    }
}
