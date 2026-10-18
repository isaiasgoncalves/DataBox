//! Apoio para testar as telas com o `egui_kittest` (recurso `apoio-testes`; só para `[dev-dependencies]`):
//! um [`Ambiente`] com o banco de exemplo numa pasta temporária (preferências também nela: nunca as reais) e
//! um [`Palco`] que desenha uma tela com o tema aplicado, um [`Contexto`] de verdade e as [`Acao`]s
//! emitidas guardadas para o teste conferir. As capturas vão para `capturas/rust/`, para comparar com as pranchetas
//! de `design/canvas`.
//!
//! ```ignore
//! let amb = Ambiente::exemplo("dados_clientes");
//! let tabela = amb.tabela("clientes");
//! let mut h = palco(&amb, (AbaDados::nova(&tabela), tabela), Tema::Claro, vec2(1440.0, 900.0),
//!     |ui, ctx, (aba, tabela)| aba.ui(ui, ctx, tabela));
//! esperar_texto(&mut h, "Lucas Brandão");
//! capturar(&mut h, "dados-clientes-claro.png");
//! assert!(h.state().acoes.is_empty());
//! ```

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use egui::{UiBuilder, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use interface::tema::{self, Tema};
use interface::{fontes, pintura};
use servico::Servico;

use crate::acao::Acao;
use crate::contexto::{Contexto, Estado};
use crate::documento::{Anatomia, BuscaDaBarra, Documento, EventoBusca, ParteTabela, Rolagem};
use crate::fonte::FonteDeColunas;
use crate::rota::Rota;
use crate::tabela::TabelaAberta;
use crate::verbo::{Ofertas, Verbo};

static CONTADOR: AtomicUsize = AtomicUsize::new(0);

/// Pasta temporária, preferências próprias, serviço e estado — com ou sem o banco de exemplo aberto.
pub struct Ambiente {
    pasta: PathBuf,
    pub servico: Arc<Servico>,
    pub estado: Estado,
}

impl Ambiente {
    /// Ambiente com uma cópia do banco de exemplo (`amostras`) aberta para edição.
    pub fn exemplo(nome_teste: &str) -> Ambiente {
        let amb = Ambiente::vazio(nome_teste);
        let caminho = amostras::criar_banco_exemplo(amb.pasta.join("exemplo.db")).expect("criar exemplo.db");
        amb.servico.abrir_banco(&caminho, false, false).expect("abrir exemplo.db");
        let mut amb = amb;
        amb.recarregar();
        amb
    }

    /// Ambiente sem banco aberto, com uma pasta de dados de verdade ([`Ambiente::pasta_de_dados`]) para os testes da
    /// pasta e do Explorador. Ela tem:
    ///
    /// - `exemplo.db` e `estoque_2026.db`: bancos SQLite de verdade (o de exemplo, duas vezes);
    /// - `sem_extensao` e `copia.dat`: o mesmo banco, sem extensão de banco (só o cabeçalho diz o que é);
    /// - `Thumbs.db`: tem extensão de banco e **não** é SQLite;
    /// - `clientes.csv`, `planilha.xlsx`, `dados.json`, `notas.txt`: arquivos de dados (o `.xlsx` só tem os bytes de
    ///   um zip: basta para a ficha);
    /// - `relatorio.pdf`: um tipo que o app não abre;
    /// - `sub\interno.csv` e `sub\vazia\` (uma subpasta com um arquivo e outra vazia).
    ///
    /// `itens` são caminhos a mais, relativos à pasta de dados: o que termina em `/` ou `\` é uma pasta, o resto é um
    /// arquivo de texto pequeno (as pastas de cima são criadas). Passe `&[]` para só o conjunto acima.
    pub fn com_pasta(nome_teste: &str, itens: &[&str]) -> Ambiente {
        let amb = Ambiente::vazio(nome_teste);
        let dados = amb.pasta_de_dados();
        std::fs::create_dir_all(dados.join("sub").join("vazia")).expect("criar a pasta de dados");
        let banco = amostras::criar_banco_exemplo(dados.join("exemplo.db")).expect("criar exemplo.db");
        let bytes = std::fs::read(&banco).expect("ler exemplo.db");
        for nome in ["estoque_2026.db", "sem_extensao", "copia.dat"] {
            std::fs::write(dados.join(nome), &bytes).expect("gravar a cópia do banco");
        }
        let escrever = |nome: &str, conteudo: &[u8]| {
            let destino = dados.join(nome);
            if let Some(pai) = destino.parent() {
                std::fs::create_dir_all(pai).expect("criar a pasta do arquivo");
            }
            std::fs::write(destino, conteudo).expect("gravar o arquivo de teste");
        };
        escrever(
            "Thumbs.db",
            b"isto nao e um banco SQLite, e so um arquivo com extensao de banco".repeat(4).as_slice(),
        );
        escrever("clientes.csv", "id;nome\n1;Ana\n2;Bruno\n".as_bytes());
        escrever("planilha.xlsx", b"PK\x03\x04 planilha de teste");
        escrever("dados.json", br#"[{"id": 1, "nome": "Ana"}]"#);
        escrever("notas.txt", "uma linha de texto\n".as_bytes());
        escrever("relatorio.pdf", b"%PDF-1.4 relatorio de teste");
        escrever(&format!("sub{}interno.csv", std::path::MAIN_SEPARATOR), "x\n1\n".as_bytes());
        for item in itens {
            if item.ends_with('/') || item.ends_with('\\') {
                std::fs::create_dir_all(dados.join(item.trim_end_matches(['/', '\\']))).expect("criar a pasta extra");
            } else {
                escrever(item, b"texto de teste\n");
            }
        }
        amb
    }

    /// A pasta de dados do [`Ambiente::com_pasta`] (`dados`, dentro da pasta temporária, longe do `preferencias.json` e do
    /// log do teste).
    pub fn pasta_de_dados(&self) -> PathBuf {
        self.pasta.join("dados")
    }

    /// Ambiente sem banco aberto (tela de Início).
    pub fn vazio(nome_teste: &str) -> Ambiente {
        let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
        let pasta = std::env::temp_dir().join(format!("editor-tela-{nome_teste}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).expect("criar pasta do teste");
        let prefs = prefs::Preferencias::abrir(Some(pasta.join("preferencias.json")));
        let servico = Arc::new(Servico::novo(prefs).com_arquivo_log(pasta.join("editor.log")));
        let estado = estado_de(&servico);
        Ambiente { pasta, servico, estado }
    }

    /// A pasta temporária do teste (apagada no fim).
    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    /// Caminho do banco de exemplo (em [`Ambiente::exemplo`]).
    pub fn caminho_banco(&self) -> PathBuf {
        self.pasta.join("exemplo.db")
    }

    /// Relê o estado (banco, objetos e preferências) direto do serviço.
    pub fn recarregar(&mut self) {
        self.estado = estado_de(&self.servico);
    }

    /// A tabela (ou visão) carregada como a casca carrega, na aba Dados.
    pub fn tabela(&self, nome: &str) -> TabelaAberta {
        let completa = self.servico.tabela(&self.estado.banco_id(), nome).expect("carregar a tabela");
        let e_visao = completa.info.tipo == banco::TipoObjeto::View;
        TabelaAberta::nova(nome, e_visao, completa, Rota::objeto(nome, e_visao))
    }
}

fn estado_de(servico: &Servico) -> Estado {
    let e = servico.estado().expect("estado do serviço");
    let objetos = e.banco.as_ref().map(|b| servico.objetos(&b.id).expect("objetos"));
    Estado { app: e.app, banco: e.banco, objetos, pasta: None, prefs: e.prefs, ultimo_diretorio: None }
}

impl Drop for Ambiente {
    fn drop(&mut self) {
        let _ = self.servico.fechar_banco(&self.estado.banco_id(), true);
        let _ = std::fs::remove_dir_all(&self.pasta);
    }
}

type Desenho<S> = Box<dyn FnMut(&mut egui::Ui, &mut Contexto, &mut S)>;

/// Estado do harness: o serviço, o estado do app, a tela testada e as ações que ela emitiu.
pub struct Palco<S> {
    pub servico: Arc<Servico>,
    pub estado: Estado,
    pub tela: S,
    /// Tudo o que a tela emitiu, na ordem (o teste confere e pode limpar).
    pub acoes: Vec<Acao>,
    /// As origens de colunas que o [`Contexto::fontes`] devolve à tela (vazia por padrão; o teste do "Trazer colunas"
    /// põe as dele).
    pub fontes: Vec<Arc<dyn FonteDeColunas>>,
    tema: Tema,
    desenhar: Desenho<S>,
}

/// Monta um harness (wgpu, 1px = 1 ponto, sem animações) que desenha `tela` sobre o fundo do tema, como a
/// casca desenha o conteúdo principal, com toasts.
pub fn palco<S: 'static>(
    amb: &Ambiente,
    tela: S,
    tema_escolhido: Tema,
    tamanho: Vec2,
    desenhar: impl FnMut(&mut egui::Ui, &mut Contexto, &mut S) + 'static,
) -> Harness<'static, Palco<S>> {
    let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
    let estado_palco = Palco {
        servico: Arc::clone(&amb.servico),
        estado: amb.estado.clone(),
        tela,
        acoes: Vec::new(),
        fontes: Vec::new(),
        tema: tema_escolhido,
        desenhar: Box::new(desenhar),
    };
    let mut h = Harness::builder()
        .with_size(tamanho)
        .with_pixels_per_point(1.0)
        .with_theme(tema_egui)
        .with_max_steps(64)
        .wgpu()
        .build_ui_state(quadro::<S>, estado_palco);
    let inicio = Instant::now();
    while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(5));
        h.step();
    }
    h.run_steps(3);
    h
}

