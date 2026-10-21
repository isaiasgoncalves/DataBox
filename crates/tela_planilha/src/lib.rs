//! A planilha do Excel só para leitura (etapa 6 do DataBox; prancheta 17, `design\canvas\Planilha.dc.html`): a tela de
//! um `.xlsx` ou `.xlsm` aberto pela árvore, pelo Ctrl+O, pelos recentes ou ao soltar, como um [`estado::Documento`] que
//! se sustenta sozinho (a casca só escolhe a tela pelo tipo do arquivo: o `TelaArquivo` do editor).
//!
//! # API (o contrato com a casca)
//!
//! ```text
//! TelaPlanilha::nova(caminho: PathBuf) -> TelaPlanilha     não lê nada: a abertura começa no primeiro quadro
//! TelaPlanilha::nome() · caminho() · icone() (a folha XLSX) · modo() -> Modo { Inicial, Abrindo, Pronta, Falhou }
//! TelaPlanilha::tomar_pedido() -> Option<PedidoDeArquivo>   "Abrir no programa padrão" / "Mostrar na pasta"
//! impl estado::Documento for TelaPlanilha
//!   anatomia (faixa, rodapé, rolagem própria) · avisos (o arquivo mudou) · faixa · conteudo · rodape · ofertas/executar
//!   (só o Atualizar) · propriedades · status_do_arquivo · ocupada · em_segundo_plano · alterado() sempre falso
//! ```
//!
//! # O que a tela faz
//!
//! - **Abrir**: uma tarefa faz `Pasta::abrir`, escolhe a planilha (`primeira_planilha_visivel`: a ativa do
//!   arquivo, se estiver à vista) e lê o primeiro bloco de 128 linhas: a grade aparece na hora. O índice roda depois,
//!   noutra tarefa e noutra `Planilha` (o módulo `sessao`), com a barra no rodapé depois de 400 ms e o Cancelar; ao
//!   terminar, a que indexou passa a servir as páginas, e o tamanho, as mesclas e o filtro ficam os do arquivo.
//! - **Várias planilhas**: a faixa das planilhas embaixo da grade (`interface::pecas::folhas`); trocar
//!   cancela o índice da que sai e a guarda num cache curto (`folhas::EM_MEMORIA`, 3 abertas ao mesmo tempo): voltar a
//!   uma indexada é instantâneo. Ctrl+PageDown e Ctrl+PageUp trocam de dentro da grade.
//! - **Erros**: o que impede de abrir vira a ficha do arquivo com o aviso e os botões que resolvem
//!   (**Abrir no programa padrão**, **Mostrar na pasta**): a senha e o formato antigo com a frase do núcleo; o arquivo
//!   estragado com "O arquivo não é uma planilha válida: …"; o que sumiu com os botões desligados. O erro de uma planilha
//!   só fica dentro da grade dela, e as outras continuam.
//! - **Sem planilha à vista**: a tela abre sem grade, com a frase e a faixa vazia; as Propriedades têm a caixa
//!   "Mostrar planilhas ocultas".
//!
//! # Testes
//!
//! `cargo test -p tela_planilha`: as regras puras nos módulos e `tests/` (a tela no `egui_kittest`, pelo
//! `estado::apoio_testes::palco_documento`, com arquivos `.xlsx` montados no próprio teste; as capturas `planilha-*` em
//! `capturas/rust/`). O de um milhão de linhas é `#[ignore]` (rode com `--release`).

#![forbid(unsafe_code)]

mod folhas;
mod fonte;
mod grade;
mod menu;
mod propriedades;
mod sessao;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use comum::ErroBanco;
use egui::{Align, Context, Id, Key, Layout, Modifiers, Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, vec2};
use estado::{
    Anatomia, Contexto, FonteDeColunas, InfoArquivo, Oferta, Ofertas, Promessa, Situacao, StatusDeArquivo, TipoDeFonte,
    Verbo,
};
use interface::fontes::{self, Peso};
use interface::formato::formatar_numero;
use interface::grade::RespostaNucleo;
use interface::icones::{self, EstadoIcone, Icone, TipoArquivo};
use interface::pecas::arranjo::em_linha;
use interface::pecas::avisos::{TomAviso, aviso_tom, carregando};
use interface::pecas::botao::Botao;
use interface::pecas::documento::Documento as DocumentoPeca;
use interface::pecas::folhas::{ALTURA_FAIXA_FOLHAS, FaixaDeFolhas, Folha};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::progresso::barra_progresso;
use interface::pecas::texto::{Trecho, galley, galley_caminho, texto, texto_rico};
use interface::pecas::toast::{toast_concluido, toast_erro};
use interface::tema::paleta;
use planilha::{LINHAS_POR_BLOCO, Pagina, Pasta, Planilha, SistemaDatas, nome_da_coluna, referencia_a1};

pub use crate::folhas::{AbaDaFolha, Contagem, EM_MEMORIA, FolhaDaFaixa, folhas_da_faixa};
pub use crate::fonte::{
    DECIMAL_PADRAO, FontePlanilha, LINHAS_DA_HEURISTICA, ler_colunas, numero_cru, parece_ter_cabecalho,
    texto_para_levar,
};
pub use crate::grade::{
    CARACTERES_NA_DICA, Grade, LARGURA_NUMERO, OPACIDADE_FANTASMA, alinhamento_do_valor, dica_da_celula,
};
pub use crate::menu::{COPIAR_COLUNA, MSG_ABRA_UM_CSV, TRAZER_DESTA_PLANILHA, rotulo_de_copiar};
pub use crate::propriedades::{COPIAR_A_FORMULA, MSG_FILTRO, MSG_FORMULAS, MSG_MACROS};
pub use crate::sessao::{
    BLOCOS_GUARDADOS, ESPERA_DA_BARRA, Mudancas, ResumoDoIndice, SessaoDaFolha, bloco_de, frase_da_folha,
};

/// Os rótulos dos botões da ficha (os mesmos da ficha do arquivo).
pub const ABRIR_NO_PADRAO: &str = "Abrir no programa padrão";
pub const MOSTRAR_NA_PASTA: &str = "Mostrar na pasta";
/// A frase da planilha sem nenhuma aba com células.
pub const MSG_SEM_PLANILHAS: &str = "Este arquivo não tem planilhas.";
/// A frase da planilha com todas as abas ocultas.
pub const MSG_SEM_PLANILHA_A_VISTA: &str =
    "Este arquivo não tem planilhas à vista. Ligue \"Mostrar planilhas ocultas\" nas Propriedades.";
/// O rótulo da barra e o botão de cancelar o índice.
pub const LENDO_A_PLANILHA: &str = "Lendo a planilha";
pub const CANCELAR: &str = "Cancelar";
/// A largura da barra de progresso do rodapé (`.prog .barra{width:160px}`).
const LARGURA_BARRA: f32 = 160.0;
/// O espaço do conteúdo da ficha (`gap:18px`).
const ESPACO_FICHA: f32 = 18.0;

