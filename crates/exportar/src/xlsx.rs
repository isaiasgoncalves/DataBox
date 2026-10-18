//! Excel (.xlsx) em fluxo: o XML de cada planilha vai comprimido direto para o arquivo, linha a linha, e só
//! as primeiras linhas de cada fonte (a amostra das larguras) ficam na memória, já na forma da célula (texto
//! cortado, BLOB grande trocado pelo marcador).
//!
//! O pacote segue o do openpyxl em modo write-only: texto em `inlineStr` (nunca
//! fórmula, nunca erro), cabeçalho em negrito branco sobre o navy #133C63, primeira linha congelada,
//! autofiltro com o nome `_xlnm._FilterDatabase` e larguras estimadas pela amostra.

use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use comum::{Resultado, Valor, formatar_real};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::arquivo::{ArquivoZip, CONTEXTO_GRAVAR, erro_io, erro_zip};
use crate::valores::{Avisos, CelulaExcel, comprimento, ilegal_excel, texto_excel, valor_excel};
use crate::{Contexto, LARGURA_MAXIMA, LARGURA_MINIMA, Limites, Progresso};

/// Colunas de uma planilha do Excel (XFD).
const MAX_COLUNAS: usize = 16_384;
/// O XML acumulado vai para o zip a cada 256 KB.
const BUFFER: usize = 256 * 1024;

const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CABECA_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const MARGENS: &str =
    "<pageMargins left=\"0.7\" right=\"0.7\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/>";

/// Nome de planilha válido no Excel e ainda não usado; o escolhido entra em `usados`.
///
/// Até 31 caracteres, sem `[]:*?/\`, sem apóstrofo nas pontas e nunca vazio ("Planilha"). A comparação com
/// os já usados não diferencia maiúsculas, como no Excel ("History" é reservado); repetido ganha " (2)",
/// " (3)"…, cortando o nome para caber nos 31.
pub fn nome_planilha(nome: &str, usados: &mut HashSet<String>) -> String {
    let texto: String = nome
        .chars()
        .filter(|c| !ilegal_excel(*c))
        .map(|c| match c {
            '[' | ']' | ':' | '*' | '?' | '/' | '\\' => '_',
            '\t' | '\r' | '\n' => ' ',
            outro => outro,
        })
        .collect();
    let cortado: String = aparar_planilha(&texto).chars().take(31).collect();
    let mut base = aparar_planilha(&cortado).to_string();
    if base.is_empty() {
        base = "Planilha".to_string();
    }
    let mut ocupados: HashSet<String> = usados.iter().map(|u| u.to_lowercase()).collect();
    ocupados.insert("history".to_string()); // o Excel reserva esse nome para o histórico de alterações
    let mut candidato = base.clone();
    let mut n = 2u64;
    while ocupados.contains(&candidato.to_lowercase()) {
        let sufixo = format!(" ({n})");
        let cabe = 31usize.saturating_sub(sufixo.chars().count());
        let inicio: String = base.chars().take(cabe).collect();
        candidato = format!("{}{sufixo}", inicio.trim_end());
        n += 1;
    }
    usados.insert(candidato.clone());
    candidato
}

/// Tira espaços e apóstrofos das pontas até estabilizar ("' 'a' '" vira "a").
fn aparar_planilha(texto: &str) -> &str {
    let mut t = texto;
    loop {
        let novo = t.trim().trim_matches('\'');
        if novo == t {
            return t;
        }
        t = novo;
    }
}

/// "A", "B", …, "Z", "AA"… da coluna `indice` (a partir de 0), guardadas para não refazer a cada célula.
fn letra(letras: &mut Vec<String>, indice: usize) -> &str {
    while letras.len() <= indice {
        let mut n = letras.len() + 1;
        let mut s = Vec::new();
        while n > 0 {
            let resto = (n - 1) % 26;
            s.push(char::from(b'A' + resto as u8));
            n = (n - 1) / 26;
        }
        letras.push(s.iter().rev().collect());
    }
    letras.get(indice).map_or("A", String::as_str)
}

/// Inteiro sem passar pela formatação genérica (milhões de células).
fn inteiro(buf: &mut Vec<u8>, n: i64) {
    let mut tmp = [0u8; 20];
    let mut i = tmp.len();
    let mut v = n.unsigned_abs();
    loop {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 || i == 0 {
            break;
        }
    }
    if n < 0 {
        buf.push(b'-');
    }
    buf.extend_from_slice(tmp.get(i..).unwrap_or_default());
}