fn quadro<S>(ui: &mut egui::Ui, p: &mut Palco<S>) {
    let ctx = ui.ctx().clone();
    tema::aplicar(&ctx, p.tema);
    ctx.all_styles_mut(|s| {
        s.animation_time = 0.0;
        s.visuals.text_cursor.blink = false;
    });
    tema::definir_movimento_reduzido(&ctx, true);
    if !fontes::instaladas(&ctx) {
        ctx.request_repaint();
        return;
    }
    let tela = ctx.content_rect();
    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
    pagina.set_clip_rect(tela);
    pintura::pintar_fundo(pagina.painter(), tema::paleta(&pagina), tela);
    let mut acoes = Vec::new();
    {
        let Palco { servico, estado, tela: s, desenhar, fontes, .. } = p;
        let mut c = Contexto::novo(servico, estado, None, &mut acoes).com_fontes(fontes);
        pagina.scope(|ui| desenhar(ui, &mut c, s));
    }
    p.acoes.extend(acoes);
    interface::pecas::toast::desenhar_toast(&ctx, f32::INFINITY);
}

/// Um documento no palco ([`palco_documento`], [`palco_parte`]), com a escolha das Propriedades: à direita (o
/// padrão) ou no fim da página ([`NoPalco::sem_painel`], como a casca faz quando o painel não está à vista).
pub struct NoPalco<D> {
    pub doc: D,
    painel: bool,
}

