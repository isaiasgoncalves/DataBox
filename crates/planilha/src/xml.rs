//! Leitor XML "pull" escrito à mão, só com o que o SpreadsheetML precisa e sem carregar o arquivo inteiro:
//! lê em blocos de 64 KB de qualquer `Read` (o fluxo descomprimido de uma parte do zip) e devolve um evento
//! por chamada de [`Leitor::avancar`].
//!
//! - Elementos e atributos chegam pelo nome local: o prefixo de espaço de nomes (`x:row`, `r:id`) é ignorado
//!   e os atributos `xmlns` somem. Elementos que ninguém conhece passam batido.
//! - Texto: as cinco entidades (`&amp; &lt; &gt; &quot; &apos;`), referências numéricas (`&#65;`, `&#x41;`),
//!   CDATA, comentários e instruções (`<?xml ?>`, `<!DOCTYPE>`) tratados. Uma entidade desconhecida
//!   (`&abc;`) fica como está, e nenhuma entidade definida no arquivo é expandida (não há "bilhão de risadas").
//! - UTF-8 (com ou sem BOM). UTF-16 é recusado com mensagem clara; byte inválido vira "\u{FFFD}".
//! - Tolerante: espaços e quebras de linha em qualquer lugar, aspas simples ou duplas, atributo sem aspas,
//!   fechamento fora de ordem. Só é erro o XML que acaba no meio de uma marca, de um comentário ou com
//!   elementos abertos.
//! - Cada [`Evento::Abre`] vem com `nome`, atributos e o deslocamento (em bytes do fluxo) do `<`:
//!   com ele um índice consegue voltar a um ponto com [`Leitor::pular_para`] sem reler o arquivo.
//!
//! Nada é alocado por evento: o nome, os atributos e o texto são reaproveitados de uma chamada para a outra,
//! por isso valem só até o próximo `avancar`.

use std::fmt;
use std::io::{self, Read};

/// Tamanho do bloco de leitura.
const BLOCO: usize = 64 * 1024;
/// Quanto de janela é preciso para tentar a marca pelo caminho rápido (uma marca de verdade tem poucas dezenas
/// de bytes; uma maior que isso, ou cortada pelo fim do bloco, vai pelo caminho lento).
const JANELA_MARCA: usize = 1024;

/// O que o leitor encontrou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Evento {
    /// Abertura de elemento (também o `<a/>`, que é seguido logo por um [`Evento::Fecha`]).
    Abre,
    /// Fechamento de elemento.
    Fecha,
    /// Texto entre marcas, já sem entidades (e o CDATA junto). Pode ser só espaços.
    Texto,
    /// Fim do documento.
    Fim,
}

/// Falha da leitura: do disco/zip ou do próprio XML.
#[derive(Debug)]
pub(crate) enum ErroXml {
    Io(io::Error),
    /// XML malformado: posição em bytes do fluxo e o motivo.
    Malformado {
        posicao: u64,
        motivo: &'static str,
    },
    /// O que a parte guardaria na memória passa do limite, em bytes (um zip bomb, por exemplo).
    Grande {
        limite: usize,
    },
}

impl fmt::Display for ErroXml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroXml::Io(e) => write!(f, "{e}"),
            ErroXml::Malformado { posicao, motivo } => write!(f, "{motivo} (byte {posicao})"),
            ErroXml::Grande { limite } => write!(f, "passa do limite de {limite} bytes na memória"),
        }
    }
}

impl From<io::Error> for ErroXml {
    fn from(e: io::Error) -> Self {
        ErroXml::Io(e)
    }
}

type R<T> = Result<T, ErroXml>;

fn espaco(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

pub(crate) struct Leitor<F: Read> {
    fonte: F,
    buf: Vec<u8>,
    pos: usize,
    fim: usize,
    /// Deslocamento, no fluxo, do `buf[0]`.
    base: u64,
    eof: bool,
    iniciado: bool,
    /// Não guarda o texto (só as marcas): para passar por cima de milhões de células sem copiar nada.
    sem_texto: bool,
    filtro: Option<&'static [&'static str]>,
    fecha_pendente: bool,
    profundidade: usize,
    inicio_evento: u64,
    nome: String,
    atributos: Vec<(String, String)>,
    n_atributos: usize,
    texto: String,
    /// Acumulador do texto em bytes (a validação UTF-8 só vale com o texto inteiro, que pode atravessar blocos).
    acumulado: Vec<u8>,
    /// Área de trabalho dos nomes e valores de atributo.
    tmp: Vec<u8>,
    nome_atributo: Vec<u8>,
    ultima_ref: Vec<u8>,
}

impl<F: Read> Leitor<F> {
    pub fn novo(fonte: F) -> Self {
        Self::com_capacidade(fonte, BLOCO)
    }

    /// Para os testes forçarem blocos minúsculos (as emendas entre blocos são onde um leitor erra).
    pub fn com_capacidade(fonte: F, capacidade: usize) -> Self {
        Leitor {
            fonte,
            buf: vec![0; capacidade.max(32)],
            pos: 0,
            fim: 0,
            base: 0,
            eof: false,
            iniciado: false,
            sem_texto: false,
            filtro: None,
            fecha_pendente: false,
            profundidade: 0,
            inicio_evento: 0,
            nome: String::new(),
            atributos: Vec::new(),
            n_atributos: 0,
            texto: String::new(),
            acumulado: Vec::new(),
            tmp: Vec::new(),
            nome_atributo: Vec::new(),
            ultima_ref: Vec::new(),
        }
    }

    pub fn sem_texto(&mut self, sem: bool) {
        self.sem_texto = sem;
    }

