//! Formatos de número do Excel (`numFmts` e `cellXfs` do `styles.xml`): de que tipo é a célula (data, hora,
//! percentual, número…) e, para mostrar na tela, como o Excel escreveria o valor.
//!
//! O que está coberto (o que aparece em planilhas de verdade): casas decimais e separador de milhar
//! (`#,##0.00`), texto fixo e moeda (`"R$" #,##0.00`, `[$R$-416] #,##0.00`), máscaras de dígitos
//! (`00000-000` do CEP, `000\.000\.000\-00` do CPF), seções por sinal (`0.00;[Red]-0.00;"zero"`), percentual,
//! científico (`0.00E+00`), frações (`# ?/?`), datas e horas por extenso (`dd/mm/yyyy`, `dddd, d "de" mmmm`,
//! `h:mm AM/PM`, com os nomes em português) e durações (`[h]:mm:ss`). Cores e condições (`[Red]`, `[>100]`)
//! são ignoradas; moeda contábil com preenchimento (`_(* #,##0_)`) sai como número simples.

use crate::datas::dia_da_semana;
use crate::valores::{Data, Hora, Valor};

/// A espécie de um formato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Categoria {
    /// `General` (ou nenhum formato).
    Geral,
    Numero,
    Percentual,
    Cientifico,
    Fracao,
    Data,
    Hora,
    DataHora,
    /// Tempo decorrido, `[h]:mm:ss`.
    Duracao,
    /// `@`.
    Texto,
}

impl Categoria {
    /// A célula guarda uma data e/ou hora (a série vira [`Valor::Data`], [`Valor::Hora`] ou [`Valor::DataHora`]).
    pub fn e_data_ou_hora(self) -> bool {
        matches!(self, Categoria::Data | Categoria::Hora | Categoria::DataHora)
    }
}

/// Como escrever o número (separadores) e os nomes de meses e dias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Localidade {
    pub decimal: char,
    pub milhar: char,
    pub portugues: bool,
}

impl Localidade {
    /// 1.234,56 · janeiro · VERDADEIRO
    pub const PT_BR: Localidade = Localidade { decimal: ',', milhar: '.', portugues: true };
    /// 1,234.56 · January · TRUE
    pub const INVARIANTE: Localidade = Localidade { decimal: '.', milhar: ',', portugues: false };
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Lit(String),
    Dig(char),
    Ponto,
    Milhar,
    Pct,
    Exp(bool),
    Barra,
    Geral,
    Texto,
    Ano(u8),
    Mes(u8),
    Min(u8),
    Dia(u8),
    Hora(u8),
    Seg(u8),
    AmPm(bool),
    Frac(u8),
    Decorrido(char, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Geral,
    Numero,
    Fracao,
    Data,
    Duracao,
    Texto,
}

#[derive(Debug, Clone, PartialEq)]
struct Secao {
    tipo: Tipo,
    toks: Vec<Tok>,
}

/// Um formato de número já analisado.
#[derive(Debug, Clone, PartialEq)]
pub struct Formato {
    codigo: String,
    categoria: Categoria,
    secoes: Vec<Secao>,
}

impl Default for Formato {
    fn default() -> Self {
        Formato::geral()
    }
}

/// O código dos formatos embutidos (`numFmtId` sem `formatCode`), no português do Brasil onde o Excel
/// escolhe pela região (14 e 22 são a data curta do sistema).
pub fn formato_interno(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 | 27..=31 | 36 | 50..=58 => "dd/mm/yyyy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 | 32..=35 => "h:mm:ss",
        22 => "dd/mm/yyyy h:mm",
        37 | 38 => "#,##0;-#,##0",
        39 | 40 => "#,##0.00;-#,##0.00",
        41 => "#,##0",
        42 => "#,##0",
        43 | 44 => "#,##0.00",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

impl Formato {
    pub fn geral() -> Formato {
        Formato::novo("General")
    }

    /// Analisa um código de formato (`formatCode`).
    pub fn novo(codigo: &str) -> Formato {
        let partes = dividir_secoes(codigo);
        let categoria = categoria_da_secao(partes.first().map_or("", String::as_str));
        let tipo = match categoria {
            Categoria::Geral => Tipo::Geral,
            Categoria::Texto => Tipo::Texto,
            Categoria::Fracao => Tipo::Fracao,
            Categoria::Duracao => Tipo::Duracao,
            Categoria::Data | Categoria::Hora | Categoria::DataHora => Tipo::Data,
            _ => Tipo::Numero,
        };
        // data, hora e duração só usam a primeira seção
        let usadas = if matches!(tipo, Tipo::Data | Tipo::Duracao | Tipo::Fracao | Tipo::Geral | Tipo::Texto) {
            &partes[..partes.len().min(1)]
        } else {
            &partes[..]
        };
        let secoes = usadas
            .iter()
            .map(|s| {
                let t = if tipo == Tipo::Numero { tipo_numerico(s) } else { tipo };
                Secao { tipo: t, toks: tokenizar(s, matches!(t, Tipo::Data | Tipo::Duracao)) }
            })
            .collect();
        Formato { codigo: codigo.to_string(), categoria, secoes }
    }

    /// Pelo `numFmtId` (e, nos personalizados, o `formatCode`).
    pub fn de_id(id: u32, codigo: Option<&str>) -> Formato {
        match codigo.or_else(|| formato_interno(id)) {
            Some(c) => Formato::novo(c),
            // id desconhecido, sem código: General
            None => Formato::geral(),
        }
    }

    pub fn codigo(&self) -> &str {
        &self.codigo
    }

    pub fn categoria(&self) -> Categoria {
        self.categoria
    }

    /// Escreve o valor como o Excel (em português do Brasil).
    pub fn exibir(&self, valor: &Valor) -> String {
        self.exibir_com(valor, &Localidade::PT_BR)
    }

    pub fn exibir_com(&self, valor: &Valor, loc: &Localidade) -> String {
        match valor {
            Valor::Vazio => String::new(),
            Valor::Texto(t) => t.clone(),
            Valor::Erro(e) => e.clone(),
            Valor::Booleano(b) => match (b, loc.portugues) {
                (true, true) => "VERDADEIRO".to_string(),
                (false, true) => "FALSO".to_string(),
                (true, false) => "TRUE".to_string(),
                (false, false) => "FALSE".to_string(),
            },
            Valor::Numero(x) => self.numero(*x, loc),
            Valor::Data(d) => self.data_hora(Some(*d), None, loc).unwrap_or_else(|| d.to_string()),
            Valor::Hora(h) => self.data_hora(None, Some(*h), loc).unwrap_or_else(|| h.to_string()),
            Valor::DataHora(m) => self.data_hora(Some(m.data), Some(m.hora), loc).unwrap_or_else(|| m.to_string()),
        }
    }

    fn numero(&self, x: f64, loc: &Localidade) -> String {
        if !x.is_finite() {
            return "#NUM!".to_string();
        }
        let Some(primeira) = self.secoes.first() else {
            return geral(x, loc);
        };
        match primeira.tipo {
            Tipo::Geral => {
                // `General` pode vir com texto fixo (`General" kg"`)
                primeira.toks.iter().map(|t| if matches!(t, Tok::Geral) { geral(x, loc) } else { lit(t) }).collect()
            }
            Tipo::Texto | Tipo::Data => geral(x, loc),
            Tipo::Duracao => duracao(&primeira.toks, x),
            Tipo::Fracao => {
                let corpo = fracao(&primeira.toks, x.abs());
                if x < 0.0 { format!("-{corpo}") } else { corpo }
            }
            Tipo::Numero => {
                let n = self.secoes.len();
                let (secao, sinal) = if x < 0.0 && n >= 2 {
                    (&self.secoes[1], false)
                } else if x == 0.0 && n >= 3 {
                    (&self.secoes[2], false)
                } else {
                    (primeira, x < 0.0)
                };
                let corpo = numerico(&secao.toks, x.abs(), loc);
                // "-0" não existe: um negativo que arredonda para zero perde o sinal só se a seção for a do zero
                if sinal { format!("-{corpo}") } else { corpo }
            }
        }
    }

    fn data_hora(&self, d: Option<Data>, h: Option<Hora>, loc: &Localidade) -> Option<String> {
        let secao = self.secoes.first().filter(|s| s.tipo == Tipo::Data)?;
        Some(data_com_tokens(&secao.toks, d.unwrap_or(Data { ano: 1900, mes: 1, dia: 1 }), h.unwrap_or_default(), loc))
    }
}

// ---- análise do código ----------------------------------------------------------------------------------

/// Divide em seções por `;` fora de aspas, colchetes e escapes.
fn dividir_secoes(codigo: &str) -> Vec<String> {
    let mut saida = vec![String::new()];
    let mut aspas = false;
    let mut colchetes = false;
    let mut escape = false;
    for c in codigo.chars() {
        if escape {
            escape = false;
        } else if aspas {
            aspas = c != '"';
        } else if colchetes {
            colchetes = c != ']';
        } else {
            match c {
                '\\' | '_' | '*' => escape = true,
                '"' => aspas = true,
                '[' => colchetes = true,
                ';' => {
                    saida.push(String::new());
                    continue;
                }
                _ => {}
            }
        }
        if let Some(ultima) = saida.last_mut() {
            ultima.push(c);
        }
    }
    saida
}

/// A seção sem o que não conta para classificar: texto entre aspas, escapes, preenchimentos e colchetes
/// (menos `[h]`, `[m]`, `[s]`, que dão a duração). Em minúsculas.
fn limpar(secao: &str) -> (String, bool) {
    let c: Vec<char> = secao.chars().collect();
    let mut limpo = String::new();
    let mut decorrido = false;
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '"' => {
                i += 1;
                while i < c.len() && c[i] != '"' {
                    i += 1;
                }
            }
            '\\' | '_' | '*' => i += 1,
            '[' => {
                let mut j = i + 1;
                let mut dentro = String::new();
                while j < c.len() && c[j] != ']' {
                    dentro.push(c[j]);
                    j += 1;
                }
                let d = dentro.to_lowercase();
                if !d.is_empty()
                    && (d.chars().all(|x| x == 'h') || d.chars().all(|x| x == 'm') || d.chars().all(|x| x == 's'))
                {
                    decorrido = true;
                }
                i = j;
            }
            outro => limpo.extend(outro.to_lowercase()),
        }
        i += 1;
    }
    (limpo, decorrido)
}

