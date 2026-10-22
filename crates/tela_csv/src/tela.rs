//! O documento do CSV ([`TelaCsv`]): o carregamento com progresso, a fila de tarefas que falam com a tabela, a grade
//! paginada, o rodapé, as Propriedades e a barra de status. A casca (faixa E) só o desenha como `estado::Documento`.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comum::{Codigo, Resultado};
use egui::{Align, Layout, Pos2, Sense, Ui, UiBuilder, vec2};
use estado::{
    Acao, Anatomia, Contexto, Documento, Estado as EstadoApp, Ofertas, Promessa, Reacao, Situacao, StatusDeArquivo,
    Verbo,
};
use interface::formato::contagem_linhas;
use interface::pecas::avisos::{Aviso, TomAviso, aviso_texto, carregando};
use interface::pecas::botao::Botao;
use interface::pecas::janela::{Confirmacao, confirmar};
use interface::pecas::progresso::barra_progresso;
use interface::pecas::texto::texto as texto_simples;
use interface::tema::paleta;
use tabular::csv::{OpcoesAbertura, Tabela};

use crate::colunas::{AvisoCola, CabecalhoEmEdicao, ColunaDaGrade, LeituraSobre, PedidoCola};
use crate::faixa::EdicaoCondicao;
use crate::filtro::{CalculoVista, ChaveVista, Condicao, Ordem};
use crate::fonte_csv::FonteCsv;
use crate::grade::GradeCsv;
use crate::janela_trazer::JanelaTrazer;
use crate::pintor::{Edicao, Pendente, Pintor};
use crate::propriedades::{self, Estatistica, PedidoPropriedades};
use crate::sessao::{
    self, Controle, Efeito, Janela, LINHAS_DA_PAGINA, Pagina, Resposta, Retrato, TabelaCompartilhada, Trabalho,
};
use crate::textos;

/// Quanto o carregamento espera antes de mostrar o card com a barra (um arquivo pequeno abre sem piscar nada).
pub const ESPERA_DO_CARD: f64 = 0.4;
/// A altura mínima da grade (o cabeçalho e uma linha), numa janela muito baixa.
const ALTURA_GRADE_MIN: f32 = 58.0;
/// A folga, em linhas, que a página pedida deixa acima das visíveis (rolar para cima sem esperar).
const FOLGA_ACIMA: u64 = 60;
/// O recuo do card de carregamento e do aviso de erro no alto do conteúdo.
const RECUO_AVISO: f32 = 20.0;
/// A largura da barra de progresso do card.
const LARGURA_BARRA: f32 = 320.0;

