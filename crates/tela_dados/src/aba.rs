//! A aba Dados: estado, tarefas de fundo, desenho do documento (a faixa com as
//! condições e o popover delas, a grade, o rodapé), os verbos da barra e o que cada clique e tecla faz.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use banco::{
    CelulaDesfeita, Contagem, Direcao, LinhaCompleta, LinhaGravada, Operador, Pagina, PedidoPagina, TabelaInfo,
    TipoChave, TipoObjeto,
};
use comum::{Celula, Chave, Codigo, Entrada};
use egui::{Align, Id, Layout, Margin, Pos2, Rect, Sense, Ui, UiBuilder, ViewportCommand, pos2, vec2};
use estado::{
    Acao, Anatomia, ConsultaDados, Contexto, FiltroDigitado, FormatoArquivo, LIMIAR_TABELA_GRANDE, MSG_SO_NA_TRANSACAO,
    Oferta, Ofertas, Promessa, Reacao, TabelaAberta, Verbo,
};
use interface::formato::{formatar_numero, formatar_segundos, nome_do_caminho, plural};
use interface::grade::{
    AcaoCelula, Intervalo, OfertaCelula, ResumoSelecao, colagem, frase_de_fora, menu_da_celula, rodape_da_selecao,
    texto_celula, texto_colado, texto_exibicao,
};
use interface::icones::{self, EstadoIcone, Icone};
use interface::pecas::avisos::{aviso_texto, carregando, vazio, vazio_da_grade};
use interface::pecas::botao::Botao;
use interface::pecas::campos::Select;
use interface::pecas::filtros::{
    CondicaoEmEdicao, ContagemFiltros, EtiquetaFiltro, FimCondicao, filtros as desenhar_filtros, popover_condicao,
};
use interface::pecas::janela::{Confirmacao, confirmar};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::rotulos::pill;
use interface::pecas::texto::texto as texto_simples;
use interface::pecas::toast::{
    AcaoToast, acao_do_toast, devolver_acao_do_toast, toast, toast_atencao, toast_concluido,
    toast_concluido_com_acao_por, toast_erro,
};
use interface::tema::paleta;
use servico::{BlobSalvo, Gravado};

use crate::Ganchos;
use crate::desfazer::{
    self, DICA_EXCLUIR, DICA_INCLUIR, Desfazivel, PRAZO_TOAST, ROTULO_ACAO, Situacao, UltimaEdicao, UltimoLote,
    frase_desfeito_lote, frase_gravado, frase_gravado_lote,
};
use crate::grade::{GradeDados, Pendente, Pintor, Piscada, preparar_edicao};
use crate::janela::{AcaoJanela, AvisoJanela, JanelaLinha, ModoJanela};
use crate::menu::{
    self, Escrita, EscritaDoIntervalo, FonteCopia, LIMITE_SEM_PERGUNTA, Lote, MOTIVO_OCUPADO, MOTIVO_SO_LEITURA,
    NADA_MUDOU, TipoLote, montar_lote,
};
use crate::modelo::{
    AvisarAoSair, ChavesDaFila, Dados, Edicao, FimEdicao, Marcadas, Sinal, chave_na_fila, coluna_que_abre,
    trocar_na_fila,
};
use crate::regras::{
    AVISO_SEM_CHAVE, BUSCA_LENTA_MS, CONTANDO, DICA_BUSCA_LENTA, ESPERA_DIGITACAO, LENDO, LINHAS_NAO_CONTADAS,
    LinhaFiltro, OPERADORES, ORDENAR_LENTO_MS, VAZIO_FILTRANDO, VAZIO_LIMPAR_FILTROS, VAZIO_NOVA_LINHA, VAZIO_TABELA,
    VAZIO_VISAO, chave_filtro, chave_texto, descricao_exclusao, dica_duplicar, dica_excluir, dica_filtrar,
    extensao_do_blob, filtros_completos, intervalo, regras_em_pills, rotulo_operador, tamanhos_por_pagina, tem_quebra,
    texto_contagem_faixa, texto_contagem_linhas, texto_contagem_sem_total, texto_edicao, texto_pagina, titulo_linha,
};

/// A altura da linha da dica de página lenta, acima do rodapé (o fio de cima e a linha do botão pequeno).
const ALTURA_DICA_LENTA: f32 = 38.0;
/// O espaço entre as contas da seleção e o "por página" no rodapé.
const ESPACO_RESUMO: f32 = 12.0;
/// A altura mínima da grade (o cabeçalho e uma linha), numa janela muito baixa.
const ALTURA_GRADE_MIN: f32 = 58.0;

/// A página pedida ao serviço.
struct Carga {
    pedido: u64,
    chave: String,
    busca: String,
    filtros: Vec<FiltroDigitado>,
    adiar: bool,
    conhecido: Option<u64>,
    pedido_pagina: PedidoPagina,
    promessa: Promessa<Pagina>,
}

/// A contagem à parte (tabela grande com busca ou filtro).
struct Contando {
    chave: String,
    busca: String,
    filtros: Vec<FiltroDigitado>,
    promessa: Promessa<Contagem>,
}

/// Uma gravação no lugar na fila.
struct GravacaoCelula {
    geracao: u64,
    linha: usize,
    coluna: usize,
    nome_coluna: String,
    /// O valor de antes da edição (o Desfazer o grava de volta).
    original: Celula,
    foco: bool,
    promessa: Promessa<Gravado<LinhaGravada>>,
}

/// O Desfazer da última edição (ou do último lote), a caminho do banco.
enum Desfazendo {
    Edicao { edicao: UltimaEdicao, promessa: Promessa<Gravado<LinhaGravada>> },
    Lote { lote: UltimoLote, promessa: Promessa<Gravado<Vec<LinhaGravada>>> },
}

/// O Colar, o Limpar ou o Definir como NULL a caminho do banco (numa transação só).
struct LoteGravando {
    lote: Lote,
    promessa: Promessa<Gravado<Vec<LinhaGravada>>>,
}

/// A linha sendo lida para a janela, ou para a ficha "Nova linha" já preenchida (o Duplicar
/// da barra).
struct Abertura {
    chave: Chave,
    posicao: u64,
    foco: Option<String>,
    duplicar: bool,
    promessa: Promessa<LinhaCompleta>,
}

/// A condição de filtro aberta no popover: a nova ("+ Condição", o Filtrar da barra) ou a de uma
/// etiqueta (a coluna é o índice em `nomes_visiveis`).
type EdicaoCondicao = CondicaoEmEdicao<Operador>;

/// O que a janela da linha mandou fazer e ainda não voltou.
enum TarefaJanela {
    Gravar { serial: u64, nova: bool, chave: Option<Chave>, promessa: Promessa<Gravado<LinhaGravada>> },
    Excluir { serial: u64, chave: Chave, promessa: Promessa<Gravado<usize>> },
    SalvarBlob { serial: u64, promessa: Promessa<BlobSalvo> },
}

/// A pergunta aberta (a janela de confirmação).
enum Pergunta {
    ExcluirMarcadas(Vec<Chave>),
    /// O lote acima de [`LIMITE_SEM_PERGUNTA`] células.
    Lote(Lote),
    ExcluirLinha {
        serial: u64,
    },
}

/// O pedido de foco que atravessa a recarga da tela depois de incluir ou excluir.
#[derive(Debug, Clone, Default)]
struct FocoPendente {
    chave: Option<String>,
    ativa: Option<(usize, usize)>,
}

/// A dica de página lenta, na linha de informação acima do rodapé.
#[derive(Debug, Clone, PartialEq)]
enum Sugestao {
    Indice { coluna: String, texto: String },
    Busca,
}

/// Os textos do rodapé.
#[derive(Debug, Clone, PartialEq)]
struct Rodape {
    /// "Linhas 1–100 de 5.000" (ou o começo, antes do "contando…").
    contagem: String,
    /// O total ainda não chegou: depois do texto, "contando…" (ou "…" com o motivo da falha).
    sem_total: Option<Option<String>>,
    pagina: String,
    anterior: bool,
    proxima: bool,
    sugestao: Option<Sugestao>,
}

/// Retrato do que a aba mostra, para os testes (`#[doc(hidden)]`).
#[doc(hidden)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Retrato {
    /// A contagem da faixa como texto ("41 de 60 linhas", "60 linhas", "contando…", "linhas não contadas"; vazia
    /// antes da página ou com ela sem leitura).
    pub faixa: String,
    pub contagem_linhas: String,
    pub pagina_atual: String,
    pub anterior_habilitado: bool,
    pub proxima_habilitada: bool,
    pub sugestao: Option<String>,
    pub marcadas: usize,
    pub linhas: usize,
    pub carregando: bool,
    pub vazio: Option<String>,
    pub erro: Option<String>,
    /// A célula ativa (linha, coluna de dados).
    pub ativa: Option<(usize, usize)>,
    /// A célula em edição (linha, coluna de dados, texto).
    pub editando: Option<(usize, usize, String)>,
    pub janela: Option<String>,
    pub janela_aviso: Option<String>,
    pub confirmacao: Option<String>,
    pub gravacoes_pendentes: usize,
    pub substituidas: usize,
    pub filtros: usize,
    /// O popover da condição está aberto.
    pub condicao_aberta: bool,
    /// O Desfazer da última edição está a caminho do banco.
    pub desfazendo: bool,
}

/// A aba Dados de uma tabela ou visão.
pub struct AbaDados {
    nome: String,
    e_visao: bool,
    /// Grava (tabela editável com chave): caixas de marcar, edição, "+ Nova linha".
    editavel: bool,
    nomes_visiveis: Vec<String>,
    /// O que vale (está no endereço): página, tamanho, ordem, busca, filtros completos.
    consulta: ConsultaDados,
    /// A consulta como foi escrita na rota pela última vez (para notar uma mudança de fora).
    consulta_escrita: Option<ConsultaDados>,
    busca: String,
    busca_mudou: Option<f64>,
    /// As condições aplicadas (as etiquetas da faixa), cada uma completa.
    filtros: Vec<LinhaFiltro>,
    proximo_filtro: u64,
    condicao: Option<EdicaoCondicao>,
    /// Onde ficou o "+ Condição" no último quadro (o Filtrar da barra prende o popover nele).
    rect_condicao: Option<Rect>,
    focar: Option<Id>,
    iniciada: bool,
    /// Pediu `tabela.recarregar` (incluiu ou excluiu linhas) e espera a casca refazer a aba.
    esperando_recarga: bool,

    pedido: u64,
    geracao: u64,
    carga_pendente: bool,
    carga: Option<Carga>,
    dados: Option<Dados>,
    erro: Option<String>,
    contagens: HashMap<String, u64>,
    contando: Option<Contando>,
    contagens_soltas: Vec<(String, Promessa<Contagem>)>,

    grade: Option<GradeDados>,
    larguras_quadro: Vec<f32>,
    marcadas: Marcadas,
    selecao_mudou: bool,
    edicao: Option<Edicao>,
    gravacoes: VecDeque<GravacaoCelula>,
    pendentes: Vec<Pendente>,
    piscadas: Vec<Piscada>,
    ultimo_sinal: Option<Arc<Sinal>>,
    chaves_da_fila: ChavesDaFila,
    viva: Arc<AtomicBool>,

    abrindo: Option<Abertura>,
    janela: Option<JanelaLinha>,
    tarefa_janela: Option<TarefaJanela>,
    serial_janela: u64,
    confirmacao: Option<Confirmacao>,
    pergunta: Option<Pergunta>,
    exclusao: Option<Promessa<Gravado<usize>>>,
    opcoes_por_pagina: Vec<(u64, String)>,

    /// A impressão da estrutura da tabela ([`desfazer::estrutura_de`]): mudou, o Desfazer desliga.
    estrutura: u64,
    /// O registro do Desfazer como estava no começo do quadro (as ofertas o leem sem o contexto do egui).
    desfazivel: Option<Desfazivel>,
    desfazendo: Option<Desfazendo>,
    lote: Option<LoteGravando>,
    /// A aba está à vista neste quadro (fora dela, o toast da gravação vai sem a ação: um clique nele não acharia
    /// a aba).
    em_vista: bool,

    ganchos: Ganchos,
    substituidas: usize,
    /// A coluna escolhida nas Propriedades (a vista Colunas, revisão de UI de 19/10/2026), acesa na grade.
    destacada: Option<String>,
    /// As contas do rodapé da seleção e para que intervalo, página e gravações a caminho elas valem.
    resumo: Option<((Intervalo, u64, usize), ResumoSelecao)>,
}

impl Drop for AbaDados {
    fn drop(&mut self) {
        // as gravações que ainda estão na fila avisam sozinhas, pelo toast, se a tela já não existe
        self.viva.store(false, Ordering::SeqCst);
    }
}

fn tabela_grande(info: &TabelaInfo) -> bool {
    info.linhas.is_none_or(|n| n > LIMIAR_TABELA_GRANDE)
}

fn id_foco_pendente(nome: &str) -> Id {
    Id::new(("tela_dados::foco_pendente", nome))
}

impl AbaDados {
    /// A aba de uma tabela (ou visão) carregada pela casca; a página é pedida no primeiro quadro, com a página,
    /// a ordem, a busca e os filtros da rota.
    pub fn nova(tabela: &TabelaAberta) -> AbaDados {
        let info = tabela.info();
        let e_visao = tabela.e_visao || info.tipo == TipoObjeto::View;
        let editavel = info.editavel && info.chave.is_some();
        let nomes_visiveis: Vec<String> =
            info.colunas.iter().filter(|c| c.oculta != 1).map(|c| c.nome.clone()).collect();
        let mut consulta = tabela.rota.dados().cloned().unwrap_or_default();
        let consulta_escrita = tabela.rota.dados().cloned();
        consulta.pagina = consulta.pagina.max(1);
        consulta.por_pagina = consulta.por_pagina.clamp(1, 1000);
        if consulta.ordem.as_ref().is_some_and(|o| !nomes_visiveis.contains(o)) {
            consulta.ordem = None;
        }
        if consulta.ordem.is_none() {
            consulta.direcao = Direcao::Asc;
        }
        consulta.filtros.retain(|f| nomes_visiveis.contains(&f.coluna));
        let mut proximo_filtro = 0;
        let filtros = consulta
            .filtros
            .iter()
            .map(|f| {
                proximo_filtro += 1;
                LinhaFiltro { id: proximo_filtro, coluna: f.coluna.clone(), op: f.op, valor: f.valor.clone() }
            })
            .collect();
        let opcoes_por_pagina = opcoes_por_pagina(consulta.por_pagina);
        AbaDados {
            nome: tabela.nome.clone(),
            e_visao,
            editavel,
            nomes_visiveis,
            busca: consulta.busca.clone(),
            consulta,
            consulta_escrita,
            busca_mudou: None,
            filtros,
            proximo_filtro,
            condicao: None,
            rect_condicao: None,
            focar: None,
            iniciada: false,
            esperando_recarga: false,
            pedido: 0,
            geracao: 0,
            carga_pendente: false,
            carga: None,
            dados: None,
            erro: None,
            contagens: HashMap::new(),
            contando: None,
            contagens_soltas: Vec::new(),
            grade: None,
            larguras_quadro: Vec::new(),
            marcadas: Marcadas::default(),
            selecao_mudou: false,
            edicao: None,
            gravacoes: VecDeque::new(),
            pendentes: Vec::new(),
            piscadas: Vec::new(),
            ultimo_sinal: None,
            chaves_da_fila: ChavesDaFila::default(),
            viva: Arc::new(AtomicBool::new(true)),
            abrindo: None,
            janela: None,
            tarefa_janela: None,
            serial_janela: 0,
            confirmacao: None,
            pergunta: None,
            exclusao: None,
            opcoes_por_pagina,
            estrutura: desfazer::estrutura_de(info),
            desfazivel: None,
            desfazendo: None,
            lote: None,
            em_vista: false,
            ganchos: Ganchos::default(),
            substituidas: 0,
            destacada: None,
            resumo: None,
        }
    }

