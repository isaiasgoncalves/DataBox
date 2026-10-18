//! A janela "Pedir o caminho" : quando o diálogo
//! do Windows não abre, o caminho é digitado. "Abrir arquivo" / "Abrir", "Abrir pasta" / "Abrir", "Novo banco" / "Criar" e "Salvar como" /
//! "Salvar" (a cópia de segurança), com o campo do caminho e o botão primário. Ela não confere se o arquivo
//! existe: quem confere é o serviço (e a Visão geral pergunta antes de substituir).
//!
//! Abrir e criar rodam a troca de banco com a janela aberta: o erro aparece nela, em
//! `.notice.perigo`, e ela continua aberta; deu certo, a casca a fecha junto com as outras janelas.

use std::path::{Path, PathBuf};

use egui::text::CCursor;
use egui::text_edit::TextEditState;
use egui::text_selection::CCursorRange;
use interface::pecas::avisos::aviso_texto;
use interface::pecas::botao::Botao;
use interface::pecas::campos::{Entrada, campo, campos_duas_colunas};
use interface::pecas::janela::Janela;

use crate::inicio::limpar_caminho_digitado;
use crate::recursos::caminho_sugerido;

/// O "i" do campo.
pub(crate) const AJUDA: &str = "A janela de escolha de arquivo do Windows não está disponível. Digite o caminho \
                                completo, com a pasta e o nome do arquivo.";
/// O campo vazio.
pub(crate) const MSG_VAZIO: &str = "Digite o caminho do arquivo.";
/// O "i" e o campo vazio do "Abrir pasta".
pub(crate) const AJUDA_PASTA: &str = "A janela de escolha de pasta do Windows não está disponível. Digite o caminho \
                                      completo da pasta.";
pub(crate) const MSG_VAZIO_PASTA: &str = "Digite o caminho da pasta.";

/// Para que o caminho foi pedido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Proposito {
    /// "Abrir arquivo" (Abrir arquivo, Abrir outro banco, Ctrl+O): o caminho é classificado como o do Início (pasta,
    /// banco ou arquivo de dados).
    Abrir { somente_leitura: bool },
    /// "Novo banco".
    Novo,
    /// "Salvar como" da cópia de segurança.
    Copia,
    /// "Abrir pasta" (Arquivo › Abrir pasta, Ctrl+Shift+O).
    Pasta,
}

/// O que a janela devolveu no quadro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaidaCaminho {
    /// O caminho digitado (sem aspas nem espaços em volta).
    Enviou(PathBuf),
    /// ✕, Esc ou clique no fundo.
    Fechou,
}

/// A janela "Pedir o caminho".
pub(crate) struct PedirCaminho {
    pub proposito: Proposito,
    titulo: &'static str,
    botao: &'static str,
    dica: &'static str,
    /// O rótulo do campo, o "i" dele e a frase do campo vazio (o arquivo ou a pasta).
    rotulo: &'static str,
    ajuda: &'static str,
    vazio: &'static str,
    caminho: String,
    aviso: Option<String>,
    /// A troca pedida por ela está em andamento (o botão fica desabilitado).
    ocupado: bool,
    /// Selecionar o caminho sugerido quando a janela abre.
    selecionar: bool,
    focar: bool,
}

impl PedirCaminho {
    fn nova(
        proposito: Proposito,
        titulo: &'static str,
        botao: &'static str,
        dica: &'static str,
        valor: String,
    ) -> Self {
        PedirCaminho {
            proposito,
            titulo,
            botao,
            dica,
            rotulo: "Caminho do arquivo",
            ajuda: AJUDA,
            vazio: MSG_VAZIO,
            selecionar: !valor.is_empty(),
            caminho: valor,
            aviso: None,
            ocupado: false,
            focar: false,
        }
    }

    /// "Abrir arquivo", com o campo vazio.
    pub fn abrir(somente_leitura: bool) -> Self {
        Self::nova(
            Proposito::Abrir { somente_leitura },
            "Abrir arquivo",
            "Abrir",
            "C:\\pasta\\arquivo.db",
            String::new(),
        )
    }

    /// "Novo banco", com "novo.db" na pasta do banco aberto.
    pub fn novo(pasta: Option<&Path>) -> Self {
        Self::nova(Proposito::Novo, "Novo banco", "Criar", "C:\\pasta\\arquivo.db", caminho_sugerido(pasta, "novo.db"))
    }

    /// "Salvar como" da cópia de segurança, com o nome sugerido na pasta do banco.
    pub fn copia(pasta: &Path, nome: &str) -> Self {
        Self::nova(Proposito::Copia, "Salvar como", "Salvar", "C:\\pasta\\arquivo", caminho_sugerido(Some(pasta), nome))
    }

