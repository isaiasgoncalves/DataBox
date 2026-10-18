//! A grade da planilha (decisões 7 a 11 da etapa 6): o [`NucleoGrade`] do `interface` com a coluna do número da linha do
//! Excel (56px, `td.lin`), o cabeçalho com a letra de cada coluna (centrada, no degradê liso) e o conteúdo pelo
//! [`DesenhoGrade`]: o texto como o Excel mostra (`Pasta::exibir`), à esquerda o texto, à direita números, datas e
//! horas, no centro o booleano e o erro (o erro em `--perigo`); a linha oculta do arquivo esmaecida (o número em
//! itálico); as linhas cujo bloco ainda não chegou só com o número, a 55%.
//!
//! - **Larguras do arquivo**: `round(caracteres × 7 + 5)` entre 24 e 640 (`larguras_do_arquivo`), a padrão
//!   da aba ou 8,43; a coluna oculta sai. A alça de redimensionar continua (a largura é da sessão).
//! - **Mesclas** e **painel congelado**: do `MetaFolha`, pelo `com_mesclas`/`com_congelado` do
//!   núcleo. As mesclas só vêm completas depois do índice: a grade se refaz quando ele termina.
//! - **Seleção**: a célula ativa do núcleo; as colunas (clique no `th`; Ctrl e Shift somam) e as linhas
//!   (clique no número) selecionadas, em `--sel`/`--tint`.
//!
//! A coluna 0 da grade é a do número; a coluna `k` (de 1) é a visível `k − 1`, que no arquivo é
//! `do_arquivo[k − 1]` (as colunas ocultas do arquivo saem). A linha `l` da grade é a linha `l + 1` do Excel.

use std::collections::{BTreeSet, HashMap};
use std::ops::RangeInclusive;
use std::sync::Arc;

use egui::{Color32, Ui};
use interface::formato::formatar_numero;
use interface::grade::{
    Alinhamento, CabecalhoVisivel, CelulaMesclada, CelulaVisivel, ColunaDoArquivo, Congelado, DesenhoGrade,
    IndiceDeMesclas, Mescla, NucleoGrade, RespostaNucleo, Th, Toques, larguras_do_arquivo, pintar_cabecalho_coluna,
    pintar_texto_alinhado,
};
use interface::tema::{Paleta, paleta};
use planilha::{Celula, Intervalo, Linha, MetaFolha, Pasta, Valor, nome_da_coluna, referencia_a1};

use crate::sessao::SessaoDaFolha;

/// A largura da coluna do número da linha (`.xl td.lin{width:56px}`).
pub const LARGURA_NUMERO: f32 = 56.0;
/// A opacidade das linhas cujo bloco ainda não chegou (`.xl tr.fant td{opacity:.55}`).
pub const OPACIDADE_FANTASMA: f32 = 0.55;
/// A coluna do número cresce para caber o maior número da planilha ("1.048.576"): o `padding:0 10px`, o fio e uns 7px
/// por algarismo e 4px por ponto, nunca menos de [`LARGURA_NUMERO`].
pub fn largura_do_numero(linhas: u32) -> f32 {
    let texto = formatar_numero(linhas as usize);
    let (algarismos, pontos) =
        texto.chars().fold((0, 0), |(a, p), c| if c.is_ascii_digit() { (a + 1, p) } else { (a, p + 1) });
    (algarismos as f32 * 7.0 + pontos as f32 * 4.0 + 21.0).ceil().max(LARGURA_NUMERO)
}

/// Até quantos caracteres a dica mostra o texto da célula.
pub const CARACTERES_NA_DICA: usize = 300;

/// Onde o texto de um valor vai na célula.
pub fn alinhamento_do_valor(valor: &Valor) -> Alinhamento {
    match valor {
        Valor::Numero(_) | Valor::Data(_) | Valor::Hora(_) | Valor::DataHora(_) => Alinhamento::Direita,
        Valor::Booleano(_) | Valor::Erro(_) => Alinhamento::Centro,
        Valor::Vazio | Valor::Texto(_) => Alinhamento::Esquerda,
    }
}

