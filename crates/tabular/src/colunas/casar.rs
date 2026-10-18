//! O motor do "Trazer colunas": casa as linhas de um destino com as de uma origem por uma coluna-chave, como um
//! PROCV sem fórmula. Funções puras sobre os textos das chaves; quem chama lê as colunas (do CSV, de uma
//! tabela do SQLite, de uma planilha, da área de transferência) e aplica o resultado.
//!
//! Regras:
//! - a chave é comparada depois de [`normalizar`] com as [`OpcoesCasamento`] (ignorar maiúsculas, espaços nas
//!   pontas, números pelo valor);
//! - chave vazia nunca casa (nem do destino, nem da origem): como um PROCV de célula em branco, é "sem par";
//! - chave repetida na origem: o casamento guarda todas as linhas, e a escolha (a primeira, a última ou todas
//!   juntas com `"; "`) se faz depois, sem refazer a conta, em [`Casamento::trazer`].

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use crate::numero;

/// O que une os valores de uma chave repetida no modo [`ChavesRepetidas::TodasJuntas`].
pub const SEPARADOR_DE_REPETIDAS: &str = "; ";

/// Quantas linhas de cada lado a sugestão automática de chaves examina.
pub const AMOSTRA_DA_SUGESTAO: usize = 2000;

/// As três opções do bloco "Casar as linhas onde".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcoesCasamento {
    /// "Ignorar maiúsculas" (ligada de saída).
    pub ignorar_maiusculas: bool,
    /// "Ignorar espaços nas pontas" (ligada de saída).
    pub ignorar_espacos: bool,
    /// "Números pelo valor" (desligada de saída): `007`, `7` e `7,0` casam. Desligada, os zeros à esquerda de
    /// códigos como CEP e CPF valem.
    pub numeros_pelo_valor: bool,
}

impl Default for OpcoesCasamento {
    fn default() -> Self {
        OpcoesCasamento { ignorar_maiusculas: true, ignorar_espacos: true, numeros_pelo_valor: false }
    }
}

/// O que fazer quando uma chave aparece mais de uma vez na origem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChavesRepetidas {
    /// "Usar: a primeira".
    #[default]
    Primeira,
    /// "a última".
    Ultima,
    /// `todas, juntas com "; "`: os valores não vazios, na ordem da origem.
    TodasJuntas,
}

/// A forma da chave que se compara: com as opções aplicadas ao texto.
pub fn normalizar<'a>(valor: &'a str, opcoes: &OpcoesCasamento) -> Cow<'a, str> {
    let base = if opcoes.ignorar_espacos { valor.trim() } else { valor };
    if opcoes.numeros_pelo_valor
        && let Some(canonico) = numero::canonico(base)
    {
        return canonico;
    }
    if opcoes.ignorar_maiusculas {
        if base.is_ascii() {
            if base.bytes().any(|b| b.is_ascii_uppercase()) {
                return Cow::Owned(base.to_ascii_lowercase());
            }
        } else if base.chars().any(char::is_uppercase) {
            return Cow::Owned(base.to_lowercase());
        }
    }
    Cow::Borrowed(base)
}

/// O bit que, no valor guardado no índice, diz que o valor é o número de um grupo de linhas (a chave repete) e
/// não o número de uma linha só.
const BIT_DE_GRUPO: usize = 1 << (usize::BITS - 1);

/// A origem indexada: a chave normalizada → o número da linha (se aparece uma vez) ou o grupo das linhas (se
/// repete). Chaves vazias ficam de fora.
struct IndiceDaOrigem<'a> {
    mapa: HashMap<&'a str, usize>,
    grupos: Vec<Vec<usize>>,
}

/// As chaves da origem já normalizadas (o que se compara).
fn normalizar_todas<'a, S: AsRef<str>>(chaves: &'a [S], opcoes: &OpcoesCasamento) -> Vec<Cow<'a, str>> {
    chaves.iter().map(|c| normalizar(c.as_ref(), opcoes)).collect()
}

