//! O corpo da janela Exportar: o aviso no alto, os campos (`.campos`: Formato, O que
//! exportar, Opções) e o bloco de progresso. Medidas: 12px entre as partes, título de seção com o fio
//! acima, opções em rádio, lista de caixas e o bloco de progresso.

use egui::{Align, Color32, Id, Layout, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use exportar::Formato;
use interface::fontes::{self, Peso};
use interface::icones::{Icone, TipoArquivo};
use interface::pecas::ajuda::ajuda_mini;
use interface::pecas::arranjo::{em_linha, linha, linha_com_direita};
use interface::pecas::avisos::{Aviso, TomAviso};
use interface::pecas::botao::Botao;
use interface::pecas::campos::{Check, Select, foco_inicial_pendente, pedir_foco_inicial, radio, rotulo_campo};
use interface::pecas::navegacao::{ParteSeletor, TamanhoTrilho, trilho_com_partes};
use interface::pecas::progresso::barra_progresso;
use interface::pecas::texto::{Trecho, galley, galley_quebrado, galley_tit, mostrar_paragrafo, texto_rico};
use interface::pintura;
use interface::tema::paleta;

use crate::progresso::VistaProgresso;

pub(crate) const FORMATOS: [(Formato, &str); 3] =
    [(Formato::Xlsx, "Excel (.xlsx)"), (Formato::Csv, "CSV"), (Formato::Json, "JSON")];

/// O arquivo de 16 de cada formato no trilho (o `.sk.s16.arq` com XLSX, CSV e JSON por cima).
fn arquivo_do_formato(formato: Formato) -> TipoArquivo {
    match formato {
        Formato::Xlsx => TipoArquivo::Xlsx,
        Formato::Csv => TipoArquivo::Csv,
        Formato::Json => TipoArquivo::Json,
    }
}
pub(crate) const SEPARADORES: [(char, &str); 3] =
    [(';', "Ponto e vírgula (Excel em português)"), (',', "Vírgula"), ('\t', "Tabulação")];
pub(crate) const DECIMAIS: [(char, &str); 2] = [(',', "Vírgula"), ('.', "Ponto")];

const AJUDA_DECIMAL: &str =
    "Só para números com casas decimais: o Excel em português espera vírgula. Números inteiros não mudam.";
const AJUDA_FORMULAS: &str = "Um texto que começa com =, +, - ou @ seria lido pelo Excel como fórmula. Com a \
                              proteção, ele ganha um apóstrofo na frente e abre como texto.";
const SOBRE_XLSX: &str = "Cabeçalho congelado e com filtro; textos que começam com = ficam como texto.";
const SOBRE_XLSX_BANCO: &str = "Uma planilha por tabela, com o cabeçalho congelado e filtro.";
const SOBRE_CSV: &str = "Texto separado, que o Excel e outros programas abrem.";
const SOBRE_CSV_BANCO: &str =
    "Texto separado, que o Excel e outros programas abrem. O banco inteiro vira um arquivo .zip com um CSV por tabela.";
const SOBRE_JSON: &str = "Uma lista de objetos, um por linha, com os nomes das colunas.";

/// A altura de uma linha de `label.op` (a caixa de 16px e o texto de 12px em linha de 17px), com os 18px do Check.
const ALTURA_CHECK: f32 = 18.0;
/// O espaço entre duas linhas de opção (`.blk{gap:8px}` com a linha de 17px: passo de 25px) e entre as da lista de
/// tabelas (`.lista-caixa{gap:6px 12px}`: passo de 23px).
const ESPACO_OPCOES: f32 = 7.0;
const ESPACO_LISTA: f32 = 5.0;
/// O `gap:18px` entre os blocos do corpo.
const GAP_BLOCOS: f32 = 18.0;
/// A linha do "Tabelas" é a do botão pequeno (`.btn.sutil` de 24px).
const ALTURA_BOTAO_PEQUENO: f32 = interface::pecas::botao::ALTURA_PEQUENO;

/// De onde vêm as linhas (decide as partes da janela).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tipo {
    Tabela,
    Consulta,
    Banco,
}

impl Tipo {
    /// O ícone sólido de 20 do cabeçalho: a ficha da tabela, o cilindro do banco e a folha SQL do resultado.
    pub fn icone(self) -> Icone {
        match self {
            Tipo::Tabela => Icone::Tabela,
            Tipo::Banco => Icone::Banco,
            Tipo::Consulta => Icone::Folha(TipoArquivo::Sql),
        }
    }
}

