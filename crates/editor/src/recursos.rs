//! Os diálogos do Windows da casca (com o gancho dos testes) e os nomes sugeridos. As telas deste crate (Início,
//! Visão geral, tabela) recebem só o `estado::Contexto` e pedem tudo à casca por `Acao`: as trocas de banco do Início
//! (`AbrirBanco`, `EscolherEAbrir`, `NovoBanco`; a casca reconhece a aba de origem, e o erro volta ao card "Abrir um
//! banco"), as releituras (`RecarregarBanco`, `RecarregarObjetos`, `RecarregarPrefs`) e o diálogo da cópia de
//! segurança (`EscolherCopia`, que a casca abre pelo [`Dialogador`] e devolve à Visão geral).
//!
//! O diálogo do Windows (rfd) devolve `None` tanto quando a pessoa cancela quanto quando ele não consegue abrir.
//! Um `None` que volta antes de [`LIMIAR_SEM_DIALOGO`] não foi uma pessoa cancelando (o diálogo nem chegou a
//! aparecer): daí em diante, nesta execução, a casca usa a janela "Pedir o caminho".

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use estado::{Dialogos, Escolha, FormatoArquivo};

/// Um diálogo nativo que "cancela" mais rápido que isto não abriu.
pub(crate) const LIMIAR_SEM_DIALOGO: Duration = Duration::from_millis(150);

/// O que um diálogo devolveu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RespostaDialogo {
    Escolhido(Escolha),
    /// A pessoa cancelou.
    Cancelado,
    /// Não há diálogo nativo: a casca abre a janela "Pedir o caminho".
    Indisponivel,
}

/// Que diálogo do Windows a casca ia abrir (para o gancho de testes [`crate::Editor::com_dialogo`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedidoDialogo {
    /// "Abrir arquivo" (Abrir arquivo, Ctrl+O): os bancos e os arquivos de dados, com os três filtros
    /// (`estado::filtros_de_abrir_arquivo`); a casca classifica o escolhido como o `Acao::AbrirCaminho`.
    AbrirBanco,
    /// "Abrir pasta" (Arquivo › Abrir pasta, Ctrl+Shift+O, o cartão do Início, o botão do Explorador).
    AbrirPasta,
    /// "Salvar banco SQLite" do Novo banco (nome sugerido "novo.db").
    NovoBanco,
    /// "Salvar banco SQLite" da cópia de segurança (nome sugerido "vendas (cópia).db").
    CopiaDeSeguranca { nome_sugerido: String },
}

type DialogoFalso = Box<dyn FnMut(&PedidoDialogo) -> Option<PathBuf>>;

/// Abre os diálogos do Windows (pelo `estado::Dialogos`) ou, nos testes, pergunta ao gancho.
#[derive(Default)]
pub(crate) struct Dialogador {
    falso: Option<DialogoFalso>,
    /// O diálogo nativo não abriu nesta execução (ou o teste pediu a janela "Pedir o caminho").
    indisponivel: bool,
}

fn escolha(caminho: PathBuf) -> Escolha {
    let pasta = caminho.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf);
    Escolha { caminho, pasta }
}

impl Dialogador {
    pub fn com_falso(f: impl FnMut(&PedidoDialogo) -> Option<PathBuf> + 'static) -> Dialogador {
        Dialogador { falso: Some(Box::new(f)), indisponivel: false }
    }

    /// Daqui em diante, sem diálogo nativo (a janela "Pedir o caminho").
    pub fn sem_nativo(&mut self) {
        self.indisponivel = true;
    }

    /// Sem janela do eframe (testes) e sem gancho, nenhum diálogo de verdade abre (ele travaria o teste).
    fn sem_dialogo(&self, frame: Option<&eframe::Frame>) -> bool {
        frame.is_none() && self.falso.is_none()
    }

    /// Roda o diálogo nativo e separa o "cancelou" do "não abriu" pelo tempo que ele levou.
    fn nativo(&mut self, abrir: impl FnOnce() -> Option<Escolha>) -> RespostaDialogo {
        let inicio = Instant::now();
        match abrir() {
            Some(e) => RespostaDialogo::Escolhido(e),
            None if inicio.elapsed() < LIMIAR_SEM_DIALOGO => {
                log::warn!("O diálogo de arquivos do sistema não abriu; o caminho passa a ser digitado numa janela.");
                self.indisponivel = true;
                RespostaDialogo::Indisponivel
            }
            None => RespostaDialogo::Cancelado,
        }
    }

    fn falso_ou(&mut self, pedido: &PedidoDialogo) -> Option<RespostaDialogo> {
        if self.indisponivel {
            return Some(RespostaDialogo::Indisponivel);
        }
        let f = self.falso.as_mut()?;
        Some(f(pedido).map_or(RespostaDialogo::Cancelado, |c| RespostaDialogo::Escolhido(escolha(c))))
    }

    /// "Abrir arquivo" (o que o DataBox abre: bancos e arquivos de dados), na última pasta usada.
    pub fn abrir_banco(&mut self, frame: Option<&eframe::Frame>, pasta: Option<PathBuf>) -> RespostaDialogo {
        if let Some(r) = self.falso_ou(&PedidoDialogo::AbrirBanco) {
            return r;
        }
        if self.sem_dialogo(frame) {
            return RespostaDialogo::Cancelado;
        }
        self.nativo(|| Dialogos::novo(frame, pasta).abrir_arquivo())
    }

