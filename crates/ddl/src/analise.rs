//! Analisador de CREATE TABLE: o texto que o SQLite guarda em sqlite_schema, devolvido em partes.

use comum::{ErroBanco, Resultado};

use crate::montagem::{ColunaDef, Gerada, Referencia};
use crate::tokens::{
    TipoToken, Token, aparar, desfazer_aspas, e_nome, fatia, fecha, normalizar_espacos, significativos, trecho,
};

/// Palavras que encerram o tipo declarado de uma coluna e começam uma restrição.
pub(crate) const INICIO_RESTRICAO: [&str; 11] =
    ["CONSTRAINT", "PRIMARY", "NOT", "NULL", "UNIQUE", "CHECK", "DEFAULT", "COLLATE", "REFERENCES", "GENERATED", "AS"];
const INICIO_RESTRICAO_TABELA: [&str; 5] = ["CONSTRAINT", "PRIMARY", "UNIQUE", "CHECK", "FOREIGN"];

/// Uma coluna como está no CREATE TABLE.
///
/// O que não foi entendido (nome de CONSTRAINT, cláusula ON CONFLICT, PRIMARY KEY DESC…) vai para `extra`
/// literalmente e é devolvido no fim da coluna pela montagem, para nada se perder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefColuna {
    pub nome: String,
    /// Tipo declarado numa linha só ("" sem tipo).
    pub tipo: String,
    /// Definição original completa da coluna, numa linha só.
    pub texto: String,
    pub pk: bool,
    /// "ASC" ou "DESC" quando a chave primária tem ordem (DESC vai para `extra` e `pk` fica falso).
    pub pk_ordem: Option<String>,
    pub autoincremento: bool,
    pub nao_nulo: bool,
    pub unica: bool,
    /// Expressão do DEFAULT como está no texto (`'São Paulo'`, `-1.5`, `(datetime('now'))`).
    pub padrao: Option<String>,
    /// O que vai dentro de CHECK (…).
    pub verificacao: Option<String>,
    pub colacao: Option<String>,
    /// Chave estrangeira; `complemento` traz MATCH e DEFERRABLE.
    pub referencia: Option<Referencia>,
    /// O trecho `REFERENCES …` inteiro, numa linha só.
    pub referencia_texto: Option<String>,
    pub gerada: Option<Gerada>,
    /// Restrições não reconhecidas, preservadas literalmente (numa linha só).
    pub extra: Option<String>,
}

impl DefColuna {
    /// A [`ColunaDef`] (§4.4 da arquitetura), o formato que a interface e o esquema usam.
    pub fn como_dict(&self) -> ColunaDef {
        ColunaDef {
            nome: self.nome.clone(),
            tipo: self.tipo.clone(),
            nao_nulo: self.nao_nulo,
            unica: self.unica,
            pk: self.pk,
            autoincremento: self.autoincremento,
            padrao: self.padrao.clone(),
            verificacao: self.verificacao.clone(),
            colacao: self.colacao.clone(),
            referencia: self
                .referencia
                .as_ref()
                .map(|r| Referencia { complemento: r.complemento.clone().filter(|c| !c.is_empty()), ..r.clone() }),
            gerada: self.gerada.clone(),
            extra: self.extra.clone(),
        }
    }
}

/// Um CREATE TABLE em partes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefTabela {
    pub nome: String,
    pub colunas: Vec<DefColuna>,
    /// Restrições de tabela como estão no texto ("PRIMARY KEY (pedido_id, produto)").
    pub restricoes: Vec<String>,
    pub sem_rowid: bool,
    pub estrita: bool,
    /// O que vem depois do ")" (WITHOUT ROWID, STRICT), numa linha só.
    pub sufixo: String,
}

fn esperar(toks: &[Token], i: &mut usize, palavras: &[&str]) -> Resultado<()> {
    match toks.get(*i) {
        Some(t) if t.e(palavras) => {
            *i += 1;
            Ok(())
        }
        achado => {
            let achado = achado.map_or("o fim do texto", |t| t.texto.as_str());
            Err(ErroBanco::novo(format!("Esperava {} no CREATE TABLE, mas veio {achado}.", palavras.join(" ou "))))
        }
    }
}

fn e_em(toks: &[Token], i: usize, palavras: &[&str]) -> bool {
    toks.get(i).is_some_and(|t| t.e(palavras))
}

fn texto_em(toks: &[Token], i: usize, texto: &str) -> bool {
    toks.get(i).is_some_and(|t| t.texto == texto)
}