/// As colunas do arquivo como a grade as conhece: a largura em caracteres (a da coluna, a padrão da aba, ou nenhuma) e
/// se está oculta, para as `n` primeiras.
pub fn colunas_do_arquivo(meta: &MetaFolha, n: u32) -> Vec<ColunaDoArquivo> {
    (0..n)
        .map(|c| {
            let info = meta.colunas.iter().find(|i| (i.primeira..=i.ultima).contains(&c));
            ColunaDoArquivo {
                caracteres: info.and_then(|i| i.largura).or(meta.largura_padrao),
                oculta: info.is_some_and(|i| i.oculta),
            }
        })
        .collect()
}

/// A dica de uma célula: o texto inteiro (até [`CARACTERES_NA_DICA`]; o cortado diz o total) e, com fórmula,
/// "= {fórmula}" na linha de baixo.
pub fn dica_da_celula(texto: &str, formula: Option<&str>) -> Option<String> {
    let n = texto.chars().count();
    let mut dica = if n > CARACTERES_NA_DICA {
        let curto: String = texto.chars().take(CARACTERES_NA_DICA).collect();
        format!("{curto}…\n{} caracteres no total", formatar_numero(n))
    } else {
        texto.to_owned()
    };
    if let Some(f) = formula {
        if !dica.is_empty() {
            dica.push('\n');
        }
        dica.push_str("= ");
        dica.push_str(f);
    }
    (!dica.is_empty()).then_some(dica)
}

/// A grade de uma planilha: o núcleo (larguras, célula ativa, rolagem), o mapa das colunas visíveis e a seleção.
pub struct Grade {
    pub nucleo: NucleoGrade,
    /// Para cada coluna visível (a partir de 0), a coluna do arquivo.
    pub do_arquivo: Vec<u32>,
    /// Quantas colunas do arquivo a grade cobre (as ocultas contam).
    pub colunas_do_arquivo: u32,
    /// As colunas (do arquivo) e as linhas (do Excel) selecionadas.
    pub colunas_selecionadas: BTreeSet<u32>,
    pub linhas_selecionadas: BTreeSet<u32>,
    /// Onde a seleção com Shift começa (a última coluna e a última linha clicadas).
    ancora_coluna: Option<u32>,
    ancora_linha: Option<u32>,
    /// O que a grade foi montada para mostrar: (colunas do arquivo, indexada). Muda → refaz larguras e mapas.
    assinatura: Option<(u32, bool)>,
    /// As linhas da grade que rolam e que o último quadro desenhou.
    desenhadas: Option<RangeInclusive<usize>>,
}