fn categoria_da_secao(secao: &str) -> Categoria {
    let (l, decorrido) = limpar(secao);
    let t = l.trim();
    if decorrido {
        return Categoria::Duracao;
    }
    if t.is_empty() || t == "general" || t == "geral" {
        return Categoria::Geral;
    }
    let digitos = t.chars().any(|c| matches!(c, '0' | '#' | '?'));
    if t.contains('@') && !digitos {
        return Categoria::Texto;
    }
    let cientifico = t.contains("e+") || t.contains("e-");
    let ampm = t.contains("am/pm") || t.contains("a/p");
    let ymd = t.contains('y') || t.contains('d');
    let hs = t.contains('h') || t.contains('s') || ampm;
    let m = t.contains('m');
    if !cientifico {
        let data = ymd || (m && !hs);
        match (data, hs) {
            (true, true) => return Categoria::DataHora,
            (true, false) => return Categoria::Data,
            (false, true) => return Categoria::Hora,
            _ => {}
        }
    }
    if cientifico && digitos {
        Categoria::Cientifico
    } else if t.contains('%') {
        Categoria::Percentual
    } else if t.contains('/') && digitos {
        Categoria::Fracao
    } else {
        Categoria::Numero
    }
}

/// Dentro de um formato numérico, a seção pode ser de fração (`# ?/?`) ou ser texto puro.
fn tipo_numerico(secao: &str) -> Tipo {
    let (l, _) = limpar(secao);
    let t = l.trim();
    if t.is_empty() || t == "general" || t == "geral" { Tipo::Geral } else { Tipo::Numero }
}

