//! Formatação pt-BR, com o mesmo resultado do `Intl.NumberFormat("pt-BR")`: milhar com ponto, decimal com vírgula, arredondamento "metade para longe do
//! zero" sobre a representação decimal mais curta do número (como o ICU faz).
//!
//! - [`formatar_numero`]: `5000` → `"5.000"`, `1.5` → `"1,5"` (até 6 casas), `None` → `"—"`.
//! - [`formatar_bytes`] / [`partes_bytes`]: `"1,2 MB"`, `"12 KB"`, `"512 bytes"`, `"1 byte"`.
//! - [`formatar_data`]: `"2026-09-22T10:31:00"` → `"22/09/2026 10:31"`.
//! - [`formatar_ms`]: `"320 ms"` (o `tempo()` da Consulta SQL) e [`formatar_segundos`]: `"4,6 s"` (o
//!   `segundos()` da aba Dados).
//! - [`plural`]: `plural(60, "linha", "linhas")` → `"60 linhas"`.
//! - [`nome_do_caminho`]: `"C:\\dados\\vendas.db"` → `"vendas.db"`.
//! - [`real_js`]: real como o `String(número)` do JavaScript (`17051.0` → `"17051"`, `1e-7` → `"1e-7"`), o que a
//!   grade mostra e copia.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

/// Traço usado onde não há valor (o `"—"` do JavaScript).
pub const TRACO: &str = "—";

/// Número aceito pelas funções de formatação: inteiros saem exatos (mesmo acima de 2^53) e reais passam pelo
/// arredondamento do `Intl.NumberFormat`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numero {
    Inteiro(i128),
    Real(f64),
}

macro_rules! numero_de_inteiro {
    ($($t:ty),*) => {$(
        impl From<$t> for Numero {
            fn from(v: $t) -> Self {
                Numero::Inteiro(v as i128)
            }
        }
    )*};
}
numero_de_inteiro!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl From<f64> for Numero {
    fn from(v: f64) -> Self {
        Numero::Real(v)
    }
}

impl From<f32> for Numero {
    fn from(v: f32) -> Self {
        Numero::Real(f64::from(v))
    }
}

impl Numero {
    fn como_f64(self) -> f64 {
        match self {
            Numero::Inteiro(i) => i as f64,
            Numero::Real(r) => r,
        }
    }

    fn e_um(self) -> bool {
        match self {
            Numero::Inteiro(i) => i == 1,
            Numero::Real(r) => r == 1.0,
        }
    }
}

/// Dígitos decimais de um número finito e não negativo: `digitos` (sem zeros à esquerda) × 10^`expoente`.
/// Para reais, são os dígitos da representação mais curta que volta ao mesmo número.
struct Decimal {
    digitos: Vec<u8>,
    /// Posição da vírgula: quantos dígitos ficam na parte inteira (pode ser ≤ 0 ou maior que `digitos.len()`).
    ponto: i64,
}

impl Decimal {
    fn de_inteiro(v: u128) -> Decimal {
        if v == 0 {
            return Decimal { digitos: vec![0], ponto: 1 };
        }
        let digitos: Vec<u8> = v.to_string().bytes().map(|b| b - b'0').collect();
        let ponto = digitos.len() as i64;
        Decimal { digitos, ponto }
    }

    fn de_real(v: f64) -> Decimal {
        // `{:e}` dá os dígitos mais curtos: "1.2345e-5", "5e3", "0e0".
        let texto = format!("{:e}", v.abs());
        let (mantissa, expoente) = texto.split_once('e').unwrap_or((texto.as_str(), "0"));
        let expoente: i64 = expoente.parse().unwrap_or(0);
        let digitos: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
        if digitos.iter().all(|&d| d == 0) {
            return Decimal { digitos: vec![0], ponto: 1 };
        }
        Decimal { digitos, ponto: expoente + 1 }
    }

