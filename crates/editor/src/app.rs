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

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use banco::{BancoInfo, ObjetosInfo, ResultadoConsulta};
use comum::app::NOME;
use comum::arquivos::{Listagem, Natureza, e_extensao_de_banco};
use comum::{Codigo, ErroBanco, Resultado};
use egui::{Align, Id, Key, Layout, Modifiers, Pos2, Rect, Ui, UiBuilder};
use egui::{ViewportCommand, pos2, vec2};
use estado::{
    Abas, Acao, Anatomia, ChaveAba, Contexto, Estado, FonteDeColunas, IdAba, NoDaPasta, Ofertas, PastaAberta,
    PedidoJanelaExportar, Promessa, Rolagem, Rota, TabelaAberta, Verbo,
};
use interface::codigo::{self, AparenciaCodigo, CoresCodigo};
use interface::fontes;
use interface::formato::formatar_numero;
use interface::pecas::abas_documento::ALTURA_FAIXA;
use interface::pecas::avisos::aviso_texto;
use interface::pecas::barra_status::ALTURA_STATUS;
use interface::pecas::barra_titulo::ALTURA_TITULO;
use interface::pecas::cartao::Cartao;
use interface::pecas::divisoria::{Divisoria, LadoDoPainel, SaidaDivisoria};
use interface::pecas::doca::{AbaDoca, Arrasto, Doca, FormaAbas};
use interface::pecas::documento::{Documento as DocumentoPeca, Pagina};
use interface::pecas::ferramentas::ALTURA_BARRA;
use interface::pecas::janela::{Confirmacao, Decisao, confirmar_decisao};
use interface::pecas::painel::Encaixe;
use interface::pecas::toast::{acao_do_toast, desenhar_toast, toast_atencao, toast_erro};
use interface::pintura;
use interface::tema::{self, Tema};
use prefs::PrefsInfo;
use servico::{InfoArquivo, Servico};

use crate::casca::docas::{Lado, Lugar, Paineis, Painel};
use crate::casca::ferramentas::{Barra, EstadoPainel, PedidoBarra};
use crate::casca::larguras::{self, Corpo, Disposicao, Larguras};
use crate::casca::menus::{ItemTabela, MenuTabela, Menus, PedidoMenu};
use crate::casca::preferencias::{Efeito, Fim, JanelaPreferencias, Secao};
use crate::casca::sobre::{GanchoAbrir, Sobre};
use crate::casca::{self, Casca, titulo_da_janela};
use crate::documento::Documento;
use crate::linha_comando::Renderizador;
use crate::pedir_caminho::{PedirCaminho, Proposito, SaidaCaminho};
use crate::recursos::{Dialogador, PedidoDialogo, RespostaDialogo};
use crate::tela_arquivo::PedidoArquivo;
use crate::telas::{JanelaExportar, JanelaNovaTabela, JanelaNovoIndice};
use crate::visao_geral::{Execucao, PedidoDoMenu, Resultado as ResultadoDaTela, avisar_resultado};

/// O `padding:8px` e o `gap:8px` do corpo (entre a doca da esquerda, o documento e a doca da direita), a margem dos lados da barra de ferramentas (`margin:6px 8px 0`) e a da barra de status
/// (`margin:0 8px 8px`).
const ESPACO: f32 = 8.0;
/// O espaço entre a faixa de 30px do alto e a barra de ferramentas (`header.vidro{margin-top:6px}`).
const MARGEM_BARRA: f32 = 6.0;
/// O quanto a página rola no End, até o fim (o Home vai ao começo pelo `ao_topo`): mais que qualquer página, e finito
/// (o `scroll_with_delta` não aceita o infinito).
const ROLAR_ATE_O_FIM: f32 = -1.0e7;

/// Uma troca de banco mais rápida que isto não mostra nada (sem piscar o card).
const ESPERA_PARA_MOSTRAR: f64 = 0.3;
/// Depois disto, o card da troca diz que ela espera o banco atual terminar o que está fazendo.
const ESPERA_LONGA: f64 = 2.0;

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

/// O toast do Tabela › Copiar o nome.
pub(crate) const MSG_NOME_COPIADO: &str = "Nome copiado.";

/// O que a pessoa pediu na janela própria de um painel.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PedidoDaJanela {
    /// O × do sistema ou o "Fechar" do cabeçalho: o painel volta escondido para a doca.
    Fechar,
    /// O "Prender de volta" (None: na doca de onde ele saiu) ou a aba solta em cima de um lado da janela principal.
    Prender(Option<Lado>),
    /// A aba foi solta neste ponto da janela própria (quem chama vê se caiu na janela principal).
    Solta(Pos2),
}

/// O tamanho com que a janela própria de um painel abre: a das Propriedades na largura da forma larga (a tabela de
/// colunas, os 720px da prancheta 6); a do Explorador e a do Histórico, na de uma doca larga.
fn tamanho_da_janela(painel: Painel) -> (f32, f32) {
    match painel {
        Painel::Propriedades => (720.0, 480.0),
        Painel::Explorador => (320.0, 560.0),
        Painel::Historico => (360.0, 520.0),
    }
}

/// O rótulo da divisória de uma doca, pelo painel à vista nela ("Largura do Explorador").
fn rotulo_da_largura(painel: Option<Painel>) -> &'static str {
    match painel {
        Some(Painel::Explorador) | None => "Largura do Explorador",
        Some(Painel::Historico) => "Largura do Histórico",
        Some(Painel::Propriedades) => "Largura das Propriedades",
    }
}

/// A faixa da beirada do corpo que recebe uma aba arrastada quando a doca daquele lado está escondida.
const FAIXA_DE_ALVO: f32 = 120.0;

/// Onde cai a aba de painel solta em `ponteiro` (prancheta 2b) e o que acender: fora da janela, a janela própria;
/// sobre uma doca (ou, com ela escondida, a faixa da beirada do corpo daquele lado), aquela doca; no meio, nada.
fn destino_do_arrasto(
    ponteiro: Pos2,
    janela: Rect,
    esquerda: Option<Rect>,
    inicio_direita: Option<f32>,
    corpo: Rect,
) -> Option<(Lugar, Rect)> {
    if !janela.contains(ponteiro) {
        return Some((Lugar::Janela, janela.shrink(2.0)));
    }
    let zona_esquerda =
        esquerda.unwrap_or_else(|| Rect::from_min_max(corpo.min, pos2(corpo.left() + FAIXA_DE_ALVO, corpo.bottom())));
    let zona_direita = match inicio_direita {
        Some(x) => Rect::from_min_max(pos2(x + ESPACO, corpo.top()), corpo.max),
        None => Rect::from_min_max(pos2(corpo.right() - FAIXA_DE_ALVO, corpo.top()), corpo.max),
    };
    if zona_esquerda.contains(ponteiro) {
        Some((Lugar::Doca(Lado::Esquerda), zona_esquerda))
    } else if zona_direita.contains(ponteiro) {
        Some((Lugar::Doca(Lado::Direita), zona_direita))
    } else {
        None
    }
}

/// prefs::Tema → o Tema da interface (pelo texto: "system" | "light" | "dark").
pub fn tema_da_interface(t: prefs::Tema) -> Tema {
    Tema::de_str(t.como_str()).unwrap_or_default()
}

/// O Tema da interface → prefs::Tema.
pub fn tema_das_prefs(t: Tema) -> prefs::Tema {
    prefs::Tema::de_texto(t.como_str()).unwrap_or_default()
}

/// A aparência do código das preferências → a do interface (as cores pelo texto do arquivo).
pub fn aparencia_da_interface(a: &prefs::Aparencia) -> AparenciaCodigo {
    AparenciaCodigo {
        cores: CoresCodigo::de_str(a.cores_codigo.como_str()).unwrap_or_default(),
        familia: a.fonte_codigo.clone(),
        tamanho: a.tamanho_codigo,
    }
}

/// Aplica a aparência do código das preferências: as cores e o tamanho na hora e, se a família não for a padrão
/// (Consolas), a escolhida, lida do disco **agora** (uma fonte, ao subir, ao lado das do sistema que o
/// `tema::aplicar` lê; nas Preferências a leitura vai numa tarefa). Uma família que não existe nesta máquina, ou que
/// não é fonte, fica de fora: vale a padrão (e a aparência em vigor diz None), com o aviso no registro.
pub fn aplicar_aparencia(ctx: &egui::Context, a: &prefs::Aparencia) {
    let mut aparencia = aparencia_da_interface(a);
    match aparencia.familia.clone().filter(|f| f != fontes::FAMILIA_MONO_PADRAO) {
        None => fontes::restaurar_mono(ctx),
        Some(nome) => {
            let lida = fontes::familias_mono_disponiveis()
                .into_iter()
                .find(|f| f.nome == nome)
                .and_then(|f| fontes::ler_familia_mono(&f));
            if !lida.is_some_and(|bytes| fontes::trocar_mono(ctx, bytes, &nome)) {
                log::warn!(
                    "A fonte do código {nome:?} não foi encontrada nesta máquina; vale a {}.",
                    fontes::FAMILIA_MONO_PADRAO
                );
                fontes::restaurar_mono(ctx);
                aparencia.familia = None;
            }
        }
    }
    interface::codigo::definir(ctx, aparencia);
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

/// Uma troca de banco.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Troca {
    Abrir { caminho: PathBuf, somente_leitura: bool },
    Novo { caminho: PathBuf },
    Fechar,
}

impl Troca {
    /// O começo da frase da confirmação de descartar a transação.
    fn acao(&self) -> &'static str {
        match self {
            Troca::Abrir { .. } => "Abrir outro banco",
            Troca::Novo { .. } => "Criar um banco novo",
            Troca::Fechar => "Fechar o banco",
        }
    }

    /// O card da troca que está demorando: logo no começo e depois de [`ESPERA_LONGA`].
    fn em_andamento(&self, longa: bool) -> &'static str {
        match (self, longa) {
            (Troca::Abrir { .. }, false) => "Abrindo o banco…",
            (Troca::Novo { .. }, false) => "Criando o banco…",
            (Troca::Fechar, false) => "Fechando o banco…",
            (Troca::Abrir { .. }, true) => MSG_ABRINDO_ESPERANDO,
            (Troca::Novo { .. }, true) => MSG_CRIANDO_ESPERANDO,
            (Troca::Fechar, true) => MSG_FECHANDO_ESPERANDO,
        }
    }
}

/// Os textos do card da troca que passou de [`ESPERA_LONGA`] (a conexão está presa numa operação longa).
pub(crate) const MSG_ABRINDO_ESPERANDO: &str =
    "Abrindo o banco. Aguardando o banco atual terminar a operação em andamento.";
pub(crate) const MSG_CRIANDO_ESPERANDO: &str =
    "Criando o banco. Aguardando o banco atual terminar a operação em andamento.";
pub(crate) const MSG_FECHANDO_ESPERANDO: &str = "Fechando o banco. Aguardando a operação em andamento terminar.";

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