/// O que os botões da ficha e da faixa pedem à casca (tomado por [`TelaPlanilha::tomar_pedido`]; a casca abre o
/// programa ou o Explorador de Arquivos numa tarefa, como faz com a ficha).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedidoDeArquivo {
    /// O arquivo no programa associado.
    AbrirNoPadrao(PathBuf),
    /// O Explorador de Arquivos com o arquivo selecionado.
    MostrarNaPasta(PathBuf),
    /// O "Trazer colunas desta planilha" do menu do cabeçalho: abrir o "Trazer colunas" do CSV aberto com esta origem
    /// já escolhida (o rótulo dela, o mesmo do [`estado::FonteDeColunas::rotulo`]).
    TrazerColunas(String),
}

/// Em que corpo a tela está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Antes do primeiro quadro: nada foi pedido ainda.
    Inicial,
    /// A tarefa abre o arquivo.
    Abrindo,
    /// A pasta abriu (com ou sem planilha à vista).
    Pronta,
    /// O arquivo não abriu: a ficha com o aviso.
    Falhou,
}

/// O que a abertura leu: o tamanho e a data do arquivo (também quando não abriu) e a pasta, com a primeira planilha.
struct Aberta {
    info: InfoArquivo,
    pasta: Result<(Pasta, Option<Inicial>), ErroBanco>,
}

/// A planilha que abre com a pasta e o primeiro bloco dela (ou o erro só dela).
struct Inicial {
    folha: usize,
    leitura: Result<(Planilha, Pagina), ErroBanco>,
}

fn ler_info(caminho: &Path) -> InfoArquivo {
    let meta = std::fs::metadata(caminho).ok();
    InfoArquivo {
        caminho: caminho.to_path_buf(),
        tamanho: meta.as_ref().map(std::fs::Metadata::len),
        modificado: meta.and_then(|m| m.modified().ok()),
    }
}

/// A abertura (numa tarefa): o arquivo, a pasta e o primeiro bloco da planilha escolhida (`preferida`, pelo nome, no
/// Atualizar; senão a primeira à vista).
fn abrir(caminho: &Path, preferida: Option<&str>) -> Aberta {
    let info = ler_info(caminho);
    let pasta = Pasta::abrir(caminho).map(|pasta| {
        let pelo_nome = preferida
            .and_then(|n| pasta.folhas().iter().position(|f| f.nome == n && f.tipo == planilha::TipoFolha::Planilha));
        let inicial = pelo_nome.or_else(|| pasta.primeira_planilha_visivel()).map(|folha| Inicial {
            folha,
            leitura: pasta.planilha(folha).and_then(|mut p| {
                let pagina = p.pagina(1, LINHAS_POR_BLOCO)?;
                Ok((p, pagina))
            }),
        });
        (pasta, inicial)
    });
    Aberta { info, pasta }
}

/// A frase da ficha, o tom do aviso e se os botões ficam ligados, para um erro de abertura.
pub fn frase_do_erro(caminho: &Path, erro: &ErroBanco) -> (String, TomAviso, bool) {
    let m = erro.mensagem.as_str();
    if erro.status == 404 {
        return (format!("O arquivo não existe (ou foi movido): {}.", caminho.display()), TomAviso::Erro, false);
    }
    // a senha e o formato antigo (`.xls`, `.xlsb` com a extensão trocada): a frase do núcleo, que já diz o que fazer
    if m.contains("protegida por senha") || m.contains("formato antigo") || m.contains(".xlsb") {
        return (m.to_owned(), TomAviso::Atencao, true);
    }
    if erro.status == 403 {
        return (m.to_owned(), TomAviso::Erro, true);
    }
    let motivo = m.strip_prefix("A planilha está corrompida: ").map_or_else(|| minuscula(m), str::to_owned);
    (format!("O arquivo não é uma planilha válida: {motivo}"), TomAviso::Erro, true)
}

