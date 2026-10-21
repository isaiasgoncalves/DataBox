//! Uma coluna em fluxo: os valores de uma coluna da grade, um por vez, na ordem da tabela e com as alterações por
//! cima, sem juntá-los num `Vec`. É o que as contas da coluna (vazias, mínimo, máximo, soma) precisam: num CSV de
//! milhões de linhas, a coluna inteira na memória seriam centenas de MB.

use comum::{ErroBanco, Resultado};

use super::edicoes::{ChaveLinha, Edicoes};
use super::leitor::Leitor;

/// Percorre os valores da coluna `coluna` (o número dela na grade) de todas as linhas, na ordem da tabela, com as
/// alterações das `edicoes` por cima: os mesmos valores de `Tabela::colunas_de`, sem guardá-los. As linhas
/// excluídas ficam de fora, as novas entram no lugar delas, e a coluna nova (sem origem no arquivo) nem lê o arquivo.
///
/// `quando_um_valor(valor)` recebe cada valor e devolve se continua (`false` para a passada no meio, sem erro); um
/// erro dele sobe como veio. O `ao_progresso(bytes, total)` é chamado de tempos em tempos durante a leitura do
/// arquivo e cancela do mesmo jeito (devolvendo um erro).
///
/// O `leitor` e as `edicoes` precisam ser da mesma tabela (o leitor de `Leitor::reabrir_para_leitura`, para não
/// segurar a tabela durante a passada).
pub fn percorrer_coluna(
    leitor: &mut Leitor,
    edicoes: &Edicoes,
    coluna: usize,
    ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    mut quando_um_valor: impl FnMut(&str) -> Resultado<bool>,
) -> Resultado<()> {
    let virtual_ = edicoes
        .colunas()
        .get(coluna)
        .ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", coluna.saturating_add(1))))?;
    let id = virtual_.id;
    let com_alteracoes = edicoes.tem_celulas_alteradas();
    let editada = |chave: ChaveLinha| if com_alteracoes { edicoes.celula_editada(chave, id) } else { None };
    let mut chaves = edicoes.percorrer_chaves().peekable();
    if let Some(origem) = virtual_.origem {
        let mut parou = false;
        leitor.varrer(ao_progresso, |linha, vista| {
            // as linhas novas que vêm antes desta do arquivo
            while let Some(&chave @ ChaveLinha::Nova(_)) = chaves.peek() {
                chaves.next();
                if !quando_um_valor(editada(chave).unwrap_or_default())? {
                    parou = true;
                    return Ok(false);
                }
            }
            match chaves.peek() {
                // todas as linhas da grade já passaram: o resto do arquivo foi excluído
                None => return Ok(false),
                Some(&chave @ ChaveLinha::Base(r)) if r == linha => {
                    chaves.next();
                    let continuar = match editada(chave) {
                        Some(v) => quando_um_valor(v)?,
                        None => quando_um_valor(&vista.campo(origem))?,
                    };
                    if !continuar {
                        parou = true;
                        return Ok(false);
                    }
                }
                // esta linha do arquivo foi excluída
                Some(_) => {}
            }
            Ok(true)
        })?;
        if parou {
            return Ok(());
        }
    }
    // as linhas novas do fim (ou todas, na coluna nova)
    for chave in chaves {
        if !quando_um_valor(editada(chave).unwrap_or_default())? {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::csv::{OpcoesAbertura, Tabela};
    use crate::testes_apoio::Pasta;

    fn sem_progresso(_: u64, _: u64) -> Resultado<()> {
        Ok(())
    }

    fn em_fluxo(t: &Tabela, coluna: usize) -> Vec<String> {
        let mut leitor = t.leitor().reabrir_para_leitura().unwrap();
        let mut saida = Vec::new();
        percorrer_coluna(&mut leitor, t.edicoes(), coluna, &mut sem_progresso, |v| {
            saida.push(v.to_string());
            Ok(true)
        })
        .unwrap();
        saida
    }

    /// Cada coluna em fluxo é a mesma de `Tabela::colunas_de` (que junta tudo).
    fn confere(t: &mut Tabela) {
        for c in 0..t.colunas() {
            let juntas = t.colunas_de(&[c], &mut sem_progresso).unwrap().into_iter().next().unwrap();
            assert_eq!(em_fluxo(t, c), juntas, "coluna {c}");
        }
    }

    #[test]
    fn a_coluna_em_fluxo_e_a_mesma_com_as_alteracoes() {
        let pasta = Pasta::nova("fluxo-alteracoes");
        let caminho = pasta.arquivo("t.csv", b"a;b\n1;x\n2;y\n3\n4;w\n5;v\n");
        let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
        confere(&mut t);
        assert_eq!(em_fluxo(&t, 1), ["x", "y", "", "w", "v"], "a linha curta dá vazio");
        t.editar_celula(1, 1, "Y").unwrap();
        t.inserir_linha(0).unwrap();
        t.editar_celula(0, 0, "novo").unwrap();
        t.excluir_linhas(&[3, 4]).unwrap();
        t.acrescentar_linhas(2).unwrap();
        t.inserir_coluna(1, "c").unwrap();
        t.editar_celula(2, 1, "z").unwrap();
        confere(&mut t);
        assert_eq!(em_fluxo(&t, 0), ["novo", "1", "2", "5", "", ""]);
        assert_eq!(em_fluxo(&t, 1), ["", "", "z", "", "", ""], "a coluna nova");
        assert_eq!(em_fluxo(&t, 2), ["", "x", "Y", "v", "", ""]);
        // o fim do arquivo excluído
        let ultimas = t.linhas() - 2;
        t.excluir_linhas(&[ultimas - 1, ultimas, ultimas + 1]).unwrap();
        confere(&mut t);
        assert_eq!(em_fluxo(&t, 2), ["", "x", "Y"]);
    }

    #[test]
    fn parar_no_meio_e_cancelar() {
        let pasta = Pasta::nova("fluxo-parar");
        let caminho = pasta.arquivo("t.csv", b"a\n1\n2\n3\n4\n");
        let mut t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap();
        t.acrescentar_linhas(2).unwrap();
        let mut leitor = t.leitor().reabrir_para_leitura().unwrap();
        for ate in [2, 5] {
            let mut vistos = 0;
            percorrer_coluna(&mut leitor, t.edicoes(), 0, &mut sem_progresso, |_| {
                vistos += 1;
                Ok(vistos < ate)
            })
            .unwrap();
            assert_eq!(vistos, ate, "a passada para no {ate}º valor");
        }
        let r =
            percorrer_coluna(&mut leitor, t.edicoes(), 0, &mut |_, _| Err(ErroBanco::novo("cancelado")), |_| Ok(true));
        assert!(r.is_err(), "o erro do progresso sobe");
        assert!(percorrer_coluna(&mut leitor, t.edicoes(), 9, &mut sem_progresso, |_| Ok(true)).is_err());
    }
}
