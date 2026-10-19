//! O Explorador (o `aside` da esquerda de `Main.dc.html` e `Dados.dc.html`):
//! o painel com o cabeçalho "Explorador" (Abrir pasta, dica "Abrir pasta (Ctrl+Shift+O)", e Recolher tudo) e a
//! árvore da pasta aberta e do banco aberto, com rolagem própria.
//!
//! **A pasta** (etapa 4, prancheta 1; o estado é `estado::Estado::pasta`, lido pela casca): a raiz com o sólido
//! `PastaSolida`, o nome e a seta, aberta no começo; cada pasta lida sob demanda (o primeiro "abrir" emite
//! `Acao::ListarPasta`, e a leitura volta para `PastaAberta::nos`): "Carregando…" enquanto lê, "Não foi possível ler a
//! pasta." com o motivo e "Tentar de novo" quando falha, "Nenhum arquivo que o DataBox abra." quando vazia e, depois
//! dos 500 primeiros, a linha "mais N itens" (sem foco, com a dica do limite). A dica de uma pasta é o caminho e, se
//! houver, "12 arquivos de outros tipos não aparecem.". Dentro dela, as pastas (o clique abre e fecha), os bancos (o
//! cilindro; o clique em qualquer parte abre o banco, `Acao::AbrirBanco`, que é a troca de hoje com a pergunta da
//! transação) e os arquivos de dados (o espaço da seta e a folha com a etiqueta; o clique abre a ficha,
//! `Acao::AbrirArquivoDeDados`; `.on` com a ficha à vista). O banco aberto fica no lugar dele com a subárvore de
//! sempre, um nível abaixo; aberto de fora da pasta (ou de um lugar que a árvore não mostra), ele é uma segunda raiz
//! no alto, igual à árvore sem pasta. Um banco ou a ficha de um arquivo de dentro da pasta abre os pais
//! dele. Cada pasta empurra o `push_id` da chave dela: o mesmo nome em duas pastas não divide o id do nó. Recolher
//! tudo deixa a raiz aberta.
//!
//! **O banco**: o nome em 600, sem ponto de estado (o "Aberto" saiu na revisão de 15/10/2026: verde e âmbar viram o
//! mesmo tom para quem tem daltonismo); com a transação aberta, o ícone Atenção de 16px com a dica "Transação
//! aberta" (prancheta `DadosClaro.dc.html`), e a dica do caminho; o clique foca a Visão geral. Tabelas (aberto no
//! começo), Visões e Sistema, com as contagens. Cada tabela tem Dados, Estrutura, Índices e gatilhos e Definição SQL;
//! cada visão, Dados e Definição SQL; os objetos do sistema abrem direto nos Dados. O clique no nome abre a aba dele
//! (ou foca a aberta) e abre o nó; o clique numa parte abre a aba da parte. A parte da aba à vista fica marcada
//! (`.no.on`), e os pais dela se abrem quando a aba à vista muda. Acima de [`LIMITE_SEM_FILTRO`] tabelas, o campo
//! "Filtrar tabelas" (sem acento nem caixa), que vale só para o banco em que foi digitado.
//!
//! **O menu de contexto** (decisão 16, prancheta 15; `pecas::menu_contexto::MenuContexto`, no clique direito e no
//! Shift+F10): banco e arquivo, Abrir · Mostrar na pasta · Copiar caminho (Ctrl+Shift+C); pasta, Atualizar · Mostrar
//! na pasta · Copiar caminho; a raiz, os da pasta e, depois de um fio, Fechar pasta. O Atualizar da raiz relê a raiz e
//! as pastas abertas (`Acao::AtualizarPasta`); o de uma subpasta, só ela (`Acao::ListarPasta`). Copiar caminho copia e
//! dá o toast "Caminho copiado."; Mostrar na pasta fica em [`Explorador::tomar_revelar`] para a casca abrir o
//! Explorador de Arquivos numa tarefa.
//!
//! **Teclado e acessibilidade**: cada nó é uma parada do Tab, com o papel `link` (`selected` no ativo,
//! `expanded` nos que abrem); ↑ e ↓ pelo foco do egui; → abre e ← fecha (o `No`); Enter e Espaço são o clique;
//! Shift+F10 abre o menu de contexto no nó com o foco e Ctrl+Shift+C copia o caminho dele (só com esses
//! modificadores: o AltGr não conta; no Windows o egui-winit entrega o Ctrl+Shift+C como `Event::Copy`). O rótulo de
//! uma pasta é "dados, pasta"; o das tabelas e das visões é o nome; as partes mostram só "Estrutura", com o rótulo
//! "clientes · Estrutura".
//!
//! **Vazio**: sem pasta e sem banco, só o título no cabeçalho e, no meio, o sólido da pasta de 48, "Nenhuma pasta
//! aberta.", "Abra uma pasta para ver os bancos, planilhas e textos que ela tem." e o botão "Abrir pasta" (prancheta 4).
//!
//! O que está aberto na árvore do banco fica por banco (pelo caminho do arquivo) e volta ao começo quando outro banco
//! abre; o que está aberto na pasta fica pela pasta e volta ao começo quando outra pasta abre.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use banco::{BancoInfo, ObjetosInfo, TipoObjeto};
use comum::arquivos::{Entrada, Listagem, Natureza, TipoDeArquivo};
use egui::text::LayoutJob;
use egui::{
    Align, Event, Key, Margin, Modifiers, Rect, Response, ScrollArea, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use estado::{Acao, ChaveAba, Estado, NoDaPasta, Parte, PastaAberta, Rota, icone_da_tabela};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_numero, sem_acento};
use interface::icones::{self, EstadoIcone, Icone, TipoArquivo};
use interface::pecas::arvore::{ALTURA_NO, MARGEM_NO, No, Seta, recuo};
use interface::pecas::avisos::carregando;
use interface::pecas::botao::Botao;
use interface::pecas::campos::Entrada as Campo;
use interface::pecas::casca::PainelMenu;
use interface::pecas::menu_contexto::MenuContexto;
use interface::pecas::painel::Painel;
use interface::pecas::texto::{formato, galley, hint, paragrafo, texto};
use interface::pecas::toast::toast_concluido;
use interface::tema::paleta;

use super::Textos;
use crate::visao_geral::MSG_CAMINHO_COPIADO;

/// Com mais tabelas que isto, o Explorador ganha o campo "Filtrar tabelas".
pub(crate) const LIMITE_SEM_FILTRO: usize = 12;
/// A chave do nó do banco.
const NO_BANCO: &str = "banco";
/// A dica do Abrir pasta do cabeçalho.
pub(crate) const DICA_ABRIR_PASTA: &str = "Abrir pasta (Ctrl+Shift+O)";
/// O atalho do Copiar caminho, no menu de contexto.
pub(crate) const ATALHO_COPIAR_CAMINHO: &str = "Ctrl+Shift+C";
/// O Explorador sem pasta e sem banco (prancheta 4).
pub(crate) const TEXTO_SEM_PASTA: &str = "Nenhuma pasta aberta.";
pub(crate) const EXPLICACAO_SEM_PASTA: &str = "Abra uma pasta para ver os bancos, planilhas e textos que ela tem.";
/// O botão do Explorador vazio.
pub(crate) const ABRIR_PASTA: &str = "Abrir pasta";
/// A leitura de uma pasta falhou (com o motivo embaixo).
pub(crate) const TEXTO_FALHA_NA_PASTA: &str = "Não foi possível ler a pasta.";
/// A pasta lida não tem nada que o app abra.
pub(crate) const TEXTO_PASTA_VAZIA: &str = "Nenhum arquivo que o DataBox abra.";
/// Quantas ações o Explorador guarda para os testes ([`Explorador::tomar_emitidas`]).
const GUARDADAS: usize = 32;
/// O espaço da seta e o `gap` até o ícone (`.seta` de 10px e 6px): onde o ícone dos irmãos começa.
const ESPACO_DA_SETA: f32 = 16.0;

