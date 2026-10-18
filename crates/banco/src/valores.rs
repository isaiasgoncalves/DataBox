//! Coerção das entradas de gravação pela afinidade da coluna (ARQUITETURA.md, §4.3) e as regras
//! numéricas (vírgula decimal, milhar pt-BR, número canônico em coluna sem tipo).

use std::fs;

use comum::{Afinidade, Entrada, Resultado, Valor, formatar_real, traduzir_io};
use rusqlite::types::ValueRef;

/// O que uma entrada vira: um valor para o comando, ou a coluna fica fora dele.
#[derive(Debug, Clone, PartialEq)]
pub enum Coagido {
    /// Valor pronto para ir como parâmetro.
    Valor(Valor),
    /// Coluna fora do INSERT (vale o DEFAULT); num UPDATE é erro 400.
    Padrao,
    /// Coluna fora do UPDATE.
    Manter,
}

/// Espaço para aparar e para o `\s` (o `char::is_whitespace` mais os separadores \x1c–\x1f).
pub(crate) fn e_espaco(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

pub(crate) fn strip(s: &str) -> &str {
    s.trim_matches(e_espaco)
}

fn digitos(b: &[u8], desde: usize) -> usize {
    b.iter().skip(desde).take_while(|c| c.is_ascii_digit()).count()
}

fn sinal(b: &[u8], desde: usize) -> usize {
    usize::from(matches!(b.get(desde), Some(b'+' | b'-')))
}

/// `^[+-]?[0-9]+$`
fn e_inteiro(s: &str) -> bool {
    let b = s.as_bytes();
    let i = sinal(b, 0);
    let d = digitos(b, i);
    d > 0 && i + d == b.len()
}

/// `^-?[0-9]+$`
fn e_inteiro_canonico(s: &str) -> bool {
    let b = s.as_bytes();
    let i = usize::from(b.first() == Some(&b'-'));
    let d = digitos(b, i);
    d > 0 && i + d == b.len()
}

/// `^[+-]?(?:[0-9]+\.[0-9]*|\.[0-9]+|[0-9]+)(?:[eE][+-]?[0-9]+)?$`
fn e_decimal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = sinal(b, 0);
    let antes = digitos(b, i);
    i += antes;
    let mut mantissa = antes > 0;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let depois = digitos(b, i);
        i += depois;
        mantissa = antes > 0 || depois > 0;
    }
    if !mantissa {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        i += sinal(b, i);
        let expoente = digitos(b, i);
        if expoente == 0 {
            return false;
        }
        i += expoente;
    }
    i == b.len()
}

/// `^[+-]?[0-9]+,[0-9]+$`
fn e_decimal_virgula(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = sinal(b, 0);
    let antes = digitos(b, i);
    i += antes;
    if antes == 0 || b.get(i) != Some(&b',') {
        return false;
    }
    i += 1;
    let depois = digitos(b, i);
    depois > 0 && i + depois == b.len()
}

/// `^[+-]?[0-9]{1,3}(?:\.[0-9]{3})+,[0-9]+$` (1.234,56 em pt-BR)
fn e_milhar_virgula(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = sinal(b, 0);
    let primeiro = digitos(b, i);
    if !(1..=3).contains(&primeiro) {
        return false;
    }
    i += primeiro;
    let mut grupos = 0;
    while b.get(i) == Some(&b'.') && digitos(b, i + 1) == 3 {
        i += 4;
        grupos += 1;
    }
    if grupos == 0 || b.get(i) != Some(&b',') {
        return false;
    }
    i += 1;
    let decimais = digitos(b, i);
    decimais > 0 && i + decimais == b.len()
}

fn real(s: &str) -> Option<Valor> {
    s.parse::<f64>().ok().map(Valor::Real)
}

/// Detecção numérica de um texto para INTEGER/REAL/NUMERIC; devolve o texto original se não for número.
pub(crate) fn numero_de_texto(texto: &str, afinidade: Afinidade) -> Valor {
    let s = strip(texto);
    if s.is_empty() {
        return Valor::Nulo;
    }
    if matches!(afinidade, Afinidade::Integer | Afinidade::Numeric) && e_inteiro(s) {
        // fora dos 64 bits vai o texto, e o SQLite aplica a afinidade
        return s.parse::<i64>().map_or_else(|_| Valor::Texto(texto.to_string()), Valor::Inteiro);
    }
    let convertido = if e_decimal(s) {
        real(s)
    } else if e_decimal_virgula(s) {
        real(&s.replace(',', "."))
    } else if e_milhar_virgula(s) {
        real(&s.replace('.', "").replace(',', "."))
    } else {
        None
    };
    convertido.unwrap_or_else(|| Valor::Texto(texto.to_string()))
}