    /// Arredonda para `casas` casas decimais, metade para longe do zero (o `halfExpand` do ICU).
    fn arredondar(mut self, casas: i64) -> Decimal {
        let manter = self.ponto + casas; // quantos dígitos ficam
        if manter >= self.digitos.len() as i64 {
            return self;
        }
        if manter < 0 {
            return Decimal { digitos: vec![0], ponto: 1 };
        }
        let manter = manter as usize;
        let sobe = self.digitos.get(manter).is_some_and(|&d| d >= 5);
        self.digitos.truncate(manter);
        if sobe {
            let mut i = self.digitos.len();
            loop {
                if i == 0 {
                    self.digitos.insert(0, 1);
                    self.ponto += 1;
                    break;
                }
                i -= 1;
                if self.digitos[i] == 9 {
                    self.digitos[i] = 0;
                } else {
                    self.digitos[i] += 1;
                    break;
                }
            }
        }
        if self.digitos.is_empty() || self.digitos.iter().all(|&d| d == 0) {
            return Decimal { digitos: vec![0], ponto: 1 };
        }
        self
    }

    /// Texto pt-BR com milhar em ponto e entre `min_casas` e `max_casas` casas decimais.
    fn escrever(&self, negativo: bool, min_casas: usize, max_casas: usize) -> String {
        let digito = |pos: i64| -> u8 {
            // pos: índice do dígito em `digitos` (pode estar fora: vira zero)
            if pos < 0 { 0 } else { self.digitos.get(pos as usize).copied().unwrap_or(0) }
        };
        let n_inteira = self.ponto.max(1);
        let mut inteira = String::with_capacity(n_inteira as usize + 8);
        for k in 0..n_inteira {
            // dígito na posição k da parte inteira; com ponto ≤ 0 a parte inteira é "0"
            let pos = self.ponto - n_inteira + k;
            if k > 0 && (n_inteira - k) % 3 == 0 {
                inteira.push('.');
            }
            inteira.push(char::from(b'0' + digito(pos)));
        }
        let mut fracao = String::new();
        for k in 0..max_casas as i64 {
            fracao.push(char::from(b'0' + digito(self.ponto + k)));
        }
        while fracao.len() > min_casas && fracao.ends_with('0') {
            fracao.pop();
        }
        let mut s = String::with_capacity(inteira.len() + fracao.len() + 2);
        // como o Intl: um negativo que arredonda para zero sai "-0"
        if negativo {
            s.push('-');
        }
        s.push_str(&inteira);
        if !fracao.is_empty() {
            s.push(',');
            s.push_str(&fracao);
        }
        s
    }
}

/// O `Intl.NumberFormat("pt-BR", {minimumFractionDigits, maximumFractionDigits})` para um número.
fn formatar_com_casas(n: Numero, min_casas: usize, max_casas: usize) -> String {
    match n {
        Numero::Inteiro(i) => Decimal::de_inteiro(i.unsigned_abs()).escrever(i < 0, min_casas, max_casas),
        Numero::Real(r) => {
            if r.is_nan() {
                return "NaN".into();
            }
            if r.is_infinite() {
                return if r < 0.0 { "-∞".into() } else { "∞".into() };
            }
            Decimal::de_real(r).arredondar(max_casas as i64).escrever(r.is_sign_negative(), min_casas, max_casas)
        }
    }
}

/// `5000` → `"5.000"`; `1.5` → `"1,5"`; até 6 casas decimais.
pub fn formatar_numero(n: impl Into<Numero>) -> String {
    formatar_com_casas(n.into(), 0, 6)
}

/// Como [`formatar_numero`], com entre `min_casas` e `max_casas` casas decimais (`98231.4` com 2 e 2 →
/// `"98.231,40"`): as contas do rodapé da seleção.
pub fn formatar_numero_com_casas(n: impl Into<Numero>, min_casas: usize, max_casas: usize) -> String {
    formatar_com_casas(n.into(), min_casas, max_casas.max(min_casas))
}