fn tokenizar(secao: &str, data: bool) -> Vec<Tok> {
    let c: Vec<char> = secao.chars().collect();
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0;
    let comeca = |i: usize, texto: &str| -> bool {
        let n = texto.chars().count();
        c.get(i..i + n).is_some_and(|f| f.iter().collect::<String>().eq_ignore_ascii_case(texto))
    };
    let repete =
        |i: usize, letra: char| -> usize { c[i..].iter().take_while(|x| x.eq_ignore_ascii_case(&letra)).count() };
    while i < c.len() {
        let ch = c[i];
        match ch {
            '"' => {
                let mut j = i + 1;
                let mut s = String::new();
                while j < c.len() && c[j] != '"' {
                    s.push(c[j]);
                    j += 1;
                }
                toks.push(Tok::Lit(s));
                i = j + 1;
                continue;
            }
            '\\' => {
                if let Some(&n) = c.get(i + 1) {
                    toks.push(Tok::Lit(n.to_string()));
                }
                i += 2;
                continue;
            }
            '_' => {
                toks.push(Tok::Lit(" ".to_string()));
                i += 2;
                continue;
            }
            '*' => {
                i += 2;
                continue;
            }
            '[' => {
                let mut j = i + 1;
                let mut dentro = String::new();
                while j < c.len() && c[j] != ']' {
                    dentro.push(c[j]);
                    j += 1;
                }
                i = j + 1;
                let d = dentro.to_lowercase();
                let todos = |letra: char| !d.is_empty() && d.chars().all(|x| x == letra);
                for letra in ['h', 'm', 's'] {
                    if todos(letra) {
                        toks.push(Tok::Decorrido(letra, d.len().min(9) as u8));
                    }
                }
                if let Some(moeda) = dentro.strip_prefix('$') {
                    let simbolo = moeda.split('-').next().unwrap_or("");
                    if !simbolo.is_empty() {
                        toks.push(Tok::Lit(simbolo.to_string()));
                    }
                }
                continue;
            }
            _ => {}
        }
        if data {
            match ch.to_ascii_lowercase() {
                'y' => {
                    let n = repete(i, 'y');
                    toks.push(Tok::Ano(if n <= 2 { 2 } else { 4 }));
                    i += n;
                    continue;
                }
                'm' => {
                    let n = repete(i, 'm');
                    toks.push(Tok::Mes(n.min(5) as u8));
                    i += n;
                    continue;
                }
                'd' => {
                    let n = repete(i, 'd');
                    toks.push(Tok::Dia(n.min(4) as u8));
                    i += n;
                    continue;
                }
                'h' => {
                    let n = repete(i, 'h');
                    toks.push(Tok::Hora(n.min(2) as u8));
                    i += n;
                    continue;
                }
                's' => {
                    let n = repete(i, 's');
                    toks.push(Tok::Seg(n.min(2) as u8));
                    i += n;
                    continue;
                }
                'a' if comeca(i, "am/pm") => {
                    toks.push(Tok::AmPm(false));
                    i += 5;
                    continue;
                }
                'a' if comeca(i, "a/p") => {
                    toks.push(Tok::AmPm(true));
                    i += 3;
                    continue;
                }
                _ => {}
            }
            if ch == '.' && matches!(toks.last(), Some(Tok::Seg(_))) {
                let n = c[i + 1..].iter().take_while(|&&x| x == '0').count();
                if n > 0 {
                    toks.push(Tok::Frac(n.min(3) as u8));
                    i += 1 + n;
                    continue;
                }
            }
            toks.push(Tok::Lit(ch.to_string()));
            i += 1;
            continue;
        }
        match ch {
            '0' | '#' | '?' => toks.push(Tok::Dig(ch)),
            '.' => toks.push(Tok::Ponto),
            ',' => toks.push(Tok::Milhar),
            '%' => toks.push(Tok::Pct),
            '@' => toks.push(Tok::Texto),
            '/' => toks.push(Tok::Barra),
            'E' | 'e' if matches!(c.get(i + 1), Some('+' | '-')) => {
                toks.push(Tok::Exp(c[i + 1] == '+'));
                i += 1;
            }
            'G' | 'g' if comeca(i, "general") => {
                toks.push(Tok::Geral);
                i += 6;
            }
            outro => toks.push(Tok::Lit(outro.to_string())),
        }
        i += 1;
    }
    if data {
        resolver_minutos(&mut toks);
    }
    toks
}

/// `m` é minuto (e não mês) logo depois de hora ou logo antes de segundo, pulando só o texto fixo.
fn resolver_minutos(toks: &mut [Tok]) {
    for i in 0..toks.len() {
        let Tok::Mes(n) = toks[i] else { continue };
        if n > 2 {
            continue;
        }
        let antes = toks[..i].iter().rev().find(|t| !matches!(t, Tok::Lit(_)));
        let depois = toks[i + 1..].iter().find(|t| !matches!(t, Tok::Lit(_)));
        let depois_de_hora = matches!(antes, Some(Tok::Hora(_) | Tok::Decorrido('h', _)));
        let antes_de_segundo = matches!(depois, Some(Tok::Seg(_)));
        if depois_de_hora || antes_de_segundo {
            toks[i] = Tok::Min(n);
        }
    }
}

// ---- escrita dos números --------------------------------------------------------------------------------