    /// Ganchos de teste: mexer na página lida, segurar a contagem e responder pelos diálogos do Windows.
    #[doc(hidden)]
    pub fn com_ganchos(mut self, ganchos: Ganchos) -> AbaDados {
        self.ganchos = ganchos;
        self
    }

    /// Troca os ganchos de teste com a aba já aberta.
    #[doc(hidden)]
    pub fn ganchos_mut(&mut self) -> &mut Ganchos {
        &mut self.ganchos
    }

    /// Desenha a aba e trata o que a pessoa fez. A página, a ordem, a busca e os filtros vão para
    /// `tabela.rota` (com `Acao::Substituir`, sem entrada no histórico); as linhas marcadas para `tabela.selecao`;
    /// o total da busca para `tabela.definir_contagem_filtrada`.
    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        estado::desenhar_parte_empilhada(self, ui, ctx, tabela);
    }

    /// O conteúdo da parte (o `ParteTabela::conteudo`), no lugar dela no documento.
    fn conteudo_da_parte(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let ectx = ui.ctx().clone();
        let agora = ui.input(|i| i.time);
        if !self.iniciada {
            self.iniciada = true;
            tabela.selecao.clear();
            self.carregar();
        }
        if self.esperando_recarga && !tabela.recarregar {
            // a casca releu a tabela e manteve esta aba: a página é relida aqui
            self.esperando_recarga = false;
            self.carregar();
        }
        self.sincronizar_rota(tabela);
        self.desfazivel = desfazer::ler(&ectx);
        self.em_vista = true;
        self.acao_do_toast(&ectx, ctx);
        self.receber(&ectx, ctx, tabela, agora);
        self.temporizadores(&ectx, ctx, tabela, agora);
        // o foco pedido no quadro anterior vai para um campo que este quadro desenha (pedir o foco de algo que
        // não está na tela deixaria a árvore de acessibilidade apontando para o nada)
        if let Some(id) = self.focar.take() {
            ectx.memory_mut(|m| m.request_focus(id));
        }
        self.desenhar(ui, ctx, tabela, agora);
        self.desenhar_janelas(&ectx, ctx, tabela);
        self.iniciar_tarefas(&ectx, ctx, tabela);
        if self.selecao_mudou {
            self.selecao_mudou = false;
            tabela.selecao = self.marcadas.chaves();
        }
        self.piscadas.retain(|p| agora - p.desde < interface::pecas::campos::DURACAO_SALVO);
    }

    // ------------------------------------------------------------------ rota

    /// A rota mudou por fora (a casca trocou o estado da aba sem refazê-la): vale a nova.
    fn sincronizar_rota(&mut self, tabela: &TabelaAberta) {
        let Some(d) = tabela.rota.dados() else { return };
        if self.consulta_escrita.as_ref() == Some(d) {
            return;
        }
        let mut nova = AbaDados::nova(tabela);
        self.consulta = std::mem::take(&mut nova.consulta);
        self.consulta_escrita = nova.consulta_escrita.take();
        self.busca = std::mem::take(&mut nova.busca);
        self.filtros = std::mem::take(&mut nova.filtros);
        self.proximo_filtro = nova.proximo_filtro;
        self.condicao = None;
        self.opcoes_por_pagina = std::mem::take(&mut nova.opcoes_por_pagina);
        self.marcadas.limpar();
        self.selecao_mudou = true;
        self.carregar();
    }

    /// Grava o estado da aba no endereço, sem entrada no histórico: trocar de página, de ordem ou de busca não cria entrada no histórico (Alt+← sai
    /// da tabela, em vez de percorrer cada página por que a pessoa passou). Por isso `Acao::Substituir`.
    fn gravar_rota(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let rota = tabela.rota.com_dados(self.consulta.clone());
        self.consulta_escrita = Some(self.consulta.clone());
        if rota != tabela.rota {
            tabela.rota = rota.clone();
            ctx.emitir(Acao::Substituir(rota));
        }
    }

    // ------------------------------------------------------------------ leitura

    /// Pede a página da consulta atual (sai depois das gravações da fila).
    fn carregar(&mut self) {
        self.pedido += 1;
        self.edicao = None;
        self.carga_pendente = true;
        self.carga = None;
    }

    /// O conjunto de linhas mudou (busca/filtros): primeira página e nada marcado (`recomecar`).
    fn recomecar(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.consulta.pagina = 1;
        if !self.marcadas.is_empty() {
            self.marcadas.limpar();
            self.selecao_mudou = true;
        }
        self.gravar_rota(ctx, tabela);
        self.carregar();
    }

    fn iniciar_tarefas(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        // uma célula que gravou ao perder o foco chega antes da releitura
        if self.carga_pendente && self.gravacoes.is_empty() {
            self.carga_pendente = false;
            self.iniciar_carga(ectx, ctx, tabela.info());
        }
    }

    fn iniciar_carga(&mut self, ectx: &egui::Context, ctx: &mut Contexto, info: &TabelaInfo) {
        let busca = self.consulta.busca.clone();
        let filtros = self.consulta.filtros.clone();
        let chave = chave_filtro(&busca, &filtros);
        let adiar = (!busca.is_empty() || !filtros.is_empty()) && tabela_grande(info);
        let conhecido = if adiar { self.contagens.get(&chave).copied() } else { None };
        let pedido_pagina = PedidoPagina {
            pagina: self.consulta.pagina,
            por_pagina: self.consulta.por_pagina,
            ordem: self.consulta.ordem.clone(),
            direcao: if self.consulta.ordem.is_some() { self.consulta.direcao } else { Direcao::Asc },
            busca: (!busca.is_empty()).then(|| busca.clone()),
            filtros: self.consulta.filtros_banco(),
            contar: !adiar,
            leitura: Some(format!("grade:{}", self.nome)),
        };
        let promessa = self.ler_pagina(ectx, ctx, pedido_pagina.clone());
        // uma contagem de outra busca ainda rodando prende o banco: pedir já a desta a abandona (mesmo grupo)
        if self.contando.as_ref().is_some_and(|c| c.chave != chave) && conhecido.is_none() {
            self.contar_depois(ectx, ctx, chave.clone(), busca.clone(), filtros.clone());
        }
        self.carga =
            Some(Carga { pedido: self.pedido, chave, busca, filtros, adiar, conhecido, pedido_pagina, promessa });
    }

    fn ler_pagina(&self, ectx: &egui::Context, ctx: &Contexto, pedido: PedidoPagina) -> Promessa<Pagina> {
        let nome = self.nome.clone();
        let gancho = self.ganchos.pagina.clone();
        let abandonada = self.ganchos.abandonada.clone();
        ctx.tarefa(ectx, move |s, id| {
            let r = s.linhas(id, &nome, &pedido);
            if let (Err(e), Some(g)) = (&r, &abandonada)
                && e.tem_codigo(Codigo::Substituido)
            {
                g();
            }
            let mut p = r?;
            if let Some(g) = gancho {
                g(&pedido, &mut p);
            }
            Ok(p)
        })
    }

    /// Pede a contagem do conjunto (tabela grande); ao chegar, completa rodapé, páginas e hint.
    fn contar_depois(
        &mut self,
        ectx: &egui::Context,
        ctx: &Contexto,
        chave: String,
        busca: String,
        filtros: Vec<FiltroDigitado>,
    ) {
        if self.contando.as_ref().is_some_and(|c| c.chave == chave) {
            return;
        }
        if let Some(antiga) = self.contando.take() {
            // terminada, a contagem antiga fica guardada (a pessoa pode voltar a ela)
            self.contagens_soltas.push((antiga.chave, antiga.promessa));
        }
        let promessa = self.pedir_contagem(ectx, ctx, &busca, &filtros, true);
        self.contando = Some(Contando { chave, busca, filtros, promessa });
    }

    fn pedir_contagem(
        &self,
        ectx: &egui::Context,
        ctx: &Contexto,
        busca: &str,
        filtros: &[FiltroDigitado],
        com_grupo: bool,
    ) -> Promessa<Contagem> {
        let nome = self.nome.clone();
        let busca = (!busca.is_empty()).then(|| busca.to_string());
        let filtros: Vec<banco::Filtro> = filtros.iter().map(FiltroDigitado::para_banco).collect();
        let leitura = com_grupo.then(|| format!("contagem:{}", self.nome));
        let gancho = self.ganchos.contagem.clone();
        let abandonada = self.ganchos.abandonada.clone();
        ctx.tarefa(ectx, move |s, id| {
            if let Some(g) = gancho {
                g(busca.as_deref(), leitura.as_deref());
            }
            let r = s.contagem(id, &nome, busca.as_deref(), &filtros, leitura.as_deref());
            if let (Err(e), Some(g)) = (&r, &abandonada)
                && e.tem_codigo(Codigo::Substituido)
            {
                g();
            }
            r
        })
    }

    fn receber(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta, agora: f64) {
        self.receber_pagina(ectx, ctx, tabela);
        self.receber_contagens(ectx, ctx, tabela);
        self.receber_gravacoes(ectx, ctx, agora);
        self.receber_abertura(ectx, ctx, tabela);
        self.receber_janela(ectx, ctx, tabela, agora);
        self.receber_exclusao(ectx, ctx, tabela);
        self.receber_desfazer(ectx, ctx, agora);
        self.receber_lote(ectx, ctx, agora);
    }

    fn receber_pagina(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let Some(r) = self.carga.as_mut().and_then(|c| c.promessa.tomar()) else { return };
        let Some(c) = self.carga.take() else { return };
        if c.pedido != self.pedido {
            return;
        }
        match r {
            Err(e) if e.tem_codigo(Codigo::Substituido) => {
                // abandonada por outra mais nova do mesmo grupo; se ainda é a que vale, refaz sem o grupo
                self.substituidas += 1;
                let mut pedido_pagina = c.pedido_pagina.clone();
                pedido_pagina.leitura = None;
                let promessa = self.ler_pagina(ectx, ctx, pedido_pagina.clone());
                self.carga = Some(Carga { pedido_pagina, promessa, ..c });
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    self.dados = None;
                    self.erro = Some(e.mensagem);
                }
            }
            Ok(p) => self.pagina_chegou(ectx, ctx, tabela, c, p),
        }
    }

    fn pagina_chegou(
        &mut self,
        ectx: &egui::Context,
        ctx: &mut Contexto,
        tabela: &mut TabelaAberta,
        c: Carga,
        p: Pagina,
    ) {
        self.geracao += 1;
        let mut d = Dados::nova(self.geracao, p, c.chave.clone(), tabela.info());
        if d.total.is_none() {
            let n = d.n_linhas() as u64;
            if !d.pagina.tem_mais && (n > 0 || d.pagina.pagina == 1) {
                // a última página já diz quantas linhas há: nada a contar
                let total = d.pagina.pagina.saturating_sub(1) * d.pagina.por_pagina + n;
                d.completar_total(total);
                self.contagens.insert(c.chave.clone(), total);
            } else if let Some(k) = c.conhecido {
                d.completar_total(k);
            } else {
                // também numa página além da última: a contagem diz qual é a última, e a tela vai para ela
                self.contar_depois(ectx, ctx, c.chave.clone(), c.busca.clone(), c.filtros.clone());
            }
        } else if c.adiar
            && let Some(t) = d.total
        {
            self.contagens.insert(c.chave.clone(), t);
        }
        // página além da última (linhas excluídas por fora, endereço antigo): vai para a última que existe
        if let (Some(t), Some(pg)) = (d.total, d.paginas)
            && t > 0
            && d.pagina.pagina > pg
        {
            self.consulta.pagina = pg;
            self.gravar_rota(ctx, tabela);
            self.carregar();
            return;
        }
        self.erro = None;
        if let Some(t) = d.total {
            tabela.definir_contagem_filtrada(&c.busca, &c.filtros, t);
        }
        let primeira = usize::from(self.editavel) + usize::from(coluna_que_abre(tabela.info()).is_none());
        let n_dados = d.colunas.len();
        if self.grade.as_ref().is_none_or(|g| g.nucleo.n_colunas() != primeira + n_dados) {
            let tem_abrir = coluna_que_abre(tabela.info()).is_none();
            self.grade = Some(GradeDados::nova(&self.nome, self.editavel, tem_abrir, n_dados));
        }
        let pendente: Option<FocoPendente> = ectx.data_mut(|m| m.remove_temp(id_foco_pendente(&self.nome)));
        let n = d.n_linhas();
        if let Some(g) = self.grade.as_mut() {
            g.nova_pagina(n);
            if n > 0 {
                let primeira = g.primeira();
                let (mut l, mut col) = g.nucleo.selecao().unwrap_or((0, primeira));
                let mut focar = false;
                if let Some(fp) = pendente {
                    // a tela foi refeita depois de incluir ou excluir: a linha nova, se estiver aqui, ou a mesma célula
                    focar = true;
                    if let Some((al, ac)) = fp.ativa {
                        (l, col) = (al, ac);
                    }
                    if let Some(achada) = fp.chave.as_deref().and_then(|k| d.linha_da_chave(k)) {
                        l = achada;
                    }
                }
                let ultima_col = g.nucleo.n_colunas().saturating_sub(1);
                g.nucleo.selecionar(Some((l.min(n - 1), col.clamp(primeira, ultima_col.max(primeira)))));
                if focar {
                    let id = g.nucleo.id_corpo();
                    ectx.memory_mut(|m| m.request_focus(id));
                }
            }
        }
        self.dados = Some(d);
        ectx.request_repaint();
    }

    fn receber_contagens(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        // as contagens de outras buscas: terminadas, ficam guardadas
        let mut i = 0;
        while let Some((_, promessa)) = self.contagens_soltas.get_mut(i) {
            match promessa.tomar() {
                Some(r) => {
                    let (chave, _) = self.contagens_soltas.swap_remove(i);
                    if let Ok(c) = r {
                        self.contagens.insert(chave, c.linhas);
                    }
                }
                None => i += 1,
            }
        }
        let Some(r) = self.contando.as_mut().and_then(|c| c.promessa.tomar()) else { return };
        let Some(registro) = self.contando.take() else { return };
        match r {
            Ok(c) => {
                self.contagens.insert(registro.chave.clone(), c.linhas);
                let Some(d) = self.dados.as_mut().filter(|d| d.chave_filtro == registro.chave) else { return };
                d.tempo_contagem = Some(c.tempo_ms);
                if d.total.is_none() {
                    d.completar_total(c.linhas);
                    tabela.definir_contagem_filtrada(&registro.busca, &registro.filtros, c.linhas);
                    if let Some(pg) = d.paginas
                        && c.linhas > 0
                        && d.pagina.pagina > pg
                    {
                        self.ir_para(ctx, tabela, pg);
                    }
                }
            }
            Err(e) if e.tem_codigo(Codigo::Substituido) => {
                // ainda é a que vale: refaz sem o grupo, e aí nada a abandona
                self.substituidas += 1;
                let promessa = self.pedir_contagem(ectx, ctx, &registro.busca, &registro.filtros, false);
                self.contando = Some(Contando { promessa, ..registro });
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Ignorar {
                    return;
                }
                if let Some(d) = self.dados.as_mut().filter(|d| d.chave_filtro == registro.chave && d.total.is_none()) {
                    d.contagem_falhou = Some(e.mensagem);
                }
            }
        }
    }

    // ------------------------------------------------------------------ busca, filtros, página, ordem

    fn temporizadores(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta, agora: f64) {
        if let Some(mudou) = self.busca_mudou {
            let falta = ESPERA_DIGITACAO - (agora - mudou);
            if falta > 0.0 {
                ectx.request_repaint_after(std::time::Duration::from_secs_f64(falta));
            }
        }
        if self.busca_mudou.is_some_and(|t| agora - t >= ESPERA_DIGITACAO) {
            self.aplicar_busca(ctx, tabela);
        }
    }

    fn aplicar_busca(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.busca_mudou = None;
        if self.busca == self.consulta.busca {
            return;
        }
        self.consulta.busca = self.busca.clone();
        self.recomecar(ctx, tabela);
    }

    fn aplicar_filtros(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let completos = filtros_completos(&self.filtros);
        if completos == self.consulta.filtros {
            return;
        }
        self.consulta.filtros = completos;
        self.recomecar(ctx, tabela);
    }

    // ------------------------------------------------------------------ condição de filtro (o popover)

    /// Abre o popover numa condição nova (`alvo` None) ou na de uma etiqueta, preso embaixo de `ancora`.
    fn abrir_condicao(&mut self, alvo: Option<u64>, ancora: Rect) {
        let existente = alvo.and_then(|id| self.filtros.iter().find(|f| f.id == id));
        let (coluna, op, valor) = match existente {
            Some(f) => (self.nomes_visiveis.iter().position(|n| *n == f.coluna).unwrap_or(0), f.op, f.valor.clone()),
            None => (0, Operador::Contem, String::new()),
        };
        self.condicao = Some(EdicaoCondicao { alvo, coluna, op, valor, ancora, focar: !op.sem_valor() });
    }

    /// Aplica a condição do popover: troca a da etiqueta ou entra como nova, e a página relê.
    fn aplicar_condicao(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let Some(c) = self.condicao.take() else { return };
        let Some(coluna) = self.nomes_visiveis.get(c.coluna).cloned() else { return };
        let valor = if c.op.sem_valor() { String::new() } else { c.valor };
        match c.alvo.and_then(|id| self.filtros.iter_mut().find(|f| f.id == id)) {
            Some(f) => {
                f.coluna = coluna;
                f.op = c.op;
                f.valor = valor;
            }
            None => {
                self.proximo_filtro += 1;
                self.filtros.push(LinhaFiltro { id: self.proximo_filtro, coluna, op: c.op, valor });
            }
        }
        self.aplicar_filtros(ctx, tabela);
    }

    /// O popover da condição (`interface::pecas::filtros::popover_condicao`): Coluna, Operador (os 14 de
    /// `OPERADORES`), Valor (fora nos operadores sem valor), Cancelar e Aplicar.
    fn desenhar_condicao(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let Some(c) = self.condicao.as_mut() else { return };
        let colunas: Vec<(usize, &str)> = self.nomes_visiveis.iter().map(String::as_str).enumerate().collect();
        let id = ("tela_dados::condicao", self.nome.as_str());
        match popover_condicao(ectx, id, c, &colunas, &OPERADORES, |o| o.sem_valor()) {
            Some(FimCondicao::Aplicar) => self.aplicar_condicao(ctx, tabela),
            Some(FimCondicao::Desistir) => self.condicao = None,
            None => {}
        }
    }

    fn ir_para(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta, pagina: u64) {
        self.consulta.pagina = pagina.max(1);
        self.gravar_rota(ctx, tabela);
        self.carregar();
    }

    fn ordenar(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta, coluna: &str) {
        if self.consulta.ordem.as_deref() != Some(coluna) {
            self.consulta.ordem = Some(coluna.to_string());
            self.consulta.direcao = Direcao::Asc;
        } else if self.consulta.direcao == Direcao::Asc {
            self.consulta.direcao = Direcao::Desc;
        } else {
            self.consulta.ordem = None;
            self.consulta.direcao = Direcao::Asc;
        }
        self.consulta.pagina = 1;
        self.gravar_rota(ctx, tabela);
        self.carregar();
    }

    fn definir_por_pagina(&mut self, ctx: &mut Contexto, tabela: &mut TabelaAberta, por_pagina: u64) {
        self.consulta.por_pagina = por_pagina;
        self.consulta.pagina = 1;
        self.opcoes_por_pagina = opcoes_por_pagina(por_pagina);
        self.gravar_rota(ctx, tabela);
        self.carregar();
    }

    // ------------------------------------------------------------------ textos

    /// A contagem da faixa: as linhas da busca e dos filtros de quantas a tabela tem ("41 de 60 linhas"), ou só as
    /// linhas sem busca nem filtro ("60 linhas").
    fn contagem_faixa(&self, info: &TabelaInfo) -> ContagemFiltros {
        if self.erro.is_some() {
            return ContagemFiltros::Nada;
        }
        let Some(d) = &self.dados else { return ContagemFiltros::Nada };
        match (d.total, &d.contagem_falhou) {
            (Some(t), _) => {
                let filtrando = !self.consulta.busca.is_empty() || !self.consulta.filtros.is_empty();
                ContagemFiltros::Texto(texto_contagem_faixa(t, info.linhas, filtrando))
            }
            (None, Some(motivo)) => ContagemFiltros::NaoContadas(motivo.clone()),
            (None, None) => ContagemFiltros::Contando,
        }
    }

    fn rodape(&self, info: &TabelaInfo, somente_leitura: bool) -> Option<Rodape> {
        let d = self.dados.as_ref().filter(|_| self.erro.is_none())?;
        let p = &d.pagina;
        let (inicio, fim) = intervalo(p.pagina, p.por_pagina, d.n_linhas(), d.total);
        let (contagem, sem_total) = match d.total {
            None => (texto_contagem_sem_total(inicio, fim), Some(d.contagem_falhou.clone())),
            Some(t) => (texto_contagem_linhas(inicio, fim, t), None),
        };
        Some(Rodape {
            contagem,
            sem_total,
            pagina: texto_pagina(p.pagina, d.paginas),
            anterior: p.pagina > 1,
            // sem o total ainda (tabela grande), o "›" segue o tem_mais da página
            proxima: match d.paginas {
                None => p.tem_mais,
                Some(n) => p.pagina < n,
            },
            sugestao: self.sugestao(d, info, somente_leitura),
        })
    }

    /// A dica de página lenta: ordenar por uma coluna sem índice, ou buscar em todas as colunas.
    fn sugestao(&self, d: &Dados, info: &TabelaInfo, somente_leitura: bool) -> Option<Sugestao> {
        let col = d.pagina.ordem.as_ref().and_then(|o| info.colunas.iter().find(|c| &c.nome == o));
        let pode_criar = !self.e_visao && info.esquema_editavel && !somente_leitura;
        if let Some(col) = col
            && !col.rowid_alias
            && d.pagina.tempo_ms > ORDENAR_LENTO_MS
            && pode_criar
            && !tem_indice_por(info, &col.nome)
        {
            return Some(Sugestao::Indice {
                coluna: col.nome.clone(),
                texto: format!(
                    "Ordenar por {} levou {}. Um índice nessa coluna deixa a ordenação imediata.",
                    col.nome,
                    formatar_segundos(d.pagina.tempo_ms)
                ),
            });
        }
        let lenta = d.pagina.tempo_ms.max(d.tempo_contagem.unwrap_or(0.0));
        (!self.consulta.busca.is_empty() && self.consulta.filtros.is_empty() && lenta > BUSCA_LENTA_MS)
            .then_some(Sugestao::Busca)
    }

    fn texto_vazio(&self) -> &'static str {
        if !self.consulta.busca.is_empty() || !self.consulta.filtros.is_empty() {
            VAZIO_FILTRANDO
        } else if self.e_visao {
            VAZIO_VISAO
        } else {
            VAZIO_TABELA
        }
    }

    /// O botão do vazio: "Limpar filtros" com condição; "Nova linha" na tabela que grava, sem busca nem filtro.
    fn acao_do_vazio(&self) -> Option<AcaoVazio> {
        if !self.consulta.filtros.is_empty() {
            Some(AcaoVazio::LimparFiltros)
        } else if self.editavel && self.consulta.busca.is_empty() {
            Some(AcaoVazio::NovaLinha)
        } else {
            None
        }
    }

    // ------------------------------------------------------------------ desenho

    /// O conteúdo do documento (`Rolagem::Propria`): a grade na altura que sobra (ou o aviso, o "Lendo as linhas…",
    /// o vazio) e, com a página lenta, a linha da dica embaixo dela.
    fn desenhar(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta, agora: f64) {
        let somente_leitura = ctx.estado.somente_leitura();
        let mut acoes = AcoesQuadro::default();
        let sugestao = self.rodape(tabela.info(), somente_leitura).and_then(|r| r.sugestao);
        let mut area = ui.available_rect_before_wrap();
        // desenhada numa página de rolagem (o quadro em que a tela acabou de ser lida), a altura que sobra não tem fim:
        // sem ela, a grade fica com o mínimo em vez de levar o cursor do pai ao infinito
        if !area.max.y.is_finite() {
            area.max.y = area.min.y + ALTURA_GRADE_MIN;
        }
        let altura_dica = if sugestao.is_some() { ALTURA_DICA_LENTA } else { 0.0 };
        let rect_grade =
            Rect::from_min_max(area.min, pos2(area.right(), (area.bottom() - altura_dica).max(area.top())));
        let mut ui_grade = ui.new_child(
            UiBuilder::new().id_salt("tela_dados::conteudo").max_rect(rect_grade).layout(Layout::top_down(Align::Min)),
        );
        ui_grade.set_clip_rect(rect_grade.intersect(ui.clip_rect()));
        ui_grade.spacing_mut().item_spacing.y = 0.0;
        let impedimento = if somente_leitura {
            Some(MOTIVO_SO_LEITURA)
        } else if self.ocupada_para_escrever() {
            Some(MOTIVO_OCUPADO)
        } else {
            None
        };
        self.desenhar_grade_dentro(&mut ui_grade, agora, tabela.info(), impedimento, &mut acoes);
        if let Some(s) = &sugestao {
            let rect = Rect::from_min_max(pos2(area.left(), rect_grade.bottom()), area.max);
            linha_da_dica_lenta(ui, rect, s, &mut acoes);
        }
        ui.allocate_rect(area, Sense::hover());
        self.aplicar_acoes(ui, ctx, tabela, acoes);
    }

    /// A faixa de contexto: "Filtros", uma etiqueta por condição aplicada (o clique abre a condição no popover, o ×
    /// a tira), "+ Condição" e, à direita, a contagem e "Limpar filtros" (só com condição).
    fn desenhar_faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let etiquetas: Vec<EtiquetaFiltro<'_>> = self
            .filtros
            .iter()
            .map(|f| EtiquetaFiltro {
                id: f.id,
                coluna: &f.coluna,
                operador: rotulo_operador(f.op),
                valor: (!f.op.sem_valor()).then_some(f.valor.as_str()),
            })
            .collect();
        let contagem = self.contagem_faixa(tabela.info());
        let r = desenhar_filtros(ui, &etiquetas, &contagem);
        self.rect_condicao = Some(r.nova_condicao);
        let (abrir, tirar, limpar) = (r.abrir, r.tirar, r.limpar);
        if let Some(id) = tirar {
            self.filtros.retain(|f| f.id != id);
            if self.condicao.as_ref().is_some_and(|c| c.alvo == Some(id)) {
                self.condicao = None;
            }
            self.aplicar_filtros(ctx, tabela);
        }
        if limpar {
            self.filtros.clear();
            self.condicao = None;
            self.aplicar_filtros(ctx, tabela);
        }
        if let Some((alvo, ancora)) = abrir {
            self.abrir_condicao(alvo, ancora);
        }
        let ectx = ui.ctx().clone();
        self.desenhar_condicao(&ectx, ctx, tabela);
    }
}

