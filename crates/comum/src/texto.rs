//! A comparação sem caixa e sem acento das buscas e dos filtros: [`sem_acento`] (a chave de um texto inteiro) e
//! [`Agulha`] (o termo da busca da barra, que acha as ocorrências num texto sem copiá-lo: o `tabular` e o `planilha`
//! buscam célula a célula, e o texto de 32 MiB do leitor, numa passada).

use std::collections::VecDeque;
use std::ops::Range;
use std::str::CharIndices;

/// A letra sem acento de uma letra latina acentuada (o `normalize("NFD")` do JavaScript seguido da remoção das
/// marcas): a base sai em minúscula (`'Ç'` → `'c'`, `'ã'` → `'a'`) e qualquer outro caractere fica como está
/// (`'B'` → `'B'`). `None` para as marcas combinantes soltas (U+0300 a U+036F), que somem.
pub fn letra_sem_acento(c: char) -> Option<char> {
    let base = match c {
        '\u{300}'..='\u{36f}' => return None,
        'À'..='Å' | 'à'..='å' | 'Ā'..='ą' => 'a',
        'Ç' | 'ç' | 'Ć'..='č' => 'c',
        'Ď' | 'ď' => 'd',
        'È'..='Ë' | 'è'..='ë' | 'Ē'..='ě' => 'e',
        'Ĝ'..='ģ' => 'g',
        'Ĥ' | 'ĥ' => 'h',
        'Ì'..='Ï' | 'ì'..='ï' | 'Ĩ'..='İ' => 'i',
        'Ĵ' | 'ĵ' => 'j',
        'Ķ' | 'ķ' => 'k',
        'Ĺ'..='ľ' => 'l',
        'Ñ' | 'ñ' | 'Ń'..='ň' => 'n',
        'Ò'..='Ö' | 'ò'..='ö' | 'Ō'..='ő' => 'o',
        'Ŕ'..='ř' => 'r',
        'Ś'..='š' => 's',
        'Ţ'..='ť' => 't',
        'Ù'..='Ü' | 'ù'..='ü' | 'Ũ'..='ų' => 'u',
        'Ŵ' | 'ŵ' => 'w',
        'Ý' | 'ý' | 'ÿ' | 'Ŷ'..='Ÿ' => 'y',
        'Ź'..='ž' => 'z',
        outro => outro,
    };
    Some(base)
}

/// O texto sem acentos e em minúsculas, a chave das buscas e dos filtros: o filtro do
/// Explorador, a busca da barra na Visão geral e na Estrutura. `"Ações Úteis"` → `"acoes uteis"`.
pub fn sem_acento(texto: &str) -> String {
    texto.chars().filter_map(letra_sem_acento).flat_map(char::to_lowercase).collect()
}

/// Os caracteres de `c` na comparação: sem acento e em minúsculas (nenhum, para a marca combinante solta).
fn dobrar(c: char) -> impl Iterator<Item = char> {
    letra_sem_acento(c).into_iter().flat_map(char::to_lowercase)
}

/// O termo de uma busca, pronto para achar as ocorrências sem caixa e sem acento (`"sao"` acha `"São"`). A busca é a do
/// KMP: uma passada pelo texto, sem voltar e sem copiar, com a memória do tamanho do termo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agulha {
    termo: Vec<char>,
    /// Para cada prefixo do termo, o tamanho do maior prefixo que também termina nele (a tabela do KMP).
    falha: Vec<usize>,
}

impl Agulha {
    /// O termo `termo` sem acento e em minúsculas; None se não sobra nada para buscar (vazio ou só marcas).
    pub fn nova(termo: &str) -> Option<Agulha> {
        let termo: Vec<char> = termo.chars().flat_map(dobrar).collect();
        if termo.is_empty() {
            return None;
        }
        let mut falha = vec![0; termo.len()];
        let mut k = 0;
        for (i, c) in termo.iter().enumerate().skip(1) {
            while k > 0 && termo.get(k) != Some(c) {
                k = falha.get(k - 1).copied().unwrap_or(0);
            }
            if termo.get(k) == Some(c) {
                k += 1;
            }
            if let Some(f) = falha.get_mut(i) {
                *f = k;
            }
        }
        Some(Agulha { termo, falha })
    }

