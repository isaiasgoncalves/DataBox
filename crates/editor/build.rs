//! Prepara o que o DataBox.exe embute:
//!
//! - o ícone, a versão e o manifesto (recursos/databox.rc), compilados pelo windres do MinGW (pelo intermediário de
//!   ferramentas\vincular.rs, que o cargo.ps1 põe à frente no PATH);
//! - `DATABOX_COMPILACAO`, o "15/10/2026 · fb34ec7" da janela "Sobre o DataBox": a data e o hash do último commit,
//!   lidos do git. Sem git (ou fora de um repositório), "desconhecida". A variável de ambiente de mesmo nome, se
//!   definida no build, vale no lugar (compilação reproduzível);
//! - `terceiros.txt` (em OUT_DIR), a lista da janela "Licenças de terceiros": as bibliotecas de fora que entram no
//!   programa, com versão e licença (ver [`terceiros`]). Nunca derruba o build: o que não for lido sai sem licença;
//! - `THIRD-PARTY-NOTICES.txt` (em OUT_DIR), os avisos de terceiros que vão no .zip da versão: a mesma lista, as
//!   fontes padrão do egui embutidas no exe e os textos de licença que vêm nos pacotes (ver [`avisos`]).

// script de build: sem CARGO_MANIFEST_DIR ou OUT_DIR não há o que fazer, e o cargo mostra o motivo
#![allow(clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=recursos");
    println!("cargo:rerun-if-env-changed=DATABOX_COMPILACAO");
    let manifesto = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let raiz = manifesto.join("..").join("..");
    println!("cargo:rustc-env=DATABOX_COMPILACAO={}", compilacao(&raiz));
    let saida = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    // nunca derruba o build: sem a lista, a janela mostra só o aviso
    let lista = terceiros(&raiz).unwrap_or_else(|| {
        println!("cargo:warning=não li o Cargo.lock: a lista de licenças de terceiros sai vazia");
        Vec::new()
    });
    let mut texto = String::new();
    for t in &lista {
        texto.push_str(&format!("{}\t{}\t{}\t{}\n", t.nome, t.versao, t.licenca, u8::from(t.direta)));
    }
    if let Err(e) = fs::write(saida.join("terceiros.txt"), texto) {
        println!("cargo:warning=não gravei terceiros.txt: {e}");
    }
    if let Err(e) = fs::write(saida.join("THIRD-PARTY-NOTICES.txt"), avisos(&lista)) {
        println!("cargo:warning=não gravei THIRD-PARTY-NOTICES.txt: {e}");
    }
    recursos_do_windows(&manifesto, &saida);
}

// ------------------------------------------------------------------------------------------------ compilação

/// "15/10/2026 · fb34ec7" (data do commit e hash curto), ou "desconhecida".
fn compilacao(raiz: &Path) -> String {
    if let Some(v) = env::var("DATABOX_COMPILACAO").ok().filter(|v| !v.trim().is_empty()) {
        return v.trim().to_string();
    }
    // o git do worktree: `logs/HEAD` muda a cada commit e a cada troca de branch
    if let Some(log) = git(raiz, &["rev-parse", "--git-path", "logs/HEAD"]) {
        let caminho = raiz.join(log.trim());
        if caminho.exists() {
            println!("cargo:rerun-if-changed={}", caminho.display());
        }
    }
    git(raiz, &["log", "-1", "--format=%h%x09%cd", "--date=format:%d/%m/%Y"])
        .and_then(|saida| {
            let (hash, data) = saida.trim().split_once('\t')?;
            (!hash.is_empty() && !data.is_empty()).then(|| format!("{data} · {hash}"))
        })
        .unwrap_or_else(|| "desconhecida".to_string())
}

fn git(raiz: &Path, args: &[&str]) -> Option<String> {
    let saida = Command::new("git").args(args).current_dir(raiz).output().ok()?;
    saida.status.success().then(|| String::from_utf8_lossy(&saida.stdout).into_owned())
}