/// A linha de informação da página lenta, acima do rodapé (no fundo do documento, com o fio `--b` em cima,
/// `padding:0 14px`, gap 8): o ícone Informação de 16, a frase em `--t2` e, na ordenação sem índice, "Criar
/// índice".
fn linha_da_dica_lenta(ui: &mut Ui, rect: Rect, s: &Sugestao, acoes: &mut AcoesQuadro) {
    /// O `padding:0 14px` (8 do lado do botão, como o rodapé), o fio de 1px em cima, o gap 8 e o ícone de 16.
    const RECUO_ESQUERDA: f32 = 14.0;
    const RECUO_DIREITA: f32 = 8.0;
    const FIO: f32 = 1.0;
    const ESPACO: f32 = 8.0;
    const LADO_ICONE: f32 = 16.0;
    let p = paleta(ui);
    ui.painter().hline(rect.x_range(), rect.top() + FIO / 2.0, egui::Stroke::new(FIO, p.estudio.b));
    let dentro = Rect::from_min_max(
        pos2(rect.left() + RECUO_ESQUERDA, rect.top() + FIO),
        pos2(rect.right() - RECUO_DIREITA, rect.bottom()),
    );
    let mut linha_ui = ui.new_child(
        UiBuilder::new().id_salt("tela_dados::sugestao").max_rect(dentro).layout(Layout::left_to_right(Align::Center)),
    );
    linha_ui.set_clip_rect(dentro.intersect(ui.clip_rect()));
    linha_ui.spacing_mut().item_spacing.x = ESPACO;
    let (r_icone, _) = linha_ui.allocate_exact_size(vec2(LADO_ICONE, LADO_ICONE), Sense::hover());
    icones::pintar(linha_ui.painter(), r_icone, Icone::Estado(EstadoIcone::Informacao), egui::Color32::WHITE);
    match s {
        Sugestao::Indice { coluna, texto } => {
            texto_simples(&mut linha_ui, texto, 12.0, p.estudio.t2);
            if linha_ui.add(Botao::new("Criar índice").pequeno()).clicked() {
                acoes.criar_indice = Some(coluna.clone());
            }
        }
        Sugestao::Busca => {
            texto_simples(&mut linha_ui, DICA_BUSCA_LENTA, 12.0, p.estudio.t2);
        }
    }
}

