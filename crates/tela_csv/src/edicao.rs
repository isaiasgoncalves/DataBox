//! As alterações do CSV (passo 11): o que cada uma faz na [`Tabela`] (dentro da tarefa), o rótulo que ela deixa no
//! desfazer ("Desfazer Excluir 2 linhas") e como ela move as linhas marcadas (decisão 12: as marcas são posições, e a
//! tela as ajusta a cada operação que conhece; desfazer, refazer e reler limpam).

use std::collections::BTreeSet;
use std::sync::Arc;

use comum::{ErroBanco, Resultado};
use interface::formato::{formatar_numero, plural};
use tabular::csv::Tabela;

/// Uma alteração da tabela. As linhas e colunas são posições na tabela (não na vista filtrada).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Mutacao {
    Celula {
        linha: u64,
        coluna: usize,
        texto: String,
    },
    /// Uma linha vazia na posição.
    NovaLinha {
        posicao: u64,
    },
    /// Uma cópia de cada linha logo abaixo dela.
    Duplicar {
        linhas: Vec<u64>,
    },
    Excluir {
        linhas: Vec<u64>,
    },
    /// Uma coluna vazia na posição (a Nova coluna confirmada).
    NovaColuna {
        posicao: usize,
        nome: String,
    },
    Renomear {
        coluna: usize,
        nome: String,
    },
    ExcluirColunas {
        colunas: Vec<usize>,
    },
    /// Colunas coladas na posição (a primeira linha colada na primeira linha da tabela), com `acrescentar` linhas no
    /// fim antes (o "Acrescentar 24 linhas").
    ColarColunas {
        posicao: usize,
        colunas: Arc<Vec<(String, Vec<String>)>>,
        acrescentar: u64,
    },
    /// Os valores sobre uma coluna que existe (o "Colar sobre esta coluna").
    ColarSobre {
        coluna: usize,
        valores: Arc<Vec<String>>,
        acrescentar: u64,
    },
    /// O Trazer colunas (passo 13): as colunas da origem casadas pela chave, num passo só.
    Trazer(crate::janela_trazer::PlanoTrazer),
    Desfazer,
    Refazer,
}

impl Mutacao {
    /// O passo no desfazer ("Desfazer {rótulo}"); `None` no próprio desfazer e refazer.
    pub(crate) fn rotulo(&self) -> Option<String> {
        Some(match self {
            Mutacao::Celula { .. } => "Editar a célula".to_string(),
            Mutacao::NovaLinha { .. } => "Nova linha".to_string(),
            Mutacao::Duplicar { linhas } => quantas_linhas("Duplicar", linhas.len()),
            Mutacao::Excluir { linhas } => quantas_linhas("Excluir", linhas.len()),
            Mutacao::NovaColuna { .. } => "Nova coluna".to_string(),
            Mutacao::Renomear { .. } => "Renomear a coluna".to_string(),
            Mutacao::ExcluirColunas { colunas } if colunas.len() == 1 => "Excluir a coluna".to_string(),
            Mutacao::ExcluirColunas { colunas } => format!("Excluir {} colunas", colunas.len()),
            Mutacao::ColarColunas { colunas, .. } if colunas.len() == 1 => "Colar a coluna".to_string(),
            Mutacao::ColarColunas { colunas, .. } => format!("Colar {} colunas", colunas.len()),
            Mutacao::ColarSobre { .. } => "Colar sobre a coluna".to_string(),
            Mutacao::Trazer(plano) => crate::trazer::rotulo_trazer(plano.colunas.len()),
            Mutacao::Desfazer | Mutacao::Refazer => return None,
        })
    }

    /// Mexe nas posições das linhas ou das colunas que já existem (insere ou tira no meio; o desfazer e o refazer
    /// podem fazer qualquer coisa). Enquanto uma destas espera na fila ou roda, a página e as marcas em mãos são de
    /// antes dela, e uma alteração pedida por posição cairia no lugar errado. Editar células, renomear e o que só
    /// acrescenta no fim (as linhas da cola, as colunas do Trazer) não mexem.
    pub(crate) fn muda_posicoes(&self) -> bool {
        match self {
            Mutacao::Celula { .. } | Mutacao::Renomear { .. } | Mutacao::ColarSobre { .. } | Mutacao::Trazer(_) => {
                false
            }
            Mutacao::NovaLinha { .. }
            | Mutacao::Duplicar { .. }
            | Mutacao::Excluir { .. }
            | Mutacao::NovaColuna { .. }
            | Mutacao::ExcluirColunas { .. }
            | Mutacao::ColarColunas { .. }
            | Mutacao::Desfazer
            | Mutacao::Refazer => true,
        }
    }
}