// ------------------------------------------------------------------------------------------------ terceiros

/// Um pacote do Cargo.lock.
struct Pacote {
    nome: String,
    versao: String,
    /// Vem de um registro (tem `source`); os crates do próprio projeto não têm.
    de_fora: bool,
    dependencias: Vec<String>,
}

/// Uma biblioteca de fora que entra no programa.
struct Terceiro {
    nome: String,
    versao: String,
    /// O `license` do Cargo.toml dela ("arquivo X" com só `license-file`); vazio quando não deu para ler.
    licenca: String,
    /// O projeto depende dela direto (e não por outra).
    direta: bool,
    /// A pasta da fonte baixada, quando achada.
    pasta: Option<PathBuf>,
}

/// As bibliotecas em ordem alfabética (no terceiros.txt, linhas "nome\tversão\tlicença\tdireta", com direta 1 se o
/// projeto depende dela e 0 se vem por outra). As bibliotecas são as que o Cargo.lock traz **alcançáveis pelas dependências normais** dos crates do
/// projeto (as de teste, como o egui_kittest, ficam de fora). O Cargo.lock não separa por sistema: a lista inclui
/// também as que só entram em outros sistemas.
fn terceiros(raiz: &Path) -> Option<Vec<Terceiro>> {
    println!("cargo:rerun-if-changed={}", raiz.join("Cargo.lock").display());
    println!("cargo:rerun-if-changed={}", raiz.join("Cargo.toml").display());
    let pacotes = ler_lock(&fs::read_to_string(raiz.join("Cargo.lock")).ok()?);
    let diretas = dependencias_normais(raiz);

    let mut por_nome: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, p) in pacotes.iter().enumerate() {
        por_nome.entry(p.nome.as_str()).or_default().push(i);
    }
    // a busca por uma entrada de `dependencies` ("nome", "nome 1.2.3" ou "nome 1.2.3 (fonte)")
    let achar = |entrada: &str| -> Vec<usize> {
        let mut partes = entrada.split(' ');
        let (nome, versao) = (partes.next().unwrap_or(""), partes.next());
        let candidatos = por_nome.get(nome).cloned().unwrap_or_default();
        match versao {
            Some(v) => candidatos.into_iter().filter(|&i| pacotes[i].versao == v).collect(),
            None => candidatos,
        }
    };
    let mut alcancados: BTreeSet<usize> = BTreeSet::new();
    let mut a_visitar: Vec<usize> =
        diretas.iter().flat_map(|n| por_nome.get(n.as_str()).cloned().unwrap_or_default()).collect();
    let raizes: BTreeSet<usize> = a_visitar.iter().copied().filter(|&i| pacotes[i].de_fora).collect();
    while let Some(i) = a_visitar.pop() {
        // as dependências de um crate do projeto vêm do Cargo.toml (as de teste não contam), não do Cargo.lock
        if !alcancados.insert(i) || !pacotes[i].de_fora {
            continue;
        }
        for d in &pacotes[i].dependencias {
            a_visitar.extend(achar(d));
        }
    }

    let registro = pasta_dos_fontes();
    if registro.is_none() {
        // o build segue (o exe tem de sair mesmo offline), mas a lista sai sem licença nenhuma: fica dito
        println!(
            "cargo:warning=não achei a pasta das fontes do cargo ($CARGO_HOME/registry/src): a lista de licenças \
             de terceiros sai sem as licenças"
        );
    }
    let mut lista: Vec<Terceiro> = alcancados
        .into_iter()
        .filter(|&i| pacotes[i].de_fora)
        .map(|i| {
            let p = &pacotes[i];
            let (licenca, pasta) = match registro.as_deref() {
                Some(r) => match licenca_de(r, &p.nome, &p.versao) {
                    Ok((licenca, pasta)) => (licenca, Some(pasta)),
                    Err(motivo) => {
                        println!(
                            "cargo:warning=licença de {} {} em branco na lista de terceiros: {motivo}",
                            p.nome, p.versao
                        );
                        (String::new(), None)
                    }
                },
                None => (String::new(), None),
            };
            Terceiro { nome: p.nome.clone(), versao: p.versao.clone(), licenca, direta: raizes.contains(&i), pasta }
        })
        .collect();
    lista.sort_by(|a, b| (&a.nome, &a.versao).cmp(&(&b.nome, &b.versao)));
    Some(lista)
}