impl AbaDados {
    fn desenhar_grade_dentro(
        &mut self,
        ui: &mut Ui,
        agora: f64,
        info: &TabelaInfo,
        impedimento: Option<&'static str>,
        acoes: &mut AcoesQuadro,
    ) {
        if let Some(erro) = &self.erro {
            let erro = erro.clone();
            linha_de_aviso(ui, |ui| {
                aviso_texto(ui, &erro, true);
            });
            return;
        }
        let Some(dados) = &self.dados else {
            linha_de_aviso(ui, |ui| {
                carregando(ui, LENDO);
            });
            return;
        };
        let sem_linhas = dados.n_linhas() == 0;
        let (texto_vazio, acao_vazio) = (self.texto_vazio(), self.acao_do_vazio());
        let Some(grade) = self.grade.as_mut() else {
            vazio(ui, texto_vazio, false);
            return;
        };
        let p = paleta(ui);
        let cabecalho_abrir = match &info.chave {
            Some(TipoChave::Rowid { alias }) => Some(alias.as_str()),
            _ => None,
        };
        grade.ajustar_larguras(ui, p, dados, cabecalho_abrir, dados.pagina.ordem.as_deref());
        // a grade ocupa a altura que sobra no documento (e rola por dentro)
        let altura_max = ui.available_height().max(ALTURA_GRADE_MIN);
        self.larguras_quadro.clear();
        self.larguras_quadro.extend_from_slice(grade.nucleo.larguras());
        let n_com_chave = dados.chaves.iter().flatten().count();
        let n_marcadas_aqui = dados.chaves.iter().flatten().filter(|k| self.marcadas.contem(k)).count();
        let ocupada = self.carga.is_some() || self.carga_pendente;
        let mut pintor = Pintor {
            p,
            dados,
            primeira: grade.primeira(),
            tem_marca: grade.tem_marca,
            tem_abrir: grade.tem_abrir,
            editavel: self.editavel,
            marcadas: &self.marcadas,
            edicao: self.edicao.as_mut(),
            pendentes: &self.pendentes,
            piscadas: &self.piscadas,
            ordem: self.consulta.ordem.as_deref().map(|o| (o, self.consulta.direcao)),
            ocupada,
            titulo_abre: if self.editavel { "Ver e alterar a linha inteira" } else { "Ver a linha inteira" },
            cabecalho_abrir,
            todas_marcadas: n_com_chave > 0 && n_marcadas_aqui == n_com_chave,
            algumas_marcadas: n_marcadas_aqui > 0,
            ponta_ida: grade.nucleo.na_ponta(false),
            ponta_volta: grade.nucleo.na_ponta(true),
            agora,
            larguras: &self.larguras_quadro,
            altura_cabecalho: grade.nucleo.altura_do_cabecalho(),
            destacada: self.destacada.as_deref(),
            ordenar: None,
            abrir: None,
            marcar: None,
            marcar_pagina: None,
            fim_edicao: None,
            edicao_desenhada: false,
            origem: None,
            visivel: None,
        };
        let topo = ui.cursor().top();
        let r = grade.nucleo.mostrar(ui, altura_max, &mut pintor);
        // sem linhas, o cabeçalho continua à vista e o vazio vai embaixo dele, com a ação (U5 da revisão de UI de
        // 22/10/2026)
        if sem_linhas {
            let largura = ui.max_rect().x_range();
            let embaixo = topo + grade.nucleo.altura_do_cabecalho();
            let rect = Rect::from_x_y_ranges(largura, embaixo..=(topo + altura_max).min(ui.max_rect().bottom()));
            let botao = acao_vazio.map(|a| match a {
                AcaoVazio::NovaLinha => {
                    Botao::primario(VAZIO_NOVA_LINHA).com_icone(Icone::Novo).habilitado(impedimento.is_none())
                }
                AcaoVazio::LimparFiltros => Botao::new(VAZIO_LIMPAR_FILTROS),
            });
            if vazio_da_grade(ui, rect, texto_vazio, None, botao) {
                acoes.vazio = acao_vazio;
            }
        }
        grade.origem = pintor.origem;
        grade.visivel = pintor.visivel;
        acoes.ordenar = pintor.ordenar.and_then(|c| dados.colunas.get(c)).map(|c| c.nome.clone());
        acoes.abrir = pintor.abrir;
        acoes.marcar = pintor.marcar;
        acoes.marcar_pagina = pintor.marcar_pagina;
        acoes.fim_edicao = pintor.fim_edicao;
        let edicao_desenhada = pintor.edicao_desenhada;
        // a célula em edição saiu da vista (rolagem) depois de o campo aparecer com o foco: grava, como se ele
        // perdesse o foco. Antes disso não: a edição que acabou de começar numa célula fora da vista espera a
        // grade rolar até ela
        if acoes.fim_edicao.is_none() && !edicao_desenhada && self.edicao.as_ref().is_some_and(|e| e.focada) {
            acoes.fim_edicao = Some(FimEdicao::Salvar { foco: false });
        }
        let primeira = grade.primeira();
        acoes.editar = r.editar.and_then(|(l, c)| c.checked_sub(primeira).map(|c| (l, c)));
        if self.editavel
            && let (Some(t), Some((l, c))) = (r.texto_digitado, grade.nucleo.selecao())
            && let Some(c) = c.checked_sub(primeira)
        {
            acoes.digitado = Some((l, c, t));
        }
        // o Delete exclui as marcadas (com a pergunta); sem marcadas, é o Limpar do intervalo
        acoes.excluir = r.excluir && !self.marcadas.is_empty();
        let limpar_pela_tecla = r.excluir && self.marcadas.is_empty();
        acoes.espaco = r.espaco;
        // o Ctrl+C e o Ctrl+Shift+C: a célula ativa, ou o intervalo como o Excel copia; a célula gravando agora vai
        // com o texto digitado
        let fonte = FonteCopia { dados, pendentes: &self.pendentes, primeira, tabela: &self.nome };
        if let Some(iv) = grade.nucleo.intervalo() {
            if r.copiar {
                fonte.copiar(ui.ctx(), AcaoCelula::Copiar, iv);
            } else if r.copiar_com_cabecalho {
                fonte.copiar(ui.ctx(), AcaoCelula::CopiarComCabecalho, iv);
            }
        }
        // o menu da célula (o clique direito, o Shift+F10): chamado todo quadro, para abrir e fechar
        let ativa = grade.nucleo.selecao().and_then(|(l, c)| Some((l, c.checked_sub(primeira)?)));
        let celula_ativa = ativa.and_then(|(l, c)| dados.celula(l, c));
        // num banco só leitura a tabela não grava, mas os itens de escrita aparecem desligados, com o porquê
        let gravaria = self.editavel
            || (impedimento == Some(MOTIVO_SO_LEITURA)
                && !self.e_visao
                && info.tipo == TipoObjeto::Table
                && info.chave.is_some());
        let escrita = |iv: Intervalo| match impedimento {
            _ if !gravaria => Escrita::Nao,
            Some(motivo) => Escrita::AgoraNao(motivo),
            None => Escrita::Sim(EscritaDoIntervalo::de(iv, primeira, dados, info)),
        };
        let ofertas = grade
            .nucleo
            .intervalo()
            .map(|iv| menu::ofertas(iv, celula_ativa, &escrita(iv), grade.tem_marca))
            .unwrap_or_default();
        let escolha = menu_da_celula(ui, &mut grade.nucleo, &ofertas);
        // o Ctrl+V com o foco na grade, e o Colar do menu (o texto chega no Paste do quadro seguinte)
        let colado = texto_colado(ui.ctx(), &grade.nucleo).filter(|_| gravaria && self.edicao.is_none());
        let pedido = match (colado, limpar_pela_tecla && gravaria) {
            (Some(texto), _) => Some((TipoLote::Colar, Some(texto))),
            (None, true) => Some((TipoLote::Limpar, None)),
            _ => None,
        };
        let pedido = pedido.or(match escolha {
            Some(AcaoCelula::Limpar) => Some((TipoLote::Limpar, None)),
            Some(AcaoCelula::DefinirNulo) => Some((TipoLote::Nulo, None)),
            _ => None,
        });
        if let (Some((tipo, texto)), Some(iv)) = (pedido, grade.nucleo.intervalo()) {
            let acao = match tipo {
                TipoLote::Colar => AcaoCelula::Colar,
                TipoLote::Limpar => AcaoCelula::Limpar,
                TipoLote::Nulo => AcaoCelula::DefinirNulo,
            };
            acoes.lote = match ofertas.get(acao) {
                OfertaCelula::Ligada => {
                    let colunas = primeira..primeira + dados.colunas.len();
                    let dado = |(l, c, t): (usize, usize, Option<String>)| Some((l, c.checked_sub(primeira)?, t));
                    let (alvos, de_fora): (Vec<_>, u64) = match texto {
                        Some(texto) => {
                            let cola = colagem(&texto, iv, dados.n_linhas(), colunas);
                            let alvos = cola.celulas.into_iter().filter_map(|(l, c, t)| dado((l, c, Some(t))));
                            (alvos.collect(), cola.de_fora)
                        }
                        None => {
                            let alvos = iv.linhas().flat_map(|l| iv.colunas().map(move |c| (l, c, None)));
                            (alvos.filter_map(dado).collect(), 0)
                        }
                    };
                    Some(montar_lote(tipo, alvos, de_fora, dados, info))
                }
                OfertaCelula::Desligada(motivo) => Some(Err(motivo.to_string())),
                OfertaCelula::Oculta => None,
            };
        }
        if let (Some(acao), Some(iv)) = (escolha, grade.nucleo.intervalo()) {
            match acao {
                AcaoCelula::Colar => ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste),
                AcaoCelula::ExcluirLinhas => {
                    acoes.excluir_linhas = Some(iv.linhas().filter_map(|l| dados.chave(l).cloned()).collect());
                }
                AcaoCelula::Copiar | AcaoCelula::CopiarComCabecalho | AcaoCelula::CopiarComo(_) => {
                    fonte.copiar(ui.ctx(), acao, iv);
                }
                AcaoCelula::FiltrarPorValor | AcaoCelula::ExcluirValorDoFiltro => {
                    let excluir = acao == AcaoCelula::ExcluirValorDoFiltro;
                    acoes.filtrar = ativa.and_then(|(l, c)| {
                        let (op, valor) = menu::condicao_do_valor(dados.celula(l, c)?, excluir)?;
                        Some((dados.colunas.get(c)?.nome.clone(), op, valor))
                    });
                }
                AcaoCelula::MarcarLinhas => acoes.marcar_linhas = Some(iv.linha0..iv.linha1 + 1),
                AcaoCelula::AbrirFicha => acoes.abrir = ativa.map(|(l, _)| l).or(acoes.abrir),
                _ => {}
            }
        }
    }

    /// As contas do rodapé da seleção (A.3), com mais de uma célula selecionada; refeitas só quando o intervalo, a
    /// página ou as gravações a caminho mudam.
    fn resumo_da_selecao(&mut self) -> Option<ResumoSelecao> {
        let (Some(d), Some(g)) = (&self.dados, &self.grade) else { return None };
        let iv = g.nucleo.intervalo().filter(Intervalo::varias)?;
        let chave = (iv, d.geracao, self.pendentes.len());
        if let Some((k, r)) = &self.resumo
            && *k == chave
        {
            return Some(*r);
        }
        let fonte = FonteCopia { dados: d, pendentes: &self.pendentes, primeira: g.primeira(), tabela: &self.nome };
        let r = fonte.resumo(iv);
        self.resumo = Some((chave, r));
        Some(r)
    }

    /// O rodapé do documento (34px): "Linhas 1–41 de 41" e " · 2 marcadas" em `--t3`; à direita, o "100 por página"
    /// de 26px, ‹ "Página 1 de 1" › (os `.ib.pq` com as silhuetas Voltar e Avançar). Sem página, fica vazio.
    fn desenhar_rodape(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let p = paleta(ui);
        let Some(r) = self.rodape(tabela.info(), ctx.estado.somente_leitura()) else { return };
        texto_simples(ui, &r.contagem, 12.0, p.estudio.t2);
        if let Some(falhou) = &r.sem_total {
            ui.add_space(-8.0);
            match falhou {
                Some(motivo) => {
                    texto_simples(ui, "…", 12.0, p.estudio.t2).on_hover_text(motivo);
                }
                None => {
                    carregando(ui, CONTANDO);
                }
            }
        }
        let n_marcadas = self.marcadas.len();
        if n_marcadas > 0 {
            texto_simples(ui, &format!("· {}", plural(n_marcadas, "marcada", "marcadas")), 12.0, p.estudio.t3);
        }
        let resumo = self.resumo_da_selecao();
        let opcoes: Vec<(u64, &str)> = self.opcoes_por_pagina.iter().map(|(n, t)| (*n, t.as_str())).collect();
        let mut por_pagina = self.consulta.por_pagina;
        let (mut novo_tamanho, mut ir) = (None, None);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let proxima = Botao::icone(Icone::Avancar).pequeno().rotulo("Próxima página").dica("Próxima página");
            if ui.add(proxima.habilitado(r.proxima)).clicked() {
                ir = Some(self.consulta.pagina + 1);
            }
            texto_simples(ui, &r.pagina, 12.0, p.estudio.t2);
            let anterior = Botao::icone(Icone::Voltar).pequeno().rotulo("Página anterior").dica("Página anterior");
            if ui.add(anterior.habilitado(r.anterior)).clicked() {
                ir = Some(self.consulta.pagina.saturating_sub(1).max(1));
            }
            if Select::new(("tela_dados::por_pagina", &self.nome), &mut por_pagina, &opcoes)
                .largura_auto()
                .pequeno()
                .rotulo("Linhas por página")
                .mostrar(ui)
                .changed()
            {
                novo_tamanho = Some(por_pagina);
            }
            // com mais de uma célula selecionada, as contas da seleção antes das páginas (A.3)
            if let Some(resumo) = &resumo {
                ui.add_space(ESPACO_RESUMO);
                rodape_da_selecao(ui, resumo);
            }
        });
        if let Some(n) = novo_tamanho {
            self.definir_por_pagina(ctx, tabela, n);
        }
        if let Some(pg) = ir {
            self.ir_para(ctx, tabela, pg);
        }
    }

    // ------------------------------------------------------------------ o que a pessoa fez

    fn aplicar_acoes(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta, a: AcoesQuadro) {
        let ectx = ui.ctx().clone();
        // primeiro a edição que terminou (a gravação entra na fila antes de qualquer releitura)
        if let Some(fim) = a.fim_edicao {
            self.terminar_edicao(&ectx, ctx, fim);
        }
        if self.focar.is_some() {
            ectx.request_repaint();
        }
        if let Some(coluna) = a.ordenar {
            self.ordenar(ctx, tabela, &coluna);
        }
        match a.vazio {
            Some(AcaoVazio::NovaLinha) => self.abrir_nova(tabela.info(), Vec::new(), false),
            Some(AcaoVazio::LimparFiltros) => {
                self.filtros.clear();
                self.condicao = None;
                self.aplicar_filtros(ctx, tabela);
            }
            None => {}
        }
        if let Some(coluna) = a.criar_indice {
            ctx.emitir(Acao::AbrirNovoIndice { tabela: self.nome.clone(), colunas: vec![coluna] });
        }
        if let Some((l, sim)) = a.marcar {
            self.marcar(l, sim);
        }
        if let Some(linhas) = a.marcar_linhas {
            for l in linhas {
                self.marcar(l, true);
            }
        }
        if let Some((coluna, op, valor)) = a.filtrar {
            self.proximo_filtro += 1;
            self.filtros.push(LinhaFiltro { id: self.proximo_filtro, coluna, op, valor });
            self.aplicar_filtros(ctx, tabela);
        }
        if let Some(sim) = a.marcar_pagina {
            let n = self.dados.as_ref().map_or(0, Dados::n_linhas);
            for l in 0..n {
                self.marcar(l, sim);
            }
        }
        if let Some(l) = a.abrir {
            self.abrir_linha(&ectx, ctx, tabela.info(), l, None);
        }
        // a edição que começou numa célula fora da vista espera a grade rolar até ela, com a grade ainda focada: o
        // que a pessoa digita nesse meio-tempo vai para ela (lá o campo já teria o foco), e Enter/F2 de novo não a
        // recomeçam. Com o campo focado, as teclas deste quadro já foram para ele.
        let pendente = self.edicao.as_ref().map(|e| (e.linha, e.coluna, e.focada));
        if let Some((l, c)) = a.editar
            && !matches!(pendente, Some((pl, pc, false)) if (pl, pc) == (l, c))
        {
            self.editar(&ectx, ctx, tabela.info(), l, c, None);
        }
        if let Some((l, c, t)) = a.digitado {
            match self.edicao.as_mut() {
                Some(ed) if ed.focada => {}
                Some(ed) if (ed.linha, ed.coluna) == (l, c) => {
                    ed.texto.push_str(&t);
                    preparar_edicao(&ectx, ed, false);
                }
                _ => self.editar(&ectx, ctx, tabela.info(), l, c, Some(t)),
            }
        }
        if a.excluir && !self.marcadas.is_empty() {
            self.excluir_marcadas();
        }
        if let Some(chaves) = a.excluir_linhas {
            self.perguntar_excluir(chaves);
        }
        match a.lote {
            Some(Ok(lote)) => self.pedir_lote(&ectx, ctx, lote),
            Some(Err(motivo)) => toast_atencao(&ectx, motivo),
            None => {}
        }
        if a.espaco
            && self.editavel
            && let Some((l, _)) = self.grade.as_ref().and_then(|g| g.nucleo.selecao())
            && let Some(k) = self.dados.as_ref().and_then(|d| d.chaves.get(l).cloned().flatten())
        {
            let sim = !self.marcadas.contem(&k);
            self.marcar(l, sim);
        }
    }

    fn marcar(&mut self, l: usize, sim: bool) {
        let Some(d) = &self.dados else { return };
        let (Some(Some(texto)), Some(chave)) = (d.chaves.get(l), d.chave(l)) else { return };
        if self.marcadas.definir(texto, chave, sim) {
            self.selecao_mudou = true;
        }
    }

    // ------------------------------------------------------------------ edição no lugar

    /// Edita a célula no lugar; o que não cabe num campo de uma linha abre a janela (`editar`).
    fn editar(
        &mut self,
        ectx: &egui::Context,
        ctx: &Contexto,
        info: &TabelaInfo,
        l: usize,
        c: usize,
        inicial: Option<String>,
    ) {
        let Some(d) = &self.dados else { return };
        let (Some(col), Some(valor)) = (d.colunas.get(c), d.celula(l, c)) else { return };
        let sem_chave = d.chave(l).is_none();
        let nao_cabe = matches!(valor, Celula::Blob { .. } | Celula::TextoCortado { .. }) || tem_quebra(valor);
        if !self.editavel || sem_chave || col.abre || col.gerada || nao_cabe {
            if inicial.is_none() {
                let foco = Some(col.nome.clone());
                self.abrir_linha(ectx, ctx, info, l, foco);
            }
            return;
        }
        let selecionar_tudo = inicial.is_none();
        let ed = Edicao {
            geracao: d.geracao,
            linha: l,
            coluna: c,
            texto: inicial.unwrap_or_else(|| texto_edicao(valor)),
            original: valor.clone(),
            numerica: col.numerica,
            rotulo: format!("{} da linha {}", col.nome, crate::regras::rotulo_linha(d.chave(l))),
            focada: false,
            pedir_foco: true,
        };
        preparar_edicao(ectx, &ed, selecionar_tudo);
        self.edicao = Some(ed);
        // a célula fica ativa e a grade rola até ela (uma tecla digitada com a célula ativa fora da vista): o campo
        // só aparece, e só recebe o foco, quando ela está à vista
        if let Some(g) = self.grade.as_mut() {
            let primeira = g.primeira();
            g.nucleo.selecionar(Some((l, primeira + c)));
        }
        ectx.request_repaint();
    }

    /// A célula volta a ser a ativa e a grade recebe o foco. Só com linhas na página (a grade é desenhada neste
    /// quadro); mais um quadro em seguida, para o filtro das setas da grade valer já na próxima tecla.
    fn focar_grade(&mut self, ectx: &egui::Context, celula: Option<(usize, usize)>) {
        let tem_linhas = self.dados.as_ref().is_some_and(|d| d.n_linhas() > 0) && self.erro.is_none();
        if let Some(g) = self.grade.as_mut().filter(|_| tem_linhas) {
            if let Some((l, c)) = celula {
                let primeira = g.primeira();
                g.nucleo.selecionar(Some((l, primeira + c)));
            }
            let id = g.nucleo.id_corpo();
            ectx.memory_mut(|m| m.request_focus(id));
            ectx.request_repaint();
        }
    }

    fn terminar_edicao(&mut self, ectx: &egui::Context, ctx: &mut Contexto, fim: FimEdicao) {
        let Some(ed) = self.edicao.take() else { return };
        match fim {
            FimEdicao::Cancelar => self.focar_grade(ectx, Some((ed.linha, ed.coluna))),
            FimEdicao::Salvar { foco } => {
                self.salvar_edicao(ectx, ctx, ed.clone(), foco);
                if foco {
                    self.focar_grade(ectx, Some((ed.linha, ed.coluna)));
                }
            }
            FimEdicao::Tab { voltando } => {
                self.salvar_edicao(ectx, ctx, ed.clone(), false);
                self.focar_grade(ectx, Some((ed.linha, ed.coluna)));
                if let Some(g) = self.grade.as_mut() {
                    g.nucleo.mover(0, if voltando { -1 } else { 1 });
                }
            }
        }
        ectx.request_repaint();
    }

    /// Grava a célula editada (na fila: espera a anterior terminar e usa a chave que ela devolveu).
    fn salvar_edicao(&mut self, ectx: &egui::Context, ctx: &Contexto, ed: Edicao, foco: bool) {
        let mudou = match &ed.original {
            Celula::Nulo => !ed.texto.is_empty(),
            o => ed.texto != texto_edicao(o),
        };
        if !mudou {
            return;
        }
        let Some(d) = self.dados.as_ref().filter(|d| d.geracao == ed.geracao) else { return };
        let (Some(chave), Some(col)) = (d.chave(ed.linha).cloned(), d.colunas.get(ed.coluna)) else { return };
        let nome_coluna = col.nome.clone();
        let anterior = self.ultimo_sinal.take();
        let proprio = Arc::new(Sinal::default());
        self.ultimo_sinal = Some(Arc::clone(&proprio));
        let chaves = Arc::clone(&self.chaves_da_fila);
        let (geracao, linha) = (ed.geracao, ed.linha);
        let viva = Arc::clone(&self.viva);
        let ectx_tarefa = ectx.clone();
        let nome = self.nome.clone();
        let coluna = nome_coluna.clone();
        let texto = ed.texto.clone();
        let gancho = self.ganchos.gravacao.clone();
        let promessa = ctx.tarefa(ectx, move |s, id| {
            let _aviso = AvisarAoSair(proprio);
            if let Some(a) = anterior {
                a.esperar();
            }
            if let Some(g) = gancho {
                g();
            }
            // a chave da linha agora: a que uma gravação anterior da fila deixou nela, ou a lida da página
            let chave = chave_na_fila(&chaves, geracao, linha, chave);
            let r = s.atualizar(id, &nome, &chave, &[(coluna.clone(), Entrada::Texto(texto))]);
            match &r {
                Ok(g) => {
                    if let Some(nova) = g.resultado.chave.as_ref().filter(|n| **n != chave) {
                        trocar_na_fila(&chaves, geracao, linha, nova.clone());
                    }
                    if !viva.load(Ordering::SeqCst) && g.transacao_aberta {
                        toast_concluido(&ectx_tarefa, MSG_SO_NA_TRANSACAO);
                    }
                }
                Err(e) => {
                    // a tela já é outra: o toast é da casca inteira, e o valor digitado não some sem explicação
                    let silencio = e.tem_codigo(Codigo::BancoTrocado) || e.tem_codigo(Codigo::Substituido);
                    if !viva.load(Ordering::SeqCst) && !silencio {
                        toast_erro(
                            &ectx_tarefa,
                            format!("O valor de {coluna} em {nome} não foi salvo: {}", e.mensagem),
                        );
                    }
                }
            }
            r
        });
        self.pendentes.push(Pendente { geracao: ed.geracao, linha: ed.linha, coluna: ed.coluna, texto: ed.texto });
        self.gravacoes.push_back(GravacaoCelula {
            geracao: ed.geracao,
            linha: ed.linha,
            coluna: ed.coluna,
            nome_coluna,
            original: ed.original,
            foco,
            promessa,
        });
    }

    fn receber_gravacoes(&mut self, ectx: &egui::Context, ctx: &mut Contexto, agora: f64) {
        while let Some(r) = self.gravacoes.front_mut().and_then(|g| g.promessa.tomar()) {
            let Some(g) = self.gravacoes.pop_front() else { break };
            if let Some(i) =
                self.pendentes.iter().position(|p| p.geracao == g.geracao && p.linha == g.linha && p.coluna == g.coluna)
            {
                self.pendentes.remove(i);
            }
            let aqui = self.dados.as_ref().is_some_and(|d| d.geracao == g.geracao);
            match r {
                Ok(gravado) => {
                    // o toast diz onde gravou e oferece o Desfazer; dentro de um BEGIN aberto na Consulta SQL a célula
                    // pisca igual, mas o arquivo só muda com o COMMIT
                    match self.registrar_edicao(ectx, ctx, &g, &gravado) {
                        Some(u) => {
                            let frase = frase_gravado(&u.tabela, &u.chave, &u.coluna);
                            let frase = ctx.aviso_de_gravacao(gravado.transacao_aberta, &frase);
                            if self.em_vista {
                                let carga = desfazer::carga(u.serial);
                                let acao = AcaoToast { rotulo: ROTULO_ACAO.to_string(), carga };
                                toast_concluido_com_acao_por(ectx, frase, acao, PRAZO_TOAST);
                            } else {
                                toast_concluido(ectx, frase);
                            }
                        }
                        None => {
                            let frase = ctx.aviso_de_gravacao(gravado.transacao_aberta, "");
                            if !frase.is_empty() {
                                toast_concluido(ectx, frase);
                            }
                        }
                    }
                    if aqui {
                        self.aplicar_gravacao(g.linha, gravado.resultado, &[g.coluna], agora);
                        if g.foco {
                            self.devolver_foco(ectx, g.linha, g.coluna);
                        }
                    }
                }
                Err(e) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        let msg = if aqui {
                            e.mensagem.clone()
                        } else {
                            format!("O valor de {} em {} não foi salvo: {}", g.nome_coluna, self.nome, e.mensagem)
                        };
                        toast_erro(ectx, msg);
                    }
                    if aqui && g.foco {
                        self.devolver_foco(ectx, g.linha, g.coluna);
                    }
                }
            }
        }
        if self.gravacoes.is_empty() {
            self.ultimo_sinal = None;
            // toda troca de chave da fila já está na página (ou a página é outra, relida depois da fila)
            self.chaves_da_fila.lock().unwrap_or_else(|e| e.into_inner()).clear();
        }
    }

    /// A resposta de uma gravação feita com Enter chegou. A grade já recebeu o foco no Enter, e a pessoa pode ter andado ou começado
    /// outra edição enquanto esperava. O foco só volta para a célula gravada se ela ainda é a ativa, sem edição
    /// aberta, e o foco não foi para outro lugar.
    fn devolver_foco(&mut self, ectx: &egui::Context, l: usize, c: usize) {
        if self.edicao.is_some() {
            return;
        }
        let Some(g) = self.grade.as_ref() else { return };
        let id = g.nucleo.id_corpo();
        let mesma = g.nucleo.selecao() == Some((l, g.primeira() + c));
        let foco_livre = ectx.memory(|m| m.focused().is_none_or(|f| f == id));
        if mesma && foco_livre {
            self.focar_grade(ectx, None);
        }
    }

    /// Aplica a linha que o serviço devolveu (valores relidos, chave que pode ter mudado) e pisca o `.salvo`.
    fn aplicar_gravacao(&mut self, l: usize, r: LinhaGravada, piscar: &[usize], agora: f64) {
        let Some(d) = self.dados.as_mut() else { return };
        let Some(linha) = r.linha else {
            // um gatilho tirou a linha de onde estava
            self.carregar();
            return;
        };
        let antiga = d.chaves.get(l).cloned().flatten();
        if let Some(nova) = r.chave {
            d.trocar_chave(l, nova.clone());
            if let Some(antiga) = antiga
                && antiga != chave_texto(&nova)
                && self.marcadas.contem(&antiga)
            {
                self.marcadas.trocar(&antiga, &nova);
                self.selecao_mudou = true;
            }
        }
        let editando = self.edicao.as_ref().filter(|e| e.geracao == d.geracao && e.linha == l).map(|e| e.coluna);
        let geracao = d.geracao;
        let Some(valores) = d.pagina.linhas.get_mut(l).map(|x| &mut x.valores) else { return };
        for (i, v) in linha.into_iter().enumerate() {
            let Some(atual) = valores.get_mut(i) else { continue };
            let mudou = *atual != v;
            *atual = v;
            // célula em edição: quem a redesenha é a própria gravação
            if editando == Some(i) {
                continue;
            }
            if mudou || piscar.contains(&i) {
                self.piscadas.push(Piscada { geracao, linha: l, coluna: i, desde: agora });
            }
        }
    }

    // ------------------------------------------------------------------ desfazer no banco

    /// Guarda a edição que acabou de gravar como a última (o Desfazer a volta); sem a linha relida (um gatilho a
    /// tirou do lugar) não há o que desfazer. Devolve o registro novo.
    fn registrar_edicao(
        &self,
        ectx: &egui::Context,
        ctx: &Contexto,
        g: &GravacaoCelula,
        gravado: &Gravado<LinhaGravada>,
    ) -> Option<UltimaEdicao> {
        let r = &gravado.resultado;
        let novo = r.chave.clone().zip(r.linha.as_ref().and_then(|l| l.get(g.coluna)).cloned());
        let Some((chave, valor)) = novo else {
            desfazer::guardar(ectx, None);
            return None;
        };
        let u = UltimaEdicao {
            serial: desfazer::proximo_serial(),
            banco_id: ctx.banco_id(),
            tabela: self.nome.clone(),
            estrutura: self.estrutura,
            transacao: gravado.transacao_aberta,
            chave,
            coluna: g.nome_coluna.clone(),
            antigo: g.original.clone(),
            gravado: valor,
        };
        desfazer::guardar(ectx, Some(Desfazivel::Edicao(u.clone())));
        Some(u)
    }

    /// A última mudança desta tabela não se desfaz (excluir, incluir): o Desfazer desliga com a dica.
    fn bloquear_desfazer(&self, ectx: &egui::Context, ctx: &Contexto, dica: &'static str) {
        let registro = Desfazivel::Bloqueado { banco_id: ctx.banco_id(), tabela: self.nome.clone(), dica };
        desfazer::guardar(ectx, Some(registro));
    }

    /// O Desfazer desta aba agora, pelo registro do começo do quadro.
    fn situacao_desfazer(&self, estado: &estado::Estado) -> Situacao {
        desfazer::situacao(
            self.desfazivel.as_ref(),
            &estado.banco_id(),
            &self.nome,
            self.estrutura,
            estado.transacao_aberta(),
        )
    }

    /// O "Desfazer · Ctrl+Z" do toast da gravação; a carga de outro (o CSV) volta para quem a espera.
    fn acao_do_toast(&mut self, ectx: &egui::Context, ctx: &mut Contexto) {
        let Some(carga) = acao_do_toast(ectx) else { return };
        match desfazer::serial_da_carga(&carga) {
            Some(serial) => self.desfazer(ectx, ctx, Some(serial)),
            None => devolver_acao_do_toast(ectx, carga),
        }
    }

    /// Desfaz a última edição de célula (ou o último lote): grava o valor de antes com um UPDATE, se a célula ainda tem o
    /// que a edição gravou. Só com o Desfazer ligado (nesta tabela, sem gravação na fila) e, vindo do toast, só a edição
    /// que ele anunciou. Não há Refazer: o registro sai.
    fn desfazer(&mut self, ectx: &egui::Context, ctx: &Contexto, serial: Option<u64>) {
        if self.ocupada_para_escrever() || !self.grava(ctx.estado) {
            return;
        }
        let registro = desfazer::ler(ectx);
        let ligado = matches!(
            desfazer::situacao(
                registro.as_ref(),
                &ctx.banco_id(),
                &self.nome,
                self.estrutura,
                ctx.estado.transacao_aberta()
            ),
            Situacao::Ligado(_)
        );
        let Some(registro) = registro.filter(|_| ligado) else { return };
        let serial_do_registro = match &registro {
            Desfazivel::Edicao(u) => u.serial,
            Desfazivel::Lote(u) => u.serial,
            Desfazivel::Bloqueado { .. } => return,
        };
        if serial.is_some_and(|s| s != serial_do_registro) {
            return;
        }
        desfazer::guardar(ectx, None);
        self.desfazivel = None;
        self.desfazendo = match registro {
            Desfazivel::Edicao(u) => {
                let e = u.clone();
                let promessa = ctx.tarefa(ectx, move |s, id| {
                    s.desfazer_celula(id, &e.tabela, &e.chave, &e.coluna, &e.gravado, &e.antigo)
                });
                Some(Desfazendo::Edicao { edicao: u, promessa })
            }
            Desfazivel::Lote(u) => {
                let (tabela, celulas) = (u.tabela.clone(), u.celulas.clone());
                let promessa = ctx.tarefa(ectx, move |s, id| s.desfazer_intervalo(id, &tabela, &celulas));
                Some(Desfazendo::Lote { lote: u, promessa })
            }
            Desfazivel::Bloqueado { .. } => None,
        };
        ectx.request_repaint();
    }

    fn receber_desfazer(&mut self, ectx: &egui::Context, ctx: &mut Contexto, agora: f64) {
        let pronto = match self.desfazendo.as_mut() {
            Some(Desfazendo::Edicao { promessa, .. }) => promessa
                .tomar()
                .map(|r| r.map(|g| Gravado { resultado: vec![g.resultado], transacao_aberta: g.transacao_aberta })),
            Some(Desfazendo::Lote { promessa, .. }) => promessa.tomar(),
            None => None,
        };
        let Some(r) = pronto else { return };
        let Some(desfazendo) = self.desfazendo.take() else { return };
        let (tabela, celulas) = match desfazendo {
            Desfazendo::Edicao { edicao: u, .. } => {
                let celula = CelulaDesfeita { chave: u.chave, coluna: u.coluna, gravado: u.gravado, antigo: u.antigo };
                (u.tabela, vec![celula])
            }
            Desfazendo::Lote { lote: u, .. } => (u.tabela, u.celulas),
        };
        match r {
            Ok(g) => {
                let frase = frase_desfeito_lote(&tabela, &celulas);
                let frase = ctx.aviso_de_gravacao(g.transacao_aberta, &frase);
                toast_concluido(ectx, frase);
                // as células na página voltam e piscam, se as linhas estão nela (a chave é a de antes do Desfazer)
                let lugares: Vec<Option<(usize, usize)>> = celulas
                    .iter()
                    .map(|c| {
                        let d = self.dados.as_ref()?;
                        let l = d.linha_da_chave(&chave_texto(&c.chave))?;
                        Some((l, d.colunas.iter().position(|col| col.nome == c.coluna)?))
                    })
                    .collect();
                self.aplicar_por_linha(lugares.into_iter().zip(g.resultado), agora);
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    toast_erro(ectx, e.mensagem);
                }
            }
        }
    }

    /// Aplica as respostas de várias células, uma vez por linha (as da mesma linha trazem a mesma linha relida), e pisca
    /// as células gravadas.
    fn aplicar_por_linha(
        &mut self,
        respostas: impl IntoIterator<Item = (Option<(usize, usize)>, LinhaGravada)>,
        agora: f64,
    ) {
        let mut por_linha: Vec<(usize, Vec<usize>, LinhaGravada)> = Vec::new();
        for (lugar, r) in respostas {
            let Some((l, c)) = lugar else { continue };
            match por_linha.iter_mut().find(|(pl, _, _)| *pl == l) {
                Some((_, cols, ultima)) => {
                    cols.push(c);
                    *ultima = r;
                }
                None => por_linha.push((l, vec![c], r)),
            }
        }
        for (l, cols, r) in por_linha {
            self.aplicar_gravacao(l, r, &cols, agora);
        }
    }

    // ------------------------------------------------------------------ o Colar, o Limpar e o Definir como NULL

    /// Uma gravação (ou um Desfazer, uma exclusão, um lote) está a caminho do banco: os itens de escrita e o Desfazer
    /// esperam (a chave de uma linha pode estar mudando).
    fn ocupada_para_escrever(&self) -> bool {
        !self.gravacoes.is_empty() || self.lote.is_some() || self.desfazendo.is_some() || self.exclusao.is_some()
    }

    /// O lote do menu (ou do Ctrl+V, do Delete): sem nada que mude, só o aviso; acima de [`LIMITE_SEM_PERGUNTA`]
    /// células, a pergunta antes ("Colar 120 células em clientes?").
    fn pedir_lote(&mut self, ectx: &egui::Context, ctx: &Contexto, lote: Lote) {
        if lote.celulas.is_empty() {
            let aviso = if lote.de_fora > 0 { frase_de_fora(lote.de_fora) } else { NADA_MUDOU.to_string() };
            toast(ectx, aviso);
            return;
        }
        if lote.celulas.len() > LIMITE_SEM_PERGUNTA {
            let frase = lote.tipo.pergunta(lote.celulas.len(), &self.nome);
            let acao = lote.tipo.acao();
            self.confirmacao = Some(Confirmacao::new(frase).titulo(acao).rotulo(acao));
            self.pergunta = Some(Pergunta::Lote(lote));
            return;
        }
        self.gravar_lote(ectx, ctx, lote);
    }

    /// Grava o lote numa transação só (`Servico::atualizar_intervalo`, tudo ou nada).
    fn gravar_lote(&mut self, ectx: &egui::Context, ctx: &Contexto, lote: Lote) {
        if self.ocupada_para_escrever() || self.dados.as_ref().is_none_or(|d| d.geracao != lote.geracao) {
            return;
        }
        let nome = self.nome.clone();
        let pedido = lote.pedido();
        let promessa = ctx.tarefa(ectx, move |s, id| s.atualizar_intervalo(id, &nome, &pedido));
        self.lote = Some(LoteGravando { lote, promessa });
        ectx.request_repaint();
    }

    /// O lote voltou: as células na página (as linhas relidas), o registro do Desfazer e o toast "Gravado em clientes
    /// (9 células)" com o Desfazer; o que a cola deixou de fora vai junto.
    fn receber_lote(&mut self, ectx: &egui::Context, ctx: &mut Contexto, agora: f64) {
        let Some(r) = self.lote.as_mut().and_then(|l| l.promessa.tomar()) else { return };
        let Some(LoteGravando { lote, .. }) = self.lote.take() else { return };
        match r {
            Ok(gravado) => {
                let registro = lote.para_desfazer(&gravado.resultado).map(|celulas| UltimoLote {
                    serial: desfazer::proximo_serial(),
                    banco_id: ctx.banco_id(),
                    tabela: self.nome.clone(),
                    estrutura: self.estrutura,
                    transacao: gravado.transacao_aberta,
                    acao: lote.tipo.acao(),
                    celulas,
                });
                let celulas: Vec<CelulaDesfeita> = match &registro {
                    Some(u) => u.celulas.clone(),
                    None => lote
                        .celulas
                        .iter()
                        .map(|c| CelulaDesfeita {
                            chave: c.chave.clone(),
                            coluna: c.nome.clone(),
                            gravado: Celula::Nulo,
                            antigo: c.antigo.clone(),
                        })
                        .collect(),
                };
                let mut frase = frase_gravado_lote(&self.nome, &celulas);
                if lote.de_fora > 0 {
                    frase = format!("{frase}; {}", frase_de_fora(lote.de_fora));
                }
                let frase = ctx.aviso_de_gravacao(gravado.transacao_aberta, &frase);
                match &registro {
                    Some(u) if self.em_vista => {
                        let acao = AcaoToast { rotulo: ROTULO_ACAO.to_string(), carga: desfazer::carga(u.serial) };
                        toast_concluido_com_acao_por(ectx, frase, acao, PRAZO_TOAST);
                    }
                    _ => toast_concluido(ectx, frase),
                }
                desfazer::guardar(ectx, registro.map(Desfazivel::Lote));
                if self.dados.as_ref().is_some_and(|d| d.geracao == lote.geracao) {
                    let lugares = lote.celulas.iter().map(|c| Some((c.linha, c.coluna)));
                    self.aplicar_por_linha(lugares.zip(gravado.resultado), agora);
                }
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    toast_erro(ectx, e.mensagem);
                }
            }
        }
    }

    // ------------------------------------------------------------------ marcação e exclusão

    fn excluir_marcadas(&mut self) {
        let chaves = self.marcadas.chaves();
        self.perguntar_excluir(chaves);
    }

    /// Pergunta antes de excluir estas linhas ("Excluir 1 linha de clientes? Isso não pode ser desfeito.").
    fn perguntar_excluir(&mut self, chaves: Vec<Chave>) {
        if chaves.is_empty() || self.exclusao.is_some() {
            return;
        }
        let frase = format!(
            "Excluir {} de {}? Isso não pode ser desfeito.",
            plural(chaves.len(), "linha", "linhas"),
            self.nome
        );
        let titulo = if chaves.len() == 1 { "Excluir linha" } else { "Excluir linhas" };
        self.confirmacao = Some(Confirmacao::new(frase).titulo(titulo).rotulo("Excluir").perigo(true));
        self.pergunta = Some(Pergunta::ExcluirMarcadas(chaves));
    }

    fn receber_exclusao(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let Some(r) = self.exclusao.as_mut().and_then(Promessa::tomar) else { return };
        self.exclusao = None;
        match r {
            Ok(g) => {
                self.marcadas.limpar();
                self.selecao_mudou = true;
                self.bloquear_desfazer(ectx, ctx, DICA_EXCLUIR);
                let frase = format!("{}.", plural(g.resultado, "linha excluída", "linhas excluídas"));
                let frase = ctx.aviso_de_gravacao(g.transacao_aberta, &frase);
                toast_concluido(ectx, frase);
                self.pedir_foco_depois(ectx, None);
                self.recarregar(tabela);
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    toast_erro(ectx, e.mensagem);
                }
            }
        }
    }

    /// Pede o foco na grade da tela refeita (depois de incluir ou excluir).
    fn pedir_foco_depois(&self, ectx: &egui::Context, chave: Option<String>) {
        let ativa = self.grade.as_ref().and_then(|g| g.nucleo.selecao());
        ectx.data_mut(|d| d.insert_temp(id_foco_pendente(&self.nome), FocoPendente { chave, ativa }));
    }

    /// A contagem do cabeçalho muda: a casca relê a tabela e os objetos e refaz as abas; a aba nova lê a página e recebe o foco pendente. Se a casca atender sem refazer esta aba, ela mesma
    /// relê a página (ver [`AbaDados::ui`]).
    fn recarregar(&mut self, tabela: &mut TabelaAberta) {
        tabela.recarregar = true;
        self.esperando_recarga = true;
    }

    // ------------------------------------------------------------------ janela da linha

    fn abrir_linha(&mut self, ectx: &egui::Context, ctx: &Contexto, info: &TabelaInfo, l: usize, foco: Option<String>) {
        // um duplo clique no botão que abre a linha dispara dois cliques: a janela abre uma vez só
        if self.abrindo.is_some() || self.janela.is_some() {
            return;
        }
        let Some(d) = &self.dados else { return };
        let Some(linha) = d.pagina.linhas.get(l) else { return };
        let posicao = (d.pagina.pagina.saturating_sub(1)) * d.pagina.por_pagina + l as u64 + 1;
        match linha.chave.clone() {
            Some(chave) => {
                let nome = self.nome.clone();
                let c2 = chave.clone();
                let promessa = ctx.tarefa(ectx, move |s, id| s.linha(id, &nome, &c2));
                self.abrindo = Some(Abertura { chave, posicao, foco, duplicar: false, promessa });
            }
            None => {
                let valores: Vec<(String, Celula)> =
                    d.pagina.colunas.iter().cloned().zip(linha.valores.iter().cloned()).collect();
                self.mostrar_janela_linha(None, &valores, posicao, foco, info, nota_so_leitura(ctx));
            }
        }
    }

    fn receber_abertura(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let Some(r) = self.abrindo.as_mut().and_then(|a| a.promessa.tomar()) else { return };
        let Some(a) = self.abrindo.take() else { return };
        match r {
            Ok(linha) if a.duplicar => self.abrir_nova(tabela.info(), linha.valores, true),
            Ok(linha) => {
                let chave = linha.chave.clone().unwrap_or(a.chave);
                let nota = nota_so_leitura(ctx);
                self.mostrar_janela_linha(Some(chave), &linha.valores, a.posicao, a.foco, tabela.info(), nota);
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    toast_erro(ectx, e.mensagem);
                }
            }
        }
    }

    fn colunas_da_janela(&self) -> Vec<String> {
        match &self.dados {
            Some(d) => d.pagina.colunas.clone(),
            None => self.nomes_visiveis.clone(),
        }
    }

    fn mostrar_janela_linha(
        &mut self,
        chave: Option<Chave>,
        valores: &[(String, Celula)],
        posicao: u64,
        foco: Option<String>,
        info: &TabelaInfo,
        nota: Option<String>,
    ) {
        let colunas = self.colunas_da_janela();
        self.serial_janela += 1;
        let serial = self.serial_janela;
        let titulo = titulo_linha(chave.as_ref(), posicao);
        // sem chave (PK nula) não há como apontar para a linha ao gravar: ela abre só para leitura
        let janela = match chave {
            Some(chave) if self.editavel => {
                JanelaLinha::edicao(serial, info, &colunas, ModoJanela::Editar { chave }, titulo, valores, foco)
            }
            chave => {
                let sem_chave = self.editavel && chave.is_none();
                // o aviso de atenção da linha sem chave, ou a nota do banco só para leitura
                let aviso = if sem_chave {
                    Some(AvisoJanela::Texto { mensagem: AVISO_SEM_CHAVE.to_string(), perigo: false })
                } else {
                    nota.map(AvisoJanela::Nota)
                };
                JanelaLinha::leitura(serial, info, &colunas, chave, titulo, valores, aviso)
            }
        };
        self.janela = Some(janela);
    }

    fn abrir_nova(&mut self, info: &TabelaInfo, base: Vec<(String, Celula)>, duplicada: bool) {
        let colunas = self.colunas_da_janela();
        self.serial_janela += 1;
        self.janela = Some(JanelaLinha::edicao(
            self.serial_janela,
            info,
            &colunas,
            ModoJanela::Nova { duplicada },
            "Nova linha".to_string(),
            &base,
            None,
        ));
    }

    fn desenhar_janelas(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        if let Some(j) = self.janela.as_mut()
            && let Some(acao) = j.mostrar(ectx)
        {
            self.acao_janela(ectx, ctx, tabela, acao);
        }
        if let Some(sim) = confirmar(ectx, &mut self.confirmacao) {
            let pergunta = self.pergunta.take();
            if sim {
                match pergunta {
                    Some(Pergunta::ExcluirMarcadas(chaves)) => {
                        let nome = self.nome.clone();
                        self.exclusao = Some(ctx.tarefa(ectx, move |s, id| s.excluir(id, &nome, &chaves)));
                    }
                    Some(Pergunta::ExcluirLinha { serial }) => self.excluir_linha_da_janela(ectx, ctx, serial),
                    Some(Pergunta::Lote(lote)) => self.gravar_lote(ectx, ctx, lote),
                    None => {}
                }
            }
        }
    }

    fn acao_janela(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta, acao: AcaoJanela) {
        let Some(j) = self.janela.as_mut() else { return };
        let serial = j.serial();
        match acao {
            AcaoJanela::Fechar => self.janela = None,
            AcaoJanela::Copiar(texto) => {
                ectx.copy_text(texto);
                toast(ectx, "Copiado.");
            }
            AcaoJanela::Enviar(valores) => {
                let nova = j.nova();
                if !nova && valores.is_empty() {
                    self.janela = None;
                    toast(ectx, "Nenhuma alteração para salvar.");
                    return;
                }
                if self.tarefa_janela.is_some() {
                    return;
                }
                j.aviso = None;
                j.ocupado = true;
                let nome = self.nome.clone();
                let chave = j.chave().cloned();
                let c2 = chave.clone();
                let promessa = ctx.tarefa(ectx, move |s, id| match &c2 {
                    Some(chave) if !nova => s.atualizar(id, &nome, chave, &valores),
                    _ => s.inserir(id, &nome, &valores),
                });
                self.tarefa_janela = Some(TarefaJanela::Gravar { serial, nova, chave, promessa });
            }
            AcaoJanela::Excluir => {
                let Some(chave) = j.chave() else { return };
                let frase = format!(
                    "Excluir a linha {} de {}? Isso não pode ser desfeito.",
                    descricao_exclusao(chave),
                    self.nome
                );
                self.confirmacao = Some(Confirmacao::new(frase).titulo("Excluir linha").rotulo("Excluir").perigo(true));
                self.pergunta = Some(Pergunta::ExcluirLinha { serial });
            }
            AcaoJanela::Duplicar(base) => {
                self.janela = None;
                self.abrir_nova(tabela.info(), base, true);
            }
            AcaoJanela::SalvarBlob { coluna, inicio } => {
                let Some(chave) = j.chave().cloned() else { return };
                let sugerido = format!("{}_{}.{}", self.nome, coluna, extensao_do_blob(&inicio));
                let caminho = match self.ganchos.salvar_como.clone() {
                    Some(c) => Some(c),
                    None => ctx.dialogos().salvar_como(&sugerido, FormatoArquivo::Arquivo).map(|e| e.caminho),
                };
                let Some(caminho) = caminho else { return };
                if self.tarefa_janela.is_some() {
                    return;
                }
                let nome = self.nome.clone();
                // o diálogo do Windows já perguntou antes de substituir
                let promessa = ctx.tarefa(ectx, move |s, id| s.salvar_blob(id, &nome, &chave, &coluna, &caminho, true));
                self.tarefa_janela = Some(TarefaJanela::SalvarBlob { serial, promessa });
            }
            AcaoJanela::EscolherArquivo { campo } => {
                let caminho: Option<PathBuf> = match self.ganchos.escolher_arquivo.clone() {
                    Some(c) => Some(c),
                    None => ctx.dialogos().escolher_arquivo("").map(|e| e.caminho),
                };
                if let (Some(c), Some(j)) = (caminho, self.janela.as_mut()) {
                    j.escolher_arquivo(campo, c);
                }
            }
        }
    }

    fn excluir_linha_da_janela(&mut self, ectx: &egui::Context, ctx: &Contexto, serial: u64) {
        let Some(j) = self.janela.as_mut().filter(|j| j.serial() == serial) else { return };
        let Some(chave) = j.chave().cloned() else { return };
        if self.tarefa_janela.is_some() {
            return;
        }
        j.aviso = None;
        j.ocupado = true;
        let nome = self.nome.clone();
        let c2 = chave.clone();
        let promessa = ctx.tarefa(ectx, move |s, id| s.excluir(id, &nome, std::slice::from_ref(&c2)));
        self.tarefa_janela = Some(TarefaJanela::Excluir { serial, chave, promessa });
    }

    fn receber_janela(&mut self, ectx: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta, agora: f64) {
        let pronta = match self.tarefa_janela.as_mut() {
            None => return,
            Some(TarefaJanela::Gravar { promessa, .. }) => promessa.tomar().map(|r| r.map(Feito::Gravado)),
            Some(TarefaJanela::Excluir { promessa, .. }) => promessa.tomar().map(|r| r.map(Feito::Excluido)),
            Some(TarefaJanela::SalvarBlob { promessa, .. }) => promessa.tomar().map(|r| r.map(Feito::Blob)),
        };
        let Some(r) = pronta else { return };
        let Some(tarefa) = self.tarefa_janela.take() else { return };
        let (serial, nova, chave) = match &tarefa {
            TarefaJanela::Gravar { serial, nova, chave, .. } => (*serial, *nova, chave.clone()),
            TarefaJanela::Excluir { serial, chave, .. } => (*serial, false, Some(chave.clone())),
            TarefaJanela::SalvarBlob { serial, .. } => (*serial, false, None),
        };
        let esta = |j: &JanelaLinha| j.serial() == serial;
        match r {
            Err(e) => {
                if let Some(j) = self.janela.as_mut().filter(|j| esta(j)) {
                    j.ocupado = false;
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        j.aviso = Some(AvisoJanela::Texto { mensagem: e.mensagem, perigo: true });
                    }
                } else {
                    let _ = ctx.reagir(&e);
                }
            }
            Ok(Feito::Blob(b)) => {
                if let Some(j) = self.janela.as_mut().filter(|j| esta(j)) {
                    let texto = b.caminho.to_string_lossy();
                    j.aviso =
                        Some(AvisoJanela::ArquivoSalvo { nome: nome_do_caminho(&texto).to_string(), bytes: b.bytes });
                }
            }
            Ok(Feito::Gravado(g)) => {
                if self.janela.as_ref().is_some_and(esta) {
                    self.janela = None;
                }
                // a ficha muda a linha inteira: incluir não se desfaz, e salvar troca a última edição de célula
                if nova {
                    self.bloquear_desfazer(ectx, ctx, DICA_INCLUIR);
                } else {
                    desfazer::guardar(ectx, None);
                }
                let frase =
                    ctx.aviso_de_gravacao(g.transacao_aberta, if nova { "Linha incluída." } else { "Linha salva." });
                toast_concluido(ectx, frase);
                if nova {
                    self.pedir_foco_depois(ectx, g.resultado.chave.as_ref().map(chave_texto));
                    self.recarregar(tabela);
                } else {
                    let idx = chave
                        .as_ref()
                        .and_then(|k| self.dados.as_ref().and_then(|d| d.linha_da_chave(&chave_texto(k))));
                    match idx {
                        Some(i) => {
                            // uma gravação no lugar desta linha ainda na fila sai com a chave que a janela deu
                            if !self.gravacoes.is_empty()
                                && let (Some(nova), Some(d)) = (g.resultado.chave.as_ref(), self.dados.as_ref())
                            {
                                trocar_na_fila(&self.chaves_da_fila, d.geracao, i, nova.clone());
                            }
                            self.aplicar_gravacao(i, g.resultado, &[], agora)
                        }
                        None => self.carregar(),
                    }
                }
            }
            Ok(Feito::Excluido(g)) => {
                if let Some(k) = &chave
                    && self.marcadas.definir(&chave_texto(k), k, false)
                {
                    self.selecao_mudou = true;
                }
                if self.janela.as_ref().is_some_and(esta) {
                    self.janela = None;
                }
                self.bloquear_desfazer(ectx, ctx, DICA_EXCLUIR);
                let frase = ctx.aviso_de_gravacao(g.transacao_aberta, "Linha excluída.");
                toast_concluido(ectx, frase);
                self.pedir_foco_depois(ectx, None);
                self.recarregar(tabela);
            }
        }
    }

    // ------------------------------------------------------------------ testes

    /// O que a aba mostra agora (para os testes).
    #[doc(hidden)]
    pub fn retrato(&self, tabela: &TabelaAberta) -> Retrato {
        let faixa = match self.contagem_faixa(tabela.info()) {
            ContagemFiltros::Texto(t) => t,
            ContagemFiltros::Contando => CONTANDO.to_string(),
            ContagemFiltros::NaoContadas(_) => LINHAS_NAO_CONTADAS.to_string(),
            ContagemFiltros::Nada => String::new(),
        };
        let rodape = self.rodape(tabela.info(), false);
        let primeira = self.grade.as_ref().map_or(0, GradeDados::primeira);
        Retrato {
            faixa,
            contagem_linhas: rodape.as_ref().map_or_else(String::new, |r| match &r.sem_total {
                None => r.contagem.clone(),
                Some(None) => format!("{}{CONTANDO}", r.contagem),
                Some(Some(_)) => format!("{}…", r.contagem),
            }),
            pagina_atual: rodape.as_ref().map_or_else(String::new, |r| r.pagina.clone()),
            anterior_habilitado: rodape.as_ref().is_some_and(|r| r.anterior),
            proxima_habilitada: rodape.as_ref().is_some_and(|r| r.proxima),
            sugestao: rodape.and_then(|r| r.sugestao).map(|s| match s {
                Sugestao::Indice { texto, .. } => texto,
                Sugestao::Busca => DICA_BUSCA_LENTA.to_string(),
            }),
            marcadas: self.marcadas.len(),
            linhas: self.dados.as_ref().map_or(0, Dados::n_linhas),
            carregando: self.carga.is_some() || self.carga_pendente,
            vazio: self.dados.as_ref().filter(|d| d.n_linhas() == 0).map(|_| self.texto_vazio().to_string()),
            erro: self.erro.clone(),
            ativa: self
                .grade
                .as_ref()
                .and_then(|g| g.nucleo.selecao())
                .and_then(|(l, c)| c.checked_sub(primeira).map(|c| (l, c))),
            editando: self.edicao.as_ref().map(|e| (e.linha, e.coluna, e.texto.clone())),
            janela: self.janela.as_ref().map(|j| j.titulo.clone()),
            janela_aviso: self.janela.as_ref().and_then(|j| match &j.aviso {
                None => None,
                Some(AvisoJanela::Texto { mensagem, .. } | AvisoJanela::Nota(mensagem)) => Some(mensagem.clone()),
                Some(AvisoJanela::ArquivoSalvo { nome, bytes }) => {
                    Some(format!("Arquivo salvo: {nome} · {}", interface::formato::formatar_bytes(*bytes)))
                }
            }),
            confirmacao: self.confirmacao.as_ref().map(|c| c.frase.clone()),
            gravacoes_pendentes: self.gravacoes.len() + usize::from(self.lote.is_some()),
            substituidas: self.substituidas,
            filtros: self.filtros.len(),
            condicao_aberta: self.condicao.is_some(),
            desfazendo: self.desfazendo.is_some(),
        }
    }

    /// O texto que a célula (linha, coluna de dados) mostra.
    #[doc(hidden)]
    pub fn texto_celula(&self, linha: usize, coluna: usize) -> Option<String> {
        let d = self.dados.as_ref()?;
        let col = d.colunas.get(coluna)?;
        if col.abre {
            return Some(crate::regras::rotulo_linha(d.chave(linha)));
        }
        d.celula(linha, coluna).map(|c| texto_exibicao(c).into_owned())
    }

    /// A célula (linha, coluna de dados) como veio do serviço.
    #[doc(hidden)]
    pub fn valor_celula(&self, linha: usize, coluna: usize) -> Option<Celula> {
        self.dados.as_ref()?.celula(linha, coluna).cloned()
    }

    /// O campo de uma coluna na janela ocupa a linha inteira (`.span2`: texto longo, BLOB).
    #[doc(hidden)]
    pub fn campo_janela_largo(&self, coluna: &str) -> Option<bool> {
        Some(self.janela.as_ref()?.campos.iter().find(|f| f.meta.nome == coluna)?.largo)
    }

    /// Os nomes das colunas de dados da grade, na ordem.
    #[doc(hidden)]
    pub fn colunas_da_grade(&self) -> Vec<String> {
        self.dados.as_ref().map(|d| d.colunas.iter().map(|c| c.nome.clone()).collect()).unwrap_or_default()
    }

    /// O texto embaixo do nome no `th` de uma coluna de dados ("TEXT", "INTEGER · gerada").
    #[doc(hidden)]
    pub fn tipo_da_coluna(&self, coluna: usize) -> Option<String> {
        Some(self.dados.as_ref()?.colunas.get(coluna)?.rotulo_tipo.clone())
    }

    /// A coluna de dados é numérica (alinha à direita, `td.num`).
    #[doc(hidden)]
    pub fn coluna_numerica(&self, coluna: usize) -> Option<bool> {
        Some(self.dados.as_ref()?.colunas.get(coluna)?.numerica)
    }

    /// A grade tem a coluna à parte do número da linha ("rowid"/"abrir").
    #[doc(hidden)]
    pub fn tem_coluna_abrir(&self) -> Option<bool> {
        self.grade.as_ref().map(|g| g.tem_abrir)
    }

    /// A grade tem a coluna das caixas de marcar (tabela editável).
    #[doc(hidden)]
    pub fn tem_caixas(&self) -> Option<bool> {
        self.grade.as_ref().map(|g| g.tem_marca)
    }

    /// O texto do número da linha (o abridor) de uma linha.
    #[doc(hidden)]
    pub fn rotulo_da_linha(&self, linha: usize) -> Option<String> {
        let d = self.dados.as_ref()?;
        (linha < d.n_linhas()).then(|| crate::regras::rotulo_linha(d.chave(linha)))
    }

    /// A célula (linha, coluna de dados) acabou de gravar e pisca o `.salvo`.
    #[doc(hidden)]
    pub fn piscando(&self, linha: usize, coluna: usize) -> bool {
        let g = self.dados.as_ref().map_or(0, |d| d.geracao);
        self.piscadas.iter().any(|p| p.geracao == g && p.linha == linha && p.coluna == coluna)
    }

    /// O centro de uma célula (linha, coluna de dados) na tela, se estiver à vista.
    #[doc(hidden)]
    pub fn centro_celula(&self, linha: usize, coluna: usize) -> Option<Pos2> {
        self.grade.as_ref()?.centro_celula(linha, coluna)
    }

    /// O centro da caixa de marcar de uma linha (`None`: a do cabeçalho).
    #[doc(hidden)]
    pub fn centro_marca(&self, linha: Option<usize>) -> Option<Pos2> {
        self.grade.as_ref()?.centro_marca(linha)
    }

    /// Faz da primeira célula da linha a ativa e rola até ela (como as setas fariam).
    #[doc(hidden)]
    pub fn rolar_ate(&mut self, linha: usize) {
        if let Some(g) = self.grade.as_mut() {
            let primeira = g.primeira();
            g.nucleo.selecionar(Some((linha, primeira)));
        }
    }

    /// O id do corpo da grade (o que recebe o foco do teclado).
    #[doc(hidden)]
    pub fn id_grade(&self) -> Option<Id> {
        self.grade.as_ref().map(|g| g.nucleo.id_corpo())
    }

    /// O id do campo de uma coluna na janela da linha.
    #[doc(hidden)]
    pub fn id_campo_janela(&self, coluna: &str) -> Option<Id> {
        let j = self.janela.as_ref()?;
        let i = j.campos.iter().position(|f| f.meta.nome == coluna)?;
        Some(Id::new(("tela_dados::campo", j.serial(), i)))
    }

    /// A caixa "NULL" e a "Usar o padrão" de um campo da janela: (nulo, padrão).
    #[doc(hidden)]
    pub fn caixas_campo_janela(&self, coluna: &str) -> Option<(Option<bool>, Option<bool>)> {
        let f = self.janela.as_ref()?.campos.iter().find(|f| f.meta.nome == coluna)?;
        Some((f.nulo, f.padrao))
    }

    /// O texto de um campo da janela.
    #[doc(hidden)]
    pub fn texto_campo_janela(&self, coluna: &str) -> Option<String> {
        let f = self.janela.as_ref()?.campos.iter().find(|f| f.meta.nome == coluna)?;
        Some(f.texto.clone())
    }

    /// Quantas linhas a ficha de leitura da janela tem (e se é a de leitura).
    #[doc(hidden)]
    pub fn linhas_leitura_janela(&self) -> Option<usize> {
        let j = self.janela.as_ref()?;
        matches!(j.modo, ModoJanela::Leitura { .. }).then_some(j.leitura.len())
    }
}

