//! Os limites de tamanho do editor de texto e as contas que dependem deles.
//!
//! Os números saíram da medição do `TextEdit` do egui com 0,5, 1 e 2 MiB (`tela_texto\tests\medicao.rs`). Todo o
//! resto do DataBox lê daqui, nunca repete o número.

/// Até este tamanho (512 KiB) o arquivo abre no editor. Acima, abre no leitor, só para leitura. A medição do passo 8
/// (`tela_texto\tests\medicao.rs`) deu uns 50 ms por tecla a 0,5 MiB e 150 a 200 ms a 1 MiB: o coordenador ficou com
/// 0,5 MiB para a digitação continuar instantânea (18/10/2026).
pub const LIMITE_EDICAO: u64 = 524_288;

/// Até este tamanho (32 MiB) o arquivo abre (no leitor, se passar de [`LIMITE_EDICAO`]). Acima, a leitura é
/// recusada com a frase do tamanho.
pub const LIMITE_LEITURA: u64 = 33_554_432;

/// Até este tamanho (8 MiB) a estrutura do JSON (a árvore) é montada. Acima, só a validação roda
/// ([`crate::json::validar`]), que não guarda nó nenhum.
pub const LIMITE_ESTRUTURA: u64 = 8_388_608;

/// Quantas fotografias do texto o `Undoer` do egui pode guardar, no máximo e no mínimo.
const UNDOS_MAXIMO: usize = 100;
const UNDOS_MINIMO: usize = 10;

/// Quanto de memória o desfazer pode ocupar (todas as fotografias do texto inteiro juntas).
const ORCAMENTO_DO_UNDO: u64 = 32 * 1_048_576;

/// O `max_undos` do `Undoer` para um texto deste tamanho (em bytes): `clamp(32 MiB / tamanho, 10, 100)`. O
/// `Undoer` guarda uma fotografia do texto inteiro por passo; 100 passos de 1 MiB seriam 100 MiB.
pub fn max_undos(tamanho: u64) -> usize {
    if tamanho == 0 {
        return UNDOS_MAXIMO;
    }
    usize::try_from(ORCAMENTO_DO_UNDO / tamanho).unwrap_or(UNDOS_MAXIMO).clamp(UNDOS_MINIMO, UNDOS_MAXIMO)
}

/// O tamanho como a pessoa lê: "812 bytes", "6,2 KB", "6,2 MB", "1,5 GB" (base 1024, como o Windows, com vírgula
/// decimal e sem o ",0" inútil).
pub fn tamanho_em_texto(bytes: u64) -> String {
    const UNIDADES: [&str; 3] = ["KB", "MB", "GB"];
    if bytes < 1024 {
        return if bytes == 1 { "1 byte".to_string() } else { format!("{bytes} bytes") };
    }
    // só para mostrar: a perda de precisão do f64 acima de 2^53 bytes não importa
    let mut valor = bytes as f64 / 1024.0;
    let mut unidade = 0;
    while valor >= 1024.0 && unidade + 1 < UNIDADES.len() {
        valor /= 1024.0;
        unidade += 1;
    }
    let texto = format!("{valor:.1}");
    let texto = texto.strip_suffix(".0").unwrap_or(&texto).replace('.', ",");
    format!("{texto} {}", UNIDADES.get(unidade).copied().unwrap_or("GB"))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limites_do_undo_pelo_tamanho() {
        assert_eq!(max_undos(0), 100);
        assert_eq!(max_undos(1_000), 100, "texto pequeno: o máximo");
        assert_eq!(max_undos(327_680), 100, "32 MiB / 320 KiB = 100");
        assert_eq!(max_undos(524_288), 64, "0,5 MiB");
        assert_eq!(max_undos(LIMITE_EDICAO), 64, "o limite da edição, 0,5 MiB");
        assert_eq!(max_undos(1_048_576), 32, "1 MiB");
        assert_eq!(max_undos(2_097_152), 16, "2 MiB");
        assert_eq!(max_undos(LIMITE_LEITURA), 10, "o mínimo");
        assert_eq!(max_undos(u64::MAX), 10);
        // nunca sobe quando o texto cresce
        let mut anterior = usize::MAX;
        for t in (0..40).map(|i| 1u64 << i) {
            let n = max_undos(t);
            assert!(n <= anterior && (10..=100).contains(&n), "{t}: {n}");
            anterior = n;
        }
    }

    #[test]
    fn constantes_do_plano() {
        assert_eq!(LIMITE_EDICAO, 512 * 1024);
        assert_eq!(LIMITE_LEITURA, 32 * 1024 * 1024);
        assert_eq!(LIMITE_ESTRUTURA, 8 * 1024 * 1024);
        const { assert!(LIMITE_EDICAO < LIMITE_ESTRUTURA && LIMITE_ESTRUTURA < LIMITE_LEITURA) };
    }

    #[test]
    fn tamanho_em_texto_com_virgula() {
        assert_eq!(tamanho_em_texto(0), "0 bytes");
        assert_eq!(tamanho_em_texto(1), "1 byte");
        assert_eq!(tamanho_em_texto(612), "612 bytes");
        assert_eq!(tamanho_em_texto(1024), "1 KB");
        assert_eq!(tamanho_em_texto(1536), "1,5 KB");
        assert_eq!(tamanho_em_texto(6_500_000), "6,2 MB");
        assert_eq!(tamanho_em_texto(LIMITE_LEITURA), "32 MB");
        assert_eq!(tamanho_em_texto(LIMITE_LEITURA + 5_000_000), "36,8 MB");
        assert_eq!(tamanho_em_texto(3 * 1024 * 1024 * 1024), "3 GB");
    }
}
