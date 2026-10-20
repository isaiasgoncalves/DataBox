//! Início (`#/`, prancheta 4, `Inicio.dc.html:373-497`): a cabeça com o logo de 60, "DataBox" e a frase; os três
//! cartões de ação (**Abrir pasta**, o primário, Abrir arquivo e Novo banco); o caminho digitado, que pode ser uma
//! pasta, um banco ou um arquivo de dados, com a caixa "Abrir bancos só para leitura"; e os abertos recentemente,
//! com o ícone de cada tipo (a pasta, o cilindro, a folha do arquivo).
//!
//! A tela só **pede**: cada gesto vira uma [`Acao`] que a casca trata ([`Acao::EscolherEAbrirPasta`],
//! [`Acao::EscolherEAbrir`], [`Acao::NovoBanco`], [`Acao::AbrirCaminho`] e, no clique de um recente, pelo tipo
//! dele: [`Acao::AbrirPasta`], [`Acao::AbrirBanco`] ou [`Acao::AbrirArquivoDeDados`]). O erro de uma abertura
//! pedida daqui volta ao aviso do campo ([`TelaInicio::mostrar_erro`]). Os únicos trabalhos da própria tela são o
//! "Remover da lista" e o "Mostrar na pasta" de cada recente (tarefas do serviço).

use std::path::PathBuf;

use egui::{Align, CursorIcon, Layout, Rect, Response, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use estado::{Acao, Contexto, Promessa, Reacao, Rota};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_bytes_opt, formatar_data_opt};
use interface::grade::{largura_cabecalho, padding_coluna};
use interface::icones::{self, Icone, TipoArquivo};
use interface::marca;
use interface::pecas::acao::Acao as CartaoAcao;
use interface::pecas::arranjo::fluxo;
use interface::pecas::avisos::{aviso, aviso_texto, vazio};
use interface::pecas::botao::{Botao, TipoBotao};
use interface::pecas::campos::{Check, Entrada, enter_no_campo};
use interface::pecas::documento::RECUO_PAGINA;
use interface::pecas::rotulos::{TipoPill, pill};
use interface::pecas::tabela::PADDING_TABELA;
use interface::pecas::tabela::{ColunaTabela, LARGURA_MIN_ESTICA, LarguraColuna, Tabela, moldura_lista};
use interface::pecas::texto::{
    ALTURA_TIT, Trecho, galley, galley_cortado, galley_quebrado, h4, mostrar_paragrafo, texto, titulo_lista,
};
use interface::pecas::toast::toast_erro;
use interface::pintura;
use interface::tema::paleta;
use prefs::{PrefsInfo, Recente, TipoRecente};

/// A frase debaixo do nome (`p{font-size:13px;color:--t2;max-width:680px}`).
pub(crate) const FRASE: &str = "Abra uma pasta para trabalhar com vários arquivos, ou um arquivo só: bancos SQLite, \
                                planilhas CSV e Excel, JSON e texto. Nos bancos, tudo o que você salvar grava no \
                                arquivo na hora.";
/// O estado vazio dos recentes.
pub(crate) const VAZIO_RECENTES: &str = "Nenhum item aberto ainda. As pastas, os bancos e os arquivos que você abrir \
                                         aparecem aqui para voltar a eles com um clique.";
/// O caminho digitado está vazio.
pub(crate) const MSG_CAMINHO_VAZIO: &str = "Digite o caminho completo da pasta ou do arquivo.";
/// O nome do campo do caminho para o leitor de tela (`aria-label`) e a dica dele (`placeholder`).
pub(crate) const ROTULO_CAMINHO: &str = "Caminho da pasta ou do arquivo";
#[cfg(windows)]
const DICA_CAMINHO: &str = "C:\\pasta ou C:\\pasta\\arquivo.db";
#[cfg(not(windows))]
const DICA_CAMINHO: &str = "/home/você/pasta ou /home/você/pasta/arquivo.db";
/// A caixa da linha do caminho (vale para os bancos: pasta e arquivo de dados não têm modo só leitura).
pub(crate) const ROTULO_SO_LEITURA: &str = "Abrir bancos só para leitura";

