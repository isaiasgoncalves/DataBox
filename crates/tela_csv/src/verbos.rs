//! Editar o CSV (passo 11): a edição no lugar, as linhas marcadas, os verbos da barra (Salvar, Nova linha,
//! Duplicar, Excluir, Desfazer, Refazer, Atualizar) com as dicas que dizem o alvo, o desfazer com rótulos, o Salvar
//! (com o aviso "Salvando…" e a pergunta do arquivo que mudou no disco) e o Ctrl+C.

use comum::{Codigo, ErroBanco};
use egui::Ui;
use estado::{Estado as EstadoApp, Oferta, Ofertas, Verbo};
use interface::grade::{Intervalo, RespostaNucleo, frase_do_intervalo_copiado, tsv_do_intervalo};
use interface::pecas::avisos::TomAviso;
use interface::pecas::botao::Botao;
use interface::pecas::progresso::barra_progresso;
use interface::pecas::texto::texto as texto_simples;
use interface::pecas::toast::{toast, toast_atencao, toast_concluido, toast_erro};
use tabular::colunas::escrever_tsv;

use crate::colunas::{ColunaDaGrade, FimCabecalho};
use crate::edicao::{Mutacao, ajustar_marcas, alvo_em_palavras};
use crate::grade::PRIMEIRA_DE_DADOS;
use crate::pintor::{Edicao, FimEdicao, Pendente, preparar_edicao};
use crate::sessao::{Efeito, LIMITE_DA_COPIA, Trabalho};
use crate::tela::{ESPERA_DO_CARD, TelaCsv};
use crate::textos;

/// A largura da barra de progresso do aviso "Salvando…".
const LARGURA_BARRA_SALVAR: f32 = 200.0;
/// O erro (e o toast) do Cancelar do aviso "Salvando…".
pub const MSG_GRAVACAO_CANCELADA: &str = "A gravação foi cancelada; o arquivo ficou como estava.";

/// O que a grade devolveu num quadro, além da resposta do núcleo.
pub(crate) struct SaidaDaGrade {
    pub visiveis: Option<(u64, u64)>,
    pub nova_no_fim: bool,
    pub marcar: Option<(usize, bool)>,
    pub marcar_todas: Option<bool>,
    pub fim_edicao: Option<FimEdicao>,
    pub edicao_desenhada: bool,
    pub clique_th: Option<(usize, bool, bool)>,
    pub menu_th: Option<usize>,
    pub fim_cabecalho: Option<FimCabecalho>,
    pub cabecalho_desenhado: bool,
    /// A seta da ordem de uma coluna (da tabela) foi clicada.
    pub ordenar: Option<usize>,
}

impl TelaCsv {
    /// Uma gravação está na fila ou rodando: a edição fica de fora.
    pub(crate) fn salvando_agora(&self) -> bool {
        self.rodando.as_ref().is_some_and(|r| matches!(r.trabalho, Trabalho::Salvar))
            || self.fila.iter().any(|(_, t)| matches!(t, Trabalho::Salvar))
    }

    /// A posição na tabela da linha da célula ativa.
    pub(crate) fn posicao_ativa(&self) -> Option<u64> {
        let (l, _) = self.grade.as_ref()?.nucleo.selecao()?;
        self.pagina.as_ref()?.posicao(l as u64)
    }

    /// Põe uma alteração na fila. Com uma célula em edição, ela grava antes (a posição dela é a de agora: depois de uma
    /// linha inserida ou excluída, ela cairia na linha errada).
    pub(crate) fn mutar(&mut self, m: Mutacao) -> u64 {
        if !matches!(m, Mutacao::Celula { .. })
            && let Some(ed) = self.edicao.take()
        {
            self.gravar_celula(ed);
        }
        self.enfileirar(Trabalho::Mutacao(m))
    }

    /// Grava o texto da célula que estava em edição (se mudou), com ele na célula até a resposta.
    pub(crate) fn gravar_celula(&mut self, ed: Edicao) {
        if ed.texto != ed.original {
            let m = Mutacao::Celula { linha: ed.posicao, coluna: ed.coluna, texto: ed.texto.clone() };
            let seq = self.enfileirar(Trabalho::Mutacao(m));
            self.pendentes.push(Pendente { seq, linha: ed.linha, coluna: ed.coluna, texto: ed.texto });
        }
    }

