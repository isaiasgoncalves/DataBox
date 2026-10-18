//! Erro de regra ou de banco com a frase para a pessoa (pt-BR) e o detalhe técnico à parte.

use std::fmt;
use std::io;
use std::path::Path;

/// Marca estável para a interface reagir sem depender da frase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codigo {
    /// Há um BEGIN aberto na Consulta SQL: grave ou descarte antes (ou peça para descartar).
    TransacaoAberta,
    /// O arquivo de destino já existe: pergunte e refaça com `substituir`.
    DestinoExiste,
    /// O banco aberto não é mais aquele em que o pedido foi feito: releia o estado e redesenhe.
    BancoTrocado,
    /// A leitura foi abandonada por outra mais nova do mesmo grupo: ignore a resposta.
    Substituido,
    /// A exportação foi interrompida a pedido; nenhum arquivo foi gravado.
    Cancelado,
}

impl Codigo {
    /// O texto estável ("transacao_aberta", …).
    pub fn como_str(self) -> &'static str {
        match self {
            Codigo::TransacaoAberta => "transacao_aberta",
            Codigo::DestinoExiste => "destino_existe",
            Codigo::BancoTrocado => "banco_trocado",
            Codigo::Substituido => "substituido",
            Codigo::Cancelado => "cancelado",
        }
    }
}

/// Erro com mensagem para a tela. `status` segue os códigos HTTP (400 regra, 403 só leitura, 404 não
/// existe, 409 conflito, 500 inesperado, 501 indisponível), para os testes e para a interface saber a natureza do erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroBanco {
    pub mensagem: String,
    pub detalhe: Option<String>,
    pub status: u16,
    pub codigo: Option<Codigo>,
}

pub type Resultado<T> = Result<T, ErroBanco>;

impl ErroBanco {
    /// Erro de regra (400) só com a mensagem.
    pub fn novo(mensagem: impl Into<String>) -> Self {
        ErroBanco { mensagem: mensagem.into(), detalhe: None, status: 400, codigo: None }
    }

    pub fn com_status(mensagem: impl Into<String>, status: u16) -> Self {
        ErroBanco { mensagem: mensagem.into(), detalhe: None, status, codigo: None }
    }

    pub fn detalhe(mut self, detalhe: impl Into<String>) -> Self {
        self.detalhe = Some(detalhe.into());
        self
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn codigo(mut self, codigo: Codigo) -> Self {
        self.codigo = Some(codigo);
        self
    }

    pub fn tem_codigo(&self, codigo: Codigo) -> bool {
        self.codigo == Some(codigo)
    }

    /// Antepõe uma frase de contexto ("Não foi possível salvar a linha. …").
    pub fn com_contexto(mut self, contexto: &str) -> Self {
        if !contexto.is_empty() {
            self.mensagem = format!("{contexto} {}", self.mensagem);
        }
        self
    }
}

impl fmt::Display for ErroBanco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detalhe {
            Some(d) if !d.is_empty() => write!(f, "{} ({d})", self.mensagem),
            _ => f.write_str(&self.mensagem),
        }
    }
}

impl std::error::Error for ErroBanco {}

impl From<rusqlite::Error> for ErroBanco {
    fn from(e: rusqlite::Error) -> Self {
        traduzir_sqlite(&e, None)
    }
}

impl From<io::Error> for ErroBanco {
    fn from(e: io::Error) -> Self {
        traduzir_io(&e, None, None)
    }
}

