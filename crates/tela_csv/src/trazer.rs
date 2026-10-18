//! O "Trazer colunas" no documento (passo 13): abrir a janela (o verbo da barra, o menu do cabeçalho, o Ctrl+Shift+V
//! e a seção Coluna), as fontes (a deste CSV, que a casca junta das abas; a área de transferência; "Outro arquivo"),
//! aplicar o plano num passo só e o toast "3 colunas trazidas · 188 linhas sem par" com "Marcar as linhas sem par".

use std::sync::{Arc, Mutex};

use estado::{Contexto, FonteDeColunas, Promessa};
use interface::formato::plural;
use interface::pecas::toast::{AcaoToast, acao_do_toast, toast_concluido, toast_concluido_com_acao};
use tabular::colunas::{atualizar_coluna, colunas_coladas, ler_tsv};
use tabular::csv::{OpcoesAbertura, Tabela};

use crate::fonte_csv::{FonteCsv, FonteTexto, Resumo};
use crate::janela_trazer::{DestinoTrazido, JanelaTrazer, PedidoDaJanela, PlanoTrazer};
use crate::tela::TelaCsv;

/// O começo da carga da ação do toast do Trazer (a casca deixa passar o que começa com isto para o documento).
pub const PREFIXO_ACAO_TOAST: &str = "tela_csv:";
/// O rótulo da ação do toast.
pub const MARCAR_SEM_PAR: &str = "Marcar as linhas sem par";

/// Aplica o plano do Trazer na tabela (dentro da tarefa, num passo só): as colunas novas no fim, com o que veio (a
/// linha sem par fica vazia), e as atualizações por cima das colunas que existem (substituir ou só as vazias).
/// Devolve se o desfazer ganhou o passo: atualizar com os mesmos valores não muda nada (nem deixa passo).
pub(crate) fn aplicar_plano(t: &mut Tabela, plano: &PlanoTrazer) -> comum::Resultado<bool> {
    if plano.colunas.is_empty() {
        return Ok(false);
    }
    let antes = t.alteracoes();
    t.em_um_passo(|t| {
        for (origem, destino) in &plano.colunas {
            let trazidos = plano.casamento.trazer(origem.as_slice(), plano.repetidas);
            match destino {
                DestinoTrazido::Nova { nome } => {
                    let valores: Vec<String> = trazidos.into_iter().map(Option::unwrap_or_default).collect();
                    let fim = t.colunas();
                    t.inserir_coluna_com_valores(fim, nome, &valores)?;
                }
                DestinoTrazido::Atualizar { coluna, modo } => {
                    let atual = t.colunas_de(&[*coluna], &mut |_, _| Ok(()))?.pop().unwrap_or_default();
                    let a = atualizar_coluna(&atual, &trazidos, *modo);
                    let mut valores = vec![None; atual.len()];
                    for i in a.alteradas {
                        valores[i] = Some(a.valores[i].clone());
                    }
                    t.preencher_coluna(*coluna, &valores)?;
                }
            }
        }
        Ok(())
    })?;
    Ok(t.alteracoes() != antes)
}

/// "Trazer 3 colunas" / "Trazer 1 coluna" (o rótulo do desfazer e do botão); sem nenhuma marcada, o botão desligado diz
/// só "Trazer colunas" (nunca "Trazer 0 colunas").
pub(crate) fn rotulo_trazer(n: usize) -> String {
    match n {
        0 => "Trazer colunas".to_string(),
        1 => "Trazer 1 coluna".to_string(),
        n => format!("Trazer {n} colunas"),
    }
}

impl TelaCsv {
    /// Põe em dia a fonte deste CSV (a mesma `Arc` sempre): o resumo só muda com a versão (os nomes e o número de
    /// linhas mudam só com o conteúdo), então ele é copiado e o `Mutex` dele travado só quando ela anda.
    pub(crate) fn fonte_deste(&mut self) {
        if self.fonte.is_some() && self.versao_da_fonte == Some(self.versao) {
            return;
        }
        let (Some(tabela), Some(r)) = (self.tabela.as_ref(), self.retrato.as_ref()) else { return };
        let resumo = Resumo { nomes: r.nomes.clone(), linhas: r.linhas };
        match &self.fonte {
            Some(f) => f.atualizar(resumo),
            None => self.fonte = Some(FonteCsv::nova(self.nome.clone(), Arc::clone(tabela), resumo)),
        }
        self.versao_da_fonte = Some(self.versao);
    }

