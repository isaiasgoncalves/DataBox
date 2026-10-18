//! O formato de um CSV: o que as Propriedades mostram (separador, decimal, codificação, fim de linha,
//! "primeira linha é o cabeçalho") e o que a gravação precisa guardar para devolver o arquivo como estava
//! (estilo das aspas, quebra no fim do arquivo).

use comum::ErroBanco;

use super::codificacao::Codificacao;

/// Os separadores que a detecção considera, em ordem de preferência no desempate.
pub const SEPARADORES: [char; 4] = [';', ',', '\t', '|'];

/// A quebra de linha dos registros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FimDeLinha {
    /// `\r\n`, a do Windows (e a do Exportar).
    Crlf,
    /// `\n`.
    Lf,
    /// `\r` sozinho (Mac antigo).
    Cr,
}

impl FimDeLinha {
    pub const TODOS: [FimDeLinha; 3] = [FimDeLinha::Crlf, FimDeLinha::Lf, FimDeLinha::Cr];

    pub fn bytes(self) -> &'static [u8] {
        match self {
            FimDeLinha::Crlf => b"\r\n",
            FimDeLinha::Lf => b"\n",
            FimDeLinha::Cr => b"\r",
        }
    }

    /// O nome que as Propriedades mostram.
    pub fn rotulo(self) -> &'static str {
        match self {
            FimDeLinha::Crlf => "CRLF (Windows)",
            FimDeLinha::Lf => "LF (Unix)",
            FimDeLinha::Cr => "CR (Mac antigo)",
        }
    }

    pub(crate) fn de_bytes(bytes: &[u8]) -> Option<FimDeLinha> {
        match bytes {
            b"\r\n" => Some(FimDeLinha::Crlf),
            b"\n" => Some(FimDeLinha::Lf),
            b"\r" => Some(FimDeLinha::Cr),
            _ => None,
        }
    }
}

/// Quando um campo leva aspas ao ser gravado de novo. O que ficou como estava não é regravado: só as linhas
/// alteradas seguem o estilo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EstiloAspas {
    /// Só o campo com o separador, aspas ou quebra de linha (o do Exportar).
    Minimas,
    /// Todo campo que não é número (o `QUOTE_NONNUMERIC`).
    NaoNumericos,
    /// Todo campo (o `QUOTE_ALL`).
    Todas,
}

/// Como um arquivo CSV é lido e gravado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formato {
    pub codificacao: Codificacao,
    /// Um de `; , \t |` na detecção; qualquer ASCII que não seja aspa nem quebra de linha se for pedido.
    pub separador: char,
    pub fim_de_linha: FimDeLinha,
    pub aspas: EstiloAspas,
    /// A primeira linha tem os nomes das colunas.
    pub cabecalho: bool,
    /// `,` ou `.`: o decimal dos números (só para reconhecer tipo; o texto das células nunca é reescrito).
    pub decimal: char,
    /// O último registro tem a quebra de linha no fim do arquivo.
    pub termina_em_quebra: bool,
}

impl Formato {
    /// O de um arquivo novo: o do Exportar do DataBox (UTF-8 com BOM, `;`, CRLF, aspas mínimas, vírgula decimal).
    pub fn padrao() -> Formato {
        Formato {
            codificacao: Codificacao::Utf8ComBom,
            separador: ';',
            fim_de_linha: FimDeLinha::Crlf,
            aspas: EstiloAspas::Minimas,
            cabecalho: true,
            decimal: ',',
            termina_em_quebra: true,
        }
    }

    /// O separador como byte (a conferência está em [`separador_valido`]).
    pub(crate) fn sep(&self) -> u8 {
        u8::try_from(u32::from(self.separador)).unwrap_or(b';')
    }

    /// Gravar com `outro` produz, para uma linha que não mudou, os mesmos bytes que já estão no arquivo: dá para
    /// copiá-la como está.
    pub(crate) fn linhas_sao_copiaveis_para(&self, outro: &Formato) -> bool {
        self.codificacao.mesma_tabela(outro.codificacao)
            && self.separador == outro.separador
            && self.fim_de_linha == outro.fim_de_linha
            && self.aspas == outro.aspas
    }
}

/// O que se pede ao abrir; o que ficar `None` é detectado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpcoesAbertura {
    pub codificacao: Option<Codificacao>,
    pub separador: Option<char>,
    pub fim_de_linha: Option<FimDeLinha>,
    pub aspas: Option<EstiloAspas>,
    pub cabecalho: Option<bool>,
    pub decimal: Option<char>,
}

/// O separador como byte, se servir: um caractere ASCII que não seja aspa, quebra de linha nem espaço.
pub fn separador_valido(c: char) -> Result<u8, ErroBanco> {
    match u8::try_from(u32::from(c)) {
        Ok(b) if b.is_ascii() && b != b'"' && b != b'\r' && b != b'\n' && b != b' ' && b != 0 => Ok(b),
        _ => Err(ErroBanco::novo(format!(
            "O separador {c:?} não serve: use um caractere simples, como ; , ou tabulação."
        ))),
    }
}

/// O decimal, se for `,` ou `.`.
pub fn decimal_valido(c: char) -> Result<char, ErroBanco> {
    if c == ',' || c == '.' { Ok(c) } else { Err(ErroBanco::novo("O decimal deve ser a vírgula ou o ponto.")) }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn padrao_e_o_do_exportar() {
        let f = Formato::padrao();
        assert_eq!(f.codificacao, Codificacao::Utf8ComBom);
        assert_eq!((f.separador, f.decimal, f.fim_de_linha), (';', ',', FimDeLinha::Crlf));
        assert!(f.cabecalho && f.termina_em_quebra);
        assert_eq!(f.sep(), b';');
    }

    #[test]
    fn separador_e_decimal_validos() {
        for c in [';', ',', '\t', '|', ':', '~'] {
            assert!(separador_valido(c).is_ok(), "{c:?}");
        }
        for c in ['"', '\n', '\r', ' ', 'ç', '\0'] {
            assert!(separador_valido(c).is_err(), "{c:?}");
        }
        assert_eq!(separador_valido('\t').unwrap(), b'\t');
        assert!(decimal_valido(',').is_ok() && decimal_valido('.').is_ok() && decimal_valido(';').is_err());
    }

    #[test]
    fn fim_de_linha_bytes_e_rotulos() {
        for f in FimDeLinha::TODOS {
            assert_eq!(FimDeLinha::de_bytes(f.bytes()), Some(f));
        }
        assert_eq!(FimDeLinha::de_bytes(b""), None);
        assert_eq!(FimDeLinha::Crlf.rotulo(), "CRLF (Windows)");
    }

    #[test]
    fn copia_so_com_a_mesma_forma() {
        let a = Formato::padrao();
        let mut b = a.clone();
        b.codificacao = Codificacao::Utf8;
        b.cabecalho = false;
        b.termina_em_quebra = false;
        assert!(a.linhas_sao_copiaveis_para(&b), "BOM, cabeçalho e quebra final não mudam as linhas");
        for muda in [
            |f: &mut Formato| f.codificacao = Codificacao::Windows1252,
            |f: &mut Formato| f.separador = ',',
            |f: &mut Formato| f.fim_de_linha = FimDeLinha::Lf,
            |f: &mut Formato| f.aspas = EstiloAspas::Todas,
        ] {
            let mut c = a.clone();
            muda(&mut c);
            assert!(!a.linhas_sao_copiaveis_para(&c));
        }
    }
}
