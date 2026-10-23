//! A janela "Sobre o DataBox" (prancheta 13) e as duas que saem dela, "Licença" e "Licenças de terceiros".
//!
//! **Sobre** (440 px): o logo completo de 20 no cabeçalho; no topo o logo de 96, "DataBox" (20 / 600), "Versão 0.2.4" e a
//! descrição em 11 px; a ficha de 92 / 1fr, com fios em cima e embaixo e sem Consolas: Compilação (o
//! `DATABOX_COMPILACAO` do build.rs, "15/10/2026 · fb34ec7"), Renderizador ("OpenGL (glow)" ou "wgpu", o que o
//! [`crate::Editor::com_renderizador`] recebeu), SQLite ("3.53.2, embutido"), Sistema ("Windows · x64", do
//! `std::env::consts`, sem FFI), Licença (o link "MIT", que abre a janela), Preferências e Arquivo de log (links que
//! abrem a pasta e o arquivo pelo `servico.abrir_arquivo`, com o caminho de verdade na dica). Rodapé: "Licenças de
//! terceiros", "Copiar informações" (copia a ficha em texto e dá o toast "Informações copiadas.") e "Fechar".
//!
//! **Licença**: o texto do LICENSE embutido (`include_str!`). **Licenças de terceiros**: as bibliotecas de fora que
//! entram no programa, com a versão e a licença de cada uma; a lista é feita pelo build.rs a partir do Cargo.lock e
//! dos pacotes baixados (as diretas do projeto primeiro, depois as que vêm por elas). O Cargo.lock não separa por
//! sistema: a lista inclui também as dependências que só valem em outros sistemas, e a janela diz isso.

use std::path::{Path, PathBuf};

use egui::{Margin, Rect, Response, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use estado::{Contexto, Estado, Promessa, Reacao};
use interface::fontes::{self, Peso};
use interface::icones::Icone;
use interface::marca::pintar_logo;
use interface::pecas::botao::{Botao, LADO_CABECALHO};
use interface::pecas::janela::Janela;
use interface::pecas::texto::{galley, galley_cortado, galley_quebrado, mostrar_paragrafo};
use interface::pecas::texto_pre::TextoPre;
use interface::pecas::toast::{toast_concluido, toast_erro};
use interface::pintura;
use interface::tema::paleta;

use crate::linha_comando::Renderizador;

/// O texto da licença do projeto.
pub(crate) const LICENCA: &str = include_str!("../../../../LICENSE");
/// "15/10/2026 · fb34ec7" (ou "desconhecida"), do build.rs.
const COMPILACAO: &str = env!("DATABOX_COMPILACAO");
/// As bibliotecas de terceiros, uma por linha: "nome\tversão\tlicença\tdireta", feitas pelo build.rs.
const TERCEIROS: &str = include_str!(concat!(env!("OUT_DIR"), "/terceiros.txt"));

const DESCRICAO: &str = "Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.";
const AUTORIA: &str = "Desenvolvido por Isaías Gonçalves. Feito em Rust, com egui e SQLite.";
/// Os caminhos como a ficha os escreve no Windows (as variáveis, sem o nome da pessoa); a dica leva o caminho de
/// verdade. Fora do Windows não existe `%APPDATA%`: a ficha mostra o caminho real. O "Copiar informações" usa sempre
/// estes rótulos, que não expõem o nome do usuário.
const ROTULO_PREFERENCIAS: &str = r"%APPDATA%\DataBox";
const ROTULO_LOG: &str = r"%LOCALAPPDATA%\DataBox\databox.log";

/// `.sobre{width:440px}`, o logo de 96 e os 18px entre o logo e o texto, a coluna dos rótulos e o resto.
const LARGURA: f32 = 440.0;
const LADO_LOGO: f32 = 96.0;
const GAP_TOPO: f32 = 18.0;
const LARGURA_ROTULO: f32 = 92.0;
const GAP_COLUNAS: f32 = 14.0;
const GAP_LINHAS: f32 = 7.0;
const ALTURA_LINHA: f32 = 17.0;
/// `.sb-corpo{padding:22px 24px 20px}` dentro do corpo da janela (18px em cima, 20px dos lados).
const RECUO_LATERAL: i8 = 4;
const RECUO_TOPO: i8 = 4;

/// Para onde vai "abrir" nos testes, no lugar do Explorador de Arquivos.
pub(crate) type GanchoAbrir = Box<dyn FnMut(&Path)>;

/// O que a janela mostra além do estado: de onde vêm as linhas da ficha.
struct Ficha {
    compilacao: &'static str,
    renderizador: &'static str,
    sqlite: String,
    sistema: String,
    pasta_preferencias: PathBuf,
    log: PathBuf,
}

impl Ficha {
    fn nova(estado: &Estado, renderizador: Renderizador, arquivo_prefs: &Path) -> Ficha {
        Ficha {
            compilacao: COMPILACAO,
            renderizador: texto_do_renderizador(renderizador),
            sqlite: format!("{}, embutido", estado.app.sqlite_versao),
            sistema: sistema(std::env::consts::OS, std::env::consts::ARCH),
            pasta_preferencias: arquivo_prefs.parent().map(Path::to_path_buf).unwrap_or_default(),
            log: servico::arquivo_log(),
        }
    }
}

impl Ficha {
    /// O que a ficha escreve na linha "Preferências": o rótulo do Windows, ou o caminho real nos outros sistemas.
    fn rotulo_preferencias(&self) -> String {
        if cfg!(windows) { ROTULO_PREFERENCIAS.to_string() } else { self.pasta_preferencias.display().to_string() }
    }

    /// O que a ficha escreve na linha "Arquivo de log".
    fn rotulo_log(&self) -> String {
        if cfg!(windows) { ROTULO_LOG.to_string() } else { self.log.display().to_string() }
    }
}

/// "OpenGL (glow)" ou "wgpu".
pub(crate) fn texto_do_renderizador(r: Renderizador) -> &'static str {
    match r {
        Renderizador::Glow => "OpenGL (glow)",
        Renderizador::Wgpu => "wgpu",
    }
}