/// O que a janela devolveu, já sem o tipo de cada tarefa.
enum Feito {
    Gravado(Gravado<LinhaGravada>),
    Excluido(Gravado<usize>),
    Blob(BlobSalvo),
}

/// O que a pessoa fez num quadro (aplicado depois de desenhar).
#[derive(Default)]
struct AcoesQuadro {
    ordenar: Option<String>,
    /// O "Filtrar por este valor" e o "Excluir este valor do filtro": a condição nova (coluna, operador, valor).
    filtrar: Option<(String, Operador, String)>,
    /// O "Marcar as linhas" (as da página).
    marcar_linhas: Option<std::ops::Range<usize>>,
    criar_indice: Option<String>,
    abrir: Option<usize>,
    marcar: Option<(usize, bool)>,
    marcar_pagina: Option<bool>,
    fim_edicao: Option<FimEdicao>,
    editar: Option<(usize, usize)>,
    digitado: Option<(usize, usize, String)>,
    excluir: bool,
    espaco: bool,
    /// O Colar, o Limpar ou o Definir como NULL (o lote, ou por que ele não vale agora).
    lote: Option<Result<Lote, String>>,
    /// O "Excluir as linhas" do menu: as chaves das linhas do intervalo.
    excluir_linhas: Option<Vec<Chave>>,
    /// O botão do vazio da grade.
    vazio: Option<AcaoVazio>,
}

