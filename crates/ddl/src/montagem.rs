//! Caminho de volta: ColunaDef → trecho SQL da coluna, e o CREATE TABLE inteiro (o que a reconstrução de
//! tabela do esquema usa).

use comum::{ErroBanco, Resultado, q};

use crate::analise::INICIO_RESTRICAO;
use crate::tokens::{TipoToken, aparar, aparar_fim, fecha, normalizar_espacos, preenchido, significativos, tokenizar};

/// Ações aceitas em ON DELETE / ON UPDATE.
pub const ACOES_FK: [&str; 5] = ["SET NULL", "SET DEFAULT", "CASCADE", "RESTRICT", "NO ACTION"];

/// Uma coluna como a interface e o esquema a descrevem.
/// Campo de texto vazio ou só com espaços vale como ausente na montagem.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColunaDef {
    pub nome: String,
    /// Tipo declarado ("" = sem tipo).
    pub tipo: String,
    pub nao_nulo: bool,
    pub unica: bool,
    pub pk: bool,
    pub autoincremento: bool,
    /// Expressão SQL como digitada: `0`, `'texto'`, `CURRENT_TIMESTAMP`, `datetime('now')`.
    pub padrao: Option<String>,
    /// O que vai dentro de CHECK (…).
    pub verificacao: Option<String>,
    pub colacao: Option<String>,
    pub referencia: Option<Referencia>,
    pub gerada: Option<Gerada>,
    /// Restrições não reconhecidas, preservadas literalmente.
    pub extra: Option<String>,
}

/// Chave estrangeira de uma coluna.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Referencia {
    pub tabela: String,
    pub coluna: Option<String>,
    /// Uma de [`ACOES_FK`] (o analisador devolve em maiúsculas; a montagem aceita qualquer caixa).
    pub ao_excluir: Option<String>,
    pub ao_atualizar: Option<String>,
    /// `MATCH x` e `[NOT] DEFERRABLE [INITIALLY …]`, como estavam (só existe quando a definição tem).
    pub complemento: Option<String>,
}

/// Coluna gerada: `GENERATED ALWAYS AS (expressao) STORED | VIRTUAL`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gerada {
    pub expressao: String,
    pub armazenada: bool,
}

/// Confere que um trecho de SQL (expressão, restrição) não escapa do lugar em que vai entrar: parênteses
/// equilibrados e nenhum ";" solto. Devolve o texto pronto para ser colado (sem espaços nas pontas e com
/// uma quebra de linha no fim se terminar num comentário `--`).
pub fn validar_trecho(texto: &str, rotulo: &str) -> Resultado<String> {
    let texto = aparar(texto);
    if texto.is_empty() {
        return Err(ErroBanco::novo(format!("{rotulo} está vazio.")));
    }
    let tokens = tokenizar(texto)?;
    let mut prof: i64 = 0;
    for t in &tokens {
        if t.e_op("(") {
            prof += 1;
        } else if t.e_op(")") {
            prof -= 1;
            if prof < 0 {
                return Err(ErroBanco::novo(format!("{rotulo} tem um parêntese fechando sem abrir: {texto}")));
            }
        } else if t.e_op(";") {
            return Err(ErroBanco::novo(format!("{rotulo} não pode ter ponto e vírgula: {texto}")));
        }
    }
    if prof != 0 {
        return Err(ErroBanco::novo(format!("{rotulo} tem parênteses sem fechamento: {texto}")));
    }
    let mut saida = texto.to_string();
    if termina_em_comentario_de_linha(&tokens) {
        saida.push('\n'); // senão o comentário de linha engoliria o que vem depois
    }
    Ok(saida)
}

fn termina_em_comentario_de_linha(tokens: &[crate::tokens::Token]) -> bool {
    tokens
        .iter()
        .rev()
        .find(|t| t.tipo != TipoToken::Espaco)
        .is_some_and(|t| t.tipo == TipoToken::Comentario && t.texto.starts_with("--"))
}