    /// Só os elementos com estes nomes locais chegam como evento (abertura e fechamento); os outros são pulados
    /// sem nem ler os atributos. É um favor, não uma garantia: marcas que o caminho rápido não resolve (muito
    /// longas, com `&` em atributo…) chegam como sempre, então quem usa tem de ignorar o que não conhece.
    pub fn filtrar(&mut self, nomes: Option<&'static [&'static str]>) {
        self.filtro = nomes;
    }

    /// Nome local do último elemento aberto ou fechado.
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// Texto do último [`Evento::Texto`].
    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// Valor de um atributo do último [`Evento::Abre`] (pelo nome local).
    pub fn atributo(&self, nome: &str) -> Option<&str> {
        self.atributos.get(..self.n_atributos)?.iter().find(|(n, _)| n == nome).map(|(_, v)| v.as_str())
    }

    /// Deslocamento, no fluxo, do `<` do último evento de marca.
    pub fn inicio_do_evento(&self) -> u64 {
        self.inicio_evento
    }

    /// Onde o leitor está no fluxo (o byte seguinte ao que já foi consumido).
    pub fn deslocamento(&self) -> u64 {
        self.base + self.pos as u64
    }

    /// Para o modo rápido do índice, logo depois do `<row …>` (não vazio): se a linha inteira está na janela do
    /// bloco, pula direto até o `</row>` sem interpretar as células e guarda o `r="…"` da última célula (em
    /// [`Leitor::ultima_referencia`], vazio se a linha não tem células). `false`, sem ter consumido nada, quando
    /// não dá (linha maior que o bloco, CDATA ou comentário no meio, célula sem referência, prefixo no nome): quem
    /// chama lê a linha pelos eventos, que sempre funciona.
    pub fn pular_linha(&mut self) -> R<bool> {
        if self.fecha_pendente {
            return Ok(false);
        }
        if self.fim - self.pos < 4096 && !self.eof {
            self.garantir(self.buf.len())?;
        }
        let s = self.buf.get(self.pos..self.fim).unwrap_or_default();
        let mut i = 0;
        let fim_linha = loop {
            let Some(k) = s.get(i..).and_then(|resto| resto.iter().position(|&c| c == b'<')) else {
                return Ok(false);
            };
            let j = i + k;
            match s.get(j + 1) {
                None | Some(b'!' | b'?') => return Ok(false),
                Some(b'/') => match s.get(j + 2..j + 6) {
                    Some(b"row>") => break j,
                    Some(_) => {}
                    None => return Ok(false),
                },
                Some(_) => {}
            }
            i = j + 1;
        };
        let corpo = s.get(..fim_linha).unwrap_or_default();
        self.ultima_ref.clear();
        let mut ate = corpo.len();
        while let Some(k) = corpo.get(..ate).and_then(|antes| antes.iter().rposition(|&c| c == b'<')) {
            let marca = corpo.get(k..).unwrap_or_default();
            if marca.get(1) == Some(&b'c') && matches!(marca.get(2), Some(b' ' | b'>' | b'/' | b'\t' | b'\r' | b'\n')) {
                let fim_marca = marca.iter().position(|&c| c == b'>').unwrap_or(marca.len());
                let Some(valor) = marca.get(..fim_marca).and_then(|m| valor_do_atributo(m, b"r")) else {
                    return Ok(false);
                };
                self.ultima_ref.extend_from_slice(valor);
                break;
            }
            ate = k;
        }
        self.pos += fim_linha + 6;
        self.profundidade = self.profundidade.saturating_sub(1);
        Ok(true)
    }

    /// O `r` da última célula da linha que [`Leitor::pular_linha`] acabou de pular.
    pub fn ultima_referencia(&self) -> &str {
        std::str::from_utf8(&self.ultima_ref).unwrap_or("")
    }

    /// Vai para o deslocamento `alvo` do fluxo (que só anda para a frente) e recomeça num ponto "limpo",
    /// fora de qualquer elemento: serve para retomar num `<row` anotado pelo índice.
    pub fn pular_para(&mut self, alvo: u64) -> R<()> {
        self.iniciado = true;
        self.fecha_pendente = false;
        self.profundidade = 0;
        self.acumulado.clear();
        let atual = self.deslocamento();
        if alvo < atual {
            return Err(ErroXml::Malformado { posicao: atual, motivo: "não dá para voltar atrás no fluxo" });
        }
        let disponivel = (self.fim - self.pos) as u64;
        if alvo - atual <= disponivel {
            self.pos += (alvo - atual) as usize;
            return Ok(());
        }
        // além do bloco: descarta o resto dele e lê (sem interpretar) até o alvo
        self.base += self.fim as u64;
        self.pos = 0;
        self.fim = 0;
        while self.base < alvo && !self.eof {
            let quer = usize::try_from(alvo - self.base).unwrap_or(usize::MAX).min(self.buf.len());
            let n = self.ler_fonte(0, quer)?;
            if n == 0 {
                self.eof = true;
            } else {
                self.base += n as u64;
            }
        }
        Ok(())
    }

    fn ler_fonte(&mut self, de: usize, ate: usize) -> io::Result<usize> {
        loop {
            match self.fonte.read(self.buf.get_mut(de..ate).unwrap_or_default()) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                outro => return outro,
            }
        }
    }

    fn malformado(&self, motivo: &'static str) -> ErroXml {
        ErroXml::Malformado { posicao: self.deslocamento(), motivo }
    }

    /// Garante `n` bytes à frente (menos no fim do arquivo) e devolve quantos há. `n` cabe no bloco.
    fn garantir(&mut self, n: usize) -> io::Result<usize> {
        while self.fim - self.pos < n && !self.eof {
            if self.pos > 0 {
                self.buf.copy_within(self.pos..self.fim, 0);
                self.base += self.pos as u64;
                self.fim -= self.pos;
                self.pos = 0;
            }
            let lido = self.ler_fonte(self.fim, self.buf.len())?;
            if lido == 0 {
                self.eof = true;
            } else {
                self.fim += lido;
            }
        }
        Ok(self.fim - self.pos)
    }

    /// O byte atual, sem consumir (`None` no fim).
    fn ver(&mut self) -> io::Result<Option<u8>> {
        if self.pos >= self.fim && self.garantir(1)? == 0 {
            return Ok(None);
        }
        Ok(self.buf.get(self.pos).copied())
    }

    fn pular_espacos(&mut self) -> io::Result<()> {
        while let Some(c) = self.ver()? {
            if !espaco(c) {
                break;
            }
            self.pos += 1;
        }
        Ok(())
    }

    /// Anda até o próximo `a` ou `b` (sem consumi-lo) e devolve qual achou; `None` no fim. Os bytes
    /// passados vão para `destino` (o acumulado do texto ou a área de trabalho), se pedido.
    fn copiar_ate(&mut self, a: u8, b: u8, destino: Destino) -> io::Result<Option<u8>> {
        loop {
            if self.pos >= self.fim && self.garantir(1)? == 0 {
                return Ok(None);
            }
            let janela = self.buf.get(self.pos..self.fim).unwrap_or_default();
            match janela.iter().enumerate().find(|&(_, &c)| c == a || c == b) {
                Some((i, &achado)) => {
                    let antes = janela.get(..i).unwrap_or_default();
                    match destino {
                        Destino::Nada => {}
                        Destino::Acumulado => self.acumulado.extend_from_slice(antes),
                        Destino::Tmp => self.tmp.extend_from_slice(antes),
                    }
                    self.pos += i;
                    return Ok(Some(achado));
                }
                None => {
                    match destino {
                        Destino::Nada => {}
                        Destino::Acumulado => self.acumulado.extend_from_slice(janela),
                        Destino::Tmp => self.tmp.extend_from_slice(janela),
                    }
                    self.pos = self.fim;
                }
            }
        }
    }

    /// Anda até depois de `padrao` (comentário `-->`, instrução `?>`).
    fn pular_ate(&mut self, padrao: &[u8], motivo: &'static str) -> R<()> {
        let Some(&primeiro) = padrao.first() else {
            return Ok(());
        };
        loop {
            if self.copiar_ate(primeiro, primeiro, Destino::Nada)?.is_none() {
                return Err(self.malformado(motivo));
            }
            let n = self.garantir(padrao.len())?;
            if self.buf.get(self.pos..self.pos + n).is_some_and(|j| j.starts_with(padrao)) {
                self.pos += padrao.len();
                return Ok(());
            }
            self.pos += 1;
        }
    }

    /// O conteúdo de um CDATA, até `]]>`, como texto.
    fn ler_cdata(&mut self) -> R<()> {
        loop {
            let destino = if self.sem_texto { Destino::Nada } else { Destino::Acumulado };
            if self.copiar_ate(b']', b']', destino)?.is_none() {
                return Err(self.malformado("CDATA sem fim"));
            }
            let n = self.garantir(3)?;
            if self.buf.get(self.pos..self.pos + n).is_some_and(|j| j.starts_with(b"]]>")) {
                self.pos += 3;
                return Ok(());
            }
            if !self.sem_texto {
                self.acumulado.push(b']');
            }
            self.pos += 1;
        }
    }

    /// `<!DOCTYPE …>` (com subconjunto interno entre colchetes), já depois do `<!`.
    fn pular_doctype(&mut self) -> R<()> {
        let mut colchetes = 0usize;
        let mut aspas: Option<u8> = None;
        while let Some(c) = self.ver()? {
            self.pos += 1;
            match (aspas, c) {
                (Some(q), c) if c == q => aspas = None,
                (Some(_), _) => {}
                (None, b'"' | b'\'') => aspas = Some(c),
                (None, b'[') => colchetes += 1,
                (None, b']') => colchetes = colchetes.saturating_sub(1),
                (None, b'>') if colchetes == 0 => return Ok(()),
                _ => {}
            }
        }
        Err(self.malformado("DOCTYPE sem fim"))
    }

    /// Depois do `&`: se for uma entidade conhecida, consome até o `;` e devolve o caractere; senão não
    /// consome nada (o `&` fica como texto).
    fn entidade(&mut self) -> io::Result<Option<char>> {
        let n = self.garantir(12)?;
        let janela = self.buf.get(self.pos..self.pos + n).unwrap_or_default();
        let Some(fim) = janela.iter().take(11).position(|&c| c == b';') else {
            return Ok(None);
        };
        let nome = janela.get(..fim).unwrap_or_default();
        let c = match nome {
            b"amp" => Some('&'),
            b"lt" => Some('<'),
            b"gt" => Some('>'),
            b"quot" => Some('"'),
            b"apos" => Some('\''),
            [b'#', resto @ ..] if !resto.is_empty() => {
                let (digitos, base) = match resto {
                    [b'x' | b'X', hex @ ..] => (hex, 16),
                    dec => (dec, 10),
                };
                std::str::from_utf8(digitos)
                    .ok()
                    .and_then(|s| u32::from_str_radix(s, base).ok())
                    .map(|cp| char::from_u32(cp).filter(|&c| c != '\0').unwrap_or('\u{FFFD}'))
            }
            _ => None,
        };
        if c.is_some() {
            self.pos += fim + 1;
        }
        Ok(c)
    }

    /// Confere o começo do fluxo: BOM e codificações que não são UTF-8.
    fn iniciar(&mut self) -> R<()> {
        self.iniciado = true;
        let n = self.garantir(4)?;
        let inicio = self.buf.get(..n).unwrap_or_default();
        if inicio.starts_with(&[0xEF, 0xBB, 0xBF]) {
            self.pos = 3;
        } else if inicio.starts_with(&[0xFF, 0xFE]) || inicio.starts_with(&[0xFE, 0xFF]) {
            return Err(self.malformado("o XML está em UTF-16; só UTF-8 é suportado"));
        } else if inicio.starts_with(&[b'<', 0, b'?', 0]) || inicio.starts_with(&[0, b'<', 0, b'?']) {
            return Err(self.malformado("o XML está em UTF-16 sem BOM; só UTF-8 é suportado"));
        }
        Ok(())
    }

    /// Próximo evento.
    pub fn avancar(&mut self) -> R<Evento> {
        if self.fecha_pendente {
            self.fecha_pendente = false;
            self.profundidade = self.profundidade.saturating_sub(1);
            return Ok(Evento::Fecha);
        }
        if !self.iniciado {
            self.iniciar()?;
        }
        self.acumulado.clear();
        loop {
            let destino = if self.sem_texto { Destino::Nada } else { Destino::Acumulado };
            match self.copiar_ate(b'<', b'&', destino)? {
                None => {
                    if !self.acumulado.is_empty() {
                        return Ok(self.emitir_texto());
                    }
                    if self.profundidade > 0 {
                        return Err(self.malformado("o XML terminou com elementos sem fechar (arquivo incompleto)"));
                    }
                    return Ok(Evento::Fim);
                }
                Some(b'&') => {
                    self.pos += 1;
                    let c = self.entidade()?;
                    if !self.sem_texto {
                        let c = c.unwrap_or('&');
                        let mut tmp = [0u8; 4];
                        self.acumulado.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
                    }
                }
                Some(_) => {
                    let n = self.garantir(JANELA_MARCA.min(self.buf.len()))?;
                    let janela = self.buf.get(self.pos..self.pos + n).unwrap_or_default();
                    if janela.starts_with(b"<!--") {
                        self.pos += 4;
                        self.pular_ate(b"-->", "comentário sem fim")?;
                    } else if janela.starts_with(b"<![CDATA[") {
                        self.pos += 9;
                        self.ler_cdata()?;
                    } else if janela.starts_with(b"<?") {
                        self.pos += 2;
                        self.pular_ate(b"?>", "instrução de processamento sem fim")?;
                    } else if janela.starts_with(b"<!") {
                        self.pos += 2;
                        self.pular_doctype()?;
                    } else if !self.acumulado.is_empty() {
                        // o texto acumulado sai antes; o `<` fica para a próxima chamada
                        return Ok(self.emitir_texto());
                    } else if let Some(evento) = self.marca()? {
                        return Ok(evento);
                    }
                }
            }
        }
    }

    fn emitir_texto(&mut self) -> Evento {
        // o acumulado vira o texto sem copiar (se for UTF-8 válido) e o texto velho vira o próximo acumulado
        let bytes = std::mem::take(&mut self.acumulado);
        let (novo, sobra) = match String::from_utf8(bytes) {
            Ok(s) => (s, std::mem::take(&mut self.texto).into_bytes()),
            Err(e) => {
                let lossy = String::from_utf8_lossy(e.as_bytes()).into_owned();
                (lossy, e.into_bytes())
            }
        };
        self.texto = novo;
        self.acumulado = sobra;
        self.acumulado.clear();
        Evento::Texto
    }

    /// Uma marca de elemento, no `<`.
    /// `None`: a marca foi descartada pelo filtro e a leitura segue.
    fn marca(&mut self) -> R<Option<Evento>> {
        self.inicio_evento = self.deslocamento();
        if self.fim - self.pos >= JANELA_MARCA || self.eof {
            match self.marca_rapida() {
                Rapida::Evento(e) => return Ok(Some(e)),
                Rapida::Ignorada => return Ok(None),
                Rapida::Lenta => {}
            }
        }
        self.pos += 1;
        match self.ver()? {
            None => Err(self.malformado("o XML terminou no meio de uma marca")),
            Some(b'/') => {
                self.pos += 1;
                self.ler_nome()?;
                self.guardar_nome();
                // até o `>`
                if self.copiar_ate(b'>', b'>', Destino::Nada)?.is_none() {
                    return Err(self.malformado("o XML terminou no meio de uma marca"));
                }
                self.pos += 1;
                self.profundidade = self.profundidade.saturating_sub(1);
                Ok(Some(Evento::Fecha))
            }
            Some(_) => self.abre().map(Some),
        }
    }

    /// O caminho rápido das marcas: a marca inteira está na janela do bloco e é do jeito comum (atributos
    /// entre aspas, sem `&`, nomes em UTF-8), então é interpretada direto da janela, sem passar byte a byte pelo
    /// controle de blocos. `None` deixa a marca para o caminho lento, que cobre todo o resto (e os erros),
    /// sem ter consumido nada.
    fn marca_rapida(&mut self) -> Rapida {
        self.marca_rapida_opcional().unwrap_or(Rapida::Lenta)
    }

    fn marca_rapida_opcional(&mut self) -> Option<Rapida> {
        let s = self.buf.get(self.pos..self.fim)?;
        let n = s.len();
        if s.get(1) == Some(&b'/') {
            let fim = s.iter().position(|&c| c == b'>')?;
            let mut nome = s.get(2..fim)?;
            while let [resto @ .., ultimo] = nome {
                if !espaco(*ultimo) {
                    break;
                }
                nome = resto;
            }
            let nome = std::str::from_utf8(nome_local(nome)).ok()?;
            self.pos += fim + 1;
            self.profundidade = self.profundidade.saturating_sub(1);
            if self.filtro.is_some_and(|f| !f.contains(&nome)) {
                return Some(Rapida::Ignorada);
            }
            self.nome.clear();
            self.nome.push_str(nome);
            return Some(Rapida::Evento(Evento::Fecha));
        }
        let mut i = 1;
        while s.get(i).is_some_and(|&c| !espaco(c) && !matches!(c, b'/' | b'>' | b'=')) {
            i += 1;
        }
        if i >= n {
            return None;
        }
        let nome_elemento = std::str::from_utf8(nome_local(s.get(1..i)?)).ok()?;
        if self.filtro.is_some_and(|f| !f.contains(&nome_elemento)) {
            // elemento que ninguém quer: só acha o fim da marca (com aspas em conta) e segue
            let mut aspas = 0u8;
            let mut j = i;
            while let Some(&c) = s.get(j) {
                if aspas != 0 {
                    if c == aspas {
                        aspas = 0;
                    }
                } else if c == b'"' || c == b'\'' {
                    aspas = c;
                } else if c == b'>' {
                    break;
                }
                j += 1;
            }
            if j >= n {
                return None;
            }
            let vazio = s.get(j - 1) == Some(&b'/');
            self.pos += j + 1;
            if !vazio {
                self.profundidade += 1;
            }
            return Some(Rapida::Ignorada);
        }
        let mut n_atributos = 0;
        let vazio;
        loop {
            while s.get(i).is_some_and(|&c| espaco(c)) {
                i += 1;
            }
            match *s.get(i)? {
                b'>' => {
                    i += 1;
                    vazio = false;
                    break;
                }
                b'/' => {
                    if s.get(i + 1) != Some(&b'>') {
                        return None;
                    }
                    i += 2;
                    vazio = true;
                    break;
                }
                _ => {
                    let ini = i;
                    while s.get(i).is_some_and(|&c| !espaco(c) && !matches!(c, b'=' | b'/' | b'>')) {
                        i += 1;
                    }
                    if i == ini {
                        return None;
                    }
                    let nome_atributo = s.get(ini..i)?;
                    while s.get(i).is_some_and(|&c| espaco(c)) {
                        i += 1;
                    }
                    if s.get(i) != Some(&b'=') {
                        return None;
                    }
                    i += 1;
                    while s.get(i).is_some_and(|&c| espaco(c)) {
                        i += 1;
                    }
                    let aspas = *s.get(i)?;
                    if aspas != b'"' && aspas != b'\'' {
                        return None;
                    }
                    i += 1;
                    let ini_valor = i;
                    while let Some(&c) = s.get(i) {
                        if c == aspas {
                            break;
                        }
                        if c == b'&' {
                            return None;
                        }
                        i += 1;
                    }
                    if i >= n {
                        return None;
                    }
                    let valor = std::str::from_utf8(s.get(ini_valor..i)?).ok()?;
                    i += 1;
                    if nome_atributo == b"xmlns" || nome_atributo.starts_with(b"xmlns:") {
                        continue;
                    }
                    let nome_atributo = std::str::from_utf8(nome_local(nome_atributo)).ok()?;
                    if n_atributos == self.atributos.len() {
                        self.atributos.push((String::new(), String::new()));
                    }
                    let (a, v) = self.atributos.get_mut(n_atributos)?;
                    a.clear();
                    a.push_str(nome_atributo);
                    v.clear();
                    v.push_str(valor);
                    n_atributos += 1;
                }
            }
        }
        self.nome.clear();
        self.nome.push_str(nome_elemento);
        self.n_atributos = n_atributos;
        self.pos += i;
        self.profundidade += 1;
        self.fecha_pendente = vazio;
        Some(Rapida::Evento(Evento::Abre))
    }

    /// Lê um nome (até espaço, `/`, `>` ou `=`) para a área de trabalho.
    fn ler_nome(&mut self) -> R<()> {
        self.tmp.clear();
        while let Some(c) = self.ver()? {
            if espaco(c) || matches!(c, b'/' | b'>' | b'=') {
                break;
            }
            self.tmp.push(c);
            self.pos += 1;
        }
        Ok(())
    }

    /// O nome da área de trabalho, sem o prefixo, vira o nome do elemento.
    fn guardar_nome(&mut self) {
        let local = nome_local(&self.tmp);
        self.nome.clear();
        self.nome.push_str(&String::from_utf8_lossy(local));
    }

    fn abre(&mut self) -> R<Evento> {
        self.ler_nome()?;
        self.guardar_nome();
        self.n_atributos = 0;
        let mut vazio = false;
        loop {
            self.pular_espacos()?;
            match self.ver()? {
                None => return Err(self.malformado("o XML terminou no meio de uma marca")),
                Some(b'>') => {
                    self.pos += 1;
                    break;
                }
                Some(b'/') => {
                    self.pos += 1;
                    self.pular_espacos()?;
                    if self.ver()? == Some(b'>') {
                        self.pos += 1;
                    }
                    vazio = true;
                    break;
                }
                Some(_) => self.ler_atributo()?,
            }
        }
        self.profundidade += 1;
        self.fecha_pendente = vazio;
        Ok(Evento::Abre)
    }

    /// Um `nome="valor"` da marca aberta.
    fn ler_atributo(&mut self) -> R<()> {
        self.ler_nome()?;
        if self.tmp.is_empty() {
            // um byte solto (aspas sem nome, por exemplo): descarta para não girar em falso
            self.pos += 1;
            return Ok(());
        }
        let namespace = self.tmp == b"xmlns" || self.tmp.starts_with(b"xmlns:");
        self.nome_atributo.clear();
        self.nome_atributo.extend_from_slice(nome_local(&self.tmp));
        self.pular_espacos()?;
        self.tmp.clear();
        if self.ver()? == Some(b'=') {
            self.pos += 1;
            self.pular_espacos()?;
            match self.ver()? {
                Some(q @ (b'"' | b'\'')) => {
                    self.pos += 1;
                    self.valor_entre_aspas(q)?;
                }
                Some(_) => self.valor_sem_aspas()?,
                None => return Err(self.malformado("o XML terminou no meio de uma marca")),
            }
        }
        if namespace {
            return Ok(());
        }
        let i = self.n_atributos;
        if i == self.atributos.len() {
            self.atributos.push((String::new(), String::new()));
        }
        let Some((nome, valor)) = self.atributos.get_mut(i) else {
            return Ok(());
        };
        nome.clear();
        nome.push_str(&String::from_utf8_lossy(&self.nome_atributo));
        valor.clear();
        valor.push_str(&String::from_utf8_lossy(&self.tmp));
        self.n_atributos += 1;
        Ok(())
    }

    fn valor_entre_aspas(&mut self, aspas: u8) -> R<()> {
        loop {
            match self.copiar_ate(aspas, b'&', Destino::Tmp)? {
                None => return Err(self.malformado("o XML terminou no meio de um atributo")),
                Some(b'&') => {
                    self.pos += 1;
                    let c = self.entidade()?.unwrap_or('&');
                    let mut b = [0u8; 4];
                    self.tmp.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                }
                Some(_) => {
                    self.pos += 1;
                    return Ok(());
                }
            }
        }
    }

    /// Atributo sem aspas (HTML-ísmo): vai até o espaço, `/>` ou `>`.
    fn valor_sem_aspas(&mut self) -> R<()> {
        while let Some(c) = self.ver()? {
            if espaco(c) || c == b'>' {
                break;
            }
            if c == b'/' {
                let n = self.garantir(2)?;
                if n >= 2 && self.buf.get(self.pos + 1) == Some(&b'>') {
                    break;
                }
            }
            self.tmp.push(c);
            self.pos += 1;
        }
        Ok(())
    }
}

