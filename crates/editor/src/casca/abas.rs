//! A faixa de abas de documento da casca, sobre a peça `interface::pecas::abas_documento`: uma aba por documento
//! aberto, na ordem das `estado::Abas`, com o ícone, o nome e o sub ("clientes · Dados", "exemplo.db · Visão
//! geral", "Consulta 2", "clientes.csv" com a folha do tipo), o × (menos no Início quando ele é a única aba) e, no fim,
//! o "+" da Nova consulta
//! (desligado sem banco). A aba de um documento com alterações não salvas (`estado::Documento::alterado`, etapa 5)
//! mostra o ponto `--acct` no lugar do ×, que volta no hover e no foco; o × dela passa pela fila de perguntas ao
//! fechar da casca ("Salvar as alterações em…?").
//!
//! Na ponta direita, o "Todas as abas" (revisão de UI de 19/10/2026): o menu com cada aba aberta, a ativa marcada.
//!
//! O clique numa aba (ou no item dela no "Todas as abas"), no × dela, o clique do meio e o "+" viram [`Acao`] (`FocarAba`, `FecharAba`, `NovaConsulta`).
//! Quem chama tira o foco do campo em edição no mesmo quadro e aplica a ação no seguinte, como os atalhos de
//! teclado: a célula da grade em edição grava antes de a aba sair de vista. Quem chama também pinta o documento
//! embaixo (`pintar_documento`), sem o fio de cima debaixo da aba ativa (a junção que a faixa devolve).

use egui::{Rangef, Ui};
use estado::{Abas, Acao, ChaveAba, Estado, Rota};
use interface::icones::Icone;
use interface::pecas::abas_documento::{AbaDocumento, FaixaAbas};

use super::explorador::{icone_do_banco, icone_do_objeto};
use crate::documento::Documento;

/// O ícone da aba de cada documento: a casa (silhueta) no Início, o cilindro (com o cadeado no banco só leitura), a
/// ficha pelo tipo do objeto (sem rowid, virtual, visão, sistema) e a folha SQL na Consulta.
fn icone(documento: &Documento, rota: &Rota, estado: &Estado) -> Icone {
    match documento {
        Documento::Inicio(_) => Icone::Casa,
        Documento::Banco(_) => estado.banco.as_ref().map_or(Icone::Banco, icone_do_banco),
        Documento::Tabela(_) => {
            let e_visao = matches!(rota, Rota::Visao { .. });
            match rota.nome_objeto() {
                Some(nome) => icone_do_objeto(estado.objetos.as_ref(), nome, e_visao),
                None if e_visao => Icone::Visao,
                None => Icone::Tabela,
            }
        }
        Documento::Consulta(_) => Icone::Consulta,
        Documento::Arquivo(t) => t.icone(),
    }
}

/// O nome e o sub de cada aba aberta, na ordem da faixa, e se ela é a ativa.
pub(crate) fn titulos(abas: &Abas<Documento>, estado: &Estado) -> Vec<(String, Option<String>, bool)> {
    let ativa = abas.id_ativa();
    abas.iter()
        .map(|a| {
            let (nome, sub) = a.conteudo.titulo(a.rota(), a.numero_consulta(), estado);
            (nome, sub, a.id() == ativa)
        })
        .collect()
}

/// Desenha a faixa na largura de `ui` e devolve o que a pessoa pediu nela e o trecho da aba ativa sobre o documento
/// (a junção).
pub(crate) fn faixa(ui: &mut Ui, abas: &Abas<Documento>, estado: &Estado) -> (Option<Acao>, Option<Rangef>) {
    let titulos = titulos(abas, estado);
    // o Início sozinho não fecha
    let so_o_inicio = abas.len() == 1 && abas.ativa().chave() == Some(&ChaveAba::Inicio);
    let itens: Vec<AbaDocumento<'_>> = abas
        .iter()
        .zip(&titulos)
        .map(|(a, (nome, sub, _))| {
            // o ponto de alterada (etapa 5): o documento diz, a cada quadro (barato)
            let mut item = AbaDocumento::new(icone(&a.conteudo, a.rota(), estado), nome)
                .fechavel(!so_o_inicio)
                .alterada(a.conteudo.alterado())
                .com_chave(a.id().0);
            if let Some(s) = sub {
                item = item.sub(s);
            }
            item
        })
        .collect();
    let ativa = titulos.iter().position(|(_, _, ativa)| *ativa);
    let r = FaixaAbas::new("abas", &itens).ativa(ativa).nova(estado.tem_banco()).todas(true).mostrar(ui);
    let id_de = |i: usize| abas.iter().nth(i).map(|a| a.id());
    let pedido = if let Some(id) = r.fechar.and_then(id_de) {
        Some(Acao::FecharAba(Some(id)))
    } else if let Some(id) = r.clicada.or(r.escolhida).and_then(id_de).filter(|&id| id != abas.id_ativa()) {
        Some(Acao::FocarAba(id))
    } else {
        r.nova.then_some(Acao::NovaConsulta { sql: None })
    };
    (pedido, r.juncao)
}