/// O botão do vazio da grade (U5 da revisão de UI de 22/10/2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcaoVazio {
    NovaLinha,
    LimparFiltros,
}

/// A nota da ficha só leitura de um banco aberto só para leitura (prancheta 15).
fn nota_so_leitura(ctx: &Contexto) -> Option<String> {
    let banco = ctx.estado.banco.as_ref().filter(|b| b.somente_leitura)?;
    Some(format!("{} está aberto só para leitura. Nada é gravado no arquivo.", banco.nome))
}

/// Os textos do select de tamanho de página ("100 por página").
fn opcoes_por_pagina(atual: u64) -> Vec<(u64, String)> {
    tamanhos_por_pagina(atual).into_iter().map(|t| (t, format!("{} por página", formatar_numero(t)))).collect()
}

/// O índice que já serve para ordenar pela coluna: o primeiro campo dele é ela (e ele cobre a tabela inteira).
fn tem_indice_por(info: &TabelaInfo, coluna: &str) -> bool {
    info.indices.iter().any(|i| !i.parcial && i.colunas.first().is_some_and(|c| c.nome == coluna))
}

/// Um bloco com o padding do `.aviso-grade` (18px 20px).
fn linha_de_aviso(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(20, 18)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        conteudo(ui);
    });
}

impl AbaDados {
    /// A aba grava (tabela editável com chave, num banco que não é só leitura): Nova linha, Duplicar e Excluir.
    fn grava(&self, estado: &estado::Estado) -> bool {
        self.editavel && !estado.somente_leitura()
    }