/// Como [`formatar_numero`], com `None` → `"—"`.
pub fn formatar_numero_opt<N: Into<Numero>>(n: Option<N>) -> String {
    match n {
        Some(n) => formatar_numero(n),
        None => TRACO.into(),
    }
}

/// Unidade de tamanho (a escala do `partesBytes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnidadeBytes {
    Kb,
    Mb,
    Gb,
    Tb,
}

impl UnidadeBytes {
    pub fn como_str(self) -> &'static str {
        match self {
            UnidadeBytes::Kb => "KB",
            UnidadeBytes::Mb => "MB",
            UnidadeBytes::Gb => "GB",
            UnidadeBytes::Tb => "TB",
        }
    }

    fn indice(self) -> i32 {
        match self {
            UnidadeBytes::Kb => 0,
            UnidadeBytes::Mb => 1,
            UnidadeBytes::Gb => 2,
            UnidadeBytes::Tb => 3,
        }
    }

    fn de_indice(i: i32) -> UnidadeBytes {
        match i {
            ..=0 => UnidadeBytes::Kb,
            1 => UnidadeBytes::Mb,
            2 => UnidadeBytes::Gb,
            _ => UnidadeBytes::Tb,
        }
    }
}

/// Tamanho em partes, para tiles: `{numero: "1,2", unidade: "MB"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartesBytes {
    pub numero: String,
    /// `"bytes"`, `"byte"`, `"KB"`, `"MB"`, `"GB"`, `"TB"` (vazio quando não há número).
    pub unidade: &'static str,
}

/// Abaixo de 1024 em bytes ("1 byte", "512 bytes"); acima, na maior
/// unidade que cabe (até TB), com uma casa abaixo de 10 ("1,2 MB") e sem casas a partir de 10 ("12 KB").
/// `unidade` força a escala.
pub fn partes_bytes(n: impl Into<Numero>, unidade: Option<UnidadeBytes>) -> PartesBytes {
    let n = n.into();
    let v = n.como_f64();
    if v.is_nan() {
        return PartesBytes { numero: TRACO.into(), unidade: "" };
    }
    let unidade = match unidade {
        Some(u) => u,
        None => {
            if v.abs() < 1024.0 {
                return PartesBytes {
                    numero: formatar_com_casas(n, 0, 0),
                    unidade: if n.e_um() { "byte" } else { "bytes" },
                };
            }
            // a maior potência de 1024 que cabe, contada sem erro de ponto flutuante nas potências exatas
            let mut indice = -1;
            let mut limite = 1024.0_f64;
            while v.abs() >= limite && indice < 3 {
                indice += 1;
                limite *= 1024.0;
            }
            UnidadeBytes::de_indice(indice)
        }
    };
    let escala = 1024.0_f64.powi(unidade.indice() + 1);
    let x = v / escala;
    let numero = if x.abs() < 10.0 {
        formatar_com_casas(Numero::Real(x), 1, 1)
    } else {
        formatar_com_casas(Numero::Real(x), 0, 0)
    };
    PartesBytes { numero, unidade: unidade.como_str() }
}

/// `"1,2 MB"`, `"12 KB"`, `"512 bytes"`.
pub fn formatar_bytes(n: impl Into<Numero>) -> String {
    juntar_partes(partes_bytes(n, None))
}

/// [`formatar_bytes`] numa unidade fixa (`"0,9 MB"`).
pub fn formatar_bytes_em(n: impl Into<Numero>, unidade: UnidadeBytes) -> String {
    juntar_partes(partes_bytes(n, Some(unidade)))
}

/// Como [`formatar_bytes`], com `None` → `"—"`.
pub fn formatar_bytes_opt<N: Into<Numero>>(n: Option<N>) -> String {
    match n {
        Some(n) => formatar_bytes(n),
        None => TRACO.into(),
    }
}

