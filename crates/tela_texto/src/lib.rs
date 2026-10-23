//! O editor de texto e de JSON (pranchetas 8 e 15, estado N1): a tela de um
//! arquivo `.txt`, `.md`, `.log` ou `.json` aberto pela ficha do arquivo, como um [`estado::Documento`] que se
//! sustenta sozinho (a casca só escolhe qual tela desenhar pelo tipo do arquivo: o `TelaArquivo` do editor).
//!
//! # API (o contrato com a casca)
//!
//! ```text
//! TelaTexto::nova(caminho: PathBuf) -> TelaTexto      não lê nada: a leitura começa no primeiro quadro
//! TelaTexto::nome() · caminho() · tipo() -> TipoDeTexto { Texto, Json } · icone() -> Icone (a folha do tipo)
//! TelaTexto::modo() -> Modo { Inicial, Lendo, Editor, Leitor, Falhou }
//! TelaTexto::tomar_pedido() -> Option<PedidoDeArquivo>   "Abrir no programa padrão" / "Mostrar na pasta" (como a ficha)
//! impl estado::Documento for TelaTexto
//!   anatomia (faixa, rodapé, rolagem própria) · avisos (o leitor) · faixa · conteudo · rodape · ofertas/executar ·
//!   busca/evento_busca · propriedades · status_do_arquivo · alterado · salvando · tomar_erro_de_gravacao · ocupada ·
//!   em_segundo_plano
//! ```
//!
//! Verbos: **Salvar**, **Desfazer**, **Refazer**, **Atualizar** (o F5) e **Buscar** (o Ctrl+F, "Buscar em notas.txt":
//! no texto inteiro, no editor e no leitor; o módulo `busca`).
//! O **Formatar** é botão da faixa. Quem fecha a aba pergunta pelo [`estado::Documento::alterado`]; o Ctrl+S da casca
//! chama o verbo Salvar; o Ctrl+Z e o Ctrl+Y só valem fora de campo de texto (com o foco no editor, quem trata é o
//! egui, pelo mesmo `Undoer` que o Desfazer da barra usa).
//!
//! # O que a tela faz
//!
//! - **Corpos** (`Modo`): *Lendo* (a tarefa lê o arquivo: codificação, fim de linha dominante, texto só com `\n`);
//!   *Editor* (até [`texto::limites::LIMITE_EDICAO`], 0,5 MiB: o [`interface::pecas::campos::AreaTexto`] em modo código,
//!   plano, com a calha dos números); *Leitor* (acima disso e até 32 MiB: só leitura, sem `TextEdit`, desenhado por
//!   linha, só as linhas da vista, com as mesmas cores); *Falhou* (acima de 32 MiB, UTF-16, arquivo sumido: o erro do
//!   núcleo num aviso, com os botões da ficha).
//! - **Faixa** (42 px), só no JSON (U4 da revisão de UI de 22/10/2026: o nome já está na aba, e o TXT fica sem
//!   faixa): a migalha do cursor a partir da raiz (`raiz › conexoes › 0 › tempo_limite`) e, à direita, o círculo de
//!   estado de 26 px (✓ verde e a dica "JSON válido", ou o ícone de erro com a frase na dica e no rótulo; o clique leva
//!   o cursor ao erro) e o botão **Formatar** (desligado com o JSON inválido, com a dica "Corrija o erro para
//!   formatar"); no leitor, sem o Formatar.
//! - **Faixa de erro** (JSON inválido, prancheta 15, revisão de UI de 19/10/2026): embaixo da faixa, enquanto o JSON
//!   estiver inválido, a faixa de erro da Consulta SQL (`.erro`) com "Linha 14, coluna 7:" em `--perigo`, o motivo em
//!   `--t2` e o botão [`IR_PARA_O_ERRO`], que põe o cursor no erro (no leitor, rola até a linha dele). O editor e o
//!   leitor ficam com o que sobra embaixo dela.
//! - **Rodapé** (34 px): "24 linhas", "· Espaços: 2" (JSON; "Tabulação", "Sem recuo"), "· Sem cores acima de 200 KB" e,
//!   à direita, "Linha 9, coluna 25" e a contagem da busca da barra ("3 de 41"). No leitor, as linhas, o tamanho e a
//!   etiqueta "Somente leitura".
//! - **Propriedades**: no JSON, a *Estrutura do documento* (árvore sob demanda, 200 filhos por página, o nó ativo é o
//!   do cursor; com o JSON inválido, a última versão válida esmaecida) e o *Arquivo* (com Codificação e Fim de linha);
//!   no texto, *Texto* (Codificação, Fim de linha, Linhas, Caracteres) e *Arquivo*.
//! - **JSON**: as cores (chave, texto, número, literal, pontuação), a validação 250 ms depois da última tecla numa
//!   tarefa (resposta de versão velha descartada), o N1 (frase, linha com fundo, sublinhado ondulado e número em
//!   `--perigo`), o Formatar (um passo no desfazer) e o aviso de erro no toast ao salvar ("salvo com o erro da linha 14").
//! - **Buscar** (Ctrl+F): numa tarefa, no texto inteiro (também no leitor de até 32 MiB), sem diferenciar maiúsculas
//!   nem acentos. O trecho achado fica pintado como na Consulta SQL (no editor, `AreaTexto::destacar`, com o cursor no
//!   fim dele; no leitor, o fundo `--sel`) e a rolagem vai até ele; Enter e Shift+Enter andam (com volta); a faixa diz
//!   "3 de 41" ("41+" enquanto conta); Esc limpa e volta ao cursor (ou à linha) de antes.
//! - **Salvar**: numa tarefa (`texto::gravar`: atômico, com a codificação, o BOM e o fim de linha do arquivo); antes,
//!   `mudou_no_disco` (a pergunta "Salvar mesmo assim"); o "alterado" é o texto diferir do salvo (desfazer até o salvo
//!   tira o ponto). **Atualizar** com alteração pergunta antes de descartar.
//!
//! # Testes
//!
//! `cargo test -p tela_texto`: as regras puras nos módulos e `tests/texto.rs` (a tela no `egui_kittest`, pelo
//! `estado::apoio_testes::palco_documento`, com as capturas `texto-*`, `json-*` e `json-invalido-*` em
//! `capturas/rust/`). A medição do `TextEdit` com 0,5, 1 e 2 MiB é `#[ignore]`.

