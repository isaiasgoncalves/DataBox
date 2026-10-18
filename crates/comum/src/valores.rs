//! Valores que circulam entre os crates (ARQUITETURA.md, §3).

use std::path::PathBuf;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use crate::erro::ErroBanco;

/// Caracteres de texto que uma célula da grade carrega antes de cortar (o resto fica na janela da linha).
pub const LIMITE_TEXTO: usize = 300;

/// Bytes do começo de um BLOB que a célula guarda (mostrados em hexadecimal).
const BYTES_INICIO_BLOB: usize = 24;

/// Cita um identificador SQL: `a"b` vira `"a""b"`.
pub fn q(ident: &str) -> String {
    let mut s = String::with_capacity(ident.len() + 2);
    s.push('"');
    s.push_str(&ident.replace('"', "\"\""));
    s.push('"');
    s
}

/// Afinidade de tipo do SQLite (regras da seção 3.1 de https://sqlite.org/datatype3.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Afinidade {
    Integer,
    Text,
    Blob,
    Real,
    Numeric,
}

impl Afinidade {
    pub fn como_str(self) -> &'static str {
        match self {
            Afinidade::Integer => "INTEGER",
            Afinidade::Text => "TEXT",
            Afinidade::Blob => "BLOB",
            Afinidade::Real => "REAL",
            Afinidade::Numeric => "NUMERIC",
        }
    }

    /// INTEGER, REAL ou NUMERIC: a grade alinha à direita e a entrada passa pela detecção numérica.
    pub fn numerica(self) -> bool {
        matches!(self, Afinidade::Integer | Afinidade::Real | Afinidade::Numeric)
    }
}

/// INTEGER, TEXT, BLOB, REAL ou NUMERIC, na ordem de precedência das regras do SQLite.
pub fn afinidade(tipo_declarado: Option<&str>) -> Afinidade {
    let t = tipo_declarado.unwrap_or("").to_uppercase();
    if t.contains("INT") {
        Afinidade::Integer
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        Afinidade::Text
    } else if t.contains("BLOB") || t.trim().is_empty() {
        Afinidade::Blob
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        Afinidade::Real
    } else {
        Afinidade::Numeric
    }
}

/// Valor exato como está no arquivo.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Nulo,
    Inteiro(i64),
    Real(f64),
    Texto(String),
    /// Texto com UTF-8 inválido (gravado por outro programa): os bytes do arquivo. Na tela aparece com "�"
    /// no lugar dos bytes ruins; numa chave, a comparação usa estes bytes (`CAST(? AS TEXT)`).
    TextoBruto(Vec<u8>),
    Blob(Vec<u8>),
}

impl Valor {
    /// Lê um valor do SQLite sem perder nada (texto inválido vira [`Valor::TextoBruto`]).
    pub fn de_ref(v: ValueRef<'_>) -> Valor {
        match v {
            ValueRef::Null => Valor::Nulo,
            ValueRef::Integer(i) => Valor::Inteiro(i),
            ValueRef::Real(r) => Valor::Real(r),
            ValueRef::Text(b) => match std::str::from_utf8(b) {
                Ok(s) => Valor::Texto(s.to_string()),
                Err(_) => Valor::TextoBruto(b.to_vec()),
            },
            ValueRef::Blob(b) => Valor::Blob(b.to_vec()),
        }
    }

    pub fn e_nulo(&self) -> bool {
        matches!(self, Valor::Nulo)
    }

    /// Texto para mostrar ou copiar: NULL → "", BLOB → "", texto inválido com "�".
    pub fn texto_exibicao(&self) -> String {
        match self {
            Valor::Nulo | Valor::Blob(_) => String::new(),
            Valor::Inteiro(i) => i.to_string(),
            Valor::Real(r) => formatar_real(*r),
            Valor::Texto(s) => s.clone(),
            Valor::TextoBruto(b) => String::from_utf8_lossy(b).into_owned(),
        }
    }
}

/// Real no texto mais curto que volta ao mesmo número ("2.5", "10.0", "1e-05", "1e+16"), com "Infinity", "-Infinity"
/// e "NaN" nos especiais.
pub fn formatar_real(r: f64) -> String {
    if r.is_nan() {
        return "NaN".into();
    }
    if r.is_infinite() {
        return if r > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    // `{:e}` dá os dígitos mais curtos que voltam ao mesmo número: "1.2345e-5".
    let cientifico = format!("{r:e}");
    let (mantissa, expoente) = cientifico.split_once('e').unwrap_or((&cientifico, "0"));
    let expoente: i32 = expoente.parse().unwrap_or(0);
    if (-4..16).contains(&expoente) {
        let s = format!("{r}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        let sinal = if expoente < 0 { '-' } else { '+' };
        format!("{mantissa}e{sinal}{:02}", expoente.abs())
    }
}

impl ToSql for Valor {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Valor::Nulo => ToSqlOutput::Borrowed(ValueRef::Null),
            Valor::Inteiro(i) => ToSqlOutput::Borrowed(ValueRef::Integer(*i)),
            Valor::Real(r) => ToSqlOutput::Borrowed(ValueRef::Real(*r)),
            Valor::Texto(s) => ToSqlOutput::Borrowed(ValueRef::Text(s.as_bytes())),
            Valor::TextoBruto(b) => ToSqlOutput::Borrowed(ValueRef::Text(b)),
            Valor::Blob(b) => ToSqlOutput::Borrowed(ValueRef::Blob(b)),
        })
    }
}

