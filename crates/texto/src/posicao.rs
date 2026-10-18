//! Posições num texto: de byte para linha e coluna e de volta, e para o índice de caractere que o egui usa.
//!
//! O editor trabalha com `\n` (o [`crate::arquivo::carregar`] troca `\r\n` e `\r`), então a linha muda só no `\n`.
//! A coluna conta **caracteres** (a tabulação conta 1), não bytes e não colunas da tela: é o que a pessoa vê no
//! rodapé ("Linha 9, coluna 25") e no erro do JSON.

/// Leva `byte` para dentro do texto e para uma fronteira de caractere (para trás).
fn ajustar(texto: &str, byte: usize) -> usize {
    texto.floor_char_boundary(byte.min(texto.len()))
}

/// A linha e a coluna (as duas de 1, em caracteres) do byte. Um `byte` além do fim vale o fim; um `byte` no
/// meio de um caractere vale o começo dele.
pub fn linha_e_coluna(texto: &str, byte: usize) -> (usize, usize) {
    let antes = &texto[..ajustar(texto, byte)];
    let linha = 1 + antes.bytes().filter(|&b| b == b'\n').count();
    let comeco = antes.rfind('\n').map_or(0, |p| p + 1);
    (linha, 1 + antes[comeco..].chars().count())
}

/// O contrário de [`linha_e_coluna`]: o byte do começo da linha e coluna pedidas (de 1). Linha além da última
/// vale o fim do texto; coluna além do fim da linha vale o fim da linha (antes do `\n`).
pub fn byte_da_linha_e_coluna(texto: &str, linha: usize, coluna: usize) -> usize {
    let mut comeco = 0;
    for _ in 1..linha.max(1) {
        match texto[comeco..].find('\n') {
            Some(p) => comeco += p + 1,
            None => return texto.len(),
        }
    }
    let fim = texto[comeco..].find('\n').map_or(texto.len(), |p| comeco + p);
    let ate = coluna.max(1) - 1;
    texto[comeco..fim].char_indices().nth(ate).map_or(fim, |(i, _)| comeco + i)
}

/// O índice de caractere (o `CCursor` do egui) do byte.
pub fn caractere_do_byte(texto: &str, byte: usize) -> usize {
    texto[..ajustar(texto, byte)].chars().count()
}

/// O byte do caractere de índice `caractere` (além do fim: o fim do texto).
pub fn byte_do_caractere(texto: &str, caractere: usize) -> usize {
    texto.char_indices().nth(caractere).map_or(texto.len(), |(i, _)| i)
}

/// Quantas linhas o texto tem: um `\n` fecha uma linha, e o que sobra depois do último `\n` é mais uma linha
/// (sem ele, "a\nb\n" e "a\nb" têm 2 linhas). Texto vazio: 0.
pub fn contar_linhas(texto: &str) -> usize {
    let quebras = texto.bytes().filter(|&b| b == b'\n').count();
    quebras + usize::from(!texto.is_empty() && !texto.ends_with('\n'))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn linha_e_coluna_de_1_em_caracteres() {
        let t = "ab\ncd\n\nçã€x";
        assert_eq!(linha_e_coluna(t, 0), (1, 1));
        assert_eq!(linha_e_coluna(t, 1), (1, 2));
        assert_eq!(linha_e_coluna(t, 2), (1, 3), "o próprio \\n ainda é da linha 1");
        assert_eq!(linha_e_coluna(t, 3), (2, 1));
        assert_eq!(linha_e_coluna(t, 6), (3, 1), "linha vazia");
        assert_eq!(linha_e_coluna(t, 7), (4, 1));
        // "ç" e "ã" têm 2 bytes e "€" 3: a coluna conta caracteres
        assert_eq!(linha_e_coluna(t, 9), (4, 2));
        assert_eq!(linha_e_coluna(t, 11), (4, 3));
        assert_eq!(linha_e_coluna(t, 14), (4, 4));
        assert_eq!(linha_e_coluna(t, t.len()), (4, 5));
    }

    #[test]
    fn byte_fora_ou_no_meio_do_caractere_nao_derruba() {
        let t = "aç\nb";
        assert_eq!(linha_e_coluna(t, 2), (1, 2), "no meio do ç: o começo dele");
        assert_eq!(linha_e_coluna(t, 999), (2, 2), "além do fim: o fim");
        assert_eq!(linha_e_coluna("", 0), (1, 1));
        assert_eq!(linha_e_coluna("", 5), (1, 1));
    }

    #[test]
    fn tabulacao_conta_um() {
        assert_eq!(linha_e_coluna("\t\tx", 2), (1, 3));
    }

    #[test]
    fn a_ida_e_a_volta_se_encontram() {
        let t = "ab\ncd\n\nçã€x\nfim";
        for (byte, _) in t.char_indices() {
            let (l, c) = linha_e_coluna(t, byte);
            let de_volta = byte_da_linha_e_coluna(t, l, c);
            // o \n pode ser a coluna do fim da linha: volta ao mesmo lugar em todos os casos
            assert_eq!(de_volta, byte, "byte {byte} → {l}:{c}");
        }
        assert_eq!(byte_da_linha_e_coluna(t, 2, 99), 5, "coluna além do fim: antes do \\n");
        assert_eq!(byte_da_linha_e_coluna(t, 99, 1), t.len());
        assert_eq!(byte_da_linha_e_coluna(t, 0, 0), 0, "linha e coluna 0 valem 1");
    }

    #[test]
    fn indice_de_caractere() {
        let t = "aç€b";
        assert_eq!(caractere_do_byte(t, 0), 0);
        assert_eq!(caractere_do_byte(t, 1), 1);
        assert_eq!(caractere_do_byte(t, 3), 2);
        assert_eq!(caractere_do_byte(t, 6), 3);
        assert_eq!(caractere_do_byte(t, 99), 4);
        for i in 0..=4 {
            assert_eq!(caractere_do_byte(t, byte_do_caractere(t, i)), i);
        }
        assert_eq!(byte_do_caractere(t, 99), t.len());
    }

    #[test]
    fn contagem_de_linhas() {
        assert_eq!(contar_linhas(""), 0);
        assert_eq!(contar_linhas("a"), 1);
        assert_eq!(contar_linhas("a\n"), 1);
        assert_eq!(contar_linhas("a\nb"), 2);
        assert_eq!(contar_linhas("a\nb\n"), 2);
        assert_eq!(contar_linhas("\n"), 1);
        assert_eq!(contar_linhas("\n\n"), 2);
    }
}