/// O que o caminho rápido das marcas fez.
enum Rapida {
    Evento(Evento),
    /// A marca era de um elemento fora do filtro e foi pulada.
    Ignorada,
    /// Não deu: a marca vai pelo caminho lento.
    Lenta,
}

#[derive(Clone, Copy)]
enum Destino {
    Nada,
    Acumulado,
    Tmp,
}

/// O valor de um atributo (` nome="…"` ou `nome='…'`) numa marca já recortada, sem entidades.
fn valor_do_atributo<'a>(marca: &'a [u8], nome: &[u8]) -> Option<&'a [u8]> {
    let mut i = 0;
    while i + nome.len() + 2 < marca.len() {
        if marca.get(i).is_some_and(|&c| espaco(c)) && marca.get(i + 1..).is_some_and(|r| r.starts_with(nome)) {
            let mut j = i + 1 + nome.len();
            if marca.get(j) == Some(&b'=') {
                j += 1;
                if let Some(&q @ (b'"' | b'\'')) = marca.get(j) {
                    let valor = marca.get(j + 1..)?;
                    let fim = valor.iter().position(|&c| c == q)?;
                    return valor.get(..fim);
                }
            }
        }
        i += 1;
    }
    None
}

/// `x:row` → `row`.
fn nome_local(nome: &[u8]) -> &[u8] {
    match nome.iter().rposition(|&c| c == b':') {
        Some(i) => nome.get(i + 1..).unwrap_or_default(),
        None => nome,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Entrega o fluxo de poucos em poucos bytes, para testar as emendas.
    struct Gota<'a> {
        dados: &'a [u8],
        passo: usize,
    }

    impl Read for Gota<'_> {
        fn read(&mut self, saida: &mut [u8]) -> io::Result<usize> {
            let n = self.passo.min(self.dados.len()).min(saida.len());
            saida[..n].copy_from_slice(&self.dados[..n]);
            self.dados = &self.dados[n..];
            Ok(n)
        }
    }

    /// Resumo textual dos eventos: `<nome a=v>`, `</nome>`, `"texto"`.
    fn eventos_de(xml: &[u8], passo: usize, capacidade: usize) -> Result<Vec<String>, ErroXml> {
        let mut l = Leitor::com_capacidade(Gota { dados: xml, passo }, capacidade);
        let mut saida = Vec::new();
        loop {
            match l.avancar()? {
                Evento::Fim => return Ok(saida),
                Evento::Abre => {
                    let mut s = format!("<{}", l.nome());
                    for (n, v) in &l.atributos[..l.n_atributos] {
                        s.push_str(&format!(" {n}={v}"));
                    }
                    s.push('>');
                    saida.push(s);
                }
                Evento::Fecha => saida.push(format!("</{}>", l.nome())),
                Evento::Texto => saida.push(format!("\"{}\"", l.texto())),
            }
        }
    }

    /// Roda em todos os tamanhos de bloco: um byte, poucos, o normal.
    fn ev(xml: &str) -> Vec<String> {
        let certo = eventos_de(xml.as_bytes(), usize::MAX, BLOCO).unwrap();
        for (passo, cap) in [(1, 32), (3, 32), (7, 40), (1, 64), (5, 33)] {
            assert_eq!(eventos_de(xml.as_bytes(), passo, cap).unwrap(), certo, "passo {passo} cap {cap}");
        }
        certo
    }

    #[test]
    fn elementos_atributos_e_texto() {
        let e = ev("<a x=\"1\" y='dois'><b/>texto<c>mais</c></a>");
        assert_eq!(e, ["<a x=1 y=dois>", "<b>", "</b>", "\"texto\"", "<c>", "\"mais\"", "</c>", "</a>"]);
    }

    #[test]
    fn prefixos_de_espaco_de_nomes_sao_ignorados_e_xmlns_some() {
        let e = ev("<x:row xmlns:x=\"u\" xmlns=\"v\" r:id=\"rId1\" x:r=\"3\"></x:row>");
        assert_eq!(e, ["<row id=rId1 r=3>", "</row>"]);
    }

    #[test]
    fn entidades_basicas_e_numericas() {
        let e =
            ev("<t a=\"&quot;&amp;&apos;&lt;&gt;\">&amp;&lt;&gt;&quot;&apos; &#65;&#x42;&#X43; &#233;&#x1F600;</t>");
        assert_eq!(e[0], "<t a=\"&'<>>");
        assert_eq!(e[1], "\"&<>\"' ABC é😀\"");
    }

    #[test]
    fn entidade_desconhecida_ou_solta_fica_como_esta() {
        let e = ev("<t>a &foo; b & c &#; d &#xZZ; e &amp</t>");
        assert_eq!(e[1], "\"a &foo; b & c &#; d &#xZZ; e &amp\"");
    }

    #[test]
    fn referencia_invalida_vira_o_caractere_de_substituicao() {
        let e = ev("<t>&#0;&#xD800;&#x110000;</t>");
        assert_eq!(e[1], "\"\u{FFFD}\u{FFFD}\u{FFFD}\"");
    }

    #[test]
    fn cdata_comentarios_e_instrucoes() {
        let e = ev("<?xml version=\"1.0\"?><!-- c --><a>x<![CDATA[<b> & ]]>y<!-- depois -->z<?pi dados?></a>");
        assert_eq!(e, ["<a>", "\"x<b> & yz\"", "</a>"]);
    }

    #[test]
    fn cdata_com_colchetes_soltos() {
        let e = ev("<a><![CDATA[a]b]]c]]></a>");
        assert_eq!(e[1], "\"a]b]]c\"");
    }

    #[test]
    fn doctype_com_subconjunto_e_entidade_nao_expandida() {
        let e = ev("<!DOCTYPE a [<!ENTITY x \"y>z\">]><a>&x;</a>");
        assert_eq!(e, ["<a>", "\"&x;\"", "</a>"]);
    }

    #[test]
    fn espacos_e_quebras_em_qualquer_lugar() {
        let e = ev("<a\n  x = \"1\"\r\n y='2' >\n  <b  />\n</a >");
        assert_eq!(e, ["<a x=1 y=2>", "\"\n  \"", "<b>", "</b>", "\"\n\"", "</a>"]);
    }

    #[test]
    fn atributo_sem_aspas_e_sem_valor() {
        let e = ev("<a x=1 y z=\"3\"/><b q=2/>");
        assert_eq!(e, ["<a x=1 y= z=3>", "</a>", "<b q=2>", "</b>"]);
    }

    #[test]
    fn utf8_e_bom() {
        let mut dados = vec![0xEF, 0xBB, 0xBF];
        dados.extend_from_slice("<a>ação 日本語 ✓</a>".as_bytes());
        for passo in [1, 2, usize::MAX] {
            let e = eventos_de(&dados, passo, 32).unwrap();
            assert_eq!(e, ["<a>", "\"ação 日本語 ✓\"", "</a>"]);
        }
    }

    #[test]
    fn byte_invalido_vira_substituicao() {
        let e = eventos_de(b"<a>x\xFFy</a>", usize::MAX, BLOCO).unwrap();
        assert_eq!(e[1], "\"x\u{FFFD}y\"");
    }

    #[test]
    fn utf16_e_recusado() {
        for dados in [&b"\xFF\xFE<\0a\0>\0"[..], b"\xFE\xFF\0<\0a\0>", b"<\0?\0x\0m\0l\0"] {
            match eventos_de(dados, usize::MAX, BLOCO) {
                Err(ErroXml::Malformado { motivo, .. }) => assert!(motivo.contains("UTF-16"), "{motivo}"),
                outro => panic!("esperava erro de UTF-16: {outro:?}"),
            }
        }
    }

    #[test]
    fn truncado_no_meio_e_erro() {
        for xml in ["<a><b x=\"1", "<a><b", "<a><!-- x", "<a><![CDATA[x", "<a>", "<a><b></b>", "<a></", "<!DOCTYPE a"] {
            assert!(matches!(eventos_de(xml.as_bytes(), usize::MAX, BLOCO), Err(ErroXml::Malformado { .. })), "{xml}");
        }
    }

    #[test]
    fn fechamento_fora_de_ordem_e_tolerado() {
        let e = ev("<a><b></a></b>");
        assert_eq!(e, ["<a>", "<b>", "</a>", "</b>"]);
    }

    #[test]
    fn documento_vazio_ou_so_com_texto() {
        assert_eq!(ev(""), Vec::<String>::new());
        assert_eq!(ev("  \n"), ["\"  \n\""]);
    }

    #[test]
    fn texto_depois_da_raiz_e_marcas_soltas_de_fechamento_nao_travam() {
        let e = ev("<a></a></b>fim");
        assert_eq!(e, ["<a>", "</a>", "</b>", "\"fim\""]);
    }

    #[test]
    fn deslocamento_do_evento_aponta_para_o_menor_que() {
        let xml = "<?xml version=\"1.0\"?>\n<a>\n  <b x=\"1\"/>\n</a>";
        let mut l = Leitor::com_capacidade(Gota { dados: xml.as_bytes(), passo: 2 }, 32);
        let mut vistos = Vec::new();
        loop {
            match l.avancar().unwrap() {
                Evento::Abre => vistos.push((l.nome().to_string(), l.inicio_do_evento())),
                Evento::Fim => break,
                _ => {}
            }
        }
        assert_eq!(vistos[0], ("a".to_string(), xml.find("<a>").unwrap() as u64));
        assert_eq!(vistos[1], ("b".to_string(), xml.find("<b").unwrap() as u64));
    }

    #[test]
    fn pular_para_retoma_no_ponto_certo() {
        // blocos pequenos para o salto cair dentro do bloco e além dele
        let mut xml = String::from("<w><d>");
        let mut pontos = Vec::new();
        for i in 0..200 {
            pontos.push(xml.len() as u64);
            xml.push_str(&format!("<row r=\"{i}\"><c>v{i}</c></row>"));
        }
        xml.push_str("</d></w>");
        for alvo in [3usize, 150, 199, 4] {
            let mut l = Leitor::com_capacidade(Gota { dados: xml.as_bytes(), passo: 13 }, 48);
            // anda um pouco antes de pular, como um cursor em uso
            for _ in 0..5 {
                l.avancar().unwrap();
            }
            if pontos[alvo] < l.deslocamento() {
                continue;
            }
            l.pular_para(pontos[alvo]).unwrap();
            assert_eq!(l.avancar().unwrap(), Evento::Abre);
            assert_eq!(l.nome(), "row");
            assert_eq!(l.atributo("r"), Some(alvo.to_string().as_str()));
            assert_eq!(l.inicio_do_evento(), pontos[alvo]);
        }
    }

    #[test]
    fn pular_para_depois_do_fim_e_para_tras() {
        let mut l = Leitor::com_capacidade(Gota { dados: b"<a>texto</a>", passo: 4 }, 32);
        l.pular_para(1000).unwrap();
        assert_eq!(l.avancar().unwrap(), Evento::Fim);
        let mut l = Leitor::com_capacidade(Gota { dados: b"<a>texto</a>", passo: 100 }, 32);
        l.avancar().unwrap();
        l.avancar().unwrap();
        assert!(l.pular_para(0).is_err());
    }

    #[test]
    fn sem_texto_ignora_o_conteudo_mas_acha_as_marcas() {
        let mut l = Leitor::com_capacidade(Gota { dados: b"<a>um &amp; dois<![CDATA[x]]><b/>tres</a>", passo: 3 }, 32);
        l.sem_texto(true);
        let mut e = Vec::new();
        loop {
            match l.avancar().unwrap() {
                Evento::Fim => break,
                Evento::Texto => e.push("texto".to_string()),
                Evento::Abre => e.push(format!("<{}", l.nome())),
                Evento::Fecha => e.push(format!("/{}", l.nome())),
            }
        }
        assert_eq!(e, ["<a", "<b", "/b", "/a"]);
    }

    #[test]
    fn atributos_vazios_e_repetidos_entre_elementos() {
        let e = ev("<a x=\"\" y=\"1\"/><b/><c z='&#10;'/>");
        assert_eq!(e, ["<a x= y=1>", "</a>", "<b>", "</b>", "<c z=\n>", "</c>"]);
    }

    #[test]
    fn erro_de_leitura_do_fluxo_sobe_como_io() {
        struct Quebrado;
        impl Read for Quebrado {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("falhou"))
            }
        }
        let mut l = Leitor::novo(Quebrado);
        assert!(matches!(l.avancar(), Err(ErroXml::Io(_))));
    }

    #[test]
    fn marca_cortada_pelo_fim_do_bloco_vai_pelo_caminho_lento_e_da_o_mesmo() {
        let longa = "x".repeat(3000);
        let xml = format!("<a v=\"{longa}\" w='y\"z' q=\"a>b\">texto</a><b p=\"1\"/>");
        let esperado = [
            "<a v=".to_string() + &longa + " w=y\"z q=a>b>",
            "\"texto\"".into(),
            "</a>".into(),
            "<b p=1>".into(),
            "</b>".into(),
        ];
        for (passo, cap) in [(usize::MAX, BLOCO), (700, 4096), (1500, 2048), (1, 5000), (9, 64)] {
            assert_eq!(eventos_de(xml.as_bytes(), passo, cap).unwrap(), esperado, "passo {passo} cap {cap}");
        }
    }

    #[test]
    fn filtro_pula_o_que_ninguem_pediu() {
        const FILTRO: &[&str] = &["a", "c"];
        let xml = "<a><b x=\"1\"><skip y=\"&amp;>\"><d/></skip></b>texto<c z=\"3\"/><skip q='>'/></a>";
        let mut l = Leitor::novo(Gota { dados: xml.as_bytes(), passo: usize::MAX });
        l.filtrar(Some(FILTRO));
        let mut e = Vec::new();
        loop {
            match l.avancar().unwrap() {
                Evento::Fim => break,
                Evento::Abre => e.push(format!("<{}", l.nome())),
                Evento::Fecha => e.push(format!("/{}", l.nome())),
                Evento::Texto => e.push(format!("\"{}\"", l.texto())),
            }
        }
        // a profundidade ficou certa (o fim do documento não reclamou de elementos abertos)
        assert_eq!(e, ["<a", "\"texto\"", "<c", "/c", "/a"]);
        // o atributo de um elemento que passa pelo filtro continua legível
        let mut l = Leitor::novo(Gota { dados: b"<x><c r=\"B2\" s='5'/></x>", passo: usize::MAX });
        l.filtrar(Some(FILTRO));
        assert_eq!(l.avancar().unwrap(), Evento::Abre);
        assert_eq!((l.nome(), l.atributo("r"), l.atributo("s")), ("c", Some("B2"), Some("5")));
    }

    #[test]
    fn filtro_com_blocos_pequenos_so_traz_a_mais_nunca_a_menos() {
        const FILTRO: &[&str] = &["row", "c"];
        let xml = "<w><d><row r=\"1\"><c r=\"A1\"><v>1</v></c></row><row r=\"2\"><c r=\"A2\"/></row></d></w>";
        let nomes = |passo: usize, cap: usize| {
            let mut l = Leitor::com_capacidade(Gota { dados: xml.as_bytes(), passo }, cap);
            l.filtrar(Some(FILTRO));
            let mut e = Vec::new();
            loop {
                match l.avancar().unwrap() {
                    Evento::Fim => break,
                    Evento::Abre if FILTRO.contains(&l.nome()) => {
                        e.push(format!("<{} r={:?}", l.nome(), l.atributo("r")))
                    }
                    Evento::Fecha if FILTRO.contains(&l.nome()) => e.push(format!("/{}", l.nome())),
                    _ => {}
                }
            }
            e
        };
        let certo = nomes(usize::MAX, BLOCO);
        assert_eq!(certo.len(), 8);
        for (passo, cap) in [(1, 32), (3, 40), (5, 64)] {
            assert_eq!(nomes(passo, cap), certo);
        }
    }

    #[test]
    fn nome_com_dois_pontos_e_hifen() {
        let e = ev("<mc:AlternateContent><a:b-c x:y-z=\"1\"/></mc:AlternateContent>");
        assert_eq!(e[0], "<AlternateContent>");
        assert_eq!(e[1], "<b-c y-z=1>");
    }
}