/// O que exportar de uma tabela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Escopo {
    Tudo,
    Filtradas,
    Marcadas,
}

/// Uma tabela da lista do banco inteiro, com o rótulo encurtado para a largura da coluna.
pub(crate) struct ItemTabela {
    pub nome: String,
    pub marcado: bool,
    /// A ficha de 16 pelo tipo da tabela (comum, sem rowid ou virtual).
    pub icone: Icone,
    /// (largura medida, rótulo com "…" quando não cabe)
    corte: Option<(f32, Option<String>)>,
}

impl ItemTabela {
    pub fn novo(nome: String, icone: Icone) -> ItemTabela {
        ItemTabela { nome, marcado: true, icone, corte: None }
    }

    /// A caixa e o rótulo que cabe em `largura` (medido só quando a largura muda); o rótulo vem com o nome
    /// inteiro quando foi encurtado (para a dica e o nome acessível).
    fn partes(&mut self, ui: &Ui, largura: f32) -> (&mut bool, &str, Option<&str>) {
        if self.corte.as_ref().is_none_or(|(l, _)| (l - largura).abs() > 0.5) {
            self.corte = Some((largura, cortar(ui, &self.nome, largura)));
        }
        match &self.corte {
            Some((_, Some(curto))) => (&mut self.marcado, curto, Some(&self.nome)),
            _ => (&mut self.marcado, &self.nome, None),
        }
    }
}

/// O nome com "…" no fim quando não cabe em `largura` (o `text-overflow:ellipsis` do `.check-nome`).
fn cortar(ui: &Ui, texto: &str, largura: f32) -> Option<String> {
    let fonte = fontes::fonte(12.5, Peso::Normal);
    let medir = |s: &str| galley(ui, s, fonte.clone(), Color32::PLACEHOLDER).size().x;
    if medir(texto) <= largura {
        return None;
    }
    let chars: Vec<char> = texto.chars().collect();
    let (mut cabe, mut nao_cabe) = (0usize, chars.len());
    while nao_cabe - cabe > 1 {
        let meio = (cabe + nao_cabe) / 2;
        let mut s: String = chars[..meio].iter().collect();
        s.push('…');
        if medir(&s) <= largura {
            cabe = meio;
        } else {
            nao_cabe = meio;
        }
    }
    let mut s: String = chars[..cabe].iter().collect();
    s.push('…');
    Some(s)
}

/// O estado do formulário da janela.
pub(crate) struct Formulario {
    pub tipo: Tipo,
    pub formato: Formato,
    pub escopo: Escopo,
    pub tabelas: Vec<ItemTabela>,
    pub separador: char,
    pub decimal: char,
    pub cabecalho: bool,
    pub proteger_formulas: bool,
    pub indentado: bool,
    /// "A tabela inteira (60 linhas)", "Só o que a busca e os filtros mostram (12 linhas)", "Só as linhas
    /// marcadas (3)" (os dois últimos só quando há).
    pub rotulo_tudo: String,
    pub rotulo_filtradas: Option<String>,
    pub rotulo_marcadas: Option<String>,
}

impl Formulario {
    /// O formato não tem campos de opção (Excel): o bloco das Opções some (também durante a exportação, que traz o
    /// fio dela).
    pub fn sem_opcoes(&self) -> bool {
        self.formato == Formato::Xlsx
    }

    fn sobre_formato(&self) -> &'static str {
        let banco = self.tipo == Tipo::Banco;
        match (self.formato, banco) {
            (Formato::Xlsx, false) => SOBRE_XLSX,
            (Formato::Xlsx, true) => SOBRE_XLSX_BANCO,
            (Formato::Csv, false) => SOBRE_CSV,
            (Formato::Csv, true) => SOBRE_CSV_BANCO,
            (Formato::Json, _) => SOBRE_JSON,
        }
    }
}

/// O aviso no alto do corpo.
pub(crate) enum AvisoJanela {
    Texto {
        texto: String,
        perigo: bool,
    },
    /// "Arquivo salvo: **nome** · 60 linhas · 12 KB", os avisos do formato e as ações do arquivo.
    Salvo {
        nome: String,
        detalhes: String,
        avisos: Vec<String>,
    },
}

/// O que a pessoa fez no corpo neste quadro.
#[derive(Debug, Default)]
pub(crate) struct Eventos {
    pub mostrar_na_pasta: bool,
    pub abrir_arquivo: bool,
    /// Uma tabela foi marcada (o aviso "Marque ao menos uma tabela" some).
    pub marcou: bool,
}

