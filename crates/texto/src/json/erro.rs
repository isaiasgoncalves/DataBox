//! O erro de JSON inválido: onde está (linha, coluna e byte) e a frase em português do motivo.

use std::fmt;

use crate::posicao::linha_e_coluna;

/// Quantos caracteres de um trecho do texto cabem numa frase de erro (o resto vira "…").
const LIMITE_DO_TRECHO: usize = 30;
const LIMITE_DA_CHAVE: usize = 40;

/// O JSON está inválido: aponta o começo do símbolo que **estraga** (no exemplo da prancheta 15, o `"arquivo"`
/// que veio sem a vírgula antes). O `Display` é a frase inteira, "JSON inválido na linha 14, coluna 7: …".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroJson {
    /// A linha do erro, de 1.
    pub linha: usize,
    /// A coluna do erro, de 1, em caracteres (a tabulação conta 1).
    pub coluna: usize,
    /// O byte do erro no texto analisado.
    pub byte: usize,
    /// O motivo, em português ("falta uma vírgula ou um } antes de \"arquivo\"").
    pub motivo: String,
}

impl ErroJson {
    pub(crate) fn em(texto: &str, byte: usize, motivo: Motivo) -> ErroJson {
        let byte = byte.min(texto.len());
        let (linha, coluna) = linha_e_coluna(texto, byte);
        ErroJson { linha, coluna, byte, motivo: motivo.frase() }
    }
}

impl fmt::Display for ErroJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON inválido na linha {}, coluna {}: {}", self.linha, self.coluna, self.motivo)
    }
}

impl std::error::Error for ErroJson {}

/// Os motivos de erro. Os 16 da interface e o 17º, o caractere de controle solto
/// (a tabulação dentro do texto, que a tabela não cobria).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Motivo {
    /// Num objeto, depois de um valor, veio outra coisa que não `,` nem `}`. O símbolo já vem como escrito.
    FaltaVirgulaObjeto(String),
    /// Numa lista, depois de um valor, veio outra coisa que não `,` nem `]`.
    FaltaVirgulaLista(String),
    EsperavaNome,
    /// O texto da chave, sem as aspas.
    FaltamDoisPontos(String),
    EsperavaValor,
    ValorInesperado(String),
    TextoSemFechamento,
    QuebraSolta,
    /// O escape como escrito, com a barra ("\q", "\u12").
    EscapeInvalido(String),
    NumeroInvalido(String),
    VirgulaSobrando,
    TerminaAntes,
    TextoDepois,
    Comentario,
    AspasSimples,
    NaoNumero,
    ControleSolto,
}

impl Motivo {
    pub(crate) fn frase(&self) -> String {
        match self {
            Motivo::FaltaVirgulaObjeto(s) => format!("falta uma vírgula ou um }} antes de {s}"),
            Motivo::FaltaVirgulaLista(s) => format!("falta uma vírgula ou um ] antes de {s}"),
            Motivo::EsperavaNome => "esperava um nome entre aspas".to_string(),
            Motivo::FaltamDoisPontos(chave) => format!("faltam os dois pontos depois de \"{chave}\""),
            Motivo::EsperavaValor => "esperava um valor".to_string(),
            Motivo::ValorInesperado(t) => format!("valor inesperado: {t}"),
            Motivo::TextoSemFechamento => "texto sem aspas de fechamento".to_string(),
            Motivo::QuebraSolta => "o texto tem uma quebra de linha solta; escreva \\n".to_string(),
            Motivo::EscapeInvalido(e) => format!("escape inválido: {e}"),
            Motivo::NumeroInvalido(t) => format!("número inválido: {t}"),
            Motivo::VirgulaSobrando => "vírgula sobrando antes de } ou ]".to_string(),
            Motivo::TerminaAntes => "o documento termina antes de fechar { ou [".to_string(),
            Motivo::TextoDepois => "há texto depois do fim do documento".to_string(),
            Motivo::Comentario => "JSON não aceita comentários".to_string(),
            Motivo::AspasSimples => "use aspas duplas, não simples".to_string(),
            Motivo::NaoNumero => "NaN e Infinity não existem em JSON".to_string(),
            Motivo::ControleSolto => "o texto tem um caractere de controle solto; escreva-o como \\uXXXX".to_string(),
        }
    }
}

/// Encurta um trecho para caber numa frase (corta em caracteres, nunca no meio de um).
pub(crate) fn limitar(trecho: &str, maximo: usize) -> String {
    let mut chars = trecho.chars();
    let inicio: String = chars.by_ref().take(maximo).collect();
    if chars.next().is_some() { format!("{inicio}…") } else { inicio }
}

pub(crate) fn limitar_trecho(trecho: &str) -> String {
    limitar(trecho, LIMITE_DO_TRECHO)
}

pub(crate) fn limitar_chave(chave: &str) -> String {
    limitar(chave, LIMITE_DA_CHAVE)
}

/// O trecho como a frase o mostra: o caractere invisível (espaço fora do ASCII, controle) aparece como
/// `U+00A0`, para a pessoa ver o que está lá.
pub(crate) fn descrever(trecho: &str) -> String {
    let mut saida = String::with_capacity(trecho.len());
    for c in trecho.chars() {
        if c.is_control() || (c.is_whitespace() && c != ' ') || c == '\u{FEFF}' || c == '\u{200B}' {
            saida.push_str(&format!("U+{:04X}", u32::from(c)));
        } else {
            saida.push(c);
        }
    }
    saida
}
