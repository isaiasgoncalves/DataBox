//! CSV em UTF-8 com BOM (aspas só quando preciso, fim de linha "\r\n"), e o
//! banco inteiro em CSV: um .zip com um CSV por tabela.

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use comum::{Resultado, Valor};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::arquivo::{self, ArquivoZip, CONTEXTO_GRAVAR, erro_io, erro_zip};
use crate::valores::{nome_arquivo, valor_csv};
use crate::{BLOCO_COPIA, Contexto, Opcoes};

const BOM: &[u8] = b"\xef\xbb\xbf";

/// Grava os campos de um registro: aspas só quando o campo tem o separador,
/// aspas ou quebra de linha (aspas dobradas dentro); um registro de um campo só e vazio sai como `""`.
fn registro<'a, W: Write>(w: &mut W, separador: u8, campos: impl ExactSizeIterator<Item = &'a str>) -> io::Result<()> {
    let unico = campos.len() == 1;
    for (i, campo) in campos.enumerate() {
        if i > 0 {
            w.write_all(&[separador])?;
        }
        if unico && campo.is_empty() {
            w.write_all(b"\"\"")?;
        } else {
            escrever_campo(w, separador, campo)?;
        }
    }
    w.write_all(b"\r\n")
}

fn escrever_campo<W: Write>(w: &mut W, separador: u8, campo: &str) -> io::Result<()> {
    let precisa = campo.bytes().any(|b| b == separador || b == b'"' || b == b'\n' || b == b'\r');
    if !precisa {
        return w.write_all(campo.as_bytes());
    }
    w.write_all(b"\"")?;
    for (i, parte) in campo.split('"').enumerate() {
        if i > 0 {
            w.write_all(b"\"\"")?;
        }
        w.write_all(parte.as_bytes())?;
    }
    w.write_all(b"\"")
}

fn separador(opcoes: &Opcoes) -> u8 {
    // conferido em Opcoes::conferir: ";", "," ou tabulação
    u8::try_from(u32::from(opcoes.csv_separador)).unwrap_or(b';')
}

fn cabecalho<W: Write>(w: &mut W, colunas: &[String], opcoes: &Opcoes) -> io::Result<()> {
    if opcoes.cabecalho {
        registro(w, separador(opcoes), colunas.iter().map(String::as_str))?;
    }
    Ok(())
}

fn linha<W: Write>(w: &mut W, valores: &[Valor], opcoes: &Opcoes) -> io::Result<()> {
    let virgula = opcoes.csv_decimal == ',';
    let proteger = opcoes.csv_proteger_formulas;
    let campos: Vec<_> = valores.iter().map(|v| valor_csv(v, virgula, proteger)).collect();
    registro(w, separador(opcoes), campos.iter().map(|c| c.as_ref()))
}

/// Uma tabela num CSV só.
pub(crate) struct CsvArquivo {
    w: BufWriter<File>,
}

impl CsvArquivo {
    pub fn abrir(arquivo: File) -> Resultado<CsvArquivo> {
        let mut w = BufWriter::with_capacity(BLOCO_COPIA, arquivo);
        w.write_all(BOM).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        Ok(CsvArquivo { w })
    }

    pub fn comecar(&mut self, colunas: &[String], ctx: &Contexto) -> Resultado<()> {
        cabecalho(&mut self.w, colunas, &ctx.opcoes).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))
    }

    pub fn linha(&mut self, valores: &[Valor], ctx: &mut Contexto) -> Resultado<()> {
        linha(&mut self.w, valores, &ctx.opcoes).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        ctx.progresso.mais_uma()
    }

    pub fn fechar(self) -> Resultado<()> {
        let arquivo = self.w.into_inner().map_err(|e| erro_io(e.error(), CONTEXTO_GRAVAR))?;
        drop(arquivo);
        Ok(())
    }

    pub fn abortar(self) {
        // sem esvaziar o buffer: o temporário vai ser apagado
        let (arquivo, _) = self.w.into_parts();
        drop(arquivo);
    }
}

/// O CSV de uma tabela, gravado num arquivo de trabalho antes de entrar no .zip (para o zip saber o tamanho
/// e só usar ZIP64 quando precisar, o que mantém o pacote legível em qualquer programa). O arquivo some
/// quando o handle fecha; o `caminho` só serve para o [`arquivo::remover`] de reserva.
struct CsvDeTrabalho {
    entrada: String,
    caminho: PathBuf,
    w: BufWriter<File>,
}