/// Lê um CREATE TABLE e devolve a definição em partes. Outro comando → [`ErroBanco`].
pub fn analisar_create_table(sql: &str) -> Resultado<DefTabela> {
    let toks = significativos(sql)?;
    let n = toks.len();
    let mut i = 0;

    esperar(&toks, &mut i, &["CREATE"])?;
    if e_em(&toks, i, &["TEMP", "TEMPORARY"]) {
        i += 1;
    }
    if e_em(&toks, i, &["VIRTUAL"]) {
        return Err(ErroBanco::novo(
            "Tabela virtual não tem estrutura editável: as colunas vêm do módulo que a criou.",
        ));
    }
    esperar(&toks, &mut i, &["TABLE"])?;
    if i + 2 < n && e_em(&toks, i, &["IF"]) && e_em(&toks, i + 1, &["NOT"]) && e_em(&toks, i + 2, &["EXISTS"]) {
        i += 3;
    }
    let Some(primeiro) = toks.get(i).filter(|t| e_nome(t)) else {
        return Err(ErroBanco::novo("O CREATE TABLE não tem o nome da tabela."));
    };
    let mut nome = desfazer_aspas(&primeiro.texto);
    i += 1;
    if texto_em(&toks, i, ".")
        && let Some(segundo) = toks.get(i + 1).filter(|t| e_nome(t))
    {
        nome = desfazer_aspas(&segundo.texto);
        i += 2;
    }
    if e_em(&toks, i, &["AS"]) {
        return Err(ErroBanco::novo(
            "Esta tabela foi definida por uma consulta (CREATE TABLE … AS SELECT); \
             a estrutura dela não pode ser editada aqui.",
        ));
    }
    if !texto_em(&toks, i, "(") {
        return Err(ErroBanco::novo("O CREATE TABLE não tem a lista de colunas entre parênteses."));
    }
    let fim = fecha(&toks, i)?;

    let mut tabela = DefTabela { nome, ..DefTabela::default() };
    for (a, z) in partes(&toks, i + 1, fim) {
        let Some(item) = toks.get(a..=z) else { continue };
        if e_em(&toks, a, &INICIO_RESTRICAO_TABELA) {
            tabela.restricoes.push(trecho(sql, &toks, a, z).to_string());
        } else {
            tabela.colunas.push(analisar_coluna(sql, item)?);
        }
    }
    if tabela.colunas.is_empty() {
        return Err(ErroBanco::novo("O CREATE TABLE não tem colunas."));
    }

    let resto: Vec<&Token> = toks.get(fim + 1..).unwrap_or(&[]).iter().filter(|t| t.texto != ";").collect();
    if let (Some(p), Some(u)) = (resto.first(), resto.last()) {
        tabela.sufixo = normalizar_espacos(fatia(sql, p.inicio, u.fim));
    }
    for (k, t) in resto.iter().enumerate() {
        if t.e(&["WITHOUT"]) && resto.get(k + 1).is_some_and(|r| r.e(&["ROWID"])) {
            tabela.sem_rowid = true;
        } else if t.e(&["STRICT"]) {
            tabela.estrita = true;
        }
    }
    Ok(tabela)
}

/// Intervalos [início, fim] (inclusive) dos itens separados por vírgula de topo em `toks[a..z]`.
fn partes(toks: &[Token], a: usize, z: usize) -> Vec<(usize, usize)> {
    let mut partes = Vec::new();
    let mut prof: i64 = 0;
    let mut inicio = a;
    for (j, t) in toks.iter().enumerate().take(z).skip(a) {
        if t.e_op("(") {
            prof += 1;
        } else if t.e_op(")") {
            prof -= 1;
        } else if t.e_op(",") && prof == 0 {
            if j > inicio {
                partes.push((inicio, j - 1));
            }
            inicio = j + 1;
        }
    }
    if z > inicio {
        partes.push((inicio, z - 1));
    }
    partes
}

/// O que uma restrição reconhecida muda na coluna.
enum Aplicar {
    /// "NULL" sozinho não muda nada.
    Nada,
    Pk {
        ordem: Option<String>,
        auto: bool,
    },
    NaoNulo,
    Unica,
    Verificacao(String),
    Padrao(String),
    Colacao(String),
    Referencia(Referencia, String),
    Gerada(Gerada),
}