    /// As linhas que Duplicar e Excluir alcançam: as marcadas ou, sem marcas, a da célula ativa.
    fn alvo_das_linhas(&self) -> Vec<u64> {
        if self.marcadas.is_empty() {
            self.posicao_ativa().into_iter().collect()
        } else {
            self.marcadas.iter().copied().collect()
        }
    }

    // ------------------------------------------------------------------ o que a tarefa respondeu

    pub(crate) fn tarefa_falhou(&mut self, ectx: &egui::Context, trabalho: &Trabalho, e: &ErroBanco, mostrar: bool) {
        let cancelado = e.tem_codigo(Codigo::Cancelado);
        if let Trabalho::Salvar = trabalho {
            if cancelado {
                self.erro_de_gravacao = Some(MSG_GRAVACAO_CANCELADA.to_string());
                toast_atencao(ectx, MSG_GRAVACAO_CANCELADA);
            } else {
                self.erro_de_gravacao = Some(e.mensagem.clone());
                self.gravacao_falhou = true;
                toast_erro(ectx, e.mensagem.clone());
            }
            return;
        }
        if mostrar && !cancelado {
            toast_erro(ectx, e.mensagem.clone());
        }
    }

    /// Depois de uma tarefa que deu certo (o retrato e a página já trocados): as marcas, os rótulos do desfazer, os
    /// toasts e a área de transferência.
    pub(crate) fn depois_da_tarefa(&mut self, ectx: &egui::Context, trabalho: &Trabalho, efeito: Efeito) {
        match (trabalho, efeito) {
            (Trabalho::Reabrir(_) | Trabalho::Cabecalho(_) | Trabalho::Recarregar(_), _) => {
                self.marcadas.clear();
                self.sem_par.clear();
                self.rotulos_desfazer.clear();
                self.rotulos_refazer.clear();
                self.edicao = None;
                self.gravacao_falhou = false;
            }
            (Trabalho::Mutacao(m), Efeito::Alterou(true)) => {
                self.marcadas = ajustar_marcas(&self.marcadas, m);
                // as linhas sem par do último Trazer andam como as marcas (o desfazer as esquece; um Trazer novo as
                // troca logo abaixo)
                if !self.sem_par.is_empty() {
                    let sem_par: std::collections::BTreeSet<u64> = self.sem_par.iter().copied().collect();
                    self.sem_par = ajustar_marcas(&sem_par, m).into_iter().collect();
                }
                match m {
                    Mutacao::Desfazer => {
                        if let Some(r) = self.rotulos_desfazer.pop() {
                            self.rotulos_refazer.push(r);
                        }
                    }
                    Mutacao::Refazer => {
                        if let Some(r) = self.rotulos_refazer.pop() {
                            self.rotulos_desfazer.push(r);
                        }
                    }
                    outra => {
                        self.rotulos_desfazer.extend(outra.rotulo());
                        self.rotulos_refazer.clear();
                    }
                }
                match m {
                    Mutacao::NovaLinha { posicao } if self.janela.vista.is_none() => self.ir_para_linha(*posicao),
                    Mutacao::ColarColunas { .. } | Mutacao::ColarSobre { .. } => self.toast_da_cola(ectx, m),
                    Mutacao::Trazer(plano) => self.depois_do_trazer(ectx, plano),
                    _ => {}
                }
                if matches!(m, Mutacao::ExcluirColunas { .. } | Mutacao::Desfazer | Mutacao::Refazer) {
                    self.colunas_selecionadas.clear();
                }
            }
            // a cola por cima e o Trazer com os valores que já estavam lá: nada entrou no desfazer, e a tela diz
            (Trabalho::Mutacao(Mutacao::ColarSobre { .. } | Mutacao::Trazer(_)), Efeito::Alterou(false)) => {
                toast(ectx, textos::NADA_MUDOU);
            }
            (Trabalho::Salvar, Efeito::Salvo(_)) => {
                self.rotulos_desfazer.clear();
                self.rotulos_refazer.clear();
                self.gravacao_falhou = false;
                self.erro_de_gravacao = None;
                toast_concluido(ectx, textos::salvo(&self.nome));
            }
            (Trabalho::Salvar, Efeito::MudouNoDisco) => {
                self.erro_de_gravacao = Some(textos::MUDOU_AO_SALVAR.to_string());
                self.gravacao_falhou = true;
                self.mudou_no_disco = true;
                toast_erro(ectx, textos::MUDOU_AO_SALVAR);
            }
            (Trabalho::LerIntervalo { .. }, Efeito::Intervalo(linhas)) => {
                let colunas = linhas.first().map_or(0, Vec::len);
                ectx.copy_text(escrever_tsv(&linhas));
                let iv = Intervalo::entre((0, 0), (linhas.len().saturating_sub(1), colunas.saturating_sub(1)));
                toast_concluido(ectx, frase_do_intervalo_copiado(iv));
            }
            (Trabalho::LerLinhas(_), Efeito::Linhas(linhas)) => {
                let n = linhas.len().saturating_sub(1);
                ectx.copy_text(escrever_tsv(&linhas));
                toast_concluido(
                    ectx,
                    if n == 1 {
                        "1 linha copiada.".to_string()
                    } else {
                        format!("{} linhas copiadas.", interface::formato::formatar_numero(n as u64))
                    },
                );
            }
            _ => {}
        }
        let (alteracoes, a_refazer) = self.retrato.as_ref().map_or((0, 0), |r| (r.alteracoes, r.passos_a_refazer));
        if self.rotulos_desfazer.len() != alteracoes && !self.rotulos_desfazer.is_empty() {
            log::warn!(
                "os rótulos do desfazer ({}) saíram de passo com o núcleo ({alteracoes}): o rótulo genérico",
                self.rotulos_desfazer.len()
            );
        }
        if self.rotulos_refazer.len() != a_refazer && !self.rotulos_refazer.is_empty() {
            log::warn!(
                "os rótulos do refazer ({}) saíram de passo com o núcleo ({a_refazer}): o rótulo genérico",
                self.rotulos_refazer.len()
            );
        }
    }

