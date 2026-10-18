//! A origem do "Trazer colunas": o trait [`FonteDeColunas`], que cada
//! documento com tabela oferece pelo [`Documento::fonte_de_colunas`](crate::Documento::fonte_de_colunas), e o
//! [`TipoDeFonte`], que diz de que tipo ela é (o ícone e a ordem da lista da janela).
//!
//! A casca monta a lista das abas abertas a cada quadro (barato: uma `Arc` por aba) e a põe em
//! [`Contexto::fontes`](crate::Contexto::fontes). A janela "Trazer colunas" não sabe de onde vêm as colunas: pergunta
//! o rótulo, os nomes e, numa tarefa de fundo, os valores de algumas colunas.
//!
//! As fontes são lidas **fora** da thread da interface: [`FonteDeColunas::colunas`] pode ler disco (o CSV trava a
//! `Tabela` dele só ali dentro), por isso o trait é `Send + Sync` e a janela chama dentro de `Promessa::rodar`.

use comum::Resultado;

/// De que tipo é uma origem de colunas. A ordem das variantes é a da lista da janela.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TipoDeFonte {
    /// Um CSV aberto numa aba (com as alterações ainda não salvas).
    Csv,
    /// Uma planilha aberta numa aba (etapa 6).
    Planilha,
    /// Uma tabela ou visão do banco (extensão anotada na prancheta 16).
    Tabela,
    /// O resultado de uma Consulta SQL (extensão anotada na prancheta 16).
    Consulta,
    /// O texto da área de transferência lido como tabela.
    AreaDeTransferencia,
}

impl TipoDeFonte {
    /// Todos os tipos, na ordem da lista da janela.
    pub const TODOS: [TipoDeFonte; 5] = [
        TipoDeFonte::Csv,
        TipoDeFonte::Planilha,
        TipoDeFonte::Tabela,
        TipoDeFonte::Consulta,
        TipoDeFonte::AreaDeTransferencia,
    ];

    /// O nome do tipo em palavras ("CSV", "Planilha", "Tabela", "Consulta", "Área de transferência").
    pub fn rotulo(self) -> &'static str {
        match self {
            TipoDeFonte::Csv => "CSV",
            TipoDeFonte::Planilha => "Planilha",
            TipoDeFonte::Tabela => "Tabela",
            TipoDeFonte::Consulta => "Consulta",
            TipoDeFonte::AreaDeTransferencia => "Área de transferência",
        }
    }
}

/// Uma origem de colunas para o "Trazer colunas": os nomes das colunas, o número de linhas e os valores de algumas
/// delas, sempre como texto.
///
/// Quem implementa não é a interface: as chamadas que leem disco ([`FonteDeColunas::colunas`]) rodam numa tarefa.
/// [`FonteDeColunas::rotulo`], [`FonteDeColunas::tipo`], [`FonteDeColunas::nomes`] e [`FonteDeColunas::linhas`] têm de
/// ser baratos (sem disco): a janela os chama a cada quadro.
pub trait FonteDeColunas: Send + Sync {
    /// O nome para o seletor da janela ("vendas_2025.csv", "Área de transferência").
    fn rotulo(&self) -> String;

    /// O tipo da origem (o ícone ao lado do rótulo).
    fn tipo(&self) -> TipoDeFonte;

    /// Os nomes das colunas, na ordem da origem.
    fn nomes(&self) -> Vec<String>;

    /// Quantas linhas há, se já se sabe (`None`: ainda não contada ou desconhecida).
    fn linhas(&self) -> Option<u64>;

    /// Os valores das colunas `quais` (índices de [`FonteDeColunas::nomes`]), uma lista de valores por coluna pedida,
    /// na ordem do pedido, todas com o mesmo tamanho (uma célula por linha). Pode ler disco e demorar:
    /// `ao_progresso(feito, total)` é chamado de vez em quando e **devolver erro cancela** a leitura (o erro volta
    /// como resultado). Índice fora do intervalo é erro, não pânico.
    fn colunas(
        &self,
        quais: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>>;

    /// A primeira linha é o cabeçalho (os nomes das colunas)? `None`: a origem não tem essa escolha (o CSV, que tem o
    /// dele). A planilha tem (etapa 6): a caixa "Primeira linha é o cabeçalho" ao lado do seletor da janela.
    fn primeira_linha_e_cabecalho(&self) -> Option<bool> {
        None
    }

    /// A mesma origem com a escolha da caixa trocada (os nomes e as linhas mudam junto). `None` para quem não tem a
    /// escolha. Barato (sem disco): a janela chama quando a caixa muda.
    fn com_primeira_linha_cabecalho(&self, _sim: bool) -> Option<std::sync::Arc<dyn FonteDeColunas>> {
        None
    }

    /// A mesma origem escrevendo os números com o separador decimal do destino (`,` ou `.`: o do CSV que recebe as
    /// colunas). `None` para quem já entrega o texto como está no arquivo (o CSV, a área de transferência). A
    /// planilha leva o número cru (decisão 16 da etapa 6) e precisa saber o decimal. Barato; quem lê chama antes de
    /// [`FonteDeColunas::colunas`], dentro da tarefa.
    fn para_o_decimal(&self, _decimal: char) -> Option<std::sync::Arc<dyn FonteDeColunas>> {
        None
    }
}

/// Uma origem de mentira para os testes do `estado` (duas colunas de texto, `linhas` linhas): lê como o contrato manda,
/// com progresso por coluna, o erro do progresso cancelando e o índice fora do intervalo virando erro.
#[cfg(test)]
pub(crate) struct FonteFalsa {
    pub rotulo: String,
    pub tipo: TipoDeFonte,
    pub linhas: u64,
}

#[cfg(test)]
impl FonteDeColunas for FonteFalsa {
    fn rotulo(&self) -> String {
        self.rotulo.clone()
    }