/// "Excluir a linha" / "Excluir 2 linhas".
fn quantas_linhas(verbo: &str, n: usize) -> String {
    if n == 1 { format!("{verbo} a linha") } else { format!("{verbo} {}", plural(n as u64, "linha", "linhas")) }
}

/// Aplica a alteração (um passo só no desfazer). Devolve se a tabela mudou, isto é, se o desfazer andou (um passo a
/// mais, ou a menos no Desfazer): a cola por cima e o Trazer com os mesmos valores, e os que não acham o que fazer,
/// não deixam passo, e a tela não pode empilhar um rótulo para eles (os rótulos andam em paralelo com o núcleo).
pub(crate) fn aplicar(t: &mut Tabela, m: Mutacao) -> Resultado<bool> {
    let antes = (t.alteracoes(), t.passos_a_refazer());
    let mudou = aplicar_sem_conferir(t, m)?;
    Ok(mudou && (t.alteracoes(), t.passos_a_refazer()) != antes)
}

fn aplicar_sem_conferir(t: &mut Tabela, m: Mutacao) -> Resultado<bool> {
    match m {
        Mutacao::Celula { linha, coluna, texto } => t.editar_celula(linha, coluna, &texto),
        Mutacao::NovaLinha { posicao } => {
            t.inserir_linha(posicao.min(t.linhas()))?;
            Ok(true)
        }
        Mutacao::Duplicar { mut linhas } => {
            linhas.sort_unstable();
            linhas.dedup();
            if linhas.is_empty() {
                return Ok(false);
            }
            t.em_um_passo(|t| {
                // de baixo para cima: as posições de cima não mudam com as cópias de baixo
                for &l in linhas.iter().rev() {
                    let valores = t
                        .pagina(l, 1)?
                        .into_iter()
                        .next()
                        .ok_or_else(|| ErroBanco::novo(format!("A linha {} não existe.", l + 1)))?;
                    t.inserir_linha(l + 1)?;
                    for (c, v) in valores.iter().enumerate().filter(|(_, v)| !v.is_empty()) {
                        t.editar_celula(l + 1, c, v)?;
                    }
                }
                Ok(())
            })?;
            Ok(true)
        }
        Mutacao::Excluir { linhas } => {
            if linhas.is_empty() {
                return Ok(false);
            }
            t.excluir_linhas(&linhas)?;
            Ok(true)
        }
        Mutacao::NovaColuna { posicao, nome } => {
            t.inserir_coluna(posicao.min(t.colunas()), &nome)?;
            Ok(true)
        }
        Mutacao::Renomear { coluna, nome } => {
            if t.nome_da_coluna(coluna) == Some(nome.as_str()) {
                return Ok(false);
            }
            t.renomear_coluna(coluna, &nome)?;
            Ok(true)
        }
        Mutacao::ExcluirColunas { colunas } => {
            if colunas.is_empty() {
                return Ok(false);
            }
            t.excluir_colunas(&colunas)?;
            Ok(true)
        }
        Mutacao::ColarColunas { posicao, colunas, acrescentar } => {
            t.em_um_passo(|t| {
                if acrescentar > 0 {
                    t.acrescentar_linhas(acrescentar)?;
                }
                let posicao = posicao.min(t.colunas());
                for (k, (nome, valores)) in colunas.iter().enumerate() {
                    t.inserir_coluna_com_valores(posicao + k, nome, valores)?;
                }
                Ok(())
            })?;
            Ok(true)
        }
        Mutacao::ColarSobre { coluna, valores, acrescentar } => {
            t.em_um_passo(|t| {
                if acrescentar > 0 {
                    t.acrescentar_linhas(acrescentar)?;
                }
                let todos: Vec<Option<String>> = valores.iter().map(|v| Some(v.clone())).collect();
                t.preencher_coluna(coluna, &todos)
            })?;
            Ok(true)
        }
        Mutacao::Trazer(plano) => crate::trazer::aplicar_plano(t, &plano),
        Mutacao::Desfazer => Ok(t.desfazer()),
        Mutacao::Refazer => Ok(t.refazer()),
    }
}

/// As linhas marcadas depois da alteração (posições na tabela): inserir empurra as de baixo, excluir tira as
/// excluídas e sobe as de baixo; desfazer e refazer mudam a lista de linhas sem a tela saber, e limpam.
pub(crate) fn ajustar_marcas(marcas: &BTreeSet<u64>, m: &Mutacao) -> BTreeSet<u64> {
    match m {
        Mutacao::Celula { .. }
        | Mutacao::NovaColuna { .. }
        | Mutacao::Renomear { .. }
        | Mutacao::ExcluirColunas { .. }
        | Mutacao::ColarColunas { .. }
        | Mutacao::ColarSobre { .. } => marcas.clone(),
        Mutacao::NovaLinha { posicao } => marcas.iter().map(|&p| if p >= *posicao { p + 1 } else { p }).collect(),
        Mutacao::Duplicar { linhas } => {
            let mut linhas = linhas.clone();
            linhas.sort_unstable();
            linhas.dedup();
            marcas.iter().map(|&p| p + linhas.iter().filter(|&&l| l < p).count() as u64).collect()
        }
        Mutacao::Excluir { linhas } => {
            let fora: BTreeSet<u64> = linhas.iter().copied().collect();
            marcas.iter().filter(|p| !fora.contains(p)).map(|&p| p - fora.range(..p).count() as u64).collect()
        }
        // o Trazer também limpa (a tela marca as sem par depois, se a pessoa pedir)
        Mutacao::Desfazer | Mutacao::Refazer | Mutacao::Trazer(_) => BTreeSet::new(),
    }
}

