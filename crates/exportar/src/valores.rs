//! Regras de valores de cada formato (as funções `_valor_excel`, `_valor_csv`, `_valor_json`, `_texto_excel`,
//! …) e os nomes saneados (planilha, arquivo dentro do .zip, chave do JSON).

use std::borrow::Cow;
use std::collections::HashSet;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use comum::{Valor, formatar_real};

/// Caracteres que cabem numa célula do Excel; o resto é cortado.
pub const LIMITE_TEXTO_EXCEL: usize = 32_767;

/// O Excel guarda números em ponto flutuante e arredonda a partir do 16º dígito.
const MAIOR_INTEIRO_EXCEL: i64 = 999_999_999_999_999;
/// Faixa dos reais que o Excel representa; fora dela o valor vai como texto.
const MAIOR_REAL_EXCEL: f64 = 9.99999999999999e307;
const MENOR_REAL_EXCEL: f64 = 2.2250738585072014e-308;

/// Começo de texto que o Excel (e outras planilhas) avaliam como fórmula ao abrir um CSV.
const INICIO_FORMULA: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

const HEX: &[u8; 16] = b"0123456789abcdef";

// ---------------------------------------------------------------------------------------------------
// Números e avisos
// ---------------------------------------------------------------------------------------------------

/// "1.234.567": milhar com ponto.
pub(crate) fn numero(n: u64) -> String {
    let digitos = n.to_string();
    let mut saida = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(c);
    }
    saida
}

/// Conta as células que o Excel não comporta como estão, para o Resumo dizer quantas foram.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Avisos {
    pub textos_cortados: u64,
    pub blobs_grandes: u64,
    pub reais_como_texto: u64,
}

impl Avisos {
    /// As frases do Resumo.
    pub fn frases(&self) -> Vec<String> {
        fn um<'a>(n: u64, singular: &'a str, plural: &'a str) -> &'a str {
            if n == 1 { singular } else { plural }
        }
        let mut saida = Vec::new();
        let n = self.textos_cortados;
        if n > 0 {
            saida.push(format!(
                "{} {} de {} caracteres, o máximo do Excel, e {}.",
                numero(n),
                um(n, "célula de texto passou", "células de texto passaram"),
                numero(LIMITE_TEXTO_EXCEL as u64),
                um(n, "foi cortada", "foram cortadas")
            ));
        }
        let n = self.blobs_grandes;
        if n > 0 {
            saida.push(format!(
                "{} {} numa célula do Excel; {} só a indicação do tamanho. Para ter o conteúdo, exporte em CSV ou \
                 JSON ou use “Salvar em arquivo”.",
                numero(n),
                um(n, "conteúdo binário (BLOB) não coube", "conteúdos binários (BLOB) não couberam"),
                um(n, "ficou", "ficaram")
            ));
        }
        let n = self.reais_como_texto;
        if n > 0 {
            saida.push(format!(
                "{} {} fora da faixa do Excel e {} como texto.",
                numero(n),
                um(n, "número real ficou", "números reais ficaram"),
                um(n, "foi gravado", "foram gravados")
            ));
        }
        saida
    }
}

pub(crate) fn base64(dados: &[u8]) -> String {
    STANDARD.encode(dados)
}

// ---------------------------------------------------------------------------------------------------
// Excel
// ---------------------------------------------------------------------------------------------------

/// Caractere que o Excel recusa (os de controle que o openpyxl recusa) ou que o XML não aceita de jeito
/// nenhum (U+FFFE e U+FFFF; metades de par substituto não existem num `str` do Rust).
pub(crate) fn ilegal_excel(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}')
}

fn tem_ilegal(texto: &str) -> bool {
    let b = texto.as_bytes();
    b.iter().enumerate().any(|(i, &c)| {
        (c < 0x20 && c != b'\t' && c != b'\n' && c != b'\r')
            || (c == 0xEF && b.get(i + 1) == Some(&0xBF) && matches!(b.get(i + 2), Some(0xBE | 0xBF)))
    })
}

/// O texto que vai para a célula: sem os caracteres ilegais e cortado em [`LIMITE_TEXTO_EXCEL`] caracteres.
/// O `bool` diz se cortou (é o que o Resumo conta como "célula de texto cortada").
pub(crate) fn texto_excel(texto: &str) -> (Cow<'_, str>, bool) {
    if !tem_ilegal(texto) {
        if texto.len() <= LIMITE_TEXTO_EXCEL {
            return (Cow::Borrowed(texto), false);
        }
        return match texto.char_indices().nth(LIMITE_TEXTO_EXCEL) {
            None => (Cow::Borrowed(texto), false),
            Some((corte, _)) => (Cow::Borrowed(texto.get(..corte).unwrap_or(texto)), true),
        };
    }
    let mut saida = String::with_capacity(texto.len().min(LIMITE_TEXTO_EXCEL * 4));
    let mut cortado = false;
    for (n, c) in texto.chars().filter(|c| !ilegal_excel(*c)).enumerate() {
        if n == LIMITE_TEXTO_EXCEL {
            cortado = true;
            break;
        }
        saida.push(c);
    }
    (Cow::Owned(saida), cortado)
}

