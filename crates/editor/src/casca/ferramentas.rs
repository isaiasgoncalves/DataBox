//! A barra de ferramentas (o `header.vidro` com o `role=toolbar` das pranchetas Main, Dados, Consulta e Inicio),
//! sobre a peça `interface::pecas::ferramentas`, na ordem fixa:
//!
//! Voltar · Avançar | Abrir pasta · Abrir arquivo · Salvar | Novo · Nova coluna · Trazer colunas · Duplicar ·
//! Excluir · Desfazer · Refazer | busca · Filtrar · Atualizar | Executar · Interromper · Nova consulta · Exportar |
//! espaço | Painel de propriedades. O Trazer colunas (etapa 5) só liga no editor de CSV; o Salvar, o Desfazer e o
//! Refazer, nos editores de arquivo (as `Ofertas` deles).
//!
//! Todo botão tem o lugar dele, ligado ou não: nenhum muda de posição ao trocar de aba nem ao executar. O
//! Interromper (depois do Executar) fica desligado e cinza fora de uma execução ([`DICA_INTERROMPER_DESLIGADO`]) e
//! vira o `.ib.risco` vermelho só enquanto a Consulta executa; o Executar ocupado fica desligado, com o arco girando
//! no lugar do ícone (prancheta 15, N3).
//!
//! - **Da casca**: Voltar e Avançar (o histórico único), Abrir pasta (a dica [`DICA_ABRIR_PASTA`], "Abrir pasta
//!   (Ctrl+Shift+O)"), Abrir arquivo, Nova consulta (desligado sem banco) e o Painel de propriedades (ligado
//!   com o painel à vista; desligado, com a dica "alargue a janela para ver", quando ele sumiu por falta de largura;
//!   desligado no Início, que não tem Propriedades: "Propriedades (não há nesta aba)").
//! - **Da aba** (`estado::Verbo`): o que o documento oferece agora (`Documento::ofertas`); o clique vira o verbo, que
//!   a casca executa no documento antes de ele se desenhar. O rótulo de acessibilidade é o da oferta ou o do verbo
//!   ("Nova tabela", "Atualizar"); a dica, a da oferta ou o rótulo com o atalho ("Exportar clientes", "Atualizar
//!   (F5)"). Os desligados também mostram a dica.
//! - A busca de 250px é a do documento: o texto que ele empresta (`estado::Documento::busca`), com a dica dele
//!   ("Buscar tabelas", "Buscar em clientes", "Buscar colunas", "Buscar no editor"); o que acontece nela vira o
//!   `estado::EventoBusca` que a casca passa ao documento, antes do conteúdo. Sem busca (o Início, os Índices e
//!   gatilhos, a Definição SQL), ela fica desligada, com o "Buscar". O Ctrl+F leva o foco a ela ([`crate::casca::atalhos`]).
//!
//! Parada, a barra não pede quadro (os halos dos botões são `animate_bool`).

use egui::{Response, Ui};
use estado::{Acao, BuscaDaBarra, EventoBusca, Ofertas, Verbo};
use interface::icones::Icone;
use interface::pecas::botao::{Botao, TipoBotao};
use interface::pecas::ferramentas::{barra_ferramentas, separador};

/// A dica do Painel de propriedades quando ele sumiu por falta de largura.
pub(crate) const DICA_PAINEL_SEM_LARGURA: &str = "Painel de propriedades (alargue a janela para ver)";
/// A dica do Painel de propriedades.
const DICA_PAINEL: &str = "Mostrar ou esconder as Propriedades (Ctrl+Alt+B)";
/// A dica do Painel de propriedades numa aba sem elas (o Início).
const DICA_SEM_PAINEL: &str = "Propriedades (não há nesta aba)";
/// A dica do Interromper fora de uma execução (desligado, no lugar dele).
pub(crate) const DICA_INTERROMPER_DESLIGADO: &str = "Interromper (só durante uma execução)";
/// A dica do Abrir pasta (a mesma do botão do cabeçalho do Explorador).
pub(crate) const DICA_ABRIR_PASTA: &str = "Abrir pasta (Ctrl+Shift+O)";

