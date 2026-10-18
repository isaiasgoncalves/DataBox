//! A grade do CSV sobre o [`NucleoGrade`] do `interface`: a coluna da caixa de
//! marcar (32px), a do número da linha (`#`, 48px: a posição no arquivo, sem o cabeçalho), as colunas de dados
//! com a pastilha de tipo no cabeçalho e o `th.nova-col` com o "+" no fim. As células são texto
//! (`pintar_texto_de_celula`: a vazia fica em branco), a alterada leva o `td.alt` e os números vão à direita.
//!
//! O [`Pintor`] é o [`DesenhoGrade`] de um quadro: lê o [`Retrato`] e a [`Pagina`] (cópias; a grade nunca espera a
//! tabela) e devolve, em campos de saída, o que a pessoa fez e que linhas ficaram à vista (a tela pede a página
//! que falta).

use egui::text::LayoutJob;
use egui::{Painter, Pos2, Rect, Ui, pos2};
use interface::fontes::{self, Peso};
use interface::grade::{
    ALTURA_LINHA, LARGURA_COLUNA_MARCAR, LARGURA_MAX_COLUNA, LARGURA_MIN_COLUNA, LARGURA_PASTILHA_TH, Navegacao,
    NucleoGrade, PADDING_X, largura_cabecalho,
};
use interface::pecas::botao::LADO_PEQUENO;
use interface::pecas::texto::formato;
use interface::tema::Paleta;

use crate::sessao::{Pagina, Retrato};

/// As colunas fixas da grade: a caixa de marcar, o número da linha; as de dados vêm depois, e o "+" no fim.
pub(crate) const COLUNA_MARCAR: usize = 0;
pub(crate) const COLUNA_NUMERO: usize = 1;
pub(crate) const PRIMEIRA_DE_DADOS: usize = 2;
/// `th.num` do número da linha: `width:48px` (o `padding:0 8px` é do pintor).
pub(crate) const LARGURA_NUMERO: f32 = 48.0;
/// `th.nova-col{padding:0 4px}` com o `.ib.pq` de 26px.
pub(crate) const LARGURA_NOVA_COLUNA: f32 = LADO_PEQUENO + 8.0;
/// Quantas linhas da página entram na medida das colunas.
const LINHAS_MEDIDAS: usize = 200;

/// A grade de um documento: o núcleo (rolagem, larguras, célula ativa) e as larguras pelo conteúdo.
pub(crate) struct GradeCsv {
    pub nucleo: NucleoGrade,
    n_dados: usize,
    naturais: Vec<f32>,
    aplicadas: Vec<f32>,
    medir: bool,
    manual: bool,
    /// O canto do conteúdo da grade (sem rolagem) e o retângulo visível do corpo no último quadro (os testes acham as
    /// células por eles).
    pub origem: Option<Pos2>,
    pub visivel: Option<Rect>,
}

impl GradeCsv {
    pub(crate) fn nova(id: &str, n_dados: usize) -> GradeCsv {
        let n = PRIMEIRA_DE_DADOS + n_dados + 1;
        let nucleo = NucleoGrade::new(("tela_csv", id), 0, vec![LARGURA_MIN_COLUNA; n])
            .com_navegacao(Navegacao::Dados)
            .com_navegaveis(PRIMEIRA_DE_DADOS..PRIMEIRA_DE_DADOS + n_dados)
            .com_coluna_marcar(COLUNA_MARCAR)
            .com_coluna_numero(COLUNA_NUMERO)
            .com_fixas(&[n - 1]);
        GradeCsv {
            nucleo,
            n_dados,
            naturais: Vec::new(),
            aplicadas: Vec::new(),
            medir: true,
            manual: false,
            origem: None,
            visivel: None,
        }
    }

    pub(crate) fn n_dados(&self) -> usize {
        self.n_dados
    }

    /// As colunas mudaram de número (uma nova, a provisória, uma excluída): o núcleo é refeito com as larguras
    /// medidas de novo, e a célula ativa fica onde estava (se ainda existir).
    pub(crate) fn refazer(&mut self, id: &str, n_dados: usize) {
        let selecao = self.nucleo.selecao();
        let linhas = self.nucleo.n_linhas();
        *self = GradeCsv::nova(id, n_dados);
        self.nucleo.definir_linhas(linhas);
        self.nucleo.definir_larguras(vec![LARGURA_MIN_COLUNA; PRIMEIRA_DE_DADOS + n_dados + 1]);
        if let Some(s) = selecao {
            self.nucleo.selecionar(Some(s));
        }
    }

    /// A coluna de dados da grade (de 0) que está na coluna `c` do núcleo.
    pub(crate) fn dado(&self, c: usize) -> Option<usize> {
        c.checked_sub(PRIMEIRA_DE_DADOS).filter(|&d| d < self.n_dados)
    }

    /// Mede as colunas de novo no próximo quadro (a página chegou, as colunas mudaram).
    pub(crate) fn medir_de_novo(&mut self) {
        self.medir = true;
    }