/// Desenha o corpo inteiro. `exportando`: as opções ficam inertes e mais apagadas.
pub(crate) fn desenhar(
    ui: &mut Ui,
    form: &mut Formulario,
    aviso: Option<&AvisoJanela>,
    progresso: Option<&VistaProgresso>,
    eventos: &mut Eventos,
) {
    ui.spacing_mut().item_spacing.y = 0.0;
    if let Some(a) = aviso {
        desenhar_aviso(ui, a, eventos);
    }
    ui.scope(|ui| {
        if progresso.is_some() {
            // o bloco dos campos inerte (`opacity:.45` da prancheta 5)
            let opacidade = ui.opacity();
            ui.disable();
            ui.set_opacity(opacidade * 0.45);
        }
        campos(ui, form, eventos, progresso.is_some());
    });
    if let Some(v) = progresso {
        bloco_progresso(ui, v);
    }
}

fn desenhar_aviso(ui: &mut Ui, aviso: &AvisoJanela, eventos: &mut Eventos) {
    // os avisos de corpo têm todos a mesma largura e o sólido na mesma coluna (cada um aparece sozinho)
    match aviso {
        AvisoJanela::Texto { texto, perigo } => {
            Aviso::new(*perigo).cheia(true).mostrar(ui, |ui, cores| {
                texto_rico(ui, &[Trecho::Normal(texto)], 12.5, cores.texto, cores.forte);
            });
        }
        AvisoJanela::Salvo { nome, detalhes, avisos } => {
            // salvo: o concluído; com o que o formato não comportou, a atenção
            let tom = if avisos.is_empty() { TomAviso::Concluido } else { TomAviso::Atencao };
            Aviso::com_tom(tom).cheia(true).mostrar(ui, |ui, cores| {
                let trechos = [
                    Trecho::Normal("Arquivo salvo: "),
                    Trecho::Forte(nome),
                    Trecho::Normal(" · "),
                    Trecho::Normal(detalhes),
                ];
                texto_rico(ui, &trechos, 12.5, cores.texto, cores.forte);
                // `ul.avisos-exportacao`: o que o formato não comportou
                if !avisos.is_empty() {
                    lista_avisos(ui, avisos, cores.texto);
                }
                // `.notice .acoes-linha{margin-top:10px}`
                ui.add_space(2.0);
                em_linha(ui, 8.0, |ui| {
                    eventos.mostrar_na_pasta = ui.add(Botao::new("Mostrar na pasta")).clicked();
                    eventos.abrir_arquivo = ui.add(Botao::new("Abrir o arquivo")).clicked();
                });
            });
        }
    }
}

/// `ul.avisos-exportacao`: `padding-left:18px`, um marcador por item e 2px entre eles.
fn lista_avisos(ui: &mut Ui, avisos: &[String], cor: Color32) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for (i, aviso) in avisos.iter().enumerate() {
            let topo = ui.cursor().min;
            let largura = ui.available_width();
            let rect = Rect::from_min_size(topo + vec2(18.0, 0.0), vec2((largura - 18.0).max(10.0), f32::INFINITY));
            let mut filho = ui.new_child(UiBuilder::new().id_salt(("avisos-exportacao", i)).max_rect(rect));
            texto_rico(&mut filho, &[Trecho::Normal(aviso)], 12.5, cor, cor);
            let altura = filho.min_rect().height().max(18.75);
            // o disco do `li` no meio da primeira linha
            ui.painter().circle_filled(pos2(topo.x + 8.0, topo.y + 18.75 / 2.0), 2.2, cor);
            ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, altura)), Sense::hover());
        }
    });
}

/// O trilho de formato (`div.track` com `button[aria-pressed]`, o seletor segmentado do kit), de ponta a ponta do
/// formulário com as três partes iguais (o `.blk .seg` das pranchetas). No quadro em que a janela abre, o foco vai
/// para o item pressionado, e não para o
/// "Exportar…" do rodapé: um Enter ou Espaço logo depois de abrir só reescolhe o formato.
fn trilho_formato(ui: &mut Ui, form: &mut Formulario) {
    // o item `i` do trilho é `id.with(i)`
    let id = Id::new("exportar::formato");
    let partes = FORMATOS.map(|(f, r)| ParteSeletor::new(r).icone(Icone::Folha(arquivo_do_formato(f))));
    let ativo = FORMATOS.iter().position(|(f, _)| *f == form.formato);
    let (clicado, _) = trilho_com_partes(ui, id, &partes, ativo, TamanhoTrilho::Cheio, true);
    // depois de desenhar: no quadro em que a janela abre, o `Ui` vem desabilitado e o egui solta o foco de um
    // item que não o aceita; pedido agora, ele vale a partir do quadro seguinte (como o `foco_inicial` do kit)
    if foco_inicial_pendente(ui) {
        if let Some(i) = ativo {
            ui.memory_mut(|m| m.request_focus(id.with(i)));
        }
        // o rodapé não toma mais o foco inicial
        pedir_foco_inicial(ui, false);
    }
    if let Some((f, _)) = clicado.and_then(|i| FORMATOS.get(i)) {
        form.formato = *f;
    }
}