fn validar_tipo(tipo: &str) -> Resultado<String> {
    let tipo = normalizar_espacos(tipo);
    if tipo.is_empty() {
        return Ok(tipo);
    }
    let invalido = || ErroBanco::novo(format!("O tipo «{tipo}» não é válido."));
    for t in significativos(&tipo)? {
        if t.tipo == TipoToken::Op && !"(),+-".contains(t.texto.as_str()) {
            return Err(invalido());
        }
        if matches!(t.tipo, TipoToken::Texto | TipoToken::Blob | TipoToken::Comentario) {
            return Err(invalido());
        }
        if t.e(&INICIO_RESTRICAO) {
            return Err(ErroBanco::novo(format!(
                "O tipo «{tipo}» não pode ter restrições ({}); use os campos da coluna para isso.",
                t.texto
            )));
        }
    }
    validar_trecho(&tipo, "O tipo")?;
    Ok(tipo)
}

/// Expressão do DEFAULT: valor simples como veio, expressão entre parênteses.
///
/// O texto sai de [`validar_trecho`] sem ser aparado de novo (ver o topo do lib.rs), então o "\n" que ele põe
/// depois de um comentário `--` fica (`DEFAULT 'x' -- obs\n`), como no CHECK e na coluna gerada. Sem esse "\n",
/// o comentário engoliria o resto da coluna (COLLATE, REFERENCES, GENERATED, extra) ou o ")" do `(expressão)`.
fn montar_padrao(padrao: &str) -> Resultado<String> {
    // já aparado nas pontas, salvo o "\n" depois de um comentário de linha
    let texto = validar_trecho(padrao, "O valor padrão")?;
    let toks = significativos(&texto)?;
    let simples = |t: &crate::tokens::Token| {
        matches!(t.tipo, TipoToken::Texto | TipoToken::Numero | TipoToken::Blob | TipoToken::Palavra | TipoToken::Ident)
    };
    match toks.as_slice() {
        [unico] if simples(unico) => return Ok(texto),
        [sinal, numero] if (sinal.texto == "+" || sinal.texto == "-") && numero.tipo == TipoToken::Numero => {
            return Ok(texto);
        }
        _ => {}
    }
    if toks.first().is_some_and(|t| t.texto == "(") && fecha(&toks, 0)? + 1 == toks.len() {
        return Ok(texto);
    }
    Ok(format!("({texto})"))
}

fn acao_fk(acao: Option<&str>, rotulo: &str) -> Resultado<Option<String>> {
    let Some(acao) = acao.filter(|a| preenchido(a)) else {
        return Ok(None);
    };
    let a = normalizar_espacos(acao).to_uppercase();
    if !ACOES_FK.contains(&a.as_str()) {
        return Err(ErroBanco::novo(format!(
            "Ação inválida em {rotulo}: {acao}. Use uma destas: {}.",
            ACOES_FK.join(", ")
        )));
    }
    Ok(Some(a))
}

/// Nome de colação sem aspas quando é um identificador simples (`NOCASE`), citado senão.
fn colacao(nome: &str) -> String {
    let mut chars = nome.chars();
    let simples = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if simples { nome.to_string() } else { q(nome) }
}

/// O texto quando tem algo além de espaços.
fn preenchida(valor: &Option<String>) -> Option<&str> {
    valor.as_deref().filter(|v| preenchido(v))
}

