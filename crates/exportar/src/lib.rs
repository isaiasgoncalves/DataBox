//! Exportação de linhas para Excel (.xlsx), CSV e JSON, sempre em fluxo.
//!
//! O crate não conhece o banco: recebe nomes de colunas e linhas de [`Valor`] e grava o arquivo. Quem chama
//! (o `servico`) decide de onde vêm as linhas: uma tabela com filtro, uma consulta livre ou o banco inteiro.
//!
//! # API
//!
//! ```text
//! pub enum Formato { Xlsx, Csv, Json }          como_str() → "xlsx"|"csv"|"json";  Formato::de_texto(".XLSX")?
//! pub struct Opcoes { cabecalho, csv_separador: char, csv_decimal: char, json_indentado, csv_proteger_formulas }
//!     Default: true, ';', ',', true, true.  conferir() → as regras (separador ; , \t · decimal , .)
//! pub fn separador_csv("tab" | "\\t" | "\t" | ";" | ",") -> Resultado<char>   (texto das preferências → char)
//! pub fn decimal_csv("," | ".") -> Resultado<char>
//! pub struct Meta { banco: Option<String>, exportado_em: Option<String> }   (JSON do banco inteiro)
//! pub struct Resumo { caminho, formato, linhas: u64, bytes: u64, tabelas: usize, tempo_ms: u64, avisos: Vec<String> }
//!     nome() → nome do arquivo gravado
//! pub struct Acompanhamento { ao_progresso, ao_tabela, ao_finalizar }   (callbacks Send que devolvem Resultado)
//!     Acompanhamento::default().ao_progresso(|linhas| Ok(())).ao_tabela(|nome, feitas| Ok(())).ao_finalizar(|| Ok(()))
//!
//! pub struct Escritor                            (Send; gravação empurrada)
//!     Escritor::novo(formato, &destino, &opcoes, varias: bool, meta: Option<Meta>, acomp) -> Resultado<Escritor>
//!     .comecar_tabela(nome, &colunas)  .linha(&[Valor])  .terminar_tabela()  .concluir() -> Resultado<Resumo>
//!     .destino() (o arquivo que vai ser gravado de fato)
//!     .linhas() (gravadas no arquivo até agora; no xlsx, a amostra das larguras, até 200 linhas por fonte, só
//!               entra na conta quando vai para a planilha, como no `ao_progresso`)
//! pub fn exportar_linhas(&colunas, linhas, formato, &destino, &opcoes, nome, acomp) -> Resultado<Resumo>
//! pub fn exportar_varias(fontes, formato, &destino, &opcoes, meta, acomp) -> Resultado<Resumo>
//!     linhas: IntoIterator<Item = Resultado<L>>, L: AsRef<[Valor]>;  fontes: IntoIterator<Item = (nome, colunas, linhas)>
//! pub fn destino_final(formato, &destino, varias) -> Resultado<PathBuf>   (banco inteiro em CSV → .zip)
//! pub fn nome_planilha(nome, &mut HashSet<String>) -> String
//! pub fn erro_de_leitura(ErroBanco) -> ErroBanco   (falha da fonte de linhas → "Não foi possível ler os dados…")
//! pub const PASSO_PROGRESSO = 1000; LIMITE_LINHAS_EXCEL = 1_048_575; LIMITE_TEXTO_EXCEL = 32_767;
//!           AMOSTRA_LARGURA = 200; LARGURA_MINIMA = 8; LARGURA_MAXIMA = 60
//! ```
//!
//! # Como usar (o servico)
//!
//! ```text
//! let mut esc = Escritor::novo(formato, &destino, &opcoes, varias, Some(meta), acomp)?;
//! for tabela in tabelas {                         // uma só, com varias = false
//!     banco.iterar_tabela(&tabela, &filtro, parar, |colunas, linhas| {
//!         esc.comecar_tabela(&tabela, colunas)?;
//!         for linha in linhas { esc.linha(&linha.map_err(erro_de_leitura)?)?; }
//!         esc.terminar_tabela()
//!     })?;
//! }
//! let resumo = esc.concluir()?;
//! ```
//!
//! - `varias = false` (uma tabela ou consulta): uma fonte só; o `nome` dela só serve de nome da planilha no
//!   Excel; o JSON é uma lista de objetos. `concluir` sem nenhuma `comecar_tabela` grava uma fonte "dados" vazia.
//! - `varias = true` (banco inteiro): xlsx com uma planilha por fonte (nomes saneados e únicos); json
//!   `{"banco", "exportado_em", "tabelas": {nome: [objetos]}}`; csv num .zip com um CSV por fonte. Destino .csv
//!   (ou sem extensão) passa a .zip, e se esse .zip já existe é 409 `Codigo::DestinoExiste` (a pessoa
//!   confirmou substituir o .csv, não o .zip).
//!
//! # Regras
//!
//! - Tudo é gravado num temporário na pasta do destino ("~nome.ext.xxxxxxxx.tmp") e trocado no fim
//!   (`std::fs::rename`, que no Windows substitui o destino). Em qualquer erro, em
//!   qualquer callback que devolva `Err` e no `Drop` de um [`Escritor`] não concluído, o temporário é apagado
//!   e o destino fica como estava. O erro de um callback sobe **como veio** (é assim que se cancela).
//! - Nada da tabela inteira fica na memória: no Excel, só as primeiras [`AMOSTRA_LARGURA`] linhas de cada fonte
//!   (para estimar as larguras, que vêm antes das linhas no XML), e já na forma da célula (BLOB grande vira o
//!   marcador, texto cortado em 32.767 caracteres), nunca o valor inteiro; o .xlsx é comprimido enquanto as
//!   linhas chegam; o CSV de cada tabela do .zip passa por um arquivo de trabalho na pasta temporária do
//!   sistema ("DataBox-exportar-xxxxxxxx.tmp"), que some sozinho: no
//!   Windows é aberto com FILE_FLAG_DELETE_ON_CLOSE, e o próprio sistema o apaga ao fechar o
//!   handle, mesmo se o processo morrer no meio (janela fechada com a exportação rodando, Gerenciador de
//!   Tarefas). O temporário ao lado do destino ("~…tmp") sobra nesse caso.
//! - Valores: NULL → vazio (CSV/Excel) / `null` (JSON); BLOB → base64; infinito → "Infinity"/"-Infinity";
//!   NaN → vazio/`null`; real no CSV no texto mais curto que volta ao mesmo número e a vírgula decimal pedida (só em REAL).
//! - Excel: inteiro fora de ±(10^15 − 1) vai como texto; texto é sempre texto ("=…" nunca vira fórmula, "#N/A"
//!   nunca vira erro); caracteres de controle ilegais são removidos; texto acima de 32.767 caracteres é
//!   cortado; BLOB cujo base64 não cabe vira "[BLOB de N bytes: …]"; real fora da faixa do Excel vai como
//!   texto. Cada caso é contado em [`Resumo::avisos`]. Cabeçalho em
//!   negrito branco sobre o navy #133C63, primeira linha congelada, autofiltro, larguras pelas 200 primeiras
//!   linhas (8 a 60); mais de 1.048.575 linhas continuam em "nome (2)", "nome (3)"…
//! - CSV: UTF-8 com BOM, "\r\n", aspas só quando preciso; com `csv_proteger_formulas`, texto que começa com
//!   `= + - @`, tabulação ou CR ganha um apóstrofo na frente (menos número guardado como texto: "-5", "+1,5").
//! - Progresso: `ao_progresso(linhas)` a cada [`PASSO_PROGRESSO`] linhas (somando as fontes), a cada bloco
//!   de 1 MB de cada CSV comprimido no .zip e uma última vez com o arquivo pronto, antes da troca;
//!   `ao_tabela(nome, feitas)` ao começar cada fonte; `ao_finalizar()` quando as linhas acabaram.
//! - Erro de disco → "Não foi possível gravar o arquivo exportado. …" (403 para arquivo aberto em outro
//!   programa); sem permissão na pasta → 403 "Sem permissão para gravar na pasta …". Falha da fonte de
//!   linhas nos atalhos passa por [`erro_de_leitura`]. Para cancelar, o callback devolve o erro que a tela
//!   espera (ex.: 409 `Codigo::Cancelado`, "Exportação interrompida; nenhum arquivo foi gravado.").
//!
//! # O .xlsx
//!
//! Gravado por este crate (SpreadsheetML em fluxo, comprimido pelo crate `zip`), e não pelo `rust_xlsxwriter`:
//! o manifesto do workspace declara o `rust_xlsxwriter` sem a feature `constant_memory`, e sem ela ele guarda
//! todas as células na memória até o `save`. O pacote segue o do openpyxl em modo write-only (conferido
//! célula a célula e aberto no Excel 16 sem reparo): texto em `inlineStr`, estilo 1 no cabeçalho,
//! `_xlnm._FilterDatabase` para o autofiltro. Como as planilhas vão comprimidas enquanto as linhas chegam, a
//! finalização do .xlsx é curta (só as partes pequenas e o índice do zip), sem a "montagem" demorada do
//! openpyxl. Uma planilha cujo XML passa de 3,5 GB continua em "nome (2)" antes do limite de linhas, para
//! nenhuma parte do pacote precisar de ZIP64.