    /// A célula ativa vai para a linha (posição na tabela; sem filtro, a mesma na vista), e o foco para a grade.
    fn ir_para_linha(&mut self, posicao: u64) {
        let linhas = self.retrato.as_ref().map_or(0, |r| r.linhas);
        let coluna = self.grade.as_ref().and_then(|g| g.nucleo.selecao()).map_or(PRIMEIRA_DE_DADOS, |(_, c)| c);
        if let Some(g) = self.grade.as_mut() {
            g.nucleo.definir_linhas(usize::try_from(linhas).unwrap_or(usize::MAX));
            g.nucleo.selecionar(usize::try_from(posicao).ok().map(|l| (l, coluna)));
        }
        self.focar_grade = true;
    }

    // ------------------------------------------------------------------ a grade

    /// O que a pessoa fez na grade neste quadro.
    pub(crate) fn aplicar_grade(&mut self, ui: &mut Ui, r: RespostaNucleo, saida: SaidaDaGrade) {
        let ectx = ui.ctx().clone();
        let mut fim = saida.fim_edicao;
        // a célula em edição saiu da vista depois de o campo aparecer com o foco: grava
        if fim.is_none() && !saida.edicao_desenhada && self.edicao.as_ref().is_some_and(|e| e.focada) {
            fim = Some(FimEdicao::Salvar { foco: false });
        }
        if let Some(fim) = fim {
            self.terminar_edicao(ui, fim);
        }
        let mut fim_cabecalho = saida.fim_cabecalho;
        // o cabeçalho em edição saiu da vista depois de ganhar o foco: confirma
        if fim_cabecalho.is_none()
            && !saida.cabecalho_desenhado
            && self.cabecalho_edicao.as_ref().is_some_and(|e| e.focada)
        {
            fim_cabecalho = Some(FimCabecalho::Confirmar);
        }
        if let Some(fim) = fim_cabecalho {
            self.terminar_cabecalho(fim);
        }
        if let Some((c, ctrl, shift)) = saida.clique_th {
            self.clicar_cabecalho(c, ctrl, shift);
            // a seleção de colunas desfaz o intervalo (o Ctrl+C leva o que foi escolhido por último)
            if let Some(g) = self.grade.as_mut() {
                g.nucleo.recolher_intervalo();
            }
        }
        if r.clicou.is_some() || r.intervalo_mudou {
            // o clique numa célula e o intervalo desfazem a seleção de colunas
            self.colunas_selecionadas.clear();
        }
        if let Some(c) = saida.ordenar
            && let Some(nome) = self.retrato.as_ref().and_then(|r| r.nomes.get(c)).cloned()
        {
            self.ordenar(&nome);
        }
        if saida.nova_no_fim {
            let n = self.retrato.as_ref().map_or(0, |r| r.colunas());
            self.nova_coluna(n);
        }
        if let Some((l, sim)) = saida.marcar {
            self.marcar(l, sim);
        }
        if let Some(sim) = saida.marcar_todas {
            self.marcadas.clear();
            if sim {
                match self.janela.vista.clone() {
                    Some(v) => self.marcadas.extend(v.iter().copied()),
                    None => {
                        let n = self.linhas_da_vista();
                        self.marcadas.extend(0..n);
                    }
                }
            }
        }
        let dado = |c: usize| c.checked_sub(PRIMEIRA_DE_DADOS);
        let pendente = self.edicao.as_ref().map(|e| (e.linha, e.coluna, e.focada));
        if let Some((l, c)) = r.editar.and_then(|(l, c)| dado(c).map(|c| (l, c)))
            && !matches!(pendente, Some((pl, pc, false)) if (pl, pc) == (l, c))
        {
            self.editar(&ectx, l, c, None);
        }
        if let (Some(t), Some((l, c))) = (r.texto_digitado, self.grade.as_ref().and_then(|g| g.nucleo.selecao()))
            && let Some(c) = dado(c)
        {
            match self.edicao.as_mut() {
                Some(ed) if ed.focada => {}
                Some(ed) if (ed.linha, ed.coluna) == (l, c) => {
                    ed.texto.push_str(&t);
                    preparar_edicao(&ectx, ed, false);
                }
                _ => self.editar(&ectx, l, c, Some(t)),
            }
        }
        // o Delete repetido (ou a tecla segura) com a exclusão ainda na fila excluiria as linhas de baixo: espera
        if r.excluir && !self.marcadas.is_empty() && self.livre_para_editar() {
            self.mutar(Mutacao::Excluir { linhas: self.marcadas.iter().copied().collect() });
        }
        if r.espaco
            && let Some(l) = self.grade.as_ref().and_then(|g| g.nucleo.selecao()).map(|(l, _)| l)
        {
            let sim =
                !self.pagina.as_ref().and_then(|p| p.posicao(l as u64)).is_some_and(|p| self.marcadas.contains(&p));
            self.marcar(l, sim);
        }
        let id_corpo = self.grade.as_ref().map(|g| g.nucleo.id_corpo());
        let copia_sem_ativa = id_corpo.is_some_and(|id| ui.memory(|m| m.has_focus(id)))
            && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if r.copiar || copia_sem_ativa {
            self.copiar(&ectx);
        }
    }