    /// "Abrir pasta", na última pasta usada; a `Escolha::pasta` é a pasta de cima da escolhida.
    pub fn abrir_pasta(&mut self, frame: Option<&eframe::Frame>, pasta: Option<PathBuf>) -> RespostaDialogo {
        if let Some(r) = self.falso_ou(&PedidoDialogo::AbrirPasta) {
            return r;
        }
        if self.sem_dialogo(frame) {
            return RespostaDialogo::Cancelado;
        }
        self.nativo(|| Dialogos::novo(frame, pasta).abrir_pasta())
    }

    /// "Salvar como" de um banco (novo banco, cópia de segurança). O Windows já pergunta antes de substituir.
    pub fn salvar_banco(
        &mut self,
        frame: Option<&eframe::Frame>,
        pasta: Option<PathBuf>,
        pedido: PedidoDialogo,
        nome_sugerido: &str,
    ) -> RespostaDialogo {
        if let Some(r) = self.falso_ou(&pedido) {
            return r;
        }
        if self.sem_dialogo(frame) {
            return RespostaDialogo::Cancelado;
        }
        self.nativo(|| Dialogos::novo(frame, pasta).salvar_como(nome_sugerido, FormatoArquivo::Db))
    }
}

/// O caminho sugerido na janela "Pedir o caminho": o nome dentro da pasta do
/// banco aberto, ou só o nome.
pub(crate) fn caminho_sugerido(pasta: Option<&Path>, nome: &str) -> String {
    match pasta.map(|p| p.display().to_string()).filter(|p| !p.is_empty()) {
        Some(p) => format!("{}{}{nome}", p.trim_end_matches(['\\', '/']), std::path::MAIN_SEPARATOR),
        None => nome.to_string(),
    }
}

/// O nome do arquivo sem a extensão: "vendas.db" → "vendas"; vazio → "banco".
pub(crate) fn nome_base(nome: &str) -> String {
    let base = match nome.rfind('.') {
        Some(i) if !nome[i + 1..].is_empty() && !nome[i + 1..].contains(['\\', '/']) => &nome[..i],
        _ => nome,
    };
    if base.is_empty() { "banco".to_string() } else { base.to_string() }
}

/// O nome sugerido para um arquivo exportado: os caracteres que o Windows não
/// aceita viram "_"; vazio → "tabela".
pub(crate) fn nome_arquivo(nome: &str) -> String {
    let mut saida = String::with_capacity(nome.len());
    let mut em_proibido = false;
    for c in nome.chars() {
        let proibido = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20;
        if proibido {
            if !em_proibido {
                saida.push('_');
            }
            em_proibido = true;
        } else {
            saida.push(c);
            em_proibido = false;
        }
    }
    let limpo = saida.trim();
    if limpo.is_empty() { "tabela".to_string() } else { limpo.to_string() }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_base_como_o_javascript() {
        assert_eq!(nome_base("vendas.db"), "vendas");
        assert_eq!(nome_base("arquivo.tar.gz"), "arquivo.tar");
        assert_eq!(nome_base("sem_extensao"), "sem_extensao");
        assert_eq!(nome_base(".db"), "banco");
        assert_eq!(nome_base(""), "banco");
        assert_eq!(nome_base("nome."), "nome.");
        assert_eq!(nome_base("."), ".");
    }

    #[test]
    fn caminho_sugerido_como_o_javascript() {
        if cfg!(windows) {
            assert_eq!(caminho_sugerido(Some(Path::new(r"C:\dados")), "novo.db"), r"C:\dados\novo.db");
            assert_eq!(caminho_sugerido(Some(Path::new(r"C:\dados\\")), "novo.db"), r"C:\dados\novo.db");
        } else {
            assert_eq!(caminho_sugerido(Some(Path::new("/dados")), "novo.db"), "/dados/novo.db");
            assert_eq!(caminho_sugerido(Some(Path::new("/dados//")), "novo.db"), "/dados/novo.db");
        }
        assert_eq!(caminho_sugerido(None, "novo.db"), "novo.db");
    }

    #[test]
    fn sem_nativo_vale_para_todos_os_dialogos() {
        let mut d = Dialogador::com_falso(|_| Some(PathBuf::from(r"C:\x\y.db")));
        assert!(matches!(d.abrir_banco(None, None), RespostaDialogo::Escolhido(_)));
        d.sem_nativo();
        assert_eq!(d.abrir_banco(None, None), RespostaDialogo::Indisponivel);
        assert_eq!(d.salvar_banco(None, None, PedidoDialogo::NovoBanco, "novo.db"), RespostaDialogo::Indisponivel);
        assert_eq!(d.abrir_pasta(None, None), RespostaDialogo::Indisponivel);
        assert_eq!(d.abrir_pasta(None, None), RespostaDialogo::Indisponivel);
        // sem gancho e sem janela (testes), nada abre: conta como cancelado
        let mut d = Dialogador::default();
        assert_eq!(d.abrir_banco(None, None), RespostaDialogo::Cancelado);
    }

    #[test]
    fn nome_arquivo_como_o_javascript() {
        assert_eq!(nome_arquivo("clientes"), "clientes");
        assert_eq!(nome_arquivo("a/b:c"), "a_b_c");
        assert_eq!(nome_arquivo("a<>b"), "a_b");
        assert_eq!(nome_arquivo("  ?  "), "_");
        assert_eq!(nome_arquivo("   "), "tabela");
        assert_eq!(nome_arquivo("Tabela com espaço"), "Tabela com espaço");
    }
}
