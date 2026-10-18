//! O que o bloco de progresso mostra a partir do andamento: a fração da barra (None = indeterminada), a linha da tabela atual (banco inteiro) e o texto.

use interface::formato::{formatar_numero, plural};
use servico::{Fase, ProgressoInfo};

/// Os textos e a barra prontos (montados a cada leitura do andamento, não a cada quadro).
#[derive(Debug, Clone, PartialEq)]
pub struct VistaProgresso {
    /// `Some(0.0..=1.0)` com total; `None` para a barra indeterminada.
    pub fracao: Option<f32>,
    /// "Tabela 3 de 7 · pedidos" (só no banco inteiro).
    pub tabela: Option<String>,
    /// "12.340 de 1.000.000 linhas · 1%", "12.340 linhas", "Contando as linhas…"…
    pub texto: String,
}

impl Default for VistaProgresso {
    /// Antes da primeira leitura: "Preparando a exportação…".
    fn default() -> Self {
        vista_progresso(None, false)
    }
}

/// A vista do andamento `p` (None = ainda sem leitura). `banco`: a exportação do banco inteiro.
pub fn vista_progresso(p: Option<&ProgressoInfo>, banco: bool) -> VistaProgresso {
    let fase = p.map_or(Fase::Preparando, |p| p.fase);
    let linhas = p.map_or(0, |p| p.linhas);
    let total = p.and_then(|p| p.total);
    let tabela = p
        .filter(|p| banco && p.tabelas_total > 0 && fase != Fase::Finalizando)
        .and_then(|p| p.tabela_atual.as_deref().map(|nome| (p, nome)))
        .map(|(p, nome)| {
            let atual = p.tabelas_total.min(p.tabelas_feitas + 1);
            format!("Tabela {} de {} · {nome}", formatar_numero(atual), formatar_numero(p.tabelas_total))
        });
    let (fracao, texto) = match (fase, total) {
        (Fase::Preparando, _) => (None, "Preparando a exportação…".to_string()),
        (Fase::Contando, _) => (None, "Contando as linhas…".to_string()),
        // as linhas acabaram; no Excel, montar o arquivo ainda leva um tempo (minutos numa tabela grande)
        (Fase::Finalizando, _) => (None, format!("{} · fechando o arquivo…", plural(linhas, "linha", "linhas"))),
        (_, None | Some(0)) => (None, plural(linhas, "linha", "linhas")),
        (_, Some(total)) => {
            let fracao = (linhas as f64 / total as f64).clamp(0.0, 1.0);
            let pct = (fracao * 100.0).floor() as u64;
            (
                Some(fracao as f32),
                format!("{} de {} · {pct}%", formatar_numero(linhas), plural(total, "linha", "linhas")),
            )
        }
    };
    VistaProgresso { fracao, tabela, texto }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn info(fase: Fase, linhas: u64, total: Option<u64>) -> ProgressoInfo {
        ProgressoInfo { fase, linhas, total, ..ProgressoInfo::default() }
    }

    #[test]
    fn textos_de_cada_fase() {
        assert_eq!(vista_progresso(None, false).texto, "Preparando a exportação…");
        assert_eq!(vista_progresso(Some(&info(Fase::Preparando, 0, None)), false).texto, "Preparando a exportação…");
        assert_eq!(vista_progresso(Some(&info(Fase::Contando, 0, None)), false).texto, "Contando as linhas…");
        let v = vista_progresso(Some(&info(Fase::Gravando, 12_340, Some(1_000_000))), false);
        assert_eq!(v.texto, "12.340 de 1.000.000 linhas · 1%");
        assert!((v.fracao.unwrap_or(0.0) - 0.01234).abs() < 1e-6);
        let v = vista_progresso(Some(&info(Fase::Gravando, 12_340, None)), false);
        assert_eq!((v.texto.as_str(), v.fracao), ("12.340 linhas", None));
        let v = vista_progresso(Some(&info(Fase::Finalizando, 1_000_000, Some(1_000_000))), false);
        assert_eq!((v.texto.as_str(), v.fracao), ("1.000.000 linhas · fechando o arquivo…", None));
        assert_eq!(vista_progresso(Some(&info(Fase::Gravando, 1, Some(1))), false).texto, "1 de 1 linha · 100%");
        // o total zero (tabela vazia) não divide por zero
        assert_eq!(vista_progresso(Some(&info(Fase::Gravando, 0, Some(0))), false).texto, "0 linhas");
    }

    #[test]
    fn tabela_atual_no_banco_inteiro() {
        let p = ProgressoInfo {
            fase: Fase::Gravando,
            linhas: 500,
            total: Some(5772),
            tabela_atual: Some("pedidos".into()),
            tabelas_feitas: 2,
            tabelas_total: 7,
        };
        assert_eq!(vista_progresso(Some(&p), true).tabela.as_deref(), Some("Tabela 3 de 7 · pedidos"));
        assert_eq!(vista_progresso(Some(&p), false).tabela, None, "só no banco inteiro");
        let fim = ProgressoInfo { tabelas_feitas: 7, ..p.clone() };
        assert_eq!(vista_progresso(Some(&fim), true).tabela.as_deref(), Some("Tabela 7 de 7 · pedidos"));
        let fechando = ProgressoInfo { fase: Fase::Finalizando, ..p };
        assert_eq!(vista_progresso(Some(&fechando), true).tabela, None);
    }
}
