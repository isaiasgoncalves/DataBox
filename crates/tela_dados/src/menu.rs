//! O menu da célula na grade dos Dados (0.2.3, `design/planos-0.2.3.md`, A.1 e A.2): o que ele oferece, a cópia nos
//! formatos (o Copiar, o Copiar com cabeçalho e o Copiar como, com a célula gravando agora no texto digitado) e a
//! condição do "Filtrar por este valor" e do "Excluir este valor do filtro"; e as contas do rodapé da seleção (A.3).

use banco::Operador;
use comum::Celula;
use interface::grade::{
    AcaoCelula, Intervalo, OfertasCelula, OpcoesCopia, ResumoSelecao, ValorCopia, frase_do_intervalo_copiado,
    texto_celula, texto_do_intervalo_como, tsv_do_intervalo, tsv_do_intervalo_com_cabecalho, valor_de_copia,
};
use interface::pecas::toast::toast_concluido;

use crate::grade::Pendente;
use crate::modelo::Dados;

/// O motivo do filtro desligado num BLOB.
pub const MOTIVO_FILTRO_BLOB: &str = "Um valor binário não vira filtro.";
/// O motivo do filtro desligado num texto cortado (a página só traz o começo dele).
pub const MOTIVO_FILTRO_CORTADO: &str = "O texto é longo demais para virar filtro.";

/// O que o menu oferece nos Dados: os itens de copiar; os de escrita desligados até a próxima etapa (numa grade que
/// grava); o filtro pela célula ativa (desligado num BLOB e num texto cortado); marcar as linhas (com as caixas de
/// marcar) e a ficha da linha.
pub(crate) fn ofertas(iv: Intervalo, ativa: Option<&Celula>, editavel: bool, tem_marca: bool) -> OfertasCelula {
    let mut o = OfertasCelula::de_leitura(iv);
    if editavel {
        o = o.com_escrita_por_vir(&[
            AcaoCelula::Colar,
            AcaoCelula::Limpar,
            AcaoCelula::DefinirNulo,
            AcaoCelula::ExcluirLinhas,
        ]);
    }
    for acao in [AcaoCelula::FiltrarPorValor, AcaoCelula::ExcluirValorDoFiltro] {
        o = match ativa {
            Some(Celula::Blob { .. }) => o.desligar(acao, MOTIVO_FILTRO_BLOB),
            Some(Celula::TextoCortado { .. }) => o.desligar(acao, MOTIVO_FILTRO_CORTADO),
            _ => o.ligar(acao),
        };
    }
    if tem_marca {
        o = o.ligar(AcaoCelula::MarcarLinhas);
    }
    o.ligar(AcaoCelula::AbrirFicha)
}

/// A condição do "Filtrar por este valor" (`excluir` false) ou do "Excluir este valor do filtro": "igual a" e
/// "diferente de" pelo texto da célula; no NULL, "é nulo" e "não é nulo"; no texto vazio, "vazio" e "não vazio". None
/// num BLOB e num texto cortado.
pub(crate) fn condicao_do_valor(celula: &Celula, excluir: bool) -> Option<(Operador, String)> {
    let par = |sim, nao| if excluir { nao } else { sim };
    match celula {
        Celula::Blob { .. } | Celula::TextoCortado { .. } => None,
        Celula::Nulo => Some((par(Operador::Nulo, Operador::NaoNulo), String::new())),
        Celula::Texto(t) if t.is_empty() => Some((par(Operador::Vazio, Operador::NaoVazio), String::new())),
        _ => Some((par(Operador::Igual, Operador::Diferente), texto_celula(celula))),
    }
}

/// As células da página para copiar: a que está gravando agora vai com o texto digitado.
pub(crate) struct FonteCopia<'a> {
    pub dados: &'a Dados,
    pub pendentes: &'a [Pendente],
    /// O índice, no núcleo, da primeira coluna de dados.
    pub primeira: usize,
    /// A tabela do `INSERT`.
    pub tabela: &'a str,
}

