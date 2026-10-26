//! A seção "Tabelas" das Propriedades da Consulta (Q1 da 0.3): as tabelas e visões do banco, cada uma abrindo as
//! colunas (a pastilha do tipo, como nos cabeçalhos dos Dados, com a chave primária e a estrangeira; à direita, o
//! tipo declarado ou "→ tabela.coluna"), com o filtro "Filtrar tabelas e colunas" no alto.
//!
//! - O clique na tabela insere o nome dela no cursor do editor (como antes); a seta (ou → e ←) abre e fecha.
//! - O clique na coluna insere o nome dela; com Shift, `tabela.coluna`. Os nomes vão citados quando precisam.
//! - O filtro (sem acento nem caixa) mostra as tabelas cujo nome casa e as que têm colunas que casam; estas se abrem
//!   sozinhas, só com as colunas que casam. Ao limpar o filtro, volta o que estava aberto antes dele.
//! - O que está aberto vale para o banco aberto (outro banco começa tudo fechado).
//!
//! A lista de tabelas vem do estado da casca (`ObjetosInfo`, já lido); as colunas, do [`ColunasDoBanco`] da tela,
//! lido numa tarefa. Antes de as colunas chegarem, a tabela aberta fica vazia e o filtro só acha nomes de tabela.

use std::collections::HashSet;
use std::sync::Arc;

use banco::{ColunaResumo, TabelaColunas};
use egui::{Id, Margin, Ui};
use estado::ColunasDoBanco;
use interface::codigo::citar as citar_nome;
use interface::formato::sem_acento;
use interface::icones::{Icone, TipoColuna};
use interface::pecas::arvore::{No, Seta};
use interface::pecas::campos::Entrada;
use interface::pecas::texto::paragrafo;
use interface::tema::paleta;

/// A dica (e o rótulo) do filtro da seção.
pub const DICA_FILTRO: &str = "Filtrar tabelas e colunas";
/// A tabela aberta sem colunas visíveis (uma tabela virtual só com colunas ocultas, uma visão quebrada).
pub const SEM_COLUNAS: &str = "Sem colunas visíveis.";
/// O filtro que não acha nada.
pub const NADA_NO_FILTRO: &str = "Nenhuma tabela ou coluna corresponde ao filtro.";

/// O recuo das tabelas (o de antes) e o das colunas, um nível (14px) adentro, com o espaço da seta.
const RECUO_TABELA: f32 = 12.0;
const RECUO_COLUNA: f32 = RECUO_TABELA + 14.0;

/// Uma tabela ou visão à vista e as colunas que ficam abertas nela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Visivel {
    /// O índice na lista de objetos.
    pub indice: usize,
    /// As colunas que o filtro achou (índices nas colunas da tabela, na ordem dela); None: todas (sem filtro, ou
    /// só o nome da tabela casou).
    pub colunas: Option<Vec<usize>>,
}

/// O que o filtro deixa à vista: `chaves_nomes` são os nomes dos objetos e `chaves_colunas(i)` os das colunas do
/// objeto `i` (None quando ainda não chegaram), todos já sem acento e em minúsculas. Sem termo, tudo.
pub(crate) fn filtrar<'a>(
    chaves_nomes: &[String],
    chaves_colunas: impl Fn(usize) -> Option<&'a [String]>,
    termo: &str,
) -> Vec<Visivel> {
    let termo = sem_acento(termo.trim());
    if termo.is_empty() {
        return (0..chaves_nomes.len()).map(|indice| Visivel { indice, colunas: None }).collect();
    }
    let mut saida = Vec::new();
    for (indice, nome) in chaves_nomes.iter().enumerate() {
        let colunas: Vec<usize> = chaves_colunas(indice)
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter(|(_, c)| c.contains(termo.as_str()))
            .map(|(i, _)| i)
            .collect();
        if !colunas.is_empty() {
            saida.push(Visivel { indice, colunas: Some(colunas) });
        } else if nome.contains(termo.as_str()) {
            saida.push(Visivel { indice, colunas: None });
        }
    }
    saida
}