    /// Abre a janela, com a área de transferência como origem quando ela vem (o Colar com correspondência), ou com a
    /// origem de rótulo `preferida` escolhida (o "Trazer colunas desta planilha"); sem nenhuma das duas, a primeira
    /// das outras abas.
    pub(crate) fn abrir_trazer(
        &mut self,
        ectx: &egui::Context,
        ctx: &Contexto,
        texto: Option<&str>,
        preferida: Option<&str>,
    ) {
        if !self.livre_para_editar() || self.retrato.is_none() {
            return;
        }
        let area: Option<Arc<dyn FonteDeColunas>> = texto.map(|t| {
            let decimal = self.retrato.as_ref().map_or(',', |r| r.formato.decimal);
            let origem = crate::colunas::origem_do_texto(ectx, t);
            let colunas = colunas_coladas(&ler_tsv(t), origem, decimal);
            Arc::new(FonteTexto { colunas }) as Arc<dyn FonteDeColunas>
        });
        let outras = self.outras_fontes(ctx);
        let escolhida = preferida.and_then(|r| outras.iter().find(|f| f.rotulo() == r).cloned());
        let fonte = area.clone().or(escolhida).or_else(|| outras.first().cloned());
        let mut janela = JanelaTrazer::nova(fonte, area);
        janela.decimal = self.retrato.as_ref().map_or(',', |r| r.formato.decimal);
        self.trazer = Some(janela);
    }

    /// Abre a janela "Trazer colunas" com a origem de rótulo `rotulo` já escolhida (uma das outras abas: o "Trazer
    /// colunas desta planilha" do menu do cabeçalho de uma planilha, que a casca passa a este CSV). Vale no próximo
    /// quadro em que este documento se desenha com a tabela lida; sem a origem entre as abas, a primeira delas.
    pub fn trazer_colunas_de(&mut self, rotulo: impl Into<String>) {
        self.trazer_de = Some(rotulo.into());
    }

    /// As origens das outras abas (a casca põe todas em `Contexto::fontes`; esta sai da lista).
    fn outras_fontes(&self, ctx: &Contexto) -> Vec<Arc<dyn FonteDeColunas>> {
        let este = self.fonte.as_ref().map(|f| Arc::as_ptr(f) as *const () as usize);
        ctx.fontes().iter().filter(|f| Some(Arc::as_ptr(f) as *const () as usize) != este).cloned().collect()
    }

    /// A janela aberta: as tarefas dela, o desenho e o que ela pediu.
    pub(crate) fn desenhar_trazer(&mut self, ectx: &egui::Context, ctx: &mut Contexto) {
        // fechada (quase sempre), nada a copiar neste quadro
        if self.trazer.is_none() {
            return;
        }
        let outras = self.outras_fontes(ctx);
        let versao = self.versao;
        // campos diferentes da tela: a janela pega o retrato e a página emprestados, sem copiá-los a cada quadro
        let (Some(janela), Some(tabela), Some(r)) = (self.trazer.as_mut(), self.tabela.as_ref(), self.retrato.as_ref())
        else {
            return;
        };
        let amostra: &[Vec<String>] =
            self.pagina.as_ref().filter(|p| p.inicio == 0).map_or(&[], |p| p.linhas.as_slice());
        janela.atualizar(ectx, tabela, &r.nomes, amostra, versao);
        let pedido = janela.mostrar(ectx, &self.nome, &r.nomes, r.linhas, &outras);
        match pedido {
            Some(PedidoDaJanela::Fechar) => {
                if let Some(j) = self.trazer.as_mut() {
                    j.cancelar_calculo();
                }
                self.trazer = None;
                self.focar_grade = true;
            }
            Some(PedidoDaJanela::Trazer(plano)) => {
                self.trazer = None;
                self.mutar(crate::edicao::Mutacao::Trazer(plano));
            }
            Some(PedidoDaJanela::OutroArquivo) => self.escolher_outro_arquivo(ectx, ctx),
            None => {}
        }
    }

    /// O "Outro arquivo": o diálogo do Windows (nos testes, o caminho do gancho) e a leitura do CSV numa tarefa.
    fn escolher_outro_arquivo(&mut self, ectx: &egui::Context, ctx: &Contexto) {
        let caminho = match self.ganchos.outro_arquivo.clone() {
            Some(c) => Some(c),
            None => ctx.dialogos().abrir_arquivo().map(|e| e.caminho),
        };
        let (Some(caminho), Some(janela)) = (caminho, self.trazer.as_mut()) else { return };
        let promessa = Promessa::rodar(ectx, move || abrir_fonte(&caminho));
        janela.abrindo_outro = Some(promessa);
    }

    /// Depois do Trazer: o toast com o "Marcar as linhas sem par" (as marcas da decisão 12). As linhas sem par andam
    /// com as alterações seguintes como as marcas (`depois_da_tarefa`), e o desfazer, o refazer e reler as esquecem.
    pub(crate) fn depois_do_trazer(&mut self, ectx: &egui::Context, plano: &PlanoTrazer) {
        let n = plano.colunas.len();
        let sem_par = plano.casamento.sem_par.len();
        let colunas = if n == 1 { "1 coluna trazida".to_string() } else { format!("{n} colunas trazidas") };
        self.sem_par = plano.casamento.sem_par.iter().map(|&i| i as u64).collect();
        if sem_par == 0 {
            toast_concluido(ectx, colunas);
            return;
        }
        let frase = format!("{colunas} · {} sem par", plural(sem_par as u64, "linha", "linhas"));
        let acao = AcaoToast {
            rotulo: MARCAR_SEM_PAR.to_string(),
            carga: format!("{PREFIXO_ACAO_TOAST}marcar_sem_par:{}", self.id),
        };
        toast_concluido_com_acao(ectx, frase, None, acao);
    }