impl Grade {
    pub fn nova(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Grade {
        let nucleo = NucleoGrade::new(id_salt, 0, vec![LARGURA_NUMERO])
            .com_larguras_externas(vec![LARGURA_NUMERO])
            .com_coluna_numero(0)
            .com_cabecalho_liso()
            .com_navegaveis(1..1);
        Grade {
            nucleo,
            do_arquivo: Vec::new(),
            colunas_do_arquivo: 0,
            colunas_selecionadas: BTreeSet::new(),
            linhas_selecionadas: BTreeSet::new(),
            ancora_coluna: None,
            ancora_linha: None,
            assinatura: None,
            desenhadas: None,
        }
    }

    /// A coluna da grade (de 1) da coluna `coluna` do arquivo, se ela está à vista.
    pub fn coluna_da_grade(&self, coluna: u32) -> Option<usize> {
        self.do_arquivo.binary_search(&coluna).ok().map(|i| i + 1)
    }

    /// A coluna do arquivo da coluna `c` da grade (de 1).
    pub fn coluna_do_arquivo(&self, c: usize) -> Option<u32> {
        c.checked_sub(1).and_then(|i| self.do_arquivo.get(i)).copied()
    }

    /// Quantas colunas do arquivo estão ocultas (nas que a grade cobre).
    pub fn colunas_ocultas(&self) -> usize {
        self.colunas_do_arquivo as usize - self.do_arquivo.len()
    }

    /// A célula ativa em linha do Excel e coluna do arquivo.
    pub fn celula_ativa(&self) -> Option<(u32, u32)> {
        let (l, c) = self.nucleo.selecao()?;
        Some((l as u32 + 1, self.coluna_do_arquivo(c)?))
    }

    /// A referência da célula ativa ("E6"), ou o intervalo da mescla que a cobre ("A1:F1").
    pub fn referencia_ativa(&self, meta: &MetaFolha) -> Option<String> {
        let (l, c) = self.celula_ativa()?;
        Some(match meta.mescla_em(l, c) {
            Some(m) => m.to_string(),
            None => referencia_a1(l, c),
        })
    }

    /// A coluna visível (do arquivo) que está no intervalo de colunas do arquivo, a primeira ou a última.
    fn visivel_entre(&self, de: u32, ate: u32, primeira: bool) -> Option<usize> {
        let a = self.do_arquivo.partition_point(|&c| c < de);
        let b = self.do_arquivo.partition_point(|&c| c <= ate);
        if a >= b {
            return None;
        }
        Some(if primeira { a + 1 } else { b })
    }

    /// A mescla do arquivo nas linhas e colunas da grade (some se nenhuma coluna dela está à vista).
    fn mescla_da_grade(&self, m: &Intervalo) -> Option<Mescla> {
        let c0 = self.visivel_entre(m.coluna_inicial, m.coluna_final, true)?;
        let c1 = self.visivel_entre(m.coluna_inicial, m.coluna_final, false)?;
        Some(Mescla::new(m.linha_inicial.max(1) as usize - 1, c0, m.linha_final.max(1) as usize - 1, c1))
    }

    /// Acerta a grade com o que a sessão sabe agora: o número de linhas a cada quadro; as larguras, o mapa das colunas,
    /// as mesclas e o painel congelado quando o tamanho ou o índice mudam.
    pub fn sincronizar(&mut self, sessao: &SessaoDaFolha) {
        let (linhas, colunas) = sessao.tamanho();
        self.nucleo.definir_linhas(linhas as usize);
        let numero = largura_do_numero(linhas);
        if self.nucleo.larguras().first().is_some_and(|&w| (w - numero).abs() > 0.5) {
            self.nucleo.definir_largura(0, numero);
        }
        let assinatura = (colunas, sessao.indexada());
        if self.assinatura == Some(assinatura) {
            return;
        }
        self.assinatura = Some(assinatura);
        let arquivo = larguras_do_arquivo(&colunas_do_arquivo(&sessao.meta, colunas));
        // as larguras que a pessoa arrastou nesta sessão ficam (por coluna do arquivo)
        let antigas: HashMap<u32, f32> =
            self.do_arquivo.iter().zip(self.nucleo.larguras().iter().skip(1)).map(|(&c, &w)| (c, w)).collect();
        self.do_arquivo = arquivo.do_arquivo.iter().map(|&c| c as u32).collect();
        self.colunas_do_arquivo = colunas;
        let mut larguras = Vec::with_capacity(self.do_arquivo.len() + 1);
        larguras.push(numero);
        for (c, &w) in self.do_arquivo.iter().zip(&arquivo.larguras) {
            larguras.push(antigas.get(c).copied().unwrap_or(w));
        }
        let n = larguras.len();
        let nucleo = std::mem::replace(&mut self.nucleo, NucleoGrade::new("vazia", 0, Vec::new()));
        let mut nucleo = nucleo.com_larguras_externas(larguras).com_navegaveis(1..n);
        nucleo.definir_linhas(linhas as usize);
        // o painel congelado do arquivo: as colunas presas são as visíveis entre as congeladas. Sempre um painel, mesmo
        // vazio: é ele que deixa a coluna do número presa à esquerda quando a grade rola para o lado (sem linha nem
        // coluna congelada, o núcleo não pinta a emenda)
        let congelado = sessao.meta.congelado.map_or_else(Congelado::default, |c| Congelado {
            linhas: c.linhas as usize,
            colunas: self.do_arquivo.partition_point(|&a| a < c.colunas),
        });
        nucleo.definir_congelado(Some(congelado));
        let mesclas: Vec<Mescla> = sessao.meta.mescladas.iter().filter_map(|m| self.mescla_da_grade(m)).collect();
        nucleo.definir_mesclas((!mesclas.is_empty()).then(|| Arc::new(IndiceDeMesclas::nova(mesclas))));
        self.nucleo = nucleo;
    }

    /// O clique no `th` da coluna `c` da grade: sozinho escolhe só ela; Ctrl soma ou tira; Shift estende da âncora.
    pub fn clicar_coluna(&mut self, c: usize, toques: Toques) {
        let Some(coluna) = self.coluna_do_arquivo(c) else { return };
        self.linhas_selecionadas.clear();
        if toques.shift
            && let Some(a) = self.ancora_coluna
        {
            let (de, ate) = (a.min(coluna), a.max(coluna));
            if !toques.ctrl {
                self.colunas_selecionadas.clear();
            }
            self.colunas_selecionadas.extend(self.do_arquivo.iter().copied().filter(|&x| (de..=ate).contains(&x)));
        } else if toques.ctrl {
            if !self.colunas_selecionadas.remove(&coluna) {
                self.colunas_selecionadas.insert(coluna);
            }
            self.ancora_coluna = Some(coluna);
        } else {
            self.colunas_selecionadas = BTreeSet::from([coluna]);
            self.ancora_coluna = Some(coluna);
        }
    }

    /// O clique no número da linha `l` da grade (como o da coluna).
    pub fn clicar_linha(&mut self, l: usize, toques: Toques) {
        let linha = l as u32 + 1;
        self.colunas_selecionadas.clear();
        if toques.shift
            && let Some(a) = self.ancora_linha
        {
            if !toques.ctrl {
                self.linhas_selecionadas.clear();
            }
            self.linhas_selecionadas.extend(a.min(linha)..=a.max(linha));
        } else if toques.ctrl {
            if !self.linhas_selecionadas.remove(&linha) {
                self.linhas_selecionadas.insert(linha);
            }
            self.ancora_linha = Some(linha);
        } else {
            self.linhas_selecionadas = BTreeSet::from([linha]);
            self.ancora_linha = Some(linha);
        }
    }

    /// Tira a seleção de colunas e de linhas (o clique numa célula).
    pub fn limpar_selecao(&mut self) {
        self.colunas_selecionadas.clear();
        self.linhas_selecionadas.clear();
    }

    /// Desenha a grade em no máximo `altura` e devolve o que o núcleo respondeu e as linhas do Excel que rolam e
    /// apareceram (as congeladas ficam de fora: são sempre as primeiras).
    pub fn mostrar(
        &mut self,
        ui: &mut Ui,
        sessao: &SessaoDaFolha,
        altura: f32,
    ) -> (RespostaNucleo, Option<RangeInclusive<u32>>) {
        let congeladas = self.nucleo.congelado().map_or(0, |c| c.linhas);
        let mut nucleo = std::mem::replace(&mut self.nucleo, NucleoGrade::new("vazia", 0, Vec::new()));
        let mut desenho = Desenho { p: paleta(ui), sessao, grade: &*self, congeladas, primeira: usize::MAX, ultima: 0 };
        let r = nucleo.mostrar(ui, altura, &mut desenho);
        let (primeira, ultima) = (desenho.primeira, desenho.ultima);
        self.nucleo = nucleo;
        self.desenhadas = (primeira <= ultima).then_some(primeira..=ultima);
        let visiveis = self.desenhadas.clone().map(|d| (*d.start() as u32 + 1)..=(*d.end() as u32 + 1));
        (r, visiveis)
    }
}

/// O texto de uma célula como a grade o mostra (o `exibir` do núcleo).
pub fn texto_da_celula(pasta: &Pasta, celula: &Celula) -> String {
    pasta.exibir(celula)
}

struct Desenho<'a> {
    p: &'static Paleta,
    sessao: &'a SessaoDaFolha,
    grade: &'a Grade,
    congeladas: usize,
    primeira: usize,
    ultima: usize,
}