fn montar_indice<'a>(normalizadas: &'a [Cow<'_, str>]) -> IndiceDaOrigem<'a> {
    let mut mapa: HashMap<&'a str, usize> = HashMap::with_capacity(normalizadas.len());
    let mut grupos: Vec<Vec<usize>> = Vec::new();
    for (i, k) in normalizadas.iter().enumerate() {
        let k: &'a str = k;
        if k.is_empty() {
            continue;
        }
        match mapa.entry(k) {
            Entry::Vacant(v) => {
                v.insert(i);
            }
            Entry::Occupied(mut o) => {
                let atual = *o.get();
                if atual & BIT_DE_GRUPO == 0 {
                    let mut grupo = Vec::with_capacity(4);
                    grupo.extend([atual, i]);
                    grupos.push(grupo);
                    o.insert(BIT_DE_GRUPO | (grupos.len() - 1));
                } else if let Some(grupo) = grupos.get_mut(atual & !BIT_DE_GRUPO) {
                    grupo.push(i);
                }
            }
        }
    }
    IndiceDaOrigem { mapa, grupos }
}

/// A chave normalizada → as linhas da origem que a têm (na ordem da origem). Chaves vazias ficam de fora.
pub fn indice_da_origem<S: AsRef<str>>(chaves: &[S], opcoes: &OpcoesCasamento) -> HashMap<String, Vec<usize>> {
    let normalizadas = normalizar_todas(chaves, opcoes);
    let indice = montar_indice(&normalizadas);
    indice
        .mapa
        .iter()
        .map(|(k, &v)| {
            let linhas = if v & BIT_DE_GRUPO == 0 {
                vec![v]
            } else {
                indice.grupos.get(v & !BIT_DE_GRUPO).cloned().unwrap_or_default()
            };
            ((*k).to_string(), linhas)
        })
        .collect()
}
/// O par de uma linha do destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Par {
    Nenhum,
    Um(usize),
    Varios(usize),
}

/// As linhas da origem que casaram com uma linha do destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linhas<'a> {
    Nenhuma,
    Uma(usize),
    /// Mais de uma, na ordem da origem.
    Varias(&'a [usize]),
}

impl Linhas<'_> {
    pub fn como_fatia(&self) -> &[usize] {
        match self {
            Linhas::Nenhuma => &[],
            Linhas::Uma(o) => std::slice::from_ref(o),
            Linhas::Varias(v) => v,
        }
    }

    pub fn quantidade(&self) -> usize {
        self.como_fatia().len()
    }

    pub fn primeira(&self) -> Option<usize> {
        self.como_fatia().first().copied()
    }

    pub fn ultima(&self) -> Option<usize> {
        self.como_fatia().last().copied()
    }
}

/// Uma chave que aparece mais de uma vez na origem (o "Ver quais").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repetida {
    /// O texto da chave na primeira vez que aparece.
    pub chave: String,
    /// As linhas da origem (de 0), na ordem.
    pub linhas: Vec<usize>,
}

/// O resultado de [`casar`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Casamento {
    /// Uma entrada por linha do destino, na ordem; veja [`Casamento::linhas_da_origem`].
    pares: Vec<Par>,
    /// As linhas do destino (de 0) sem par: o "Marcar as linhas sem par".
    pub sem_par: Vec<usize>,
    /// As chaves repetidas na origem, pela ordem em que aparecem.
    pub repetidas: Vec<Repetida>,
    grupos: Vec<Vec<usize>>,
}

impl Casamento {
    /// Linhas do destino.
    pub fn total(&self) -> usize {
        self.pares.len()
    }

    /// Linhas do destino que têm par na origem.
    pub fn com_par(&self) -> usize {
        self.pares.len() - self.sem_par.len()
    }