/// [`ColunaDef`] → trecho SQL da coluna dentro de um CREATE TABLE (ou de um ADD COLUMN).
///
/// Um trecho digitado que termina num comentário `--` (padrão, complemento da referência, extra) leva um
/// "\n" depois dele, para o comentário não engolir o que vem em seguida; o trecho da coluna pode então
/// terminar em "\n" (o [`montar_create_table`] já põe a vírgula na linha seguinte nesse caso).
pub fn montar_coluna(c: &ColunaDef) -> Resultado<String> {
    if !preenchido(&c.nome) {
        return Err(ErroBanco::novo("Toda coluna precisa de um nome."));
    }
    let mut partes = vec![q(&c.nome)];
    let tipo = validar_tipo(&c.tipo)?;
    if !tipo.is_empty() {
        partes.push(tipo);
    }
    if c.pk {
        partes.push(if c.autoincremento { "PRIMARY KEY AUTOINCREMENT" } else { "PRIMARY KEY" }.to_string());
    } else if c.autoincremento {
        return Err(ErroBanco::novo(format!("Autoincremento só vale para a chave primária (coluna {}).", c.nome)));
    }
    if c.nao_nulo {
        partes.push("NOT NULL".to_string());
    }
    if c.unica {
        partes.push("UNIQUE".to_string());
    }
    if let Some(verificacao) = preenchida(&c.verificacao) {
        partes.push(format!("CHECK ({})", validar_trecho(verificacao, "A regra de verificação")?));
    }
    if let Some(padrao) = preenchida(&c.padrao) {
        partes.push(format!("DEFAULT {}", montar_padrao(padrao)?));
    }
    if let Some(nome) = preenchida(&c.colacao) {
        partes.push(format!("COLLATE {}", colacao(aparar(nome))));
    }
    if let Some(r) = c.referencia.as_ref().filter(|r| preenchido(&r.tabela)) {
        let mut trecho = format!("REFERENCES {}", q(&r.tabela));
        if let Some(coluna) = preenchida(&r.coluna) {
            trecho.push_str(&format!(" ({})", q(coluna)));
        }
        if let Some(acao) = acao_fk(r.ao_excluir.as_deref(), "Ao excluir")? {
            trecho.push_str(&format!(" ON DELETE {acao}"));
        }
        if let Some(acao) = acao_fk(r.ao_atualizar.as_deref(), "Ao atualizar")? {
            trecho.push_str(&format!(" ON UPDATE {acao}"));
        }
        if let Some(complemento) = preenchida(&r.complemento) {
            trecho.push(' ');
            trecho.push_str(&validar_trecho(complemento, "O complemento da referência")?);
        }
        partes.push(trecho);
    }
    if let Some(g) = c.gerada.as_ref().filter(|g| preenchido(&g.expressao)) {
        let expr = validar_trecho(&g.expressao, "A expressão da coluna gerada")?;
        partes.push(format!("GENERATED ALWAYS AS ({expr}) {}", if g.armazenada { "STORED" } else { "VIRTUAL" }));
    }
    if let Some(extra) = preenchida(&c.extra) {
        partes.push(validar_trecho(extra, "O complemento da coluna")?);
    }
    Ok(partes.join(" "))
}

/// CREATE TABLE com uma coluna ou restrição por linha, como o SQLite guarda e a prévia mostra.
/// `colunas_sql` vem de [`montar_coluna`]; `restricoes` e `sufixo` são validados aqui.
pub fn montar_create_table(
    nome: &str,
    colunas_sql: &[String],
    restricoes: &[String],
    sufixo: &str,
) -> Resultado<String> {
    if !preenchido(nome) {
        return Err(ErroBanco::novo("A tabela precisa de um nome."));
    }
    if colunas_sql.is_empty() {
        return Err(ErroBanco::novo("A tabela precisa de ao menos uma coluna."));
    }
    let mut itens: Vec<String> = colunas_sql.to_vec();
    for r in restricoes {
        itens.push(validar_trecho(r, "A restrição da tabela")?);
    }
    let total = itens.len();
    let mut linhas = Vec::with_capacity(total);
    for (k, item) in itens.iter().enumerate() {
        let item = aparar_fim(item);
        let sep = if k + 1 < total { "," } else { "" };
        // comentário de linha no fim de um item: a vírgula vai para a linha seguinte
        if !sep.is_empty() && termina_em_comentario(item)? {
            linhas.push(format!("    {item}\n    {sep}"));
        } else {
            linhas.push(format!("    {item}{sep}"));
        }
    }
    let mut sql = format!("CREATE TABLE {} (\n{}\n)", q(nome), linhas.join("\n"));
    let sufixo = aparar(sufixo);
    if !sufixo.is_empty() {
        sql.push(' ');
        sql.push_str(aparar(&validar_trecho(sufixo, "O complemento da tabela")?));
    }
    Ok(sql)
}

fn termina_em_comentario(texto: &str) -> Resultado<bool> {
    Ok(termina_em_comentario_de_linha(&tokenizar(texto)?))
}