#![forbid(unsafe_code)]

mod busca;
mod carga;
mod edicao;
mod estrutura;
mod json;
mod leitor;

use std::path::{Path, PathBuf};

use ::texto::json::{ErroJson, Recuo, detectar_recuo, formatar};
use ::texto::limites::tamanho_em_texto;
use ::texto::posicao::caractere_do_byte;
use comum::ErroBanco;
use egui::{Align, Context, CornerRadius, Id, Layout, Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, vec2};
use estado::{Anatomia, Contexto, InfoArquivo, Oferta, Ofertas, Promessa, Situacao, StatusDeArquivo, Verbo};
use interface::codigo::{LIMITE_REALCE, Linguagem};
use interface::fontes::{self, Peso};
use interface::formato::formatar_numero;
use interface::icones::{self, EstadoIcone, Icone, TipoArquivo};
use interface::pecas::arranjo::em_linha;
use interface::pecas::avisos::{Aviso, TomAviso, aviso_texto, carregando};
use interface::pecas::botao::{ALTURA_PEQUENO, Botao};
use interface::pecas::campos::AreaTexto;
use interface::pecas::detalhes::Detalhes;
use interface::pecas::documento::Documento as DocumentoPeca;
use interface::pecas::janela::{Confirmacao, confirmar};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::rotulos::{TipoPill, pill};
use interface::pecas::texto::{Trecho, galley, texto, texto_rico};
use interface::pecas::toast::{toast, toast_atencao, toast_concluido, toast_erro};
use interface::tema::paleta;

use crate::carga::{Lido, ler};
use crate::edicao::{Editor, FimDaGravacao, colocar_cursor, cursor_do_editor, preparar_campo};
use crate::leitor::Leitor;

/// Os rótulos dos dois botões da ficha (o mesmo texto da ficha do arquivo).
pub const ABRIR_NO_PADRAO: &str = "Abrir no programa padrão";
pub const MOSTRAR_NA_PASTA: &str = "Mostrar na pasta";

/// A frase do toast de um Salvar sem nada a salvar.
pub const MSG_NADA_A_SALVAR: &str = "Nenhuma alteração para salvar.";
/// A pergunta do Salvar quando o arquivo mudou no disco.
pub const MSG_MUDOU_NO_DISCO: &str =
    "O arquivo mudou no disco desde que foi aberto. Salvar mesmo assim substitui o que está lá.";

/// O botão da faixa de erro do JSON inválido, que leva o cursor ao erro.
pub const IR_PARA_O_ERRO: &str = "Ir para o erro";
/// O começo da migalha do JSON, o nome que a estrutura das Propriedades dá ao valor de cima.
pub const RAIZ_DA_MIGALHA: &str = "raiz";
/// A dica do Formatar desligado pelo JSON inválido.
const DICA_FORMATAR_COM_ERRO: &str = "Corrija o erro para formatar";

/// O Formatar com o JSON que já está formatado.
const MSG_JA_FORMATADO: &str = "O JSON já está formatado.";
/// O Formatar que levaria o texto para além do limite de edição (o editor ficaria lento e o desfazer, enorme).
pub const MSG_FORMATADO_GRANDE_DEMAIS: &str =
    "O JSON formatado passaria do limite de edição (512 KB). Formate-o num programa de texto.";
/// O F5 cujo texto mudou enquanto o arquivo era lido de novo: o que se digitou fica.
pub const MSG_MUDOU_DURANTE_A_LEITURA: &str =
    "O arquivo não foi lido de novo: o texto mudou durante a leitura. Use Atualizar outra vez para descartar.";

/// O que o arquivo é para o editor: o JSON leva cores, validação, estrutura e Formatar; o resto (TXT, MD, LOG), só texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoDeTexto {
    Texto,
    Json,
}

impl TipoDeTexto {
    /// O tipo pela extensão do caminho (`.json`, sem diferença de caixa): o resto é texto.
    pub fn do_caminho(caminho: &Path) -> TipoDeTexto {
        let e_json = caminho.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if e_json { TipoDeTexto::Json } else { TipoDeTexto::Texto }
    }

    /// "JSON" ou "Texto", como a barra de status o diz.
    pub fn rotulo(self) -> &'static str {
        match self {
            TipoDeTexto::Json => "JSON",
            TipoDeTexto::Texto => "Texto",
        }
    }
}

/// Em que corpo a tela está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Antes do primeiro quadro: nada foi pedido ainda.
    Inicial,
    /// A tarefa lê o arquivo.
    Lendo,
    /// O editor (até 0,5 MiB).
    Editor,
    /// O leitor, só leitura (acima de 0,5 MiB).
    Leitor,
    /// O arquivo não abriu (grande demais, UTF-16, sumiu…): o erro com os botões da ficha.
    Falhou,
}

/// O que os botões da ficha pedem à casca (tomado por [`TelaTexto::tomar_pedido`]; a casca abre o programa ou o
/// Explorador de Arquivos numa tarefa, como faz com a ficha).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedidoDeArquivo {
    /// O arquivo no programa associado.
    AbrirNoPadrao(PathBuf),
    /// O Explorador de Arquivos com o arquivo selecionado.
    MostrarNaPasta(PathBuf),
}

enum Corpo {
    Inicial,
    Lendo,
    Editor(Box<Editor>),
    Leitor(Box<Leitor>),
    Falhou(ErroBanco),
}

/// O que a janela de confirmação aberta está perguntando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pergunta {
    SalvarMesmoAssim,
    DescartarParaAtualizar,
}

/// A tela de um arquivo de texto ou JSON (veja o topo do módulo).
pub struct TelaTexto {
    caminho: PathBuf,
    nome: String,
    tipo: TipoDeTexto,
    /// O que dá os ids do editor e da rolagem do leitor (um por arquivo).
    sal: Id,
    corpo: Corpo,
    carga: Option<Promessa<Lido>>,
    /// A versão do texto do editor quando a releitura (o F5) começou: o que se digitou enquanto ela lia não se perde.
    versao_ao_reler: Option<u64>,
    confirmacao: Option<Confirmacao>,
    pergunta: Option<Pergunta>,
    erro_de_gravacao: Option<String>,
    pedido: Option<PedidoDeArquivo>,
    /// O relógio do egui no último quadro (a validação do JSON e o desfazer o usam).
    agora: f64,
    /// A busca da barra (Ctrl+F) no texto inteiro.
    busca: busca::BuscaTexto,
}

