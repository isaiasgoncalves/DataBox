//! A pasta de trabalho: abre o pacote (zip), confere que é mesmo um `.xlsx`, e lê as partes pequenas
//! (`[Content_Types].xml`, `xl/workbook.xml` e suas relações, `xl/sharedStrings.xml`, `xl/styles.xml`). As
//! planilhas (`xl/worksheets/sheetN.xml`) só são lidas sob demanda, em fluxo.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use comum::{ErroBanco, Resultado, traduzir_io};
use zip::ZipArchive;

use crate::datas::SistemaDatas;
use crate::erros;
use crate::fluxo::Fluxo;
use crate::folha::LeitorLinhas;
use crate::formato::Formato;
use crate::planilha::Planilha;
use crate::valores::{Celula, Linha};
use crate::xml::{ErroXml, Evento, Leitor};

/// Se a aba aparece na lista de abas do Excel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Visivel,
    /// Escondida pelo usuário ("Ocultar"); o Excel a mostra de novo por "Reexibir".
    Oculta,
    /// Escondida por macro (`veryHidden`): só o editor de VBA a mostra.
    MuitoOculta,
}

/// O que a aba contém.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoFolha {
    /// Planilha com células.
    Planilha,
    /// Folha de gráfico (sem células).
    Grafico,
    /// Caixa de diálogo do Excel 5.
    Dialogo,
    /// Folha de macro do Excel 4.
    Macro,
}

/// Uma aba da pasta de trabalho, na ordem em que aparece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folha {
    pub nome: String,
    /// `sheetId` do arquivo.
    pub id: u32,
    pub estado: Estado,
    pub tipo: TipoFolha,
    /// Nome da parte no zip (`xl/worksheets/sheet1.xml`); vazio se o arquivo não tem a parte.
    pub parte: String,
    /// Tamanho do XML da aba, descomprimido, em bytes.
    pub tamanho_xml: u64,
}

/// As frases do `sharedStrings.xml`, todas num bloco só de memória (um `String` por frase custaria muito
/// em planilhas com milhões delas).
#[derive(Debug, Default)]
pub(crate) struct Textos {
    dados: String,
    fins: Vec<usize>,
}

impl Textos {
    fn empurrar(&mut self, texto: &str) {
        self.dados.push_str(texto);
        self.fins.push(self.dados.len());
    }

    pub fn len(&self) -> usize {
        self.fins.len()
    }

    pub fn get(&self, i: usize) -> Option<&str> {
        let fim = *self.fins.get(i)?;
        let inicio = if i == 0 { 0 } else { self.fins[i - 1] };
        self.dados.get(inicio..fim)
    }
}

/// Tudo o que as leituras de planilha precisam da pasta de trabalho.
#[derive(Debug)]
pub(crate) struct Interno {
    pub caminho: PathBuf,
    pub folhas: Vec<Folha>,
    pub sistema: SistemaDatas,
    pub aba_ativa: usize,
    pub textos: Textos,
    /// Os formatos distintos usados e, por índice de estilo (`s=` da célula), qual deles.
    pub formatos: Vec<Formato>,
    pub estilos: Vec<usize>,
    pub geral: Formato,
}

impl Interno {
    pub fn formato(&self, estilo: u32) -> &Formato {
        self.estilos.get(estilo as usize).and_then(|&i| self.formatos.get(i)).unwrap_or(&self.geral)
    }
}

/// Uma pasta de trabalho do Excel (`.xlsx`, `.xlsm`) aberta só para leitura. Guarda só o que é pequeno
/// (nomes das abas, frases compartilhadas, estilos); clonar é barato (compartilha tudo) e vale entre linhas de
/// execução. As células de cada aba são lidas pela [`Planilha`] ou por [`Pasta::linhas`].
#[derive(Debug, Clone)]
pub struct Pasta {
    pub(crate) interno: Arc<Interno>,
}

const ASSINATURA_OLE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