/// Os pacotes do Cargo.lock (o formato é regular: `[[package]]` com `name`, `version`, `source` e `dependencies`).
fn ler_lock(texto: &str) -> Vec<Pacote> {
    let mut pacotes: Vec<Pacote> = Vec::new();
    let mut em_dependencias = false;
    for linha in texto.lines() {
        let linha = linha.trim();
        if linha == "[[package]]" {
            pacotes.push(Pacote { nome: String::new(), versao: String::new(), de_fora: false, dependencias: vec![] });
            em_dependencias = false;
            continue;
        }
        let Some(p) = pacotes.last_mut() else { continue };
        if em_dependencias {
            if linha == "]" {
                em_dependencias = false;
            } else if let Some(d) = valor_entre_aspas(linha.trim_end_matches(',')) {
                p.dependencias.push(d);
            }
        } else if let Some(v) = linha.strip_prefix("name = ") {
            p.nome = valor_entre_aspas(v).unwrap_or_default();
        } else if let Some(v) = linha.strip_prefix("version = ") {
            p.versao = valor_entre_aspas(v).unwrap_or_default();
        } else if linha.starts_with("source = ") {
            p.de_fora = true;
        } else if linha == "dependencies = [" {
            em_dependencias = true;
        }
    }
    pacotes
}

fn valor_entre_aspas(texto: &str) -> Option<String> {
    let t = texto.trim();
    t.strip_prefix('"')?.strip_suffix('"').map(str::to_string)
}

/// Os nomes das dependências normais (e de build) de todos os crates do projeto: o que está em `[dependencies]`,
/// `[build-dependencies]` e `[target.….dependencies]`, nunca em `[dev-dependencies]`. Os crates do próprio projeto
/// entram também, para a busca seguir por eles.
fn dependencias_normais(raiz: &Path) -> BTreeSet<String> {
    let mut nomes = BTreeSet::new();
    let Ok(pasta) = fs::read_dir(raiz.join("crates")) else { return nomes };
    for entrada in pasta.flatten() {
        let caminho = entrada.path().join("Cargo.toml");
        if let Some(n) = entrada.file_name().to_str() {
            nomes.insert(n.to_string());
        }
        let Ok(texto) = fs::read_to_string(&caminho) else { continue };
        let mut valendo = false;
        for linha in texto.lines() {
            let linha = linha.trim();
            if linha.starts_with('[') {
                valendo = linha.trim_matches(|c| c == '[' || c == ']').ends_with("dependencies")
                    && !linha.contains("dev-dependencies");
            } else if valendo
                && !linha.contains("optional = true")
                && let Some((chave, _)) = linha.split_once('=')
            {
                let nome = chave.trim().split('.').next().unwrap_or("").trim();
                if !nome.is_empty() && !nome.starts_with('#') {
                    nomes.insert(nome.to_string());
                }
            }
        }
    }
    nomes
}

/// A pasta com as fontes baixadas (`$CARGO_HOME/registry/src`).
fn pasta_dos_fontes() -> Option<PathBuf> {
    let casa = env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| {
        env::var_os("USERPROFILE").or_else(|| env::var_os("HOME")).map(|h| PathBuf::from(h).join(".cargo"))
    })?;
    Some(casa.join("registry").join("src"))
}