/// Um grupo de objetos do banco na árvore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grupo {
    Tabelas,
    Visoes,
    Sistema,
}

impl Grupo {
    fn rotulo(self) -> &'static str {
        match self {
            Grupo::Tabelas => "Tabelas",
            Grupo::Visoes => "Visões",
            Grupo::Sistema => "Sistema",
        }
    }

    /// A chave do nó do grupo (no conjunto dos abertos).
    fn chave(self) -> &'static str {
        match self {
            Grupo::Tabelas => "g:tabelas",
            Grupo::Visoes => "g:visoes",
            Grupo::Sistema => "g:sistema",
        }
    }
}

/// A chave do nó de uma tabela ou visão (no conjunto dos abertos).
fn chave_objeto(nome: &str, e_visao: bool) -> String {
    format!("{}:{nome}", if e_visao { "v" } else { "t" })
}

/// O cilindro do banco: o cinza com o cadeado quando ele está aberto só para leitura.
pub(crate) fn icone_do_banco(b: &BancoInfo) -> Icone {
    if b.somente_leitura { Icone::BancoSoLeitura } else { Icone::Banco }
}

/// A folha de um arquivo de dados, com a etiqueta do tipo (CSV, XLSX, JSON, TXT).
pub(crate) fn icone_do_arquivo(tipo: TipoDeArquivo) -> Icone {
    Icone::Folha(match tipo {
        TipoDeArquivo::Csv => TipoArquivo::Csv,
        TipoDeArquivo::Xlsx => TipoArquivo::Xlsx,
        TipoDeArquivo::Json => TipoArquivo::Json,
        TipoDeArquivo::Texto => TipoArquivo::Txt,
    })
}

/// A ficha de um objeto pelo nome (a aba e o Explorador usam a mesma): visão, objeto do sistema ou a tabela pelo
/// tipo dela. Sem a lista de objetos (ainda chegando), a visão ou a tabela comum.
pub(crate) fn icone_do_objeto(objetos: Option<&ObjetosInfo>, nome: &str, e_visao: bool) -> Icone {
    if e_visao {
        return Icone::Visao;
    }
    let Some(o) = objetos else { return Icone::Tabela };
    if let Some(t) = o.tabelas.iter().find(|t| t.nome == nome) {
        icone_da_tabela(t.tipo, t.sem_rowid)
    } else if o.visoes.iter().any(|v| v.nome == nome) {
        Icone::Visao
    } else if o.sistema.iter().any(|s| s.nome == nome) {
        Icone::Sistema
    } else {
        Icone::Tabela
    }
}

/// O ícone de cada parte da tabela.
fn icone_da_parte(parte: Parte) -> Icone {
    match parte {
        Parte::Dados => Icone::Dados,
        Parte::Estrutura => Icone::Estrutura,
        Parte::Indices => Icone::Indices,
        Parte::Definicao => Icone::Definicao,
    }
}

/// O rótulo de uma pasta para o leitor de tela: "dados, pasta".
pub(crate) fn rotulo_da_pasta(nome: &str) -> String {
    format!("{nome}, pasta")
}

/// "12 arquivos de outros tipos não aparecem." (a dica de uma pasta com arquivos que a árvore esconde).
pub(crate) fn frase_das_ocultas(ocultas: usize) -> Option<String> {
    match ocultas {
        0 => None,
        1 => Some("1 arquivo de outro tipo não aparece.".to_string()),
        n => Some(format!("{} arquivos de outros tipos não aparecem.", formatar_numero(n))),
    }
}

/// A dica da pasta cuja leitura parou antes do fim (`Listagem::parcial`).
pub(crate) const FRASE_PARCIAL: &str = "A leitura parou antes do fim: alguns itens podem não aparecer.";

/// A linha depois do limite por pasta: "mais 37 itens".
pub(crate) fn frase_mais_itens(a_mais: usize) -> String {
    if a_mais == 1 { "mais 1 item".to_string() } else { format!("mais {} itens", formatar_numero(a_mais)) }
}

/// A dica da linha "mais N itens".
pub(crate) fn dica_do_limite(mostrados: usize) -> String {
    format!("Só os primeiros {} itens aparecem. Use Mostrar na pasta para ver o resto.", formatar_numero(mostrados))
}

/// Onde o banco aberto fica com uma pasta aberta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lugar {
    /// Na árvore da pasta, no lugar dele.
    NaArvore,
    /// Dentro da pasta, mas algum pai ainda não foi lido: ele aparece quando a leitura chegar.
    Esperando,
    /// Fora da pasta, ou num lugar que a árvore não mostra (oculto, depois do limite, numa pasta que falhou): a
    /// segunda raiz, no alto.
    Fora,
}

/// Onde o banco aberto (o arquivo `caminho`, de chave `chave`) fica na árvore da pasta `p`.
fn lugar_do_banco(p: &PastaAberta, caminho: &Path, chave: &str) -> Lugar {
    if !p.contem(caminho) {
        return Lugar::Fora;
    }
    // os pais, da raiz para baixo
    let mut pais: Vec<&Path> = caminho.ancestors().skip(1).take_while(|a| p.contem(a)).collect();
    pais.reverse();
    if pais.is_empty() {
        return Lugar::Fora;
    }
    for (i, pai) in pais.iter().enumerate() {
        let chave_pai = if i == 0 { p.chave.clone() } else { prefs::chave_caminho(pai) };
        let ultimo = i + 1 == pais.len();
        let filho = if ultimo { chave.to_string() } else { prefs::chave_caminho(pais[i + 1]) };
        match p.nos.get(&chave_pai) {
            None | Some(NoDaPasta::Lendo) => return Lugar::Esperando,
            Some(NoDaPasta::Falhou(_)) => return Lugar::Fora,
            Some(NoDaPasta::Pronto(l)) => match l.entrada(&filho) {
                Some(e) if (ultimo && e.natureza == Natureza::Banco) || (!ultimo && e.natureza.e_pasta()) => {}
                _ => return Lugar::Fora,
            },
        }
    }
    Lugar::NaArvore
}

/// O tipo de nó, para os itens do menu de contexto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoMenu {
    /// A raiz da pasta: os da pasta e o Fechar pasta.
    Raiz,
    /// Uma subpasta: Atualizar, Mostrar na pasta, Copiar caminho.
    Pasta,
    /// Um banco ou um arquivo de dados: Abrir, Mostrar na pasta, Copiar caminho.
    Arquivo,
}

/// O que a pessoa escolheu no menu de contexto (o Mostrar na pasta e o Copiar caminho o próprio Explorador cumpre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Escolha {
    Abrir,
    Atualizar,
    MostrarNaPasta,
    CopiarCaminho,
    FecharPasta,
}