impl Pasta {
    /// Abre e confere o arquivo. Erros em português: arquivo vazio, protegido por senha, `.xls` antigo,
    /// `.xlsb`, outro tipo de documento, zip corrompido, parte essencial faltando.
    pub fn abrir(caminho: impl AsRef<Path>) -> Resultado<Pasta> {
        let caminho = caminho.as_ref();
        let mut arquivo = File::open(caminho)
            .map_err(|e| traduzir_io(&e, Some(caminho), Some("Não foi possível abrir a planilha.")))?;
        let tamanho = arquivo.metadata().map_err(|e| traduzir_io(&e, Some(caminho), None))?.len();
        if tamanho == 0 {
            return Err(erros::vazio());
        }
        let mut cabeca = [0u8; 8];
        let lidos = ler_ate_encher(&mut arquivo, &mut cabeca).map_err(|e| traduzir_io(&e, Some(caminho), None))?;
        if lidos == 8 && cabeca == ASSINATURA_OLE {
            return Err(classificar_ole(&mut arquivo));
        }
        arquivo.seek(SeekFrom::Start(0)).map_err(|e| traduzir_io(&e, Some(caminho), None))?;
        let mut zip = match ZipArchive::new(arquivo) {
            Ok(z) => z,
            Err(zip::result::ZipError::Io(e)) => return Err(traduzir_io(&e, Some(caminho), None)),
            Err(e) if cabeca.starts_with(b"PK") => return Err(erros::corrompido(e.to_string())),
            Err(_) => return Err(erros::nao_e_pacote()),
        };
        let nomes = Nomes::de(&zip);

        let tipos = ler_parte(&mut zip, &nomes, "[Content_Types].xml", tipos_de_conteudo)?.unwrap_or_default();
        let parte_livro = achar_livro(&mut zip, &nomes, &tipos)?;
        if parte_livro.to_lowercase().ends_with(".bin") {
            return Err(erros::xlsb());
        }
        if tipos.tipo_da_parte(&parte_livro).is_some_and(|t| t.contains("ms-excel.sheet.binary")) {
            return Err(erros::xlsb());
        }
        let livro = ler_parte(&mut zip, &nomes, &parte_livro, ler_livro)?.ok_or_else(erros::zip_sem_excel)?;
        if livro.folhas.is_empty() {
            return Err(erros::sem_planilhas());
        }

        let parte_norm = parte_livro.replace('\\', "/");
        let diretorio = diretorio_de(&parte_norm);
        let arquivo_rels = format!("{diretorio}_rels/{}.rels", nome_do_arquivo(&parte_norm));
        let relacoes = ler_parte(&mut zip, &nomes, &arquivo_rels, ler_relacoes)?.unwrap_or_default();
        let alvo_de = |sufixo: &str| -> Option<String> {
            relacoes.iter().find(|r| !r.externo && r.tipo.ends_with(sufixo)).map(|r| resolver(&diretorio, &r.alvo))
        };

        let mut folhas = Vec::with_capacity(livro.folhas.len());
        for (i, bruta) in livro.folhas.iter().enumerate() {
            let relacao = relacoes.iter().find(|r| !r.externo && !bruta.rid.is_empty() && r.id == bruta.rid);
            let (alvo, tipo) = match relacao {
                Some(r) => (resolver(&diretorio, &r.alvo), tipo_da_relacao(&r.tipo)),
                None if relacoes.is_empty() => {
                    (format!("{diretorio}worksheets/sheet{}.xml", i + 1), TipoFolha::Planilha)
                }
                None => (String::new(), TipoFolha::Planilha),
            };
            let parte = nomes.achar(&alvo).map(str::to_string).unwrap_or_default();
            let tamanho_xml = if parte.is_empty() { 0 } else { zip.by_name(&parte).map_or(0, |e| e.size()) };
            folhas.push(Folha {
                nome: bruta.nome.clone(),
                id: bruta.id,
                estado: bruta.estado,
                tipo,
                parte,
                tamanho_xml,
            });
        }

        let alvo_textos = alvo_de("/sharedStrings").unwrap_or_else(|| format!("{diretorio}sharedStrings.xml"));
        let textos = ler_parte(&mut zip, &nomes, &alvo_textos, ler_textos)?.unwrap_or_default();
        let alvo_estilos = alvo_de("/styles").unwrap_or_else(|| format!("{diretorio}styles.xml"));
        let (formatos, estilos) = ler_parte(&mut zip, &nomes, &alvo_estilos, ler_estilos)?.unwrap_or_default();

        Ok(Pasta {
            interno: Arc::new(Interno {
                caminho: caminho.to_path_buf(),
                folhas,
                sistema: livro.sistema,
                aba_ativa: livro.aba_ativa,
                textos,
                formatos,
                estilos,
                geral: Formato::geral(),
            }),
        })
    }

