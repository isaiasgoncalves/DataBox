//! O aplicativo: carrega o estado, mantém as abas de documento com o histórico único, desenha a
//! casca e o documento da aba ativa, abre as janelas globais e aplica as [`Acao`]s que as telas emitem.
//!
//! As telas são os [`Documento`]s das `estado::Abas`. Navegar abre a aba da rota ou foca a que já está aberta (sem
//! refazer o documento nem trocar o endereço dela); voltar e avançar só focam a aba da entrada do histórico. Cada
//! ação sai marcada com a aba que a emitiu (None: a casca, as janelas, os testes): o `Substituir` e o `Atualizar`
//! valem para a aba de origem, mesmo que ela já tenha saído de vista no mesmo quadro, e o `NotarTransacao` diz qual
//! Consulta abriu a transação. Só o documento da aba ativa é desenhado; as Consultas escondidas recebem em segundo
//! plano as respostas que chegam.
//!
//! A lógica do quadro fica em [`Editor::desenhar`], que não depende do eframe (os testes a chamam num
//! `egui_kittest::Harness`); o `eframe::App` só a chama com a janela.
//!
//! Aqui ficam o [`Editor`] (os campos, o construtor e as consultas dos testes) e os tipos que mais de um assunto usa;
//! o resto do `impl Editor` mora nos módulos por assunto: [`abas`] (navegar, fechar, trocar o endereço, reler),
//! [`abrir`] (a pasta, o banco e o arquivo de dados; a troca de banco), [`acoes`] (o despacho das ações, os itens de
//! menu, os verbos e os atalhos de teclado), [`fechar`] (a fila de perguntas ao fechar e as perguntas da janela),
//! [`paineis`] (as docas, as divisórias e as janelas próprias), [`pasta`] (a pasta aberta), [`preferencias`] (o tema,
//! a aparência e a janela Preferências), [`quadro`] (o ciclo da janela: um quadro inteiro e o `eframe::App`),
//! [`tarefas`] (as leituras e gravações em andamento) e [`toasts`] (os erros e a ação do toast).

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use comum::app::NOME;
use comum::{ErroBanco, Resultado};
use egui::{Pos2, Rect};
use estado::{Abas, Acao, Estado, IdAba, PedidoJanelaExportar, Promessa, Rota, TabelaAberta, Verbo};
use interface::pecas::janela::Confirmacao;
use interface::tema::Tema;
use prefs::PrefsInfo;
use servico::Servico;

use crate::casca::docas::{Lado, Lugar, Paineis, Painel};
use crate::casca::larguras::{Disposicao, Larguras};
use crate::casca::preferencias::JanelaPreferencias;
use crate::casca::sobre::{GanchoAbrir, Sobre};
use crate::casca::{self, Casca, titulo_da_janela};
use crate::documento::Documento;
use crate::linha_comando::Renderizador;
use crate::pedir_caminho::PedirCaminho;
use crate::recursos::{Dialogador, PedidoDialogo};
use crate::telas::{JanelaExportar, JanelaNovaTabela, JanelaNovoIndice};

mod abas;
mod abrir;
mod acoes;
mod fechar;
mod paineis;
mod pasta;
mod preferencias;
mod quadro;
mod tarefas;
mod toasts;

use abrir::{Troca, TrocaPendente};
use fechar::FilaDeSalvar;
use preferencias::FontePendente;
pub use preferencias::{aparencia_da_interface, aplicar_aparencia, tema_da_interface, tema_das_prefs};
use tarefas::{FimTarefa, Tarefa};

/// O `padding:8px` e o `gap:8px` do corpo (entre a doca da esquerda, o documento e a doca da direita), a margem dos lados da barra de ferramentas (`margin:6px 8px 0`) e a da barra de status
/// (`margin:0 8px 8px`).
const ESPACO: f32 = 8.0;

/// A releitura automática da pasta aberta (a volta do foco da janela, o fim de uma troca de banco ou de uma janela)
/// não acontece mais de uma vez neste intervalo, em segundos (decisão 6 da etapa 4: sem temporizador).
const INTERVALO_RELEITURA: f64 = 2.0;