#![forbid(unsafe_code)]

mod arquivo;
mod csv;
mod json;
mod valores;
mod xlsx;

#[cfg(test)]
mod testes;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{Datelike, Timelike};
use comum::{Codigo, ErroBanco, Resultado, Valor};

pub use valores::LIMITE_TEXTO_EXCEL;
pub use xlsx::nome_planilha;

use arquivo::{CONTEXTO_GRAVAR, conferir_destino, erro_io};
use valores::{Avisos, citado};

/// `ao_progresso` é chamado a cada PASSO_PROGRESSO linhas gravadas.
pub const PASSO_PROGRESSO: u64 = 1000;
/// Linhas de dados por planilha: com o cabeçalho, dá o máximo do Excel (1.048.576 linhas).
pub const LIMITE_LINHAS_EXCEL: u64 = 1_048_575;
/// Linhas do começo de cada fonte usadas para estimar a largura das colunas no Excel.
pub const AMOSTRA_LARGURA: usize = 200;
pub const LARGURA_MINIMA: u32 = 8;
pub const LARGURA_MAXIMA: u32 = 60;

/// Tamanho dos buffers de gravação e dos blocos copiados para o .zip.
pub(crate) const BLOCO_COPIA: usize = 1 << 20;
/// O XML de uma planilha que passa disto continua na seguinte (cada parte do zip sem ZIP64 vai até 4 GiB;
/// a folga cobre a maior linha possível).
const LIMITE_BYTES_FOLHA: u64 = 3_500_000_000;