/// O conteúdo (`max-width:960px;margin:0 auto;padding:36px 36px 28px`), por dentro da página da casca, que já tem
/// [`RECUO_PAGINA`] de cada lado: o que falta até os 36 e os 28.
const LARGURA_MAX_CONTEUDO: f32 = 960.0;
const RECUO_LADOS: f32 = 36.0;
const RECUO_TOPO: f32 = 36.0;
const RECUO_BASE: f32 = 28.0;
/// O `gap:26px` entre os blocos.
const ESPACO_BLOCOS: f32 = 26.0;
/// A cabeça: o logo de 60, o `gap:14px`, o nome em 24px / 600 e a frase 6px abaixo, até 680px.
const LADO_LOGO: f32 = 60.0;
const ESPACO_LOGO: f32 = 14.0;
const TAMANHO_NOME: f32 = 24.0;
const MARGEM_FRASE: f32 = 6.0;
const LARGURA_FRASE: f32 = 680.0;
const TAMANHO_FRASE: f32 = 13.0;
/// O `gap:12px` dos cartões de ação.
const ESPACO_CARTOES: f32 = 12.0;
/// A linha do caminho: o bloco do campo (`flex:1;min-width:300px;max-width:560px`), o `gap:20px` até a caixa, os
/// 6px entre o `.tit` e o campo e os 8px entre o campo e o "Abrir".
const LARGURA_MIN_CAMINHO: f32 = 300.0;
const LARGURA_MAX_CAMINHO: f32 = 560.0;
const ESPACO_LINHA_CAMINHO: f32 = 20.0;
const MARGEM_TIT: f32 = 6.0;
const ESPACO_CAMPO_BOTAO: f32 = 8.0;
const ALTURA_CAMPO: f32 = 28.0;
/// A caixa de marcar (16px) e o `gap:8px` até o texto (`.op`).
const LARGURA_CAIXA: f32 = 16.0 + 8.0;
/// O aviso de erro do campo (`.slot-aviso:not(:empty){margin-top:14px}`).
const MARGEM_AVISO: f32 = 14.0;
/// O título "Abertos recentemente" (`h2{font-size:13px}`, `margin-bottom:8px`).
const MARGEM_TITULO_RECENTES: f32 = 8.0;

/// Os `th` da tabela de recentes (a coluna das ações não tem texto).
const COLUNAS_RECENTES: [&str; 5] = ["Nome", "Pasta", "Tamanho", "Modificado em", ""];
/// A coluna da pasta (o `td.caminho-curto`).
const COLUNA_PASTA: usize = 1;
/// A coluna das ações (`th style="width:76px"`): os dois `.ib.pq` de 26px, sem espaço entre eles, encostados a 6px
/// da borda (`td{padding-right:6px}`, no lugar dos 12 das outras).
const COLUNA_ACOES: usize = 4;
const LARGURA_ACOES: f32 = 76.0;
const RECUO_DIREITA_ACOES: f32 = 6.0;
/// O ícone de 16px do nome (`.sk.s16`) e o `gap:8px` do `.link`.
const LADO_ICONE: f32 = 16.0;
const ESPACO_ICONE: f32 = 8.0;
/// O texto das células (`.lista-t td`: 12px).
const TAMANHO_CELULA: f32 = 12.0;

/// O nome de um recente como aparece: o da raiz de uma unidade ou de um compartilhamento (`C:\`, `\\srv\dados`) é
/// vazio, e fica o caminho inteiro.
pub(crate) fn rotulo_do_recente(r: &Recente) -> &str {
    if r.nome.is_empty() { &r.caminho } else { &r.nome }
}

/// O ícone de um recente pelo tipo: a pasta sólida, o cilindro do banco, a folha do arquivo de dados pela extensão
/// (CSV, XLSX, JSON, TXT; o que não tiver extensão conhecida fica com a folha de texto).
pub(crate) fn icone_do_recente(r: &Recente) -> Icone {
    match r.tipo {
        TipoRecente::Pasta => Icone::PastaSolida,
        TipoRecente::Banco => Icone::Banco,
        TipoRecente::Arquivo => Icone::Folha(folha_do_arquivo(&r.caminho)),
    }
}

