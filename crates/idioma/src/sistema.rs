//! O idioma do sistema (decisão 12 de `planos-0.3.md`): o do sistema quando o DataBox o tem; o inglês quando não.

use crate::{IDIOMAS, INGLES, Idioma};

/// O idioma do sistema: o locale que o `sys-locale` lê (no Windows, o idioma de exibição; no Linux, `LC_ALL`,
/// `LC_MESSAGES` ou `LANG`), por [`de_locale`].
pub fn do_sistema() -> Idioma {
    de_locale(sys_locale::get_locale().as_deref())
}

/// O idioma que o DataBox usa para um locale: o idioma registrado com o mesmo código ("pt-BR", "en-US" → "en") ou,
/// sem ele, o de mesma língua ("pt-PT" e "pt_BR.UTF-8" → português); um locale que o DataBox não tem, `C`, `POSIX`
/// ou nenhum → inglês.
pub fn de_locale(locale: Option<&str>) -> Idioma {
    locale.and_then(reconhecer).unwrap_or(INGLES)
}

fn reconhecer(locale: &str) -> Option<Idioma> {
    // "pt_BR.UTF-8@euro" → "pt-br"
    let base = locale.split(['.', '@']).next().unwrap_or(locale).trim().replace('_', "-").to_ascii_lowercase();
    let lingua = base.split('-').next().unwrap_or(&base);
    if lingua.is_empty() {
        return None;
    }
    let lingua_de = |i: &Idioma| i.codigo().split('-').next().unwrap_or("").to_ascii_lowercase();
    IDIOMAS
        .iter()
        .find(|i| i.codigo().eq_ignore_ascii_case(&base))
        .or_else(|| IDIOMAS.iter().find(|i| lingua_de(i) == lingua))
        .copied()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::PORTUGUES;

    #[test]
    fn portugues_de_qualquer_jeito() {
        for l in ["pt-BR", "pt_BR.UTF-8", "pt", "PT-br", "pt-PT", "pt_PT@euro", "pt-Latn-BR"] {
            assert_eq!(de_locale(Some(l)), PORTUGUES, "{l}");
        }
    }

    #[test]
    fn ingles_e_o_resto_caem_no_ingles() {
        for l in ["en-US", "en_GB.UTF-8", "en", "fr-FR", "de_DE.UTF-8", "ja-JP", "C", "POSIX", "", "C.UTF-8", "-"] {
            assert_eq!(de_locale(Some(l)), INGLES, "{l}");
        }
        assert_eq!(de_locale(None), INGLES);
    }

    #[test]
    fn o_do_sistema_e_um_dos_registrados() {
        assert!(IDIOMAS.contains(&do_sistema()));
    }
}
