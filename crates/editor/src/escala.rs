//! O tamanho da interface que o sistema pede além da escala do monitor (revisão de 20/10/2026): no GNOME, o
//! `text-scaling-factor` de `org.gnome.desktop.interface` (Acessibilidade › Texto grande, ou o "Fator de escala" das
//! fontes no Ajustes). O Terminal e os apps GTK aumentam o texto por ele; o egui só segue a escala do monitor, e o
//! DataBox ficava menor que os outros programas na mesma tela. O main.rs aplica o fator como o zoom do egui antes do
//! primeiro quadro (a interface inteira cresce: os tamanhos das pranchetas são fixos, e só o texto quebraria o
//! desenho) e aumenta a janela inicial junto.
//!
//! Lido uma vez, antes da janela, pelo `gsettings` (sem dependência nova); sem ele, fora do GNOME ou com um valor
//! estranho, o fator é 1. Fora do Linux, também 1.

/// Os limites do fator aceito: fora deles, o valor é tratado como estranho (o GNOME aceita de 0,5 a 3).
const FATOR_MINIMO: f32 = 0.5;
const FATOR_MAXIMO: f32 = 3.0;

/// O fator do sistema: o `text-scaling-factor` do GNOME; 1 sem ele.
#[cfg(target_os = "linux")]
pub fn fator_do_sistema() -> f32 {
    let saida = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
        .stderr(std::process::Stdio::null())
        .output();
    match saida {
        Ok(s) if s.status.success() => ler_fator(&String::from_utf8_lossy(&s.stdout)).unwrap_or(1.0),
        _ => 1.0,
    }
}

/// Fora do Linux, o fator é sempre 1.
#[cfg(not(target_os = "linux"))]
pub fn fator_do_sistema() -> f32 {
    1.0
}

/// O fator na saída do `gsettings get` ("1.25\n"); None se não é um número entre [`FATOR_MINIMO`] e
/// [`FATOR_MAXIMO`].
pub fn ler_fator(saida: &str) -> Option<f32> {
    let fator: f32 = saida.trim().parse().ok()?;
    (fator.is_finite() && (FATOR_MINIMO..=FATOR_MAXIMO).contains(&fator)).then_some(fator)
}

/// Um tamanho da janela (largura, altura, em pontos da interface) multiplicado pelo fator: a janela inicial e a
/// mínima crescem junto com a interface, para caber o mesmo desenho.
pub fn tamanho_com_fator(tamanho: [f32; 2], fator: f32) -> [f32; 2] {
    [(tamanho[0] * fator).round(), (tamanho[1] * fator).round()]
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_o_fator_do_gsettings() {
        assert_eq!(ler_fator("1.25\n"), Some(1.25));
        assert_eq!(ler_fator("1.0"), Some(1.0));
        assert_eq!(ler_fator("0.75"), Some(0.75));
        for estranho in ["", "abc", "0", "-1", "7.5", "NaN", "inf", "'1.25'"] {
            assert_eq!(ler_fator(estranho), None, "{estranho:?}");
        }
    }

    #[test]
    fn a_janela_cresce_com_o_fator() {
        assert_eq!(tamanho_com_fator([1280.0, 800.0], 1.25), [1600.0, 1000.0]);
        assert_eq!(tamanho_com_fator([960.0, 600.0], 1.0), [960.0, 600.0]);
    }
}