    fn marcar(&mut self, l: usize, sim: bool) {
        let Some(p) = self.pagina.as_ref().and_then(|p| p.posicao(l as u64)) else { return };
        if sim {
            self.marcadas.insert(p);
        } else {
            self.marcadas.remove(&p);
        }
    }

    /// O Ctrl+C: o intervalo de células (em TSV, sem o cabeçalho), as colunas selecionadas, as linhas marcadas (em TSV,
    /// com o cabeçalho) ou a célula ativa.
    pub(crate) fn copiar(&mut self, ectx: &egui::Context) {
        if let Some(iv) = self.grade.as_ref().and_then(|g| g.nucleo.intervalo()).filter(Intervalo::varias) {
            self.copiar_intervalo(ectx, iv);
            return;
        }
        if !self.colunas_selecionadas.is_empty() {
            let colunas = self.colunas_selecionadas.iter().copied().collect();
            self.copiar_colunas(ectx, colunas);
            return;
        }
        if !self.marcadas.is_empty() {
            let colunas = self.retrato.as_ref().map_or(0, |r| r.colunas()) as u64;
            let celulas = self.marcadas.len() as u64 * colunas.max(1);
            if celulas > LIMITE_DA_COPIA {
                toast_atencao(ectx, textos::grande_demais_para_copiar(celulas));
                return;
            }
            self.enfileirar(Trabalho::LerLinhas(self.marcadas.iter().copied().collect()));
            return;
        }
        let texto =
            self.celula_ativa().and_then(|(l, c)| self.pagina.as_ref()?.celula(l as u64, c).map(str::to_string));
        if let Some(t) = texto {
            ectx.copy_text(t);
            toast_concluido(ectx, "Copiado.");
        }
    }

