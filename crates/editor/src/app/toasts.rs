//! Os toasts: o erro de uma leitura ou de uma abertura (para quem a pediu), e a ação do toast: o "Mostrar na
//! pasta" da cópia de segurança, ou a carga de um documento (`tela_csv:`, `tela_dados:`), devolvida a ele.

use std::path::PathBuf;

use comum::{Codigo, ErroBanco};
use estado::ChaveAba;
use interface::pecas::toast::{acao_do_toast, toast_erro};

use crate::documento::Documento;

use super::tarefas::Tarefa;
use super::{Editor, Origem, Situacao};

impl Editor {
    /// Mostra o erro de uma leitura da casca: `Substituido` some, `BancoTrocado` ressincroniza, o resto é toast.
    pub(super) fn erro_da_casca(&mut self, ctx: &egui::Context, e: &ErroBanco) {
        if e.tem_codigo(Codigo::Substituido) {
            return;
        }
        if e.tem_codigo(Codigo::BancoTrocado) {
            self.ressincronizar(ctx);
            return;
        }
        toast_erro(ctx, e.mensagem.clone());
    }

    /// O erro de uma abertura vai para quem a pediu: a janela "Pedir o caminho" (se ainda aberta), o Início (mesmo
    /// fora de vista; o da linha de comando espera o Início abrir), ou o toast.
    pub(super) fn avisar_erro(&mut self, ctx: &egui::Context, origem: Origem, mensagem: String) {
        ctx.request_repaint();
        let pronta = matches!(self.situacao, Situacao::Pronta);
        let inicio_aberto = pronta && self.abas.achar(&ChaveAba::Inicio).is_some();
        match origem {
            Origem::Janela if self.janelas.caminho.is_some() => {
                if let Some(j) = self.janelas.caminho.as_mut() {
                    j.mostrar_erro(mensagem);
                }
            }
            Origem::Inicio | Origem::Partida if inicio_aberto => {
                if let Some(Documento::Inicio(t)) = self.documento_da_chave(&ChaveAba::Inicio) {
                    t.mostrar_erro(mensagem);
                }
            }
            Origem::Partida => self.erro_inicial = Some(mensagem),
            _ => toast_erro(ctx, mensagem),
        }
    }

    /// O clique na ação do toast da cópia de segurança ("Mostrar na pasta"): abre o Explorador de Arquivos no arquivo
    /// (nos testes, o gancho que recebe o caminho no lugar dele).
    /// As ações dos toasts dos documentos (o "Marcar as linhas sem par" do CSV, com a carga
    /// `tela_csv::PREFIXO_ACAO_TOAST…`, e o "Desfazer · Ctrl+Z" da aba Dados, `tela_dados::PREFIXO_ACAO_TOAST…`)
    /// voltam para o documento, que as lê no quadro seguinte.
    pub(super) fn cumprir_acao_do_toast(&mut self, ctx: &egui::Context) {
        let Some(caminho) = acao_do_toast(ctx) else { return };
        #[cfg(feature = "telas")]
        if caminho.starts_with(tela_csv::PREFIXO_ACAO_TOAST) || caminho.starts_with(tela_dados::PREFIXO_ACAO_TOAST) {
            interface::pecas::toast::devolver_acao_do_toast(ctx, caminho);
            return;
        }
        self.revelar(ctx, PathBuf::from(caminho));
    }

    /// O "Mostrar na pasta" (o do toast e o do menu de contexto do Explorador): abre o Explorador de Arquivos no
    /// arquivo numa tarefa; nos testes, chama o gancho com o caminho no lugar dele.
    pub(super) fn revelar(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        if let Some(gancho) = self.abrir_de_teste.as_mut() {
            gancho(&caminho);
            return;
        }
        let promessa = self.tarefa(ctx, move |s, _| s.revelar(&caminho));
        self.tarefas.push(Tarefa::Revelar(promessa));
    }

    /// O "Abrir no programa padrão" da ficha do arquivo de dados: o programa associado abre numa tarefa; nos testes,
    /// o gancho recebe o caminho no lugar dele.
    pub(super) fn abrir_no_programa(&mut self, ctx: &egui::Context, caminho: PathBuf) {
        if let Some(gancho) = self.abrir_de_teste.as_mut() {
            gancho(&caminho);
            return;
        }
        let promessa = self.tarefa(ctx, move |s, _| s.abrir_arquivo(&caminho));
        self.tarefas.push(Tarefa::Revelar(promessa));
    }
}