impl TelaTexto {
    /// A tela do arquivo `caminho`. Não lê nada aqui: a leitura é uma tarefa e começa no primeiro quadro.
    pub fn nova(caminho: PathBuf) -> TelaTexto {
        let nome =
            caminho.file_name().map_or_else(|| caminho.display().to_string(), |n| n.to_string_lossy().into_owned());
        TelaTexto {
            tipo: TipoDeTexto::do_caminho(&caminho),
            sal: Id::new(("databox::tela_texto", &caminho)),
            caminho,
            nome,
            corpo: Corpo::Inicial,
            carga: None,
            versao_ao_reler: None,
            confirmacao: None,
            pergunta: None,
            erro_de_gravacao: None,
            pedido: None,
            agora: 0.0,
            busca: busca::BuscaTexto::default(),
        }
    }

    /// O nome do arquivo (o da aba).
    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    pub fn tipo(&self) -> TipoDeTexto {
        self.tipo
    }

    /// A folha do tipo (a da aba e a da faixa).
    pub fn icone(&self) -> Icone {
        Icone::Folha(match self.tipo {
            TipoDeTexto::Json => TipoArquivo::Json,
            TipoDeTexto::Texto => TipoArquivo::Txt,
        })
    }

    pub fn modo(&self) -> Modo {
        match &self.corpo {
            Corpo::Inicial => Modo::Inicial,
            Corpo::Lendo => Modo::Lendo,
            Corpo::Editor(_) => Modo::Editor,
            Corpo::Leitor(_) => Modo::Leitor,
            Corpo::Falhou(_) => Modo::Falhou,
        }
    }

    /// O texto à vista, no editor ou no leitor.
    pub fn texto(&self) -> Option<&str> {
        match &self.corpo {
            Corpo::Editor(e) => Some(&e.texto),
            Corpo::Leitor(l) => Some(&l.texto),
            _ => None,
        }
    }

    /// O erro que impediu de abrir o arquivo (o corpo *Falhou*).
    pub fn falha(&self) -> Option<&str> {
        match &self.corpo {
            Corpo::Falhou(e) => Some(&e.mensagem),
            _ => None,
        }
    }

    /// Há uma pergunta aberta na tela ("Salvar mesmo assim" depois de o arquivo mudar no disco, ou o "Descartar" do
    /// Atualizar). A fila de perguntas ao fechar espera enquanto ela está aberta: o Salvar que termina sem gravar, sem
    /// erro e com a pergunta aberta está esperando a resposta da pessoa (a aba precisa estar à vista para a janela
    /// desenhar); fechada a pergunta, com o documento ainda alterado, ela foi cancelada.
    pub fn pergunta_aberta(&self) -> bool {
        self.confirmacao.is_some()
    }

    /// O que os botões da ficha pediram desde a última chamada (a casca cumpre).
    pub fn tomar_pedido(&mut self) -> Option<PedidoDeArquivo> {
        self.pedido.take()
    }

    /// O erro de JSON do documento (no editor, só o do texto de agora; no leitor, o do arquivo).
    pub fn erro_de_json(&self) -> Option<&::texto::json::ErroJson> {
        match &self.corpo {
            Corpo::Editor(e) => e.json.erro.as_ref(),
            Corpo::Leitor(l) => l.json.erro.as_ref(),
            _ => None,
        }
    }

    /// O arquivo em disco já não é o que foi lido, ou está sendo trocado: para testes e para a casca saber que há
    /// trabalho em andamento.
    fn trabalhando(&self) -> bool {
        let tarefa = self.carga.is_some() || matches!(self.corpo, Corpo::Inicial | Corpo::Lendo);
        let editor = match &self.corpo {
            Corpo::Editor(e) => e.gravando.is_some() || e.json.ocupado(),
            Corpo::Leitor(l) => l.json.ocupado(),
            _ => false,
        };
        tarefa || editor || self.busca.ocupada()
    }

    fn id_do_editor(&self) -> Id {
        Id::new(self.sal)
    }

    // Os ganchos dos testes (`#[doc(hidden)]`): o que a tela sabe de si e que a interface não mostra.

    /// O id do campo de texto do editor (o que guarda o cursor e o desfazer no egui).
    #[doc(hidden)]
    pub fn id_do_campo(&self) -> Id {
        self.id_do_editor()
    }

    /// Quantas análises do JSON foram pedidas e quantas respostas velhas foram descartadas.
    #[doc(hidden)]
    pub fn contas_da_validacao(&self) -> (usize, usize) {
        match &self.corpo {
            Corpo::Editor(e) => (e.json.pedidas, e.json.descartadas),
            Corpo::Leitor(l) => (l.json.pedidas, l.json.descartadas),
            _ => (0, 0),
        }
    }

    /// Quantas vezes a oferta do Desfazer e do Refazer foi recalculada.
    #[doc(hidden)]
    pub fn recalculos_da_oferta(&self) -> usize {
        match &self.corpo {
            Corpo::Editor(e) => e.recalculos,
            _ => 0,
        }
    }

    /// As linhas que o leitor desenhou no último quadro.
    #[doc(hidden)]
    pub fn linhas_desenhadas(&self) -> std::ops::Range<usize> {
        match &self.corpo {
            Corpo::Leitor(l) => l.desenhadas.clone(),
            _ => 0..0,
        }
    }

    /// A linha (de 0) e os caracteres que o editor marca como erro, quando a validação é a do texto de agora.
    #[doc(hidden)]
    pub fn marca_do_erro(&mut self) -> Option<(usize, std::ops::Range<usize>)> {
        match &mut self.corpo {
            Corpo::Editor(e) => e.marca_do_erro().map(|m| (m.linha, m.caracteres)),
            _ => None,
        }
    }

    /// A estrutura à vista é a última versão válida (o JSON está inválido e já houve uma estrutura).
    #[doc(hidden)]
    pub fn estrutura_e_a_ultima_valida(&self) -> bool {
        match &self.corpo {
            Corpo::Editor(e) => e.json.erro.is_some() && e.json.valida.is_some(),
            _ => false,
        }
    }

    // ---------------------------------------------------------------------------------------- a cada quadro

