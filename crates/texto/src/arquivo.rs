//! Carregar e gravar um arquivo de texto (TXT, MD, LOG, JSON…) como o editor precisa.
//!
//! A regra é a do CSV, pela [`Codificacao`] do `tabular`: BOM de UTF-8 → UTF-8 com BOM;
//! UTF-8 válido do começo ao fim → UTF-8; qualquer byte inválido → Windows-1252. UTF-16 e arquivo que não é
//! texto são recusados com a frase pronta. O editor trabalha com `\n`: [`carregar`] troca `\r\n` e `\r` por `\n`
//! e guarda o fim de linha dominante (empate: CRLF), e [`gravar`] o devolve. O texto carregado é o do arquivo, **com**
//! a quebra do fim se ela existia, então o que a pessoa vê (e apaga ou acrescenta no fim) é o que volta a ser gravado.
//!
//! A gravação é atômica: temporário ao lado do destino (`~nome.xxxxxxxx.tmp`), `sync_all` e `rename` por cima,
//! insistindo uns 200 ms se outro programa estiver lendo o arquivo; em qualquer erro o temporário é apagado e o
//! arquivo fica como estava. Caractere que o Windows-1252 não tem recusa a gravação com a frase e a linha.

use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use comum::{ErroBanco, Resultado, traduzir_io};
use tabular::csv::{BOM_UTF8, Codificacao, FimDeLinha, detectar_codificacao};

use crate::limites::{LIMITE_LEITURA, tamanho_em_texto};

const CONTEXTO_LER: &str = "Não foi possível ler o arquivo.";
const CONTEXTO_GRAVAR: &str = "Não foi possível gravar o arquivo.";

/// Quantas vezes se tenta trocar o arquivo, e o intervalo entre as tentativas (somam os 200 ms do plano).
const TENTATIVAS_DA_TROCA: u32 = 4;
const ESPERA_DA_TROCA: Duration = Duration::from_millis(50);

/// Quantos bytes do começo se olham para decidir se um arquivo com bytes nulos é UTF-16 sem BOM.
const AMOSTRA_UTF16: usize = 4096;

/// Um arquivo de texto lido: o texto e tudo o que a gravação precisa para devolvê-lo como estava.
#[derive(Debug, Clone, PartialEq)]
pub struct Carregado {
    /// O texto, só com `\n` como quebra de linha, sem o BOM, com a quebra do fim se o arquivo tinha.
    pub texto: String,
    pub codificacao: Codificacao,
    /// O fim de linha dominante (CRLF, LF ou CR; empate, CRLF; sem nenhuma quebra, CRLF). É o da gravação.
    pub fim_de_linha: FimDeLinha,
    /// O arquivo misturava mais de um fim de linha: a gravação o regrava todo com o dominante ("CRLF (misto)").
    pub fim_misto: bool,
    /// O texto termina em `\n`.
    pub termina_em_quebra: bool,
    /// O tamanho do arquivo em disco na leitura (ou na última gravação).
    pub bytes: u64,
    /// A data de modificação na leitura (ou na última gravação).
    pub modificado: Option<SystemTime>,
}

/// O que uma gravação fez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoGravado {
    /// O tamanho do arquivo gravado.
    pub bytes: u64,
    pub tempo_ms: u64,
    /// A data de modificação do arquivo gravado.
    pub modificado: Option<SystemTime>,
}

impl Carregado {
    /// Guarda o que a gravação fez (tamanho, data, e se o texto gravado termina em quebra), para o
    /// [`mudou_no_disco`] seguinte comparar com o arquivo novo, e não com o antigo. O fim misto acaba aqui: o
    /// arquivo foi regravado com o dominante.
    pub fn registrar_gravacao(&mut self, resumo: &ResumoGravado, texto: &str) {
        self.bytes = resumo.bytes;
        self.modificado = resumo.modificado;
        self.termina_em_quebra = texto.ends_with('\n');
        self.fim_misto = false;
    }

    /// "UTF-8", "UTF-8 com BOM" ou "Windows-1252", como as Propriedades mostram.
    pub fn rotulo_da_codificacao(&self) -> &'static str {
        self.codificacao.rotulo()
    }

    /// "LF", "CRLF" ou "CR", e "CRLF (misto)" quando o arquivo misturava fins de linha (a Propriedades do editor
    /// de texto).
    pub fn rotulo_do_fim_de_linha(&self) -> String {
        let nome = match self.fim_de_linha {
            FimDeLinha::Crlf => "CRLF",
            FimDeLinha::Lf => "LF",
            FimDeLinha::Cr => "CR",
        };
        if self.fim_misto { format!("{nome} (misto)") } else { nome.to_string() }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Erros

fn erro_io(e: &io::Error, caminho: Option<&Path>, contexto: &str) -> ErroBanco {
    // no Windows, arquivo travado por outro programa (violação de compartilhamento ou de trava) é falta de permissão
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return ErroBanco::com_status(
            "Sem permissão para acessar o arquivo. Ele pode estar aberto em outro programa.",
            403,
        )
        .detalhe(e.to_string())
        .com_contexto(contexto);
    }
    traduzir_io(e, caminho, Some(contexto))
}