/// A frase com a primeira letra minúscula (para entrar depois de dois pontos).
fn minuscula(frase: &str) -> String {
    let mut chars = frase.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// A pasta aberta: as planilhas da faixa, a da tela e as guardadas.
struct Pronta {
    pasta: Pasta,
    /// A caixa "Mostrar planilhas ocultas" (vale para o documento aberto; não é guardada).
    ocultas: bool,
    faixa: Vec<FolhaDaFaixa>,
    aba: Option<AbaDaFolha>,
    cache: folhas::Cache,
}

enum Corpo {
    Inicial,
    Abrindo(Promessa<Aberta>),
    Pronta(Box<Pronta>),
    Falhou(ErroBanco),
}

/// A tela de uma planilha do Excel (veja o topo do módulo).
pub struct TelaPlanilha {
    caminho: PathBuf,
    nome: String,
    /// O que dá os ids da grade e da faixa (um por arquivo).
    sal: Id,
    corpo: Corpo,
    info: InfoArquivo,
    pedido: Option<PedidoDeArquivo>,
    /// A planilha e a célula para onde o Atualizar volta.
    voltar_a: Option<(String, Option<(usize, usize)>)>,
    /// O índice começa sozinho quando a planilha abre (os testes o seguram para começar e cancelar na hora certa).
    indice_automatico: bool,
    /// O Ctrl+C de colunas lendo a planilha numa tarefa: o texto e a frase do toast.
    copia: Option<Promessa<(String, String)>>,
    /// A janela tinha o foco no quadro anterior (a volta do foco confere se o arquivo mudou no disco).
    foco_antes: bool,
    checagem: Option<Promessa<InfoArquivo>>,
    /// O aviso de topo "O arquivo mudou no disco." está à vista, com o que a conferência viu no disco.
    mudou_no_disco: bool,
    visto_no_disco: Option<InfoArquivo>,
    /// A origem de colunas da planilha da tela (montada quando ela muda) e o que a montou.
    fonte: Option<(ChaveDaFonte, Arc<dyn FonteDeColunas>)>,
}

/// O que a origem de colunas descreve: a aba, as colunas à vista e a última linha conhecida.
type ChaveDaFonte = (usize, Vec<u32>, Option<u32>);

/// O aviso de topo do arquivo que mudou e os botões dele.
pub const MSG_MUDOU_NO_DISCO: &str = "O arquivo mudou no disco.";
pub const LER_DE_NOVO: &str = "Ler de novo";
pub const IGNORAR: &str = "Ignorar";
/// O toast da cópia de uma célula ou de linhas.
pub const MSG_COPIADO: &str = "Copiado.";

impl TelaPlanilha {
    /// A tela do arquivo `caminho`. Não lê nada aqui: a abertura é uma tarefa e começa no primeiro quadro.
    pub fn nova(caminho: PathBuf) -> TelaPlanilha {
        let nome =
            caminho.file_name().map_or_else(|| caminho.display().to_string(), |n| n.to_string_lossy().into_owned());
        TelaPlanilha {
            sal: Id::new(("databox::tela_planilha", &caminho)),
            info: InfoArquivo { caminho: caminho.clone(), tamanho: None, modificado: None },
            caminho,
            nome,
            corpo: Corpo::Inicial,
            pedido: None,
            voltar_a: None,
            indice_automatico: true,
            copia: None,
            foco_antes: true,
            checagem: None,
            mudou_no_disco: false,
            visto_no_disco: None,
            fonte: None,
        }
    }

    /// O aviso "O arquivo mudou no disco." está à vista.
    pub fn mudou_no_disco(&self) -> bool {
        self.mudou_no_disco
    }

    /// O índice não começa sozinho: só com [`TelaPlanilha::iniciar_indice`] (os testes).
    #[doc(hidden)]
    pub fn com_indice_manual(mut self) -> TelaPlanilha {
        self.indice_automatico = false;
        self
    }

    /// Começa o índice da planilha da tela (os testes, com o [`TelaPlanilha::com_indice_manual`]).
    #[doc(hidden)]
    pub fn iniciar_indice(&mut self, egui: &Context) {
        if let Some(aba) = self.pronta_mut().and_then(|p| p.aba.as_mut()) {
            aba.sessao.iniciar_indice(egui);
        }
    }

    /// Cancela o índice da planilha da tela (o Cancelar do rodapé, quando `pela_pessoa`).
    pub fn cancelar_indice(&mut self, pela_pessoa: bool) {
        if let Some(aba) = self.pronta_mut().and_then(|p| p.aba.as_mut()) {
            aba.sessao.cancelar_indice(pela_pessoa);
        }
    }

    /// O nome do arquivo (o da aba).
    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// A folha XLSX (a da aba e a da faixa).
    pub fn icone(&self) -> Icone {
        Icone::Folha(TipoArquivo::Xlsx)
    }

    pub fn modo(&self) -> Modo {
        match &self.corpo {
            Corpo::Inicial => Modo::Inicial,
            Corpo::Abrindo(_) => Modo::Abrindo,
            Corpo::Pronta(_) => Modo::Pronta,
            Corpo::Falhou(_) => Modo::Falhou,
        }
    }

    /// O erro que impediu de abrir o arquivo (o corpo *Falhou*), na frase da ficha.
    pub fn falha(&self) -> Option<String> {
        match &self.corpo {
            Corpo::Falhou(e) => Some(frase_do_erro(&self.caminho, e).0),
            _ => None,
        }
    }

    /// O que os botões da ficha pediram desde a última chamada (a casca cumpre).
    pub fn tomar_pedido(&mut self) -> Option<PedidoDeArquivo> {
        self.pedido.take()
    }

    fn pronta(&self) -> Option<&Pronta> {
        match &self.corpo {
            Corpo::Pronta(p) => Some(p),
            _ => None,
        }
    }

    fn pronta_mut(&mut self) -> Option<&mut Pronta> {
        match &mut self.corpo {
            Corpo::Pronta(p) => Some(p),
            _ => None,
        }
    }

    /// A planilha da tela (a sessão de leitura dela).
    pub fn sessao(&self) -> Option<&SessaoDaFolha> {
        self.pronta()?.aba.as_ref().map(|a| &a.sessao)
    }

    /// A grade da planilha da tela.
    pub fn grade(&self) -> Option<&Grade> {
        self.pronta()?.aba.as_ref().map(|a| &a.grade)
    }

    /// A grade da planilha da tela, para mexer (os testes escolhem a célula).
    pub fn grade_mut(&mut self) -> Option<&mut Grade> {
        self.pronta_mut()?.aba.as_mut().map(|a| &mut a.grade)
    }

    /// A aba (na pasta) da planilha da tela.
    pub fn folha_ativa(&self) -> Option<usize> {
        self.sessao().map(|s| s.folha)
    }

    /// O nome da planilha da tela.
    pub fn nome_da_folha(&self) -> Option<&str> {
        self.sessao().map(|s| s.nome.as_str())
    }

    /// As planilhas que a faixa mostra, na ordem.
    pub fn folhas_na_faixa(&self) -> Vec<FolhaDaFaixa> {
        self.pronta().map(|p| p.faixa.clone()).unwrap_or_default()
    }

    /// As abas guardadas no cache, da mais recente à mais antiga.
    pub fn folhas_no_cache(&self) -> Vec<usize> {
        self.pronta().map(|p| p.cache.folhas()).unwrap_or_default()
    }

    /// A caixa "Mostrar planilhas ocultas".
    pub fn mostrando_ocultas(&self) -> bool {
        self.pronta().is_some_and(|p| p.ocultas)
    }

    /// Liga ou desliga a caixa "Mostrar planilhas ocultas" (a faixa muda; sem planilha na tela, abre a primeira).
    pub fn mostrar_ocultas(&mut self, egui: &Context, sim: bool) {
        let sal = self.sal;
        let Some(p) = self.pronta_mut() else { return };
        if p.ocultas == sim {
            return;
        }
        p.ocultas = sim;
        p.faixa = folhas_da_faixa(&p.pasta, sim);
        let ativa = p.aba.as_ref().map(|a| a.sessao.folha);
        let na_faixa = ativa.is_some_and(|f| p.faixa.iter().any(|x| x.folha == f));
        if !na_faixa {
            // a da tela sumiu da faixa (era oculta), ou não havia nenhuma: a primeira da faixa
            match p.faixa.first().map(|f| f.folha) {
                Some(f) => trocar(p, egui, sal, f),
                None => {
                    if let Some(a) = p.aba.take() {
                        p.cache.guardar(a);
                    }
                }
            }
        }
    }

    /// Troca a planilha da tela pela aba `folha` da pasta (a anterior vai para o cache, com o índice cancelado).
    pub fn escolher_folha(&mut self, egui: &Context, folha: usize) {
        let sal = self.sal;
        if let Some(p) = self.pronta_mut() {
            trocar(p, egui, sal, folha);
        }
    }

    /// A planilha anterior (`-1`) ou a seguinte (`+1`) da faixa, sem dar a volta.
    fn andar_na_faixa(&mut self, egui: &Context, passo: isize) {
        let Some(p) = self.pronta() else { return };
        let atual = p.aba.as_ref().and_then(|a| p.faixa.iter().position(|f| f.folha == a.sessao.folha));
        let Some(i) = atual else { return };
        let j = i as isize + passo;
        if let Some(destino) = usize::try_from(j).ok().and_then(|j| p.faixa.get(j)).map(|f| f.folha) {
            self.escolher_folha(egui, destino);
        }
    }

    /// Lê o arquivo de novo (o Atualizar): volta à mesma planilha e à mesma célula quando ainda existem.
    fn atualizar(&mut self, egui: &Context) {
        if matches!(self.corpo, Corpo::Abrindo(_)) {
            return;
        }
        self.voltar_a = self.sessao().map(|s| (s.nome.clone(), self.grade().and_then(|g| g.nucleo.selecao())));
        self.iniciar_abertura(egui);
    }

    fn iniciar_abertura(&mut self, egui: &Context) {
        let caminho = self.caminho.clone();
        let preferida = self.voltar_a.as_ref().map(|(n, _)| n.clone());
        self.corpo = Corpo::Abrindo(Promessa::rodar(egui, move || Ok(abrir(&caminho, preferida.as_deref()))));
    }

    // ---------------------------------------------------------------------------------------- a cada quadro

    /// O que acontece antes de qualquer desenho: a abertura que chegou (ou a pedida agora), as páginas e o índice.
    fn sincronizar(&mut self, egui: &Context) {
        let automatico = self.indice_automatico;
        if matches!(self.corpo, Corpo::Inicial) {
            self.iniciar_abertura(egui);
        }
        if let Corpo::Abrindo(p) = &mut self.corpo
            && let Some(r) = p.tomar()
        {
            match r {
                Ok(aberta) => self.instalar(aberta, egui),
                Err(e) => self.corpo = Corpo::Falhou(e),
            }
            egui.request_repaint();
        }
        self.conferir_o_disco(egui);
        if let Some(r) = self.copia.as_mut().and_then(Promessa::tomar) {
            self.copia = None;
            match r {
                Ok((texto, frase)) => {
                    egui.copy_text(texto);
                    toast_concluido(egui, frase);
                }
                Err(e) => toast_erro(egui, e.mensagem),
            }
        }
        let Corpo::Pronta(p) = &mut self.corpo else { return };
        p.cache.passo(egui);
        let Some(aba) = &mut p.aba else { return };
        let _ = aba.sessao.passo(egui);
        let s = &mut aba.sessao;
        if automatico && s.pronta() && !s.indexada() && !s.indexando() && !s.indice_cancelado && s.erro.is_none() {
            s.iniciar_indice(egui);
        }
        aba.grade.sincronizar(&aba.sessao);
        if let Some(espera) = aba.sessao.espera_da_barra() {
            egui.request_repaint_after(espera);
        }
        // a origem do "Trazer colunas", refeita só quando a planilha, as colunas ou o tamanho mudam
        let (s, g) = (&aba.sessao, &aba.grade);
        let ultima = s.resumo.map(|r| r.ultima_linha);
        let em_dia =
            self.fonte.as_ref().is_some_and(|((f, c, u), _)| *f == s.folha && c == &g.do_arquivo && *u == ultima);
        if s.pronta() && s.linha(1).is_some() && !em_dia {
            let primeiras: Vec<&planilha::Linha> =
                (1..=LINHAS_DA_HEURISTICA as u32).filter_map(|n| s.linha(n)).collect();
            let cabecalho = parece_ter_cabecalho(s.pasta(), &primeiras, &g.do_arquivo);
            let primeira_linha = g
                .do_arquivo
                .iter()
                .map(|&c| s.linha(1).and_then(|l| l.celula(c)).map(|cel| s.pasta().exibir(cel)).unwrap_or_default())
                .collect();
            let fonte = FontePlanilha::nova(
                s.pasta().clone(),
                s.folha,
                g.do_arquivo.clone(),
                primeira_linha,
                ultima,
                cabecalho,
            );
            self.fonte = Some(((s.folha, g.do_arquivo.clone(), ultima), Arc::new(fonte)));
        }
    }

    /// A volta do foco da janela: confere numa tarefa se o tamanho ou a data do arquivo mudaram.
    fn conferir_o_disco(&mut self, egui: &Context) {
        let foco = egui.input(|i| i.focused);
        let voltou = foco && !self.foco_antes;
        self.foco_antes = foco;
        if voltou && self.checagem.is_none() && matches!(self.corpo, Corpo::Pronta(_)) {
            let caminho = self.caminho.clone();
            self.checagem = Some(Promessa::rodar(egui, move || Ok(ler_info(&caminho))));
        }
        if let Some(r) = self.checagem.as_mut().and_then(Promessa::tomar) {
            self.checagem = None;
            if let Ok(agora) = r
                && (agora.tamanho, agora.modificado) != (self.info.tamanho, self.info.modificado)
            {
                self.mudou_no_disco = true;
                self.visto_no_disco = Some(agora);
                egui.request_repaint();
            }
        }
    }

    /// O Ctrl+C, por prioridade: o intervalo de células (o texto de cada uma como vai para fora, das linhas que já
    /// chegaram), as colunas selecionadas (lidas numa tarefa, inteiras, pela regra do
    /// `fonte`), as linhas selecionadas (as que já chegaram) e a célula ativa (o texto de exibição).
    fn copiar(&mut self, egui: &Context) {
        let Some(p) = self.pronta() else { return };
        let Some(aba) = &p.aba else { return };
        let (g, s) = (&aba.grade, &aba.sessao);
        // o intervalo da grade (as linhas que já chegaram), como o Excel copia
        if let Some(iv) = g.nucleo.intervalo().filter(interface::grade::Intervalo::varias) {
            let texto = interface::grade::tsv_do_intervalo(iv, |l, c| {
                let coluna = g.coluna_do_arquivo(c)?;
                let celula = s.linha(l as u32 + 1)?.celula(coluna)?;
                Some(texto_para_levar(s.pasta(), celula, DECIMAL_PADRAO))
            });
            egui.copy_text(texto);
            toast_concluido(egui, interface::grade::frase_do_intervalo_copiado(iv));
            return;
        }
        if !g.colunas_selecionadas.is_empty() {
            if self.copia.is_some() {
                return;
            }
            let colunas: Vec<u32> = g.colunas_selecionadas.iter().copied().collect();
            let (pasta, folha) = (s.pasta().clone(), s.folha);
            // só a última linha do índice; sem ele, a leitura vai até o fim do XML (o tamanho de antes do índice é a
            // dimensão declarada ou o que já se viu, e cortaria a coluna ou a completaria com vazios)
            let ultima = s.resumo.map(|r| r.ultima_linha).filter(|&u| u > 0);
            self.copia = Some(Promessa::rodar(egui, move || {
                let valores = ler_colunas(&pasta, folha, &colunas, 1, ultima, DECIMAL_PADRAO, &mut |_, _| Ok(()))?;
                let n = valores.first().map_or(0, Vec::len);
                let texto = tabular::colunas::escrever_tsv((0..n).map(|i| valores.iter().map(move |v| v[i].as_str())));
                let linhas = format!("{} {}", formatar_numero(n), if n == 1 { "linha" } else { "linhas" });
                let frase = match colunas.as_slice() {
                    [c] => format!("Coluna {} copiada · {linhas}", nome_da_coluna(*c)),
                    v => format!("{} colunas copiadas · {linhas}", formatar_numero(v.len())),
                };
                Ok((texto, frase))
            }));
            return;
        }
        if !g.linhas_selecionadas.is_empty() {
            let colunas = &g.do_arquivo;
            let linhas: Vec<Vec<String>> = g
                .linhas_selecionadas
                .iter()
                .filter_map(|&l| s.linha(l))
                .map(|l| {
                    colunas
                        .iter()
                        .map(|&c| {
                            l.celula(c).map(|cel| texto_para_levar(s.pasta(), cel, DECIMAL_PADRAO)).unwrap_or_default()
                        })
                        .collect()
                })
                .collect();
            egui.copy_text(tabular::colunas::escrever_tsv(linhas.iter().map(|l| l.iter().map(String::as_str))));
            toast_concluido(egui, MSG_COPIADO);
            return;
        }
        if let Some((l, c)) = g.celula_ativa() {
            let texto =
                s.linha(l).and_then(|linha| linha.celula(c)).map(|cel| s.pasta().exibir(cel)).unwrap_or_default();
            egui.copy_text(texto);
            toast_concluido(egui, MSG_COPIADO);
        }
    }

    fn instalar(&mut self, aberta: Aberta, egui: &Context) {
        self.info = aberta.info;
        self.mudou_no_disco = false;
        match aberta.pasta {
            Err(e) => self.corpo = Corpo::Falhou(e),
            Ok((pasta, inicial)) => {
                let ocultas = self.pronta().is_some_and(|p| p.ocultas)
                    || inicial.as_ref().is_some_and(|i| {
                        pasta.folhas().get(i.folha).is_some_and(|f| f.estado != planilha::Estado::Visivel)
                    });
                let faixa = folhas_da_faixa(&pasta, ocultas);
                let aba = inicial.map(|i| {
                    let sessao = match i.leitura {
                        Ok((planilha, pagina)) => SessaoDaFolha::ja_aberta(&pasta, i.folha, planilha, pagina),
                        Err(e) => {
                            let nome = pasta.folhas().get(i.folha).map(|f| f.nome.clone()).unwrap_or_default();
                            SessaoDaFolha::falhou(&pasta, i.folha, frase_da_folha(&nome, &e))
                        }
                    };
                    let mut grade = Grade::nova((self.sal, i.folha));
                    grade.sincronizar(&sessao);
                    AbaDaFolha { sessao, grade }
                });
                let mut pronta = Pronta { pasta, ocultas, faixa, aba, cache: folhas::Cache::default() };
                if let Some((nome, Some(celula))) = self.voltar_a.take()
                    && let Some(a) = &mut pronta.aba
                    && a.sessao.nome == nome
                {
                    a.grade.nucleo.selecionar(Some(celula));
                }
                self.corpo = Corpo::Pronta(Box::new(pronta));
                egui.request_repaint();
            }
        }
    }

    // ---------------------------------------------------------------------------------------- o desenho

    /// O corpo de um arquivo que não abriu: o aviso e os botões da ficha, centrados.
    fn desenhar_falha(&mut self, ui: &mut Ui) {
        let Corpo::Falhou(erro) = &self.corpo else { return };
        let (frase, tom, ligados) = frase_do_erro(&self.caminho, erro);
        ui.add_space(ESPACO_FICHA);
        aviso_tom(ui, &[Trecho::Normal(&frase)], tom);
        ui.add_space(ESPACO_FICHA);
        let (abrir, mostrar) = botoes_da_ficha(ui, self.sal, ligados);
        if abrir {
            self.pedido = Some(PedidoDeArquivo::AbrirNoPadrao(self.caminho.clone()));
        }
        if mostrar {
            self.pedido = Some(PedidoDeArquivo::MostrarNaPasta(self.caminho.clone()));
        }
    }

    /// A grade (ou o aviso dela) e a faixa das planilhas embaixo.
    fn desenhar_pronta(&mut self, ui: &mut Ui, ha_csv: bool) {
        let egui = ui.ctx().clone();
        let sal = self.sal;
        let rect = ui.available_rect_before_wrap();
        if !rect.is_finite() {
            // o quadro em que a planilha abriu começou com a anatomia da ficha (a página que rola): a grade, que tem a
            // rolagem dela, entra no próximo
            egui.request_repaint();
            return;
        }
        let rect_faixa = Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - ALTURA_FAIXA_FOLHAS), rect.max);
        let rect_grade = Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect_faixa.top()));
        let Corpo::Pronta(p) = &mut self.corpo else { return };
        let mut escolhida = None;
        let mut copiar = false;
        let mut trazer = false;
        {
            let mut ui_grade = ui.new_child(UiBuilder::new().id_salt((sal, "grade")).max_rect(rect_grade));
            ui_grade.set_clip_rect(rect_grade.intersect(ui.clip_rect()));
            match &mut p.aba {
                None => {
                    let frase = if Contagem::de(&p.pasta).planilhas == 0 {
                        MSG_SEM_PLANILHAS
                    } else {
                        MSG_SEM_PLANILHA_A_VISTA
                    };
                    ui_grade.add_space(ESPACO_FICHA);
                    aviso_tom(&mut ui_grade, &[Trecho::Normal(frase)], TomAviso::Atencao);
                }
                Some(aba) if aba.sessao.abrindo() => {
                    ui_grade.add_space(ESPACO_FICHA);
                    carregando(&mut ui_grade, "Lendo a planilha…");
                }
                Some(aba) if aba.sessao.erro.is_some() && !aba.sessao.pronta() => {
                    let frase = aba.sessao.erro.clone().unwrap_or_default();
                    ui_grade.add_space(ESPACO_FICHA);
                    aviso_tom(&mut ui_grade, &[Trecho::Normal(&frase)], TomAviso::Erro);
                }
                Some(aba) => {
                    let foco = ui_grade.memory(|m| m.has_focus(aba.grade.nucleo.id_corpo()));
                    let (resposta, visiveis) = aba.grade.mostrar(&mut ui_grade, &aba.sessao, rect_grade.height());
                    let visiveis = visiveis.unwrap_or(1..=LINHAS_POR_BLOCO);
                    let congeladas = aba.grade.nucleo.congelado_efetivo().linhas as u32;
                    aba.sessao.pedir(&egui, visiveis, congeladas);
                    if let Some(erro) = &aba.sessao.erro {
                        // a leitura de um bloco falhou depois de a planilha abrir: o aviso por cima da grade
                        let r = Rect::from_min_size(rect_grade.min + vec2(0.0, 40.0), vec2(rect_grade.width(), 60.0));
                        let mut ui_aviso = ui.new_child(UiBuilder::new().id_salt((sal, "erro")).max_rect(r));
                        aviso_tom(&mut ui_aviso, &[Trecho::Normal(erro)], TomAviso::Erro);
                    }
                    copiar = tratar_resposta(&mut aba.grade, &resposta, foco, &ui_grade);
                    let th = resposta.menu_cabecalho.and_then(|c| {
                        ui_grade.ctx().read_response(aba.grade.nucleo.id().with(("th", c))).map(|r| (c, r))
                    });
                    let id_corpo = aba.grade.nucleo.id_corpo();
                    let ctx_grade = ui_grade.ctx().clone();
                    let corpo = move || ctx_grade.read_response(id_corpo);
                    match menu::menu_do_cabecalho(&ui_grade, (sal, "menu_th"), &mut aba.grade, th, corpo, ha_csv) {
                        Some(menu::PedidoDoMenu::Copiar) => copiar = true,
                        Some(menu::PedidoDoMenu::Trazer) => trazer = true,
                        None => {}
                    }
                }
            }
        }
        // a faixa das planilhas, entre a grade e o rodapé
        let folhas: Vec<Folha<'_>> = p.faixa.iter().map(|f| Folha::new(&f.nome).estado(f.estado)).collect();
        let ativa = p.aba.as_ref().and_then(|a| p.faixa.iter().position(|f| f.folha == a.sessao.folha));
        let mut ui_faixa = ui.new_child(UiBuilder::new().id_salt((sal, "faixa")).max_rect(rect_faixa));
        let r = FaixaDeFolhas::new((sal, "folhas"), &folhas).ativa(ativa).mostrar(&mut ui_faixa);
        if let Some(i) = r.escolhida {
            escolhida = p.faixa.get(i).map(|f| f.folha);
        }
        ui.allocate_rect(rect, Sense::hover());
        // Ctrl+PageDown e Ctrl+PageUp: a planilha seguinte e a anterior (o núcleo da grade os deixa livres)
        let (seguinte, anterior) = ui.input_mut(|i| {
            (i.consume_key(Modifiers::COMMAND, Key::PageDown), i.consume_key(Modifiers::COMMAND, Key::PageUp))
        });
        if copiar {
            self.copiar(&egui);
        }
        if trazer && let Some((_, f)) = &self.fonte {
            self.pedido = Some(PedidoDeArquivo::TrazerColunas(f.rotulo()));
        }
        if let Some(f) = escolhida {
            self.escolher_folha(&egui, f);
        } else if seguinte {
            self.andar_na_faixa(&egui, 1);
        } else if anterior {
            self.andar_na_faixa(&egui, -1);
        }
    }

    // ---------------------------------------------------------------------------------------- faixa e rodapé

    fn faixa_do_arquivo(&mut self, ui: &mut Ui) {
        let e = &paleta(ui).estudio;
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        icones::pintar(ui.painter(), r, self.icone(), e.t2);
        if matches!(self.corpo, Corpo::Falhou(_)) {
            // a ficha: o nome e o caminho em `--t3`
            nome_forte(ui, &self.nome, 13.0);
            let caminho = self.caminho.display().to_string();
            let g = galley_caminho(ui, &caminho, fontes::fonte(12.0, Peso::Leve), e.t3, ui.available_width());
            let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
            ui.painter().galley(rect.min, g, e.t3);
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &caminho));
            return;
        }
        let folha = self.nome_da_folha().map(str::to_owned);
        let celula = self.pronta().and_then(|p| p.aba.as_ref()).and_then(|a| a.grade.referencia_ativa(&a.sessao.meta));
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            nome_forte(ui, &self.nome, 12.0);
            if let Some(f) = &folha {
                seta_da_migalha(ui);
                texto(ui, f, 12.0, e.t2);
            }
            // a célula ativa (a mesclada diz o intervalo: `A1:F1`)
            if let Some(c) = &celula {
                seta_da_migalha(ui);
                nome_forte(ui, c, 12.0);
            }
        });
        let mut abrir = false;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            abrir = ui.add(Botao::ghost(ABRIR_NO_PADRAO).pequeno()).clicked();
            chip_somente_leitura(ui);
        });
        if abrir {
            self.pedido = Some(PedidoDeArquivo::AbrirNoPadrao(self.caminho.clone()));
        }
    }

    fn rodape_da_planilha(&mut self, ui: &mut Ui) {
        let e = &paleta(ui).estudio;
        let Some(p) = self.pronta() else { return };
        let Some(aba) = &p.aba else {
            let c = Contagem::de(&p.pasta);
            let frase = if c.planilhas == 0 {
                "Nenhuma planilha".to_owned()
            } else {
                format!("{} · todas ocultas", plural(c.planilhas, "planilha", "planilhas"))
            };
            texto(ui, &frase, 12.0, e.t2);
            return;
        };
        let s = &aba.sessao;
        let (linhas, colunas) = s.tamanho();
        let mut contagem = format!(
            "{} · {}",
            plural(linhas as usize, "linha", "linhas"),
            plural(colunas as usize, "coluna", "colunas")
        );
        if linhas > 0 && colunas > 0 {
            contagem.push_str(&format!(" · A1:{}", referencia_a1(linhas, colunas - 1)));
        }
        texto(ui, &contagem, 12.0, e.t2);
        let mescladas = s.meta.mescladas.len();
        if mescladas > 0 {
            texto(ui, &format!("· {}", plural(mescladas, "mesclada", "mescladas")), 12.0, e.t3);
        }
        if let Some(frase) = selecao_em_palavras(&aba.grade) {
            texto(ui, &format!("· {frase}"), 12.0, e.t3);
        }
        let fracao = s.fracao_da_barra();
        let mut cancelar = false;
        if let Some(f) = fracao {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                cancelar = ui.add(Botao::ghost(CANCELAR).pequeno()).clicked();
                let (r, _) = ui.allocate_exact_size(vec2(LARGURA_BARRA, 8.0), Sense::hover());
                let mut barra = ui.new_child(UiBuilder::new().max_rect(r));
                barra_progresso(&mut barra, Some(f))
                    .widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, LENDO_A_PLANILHA));
                let pct = (f * 100.0).floor() as u32;
                texto(ui, &format!("{LENDO_A_PLANILHA}… {pct}%"), 12.0, e.t2);
            });
        }
        if cancelar {
            self.cancelar_indice(true);
        }
    }

    // ---------------------------------------------------------------------------------------- propriedades

    /// O que a seção Planilha mostra da planilha da tela.
    fn dados_da_planilha(&self) -> Option<propriedades::DadosDaPlanilha> {
        let p = self.pronta()?;
        let aba = p.aba.as_ref()?;
        let (s, g) = (&aba.sessao, &aba.grade);
        if !s.pronta() {
            return None;
        }
        let (linhas, colunas) = s.tamanho();
        let dimensao = if s.indexada() {
            (linhas > 0 && colunas > 0).then(|| format!("A1:{}", referencia_a1(linhas, colunas - 1)))
        } else {
            s.meta.dimensao.map(|d| d.to_string())
        };
        let congelado = s.meta.congelado.map_or((0, 0), |c| (c.linhas, c.colunas));
        let ocultas = g.colunas_ocultas();
        Some(propriedades::DadosDaPlanilha {
            nome: s.nome.clone(),
            estado: p.pasta.folhas().get(s.folha).map_or("visível", |f| propriedades::estado_em_palavras(f.estado)),
            dimensao: dimensao.unwrap_or_else(|| "—".to_owned()),
            linhas: formatar_numero(linhas as usize),
            colunas: formatar_numero(colunas as usize),
            mescladas: if s.indexada() { formatar_numero(s.meta.mescladas.len()) } else { "—".to_owned() },
            congelado: propriedades::congelado_em_palavras(congelado.0, congelado.1),
            filtro: s.meta.filtro.map(|f| f.to_string()),
            colunas_ocultas: if ocultas == 0 { "nenhuma".to_owned() } else { formatar_numero(ocultas) },
        })
    }

    fn secao_pasta(&mut self, ui: &mut Ui) {
        let egui = ui.ctx().clone();
        let Some(p) = self.pronta() else { return };
        let c = Contagem::de(&p.pasta);
        let sistema = match p.pasta.sistema_de_datas() {
            SistemaDatas::D1900 => "1900",
            SistemaDatas::D1904 => "1904",
        };
        let mut ocultas = p.ocultas;
        let _ = Secao::new(("propriedades::pasta", self.sal), "Pasta de trabalho").mostrar(ui, |ui| {
            ficha(ui, |d| {
                d.linha_texto("Planilhas", Some(&c.texto_planilhas()), "—");
                if let Some(outras) = c.texto_outras() {
                    d.linha_texto("Outras folhas", Some(&outras), "—");
                }
                d.linha_texto("Sistema de datas", Some(sistema), "—");
            });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                interface::pecas::campos::check(ui, &mut ocultas, "Mostrar planilhas ocultas");
            });
            ui.add_space(14.0);
        });
        if ocultas != p.ocultas {
            self.mostrar_ocultas(&egui, ocultas);
        }
    }
}

