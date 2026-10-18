//! Intermediário entre o rustc e o gcc do MinGW na hora de ligar (linker).
//!
//! O gcc da WinLibs roda com página de código UTF-8 e o ld com a ANSI do Windows (1252): um caminho com
//! acento (C:\Users\Conceição\…) não chega certo aos dois por nenhuma codificação, e o link quebra com
//! "cannot find …" ou "cannot open output file …". Este programa troca cada caminho que não é ASCII pelo nome
//! curto 8.3 do Windows (C:\Users\CONCEI~1\…), que é ASCII, tanto nos argumentos quanto dentro dos arquivos
//! de resposta (`@arquivo`) que o rustc escreve quando a lista é longa, e chama o gcc.
//!
//! O mesmo vale para outras ferramentas do MinGW: o `dlltool`, que o rustc chama para montar bibliotecas de
//! importação (`raw-dylib`), e o `windres`, que o build.rs do editor chama para embutir ícone e versão. Uma
//! cópia deste programa com o nome da ferramenta (`dlltool.exe`, `windres.exe`), à frente no PATH, encurta os
//! caminhos e chama a de verdade, indicada em VINCULAR_<NOME> (VINCULAR_DLLTOOL, VINCULAR_WINDRES).
//!
//! Compilado pelo `cargo.ps1` (rustc direto, sem cargo) e ligado por ele em
//! CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::process::{Command, ExitCode};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetShortPathNameW(longo: *const u16, curto: *mut u16, tamanho: u32) -> u32;
}

/// Nome curto de um arquivo ou pasta que existe; None se o Windows não der um.
fn curto_existente(caminho: &str) -> Option<String> {
    let largo: Vec<u16> = Path::new(caminho).as_os_str().encode_wide().chain([0]).collect();
    let mut saida = vec![0u16; 1024];
    // SAFETY: `largo` termina em zero; `saida` tem o tamanho informado.
    let n = unsafe { GetShortPathNameW(largo.as_ptr(), saida.as_mut_ptr(), saida.len() as u32) };
    if n == 0 || n as usize >= saida.len() {
        return None;
    }
    saida.truncate(n as usize);
    OsString::from_wide(&saida).into_string().ok()
}

/// Nome curto de um caminho que pode ainda não existir (o arquivo de saída): encurta a pasta e mantém o nome.
fn encurtar(caminho: &str) -> Option<String> {
    if caminho.is_ascii() {
        return Some(caminho.to_string());
    }
    if let Some(c) = curto_existente(caminho) {
        return c.is_ascii().then_some(c);
    }
    let corte = caminho.rfind(['\\', '/'])?;
    let (pasta, nome) = caminho.split_at(corte);
    if !nome.is_ascii() {
        return None;
    }
    Some(format!("{}{nome}", encurtar(pasta)?))
}

/// Encurta o caminho absoluto que houver dentro do argumento ("-LC:\…", "-Wl,--out-implib=C:\…" ou só o caminho).
fn tratar(arg: &str) -> Result<String, String> {
    if arg.is_ascii() {
        return Ok(arg.to_string());
    }
    let b = arg.as_bytes();
    let inicio = (0..b.len().saturating_sub(2))
        .find(|&i| b[i].is_ascii_alphabetic() && b[i + 1] == b':' && (b[i + 2] == b'\\' || b[i + 2] == b'/'))
        .ok_or_else(|| format!("argumento com acento e sem caminho absoluto: {arg}"))?;
    let (prefixo, caminho) = arg.split_at(inicio);
    let curto = encurtar(caminho).ok_or_else(|| format!("sem nome curto 8.3 para {caminho}"))?;
    Ok(format!("{prefixo}{curto}"))
}

/// Lê um arquivo de resposta no formato do rustc para o gcc: um argumento por linha, com `\` e espaço
/// escapados por `\`.
fn ler_resposta(texto: &str) -> Vec<String> {
    let mut argumentos = Vec::new();
    for linha in texto.lines() {
        let mut arg = String::with_capacity(linha.len());
        let mut chars = linha.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(seguinte) = chars.next() {
                    arg.push(seguinte);
                }
            } else {
                arg.push(c);
            }
        }
        if !arg.is_empty() {
            argumentos.push(arg);
        }
    }
    argumentos
}

fn escrever_resposta(argumentos: &[String]) -> String {
    let mut texto = String::new();
    for arg in argumentos {
        for c in arg.chars() {
            if c == '\\' || c == ' ' {
                texto.push('\\');
            }
            texto.push(c);
        }
        texto.push('\n');
    }
    texto
}

/// Programa de verdade a chamar: a ferramenta com o nome deste executável (VINCULAR_DLLTOOL,
/// VINCULAR_WINDRES…) ou, com o nome vincular, o gcc (como linker).
fn programa() -> OsString {
    let proprio = env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    if proprio.starts_with("vincular") {
        return env::var_os("VINCULAR_GCC").unwrap_or_else(|| OsString::from("x86_64-w64-mingw32-gcc"));
    }
    env::var_os(format!("VINCULAR_{}", proprio.to_uppercase()))
        .unwrap_or_else(|| OsString::from(format!("x86_64-w64-mingw32-{proprio}")))
}

fn executar() -> Result<i32, String> {
    let gcc = programa();
    let mut argumentos: Vec<String> = Vec::new();
    for (i, arg) in env::args_os().skip(1).enumerate() {
        let arg = arg.into_string().map_err(|a| format!("argumento que não é Unicode: {a:?}"))?;
        let Some(arquivo) = arg.strip_prefix('@') else {
            argumentos.push(tratar(&arg)?);
            continue;
        };
        let texto = fs::read_to_string(arquivo).map_err(|e| format!("não consegui ler {arquivo}: {e}"))?;
        let dentro = ler_resposta(&texto).iter().map(|a| tratar(a)).collect::<Result<Vec<_>, _>>()?;
        let copia = format!("{arquivo}.curto{i}");
        fs::write(&copia, escrever_resposta(&dentro)).map_err(|e| format!("não consegui gravar {copia}: {e}"))?;
        argumentos.push(format!("@{}", tratar(&copia)?));
    }
    let status = Command::new(&gcc)
        .args(&argumentos)
        .status()
        .map_err(|e| format!("não consegui executar {}: {e}", gcc.to_string_lossy()))?;
    Ok(status.code().unwrap_or(1))
}

fn main() -> ExitCode {
    match executar() {
        Ok(codigo) => ExitCode::from(codigo.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("vincular: {e}");
            ExitCode::from(2)
        }
    }
}