/// O texto à direita da coluna: "→ clientes.id" na chave estrangeira, senão o tipo declarado ("" sem tipo).
pub(crate) fn lado_direito(col: &ColunaResumo) -> String {
    match &col.fk {
        Some(r) => match &r.coluna {
            Some(c) => format!("→ {}.{c}", r.tabela),
            None => format!("→ {}", r.tabela),
        },
        None => col.tipo.clone(),
    }
}

/// O que a coluna insere no editor: o nome citado, ou `tabela.coluna` (com Shift).
pub(crate) fn texto_da_coluna(tabela: &str, coluna: &str, qualificado: bool) -> String {
    if qualificado { format!("{}.{}", citar_nome(tabela), citar_nome(coluna)) } else { citar_nome(coluna) }
}

/// As colunas do cache e as chaves de busca delas (uma lista por tabela, na ordem do cache).
type ChavesDasColunas = (Arc<Vec<TabelaColunas>>, Vec<Vec<String>>);

/// O estado da seção: o filtro e o que está aberto.
#[derive(Default)]
pub(crate) struct DocaTabelas {
    /// O banco de onde é o que está aberto ("" antes do primeiro quadro).
    banco_id: String,
    pub filtro: String,
    /// As tabelas abertas (pelo nome).
    abertas: HashSet<String>,
    /// As abertas antes de o filtro começar, que voltam quando ele é limpo.
    antes_do_filtro: Option<HashSet<String>>,
    /// O termo e as colunas de onde saiu a última abertura automática.
    aplicado: (String, Option<usize>),
    /// As chaves (sem acento, minúsculas) das colunas de cada tabela do cache, refeitas só quando ele muda.
    chaves_colunas: Option<ChavesDasColunas>,
}

impl DocaTabelas {
    /// A tabela está aberta (para os testes e a casca).
    pub fn aberta(&self, nome: &str) -> bool {
        self.abertas.contains(nome)
    }

