//! Caminhos que a tela manda e arquivos que o serviço grava: a limpeza do caminho digitado, a chave de
//! comparação, os arquivos do próprio aplicativo e a gravação de um arquivo.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use comum::{ErroBanco, Resultado, traduzir_io};

/// O `_limpar_caminho`: sem espaços nem aspas nas pontas ("Copiar como caminho" do Explorer põe aspas), com
/// `~` e `~usuario` expandidos (o `os.path.expanduser`) e absoluto. Vazio → 400 "Informe <rotulo>.".
pub(crate) fn limpar_caminho(caminho: &Path, rotulo: &str) -> Resultado<PathBuf> {
    let informe = || ErroBanco::novo(format!("Informe {rotulo}."));
    let Some(texto) = caminho.to_str() else {
        // nome com UTF-16 inválido (só no Windows): não há como ter vindo digitado; só fica absoluto
        return std::path::absolute(caminho).map_err(|e| traduzir_io(&e, Some(caminho), None));
    };
    let mut limpo = ddl::aparar(texto);
    if limpo.is_empty() {
        return Err(informe());
    }
    if let Some(sem) = sem_aspas(limpo) {
        limpo = ddl::aparar(sem);
    }
    if limpo.is_empty() {
        return Err(informe());
    }
    Ok(PathBuf::from(prefs::caminho_exibido(expandir_usuario(limpo))))
}

/// O texto entre um par de aspas iguais (simples ou duplas) nas pontas.
fn sem_aspas(texto: &str) -> Option<&str> {
    let primeiro = texto.chars().next()?;
    if texto.chars().count() < 2 || !matches!(primeiro, '"' | '\'') || !texto.ends_with(primeiro) {
        return None;
    }
    // as aspas têm um byte só
    texto.get(1..texto.len() - 1)
}

/// Expande o `~` do começo do caminho (como no Windows), com as variáveis do processo.
fn expandir_usuario(texto: &str) -> String {
    expandir_usuario_com(texto, |nome| std::env::var_os(nome).map(|v| v.to_string_lossy().into_owned()))
}

const SEPARADORES: [char; 2] = ['\\', '/'];

/// O `ntpath.expanduser`, com as variáveis de ambiente vindas de `var` (os testes não mexem nas do processo):
///
/// - "~", "~\…" e "~/…" → a pasta pessoal: USERPROFILE (mesmo vazia); sem ela, HOMEDRIVE + HOMEPATH; sem
///   HOMEPATH, o texto fica como veio;
/// - "~fulano\…" → a pasta irmã da pessoal com esse nome ("C:\Users\fulano\…"), só quando a pasta pessoal se
///   chama USERNAME (senão o palpite seria errado e o texto fica como veio); "~<USERNAME>\…" é a própria.
///
/// O resto do texto (do primeiro separador em diante) é colado como veio; quem chama normaliza (abspath).
pub(crate) fn expandir_usuario_com(texto: &str, var: impl Fn(&str) -> Option<String>) -> String {
    let Some(depois) = texto.strip_prefix('~') else {
        return texto.to_string();
    };
    // fim do nome do usuário (o "~" tem um byte; os separadores também)
    let i = depois.find(SEPARADORES).map_or(texto.len(), |j| j + 1);
    let casa = match var("USERPROFILE") {
        Some(perfil) => perfil,
        None => match var("HOMEPATH") {
            None => return texto.to_string(),
            Some(caminho) => juntar(&var("HOMEDRIVE").unwrap_or_default(), &caminho),
        },
    };
    let alvo = texto.get(1..i).unwrap_or_default();
    let casa = if alvo.is_empty() {
        casa
    } else {
        let atual = var("USERNAME");
        if atual.as_deref() == Some(alvo) {
            casa
        } else {
            // as pastas pessoais costumam ficar lado a lado, com o nome de cada usuário
            let (pasta, nome) = dividir(&casa);
            if atual.as_deref() != Some(nome) {
                return texto.to_string();
            }
            juntar(&pasta, alvo)
        }
    };
    format!("{casa}{}", texto.get(i..).unwrap_or_default())
}