/// Os itens do menu de contexto de um nó.
fn itens_do_menu(m: &mut PainelMenu<'_>, tipo: TipoMenu) -> Option<Escolha> {
    let mut escolha = None;
    if tipo == TipoMenu::Arquivo {
        // o "Abrir" da prancheta 15 leva só o espaço da silhueta
        if m.item("Abrir", None, false).clicked() {
            escolha = Some(Escolha::Abrir);
        }
    } else if m.silhueta(Icone::Atualizar).item("Atualizar", None, false).clicked() {
        escolha = Some(Escolha::Atualizar);
    }
    if m.silhueta(Icone::MostrarNaPasta).item("Mostrar na pasta", None, false).clicked() {
        escolha = Some(Escolha::MostrarNaPasta);
    }
    if m.silhueta(Icone::CopiarCaminho).item("Copiar caminho", Some(ATALHO_COPIAR_CAMINHO), false).clicked() {
        escolha = Some(Escolha::CopiarCaminho);
    }
    if tipo == TipoMenu::Raiz {
        m.separador();
        // como no menu Arquivo: fechar a pasta só tira a árvore (nada se perde), sem o vermelho de perigo
        if m.item("Fechar pasta", None, false).clicked() {
            escolha = Some(Escolha::FecharPasta);
        }
    }
    escolha
}

/// O Ctrl+Shift+C com o foco no nó (só com esses modificadores: o AltGr, que chega como Ctrl+Alt, não conta). O
/// egui-winit entrega o Ctrl+Shift+C como `Event::Copy`, sem o `Event::Key`; os dois valem e são consumidos.
fn pediu_copiar(resp: &Response) -> bool {
    if !resp.has_focus() {
        return false;
    }
    let atalho = Modifiers::COMMAND | Modifiers::SHIFT;
    resp.ctx.input_mut(|i| {
        let agora = i.modifiers;
        let e_o_atalho = |e: &Event| match e {
            Event::Key { key: Key::C, pressed: true, modifiers, .. } => modifiers.matches_exact(atalho),
            Event::Copy => agora.matches_exact(atalho),
            _ => false,
        };
        if !i.events.iter().any(&e_o_atalho) {
            return false;
        }
        i.events.retain(|e| !e_o_atalho(e));
        true
    })
}

/// O clique que abre ou fecha uma pasta: o segundo clique de um clique duplo (o gesto do Explorador do Windows) não
/// fecha de novo o que o primeiro abriu.
fn clique_simples(resp: &Response) -> bool {
    resp.clicked() && !resp.double_clicked()
}

/// Copia o caminho e dá o toast "Caminho copiado." (o mesmo da Visão geral).
fn copiar_caminho(ctx: &egui::Context, caminho: &Path) {
    ctx.copy_text(caminho.display().to_string());
    toast_concluido(ctx, MSG_CAMINHO_COPIADO);
}

/// A dica de uma pasta (só montada quando aparece): o caminho e, se a pasta já foi lida, a contagem dos arquivos que
/// ela esconde e o aviso da leitura que parou antes do fim (o prazo de 5 s ou os limites da decisão 5).
fn dica_da_pasta(resp: Response, caminho: &Path, listagem: Option<&Listagem>) -> Response {
    resp.on_hover_ui(|ui| {
        let mut dica = caminho.display().to_string();
        let ocultas = listagem.and_then(|l| frase_das_ocultas(l.ocultas));
        let parcial = listagem.is_some_and(|l| l.parcial).then_some(FRASE_PARCIAL);
        for frase in ocultas.iter().map(String::as_str).chain(parcial) {
            dica.push('\n');
            dica.push_str(frase);
        }
        ui.label(dica);
    })
}

/// A dica de um arquivo (só montada quando aparece): o caminho.
fn dica_do_caminho(resp: Response, caminho: &Path) -> Response {
    resp.on_hover_ui(|ui| {
        ui.label(caminho.display().to_string());
    })
}

/// O que a árvore precisa do quadro.
struct Vista<'a> {
    estado: &'a Estado,
    textos: &'a Textos,
    rota: &'a Rota,
    falha: Option<&'a str>,
    consulta: &'a str,
    /// A chave do caminho do banco aberto (o nó dele na pasta é a subárvore de sempre).
    chave_banco: Option<String>,
    /// A chave do caminho do arquivo de dados da aba à vista (o nó dele fica marcado).
    chave_arquivo: Option<String>,
}

/// O estado do Explorador entre os quadros.
#[derive(Default)]
pub(crate) struct Explorador {
    /// O texto do filtro das tabelas: vale só para o banco em que foi digitado e enquanto o campo existe.
    filtro: String,
    banco_do_filtro: Option<String>,
    /// Os nós abertos ("banco", "g:tabelas", "t:clientes", "v:vendas"…) do banco de `arquivo_dos_abertos`.
    abertos: HashSet<String>,
    arquivo_dos_abertos: Option<String>,
    /// A aba à vista da última vez que os pais dela foram abertos.
    revelada: Option<ChaveAba>,
    /// As pastas abertas na árvore (pela chave do caminho) da pasta de `pasta_dos_abertos`; a raiz abre sozinha.
    pastas_abertas: HashSet<String>,
    pasta_dos_abertos: Option<String>,
    /// As pastas cuja leitura já foi pedida (`Acao::ListarPasta`) e que a casca ainda não pôs em `nos`.
    pedidas: HashSet<String>,
    /// O banco e o arquivo de dados cujos pais já foram abertos (pela chave do caminho).
    banco_revelado: Option<String>,
    arquivo_revelado: Option<String>,
    /// O Mostrar na pasta pedido (a casca abre o Explorador de Arquivos, [`Explorador::tomar_revelar`]).
    revelar: Option<PathBuf>,
    /// As últimas ações emitidas (para os testes, [`Explorador::tomar_emitidas`]).
    emitidas: VecDeque<Acao>,
}

impl Explorador {
    fn aberto(&self, chave: &str) -> bool {
        self.abertos.contains(chave)
    }

    fn seta(&self, chave: &str) -> Seta {
        if self.aberto(chave) { Seta::Aberta } else { Seta::Fechada }
    }

    fn alternar(&mut self, chave: &str) {
        if !self.abertos.remove(chave) {
            self.abertos.insert(chave.to_string());
        }
    }

    fn pasta_aberta(&self, chave: &str) -> bool {
        self.pastas_abertas.contains(chave)
    }

    fn alternar_pasta(&mut self, chave: &str) {
        if !self.pastas_abertas.remove(chave) {
            self.pastas_abertas.insert(chave.to_string());
        }
    }

    /// O Mostrar na pasta que a pessoa pediu desde a última chamada (a casca abre o Explorador de Arquivos numa
    /// tarefa; nos testes, o gancho).
    pub(crate) fn tomar_revelar(&mut self) -> Option<PathBuf> {
        self.revelar.take()
    }

    /// As ações que o Explorador emitiu desde a última chamada (as últimas [`GUARDADAS`]), para os testes.
    pub(crate) fn tomar_emitidas(&mut self) -> Vec<Acao> {
        self.emitidas.drain(..).collect()
    }