impl FonteCopia<'_> {
    fn pendente(&self, l: usize, c: usize) -> Option<&str> {
        let p = self.pendentes.iter().rev().find(|g| g.geracao == self.dados.geracao && g.linha == l && g.coluna == c);
        p.map(|g| g.texto.as_str())
    }

    /// O texto de uma célula (coluna do núcleo), como o Ctrl+C leva.
    fn texto(&self, l: usize, c: usize) -> Option<String> {
        let c = c.checked_sub(self.primeira)?;
        self.pendente(l, c).map(str::to_string).or_else(|| self.dados.celula(l, c).map(texto_celula))
    }

    /// A célula para os formatos do "Copiar como" (a que grava agora: número, se a coluna é numérica e o texto é um).
    fn valor(&self, l: usize, c: usize) -> ValorCopia<'static> {
        let Some(c) = c.checked_sub(self.primeira) else { return ValorCopia::Nulo };
        if let Some(t) = self.pendente(l, c) {
            let numerica = self.dados.colunas.get(c).is_some_and(|col| col.numerica);
            return match t.trim().parse::<f64>() {
                Ok(n) if numerica && n.is_finite() => ValorCopia::numero(t.trim().to_string()),
                _ => ValorCopia::texto(t.to_string()),
            };
        }
        self.dados.celula(l, c).map_or(ValorCopia::Nulo, valor_de_copia)
    }

    fn nome(&self, c: usize) -> String {
        c.checked_sub(self.primeira)
            .and_then(|c| self.dados.colunas.get(c))
            .map(|col| col.nome.clone())
            .unwrap_or_default()
    }

    /// O texto que a ação de copiar leva do intervalo (o Copiar de uma célula só: o texto dela, sem o TSV).
    pub(crate) fn texto_da_acao(&self, acao: AcaoCelula, iv: Intervalo) -> Option<String> {
        Some(match acao {
            AcaoCelula::Copiar if iv.varias() => tsv_do_intervalo(iv, |l, c| self.texto(l, c)),
            AcaoCelula::Copiar => self.texto(iv.linha0, iv.coluna0).unwrap_or_default(),
            AcaoCelula::CopiarComCabecalho => {
                tsv_do_intervalo_com_cabecalho(iv, |c| self.nome(c), |l, c| self.texto(l, c))
            }
            AcaoCelula::CopiarComo(formato) => {
                let opcoes = OpcoesCopia { separador: ',', tabela: self.tabela };
                texto_do_intervalo_como(formato, iv, |c| self.nome(c), |l, c| self.valor(l, c), &opcoes)
            }
            _ => return None,
        })
    }

    /// Copia o intervalo como a ação pede e avisa no toast ("3 linhas × 2 colunas copiadas.").
    pub(crate) fn copiar(&self, ctx: &egui::Context, acao: AcaoCelula, iv: Intervalo) {
        if let Some(texto) = self.texto_da_acao(acao, iv) {
            ctx.copy_text(texto);
            toast_concluido(ctx, frase_do_intervalo_copiado(iv));
        }
    }

    /// As contas do rodapé da seleção: o tipo da coluna decide o que é número.
    pub(crate) fn resumo(&self, iv: Intervalo) -> ResumoSelecao {
        let mut r = ResumoSelecao::default();
        for l in iv.linhas() {
            for c in iv.colunas() {
                let Some(dc) = c.checked_sub(self.primeira) else { continue };
                let numerica = self.dados.colunas.get(dc).is_some_and(|col| col.numerica);
                match self.pendente(l, dc) {
                    Some(t) if numerica => match t.trim().parse::<f64>() {
                        Ok(n) => r.somar_numero(n, casas(t)),
                        Err(_) => r.somar_texto(t),
                    },
                    Some(t) => r.somar_texto(t),
                    None => {
                        if let Some(celula) = self.dados.celula(l, dc) {
                            r.somar_celula(celula, numerica);
                        }
                    }
                }
            }
        }
        r
    }
}

/// As casas decimais de um número digitado ("12.50" → 2).
fn casas(texto: &str) -> u8 {
    texto.trim().split_once('.').map_or(0, |(_, f)| u8::try_from(f.len()).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod testes {
    use super::*;
    use interface::grade::{MOTIVO_PROXIMA_ETAPA, OfertaCelula};

    #[test]
    fn condicoes_pelo_valor() {
        assert_eq!(condicao_do_valor(&Celula::Nulo, false), Some((Operador::Nulo, String::new())));
        assert_eq!(condicao_do_valor(&Celula::Nulo, true), Some((Operador::NaoNulo, String::new())));
        assert_eq!(condicao_do_valor(&Celula::Texto(String::new()), true), Some((Operador::NaoVazio, String::new())));
        assert_eq!(condicao_do_valor(&Celula::Real(17051.0), false), Some((Operador::Igual, "17051".into())));
        assert_eq!(condicao_do_valor(&Celula::Texto("Ana".into()), true), Some((Operador::Diferente, "Ana".into())));
        assert_eq!(condicao_do_valor(&Celula::Blob { tamanho: 1, inicio: vec![0] }, false), None);
    }

    #[test]
    fn ofertas_dos_dados() {
        let iv = Intervalo::entre((0, 2), (1, 3));
        let o = ofertas(iv, Some(&Celula::Inteiro(1)), true, true);
        assert!(o.ligado(AcaoCelula::FiltrarPorValor) && o.ligado(AcaoCelula::MarcarLinhas));
        assert!(o.ligado(AcaoCelula::AbrirFicha));
        assert_eq!(*o.get(AcaoCelula::Colar), OfertaCelula::Desligada(MOTIVO_PROXIMA_ETAPA.into()));
        let visao = ofertas(iv, Some(&Celula::Blob { tamanho: 1, inicio: vec![0] }), false, false);
        assert_eq!(*visao.get(AcaoCelula::Colar), OfertaCelula::Oculta, "a visão não grava");
        assert_eq!(*visao.get(AcaoCelula::MarcarLinhas), OfertaCelula::Oculta);
        assert_eq!(*visao.get(AcaoCelula::FiltrarPorValor), OfertaCelula::Desligada(MOTIVO_FILTRO_BLOB.into()));
    }
}
