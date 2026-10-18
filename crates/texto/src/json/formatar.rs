//! Formatar um JSON pelos símbolos: só os espaços e as quebras de linha mudam.
//!
//! Não passa por valores (nada de `f64`, nada de mapa): a ordem das chaves, as chaves repetidas, o texto de cada
//! número (`1.0`, `1e5`, `-0`) e o de cada cadeia (com os escapes como estavam) saem exatamente como entraram. `{}` e `[]`
//! vazios ficam juntos; depois de `:` vai um espaço; a quebra no fim do documento se mantém (se havia, há). O
//! documento tem de ser válido: com erro, devolve o erro.

use super::analise::validar;
use super::erro::ErroJson;

/// Até quantos níveis o recuo cresce. Abaixo disso o recuo para de aumentar: sem o limite, um documento de 100.000
/// níveis (200 KB) viraria 20 GB de espaços. Em JSON de verdade ninguém chega perto.
pub const NIVEL_MAXIMO_DO_RECUO: usize = 64;

/// O recuo de cada nível.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recuo {
    /// Esta quantidade de espaços.
    Espacos(u8),
    Tabulacao,
}

impl Recuo {
    /// Como o rodapé o mostra: "Espaços: 2" ou "Tabulação".
    pub fn rotulo(self) -> String {
        match self {
            Recuo::Espacos(n) => format!("Espaços: {n}"),
            Recuo::Tabulacao => "Tabulação".to_string(),
        }
    }

    fn unidade(self) -> String {
        match self {
            Recuo::Espacos(n) => " ".repeat(usize::from(n)),
            Recuo::Tabulacao => "\t".to_string(),
        }
    }
}

/// O recuo que o texto já usa: o da primeira linha recuada (uma tabulação, ou os espaços dela, de 1 a 8), ou `None`
/// se nenhuma linha tem recuo ("Sem recuo" no rodapé).
pub fn recuo_do_texto(texto: &str) -> Option<Recuo> {
    for linha in texto.lines() {
        let sem_recuo = linha.trim_start_matches([' ', '\t']);
        if sem_recuo.is_empty() || sem_recuo.len() == linha.len() {
            continue;
        }
        if linha.starts_with('\t') {
            return Some(Recuo::Tabulacao);
        }
        let espacos = linha.bytes().take_while(|&b| b == b' ').count();
        return Some(Recuo::Espacos(u8::try_from(espacos.clamp(1, 8)).unwrap_or(2)));
    }
    None
}

/// O recuo para formatar: o do texto, ou 2 espaços quando ele não tem nenhum.
pub fn detectar_recuo(texto: &str) -> Recuo {
    recuo_do_texto(texto).unwrap_or(Recuo::Espacos(2))
}

fn e_espaco(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Reescreve o documento com um valor por linha e o recuo pedido. Devolve o erro se o documento é inválido.
pub fn formatar(texto: &str, recuo: Recuo) -> Result<String, ErroJson> {
    if let Some(erro) = validar(texto) {
        return Err(erro);
    }
    let unidade = recuo.unidade();
    let b = texto.as_bytes();
    let n = b.len();
    let mut saida = String::with_capacity(texto.len() + texto.len() / 2 + 16);
    let mut nivel = 0usize;
    let mut i = if texto.starts_with('\u{FEFF}') { '\u{FEFF}'.len_utf8() } else { 0 };

    let quebrar = |saida: &mut String, nivel: usize| {
        saida.push('\n');
        for _ in 0..nivel.min(NIVEL_MAXIMO_DO_RECUO) {
            saida.push_str(&unidade);
        }
    };

    while let Some(&c) = b.get(i) {
        match c {
            _ if e_espaco(c) => i += 1,
            b'{' | b'[' => {
                let fecho = if c == b'{' { b'}' } else { b']' };
                let mut j = i + 1;
                while b.get(j).is_some_and(|&x| e_espaco(x)) {
                    j += 1;
                }
                saida.push(char::from(c));
                if b.get(j) == Some(&fecho) {
                    // vazio: fica junto
                    saida.push(char::from(fecho));
                    i = j + 1;
                } else {
                    nivel += 1;
                    quebrar(&mut saida, nivel);
                    i += 1;
                }
            }
            b'}' | b']' => {
                nivel = nivel.saturating_sub(1);
                quebrar(&mut saida, nivel);
                saida.push(char::from(c));
                i += 1;
            }
            b',' => {
                saida.push(',');
                quebrar(&mut saida, nivel);
                i += 1;
            }
            b':' => {
                saida.push_str(": ");
                i += 1;
            }
            b'"' => {
                let mut j = i + 1;
                while let Some(&x) = b.get(j) {
                    match x {
                        b'\\' => j += 2,
                        b'"' => {
                            j += 1;
                            break;
                        }
                        _ => j += 1,
                    }
                }
                saida.push_str(&texto[i..j.min(n)]);
                i = j;
            }
            _ => {
                // número, true, false ou null: até o próximo símbolo
                let mut j = i;
                while b
                    .get(j)
                    .is_some_and(|&x| !e_espaco(x) && !matches!(x, b',' | b':' | b'{' | b'}' | b'[' | b']' | b'"'))
                {
                    j += 1;
                }
                saida.push_str(&texto[i..j]);
                i = j;
            }
        }
    }

    // a quebra no fim se mantém: havia uma depois do último símbolo, então há
    let fim = texto.trim_end_matches([' ', '\t', '\n', '\r']).len();
    if texto[fim..].contains('\n') {
        saida.push('\n');
    }
    Ok(saida)
}