    /// Acompanha o banco e a aba à vista: o filtro e os nós abertos são do banco (o filtro, desta abertura; os nós,
    /// do arquivo); quando a aba à vista muda para uma parte de tabela ou visão, os pais dela se abrem.
    fn acompanhar(&mut self, b: &BancoInfo, objetos: Option<&ObjetosInfo>, rota: &Rota) {
        if self.banco_do_filtro.as_deref() != Some(b.id.as_str()) {
            self.filtro.clear();
            self.banco_do_filtro = Some(b.id.clone());
        }
        let arquivo = prefs::chave_caminho(&b.caminho);
        if self.arquivo_dos_abertos.as_deref() != Some(arquivo.as_str()) {
            self.abertos = [NO_BANCO, Grupo::Tabelas.chave()].into_iter().map(str::to_string).collect();
            self.arquivo_dos_abertos = Some(arquivo);
            self.revelada = None;
        }
        let chave = ChaveAba::de(rota);
        if chave == self.revelada {
            return;
        }
        if let Some(ChaveAba::Parte { nome, .. }) = &chave {
            // o grupo sai da lista de objetos (a rota pode ainda não ter o tipo certo); sem ela, espera chegar
            let Some(o) = objetos else { return };
            self.abertos.insert(NO_BANCO.to_string());
            if o.visoes.iter().any(|v| &v.nome == nome) {
                self.abertos.insert(Grupo::Visoes.chave().to_string());
                self.abertos.insert(chave_objeto(nome, true));
            } else if o.sistema.iter().any(|s| &s.nome == nome) {
                self.abertos.insert(Grupo::Sistema.chave().to_string());
            } else {
                self.abertos.insert(Grupo::Tabelas.chave().to_string());
                self.abertos.insert(chave_objeto(nome, false));
            }
        }
        self.revelada = chave;
    }

    /// Acompanha a pasta: outra pasta volta a árvore ao começo (só a raiz aberta); um banco ou a ficha de um arquivo
    /// de dentro dela abre os pais dele (as leituras que faltam são pedidas quando cada pai aparece aberto).
    fn acompanhar_pasta(&mut self, p: &PastaAberta, banco: Option<&BancoInfo>, rota: &Rota) {
        if self.pasta_dos_abertos.as_deref() != Some(p.chave.as_str()) {
            self.pastas_abertas = HashSet::from([p.chave.clone()]);
            self.pasta_dos_abertos = Some(p.chave.clone());
            self.pedidas.clear();
            self.banco_revelado = None;
            self.arquivo_revelado = None;
        }
        let chave_banco = banco.map(|b| prefs::chave_caminho(&b.caminho));
        if chave_banco != self.banco_revelado {
            if let Some(b) = banco {
                self.abrir_os_pais(p, &b.caminho);
            }
            self.banco_revelado = chave_banco;
        }
        let arquivo = rota.caminho_arquivo();
        let chave_arquivo = arquivo.map(prefs::chave_caminho);
        if chave_arquivo != self.arquivo_revelado {
            if let Some(a) = arquivo {
                self.abrir_os_pais(p, a);
            }
            self.arquivo_revelado = chave_arquivo;
        }
    }

    /// A pasta fechou: os nós abertos, as leituras pedidas e os pais revelados eram dela.
    fn esquecer_a_pasta(&mut self) {
        if self.pasta_dos_abertos.take().is_some() {
            self.pastas_abertas.clear();
            self.pedidas.clear();
            self.banco_revelado = None;
            self.arquivo_revelado = None;
        }
    }

    /// Abre na árvore as pastas entre a raiz e o arquivo, se ele está dentro da pasta.
    fn abrir_os_pais(&mut self, p: &PastaAberta, caminho: &Path) {
        if !p.contem(caminho) {
            return;
        }
        for pai in caminho.ancestors().skip(1).take_while(|a| p.contem(a)) {
            self.pastas_abertas.insert(prefs::chave_caminho(pai));
        }
    }