/// A folha de um arquivo de dados pela extensão do caminho.
fn folha_do_arquivo(caminho: &str) -> TipoArquivo {
    use comum::arquivos::TipoDeArquivo;
    let ext = std::path::Path::new(caminho).extension().and_then(|e| e.to_str()).unwrap_or("");
    match TipoDeArquivo::da_extensao(ext) {
        Some(TipoDeArquivo::Csv) => TipoArquivo::Csv,
        Some(TipoDeArquivo::Xlsx) => TipoArquivo::Xlsx,
        Some(TipoDeArquivo::Json) => TipoArquivo::Json,
        Some(TipoDeArquivo::Texto) | None => TipoArquivo::Txt,
    }
}

/// O que o clique no nome de um recente pede, pelo tipo dele (o `somente_leitura` vale só para o banco).
pub(crate) fn acao_do_recente(r: &Recente, somente_leitura: bool) -> Acao {
    let caminho = PathBuf::from(&r.caminho);
    match r.tipo {
        TipoRecente::Pasta => Acao::AbrirPasta { caminho },
        TipoRecente::Banco => Acao::AbrirBanco { caminho, somente_leitura },
        TipoRecente::Arquivo => Acao::AbrirArquivoDeDados { caminho },
    }
}

/// A coluna Pasta para a largura disponível: quando as pastas inteiras cabem (`natural`, a mais larga), ela é como
/// as outras e a sobra se reparte entre elas (o `table{width:100%}` da prancheta); senão, fica com a sobra
/// (`estica`) e encurta com "…"; numa janela estreita, com a sobra de verdade, mesmo abaixo do mínimo de 60px da
/// peça (o `td.caminho-curto{max-width:0}`, que não empurra a tabela: as ações da linha continuam dentro da
/// moldura). Nunca fica mais estreita que o próprio padding. `medidas` são as larguras do conteúdo de cada coluna
/// no quadro anterior.
fn coluna_pasta(ui: &Ui, medidas: &[f32; 5], disponivel: f32, natural: f32) -> ColunaTabela<'static> {
    let n = COLUNAS_RECENTES.len();
    let outras: f32 = (0..n)
        .filter(|&c| c != COLUNA_PASTA)
        .map(|c| {
            if c == COLUNA_ACOES {
                return LARGURA_ACOES;
            }
            let (e, d) = padding_coluna(c, n);
            let titulo = COLUNAS_RECENTES[c];
            let th = if titulo.is_empty() { 0.0 } else { largura_cabecalho(ui.painter(), titulo, None) };
            (th.max(medidas[c]) + e + d).ceil()
        })
        .sum();
    let (e, d) = padding_coluna(COLUNA_PASTA, n);
    let sobra = (disponivel - outras).floor();
    let titulo = COLUNAS_RECENTES[COLUNA_PASTA];
    if sobra >= (natural + e + d).ceil() {
        return ColunaTabela::new(titulo);
    }
    if sobra >= LARGURA_MIN_ESTICA + e + d {
        return ColunaTabela::new(titulo).estica();
    }
    // mais estreita que o próprio `th` ("Pasta"), a coluna fica sem o título (a peça cortaria o texto ao meio;
    // as células mostram só o "…", com o caminho na dica)
    let th = largura_cabecalho(ui.painter(), titulo, None);
    let titulo = if sobra >= th + e + d { titulo } else { "" };
    ColunaTabela::new(titulo).fixa(sobra.max(e + d))
}

/// Uma ação da linha de um recente: o botão de ícone pequeno (`.ib.pq`), com o verbo na dica e, no rótulo, o verbo
/// com o nome ("Remover vendas.db da lista": com vários recentes, só o verbo não diria qual).
fn acao_recente<'a>(icone: Icone, rotulo: &'a str, dica: &'a str) -> Botao<'a> {
    Botao::icone(icone).pequeno().rotulo(rotulo).dica(dica)
}

