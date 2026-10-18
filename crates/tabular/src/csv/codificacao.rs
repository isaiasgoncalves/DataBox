//! As codificações do texto de um CSV: UTF-8, UTF-8 com BOM e Windows-1252 (a do Excel em português quando se
//! salva "CSV (separado por vírgulas)"), com a tabela do Windows-1252 própria, sem biblioteca.
//!
//! Tanto o UTF-8 quanto o Windows-1252 guardam o separador, as aspas e as quebras de linha como os mesmos
//! bytes ASCII. Por isso a leitura acha os registros nos bytes e só decodifica o texto das células pedidas.

use std::borrow::Cow;

/// Os três bytes que abrem um arquivo UTF-8 com BOM.
pub const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Os caracteres dos bytes 0x80 a 0x9F do Windows-1252. Os cinco bytes sem uso (0x81, 0x8D, 0x8F, 0x90 e
/// 0x9D) vão para o ponto de código de mesmo valor, como faz o Windows, e assim voltam iguais na gravação.
const ALTA: [char; 32] = [
    '\u{20AC}', '\u{81}', '\u{201A}', '\u{192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2C6}', '\u{2030}',
    '\u{160}', '\u{2039}', '\u{152}', '\u{8D}', '\u{17D}', '\u{8F}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201C}',
    '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{2DC}', '\u{2122}', '\u{161}', '\u{203A}', '\u{153}', '\u{9D}',
    '\u{17E}', '\u{178}',
];

/// A codificação do arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codificacao {
    /// UTF-8 sem marca no começo.
    Utf8,
    /// UTF-8 com os três bytes `EF BB BF` no começo (o que o Excel precisa para abrir acentos, e o que o
    /// Exportar do DataBox grava).
    Utf8ComBom,
    /// Windows-1252 (Latin-1 do Windows).
    Windows1252,
}

impl Codificacao {
    pub const TODAS: [Codificacao; 3] = [Codificacao::Utf8, Codificacao::Utf8ComBom, Codificacao::Windows1252];

    /// O nome que as Propriedades mostram.
    pub fn rotulo(self) -> &'static str {
        match self {
            Codificacao::Utf8 => "UTF-8",
            Codificacao::Utf8ComBom => "UTF-8 com BOM",
            Codificacao::Windows1252 => "Windows-1252",
        }
    }

    /// O arquivo começa com os bytes do BOM.
    pub fn tem_bom(self) -> bool {
        self == Codificacao::Utf8ComBom
    }

    /// UTF-8, com ou sem BOM.
    pub fn e_utf8(self) -> bool {
        self != Codificacao::Windows1252
    }

    /// Escrevem o mesmo texto com os mesmos bytes (UTF-8 com e sem BOM só diferem no começo do arquivo).
    pub fn mesma_tabela(self, outra: Codificacao) -> bool {
        self.e_utf8() == outra.e_utf8()
    }

    /// Os bytes do texto (sem o BOM) como texto. UTF-8 inválido vira U+FFFD; todo byte é um caractere no
    /// Windows-1252.
    pub fn decodificar(self, bytes: &[u8]) -> Cow<'_, str> {
        if self.e_utf8() {
            return String::from_utf8_lossy(bytes);
        }
        if bytes.is_ascii() {
            // is_ascii garante UTF-8 válido
            return String::from_utf8_lossy(bytes);
        }
        Cow::Owned(bytes.iter().map(|&b| char_do_byte(b)).collect())
    }

    /// O texto como bytes da codificação. `Err` traz o primeiro caractere que a codificação não tem
    /// (só o Windows-1252 tem limite).
    pub fn codificar<'a>(self, texto: &'a str) -> Result<Cow<'a, [u8]>, char> {
        if self.e_utf8() || texto.is_ascii() {
            return Ok(Cow::Borrowed(texto.as_bytes()));
        }
        let mut saida = Vec::with_capacity(texto.len());
        for c in texto.chars() {
            saida.push(byte_do_char(c).ok_or(c)?);
        }
        Ok(Cow::Owned(saida))
    }

    /// Acrescenta o texto, codificado, ao fim de `saida`.
    pub(crate) fn anexar(self, saida: &mut Vec<u8>, texto: &str) -> Result<(), char> {
        if self.e_utf8() || texto.is_ascii() {
            saida.extend_from_slice(texto.as_bytes());
            return Ok(());
        }
        for c in texto.chars() {
            saida.push(byte_do_char(c).ok_or(c)?);
        }
        Ok(())
    }
}