fn erro_utf16() -> ErroBanco {
    ErroBanco::novo(
        "O arquivo está em UTF-16, que o DataBox ainda não abre como texto. Salve-o como UTF-8 no programa de origem.",
    )
}

fn erro_nao_e_texto() -> ErroBanco {
    ErroBanco::novo("O arquivo não parece de texto (tem bytes nulos); o DataBox só abre arquivos de texto.")
}

fn erro_grande_demais(bytes: u64) -> ErroBanco {
    ErroBanco::novo(format!(
        "Este arquivo é grande demais para o DataBox mostrar ({}). Abra no programa padrão.",
        tamanho_em_texto(bytes)
    ))
}

fn erro_de_caractere(c: char, codificacao: Codificacao, linha: usize) -> ErroBanco {
    ErroBanco::novo(format!(
        "O caractere {c:?} (linha {linha}) não existe na codificação {}, que é a deste arquivo. Troque-o para poder \
         salvar.",
        codificacao.rotulo()
    ))
}

// ---------------------------------------------------------------------------------------------------------------
// Ler

/// Lê o arquivo de texto. Erros: não existe (404) ou sem permissão (403) como no resto do DataBox; UTF-16 e
/// arquivo com bytes nulos, com a frase da decisão 5; acima de [`LIMITE_LEITURA`], 400 com a frase do tamanho.
pub fn carregar(caminho: &Path) -> Resultado<Carregado> {
    let io = |e: io::Error| erro_io(&e, Some(caminho), CONTEXTO_LER);
    let arquivo = File::open(caminho).map_err(io)?;
    let meta = arquivo.metadata().map_err(io)?;
    if !meta.is_file() {
        return Err(ErroBanco::novo(format!("Isto não é um arquivo: {}.", caminho.display())));
    }
    if meta.len() > LIMITE_LEITURA {
        return Err(erro_grande_demais(meta.len()));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    // o arquivo pode ter crescido depois do `metadata`: lê um byte além do limite só para perceber
    arquivo.take(LIMITE_LEITURA + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 > LIMITE_LEITURA {
        return Err(erro_grande_demais(bytes.len() as u64));
    }
    decodificar(&bytes, meta.modified().ok())
}

/// O que [`carregar`] faz depois de ler os bytes (e que serve a quem já os tem na memória): a codificação, o
/// fim de linha dominante e o texto com `\n`.
pub fn decodificar(bytes: &[u8], modificado: Option<SystemTime>) -> Resultado<Carregado> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(erro_utf16());
    }
    if bytes.contains(&0) {
        return Err(if parece_utf16_sem_bom(bytes) { erro_utf16() } else { erro_nao_e_texto() });
    }
    let (codificacao, texto) = if bytes.starts_with(&BOM_UTF8) {
        match std::str::from_utf8(&bytes[BOM_UTF8.len()..]) {
            Ok(t) => (Codificacao::Utf8ComBom, t.to_string()),
            // o BOM de UTF-8 e depois bytes que não são UTF-8: é um arquivo em Windows-1252 que por azar começa
            // com "ï»¿"; lido assim, volta byte a byte igual na gravação (UTF-8 "com perda" gravaria U+FFFD)
            Err(_) => (Codificacao::Windows1252, Codificacao::Windows1252.decodificar(bytes).into_owned()),
        }
    } else {
        let codificacao = detectar_codificacao(bytes, false);
        (codificacao, codificacao.decodificar(bytes).into_owned())
    };
    let (texto, fim_de_linha, fim_misto) = normalizar_quebras(texto);
    Ok(Carregado {
        termina_em_quebra: texto.ends_with('\n'),
        texto,
        codificacao,
        fim_de_linha,
        fim_misto,
        bytes: bytes.len() as u64,
        modificado,
    })
}

/// UTF-16 sem BOM: os bytes nulos se repetem de dois em dois (a letra latina é um byte e um zero). Um arquivo
/// binário qualquer tem zeros, mas não nesse compasso.
fn parece_utf16_sem_bom(bytes: &[u8]) -> bool {
    let amostra = &bytes[..bytes.len().min(AMOSTRA_UTF16)];
    let pares = amostra.len() / 2;
    if pares == 0 {
        return false;
    }
    let (mut pares_nulos, mut impares_nulos) = (0usize, 0usize);
    for (i, &b) in amostra.iter().enumerate() {
        if b == 0 {
            if i % 2 == 0 {
                pares_nulos += 1;
            } else {
                impares_nulos += 1;
            }
        }
    }
    // zero em pelo menos dois quintos das posições de um dos lados
    pares_nulos.max(impares_nulos) * 5 >= pares * 2
}

