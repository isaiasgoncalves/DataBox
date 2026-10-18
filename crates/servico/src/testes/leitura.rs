//! Leitores mínimos para conferir os arquivos exportados sem dependências novas (o crate não tem zip nem
//! serde_json): DEFLATE (RFC 1951) e .zip, JSON, CSV, células do .xlsx e base64.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---- DEFLATE ------------------------------------------------------------------------------------------

struct Bits<'a> {
    dados: &'a [u8],
    pos: usize,
    buf: u64,
    n: u32,
}

impl Bits<'_> {
    fn bits(&mut self, quantos: u32) -> u32 {
        while self.n < quantos {
            let byte = *self.dados.get(self.pos).expect("DEFLATE: dados acabaram no meio");
            self.pos += 1;
            self.buf |= u64::from(byte) << self.n;
            self.n += 8;
        }
        let v = (self.buf & ((1u64 << quantos) - 1)) as u32;
        self.buf >>= quantos;
        self.n -= quantos;
        v
    }

    fn alinhar(&mut self) {
        let sobra = self.n % 8;
        self.bits(sobra);
    }
}

/// Código de Huffman canônico: quantos códigos de cada tamanho e os símbolos em ordem.
struct Huffman {
    contagem: [u16; 16],
    simbolos: Vec<u16>,
}

impl Huffman {
    fn novo(tamanhos: &[u8]) -> Huffman {
        let mut contagem = [0u16; 16];
        for &t in tamanhos {
            contagem[usize::from(t)] += 1;
        }
        contagem[0] = 0;
        let mut inicio = [0u16; 16];
        for i in 1..16 {
            inicio[i] = inicio[i - 1] + contagem[i - 1];
        }
        let mut simbolos = vec![0u16; tamanhos.len()];
        for (s, &t) in tamanhos.iter().enumerate() {
            if t != 0 {
                simbolos[usize::from(inicio[usize::from(t)])] = s as u16;
                inicio[usize::from(t)] += 1;
            }
        }
        Huffman { contagem, simbolos }
    }

    fn decodificar(&self, b: &mut Bits<'_>) -> u16 {
        let (mut codigo, mut primeiro, mut indice) = (0i32, 0i32, 0i32);
        for tamanho in 1..16 {
            codigo |= b.bits(1) as i32;
            let n = i32::from(self.contagem[tamanho]);
            if codigo - n < primeiro {
                return self.simbolos[(indice + (codigo - primeiro)) as usize];
            }
            indice += n;
            primeiro = (primeiro + n) << 1;
            codigo <<= 1;
        }
        panic!("DEFLATE: código inválido");
    }
}