const CONTEXTO_LER: &str = "Não foi possível ler os dados para exportar.";

// ---------------------------------------------------------------------------------------------------
// Tipos públicos
// ---------------------------------------------------------------------------------------------------

/// Formato de exportação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Formato {
    Xlsx,
    Csv,
    Json,
}

impl Formato {
    /// Os três, na ordem da janela Exportar.
    pub const TODOS: [Formato; 3] = [Formato::Xlsx, Formato::Csv, Formato::Json];

    /// "xlsx", "csv" ou "json".
    pub fn como_str(self) -> &'static str {
        match self {
            Formato::Xlsx => "xlsx",
            Formato::Csv => "csv",
            Formato::Json => "json",
        }
    }

    /// O `_conferir_formato`: sem espaços, sem diferenciar maiúsculas e sem o ponto (".XLSX" vale).
    pub fn de_texto(texto: &str) -> Resultado<Formato> {
        match texto.trim().to_lowercase().trim_start_matches('.') {
            "xlsx" => Ok(Formato::Xlsx),
            "csv" => Ok(Formato::Csv),
            "json" => Ok(Formato::Json),
            _ => Err(ErroBanco::novo(format!("Formato de exportação desconhecido: {texto}. Use xlsx, csv ou json."))),
        }
    }
}

impl fmt::Display for Formato {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

/// Opções de exportação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opcoes {
    /// CSV: primeira linha com os nomes das colunas.
    pub cabecalho: bool,
    /// `;`, `,` ou tabulação.
    pub csv_separador: char,
    /// `,` ou `.` (só números REAL; inteiros não mudam).
    pub csv_decimal: char,
    pub json_indentado: bool,
    /// CSV: `'` antes de texto que começa com = + - @, tabulação ou CR (injeção de fórmula).
    pub csv_proteger_formulas: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Opcoes {
            cabecalho: true,
            csv_separador: ';',
            csv_decimal: ',',
            json_indentado: true,
            csv_proteger_formulas: true,
        }
    }
}

impl Opcoes {
    /// O `_conferir_opcoes`: separador `;` `,` ou tabulação; decimal `,` ou `.`.
    pub fn conferir(&self) -> Resultado<Opcoes> {
        if !matches!(self.csv_separador, ';' | ',' | '\t') {
            return Err(erro_separador(&self.csv_separador.to_string()));
        }
        if !matches!(self.csv_decimal, ',' | '.') {
            return Err(erro_decimal(&self.csv_decimal.to_string()));
        }
        Ok(self.clone())
    }
}

fn erro_separador(texto: &str) -> ErroBanco {
    ErroBanco::novo("Separador de CSV inválido. Use ponto e vírgula, vírgula ou tabulação.").detalhe(citado(texto))
}

fn erro_decimal(texto: &str) -> ErroBanco {
    ErroBanco::novo("Separador decimal inválido. Use vírgula ou ponto.").detalhe(citado(texto))
}