    /// O que acontece antes de qualquer desenho: a leitura que chegou (ou a pedida agora), o fim da gravação e a
    /// validação do JSON. Barato e sem efeito se nada chegou.
    fn sincronizar(&mut self, egui: &Context) {
        self.agora = egui.input(|i| i.time);
        if matches!(self.corpo, Corpo::Inicial) && self.carga.is_none() {
            self.corpo = Corpo::Lendo;
            self.iniciar_carga(egui);
        }
        if let Some(r) = self.carga.as_mut().and_then(Promessa::tomar) {
            self.carga = None;
            let reler_desde = self.versao_ao_reler.take();
            let digitou = matches!((&self.corpo, reler_desde), (Corpo::Editor(e), Some(v)) if e.versao != v);
            match r {
                // o texto mudou enquanto a releitura lia: o que se digitou fica, e o arquivo não é trocado
                Ok(_) if digitou => toast_atencao(egui, MSG_MUDOU_DURANTE_A_LEITURA),
                Ok(lido) => self.instalar(lido, egui),
                // a releitura que falhou com texto por salvar no editor não o joga fora
                Err(e) if digitou || matches!(&self.corpo, Corpo::Editor(ed) if ed.alterado) => {
                    toast_erro(egui, e.mensagem);
                }
                Err(mut e) => {
                    // a mesma frase da ficha do arquivo para o que sumiu
                    if e.status == 404 {
                        e.mensagem = format!("O arquivo não existe (ou foi movido): {}.", self.caminho.display());
                    }
                    self.corpo = Corpo::Falhou(e);
                }
            }
            egui.request_repaint();
        }
        let agora = self.agora;
        let mut fim = None;
        match &mut self.corpo {
            Corpo::Editor(e) => {
                fim = e.fim_da_gravacao();
                if e.tipo == TipoDeTexto::Json {
                    e.json.passo(egui, agora, e.versao, &e.texto);
                }
            }
            Corpo::Leitor(l) => l.json.passo(egui, agora, 0, ""),
            _ => {}
        }
        if let Some(fim) = fim {
            self.terminou_a_gravacao(fim, egui);
        }
    }

    fn iniciar_carga(&mut self, egui: &Context) {
        let (caminho, tipo) = (self.caminho.clone(), self.tipo);
        self.versao_ao_reler = match &self.corpo {
            Corpo::Editor(e) => Some(e.versao),
            _ => None,
        };
        self.carga = Some(Promessa::rodar(egui, move || ler(&caminho, tipo)));
    }

    /// Põe o arquivo lido no corpo certo: o editor, ou o leitor acima do limite de edição.
    fn instalar(&mut self, lido: Lido, egui: &Context) {
        let id = self.id_do_editor();
        if lido.grande {
            self.corpo = Corpo::Leitor(Box::new(Leitor::novo(lido)));
        } else {
            preparar_campo(egui, id, lido.texto.len());
            self.corpo = Corpo::Editor(Box::new(Editor::novo(lido, self.tipo)));
        }
        self.erro_de_gravacao = None;
    }

    fn terminou_a_gravacao(&mut self, fim: FimDaGravacao, egui: &Context) {
        match fim {
            FimDaGravacao::PerguntarDisco => {
                self.pergunta = Some(Pergunta::SalvarMesmoAssim);
                self.confirmacao = Some(
                    Confirmacao::new(MSG_MUDOU_NO_DISCO)
                        .titulo("O arquivo mudou no disco")
                        .rotulo("Salvar mesmo assim")
                        .perigo(true),
                );
            }
            FimDaGravacao::Gravado { erro } => {
                match erro {
                    Some(e) => toast_atencao(egui, format!("{} salvo com o erro da linha {}.", self.nome, e.linha)),
                    None => toast_concluido(egui, format!("{} salvo.", self.nome)),
                }
                if let Corpo::Editor(e) = &mut self.corpo {
                    let id = Id::new(self.sal);
                    e.recalcular_oferta(egui, id);
                }
            }
            FimDaGravacao::Falhou(e) => {
                toast_erro(egui, e.mensagem.clone());
                self.erro_de_gravacao = Some(e.mensagem);
            }
        }
    }

    // ---------------------------------------------------------------------------------------- os verbos

    fn salvar(&mut self, egui: &Context, forcar: bool) {
        let caminho = self.caminho.clone();
        if let Corpo::Editor(e) = &mut self.corpo {
            if !e.alterado && !forcar {
                toast(egui, MSG_NADA_A_SALVAR);
                return;
            }
            self.erro_de_gravacao = None;
            e.gravar(egui, &caminho, forcar);
        }
    }

    /// O F5: com alteração pergunta antes de descartar; sem ela, lê de novo.
    fn atualizar(&mut self, egui: &Context) {
        if self.carga.is_some() {
            return;
        }
        let alterado = matches!(&self.corpo, Corpo::Editor(e) if e.alterado);
        if alterado {
            self.pergunta = Some(Pergunta::DescartarParaAtualizar);
            self.confirmacao = Some(
                Confirmacao::new(format!("Descartar as alterações de {} e ler o arquivo de novo?", self.nome))
                    .titulo("Ler o arquivo de novo")
                    .rotulo("Descartar")
                    .perigo(true),
            );
        } else {
            self.iniciar_carga(egui);
        }
    }

    /// O Formatar: reescreve os espaços e as quebras, num passo do desfazer.
    fn formatar_agora(&mut self, egui: &Context) {
        let id = self.id_do_editor();
        let agora = self.agora;
        let Corpo::Editor(e) = &mut self.corpo else {
            return;
        };
        let recuo = detectar_recuo(&e.texto);
        match formatar(&e.texto, recuo) {
            Ok(novo) if novo == e.texto => toast(egui, MSG_JA_FORMATADO),
            // o texto formatado acima do limite de edição deixaria cada tecla lenta e o desfazer enorme
            Ok(novo) if novo.len() as u64 > ::texto::limites::LIMITE_EDICAO => {
                toast_atencao(egui, MSG_FORMATADO_GRANDE_DEMAIS)
            }
            Ok(novo) => e.substituir_como_um_passo(egui, id, novo, agora),
            Err(erro) => toast_atencao(egui, erro.to_string()),
        }
    }

    // ---------------------------------------------------------------------------------------- o desenho