    /// A chave da linha da célula ativa (se ela tiver chave).
    fn chave_da_ativa(&self) -> Option<Chave> {
        let (l, _) = self.grade.as_ref()?.nucleo.selecao()?;
        self.dados.as_ref()?.chave(l).cloned()
    }

    /// O alvo do Duplicar: a linha marcada, se for uma só; sem marcadas, a da célula ativa.
    fn alvo_unico(&self) -> Option<Chave> {
        match self.marcadas.len() {
            0 => self.chave_da_ativa(),
            1 => self.marcadas.primeira().cloned(),
            _ => None,
        }
    }

    /// O alvo do Excluir: as marcadas ou, sem marcadas, a linha da célula ativa.
    fn alvo_excluir(&self) -> Vec<Chave> {
        if self.marcadas.is_empty() { self.chave_da_ativa().into_iter().collect() } else { self.marcadas.chaves() }
    }

    /// Lê a linha inteira (os textos cortados da página não servem) e abre a ficha "Nova linha" com os valores dela
    /// (o BLOB não vem: ver `montar_campo`).
    fn duplicar(&mut self, ectx: &egui::Context, ctx: &Contexto, chave: Chave) {
        if self.abrindo.is_some() || self.janela.is_some() {
            return;
        }
        let nome = self.nome.clone();
        let c2 = chave.clone();
        let promessa = ctx.tarefa(ectx, move |s, id| s.linha(id, &nome, &c2));
        self.abrindo = Some(Abertura { chave, posicao: 0, foco: None, duplicar: true, promessa });
    }