/// O que o rodapé diz da seleção: "coluna D selecionada", "3 colunas selecionadas", "linha 9 selecionada".
fn selecao_em_palavras(g: &Grade) -> Option<String> {
    let (cols, lins) = (&g.colunas_selecionadas, &g.linhas_selecionadas);
    match (cols.len(), lins.len()) {
        (1, _) => cols.first().map(|c| format!("coluna {} selecionada", nome_da_coluna(*c))),
        (n, _) if n > 1 => Some(format!("{} colunas selecionadas", formatar_numero(n))),
        (_, 1) => lins.first().map(|l| format!("linha {} selecionada", formatar_numero(*l as usize))),
        (_, n) if n > 1 => Some(format!("{} linhas selecionadas", formatar_numero(n))),
        _ => None,
    }
}

/// A seleção pelo que a grade respondeu: o clique numa célula tira a das colunas e linhas; o do `th`
/// escolhe a coluna e o do número a linha (Ctrl e Shift somam); com o foco na grade, Ctrl+Espaço escolhe a coluna da
/// célula ativa e Shift+Espaço a linha. Devolve se pediu o Ctrl+C.
fn tratar_resposta(grade: &mut Grade, r: &RespostaNucleo, foco: bool, ui: &Ui) -> bool {
    if r.clicou.is_some() || r.intervalo_mudou {
        grade.limpar_selecao();
    }
    // escolher colunas ou linhas desfaz o intervalo (o Ctrl+C leva o que foi escolhido por último)
    if let Some((c, toques)) = r.clicou_cabecalho {
        grade.clicar_coluna(c, toques);
        grade.nucleo.recolher_intervalo();
        grade.nucleo.focar(ui);
    }
    if let Some((l, toques)) = r.clicou_numero_da_linha {
        grade.clicar_linha(l, toques);
        grade.nucleo.recolher_intervalo();
        grade.nucleo.focar(ui);
    }
    if foco {
        let (coluna, linha) = ui.input_mut(|i| {
            (i.consume_key(Modifiers::COMMAND, Key::Space), i.consume_key(Modifiers::SHIFT, Key::Space))
        });
        if let Some((l, c)) = grade.nucleo.selecao() {
            let toques = interface::grade::Toques::default();
            if coluna {
                grade.clicar_coluna(c, toques);
            } else if linha {
                grade.clicar_linha(l, toques);
            }
        }
    }
    // o Ctrl+C com a coluna ou a linha escolhida e sem célula ativa: o núcleo só o passa com uma célula ativa
    r.copiar || (foco && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy))))
}

