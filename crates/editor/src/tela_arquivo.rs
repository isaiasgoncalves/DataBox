//! A aba de um arquivo de dados (`Rota::Arquivo`): a [`TelaArquivo`] escolhe a tela pelo tipo do arquivo e é ela que mora no `Documento::Arquivo` do editor.
//!
//! - [`TelaArquivo::Ficha`]: a ficha da etapa 4 ([`Ficha`], abaixo), para o que ainda não tem editor (o Excel até a
//!   etapa 6) e para tudo sem o recurso `telas`.
//! - [`TelaArquivo::Csv`] e [`TelaArquivo::Texto`]: um editor de arquivo ([`EditorDeArquivo`]), a tela de outro crate
//!   (`tela_csv`, `tela_texto`) atrás do trait `estado::Documento`, com o nome e a folha do tipo, que valem para todas.
//!   O `Texto` é a `tela_texto::TelaTexto` (TXT, MD, LOG e JSON) com o recurso `telas`.
//!
//! **O ponto de encaixe das telas**: [`editor_de_csv`] e [`editor_de_texto`], uma função por tipo. O de texto já é a
//! `TelaTexto` e o de CSV a `TelaCsv`, cada um uma variante em [`TelaDoEditor`] (com o `tomar_pedido` e o
//! `pergunta_aberta` dela), sob o recurso `telas`; sem ele, a ficha.
//! Nada mais na casca muda: a aba, o ponto de alterada, a barra de status, Ctrl+S/Z/Y e a fila de perguntas ao
//! fechar vêm todos do trait (`alterado`, `alteracoes`, `salvando`, `tomar_erro_de_gravacao`, `status_do_arquivo`,
//! `fonte_de_colunas`, as `Ofertas` e o `executar`).
//!
//! O que a casca espera do editor, além do trait:
//! - `Verbo::Salvar` começa a gravação (numa tarefa) e, até ela terminar, `salvando()` é verdadeiro; no fim,
//!   `alterado()` falso, ou o erro em `tomar_erro_de_gravacao()` (a fila ao fechar para tudo com o toast dele). A
//!   resposta tem de chegar também com a aba fora de vista (`em_segundo_plano`). Uma pergunta da própria tela no meio
//!   (o "Salvar mesmo assim" de quando o arquivo mudou no disco) é o `pergunta_aberta()`: a fila espera a resposta,
//!   com a aba à vista; fechada a pergunta com o documento ainda alterado, a pessoa cancelou e a fila para. O Ctrl+S
//!   vale com um campo de texto focado e não tira o foco dele: a célula em edição do CSV grava antes, por conta da tela.
//! - `Verbo::Atualizar` (o F5): com `alterado()`, a **tela** pergunta antes ("Descartar as alterações de {nome} e ler o
//!   arquivo de novo?"); a casca só passa o verbo.
//! - Ctrl+Z e Ctrl+Y (e Ctrl+Shift+Z) viram `Verbo::Desfazer` e `Verbo::Refazer` só sem campo de texto focado; com
//!   o `TextEdit` focado, quem trata é o egui.
//!
//! # A ficha
//!
//! Enquanto os editores de CSV, JSON e texto (etapa 5) e o do Excel (etapa 6) não existem, a aba diz o que o
//! arquivo é e o abre no programa padrão do Windows. Sem prancheta própria: o sistema visual de sempre.
//!
//! - **Faixa**: a folha de 20 do tipo (CSV, XLSX, JSON, TXT), o nome em 600/13 e o caminho em `--t3` com a
//!   reticência no meio.
//! - **Conteúdo** (a página): o aviso informativo de até 560 pt, centrado ("O editor de CSV ainda não está pronto.
//!   Enquanto isso, o arquivo abre no programa padrão do Windows.") e, embaixo, centrados, **Abrir no programa
//!   padrão** (primário) e **Mostrar na pasta**. O arquivo que sumiu: o erro do serviço ("O arquivo não existe (ou foi
//!   movido): …") no lugar do aviso, com os dois botões desligados.
//! - **Propriedades**: a seção "Arquivo" (Tipo, Tamanho, Modificado em, Caminho com o Copiar caminho e o toast
//!   "Caminho copiado.").
//! - **Status**: "CSV · 96 KB". Nenhum verbo ligado (a etapa 5 liga o Salvar, o Desfazer e os outros).
//!
//! O que o arquivo é sai do `Servico::info_arquivo` numa tarefa, quando a aba aparece (e de novo no
//! [`TelaArquivo::reler`]). Os dois botões não abrem nada daqui: a casca toma o pedido
//! ([`TelaArquivo::tomar_pedido`]) e abre o programa ou o Explorador de Arquivos numa tarefa (nos testes, o gancho).