impl<D> NoPalco<D> {
    /// As Propriedades no fim da página (nas telas de rolagem de página), e não no painel da direita.
    pub fn sem_painel(&mut self) {
        self.painel = false;
    }
}

/// Uma parte de tabela com a [`TabelaAberta`] dela, como documento (o que a tela de tabela faz na casca, sem o
/// cabeçalho e as abas): para o [`palco_parte`].
pub struct ParteNoPalco<P> {
    pub parte: P,
    pub tabela: TabelaAberta,
}

impl<P: ParteTabela> Documento for ParteNoPalco<P> {
    fn anatomia(&self) -> Anatomia {
        self.parte.anatomia()
    }
    fn faixa(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        self.parte.faixa(ui, ctx, &mut self.tabela);
    }
    fn conteudo(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        self.parte.conteudo(ui, ctx, &mut self.tabela);
    }
    fn rodape(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        self.parte.rodape(ui, ctx, &mut self.tabela);
    }
    fn ofertas(&self, estado: &Estado) -> Ofertas {
        self.parte.ofertas(&self.tabela, estado)
    }
    fn executar(&mut self, verbo: Verbo, egui: &egui::Context, ctx: &mut Contexto) {
        self.parte.executar(verbo, egui, ctx, &mut self.tabela);
    }
    fn busca(&mut self) -> Option<BuscaDaBarra<'_>> {
        self.parte.busca()
    }
    fn evento_busca(&mut self, evento: EventoBusca, egui: &egui::Context, ctx: &mut Contexto) {
        self.parte.evento_busca(evento, egui, ctx, &mut self.tabela);
    }
    fn tem_propriedades(&self) -> bool {
        true
    }
    fn propriedades(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        let (parte, tabela) = (&mut self.parte, &self.tabela);
        crate::documento::secao_tabela(ui, tabela, |ui| parte.acoes_da_tabela(ui, ctx, tabela));
        self.parte.propriedades(ui, ctx, &mut self.tabela);
    }
    fn ocupada(&self) -> bool {
        self.parte.ocupada()
    }
    fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        self.parte.em_segundo_plano(egui, ctx, &mut self.tabela);
    }
}