/// Troca a planilha da tela (veja [`TelaPlanilha::escolher_folha`]).
fn trocar(p: &mut Pronta, egui: &Context, sal: Id, folha: usize) {
    if p.aba.as_ref().is_some_and(|a| a.sessao.folha == folha) {
        return;
    }
    // primeiro tira do cache a que volta, depois guarda a que sai: com o cache cheio, guardar antes faria sair
    // justamente a mais antiga, que pode ser a que está voltando
    let aba = p.cache.tomar(folha).unwrap_or_else(|| AbaDaFolha {
        sessao: SessaoDaFolha::abrir(egui, &p.pasta, folha),
        grade: Grade::nova((sal, folha)),
    });
    if let Some(a) = p.aba.replace(aba) {
        p.cache.guardar(a);
    }
    egui.request_repaint();
}

/// "1 linha", "1.204 linhas".
fn plural(n: usize, um: &str, varios: &str) -> String {
    format!("{} {}", formatar_numero(n), if n == 1 { um } else { varios })
}

/// O nome em 600 `--t`.
fn nome_forte(ui: &mut Ui, nome: &str, tamanho: f32) {
    let e = &paleta(ui).estudio;
    let g = galley(ui, nome, fontes::fonte(tamanho, Peso::Medio), e.t);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, e.t);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, nome));
}