fn analisar_coluna(sql: &str, toks: &[Token]) -> Resultado<DefColuna> {
    let (Some(primeiro), Some(ultimo)) = (toks.first(), toks.last()) else {
        return Err(ErroBanco::novo("O CREATE TABLE não tem colunas."));
    };
    let mut col = DefColuna {
        nome: desfazer_aspas(&primeiro.texto),
        texto: normalizar_espacos(fatia(sql, primeiro.inicio, ultimo.fim)),
        ..DefColuna::default()
    };
    let n = toks.len();
    let mut i = 1;
    while i < n && !e_em(toks, i, &INICIO_RESTRICAO) {
        i = if texto_em(toks, i, "(") { fecha(toks, i)? + 1 } else { i + 1 };
    }
    if i > 1 {
        col.tipo = normalizar_espacos(trecho(sql, toks, 1, i - 1));
    }

    let mut extras: Vec<String> = Vec::new();
    let conflito = |k: usize| -> (usize, bool) {
        if k + 2 < n && e_em(toks, k, &["ON"]) && e_em(toks, k + 1, &["CONFLICT"]) { (k + 3, true) } else { (k, false) }
    };

    while let Some(t) = toks.get(i) {
        let inicio = i;
        let mut t = t;
        let mut nomeada = false;
        if t.e(&["CONSTRAINT"]) && i + 1 < n {
            nomeada = true;
            i += 2;
            match toks.get(i) {
                Some(depois) => t = depois,
                None => {
                    extras.push(trecho(sql, toks, inicio, n - 1).to_string());
                    break;
                }
            }
        }
        // restrição que não cabe nos campos: vai inteira para extra
        let mut literal = nomeada;
        let aplicar: Option<Aplicar>;

        if t.e(&["PRIMARY"]) && e_em(toks, i + 1, &["KEY"]) {
            i += 2;
            let mut ordem = None;
            if let Some(o) = toks.get(i).filter(|o| o.e(&["ASC", "DESC"])) {
                ordem = Some(o.alto());
                i += 1;
            }
            let (k, com_conflito) = conflito(i);
            i = k;
            let mut auto = false;
            if e_em(toks, i, &["AUTOINCREMENT"]) {
                auto = true;
                i += 1;
            }
            literal = literal || com_conflito || ordem.as_deref() == Some("DESC");
            aplicar = Some(Aplicar::Pk { ordem, auto });
        } else if t.e(&["NOT"]) && e_em(toks, i + 1, &["NULL"]) {
            let (k, com_conflito) = conflito(i + 2);
            i = k;
            literal = literal || com_conflito;
            aplicar = Some(Aplicar::NaoNulo);
        } else if t.e(&["NULL"]) {
            let (k, com_conflito) = conflito(i + 1);
            i = k;
            literal = literal || com_conflito;
            aplicar = Some(Aplicar::Nada);
        } else if t.e(&["UNIQUE"]) {
            let (k, com_conflito) = conflito(i + 1);
            i = k;
            literal = literal || com_conflito;
            aplicar = Some(Aplicar::Unica);
        } else if t.e(&["CHECK"]) && texto_em(toks, i + 1, "(") {
            let z = fecha(toks, i + 1)?;
            let expr = expressao_entre(sql, toks, i + 1, z);
            i = z + 1;
            aplicar = Some(Aplicar::Verificacao(expr));
        } else if t.e(&["DEFAULT"]) && i + 1 < n {
            let k = i + 1;
            let z = if texto_em(toks, k, "(") {
                fecha(toks, k)?
            } else if (texto_em(toks, k, "+") || texto_em(toks, k, "-"))
                && toks.get(k + 1).is_some_and(|u| u.tipo == TipoToken::Numero)
            {
                k + 1
            } else {
                k
            };
            let valor = trecho(sql, toks, k, z).to_string();
            i = z + 1;
            aplicar = Some(Aplicar::Padrao(valor));
        } else if t.e(&["COLLATE"])
            && let Some(nome_col) = toks.get(i + 1)
        {
            let nome_col = desfazer_aspas(&nome_col.texto);
            i += 2;
            aplicar = Some(Aplicar::Colacao(nome_col));
        } else if t.e(&["REFERENCES"]) && i + 1 < n {
            let (k, referencia, texto, simples) = analisar_referencia(sql, toks, i)?;
            i = k;
            literal = literal || !simples;
            aplicar = Some(Aplicar::Referencia(referencia, texto));
        } else if t.e(&["GENERATED", "AS"]) {
            let mut k = i;
            if t.e(&["GENERATED"]) {
                if !(e_em(toks, k + 1, &["ALWAYS"]) && e_em(toks, k + 2, &["AS"])) {
                    extras.push(t.texto.clone());
                    i += 1;
                    continue;
                }
                k += 2;
            }
            if texto_em(toks, k + 1, "(") {
                let z = fecha(toks, k + 1)?;
                let expressao = expressao_entre(sql, toks, k + 1, z);
                i = z + 1;
                let mut armazenada = false;
                if let Some(s) = toks.get(i).filter(|s| s.e(&["STORED", "VIRTUAL"])) {
                    armazenada = s.e(&["STORED"]);
                    i += 1;
                }
                aplicar = Some(Aplicar::Gerada(Gerada { expressao, armazenada }));
            } else {
                i = k + 1;
                literal = true;
                aplicar = None;
            }
        } else {
            // Não reconhecido: guarda o token (ou o grupo entre parênteses) como veio.
            i = if t.texto == "(" { fecha(toks, i)? + 1 } else { i + 1 };
            literal = true;
            aplicar = None;
        }

        match aplicar {
            Some(a) if !literal => match a {
                Aplicar::Nada => {}
                Aplicar::Pk { ordem, auto } => {
                    col.pk = true;
                    col.pk_ordem = ordem;
                    col.autoincremento = auto;
                }
                Aplicar::NaoNulo => col.nao_nulo = true,
                Aplicar::Unica => col.unica = true,
                Aplicar::Verificacao(expr) => col.verificacao = Some(expr),
                Aplicar::Padrao(valor) => col.padrao = Some(valor),
                Aplicar::Colacao(nome) => col.colacao = Some(nome),
                Aplicar::Referencia(referencia, texto) => {
                    col.referencia = Some(referencia);
                    col.referencia_texto = Some(texto);
                }
                Aplicar::Gerada(gerada) => col.gerada = Some(gerada),
            },
            _ => extras.push(trecho(sql, toks, inicio, i.saturating_sub(1)).to_string()),
        }
    }

    let extra = extras.iter().map(|e| normalizar_espacos(e)).collect::<Vec<_>>().join(" ");
    col.extra = if extra.is_empty() { None } else { Some(extra) };
    Ok(col)
}