fn juntar_partes(p: PartesBytes) -> String {
    if p.unidade.is_empty() { p.numero } else { format!("{} {}", p.numero, p.unidade) }
}

/// `"2026-09-22T10:31:00"` → `"22/09/2026 10:31"`; data sem hora → `"22/09/2026"`; vazio → `"—"`.
/// Com fuso (`Z`, `+03:00`), converte para a hora local, como o `new Date()` do JavaScript. Texto que não é
/// data volta como veio.
pub fn formatar_data(iso: &str) -> String {
    if iso.is_empty() {
        return TRACO.into();
    }
    let b = iso.as_bytes();
    let tem_fuso = termina_com_fuso(iso);
    if !tem_fuso && let Some((a, m, d, hora)) = data_iso_sem_fuso(b) {
        return match hora {
            Some((hh, mm)) => format!("{d}/{m}/{a} {hh}:{mm}"),
            None => format!("{d}/{m}/{a}"),
        };
    }
    if let Some(data) = ler_com_fuso(iso) {
        let local = data.with_timezone(&Local);
        return local.format("%d/%m/%Y %H:%M").to_string();
    }
    if let Some(data) = ler_sem_fuso(iso) {
        return data.format("%d/%m/%Y %H:%M").to_string();
    }
    iso.to_string()
}

/// Como [`formatar_data`], com `None` → `"—"`.
pub fn formatar_data_opt(iso: Option<&str>) -> String {
    formatar_data(iso.unwrap_or(""))
}

/// `/(Z|[+-]\d{2}:?\d{2})$/`
fn termina_com_fuso(s: &str) -> bool {
    if s.ends_with('Z') {
        return true;
    }
    let b = s.as_bytes();
    let digitos = |fatia: &[u8]| fatia.iter().all(u8::is_ascii_digit);
    // [+-]dd:dd
    if b.len() >= 6 {
        let f = &b[b.len() - 6..];
        if (f[0] == b'+' || f[0] == b'-') && digitos(&f[1..3]) && f[3] == b':' && digitos(&f[4..6]) {
            return true;
        }
    }
    // [+-]dddd
    if b.len() >= 5 {
        let f = &b[b.len() - 5..];
        if (f[0] == b'+' || f[0] == b'-') && digitos(&f[1..5]) {
            return true;
        }
    }
    false
}

type DataPartes<'a> = (&'a str, &'a str, &'a str, Option<(&'a str, &'a str)>);

/// `/^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2}))?/`
fn data_iso_sem_fuso(b: &[u8]) -> Option<DataPartes<'_>> {
    let digitos = |r: core::ops::Range<usize>| b.get(r.clone()).is_some_and(|f| f.iter().all(u8::is_ascii_digit));
    if !(digitos(0..4) && b.get(4) == Some(&b'-') && digitos(5..7) && b.get(7) == Some(&b'-') && digitos(8..10)) {
        return None;
    }
    // só ASCII até aqui: as fatias caem em fronteira de caractere
    let s = core::str::from_utf8(b).ok()?;
    let (a, m, d) = (s.get(0..4)?, s.get(5..7)?, s.get(8..10)?);
    let hora = if matches!(b.get(10), Some(b'T') | Some(b' '))
        && digitos(11..13)
        && b.get(13) == Some(&b':')
        && digitos(14..16)
    {
        Some((s.get(11..13)?, s.get(14..16)?))
    } else {
        None
    };
    Some((a, m, d, hora))
}

fn ler_com_fuso(s: &str) -> Option<DateTime<chrono::FixedOffset>> {
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d);
    }
    for formato in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%dT%H:%M%z", "%Y-%m-%d %H:%M:%S%.f%z", "%Y-%m-%d %H:%M%z"] {
        if let Ok(d) = DateTime::parse_from_str(s, formato) {
            return Some(d);
        }
    }
    if let Some(sem_z) = s.strip_suffix('Z') {
        let utc = chrono::FixedOffset::east_opt(0)?;
        if let Some(n) = ler_sem_fuso(sem_z) {
            return utc.from_local_datetime(&n).single();
        }
    }
    None
}

