//! O mostruário: todas as peças numa tela só, com dados inventados (com o vocabulário do
//! DataBox). Serve para conferir o visual nos dois temas, no exemplo
//! `examples/mostruario.rs` e no teste que grava as capturas em `capturas/rust/`.
//!
//! ```ignore
//! let mut m = Mostruario::new(Tema::Sistema);
//! // a cada quadro, com o `Ui` da janela inteira:
//! m.ui(ui);
//! ```

mod casca;
mod contexto;
pub mod planilha;

use comum::Celula;
use egui::{Align, Color32, Layout, Rect, ScrollArea, Sense, Ui, UiBuilder, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::formato::{formatar_bytes, formatar_numero, partes_bytes, plural};
use crate::grade::{
    CabecalhoVisivel, CelulaVisivel, ColunaGrade, DesenhoGrade, GradeSomenteLeitura, LARGURA_COLUNA_MARCAR, Navegacao,
    NucleoGrade, Ordem, Th, pintar_cabecalho_coluna, pintar_celula, pintar_celula_chave, pintar_numero_linha,
};
use crate::icones::{self, Icone, TipoColuna};
use crate::pecas::ajuda::{Ajuda, IconeEstado, pintar_icone_estado};
use crate::pecas::arranjo::{colunas_iguais, em_linha, fluxo, linha};
use crate::pecas::avisos::{TomAviso, aviso, aviso_tom, carregando, vazio};
use crate::pecas::barra_titulo::ALTURA_TITULO;
use crate::pecas::botao::{ALTURA_JANELA, Botao, TipoBotao, com_altura};
use crate::pecas::campos::{
    AreaTexto, Check, Entrada, EntradaSugestoes, Select, campo, campos_duas_colunas, check, enter_no_campo,
    pintar_salvo, radio,
};
use crate::pecas::cartao::Cartao;
use crate::pecas::detalhes::detalhes;
use crate::pecas::ferramentas::{ALTURA_BARRA, busca_desligada, separador};
use crate::pecas::janela::{Confirmacao, Janela, confirmar};
use crate::pecas::modelo::Modelo;
use crate::pecas::navegacao::{Cabecalho, Passo, TamanhoTrilho, abas, segmentado};
use crate::pecas::progresso::barra_progresso;
use crate::pecas::rotulos::{TipoPill, TipoTag, chip, chip_filtro, etiqueta_mais, pill, tag, tag_clicavel};
use crate::pecas::tabela::{ColunaTabela, Tabela};
use crate::pecas::texto::{
    Trecho, codigo, forte, galley, h4, hint, hint_tamanho, link, link_tamanho, nome_abre, rotulo_caps, texto,
};
use crate::pecas::texto_pre::TextoPre;
use crate::pecas::tiles::{Tile, tiles};
use crate::pecas::toast::{desenhar_toast, toast, toast_concluido};
use crate::pecas::zona_risco::ZonaRisco;
use crate::pintura::{RAIO_PADRAO, pintar_fundo};
use crate::tema::{self, Tema, paleta};

/// Todos os ícones em cada lado (em pontos), uma linha por lado, com o nome na dica: as silhuetas em `--t2`.
fn grade_de_icones(ui: &mut Ui, lados: &[f32]) {
    let cor = paleta(ui).estudio.t2;
    for &lado in lados {
        fluxo(ui, 8.0, |ui| {
            for icone in Icone::TODOS {
                let (r, resp) = ui.allocate_exact_size(vec2(lado, lado), Sense::hover());
                icones::pintar(ui.painter(), r, icone, cor);
                resp.on_hover_text(icone.nome());
            }
        });
    }
}

/// Largura do Explorador à esquerda da página (o painel esquerdo da casca, 258px).
const LARGURA_EXPLORADOR: f32 = 258.0;
/// Altura do Explorador na página: o banco, os três grupos, as sete tabelas e as quatro partes de `clientes`
/// abertos, com folga.
const ALTURA_EXPLORADOR: f32 = 460.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aba {
    Componentes,
    Padroes,
    Decisoes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Formato {
    Xlsx,
    Csv,
    Json,
}

/// O estado do mostruário (os campos de exemplo, a janela aberta, a grade).
pub struct Mostruario {
    tema: Tema,
    aba: Aba,
    formato: Formato,
    nome: String,
    email: String,
    perfil: u8,
    observacoes: String,
    caminho: String,
    recebe: bool,
    marcadas: bool,
    escopo: u8,
    grade: GradeSomenteLeitura,
    /// A grade "Dados" (o núcleo da aba Dados) e as linhas marcadas nela.
    grade_dados: NucleoGrade,
    marcadas_dados: [bool; 5],
    /// A planilha do Excel em miniatura (painel congelado, título mesclado, larguras do arquivo).
    planilha: planilha::PlanilhaExemplo,
    janela_aberta: bool,
    nome_janela: String,
    confirmacao: Option<Confirmacao>,
    sql: String,
    cidade: String,
    usar_padrao: bool,
    comando: String,
    /// As observações da tabela "Cadastrados" (o `td.obs` que salva sozinho) e a última salva (para piscar).
    observacoes_tabela: [String; 3],
    salvo: Option<(usize, f64)>,
    tipo_coluna: String,
    /// A casca nova ([`Mostruario::pagina_casca`]) e se é ela que a janela mostra.
    casca: casca::EstadoCasca,
    vista_casca: bool,
    /// A página dos ícones ([`Mostruario::pagina_icones`]).
    vista_icones: bool,
    /// A página do menu de contexto e dos cartões de ação ([`Mostruario::pagina_contexto`]).
    vista_contexto: bool,
    contexto: contexto::EstadoContexto,
    /// A página do código ([`Mostruario::pagina_codigo`]) e o texto do editor dela.
    vista_codigo: bool,
    editor_codigo: String,
    /// O texto do editor de JSON da página do código.
    editor_json: String,
}

/// A amostra das cartas das Preferências (prancheta 13).
const AMOSTRA_CORES: &str = "SELECT nome, limite\n  FROM \"clientes\"\n WHERE limite > 1000;";

/// O editor da prancheta 3 (Consulta), com o comando 3 com erro.
const EDITOR_CODIGO: &str = "SELECT id, nome, email, cidade, limite FROM clientes ORDER BY id LIMIT 8;\nUPDATE pedidos SET \
                             observacao = observacao WHERE id <= 3;\nSELECT * FROM tabela_que_nao_existe;\n-- \
                             comentário com 'texto' e \"citado\"\n";

/// O editor de JSON (prancheta 8) com a vírgula que falta na linha 6: a chave, o texto, o número e o literal nas
/// cores, e a linha do erro com o fundo, o sublinhado e o número da margem em `--perigo`.
const EDITOR_JSON: &str = "{\n  \"versao\": 3,\n  \"conexoes\": [\n    {\n      \"nome\": \"Produção\",\n      \"somente_leitura\": false\n      \"tempo_limite\": 30\n    }\n  ]\n}\n";

/// Os tipos sugeridos no campo Tipo da janela de coluna.
const TIPOS_SUGERIDOS: [&str; 9] =
    ["INTEGER", "TEXT", "REAL", "NUMERIC", "BLOB", "DATE", "DATETIME", "BOOLEAN", "VARCHAR(255)"];

/// As pessoas da tabela "Cadastrados" do mostruário do kit: nome, e-mail, perfil, situação, último acesso.
type Pessoa = (&'static str, &'static str, (&'static str, TipoPill), (&'static str, TipoPill), &'static str);
const PESSOAS: [Pessoa; 3] = [
    ("Ana Exemplo", "ana.exemplo@example.test", ("rh", TipoPill::Neutra), ("ativo", TipoPill::Ok), "17/09/2026 09:12"),
    (
        "Bruno Fictício",
        "bruno@example.test",
        ("admin", TipoPill::Acc),
        ("bloqueado", TipoPill::Wait),
        "16/09/2026 18:40",
    ),
    ("Carla Inventada", "carla@example.test", ("gestor", TipoPill::Neutra), ("inativo", TipoPill::No), "—"),
];

const COMANDO_EXEMPLO: &str = "SELECT \"cidade\", COUNT(*) AS \"clientes\"\n  FROM \"clientes\"\n GROUP BY \"cidade\"\n ORDER BY \"clientes\" DESC;";

const SQL_EXEMPLO: &str = "CREATE TABLE \"clientes\" (\n  \"id\" INTEGER PRIMARY KEY,\n  \"nome\" TEXT NOT NULL,\n  \"cidade\" TEXT DEFAULT 'São Paulo',\n  \"limite\" REAL CHECK (\"limite\" >= 0),\n  \"criado_em\" TEXT DEFAULT CURRENT_TIMESTAMP\n);\nCREATE INDEX \"idx_clientes_cidade\" ON \"clientes\" (\"cidade\");";

fn grade_exemplo() -> GradeSomenteLeitura {
    // com o tipo ao lado do nome (o `.tp`) e uma célula ativa (`td.ativa`)
    let colunas = vec![
        ColunaGrade::com_tipo("id", "INTEGER"),
        ColunaGrade::com_tipo("nome", "TEXT"),
        ColunaGrade::com_tipo("cidade", "TEXT"),
        ColunaGrade::com_tipo("limite", "REAL"),
        ColunaGrade::com_tipo("foto", "BLOB"),
        ColunaGrade::com_tipo("observacao", "TEXT"),
    ];
    let nomes = [
        "Ana Exemplo",
        "Bruno Fictício",
        "Carla Inventada",
        "Diego Modelo",
        "Elisa Teste",
        "Fábio Amostra",
        "Gina Simulada",
        "Hugo Protótipo",
    ];
    let cidades = ["São Paulo", "Campinas", "", "Santos", "São Paulo", "Recife", "Curitiba", "Natal"];
    let mut linhas = Vec::new();
    for (i, nome) in nomes.iter().enumerate() {
        let limite = match i {
            2 => Celula::Nulo,
            5 => Celula::Real(1_234_567.5),
            _ => Celula::Real(1500.0 + 250.0 * i as f64),
        };
        let foto = if i % 3 == 0 {
            Celula::Blob { tamanho: 2355 + i * 1000, inicio: vec![0x89, 0x50, 0x4E, 0x47] }
        } else {
            Celula::Nulo
        };
        let obs = match i {
            1 => Celula::TextoCortado { inicio: "Cliente desde a primeira loja; prefere contato por e-mail e pediu que as faturas cheguem sempre no começo do mês".into(), tamanho: 1840 },
            4 => Celula::Texto("revisar o cadastro".into()),
            _ => Celula::Nulo,
        };
        linhas.push(vec![
            Celula::Inteiro(i as i64 + 1),
            Celula::Texto((*nome).into()),
            Celula::Texto(cidades[i].into()),
            limite,
            foto,
            obs,
        ]);
    }
    let mut g = GradeSomenteLeitura::new("mostruario", colunas, linhas);
    g.selecionar(Some((1, 2)));
    g
}

/// As linhas da grade "Dados" do mostruário (a `clientes` de Dados.dc.html): id, nome, cidade e limite.
const LINHAS_DADOS: [(i64, &str, &str, Option<f64>); 5] = [
    (1, "Lucas Brandão", "Porto Alegre", Some(154.03)),
    (2, "Carla Ibiapina", "Recife", Some(14542.56)),
    (3, "Bruno Brandão", "Recife", Some(13741.03)),
    (4, "Olívia Lacerda", "Curitiba", Some(17051.0)),
    (7, "Gabriela Guimarães", "Rio de Janeiro", None),
];

/// Colunas da grade "Dados": a caixa de marcar, o número da linha e as de dados (nome, tipo, numérica).
const COLUNAS_DADOS: [(&str, &str, bool); 4] =
    [("id", "INTEGER", true), ("nome", "TEXT", false), ("cidade", "TEXT", false), ("limite", "REAL", true)];

fn nucleo_dados(id: &str) -> NucleoGrade {
    let larguras = vec![LARGURA_COLUNA_MARCAR, 40.0, 110.0, 200.0, 170.0, 130.0];
    let mut n = NucleoGrade::new(id, LINHAS_DADOS.len(), larguras)
        .com_navegacao(Navegacao::Dados)
        .com_navegaveis(2..6)
        .com_coluna_marcar(0)
        .com_coluna_numero(1);
    n.selecionar(Some((2, 4)));
    n
}

/// Os ganchos da grade "Dados" (a da aba Dados em miniatura: `td.selc`, `td.lin`, `td.chave`, `td.nulo`,
/// `tr.mar`).
struct DesenhoDados<'a> {
    p: &'static tema::Paleta,
    marcadas: &'a mut [bool; 5],
}

impl DesenhoGrade for DesenhoDados<'_> {
    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        let p = paleta(ui);
        let Some(&(id, nome, cidade, limite)) = LINHAS_DADOS.get(c.linha) else { return };
        let pintor = ui.painter().with_clip_rect(c.recorte);
        match c.coluna {
            0 => {
                let caixa = Rect::from_center_size(c.conteudo.center(), vec2(16.0, 16.0));
                let mut filho = ui.new_child(
                    UiBuilder::new()
                        .id_salt(("mostruario::dados::marca", c.linha))
                        .max_rect(caixa)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                if let Some(m) = self.marcadas.get_mut(c.linha) {
                    Check::new(m, "").mostrar(&mut filho);
                }
            }
            1 => pintar_numero_linha(&pintor, p, c.conteudo, &(c.linha + 1).to_string()),
            2 => {
                pintar_celula_chave(&pintor, p, c.conteudo, &Celula::Inteiro(id), true);
            }
            3 => {
                pintar_celula(&pintor, p, c.conteudo, &Celula::Texto(nome.into()), false);
            }
            4 => {
                pintar_celula(&pintor, p, c.conteudo, &Celula::Texto(cidade.into()), false);
            }
            _ => {
                pintar_celula(&pintor, p, c.conteudo, &limite.map_or(Celula::Nulo, Celula::Real), true);
            }
        }
    }

    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        let p = paleta(ui);
        match h.coluna {
            0 => {
                let todas = self.marcadas.iter().all(|&m| m);
                let algumas = self.marcadas.iter().any(|&m| m);
                let caixa = Rect::from_center_size(h.conteudo.center(), vec2(16.0, 16.0));
                let mut filho = ui.new_child(
                    UiBuilder::new()
                        .id_salt("mostruario::dados::marca_pagina")
                        .max_rect(caixa)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                let mut m = todas;
                if Check::new(&mut m, "").indeterminado(algumas && !todas).mostrar(&mut filho).changed() {
                    *self.marcadas = [m; 5];
                }
            }
            1 => {}
            c => {
                let Some(&(nome, tipo, numerica)) = COLUNAS_DADOS.get(c - 2) else { return };
                let ordem = (c == 2).then_some(Ordem::Crescente);
                let th = Th { tipo: Some(tipo), numerica, ordem, clicavel: true, ..Th::new(nome) };
                pintar_cabecalho_coluna(&ui.painter().with_clip_rect(h.recorte), p, h.conteudo, &th);
            }
        }
    }

    /// `tr.mar td{background:--sel}`
    fn fundo_linha(&mut self, linha: usize) -> Option<egui::Color32> {
        self.marcadas.get(linha).copied().unwrap_or(false).then_some(self.p.estudio.sel)
    }
}

impl Mostruario {
    pub fn new(tema: Tema) -> Self {
        Mostruario {
            tema,
            aba: Aba::Componentes,
            formato: Formato::Xlsx,
            nome: String::new(),
            email: "ana.exemplo@example.test".into(),
            perfil: 1,
            observacoes: String::new(),
            caminho: String::new(),
            recebe: true,
            marcadas: false,
            escopo: 0,
            grade: grade_exemplo(),
            grade_dados: nucleo_dados("mostruario-dados"),
            planilha: planilha::PlanilhaExemplo::new("mostruario-planilha"),
            marcadas_dados: [false, true, false, true, false],
            janela_aberta: false,
            nome_janela: String::new(),
            confirmacao: None,
            sql: SQL_EXEMPLO.into(),
            cidade: String::new(),
            usar_padrao: true,
            comando: COMANDO_EXEMPLO.into(),
            observacoes_tabela: ["referência da área; texto comprido".into(), String::new(), "saiu em agosto".into()],
            salvo: None,
            tipo_coluna: "TEXT".into(),
            casca: casca::EstadoCasca::new(),
            vista_casca: false,
            vista_codigo: false,
            vista_icones: false,
            vista_contexto: false,
            contexto: contexto::EstadoContexto::new(),
            editor_codigo: EDITOR_CODIGO.into(),
            editor_json: EDITOR_JSON.into(),
        }
    }

    /// Mostra a página do código ([`Mostruario::pagina_codigo`]) no lugar das peças.
    pub fn mostrar_codigo(&mut self, sim: bool) {
        self.vista_codigo = sim;
    }

    /// A janela mostra a página do código?
    pub fn codigo_a_vista(&self) -> bool {
        self.vista_codigo
    }

    /// A peça de código: as quatro cores do código (as cartas da prancheta 13, com o `TextoPre::sql` de cada uma) e
    /// o editor da Consulta SQL nas cores em vigor, com os números das linhas, a linha do comando com erro e o
    /// sublinhado ondulado no nome que a mensagem cita.
    pub fn pagina_codigo(&mut self, ui: &mut Ui) {
        let largura = ui.available_width().min(960.0);
        let topo = ui.cursor().min;
        let rect = Rect::from_min_size(pos2(topo.x + 40.0, topo.y + 30.0), vec2(largura - 80.0, f32::INFINITY));
        let mut ui =
            ui.new_child(UiBuilder::new().id_salt("codigo").max_rect(rect).layout(Layout::top_down(Align::Min)));
        crate::pecas::texto::h2(&mut ui, "Cores do código");
        ui.add_space(12.0);
        for par in crate::codigo::CoresCodigo::TODAS.chunks(2) {
            colunas_iguais(&mut ui, 2, 14.0, |ui, i| {
                let Some(&cores) = par.get(i) else { return };
                let mut cartao = Cartao::new(("cores", cores.como_str())).esticar().titulo(cores.rotulo());
                if !cores.nota().is_empty() {
                    cartao = cartao.hint(cores.nota());
                }
                cartao.mostrar(ui, |c| {
                    c.corpo(|ui| {
                        TextoPre::sql(AMOSTRA_CORES).cores(cores).id_salt(("amostra", cores.como_str())).mostrar(ui);
                    });
                });
            });
            ui.add_space(14.0);
        }
        let texto = &mut self.editor_codigo;
        let perigo = paleta(&ui).estudio.perigo.gamma_multiply(0.09);
        let nome = "tabela_que_nao_existe";
        let inicio = texto.find(nome).map_or(0, |b| texto[..b].chars().count());
        Cartao::new("editor-codigo").titulo("Editor").hint("Consulta SQL, com o comando 3 com erro").mostrar(
            &mut ui,
            |c| {
                c.corpo(|ui| {
                    AreaTexto::new(texto)
                        .id_salt("mostruario::editor_codigo")
                        .mono(true)
                        .codigo(true)
                        .numeros_de_linha(true)
                        .marcar_linhas(2..3, perigo)
                        .sublinhar(inicio..inicio + nome.chars().count())
                        .altura(150.0)
                        .mostrar(ui);
                });
            },
        );
        ui.add_space(14.0);
        let json = &mut self.editor_json;
        let nome = "\"tempo_limite\"";
        let inicio = json.find(nome).map_or(0, |b| json[..b].chars().count());
        Cartao::new("editor-json")
            .titulo("Editor JSON")
            .hint("a chave, o texto, o número e o literal; a vírgula que falta na linha 7")
            .mostrar(&mut ui, |c| {
                c.corpo(|ui| {
                    AreaTexto::new(json)
                        .id_salt("mostruario::editor_json")
                        .mono(true)
                        .codigo(true)
                        .linguagem(crate::codigo::Linguagem::Json)
                        .numeros_de_linha(true)
                        .marcar_linhas(6..7, perigo)
                        .sublinhar(inicio..inicio + nome.chars().count())
                        .erro_na_linha(6)
                        .altura(200.0)
                        .mostrar(ui);
                });
            });
    }

    /// Mostra a página dos ícones ([`Mostruario::pagina_icones`]) no lugar das peças.
    pub fn mostrar_icones(&mut self, sim: bool) {
        self.vista_icones = sim;
    }

    /// A janela mostra a página dos ícones?
    pub fn icones_a_vista(&self) -> bool {
        self.vista_icones
    }

    /// Mostra a casca nova ([`Mostruario::pagina_casca`]) no lugar das peças, na janela de [`Mostruario::ui`].
    pub fn mostrar_casca(&mut self, sim: bool) {
        self.vista_casca = sim;
    }

    /// A janela mostra a casca (e não as peças)?
    pub fn casca_a_vista(&self) -> bool {
        self.vista_casca
    }

    /// O tema escolhido no seletor do mostruário.
    pub fn tema(&self) -> Tema {
        self.tema
    }

    /// Abre a janela de exemplo (o teste usa para capturar a janela).
    pub fn abrir_janela(&mut self) {
        self.janela_aberta = true;
    }

    /// Abre a confirmação de exemplo.
    pub fn abrir_confirmacao(&mut self) {
        self.confirmacao = Some(
            Confirmacao::new("Excluir as 3 linhas marcadas de clientes? Não há como desfazer.")
                .titulo("Excluir linhas")
                .rotulo("Excluir")
                .perigo(true),
        );
    }

    /// Quadro completo para uma janela: aplica o tema, pinta o fundo, rola a página e desenha a janela e o
    /// toast por cima.
    pub fn ui(&mut self, ui: &mut Ui) {
        tema::aplicar(ui.ctx(), self.tema);
        if !fontes::instaladas(ui.ctx()) {
            ui.ctx().request_repaint();
            return;
        }
        let tela = ui.ctx().content_rect();
        pintar_fundo(ui.painter(), paleta(ui), tela);
        if self.vista_casca {
            let mut pagina = ui.new_child(UiBuilder::new().id_salt("databox::mostruario::casca").max_rect(tela));
            self.pagina_casca(&mut pagina);
        } else if self.vista_codigo {
            let mut pagina = ui.new_child(UiBuilder::new().id_salt("databox::mostruario::codigo").max_rect(tela));
            self.pagina_codigo(&mut pagina);
        } else if self.vista_contexto {
            let mut pagina = ui.new_child(UiBuilder::new().id_salt("databox::mostruario::contexto").max_rect(tela));
            self.pagina_contexto(&mut pagina);
        } else {
            ScrollArea::vertical().id_salt("databox::mostruario").auto_shrink([false, false]).show(ui, |ui| {
                self.pagina(ui);
            });
        }
        self.sobreposicoes(ui);
    }

    /// Só a página, sem rolagem nem fundo (o teste desenha numa janela alta o bastante).
    pub fn pagina(&mut self, ui: &mut Ui) {
        let largura = ui.available_width();
        let topo = ui.cursor().min;
        // em cima, a casca nova: a barra de título com os menus e a barra de ferramentas (`margin:6px 8px 0`)
        self.barra_de_titulo(ui, Rect::from_min_size(topo, vec2(largura, ALTURA_TITULO)));
        let barra =
            Rect::from_min_size(pos2(topo.x + 8.0, topo.y + ALTURA_TITULO + 6.0), vec2(largura - 16.0, ALTURA_BARRA));
        let mut ui_barra = ui.new_child(UiBuilder::new().id_salt("ferramentas").max_rect(barra));
        self.ferramentas(&mut ui_barra);
        let max_total = 1520.0_f32.min(largura);
        let esquerda = topo.x + ((largura - max_total) / 2.0).max(0.0) + 20.0;
        let y = barra.bottom() + 20.0;
        let largura_conteudo = (max_total - 40.0 - LARGURA_EXPLORADOR - 20.0).max(200.0);

        // o Explorador à esquerda (o mesmo da casca)
        let rect_explorador = Rect::from_min_size(pos2(esquerda, y), vec2(LARGURA_EXPLORADOR, ALTURA_EXPLORADOR));
        let mut ui_explorador = ui.new_child(UiBuilder::new().id_salt("explorador").max_rect(rect_explorador));
        self.explorador(&mut ui_explorador);

        // conteúdo
        let rect_conteudo =
            Rect::from_min_size(pos2(esquerda + LARGURA_EXPLORADOR + 20.0, y), vec2(largura_conteudo, f32::INFINITY));
        let mut ui_conteudo = ui.new_child(
            UiBuilder::new().id_salt("conteudo").max_rect(rect_conteudo).layout(Layout::top_down(Align::Min)),
        );
        ui_conteudo.spacing_mut().item_spacing.y = 0.0;
        self.conteudo(&mut ui_conteudo);
        let fim = ui_conteudo.min_rect().bottom().max(rect_explorador.bottom()) + 60.0;
        ui.allocate_rect(Rect::from_min_max(topo, pos2(topo.x + largura, fim)), Sense::hover());
    }
    /// A janela, a confirmação e o toast (por cima da página).
    pub fn sobreposicoes(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.janela_aberta {
            let mut criar = false;
            let nome = &mut self.nome_janela;
            let resp = Janela::new("nova-pessoa", "Nova tabela")
                .icone(Icone::Tabela)
                .sub("exemplo.db")
                .larga(false)
                .rodape_esquerda(|ui| {
                    ui.add(Botao::ghost_perigo("Descartar"));
                })
                .rodape_direita(|ui| {
                    if ui.add(Botao::primario("Criar tabela")).clicked() {
                        criar = true;
                    }
                })
                .mostrar(&ctx, |ui| {
                    campos_duas_colunas(ui, |c| {
                        c.largo(|ui| {
                            campo(ui, "Nome da tabela", Some("Letras, números e _; sem espaço."), |ui| {
                                ui.add(Entrada::new(nome).dica("nome_da_tabela"));
                            });
                        });
                        c.par(
                            |ui| {
                                campo(ui, "Tipo", None, |ui| {
                                    let mut t = 0_u8;
                                    Select::new("tipo-janela", &mut t, &[(0, "INTEGER"), (1, "TEXT"), (2, "REAL")])
                                        .mostrar(ui);
                                });
                            },
                            |ui| {
                                campo(ui, "Padrão", None, |ui| {
                                    let mut s = String::new();
                                    // o `input.mono` da janela de esquema troca de fonte de verdade
                                    ui.add(Entrada::new(&mut s).monoespacada(true).dica("CURRENT_TIMESTAMP"));
                                });
                            },
                        );
                    });
                    h4(ui, "SQL que será executado");
                    TextoPre::sql("CREATE TABLE \"fornecedores\" (\n  \"id\" INTEGER PRIMARY KEY\n);").mostrar(ui);
                    aviso_tom(
                        ui,
                        &[Trecho::Normal("A tabela nasce vazia; inclua as linhas na aba Dados.")],
                        TomAviso::Informacao,
                    );
                });
            if resp.pediu_fechar || criar {
                self.janela_aberta = false;
                if criar {
                    toast_concluido(&ctx, "Tabela criada.");
                }
            }
        }
        if let Some(sim) = confirmar(&ctx, &mut self.confirmacao) {
            if sim {
                toast_concluido(&ctx, "Linhas excluídas.");
            } else {
                toast(&ctx, "Nada foi excluído.");
            }
        }
        desenhar_toast(&ctx, f32::INFINITY);
    }

    fn conteudo(&mut self, ui: &mut Ui) {
        // avisos no alto
        Cartao::new("avisos").mostrar(ui, |c| {
            c.corpo_avisos(|ui| {
                aviso(
                    ui,
                    &[
                        Trecho::Normal("Aviso âmbar (.aviso): faixa com o sólido de atenção "),
                        Trecho::Forte("dentro de um cartão"),
                        Trecho::Normal(", nunca solta na página."),
                    ],
                    false,
                );
                aviso(
                    ui,
                    &[
                        Trecho::Normal("Aviso de erro: "),
                        Trecho::Forte("não foi possível salvar."),
                        Trecho::Normal(" Confira os campos marcados."),
                    ],
                    true,
                );
                aviso_tom(
                    ui,
                    &[
                        Trecho::Normal("Concluído: "),
                        Trecho::Forte("arquivo salvo"),
                        Trecho::Normal(" · 60 linhas · 12 KB"),
                    ],
                    TomAviso::Concluido,
                );
                aviso_tom(
                    ui,
                    &[Trecho::Normal("Informação: a prévia do que vai acontecer, sem alarme.")],
                    TomAviso::Informacao,
                );
                aviso_tom(
                    ui,
                    &[Trecho::Normal("exemplo.db está aberto só para leitura. Nada é gravado no arquivo.")],
                    TomAviso::Nota,
                );
            });
        });
        ui.add_space(16.0);

        // cabeçalho da tela
        let mut tema = self.tema;
        let passos = [Passo { rotulo: "Design system", link: true }, Passo { rotulo: "Mostruário", link: false }];
        Cabecalho::new("mostruario", "Mostruário")
            .trilha(&passos)
            .ajuda("Todas as peças do visual Estúdio numa tela só, com dados inventados. O texto explicativo da tela mora aqui, no balão, e não sob o título.")
            .sub("C:\\dados\\vendas.db · 1,2 MB")
            .apos(|ui| {
                pill(ui, "só leitura", TipoPill::Wait);
            })
            .acoes(|ui| {
                if segmentado(ui, "tema", &[(Tema::Sistema, "Sistema"), (Tema::Claro, "Claro"), (Tema::Escuro, "Escuro")], &mut tema, TamanhoTrilho::Compacto) {
                    tema::aplicar(ui.ctx(), tema);
                }
                ui.add(Botao::new("Ajuda"));
                ui.add(Botao::ouro("Exportar o banco"));
            })
            .mostrar(ui);
        self.tema = tema;
        if let Some(a) = abas(
            ui,
            "abas",
            &[(Aba::Componentes, "Componentes"), (Aba::Padroes, "Padrões"), (Aba::Decisoes, "Decisões")],
            self.aba,
        ) {
            self.aba = a;
        }
        ui.add_space(16.0);

        // cores e tipografia
        colunas_iguais(ui, 2, 14.0, |ui, i| if i == 0 { self.card_cores(ui) } else { self.card_tipografia(ui) });
        ui.add_space(16.0);
        colunas_iguais(ui, 2, 14.0, |ui, i| if i == 0 { self.card_botoes(ui) } else { self.card_campos(ui) });
        ui.add_space(16.0);
        self.card_estados(ui);
        ui.add_space(16.0);
        self.card_casca(ui);
        ui.add_space(16.0);
        self.card_grade(ui);
        ui.add_space(16.0);
        self.card_dados(ui);
        ui.add_space(16.0);
        self.card_planilha(ui);
        ui.add_space(16.0);
        self.card_cadastrados(ui);
        ui.add_space(16.0);
        colunas_iguais(ui, 2, 14.0, |ui, i| if i == 0 { self.card_textos(ui) } else { self.card_gatilhos(ui) });
        ui.add_space(16.0);
        self.card_vazio(ui);
        ui.add_space(16.0);
        colunas_iguais(ui, 2, 14.0, |ui, i| if i == 0 { self.card_tiles(ui) } else { self.card_arquivo(ui) });
        ui.add_space(16.0);
        colunas_iguais(ui, 2, 14.0, |ui, i| if i == 0 { self.card_sql(ui) } else { self.card_comando(ui) });
        ui.add_space(16.0);
        em_linha(ui, 8.0, |ui| {
            if ui.add(Botao::new("Mostrar toast")).clicked() {
                toast_concluido(ui.ctx(), "Salvo. Este é o toast.");
            }
            if ui.add(Botao::new("Abrir janela")).clicked() {
                self.abrir_janela();
            }
            if ui.add(Botao::ghost_perigo("Excluir marcadas")).clicked() {
                self.abrir_confirmacao();
            }
        });
    }

    fn card_cores(&mut self, ui: &mut Ui) {
        Cartao::new("cores").esticar().titulo("Cores").hint("tokens das pranchetas").mostrar(ui, |c| {
            c.corpo(|ui| {
                let p = paleta(ui);
                let e = &p.estudio;
                let cores: [(&str, egui::Color32); 9] = [
                    ("destaque · #2C6FDC", e.acc),
                    ("texto no destaque", e.acct),
                    ("fundo", p.bg),
                    ("concluído", e.ok),
                    ("aviso", e.aviso),
                    ("perigo", e.perigo),
                    ("volume neutro", e.sup_meio),
                    ("painel", e.painel),
                    ("documento", e.doc),
                ];
                let largura = ui.available_width();
                let colunas = ((largura + 8.0) / 128.0).floor().max(1.0) as usize;
                let w = (largura - 8.0 * (colunas as f32 - 1.0)) / colunas as f32;
                let topo = ui.cursor().min;
                let linhas = cores.len().div_ceil(colunas);
                let altura_item = 10.0 + 28.0 + 6.0 + 16.5 + 10.0;
                ui.allocate_rect(
                    Rect::from_min_size(topo, vec2(largura, linhas as f32 * (altura_item + 8.0) - 8.0)),
                    Sense::hover(),
                );
                for (i, (nome, cor)) in cores.iter().enumerate() {
                    let r = Rect::from_min_size(
                        pos2(
                            topo.x + (i % colunas) as f32 * (w + 8.0),
                            topo.y + (i / colunas) as f32 * (altura_item + 8.0),
                        ),
                        vec2(w, altura_item),
                    );
                    crate::pintura::anel_rebaixado(ui.painter(), r, RAIO_PADRAO, p.track_ring);
                    let amostra = Rect::from_min_size(r.min + vec2(12.0, 10.0), vec2(w - 24.0, 28.0));
                    ui.painter().rect_filled(amostra, 6.0, *cor);
                    crate::pintura::anel_rebaixado(ui.painter(), amostra, 6.0, p.track_ring);
                    let g = galley(ui, nome, fontes::fonte(11.0, Peso::Normal), p.fg);
                    ui.painter().galley(pos2(r.left() + 12.0, amostra.bottom() + 6.0), g, p.fg);
                }
            });
        });
    }

    fn card_tipografia(&mut self, ui: &mut Ui) {
        Cartao::new("tipografia").esticar().titulo("Tipografia").hint("Segoe UI · corpo 12px").mostrar(ui, |c| {
            c.corpo(|ui| {
                let p = paleta(ui);
                ui.spacing_mut().item_spacing.y = 0.0;
                let item = |ui: &mut Ui, rotulo: &str, desenho: &mut dyn FnMut(&mut Ui)| {
                    let topo = ui.cursor().min;
                    let largura = ui.available_width();
                    linha(ui, 34.0, 16.0, |ui| {
                        ui.allocate_ui_with_layout(vec2(220.0, 34.0), Layout::left_to_right(Align::Center), |ui| {
                            hint(ui, rotulo);
                        });
                        desenho(ui);
                    });
                    let y = topo.y + 34.0;
                    ui.painter().hline(topo.x..=topo.x + largura, y + 0.5, egui::Stroke::new(1.0, p.hairline));
                    ui.add_space(1.0);
                };
                item(ui, "h2 · título de página · 20/600", &mut |ui| {
                    crate::pecas::texto::h2(ui, "Visão geral");
                });
                item(ui, "h3 · título de janela · 14/600", &mut |ui| {
                    crate::pecas::texto::h3(ui, "Exportar clientes");
                });
                item(ui, ".tit · título de seção · 11/600 caps", &mut |ui| {
                    crate::pecas::texto::eyebrow(ui, "Formato");
                });
                item(ui, "corpo · 12/400", &mut |ui| {
                    let p = paleta(ui);
                    texto(ui, "O texto corrido da interface, sem jargão.", 12.0, p.fg);
                });
                item(ui, "strong · 700", &mut |ui| {
                    forte(ui, "Ana Exemplo");
                });
                item(ui, "hint · 11.5 t3", &mut |ui| {
                    hint(ui, "texto auxiliar: 7 tabelas");
                });
                item(ui, "número da Visão geral · 24/350", &mut |ui| {
                    let p = paleta(ui);
                    let g = galley(ui, &formatar_numero(1284), fontes::fonte(24.0, Peso::Leve), p.fg);
                    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                    ui.painter().galley(r.min, g, p.fg);
                });
                item(ui, "link de texto (.link)", &mut |ui| {
                    link(ui, "acct em 600, t no hover");
                });
                item(ui, "code · interface 350", &mut |ui| {
                    codigo(ui, "SELECT * FROM clientes LIMIT 100");
                });
            });
        });
    }

    fn card_botoes(&mut self, ui: &mut Ui) {
        Cartao::new("botoes").esticar().titulo("Botões").hint("pílula de 28px · 30px nas janelas").mostrar(ui, |c| {
            c.corpo(|ui| {
                fluxo(ui, 8.0, |ui| {
                    ui.add(Botao::new("Padrão (.btn)"));
                    ui.add(Botao::primario("Salvar (.pri)"));
                    ui.add(Botao::ghost("Cancelar (.sutil)"));
                    ui.add(Botao::perigo("Excluir (.risco)"));
                    ui.add(Botao::ghost_perigo("Remover (.sutil.perigo)"));
                    ui.add(Botao::new("Desabilitado").habilitado(false));
                    ui.add(Botao::ouro("Gerando arquivo").ocupado(true));
                });
                fluxo(ui, 8.0, |ui| {
                    // `.ib`: transparente até o hover; ligado (`.ib.on`) com o volume neutro
                    ui.add(Botao::redondo("‹").rotulo("Voltar"));
                    ui.add(Botao::redondo("›").rotulo("Avançar").habilitado(false));
                    ui.add(Botao::redondo("+").rotulo("Nova consulta").selecionado(true));
                    ui.add(Botao::redondo("+").rotulo("Nova linha").tipo(TipoBotao::Primario));
                    ui.add(Botao::fechar().rotulo("Fechar o exemplo"));
                    ui.add(Botao::ghost("✕").estreito(true).rotulo("Remover este filtro"));
                    com_altura(ui, ALTURA_JANELA, |ui| {
                        ui.add(Botao::ghost("Cancelar"));
                        ui.add(Botao::primario("Exportar (janela, 30px)"));
                    });
                });
                ui.add_space(4.0);
                rotulo_caps(ui, "Seletor segmentado (.seg) · partes iguais, de ponta a ponta no formulário", 0.12);
                let mut f = self.formato;
                segmentado(
                    ui,
                    "formato",
                    &[(Formato::Xlsx, "Excel (.xlsx)"), (Formato::Csv, "CSV"), (Formato::Json, "JSON")],
                    &mut f,
                    TamanhoTrilho::Cheio,
                );
                self.formato = f;
                ui.add_space(4.0);
                rotulo_caps(ui, "Seletor de resultados (.seg role=tablist) · a nota e o comando com erro", 0.12);
                {
                    use crate::pecas::navegacao::{ParteSeletor, seletor};
                    let partes = [
                        ParteSeletor::new("Comando 1").nota("8 linhas"),
                        ParteSeletor::new("Comando 2").nota("3 linhas alteradas"),
                        ParteSeletor::new("Comando 3").nota("erro").perigo(true),
                    ];
                    let id = ui.id().with("seletor-resultados");
                    let ativo = ui.ctx().data(|d| d.get_temp::<usize>(id)).unwrap_or(0);
                    if let Some(i) = seletor(ui, "mostruario-seletor", "Resultados", &partes, ativo) {
                        ui.ctx().data_mut(|d| d.insert_temp(id, i));
                    }
                }
                ui.add_space(4.0);
                rotulo_caps(ui, "Progresso · com total, sem total e carregando", 0.12);
                barra_progresso(ui, Some(0.71));
                crate::pecas::texto::texto(ui, "12.340 de 1.000.000 linhas · 1%", 12.0, paleta(ui).muted);
                barra_progresso(ui, None);
                em_linha(ui, 16.0, |ui| {
                    carregando(ui, "contando…");
                    carregando(ui, "Carregando…");
                });
            });
        });
    }

    fn card_campos(&mut self, ui: &mut Ui) {
        let nome = &mut self.nome;
        let email = &mut self.email;
        let perfil = &mut self.perfil;
        let obs = &mut self.observacoes;
        let caminho = &mut self.caminho;
        let recebe = &mut self.recebe;
        let marcadas = &mut self.marcadas;
        let escopo = &mut self.escopo;
        let cidade = &mut self.cidade;
        let usar_padrao = &mut self.usar_padrao;
        Cartao::new("campos")
            .esticar()
            .titulo("Campos")
            .hint("raio 10 · caixas quadradas · rótulo em caps; explicação no \"i\"")
            .acoes(|ui| {
                ui.add(Botao::primario("Salvar"));
            })
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    campos_duas_colunas(ui, |cs| {
                        cs.par(
                            |ui| {
                                campo(ui, "Nome", None, |ui| {
                                    ui.add(Entrada::new(nome).dica("Nome completo"));
                                });
                            },
                            |ui| {
                                campo(ui, "E-mail", None, |ui| {
                                    ui.add(Entrada::new(email).dica("nome@example.test"));
                                });
                            },
                        );
                        cs.par(
                            |ui| {
                                campo(
                                    ui,
                                    "Perfil",
                                    Some("gestor: só acessa o que receber. rh: cria e administra."),
                                    |ui| {
                                        Select::new("perfil", perfil, &[(0, "gestor"), (1, "rh"), (2, "admin")])
                                            .mostrar(ui);
                                    },
                                );
                            },
                            |ui| {
                                // `input.mono` fora da janela de esquema: Avenir (o `.mono` do kit só liga os
                                // algarismos tabulares, que a Avenir já tem)
                                campo(ui, "Caminho", None, |ui| {
                                    ui.add(Entrada::new(caminho).dica("C:\\pasta\\arquivo.db"));
                                });
                            },
                        );
                        let cidade_livre = !*usar_padrao;
                        cs.par(
                            |ui| {
                                campo(ui, "Cidade", Some("Desligado enquanto o padrão estiver marcado."), |ui| {
                                    ui.add(Entrada::new(cidade).dica("padrão: 'São Paulo'").habilitado(cidade_livre));
                                });
                            },
                            |ui| {
                                campo(ui, "Padrão", None, |ui| {
                                    em_linha(ui, 0.0, |ui| {
                                        check(ui, usar_padrao, "Usar o padrão");
                                    });
                                });
                            },
                        );
                        cs.largo(|ui| {
                            campo(ui, "Observações", None, |ui| {
                                ui.add(AreaTexto::new(obs).dica("Texto livre"));
                            });
                        });
                        cs.largo(|ui| {
                            fluxo(ui, 16.0, |ui| {
                                check(ui, recebe, "Recebe envio do candidato");
                                Check::new(marcadas, "Marcar a página").indeterminado(true).mostrar(ui);
                            });
                        });
                        cs.largo(|ui| {
                            ui.spacing_mut().item_spacing.y = 8.0;
                            if radio(ui, *escopo == 0, &format!("A tabela inteira ({})", plural(60, "linha", "linhas")))
                                .clicked()
                            {
                                *escopo = 0;
                            }
                            if radio(ui, *escopo == 1, "Só as linhas marcadas (3)").clicked() {
                                *escopo = 1;
                            }
                        });
                    });
                });
            });
    }

    fn card_estados(&mut self, ui: &mut Ui) {
        Cartao::new("estados")
            .titulo("Estados, chips e situação")
            .hint(".pill para estado · .tag para coisas · .chip para filtro e arquivo")
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    fluxo(ui, 8.0, |ui| {
                        pill(ui, "neutro", TipoPill::Neutra);
                        pill(ui, "verificadas", TipoPill::Ok);
                        pill(ui, "inativo", TipoPill::No);
                        pill(ui, "só leitura", TipoPill::Wait);
                        pill(ui, "chave primária", TipoPill::Acc);
                        pill(ui, "sem rowid", TipoPill::Mute);
                    });
                    fluxo(ui, 8.0, |ui| {
                        tag(ui, "→ clientes.id");
                        tag_clicavel(ui, "pedidos", TipoTag::Ouro);
                        tag_clicavel(ui, "+ vincular", TipoTag::Tracejada);
                        chip_filtro(ui, &[("ativo", true), ("igual a", false), ("1", true)], "Tirar o filtro");
                        chip(ui, &format!("BLOB · {}", formatar_bytes(2355)), false);
                        chip(ui, "sem material", true);
                        codigo(ui, "INTEGER");
                        nome_abre(ui, "Ana Exemplo", false);
                        nome_abre(ui, "Carla Inventada", true);
                        Ajuda::new(&[
                            Trecho::Normal("Salva sozinho, sem botão: "),
                            Trecho::Forte("Enter"),
                            Trecho::Normal(" grava, Esc desiste."),
                        ])
                        .mini(true)
                        .mostrar(ui);
                    });
                    // o atalho de SQL das Propriedades (`a.atalho`), numa coluna de 240px
                    ui.scope(|ui| {
                        ui.set_max_width(240.0);
                        crate::pecas::texto::atalho(ui, "SELECT * FROM clientes LIMIT 100");
                    });
                    // os sólidos de estado (Icones.dc.html), em 16 e 14px
                    fluxo(ui, 8.0, |ui| {
                        for lado in [16.0, 14.0] {
                            for icone in IconeEstado::TODOS {
                                let (r, _) = ui.allocate_exact_size(vec2(lado, lado), Sense::hover());
                                pintar_icone_estado(ui.painter(), r, icone);
                            }
                        }
                    });
                });
            });
    }

    /// Os ícones do app (os PNG em 16 e 24px, as pastilhas e as folhas com o texto por cima em 16, 24 e 32) e os
    /// botões da casca: o `.ib` com ícone, ligado, principal e ocupado (o arco), o `.ib.pq`, o `.rail` desligado e
    /// ligado (com a barrinha), o botão com ícone, o `.btn.lista`, o "+N", o separador e a busca da barra. "Ver a
    /// casca" troca a página pela janela inteira ([`Mostruario::pagina_casca`]).
    fn card_casca(&mut self, ui: &mut Ui) {
        let mut ver = false;
        Cartao::new("casca")
            .titulo("Ícones e botões da casca")
            .hint("silhuetas e sólidos em PNG, traços no pintor · .ib, .ib.pq, .rail, .btn .sil e .btn.lista")
            .acoes(|ui| {
                ver = ui.add(Botao::new("Ver a casca")).clicked();
            })
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    grade_de_icones(ui, &[16.0, 24.0]);
                    // as pastilhas e as folhas com o texto por cima, nos três desenhos
                    for lado in [16.0, 24.0, 32.0] {
                        fluxo(ui, 8.0, |ui| {
                            for icone in Icone::TODOS {
                                if matches!(icone, Icone::Tipo(_) | Icone::Folha(_)) {
                                    let (r, resp) = ui.allocate_exact_size(vec2(lado, lado), Sense::hover());
                                    icones::pintar(ui.painter(), r, icone, Color32::WHITE);
                                    resp.on_hover_text(icone.nome());
                                }
                            }
                        });
                    }
                    fluxo(ui, 8.0, |ui| {
                        ui.add(Botao::icone(Icone::Salvar).dica("Salvar (Ctrl+S)"));
                        ui.add(Botao::icone(Icone::Novo).rotulo("Nova tabela").selecionado(true));
                        ui.add(Botao::icone(Icone::Executar).principal().dica("Executar (Ctrl+Enter)"));
                        ui.add(Botao::icone(Icone::Executar).principal().ocupado(true).dica("Executando"));
                        ui.add(Botao::icone(Icone::Interromper).tipo(TipoBotao::Perigo).dica("Interromper (Esc)"));
                        ui.add(Botao::icone(Icone::Excluir).habilitado(false));
                        ui.add(Botao::icone(Icone::Mais).pequeno().rotulo("Nova aba"));
                        ui.add(Botao::icone(Icone::CopiarCaminho).pequeno().dica("Copiar caminho"));
                        separador(ui);
                        ui.add(Botao::icone(Icone::Inicio).rail());
                        ui.add_space(6.0);
                        ui.add(Botao::icone(Icone::Explorador).rail().selecionado(true));
                        ui.add_space(6.0);
                        ui.add(Botao::icone(Icone::Historico).rail());
                        separador(ui);
                        busca_desligada(ui);
                    });
                    fluxo(ui, 8.0, |ui| {
                        ui.add(Botao::new("Exportar").com_icone(Icone::Exportar));
                        ui.add(Botao::ghost("Mostrar na pasta").com_icone(Icone::MostrarNaPasta));
                        ui.add(Botao::new("Novo banco").com_icone(Icone::NovoBanco).pequeno());
                        etiqueta_mais(ui, 3, "UNIQUE · CHECK (length(nome) > 1) · COLLATE NOCASE");
                    });
                    // a Manutenção das Propriedades: `.btn.sutil.lista` na largura do painel
                    ui.allocate_ui(vec2(260.0, 5.0 * 28.0), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (icone, texto) in [
                            (Icone::VerificarIntegridade, "Verificar integridade"),
                            (Icone::VerificarChaves, "Verificar chaves estrangeiras"),
                            (Icone::Otimizar, "Otimizar"),
                            (Icone::Compactar, "Compactar (VACUUM)"),
                            (Icone::CopiaSeguranca, "Fazer cópia de segurança"),
                        ] {
                            ui.add(Botao::ghost(texto).com_icone(icone).lista());
                        }
                    });
                });
            });
        if ver {
            self.vista_casca = true;
        }
    }

    /// A página só dos ícones, para as capturas de nitidez (a 100%, 150% e 200%): todos os [`Icone::TODOS`] em 12,
    /// 14, 16, 20, 22, 24 e 32 pt, as pastilhas e as folhas com o texto, e os botões que levam ícone.
    pub fn pagina_icones(&mut self, ui: &mut Ui) {
        egui::Frame::NONE.inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
            h4(ui, "Ícones: o PNG pelos pixels, o desenho pelos pontos (12, 14, 16, 20, 22, 24 e 32)");
            grade_de_icones(ui, &[12.0, 14.0, 16.0, 20.0, 22.0, 24.0, 32.0]);
        });
        self.card_casca(ui);
    }

    fn card_grade(&mut self, ui: &mut Ui) {
        let n = self.grade.linhas().len();
        let hint_texto = format!("{} · {}", plural(n, "linha", "linhas"), crate::formato::formatar_ms(3));
        let grade = &mut self.grade;
        Cartao::new("grade")
            .titulo("Resultado 1")
            .dica_titulo("SELECT * FROM clientes")
            .hint(&hint_texto)
            .acoes(|ui| {
                ui.add(Botao::ouro("Exportar"));
            })
            .mostrar(ui, |c| {
                c.corpo_flush(|ui| {
                    grade.mostrar(ui, 360.0);
                });
                c.rodape_dividido(
                    |ui| {
                        let p = paleta(ui);
                        crate::pecas::texto::texto(ui, "Linhas 1–8 de 8", 11.0, p.muted);
                    },
                    |ui| {
                        ui.add(Botao::redondo("‹").rotulo("Anterior").habilitado(false));
                        let p = paleta(ui);
                        crate::pecas::texto::texto(ui, "Página 1 de 1", 11.0, p.muted);
                        ui.add(Botao::redondo("›").rotulo("Próxima").habilitado(false));
                    },
                );
            });
    }

    /// A grade da aba Dados em miniatura, no [`NucleoGrade`]: a coluna da caixa de marcar (`td.selc`), a do número
    /// da linha (`td.lin`), a chave (`td.chave`), o NULL, as linhas marcadas (`tr.mar`) e a célula ativa.
    fn card_dados(&mut self, ui: &mut Ui) {
        let n_marcadas = self.marcadas_dados.iter().filter(|&&m| m).count();
        let hint_texto = format!(
            "{} · {}",
            plural(LINHAS_DADOS.len(), "linha", "linhas"),
            plural(n_marcadas, "marcada", "marcadas")
        );
        let nucleo = &mut self.grade_dados;
        let marcadas = &mut self.marcadas_dados;
        Cartao::new("dados").titulo("clientes").hint(&hint_texto).mostrar(ui, |c| {
            c.corpo_flush(|ui| {
                let mut desenho = DesenhoDados { p: paleta(ui), marcadas };
                nucleo.mostrar(ui, 240.0, &mut desenho);
            });
        });
    }

    /// A planilha do Excel em miniatura (prancheta 17, cena a): letras de coluna, números da linha do Excel, larguras do
    /// arquivo, o título A1:F1 mesclado, a linha 1 e a coluna A congeladas (o fio de 2px `--b2` na emenda), números à
    /// direita e o erro em `--perigo`. O clique no número escolhe a linha.
    fn card_planilha(&mut self, ui: &mut Ui) {
        let planilha = &mut self.planilha;
        let hint_texto = format!("{} · 1 linha e 1 coluna congeladas", plural(planilha::LINHAS, "linha", "linhas"));
        Cartao::new("planilha").titulo("relatorio_q3.xlsx · Vendas").hint(&hint_texto).mostrar(ui, |c| {
            c.corpo_flush(|ui| {
                planilha.mostrar(ui, 300.0);
                planilha.mostrar_folhas(ui, false);
            });
        });
    }

    /// A tabela simples ("Cadastrados" no mostruário do kit), com o `td.obs` que salva sozinho e pisca e a linha
    /// marcada (`tr.sel`).
    fn card_cadastrados(&mut self, ui: &mut Ui) {
        let colunas = [
            ColunaTabela::new("Nome"),
            ColunaTabela::new("E-mail"),
            ColunaTabela::new("Perfil"),
            ColunaTabela::new("Situação"),
            ColunaTabela::new("Observação").estica(),
            ColunaTabela::new("Último acesso"),
        ];
        let observacoes = &mut self.observacoes_tabela;
        let salvo = &mut self.salvo;
        Cartao::new("cadastrados")
            .titulo("Cadastrados")
            .hint("3 pessoas · clique no nome para ver e alterar")
            .acoes(|ui| {
                ui.add(Botao::new("+ Nova pessoa"));
            })
            .mostrar(ui, |c| {
                c.corpo_flush(|ui| {
                    Tabela::new("cadastrados", &colunas, PESSOAS.len())
                        .inativas(|l| l == 2)
                        .marcadas(|l| l == 1)
                        .mostrar(ui, |ui, l, col| {
                            let p = paleta(ui);
                            let Some(&(nome, email, perfil, situacao, acesso)) = PESSOAS.get(l) else { return };
                            match col {
                                0 => {
                                    if nome_abre(ui, nome, l == 2).clicked() {
                                        toast(ui.ctx(), format!("Abriria a janela de {nome}."));
                                    }
                                    if l == 0 {
                                        hint(ui, "(você)");
                                    }
                                }
                                // `td.hint`: 12px `--k-muted`
                                1 => {
                                    texto(ui, email, 12.0, p.muted);
                                }
                                2 => {
                                    pill(ui, perfil.0, perfil.1);
                                }
                                3 => {
                                    pill(ui, situacao.0, situacao.1);
                                }
                                4 => {
                                    // `td.obs`: o campo só ganha moldura no hover e no foco; Enter salva e a célula pisca
                                    if let Some((linha, desde)) = *salvo
                                        && linha == l
                                    {
                                        pintar_salvo(
                                            ui.painter(),
                                            p,
                                            ui.max_rect().expand2(vec2(6.0, 4.0)),
                                            7.0,
                                            desde,
                                        );
                                    }
                                    if let Some(obs) = observacoes.get_mut(l) {
                                        let saida = Entrada::new(obs)
                                            .compacta()
                                            .id_salt(("databox::mostruario::obs", l))
                                            .dica("—")
                                            .rotulo("Observação")
                                            .mostrar(ui);
                                        if enter_no_campo(ui, &saida.response.response) {
                                            *salvo = Some((l, ui.input(|i| i.time)));
                                        }
                                    }
                                }
                                _ => {
                                    texto(ui, acesso, 12.0, p.muted);
                                }
                            }
                        });
                });
                c.rodape(|ui| {
                    hint_tamanho(ui, "Ordem alfabética ·", 11.0);
                    link_tamanho(ui, "exportar em CSV", 11.0);
                });
            });
    }

    /// Avisos com link e com o "i" mini, campo com sugestões.
    fn card_textos(&mut self, ui: &mut Ui) {
        let tipo = &mut self.tipo_coluna;
        Cartao::new("textos").esticar().titulo("Avisos e campos com sugestões").mostrar(ui, |c| {
            c.corpo(|ui| {
                let r = aviso(
                    ui,
                    &[
                        Trecho::Forte("Há uma transação aberta na Consulta SQL."),
                        Trecho::Normal(" O que for salvo agora só vai para o arquivo com COMMIT. "),
                        Trecho::Link("Ir para a Consulta SQL"),
                    ],
                    false,
                );
                if r.link.is_some() {
                    toast(ui.ctx(), "Iria para a Consulta SQL.");
                }
                aviso(
                    ui,
                    &[
                        Trecho::Forte("A tabela clientes_x não existe."),
                        Trecho::Normal(" "),
                        Trecho::Ajuda(&[
                            Trecho::Forte("Mensagem original do SQLite: "),
                            Trecho::Codigo("no such table: clientes_x"),
                        ]),
                        Trecho::Quebra,
                        Trecho::Codigo("SELECT * FROM clientes_x"),
                    ],
                    true,
                );
                campo(ui, "Tipo", Some("Os tipos mais usados; qualquer outro também vale."), |ui| {
                    // a pastilha segue o texto
                    let pastilha = TipoColuna::do_declarado(tipo, false, false);
                    EntradaSugestoes::new("tipo", tipo, &TIPOS_SUGERIDOS)
                        .pastilha(pastilha)
                        .rotulo("Tipo")
                        .largura(220.0)
                        .mostrar(ui);
                });
                campo(ui, "Caminho (só leitura)", None, |ui| {
                    let mut caminho = "C:\\dados\\exemplo.db".to_owned();
                    ui.add(Entrada::new(&mut caminho).somente_leitura(true).largura(320.0).rotulo("Caminho"));
                });
            });
        });
    }

    /// As sanfonas `details.modelo` (os gatilhos da aba Índices).
    fn card_gatilhos(&mut self, ui: &mut Ui) {
        Cartao::new("gatilhos").esticar().titulo("Gatilhos").hint("2 gatilhos").mostrar(ui, |c| {
            c.corpo(|ui| {
                let gatilhos = [
                    ("trg_pedidos_total", "CREATE TRIGGER \"trg_pedidos_total\" AFTER INSERT ON \"itens_pedido\"\nBEGIN\n  UPDATE \"pedidos\" SET \"total\" = \"total\" + NEW.\"preco\" WHERE \"id\" = NEW.\"pedido_id\";\nEND;"),
                    ("trg_clientes_criado", "CREATE TRIGGER \"trg_clientes_criado\" AFTER INSERT ON \"clientes\"\nBEGIN\n  UPDATE \"clientes\" SET \"criado_em\" = CURRENT_TIMESTAMP WHERE \"id\" = NEW.\"id\";\nEND;"),
                ];
                for (i, (nome, sql)) in gatilhos.iter().enumerate() {
                    Modelo::new(("mostruario", *nome), nome).primeiro(i == 0).aberto_inicial(i == 0).mostrar(ui, |ui| {
                        TextoPre::sql(sql).id_salt(("gatilho", *nome)).mostrar(ui);
                        em_linha(ui, 8.0, |ui| {
                            ui.add(Botao::new("Copiar"));
                            ui.add(Botao::ghost_perigo("Excluir"));
                        });
                    });
                }
            });
        });
    }

    fn card_vazio(&mut self, ui: &mut Ui) {
        Cartao::new("vazio").titulo("Estado vazio e zona de risco").mostrar(ui, |c| {
            c.corpo(|ui| {
                vazio(ui, "Nenhum índice nesta tabela. Índices aceleram buscas e ordenações por uma coluna.", true);
            });
            c.livre(|ui| {
                ZonaRisco::new("mostruario", "Zona de risco").hint("excluir esta tabela e tudo o que ela contém").no_card_tabela().mostrar(ui, |ui| {
                    let p = paleta(ui);
                    crate::pecas::texto::paragrafo(ui, "A tabela e as 60 linhas somem do arquivo, com os índices e gatilhos dela. Não há como desfazer.", 12.5, p.fg);
                    em_linha(ui, 8.0, |ui| {
                        let mut s = String::new();
                        ui.add(Entrada::new(&mut s).largura(320.0).dica("clientes"));
                        ui.add(Botao::perigo("Excluir tabela").habilitado(false));
                    });
                });
            });
        });
    }

    fn card_tiles(&mut self, ui: &mut Ui) {
        let tamanho = partes_bytes(1_258_291, None);
        let n_tabelas = formatar_numero(7);
        let n_linhas = formatar_numero(17_420);
        Cartao::new("tiles").esticar().titulo("Tiles").mostrar(ui, |c| {
            c.corpo(|ui| {
                tiles(
                    ui,
                    &[
                        Tile::new(&n_tabelas, "Tabelas").clicavel(true),
                        Tile::new("1", "Visões"),
                        Tile::new("3", "Índices"),
                        Tile::new(&tamanho.numero, "Tamanho").sufixo(tamanho.unidade),
                        Tile::new(&n_linhas, "Linhas (contadas)"),
                        Tile::new("—", "Gatilhos"),
                    ],
                );
            });
        });
    }

    fn card_arquivo(&mut self, ui: &mut Ui) {
        Cartao::new("arquivo").esticar().titulo("Arquivo").mostrar(ui, |c| {
            c.corpo(|ui| {
                detalhes(ui, |d| {
                    // a peça do Caminho (D3 da revisão de 26/10/2026): o rótulo em cima, até duas linhas, o Copiar na ponta
                    let _ = d.linha_caminho("Caminho", "C:\\Users\\ana\\Documents\\Projetos\\2026\\dados\\vendas.db");
                    d.linha_texto("Tamanho", Some(&formatar_bytes(1_258_291)), "—");
                    d.linha_texto("Modificado em", Some(&crate::formato::formatar_data("2026-09-22T10:31:00")), "—");
                    d.linha_texto("Versão do usuário", None, "não informada");
                    d.linha("Chaves estrangeiras", |ui| {
                        pill(ui, "verificadas", TipoPill::Ok);
                    });
                    // `h4.titulo-grupo` (o "Na grade de dados" da janela de atalhos)
                    d.titulo_grupo("Na grade de dados");
                    d.linha_texto("Setas", Some("Mover a célula ativa"), "—");
                    d.linha_texto("Enter ou F2", Some("Editar a célula ativa"), "—");
                });
            });
        });
    }

    fn card_sql(&mut self, ui: &mut Ui) {
        let sql = &self.sql;
        Cartao::new("sql")
            .esticar()
            .titulo("Comando de criação")
            .acoes(|ui| {
                if ui.add(Botao::new("Copiar")).clicked() {
                    ui.ctx().copy_text(sql.clone());
                    toast_concluido(ui.ctx(), "Copiado.");
                }
            })
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    TextoPre::sql(sql).altura_max(200.0).mostrar(ui);
                    TextoPre::new("Mensagem de exemplo, em texto pré-formatado:\n  linha recuada e quebra mantida.")
                        .mostrar(ui);
                });
            });
    }

    fn card_comando(&mut self, ui: &mut Ui) {
        let comando = &mut self.comando;
        let mut historico = 0u8;
        Cartao::new("comando")
            .esticar()
            .titulo("Comando")
            .hint("Ctrl+Enter executa")
            .acoes(|ui| {
                Select::new("historico", &mut historico, &[(0, "Histórico"), (1, "SELECT * FROM \"clientes\"")])
                    .largura(140.0)
                    .mostrar(ui);
                ui.add(Botao::primario("Executar"));
            })
            .mostrar(ui, |c| {
                c.corpo(|ui| {
                    ui.add(AreaTexto::new(comando).mono(true).codigo(true).altura(150.0));
                    ui.add_space(4.0);
                    fluxo(ui, 6.0, |ui| {
                        for nome in ["clientes", "pedidos", "itens_pedido"] {
                            if tag_clicavel(ui, nome, TipoTag::Tracejada).clicked() {
                                comando.push_str(&format!(" \"{nome}\""));
                            }
                        }
                    });
                });
            });
        // escolher no histórico traz o comando de volta para o editor (e o seletor volta ao rótulo)
        if historico == 1 {
            self.comando = "SELECT * FROM \"clientes\";".into();
        }
    }
}