fn campos(ui: &mut Ui, form: &mut Formulario, eventos: &mut Eventos, exportando: bool) {
    // Formato
    // o corpo da prancheta: `.blk{gap:8px}` entre o título e o controle e `gap:18px` entre os blocos; o que o
    // formato é (o texto do "i" de antes) vai na dica do rótulo
    let sobre = form.sobre_formato();
    rotulo_campo(ui, "Formato", None).on_hover_text(sobre);
    ui.add_space(8.0);
    trilho_formato(ui, form);
    match form.tipo {
        Tipo::Tabela => {
            ui.add_space(GAP_BLOCOS);
            rotulo_campo(ui, "O que exportar", None);
            ui.add_space(8.0);
            radios(ui, form);
        }
        Tipo::Banco => {
            ui.add_space(GAP_BLOCOS);
            lista_tabelas(ui, form, eventos);
        }
        Tipo::Consulta => {}
    }
    // o bloco das Opções: `border-top:1px solid --vidrob; padding-top:16px` depois do `gap:18px`. O Excel não tem
    // opções (o bloco some, como no "Exportar o banco" da prancheta 5) e, durante a exportação, o bloco do andamento
    // ocupa o lugar dele com o fio dele
    if form.sem_opcoes() || exportando {
        return;
    }
    ui.add_space(GAP_BLOCOS);
    let e = &paleta(ui).estudio;
    let y = ui.cursor().min.y;
    let x = ui.max_rect().x_range();
    ui.painter().hline(x, y + 0.5, Stroke::new(1.0, e.vidrob));
    ui.add_space(16.0);
    linha(ui, 17.0, 0.0, |ui| {
        // o `.tit` da prancheta (11px / 600, `--t3`), como "Formato" e "Tabelas", com o "i" 6px depois
        let e = &paleta(ui).estudio;
        let g = galley_tit(ui, "Opções", e.t3);
        let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(r.min, g, e.t3);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Opções"));
        // o "i" do título (`gap:6px`, o `.sk.s14` da prancheta 5)
        ui.add_space(6.0 - ui.spacing().item_spacing.x);
        ajuda_mini(ui, sobre);
    });
    match form.formato {
        Formato::Csv => {
            ui.add_space(8.0);
            opcoes_csv(ui, form);
        }
        Formato::Json => {
            ui.add_space(8.0);
            linha_check(ui, |ui| {
                Check::new(&mut form.indentado, "Indentado (mais fácil de ler)").mostrar(ui);
            });
        }
        Formato::Xlsx => {}
    }
}

/// Uma linha de `label.op` (18px), com o conteúdo centrado na vertical.
fn linha_check(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
    let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), ALTURA_CHECK));
    ui.scope_builder(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        conteudo(ui);
    });
    ui.allocate_rect(rect, Sense::hover());
}

fn radios(ui: &mut Ui, form: &mut Formulario) {
    let mut opcoes: Vec<(Escopo, &str)> = vec![(Escopo::Tudo, form.rotulo_tudo.as_str())];
    if let Some(r) = &form.rotulo_filtradas {
        opcoes.push((Escopo::Filtradas, r));
    }
    if let Some(r) = &form.rotulo_marcadas {
        opcoes.push((Escopo::Marcadas, r));
    }
    let mut escolhido = form.escopo;
    for (i, (escopo, rotulo)) in opcoes.iter().enumerate() {
        if i > 0 {
            ui.add_space(ESPACO_OPCOES);
        }
        linha_check(ui, |ui| {
            if radio(ui, escolhido == *escopo, rotulo).clicked() {
                escolhido = *escopo;
            }
        });
    }
    form.escopo = escolhido;
}

