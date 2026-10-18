//! Busca em todas as colunas e filtros por coluna: LIKE (termo só ASCII) ou GLOB com as duas formas de cada
//! letra (termo com acento), com as peneiras que não mudam o resultado, só o tempo.

use comum::{Afinidade, Entrada, ErroBanco, Resultado, Valor, q};

use crate::meta::{ColunaMeta, Meta, coluna_por_nome};
use crate::valores::{Coagido, coagir, texto_do_valor};

/// Tudo o que pode aparecer no texto de um INTEGER ou REAL (CAST(-1.5e+20 AS TEXT), 'Inf', '-Inf'), nas duas
/// caixas. Um termo com algum caractere fora disso nunca acha um número, e a busca pula a conversão dele.
pub(crate) const CARACTERES_DE_NUMERO: &str = "0123456789.+-eEiInNfF";

/// Operador de filtro (os 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operador {
    Contem,
    NaoContem,
    Igual,
    Diferente,
    Maior,
    MaiorIgual,
    Menor,
    MenorIgual,
    Comeca,
    Termina,
    Nulo,
    NaoNulo,
    Vazio,
    NaoVazio,
}

impl Operador {
    /// Todos, na ordem do menu de filtros.
    pub const TODOS: [Operador; 14] = [
        Operador::Contem,
        Operador::NaoContem,
        Operador::Igual,
        Operador::Diferente,
        Operador::Maior,
        Operador::MaiorIgual,
        Operador::Menor,
        Operador::MenorIgual,
        Operador::Comeca,
        Operador::Termina,
        Operador::Nulo,
        Operador::NaoNulo,
        Operador::Vazio,
        Operador::NaoVazio,
    ];

    /// "contem", "nao_contem", "igual"…
    pub fn como_str(self) -> &'static str {
        match self {
            Operador::Contem => "contem",
            Operador::NaoContem => "nao_contem",
            Operador::Igual => "igual",
            Operador::Diferente => "diferente",
            Operador::Maior => "maior",
            Operador::MaiorIgual => "maior_igual",
            Operador::Menor => "menor",
            Operador::MenorIgual => "menor_igual",
            Operador::Comeca => "comeca",
            Operador::Termina => "termina",
            Operador::Nulo => "nulo",
            Operador::NaoNulo => "nao_nulo",
            Operador::Vazio => "vazio",
            Operador::NaoVazio => "nao_vazio",
        }
    }

    /// O operador pelo texto; desconhecido → 400 "Operador de filtro desconhecido: x.".
    pub fn de_texto(texto: &str) -> Resultado<Operador> {
        Operador::TODOS
            .into_iter()
            .find(|op| op.como_str() == texto)
            .ok_or_else(|| ErroBanco::novo(format!("Operador de filtro desconhecido: {texto}.")))
    }

    /// Nulo, não nulo, vazio e não vazio não usam valor.
    pub fn sem_valor(self) -> bool {
        matches!(self, Operador::Nulo | Operador::NaoNulo | Operador::Vazio | Operador::NaoVazio)
    }
}

/// Um filtro `{coluna, op, valor}`; os filtros de uma lista se juntam com AND.
#[derive(Debug, Clone, PartialEq)]
pub struct Filtro {
    pub coluna: String,
    pub op: Operador,
    /// O valor como a pessoa digitou ([`Entrada::Texto`]) é coagido pela afinidade da coluna, como numa
    /// gravação; contem/nao_contem/comeca/termina usam o texto.
    pub valor: Entrada,
}

impl Filtro {
    pub fn novo(coluna: impl Into<String>, op: Operador, valor: Entrada) -> Filtro {
        Filtro { coluna: coluna.into(), op, valor }
    }

    /// Filtro com o texto digitado (o caso da interface).
    pub fn texto(coluna: impl Into<String>, op: Operador, valor: impl Into<String>) -> Filtro {
        Filtro { coluna: coluna.into(), op, valor: Entrada::Texto(valor.into()) }
    }
}