/// Ganchos dos testes: segurar uma tarefa (a máquina lenta) sem um arquivo enorme.
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct Ganchos {
    /// Chamado na thread da tarefa antes de abrir o arquivo.
    pub abrir: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Chamado na thread da tarefa antes de cada trabalho da fila.
    pub trabalho: Option<Arc<dyn Fn() + Send + Sync>>,
    /// O caminho que o diálogo "Outro arquivo" do Trazer colunas devolveria.
    pub outro_arquivo: Option<PathBuf>,
    /// Chamado na thread da conta do rodapé da seleção antes de ler o arquivo.
    pub somar: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// A tabela aberta pela tarefa de abertura.
struct Aberta {
    tabela: Tabela,
    retrato: Retrato,
    pagina: Pagina,
}

enum Fase {
    /// Ainda não começou (a primeira chamada com o `egui::Context` abre).
    Nova,
    Abrindo {
        promessa: Promessa<Aberta>,
        controle: Arc<Controle>,
        desde: f64,
    },
    Aberta,
    Falhou(String),
}

/// A tarefa da fila que está rodando.
pub(crate) struct Rodando {
    pub trabalho: Trabalho,
    promessa: Promessa<Resposta>,
    /// O número da tarefa (as células gravando esperam a resposta dela).
    pub seq: u64,
    pub controle: Arc<Controle>,
    pub desde: f64,
}

/// O cálculo das estatísticas de uma coluna (a seção Coluna), guardado por coluna e versão. Trocado (outra coluna,
/// outra versão) ou largado, ele é cancelado: as contas de uma coluna longa não se acumulam a cada clique.
struct CalculoColuna {
    coluna: usize,
    versao: u64,
    promessa: Promessa<Estatistica>,
    controle: Arc<Controle>,
}

impl Drop for CalculoColuna {
    /// A conta para na próxima vez que avisar o progresso.
    fn drop(&mut self) {
        self.controle.cancelar();
    }
}

/// O editor tabular de CSV (prancheta 7). Veja o topo do crate.
pub struct TelaCsv {
    pub(crate) caminho: PathBuf,
    pub(crate) nome: String,
    /// O que vai nos ids do egui (o caminho).
    pub(crate) id: String,
    /// As escolhas do Formato (o que fica `None` é detectado).
    pub(crate) opcoes: OpcoesAbertura,
    fase: Fase,
    pub(crate) tabela: Option<TabelaCompartilhada>,
    pub(crate) retrato: Option<Retrato>,
    pub(crate) pagina: Option<Pagina>,
    pub(crate) grade: Option<GradeCsv>,
    pub(crate) fila: VecDeque<(u64, Trabalho)>,
    pub(crate) rodando: Option<Rodando>,
    /// O número da última tarefa posta na fila (as respostas chegam na ordem).
    pub(crate) seq: u64,
    /// As linhas marcadas (posições na tabela).
    pub(crate) marcadas: BTreeSet<u64>,
    /// Os rótulos do desfazer e do refazer, em paralelo com as pilhas do núcleo.
    pub(crate) rotulos_desfazer: Vec<String>,
    pub(crate) rotulos_refazer: Vec<String>,
    /// A edição no lugar aberta e as células gravando.
    pub(crate) edicao: Option<Edicao>,
    pub(crate) pendentes: Vec<Pendente>,
    /// O erro da última gravação, até a casca tomá-lo; e se ela falhou (o status "Com erro").
    pub(crate) erro_de_gravacao: Option<String>,
    pub(crate) gravacao_falhou: bool,
    /// O foco volta à grade no próximo quadro (depois de uma edição com Enter, de uma linha nova).
    pub(crate) focar_grade: bool,
    /// As colunas da grade neste quadro (com a provisória).
    pub(crate) mapa: Vec<ColunaDaGrade>,
    /// As colunas selecionadas (da tabela) e a âncora do Shift.
    pub(crate) colunas_selecionadas: BTreeSet<usize>,
    pub(crate) ancora_coluna: Option<usize>,
    /// O cabeçalho em edição (a Nova coluna, a coluna colada, o Renomear) e se o nome dele começa selecionado.
    pub(crate) cabecalho_edicao: Option<CabecalhoEmEdicao>,
    pub(crate) selecionar_nome_do_cabecalho: bool,
    /// A coluna do menu do cabeçalho, e a que o Shift+F10 pediu (o foco vai ao `th` dela antes).
    pub(crate) menu_coluna: Option<usize>,
    pub(crate) menu_teclado: Option<usize>,
    /// A cola pedida pelo menu (o texto chega no `Event::Paste` seguinte), o aviso dela e a leitura da cola por cima.
    pub(crate) cola_pedida: Option<PedidoCola>,
    pub(crate) aviso_cola: Option<AvisoCola>,
    pub(crate) leitura_sobre: Option<LeituraSobre>,
    /// A cópia de colunas em andamento: o TSV e o toast.
    pub(crate) copia: Option<Promessa<(String, String)>>,
    /// A janela Trazer colunas, o pedido de abri-la (com o texto da área de transferência, quando veio) e as linhas sem
    /// par do último Trazer (o "Marcar as linhas sem par").
    pub(crate) trazer: Option<JanelaTrazer>,
    pub(crate) trazer_pedido: Option<Option<String>>,
    /// O pedido de abrir a janela com uma origem já escolhida (o rótulo dela: o "Trazer colunas desta planilha" de uma
    /// aba de planilha, que a casca passa a este CSV); vale quando a tabela estiver lida.
    pub(crate) trazer_de: Option<String>,
    pub(crate) sem_par: Vec<u64>,
    /// A fonte deste CSV para o Trazer colunas de outras abas (a mesma `Arc` sempre) e a versão do resumo dela.
    pub(crate) fonte: Option<Arc<FonteCsv>>,
    pub(crate) versao_da_fonte: Option<u64>,
    /// O filtro e a ordem (a faixa e a seta do cabeçalho), a condição no popover e a vista em cálculo.
    pub(crate) filtros: Vec<Condicao>,
    pub(crate) ordem: Option<Ordem>,
    pub(crate) proximo_filtro: u64,
    pub(crate) condicao: Option<EdicaoCondicao>,
    pub(crate) abrir_condicao_pendente: bool,
    pub(crate) calculo_vista: Option<CalculoVista>,
    /// O que pediu a vista que está na janela.
    pub(crate) vista_de: Option<ChaveVista>,
    /// A janela que a grade quer à vista agora (a próxima tarefa lê a página dela).
    pub(crate) janela: Janela,
    /// A página já pedida e ainda não chegada (`inicio`, para não pedir de novo a cada quadro).
    pagina_pedida: Option<u64>,
    /// As linhas da vista à vista no último quadro (a primeira e a última, de 0): o "Linhas a–b de n" do rodapé.
    visiveis: Option<(u64, u64)>,
    /// Muda a cada resposta que pode ter mudado o conteúdo (as estatísticas guardadas, a vista do filtro, o resumo da
    /// fonte e o casamento da janela Trazer valem por versão): uma alteração que mudou alguma coisa, ler de novo, uma
    /// gravação. Ler uma página, conferir o disco e copiar linhas não mudam.
    pub(crate) versao: u64,
    estatisticas: HashMap<usize, (u64, Estatistica)>,
    calculo: Option<CalculoColuna>,
    /// A conta que falhou (coluna e versão): não é pedida de novo a cada quadro.
    calculo_falhou: Option<(usize, u64)>,
    /// As contas do rodapé da seleção (prontas, ou a tarefa que as faz).
    pub(crate) soma: Option<crate::selecao::SomaSelecao>,
    /// O arquivo mudou por fora (a volta do foco conferiu): o aviso de topo.
    pub(crate) mudou_no_disco: bool,
    /// A janela tinha o foco no quadro anterior.
    com_foco: bool,
    /// A pergunta do Atualizar com alterações.
    confirmar_descartar: Option<Confirmacao>,
    /// O Cancelar do card de carregamento foi clicado: a aba fecha no fim do quadro.
    cancelar_abertura: bool,
    /// A busca da barra (Ctrl+F) no arquivo inteiro.
    pub(crate) busca: crate::busca::BuscaCsv,
    #[doc(hidden)]
    pub ganchos: Ganchos,
}

impl TelaCsv {
    /// O documento de `caminho`; o arquivo é lido numa tarefa no primeiro quadro.
    pub fn nova(caminho: PathBuf) -> TelaCsv {
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| caminho.display().to_string());
        let id = caminho.display().to_string();
        TelaCsv {
            caminho,
            nome,
            id,
            opcoes: OpcoesAbertura::default(),
            fase: Fase::Nova,
            tabela: None,
            retrato: None,
            pagina: None,
            grade: None,
            fila: VecDeque::new(),
            rodando: None,
            seq: 0,
            marcadas: BTreeSet::new(),
            rotulos_desfazer: Vec::new(),
            rotulos_refazer: Vec::new(),
            edicao: None,
            pendentes: Vec::new(),
            erro_de_gravacao: None,
            gravacao_falhou: false,
            focar_grade: false,
            mapa: Vec::new(),
            colunas_selecionadas: BTreeSet::new(),
            ancora_coluna: None,
            cabecalho_edicao: None,
            selecionar_nome_do_cabecalho: false,
            menu_coluna: None,
            menu_teclado: None,
            cola_pedida: None,
            aviso_cola: None,
            leitura_sobre: None,
            copia: None,
            trazer: None,
            trazer_pedido: None,
            trazer_de: None,
            sem_par: Vec::new(),
            fonte: None,
            versao_da_fonte: None,
            filtros: Vec::new(),
            ordem: None,
            proximo_filtro: 0,
            condicao: None,
            abrir_condicao_pendente: false,
            calculo_vista: None,
            vista_de: None,
            visiveis: None,
            janela: Janela::inicial(),
            pagina_pedida: None,
            versao: 0,
            estatisticas: HashMap::new(),
            calculo: None,
            calculo_falhou: None,
            soma: None,
            mudou_no_disco: false,
            com_foco: true,
            confirmar_descartar: None,
            cancelar_abertura: false,
            busca: Default::default(),
            ganchos: Ganchos::default(),
        }
    }

    /// Com os ganchos dos testes.
    #[doc(hidden)]
    pub fn com_ganchos(mut self, ganchos: Ganchos) -> TelaCsv {
        self.ganchos = ganchos;
        self
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// O nome do arquivo ("vendas_2025.csv").
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// O arquivo já foi lido (a grade está à vista).
    pub fn aberta(&self) -> bool {
        matches!(self.fase, Fase::Aberta)
    }

    /// A frase do erro que impediu de abrir o arquivo ("O arquivo está em UTF-16…", "O arquivo não existe…"): a
    /// casca troca a tela pela ficha com ela.
    pub fn erro_ao_abrir(&self) -> Option<&str> {
        match &self.fase {
            Fase::Falhou(e) => Some(e),
            _ => None,
        }
    }

    /// O carregamento está em andamento.
    pub fn carregando(&self) -> bool {
        matches!(self.fase, Fase::Nova | Fase::Abrindo { .. })
    }

    /// Quando a leitura começou (o relógio do egui), enquanto ela anda.
    #[doc(hidden)]
    pub fn abrindo_desde(&self) -> Option<f64> {
        match &self.fase {
            Fase::Abrindo { desde, .. } => Some(*desde),
            _ => None,
        }
    }

    /// Uma pergunta da tela espera a resposta da pessoa (o "Descartar as alterações… e ler o arquivo de novo?" do F5).
    /// O Salvar do CSV não pergunta nada: com o arquivo mudado por fora, ele recusa com o erro de gravação.
    pub fn pergunta_aberta(&self) -> bool {
        self.confirmar_descartar.is_some()
    }

    /// O que a tabela é agora (a cópia que a tela desenha).
    pub fn retrato(&self) -> Option<&Retrato> {
        self.retrato.as_ref()
    }

    /// As linhas que a grade tem agora em mãos.
    pub fn pagina(&self) -> Option<&Pagina> {
        self.pagina.as_ref()
    }

    /// A tabela compartilhada (os testes travam o `Mutex` para provar que a grade não espera por ele).
    #[doc(hidden)]
    pub fn tabela_compartilhada(&self) -> Option<Arc<Mutex<Tabela>>> {
        self.tabela.clone()
    }

    /// A versão do conteúdo (os testes conferem que ler páginas e conferir o disco não a andam).
    #[doc(hidden)]
    pub fn versao(&self) -> u64 {
        self.versao
    }

    /// O centro de uma célula (linha da vista, coluna de dados) à vista, para os testes clicarem.
    #[doc(hidden)]
    pub fn centro_celula(&self, linha: usize, coluna: usize) -> Option<Pos2> {
        self.grade.as_ref()?.centro_celula(linha, coluna)
    }

    #[doc(hidden)]
    pub fn centro_marca(&self, linha: usize) -> Option<Pos2> {
        self.grade.as_ref()?.centro_marca(linha)
    }

    #[doc(hidden)]
    pub fn centro_cabecalho(&self, coluna: usize) -> Option<Pos2> {
        self.grade.as_ref()?.centro_cabecalho(coluna)
    }

    /// A célula ativa (linha da vista, coluna de dados).
    pub fn celula_ativa(&self) -> Option<(usize, usize)> {
        let g = self.grade.as_ref()?;
        let (l, c) = g.nucleo.selecao()?;
        let d = g.dado(c)?;
        match self.mapa.get(d) {
            Some(ColunaDaGrade::Dados(c)) => Some((l, *c)),
            Some(ColunaDaGrade::Provisoria) => None,
            None => Some((l, d)),
        }
    }

    /// Escolhe a célula ativa (linha da vista, coluna de dados).
    pub fn selecionar(&mut self, celula: Option<(usize, usize)>) {
        if let Some(g) = self.grade.as_mut() {
            g.nucleo.selecionar(celula.map(|(l, c)| (l, c + crate::grade::PRIMEIRA_DE_DADOS)));
        }
    }

    /// O id do corpo da grade (o foco do teclado).
    #[doc(hidden)]
    pub fn id_grade(&self) -> Option<egui::Id> {
        self.grade.as_ref().map(|g| g.nucleo.id_corpo())
    }

    /// Há tarefa da tabela rodando ou na fila.
    pub(crate) fn trabalhando(&self) -> bool {
        self.rodando.is_some() || !self.fila.is_empty()
    }

    /// Os trabalhos rodando e na fila, nessa ordem.
    fn trabalhos(&self) -> impl Iterator<Item = &Trabalho> {
        self.rodando.as_ref().map(|r| &r.trabalho).into_iter().chain(self.fila.iter().map(|(_, t)| t))
    }

    /// Uma alteração (uma `Mutacao`) está na fila ou rodando.
    pub(crate) fn mutacao_pendente(&self) -> bool {
        self.trabalhos().any(|t| matches!(t, Trabalho::Mutacao(_)))
    }

    /// Um trabalho que mexe nas posições das linhas ou das colunas (inserir ou tirar no meio, desfazer, ler de novo)
    /// está na fila ou rodando: a página e as marcas em mãos são de antes dele, e uma alteração pedida agora por posição
    /// cairia no lugar errado depois dele (o Delete repetido excluiria a linha de baixo). A edição espera a resposta.
    pub(crate) fn posicoes_pendentes(&self) -> bool {
        self.trabalhos().any(|t| match t {
            Trabalho::Mutacao(m) => m.muda_posicoes(),
            Trabalho::Reabrir(_) | Trabalho::Cabecalho(_) | Trabalho::Recarregar(_) => true,
            Trabalho::Pagina
            | Trabalho::ConferirDisco
            | Trabalho::Salvar
            | Trabalho::LerLinhas(_)
            | Trabalho::LerIntervalo { .. } => false,
        })
    }

    /// Há alteração que o retrato ainda não conta: a célula em edição com outro texto, o cabeçalho em edição que vai
    /// mudar a tabela (a Nova coluna, a cola, o Renomear com outro nome) ou uma alteração na fila ou rodando.
    pub(crate) fn alteracao_a_caminho(&self) -> bool {
        let celula = self.edicao.as_ref().is_some_and(|e| e.texto != e.original);
        let cabecalho = self.cabecalho_edicao.as_ref().is_some_and(|e| match e.alvo {
            crate::colunas::AlvoCabecalho::Renomear { .. } => !e.nome.trim().is_empty() && e.nome != e.sugerido,
            _ => true,
        });
        celula || cabecalho || self.mutacao_pendente()
    }

    // ------------------------------------------------------------------ tarefas

    fn iniciar_abertura(&mut self, ectx: &egui::Context) {
        let caminho = self.caminho.clone();
        let opcoes = self.opcoes.clone();
        let controle = Controle::novo();
        let c2 = Arc::clone(&controle);
        let gancho = self.ganchos.abrir.clone();
        let promessa = Promessa::rodar(ectx, move || {
            if let Some(g) = gancho {
                g();
            }
            let mut tabela = Tabela::abrir_com_progresso(&caminho, &opcoes, &mut |f, t| c2.avisar(f, t))?;
            let retrato = sessao::retrato(&mut tabela)?;
            let pagina = sessao::ler_pagina(&mut tabela, &Janela::inicial())?;
            Ok(Aberta { tabela, retrato, pagina })
        });
        self.fase = Fase::Abrindo { promessa, controle, desde: ectx.input(|i| i.time) };
    }

    /// Põe um trabalho na fila (uma página pedida substitui a que ainda esperava).
    pub(crate) fn enfileirar(&mut self, trabalho: Trabalho) -> u64 {
        if matches!(trabalho, Trabalho::Pagina) {
            self.fila.retain(|(_, t)| !matches!(t, Trabalho::Pagina));
        }
        self.seq += 1;
        self.fila.push_back((self.seq, trabalho));
        self.seq
    }

    fn iniciar_proximo(&mut self, ectx: &egui::Context) {
        if self.rodando.is_some() {
            return;
        }
        let Some(tabela) = self.tabela.clone() else { return };
        let Some((seq, trabalho)) = self.fila.pop_front() else { return };
        let controle = Controle::novo();
        let c2 = Arc::clone(&controle);
        let janela = self.janela.clone();
        let t2 = trabalho.clone();
        let gancho = self.ganchos.trabalho.clone();
        if matches!(trabalho, Trabalho::Pagina) {
            self.pagina_pedida = Some(janela.inicio);
        }
        let promessa = Promessa::rodar(ectx, move || {
            if let Some(g) = gancho {
                g();
            }
            sessao::executar(&tabela, t2, &janela, &c2)
        });
        let desde = ectx.input(|i| i.time);
        self.rodando = Some(Rodando { trabalho, promessa, seq, controle, desde });
    }

    /// Toma o que as tarefas entregaram e começa a próxima da fila (a cada quadro, à vista ou não).
    pub(crate) fn receber(&mut self, ectx: &egui::Context, ctx: &mut Contexto) {
        if matches!(self.fase, Fase::Nova) {
            self.iniciar_abertura(ectx);
        }
        if let Fase::Abrindo { promessa, .. } = &mut self.fase
            && let Some(r) = promessa.tomar()
        {
            match r {
                Ok(a) => {
                    let n = a.retrato.colunas();
                    self.grade = Some(GradeCsv::nova(&self.id, n));
                    self.tabela = Some(Arc::new(Mutex::new(a.tabela)));
                    self.retrato = Some(a.retrato);
                    self.pagina = Some(a.pagina);
                    self.fase = Fase::Aberta;
                    self.versao += 1;
                }
                Err(e) if e.tem_codigo(Codigo::Cancelado) => {
                    self.fase = Fase::Falhou(e.mensagem.clone());
                }
                Err(e) => {
                    let _ = ctx.reagir(&e);
                    self.fase = Fase::Falhou(e.mensagem.clone());
                }
            }
        }
        if let Some(r) = self.rodando.as_mut().and_then(|rod| rod.promessa.tomar())
            && let Some(rod) = self.rodando.take()
        {
            self.resposta(ectx, ctx, rod.trabalho, rod.seq, r);
        }
        self.receber_calculo();
        self.receber_copia(ectx);
        self.receber_leitura_sobre();
        self.acao_do_toast(ectx);
        self.receber_vista();
        if self.aberta() {
            self.pedir_vista(ectx);
        }
        self.passo_da_busca(ectx);
        self.fonte_deste();
        self.iniciar_proximo(ectx);
    }

    fn resposta(
        &mut self,
        ectx: &egui::Context,
        ctx: &mut Contexto,
        trabalho: Trabalho,
        seq: u64,
        r: Resultado<Resposta>,
    ) {
        if matches!(trabalho, Trabalho::Pagina) {
            self.pagina_pedida = None;
        }
        // as células gravando até esta tarefa já têm o texto na página nova (ou a gravação falhou)
        self.pendentes.retain(|p| p.seq > seq);
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                let mostrar = ctx.reagir(&e) == Reacao::Mostrar;
                self.tarefa_falhou(ectx, &trabalho, &e, mostrar);
                return;
            }
        };
        if let Efeito::Disco(mudou) = r.efeito {
            self.mudou_no_disco = mudou;
        }
        if let Some(retrato) = r.retrato {
            let colunas_antes = self.retrato.as_ref().map(Retrato::colunas);
            let estrutura_mudou = colunas_antes != Some(retrato.colunas())
                || self.retrato.as_ref().is_some_and(|a| a.nomes != retrato.nomes || a.linhas != retrato.linhas);
            let reler = matches!(trabalho, Trabalho::Reabrir(_) | Trabalho::Cabecalho(_) | Trabalho::Recarregar(_));
            // só o que pode ter mudado o conteúdo anda a versão (a vista do filtro e as estatísticas se refazem com
            // ela): a volta do foco, a página e a cópia das linhas não
            let gravou = matches!(r.efeito, Efeito::Salvo(_)) && self.retrato.as_ref().is_some_and(|a| a.alterada);
            let mudou = matches!(r.efeito, Efeito::Alterou(true)) || reler || gravou || estrutura_mudou;
            if mudou {
                self.versao += 1;
            }
            if reler {
                self.mudou_no_disco = false;
            }
            if let Some(g) = self.grade.as_mut() {
                if colunas_antes != Some(retrato.colunas()) {
                    *g = GradeCsv::nova(&self.id, retrato.colunas());
                } else if estrutura_mudou || reler {
                    g.medir_de_novo();
                }
            }
            self.retrato = Some(retrato);
        }
        if let Some(pagina) = r.pagina {
            self.pagina = Some(pagina);
        }
        self.depois_da_tarefa(ectx, &trabalho, r.efeito);
    }

    fn receber_calculo(&mut self) {
        let Some(c) = self.calculo.as_mut() else { return };
        let Some(r) = c.promessa.tomar() else { return };
        let (coluna, versao) = (c.coluna, c.versao);
        self.calculo = None;
        match r {
            Ok(e) => {
                self.estatisticas.insert(coluna, (versao, e));
            }
            Err(e) if e.tem_codigo(Codigo::Cancelado) => {}
            Err(e) => {
                log::warn!("As contas da coluna {} falharam: {e}", coluna + 1);
                self.calculo_falhou = Some((coluna, versao));
            }
        }
    }

    /// A estatística da coluna `c` na versão de agora e se a conta dela está em curso; sem ela, pede a conta (uma
    /// coluna por vez: a de outra coluna ou de outra versão em curso é cancelada). A conta passa pela coluna em fluxo
    /// (as [`propriedades::Contas`] somam um valor por vez), sem juntá-la na memória.
    fn estatistica(&mut self, ectx: &egui::Context, c: usize) -> (Option<Estatistica>, bool) {
        if let Some((v, e)) = self.estatisticas.get(&c)
            && *v == self.versao
        {
            return (Some(e.clone()), false);
        }
        // a de uma versão anterior, enquanto a nova não chega
        let anterior = self.estatisticas.get(&c).map(|(_, e)| e.clone());
        if self.calculo.as_ref().is_some_and(|k| k.coluna == c && k.versao == self.versao) {
            return (anterior, true);
        }
        if self.calculo_falhou == Some((c, self.versao)) {
            return (anterior, false);
        }
        self.calculo = None;
        let pedido = (|| {
            let tabela = self.tabela.clone()?;
            let retrato = self.retrato.as_ref()?;
            Some((tabela, retrato.tipos.get(c).copied()?, retrato.formato.decimal))
        })();
        let Some((tabela, tipo, decimal)) = pedido else { return (anterior, false) };
        let controle = Controle::novo();
        let na_tarefa = Arc::clone(&controle);
        let promessa = Promessa::rodar(ectx, move || {
            let mut contas = propriedades::Contas::nova(tipo, decimal);
            sessao::instantaneo(&tabela)?.percorrer_coluna(
                c,
                &mut |feito, total| na_tarefa.avisar(feito, total),
                |valor| {
                    contas.somar(valor);
                    Ok(true)
                },
            )?;
            Ok(contas.estatistica())
        });
        self.calculo = Some(CalculoColuna { coluna: c, versao: self.versao, promessa, controle });
        (anterior, true)
    }

    /// A volta do foco da janela confere se o arquivo mudou por fora.
    fn conferir_foco(&mut self, ectx: &egui::Context) {
        let foco = ectx.input(|i| i.focused);
        if foco && !self.com_foco && self.aberta() {
            self.enfileirar(Trabalho::ConferirDisco);
        }
        self.com_foco = foco;
    }

    /// O Atualizar (F5): sem alterações, lê de novo; com elas, pergunta antes de descartar.
    pub(crate) fn atualizar(&mut self) {
        if self.retrato.as_ref().is_some_and(|r| r.alterada) {
            self.confirmar_descartar = Some(
                Confirmacao::new(format!("Descartar as alterações de {} e ler o arquivo de novo?", self.nome))
                    .rotulo("Descartar")
                    .perigo(true),
            );
        } else {
            self.enfileirar(Trabalho::Recarregar(self.opcoes.clone()));
        }
    }

    fn aplicar_propriedades(&mut self, pedido: PedidoPropriedades) {
        match pedido {
            PedidoPropriedades::Reabrir(opcoes) => {
                self.opcoes = opcoes.clone();
                self.enfileirar(Trabalho::Reabrir(opcoes));
            }
            PedidoPropriedades::Cabecalho(sim) => {
                self.opcoes.cabecalho = Some(sim);
                self.enfileirar(Trabalho::Cabecalho(sim));
            }
        }
    }

    // ------------------------------------------------------------------ desenho

    /// O total de linhas que a grade mostra.
    pub(crate) fn linhas_da_vista(&self) -> u64 {
        match &self.janela.vista {
            Some(v) => v.len() as u64,
            None => self.retrato.as_ref().map_or(0, |r| r.linhas),
        }
    }

    fn desenhar_carregando(&mut self, ui: &mut Ui, agora: f64) {
        let Fase::Abrindo { controle, desde, .. } = &self.fase else { return };
        if agora - desde < ESPERA_DO_CARD {
            ui.ctx().request_repaint_after(Duration::from_secs_f64(ESPERA_DO_CARD - (agora - desde) + 0.01));
            return;
        }
        let controle = Arc::clone(controle);
        let fracao = controle.fracao();
        let mut cancelar = false;
        let nome = self.nome.clone();
        ui.add_space(RECUO_AVISO);
        Aviso::com_tom(TomAviso::Informacao).cheia(true).mostrar(ui, |ui, cores| {
            let rotulo = match fracao {
                Some(f) => format!("{} {}%", textos::lendo(&nome), (f * 100.0).round() as u32),
                None => textos::lendo(&nome),
            };
            texto_simples(ui, &rotulo, 12.0, cores.texto);
            ui.allocate_ui(vec2(LARGURA_BARRA.min(ui.available_width()), 8.0), |ui| {
                barra_progresso(ui, fracao);
            });
            if ui.add(Botao::new("Cancelar").pequeno()).clicked() {
                cancelar = true;
            }
        });
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        if cancelar {
            controle.cancelar();
            self.cancelar_abertura = true;
        }
    }

    fn desenhar_grade(&mut self, ui: &mut Ui) {
        let ectx = ui.ctx().clone();
        let editavel = self.livre_para_editar();
        self.mapa = self.montar_mapa();
        let n_dados = self.mapa.len();
        if let Some(g) = self.grade.as_mut()
            && g.n_dados() != n_dados
        {
            g.refazer(&self.id, n_dados);
        }
        if std::mem::take(&mut self.selecionar_nome_do_cabecalho)
            && let Some(ed) = &self.cabecalho_edicao
        {
            let n = ed.nome.chars().count();
            let mut estado = egui::text_edit::TextEditState::load(&ectx, ed.id()).unwrap_or_default();
            estado.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n),
            )));
            estado.store(&ectx, ed.id());
        }
        if let Some(id) = self.grade.as_ref().map(|g| g.nucleo.id_corpo()) {
            self.teclas_das_colunas(&ectx, id);
        }
        let provisoria: Option<Arc<Vec<String>>> = match self.cabecalho_edicao.as_ref().map(|e| &e.alvo) {
            Some(crate::colunas::AlvoCabecalho::Colar { valores, .. }) => Some(Arc::clone(valores)),
            _ => None,
        };
        let linhas_da_vista = self.linhas_da_vista();
        let ordem = self.ordem.as_ref().and_then(|o| {
            let c = self.retrato.as_ref()?.nomes.iter().position(|n| *n == o.coluna)?;
            Some((c, o.decrescente))
        });
        let (Some(retrato), Some(grade)) = (self.retrato.as_ref(), self.grade.as_mut()) else { return };
        let p = paleta(ui);
        grade.ajustar_larguras(ui, p, retrato, self.pagina.as_ref());
        let linhas = usize::try_from(linhas_da_vista).unwrap_or(usize::MAX);
        if grade.nucleo.n_linhas() != linhas {
            grade.nucleo.definir_linhas(linhas);
        }
        if std::mem::take(&mut self.focar_grade) {
            grade.nucleo.focar(ui);
        }
        let altura_max = ui.available_height().max(ALTURA_GRADE_MIN);
        let larguras = grade.nucleo.larguras().to_vec();
        let n_marcadas = self.marcadas.len() as u64;
        let mut pintor = Pintor {
            p,
            retrato,
            pagina: self.pagina.as_ref(),
            n_dados: grade.n_dados(),
            larguras: &larguras,
            altura_cabecalho: grade.nucleo.altura_do_cabecalho(),
            marcadas: &self.marcadas,
            todas_marcadas: linhas_da_vista > 0 && n_marcadas >= linhas_da_vista,
            algumas_marcadas: n_marcadas > 0,
            editavel,
            edicao: self.edicao.as_mut(),
            pendentes: &self.pendentes,
            ponta_ida: grade.nucleo.na_ponta(false),
            ponta_volta: grade.nucleo.na_ponta(true),
            mapa: &self.mapa,
            provisoria: provisoria.as_deref().map(Vec::as_slice),
            selecionadas: &self.colunas_selecionadas,
            cabecalho_edicao: self.cabecalho_edicao.as_mut(),
            id_doc: &self.id,
            ordem,
            ordenar: None,
            clique_th: None,
            menu_th: None,
            ths: Vec::new(),
            fim_cabecalho: None,
            cabecalho_desenhado: false,
            visiveis: None,
            nova_no_fim: false,
            marcar: None,
            marcar_todas: None,
            fim_edicao: None,
            edicao_desenhada: false,
            origem: None,
            visivel: None,
        };
        let resposta = grade.nucleo.mostrar(ui, altura_max, &mut pintor);
        grade.origem = pintor.origem;
        grade.visivel = pintor.visivel;
        let id_corpo = grade.nucleo.id_corpo();
        let ths = std::mem::take(&mut pintor.ths);
        let saida = crate::verbos::SaidaDaGrade {
            visiveis: pintor.visiveis,
            nova_no_fim: pintor.nova_no_fim,
            marcar: pintor.marcar,
            marcar_todas: pintor.marcar_todas,
            fim_edicao: pintor.fim_edicao,
            edicao_desenhada: pintor.edicao_desenhada,
            clique_th: pintor.clique_th,
            menu_th: pintor.menu_th,
            fim_cabecalho: pintor.fim_cabecalho,
            cabecalho_desenhado: pintor.cabecalho_desenhado,
            ordenar: pintor.ordenar,
        };
        self.visiveis = saida.visiveis;
        self.pedir_pagina(saida.visiveis);
        let mut focos: Vec<egui::Id> = ths.iter().map(|(_, r)| r.id).collect();
        focos.push(id_corpo);
        self.tomar_colas(&ectx, &focos);
        let menu_th = saida.menu_th;
        self.aplicar_grade(ui, resposta, saida);
        self.mostrar_menu(ui, &ths, menu_th);
        self.menu_da_celula(ui);
    }

    /// Pede a página que cobre as linhas à vista, se a que está em mãos (ou a já pedida) não cobre.
    fn pedir_pagina(&mut self, visiveis: Option<(u64, u64)>) {
        let Some((a, b)) = visiveis else { return };
        let coberta = self.pagina.as_ref().is_some_and(|p| p.contem(a) && p.contem(b))
            || self.pagina.as_ref().is_some_and(|p| p.linhas.is_empty() && a >= self.linhas_da_vista());
        if coberta {
            return;
        }
        let inicio = a.saturating_sub(FOLGA_ACIMA);
        let pedida = self.pagina_pedida.is_some_and(|i| a >= i && b < i + LINHAS_DA_PAGINA as u64)
            || self.fila.iter().any(|(_, t)| matches!(t, Trabalho::Pagina))
                && a >= self.janela.inicio
                && b < self.janela.inicio + LINHAS_DA_PAGINA as u64;
        if pedida {
            return;
        }
        self.janela.inicio = inicio;
        self.janela.quantidade = LINHAS_DA_PAGINA;
        self.enfileirar(Trabalho::Pagina);
    }

    fn desenhar_conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let ectx = ui.ctx().clone();
        let agora = ectx.input(|i| i.time);
        self.receber(&ectx, ctx);
        self.conferir_foco(&ectx);
        let mut area = ui.available_rect_before_wrap();
        if !area.max.y.is_finite() {
            area.max.y = area.min.y + ALTURA_GRADE_MIN;
        }
        let mut dentro = ui.new_child(
            UiBuilder::new()
                .id_salt(("tela_csv::conteudo", &self.id))
                .max_rect(area)
                .layout(Layout::top_down(Align::Min)),
        );
        dentro.set_clip_rect(area.intersect(ui.clip_rect()));
        dentro.spacing_mut().item_spacing.y = 0.0;
        match &self.fase {
            Fase::Nova | Fase::Abrindo { .. } => self.desenhar_carregando(&mut dentro, agora),
            Fase::Falhou(e) => {
                let e = e.clone();
                dentro.add_space(RECUO_AVISO);
                aviso_texto(&mut dentro, &e, true);
            }
            Fase::Aberta => {
                if self.retrato.is_none() {
                    carregando(&mut dentro, "Lendo as linhas…");
                } else {
                    self.desenhar_filtros(&mut dentro);
                    self.desenhar_aviso_cola(&mut dentro);
                    self.desenhar_grade(&mut dentro);
                }
            }
        }
        ui.allocate_rect(area, Sense::hover());
        if std::mem::take(&mut self.cancelar_abertura) {
            ctx.emitir(Acao::FecharAba(None));
        }
        if confirmar(&ectx, &mut self.confirmar_descartar) == Some(true) {
            self.enfileirar(Trabalho::Recarregar(self.opcoes.clone()));
        }
        if let Some(texto) = self.trazer_pedido.take() {
            self.abrir_trazer(&ectx, ctx, texto.as_deref(), None);
        }
        // o pedido espera a tabela lida e livre (sem alteração de linhas a caminho)
        if self.trazer_de.is_some()
            && self.retrato.is_some()
            && self.livre_para_editar()
            && let Some(rotulo) = self.trazer_de.take()
        {
            self.abrir_trazer(&ectx, ctx, None, Some(&rotulo));
        }
        self.desenhar_trazer(&ectx, ctx);
        self.iniciar_proximo(&ectx);
    }

    /// O rodapé (o dos Dados, sem as páginas: a grade rola todas as linhas): "Linhas 1–24 de 1.204" (as à vista, de
    /// quantas a vista tem) e, em `--acct`, "· 1 alteração não salva"; em `--t3`, as marcadas e as colunas
    /// selecionadas.
    fn desenhar_rodape(&mut self, ui: &mut Ui) {
        let Some(r) = &self.retrato else { return };
        let p = paleta(ui);
        let total = self.linhas_da_vista();
        let (a, b) = self.visiveis.filter(|_| total > 0).map_or((0, 0), |(a, b)| (a + 1, (b + 1).min(total)));
        texto_simples(ui, &contagem_linhas(a, b, total), 12.0, p.estudio.t2);
        if r.alterada {
            texto_simples(ui, &textos::alteracoes_nao_salvas(r.alteracoes.max(1)), 12.0, p.estudio.acct);
        }
        if !self.marcadas.is_empty() {
            texto_simples(ui, &textos::marcadas(self.marcadas.len()), 12.0, p.estudio.t3);
        }
        let selecionadas: Vec<&str> =
            self.colunas_selecionadas.iter().filter_map(|&c| r.nomes.get(c).map(String::as_str)).collect();
        if !selecionadas.is_empty() {
            texto_simples(ui, &textos::colunas_selecionadas(&selecionadas), 12.0, p.estudio.t3);
        }
        self.rodape_da_selecao(ui);
    }

    fn desenhar_propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let ectx = ui.ctx().clone();
        let ocupada = self.trabalhando();
        // o retrato emprestado (nada de cópia a cada quadro): o pedido do Formato se aplica depois do empréstimo
        let Some(retrato) = self.retrato.as_ref() else { return };
        let pedido = propriedades::secao_formato(ui, &self.id, retrato, &self.opcoes, ocupada);
        propriedades::secao_do_arquivo(ui, ctx, retrato);
        if let Some(pedido) = pedido {
            self.aplicar_propriedades(pedido);
        }
        let coluna = match self.colunas_selecionadas.len() {
            1 => self.colunas_selecionadas.iter().next().copied(),
            _ => self.celula_ativa().map(|(_, c)| c),
        };
        let da_coluna = coluna.and_then(|c| {
            let r = self.retrato.as_ref()?;
            Some((c, r.nomes.get(c)?.clone(), *r.tipos.get(c)?))
        });
        if let Some((c, nome, tipo)) = da_coluna {
            let (estatistica, calculando) = self.estatistica(&ectx, c);
            let livre = self.livre_para_editar();
            let renomear = self.renomear_permitido();
            let mut acao = None;
            propriedades::secao_coluna(ui, &self.id, &nome, tipo, estatistica.as_ref(), calculando, |ui| {
                acao = propriedades::acoes_da_coluna(ui, livre, renomear);
            });
            match acao {
                Some(propriedades::AcaoColuna::Trazer) => self.trazer_pedido = Some(None),
                Some(propriedades::AcaoColuna::Copiar) => self.copiar_colunas(&ectx, vec![c]),
                Some(propriedades::AcaoColuna::Renomear) => self.renomear(c),
                Some(propriedades::AcaoColuna::Excluir) => self.excluir_colunas(vec![c]),
                None => {}
            }
        } else {
            // a coluna saiu das Propriedades: a conta dela para
            self.calculo = None;
        }
    }

    fn desenhar_avisos(&mut self, doc: &mut interface::pecas::documento::Documento, ui: &mut Ui) {
        self.aviso_da_gravacao(doc, ui);
        if !self.mudou_no_disco {
            return;
        }
        let (mut ler, mut ignorar) = (false, false);
        doc.aviso(ui, TomAviso::Atencao, |ui, cores| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                texto_simples(ui, textos::MUDOU_NO_DISCO, 12.0, cores.texto);
                if ui.add(Botao::new("Ler de novo").pequeno()).clicked() {
                    ler = true;
                }
                if ui.add(Botao::ghost("Ignorar").pequeno()).clicked() {
                    ignorar = true;
                }
            });
        });
        if ler {
            self.mudou_no_disco = false;
            self.atualizar();
        } else if ignorar {
            self.mudou_no_disco = false;
        }
    }
}