/// "Windows · x64": o sistema e a arquitetura de `std::env::consts`, sem consultar a versão do Windows (isso pediria
/// o registro ou o `RtlGetVersion`, FFI que o editor não pode ter).
pub(crate) fn sistema(os: &str, arch: &str) -> String {
    let nome = match os {
        "windows" => "Windows",
        "linux" => "Linux",
        "macos" => "macOS",
        outro => outro,
    };
    let arquitetura = match arch {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "ARM64",
        outra => outra,
    };
    format!("{nome} · {arquitetura}")
}

/// A ficha em texto para colar num chamado ("Copiar informações").
fn informacoes(versao: &str, f: &Ficha) -> String {
    format!(
        "{} {versao}\nCompilação: {}\nRenderizador: {}\nSQLite: {}\nSistema: {}\nLicença: MIT\nPreferências: \
         {ROTULO_PREFERENCIAS}\nArquivo de log: {ROTULO_LOG}",
        comum::app::NOME,
        f.compilacao,
        f.renderizador,
        f.sqlite,
        f.sistema,
    )
}

/// Uma biblioteca da janela de terceiros.
struct Biblioteca {
    nome: &'static str,
    versao: &'static str,
    licenca: &'static str,
    direta: bool,
}

fn bibliotecas() -> Vec<Biblioteca> {
    TERCEIROS
        .lines()
        .filter_map(|l| {
            let mut c = l.split('\t');
            Some(Biblioteca { nome: c.next()?, versao: c.next()?, licenca: c.next()?, direta: c.next()? == "1" })
        })
        .collect()
}

/// O estado das janelas do Sobre entre os quadros.
#[derive(Default)]
pub(crate) struct Sobre {
    licenca: bool,
    terceiros: bool,
    /// "Abrir a pasta" e "Abrir o arquivo" em andamento (o Explorador de Arquivos abre numa tarefa).
    abrindo: Vec<Promessa<()>>,
}

/// O que a pessoa fez no corpo neste quadro.
#[derive(Default)]
struct Pedidos {
    licenca: bool,
    pasta: bool,
    log: bool,
    /// O Copiar caminho de uma das duas linhas (`.cam .cp`): o caminho inteiro.
    copiar: Option<PathBuf>,
}