    fn tipo(&self) -> TipoDeFonte {
        self.tipo
    }

    fn nomes(&self) -> Vec<String> {
        vec!["id".to_string(), "nome".to_string()]
    }

    fn linhas(&self) -> Option<u64> {
        Some(self.linhas)
    }

    fn colunas(
        &self,
        quais: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let mut saida = Vec::with_capacity(quais.len());
        for (feito, &c) in quais.iter().enumerate() {
            if c >= self.nomes().len() {
                return Err(comum::ErroBanco::novo(format!("A coluna {c} não existe.")));
            }
            ao_progresso(feito as u64, quais.len() as u64)?;
            saida.push((0..self.linhas).map(|l| if c == 0 { l.to_string() } else { format!("nome {l}") }).collect());
        }
        ao_progresso(quais.len() as u64, quais.len() as u64)?;
        Ok(saida)
    }
}

#[cfg(test)]
mod testes {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn fonte_atravessa_threads_e_le_na_ordem_do_pedido() {
        let fonte: Arc<dyn FonteDeColunas> =
            Arc::new(FonteFalsa { rotulo: "vendas.csv".into(), tipo: TipoDeFonte::Csv, linhas: 3 });
        // `Send + Sync`: a janela lê dentro de uma tarefa
        let lida = Arc::clone(&fonte);
        let valores = std::thread::spawn(move || {
            let mut passos = Vec::new();
            let r = lida.colunas(&[1, 0], &mut |feito, total| {
                passos.push((feito, total));
                Ok(())
            });
            (r, passos)
        })
        .join()
        .expect("a tarefa não entrou em pânico");
        let (colunas, passos) = valores;
        let colunas = colunas.expect("ler as colunas");
        assert_eq!(colunas, [vec!["nome 0", "nome 1", "nome 2"], vec!["0", "1", "2"]]);
        assert_eq!(passos, [(0, 2), (1, 2), (2, 2)]);
        assert_eq!((fonte.rotulo().as_str(), fonte.tipo(), fonte.linhas()), ("vendas.csv", TipoDeFonte::Csv, Some(3)));
    }

    #[test]
    fn o_erro_do_progresso_cancela_e_indice_fora_e_erro() {
        let fonte = FonteFalsa { rotulo: "x".into(), tipo: TipoDeFonte::AreaDeTransferencia, linhas: 2 };
        let cancelada = fonte
            .colunas(&[0, 1], &mut |_, _| Err(comum::ErroBanco::novo("Cancelado.").codigo(comum::Codigo::Cancelado)));
        let erro = cancelada.expect_err("cancelada");
        assert!(erro.tem_codigo(comum::Codigo::Cancelado), "o erro do progresso volta como resultado");
        let fora = fonte.colunas(&[5], &mut |_, _| Ok(()));
        assert!(fora.is_err(), "índice fora do intervalo é erro, não pânico");
        // quem não tem a caixa do cabeçalho diz que não tem
        assert_eq!(fonte.primeira_linha_e_cabecalho(), None);
        assert!(fonte.com_primeira_linha_cabecalho(true).is_none());
        assert_eq!(fonte.colunas(&[], &mut |_, _| Ok(())).expect("nada a ler"), Vec::<Vec<String>>::new());
    }

    #[test]
    fn tipos_na_ordem_da_lista_com_o_nome_em_palavras() {
        let rotulos: Vec<&str> = TipoDeFonte::TODOS.iter().map(|t| t.rotulo()).collect();
        assert_eq!(rotulos, ["CSV", "Planilha", "Tabela", "Consulta", "Área de transferência"]);
        let mut sem_repeticao = TipoDeFonte::TODOS.to_vec();
        sem_repeticao.dedup();
        assert_eq!(sem_repeticao.len(), TipoDeFonte::TODOS.len());
    }
}