impl Documento for TelaCsv {
    fn anatomia(&self) -> Anatomia {
        Anatomia::default().com_faixa().com_rodape().rolagem_propria()
    }

    fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        self.desenhar_faixa(ui);
    }

    fn avisos(&mut self, doc: &mut interface::pecas::documento::Documento, ui: &mut Ui, _ctx: &mut Contexto) {
        self.desenhar_avisos(doc, ui);
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.desenhar_conteudo(ui, ctx);
    }

    fn rodape(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        self.desenhar_rodape(ui);
    }

    fn ofertas(&self, estado: &EstadoApp) -> Ofertas {
        self.ofertas_do_csv(estado)
    }

    fn executar(&mut self, verbo: Verbo, egui: &egui::Context, _ctx: &mut Contexto) {
        self.executar_verbo(verbo, egui);
        egui.request_repaint();
    }

    /// "Buscar em vendas.csv": no arquivo inteiro, nas linhas que o filtro mostra (o módulo `busca`).
    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        if !self.aberta() {
            return None;
        }
        let dica = self.dica_da_busca();
        Some(estado::BuscaDaBarra { texto: &mut self.busca.texto, dica: dica.into() })
    }

    fn evento_busca(&mut self, evento: estado::EventoBusca, egui: &egui::Context, _ctx: &mut Contexto) {
        self.evento_da_busca(evento, egui);
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.desenhar_propriedades(ui, ctx);
    }

    fn status(&self, _estado: &EstadoApp) -> Option<String> {
        Some(self.nome.clone())
    }

    fn ocupada(&self) -> bool {
        self.carregando()
            || self.trabalhando()
            || self.calculo.is_some()
            || self.copia.is_some()
            || self.leitura_sobre.is_some()
            || self.trazer.as_ref().is_some_and(JanelaTrazer::ocupada)
            || self.calculo_vista.is_some()
            || self.busca.ocupada()
    }

    fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        self.receber(egui, ctx);
    }

    /// Alterado também com o que ainda não chegou ao núcleo: a célula em edição com outro texto, o cabeçalho em edição
    /// que vai mudar alguma coisa e a alteração na fila ou rodando (a pergunta ao fechar não pode deixá-los para trás).
    fn alterado(&self) -> bool {
        self.retrato.as_ref().is_some_and(|r| r.alterada) || self.alteracao_a_caminho()
    }

    fn alteracoes(&self) -> Option<usize> {
        let n = self.retrato.as_ref().map(|r| r.alteracoes);
        if self.alteracao_a_caminho() { Some(n.unwrap_or(0).max(1)) } else { n }
    }

    fn salvando(&self) -> bool {
        self.salvando_agora()
    }

    fn fonte_de_colunas(&self) -> Option<Arc<dyn estado::FonteDeColunas>> {
        self.fonte.clone().map(|f| f as Arc<dyn estado::FonteDeColunas>)
    }

    fn tomar_erro_de_gravacao(&mut self) -> Option<String> {
        self.erro_de_gravacao.take()
    }

    fn status_do_arquivo(&self, _estado: &EstadoApp) -> Option<StatusDeArquivo> {
        let r = self.retrato.as_ref()?;
        let situacao = match (r.alterada, self.gravacao_falhou) {
            (true, true) => Situacao::ComErro,
            (true, false) => Situacao::Alterado,
            (false, _) => Situacao::Salvo,
        };
        let mut s = StatusDeArquivo::novo(self.nome.clone(), situacao);
        s.detalhes = textos::detalhes_do_status(&r.formato);
        Some(s)
    }
}
