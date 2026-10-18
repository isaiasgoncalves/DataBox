//! A estrutura do documento e o caminho do cursor (a migalha da faixa).
//!
//! A estrutura é uma árvore **sob demanda**: só os nós abertos são desenhados, e um objeto ou lista com mais de 200
//! filhos mostra os 200 primeiros e a linha "mais 1.800 itens", que mostra os 200 seguintes ao clique. Sem recursão
//! (uma pilha de nós): um documento de milhares de níveis não estoura a pilha. O nó que o cursor está é o ativo, e os
//! pais dele se abrem quando o cursor muda de nó. Com o JSON inválido, é a última estrutura válida, esmaecida.

use ::texto::json::{Arvore, FILHOS_POR_PAGINA, NoId, Tipo, mais_itens_em_texto};
use egui::{Margin, Ui};
use interface::icones::{Icone, TipoColuna};
use interface::pecas::arvore::{No, Seta};
use interface::pecas::painel::Secao;
use interface::pecas::texto::{galley_tit, paragrafo};
use interface::tema::paleta;

use crate::json::EstadoJson;

/// Separa os nomes de um caminho na chave de um nó (um caractere que não aparece num nome de verdade).
const SEPARADOR: char = '\u{1f}';

/// O recuo do nó que abre (`padding-left` do `.no` da prancheta 8: 10, 24, 38…) e o do que não abre (mais 16, o lugar
/// da seta e o espaço dela: 40, 54, 68…).
fn recuo_do_no(nivel: usize, abre: bool) -> f32 {
    let base = 10.0 + 14.0 * nivel as f32;
    if abre { base } else { base + 16.0 }
}

/// A pastilha do nó pelo tipo do valor: objeto `{}`, lista `[]`, texto `Aa`, inteiro `12`, número `,5`, booleano
/// `✓`, nulo `∅`.
pub(crate) fn icone_do_tipo(tipo: Tipo) -> Icone {
    Icone::Tipo(match tipo {
        Tipo::Objeto => TipoColuna::Json,
        Tipo::Lista => TipoColuna::Lista,
        Tipo::Texto => TipoColuna::Texto,
        Tipo::Inteiro => TipoColuna::Inteiro,
        Tipo::Numero => TipoColuna::Real,
        Tipo::Booleano => TipoColuna::Booleano,
        Tipo::Nulo => TipoColuna::Nulo,
    })
}

/// As chaves de cada nó de um caminho (da raiz ao mais interno): "" na raiz, depois os nomes juntos.
pub(crate) fn chaves_do_caminho(arvore: &Arvore, texto: &str, caminho: &[NoId]) -> Vec<String> {
    let mut chaves = Vec::with_capacity(caminho.len());
    let mut atual = String::new();
    for (i, &id) in caminho.iter().enumerate() {
        if i > 0 {
            atual.push(SEPARADOR);
            atual.push_str(&arvore.nome(id, texto));
        }
        chaves.push(atual.clone());
    }
    chaves
}

/// O rótulo de um nó pela chave dele: os nomes do caminho sem a raiz, "conexoes › 0 › tempo_limite" (a raiz não tem).
fn rotulo_do_caminho(chave: &str) -> Option<String> {
    chave.strip_prefix(SEPARADOR).map(|c| c.replace(SEPARADOR, " › "))
}

/// O caminho como a migalha o escreve: os nomes sem a raiz (`conexoes`, `0`, `tempo_limite`).
pub(crate) fn nomes_da_migalha(json: &EstadoJson, caminho: &[NoId]) -> Vec<String> {
    let Some(v) = &json.valida else {
        return Vec::new();
    };
    caminho.iter().skip(1).map(|&id| v.arvore.nome(id, &v.texto)).collect()
}

/// O caminho do cursor (o byte `byte` de `texto_atual`, a versão `versao`) na estrutura. Com o texto em dia com a
/// análise, é a busca exata; com o JSON inválido (ou a análise atrasada), o byte é levado para o texto da última
/// versão válida pelo começo e pelo fim em comum dos dois textos (o que ficou antes e depois da edição).
pub(crate) fn caminho_do_cursor(json: &EstadoJson, texto_atual: &str, versao: u64, byte: usize) -> Vec<NoId> {
    let Some(v) = &json.valida else {
        return Vec::new();
    };
    let exato = json.erro.is_none() && json.em_dia(versao);
    let byte = if exato { byte } else { byte_na_versao_valida(texto_atual, &v.texto, byte) };
    v.arvore.caminho_em(byte)
}

