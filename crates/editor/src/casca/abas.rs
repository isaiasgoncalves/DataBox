//! A faixa de abas de documento da casca, sobre a peça `interface::pecas::abas_documento`: uma aba por documento
//! aberto, na ordem das `estado::Abas`, com o ícone, o nome e o sub ("clientes · Dados", "exemplo.db · Visão
//! geral", "Consulta 2", "clientes.csv" com a folha do tipo), o × (menos no Início quando ele é a única aba) e, no fim,
//! o "+" da Nova consulta
//! (desligado sem banco). A aba de um documento com alterações não salvas (`estado::Documento::alterado`, etapa 5)
//! mostra o ponto `--acct` no lugar do ×, que volta no hover e no foco; o × dela passa pela fila de perguntas ao
//! fechar da casca ("Salvar as alterações em…?").
//!
//! A dica de cada aba é o caminho inteiro do arquivo ou do banco (U4 da revisão de UI de 22/10/2026). Dois arquivos
//! abertos com o mesmo nome ganham a pasta-mãe na aba ([`pastas_dos_homonimos`], D1 da revisão de 26/10/2026); a
//! largura mínima, o corte no meio e a rolagem até a aba ativa são da peça.
//!
//! Na ponta direita, o "Todas as abas" (revisão de UI de 19/10/2026): o menu com cada aba aberta, a ativa marcada.
//!
//! O clique numa aba (ou no item dela no "Todas as abas"), no × dela, o clique do meio e o "+" viram [`Acao`]
//! (`FocarAba`, `FecharAba`, `NovaConsulta`); o arrasto de uma aba e o "Mover à esquerda/direita" do menu dela,
//! `MoverAba`.
//! Quem chama tira o foco do campo em edição no mesmo quadro e aplica a ação no seguinte, como os atalhos de
//! teclado: a célula da grade em edição grava antes de a aba sair de vista. Quem chama também pinta o documento
//! embaixo (`pintar_documento`), sem o fio de cima debaixo da aba ativa (a junção que a faixa devolve).

use std::path::{Component, MAIN_SEPARATOR_STR, Path};

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

/// A dica da aba: o caminho inteiro do arquivo, ou o do banco nas abas dele (a Visão geral, as tabelas e as
/// Consultas); o Início não tem (U4 da revisão de UI de 22/10/2026: o caminho saiu da faixa da Visão geral).
fn dica(documento: &Documento, estado: &Estado) -> Option<String> {
    match documento {
        Documento::Inicio(_) => None,
        Documento::Banco(_) | Documento::Tabela(_) | Documento::Consulta(_) => {
            estado.banco.as_ref().map(|b| b.caminho.display().to_string())
        }
        Documento::Arquivo(t) => Some(t.caminho().display().to_string()),
    }
}