/// O chevron da migalha (`.migalha .ic`: 11px em `--t3`).
fn seta_da_migalha(ui: &mut Ui) {
    let e = &paleta(ui).estudio;
    let (r, _) = ui.allocate_exact_size(vec2(11.0, 11.0), Sense::hover());
    icones::pintar(ui.painter(), r, Icone::SetaDireita, e.t3);
}

/// O chip "Somente leitura" (`.so-leit`: 22px em pílula, `padding:0 10px 0 6px`, fundo `--h`, borda `--vidrob`, o
/// cadeado de 14 e o texto 11px / 600 em `--t2`).
fn chip_somente_leitura(ui: &mut Ui) {
    let e = &paleta(ui).estudio;
    let rotulo = Situacao::SoLeitura.rotulo();
    let g = galley(ui, rotulo, fontes::fonte(11.0, Peso::Medio), e.t2);
    let largura = 1.0 + 6.0 + 14.0 + 6.0 + g.size().x + 10.0 + 1.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(largura.ceil(), 22.0), Sense::hover());
    let pintor = ui.painter();
    pintor.rect_filled(rect, rect.height() / 2.0, e.h);
    pintor.rect_stroke(rect, rect.height() / 2.0, egui::Stroke::new(1.0, e.vidrob), egui::StrokeKind::Inside);
    let cadeado = Rect::from_min_size(egui::pos2(rect.left() + 7.0, rect.center().y - 7.0), vec2(14.0, 14.0));
    icones::pintar(pintor, cadeado, Icone::Estado(EstadoIcone::Bloqueado), e.t2);
    pintor.galley(egui::pos2(cadeado.right() + 6.0, rect.center().y - g.size().y / 2.0), g, e.t2);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, rotulo));
}

