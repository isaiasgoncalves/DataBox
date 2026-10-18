//! Apoio dos testes da tela da planilha: montar um `.xlsx` no próprio teste (as partes XML mínimas num zip, como os
//! montadores do núcleo em `crates\planilha\src\testes.rs`), abri-lo num palco (1440×900, a tela desenhada como a casca),
//! esperar a abertura e o índice, e gravar as capturas.
//!
//! Criar, renderizar e desfazer um palco (um wgpu) passa por uma vez só ([`vez`]): vários ao mesmo tempo, nas threads dos
//! testes, às vezes derrubam o processo dentro do driver.

#![allow(dead_code)]

use std::fmt::Write as _;
use std::io::{Cursor, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use egui::vec2;
use egui_kittest::Harness;
use estado::apoio_testes::{Ambiente, NoPalco, Palco, esperar, palco_documento};
use interface::tema::Tema;
use tela_planilha::{Modo, TelaPlanilha};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub type Tela = NoPalco<TelaPlanilha>;
pub type H = Harness<'static, Palco<Tela>>;

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CABECA: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

static VEZ: Mutex<()> = Mutex::new(());

/// A vez de mexer no wgpu de um palco (criar, renderizar, desfazer), uma thread de cada vez.
pub fn vez() -> MutexGuard<'static, ()> {
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// O tamanho das capturas (a viewport das pranchetas).
pub const TAMANHO: egui::Vec2 = vec2(1440.0, 900.0);

// ------------------------------------------------------------------ o montador de .xlsx

/// Uma célula para o XML da aba.
#[derive(Debug, Clone)]
pub enum C {
    /// Vazia (a referência não é escrita).
    Nada,
    Texto(String),
    Num(f64),
    /// Número com o estilo `s` (o índice em `cellXfs`).
    NumEstilo(f64, u32),
    Bool(bool),
    Erro(&'static str),
    /// Fórmula com o último valor calculado.
    Formula(&'static str, f64, u32),
}

pub fn t(s: &str) -> C {
    C::Texto(s.to_string())
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// O `<row>` da linha `numero` (a partir de 1) com as células dadas, a partir da coluna A; `oculta` põe `hidden="1"`.
pub fn linha_xml(numero: u32, celulas: &[C], oculta: bool) -> String {
    let mut s = format!("<row r=\"{numero}\"{}>", if oculta { " hidden=\"1\"" } else { "" });
    for (i, c) in celulas.iter().enumerate() {
        let r = planilha::referencia_a1(numero, i as u32);
        match c {
            C::Nada => {}
            C::Texto(t) => {
                let _ = write!(s, "<c r=\"{r}\" t=\"inlineStr\"><is><t>{}</t></is></c>", escapar(t));
            }
            C::Num(n) => {
                let _ = write!(s, "<c r=\"{r}\"><v>{n}</v></c>");
            }
            C::NumEstilo(n, e) => {
                let _ = write!(s, "<c r=\"{r}\" s=\"{e}\"><v>{n}</v></c>");
            }
            C::Bool(b) => {
                let _ = write!(s, "<c r=\"{r}\" t=\"b\"><v>{}</v></c>", u8::from(*b));
            }
            C::Erro(e) => {
                let _ = write!(s, "<c r=\"{r}\" t=\"e\"><v>{e}</v></c>");
            }
            C::Formula(f, v, e) => {
                let _ = write!(s, "<c r=\"{r}\" s=\"{e}\"><f>{}</f><v>{v}</v></c>", escapar(f));
            }
        }
    }
    s.push_str("</row>");
    s
}

/// Uma aba: o nome, o estado (`""`, `"hidden"`, `"veryHidden"`) e o XML inteiro dela.
struct Aba {
    nome: String,
    estado: &'static str,
    xml: String,
}

/// O que vai antes e depois do `<sheetData>` de uma aba.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub dimensao: Option<String>,
    /// `(linhas, colunas)` congeladas (o `<pane>` com `state="frozen"`).
    pub congelado: Option<(u32, u32)>,
    /// `(primeira, última, largura, oculta)` das colunas (a partir de 1, como no XML).
    pub colunas: Vec<(u32, u32, f64, bool)>,
    pub mescladas: Vec<String>,
    pub filtro: Option<String>,
}

/// Monta um `.xlsx` mínimo, aba por aba.
#[derive(Default)]
pub struct Montador {
    abas: Vec<Aba>,
    estilos: Option<String>,
    ativa: Option<usize>,
    d1904: bool,
    graficos: usize,
}

impl Montador {
    pub fn novo() -> Montador {
        Montador::default()
    }

    /// Uma aba com as `<row>` dadas.
    pub fn aba(self, nome: &str, linhas: &str) -> Montador {
        self.aba_com(nome, linhas, &Extras::default())
    }

    /// Uma aba com as `<row>` e os extras (dimensão, congelado, colunas, mescladas, filtro).
    pub fn aba_com(mut self, nome: &str, linhas: &str, x: &Extras) -> Montador {
        let mut s = format!("{CABECA}<worksheet xmlns=\"{NS}\">");
        if let Some(d) = &x.dimensao {
            let _ = write!(s, "<dimension ref=\"{d}\"/>");
        }
        if let Some((l, c)) = x.congelado {
            let canto = planilha::referencia_a1(l + 1, c);
            let mut pane = String::from("<pane");
            if c > 0 {
                let _ = write!(pane, " xSplit=\"{c}\"");
            }
            if l > 0 {
                let _ = write!(pane, " ySplit=\"{l}\"");
            }
            let _ = write!(pane, " topLeftCell=\"{canto}\" activePane=\"bottomRight\" state=\"frozen\"/>");
            let _ = write!(s, "<sheetViews><sheetView workbookViewId=\"0\">{pane}</sheetView></sheetViews>");
        }
        if !x.colunas.is_empty() {
            s.push_str("<cols>");
            for (a, b, w, oculta) in &x.colunas {
                let _ = write!(
                    s,
                    "<col min=\"{a}\" max=\"{b}\" width=\"{w}\" customWidth=\"1\"{}/>",
                    if *oculta { " hidden=\"1\"" } else { "" }
                );
            }
            s.push_str("</cols>");
        }
        let _ = write!(s, "<sheetData>{linhas}</sheetData>");
        if let Some(f) = &x.filtro {
            let _ = write!(s, "<autoFilter ref=\"{f}\"/>");
        }
        if !x.mescladas.is_empty() {
            let _ = write!(s, "<mergeCells count=\"{}\">", x.mescladas.len());
            for m in &x.mescladas {
                let _ = write!(s, "<mergeCell ref=\"{m}\"/>");
            }
            s.push_str("</mergeCells>");
        }
        s.push_str("</worksheet>");
        self.abas.push(Aba { nome: nome.to_string(), estado: "", xml: s });
        self
    }

    /// Uma aba com o XML dado por inteiro (o estragado).
    pub fn aba_xml(mut self, nome: &str, xml: &str) -> Montador {
        self.abas.push(Aba { nome: nome.to_string(), estado: "", xml: xml.to_string() });
        self
    }

    /// O estado da última aba (`"hidden"`, `"veryHidden"`).
    pub fn estado(mut self, estado: &'static str) -> Montador {
        if let Some(a) = self.abas.last_mut() {
            a.estado = estado;
        }
        self
    }

    pub fn ativa(mut self, i: usize) -> Montador {
        self.ativa = Some(i);
        self
    }

    pub fn d1904(mut self) -> Montador {
        self.d1904 = true;
        self
    }

    /// Mais uma folha de gráfico (sem células) no fim.
    pub fn grafico(mut self) -> Montador {
        self.graficos += 1;
        self
    }

    /// Os formatos de número (`numFmts`) e os estilos (`cellXfs`, cada um com o `numFmtId`).
    pub fn estilos(mut self, numfmts: &[(u32, &str)], xfs: &[u32]) -> Montador {
        let mut s = format!("{CABECA}<styleSheet xmlns=\"{NS}\">");
        if !numfmts.is_empty() {
            let _ = write!(s, "<numFmts count=\"{}\">", numfmts.len());
            for (id, codigo) in numfmts {
                let _ = write!(s, "<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>", escapar(codigo));
            }
            s.push_str("</numFmts>");
        }
        s.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\"/></cellStyleXfs>");
        let _ = write!(s, "<cellXfs count=\"{}\">", xfs.len());
        for id in xfs {
            let _ = write!(s, "<xf numFmtId=\"{id}\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/>");
        }
        s.push_str("</cellXfs></styleSheet>");
        self.estilos = Some(s);
        self
    }

    pub fn bytes(&self) -> Vec<u8> {
        let n = self.abas.len();
        let mut tipos = format!(
            "{CABECA}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>"
        );
        for i in 1..=n {
            let _ = write!(
                tipos,
                "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
            );
        }
        tipos.push_str("</Types>");
        let raiz = format!(
            "{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rId1\" Type=\"{NS_R}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
        );
        let mut livro = format!("{CABECA}<workbook xmlns=\"{NS}\" xmlns:r=\"{NS_R}\">");
        let _ = write!(livro, "<workbookPr{}/>", if self.d1904 { " date1904=\"1\"" } else { "" });
        if let Some(a) = self.ativa {
            let _ = write!(livro, "<bookViews><workbookView activeTab=\"{a}\"/></bookViews>");
        }
        livro.push_str("<sheets>");
        let mut rels =
            format!("{CABECA}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        for (i, aba) in self.abas.iter().enumerate() {
            let i = i + 1;
            let estado = if aba.estado.is_empty() { String::new() } else { format!(" state=\"{}\"", aba.estado) };
            let _ = write!(livro, "<sheet name=\"{}\" sheetId=\"{i}\"{estado} r:id=\"rId{i}\"/>", escapar(&aba.nome));
            let _ = write!(
                rels,
                "<Relationship Id=\"rId{i}\" Type=\"{NS_R}/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
            );
        }
        let mut extra = n + 1;
        for g in 0..self.graficos {
            let _ = write!(livro, "<sheet name=\"Gráfico{}\" sheetId=\"{extra}\" r:id=\"rId{extra}\"/>", g + 1);
            let _ = write!(
                rels,
                "<Relationship Id=\"rId{extra}\" Type=\"{NS_R}/chartsheet\" Target=\"chartsheets/sheet{}.xml\"/>",
                g + 1
            );
            extra += 1;
        }
        livro.push_str("</sheets></workbook>");
        if self.estilos.is_some() {
            let _ = write!(rels, "<Relationship Id=\"rId{extra}\" Type=\"{NS_R}/styles\" Target=\"styles.xml\"/>");
        }
        rels.push_str("</Relationships>");
        let mut partes = vec![
            ("[Content_Types].xml".to_string(), tipos),
            ("_rels/.rels".to_string(), raiz),
            ("xl/workbook.xml".to_string(), livro),
            ("xl/_rels/workbook.xml.rels".to_string(), rels),
        ];
        for (i, aba) in self.abas.iter().enumerate() {
            partes.push((format!("xl/worksheets/sheet{}.xml", i + 1), aba.xml.clone()));
        }
        for g in 0..self.graficos {
            partes
                .push((format!("xl/chartsheets/sheet{}.xml", g + 1), format!("{CABECA}<chartsheet xmlns=\"{NS}\"/>")));
        }
        if let Some(e) = &self.estilos {
            partes.push(("xl/styles.xml".to_string(), e.clone()));
        }
        zip_de(&partes)
    }
}

/// Um zip com as partes dadas (comprimidas).
pub fn zip_de(partes: &[(String, String)]) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (nome, conteudo) in partes {
        z.start_file(nome.as_str(), opcoes).expect("começar a parte");
        z.write_all(conteudo.as_bytes()).expect("gravar a parte");
    }
    z.finish().expect("fechar o zip").into_inner()
}

/// Um arquivo do Office antigo (a assinatura OLE) com os nomes de fluxo dados (o `EncryptedPackage` da senha, o
/// `Workbook` do `.xls`), como os testes do núcleo.
pub fn ole_com(nomes: &[&str]) -> Vec<u8> {
    let mut b = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    b.resize(600, 0);
    for n in nomes {
        b.extend(n.encode_utf16().flat_map(u16::to_le_bytes));
        b.extend([0u8; 16]);
    }
    b.resize(b.len() + 4096, 0);
    b
}

/// `n` linhas simples a partir da 1: o número da linha em A e um texto em B.
pub fn linhas_simples(n: u32) -> String {
    let mut s = String::with_capacity(n as usize * 60);
    for i in 1..=n {
        let _ = write!(
            s,
            "<row r=\"{i}\"><c r=\"A{i}\"><v>{i}</v></c><c r=\"B{i}\" t=\"inlineStr\"><is><t>l{i}</t></is></c></row>"
        );
    }
    s
}

// ------------------------------------------------------------------ o palco

/// O palco de um teste: usado como o próprio harness (`Deref`), desfeito na sua vez.
pub struct Teste {
    h: Option<H>,
    /// O ambiente (a pasta temporária do teste) vive até o fim do teste.
    pub amb: Ambiente,
    pub caminho: PathBuf,
}

impl Deref for Teste {
    type Target = H;
    fn deref(&self) -> &H {
        self.h.as_ref().expect("palco desfeito")
    }
}

impl DerefMut for Teste {
    fn deref_mut(&mut self) -> &mut H {
        self.h.as_mut().expect("palco desfeito")
    }
}

impl Drop for Teste {
    fn drop(&mut self) {
        let _vez = vez();
        drop(self.h.take());
    }
}

/// Grava `bytes` como `nome` numa pasta do ambiente e devolve o caminho.
pub fn gravar(amb: &Ambiente, nome: &str, bytes: &[u8]) -> PathBuf {
    let pasta = amb.pasta().join("arquivos");
    std::fs::create_dir_all(&pasta).expect("criar a pasta dos arquivos");
    let caminho = pasta.join(nome);
    std::fs::write(&caminho, bytes).expect("gravar o arquivo de teste");
    caminho
}

/// Abre `nome` (com os `bytes`) num palco claro e espera a abertura terminar (pronta ou falhou).
pub fn abrir(nome_do_teste: &str, nome: &str, bytes: &[u8]) -> Teste {
    abrir_com(nome_do_teste, nome, bytes, Tema::Claro, TAMANHO, |t| t)
}

/// [`abrir`] com o tema, o tamanho e um ajuste na tela antes de ela abrir (o índice manual).
pub fn abrir_com(
    nome_do_teste: &str,
    nome: &str,
    bytes: &[u8],
    tema: Tema,
    tamanho: egui::Vec2,
    ajustar: impl FnOnce(TelaPlanilha) -> TelaPlanilha,
) -> Teste {
    let amb = Ambiente::vazio(nome_do_teste);
    let caminho = gravar(&amb, nome, bytes);
    abrir_caminho(amb, caminho, tema, tamanho, ajustar)
}

/// Abre o `caminho` (que pode não existir: o arquivo que sumiu).
pub fn abrir_caminho(
    amb: Ambiente,
    caminho: PathBuf,
    tema: Tema,
    tamanho: egui::Vec2,
    ajustar: impl FnOnce(TelaPlanilha) -> TelaPlanilha,
) -> Teste {
    let tela = ajustar(TelaPlanilha::nova(caminho.clone()));
    let h = {
        let _vez = vez();
        palco_documento(&amb, tela, tema, tamanho)
    };
    let mut t = Teste { h: Some(h), amb, caminho };
    esperar_a_abertura(&mut t);
    t
}

/// Roda quadros até a tela sair do "Abrindo" (e a planilha da tela ter a primeira página, se houver uma).
pub fn esperar_a_abertura(t: &mut Teste) {
    esperar(&mut **t, |h| {
        let d = &h.state().tela.doc;
        !matches!(d.modo(), Modo::Inicial | Modo::Abrindo) && d.sessao().is_none_or(|s| !s.abrindo())
    });
}

/// Roda quadros até o índice da planilha da tela terminar.
pub fn esperar_o_indice(t: &mut Teste) {
    esperar(&mut **t, |h| h.state().tela.doc.sessao().is_some_and(|s| s.indexada()));
}

/// Roda quadros até `condicao` valer (as tarefas de fundo terminam em outras threads).
pub fn esperar_ate(t: &mut Teste, condicao: impl FnMut(&H) -> bool) {
    esperar(&mut **t, condicao);
}

/// A tela no palco.
pub fn tela(t: &H) -> &TelaPlanilha {
    &t.state().tela.doc
}

pub fn tela_mut(t: &mut H) -> &mut TelaPlanilha {
    &mut t.state_mut().tela.doc
}

/// Renderiza (na vez do wgpu) e grava a captura em `capturas/rust/`.
pub fn capturar(t: &mut H, nome: &str) -> PathBuf {
    t.run_steps(2);
    let imagem = {
        let _vez = vez();
        t.render().expect("renderizar com wgpu")
    };
    let destino = estado::apoio_testes::pasta_capturas().join(nome);
    imagem.save(&destino).expect("gravar png");
    destino
}

/// O caminho de um arquivo que não existe na pasta do ambiente.
pub fn inexistente(amb: &Ambiente, nome: &str) -> PathBuf {
    amb.pasta().join("arquivos").join(nome)
}

/// O arquivo existe?
pub fn existe(p: &Path) -> bool {
    p.exists()
}

// ------------------------------------------------------------------ a planilha da prancheta 17

/// As regiões e os produtos da cena a da prancheta 17 (os mesmos do `PlanilhaExemplo` do mostruário).
const REGIOES: [&str; 5] = ["Sul", "Sudeste", "Nordeste", "Norte", "Centro-Oeste"];
const PRODUTOS: [&str; 6] = [
    "Caderno universitário",
    "Caneta gel 0,7 mm",
    "Mochila escolar G",
    "Agenda 2026 diária",
    "Estojo duplo",
    "Marca-texto (kit 6)",
];
/// O título mesclado A1:F1.
pub const TITULO: &str = "Vendas do 3º trimestre de 2026 · por região e produto";
/// O número de série de 01/07/2026 no sistema de 1900.
const SERIE_1_JUL_2026: f64 = 46_204.0;

/// O `relatorio_q3.xlsx` da prancheta 17, cena a: as planilhas Resumo, Vendas (a ativa), Regiões e Custos (oculta), e um
/// gráfico; em Vendas, o título A1:F1 mesclado, o cabeçalho na linha 2, a linha 1 e a coluna A congeladas, as larguras
/// 18, 30, 14, 18, 14 e 16 caracteres, datas, reais e percentuais pelo estilo, o `#DIV/0!` em E12, a linha 9 oculta pelo
/// filtro A2:F{linhas}, a fórmula `D6/Resumo!C3` em E6 (98%) e os booleanos em F. `linhas` é a última linha.
pub fn relatorio_q3(linhas: u32) -> Vec<u8> {
    let mut xml = linha_xml(1, &[t(TITULO)], false);
    xml.push_str(&linha_xml(
        2,
        &[t("Região"), t("Produto"), t("Fechado em"), t("Receita"), t("Atingido"), t("Meta batida")],
        false,
    ));
    for n in 3..=linhas {
        let k = (n - 2) as usize;
        let pct = 72 + (k * 37) % 61;
        let receita = ((2200 + (k * 7919) % 11_800) * 100 + (k * 37) % 100) as f64 / 100.0;
        let atingido = if n == 12 {
            C::Erro("#DIV/0!")
        } else if n == 6 {
            C::Formula("D6/Resumo!C3", pct as f64 / 100.0, 3)
        } else {
            C::NumEstilo(pct as f64 / 100.0, 3)
        };
        let celulas = [
            t(REGIOES[k % 5]),
            t(PRODUTOS[(k * 5) % 6]),
            C::NumEstilo(SERIE_1_JUL_2026 + (k - 1) as f64, 1),
            C::NumEstilo(receita, 2),
            atingido,
            C::Bool(pct >= 100 && n != 12),
        ];
        xml.push_str(&linha_xml(n, &celulas, n == 9));
    }
    let extras = Extras {
        dimensao: Some(format!("A1:F{linhas}")),
        congelado: Some((1, 1)),
        colunas: vec![
            (1, 1, 18.0, false),
            (2, 2, 30.0, false),
            (3, 3, 14.0, false),
            (4, 4, 18.0, false),
            (5, 5, 14.0, false),
            (6, 6, 16.0, false),
        ],
        mescladas: vec!["A1:F1".to_string()],
        filtro: Some(format!("A2:F{linhas}")),
    };
    Montador::novo()
        .aba("Resumo", &linha_xml(1, &[t("Resumo"), C::Num(1.0), C::Num(1.0)], false))
        .aba_com("Vendas", &xml, &extras)
        .aba("Regiões", &linhas_simples(8))
        .aba("Custos", &linhas_simples(4))
        .estado("hidden")
        .grafico()
        .ativa(1)
        .estilos(&[(164, "\"R$ \"#,##0.00")], &[0, 14, 164, 9])
        .bytes()
}