/// O que abrir ao iniciar (a linha de comando): uma pasta, um banco ou um arquivo de dados, pelo
/// `Servico::alvo_do_caminho` (o `somente_leitura` vale para o banco).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Partida {
    pub caminho: Option<PathBuf>,
    pub somente_leitura: bool,
}

/// O caminho da linha de comando que não é banco: abre depois de o estado chegar.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DepoisDaPartida {
    Pasta(PathBuf),
    Arquivo(PathBuf),
}

/// O estado lido do serviço, com o aviso de um erro ao ler os objetos. Na partida, também
/// a pasta que estava aberta (`pasta_aberta` das preferências) e o caminho da linha de comando que não é banco.
struct Carga {
    estado: Estado,
    aviso: Option<String>,
    erro_partida: Option<String>,
    pasta_aberta: Option<PathBuf>,
    depois: Option<DepoisDaPartida>,
}

impl Carga {
    /// A carga de uma troca de banco ou de uma ressincronização (sem o que é só da partida).
    fn de(estado: Estado, aviso: Option<String>) -> Carga {
        Carga { estado, aviso, erro_partida: None, pasta_aberta: None, depois: None }
    }
}

/// Lê o estado inteiro: aplicação, banco, preferências, última pasta e os objetos do banco.
fn carregar_estado(servico: &Servico) -> Resultado<(Estado, Option<String>)> {
    let e = servico.estado()?;
    let mut aviso = None;
    let objetos = match &e.banco {
        Some(b) => match servico.objetos(&b.id) {
            Ok(o) => Some(o),
            Err(erro) => {
                aviso = Some(erro.mensagem);
                None
            }
        },
        None => None,
    };
    let ultimo_diretorio = servico.prefs().ultimo_diretorio().map(PathBuf::from);
    Ok((Estado { app: e.app, banco: e.banco, objetos, pasta: None, prefs: e.prefs, ultimo_diretorio }, aviso))
}

/// Quem pediu a troca (ou a abertura de uma pasta ou de um arquivo de dados), para onde vai o erro dela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origem {
    /// Menu, barra de ferramentas, Explorador, Ctrl+O, soltar na janela: o toast.
    Casca,
    /// O Início: o aviso de erro dele (o `.notice.perigo` do campo do caminho).
    Inicio,
    /// A janela "Pedir o caminho": o aviso dela, que continua aberta.
    Janela,
    /// A linha de comando (a pasta ou o arquivo de dados): o erro, com a frase da partida, vai ao Início.
    Partida,
}

/// O aviso do banco da linha de comando que não abriu (com as linhas em branco).
pub(crate) fn erro_da_partida(caminho: &std::path::Path, mensagem: &str) -> String {
    format!("Não foi possível abrir o banco {}.\n\n{mensagem}\n\nO {NOME} abre sem ele.", caminho.display())
}

/// O aviso da pasta da linha de comando que não abriu (as linhas em branco do [`erro_da_partida`]).
pub(crate) fn erro_da_partida_pasta(caminho: &std::path::Path, mensagem: &str) -> String {
    format!("Não foi possível abrir a pasta {}.\n\n{mensagem}\n\nO {NOME} abre sem ela.", caminho.display())
}

/// O aviso do arquivo de dados da linha de comando que não abriu.
pub(crate) fn erro_da_partida_arquivo(caminho: &std::path::Path, mensagem: &str) -> String {
    format!("Não foi possível abrir o arquivo {}.\n\n{mensagem}\n\nO {NOME} abre sem ele.", caminho.display())
}

/// Termina a edição de texto em curso (a célula da grade em edição vê o campo perder o foco e grava neste quadro) e
/// pede o quadro seguinte, em que as ações pendentes trocam de aba.
fn terminar_edicao(ctx: &egui::Context) {
    ctx.memory_mut(|m| m.stop_text_input());
    ctx.request_repaint();
}

/// A frase da confirmação de uma troca de banco (ou do fechamento) com a transação aberta (com o nome
/// da Consulta que a abriu, [`casca::nome_da_consulta`]).
pub(crate) fn frase_descartar(acao: &str, consulta: &str) -> String {
    format!(
        "{acao} descarta a transação aberta na {consulta}: tudo o que foi feito desde o BEGIN, também o que foi \
         salvo pelas abas Dados e Estrutura, se perde e não vai para o arquivo. Continuar?"
    )
}