/// 15 algarismos significativos (o que o Excel guarda e mostra): parte inteira e parte fracionária, sem os
/// zeros finais desta.
fn decimal_15(x: f64) -> (Vec<u8>, Vec<u8>) {
    if x == 0.0 {
        return (vec![b'0'], Vec::new());
    }
    let s = format!("{x:.14e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = e.parse().unwrap_or(0);
    let digs: Vec<u8> = m.bytes().filter(u8::is_ascii_digit).collect();
    let (int, mut frac) = if exp >= 0 {
        let n = exp as usize + 1;
        if n >= digs.len() {
            let mut int = digs.clone();
            int.resize(n, b'0');
            (int, Vec::new())
        } else {
            (digs[..n].to_vec(), digs[n..].to_vec())
        }
    } else {
        let mut frac = vec![b'0'; (-exp - 1) as usize];
        frac.extend_from_slice(&digs);
        (vec![b'0'], frac)
    };
    while frac.last() == Some(&b'0') {
        frac.pop();
    }
    (int, frac)
}

/// Soma 1 ao último algarismo de `digitos` (com vai-um); devolve se cresceu um algarismo à esquerda.
fn somar_um(digitos: &mut Vec<u8>) -> bool {
    for d in digitos.iter_mut().rev() {
        if *d == b'9' {
            *d = b'0';
        } else {
            *d += 1;
            return false;
        }
    }
    digitos.insert(0, b'1');
    true
}

/// Arredonda (meio para cima, como o Excel, sobre os 15 algarismos) para `casas` decimais.
fn arredondar(x: f64, casas: usize) -> (String, String) {
    let (mut int, mut frac) = decimal_15(x);
    if frac.len() > casas {
        let sobe = frac[casas] >= b'5';
        frac.truncate(casas);
        if sobe {
            let tam_int = int.len();
            let mut todos = int.clone();
            todos.extend_from_slice(&frac);
            somar_um(&mut todos);
            let novo_int = todos.len() - casas;
            frac = todos[novo_int..].to_vec();
            int = todos[..novo_int].to_vec();
            debug_assert!(int.len() >= tam_int);
        }
    }
    frac.resize(casas, b'0');
    (String::from_utf8_lossy(&int).into_owned(), String::from_utf8_lossy(&frac).into_owned())
}

/// `General`: até 15 algarismos significativos, sem zeros sobrando; científico para muito grande ou pequeno.
pub(crate) fn geral(x: f64, loc: &Localidade) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    let sinal = if x < 0.0 { "-" } else { "" };
    let (int, frac) = decimal_15(x.abs());
    let exp = if int.len() == 1 && int[0] == b'0' {
        -(frac.iter().take_while(|&&d| d == b'0').count() as i32) - 1
    } else {
        int.len() as i32 - 1
    };
    if (-5..=14).contains(&exp) {
        let mut s = String::from_utf8_lossy(&int).into_owned();
        if !frac.is_empty() {
            s.push(loc.decimal);
            s.push_str(&String::from_utf8_lossy(&frac));
        }
        return format!("{sinal}{s}");
    }
    // científico: d,dddE+XX
    let mut todos = int;
    todos.extend_from_slice(&frac);
    let zeros = todos.iter().take_while(|&&d| d == b'0').count();
    let mut sig: Vec<u8> = todos[zeros..].to_vec();
    while sig.last() == Some(&b'0') {
        sig.pop();
    }
    let mut mant = String::from_utf8_lossy(&sig[..1]).into_owned();
    if sig.len() > 1 {
        mant.push(loc.decimal);
        mant.push_str(&String::from_utf8_lossy(&sig[1..]));
    }
    format!("{sinal}{mant}E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
}

/// Uma seção numérica com o valor absoluto `x`.
fn numerico(toks: &[Tok], x: f64, loc: &Localidade) -> String {
    let mut x = x;
    for t in toks {
        if matches!(t, Tok::Pct) {
            x *= 100.0;
        }
    }
    // vírgulas depois do último algarismo dividem por mil cada
    let ultimo = toks.iter().rposition(|t| matches!(t, Tok::Dig(_) | Tok::Ponto));
    let escala = ultimo.map_or(0, |u| toks[u + 1..].iter().filter(|t| matches!(t, Tok::Milhar)).count());
    for _ in 0..escala {
        x /= 1000.0;
    }
    let toks: Vec<Tok> = toks
        .iter()
        .enumerate()
        .filter(|(i, t)| !(matches!(t, Tok::Milhar) && ultimo.is_some_and(|u| *i > u)))
        .map(|(_, t)| if matches!(t, Tok::Pct) { Tok::Lit("%".to_string()) } else { t.clone() })
        .collect();
    if toks.iter().any(|t| matches!(t, Tok::Geral)) {
        return toks.iter().map(|t| if matches!(t, Tok::Geral) { geral(x, loc) } else { lit(t) }).collect();
    }
    if toks.iter().any(|t| matches!(t, Tok::Exp(_))) {
        return cientifico(&toks, x, loc);
    }
    let ponto = toks.iter().position(|t| matches!(t, Tok::Ponto));
    let (parte_int, parte_dec): (&[Tok], &[Tok]) = match ponto {
        Some(p) => (&toks[..p], &toks[p + 1..]),
        None => (&toks[..], &[]),
    };
    let casas_max = parte_dec.iter().filter(|t| matches!(t, Tok::Dig(_))).count();
    let casas_min = parte_dec.iter().filter(|t| matches!(t, Tok::Dig('0'))).count();
    let (int, mut frac) = arredondar(x, casas_max);
    while frac.len() > casas_min && frac.ends_with('0') {
        frac.pop();
    }
    let tem_zero_obrigatorio = parte_int.iter().any(|t| matches!(t, Tok::Dig('0')));
    // sem `0` na parte inteira o zero não aparece (`#.##` escreve ".5", `#` escreve nada para 0)
    let int = if int == "0" && !tem_zero_obrigatorio { String::new() } else { int };
    let mut saida = inteiro_com_mascara(parte_int, &int, loc);
    let mut depois = String::new();
    if ponto.is_some() {
        if !frac.is_empty() {
            saida.push(loc.decimal);
            saida.push_str(&frac);
        }
        for t in parte_dec {
            if let Tok::Lit(l) = t {
                depois.push_str(l);
            }
        }
    }
    saida + &depois
}

fn lit(t: &Tok) -> String {
    match t {
        Tok::Lit(s) => s.clone(),
        _ => String::new(),
    }
}

/// Encaixa os algarismos nos `0 # ?`, da direita para a esquerda como o Excel: o que sobra de algarismos vai
/// para o placeholder mais à esquerda; `0` completa com zero, `?` com espaço e `#` com nada. O texto fixo entre
/// os placeholders só aparece se ainda houver algarismo ou `0` à esquerda dele.
fn inteiro_com_mascara(toks: &[Tok], int: &str, loc: &Localidade) -> String {
    let milhar = toks.iter().any(|t| matches!(t, Tok::Milhar));
    let mut toks: Vec<&Tok> = toks.iter().filter(|t| !matches!(t, Tok::Milhar)).collect();
    let implicito = Tok::Dig('#');
    if !toks.iter().any(|t| matches!(t, Tok::Dig(_))) && !int.is_empty() {
        toks.insert(0, &implicito);
    }
    let primeiro_dig = toks.iter().position(|t| matches!(t, Tok::Dig(_)));
    let ultimo_dig = toks.iter().rposition(|t| matches!(t, Tok::Dig(_)));
    let digitos: Vec<char> = int.chars().collect();
    let mut restam = digitos.len();
    let mut emitidos = 0usize;
    let mut saida: Vec<char> = Vec::new(); // de trás para frente
    let empurrar = |saida: &mut Vec<char>, d: char, emitidos: &mut usize, mais: bool| {
        saida.push(d);
        *emitidos += 1;
        if milhar && (*emitidos).is_multiple_of(3) && mais {
            saida.push(loc.milhar);
        }
    };
    for i in (0..toks.len()).rev() {
        match toks[i] {
            Tok::Dig(c) => {
                if restam > 0 {
                    if Some(i) == primeiro_dig {
                        // o placeholder mais à esquerda leva todos os algarismos que sobraram
                        while restam > 0 {
                            restam -= 1;
                            empurrar(&mut saida, digitos[restam], &mut emitidos, restam > 0);
                        }
                    } else {
                        restam -= 1;
                        let mais = restam > 0 || toks[..i].iter().any(|t| matches!(t, Tok::Dig('0' | '?')));
                        empurrar(&mut saida, digitos[restam], &mut emitidos, mais);
                    }
                } else {
                    match c {
                        '0' => {
                            let mais = toks[..i].iter().any(|t| matches!(t, Tok::Dig(_)));
                            empurrar(&mut saida, '0', &mut emitidos, mais);
                        }
                        '?' => saida.push(' '),
                        _ => {}
                    }
                }
            }
            Tok::Lit(l) => {
                let antes_de_tudo = primeiro_dig.is_none_or(|p| i < p);
                let depois_de_tudo = ultimo_dig.is_none_or(|u| i > u);
                let algo_a_esquerda = restam > 0 || toks[..i].iter().any(|t| matches!(t, Tok::Dig('0')));
                if antes_de_tudo || depois_de_tudo || algo_a_esquerda {
                    saida.extend(l.chars().rev());
                }
            }
            _ => {}
        }
    }
    saida.iter().rev().collect()
}
fn cientifico(toks: &[Tok], x: f64, loc: &Localidade) -> String {
    let pos_exp = toks.iter().position(|t| matches!(t, Tok::Exp(_))).unwrap_or(toks.len());
    let mais = matches!(toks.get(pos_exp), Some(Tok::Exp(true)));
    let (antes, depois) = toks.split_at(pos_exp);
    let ponto = antes.iter().position(|t| matches!(t, Tok::Ponto));
    let (parte_int, parte_dec): (&[Tok], &[Tok]) = match ponto {
        Some(p) => (&antes[..p], &antes[p + 1..]),
        None => (antes, &[]),
    };
    let k = parte_int.iter().filter(|t| matches!(t, Tok::Dig(_))).count().max(1);
    let casas = parte_dec.iter().filter(|t| matches!(t, Tok::Dig(_))).count();
    let casas_exp = depois.iter().filter(|t| matches!(t, Tok::Dig(_))).count().max(1);
    let prefixo: String = parte_int.iter().take_while(|t| !matches!(t, Tok::Dig(_))).map(lit).collect();
    let sufixo: String = depois.iter().map(lit).collect();

    // 15 algarismos significativos: d1 d2 d3… e o expoente decimal
    let (int, frac) = decimal_15(x);
    let mut todos = int.clone();
    todos.extend_from_slice(&frac);
    let (mut sig, mut expoente): (Vec<u8>, i32) = if x == 0.0 {
        (vec![b'0'; 15], 0)
    } else {
        let zeros = todos.iter().take_while(|&&d| d == b'0').count();
        let e = if int.len() == 1 && int[0] == b'0' { -(zeros as i32) } else { int.len() as i32 - 1 };
        (todos[zeros..].to_vec(), e)
    };
    sig.resize(k + casas + 1, b'0');
    let sobe = sig[k + casas] >= b'5';
    sig.truncate(k + casas);
    if sobe && somar_um(&mut sig) {
        sig.pop();
        expoente += 1;
    }
    expoente -= k as i32 - 1;
    let mut mant = String::from_utf8_lossy(&sig[..k]).into_owned();
    if casas > 0 {
        mant.push(loc.decimal);
        mant.push_str(&String::from_utf8_lossy(&sig[k..]));
    }
    let sinal = if expoente < 0 {
        "-"
    } else if mais {
        "+"
    } else {
        ""
    };
    format!("{prefixo}{mant}E{sinal}{:0>casas_exp$}{sufixo}", expoente.abs())
}

/// Fração `# ?/?`, `?/??`, `# ?/8`.
fn fracao(toks: &[Tok], x: f64) -> String {
    let Some(barra) = toks.iter().position(|t| matches!(t, Tok::Barra)) else {
        return numerico(toks, x, &Localidade::INVARIANTE);
    };
    let (esq, dir) = (&toks[..barra], &toks[barra + 1..]);
    let digitos_dir = dir.iter().filter(|t| matches!(t, Tok::Dig(_))).count();
    let fixo: Option<u32> = {
        let s: String = dir.iter().filter_map(|t| if let Tok::Lit(l) = t { Some(l.as_str()) } else { None }).collect();
        s.chars().filter(char::is_ascii_digit).collect::<String>().parse().ok()
    };
    let largura_den = fixo.map_or(digitos_dir, |f| f.to_string().len()).max(1);
    let den_max = fixo.unwrap_or_else(|| 10u32.pow(digitos_dir.clamp(1, 3) as u32) - 1);
    // dígitos do numerador: a última sequência de Dig antes da barra
    let fim_num = esq.iter().rposition(|t| matches!(t, Tok::Dig(_))).map_or(0, |p| p + 1);
    let ini_num = esq[..fim_num].iter().rposition(|t| !matches!(t, Tok::Dig(_))).map_or(0, |p| p + 1);
    let largura_num = (fim_num - ini_num).max(1);
    let tem_inteiro = esq[..ini_num].iter().any(|t| matches!(t, Tok::Dig(_)));
    let inteiro = if tem_inteiro { x.floor() } else { 0.0 };
    let resto = x - inteiro;
    let (num, den) = if let Some(f) = fixo {
        ((resto * f64::from(f)).round() as u64, u64::from(f))
    } else {
        let mut melhor = (0u64, 1u64);
        let mut erro = f64::MAX;
        for d in 1..=u64::from(den_max) {
            let n = (resto * d as f64).round() as u64;
            let e = (resto - n as f64 / d as f64).abs();
            if e < erro - 1e-12 {
                erro = e;
                melhor = (n, d);
            }
        }
        melhor
    };
    let (inteiro, num) = if tem_inteiro && num == den { (inteiro + 1.0, 0) } else { (inteiro, num) };
    let mut s = String::new();
    if tem_inteiro {
        // o zero à esquerda do `#` some, mas o espaço que separa a fração fica
        if inteiro != 0.0 || num == 0 {
            s.push_str(&format!("{}", inteiro as u64));
        }
        s.push(' ');
    }
    if num == 0 && tem_inteiro {
        s.push_str(&" ".repeat(largura_num + 1 + largura_den));
    } else if num == 0 && !tem_inteiro {
        s.push_str(&format!("{:>largura_num$}/{:<largura_den$}", 0, den));
    } else {
        s.push_str(&format!("{num:>largura_num$}/{den:<largura_den$}"));
    }
    s
}

// ---- datas e durações -----------------------------------------------------------------------------------

const MESES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];
const MESES_CURTOS: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
const DIAS: [&str; 7] =
    ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];