/// O id da aba do palco (a busca da barra é a dela).
pub const ABA_DO_PALCO: u64 = 0;

/// O id do campo da busca da barra no palco (para o teste pôr o foco nele ou conferir quem o tem).
pub fn id_busca_do_palco() -> egui::Id {
    crate::documento::id_busca_da_barra(ABA_DO_PALCO)
}

/// O Ctrl+F da casca (só com os modificadores exatos): o foco na busca, com o texto selecionado. Devolve se foi
/// apertado.
fn ctrl_f(ui: &egui::Ui, texto: &str) -> bool {
    let apertou = ui.input_mut(|i| {
        i.modifiers.matches_exact(egui::Modifiers::COMMAND) && i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)
    });
    if apertou {
        crate::documento::focar_busca_da_barra(ui.ctx(), ABA_DO_PALCO, texto);
    }
    apertou
}

/// A busca da barra de uma parte de tabela no espaço de `ui`, como a casca faz por ela: o Ctrl+F (antes de a parte
/// se desenhar), o campo com o texto e a dica dela e o [`EventoBusca`] passado a ela. Para os palcos próprios das
/// telas, que desenham a parte numa área fixa.
pub fn busca_da_parte(ui: &mut egui::Ui, ctx: &mut Contexto, parte: &mut dyn ParteTabela, tabela: &mut TabelaAberta) {
    let egui = ui.ctx().clone();
    if let Some(b) = parte.busca() {
        ctrl_f(ui, b.texto);
    }
    let evento = crate::documento::busca_da_barra(ui, parte.busca(), ABA_DO_PALCO);
    if let Some(e) = evento {
        parte.evento_busca(e, &egui, ctx, tabela);
    }
}
/// A largura do painel de Propriedades da casca.
const LARGURA_PROPRIEDADES: f32 = 272.0;

