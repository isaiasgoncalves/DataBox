//! Números e tipos que se leem do texto de uma célula.
//!
//! Um CSV não declara tipo: o que a grade, o cabeçalho e o casamento de colunas sabem de uma célula vem daqui.
//! Tudo trabalha sobre o texto e sobre o caractere decimal do arquivo (`,` nos arquivos em português, `.` nos
//! demais); nada vai para ponto flutuante quando a conta é só de igualdade (o "Números pelo valor" do Trazer
//! colunas compara a forma canônica em texto, e um código de 25 algarismos não perde precisão).

use std::borrow::Cow;

/// O tipo que os valores de uma coluna sugerem (o "Tipo detectado" das Propriedades).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoDetectado {
    /// Nenhum valor (todas as células vazias).
    Vazio,
    Inteiro,
    /// Número com parte decimal ou em notação científica.
    Decimal,
    /// `2025-01-02`, `02/01/2025`, com ou sem hora.
    Data,
    /// `true` e `false` (e `verdadeiro` e `falso`), em qualquer caixa.
    Booleano,
    Texto,
}

impl TipoDetectado {
    /// Número, data ou booleano: o que um cabeçalho (texto) não é.
    pub fn e_tipado(self) -> bool {
        !matches!(self, TipoDetectado::Vazio | TipoDetectado::Texto)
    }
}

/// As partes de um número escrito: sinal, algarismos inteiros (sem os separadores de milhar), algarismos
/// decimais e expoente.
struct Partes {
    negativo: bool,
    inteiro: String,
    fracao: String,
    expoente: Option<i32>,
}

fn so_algarismos(texto: &str) -> bool {
    texto.bytes().all(|b| b.is_ascii_digit())
}

/// Lê `texto` (sem espaços nas pontas) como número com o `decimal` dado. O outro de `.` e `,` só vale como
/// separador de milhar, em grupos de três algarismos (`1.234.567,89`).
fn partes(texto: &str, decimal: char, aceita_expoente: bool) -> Option<Partes> {
    let milhar = if decimal == ',' { '.' } else { ',' };
    let mut resto = texto;
    let mut negativo = false;
    if let Some(r) = resto.strip_prefix('-') {
        negativo = true;
        resto = r;
    } else if let Some(r) = resto.strip_prefix('+') {
        resto = r;
    }
    let mut expoente = None;
    let mut mantissa = resto;
    if aceita_expoente && let Some(i) = resto.find(['e', 'E']) {
        let (m, e) = resto.split_at(i);
        let e = e.get(1..)?;
        let digitos = e.strip_prefix(['+', '-']).unwrap_or(e);
        if digitos.is_empty() || digitos.len() > 4 || !so_algarismos(digitos) {
            return None;
        }
        expoente = Some(e.parse::<i32>().ok()?);
        mantissa = m;
    }
    let (parte_inteira, fracao) = match mantissa.split_once(decimal) {
        Some((i, f)) => (i, f),
        None => (mantissa, ""),
    };
    if mantissa.contains(decimal) && (fracao.is_empty() || !so_algarismos(fracao)) {
        return None;
    }
    let inteiro = if parte_inteira.contains(milhar) {
        let mut grupos = parte_inteira.split(milhar);
        let primeiro = grupos.next()?;
        if primeiro.is_empty() || primeiro.len() > 3 || !so_algarismos(primeiro) {
            return None;
        }
        let mut inteiro = String::from(primeiro);
        for g in grupos {
            if g.len() != 3 || !so_algarismos(g) {
                return None;
            }
            inteiro.push_str(g);
        }
        inteiro
    } else {
        if !so_algarismos(parte_inteira) {
            return None;
        }
        parte_inteira.to_string()
    };
    if inteiro.is_empty() && fracao.is_empty() {
        return None;
    }
    Some(Partes { negativo, inteiro, fracao: fracao.to_string(), expoente })
}

/// Lê o texto como número, com o `decimal` do arquivo. Espaços nas pontas não contam; aceita sinal, milhar
/// (`1.234,5` com decimal `,`) e notação científica (`1,5E3`).
pub fn analisar_numero(texto: &str, decimal: char) -> Option<f64> {
    let p = partes(texto.trim(), decimal, true)?;
    let mut s = String::with_capacity(p.inteiro.len() + p.fracao.len() + 8);
    if p.negativo {
        s.push('-');
    }
    s.push_str(if p.inteiro.is_empty() { "0" } else { &p.inteiro });
    if !p.fracao.is_empty() {
        s.push('.');
        s.push_str(&p.fracao);
    }
    if let Some(e) = p.expoente {
        s.push('e');
        s.push_str(&e.to_string());
    }
    s.parse().ok()
}