const DIAS_CURTOS: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
const MESES_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DIAS_EN: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

fn data_com_tokens(toks: &[Tok], d: Data, h: Hora, loc: &Localidade) -> String {
    let ampm = toks.iter().any(|t| matches!(t, Tok::AmPm(_)));
    let mut s = String::new();
    let mes_i = usize::from(d.mes.clamp(1, 12)) - 1;
    let dia_semana = usize::from(dia_da_semana(d));
    for t in toks {
        match t {
            Tok::Lit(l) => s.push_str(l),
            Tok::Ano(2) => s.push_str(&format!("{:02}", d.ano.rem_euclid(100))),
            Tok::Ano(_) => s.push_str(&format!("{:04}", d.ano)),
            Tok::Mes(1) => s.push_str(&d.mes.to_string()),
            Tok::Mes(2) => s.push_str(&format!("{:02}", d.mes)),
            Tok::Mes(3) => {
                let nome = if loc.portugues { MESES_CURTOS[mes_i] } else { &MESES_EN[mes_i][..3] };
                s.push_str(nome);
            }
            Tok::Mes(4) => s.push_str(if loc.portugues { MESES[mes_i] } else { MESES_EN[mes_i] }),
            Tok::Mes(_) => {
                let nome = if loc.portugues { MESES[mes_i] } else { MESES_EN[mes_i] };
                s.extend(nome.chars().next().map(|c| c.to_uppercase().next().unwrap_or(c)));
            }
            Tok::Dia(1) => s.push_str(&d.dia.to_string()),
            Tok::Dia(2) => s.push_str(&format!("{:02}", d.dia)),
            Tok::Dia(3) => {
                let nome = if loc.portugues { DIAS_CURTOS[dia_semana] } else { &DIAS_EN[dia_semana][..3] };
                s.push_str(nome);
            }
            Tok::Dia(_) => s.push_str(if loc.portugues { DIAS[dia_semana] } else { DIAS_EN[dia_semana] }),
            Tok::Hora(n) => {
                let hora = if ampm {
                    match h.hora % 12 {
                        0 => 12,
                        x => x,
                    }
                } else {
                    h.hora
                };
                if *n == 2 { s.push_str(&format!("{hora:02}")) } else { s.push_str(&hora.to_string()) }
            }
            Tok::Min(n) => {
                if *n == 2 {
                    s.push_str(&format!("{:02}", h.minuto))
                } else {
                    s.push_str(&h.minuto.to_string())
                }
            }
            Tok::Seg(n) => {
                if *n == 2 {
                    s.push_str(&format!("{:02}", h.segundo))
                } else {
                    s.push_str(&h.segundo.to_string())
                }
            }
            Tok::Frac(n) => {
                let tres = format!("{:03}", h.milissegundo);
                s.push(loc.decimal);
                s.push_str(&tres[..usize::from(*n).min(3)]);
            }
            Tok::AmPm(curto) => {
                let pm = h.hora >= 12;
                s.push_str(match (curto, pm) {
                    (false, false) => "AM",
                    (false, true) => "PM",
                    (true, false) => "A",
                    (true, true) => "P",
                });
            }
            _ => {}
        }
    }
    s
}