impl Sobre {
    /// Desenha o Sobre e, por cima dele, as janelas que ele abriu. Devolve false quando o Sobre fechou (✕, Esc, clique
    /// no fundo ou "Fechar"). `gancho` troca o Explorador de Arquivos nos testes.
    pub fn mostrar(
        &mut self,
        egui: &egui::Context,
        ctx: &mut Contexto,
        renderizador: Renderizador,
        gancho: &mut Option<GanchoAbrir>,
    ) -> bool {
        self.tomar_resultados(egui, ctx);
        let ficha = Ficha::nova(ctx.estado, renderizador, ctx.servico.prefs().arquivo());
        let titulo = format!("Sobre o {}", comum::app::NOME);
        let (mut copiar, mut fechar, mut terceiros) = (false, false, false);
        let mut pedidos = Pedidos::default();
        let versao = ctx.estado.app.versao;
        let r = Janela::new("sobre", &titulo)
            .logo(true)
            .largura(LARGURA)
            .rodape_esquerda(|ui| {
                terceiros = ui.add(Botao::ghost("Licenças de terceiros")).clicked();
            })
            .rodape_direita(|ui| {
                copiar = ui.add(Botao::new("Copiar informações")).clicked();
                fechar = ui.add(Botao::primario("Fechar")).clicked();
            })
            .mostrar(egui, |ui| corpo(ui, versao, &ficha, &mut pedidos));

        if copiar {
            egui.copy_text(informacoes(versao, &ficha));
            toast_concluido(egui, "Informações copiadas.");
        }
        self.licenca |= pedidos.licenca;
        self.terceiros |= terceiros;
        if let Some(c) = pedidos.copiar.take() {
            egui.copy_text(c.display().to_string());
            toast_concluido(egui, estado::MSG_CAMINHO_COPIADO);
        }
        if pedidos.pasta {
            self.abrir(egui, ctx, gancho, ficha.pasta_preferencias.clone());
        }
        if pedidos.log {
            self.abrir(egui, ctx, gancho, ficha.log.clone());
        }
        // as janelas de cima: o Esc e o clique no fundo fecham só a de cima
        if self.licenca && !janela_licenca(egui) {
            self.licenca = false;
        }
        if self.terceiros && !janela_terceiros(egui) {
            self.terceiros = false;
        }
        !(r.pediu_fechar || fechar)
    }

    fn abrir(&mut self, egui: &egui::Context, ctx: &Contexto, gancho: &mut Option<GanchoAbrir>, caminho: PathBuf) {
        match gancho.as_mut() {
            Some(g) => g(&caminho),
            None => self.abrindo.push(ctx.tarefa(egui, move |servico, _| servico.abrir_arquivo(&caminho))),
        }
    }

    /// Toma o que as tarefas de abrir devolveram: um erro vira toast.
    fn tomar_resultados(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        let mut restantes = Vec::new();
        for mut p in std::mem::take(&mut self.abrindo) {
            match p.tomar() {
                None => restantes.push(p),
                Some(Ok(())) => {}
                Some(Err(e)) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        toast_erro(egui, e.mensagem);
                    }
                }
            }
        }
        self.abrindo = restantes;
    }
}

// ------------------------------------------------------------------------------------------------ o corpo

/// Um texto de uma linha no peso pedido, com o nome acessível dele.
fn texto_peso(ui: &mut Ui, texto: &str, tamanho: f32, peso: Peso, cor: egui::Color32) -> Response {
    let g = galley(ui, texto, fontes::fonte(tamanho, peso), cor);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, cor);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, texto));
    resp
}

/// O link da ficha (`.sb-det a`): `--acct`, sem sublinhado até o hover. Com a dica de `title`.
fn ligacao(ui: &mut Ui, texto: &str, peso: Peso, dica: &str) -> Response {
    ligacao_cortada(ui, texto, peso, dica, f32::INFINITY)
}

/// O [`ligacao`] cortado com "…" em `largura` (o `.cam>a` da revisão de UI de 19/10/2026).
fn ligacao_cortada(ui: &mut Ui, texto: &str, peso: Peso, dica: &str, largura: f32) -> Response {
    let p = paleta(ui);
    let cor = p.estudio.acct;
    let g = galley_cortado(ui, texto, fontes::fonte(12.0, peso), cor, largura);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::click());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, cor);
        if resp.hovered() {
            ui.painter().hline(rect.x_range(), rect.bottom() - 2.0, Stroke::new(1.0, cor));
        }
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(ui.painter(), p, rect, 4.0);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, texto));
    resp.on_hover_text(dica)
}