/// A forma canônica de um número escrito, para comparar "007", "7", "7,0" e "+7" pelo valor: sem sinal `+`,
/// sem zeros à esquerda nem à direita da parte decimal, com `.` decimal e sem milhar. `None` quando o texto não
/// é só um número (letras, expoente, vazio). O texto não diz qual é o decimal: com os dois separadores, o
/// último é o decimal; com um só e uma vez, ele é o decimal (`1.234` vale 1,234); com um só repetido, é milhar
/// (`1.234.567`).
pub fn canonico(texto: &str) -> Option<Cow<'_, str>> {
    // caminho rápido, o dos códigos: só algarismos (e um sinal), sem copiar o texto
    let (negativo, corpo) = match texto.as_bytes().first() {
        Some(b'-') => (true, texto.get(1..).unwrap_or_default()),
        Some(b'+') => (false, texto.get(1..).unwrap_or_default()),
        _ => (false, texto),
    };
    if !corpo.is_empty() && so_algarismos(corpo) {
        let sem_zeros = corpo.trim_start_matches('0');
        return Some(match (sem_zeros.is_empty(), negativo) {
            (true, _) => Cow::Borrowed("0"),
            (false, true) => Cow::Owned(format!("-{sem_zeros}")),
            (false, false) => Cow::Borrowed(sem_zeros),
        });
    }
    canonico_com_decimal(texto).map(Cow::Owned)
}

/// O caminho geral de [`canonico`]: com parte decimal ou separador de milhar.
fn canonico_com_decimal(texto: &str) -> Option<String> {
    let corpo = texto.strip_prefix(['+', '-']).unwrap_or(texto);
    if corpo.is_empty() || !corpo.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b',') {
        return None;
    }
    let pontos = corpo.matches('.').count();
    let virgulas = corpo.matches(',').count();
    let decimal = match (pontos, virgulas) {
        (0 | 1, 0) => '.',
        (_, 0) => ',',
        (0, 1) => ',',
        (0, _) => '.',
        _ => {
            if corpo.rfind('.') > corpo.rfind(',') {
                '.'
            } else {
                ','
            }
        }
    };
    let p = partes(texto, decimal, false)?;
    let inteiro = p.inteiro.trim_start_matches('0');
    let inteiro = if inteiro.is_empty() { "0" } else { inteiro };
    let fracao = p.fracao.trim_end_matches('0');
    let zero = inteiro == "0" && fracao.is_empty();
    let mut saida = String::with_capacity(inteiro.len() + fracao.len() + 2);
    if p.negativo && !zero {
        saida.push('-');
    }
    saida.push_str(inteiro);
    if !fracao.is_empty() {
        saida.push('.');
        saida.push_str(fracao);
    }
    Some(saida)
}

fn dias_no_mes(ano: u32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (ano.is_multiple_of(4) && !ano.is_multiple_of(100)) || ano.is_multiple_of(400) => 29,
        2 => 28,
        _ => 0,
    }
}

fn numero_de(texto: &str) -> Option<u32> {
    if texto.is_empty() || !so_algarismos(texto) { None } else { texto.parse().ok() }
}

fn data_valida(ano: u32, mes: u32, dia: u32) -> bool {
    (1..=12).contains(&mes) && dia >= 1 && dia <= dias_no_mes(ano, mes)
}

/// `HH:MM`, `HH:MM:SS` ou `HH:MM:SS.fff`, com um `Z` opcional no fim.
fn hora_valida(texto: &str) -> bool {
    let texto = texto.strip_suffix('Z').unwrap_or(texto);
    let (sem_fracao, fracao) = match texto.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (texto, None),
    };
    if fracao.is_some_and(|f| f.is_empty() || !so_algarismos(f)) {
        return false;
    }
    let partes: Vec<&str> = sem_fracao.split(':').collect();
    if !(2..=3).contains(&partes.len()) || (fracao.is_some() && partes.len() != 3) {
        return false;
    }
    let limites = [23, 59, 59];
    partes.iter().zip(limites).all(|(p, max)| p.len() == 2 && numero_de(p).is_some_and(|n| n <= max))
}