/// Texto entre o "(" de `toks[abre]` e o ")" de `toks[fecha]`, sem espaços nas pontas.
fn expressao_entre(sql: &str, toks: &[Token], abre: usize, fecha: usize) -> String {
    match (toks.get(abre), toks.get(fecha)) {
        (Some(a), Some(z)) => aparar(fatia(sql, a.fim, z.inicio)).to_string(),
        _ => String::new(),
    }
}

/// Lê `REFERENCES t [(c…)] [ON DELETE/UPDATE ação] [MATCH x] [[NOT] DEFERRABLE …]`.
///
/// Devolve (próximo índice, referência, texto do trecho, simples); simples=false quando há mais de uma
/// coluna e a restrição precisa ir literal para `extra`.
fn analisar_referencia(sql: &str, toks: &[Token], i: usize) -> Resultado<(usize, Referencia, String, bool)> {
    let n = toks.len();
    let inicio = i;
    let mut referencia = Referencia {
        tabela: toks.get(i + 1).map(|t| desfazer_aspas(&t.texto)).unwrap_or_default(),
        ..Referencia::default()
    };
    let mut i = i + 2;
    let mut simples = true;
    if texto_em(toks, i, "(") {
        let z = fecha(toks, i)?;
        let mut nomes: Vec<String> = toks
            .get(i + 1..z)
            .unwrap_or(&[])
            .iter()
            .filter(|t| t.texto != ",")
            .map(|t| desfazer_aspas(&t.texto))
            .collect();
        if nomes.len() == 1 {
            referencia.coluna = nomes.pop();
        } else {
            simples = false;
        }
        i = z + 1;
    }
    let mut complemento: Vec<&str> = Vec::new();
    while let Some(t) = toks.get(i) {
        if t.e(&["ON"]) && i + 2 < n && e_em(toks, i + 1, &["DELETE", "UPDATE"]) {
            let exclui = e_em(toks, i + 1, &["DELETE"]);
            let mut k = i + 2;
            let acao = match (toks.get(k), toks.get(k + 1)) {
                (Some(a), Some(b)) if a.e(&["SET", "NO"]) => {
                    k += 2;
                    format!("{} {}", a.alto(), b.alto())
                }
                (Some(a), _) => {
                    k += 1;
                    a.alto()
                }
                (None, _) => break,
            };
            if exclui {
                referencia.ao_excluir = Some(acao);
            } else {
                referencia.ao_atualizar = Some(acao);
            }
            i = k;
        } else if t.e(&["MATCH"]) && i + 1 < n {
            complemento.push(trecho(sql, toks, i, i + 1));
            i += 2;
        } else if t.e(&["NOT", "DEFERRABLE"]) {
            let mut k = i;
            if t.e(&["NOT"]) {
                if !e_em(toks, k + 1, &["DEFERRABLE"]) {
                    break;
                }
                k += 1;
            }
            k += 1;
            if e_em(toks, k, &["INITIALLY"]) && e_em(toks, k + 1, &["DEFERRED", "IMMEDIATE"]) {
                k += 2;
            }
            complemento.push(trecho(sql, toks, i, k - 1));
            i = k;
        } else {
            break;
        }
    }
    let texto = normalizar_espacos(trecho(sql, toks, inicio, i.saturating_sub(1)));
    if !complemento.is_empty() {
        referencia.complemento = Some(complemento.iter().map(|c| normalizar_espacos(c)).collect::<Vec<_>>().join(" "));
    }
    Ok((i, referencia, texto, simples))
}
