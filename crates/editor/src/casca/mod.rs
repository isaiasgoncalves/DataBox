//! A casca: a barra de título integrada aos
//! menus ([`janela`], com os menus de [`menus`] e os controles da janela sem decoração), a barra de ferramentas
//! ([`ferramentas`]), a barra de ícones ([`icones`]), o painel da esquerda com o Explorador ([`explorador`]) ou o
//! Histórico de consultas ([`historico`]), a faixa de abas ([`abas`]), o painel de Propriedades ([`propriedades`]), as
//! larguras dos painéis laterais e as divisórias que as mudam ([`larguras`]), a barra de status ([`status`]), os atalhos de teclado ([`atalhos`]), o aviso de transação aberta e as janelas
//! "Atalhos do teclado", "Preferências" ([`preferencias`]) e "Sobre o DataBox". As peças são as do interface
//! (`pecas::casca`, `barra_titulo`,
//! `ferramentas`, `arvore`, `painel`, `abas_documento`, `barra_status`); o que elas pedem vira [`Acao`] para a casca
//! aplicar no fim do quadro. Os nós do Explorador e o Início da barra de ícones abrem a aba da rota ou focam a que
//! já está aberta.

pub(crate) mod abas;
pub(crate) mod atalhos;
pub(crate) mod explorador;
pub(crate) mod ferramentas;
pub(crate) mod historico;
pub(crate) mod icones;
pub(crate) mod janela;
pub(crate) mod larguras;
pub(crate) mod menus;
pub(crate) mod preferencias;
pub(crate) mod propriedades;
pub(crate) mod sobre;
pub(crate) mod status;

use std::path::PathBuf;

use egui::{Id, Rect, Ui, UiBuilder, vec2};
use estado::{Acao, Contexto, Estado, Rota};
use interface::formato::{formatar_numero, formatar_numero_opt, sem_acento};
use interface::pecas::avisos::{TomAviso, carregando, texto_com_link_a_direita};
use interface::pecas::detalhes::detalhes;
use interface::pecas::documento::Documento as DocumentoPeca;
use interface::pecas::janela::Janela;
use interface::pecas::texto::Trecho;

use crate::documento::Documento;

/// A frase do aviso de transação aberta (depois do `strong`). A mesma frase, inteira, é a dica do modo na barra de
/// status ([`frase_da_transacao`]).
pub(crate) const AVISO_TRANSACAO: &str = " O que for salvo agora só vai para o arquivo depois do COMMIT; fechar o banco \
                                          descarta tudo desde o BEGIN. ";

/// O nome da Consulta que abriu a transação, como está na faixa de abas ("Consulta 2"); "Consulta SQL" quando
/// nenhuma aba abriu a transação (ela veio de uma Consulta já fechada ou de outro lugar).
pub(crate) fn nome_da_consulta(numero: Option<u32>) -> String {
    match numero {
        Some(n) => format!("Consulta {n}"),
        None => "Consulta SQL".to_string(),
    }
}

/// O começo do aviso, em destaque: "Há uma transação aberta na Consulta 1.".
pub(crate) fn inicio_do_aviso(consulta: &str) -> String {
    format!("Há uma transação aberta na {consulta}.")
}

/// O aviso inteiro numa frase só, para a dica do modo na barra de status e para os leitores de tela.
pub(crate) fn frase_da_transacao(consulta: &str) -> String {
    format!("{}{}", inicio_do_aviso(consulta), AVISO_TRANSACAO.trim_end())
}

/// As linhas da janela "Atalhos do teclado" (os da grade, da Consulta e da casca: as
/// abas, os painéis, o Atualizar, a pasta e o Copiar caminho do Explorador).
pub(crate) const ATALHOS: [(&str, &str); 17] = [
    ("Ctrl+Shift+O", "Abrir uma pasta"),
    ("Ctrl+O", "Abrir um arquivo"),
    ("Ctrl+S", "Salvar"),
    ("Ctrl+W", "Fechar aba"),
    ("Ctrl+Z", "Desfazer"),
    ("Ctrl+Y", "Refazer"),
    ("Ctrl+Shift+V", "Colar com correspondência"),
    ("Ctrl+Tab e Ctrl+Shift+Tab", "Ir para a aba seguinte e para a anterior"),
    ("Alt+← e Alt+→", "Voltar e avançar entre as abas"),
    ("F5", "Atualizar a aba (a Visão geral ou a tabela)"),
    ("Ctrl+B", "Mostrar ou esconder o Explorador"),
    ("Ctrl+Alt+B", "Mostrar ou esconder as Propriedades"),
    ("Ctrl+Enter", "Executar o comando, na Consulta SQL"),
    ("Ctrl+F", "Ir para a busca da barra (tabelas, linhas, colunas ou o editor)"),
    ("Ctrl+,", "Abrir as Preferências"),
    ("Ctrl+Shift+C", "Copiar o caminho do item do Explorador"),
    ("Esc", "Fechar a janela ou o menu aberto, ou interromper a consulta"),
];
/// As linhas do grupo "Na grade de dados".
pub(crate) const ATALHOS_GRADE: [(&str, &str); 5] = [
    ("Setas", "Mover a célula ativa"),
    ("Enter ou F2", "Editar a célula ativa"),
    ("Esc", "Desistir da edição"),
    ("Tab", "Gravar e ir para a próxima célula"),
    ("Delete (com linhas marcadas)", "Excluir as linhas marcadas (pergunta antes)"),
];

