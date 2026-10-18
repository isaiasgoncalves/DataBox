//! Tradução dos erros do rusqlite e as mensagens fixas do módulo .

use comum::{Codigo, ErroBanco, Resultado, traduzir_sqlite};

pub(crate) const MSG_SOMENTE_LEITURA: &str = "O banco foi aberto só para leitura; nada pode ser gravado nele.";
pub(crate) const MSG_SALVAR: &str = "Não foi possível salvar a linha.";
pub(crate) const MSG_SO_SELECT: &str = "Só dá para exportar o resultado de uma consulta SELECT.";
pub(crate) const MSG_PROTEGIDO: &str = "O arquivo está protegido contra gravação (atributo somente leitura ou permissão \
     da pasta); ele foi aberto só para leitura.";
pub(crate) const MSG_JOURNAL_QUENTE: &str = "O arquivo ficou com uma transação interrompida (arquivo -journal ao lado \
     dele), e só leitura não deixa o SQLite recuperá-lo. Abra-o uma vez para edição: o SQLite desfaz a transação \
     interrompida e depois ele abre normalmente, até só para leitura.";
pub(crate) const MSG_EXPORTANDO: &str = "Há uma exportação em andamento lendo este banco; a edição fica disponível \
     quando ela terminar. Tente de novo em seguida.";
pub(crate) const MSG_SEM_IDENTIDADE: &str = "Esta linha não tem identidade: a chave primária dela é nula (NULL), e mais \
     de uma linha pode ter a mesma chave. Edite-a pela Consulta SQL.";
pub(crate) const MSG_FECHADO: &str = "O banco já foi fechado. Abra-o de novo.";

/// Mensagem do erro 409 `substituido` (a leitura foi abandonada por outra mais nova do mesmo grupo).
pub const MSG_SUBSTITUIDA: &str = "Leitura substituída por outra mais nova.";

/// Mensagem do comando da Consulta SQL cortado pelo `interromper_consulta` .
pub const MSG_INTERROMPIDA: &str = "A consulta foi interrompida.";

pub(crate) fn msg_transacao(acao: &str) -> String {
    format!(
        "Há uma transação aberta na Consulta SQL; grave (COMMIT) ou descarte (ROLLBACK) antes de {acao}. \
         Enquanto ela estiver aberta, nada do que foi mudado desde o BEGIN está no arquivo."
    )
}

pub(crate) fn erro_substituida() -> ErroBanco {
    ErroBanco::com_status(MSG_SUBSTITUIDA, 409).codigo(Codigo::Substituido)
}

pub(crate) fn erro_fechado() -> ErroBanco {
    ErroBanco::com_status(MSG_FECHADO, 409)
}

/// O rusqlite (com `modern_sqlite`) junta ao texto do SQLite o comando e a posição do erro ("… in SELEC x at
/// offset 0"). A mensagem para a pessoa usa só o texto do SQLite.
pub(crate) fn normalizar(e: rusqlite::Error) -> rusqlite::Error {
    match e {
        rusqlite::Error::SqlInputError { error, msg, .. } => rusqlite::Error::SqliteFailure(error, Some(msg)),
        outro => outro,
    }
}

/// Tradução de um erro do rusqlite.
///
/// UNIQUE e NOT NULL (as duas regras que tiram as colunas do texto) são traduzidas aqui; o resto vai para
/// `comum::traduzir_sqlite`. O `comum` acha "constraint failed: " no texto em minúsculas e corta o original na
/// mesma posição em bytes: um caractere que muda de tamanho em minúscula ('İ' passa de 2 para 3 bytes, o
/// sinal de kelvin de 3 para 1) numa mensagem de RAISE gravada num gatilho do arquivo dava pânico (corte fora
/// da fronteira do caractere ou além do fim) ou tirava as colunas erradas.
pub(crate) fn traduzir(e: rusqlite::Error, contexto: Option<&str>) -> ErroBanco {
    let e = normalizar(e);
    // um ErroBanco que atravessou um callback do rusqlite volta como estava (o comum cuida)
    let atravessou =
        matches!(&e, rusqlite::Error::ToSqlConversionFailure(i) if i.downcast_ref::<ErroBanco>().is_some());
    if !atravessou {
        let original = e.to_string();
        if let Some(mensagem) = traduzir_restricao(&original) {
            let erro = ErroBanco { mensagem, detalhe: Some(original), status: 400, codigo: None };
            return match contexto {
                Some(c) => erro.com_contexto(c),
                None => erro,
            };
        }
    }
    traduzir_sqlite(&e, contexto)
}

