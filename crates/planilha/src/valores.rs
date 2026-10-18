//! Os valores que saem da planilha: a célula, a linha, a página e os endereços ("C12", "A1:C3").

use std::fmt;

/// Data de calendário (proleptic gregoriano). Pode ser `1900-01-00` (série 0) ou `1900-02-29` (série 60,
/// que não existiu: é o defeito de compatibilidade com o Lotus 1-2-3, que o Excel mantém).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Data {
    pub ano: i32,
    pub mes: u8,
    pub dia: u8,
}

/// Hora do dia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hora {
    pub hora: u8,
    pub minuto: u8,
    pub segundo: u8,
    pub milissegundo: u16,
}

/// Data e hora juntas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Momento {
    pub data: Data,
    pub hora: Hora,
}

impl fmt::Display for Data {
    /// ISO: `2024-03-15`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.ano, self.mes, self.dia)
    }
}

impl fmt::Display for Hora {
    /// `14:30:00`, com `.mmm` só quando há milissegundos.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hora, self.minuto, self.segundo)?;
        if self.milissegundo != 0 {
            write!(f, ".{:03}", self.milissegundo)?;
        }
        Ok(())
    }
}

impl fmt::Display for Momento {
    /// `2024-03-15 14:30:00`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.data, self.hora)
    }
}

/// O conteúdo de uma célula, já tipado.
///
/// Datas e horas vêm como [`Valor::Data`], [`Valor::Hora`] e [`Valor::DataHora`] quando o formato de número
/// do estilo da célula é de data/hora (e a série cabe no calendário); nos demais casos o número fica
/// [`Valor::Numero`]. Fórmula entra com o último valor calculado que o arquivo guardou.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Valor {
    #[default]
    Vazio,
    Texto(String),
    Numero(f64),
    Booleano(bool),
    /// Erro do Excel, no texto do arquivo: `#DIV/0!`, `#N/A`, `#REF!`…
    Erro(String),
    Data(Data),
    Hora(Hora),
    DataHora(Momento),
}

impl Valor {
    pub fn vazio(&self) -> bool {
        matches!(self, Valor::Vazio)
    }

    /// O texto, se for texto.
    pub fn como_texto(&self) -> Option<&str> {
        match self {
            Valor::Texto(t) => Some(t),
            _ => None,
        }
    }

    /// O número, se for número (data e hora não contam: use o tipo delas).
    pub fn como_numero(&self) -> Option<f64> {
        match self {
            Valor::Numero(n) => Some(*n),
            _ => None,
        }
    }
}

/// Uma célula que existe no XML: a coluna (a partir de 0), o valor, o índice de estilo (`s=`) e a fórmula.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Celula {
    pub coluna: u32,
    pub valor: Valor,
    /// Índice em `cellXfs`; leva ao formato por [`crate::Pasta::formato`].
    pub estilo: u32,
    /// Texto da fórmula (sem o `=`), quando a célula tem uma e ela está escrita nela (fórmula compartilhada
    /// só leva o texto na primeira célula).
    pub formula: Option<String>,
}

/// Uma linha (`<row>`). Só as células que existem no arquivo, em ordem de coluna; as que faltam são vazias.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Linha {
    /// Número da linha como no Excel (a partir de 1).
    pub numero: u32,
    pub oculta: bool,
    /// Altura em pontos, se o arquivo define.
    pub altura: Option<f32>,
    pub celulas: Vec<Celula>,
}

impl Linha {
    pub fn vazia(numero: u32) -> Linha {
        Linha { numero, ..Linha::default() }
    }

    /// A célula da coluna `coluna` (a partir de 0), se existir no arquivo.
    pub fn celula(&self, coluna: u32) -> Option<&Celula> {
        self.celulas.binary_search_by_key(&coluna, |c| c.coluna).ok().map(|i| &self.celulas[i])
    }

    /// O valor da coluna: vazio onde não há célula.
    pub fn valor(&self, coluna: u32) -> &Valor {
        const VAZIO: Valor = Valor::Vazio;
        self.celula(coluna).map_or(&VAZIO, |c| &c.valor)
    }

    /// Os valores das `n` primeiras colunas (as que faltam, vazias).
    pub fn valores(&self, n: u32) -> Vec<Valor> {
        (0..n).map(|c| self.valor(c).clone()).collect()
    }

    /// Não tem nenhuma célula com valor.
    pub fn sem_valores(&self) -> bool {
        self.celulas.iter().all(|c| c.valor.vazio())
    }