    /// Desenha o Explorador no espaço de `ui` (o painel inteiro); `consulta` é o nome da Consulta que abriu a
    /// transação, para a dica do ícone de atenção do banco.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mostrar(
        &mut self,
        ui: &mut Ui,
        estado: &Estado,
        textos: &Textos,
        rota: &Rota,
        falha: Option<&str>,
        consulta: &str,
        acoes: &mut Vec<Acao>,
    ) {
        let inicio = acoes.len();
        self.painel(ui, estado, textos, rota, falha, consulta, acoes);
        for a in &acoes[inicio..] {
            if self.emitidas.len() == GUARDADAS {
                self.emitidas.pop_front();
            }
            self.emitidas.push_back(a.clone());
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn painel(
        &mut self,
        ui: &mut Ui,
        estado: &Estado,
        textos: &Textos,
        rota: &Rota,
        falha: Option<&str>,
        consulta: &str,
        acoes: &mut Vec<Acao>,
    ) {
        let banco = estado.banco.as_ref();
        let pasta = estado.pasta.as_ref();
        if pasta.is_none() {
            // sem pasta, o que era dela some: a mesma pasta aberta de novo volta ao começo (a raiz aberta, os pais do
            // banco e da ficha abertos outra vez)
            self.esquecer_a_pasta();
        }
        if banco.is_none() && pasta.is_none() {
            // prancheta 4: só o título no cabeçalho
            Painel::new("explorador", "Explorador").mostrar(ui, |ui| {
                if vazio_centrado_com(
                    ui,
                    Icone::PastaSolida,
                    TEXTO_SEM_PASTA,
                    Some(EXPLICACAO_SEM_PASTA),
                    Some(ABRIR_PASTA),
                ) {
                    acoes.push(Acao::EscolherEAbrirPasta);
                }
            });
            return;
        }
        if let Some(b) = banco {
            self.acompanhar(b, estado.objetos.as_ref(), rota);
        }
        if let Some(p) = pasta {
            self.acompanhar_pasta(p, banco, rota);
        }
        let vista = Vista {
            estado,
            textos,
            rota,
            falha,
            consulta,
            chave_banco: banco.map(|b| prefs::chave_caminho(&b.caminho)),
            chave_arquivo: rota.caminho_arquivo().map(prefs::chave_caminho),
        };
        let (mut recolher, mut abrir_pasta) = (false, false);
        Painel::new("explorador", "Explorador")
            .acoes(|ui| {
                // da direita para a esquerda: Abrir pasta e Recolher tudo
                recolher = ui.add(Botao::icone(Icone::Recolher).do_cabecalho().dica("Recolher tudo")).clicked();
                abrir_pasta = ui
                    .add(Botao::icone(Icone::Pasta).do_cabecalho().rotulo(ABRIR_PASTA).dica(DICA_ABRIR_PASTA))
                    .clicked();
            })
            .mostrar(ui, |ui| {
                ScrollArea::vertical().id_salt("explorador-rolagem").auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // `padding:6px 0 8px`
                    ui.add_space(6.0);
                    match (pasta, banco) {
                        (None, Some(b)) => self.arvore_do_banco(ui, &vista, b, 0, acoes),
                        (Some(p), b) => {
                            // o banco aberto de fora da pasta: a segunda raiz, no alto
                            if let (Some(b), Some(chave)) = (b, vista.chave_banco.as_deref())
                                && lugar_do_banco(p, &b.caminho, chave) == Lugar::Fora
                            {
                                self.arvore_do_banco(ui, &vista, b, 0, acoes);
                            }
                            self.raiz(ui, &vista, p, acoes);
                        }
                        (None, None) => {}
                    }
                    ui.add_space(8.0);
                });
            });
        if abrir_pasta {
            acoes.push(Acao::EscolherEAbrirPasta);
        }
        if recolher {
            self.abertos.clear();
            // com pasta, a raiz fica aberta: sem ela sobraria uma linha só
            self.pastas_abertas.clear();
            if let Some(p) = pasta {
                self.pastas_abertas.insert(p.chave.clone());
            }
        }
    }

    /// O menu de contexto e o Ctrl+Shift+C de um nó: cumpre o Copiar caminho e guarda o Mostrar na pasta; devolve o
    /// resto do que a pessoa escolheu.
    fn menu_do_no(
        &mut self,
        resp: &Response,
        chave: &str,
        nome: &str,
        caminho: &Path,
        tipo: TipoMenu,
    ) -> Option<Escolha> {
        let escolha = MenuContexto::new(("explorador", chave))
            .rotulo(nome)
            .mostrar(resp, |m| itens_do_menu(m, tipo))
            .flatten()
            .or_else(|| pediu_copiar(resp).then_some(Escolha::CopiarCaminho));
        match escolha {
            Some(Escolha::CopiarCaminho) => {
                copiar_caminho(&resp.ctx, caminho);
                None
            }
            Some(Escolha::MostrarNaPasta) => {
                self.revelar = Some(caminho.to_path_buf());
                None
            }
            outra => outra,
        }
    }

    /// A raiz da pasta aberta e, aberta, o que ela tem.
    fn raiz(&mut self, ui: &mut Ui, v: &Vista<'_>, p: &PastaAberta, acoes: &mut Vec<Acao>) {
        let aberta = self.pasta_aberta(&p.chave);
        let rotulo = rotulo_da_pasta(&p.nome);
        let r = No::new(&p.nome)
            .rotulo(&rotulo)
            .recuo(recuo(0))
            .seta(if aberta { Seta::Aberta } else { Seta::Fechada })
            .icone(Icone::PastaSolida)
            .mostrar(ui);
        if r.alternou || clique_simples(&r.resposta) {
            self.alternar_pasta(&p.chave);
        }
        let resp = dica_da_pasta(r.resposta, &p.caminho, p.listagem(&p.chave));
        match self.menu_do_no(&resp, &p.chave, &p.nome, &p.caminho, TipoMenu::Raiz) {
            Some(Escolha::Atualizar) => acoes.push(Acao::AtualizarPasta),
            Some(Escolha::FecharPasta) => acoes.push(Acao::FecharPasta),
            _ => {}
        }
        if self.pasta_aberta(&p.chave) {
            ui.push_id(&p.chave, |ui| self.conteudo_da_pasta(ui, v, p, &p.chave, &p.caminho, 1, acoes));
        }
    }

    /// O que uma pasta aberta tem, no `nivel` dos filhos: a leitura em andamento, a falha, o vazio ou as entradas
    /// (e o "mais N itens"). Uma pasta aberta que ainda não foi pedida pede a leitura agora.
    #[allow(clippy::too_many_arguments)]
    fn conteudo_da_pasta(
        &mut self,
        ui: &mut Ui,
        v: &Vista<'_>,
        p: &PastaAberta,
        chave: &str,
        caminho: &Path,
        nivel: usize,
        acoes: &mut Vec<Acao>,
    ) {
        let no = p.nos.get(chave);
        if no.is_some() {
            self.pedidas.remove(chave);
        }
        match no {
            None => {
                if self.pedidas.insert(chave.to_string()) {
                    acoes.push(Acao::ListarPasta { caminho: caminho.to_path_buf() });
                }
                recuado(ui, recuo(nivel), |ui| {
                    carregando(ui, "Carregando…");
                });
            }
            Some(NoDaPasta::Lendo) => recuado(ui, recuo(nivel), |ui| {
                carregando(ui, "Carregando…");
            }),
            Some(NoDaPasta::Falhou(mensagem)) => {
                if falha_na_leitura(ui, recuo(nivel), TEXTO_FALHA_NA_PASTA, mensagem) {
                    acoes.push(Acao::ListarPasta { caminho: caminho.to_path_buf() });
                }
            }
            Some(NoDaPasta::Pronto(l)) => {
                if l.entradas.is_empty() && l.a_mais == 0 {
                    recuado(ui, recuo(nivel), |ui| {
                        hint(ui, TEXTO_PASTA_VAZIA);
                    });
                }
                for e in &l.entradas {
                    self.entrada(ui, v, p, e, nivel, acoes);
                }
                if l.a_mais > 0 {
                    mais_itens(ui, nivel, l.a_mais, l.entradas.len());
                }
            }
        }
    }

    /// Uma entrada da pasta: uma subpasta (e, aberta, o que ela tem), um banco ou um arquivo de dados.
    fn entrada(
        &mut self,
        ui: &mut Ui,
        v: &Vista<'_>,
        p: &PastaAberta,
        e: &Entrada,
        nivel: usize,
        acoes: &mut Vec<Acao>,
    ) {
        match e.natureza {
            Natureza::Pasta => {
                let aberta = self.pasta_aberta(&e.chave);
                let rotulo = rotulo_da_pasta(&e.nome);
                let r = No::new(&e.nome)
                    .rotulo(&rotulo)
                    .recuo(recuo(nivel))
                    .seta(if aberta { Seta::Aberta } else { Seta::Fechada })
                    .icone(Icone::PastaSolida)
                    .mostrar(ui);
                if r.alternou || clique_simples(&r.resposta) {
                    self.alternar_pasta(&e.chave);
                }
                let resp = dica_da_pasta(r.resposta, &e.caminho, p.listagem(&e.chave));
                if self.menu_do_no(&resp, &e.chave, &e.nome, &e.caminho, TipoMenu::Pasta) == Some(Escolha::Atualizar) {
                    acoes.push(Acao::ListarPasta { caminho: e.caminho.clone() });
                }
                if self.pasta_aberta(&e.chave) {
                    ui.push_id(&e.chave, |ui| self.conteudo_da_pasta(ui, v, p, &e.chave, &e.caminho, nivel + 1, acoes));
                }
            }
            Natureza::Banco => {
                if v.chave_banco.as_deref() == Some(e.chave.as_str())
                    && let Some(b) = v.estado.banco.as_ref()
                {
                    // o banco aberto: a subárvore de sempre, no lugar dele (com o id próprio: os grupos "Tabelas",
                    // "Visões" e "Sistema" não dividem o id com um arquivo vizinho de mesmo nome)
                    ui.push_id(("banco", &e.chave), |ui| self.arvore_do_banco(ui, v, b, nivel, acoes));
                    return;
                }
                let r = No::new(&e.nome).recuo(recuo(nivel)).seta(Seta::Fechada).icone(Icone::Banco).mostrar(ui);
                // o → com o foco não troca de banco (a troca fecha as abas do banco aberto): só o clique, o Enter e o
                // Abrir do menu de contexto; o clique na seta abre, como o resto do nó
                let pela_seta_do_teclado = r.resposta.has_focus() && ui.input(|i| i.key_pressed(Key::ArrowRight));
                let abrir = (r.alternou && !pela_seta_do_teclado) || r.resposta.clicked();
                let resp = dica_do_caminho(r.resposta, &e.caminho);
                let escolha = self.menu_do_no(&resp, &e.chave, &e.nome, &e.caminho, TipoMenu::Arquivo);
                if abrir || escolha == Some(Escolha::Abrir) {
                    // a troca de hoje: a casca pergunta da transação e mostra o erro no toast
                    acoes.push(Acao::AbrirBanco { caminho: e.caminho.clone(), somente_leitura: false });
                }
            }
            Natureza::Dados(tipo) => {
                let ativo = v.chave_arquivo.as_deref() == Some(e.chave.as_str());
                let r = No::new(&e.nome)
                    .recuo(recuo(nivel))
                    .seta(Seta::Vazia)
                    .icone(icone_do_arquivo(tipo))
                    .ativo(ativo)
                    .mostrar(ui);
                let abrir = r.resposta.clicked();
                let resp = dica_do_caminho(r.resposta, &e.caminho);
                let escolha = self.menu_do_no(&resp, &e.chave, &e.nome, &e.caminho, TipoMenu::Arquivo);
                if abrir || escolha == Some(Escolha::Abrir) {
                    acoes.push(Acao::AbrirArquivoDeDados { caminho: e.caminho.clone() });
                }
            }
        }
    }

    /// O banco aberto e a árvore dele, com o banco no nível `base` (0 sem pasta ou na segunda raiz; um a mais que a
    /// pasta onde ele está).
    fn arvore_do_banco(&mut self, ui: &mut Ui, v: &Vista<'_>, b: &BancoInfo, base: usize, acoes: &mut Vec<Acao>) {
        // sem ponto "Aberto" (revisão de 15/10/2026, item 15: verde e âmbar viram o mesmo tom para quem tem
        // daltonismo); com a transação aberta, o ícone Atenção de 16px ("Transação aberta" para o leitor de tela) com a
        // dica do aviso inteiro ("Há uma transação aberta na Consulta 1. O que for salvo…", a da barra de status)
        let dica_transacao = b.transacao_aberta.then(|| super::frase_da_transacao(v.consulta));
        let mut no = No::new(&b.nome)
            .recuo(recuo(base))
            .seta(self.seta(NO_BANCO))
            .icone(icone_do_banco(b))
            .forte()
            .dica(&v.textos.caminho)
            .ativo(matches!(v.rota, Rota::Banco));
        if let Some(dica) = &dica_transacao {
            no = no.estado(Icone::Estado(EstadoIcone::Atencao), "Transação aberta").dica_do_estado(dica);
        }
        let r = no.mostrar(ui);
        let chave = v.chave_banco.clone().unwrap_or_default();
        let escolha = self.menu_do_no(&r.resposta, &chave, &b.nome, &b.caminho, TipoMenu::Arquivo);
        if r.alternou {
            self.alternar(NO_BANCO);
        } else if r.resposta.clicked() || escolha == Some(Escolha::Abrir) {
            self.abertos.insert(NO_BANCO.to_string());
            acoes.push(Acao::Navegar(Rota::Banco));
        }
        if !self.aberto(NO_BANCO) {
            return;
        }
        let Some(o) = v.estado.objetos.as_ref() else {
            match v.falha {
                // a leitura falhou: a mensagem e o "Tentar de novo", sem ponto pulsante (nada está carregando)
                Some(mensagem) => {
                    if falha_na_leitura(ui, recuo(base + 1), TEXTO_FALHA_NA_LEITURA, mensagem) {
                        acoes.push(Acao::RecarregarObjetos);
                    }
                }
                None => recuado(ui, recuo(base + 1), |ui| {
                    carregando(ui, "Carregando…");
                }),
            }
            return;
        };

        let textos = v.textos;
        if self.grupo(ui, Grupo::Tabelas, &textos.grupos[0], base) {
            ui.push_id(Grupo::Tabelas.chave(), |ui| self.tabelas(ui, o, textos, v.rota, base, acoes));
        }
        if !o.visoes.is_empty() && self.grupo(ui, Grupo::Visoes, &textos.grupos[1], base) {
            ui.push_id(Grupo::Visoes.chave(), |ui| {
                for vis in &o.visoes {
                    self.objeto(ui, &vis.nome, true, Icone::Visao, None, v.rota, base, acoes);
                }
            });
        }
        if !o.sistema.is_empty() && self.grupo(ui, Grupo::Sistema, &textos.grupos[2], base) {
            ui.push_id(Grupo::Sistema.chave(), |ui| {
                for (i, s) in o.sistema.iter().enumerate() {
                    let ativo = v.rota.nome_objeto() == Some(s.nome.as_str());
                    let mut no =
                        No::new(&s.nome).recuo(recuo(base + 2)).seta(Seta::Vazia).icone(Icone::Sistema).dica(&s.nome);
                    if let Some(c) = textos.contagens_sistema.get(i) {
                        no = no.contagem(c);
                    }
                    if no.ativo(ativo).mostrar(ui).resposta.clicked() {
                        acoes.push(Acao::Navegar(Rota::objeto(s.nome.clone(), s.tipo == TipoObjeto::View)));
                    }
                }
            });
        }
    }

    /// O nó de um grupo (o clique abre e fecha); devolve se ele está aberto.
    fn grupo(&mut self, ui: &mut Ui, grupo: Grupo, contagem: &str, base: usize) -> bool {
        let chave = grupo.chave();
        let r = No::new(grupo.rotulo()).recuo(recuo(base + 1)).seta(self.seta(chave)).contagem(contagem).mostrar(ui);
        if r.alternou || r.resposta.clicked() {
            self.alternar(chave);
        }
        self.aberto(chave)
    }

    /// As tabelas, com o filtro acima de [`LIMITE_SEM_FILTRO`].
    fn tabelas(
        &mut self,
        ui: &mut Ui,
        o: &ObjetosInfo,
        textos: &Textos,
        rota: &Rota,
        base: usize,
        acoes: &mut Vec<Acao>,
    ) {
        let com_filtro = o.tabelas.len() > LIMITE_SEM_FILTRO;
        if com_filtro {
            let filtro = &mut self.filtro;
            egui::Frame::NONE
                .inner_margin(Margin { left: (MARGEM_NO + recuo(base + 2)) as i8, right: 12, top: 4, bottom: 6 })
                .show(ui, |ui| {
                    Campo::new(filtro)
                        .busca(true)
                        .dica("Filtrar tabelas")
                        .rotulo("Filtrar tabelas")
                        .id_salt("explorador::filtro")
                        .mostrar(ui);
                });
        } else {
            self.filtro.clear();
        }
        if o.tabelas.is_empty() {
            recuado(ui, recuo(base + 2), |ui| {
                hint(ui, "Nenhuma tabela ainda.");
            });
        }
        let chave = (com_filtro && !self.filtro.trim().is_empty()).then(|| sem_acento(self.filtro.trim()));
        for (i, t) in o.tabelas.iter().enumerate() {
            if let Some(chave) = &chave
                && !textos.chaves_tabelas.get(i).is_some_and(|c| c.contains(chave.as_str()))
            {
                continue;
            }
            let icone = icone_da_tabela(t.tipo, t.sem_rowid);
            let contagem = textos.contagens_tabelas.get(i).map(String::as_str);
            self.objeto(ui, &t.nome, false, icone, contagem, rota, base, acoes);
        }
    }

    /// Uma tabela ou visão e, aberta, as partes dela.
    #[allow(clippy::too_many_arguments)]
    fn objeto(
        &mut self,
        ui: &mut Ui,
        nome: &str,
        e_visao: bool,
        icone: Icone,
        contagem: Option<&str>,
        rota: &Rota,
        base: usize,
        acoes: &mut Vec<Acao>,
    ) {
        let chave = chave_objeto(nome, e_visao);
        let mut no = No::new(nome).recuo(recuo(base + 2)).seta(self.seta(&chave)).icone(icone).dica(nome);
        if let Some(c) = contagem {
            no = no.contagem(c);
        }
        let r = no.mostrar(ui);
        if r.alternou {
            self.alternar(&chave);
        } else if r.resposta.clicked() {
            self.abertos.insert(chave.clone());
            acoes.push(Acao::Navegar(Rota::objeto(nome, e_visao)));
        }
        if !self.aberto(&chave) {
            return;
        }
        let partes: &[Parte] = if e_visao { &Parte::DA_VISAO } else { &Parte::DA_TABELA };
        for &parte in partes {
            let rotulo = format!("{nome} · {}", parte.rotulo());
            let ativo = rota.nome_objeto() == Some(nome) && rota.aba() == Some(parte);
            let r = No::new(parte.rotulo())
                .rotulo(&rotulo)
                .recuo(recuo(base + 3))
                .seta(Seta::Vazia)
                .icone(icone_da_parte(parte))
                .ativo(ativo)
                .mostrar(ui);
            if r.resposta.clicked() {
                acoes.push(Acao::Navegar(Rota::objeto_na_aba(nome, e_visao, parte)));
            }
        }
    }
}