/// O título da janela: "dados · DataBox" com uma pasta aberta (a etapa 4); sem pasta, o arquivo, "vendas.db ·
/// DataBox"; sem nada, "DataBox".
pub fn titulo_da_janela(pasta: Option<&str>, arquivo: Option<&str>) -> String {
    match pasta.or(arquivo) {
        Some(nome) => format!("{nome} · {}", comum::app::NOME),
        None => comum::app::NOME.to_string(),
    }
}

/// Um recente do submenu "Abertos recentemente": o nome, a pasta, o ícone do tipo e o que o clique pede (pelo tipo:
/// a pasta, o banco ou a ficha do arquivo de dados).
pub(crate) struct RecenteMenu {
    nome: String,
    pasta: String,
    icone: interface::icones::Icone,
    acao: Acao,
}

/// Os textos da casca que dependem do estado (o menu Arquivo, o Explorador e a barra de status),
/// refeitos só quando ele muda (a `versao`).
pub(crate) struct Textos {
    versao: u64,
    /// Os recentes que existem, menos o banco aberto (abrir de novo o aberto fecharia as abas dele) e a pasta aberta.
    recentes: Vec<RecenteMenu>,
    /// O caminho do banco aberto (a dica do nó dele no Explorador).
    caminho: String,
    /// As contagens dos grupos do Explorador: Tabelas, Visões e Sistema.
    grupos: [String; 3],
    contagens_tabelas: Vec<String>,
    chaves_tabelas: Vec<String>,
    contagens_sistema: Vec<String>,
    status: status::TextosStatus,
}

impl Textos {
    fn de(versao: u64, estado: &Estado) -> Textos {
        let b = estado.banco.as_ref();
        let chave_aberto = b.map(|b| prefs::chave_caminho(&b.caminho));
        let chave_pasta = estado.pasta.as_ref().map(|p| p.chave.clone());
        let recentes = estado
            .prefs
            .recentes
            .iter()
            .filter(|r| {
                let chave = Some(prefs::chave_caminho(&r.caminho));
                r.existe && chave != chave_aberto && chave != chave_pasta
            })
            .map(|r| RecenteMenu {
                nome: crate::inicio::rotulo_do_recente(r).to_string(),
                pasta: r.pasta.clone(),
                icone: crate::inicio::icone_do_recente(r),
                acao: crate::inicio::acao_do_recente(r, false),
            })
            .collect();
        let objetos = estado.objetos.as_ref();
        let tabelas = objetos.map(|o| o.tabelas.as_slice()).unwrap_or_default();
        let visoes = objetos.map(|o| o.visoes.len()).unwrap_or_default();
        let sistema = objetos.map(|o| o.sistema.as_slice()).unwrap_or_default();
        Textos {
            versao,
            recentes,
            caminho: b.map(|b| b.caminho.display().to_string()).unwrap_or_default(),
            grupos: [formatar_numero(tabelas.len()), formatar_numero(visoes), formatar_numero(sistema.len())],
            contagens_tabelas: tabelas.iter().map(|t| formatar_numero_opt(t.linhas)).collect(),
            chaves_tabelas: tabelas.iter().map(|t| sem_acento(&t.nome)).collect(),
            contagens_sistema: sistema.iter().map(|t| formatar_numero_opt(t.linhas)).collect(),

            status: status::TextosStatus::de(estado),
        }
    }
}