/// Datas em `AAAA-MM-DD` ou `DD/MM/AAAA`, com hora opcional depois (separada por espaço ou `T`).
pub fn e_data(texto: &str) -> bool {
    let t = texto.trim();
    let Some(data) = t.get(..10) else {
        return false;
    };
    let resto = t.get(10..).unwrap_or("");
    let b = data.as_bytes();
    let valida = if b.get(4) == Some(&b'-') && b.get(7) == Some(&b'-') {
        match (numero_de(&data[..4]), numero_de(&data[5..7]), numero_de(&data[8..10])) {
            (Some(a), Some(m), Some(d)) => data_valida(a, m, d),
            _ => false,
        }
    } else if b.get(2) == Some(&b'/') && b.get(5) == Some(&b'/') {
        match (numero_de(&data[..2]), numero_de(&data[3..5]), numero_de(&data[6..10])) {
            (Some(d), Some(m), Some(a)) => data_valida(a, m, d),
            _ => false,
        }
    } else {
        false
    };
    if !valida {
        return false;
    }
    match resto.chars().next() {
        None => true,
        Some(' ' | 'T') => hora_valida(resto.get(1..).unwrap_or("")),
        Some(_) => false,
    }
}

fn e_booleano(texto: &str) -> bool {
    ["true", "false", "verdadeiro", "falso"].iter().any(|b| texto.eq_ignore_ascii_case(b))
}

/// O tipo de um valor. Número com zero à esquerda (`004718`) é texto: é um código, e os zeros valem.
pub fn classificar(texto: &str, decimal: char) -> TipoDetectado {
    let t = texto.trim();
    if t.is_empty() {
        return TipoDetectado::Vazio;
    }
    if e_booleano(t) {
        return TipoDetectado::Booleano;
    }
    if e_data(t) {
        return TipoDetectado::Data;
    }
    match partes(t, decimal, true) {
        Some(p) => {
            if p.inteiro.len() > 1 && p.inteiro.starts_with('0') {
                TipoDetectado::Texto
            } else if p.fracao.is_empty() && p.expoente.is_none() {
                TipoDetectado::Inteiro
            } else {
                TipoDetectado::Decimal
            }
        }
        None => TipoDetectado::Texto,
    }
}