/// O que fazer quando a confirmação aberta for aceita.
enum Confirmado {
    /// Refazer a troca com `descartar_transacao`.
    Troca(Troca, Origem),
    /// Fechar o aplicativo (falta perguntar da transação aberta, se houver).
    FecharApp { falta_transacao: bool },
    /// A pergunta da fila ao fechar sobre esta aba alterada: Salvar grava e espera; Não salvar segue sem gravar;
    /// Cancelar para a fila toda.
    SalvarAlterado(IdAba),
}

/// O documento de mentira que os testes da casca põem no lugar do editor de um arquivo de dados
/// ([`Editor::com_editor_de_teste`]).
type FabricaDeTeste = dyn Fn(&Path) -> Option<Box<dyn estado::Documento>>;

/// Em que pé está a casca: o documento da aba só aparece depois de o estado chegar.
enum Situacao {
    /// Antes de o estado chegar.
    Carregando,
    /// O estado não pôde ser lido ao iniciar.
    Falha(String),
    /// O documento da aba ativa está à vista.
    Pronta,
}

/// As janelas globais (abertas por [`Acao`]).
#[derive(Default)]
struct Janelas {
    exportar: Option<JanelaExportar>,
    /// O pedido com que a janela Exportar abriu (para os testes conferirem a origem).
    pedido_exportar: Option<PedidoJanelaExportar>,
    nova_tabela: Option<JanelaNovaTabela>,
    novo_indice: Option<JanelaNovoIndice>,
    /// A tabela da janela Novo índice (as abas dela são relidas quando a janela fecha depois de criar).
    tabela_do_indice: String,
    /// A janela Novo índice já criou o índice (emitiu `RecarregarObjetos`): ao fechar, as abas da tabela são
    /// relidas.
    indice_criado: bool,
    /// As janelas do menu Ajuda: "Atalhos do teclado" e "Sobre o DataBox".
    atalhos: bool,
    sobre: Option<Sobre>,
    /// "Pedir o caminho" (sem o diálogo do Windows).
    caminho: Option<PedirCaminho>,
    /// As Preferências (Arquivo, Exibir › Tema, o pé da barra de ícones e o Ctrl+,).
    preferencias: Option<JanelaPreferencias>,
}

impl Janelas {
    fn alguma(&self) -> bool {
        self.exportar.is_some()
            || self.nova_tabela.is_some()
            || self.novo_indice.is_some()
            || self.atalhos
            || self.sobre.is_some()
            || self.caminho.is_some()
            || self.preferencias.is_some()
    }

    /// Fecha as janelas que são do banco de antes (a ressincronização). As Preferências não são de banco nenhum: ficam
    /// abertas, e o que a pessoa mudou nelas continua valendo até o Salvar ou o Cancelar.
    fn fechar_todas(&mut self) {
        let preferencias = self.preferencias.take();
        *self = Janelas::default();
        self.preferencias = preferencias;
    }
}