    /// Coluna da última célula escrita (a partir de 0).
    pub fn ultima_coluna(&self) -> Option<u32> {
        self.celulas.last().map(|c| c.coluna)
    }
}

/// Um trecho contínuo de linhas, sem buracos: as que não existem no arquivo vêm como [`Linha::vazia`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pagina {
    /// Número (a partir de 1) da primeira linha.
    pub primeira_linha: u32,
    pub linhas: Vec<Linha>,
}

/// Um intervalo de células, com linhas a partir de 1 e colunas a partir de 0, extremos incluídos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Intervalo {
    pub linha_inicial: u32,
    pub coluna_inicial: u32,
    pub linha_final: u32,
    pub coluna_final: u32,
}

impl Intervalo {
    /// `"A1:C3"`, `"B2"` (uma célula) ou com cifrões (`$A$1:$C$3`). Os extremos podem vir invertidos.
    pub fn de_texto(texto: &str) -> Option<Intervalo> {
        let texto = texto.trim();
        let (a, b) = match texto.split_once(':') {
            Some((a, b)) => (a, b),
            None => (texto, texto),
        };
        let (l1, c1) = referencia(a)?;
        let (l2, c2) = referencia(b)?;
        Some(Intervalo {
            linha_inicial: l1.min(l2),
            coluna_inicial: c1.min(c2),
            linha_final: l1.max(l2),
            coluna_final: c1.max(c2),
        })
    }

    pub fn contem(&self, linha: u32, coluna: u32) -> bool {
        (self.linha_inicial..=self.linha_final).contains(&linha)
            && (self.coluna_inicial..=self.coluna_final).contains(&coluna)
    }

    /// Cruza as linhas `de..=ate`.
    pub fn toca_linhas(&self, de: u32, ate: u32) -> bool {
        self.linha_inicial <= ate && self.linha_final >= de
    }

    pub fn linhas(&self) -> u32 {
        self.linha_final - self.linha_inicial + 1
    }

    pub fn colunas(&self) -> u32 {
        self.coluna_final - self.coluna_inicial + 1
    }
}

impl fmt::Display for Intervalo {
    /// `A1:C3` (ou `B2`, de uma célula só).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let a = referencia_a1(self.linha_inicial, self.coluna_inicial);
        if self.linha_inicial == self.linha_final && self.coluna_inicial == self.coluna_final {
            write!(f, "{a}")
        } else {
            write!(f, "{a}:{}", referencia_a1(self.linha_final, self.coluna_final))
        }
    }
}

/// Maior coluna do Excel (XFD), a partir de 0.
pub const ULTIMA_COLUNA: u32 = 16_383;

/// Letras da coluna (a partir de 0): 0 → "A", 25 → "Z", 26 → "AA", 16383 → "XFD".
pub fn nome_da_coluna(coluna: u32) -> String {
    let mut n = coluna + 1;
    let mut letras = Vec::new();
    while n > 0 {
        let resto = (n - 1) % 26;
        letras.push(char::from(b'A' + resto as u8));
        n = (n - 1) / 26;
    }
    letras.iter().rev().collect()
}

/// "C12" a partir da linha (1…) e da coluna (0…).
pub fn referencia_a1(linha: u32, coluna: u32) -> String {
    format!("{}{linha}", nome_da_coluna(coluna))
}

/// Índice da coluna (a partir de 0) pelas letras: "A" → 0, "aa" → 26. Vazio ou além de XFD, `None`.
pub fn indice_da_coluna(letras: &str) -> Option<u32> {
    if letras.is_empty() || letras.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for c in letras.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a',
            _ => return None,
        };
        n = n * 26 + u32::from(v) + 1;
    }
    (n <= ULTIMA_COLUNA + 1).then(|| n - 1)
}