fn corpo(ui: &mut Ui, versao: &str, ficha: &Ficha, pedidos: &mut Pedidos) {
    egui::Frame::NONE
        .inner_margin(Margin { left: RECUO_LATERAL, right: RECUO_LATERAL, top: RECUO_TOPO, bottom: 0 })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            topo(ui, versao);
            ui.add_space(18.0);
            tabela(ui, ficha, pedidos);
            ui.add_space(18.0);
            let e = &paleta(ui).estudio;
            let g = galley_quebrado(
                ui,
                AUTORIA,
                fontes::fonte(11.0, Peso::Normal),
                e.t2,
                ui.available_width(),
                16.0 / 11.0,
            );
            mostrar_paragrafo(ui, &g, AUTORIA);
        });
}

/// O logo de 96 e, ao lado, o nome, a versão e a descrição, centrados na altura do logo (`.sb-topo`).
fn topo(ui: &mut Ui, versao: &str) {
    let e = &paleta(ui).estudio;
    let origem = ui.cursor().min;
    let largura = ui.available_width();
    let largura_texto = (largura - LADO_LOGO - GAP_TOPO).max(40.0);
    let descricao = galley_quebrado(ui, DESCRICAO, fontes::fonte(11.0, Peso::Normal), e.t3, largura_texto, 15.0 / 11.0);
    // 20px em linha de 28 (1.4), 3px, 12px em 17, 3px do gap e 3px do `margin-top` da descrição
    let altura_coluna = 28.0 + 3.0 + ALTURA_LINHA + 3.0 + 3.0 + descricao.tamanho().y;
    let altura = LADO_LOGO.max(altura_coluna);
    let logo = Rect::from_min_size(pos2(origem.x, origem.y + (altura - LADO_LOGO) / 2.0), vec2(LADO_LOGO, LADO_LOGO));
    pintar_logo(ui, logo);
    let coluna = Rect::from_min_size(
        pos2(origem.x + LADO_LOGO + GAP_TOPO, origem.y + (altura - altura_coluna) / 2.0),
        vec2(largura_texto, altura_coluna),
    );
    let linha_nome = Rect::from_min_size(coluna.min, vec2(largura_texto, 28.0));
    let linha_versao =
        Rect::from_min_size(pos2(coluna.left(), linha_nome.bottom() + 3.0), vec2(largura_texto, ALTURA_LINHA));
    let linha_descricao = Rect::from_min_size(
        pos2(coluna.left(), linha_versao.bottom() + 6.0),
        vec2(largura_texto, descricao.tamanho().y),
    );
    let mut nome = ui.new_child(UiBuilder::new().id_salt("sobre::nome").max_rect(linha_nome).layout(centrado()));
    texto_peso(&mut nome, comum::app::NOME, 20.0, Peso::Medio, e.t);
    let mut ver = ui.new_child(UiBuilder::new().id_salt("sobre::versao").max_rect(linha_versao).layout(centrado()));
    texto_peso(&mut ver, &format!("Versão {versao}"), 12.0, Peso::Normal, e.t2);
    let mut desc = ui.new_child(UiBuilder::new().id_salt("sobre::descricao").max_rect(linha_descricao));
    mostrar_paragrafo(&mut desc, &descricao, DESCRICAO);
    ui.allocate_rect(Rect::from_min_size(origem, vec2(largura, altura)), Sense::hover());
}