/// O `ntpath.splitroot` (simplificado para unidade "C:" e "\\servidor\compartilhamento"): unidade, raiz
/// (um separador ou nada) e o resto.
fn separar_raiz(p: &str) -> (&str, &str, &str) {
    let e_sep = |c: char| SEPARADORES.contains(&c);
    let mut letras = p.chars();
    let (primeira, segunda) = (letras.next(), letras.next());
    let fim_unidade = match (primeira, segunda) {
        (Some(a), Some(b)) if e_sep(a) && e_sep(b) => {
            // \\servidor\compartilhamento: a unidade vai até o separador depois do compartilhamento
            let servidor = p[2..].find(SEPARADORES).map(|j| j + 2);
            let compartilhamento = servidor.and_then(|s| p[s + 1..].find(SEPARADORES).map(|j| j + s + 1));
            match compartilhamento {
                Some(fim) => fim,
                None => return (p, "", ""),
            }
        }
        (Some(a), Some(':')) => a.len_utf8() + 1,
        _ => 0,
    };
    let (unidade, resto) = p.split_at(fim_unidade);
    match resto.chars().next() {
        Some(c) if e_sep(c) => (unidade, &resto[..1], &resto[1..]),
        _ => (unidade, "", resto),
    }
}

/// O `ntpath.split`: (pasta, sem os separadores do fim; nome depois do último separador).
fn dividir(p: &str) -> (String, &str) {
    let (unidade, raiz, resto) = separar_raiz(p);
    let i = resto.rfind(SEPARADORES).map_or(0, |j| j + 1);
    let (cabeca, cauda) = resto.split_at(i);
    (format!("{unidade}{raiz}{}", cabeca.trim_end_matches(SEPARADORES)), cauda)
}

/// O `ntpath.join` de dois pedaços.
fn juntar(a: &str, b: &str) -> String {
    let (mut unidade, mut raiz, mut caminho) = {
        let (u, r, c) = separar_raiz(a);
        (u.to_string(), r.to_string(), c.to_string())
    };
    let (b_unidade, b_raiz, b_caminho) = separar_raiz(b);
    if !b_raiz.is_empty() {
        // o segundo é absoluto
        if !b_unidade.is_empty() || unidade.is_empty() {
            unidade = b_unidade.to_string();
        }
        raiz = b_raiz.to_string();
        caminho = b_caminho.to_string();
    } else if !b_unidade.is_empty() && b_unidade != unidade && b_unidade.to_lowercase() != unidade.to_lowercase() {
        // outra unidade: o primeiro é ignorado
        unidade = b_unidade.to_string();
        raiz = b_raiz.to_string();
        caminho = b_caminho.to_string();
    } else {
        if !b_unidade.is_empty() {
            unidade = b_unidade.to_string();
        }
        if !caminho.is_empty() && !caminho.ends_with(SEPARADORES) {
            caminho.push('\\');
        }
        caminho.push_str(b_caminho);
    }
    if !caminho.is_empty() && raiz.is_empty() && !unidade.is_empty() && !unidade.ends_with([':', '\\', '/']) {
        return format!("{unidade}\\{caminho}");
    }
    format!("{unidade}{raiz}{caminho}")
}

/// Tira o prefixo `\\?\` que o `canonicalize` do Windows põe (`\\?\UNC\srv\x` → `\\srv\x`).
fn sem_prefixo_estendido(caminho: PathBuf) -> PathBuf {
    let texto = caminho.as_os_str().to_string_lossy();
    if let Some(resto) = texto.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{resto}"));
    }
    if let Some(resto) = texto.strip_prefix(r"\\?\")
        && resto.as_bytes().get(1) == Some(&b':')
    {
        return PathBuf::from(resto);
    }
    caminho
}

/// Caminho absoluto, com links e nomes curtos (8.3) resolvidos na parte que existe; o
/// resto (um arquivo que ainda vai ser criado) é anexado como veio.
pub(crate) fn resolver(caminho: &Path) -> io::Result<PathBuf> {
    let caminho = if caminho.as_os_str().is_empty() { Path::new(".") } else { caminho };
    let absoluto = std::path::absolute(caminho)?;
    let mut base = absoluto.clone();
    let mut resto: Vec<OsString> = Vec::new();
    loop {
        if let Ok(real) = fs::canonicalize(&base) {
            let mut saida = sem_prefixo_estendido(real);
            for parte in resto.iter().rev() {
                saida.push(parte);
            }
            return Ok(saida);
        }
        match (base.parent(), base.file_name()) {
            (Some(pai), Some(nome)) => {
                resto.push(nome.to_os_string());
                base = pai.to_path_buf();
            }
            _ => return Ok(absoluto),
        }
    }
}