    /// Leva o cursor ao byte `byte` do texto (a estrutura e o erro): o editor toma o cursor e o foco e rola até ele; o
    /// leitor rola até a linha.
    fn ir_para_o_byte(&mut self, byte: usize) {
        match &mut self.corpo {
            Corpo::Editor(e) => e.ir_para = Some(caractere_do_byte(&e.texto, byte)),
            Corpo::Leitor(l) => l.rolar_para = Some(l.linha_do_byte(byte)),
            _ => {}
        }
    }

    /// O cursor do editor em caracteres (0 antes de o egui guardar um).
    fn cursor(&mut self, egui: &Context) -> usize {
        let id = self.id_do_editor();
        let Corpo::Editor(e) = &mut self.corpo else {
            return 0;
        };
        let n = e.caracteres();
        cursor_do_editor(egui, id).unwrap_or(0).min(n)
    }

    fn desenhar_editor(&mut self, ui: &mut Ui) {
        let egui = ui.ctx().clone();
        let id = self.id_do_editor();
        let nome = self.nome.clone();
        let agora = self.agora;
        let cursor = self.cursor(&egui);
        let sal = self.sal;
        let Corpo::Editor(e) = &mut self.corpo else {
            return;
        };
        let p = paleta(ui);
        let rect = ui.available_rect_before_wrap();
        let (linha_do_cursor, _) = {
            let pos = e.posicao(cursor);
            (pos.linha - 1, pos.coluna)
        };
        let mut destacar = None;
        if let Some(i) = e.ir_para.take() {
            let n = e.caracteres();
            let i = i.min(n);
            colocar_cursor(&egui, id, i);
            egui.memory_mut(|m| m.request_focus(id));
            destacar = Some(i..(i + 1).min(n));
        }
        // o erro só se marca quando a validação é a do texto de agora (senão o lugar dele pode ter se mexido)
        let marca_do_erro = e.marca_do_erro();
        let linguagem = match e.tipo {
            TipoDeTexto::Json => Linguagem::Json,
            TipoDeTexto::Texto => Linguagem::Texto,
        };
        let mut area = AreaTexto::new(&mut e.texto)
            .codigo(true)
            .plana(true)
            .numeros_de_linha(true)
            .linguagem(linguagem)
            .altura(rect.height())
            .largura(rect.width())
            .id_salt(sal)
            .marcar_linhas(linha_do_cursor..linha_do_cursor + 1, p.estudio.selh);
        if let Some(m) = &marca_do_erro {
            area = area
                .marcar_linhas(m.linha..m.linha + 1, p.estudio.perigo.gamma_multiply(0.09))
                .erro_na_linha(m.linha)
                .sublinhar(m.caracteres.clone());
        }
        // o que a busca da barra achou (só o da versão de agora do texto), quando não há outro destaque
        let achado = self.busca.trecho.clone().filter(|(_, v)| *v == e.versao).map(|(t, _)| t);
        let rolar_achado = std::mem::take(&mut self.busca.rolar);
        if let Some(trecho) = destacar {
            area = area.destacar(trecho, true);
        } else if let Some(trecho) = achado {
            area = area.destacar(trecho, rolar_achado);
        }
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)));
        filho.set_clip_rect(rect.intersect(ui.clip_rect()));
        let saida = area.mostrar(&mut filho);
        let r = &saida.response.response;
        let rotulo = format!("Texto de {nome}");
        egui.accesskit_node_builder(r.id, |no| no.set_label(&*rotulo));
        if r.changed() {
            e.mudou(agora);
            e.oferta_apos_digitar(&egui, id, agora);
        } else if e.pode_refazer && r.has_focus() && cursor_do_editor(&egui, id).is_some_and(|c| c != cursor) {
            // com o foco, o egui esvazia o refazer quando o cursor anda: a oferta acompanha
            e.recalcular_oferta(&egui, id);
        } else {
            e.oferta_pendente(&egui, id, agora);
        }
    }

    /// A faixa de erro do JSON inválido, no alto do conteúdo (logo embaixo da faixa do documento), enquanto o JSON
    /// estiver inválido; o "Ir para o erro" leva o cursor (ou a rolagem do leitor) ao erro. O editor e o leitor
    /// ocupam o que sobra, encostados nela.
    fn desenhar_faixa_de_erro(&mut self, ui: &mut Ui) {
        let Some(erro) = self.erro_de_json().cloned() else {
            return;
        };
        ui.spacing_mut().item_spacing.y = 0.0;
        if faixa_de_erro(ui, self.sal, &erro) {
            self.ir_para_o_byte(erro.byte);
        }
    }

    /// Um quadro do corpo de um arquivo que não abriu: o erro do núcleo e os botões da ficha, centrados.
    fn desenhar_falha(&mut self, ui: &mut Ui) {
        let Corpo::Falhou(erro) = &self.corpo else {
            return;
        };
        let mensagem = erro.mensagem.clone();
        // o arquivo que sumiu não tem o que abrir; o grande demais e o UTF-16 abrem no programa padrão
        let ligados = erro.status != 404;
        ui.add_space(18.0);
        aviso_texto(ui, &mensagem, true);
        ui.add_space(18.0);
        let (abrir, mostrar) = botoes_da_ficha(ui, self.sal, ligados);
        if abrir {
            self.pedido = Some(PedidoDeArquivo::AbrirNoPadrao(self.caminho.clone()));
        }
        if mostrar {
            self.pedido = Some(PedidoDeArquivo::MostrarNaPasta(self.caminho.clone()));
        }
    }

    fn janelas(&mut self, egui: &Context) {
        let Some(decisao) = confirmar(egui, &mut self.confirmacao) else {
            return;
        };
        let pergunta = self.pergunta.take();
        if !decisao {
            return;
        }
        match pergunta {
            Some(Pergunta::SalvarMesmoAssim) => self.salvar(egui, true),
            Some(Pergunta::DescartarParaAtualizar) => self.iniciar_carga(egui),
            None => {}
        }
    }

    // ---------------------------------------------------------------------------------------- faixa e rodapé

    /// A faixa do JSON (o TXT não tem faixa: o nome já está na aba, U4 da revisão de UI de 22/10/2026): a migalha do
    /// ponto do cursor, a partir da raiz (`raiz › conexoes › 0 › tempo_limite`), e, na ponta direita, o círculo de
    /// estado e o Formatar.
    fn faixa_do_arquivo(&mut self, ui: &mut Ui) {
        let egui = ui.ctx().clone();
        let cursor = self.cursor(&egui);
        let e = &paleta(ui).estudio;
        let nomes = match &mut self.corpo {
            Corpo::Editor(ed) if self.tipo == TipoDeTexto::Json => {
                let caminho = ed.posicao(cursor).caminho.clone();
                estrutura::nomes_da_migalha(&ed.json, &caminho)
            }
            _ => Vec::new(),
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(-1.0);
            let ultimo = nomes.len();
            let cor = if ultimo == 0 { e.t } else { e.t3 };
            texto(ui, RAIZ_DA_MIGALHA, 12.0, cor);
            for (i, nome) in nomes.iter().enumerate() {
                let (r, _) = ui.allocate_exact_size(vec2(11.0, 11.0), Sense::hover());
                icones::pintar(ui.painter(), r, Icone::SetaDireita, e.t3);
                if i + 1 == ultimo {
                    let g = galley(ui, nome, fontes::fonte(12.0, Peso::Medio), e.t);
                    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                    ui.painter().galley(rect.min, g, e.t);
                    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, nome));
                } else {
                    texto(ui, nome, 12.0, e.t3);
                }
            }
        });
        let leitor = matches!(self.corpo, Corpo::Leitor(_));
        let veredito = match &self.corpo {
            Corpo::Editor(ed) => Some(ed.json.veredito()),
            Corpo::Leitor(l) if self.tipo == TipoDeTexto::Json => Some(l.json.veredito()),
            _ => None,
        };
        let mut formatar_clicado = false;
        let mut ir_ao_erro = None;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !leitor && veredito.is_some() {
                let pode = matches!(veredito, Some(json::Veredito::Valido));
                let dica = match veredito {
                    Some(json::Veredito::Invalido(_)) => DICA_FORMATAR_COM_ERRO,
                    _ => "Formatar o JSON",
                };
                let b = Botao::ghost("Formatar").pequeno().habilitado(pode).dica(dica);
                formatar_clicado = ui.add(b).clicked();
            }
            if let Some(v) = &veredito {
                ir_ao_erro = circulo_de_estado(ui, v);
            }
        });
        if formatar_clicado {
            self.formatar_agora(&egui);
        }
        if let Some(byte) = ir_ao_erro {
            self.ir_para_o_byte(byte);
        }
    }

    /// O rodapé: as linhas, o recuo do JSON e, na ponta direita, a posição do cursor e a contagem da busca da barra
    /// ("3 de 41"); no leitor, as linhas, o tamanho e "Somente leitura" (a contagem e a posição moram no rodapé, U4
    /// da revisão de UI de 22/10/2026).
    fn rodape_do_arquivo(&mut self, ui: &mut Ui) {
        let egui = ui.ctx().clone();
        let cursor = self.cursor(&egui);
        let tipo = self.tipo;
        let busca = self.busca.rotulo();
        let e = &paleta(ui).estudio;
        match &mut self.corpo {
            Corpo::Editor(ed) => {
                let grande_para_cores = tipo == TipoDeTexto::Json && ed.texto.len() > LIMITE_REALCE;
                let (linhas, recuo) = {
                    let m = ed.medidas();
                    (m.linhas, m.recuo)
                };
                texto(ui, &linhas_em_texto(linhas), 12.0, e.t2);
                if tipo == TipoDeTexto::Json {
                    let frase = recuo.map_or_else(|| "Sem recuo".to_string(), Recuo::rotulo);
                    texto(ui, &format!("· {frase}"), 12.0, e.t3);
                }
                if grande_para_cores {
                    texto(ui, "· Sem cores acima de 200 KB", 12.0, e.t3);
                }
                let (linha, coluna) = {
                    let p = ed.posicao(cursor);
                    (p.linha, p.coluna)
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    texto(ui, &format!("Linha {linha}, coluna {coluna}"), 12.0, e.t3);
                    if let Some(rotulo) = &busca {
                        estado::busca::rotulo_na_faixa(ui, rotulo, e.t2);
                    }
                });
            }
            Corpo::Leitor(l) => {
                texto(ui, &linhas_em_texto(l.n_linhas()), 12.0, e.t2);
                texto(ui, &format!("· {}", tamanho_em_texto(l.carregado.bytes)), 12.0, e.t3);
                pill(ui, "Somente leitura", TipoPill::Neutra);
                if let Some(rotulo) = &busca {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        estado::busca::rotulo_na_faixa(ui, rotulo, e.t2);
                    });
                }
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------------------------------- propriedades

    fn info_do_arquivo(&self) -> InfoArquivo {
        let (tamanho, modificado) = match &self.corpo {
            Corpo::Editor(e) => (Some(e.carregado.bytes), e.carregado.modificado),
            Corpo::Leitor(l) => (Some(l.carregado.bytes), l.carregado.modificado),
            _ => (None, None),
        };
        InfoArquivo { caminho: self.caminho.clone(), tamanho, modificado }
    }

    /// "UTF-8" e "LF" como as Propriedades e a barra de status os dizem, quando o arquivo já foi lido.
    fn formato(&self) -> Option<(&'static str, String)> {
        match &self.corpo {
            Corpo::Editor(e) => Some((e.carregado.rotulo_da_codificacao(), e.carregado.rotulo_do_fim_de_linha())),
            Corpo::Leitor(l) => Some((l.carregado.rotulo_da_codificacao(), l.carregado.rotulo_do_fim_de_linha())),
            _ => None,
        }
    }

    fn secao_texto(&mut self, ui: &mut Ui) {
        let Some((codificacao, fim)) = self.formato() else {
            return;
        };
        let (linhas, caracteres) = match &mut self.corpo {
            Corpo::Editor(e) => {
                let m = e.medidas();
                (m.linhas, m.caracteres)
            }
            Corpo::Leitor(l) => (l.n_linhas(), l.caracteres),
            _ => return,
        };
        let _ = Secao::new(("propriedades::texto", self.sal), "Texto").primeira(true).mostrar(ui, |ui| {
            ficha(ui, |d| {
                d.linha_texto("Codificação", Some(codificacao), "—");
                d.linha_texto("Fim de linha", Some(&fim), "—");
                d.linha_texto("Linhas", Some(&formatar_numero(linhas)), "—");
                d.linha_texto("Caracteres", Some(&formatar_numero(caracteres)), "—");
            });
        });
    }
}