/// Desenha um documento como a casca: em cima, uma barra mínima com um `Botao::icone` por verbo ligado (rotulado pela
/// oferta: "Nova linha", "Executar"; o clique executa antes do conteúdo) e a busca da barra (o texto do documento,
/// com a dica dele); embaixo, o `.doc` com os avisos, a faixa, o conteúdo e o rodapé que ele pede, e as
/// Propriedades no painel de 272px à direita ou, com [`NoPalco::sem_painel`], no fim da página.
pub fn desenhar_como_a_casca(ui: &mut egui::Ui, ctx: &mut Contexto, palco: &mut NoPalco<impl Documento>) {
    use interface::pecas::botao::Botao;
    use interface::pecas::documento::{Documento as DocumentoPeca, Pagina};
    use interface::pecas::ferramentas::{ALTURA_BARRA, barra_ferramentas};
    let egui = ui.ctx().clone();
    let tela = ui.max_rect();
    let doc = &mut palco.doc;
    // o Ctrl+F e a barra: os verbos ligados e a busca
    if let Some(b) = doc.busca() {
        ctrl_f(ui, b.texto);
    }
    let ofertas = doc.ofertas(ctx.estado);
    let mut clicado = None;
    let mut evento = None;
    let rect_barra = egui::Rect::from_min_size(tela.min, egui::vec2(tela.width(), ALTURA_BARRA));
    let mut ui_barra = ui.new_child(UiBuilder::new().max_rect(rect_barra));
    barra_ferramentas(
        &mut ui_barra,
        |ui| {
            for (verbo, oferta) in ofertas.iter().filter(|(_, o)| o.ligado || o.ocupado) {
                let rotulo = ofertas.rotulo(verbo);
                let dica = ofertas.dica(verbo);
                let mut b = Botao::icone(verbo.icone()).rotulo(&rotulo).dica(&dica).selecionado(oferta.marcado);
                if oferta.principal {
                    b = b.principal();
                }
                if verbo == Verbo::Interromper {
                    b = b.tipo(interface::pecas::botao::TipoBotao::Perigo);
                }
                if ui.add(b.ocupado(oferta.ocupado).habilitado(oferta.ligado)).clicked() {
                    clicado = Some(verbo);
                }
            }
            evento = crate::documento::busca_da_barra(ui, doc.busca(), ABA_DO_PALCO);
        },
        |_| {},
    );
    if let Some(v) = clicado {
        doc.executar(v, &egui, ctx);
    }
    if let Some(e) = evento {
        doc.evento_busca(e, &egui, ctx);
    }
    // o documento e as Propriedades
    let com_painel = palco.painel && doc.tem_propriedades();
    let topo = rect_barra.bottom() + 8.0;
    let direita = if com_painel { tela.right() - LARGURA_PROPRIEDADES - 8.0 } else { tela.right() };
    let rect_doc = egui::Rect::from_min_max(egui::pos2(tela.left(), topo), egui::pos2(direita, tela.bottom()));
    let anatomia = doc.anatomia();
    let mut peca = DocumentoPeca::abrir(ui, rect_doc, None, anatomia.rodape);
    doc.avisos(&mut peca, ui, ctx);
    if anatomia.faixa {
        peca.faixa(ui, |ui| doc.faixa(ui, ctx));
    }
    let no_fim = !palco.painel && doc.tem_propriedades() && anatomia.rolagem == Rolagem::Pagina;
    match anatomia.rolagem {
        Rolagem::Pagina => peca.pagina(ui, Pagina::new("palco::documento"), |ui| {
            doc.conteudo(ui, ctx);
            if no_fim {
                crate::documento::propriedades_no_fim(ui, |ui| doc.propriedades(ui, ctx));
            }
        }),
        Rolagem::Propria => peca.propria(ui, |ui| doc.conteudo(ui, ctx)),
    }
    let _ = peca.rodape(ui, |ui| doc.rodape(ui, ctx));
    if com_painel {
        let r = egui::Rect::from_min_max(egui::pos2(direita + 8.0, topo), tela.max);
        let mut ui_painel = ui.new_child(UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
        interface::pecas::painel::Painel::new("palco::propriedades", "Propriedades").mostrar(&mut ui_painel, |ui| {
            egui::ScrollArea::vertical().id_salt("palco::propriedades").auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                doc.propriedades(ui, ctx);
            });
        });
    }
}

/// Um palco ([`palco`]) que desenha `doc` como a casca ([`desenhar_como_a_casca`]): os testes clicam nos verbos
/// pelo rótulo da oferta ("Nova linha", "Executar") e digitam na busca pela dica ("Buscar em clientes"). O documento
/// fica em `h.state().tela.doc`.
pub fn palco_documento<D: Documento + 'static>(
    amb: &Ambiente,
    doc: D,
    tema_escolhido: Tema,
    tamanho: Vec2,
) -> Harness<'static, Palco<NoPalco<D>>> {
    palco(amb, NoPalco { doc, painel: true }, tema_escolhido, tamanho, |ui, ctx, p| desenhar_como_a_casca(ui, ctx, p))
}

/// [`palco_documento`] para uma parte de tabela, com a tabela dela (a parte fica em `h.state().tela.doc.parte`, a
/// tabela em `h.state().tela.doc.tabela`).
pub fn palco_parte<P: ParteTabela + 'static>(
    amb: &Ambiente,
    tabela: TabelaAberta,
    parte: P,
    tema_escolhido: Tema,
    tamanho: Vec2,
) -> Harness<'static, Palco<NoPalco<ParteNoPalco<P>>>> {
    palco_documento(amb, ParteNoPalco { parte, tabela }, tema_escolhido, tamanho)
}