impl FromSql for Valor {
    fn column_result(v: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(Valor::de_ref(v))
    }
}

/// O que uma célula da grade (ou do resultado de uma consulta) mostra: texto longo e BLOB vêm resumidos.
#[derive(Debug, Clone, PartialEq)]
pub enum Celula {
    Nulo,
    Inteiro(i64),
    /// Inclui infinito e NaN (a tela escreve "Infinity", "-Infinity", "NaN").
    Real(f64),
    /// Texto inteiro (até o limite pedido). Texto com UTF-8 inválido chega com "�".
    Texto(String),
    /// Texto maior que o limite: os primeiros `limite` caracteres e o tamanho total, em caracteres.
    TextoCortado {
        inicio: String,
        tamanho: usize,
    },
    /// BLOB: tamanho em bytes e os primeiros bytes (até 24), para mostrar em hexadecimal.
    Blob {
        tamanho: usize,
        inicio: Vec<u8>,
    },
}

impl Celula {
    /// Lê uma célula direto do SQLite, copiando só o que vai aparecer. `limite` None = texto inteiro.
    pub fn de_ref(v: ValueRef<'_>, limite: Option<usize>) -> Celula {
        match v {
            ValueRef::Null => Celula::Nulo,
            ValueRef::Integer(i) => Celula::Inteiro(i),
            ValueRef::Real(r) => Celula::Real(r),
            ValueRef::Text(b) => Celula::de_texto(&String::from_utf8_lossy(b), limite),
            ValueRef::Blob(b) => {
                Celula::Blob { tamanho: b.len(), inicio: b[..b.len().min(BYTES_INICIO_BLOB)].to_vec() }
            }
        }
    }

    /// Valor exato → célula.
    pub fn de_valor(v: &Valor, limite: Option<usize>) -> Celula {
        match v {
            Valor::Nulo => Celula::Nulo,
            Valor::Inteiro(i) => Celula::Inteiro(*i),
            Valor::Real(r) => Celula::Real(*r),
            Valor::Texto(s) => Celula::de_texto(s, limite),
            Valor::TextoBruto(b) => Celula::de_texto(&String::from_utf8_lossy(b), limite),
            Valor::Blob(b) => Celula::Blob { tamanho: b.len(), inicio: b[..b.len().min(BYTES_INICIO_BLOB)].to_vec() },
        }
    }

    fn de_texto(s: &str, limite: Option<usize>) -> Celula {
        match limite {
            Some(lim) => match s.char_indices().nth(lim) {
                None => Celula::Texto(s.to_string()),
                Some((corte, _)) => Celula::TextoCortado { inicio: s[..corte].to_string(), tamanho: s.chars().count() },
            },
            None => Celula::Texto(s.to_string()),
        }
    }

    pub fn e_nulo(&self) -> bool {
        matches!(self, Celula::Nulo)
    }

    /// Número (INTEGER ou REAL): a grade alinha à direita, em fonte tabular.
    pub fn e_numero(&self) -> bool {
        matches!(self, Celula::Inteiro(_) | Celula::Real(_))
    }

    /// Texto cru para um campo de edição ou para copiar (o `textoCelula` do JavaScript): NULL e BLOB → "",
    /// texto cortado → só o começo (quem edita texto cortado abre a janela da linha, que lê o texto inteiro).
    pub fn texto(&self) -> String {
        match self {
            Celula::Nulo | Celula::Blob { .. } => String::new(),
            Celula::Inteiro(i) => i.to_string(),
            Celula::Real(r) => formatar_real(*r),
            Celula::Texto(s) => s.clone(),
            Celula::TextoCortado { inicio, .. } => inicio.clone(),
        }
    }
}

/// Identidade de uma linha, opaca para a interface: ela guarda e devolve (§3.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Chave {
    /// Tabela com rowid.
    Rowid(i64),
    /// Tabela WITHOUT ROWID (ou com rowid cujos três apelidos são colunas): os valores exatos da chave
    /// primária, na ordem da chave. Texto inválido vai como [`Valor::TextoBruto`] e BLOB inteiro.
    Pk(Vec<(String, Valor)>),
}