/// O DataBox inteiro.
pub struct Editor {
    servico: Arc<Servico>,
    estado: Estado,
    versao: u64,
    carregado: bool,
    /// Por que a lista de objetos do banco aberto não chegou (a leitura falhou): o Explorador mostra o motivo e o
    /// "Tentar de novo" em vez do ponto pulsante. None com a lista à vista ou com a leitura ainda correndo.
    falha_objetos: Option<String>,
    /// As leituras do BancoInfo são numeradas (a próxima é a `leituras_banco`): uma que volta depois de outra mais
    /// nova já ter sido aplicada é descartada (`banco_aplicada`), e uma pedida antes da última nota de transação que
    /// mudou (`banco_nota`) não sobrescreve o estado da transação, que a nota acertou depois dela.
    leituras_banco: u64,
    banco_aplicada: u64,
    banco_nota: u64,
    iniciado: bool,
    tema: Tema,
    situacao: Situacao,
    /// As abas de documento, com o histórico único de voltar e avançar.
    abas: Abas<Documento>,
    /// A aba de Consulta SQL que abriu a transação (a que o "Ir para a Consulta SQL" foca); None sem transação ou
    /// quando ela não veio de uma Consulta.
    consulta_da_transacao: Option<IdAba>,
    casca: Casca,
    /// Onde mora cada painel (as docas e as janelas próprias; o lugar fica nas preferências) e o que está à vista
    /// (só na sessão).
    paineis: Paineis,
    /// As larguras dos painéis laterais que a pessoa escolheu (as divisórias; ficam nas preferências) e as que já
    /// estão guardadas.
    larguras: Larguras,
    larguras_guardadas: Larguras,
    /// Há uma gravação das larguras em andamento.
    gravando_larguras: bool,
    /// Como o corpo se repartiu no último quadro.
    disposicao: Option<Disposicao>,
    /// Onde abrir a janela própria do painel que a pessoa arrastou para fora (no ponto em que soltou a aba, em
    /// coordenadas da tela), no quadro em que ela abre.
    posicao_da_janela: Option<(Painel, Pos2)>,
    /// O verbo do atalho do quadro anterior (o F5), para o documento executar antes de se desenhar, depois de a
    /// edição em curso gravar.
    verbo_pendente: Option<Verbo>,
    /// Onde o documento (o `.doc`) ficou no último quadro.
    rect_documento: Option<Rect>,
    /// O painel de Propriedades ficou à vista no último quadro.
    propriedades_a_vista: bool,
    janelas: Janelas,
    confirmacao: Option<Confirmacao>,
    confirmado: Option<Confirmado>,
    carga: Option<Promessa<Carga>>,
    /// A carga em andamento veio de um `BancoTrocado`: o caminho do banco que a tela mostrava.
    ressincronizando: Option<Option<PathBuf>>,
    troca: Option<TrocaPendente>,
    tarefas: Vec<Tarefa>,
    dialogos: Dialogador,
    partida: Option<Partida>,
    erro_inicial: Option<String>,
    titulo: String,
    titulo_enviado: Option<String>,
    /// A janela não tem a decoração do sistema: a barra de título desenha os botões minimizar, maximizar e fechar,
    /// arrasta a janela e a redimensiona pelas bordas ([`casca::janela`]).
    barra_propria: bool,
    /// A barra de status à vista (Exibir › Barra de status; o main.rs passa a das preferências, escondida por padrão).
    barra_status: bool,
    pode_fechar: bool,
    /// Um pedido de fechar chegou com a janela minimizada ou encoberta e foi cancelado (ver [`Editor::logica`]):
    /// a pergunta sai no primeiro quadro da janela restaurada.
    fechar_pendente: bool,
    /// Ações para o fim do próximo quadro, com a aba que as emitiu: as dos atalhos e da faixa de abas (a edição em
    /// curso grava antes de a aba sair de vista), as das execuções soltas e as dos testes.
    pendentes: Vec<(Option<IdAba>, Acao)>,
    /// A aba ativa que outra aba mudou (o `RecarregarObjetos` de uma Consulta, da execução solta, das janelas): ela
    /// relê no fim do quadro seguinte, depois de a célula em edição gravar (ver [`Editor::marcar_para_reler`]).
    reler_ativa: Option<IdAba>,
    /// A fonte do código que as Preferências estão lendo do disco.
    fonte_pendente: Option<FontePendente>,
    /// O renderizador que abriu a janela (a janela "Sobre o DataBox" o mostra).
    renderizador: Renderizador,
    /// Nos testes: para onde vai o "abrir a pasta" e o "abrir o arquivo" da janela "Sobre o DataBox", no lugar do
    /// Explorador de Arquivos.
    abrir_de_teste: Option<GanchoAbrir>,
    /// Quando (o `input.time`) foi a última releitura automática da pasta aberta ([`INTERVALO_RELEITURA`]).
    ultima_releitura: Option<f64>,
    /// A janela tinha o foco no quadro anterior (a volta do foco relê a pasta).
    focada: bool,
    /// Havia uma janela global aberta no quadro anterior (o fim dela relê a pasta: a Exportar e a cópia criam
    /// arquivos).
    com_janela: bool,
    /// Quantas leituras de pasta a casca já pediu (para os testes).
    leituras_de_pasta: u64,
    /// Sobe a cada Fechar pasta e a cada pasta nova: a abertura pedida antes de um Fechar pasta chega e é desfeita
    /// (`Tarefa::AbrirPasta`), e as leituras da pasta de antes são descartadas (`Tarefa::Pasta`).
    geracao_pasta: u64,
    /// Uma releitura pedida ([`Editor::reler_a_pasta`]) espera a leitura em andamento terminar.
    reler_pendente: bool,
    /// A fila de perguntas ao fechar abas alteradas (etapa 5), enquanto ela anda.
    fila: Option<FilaDeSalvar>,
    /// Nos testes: o documento de mentira no lugar do editor de um arquivo de dados.
    editor_de_teste: Option<Rc<FabricaDeTeste>>,
}

