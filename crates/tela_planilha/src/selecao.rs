//! O menu da célula na grade da planilha (0.2.3, `design/planos-0.2.3.md`, A.1 e A.2) e o rodapé da seleção (A.3).
//!
//! - **Copiar, Copiar com cabeçalho, Copiar como:** o intervalo, das linhas que já chegaram (como o Ctrl+C); o
//!   cabeçalho é a letra da coluna, os números vão como números e o `INSERT` leva o nome da planilha.
//! - **Filtrar por este valor / Excluir este valor do filtro:** a planilha ainda não tem a barra de filtros: os dois
//!   aparecem desligados, com o motivo na dica.
//! - **O rodapé da seleção:** as contas do intervalo pelo valor de cada célula (os números do Excel), na hora se todas
//!   as linhas dele já chegaram; senão numa tarefa que lê a planilha em fluxo (parada quando a seleção muda), com o
//!   "Somando…" enquanto ela roda.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use comum::{Codigo, ErroBanco};
use egui::Context;
use estado::Promessa;
use interface::grade::{
    AcaoCelula, Intervalo, OfertasCelula, OpcoesCopia, ResumoSelecao, ValorCopia, frase_do_intervalo_copiado,
    texto_do_intervalo_como, tsv_do_intervalo, tsv_do_intervalo_com_cabecalho,
};
use interface::pecas::toast::toast_concluido;
use planilha::{Celula, Linha, Pasta, Valor, nome_da_coluna};

use crate::fonte::{DECIMAL_PADRAO, numero_cru, texto_para_levar};
use crate::grade::Grade;
use crate::sessao::SessaoDaFolha;

/// O motivo do "Filtrar por este valor" desligado: a planilha não tem a barra de filtros.
pub const MSG_SEM_FILTRO: &str = "A planilha ainda não tem a barra de filtros.";
/// O que a tarefa do rodapé devolve quando a seleção mudou no meio.
const CANCELADO: &str = "Cancelado.";

/// O que o menu oferece na planilha: os itens de copiar e o filtro desligado.
pub(crate) fn ofertas(iv: Intervalo) -> OfertasCelula {
    OfertasCelula::de_leitura(iv)
        .desligar(AcaoCelula::FiltrarPorValor, MSG_SEM_FILTRO)
        .desligar(AcaoCelula::ExcluirValorDoFiltro, MSG_SEM_FILTRO)
}

/// A célula de uma linha que já chegou, na coluna `c` da grade.
fn celula<'a>(g: &Grade, s: &'a SessaoDaFolha, l: usize, c: usize) -> Option<&'a Celula> {
    s.linha(l as u32 + 1)?.celula(g.coluna_do_arquivo(c)?)
}

/// A célula para os formatos do "Copiar como": o número do Excel como número (o literal, com ponto), o vazio como
/// NULL e o resto como o texto que vai para fora.
fn valor_de_copia(pasta: &Pasta, celula: Option<&Celula>) -> ValorCopia<'static> {
    match celula.map(|c| &c.valor) {
        None | Some(Valor::Vazio) => ValorCopia::Nulo,
        Some(Valor::Numero(n)) if n.is_finite() => ValorCopia::numero(numero_cru(*n, '.')),
        Some(_) => ValorCopia::texto(celula.map(|c| texto_para_levar(pasta, c, DECIMAL_PADRAO)).unwrap_or_default()),
    }
}

/// Copia o intervalo como a `acao` pede (das linhas que já chegaram) e avisa no toast.
pub(crate) fn copiar(egui: &Context, acao: AcaoCelula, iv: Intervalo, g: &Grade, s: &SessaoDaFolha) {
    let texto = |l: usize, c: usize| Some(texto_para_levar(s.pasta(), celula(g, s, l, c)?, DECIMAL_PADRAO));
    let nome = |c: usize| g.coluna_do_arquivo(c).map(nome_da_coluna).unwrap_or_default();
    let saida = match acao {
        AcaoCelula::Copiar if iv.varias() => tsv_do_intervalo(iv, texto),
        AcaoCelula::Copiar => texto(iv.linha0, iv.coluna0).unwrap_or_default(),
        AcaoCelula::CopiarComCabecalho => tsv_do_intervalo_com_cabecalho(iv, nome, texto),
        AcaoCelula::CopiarComo(formato) => {
            let opcoes = OpcoesCopia { separador: ',', tabela: &s.nome };
            let valor = |l: usize, c: usize| valor_de_copia(s.pasta(), celula(g, s, l, c));
            texto_do_intervalo_como(formato, iv, nome, valor, &opcoes)
        }
        _ => return,
    };
    egui.copy_text(saida);
    toast_concluido(egui, frase_do_intervalo_copiado(iv));
}

/// As casas decimais de um número do Excel (as do texto mais curto dele).
fn casas(n: f64) -> u8 {
    let t = numero_cru(n, '.');
    t.split_once('.').map_or(0, |(_, f)| u8::try_from(f.len()).unwrap_or(u8::MAX))
}