/// "Tabelas" + "Marcar todas"/"Desmarcar todas" e a `.lista-checks` (duas colunas, até 216px, rolando).
fn lista_tabelas(ui: &mut Ui, form: &mut Formulario, eventos: &mut Eventos) {
    let todas = form.tabelas.iter().all(|t| t.marcado);
    let mut alternar = false;
    linha_com_direita(
        ui,
        Id::new("exportar::tabelas::cabecalho"),
        ALTURA_BOTAO_PEQUENO,
        8.0,
        |ui| {
            // o `.tit` no meio da linha do botão (`align-items:center`)
            let e = &paleta(ui).estudio;
            let g = galley_tit(ui, "Tabelas", e.t3);
            let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
            ui.painter().galley(r.min, g, e.t3);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Tabelas"));
        },
        |ui| {
            // `.btn.sutil` de 24px com `padding:0 10px`
            let rotulo = if todas { "Desmarcar todas" } else { "Marcar todas" };
            alternar = ui.add(Botao::ghost(rotulo).pequeno()).clicked();
        },
    );
    if alternar {
        let marcar = !todas;
        for t in &mut form.tabelas {
            t.marcado = marcar;
        }
        eventos.marcou |= marcar;
    }
    // o `gap:8px` do `.blk` até a caixa
    ui.add_space(8.0);
    // `.lista-checks{padding:10px 12px; border-radius:--r-panel; background:--k-surface2; box-shadow:--k-trackRing}`,
    // com o raio único dos painéis
    let raio = pintura::RAIO_PADRAO;
    let p = paleta(ui);
    let topo = ui.cursor().min;
    let largura = ui.available_width();
    let lugar = ui.painter().add(egui::Shape::Noop);
    let n_linhas = form.tabelas.len().div_ceil(2);
    let altura_conteudo = n_linhas as f32 * ALTURA_CHECK + n_linhas.saturating_sub(1) as f32 * ESPACO_LISTA;
    let altura_visivel = altura_conteudo.min(216.0 - 20.0);
    let dentro = Rect::from_min_size(topo + vec2(12.0, 10.0), vec2(largura - 24.0, altura_visivel));
    let mut filho = ui.new_child(UiBuilder::new().id_salt("exportar::tabelas").max_rect(dentro));
    let coluna = ((dentro.width() - 16.0) / 2.0).max(40.0);
    ScrollArea::vertical()
        .id_salt("exportar::tabelas::rolagem")
        .max_height(altura_visivel)
        .auto_shrink([false, true])
        .show(&mut filho, |ui| {
            let inicio = ui.cursor().min;
            for (i, t) in form.tabelas.iter_mut().enumerate() {
                let (linha_i, col) = (i / 2, i % 2);
                let celula = Rect::from_min_size(
                    pos2(
                        inicio.x + col as f32 * (coluna + 16.0),
                        inicio.y + linha_i as f32 * (ALTURA_CHECK + ESPACO_LISTA),
                    ),
                    vec2(coluna, ALTURA_CHECK),
                );
                let mut sub = ui.new_child(
                    UiBuilder::new()
                        .id_salt(("exportar::tabela", i))
                        .max_rect(celula)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                // a caixa (16px), a ficha (16px) e os dois espaços de 8px antes do nome
                let icone = t.icone;
                let (marcado, rotulo, inteiro) = t.partes(&sub, coluna - 48.0);
                let antes = *marcado;
                let r = Check::new(marcado, rotulo).icone(icone).mostrar(&mut sub);
                if let Some(nome) = inteiro {
                    // o corte é só o que se vê (o `text-overflow:ellipsis` do `.check-nome`): o leitor de tela e a
                    // busca pelo rótulo leem o nome inteiro, que também vai na dica
                    sub.ctx().accesskit_node_builder(r.id, |n| n.set_label(nome));
                    r.on_hover_text(nome);
                }
                if t.marcado && !antes {
                    eventos.marcou = true;
                }
            }
            ui.allocate_rect(Rect::from_min_size(inicio, vec2(dentro.width(), altura_conteudo)), Sense::hover());
        });
    let caixa = Rect::from_min_size(topo, vec2(largura, altura_visivel + 20.0));
    ui.painter().set(lugar, egui::Shape::rect_filled(caixa, raio, p.surface2));
    pintura::anel_rebaixado(ui.painter(), caixa, raio, p.track_ring);
    ui.allocate_rect(caixa, Sense::hover());
}

/// A coluna dos rótulos das opções (`grid-template-columns:90px minmax(0,1fr)`) e o `gap:8px 12px`.
const COLUNA_ROTULO_OPCAO: f32 = 90.0;
const ESPACO_COLUNA_OPCAO: f32 = 12.0;
const ESPACO_LINHA_OPCAO: f32 = 8.0;

/// Uma linha das opções: o rótulo à esquerda (`--t2`, 12px) na coluna de 90px, centrado na altura do controle, e o
/// controle no resto da largura. A explicação do campo, quando há, vai na dica do rótulo.
fn linha_de_opcao(ui: &mut Ui, rotulo: &str, dica: Option<&str>, controle: impl FnOnce(&mut Ui)) {
    let largura = ui.available_width();
    let topo = ui.cursor().min;
    let x_controle = COLUNA_ROTULO_OPCAO + ESPACO_COLUNA_OPCAO;
    let area = Rect::from_min_size(topo + vec2(x_controle, 0.0), vec2((largura - x_controle).max(40.0), f32::INFINITY));
    let mut filho = ui.new_child(UiBuilder::new().id_salt(("exportar::opcao", rotulo)).max_rect(area));
    controle(&mut filho);
    let altura = filho.min_rect().height();
    let cor = paleta(ui).estudio.t2;
    let g = galley(ui, rotulo, fontes::fonte(12.0, Peso::Normal), cor);
    let rect_rotulo = Rect::from_min_size(pos2(topo.x, topo.y + (altura - g.size().y) / 2.0), g.size());
    ui.painter().galley(rect_rotulo.min, g, cor);
    let resp = ui.interact(rect_rotulo, ui.id().with(("exportar::rotulo_opcao", rotulo)), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, rotulo));
    if let Some(d) = dica {
        resp.on_hover_text(d);
    }
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, altura)), Sense::hover());
}