impl Editor {
    /// O aplicativo com o serviço, o tema das preferências e o banco da linha de comando. Não toca no disco:
    /// o estado é lido numa tarefa no primeiro quadro.
    pub fn novo(servico: Arc<Servico>, tema: Tema, partida: Partida) -> Editor {
        let app = servico.app_info();
        let estado = Estado {
            app,
            banco: None,
            objetos: None,
            pasta: None,
            prefs: PrefsInfo {
                tema: tema_das_prefs(tema),
                recentes: Vec::new(),
                exportacao: prefs::OpcoesExportacao::default(),
                aparencia: prefs::Aparencia::default(),
            },
            ultimo_diretorio: None,
        };
        Editor {
            servico,
            estado,
            versao: 0,
            carregado: false,
            falha_objetos: None,
            leituras_banco: 0,
            banco_aplicada: 0,
            banco_nota: 0,
            iniciado: false,
            tema,
            situacao: Situacao::Carregando,
            abas: Abas::nova(Rota::Inicio, |id, r| Documento::novo(id, r, None)),
            consulta_da_transacao: None,
            casca: Casca::default(),
            paineis: Paineis::default(),
            larguras: Larguras::default(),
            larguras_guardadas: Larguras::default(),
            gravando_larguras: false,
            disposicao: None,
            posicao_da_janela: None,
            verbo_pendente: None,
            rect_documento: None,
            propriedades_a_vista: false,
            janelas: Janelas::default(),
            confirmacao: None,
            confirmado: None,
            carga: None,
            ressincronizando: None,
            troca: None,
            tarefas: Vec::new(),
            dialogos: Dialogador::default(),
            partida: Some(partida),
            erro_inicial: None,
            titulo: titulo_da_janela(None, None),
            titulo_enviado: None,
            barra_propria: true,
            barra_status: true,
            pode_fechar: false,
            fechar_pendente: false,
            pendentes: Vec::new(),
            reler_ativa: None,
            fonte_pendente: None,
            renderizador: Renderizador::default(),
            abrir_de_teste: None,
            ultima_releitura: None,
            focada: true,
            com_janela: false,
            leituras_de_pasta: 0,
            geracao_pasta: 0,
            reler_pendente: false,
            fila: None,
            editor_de_teste: None,
        }
    }

    /// Nos testes: o editor de um arquivo de dados que abre (`Rota::Arquivo`) é o documento que `f` devolve para o
    /// caminho (None: a tela de sempre). É como os testes da casca (a fila de perguntas ao fechar, o ponto da aba, a
    /// barra de status, Ctrl+S/Z/Y) usam um documento editável de mentira, sem depender dos editores de verdade.
    #[doc(hidden)]
    pub fn com_editor_de_teste(mut self, f: impl Fn(&Path) -> Option<Box<dyn estado::Documento>> + 'static) -> Editor {
        self.editor_de_teste = Some(Rc::new(f));
        self
    }

    /// Nos testes: a fila de perguntas ao fechar abas alteradas está andando (uma pergunta aberta, ou uma gravação
    /// sendo esperada).
    #[doc(hidden)]
    pub fn fila_ao_fechar(&self) -> bool {
        self.fila.is_some()
    }