/// Leva `byte` do texto atual para o texto da versão válida: o começo em comum não muda, o fim em comum se mede
/// pela cauda, e o miolo (o que foi editado) vale o começo dele.
pub(crate) fn byte_na_versao_valida(atual: &str, valido: &str, byte: usize) -> usize {
    let (a, v) = (atual.as_bytes(), valido.as_bytes());
    let prefixo = a.iter().zip(v).take_while(|(x, y)| x == y).count();
    let limite = a.len().min(v.len()) - prefixo;
    let sufixo = a.iter().rev().zip(v.iter().rev()).take(limite).take_while(|(x, y)| x == y).count();
    let byte = byte.min(a.len());
    if byte <= prefixo {
        byte
    } else if a.len() - byte <= sufixo {
        v.len() - (a.len() - byte)
    } else {
        prefixo
    }
}

/// Uma linha da estrutura por desenhar.
enum Item {
    No { id: NoId, nivel: usize, chave: String },
    Mais { pai: String, restantes: usize, nivel: usize, rotulo: String },
}

/// A seção "Estrutura do documento" das Propriedades. `caminho` é o do cursor (o último nó é o ativo). Devolve o byte
/// para onde o cursor deve ir, se um nó foi clicado.
pub(crate) fn secao(ui: &mut Ui, sal: egui::Id, json: &mut EstadoJson, caminho: &[NoId]) -> Option<usize> {
    let mut ir = None;
    let _ = Secao::new(("propriedades::estrutura", sal), "Estrutura do documento").primeira(true).mostrar(ui, |ui| {
        ir = corpo(ui, json, caminho);
    });
    ir
}

fn corpo(ui: &mut Ui, json: &mut EstadoJson, caminho: &[NoId]) -> Option<usize> {
    let Some(valida) = json.valida.take() else {
        let frase = if json.sem_estrutura {
            "A estrutura não aparece em arquivo grande."
        } else if json.erro.is_some() {
            "O documento ainda não tem uma estrutura válida."
        } else {
            "Lendo a estrutura…"
        };
        dica_com_recuo(ui, frase);
        return None;
    };
    let invalido = json.erro.is_some();
    let e = &paleta(ui).estudio;
    if invalido {
        // N1: o título pequeno acima da árvore, que fica em `--t3`
        let g = galley_tit(ui, "Última versão válida", e.t3);
        egui::Frame::NONE.inner_margin(Margin { left: 14, right: 14, top: 0, bottom: 6 }).show(ui, |ui| {
            ui.label(g);
        });
    }
    let arvore = &valida.arvore;
    let texto: &str = &valida.texto;
    // a raiz abre sozinha; um nó ativo novo abre os pais dele
    if !json.raiz_iniciada {
        json.raiz_iniciada = true;
        json.abertos.insert(String::new());
    }
    let chaves = chaves_do_caminho(arvore, texto, caminho);
    let chave_ativa = chaves.last().cloned();
    if chave_ativa != json.ativo_anterior {
        if let Some((_, pais)) = chaves.split_last() {
            json.abertos.extend(pais.iter().cloned());
        }
        json.ativo_anterior = chave_ativa;
    }
    let ativo = caminho.last().copied();

    let mut ir = None;
    ui.scope(|ui| {
        if invalido {
            ui.multiply_opacity(0.6);
        }
        let mut pilha = vec![Item::No { id: arvore.raiz(), nivel: 0, chave: String::new() }];
        while let Some(item) = pilha.pop() {
            match item {
                Item::No { id, nivel, chave } => {
                    let no = arvore.no(id);
                    let abre = no.tipo.e_container() && no.filhos > 0;
                    let aberto = abre && json.abertos.contains(&chave);
                    let nome = arvore.nome(id, texto);
                    let rotulo = rotulo_do_caminho(&chave).unwrap_or_else(|| nome.clone());
                    let contagem = arvore.contagem(id);
                    let seta = match (abre, aberto) {
                        (false, _) => Seta::Nenhuma,
                        (true, true) => Seta::Aberta,
                        (true, false) => Seta::Fechada,
                    };
                    let mut linha = No::new(&nome)
                        .rotulo(&rotulo)
                        .recuo(recuo_do_no(nivel, abre))
                        .seta(seta)
                        .icone(icone_do_tipo(no.tipo))
                        .ativo(ativo == Some(id));
                    if let Some(c) = &contagem {
                        linha = linha.contagem(c);
                    }
                    let r = ui.push_id(id.indice(), |ui| linha.mostrar(ui)).inner;
                    if r.alternou {
                        if aberto {
                            json.abertos.remove(&chave);
                        } else {
                            json.abertos.insert(chave.clone());
                        }
                    }
                    if r.resposta.clicked() {
                        ir = Some(no.inicio);
                    }
                    if aberto {
                        let mostrados = *json.mostrados.entry(chave.clone()).or_insert(FILHOS_POR_PAGINA);
                        let filhos: Vec<NoId> = arvore.filhos(id).take(mostrados).collect();
                        let restantes = (no.filhos as usize).saturating_sub(filhos.len());
                        if restantes > 0 {
                            let rotulo = format!("{} em {}", mais_itens_em_texto(restantes), rotulo);
                            pilha.push(Item::Mais { pai: chave.clone(), restantes, nivel: nivel + 1, rotulo });
                        }
                        for filho in filhos.into_iter().rev() {
                            let mut chave_filho = chave.clone();
                            chave_filho.push(SEPARADOR);
                            chave_filho.push_str(&arvore.nome(filho, texto));
                            pilha.push(Item::No { id: filho, nivel: nivel + 1, chave: chave_filho });
                        }
                    }
                }
                Item::Mais { pai, restantes, nivel, rotulo } => {
                    let frase = mais_itens_em_texto(restantes);
                    let r = ui
                        .push_id(("mais", &pai), |ui| {
                            No::new(&frase).rotulo(&rotulo).recuo(recuo_do_no(nivel, false)).mostrar(ui)
                        })
                        .inner;
                    if r.resposta.clicked() {
                        *json.mostrados.entry(pai).or_insert(FILHOS_POR_PAGINA) += FILHOS_POR_PAGINA;
                    }
                }
            }
        }
    });
    ui.add_space(10.0);
    json.valida = Some(valida);
    ir
}