/// A licença do pacote, lida do Cargo.toml da fonte baixada (em qualquer índice do registro): o campo `license` ou,
/// sem ele, o `license-file` ("arquivo LICENSE-X"), com a pasta da fonte. Sem nenhum dos dois, ou sem a fonte, o
/// motivo, para o aviso do build (a licença sai em branco, e o build segue).
fn licenca_de(registro: &Path, nome: &str, versao: &str) -> Result<(String, PathBuf), String> {
    let pasta = format!("{nome}-{versao}");
    let indices = fs::read_dir(registro).map_err(|e| format!("não li {}: {e}", registro.display()))?;
    for indice in indices.flatten() {
        let fonte = indice.path().join(&pasta);
        let toml = fonte.join("Cargo.toml");
        let Ok(texto) = fs::read_to_string(&toml) else { continue };
        let mut no_pacote = false;
        let mut arquivo = None;
        for linha in texto.lines() {
            let linha = linha.trim();
            if linha.starts_with('[') {
                no_pacote = linha == "[package]";
            } else if no_pacote && let Some(v) = linha.strip_prefix("license =") {
                return valor_entre_aspas(v)
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| (l, fonte))
                    .ok_or_else(|| "o campo license está vazio".to_string());
            } else if no_pacote && let Some(v) = linha.strip_prefix("license-file =") {
                arquivo = valor_entre_aspas(v).filter(|a| !a.trim().is_empty());
            }
        }
        return arquivo
            .map(|a| (format!("arquivo {a}"), fonte))
            .ok_or_else(|| "o Cargo.toml dela não tem license nem license-file".to_string());
    }
    Err(format!("a fonte {pasta} não está em {}", registro.display()))
}

// ------------------------------------------------------------------------------------------------ avisos

/// As fontes padrão do egui (crate `epaint_default_fonts`), compiladas dentro do exe: o nome, a licença e o arquivo
/// dela na pasta `fonts/` do pacote.
const FONTES_DO_EGUI: &[(&str, &str, &str)] = &[
    ("Ubuntu Light", "Ubuntu Font Licence 1.0", "UFL.txt"),
    ("Noto Emoji", "SIL Open Font License 1.1", "OFL.txt"),
    ("Hack", "MIT, with parts under the Bitstream Vera License", "Hack-Regular.txt"),
    ("emoji-icon-font", "MIT", "emoji-icon-font-mit-license.txt"),
];