    /// O medidor "4.812 de 5.000 linhas têm par (96%)": o percentual inteiro, arredondado. Não mostra 100%
    /// se alguma linha ficou sem par, nem 0% se alguma casou.
    pub fn percentual(&self) -> u8 {
        let total = self.total();
        if total == 0 {
            return 0;
        }
        let com = self.com_par();
        let p = (com * 100 + total / 2) / total;
        let p = if com < total { p.min(99) } else { p };
        let p = if com > 0 { p.max(1) } else { p };
        u8::try_from(p).unwrap_or(100)
    }

    /// As linhas da origem que casaram com a linha `i` do destino.
    pub fn linhas_da_origem(&self, i: usize) -> Linhas<'_> {
        match self.pares.get(i) {
            Some(Par::Um(o)) => Linhas::Uma(*o),
            Some(Par::Varios(g)) => Linhas::Varias(self.grupos.get(*g).map_or(&[][..], Vec::as_slice)),
            Some(Par::Nenhum) | None => Linhas::Nenhuma,
        }
    }

    /// O valor que a linha `i` do destino recebe da `coluna_origem` (uma célula por linha da origem): `None`
    /// sem par. Com chave repetida, vale o `modo`.
    pub fn trazer_valor<S: AsRef<str>>(&self, i: usize, coluna_origem: &[S], modo: ChavesRepetidas) -> Option<String> {
        let valor = |o: usize| coluna_origem.get(o).map_or("", AsRef::as_ref);
        match self.linhas_da_origem(i) {
            Linhas::Nenhuma => None,
            Linhas::Uma(o) => Some(valor(o).to_string()),
            Linhas::Varias(v) => Some(match modo {
                ChavesRepetidas::Primeira => v.first().map_or("", |&o| valor(o)).to_string(),
                ChavesRepetidas::Ultima => v.last().map_or("", |&o| valor(o)).to_string(),
                ChavesRepetidas::TodasJuntas => v
                    .iter()
                    .map(|&o| valor(o))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(SEPARADOR_DE_REPETIDAS),
            }),
        }
    }

    /// A coluna trazida: uma entrada por linha do destino (`None` nas sem par).
    pub fn trazer<S: AsRef<str>>(&self, coluna_origem: &[S], modo: ChavesRepetidas) -> Vec<Option<String>> {
        (0..self.pares.len()).map(|i| self.trazer_valor(i, coluna_origem, modo)).collect()
    }
}

/// A partir de quantas linhas no destino a busca se divide entre as threads do computador.
const PARALELO_A_PARTIR_DE: usize = 100_000;