/// "clientes.email" ou "itens.pedido_id, itens.produto_id" → "email" / "pedido_id, produto_id".
fn colunas_da_mensagem(msg: &str) -> String {
    // Minúsculas só no ASCII: o texto mantém o tamanho em bytes, e a posição achada vale no original (com
    // to_lowercase, um "İ" antes do marcador muda o tamanho e o corte cairia fora de fronteira).
    let baixo = msg.to_ascii_lowercase();
    let Some(pos) = baixo.find("constraint failed: ") else {
        return String::new();
    };
    let resto = &msg[pos + "constraint failed: ".len()..];
    resto
        .split(',')
        .map(|p| {
            let p = p.trim();
            p.split_once('.').map_or(p, |(_, coluna)| coluna)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn depois_dos_dois_pontos(original: &str) -> &str {
    original.split_once(':').map_or(original, |(_, r)| r).trim()
}

/// As regras por texto da mensagem, que valem para qualquer erro.
/// None quando nenhuma casa.
fn por_mensagem(original: &str) -> Option<(String, u16)> {
    let baixo = original.to_lowercase();
    let tem = |s: &str| baixo.contains(s);
    let r = if tem("unique constraint failed") {
        let cols = colunas_da_mensagem(original);
        if cols.is_empty() {
            "Já existe uma linha com esse valor; a coluna não aceita repetição.".to_string()
        } else {
            format!("Já existe uma linha com esse valor em {cols}; a coluna não aceita repetição.")
        }
    } else if tem("not null constraint failed") {
        let cols = colunas_da_mensagem(original);
        if cols.is_empty() {
            "Uma coluna obrigatória ficou sem valor.".to_string()
        } else {
            format!("A coluna {cols} não aceita valor nulo.")
        }
    } else if tem("foreign key constraint failed") {
        "A alteração quebra uma chave estrangeira: o valor não existe na tabela referenciada, \
         ou outra tabela ainda aponta para esta linha."
            .to_string()
    } else if tem("check constraint failed") {
        let regra = if original.contains(':') { depois_dos_dois_pontos(original) } else { "" };
        if regra.is_empty() {
            "O valor não passa na regra de verificação da tabela.".to_string()
        } else {
            format!("O valor não passa na regra de verificação da tabela ({regra}).")
        }
    } else if tem("primary key must be unique") {
        "Já existe uma linha com essa chave primária.".to_string()
    } else if tem("datatype mismatch") {
        "Tipo de valor incompatível com a coluna (por exemplo, texto numa chave primária inteira).".to_string()
    } else if tem("database is locked") || tem("database table is locked") {
        return Some(("O banco está em uso por outro programa. Feche-o lá e tente de novo.".to_string(), 409));
    } else if tem("readonly database") || tem("read-only") {
        return Some(("O banco foi aberto só para leitura; nada pode ser gravado nele.".to_string(), 403));
    } else if tem("file is not a database") || tem("file is encrypted") {
        "O arquivo não é um banco SQLite (ou está criptografado).".to_string()
    } else if tem("unable to open database") {
        "Não foi possível abrir o arquivo do banco. Verifique se ele existe e se você tem acesso à pasta.".to_string()
    } else if tem("no such table") {
        format!("A tabela não existe: {}.", depois_dos_dois_pontos(original))
    } else if tem("no such column") {
        format!("A coluna não existe: {}.", depois_dos_dois_pontos(original))
    } else if tem("already exists") {
        format!("Já existe um objeto com esse nome no banco ({original}).")
    } else if tem("syntax error") || tem("incomplete input") || tem("unrecognized token") {
        format!("Erro no comando SQL: {original}.")
    } else if tem("disk i/o error") || tem("disk is full") {
        return Some((
            "Erro de gravação no disco. Verifique o espaço livre e se o arquivo não está numa pasta de rede \
             instável."
                .to_string(),
            500,
        ));
    } else {
        return None;
    };
    Some((r, 400))
}

/// Erro do SQLite (rusqlite) → [`ErroBanco`] com frase em português. `contexto` antecede a frase.
/// Um [`ErroBanco`] que atravessou um callback do rusqlite volta como estava (sem contexto).
pub fn traduzir_sqlite(e: &rusqlite::Error, contexto: Option<&str>) -> ErroBanco {
    if let rusqlite::Error::ToSqlConversionFailure(interno) = e
        && let Some(erro) = interno.downcast_ref::<ErroBanco>()
    {
        return erro.clone();
    }
    let original = e.to_string();
    let (mensagem, status) = por_mensagem(&original).unwrap_or_else(|| match e {
        rusqlite::Error::IntegralValueOutOfRange(..) => {
            ("Número grande demais para o SQLite (o inteiro vai até 64 bits).".to_string(), 400)
        }
        _ => (format!("O SQLite recusou a operação: {original}."), 400),
    });
    let erro = ErroBanco { mensagem, detalhe: Some(original), status, codigo: None };
    match contexto {
        Some(c) => erro.com_contexto(c),
        None => erro,
    }
}

/// Erro de arquivo → [`ErroBanco`]. `caminho` entra na frase de "não encontrado" (o `io::Error` não guarda
/// o nome do arquivo).
pub fn traduzir_io(e: &io::Error, caminho: Option<&Path>, contexto: Option<&str>) -> ErroBanco {
    let original = e.to_string();
    let (mensagem, status) = por_mensagem(&original).unwrap_or_else(|| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => {
            let nome = caminho.map(|c| c.display().to_string()).unwrap_or_else(|| original.clone());
            (format!("Arquivo ou pasta não encontrado: {nome}."), 404)
        }
        io::ErrorKind::PermissionDenied => {
            ("Sem permissão para acessar o arquivo. Ele pode estar aberto em outro programa.".to_string(), 403)
        }
        _ => (format!("Erro ao acessar o arquivo: {original}."), 400),
    });
    let erro = ErroBanco { mensagem, detalhe: Some(original), status, codigo: None };
    match contexto {
        Some(c) => erro.com_contexto(c),
        None => erro,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn falha(msg: &str) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some(msg.to_string()),
        )
    }

    #[test]
    fn unica_com_colunas() {
        let e = traduzir_sqlite(&falha("UNIQUE constraint failed: itens.pedido_id, itens.produto_id"), None);
        assert_eq!(
            e.mensagem,
            "Já existe uma linha com esse valor em pedido_id, produto_id; a coluna não aceita repetição."
        );
        assert_eq!(e.status, 400);
        assert_eq!(e.detalhe.as_deref(), Some("UNIQUE constraint failed: itens.pedido_id, itens.produto_id"));
    }

    #[test]
    fn nao_nulo_e_contexto() {
        let e = traduzir_sqlite(
            &falha("NOT NULL constraint failed: clientes.nome"),
            Some("Não foi possível salvar a linha."),
        );
        assert_eq!(e.mensagem, "Não foi possível salvar a linha. A coluna nome não aceita valor nulo.");
    }

    #[test]
    fn travado_e_409_e_so_leitura_403() {
        assert_eq!(traduzir_sqlite(&falha("database is locked"), None).status, 409);
        assert_eq!(traduzir_sqlite(&falha("attempt to write a readonly database"), None).status, 403);
    }

    #[test]
    fn letra_que_muda_de_tamanho_antes_do_marcador_nao_derruba() {
        let e = traduzir_sqlite(&falha("İİİ UNIQUE constraint failed: t.a"), None);
        assert_eq!(e.mensagem, "Já existe uma linha com esse valor em a; a coluna não aceita repetição.");
        let e = traduzir_sqlite(&falha("\u{212A}elvin NOT NULL constraint failed: t.b"), None);
        assert_eq!(e.mensagem, "A coluna b não aceita valor nulo.");
    }

    #[test]
    fn verificacao_com_regra() {
        let e = traduzir_sqlite(&falha("CHECK constraint failed: preco >= 0"), None);
        assert_eq!(e.mensagem, "O valor não passa na regra de verificação da tabela (preco >= 0).");
    }

    #[test]
    fn desconhecido_cai_no_generico() {
        let e = traduzir_sqlite(&falha("interrupted"), None);
        assert_eq!(e.mensagem, "O SQLite recusou a operação: interrupted.");
    }

    #[test]
    fn io_nao_encontrado_404() {
        let e = traduzir_io(&io::Error::from(io::ErrorKind::NotFound), Some(Path::new("C:\\x\\a.db")), None);
        assert_eq!(e.status, 404);
        assert_eq!(e.mensagem, "Arquivo ou pasta não encontrado: C:\\x\\a.db.");
    }

    #[test]
    fn erro_banco_atravessa_o_rusqlite() {
        let original = ErroBanco::novo("Regra.").codigo(Codigo::Cancelado).status(409);
        let via: rusqlite::Error = original.clone().into();
        assert_eq!(ErroBanco::from(via), original);
    }

    #[test]
    fn exibicao_com_detalhe() {
        let e = ErroBanco::novo("Frase.").detalhe("técnico");
        assert_eq!(e.to_string(), "Frase. (técnico)");
        assert_eq!(Codigo::TransacaoAberta.como_str(), "transacao_aberta");
    }
}