/// O caractere de um byte do Windows-1252.
pub fn char_do_byte(b: u8) -> char {
    match b {
        0x80..=0x9F => ALTA.get(usize::from(b - 0x80)).copied().unwrap_or('\u{FFFD}'),
        _ => char::from(b),
    }
}

/// O byte do Windows-1252 de um caractere, se ele existir.
pub fn byte_do_char(c: char) -> Option<u8> {
    let n = u32::from(c);
    match n {
        0..=0x7F | 0xA0..=0xFF => u8::try_from(n).ok(),
        _ => ALTA.iter().position(|&a| a == c).and_then(|i| u8::try_from(i + 0x80).ok()),
    }
}

/// A codificação de um trecho de arquivo inteiro: BOM → UTF-8 com BOM; UTF-8 válido (inclusive só ASCII) →
/// UTF-8; o resto → Windows-1252. Com `truncado`, um caractere UTF-8 cortado no fim do trecho não conta como
/// erro.
pub fn detectar_codificacao(bytes: &[u8], truncado: bool) -> Codificacao {
    if bytes.starts_with(&BOM_UTF8) {
        return Codificacao::Utf8ComBom;
    }
    match std::str::from_utf8(bytes) {
        Ok(_) => Codificacao::Utf8,
        Err(e) if truncado && e.error_len().is_none() => Codificacao::Utf8,
        Err(_) => Codificacao::Windows1252,
    }
}

/// Confere, bloco a bloco, se um arquivo todo é UTF-8 válido (um caractere pode ficar partido entre dois
/// blocos).
#[derive(Debug, Default)]
pub(crate) struct ValidadorUtf8 {
    cauda: [u8; 3],
    n: usize,
    invalido: bool,
}

fn tamanho_esperado(primeiro: u8) -> usize {
    match primeiro {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 1,
    }
}

impl ValidadorUtf8 {
    pub fn alimentar(&mut self, bloco: &[u8]) {
        if self.invalido || bloco.is_empty() {
            return;
        }
        let mut resto = bloco;
        if self.n > 0 {
            // completa o caractere que ficou partido no bloco anterior
            let falta = tamanho_esperado(self.cauda[0]).saturating_sub(self.n);
            let usa = falta.min(bloco.len());
            let mut junto = [0u8; 4];
            junto[..self.n].copy_from_slice(&self.cauda[..self.n]);
            junto[self.n..self.n + usa].copy_from_slice(&bloco[..usa]);
            let total = self.n + usa;
            match std::str::from_utf8(&junto[..total]) {
                Ok(_) => {
                    self.n = 0;
                    resto = &bloco[usa..];
                }
                Err(e) if e.error_len().is_none() => {
                    // ainda incompleto (bloco minúsculo): guarda o que já se tem
                    self.cauda[..total].copy_from_slice(&junto[..total]);
                    self.n = total;
                    return;
                }
                Err(_) => {
                    self.invalido = true;
                    return;
                }
            }
        }
        match std::str::from_utf8(resto) {
            Ok(_) => {}
            Err(e) if e.error_len().is_none() => {
                let cauda = &resto[e.valid_up_to()..];
                self.cauda[..cauda.len()].copy_from_slice(cauda);
                self.n = cauda.len();
            }
            Err(_) => self.invalido = true,
        }
    }