    /// "Abrir pasta", com o campo vazio ("Caminho da pasta", dica `C:\pasta`).
    pub fn pasta() -> Self {
        PedirCaminho {
            rotulo: "Caminho da pasta",
            ajuda: AJUDA_PASTA,
            vazio: MSG_VAZIO_PASTA,
            ..Self::nova(Proposito::Pasta, "Abrir pasta", "Abrir", "C:\\pasta", String::new())
        }
    }

    /// A troca pedida não deu certo: a mensagem aparece na janela, que continua aberta.
    pub fn mostrar_erro(&mut self, mensagem: String) {
        self.aviso = Some(mensagem);
        self.ocupado = false;
    }

    /// A troca não chegou a começar (outra estava em andamento): o botão volta.
    pub fn desocupar(&mut self) {
        self.ocupado = false;
    }

    /// Desenha a janela; devolve o caminho enviado (Enter no campo ou o botão) ou o pedido de fechar.
    pub fn mostrar(&mut self, ctx: &egui::Context) -> Option<SaidaCaminho> {
        let mut clicou = false;
        let ocupado = self.ocupado;
        let botao = self.botao;
        let dica = self.dica;
        let titulo = self.titulo;
        let (rotulo, ajuda) = (self.rotulo, self.ajuda);
        let PedirCaminho { caminho, aviso, selecionar, focar, .. } = self;
        let r = Janela::new("pedir-caminho", titulo)
            .rodape_direita(|ui| {
                if ui.add(Botao::primario(botao).habilitado(!ocupado)).clicked() {
                    clicou = true;
                }
            })
            .mostrar(ctx, |ui| {
                if let Some(a) = aviso.as_deref() {
                    aviso_texto(ui, a, true);
                }
                campos_duas_colunas(ui, |c| {
                    c.largo(|ui| {
                        campo(ui, rotulo, Some(ajuda), |ui| {
                            let saida = Entrada::new(caminho).dica(dica).rotulo(rotulo).mostrar(ui);
                            let resp = saida.response.response;
                            if std::mem::take(selecionar) {
                                let n = caminho.chars().count();
                                let mut estado = TextEditState::load(ui.ctx(), resp.id).unwrap_or_default();
                                estado.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(n))));
                                estado.store(ui.ctx(), resp.id);
                            }
                            if std::mem::take(focar) {
                                resp.request_focus();
                            }
                        });
                    });
                });
            });
        if r.pediu_fechar {
            return Some(SaidaCaminho::Fechou);
        }
        // com a troca a caminho, o botão está desabilitado e o Enter não envia (o submit implícito do `<form>`
        // não acontece com o botão padrão desabilitado)
        if !(clicou || r.enviou) || self.ocupado {
            return None;
        }
        let texto = limpar_caminho_digitado(&self.caminho);
        if texto.is_empty() {
            self.aviso = Some(self.vazio.to_string());
            self.focar = true;
            ctx.request_repaint();
            return None;
        }
        self.aviso = None;
        if self.proposito != Proposito::Copia {
            self.ocupado = true;
        }
        Some(SaidaCaminho::Enviou(PathBuf::from(texto)))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn titulos_botoes_e_valores_como_o_javascript() {
        let a = PedirCaminho::abrir(false);
        assert_eq!((a.titulo, a.botao, a.caminho.as_str(), a.selecionar), ("Abrir arquivo", "Abrir", "", false));
        let n = PedirCaminho::novo(Some(Path::new(r"C:\dados\")));
        assert_eq!((n.titulo, n.botao, n.caminho.as_str()), ("Novo banco", "Criar", r"C:\dados\novo.db"));
        assert!(n.selecionar);
        let n = PedirCaminho::novo(None);
        assert_eq!(n.caminho, "novo.db");
        let c = PedirCaminho::copia(Path::new(r"C:\dados"), "vendas (cópia).db");
        assert_eq!((c.titulo, c.botao, c.dica), ("Salvar como", "Salvar", "C:\\pasta\\arquivo"));
        assert_eq!(c.caminho, r"C:\dados\vendas (cópia).db");
        let p = PedirCaminho::pasta();
        assert_eq!((p.titulo, p.botao, p.dica, p.caminho.as_str()), ("Abrir pasta", "Abrir", "C:\\pasta", ""));
        assert_eq!(
            (p.rotulo, p.vazio, p.proposito),
            ("Caminho da pasta", "Digite o caminho da pasta.", Proposito::Pasta)
        );
        assert_eq!((a.rotulo, a.vazio), ("Caminho do arquivo", MSG_VAZIO));
    }
}