/// Separador de CSV a partir do texto das preferências ou do pedido: ";", ",", "\t", "tab" ou "\\t".
pub fn separador_csv(texto: &str) -> Resultado<char> {
    match texto {
        ";" => Ok(';'),
        "," => Ok(','),
        "\t" | "tab" | "\\t" => Ok('\t'),
        outro => Err(erro_separador(outro)),
    }
}

/// Separador decimal a partir do texto das preferências ou do pedido: "," ou ".".
pub fn decimal_csv(texto: &str) -> Resultado<char> {
    match texto {
        "," => Ok(','),
        "." => Ok('.'),
        outro => Err(erro_decimal(outro)),
    }
}

/// O cabeçalho do JSON do banco inteiro. `exportado_em` vazio = agora (hora local, "2026-09-22T10:31:00").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub banco: Option<String>,
    pub exportado_em: Option<String>,
}

/// O que foi gravado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumo {
    /// O arquivo gravado de fato (no banco inteiro em CSV, o .zip).
    pub caminho: PathBuf,
    pub formato: Formato,
    pub linhas: u64,
    pub bytes: u64,
    pub tabelas: usize,
    pub tempo_ms: u64,
    /// Frases: células que o Excel não comportou como estavam (cortadas, trocadas, como texto).
    pub avisos: Vec<String>,
}

impl Resumo {
    /// O nome do arquivo gravado ("vendas.xlsx").
    pub fn nome(&self) -> String {
        self.caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

pub type AoProgresso = Box<dyn FnMut(u64) -> Resultado<()> + Send>;
pub type AoTabela = Box<dyn FnMut(&str, usize) -> Resultado<()> + Send>;
pub type AoFinalizar = Box<dyn FnMut() -> Resultado<()> + Send>;

/// Quem acompanha a exportação. Um callback que devolve `Err` interrompe: o temporário é apagado, o destino
/// fica como estava e o erro sobe como veio.
#[derive(Default)]
pub struct Acompanhamento {
    /// Total de linhas gravadas até ali (somando as fontes): a cada [`PASSO_PROGRESSO`] linhas, de novo com o
    /// mesmo total a cada bloco de 1 MB de cada CSV comprimido no .zip, e uma última vez com o arquivo
    /// pronto, antes de ele tomar o lugar do destino.
    pub ao_progresso: Option<AoProgresso>,
    /// Ao começar cada fonte: o nome dela e quantas já foram gravadas antes.
    pub ao_tabela: Option<AoTabela>,
    /// As linhas acabaram e o arquivo está sendo fechado.
    pub ao_finalizar: Option<AoFinalizar>,
}

impl Acompanhamento {
    pub fn ao_progresso(mut self, f: impl FnMut(u64) -> Resultado<()> + Send + 'static) -> Self {
        self.ao_progresso = Some(Box::new(f));
        self
    }

    pub fn ao_tabela(mut self, f: impl FnMut(&str, usize) -> Resultado<()> + Send + 'static) -> Self {
        self.ao_tabela = Some(Box::new(f));
        self
    }

    pub fn ao_finalizar(mut self, f: impl FnMut() -> Resultado<()> + Send + 'static) -> Self {
        self.ao_finalizar = Some(Box::new(f));
        self
    }
}

impl fmt::Debug for Acompanhamento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Acompanhamento")
            .field("ao_progresso", &self.ao_progresso.is_some())
            .field("ao_tabela", &self.ao_tabela.is_some())
            .field("ao_finalizar", &self.ao_finalizar.is_some())
            .finish()
    }
}

/// Falha da fonte de linhas (o `Err` do iterador do banco): sobe com "Não foi possível ler os dados para
/// exportar." antes da frase. Erro com `codigo` (cancelado, substituído…)
/// sobe como veio.
pub fn erro_de_leitura(erro: ErroBanco) -> ErroBanco {
    if erro.codigo.is_some() || erro.mensagem.starts_with(CONTEXTO_LER) {
        erro
    } else {
        erro.com_contexto(CONTEXTO_LER)
    }
}

/// O arquivo que a exportação grava de fato: o banco inteiro em CSV (`varias`) vira um .zip.
///
/// Destino terminado em .csv (ou sem extensão) passa a .zip; o resto fica como veio. O destino é conferido
/// como na exportação (caminho vazio, pasta que não existe, destino que é uma pasta).
pub fn destino_final(formato: Formato, destino: &Path, varias: bool) -> Resultado<PathBuf> {
    let caminho = conferir_destino(destino)?;
    let sufixo = caminho.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase()));
    if varias && formato == Formato::Csv && matches!(sufixo.as_deref(), None | Some(".csv")) {
        return conferir_destino(&caminho.with_extension("zip"));
    }
    Ok(caminho)
}