    /// O Ctrl+C de um intervalo da grade (linhas da vista, colunas da grade), como o Excel copia: na hora, se as linhas
    /// estão na página em mãos; senão numa tarefa, que lê as que faltam.
    fn copiar_intervalo(&mut self, ectx: &egui::Context, iv: Intervalo) {
        if iv.celulas() > LIMITE_DA_COPIA {
            toast_atencao(ectx, textos::grande_demais_para_copiar(iv.celulas()));
            return;
        }
        let Some(g) = self.grade.as_ref() else { return };
        // a coluna da tabela de cada coluna da grade (a provisória vai vazia)
        let colunas: Vec<Option<usize>> = iv
            .colunas()
            .map(|c| match g.dado(c).map(|d| self.mapa.get(d)) {
                Some(Some(ColunaDaGrade::Dados(t))) => Some(*t),
                Some(None) => g.dado(c),
                _ => None,
            })
            .collect();
        let pagina = self.pagina.as_ref();
        if pagina.is_some_and(|p| p.contem(iv.linha0 as u64) && p.contem(iv.linha1 as u64)) {
            let primeira = iv.coluna0;
            let texto = tsv_do_intervalo(iv, |l, c| {
                let t = colunas.get(c - primeira).copied().flatten()?;
                pagina?.celula(l as u64, t).map(str::to_string)
            });
            ectx.copy_text(texto);
            toast_concluido(ectx, frase_do_intervalo_copiado(iv));
            return;
        }
        // as posições na tabela das linhas da vista (sem filtro nem ordem, a vista é a própria tabela)
        let vista = self.janela.vista.clone();
        let posicoes: Vec<u64> = iv
            .linhas()
            .filter_map(|l| match &vista {
                Some(v) => v.get(l).copied(),
                None => Some(l as u64),
            })
            .collect();
        self.enfileirar(Trabalho::LerIntervalo { posicoes, colunas });
    }

    // ------------------------------------------------------------------ a edição no lugar

    /// Começa a editar a célula (linha da vista, coluna de dados), com o texto dela (ou com a tecla que começou).
    pub(crate) fn editar(&mut self, ectx: &egui::Context, l: usize, c: usize, digitado: Option<String>) {
        if !self.livre_para_editar() {
            return;
        }
        let Some(pagina) = self.pagina.as_ref() else { return };
        let (Some(original), Some(posicao)) = (pagina.celula(l as u64, c), pagina.posicao(l as u64)) else { return };
        let original = self
            .pendentes
            .iter()
            .rev()
            .find(|p| p.linha == l && p.coluna == c)
            .map_or(original, |p| p.texto.as_str())
            .to_string();
        let retrato = self.retrato.as_ref();
        let nome = retrato.and_then(|r| r.nomes.get(c)).cloned().unwrap_or_default();
        let numerica = retrato.and_then(|r| r.tipos.get(c)).copied().is_some_and(textos::numerica);
        let selecionar_tudo = digitado.is_none();
        let ed = Edicao {
            linha: l,
            coluna: c,
            posicao,
            texto: digitado.unwrap_or_else(|| original.clone()),
            original,
            numerica,
            rotulo: format!("{nome}, linha {}", posicao + 1),
            focada: false,
            pedir_foco: true,
        };
        preparar_edicao(ectx, &ed, selecionar_tudo);
        if let Some(g) = self.grade.as_mut() {
            g.nucleo.selecionar(Some((l, c + PRIMEIRA_DE_DADOS)));
        }
        self.edicao = Some(ed);
    }

    /// Termina a edição: grava (se o texto mudou), desiste ou grava e anda com o Tab.
    pub(crate) fn terminar_edicao(&mut self, ui: &Ui, fim: FimEdicao) {
        // grava mesmo com uma alteração na fila: a posição da célula é anterior a ela (`mutar` grava a célula aberta
        // antes de pôr outra alteração na fila, e nenhuma célula abre com uma que mexe nas posições esperando)
        let Some(ed) = self.edicao.take() else { return };
        if fim != FimEdicao::Cancelar {
            self.gravar_celula(ed);
        }
        match fim {
            FimEdicao::Salvar { foco: true } | FimEdicao::Cancelar => self.focar_grade = true,
            FimEdicao::Tab { voltando } => {
                if let Some(g) = self.grade.as_mut() {
                    g.nucleo.mover(0, if voltando { -1 } else { 1 });
                }
                self.focar_grade = true;
            }
            FimEdicao::Salvar { foco: false } => {}
        }
        ui.ctx().request_repaint();
    }