/// O nome de um recente: o `a.link` com o ícone de 16px e `gap:8px` (12px / 600 em `--acct`, `--t` no hover), que é
/// o botão de abrir; o que sumiu é o `span` de 600 que não abre (nem mãozinha), com o ícone apagado (`.sk.inativo`:
/// metade da opacidade e sem cor; a linha `tr.inativo` já leva o texto ao `--t3`).
fn nome_do_recente(ui: &mut Ui, icone: Icone, nome: &str, existe: bool) -> Response {
    let p = paleta(ui);
    let e = &p.estudio;
    let id = ui.next_auto_id();
    let hover = existe && ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let cor = if !existe || hover { e.t } else { e.acct };
    // o nome inteiro (a coluna mede o conteúdo; quem encurta é a coluna Pasta)
    let g = galley(ui, nome, fontes::fonte(TAMANHO_CELULA, Peso::Medio), cor);
    let tamanho = vec2(LADO_ICONE + ESPACO_ICONE + g.size().x, g.size().y.max(LADO_ICONE));
    let (rect, resp) = ui.allocate_exact_size(tamanho, if existe { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let lugar_icone =
            Rect::from_min_size(pos2(rect.left(), rect.center().y - LADO_ICONE / 2.0), vec2(LADO_ICONE, LADO_ICONE));
        if existe {
            icones::pintar(ui.painter(), lugar_icone, icone, e.t3);
        } else {
            let mut painter = ui.painter().clone();
            painter.multiply_opacity((0.5 / ui.opacity().max(0.01)).min(1.0));
            icones::pintar_apagado(&painter, lugar_icone, icone, 1.0);
        }
        let y = rect.center().y - g.size().y / 2.0;
        ui.painter().galley(pos2(lugar_icone.right() + ESPACO_ICONE, y), g, cor);
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(ui.painter(), p, rect.expand(2.0), 4.0);
        }
    }
    if existe && resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let tipo = if existe { WidgetType::Button } else { WidgetType::Label };
    resp.widget_info(|| WidgetInfo::labeled(tipo, ui.is_enabled(), nome));
    resp
}

/// O `td.mono` da pasta dos recentes: a pasta numa linha (12px / 350 em `--t3`), encurtada com "…", e a dica com o
/// caminho completo, encurtada ou não.
fn pasta_curta(ui: &mut Ui, pasta: &str, caminho: &str, cortar: bool) -> Response {
    let cor = paleta(ui).estudio.t3;
    let fonte = fontes::fonte(TAMANHO_CELULA, Peso::Leve);
    // a coluna automática mede o conteúdo: a pasta vai inteira (ela só é automática quando todas cabem)
    let g = if cortar {
        galley_cortado(ui, pasta, fonte, cor, ui.available_width())
    } else {
        galley(ui, pasta, fonte, cor)
    };
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, cor);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, pasta));
    resp.on_hover_text(caminho)
}

/// Os textos da tabela de recentes, refeitos só quando as preferências mudam.
struct TextosRecentes {
    versao: u64,
    tamanhos: Vec<String>,
    datas: Vec<String>,
    /// Os rótulos das ações de cada linha: "Remover vendas.db da lista" e "Mostrar vendas.db na pasta".
    remover: Vec<String>,
    mostrar: Vec<String>,
}

/// A tela de Início.
pub(crate) struct TelaInicio {
    caminho: String,
    so_leitura: bool,
    /// O aviso de erro do campo do caminho (`.notice.perigo`).
    erro: Option<String>,
    focar_caminho: bool,
    remocoes: Vec<Promessa<PrefsInfo>>,
    revelacoes: Vec<Promessa<()>>,
    textos: Option<TextosRecentes>,
    /// A largura do conteúdo de cada coluna dos recentes no quadro anterior (para a coluna Pasta).
    medidas_recentes: [f32; 5],
}

/// Tira os espaços e as aspas em volta do caminho colado do Explorer (o `"C:\x.db"` do "Copiar como caminho").
pub(crate) fn limpar_caminho_digitado(texto: &str) -> String {
    let t = texto.trim();
    match t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(dentro) => dentro.to_string(),
        None => t.to_string(),
    }
}

impl TelaInicio {
    pub fn nova(erro: Option<String>) -> TelaInicio {
        TelaInicio {
            caminho: String::new(),
            so_leitura: false,
            erro,
            focar_caminho: false,
            remocoes: Vec::new(),
            revelacoes: Vec::new(),
            textos: None,
            medidas_recentes: [0.0; 5],
        }
    }