/// Soma as células das `colunas` (do arquivo) de uma linha: o número do Excel entra nas contas; o vazio não conta.
fn somar_linha(r: &mut ResumoSelecao, pasta: &Pasta, linha: &Linha, colunas: &[u32]) {
    for &c in colunas {
        let Some(cel) = linha.celula(c) else { continue };
        match &cel.valor {
            Valor::Vazio => {}
            Valor::Numero(n) => r.somar_numero(*n, casas(*n)),
            _ => r.somar_texto(&texto_para_levar(pasta, cel, DECIMAL_PADRAO)),
        }
    }
}

/// As contas do rodapé da seleção.
enum EstadoSoma {
    Pronta(ResumoSelecao),
    Somando { promessa: Promessa<ResumoSelecao>, parar: Arc<AtomicBool> },
    Falhou,
}

/// As contas da seleção e para que intervalo de que planilha elas valem.
pub(crate) struct SomaSelecao {
    chave: (Intervalo, usize),
    estado: EstadoSoma,
}

impl Drop for SomaSelecao {
    fn drop(&mut self) {
        if let EstadoSoma::Somando { parar, .. } = &self.estado {
            parar.store(true, Ordering::Relaxed);
        }
    }
}

/// Começa as contas do intervalo: na hora, se todas as linhas dele já chegaram; senão numa tarefa que lê a planilha
/// em fluxo (só as linhas e as colunas do intervalo).
fn somar(egui: &Context, iv: Intervalo, g: &Grade, s: &SessaoDaFolha) -> EstadoSoma {
    let colunas: Vec<u32> = iv.colunas().filter_map(|c| g.coluna_do_arquivo(c)).collect();
    let (primeira, ultima) = (iv.linha0 as u32 + 1, iv.linha1 as u32 + 1);
    if (primeira..=ultima).all(|l| s.linha(l).is_some()) {
        let mut r = ResumoSelecao::default();
        for l in primeira..=ultima {
            if let Some(linha) = s.linha(l) {
                somar_linha(&mut r, s.pasta(), linha, &colunas);
            }
        }
        return EstadoSoma::Pronta(r);
    }
    let (pasta, folha) = (s.pasta().clone(), s.folha);
    let parar = Arc::new(AtomicBool::new(false));
    let na_tarefa = Arc::clone(&parar);
    let promessa = Promessa::rodar(egui, move || {
        let mut r = ResumoSelecao::default();
        for linha in pasta.linhas(folha)? {
            if na_tarefa.load(Ordering::Relaxed) {
                return Err(ErroBanco::novo(CANCELADO).codigo(Codigo::Cancelado));
            }
            let linha = linha?;
            if linha.numero > ultima {
                break;
            }
            if linha.numero >= primeira {
                somar_linha(&mut r, &pasta, &linha, &colunas);
            }
        }
        Ok(r)
    });
    EstadoSoma::Somando { promessa, parar }
}

/// As contas do rodapé da seleção, com mais de uma célula selecionada (`Some(None)`: somando). A soma guardada em
/// `soma` vale para o intervalo e a planilha; outra seleção para a conta da anterior.
pub(crate) fn resumo(
    soma: &mut Option<SomaSelecao>,
    egui: &Context,
    g: &Grade,
    s: &SessaoDaFolha,
) -> Option<Option<ResumoSelecao>> {
    let Some(iv) = g.nucleo.intervalo().filter(Intervalo::varias) else {
        *soma = None;
        return None;
    };
    let chave = (iv, s.folha);
    if soma.as_ref().is_none_or(|x| x.chave != chave) {
        *soma = Some(SomaSelecao { chave, estado: somar(egui, iv, g, s) });
    }
    let x = soma.as_mut()?;
    if let EstadoSoma::Somando { promessa, .. } = &mut x.estado
        && let Some(r) = promessa.tomar()
    {
        // a que falhou (o arquivo sumiu no meio) deixa o rodapé de sempre
        x.estado = r.map_or(EstadoSoma::Falhou, EstadoSoma::Pronta);
    }
    match &x.estado {
        EstadoSoma::Pronta(r) => Some(Some(*r)),
        EstadoSoma::Somando { .. } => Some(None),
        EstadoSoma::Falhou => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use interface::grade::OfertaCelula;

    #[test]
    fn filtro_desligado_na_planilha() {
        let o = ofertas(Intervalo::entre((0, 1), (2, 1)));
        assert!(o.ligado(AcaoCelula::Copiar));
        assert_eq!(*o.get(AcaoCelula::FiltrarPorValor), OfertaCelula::Desligada(MSG_SEM_FILTRO.into()));
        assert_eq!(*o.get(AcaoCelula::MarcarLinhas), OfertaCelula::Oculta, "a planilha não marca linhas");
        assert_eq!(*o.get(AcaoCelula::Colar), OfertaCelula::Oculta, "nem grava");
    }

    #[test]
    fn casas_do_numero_do_excel() {
        assert_eq!(casas(17051.0), 0);
        assert_eq!(casas(154.03), 2);
    }
}