    pub fn caminho(&self) -> &Path {
        &self.interno.caminho
    }

    /// As abas, na ordem do Excel (inclusive as ocultas).
    pub fn folhas(&self) -> &[Folha] {
        &self.interno.folhas
    }

    /// Índice da primeira aba com células e visível (a que o Excel abre, se for uma delas); `None` se não há.
    pub fn primeira_planilha_visivel(&self) -> Option<usize> {
        let f = &self.interno.folhas;
        let ok = |i: &usize| f[*i].tipo == TipoFolha::Planilha && f[*i].estado == Estado::Visivel;
        let ativa = self.interno.aba_ativa;
        if ativa < f.len() && ok(&ativa) { Some(ativa) } else { (0..f.len()).find(ok) }
    }

    /// A aba que estava ativa quando o arquivo foi salvo.
    pub fn aba_ativa(&self) -> usize {
        self.interno.aba_ativa
    }

    /// 1900 (o normal) ou 1904 (`date1904`).
    pub fn sistema_de_datas(&self) -> SistemaDatas {
        self.interno.sistema
    }

    /// O formato de número de um estilo (`Celula::estilo`); estilo desconhecido é General.
    pub fn formato(&self, estilo: u32) -> &Formato {
        self.interno.formato(estilo)
    }

    /// O texto da célula como o Excel o mostra (formato de número aplicado, em português do Brasil).
    pub fn exibir(&self, celula: &Celula) -> String {
        self.formato(celula.estilo).exibir(&celula.valor)
    }

    /// Quantas frases há na tabela de textos compartilhados.
    pub fn textos_compartilhados(&self) -> usize {
        self.interno.textos.len()
    }

    /// Uma frase da tabela de textos compartilhados.
    pub fn texto_compartilhado(&self, i: usize) -> Option<&str> {
        self.interno.textos.get(i)
    }

    /// Prepara a leitura paginada da aba `folha` (a partir de 0): lê só o cabeçalho do XML. Para saber onde
    /// a planilha acaba e paginar rápido, chame depois [`Planilha::indexar`].
    pub fn planilha(&self, folha: usize) -> Resultado<Planilha> {
        Planilha::nova(self, folha)
    }

    /// Todas as linhas da aba, em ordem, em fluxo (sem guardar nada: serve para converter ou exportar).
    /// As linhas que não existem no XML não aparecem.
    pub fn linhas(&self, folha: usize) -> Resultado<Linhas> {
        let f = self.folha_com_celulas(folha)?;
        let fluxo = Fluxo::abrir(self.interno.caminho.clone(), f.parte.clone());
        Ok(Linhas { leitor: LeitorLinhas::novo(fluxo), pasta: self.clone(), parte: f.parte.clone(), fim: false })
    }

    pub(crate) fn folha_com_celulas(&self, indice: usize) -> Resultado<&Folha> {
        let f = self
            .interno
            .folhas
            .get(indice)
            .ok_or_else(|| erros::folha_inexistente(indice, self.interno.folhas.len()))?;
        if f.tipo != TipoFolha::Planilha {
            return Err(erros::sem_celulas(&f.nome));
        }
        if f.parte.is_empty() {
            return Err(erros::folha_sem_parte(&f.nome));
        }
        Ok(f)
    }
}

/// As linhas de uma aba, uma a uma. Termina no primeiro erro (devolvido uma vez).
pub struct Linhas {
    leitor: LeitorLinhas<Fluxo>,
    pasta: Pasta,
    parte: String,
    fim: bool,
}

