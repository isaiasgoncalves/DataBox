//! O analisador de JSON: estrito (RFC 8259), sem recursão e com a posição de cada valor.
//!
//! Uma máquina de estados com a pilha dos objetos e listas abertos num `Vec`: o aninhamento é limitado pela memória,
//! não pela pilha de chamadas (100.000 níveis, ou um milhão, não estouram nada). Nada além da gramática do JSON é
//! aceito: sem comentários, sem `NaN` nem `Infinity`, sem aspas simples, sem vírgula sobrando, sem zero à esquerda
//! nem número como `1.` ou `.5`. Chave repetida é aceita e guardada (as duas). Um BOM no começo é pulado.
//!
//! O erro aponta o começo do símbolo que **estraga** e diz o motivo numa das 17 frases de [`super::erro`]. Quando
//! o documento acaba antes da hora, aponta o fim do último símbolo (sem contar os espaços e quebras finais).

use std::ops::Range;

use super::arvore::{Arvore, NENHUM, No, Tipo};
use super::erro::{ErroJson, Motivo, descrever, limitar_chave, limitar_trecho};

/// O resultado de [`analisar`]: a estrutura se o documento é válido, o erro se não é (nunca os dois).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analise {
    pub arvore: Option<Arvore>,
    pub erro: Option<ErroJson>,
}

/// Analisa o documento e monta a estrutura (a [`Arvore`]). Com o documento inválido, `arvore` é `None` e `erro`
/// diz onde e por quê.
pub fn analisar(texto: &str) -> Analise {
    match Analisador::novo(texto, true).executar() {
        Ok(nos) => Analise { arvore: Some(Arvore::de_nos(nos)), erro: None },
        Err(erro) => Analise { arvore: None, erro: Some(erro) },
    }
}

/// Só confere o documento, sem guardar nó nenhum: para os arquivos grandes demais para a estrutura
/// ([`crate::limites::LIMITE_ESTRUTURA`]) e para quem só quer o veredito.
pub fn validar(texto: &str) -> Option<ErroJson> {
    Analisador::novo(texto, false).executar().err()
}

/// Onde a máquina está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Passo {
    /// Esperando um valor (começo do documento, depois de `:` num objeto ou de `,` numa lista).
    Valor,
    /// Logo depois de `[`: um valor ou o `]` de uma lista vazia.
    PrimeiroDaLista,
    /// Logo depois de `{`: um nome ou o `}` de um objeto vazio.
    PrimeiroNome,
    /// Depois de `,` num objeto: um nome.
    Nome,
    /// Depois do nome: os dois pontos.
    DoisPontos,
    /// Depois de um valor dentro de um objeto ou lista: `,` ou o fecho.
    DepoisDoValor,
    /// O valor do documento fechou: só pode sobrar espaço.
    Fim,
}

/// Um objeto ou lista aberto.
struct Quadro {
    /// O nó dele na arena (`NENHUM` quando só se valida).
    no: u32,
    objeto: bool,
    filhos: u32,
    /// O último filho já ligado (para ligar o próximo irmão).
    ultimo: u32,
    /// A chave do valor que vem a seguir, com as aspas (só em objeto).
    chave: Range<usize>,
    /// Onde está a última vírgula (para "vírgula sobrando").
    virgula: usize,
}

struct Analisador<'t> {
    texto: &'t str,
    bytes: &'t [u8],
    pos: usize,
    construir: bool,
    nos: Vec<No>,
    pilha: Vec<Quadro>,
}

/// O caractere pode fazer parte de uma "palavra": número, `true`, `null`, `NaN`, qualquer coisa sem aspas nem
/// pontuação. Os invisíveis fora do ASCII (o espaço sem quebra, o BOM no meio do texto…) não entram: ficam como o
/// erro "valor inesperado: U+00A0" em vez de grudar no que vem depois.
fn e_de_palavra(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-' | '$')
    } else {
        !c.is_whitespace() && !c.is_control() && c != '\u{FEFF}' && c != '\u{200B}'
    }
}