    /// A seção "Coluna" das Propriedades, da coluna da célula ativa: o nome no título; Tipo (a pastilha e o tipo),
    /// Regras (pills), Padrão e Valor (o texto cru da célula, como a grade o edita).
    fn secao_coluna(&self, ui: &mut Ui, info: &TabelaInfo) {
        let ativa = self.grade.as_ref().and_then(|g| {
            let (l, c) = g.nucleo.selecao()?;
            Some((l, c.checked_sub(g.primeira())?))
        });
        let Some(((l, c), d)) = ativa.zip(self.dados.as_ref()) else { return };
        let Some(col) = d.colunas.get(c) else { return };
        let meta = info.colunas.iter().find(|m| m.nome == col.nome);
        let valor = d.celula(l, c).map(|v| if v.e_nulo() { None } else { Some(texto_celula(v)) }).unwrap_or(None);
        let _ = Secao::new(("propriedades::coluna", &self.nome), "Coluna").extra(&col.nome).mostrar(ui, |ui| {
            ficha(ui, |f| {
                f.linha("Tipo", |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        icones::pintar(ui.painter(), r, Icone::Tipo(col.pastilha), egui::Color32::WHITE);
                        let tipo = if col.tipo.is_empty() { "sem tipo" } else { col.tipo.as_str() };
                        texto_simples(ui, tipo, 12.0, paleta(ui).estudio.t);
                    });
                });
                let pills = meta.map(regras_em_pills).unwrap_or_default();
                if pills.is_empty() {
                    f.linha_texto("Regras", None, "—");
                } else {
                    f.linha("Regras", |ui| {
                        // a linha vale a pill (16px, numa linha de 17), e não a altura mínima de um controle (o
                        // `horizontal_wrapped` a lê ao nascer): 4px entre as pills empilhadas (V16 da revisão de
                        // 17/10/2026)
                        ui.spacing_mut().interact_size.y = 17.0;
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                            for (texto, tipo) in &pills {
                                pill(ui, texto, *tipo);
                            }
                        });
                    });
                }
                f.linha_texto("Padrão", meta.and_then(|m| m.padrao.as_deref()), "—");
                f.linha_texto("Valor", valor.as_deref(), "NULL");
            });
        });
    }
}

/// A aba Dados como parte da tela de tabela, no formato de documento (prancheta 2): a faixa com as condições, a
/// grade com a rolagem dela e o rodapé com as páginas. A busca da barra ("Buscar em clientes") é o texto da busca da
/// aba, com a mesma espera de 0,3 s depois de parar de digitar (`ESPERA_DIGITACAO`); o Enter aplica na hora, e o Esc
/// com texto limpa e aplica. A página, a busca e os filtros continuam indo para o endereço. Os verbos: Nova linha,
/// Duplicar (com um alvo só), Excluir (as marcadas ou a linha da célula ativa) e Filtrar (o popover numa condição
/// nova); numa visão ou num banco só leitura, só Filtrar e Buscar (a tela de tabela junta Atualizar e Exportar).
impl estado::ParteTabela for AbaDados {
    fn anatomia(&self) -> Anatomia {
        Anatomia::default().com_faixa().com_rodape().rolagem_propria()
    }

    fn destacar_coluna(&mut self, coluna: Option<&str>) {
        if self.destacada.as_deref() != coluna {
            self.destacada = coluna.map(str::to_string);
        }
    }

    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.desenhar_faixa(ui, ctx, tabela);
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.conteudo_da_parte(ui, ctx, tabela);
    }

    fn rodape(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.desenhar_rodape(ui, ctx, tabela);
    }

    fn ofertas(&self, _tabela: &TabelaAberta, estado: &estado::Estado) -> Ofertas {
        let n = self.filtros.len();
        let o = Ofertas::default()
            .ligar(Verbo::Buscar)
            .com(Verbo::Filtrar, Oferta::ligada().marcada_se(n > 0).com_dica(dica_filtrar(n)));
        if !self.grava(estado) {
            return o;
        }
        let ocupado = self.ocupada_para_escrever();
        let desfazer = match self.situacao_desfazer(estado) {
            Situacao::Ligado(dica) => Oferta::ligada().ligada_se(!ocupado).com_dica(dica),
            Situacao::Desligado(dica) => Oferta::default().com_dica(dica),
            Situacao::Nada => Oferta::default(),
        };
        let o = o.com(Verbo::Desfazer, desfazer);
        // chamado a cada quadro: só a contagem das marcadas e, no máximo, uma chave (nenhuma cópia da lista)
        let marcadas = self.marcadas.len();
        let ativa = if marcadas == 0 { self.chave_da_ativa() } else { None };
        let unica = match marcadas {
            0 => ativa.as_ref(),
            1 => self.marcadas.primeira(),
            _ => None,
        };
        let duplicar = match unica {
            Some(chave) => Oferta::ligada().com_dica(dica_duplicar(Some(chave), marcadas)),
            None => Oferta::default().com_dica(dica_duplicar(None, marcadas)),
        };
        // o alvo do Excluir: as marcadas ou, sem marcadas, a linha da célula ativa; a dica só olha uma chave só
        let ha_alvo = marcadas > 0 || ativa.is_some();
        let excluir = Oferta::ligada()
            .ligada_se(ha_alvo && self.exclusao.is_none())
            .com_dica(dica_excluir(unica.map_or(&[][..], std::slice::from_ref), marcadas));
        o.com(Verbo::Novo, Oferta::ligada().com_rotulo("Nova linha"))
            .com(Verbo::Duplicar, duplicar)
            .com(Verbo::Excluir, excluir)
    }

    fn executar(&mut self, verbo: Verbo, egui: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        match verbo {
            Verbo::Novo => self.abrir_nova(tabela.info(), Vec::new(), false),
            Verbo::Duplicar => {
                if let Some(chave) = self.alvo_unico() {
                    self.duplicar(egui, ctx, chave);
                }
            }
            Verbo::Excluir => {
                let alvo = self.alvo_excluir();
                self.perguntar_excluir(alvo);
            }
            Verbo::Filtrar => {
                let ancora = self.rect_condicao.unwrap_or(Rect::ZERO);
                self.abrir_condicao(None, ancora);
            }
            Verbo::Desfazer => self.desfazer(egui, ctx, None),
            _ => {}
        }
        egui.request_repaint();
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.secao_coluna(ui, tabela.info());
        estado::secao_abrir_na_consulta(ui, ctx, &self.nome);
    }

    /// As respostas que chegam com a aba fora de vista: as gravações, a exclusão e as tarefas da ficha dão o toast na
    /// hora e notam a transação (item 6 do contrato).
    fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        let agora = egui.input(|i| i.time);
        self.desfazivel = desfazer::ler(egui);
        self.em_vista = false;
        self.receber(egui, ctx, tabela, agora);
        if self.selecao_mudou {
            self.selecao_mudou = false;
            tabela.selecao = self.marcadas.chaves();
        }
    }

    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        let dica = format!("Buscar em {}", self.nome);
        Some(estado::BuscaDaBarra { texto: &mut self.busca, dica: dica.into() })
    }

    fn evento_busca(
        &mut self,
        evento: estado::EventoBusca,
        egui: &egui::Context,
        ctx: &mut Contexto,
        tabela: &mut TabelaAberta,
    ) {
        match evento {
            estado::EventoBusca::Mudou => {
                let agora = egui.input(|i| i.time);
                // a busca da barra é desenhada antes da aba: a espera que já venceu (a tecla chegou depois dos 0,3 s)
                // aplica antes de começar a próxima, como o temporizador faria no quadro dele
                if self.busca_mudou.is_some_and(|t| agora - t >= ESPERA_DIGITACAO) {
                    self.aplicar_busca(ctx, tabela);
                }
                self.busca_mudou = Some(agora);
            }
            estado::EventoBusca::Enter | estado::EventoBusca::Anterior | estado::EventoBusca::Limpar => {
                self.aplicar_busca(ctx, tabela)
            }
        }
        egui.request_repaint();
    }
}