impl estado::Documento for TelaTexto {
    /// Só o JSON tem faixa (a migalha, o estado e o Formatar); o TXT não tem nada nela que a aba não diga.
    fn anatomia(&self) -> Anatomia {
        let base = if self.tipo == TipoDeTexto::Json { Anatomia::default().com_faixa() } else { Anatomia::default() };
        match self.corpo {
            Corpo::Editor(_) | Corpo::Leitor(_) => base.com_rodape().rolagem_propria(),
            Corpo::Inicial | Corpo::Lendo | Corpo::Falhou(_) => base,
        }
    }

    /// O leitor diz que abre só para leitura, com os botões da ficha.
    fn avisos(&mut self, doc: &mut DocumentoPeca, ui: &mut Ui, _ctx: &mut Contexto) {
        let Corpo::Leitor(l) = &self.corpo else {
            return;
        };
        let frase = format!(
            "Este arquivo tem {} e abre só para leitura. Para editar, use um programa de texto.",
            tamanho_em_texto(l.carregado.bytes)
        );
        let (mut abrir, mut mostrar) = (false, false);
        doc.aviso(ui, TomAviso::Informacao, |ui, cores| {
            texto_rico(ui, &[Trecho::Normal(&frase)], 12.0, cores.texto, cores.forte);
            em_linha(ui, 6.0, |ui| {
                abrir = ui.add(Botao::primario(ABRIR_NO_PADRAO).pequeno()).clicked();
                mostrar = ui.add(Botao::new(MOSTRAR_NA_PASTA).pequeno()).clicked();
            });
        });
        if abrir {
            self.pedido = Some(PedidoDeArquivo::AbrirNoPadrao(self.caminho.clone()));
        }
        if mostrar {
            self.pedido = Some(PedidoDeArquivo::MostrarNaPasta(self.caminho.clone()));
        }
    }

    fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        self.faixa_do_arquivo(ui);
    }

    fn conteudo(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        self.passo_da_busca(&egui);
        match &self.corpo {
            Corpo::Inicial | Corpo::Lendo => {
                ui.add_space(18.0);
                carregando(ui, "Lendo o arquivo…");
            }
            Corpo::Falhou(_) => self.desenhar_falha(ui),
            Corpo::Editor(_) => {
                self.desenhar_faixa_de_erro(ui);
                self.desenhar_editor(ui);
            }
            Corpo::Leitor(_) => {
                self.desenhar_faixa_de_erro(ui);
                let rect = ui.available_rect_before_wrap();
                let linguagem = if self.tipo == TipoDeTexto::Json { Linguagem::Json } else { Linguagem::Texto };
                let sal = self.sal;
                if let Corpo::Leitor(l) = &mut self.corpo {
                    l.desenhar(ui, sal, rect, linguagem);
                }
            }
        }
        // a validação que a digitação deste quadro agendou
        self.sincronizar(&egui);
        self.janelas(&egui);
    }

    fn rodape(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        self.rodape_do_arquivo(ui);
    }

    fn ofertas(&self, _estado: &estado::Estado) -> Ofertas {
        match &self.corpo {
            Corpo::Editor(e) => {
                let livre = e.gravando.is_none() && self.carga.is_none();
                Ofertas::default()
                    .com(
                        Verbo::Salvar,
                        Oferta::ligada()
                            .ligada_se(e.alterado && livre)
                            .com_dica(format!("Salvar {} (Ctrl+S)", self.nome)),
                    )
                    .com(Verbo::Desfazer, Oferta::ligada().ligada_se(e.pode_desfazer && livre))
                    .com(Verbo::Refazer, Oferta::ligada().ligada_se(e.pode_refazer && livre))
                    .com(Verbo::Atualizar, Oferta::ligada().ligada_se(livre))
                    .ligar(Verbo::Buscar)
            }
            Corpo::Leitor(_) => Ofertas::default()
                .com(Verbo::Atualizar, Oferta::ligada().ligada_se(self.carga.is_none()))
                .ligar(Verbo::Buscar),
            Corpo::Falhou(_) => {
                Ofertas::default().com(Verbo::Atualizar, Oferta::ligada().ligada_se(self.carga.is_none()))
            }
            Corpo::Inicial | Corpo::Lendo => Ofertas::default(),
        }
    }

    fn executar(&mut self, verbo: Verbo, egui: &Context, _ctx: &mut Contexto) {
        self.sincronizar(egui);
        let id = self.id_do_editor();
        let agora = self.agora;
        match verbo {
            Verbo::Salvar => self.salvar(egui, false),
            Verbo::Desfazer | Verbo::Refazer => {
                if let Corpo::Editor(e) = &mut self.corpo {
                    e.desfazer_ou_refazer(egui, id, verbo == Verbo::Refazer, agora);
                }
            }
            Verbo::Atualizar => self.atualizar(egui),
            _ => {}
        }
    }

    /// "Buscar em notas.txt": no texto inteiro, no editor e no leitor (o módulo `busca`).
    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        if !matches!(self.corpo, Corpo::Editor(_) | Corpo::Leitor(_)) {
            return None;
        }
        let dica = self.dica_da_busca();
        Some(estado::BuscaDaBarra { texto: &mut self.busca.texto, dica: dica.into() })
    }

    fn evento_busca(&mut self, evento: estado::EventoBusca, egui: &Context, _ctx: &mut Contexto) {
        self.sincronizar(egui);
        self.evento_da_busca(evento, egui);
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        self.sincronizar(&egui);
        let primeira_e_arquivo = match self.tipo {
            TipoDeTexto::Json => {
                let cursor = self.cursor(&egui);
                let sal = self.sal;
                let mut ir = None;
                match &mut self.corpo {
                    Corpo::Editor(e) => {
                        let caminho = e.posicao(cursor).caminho.clone();
                        ir = estrutura::secao(ui, sal, &mut e.json, &caminho);
                    }
                    Corpo::Leitor(l) => ir = estrutura::secao(ui, sal, &mut l.json, &[]),
                    _ => {}
                }
                if let Some(byte) = ir {
                    self.ir_para_o_byte(byte);
                }
                !matches!(self.corpo, Corpo::Editor(_) | Corpo::Leitor(_))
            }
            TipoDeTexto::Texto => {
                self.secao_texto(ui);
                !matches!(self.corpo, Corpo::Editor(_) | Corpo::Leitor(_))
            }
        };
        let info = self.info_do_arquivo();
        let formato = self.formato();
        let json = self.tipo == TipoDeTexto::Json;
        estado::documento::secao_arquivo_com(
            ui,
            ctx,
            &info,
            primeira_e_arquivo,
            |d: &mut Detalhes<'_>| {
                if let (true, Some((codificacao, fim))) = (json, &formato) {
                    d.linha_texto("Codificação", Some(codificacao), "—");
                    d.linha_texto("Fim de linha", Some(fim), "—");
                }
            },
            |_| {},
        );
    }

    fn status_do_arquivo(&self, _estado: &estado::Estado) -> Option<StatusDeArquivo> {
        let situacao = match &self.corpo {
            Corpo::Editor(_) if self.erro_de_gravacao.is_some() => Situacao::ComErro,
            Corpo::Editor(e) if e.alterado => Situacao::Alterado,
            Corpo::Editor(_) => Situacao::Salvo,
            Corpo::Leitor(_) => Situacao::SoLeitura,
            _ => return None,
        };
        let (codificacao, fim) = self.formato()?;
        Some(
            StatusDeArquivo::novo(self.nome.clone(), situacao)
                .com_detalhe(self.tipo.rotulo())
                .com_detalhe(codificacao)
                .com_detalhe(fim),
        )
    }

    fn ocupada(&self) -> bool {
        self.trabalhando()
    }

    fn em_segundo_plano(&mut self, egui: &Context, _ctx: &mut Contexto) {
        self.sincronizar(egui);
        self.passo_da_busca(egui);
    }

    fn alterado(&self) -> bool {
        matches!(&self.corpo, Corpo::Editor(e) if e.alterado)
    }

    fn alteracoes(&self) -> Option<usize> {
        None
    }

    fn salvando(&self) -> bool {
        matches!(&self.corpo, Corpo::Editor(e) if e.gravando.is_some())
    }

    fn tomar_erro_de_gravacao(&mut self) -> Option<String> {
        self.erro_de_gravacao.take()
    }
}