impl Iterator for Linhas {
    type Item = Resultado<Linha>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fim {
            return None;
        }
        match self.leitor.proxima(Some(&self.pasta.interno)) {
            Ok(Some(l)) => Some(Ok(l.linha)),
            Ok(None) => {
                self.fim = true;
                None
            }
            Err(e) => {
                self.fim = true;
                Some(Err(erros::da_leitura(e, &self.parte)))
            }
        }
    }
}

// ---- reconhecimento de arquivos que não são .xlsx --------------------------------------------------------

fn ler_ate_encher(arq: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = arq.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

fn utf16(texto: &str) -> Vec<u8> {
    texto.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Um arquivo OLE (o contêiner do Office antigo e das planilhas protegidas por senha): procura os nomes de
/// fluxo, que ficam em UTF-16, por todo o arquivo.
fn classificar_ole(arq: &mut File) -> ErroBanco {
    let protegido = utf16("EncryptedPackage");
    let livro = utf16("Workbook");
    let livro_5 = utf16("Book");
    let (mut achou_livro, mut achou_livro_5) = (false, false);
    let longo = protegido.len();
    let mut janela: Vec<u8> = Vec::new();
    let mut bloco = vec![0u8; 1024 * 1024];
    let mut varridos: u64 = 0;
    if arq.seek(SeekFrom::Start(0)).is_err() {
        return erros::office_antigo();
    }
    loop {
        let n = match arq.read(&mut bloco) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        janela.extend_from_slice(&bloco[..n]);
        if contem(&janela, &protegido) {
            return erros::protegida();
        }
        achou_livro |= contem(&janela, &livro);
        achou_livro_5 |= contem(&janela, &livro_5);
        // guarda o fim, para um nome cortado entre dois blocos
        let manter = longo.min(janela.len());
        janela.drain(..janela.len() - manter);
        varridos += n as u64;
        if varridos > 512 * 1024 * 1024 {
            break;
        }
    }
    if achou_livro || achou_livro_5 { erros::xls_antigo() } else { erros::office_antigo() }
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// O zip abriu mas não tem uma pasta de trabalho: diz o que ele é.
fn classificar_zip(zip: &mut ZipArchive<File>, nomes: &Nomes) -> ErroBanco {
    if nomes.achar("word/document.xml").is_some() {
        return erros::zip_de_outro_tipo("um documento do Word (.docx)");
    }
    if nomes.achar("ppt/presentation.xml").is_some() {
        return erros::zip_de_outro_tipo("uma apresentação do PowerPoint (.pptx)");
    }
    if nomes.achar("xl/workbook.bin").is_some() {
        return erros::xlsb();
    }
    if let Some(real) = nomes.achar("mimetype").map(str::to_string) {
        let mut tipo = String::new();
        if let Ok(e) = zip.by_name(&real) {
            let _ = e.take(200).read_to_string(&mut tipo);
        }
        return erros::zip_de_outro_tipo(if tipo.contains("spreadsheet") {
            "uma planilha do LibreOffice (.ods)"
        } else {
            "um documento do formato OpenDocument"
        });
    }
    erros::zip_sem_excel()
}

// ---- nomes das partes e relações ---------------------------------------------------------------------------

/// Os nomes das partes do zip, comparados sem diferenciar maiúsculas e com `\` e `/` iguais (alguns programas
/// gravam do jeito errado).
struct Nomes {
    mapa: HashMap<String, String>,
}

fn chave(nome: &str) -> String {
    nome.replace('\\', "/").trim_start_matches('/').to_lowercase()
}

impl Nomes {
    fn de(zip: &ZipArchive<File>) -> Nomes {
        Nomes { mapa: zip.file_names().map(|n| (chave(n), n.to_string())).collect() }
    }

    fn achar(&self, caminho: &str) -> Option<&str> {
        if let Some(n) = self.mapa.get(&chave(caminho)) {
            return Some(n);
        }
        let decodificado = decodificar_percentual(caminho);
        self.mapa.get(&chave(&decodificado)).map(String::as_str)
    }
}

/// `%20` e afins nos alvos das relações.
fn decodificar_percentual(texto: &str) -> String {
    let b = texto.as_bytes();
    let mut saida = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
            && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
        {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("00");
            saida.push(u8::from_str_radix(hex, 16).unwrap_or(b'%'));
            i += 3;
        } else {
            saida.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&saida).into_owned()
}

fn diretorio_de(parte: &str) -> String {
    match parte.rfind('/') {
        Some(i) => parte[..=i].to_string(),
        None => String::new(),
    }
}

fn nome_do_arquivo(parte: &str) -> &str {
    parte.rsplit('/').next().unwrap_or(parte)
}

/// Alvo de uma relação, relativo à pasta da parte de origem (ou absoluto, com `/` na frente), sem `.` e `..`.
fn resolver(diretorio: &str, alvo: &str) -> String {
    let alvo = alvo.replace('\\', "/");
    let bruto =
        if let Some(absoluto) = alvo.strip_prefix('/') { absoluto.to_string() } else { format!("{diretorio}{alvo}") };
    let mut partes: Vec<&str> = Vec::new();
    for p in bruto.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                partes.pop();
            }
            p => partes.push(p),
        }
    }
    partes.join("/")
}