/// As duas primeiras regras de `traduzir_erro` (UNIQUE, depois NOT NULL), com as mesmas frases. None quando o
/// texto não é de nenhuma delas.
fn traduzir_restricao(original: &str) -> Option<String> {
    let baixo = original.to_lowercase();
    if baixo.contains("unique constraint failed") {
        let cols = colunas_da_mensagem(original);
        Some(if cols.is_empty() {
            "Já existe uma linha com esse valor; a coluna não aceita repetição.".to_string()
        } else {
            format!("Já existe uma linha com esse valor em {cols}; a coluna não aceita repetição.")
        })
    } else if baixo.contains("not null constraint failed") {
        let cols = colunas_da_mensagem(original);
        Some(if cols.is_empty() {
            "Uma coluna obrigatória ficou sem valor.".to_string()
        } else {
            format!("A coluna {cols} não aceita valor nulo.")
        })
    } else {
        None
    }
}

const MARCADOR: &str = "constraint failed: ";

/// Compara sem caixa uma letra (minúscula, ASCII) do marcador: pela minúscula simples (a de 'İ' é 'i'; a
/// completa é "i̇") e com as equivalências Unicode a mais (ı ~ i, ſ ~ s, K ~ k).
fn casa_sem_caixa(c: char, letra: char) -> bool {
    c.to_ascii_lowercase() == letra
        || match letra {
            'i' => matches!(c, 'İ' | 'ı'),
            's' => c == 'ſ',
            'k' => c == '\u{212A}',
            _ => false,
        }
}

/// O texto depois do marcador, se `texto` começa por ele (sem diferenciar maiúsculas).
fn depois_do_marcador(texto: &str) -> Option<&str> {
    let mut letras = texto.char_indices();
    let mut fim = 0;
    for letra in MARCADOR.chars() {
        let (i, c) = letras.next()?;
        if !casa_sem_caixa(c, letra) {
            return None;
        }
        fim = i + c.len_utf8();
    }
    texto.get(fim..)
}

/// O grupo de `constraint failed: (.+)$` (sem caixa): a partir do primeiro marcador cujo resto é uma linha só,
/// não vazia, que vai até o fim do texto (ou até a quebra de linha que o termina). Nunca corta fora da fronteira de um caractere.
fn grupo_das_colunas(msg: &str) -> Option<&str> {
    msg.char_indices().find_map(|(inicio, _)| {
        let resto = depois_do_marcador(msg.get(inicio..)?)?;
        let (linha, depois) = resto.split_once('\n').unwrap_or((resto, ""));
        (!linha.is_empty() && depois.is_empty()).then_some(linha)
    })
}

/// Os espaços aparados: os do Unicode e os separadores \x1c a \x1f.
fn e_espaco_aparado(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// "clientes.email" ou "itens.pedido_id, itens.produto_id" → "email" / "pedido_id, produto_id".
fn colunas_da_mensagem(msg: &str) -> String {
    let Some(grupo) = grupo_das_colunas(msg) else {
        return String::new();
    };
    grupo
        .split(',')
        .map(|p| {
            let p = p.trim_matches(e_espaco_aparado);
            p.split_once('.').map_or(p, |(_, coluna)| coluna)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Atalhos para traduzir o erro de uma chamada ao rusqlite, com ou sem a frase de contexto.
pub(crate) trait Contexto<T> {
    fn ctx(self, contexto: &str) -> Resultado<T>;
    fn sem_ctx(self) -> Resultado<T>;
}

impl<T> Contexto<T> for rusqlite::Result<T> {
    fn ctx(self, contexto: &str) -> Resultado<T> {
        self.map_err(|e| traduzir(e, Some(contexto)))
    }

    fn sem_ctx(self) -> Resultado<T> {
        self.map_err(|e| traduzir(e, None))
    }
}

pub(crate) fn texto_e_trava(texto: &str) -> bool {
    let baixo = texto.to_lowercase();
    baixo.contains("database is locked") || baixo.contains("database table is locked")
}

/// O erro é "database is locked" (do SQLite ou já traduzido num ErroBanco)?
pub(crate) fn e_trava(e: &ErroBanco) -> bool {
    texto_e_trava(&e.mensagem) || e.detalhe.as_deref().is_some_and(texto_e_trava)
}

pub(crate) fn e_trava_sqlite(e: &rusqlite::Error) -> bool {
    texto_e_trava(&e.to_string())
}

/// Os primeiros 200 caracteres: o detalhe técnico de um erro de formato, cortado sem partir caractere.
pub(crate) fn detalhe_curto(texto: &str) -> String {
    match texto.char_indices().nth(200) {
        Some((corte, _)) => texto[..corte].to_string(),
        None => texto.to_string(),
    }
}