/// "C12" → (linha 12, coluna 2). Aceita cifrões ("$C$12") e minúsculas.
pub fn referencia(texto: &str) -> Option<(u32, u32)> {
    let sem = texto.trim().as_bytes();
    let mut i = 0;
    if sem.get(i) == Some(&b'$') {
        i += 1;
    }
    let ini = i;
    while sem.get(i).is_some_and(u8::is_ascii_alphabetic) {
        i += 1;
    }
    let coluna = indice_da_coluna(std::str::from_utf8(&sem[ini..i]).ok()?)?;
    if sem.get(i) == Some(&b'$') {
        i += 1;
    }
    let digitos = &sem[i..];
    if digitos.is_empty() || digitos.len() > 9 || !digitos.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let linha: u32 = std::str::from_utf8(digitos).ok()?.parse().ok()?;
    (linha >= 1).then_some((linha, coluna))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn letras_de_coluna() {
        for (n, s) in [(0, "A"), (25, "Z"), (26, "AA"), (27, "AB"), (51, "AZ"), (52, "BA"), (701, "ZZ"), (702, "AAA")] {
            assert_eq!(nome_da_coluna(n), s);
            assert_eq!(indice_da_coluna(s), Some(n));
        }
        assert_eq!(nome_da_coluna(ULTIMA_COLUNA), "XFD");
        assert_eq!(indice_da_coluna("XFD"), Some(ULTIMA_COLUNA));
        assert_eq!(indice_da_coluna("XFE"), None);
        assert_eq!(indice_da_coluna("AAAA"), None);
        assert_eq!(indice_da_coluna(""), None);
        assert_eq!(indice_da_coluna("a1"), None);
        assert_eq!(indice_da_coluna("b"), Some(1));
    }

    #[test]
    fn referencias() {
        assert_eq!(referencia("A1"), Some((1, 0)));
        assert_eq!(referencia("C12"), Some((12, 2)));
        assert_eq!(referencia("$C$12"), Some((12, 2)));
        assert_eq!(referencia("xfd1048576"), Some((1_048_576, ULTIMA_COLUNA)));
        assert_eq!(referencia("A0"), None);
        assert_eq!(referencia("12"), None);
        assert_eq!(referencia("A"), None);
        assert_eq!(referencia("A1B"), None);
        assert_eq!(referencia("A-1"), None);
        assert_eq!(referencia(""), None);
        assert_eq!(referencia_a1(12, 2), "C12");
    }

    #[test]
    fn intervalos() {
        let i = Intervalo::de_texto("B2:D5").unwrap();
        assert_eq!(i, Intervalo { linha_inicial: 2, coluna_inicial: 1, linha_final: 5, coluna_final: 3 });
        assert_eq!(i.to_string(), "B2:D5");
        assert_eq!((i.linhas(), i.colunas()), (4, 3));
        assert!(i.contem(2, 1) && i.contem(5, 3) && i.contem(3, 2));
        assert!(!i.contem(1, 1) && !i.contem(6, 1) && !i.contem(3, 4) && !i.contem(3, 0));
        assert!(i.toca_linhas(5, 9) && i.toca_linhas(1, 2) && !i.toca_linhas(6, 9) && !i.toca_linhas(1, 1));
        let um = Intervalo::de_texto("$C$3").unwrap();
        assert_eq!(um.to_string(), "C3");
        assert_eq!(Intervalo::de_texto("D5:B2"), Some(i));
        assert_eq!(Intervalo::de_texto("A1:"), None);
        assert_eq!(Intervalo::de_texto("lixo"), None);
    }

    #[test]
    fn linha_com_buracos() {
        let l = Linha {
            numero: 3,
            celulas: vec![
                Celula { coluna: 1, valor: Valor::Numero(2.0), ..Celula::default() },
                Celula { coluna: 4, valor: Valor::Texto("x".into()), ..Celula::default() },
            ],
            ..Linha::default()
        };
        assert_eq!(l.valor(0), &Valor::Vazio);
        assert_eq!(l.valor(1), &Valor::Numero(2.0));
        assert_eq!(l.valor(2), &Valor::Vazio);
        assert_eq!(l.valor(4).como_texto(), Some("x"));
        assert_eq!(
            l.valores(6),
            [Valor::Vazio, Valor::Numero(2.0), Valor::Vazio, Valor::Vazio, Valor::Texto("x".into()), Valor::Vazio]
        );
        assert_eq!(l.ultima_coluna(), Some(4));
        assert!(!l.sem_valores());
        assert!(Linha::vazia(9).sem_valores());
    }

    #[test]
    fn exibicao_iso() {
        let d = Data { ano: 2024, mes: 3, dia: 5 };
        let h = Hora { hora: 14, minuto: 30, segundo: 0, milissegundo: 0 };
        assert_eq!(d.to_string(), "2024-03-05");
        assert_eq!(h.to_string(), "14:30:00");
        assert_eq!(Hora { milissegundo: 250, ..h }.to_string(), "14:30:00.250");
        assert_eq!(Momento { data: d, hora: h }.to_string(), "2024-03-05 14:30:00");
    }
}