    /// O arquivo inteiro é UTF-8 válido (sem caractere cortado no fim).
    pub fn valido(&self) -> bool {
        !self.invalido && self.n == 0
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tabela_do_windows_1252() {
        assert_eq!(char_do_byte(0x41), 'A');
        assert_eq!(char_do_byte(0xE7), 'ç');
        assert_eq!(char_do_byte(0xE3), 'ã');
        assert_eq!(char_do_byte(0x80), '€');
        assert_eq!(char_do_byte(0x85), '…');
        assert_eq!(char_do_byte(0x93), '“');
        assert_eq!(char_do_byte(0x94), '”');
        assert_eq!(char_do_byte(0x96), '–');
        assert_eq!(char_do_byte(0x99), '™');
        assert_eq!(char_do_byte(0x9F), 'Ÿ');
        assert_eq!(char_do_byte(0xA0), '\u{A0}');
        assert_eq!(char_do_byte(0xFF), 'ÿ');
        assert_eq!(char_do_byte(0x81), '\u{81}');
    }

    #[test]
    fn todo_byte_vai_e_volta() {
        for b in 0..=255u8 {
            let c = char_do_byte(b);
            assert_eq!(byte_do_char(c), Some(b), "byte {b:#04x} ({c:?})");
        }
    }

    #[test]
    fn caracteres_fora_do_windows_1252() {
        assert_eq!(byte_do_char('日'), None);
        assert_eq!(byte_do_char('Ā'), None);
        assert_eq!(byte_do_char('\u{82}'), None, "C1 que o Windows-1252 troca por outro caractere");
        let e = Codificacao::Windows1252.codificar("ação 日本");
        assert_eq!(e, Err('日'));
        assert_eq!(
            Codificacao::Windows1252.codificar("ação €").unwrap().as_ref(),
            &[0x61, 0xE7, 0xE3, 0x6F, 0x20, 0x80]
        );
    }

    #[test]
    fn decodificar_cada_uma() {
        assert_eq!(Codificacao::Utf8.decodificar("ação".as_bytes()), "ação");
        assert_eq!(Codificacao::Utf8ComBom.decodificar("ação".as_bytes()), "ação");
        assert_eq!(Codificacao::Windows1252.decodificar(&[0x61, 0xE7, 0xE3, 0x6F]), "ação");
        assert_eq!(Codificacao::Utf8.decodificar(&[0x61, 0xFF]), "a\u{FFFD}");
        assert!(matches!(Codificacao::Windows1252.decodificar(b"abc"), Cow::Borrowed(_)));
    }

    #[test]
    fn rotulos_e_relacoes() {
        assert_eq!(Codificacao::Utf8ComBom.rotulo(), "UTF-8 com BOM");
        assert!(Codificacao::Utf8ComBom.tem_bom() && !Codificacao::Utf8.tem_bom());
        assert!(Codificacao::Utf8.mesma_tabela(Codificacao::Utf8ComBom));
        assert!(!Codificacao::Utf8.mesma_tabela(Codificacao::Windows1252));
    }

    #[test]
    fn deteccao_de_trecho() {
        assert_eq!(detectar_codificacao(b"\xEF\xBB\xBFa;b", false), Codificacao::Utf8ComBom);
        assert_eq!(detectar_codificacao("a;ação".as_bytes(), false), Codificacao::Utf8);
        assert_eq!(detectar_codificacao(b"a;b", false), Codificacao::Utf8);
        assert_eq!(detectar_codificacao(b"a;a\xE7\xE3o", false), Codificacao::Windows1252);
        // caractere cortado no fim: só perdoado quando o trecho é parte de um arquivo maior
        let cortado = &"ação".as_bytes()[..4];
        assert_eq!(detectar_codificacao(cortado, true), Codificacao::Utf8);
        assert_eq!(detectar_codificacao(cortado, false), Codificacao::Windows1252);
        assert_eq!(detectar_codificacao(b"", false), Codificacao::Utf8);
    }

    #[test]
    fn validador_com_caractere_partido_entre_blocos() {
        let texto = "ação €uro 😀 fim".as_bytes();
        for corte in 0..=texto.len() {
            let mut v = ValidadorUtf8::default();
            v.alimentar(&texto[..corte]);
            v.alimentar(&texto[corte..]);
            assert!(v.valido(), "corte em {corte}");
        }
        // um byte de cada vez
        let mut v = ValidadorUtf8::default();
        for b in texto {
            v.alimentar(&[*b]);
        }
        assert!(v.valido());
    }

    #[test]
    fn validador_acha_o_invalido_e_o_truncado() {
        let mut v = ValidadorUtf8::default();
        v.alimentar(b"abc\xE7\xE3o");
        assert!(!v.valido());
        let mut v = ValidadorUtf8::default();
        v.alimentar(b"abc");
        v.alimentar(&"ç".as_bytes()[..1]);
        assert!(!v.valido(), "caractere cortado no fim do arquivo");
        let mut v = ValidadorUtf8::default();
        v.alimentar(&"ç".as_bytes()[..1]);
        v.alimentar(b"a");
        assert!(!v.valido(), "continuação errada entre blocos");
        let mut v = ValidadorUtf8::default();
        v.alimentar(&"😀".as_bytes()[..2]);
        v.alimentar(&"😀".as_bytes()[2..3]);
        assert!(!v.valido(), "falta o último byte");
        v.alimentar(&"😀".as_bytes()[3..]);
        assert!(v.valido(), "o caractere fechou em três blocos");
        v.alimentar(b"\xFF");
        assert!(!v.valido());
        v.alimentar(b"abc");
        assert!(!v.valido(), "depois de inválido, fica inválido");
    }
}