/// Os dois botões da ficha numa linha centrada (a largura dela vem do quadro anterior). Devolve (abrir, mostrar).
fn botoes_da_ficha(ui: &mut Ui, sal: Id, ligados: bool) -> (bool, bool) {
    let id = Id::new(("tela_planilha::botoes", sal));
    let largura_antes: Option<f32> = ui.ctx().data(|d| d.get_temp(id));
    let livre = ui.available_width();
    let (largura, abrir, mostrar) = ui
        .horizontal(|ui| {
            ui.add_space(largura_antes.map_or(0.0, |l| ((livre - l) / 2.0).max(0.0)));
            let inicio = ui.cursor().min.x;
            let abrir = ui.add(Botao::primario(ABRIR_NO_PADRAO).habilitado(ligados)).clicked();
            let mostrar = ui.add(Botao::new(MOSTRAR_NA_PASTA).habilitado(ligados)).clicked();
            (ui.min_rect().right() - inicio, abrir, mostrar)
        })
        .inner;
    if largura_antes.is_none_or(|l| (l - largura).abs() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(id, largura));
        ui.ctx().request_repaint();
    }
    (abrir, mostrar)
}

impl estado::Documento for TelaPlanilha {
    fn anatomia(&self) -> Anatomia {
        match self.corpo {
            Corpo::Pronta(_) => Anatomia::default().com_faixa().com_rodape().rolagem_propria(),
            Corpo::Inicial | Corpo::Abrindo(_) | Corpo::Falhou(_) => Anatomia::default().com_faixa(),
        }
    }

