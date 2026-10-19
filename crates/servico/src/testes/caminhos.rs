//! A limpeza do caminho digitado: o `~` do começo e as variáveis de ambiente, como no Windows. E o lugar do registro do aplicativo.

use std::path::{Path, PathBuf};

use super::erro_com;
use crate::caminhos::{arquivo_log_de, expandir_usuario_com, expandir_usuario_posix, limpar_caminho};

type Variaveis<'a> = &'a [(&'a str, &'a str)];

/// As variáveis de ambiente de um caso (as do processo ficam como estão: os testes rodam em paralelo).
fn ler(variaveis: Variaveis<'_>) -> impl Fn(&str) -> Option<String> + '_ {
    move |nome| variaveis.iter().find(|(k, _)| *k == nome).map(|(_, v)| (*v).to_string())
}

#[test]
fn expandir_usuario_como_o_ntpath() {
    let eu: Variaveis<'_> = &[("USERPROFILE", r"C:\Users\eu"), ("USERNAME", "eu")];
    let casos: &[(Variaveis<'_>, &str, &str)] = &[
        (eu, "~", r"C:\Users\eu"),
        (eu, r"~\x.db", r"C:\Users\eu\x.db"),
        (eu, "~/x.db", r"C:\Users\eu/x.db"),
        (eu, r"~\\x", r"C:\Users\eu\\x"),
        // ~outro_usuario: a pasta irmã da pessoal
        (eu, r"~fulano\x.db", r"C:\Users\fulano\x.db"),
        (eu, "~fulano", r"C:\Users\fulano"),
        (eu, "~fulano/x/y.db", r"C:\Users\fulano/x/y.db"),
        (eu, "~eu/x.db", r"C:\Users\eu/x.db"),
        (eu, r"~~\x", r"C:\Users\~\x"),
        (eu, "a~b", "a~b"),
        (eu, "", ""),
        // a pasta pessoal não tem o nome do usuário (caixa diferente, barra no fim, sem USERNAME): não adivinha
        (&[("USERPROFILE", r"C:\Users\eu"), ("USERNAME", "Eu")], r"~fulano\x.db", r"~fulano\x.db"),
        (&[("USERPROFILE", "C:\\Users\\eu\\"), ("USERNAME", "eu")], r"~fulano\x.db", r"~fulano\x.db"),
        (&[("USERPROFILE", r"C:\Users\eu")], r"~fulano\x.db", r"~fulano\x.db"),
        (&[("USERPROFILE", r"C:\eu"), ("USERNAME", "eu")], r"~fulano\x.db", r"C:\fulano\x.db"),
        (&[("USERPROFILE", "C:eu"), ("USERNAME", "eu")], r"~fulano\x.db", r"C:fulano\x.db"),
        (&[("USERPROFILE", r"\\srv\perfis\eu"), ("USERNAME", "eu")], r"~fulano\x.db", r"\\srv\perfis\fulano\x.db"),
        (&[("USERPROFILE", r"\\srv\eu"), ("USERNAME", "eu")], r"~fulano\x.db", r"~fulano\x.db"),
        // sem USERPROFILE: HOMEDRIVE + HOMEPATH; sem HOMEPATH, nada muda; USERPROFILE vazia ainda vale
        (
            &[("HOMEDRIVE", "D:"), ("HOMEPATH", r"\casa\eu"), ("USERNAME", "eu")],
            r"~fulano\x.db",
            r"D:\casa\fulano\x.db",
        ),
        (&[("HOMEDRIVE", "D:"), ("HOMEPATH", r"\casa\eu")], r"~\x.db", r"D:\casa\eu\x.db"),
        (&[("HOMEPATH", r"\casa\eu")], r"~\x.db", r"\casa\eu\x.db"),
        (&[], r"~\x.db", r"~\x.db"),
        (&[("USERPROFILE", "")], r"~\x.db", r"\x.db"),
    ];
    for (variaveis, texto, esperado) in casos {
        assert_eq!(expandir_usuario_com(texto, ler(variaveis)), *esperado, "{texto:?} com {variaveis:?}");
    }
}

#[test]
fn til_fora_do_windows_e_o_home() {
    let casa = || Some("/home/eu/".to_string());
    assert_eq!(expandir_usuario_posix("~", casa()), "/home/eu");
    assert_eq!(expandir_usuario_posix("~/dados/x.db", casa()), "/home/eu/dados/x.db");
    assert_eq!(expandir_usuario_posix("~fulano/x.db", casa()), "~fulano/x.db");
    assert_eq!(expandir_usuario_posix("~/x.db", None), "~/x.db");
    assert_eq!(expandir_usuario_posix("~", Some("/".to_string())), "/");
    assert_eq!(expandir_usuario_posix("/tmp/~/x", casa()), "/tmp/~/x");
}

#[test]
#[cfg(not(windows))]
fn limpar_caminho_fora_do_windows() {
    // o caminho absoluto continua absoluto (sem virar "\tmp\…" relativo à pasta atual)
    assert_eq!(limpar_caminho(Path::new(r#"  "/tmp/a/../x.db" "#), "o caminho").unwrap(), Path::new("/tmp/x.db"));
    let atual = std::env::current_dir().unwrap();
    assert_eq!(limpar_caminho(Path::new("x.db"), "o caminho").unwrap(), atual.join("x.db"));
    assert_eq!(erro_com(limpar_caminho(Path::new(" '' "), "o caminho"), 400).mensagem, "Informe o caminho.");
}

#[test]
#[cfg(windows)]
fn limpar_caminho_expande_o_usuario() {
    let perfil = std::env::var("USERPROFILE").unwrap();
    let pessoal = Path::new(&perfil);
    assert_eq!(limpar_caminho(Path::new(r#"  "~\x.db" "#), "o caminho").unwrap(), pessoal.join("x.db"));
    assert_eq!(limpar_caminho(Path::new("~/a/b.db"), "o caminho").unwrap(), pessoal.join("a").join("b.db"));
    // "~fulano\x.db": a pasta de perfil irmã (C:\Users\fulano\x.db), quando a pessoal tem o nome do usuário
    let usuario = std::env::var("USERNAME").unwrap_or_default();
    let esperado = if pessoal.file_name().is_some_and(|n| n == usuario.as_str()) {
        pessoal.parent().unwrap().join("fulano").join("x.db")
    } else {
        std::env::current_dir().unwrap().join("~fulano").join("x.db")
    };
    assert_eq!(limpar_caminho(Path::new(r"~fulano\x.db"), "o caminho").unwrap(), esperado);
    assert_eq!(erro_com(limpar_caminho(Path::new(" '' "), "o caminho"), 400).mensagem, "Informe o caminho.");
}

#[test]
fn arquivo_log_fica_em_localappdata_databox() {
    let local = PathBuf::from(r"C:\Users\eu\AppData\Local");
    let casa = PathBuf::from(r"C:\Users\eu");
    assert_eq!(
        arquivo_log_de(Some(local.clone().into_os_string()), || unreachable!()),
        local.join("DataBox").join("databox.log")
    );
    // sem LOCALAPPDATA (ou vazia), a pasta pessoal
    let na_casa = casa.join("DataBox").join("databox.log");
    assert_eq!(arquivo_log_de(None, || casa.clone()), na_casa);
    assert_eq!(arquivo_log_de(Some("".into()), || casa.clone()), na_casa);
    // o arquivo_log() de verdade segue a mesma regra com o LOCALAPPDATA do processo (só calcula)
    if cfg!(windows) {
        assert_eq!(crate::arquivo_log(), arquivo_log_de(std::env::var_os("LOCALAPPDATA"), comum::app::pasta_pessoal));
    }
}