    /// O renderizador com que a janela foi aberta (o padrão é o glow, o OpenGL): a janela "Sobre o DataBox" o
    /// mostra, "OpenGL (glow)" ou "wgpu".
    pub fn com_renderizador(mut self, renderizador: Renderizador) -> Editor {
        self.renderizador = renderizador;
        self
    }

    /// Com as larguras dos painéis laterais que a pessoa escolheu nas sessões de antes (`prefs.paineis()`, lidas pelo
    /// executável antes da janela; a que nunca foi escolhida fica a padrão).
    pub fn com_larguras(mut self, paineis: prefs::LarguraPaineis) -> Editor {
        self.larguras = Larguras::from(paineis);
        self.larguras_guardadas = self.larguras;
        self
    }

    /// Com ou sem a barra de status (`prefs.barra_status()`, lida pelo executável antes da janela; escondida enquanto
    /// a pessoa não a mostrar pelo Exibir). O padrão do `Editor` é com ela, como o dos testes de tela.
    pub fn com_barra_status(mut self, sim: bool) -> Editor {
        self.barra_status = sim;
        self
    }

    /// A barra de status está à vista.
    #[doc(hidden)]
    pub fn barra_status_a_vista(&self) -> bool {
        self.barra_status
    }

    /// Com os lugares dos painéis que a pessoa escolheu nas sessões de antes (`prefs.lugares()`, lidos pelo
    /// executável antes da janela; o que nunca foi mudado fica no padrão).
    pub fn com_lugares(mut self, lugares: prefs::LugaresPaineis) -> Editor {
        self.paineis = Paineis::de_prefs(lugares);
        self
    }