impl Desenho<'_> {
    fn linha(&self, l: usize) -> Option<&Linha> {
        self.sessao.linha(l as u32 + 1)
    }

    /// O texto, a cor e o alinhamento da célula (`l`, `c` da grade), ou nada se ela está vazia ou não chegou.
    fn conteudo(&self, l: usize, c: usize) -> Option<(String, Option<Color32>, Alinhamento)> {
        let e = &self.p.estudio;
        let linha = self.linha(l)?;
        let celula = linha.celula(self.grade.coluna_do_arquivo(c)?)?;
        if celula.valor.vazio() {
            return None;
        }
        let texto = texto_da_celula(self.sessao.pasta(), celula);
        let cor = match &celula.valor {
            Valor::Erro(_) => Some(e.perigo),
            _ if linha.oculta => Some(e.t3),
            _ => None,
        };
        Some((texto, cor, alinhamento_do_valor(&celula.valor)))
    }
}

impl DesenhoGrade for Desenho<'_> {
    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        if c.linha >= self.congeladas {
            self.primeira = self.primeira.min(c.linha);
            self.ultima = self.ultima.max(c.linha);
        }
        let p = self.p;
        let e = &p.estudio;
        let mut pintor = ui.painter().with_clip_rect(c.recorte);
        let linha = self.linha(c.linha);
        if linha.is_none() {
            pintor.multiply_opacity(OPACIDADE_FANTASMA);
        }
        if c.coluna == 0 {
            // `td.lin`: o número do Excel em `--t3`; o da linha oculta em itálico (`tr.ocul td.lin`)
            let oculta = linha.is_some_and(|l| l.oculta);
            let numero = formatar_numero(c.linha + 1);
            pintar_texto_alinhado(&pintor, p, c.conteudo, &numero, Alinhamento::Direita, Some(e.t3), oculta);
            return;
        }
        if let Some((texto, cor, alinhamento)) = self.conteudo(c.linha, c.coluna) {
            pintar_texto_alinhado(&pintor, p, c.conteudo, &texto, alinhamento, cor, false);
        }
    }

    fn celula_mesclada(&mut self, ui: &mut Ui, c: &CelulaMesclada) {
        // o texto da de cima à esquerda, à esquerda e no meio na vertical (`td.mesc`)
        if let Some((texto, cor, _)) = self.conteudo(c.linha, c.coluna) {
            let pintor = ui.painter().with_clip_rect(c.recorte);
            pintar_texto_alinhado(&pintor, self.p, c.conteudo, &texto, Alinhamento::Esquerda, cor, false);
        }
    }

    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        let Some(coluna) = self.grade.coluna_do_arquivo(h.coluna) else {
            return;
        };
        let letra = nome_da_coluna(coluna);
        let selecionada = self.grade.colunas_selecionadas.contains(&coluna);
        let th = Th { centrada: true, selecionada, ..Th::new(&letra) };
        pintar_cabecalho_coluna(&ui.painter().with_clip_rect(h.recorte), self.p, h.conteudo, &th);
    }

    fn fundo_linha(&mut self, linha: usize) -> Option<Color32> {
        self.grade.linhas_selecionadas.contains(&(linha as u32 + 1)).then_some(self.p.estudio.sel)
    }

    fn th_selecionada(&mut self, coluna: usize) -> bool {
        self.grade.coluna_do_arquivo(coluna).is_some_and(|c| self.grade.colunas_selecionadas.contains(&c))
    }

    fn dica(&mut self, linha: usize, coluna: usize) -> Option<String> {
        let celula = self.linha(linha)?.celula(self.grade.coluna_do_arquivo(coluna)?)?;
        let texto = if celula.valor.vazio() { String::new() } else { texto_da_celula(self.sessao.pasta(), celula) };
        dica_da_celula(&texto, celula.formula.as_deref())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn alinhamento_por_tipo() {
        assert_eq!(alinhamento_do_valor(&Valor::Numero(1.0)), Alinhamento::Direita);
        assert_eq!(alinhamento_do_valor(&Valor::Texto("a".into())), Alinhamento::Esquerda);
        assert_eq!(alinhamento_do_valor(&Valor::Booleano(true)), Alinhamento::Centro);
        assert_eq!(alinhamento_do_valor(&Valor::Erro("#DIV/0!".into())), Alinhamento::Centro);
    }

    #[test]
    fn coluna_do_numero_cabe_o_maior_numero() {
        assert_eq!(largura_do_numero(60), LARGURA_NUMERO);
        assert_eq!(largura_do_numero(1_204), LARGURA_NUMERO);
        assert!(largura_do_numero(200_000) >= 6.0 * 7.0 + 4.0 + 21.0);
        assert!(largura_do_numero(1_048_576) > largura_do_numero(200_000));
    }

    #[test]
    fn dica_com_formula_e_texto_longo() {
        assert_eq!(dica_da_celula("98%", Some("D6/Resumo!C3")).as_deref(), Some("98%\n= D6/Resumo!C3"));
        assert_eq!(dica_da_celula("", None), None);
        let longo = "a".repeat(400);
        let d = dica_da_celula(&longo, None).expect("a dica");
        assert!(d.ends_with("…\n400 caracteres no total"));
    }

    #[test]
    fn mescla_e_colunas_ocultas() {
        let mut g = Grade::nova("t");
        g.do_arquivo = vec![0, 2, 3];
        g.colunas_do_arquivo = 4;
        // A1:D1 com a B oculta: da coluna 1 à 3 da grade
        let m = Intervalo { linha_inicial: 1, coluna_inicial: 0, linha_final: 1, coluna_final: 3 };
        assert_eq!(g.mescla_da_grade(&m), Some(Mescla::new(0, 1, 0, 3)));
        // só a oculta: some
        let so_b = Intervalo { linha_inicial: 2, coluna_inicial: 1, linha_final: 3, coluna_final: 1 };
        assert_eq!(g.mescla_da_grade(&so_b), None);
        assert_eq!(g.colunas_ocultas(), 1);
        assert_eq!((g.coluna_da_grade(2), g.coluna_do_arquivo(3)), (Some(2), Some(3)));
    }

    #[test]
    fn selecao_de_colunas_e_linhas_com_toques() {
        let mut g = Grade::nova("t");
        g.do_arquivo = vec![0, 1, 2, 3, 4];
        let nada = Toques::default();
        let ctrl = Toques { ctrl: true, shift: false };
        let shift = Toques { ctrl: false, shift: true };
        g.clicar_coluna(2, nada);
        assert_eq!(g.colunas_selecionadas.iter().copied().collect::<Vec<_>>(), [1]);
        g.clicar_coluna(4, ctrl);
        assert_eq!(g.colunas_selecionadas.iter().copied().collect::<Vec<_>>(), [1, 3]);
        g.clicar_coluna(1, shift);
        assert_eq!(g.colunas_selecionadas.iter().copied().collect::<Vec<_>>(), [0, 1, 2, 3]);
        g.clicar_linha(4, nada);
        assert!(g.colunas_selecionadas.is_empty(), "a linha tira a coluna");
        g.clicar_linha(6, shift);
        assert_eq!(g.linhas_selecionadas.iter().copied().collect::<Vec<_>>(), [5, 6, 7]);
        g.clicar_linha(6, ctrl);
        assert_eq!(g.linhas_selecionadas.iter().copied().collect::<Vec<_>>(), [5, 6]);
    }
}
