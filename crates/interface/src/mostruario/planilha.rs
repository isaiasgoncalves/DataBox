//! A planilha do Excel em miniatura (prancheta 17, cena a): a grade com a letra de cada coluna, o número da linha do
//! Excel, as larguras do arquivo (`round(caracteres × 7 + 5)`), o título A1:F1 mesclado, a linha 1 e a coluna A
//! congeladas com o fio de 2px na emenda, números e datas à direita, o booleano e o erro no centro (o erro em
//! `--perigo`), a linha 9 oculta (esmaecida) e a célula ativa E6. Mostra o que o núcleo da grade sabe fazer para a tela
//! da planilha ([`NucleoGrade::com_congelado`], [`NucleoGrade::com_mesclas`], [`NucleoGrade::com_larguras_externas`] e o
//! clique no número da linha) sem depender do crate dela; as capturas `grade-congelada-{claro,escuro}` usam esta peça.

use std::sync::Arc;

use egui::{Color32, Ui};

use crate::formato::formatar_numero;
use crate::grade::{
    Alinhamento, CabecalhoVisivel, CelulaMesclada, CelulaVisivel, ColunaDoArquivo, DesenhoGrade, IndiceDeMesclas,
    Mescla, NucleoGrade, RespostaNucleo, Th, larguras_do_arquivo, pintar_cabecalho_coluna, pintar_numero_linha,
    pintar_texto_alinhado,
};
use crate::pecas::folhas::{EstadoDaFolha, FaixaDeFolhas, Folha, RespostaFolhas};
use crate::tema::{Paleta, paleta};

/// Quantas linhas a planilha de exemplo tem (as do Excel: 1 a 60).
pub const LINHAS: usize = 60;
/// A coluna do número da linha (`td.lin`, 56px) é a 0 da grade; A..F são as de 1 a 6.
const COLUNAS: usize = 6;
const LETRAS: [&str; COLUNAS] = ["A", "B", "C", "D", "E", "F"];
const CABECALHO: [&str; COLUNAS] = ["Região", "Produto", "Fechado em", "Receita", "Atingido", "Meta batida"];
const TITULO: &str = "Vendas do 3º trimestre de 2026 · por região e produto";
const REGIOES: [&str; 5] = ["Sul", "Sudeste", "Nordeste", "Norte", "Centro-Oeste"];
const PRODUTOS: [&str; 6] = [
    "Caderno universitário",
    "Caneta gel 0,7 mm",
    "Mochila escolar G",
    "Agenda 2026 diária",
    "Estojo duplo",
    "Marca-texto (kit 6)",
];
/// A largura de cada coluna no arquivo, em caracteres (131, 215, 103, 131, 103 e 117px).
const CARACTERES: [f64; COLUNAS] = [18.0, 30.0, 14.0, 18.0, 14.0, 16.0];
/// A linha do `#DIV/0!`, a que o filtro do Excel esconde e a célula ativa (linhas e letras do Excel).
const LINHA_DO_ERRO: usize = 12;
const LINHA_OCULTA: usize = 9;
const ATIVA: (usize, usize) = (5, 5);

/// A planilha de exemplo: o núcleo da grade e a linha que o clique no número selecionou.
pub struct PlanilhaExemplo {
    nucleo: NucleoGrade,
    linha_selecionada: Option<usize>,
    /// A planilha ativa na faixa das planilhas ([`FOLHAS`]).
    folha_ativa: usize,
}

