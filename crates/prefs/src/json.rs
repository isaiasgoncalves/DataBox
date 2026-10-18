//! O JSON do `preferencias.json`: leitor tolerante e escritor com recuo de 2 espaços.
//!
//! [`ler`] aceita o arquivo com ou sem BOM. O que ele recusa vira `preferencias.json.corrompido` e tudo recomeça
//! do padrão, por isso é tolerante: aceita `NaN`, `Infinity` e `-Infinity`; número real fora da faixa vira
//! infinito (`1e400`) ou zero (`1e-400`); inteiro com mais de 4300 algarismos é recusado; e não há limite de
//! profundidade. As mensagens de erro (só vão para o log) dizem linha, coluna e posição. O leitor não usa
//! recursão: um arquivo com milhares de níveis não estoura a pilha.
//!
//! Um escape de surrogate solto (`"\ud800"`) vira U+FFFD, porque uma `String` não guarda surrogate; os contêineres
//! além de [`PROFUNDIDADE_GUARDADA`] níveis são conferidos por inteiro, mas não guardados (viram [`Json::Nulo`]): o
//! saneamento não olha além do 4º nível, e assim nenhum valor fundo chega às funções recursivas (`Drop`, `Clone`,
//! [`escrever`]) e a memória fica proporcional ao arquivo.
//!
//! [`Json`] guarda os objetos como lista de pares, na ordem do arquivo (no `historico_sql`, o banco mais antigo é o
//! primeiro, o que sai quando passa de 50); chave repetida fica na posição da primeira com o valor da última.
//! [`escrever`] grava com recuo de 2 espaços, sem escapar o que não é ASCII.

use std::collections::HashMap;

/// Até quantos níveis de contêiner o valor lido é guardado (o resto é conferido e descartado).
pub(crate) const PROFUNDIDADE_GUARDADA: usize = 64;

/// Inteiro mais longo que isso é recusado.
const MAX_ALGARISMOS_INTEIRO: usize = 4300;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Nulo,
    Bool(bool),
    Inteiro(i64),
    Real(f64),
    Texto(String),
    Lista(Vec<Json>),
    Objeto(Vec<(String, Json)>),
}

impl Json {
    /// O valor de uma chave de um objeto (None se não é objeto ou não tem a chave).
    pub(crate) fn campo(&self, nome: &str) -> Option<&Json> {
        match self {
            Json::Objeto(pares) => pares.iter().find(|(k, _)| k == nome).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn como_texto(&self) -> Option<&str> {
        match self {
            Json::Texto(t) => Some(t),
            _ => None,
        }
    }

    /// Quantos níveis de contêiner há no valor (0 num escalar, 1 em `[]`). Recursiva como `Drop` e [`escrever`]:
    /// o leitor não guarda mais que [`PROFUNDIDADE_GUARDADA`] níveis.
    pub(crate) fn profundidade(&self) -> usize {
        match self {
            Json::Lista(itens) => 1 + itens.iter().map(Json::profundidade).max().unwrap_or(0),
            Json::Objeto(pares) => 1 + pares.iter().map(|(_, v)| v.profundidade()).max().unwrap_or(0),
            _ => 0,
        }
    }
}

/// Lê o conteúdo do arquivo (UTF-8, com ou sem BOM). Err traz o motivo (para o log),
/// com a mensagem de erro quando o texto não é JSON ("Expecting value: line 1 column 1 (char 0)").
pub(crate) fn ler(conteudo: &[u8]) -> Result<Json, String> {
    let sem_bom = conteudo.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(conteudo);
    let texto = std::str::from_utf8(sem_bom).map_err(|e| format!("o arquivo não é UTF-8 válido: {e}"))?;
    Leitor { texto, bytes: texto.as_bytes(), pos: 0 }.documento()
}

// ---- leitura ----------------------------------------------------------------------------------------

/// Um objeto sendo lido: os pares até agora e a chave do valor que está sendo lido.
struct ObjetoAberto {
    pares: Vec<(String, Json)>,
    posicoes: HashMap<String, usize>,
    chave: String,
}

impl ObjetoAberto {
    fn novo(chave: String) -> ObjetoAberto {
        ObjetoAberto { pares: Vec::new(), posicoes: HashMap::new(), chave }
    }

    /// Chave repetida: fica na posição da primeira, com o valor da última.
    fn inserir(&mut self, valor: Json) {
        let chave = std::mem::take(&mut self.chave);
        match self.posicoes.get(&chave).and_then(|&i| self.pares.get_mut(i)) {
            Some(par) => par.1 = valor,
            None => {
                self.posicoes.insert(chave.clone(), self.pares.len());
                self.pares.push((chave, valor));
            }
        }
    }
}

/// Um contêiner guardado ainda aberto.
enum Aberto {
    Lista(Vec<Json>),
    Objeto(Box<ObjetoAberto>),
}

struct Leitor<'a> {
    texto: &'a str,
    bytes: &'a [u8],
    /// Posição em bytes; sempre num caractere ASCII ou no fim (fronteira de caractere).
    pos: usize,
}

impl Leitor<'_> {
    /// O erro: "mensagem: line L column C (char N)", com N contado em caracteres.
    fn erro(&self, mensagem: &str, pos: usize) -> String {
        let antes = self.texto.get(..pos).unwrap_or(self.texto);
        let caractere = antes.chars().count();
        let linha = antes.matches('\n').count() + 1;
        let coluna = match antes.rfind('\n') {
            Some(i) => antes.get(i..).map_or(0, |resto| resto.chars().count()),
            None => caractere + 1,
        };
        format!("{mensagem}: line {linha} column {coluna} (char {caractere})")
    }