/// A pasta-mãe de cada arquivo cujo nome outro arquivo aberto também tem: a última pasta ("dados") ou, se ela também
/// for igual, as duas últimas ("2024/dados"), e assim por diante; `None` nas outras abas. `arquivos` traz o nome e o
/// caminho das abas de arquivo, na ordem da faixa (`None` nas que não são de arquivo).
pub(crate) fn pastas_dos_homonimos(arquivos: &[Option<(&str, &Path)>]) -> Vec<Option<String>> {
    // as pastas de cima para baixo até o arquivo, sem a raiz
    let pastas = |c: &Path| -> Vec<String> {
        c.parent()
            .map(|p| {
                p.components()
                    .filter(|c| matches!(c, Component::Normal(_)))
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut saida = vec![None; arquivos.len()];
    for (i, item) in arquivos.iter().enumerate() {
        let Some((nome, caminho)) = item else { continue };
        let iguais: Vec<&Path> = arquivos
            .iter()
            .enumerate()
            .filter_map(|(k, a)| a.filter(|(n, c)| k != i && n == nome && c != caminho).map(|(_, c)| c))
            .collect();
        if iguais.is_empty() {
            continue;
        }
        let minhas = pastas(caminho);
        let outras: Vec<Vec<String>> = iguais.iter().map(|c| pastas(c)).collect();
        let cauda = |v: &[String], k: usize| v[v.len().saturating_sub(k)..].join(MAIN_SEPARATOR_STR);
        // a menor cauda de pastas que a separa de todas as outras (ou todas as pastas, se nenhuma separar)
        let k = (1..=minhas.len())
            .find(|&k| outras.iter().all(|o| cauda(o, k) != cauda(&minhas, k)))
            .unwrap_or(minhas.len());
        if k > 0 {
            saida[i] = Some(cauda(&minhas, k));
        }
    }
    saida
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
    let dicas: Vec<Option<String>> = abas.iter().map(|a| dica(&a.conteudo, estado)).collect();
    let arquivos: Vec<Option<(&str, &Path)>> = abas
        .iter()
        .zip(&titulos)
        .map(|(a, (nome, _, _))| match &a.conteudo {
            Documento::Arquivo(t) => Some((nome.as_str(), t.caminho())),
            _ => None,
        })
        .collect();
    let pastas = pastas_dos_homonimos(&arquivos);
    // o Início sozinho não fecha
    let so_o_inicio = abas.len() == 1 && abas.ativa().chave() == Some(&ChaveAba::Inicio);
    let itens: Vec<AbaDocumento<'_>> = abas
        .iter()
        .zip(&titulos)
        .zip(&dicas)
        .zip(&pastas)
        .map(|(((a, (nome, sub, _)), dica), pasta)| {
            // o ponto de alterada (etapa 5): o documento diz, a cada quadro (barato)
            let mut item = AbaDocumento::new(icone(&a.conteudo, a.rota(), estado), nome)
                .fechavel(!so_o_inicio)
                .alterada(a.conteudo.alterado())
                .com_chave(a.id().0);
            if let Some(s) = sub {
                item = item.sub(s);
            }
            if let Some(d) = dica {
                item = item.dica(d);
            }
            if let Some(p) = pasta {
                item = item.pasta(p);
            }
            item
        })
        .collect();
    let ativa = titulos.iter().position(|(_, _, ativa)| *ativa);
    let r = FaixaAbas::new("abas", &itens).ativa(ativa).nova(estado.tem_banco()).todas(true).mostrar(ui);
    let id_de = |i: usize| abas.iter().nth(i).map(|a| a.id());
    let pedido = if let Some(id) = r.fechar.and_then(id_de) {
        Some(Acao::FecharAba(Some(id)))
    } else if let Some((id, para)) = r.mover.and_then(|(de, para)| Some((id_de(de)?, para))) {
        Some(Acao::MoverAba { id, para })
    } else if let Some(id) = r.clicada.or(r.escolhida).and_then(id_de).filter(|&id| id != abas.id_ativa()) {
        Some(Acao::FocarAba(id))
    } else {
        r.nova.then_some(Acao::NovaConsulta { sql: None })
    };
    (pedido, r.juncao)
}

#[cfg(test)]
mod testes {
    use std::path::PathBuf;

    use super::*;

    fn pastas(caminhos: &[Option<&str>]) -> Vec<Option<String>> {
        let caminhos: Vec<Option<PathBuf>> = caminhos.iter().map(|c| c.map(PathBuf::from)).collect();
        let nomes: Vec<Option<String>> = caminhos
            .iter()
            .map(|c| c.as_ref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()))
            .collect();
        let arquivos: Vec<Option<(&str, &Path)>> =
            nomes.iter().zip(&caminhos).map(|(n, c)| Some((n.as_deref()?, c.as_deref()?))).collect();
        pastas_dos_homonimos(&arquivos)
    }

    #[test]
    fn nomes_iguais_ganham_a_pasta_mae() {
        let p = pastas(&[
            Some("/casa/dados/2024/vendas.csv"),
            None,
            Some("/casa/dados/2025/vendas.csv"),
            Some("/casa/dados/2025/clientes.csv"),
        ]);
        assert_eq!(p, [Some("2024".into()), None, Some("2025".into()), None]);
        // a última pasta também igual: as duas últimas
        let p = pastas(&[Some("/a/x/dados/v.csv"), Some("/b/y/dados/v.csv"), Some("/b/y/outra/v.csv")]);
        let duas = |a: &str| Some(format!("{a}{MAIN_SEPARATOR_STR}dados"));
        assert_eq!(p, [duas("x"), duas("y"), Some("outra".into())]);
        // o mesmo arquivo não é homônimo de si mesmo; um nome só não ganha nada
        assert_eq!(pastas(&[Some("/a/v.csv"), Some("/a/v.csv")]), [None, None]);
        assert_eq!(pastas(&[Some("/a/v.csv")]), [None]);
    }
}