    /// Onde mora o painel ("Explorador", "Histórico" ou "Propriedades") agora: `"esquerda"`, `"direita"` ou
    /// `"janela"`.
    #[doc(hidden)]
    pub fn lugar_do_painel(&self, nome: &str) -> Option<&'static str> {
        let painel = Painel::TODOS.into_iter().find(|p| p.nome() == nome)?;
        Some(match self.paineis.lugar(painel) {
            Lugar::Doca(Lado::Esquerda) => "esquerda",
            Lugar::Doca(Lado::Direita) => "direita",
            Lugar::Janela => "janela",
        })
    }

    /// A vista das Propriedades da tabela à vista ("Resumo", "Colunas", "Índices", "SQL"); None fora de uma tabela.
    #[doc(hidden)]
    pub fn vista_da_tabela(&self) -> Option<&'static str> {
        match &self.abas.ativa().conteudo {
            Documento::Tabela(t) => Some(t.vista().rotulo(false)),
            _ => None,
        }
    }

    /// O painel está à vista (a aba ativa de uma doca à vista, ou a janela própria aberta).
    #[doc(hidden)]
    pub fn painel_a_vista(&self, nome: &str) -> bool {
        let tem = matches!(self.situacao, Situacao::Pronta) && self.abas.ativa().conteudo.tem_propriedades();
        Painel::TODOS.into_iter().find(|p| p.nome() == nome).is_some_and(|p| self.paineis.a_vista(p, tem))
    }

    /// As larguras das docas da esquerda e da direita que a casca desenhou no último quadro (a escolha da pessoa já
    /// limitada e, numa janela estreita, encolhida); `None` antes do primeiro quadro.
    #[doc(hidden)]
    pub fn larguras_desenhadas(&self) -> Option<(f32, f32)> {
        self.disposicao.map(|d| (d.esquerda, d.direita))
    }

    /// A escolha da pessoa para as docas da esquerda e da direita (a que vai para as preferências).
    #[doc(hidden)]
    pub fn larguras_escolhidas(&self) -> (f32, f32) {
        (self.larguras.esquerda, self.larguras.direita)
    }

    /// Nos testes: o "abrir a pasta das preferências" e o "abrir o log" da janela "Sobre o DataBox" chamam `f` com o
    /// caminho, em vez de abrir o Explorador de Arquivos.
    #[doc(hidden)]
    pub fn com_abrir_de_teste(mut self, f: impl FnMut(&Path) + 'static) -> Editor {
        self.abrir_de_teste = Some(Box::new(f));
        self
    }

    /// Com a barra de título própria (a janela sem a decoração do sistema, como o main.rs a abre no Windows) ou sem
    /// ela (a barra do sistema em cima: a faixa fica só com o logo, os menus e o título, sem os botões, o arrasto e
    /// as bordas). O padrão é com ela, o desenho das pranchetas, para os testes de tela serem os mesmos em todo
    /// sistema; o main.rs escolhe pelo sistema (`BARRA_PROPRIA`: só no Windows).
    pub fn com_barra_propria(mut self, sim: bool) -> Editor {
        self.barra_propria = sim;
        self
    }

    /// Nos testes: o que os diálogos do Windows devolveriam (None = a pessoa cancelou).
    #[doc(hidden)]
    pub fn com_dialogo(mut self, f: impl FnMut(&PedidoDialogo) -> Option<PathBuf> + 'static) -> Editor {
        self.dialogos = Dialogador::com_falso(f);
        self
    }

    /// Nos testes: como se o diálogo do Windows não abrisse (a janela "Pedir o caminho" aparece no lugar dele).
    #[doc(hidden)]
    pub fn sem_dialogo_nativo(mut self) -> Editor {
        self.dialogos.sem_nativo();
        self
    }

    /// Nos testes: uma ação como as que a casca e as janelas emitem (sem aba de origem), aplicada no fim do próximo
    /// quadro.
    #[doc(hidden)]
    pub fn emitir(&mut self, acao: Acao) {
        self.pendentes.push((None, acao));
    }

    /// Nos testes: uma ação como se o documento desta aba a emitisse, aplicada no fim do próximo quadro.
    #[doc(hidden)]
    pub fn emitir_de(&mut self, aba: IdAba, acao: Acao) {
        self.pendentes.push((Some(aba), acao));
    }

    /// Nos testes: a leitura dos objetos do banco aberto falhou com `mensagem` (sem a lista de antes), como quando o
    /// serviço devolve um erro na primeira leitura: o Explorador mostra o motivo e o "Tentar de novo".
    #[doc(hidden)]
    pub fn simular_falha_dos_objetos(&mut self, ctx: &egui::Context, mensagem: &str) {
        self.estado.objetos = None;
        let id = self.estado.banco_id();
        self.fim_da_tarefa(ctx, FimTarefa::Objetos(id, Err(ErroBanco::novo(mensagem))));
    }

    /// Nos testes: a pasta aberta (e o que já foi lido dela) como se a casca a tivesse posto no estado, montada à mão
    /// (sem a varredura do disco). Etapa 4, passo 9.
    #[doc(hidden)]
    pub fn simular_pasta(&mut self, ctx: &egui::Context, pasta: Option<estado::PastaAberta>) {
        self.estado.pasta = pasta;
        self.mudou_estado(ctx);
    }

    /// Nos testes: quantas leituras de pasta (a raiz e as subpastas, pelo `Servico::listar_pasta`) a casca já pediu.
    #[doc(hidden)]
    pub fn leituras_de_pasta(&self) -> u64 {
        self.leituras_de_pasta
    }

    /// Nos testes: as ações que o Explorador emitiu desde a última chamada (as últimas 32), tratadas ou não pela casca.
    #[doc(hidden)]
    pub fn acoes_do_explorador(&mut self) -> Vec<Acao> {
        self.casca.acoes_do_explorador()
    }

    /// Nos testes: a leitura do Histórico de consultas falhou com `mensagem` (sem lista nenhuma): fica o aviso, sem
    /// o ponto pulsante.
    #[doc(hidden)]
    pub fn simular_falha_do_historico(&mut self, mensagem: &str) {
        let id = self.estado.banco_id();
        self.casca.historico.simular_falha(id, mensagem);
    }

    /// As abas abertas, na ordem da faixa: o rótulo ("clientes · Dados", "exemplo.db · Visão geral", "Consulta 2")
    /// e se é a ativa.
    #[doc(hidden)]
    pub fn abas_abertas(&self) -> Vec<(String, bool)> {
        casca::abas::titulos(&self.abas, &self.estado)
            .into_iter()
            .map(|(nome, sub, ativa)| (sub.map_or_else(|| nome.clone(), |s| format!("{nome} · {s}")), ativa))
            .collect()
    }

    /// As rotas das abas abertas, na ordem da faixa.
    #[doc(hidden)]
    pub fn rotas_abertas(&self) -> Vec<Rota> {
        self.abas.iter().map(|a| a.rota().clone()).collect()
    }

    /// A aba ativa.
    #[doc(hidden)]
    pub fn id_aba_ativa(&self) -> IdAba {
        self.abas.id_ativa()
    }

    /// Onde o documento da aba ativa (o `.doc`, com a borda) ficou no último quadro.
    #[doc(hidden)]
    pub fn rect_do_documento(&self) -> Option<Rect> {
        self.rect_documento
    }

    /// O painel de Propriedades ficou à vista no último quadro.
    #[doc(hidden)]
    pub fn propriedades_a_vista(&self) -> bool {
        self.propriedades_a_vista
    }

    /// O pedido com que a janela Exportar aberta abriu (None sem ela).
    #[doc(hidden)]
    pub fn exportacao_aberta(&self) -> Option<&PedidoJanelaExportar> {
        self.janelas.exportar.as_ref().and(self.janelas.pedido_exportar.as_ref())
    }

    /// A tabela (ou visão) carregada na aba ativa, se for a tela de tabela.
    #[doc(hidden)]
    pub fn tabela_aberta(&self) -> Option<&TabelaAberta> {
        match self.documento() {
            Some(Documento::Tabela(t)) => t.tabela_aberta(),
            _ => None,
        }
    }

    /// Há uma troca de banco em andamento (abrir, criar ou fechar).
    #[doc(hidden)]
    pub fn trocando(&self) -> bool {
        self.troca.is_some()
    }

    /// O título da janela agora ("vendas.db · DataBox").
    pub fn titulo(&self) -> &str {
        &self.titulo
    }

    /// A rota da aba ativa.
    pub fn rota(&self) -> &Rota {
        self.abas.ativa().rota()
    }

    /// O estado que a casca mantém.
    pub fn estado(&self) -> &Estado {
        &self.estado
    }

    /// O tema escolhido.
    pub fn tema(&self) -> Tema {
        self.tema
    }

    /// O estado inicial já chegou.
    pub fn carregado(&self) -> bool {
        self.carregado
    }

    /// Alguma janela global (Exportar, Nova tabela, Novo índice, Atalhos) ou confirmação está aberta.
    pub fn janela_aberta(&self) -> bool {
        self.janelas.alguma() || self.confirmacao.is_some()
    }

    /// Há tarefas da casca (com a leitura do Histórico de consultas) ou do documento à vista rodando (para os testes
    /// esperarem). Só a aba ativa conta: as respostas das escondidas só são tomadas quando elas voltam (menos as das
    /// Consultas, que não contam).
    #[doc(hidden)]
    pub fn ocupado(&self) -> bool {
        let documento = self.documento().is_some_and(Documento::ocupada);
        self.carga.is_some()
            || self.troca.is_some()
            || !self.tarefas.is_empty()
            || self.fonte_pendente.is_some()
            || self.janelas.preferencias.as_ref().is_some_and(JanelaPreferencias::ocupada)
            || self.casca.ocupada()
            || documento
    }

    /// A janela Preferências está aberta, e em que seção (para os testes).
    #[doc(hidden)]
    pub fn preferencias_aberta(&self) -> Option<&'static str> {
        self.janelas.preferencias.as_ref().map(|j| j.secao().rotulo())
    }

    /// O documento à vista: o da aba ativa, depois de o estado chegar.
    fn documento(&self) -> Option<&Documento> {
        matches!(self.situacao, Situacao::Pronta).then(|| &self.abas.ativa().conteudo)
    }

    /// Nos testes: abre a aba da rota ou foca a que já está aberta, como um clique num link.
    #[doc(hidden)]
    pub fn ir(&mut self, rota: Rota) {
        self.navegar(rota);
    }

    fn mudou_estado(&mut self, ctx: &egui::Context) {
        self.versao = self.versao.wrapping_add(1);
        let pasta = self.estado.pasta.as_ref().map(|p| p.nome.as_str());
        self.titulo = titulo_da_janela(pasta, self.estado.banco.as_ref().map(|b| b.nome.as_str()));
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod testes;