pub(crate) fn real_cabe_no_excel(valor: f64) -> bool {
    valor == 0.0 || (MENOR_REAL_EXCEL..=MAIOR_REAL_EXCEL).contains(&valor.abs())
}

/// O base64 de um BLOB deste tamanho passa do limite de uma célula.
pub(crate) fn blob_grande(tamanho: usize) -> bool {
    4 * tamanho.div_ceil(3) > LIMITE_TEXTO_EXCEL
}

/// Cortar o base64 daria um texto que não decodifica e nem guarda o começo inteiro: melhor dizer o que houve.
pub(crate) fn marcador_blob(tamanho: usize) -> String {
    format!(
        "[BLOB de {} bytes: grande demais para uma célula do Excel; exporte em CSV ou JSON para ter o conteúdo]",
        numero(tamanho as u64)
    )
}

/// O que vai numa célula do Excel. Texto vazio vira célula vazia (o Excel não distingue uma da outra).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CelulaExcel<'a> {
    Vazia,
    Inteiro(i64),
    Real(f64),
    /// Sempre gravado como texto: "=…" nunca vira fórmula e "#N/A" nunca vira erro.
    Texto(Cow<'a, str>),
}

impl CelulaExcel<'_> {
    /// A mesma célula sem empréstimo. É o que a amostra das larguras guarda: só o que vai para a planilha
    /// (texto já cortado em [`LIMITE_TEXTO_EXCEL`] caracteres, BLOB grande já trocado pelo marcador), no
    /// máximo uns 128 KB por célula, e nunca o valor inteiro que veio do banco.
    pub(crate) fn em_posse(self) -> CelulaExcel<'static> {
        match self {
            CelulaExcel::Vazia => CelulaExcel::Vazia,
            CelulaExcel::Inteiro(n) => CelulaExcel::Inteiro(n),
            CelulaExcel::Real(x) => CelulaExcel::Real(x),
            CelulaExcel::Texto(t) => CelulaExcel::Texto(Cow::Owned(t.into_owned())),
        }
    }
}

/// Valor pronto para a célula (o `_valor_excel`); com `avisos`, conta o que o Excel não comporta como está.
pub(crate) fn valor_excel<'a>(valor: &'a Valor, avisos: Option<&mut Avisos>) -> CelulaExcel<'a> {
    match valor {
        Valor::Nulo => CelulaExcel::Vazia,
        Valor::Inteiro(i) => {
            if (-MAIOR_INTEIRO_EXCEL..=MAIOR_INTEIRO_EXCEL).contains(i) {
                CelulaExcel::Inteiro(*i)
            } else {
                CelulaExcel::Texto(Cow::Owned(i.to_string()))
            }
        }
        Valor::Real(r) => {
            if r.is_nan() {
                CelulaExcel::Vazia
            } else if r.is_infinite() {
                CelulaExcel::Texto(Cow::Borrowed(if *r > 0.0 { "Infinity" } else { "-Infinity" }))
            } else if real_cabe_no_excel(*r) {
                CelulaExcel::Real(*r)
            } else {
                // fora da faixa do Excel (1,797…e308 seria relido como infinito) vai como texto
                if let Some(a) = avisos {
                    a.reais_como_texto += 1;
                }
                CelulaExcel::Texto(Cow::Owned(formatar_real(*r)))
            }
        }
        Valor::Texto(s) => {
            let (texto, cortado) = texto_excel(s);
            texto_celula(texto, cortado, avisos)
        }
        Valor::TextoBruto(b) => {
            let lido = String::from_utf8_lossy(b);
            let (texto, cortado) = texto_excel(&lido);
            texto_celula(Cow::Owned(texto.into_owned()), cortado, avisos)
        }
        Valor::Blob(b) => {
            if blob_grande(b.len()) {
                if let Some(a) = avisos {
                    a.blobs_grandes += 1;
                }
                CelulaExcel::Texto(Cow::Owned(marcador_blob(b.len())))
            } else if b.is_empty() {
                CelulaExcel::Vazia
            } else {
                CelulaExcel::Texto(Cow::Owned(base64(b)))
            }
        }
    }
}