use std::path::{Path, PathBuf};

use comum::arquivos::TipoDeArquivo;
use egui::{Id, Sense, Ui, WidgetInfo, WidgetType, vec2};
use estado::{Anatomia, Contexto, Estado, Promessa};
use interface::fontes::{self, Peso};
use interface::formato::formatar_bytes;
use interface::icones::{self, Icone};
use interface::pecas::avisos::{TomAviso, aviso_texto, aviso_tom, carregando};
use interface::pecas::botao::Botao;
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::{Trecho, galley, galley_caminho};
use interface::pecas::toast::toast_concluido;
use interface::tema::paleta;
use servico::InfoArquivo;

use crate::casca::explorador::icone_do_arquivo;
use crate::visao_geral::{MSG_CAMINHO_COPIADO, linha_do_caminho};

// ------------------------------------------------------------------ a escolha da tela

/// Que tela abre um tipo de arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tela {
    Ficha,
    Csv,
    Texto,
    Planilha,
}

/// A tela de cada tipo: CSV → o editor de CSV; JSON e TXT → o editor de texto; o Excel (`.xlsx` e `.xlsm`) → a
/// planilha só leitura (etapa 6); o tipo que não se sabe → a ficha.
pub(crate) fn tela_do_tipo(tipo: Option<TipoDeArquivo>) -> Tela {
    match tipo {
        Some(TipoDeArquivo::Csv) => Tela::Csv,
        Some(TipoDeArquivo::Json | TipoDeArquivo::Texto) => Tela::Texto,
        Some(TipoDeArquivo::Xlsx) => Tela::Planilha,
        None => Tela::Ficha,
    }
}

/// A planilha do Excel de `caminho`, só leitura: a `tela_planilha::TelaPlanilha`.
#[cfg(feature = "telas")]
pub(crate) fn tela_de_planilha(caminho: &Path) -> TelaArquivo {
    TelaArquivo::Planilha(Box::new(tela_planilha::TelaPlanilha::nova(caminho.to_path_buf())))
}

/// Sem o recurso `telas`, a ficha.
#[cfg(not(feature = "telas"))]
pub(crate) fn tela_de_planilha(caminho: &Path) -> TelaArquivo {
    TelaArquivo::ficha(caminho)
}

/// O tipo pela extensão do caminho (antes de o serviço dizer).
fn tipo_pela_extensao(caminho: &Path) -> Option<TipoDeArquivo> {
    caminho.extension().and_then(|e| e.to_str()).and_then(TipoDeArquivo::da_extensao)
}