    /// Mostra o erro de uma abertura pedida por esta tela.
    pub fn mostrar_erro(&mut self, mensagem: String) {
        self.erro = Some(mensagem);
    }

    /// Há tarefas desta tela rodando.
    pub fn ocupada(&self) -> bool {
        !self.remocoes.is_empty() || !self.revelacoes.is_empty()
    }

    fn receber(&mut self, ui: &Ui, ctx: &mut Contexto) {
        let mut i = 0;
        while i < self.remocoes.len() {
            match self.remocoes[i].tomar() {
                Some(r) => {
                    self.remocoes.remove(i);
                    match r {
                        // a casca relê as preferências (os recentes do menu Arquivo também)
                        Ok(_) => ctx.emitir(Acao::RecarregarPrefs),
                        Err(e) => {
                            if ctx.reagir(&e) == Reacao::Mostrar {
                                toast_erro(ui.ctx(), e.mensagem);
                            }
                        }
                    }
                }
                None => i += 1,
            }
        }
        let mut i = 0;
        while i < self.revelacoes.len() {
            match self.revelacoes[i].tomar() {
                Some(r) => {
                    self.revelacoes.remove(i);
                    if let Err(e) = r
                        && ctx.reagir(&e) == Reacao::Mostrar
                    {
                        toast_erro(ui.ctx(), e.mensagem);
                    }
                }
                None => i += 1,
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.receber(ui, ctx);
        // a coluna de até 960px (com o padding) no meio do documento; a página da casca já recua 20px de cada lado
        let coluna = ui.available_width();
        let documento = coluna + 2.0 * RECUO_PAGINA;
        let largura = (documento.min(LARGURA_MAX_CONTEUDO) - 2.0 * RECUO_LADOS).max(0.0);
        let esquerda = ui.cursor().left() + ((coluna - largura) / 2.0).max(0.0);
        let topo = ui.cursor().top() + (RECUO_TOPO - RECUO_PAGINA).max(0.0);
        let area = Rect::from_min_size(pos2(esquerda, topo), vec2(largura, f32::INFINITY));
        ui.scope_builder(UiBuilder::new().max_rect(area).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            cabeca(ui);
            ui.add_space(ESPACO_BLOCOS);
            if let Some(b) = &ctx.estado.banco {
                let r = aviso(
                    ui,
                    &[
                        Trecho::Forte(&b.nome),
                        Trecho::Normal(" está aberto. Abrir outro fecha este. "),
                        Trecho::Link("Ir para o banco"),
                    ],
                    false,
                );
                if r.link.is_some() {
                    ctx.navegar(Rota::Banco);
                }
                ui.add_space(ESPACO_BLOCOS);
            }
            self.cartoes(ui, ctx);
            ui.add_space(ESPACO_BLOCOS);
            self.linha_do_caminho(ui, ctx);
            ui.add_space(ESPACO_BLOCOS);
            self.recentes(ui, ctx);
        });
        ui.add_space((RECUO_BASE - RECUO_PAGINA).max(0.0));
    }

    /// Os três cartões de ação (`.acao`, `gap:12px`, quebram linha numa janela estreita).
    fn cartoes(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let habilitados = !ctx.trocando;
        let mut pedido = None;
        fluxo(ui, ESPACO_CARTOES, |ui| {
            let pasta = CartaoAcao::new("Abrir pasta", Icone::Pasta)
                .atalho("Ctrl+Shift+O")
                .primaria(true)
                .habilitado(habilitados);
            if ui.add(pasta).clicked() {
                pedido = Some(Acao::EscolherEAbrirPasta);
            }
            let arquivo = CartaoAcao::new("Abrir arquivo", Icone::Arquivo).atalho("Ctrl+O").habilitado(habilitados);
            if ui.add(arquivo).clicked() {
                pedido = Some(Acao::EscolherEAbrir { somente_leitura: self.so_leitura });
            }
            if ui.add(CartaoAcao::new("Novo banco", Icone::NovoBanco).habilitado(habilitados)).clicked() {
                pedido = Some(Acao::NovoBanco);
            }
        });
        // a casca sabe que o pedido veio do Início (a marca da ação): o erro da abertura volta ao aviso do campo
        if let Some(p) = pedido {
            self.erro = None;
            ctx.emitir(p);
        }
    }

    /// "Ou digite o caminho": o `.tit`, o campo e o "Abrir" (até 560px) e, ao lado, a caixa "Abrir bancos só para
    /// leitura", embaixo da linha do campo (`align-items:flex-end`); numa janela estreita, a caixa desce. O erro do
    /// campo fica logo abaixo.
    fn linha_do_caminho(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let p = paleta(ui);
        let e = &p.estudio;
        let habilitados = !ctx.trocando;
        let largura = ui.available_width();
        let texto_caixa = galley(ui, ROTULO_SO_LEITURA, fontes::fonte(12.0, Peso::Normal), e.t2);
        let largura_caixa = (LARGURA_CAIXA + texto_caixa.size().x).ceil();
        let cabe_ao_lado = largura - ESPACO_LINHA_CAMINHO - largura_caixa >= LARGURA_MIN_CAMINHO;
        let largura_campo = if cabe_ao_lado {
            (largura - ESPACO_LINHA_CAMINHO - largura_caixa).min(LARGURA_MAX_CAMINHO)
        } else {
            largura.min(LARGURA_MAX_CAMINHO)
        };
        let topo = ui.cursor().min;
        let altura_bloco = ALTURA_TIT + MARGEM_TIT + ALTURA_CAMPO;
        let bloco = Rect::from_min_size(topo, vec2(largura_campo, altura_bloco));
        let mut abrir_digitado = false;
        ui.scope_builder(UiBuilder::new().max_rect(bloco).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            h4(ui, "Ou digite o caminho");
            ui.add_space(MARGEM_TIT);
            let linha = Rect::from_min_size(ui.cursor().min, vec2(largura_campo, ALTURA_CAMPO));
            ui.scope_builder(UiBuilder::new().max_rect(linha).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing = vec2(ESPACO_CAMPO_BOTAO, 0.0);
                let largura_botao =
                    (galley(ui, "Abrir", fontes::fonte(12.0, Peso::Medio), e.t).size().x + 30.0 + 2.0).ceil();
                let saida = Entrada::new(&mut self.caminho)
                    .dica(DICA_CAMINHO)
                    .rotulo(ROTULO_CAMINHO)
                    .monoespacada(true)
                    .largura((largura_campo - ESPACO_CAMPO_BOTAO - largura_botao).max(80.0))
                    .mostrar(ui);
                let campo = saida.response.response;
                if self.focar_caminho {
                    campo.request_focus();
                    self.focar_caminho = false;
                }
                if habilitados && enter_no_campo(ui, &campo) {
                    abrir_digitado = true;
                }
                if ui.add(Botao::new("Abrir").habilitado(habilitados)).clicked() {
                    abrir_digitado = true;
                }
            });
        });
        // a caixa: à direita do bloco, na altura do campo; ou embaixo, depois do `gap:20px`
        let lugar_caixa = if cabe_ao_lado {
            Rect::from_min_size(
                pos2(bloco.right() + ESPACO_LINHA_CAMINHO, bloco.bottom() - ALTURA_CAMPO),
                vec2(largura_caixa, ALTURA_CAMPO),
            )
        } else {
            Rect::from_min_size(
                pos2(bloco.left(), bloco.bottom() + ESPACO_LINHA_CAMINHO),
                vec2(largura_caixa.min(largura), ALTURA_CAMPO),
            )
        };
        ui.scope_builder(UiBuilder::new().max_rect(lugar_caixa).layout(Layout::left_to_right(Align::Center)), |ui| {
            let _ = Check::new(&mut self.so_leitura, ROTULO_SO_LEITURA).cor_texto(e.t2).mostrar(ui);
        });
        let fim = bloco.bottom().max(lugar_caixa.bottom());
        ui.allocate_rect(Rect::from_min_max(topo, pos2(topo.x + largura, fim)), Sense::hover());
        if let Some(erro) = &self.erro {
            ui.add_space(MARGEM_AVISO);
            aviso_texto(ui, erro, true);
        }
        if abrir_digitado {
            let texto = limpar_caminho_digitado(&self.caminho);
            if texto.is_empty() {
                self.erro = Some(MSG_CAMINHO_VAZIO.to_string());
                self.focar_caminho = true;
            } else {
                // a casca classifica (pasta, banco ou arquivo de dados) e o erro volta para cá
                self.erro = None;
                ctx.emitir(Acao::AbrirCaminho { caminho: PathBuf::from(texto), somente_leitura: self.so_leitura });
            }
        }
    }

    fn textos(&mut self, estado: &estado::Estado, versao: u64) -> &TextosRecentes {
        let guardados = self.textos.take().filter(|t| t.versao == versao);
        let textos = guardados.unwrap_or_else(|| {
            let recentes = &estado.prefs.recentes;
            TextosRecentes {
                versao,
                tamanhos: recentes.iter().map(|r| formatar_bytes_opt(r.tamanho)).collect(),
                datas: recentes
                    .iter()
                    .map(|r| if r.existe { formatar_data_opt(r.modificado_em.as_deref()) } else { "—".to_string() })
                    .collect(),
                remover: recentes.iter().map(|r| format!("Remover {} da lista", rotulo_do_recente(r))).collect(),
                mostrar: recentes.iter().map(|r| format!("Mostrar {} na pasta", rotulo_do_recente(r))).collect(),
            }
        });
        &*self.textos.insert(textos)
    }

    /// "Abertos recentemente": o título de 13px e a tabela numa moldura (`border:1px solid --b2; radius:10px`).
    fn recentes(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let estado = ctx.estado;
        let recentes = &estado.prefs.recentes;
        titulo_lista(ui, "Abertos recentemente");
        ui.add_space(MARGEM_TITULO_RECENTES);
        if recentes.is_empty() {
            moldura_lista(ui, |ui| {
                vazio(ui, VAZIO_RECENTES, false);
            });
            return;
        }
        let so_leitura = self.so_leitura;
        let habilitados = !ctx.trocando;
        let medidas = self.medidas_recentes;
        let mut novas_medidas = [0.0_f32; 5];
        let textos = self.textos(estado, ctx.versao);
        let mut abrir: Option<Acao> = None;
        let mut remover: Option<PathBuf> = None;
        let mut revelar: Option<PathBuf> = None;
        moldura_lista(ui, |ui| {
            let cor = paleta(ui).estudio.t3;
            let natural_pasta = recentes
                .iter()
                .map(|r| galley(ui, &r.pasta, fontes::fonte(TAMANHO_CELULA, Peso::Leve), cor).size().x)
                .fold(0.0_f32, f32::max);
            let colunas = [
                ColunaTabela::new(COLUNAS_RECENTES[0]),
                coluna_pasta(ui, &medidas, ui.available_width(), natural_pasta),
                ColunaTabela::new(COLUNAS_RECENTES[2]).direita(),
                ColunaTabela::new(COLUNAS_RECENTES[3]),
                ColunaTabela::new(COLUNAS_RECENTES[COLUNA_ACOES]).direita().fixa(LARGURA_ACOES),
            ];
            let cortar_pasta = colunas[COLUNA_PASTA].largura != LarguraColuna::Auto;
            Tabela::new("recentes", &colunas, recentes.len())
                .inativas(|l| recentes.get(l).is_some_and(|r| !r.existe))
                .mostrar(ui, |ui, l, coluna| {
                    let Some(r) = recentes.get(l) else { return };
                    let e = &paleta(ui).estudio;
                    match coluna {
                        0 => {
                            let resp = nome_do_recente(ui, icone_do_recente(r), rotulo_do_recente(r), r.existe)
                                .on_hover_text(&r.caminho);
                            if r.existe && resp.clicked() && habilitados {
                                abrir = Some(acao_do_recente(r, so_leitura));
                            }
                        }
                        1 => {
                            pasta_curta(ui, &r.pasta, &r.caminho, cortar_pasta);
                        }
                        2 => {
                            if !r.existe {
                                pill(ui, "não encontrado", TipoPill::Mute);
                            } else if r.tipo == TipoRecente::Pasta {
                                texto(ui, "pasta", TAMANHO_CELULA, e.t3);
                            } else {
                                let tamanho = textos.tamanhos.get(l).map_or("—", String::as_str);
                                texto(ui, tamanho, TAMANHO_CELULA, e.t);
                            }
                        }
                        3 => {
                            texto(ui, textos.datas.get(l).map_or("—", String::as_str), TAMANHO_CELULA, e.t);
                        }
                        _ => {
                            // da direita para a esquerda: "Remover da lista" fica na ponta, a 6px da borda
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add_space(-(PADDING_TABELA - RECUO_DIREITA_ACOES));
                            let rotulo_remover = textos.remover.get(l).map_or("", String::as_str);
                            let b = acao_recente(Icone::Fechar, rotulo_remover, "Remover da lista");
                            if ui.add(b.tipo(TipoBotao::GhostPerigo)).clicked() {
                                remover = Some(PathBuf::from(&r.caminho));
                            }
                            let rotulo_mostrar = textos.mostrar.get(l).map_or("", String::as_str);
                            let b = acao_recente(Icone::MostrarNaPasta, rotulo_mostrar, "Mostrar na pasta");
                            if r.existe && ui.add(b).clicked() {
                                revelar = Some(PathBuf::from(&r.caminho));
                            }
                        }
                    }
                    let usado = ui.min_rect();
                    if coluna != COLUNA_ACOES
                        && usado.is_positive()
                        && let Some(m) = novas_medidas.get_mut(coluna)
                    {
                        *m = m.max(usado.width());
                    }
                });
        });
        if novas_medidas.iter().zip(&medidas).any(|(a, b)| (a - b).abs() > 0.5) {
            // a coluna Pasta depende das outras: refaz a passada sem mostrar a errada
            self.medidas_recentes = novas_medidas;
            ui.ctx().request_discard("larguras dos recentes");
        }
        if let Some(acao) = abrir {
            self.erro = None;
            ctx.emitir(acao);
        }
        if let Some(caminho) = remover {
            self.remocoes.push(ctx.tarefa(ui.ctx(), move |s, _| s.remover_recente(&caminho)));
        }
        if let Some(caminho) = revelar {
            self.revelacoes.push(ctx.tarefa(ui.ctx(), move |s, _| s.revelar(&caminho)));
        }
    }
}