/// Texto de elemento: `&`, `<`, `>` escapados, CR como referência (o XML trocaria um CR cru por LF) e o
/// `_xHHHH_` literal protegido como `_x005F_xHHHH_` (senão o Excel o leria como um caractere escapado).
fn escapar_texto(buf: &mut Vec<u8>, texto: &str) {
    let b = texto.as_bytes();
    let mut inicio = 0;
    for (i, &c) in b.iter().enumerate() {
        let troca: &[u8] = match c {
            b'&' => b"&amp;",
            b'<' => b"&lt;",
            b'>' => b"&gt;",
            b'\r' => b"&#13;",
            b'_' if parece_escape(b, i) => b"_x005F_",
            _ => continue,
        };
        buf.extend_from_slice(b.get(inicio..i).unwrap_or_default());
        buf.extend_from_slice(troca);
        inicio = i + 1;
    }
    buf.extend_from_slice(b.get(inicio..).unwrap_or_default());
}

fn parece_escape(b: &[u8], i: usize) -> bool {
    b.get(i + 1) == Some(&b'x')
        && b.get(i + 2..i + 6).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit))
        && b.get(i + 6) == Some(&b'_')
}

/// Texto de atributo (nome de planilha): também as aspas.
fn escapar_atributo(buf: &mut Vec<u8>, texto: &str) {
    for c in texto.chars() {
        match c {
            '&' => buf.extend_from_slice(b"&amp;"),
            '<' => buf.extend_from_slice(b"&lt;"),
            '>' => buf.extend_from_slice(b"&gt;"),
            '"' => buf.extend_from_slice(b"&quot;"),
            c => {
                let mut tmp = [0u8; 4];
                buf.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
        }
    }
}

/// `<t>` com `xml:space="preserve"` quando o texto começa ou termina com espaço (senão o Excel os tira).
fn elemento_t(buf: &mut Vec<u8>, texto: &str) {
    if texto.trim() != texto {
        buf.extend_from_slice(b"<t xml:space=\"preserve\">");
    } else {
        buf.extend_from_slice(b"<t>");
    }
    escapar_texto(buf, texto);
    buf.extend_from_slice(b"</t>");
}

fn abrir_celula(buf: &mut Vec<u8>, letras: &mut Vec<String>, coluna: usize, linha: &[u8]) {
    buf.extend_from_slice(b"<c r=\"");
    buf.extend_from_slice(letra(letras, coluna).as_bytes());
    buf.extend_from_slice(linha);
    buf.push(b'"');
}

/// As células de uma linha (até a última coluna do Excel), contando em `avisos` o que o Excel não comporta
/// como está. Cada linha passa por aqui uma vez só (direto para a planilha ou para a amostra).
fn celulas<'v>(valores: &'v [Valor], avisos: &mut Avisos) -> impl Iterator<Item = CelulaExcel<'v>> {
    valores.iter().take(MAX_COLUNAS).map(move |v| valor_excel(v, Some(&mut *avisos)))
}

/// Uma linha de dados; células vazias (NULL, texto vazio) ficam de fora.
fn linha_xml<'v>(
    buf: &mut Vec<u8>,
    letras: &mut Vec<String>,
    numero: u64,
    celulas: impl IntoIterator<Item = CelulaExcel<'v>>,
) {
    let r = numero.to_string();
    buf.extend_from_slice(b"<row r=\"");
    buf.extend_from_slice(r.as_bytes());
    buf.extend_from_slice(b"\">");
    for (i, celula) in celulas.into_iter().enumerate().take(MAX_COLUNAS) {
        match celula {
            CelulaExcel::Vazia => {}
            CelulaExcel::Inteiro(n) => {
                abrir_celula(buf, letras, i, r.as_bytes());
                buf.extend_from_slice(b"><v>");
                inteiro(buf, n);
                buf.extend_from_slice(b"</v></c>");
            }
            CelulaExcel::Real(x) => {
                abrir_celula(buf, letras, i, r.as_bytes());
                buf.extend_from_slice(b"><v>");
                buf.extend_from_slice(formatar_real(x).as_bytes());
                buf.extend_from_slice(b"</v></c>");
            }
            CelulaExcel::Texto(t) => {
                abrir_celula(buf, letras, i, r.as_bytes());
                buf.extend_from_slice(b" t=\"inlineStr\"><is>");
                elemento_t(buf, &t);
                buf.extend_from_slice(b"</is></c>");
            }
        }
    }
    buf.extend_from_slice(b"</row>");
}