const BASE_TAM: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const EXTRA_TAM: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const BASE_DIST: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const EXTRA_DIST: [u32; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn bloco(b: &mut Bits<'_>, saida: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) {
    loop {
        let s = usize::from(lit.decodificar(b));
        if s < 256 {
            saida.push(s as u8);
        } else if s == 256 {
            return;
        } else {
            let i = s - 257;
            let tamanho = usize::from(BASE_TAM[i]) + b.bits(EXTRA_TAM[i]) as usize;
            let d = usize::from(dist.decodificar(b));
            let distancia = usize::from(BASE_DIST[d]) + b.bits(EXTRA_DIST[d]) as usize;
            let inicio = saida.len() - distancia;
            for k in 0..tamanho {
                let byte = saida[inicio + k];
                saida.push(byte);
            }
        }
    }
}

/// Descomprime um fluxo DEFLATE cru (o método 8 do zip).
pub(crate) fn inflar(dados: &[u8]) -> Vec<u8> {
    let mut b = Bits { dados, pos: 0, buf: 0, n: 0 };
    let mut saida = Vec::new();
    loop {
        let ultimo = b.bits(1) == 1;
        match b.bits(2) {
            0 => {
                b.alinhar();
                let tamanho = b.bits(16) as usize;
                let _complemento = b.bits(16);
                for _ in 0..tamanho {
                    saida.push(b.bits(8) as u8);
                }
            }
            1 => {
                let mut tamanhos = [0u8; 288];
                for (i, t) in tamanhos.iter_mut().enumerate() {
                    *t = match i {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                bloco(&mut b, &mut saida, &Huffman::novo(&tamanhos), &Huffman::novo(&[5u8; 30]));
            }
            2 => {
                let nlit = b.bits(5) as usize + 257;
                let ndist = b.bits(5) as usize + 1;
                let ncod = b.bits(4) as usize + 4;
                const ORDEM: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut tam_cod = [0u8; 19];
                for &o in ORDEM.iter().take(ncod) {
                    tam_cod[o] = b.bits(3) as u8;
                }
                let codigos = Huffman::novo(&tam_cod);
                let mut tamanhos = Vec::with_capacity(nlit + ndist);
                while tamanhos.len() < nlit + ndist {
                    match codigos.decodificar(&mut b) {
                        s @ 0..=15 => tamanhos.push(s as u8),
                        16 => {
                            let anterior = *tamanhos.last().expect("DEFLATE: repetição sem anterior");
                            for _ in 0..3 + b.bits(2) {
                                tamanhos.push(anterior);
                            }
                        }
                        17 => tamanhos.extend(std::iter::repeat_n(0, 3 + b.bits(3) as usize)),
                        _ => tamanhos.extend(std::iter::repeat_n(0, 11 + b.bits(7) as usize)),
                    }
                }
                let lit = Huffman::novo(&tamanhos[..nlit]);
                let dist = Huffman::novo(&tamanhos[nlit..]);
                bloco(&mut b, &mut saida, &lit, &dist);
            }
            _ => panic!("DEFLATE: tipo de bloco inválido"),
        }
        if ultimo {
            return saida;
        }
    }
}

// ---- .zip ---------------------------------------------------------------------------------------------

fn u16_em(d: &[u8], p: usize) -> usize {
    usize::from(u16::from_le_bytes([d[p], d[p + 1]]))
}

fn u32_em(d: &[u8], p: usize) -> u64 {
    u64::from(u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]))
}

fn u64_em(d: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(d[p..p + 8].try_into().unwrap())
}

/// As partes de um .zip, na ordem do diretório central: (nome, conteúdo descomprimido).
pub(crate) fn ler_zip(caminho: &Path) -> Vec<(String, Vec<u8>)> {
    let d = fs::read(caminho).unwrap();
    let fim = (0..=d.len() - 22).rev().find(|&p| u32_em(&d, p) == 0x0605_4b50).expect("zip sem o fim do diretório");
    let total = u16_em(&d, fim + 10);
    let mut p = u32_em(&d, fim + 16) as usize;
    let mut partes = Vec::new();
    for _ in 0..total {
        assert_eq!(u32_em(&d, p), 0x0201_4b50, "entrada do diretório central");
        let metodo = u16_em(&d, p + 10);
        let mut comprimido = u32_em(&d, p + 20);
        let mut tamanho = u32_em(&d, p + 24);
        let n_nome = u16_em(&d, p + 28);
        let n_extra = u16_em(&d, p + 30);
        let n_coment = u16_em(&d, p + 32);
        let mut local = u32_em(&d, p + 42);
        let nome = String::from_utf8(d[p + 46..p + 46 + n_nome].to_vec()).unwrap();
        // ZIP64: os campos que valem 0xFFFFFFFF vêm no extra 0x0001, nessa ordem
        let mut e = p + 46 + n_nome;
        let fim_extra = e + n_extra;
        while e + 4 <= fim_extra {
            let (id, n) = (u16_em(&d, e), u16_em(&d, e + 2));
            if id == 1 {
                let mut q = e + 4;
                for campo in [&mut tamanho, &mut comprimido, &mut local] {
                    if *campo == 0xFFFF_FFFF {
                        *campo = u64_em(&d, q);
                        q += 8;
                    }
                }
            }
            e += 4 + n;
        }
        let l = local as usize;
        assert_eq!(u32_em(&d, l), 0x0403_4b50, "cabeçalho local");
        let inicio = l + 30 + u16_em(&d, l + 26) + u16_em(&d, l + 28);
        let bruto = &d[inicio..inicio + comprimido as usize];
        let conteudo = match metodo {
            0 => bruto.to_vec(),
            8 => inflar(bruto),
            outro => panic!("método de compressão {outro} não suportado"),
        };
        assert_eq!(conteudo.len() as u64, tamanho, "tamanho de {nome}");
        partes.push((nome, conteudo));
        p += 46 + n_nome + n_extra + n_coment;
    }
    partes
}

// ---- .xlsx --------------------------------------------------------------------------------------------

fn desescapar(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#13;", "\r").replace("&amp;", "&")
}

fn atributo(tag: &str, nome: &str) -> Option<String> {
    let chave = format!(" {nome}=\"");
    let inicio = tag.find(&chave)? + chave.len();
    let fim = tag[inicio..].find('"')? + inicio;
    Some(desescapar(&tag[inicio..fim]))
}

/// Uma planilha do .xlsx lido: nome, XML e as células (referência → (tipo, valor)).
pub(crate) struct Planilha {
    pub nome: String,
    pub xml: String,
    pub celulas: HashMap<String, (Option<String>, String)>,
}

impl Planilha {
    /// Texto da célula (inlineStr); None se a célula não existe.
    pub fn texto(&self, referencia: &str) -> Option<String> {
        let (tipo, valor) = self.celulas.get(referencia)?;
        assert_eq!(tipo.as_deref(), Some("inlineStr"), "{referencia} devia ser texto");
        Some(valor.clone())
    }

    /// Número da célula (sem tipo), como está no XML.
    pub fn numero(&self, referencia: &str) -> Option<String> {
        let (tipo, valor) = self.celulas.get(referencia)?;
        assert_eq!(*tipo, None, "{referencia} devia ser número");
        Some(valor.clone())
    }

    /// Linhas no XML (`<row `), contando o cabeçalho.
    pub fn linhas(&self) -> usize {
        self.xml.matches("<row ").count()
    }

    /// O `ref` do autoFilter.
    pub fn filtro(&self) -> Option<String> {
        let p = self.xml.find("<autoFilter ")?;
        atributo(&self.xml[p..], "ref")
    }
}

fn celulas(xml: &str) -> HashMap<String, (Option<String>, String)> {
    let mut saida = HashMap::new();
    let mut resto = xml;
    while let Some(p) = resto.find("<c r=\"") {
        resto = &resto[p..];
        let fim_tag = resto.find('>').unwrap();
        let tag = &resto[..fim_tag];
        let referencia = atributo(tag, "r").unwrap();
        let tipo = atributo(tag, "t");
        let (valor, avanco) = if tag.ends_with('/') {
            (String::new(), fim_tag + 1)
        } else {
            let fecha = resto.find("</c>").unwrap();
            let dentro = &resto[fim_tag + 1..fecha];
            let valor = if let Some(v) = dentro.strip_prefix("<v>") {
                v.trim_end_matches("</v>").to_string()
            } else if let Some(p) = dentro.find("<t") {
                let abre = p + dentro[p..].find('>').unwrap() + 1;
                desescapar(&dentro[abre..dentro.find("</t>").unwrap()])
            } else {
                String::new()
            };
            (valor, fecha + 4)
        };
        saida.insert(referencia, (tipo, valor));
        resto = &resto[avanco..];
    }
    saida
}

/// As planilhas do .xlsx, na ordem do livro.
pub(crate) fn ler_xlsx(caminho: &Path) -> Vec<Planilha> {
    let partes: HashMap<String, Vec<u8>> = ler_zip(caminho).into_iter().collect();
    let livro = String::from_utf8(partes["xl/workbook.xml"].clone()).unwrap();
    let nomes: Vec<String> =
        livro.split("<sheet ").skip(1).map(|s| atributo(&format!(" {s}"), "name").unwrap()).collect();
    nomes
        .into_iter()
        .enumerate()
        .map(|(i, nome)| {
            let xml = String::from_utf8(partes[&format!("xl/worksheets/sheet{}.xml", i + 1)].clone()).unwrap();
            Planilha { nome, celulas: celulas(&xml), xml }
        })
        .collect()
}

/// "A", "B"… "AA" da coluna (a partir de 1).
pub(crate) fn letra_coluna(mut n: usize) -> String {
    let mut s = Vec::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.push(b'A' + r as u8);
        n = (n - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

// ---- CSV ----------------------------------------------------------------------------------------------

/// Leitor de CSV (aspas, aspas dobradas, quebras dentro de aspas).
pub(crate) fn ler_csv_texto(texto: &str, separador: char) -> Vec<Vec<String>> {
    let mut registros = Vec::new();
    let mut registro = Vec::new();
    let mut campo = String::new();
    let mut aspas = false;
    let mut pendente = false;
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if aspas {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    campo.push('"');
                } else {
                    aspas = false;
                }
            } else {
                campo.push(c);
            }
        } else if c == '"' {
            aspas = true;
            pendente = true;
        } else if c == separador {
            registro.push(std::mem::take(&mut campo));
            pendente = true;
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            registro.push(std::mem::take(&mut campo));
            registros.push(std::mem::take(&mut registro));
            pendente = false;
        } else {
            campo.push(c);
            pendente = true;
        }
    }
    if pendente {
        registro.push(campo);
        registros.push(registro);
    }
    registros
}