// ---------------------------------------------------------------------------------------------------
// Acompanhamento e limites internos
// ---------------------------------------------------------------------------------------------------

/// Conta as linhas gravadas (somando todas as fontes) e chama os callbacks de quem acompanha.
pub(crate) struct Progresso {
    pub linhas: u64,
    proximo: u64,
    acomp: Acompanhamento,
}

impl Progresso {
    fn novo(acomp: Acompanhamento) -> Progresso {
        Progresso { linhas: 0, proximo: PASSO_PROGRESSO, acomp }
    }

    pub fn mais_uma(&mut self) -> Resultado<()> {
        self.linhas += 1;
        if self.linhas >= self.proximo {
            self.proximo = self.linhas + PASSO_PROGRESSO;
            self.avisar()?;
        }
        Ok(())
    }

    pub fn avisar(&mut self) -> Resultado<()> {
        match self.acomp.ao_progresso.as_mut() {
            Some(f) => f(self.linhas),
            None => Ok(()),
        }
    }

    fn tabela(&mut self, nome: &str, feitas: usize) -> Resultado<()> {
        match self.acomp.ao_tabela.as_mut() {
            Some(f) => f(nome, feitas),
            None => Ok(()),
        }
    }

    fn finalizar(&mut self) -> Resultado<()> {
        match self.acomp.ao_finalizar.as_mut() {
            Some(f) => f(),
            None => Ok(()),
        }
    }
}

/// Limites que os testes diminuem e a pasta dos arquivos de trabalho, que os
/// testes apontam para uma pasta própria para conferir que nada sobra nela.
#[derive(Debug, Clone)]
pub(crate) struct Limites {
    pub amostra_largura: usize,
    pub linhas_excel: u64,
    pub bytes_folha: u64,
    /// Onde fica o CSV de cada tabela antes de entrar no .zip; `None` = a pasta temporária do sistema.
    pub pasta_trabalho: Option<PathBuf>,
}

impl Default for Limites {
    fn default() -> Self {
        Limites {
            amostra_largura: AMOSTRA_LARGURA,
            linhas_excel: LIMITE_LINHAS_EXCEL,
            bytes_folha: LIMITE_BYTES_FOLHA,
            pasta_trabalho: None,
        }
    }
}

/// O que os gravadores de cada formato usam além do próprio arquivo.
pub(crate) struct Contexto {
    pub progresso: Progresso,
    pub avisos: Avisos,
    pub limites: Limites,
    pub opcoes: Opcoes,
}

pub(crate) fn erro_sem_tabela() -> ErroBanco {
    ErroBanco::com_status("Exportação mal conduzida: linha gravada fora de uma tabela (falta comecar_tabela).", 500)
}

// ---------------------------------------------------------------------------------------------------
// Escritor
// ---------------------------------------------------------------------------------------------------

/// O gravador de cada formato.
enum Saida {
    Csv(csv::CsvArquivo),
    ZipCsv(Box<csv::ZipCsv>),
    Json(json::JsonArquivo),
    Xlsx(Box<xlsx::Livro>),
}

impl Saida {
    fn abrir(
        formato: Formato,
        varias: bool,
        arquivo: fs::File,
        meta: Option<Meta>,
        opcoes: &Opcoes,
    ) -> Resultado<Saida> {
        Ok(match formato {
            Formato::Xlsx => Saida::Xlsx(Box::new(xlsx::Livro::abrir(arquivo, momento_zip()))),
            Formato::Csv if varias => Saida::ZipCsv(Box::new(csv::ZipCsv::abrir(arquivo, momento_zip()))),
            Formato::Csv => Saida::Csv(csv::CsvArquivo::abrir(arquivo)?),
            Formato::Json => Saida::Json(json::JsonArquivo::abrir(arquivo, varias, opcoes.json_indentado, meta)?),
        })
    }

    fn comecar(&mut self, nome: &str, colunas: &[String], ctx: &mut Contexto) -> Resultado<()> {
        match self {
            Saida::Csv(s) => s.comecar(colunas, ctx),
            Saida::ZipCsv(s) => s.comecar(nome, colunas, ctx),
            Saida::Json(s) => s.comecar(nome, colunas),
            Saida::Xlsx(s) => {
                s.comecar(nome, colunas);
                Ok(())
            }
        }
    }