/// Roda quadros até `condicao` valer (as tarefas de fundo terminam em outras threads); falha em 15 s.
pub fn esperar<S>(h: &mut Harness<'static, Palco<S>>, mut condicao: impl FnMut(&Harness<'static, Palco<S>>) -> bool) {
    let inicio = Instant::now();
    loop {
        h.step();
        if condicao(h) {
            h.run_steps(2);
            return;
        }
        assert!(inicio.elapsed() < Duration::from_secs(15), "a condição não ficou verdadeira em 15 s");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Roda quadros até aparecer um nó (rótulo de acessibilidade) que contém `texto`.
pub fn esperar_texto<S>(h: &mut Harness<'static, Palco<S>>, texto: &str) {
    let alvo = texto.to_string();
    esperar(h, move |h| h.query_by_label_contains(&alvo).is_some());
}

/// Roda alguns quadros com pausas curtas, para tarefas rápidas terminarem (sem condição).
pub fn assentar<S>(h: &mut Harness<'static, Palco<S>>) {
    for _ in 0..10 {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    h.run_steps(2);
}

/// `capturas/rust/` na raiz do projeto (o crate fica em `crates/estado`).
pub fn pasta_capturas() -> PathBuf {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let pasta = raiz.join("capturas").join("rust");
    let _ = std::fs::create_dir_all(&pasta);
    pasta
}

/// Renderiza o harness e grava `capturas/rust/<nome>` (PNG). Devolve o caminho.
pub fn capturar<S>(h: &mut Harness<'static, Palco<S>>, nome: &str) -> PathBuf {
    h.run_steps(2);
    let imagem = h.render().expect("renderizar com wgpu");
    let destino = pasta_capturas().join(nome);
    imagem.save(&destino).expect("gravar png");
    destino
}

#[cfg(test)]
mod testes {
    use super::*;
    use comum::arquivos::{Palpite, e_cabecalho_sqlite, palpite};

    fn comeco(caminho: &Path) -> Vec<u8> {
        let mut bytes = std::fs::read(caminho).expect("ler o arquivo");
        bytes.truncate(16);
        bytes
    }

    #[test]
    fn com_pasta_cria_o_conjunto_de_arquivos_dos_testes() {
        let amb = Ambiente::com_pasta("com-pasta", &["extra/fundo/a.csv", "so_pasta/"]);
        let dados = amb.pasta_de_dados();
        assert!(!amb.estado.tem_banco() && amb.estado.pasta.is_none(), "nada aberto");
        for nome in [
            "exemplo.db",
            "estoque_2026.db",
            "sem_extensao",
            "copia.dat",
            "Thumbs.db",
            "clientes.csv",
            "planilha.xlsx",
            "dados.json",
            "notas.txt",
            "relatorio.pdf",
            r"sub\interno.csv",
            r"extra\fundo\a.csv",
        ] {
            assert!(dados.join(nome).is_file(), "falta {nome}");
        }
        assert!(dados.join("sub").join("vazia").is_dir() && dados.join("so_pasta").is_dir());
        // os bancos e o arquivo sem extensão são SQLite de verdade; o Thumbs.db só tem a extensão
        for nome in ["exemplo.db", "estoque_2026.db", "sem_extensao", "copia.dat"] {
            assert!(e_cabecalho_sqlite(&comeco(&dados.join(nome))), "{nome} devia ser SQLite");
        }
        let thumbs = dados.join("Thumbs.db");
        assert!(!e_cabecalho_sqlite(&comeco(&thumbs)));
        assert!(matches!(
            palpite("Thumbs.db", std::fs::metadata(&thumbs).expect("metadata").len()),
            Palpite::LerCabecalho
        ));
        // os bancos abrem pelo serviço, e a pasta de dados não mistura com os arquivos do teste
        let info =
            amb.servico.abrir_banco(&dados.join("estoque_2026.db"), false, false).expect("abrir um banco da pasta");
        amb.servico.fechar_banco(&info.id, true).expect("fechar o banco");
        assert!(!dados.join("preferencias.json").exists(), "a pasta de dados é só dos arquivos de teste");
    }
}