    // ------------------------------------------------------------------ os verbos

    fn dica_desfazer(&self, refazer: bool) -> Option<String> {
        let r = self.retrato.as_ref()?;
        let (pilha, verbo, no_nucleo) = if refazer {
            (&self.rotulos_refazer, "Refazer", r.passos_a_refazer)
        } else {
            (&self.rotulos_desfazer, "Desfazer", r.alteracoes)
        };
        // fora de passo com o núcleo: o rótulo genérico (Riscos)
        if pilha.len() != no_nucleo {
            return None;
        }
        pilha.last().map(|rotulo| format!("{verbo} {rotulo}"))
    }

    pub(crate) fn ofertas_do_csv(&self, _estado: &EstadoApp) -> Ofertas {
        let Some(r) = self.retrato.as_ref().filter(|_| self.aberta()) else { return Ofertas::default() };
        // o que leva posições (linhas, colunas) espera as alterações que mexem nelas; o resto entra na fila atrás
        let livre = self.livre_para_editar();
        let sem_posicoes = self.livre_para_salvar();
        let alvo = alvo_em_palavras(self.marcadas.len(), self.posicao_ativa());
        let com_alvo = |verbo: &str| match &alvo {
            Some(a) => Oferta::ligada().ligada_se(livre).com_dica(format!("{verbo} {a}")),
            None => Oferta::default().com_dica(verbo.to_string()),
        };
        let mut desfazer = Oferta::ligada().ligada_se(sem_posicoes && r.pode_desfazer);
        if let Some(d) = self.dica_desfazer(false) {
            desfazer = desfazer.com_dica(d);
        }
        let mut refazer = Oferta::ligada().ligada_se(sem_posicoes && r.pode_refazer);
        if let Some(d) = self.dica_desfazer(true) {
            refazer = refazer.com_dica(d);
        }
        // a célula em edição com outro texto e a alteração ainda na fila também valem (o Ctrl+S grava antes)
        let a_caminho = self.edicao.as_ref().is_some_and(|e| e.texto != e.original) || self.mutacao_pendente();
        Ofertas::default()
            .com(
                Verbo::Salvar,
                Oferta::ligada()
                    .ligada_se(sem_posicoes && (r.alterada || a_caminho))
                    .ocupada_se(!sem_posicoes)
                    .com_dica(format!("Salvar {} (Ctrl+S)", self.nome)),
            )
            .com(Verbo::Novo, Oferta::ligada().ligada_se(livre).com_rotulo("Nova linha"))
            .com(Verbo::NovaColuna, Oferta::ligada().ligada_se(livre).com_dica(self.dica_nova_coluna()))
            .com(Verbo::TrazerColunas, Oferta::ligada().ligada_se(livre))
            .com(Verbo::Duplicar, com_alvo("Duplicar"))
            .com(Verbo::Excluir, com_alvo("Excluir"))
            .com(Verbo::Desfazer, desfazer)
            .com(Verbo::Refazer, refazer)
            .com(Verbo::Atualizar, Oferta::ligada().ligada_se(sem_posicoes))
            .com(Verbo::Filtrar, Oferta::ligada().marcada_se(!self.filtros.is_empty()).com_dica(self.dica_filtrar()))
    }

    pub(crate) fn executar_verbo(&mut self, verbo: Verbo, ectx: &egui::Context) {
        match verbo {
            Verbo::Salvar => self.salvar(ectx),
            Verbo::TrazerColunas => self.trazer_pedido = Some(None),
            Verbo::Filtrar => self.abrir_condicao_pendente = true,
            Verbo::NovaColuna => {
                let n = self.retrato.as_ref().map_or(0, |r| r.colunas());
                let posicao = self.coluna_em_foco().map_or(n, |c| c + 1);
                self.nova_coluna(posicao);
            }
            // as posições destes saem da página e das marcas: com uma alteração que mexe nelas ainda na fila (o Ctrl+N
            // repetido, o atalho segurado), esperam
            Verbo::Novo | Verbo::Duplicar | Verbo::Excluir if !self.livre_para_editar() => {}
            Verbo::Novo => {
                let posicao = self.posicao_ativa().map_or(self.retrato.as_ref().map_or(0, |r| r.linhas), |p| p + 1);
                self.mutar(Mutacao::NovaLinha { posicao });
            }
            Verbo::Duplicar => {
                let linhas = self.alvo_das_linhas();
                if !linhas.is_empty() {
                    self.mutar(Mutacao::Duplicar { linhas });
                }
            }
            Verbo::Excluir => {
                let linhas = self.alvo_das_linhas();
                if !linhas.is_empty() {
                    self.mutar(Mutacao::Excluir { linhas });
                }
            }
            Verbo::Desfazer | Verbo::Refazer if !self.livre_para_salvar() => {}
            Verbo::Desfazer => {
                self.mutar(Mutacao::Desfazer);
            }
            Verbo::Refazer => {
                self.mutar(Mutacao::Refazer);
            }
            Verbo::Atualizar => self.atualizar(),
            _ => {}
        }
    }