impl<'t> Analisador<'t> {
    fn novo(texto: &'t str, construir: bool) -> Analisador<'t> {
        Analisador { texto, bytes: texto.as_bytes(), pos: 0, construir, nos: Vec::new(), pilha: Vec::new() }
    }

    fn erro(&self, byte: usize, motivo: Motivo) -> ErroJson {
        ErroJson::em(self.texto, byte, motivo)
    }

    /// O fim do último símbolo: onde se aponta o documento que acaba antes da hora.
    fn fim_do_conteudo(&self) -> usize {
        self.texto.trim_end_matches([' ', '\t', '\n', '\r']).len()
    }

    /// O documento acabou onde ainda faltava alguma coisa.
    fn fim_inesperado(&self) -> ErroJson {
        let motivo = if self.pilha.is_empty() { Motivo::EsperavaValor } else { Motivo::TerminaAntes };
        self.erro(self.fim_do_conteudo(), motivo)
    }

    /// Pula os espaços. Um comentário (`//` ou `/*`) em qualquer lugar entre símbolos é o erro dele.
    fn pular_espacos(&mut self) -> Result<(), ErroJson> {
        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                b'/' if matches!(self.bytes.get(self.pos + 1), Some(b'/' | b'*')) => {
                    return Err(self.erro(self.pos, Motivo::Comentario));
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Cria o nó de um valor (ligado ao quadro de cima da pilha, que é o pai) e conta o filho. Devolve o id, ou
    /// `NENHUM` quando só se valida.
    fn novo_no(&mut self, tipo: Tipo, inicio: usize, fim: usize) -> u32 {
        let (pai, ordem, chave, anterior) = match self.pilha.last_mut() {
            Some(q) => {
                let ordem = q.filhos;
                q.filhos += 1;
                if !self.construir {
                    return NENHUM;
                }
                let chave = q.objeto.then(|| q.chave.clone());
                (q.no, ordem, chave, q.ultimo)
            }
            None => (NENHUM, 0, None, NENHUM),
        };
        if !self.construir {
            return NENHUM;
        }
        let id = u32::try_from(self.nos.len()).unwrap_or(NENHUM - 1);
        self.nos.push(No::novo(tipo, inicio, fim, chave, pai, ordem));
        if anterior == NENHUM {
            if let Some(no_pai) = self.nos.get_mut(pai as usize) {
                no_pai.primeiro = id;
            }
        } else if let Some(no_anterior) = self.nos.get_mut(anterior as usize) {
            no_anterior.irmao = id;
        }
        if let Some(q) = self.pilha.last_mut() {
            q.ultimo = id;
        }
        id
    }

    /// Abre um objeto ou lista na posição atual.
    fn abrir(&mut self, tipo: Tipo) {
        let id = self.novo_no(tipo, self.pos, self.pos + 1);
        self.pilha.push(Quadro {
            no: id,
            objeto: tipo == Tipo::Objeto,
            filhos: 0,
            ultimo: NENHUM,
            chave: 0..0,
            virgula: 0,
        });
        self.pos += 1;
    }

    /// Fecha o objeto ou lista de cima da pilha; `pos` está no `}` ou `]`.
    fn fechar(&mut self) {
        let Some(quadro) = self.pilha.pop() else {
            return;
        };
        self.pos += 1;
        if self.construir
            && let Some(no) = self.nos.get_mut(quadro.no as usize)
        {
            no.fim = self.pos;
            no.filhos = quadro.filhos;
        }
    }

    /// Para onde a máquina vai depois de um valor completo.
    fn depois(&self) -> Passo {
        if self.pilha.is_empty() { Passo::Fim } else { Passo::DepoisDoValor }
    }

    /// Lê uma cadeia: `pos` está na aspa de abertura e termina logo depois da de fechamento.
    fn ler_texto(&mut self) -> Result<(), ErroJson> {
        let abertura = self.pos;
        let mut i = abertura + 1;
        loop {
            let resto = self.bytes.get(i..).unwrap_or_default();
            let Some((rel, &b)) = resto.iter().enumerate().find(|&(_, &b)| b == b'"' || b == b'\\' || b < 0x20) else {
                return Err(self.erro(abertura, Motivo::TextoSemFechamento));
            };
            i += rel;
            if b == b'"' {
                self.pos = i + 1;
                return Ok(());
            }
            if b < 0x20 {
                return Err(self.controle_solto(abertura, i));
            }
            // a barra: o que vem depois tem de ser um escape do JSON
            match self.bytes.get(i + 1).copied() {
                None => return Err(self.erro(abertura, Motivo::TextoSemFechamento)),
                Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => i += 2,
                Some(b'u') => {
                    let hexa = self
                        .bytes
                        .get(i + 2..)
                        .unwrap_or_default()
                        .iter()
                        .take(4)
                        .take_while(|b| b.is_ascii_hexdigit())
                        .count();
                    if hexa < 4 {
                        let seguinte: String = self
                            .texto
                            .get(i + 2..)
                            .unwrap_or_default()
                            .chars()
                            .take(4)
                            .take_while(|&c| c != '"' && !c.is_control())
                            .collect();
                        return Err(self.erro(i, Motivo::EscapeInvalido(format!("\\u{}", descrever(&seguinte)))));
                    }
                    i += 6;
                }
                Some(controle) if controle < 0x20 => return Err(self.controle_solto(abertura, i + 1)),
                Some(_) => {
                    let c = self.texto.get(i + 1..).and_then(|t| t.chars().next()).unwrap_or(' ');
                    return Err(self.erro(i, Motivo::EscapeInvalido(format!("\\{}", descrever(&c.to_string())))));
                }
            }
        }
    }

    /// Um caractere de controle dentro de uma cadeia: a quebra de linha tem a frase própria (e aponta a aspa de
    /// abertura, na mesma linha); os outros apontam o próprio caractere.
    fn controle_solto(&self, abertura: usize, onde: usize) -> ErroJson {
        if matches!(self.bytes.get(onde), Some(b'\n' | b'\r')) {
            self.erro(abertura, Motivo::QuebraSolta)
        } else {
            self.erro(onde, Motivo::ControleSolto)
        }
    }

    /// O fim da palavra que começa em `de` (o próprio `de`, se não começa palavra nenhuma).
    fn fim_da_palavra(&self, de: usize) -> usize {
        self.texto[de..].char_indices().find(|&(_, c)| !e_de_palavra(c)).map_or(self.texto.len(), |(p, _)| de + p)
    }

    /// Um valor que não é cadeia, objeto nem lista: `true`, `false`, `null` ou um número. Todo o resto é erro,
    /// com a frase que mais ajuda.
    fn valor_simples(&mut self) -> Result<(), ErroJson> {
        let inicio = self.pos;
        let primeiro = self.bytes.get(inicio).copied().unwrap_or(b' ');
        let fim = self.fim_da_palavra(inicio);
        if fim == inicio {
            let c = self.texto[inicio..].chars().next().unwrap_or(' ');
            return Err(self.erro(inicio, Motivo::ValorInesperado(descrever(&c.to_string()))));
        }
        let palavra = &self.texto[inicio..fim];
        let tipo = match palavra {
            "true" | "false" => Tipo::Booleano,
            "null" => Tipo::Nulo,
            "NaN" | "Infinity" | "-Infinity" => return Err(self.erro(inicio, Motivo::NaoNumero)),
            _ => match tipo_do_numero(palavra) {
                Some(tipo) => tipo,
                None if matches!(primeiro, b'-' | b'+' | b'.' | b'0'..=b'9') => {
                    return Err(self.erro(inicio, Motivo::NumeroInvalido(limitar_trecho(palavra))));
                }
                None => {
                    return Err(self.erro(inicio, Motivo::ValorInesperado(descrever(&limitar_trecho(palavra)))));
                }
            },
        };
        self.novo_no(tipo, inicio, fim);
        self.pos = fim;
        Ok(())
    }

    /// O símbolo que está em `pos`, como a frase o mostra: a cadeia com as aspas, a palavra ou o caractere.
    fn simbolo_em(&self, pos: usize) -> String {
        if self.bytes.get(pos) == Some(&b'"') {
            let mut i = pos + 1;
            while let Some(&b) = self.bytes.get(i) {
                match b {
                    b'"' | b'\n' | b'\r' => break,
                    b'\\' => i += 2,
                    _ => i += 1,
                }
            }
            let fim = i.min(self.bytes.len());
            let dentro = self.texto.get(pos + 1..fim).unwrap_or_default();
            return format!("\"{}\"", limitar_trecho(dentro));
        }
        let fim = self.fim_da_palavra(pos);
        if fim > pos {
            return descrever(&limitar_trecho(&self.texto[pos..fim]));
        }
        descrever(&self.texto[pos..].chars().next().unwrap_or(' ').to_string())
    }

    /// O nome da chave pendente, sem as aspas, para a frase dos dois pontos.
    fn chave_pendente(&self) -> String {
        let chave = self.pilha.last().map(|q| q.chave.clone()).unwrap_or(0..0);
        let dentro = self.texto.get(chave.start + 1..chave.end.saturating_sub(1)).unwrap_or_default();
        limitar_chave(dentro)
    }

    /// Um `}` ou `]` onde devia vir um valor: depois de uma vírgula numa lista, a vírgula é que sobra.
    fn fecho_no_lugar_do_valor(&self, passo: Passo) -> ErroJson {
        match self.pilha.last() {
            Some(q) if passo == Passo::Valor && !q.objeto => self.erro(q.virgula, Motivo::VirgulaSobrando),
            _ => self.erro(self.pos, Motivo::EsperavaValor),
        }
    }

    fn executar(mut self) -> Result<Vec<No>, ErroJson> {
        if self.texto.starts_with('\u{FEFF}') {
            self.pos = '\u{FEFF}'.len_utf8();
        }
        let mut passo = Passo::Valor;
        loop {
            self.pular_espacos()?;
            let atual = self.bytes.get(self.pos).copied();
            match passo {
                Passo::Valor | Passo::PrimeiroDaLista => {
                    let Some(c) = atual else {
                        return Err(self.fim_inesperado());
                    };
                    match c {
                        b'{' => {
                            self.abrir(Tipo::Objeto);
                            passo = Passo::PrimeiroNome;
                        }
                        b'[' => {
                            self.abrir(Tipo::Lista);
                            passo = Passo::PrimeiroDaLista;
                        }
                        b']' if passo == Passo::PrimeiroDaLista => {
                            self.fechar();
                            passo = self.depois();
                        }
                        b'}' | b']' => return Err(self.fecho_no_lugar_do_valor(passo)),
                        b',' | b':' => return Err(self.erro(self.pos, Motivo::EsperavaValor)),
                        b'\'' => return Err(self.erro(self.pos, Motivo::AspasSimples)),
                        b'"' => {
                            let inicio = self.pos;
                            self.ler_texto()?;
                            self.novo_no(Tipo::Texto, inicio, self.pos);
                            passo = self.depois();
                        }
                        _ => {
                            self.valor_simples()?;
                            passo = self.depois();
                        }
                    }
                }
                Passo::PrimeiroNome | Passo::Nome => {
                    let Some(c) = atual else {
                        return Err(self.fim_inesperado());
                    };
                    match c {
                        b'}' if passo == Passo::PrimeiroNome => {
                            self.fechar();
                            passo = self.depois();
                        }
                        b'}' => {
                            let virgula = self.pilha.last().map_or(self.pos, |q| q.virgula);
                            return Err(self.erro(virgula, Motivo::VirgulaSobrando));
                        }
                        b'"' => {
                            let inicio = self.pos;
                            self.ler_texto()?;
                            let fim = self.pos;
                            if let Some(q) = self.pilha.last_mut() {
                                q.chave = inicio..fim;
                            }
                            passo = Passo::DoisPontos;
                        }
                        b'\'' => return Err(self.erro(self.pos, Motivo::AspasSimples)),
                        _ => return Err(self.erro(self.pos, Motivo::EsperavaNome)),
                    }
                }
                Passo::DoisPontos => match atual {
                    None => return Err(self.fim_inesperado()),
                    Some(b':') => {
                        self.pos += 1;
                        passo = Passo::Valor;
                    }
                    Some(_) => return Err(self.erro(self.pos, Motivo::FaltamDoisPontos(self.chave_pendente()))),
                },
                Passo::DepoisDoValor => {
                    let Some(c) = atual else {
                        return Err(self.fim_inesperado());
                    };
                    let objeto = self.pilha.last().is_some_and(|q| q.objeto);
                    match c {
                        b',' => {
                            let virgula = self.pos;
                            if let Some(q) = self.pilha.last_mut() {
                                q.virgula = virgula;
                            }
                            self.pos += 1;
                            passo = if objeto { Passo::Nome } else { Passo::Valor };
                        }
                        b'}' if objeto => {
                            self.fechar();
                            passo = self.depois();
                        }
                        b']' if !objeto => {
                            self.fechar();
                            passo = self.depois();
                        }
                        _ => {
                            let simbolo = self.simbolo_em(self.pos);
                            let motivo = if objeto {
                                Motivo::FaltaVirgulaObjeto(simbolo)
                            } else {
                                Motivo::FaltaVirgulaLista(simbolo)
                            };
                            return Err(self.erro(self.pos, motivo));
                        }
                    }
                }
                Passo::Fim => {
                    return match atual {
                        None => Ok(self.nos),
                        Some(_) => Err(self.erro(self.pos, Motivo::TextoDepois)),
                    };
                }
            }
        }
    }
}

/// O tipo de uma palavra que é um número JSON (`-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`), ou `None`.
fn tipo_do_numero(palavra: &str) -> Option<Tipo> {
    let b = palavra.as_bytes();
    let digitos = |de: usize| b.get(de..).unwrap_or_default().iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = usize::from(b.first() == Some(&b'-'));
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => i += digitos(i),
        _ => return None,
    }
    let mut inteiro = true;
    if b.get(i) == Some(&b'.') {
        let n = digitos(i + 1);
        if n == 0 {
            return None;
        }
        i += 1 + n;
        inteiro = false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let n = digitos(i);
        if n == 0 {
            return None;
        }
        i += n;
        inteiro = false;
    }
    (i == b.len()).then_some(if inteiro { Tipo::Inteiro } else { Tipo::Numero })
}
