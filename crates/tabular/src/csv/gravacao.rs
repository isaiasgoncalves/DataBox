//! A gravação em fluxo: lê o arquivo de origem registro a registro, aplica as [`Edicoes`] e escreve num
//! temporário ao lado do destino; no fim, troca o destino pelo temporário (o destino só muda quando tudo deu
//! certo). Nada do arquivo inteiro fica na memória.
//!
//! - **O que não mudou, não é regravado:** a linha sem alteração é copiada byte a byte (com as aspas, a quebra
//!   de linha e o espaço como estavam), desde que o formato de saída escreva as linhas do mesmo jeito que o de
//!   entrada (mesmo separador, aspas, quebra de linha e a mesma tabela de caracteres). Uma célula alterada em
//!   1 milhão de linhas regrava uma linha.
//! - **O que mudou segue o estilo do arquivo:** separador, quebra de linha, aspas e codificação do formato
//!   (a quebra original de cada linha é mantida quando o arquivo mistura as duas).
//! - **O índice do arquivo novo sai junto:** as posições são as da gravação, e a leitura seguinte não precisa de
//!   outra passada.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

use comum::{ErroBanco, Resultado};

use super::analise::{BLOCO, Campo, Varredura, decodificar_campo, proximo_registro};
use super::arquivos::{CONTEXTO_GRAVAR, conferir_destino, criar_temporario, erro_io, remover, trocar};
use super::codificacao::BOM_UTF8;
use super::edicoes::{ChaveLinha, Edicoes, IdColuna, Trecho};
use super::escrita::escrever_registro;
use super::formato::{FimDeLinha, Formato};
use super::indice::{ConstrutorIndice, Indice};
use super::leitor::Leitor;

/// De quantos em quantos bytes lidos a gravação avisa o progresso.
const PASSO_DO_AVISO: u64 = 4 << 20;

/// O que uma gravação fez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoSalvar {
    /// Linhas de dados do arquivo gravado (sem o cabeçalho).
    pub linhas: u64,
    /// Tamanho do arquivo gravado.
    pub bytes: u64,
    /// Linhas escritas de novo (as demais foram copiadas como estavam).
    pub linhas_regravadas: u64,
    pub tempo_ms: u64,
}

pub(crate) struct Gravado {
    pub indice: Indice,
    pub formato: Formato,
    pub resumo: ResumoSalvar,
}

/// Conta os bytes que passam.
struct Contador<W: Write> {
    w: W,
    n: u64,
}

impl<W: Write> Write for Contador<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let k = self.w.write(buf)?;
        self.n += k as u64;
        Ok(k)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

fn erro_de_caractere(c: char, formato: &Formato, linha: Option<u64>) -> ErroBanco {
    let onde = linha.map_or_else(String::new, |l| format!(" (linha {})", l + 1));
    ErroBanco::novo(format!(
        "O caractere {c:?}{onde} não existe na codificação {}. Troque-o ou salve o arquivo em UTF-8.",
        formato.codificacao.rotulo()
    ))
}

fn erro_de_origem_mudou() -> ErroBanco {
    ErroBanco::com_status(
        "O arquivo foi alterado por outro programa depois de aberto; nada foi gravado. Abra-o de novo antes de salvar.",
        409,
    )
}

/// A frase da gravação recusada de um arquivo que não é UTF-8 lido como UTF-8.
pub const MSG_NAO_E_UTF8: &str = "O arquivo não é UTF-8 válido, mas está sendo lido como UTF-8: gravar trocaria os \
     caracteres de outra codificação por \u{FFFD}, também nas células que ninguém mexeu. Nada foi gravado. Para \
     editá-lo, desfaça as alterações e escolha Windows-1252 em Propriedades › Codificação.";