/// O caminho tem uma extensão de banco (`.db`, `.sqlite`…): pedido pelo nome, ele abre como banco mesmo quando a
/// classificação não o reconhece, e o erro do SQLite diz por quê ("O arquivo não é um banco SQLite…").
fn tem_extensao_de_banco(caminho: &Path) -> bool {
    caminho.extension().and_then(|e| e.to_str()).is_some_and(e_extensao_de_banco)
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

struct TrocaPendente {
    troca: Troca,
    origem: Origem,
    promessa: Promessa<Carga>,
    /// Quando começou (o `input.time`), para o card da troca que demora.
    desde: f64,
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

/// Para onde a fila de perguntas ao fechar vai quando acaba.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Destino {
    /// O ×, o clique do meio, o Ctrl+W, o Arquivo › Fechar aba.
    Aba(IdAba),
    /// O Fechar pasta (fecha as abas de arquivo de dentro dela), com a chave da pasta de quando a fila começou: se
    /// outra pasta abriu no meio, nada fecha.
    Pasta(String),
    /// O pedido de fechar a janela: depois dos arquivos, as perguntas da exportação e da transação que já existiam.
    Janela,
}

/// A fila de perguntas ao fechar: as abas alteradas que vão fechar, na ordem da faixa. Uma de cada vez,
/// a aba vai à frente e a pergunta "Salvar as alterações em {nome}?" sai (Não salvar · Cancelar · Salvar). Salvar
/// roda o verbo Salvar do documento e espera a gravação terminar (`salvando`); se ela falha (o erro de
/// `tomar_erro_de_gravacao`), o toast com o erro e tudo para, sem fechar nada. Cancelar (e o Esc) também para tudo.
/// Quando a fila acaba, o [`Destino`], depois de conferir de novo: a aba alterada no meio (enquanto uma gravação era
/// esperada) entra na fila antes de fechar. Se a pergunta aberta some sem resposta (outra confirmação tomou o lugar
/// dela, o banco trocado por fora), a fila para: nada fecha sem a pessoa responder.
struct FilaDeSalvar {
    restantes: VecDeque<IdAba>,
    destino: Destino,
    /// A aba cuja gravação a fila espera terminar.
    esperando: Option<IdAba>,
    /// A aba da pergunta aberta agora (até a resposta).
    perguntando: Option<IdAba>,
    /// As abas já respondidas (Não salvar, ou Salvar que gravou): não se pergunta de novo.
    respondidas: HashSet<IdAba>,
}

/// O documento de mentira que os testes da casca põem no lugar do editor de um arquivo de dados
/// ([`Editor::com_editor_de_teste`]).
type FabricaDeTeste = dyn Fn(&Path) -> Option<Box<dyn estado::Documento>>;

/// O botão da segunda saída da pergunta ao fechar (prancheta 15, N2).
pub(crate) const NAO_SALVAR: &str = "Não salvar";

/// O título da pergunta da fila ao fechar: "Salvar as alterações em vendas_2025.csv?".
pub(crate) fn titulo_salvar_alteracoes(nome: &str) -> String {
    format!("Salvar as alterações em {nome}?")
}

/// A frase da pergunta: "Há 1 alteração não salva. Se não salvar, ela se perde.", "Há 3 alterações não salvas. Se
/// não salvar, elas se perdem." ou, sem a contagem (o texto e o JSON), "Há alterações não salvas. Se não salvar, elas
/// se perdem.".
pub(crate) fn frase_salvar_alteracoes(alteracoes: Option<usize>) -> String {
    match alteracoes {
        Some(1) => "Há 1 alteração não salva. Se não salvar, ela se perde.".to_string(),
        Some(n) if n > 1 => format!("Há {} alterações não salvas. Se não salvar, elas se perdem.", formatar_numero(n)),
        _ => "Há alterações não salvas. Se não salvar, elas se perdem.".to_string(),
    }
}

/// Uma leitura da casca em andamento (com o id do banco para que foi pedida).
enum Tarefa {
    Objetos(String, Promessa<ObjetosInfo>),
    /// A leitura do BancoInfo, com o número dela (ver [`Editor::pedir_banco`]).
    Banco(String, u64, Promessa<BancoInfo>),
    Prefs(Promessa<(PrefsInfo, Option<PathBuf>)>),
    /// As preferências que voltam de uma gravação; `limpou_historico`: a gravação limpou o histórico de consultas,
    /// que só vale reler quando ela terminar.
    Tema {
        promessa: Promessa<PrefsInfo>,
        limpou_historico: bool,
    },
    /// A gravação das larguras dos painéis (uma por vez, ver [`Editor::guardar_larguras`]).
    Larguras(Promessa<PrefsInfo>),
    Diretorio(Promessa<()>),
    /// A execução da Consulta SQL que ficou correndo quando a pessoa saiu da tela (com o id do banco).
    Consulta(String, Promessa<ResultadoConsulta>),
    /// A manutenção ou a cópia de segurança da Visão geral que ficou correndo quando a aba saiu (com o id do banco).
    VisaoGeral(String, Execucao),
    /// O "Mostrar na pasta" do toast da cópia de segurança e do Explorador, e o "Abrir no programa padrão" da ficha do
    /// arquivo (o Explorador de Arquivos, ou o programa, abre numa tarefa).
    Revelar(Promessa<()>),
    /// A leitura de uma pasta da árvore (etapa 4): a chave da pasta aberta quando foi pedida (a leitura de outra
    /// pasta é descartada) e a chave da pasta lida (a raiz ou uma subpasta).
    Pasta {
        raiz: String,
        no: String,
        /// O [`Editor::geracao_pasta`] de quando foi pedida: a leitura pedida antes de um Fechar pasta (ou da troca
        /// de pasta) não vale para a pasta aberta depois, mesmo que seja a mesma.
        geracao: u64,
        promessa: Promessa<Listagem>,
    },
    /// Abrir uma pasta (Abrir pasta, um recente, o caminho digitado, a linha de comando, soltar na janela): a
    /// listagem da raiz e as preferências de depois (o recente novo).
    AbrirPasta {
        origem: Origem,
        caminho: PathBuf,
        /// O [`Editor::geracao_pasta`] de quando foi pedida: um Fechar pasta no meio a desfaz.
        geracao: u64,
        promessa: Promessa<(Listagem, PrefsInfo)>,
    },
    /// O que um caminho é (`Acao::AbrirCaminho`), para virar a abertura da pasta, do banco ou do arquivo.
    Alvo {
        origem: Origem,
        caminho: PathBuf,
        somente_leitura: bool,
        promessa: Promessa<Natureza>,
    },
    /// Registrar o arquivo de dados que vai abrir (o recente), antes de abrir a ficha dele.
    Arquivo {
        origem: Origem,
        caminho: PathBuf,
        /// O [`Editor::geracao_pasta`] de quando foi pedido, se o arquivo é da pasta aberta: a ficha de um arquivo da
        /// pasta que fechou nesse meio-tempo não abre (o Fechar pasta fecha as fichas de dentro dela).
        da_pasta: Option<u64>,
        promessa: Promessa<(InfoArquivo, PrefsInfo)>,
    },
    /// Uma gravação sem resposta para a tela (apagar a pasta aberta das preferências no Fechar pasta).
    Gravar(Promessa<()>),
    /// O que foi solto na janela: quantos itens vieram e o escolhido (a primeira pasta, ou o primeiro item) com o que
    /// ele é.
    Soltar {
        quantos: usize,
        promessa: Promessa<(PathBuf, Resultado<Natureza>)>,
    },
}

/// O resultado de uma [`Tarefa`].
enum FimTarefa {
    Objetos(String, Resultado<ObjetosInfo>),
    Banco(String, u64, Resultado<BancoInfo>),
    Prefs(Resultado<(PrefsInfo, Option<PathBuf>)>),
    Tema(Resultado<PrefsInfo>, bool),
    Larguras(Resultado<PrefsInfo>),
    Consulta(String, Resultado<ResultadoConsulta>),
    VisaoGeral(String, Resultado<(ResultadoDaTela, bool)>),
    Revelar(Resultado<()>),
    Pasta { raiz: String, no: String, geracao: u64, r: Resultado<Listagem> },
    AbrirPasta { origem: Origem, caminho: PathBuf, geracao: u64, r: Resultado<(Listagem, PrefsInfo)> },
    Alvo { origem: Origem, caminho: PathBuf, somente_leitura: bool, r: Resultado<Natureza> },
    Arquivo { origem: Origem, caminho: PathBuf, da_pasta: Option<u64>, r: Resultado<(InfoArquivo, PrefsInfo)> },
    Soltar { quantos: usize, caminho: PathBuf, r: Resultado<Natureza> },
    Nada,
}

/// Em que pé está a casca: o documento da aba só aparece depois de o estado chegar.
enum Situacao {
    /// Antes de o estado chegar.
    Carregando,
    /// O estado não pôde ser lido ao iniciar.
    Falha(String),
    /// O documento da aba ativa está à vista.
    Pronta,
}

/// Onde ficam o painel da esquerda, a seção do documento (a faixa de abas e o `.doc`) e as Propriedades, e o estado
/// do botão Painel de propriedades (o mesmo no menu Exibir).
struct Medidas {
    /// A doca da esquerda, se está à vista.
    esquerda: Option<Rect>,
    secao: Rect,
    /// Onde a seção do documento termina (a doca da direita começa `ESPACO` depois).
    direita: f32,
    /// A doca da direita está à vista (a pessoa a quer, ela tem aba e cabe).
    com_direita: bool,
    botao_do_painel: EstadoPainel,
    /// As larguras dos painéis neste quadro e até onde as divisórias os levam.
    disposicao: Disposicao,
}

/// Qual das docas a divisória redimensiona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PainelLateral {
    Esquerdo,
    Direito,
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

/// A leitura do arquivo da fonte do código escolhida nas Preferências (numa tarefa; o `trocar_mono` vem depois, na
/// thread da interface). Mora no editor, não na janela: o Cancelar fecha a janela com uma leitura em andamento.
struct FontePendente {
    nome: String,
    promessa: Promessa<Vec<u8>>,
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

    // ------------------------------------------------------------------ rotas

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
    fn soltar(&mut self, mut documento: Documento, id_banco: &str) {
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
    fn reiniciar_abas(&mut self, rota: Rota, id_antes: &str) {
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
    fn refazer_documentos(&mut self, id_antes: &str) {
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
    fn navegar(&mut self, rota: Rota) {
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

    /// Um item do menu Tabela, para a tabela à vista (revisão de UI de 19/10/2026): as vistas e os pedidos à Estrutura
    /// põem as Propriedades à vista; o resto vira a ação de sempre.
    fn item_da_tabela(&mut self, ctx: &egui::Context, item: ItemTabela) {
        let Documento::Tabela(t) = &mut self.abas.ativa_mut().conteudo else { return };
        let nome = t.nome().to_string();
        match item {
            ItemTabela::Vista(parte) => {
                t.mostrar_parte(parte);
                self.paineis.mostrar(Painel::Propriedades);
            }
            ItemTabela::NovaColuna | ItemTabela::Renomear | ItemTabela::Excluir => {
                t.pedir(match item {
                    ItemTabela::NovaColuna => estado::PedidoTabela::NovaColuna,
                    ItemTabela::Renomear => estado::PedidoTabela::Renomear,
                    _ => estado::PedidoTabela::Excluir,
                });
                self.paineis.mostrar(Painel::Propriedades);
            }
            ItemTabela::AbrirNaConsulta => {
                let [sql, _] = estado::consultas_da_tabela(&nome);
                self.pendentes.push((None, Acao::Navegar(Rota::consulta_com(sql))));
            }
            ItemTabela::Exportar => {
                if let Some(pedido) = t.pedido_exportacao() {
                    self.pendentes.push((None, Acao::AbrirExportacao(pedido)));
                }
            }
            ItemTabela::CopiarNome => {
                ctx.copy_text(nome);
                interface::pecas::toast::toast_concluido(ctx, MSG_NOME_COPIADO);
            }
            ItemTabela::PropriedadesEmJanela => self.mover_painel(ctx, Painel::Propriedades, Lugar::Janela),
            ItemTabela::NovoIndice => {
                self.pendentes.push((None, Acao::AbrirNovoIndice { tabela: nome, colunas: Vec::new() }));
            }
        }
        ctx.request_repaint();
    }

    /// Põe a Visão geral à vista e passa a ela o pedido do menu Banco (sem banco, nada).
    fn pedir_a_visao_geral(&mut self, pedido: PedidoDoMenu) {
        if !self.estado.tem_banco() {
            return;
        }
        self.navegar(Rota::Banco);
        if let Documento::Banco(t) = &mut self.abas.ativa_mut().conteudo {
            t.pedir(pedido);
        }
    }

    /// Uma Consulta SQL nova, sempre (o "+" da faixa de abas, um item do Histórico); sem banco, o Início.
    fn nova_consulta(&mut self, sql: Option<String>) {
        if !self.estado.tem_banco() {
            self.navegar(Rota::Inicio);
            return;
        }
        self.abas.nova_consulta(sql, |id, r| Documento::novo(id, r, None));
    }

    // ------------------------------------------------------------------ a fila de perguntas ao fechar (etapa 5)

    /// As abas alteradas (`estado::Documento::alterado`) entre as que `que` escolhe, na ordem da faixa.
    fn abas_alteradas(&self, que: impl Fn(&estado::AbaAberta<Documento>) -> bool) -> VecDeque<IdAba> {
        self.abas.iter().filter(|a| que(a) && a.conteudo.alterado()).map(|a| a.id()).collect()
    }

    /// Começa a fila de perguntas sobre estas abas alteradas, com o destino de quando acabar; false (nada começa)
    /// sem nenhuma.
    fn comecar_fila(&mut self, ctx: &egui::Context, restantes: VecDeque<IdAba>, destino: Destino) -> bool {
        if restantes.is_empty() {
            return false;
        }
        self.fila =
            Some(FilaDeSalvar { restantes, destino, esperando: None, perguntando: None, respondidas: HashSet::new() });
        ctx.request_repaint();
        true
    }

    /// As abas que o destino fecha (a da aba, as de arquivo de dentro da pasta, todas).
    fn abas_do_destino(&self, destino: &Destino) -> VecDeque<IdAba> {
        match destino {
            Destino::Aba(id) => self.abas_alteradas(|a| a.id() == *id),
            Destino::Pasta(chave) => match self.estado.pasta.as_ref().filter(|p| p.chave == *chave) {
                Some(pasta) => self.abas_alteradas(|a| a.rota().caminho_arquivo().is_some_and(|c| pasta.contem(c))),
                None => VecDeque::new(),
            },
            Destino::Janela => self.abas_alteradas(|_| true),
        }
    }

    /// O `FecharAba`: a aba alterada passa pela fila de perguntas; a outra fecha. Com a fila andando, outro pedido
    /// de fechar espera (é ignorado: a pergunta é modal, e só a gravação esperada deixa clicar).
    fn pedir_fechar_aba(&mut self, ctx: &egui::Context, id: IdAba) {
        if self.fila.is_some() {
            return;
        }
        let alteradas = self.abas_alteradas(|a| a.id() == id);
        if !self.comecar_fila(ctx, alteradas, Destino::Aba(id)) {
            self.fechar_aba(id);
        }
    }

    /// O `FecharPasta`: uma pergunta por arquivo alterado de dentro da pasta, e depois o fechar; sem nenhum, fecha.
    fn pedir_fechar_pasta(&mut self, ctx: &egui::Context) {
        if self.fila.is_some() {
            return;
        }
        let Some(chave) = self.estado.pasta.as_ref().map(|p| p.chave.clone()) else { return };
        let destino = Destino::Pasta(chave);
        let alteradas = self.abas_do_destino(&destino);
        if !self.comecar_fila(ctx, alteradas, destino) {
            self.fechar_pasta(ctx);
        }
    }

    /// O pedido de fechar a janela com abas alteradas: a fila de todas, antes das perguntas da exportação e da
    /// transação. True quando a fila começou (ou já andava): quem chama cancela o fechar. Com uma fila de outro
    /// destino andando (fechar uma aba, a pasta), ela passa a ser a de fechar a janela: no fim, as alteradas que
    /// faltam entram nela e a janela fecha.
    fn fila_ao_fechar_a_janela(&mut self, ctx: &egui::Context) -> bool {
        if let Some(f) = self.fila.as_mut() {
            f.destino = Destino::Janela;
            ctx.request_repaint();
            return true;
        }
        let alteradas = self.abas_alteradas(|_| true);
        self.comecar_fila(ctx, alteradas, Destino::Janela)
    }

    /// Anda a fila (no fim do quadro, antes da confirmação): espera a gravação pedida, faz a próxima pergunta (com a
    /// aba dela à frente) ou, vazia, cumpre o destino.
    fn andar_fila(&mut self, ctx: &egui::Context) {
        let ativa = self.abas.id_ativa();
        let Some(fila) = self.fila.as_mut() else { return };
        if let Some(id) = fila.esperando {
            if let Some(aba) = self.abas.get_mut(id) {
                // a tela perguntou algo no meio (o "Salvar mesmo assim" do arquivo que mudou no disco): espera a
                // resposta, com a aba à vista para a janela dela desenhar; fechada a pergunta com o documento ainda
                // alterado (abaixo), a pessoa cancelou
                if aba.conteudo.pergunta_aberta() {
                    if id != ativa {
                        self.abas.focar(id);
                        ctx.request_repaint();
                    }
                    return;
                }
                let doc = aba.conteudo.doc();
                if doc.salvando() {
                    // a resposta da gravação acorda a interface (a `Promessa` da tela)
                    return;
                }
                let erro = doc.tomar_erro_de_gravacao();
                let ainda_alterado = doc.alterado();
                fila.esperando = None;
                if let Some(e) = erro {
                    toast_erro(ctx, e);
                    self.fila = None;
                    return;
                }
                if ainda_alterado {
                    // o documento não gravou (o Salvar não estava ligado, a gravação foi cancelada): nada fecha
                    self.fila = None;
                    return;
                }
                fila.respondidas.insert(id);
            }
            // a aba que gravava fechou no meio: não há o que esperar
            fila.esperando = None;
        }
        // a pergunta desta fila sumiu sem resposta (outra confirmação tomou o lugar dela, o banco trocado por fora
        // fechou tudo): a fila para, e nada fecha sem a pessoa responder
        if let Some(id) = fila.perguntando {
            let ainda =
                self.confirmacao.is_some() && matches!(self.confirmado, Some(Confirmado::SalvarAlterado(p)) if p == id);
            if !ainda {
                self.fila = None;
                return;
            }
        }
        // uma pergunta (a desta fila ou outra) está aberta: espera a resposta
        if self.confirmacao.is_some() {
            return;
        }
        while let Some(id) = fila.restantes.pop_front() {
            if fila.respondidas.contains(&id) {
                continue;
            }
            let Some(aba) = self.abas.get(id).filter(|a| a.conteudo.alterado()) else { continue };
            let (nome, _) = aba.conteudo.titulo(aba.rota(), aba.numero_consulta(), &self.estado);
            let alteracoes = aba.conteudo.doc_ref().alteracoes();
            self.confirmacao = Some(
                Confirmacao::new(frase_salvar_alteracoes(alteracoes))
                    .titulo(titulo_salvar_alteracoes(&nome))
                    .rotulo("Salvar")
                    .descartar(NAO_SALVAR),
            );
            self.confirmado = Some(Confirmado::SalvarAlterado(id));
            fila.perguntando = Some(id);
            // a aba da pergunta vai à frente
            self.abas.focar(id);
            ctx.request_repaint();
            return;
        }
        // antes de fechar, de novo: uma aba do destino alterada no meio (enquanto uma gravação era esperada) pergunta
        // também
        let destino = fila.destino.clone();
        let faltam: VecDeque<IdAba> = self
            .abas_do_destino(&destino)
            .into_iter()
            .filter(|id| self.fila.as_ref().is_some_and(|f| !f.respondidas.contains(id)))
            .collect();
        if !faltam.is_empty() {
            if let Some(f) = self.fila.as_mut() {
                f.restantes = faltam;
            }
            ctx.request_repaint();
            return;
        }
        let Some(FilaDeSalvar { destino, .. }) = self.fila.take() else { return };
        match destino {
            Destino::Aba(id) => self.fechar_aba(id),
            Destino::Pasta(chave) => {
                // outra pasta abriu enquanto a fila andava: a fila era da de antes, e nada fecha
                if self.estado.pasta.as_ref().is_some_and(|p| p.chave == chave) {
                    self.fechar_pasta(ctx);
                }
            }
            Destino::Janela => {
                let (exportando, transacao) = (self.exportando(), self.estado.transacao_aberta());
                if exportando || transacao {
                    self.perguntar_ao_fechar(exportando, transacao);
                } else {
                    self.pode_fechar = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint();
    }

    /// Executa um verbo no documento da aba `id` fora do caminho da barra (o Salvar da fila ao fechar); o que ele pede
    /// à casca vale no fim do próximo quadro, marcado com a aba.
    fn executar_na_aba(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, id: IdAba, verbo: Verbo) {
        let fontes = self.fontes_de_colunas();
        let (versao, trocando) = (self.versao, self.troca.is_some());
        let Editor { servico, estado, abas, pendentes, .. } = self;
        let Some(aba) = abas.get_mut(id) else { return };
        let mut emitidas = Vec::new();
        let mut contexto =
            Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando).com_fontes(&fontes);
        aba.conteudo.executar(verbo, ctx, &mut contexto);
        pendentes.extend(emitidas.into_iter().map(|a| (Some(id), a)));
        ctx.request_repaint();
    }

    /// As origens de colunas das abas abertas (o "Trazer colunas"), na ordem das abas: uma `Arc` por aba, montadas a
    /// cada quadro para o `Contexto::fontes`.
    fn fontes_de_colunas(&self) -> Vec<Arc<dyn FonteDeColunas>> {
        if !matches!(self.situacao, Situacao::Pronta) {
            return Vec::new();
        }
        self.abas.iter().filter_map(|a| a.conteudo.doc_ref().fonte_de_colunas()).collect()
    }

    /// O "Trazer colunas desta planilha" (o menu do cabeçalho de uma planilha): foca o CSV ativo, ou o usado mais
    /// recentemente, com a janela Trazer colunas aberta e a planilha (`rotulo`, a `FonteDeColunas` dela) como origem.
    /// Sem CSV aberto não há o que fazer (o item do menu fica desligado com "Abra um CSV para trazer colunas.").
    fn trazer_colunas_de(&mut self, ctx: &egui::Context, rotulo: String) {
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
    fn fechar_aba(&mut self, id: IdAba) {
        if let Some(aba) = self.abas.fechar(id, |id, r| Documento::novo(id, r, None)) {
            let id_banco = self.estado.banco_id();
            self.soltar(aba.conteudo, &id_banco);
        }
    }

    /// O `Substituir` (`nova_entrada` false) ou o `Atualizar` de uma aba: só o endereço muda, sem refazer o
    /// documento. Vale para a aba de origem (sem ela, a ativa), se o documento dela servir para a rota; se a chave
    /// nova for a de outra aba aberta (uma visão pedida como tabela, que a tela corrigiu), as duas viram uma e a de
    /// origem fecha.
    fn trocar_endereco(&mut self, origem: Option<IdAba>, rota: Rota, nova_entrada: bool) {
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
    fn marcar_para_reler(&mut self, ctx: &egui::Context, origem: Option<IdAba>) {
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
    fn reler_a_ativa(&mut self, ctx: &egui::Context, pedida: Option<IdAba>) {
        if let Some(aba) = pedida.and_then(|id| self.abas.get_mut(id)) {
            aba.conteudo.marcar_para_reler();
            ctx.request_repaint();
        }
    }

    /// Chegou uma lista de objetos nova do mesmo banco: fecham as abas das tabelas e visões que sumiram dela
    /// (renomeadas ou excluídas), menos a ativa, que relê e mostra o card de que a tabela não existe mais.
    fn trocar_objetos(&mut self, ctx: &egui::Context, objetos: ObjetosInfo) {
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
    fn notar_transacao(&mut self, ctx: &egui::Context, origem: Option<IdAba>, aberta: bool) {
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
    fn pedir_banco(&mut self, ctx: &egui::Context) {
        let id = self.estado.banco_id();
        let numero = self.leituras_banco;
        self.leituras_banco += 1;
        self.tarefas.push(Tarefa::Banco(id, numero, self.tarefa(ctx, |s, id| s.info_banco(id))));
    }

    /// O nome da Consulta que abriu a transação, como está na faixa de abas ("Consulta 2"); "Consulta SQL" quando
    /// nenhuma aba a abriu (o aviso e a dica do status citam este nome).
    fn nome_da_consulta_da_transacao(&self) -> String {
        let numero = self.consulta_da_transacao.and_then(|id| self.abas.get(id)).and_then(|a| a.numero_consulta());
        casca::nome_da_consulta(numero)
    }

    /// A aba desta chave.
    fn documento_da_chave(&mut self, chave: &ChaveAba) -> Option<&mut Documento> {
        let id = self.abas.achar(chave)?;
        self.abas.get_mut(id).map(|a| &mut a.conteudo)
    }

    // ------------------------------------------------------------------ tarefas

    fn iniciar(&mut self, ctx: &egui::Context) {
        self.iniciado = true;
        let partida = self.partida.take().unwrap_or_default();
        let servico = Arc::clone(&self.servico);
        self.carga = Some(Promessa::rodar(ctx, move || {
            let mut erro_partida = None;
            let mut depois = None;
            if let Some(caminho) = &partida.caminho {
                match servico.alvo_do_caminho(caminho) {
                    // a pasta e o arquivo de dados abrem depois de o estado chegar (a casca os trata)
                    Ok(Natureza::Pasta) => depois = Some(DepoisDaPartida::Pasta(caminho.clone())),
                    Ok(Natureza::Dados(_)) => depois = Some(DepoisDaPartida::Arquivo(caminho.clone())),
                    // o banco, e o que não se sabe o que é: abre como banco, e o erro do SQLite diz por quê
                    Ok(Natureza::Banco) | Err(_) => {
                        if let Err(e) = servico.abrir_banco(caminho, partida.somente_leitura, false) {
                            log::warn!("Não foi possível abrir {} ao iniciar: {}", caminho.display(), e.mensagem);
                            erro_partida = Some(erro_da_partida(caminho, &e.mensagem));
                        }
                    }
                }
            }
            // a pasta que estava aberta volta (nunca o banco); a leitura dela é pedida sem esperar
            let pasta_aberta = servico.prefs().pasta_aberta().map(PathBuf::from);
            let (estado, aviso) = carregar_estado(&servico)?;
            Ok(Carga { estado, aviso, erro_partida, pasta_aberta, depois })
        }));
    }

    fn tarefa<T: Send + 'static>(
        &self,
        ctx: &egui::Context,
        f: impl FnOnce(&Servico, &str) -> Resultado<T> + Send + 'static,
    ) -> Promessa<T> {
        let servico = Arc::clone(&self.servico);
        let id = self.estado.banco_id();
        Promessa::rodar(ctx, move || f(&servico, &id))
    }

    /// Mostra o erro de uma leitura da casca: `Substituido` some, `BancoTrocado` ressincroniza, o resto é toast.
    fn erro_da_casca(&mut self, ctx: &egui::Context, e: &ErroBanco) {
        if e.tem_codigo(Codigo::Substituido) {
            return;
        }
        if e.tem_codigo(Codigo::BancoTrocado) {
            self.ressincronizar(ctx);
            return;
        }
        toast_erro(ctx, e.mensagem.clone());
    }

    fn receber(&mut self, ctx: &egui::Context) {
        // o estado inicial ou a ressincronização
        if let Some(r) = self.carga.as_mut().and_then(Promessa::tomar) {
            self.carga = None;
            let ressincronizando = self.ressincronizando.take();
            match r {
                Ok(carga) => {
                    let anterior = ressincronizando.clone().flatten();
                    // a execução que o documento refeito solta foi pedida ao banco de antes
                    let id_antes = self.estado.banco_id();
                    // etapa 4, passo 9: a pasta não é do banco e continua aberta
                    let pasta = self.estado.pasta.take();
                    self.estado = carga.estado;
                    self.estado.pasta = pasta;
                    self.falha_objetos = carga.aviso.clone();
                    self.mudou_estado(ctx);
                    if let Some(a) = carga.aviso {
                        toast_erro(ctx, a);
                    }
                    if !self.carregado {
                        self.carregado = true;
                        self.erro_inicial = carga.erro_partida;
                        let rota = if self.estado.tem_banco() { Rota::Banco } else { Rota::Inicio };
                        self.reiniciar_abas(rota, &id_antes);
                        // a pasta da linha de comando vale pela que estava aberta; a que estava aberta volta (a
                        // leitura da raiz é pedida agora, sem esperar)
                        match carga.depois {
                            Some(DepoisDaPartida::Pasta(c)) => self.abrir_pasta(ctx, c, Origem::Partida),
                            outro => {
                                if let Some(p) = carga.pasta_aberta {
                                    self.restaurar_pasta(ctx, p);
                                }
                                if let Some(DepoisDaPartida::Arquivo(c)) = outro {
                                    self.abrir_arquivo_de_dados(ctx, c, Origem::Partida);
                                }
                            }
                        }
                    } else if ressincronizando.is_some() {
                        // uma janela aberta no meio da releitura (no mesmo quadro do pedido que voltou
                        // `BancoTrocado`, por exemplo) também era do estado de antes
                        self.janelas.fechar_todas();
                        // o mesmo arquivo: as abas ficam, com os documentos refeitos; outro: a visão geral dele;
                        // nenhum: o Início
                        let mesmo = match (&anterior, &self.estado.banco) {
                            (Some(a), Some(b)) => prefs::chave_caminho(a) == prefs::chave_caminho(&b.caminho),
                            _ => false,
                        };
                        if mesmo {
                            self.refazer_documentos(&id_antes);
                        } else {
                            let rota = if self.estado.tem_banco() { Rota::Banco } else { Rota::Inicio };
                            self.reiniciar_abas(rota, &id_antes);
                        }
                    }
                }
                Err(e) => {
                    if !self.carregado {
                        self.carregado = true;
                        self.situacao = Situacao::Falha(e.mensagem);
                    } else {
                        toast_erro(ctx, e.mensagem);
                    }
                }
            }
        }

        // a troca de banco
        if let Some(r) = self.troca.as_mut().and_then(|t| t.promessa.tomar())
            && let Some(TrocaPendente { troca, origem, .. }) = self.troca.take()
        {
            self.fim_da_troca(ctx, troca, origem, r);
        }

        // as leituras menores
        let mut i = 0;
        while i < self.tarefas.len() {
            let fim = match &mut self.tarefas[i] {
                Tarefa::Objetos(id, p) => p.tomar().map(|r| FimTarefa::Objetos(id.clone(), r)),
                Tarefa::Banco(id, n, p) => p.tomar().map(|r| FimTarefa::Banco(id.clone(), *n, r)),
                Tarefa::Prefs(p) => p.tomar().map(FimTarefa::Prefs),
                Tarefa::Tema { promessa, limpou_historico } => {
                    let limpou = *limpou_historico;
                    promessa.tomar().map(|r| FimTarefa::Tema(r, limpou))
                }
                Tarefa::Larguras(p) => p.tomar().map(FimTarefa::Larguras),
                Tarefa::Diretorio(p) => p.tomar().map(|_| FimTarefa::Nada),
                Tarefa::Consulta(id, p) => p.tomar().map(|r| FimTarefa::Consulta(id.clone(), r)),
                Tarefa::VisaoGeral(id, e) => e.tomar_solta().map(|r| FimTarefa::VisaoGeral(id.clone(), r)),
                Tarefa::Revelar(p) => p.tomar().map(FimTarefa::Revelar),
                Tarefa::Pasta { raiz, no, geracao, promessa } => promessa.tomar().map(|r| FimTarefa::Pasta {
                    raiz: raiz.clone(),
                    no: no.clone(),
                    geracao: *geracao,
                    r,
                }),
                Tarefa::AbrirPasta { origem, caminho, geracao, promessa } => promessa
                    .tomar()
                    .map(|r| FimTarefa::AbrirPasta { origem: *origem, caminho: caminho.clone(), geracao: *geracao, r }),
                Tarefa::Alvo { origem, caminho, somente_leitura, promessa } => promessa.tomar().map(|r| {
                    FimTarefa::Alvo { origem: *origem, caminho: caminho.clone(), somente_leitura: *somente_leitura, r }
                }),
                Tarefa::Arquivo { origem, caminho, da_pasta, promessa } => promessa
                    .tomar()
                    .map(|r| FimTarefa::Arquivo { origem: *origem, caminho: caminho.clone(), da_pasta: *da_pasta, r }),
                Tarefa::Gravar(p) => p.tomar().map(|_| FimTarefa::Nada),
                Tarefa::Soltar { quantos, promessa } => {
                    let quantos = *quantos;
                    promessa.tomar().map(|r| match r {
                        Ok((caminho, r)) => FimTarefa::Soltar { quantos, caminho, r },
                        Err(e) => FimTarefa::Soltar { quantos, caminho: PathBuf::new(), r: Err(e) },
                    })
                }
            };
            match fim {
                Some(fim) => {
                    self.tarefas.remove(i);
                    self.fim_da_tarefa(ctx, fim);
                }
                None => i += 1,
            }
        }
    }

    /// As abas fora de vista não são desenhadas; elas recebem só o `em_segundo_plano` do documento: as Consultas SQL
    /// escondidas tomam as respostas que chegam (a execução que segue com a aba escondida avisa a transação e os
    /// objetos na hora). Parado, nada pede quadro.
    fn documentos_em_segundo_plano(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) {
        if !matches!(self.situacao, Situacao::Pronta) {
            return;
        }
        let ativa = self.abas.id_ativa();
        let (versao, trocando) = (self.versao, self.troca.is_some());
        let Editor { servico, estado, abas, .. } = self;
        for aba in abas.iter_mut().filter(|a| a.id() != ativa) {
            let mut emitidas = Vec::new();
            let mut contexto =
                Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando).com_fontes(fontes);
            aba.conteudo.em_segundo_plano(ctx, &mut contexto);
            let id = aba.id();
            acoes.extend(emitidas.into_iter().map(|a| (Some(id), a)));
        }
    }

    fn fim_da_tarefa(&mut self, ctx: &egui::Context, fim: FimTarefa) {
        match fim {
            // a resposta de um pedido feito para um banco que já não é o aberto não vale mais (nem o erro)
            FimTarefa::Objetos(id, r) => match r {
                _ if id != self.estado.banco_id() => {}
                Ok(o) => {
                    self.falha_objetos = None;
                    self.trocar_objetos(ctx, o);
                    self.mudou_estado(ctx);
                }
                // releitura em silêncio: o Explorador fica
                // com a lista anterior; só o banco trocado ressincroniza. Sem lista nenhuma (a primeira leitura
                // falhou), o Explorador mostra o motivo e o "Tentar de novo": sem isso ficaria em "Carregando…" para
                // sempre, repintando
                Err(e) if e.tem_codigo(Codigo::BancoTrocado) => self.ressincronizar(ctx),
                Err(e) => {
                    log::warn!("Não foi possível reler os objetos do banco: {e}");
                    if self.estado.objetos.is_none() {
                        self.falha_objetos = Some(e.mensagem);
                        self.mudou_estado(ctx);
                    }
                }
            },
            FimTarefa::Banco(id, numero, r) => match r {
                _ if id != self.estado.banco_id() => {}
                // uma leitura mais velha que outra já aplicada (voltaram fora de ordem) não desfaz a mais nova
                Ok(_) if numero < self.banco_aplicada => {}
                Ok(mut b) if b.id == id => {
                    self.banco_aplicada = numero;
                    // pedida antes da última mudança de transação: o resto vale, a transação é a que a nota disse
                    if numero < self.banco_nota
                        && let Some(atual) = self.estado.banco.as_ref()
                    {
                        b.transacao_aberta = atual.transacao_aberta;
                    }
                    self.estado.banco = Some(b);
                    self.mudou_estado(ctx);
                }
                Ok(_) => {}
                Err(e) => self.erro_da_casca(ctx, &e),
            },
            FimTarefa::Prefs(r) => match r {
                Ok((prefs, ultimo)) => {
                    self.estado.prefs = prefs;
                    if ultimo.is_some() {
                        self.estado.ultimo_diretorio = ultimo;
                    }
                    self.mudou_estado(ctx);
                }
                Err(e) => self.erro_da_casca(ctx, &e),
            },
            FimTarefa::Tema(r, limpou_historico) => {
                // o histórico foi limpo no arquivo só agora: a lista à vista (ou a próxima leitura) é de antes
                if limpou_historico {
                    self.casca.historico.marcar_velha();
                    ctx.request_repaint();
                }
                match r {
                    Ok(prefs) => {
                        self.estado.prefs = prefs;
                        self.mudou_estado(ctx);
                    }
                    Err(e) => toast_erro(ctx, e.mensagem),
                }
            }
            // a manutenção ou a cópia que ficou correndo sem a aba: o resultado, num toast. O erro do banco que já
            // não é o aberto não vale; o sucesso (o arquivo da cópia existe) vale sempre
            FimTarefa::VisaoGeral(id, r) => match r {
                Ok((r, escreveu)) => {
                    avisar_resultado(ctx, &r);
                    if escreveu && id == self.estado.banco_id() {
                        self.pendentes.push((None, Acao::RecarregarBanco));
                    }
                    // a cópia de segurança criou um arquivo, talvez na pasta aberta: a árvore relê
                    if matches!(r, ResultadoDaTela::Copia { .. }) {
                        self.reler_a_pasta(ctx);
                    }
                    ctx.request_repaint();
                }
                Err(_) if id != self.estado.banco_id() => {}
                Err(e) => self.erro_da_casca(ctx, &e),
            },
            // a gravação das larguras terminou: se a pessoa mexeu de novo enquanto ela corria, a próxima sai agora
            FimTarefa::Larguras(r) => {
                self.gravando_larguras = false;
                match r {
                    Ok(prefs) => {
                        self.estado.prefs = prefs;
                        self.mudou_estado(ctx);
                        self.guardar_larguras(ctx);
                    }
                    Err(e) => toast_erro(ctx, e.mensagem),
                }
            }
            // a execução que ficou correndo: a transação e, se o comando mudou dados ou estrutura (ou fechou a
            // transação: um COMMIT ou um ROLLBACK), os objetos e as abas; uma falha não aparece fora da tela (lá também
            // não), só o banco trocado ressincroniza
            FimTarefa::Consulta(id, r) => match r {
                // a resposta é do banco de antes de uma troca: não vale para o aberto agora
                _ if id != self.estado.banco_id() => {}
                Ok(r) => {
                    let fechou_a_transacao = self.estado.transacao_aberta() && !r.transacao_aberta;
                    self.pendentes.push((None, Acao::NotarTransacao(r.transacao_aberta)));
                    if r.alterou_dados || r.alterou_esquema || fechou_a_transacao {
                        self.pendentes.push((None, Acao::RecarregarObjetos));
                    }
                    ctx.request_repaint();
                }
                Err(e) if e.tem_codigo(Codigo::BancoTrocado) => self.ressincronizar(ctx),
                Err(_) => {}
            },
            FimTarefa::Revelar(r) => {
                if let Err(e) = r {
                    self.erro_da_casca(ctx, &e);
                }
            }
            FimTarefa::Pasta { raiz, no, geracao, r } => {
                if geracao == self.geracao_pasta {
                    self.chegou_a_pasta(ctx, &raiz, no, r);
                }
            }
            // a pasta pedida antes de um Fechar pasta: o fechar vale (a pessoa fechou depois de pedir), e a chave
            // `pasta_aberta` que a abertura gravou sai de novo
            FimTarefa::AbrirPasta { origem, geracao, r: Ok(_), .. } if geracao != self.geracao_pasta => {
                if origem == Origem::Janela {
                    self.janelas.caminho = None;
                }
                let promessa = self.tarefa(ctx, |s, _| {
                    s.fechar_pasta();
                    Ok(())
                });
                self.tarefas.push(Tarefa::Gravar(promessa));
            }
            FimTarefa::AbrirPasta { origem, caminho, r, .. } => match r {
                Ok((listagem, prefs)) => {
                    self.estado.prefs = prefs;
                    self.por_a_pasta(listagem);
                    if origem == Origem::Janela {
                        self.janelas.caminho = None;
                    }
                    self.mudou_estado(ctx);
                }
                Err(e) => {
                    let mensagem = if origem == Origem::Partida {
                        erro_da_partida_pasta(&caminho, &e.mensagem)
                    } else {
                        e.mensagem
                    };
                    self.avisar_erro(ctx, origem, mensagem);
                }
            },
            FimTarefa::Alvo { origem, caminho, somente_leitura, r } => {
                self.abrir_pelo_alvo(ctx, caminho, somente_leitura, origem, r)
            }
            FimTarefa::Soltar { quantos, caminho, r } => {
                if quantos > 1 && r.is_ok() {
                    let nome = caminho
                        .file_name()
                        .map_or_else(|| caminho.display().to_string(), |n| n.to_string_lossy().into_owned());
                    toast_atencao(ctx, format!("Só um item abre por vez; abri {nome}."));
                }
                self.abrir_pelo_alvo(ctx, caminho, false, Origem::Casca, r)
            }
            FimTarefa::Arquivo { origem, caminho, da_pasta, r } => match r {
                Ok((info, prefs)) => {
                    self.estado.prefs = prefs;
                    if origem == Origem::Janela {
                        self.janelas.caminho = None;
                    }
                    self.mudou_estado(ctx);
                    // o arquivo era da pasta que fechou enquanto ele era registrado: a ficha não abre (o Fechar pasta
                    // fechou as de dentro dela); o recente fica
                    let pasta_fechou = da_pasta.is_some_and(|g| g != self.geracao_pasta)
                        && !self.estado.pasta.as_ref().is_some_and(|p| p.contem(&info.caminho));
                    if !pasta_fechou {
                        self.navegar(Rota::Arquivo { caminho: info.caminho });
                    }
                }
                Err(e) => {
                    let mensagem = if origem == Origem::Partida {
                        erro_da_partida_arquivo(&caminho, &e.mensagem)
                    } else {
                        e.mensagem
                    };
                    self.avisar_erro(ctx, origem, mensagem);
                }
            },
            FimTarefa::Nada => {}
        }
    }

    /// O erro de uma abertura vai para quem a pediu: a janela "Pedir o caminho" (se ainda aberta), o Início (mesmo
    /// fora de vista; o da linha de comando espera o Início abrir), ou o toast.
    fn avisar_erro(&mut self, ctx: &egui::Context, origem: Origem, mensagem: String) {
        ctx.request_repaint();
        let pronta = matches!(self.situacao, Situacao::Pronta);
        let inicio_aberto = pronta && self.abas.achar(&ChaveAba::Inicio).is_some();
        match origem {
            Origem::Janela if self.janelas.caminho.is_some() => {
                if let Some(j) = self.janelas.caminho.as_mut() {
                    j.mostrar_erro(mensagem);
                }
            }
            Origem::Inicio | Origem::Partida if inicio_aberto => {
                if let Some(Documento::Inicio(t)) = self.documento_da_chave(&ChaveAba::Inicio) {
                    t.mostrar_erro(mensagem);
                }
            }
            Origem::Partida => self.erro_inicial = Some(mensagem),
            _ => toast_erro(ctx, mensagem),
        }
    }

    // ------------------------------------------------------------------ a pasta (etapa 4)

    /// Há uma leitura de pasta em andamento (a raiz, uma subpasta ou a abertura de uma pasta).
    fn lendo_pasta(&self) -> bool {
        self.tarefas.iter().any(|t| matches!(t, Tarefa::Pasta { .. } | Tarefa::AbrirPasta { .. }))
    }

    /// Abre uma pasta numa tarefa (`Servico::abrir_pasta`: confere, lê a raiz, entra nos recentes e fica como a
    /// pasta aberta nas preferências). A pasta aberta de antes continua à vista até a nova chegar; uma abertura de
    /// cada vez (outra pedida no meio é ignorada).
    fn abrir_pasta(&mut self, ctx: &egui::Context, caminho: PathBuf, origem: Origem) {
        if !self.carregado || self.tarefas.iter().any(|t| matches!(t, Tarefa::AbrirPasta { .. })) {
            if origem == Origem::Janela
                && let Some(j) = self.janelas.caminho.as_mut()
            {
                j.desocupar();
            }
            return;
        }
        let c = caminho.clone();
        let promessa = self.tarefa(ctx, move |s, _| {
            let listagem = s.abrir_pasta(&c)?;
            Ok((listagem, s.prefs().resumo()))
        });
        let geracao = self.geracao_pasta;
        self.tarefas.push(Tarefa::AbrirPasta { origem, caminho, geracao, promessa });
    }

    /// A pasta que abriu: vira a pasta aberta, com a raiz já lida. A mesma pasta de novo só troca a raiz (as
    /// subpastas lidas ficam).
    fn por_a_pasta(&mut self, listagem: Listagem) {
        let chave = prefs::chave_caminho(&listagem.pasta);
        match self.estado.pasta.as_mut() {
            Some(p) if p.chave == chave => {
                p.nos.insert(chave, NoDaPasta::Pronto(Arc::new(listagem)));
            }
            _ => {
                // outra pasta: as leituras da de antes que ainda correm não valem
                self.geracao_pasta += 1;
                let mut p = PastaAberta::nova(listagem.pasta.clone());
                p.nos.insert(chave, NoDaPasta::Pronto(Arc::new(listagem)));
                self.estado.pasta = Some(p);
            }
        }
    }

    /// A pasta que estava aberta na sessão anterior (as preferências): volta com a raiz lendo, e a leitura é pedida
    /// agora. A que sumiu fica no Explorador com "Não foi possível ler a pasta." e o "Tentar de novo" (a chave fica).
    fn restaurar_pasta(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        self.geracao_pasta += 1;
        self.estado.pasta = Some(PastaAberta::nova(caminho.clone()));
        self.mudou_estado(ctx);
        self.listar_pasta(ctx, caminho);
    }

    /// Lê uma pasta da árvore (a raiz ou uma subpasta da pasta aberta) numa tarefa. A pasta ainda não lida (ou que
    /// falhou) mostra "Carregando…" enquanto isso; a já lida continua à vista, e a leitura nova reaproveita os
    /// cabeçalhos dela. Uma leitura da mesma pasta já em andamento não é pedida de novo.
    fn listar_pasta(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        let no = prefs::chave_caminho(&caminho);
        let Some(p) = self.estado.pasta.as_mut() else { return };
        let raiz = p.chave.clone();
        let em_andamento =
            self.tarefas.iter().any(|t| matches!(t, Tarefa::Pasta { raiz: r, no: n, .. } if *r == raiz && *n == no));
        if em_andamento {
            return;
        }
        let anterior = match p.nos.get(&no) {
            Some(NoDaPasta::Pronto(l)) => Some(Arc::clone(l)),
            _ => None,
        };
        let marcar = anterior.is_none() && p.nos.get(&no) != Some(&NoDaPasta::Lendo);
        if marcar {
            p.nos.insert(no.clone(), NoDaPasta::Lendo);
        }
        self.leituras_de_pasta += 1;
        let promessa = self.tarefa(ctx, move |s, _| s.listar_pasta(&caminho, anterior.as_deref()));
        let geracao = self.geracao_pasta;
        self.tarefas.push(Tarefa::Pasta { raiz, no, geracao, promessa });
        if marcar {
            self.mudou_estado(ctx);
        }
    }

    /// Chegou a leitura de uma pasta: vai para `estado.pasta.nos`, a não ser que a pasta aberta tenha mudado nesse
    /// meio-tempo. Se nada mudou (a releitura do foco), o estado fica como está e nada repinta.
    fn chegou_a_pasta(&mut self, ctx: &egui::Context, raiz: &str, no: String, r: Resultado<Listagem>) {
        let Some(p) = self.estado.pasta.as_mut() else { return };
        if p.chave != raiz {
            return;
        }
        let novo = match r {
            Ok(l) => NoDaPasta::Pronto(Arc::new(l)),
            Err(e) => {
                log::warn!("Não foi possível ler a pasta: {e}");
                NoDaPasta::Falhou(e.mensagem)
            }
        };
        if p.nos.get(&no) == Some(&novo) {
            return;
        }
        p.nos.insert(no, novo);
        self.mudou_estado(ctx);
    }

    /// O `Acao::AtualizarPasta`: relê a raiz e as subpastas já lidas (o caminho delas sai da listagem de quem as
    /// contém). Sem pasta aberta, nada.
    fn atualizar_pasta(&mut self, ctx: &egui::Context) {
        let Some(p) = self.estado.pasta.as_ref() else { return };
        let subpastas: HashMap<&str, &Path> = p
            .nos
            .values()
            .filter_map(|no| match no {
                NoDaPasta::Pronto(l) => Some(l),
                _ => None,
            })
            .flat_map(|l| l.entradas.iter().filter(|e| e.natureza == Natureza::Pasta))
            .map(|e| (e.chave.as_str(), e.caminho.as_path()))
            .collect();
        let mut caminhos: Vec<PathBuf> = p
            .nos
            .iter()
            // as que falharam também (uma falha passageira da rede não fica até a pessoa abrir a pasta de novo)
            .filter(|(chave, no)| **chave != p.chave && matches!(no, NoDaPasta::Pronto(_) | NoDaPasta::Falhou(_)))
            .filter_map(|(chave, _)| subpastas.get(chave.as_str()).map(|c| c.to_path_buf()))
            .collect();
        caminhos.sort();
        caminhos.insert(0, p.caminho.clone());
        for c in caminhos {
            self.listar_pasta(ctx, c);
        }
    }

    /// A releitura de quando o próprio app pode ter criado arquivos na pasta (o fim de uma troca de banco, o Novo
    /// banco; o fim de uma janela, a Exportar; a cópia de segurança): sem o espaçamento da volta do foco; com uma
    /// leitura em andamento, fica para quando ela terminar ([`Editor::reler_pendente`]), sem se perder.
    fn reler_a_pasta(&mut self, ctx: &egui::Context) {
        if self.estado.pasta.is_none() {
            return;
        }
        if self.lendo_pasta() {
            self.reler_pendente = true;
            return;
        }
        self.reler_pendente = false;
        self.atualizar_pasta(ctx);
    }

    /// A releitura da volta do foco da janela: no máximo uma a cada [`INTERVALO_RELEITURA`] e nunca com uma leitura
    /// em andamento.
    fn releitura_automatica(&mut self, ctx: &egui::Context) {
        if self.estado.pasta.is_none() || self.lendo_pasta() {
            return;
        }
        let agora = ctx.input(|i| i.time);
        if self.ultima_releitura.is_some_and(|antes| agora - antes < INTERVALO_RELEITURA) {
            return;
        }
        self.ultima_releitura = Some(agora);
        self.atualizar_pasta(ctx);
    }

    /// O Fechar pasta: tira a árvore, fecha as abas de arquivo de dados de dentro da pasta (as de fora ficam) e apaga
    /// a pasta aberta das preferências numa tarefa. O banco continua aberto; os recentes ficam.
    fn fechar_pasta(&mut self, ctx: &egui::Context) {
        let Some(p) = self.estado.pasta.take() else { return };
        // uma abertura de pasta ainda a caminho não reabre a árvore depois deste fechar, e as leituras pedidas antes
        // dele não valem para a pasta aberta depois
        self.geracao_pasta += 1;
        self.reler_pendente = false;
        let dentro = |a: &estado::AbaAberta<Documento>| a.rota().caminho_arquivo().is_some_and(|c| p.contem(c));
        let fechadas = self.abas.fechar_onde(dentro, |id, r| Documento::novo(id, r, None));
        let id_banco = self.estado.banco_id();
        for aba in fechadas {
            self.soltar(aba.conteudo, &id_banco);
        }
        let promessa = self.tarefa(ctx, |s, _| {
            s.fechar_pasta();
            Ok(())
        });
        self.tarefas.push(Tarefa::Gravar(promessa));
        self.mudou_estado(ctx);
    }

    /// O `Acao::AbrirCaminho`: o serviço diz o que o caminho é (numa tarefa), e ele abre como pasta, banco ou arquivo
    /// de dados ([`FimTarefa::Alvo`]).
    fn abrir_caminho(&mut self, ctx: &egui::Context, caminho: PathBuf, somente_leitura: bool, origem: Origem) {
        if !self.carregado {
            return;
        }
        let c = caminho.clone();
        let promessa = self.tarefa(ctx, move |s, _| s.alvo_do_caminho(&c));
        self.tarefas.push(Tarefa::Alvo { origem, caminho, somente_leitura, promessa });
    }

    /// O caminho classificado abre pelo que ele é: a pasta, a ficha do arquivo de dados ou o banco.
    fn abrir_pelo_alvo(
        &mut self,
        ctx: &egui::Context,
        caminho: PathBuf,
        somente_leitura: bool,
        origem: Origem,
        r: Resultado<Natureza>,
    ) {
        match r {
            Ok(Natureza::Pasta) => self.abrir_pasta(ctx, caminho, origem),
            Ok(Natureza::Dados(_)) => self.abrir_arquivo_de_dados(ctx, caminho, origem),
            Ok(Natureza::Banco) => self.abrir_banco_de(ctx, caminho, somente_leitura, origem),
            // um banco pedido pelo nome que não abre: o erro do SQLite (ou o do arquivo que não existe) diz por quê
            Err(_) if tem_extensao_de_banco(&caminho) => self.abrir_banco_de(ctx, caminho, somente_leitura, origem),
            Err(e) => self.avisar_erro(ctx, origem, e.mensagem),
        }
    }

    /// Soltar arquivos ou pastas na janela (decisão 18, sem o vidro "Solte para abrir", que não tem prancheta): abre
    /// um item só, a primeira pasta ou, sem pasta, o primeiro que o app abre, como o caminho digitado (`AbrirCaminho`); com mais de
    /// um, o aviso "Só um item abre por vez; abri dados.". Com uma janela ou um modal aberto, nada abre. O que cada
    /// item é sai do serviço numa tarefa (a thread da interface não toca no disco).
    fn soltar_na_janela(&mut self, ctx: &egui::Context) {
        let soltos: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|c| !c.as_os_str().is_empty()).collect()
        });
        if soltos.is_empty() || !self.carregado || self.janela_aberta() {
            return;
        }
        if ctx.memory(|m| m.top_modal_layer().is_some()) {
            return;
        }
        let quantos = soltos.len();
        let servico = Arc::clone(&self.servico);
        let promessa = Promessa::rodar(ctx, move || {
            let alvos: Vec<(PathBuf, Resultado<Natureza>)> =
                soltos.into_iter().map(|c| (c.clone(), servico.alvo_do_caminho(&c))).collect();
            // a primeira pasta; sem pasta, o primeiro que o app abre (um PDF solto junto não esconde o CSV); sem
            // nenhum, o primeiro, com o erro dele
            let i = alvos
                .iter()
                .position(|(_, r)| matches!(r, Ok(Natureza::Pasta)))
                .or_else(|| alvos.iter().position(|(_, r)| r.is_ok()))
                .unwrap_or(0);
            alvos.into_iter().nth(i).ok_or_else(|| ErroBanco::novo("Nada foi solto na janela."))
        });
        self.tarefas.push(Tarefa::Soltar { quantos, promessa });
    }

    /// O `Acao::AbrirArquivoDeDados`: registra o recente (numa tarefa) e abre a ficha do arquivo (`Rota::Arquivo`),
    /// ou foca a que já está aberta.
    fn abrir_arquivo_de_dados(&mut self, ctx: &egui::Context, caminho: PathBuf, origem: Origem) {
        let c = caminho.clone();
        let promessa = self.tarefa(ctx, move |s, _| {
            let info = s.registrar_arquivo(&c)?;
            Ok((info, s.prefs().resumo()))
        });
        let da_pasta = self.estado.pasta.as_ref().filter(|p| p.contem(&caminho)).map(|_| self.geracao_pasta);
        self.tarefas.push(Tarefa::Arquivo { origem, caminho, da_pasta, promessa });
    }

    /// Abre um banco (a troca de sempre); a janela "Pedir o caminho" volta a aceitar o botão se a troca não começou.
    fn abrir_banco_de(&mut self, ctx: &egui::Context, caminho: PathBuf, somente_leitura: bool, origem: Origem) {
        if !self.iniciar_troca(ctx, Troca::Abrir { caminho, somente_leitura }, false, origem)
            && origem == Origem::Janela
            && let Some(j) = self.janelas.caminho.as_mut()
        {
            j.desocupar();
        }
    }

    /// O diálogo "Abrir pasta" (ou, sem ele, a janela "Pedir o caminho" com o propósito Pasta). A pasta de cima da
    /// escolhida vira a última usada, para o próximo diálogo abrir ao lado dela.
    fn escolher_e_abrir_pasta(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, origem: Origem) {
        if !self.carregado {
            return;
        }
        let pasta = self.estado.ultimo_diretorio.clone();
        let escolha = match self.dialogos.abrir_pasta(frame, pasta) {
            RespostaDialogo::Escolhido(escolha) => escolha,
            RespostaDialogo::Cancelado => return,
            RespostaDialogo::Indisponivel => {
                self.janelas.caminho = Some(PedirCaminho::pasta());
                return;
            }
        };
        if let Some(de_cima) = escolha.pasta {
            self.estado.ultimo_diretorio = Some(de_cima.clone());
            let p = self.tarefa(ctx, move |s, _| {
                s.lembrar_diretorio(&de_cima);
                Ok(())
            });
            self.tarefas.push(Tarefa::Diretorio(p));
        }
        self.abrir_pasta(ctx, escolha.caminho, origem);
    }

    /// O clique na ação do toast da cópia de segurança ("Mostrar na pasta"): abre o Explorador de Arquivos no arquivo
    /// (nos testes, o gancho que recebe o caminho no lugar dele).
    /// As ações dos toasts dos documentos (o "Marcar as linhas sem par" do CSV, com a carga
    /// `tela_csv::PREFIXO_ACAO_TOAST…`, e o "Desfazer · Ctrl+Z" da aba Dados, `tela_dados::PREFIXO_ACAO_TOAST…`)
    /// voltam para o documento, que as lê no quadro seguinte.
    fn cumprir_acao_do_toast(&mut self, ctx: &egui::Context) {
        let Some(caminho) = acao_do_toast(ctx) else { return };
        #[cfg(feature = "telas")]
        if caminho.starts_with(tela_csv::PREFIXO_ACAO_TOAST) || caminho.starts_with(tela_dados::PREFIXO_ACAO_TOAST) {
            interface::pecas::toast::devolver_acao_do_toast(ctx, caminho);
            return;
        }
        self.revelar(ctx, PathBuf::from(caminho));
    }

    /// O "Mostrar na pasta" (o do toast e o do menu de contexto do Explorador): abre o Explorador de Arquivos no
    /// arquivo numa tarefa; nos testes, chama o gancho com o caminho no lugar dele.
    fn revelar(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        if let Some(gancho) = self.abrir_de_teste.as_mut() {
            gancho(&caminho);
            return;
        }
        let promessa = self.tarefa(ctx, move |s, _| s.revelar(&caminho));
        self.tarefas.push(Tarefa::Revelar(promessa));
    }

    /// O "Abrir no programa padrão" da ficha do arquivo de dados: o programa associado abre numa tarefa; nos testes,
    /// o gancho recebe o caminho no lugar dele.
    fn abrir_no_programa(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        if let Some(gancho) = self.abrir_de_teste.as_mut() {
            gancho(&caminho);
            return;
        }
        let promessa = self.tarefa(ctx, move |s, _| s.abrir_arquivo(&caminho));
        self.tarefas.push(Tarefa::Revelar(promessa));
    }

    /// Começa uma troca de banco; false quando já há uma em andamento (ou o estado ainda não chegou).
    fn iniciar_troca(&mut self, ctx: &egui::Context, troca: Troca, descartar: bool, origem: Origem) -> bool {
        // uma troca de cada vez: não enfileira outra igual enquanto a primeira espera (o card no alto do
        // conteúdo mostra que ela está esperando)
        if self.troca.is_some() || !self.carregado {
            return false;
        }
        let servico = Arc::clone(&self.servico);
        let id = self.estado.banco_id();
        let t = troca.clone();
        let promessa = Promessa::rodar(ctx, move || {
            match &t {
                Troca::Abrir { caminho, somente_leitura } => {
                    servico.abrir_banco(caminho, *somente_leitura, descartar)?;
                }
                Troca::Novo { caminho } => {
                    servico.novo_banco(caminho, descartar)?;
                }
                Troca::Fechar => servico.fechar_banco(&id, descartar)?,
            }
            let (estado, aviso) = carregar_estado(&servico)?;
            Ok(Carga::de(estado, aviso))
        });
        let desde = ctx.input(|i| i.time);
        self.troca = Some(TrocaPendente { troca, origem, promessa, desde });
        // o card da troca aparece se ela demorar (a promessa acorda a interface quando terminar)
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(ESPERA_PARA_MOSTRAR));
        true
    }

    fn fim_da_troca(&mut self, ctx: &egui::Context, troca: Troca, origem: Origem, r: Resultado<Carga>) {
        match r {
            Ok(carga) => {
                self.janelas.fechar_todas();
                // uma releitura pedida durante a troca (um pedido que voltou `BancoTrocado` antes de ela
                // terminar) chegaria com o estado de antes e puxaria a tela de volta: a troca já trouxe o novo
                if self.ressincronizando.is_some() {
                    self.carga = None;
                    self.ressincronizando = None;
                }
                // a execução da Consulta SQL que ainda corre foi pedida ao banco de antes: a resposta dela não vale
                // para o novo
                let id_antes = self.estado.banco_id();
                // etapa 4, passo 9: a pasta não é do banco e continua aberta (o passo 8 a lê e a relê)
                let pasta = self.estado.pasta.take();
                self.estado = carga.estado;
                self.estado.pasta = pasta;
                self.falha_objetos = carga.aviso.clone();
                self.mudou_estado(ctx);
                if let Some(a) = carga.aviso {
                    toast_erro(ctx, a);
                }
                // as rotas antigas eram de outro banco
                let rota = if self.estado.tem_banco() { Rota::Banco } else { Rota::Inicio };
                self.reiniciar_abas(rota, &id_antes);
                // o Novo banco cria um arquivo na pasta: a árvore relê
                self.reler_a_pasta(ctx);
            }
            Err(e) if e.tem_codigo(Codigo::TransacaoAberta) => {
                self.confirmacao = Some(
                    Confirmacao::new(frase_descartar(troca.acao(), &self.nome_da_consulta_da_transacao()))
                        .titulo("Transação aberta")
                        .rotulo("Descartar e continuar")
                        .perigo(true),
                );
                self.confirmado = Some(Confirmado::Troca(troca, origem));
            }
            Err(e) if e.tem_codigo(Codigo::BancoTrocado) => self.ressincronizar(ctx),
            // o erro vai para quem pediu: a janela "Pedir o caminho" ou o Início (mesmo fora de vista); o resto é
            // toast
            Err(e) => self.avisar_erro(ctx, origem, e.mensagem),
        }
    }

    /// Outro lugar trocou o banco (um pedido voltou `BancoTrocado`): relê o estado, fecha as janelas e redesenha.
    /// Durante uma troca pedida aqui, não faz nada: os pedidos com o id antigo voltam `BancoTrocado` porque o
    /// serviço já trocou, e a própria troca traz o estado novo (sem o toast de "outra janela").
    fn ressincronizar(&mut self, ctx: &egui::Context) {
        if self.carga.is_some() || self.troca.is_some() {
            return;
        }
        let anterior = self.estado.banco.as_ref().map(|b| b.caminho.clone());
        self.janelas.fechar_todas();
        self.confirmacao = None;
        self.confirmado = None;
        toast_atencao(ctx, servico::MSG_BANCO_TROCADO);
        let servico = Arc::clone(&self.servico);
        self.ressincronizando = Some(anterior);
        self.carga = Some(Promessa::rodar(ctx, move || {
            let (estado, aviso) = carregar_estado(&servico)?;
            Ok(Carga::de(estado, aviso))
        }));
    }

    // ------------------------------------------------------------------ ações

    fn escolher_e_abrir(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        so_leitura: bool,
        origem: Origem,
    ) {
        if self.troca.is_some() || !self.carregado {
            return;
        }
        let pasta = self.estado.ultimo_diretorio.clone();
        let escolha = match self.dialogos.abrir_banco(frame, pasta) {
            RespostaDialogo::Escolhido(escolha) => escolha,
            RespostaDialogo::Cancelado => return,
            RespostaDialogo::Indisponivel => {
                self.janelas.caminho = Some(PedirCaminho::abrir(so_leitura));
                return;
            }
        };
        // a pasta do arquivo escolhido vira a última usada
        if let Some(p) = &escolha.pasta {
            self.estado.ultimo_diretorio = Some(p.clone());
        }
        let caminho = escolha.caminho.clone();
        let servico = Arc::clone(&self.servico);
        self.tarefas.push(Tarefa::Diretorio(Promessa::rodar(ctx, move || {
            servico.lembrar_diretorio(&caminho);
            Ok(())
        })));
        // o "Abrir arquivo" mostra também os arquivos de dados: o escolhido abre pelo que ele é
        self.abrir_caminho(ctx, escolha.caminho, so_leitura, origem);
    }

    fn novo_banco(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, origem: Origem) {
        if self.troca.is_some() || !self.carregado {
            return;
        }
        let pasta_banco = self.estado.banco.as_ref().map(|b| b.pasta.clone());
        let pasta = pasta_banco.clone().or_else(|| self.estado.ultimo_diretorio.clone());
        match self.dialogos.salvar_banco(frame, pasta, PedidoDialogo::NovoBanco, "novo.db") {
            RespostaDialogo::Escolhido(escolha) => {
                self.iniciar_troca(ctx, Troca::Novo { caminho: escolha.caminho }, false, origem);
            }
            RespostaDialogo::Cancelado => {}
            RespostaDialogo::Indisponivel => {
                self.janelas.caminho = Some(PedirCaminho::novo(pasta_banco.as_deref()));
            }
        }
    }

    /// O que a janela "Pedir o caminho" devolveu.
    fn caminho_digitado(&mut self, ctx: &egui::Context, saida: SaidaCaminho) {
        let Some(proposito) = self.janelas.caminho.as_ref().map(|j| j.proposito) else { return };
        let caminho = match saida {
            SaidaCaminho::Fechou => {
                self.janelas.caminho = None;
                return;
            }
            SaidaCaminho::Enviou(caminho) => caminho,
        };
        let troca = match proposito {
            // o caminho digitado abre pelo que ele é, como o do Início; o erro aparece na janela
            Proposito::Abrir { somente_leitura } => {
                self.abrir_caminho(ctx, caminho, somente_leitura, Origem::Janela);
                return;
            }
            Proposito::Pasta => {
                self.abrir_pasta(ctx, caminho, Origem::Janela);
                return;
            }
            Proposito::Novo => Troca::Novo { caminho },
            Proposito::Copia => {
                // o caminho volta à Visão geral, que faz a cópia (e pergunta antes de substituir); a janela é modal,
                // e nada fecha nem troca a aba no meio
                self.janelas.caminho = None;
                if let Some(Documento::Banco(t)) = self.documento_da_chave(&ChaveAba::VisaoGeral) {
                    t.copiar_para(caminho, false);
                    ctx.request_repaint();
                }
                return;
            }
        };
        if !self.iniciar_troca(ctx, troca, false, Origem::Janela)
            && let Some(j) = self.janelas.caminho.as_mut()
        {
            j.desocupar();
        }
    }

    /// A cópia de segurança pedida pela Visão geral (`Acao::EscolherCopia`): o diálogo "Salvar banco SQLite" na pasta
    /// do banco; o caminho escolhido volta à Visão geral (o Windows já perguntou antes de substituir). Sem o diálogo
    /// nativo, a janela "Pedir o caminho" ("Salvar como"), cujo caminho volta pelo [`Editor::caminho_digitado`].
    fn escolher_copia(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, nome: String) {
        let Some(pasta) = self.estado.banco.as_ref().map(|b| b.pasta.clone()) else { return };
        let pedido = PedidoDialogo::CopiaDeSeguranca { nome_sugerido: nome.clone() };
        match self.dialogos.salvar_banco(frame, Some(pasta.clone()), pedido, &nome) {
            RespostaDialogo::Escolhido(escolha) => {
                if let Some(Documento::Banco(t)) = self.documento_da_chave(&ChaveAba::VisaoGeral) {
                    t.copiar_para(escolha.caminho, true);
                    ctx.request_repaint();
                }
            }
            RespostaDialogo::Cancelado => {}
            RespostaDialogo::Indisponivel => self.janelas.caminho = Some(PedirCaminho::copia(&pasta, &nome)),
        }
    }

    /// Quem pediu a troca de banco: o Início (pela marca da ação: o erro volta ao card "Abrir um banco") ou a casca.
    fn origem_da_troca(&self, origem: Option<IdAba>) -> Origem {
        let do_inicio = origem.and_then(|id| self.abas.get(id)).is_some_and(|a| a.chave() == Some(&ChaveAba::Inicio));
        if do_inicio { Origem::Inicio } else { Origem::Casca }
    }

    /// Aplica as ações do quadro, cada uma com a aba que a emitiu (None: a casca, as janelas, os testes).
    fn aplicar(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>, acoes: Vec<(Option<IdAba>, Acao)>) {
        for (origem, acao) in acoes {
            match acao {
                Acao::Navegar(rota) => self.navegar(rota),
                // o endereço vai para a aba que o emitiu, mesmo que ela tenha saído de vista no mesmo quadro (o
                // adiamento da busca ou do texto da Consulta vencendo no quadro do clique em outra aba)
                Acao::Atualizar(rota) => self.trocar_endereco(origem, rota, true),
                Acao::Substituir(rota) => self.trocar_endereco(origem, rota, false),
                Acao::Voltar => {
                    self.abas.voltar();
                }
                Acao::Avancar => {
                    self.abas.avancar();
                }
                Acao::NovaConsulta { sql } => self.nova_consulta(sql),
                Acao::FocarAba(id) => {
                    self.abas.focar(id);
                }
                Acao::MoverAba { id, para } => {
                    self.abas.mover(id, para);
                }
                // a aba alterada pergunta antes (a fila de perguntas ao fechar, etapa 5)
                Acao::FecharAba(id) => {
                    let id = id.unwrap_or_else(|| self.abas.id_ativa());
                    self.pedir_fechar_aba(ctx, id);
                }
                Acao::AbrirBanco { caminho, somente_leitura } => {
                    let de = self.origem_da_troca(origem);
                    self.iniciar_troca(ctx, Troca::Abrir { caminho, somente_leitura }, false, de);
                }
                Acao::EscolherEAbrir { somente_leitura } => {
                    let de = self.origem_da_troca(origem);
                    self.escolher_e_abrir(ctx, frame, somente_leitura, de)
                }
                Acao::NovoBanco => {
                    let de = self.origem_da_troca(origem);
                    self.novo_banco(ctx, frame, de)
                }
                Acao::FecharBanco => {
                    if self.estado.tem_banco() {
                        self.iniciar_troca(ctx, Troca::Fechar, false, Origem::Casca);
                    }
                }
                Acao::RecarregarObjetos => {
                    if self.estado.tem_banco() {
                        let id = self.estado.banco_id();
                        // o "Tentar de novo" do Explorador: volta ao ponto pulsante enquanto a leitura corre
                        if self.falha_objetos.take().is_some() {
                            self.mudou_estado(ctx);
                        }
                        self.tarefas.push(Tarefa::Objetos(id, self.tarefa(ctx, |s, id| s.objetos(id))));
                        // a Visão geral pede ao abrir (e no Atualizar) só para mostrar a lista de agora: nada mudou no
                        // banco, e as outras abas não releem
                        let da_visao_geral = origem
                            .and_then(|id| self.abas.get(id))
                            .is_some_and(|a| matches!(a.conteudo, Documento::Banco(_)));
                        if !da_visao_geral {
                            self.marcar_para_reler(ctx, origem);
                        }
                    }
                }
                Acao::RecarregarBanco => {
                    if self.estado.tem_banco() {
                        self.pedir_banco(ctx);
                    }
                }
                Acao::RecarregarPrefs => {
                    let p = self
                        .tarefa(ctx, |s, _| Ok((s.prefs().resumo(), s.prefs().ultimo_diretorio().map(PathBuf::from))));
                    self.tarefas.push(Tarefa::Prefs(p));
                }
                Acao::NotarTransacao(aberta) => {
                    // toda resposta de uma Consulta (também a da execução solta, sem aba) avisa a transação: um
                    // comando pode ter entrado no histórico
                    let da_consulta = origem.is_none_or(|id| {
                        self.abas.get(id).is_some_and(|a| matches!(a.conteudo, Documento::Consulta(_)))
                    });
                    if da_consulta {
                        self.casca.historico.marcar_velha();
                    }
                    self.notar_transacao(ctx, origem, aberta)
                }
                Acao::BancoTrocado => self.ressincronizar(ctx),
                Acao::LembrarDiretorio(pasta) => {
                    self.estado.ultimo_diretorio = Some(pasta.clone());
                    let p = self.tarefa(ctx, move |s, _| {
                        s.lembrar_diretorio(&pasta);
                        Ok(())
                    });
                    self.tarefas.push(Tarefa::Diretorio(p));
                }
                Acao::TrocarTema(t) => {
                    self.tema = tema_da_interface(t);
                    tema::aplicar(ctx, self.tema);
                    self.estado.prefs.tema = t;
                    self.mudou_estado(ctx);
                    let promessa = self.tarefa(ctx, move |s, _| s.gravar_prefs(Some(t), None));
                    self.tarefas.push(Tarefa::Tema { promessa, limpou_historico: false });
                }
                Acao::MostrarPainel(painel) => {
                    self.paineis.mostrar(Painel::from(painel));
                    ctx.request_repaint();
                }
                Acao::DestacarPainel(painel) => self.mover_painel(ctx, Painel::from(painel), Lugar::Janela),
                Acao::AbrirPreferencias => self.abrir_preferencias(ctx, Secao::Geral),
                Acao::ManutencaoDoBanco(m) => self.pedir_a_visao_geral(PedidoDoMenu::Manutencao(m)),
                Acao::CopiaDeSeguranca => self.pedir_a_visao_geral(PedidoDoMenu::Copia),
                Acao::AbrirExportacao(pedido) => {
                    self.janelas.pedido_exportar = Some(pedido.clone());
                    self.janelas.exportar = Some(JanelaExportar::nova(pedido));
                }
                Acao::AbrirNovaTabela => self.janelas.nova_tabela = Some(JanelaNovaTabela::nova()),
                Acao::AbrirNovoIndice { tabela, colunas } => {
                    self.janelas.tabela_do_indice = tabela.clone();
                    self.janelas.novo_indice = Some(JanelaNovoIndice::nova(tabela, colunas))
                }
                Acao::EscolherCopia { nome_sugerido } => self.escolher_copia(ctx, frame, nome_sugerido),
                // a pasta e os arquivos de dados (etapa 4): o erro volta ao Início quando a ação veio dele
                Acao::AbrirCaminho { caminho, somente_leitura } => {
                    let de = self.origem_da_troca(origem);
                    self.abrir_caminho(ctx, caminho, somente_leitura, de);
                }
                Acao::EscolherEAbrirPasta => {
                    let de = self.origem_da_troca(origem);
                    self.escolher_e_abrir_pasta(ctx, frame, de);
                }
                Acao::AbrirPasta { caminho } => {
                    let de = self.origem_da_troca(origem);
                    self.abrir_pasta(ctx, caminho, de);
                }
                Acao::AbrirArquivoDeDados { caminho } => {
                    let de = self.origem_da_troca(origem);
                    self.abrir_arquivo_de_dados(ctx, caminho, de);
                }
                Acao::ListarPasta { caminho } => self.listar_pasta(ctx, caminho),
                Acao::AtualizarPasta => self.atualizar_pasta(ctx),
                Acao::FecharPasta => self.pedir_fechar_pasta(ctx),
            }
        }
    }

    // ------------------------------------------------------------------ teclado e fechar a janela

    /// Os atalhos da casca ([`casca::atalhos`]): Ctrl+O, Ctrl+Shift+O, Alt+←/→ e os botões laterais do mouse, Ctrl+W, Ctrl+Tab,
    /// Ctrl+Shift+Tab, F5, Ctrl+B, Ctrl+Alt+B e Ctrl+F. Não valem com uma janela ou modal aberta. O F5 só vale com o
    /// Atualizar ligado na aba (`ofertas`), o Ctrl+F, com o Buscar, e o Ctrl+Alt+B, com o botão Painel de propriedades
    /// ligado (`painel`): fora disso, as teclas ficam com quem tem o foco. O Ctrl+B e o Ctrl+Alt+B mostram ou escondem
    /// o Explorador e as Propriedades na hora, sem tirar o foco de ninguém (o Ctrl+Alt+B fica com o campo de texto
    /// focado), e o Ctrl+F leva o foco à busca da barra, com o texto dela selecionado. Nos outros, a ação (e o
    /// Atualizar do F5) fica para o quadro seguinte e a edição de texto em curso termina agora: a célula da grade em
    /// edição vê o campo perder o foco neste quadro e grava antes de a aba sair de vista ou ser relida.
    fn atalhos_de_teclado(
        &mut self,
        ctx: &egui::Context,
        ui: &Ui,
        ofertas: &Ofertas,
        painel: EstadoPainel,
    ) -> Option<Verbo> {
        // uma janela aberta tem prioridade
        let modal = self.janela_aberta() || ctx.memory(|m| m.top_modal_layer().is_some());
        if modal {
            return None;
        }
        let valem = casca::atalhos::Valem {
            campo_de_texto: ctx.text_edit_focused(),
            tabela: matches!(self.situacao, Situacao::Pronta)
                && matches!(self.abas.ativa().conteudo, Documento::Tabela(_)),
            atualizar: ofertas.ligado(Verbo::Atualizar),
            propriedades: painel.ligado(),
            buscar: ofertas.ligado(Verbo::Buscar),
            salvar: ofertas.ligado(Verbo::Salvar),
            desfazer: ofertas.ligado(Verbo::Desfazer),
            refazer: ofertas.ligado(Verbo::Refazer),
        };
        let a = casca::atalhos::ler(ctx, valem);
        if a.explorador {
            self.paineis.alternar(Painel::Explorador);
        }
        if a.propriedades {
            self.paineis.alternar(Painel::Propriedades);
        }
        if a.buscar {
            self.focar_busca(ctx);
        }
        // Ctrl+S, Ctrl+Z e Ctrl+Y: o verbo vai ao documento neste quadro, antes de ele se desenhar
        let verbo = a.verbo();
        if !a.sai_do_documento() {
            return verbo;
        }
        if a.atualizar {
            self.verbo_pendente = Some(Verbo::Atualizar);
        }
        if a.inicio {
            self.pendentes.push((None, Acao::Navegar(Rota::Inicio)));
        }
        if let Some(n) = a.vista {
            let parte = estado::Parte::DA_TABELA[usize::from(n - 1)];
            self.item_da_tabela(ctx, ItemTabela::Vista(parte));
        }
        if a.abrir {
            casca::menus::fechar_todos(ui);
            self.pendentes.push((None, Acao::EscolherEAbrir { somente_leitura: false }));
        }
        if a.abrir_pasta {
            casca::menus::fechar_todos(ui);
            self.pendentes.push((None, Acao::EscolherEAbrirPasta));
        }
        if a.preferencias {
            casca::menus::fechar_todos(ui);
            self.abrir_preferencias(ctx, Secao::Geral);
        }
        if a.voltar {
            self.pendentes.push((None, Acao::Voltar));
        }
        if a.avancar {
            self.pendentes.push((None, Acao::Avancar));
        }
        if a.fechar_aba {
            self.pendentes.push((None, Acao::FecharAba(Some(self.abas.id_ativa()))));
        }
        // a aba seguinte e a anterior na faixa, com volta
        for (pedida, seguinte) in [(a.proxima, true), (a.anterior, false)] {
            if let Some(id) = self.abas.vizinha(seguinte).filter(|_| pedida) {
                self.pendentes.push((None, Acao::FocarAba(id)));
            }
        }
        // a ativa uma casa para a esquerda ou para a direita, sem volta
        let id = self.abas.id_ativa();
        if let Some(i) = self.abas.posicao_na_faixa(id) {
            let para = match (a.mover_esquerda, a.mover_direita) {
                (true, _) => i.checked_sub(1),
                (_, true) => (i + 1 < self.abas.len()).then_some(i + 1),
                _ => None,
            };
            if let Some(para) = para {
                self.pendentes.push((None, Acao::MoverAba { id, para }));
            }
        }
        terminar_edicao(ctx);
        verbo
    }

    /// O foco na busca da barra da aba ativa, com o texto dela selecionado (o Ctrl+F e o Buscar do menu Editar).
    fn focar_busca(&mut self, ctx: &egui::Context) {
        if !matches!(self.situacao, Situacao::Pronta) {
            return;
        }
        let aba = self.abas.id_ativa().0;
        if let Some(b) = self.abas.ativa_mut().conteudo.doc().busca() {
            estado::focar_busca_da_barra(ctx, aba, b.texto);
            ctx.request_repaint();
        }
    }

    fn exportando(&self) -> bool {
        self.janelas.exportar.as_ref().is_some_and(JanelaExportar::exportando)
    }

    /// Há o que perguntar ao fechar ou o que ainda pode mudar a resposta: uma aba com alterações não salvas (etapa 5),
    /// uma exportação em andamento, a transação aberta, uma Consulta que executa (também com a resposta já pronta,
    /// que só um quadro toma: um BEGIN que chegou com a janela minimizada), a execução solta de uma Consulta fechada
    /// ou a fila de perguntas ao fechar andando.
    fn algo_a_conferir_ao_fechar(&self) -> bool {
        self.fila.is_some()
            || self.abas.iter().any(|a| a.conteudo.alterado())
            || self.exportando()
            || self.estado.transacao_aberta()
            || self.abas.iter().any(|a| a.conteudo.executando())
            || self.tarefas.iter().any(|t| matches!(t, Tarefa::Consulta(..)))
    }

    /// O corpo do `eframe::App::logic`. Com a janela minimizada ou encoberta, o eframe não desenha quadro e só
    /// chama o `logic`: um pedido de fechar que chega assim (pela barra de tarefas) fecharia sem a pergunta do
    /// `pedido_de_fechar`, e o estado da casca pode estar atrasado (as respostas que chegaram esperam um quadro).
    /// Com algo a conferir ([`Editor::algo_a_conferir_ao_fechar`]), cancela o fechar, restaura a janela e deixa a
    /// decisão para o primeiro quadro dela. Com a janela à vista não faz nada: quem pergunta é o quadro.
    #[doc(hidden)]
    pub fn logica(&mut self, ctx: &egui::Context) {
        let (fechar, escondida) =
            ctx.input(|i| (i.viewport().close_requested(), i.viewport().visible() == Some(false)));
        if !fechar || !escondida || self.pode_fechar || !self.algo_a_conferir_ao_fechar() {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.fechar_pendente = true;
        ctx.request_repaint();
    }

    /// Fechar a janela com uma exportação em andamento ou com a transação aberta pergunta antes. O pedido que chegou com a janela minimizada ou encoberta já foi cancelado pelo
    /// [`Editor::logica`], que a restaurou: ele é decidido no fim do primeiro quadro, por
    /// [`Editor::decidir_fechar_pendente`].
    fn pedido_de_fechar(&mut self, ctx: &egui::Context) {
        let pedido = ctx.input(|i| i.viewport().close_requested());
        if self.pode_fechar || !pedido {
            return;
        }
        // um pedido novo vale pelo que estava pendente
        self.fechar_pendente = false;
        // os arquivos alterados primeiro (etapa 5): a fila pergunta um por um e, no fim, faz as perguntas de baixo
        if self.fila_ao_fechar_a_janela(ctx) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            return;
        }
        let exportando = self.exportando();
        let transacao = self.estado.transacao_aberta();
        if !exportando && !transacao {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        self.perguntar_ao_fechar(exportando, transacao);
    }

    /// O pedido de fechar que chegou com a janela minimizada ou encoberta, no fim do primeiro quadro da janela
    /// restaurada (depois do `aplicar`): as respostas que esperavam um quadro (o BEGIN de uma Consulta, a execução
    /// solta) já acertaram a transação. Com o motivo, a pergunta; sem ele (a troca de banco terminou, a Consulta não
    /// abriu transação), fecha, como a pessoa pediu.
    fn decidir_fechar_pendente(&mut self, ctx: &egui::Context) {
        if !std::mem::take(&mut self.fechar_pendente) || self.pode_fechar {
            return;
        }
        if self.fila_ao_fechar_a_janela(ctx) {
            return;
        }
        let exportando = self.exportando();
        let transacao = self.estado.transacao_aberta();
        if !exportando && !transacao {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        self.perguntar_ao_fechar(exportando, transacao);
        ctx.request_repaint();
    }

    /// A pergunta antes de fechar: a da exportação (e, depois dela, a da transação) ou a da transação.
    fn perguntar_ao_fechar(&mut self, exportando: bool, transacao: bool) {
        if self.confirmacao.is_some() {
            return;
        }
        if exportando {
            // título próprio: a confirmação da janela Exportar ("Interromper a exportação") tem o id do título e
            // pode estar aberta ao mesmo tempo
            self.confirmacao = Some(
                Confirmacao::new(format!(
                    "Há uma exportação em andamento. Fechar o {NOME} a interrompe; nenhum arquivo é gravado."
                ))
                .titulo(format!("Fechar o {NOME}"))
                .rotulo("Interromper e fechar")
                .perigo(true),
            );
            self.confirmado = Some(Confirmado::FecharApp { falta_transacao: transacao });
        } else {
            self.perguntar_fechar_com_transacao();
        }
    }

    fn perguntar_fechar_com_transacao(&mut self) {
        self.confirmacao = Some(
            Confirmacao::new(frase_descartar(&format!("Fechar o {NOME}"), &self.nome_da_consulta_da_transacao()))
                .titulo("Transação aberta")
                .rotulo("Descartar e fechar")
                .perigo(true),
        );
        self.confirmado = Some(Confirmado::FecharApp { falta_transacao: false });
    }

    fn mostrar_confirmacao(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>) {
        let Some(decisao) = confirmar_decisao(ctx, &mut self.confirmacao) else { return };
        let Some(confirmado) = self.confirmado.take() else { return };
        // as perguntas dos arquivos (etapa 5)
        match confirmado {
            Confirmado::SalvarAlterado(id) => {
                if let Some(f) = self.fila.as_mut() {
                    f.perguntando = None;
                    if decisao == Decisao::Descartou {
                        f.respondidas.insert(id);
                    }
                }
                match decisao {
                    // grava e espera: a fila anda quando a gravação terminar
                    Decisao::Confirmou => {
                        self.executar_na_aba(ctx, frame, id, Verbo::Salvar);
                        if let Some(f) = self.fila.as_mut() {
                            f.esperando = Some(id);
                        }
                    }
                    // segue sem gravar: a aba fecha com o destino
                    Decisao::Descartou => {}
                    Decisao::Cancelou => self.fila = None,
                }
                ctx.request_repaint();
                return;
            }
            Confirmado::Troca(..) | Confirmado::FecharApp { .. } => {}
        }
        // as outras não têm a segunda saída: só a ação vale
        let resposta = decisao == Decisao::Confirmou;
        if !resposta {
            // desistir da troca pedida pela janela "Pedir o caminho" fecha a janela também (lá, o `executar`
            // terminava sem erro e a janela fechava)
            if let Confirmado::Troca(_, Origem::Janela) = confirmado {
                self.janelas.caminho = None;
            }
            return;
        }
        match confirmado {
            Confirmado::Troca(troca, origem) => {
                self.iniciar_troca(ctx, troca, true, origem);
            }
            Confirmado::FecharApp { falta_transacao: true } => {
                self.janelas.exportar = None;
                self.perguntar_fechar_com_transacao();
            }
            Confirmado::FecharApp { falta_transacao: false } => {
                self.janelas.exportar = None;
                self.pode_fechar = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            // já tratadas acima
            Confirmado::SalvarAlterado(_) => {}
        }
    }

    // ------------------------------------------------------------------ as Preferências

    /// Abre a janela Preferências na `secao` (com o estado já lido: a janela guarda o retrato do que vale agora).
    fn abrir_preferencias(&mut self, ctx: &egui::Context, secao: Secao) {
        if self.carregado && self.janelas.preferencias.is_none() {
            self.janelas.preferencias = Some(JanelaPreferencias::nova(ctx, self.tema, &self.estado, secao));
        }
    }

    /// Põe em vigor, agora, uma mudança das Preferências (e a que o Cancelar desfaz): o tema, as cores e o tamanho do
    /// código na hora; a família da fonte depois de ler o arquivo, numa tarefa ([`Editor::receber_fonte`]).
    fn aplicar_efeito(&mut self, ctx: &egui::Context, efeito: Efeito) {
        match efeito {
            Efeito::Tema(t) => {
                self.tema = tema_da_interface(t);
                tema::aplicar(ctx, self.tema);
            }
            Efeito::Cores(c) => {
                let mut a = codigo::atual(ctx);
                a.cores = CoresCodigo::de_str(c.como_str()).unwrap_or_default();
                codigo::definir(ctx, a);
            }
            Efeito::Tamanho(px) => {
                let mut a = codigo::atual(ctx);
                a.tamanho = px;
                codigo::definir(ctx, a);
            }
            Efeito::Fonte(None) => {
                // sem leitura: a padrão já está nas definições de base
                self.fonte_pendente = None;
                fontes::restaurar_mono(ctx);
                let mut a = codigo::atual(ctx);
                a.familia = None;
                codigo::definir(ctx, a);
            }
            Efeito::Fonte(Some(nome)) => {
                // procurar a família no disco (um `stat` por candidata) e ler o arquivo, tudo numa tarefa: a thread
                // da interface não toca no disco. A que não existe ou não é fonte volta como erro, que o
                // `receber_fonte` trata como a recusa de sempre
                let procurada = nome.clone();
                let promessa = Promessa::rodar(ctx, move || {
                    let familia = fontes::familias_mono_disponiveis()
                        .into_iter()
                        .find(|f| f.nome == procurada)
                        .ok_or_else(|| ErroBanco::novo(format!("A fonte {procurada} não está nesta máquina.")))?;
                    fontes::ler_familia_mono(&familia).ok_or_else(|| {
                        ErroBanco::novo(format!("Não foi possível ler o arquivo da fonte {}.", familia.nome))
                    })
                });
                self.fonte_pendente = Some(FontePendente { nome, promessa });
            }
        }
        ctx.request_repaint();
    }

    /// A leitura da fonte terminou: a família passa a valer (a partir da passada seguinte, regra do `set_fonts`); se o
    /// arquivo não pôde ser lido, a escolha das Preferências volta para a que estava em vigor.
    fn receber_fonte(&mut self, ctx: &egui::Context) {
        let Some(r) = self.fonte_pendente.as_mut().and_then(|f| f.promessa.tomar()) else { return };
        let Some(FontePendente { nome, .. }) = self.fonte_pendente.take() else { return };
        let carregou = match r {
            Ok(bytes) => fontes::trocar_mono(ctx, bytes, &nome),
            Err(_) => false,
        };
        if carregou {
            let mut a = codigo::atual(ctx);
            a.familia = Some(nome);
            codigo::definir(ctx, a);
            ctx.request_repaint();
        } else {
            self.fonte_recusada(ctx, &nome);
        }
    }

    /// A fonte `nome` não está nesta máquina ou o arquivo não é uma fonte: o toast e a escolha desfeita.
    fn fonte_recusada(&mut self, ctx: &egui::Context, nome: &str) {
        log::warn!("A fonte do código {nome:?} não pôde ser carregada.");
        toast_erro(ctx, format!("Não foi possível carregar a fonte {nome}."));
        let em_vigor = codigo::atual(ctx).familia;
        if let Some(j) = self.janelas.preferencias.as_mut() {
            j.fonte_recusada(em_vigor);
        }
    }

    /// Desenha as Preferências (por cima de tudo, como a Exportar) e cumpre o que ela pede: as mudanças valem na hora;
    /// Cancelar reaplica o retrato da abertura; Salvar grava (numa tarefa) só o que mudou e fecha.
    fn mostrar_preferencias(&mut self, ctx: &egui::Context) {
        let Some(janela) = self.janelas.preferencias.as_mut() else { return };
        let saida = janela.mostrar(ctx, &self.estado);
        for efeito in saida.efeitos {
            self.aplicar_efeito(ctx, efeito);
        }
        match saida.fim {
            None => {}
            Some(Fim::Cancelou { reverter }) => {
                for efeito in reverter {
                    self.aplicar_efeito(ctx, efeito);
                }
                self.janelas.preferencias = None;
            }
            Some(Fim::Salvou(g)) => {
                self.janelas.preferencias = None;
                let limpou_historico = g.limpar_historico;
                let promessa = self.tarefa(ctx, move |s, _| {
                    if g.limpar_recentes {
                        s.prefs().limpar_recentes();
                    }
                    if g.limpar_historico {
                        s.prefs().limpar_historico_sql();
                    }
                    if g.mudanca == prefs::Mudanca::default() {
                        Ok(s.prefs().resumo())
                    } else {
                        s.gravar_preferencias(g.mudanca)
                    }
                });
                // o resumo que volta vale como o do Exibir › Tema: as preferências, os recentes e a exportação novos;
                // o Histórico só é relido quando a gravação terminar (antes, a leitura corria contra a limpeza)
                self.tarefas.push(Tarefa::Tema { promessa, limpou_historico });
            }
        }
    }

    // ------------------------------------------------------------------ o quadro

    /// As larguras do corpo com as docas que a pessoa quer agora, nas larguras dela ([`larguras::dispor`]): numa
    /// janela estreita as docas encolhem até [`larguras::MINIMA`] e, se o documento ainda ficaria com menos de 560px
    /// (`flex:999 1 560px`), a da direita some (sem mudar a escolha dela), como no Início quando ela só tem as
    /// Propriedades.
    fn medidas(&self, corpo: Rect) -> Medidas {
        let pronta = matches!(self.situacao, Situacao::Pronta);
        let tem_propriedades = pronta && self.abas.ativa().conteudo.tem_propriedades();
        let largura = Corpo { janela: corpo.width() + 2.0 * ESPACO, largura: corpo.width(), espaco: ESPACO };
        let com_esquerda = self.paineis.doca_a_vista(Lado::Esquerda, tem_propriedades);
        let quer_direita = self.paineis.doca_a_vista(Lado::Direita, tem_propriedades);
        let disposicao = larguras::dispor(&largura, self.larguras, com_esquerda, quer_direita);
        let esquerda = com_esquerda.then(|| Rect::from_min_size(corpo.min, vec2(disposicao.esquerda, corpo.height())));
        let inicio = esquerda.map_or(corpo.left(), |r| r.right() + ESPACO);
        let com_direita = disposicao.com_direita;
        let direita = if com_direita { corpo.right() - disposicao.direita - ESPACO } else { corpo.right() };
        let secao = Rect::from_min_max(pos2(inicio, corpo.top()), pos2(direita.max(inicio), corpo.bottom()));
        let botao_do_painel = if !tem_propriedades {
            // o Início (e o quadro antes de o estado chegar): o botão desligado, sem mudar a escolha da pessoa
            EstadoPainel::SemPainel
        } else if !self.paineis.a_vista(Painel::Propriedades, true) {
            EstadoPainel::Escondido
        } else if self.paineis.lugar(Painel::Propriedades) == Lugar::Doca(Lado::Direita) && !com_direita {
            EstadoPainel::SemLargura
        } else {
            EstadoPainel::AVista
        };
        Medidas { esquerda, secao, direita, com_direita, botao_do_painel, disposicao }
    }

    /// A divisória de uma doca acabou de pedir `largura` (o arrasto, as setas, o duplo clique). O que a pessoa vê
    /// passa a ser a escolha dela: a outra doca, se está à vista, guarda a largura com que foi desenhada. Quando o
    /// gesto termina, a escolha nova vai para as preferências (só o que mudou, numa tarefa).
    fn largura_pedida(
        &mut self,
        ctx: &egui::Context,
        painel: PainelLateral,
        saida: &SaidaDivisoria,
        d: &Disposicao,
        com_esquerda: bool,
    ) {
        if saida.mudou {
            match painel {
                PainelLateral::Esquerdo => {
                    self.larguras.esquerda = saida.largura;
                    if d.com_direita {
                        self.larguras.direita = d.direita;
                    }
                }
                PainelLateral::Direito => {
                    self.larguras.direita = saida.largura;
                    if com_esquerda {
                        self.larguras.esquerda = d.esquerda;
                    }
                }
            }
            // a doca e o documento deste quadro já foram medidos: o quadro seguinte os desenha na largura nova
            ctx.request_repaint();
        }
        if saida.concluiu {
            self.guardar_larguras(ctx);
        }
    }

    /// Uma doca no retângulo dela (revisão de UI de 19/10/2026): a fileira de abas (só ícone à esquerda, com o nome à
    /// direita), o painel à vista no corpo e o que a pessoa fez nela. O clique numa aba a põe à vista; o "Destacar em
    /// janela" leva o painel à vista para a janela própria. Devolve a aba arrastada, se há uma, com o painel dela.
    #[allow(clippy::too_many_arguments)]
    fn doca(
        &mut self,
        raiz: &mut Ui,
        lado: Lado,
        rect: Rect,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) -> Option<(Painel, Arrasto)> {
        let ctx = raiz.ctx().clone();
        let tem_propriedades = self.abas.ativa().conteudo.tem_propriedades();
        let paineis = self.paineis.abas(lado, tem_propriedades);
        let ativo = self.paineis.ativo(lado, tem_propriedades)?;
        // a aba das Propriedades leva o objeto do documento à vista ("Propriedades · clientes")
        let objeto = {
            let a = self.abas.ativa();
            a.conteudo.titulo(a.rota(), a.numero_consulta(), &self.estado).0
        };
        let itens: Vec<AbaDoca<'_>> = paineis
            .iter()
            .map(|&p| AbaDoca::new(p.icone(), p.nome()).sub((p == Painel::Propriedades).then_some(objeto.as_str())))
            .collect();
        let forma = match lado {
            Lado::Esquerda => FormaAbas::SoIcone,
            Lado::Direita => FormaAbas::ComNome,
        };
        let indice = paineis.iter().position(|&p| p == ativo).unwrap_or(0);
        let mut ui_doca = raiz
            .new_child(UiBuilder::new().id_salt(("doca", lado)).max_rect(rect).layout(Layout::top_down(Align::Min)));
        let r = Doca::new(("doca", lado), lado.rotulo(), &itens).ativa(indice).forma(forma).mostrar(&mut ui_doca);
        if let Some(p) = r.clicada.and_then(|i| paineis.get(i)) {
            self.paineis.focar(*p);
            ctx.request_repaint();
        }
        let arrasto = r.arrasto.and_then(|a| paineis.get(a.aba).map(|p| (*p, a)));
        let mut ui_corpo = ui_doca.new_child(
            UiBuilder::new().id_salt(("doca-corpo", lado)).max_rect(r.corpo).layout(Layout::top_down(Align::Min)),
        );
        ui_corpo.set_clip_rect(r.corpo.intersect(raiz.clip_rect()));
        self.conteudo_do_painel(&mut ui_corpo, ativo, Encaixe::doca(r.acoes), frame, fontes, rota, acoes);
        if r.destacar {
            self.mover_painel(&ctx, ativo, Lugar::Janela);
        }
        arrasto
    }

    /// O corpo de um painel (o Explorador, o Histórico, as Propriedades do documento à vista) no espaço de `ui`, com
    /// as ações dele em `lugar_acoes` (a fileira da doca, ou o rodapé da janela própria).
    #[allow(clippy::too_many_arguments)]
    fn conteudo_do_painel(
        &mut self,
        ui: &mut Ui,
        painel: Painel,
        lugar_acoes: Encaixe,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) {
        let ctx = ui.ctx().clone();
        let (versao, trocando) = (self.versao, self.troca.is_some());
        match painel {
            Painel::Explorador => {
                let mut emitidas = Vec::new();
                let consulta = self.nome_da_consulta_da_transacao();
                let (estado, falha) = (&self.estado, self.falha_objetos.as_deref());
                self.casca.explorador(ui, lugar_acoes, estado, versao, rota, falha, &consulta, &mut emitidas);
                acoes.extend(emitidas.into_iter().map(|a| (None, a)));
                // etapa 4, passo 9: o "Mostrar na pasta" do menu de contexto do Explorador
                if let Some(caminho) = self.casca.revelar_do_explorador() {
                    self.revelar(&ctx, caminho);
                }
            }
            Painel::Historico => {
                let mut emitidas = Vec::new();
                let Editor { servico, estado, casca, .. } = self;
                let mut contexto = Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando);
                casca.historico.mostrar(ui, lugar_acoes, &mut contexto);
                acoes.extend(emitidas.into_iter().map(|a| (None, a)));
            }
            Painel::Propriedades => {
                let id_ativa = self.abas.id_ativa();
                let mut das_propriedades = Vec::new();
                let Editor { servico, estado, casca, abas, .. } = self;
                let mut contexto = Contexto::novo(servico, estado, frame, &mut das_propriedades)
                    .com_casca(versao, trocando)
                    .com_fontes(fontes);
                casca.propriedades(ui, lugar_acoes, &mut abas.ativa_mut().conteudo, id_ativa, &mut contexto);
                acoes.extend(das_propriedades.into_iter().map(|a| (Some(id_ativa), a)));
            }
        }
    }

    /// As janelas próprias dos painéis (revisão de UI de 19/10/2026, prancheta 6): uma janela do sistema, não modal,
    /// para cada painel que mora nela com a janela aberta (as Propriedades, só quando o documento à vista as tem). O
    /// fechar da janela (o × do sistema ou o "Fechar" do cabeçalho) devolve o painel escondido para a doca; o "Prender
    /// de volta" e a aba arrastada até a janela principal o prendem na doca, à vista.
    fn janelas_dos_paineis(
        &mut self,
        ctx: &egui::Context,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
        principal: Rect,
    ) {
        let tem_propriedades =
            matches!(self.situacao, Situacao::Pronta) && self.abas.ativa().conteudo.tem_propriedades();
        let raiz = ctx.input(|i| i.viewport().inner_rect);
        for painel in Painel::TODOS {
            if !self.paineis.janela_aberta(painel) || (painel == Painel::Propriedades && !tem_propriedades) {
                continue;
            }
            let (largura, altura) = tamanho_da_janela(painel);
            let mut construtor = egui::ViewportBuilder::default()
                .with_title(format!("{} · {}", painel.nome(), comum::app::NOME))
                .with_inner_size([largura, altura])
                .with_min_inner_size([260.0, 220.0]);
            if let Some(icone) = interface::marca::icone_janela() {
                construtor = construtor.with_icon(Arc::new(icone));
            }
            if let Some((p, pos)) = self.posicao_da_janela
                && p == painel
            {
                construtor = construtor.with_position(pos);
                self.posicao_da_janela = None;
            }
            let id = egui::ViewportId::from_hash_of(("janela-do-painel", painel));
            let mut pedido = None;
            ctx.show_viewport_immediate(id, construtor, |ui, _| {
                pedido = self.janela_do_painel(ui, painel, frame, fontes, rota, acoes);
                if ui.input(|i| i.viewport().close_requested()) {
                    pedido = Some(PedidoDaJanela::Fechar);
                }
                // a aba solta fora da janela própria: em cima da janela principal, prende na doca daquele lado
                if let Some(PedidoDaJanela::Solta(ponteiro)) = pedido {
                    let tela = ui.input(|i| i.viewport().inner_rect).map_or(ponteiro, |r| r.min + ponteiro.to_vec2());
                    let alvo = raiz.unwrap_or(principal);
                    pedido = alvo.contains(tela).then(|| {
                        let lado = if tela.x < alvo.center().x { Lado::Esquerda } else { Lado::Direita };
                        PedidoDaJanela::Prender(Some(lado))
                    });
                }
            });
            match pedido {
                Some(PedidoDaJanela::Fechar) => {
                    let mudou = self.paineis.fechar_janela(painel);
                    self.depois_de_mover(ctx, mudou);
                }
                Some(PedidoDaJanela::Prender(Some(lado))) => self.mover_painel(ctx, painel, Lugar::Doca(lado)),
                Some(PedidoDaJanela::Prender(None)) => self.prender_painel(ctx, painel),
                Some(PedidoDaJanela::Solta(_)) | None => {}
            }
        }
    }

    /// O conteúdo da janela própria de um painel (prancheta 6): o cabeçalho de 50px com a aba do painel encostada
    /// embaixo (arrastá-la de volta prende), o "Prender de volta" (a silhueta espelhada) e o "Fechar"; o corpo do
    /// painel e, no Explorador, o rodapé de 58px com as ações dele.
    #[allow(clippy::too_many_arguments)]
    fn janela_do_painel(
        &mut self,
        ui: &mut Ui,
        painel: Painel,
        frame: Option<&eframe::Frame>,
        fontes: &[Arc<dyn FonteDeColunas>],
        rota: &Rota,
        acoes: &mut Vec<(Option<IdAba>, Acao)>,
    ) -> Option<PedidoDaJanela> {
        use interface::pecas::janela::{ALTURA_CABECALHO, ALTURA_RODAPE};
        let p = tema::paleta(ui);
        let e = &p.estudio;
        let rect = ui.available_rect_before_wrap();
        pintura::pintar_fundo(ui.painter(), p, rect);
        let cab = Rect::from_min_size(rect.min, vec2(rect.width(), ALTURA_CABECALHO));
        let com_rodape = painel == Painel::Explorador;
        let rodape = Rect::from_min_max(pos2(rect.left(), rect.bottom() - ALTURA_RODAPE), rect.max);
        let corpo = Rect::from_min_max(
            pos2(rect.left(), cab.bottom()),
            pos2(rect.right(), if com_rodape { rodape.top() } else { rect.bottom() }),
        );
        ui.painter().rect_filled(corpo, 0.0, e.painel);
        ui.painter().hline(cab.x_range(), cab.bottom() - 0.5, egui::Stroke::new(1.0, e.vidrob));
        if com_rodape {
            ui.painter().hline(rodape.x_range(), rodape.top() + 0.5, egui::Stroke::new(1.0, e.vidrob));
        }
        let mut pedido = None;
        // a aba (`padding:0 10px 0 12px`), com o objeto das Propriedades
        let objeto = {
            let a = self.abas.ativa();
            a.conteudo.titulo(a.rota(), a.numero_consulta(), &self.estado).0
        };
        let aba = AbaDoca::new(painel.icone(), painel.nome())
            .sub((painel == Painel::Propriedades).then_some(objeto.as_str()));
        let (_, arrasto) = interface::pecas::doca::aba_da_janela(
            ui,
            ("janela-do-painel", painel),
            &aba,
            pos2(cab.left() + 12.0, cab.bottom()),
        );
        if let Some(a) = arrasto
            && a.solto
        {
            pedido = Some(PedidoDaJanela::Solta(a.ponteiro));
        }
        // o Prender de volta e o Fechar, à direita (`gap:2px`)
        let dica_prender = match self.paineis.origem(painel) {
            Lado::Esquerda => "Prender de volta na doca da esquerda",
            Lado::Direita => "Prender de volta na doca da direita",
        };
        let botoes =
            Rect::from_min_max(pos2(cab.right() - 10.0 - 62.0, cab.top()), pos2(cab.right() - 10.0, cab.bottom()));
        let mut ui_botoes = ui.new_child(
            UiBuilder::new()
                .id_salt(("janela-do-painel-botoes", painel))
                .max_rect(botoes)
                .layout(Layout::right_to_left(Align::Center)),
        );
        ui_botoes.spacing_mut().item_spacing.x = 2.0;
        let fechar = interface::pecas::botao::Botao::icone(interface::icones::Icone::Fechar)
            .rotulo("Fechar")
            .dica("Fechar (o painel volta escondido para a doca)");
        if ui_botoes.add(fechar).clicked() {
            pedido = Some(PedidoDaJanela::Fechar);
        }
        let prender = interface::pecas::botao::Botao::icone(interface::icones::Icone::Prender)
            .rotulo("Prender de volta")
            .dica(dica_prender);
        if ui_botoes.add(prender).clicked() {
            pedido = Some(PedidoDaJanela::Prender(None));
        }
        let lugar_acoes = if com_rodape {
            // as ações no rodapé (`padding:0 14px 0 16px`), com os botões de 20px no meio da altura
            let meio = rodape.center().y;
            Rect::from_min_max(pos2(rodape.left() + 16.0, meio - 10.0), pos2(rodape.right() - 14.0, meio + 10.0))
        } else {
            Rect::NOTHING
        };
        let mut ui_corpo = ui.new_child(
            UiBuilder::new()
                .id_salt(("janela-do-painel-corpo", painel))
                .max_rect(corpo)
                .layout(Layout::top_down(Align::Min)),
        );
        ui_corpo.set_clip_rect(corpo);
        self.conteudo_do_painel(&mut ui_corpo, painel, Encaixe::janela(lugar_acoes), frame, fontes, rota, acoes);
        ui.allocate_rect(rect, egui::Sense::hover());
        pedido
    }

    /// Muda um painel de lugar (uma doca, a janela própria) e grava o lugar novo nas preferências, numa tarefa.
    fn mover_painel(&mut self, ctx: &egui::Context, painel: Painel, destino: Lugar) {
        let mudou = self.paineis.mover(painel, destino);
        self.depois_de_mover(ctx, mudou);
    }

    /// Prende de volta, na doca de onde saiu, o painel da janela própria.
    fn prender_painel(&mut self, ctx: &egui::Context, painel: Painel) {
        let mudou = self.paineis.prender(painel);
        self.depois_de_mover(ctx, mudou);
    }

    /// O Exibir › Barra de status: mostra ou esconde a barra e grava a escolha nas preferências, numa tarefa.
    fn alternar_barra_status(&mut self, ctx: &egui::Context) {
        self.barra_status = !self.barra_status;
        let mudanca = prefs::Mudanca::barra_status(self.barra_status);
        let promessa = self.tarefa(ctx, move |s, _| s.gravar_preferencias(mudanca).map(|_| ()));
        self.tarefas.push(Tarefa::Gravar(promessa));
        ctx.request_repaint();
    }

    fn depois_de_mover(&mut self, ctx: &egui::Context, mudou: bool) {
        if mudou {
            let mudanca = prefs::Mudanca::lugares(self.paineis.para_prefs());
            let promessa = self.tarefa(ctx, move |s, _| s.gravar_preferencias(mudanca).map(|_| ()));
            self.tarefas.push(Tarefa::Gravar(promessa));
        }
        ctx.request_repaint();
    }

    /// Grava nas preferências as larguras escolhidas que ainda não estão lá (só as que mudaram), numa tarefa. Uma
    /// gravação por vez: duas em voo, de threads diferentes, podiam terminar fora de ordem e deixar no arquivo a
    /// largura mais velha (o dedo na seta, uma tecla por quadro); o que a pessoa mexeu enquanto uma corria sai quando
    /// ela termina.
    fn guardar_larguras(&mut self, ctx: &egui::Context) {
        if self.gravando_larguras || self.larguras == self.larguras_guardadas {
            return;
        }
        let (agora, antes) = (self.larguras, self.larguras_guardadas);
        let mudanca = prefs::Mudanca::paineis(prefs::MudancaLarguraPaineis {
            explorador: (agora.esquerda != antes.esquerda).then_some(agora.esquerda),
            propriedades: (agora.direita != antes.direita).then_some(agora.direita),
        });
        self.larguras_guardadas = agora;
        self.gravando_larguras = true;
        self.tarefas.push(Tarefa::Larguras(self.tarefa(ctx, move |s, _| s.gravar_preferencias(mudanca))));
    }

    /// Desenha um quadro inteiro na janela (`ui` = a janela toda, sem margem). `frame` é a janela do eframe, dona
    /// dos diálogos do Windows (None nos testes).
    pub fn desenhar(&mut self, ui: &mut Ui, frame: Option<&eframe::Frame>) {
        let ctx = ui.ctx().clone();
        tema::aplicar(&ctx, self.tema);
        if !fontes::instaladas(&ctx) {
            ctx.request_repaint();
            return;
        }
        if !self.iniciado {
            self.iniciar(&ctx);
        }
        self.receber(&ctx);
        self.receber_fonte(&ctx);
        self.cumprir_acao_do_toast(&ctx);
        // a janela voltou a ter o foco (o Alt+Tab de volta, um clique nela): a pasta aberta relê, porque outro
        // programa pode ter criado ou apagado arquivos nela (sem vigia do sistema nem temporizador)
        let focada = ctx.input(|i| i.focused);
        if focada && !self.focada {
            self.releitura_automatica(&ctx);
        }
        self.focada = focada;
        self.soltar_na_janela(&ctx);
        // as ações do quadro, com a aba que as emitiu (None: a casca, as janelas)
        let mut acoes: Vec<(Option<IdAba>, Acao)> = std::mem::take(&mut self.pendentes);
        // o verbo de um atalho do quadro anterior (o F5 deste quadro fica para o próximo)
        let verbo_do_quadro = self.verbo_pendente.take();
        // a aba ativa que outra aba mudou no quadro anterior: relê no fim deste, depois de se desenhar
        let reler_ativa = self.reler_ativa.take();
        let da_casca = |acoes: &mut Vec<(Option<IdAba>, Acao)>, emitidas: Vec<Acao>| {
            acoes.extend(emitidas.into_iter().map(|a| (None, a)));
        };

        // as medidas, de cima para baixo: a barra de título de 30px, a barra de ferramentas (`margin:6px 8px 0`,
        // 44px) e o corpo (`padding:8px; gap:8px`), até a barra de status (`margin:0 8px 8px`; sem ela, até a margem
        // de baixo): a doca da esquerda, a seção do documento (a faixa de abas e o `.doc`) e a doca da direita
        let janela = ctx.content_rect();
        let topo = Rect::from_min_size(janela.min, vec2(janela.width(), ALTURA_TITULO));
        let rect_barra = Rect::from_min_size(
            pos2(janela.left() + ESPACO, topo.bottom() + MARGEM_BARRA),
            vec2(janela.width() - 2.0 * ESPACO, ALTURA_BARRA),
        );
        let altura_status = if self.barra_status { ALTURA_STATUS + ESPACO } else { 0.0 };
        let rect_status = Rect::from_min_max(
            pos2(janela.left() + ESPACO, janela.bottom() - ESPACO - ALTURA_STATUS),
            pos2(janela.right() - ESPACO, janela.bottom() - ESPACO),
        );
        let corpo = Rect::from_min_max(
            pos2(janela.left() + ESPACO, rect_barra.bottom() + ESPACO),
            pos2(janela.right() - ESPACO, janela.bottom() - ESPACO - altura_status),
        );
        let pronta = matches!(self.situacao, Situacao::Pronta);

        // os atalhos: o F5 só com o Atualizar da aba ligado e o Ctrl+Alt+B só com o botão das Propriedades ligado
        // (uma vez por quadro: o que o documento oferece não muda até ele se desenhar; as ofertas dos Dados clonavam
        // as chaves marcadas a cada chamada)
        let ofertas = if pronta { self.abas.ativa().conteudo.ofertas(&self.estado) } else { Ofertas::default() };
        let painel_agora = self.medidas(corpo).botao_do_painel;
        let verbo_do_atalho = self.atalhos_de_teclado(&ctx, ui, &ofertas, painel_agora);
        self.pedido_de_fechar(&ctx);
        // as origens de colunas das abas abertas (o "Trazer colunas"), uma `Arc` por aba, no `Contexto` de todas
        let fontes = self.fontes_de_colunas();
        self.documentos_em_segundo_plano(&ctx, frame, &fontes, &mut acoes);

        let mut raiz =
            ui.new_child(UiBuilder::new().id_salt("editor").max_rect(janela).layout(Layout::top_down(Align::Min)));
        raiz.set_clip_rect(janela);
        pintura::pintar_fundo(raiz.painter(), tema::paleta(&raiz), janela);
        let rota_ativa = self.abas.ativa().rota().clone();
        // o Ctrl+S, o Ctrl+Z e o Ctrl+Y deste quadro; o F5 do quadro anterior
        let mut verbo = verbo_do_atalho.or(verbo_do_quadro);

        // a barra de título, antes de tudo (os menus vêm antes dos painéis na árvore de acessibilidade e no Tab): o
        // verbo do Editar é executado no documento antes de ele se desenhar, como o da barra de ferramentas; os
        // painéis mudam na hora
        let so_o_inicio = self.abas.len() == 1 && self.abas.ativa().chave() == Some(&ChaveAba::Inicio);
        let menus = Menus {
            ofertas: &ofertas,
            tem_banco: self.estado.tem_banco(),
            tem_pasta: self.estado.tem_pasta(),
            pode_fechar_aba: pronta && !so_o_inicio,
            paineis: self.paineis,
            tem_propriedades: pronta && self.abas.ativa().conteudo.tem_propriedades(),
            propriedades: self.medidas(corpo).botao_do_painel,
            tema: self.tema,
            barra_status: self.barra_status,
            banco: self.estado.banco.as_ref(),
            tabela: match (pronta, &self.abas.ativa().conteudo) {
                (true, Documento::Tabela(t)) => Some(MenuTabela {
                    nome: t.nome(),
                    e_visao: t.tabela_aberta().map_or(matches!(rota_ativa, Rota::Visao { .. }), |a| a.e_visao),
                    editavel: t.editavel(self.estado.somente_leitura()),
                }),
                _ => None,
            },
        };
        let mut ui_topo = raiz
            .new_child(UiBuilder::new().id_salt("barra-titulo").max_rect(topo).layout(Layout::top_down(Align::Min)));
        let faixa =
            self.casca.faixa_titulo(&mut ui_topo, &self.estado, self.versao, &self.titulo, self.barra_propria, &menus);
        match faixa.pedido {
            Some(PedidoMenu::Acao(a)) => acoes.push((None, a)),
            Some(PedidoMenu::Verbo(v)) => verbo = Some(v),
            Some(PedidoMenu::Painel(painel)) => self.paineis.alternar(painel),
            Some(PedidoMenu::BarraStatus) => self.alternar_barra_status(&ctx),
            Some(PedidoMenu::Tabela(item)) => self.item_da_tabela(&ctx, item),
            Some(PedidoMenu::Atalhos) => self.janelas.atalhos = true,
            Some(PedidoMenu::Sobre) => {
                self.janelas.sobre.get_or_insert_with(Sobre::default);
            }
            Some(PedidoMenu::Preferencias(secao)) => self.abrir_preferencias(&ctx, secao),
            None => {}
        }

        // a barra de ferramentas, antes das docas (é a ordem do Tab) e do documento: o verbo clicado (ou o
        // F5 do quadro anterior) é executado no documento antes de ele se desenhar, e a busca da barra é o texto que o
        // documento empresta (o que acontece nela também vai ao documento antes do conteúdo); os pedidos da casca
        // valem no quadro seguinte, depois de a edição em curso gravar. O botão das Propriedades é medido com os
        // painéis de agora (o clique numa aba de doca, logo depois, pede outro quadro)
        let barra = Barra {
            ofertas: &ofertas,
            pode_voltar: pronta && self.abas.pode_voltar(),
            pode_avancar: pronta && self.abas.pode_avancar(),
            tem_banco: self.estado.tem_banco(),
            painel: self.medidas(corpo).botao_do_painel,
            aba: self.abas.id_ativa().0,
        };
        let mut ui_barra = raiz.new_child(UiBuilder::new().id_salt("barra-ferramentas").max_rect(rect_barra));
        let busca = if pronta { self.abas.ativa_mut().conteudo.doc().busca() } else { None };
        let (pedido_barra, evento_busca) = casca::ferramentas::barra(&mut ui_barra, &barra, busca);
        match pedido_barra {
            Some(PedidoBarra::Acao(a)) => {
                self.pendentes.push((None, a));
                terminar_edicao(&ctx);
            }
            Some(PedidoBarra::Verbo(v)) => verbo = Some(v),
            Some(PedidoBarra::AlternarPropriedades) => {
                self.paineis.alternar(Painel::Propriedades);
                ctx.request_repaint();
            }
            None => {}
        }
        // o Buscar do menu Editar leva o foco à busca da barra, como o Ctrl+F
        if verbo == Some(Verbo::Buscar) {
            verbo = None;
            self.focar_busca(&ctx);
        }
        // (o Atualizar de uma aba com alterações não salvas: a própria tela pergunta antes, decisão 27 da etapa 5)
        if pronta && (verbo.is_some() || evento_busca.is_some()) {
            let id = self.abas.id_ativa();
            let (versao, trocando) = (self.versao, self.troca.is_some());
            let Editor { servico, estado, abas, .. } = self;
            let mut emitidas = Vec::new();
            let mut contexto =
                Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando).com_fontes(&fontes);
            let documento = &mut abas.ativa_mut().conteudo;
            if let Some(v) = verbo {
                documento.executar(v, &ctx, &mut contexto);
            }
            if let Some(e) = evento_busca {
                documento.doc().evento_busca(e, &ctx, &mut contexto);
                ctx.request_repaint();
            }
            acoes.extend(emitidas.into_iter().map(|a| (Some(id), a)));
        }

        // as larguras, com as docas que a pessoa quer agora
        let Medidas { esquerda: rect_esquerda, secao, direita, com_direita, botao_do_painel, disposicao } =
            self.medidas(corpo);
        let tem_propriedades = pronta && self.abas.ativa().conteudo.tem_propriedades();
        self.propriedades_a_vista = match self.paineis.lugar(Painel::Propriedades) {
            Lugar::Doca(Lado::Direita) => com_direita && self.paineis.a_vista(Painel::Propriedades, tem_propriedades),
            _ => self.paineis.a_vista(Painel::Propriedades, tem_propriedades),
        };
        self.disposicao = Some(disposicao);
        // o documento tem Propriedades, mas o painel não está à vista (escondido ou sem largura): nas telas de
        // rolagem de página, as seções vão para o fim do documento (decisão 4, `estado::propriedades_no_fim`)
        let propriedades_no_fim = matches!(botao_do_painel, EstadoPainel::Escondido | EstadoPainel::SemLargura);

        // a doca da esquerda, antes do documento (é a ordem do Tab)
        let mut arrasto = None;
        if let Some(r) = rect_esquerda {
            arrasto = self.doca(&mut raiz, Lado::Esquerda, r, frame, &fontes, &rota_ativa, &mut acoes);
            // a divisória no vão entre a doca e o documento (logo depois dela, na ordem do Tab)
            let rotulo = rotulo_da_largura(self.paineis.ativo(Lado::Esquerda, tem_propriedades));
            let vao = Rect::from_min_max(pos2(r.right(), r.top()), pos2(r.right() + ESPACO, r.bottom()));
            let saida =
                Divisoria::new("painel-esquerdo", rotulo, vao, LadoDoPainel::Esquerda, disposicao.limites_esquerda())
                    .mostrar(&mut raiz);
            self.largura_pedida(&ctx, PainelLateral::Esquerdo, &saida, &disposicao, true);
        }
        if !self.paineis.a_vista(Painel::Historico, tem_propriedades) {
            self.casca.historico.esconder();
        }

        // a troca de banco que está demorando: o card no alto do conteúdo (e um quadro quando o texto muda)
        let texto_troca = self.troca.as_ref().and_then(|t| {
            let decorrido = ctx.input(|i| i.time) - t.desde;
            if decorrido < ESPERA_PARA_MOSTRAR {
                return None;
            }
            if decorrido < ESPERA_LONGA {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(ESPERA_LONGA - decorrido));
            }
            Some(t.troca.em_andamento(decorrido >= ESPERA_LONGA))
        });

        // a faixa de abas no alto da seção (só com o documento à vista); o que se pede nela vale no quadro seguinte,
        // depois de a edição em curso gravar. O documento (`.doc`) começa 1px antes do fim dela e forma uma superfície
        // só com a aba ativa
        let mut juncao = None;
        let topo_documento = if pronta {
            let r = Rect::from_min_size(secao.min, vec2(secao.width(), ALTURA_FAIXA));
            let mut ui_faixa = raiz.new_child(UiBuilder::new().id_salt("faixa-abas").max_rect(r));
            let (pedido, j) = casca::abas::faixa(&mut ui_faixa, &self.abas, &self.estado);
            if let Some(a) = pedido {
                self.pendentes.push((None, a));
                terminar_edicao(&ctx);
            }
            juncao = j;
            r.bottom() - 1.0
        } else {
            secao.top()
        };
        let rect_documento = Rect::from_min_max(pos2(secao.left(), topo_documento), secao.max);
        self.rect_documento = Some(rect_documento);
        let anatomia = if pronta { self.abas.ativa().conteudo.anatomia() } else { Anatomia::default() };

        // o documento (a peça `.doc`): os avisos do topo (a troca que demora, a transação aberta fora das Consultas), a
        // faixa, o conteúdo e o rodapé que ele pede, por dentro da borda (a roda em cima dos painéis é deles)
        let id_ativa = self.abas.id_ativa();
        let consulta_da_transacao = self.nome_da_consulta_da_transacao();
        let mut do_documento = Vec::new();
        let mut ir_para_a_consulta = false;
        {
            // A página rola pelo teclado como a do navegador (o ScrollArea do egui não faz isso), quando nada tem o
            // foco: um campo de texto, a grade ou um botão focado ficam com as teclas. O documento que rola por
            // dentro (`Rolagem::Propria`) fica com elas.
            let pagina_da_casca = anatomia.rolagem == Rolagem::Pagina;
            let teclado = if !pagina_da_casca
                || self.janela_aberta()
                || ctx.memory(|m| m.focused().is_some() || m.top_modal_layer().is_some())
            {
                None
            } else {
                ctx.input_mut(|i| {
                    let shift = i.modifiers.shift;
                    if i.consume_key(Modifiers::NONE, Key::PageDown)
                        || (!shift && i.consume_key(Modifiers::NONE, Key::Space))
                    {
                        Some(-0.9)
                    } else if i.consume_key(Modifiers::NONE, Key::PageUp)
                        || (shift && i.consume_key(Modifiers::SHIFT, Key::Space))
                    {
                        Some(0.9)
                    } else if i.consume_key(Modifiers::NONE, Key::Home) {
                        Some(f32::INFINITY)
                    } else if i.consume_key(Modifiers::NONE, Key::End) {
                        Some(f32::NEG_INFINITY)
                    } else {
                        None
                    }
                })
            };
            let Editor { servico, estado, situacao, abas, troca, versao, .. } = self;
            let trocando = troca.is_some();
            let versao = *versao;
            let na_consulta = matches!(rota_ativa, Rota::Consulta { .. });
            let mut doc = DocumentoPeca::abrir(&raiz, rect_documento, juncao, anatomia.rodape);
            if let Some(texto) = texto_troca {
                casca::aviso_troca(&mut raiz, &mut doc, texto);
            }
            if estado.transacao_aberta() && !na_consulta {
                ir_para_a_consulta = casca::aviso_transacao(&mut raiz, &mut doc, &consulta_da_transacao);
            }
            // cada aba guarda a própria rolagem
            let mut pagina = Pagina::new(("doc", id_ativa));
            pagina.ao_topo = teclado == Some(f32::INFINITY);
            pagina.rolar_por = match teclado {
                Some(f) if f.is_finite() => Some(f * doc.rect_conteudo().height()),
                Some(f) if f == f32::NEG_INFINITY => Some(ROLAR_ATE_O_FIM),
                _ => None,
            };
            let mut contexto = Contexto::novo(servico, estado, frame, &mut do_documento)
                .com_casca(versao, trocando)
                .com_fontes(&fontes);
            match situacao {
                Situacao::Carregando => {}
                Situacao::Falha(m) => doc.pagina(&mut raiz, pagina, |ui| {
                    Cartao::new("falha-inicio").titulo("Não foi possível iniciar").mostrar(ui, |c| {
                        c.corpo(|ui| {
                            aviso_texto(ui, m, true);
                        });
                    });
                }),
                Situacao::Pronta => {
                    let tela = abas.ativa_mut().conteudo.doc();
                    // os avisos do documento (a transação da Consulta SQL), antes da faixa
                    tela.avisos(&mut doc, &mut raiz, &mut contexto);
                    if anatomia.faixa {
                        doc.faixa(&mut raiz, |ui| tela.faixa(ui, &mut contexto));
                    }
                    match anatomia.rolagem {
                        Rolagem::Pagina => doc.pagina(&mut raiz, pagina, |ui| {
                            tela.conteudo(ui, &mut contexto);
                            // A anatomia foi lida antes do conteúdo: a tela de tabela que acabou de ser lida
                            // (Carregando → Pronta, dentro do `conteudo`) vira a parte de rolagem própria, que enche a
                            // altura toda e deixa o cursor do `ScrollArea` no infinito. Nesse quadro não há fim de
                            // página onde pôr as Propriedades (o próximo quadro já nasce com a anatomia certa).
                            let ainda_pagina = tela.anatomia().rolagem == Rolagem::Pagina;
                            if !ainda_pagina {
                                ui.ctx().request_repaint();
                            }
                            if propriedades_no_fim && ainda_pagina {
                                estado::propriedades_no_fim(ui, |ui| tela.propriedades(ui, &mut contexto));
                            }
                        }),
                        Rolagem::Propria => doc.propria(&mut raiz, |ui| tela.conteudo(ui, &mut contexto)),
                    }
                    let _ = doc.rodape(&mut raiz, |ui| tela.rodape(ui, &mut contexto));
                }
            }
        }
        if ir_para_a_consulta {
            acoes.push((None, Acao::Navegar(Rota::Consulta { sql: None })));
        }
        acoes.extend(do_documento.into_iter().map(|a| (Some(id_ativa), a)));
        // os botões da ficha do arquivo de dados: o programa padrão e o Explorador de Arquivos, numa tarefa
        if let Some(Documento::Arquivo(t)) = pronta.then(|| &mut self.abas.ativa_mut().conteudo)
            && let Some(pedido) = t.tomar_pedido()
        {
            match pedido {
                PedidoArquivo::AbrirNoPadrao(caminho) => self.abrir_no_programa(&ctx, caminho),
                PedidoArquivo::MostrarNaPasta(caminho) => self.revelar(&ctx, caminho),
                PedidoArquivo::TrazerColunas(rotulo) => self.trazer_colunas_de(&ctx, rotulo),
            }
        }

        // a doca da direita, depois de o documento se desenhar neste quadro (as Propriedades da tabela já lida); o que
        // o painel pede vem marcado com a aba
        if com_direita {
            let r = Rect::from_min_max(pos2(direita + ESPACO, corpo.top()), corpo.max);
            // a divisória no vão entre o documento e a doca (antes dela, na ordem do Tab)
            let vao = Rect::from_min_max(pos2(direita, r.top()), pos2(r.left(), r.bottom()));
            let rotulo = rotulo_da_largura(self.paineis.ativo(Lado::Direita, tem_propriedades));
            let saida =
                Divisoria::new("propriedades", rotulo, vao, LadoDoPainel::Direita, disposicao.limites_direita())
                    .mostrar(&mut raiz);
            self.largura_pedida(&ctx, PainelLateral::Direito, &saida, &disposicao, rect_esquerda.is_some());
            if let Some(a) = self.doca(&mut raiz, Lado::Direita, r, frame, &fontes, &rota_ativa, &mut acoes) {
                arrasto = Some(a);
            }
        }
        // a aba de painel arrastada: acende o destino e, solta, muda o painel de lugar
        if let Some((painel, a)) = arrasto {
            let destino = destino_do_arrasto(a.ponteiro, janela, rect_esquerda, com_direita.then_some(direita), corpo);
            if let Some((lugar, alvo)) = destino {
                if a.solto {
                    if lugar == Lugar::Janela {
                        // a janela abre onde a aba foi solta (o ponteiro, na tela)
                        let tela =
                            ctx.input(|i| i.viewport().inner_rect).map_or(a.ponteiro, |r| r.min + a.ponteiro.to_vec2());
                        self.posicao_da_janela = Some((painel, tela));
                    }
                    self.mover_painel(&ctx, painel, lugar);
                } else {
                    interface::pecas::doca::pintar_alvo(&raiz, Id::new("alvo-da-doca"), alvo);
                }
            }
            ctx.request_repaint();
        }

        // as janelas próprias dos painéis (depois do documento: as Propriedades da tabela já lida)
        self.janelas_dos_paineis(&ctx, frame, &fontes, &rota_ativa, &mut acoes, janela);

        // a barra de status, com o que o documento à vista diz de si (depois de ele se desenhar neste quadro)
        let do_status = self.documento().and_then(|d| d.status(&self.estado));
        // a aba de arquivo que diz o próprio status (os editores da etapa 5): a barra do arquivo, no lugar da do banco
        let do_arquivo = self.documento().and_then(|d| d.doc_ref().status_do_arquivo(&self.estado));
        if self.barra_status {
            let mut ui_status = raiz.new_child(UiBuilder::new().id_salt("barra-status").max_rect(rect_status));
            let consulta = self.nome_da_consulta_da_transacao();
            let (estado, versao) = (&self.estado, self.versao);
            self.casca.status(&mut ui_status, estado, versao, do_status.as_deref(), &consulta, do_arquivo.as_ref());
        }

        // a aba que outra aba mudou no quadro anterior relê agora (a célula em edição já gravou neste quadro)
        self.reler_a_ativa(&ctx, reler_ativa);

        // janelas globais, por cima de tudo
        let mut indice_criado = false;
        let mut caminho_digitado = None;
        let mut das_janelas = Vec::new();
        {
            let (versao, trocando) = (self.versao, self.troca.is_some());
            let Editor { servico, estado, janelas, casca, renderizador, abrir_de_teste, .. } = self;
            {
                let mut contexto = Contexto::novo(servico, estado, frame, &mut das_janelas).com_casca(versao, trocando);
                if let Some(j) = janelas.sobre.as_mut()
                    && !j.mostrar(&ctx, &mut contexto, *renderizador, abrir_de_teste)
                {
                    janelas.sobre = None;
                    casca.focar_menu();
                }
                if let Some(j) = janelas.exportar.as_mut()
                    && !j.mostrar(&ctx, &mut contexto)
                {
                    janelas.exportar = None;
                }
                if let Some(j) = janelas.nova_tabela.as_mut()
                    && !j.mostrar(&ctx, &mut contexto)
                {
                    janelas.nova_tabela = None;
                }
            }
            if let Some(j) = janelas.novo_indice.as_mut() {
                // o que a janela emite fica à parte para saber se ela criou o índice (`RecarregarObjetos`)
                let mut emitidas = Vec::new();
                let mut contexto = Contexto::novo(servico, estado, frame, &mut emitidas).com_casca(versao, trocando);
                let aberta = j.mostrar(&ctx, &mut contexto);
                janelas.indice_criado |= emitidas.iter().any(|a| matches!(a, Acao::RecarregarObjetos));
                das_janelas.extend(emitidas);
                if !aberta {
                    janelas.novo_indice = None;
                    indice_criado = std::mem::take(&mut janelas.indice_criado);
                }
            }
            if janelas.atalhos && !casca::janela_atalhos(&ctx) {
                janelas.atalhos = false;
                casca.focar_menu();
            }
            if let Some(j) = janelas.caminho.as_mut() {
                caminho_digitado = j.mostrar(&ctx);
            }
        }
        self.mostrar_preferencias(&ctx);
        da_casca(&mut acoes, das_janelas);
        if indice_criado {
            // criado o índice, as abas da tabela releem (a ativa na hora) e a dica de ordenação lenta some
            let tabela = std::mem::take(&mut self.janelas.tabela_do_indice);
            for aba in self.abas.iter_mut() {
                if matches!(aba.conteudo, Documento::Tabela(_)) && aba.rota().nome_objeto() == Some(tabela.as_str()) {
                    aba.conteudo.marcar_para_reler();
                }
            }
            ctx.request_repaint();
        }
        if let Some(s) = caminho_digitado {
            self.caminho_digitado(&ctx, s);
        }
        // a fila de perguntas ao fechar abas alteradas: a gravação esperada, a próxima pergunta ou o destino
        self.andar_fila(&ctx);
        self.mostrar_confirmacao(&ctx, frame);
        // o toast fica acima da barra de status, com o espaço de sempre entre os painéis; sem ela, acima do rodapé
        // do documento (a paginação e o "Linhas 1–60 de 60" continuam à vista)
        let limite_toast = if self.barra_status {
            rect_status.top() - ESPACO
        } else {
            corpo.bottom() - interface::pecas::documento::ALTURA_RODAPE - ESPACO
        };
        desenhar_toast(&ctx, limite_toast);
        // os controles da janela sem decoração, por cima de tudo (também da janela modal)
        if self.barra_propria {
            casca::janela::controles_da_janela(&mut raiz, faixa.logo, faixa.livre);
        }

        self.aplicar(&ctx, frame, acoes);
        // o fechar que chegou com a janela minimizada, com o que este quadro tomou (a transação já acertada)
        self.decidir_fechar_pendente(&ctx);
        // uma janela global fechou (a Exportar e a cópia criam arquivos): a pasta aberta relê
        let com_janela = self.janelas.alguma();
        if self.com_janela && !com_janela {
            self.reler_a_pasta(&ctx);
        }
        // a releitura que esperava uma leitura em andamento terminar
        if self.reler_pendente && !self.lendo_pasta() {
            self.reler_a_pasta(&ctx);
        }
        self.com_janela = com_janela;
        let janela_aberta = self.janela_aberta();
        self.casca.devolver_foco(&ctx, janela_aberta);
        if self.titulo_enviado.as_deref() != Some(self.titulo.as_str()) {
            ctx.send_viewport_cmd(ViewportCommand::Title(self.titulo.clone()));
            self.titulo_enviado = Some(self.titulo.clone());
        }
    }
}

/// As variáveis de ambiente que ligam o [`diagnosticar_repintura`]: a do DataBox e a do nome antigo, que continua
/// valendo.
const VARIAVEIS_DIAGNOSTICO: [&str; 2] = ["DATABOX_DIAGNOSTICO_REPINTURA", "EDITOR_DIAGNOSTICO_REPINTURA"];

/// Com a variável de ambiente `DATABOX_DIAGNOSTICO_REPINTURA` (ou a antiga, `EDITOR_DIAGNOSTICO_REPINTURA`)
/// definida, registra no log, a cada segundo com quadros, quantos quadros houve e quem pediu repintura (o app
/// parado não deve pintar nada).
fn diagnosticar_repintura(ctx: &egui::Context) {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    /// (início da janela de 1 s, quadros, causas e eventos contados)
    type Conta = Option<(f64, u32, BTreeMap<String, u32>)>;
    static LIGADO: OnceLock<bool> = OnceLock::new();
    static CONTA: Mutex<Conta> = Mutex::new(None);
    if !*LIGADO.get_or_init(|| VARIAVEIS_DIAGNOSTICO.iter().any(|v| std::env::var_os(v).is_some())) {
        return;
    }
    let agora = ctx.input(|i| i.time);
    let mut conta = CONTA.lock().unwrap_or_else(|e| e.into_inner());
    let (inicio, quadros, causas) = conta.get_or_insert_with(|| (agora, 0, BTreeMap::new()));
    *quadros += 1;
    for c in ctx.repaint_causes() {
        *causas.entry(format!("{}:{} {}", c.file, c.line, c.reason)).or_default() += 1;
    }
    ctx.input(|i| {
        for e in &i.raw.events {
            let nome: String = format!("{e:?}").chars().take_while(|c| c.is_alphanumeric()).collect();
            *causas.entry(format!("evento {nome}")).or_default() += 1;
        }
    });
    if agora - *inicio >= 1.0 {
        log::info!("repintura: {quadros} quadros em {:.1} s; causas: {causas:?}", agora - *inicio);
        *conta = None;
    }
}

impl eframe::App for Editor {
    /// Antes de cada `ui` e, com a janela minimizada ou encoberta, no lugar dele (ver [`Editor::logica`]).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.logica(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.desenhar(ui, Some(frame));
        diagnosticar_repintura(ui.ctx());
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aparencia_das_prefs_vira_a_do_interface() {
        // as quatro cores pelo texto do arquivo (os dois enums têm de andar juntos)
        for (de_prefs, do_interface) in prefs::CoresCodigo::TODAS.into_iter().zip(CoresCodigo::TODAS) {
            assert_eq!(de_prefs.como_str(), do_interface.como_str());
            let a = prefs::Aparencia { cores_codigo: de_prefs, ..prefs::Aparencia::default() };
            assert_eq!(aparencia_da_interface(&a).cores, do_interface);
        }
        assert_eq!(aparencia_da_interface(&prefs::Aparencia::default()), AparenciaCodigo::default());
        assert_eq!(prefs::TAMANHO_CODIGO_PADRAO, interface::codigo::TAMANHO_PADRAO);
        let a = prefs::Aparencia {
            cores_codigo: prefs::CoresCodigo::Monokai,
            fonte_codigo: Some("Família que não existe".into()),
            tamanho_codigo: 14.0,
        };
        // aplicada: cores e tamanho valem; a família que não existe fica de fora
        let ctx = egui::Context::default();
        aplicar_aparencia(&ctx, &a);
        let atual = interface::codigo::atual(&ctx);
        assert_eq!(atual, AparenciaCodigo { cores: CoresCodigo::Monokai, familia: None, tamanho: 14.0 });
        assert_eq!(fontes::mono_escolhida(&ctx), None);
    }

    #[test]
    fn erro_da_partida_como_janela_py() {
        let texto = erro_da_partida(std::path::Path::new(r"C:\dados\estragado.db"), "O arquivo não é um banco.");
        assert_eq!(
            texto,
            "Não foi possível abrir o banco C:\\dados\\estragado.db.\n\nO arquivo não é um banco.\n\nO DataBox abre \
             sem ele."
        );
    }

    #[test]
    fn texto_da_troca_que_demora() {
        let abrir = Troca::Abrir { caminho: PathBuf::from("x.db"), somente_leitura: false };
        assert_eq!(abrir.em_andamento(false), "Abrindo o banco…");
        assert_eq!(Troca::Novo { caminho: PathBuf::from("x.db") }.em_andamento(false), "Criando o banco…");
        assert_eq!(Troca::Fechar.em_andamento(false), "Fechando o banco…");
        assert_eq!(
            abrir.em_andamento(true),
            "Abrindo o banco. Aguardando o banco atual terminar a operação em andamento."
        );
        assert!(Troca::Fechar.em_andamento(true).contains("operação em andamento"));
    }

    /// O pedido de fechar como o eframe o entrega ao `logic` (só o estado da janela conta no `run_logic`).
    fn pedido_de_fechar_com(minimizada: bool, encoberta: bool) -> egui::RawInput {
        let mut raw = egui::RawInput::default();
        let info = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        info.minimized = Some(minimizada);
        info.occluded = Some(encoberta);
        info.events.push(egui::ViewportEvent::Close);
        raw
    }

    fn comandos_da_logica(e: &mut Editor, ctx: &egui::Context, raw: &egui::RawInput) -> Vec<ViewportCommand> {
        let saida = ctx.run_logic(raw, |c| e.logica(c));
        saida.viewport_commands.get(&egui::ViewportId::ROOT).cloned().unwrap_or_default()
    }

    /// O primeiro quadro da janela restaurada (sem pedido de fechar novo), só com o `pedido_de_fechar` no começo e o
    /// `decidir_fechar_pendente` no fim: os comandos de fechar que eles mandaram (o primeiro quadro de um contexto
    /// novo também manda o `SetTheme`).
    fn comandos_do_quadro(e: &mut Editor, ctx: &egui::Context) -> Vec<ViewportCommand> {
        let saida = ctx.run_ui(egui::RawInput::default(), |ui| {
            e.pedido_de_fechar(ui.ctx());
            e.decidir_fechar_pendente(ui.ctx());
        });
        let comandos = saida.viewport_output.get(&egui::ViewportId::ROOT).map_or_else(Vec::new, |v| {
            v.commands
                .iter()
                .filter(|c| matches!(c, ViewportCommand::Close | ViewportCommand::CancelClose))
                .cloned()
                .collect()
        });
        // as texturas das fontes não vão a lugar nenhum aqui (sem pintura)
        saida.drop_without_applying_deltas();
        comandos
    }

    fn com_transacao(e: &mut Editor, aberta: bool) {
        if let Some(b) = e.estado.banco.as_mut() {
            b.transacao_aberta = aberta;
        }
    }

    /// O tipo do documento da aba (para conferir que a navegação o refez).
    fn tipo_do_documento(e: &Editor) -> &'static str {
        match e.documento() {
            None => "nenhum",
            Some(Documento::Inicio(_)) => "inicio",
            Some(Documento::Banco(_)) => "banco",
            Some(Documento::Tabela(_)) => "tabela",
            Some(Documento::Consulta(_)) => "consulta",
            Some(Documento::Arquivo(_)) => "arquivo",
        }
    }

    /// O editor com o estado do ambiente já carregado, na Visão geral.
    fn editor_pronto(amb: &estado::apoio_testes::Ambiente) -> Editor {
        let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
        e.estado = amb.estado.clone();
        e.carregado = true;
        e.reiniciar_abas(Rota::Banco, "");
        e
    }

    #[test]
    fn navegar_abre_ou_foca_e_voltar_so_foca() {
        let amb = estado::apoio_testes::Ambiente::exemplo("abas_da_casca");
        let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
        assert_eq!(tipo_do_documento(&e), "nenhum", "antes de o estado chegar, nada à vista");
        assert!(!e.ocupado());
        e.estado = amb.estado.clone();
        e.carregado = true;
        // o erro do banco da linha de comando espera o primeiro Início
        e.erro_inicial = Some("não abriu".into());
        e.reiniciar_abas(Rota::Banco, "");
        assert_eq!(tipo_do_documento(&e), "banco");
        assert!(e.erro_inicial.is_some());
        let banco = e.abas.id_ativa();

        let clientes = Rota::objeto("clientes", false);
        e.navegar(clientes.clone());
        let tabela = e.abas.id_ativa();
        assert_ne!(tabela, banco);
        assert_eq!(tipo_do_documento(&e), "tabela");
        e.navegar(Rota::consulta_com("SELECT 1"));
        let consulta = e.abas.id_ativa();
        assert_eq!(tipo_do_documento(&e), "consulta");
        // o Substituir vai para a aba que o emitiu, mesmo fora de vista, sem refazer nada
        let pagina_2 = clientes.com_dados(estado::ConsultaDados { pagina: 2, ..Default::default() });
        e.trocar_endereco(Some(tabela), pagina_2.clone(), false);
        assert_eq!(e.abas.get(tabela).map(|a| a.rota()), Some(&pagina_2));
        assert_eq!(e.abas.id_ativa(), consulta);
        // um endereço que não é do documento da aba não vale
        e.trocar_endereco(Some(consulta), Rota::Banco, false);
        assert_eq!(e.rota(), &Rota::consulta_com("SELECT 1"));
        // voltar só foca a aba, com o endereço dela
        assert!(e.abas.voltar());
        assert_eq!((e.rota(), tipo_do_documento(&e)), (&pagina_2, "tabela"));
        // navegar até uma aba aberta a foca sem trocar o endereço
        e.navegar(Rota::Banco);
        e.navegar(clientes.clone());
        assert_eq!((e.abas.id_ativa(), e.rota()), (tabela, &pagina_2));
        assert_eq!(e.abas.len(), 3);

        e.navegar(Rota::Inicio);
        assert_eq!(tipo_do_documento(&e), "inicio");
        assert!(e.erro_inicial.is_none(), "o Início levou o erro");
        // fechar a ativa vai para a usada antes
        e.fechar_aba(e.abas.id_ativa());
        assert_eq!(e.abas.id_ativa(), tabela);
        assert_eq!(e.abas.len(), 3);

        // a troca de banco recomeça as abas (outros ids) e o histórico
        e.reiniciar_abas(Rota::Banco, "");
        assert_eq!(e.abas.len(), 1);
        assert!(e.abas.id_ativa() > consulta);
        assert!(!e.abas.pode_voltar());
        assert_eq!(e.rota(), &Rota::Banco);
        // sem banco, toda rota volta ao Início, também a Nova consulta
        e.estado.banco = None;
        e.navegar(clientes);
        assert_eq!((e.rota(), tipo_do_documento(&e)), (&Rota::Inicio, "inicio"));
        e.nova_consulta(Some("SELECT 1".into()));
        assert_eq!((e.rota(), e.abas.len()), (&Rota::Inicio, 2));
    }

    #[test]
    fn a_transacao_lembra_a_consulta_que_a_abriu() {
        let amb = estado::apoio_testes::Ambiente::exemplo("consulta_da_transacao");
        let mut e = editor_pronto(&amb);
        let ctx = egui::Context::default();
        e.nova_consulta(None);
        let c1 = e.abas.id_ativa();
        e.nova_consulta(None);
        let c2 = e.abas.id_ativa();
        // uma aba de tabela que grava dentro da transação não é a da transação
        e.navegar(Rota::objeto("clientes", false));
        let tabela = e.abas.id_ativa();
        e.notar_transacao(&ctx, Some(tabela), true);
        assert!(e.estado.transacao_aberta());
        assert_eq!(e.consulta_da_transacao, None);
        e.notar_transacao(&ctx, None, false);

        // o BEGIN na 1; a sondagem da 2 vê a mesma transação, que continua sendo da 1
        e.notar_transacao(&ctx, Some(c1), true);
        e.notar_transacao(&ctx, Some(c2), true);
        assert_eq!(e.consulta_da_transacao, Some(c1));
        e.navegar(Rota::Consulta { sql: None });
        assert_eq!(e.abas.id_ativa(), c1, "o \"Ir para a Consulta SQL\" foca a da transação");
        // fechada a 1, a mais recente; e a primeira Consulta que vê a transação aberta passa a ser a dela
        e.fechar_aba(c1);
        e.navegar(Rota::objeto("clientes", false));
        e.navegar(Rota::Consulta { sql: None });
        assert_eq!(e.abas.id_ativa(), c2);
        e.notar_transacao(&ctx, Some(c2), true);
        assert_eq!(e.consulta_da_transacao, Some(c2));
        // o COMMIT (de onde vier) tira a transação
        e.notar_transacao(&ctx, None, false);
        assert_eq!(e.consulta_da_transacao, None);
        assert!(!e.estado.transacao_aberta());
    }

    /// C14: a leitura do BancoInfo que volta atrasada não desfaz a transação que a nota seguinte acertou, nem uma
    /// leitura mais nova já aplicada.
    #[test]
    fn leitura_atrasada_do_banco_nao_desfaz_a_transacao_nem_a_leitura_mais_nova() {
        let amb = estado::apoio_testes::Ambiente::exemplo("leitura_atrasada");
        let mut e = editor_pronto(&amb);
        let ctx = egui::Context::default();
        let id = e.estado.banco_id();
        let base = e.estado.banco.clone().expect("banco");
        let info = |aberta: bool, tamanho: u64| BancoInfo { transacao_aberta: aberta, tamanho, ..base.clone() };
        // um BEGIN e um COMMIT seguidos: duas notas que mudam, duas leituras no ar (0 e 1)
        e.notar_transacao(&ctx, None, true);
        e.notar_transacao(&ctx, None, false);
        assert!(!e.estado.transacao_aberta());
        // a leitura 0, do tempo do BEGIN, chega depois do COMMIT: o resto vale, a transação não volta
        e.fim_da_tarefa(&ctx, FimTarefa::Banco(id.clone(), 0, Ok(info(true, 1234))));
        assert!(!e.estado.transacao_aberta(), "a leitura velha não reabre a transação");
        assert_eq!(e.estado.banco.as_ref().map(|b| b.tamanho), Some(1234));
        // a leitura 1, de depois do COMMIT, vale inteira; a 0 que chegue de novo é mais velha que a aplicada
        e.fim_da_tarefa(&ctx, FimTarefa::Banco(id.clone(), 1, Ok(info(false, 5678))));
        assert_eq!(e.estado.banco.as_ref().map(|b| b.tamanho), Some(5678));
        e.fim_da_tarefa(&ctx, FimTarefa::Banco(id, 0, Ok(info(true, 1))));
        assert_eq!(e.estado.banco.as_ref().map(|b| b.tamanho), Some(5678), "a mais velha é descartada");
        assert!(!e.estado.transacao_aberta());
    }

    /// C13: limpar o histórico só marca a lista como velha quando a gravação (que limpa o arquivo) termina: antes, a
    /// releitura corria contra a limpeza e podia trazer a lista de antes.
    #[test]
    fn limpar_o_historico_marca_a_lista_velha_so_quando_a_gravacao_termina() {
        let amb = estado::apoio_testes::Ambiente::exemplo("limpar_historico");
        let mut e = editor_pronto(&amb);
        let ctx = egui::Context::default();
        e.casca.historico = Default::default();
        assert!(!e.casca.historico.velha());
        let promessa = Promessa::rodar(&ctx, {
            let prefs = e.estado.prefs.clone();
            move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                Ok(prefs)
            }
        });
        e.tarefas.push(Tarefa::Tema { promessa, limpou_historico: true });
        e.receber(&ctx);
        assert!(!e.casca.historico.velha(), "a gravação ainda não terminou");
        let inicio = std::time::Instant::now();
        while !e.casca.historico.velha() {
            assert!(inicio.elapsed() < std::time::Duration::from_secs(10), "a gravação não terminou");
            std::thread::sleep(std::time::Duration::from_millis(20));
            e.receber(&ctx);
        }
        assert!(e.tarefas.is_empty());
    }

    #[test]
    fn objetos_que_sumiram_fecham_as_abas_menos_a_ativa() {
        let amb = estado::apoio_testes::Ambiente::exemplo("objetos_sumidos");
        let mut e = editor_pronto(&amb);
        e.navegar(Rota::objeto("clientes", false));
        // a mesma aba (uma aba por objeto): a parte vira a vista das Propriedades
        e.navegar(Rota::objeto_na_aba("clientes", false, estado::Parte::Estrutura));
        e.navegar(Rota::objeto("produtos", false));
        e.navegar(Rota::objeto("pedidos", false));
        let pedidos = e.abas.id_ativa();
        e.navegar(Rota::objeto("fantasma", false));
        let fantasma = e.abas.id_ativa();
        e.navegar(Rota::objeto("pedidos", false));
        let mut objetos = e.estado.objetos.clone().expect("objetos");
        let ctx = egui::Context::default();
        // a lista igual não fecha nada (nem a aba de um nome que nunca existiu)
        e.trocar_objetos(&ctx, objetos.clone());
        assert_eq!(e.abas.len(), 5);
        objetos.tabelas.retain(|t| t.nome != "clientes" && t.nome != "produtos" && t.nome != "pedidos");
        e.trocar_objetos(&ctx, objetos);
        let ids: Vec<IdAba> = e.abas.iter().map(|a| a.id()).collect();
        assert_eq!(ids.len(), 3, "a de clientes e a de produtos fecharam");
        assert!(ids.contains(&pedidos), "a ativa fica, com o card de que não existe mais");
        assert!(ids.contains(&fantasma));
    }

    #[test]
    fn recarregar_objetos_de_outra_aba_rele_tambem_a_ativa_no_quadro_seguinte() {
        // antes, a ativa ficava como estava: a grade à vista mostrava as linhas de antes de um INSERT da Consulta
        // escondida ou da execução solta
        let amb = estado::apoio_testes::Ambiente::exemplo("reler_a_ativa");
        let mut e = editor_pronto(&amb);
        let ctx = egui::Context::default();
        e.nova_consulta(None);
        let consulta = e.abas.id_ativa();
        e.navegar(Rota::objeto("clientes", false));
        let tabela = e.abas.id_ativa();
        // da Consulta escondida: a ativa relê no fim do quadro seguinte (depois de a célula em edição gravar)
        e.marcar_para_reler(&ctx, Some(consulta));
        assert_eq!(e.reler_ativa, Some(tabela));
        // da execução solta (sem aba) também
        e.reler_ativa = None;
        e.marcar_para_reler(&ctx, None);
        assert_eq!(e.reler_ativa, Some(tabela));
        // a aba de origem não se marca
        e.reler_ativa = None;
        e.marcar_para_reler(&ctx, Some(tabela));
        assert_eq!(e.reler_ativa, None);
    }

    #[test]
    fn execucao_solta_so_recarrega_os_objetos_quando_mudou_algo() {
        let amb = estado::apoio_testes::Ambiente::exemplo("solta_sem_mudanca");
        let mut e = editor_pronto(&amb);
        let ctx = egui::Context::default();
        let id = e.estado.banco_id();
        let resposta = |dados: bool, esquema: bool, aberta: bool| banco::ResultadoConsulta {
            resultados: Vec::new(),
            alterou_esquema: esquema,
            alterou_dados: dados,
            transacao_aberta: aberta,
        };
        let recarregou = |e: &mut Editor| {
            let sim = e.pendentes.iter().any(|(_, a)| matches!(a, Acao::RecarregarObjetos));
            e.pendentes.clear();
            sim
        };
        // um SELECT: só a transação
        e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id.clone(), Ok(resposta(false, false, false))));
        assert!(!recarregou(&mut e));
        // um INSERT ou um CREATE: os objetos e as abas
        e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id.clone(), Ok(resposta(true, false, false))));
        assert!(recarregou(&mut e));
        e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id.clone(), Ok(resposta(false, true, false))));
        assert!(recarregou(&mut e));
        // o ROLLBACK de uma transação aberta (os dados voltaram)
        com_transacao(&mut e, true);
        e.fim_da_tarefa(&ctx, FimTarefa::Consulta(id, Ok(resposta(false, false, false))));
        assert!(recarregou(&mut e));
    }

    #[test]
    fn logica_com_a_janela_minimizada_cancela_e_restaura() {
        let amb = estado::apoio_testes::Ambiente::exemplo("logica_minimizada");
        let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
        e.estado = amb.estado.clone();
        let ctx = egui::Context::default();
        let minimizada = pedido_de_fechar_com(true, false);
        let restaurar = vec![ViewportCommand::CancelClose, ViewportCommand::Minimized(false), ViewportCommand::Focus];

        // sem exportação nem transação, fecha como antes, sem perguntar
        assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), vec![]);
        assert!(!e.fechar_pendente);

        com_transacao(&mut e, true);
        // à vista, quem pergunta é o quadro
        assert_eq!(comandos_da_logica(&mut e, &ctx, &pedido_de_fechar_com(false, false)), vec![]);
        assert!(!e.fechar_pendente);

        // minimizada, com a transação aberta: não fecha, volta à vista e pergunta no quadro seguinte (sem outro
        // CancelClose: o pedido já foi cancelado)
        assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), restaurar);
        assert!(e.fechar_pendente);
        assert_eq!(comandos_do_quadro(&mut e, &ctx), vec![]);
        assert!(!e.fechar_pendente);
        assert!(e.confirmacao.is_some());
        assert!(matches!(e.confirmado, Some(Confirmado::FecharApp { falta_transacao: false })));
        e.confirmacao = None;
        e.confirmado = None;

        // encoberta, o mesmo; se a transação acabou enquanto a janela voltava, ela fecha, como a pessoa pediu
        assert_eq!(comandos_da_logica(&mut e, &ctx, &pedido_de_fechar_com(false, true)), restaurar);
        assert!(e.fechar_pendente);
        com_transacao(&mut e, false);
        assert_eq!(comandos_do_quadro(&mut e, &ctx), vec![ViewportCommand::Close]);
        assert!(e.confirmacao.is_none());
        com_transacao(&mut e, true);

        // sem o pedido de fechar, nada
        let mut so_minimizada = minimizada.clone();
        if let Some(info) = so_minimizada.viewports.get_mut(&egui::ViewportId::ROOT) {
            info.events.clear();
        }
        assert_eq!(comandos_da_logica(&mut e, &ctx, &so_minimizada), vec![]);
        assert!(!e.fechar_pendente);

        // depois do "Descartar e continuar", fecha sem perguntar de novo
        e.pode_fechar = true;
        assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), vec![]);
        assert!(!e.fechar_pendente);
    }

    #[test]
    fn logica_restaura_com_a_resposta_do_begin_ainda_nao_tomada() {
        // antes, minimizada, a casca decidia pelo estado de antes da resposta (sem transação) e fechava sem perguntar
        let amb = estado::apoio_testes::Ambiente::exemplo("logica_begin");
        let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
        e.estado = amb.estado.clone();
        let ctx = egui::Context::default();
        let minimizada = pedido_de_fechar_com(true, false);
        let restaurar = vec![ViewportCommand::CancelClose, ViewportCommand::Minimized(false), ViewportCommand::Focus];
        // a execução solta de uma Consulta fechada, com a resposta do BEGIN já pronta (só um quadro a toma)
        let resposta = banco::ResultadoConsulta {
            resultados: Vec::new(),
            alterou_esquema: false,
            alterou_dados: false,
            transacao_aberta: true,
        };
        e.tarefas.push(Tarefa::Consulta(e.estado.banco_id(), Promessa::pronta(Ok(resposta))));
        assert_eq!(comandos_da_logica(&mut e, &ctx, &minimizada), restaurar);
        assert!(e.fechar_pendente);
        // o primeiro quadro da janela restaurada toma a resposta e só depois decide: pergunta da transação
        let saida = ctx.run_ui(egui::RawInput::default(), |ui| {
            let c = ui.ctx();
            e.pedido_de_fechar(c);
            e.receber(c);
            let acoes = std::mem::take(&mut e.pendentes);
            e.aplicar(c, None, acoes);
            e.decidir_fechar_pendente(c);
        });
        saida.drop_without_applying_deltas();
        assert!(e.estado.transacao_aberta());
        assert!(matches!(e.confirmado, Some(Confirmado::FecharApp { falta_transacao: false })));
        assert!(e.confirmacao.is_some());
    }

    /// A leitura de uma pasta que volta depois de outra pasta abrir é descartada; a da pasta aberta entra, e a mesma
    /// listagem de novo não muda o estado (a versão fica: nada repinta).
    #[test]
    fn leitura_de_outra_pasta_e_descartada() {
        let amb = estado::apoio_testes::Ambiente::vazio("leitura_de_outra_pasta");
        let mut e = editor_pronto(&amb);
        let ctx = egui::Context::default();
        let a = PathBuf::from(r"C:\dados\a");
        let b = PathBuf::from(r"C:\dados\b");
        e.estado.pasta = Some(PastaAberta::nova(b.clone()));
        let chave_a = prefs::chave_caminho(&a);
        let chave_b = prefs::chave_caminho(&b);
        let lida = |p: &Path| Listagem::montar(p.to_path_buf(), Vec::new(), 0, false, 500);
        e.chegou_a_pasta(&ctx, &chave_a, chave_a.clone(), Ok(lida(&a)));
        let p = e.estado.pasta.as_ref().expect("pasta");
        assert_eq!(p.chave, chave_b, "a pasta aberta continua a b");
        assert_eq!(p.nos.get(&chave_b), Some(&NoDaPasta::Lendo), "nada da a entrou");
        assert!(!p.nos.contains_key(&chave_a));
        e.chegou_a_pasta(&ctx, &chave_b, chave_b.clone(), Ok(lida(&b)));
        assert!(e.estado.pasta.as_ref().and_then(|p| p.listagem(&chave_b)).is_some());
        let versao = e.versao;
        e.chegou_a_pasta(&ctx, &chave_b, chave_b.clone(), Ok(lida(&b)));
        assert_eq!(e.versao, versao, "a mesma listagem não muda o estado");
        // a falha de uma subpasta fica no nó dela
        let sub = prefs::chave_caminho(b.join("sub"));
        e.chegou_a_pasta(&ctx, &chave_b, sub.clone(), Err(ErroBanco::novo("sem acesso")));
        assert_eq!(
            e.estado.pasta.as_ref().and_then(|p| p.nos.get(&sub)),
            Some(&NoDaPasta::Falhou("sem acesso".into()))
        );
        assert!(e.versao > versao);
    }

    #[test]
    fn textos_da_partida_da_pasta_e_do_arquivo() {
        assert_eq!(
            erro_da_partida_pasta(Path::new(r"C:\dados"), "A pasta não existe (ou foi movida): C:\\dados."),
            "Não foi possível abrir a pasta C:\\dados.\n\nA pasta não existe (ou foi movida): C:\\dados.\n\nO DataBox \
             abre sem ela."
        );
        assert!(erro_da_partida_arquivo(Path::new(r"C:\a.csv"), "x").starts_with("Não foi possível abrir o arquivo"));
        assert!(tem_extensao_de_banco(Path::new(r"C:\x\vendas.DB")) && tem_extensao_de_banco(Path::new("a.sqlite3")));
        assert!(!tem_extensao_de_banco(Path::new("a.csv")) && !tem_extensao_de_banco(Path::new("sem_extensao")));
    }

    #[test]
    fn pergunta_da_fila_que_some_sem_resposta_para_a_fila() {
        // a pergunta "Salvar as alterações em…?" que outra confirmação apaga (o banco trocado por fora fecha as
        // janelas) não vale como "Não salvar": a fila para e a aba alterada continua aberta
        struct Alterado;
        impl estado::Documento for Alterado {
            fn conteudo(&mut self, _ui: &mut egui::Ui, _ctx: &mut Contexto) {}
            fn alterado(&self) -> bool {
                true
            }
        }
        let amb = estado::apoio_testes::Ambiente::vazio("fila_sem_resposta");
        let mut e = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default());
        let ctx = egui::Context::default();
        let caminho = amb.pasta().join("vendas.csv");
        let id =
            e.abas.abrir(Rota::arquivo(&caminho), |_, _| Documento::de_arquivo_com(&caminho, Box::new(Alterado))).id();
        e.pedir_fechar_aba(&ctx, id);
        e.andar_fila(&ctx);
        assert!(matches!(e.confirmado, Some(Confirmado::SalvarAlterado(p)) if p == id), "a pergunta da aba");
        e.confirmacao = None;
        e.confirmado = None;
        e.andar_fila(&ctx);
        assert!(e.fila.is_none(), "a fila parou");
        assert!(e.abas.get(id).is_some(), "a aba alterada continua aberta");
    }
}