/// O texto de quando a leitura das tabelas falhou, embaixo do nó do banco.
const TEXTO_FALHA_NA_LEITURA: &str = "Não foi possível ler as tabelas.";
/// O botão que repete a leitura que falhou.
pub(crate) const TENTAR_DE_NOVO: &str = "Tentar de novo";

/// Uma leitura falhou (as tabelas do banco, uma pasta): a frase, o motivo e o botão "Tentar de novo" (sem o ponto
/// pulsante, que é de quem ainda espera), no `recuo` do nível. Devolve true quando a pessoa clicou no botão.
fn falha_na_leitura(ui: &mut Ui, recuo: f32, frase: &str, mensagem: &str) -> bool {
    let mut de_novo = false;
    recuado(ui, recuo, |ui| {
        let perigo = paleta(ui).estudio.perigo;
        texto(ui, frase, 11.5, perigo);
        if !mensagem.is_empty() {
            paragrafo(ui, mensagem, 11.5, paleta(ui).estudio.t3);
        }
        ui.add_space(6.0);
        de_novo = ui.add(Botao::ghost(TENTAR_DE_NOVO).pequeno()).clicked();
    });
    de_novo
}

/// O que vai na linha de um nó, com o recuo dele (o "Carregando…" e o "Nenhuma tabela ainda.").
fn recuado(ui: &mut Ui, recuo: f32, conteudo: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE.inner_margin(Margin { left: (MARGEM_NO + recuo + 2.0) as i8, right: 6, top: 4, bottom: 4 }).show(
        ui,
        |ui| {
            ui.set_min_height(16.0);
            conteudo(ui);
        },
    );
}