/// A ficha (`.sb-det`): 92 / 1fr, `gap:7px 14px`, `padding:14px 0` e o fio `--vidrob` em cima e embaixo.
fn tabela(ui: &mut Ui, f: &Ficha, pedidos: &mut Pedidos) {
    let e = &paleta(ui).estudio;
    let largura = ui.available_width();
    let fio = Stroke::new(1.0, e.vidrob);
    let x = ui.max_rect().x_range();
    let y = ui.cursor().min.y;
    ui.painter().hline(x, y + 0.5, fio);
    ui.add_space(1.0 + 14.0);
    let linha = |ui: &mut Ui, indice: usize, rotulo: &str, valor: &mut dyn FnMut(&mut Ui)| {
        if indice > 0 {
            ui.add_space(GAP_LINHAS);
        }
        let topo = ui.cursor().min;
        let rect_rotulo = Rect::from_min_size(topo, vec2(LARGURA_ROTULO, ALTURA_LINHA));
        let mut r =
            ui.new_child(UiBuilder::new().id_salt(("sobre::rotulo", indice)).max_rect(rect_rotulo).layout(centrado()));
        texto_peso(&mut r, rotulo, 12.0, Peso::Normal, e.t3);
        let x_valor = topo.x + LARGURA_ROTULO + GAP_COLUNAS;
        let rect_valor =
            Rect::from_min_size(pos2(x_valor, topo.y), vec2((topo.x + largura - x_valor).max(0.0), ALTURA_LINHA));
        let mut v =
            ui.new_child(UiBuilder::new().id_salt(("sobre::valor", indice)).max_rect(rect_valor).layout(centrado()));
        valor(&mut v);
        ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, ALTURA_LINHA)), Sense::hover());
    };
    // o `.mono` das pranchetas é a fonte da interface em peso 350: sem Consolas
    linha(ui, 0, "Compilação", &mut |ui| {
        texto_peso(ui, f.compilacao, 12.0, Peso::Leve, e.t);
    });
    linha(ui, 1, "Renderizador", &mut |ui| {
        texto_peso(ui, f.renderizador, 12.0, Peso::Normal, e.t);
    });
    linha(ui, 2, "SQLite", &mut |ui| {
        texto_peso(ui, &f.sqlite, 12.0, Peso::Normal, e.t);
    });
    linha(ui, 3, "Sistema", &mut |ui| {
        texto_peso(ui, &f.sistema, 12.0, Peso::Normal, e.t);
    });
    linha(ui, 4, "Licença", &mut |ui| {
        pedidos.licenca |= ligacao(ui, "MIT", Peso::Normal, "Abrir o texto da licença").clicked();
    });
    // os caminhos (`.cam`, revisão de UI de 19/10/2026): o link cortado com "…" e o Copiar caminho de 20px ao lado
    let caminho = |ui: &mut Ui, rotulo: &str, dica: &str, inteiro: &Path, copiar: &mut Option<PathBuf>| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let largura = (ui.available_width() - 4.0 - LADO_CABECALHO).max(24.0);
        let clicou = ligacao_cortada(ui, rotulo, Peso::Leve, dica, largura).clicked();
        let botao = Botao::icone(Icone::CopiarCaminho).do_cabecalho().rotulo("Copiar caminho").dica("Copiar caminho");
        if ui.add(botao).clicked() {
            *copiar = Some(inteiro.to_path_buf());
        }
        clicou
    };
    linha(ui, 5, "Preferências", &mut |ui| {
        let dica = format!("Abrir a pasta no Explorador: {}", f.pasta_preferencias.display());
        pedidos.pasta |= caminho(ui, &f.rotulo_preferencias(), &dica, &f.pasta_preferencias, &mut pedidos.copiar);
    });
    linha(ui, 6, "Arquivo de log", &mut |ui| {
        let dica = format!("Abrir o arquivo: {}", f.log.display());
        pedidos.log |= caminho(ui, &f.rotulo_log(), &dica, &f.log, &mut pedidos.copiar);
    });
    ui.add_space(14.0);
    let y = ui.cursor().min.y;
    ui.painter().hline(x, y + 0.5, fio);
    ui.add_space(1.0);
}

fn centrado() -> egui::Layout {
    egui::Layout::left_to_right(egui::Align::Center)
}

// ------------------------------------------------------------------------------------------------ as janelas de cima

/// "Licença": o texto do LICENSE. Devolve false quando fechou.
fn janela_licenca(egui: &egui::Context) -> bool {
    let mut fechar = false;
    let r = Janela::new("sobre::licenca", "Licença")
        .largura(620.0)
        .rodape_direita(|ui| {
            fechar = ui.add(Botao::primario("Fechar")).clicked();
        })
        .mostrar(egui, |ui| {
            TextoPre::new(LICENCA).largura_total(true).altura_max(360.0).id_salt("sobre::licenca::texto").mostrar(ui);
        });
    !(r.pediu_fechar || fechar)
}