/// Larguras das colunas pelo cabeçalho e pela amostra (o `_larguras`): +3 no cabeçalho pela setinha do
/// filtro, +2 nos valores, entre LARGURA_MINIMA e LARGURA_MAXIMA.
fn larguras(cabecalho: &[String], amostra: &[Vec<CelulaExcel<'_>>]) -> Vec<u32> {
    let mut maiores: Vec<usize> = cabecalho.iter().map(|t| t.chars().count() + 3).collect();
    for linha in amostra {
        for (maior, celula) in maiores.iter_mut().zip(linha) {
            let tamanho = comprimento(celula) + 2;
            if tamanho > *maior {
                *maior = tamanho;
            }
        }
    }
    maiores.into_iter().map(|m| u32::try_from(m).unwrap_or(u32::MAX).clamp(LARGURA_MINIMA, LARGURA_MAXIMA)).collect()
}

/// Planilha já gravada: o título e, se tem cabeçalho, o tamanho do autofiltro (colunas, última linha).
struct FolhaFeita {
    titulo: String,
    filtro: Option<(usize, u64)>,
}

/// A planilha aberta agora.
struct Folha {
    titulo: String,
    linhas: u64,
    bytes: u64,
}

/// A fonte (tabela ou consulta) sendo gravada.
struct Fonte {
    nome: String,
    cabecalho: Vec<String>,
    /// As primeiras linhas, guardadas até dar para estimar as larguras (que vêm antes das linhas no XML). Só
    /// as células como vão para a planilha (ver [`CelulaExcel::em_posse`]): 200 linhas com anexos de 20 MB
    /// seriam 4 GB guardados só para estimar larguras, e no Rust a falta de memória derruba o processo.
    amostra: Option<Vec<Vec<CelulaExcel<'static>>>>,
    larguras: Vec<u32>,
    /// Título da primeira planilha da fonte: base dos nomes das continuações ("eventos (2)").
    titulo: String,
}

pub(crate) struct Livro {
    zip: ZipWriter<ArquivoZip>,
    parado: Arc<AtomicBool>,
    opcoes_zip: SimpleFileOptions,
    momento_utc: String,
    usados: HashSet<String>,
    folhas: Vec<FolhaFeita>,
    fonte: Option<Fonte>,
    folha: Option<Folha>,
    letras: Vec<String>,
    buf: Vec<u8>,
}

impl Livro {
    pub fn abrir(arquivo: File, momento: zip::DateTime) -> Livro {
        let (arquivo, parado) = ArquivoZip::novo(arquivo);
        let opcoes_zip =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).last_modified_time(momento);
        Livro {
            zip: ZipWriter::new(arquivo),
            parado,
            opcoes_zip,
            momento_utc: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            usados: HashSet::new(),
            folhas: Vec::new(),
            fonte: None,
            folha: None,
            letras: Vec::new(),
            buf: Vec::with_capacity(BUFFER + 64 * 1024),
        }
    }

    pub fn comecar(&mut self, nome: &str, colunas: &[String]) {
        self.fonte = Some(Fonte {
            nome: nome.to_string(),
            cabecalho: colunas.iter().map(|c| texto_excel(c).0.into_owned()).collect(),
            amostra: Some(Vec::new()),
            larguras: Vec::new(),
            titulo: String::new(),
        });
    }

    pub fn linha(&mut self, valores: &[Valor], ctx: &mut Contexto) -> Resultado<()> {
        let Contexto { progresso, avisos, limites, .. } = ctx;
        let despejar = match &mut self.fonte {
            None => return Err(crate::erro_sem_tabela()),
            Some(Fonte { amostra: Some(amostra), .. }) => {
                if amostra.len() < limites.amostra_largura {
                    // os avisos já contam aqui: toda linha da amostra vai para a planilha depois
                    amostra.push(celulas(valores, avisos).map(CelulaExcel::em_posse).collect());
                    return Ok(());
                }
                true
            }
            Some(_) => false,
        };
        if despejar {
            self.despejar_amostra(limites, progresso)?;
        }
        self.gravar_linha(celulas(valores, avisos), limites, progresso)
    }

    /// Larguras pela amostra, primeira planilha da fonte e as linhas da amostra nela.
    fn despejar_amostra(&mut self, limites: &Limites, progresso: &mut Progresso) -> Resultado<()> {
        let Some(fonte) = self.fonte.as_mut() else {
            return Ok(());
        };
        let Some(amostra) = fonte.amostra.take() else {
            return Ok(());
        };
        fonte.larguras = larguras(&fonte.cabecalho, &amostra);
        fonte.titulo = nome_planilha(&fonte.nome, &mut self.usados);
        let titulo = fonte.titulo.clone();
        self.abrir_folha(titulo)?;
        for linha in amostra {
            self.gravar_linha(linha, limites, progresso)?;
        }
        Ok(())
    }

    fn gravar_linha<'v>(
        &mut self,
        celulas: impl IntoIterator<Item = CelulaExcel<'v>>,
        limites: &Limites,
        progresso: &mut Progresso,
    ) -> Resultado<()> {
        // Planilha cheia continua em "nome (2)". Também antes do limite de 4 GB de cada parte do zip sem ZIP64
        // (só uma tabela de linhas enormes chega lá).
        let cheia =
            self.folha.as_ref().is_none_or(|f| f.linhas >= limites.linhas_excel || f.bytes >= limites.bytes_folha);
        if cheia {
            self.fechar_folha()?;
            let base = self.fonte.as_ref().map(|f| f.titulo.clone()).unwrap_or_default();
            let titulo = nome_planilha(&base, &mut self.usados);
            self.abrir_folha(titulo)?;
        }
        let com_cabecalho = self.fonte.as_ref().is_some_and(|f| !f.cabecalho.is_empty());
        let Some(folha) = self.folha.as_mut() else {
            return Err(crate::erro_sem_tabela());
        };
        let numero = folha.linhas + 1 + u64::from(com_cabecalho);
        let antes = self.buf.len();
        linha_xml(&mut self.buf, &mut self.letras, numero, celulas);
        folha.linhas += 1;
        folha.bytes += (self.buf.len() - antes) as u64;
        if self.buf.len() >= BUFFER {
            self.esvaziar()?;
        }
        progresso.mais_uma()
    }

    fn esvaziar(&mut self) -> Resultado<()> {
        if !self.buf.is_empty() {
            self.zip.write_all(&self.buf).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
            self.buf.clear();
        }
        Ok(())
    }

    /// Abre uma planilha com as larguras, o congelamento e o cabeçalho da fonte atual (ou vazia, sem fonte).
    fn abrir_folha(&mut self, titulo: String) -> Resultado<()> {
        let indice = self.folhas.len() + 1;
        self.zip.start_file(format!("xl/worksheets/sheet{indice}.xml"), self.opcoes_zip).map_err(erro_zip)?;
        let (cabecalho, larguras): (&[String], &[u32]) = match &self.fonte {
            Some(f) => (&f.cabecalho, &f.larguras),
            None => (&[], &[]),
        };
        let buf = &mut self.buf;
        buf.extend_from_slice(CABECA_XML.as_bytes());
        buf.extend_from_slice(format!("<worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">").as_bytes());
        buf.extend_from_slice(b"<sheetViews><sheetView");
        if indice == 1 {
            buf.extend_from_slice(b" tabSelected=\"1\"");
        }
        buf.extend_from_slice(b" workbookViewId=\"0\"");
        if cabecalho.is_empty() {
            buf.extend_from_slice(b"/>");
        } else {
            buf.extend_from_slice(
                b"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/>\
                  <selection pane=\"bottomLeft\" activeCell=\"A2\" sqref=\"A2\"/></sheetView>",
            );
        }
        buf.extend_from_slice(b"</sheetViews><sheetFormatPr defaultRowHeight=\"15\"/>");
        if !larguras.is_empty() {
            buf.extend_from_slice(b"<cols>");
            for (i, largura) in larguras.iter().enumerate() {
                let n = i + 1;
                buf.extend_from_slice(
                    format!("<col min=\"{n}\" max=\"{n}\" width=\"{largura}\" customWidth=\"1\"/>").as_bytes(),
                );
            }
            buf.extend_from_slice(b"</cols>");
        }
        buf.extend_from_slice(b"<sheetData>");
        if !cabecalho.is_empty() {
            buf.extend_from_slice(b"<row r=\"1\">");
            for (i, texto) in cabecalho.iter().enumerate().take(MAX_COLUNAS) {
                abrir_celula(buf, &mut self.letras, i, b"1");
                if texto.is_empty() {
                    buf.extend_from_slice(b" s=\"1\"/>");
                } else {
                    // texto explícito: uma coluna chamada "=x" continua sendo texto
                    buf.extend_from_slice(b" s=\"1\" t=\"inlineStr\"><is>");
                    elemento_t(buf, texto);
                    buf.extend_from_slice(b"</is></c>");
                }
            }
            buf.extend_from_slice(b"</row>");
        }
        let bytes = self.buf.len() as u64;
        self.folha = Some(Folha { titulo, linhas: 0, bytes });
        Ok(())
    }

    /// Fecha a planilha aberta com o autofiltro (se tem cabeçalho) e as margens.
    fn fechar_folha(&mut self) -> Resultado<()> {
        let Some(folha) = self.folha.take() else {
            return Ok(());
        };
        let colunas = self.fonte.as_ref().map_or(0, |f| f.cabecalho.len().min(MAX_COLUNAS));
        self.buf.extend_from_slice(b"</sheetData>");
        let filtro = (colunas > 0).then_some((colunas, folha.linhas + 1));
        if let Some((colunas, ultima)) = filtro {
            let referencia = format!("A1:{}{ultima}", letra(&mut self.letras, colunas - 1));
            self.buf.extend_from_slice(format!("<autoFilter ref=\"{referencia}\"/>").as_bytes());
        }
        self.buf.extend_from_slice(MARGENS.as_bytes());
        self.buf.extend_from_slice(b"</worksheet>");
        self.esvaziar()?;
        self.folhas.push(FolhaFeita { titulo: folha.titulo, filtro });
        Ok(())
    }

    /// Fim da fonte: a que teve menos linhas que a amostra só agora ganha a planilha.
    pub fn terminar(&mut self, ctx: &mut Contexto) -> Resultado<()> {
        if self.fonte.as_ref().is_some_and(|f| f.amostra.is_some()) {
            self.despejar_amostra(&ctx.limites, &mut ctx.progresso)?;
        }
        self.fechar_folha()?;
        self.fonte = None;
        Ok(())
    }

    /// Grava as partes do pacote (livro, estilos, propriedades, tipos) e fecha o zip.
    pub fn fechar(mut self) -> Resultado<()> {
        if let Err(e) = self.montar() {
            self.parado.store(true, Ordering::Relaxed);
            return Err(e);
        }
        let Livro { zip, parado, .. } = self;
        match zip.finish() {
            Ok(arquivo) => arquivo.terminar().map_err(|e| erro_io(&e, CONTEXTO_GRAVAR)),
            Err(e) => {
                parado.store(true, Ordering::Relaxed);
                Err(erro_zip(e))
            }
        }
    }

    fn montar(&mut self) -> Resultado<()> {
        self.fechar_folha()?;
        self.fonte = None;
        if self.folhas.is_empty() {
            // livro sem fontes: uma planilha vazia, para o arquivo abrir
            let titulo = nome_planilha("", &mut self.usados);
            self.abrir_folha(titulo)?;
            self.fechar_folha()?;
        }
        let partes = [
            ("xl/workbook.xml", self.workbook()),
            ("xl/_rels/workbook.xml.rels", self.rels_workbook()),
            ("xl/styles.xml", ESTILOS.as_bytes().to_vec()),
            ("docProps/core.xml", self.core()),
            ("docProps/app.xml", APP.as_bytes().to_vec()),
            ("_rels/.rels", RELS.as_bytes().to_vec()),
            ("[Content_Types].xml", self.tipos()),
        ];
        for (nome, conteudo) in partes {
            self.zip.start_file(nome, self.opcoes_zip).map_err(erro_zip)?;
            self.zip.write_all(&conteudo).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        }
        Ok(())
    }

    fn workbook(&mut self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(CABECA_XML.as_bytes());
        b.extend_from_slice(format!("<workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">").as_bytes());
        b.extend_from_slice(b"<workbookPr/><bookViews><workbookView activeTab=\"0\"/></bookViews><sheets>");
        for (i, folha) in self.folhas.iter().enumerate() {
            b.extend_from_slice(b"<sheet name=\"");
            escapar_atributo(&mut b, &folha.titulo);
            b.extend_from_slice(format!("\" sheetId=\"{n}\" r:id=\"rId{n}\"/>", n = i + 1).as_bytes());
        }
        b.extend_from_slice(b"</sheets>");
        let mut nomes = Vec::new();
        for (i, folha) in self.folhas.iter().enumerate() {
            let Some((colunas, ultima)) = folha.filtro else {
                continue;
            };
            let col = letra(&mut self.letras, colunas - 1).to_string();
            nomes.extend_from_slice(
                format!("<definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"{i}\" hidden=\"1\">").as_bytes(),
            );
            let referencia = format!("'{}'!$A$1:${col}${ultima}", folha.titulo.replace('\'', "''"));
            escapar_atributo(&mut nomes, &referencia);
            nomes.extend_from_slice(b"</definedName>");
        }
        if !nomes.is_empty() {
            b.extend_from_slice(b"<definedNames>");
            b.extend_from_slice(&nomes);
            b.extend_from_slice(b"</definedNames>");
        }
        b.extend_from_slice(b"<calcPr calcId=\"191029\"/></workbook>");
        b
    }

    fn rels_workbook(&self) -> Vec<u8> {
        let mut b = String::new();
        b.push_str(CABECA_XML);
        b.push_str(&format!("<Relationships xmlns=\"{NS_PKG_REL}\">"));
        for i in 1..=self.folhas.len() {
            b.push_str(&format!(
                "<Relationship Id=\"rId{i}\" Type=\"{NS_REL}/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
            ));
        }
        b.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"{NS_REL}/styles\" Target=\"styles.xml\"/></Relationships>",
            self.folhas.len() + 1
        ));
        b.into_bytes()
    }

    fn core(&self) -> Vec<u8> {
        format!(
            "{CABECA_XML}<cp:coreProperties \
             xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
             xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
             xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
             <dc:creator>{autor}</dc:creator>\
             <dcterms:created xsi:type=\"dcterms:W3CDTF\">{m}</dcterms:created>\
             <dcterms:modified xsi:type=\"dcterms:W3CDTF\">{m}</dcterms:modified></cp:coreProperties>",
            autor = comum::app::NOME,
            m = self.momento_utc
        )
        .into_bytes()
    }

    fn tipos(&self) -> Vec<u8> {
        let mut b = String::new();
        b.push_str(CABECA_XML);
        b.push_str(
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Override PartName=\"/xl/workbook.xml\" \
             ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>",
        );
        for i in 1..=self.folhas.len() {
            b.push_str(&format!(
                "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" \
                 ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
            ));
        }
        b.push_str(
            "<Override PartName=\"/xl/styles.xml\" \
             ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\
             <Override PartName=\"/docProps/core.xml\" \
             ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
             <Override PartName=\"/docProps/app.xml\" \
             ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/></Types>",
        );
        b.into_bytes()
    }

    /// Descarta o livro sem gravar mais nada (o temporário vai ser apagado).
    pub fn abortar(self) {
        self.parado.store(true, Ordering::Relaxed);
        drop(self);
    }
}

/// Estilos: a fonte padrão e o cabeçalho (estilo 1: negrito branco sobre o navy da marca, --cr-navy).
const ESTILOS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
<fonts count=\"2\">\
<font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font>\
<font><b/><sz val=\"11\"/><color rgb=\"FFFFFFFF\"/><name val=\"Calibri\"/><family val=\"2\"/></font>\
</fonts>\
<fills count=\"3\">\
<fill><patternFill patternType=\"none\"/></fill>\
<fill><patternFill patternType=\"gray125\"/></fill>\
<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF133C63\"/><bgColor rgb=\"FF133C63\"/></patternFill></fill>\
</fills>\
<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"2\">\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\"/>\
</cellXfs>\
<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\
<dxfs count=\"0\"/>\
<tableStyles count=\"0\" defaultTableStyle=\"TableStyleMedium9\" defaultPivotStyle=\"PivotStyleLight16\"/>\
</styleSheet>";

const APP: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" \
xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">\
<Application>Microsoft Excel</Application></Properties>";

const RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" \
Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
Target=\"xl/workbook.xml\"/>\
<Relationship Id=\"rId2\" \
Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" \
Target=\"docProps/core.xml\"/>\
<Relationship Id=\"rId3\" \
Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" \
Target=\"docProps/app.xml\"/>\
</Relationships>";

#[cfg(test)]
pub(crate) fn letra_coluna(indice: usize) -> String {
    letra(&mut Vec::new(), indice).to_string()
}

#[cfg(test)]
pub(crate) fn escapar_para_teste(texto: &str) -> String {
    let mut b = Vec::new();
    escapar_texto(&mut b, texto);
    String::from_utf8_lossy(&b).into_owned()
}