/// A linha "mais 37 itens" depois das entradas que o limite deixou à vista: 11px em `--t3`, alinhada com os ícones
/// dos irmãos, sem foco (não é uma parada do Tab), com a dica do limite.
fn mais_itens(ui: &mut Ui, nivel: usize, a_mais: usize, mostrados: usize) {
    let frase = frase_mais_itens(a_mais);
    let largura = ui.available_width();
    let (linha, resp) = ui.allocate_exact_size(vec2(largura, ALTURA_NO), Sense::hover());
    if ui.is_rect_visible(linha) {
        let t3 = paleta(ui).estudio.t3;
        let g = galley(ui, &frase, fontes::fonte(11.0, Peso::Normal), t3);
        let x = linha.left() + MARGEM_NO + recuo(nivel) + ESPACO_DA_SETA;
        ui.painter().galley(pos2(x, linha.center().y - g.size().y / 2.0), g, t3);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &frase));
    resp.on_hover_ui(|ui| {
        ui.label(dica_do_limite(mostrados));
    });
}

/// O estado vazio de um painel da esquerda, no meio dele (`justify-content:center; gap:12px; padding:24px`): o
/// sólido do banco de 48px, o texto em `--t2`, a explicação em `--t3` e o botão. Devolve true quando o botão foi
/// clicado.
pub(crate) fn vazio_centrado(ui: &mut Ui, texto: &str, explicacao: Option<&str>, botao: Option<&str>) -> bool {
    vazio_centrado_com(ui, Icone::Banco, texto, explicacao, botao)
}

/// O [`vazio_centrado`] com outro sólido de 48px (o relógio do Histórico vazio, N5; a pasta do Explorador sem pasta).
pub(crate) fn vazio_centrado_com(
    ui: &mut Ui,
    icone: Icone,
    texto: &str,
    explicacao: Option<&str>,
    botao: Option<&str>,
) -> bool {
    let e = paleta(ui).estudio;
    let area = ui.max_rect().shrink(24.0);
    let largura = area.width().max(0.0);
    let linha = |texto: &str, cor| {
        let mut job = LayoutJob::single_section(texto.to_string(), formato(fontes::fonte(12.0, Peso::Normal), cor));
        job.wrap.max_width = largura;
        job.halign = Align::Center;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let g_texto = linha(texto, e.t2);
    let g_explicacao = explicacao.map(|t| linha(t, e.t3));
    let altura_botao = if botao.is_some() { 12.0 + interface::pecas::botao::ALTURA } else { 0.0 };
    let altura_textos = g_texto.size().y + g_explicacao.as_ref().map_or(0.0, |g| g.size().y);
    let total = 48.0 + 12.0 + altura_textos + altura_botao;
    let mut y = area.top() + ((area.height() - total) / 2.0).max(0.0);
    let centro = area.center().x;
    icones::pintar(ui.painter(), Rect::from_center_size(pos2(centro, y + 24.0), vec2(48.0, 48.0)), icone, e.t3);
    y += 48.0 + 12.0;
    for (g, cor, rotulo) in [(Some(g_texto), e.t2, Some(texto)), (g_explicacao, e.t3, explicacao)] {
        let (Some(g), Some(rotulo)) = (g, rotulo) else { continue };
        let rect = Rect::from_min_size(pos2(area.left(), y), vec2(largura, g.size().y));
        y += g.size().y;
        ui.painter().galley(pos2(centro, rect.top()), g, cor);
        let resp = ui.interact(rect, ui.id().with(("vazio", rotulo)), Sense::hover());
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, rotulo));
    }
    let Some(rotulo) = botao else { return false };
    let largura_botao = largura.min(160.0);
    let rect = Rect::from_min_size(
        pos2(centro - largura_botao / 2.0, y + 12.0),
        vec2(largura_botao, interface::pecas::botao::ALTURA),
    );
    let mut ui_botao =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(Align::Center)));
    ui_botao.add(Botao::new(rotulo)).clicked()
}

#[cfg(test)]
mod testes {
    use std::sync::Arc;

    use super::*;
    use estado::apoio_testes::Ambiente;

    fn entrada(pasta: &Path, nome: &str, natureza: Natureza) -> Entrada {
        let caminho = pasta.join(nome);
        Entrada {
            nome: nome.to_string(),
            chave: prefs::chave_caminho(&caminho),
            caminho,
            natureza,
            tamanho: None,
            modificado: None,
        }
    }

