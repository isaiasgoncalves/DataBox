//! O Colar da grade (0.2.3, `design/planos-0.2.3.md`, A.2), nos Dados e no CSV: onde cada valor do TSV da área de
//! transferência cai ([`colagem`]) e o texto colado com o foco na grade ([`texto_colado`]). A tela confere o que pode
//! gravar e grava.

use std::ops::Range;

use egui::{Context, Event};

use super::{Intervalo, NucleoGrade};
use crate::formato::plural;

/// Onde a cola cai: as células (linha e coluna da grade, texto), na ordem do TSV, e quantas ficaram de fora (passavam
/// da última linha ou da última coluna).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Colagem {
    pub celulas: Vec<(usize, usize, String)>,
    pub de_fora: u64,
}

/// O TSV `texto` (como o Excel copia; [`tabular::colunas::ler_tsv`]) sobre a grade, a partir da célula de cima à
/// esquerda do `intervalo` (a ativa, sem intervalo), por cima do que houver. Um valor só, com um intervalo de várias
/// células, preenche o intervalo inteiro. Não cria linhas nem colunas: o que passar da última das `linhas` ou sair das
/// `colunas` (as da grade que recebem valores) fica de fora e é contado. Texto vazio: nada a colar.
pub fn colagem(texto: &str, intervalo: Intervalo, linhas: usize, colunas: Range<usize>) -> Colagem {
    let tsv = tabular::colunas::ler_tsv(texto);
    let mut c = Colagem::default();
    let dentro = |l: usize, col: usize| l < linhas && colunas.contains(&col);
    if let [linha] = tsv.as_slice()
        && let [valor] = linha.as_slice()
        && intervalo.varias()
    {
        for l in intervalo.linhas() {
            for col in intervalo.colunas() {
                if dentro(l, col) {
                    c.celulas.push((l, col, valor.clone()));
                } else {
                    c.de_fora += 1;
                }
            }
        }
        return c;
    }
    for (i, linha) in tsv.into_iter().enumerate() {
        for (j, valor) in linha.into_iter().enumerate() {
            let (l, col) = (intervalo.linha0 + i, intervalo.coluna0 + j);
            if dentro(l, col) {
                c.celulas.push((l, col, valor));
            } else {
                c.de_fora += 1;
            }
        }
    }
    c
}

/// O texto colado neste quadro (o `Event::Paste` do Ctrl+V, ou o do `RequestPaste` que o item Colar do menu pediu) com
/// o foco no corpo da grade; o último, se vieram vários.
pub fn texto_colado(ctx: &Context, nucleo: &NucleoGrade) -> Option<String> {
    if !ctx.memory(|m| m.has_focus(nucleo.id_corpo())) {
        return None;
    }
    ctx.input(|i| i.events.iter().rev().find_map(|e| if let Event::Paste(t) = e { Some(t.clone()) } else { None }))
}

/// O aviso do que a cola deixou de fora: "3 células ficaram de fora: passavam da última linha ou coluna."
pub fn frase_de_fora(n: u64) -> String {
    let verbo = if n == 1 { "ficou" } else { "ficaram" };
    let passava = if n == 1 { "passava" } else { "passavam" };
    format!("{} {verbo} de fora: {passava} da última linha ou coluna.", plural(n, "célula", "células"))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn celulas(c: &Colagem) -> Vec<(usize, usize, &str)> {
        c.celulas.iter().map(|(l, col, t)| (*l, *col, t.as_str())).collect()
    }

    #[test]
    fn cola_a_partir_da_ativa_e_conta_o_que_sobra() {
        let ativa = Intervalo::entre((1, 2), (1, 2));
        let c = colagem("a\tb\r\nc\td\r\n", ativa, 10, 2..5);
        assert_eq!(celulas(&c), [(1, 2, "a"), (1, 3, "b"), (2, 2, "c"), (2, 3, "d")]);
        assert_eq!(c.de_fora, 0);
        // a última coluna é a 4 e a última linha a 2: o "c" da coluna 5 e a linha 3 ficam de fora
        let c = colagem("a\tb\tc\nd\te\tf\ng\th\ti\n", Intervalo::entre((1, 3), (1, 3)), 3, 2..5);
        assert_eq!(celulas(&c), [(1, 3, "a"), (1, 4, "b"), (2, 3, "d"), (2, 4, "e")]);
        assert_eq!(c.de_fora, 5);
        assert_eq!(colagem("", ativa, 10, 2..5), Colagem::default());
    }

    #[test]
    fn um_valor_preenche_o_intervalo() {
        let iv = Intervalo::entre((0, 4), (2, 3));
        let c = colagem("x\r\n", iv, 2, 2..5);
        assert_eq!(celulas(&c), [(0, 3, "x"), (0, 4, "x"), (1, 3, "x"), (1, 4, "x")]);
        assert_eq!(c.de_fora, 2, "a linha 2 não existe");
        // vários valores com um intervalo: colam a partir do canto de cima à esquerda
        let c = colagem("1\n2\n", iv, 10, 2..5);
        assert_eq!(celulas(&c), [(0, 3, "1"), (1, 3, "2")]);
        // um valor com a célula só: ela
        assert_eq!(celulas(&colagem("x", Intervalo::entre((0, 2), (0, 2)), 1, 2..5)), [(0, 2, "x")]);
    }

    #[test]
    fn frases_do_que_ficou_de_fora() {
        assert_eq!(frase_de_fora(1), "1 célula ficou de fora: passava da última linha ou coluna.");
        assert_eq!(frase_de_fora(1204), "1.204 células ficaram de fora: passavam da última linha ou coluna.");
    }
}