/// Troca `\r\n` e `\r` por `\n` e conta cada tipo. O dominante é o que mais aparece (empate: CRLF, depois LF,
/// depois CR; nenhum: CRLF); é "misto" quando mais de um tipo aparece.
fn normalizar_quebras(texto: String) -> (String, FimDeLinha, bool) {
    if !texto.contains('\r') {
        let lf = texto.bytes().filter(|&b| b == b'\n').count();
        let dominante = if lf > 0 { FimDeLinha::Lf } else { FimDeLinha::Crlf };
        return (texto, dominante, false);
    }
    let (mut crlf, mut cr) = (0usize, 0usize);
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto.as_str();
    while let Some(p) = resto.find('\r') {
        saida.push_str(&resto[..p]);
        saida.push('\n');
        let depois = &resto[p + 1..];
        if let Some(sem_lf) = depois.strip_prefix('\n') {
            crlf += 1;
            resto = sem_lf;
        } else {
            cr += 1;
            resto = depois;
        }
    }
    saida.push_str(resto);
    let total = saida.bytes().filter(|&b| b == b'\n').count();
    let lf = total - crlf - cr;
    let tipos = [(FimDeLinha::Crlf, crlf), (FimDeLinha::Lf, lf), (FimDeLinha::Cr, cr)];
    let maior = tipos.iter().map(|&(_, n)| n).max().unwrap_or(0);
    let dominante = tipos.iter().find(|&&(_, n)| n == maior).map_or(FimDeLinha::Crlf, |&(t, _)| t);
    let misto = tipos.iter().filter(|&&(_, n)| n > 0).count() > 1;
    (saida, dominante, misto)
}

// ---------------------------------------------------------------------------------------------------------------
// Gravar

/// Os bytes do texto como o arquivo os guarda: a codificação (com o BOM, se tinha) e o fim de linha, no lugar do
/// `\n`. Caractere que a codificação não tem (Windows-1252) é erro com a frase e a linha.
pub fn codificar(texto: &str, codificacao: Codificacao, fim_de_linha: FimDeLinha) -> Resultado<Vec<u8>> {
    let quebra = fim_de_linha.bytes();
    let quebras = texto.bytes().filter(|&b| b == b'\n').count();
    let mut saida = Vec::with_capacity(texto.len() + quebras * (quebra.len() - 1) + BOM_UTF8.len());
    if codificacao.tem_bom() {
        saida.extend_from_slice(&BOM_UTF8);
    }
    if codificacao.e_utf8() && quebra == b"\n" {
        saida.extend_from_slice(texto.as_bytes());
        return Ok(saida);
    }
    for (i, linha) in texto.split('\n').enumerate() {
        if i > 0 {
            saida.extend_from_slice(quebra);
        }
        match codificacao.codificar(linha) {
            Ok(bytes) => saida.extend_from_slice(&bytes),
            Err(c) => return Err(erro_de_caractere(c, codificacao, i + 1)),
        }
    }
    Ok(saida)
}

/// Grava o texto em `caminho` no formato do arquivo `original` (codificação com BOM e fim de linha dominante),
/// de forma atômica. Devolve o tamanho gravado e o tempo. O arquivo não é conferido contra `original` aqui: quem
/// quer perguntar "o arquivo mudou no disco" chama [`mudou_no_disco`] antes.
pub fn gravar(caminho: &Path, texto: &str, original: &Carregado) -> Resultado<ResumoGravado> {
    let inicio = Instant::now();
    let bytes = codificar(texto, original.codificacao, original.fim_de_linha)?;
    gravar_atomico(caminho, &bytes)?;
    let modificado = fs::metadata(caminho).and_then(|m| m.modified()).ok();
    Ok(ResumoGravado {
        bytes: bytes.len() as u64,
        tempo_ms: u64::try_from(inicio.elapsed().as_millis()).unwrap_or(u64::MAX),
        modificado,
    })
}

/// O arquivo no disco já não é o que foi lido: o tamanho ou a data são outros (ou ele sumiu).
pub fn mudou_no_disco(caminho: &Path, original: &Carregado) -> bool {
    let Ok(meta) = fs::metadata(caminho) else {
        return true;
    };
    if meta.len() != original.bytes {
        return true;
    }
    match (meta.modified().ok(), original.modificado) {
        (Some(agora), Some(antes)) => agora != antes,
        _ => false,
    }
}