/// O alvo de Duplicar e Excluir em palavras, para a dica: "as 2 linhas marcadas", "a linha 7".
pub(crate) fn alvo_em_palavras(marcadas: usize, linha_ativa: Option<u64>) -> Option<String> {
    match (marcadas, linha_ativa) {
        (0, Some(p)) => Some(format!("a linha {}", formatar_numero(p + 1))),
        (0, None) => None,
        (1, _) => Some("a linha marcada".to_string()),
        (n, _) => Some(format!("as {} linhas marcadas", formatar_numero(n as u64))),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn set(v: &[u64]) -> BTreeSet<u64> {
        v.iter().copied().collect()
    }

    #[test]
    fn marcas_acompanham_inserir_duplicar_e_excluir() {
        let m = set(&[1, 4, 7]);
        assert_eq!(ajustar_marcas(&m, &Mutacao::NovaLinha { posicao: 4 }), set(&[1, 5, 8]));
        // a original fica marcada (a cópia vem logo abaixo dela)
        assert_eq!(ajustar_marcas(&m, &Mutacao::Duplicar { linhas: vec![4, 1] }), set(&[1, 5, 9]));
        assert_eq!(ajustar_marcas(&m, &Mutacao::Excluir { linhas: vec![1, 2] }), set(&[2, 5]));
        assert!(ajustar_marcas(&m, &Mutacao::Desfazer).is_empty());
        assert_eq!(ajustar_marcas(&m, &Mutacao::Celula { linha: 1, coluna: 0, texto: "x".into() }), m);
    }

    #[test]
    fn mutacao_sem_passo_no_desfazer_nao_conta_como_alterou() {
        let pasta = std::env::temp_dir().join(format!("tela_csv-edicao-sem-passo-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let caminho = pasta.join("t.csv");
        std::fs::write(&caminho, "a;b\n1;x\n2;y\n").expect("gravar");
        let mut t = Tabela::abrir(&caminho, &tabular::csv::OpcoesAbertura::default()).expect("abrir");
        let colar =
            || Mutacao::ColarSobre { coluna: 1, valores: Arc::new(vec!["X".into(), "Y".into()]), acrescentar: 0 };
        assert!(aplicar(&mut t, colar()).expect("colar"));
        // a mesma cola de novo não deixa passo: nada de rótulo
        assert!(!aplicar(&mut t, colar()).expect("colar de novo"));
        assert_eq!(t.alteracoes(), 1);
        assert!(aplicar(&mut t, Mutacao::Desfazer).expect("desfazer"));
        assert!(!aplicar(&mut t, Mutacao::Desfazer).expect("nada a desfazer"));
        assert!(aplicar(&mut t, Mutacao::Refazer).expect("refazer"));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn so_inserir_e_tirar_no_meio_mudam_posicoes() {
        assert!(Mutacao::Excluir { linhas: vec![1] }.muda_posicoes());
        assert!(Mutacao::NovaLinha { posicao: 0 }.muda_posicoes());
        assert!(Mutacao::Desfazer.muda_posicoes());
        assert!(!Mutacao::Celula { linha: 1, coluna: 0, texto: "x".into() }.muda_posicoes());
        assert!(!Mutacao::Renomear { coluna: 0, nome: "x".into() }.muda_posicoes());
    }

    #[test]
    fn rotulos_do_desfazer() {
        assert_eq!(Mutacao::Excluir { linhas: vec![3] }.rotulo().as_deref(), Some("Excluir a linha"));
        assert_eq!(Mutacao::Duplicar { linhas: vec![3, 4] }.rotulo().as_deref(), Some("Duplicar 2 linhas"));
        assert_eq!(Mutacao::Desfazer.rotulo(), None);
        assert_eq!(alvo_em_palavras(2, Some(5)).as_deref(), Some("as 2 linhas marcadas"));
        assert_eq!(alvo_em_palavras(0, Some(6)).as_deref(), Some("a linha 7"));
        assert_eq!(alvo_em_palavras(0, None), None);
    }
}
