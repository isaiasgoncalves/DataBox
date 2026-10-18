//! O que a planilha leva para fora (decisão 16 da etapa 6): a regra num lugar só ([`texto_para_levar`]), que vale para o
//! Ctrl+C de colunas e linhas, o Copiar coluna e o Trazer colunas; e a leitura das colunas em fluxo ([`ler_colunas`],
//! pela `Pasta::linhas`, sem guardar a planilha inteira).
//!
//! - **Número**: o valor, sem formato, no menor texto que o representa (sem milhar e sem notação científica), com o
//!   separador decimal do destino: `1234,5`, `410000`.
//! - **Data, hora e data/hora**: o texto do formato da célula (o `exibir`: "12/10/2026").
//! - **Texto**: o texto. **Booleano**: "VERDADEIRO" ou "FALSO". **Erro**: o texto do erro ("#DIV/0!"). **Vazio**: "".
//!
//! O Ctrl+C de **uma** célula é outra coisa: copia o texto de exibição (cola no Excel como se vê).
//!
//! A origem **Planilha** do "Trazer colunas" é a [`FontePlanilha`]: uma planilha por entrada
//! ("relatorio_q3.xlsx › Vendas"), com a escolha "Primeira linha é o cabeçalho" (ligada quando as 50 primeiras linhas
//! parecem ter cabeçalho, pela mesma heurística do colar); desligada, as colunas se chamam A, B, C. A planilha nunca é
//! destino.

use std::sync::Arc;

use comum::{ErroBanco, Resultado};
use estado::{FonteDeColunas, TipoDeFonte};
use planilha::{Celula, Linha, Pasta, Valor, nome_da_coluna};
use tabular::colunas::{OrigemDoTexto, texto_tem_cabecalho};

/// Quantas linhas a heurística do cabeçalho olha.
pub const LINHAS_DA_HEURISTICA: usize = 50;

/// O decimal do destino quando não se sabe outro (o CSV em pt-BR).
pub const DECIMAL_PADRAO: char = ',';
/// De quantas em quantas linhas a leitura das colunas avisa o andamento.
const PASSO_DO_PROGRESSO: u64 = 4_096;

/// O número cru: o menor texto que o representa, sem milhar e sem expoente, com o `decimal` pedido.
pub fn numero_cru(n: f64, decimal: char) -> String {
    if !n.is_finite() {
        return String::new();
    }
    // o `Display` do f64 já é o menor texto que volta ao mesmo número, e nunca usa expoente
    let mut s = format!("{n}");
    if s == "-0" {
        s = "0".to_owned();
    }
    if decimal != '.' {
        s = s.replace('.', &decimal.to_string());
    }
    s
}

/// O texto que a célula leva para fora (veja o topo do módulo).
pub fn texto_para_levar(pasta: &Pasta, celula: &Celula, decimal: char) -> String {
    match &celula.valor {
        Valor::Vazio => String::new(),
        Valor::Numero(n) => numero_cru(*n, decimal),
        Valor::Texto(t) => t.clone(),
        Valor::Erro(e) => e.clone(),
        Valor::Booleano(_) | Valor::Data(_) | Valor::Hora(_) | Valor::DataHora(_) => pasta.exibir(celula),
    }
}

/// Os valores das colunas `colunas` (do arquivo, a partir de 0) da aba `folha`, da linha `primeira` do Excel (a primeira
/// é a 1) até `ultima` (ou até a última que existir), uma lista por coluna pedida, todas do mesmo tamanho: a linha que
/// não existe no arquivo vem vazia. Lê em fluxo. `ao_progresso(feito, total)` a cada 4.096 linhas (o total é `ultima`, ou
/// 0 sem ela); devolver erro interrompe a leitura (o erro volta).
pub fn ler_colunas(
    pasta: &Pasta,
    folha: usize,
    colunas: &[u32],
    primeira: u32,
    ultima: Option<u32>,
    decimal: char,
    ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
) -> Resultado<Vec<Vec<String>>> {
    let mut saida: Vec<Vec<String>> = vec![Vec::new(); colunas.len()];
    let primeira = primeira.max(1);
    let total = ultima.map_or(0, |u| u64::from(u.saturating_sub(primeira) + 1));
    let mut proxima = primeira;
    for linha in pasta.linhas(folha)? {
        let linha = linha?;
        if linha.numero < primeira {
            continue;
        }
        if ultima.is_some_and(|u| linha.numero > u) {
            break;
        }
        while proxima < linha.numero {
            saida.iter_mut().for_each(|v| v.push(String::new()));
            proxima += 1;
        }
        for (v, &c) in saida.iter_mut().zip(colunas) {
            v.push(linha.celula(c).map(|cel| texto_para_levar(pasta, cel, decimal)).unwrap_or_default());
        }
        proxima = linha.numero + 1;
        let feito = u64::from(proxima - primeira);
        if feito.is_multiple_of(PASSO_DO_PROGRESSO) {
            ao_progresso(feito, total)?;
        }
    }
    if let Some(u) = ultima {
        while proxima <= u {
            saida.iter_mut().for_each(|v| v.push(String::new()));
            proxima += 1;
        }
    }
    let feito = saida.first().map_or(0, |v| v.len() as u64);
    ao_progresso(feito, total.max(feito))?;
    Ok(saida)
}