/// O botão Painel de propriedades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EstadoPainel {
    /// À vista (`.ib.on`).
    AVista,
    /// Escondido pela pessoa.
    Escondido,
    /// A pessoa quer o painel, mas o documento ficaria estreito demais: desligado, com a dica de alargar.
    SemLargura,
    /// A aba não tem Propriedades (o Início): desligado, sem mudar a escolha da pessoa (o botão, o item do Exibir e
    /// o Ctrl+Alt+B não fazem nada).
    SemPainel,
}

impl EstadoPainel {
    /// O botão (e o item do Exibir) alterna o painel: à vista ou escondido pela pessoa.
    pub(crate) fn ligado(self) -> bool {
        matches!(self, EstadoPainel::AVista | EstadoPainel::Escondido)
    }
}

/// O que a barra precisa saber a cada quadro.
pub(crate) struct Barra<'a> {
    pub ofertas: &'a Ofertas,
    pub pode_voltar: bool,
    pub pode_avancar: bool,
    pub tem_banco: bool,
    pub painel: EstadoPainel,
    /// A aba ativa (a busca da barra é dela: o foco, o cursor e a seleção do campo).
    pub aba: u64,
}

/// O que a pessoa pediu na barra.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PedidoBarra {
    /// Um pedido da casca (Voltar, Avançar, Abrir arquivo, Nova consulta), para o quadro seguinte.
    Acao(Acao),
    /// Um verbo da aba, para o documento executar agora.
    Verbo(Verbo),
    /// Mostrar ou esconder o painel de Propriedades.
    AlternarPropriedades,
}

/// O ícone de cada verbo da aba (a busca é o campo, não um botão): o `Verbo::icone`.
fn icone(verbo: Verbo) -> Icone {
    verbo.icone()
}

/// Um botão de ícone com a dica também desligado (o `title` do botão `disabled`).
fn botao<'a>(ui: &mut Ui, botao: Botao<'a>, dica: &'a str) -> Response {
    ui.add(botao.dica(dica)).on_disabled_hover_text(dica)
}

/// O botão de um verbo da aba, com o que ela oferece dele; devolve se foi clicado.
fn verbo(ui: &mut Ui, ofertas: &Ofertas, v: Verbo) -> bool {
    let oferta = ofertas.get(v);
    let rotulo = ofertas.rotulo(v);
    let dica = if v == Verbo::Interromper && !oferta.ligado {
        std::borrow::Cow::Borrowed(DICA_INTERROMPER_DESLIGADO)
    } else {
        ofertas.dica(v)
    };
    let mut b = Botao::icone(icone(v)).rotulo(&rotulo).habilitado(oferta.ligado);
    if oferta.principal {
        b = b.principal();
    }
    if oferta.marcado {
        b = b.selecionado(true);
    }
    if oferta.ocupado {
        b = b.ocupado(true);
    }
    // o Interromper é o `.ib.risco` (o botão redondo de perigo) só enquanto a Consulta executa; desligado, é cinza
    // como qualquer botão de ícone desligado
    if v == Verbo::Interromper && oferta.ligado {
        b = b.tipo(TipoBotao::Perigo);
    }
    botao(ui, b, &dica).clicked()
}