    /// O termo aparece em `texto`.
    pub fn em(&self, texto: &str) -> bool {
        self.achados(texto).next().is_some()
    }

    /// As ocorrências do termo em `texto`, da esquerda para a direita e sem se sobrepor, como trechos em bytes de
    /// `texto` (do primeiro caractere da ocorrência ao fim do último).
    pub fn achados<'a, 't>(&'a self, texto: &'t str) -> Achados<'a, 't> {
        Achados { agulha: self, chars: texto.char_indices(), casados: 0, inicios: VecDeque::new() }
    }
}

/// As ocorrências de uma [`Agulha`] num texto (veja [`Agulha::achados`]).
#[derive(Debug)]
pub struct Achados<'a, 't> {
    agulha: &'a Agulha,
    chars: CharIndices<'t>,
    /// Quantos caracteres do termo já casaram.
    casados: usize,
    /// O byte de onde veio cada um dos últimos caracteres comparados (no máximo o tamanho do termo).
    inicios: VecDeque<usize>,
}

impl Iterator for Achados<'_, '_> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Range<usize>> {
        let (termo, falha) = (&self.agulha.termo, &self.agulha.falha);
        for (byte, c) in self.chars.by_ref() {
            for d in dobrar(c) {
                if self.inicios.len() == termo.len() {
                    self.inicios.pop_front();
                }
                self.inicios.push_back(byte);
                while self.casados > 0 && termo.get(self.casados) != Some(&d) {
                    self.casados = falha.get(self.casados - 1).copied().unwrap_or(0);
                }
                if termo.get(self.casados) == Some(&d) {
                    self.casados += 1;
                }
                if self.casados == termo.len() {
                    let inicio = self.inicios.front().copied().unwrap_or(byte);
                    self.casados = 0;
                    self.inicios.clear();
                    return Some(inicio..byte + c.len_utf8());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// As ocorrências como (início, fim), em bytes.
    fn achados(termo: &str, texto: &str) -> Vec<(usize, usize)> {
        Agulha::nova(termo).expect("termo").achados(texto).map(|r| (r.start, r.end)).collect()
    }

    #[test]
    fn sem_acento_para_buscas() {
        assert_eq!(sem_acento("Tabela com Espaço"), "tabela com espaco");
        assert_eq!(sem_acento("a\u{303}o"), "ao");
        assert_eq!(letra_sem_acento('B'), Some('B'));
        assert_eq!(letra_sem_acento('\u{301}'), None);
    }

    #[test]
    fn agulha_acha_sem_caixa_nem_acento_em_bytes_do_texto() {
        let texto = "Ação em São Paulo; sao paulo; SÃO";
        assert_eq!(achados("são", texto), [(10, 14), (22, 25), (33, 37)]);
        assert_eq!(&texto[10..14], "São");
        assert_eq!(achados("ACAO", texto), [(0, 6)]);
        // a marca combinante solta some dos dois lados
        assert_eq!(achados("sao", "Sa\u{303}o"), [(0, 5)]);
        assert!(Agulha::nova("").is_none() && Agulha::nova("\u{301}").is_none());
        assert!(Agulha::nova("x").expect("x").achados("").next().is_none());
    }

    #[test]
    fn agulha_volta_pelo_kmp_e_nao_se_sobrepoe() {
        assert_eq!(achados("aab", "aaab"), [(1, 4)]);
        assert_eq!(achados("abab", "abababab"), [(0, 4), (4, 8)]);
        assert_eq!(achados("aa", "aaaaa"), [(0, 2), (2, 4)]);
        assert_eq!(achados("abc", "ababcabc"), [(2, 5), (5, 8)]);
        let agulha = Agulha::nova("Recife").expect("termo");
        assert!(agulha.em("de RECIFE a Olinda"));
        assert!(!agulha.em("Recif"));
    }
}
