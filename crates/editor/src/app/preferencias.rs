//! As preferências: o tema, o idioma e a aparência do código (das preferências para o interface) e a janela
//! Preferências, com o que ela põe em vigor na hora e a leitura da fonte do código numa tarefa.

use comum::ErroBanco;
use estado::Promessa;
use interface::codigo::{self, AparenciaCodigo, CoresCodigo};
use interface::fontes;
use interface::pecas::toast::toast_erro;
use interface::tema::{self, Tema};

use crate::casca::preferencias::{Efeito, Fim, JanelaPreferencias, Secao};

use super::Editor;
use super::tarefas::Tarefa;

/// prefs::Tema → o Tema da interface (pelo texto: "system" | "light" | "dark").
pub fn tema_da_interface(t: prefs::Tema) -> Tema {
    Tema::de_str(t.como_str()).unwrap_or_default()
}

/// O Tema da interface → prefs::Tema.
pub fn tema_das_prefs(t: Tema) -> prefs::Tema {
    prefs::Tema::de_texto(t.como_str()).unwrap_or_default()
}

/// A aparência do código das preferências → a do interface (as cores pelo texto do arquivo).
pub fn aparencia_da_interface(a: &prefs::Aparencia) -> AparenciaCodigo {
    AparenciaCodigo {
        cores: CoresCodigo::de_str(a.cores_codigo.como_str()).unwrap_or_default(),
        familia: a.fonte_codigo.clone(),
        tamanho: a.tamanho_codigo,
    }
}

/// Aplica a aparência do código das preferências: as cores e o tamanho na hora e, se a família não for a padrão
/// (Consolas), a escolhida, lida do disco **agora** (uma fonte, ao subir, ao lado das do sistema que o
/// `tema::aplicar` lê; nas Preferências a leitura vai numa tarefa). Uma família que não existe nesta máquina, ou que
/// não é fonte, fica de fora: vale a padrão (e a aparência em vigor diz None), com o aviso no registro.
pub fn aplicar_aparencia(ctx: &egui::Context, a: &prefs::Aparencia) {
    let mut aparencia = aparencia_da_interface(a);
    match aparencia.familia.clone().filter(|f| f != fontes::FAMILIA_MONO_PADRAO) {
        None => fontes::restaurar_mono(ctx),
        Some(nome) => {
            let lida = fontes::familias_mono_disponiveis()
                .into_iter()
                .find(|f| f.nome == nome)
                .and_then(|f| fontes::ler_familia_mono(&f));
            if !lida.is_some_and(|bytes| fontes::trocar_mono(ctx, bytes, &nome)) {
                log::warn!(
                    "A fonte do código {nome:?} não foi encontrada nesta máquina; vale a {}.",
                    fontes::FAMILIA_MONO_PADRAO
                );
                fontes::restaurar_mono(ctx);
                aparencia.familia = None;
            }
        }
    }
    interface::codigo::definir(ctx, aparencia);
}

/// A leitura do arquivo da fonte do código escolhida nas Preferências (numa tarefa; o `trocar_mono` vem depois, na
/// thread da interface). Mora no editor, não na janela: o Cancelar fecha a janela com uma leitura em andamento.
pub(super) struct FontePendente {
    nome: String,
    promessa: Promessa<Vec<u8>>,
}

impl Editor {
    /// Abre a janela Preferências na `secao` (com o estado já lido: a janela guarda o retrato do que vale agora).
    pub(super) fn abrir_preferencias(&mut self, ctx: &egui::Context, secao: Secao) {
        if self.carregado && self.janelas.preferencias.is_none() {
            self.janelas.preferencias = Some(JanelaPreferencias::nova(ctx, self.tema, &self.estado, secao));
        }
    }