fn opcoes_csv(ui: &mut Ui, form: &mut Formulario) {
    // as opções em linhas, o rótulo à esquerda (prancheta 5): a grade `90px | 1fr` com `gap:8px 12px`
    linha_de_opcao(ui, "Separador", None, |ui| {
        Select::new("exportar::separador", &mut form.separador, &SEPARADORES).rotulo("Separador").mostrar(ui);
    });
    ui.add_space(ESPACO_LINHA_OPCAO);
    linha_de_opcao(ui, "Decimal", Some(AJUDA_DECIMAL), |ui| {
        Select::new("exportar::decimal", &mut form.decimal, &DECIMAIS).rotulo("Decimal").mostrar(ui);
    });
    // `.blk{gap:8px}` e o `margin-top:4px` da primeira caixa
    ui.add_space(ESPACO_LINHA_OPCAO + 4.0);
    linha_check(ui, |ui| {
        Check::new(&mut form.cabecalho, "Primeira linha com os nomes das colunas").mostrar(ui);
    });
    ui.add_space(ESPACO_OPCOES);
    linha_check(ui, |ui| {
        // a explicação do apóstrofo vai na dica da caixa (a prancheta 5 não tem o "i" aqui)
        Check::new(&mut form.proteger_formulas, "Proteger contra fórmulas do Excel")
            .mostrar(ui)
            .on_hover_text(AJUDA_FORMULAS);
    });
}

/// O bloco de andamento (prancheta 5): o fio acima (`border-top` com `padding-top:16px`), 18px depois do bloco
/// anterior; a tabela atual do banco inteiro ("Tabela 2 de 7 · eventos", `--t2`), a barra e o texto das linhas
/// (`--t`), os dois em 12px semilight, com 8px entre os três.
fn bloco_progresso(ui: &mut Ui, v: &VistaProgresso) {
    let e = &paleta(ui).estudio;
    ui.add_space(GAP_BLOCOS);
    let y = ui.cursor().min.y;
    ui.painter().hline(ui.max_rect().x_range(), y + 0.5, Stroke::new(1.0, e.vidrob));
    ui.add_space(16.0);
    let largura = ui.available_width();
    let fonte = fontes::fonte(12.0, Peso::Leve);
    if let Some(t) = &v.tabela {
        let g = galley_quebrado(ui, t, fonte.clone(), e.t2, largura, 1.5);
        mostrar_paragrafo(ui, &g, t);
        ui.add_space(8.0);
    }
    barra_progresso(ui, v.fracao);
    ui.add_space(8.0);
    let g = galley_quebrado(ui, &v.texto, fonte, e.t, largura, 1.5);
    mostrar_paragrafo(ui, &g, &v.texto);
}