/// Os textos guardados, refeitos se a `versao` mudou (uma função sobre o campo, para os outros campos da casca
/// continuarem livres enquanto os textos estão emprestados).
fn textos<'a>(guardados: &'a mut Option<Textos>, versao: u64, estado: &Estado) -> &'a Textos {
    let atuais = guardados.take().filter(|t| t.versao == versao);
    guardados.insert(atuais.unwrap_or_else(|| Textos::de(versao, estado)))
}

/// O estado da casca entre os quadros.
#[derive(Default)]
pub(crate) struct Casca {
    textos: Option<Textos>,
    explorador: explorador::Explorador,
    pub historico: historico::Historico,
    /// O botão "Ajuda" (o foco volta para ele quando a janela "Atalhos do teclado" ou a "Sobre o DataBox" fecha).
    id_ajuda: Option<Id>,
    /// Quantos quadros ainda esperar para devolver o foco ao botão "Ajuda" (0 = nada pendente).
    foco_pendente: u8,
}

impl Casca {
    /// A barra de título no espaço de `ui` (a faixa de 30px no alto da janela, ver [`janela::faixa_titulo`]): o
    /// logo, os menus e o título no centro, sem fundo próprio (a cor do fundo da janela). Com `barra_propria`, o
    /// lugar dos três botões fica reservado; quem chama desenha os controles da janela no fim do quadro.
    pub fn faixa_titulo(
        &mut self,
        ui: &mut Ui,
        estado: &Estado,
        versao: u64,
        titulo: &str,
        barra_propria: bool,
        menus: &menus::Menus<'_>,
    ) -> janela::SaidaFaixa {
        let textos = textos(&mut self.textos, versao, estado);
        let saida = janela::faixa_titulo(ui, titulo, barra_propria, menus, &textos.recentes);
        self.id_ajuda = Some(saida.id_ajuda);
        saida
    }

    /// O painel de Propriedades do documento à vista (a aba `aba`) no espaço de `ui` (ver [`propriedades`]).
    pub fn propriedades(&mut self, ui: &mut Ui, documento: &mut Documento, aba: estado::IdAba, ctx: &mut Contexto) {
        propriedades::painel(ui, documento, aba, ctx);
    }

    /// O Explorador no espaço de `ui` (ver [`explorador`]). `falha` é o motivo de a lista de objetos não ter chegado
    /// (sem ela e sem a lista, a leitura ainda está correndo); `consulta`, o nome da Consulta que abriu a transação
    /// ([`nome_da_consulta`]), para a dica do ícone de atenção do banco.
    #[allow(clippy::too_many_arguments)]
    pub fn explorador(
        &mut self,
        ui: &mut Ui,
        estado: &Estado,
        versao: u64,
        rota: &Rota,
        falha: Option<&str>,
        consulta: &str,
        acoes: &mut Vec<Acao>,
    ) {
        let textos = textos(&mut self.textos, versao, estado);
        self.explorador.mostrar(ui, estado, textos, rota, falha, consulta, acoes);
    }

    /// O "Mostrar na pasta" do menu de contexto do Explorador pedido desde a última chamada (a casca abre o
    /// Explorador de Arquivos numa tarefa; nos testes, o gancho `abrir_de_teste`).
    pub fn revelar_do_explorador(&mut self) -> Option<PathBuf> {
        self.explorador.tomar_revelar()
    }

    /// As ações que o Explorador emitiu desde a última chamada (as últimas 32), para os testes.
    pub fn acoes_do_explorador(&mut self) -> Vec<Acao> {
        self.explorador.tomar_emitidas()
    }

    /// A barra de status no espaço de `ui` (ver [`status`]): `documento` é o que a aba ativa diz de si, `consulta` o
    /// nome da Consulta que abriu a transação ([`nome_da_consulta`]) e `arquivo` o status da aba de arquivo (com ele,
    /// a barra é a do arquivo).
    pub fn status(
        &mut self,
        ui: &mut Ui,
        estado: &Estado,
        versao: u64,
        documento: Option<&str>,
        consulta: &str,
        arquivo: Option<&estado::StatusDeArquivo>,
    ) {
        let textos = textos(&mut self.textos, versao, estado);
        status::barra(ui, &textos.status, documento, consulta, arquivo);
    }

    /// Há leituras da casca rodando (o Histórico de consultas), para os testes esperarem.
    pub fn ocupada(&self) -> bool {
        self.historico.ocupado()
    }