fn ler_sem_fuso(s: &str) -> Option<NaiveDateTime> {
    for formato in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M"] {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, formato) {
            return Some(d);
        }
    }
    None
}

/// `"320 ms"`, `"12,3 ms"`, `"1.234 ms"`: o `tempo(ms)` da Consulta SQL (`${formatarNumero(ms)} ms`).
pub fn formatar_ms(ms: impl Into<Numero>) -> String {
    format!("{} ms", formatar_numero(ms))
}

/// `"4,6 s"`: o `segundos(ms)` da aba Dados (uma casa decimal, nunca negativo).
pub fn formatar_segundos(ms: impl Into<Numero>) -> String {
    let ms = ms.into().como_f64();
    let s = if ms > 0.0 { ms / 1000.0 } else { 0.0 };
    format!("{} s", formatar_com_casas(Numero::Real(s), 1, 1))
}

/// Contagem com a palavra no número certo: `plural(60, "linha", "linhas")` → `"60 linhas"`;
/// `plural(1, …)` → `"1 linha"`.
pub fn plural(n: impl Into<Numero>, um: &str, varios: &str) -> String {
    let n = n.into();
    format!("{} {}", formatar_numero(n), if n.e_um() { um } else { varios })
}

/// O rodapé das grades (a aba Dados e o CSV): "Linhas 1–100 de 5.000", com as linhas à vista (ou as da página) de
/// quantas a grade tem; "Nenhuma linha" sem linhas.
pub fn contagem_linhas(inicio: u64, fim: u64, total: u64) -> String {
    if total > 0 {
        format!("Linhas {}–{} de {}", formatar_numero(inicio), formatar_numero(fim), formatar_numero(total))
    } else {
        "Nenhuma linha".to_string()
    }
}

/// A contagem das linhas que a busca e os filtros deixam passar (a da barra dos filtros até a 0.2.3; hoje, só a dos
/// retratos dos testes): filtrando, as linhas que passam de quantas há ("41 de 60 linhas"; sem o
/// total, só as que passam); sem filtro, as linhas ("60 linhas").
pub fn contagem_filtrada(linhas: u64, total: Option<u64>, filtrando: bool) -> String {
    match total {
        Some(t) if filtrando => format!("{} de {}", formatar_numero(linhas), plural(t, "linha", "linhas")),
        _ => plural(linhas, "linha", "linhas"),
    }
}

/// O que o rodapé das grades diz depois da contagem quando a busca ou os filtros escondem linhas: de quantas a tabela
/// tem ("filtradas de 1.180"), U3 da revisão de UI de 22/10/2026.
pub fn filtradas_de(total: u64) -> String {
    format!("filtradas de {}", formatar_numero(total))
}

/// Nome do arquivo de um caminho do Windows (`"C:\\dados\\vendas.db"` → `"vendas.db"`).
pub fn nome_do_caminho(caminho: &str) -> &str {
    caminho.split(['\\', '/']).rfind(|p| !p.is_empty()).unwrap_or(caminho)
}

/// A letra sem acento ([`comum::texto::letra_sem_acento`]) e o texto sem acentos e em minúsculas, a chave das
/// buscas e dos filtros ([`comum::texto::sem_acento`]): moram no `comum` para o `tabular` e o `planilha` buscarem do
/// mesmo jeito.
pub use comum::texto::{letra_sem_acento, sem_acento};