/// O nome do arquivo (o da aba).
fn nome_do_arquivo(caminho: &Path) -> String {
    caminho.file_name().map_or_else(|| caminho.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// O editor de CSV de `caminho`: a `tela_csv::TelaCsv`.
#[cfg(feature = "telas")]
pub(crate) fn editor_de_csv(caminho: &Path) -> TelaArquivo {
    let tela = tela_csv::TelaCsv::nova(caminho.to_path_buf());
    TelaArquivo::Csv(EditorDeArquivo::novo(caminho, TelaDoEditor::Csv(Box::new(tela))))
}

/// Sem o recurso `telas`, a ficha.
#[cfg(not(feature = "telas"))]
pub(crate) fn editor_de_csv(caminho: &Path) -> TelaArquivo {
    TelaArquivo::ficha(caminho)
}

/// O editor de texto e JSON de `caminho`: a `tela_texto::TelaTexto`.
#[cfg(feature = "telas")]
pub(crate) fn editor_de_texto(caminho: &Path) -> TelaArquivo {
    let tela = tela_texto::TelaTexto::nova(caminho.to_path_buf());
    TelaArquivo::Texto(EditorDeArquivo::novo(caminho, TelaDoEditor::Texto(Box::new(tela))))
}

/// Sem o recurso `telas`, a ficha.
#[cfg(not(feature = "telas"))]
pub(crate) fn editor_de_texto(caminho: &Path) -> TelaArquivo {
    TelaArquivo::ficha(caminho)
}

/// A tela de um editor de arquivo: as de verdade pelo tipo delas (o que a casca pede além do trait: o pedido dos
/// botões da ficha e a pergunta aberta), e a de qualquer documento pelo trait só (os testes da casca).
pub(crate) enum TelaDoEditor {
    /// Um documento pelo trait só (sem pedido nem pergunta própria).
    Generica(Box<dyn estado::Documento>),
    /// O editor de texto e JSON.
    #[cfg(feature = "telas")]
    Texto(Box<tela_texto::TelaTexto>),
    /// O editor tabular de CSV.
    #[cfg(feature = "telas")]
    Csv(Box<tela_csv::TelaCsv>),
}

/// Um editor de arquivo: a tela e o que a casca sabe do arquivo pelo caminho (o nome e a folha do tipo da aba).
pub(crate) struct EditorDeArquivo {
    caminho: PathBuf,
    nome: String,
    tipo: Option<TipoDeArquivo>,
    tela: TelaDoEditor,
}

impl EditorDeArquivo {
    pub(crate) fn novo(caminho: &Path, tela: TelaDoEditor) -> EditorDeArquivo {
        EditorDeArquivo {
            caminho: caminho.to_path_buf(),
            nome: nome_do_arquivo(caminho),
            tipo: tipo_pela_extensao(caminho),
            tela,
        }
    }

    fn doc(&mut self) -> &mut dyn estado::Documento {
        match &mut self.tela {
            TelaDoEditor::Generica(d) => d.as_mut(),
            #[cfg(feature = "telas")]
            TelaDoEditor::Texto(t) => t.as_mut(),
            #[cfg(feature = "telas")]
            TelaDoEditor::Csv(t) => t.as_mut(),
        }
    }

    fn doc_ref(&self) -> &dyn estado::Documento {
        match &self.tela {
            TelaDoEditor::Generica(d) => d.as_ref(),
            #[cfg(feature = "telas")]
            TelaDoEditor::Texto(t) => t.as_ref(),
            #[cfg(feature = "telas")]
            TelaDoEditor::Csv(t) => t.as_ref(),
        }
    }

    fn tomar_pedido(&mut self) -> Option<PedidoArquivo> {
        match &mut self.tela {
            TelaDoEditor::Generica(_) => None,
            #[cfg(feature = "telas")]
            TelaDoEditor::Texto(t) => t.tomar_pedido().map(|p| match p {
                tela_texto::PedidoDeArquivo::AbrirNoPadrao(c) => PedidoArquivo::AbrirNoPadrao(c),
                tela_texto::PedidoDeArquivo::MostrarNaPasta(c) => PedidoArquivo::MostrarNaPasta(c),
            }),
            // o CSV mostra a falha de abrir no próprio documento e não tem os botões da ficha
            #[cfg(feature = "telas")]
            TelaDoEditor::Csv(_) => None,
        }
    }

    fn pergunta_aberta(&self) -> bool {
        match &self.tela {
            TelaDoEditor::Generica(_) => false,
            #[cfg(feature = "telas")]
            TelaDoEditor::Texto(t) => t.pergunta_aberta(),
            #[cfg(feature = "telas")]
            TelaDoEditor::Csv(t) => t.pergunta_aberta(),
        }
    }
}

/// A aba de um arquivo de dados: a ficha ou o editor do tipo (veja o topo do módulo).
pub(crate) enum TelaArquivo {
    /// A ficha da etapa 4 (o Excel até a etapa 6; tudo sem o recurso `telas`).
    Ficha(Box<Ficha>),
    /// O editor de CSV (`tela_csv`).
    Csv(EditorDeArquivo),
    /// O editor de texto e JSON (`tela_texto`).
    Texto(EditorDeArquivo),
    /// A planilha do Excel, só leitura (`tela_planilha`, etapa 6).
    #[cfg(feature = "telas")]
    Planilha(Box<tela_planilha::TelaPlanilha>),
}

impl TelaArquivo {
    /// A tela do tipo do arquivo, pela extensão ([`tela_do_tipo`]).
    pub(crate) fn nova(caminho: &Path) -> TelaArquivo {
        match tela_do_tipo(tipo_pela_extensao(caminho)) {
            Tela::Csv => editor_de_csv(caminho),
            Tela::Texto => editor_de_texto(caminho),
            Tela::Planilha => tela_de_planilha(caminho),
            Tela::Ficha => TelaArquivo::ficha(caminho),
        }
    }

    /// A ficha do arquivo.
    pub(crate) fn ficha(caminho: &Path) -> TelaArquivo {
        TelaArquivo::Ficha(Box::new(Ficha::nova(caminho)))
    }

    /// O editor de CSV com esta tela.
    #[allow(dead_code)] // a faixa G o chama de `editor_de_csv`
    pub(crate) fn csv(caminho: &Path, tela: impl estado::Documento + 'static) -> TelaArquivo {
        TelaArquivo::Csv(EditorDeArquivo::novo(caminho, TelaDoEditor::Generica(Box::new(tela))))
    }

    /// O editor de texto e JSON com esta tela.
    #[allow(dead_code)] // a faixa F o chama de `editor_de_texto`
    pub(crate) fn texto(caminho: &Path, tela: impl estado::Documento + 'static) -> TelaArquivo {
        TelaArquivo::Texto(EditorDeArquivo::novo(caminho, TelaDoEditor::Generica(Box::new(tela))))
    }

    /// Um editor qualquer no lugar da tela do tipo (os testes da casca, com um documento de mentira): o de CSV
    /// para o CSV, o de texto para o resto.
    pub(crate) fn com_editor(caminho: &Path, tela: Box<dyn estado::Documento>) -> TelaArquivo {
        let editor = EditorDeArquivo::novo(caminho, TelaDoEditor::Generica(tela));
        match tela_do_tipo(editor.tipo) {
            Tela::Csv => TelaArquivo::Csv(editor),
            Tela::Texto | Tela::Planilha | Tela::Ficha => TelaArquivo::Texto(editor),
        }
    }

    /// Que tela é.
    #[allow(dead_code)] // os testes (e as faixas das telas)
    pub(crate) fn tela(&self) -> Tela {
        match self {
            TelaArquivo::Ficha(_) => Tela::Ficha,
            TelaArquivo::Csv(_) => Tela::Csv,
            TelaArquivo::Texto(_) => Tela::Texto,
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(_) => Tela::Planilha,
        }
    }

    /// A tela, pelo contrato delas.
    pub(crate) fn doc(&mut self) -> &mut dyn estado::Documento {
        match self {
            TelaArquivo::Ficha(f) => f.as_mut(),
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => e.doc(),
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(p) => p.as_mut(),
        }
    }

    /// A tela, pelo contrato delas, só para ler.
    pub(crate) fn doc_ref(&self) -> &dyn estado::Documento {
        match self {
            TelaArquivo::Ficha(f) => f.as_ref(),
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => e.doc_ref(),
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(p) => p.as_ref(),
        }
    }

    /// O caminho do arquivo.
    #[allow(dead_code)] // os editores e os testes
    pub(crate) fn caminho(&self) -> &Path {
        match self {
            TelaArquivo::Ficha(f) => &f.caminho,
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => &e.caminho,
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(p) => p.caminho(),
        }
    }

    /// O nome do arquivo (o da aba e o da pergunta "Salvar as alterações em vendas.csv?").
    pub(crate) fn nome(&self) -> &str {
        match self {
            TelaArquivo::Ficha(f) => f.nome(),
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => &e.nome,
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(p) => p.nome(),
        }
    }

    /// A folha do tipo (a da aba); sem tipo conhecido, a silhueta do arquivo.
    pub(crate) fn icone(&self) -> Icone {
        match self {
            TelaArquivo::Ficha(f) => f.icone(),
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => e.tipo.map_or(Icone::Arquivo, icone_do_arquivo),
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(p) => p.icone(),
        }
    }

    /// O que os botões da ficha (a de verdade ou a do editor que não abriu o arquivo) pediram desde a última chamada
    /// (a casca cumpre).
    pub(crate) fn tomar_pedido(&mut self) -> Option<PedidoArquivo> {
        match self {
            TelaArquivo::Ficha(f) => f.tomar_pedido(),
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => e.tomar_pedido(),
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(p) => p.tomar_pedido().map(|pedido| match pedido {
                tela_planilha::PedidoDeArquivo::AbrirNoPadrao(c) => PedidoArquivo::AbrirNoPadrao(c),
                tela_planilha::PedidoDeArquivo::MostrarNaPasta(c) => PedidoArquivo::MostrarNaPasta(c),
                // o "Trazer colunas desta planilha": a casca leva a origem ao CSV de destino
                tela_planilha::PedidoDeArquivo::TrazerColunas(fonte) => PedidoArquivo::TrazerColunas(fonte),
            }),
        }
    }

    /// É um editor de CSV com a tabela lida: pode ser o destino do "Trazer colunas desta planilha".
    pub(crate) fn recebe_trazer(&self) -> bool {
        #[cfg(feature = "telas")]
        if let TelaArquivo::Csv(EditorDeArquivo { tela: TelaDoEditor::Csv(t), .. }) = self {
            return t.retrato().is_some();
        }
        false
    }

    /// Pede ao editor de CSV a janela "Trazer colunas" com a origem de rótulo `rotulo` já escolhida (a planilha do
    /// menu do cabeçalho); vale quando a aba se desenhar. False se esta tela não é um editor de CSV.
    pub(crate) fn trazer_colunas_de(&mut self, rotulo: String) -> bool {
        #[cfg(feature = "telas")]
        if let TelaArquivo::Csv(EditorDeArquivo { tela: TelaDoEditor::Csv(t), .. }) = self {
            t.trazer_colunas_de(rotulo);
            return true;
        }
        let _ = rotulo;
        false
    }

    /// A tela tem uma pergunta própria aberta (o "Salvar mesmo assim"): a fila de perguntas ao fechar espera a
    /// resposta, com a aba à vista.
    pub(crate) fn pergunta_aberta(&self) -> bool {
        match self {
            TelaArquivo::Ficha(_) => false,
            TelaArquivo::Csv(e) | TelaArquivo::Texto(e) => e.pergunta_aberta(),
            #[cfg(feature = "telas")]
            TelaArquivo::Planilha(_) => false,
        }
    }

    /// A ficha lê de novo o que o arquivo é quando a aba aparecer; os editores não releem por conta de outra aba (o
    /// arquivo deles só muda pelo F5).
    pub(crate) fn reler(&mut self) {
        if let TelaArquivo::Ficha(f) = self {
            f.reler();
        }
    }
}

// ------------------------------------------------------------------ a ficha

/// A folha da faixa (20) e o espaço entre os botões e o aviso (o `gap:18px` do conteúdo).
const LADO_FOLHA_FAIXA: f32 = 20.0;
const ESPACO_CONTEUDO: f32 = 18.0;
/// Os rótulos dos dois botões.
pub(crate) const ABRIR_NO_PADRAO: &str = "Abrir no programa padrão";
pub(crate) const MOSTRAR_NA_PASTA: &str = "Mostrar na pasta";

/// A frase da ficha: "O editor de CSV ainda não está pronto. Enquanto isso, o arquivo abre no programa padrão do
/// Windows." (com o nome do tipo nas frases: CSV, Excel, JSON, texto).
pub(crate) fn frase_da_ficha(tipo: TipoDeArquivo) -> String {
    format!(
        "O editor de {} ainda não está pronto. Enquanto isso, o arquivo abre no programa padrão do Windows.",
        tipo.nome()
    )
}

/// O que a ficha pede à casca (tomado por [`TelaArquivo::tomar_pedido`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PedidoArquivo {
    /// O arquivo no programa associado (`Servico::abrir_arquivo`).
    AbrirNoPadrao(PathBuf),
    /// O Explorador de Arquivos com o arquivo selecionado (`Servico::revelar`).
    MostrarNaPasta(PathBuf),
    /// O "Trazer colunas desta planilha" (o menu do cabeçalho da planilha): a janela Trazer colunas do CSV ativo, ou do
    /// usado mais recentemente, com esta origem (o rótulo da `FonteDeColunas`: "relatorio_q3.xlsx › Vendas").
    #[cfg_attr(not(feature = "telas"), allow(dead_code))] // só a planilha (com o recurso `telas`) pede
    TrazerColunas(String),
}

/// O que a ficha mostra do arquivo, já formatado.
struct Textos {
    tipo: TipoDeArquivo,
    caminho: String,
    tamanho: String,
    modificado: String,
}

impl Textos {
    fn de(info: &InfoArquivo) -> Textos {
        Textos {
            tipo: info.tipo,
            caminho: info.caminho.display().to_string(),
            tamanho: formatar_bytes(info.tamanho),
            modificado: info.modificado_em.map_or_else(
                || "—".to_string(),
                |t| chrono::DateTime::<chrono::Local>::from(t).format("%d/%m/%Y %H:%M").to_string(),
            ),
        }
    }
}

/// A ficha do arquivo de dados (veja o topo do módulo).
pub(crate) struct Ficha {
    caminho: PathBuf,
    nome: String,
    /// O tipo pela extensão, antes de o serviço responder (o ícone da aba e da faixa).
    tipo: Option<TipoDeArquivo>,
    /// O que o serviço disse do arquivo (None antes da primeira resposta), ou o erro dele.
    info: Option<Result<Textos, String>>,
    leitura: Option<Promessa<InfoArquivo>>,
    pedido: Option<PedidoArquivo>,
}

impl Ficha {
    pub(crate) fn nova(caminho: &Path) -> Ficha {
        let (nome, tipo) = (nome_do_arquivo(caminho), tipo_pela_extensao(caminho));
        Ficha { caminho: caminho.to_path_buf(), nome, tipo, info: None, leitura: None, pedido: None }
    }

    /// O nome do arquivo (o da aba).
    pub(crate) fn nome(&self) -> &str {
        &self.nome
    }

    /// A folha do tipo (a da aba e a da faixa); sem tipo conhecido, a silhueta do arquivo.
    pub(crate) fn icone(&self) -> Icone {
        self.tipo().map_or(Icone::Arquivo, icone_do_arquivo)
    }

    fn tipo(&self) -> Option<TipoDeArquivo> {
        match &self.info {
            Some(Ok(t)) => Some(t.tipo),
            _ => self.tipo,
        }
    }

    /// O que a pessoa pediu desde a última chamada (a casca cumpre).
    pub(crate) fn tomar_pedido(&mut self) -> Option<PedidoArquivo> {
        self.pedido.take()
    }

    /// Lê de novo o que o arquivo é quando a aba aparecer (a volta do foco, outra aba que mudou o arquivo).
    pub(crate) fn reler(&mut self) {
        if self.leitura.is_none() {
            self.info = None;
        }
    }

    /// Pede a leitura (se ainda não pediu) e toma a resposta que chegou.
    fn acompanhar(&mut self, egui: &egui::Context, ctx: &Contexto) {
        if self.info.is_none() && self.leitura.is_none() {
            let caminho = self.caminho.clone();
            self.leitura = Some(ctx.tarefa(egui, move |s, _| s.info_arquivo(&caminho)));
        }
        if let Some(r) = self.leitura.as_mut().and_then(Promessa::tomar) {
            self.leitura = None;
            self.info = Some(r.map(|i| Textos::de(&i)).map_err(|e| e.mensagem));
            egui.request_repaint();
        }
    }

    /// Os dois botões numa linha centrada (a largura dela vem do quadro anterior: o primeiro quadro a mede).
    fn botoes(&mut self, ui: &mut Ui, ligados: bool) {
        let id = Id::new(("tela_arquivo::botoes", &self.caminho));
        let largura_antes: Option<f32> = ui.ctx().data(|d| d.get_temp(id));
        let livre = ui.available_width();
        let largura = ui
            .horizontal(|ui| {
                ui.add_space(largura_antes.map_or(0.0, |l| ((livre - l) / 2.0).max(0.0)));
                let inicio = ui.cursor().min.x;
                if ui.add(Botao::primario(ABRIR_NO_PADRAO).habilitado(ligados)).clicked() {
                    self.pedido = Some(PedidoArquivo::AbrirNoPadrao(self.caminho.clone()));
                }
                if ui.add(Botao::new(MOSTRAR_NA_PASTA).habilitado(ligados)).clicked() {
                    self.pedido = Some(PedidoArquivo::MostrarNaPasta(self.caminho.clone()));
                }
                ui.min_rect().right() - inicio
            })
            .inner;
        if largura_antes.is_none_or(|l| (l - largura).abs() > 0.5) {
            ui.ctx().data_mut(|d| d.insert_temp(id, largura));
            ui.ctx().request_repaint();
        }
    }
}

impl estado::Documento for Ficha {
    fn anatomia(&self) -> Anatomia {
        Anatomia::default().com_faixa()
    }

    fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let e = &paleta(ui).estudio;
        let (r, _) = ui.allocate_exact_size(vec2(LADO_FOLHA_FAIXA, LADO_FOLHA_FAIXA), Sense::hover());
        icones::pintar(ui.painter(), r, self.icone(), e.t2);
        let g = galley(ui, &self.nome, fontes::fonte(13.0, Peso::Medio), e.t);
        let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(rect.min, g, e.t);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &self.nome));
        let caminho = self.caminho.display().to_string();
        let g = galley_caminho(ui, &caminho, fontes::fonte(12.0, Peso::Leve), e.t3, ui.available_width());
        let encurtado = g.text() != caminho;
        let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(rect.min, g, e.t3);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &caminho));
        if encurtado {
            resp.on_hover_text(&caminho);
        }
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.acompanhar(ui.ctx(), ctx);
        ui.add_space(ESPACO_CONTEUDO);
        let ligados = match &self.info {
            None => {
                carregando(ui, "Lendo o arquivo…");
                return;
            }
            Some(Ok(t)) => {
                let frase = frase_da_ficha(t.tipo);
                aviso_tom(ui, &[Trecho::Normal(&frase)], TomAviso::Informacao);
                true
            }
            Some(Err(mensagem)) => {
                aviso_texto(ui, mensagem, true);
                false
            }
        };
        ui.add_space(ESPACO_CONTEUDO);
        self.botoes(ui, ligados);
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.acompanhar(ui.ctx(), ctx);
        let caminho = self.caminho.display().to_string();
        let (tipo, tamanho, modificado) = match &self.info {
            Some(Ok(t)) => (Some(t.tipo.rotulo()), Some(t.tamanho.as_str()), Some(t.modificado.as_str())),
            _ => (self.tipo.map(TipoDeArquivo::rotulo), None, None),
        };
        let caminho_a_copiar = match &self.info {
            Some(Ok(t)) => t.caminho.clone(),
            _ => caminho,
        };
        let mut copiar = false;
        let _ = Secao::new("propriedades::arquivo_de_dados", "Arquivo").primeira(true).mostrar(ui, |ui| {
            ficha(ui, |d| {
                d.linha_texto("Tipo", tipo, "—");
                d.linha_texto("Tamanho", tamanho, "—");
                d.linha_texto("Modificado em", modificado, "—");
                d.linha("Caminho", |ui| copiar = linha_do_caminho(ui, &caminho_a_copiar));
            });
        });
        if copiar {
            ui.ctx().copy_text(caminho_a_copiar);
            toast_concluido(ui.ctx(), MSG_CAMINHO_COPIADO);
        }
    }

    fn status(&self, _estado: &Estado) -> Option<String> {
        match &self.info {
            Some(Ok(t)) => Some(format!("{} · {}", t.tipo.rotulo(), t.tamanho)),
            _ => self.tipo.map(|t| t.rotulo().to_string()),
        }
    }

    fn ocupada(&self) -> bool {
        self.leitura.is_some()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn frase_e_tipo_pela_extensao() {
        assert_eq!(
            frase_da_ficha(TipoDeArquivo::Csv),
            "O editor de CSV ainda não está pronto. Enquanto isso, o arquivo abre no programa padrão do Windows."
        );
        assert!(frase_da_ficha(TipoDeArquivo::Xlsx).starts_with("O editor de Excel "));
        assert!(frase_da_ficha(TipoDeArquivo::Texto).starts_with("O editor de texto "));
        let t = Ficha::nova(Path::new(r"C:\dados\Vendas.CSV"));
        assert_eq!((t.nome(), t.tipo), ("Vendas.CSV", Some(TipoDeArquivo::Csv)));
        assert_eq!(t.icone(), icone_do_arquivo(TipoDeArquivo::Csv));
        assert_eq!(Ficha::nova(Path::new(r"C:\x\sem")).icone(), Icone::Arquivo);
        assert_eq!(
            estado::Documento::status(&t, &estado::apoio_testes::Ambiente::vazio("ficha_status").estado),
            Some("CSV".into())
        );
    }

    /// Um editor de mentira (só o conteúdo).
    struct Editor;

    impl estado::Documento for Editor {
        fn conteudo(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}
        fn alterado(&self) -> bool {
            true
        }
    }

    #[test]
    fn tela_arquivo_escolhe_a_tela_pelo_tipo() {
        // CSV → o editor de CSV; JSON e texto (TXT, MD, LOG) → o de texto; o Excel → a planilha; o que não se sabe → a
        // ficha
        assert_eq!(tela_do_tipo(Some(TipoDeArquivo::Csv)), Tela::Csv);
        assert_eq!(tela_do_tipo(Some(TipoDeArquivo::Json)), Tela::Texto);
        assert_eq!(tela_do_tipo(Some(TipoDeArquivo::Texto)), Tela::Texto);
        assert_eq!(tela_do_tipo(Some(TipoDeArquivo::Xlsx)), Tela::Planilha);
        assert_eq!(tela_do_tipo(None), Tela::Ficha);
        for (nome, tipo) in [("a.csv", Tela::Csv), ("a.tsv", Tela::Csv), ("b.json", Tela::Texto), ("c.md", Tela::Texto)]
        {
            assert_eq!(tela_do_tipo(tipo_pela_extensao(Path::new(nome))), tipo, "{nome}");
        }
        // um editor no lugar da tela do tipo: o de CSV para o CSV, o de texto para o resto; o nome e a folha da aba
        // vêm do caminho, o resto da tela
        let csv = TelaArquivo::com_editor(Path::new(r"C:\dados\vendas_2025.csv"), Box::new(Editor));
        assert_eq!((csv.tela(), csv.nome()), (Tela::Csv, "vendas_2025.csv"));
        assert_eq!(csv.icone(), icone_do_arquivo(TipoDeArquivo::Csv));
        assert!(csv.doc_ref().alterado(), "o trait é o da tela");
        assert_eq!(csv.caminho(), Path::new(r"C:\dados\vendas_2025.csv"));
        let mut json = TelaArquivo::com_editor(Path::new(r"C:\dados\config.json"), Box::new(Editor));
        assert_eq!((json.tela(), json.icone()), (Tela::Texto, icone_do_arquivo(TipoDeArquivo::Json)));
        assert!(json.tomar_pedido().is_none() && !json.pergunta_aberta(), "o documento só pelo trait não pede nada");
        assert!(TelaArquivo::texto(Path::new("notas.txt"), Editor).doc().alterado());
        assert_eq!(TelaArquivo::csv(Path::new("x.csv"), Editor).tela(), Tela::Csv);
    }

    #[test]
    fn sem_o_recurso_telas_cai_na_ficha() {
        // sem o recurso `telas`, todo tipo abre na ficha, pelo mesmo ponto de encaixe; com ele, o texto e o JSON abrem
        // no editor de texto e o CSV no editor tabular
        let texto = if cfg!(feature = "telas") { Tela::Texto } else { Tela::Ficha };
        let csv = if cfg!(feature = "telas") { Tela::Csv } else { Tela::Ficha };
        let planilha = if cfg!(feature = "telas") { Tela::Planilha } else { Tela::Ficha };
        for (nome, esperada) in [
            ("vendas.csv", csv),
            ("config.json", texto),
            ("notas.txt", texto),
            ("leia-me.md", texto),
            ("planilha.xlsx", planilha),
            ("sem_extensao", Tela::Ficha),
        ] {
            let t = TelaArquivo::nova(Path::new(nome));
            assert_eq!(t.tela(), esperada, "{nome}");
            assert!(!t.pergunta_aberta());
            assert_eq!(t.nome(), nome);
            assert!(!t.doc_ref().alterado(), "a ficha não edita");
        }
    }

    #[test]
    fn tela_arquivo_escolhe_a_tela_de_planilha_para_xlsx_e_xlsm() {
        // o `.xlsx` e o `.xlsm` (com macros, o mesmo pacote) abrem na planilha só leitura; o `.xls` e o `.xlsb` nem
        // chegam aqui (ficam fora da árvore e do Abrir arquivo)
        for nome in ["relatorio_q3.xlsx", "macros.XLSM"] {
            assert_eq!(tela_do_tipo(tipo_pela_extensao(Path::new(nome))), Tela::Planilha, "{nome}");
            let t = TelaArquivo::nova(Path::new(nome));
            let esperada = if cfg!(feature = "telas") { Tela::Planilha } else { Tela::Ficha };
            assert_eq!(t.tela(), esperada, "{nome}");
            assert_eq!(t.nome(), nome);
            assert_eq!(t.icone(), icone_do_arquivo(TipoDeArquivo::Xlsx));
            assert!(!t.doc_ref().alterado() && !t.pergunta_aberta(), "a planilha é só leitura");
        }
        assert_eq!(tela_do_tipo(tipo_pela_extensao(Path::new("antigo.xls"))), Tela::Ficha);
    }

    #[test]
    fn sem_o_recurso_telas_xlsx_cai_na_ficha() {
        // o ponto de encaixe da planilha: com o recurso, a `TelaPlanilha`; sem ele, a ficha de sempre
        let t = tela_de_planilha(Path::new(r"C:\dados\relatorio_q3.xlsx"));
        if cfg!(feature = "telas") {
            assert_eq!(t.tela(), Tela::Planilha);
        } else {
            assert_eq!(t.tela(), Tela::Ficha);
        }
        assert_eq!(t.caminho(), Path::new(r"C:\dados\relatorio_q3.xlsx"));
    }
}