/// "1 linha", "24 linhas", "1.204 linhas".
fn linhas_em_texto(n: usize) -> String {
    format!("{} {}", formatar_numero(n), if n == 1 { "linha" } else { "linhas" })
}

/// O círculo de estado de 26 px da faixa (`.estado-v`): o ✓ verde ("JSON válido") ou o ícone de erro (a frase na dica e
/// no rótulo). Devolve o byte do erro se foi clicado.
fn circulo_de_estado(ui: &mut Ui, veredito: &json::Veredito) -> Option<usize> {
    let e = &paleta(ui).estudio;
    let (icone, frase, byte) = match veredito {
        json::Veredito::Valido => (EstadoIcone::Concluido, "JSON válido".to_string(), None),
        json::Veredito::Invalido(erro) => (EstadoIcone::Erro, erro.to_string(), Some(erro.byte)),
        json::Veredito::Verificando => (EstadoIcone::Informacao, "Verificando o JSON…".to_string(), None),
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    if resp.hovered() || resp.has_focus() {
        ui.painter().circle_filled(rect.center(), 13.0, e.h);
    }
    let lado = 16.0;
    let icone_rect = egui::Rect::from_center_size(rect.center(), vec2(lado, lado));
    icones::pintar(ui.painter(), icone_rect, Icone::Estado(icone), e.t2);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &frase));
    let resp = resp.on_hover_text(&frase);
    (resp.clicked() && byte.is_some()).then(|| byte.unwrap_or(0))
}

/// A faixa de erro do JSON inválido (prancheta 15, revisão de 19/10/2026): a `.erro` da Consulta SQL, de ponta a
/// ponta, com o sólido de erro de 24px, "Linha 14, coluna 7:" em 600 `--perigo`, o motivo em `--t2` e, na ponta
/// direita, o "Ir para o erro" (`.btn.pq`), os três centrados na altura da linha (`align-items:center`). Devolve se o
/// botão foi clicado.
fn faixa_de_erro(ui: &mut Ui, sal: Id, erro: &ErroJson) -> bool {
    // `gap:12px` entre o texto e o botão
    const GAP: f32 = 12.0;
    let lugar = format!("Linha {}, coluna {}:", erro.linha, erro.coluna);
    let motivo = format!(" {}.", erro.motivo.trim_end_matches('.'));
    let trechos = [Trecho::Forte(&lugar), Trecho::Normal(&motivo)];
    let id = sal.with("tela_texto::faixa_de_erro");
    let mut ir = false;
    let resp = Aviso::com_tom(TomAviso::Erro).faixa_do_documento(CornerRadius::ZERO).mostrar(ui, |ui, cores| {
        let largura = ui.available_width();
        let topo = ui.cursor().min;
        // a linha tem a altura do mais alto (o texto ou o botão); a do texto é a do quadro anterior (no primeiro, uma
        // linha de 18px), e quando muda o quadro seguinte já sai certo
        let alto_texto: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(18.0);
        let alto = alto_texto.max(ALTURA_PEQUENO);
        let linha = Rect::from_min_size(topo, vec2(largura, alto));
        let mut direita = ui.new_child(UiBuilder::new().max_rect(linha).layout(Layout::right_to_left(Align::Center)));
        ir = direita.add(Botao::new(IR_PARA_O_ERRO).pequeno()).clicked();
        let largura_texto = (largura - direita.min_rect().width() - GAP).max(40.0);
        let caixa = Rect::from_min_size(
            topo + vec2(0.0, ((alto - alto_texto) / 2.0).max(0.0)),
            vec2(largura_texto, f32::INFINITY),
        );
        let mut esquerda = ui.new_child(UiBuilder::new().max_rect(caixa).layout(Layout::top_down(Align::Min)));
        texto_rico(&mut esquerda, &trechos, 12.0, cores.texto, cores.forte);
        let medido = esquerda.min_rect().height();
        if (medido - alto_texto).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(id, medido));
            ui.ctx().request_repaint();
        }
        ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, medido.max(ALTURA_PEQUENO))), Sense::hover());
    });
    // `role="alert"`, como na prancheta
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_role(egui::accesskit::Role::Alert));
    ir
}

/// Os dois botões da ficha numa linha centrada (a largura dela vem do quadro anterior: o primeiro quadro a mede).
/// Devolve (abrir, mostrar).
fn botoes_da_ficha(ui: &mut Ui, sal: Id, ligados: bool) -> (bool, bool) {
    let id = Id::new(("tela_texto::botoes", sal));
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