/// `[h]:mm:ss`, `[mm]:ss`, `[ss]` a partir da série (dias).
fn duracao(toks: &[Tok], serie: f64) -> String {
    let total_ms = (serie.abs() * 86_400_000.0).round() as u64;
    let primeira = toks.iter().find_map(|t| if let Tok::Decorrido(u, _) = t { Some(*u) } else { None }).unwrap_or('h');
    let (horas, minutos, segundos) = match primeira {
        'h' => (total_ms / 3_600_000, total_ms / 60_000 % 60, total_ms / 1000 % 60),
        'm' => (0, total_ms / 60_000, total_ms / 1000 % 60),
        _ => (0, 0, total_ms / 1000),
    };
    let mut s = if serie < 0.0 { "-".to_string() } else { String::new() };
    for t in toks {
        match t {
            Tok::Lit(l) => s.push_str(l),
            Tok::Decorrido(u, n) => {
                let v = match u {
                    'h' => horas,
                    'm' => minutos,
                    _ => segundos,
                };
                s.push_str(&format!("{v:0>w$}", w = usize::from(*n)));
            }
            Tok::Hora(n) => s.push_str(&format!("{:0>w$}", horas % 24, w = usize::from(*n))),
            Tok::Min(n) => {
                let m = if primeira == 'm' { minutos } else { minutos % 60 };
                s.push_str(&format!("{m:0>w$}", w = usize::from(*n)));
            }
            Tok::Seg(n) => {
                let sg = if primeira == 's' { segundos } else { segundos % 60 };
                s.push_str(&format!("{sg:0>w$}", w = usize::from(*n)));
            }
            Tok::Frac(n) => {
                let tres = format!("{:03}", total_ms % 1000);
                s.push('.');
                s.push_str(&tres[..usize::from(*n).min(3)]);
            }
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::datas::{SistemaDatas, serial_para_momento};
    use crate::valores::Momento;

    fn num(codigo: &str, x: f64) -> String {
        Formato::novo(codigo).exibir_com(&Valor::Numero(x), &Localidade::INVARIANTE)
    }

    fn br(codigo: &str, x: f64) -> String {
        Formato::novo(codigo).exibir(&Valor::Numero(x))
    }

    fn data(codigo: &str, serie: f64) -> String {
        let m = serial_para_momento(serie, SistemaDatas::D1900).unwrap();
        Formato::novo(codigo).exibir(&Valor::DataHora(m))
    }

    #[test]
    fn categorias_dos_formatos_embutidos() {
        let cat = |id| Formato::de_id(id, None).categoria();
        assert_eq!(cat(0), Categoria::Geral);
        assert_eq!(cat(1), Categoria::Numero);
        assert_eq!(cat(2), Categoria::Numero);
        assert_eq!(cat(3), Categoria::Numero);
        assert_eq!(cat(4), Categoria::Numero);
        assert_eq!(cat(9), Categoria::Percentual);
        assert_eq!(cat(10), Categoria::Percentual);
        assert_eq!(cat(11), Categoria::Cientifico);
        assert_eq!(cat(12), Categoria::Fracao);
        assert_eq!(cat(13), Categoria::Fracao);
        for id in [14, 15, 16, 17] {
            assert_eq!(cat(id), Categoria::Data, "{id}");
        }
        for id in [18, 19, 20, 21, 45, 47] {
            assert_eq!(cat(id), Categoria::Hora, "{id}");
        }
        assert_eq!(cat(22), Categoria::DataHora);
        assert_eq!(cat(46), Categoria::Duracao);
        assert_eq!(cat(48), Categoria::Cientifico);
        assert_eq!(cat(49), Categoria::Texto);
        for id in [37, 38, 39, 40, 41, 42, 43, 44] {
            assert_eq!(cat(id), Categoria::Numero, "{id}");
        }
        // id desconhecido sem código = General
        assert_eq!(cat(164), Categoria::Geral);
    }

    #[test]
    fn categorias_dos_formatos_personalizados() {
        let cat = |c: &str| Formato::novo(c).categoria();
        assert_eq!(cat("yyyy-mm-dd"), Categoria::Data);
        assert_eq!(cat("dd/mm/yyyy"), Categoria::Data);
        assert_eq!(cat("dd/mm/yy"), Categoria::Data);
        assert_eq!(cat("mmm-yy"), Categoria::Data);
        assert_eq!(cat("mmmm"), Categoria::Data);
        assert_eq!(cat("dddd"), Categoria::Data);
        assert_eq!(cat("[$-416]dddd\\, d \" de \"mmmm \" de \"yyyy;@"), Categoria::Data);
        assert_eq!(cat("yyyy-mm-dd hh:mm:ss"), Categoria::DataHora);
        assert_eq!(cat("dd/mm/yyyy\\ hh:mm"), Categoria::DataHora);
        assert_eq!(cat("hh:mm:ss"), Categoria::Hora);
        assert_eq!(cat("mm:ss"), Categoria::Hora);
        assert_eq!(cat("h:mm AM/PM"), Categoria::Hora);
        assert_eq!(cat("[h]:mm"), Categoria::Duracao);
        assert_eq!(cat("[mm]:ss"), Categoria::Duracao);
        assert_eq!(cat("0.00%"), Categoria::Percentual);
        assert_eq!(cat("0%"), Categoria::Percentual);
        assert_eq!(cat("0.0E+0"), Categoria::Cientifico);
        assert_eq!(cat("# ?/?"), Categoria::Fracao);
        assert_eq!(cat("@"), Categoria::Texto);
        assert_eq!(cat("General"), Categoria::Geral);
        assert_eq!(cat("0.00"), Categoria::Numero);
        assert_eq!(cat("#,##0.00_);[Red](#,##0.00)"), Categoria::Numero);
        assert_eq!(cat("\"R$\" #,##0.00"), Categoria::Numero);
        assert_eq!(cat("[$R$-416] #,##0.00"), Categoria::Numero);
        assert_eq!(cat("00000-000"), Categoria::Numero);
        // letras de data dentro de texto entre aspas não contam
        assert_eq!(cat("0.00 \"dias\""), Categoria::Numero);
        assert_eq!(cat("0 \"horas\""), Categoria::Numero);
        assert_eq!(cat("\\d0"), Categoria::Numero);
        // cores e condições não contam
        assert_eq!(cat("[Red]0.00"), Categoria::Numero);
        assert_eq!(cat("[>100]0;0.0"), Categoria::Numero);
    }

    #[test]
    fn numeros_simples() {
        assert_eq!(num("0", 3.6), "4");
        assert_eq!(num("0", 3.4), "3");
        assert_eq!(num("0", 0.5), "1");
        assert_eq!(num("0.00", 3.14158), "3.14");
        assert_eq!(num("0.00", 2.675), "2.68"); // como o Excel, não como o binário
        assert_eq!(num("0.00", 1.005), "1.01");
        assert_eq!(num("0.0", 0.05), "0.1");
        assert_eq!(num("0.00", 0.0), "0.00");
        assert_eq!(num("0.00", 9.999), "10.00");
        assert_eq!(num("#,##0", 1234567.0), "1,234,567");
        assert_eq!(num("#,##0", 999.0), "999");
        assert_eq!(num("#,##0", 1000.0), "1,000");
        assert_eq!(num("#,##0.00", 1234.5), "1,234.50");
        assert_eq!(num("#,##0.00", -1234.5), "-1,234.50");
        assert_eq!(num("#,##0.00", 0.126), "0.13");
        assert_eq!(num("0.0#", 1.25), "1.25");
        assert_eq!(num("0.0#", 1.2), "1.2");
        assert_eq!(num("#.##", 0.5), ".5");
        assert_eq!(num("0000", 7.0), "0007");
        assert_eq!(num("#", 0.0), "");
        assert_eq!(num("##0", 0.0), "0");
        assert_eq!(num("0", 123456789012.0), "123456789012");
    }

    #[test]
    fn localidade_brasileira() {
        assert_eq!(br("#,##0.00", 1234567.891), "1.234.567,89");
        assert_eq!(br("0.00", 0.5), "0,50");
        assert_eq!(br("General", 1234.5), "1234,5");
    }

    #[test]
    fn texto_fixo_e_moeda() {
        assert_eq!(br("\"R$\" #,##0.00", 1234.5), "R$ 1.234,50");
        assert_eq!(br("[$R$-416] #,##0.00", 1234.5), "R$ 1.234,50");
        assert_eq!(br("[$$-409]#,##0.00", 5.0), "$5,00");
        assert_eq!(br("0.00 \"kg\"", 2.5), "2,50 kg");
        assert_eq!(br("0\" dias\"", 3.0), "3 dias");
        assert_eq!(br("#,##0.00_);(#,##0.00)", 1234.5), "1.234,50 ");
    }

    #[test]
    fn mascaras_de_digitos() {
        assert_eq!(num("00000-000", 1310100.0), "01310-100");
        assert_eq!(num("00000-000", 12345678.0), "12345-678");
        assert_eq!(num("000\\.000\\.000\\-00", 12345678901.0), "123.456.789-01");
        assert_eq!(num("(00) 0000-0000", 1134567890.0), "(11) 3456-7890");
        assert_eq!(num("(00) 00000-0000", 11987654321.0), "(11) 98765-4321");
        // mais algarismos que posições: os que sobram vão para a esquerda
        assert_eq!(num("00-00", 123456.0), "1234-56");
        assert_eq!(num("0", 5.0), "5");
    }

    #[test]
    fn cpf_com_zeros_a_esquerda() {
        assert_eq!(num("000\\.000\\.000\\-00", 1234567.0), "000.012.345-67");
    }

    #[test]
    fn secoes_por_sinal_e_zero() {
        assert_eq!(num("0.00;(0.00)", -5.0), "(5.00)");
        assert_eq!(num("0.00;(0.00)", 5.0), "5.00");
        assert_eq!(num("0.00;(0.00)", 0.0), "0.00");
        assert_eq!(num("0.00;[Red]-0.00", -5.0), "-5.00");
        assert_eq!(num("0.00;-0.00;\"zero\"", 0.0), "zero");
        assert_eq!(num("0.00;-0.00;\"zero\"", 2.0), "2.00");
        assert_eq!(num("0.00;;", 0.0), "");
        assert_eq!(num("#,##0;-#,##0", -1500.0), "-1,500");
    }

    #[test]
    fn percentual() {
        assert_eq!(num("0%", 0.256), "26%");
        assert_eq!(num("0.00%", 0.256), "25.60%");
        assert_eq!(num("0.0%", 1.0), "100.0%");
        assert_eq!(num("0.00%", -0.05), "-5.00%");
        assert_eq!(br("0.0%", 0.1234), "12,3%");
    }

    #[test]
    fn cientifico() {
        assert_eq!(num("0.00E+00", 12345.0), "1.23E+04");
        assert_eq!(num("0.00E+00", 0.00012345), "1.23E-04");
        assert_eq!(num("0.0E+0", 99999.0), "1.0E+5");
        assert_eq!(num("0E+00", 5.0), "5E+00");
        assert_eq!(num("0.00E+00", 0.0), "0.00E+00");
        assert_eq!(num("0.00E-00", 12345.0), "1.23E04");
    }

    #[test]
    fn fracoes() {
        assert_eq!(num("# ?/?", 1.5), "1 1/2");
        assert_eq!(num("# ?/?", 0.25), " 1/4");
        assert_eq!(num("# ??/??", 3.14158), "3 14/99");
        assert_eq!(num("# ?/?", 2.0), "2    ");
        assert_eq!(num("?/?", 1.5), "3/2");
        assert_eq!(num("# ?/8", 1.3), "1 2/8");
        assert_eq!(num("# ?/?", -1.5), "-1 1/2");
    }

    #[test]
    fn geral_com_quinze_algarismos() {
        let g = |x: f64| Formato::geral().exibir_com(&Valor::Numero(x), &Localidade::INVARIANTE);
        assert_eq!(g(0.0), "0");
        assert_eq!(g(1.0), "1");
        assert_eq!(g(-42.0), "-42");
        assert_eq!(g(0.5), "0.5");
        assert_eq!(g(0.1 + 0.2), "0.3");
        assert_eq!(g(1.0 / 3.0), "0.333333333333333");
        assert_eq!(g(123456789012345.0), "123456789012345");
        assert_eq!(g(1234.5678), "1234.5678");
        assert_eq!(g(0.00001), "0.00001");
        assert_eq!(g(1e15), "1E+15");
        assert_eq!(g(1.5e20), "1.5E+20");
        assert_eq!(g(1.234e-8), "1.234E-08");
        assert_eq!(g(-2.5e-7), "-2.5E-07");
        assert_eq!(g(100.0), "100");
        assert_eq!(g(1e14), "100000000000000");
    }

    #[test]
    fn datas_com_nomes_em_portugues() {
        // 45000,5 = 2023-03-15 12:00, uma quarta-feira
        assert_eq!(data("dd/mm/yyyy", 45000.5), "15/03/2023");
        assert_eq!(data("yyyy-mm-dd", 45000.5), "2023-03-15");
        assert_eq!(data("d/m/yy", 45000.5), "15/3/23");
        assert_eq!(data("dd-mmm-yy", 45000.5), "15-mar-23");
        assert_eq!(data("mmmm", 45000.5), "março");
        assert_eq!(data("mmmmm", 45000.5), "M");
        assert_eq!(data("dddd", 45000.5), "quarta-feira");
        assert_eq!(data("ddd", 45000.5), "qua");
        assert_eq!(data("dddd, d \"de\" mmmm \"de\" yyyy", 45000.5), "quarta-feira, 15 de março de 2023");
        assert_eq!(data("[$-416]dddd\\, d\" de \"mmmm\" de \"yyyy", 45000.5), "quarta-feira, 15 de março de 2023");
        assert_eq!(data("mmm-yy", 45000.5), "mar-23");
        let ingles = Formato::novo("dddd, mmmm d").exibir_com(
            &Valor::DataHora(serial_para_momento(45000.5, SistemaDatas::D1900).unwrap()),
            &Localidade::INVARIANTE,
        );
        assert_eq!(ingles, "Wednesday, March 15");
    }

    #[test]
    fn horas_e_minutos_pelo_contexto() {
        assert_eq!(data("hh:mm:ss", 45000.604166666664), "14:30:00");
        assert_eq!(data("h:mm AM/PM", 45000.604166666664), "2:30 PM");
        assert_eq!(data("h:mm A/P", 45000.025), "12:36 A");
        assert_eq!(data("hh:mm:ss.000", 45000.5000115740), "12:00:01,000");
        assert_eq!(data("mm:ss", 45000.0104166667), "15:00");
        assert_eq!(data("yyyy-mm-dd hh:mm", 45000.5), "2023-03-15 12:00");
        assert_eq!(data("mm/dd/yyyy", 45000.5), "03/15/2023");
        assert_eq!(data("dd/mm/yyyy hh:mm", 45000.75), "15/03/2023 18:00");
        assert_eq!(data("h \"h\" mm \"min\"", 0.5625), "13 h 30 min");
    }

    #[test]
    fn datas_do_defeito_de_1900() {
        let f = Formato::novo("dd/mm/yyyy");
        let m = |s: f64| Valor::Data(serial_para_momento(s, SistemaDatas::D1900).unwrap().data);
        assert_eq!(f.exibir(&m(60.0)), "29/02/1900");
        assert_eq!(f.exibir(&m(1.0)), "01/01/1900");
        assert_eq!(f.exibir(&m(0.0)), "00/01/1900");
        assert_eq!(Formato::novo("dddd").exibir(&m(60.0)), "quarta-feira");
    }

    #[test]
    fn duracoes() {
        let d = |c: &str, x: f64| Formato::novo(c).exibir(&Valor::Numero(x));
        assert_eq!(d("[h]:mm:ss", 1.5), "36:00:00");
        assert_eq!(d("[h]:mm", 2.0 / 24.0 + 15.0 / 1440.0), "2:15");
        assert_eq!(d("[hh]:mm", 0.5), "12:00");
        assert_eq!(d("[mm]:ss", 0.01), "14:24");
        assert_eq!(d("[ss]", 0.001), "86");
        assert_eq!(d("[h]:mm:ss", -0.5), "-12:00:00");
        assert_eq!(d("[h]:mm:ss", 100.0), "2400:00:00");
    }

    #[test]
    fn outros_valores() {
        let f = Formato::novo("0.00");
        assert_eq!(f.exibir(&Valor::Vazio), "");
        assert_eq!(f.exibir(&Valor::Texto("abc".into())), "abc");
        assert_eq!(f.exibir(&Valor::Erro("#DIV/0!".into())), "#DIV/0!");
        assert_eq!(f.exibir(&Valor::Booleano(true)), "VERDADEIRO");
        assert_eq!(f.exibir(&Valor::Booleano(false)), "FALSO");
        assert_eq!(f.exibir_com(&Valor::Booleano(true), &Localidade::INVARIANTE), "TRUE");
        assert_eq!(f.exibir(&Valor::Numero(f64::NAN)), "#NUM!");
    }

    #[test]
    fn data_com_formato_de_numero_cai_no_iso() {
        // formato numérico aplicado a um valor de data: mostra ISO
        let f = Formato::novo("0.00");
        let d = Data { ano: 2024, mes: 3, dia: 5 };
        assert_eq!(f.exibir(&Valor::Data(d)), "2024-03-05");
        let h = Hora { hora: 1, minuto: 2, segundo: 3, milissegundo: 0 };
        assert_eq!(f.exibir(&Valor::Hora(h)), "01:02:03");
        assert_eq!(f.exibir(&Valor::DataHora(Momento { data: d, hora: h })), "2024-03-05 01:02:03");
    }

    #[test]
    fn numero_com_formato_de_data_fora_do_calendario_fica_geral() {
        assert_eq!(Formato::novo("dd/mm/yyyy").exibir(&Valor::Numero(-5.0)), "-5");
    }

    #[test]
    fn texto_e_general_literal() {
        assert_eq!(Formato::novo("@").exibir(&Valor::Numero(12.5)), "12,5");
        assert_eq!(Formato::novo("@").exibir(&Valor::Texto("x".into())), "x");
        assert_eq!(Formato::novo("General\" kg\"").exibir(&Valor::Numero(12.5)), "12,5 kg");
    }

    #[test]
    fn escala_por_milhares() {
        assert_eq!(num("#,##0,", 1234567.0), "1,235");
        assert_eq!(num("0.0,,\"M\"", 1_500_000.0), "1.5M");
    }
}