    fn linha(&mut self, valores: &[Valor], ctx: &mut Contexto) -> Resultado<()> {
        match self {
            Saida::Csv(s) => s.linha(valores, ctx),
            Saida::ZipCsv(s) => s.linha(valores, ctx),
            Saida::Json(s) => s.linha(valores, ctx),
            Saida::Xlsx(s) => s.linha(valores, ctx),
        }
    }

    fn terminar(&mut self, ctx: &mut Contexto) -> Resultado<()> {
        match self {
            Saida::Csv(_) => Ok(()),
            Saida::ZipCsv(s) => s.terminar(ctx),
            Saida::Json(s) => s.terminar(),
            Saida::Xlsx(s) => s.terminar(ctx),
        }
    }

    fn fechar(self) -> Resultado<()> {
        match self {
            Saida::Csv(s) => s.fechar(),
            Saida::ZipCsv(s) => s.fechar(),
            Saida::Json(s) => s.fechar(),
            Saida::Xlsx(s) => s.fechar(),
        }
    }

    fn abortar(self) {
        match self {
            Saida::Csv(s) => s.abortar(),
            Saida::ZipCsv(s) => s.abortar(),
            Saida::Json(s) => s.abortar(),
            Saida::Xlsx(s) => s.abortar(),
        }
    }
}

/// Data e hora local para as entradas do zip.
fn momento_zip() -> zip::DateTime {
    let agora = chrono::Local::now();
    let parte = |v: u32| u8::try_from(v).unwrap_or(0);
    zip::DateTime::from_date_and_time(
        u16::try_from(agora.year()).unwrap_or(1980),
        parte(agora.month()),
        parte(agora.day()),
        parte(agora.hour()),
        parte(agora.minute()),
        parte(agora.second()),
    )
    .unwrap_or_default()
}

/// Gravação empurrada de uma exportação: [`Escritor::novo`], depois, para cada fonte,
/// [`comecar_tabela`](Escritor::comecar_tabela), [`linha`](Escritor::linha)… e
/// [`terminar_tabela`](Escritor::terminar_tabela); no fim, [`concluir`](Escritor::concluir).
///
/// Grava num temporário ao lado do destino; `concluir` troca o destino por ele. Qualquer erro (de disco, de
/// callback) apaga o temporário na hora e o mesmo erro volta nas chamadas seguintes; descartar o escritor
/// sem concluir também apaga o temporário.
pub struct Escritor {
    formato: Formato,
    destino: PathBuf,
    temporario: PathBuf,
    varias: bool,
    inicio: Instant,
    ctx: Contexto,
    tabelas: usize,
    tabela_aberta: bool,
    saida: Option<Saida>,
    falha: Option<ErroBanco>,
    concluido: bool,
}

impl fmt::Debug for Escritor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Escritor")
            .field("formato", &self.formato)
            .field("destino", &self.destino)
            .field("linhas", &self.ctx.progresso.linhas)
            .field("tabelas", &self.tabelas)
            .finish()
    }
}

impl Escritor {
    /// Confere as opções e o destino (no banco inteiro em CSV, o .zip; se ele já existe e não foi o
    /// escolhido, 409 `destino_existe`) e cria o temporário. No JSON do banco inteiro, `meta` vai no começo.
    pub fn novo(
        formato: Formato,
        destino: &Path,
        opcoes: &Opcoes,
        varias: bool,
        meta: Option<Meta>,
        acomp: Acompanhamento,
    ) -> Resultado<Escritor> {
        let inicio = Instant::now();
        let opcoes = opcoes.conferir()?;
        let pedido = conferir_destino(destino)?;
        let destino = if varias { destino_final(formato, &pedido, true)? } else { pedido.clone() };
        if destino != pedido && destino.exists() {
            let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Err(ErroBanco::com_status(
                format!(
                    "O banco inteiro em CSV é gravado num .zip, e já existe {nome} nesta pasta; escolha outro nome."
                ),
                409,
            )
            .detalhe(destino.display().to_string())
            .codigo(Codigo::DestinoExiste));
        }
        let (temporario, arquivo) = arquivo::criar_temporario(&destino)?;
        let mut escritor = Escritor {
            formato,
            destino,
            temporario,
            varias,
            inicio,
            ctx: Contexto {
                progresso: Progresso::novo(acomp),
                avisos: Avisos::default(),
                limites: Limites::default(),
                opcoes,
            },
            tabelas: 0,
            tabela_aberta: false,
            saida: None,
            falha: None,
            concluido: false,
        };
        match Saida::abrir(formato, varias, arquivo, meta, &escritor.ctx.opcoes) {
            Ok(saida) => {
                escritor.saida = Some(saida);
                Ok(escritor)
            }
            Err(e) => Err(escritor.falhar(e)),
        }
    }