/// Coluna sem tipo (afinidade BLOB, ou ANY de tabela STRICT): vira número só o que é a forma exata dele.
///
/// "6" vira 6 e "2.5" vira 2.5; "007", "1.50", " 6" e "1e3" continuam texto, como digitados: sem tipo
/// declarado não há regra que diga que zeros à esquerda ou casas decimais podem ser jogados fora.
pub(crate) fn numero_canonico(texto: &str) -> Valor {
    if e_inteiro_canonico(texto) {
        return match texto.parse::<i64>() {
            Ok(n) if n.to_string() == texto => Valor::Inteiro(n),
            _ => Valor::Texto(texto.to_string()),
        };
    }
    if e_decimal(texto)
        && let Ok(f) = texto.parse::<f64>()
        && f.is_finite()
        && formatar_real(f) == texto
    {
        return Valor::Real(f);
    }
    Valor::Texto(texto.to_string())
}

/// Entrada de gravação (§3.3) → valor pronto para parâmetro, ou a marca de coluna omitida.
///
/// Texto: INTEGER → inteiro se `^[+-]?\d+$`, real se número decimal (com ponto, vírgula decimal ou milhar
/// pt-BR); REAL → real se número; NUMERIC → inteiro, senão real; TEXT → como veio; BLOB → número só na forma
/// exata dele. Vazio (ou só espaços) em coluna numérica → NULL. [`Entrada::Arquivo`] lê o arquivo como BLOB.
pub fn coagir(entrada: &Entrada, afinidade: Afinidade) -> Resultado<Coagido> {
    let valor = match entrada {
        Entrada::Nulo => Valor::Nulo,
        Entrada::Inteiro(i) => Valor::Inteiro(*i),
        Entrada::Real(r) => Valor::Real(*r),
        Entrada::Texto(s) => match afinidade {
            Afinidade::Integer | Afinidade::Real | Afinidade::Numeric => numero_de_texto(s, afinidade),
            Afinidade::Blob => numero_canonico(s),
            Afinidade::Text => Valor::Texto(s.clone()),
        },
        Entrada::TextoLiteral(s) => Valor::Texto(s.clone()),
        Entrada::Padrao => return Ok(Coagido::Padrao),
        Entrada::Manter => return Ok(Coagido::Manter),
        Entrada::Arquivo(caminho) => Valor::Blob(
            fs::read(caminho)
                .map_err(|e| traduzir_io(&e, Some(caminho), Some("Não foi possível ler o arquivo escolhido.")))?,
        ),
        Entrada::Blob(b) => Valor::Blob(b.clone()),
    };
    Ok(Coagido::Valor(valor))
}

/// Texto de um valor lido de uma tabela do sistema (nome, tipo, SQL): UTF-8 inválido vira "�", NULL vira "".
pub(crate) fn texto_de(v: ValueRef<'_>) -> String {
    texto_opcional(v).unwrap_or_default()
}

pub(crate) fn texto_opcional(v: ValueRef<'_>) -> Option<String> {
    match v {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(r) => Some(formatar_real(r)),
        ValueRef::Text(b) | ValueRef::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
    }
}

/// Inteiro de um valor lido de um PRAGMA (NULL → 0).
pub(crate) fn inteiro_de(v: ValueRef<'_>) -> i64 {
    match v {
        ValueRef::Integer(i) => i,
        ValueRef::Real(r) => r as i64,
        ValueRef::Text(b) => std::str::from_utf8(b).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        ValueRef::Null | ValueRef::Blob(_) => 0,
    }
}

/// Contagem (sempre ≥ 0) de um `count(*)`.
pub(crate) fn contagem_de(v: ValueRef<'_>) -> u64 {
    u64::try_from(inteiro_de(v)).unwrap_or(0)
}

/// O texto do valor (para filtros de texto e para salvar uma célula): real como `repr`, NaN e infinitos como
/// "nan", "inf" e "-inf", BLOB como UTF-8 com substituição.
pub(crate) fn texto_do_valor(v: &Valor) -> String {
    match v {
        Valor::Nulo => String::new(),
        Valor::Blob(b) => String::from_utf8_lossy(b).into_owned(),
        Valor::Real(r) if r.is_nan() => "nan".to_string(),
        Valor::Real(r) if r.is_infinite() => if *r > 0.0 { "inf" } else { "-inf" }.to_string(),
        outro => outro.texto_exibicao(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn padroes_numericos() {
        for s in ["1", "+7", "-12", "007"] {
            assert!(e_inteiro(s), "{s}");
        }
        for s in ["", "+", "1.0", "1 ", "a1"] {
            assert!(!e_inteiro(s), "{s}");
        }
        for s in ["1.", ".5", "1.5", "1e3", "-2.25e-07", "+3E+2", "10"] {
            assert!(e_decimal(s), "{s}");
            assert!(s.parse::<f64>().is_ok(), "{s}");
        }
        for s in [".", "e3", "1e", "1.5.2", "1,5", "--1", "inf", "nan"] {
            assert!(!e_decimal(s), "{s}");
        }
        assert!(
            e_decimal_virgula("10,5")
                && e_decimal_virgula("-1,0")
                && !e_decimal_virgula("1,")
                && !e_decimal_virgula(",5")
        );
        assert!(e_milhar_virgula("1.234,56") && e_milhar_virgula("-12.345.678,9"));
        assert!(!e_milhar_virgula("1.23,4") && !e_milhar_virgula("1234.567,8") && !e_milhar_virgula("1.234"));
        assert!(e_inteiro_canonico("-0") && !e_inteiro_canonico("+1"));
    }
}