/// A primeira linha parece ser o cabeçalho? A heurística do colar (`texto_tem_cabecalho`, texto de fora) nas
/// [`LINHAS_DA_HEURISTICA`] primeiras linhas, com os valores como a planilha os leva (a regra de [`texto_para_levar`]).
pub fn parece_ter_cabecalho(pasta: &Pasta, primeiras: &[&Linha], colunas: &[u32]) -> bool {
    let texto: Vec<Vec<String>> = primeiras
        .iter()
        .take(LINHAS_DA_HEURISTICA)
        .map(|l| {
            colunas
                .iter()
                .map(|&c| l.celula(c).map(|cel| texto_para_levar(pasta, cel, DECIMAL_PADRAO)).unwrap_or_default())
                .collect()
        })
        .collect();
    texto_tem_cabecalho(&texto, OrigemDoTexto::Externo, DECIMAL_PADRAO)
}

/// Uma planilha aberta como origem do "Trazer colunas" (veja o topo do módulo). Barata de clonar e de montar: os nomes
/// vêm da primeira linha que a tela já leu; as colunas são lidas em fluxo só quando a janela pede.
#[derive(Clone)]
pub struct FontePlanilha {
    pasta: Pasta,
    folha: usize,
    rotulo: String,
    /// As colunas do arquivo que a origem oferece (as visíveis) e o texto da primeira linha de cada uma.
    colunas: Vec<u32>,
    primeira_linha: Vec<String>,
    /// A última linha da planilha, se já se sabe (o índice).
    ultima: Option<u32>,
    cabecalho: bool,
    decimal: char,
}

impl FontePlanilha {
    /// A origem da aba `folha` da `pasta`, com as `colunas` do arquivo, o texto da primeira linha de cada uma, a última
    /// linha (se já se sabe) e a escolha do cabeçalho.
    pub fn nova(
        pasta: Pasta,
        folha: usize,
        colunas: Vec<u32>,
        primeira_linha: Vec<String>,
        ultima: Option<u32>,
        cabecalho: bool,
    ) -> FontePlanilha {
        let arquivo = pasta.caminho().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let nome = pasta.folhas().get(folha).map(|f| f.nome.clone()).unwrap_or_default();
        FontePlanilha {
            rotulo: format!("{arquivo} › {nome}"),
            pasta,
            folha,
            colunas,
            primeira_linha,
            ultima,
            cabecalho,
            decimal: DECIMAL_PADRAO,
        }
    }

    /// Com outro separador decimal para os números (o do CSV de destino; o padrão é a vírgula).
    pub fn com_decimal(mut self, decimal: char) -> FontePlanilha {
        self.decimal = decimal;
        self
    }

    /// A aba da pasta.
    pub fn folha(&self) -> usize {
        self.folha
    }
}

impl FonteDeColunas for FontePlanilha {
    fn rotulo(&self) -> String {
        self.rotulo.clone()
    }

    fn tipo(&self) -> TipoDeFonte {
        TipoDeFonte::Planilha
    }

    fn nomes(&self) -> Vec<String> {
        self.colunas
            .iter()
            .enumerate()
            .map(|(i, &c)| match self.primeira_linha.get(i) {
                Some(n) if self.cabecalho && !n.trim().is_empty() => n.trim().to_owned(),
                _ => nome_da_coluna(c),
            })
            .collect()
    }

    fn linhas(&self) -> Option<u64> {
        self.ultima.map(|u| u64::from(u.saturating_sub(u32::from(self.cabecalho))))
    }

    fn colunas(
        &self,
        quais: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let mut arquivo = Vec::with_capacity(quais.len());
        for &q in quais {
            let c = self.colunas.get(q).ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", q + 1)))?;
            arquivo.push(*c);
        }
        let primeira = if self.cabecalho { 2 } else { 1 };
        ler_colunas(&self.pasta, self.folha, &arquivo, primeira, self.ultima, self.decimal, ao_progresso)
    }

    fn primeira_linha_e_cabecalho(&self) -> Option<bool> {
        Some(self.cabecalho)
    }

    fn com_primeira_linha_cabecalho(&self, sim: bool) -> Option<Arc<dyn FonteDeColunas>> {
        Some(Arc::new(FontePlanilha { cabecalho: sim, ..self.clone() }))
    }

    fn para_o_decimal(&self, decimal: char) -> Option<Arc<dyn FonteDeColunas>> {
        Some(Arc::new(self.clone().com_decimal(decimal)))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numero_cru_sem_milhar_nem_expoente() {
        assert_eq!(numero_cru(410_000.0, ','), "410000");
        assert_eq!(numero_cru(1234.5, ','), "1234,5");
        assert_eq!(numero_cru(0.98, ','), "0,98");
        assert_eq!(numero_cru(1e-7, ','), "0,0000001");
        assert_eq!(numero_cru(1e16, ','), "10000000000000000");
        assert_eq!(numero_cru(-0.0, ','), "0");
        assert_eq!(numero_cru(-12.25, '.'), "-12.25");
    }
}