fn tipo_da_relacao(tipo: &str) -> TipoFolha {
    if tipo.ends_with("/chartsheet") {
        TipoFolha::Grafico
    } else if tipo.ends_with("/dialogsheet") {
        TipoFolha::Dialogo
    } else if tipo.ends_with("/xlMacrosheet") || tipo.ends_with("/xlIntlMacrosheet") {
        TipoFolha::Macro
    } else {
        TipoFolha::Planilha
    }
}

// ---- leitura das partes pequenas ------------------------------------------------------------------------

/// A fonte das partes pequenas: o fluxo descomprimido de uma entrada do zip.
type Fonte<'a> = Box<dyn Read + 'a>;

/// Lê uma parte em XML com `f`; `Ok(None)` se a parte não existe.
fn ler_parte<T>(
    zip: &mut ZipArchive<File>,
    nomes: &Nomes,
    caminho: &str,
    f: impl for<'a> FnOnce(&mut Leitor<Fonte<'a>>) -> Result<T, ErroXml>,
) -> Resultado<Option<T>> {
    let Some(real) = nomes.achar(caminho).map(str::to_string) else {
        return Ok(None);
    };
    let entrada = zip.by_name(&real).map_err(|e| erros::do_zip(&e, &real))?;
    let fonte: Fonte<'_> = Box::new(entrada);
    let mut leitor = Leitor::novo(fonte);
    f(&mut leitor).map(Some).map_err(|e| erros::da_leitura(e, &real))
}

#[derive(Debug, Default)]
struct TiposDeConteudo {
    padroes: HashMap<String, String>,
    sobrescritos: HashMap<String, String>,
}

impl TiposDeConteudo {
    fn tipo_da_parte(&self, parte: &str) -> Option<&str> {
        if let Some(t) = self.sobrescritos.get(&chave(parte)) {
            return Some(t);
        }
        let ext = parte.rsplit_once('.').map(|(_, e)| e.to_lowercase())?;
        self.padroes.get(&ext).map(String::as_str)
    }
}

