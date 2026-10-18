//! Prepara o que o DataBox.exe embute:
//!
//! - o ícone, a versão e o manifesto (recursos/databox.rc), compilados pelo windres do MinGW (pelo intermediário de
//!   ferramentas\vincular.rs, que o cargo.ps1 põe à frente no PATH);
//! - `DATABOX_COMPILACAO`, o "15/10/2026 · fb34ec7" da janela "Sobre o DataBox": a data e o hash do último commit,
//!   lidos do git. Sem git (ou fora de um repositório), "desconhecida". A variável de ambiente de mesmo nome, se
//!   definida no build, vale no lugar (compilação reproduzível);
//! - `terceiros.txt` (em OUT_DIR), a lista da janela "Licenças de terceiros": as bibliotecas de fora que entram no
//!   programa, com versão e licença (ver [`terceiros`]). Nunca derruba o build: o que não for lido sai sem licença.

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
        String::new()
    });
    if let Err(e) = fs::write(saida.join("terceiros.txt"), lista) {
        println!("cargo:warning=não gravei terceiros.txt: {e}");
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

/// As linhas "nome\tversão\tlicença\tdireta" (direta: 1 se o projeto depende dela, 0 se vem por outra), em ordem
/// alfabética. As bibliotecas são as que o Cargo.lock traz **alcançáveis pelas dependências normais** dos crates do
/// projeto (as de teste, como o egui_kittest, ficam de fora). O Cargo.lock não separa por sistema: a lista inclui
/// também as que só entram em outros sistemas.
fn terceiros(raiz: &Path) -> Option<String> {
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
    let mut linhas: Vec<(String, String, String, bool)> = alcancados
        .into_iter()
        .filter(|&i| pacotes[i].de_fora)
        .map(|i| {
            let p = &pacotes[i];
            let licenca = match registro.as_deref() {
                Some(r) => licenca_de(r, &p.nome, &p.versao).unwrap_or_else(|motivo| {
                    println!(
                        "cargo:warning=licença de {} {} em branco na lista de terceiros: {motivo}",
                        p.nome, p.versao
                    );
                    String::new()
                }),
                None => String::new(),
            };
            (p.nome.clone(), p.versao.clone(), licenca, raizes.contains(&i))
        })
        .collect();
    linhas.sort();
    let mut texto = String::new();
    for (nome, versao, licenca, direta) in linhas {
        texto.push_str(&format!("{nome}\t{versao}\t{licenca}\t{}\n", u8::from(direta)));
    }
    Some(texto)
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
/// sem ele, o `license-file` ("arquivo LICENSE-X"). Sem nenhum dos dois, ou sem a fonte, o motivo, para o aviso do
/// build (a licença sai em branco, e o build segue).
fn licenca_de(registro: &Path, nome: &str, versao: &str) -> Result<String, String> {
    let pasta = format!("{nome}-{versao}");
    let indices = fs::read_dir(registro).map_err(|e| format!("não li {}: {e}", registro.display()))?;
    for indice in indices.flatten() {
        let toml = indice.path().join(&pasta).join("Cargo.toml");
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
                    .ok_or_else(|| "o campo license está vazio".to_string());
            } else if no_pacote && let Some(v) = linha.strip_prefix("license-file =") {
                arquivo = valor_entre_aspas(v).filter(|a| !a.trim().is_empty());
            }
        }
        return arquivo
            .map(|a| format!("arquivo {a}"))
            .ok_or_else(|| "o Cargo.toml dela não tem license nem license-file".to_string());
    }
    Err(format!("a fonte {pasta} não está em {}", registro.display()))
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
