//! A identidade do aplicativo num lugar só: o nome e as pastas por pessoa onde ele guarda as preferências e o
//! registro. O que é de cada sistema fica isolado em `cfg(windows)`; os ramos de fora do Windows (as pastas do
//! XDG) ainda não são testados.

use std::ffi::OsString;
use std::path::PathBuf;

/// O nome do aplicativo, como a pessoa o lê.
pub const NOME: &str = "DataBox";
/// A pasta própria do aplicativo dentro da pasta de configuração e da pasta de estado.
pub const PASTA: &str = "DataBox";

/// A opção de tema que segue o claro ou escuro do sistema: "Seguir o Windows" no Windows, "Seguir o sistema" fora.
pub const SEGUIR_O_SISTEMA: &str = if cfg!(windows) { "Seguir o Windows" } else { "Seguir o sistema" };
/// O programa que abre um arquivo pelo tipo dele, na frase: "programa padrão do Windows" ou "do sistema".
pub const PROGRAMA_PADRAO: &str =
    if cfg!(windows) { "programa padrão do Windows" } else { "programa padrão do sistema" };

/// A pasta pessoal. No Windows: USERPROFILE (se não estiver vazia); senão
/// HOMEDRIVE + HOMEPATH; sem nenhuma, a pasta atual.
#[cfg(windows)]
pub fn pasta_pessoal() -> PathBuf {
    if let Some(perfil) = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()) {
        return PathBuf::from(perfil);
    }
    if let Some(caminho) = std::env::var_os("HOMEPATH") {
        let mut base = std::env::var_os("HOMEDRIVE").unwrap_or_default();
        base.push(caminho);
        return PathBuf::from(base);
    }
    PathBuf::from(".")
}

/// A pasta pessoal: $HOME; sem ela, a pasta atual.
#[cfg(not(windows))]
pub fn pasta_pessoal() -> PathBuf {
    base_ou(std::env::var_os("HOME"), || PathBuf::from("."))
}

/// Onde ficam as preferências: `%APPDATA%` (sem ela, a pasta pessoal). Fora do Windows, `$XDG_CONFIG_HOME` ou
/// `~/.config`.
pub fn pasta_de_configuracao() -> PathBuf {
    #[cfg(windows)]
    {
        base_ou(std::env::var_os("APPDATA"), pasta_pessoal)
    }
    #[cfg(not(windows))]
    {
        base_ou(std::env::var_os("XDG_CONFIG_HOME"), || pasta_pessoal().join(".config"))
    }
}

/// Onde fica o registro: `%LOCALAPPDATA%` (sem ela, a pasta pessoal). Fora do Windows, `$XDG_STATE_HOME` ou
/// `~/.local/state`.
pub fn pasta_de_estado() -> PathBuf {
    #[cfg(windows)]
    {
        base_ou(std::env::var_os("LOCALAPPDATA"), pasta_pessoal)
    }
    #[cfg(not(windows))]
    {
        base_ou(std::env::var_os("XDG_STATE_HOME"), || pasta_pessoal().join(".local").join("state"))
    }
}

/// O valor de uma variável de ambiente como pasta; ausente ou vazia, `casa()`. É a parte testável das pastas
/// acima (os testes não mexem nas variáveis do processo).
pub fn base_ou(var: Option<OsString>, casa: impl FnOnce() -> PathBuf) -> PathBuf {
    match var {
        Some(valor) if !valor.is_empty() => PathBuf::from(valor),
        _ => casa(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn base_ou_usa_a_variavel_e_cai_na_pasta_pessoal() {
        let casa = PathBuf::from(r"C:\Users\eu");
        let roaming = PathBuf::from(r"C:\Users\eu\AppData\Roaming");
        assert_eq!(base_ou(Some(roaming.clone().into_os_string()), || unreachable!()), roaming);
        assert_eq!(base_ou(None, || casa.clone()), casa);
        assert_eq!(base_ou(Some(OsString::new()), || casa.clone()), casa);
    }

    #[cfg(windows)]
    #[test]
    fn pastas_do_windows_seguem_as_variaveis_do_processo() {
        assert_eq!(pasta_de_configuracao(), base_ou(std::env::var_os("APPDATA"), pasta_pessoal));
        assert_eq!(pasta_de_estado(), base_ou(std::env::var_os("LOCALAPPDATA"), pasta_pessoal));
        assert!(pasta_pessoal().is_absolute() || pasta_pessoal() == std::path::Path::new("."));
    }
}