/// Uma frase em `hint` com o recuo de 14 dos lados da seção.
fn dica_com_recuo(ui: &mut Ui, frase: &str) {
    egui::Frame::NONE.inner_margin(Margin { left: 14, right: 14, top: 2, bottom: 12 }).show(ui, |ui| {
        let t3 = paleta(ui).estudio.t3;
        paragrafo(ui, frase, 11.5, t3);
    });
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recuos_da_prancheta_8() {
        // raiz 10, filho que não abre 40, nível 1 que abre 24, o filho dele que não abre 54 (a prancheta usa 38 e 68 no
        // nível 2 e 3)
        assert_eq!(recuo_do_no(0, true), 10.0);
        assert_eq!(recuo_do_no(1, false), 40.0);
        assert_eq!(recuo_do_no(1, true), 24.0);
        assert_eq!(recuo_do_no(2, true), 38.0);
        assert_eq!(recuo_do_no(3, false), 68.0);
    }

    #[test]
    fn byte_vai_para_a_versao_valida_pelo_comeco_e_pelo_fim_em_comum() {
        let valido = "{\"a\": 1, \"b\": 2}";
        // uma vírgula a menos no meio: o texto atual é 1 byte mais curto
        let atual = "{\"a\": 1 \"b\": 2}";
        let b = atual.find("\"b\"").unwrap();
        // depois da edição: pela cauda
        assert_eq!(&valido[byte_na_versao_valida(atual, valido, b)..][..3], "\"b\"");
        // antes da edição: o mesmo byte
        assert_eq!(byte_na_versao_valida(atual, valido, 2), 2);
        // textos iguais: o mesmo byte; além do fim: o fim
        assert_eq!(byte_na_versao_valida(valido, valido, 7), 7);
        assert_eq!(byte_na_versao_valida(valido, valido, 999), valido.len());
    }

    #[test]
    fn pastilha_de_cada_tipo() {
        assert_eq!(icone_do_tipo(Tipo::Objeto), Icone::Tipo(TipoColuna::Json));
        assert_eq!(icone_do_tipo(Tipo::Lista), Icone::Tipo(TipoColuna::Lista));
        assert_eq!(icone_do_tipo(Tipo::Texto), Icone::Tipo(TipoColuna::Texto));
        assert_eq!(icone_do_tipo(Tipo::Inteiro), Icone::Tipo(TipoColuna::Inteiro));
        assert_eq!(icone_do_tipo(Tipo::Numero), Icone::Tipo(TipoColuna::Real));
        assert_eq!(icone_do_tipo(Tipo::Booleano), Icone::Tipo(TipoColuna::Booleano));
        assert_eq!(icone_do_tipo(Tipo::Nulo), Icone::Tipo(TipoColuna::Nulo));
    }
}