/// As planilhas da prancheta 17, cena a: Resumo, Vendas (a ativa), Regiões e Custos (oculta, mostrada pela caixa).
pub const FOLHAS: [Folha<'static>; 4] = [
    Folha { nome: "Resumo", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Vendas", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Regiões", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Custos", estado: EstadoDaFolha::Oculta },
];

/// As doze planilhas da cena b (muitas planilhas: a faixa rola e ganha a lista).
pub const MUITAS_FOLHAS: [Folha<'static>; 12] = [
    Folha { nome: "Resumo", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Vendas", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Regiões", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Metas", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Custos", estado: EstadoDaFolha::Oculta },
    Folha { nome: "Comissões", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Estoque", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Devoluções", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Parâmetros", estado: EstadoDaFolha::MuitoOculta },
    Folha { nome: "Câmbio", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Calendário fiscal", estado: EstadoDaFolha::Visivel },
    Folha { nome: "Notas e fontes", estado: EstadoDaFolha::Visivel },
];

impl PlanilhaExemplo {
    pub fn new(id_salt: &str) -> Self {
        let colunas: Vec<ColunaDoArquivo> =
            CARACTERES.iter().map(|&c| ColunaDoArquivo { caracteres: Some(c), oculta: false }).collect();
        let do_arquivo = larguras_do_arquivo(&colunas);
        let mut larguras = vec![56.0];
        larguras.extend(do_arquivo.larguras);
        let mesclas = IndiceDeMesclas::nova([Mescla::new(0, 1, 0, COLUNAS)]);
        let mut nucleo = NucleoGrade::new(id_salt.to_owned(), LINHAS, Vec::new())
            .com_larguras_externas(larguras)
            .com_coluna_numero(0)
            .com_cabecalho_liso()
            .com_navegaveis(1..COLUNAS + 1)
            .com_congelado(1, 1)
            .com_mesclas(Arc::new(mesclas));
        nucleo.selecionar(Some(ATIVA));
        PlanilhaExemplo { nucleo, linha_selecionada: None, folha_ativa: 1 }
    }

    /// A planilha ativa na faixa.
    pub fn folha_ativa(&self) -> usize {
        self.folha_ativa
    }

    /// A faixa das planilhas embaixo da grade ([`FOLHAS`] ou, com `muitas`, [`MUITAS_FOLHAS`]); o clique troca a ativa.
    pub fn mostrar_folhas(&mut self, ui: &mut Ui, muitas: bool) -> RespostaFolhas {
        let folhas: &[Folha<'static>] = if muitas { &MUITAS_FOLHAS } else { &FOLHAS };
        let r = FaixaDeFolhas::new(("mostruario", muitas), folhas).ativa(Some(self.folha_ativa)).mostrar(ui);
        if let Some(i) = r.escolhida {
            self.folha_ativa = i;
        }
        r
    }

    /// O núcleo (as larguras, a célula ativa, o painel congelado que valeu).
    pub fn nucleo(&self) -> &NucleoGrade {
        &self.nucleo
    }

    pub fn nucleo_mut(&mut self) -> &mut NucleoGrade {
        &mut self.nucleo
    }

    /// A linha que o clique no número escolheu (a do Excel menos 1).
    pub fn linha_selecionada(&self) -> Option<usize> {
        self.linha_selecionada
    }

    /// Desenha a planilha com no máximo `altura_max` de altura.
    pub fn mostrar(&mut self, ui: &mut Ui, altura_max: f32) -> RespostaNucleo {
        let mut desenho = DesenhoPlanilha { p: paleta(ui), selecionada: self.linha_selecionada };
        let r = self.nucleo.mostrar(ui, altura_max, &mut desenho);
        if let Some((linha, _)) = r.clicou_numero_da_linha {
            self.linha_selecionada = Some(linha);
        }
        r
    }
}

/// As seis células de uma linha do Excel (`n` de 1 a [`LINHAS`]), como o Excel mostra: o texto, o alinhamento e se é o
/// erro.
fn valores(n: usize) -> [(String, Alinhamento, bool); COLUNAS] {
    let texto = |t: &str| (t.to_owned(), Alinhamento::Esquerda, false);
    match n {
        1 => [texto(TITULO), texto(""), texto(""), texto(""), texto(""), texto("")],
        2 => CABECALHO.map(texto),
        _ => {
            let k = n - 2;
            let pct = 72 + (k * 37) % 61;
            let (dia, mes) = data(k);
            let receita = (2200 + (k * 7919) % 11_800) * 100 + (k * 37) % 100;
            let (percentual, alinhamento, erro) = if n == LINHA_DO_ERRO {
                ("#DIV/0!".to_owned(), Alinhamento::Centro, true)
            } else {
                (format!("{pct}%"), Alinhamento::Direita, false)
            };
            [
                texto(REGIOES[k % 5]),
                texto(PRODUTOS[(k * 5) % 6]),
                (format!("{dia:02}/{mes:02}/2026"), Alinhamento::Direita, false),
                (format!("R$ {},{:02}", formatar_numero(receita / 100), receita % 100), Alinhamento::Direita, false),
                (percentual, alinhamento, erro),
                (
                    (if pct >= 100 && n != LINHA_DO_ERRO { "VERDADEIRO" } else { "FALSO" }).to_owned(),
                    Alinhamento::Centro,
                    false,
                ),
            ]
        }
    }
}

/// O dia e o mês de 1.º de julho de 2026 mais `k − 1` dias (a planilha de exemplo não passa de dezembro).
fn data(k: usize) -> (usize, usize) {
    const DIAS: [usize; 6] = [31, 31, 30, 31, 30, 31];
    let (mut resto, mut mes) = (k - 1, 7);
    for d in DIAS {
        if resto < d {
            break;
        }
        resto -= d;
        mes += 1;
    }
    (resto + 1, mes)
}

struct DesenhoPlanilha {
    p: &'static Paleta,
    selecionada: Option<usize>,
}

impl DesenhoGrade for DesenhoPlanilha {
    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        let p = self.p;
        let pintor = ui.painter().with_clip_rect(c.recorte);
        let n = c.linha + 1;
        let oculta = n == LINHA_OCULTA;
        if c.coluna == 0 {
            if oculta {
                pintar_texto_alinhado(
                    &pintor,
                    p,
                    c.conteudo,
                    &formatar_numero(n),
                    Alinhamento::Direita,
                    Some(p.estudio.t3),
                    true,
                );
            } else {
                pintar_numero_linha(&pintor, p, c.conteudo, &formatar_numero(n));
            }
            return;
        }
        let (texto, alinhamento, erro) = valores(n)[c.coluna - 1].clone();
        let cor: Option<Color32> = if erro {
            Some(p.estudio.perigo)
        } else if oculta {
            Some(p.estudio.t3)
        } else {
            None
        };
        pintar_texto_alinhado(&pintor, p, c.conteudo, &texto, alinhamento, cor, false);
    }

    fn celula_mesclada(&mut self, ui: &mut Ui, c: &CelulaMesclada) {
        let pintor = ui.painter().with_clip_rect(c.recorte);
        let (texto, ..) = valores(c.linha + 1)[c.coluna - 1].clone();
        pintar_texto_alinhado(&pintor, self.p, c.conteudo, &texto, Alinhamento::Esquerda, None, false);
    }

    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        if h.coluna == 0 {
            return;
        }
        let th = Th { centrada: true, ..Th::new(LETRAS[h.coluna - 1]) };
        pintar_cabecalho_coluna(&ui.painter().with_clip_rect(h.recorte), self.p, h.conteudo, &th);
    }

    fn fundo_linha(&mut self, linha: usize) -> Option<Color32> {
        (self.selecionada == Some(linha)).then_some(self.p.estudio.sel)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_valores_batem_com_a_prancheta() {
        // a linha 3 e a 12 da prancheta 17 (Sudeste, Marca-texto (kit 6), 01/07/2026, R$ 10.119,37, 109%, VERDADEIRO)
        let l3 = valores(3);
        assert_eq!(l3[0].0, "Sudeste");
        assert_eq!(l3[1].0, "Marca-texto (kit 6)");
        assert_eq!(l3[2].0, "01/07/2026");
        assert_eq!(l3[3].0, "R$ 10.119,37");
        assert_eq!(l3[4].0, "109%");
        assert_eq!(l3[5].0, "VERDADEIRO");
        let l12 = valores(12);
        assert_eq!((l12[4].0.as_str(), l12[4].2), ("#DIV/0!", true));
        assert_eq!(l12[3].0, "R$ 10.590,70");
        assert_eq!(data(1), (1, 7));
        assert_eq!(data(32), (1, 8));
    }
}