    /// Desenha o conteúdo da seção (o filtro e a árvore) e devolve o texto a inserir no editor, se houve clique.
    /// `objetos` são as tabelas e as visões, com o ícone de cada uma, na ordem da lista.
    pub fn mostrar(
        &mut self,
        ui: &mut Ui,
        id_filtro: Id,
        banco_id: &str,
        objetos: &[(&str, Icone)],
        colunas: &ColunasDoBanco,
    ) -> Option<String> {
        if self.banco_id != banco_id {
            *self = DocaTabelas { banco_id: banco_id.to_string(), ..DocaTabelas::default() };
        }
        let e = paleta(ui).estudio;
        egui::Frame::NONE.inner_margin(Margin { left: 14, right: 14, top: 2, bottom: 6 }).show(ui, |ui| {
            let _ = Entrada::new(&mut self.filtro)
                .busca(true)
                .dica(DICA_FILTRO)
                .rotulo(DICA_FILTRO)
                .id_salt(id_filtro)
                .mostrar(ui);
        });

        // as colunas de cada objeto, pelo nome (o cache segue a ordem dele, que pode não ser a da lista)
        let cache = colunas.compartilhadas();
        if let Some(c) = &cache
            && self.chaves_colunas.as_ref().is_none_or(|(a, _)| !Arc::ptr_eq(a, c))
        {
            let chaves = c.iter().map(|t| t.colunas.iter().map(|col| sem_acento(&col.nome)).collect()).collect();
            self.chaves_colunas = Some((Arc::clone(c), chaves));
        }
        let no_cache: Vec<Option<usize>> = objetos
            .iter()
            .map(|(nome, _)| {
                let tabelas = cache.as_deref()?;
                tabelas
                    .iter()
                    .position(|t| t.nome == *nome)
                    .or_else(|| tabelas.iter().position(|t| t.nome.eq_ignore_ascii_case(nome)))
            })
            .collect();
        let chaves_nomes: Vec<String> = objetos.iter().map(|(n, _)| sem_acento(n)).collect();
        let chaves = self.chaves_colunas.as_ref().map(|(_, c)| c);
        let visiveis =
            filtrar(&chaves_nomes, |i| Some(chaves?.get(no_cache.get(i).copied()??)?.as_slice()), &self.filtro);
        self.abrir_pelo_filtro(objetos, &visiveis, cache.as_ref().map(|c| Arc::as_ptr(c) as usize));

        if visiveis.is_empty() {
            egui::Frame::NONE.inner_margin(Margin { left: 14, right: 14, top: 2, bottom: 12 }).show(ui, |ui| {
                paragrafo(ui, NADA_NO_FILTRO, 12.0, e.t3);
            });
            return None;
        }
        let mut inserir = None;
        for v in &visiveis {
            let Some(&(nome, icone)) = objetos.get(v.indice) else { continue };
            let aberta = self.abertas.contains(nome);
            let dica = format!("Inserir {} no comando", citar_nome(nome));
            // o rótulo diz o que o clique faz (e não repete o nó do Explorador)
            let r = No::new(nome)
                .rotulo(&dica)
                .recuo(RECUO_TABELA)
                .seta(if aberta { Seta::Aberta } else { Seta::Fechada })
                .icone(icone)
                .icone_a_direita(Icone::Inserir)
                .dica(&dica)
                .mostrar(ui);
            if r.alternou {
                if aberta {
                    self.abertas.remove(nome);
                } else {
                    self.abertas.insert(nome.to_string());
                }
            } else if r.resposta.clicked() {
                inserir = Some(citar_nome(nome));
            }
            if !aberta {
                continue;
            }
            let tabela = no_cache.get(v.indice).copied().flatten().and_then(|i| cache.as_deref()?.get(i));
            let Some(tabela) = tabela else {
                // ainda lendo (ou a leitura falhou: a frase dela, uma vez, embaixo da tabela)
                if let Some(erro) = colunas.erro() {
                    recuado(ui, |ui| {
                        paragrafo(ui, erro, 12.0, e.perigo);
                    });
                }
                continue;
            };
            if tabela.colunas.is_empty() {
                recuado(ui, |ui| {
                    paragrafo(ui, SEM_COLUNAS, 12.0, e.t3);
                });
                continue;
            }
            let indices: Vec<usize> = match &v.colunas {
                Some(c) => c.clone(),
                None => (0..tabela.colunas.len()).collect(),
            };
            ui.push_id(("consulta::colunas", nome), |ui| {
                for i in indices {
                    let Some(col) = tabela.colunas.get(i) else { continue };
                    if let Some(texto) = coluna(ui, nome, col) {
                        inserir = Some(texto);
                    }
                }
            });
        }
        // `padding:0 0 10px`
        ui.add_space(10.0);
        inserir
    }

    /// Com o filtro novo (ou as colunas recém-chegadas), abre só as tabelas em que ele achou colunas; ao limpá-lo,
    /// volta o que estava aberto antes.
    fn abrir_pelo_filtro(&mut self, objetos: &[(&str, Icone)], visiveis: &[Visivel], cache: Option<usize>) {
        let termo = self.filtro.trim().to_string();
        if (termo.as_str(), cache) == (self.aplicado.0.as_str(), self.aplicado.1) {
            return;
        }
        let mudou_termo = termo != self.aplicado.0;
        self.aplicado = (termo.clone(), cache);
        if termo.is_empty() {
            if let Some(antes) = self.antes_do_filtro.take() {
                self.abertas = antes;
            }
            return;
        }
        if !mudou_termo && self.antes_do_filtro.is_none() {
            return;
        }
        if self.antes_do_filtro.is_none() {
            self.antes_do_filtro = Some(self.abertas.clone());
        }
        self.abertas = visiveis
            .iter()
            .filter(|v| v.colunas.is_some())
            .filter_map(|v| objetos.get(v.indice).map(|(n, _)| n.to_string()))
            .collect();
    }
}