    /// Pede que o foco volte ao botão "Ajuda" (o `aoFechar` das janelas "Atalhos do teclado" e "Sobre o DataBox":
    /// o item que as abriu estava no menu, que já fechou). O egui só deixa o botão ter foco quando a camada modal da
    /// janela some (um quadro depois de ela fechar): [`Casca::devolver_foco`] espera por isso.
    pub fn focar_menu(&mut self) {
        self.foco_pendente = 3;
    }

    /// No fim do quadro: devolve o foco ao botão "Ajuda" quando nenhuma janela modal segura mais a interação.
    pub fn devolver_foco(&mut self, ctx: &egui::Context, janela_aberta: bool) {
        if self.foco_pendente == 0 {
            return;
        }
        if janela_aberta {
            self.foco_pendente = 0;
        } else if ctx.memory(|m| m.top_modal_layer().is_none()) {
            if let Some(id) = self.id_ajuda {
                ctx.memory_mut(|m| m.request_focus(id));
            }
            self.foco_pendente = 0;
        } else {
            self.foco_pendente -= 1;
            ctx.request_repaint();
        }
    }
}

/// O aviso de transação aberta (fora da Consulta SQL): a faixa `.aviso` do topo do documento, de ponta a ponta
/// (prancheta 6), com o nome da Consulta que abriu a transação ([`nome_da_consulta`]). Devolve true quando a pessoa
/// clicou em "Ir para a Consulta N".
pub(crate) fn aviso_transacao(ui: &mut Ui, documento: &mut DocumentoPeca, consulta: &str) -> bool {
    let inicio = inicio_do_aviso(consulta);
    let ir = format!("Ir para a {consulta}");
    // o link vai na ponta direita da faixa (`margin-left:auto`, prancheta 6)
    let trechos = [Trecho::Forte(&inicio), Trecho::Normal(AVISO_TRANSACAO)];
    documento.aviso(ui, TomAviso::Atencao, |ui, cores| texto_com_link_a_direita(ui, &trechos, &ir, cores))
}

/// Uma troca de banco que está demorando: o `.carregando` com o que está acontecendo, numa faixa no topo do
/// documento (na informação). A troca espera o banco atual soltar a conexão (Consulta SQL demorada, VACUUM, cópia,
/// exportação com transação aberta), o que pode levar minutos: depois de um tempo, o texto diz isso.
pub(crate) fn aviso_troca(ui: &mut Ui, documento: &mut DocumentoPeca, texto: &str) {
    documento.aviso(ui, TomAviso::Informacao, |ui, _| {
        carregando(ui, texto);
    });
}

/// A janela "Atalhos do teclado"; devolve false quando ela fechou (✕, Esc ou clique no fundo).
pub(crate) fn janela_atalhos(ctx: &egui::Context) -> bool {
    let r = Janela::new("atalhos", "Atalhos do teclado").mostrar(ctx, |ui| {
        // A ficha empilha rótulo e valor abaixo de 480px de largura; no navegador quem decide é a largura da
        // janela inteira (`@media (max-width:480px)`), e o corpo da janela de 520px (478px por dentro) fica com
        // as duas colunas. Aqui a ficha ganha os 2px que faltam, dentro do padding de 20px do corpo.
        let largura = ui.available_width().max(480.0);
        let rect = Rect::from_min_size(ui.cursor().min, vec2(largura, f32::INFINITY));
        ui.scope_builder(UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
            detalhes(ui, |d| {
                for (tecla, texto) in ATALHOS {
                    d.linha_texto(tecla, Some(texto), "");
                }
                d.titulo_grupo("Na grade de dados");
                for (tecla, texto) in ATALHOS_GRADE {
                    d.linha_texto(tecla, Some(texto), "");
                }
            });
        });
    });
    !r.pediu_fechar
}