fn tipos_de_conteudo(l: &mut Leitor<Fonte<'_>>) -> Result<TiposDeConteudo, ErroXml> {
    let mut t = TiposDeConteudo::default();
    loop {
        match l.avancar()? {
            Evento::Fim => return Ok(t),
            Evento::Abre => match l.nome() {
                "Default" => {
                    if let (Some(e), Some(c)) = (l.atributo("Extension"), l.atributo("ContentType")) {
                        t.padroes.insert(e.to_lowercase(), c.to_string());
                    }
                }
                "Override" => {
                    if let (Some(p), Some(c)) = (l.atributo("PartName"), l.atributo("ContentType")) {
                        t.sobrescritos.insert(chave(p), c.to_string());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
struct Relacao {
    id: String,
    tipo: String,
    alvo: String,
    externo: bool,
}

fn ler_relacoes(l: &mut Leitor<Fonte<'_>>) -> Result<Vec<Relacao>, ErroXml> {
    let mut saida = Vec::new();
    loop {
        match l.avancar()? {
            Evento::Fim => return Ok(saida),
            Evento::Abre if l.nome() == "Relationship" => {
                saida.push(Relacao {
                    id: l.atributo("Id").unwrap_or("").to_string(),
                    tipo: l.atributo("Type").unwrap_or("").to_string(),
                    alvo: l.atributo("Target").unwrap_or("").to_string(),
                    externo: l.atributo("TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("External")),
                });
            }
            _ => {}
        }
    }
}

/// Acha a parte da pasta de trabalho: pela relação `officeDocument` de `_rels/.rels`, pelo tipo de conteúdo ou
/// pelo nome de sempre.
fn achar_livro(zip: &mut ZipArchive<File>, nomes: &Nomes, tipos: &TiposDeConteudo) -> Resultado<String> {
    if let Some(rels) = ler_parte(zip, nomes, "_rels/.rels", ler_relacoes)? {
        let doc = rels.iter().find(|r| !r.externo && r.tipo.ends_with("/officeDocument"));
        if let Some(r) = doc {
            let alvo = resolver("", &r.alvo);
            if let Some(real) = nomes.achar(&alvo) {
                // o `.rels` de um .docx também aponta para o "documento": confere o tipo antes
                let de_outro_tipo = tipos
                    .tipo_da_parte(real)
                    .is_some_and(|t| !t.contains("spreadsheetml") && !t.contains("ms-excel") && t != "application/xml");
                if de_outro_tipo {
                    return Err(classificar_zip(zip, nomes));
                }
                return Ok(real.to_string());
            }
        }
    }
    let principal = tipos.sobrescritos.iter().find(|(_, t)| {
        (t.contains("spreadsheetml") && (t.ends_with("sheet.main+xml") || t.ends_with("template.main+xml")))
            || t.contains("macroEnabled.main")
            || t.contains("ms-excel.sheet.binary")
    });
    if let Some((parte, _)) = principal
        && let Some(real) = nomes.achar(parte)
    {
        return Ok(real.to_string());
    }
    for candidato in ["xl/workbook.xml", "xl/workbook.bin"] {
        if let Some(real) = nomes.achar(candidato) {
            return Ok(real.to_string());
        }
    }
    Err(classificar_zip(zip, nomes))
}

struct FolhaBruta {
    nome: String,
    id: u32,
    rid: String,
    estado: Estado,
}

struct Livro {
    folhas: Vec<FolhaBruta>,
    sistema: SistemaDatas,
    aba_ativa: usize,
}

fn verdadeiro(v: &str) -> bool {
    matches!(v.trim(), "1") || v.trim().eq_ignore_ascii_case("true")
}

fn ler_livro(l: &mut Leitor<Fonte<'_>>) -> Result<Livro, ErroXml> {
    let mut livro = Livro { folhas: Vec::new(), sistema: SistemaDatas::D1900, aba_ativa: 0 };
    loop {
        match l.avancar()? {
            Evento::Fim => return Ok(livro),
            Evento::Abre => match l.nome() {
                "sheet" => {
                    let estado = match l.atributo("state") {
                        Some("hidden") => Estado::Oculta,
                        Some("veryHidden") => Estado::MuitoOculta,
                        _ => Estado::Visivel,
                    };
                    let numero = livro.folhas.len() as u32 + 1;
                    livro.folhas.push(FolhaBruta {
                        nome: l.atributo("name").unwrap_or("").to_string(),
                        id: l.atributo("sheetId").and_then(|v| v.trim().parse().ok()).unwrap_or(numero),
                        rid: l.atributo("id").unwrap_or("").to_string(),
                        estado,
                    });
                }
                "workbookPr" => {
                    if l.atributo("date1904").is_some_and(verdadeiro) {
                        livro.sistema = SistemaDatas::D1904;
                    }
                }
                "workbookView" => {
                    if let Some(n) = l.atributo("activeTab").and_then(|v| v.trim().parse().ok()) {
                        livro.aba_ativa = n;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// `_xHHHH_` do OOXML (ST_Xstring): um caractere escapado; `_x005F_` é o sublinhado literal. Um par de
/// substitutos (`_xD83D__xDE00_`) vira um caractere só.
pub(crate) fn desescapar_x(texto: &str) -> Cow<'_, str> {
    if !texto.contains("_x") {
        return Cow::Borrowed(texto);
    }
    let b = texto.as_bytes();
    let hex4 = |i: usize| -> Option<u16> {
        let h = b.get(i + 2..i + 6)?;
        if b[i] == b'_' && b[i + 1] == b'x' && b.get(i + 6) == Some(&b'_') && h.iter().all(u8::is_ascii_hexdigit) {
            u16::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
        } else {
            None
        }
    };
    let mut saida = String::with_capacity(texto.len());
    let mut i = 0;
    let mut inicio = 0;
    while i < b.len() {
        if b[i] == b'_'
            && let Some(u) = hex4(i)
        {
            let (c, gasto) = if (0xD800..0xDC00).contains(&u) {
                match hex4(i + 7) {
                    Some(baixo) if (0xDC00..0xE000).contains(&baixo) => {
                        let cp = 0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(baixo) - 0xDC00);
                        (char::from_u32(cp).unwrap_or('\u{FFFD}'), 14)
                    }
                    _ => ('\u{FFFD}', 7),
                }
            } else {
                (char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'), 7)
            };
            saida.push_str(&texto[inicio..i]);
            saida.push(c);
            i += gasto;
            inicio = i;
            continue;
        }
        i += 1;
    }
    saida.push_str(&texto[inicio..]);
    Cow::Owned(saida)
}

/// `xl/sharedStrings.xml`: o texto de cada `<si>` (as partes `<t>` dos trechos formatados emendadas; o
/// fonético `<rPh>` fica de fora).
fn ler_textos(l: &mut Leitor<Fonte<'_>>) -> Result<Textos, ErroXml> {
    let mut textos = Textos::default();
    let mut item = String::new();
    let mut em_item = false;
    let mut em_t = false;
    let mut fonetico = 0usize;
    loop {
        match l.avancar()? {
            Evento::Fim => return Ok(textos),
            Evento::Abre => match l.nome() {
                "si" => {
                    em_item = true;
                    item.clear();
                }
                "t" if em_item && fonetico == 0 => em_t = true,
                "rPh" => fonetico += 1,
                _ => {}
            },
            Evento::Texto => {
                if em_t {
                    item.push_str(l.texto());
                }
            }
            Evento::Fecha => match l.nome() {
                "t" => em_t = false,
                "rPh" => fonetico = fonetico.saturating_sub(1),
                "si" => {
                    em_item = false;
                    textos.empurrar(&desescapar_x(&item));
                }
                _ => {}
            },
        }
    }
}

/// `xl/styles.xml`: os formatos de número (`numFmts`) e, por `cellXfs`, o formato de cada estilo.
fn ler_estilos(l: &mut Leitor<Fonte<'_>>) -> Result<(Vec<Formato>, Vec<usize>), ErroXml> {
    let mut codigos: HashMap<u32, String> = HashMap::new();
    let mut xfs: Vec<u32> = Vec::new();
    let mut em_xfs = false;
    loop {
        match l.avancar()? {
            Evento::Fim => break,
            Evento::Abre => match l.nome() {
                "numFmt" => {
                    if let (Some(id), Some(codigo)) = (l.atributo("numFmtId"), l.atributo("formatCode"))
                        && let Ok(id) = id.trim().parse()
                    {
                        codigos.insert(id, codigo.to_string());
                    }
                }
                "cellXfs" => em_xfs = true,
                "xf" if em_xfs => {
                    xfs.push(l.atributo("numFmtId").and_then(|v| v.trim().parse().ok()).unwrap_or(0));
                }
                _ => {}
            },
            Evento::Fecha if l.nome() == "cellXfs" => em_xfs = false,
            _ => {}
        }
    }
    let mut formatos: Vec<Formato> = Vec::new();
    let mut por_id: HashMap<u32, usize> = HashMap::new();
    let estilos = xfs
        .iter()
        .map(|&id| {
            *por_id.entry(id).or_insert_with(|| {
                formatos.push(Formato::de_id(id, codigos.get(&id).map(String::as_str)));
                formatos.len() - 1
            })
        })
        .collect();
    Ok((formatos, estilos))
}

#[cfg(test)]
mod testes_internos {
    use super::*;

    #[test]
    fn resolve_alvos() {
        assert_eq!(resolver("xl/", "worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(resolver("xl/", "/xl/worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(resolver("xl/worksheets/", "../sharedStrings.xml"), "xl/sharedStrings.xml");
        assert_eq!(resolver("", "xl/workbook.xml"), "xl/workbook.xml");
        assert_eq!(resolver("xl/", ".\\worksheets\\sheet2.xml"), "xl/worksheets/sheet2.xml");
        assert_eq!(resolver("xl/", "../../x.xml"), "x.xml");
    }

    #[test]
    fn percentuais_nos_alvos() {
        assert_eq!(decodificar_percentual("a%20b%2Fc"), "a b/c");
        assert_eq!(decodificar_percentual("100%"), "100%");
        assert_eq!(decodificar_percentual("%zz%4"), "%zz%4");
    }

    #[test]
    fn diretorio_e_nome() {
        assert_eq!(diretorio_de("xl/workbook.xml"), "xl/");
        assert_eq!(diretorio_de("workbook.xml"), "");
        assert_eq!(nome_do_arquivo("xl/workbook.xml"), "workbook.xml");
        assert_eq!(nome_do_arquivo("workbook.xml"), "workbook.xml");
    }

    #[test]
    fn escapes_x_do_ooxml() {
        assert_eq!(desescapar_x("sem nada"), "sem nada");
        assert!(matches!(desescapar_x("sem nada"), Cow::Borrowed(_)));
        assert_eq!(desescapar_x("a_x000D_b"), "a\rb");
        assert_eq!(desescapar_x("_x005F_x0041_"), "_x0041_");
        assert_eq!(desescapar_x("_x0041_"), "A");
        assert_eq!(desescapar_x("_x00e9_"), "é");
        assert_eq!(desescapar_x("_xD83D__xDE00_"), "😀");
        assert_eq!(desescapar_x("_xD83D_ sozinho"), "\u{FFFD} sozinho");
        assert_eq!(desescapar_x("_x12_ _xZZZZ_ _x0041"), "_x12_ _xZZZZ_ _x0041");
        assert_eq!(desescapar_x("x_x0009_y_x000A_z"), "x\ty\nz");
        assert_eq!(desescapar_x("__x0041_"), "_A");
    }

    #[test]
    fn textos_em_um_bloco() {
        let mut t = Textos::default();
        t.empurrar("um");
        t.empurrar("");
        t.empurrar("três");
        assert_eq!(t.len(), 3);
        assert_eq!(t.get(0), Some("um"));
        assert_eq!(t.get(1), Some(""));
        assert_eq!(t.get(2), Some("três"));
        assert_eq!(t.get(3), None);
    }

    #[test]
    fn tipos_pelo_padrao_e_pelo_override() {
        let mut t = TiposDeConteudo::default();
        t.padroes.insert("xml".into(), "application/xml".into());
        t.sobrescritos.insert(chave("/xl/workbook.xml"), "wb".into());
        assert_eq!(t.tipo_da_parte("xl/workbook.xml"), Some("wb"));
        assert_eq!(t.tipo_da_parte("XL\\Workbook.XML"), Some("wb"));
        assert_eq!(t.tipo_da_parte("xl/outro.xml"), Some("application/xml"));
        assert_eq!(t.tipo_da_parte("xl/semextensao"), None);
    }
}