/// Uma coluna da tabela aberta: a pastilha, o nome e, à direita, o tipo ou a referência. Devolve o texto a inserir
/// quando clicada.
fn coluna(ui: &mut Ui, tabela: &str, col: &ColunaResumo) -> Option<String> {
    let pastilha = TipoColuna::do_declarado(&col.tipo, col.pk > 0, col.fk.is_some());
    let direita = lado_direito(col);
    let rotulo = format!("Inserir {} no comando", citar_nome(&col.nome));
    let dica = format!("{rotulo}; com Shift, {}", texto_da_coluna(tabela, &col.nome, true));
    let mut no = No::new(&col.nome).rotulo(&rotulo).recuo(RECUO_COLUNA).seta(Seta::Vazia).icone(Icone::Tipo(pastilha));
    if !direita.is_empty() {
        no = no.contagem(&direita);
    }
    let r = no.dica(&dica).mostrar(ui);
    r.resposta.clicked().then(|| {
        let shift = ui.input(|i| i.modifiers.shift);
        texto_da_coluna(tabela, &col.nome, shift)
    })
}

/// Um parágrafo alinhado com as colunas.
fn recuado(ui: &mut Ui, conteudo: impl FnOnce(&mut Ui)) {
    let esquerda = (6.0 + RECUO_COLUNA + 16.0) as i8;
    egui::Frame::NONE.inner_margin(Margin { left: esquerda, right: 14, top: 2, bottom: 6 }).show(ui, conteudo);
}

#[cfg(test)]
mod testes {
    use super::*;
    use banco::Referencia;

    fn chaves(nomes: &[&str]) -> Vec<String> {
        nomes.iter().map(|n| sem_acento(n)).collect()
    }

    #[test]
    fn sem_filtro_mostra_tudo_com_todas_as_colunas() {
        let nomes = chaves(&["clientes", "pedidos"]);
        let v = filtrar(&nomes, |_| None, "  ");
        assert_eq!(v, vec![Visivel { indice: 0, colunas: None }, Visivel { indice: 1, colunas: None }]);
    }

    #[test]
    fn filtro_acha_colunas_sem_acento_nem_caixa_e_nomes_de_tabela() {
        let nomes = chaves(&["clientes", "pedidos", "Endereços"]);
        let cols = [chaves(&["id", "Email", "nome"]), chaves(&["id", "cliente_id"]), chaves(&["rua"])];
        let por = |i: usize| cols.get(i).map(Vec::as_slice);
        // a coluna em duas tabelas: só elas, só com as colunas que casam
        assert_eq!(filtrar(&nomes, por, "ÉMAIL"), vec![Visivel { indice: 0, colunas: Some(vec![1]) }]);
        assert_eq!(
            filtrar(&nomes, por, "id"),
            vec![Visivel { indice: 0, colunas: Some(vec![0]) }, Visivel { indice: 1, colunas: Some(vec![0, 1]) }]
        );
        // o nome da tabela, sem acento: todas as colunas
        assert_eq!(filtrar(&nomes, por, "enderecos"), vec![Visivel { indice: 2, colunas: None }]);
        // as colunas ainda não lidas: só os nomes
        assert_eq!(filtrar(&nomes, |_| None, "email"), vec![]);
    }

    #[test]
    fn lado_direito_e_texto_inserido() {
        let mut col = ColunaResumo { nome: "cliente id".into(), tipo: "INTEGER".into(), pk: 0, fk: None };
        assert_eq!(lado_direito(&col), "INTEGER");
        col.fk = Some(Referencia { tabela: "clientes".into(), coluna: Some("id".into()) });
        assert_eq!(lado_direito(&col), "→ clientes.id");
        col.fk = Some(Referencia { tabela: "clientes".into(), coluna: None });
        assert_eq!(lado_direito(&col), "→ clientes");
        assert_eq!(texto_da_coluna("pedidos", "cliente id", false), "\"cliente id\"");
        assert_eq!(texto_da_coluna("meus pedidos", "id", true), "\"meus pedidos\".id");
    }
}