    /// O Salvar (o verbo, o Ctrl+S e o "Salvar" da pergunta ao fechar, da casca): confirma o cabeçalho em edição,
    /// grava a célula aberta e depois o arquivo.
    pub(crate) fn salvar(&mut self, ectx: &egui::Context) {
        if self.salvando_agora() {
            return;
        }
        if self.cabecalho_edicao.is_some() {
            self.terminar_cabecalho(FimCabecalho::Confirmar);
        }
        if let Some(ed) = self.edicao.take() {
            self.gravar_celula(ed);
        } else if !self.retrato.as_ref().is_some_and(|r| r.alterada) && !self.trabalhando() {
            toast(ectx, "Nenhuma alteração para salvar.");
            return;
        }
        self.erro_de_gravacao = None;
        self.enfileirar(Trabalho::Salvar);
    }

    /// O aviso "Salvando vendas_2025.csv… 43%" com a barra e o Cancelar, quando a gravação passa de 0,4 s.
    pub(crate) fn aviso_da_gravacao(&mut self, doc: &mut interface::pecas::documento::Documento, ui: &mut Ui) {
        let Some(rod) = self.rodando.as_ref().filter(|r| matches!(r.trabalho, Trabalho::Salvar)) else { return };
        let agora = ui.ctx().input(|i| i.time);
        if agora - rod.desde < ESPERA_DO_CARD {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(ESPERA_DO_CARD));
            return;
        }
        let controle = std::sync::Arc::clone(&rod.controle);
        let fracao = controle.fracao();
        let nome = self.nome.clone();
        let mut cancelar = false;
        doc.aviso(ui, TomAviso::Informacao, |ui, cores| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let rotulo = match fracao {
                    Some(f) => format!("{} {}%", textos::salvando(&nome), (f * 100.0).round() as u32),
                    None => textos::salvando(&nome),
                };
                texto_simples(ui, &rotulo, 12.0, cores.texto);
                ui.allocate_ui(egui::vec2(LARGURA_BARRA_SALVAR, 8.0), |ui| {
                    barra_progresso(ui, fracao);
                });
                if ui.add(Botao::new("Cancelar").pequeno()).clicked() {
                    cancelar = true;
                }
            });
        });
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        if cancelar {
            controle.cancelar();
        }
    }

    /// "Nova coluna depois de quantidade" / "Nova coluna no fim".
    fn dica_nova_coluna(&self) -> String {
        let nome = self.coluna_em_foco().and_then(|c| self.retrato.as_ref()?.nomes.get(c).cloned());
        match nome {
            Some(n) => format!("Nova coluna depois de {n}"),
            None => "Nova coluna no fim".to_string(),
        }
    }

    /// Os rótulos do desfazer (os testes conferem e simulam o descompasso).
    #[doc(hidden)]
    pub fn rotulos_do_desfazer(&mut self) -> &mut Vec<String> {
        &mut self.rotulos_desfazer
    }

    #[doc(hidden)]
    pub fn rotulos_do_refazer(&mut self) -> &mut Vec<String> {
        &mut self.rotulos_refazer
    }

    /// As linhas marcadas (posições na tabela).
    pub fn linhas_marcadas(&self) -> Vec<u64> {
        self.marcadas.iter().copied().collect()
    }

    /// Marca exatamente estas linhas (posições na tabela).
    pub fn marcar_linhas(&mut self, linhas: impl IntoIterator<Item = u64>) {
        self.marcadas = linhas.into_iter().collect();
    }

    /// Há uma edição no lugar aberta.
    pub fn editando(&self) -> bool {
        self.edicao.is_some()
    }
}