    /// O arquivo mudou no disco (a volta do foco o descobriu): o aviso de topo com **Ler de novo** e **Ignorar**.
    fn avisos(&mut self, doc: &mut DocumentoPeca, ui: &mut Ui, _ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        if !self.mudou_no_disco {
            return;
        }
        let (mut ler, mut ignorar) = (false, false);
        doc.aviso(ui, TomAviso::Informacao, |ui, cores| {
            texto_rico(ui, &[Trecho::Normal(MSG_MUDOU_NO_DISCO)], 12.0, cores.texto, cores.forte);
            em_linha(ui, 6.0, |ui| {
                ler = ui.add(Botao::primario(LER_DE_NOVO).pequeno()).clicked();
                ignorar = ui.add(Botao::new(IGNORAR).pequeno()).clicked();
            });
        });
        if ler {
            self.mudou_no_disco = false;
            self.atualizar(&egui);
        } else if ignorar {
            self.mudou_no_disco = false;
            // o que está no disco agora passa a ser o conhecido (a próxima volta do foco só avisa de outra mudança)
            if let Some(info) = self.visto_no_disco.take() {
                self.info = info;
            }
        }
    }

    fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        self.faixa_do_arquivo(ui);
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let ha_csv = ctx.fontes().iter().any(|f| f.tipo() == TipoDeFonte::Csv);
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        match &self.corpo {
            Corpo::Inicial | Corpo::Abrindo(_) => {
                ui.add_space(ESPACO_FICHA);
                carregando(ui, "Lendo o arquivo…");
            }
            Corpo::Falhou(_) => self.desenhar_falha(ui),
            Corpo::Pronta(_) => self.desenhar_pronta(ui, ha_csv),
        }
    }

    fn rodape(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        self.rodape_da_planilha(ui);
    }

    fn ofertas(&self, _estado: &estado::Estado) -> Ofertas {
        let livre = !matches!(self.corpo, Corpo::Abrindo(_) | Corpo::Inicial);
        Ofertas::default()
            .com(Verbo::Atualizar, Oferta::ligada().ligada_se(livre).com_dica(format!("Atualizar {} (F5)", self.nome)))
    }

    fn executar(&mut self, verbo: Verbo, egui: &Context, _ctx: &mut Contexto) {
        if verbo == Verbo::Atualizar {
            self.atualizar(egui);
        }
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        let info = self.info.clone();
        let xlsm = self.caminho.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("xlsm"));
        estado::documento::secao_arquivo_com(
            ui,
            ctx,
            &info,
            true,
            |_| {},
            |ui| {
                if xlsm {
                    let e = &paleta(ui).estudio;
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        texto(ui, MSG_MACROS, 12.0, e.t3);
                    });
                    ui.add_space(14.0);
                }
            },
        );
        let sal = self.sal;
        if let Some(dados) = self.dados_da_planilha() {
            propriedades::secao_planilha(ui, sal, &dados);
        }
        self.secao_pasta(ui);
        let mut formula = None;
        if let Some(aba) = self.pronta().and_then(|p| p.aba.as_ref())
            && let (Some((l, c)), Some(referencia)) =
                (aba.grade.celula_ativa(), aba.grade.referencia_ativa(&aba.sessao.meta))
        {
            let celula = aba.sessao.linha(l).and_then(|linha| linha.celula(c));
            formula = propriedades::secao_celula(ui, sal, aba.sessao.pasta(), &referencia, celula);
        }
        if let Some(f) = formula {
            ui.ctx().copy_text(f);
            toast_concluido(ui.ctx(), MSG_COPIADO);
        }
    }

    /// A ficha do arquivo que não abriu diz o tipo e o tamanho, como a ficha da etapa 4 ("XLSX · 184 KB").
    fn status(&self, _estado: &estado::Estado) -> Option<String> {
        matches!(self.corpo, Corpo::Falhou(_)).then(|| match self.info.tamanho {
            Some(t) => format!("XLSX · {}", interface::formato::formatar_bytes(t)),
            None => "XLSX".to_owned(),
        })
    }

    fn status_do_arquivo(&self, _estado: &estado::Estado) -> Option<StatusDeArquivo> {
        if matches!(self.corpo, Corpo::Falhou(_)) {
            return None;
        }
        let mut s = StatusDeArquivo::novo(self.nome.clone(), Situacao::SoLeitura).com_detalhe("XLSX");
        if let Some(f) = self.nome_da_folha() {
            s = s.com_detalhe(f);
        }
        if self.pronta().is_some_and(|p| p.pasta.sistema_de_datas() == SistemaDatas::D1904) {
            s = s.com_detalhe("1904");
        }
        Some(s)
    }

    fn fonte_de_colunas(&self) -> Option<Arc<dyn FonteDeColunas>> {
        self.fonte.as_ref().map(|(_, f)| Arc::clone(f))
    }

    fn ocupada(&self) -> bool {
        match &self.corpo {
            Corpo::Inicial | Corpo::Abrindo(_) => true,
            Corpo::Pronta(p) => p.aba.as_ref().is_some_and(|a| a.sessao.ocupada()) || p.cache.ocupado(),
            Corpo::Falhou(_) => false,
        }
    }

    fn em_segundo_plano(&mut self, egui: &Context, _ctx: &mut Contexto) {
        self.sincronizar(egui);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn frases_dos_erros_de_abertura() {
        let c = Path::new(r"C:\rh\folha.xlsx");
        let senha =
            ErroBanco::novo("A planilha está protegida por senha e o DataBox não abre arquivos criptografados.");
        let (f, tom, ligados) = frase_do_erro(c, &senha);
        assert!(f.starts_with("A planilha está protegida por senha") && tom == TomAviso::Atencao && ligados);
        let antigo = ErroBanco::novo("Este é o formato antigo do Excel (.xls, de 1997 a 2003), que o DataBox não lê.");
        assert_eq!(frase_do_erro(c, &antigo).1, TomAviso::Atencao);
        let estragado = ErroBanco::novo("A planilha está corrompida: falta a parte «xl/workbook.xml» do arquivo.");
        assert_eq!(
            frase_do_erro(c, &estragado).0,
            "O arquivo não é uma planilha válida: falta a parte «xl/workbook.xml» do arquivo."
        );
        let zip = ErroBanco::novo(
            "O arquivo está corrompido ou incompleto e não pôde ser lido como planilha do Excel (.xlsx).",
        );
        assert!(frase_do_erro(c, &zip).0.starts_with("O arquivo não é uma planilha válida: o arquivo está corrompido"));
        let sumiu = ErroBanco::com_status("Arquivo ou pasta não encontrado.", 404);
        let (f, _, ligados) = frase_do_erro(c, &sumiu);
        assert_eq!(f, r"O arquivo não existe (ou foi movido): C:\rh\folha.xlsx.");
        assert!(!ligados);
    }
}