/// Grava a tabela (o arquivo do `leitor` com as `edicoes`) em `destino`, no formato `saida`. Recusa (sem tocar o
/// disco) o arquivo de origem que não é UTF-8 válido lido como UTF-8: a decodificação dele perde os bytes que não
/// são UTF-8, e toda linha regravada (ou o arquivo todo, noutro formato) sairia estragada.
pub(crate) fn gravar(
    leitor: &mut Leitor,
    edicoes: &Edicoes,
    destino: &Path,
    saida: &Formato,
    ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
) -> Resultado<Gravado> {
    let inicio = Instant::now();
    if leitor.formato().codificacao.e_utf8() && !leitor.utf8_valido() {
        return Err(ErroBanco::com_status(MSG_NAO_E_UTF8, 409));
    }
    conferir_destino(destino)?;
    if leitor.mudou_no_disco() {
        return Err(erro_de_origem_mudou());
    }
    let mut saida = saida.clone();
    saida.cabecalho = leitor.formato().cabecalho;
    let (temporario, arquivo) = criar_temporario(destino)?;
    let resultado = escrever(leitor, edicoes, arquivo, &saida, ao_progresso);
    let (indice, regravadas) = match resultado {
        Ok(r) => r,
        Err(e) => {
            remover(&temporario);
            return Err(e);
        }
    };
    // o destino pode ser o próprio arquivo que está aberto para leitura: solta antes de trocar
    leitor.soltar();
    if let Err(e) = trocar(&temporario, destino) {
        remover(&temporario);
        return Err(erro_io(&e, Some(destino), CONTEXTO_GRAVAR));
    }
    let formato = Formato { termina_em_quebra: indice.termina_em_quebra, ..saida };
    let resumo = ResumoSalvar {
        linhas: edicoes.total_linhas(),
        bytes: indice.tamanho,
        linhas_regravadas: regravadas,
        tempo_ms: u64::try_from(inicio.elapsed().as_millis()).unwrap_or(u64::MAX),
    };
    Ok(Gravado { indice, formato, resumo })
}