/// Texto UTF-8 sem o BOM (o `utf-8-sig`).
pub(crate) fn sem_bom(bruto: &[u8]) -> String {
    let sem = bruto.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bruto);
    String::from_utf8(sem.to_vec()).unwrap()
}

pub(crate) fn ler_csv(caminho: &Path, separador: char) -> Vec<Vec<String>> {
    ler_csv_texto(&sem_bom(&fs::read(caminho).unwrap()), separador)
}

// ---- JSON ---------------------------------------------------------------------------------------------

/// Valor JSON; números guardam o texto (inteiros grandes exatos).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Nulo,
    Bool(bool),
    Numero(String),
    Texto(String),
    Lista(Vec<Json>),
    Objeto(Vec<(String, Json)>),
}

impl Json {
    pub fn campo(&self, nome: &str) -> &Json {
        match self {
            Json::Objeto(pares) => {
                &pares.iter().find(|(k, _)| k == nome).unwrap_or_else(|| panic!("sem o campo {nome}")).1
            }
            outro => panic!("esperava objeto, veio {outro:?}"),
        }
    }

    pub fn lista(&self) -> &[Json] {
        match self {
            Json::Lista(itens) => itens,
            outro => panic!("esperava lista, veio {outro:?}"),
        }
    }

    pub fn chaves(&self) -> Vec<String> {
        match self {
            Json::Objeto(pares) => pares.iter().map(|(k, _)| k.clone()).collect(),
            outro => panic!("esperava objeto, veio {outro:?}"),
        }
    }