    fn atual(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn comeca_com(&self, literal: &[u8]) -> bool {
        self.bytes.get(self.pos..).is_some_and(|resto| resto.starts_with(literal))
    }

    /// Espaço do JSON: só espaço, tabulação, quebra de linha e retorno de carro.
    fn pular_espacos(&mut self) {
        while matches!(self.atual(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    /// O `JSONDecoder.decode`: um valor, cercado só de espaços.
    fn documento(mut self) -> Result<Json, String> {
        if self.texto.starts_with('\u{feff}') {
            return Err(self.erro("Unexpected UTF-8 BOM (decode using utf-8-sig)", 0));
        }
        self.pular_espacos();
        let valor = self.valor()?;
        self.pular_espacos();
        if self.pos != self.bytes.len() {
            return Err(self.erro("Extra data", self.pos));
        }
        Ok(valor)
    }

    /// Um valor a partir de `pos` (espaços já pulados), sem recursão: os contêineres abertos ficam numa pilha.
    fn valor(&mut self) -> Result<Json, String> {
        // um item por nível aberto (true = objeto); guardados só os primeiros PROFUNDIDADE_GUARDADA
        let mut tipos: Vec<bool> = Vec::new();
        let mut abertos: Vec<Aberto> = Vec::new();
        loop {
            // começo de um valor
            let mut pronto = match self.atual() {
                Some(b'{') => {
                    self.pos += 1;
                    self.pular_espacos();
                    if self.atual() == Some(b'}') {
                        self.pos += 1;
                        Json::Objeto(Vec::new())
                    } else {
                        let chave = self.chave()?;
                        tipos.push(true);
                        if tipos.len() <= PROFUNDIDADE_GUARDADA {
                            abertos.push(Aberto::Objeto(Box::new(ObjetoAberto::novo(chave))));
                        }
                        continue;
                    }
                }
                Some(b'[') => {
                    self.pos += 1;
                    self.pular_espacos();
                    if self.atual() == Some(b']') {
                        self.pos += 1;
                        Json::Lista(Vec::new())
                    } else {
                        tipos.push(false);
                        if tipos.len() <= PROFUNDIDADE_GUARDADA {
                            abertos.push(Aberto::Lista(Vec::new()));
                        }
                        continue;
                    }
                }
                _ => self.escalar()?,
            };
            // um valor completo: entra no contêiner aberto, que pode fechar em seguida (e assim por diante)
            loop {
                let nivel = tipos.len();
                let Some(&objeto) = tipos.last() else {
                    return Ok(pronto);
                };
                let guardado = nivel <= PROFUNDIDADE_GUARDADA;
                if guardado {
                    match abertos.last_mut() {
                        Some(Aberto::Lista(itens)) => itens.push(pronto),
                        Some(Aberto::Objeto(o)) => o.inserir(pronto),
                        None => {}
                    }
                }
                self.pular_espacos();
                let virgula = self.pos;
                match self.atual() {
                    Some(b',') => {
                        self.pos += 1;
                        self.pular_espacos();
                        if objeto {
                            if self.atual() == Some(b'}') {
                                return Err(self.erro("Illegal trailing comma before end of object", virgula));
                            }
                            let chave = self.chave()?;
                            if guardado && let Some(Aberto::Objeto(o)) = abertos.last_mut() {
                                o.chave = chave;
                            }
                        } else if self.atual() == Some(b']') {
                            return Err(self.erro("Illegal trailing comma before end of array", virgula));
                        }
                        break; // o próximo valor do contêiner
                    }
                    Some(b'}') if objeto => self.pos += 1,
                    Some(b']') if !objeto => self.pos += 1,
                    _ => return Err(self.erro("Expecting ',' delimiter", self.pos)),
                }
                tipos.pop();
                pronto = if guardado {
                    match abertos.pop() {
                        Some(Aberto::Lista(itens)) => Json::Lista(itens),
                        Some(Aberto::Objeto(o)) => Json::Objeto(o.pares),
                        None => Json::Nulo,
                    }
                } else {
                    Json::Nulo // fundo demais: conferido, não guardado
                };
            }
        }
    }

    /// `"nome"` seguido de `:` (e dos espaços depois dele).
    fn chave(&mut self) -> Result<String, String> {
        if self.atual() != Some(b'"') {
            return Err(self.erro("Expecting property name enclosed in double quotes", self.pos));
        }
        let chave = self.texto_entre_aspas()?;
        self.pular_espacos();
        if self.atual() != Some(b':') {
            return Err(self.erro("Expecting ':' delimiter", self.pos));
        }
        self.pos += 1;
        self.pular_espacos();
        Ok(chave)
    }

    /// Texto, constante ou número (o que não é contêiner).
    fn escalar(&mut self) -> Result<Json, String> {
        if self.atual() == Some(b'"') {
            return self.texto_entre_aspas().map(Json::Texto);
        }
        let constantes: [(&[u8], Json); 6] = [
            (b"null", Json::Nulo),
            (b"true", Json::Bool(true)),
            (b"false", Json::Bool(false)),
            (b"NaN", Json::Real(f64::NAN)),
            (b"Infinity", Json::Real(f64::INFINITY)),
            (b"-Infinity", Json::Real(f64::NEG_INFINITY)),
        ];
        for (literal, valor) in constantes {
            if self.comeca_com(literal) {
                self.pos += literal.len();
                return Ok(valor);
            }
        }
        self.numero()
    }

    /// Número: `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][-+]?[0-9]+)?`; fração e expoente
    /// incompletos ficam de fora (e o que sobra dá erro adiante).
    fn numero(&mut self) -> Result<Json, String> {
        let inicio = self.pos;
        let algarismo = |i: usize| self.bytes.get(i).is_some_and(u8::is_ascii_digit);
        let mut i = inicio;
        if self.bytes.get(i) == Some(&b'-') {
            i += 1;
        }
        match self.bytes.get(i) {
            Some(b'1'..=b'9') => {
                i += 1;
                while algarismo(i) {
                    i += 1;
                }
            }
            Some(b'0') => i += 1,
            _ => return Err(self.erro("Expecting value", inicio)),
        }
        let mut real = false;
        if self.bytes.get(i) == Some(&b'.') && algarismo(i + 1) {
            real = true;
            i += 2;
            while algarismo(i) {
                i += 1;
            }
        }
        if matches!(self.bytes.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(self.bytes.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            let primeiro = j;
            while algarismo(j) {
                j += 1;
            }
            if j > primeiro {
                real = true;
                i = j;
            }
        }
        let literal = self.texto.get(inicio..i).unwrap_or_default();
        self.pos = i;
        if real {
            // real fora da faixa vira infinito; pequeno demais, zero
            return literal.parse::<f64>().map(Json::Real).map_err(|_| self.erro("Expecting value", inicio));
        }
        let algarismos = literal.trim_start_matches('-').len();
        if algarismos > MAX_ALGARISMOS_INTEIRO {
            return Err(format!(
                "Exceeds the limit ({MAX_ALGARISMOS_INTEIRO} digits) for integer string conversion: value has \
                 {algarismos} digits; use sys.set_int_max_str_digits() to increase the limit"
            ));
        }
        // o que não cabe em i64 fica como real (o saneamento não usa números)
        Ok(literal
            .parse::<i64>()
            .map_or_else(|_| Json::Real(literal.parse::<f64>().unwrap_or(f64::NAN)), Json::Inteiro))
    }

    /// Texto entre aspas (controles crus recusados), a partir da aspa de abertura.
    fn texto_entre_aspas(&mut self) -> Result<String, String> {
        let aspa = self.pos;
        let mut i = aspa + 1;
        let mut saida = String::new();
        loop {
            let trecho = i;
            while self.bytes.get(i).is_some_and(|&c| c != b'"' && c != b'\\' && c >= 0x20) {
                i += 1;
            }
            // `i` para num caractere ASCII ou no fim: o trecho é uma fatia inteira de caracteres
            saida.push_str(self.texto.get(trecho..i).unwrap_or_default());
            match self.bytes.get(i) {
                None => return Err(self.erro("Unterminated string starting at", aspa)),
                Some(b'"') => {
                    self.pos = i + 1;
                    return Ok(saida);
                }
                Some(b'\\') => {}
                Some(_) => return Err(self.erro("Invalid control character at", i)),
            }
            let simples = match self.bytes.get(i + 1) {
                None => return Err(self.erro("Unterminated string starting at", aspa)),
                Some(b'"') => '"',
                Some(b'\\') => '\\',
                Some(b'/') => '/',
                Some(b'b') => '\u{8}',
                Some(b'f') => '\u{c}',
                Some(b'n') => '\n',
                Some(b'r') => '\r',
                Some(b't') => '\t',
                Some(b'u') => {
                    let (caractere, depois) = self.escape_u(i)?;
                    saida.push(caractere);
                    i = depois;
                    continue;
                }
                Some(_) => return Err(self.erro("Invalid \\escape", i)),
            };
            saida.push(simples);
            i += 2;
        }
    }

    /// `\uXXXX` na posição `barra`: o caractere e a posição depois do escape. Um par de surrogates em dois
    /// escapes vira um caractere só; surrogate solto vira U+FFFD.
    fn escape_u(&self, barra: usize) -> Result<(char, usize), String> {
        let hex = |de: usize| -> Option<u32> {
            let digitos = self.bytes.get(de..de + 4)?;
            digitos.iter().try_fold(0u32, |n, &d| Some(n * 16 + char::from(d).to_digit(16)?))
        };
        let depois = barra + 6;
        // precisa caber ao menos mais um caractere (a aspa de fechamento) depois do escape
        if depois >= self.bytes.len() {
            return Err(self.erro("Invalid \\uXXXX escape", barra + 1));
        }
        let Some(codigo) = hex(barra + 2) else {
            return Err(self.erro("Invalid \\uXXXX escape", barra + 1));
        };
        if (0xD800..0xDC00).contains(&codigo)
            && depois + 6 < self.bytes.len()
            && self.bytes.get(depois..depois + 2) == Some(b"\\u")
            && let Some(baixo) = hex(depois + 2)
            && (0xDC00..0xE000).contains(&baixo)
        {
            let junto = 0x10000 + ((codigo - 0xD800) << 10) + (baixo - 0xDC00);
            return Ok((char::from_u32(junto).unwrap_or('\u{fffd}'), depois + 6));
        }
        Ok((char::from_u32(codigo).unwrap_or('\u{fffd}'), depois))
    }
}

// ---- escrita ----------------------------------------------------------------------------------------

/// O texto gravado: recuo de 2 espaços, sem escapar o que não é ASCII, sem quebra de linha no fim.
pub(crate) fn escrever(json: &Json) -> String {
    let mut saida = String::new();
    escrever_em(json, 0, &mut saida);
    saida
}

fn recuo(nivel: usize, saida: &mut String) {
    saida.push('\n');
    for _ in 0..nivel {
        saida.push_str("  ");
    }
}

fn escrever_em(json: &Json, nivel: usize, saida: &mut String) {
    match json {
        Json::Nulo => saida.push_str("null"),
        Json::Bool(true) => saida.push_str("true"),
        Json::Bool(false) => saida.push_str("false"),
        Json::Inteiro(n) => saida.push_str(&n.to_string()),
        Json::Real(r) => escrever_real(*r, saida),
        Json::Texto(t) => escrever_texto(t, saida),
        Json::Lista(itens) if itens.is_empty() => saida.push_str("[]"),
        Json::Lista(itens) => {
            saida.push('[');
            for (i, item) in itens.iter().enumerate() {
                if i > 0 {
                    saida.push(',');
                }
                recuo(nivel + 1, saida);
                escrever_em(item, nivel + 1, saida);
            }
            recuo(nivel, saida);
            saida.push(']');
        }
        Json::Objeto(pares) if pares.is_empty() => saida.push_str("{}"),
        Json::Objeto(pares) => {
            saida.push('{');
            for (i, (chave, valor)) in pares.iter().enumerate() {
                if i > 0 {
                    saida.push(',');
                }
                recuo(nivel + 1, saida);
                escrever_texto(chave, saida);
                saida.push_str(": ");
                escrever_em(valor, nivel + 1, saida);
            }
            recuo(nivel, saida);
            saida.push('}');
        }
    }
}

/// `float.__repr__` nos casos simples (as preferências nunca gravam número real; fica por completude).
fn escrever_real(r: f64, saida: &mut String) {
    if r.is_nan() {
        saida.push_str("NaN");
    } else if r.is_infinite() {
        saida.push_str(if r > 0.0 { "Infinity" } else { "-Infinity" });
    } else if r.fract() == 0.0 && r.abs() < 1e16 {
        saida.push_str(&format!("{r:.1}"));
    } else {
        saida.push_str(&format!("{r}"));
    }
}

/// Texto entre aspas com os escapes: `"` `\` e os controles
/// abaixo de 0x20 (`\b \f \n \r \t` curtos, o resto `\u00xx` em minúsculas); todo o resto vai como está.
fn escrever_texto(texto: &str, saida: &mut String) {
    saida.push('"');
    for c in texto.chars() {
        match c {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            '\u{8}' => saida.push_str("\\b"),
            '\u{c}' => saida.push_str("\\f"),
            c if u32::from(c) < 0x20 => saida.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => saida.push(c),
        }
    }
    saida.push('"');
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;

    #[test]
    fn chave_repetida_fica_na_primeira_posicao_com_o_ultimo_valor() {
        let j = ler(br#"{"b": 1, "a": 2, "b": 3}"#).unwrap();
        assert_eq!(j, Json::Objeto(vec![("b".into(), Json::Inteiro(3)), ("a".into(), Json::Inteiro(2))]));
    }

    #[test]
    fn escreve_com_recuo_de_dois() {
        let j = Json::Objeto(vec![
            ("versao".into(), Json::Inteiro(1)),
            ("vazio".into(), Json::Objeto(vec![])),
            ("lista".into(), Json::Lista(vec![])),
            (
                "x".into(),
                Json::Lista(vec![Json::Objeto(vec![
                    ("t".into(), Json::Texto("a\"b\\c\td\u{1}\u{7f}ç\u{2028}".into())),
                    ("n".into(), Json::Nulo),
                    ("b".into(), Json::Bool(false)),
                ])]),
            ),
        ]);
        let esperado = "{\n  \"versao\": 1,\n  \"vazio\": {},\n  \"lista\": [],\n  \"x\": [\n    {\n      \"t\": \
                        \"a\\\"b\\\\c\\td\\u0001\u{7f}ç\u{2028}\",\n      \"n\": null,\n      \"b\": false\n    }\n  ]\n}";
        assert_eq!(escrever(&j), esperado);
    }

    #[test]
    fn bom_e_utf8_invalido() {
        assert_eq!(ler(b"\xEF\xBB\xBF{\"tema\": \"dark\"}").unwrap().campo("tema"), Some(&Json::Texto("dark".into())));
        assert!(ler(b"\xff\xfe\x00lixo").is_err());
        assert!(ler(b"").is_err());
        assert!(ler(b"{} x").is_err());
        assert_eq!(ler(b" null ").unwrap(), Json::Nulo);
    }

    fn real(texto: &str) -> f64 {
        match ler(texto.as_bytes()).unwrap() {
            Json::Real(r) => r,
            outro => panic!("{texto:?} deu {outro:?}"),
        }
    }

    fn texto(json: &str) -> String {
        match ler(json.as_bytes()).unwrap() {
            Json::Texto(t) => t,
            outro => panic!("{json:?} deu {outro:?}"),
        }
    }

    #[test]
    fn aceita_as_constantes_e_os_numeros() {
        assert!(real("NaN").is_nan());
        assert_eq!(real("Infinity"), f64::INFINITY);
        assert_eq!(real(" -Infinity "), f64::NEG_INFINITY);
        assert_eq!(real("1e400"), f64::INFINITY);
        assert_eq!(real("-1e400"), f64::NEG_INFINITY);
        assert_eq!(real("1e999999999999"), f64::INFINITY);
        assert_eq!(real("1e-400"), 0.0);
        assert_eq!(real("1.0e-999999"), 0.0);
        assert_eq!(real(&format!("{}.5", "1".repeat(4301))), f64::INFINITY); // real não tem limite de algarismos
        assert_eq!(real("1E5"), 100000.0);
        assert_eq!(real("1e05"), 100000.0);
        assert_eq!(real("-0.0").to_bits(), (-0.0f64).to_bits());
        assert_eq!(real("123456789012345678901234567890"), 1.2345678901234568e29);
        assert_eq!(ler(b"-0").unwrap(), Json::Inteiro(0));
        assert_eq!(ler(b"9223372036854775807").unwrap(), Json::Inteiro(i64::MAX));
        assert_eq!(ler(b"-9223372036854775808").unwrap(), Json::Inteiro(i64::MIN));
        assert!(matches!(ler("1".repeat(4300).as_bytes()).unwrap(), Json::Real(_)));
        assert!(matches!(ler(format!("-{}", "1".repeat(4300)).as_bytes()).unwrap(), Json::Real(_)));
        let lista = ler(b"[NaN, Infinity,-Infinity , true,false,null]").unwrap();
        let Json::Lista(itens) = &lista else { panic!("{lista:?}") };
        assert_eq!(itens.len(), 6);
        assert_eq!(itens[3..], [Json::Bool(true), Json::Bool(false), Json::Nulo]);
    }

    #[test]
    fn escapes_e_surrogates() {
        // os escapes montados com format! para o texto do fonte não ter `\u` seguido de 4 dígitos hexadecimais
        let u = |hex: &str| format!("\\u{hex}");
        let e = format!(r#""a\"b\\c\/d\b\f\n\r\t{}{}""#, u("00e7"), u("00C7"));
        assert_eq!(texto(&e), "a\"b\\c/d\u{8}\u{c}\n\r\tçÇ");
        assert_eq!(texto(&format!("\"{}{}\"", u("d83d"), u("DE00"))), "\u{1f600}");
        assert_eq!(texto(&format!("\"{}\"", u("d800"))), "\u{fffd}");
        assert_eq!(texto(&format!("\"{}{}\"", u("dc00"), u("d800"))), "\u{fffd}\u{fffd}");
        assert_eq!(texto(&format!("\"{}{}\"", u("d800"), u("0041"))), "\u{fffd}A");
        assert_eq!(texto(&format!("\"{}{}{}\"", u("d800"), u("d800"), u("dc00"))), "\u{fffd}\u{10000}");
        assert_eq!(texto(&format!("\"x{}\"", u("dbff"))), "x\u{fffd}");
        assert_eq!(texto(&format!("\"{}{}\"", u("dbff"), u("dfff"))), "\u{10ffff}");
        assert_eq!(texto("\"a\u{7f}ç\u{2028}😀\""), "a\u{7f}ç\u{2028}😀");
    }

    /// Textos recusados, com a mensagem de cada um (só vão para o log).
    pub(crate) fn casos_recusados() -> Vec<(String, &'static str)> {
        let inteiro_grande = "Exceeds the limit (4300 digits) for integer string conversion: value has 4301 digits; \
                              use sys.set_int_max_str_digits() to increase the limit";
        vec![
            ("-NaN".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("+1".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("1.".into(), "Extra data: line 1 column 2 (char 1)"),
            ("1.e5".into(), "Extra data: line 1 column 2 (char 1)"),
            (".5".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("01".into(), "Extra data: line 1 column 2 (char 1)"),
            ("-".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("1e".into(), "Extra data: line 1 column 2 (char 1)"),
            ("1e+".into(), "Extra data: line 1 column 2 (char 1)"),
            ("[1,]".into(), "Illegal trailing comma before end of array: line 1 column 3 (char 2)"),
            ("{\"a\":1,}".into(), "Illegal trailing comma before end of object: line 1 column 7 (char 6)"),
            ("[1 2]".into(), "Expecting ',' delimiter: line 1 column 4 (char 3)"),
            ("{\"a\" 1}".into(), "Expecting ':' delimiter: line 1 column 6 (char 5)"),
            ("{1:2}".into(), "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)"),
            (r#""\ud800\uzzzz""#.into(), "Invalid \\uXXXX escape: line 1 column 9 (char 8)"),
            (r#""\x""#.into(), "Invalid \\escape: line 1 column 2 (char 1)"),
            (r#""\u12""#.into(), "Invalid \\uXXXX escape: line 1 column 3 (char 2)"),
            (r#""\U0041""#.into(), "Invalid \\escape: line 1 column 2 (char 1)"),
            ("\"a\u{1}\"".into(), "Invalid control character at: line 1 column 3 (char 2)"),
            ("".into(), "Expecting value: line 1 column 1 (char 0)"),
            (" ".into(), "Expecting value: line 1 column 2 (char 1)"),
            ("nul".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("[".into(), "Expecting value: line 1 column 2 (char 1)"),
            ("{".into(), "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)"),
            ("{\"a\"".into(), "Expecting ':' delimiter: line 1 column 5 (char 4)"),
            ("{\"a\":".into(), "Expecting value: line 1 column 6 (char 5)"),
            ("\"abc".into(), "Unterminated string starting at: line 1 column 1 (char 0)"),
            // o arquivo com dois BOMs: um sai, o outro é recusado
            ("\u{feff}\u{feff}{}".into(), "Unexpected UTF-8 BOM (decode using utf-8-sig): line 1 column 1 (char 0)"),
            ("{} x".into(), "Extra data: line 1 column 4 (char 3)"),
            ("[] ]".into(), "Extra data: line 1 column 4 (char 3)"),
            ("\u{a0}{}".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("\u{c}{}".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("tru".into(), "Expecting value: line 1 column 1 (char 0)"),
            ("true1".into(), "Extra data: line 1 column 5 (char 4)"),
            ("[true1]".into(), "Expecting ',' delimiter: line 1 column 6 (char 5)"),
            ("1 2".into(), "Extra data: line 1 column 3 (char 2)"),
            ("-Infinityx".into(), "Extra data: line 1 column 10 (char 9)"),
            ("[\"a\"\n,\n 1,\n x]".into(), "Expecting value: line 4 column 2 (char 12)"),
            ("-01".into(), "Extra data: line 1 column 3 (char 2)"),
            ("0x10".into(), "Extra data: line 1 column 2 (char 1)"),
            ("[.5]".into(), "Expecting value: line 1 column 2 (char 1)"),
            ("[-]".into(), "Expecting value: line 1 column 2 (char 1)"),
            ("[1]\u{0}".into(), "Extra data: line 1 column 4 (char 3)"),
            ("{\"ç\": [1 2]}".into(), "Expecting ',' delimiter: line 1 column 10 (char 9)"),
            ("1".repeat(4301), inteiro_grande),
            (format!("[-{}]", "1".repeat(4301)), inteiro_grande),
            (format!("{{\"a\": [{}1,]{}}}", "[".repeat(1000), "]".repeat(1000)), {
                "Illegal trailing comma before end of array: line 1 column 1009 (char 1008)"
            }),
        ]
    }

    #[test]
    fn recusa_com_a_mensagem() {
        for (json, mensagem) in casos_recusados() {
            assert_eq!(ler(json.as_bytes()), Err(mensagem.to_string()), "{json:?}");
        }
    }

    /// `[[[…]]]` com `n` níveis.
    fn listas(n: usize) -> String {
        format!("{}{}", "[".repeat(n), "]".repeat(n))
    }

    #[test]
    fn profundidade_sem_limite_e_sem_recursao() {
        // 200 níveis: o serde_json recusaria a partir de 128
        let j = ler(format!(r#"{{"tema": "dark", "x": {}, "y": {{"a": {}}}}}"#, listas(200), listas(3)).as_bytes())
            .unwrap();
        assert_eq!(j.campo("tema"), Some(&Json::Texto("dark".into())));
        assert_eq!(j.campo("y"), Some(&Json::Objeto(vec![("a".into(), ler(listas(3).as_bytes()).unwrap())])));
        // guardado até PROFUNDIDADE_GUARDADA níveis (o objeto de fora é o 1º, o "x" o 2º); o que passa disso foi
        // conferido e virou Nulo
        let mut nivel = j.campo("x").unwrap();
        for _ in 2..PROFUNDIDADE_GUARDADA {
            match nivel {
                Json::Lista(itens) if itens.len() == 1 => nivel = &itens[0],
                outro => panic!("{outro:?}"),
            }
        }
        assert_eq!(nivel, &Json::Lista(vec![Json::Nulo]));

        // um milhão de níveis (de listas e de objetos) num arquivo de poucos MB: nem pilha nem memória estouram
        assert_eq!(ler(listas(1_000_000).as_bytes()).unwrap(), {
            let mut v = Json::Lista(vec![Json::Nulo]);
            for _ in 1..PROFUNDIDADE_GUARDADA {
                v = Json::Lista(vec![v]);
            }
            v
        });
        let objetos = format!("{}1{}", r#"{"a": "#.repeat(1_000_000), "}".repeat(1_000_000));
        let objetos = ler(objetos.as_bytes()).unwrap();
        assert!(matches!(objetos, Json::Objeto(_)));
        // a profundidade do que foi guardado nunca passa do limite
        assert_eq!(objetos.profundidade(), PROFUNDIDADE_GUARDADA);
        assert_eq!(ler(listas(1_000_000).as_bytes()).unwrap().profundidade(), PROFUNDIDADE_GUARDADA);
        assert_eq!(ler(listas(3).as_bytes()).unwrap().profundidade(), 3);
        assert_eq!(ler(br#"{"a": [1, {}], "b": "x"}"#).unwrap().profundidade(), 3);
        assert_eq!(ler(b"[]").unwrap().profundidade(), 1);
        assert_eq!(ler(b"\"x\"").unwrap().profundidade(), 0);
        // e o que está errado lá no fundo continua sendo erro
        let quebrado = format!("{}1,]{}", "[".repeat(100_000), "]".repeat(99_999));
        assert_eq!(
            ler(quebrado.as_bytes()).unwrap_err(),
            "Illegal trailing comma before end of array: line 1 column 100002 (char 100001)"
        );
        assert!(ler(format!("{}]", listas(100_000)).as_bytes()).is_err());
        assert!(ler(listas(100_000).trim_end_matches(']').as_bytes()).is_err());
        let fundo_invalido = format!(r#"{}{{"a": "\x"}}{}"#, "[".repeat(1000), "]".repeat(1000));
        assert_eq!(ler(fundo_invalido.as_bytes()).unwrap_err(), "Invalid \\escape: line 1 column 1008 (char 1007)");
    }
}