    /// Põe em vigor, agora, uma mudança das Preferências (e a que o Cancelar desfaz): o tema, as cores e o tamanho do
    /// código na hora; a família da fonte depois de ler o arquivo, numa tarefa ([`Editor::receber_fonte`]).
    fn aplicar_efeito(&mut self, ctx: &egui::Context, efeito: Efeito) {
        match efeito {
            Efeito::Tema(t) => {
                self.tema = tema_da_interface(t);
                tema::aplicar(ctx, self.tema);
            }
            Efeito::Idioma(e) => {
                // os textos do t!, os números e as datas mudam no próximo quadro
                idioma::definir(e.resolver());
            }
            Efeito::Cores(c) => {
                let mut a = codigo::atual(ctx);
                a.cores = CoresCodigo::de_str(c.como_str()).unwrap_or_default();
                codigo::definir(ctx, a);
            }
            Efeito::Tamanho(px) => {
                let mut a = codigo::atual(ctx);
                a.tamanho = px;
                codigo::definir(ctx, a);
            }
            Efeito::Fonte(None) => {
                // sem leitura: a padrão já está nas definições de base
                self.fonte_pendente = None;
                fontes::restaurar_mono(ctx);
                let mut a = codigo::atual(ctx);
                a.familia = None;
                codigo::definir(ctx, a);
            }
            Efeito::Fonte(Some(nome)) => {
                // procurar a família no disco (um `stat` por candidata) e ler o arquivo, tudo numa tarefa: a thread
                // da interface não toca no disco. A que não existe ou não é fonte volta como erro, que o
                // `receber_fonte` trata como a recusa de sempre
                let procurada = nome.clone();
                let promessa = Promessa::rodar(ctx, move || {
                    let familia = fontes::familias_mono_disponiveis()
                        .into_iter()
                        .find(|f| f.nome == procurada)
                        .ok_or_else(|| ErroBanco::novo(format!("A fonte {procurada} não está nesta máquina.")))?;
                    fontes::ler_familia_mono(&familia).ok_or_else(|| {
                        ErroBanco::novo(format!("Não foi possível ler o arquivo da fonte {}.", familia.nome))
                    })
                });
                self.fonte_pendente = Some(FontePendente { nome, promessa });
            }
        }
        ctx.request_repaint();
    }

    /// A leitura da fonte terminou: a família passa a valer (a partir da passada seguinte, regra do `set_fonts`); se o
    /// arquivo não pôde ser lido, a escolha das Preferências volta para a que estava em vigor.
    pub(super) fn receber_fonte(&mut self, ctx: &egui::Context) {
        let Some(r) = self.fonte_pendente.as_mut().and_then(|f| f.promessa.tomar()) else { return };
        let Some(FontePendente { nome, .. }) = self.fonte_pendente.take() else { return };
        let carregou = match r {
            Ok(bytes) => fontes::trocar_mono(ctx, bytes, &nome),
            Err(_) => false,
        };
        if carregou {
            let mut a = codigo::atual(ctx);
            a.familia = Some(nome);
            codigo::definir(ctx, a);
            ctx.request_repaint();
        } else {
            self.fonte_recusada(ctx, &nome);
        }
    }

    /// A fonte `nome` não está nesta máquina ou o arquivo não é uma fonte: o toast e a escolha desfeita.
    fn fonte_recusada(&mut self, ctx: &egui::Context, nome: &str) {
        log::warn!("A fonte do código {nome:?} não pôde ser carregada.");
        toast_erro(ctx, format!("Não foi possível carregar a fonte {nome}."));
        let em_vigor = codigo::atual(ctx).familia;
        if let Some(j) = self.janelas.preferencias.as_mut() {
            j.fonte_recusada(em_vigor);
        }
    }

    /// Desenha as Preferências (por cima de tudo, como a Exportar) e cumpre o que ela pede: as mudanças valem na hora;
    /// Cancelar reaplica o retrato da abertura; Salvar grava (numa tarefa) só o que mudou e fecha.
    pub(super) fn mostrar_preferencias(&mut self, ctx: &egui::Context) {
        let Some(janela) = self.janelas.preferencias.as_mut() else { return };
        let saida = janela.mostrar(ctx, &self.estado);
        for efeito in saida.efeitos {
            self.aplicar_efeito(ctx, efeito);
        }
        match saida.fim {
            None => {}
            Some(Fim::Cancelou { reverter }) => {
                for efeito in reverter {
                    self.aplicar_efeito(ctx, efeito);
                }
                self.janelas.preferencias = None;
            }
            Some(Fim::Salvou(g)) => {
                self.janelas.preferencias = None;
                let limpou_historico = g.limpar_historico;
                let promessa = self.tarefa(ctx, move |s, _| {
                    if g.limpar_recentes {
                        s.prefs().limpar_recentes();
                    }
                    if g.limpar_historico {
                        s.prefs().limpar_historico_sql();
                    }
                    if g.mudanca == prefs::Mudanca::default() {
                        Ok(s.prefs().resumo())
                    } else {
                        s.gravar_preferencias(g.mudanca)
                    }
                });
                // o resumo que volta vale como o do Exibir › Tema: as preferências, os recentes e a exportação novos;
                // o Histórico só é relido quando a gravação terminar (antes, a leitura corria contra a limpeza)
                self.tarefas.push(Tarefa::Tema { promessa, limpou_historico });
            }
        }
    }
}