/// O banco inteiro em CSV: um .zip com um CSV por tabela.
pub(crate) struct ZipCsv {
    zip: ZipWriter<ArquivoZip>,
    parado: Arc<AtomicBool>,
    opcoes_zip: SimpleFileOptions,
    usados: HashSet<String>,
    atual: Option<CsvDeTrabalho>,
}

impl ZipCsv {
    pub fn abrir(arquivo: File, momento: zip::DateTime) -> ZipCsv {
        let (arquivo, parado) = ArquivoZip::novo(arquivo);
        let opcoes_zip =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).last_modified_time(momento);
        ZipCsv { zip: ZipWriter::new(arquivo), parado, opcoes_zip, usados: HashSet::new(), atual: None }
    }

    pub fn comecar(&mut self, nome: &str, colunas: &[String], ctx: &Contexto) -> Resultado<()> {
        let entrada = nome_arquivo(nome, &mut self.usados, ".csv");
        // apagado sozinho pelo sistema ao fechar, mesmo se o processo morrer no meio (ver arquivo.rs)
        let (caminho, arquivo) = arquivo::criar_temporario_de_trabalho(ctx.limites.pasta_trabalho.as_deref())?;
        let atual =
            self.atual.insert(CsvDeTrabalho { entrada, caminho, w: BufWriter::with_capacity(BLOCO_COPIA, arquivo) });
        atual.w.write_all(BOM).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        cabecalho(&mut atual.w, colunas, &ctx.opcoes).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))
    }

    pub fn linha(&mut self, valores: &[Valor], ctx: &mut Contexto) -> Resultado<()> {
        let Some(atual) = self.atual.as_mut() else {
            return Err(crate::erro_sem_tabela());
        };
        linha(&mut atual.w, valores, &ctx.opcoes).map_err(|e| erro_io(&e, CONTEXTO_GRAVAR))?;
        ctx.progresso.mais_uma()
    }

    /// Comprime o CSV da tabela no .zip; quem acompanha recebe um aviso a cada bloco de 1 MB (comprimir um CSV
    /// grande leva tempo, e é assim que se interrompe no meio).
    pub fn terminar(&mut self, ctx: &mut Contexto) -> Resultado<()> {
        let Some(CsvDeTrabalho { entrada, caminho, w }) = self.atual.take() else {
            return Ok(());
        };
        let resultado = match w.into_inner() {
            Ok(arquivo) => self.copiar(&entrada, arquivo, ctx),
            Err(e) => Err(erro_io(e.error(), CONTEXTO_GRAVAR)),
        };
        arquivo::remover(&caminho);
        resultado
    }

    fn copiar(&mut self, entrada: &str, mut arquivo: File, ctx: &mut Contexto) -> Resultado<()> {
        let io_erro = |e: io::Error| erro_io(&e, CONTEXTO_GRAVAR);
        let tamanho = arquivo.stream_position().map_err(io_erro)?;
        arquivo.seek(SeekFrom::Start(0)).map_err(io_erro)?;
        // ZIP64 só quando o arquivo chega perto do limite
        let grande = tamanho.saturating_add(tamanho / 20) > zip::ZIP64_BYTES_THR;
        self.zip.start_file(entrada, self.opcoes_zip.large_file(grande)).map_err(erro_zip)?;
        let mut bloco = vec![0u8; BLOCO_COPIA];
        loop {
            let n = match arquivo.read(&mut bloco) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_erro(e)),
            };
            self.zip.write_all(bloco.get(..n).unwrap_or_default()).map_err(io_erro)?;
            ctx.progresso.avisar()?;
        }
        Ok(())
    }

    pub fn fechar(self) -> Resultado<()> {
        let ZipCsv { zip, parado, atual, .. } = self;
        if let Some(t) = atual {
            descartar_trabalho(t);
        }
        match zip.finish() {
            Ok(arquivo) => arquivo.terminar().map_err(|e| erro_io(&e, CONTEXTO_GRAVAR)),
            Err(e) => {
                parado.store(true, Ordering::Relaxed);
                Err(erro_zip(e))
            }
        }
    }

    pub fn abortar(self) {
        let ZipCsv { zip, parado, atual, .. } = self;
        parado.store(true, Ordering::Relaxed);
        if let Some(t) = atual {
            descartar_trabalho(t);
        }
        drop(zip);
    }
}

fn descartar_trabalho(t: CsvDeTrabalho) {
    let (arquivo, _) = t.w.into_parts();
    drop(arquivo);
    arquivo::remover(&t.caminho);
}