    /// As larguras pelo conteúdo e a repartição da largura disponível (o `table-layout:fixed` de 100% da prancheta:
    /// a sobra vai para as colunas que cabem com folga, por igual). Depois que a pessoa arrasta uma borda, as
    /// larguras ficam como ela deixou.
    pub(crate) fn ajustar_larguras(&mut self, ui: &Ui, p: &Paleta, retrato: &Retrato, pagina: Option<&Pagina>) {
        if self.medir {
            self.medir = false;
            self.naturais = medir_colunas(ui.painter(), p, retrato, pagina, self.n_dados);
            self.manual = false;
        }
        if self.naturais.len() != self.n_dados {
            return;
        }
        if !self.manual && !self.aplicadas.is_empty() && self.nucleo.larguras() != self.aplicadas.as_slice() {
            self.manual = true;
        }
        if self.manual {
            return;
        }
        let fixas = LARGURA_COLUNA_MARCAR + LARGURA_NUMERO + LARGURA_NOVA_COLUNA;
        let dados = repartir(&self.naturais, (ui.available_width() - fixas).max(0.0));
        let mut novas = Vec::with_capacity(dados.len() + 3);
        novas.extend([LARGURA_COLUNA_MARCAR, LARGURA_NUMERO]);
        novas.extend(dados);
        novas.push(LARGURA_NOVA_COLUNA);
        if novas.as_slice() != self.nucleo.larguras() {
            self.nucleo.definir_larguras(novas.clone());
        }
        self.aplicadas = novas;
    }

    /// O centro de uma célula (linha da vista, coluna de dados) no último quadro, se estiver à vista (os testes).
    pub(crate) fn centro_celula(&self, linha: usize, coluna: usize) -> Option<Pos2> {
        self.centro(linha, PRIMEIRA_DE_DADOS + coluna)
    }

    /// O centro da caixa de marcar de uma linha.
    pub(crate) fn centro_marca(&self, linha: usize) -> Option<Pos2> {
        self.centro(linha, COLUNA_MARCAR)
    }

    fn centro(&self, linha: usize, c: usize) -> Option<Pos2> {
        let origem = self.origem?;
        let larguras = self.nucleo.larguras();
        let x0: f32 = larguras.iter().take(c).sum();
        let w = *larguras.get(c)?;
        let y = origem.y + self.nucleo.altura_do_cabecalho() + linha as f32 * ALTURA_LINHA + (ALTURA_LINHA - 1.0) / 2.0;
        let pos = pos2(origem.x + x0 + w / 2.0, y);
        self.visivel.filter(|v| v.contains(pos)).map(|_| pos)
    }

    /// O centro do `th` de uma coluna de dados (os testes clicam nele).
    pub(crate) fn centro_cabecalho(&self, coluna: usize) -> Option<Pos2> {
        let origem = self.origem?;
        let c = PRIMEIRA_DE_DADOS + coluna;
        let larguras = self.nucleo.larguras();
        let x0: f32 = larguras.iter().take(c).sum();
        let w = *larguras.get(c)?;
        let visivel = self.visivel?;
        Some(pos2(origem.x + x0 + w / 2.0, visivel.top() + self.nucleo.altura_do_cabecalho() / 2.0))
    }
}

/// A largura de um texto de célula numa linha (12px).
fn largura_texto(painter: &Painter, texto: &str) -> f32 {
    if texto.is_empty() {
        return 0.0;
    }
    let mut job = LayoutJob::single_section(
        texto.replace(['\n', '\r', '\t'], " "),
        formato(fontes::fonte(12.0, Peso::Normal), egui::Color32::PLACEHOLDER),
    );
    job.break_on_newline = false;
    painter.layout_job(job).size().x
}

/// As larguras pelo conteúdo de cada coluna de dados: a pastilha e o nome, e as primeiras linhas da página, até
/// 320px (com o padding e o fio).
fn medir_colunas(painter: &Painter, _p: &Paleta, retrato: &Retrato, pagina: Option<&Pagina>, n: usize) -> Vec<f32> {
    let (esq, dir) = (PADDING_X, PADDING_X + 1.0);
    let limite = LARGURA_MAX_COLUNA - esq - dir;
    (0..n)
        .map(|c| {
            let nome = retrato.nomes.get(c).map_or("", String::as_str);
            let mut maior = LARGURA_PASTILHA_TH + largura_cabecalho(painter, nome, None);
            if let Some(p) = pagina {
                for linha in p.linhas.iter().take(LINHAS_MEDIDAS) {
                    if maior >= limite {
                        break;
                    }
                    if let Some(v) = linha.get(c) {
                        maior = maior.max(largura_texto(painter, v));
                    }
                }
            }
            (maior.min(limite) + esq + dir).ceil().max(LARGURA_MIN_COLUNA)
        })
        .collect()
}

/// Reparte `disponivel` entre as colunas: cada uma fica com pelo menos a natural; se todas cabem, a sobra vai por
/// igual às que precisam de menos que a parte igual (as largas ficam com a natural).
pub(crate) fn repartir(naturais: &[f32], disponivel: f32) -> Vec<f32> {
    let total: f32 = naturais.iter().sum();
    if naturais.is_empty() || total >= disponivel {
        return naturais.to_vec();
    }
    let mut largas = vec![false; naturais.len()];
    loop {
        let restantes = largas.iter().filter(|l| !**l).count();
        if restantes == 0 {
            return naturais.to_vec();
        }
        let ocupado: f32 = naturais.iter().zip(&largas).filter(|(_, l)| **l).map(|(w, _)| w).sum();
        let parte = ((disponivel - ocupado) / restantes as f32).floor();
        let mudou = naturais.iter().zip(largas.iter_mut()).fold(false, |m, (w, l)| {
            if !*l && *w > parte {
                *l = true;
                true
            } else {
                m
            }
        });
        if !mudou {
            return naturais.iter().zip(&largas).map(|(w, l)| if *l { *w } else { parte }).collect();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn repartir_da_a_sobra_por_igual_e_deixa_as_largas() {
        assert_eq!(repartir(&[50.0, 60.0], 300.0), [150.0, 150.0]);
        assert_eq!(repartir(&[50.0, 200.0], 300.0), [100.0, 200.0]);
        // sem sobra, as naturais
        assert_eq!(repartir(&[200.0, 200.0], 300.0), [200.0, 200.0]);
        assert!(repartir(&[], 300.0).is_empty());
    }
}