fn texto_celula<'a>(texto: Cow<'a, str>, cortado: bool, avisos: Option<&mut Avisos>) -> CelulaExcel<'a> {
    if cortado && let Some(a) = avisos {
        a.textos_cortados += 1;
    }
    if texto.is_empty() { CelulaExcel::Vazia } else { CelulaExcel::Texto(texto) }
}

/// Caracteres que a célula ocupa na tela, para estimar a largura da coluna (o `_comprimento`).
pub(crate) fn comprimento(celula: &CelulaExcel<'_>) -> usize {
    match celula {
        CelulaExcel::Vazia => 0,
        CelulaExcel::Inteiro(i) => i.to_string().len(),
        // o formato Geral do Excel mostra no máximo uns 11 caracteres
        CelulaExcel::Real(r) => formatar_real(*r).len().min(12),
        CelulaExcel::Texto(t) => {
            if t.contains('\n') {
                t.split('\n').map(|linha| linha.chars().count()).max().unwrap_or(0)
            } else {
                t.chars().count()
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// CSV
// ---------------------------------------------------------------------------------------------------

/// Real como texto no CSV: o texto mais curto que volta ao mesmo número, com vírgula decimal se pedida; infinito por extenso; NaN vazio.
pub(crate) fn real_texto(valor: f64, virgula: bool) -> String {
    if valor.is_nan() {
        return String::new();
    }
    if valor.is_infinite() {
        return if valor > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let texto = formatar_real(valor);
    if virgula { texto.replace('.', ",") } else { texto }
}

/// `^[+-]?[0-9]+(?:[.,][0-9]+)?$` (um "\n" no fim também é aceito).
fn parece_numero(texto: &str) -> bool {
    let t = texto.strip_suffix('\n').unwrap_or(texto);
    let t = t.strip_prefix(['+', '-']).unwrap_or(t);
    let digitos = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match t.split_once(['.', ',']) {
        Some((inteira, fracao)) => digitos(inteira) && digitos(fracao),
        None => digitos(t),
    }
}

/// Texto que o Excel executaria como fórmula ao abrir o CSV (OWASP). Um número guardado como texto
/// ("-5", "+1,5") não é fórmula.
pub(crate) fn precisa_protecao(texto: &str) -> bool {
    texto.starts_with(INICIO_FORMULA) && !parece_numero(texto)
}

/// O campo do CSV (o `_valor_csv`), antes das aspas do formato.
pub(crate) fn valor_csv(valor: &Valor, virgula: bool, proteger: bool) -> Cow<'_, str> {
    let texto = match valor {
        Valor::Nulo => return Cow::Borrowed(""),
        Valor::Inteiro(i) => return Cow::Owned(i.to_string()),
        Valor::Real(r) => return Cow::Owned(real_texto(*r, virgula)),
        Valor::Blob(b) => return Cow::Owned(base64(b)),
        Valor::Texto(s) => Cow::Borrowed(s.as_str()),
        Valor::TextoBruto(b) => String::from_utf8_lossy(b),
    };
    if proteger && precisa_protecao(&texto) { Cow::Owned(format!("'{texto}")) } else { texto }
}

/// Nome de arquivo aceito pelo Windows e único (sem diferenciar maiúsculas); entra em `usados`.
pub(crate) fn nome_arquivo(nome: &str, usados: &mut HashSet<String>, extensao: &str) -> String {
    let limpo: String = nome
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\u{0}'..='\u{1f}') { '_' } else { c }
        })
        .collect();
    let base: String = limpo.trim().trim_end_matches(['.', ' ']).chars().take(120).collect();
    let mut base = base.trim_end_matches(['.', ' ']).to_string();
    if base.is_empty() {
        base = "tabela".to_string();
    }
    let primeiro = base.split('.').next().unwrap_or_default().trim_end().to_lowercase();
    if reservado_windows(&primeiro) {
        base = format!("_{base}");
    }
    let ocupados: HashSet<String> = usados.iter().map(|u| u.to_lowercase()).collect();
    let mut candidato = format!("{base}{extensao}");
    let mut n = 2u64;
    while ocupados.contains(&candidato.to_lowercase()) {
        candidato = format!("{base} ({n}){extensao}");
        n += 1;
    }
    usados.insert(candidato.clone());
    candidato
}

fn reservado_windows(nome: &str) -> bool {
    match nome {
        "con" | "prn" | "aux" | "nul" => true,
        _ => {
            let mut letras = nome.chars();
            let prefixo: String = letras.by_ref().take(3).collect();
            let resto: String = letras.collect();
            (prefixo == "com" || prefixo == "lpt")
                && matches!(resto.as_str(), "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------------------------------

/// Texto em JSON como o `json.dumps(..., ensure_ascii=False)`: só aspas, barra invertida e os caracteres de
/// controle são escapados (estes em `\u00xx`, minúsculo, salvo os curtos `\b \f \n \r \t`).
pub(crate) fn texto_json(saida: &mut Vec<u8>, texto: &str) {
    saida.push(b'"');
    let b = texto.as_bytes();
    let mut inicio = 0;
    for (i, &c) in b.iter().enumerate() {
        if c >= 0x20 && c != b'"' && c != b'\\' {
            continue;
        }
        saida.extend_from_slice(b.get(inicio..i).unwrap_or_default());
        match c {
            b'"' => saida.extend_from_slice(b"\\\""),
            b'\\' => saida.extend_from_slice(b"\\\\"),
            b'\n' => saida.extend_from_slice(b"\\n"),
            b'\r' => saida.extend_from_slice(b"\\r"),
            b'\t' => saida.extend_from_slice(b"\\t"),
            0x08 => saida.extend_from_slice(b"\\b"),
            0x0c => saida.extend_from_slice(b"\\f"),
            _ => {
                saida.extend_from_slice(b"\\u00");
                saida.push(HEX[usize::from(c >> 4)]);
                saida.push(HEX[usize::from(c & 0x0f)]);
            }
        }
        inicio = i + 1;
    }
    saida.extend_from_slice(b.get(inicio..).unwrap_or_default());
    saida.push(b'"');
}

/// Valor em JSON (o `_valor_json`): NULL → null, BLOB → base64, infinito → "Infinity", NaN → null.
pub(crate) fn valor_json(saida: &mut Vec<u8>, valor: &Valor) {
    match valor {
        Valor::Nulo => saida.extend_from_slice(b"null"),
        Valor::Inteiro(i) => saida.extend_from_slice(i.to_string().as_bytes()),
        Valor::Real(r) => {
            if r.is_nan() {
                saida.extend_from_slice(b"null");
            } else if r.is_infinite() {
                saida.extend_from_slice(if *r > 0.0 { b"\"Infinity\"" } else { b"\"-Infinity\"" });
            } else {
                saida.extend_from_slice(formatar_real(*r).as_bytes());
            }
        }
        Valor::Texto(s) => texto_json(saida, s),
        Valor::TextoBruto(b) => texto_json(saida, &String::from_utf8_lossy(b)),
        Valor::Blob(b) => {
            saida.push(b'"');
            saida.extend_from_slice(base64(b).as_bytes());
            saida.push(b'"');
        }
    }
}

/// Nomes de coluna repetidos (SELECT a, a) ganham "_2", "_3"… sem tomar o nome de outra coluna real.
pub(crate) fn chaves_unicas(colunas: &[String]) -> Vec<String> {
    let mut reservados: HashSet<String> = colunas.iter().cloned().collect();
    let mut vistos: HashSet<&str> = HashSet::new();
    let mut chaves = Vec::with_capacity(colunas.len());
    for nome in colunas {
        let mut chave = nome.clone();
        if vistos.contains(nome.as_str()) {
            let mut n = 2u64;
            while reservados.contains(&format!("{nome}_{n}")) {
                n += 1;
            }
            chave = format!("{nome}_{n}");
            reservados.insert(chave.clone());
        }
        vistos.insert(nome.as_str());
        chaves.push(chave);
    }
    chaves
}

/// Chave ainda não usada no objeto "tabelas" do JSON do banco ("clientes", "clientes_2"…).
pub(crate) fn chave_livre(nome: &str, usados: &mut HashSet<String>) -> String {
    let mut chave = nome.to_string();
    let mut n = 2u64;
    while usados.contains(&chave) {
        chave = format!("{nome}_{n}");
        n += 1;
    }
    usados.insert(chave.clone());
    chave
}

// ---------------------------------------------------------------------------------------------------
// Texto para mensagens
// ---------------------------------------------------------------------------------------------------

/// O texto curto entre aspas simples, com escapes (vai no `detalhe` dos erros de opção).
pub(crate) fn citado(texto: &str) -> String {
    let aspa = if texto.contains('\'') && !texto.contains('"') { '"' } else { '\'' };
    let mut saida = String::with_capacity(texto.len() + 2);
    saida.push(aspa);
    for c in texto.chars() {
        match c {
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if c == aspa => {
                saida.push('\\');
                saida.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => saida.push_str(&format!("\\x{:02x}", c as u32)),
            c => saida.push(c),
        }
    }
    saida.push(aspa);
    saida
}
