//! Filtrar e ordenar o CSV (o passo a mais da faixa G; a prancheta 16 b desenha o CSV filtrado): as condições da
//! faixa (como as da aba Dados: "regiao igual a Sul", com o popover de Coluna, Operador e Valor) e a ordem pela seta
//! do cabeçalho. A **vista** é a lista das posições da tabela que passam nas condições, na ordem pedida; ela é
//! calculada numa tarefa, com um instantâneo da tabela (as alterações pendentes valem) e uma passada pelo arquivo
//! (`Leitor::varrer`, pelas colunas que as condições e a ordem usam), e a grade mostra as linhas dela. Com a vista, o
//! número da linha continua a posição no arquivo, e os dois Colar por posição ficam desligados.

use std::cmp::Ordering;

use comum::Resultado;
use tabular::numero::analisar_numero;

use crate::sessao::{self, Controle, TabelaCompartilhada};

/// Os operadores de uma condição (os da aba Dados, sem os de NULL: o CSV não tem NULL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperadorCsv {
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
    Vazio,
    NaoVazio,
}

/// Os operadores na ordem do popover, com os rótulos da aba Dados.
pub const OPERADORES: [(OperadorCsv, &str); 12] = [
    (OperadorCsv::Contem, "contém"),
    (OperadorCsv::NaoContem, "não contém"),
    (OperadorCsv::Igual, "igual a"),
    (OperadorCsv::Diferente, "diferente de"),
    (OperadorCsv::Maior, "maior que"),
    (OperadorCsv::MaiorIgual, "maior ou igual"),
    (OperadorCsv::Menor, "menor que"),
    (OperadorCsv::MenorIgual, "menor ou igual"),
    (OperadorCsv::Comeca, "começa com"),
    (OperadorCsv::Termina, "termina com"),
    (OperadorCsv::Vazio, "é vazio"),
    (OperadorCsv::NaoVazio, "não é vazio"),
];

impl OperadorCsv {
    pub fn rotulo(self) -> &'static str {
        OPERADORES.iter().find(|(o, _)| *o == self).map_or("", |(_, r)| r)
    }

    /// O operador não pede valor ("é vazio").
    pub fn sem_valor(self) -> bool {
        matches!(self, OperadorCsv::Vazio | OperadorCsv::NaoVazio)
    }
}

/// Uma condição da faixa. A coluna vai pelo nome (as colunas mudam de lugar com inserir e excluir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condicao {
    pub id: u64,
    pub coluna: String,
    pub op: OperadorCsv,
    pub valor: String,
}

/// A ordem pela seta do cabeçalho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ordem {
    pub coluna: String,
    pub decrescente: bool,
}