    /// A ação do toast, quando é deste documento (a casca a deixa passar: veja [`PREFIXO_ACAO_TOAST`]).
    pub(crate) fn acao_do_toast(&mut self, ectx: &egui::Context) {
        let minha = format!("{PREFIXO_ACAO_TOAST}marcar_sem_par:{}", self.id);
        let Some(carga) = acao_do_toast(ectx) else { return };
        // esquecidas (um desfazer depois do Trazer), o toast velho não apaga as marcas de agora
        if carga == minha && !self.sem_par.is_empty() {
            self.marcadas = self.sem_par.iter().copied().collect();
        }
    }

    /// A janela "Trazer colunas" está aberta.
    pub fn trazer_aberta(&self) -> bool {
        self.trazer.is_some()
    }

    // ---- o que os testes leem e mexem na janela (sem passar pelos seletores)

    #[doc(hidden)]
    pub fn trazer_origem(&self) -> Option<(String, Vec<String>)> {
        let j = self.trazer.as_ref()?;
        Some((j.fonte.as_ref()?.rotulo(), j.nomes_origem().to_vec()))
    }

    #[doc(hidden)]
    pub fn trazer_chaves(&self) -> Option<(Option<usize>, Option<usize>)> {
        self.trazer.as_ref().map(|j| (j.chave_destino, j.chave_origem))
    }

    #[doc(hidden)]
    pub fn trazer_definir_chaves(&mut self, destino: usize, origem: usize) {
        if let Some(j) = self.trazer.as_mut() {
            j.chave_destino = Some(destino);
            j.chave_origem = Some(origem);
        }
    }

    /// O medidor: (com par, linhas deste arquivo).
    #[doc(hidden)]
    pub fn trazer_medidor(&self) -> Option<(usize, usize)> {
        let c = self.trazer.as_ref()?.casamento()?;
        Some((c.com_par(), c.total()))
    }

    #[doc(hidden)]
    pub fn trazer_marcar(&mut self, coluna: usize, sim: bool) {
        if let Some(j) = self.trazer.as_mut() {
            j.marcar(coluna, sim);
        }
    }

    /// O destino de uma coluna da origem: atualizar `coluna` deste arquivo (só as vazias, ou substituir).
    #[doc(hidden)]
    pub fn trazer_atualizar(&mut self, origem: usize, coluna: usize, so_vazias: bool) {
        let modo = if so_vazias {
            tabular::colunas::ModoAtualizacao::SoCelulasVazias
        } else {
            tabular::colunas::ModoAtualizacao::Substituir
        };
        if let Some(j) = self.trazer.as_mut() {
            j.definir_destino(origem, DestinoTrazido::Atualizar { coluna, modo });
        }
    }

    /// O nome da coluna nova de uma coluna da origem (`None`: o destino é atualizar).
    #[doc(hidden)]
    pub fn trazer_nome_novo(&self, origem: usize) -> Option<String> {
        self.trazer.as_ref()?.destino_de(origem).and_then(|d| match d {
            DestinoTrazido::Nova { nome } => Some(nome.clone()),
            DestinoTrazido::Atualizar { .. } => None,
        })
    }

    #[doc(hidden)]
    pub fn trazer_repetidas(&mut self, modo: tabular::colunas::ChavesRepetidas) {
        if let Some(j) = self.trazer.as_mut() {
            j.repetidas = modo;
        }
    }

    #[doc(hidden)]
    pub fn trazer_opcoes(&mut self) -> Option<&mut tabular::colunas::OpcoesCasamento> {
        self.trazer.as_mut().map(|j| &mut j.opcoes)
    }
}

/// Uma origem de colunas de um CSV no disco (o "Outro arquivo" do Trazer colunas; a casca também pode usar):
/// abre o arquivo (lê o disco: chame numa tarefa).
pub fn abrir_fonte(caminho: &std::path::Path) -> comum::Resultado<Arc<dyn FonteDeColunas>> {
    let tabela = Tabela::abrir(caminho, &OpcoesAbertura::default())?;
    let resumo = Resumo { nomes: tabela.nomes(), linhas: tabela.linhas() };
    let rotulo = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(FonteCsv::nova(rotulo, Arc::new(Mutex::new(tabela)), resumo) as Arc<dyn FonteDeColunas>)
}

#[cfg(test)]
mod testes {
    #[test]
    fn rotulo_do_botao_trazer_sem_zero() {
        assert_eq!(super::rotulo_trazer(0), "Trazer colunas");
        assert_eq!(super::rotulo_trazer(1), "Trazer 1 coluna");
        assert_eq!(super::rotulo_trazer(3), "Trazer 3 colunas");
    }
}