    pub fn texto(&self) -> &str {
        match self {
            Json::Texto(s) => s,
            outro => panic!("esperava texto, veio {outro:?}"),
        }
    }

    pub fn inteiro(&self) -> i64 {
        match self {
            Json::Numero(n) => n.parse().unwrap_or_else(|_| panic!("esperava inteiro, veio {n}")),
            outro => panic!("esperava número, veio {outro:?}"),
        }
    }
}

struct Leitor<'a> {
    s: &'a [u8],
    p: usize,
}

impl Leitor<'_> {
    fn espacos(&mut self) {
        while self.s.get(self.p).is_some_and(|c| c.is_ascii_whitespace()) {
            self.p += 1;
        }
    }

    fn literal(&mut self, palavra: &str) -> bool {
        if self.s[self.p..].starts_with(palavra.as_bytes()) {
            self.p += palavra.len();
            true
        } else {
            false
        }
    }

    fn hex4(&mut self) -> u32 {
        let t = std::str::from_utf8(&self.s[self.p..self.p + 4]).unwrap();
        self.p += 4;
        u32::from_str_radix(t, 16).unwrap()
    }

    fn texto(&mut self) -> String {
        assert_eq!(self.s[self.p], b'"');
        self.p += 1;
        let mut bytes = Vec::new();
        loop {
            let c = self.s[self.p];
            self.p += 1;
            match c {
                b'"' => return String::from_utf8(bytes).unwrap(),
                b'\\' => {
                    let e = self.s[self.p];
                    self.p += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let a = self.hex4();
                            let cp = if (0xD800..0xDC00).contains(&a) && self.s[self.p..].starts_with(b"\\u") {
                                self.p += 2;
                                let b = self.hex4();
                                0x10000 + ((a - 0xD800) << 10) + (b - 0xDC00)
                            } else {
                                a
                            };
                            char::from_u32(cp).unwrap_or('\u{fffd}')
                        }
                        outro => panic!("escape inválido {outro}"),
                    };
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                outro => bytes.push(outro),
            }
        }
    }

    fn valor(&mut self) -> Json {
        self.espacos();
        let primeiro = self.s[self.p];
        let v = match primeiro {
            b'{' => {
                self.p += 1;
                let mut pares = Vec::new();
                self.espacos();
                if self.s[self.p] == b'}' {
                    self.p += 1;
                } else {
                    loop {
                        self.espacos();
                        let k = self.texto();
                        self.espacos();
                        assert_eq!(self.s[self.p], b':');
                        self.p += 1;
                        pares.push((k, self.valor()));
                        self.espacos();
                        let c = self.s[self.p];
                        self.p += 1;
                        if c == b'}' {
                            break;
                        }
                        assert_eq!(c, b',');
                    }
                }
                Json::Objeto(pares)
            }
            b'[' => {
                self.p += 1;
                let mut itens = Vec::new();
                self.espacos();
                if self.s[self.p] == b']' {
                    self.p += 1;
                } else {
                    loop {
                        itens.push(self.valor());
                        self.espacos();
                        let c = self.s[self.p];
                        self.p += 1;
                        if c == b']' {
                            break;
                        }
                        assert_eq!(c, b',');
                    }
                }
                Json::Lista(itens)
            }
            b'"' => Json::Texto(self.texto()),
            _ if self.literal("null") => Json::Nulo,
            _ if self.literal("true") => Json::Bool(true),
            _ if self.literal("false") => Json::Bool(false),
            _ if self.literal("NaN") => Json::Numero("NaN".into()),
            _ if self.literal("Infinity") => Json::Numero("Infinity".into()),
            _ if self.literal("-Infinity") => Json::Numero("-Infinity".into()),
            _ => {
                let inicio = self.p;
                while self.s.get(self.p).is_some_and(|c| matches!(c, b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')) {
                    self.p += 1;
                }
                Json::Numero(String::from_utf8(self.s[inicio..self.p].to_vec()).unwrap())
            }
        };
        self.espacos();
        v
    }
}

pub(crate) fn ler_json_texto(texto: &str) -> Json {
    let mut l = Leitor { s: texto.as_bytes(), p: 0 };
    let v = l.valor();
    assert_eq!(l.p, texto.len(), "sobrou texto depois do JSON");
    v
}

pub(crate) fn ler_json(caminho: &Path) -> Json {
    ler_json_texto(&String::from_utf8(fs::read(caminho).unwrap()).unwrap())
}

// ---- base64 -------------------------------------------------------------------------------------------

pub(crate) fn base64(dados: &[u8]) -> String {
    const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for bloco in dados.chunks(3) {
        let b = [bloco[0], *bloco.get(1).unwrap_or(&0), *bloco.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= bloco.len() {
                s.push(char::from(ALFABETO[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                s.push('=');
            }
        }
    }
    s
}