/// Escreve o arquivo inteiro no temporário e o deixa em disco.
fn escrever(
    leitor: &Leitor,
    edicoes: &Edicoes,
    arquivo: File,
    saida: &Formato,
    ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
) -> Resultado<(Indice, u64)> {
    let io = |e: io::Error| erro_io(&e, None, CONTEXTO_GRAVAR);
    let entrada = leitor.formato();
    let origem = leitor.indice();
    let copiavel = entrada.linhas_sao_copiaveis_para(saida);
    let copia_integral = copiavel && edicoes.colunas_como_no_arquivo();
    let nomes_iguais = edicoes.colunas().iter().map(|c| c.nome.as_str()).eq(leitor.nomes().iter().map(String::as_str));
    let cabecalho_copiavel = copia_integral && nomes_iguais && origem.registros > 0;

    let mut w = Contador { w: BufWriter::with_capacity(BLOCO, arquivo), n: 0 };
    if saida.codificacao.tem_bom() {
        w.write_all(&BOM_UTF8).map_err(io)?;
    }
    let inicio_da_saida = w.n;

    let mut fonte = File::open(leitor.caminho()).map_err(|e| erro_io(&e, Some(leitor.caminho()), CONTEXTO_GRAVAR))?;
    fonte.seek(SeekFrom::Start(origem.inicio)).map_err(io)?;
    let mut v = Varredura::nova(fonte, origem.inicio, entrada.sep(), false, false, BLOCO);

    let posicao_da_coluna: std::collections::HashMap<IdColuna, usize> =
        edicoes.colunas().iter().enumerate().map(|(i, c)| (c.id, i)).collect();
    let mut editadas: Vec<u64> = edicoes
        .celulas()
        .filter_map(|(l, _, _)| match l {
            ChaveLinha::Base(r) => Some(r),
            ChaveLinha::Nova(_) => None,
        })
        .collect();
    editadas.dedup();

    let mut registros = ConstrutorIndice::default();
    let mut regravadas = 0u64;
    let mut ja_escreveu = false;
    // a quebra de linha que fecha o último registro escrito (vazia: o de origem não tinha)
    let mut pendente: &'static [u8] = b"";
    let mut proximo_aviso = origem.inicio + PASSO_DO_AVISO;
    let mut campos_da_origem: Vec<Campo> = Vec::new();
    let mut buffer: Vec<u8> = Vec::new();
    let eol_de_saida = saida.fim_de_linha.bytes();

    // escreve um registro já pronto (os bytes sem a quebra de linha) e guarda a quebra que o fecha
    macro_rules! registro {
        ($bytes:expr, $n_campos:expr, $quebra:expr) => {{
            if ja_escreveu {
                w.write_all(if pendente.is_empty() { eol_de_saida } else { pendente }).map_err(io)?;
            }
            registros.registro(w.n, $n_campos);
            w.write_all($bytes).map_err(io)?;
            pendente = $quebra;
            ja_escreveu = true;
        }};
    }
    // a quebra original do registro de origem, quando as linhas são copiáveis; senão a da saída
    let quebra_de = |v: &Varredura<File>, copiavel: bool| -> &'static [u8] {
        if copiavel {
            FimDeLinha::de_bytes(v.terminador()).map_or(b"" as &[u8], FimDeLinha::bytes)
        } else {
            eol_de_saida
        }
    };

    // cabeçalho
    if saida.cabecalho {
        let mut tinha = false;
        if origem.registros > 0 {
            v.seguinte().map_err(io)?;
            tinha = true;
        }
        if cabecalho_copiavel && tinha {
            let quebra = quebra_de(&v, true);
            registro!(v.conteudo(), v.registro().n_campos, quebra);
        } else {
            let nomes: Vec<&str> = edicoes.colunas().iter().map(|c| c.nome.as_str()).collect();
            buffer.clear();
            escrever_registro(&mut buffer, &nomes, saida).map_err(|c| erro_de_caractere(c, saida, None))?;
            let n = u32::try_from(nomes.len()).unwrap_or(u32::MAX);
            registro!(&buffer, n, eol_de_saida);
        }
    }

    let n_colunas = edicoes.n_colunas();
    let n_colunas_u32 = u32::try_from(n_colunas).unwrap_or(u32::MAX);
    let mut proxima_da_origem = 0u64; // a próxima linha de dados que o scanner entregaria
    let mut cursor_editadas = 0usize;
    let mut linha_de_saida = vec![String::new(); n_colunas];

    for trecho in edicoes.trechos() {
        match trecho {
            Trecho::Base { de, ate } => {
                if *de > proxima_da_origem {
                    v.pular(de - proxima_da_origem).map_err(io)?;
                    proxima_da_origem = *de;
                }
                for r in *de..*ate {
                    if !v.seguinte().map_err(io)? {
                        return Err(erro_de_origem_mudou());
                    }
                    proxima_da_origem += 1;
                    while editadas.get(cursor_editadas).is_some_and(|&e| e < r) {
                        cursor_editadas += 1;
                    }
                    let editada = editadas.get(cursor_editadas) == Some(&r);
                    if copia_integral && !editada {
                        let quebra = quebra_de(&v, true);
                        registro!(v.conteudo(), v.registro().n_campos, quebra);
                    } else {
                        // os campos da origem, só os das colunas que ficaram
                        let conteudo = v.conteudo();
                        proximo_registro(conteudo, 0, entrada.sep(), true, Some(&mut campos_da_origem));
                        for (destino_c, coluna) in linha_de_saida.iter_mut().zip(edicoes.colunas()) {
                            *destino_c = match coluna.origem.and_then(|o| campos_da_origem.get(o)) {
                                Some(c) => decodificar_campo(conteudo, *c, entrada.codificacao),
                                None => String::new(),
                            };
                        }
                        for (id, valor) in edicoes.celulas_da_linha(ChaveLinha::Base(r)) {
                            if let Some(c) = posicao_da_coluna.get(&id).and_then(|&p| linha_de_saida.get_mut(p)) {
                                *c = valor.to_string();
                            }
                        }
                        buffer.clear();
                        escrever_registro(&mut buffer, &linha_de_saida, saida)
                            .map_err(|c| erro_de_caractere(c, saida, Some(r)))?;
                        let quebra = quebra_de(&v, copiavel);
                        registro!(&buffer, n_colunas_u32, quebra);
                        regravadas += 1;
                    }
                    if v.deslocamento_do_fim() >= proximo_aviso {
                        ao_progresso(v.deslocamento_do_fim(), origem.tamanho)?;
                        proximo_aviso = v.deslocamento_do_fim() + PASSO_DO_AVISO;
                    }
                }
            }
            Trecho::Nova(id) => {
                for c in &mut linha_de_saida {
                    c.clear();
                }
                for (idc, valor) in edicoes.celulas_da_linha(ChaveLinha::Nova(*id)) {
                    if let Some(c) = posicao_da_coluna.get(&idc).and_then(|&p| linha_de_saida.get_mut(p)) {
                        *c = valor.to_string();
                    }
                }
                buffer.clear();
                escrever_registro(&mut buffer, &linha_de_saida, saida)
                    .map_err(|c| erro_de_caractere(c, saida, None))?;
                registro!(&buffer, n_colunas_u32, eol_de_saida);
                regravadas += 1;
            }
        }
    }

    let termina = !ja_escreveu || saida.termina_em_quebra;
    if ja_escreveu && saida.termina_em_quebra {
        w.write_all(if pendente.is_empty() { eol_de_saida } else { pendente }).map_err(io)?;
    }
    let tamanho = w.n;
    let arquivo = w.w.into_inner().map_err(|e| erro_io(e.error(), None, CONTEXTO_GRAVAR))?;
    arquivo.sync_all().map_err(io)?;
    drop(arquivo);
    ao_progresso(origem.tamanho, origem.tamanho)?;
    Ok((registros.concluir(inicio_da_saida, tamanho, termina), regravadas))
}