/// O texto para comparar sem diferença de caixa nem de espaços nas pontas.
fn normal(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Compara dois valores: números pelo valor (com o decimal do arquivo) quando os dois são números; senão o texto, sem
/// diferença de caixa. O vazio vai para o fim na ordem crescente.
pub fn comparar(a: &str, b: &str, decimal: char) -> Ordering {
    match (a.trim().is_empty(), b.trim().is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    match (analisar_numero(a, decimal), analisar_numero(b, decimal)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => normal(a).cmp(&normal(b)),
    }
}

/// O valor passa na condição?
pub fn passa(valor: &str, op: OperadorCsv, alvo: &str, decimal: char) -> bool {
    let (v, a) = (normal(valor), normal(alvo));
    match op {
        OperadorCsv::Contem => v.contains(&a),
        OperadorCsv::NaoContem => !v.contains(&a),
        OperadorCsv::Igual => comparar(valor, alvo, decimal) == Ordering::Equal,
        OperadorCsv::Diferente => comparar(valor, alvo, decimal) != Ordering::Equal,
        OperadorCsv::Maior => !v.is_empty() && comparar(valor, alvo, decimal) == Ordering::Greater,
        OperadorCsv::MaiorIgual => !v.is_empty() && comparar(valor, alvo, decimal) != Ordering::Less,
        OperadorCsv::Menor => !v.is_empty() && comparar(valor, alvo, decimal) == Ordering::Less,
        OperadorCsv::MenorIgual => !v.is_empty() && comparar(valor, alvo, decimal) != Ordering::Greater,
        OperadorCsv::Comeca => v.starts_with(&a),
        OperadorCsv::Termina => v.ends_with(&a),
        OperadorCsv::Vazio => v.is_empty(),
        OperadorCsv::NaoVazio => !v.is_empty(),
    }
}

/// Calcula a vista (as posições da tabela que passam, na ordem) com um instantâneo da tabela. As condições e a ordem
/// de colunas que não existem mais ficam de fora.
pub(crate) fn calcular_vista(
    tabela: &TabelaCompartilhada,
    condicoes: &[Condicao],
    ordem: Option<&Ordem>,
    nomes: &[String],
    decimal: char,
    controle: &Controle,
) -> Resultado<Vec<u64>> {
    let indice = |nome: &str| nomes.iter().position(|n| n == nome);
    let mut usadas: Vec<usize> = condicoes.iter().filter_map(|c| indice(&c.coluna)).collect();
    let coluna_da_ordem = ordem.and_then(|o| indice(&o.coluna));
    usadas.extend(coluna_da_ordem);
    usadas.sort_unstable();
    usadas.dedup();
    let mut inst = sessao::instantaneo(tabela)?;
    let total = inst.linhas();
    let valores =
        if usadas.is_empty() { Vec::new() } else { inst.colunas(&usadas, &mut |f, t| controle.avisar(f, t))? };
    let da_coluna = |c: usize| usadas.iter().position(|&u| u == c).map(|k| &valores[k]);
    let filtros: Vec<(&Vec<String>, OperadorCsv, &str)> =
        condicoes.iter().filter_map(|c| Some((da_coluna(indice(&c.coluna)?)?, c.op, c.valor.as_str()))).collect();
    let mut posicoes: Vec<u64> = (0..total)
        .filter(|&i| {
            let i = i as usize;
            filtros.iter().all(|(col, op, alvo)| passa(col.get(i).map_or("", String::as_str), *op, alvo, decimal))
        })
        .collect();
    if let (Some(o), Some(col)) = (ordem, coluna_da_ordem.and_then(da_coluna)) {
        let valor = |p: u64| col.get(p as usize).map_or("", String::as_str);
        posicoes.sort_by(|&a, &b| {
            let c = comparar(valor(a), valor(b), decimal);
            if o.decrescente { c.reverse() } else { c }
        });
    }
    Ok(posicoes)
}

/// A vista em cálculo: o que a pediu (para descartar a resposta velha), a tarefa e o cancelamento dela (a vista
/// pedida de novo cancela a velha: os cálculos não se acumulam a cada alteração).
pub(crate) struct CalculoVista {
    pub chave: ChaveVista,
    pub promessa: estado::Promessa<Vec<u64>>,
    pub controle: std::sync::Arc<Controle>,
}

impl Drop for CalculoVista {
    /// Trocado ou largado, o cálculo para na próxima vez que avisar o progresso.
    fn drop(&mut self) {
        self.controle.cancelar();
    }
}

/// O que identifica uma vista: as condições, a ordem e a versão do documento.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChaveVista {
    pub condicoes: Vec<Condicao>,
    pub ordem: Option<Ordem>,
    pub versao: u64,
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn comparar_numeros_pelo_valor_e_texto_sem_caixa() {
        assert_eq!(comparar("9", "10", ','), Ordering::Less);
        assert_eq!(comparar("1.234,5", "999", ','), Ordering::Greater);
        assert_eq!(comparar("abc", "ABD", ','), Ordering::Less);
        assert_eq!(comparar("", "a", ','), Ordering::Greater, "o vazio no fim");
        assert_eq!(comparar("5", "a", ','), Ordering::Less, "números antes do texto");
    }

    #[test]
    fn condicoes_como_na_aba_dados() {
        assert!(passa("Sul", OperadorCsv::Igual, "sul", ','));
        assert!(!passa("Sudeste", OperadorCsv::Igual, "Sul", ','));
        assert!(passa("Sudeste", OperadorCsv::Comeca, "su", ','));
        assert!(passa("12,90", OperadorCsv::Maior, "9", ','));
        assert!(!passa("", OperadorCsv::Maior, "9", ','));
        assert!(passa("7,0", OperadorCsv::Igual, "7", ','));
        assert!(passa(" ", OperadorCsv::Vazio, "", ','));
        assert!(passa("Norte", OperadorCsv::NaoContem, "sul", ','));
        assert_eq!(OperadorCsv::NaoVazio.rotulo(), "não é vazio");
    }
}