    /// Os limites que os testes diminuem (antes da primeira tabela).
    #[cfg(test)]
    pub(crate) fn com_limites(mut self, limites: Limites) -> Escritor {
        self.ctx.limites = limites;
        self
    }

    /// O arquivo que vai ser gravado (no banco inteiro em CSV, o .zip).
    pub fn destino(&self) -> &Path {
        &self.destino
    }

    /// Linhas gravadas no arquivo até agora (somando as fontes): a mesma conta que chega ao `ao_progresso`.
    ///
    /// No Excel, as primeiras linhas de cada fonte (até [`AMOSTRA_LARGURA`]) ficam guardadas para estimar as
    /// larguras e só entram na conta quando vão para a planilha: ao chegar a linha seguinte à amostra ou em
    /// [`terminar_tabela`](Escritor::terminar_tabela) / [`concluir`](Escritor::concluir). Uma fonte de 150
    /// linhas em Excel mostra 0 até terminar Não é o número de linhas
    /// recebidas; para isso, conte as chamadas a [`linha`](Escritor::linha).
    pub fn linhas(&self) -> u64 {
        self.ctx.progresso.linhas
    }

    /// Começa uma fonte (termina a anterior, se ficou aberta). Chama `ao_tabela(nome, feitas)`.
    pub fn comecar_tabela(&mut self, nome: &str, colunas: &[String]) -> Resultado<()> {
        self.conferir_ativo()?;
        let r = self.comecar_interno(nome, colunas);
        self.depois(r)
    }

    /// Grava uma linha da fonte atual (valores na ordem das colunas).
    pub fn linha(&mut self, valores: &[Valor]) -> Resultado<()> {
        self.conferir_ativo()?;
        let r = if self.tabela_aberta {
            match self.saida.as_mut() {
                Some(saida) => saida.linha(valores, &mut self.ctx),
                None => Err(erro_encerrado()),
            }
        } else {
            Err(erro_sem_tabela())
        };
        self.depois(r)
    }

    /// Termina a fonte atual (no .zip, comprime o CSV dela). Sem fonte aberta, não faz nada.
    pub fn terminar_tabela(&mut self) -> Resultado<()> {
        self.conferir_ativo()?;
        let r = self.terminar_interno();
        self.depois(r)
    }

    /// Termina a fonte aberta, chama `ao_finalizar`, fecha o arquivo, avisa o progresso uma última vez e troca
    /// o destino pelo temporário.
    pub fn concluir(mut self) -> Resultado<Resumo> {
        self.conferir_ativo()?;
        match self.concluir_interno() {
            Ok(resumo) => Ok(resumo),
            Err(e) => Err(self.falhar(e)),
        }
    }

    fn comecar_interno(&mut self, nome: &str, colunas: &[String]) -> Resultado<()> {
        if self.tabela_aberta {
            self.terminar_interno()?;
        }
        if !self.varias && self.tabelas > 0 {
            return Err(ErroBanco::com_status(
                "Exportação mal conduzida: a exportação de uma tabela ou consulta recebe uma fonte só.",
                500,
            ));
        }
        self.ctx.progresso.tabela(nome, self.tabelas)?;
        let saida = self.saida.as_mut().ok_or_else(erro_encerrado)?;
        saida.comecar(nome, colunas, &mut self.ctx)?;
        self.tabela_aberta = true;
        Ok(())
    }

    fn terminar_interno(&mut self) -> Resultado<()> {
        if !self.tabela_aberta {
            return Ok(());
        }
        let saida = self.saida.as_mut().ok_or_else(erro_encerrado)?;
        saida.terminar(&mut self.ctx)?;
        self.tabela_aberta = false;
        self.tabelas += 1;
        Ok(())
    }

    fn concluir_interno(&mut self) -> Resultado<Resumo> {
        if !self.varias && self.tabelas == 0 && !self.tabela_aberta {
            self.comecar_interno("dados", &[])?;
        }
        self.terminar_interno()?;
        self.ctx.progresso.finalizar()?;
        let saida = self.saida.take().ok_or_else(erro_encerrado)?;
        saida.fechar()?;
        self.ctx.progresso.avisar()?;
        fs::rename(&self.temporario, &self.destino).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        self.concluido = true;
        let bytes = fs::metadata(&self.destino).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?.len();
        Ok(Resumo {
            caminho: self.destino.clone(),
            formato: self.formato,
            linhas: self.ctx.progresso.linhas,
            bytes,
            tabelas: self.tabelas,
            tempo_ms: u64::try_from(self.inicio.elapsed().as_millis()).unwrap_or(u64::MAX),
            avisos: self.ctx.avisos.frases(),
        })
    }