/// O tipo de uma coluna: o de todos os valores não vazios quando concordam (inteiros com decimais dão
/// decimal); qualquer discordância é texto; sem valores, vazio.
pub fn tipo_da_coluna<'a>(valores: impl IntoIterator<Item = &'a str>, decimal: char) -> TipoDetectado {
    let mut tipo = TipoDetectado::Vazio;
    for v in valores {
        let t = classificar(v, decimal);
        tipo = match (tipo, t) {
            (a, TipoDetectado::Vazio) => a,
            (TipoDetectado::Vazio, b) => b,
            (a, b) if a == b => a,
            (TipoDetectado::Inteiro, TipoDetectado::Decimal) | (TipoDetectado::Decimal, TipoDetectado::Inteiro) => {
                TipoDetectado::Decimal
            }
            _ => return TipoDetectado::Texto,
        };
    }
    tipo
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numeros_com_decimal_virgula_e_milhar() {
        assert_eq!(analisar_numero("12", ','), Some(12.0));
        assert_eq!(analisar_numero("12,5", ','), Some(12.5));
        assert_eq!(analisar_numero("-1.234,5", ','), Some(-1234.5));
        assert_eq!(analisar_numero("1.234.567", ','), Some(1_234_567.0));
        assert_eq!(analisar_numero("  7  ", ','), Some(7.0));
        assert_eq!(analisar_numero(",5", ','), Some(0.5));
        assert_eq!(analisar_numero("1,5E3", ','), Some(1500.0));
        assert_eq!(analisar_numero("1.5", ','), None, "ponto com decimal vírgula só vale como milhar");
        assert_eq!(analisar_numero("12.34.5", ','), None);
        assert_eq!(analisar_numero("1.23", ','), None);
        assert_eq!(analisar_numero("5,", ','), None);
        assert_eq!(analisar_numero("abc", ','), None);
        assert_eq!(analisar_numero("", ','), None);
        assert_eq!(analisar_numero("-", ','), None);
        assert_eq!(analisar_numero("1,2,3", ','), None);
    }

    #[test]
    fn numeros_com_decimal_ponto() {
        assert_eq!(analisar_numero("12.5", '.'), Some(12.5));
        assert_eq!(analisar_numero("1,234.5", '.'), Some(1234.5));
        assert_eq!(analisar_numero("1e-3", '.'), Some(0.001));
        assert_eq!(analisar_numero("+3", '.'), Some(3.0));
        assert_eq!(analisar_numero("1,5", '.'), None);
    }

    #[test]
    fn forma_canonica() {
        assert_eq!(canonico("007").as_deref(), Some("7"));
        assert_eq!(canonico("7").as_deref(), Some("7"));
        assert_eq!(canonico("+7").as_deref(), Some("7"));
        assert_eq!(canonico("7,0").as_deref(), Some("7"));
        assert_eq!(canonico("7.00").as_deref(), Some("7"));
        assert_eq!(canonico("0007,50").as_deref(), Some("7.5"));
        assert_eq!(canonico("-0").as_deref(), Some("0"));
        assert_eq!(canonico("-0,0").as_deref(), Some("0"));
        assert_eq!(canonico("-12,5").as_deref(), Some("-12.5"));
        assert_eq!(canonico(",5").as_deref(), Some("0.5"));
        assert_eq!(canonico("1.234,56").as_deref(), Some("1234.56"));
        assert_eq!(canonico("1,234.56").as_deref(), Some("1234.56"));
        assert_eq!(canonico("1.234.567").as_deref(), Some("1234567"));
        assert_eq!(canonico("1,234,567").as_deref(), Some("1234567"));
        assert_eq!(canonico("12345678901234567890123").as_deref(), Some("12345678901234567890123"));
        assert_eq!(canonico("abc"), None);
        assert_eq!(canonico(""), None);
        assert_eq!(canonico("-"), None);
        assert_eq!(canonico("1e5"), None);
        assert_eq!(canonico("1.2.3,4,5"), None);
        assert_eq!(canonico("12 3"), None);
        assert_eq!(canonico("7-"), None);
    }

    #[test]
    fn datas() {
        for ok in [
            "2025-01-02",
            "02/01/2025",
            "2024-02-29",
            "2025-01-02 13:45",
            "2025-01-02T13:45:59",
            "2025-01-02T13:45:59.123Z",
        ] {
            assert!(e_data(ok), "{ok}");
        }
        for ruim in [
            "2025-13-02",
            "2025-02-29",
            "31/04/2025",
            "2025-01-02 25:00",
            "2025/01/02",
            "2025-01-02x",
            "12",
            "",
            "2025-01-0",
        ] {
            assert!(!e_data(ruim), "{ruim}");
        }
        assert!(e_data("1900-02-28") && !e_data("1900-02-29") && e_data("2000-02-29"));
    }

    #[test]
    fn classificacao() {
        assert_eq!(classificar("", ','), TipoDetectado::Vazio);
        assert_eq!(classificar("   ", ','), TipoDetectado::Vazio);
        assert_eq!(classificar("12", ','), TipoDetectado::Inteiro);
        assert_eq!(classificar("0", ','), TipoDetectado::Inteiro);
        assert_eq!(classificar("12,5", ','), TipoDetectado::Decimal);
        assert_eq!(classificar("12,5", '.'), TipoDetectado::Texto);
        assert_eq!(classificar("004718", ','), TipoDetectado::Texto, "zeros à esquerda: código");
        assert_eq!(classificar("0,5", ','), TipoDetectado::Decimal);
        assert_eq!(classificar("2025-01-02", ','), TipoDetectado::Data);
        assert_eq!(classificar("TRUE", ','), TipoDetectado::Booleano);
        assert_eq!(classificar("Falso", ','), TipoDetectado::Booleano);
        assert_eq!(classificar("Sul", ','), TipoDetectado::Texto);
        assert_eq!(classificar("1e5", '.'), TipoDetectado::Decimal);
    }

    #[test]
    fn tipo_da_coluna_por_concordancia() {
        assert_eq!(tipo_da_coluna(["1", "2", ""], ','), TipoDetectado::Inteiro);
        assert_eq!(tipo_da_coluna(["1", "2,5"], ','), TipoDetectado::Decimal);
        assert_eq!(tipo_da_coluna(["1", "a"], ','), TipoDetectado::Texto);
        assert_eq!(tipo_da_coluna(["", " "], ','), TipoDetectado::Vazio);
        assert_eq!(tipo_da_coluna(["2025-01-01", "2025-01-02"], ','), TipoDetectado::Data);
        assert_eq!(tipo_da_coluna(["2025-01-01", "5"], ','), TipoDetectado::Texto);
        assert!(TipoDetectado::Data.e_tipado() && !TipoDetectado::Texto.e_tipado() && !TipoDetectado::Vazio.e_tipado());
    }
}