/// Desenha a barra na largura de `ui` (a altura da peça, 44px), com a busca do documento (`busca`; None desliga),
/// e devolve o que a pessoa pediu nela e o que aconteceu na busca.
pub(crate) fn barra(
    ui: &mut Ui,
    b: &Barra<'_>,
    busca: Option<BuscaDaBarra<'_>>,
) -> (Option<PedidoBarra>, Option<EventoBusca>) {
    let mut pedido = None;
    let mut alternar = false;
    let mut evento = None;
    barra_ferramentas(
        ui,
        |ui| {
            let mut casca = |ui: &mut Ui, icone: Icone, dica: &str, ligado: bool, acao: Acao| {
                if botao(ui, Botao::icone(icone).habilitado(ligado), dica).clicked() {
                    pedido = Some(PedidoBarra::Acao(acao));
                }
            };
            casca(ui, Icone::Voltar, "Voltar (Alt+←)", b.pode_voltar, Acao::Voltar);
            casca(ui, Icone::Avancar, "Avançar (Alt+→)", b.pode_avancar, Acao::Avancar);
            separador(ui);
            casca(ui, Icone::Pasta, DICA_ABRIR_PASTA, true, Acao::EscolherEAbrirPasta);
            let abrir = Acao::EscolherEAbrir { somente_leitura: false };
            casca(ui, Icone::Arquivo, "Abrir arquivo (Ctrl+O)", true, abrir);
            let mut verbos = Vec::new();
            let mut v = |ui: &mut Ui, verbo_da_aba: Verbo| {
                if verbo(ui, b.ofertas, verbo_da_aba) {
                    verbos.push(verbo_da_aba);
                }
            };
            v(ui, Verbo::Salvar);
            separador(ui);
            for verbo_da_aba in [
                Verbo::Novo,
                Verbo::NovaColuna,
                Verbo::TrazerColunas,
                Verbo::Duplicar,
                Verbo::Excluir,
                Verbo::Desfazer,
                Verbo::Refazer,
            ] {
                v(ui, verbo_da_aba);
            }
            separador(ui);
            evento = estado::busca_da_barra(ui, busca, b.aba);
            v(ui, Verbo::Filtrar);
            v(ui, Verbo::Atualizar);
            separador(ui);
            v(ui, Verbo::Executar);
            // o Interromper tem o lugar dele, logo depois do Executar, em toda aba (desligado, como a Nova coluna, e
            // vermelho só enquanto a aba o oferece: a Consulta executando, N3): aparecer só então empurraria todos
            // os botões depois dele 30px para a direita no meio da execução
            v(ui, Verbo::Interromper);
            let mut nova = false;
            if botao(ui, Botao::icone(Icone::NovaConsulta).habilitado(b.tem_banco), "Nova consulta SQL").clicked() {
                nova = true;
            }
            v(ui, Verbo::Exportar);
            if let Some(&escolhido) = verbos.first() {
                pedido = Some(PedidoBarra::Verbo(escolhido));
            } else if nova {
                pedido = Some(PedidoBarra::Acao(Acao::NovaConsulta { sql: None }));
            }
        },
        |ui| {
            let (ligado, marcado, dica) = match b.painel {
                EstadoPainel::AVista => (true, true, DICA_PAINEL),
                EstadoPainel::Escondido => (true, false, DICA_PAINEL),
                EstadoPainel::SemLargura => (false, false, DICA_PAINEL_SEM_LARGURA),
                EstadoPainel::SemPainel => (false, false, DICA_SEM_PAINEL),
            };
            alternar = botao(ui, Botao::icone(Icone::Painel).selecionado(marcado).habilitado(ligado), dica).clicked();
        },
    );
    if alternar {
        pedido = Some(PedidoBarra::AlternarPropriedades);
    }
    (pedido, evento)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cada_verbo_tem_o_seu_icone() {
        let icones: std::collections::HashSet<Icone> = Verbo::TODOS.iter().map(|&v| icone(v)).collect();
        assert_eq!(icones.len(), Verbo::TODOS.len());
        assert_eq!(icone(Verbo::NovaColuna), Icone::NovaColuna);
        // o leitor de tela ouve o nome do verbo quando a aba não dá outro
        for v in Verbo::TODOS {
            assert_eq!(icone(v).nome(), v.rotulo(), "{v:?}");
        }
    }
}