/// Casa as linhas do `destino` (a coluna-chave dele) com as da `origem` (a coluna-chave dela). Com muitas
/// linhas, a busca das chaves do destino se divide entre as threads (o índice da origem é só lido).
pub fn casar<D: AsRef<str> + Sync, O: AsRef<str>>(destino: &[D], origem: &[O], opcoes: &OpcoesCasamento) -> Casamento {
    let normalizadas = normalizar_todas(origem, opcoes);
    let IndiceDaOrigem { mapa, grupos } = montar_indice(&normalizadas);
    let procurar = |trecho: &[D]| -> Vec<Par> {
        trecho
            .iter()
            .map(|d| {
                let k = normalizar(d.as_ref(), opcoes);
                match if k.is_empty() { None } else { mapa.get(k.as_ref()) } {
                    None => Par::Nenhum,
                    Some(&v) if v & BIT_DE_GRUPO == 0 => Par::Um(v),
                    Some(&v) => Par::Varios(v & !BIT_DE_GRUPO),
                }
            })
            .collect()
    };
    let threads = if destino.len() >= PARALELO_A_PARTIR_DE {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get).min(8)
    } else {
        1
    };
    let pares: Vec<Par> = if threads <= 1 {
        procurar(destino)
    } else {
        let tamanho = destino.len().div_ceil(threads);
        std::thread::scope(|escopo| {
            let procurar = &procurar;
            let trabalhos: Vec<_> = destino.chunks(tamanho).map(|c| escopo.spawn(move || procurar(c))).collect();
            let mut pares = Vec::with_capacity(destino.len());
            for t in trabalhos {
                match t.join() {
                    Ok(parte) => pares.extend(parte),
                    Err(erro) => std::panic::resume_unwind(erro),
                }
            }
            pares
        })
    };
    let sem_par: Vec<usize> = pares.iter().enumerate().filter(|(_, p)| **p == Par::Nenhum).map(|(i, _)| i).collect();
    let mut repetidas: Vec<Repetida> = grupos
        .iter()
        .map(|g| {
            let chave = g.first().and_then(|&o| origem.get(o)).map_or("", AsRef::as_ref);
            Repetida { chave: chave.to_string(), linhas: g.clone() }
        })
        .collect();
    repetidas.sort_by_key(|r| r.linhas.first().copied());
    Casamento { pares, sem_par, repetidas, grupos }
}
/// A sugestão automática das colunas-chave: a coluna de mesmo nome (sem diferenciar maiúsculas nem espaços
/// nas pontas); sem ela, o par que mais valores distintos tem em comum numa amostra de
/// [`AMOSTRA_DA_SUGESTAO`] linhas. As amostras são por coluna (`amostra_destino[j]` são os valores da coluna
/// `j`). Devolve (coluna do destino, coluna da origem); `None` se nenhum par tem valor em comum.
pub fn sugerir_chaves(
    nomes_destino: &[String],
    amostra_destino: &[Vec<String>],
    nomes_origem: &[String],
    amostra_origem: &[Vec<String>],
    opcoes: &OpcoesCasamento,
) -> Option<(usize, usize)> {
    let nome = |n: &str| n.trim().to_lowercase();
    let da_origem: Vec<String> = nomes_origem.iter().map(|n| nome(n)).collect();
    for (d, n) in nomes_destino.iter().enumerate() {
        let n = nome(n);
        if !n.is_empty()
            && let Some(o) = da_origem.iter().position(|x| *x == n)
        {
            return Some((d, o));
        }
    }
    let distintos = |valores: &Vec<String>| -> HashSet<String> {
        valores
            .iter()
            .take(AMOSTRA_DA_SUGESTAO)
            .map(|v| normalizar(v, opcoes))
            .filter(|k| !k.is_empty())
            .map(Cow::into_owned)
            .collect()
    };
    let origens: Vec<HashSet<String>> = amostra_origem.iter().map(distintos).collect();
    let mut melhor: Option<(usize, usize, usize)> = None;
    for (d, valores) in amostra_destino.iter().enumerate() {
        let conjunto = distintos(valores);
        for (o, da_origem) in origens.iter().enumerate() {
            let comuns = conjunto.iter().filter(|k| da_origem.contains(k.as_str())).count();
            if comuns > 0 && melhor.is_none_or(|(_, _, m)| comuns > m) {
                melhor = Some((d, o, comuns));
            }
        }
    }
    melhor.map(|(d, o, _)| (d, o))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn v(textos: &[&str]) -> Vec<String> {
        textos.iter().map(|t| t.to_string()).collect()
    }

    fn opcoes(maiusculas: bool, espacos: bool, numeros: bool) -> OpcoesCasamento {
        OpcoesCasamento { ignorar_maiusculas: maiusculas, ignorar_espacos: espacos, numeros_pelo_valor: numeros }
    }

    #[test]
    fn normalizar_cada_opcao() {
        let todas = opcoes(true, true, true);
        let nenhuma = opcoes(false, false, false);
        assert_eq!(normalizar("  ABC ", &opcoes(true, true, false)), "abc");
        assert_eq!(normalizar("  ABC ", &opcoes(false, true, false)), "ABC");
        assert_eq!(normalizar("  ABC ", &opcoes(true, false, false)), "  abc ");
        assert_eq!(normalizar("  ABC ", &nenhuma), "  ABC ");
        assert_eq!(normalizar("007", &opcoes(true, true, false)), "007");
        assert_eq!(normalizar("007", &todas), "7");
        assert_eq!(normalizar(" 7,0 ", &todas), "7");
        assert_eq!(normalizar(" 7,0 ", &opcoes(true, false, true)), " 7,0 ", "com espaço guardado, não é número");
        assert_eq!(normalizar("ÁGUA", &opcoes(true, true, false)), "água");
        assert_eq!(normalizar("", &todas), "");
        assert_eq!(normalizar("   ", &todas), "");
        assert!(
            matches!(normalizar("abc", &opcoes(true, true, false)), Cow::Borrowed(_)),
            "sem copiar quando já está pronto"
        );
    }

    #[test]
    fn padrao_das_opcoes() {
        let o = OpcoesCasamento::default();
        assert!(o.ignorar_maiusculas && o.ignorar_espacos && !o.numeros_pelo_valor);
    }

    #[test]
    fn casa_ignorando_maiusculas_e_espacos() {
        let c = casar(&v(&["A1", " b2 ", "C3", "d4"]), &v(&["a1", "B2", "c3 "]), &OpcoesCasamento::default());
        assert_eq!(c.total(), 4);
        assert_eq!(c.com_par(), 3);
        assert_eq!(c.sem_par, vec![3]);
        assert_eq!(c.linhas_da_origem(0), Linhas::Uma(0));
        assert_eq!(c.linhas_da_origem(1), Linhas::Uma(1));
        assert_eq!(c.linhas_da_origem(3), Linhas::Nenhuma);
        assert_eq!(c.linhas_da_origem(99), Linhas::Nenhuma);
        assert!(c.repetidas.is_empty());
    }

    #[test]
    fn sem_as_opcoes_nao_casa() {
        let destino = v(&["A1", " b2 "]);
        let origem = v(&["a1", "b2"]);
        let c = casar(&destino, &origem, &opcoes(false, true, false));
        assert_eq!(c.sem_par, vec![0], "só o espaço foi ignorado");
        let c = casar(&destino, &origem, &opcoes(true, false, false));
        assert_eq!(c.sem_par, vec![1], "só a maiúscula foi ignorada");
        let c = casar(&destino, &origem, &opcoes(false, false, false));
        assert_eq!(c.sem_par, vec![0, 1]);
    }

    #[test]
    fn numeros_pelo_valor_so_com_a_opcao() {
        let destino = v(&["007", "7", "7,0", "1.234,50", "abc"]);
        let origem = v(&["7", "1234,5", "ABC"]);
        let sem = casar(&destino, &origem, &opcoes(true, true, false));
        assert_eq!(sem.sem_par, vec![0, 2, 3], "só o 7 e o abc casam como texto");
        let com = casar(&destino, &origem, &opcoes(true, true, true));
        assert_eq!(com.sem_par, Vec::<usize>::new());
        assert_eq!(com.linhas_da_origem(0), Linhas::Uma(0), "007 casa com 7");
        assert_eq!(com.linhas_da_origem(3), Linhas::Uma(1), "1.234,50 casa com 1234,5");
        assert_eq!(com.linhas_da_origem(4), Linhas::Uma(2));
    }

    #[test]
    fn codigos_com_zeros_ficam_diferentes_por_padrao() {
        let c = casar(&v(&["004718", "4718"]), &v(&["4718", "004718"]), &OpcoesCasamento::default());
        assert_eq!(c.linhas_da_origem(0), Linhas::Uma(1));
        assert_eq!(c.linhas_da_origem(1), Linhas::Uma(0));
    }

    #[test]
    fn numeros_enormes_nao_perdem_precisao() {
        let a = "12345678901234567890";
        let b = "12345678901234567891";
        let c = casar(&v(&[a]), &v(&[b, a]), &opcoes(true, true, true));
        assert_eq!(c.linhas_da_origem(0), Linhas::Uma(1));
    }

    #[test]
    fn chave_vazia_nunca_casa() {
        let c = casar(&v(&["", "  ", "a"]), &v(&["", "a", "  "]), &OpcoesCasamento::default());
        assert_eq!(c.sem_par, vec![0, 1]);
        assert_eq!(c.linhas_da_origem(2), Linhas::Uma(1));
        assert!(c.repetidas.is_empty(), "as vazias da origem não contam como repetidas");
    }

    fn com_repetidas() -> (Casamento, Vec<String>) {
        let destino = v(&["x", "y", "z", "w"]);
        let origem = v(&["x", "y", "X", "z", "y", "y"]);
        let valores = v(&["v0", "v1", "v2", "", "v4", ""]);
        (casar(&destino, &origem, &OpcoesCasamento::default()), valores)
    }

    #[test]
    fn repetidas_sao_listadas_pela_ordem_da_origem() {
        let (c, _) = com_repetidas();
        assert_eq!(
            c.repetidas,
            vec![
                Repetida { chave: "x".into(), linhas: vec![0, 2] },
                Repetida { chave: "y".into(), linhas: vec![1, 4, 5] },
            ]
        );
        assert_eq!(c.linhas_da_origem(0), Linhas::Varias(&[0, 2]));
        assert_eq!(c.linhas_da_origem(1).como_fatia(), &[1, 4, 5]);
        assert_eq!(c.linhas_da_origem(1).quantidade(), 3);
        assert_eq!(c.linhas_da_origem(1).primeira(), Some(1));
        assert_eq!(c.linhas_da_origem(1).ultima(), Some(5));
        assert_eq!(c.linhas_da_origem(3).primeira(), None);
    }

    #[test]
    fn repetidas_modo_primeira() {
        let (c, valores) = com_repetidas();
        let r = c.trazer(&valores, ChavesRepetidas::Primeira);
        assert_eq!(r, vec![Some("v0".to_string()), Some("v1".to_string()), Some("".to_string()), None]);
    }

    #[test]
    fn repetidas_modo_ultima() {
        let (c, valores) = com_repetidas();
        let r = c.trazer(&valores, ChavesRepetidas::Ultima);
        assert_eq!(r, vec![Some("v2".to_string()), Some("".to_string()), Some("".to_string()), None]);
    }

    #[test]
    fn repetidas_modo_todas_juntas_pula_os_vazios() {
        let (c, valores) = com_repetidas();
        let r = c.trazer(&valores, ChavesRepetidas::TodasJuntas);
        assert_eq!(r, vec![Some("v0; v2".to_string()), Some("v1; v4".to_string()), Some("".to_string()), None]);
    }

    #[test]
    fn trazer_uma_linha_so_e_valores_fora_da_origem() {
        let (c, valores) = com_repetidas();
        assert_eq!(c.trazer_valor(0, &valores, ChavesRepetidas::TodasJuntas).as_deref(), Some("v0; v2"));
        assert_eq!(c.trazer_valor(3, &valores, ChavesRepetidas::TodasJuntas), None);
        assert_eq!(c.trazer_valor(99, &valores, ChavesRepetidas::Primeira), None);
        // coluna da origem mais curta que as linhas casadas: o que falta é vazio
        let curta = v(&["so uma"]);
        assert_eq!(c.trazer_valor(1, &curta, ChavesRepetidas::Primeira).as_deref(), Some(""));
    }

    #[test]
    fn indice_da_origem_publico() {
        let i = indice_da_origem(&v(&["a", "B", "A", "", "b "]), &OpcoesCasamento::default());
        assert_eq!(i.len(), 2);
        assert_eq!(i["a"], vec![0, 2]);
        assert_eq!(i["b"], vec![1, 4]);
    }

    #[test]
    fn percentual_do_medidor() {
        let origem: Vec<String> = (0..5000).map(|i| format!("k{i}")).collect();
        let destino: Vec<String> = (0..5000).map(|i| format!("k{}", if i < 4812 { i } else { 10_000 + i })).collect();
        let c = casar(&destino, &origem, &OpcoesCasamento::default());
        assert_eq!((c.com_par(), c.total(), c.percentual()), (4812, 5000, 96));
        assert_eq!(c.sem_par.len(), 188);
        let nenhum = casar(&v(&["a"]), &v(&["b"]), &OpcoesCasamento::default());
        assert_eq!(nenhum.percentual(), 0);
        let todos = casar(&v(&["a"]), &v(&["a"]), &OpcoesCasamento::default());
        assert_eq!(todos.percentual(), 100);
        let vazio: Vec<String> = Vec::new();
        assert_eq!(casar(&vazio, &v(&["a"]), &OpcoesCasamento::default()).percentual(), 0);
        // quase todos: nunca 100% se falta um; quase nenhum: nunca 0% se casou um
        let muitos: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
        let mut quase = muitos.clone();
        quase[0] = "zzz".into();
        assert_eq!(casar(&quase, &muitos, &OpcoesCasamento::default()).percentual(), 99);
        let mut pouco = vec!["nada".to_string(); 1000];
        pouco[0] = "5".into();
        assert_eq!(casar(&pouco, &muitos, &OpcoesCasamento::default()).percentual(), 1);
    }

    #[test]
    fn destino_e_origem_vazios() {
        let vazio: Vec<String> = Vec::new();
        let c = casar(&vazio, &vazio, &OpcoesCasamento::default());
        assert_eq!((c.total(), c.com_par()), (0, 0));
        assert!(c.trazer(&vazio, ChavesRepetidas::Primeira).is_empty());
        let c = casar(&v(&["a", "b"]), &vazio, &OpcoesCasamento::default());
        assert_eq!(c.sem_par, vec![0, 1]);
    }

    fn colunas(m: &[&[&str]]) -> Vec<Vec<String>> {
        m.iter().map(|c| v(c)).collect()
    }

    #[test]
    fn sugestao_pela_coluna_de_mesmo_nome() {
        let r = sugerir_chaves(
            &v(&["deposito", " Codigo ", "estoque"]),
            &colunas(&[&["a"], &["1"], &["2"]]),
            &v(&["nome", "categoria", "CODIGO"]),
            &colunas(&[&["x"], &["y"], &["1"]]),
            &OpcoesCasamento::default(),
        );
        assert_eq!(r, Some((1, 2)));
    }

    #[test]
    fn sugestao_pelo_par_com_mais_valores_em_comum() {
        let r = sugerir_chaves(
            &v(&["a", "b", "c"]),
            &colunas(&[&["g", "g", "h"], &["004718", "004725", "004731", "019044"], &["10", "20", "30"]]),
            &v(&["x", "y", "z"]),
            &colunas(&[&["m", "n"], &["10", "11"], &["004725", "004731", "005102", "004718"]]),
            &OpcoesCasamento::default(),
        );
        assert_eq!(r, Some((1, 2)), "três códigos em comum, contra um número");
    }

    #[test]
    fn sugestao_respeita_as_opcoes_e_pode_nao_achar() {
        let destino = colunas(&[&["ABC", "DEF"]]);
        let origem = colunas(&[&["abc", "def"]]);
        let nomes_d = v(&["a"]);
        let nomes_o = v(&["b"]);
        assert_eq!(sugerir_chaves(&nomes_d, &destino, &nomes_o, &origem, &OpcoesCasamento::default()), Some((0, 0)));
        assert_eq!(sugerir_chaves(&nomes_d, &destino, &nomes_o, &origem, &opcoes(false, true, false)), None);
        assert_eq!(sugerir_chaves(&[], &[], &[], &[], &OpcoesCasamento::default()), None);
        let so_vazios = colunas(&[&["", " "]]);
        assert_eq!(sugerir_chaves(&nomes_d, &so_vazios, &nomes_o, &so_vazios, &OpcoesCasamento::default()), None);
    }

    #[test]
    #[ignore = "benchmark: cargo test --release -p tabular -- --ignored --nocapture"]
    fn um_milhao_de_linhas_casam_em_menos_de_1_segundo() {
        use std::time::{Duration, Instant};
        let n = 1_000_000u64;
        let precos: Vec<String> = (0..n).map(|i| format!("{},{:02}", i % 1000, i % 100)).collect();
        let casos: Vec<(&str, Vec<String>, Vec<String>, OpcoesCasamento)> = vec![
            (
                "códigos numéricos, opções de saída",
                (0..n).map(|i| format!("{i:07}")).collect(),
                (0..n).map(|i| format!("{:07}", (i * 7919) % 1_050_000)).collect(),
                OpcoesCasamento::default(),
            ),
            (
                "códigos numéricos, Números pelo valor",
                (0..n).map(|i| format!("{i:07}")).collect(),
                (0..n).map(|i| format!("{:07}", (i * 7919) % 1_050_000)).collect(),
                opcoes(true, true, true),
            ),
            (
                "texto com maiúsculas e espaços (cada chave é copiada)",
                (0..n).map(|i| format!(" SKU-{i:07}-AB ")).collect(),
                (0..n).map(|i| format!("sku-{:07}-ab", (i * 7919) % 1_050_000)).collect(),
                OpcoesCasamento::default(),
            ),
            (
                "chaves repetidas na origem (cada uma 5 vezes)",
                (0..n).map(|i| format!("{:07}", i % (n / 5))).collect(),
                (0..n).map(|i| format!("{i:07}")).collect(),
                OpcoesCasamento::default(),
            ),
        ];
        for (nome, origem, destino, opcoes) in casos {
            let t0 = Instant::now();
            let normalizadas = normalizar_todas(&origem, &opcoes);
            eprintln!("  normalizar a origem: {:?}", t0.elapsed());
            let t0 = Instant::now();
            let indice = montar_indice(&normalizadas);
            eprintln!("  indexar a origem: {:?} ({} chaves)", t0.elapsed(), indice.mapa.len());
            drop(indice);
            // a melhor de três rodadas: o computador pode estar ocupado com outra coisa, e o que se mede é o motor
            let (mut tempo_casar, mut tempo_trazer) = (Duration::MAX, Duration::MAX);
            let mut resultado = None;
            for _ in 0..3 {
                let t0 = Instant::now();
                let c = casar(&destino, &origem, &opcoes);
                tempo_casar = tempo_casar.min(t0.elapsed());
                let t0 = Instant::now();
                let trazidos = c.trazer(&precos, ChavesRepetidas::TodasJuntas);
                tempo_trazer = tempo_trazer.min(t0.elapsed());
                assert_eq!(trazidos.len(), n as usize);
                resultado = Some(c);
            }
            let c = resultado.unwrap();
            eprintln!(
                "{nome}: casar 1.000.000 x 1.000.000 em {tempo_casar:?} ({} com par, {} repetidas); trazer a coluna em {tempo_trazer:?} (melhor de 3)",
                c.com_par(),
                c.repetidas.len()
            );
            let limite = if cfg!(debug_assertions) { Duration::from_secs(60) } else { Duration::from_secs(1) };
            assert!(tempo_casar < limite, "{nome}: casar demorou {tempo_casar:?}");
        }
    }

    #[test]
    fn sugestao_olha_so_a_amostra() {
        let mut destino = vec!["nada".to_string(); AMOSTRA_DA_SUGESTAO];
        destino.push("so_depois_da_amostra".into());
        let origem = v(&["so_depois_da_amostra"]);
        let r = sugerir_chaves(&v(&["a"]), &[destino], &v(&["b"]), &[origem], &OpcoesCasamento::default());
        assert_eq!(r, None);
    }
}