/// Real como o JavaScript escreve com `String(número)` (o `Number.prototype.toString`): os dígitos mais curtos que voltam ao mesmo número, inteiro sem ".0"
/// (`17051.0` → `"17051"`), notação exponencial só fora de [1e-7, 1e21) com `e+`/`e-` e sem zero à esquerda
/// no expoente (`1e-7` → `"1e-7"`, `1.5e300` → `"1.5e+300"`, `1e-5` → `"0.00001"`, `1e16` →
/// `"10000000000000000"`), `-0` → `"0"`, e `"Infinity"`, `"-Infinity"`, `"NaN"`.
pub fn real_js(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let sinal = if x < 0.0 { "-" } else { "" };
    // `{:e}` dá os dígitos mais curtos que voltam ao mesmo número (como o JavaScript): "1.7051e4"
    let cientifico = format!("{:e}", x.abs());
    let (mantissa, expoente) = cientifico.split_once('e').unwrap_or((cientifico.as_str(), "0"));
    let expoente: i32 = expoente.parse().unwrap_or(0);
    let digitos: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    // a especificação: x = s × 10^(n−k), s com k dígitos
    let k = digitos.len() as i32;
    let n = expoente + 1;
    let corpo = if k <= n && n <= 21 {
        format!("{digitos}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        match digitos.split_at_checked(n as usize) {
            Some((inteira, fracao)) => format!("{inteira}.{fracao}"),
            None => digitos,
        }
    } else if -6 < n && n <= 0 {
        format!("0.{}{digitos}", "0".repeat((-n) as usize))
    } else {
        let sinal_expoente = if n - 1 < 0 { '-' } else { '+' };
        let e = (n - 1).abs();
        match digitos.split_at_checked(1) {
            Some((primeiro, resto)) if !resto.is_empty() => format!("{primeiro}.{resto}e{sinal_expoente}{e}"),
            _ => format!("{digitos}e{sinal_expoente}{e}"),
        }
    };
    format!("{sinal}{corpo}")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contagens_das_grades() {
        assert_eq!(contagem_linhas(101, 200, 5000), "Linhas 101–200 de 5.000");
        assert_eq!(contagem_linhas(0, 0, 0), "Nenhuma linha");
        assert_eq!(contagem_filtrada(41, Some(60), true), "41 de 60 linhas");
        assert_eq!(contagem_filtrada(60, Some(60), false), "60 linhas");
        assert_eq!(contagem_filtrada(1, Some(1), true), "1 de 1 linha");
        assert_eq!(contagem_filtrada(2500, None, true), "2.500 linhas", "sem o total, só as que passam");
        assert_eq!(filtradas_de(1180), "filtradas de 1.180");
    }

    #[test]
    fn sem_acento_para_buscas() {
        assert_eq!(sem_acento("Tabela com Espaço"), "tabela com espaco");
        assert!(sem_acento("Ações_Úteis").contains(&sem_acento("acoes_u")));
        assert_eq!(sem_acento("São Paulo · Ñandú ÿ Šódź"), "sao paulo · nandu y sodz");
        // a marca combinante solta (o "a" + U+0303 do NFD) some
        assert_eq!(sem_acento("a\u{303}o"), "ao");
        assert_eq!(letra_sem_acento('B'), Some('B'));
        assert_eq!(letra_sem_acento('\u{301}'), None);
    }

    #[test]
    fn numeros_inteiros_com_milhar() {
        assert_eq!(formatar_numero(0), "0");
        assert_eq!(formatar_numero(60), "60");
        assert_eq!(formatar_numero(999), "999");
        assert_eq!(formatar_numero(1000), "1.000");
        assert_eq!(formatar_numero(5000), "5.000");
        assert_eq!(formatar_numero(1_000_000), "1.000.000");
        assert_eq!(formatar_numero(-12_345), "-12.345");
        assert_eq!(formatar_numero(9_007_199_254_740_993_i64), "9.007.199.254.740.993");
        assert_eq!(formatar_numero(u64::MAX), "18.446.744.073.709.551.615");
    }

    #[test]
    fn numeros_reais_ate_seis_casas() {
        assert_eq!(formatar_numero(1.5), "1,5");
        assert_eq!(formatar_numero(1234.5678), "1.234,5678");
        assert_eq!(formatar_numero(0.1234567), "0,123457");
        assert_eq!(formatar_numero(0.0000001), "0");
        assert_eq!(formatar_numero(-0.0000001), "-0");
        assert_eq!(formatar_numero(2.0), "2");
        assert_eq!(formatar_numero(12.3), "12,3");
        assert_eq!(formatar_numero(0.1 + 0.2), "0,3");
        assert_eq!(formatar_numero(1e21), "1.000.000.000.000.000.000.000");
        assert_eq!(formatar_numero(f64::NAN), "NaN");
        assert_eq!(formatar_numero(f64::INFINITY), "∞");
        assert_eq!(formatar_numero_opt::<i64>(None), "—");
    }

    #[test]
    fn arredonda_metade_para_longe_do_zero_na_forma_decimal_curta() {
        // 1.15 em binário é 1,1499999…; o ICU arredonda os dígitos "1.15" e dá 1,2
        assert_eq!(formatar_com_casas(Numero::Real(1.15), 1, 1), "1,2");
        assert_eq!(formatar_com_casas(Numero::Real(1.25), 1, 1), "1,3");
        assert_eq!(formatar_com_casas(Numero::Real(-1.25), 1, 1), "-1,3");
        assert_eq!(formatar_com_casas(Numero::Real(9.96), 1, 1), "10,0");
        assert_eq!(formatar_com_casas(Numero::Real(999.5), 0, 0), "1.000");
        assert_eq!(formatar_com_casas(Numero::Real(0.5), 0, 0), "1");
        assert_eq!(formatar_com_casas(Numero::Real(0.4), 0, 0), "0");
    }

    #[test]
    fn bytes_como_ui_js() {
        assert_eq!(formatar_bytes(0), "0 bytes");
        assert_eq!(formatar_bytes(1), "1 byte");
        assert_eq!(formatar_bytes(512), "512 bytes");
        assert_eq!(formatar_bytes(1023), "1.023 bytes");
        assert_eq!(formatar_bytes(1024), "1,0 KB");
        assert_eq!(formatar_bytes(2355), "2,3 KB");
        assert_eq!(formatar_bytes(12 * 1024), "12 KB");
        assert_eq!(formatar_bytes(1_258_291), "1,2 MB");
        assert_eq!(formatar_bytes(1024 * 1024), "1,0 MB");
        assert_eq!(formatar_bytes(1024_u64.pow(3)), "1,0 GB");
        assert_eq!(formatar_bytes(1024_u64.pow(4)), "1,0 TB");
        assert_eq!(formatar_bytes(1024_u64.pow(5)), "1.024 TB");
        assert_eq!(formatar_bytes_opt::<u64>(None), "—");
        assert_eq!(formatar_bytes(f64::NAN), "—");
    }

    #[test]
    fn bytes_em_partes_e_com_unidade_fixa() {
        assert_eq!(partes_bytes(1_258_291, None), PartesBytes { numero: "1,2".into(), unidade: "MB" });
        assert_eq!(partes_bytes(943_718, Some(UnidadeBytes::Mb)), PartesBytes { numero: "0,9".into(), unidade: "MB" });
        assert_eq!(formatar_bytes_em(943_718, UnidadeBytes::Mb), "0,9 MB");
        assert_eq!(formatar_bytes_em(100, UnidadeBytes::Kb), "0,1 KB");
    }

    #[test]
    fn datas_iso() {
        assert_eq!(formatar_data(""), "—");
        assert_eq!(formatar_data_opt(None), "—");
        assert_eq!(formatar_data("2026-09-22T10:31:00"), "22/09/2026 10:31");
        assert_eq!(formatar_data("2026-09-22 10:31:45.123"), "22/09/2026 10:31");
        assert_eq!(formatar_data("2026-09-22"), "22/09/2026");
        assert_eq!(formatar_data("ontem"), "ontem");
        // com fuso: hora local (aqui só a forma; o fuso da máquina decide a hora)
        let local = formatar_data("2026-09-22T10:31:00Z");
        assert_eq!(local.len(), "22/09/2026 10:31".len());
        assert!(local.contains("/2026 "), "{local}");
        let esperado = DateTime::parse_from_rfc3339("2026-09-22T10:31:00+03:00")
            .map(|d| d.with_timezone(&Local).format("%d/%m/%Y %H:%M").to_string())
            .unwrap_or_default();
        assert_eq!(formatar_data("2026-09-22T10:31:00+03:00"), esperado);
        assert_eq!(formatar_data("2026-09-22T10:31:00+0300"), esperado);
    }

    #[test]
    fn tempos() {
        assert_eq!(formatar_ms(320), "320 ms");
        assert_eq!(formatar_ms(12.3), "12,3 ms");
        assert_eq!(formatar_ms(1234.5), "1.234,5 ms");
        assert_eq!(formatar_segundos(4600.0), "4,6 s");
        assert_eq!(formatar_segundos(1500), "1,5 s");
        assert_eq!(formatar_segundos(-20.0), "0,0 s");
        assert_eq!(formatar_segundos(12_345.0), "12,3 s");
    }

    #[test]
    fn plurais() {
        assert_eq!(plural(60, "linha", "linhas"), "60 linhas");
        assert_eq!(plural(1, "linha", "linhas"), "1 linha");
        assert_eq!(plural(0, "linha", "linhas"), "0 linhas");
        assert_eq!(plural(5000, "linha", "linhas"), "5.000 linhas");
        assert_eq!(plural(1.0, "tabela", "tabelas"), "1 tabela");
        assert_eq!(plural(1.5, "tabela", "tabelas"), "1,5 tabelas");
    }

    #[test]
    fn nome_do_arquivo() {
        assert_eq!(nome_do_caminho("C:\\dados\\vendas.db"), "vendas.db");
        assert_eq!(nome_do_caminho("/tmp/x/y.sqlite"), "y.sqlite");
        assert_eq!(nome_do_caminho("vendas.db"), "vendas.db");
        assert_eq!(nome_do_caminho("C:\\dados\\"), "dados");
        assert_eq!(nome_do_caminho(""), "");
    }

    #[test]
    fn reais_como_o_string_do_javascript() {
        // o limite de clientes no banco de exemplo (round(…, 2) que caiu num inteiro): a grade web mostra "17051"
        assert_eq!(real_js(17051.0), "17051");
        assert_eq!(real_js(1500.0), "1500");
        assert_eq!(real_js(10.0), "10");
        assert_eq!(real_js(2.5), "2.5");
        assert_eq!(real_js(-2.5), "-2.5");
        assert_eq!(real_js(1234.5678), "1234.5678");
        assert_eq!(real_js(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(real_js(1_234_567.5), "1234567.5");
        assert_eq!(real_js(1e-5), "0.00001");
        assert_eq!(real_js(0.000001), "0.000001");
        assert_eq!(real_js(1e-7), "1e-7");
        assert_eq!(real_js(2.5e-7), "2.5e-7");
        assert_eq!(real_js(1e16), "10000000000000000");
        assert_eq!(real_js(123_456_789_012_345_680_000.0), "123456789012345680000");
        assert_eq!(real_js(1e21), "1e+21");
        assert_eq!(real_js(1.5e300), "1.5e+300");
        assert_eq!(real_js(f64::MAX), "1.7976931348623157e+308");
        assert_eq!(real_js(5e-324), "5e-324");
        assert_eq!(real_js(-1e-7), "-1e-7");
        assert_eq!(real_js(0.0), "0");
        assert_eq!(real_js(-0.0), "0");
        assert_eq!(real_js(f64::INFINITY), "Infinity");
        assert_eq!(real_js(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(real_js(f64::NAN), "NaN");
    }
}