pub(crate) fn escapar_like(texto: &str) -> String {
    texto.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn um_caractere(mut it: impl Iterator<Item = char>) -> Option<char> {
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

/// Padrão GLOB que acha o texto sem diferença de maiúsculas, também nas letras acentuadas.
///
/// O LIKE do SQLite só ignora a caixa das letras ASCII ("joão" não acharia "JOÃO"). Aqui cada letra vira
/// uma classe com as duas formas ("[jJ][oO][ãÃ][oO]"), e o GLOB compara dentro do SQLite. *, ? e [ do texto
/// viram classes de um caractere só.
pub(crate) fn glob_sem_caixa(texto: &str, antes: bool, depois: bool) -> String {
    let mut saida = String::with_capacity(texto.len() * 4 + 2);
    if antes {
        saida.push('*');
    }
    for c in texto.chars() {
        let menor = um_caractere(c.to_lowercase());
        let maior = um_caractere(c.to_uppercase());
        match (menor, maior) {
            (Some(m), Some(n)) if m != n => {
                saida.push('[');
                saida.push(m);
                saida.push(n);
                saida.push(']');
            }
            _ if matches!(c, '*' | '?' | '[') => {
                saida.push('[');
                saida.push(c);
                saida.push(']');
            }
            _ => saida.push(c),
        }
    }
    if depois {
        saida.push('*');
    }
    saida
}

fn e_de_numero(c: char) -> bool {
    CARACTERES_DE_NUMERO.contains(c)
}

/// O termo pode aparecer no texto de um INTEGER ou REAL, sem diferença de maiúsculas?
///
/// Cada caractere precisa ter alguma forma (ele mesmo, a minúscula ou a maiúscula) entre os que o texto
/// de um número tem. "12345" passa, "1,5" não (vírgula), "inf" passa; "paulo" e "joão" não passam.
pub(crate) fn pode_estar_num_numero(termo: &str) -> bool {
    termo.chars().all(|c| {
        e_de_numero(c)
            || um_caractere(c.to_lowercase()).is_some_and(e_de_numero)
            || um_caractere(c.to_uppercase()).is_some_and(e_de_numero)
    })
}

fn sem_caixa(c: char) -> bool {
    c.to_lowercase().eq(std::iter::once(c)) && c.to_uppercase().eq(std::iter::once(c))
}

/// O maior trecho do termo feito só de caracteres sem maiúscula e minúscula (dígitos, espaço, pontuação).
///
/// Esse trecho aparece exatamente assim em todo texto que o LIKE ou o GLOB acham, e instr() o procura bem
/// mais depressa: vale como peneira antes da comparação de verdade. Trecho de um caractere só não
/// peneira quase nada (a menos que seja o termo inteiro), e aí volta vazio. NUL corta o trecho: o LIKE e o
/// GLOB param no primeiro NUL do padrão, e o instr() não.
pub(crate) fn trecho_sem_caixa(termo: &str) -> String {
    let (mut melhor, mut n_melhor) = (String::new(), 0);
    let (mut atual, mut n_atual) = (String::new(), 0);
    for c in termo.chars() {
        if c != '\0' && sem_caixa(c) {
            atual.push(c);
            n_atual += 1;
            if n_atual > n_melhor {
                melhor.clone_from(&atual);
                n_melhor = n_atual;
            }
        } else {
            atual.clear();
            n_atual = 0;
        }
    }
    if n_melhor >= 2 || melhor == termo { melhor } else { String::new() }
}

/// Condição "o texto da coluna tem o termo" (antes/depois: com algo antes/depois dele), sem diferença de caixa.
///
/// O resultado é o mesmo de `CAST(coluna AS TEXT) LIKE '%termo%'` (termo só ASCII) ou GLOB com as duas formas
/// de cada letra (termo com acento), com duas peneiras na frente que não mudam o que é achado, só o tempo:
/// `coluna >= '' COLLATE BINARY` (verdadeiro só para texto e BLOB) quando o termo não cabe no texto de um
/// número, e o maior trecho do termo sem maiúsculas num instr() antes do LIKE/GLOB.
pub(crate) fn contem_texto(coluna: &str, termo: &str, antes: bool, depois: bool) -> (String, Vec<Valor>) {
    let texto = format!("CAST({coluna} AS TEXT)");
    let (mut condicao, mut params) = if termo.is_ascii() {
        let padrao =
            format!("{}{}{}", if antes { "%" } else { "" }, escapar_like(termo), if depois { "%" } else { "" });
        (format!("{texto} LIKE ? ESCAPE '\\'"), vec![Valor::Texto(padrao)])
    } else {
        (format!("{texto} GLOB ?"), vec![Valor::Texto(glob_sem_caixa(termo, antes, depois))])
    };
    let trecho = if antes { trecho_sem_caixa(termo) } else { String::new() };
    if !trecho.is_empty() {
        condicao = format!("instr({texto}, ?) AND {condicao}");
        params.insert(0, Valor::Texto(trecho));
    }
    if !pode_estar_num_numero(termo) {
        condicao = format!("{coluna} >= '' COLLATE BINARY AND {condicao}");
    }
    (format!("({condicao})"), params)
}

fn sem_filtro() -> ErroBanco {
    ErroBanco::novo("Esse valor não serve como filtro.")
}

/// A condição SQL de um filtro numa coluna, com os parâmetros.
fn condicao(col: &ColunaMeta, op: Operador, valor: &Entrada) -> Resultado<(String, Vec<Valor>)> {
    let c = q(&col.nome);
    let texto = format!("CAST({c} AS TEXT)");
    match op {
        Operador::Nulo => return Ok((format!("{c} IS NULL"), vec![])),
        Operador::NaoNulo => return Ok((format!("{c} IS NOT NULL"), vec![])),
        Operador::Vazio => return Ok((format!("({c} IS NULL OR {texto} = '')"), vec![])),
        Operador::NaoVazio => return Ok((format!("({c} IS NOT NULL AND {texto} <> '')"), vec![])),
        Operador::Contem | Operador::NaoContem | Operador::Comeca | Operador::Termina => {
            let Coagido::Valor(bruto) = coagir(valor, Afinidade::Text)? else {
                return Err(sem_filtro());
            };
            let s = texto_do_valor(&bruto);
            // LIKE (termo só ASCII) ou GLOB com as duas formas de cada letra (com acento), como na busca
            let (condicao, params) = contem_texto(&c, &s, op != Operador::Comeca, op != Operador::Termina);
            if op == Operador::NaoContem {
                return Ok((format!("({c} IS NULL OR NOT {condicao})"), params));
            }
            return Ok((condicao, params));
        }
        _ => {}
    }
    let Coagido::Valor(v) = coagir(valor, col.afinidade)? else {
        return Err(sem_filtro());
    };
    if col.afinidade == Afinidade::Blob
        && let Entrada::Texto(digitado) = valor
        && !v.e_nulo()
        && v != Valor::Texto(digitado.clone())
    {
        // sem tipo declarado, a coluna pode guardar 10 (número) ou '10' (texto): "igual" acha os dois
        let como_texto = Valor::Texto(digitado.clone());
        match op {
            Operador::Igual => return Ok((format!("{c} IN (?, ?)"), vec![v, como_texto])),
            Operador::Diferente => return Ok((format!("({c} IS NOT ? AND {c} IS NOT ?)"), vec![v, como_texto])),
            _ => {}
        }
    }
    Ok(match op {
        Operador::Igual if v.e_nulo() => (format!("{c} IS NULL"), vec![]),
        Operador::Igual => (format!("{c} = ?"), vec![v]),
        Operador::Diferente if v.e_nulo() => (format!("{c} IS NOT NULL"), vec![]),
        Operador::Diferente => (format!("{c} IS NOT ?"), vec![v]),
        Operador::Maior => (format!("{c} > ?"), vec![v]),
        Operador::MaiorIgual => (format!("{c} >= ?"), vec![v]),
        Operador::Menor => (format!("{c} < ?"), vec![v]),
        _ => (format!("{c} <= ?"), vec![v]),
    })
}

/// " WHERE …" da busca (OR em todas as colunas visíveis) e dos filtros (AND), com os parâmetros.
pub(crate) fn montar_where(meta: &Meta, busca: Option<&str>, filtros: &[Filtro]) -> Resultado<(String, Vec<Valor>)> {
    let mut partes = Vec::new();
    let mut params = Vec::new();
    if let Some(termo) = busca
        && !termo.is_empty()
        && !meta.visiveis.is_empty()
    {
        let mut ors = Vec::with_capacity(meta.visiveis.len());
        for c in &meta.visiveis {
            let (sql, ps) = contem_texto(&q(&c.nome), termo, true, true);
            ors.push(sql);
            params.extend(ps);
        }
        partes.push(format!("({})", ors.join(" OR ")));
    }
    for f in filtros {
        let col = coluna_por_nome(&meta.visiveis, &f.coluna)
            .ok_or_else(|| ErroBanco::novo(format!("A coluna do filtro não existe em {}: {}.", meta.nome, f.coluna)))?;
        let (sql, ps) = condicao(col, f.op, &f.valor)?;
        partes.push(sql);
        params.extend(ps);
    }
    let sql = if partes.is_empty() { String::new() } else { format!(" WHERE {}", partes.join(" AND ")) };
    Ok((sql, params))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn glob_com_as_duas_formas() {
        assert_eq!(glob_sem_caixa("joão", true, true), "*[jJ][oO][ãÃ][oO]*");
        assert_eq!(glob_sem_caixa("a*?[1]", false, true), "[aA][*][?][[]1]*");
        assert_eq!(glob_sem_caixa("ß", false, false), "ß"); // a maiúscula é "SS": fica como está
    }

    #[test]
    fn operadores_vao_e_voltam_pelo_texto() {
        for op in Operador::TODOS {
            assert_eq!(Operador::de_texto(op.como_str()), Ok(op));
        }
        assert_eq!(
            Operador::de_texto("parecido").map_err(|e| e.mensagem),
            Err("Operador de filtro desconhecido: parecido.".into())
        );
    }
}