/// Grava `bytes` em `caminho` sem nunca deixar o arquivo pela metade: temporário na mesma pasta, `sync_all` e
/// `rename` por cima. Em qualquer erro o temporário é apagado e o arquivo fica como estava.
pub fn gravar_atomico(caminho: &Path, bytes: &[u8]) -> Resultado<()> {
    gravar_atomico_com(caminho, bytes, trocar)
}

fn gravar_atomico_com(
    caminho: &Path,
    bytes: &[u8],
    troca: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Resultado<()> {
    // um link simbólico: o que se grava é o arquivo para onde ele aponta (o `rename` por cima trocaria o link por um
    // arquivo comum e deixaria o de verdade como estava)
    let alvo;
    let caminho = match fs::symlink_metadata(caminho) {
        Ok(m) if m.file_type().is_symlink() => {
            alvo = fs::canonicalize(caminho).map_err(|e| erro_io(&e, Some(caminho), CONTEXTO_GRAVAR))?;
            alvo.as_path()
        }
        _ => caminho,
    };
    conferir_destino(caminho)?;
    let (temporario, mut arquivo) = criar_temporario(caminho)?;
    let resultado = (|| -> Resultado<()> {
        let io = |e: io::Error| erro_io(&e, None, CONTEXTO_GRAVAR);
        arquivo.write_all(bytes).map_err(io)?;
        arquivo.sync_all().map_err(io)?;
        // o Windows não troca um arquivo que ainda está aberto
        drop(arquivo);
        troca(&temporario, caminho).map_err(|e| erro_io(&e, Some(caminho), CONTEXTO_GRAVAR))
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
}

/// Sufixo de nome com 8 letras, números ou "_".
fn sufixo_aleatorio() -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let agora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut v = RandomState::new().hash_one((std::process::id(), n, agora));
    const ALFABETO: &[u8; 37] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut s = String::with_capacity(8);
    for _ in 0..8 {
        s.push(char::from(ALFABETO.get((v % 37) as usize).copied().unwrap_or(b'x')));
        v /= 37;
    }
    s
}

/// A pasta existe, o destino não é uma pasta e, se já existe, aceita gravação.
fn conferir_destino(destino: &Path) -> Resultado<()> {
    let pasta = destino.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !pasta.is_dir() {
        return Err(ErroBanco::com_status(
            format!("A pasta de destino não existe: {}. Escolha outra pasta ou crie esta antes.", pasta.display()),
            404,
        ));
    }
    if destino.is_dir() {
        return Err(ErroBanco::novo(format!(
            "O destino escolhido é uma pasta, não um arquivo: {}.",
            destino.display()
        )));
    }
    if let Ok(meta) = fs::metadata(destino)
        && meta.permissions().readonly()
    {
        return Err(ErroBanco::com_status(
            format!(
                "O arquivo {} é somente leitura; tire essa marca nas propriedades dele ou salve com outro nome.",
                destino.display()
            ),
            403,
        ));
    }
    Ok(())
}

/// Cria o temporário na pasta do destino ("~dados.json.k3j9x0ab.tmp"), aberto para gravar.
fn criar_temporario(destino: &Path) -> Resultado<(PathBuf, File)> {
    let pasta = destino.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let nome = destino.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut ultimo = None;
    for _ in 0..100 {
        let caminho = pasta.join(format!("~{nome}.{}.tmp", sufixo_aleatorio()));
        match OpenOptions::new().write(true).create_new(true).open(&caminho) {
            Ok(arquivo) => return Ok((caminho, arquivo)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => ultimo = Some(e),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                return Err(ErroBanco::com_status(
                    format!("Sem permissão para gravar na pasta {}.", pasta.display()),
                    403,
                )
                .detalhe(e.to_string()));
            }
            Err(e) => return Err(erro_io(&e, Some(&caminho), CONTEXTO_GRAVAR)),
        }
    }
    let e = ultimo.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists));
    Err(erro_io(&e, None, CONTEXTO_GRAVAR))
}

fn e_permissao(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// Põe o temporário no lugar do destino. No Windows a troca falha enquanto outro processo lê o destino, e
/// costuma passar em milissegundos: tenta mais algumas vezes antes de desistir.
fn trocar(temporario: &Path, destino: &Path) -> io::Result<()> {
    let mut tentativa = 0;
    loop {
        match fs::rename(temporario, destino) {
            Ok(()) => return Ok(()),
            Err(e) if tentativa < TENTATIVAS_DA_TROCA && e_permissao(&e) => {
                tentativa += 1;
                thread::sleep(ESPERA_DA_TROCA);
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod testes;