    #[test]
    fn os_pais_da_parte_a_vista_se_abrem_quando_ela_muda() {
        let amb = Ambiente::exemplo("explorador_revelar");
        let b = amb.estado.banco.clone().expect("banco");
        let o = amb.estado.objetos.clone().expect("objetos");
        let mut x = Explorador::default();
        x.acompanhar(&b, Some(&o), &Rota::Banco);
        // no começo, o banco e as Tabelas
        assert!(x.aberto(NO_BANCO) && x.aberto(Grupo::Tabelas.chave()));
        assert!(!x.aberto(Grupo::Visoes.chave()) && !x.aberto(Grupo::Sistema.chave()));
        // a aba à vista passa a ser a estrutura de clientes: a tabela se abre
        let estrutura = Rota::objeto_na_aba("clientes", false, Parte::Estrutura);
        x.acompanhar(&b, Some(&o), &estrutura);
        assert!(x.aberto("t:clientes"));
        // fechada pela pessoa, continua fechada enquanto a aba à vista é a mesma (mesmo com outra página)
        x.alternar("t:clientes");
        x.acompanhar(&b, Some(&o), &estrutura);
        assert!(!x.aberto("t:clientes"));
        // a visão abre o grupo das Visões, mesmo pedida como tabela; o objeto do sistema abre o Sistema
        x.acompanhar(&b, Some(&o), &Rota::objeto("v_pedidos_por_cliente", false));
        assert!(x.aberto(Grupo::Visoes.chave()) && x.aberto("v:v_pedidos_por_cliente"));
        let sistema = o.sistema.first().map(|s| s.nome.clone()).expect("um objeto do sistema");
        x.acompanhar(&b, Some(&o), &Rota::objeto(sistema, false));
        assert!(x.aberto(Grupo::Sistema.chave()));
        // sem a lista de objetos, espera por ela
        let mut y = Explorador::default();
        y.acompanhar(&b, None, &Rota::objeto("clientes", false));
        assert!(!y.aberto("t:clientes"));
        y.acompanhar(&b, Some(&o), &Rota::objeto("clientes", false));
        assert!(y.aberto("t:clientes"));
    }

    #[test]
    fn outro_banco_volta_ao_comeco_e_limpa_o_filtro() {
        let amb = Ambiente::exemplo("explorador_outro_banco");
        let b = amb.estado.banco.clone().expect("banco");
        let mut x = Explorador::default();
        x.acompanhar(&b, None, &Rota::Banco);
        x.alternar(Grupo::Tabelas.chave());
        x.alternar("g:sistema");
        x.filtro = "cli".into();
        // a mesma abertura: nada muda
        x.acompanhar(&b, None, &Rota::Banco);
        assert_eq!(x.filtro, "cli");
        assert!(x.aberto("g:sistema") && !x.aberto(Grupo::Tabelas.chave()));
        // o mesmo arquivo reaberto (outro id): o filtro sai, a árvore fica
        let mut reaberto = b.clone();
        reaberto.id = "outro-id".into();
        x.acompanhar(&reaberto, None, &Rota::Banco);
        assert!(x.filtro.is_empty());
        assert!(x.aberto("g:sistema"));
        // outro arquivo: a árvore volta ao começo
        let mut outro = b;
        outro.caminho = outro.caminho.with_file_name("outro.db");
        x.acompanhar(&outro, None, &Rota::Banco);
        assert!(x.aberto(Grupo::Tabelas.chave()) && !x.aberto("g:sistema"));
    }

    #[test]
    fn outra_pasta_volta_ao_comeco_e_o_aberto_de_dentro_abre_os_pais() {
        use estado::apoio_testes::abs;
        let mut p = PastaAberta::nova(abs("Dados/Projeto"));
        let mut x = Explorador::default();
        x.acompanhar_pasta(&p, None, &Rota::Inicio);
        assert!(x.pasta_aberta(&p.chave), "a raiz abre sozinha");
        // a ficha de um arquivo fundo abre os pais (a raiz já estava), e só uma vez por arquivo
        let ficha = Rota::arquivo(abs("Dados/Projeto/a/b/vendas.csv"));
        x.acompanhar_pasta(&p, None, &ficha);
        let a = prefs::chave_caminho(abs("Dados/Projeto/a"));
        let ab = prefs::chave_caminho(abs("Dados/Projeto/a/b"));
        assert!(x.pasta_aberta(&a) && x.pasta_aberta(&ab));
        x.alternar_pasta(&ab);
        x.acompanhar_pasta(&p, None, &ficha);
        assert!(!x.pasta_aberta(&ab), "fechada pela pessoa, fica fechada");
        // de fora da pasta: nada abre
        x.acompanhar_pasta(&p, None, &Rota::arquivo(abs("Outro/x.csv")));
        assert_eq!(x.pastas_abertas.len(), 2);
        // outra pasta: só a raiz dela
        p = PastaAberta::nova(abs("Dados/Outra"));
        x.acompanhar_pasta(&p, None, &Rota::Inicio);
        assert_eq!(x.pastas_abertas, HashSet::from([p.chave.clone()]));
    }

    #[test]
    fn lugar_do_banco_na_arvore_esperando_ou_fora() {
        use estado::apoio_testes::abs;
        let mut p = PastaAberta::nova(abs("Dados"));
        let raiz = p.chave.clone();
        let banco = abs("Dados/sub/fundo.db");
        let banco = banco.as_path();
        let chave = prefs::chave_caminho(banco);
        // fora da pasta
        assert_eq!(lugar_do_banco(&p, &abs("Outro/x.db"), "x"), Lugar::Fora);
        // a raiz ainda lendo
        assert_eq!(lugar_do_banco(&p, banco, &chave), Lugar::Esperando);
        let sub = entrada(&p.caminho, "sub", Natureza::Pasta);
        let pronta = |entradas| NoDaPasta::Pronto(Arc::new(Listagem::montar(PathBuf::new(), entradas, 0, false, 500)));
        p.nos.insert(raiz.clone(), pronta(vec![sub.clone()]));
        // a subpasta ainda não foi lida
        assert_eq!(lugar_do_banco(&p, banco, &chave), Lugar::Esperando);
        p.nos.insert(sub.chave.clone(), pronta(vec![entrada(&sub.caminho, "fundo.db", Natureza::Banco)]));
        assert_eq!(lugar_do_banco(&p, banco, &chave), Lugar::NaArvore);
        // a subpasta que não tem o banco (oculto, depois do limite) ou que falhou: a segunda raiz
        p.nos.insert(sub.chave.clone(), pronta(vec![]));
        assert_eq!(lugar_do_banco(&p, banco, &chave), Lugar::Fora);
        p.nos.insert(sub.chave.clone(), NoDaPasta::Falhou("sem acesso".into()));
        assert_eq!(lugar_do_banco(&p, banco, &chave), Lugar::Fora);
        // a raiz sem a subpasta
        p.nos.insert(raiz, pronta(vec![]));
        assert_eq!(lugar_do_banco(&p, banco, &chave), Lugar::Fora);
    }

    #[test]
    fn frases_da_pasta_no_singular_e_no_plural() {
        assert_eq!(frase_das_ocultas(0), None);
        assert_eq!(frase_das_ocultas(1).as_deref(), Some("1 arquivo de outro tipo não aparece."));
        assert_eq!(frase_das_ocultas(12).as_deref(), Some("12 arquivos de outros tipos não aparecem."));
        assert_eq!(frase_das_ocultas(1500).as_deref(), Some("1.500 arquivos de outros tipos não aparecem."));
        assert_eq!(frase_mais_itens(1), "mais 1 item");
        assert_eq!(frase_mais_itens(37), "mais 37 itens");
        assert_eq!(dica_do_limite(500), "Só os primeiros 500 itens aparecem. Use Mostrar na pasta para ver o resto.");
        assert_eq!(rotulo_da_pasta("dados"), "dados, pasta");
        assert_eq!(icone_do_arquivo(TipoDeArquivo::Texto), Icone::Folha(TipoArquivo::Txt));
        assert_eq!(icone_do_arquivo(TipoDeArquivo::Xlsx), Icone::Folha(TipoArquivo::Xlsx));
    }
}