/// O THIRD-PARTY-NOTICES.txt (em inglês, como os textos de licença que ele reúne): as fontes do egui, a lista das
/// bibliotecas e os arquivos de licença e de aviso (LICENSE*, LICENCE*, COPYING*, COPYRIGHT*, NOTICE*, UNLICENSE)
/// que vêm na fonte de cada uma, com os textos iguais juntos. A lista vem do Cargo.lock, que não separa por sistema:
/// as que só entram em outros sistemas não têm a fonte baixada num build do Windows e ficam sem texto.
fn avisos(lista: &[Terceiro]) -> String {
    let mut s = String::new();
    s.push_str(
        "DataBox - third-party notices\n\
         =============================\n\n\
         DataBox is licensed under the MIT License (see LICENSE.txt). DataBox.exe also contains the third-party\n\
         software listed below, each under its own license. The list comes from Cargo.lock; libraries marked\n\
         \"not in the Windows build\" only build on other operating systems and are not part of DataBox.exe.\n\n",
    );

    s.push_str("1. Fonts embedded by egui (epaint_default_fonts)\n");
    s.push_str("------------------------------------------------\n\n");
    for (fonte, licenca, _) in FONTES_DO_EGUI {
        s.push_str(&format!("{fonte}: {licenca}\n"));
    }
    s.push('\n');
    match lista.iter().find(|t| t.nome == "epaint_default_fonts").and_then(|t| t.pasta.as_ref()) {
        Some(pasta) => {
            for (fonte, _, arquivo) in FONTES_DO_EGUI {
                match fs::read_to_string(pasta.join("fonts").join(arquivo)) {
                    Ok(texto) => s.push_str(&format!(
                        "--- {fonte} ({arquivo}) ---\n\n{}\n\n",
                        texto.replace('\r', "").trim_end()
                    )),
                    Err(e) => println!("cargo:warning=não li a licença da fonte {fonte} ({arquivo}): {e}"),
                }
            }
        }
        None => println!(
            "cargo:warning=não achei a fonte do epaint_default_fonts: os avisos saem sem as licenças das fontes"
        ),
    }

    s.push_str("2. Libraries\n------------\n\n");
    let largura = lista.iter().map(|t| t.nome.len() + t.versao.len() + 1).max().unwrap_or(0);
    for t in lista {
        let licenca = if t.pasta.is_none() { "(not in the Windows build)" } else { t.licenca.as_str() };
        s.push_str(&format!("{:largura$}  {licenca}\n", format!("{} {}", t.nome, t.versao)));
    }
    s.push('\n');

    s.push_str("3. License texts\n----------------\n\n");
    s.push_str(
        "The license and notice files shipped with each library, verbatim. Identical texts are shown once, under\n\
         the list of libraries that ship them. Libraries without such a file are under the standard text of the\n\
         license named in section 2.\n\n",
    );
    // cada texto com as bibliotecas que o trazem, na ordem da lista
    let mut textos: Vec<(String, Vec<String>)> = Vec::new();
    let mut sem_arquivo: Vec<String> = Vec::new();
    for t in lista {
        let Some(pasta) = &t.pasta else { continue };
        let arquivos = arquivos_de_licenca(pasta);
        if arquivos.is_empty() {
            sem_arquivo.push(format!("{} {} ({})", t.nome, t.versao, t.licenca));
        }
        for (arquivo, texto) in arquivos {
            let quem = format!("{} {} ({arquivo})", t.nome, t.versao);
            match textos.iter_mut().find(|(x, _)| *x == texto) {
                Some((_, quem_traz)) => quem_traz.push(quem),
                None => textos.push((texto, vec![quem])),
            }
        }
    }
    let regua = "=".repeat(78);
    for (texto, quem_traz) in &textos {
        s.push_str(&format!("{regua}\n{}\n{}\n\n{}\n\n", quem_traz.join("\n"), "-".repeat(78), texto.trim_end()));
    }
    if !sem_arquivo.is_empty() {
        s.push_str(&format!(
            "{regua}\nNo license file in the package (standard text of the license named):\n\n{}\n",
            sem_arquivo.join("\n")
        ));
    }
    s
}

/// Os arquivos de licença e de aviso na raiz da fonte de um pacote, em ordem de nome, com o texto (sem `\r`).
fn arquivos_de_licenca(pasta: &Path) -> Vec<(String, String)> {
    let Ok(entradas) = fs::read_dir(pasta) else { return Vec::new() };
    let mut arquivos: Vec<(String, String)> = entradas
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let nome = e.file_name().to_str()?.to_string();
            let maiusculo = nome.to_uppercase();
            ["LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE"]
                .iter()
                .any(|p| maiusculo.starts_with(p))
                .then_some(())?;
            let texto = fs::read_to_string(e.path()).ok()?.replace('\r', "");
            Some((nome, texto))
        })
        .collect();
    arquivos.sort();
    arquivos
}

// ------------------------------------------------------------------------------------------------ recursos

fn recursos_do_windows(manifesto: &Path, saida: &Path) {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let pasta = manifesto.join("recursos");
    let saida = saida.join("recursos.o");
    let windres = env::var("WINDRES").unwrap_or_else(|_| "windres".into());
    let status = Command::new(&windres)
        .arg("--input")
        .arg(pasta.join("databox.rc"))
        .arg("--output")
        .arg(&saida)
        .arg("--output-format=coff")
        .arg("--include-dir")
        .arg(&pasta)
        .status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", saida.display()),
        Ok(s) => panic!("windres falhou ({s}) ao compilar recursos/databox.rc"),
        Err(e) => panic!("não consegui executar {windres}: {e} (rode o build pelo cargo.ps1 da raiz do projeto)"),
    }
}
