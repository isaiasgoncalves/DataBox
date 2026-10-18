//! As planilhas da pasta como a faixa as mostra (decisão 5 da etapa 6) e o cache das que saíram da tela.
//!
//! - Na faixa entram só as abas com células (`TipoFolha::Planilha`); gráficos, diálogos e macros ficam fora e a seção
//!   "Pasta de trabalho" os conta ("1 gráfico (não aparece)").
//! - As ocultas (e as muito ocultas) só entram com a caixa "Mostrar planilhas ocultas", na ordem do arquivo.
//! - Trocar de planilha guarda a sessão que sai num cache curto: no máximo [`EM_MEMORIA`] planilhas abertas ao mesmo
//!   tempo, contando a da tela; a que sai do cache é solta (a `Planilha` dela fecha o fluxo numa thread à parte).

use interface::formato::formatar_numero;
use interface::pecas::folhas::EstadoDaFolha;
use planilha::{Estado, Pasta, TipoFolha};

use crate::grade::Grade;
use crate::sessao::SessaoDaFolha;

/// Uma planilha aberta: a leitura e a grade (as larguras que a pessoa arrastou, a célula ativa e a rolagem ficam nela).
pub struct AbaDaFolha {
    pub sessao: SessaoDaFolha,
    pub grade: Grade,
}

/// Quantas planilhas ficam abertas ao mesmo tempo (a da tela e as duas últimas que saíram dela).
pub const EM_MEMORIA: usize = 3;

/// Uma planilha da faixa: o índice dela na pasta, o nome e o estado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolhaDaFaixa {
    pub folha: usize,
    pub nome: String,
    pub estado: EstadoDaFolha,
}

/// O estado do núcleo como a peça da faixa o entende.
pub fn estado_da_faixa(estado: Estado) -> EstadoDaFolha {
    match estado {
        Estado::Visivel => EstadoDaFolha::Visivel,
        Estado::Oculta => EstadoDaFolha::Oculta,
        Estado::MuitoOculta => EstadoDaFolha::MuitoOculta,
    }
}

/// As planilhas que a faixa mostra: as visíveis e, com `ocultas`, também as ocultas e as muito ocultas, na ordem do
/// arquivo; nunca gráficos, diálogos nem macros.
pub fn folhas_da_faixa(pasta: &Pasta, ocultas: bool) -> Vec<FolhaDaFaixa> {
    pasta
        .folhas()
        .iter()
        .enumerate()
        .filter(|(_, f)| f.tipo == TipoFolha::Planilha && (ocultas || f.estado == Estado::Visivel))
        .map(|(i, f)| FolhaDaFaixa { folha: i, nome: f.nome.clone(), estado: estado_da_faixa(f.estado) })
        .collect()
}

/// As contas da seção "Pasta de trabalho".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contagem {
    /// Abas com células.
    pub planilhas: usize,
    /// Dessas, as ocultas (as muito ocultas também).
    pub ocultas: usize,
    pub graficos: usize,
    /// Diálogos e macros do Excel 5 e 4.
    pub outras: usize,
}

impl Contagem {
    pub fn de(pasta: &Pasta) -> Contagem {
        let mut c = Contagem::default();
        for f in pasta.folhas() {
            match f.tipo {
                TipoFolha::Planilha => {
                    c.planilhas += 1;
                    if f.estado != Estado::Visivel {
                        c.ocultas += 1;
                    }
                }
                TipoFolha::Grafico => c.graficos += 1,
                TipoFolha::Dialogo | TipoFolha::Macro => c.outras += 1,
            }
        }
        c
    }

    /// "4 · 1 oculta", "3 · 3 ocultas", "2".
    pub fn texto_planilhas(&self) -> String {
        let n = formatar_numero(self.planilhas);
        match self.ocultas {
            0 => n,
            1 => format!("{n} · 1 oculta"),
            k => format!("{n} · {} ocultas", formatar_numero(k)),
        }
    }

    /// "1 gráfico (não aparece)", "2 gráficos (não aparecem)", "1 gráfico e 1 outra (não aparecem)"; `None` sem nenhuma.
    pub fn texto_outras(&self) -> Option<String> {
        let mut partes = Vec::new();
        if self.graficos > 0 {
            let nome = if self.graficos == 1 { "gráfico" } else { "gráficos" };
            partes.push(format!("{} {nome}", formatar_numero(self.graficos)));
        }
        if self.outras > 0 {
            let nome = if self.outras == 1 { "outra" } else { "outras" };
            partes.push(format!("{} {nome}", formatar_numero(self.outras)));
        }
        if partes.is_empty() {
            return None;
        }
        let verbo = if self.graficos + self.outras == 1 { "não aparece" } else { "não aparecem" };
        Some(format!("{} ({verbo})", partes.join(" e ")))
    }
}

/// As sessões das planilhas que saíram da tela, da mais recente à mais antiga.
#[derive(Default)]
pub struct Cache {
    sessoes: Vec<AbaDaFolha>,
}

impl Cache {
    /// Tira do cache a sessão da aba `folha`, se ela está lá.
    pub fn tomar(&mut self, folha: usize) -> Option<AbaDaFolha> {
        let i = self.sessoes.iter().position(|a| a.sessao.folha == folha)?;
        Some(self.sessoes.remove(i))
    }

    /// Guarda a sessão que saiu da tela (liberada: sem índice rodando, sem fluxo aberto); as mais antigas saem para caber
    /// [`EM_MEMORIA`] contando a da tela.
    pub fn guardar(&mut self, mut aba: AbaDaFolha) {
        aba.sessao.liberar();
        self.sessoes.insert(0, aba);
        self.sessoes.truncate(EM_MEMORIA - 1);
    }

    /// As abas guardadas, da mais recente à mais antiga.
    pub fn folhas(&self) -> Vec<usize> {
        self.sessoes.iter().map(|a| a.sessao.folha).collect()
    }

    /// Alguma sessão guardada tem tarefa em voo (uma página que volta).
    pub fn ocupado(&self) -> bool {
        self.sessoes.iter().any(|a| a.sessao.pedindo())
    }

    /// As respostas que chegam para as guardadas (uma página em voo quando ela saiu da tela).
    pub fn passo(&mut self, egui: &egui::Context) {
        for a in &mut self.sessoes {
            let _ = a.sessao.passo(egui);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contagem_em_palavras() {
        let c = Contagem { planilhas: 4, ocultas: 1, graficos: 1, outras: 0 };
        assert_eq!(c.texto_planilhas(), "4 · 1 oculta");
        assert_eq!(c.texto_outras().as_deref(), Some("1 gráfico (não aparece)"));
        let c = Contagem { planilhas: 3, ocultas: 3, graficos: 2, outras: 0 };
        assert_eq!(c.texto_planilhas(), "3 · 3 ocultas");
        assert_eq!(c.texto_outras().as_deref(), Some("2 gráficos (não aparecem)"));
        let c = Contagem { planilhas: 2, ocultas: 0, graficos: 0, outras: 0 };
        assert_eq!((c.texto_planilhas().as_str(), c.texto_outras()), ("2", None));
        let c = Contagem { planilhas: 1, ocultas: 0, graficos: 1, outras: 1 };
        assert_eq!(c.texto_outras().as_deref(), Some("1 gráfico e 1 outra (não aparecem)"));
    }
}