/// A altura de cada linha da lista, o recuo à direita (a barra de rolagem flutuante não cobre o texto) e a menor
/// largura da coluna da licença.
const ALTURA_LINHA_LISTA: f32 = 22.0;
const RECUO_DIREITO_LISTA: f32 = 12.0;
const LICENCA_MINIMA: f32 = 120.0;

/// A largura das três colunas da lista: o nome e a versão têm a medida do maior texto (até uma fração do corpo), a
/// licença fica com o que sobra e é cortada com "…" quando não cabe.
fn colunas_da_lista(lista: &[Biblioteca], total: f32, mede: impl Fn(&str) -> f32) -> [f32; 3] {
    let util = (total - 2.0 * GAP_COLUNAS).max(0.0);
    let maior = |campo: fn(&Biblioteca) -> &'static str| lista.iter().map(|b| mede(campo(b))).fold(0.0_f32, f32::max);
    let nome = maior(|b| b.nome).min(util * 0.40).ceil();
    let versao = maior(|b| b.versao).min(util * 0.28).ceil();
    let licenca = (util - nome - versao).max(LICENCA_MINIMA.min(util));
    [nome, versao, licenca]
}

/// Uma célula da lista: o texto numa linha só, cortado com "…" se passar de `largura` (e então a dica leva o texto
/// inteiro). O nome acessível é sempre o texto inteiro.
fn celula(ui: &mut Ui, texto: &str, peso: Peso, cor: egui::Color32, largura: f32) -> Response {
    let fonte = fontes::fonte(12.0, peso);
    let inteiro = galley(ui, texto, fonte.clone(), cor);
    let cortado = inteiro.size().x > largura;
    let g = if cortado { galley_cortado(ui, texto, fonte, cor, largura) } else { inteiro };
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, g, cor);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, texto));
    if cortado { resp.on_hover_text(texto) } else { resp }
}

/// "Licenças de terceiros": as diretas e depois as que vêm por elas. Devolve false quando fechou.
fn janela_terceiros(egui: &egui::Context) -> bool {
    let mut fechar = false;
    let lista = bibliotecas();
    let r = Janela::new("sobre::terceiros", "Licenças de terceiros")
        .largura(620.0)
        .rodape_direita(|ui| {
            fechar = ui.add(Botao::primario("Fechar")).clicked();
        })
        .mostrar(egui, |ui| {
            let e = &paleta(ui).estudio;
            let (t, t2, t3) = (e.t, e.t2, e.t3);
            let intro = "O DataBox usa as bibliotecas abaixo, todas de código aberto, com a versão e a licença que \
                         vieram na compilação. O SQLite, embutido pelo rusqlite, é de domínio público. A lista \
                         inclui também dependências que só valem em outros sistemas.";
            let g = galley_quebrado(ui, intro, fontes::fonte(12.0, Peso::Normal), t2, ui.available_width(), 1.5);
            mostrar_paragrafo(ui, &g, intro);
            ui.add_space(14.0);
            // as linhas: o título do grupo e as bibliotecas dele
            let mut linhas: Vec<Linha> = vec![Linha::Titulo("Bibliotecas do projeto")];
            linhas.extend(lista.iter().filter(|b| b.direta).map(Linha::Biblioteca));
            linhas.push(Linha::Titulo("Dependências delas"));
            linhas.extend(lista.iter().filter(|b| !b.direta).map(Linha::Biblioteca));
            // o `show_rows` mede a linha com o espaçamento do `ui` de fora (o corpo da janela usa 12 px entre os
            // itens): o espaçamento vertical da lista tem de ser zero ANTES dele, senão ele conta 22 + 12 por linha e
            // desenha 22, e as últimas linhas nunca aparecem
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let [w_nome, w_versao, w_licenca] =
                    colunas_da_lista(&lista, ui.available_width() - RECUO_DIREITO_LISTA, |s| {
                        galley(ui, s, fontes::fonte(12.0, Peso::Normal), t).size().x
                    });
                let x_versao = w_nome + GAP_COLUNAS;
                let x_licenca = x_versao + w_versao + GAP_COLUNAS;
                ScrollArea::vertical()
                    .id_salt("sobre::terceiros::rolagem")
                    .max_height(380.0)
                    .auto_shrink([false, true])
                    .show_rows(ui, ALTURA_LINHA_LISTA, linhas.len(), |ui, faixa| {
                        for i in faixa {
                            let rect =
                                Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), ALTURA_LINHA_LISTA));
                            // cada coluna num lugar fixo da linha, com a largura medida
                            let mut coluna = |x: f32, id: &str| {
                                let r = Rect::from_min_max(pos2(rect.left() + x, rect.top()), rect.right_bottom());
                                ui.new_child(
                                    UiBuilder::new()
                                        .id_salt(("sobre::terceiro", i, id.to_string()))
                                        .max_rect(r)
                                        .layout(centrado()),
                                )
                            };
                            match &linhas[i] {
                                Linha::Titulo(titulo) => {
                                    texto_peso(&mut coluna(0.0, "titulo"), titulo, 12.0, Peso::Medio, t);
                                }
                                Linha::Biblioteca(b) => {
                                    celula(&mut coluna(0.0, "nome"), b.nome, Peso::Normal, t, w_nome);
                                    celula(&mut coluna(x_versao, "versao"), b.versao, Peso::Normal, t2, w_versao);
                                    let (licenca, cor) =
                                        if b.licenca.is_empty() { ("não informada", t3) } else { (b.licenca, t2) };
                                    celula(&mut coluna(x_licenca, "licenca"), licenca, Peso::Normal, cor, w_licenca);
                                }
                            }
                            ui.allocate_rect(rect, Sense::hover());
                        }
                    });
            });
        });
    !(r.pediu_fechar || fechar)
}