/// Algum dos menus da barra de título (Arquivo, Editar, Exibir, Ajuda) está aberto (para os testes).
pub fn menu_aberto(ctx: &egui::Context) -> bool {
    menus::algum_aberto(ctx)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn frase_da_transacao_cita_a_consulta() {
        assert_eq!(nome_da_consulta(Some(2)), "Consulta 2");
        assert_eq!(nome_da_consulta(None), "Consulta SQL");
        assert_eq!(inicio_do_aviso("Consulta 1"), "Há uma transação aberta na Consulta 1.");
        // o aviso inteiro (o começo em destaque e o resto) é a frase da dica do status
        assert_eq!(
            frase_da_transacao("Consulta 1"),
            "Há uma transação aberta na Consulta 1. O que for salvo agora só vai para o arquivo depois do COMMIT; fechar \
             o banco descarta tudo desde o BEGIN."
        );
    }

    #[test]
    fn titulo_com_pasta_com_arquivo_e_sem_nada() {
        assert_eq!(titulo_da_janela(Some("dados"), Some("vendas.db")), "dados · DataBox");
        assert_eq!(titulo_da_janela(None, Some("vendas.db")), "vendas.db · DataBox");
        assert_eq!(titulo_da_janela(None, None), "DataBox");
    }

    #[test]
    fn janela_de_atalhos_com_os_novos() {
        let teclas: Vec<&str> = ATALHOS.iter().map(|(t, _)| *t).collect();
        assert_eq!(
            teclas,
            [
                "Ctrl+Shift+O",
                "Ctrl+O",
                "Ctrl+S",
                "Ctrl+W",
                "Ctrl+Z",
                "Ctrl+Y",
                "Ctrl+Shift+V",
                "Ctrl+Tab e Ctrl+Shift+Tab",
                "Alt+← e Alt+→",
                "F5",
                "Ctrl+B",
                "Ctrl+Alt+B",
                "Ctrl+Enter",
                "Ctrl+F",
                "Ctrl+,",
                "Ctrl+Shift+C",
                "Esc"
            ]
        );
        // a etapa 4: a pasta, o arquivo e o Copiar caminho do Explorador
        assert!(ATALHOS.contains(&("Ctrl+Shift+O", "Abrir uma pasta")));
        assert!(ATALHOS.contains(&("Ctrl+O", "Abrir um arquivo")));
        assert!(ATALHOS.contains(&("Ctrl+Shift+C", "Copiar o caminho do item do Explorador")));
        assert!(ATALHOS.contains(&("Alt+← e Alt+→", "Voltar e avançar entre as abas")));
        assert!(ATALHOS.contains(&("Ctrl+B", "Mostrar ou esconder o Explorador")));
        // a etapa 5: os editores de arquivo
        for par in [
            ("Ctrl+S", "Salvar"),
            ("Ctrl+Z", "Desfazer"),
            ("Ctrl+Y", "Refazer"),
            ("Ctrl+Shift+V", "Colar com correspondência"),
        ] {
            assert!(ATALHOS.contains(&par), "{par:?}");
        }
    }

    #[test]
    fn recentes_do_menu_sem_o_banco_aberto_nem_a_pasta_aberta() {
        let amb = estado::apoio_testes::Ambiente::exemplo("recentes_do_menu");
        let outro = amb.pasta().join("outro.db");
        std::fs::write(&outro, b"").expect("outro");
        let sumido = amb.pasta().join("sumido.db");
        std::fs::write(&sumido, b"").expect("sumido");
        // o aberto pelo caminho com que o serviço o guardou
        let aberto = amb.estado.banco.as_ref().map(|b| b.caminho.clone()).expect("banco");
        for c in [&sumido, &outro, &aberto] {
            amb.servico.prefs().adicionar_recente(c);
        }
        std::fs::remove_file(&sumido).expect("apagar");
        let mut estado = amb.estado.clone();
        estado.prefs = amb.servico.prefs().resumo();
        let t = Textos::de(1, &estado);
        let nomes: Vec<&str> = t.recentes.iter().map(|r| r.nome.as_str()).collect();
        assert_eq!(nomes, ["outro.db"]);
        assert_eq!(t.grupos[0], "7");
        // sem banco, todos os que existem
        estado.banco = None;
        estado.objetos = None;
        let t = Textos::de(2, &estado);
        assert_eq!(t.recentes.len(), 2);
        assert_eq!(t.grupos, ["0", "0", "0"].map(String::from));
        // a pasta aberta também sai; a de outra pasta fica, com o ícone e a ação do tipo
        let dados = amb.pasta().join("dados");
        std::fs::create_dir_all(&dados).expect("a pasta");
        amb.servico.prefs().adicionar_recente_de(&dados, prefs::TipoRecente::Pasta);
        estado.prefs = amb.servico.prefs().resumo();
        let t = Textos::de(3, &estado);
        let pasta = t.recentes.iter().find(|r| r.nome == "dados").expect("o recente da pasta");
        assert_eq!(pasta.icone, interface::icones::Icone::PastaSolida);
        assert_eq!(pasta.acao, Acao::AbrirPasta { caminho: dados.clone() });
        estado.pasta = Some(estado::PastaAberta::nova(&dados));
        let t = Textos::de(4, &estado);
        assert!(!t.recentes.iter().any(|r| r.nome == "dados"), "a pasta aberta não aparece");
    }
}