    fn conferir_ativo(&self) -> Resultado<()> {
        match &self.falha {
            Some(e) => Err(e.clone()),
            None if self.concluido => Err(erro_encerrado()),
            None => Ok(()),
        }
    }

    fn depois(&mut self, r: Resultado<()>) -> Resultado<()> {
        r.map_err(|e| self.falhar(e))
    }

    /// Apaga o temporário (fechando antes o que estava aberto) e guarda o erro para as próximas chamadas.
    fn falhar(&mut self, erro: ErroBanco) -> ErroBanco {
        self.limpar();
        self.falha.get_or_insert(erro).clone()
    }

    fn limpar(&mut self) {
        if let Some(saida) = self.saida.take() {
            saida.abortar();
        }
        if !self.concluido {
            arquivo::remover(&self.temporario);
        }
    }
}

impl Drop for Escritor {
    fn drop(&mut self) {
        if !self.concluido && self.falha.is_none() {
            self.limpar();
        }
    }
}

fn erro_encerrado() -> ErroBanco {
    ErroBanco::com_status("Exportação mal conduzida: o escritor já foi encerrado.", 500)
}

// ---------------------------------------------------------------------------------------------------
// Atalhos
// ---------------------------------------------------------------------------------------------------

/// Grava uma tabela (ou o resultado de uma consulta) em `destino`.
///
/// `nome` só é usado no Excel, como nome da planilha. O JSON é uma lista de objetos {coluna: valor}. Um `Err`
/// das linhas interrompe (temporário apagado) e sobe por [`erro_de_leitura`].
pub fn exportar_linhas<I, L>(
    colunas: &[String],
    linhas: I,
    formato: Formato,
    destino: &Path,
    opcoes: &Opcoes,
    nome: &str,
    acomp: Acompanhamento,
) -> Resultado<Resumo>
where
    I: IntoIterator<Item = Resultado<L>>,
    L: AsRef<[Valor]>,
{
    let escritor = Escritor::novo(formato, destino, opcoes, false, None, acomp)?;
    gravar_fonte_unica(escritor, nome, colunas, linhas)
}

pub(crate) fn gravar_fonte_unica<I, L>(
    mut escritor: Escritor,
    nome: &str,
    colunas: &[String],
    linhas: I,
) -> Resultado<Resumo>
where
    I: IntoIterator<Item = Resultado<L>>,
    L: AsRef<[Valor]>,
{
    escritor.comecar_tabela(nome, colunas)?;
    for item in linhas {
        let linha = item.map_err(erro_de_leitura)?;
        escritor.linha(linha.as_ref())?;
    }
    escritor.concluir()
}

/// Grava várias fontes (nome, colunas, linhas) num arquivo só, uma de cada vez: xlsx com uma planilha por
/// fonte, json `{"banco", "exportado_em", "tabelas": {…}}`, csv num .zip com um CSV por fonte (destino .csv
/// ou sem extensão passa a .zip; se esse .zip já existe, 409 `destino_existe`).
pub fn exportar_varias<F, N, I, L>(
    fontes: F,
    formato: Formato,
    destino: &Path,
    opcoes: &Opcoes,
    meta: Option<Meta>,
    acomp: Acompanhamento,
) -> Resultado<Resumo>
where
    F: IntoIterator<Item = (N, Vec<String>, I)>,
    N: AsRef<str>,
    I: IntoIterator<Item = Resultado<L>>,
    L: AsRef<[Valor]>,
{
    let escritor = Escritor::novo(formato, destino, opcoes, true, meta, acomp)?;
    gravar_fontes(escritor, fontes)
}

pub(crate) fn gravar_fontes<F, N, I, L>(mut escritor: Escritor, fontes: F) -> Resultado<Resumo>
where
    F: IntoIterator<Item = (N, Vec<String>, I)>,
    N: AsRef<str>,
    I: IntoIterator<Item = Resultado<L>>,
    L: AsRef<[Valor]>,
{
    for (nome, colunas, linhas) in fontes {
        escritor.comecar_tabela(nome.as_ref(), &colunas)?;
        for item in linhas {
            let linha = item.map_err(erro_de_leitura)?;
            escritor.linha(linha.as_ref())?;
        }
        escritor.terminar_tabela()?;
    }
    escritor.concluir()
}