/// Uma linha da lista rolável: o título de um grupo ou uma biblioteca.
enum Linha<'a> {
    Titulo(&'static str),
    Biblioteca(&'a Biblioteca),
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sistema_com_a_arquitetura_do_programa() {
        assert_eq!(sistema("windows", "x86_64"), "Windows · x64");
        assert_eq!(sistema("windows", "aarch64"), "Windows · ARM64");
        assert_eq!(sistema("linux", "x86"), "Linux · x86");
        assert_eq!(sistema("freebsd", "riscv64"), "freebsd · riscv64");
        assert_eq!(
            std::env::consts::OS == "windows",
            sistema(std::env::consts::OS, std::env::consts::ARCH).starts_with("Windows")
        );
    }

    /// C29: a ficha mostra o rótulo do Windows ou o caminho real (fora dele não existe `%APPDATA%`), e o texto
    /// copiado nunca leva o nome da pessoa: usa sempre os rótulos.
    #[test]
    fn a_ficha_mostra_o_caminho_real_fora_do_windows_e_a_copia_so_leva_os_rotulos() {
        let ficha = Ficha {
            compilacao: "17/10/2026 · abc1234",
            renderizador: "wgpu",
            sqlite: "3.53.2, embutido".into(),
            sistema: "Windows · x64".into(),
            pasta_preferencias: PathBuf::from(r"C:\Users\Fulano\AppData\Roaming\DataBox"),
            log: PathBuf::from(r"C:\Users\Fulano\AppData\Local\DataBox\databox.log"),
        };
        if cfg!(windows) {
            assert_eq!(ficha.rotulo_preferencias(), ROTULO_PREFERENCIAS);
            assert_eq!(ficha.rotulo_log(), ROTULO_LOG);
        } else {
            assert!(ficha.rotulo_preferencias().contains("Fulano") && ficha.rotulo_log().contains("Fulano"));
        }
        let copiado = informacoes("1.0.0", &ficha);
        assert!(!copiado.contains("Fulano"), "{copiado}");
        assert!(copiado.contains(ROTULO_PREFERENCIAS) && copiado.contains(ROTULO_LOG), "{copiado}");
    }

    #[test]
    fn renderizador_em_palavras() {
        assert_eq!(texto_do_renderizador(Renderizador::Glow), "OpenGL (glow)");
        assert_eq!(texto_do_renderizador(Renderizador::Wgpu), "wgpu");
    }

    #[test]
    fn compilacao_tem_data_e_hash_ou_diz_que_nao_sabe() {
        let esperado = COMPILACAO == "desconhecida" || {
            let (data, hash) = COMPILACAO.split_once(" · ").expect("data · hash");
            let partes: Vec<&str> = data.split('/').collect();
            partes.len() == 3
                && partes.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
                && hash.len() >= 7
                && hash.chars().all(|c| c.is_ascii_hexdigit())
        };
        assert!(esperado, "{COMPILACAO}");
    }

    /// O que o Explorer mostra em Propriedades › Detalhes do DataBox.exe (o .rc é UTF-8, com o `code_page` dele).
    #[test]
    fn recursos_tem_a_empresa_e_a_licenca() {
        let rc =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/recursos/databox.rc")).expect("databox.rc");
        assert!(rc.contains("#pragma code_page(65001)"), "o .rc em UTF-8");
        for linha in [
            r#"VALUE "CompanyName", "Isaías Gonçalves""#,
            r#"VALUE "LegalCopyright", "© 2026 Isaías Gonçalves — licença MIT""#,
            r#"VALUE "FileDescription", "DataBox""#,
            r#"VALUE "OriginalFilename", "DataBox.exe""#,
        ] {
            assert!(rc.contains(linha), "{linha}");
        }
        // a versão do .rc é a do pacote (0.2.4): o texto e os quatro números
        let versao = env!("CARGO_PKG_VERSION");
        let numeros = versao.replace('.', ",");
        for linha in [
            format!(r#"VALUE "FileVersion", "{versao}""#),
            format!(r#"VALUE "ProductVersion", "{versao}""#),
            format!("FILEVERSION {numeros},0"),
            format!("PRODUCTVERSION {numeros},0"),
        ] {
            assert!(rc.contains(&linha), "{linha}");
        }
        // o ícone e o manifesto que ele embute existem ao lado
        for arquivo in ["databox.ico", "databox.manifest"] {
            assert!(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("recursos").join(arquivo).exists(),
                "{arquivo}"
            );
        }
    }

    #[test]
    fn licenca_e_a_mit_do_projeto() {
        assert!(LICENCA.starts_with("MIT License"));
        assert!(LICENCA.contains("Permission is hereby granted, free of charge"));
    }

    #[test]
    fn colunas_da_lista_cabem_no_corpo() {
        let lib = |nome, versao, licenca| Biblioteca { nome, versao, licenca, direta: false };
        let lista =
            [lib("egui", "0.36.2", "MIT OR Apache-2.0"), lib("nome-muito-comprido-mesmo", "1.0.4+wasi-0.2.12", "MIT")];
        let mede = |s: &str| s.chars().count() as f32 * 7.0;
        let [nome, versao, licenca] = colunas_da_lista(&lista, 550.0, mede);
        // as duas primeiras têm a medida do maior texto; a licença fica com o resto, e tudo cabe com os vãos
        assert_eq!(nome, 25.0 * 7.0);
        assert_eq!(versao, 17.0 * 7.0);
        assert!(nome + versao + licenca + 2.0 * GAP_COLUNAS <= 550.0 + 0.5, "{nome} {versao} {licenca}");
        assert!(licenca >= LICENCA_MINIMA);
        // com o corpo estreito, o nome e a versão cedem espaço (frações do corpo) e a licença nunca some
        let [n, v, l] = colunas_da_lista(&lista, 300.0, |s| mede(s) * 3.0);
        assert!(n <= 300.0 * 0.40 && v <= 300.0 * 0.28 && l >= LICENCA_MINIMA, "{n} {v} {l}");
    }

    #[test]
    fn terceiros_listam_as_bibliotecas_da_compilacao() {
        let lista = bibliotecas();
        let nomes: Vec<&str> = lista.iter().map(|b| b.nome).collect();
        for direta in ["egui", "eframe", "rusqlite", "chrono", "rfd", "zip"] {
            let b = lista.iter().find(|b| b.nome == direta).unwrap_or_else(|| panic!("falta {direta}: {nomes:?}"));
            assert!(b.direta && !b.versao.is_empty(), "{direta}");
        }
        assert!(!nomes.contains(&"egui_kittest"), "as de teste ficam de fora");
        assert!(!nomes.contains(&"editor"), "os crates do projeto não são de terceiros");
        // quem vem por outra biblioteca também está lá, sem ser direta
        assert!(lista.iter().any(|b| !b.direta));
        // a licença veio dos pacotes (egui é MIT ou Apache-2.0)
        let egui = lista.iter().find(|b| b.nome == "egui").expect("egui");
        assert!(egui.licenca.contains("MIT") || egui.licenca.contains("Apache"), "{}", egui.licenca);
    }
}