/// A cabeça: o logo de 60 e "DataBox" (24px / 600) lado a lado, centrados na altura (`gap:14px`), e a frase de
/// 13px em `--t2` (até 680px, `line-height:1.4`), 6px abaixo.
fn cabeca(ui: &mut Ui) {
    let e = &paleta(ui).estudio;
    let largura = ui.available_width();
    let linha = Rect::from_min_size(ui.cursor().min, vec2(largura, LADO_LOGO));
    ui.scope_builder(UiBuilder::new().max_rect(linha).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing = vec2(ESPACO_LOGO, 0.0);
        // o logo é decoração (`aria-hidden`): sem nome para o leitor de tela
        let (lugar, _) = ui.allocate_exact_size(vec2(LADO_LOGO, LADO_LOGO), Sense::hover());
        if ui.is_rect_visible(lugar) {
            marca::pintar_logo(ui, lugar);
        }
        let g = galley(ui, comum::app::NOME, fontes::fonte(TAMANHO_NOME, Peso::Medio), e.t);
        let (lugar, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
        if ui.is_rect_visible(lugar) {
            ui.painter().galley(lugar.min, g, e.t);
        }
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, comum::app::NOME));
    });
    ui.add_space(MARGEM_FRASE);
    let frase =
        galley_quebrado(ui, FRASE, fontes::fonte(TAMANHO_FRASE, Peso::Normal), e.t2, largura.min(LARGURA_FRASE), 1.4);
    mostrar_paragrafo(ui, &frase, FRASE);
}

/// O Início como documento da casca: não oferece nenhum verbo nem tem Propriedades.
impl estado::Documento for TelaInicio {
    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ui(ui, ctx);
    }

    fn ocupada(&self) -> bool {
        TelaInicio::ocupada(self)
    }
}

#[cfg(test)]
mod testes;