/// O que a pessoa manda gravar numa coluna (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    /// NULL.
    Nulo,
    /// Texto digitado: coagido pela afinidade da coluna (número quando parece número, vírgula decimal e
    /// milhar pt-BR aceitos; vazio em coluna numérica vira NULL). Ver `banco::coagir`.
    Texto(String),
    Inteiro(i64),
    Real(f64),
    /// Texto gravado como texto, sem coagir (o `{"$texto": …}`).
    TextoLiteral(String),
    /// Coluna fora do INSERT: vale o DEFAULT / autoincremento. Num UPDATE é erro.
    Padrao,
    /// Coluna fora do UPDATE.
    Manter,
    /// O conteúdo deste arquivo, como BLOB.
    Arquivo(PathBuf),
    /// BLOB.
    Blob(Vec<u8>),
}

impl Entrada {
    /// Entrada que grava exatamente este valor (sem coerção).
    pub fn de_valor(v: Valor) -> Entrada {
        match v {
            Valor::Nulo => Entrada::Nulo,
            Valor::Inteiro(i) => Entrada::Inteiro(i),
            Valor::Real(r) => Entrada::Real(r),
            Valor::Texto(s) => Entrada::TextoLiteral(s),
            Valor::TextoBruto(b) => Entrada::TextoLiteral(String::from_utf8_lossy(&b).into_owned()),
            Valor::Blob(b) => Entrada::Blob(b),
        }
    }
}

impl From<ErroBanco> for rusqlite::Error {
    /// Deixa um [`ErroBanco`] atravessar um callback do rusqlite (ex.: dentro de `query_map`); a volta para
    /// [`ErroBanco`] (`From<rusqlite::Error>`) o devolve intacto.
    fn from(e: ErroBanco) -> Self {
        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cita_identificador() {
        assert_eq!(q("a\"b"), "\"a\"\"b\"");
        assert_eq!(q("clientes"), "\"clientes\"");
    }

    #[test]
    fn afinidades() {
        assert_eq!(afinidade(Some("INTEGER")), Afinidade::Integer);
        assert_eq!(afinidade(Some("varchar(255)")), Afinidade::Text);
        assert_eq!(afinidade(Some("")), Afinidade::Blob);
        assert_eq!(afinidade(None), Afinidade::Blob);
        assert_eq!(afinidade(Some("DOUBLE PRECISION")), Afinidade::Real);
        assert_eq!(afinidade(Some("DECIMAL(10,2)")), Afinidade::Numeric);
        assert_eq!(afinidade(Some("FLOATING POINT")), Afinidade::Integer); // "INT" ganha, como no SQLite
    }

    #[test]
    fn celula_corta_por_caracteres() {
        let c = Celula::de_valor(&Valor::Texto("ação".repeat(100)), Some(300));
        match c {
            Celula::TextoCortado { inicio, tamanho } => {
                assert_eq!(inicio.chars().count(), 300);
                assert_eq!(tamanho, 400);
            }
            outra => panic!("esperava corte, veio {outra:?}"),
        }
        assert_eq!(Celula::de_valor(&Valor::Texto("curto".into()), Some(300)), Celula::Texto("curto".into()));
    }

    #[test]
    fn celula_blob_e_texto_invalido() {
        let c = Celula::de_ref(ValueRef::Blob(&[7u8; 40]), Some(300));
        assert_eq!(c, Celula::Blob { tamanho: 40, inicio: vec![7u8; 24] });
        let v = Valor::de_ref(ValueRef::Text(b"S\xe3o Paulo"));
        assert_eq!(v, Valor::TextoBruto(b"S\xe3o Paulo".to_vec()));
        assert_eq!(Celula::de_valor(&v, None), Celula::Texto("S\u{fffd}o Paulo".into()));
    }

    #[test]
    fn reais_no_texto_mais_curto() {
        assert_eq!(formatar_real(2.5), "2.5");
        assert_eq!(formatar_real(10.0), "10.0");
        assert_eq!(formatar_real(f64::INFINITY), "Infinity");
        assert_eq!(formatar_real(-0.5), "-0.5");
        assert_eq!(formatar_real(0.0001), "0.0001");
        assert_eq!(formatar_real(0.00001), "1e-05");
        assert_eq!(formatar_real(1e15), "1000000000000000.0");
        assert_eq!(formatar_real(1e16), "1e+16");
        assert_eq!(formatar_real(1.5e300), "1.5e+300");
        assert_eq!(formatar_real(-2.5e-7), "-2.5e-07");
    }

    #[test]
    fn valor_vai_e_volta_pelo_sqlite() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let bruto = Valor::TextoBruto(b"S\xe3o".to_vec());
        let (tipo, volta): (String, Valor) =
            conn.query_row("select typeof(?1), ?1", [&bruto], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(tipo, "text");
        assert_eq!(volta, bruto);
    }
}