/// O `_chave_caminho`: resolvido e sem diferença de caixa nem de barra, para comparar destinos.
pub(crate) fn chave_destino(caminho: &Path) -> String {
    match resolver(caminho) {
        Ok(resolvido) => prefs::chave_caminho(resolvido),
        Err(_) => prefs::chave_caminho(caminho),
    }
}

/// O nome do arquivo do registro.
const NOME_LOG: &str = "databox.log";

/// O registro do aplicativo, na pasta do DataBox:
/// `%LOCALAPPDATA%\DataBox\databox.log` (sem LOCALAPPDATA, a pasta pessoal; fora do Windows, a pasta de estado
/// do XDG: ver `comum::app::pasta_de_estado`).
pub fn arquivo_log() -> PathBuf {
    comum::app::pasta_de_estado().join(comum::app::PASTA).join(NOME_LOG)
}

/// O [`arquivo_log`] com o LOCALAPPDATA e a pasta pessoal vindos de fora (os testes não mexem nas variáveis do
/// processo).
#[cfg(test)]
pub(crate) fn arquivo_log_de(var: Option<OsString>, casa: impl FnOnce() -> PathBuf) -> PathBuf {
    comum::app::base_ou(var, casa).join(comum::app::PASTA).join(NOME_LOG)
}

/// O `_arquivos_do_app`: preferências, a trava delas e o registro com as cópias da rotação (`databox.log.1`…
/// `databox.log.9`). Gravar um exportado por cima deles apagaria os dados do aplicativo.
pub(crate) fn arquivos_do_app(prefs: &prefs::Preferencias, log: &Path) -> HashSet<String> {
    let mut candidatos = vec![prefs.arquivo().to_path_buf(), prefs.arquivo_trava(), log.to_path_buf()];
    for i in 1..10 {
        let mut nome = log.as_os_str().to_os_string();
        nome.push(format!(".{i}"));
        candidatos.push(PathBuf::from(nome));
    }
    candidatos.iter().map(|c| chave_destino(c)).collect()
}

/// Sufixo de nome com 8 letras, números ou "_" (como os do `tempfile.mkstemp`).
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

/// Erro de disco ao gravar; arquivo travado por outro programa (violação de compartilhamento ou de trava) é
/// falta de permissão.
fn erro_de_gravacao(e: &io::Error, caminho: Option<&Path>, contexto: &str) -> ErroBanco {
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

/// O `mkstemp(prefix=".<nome>.", suffix=".tmp", dir=<pasta>)`.
fn criar_temporario(pasta: &Path, nome: &str) -> io::Result<(PathBuf, File)> {
    let mut ultimo = None;
    for _ in 0..100 {
        let caminho = pasta.join(format!(".{nome}.{}.tmp", sufixo_aleatorio()));
        match OpenOptions::new().write(true).create_new(true).open(&caminho) {
            Ok(arquivo) => return Ok((caminho, arquivo)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => ultimo = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(ultimo.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)))
}

/// O `_gravar_arquivo`: grava num temporário ao lado e troca no fim, para não deixar arquivo pela metade.
pub(crate) fn gravar_arquivo(alvo: &Path, dados: &[u8]) -> Resultado<()> {
    if alvo.is_dir() {
        return Err(ErroBanco::novo(format!("O destino escolhido é uma pasta, não um arquivo: {}.", alvo.display())));
    }
    let pasta = alvo.parent().unwrap_or(Path::new(""));
    if !pasta.is_dir() {
        return Err(ErroBanco::com_status(format!("A pasta de destino não existe: {}.", pasta.display()), 404));
    }
    let contexto = "Não foi possível gravar o arquivo.";
    let nome = alvo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (temporario, mut arquivo) = criar_temporario(pasta, &nome).map_err(|e| erro_de_gravacao(&e, None, contexto))?;
    let escrito = arquivo.write_all(dados).and_then(|()| arquivo.flush());
    drop(arquivo);
    if let Err(e) = escrito.and_then(|()| fs::rename(&temporario, alvo)) {
        let _ = fs::remove_file(&temporario);
        return Err(erro_de_gravacao(&e, Some(alvo), contexto));
    }
    Ok(())
}
