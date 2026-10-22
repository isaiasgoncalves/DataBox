//! O rodapé da seleção (0.2.3, `design/planos-0.2.3.md`, A.3): com mais de uma célula selecionada, o rodapé do
//! documento mostra "Contagem 33 · Soma 98.231,40 · Média 8.930,13 · Mín. 154,03 · Máx. 17.810,34", como a barra de
//! status do Excel. A contagem é das células não vazias; a soma, a média, o mínimo e o máximo, só das numéricas (o
//! tipo da coluna nos Dados, o detectado no CSV e no Excel, o valor na Consulta), no formato pt-BR. Onde não couber,
//! sai primeiro o Mín. e o Máx., depois a Média, e o que saiu vai para a dica.

use comum::Celula;
use egui::{Response, Sense, Ui, WidgetInfo, WidgetType};
use tabular::numero::literal_numerico;

use crate::fontes::{self, Peso};
use crate::formato::{formatar_numero, formatar_numero_com_casas, real_js};
use crate::pecas::avisos::carregando;
use crate::pecas::texto::{galley, mostrar_galley};
use crate::tema::paleta;

/// O que o rodapé diz enquanto a conta roda em segundo plano (o CSV com o intervalo lido só em parte).
pub const SOMANDO: &str = "Somando…";
/// As casas decimais que um número leva no máximo para as contas (as de `formatar_numero`).
const MAX_CASAS: u8 = 6;
/// A média leva pelo menos estas casas (a média de inteiros, "3,33").
const CASAS_DA_MEDIA: u8 = 2;
/// O que vai entre as partes.
const SEPARADOR: &str = " · ";

/// As contas de uma seleção, somadas célula a célula ([`ResumoSelecao::somar_celula`] nos Dados e na Consulta,
/// [`ResumoSelecao::somar_do_csv`] no CSV e no Excel); [`ResumoSelecao::juntar`] soma duas partes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResumoSelecao {
    /// As células não vazias.
    pub contagem: u64,
    /// As numéricas, das quais saem a soma, a média, o mínimo e o máximo.
    pub numeros: u64,
    pub soma: f64,
    pub minimo: f64,
    pub maximo: f64,
    /// As casas decimais do número mais longo somado (até 6): a soma, o mínimo e o máximo saem com elas.
    pub casas: u8,
}

/// As casas decimais de um literal (`12.50` → 2, `1e-7` → 6, `17051` → 0).
fn casas_do_literal(literal: &str) -> u8 {
    if literal.contains(['e', 'E']) {
        return MAX_CASAS;
    }
    literal.split_once('.').map_or(0, |(_, f)| u8::try_from(f.len()).unwrap_or(MAX_CASAS).min(MAX_CASAS))
}

impl ResumoSelecao {
    /// Uma célula que não é número: conta, se não está vazia.
    pub fn somar_texto(&mut self, texto: &str) {
        if !texto.is_empty() {
            self.contagem += 1;
        }
    }

    /// Um número, com as `casas` decimais com que ele foi escrito. Infinito e NaN contam, mas não entram nas contas.
    pub fn somar_numero(&mut self, valor: f64, casas: u8) {
        self.contagem += 1;
        if !valor.is_finite() {
            return;
        }
        if self.numeros == 0 {
            (self.minimo, self.maximo) = (valor, valor);
        } else {
            self.minimo = self.minimo.min(valor);
            self.maximo = self.maximo.max(valor);
        }
        self.numeros += 1;
        self.soma += valor;
        self.casas = self.casas.max(casas.min(MAX_CASAS));
    }

    /// Uma célula do banco: o NULL e o texto vazio não contam; o inteiro e o real entram nas contas numa coluna
    /// `numerica` (o tipo da coluna, nos Dados; na Consulta, `true`: o valor decide); o resto só conta.
    pub fn somar_celula(&mut self, celula: &Celula, numerica: bool) {
        match celula {
            Celula::Nulo => {}
            Celula::Texto(t) if t.is_empty() => {}
            Celula::Inteiro(i) if numerica => self.somar_numero(*i as f64, 0),
            Celula::Real(r) if numerica => self.somar_numero(*r, casas_do_literal(&real_js(*r))),
            _ => self.contagem += 1,
        }
    }

    /// Uma célula de arquivo (o CSV, o Excel): o texto vazio não conta; numa coluna `numerica` (o tipo detectado), o
    /// que se lê como número com o `decimal` do arquivo entra nas contas.
    pub fn somar_do_csv(&mut self, texto: &str, numerica: bool, decimal: char) {
        if texto.trim().is_empty() {
            return;
        }
        let numero = numerica
            .then(|| literal_numerico(texto, decimal))
            .flatten()
            .and_then(|l| Some((l.parse::<f64>().ok()?, casas_do_literal(&l))));
        match numero {
            Some((valor, casas)) => self.somar_numero(valor, casas),
            None => self.contagem += 1,
        }
    }

    /// Soma as contas de outra parte da seleção.
    pub fn juntar(&mut self, outro: &ResumoSelecao) {
        if outro.numeros > 0 {
            if self.numeros == 0 {
                (self.minimo, self.maximo) = (outro.minimo, outro.maximo);
            } else {
                self.minimo = self.minimo.min(outro.minimo);
                self.maximo = self.maximo.max(outro.maximo);
            }
        }
        self.contagem += outro.contagem;
        self.numeros += outro.numeros;
        self.soma += outro.soma;
        self.casas = self.casas.max(outro.casas);
    }

    /// As partes do rodapé, na ordem: "Contagem 33", e com algum número "Soma 98.231,40", "Média 8.930,13",
    /// "Mín. 154,03", "Máx. 17.810,34".
    pub fn partes(&self) -> Vec<String> {
        let mut partes = vec![format!("Contagem {}", formatar_numero(self.contagem))];
        if self.numeros > 0 {
            let casas = usize::from(self.casas);
            let numero = |n: f64| formatar_numero_com_casas(n, casas, casas);
            let media = self.soma / self.numeros as f64;
            partes.push(format!("Soma {}", numero(self.soma)));
            partes.push(format!(
                "Média {}",
                formatar_numero_com_casas(media, casas, casas.max(usize::from(CASAS_DA_MEDIA)))
            ));
            partes.push(format!("Mín. {}", numero(self.minimo)));
            partes.push(format!("Máx. {}", numero(self.maximo)));
        }
        partes
    }

    /// O rodapé inteiro, numa linha ("Contagem 33 · Soma 98.231,40 · …").
    pub fn texto(&self) -> String {
        self.partes().join(SEPARADOR)
    }
}

/// Quantas partes ficam à vista, na ordem em que saem: as cinco, sem o Mín. e o Máx., sem a Média (a contagem e a soma
/// ficam sempre). `cabe(n)` diz se as `n` primeiras cabem.
fn partes_que_cabem(total: usize, cabe: impl Fn(usize) -> bool) -> usize {
    [total, 3, 2].into_iter().filter(|&n| n <= total).find(|&n| cabe(n)).unwrap_or(total.min(2))
}

/// O rodapé da seleção: as contas em 12px `--t2`, na largura que sobra na linha (`ui.available_width()`); o que não
/// couber sai (primeiro o Mín. e o Máx., depois a Média) e vai para a dica. O leitor de tela ouve o texto inteiro.
pub fn rodape_da_selecao(ui: &mut Ui, resumo: &ResumoSelecao) -> Response {
    let cor = paleta(ui).estudio.t2;
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let partes = resumo.partes();
    let largura = ui.available_width();
    let texto_de = |n: usize| partes[..n].join(SEPARADOR);
    let n = partes_que_cabem(partes.len(), |n| galley(ui, &texto_de(n), fonte.clone(), cor).size().x <= largura + 0.5);
    let g = galley(ui, &texto_de(n), fonte, cor);
    let r = mostrar_galley(ui, g, Sense::hover());
    let inteiro = partes.join(SEPARADOR);
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &inteiro));
    if n < partes.len() { r.on_hover_text(partes[n..].join(SEPARADOR)) } else { r }
}

/// O rodapé da seleção enquanto a conta roda: "Somando…" com o ponto pulsante.
pub fn rodape_somando(ui: &mut Ui) -> Response {
    carregando(ui, SOMANDO)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contas_como_a_barra_do_excel() {
        let mut r = ResumoSelecao::default();
        for v in ["154,03", "17.810,34", "", "abc", "9,5"] {
            r.somar_do_csv(v, true, ',');
        }
        assert_eq!((r.contagem, r.numeros, r.casas), (4, 3, 2), "o vazio não conta; o texto conta e não soma");
        assert_eq!(
            r.texto(),
            "Contagem 4 · Soma 17.973,87 · Média 5.991,29 · Mín. 9,50 · Máx. 17.810,34",
            "as casas do número mais longo"
        );
        let mut t = ResumoSelecao::default();
        t.somar_do_csv("12", false, ',');
        t.somar_do_csv("x", false, ',');
        assert_eq!(t.texto(), "Contagem 2", "coluna de texto: só a contagem");
    }

    #[test]
    fn celulas_do_banco() {
        let mut r = ResumoSelecao::default();
        for c in
            [Celula::Inteiro(1), Celula::Inteiro(2), Celula::Nulo, Celula::Texto(String::new()), Celula::Inteiro(2)]
        {
            r.somar_celula(&c, true);
        }
        assert_eq!(r.texto(), "Contagem 3 · Soma 5 · Média 1,67 · Mín. 1 · Máx. 2", "a média de inteiros com 2 casas");
        let mut texto = ResumoSelecao::default();
        texto.somar_celula(&Celula::Inteiro(7), false);
        texto.somar_celula(&Celula::Texto("a".into()), false);
        assert_eq!(texto.texto(), "Contagem 2", "o tipo da coluna decide nos Dados");
        let mut reais = ResumoSelecao::default();
        reais.somar_celula(&Celula::Real(0.1), true);
        reais.somar_celula(&Celula::Real(0.2), true);
        assert_eq!(reais.texto(), "Contagem 2 · Soma 0,3 · Média 0,15 · Mín. 0,1 · Máx. 0,2", "sem o ruído do f64");
    }

    #[test]
    fn juntar_partes() {
        let (mut a, mut b) = (ResumoSelecao::default(), ResumoSelecao::default());
        a.somar_numero(5.0, 0);
        b.somar_texto("x");
        b.somar_numero(-1.25, 2);
        let mut c = ResumoSelecao::default();
        c.juntar(&a);
        c.juntar(&b);
        assert_eq!((c.contagem, c.numeros, c.soma, c.minimo, c.maximo, c.casas), (3, 2, 3.75, -1.25, 5.0, 2));
        c.juntar(&ResumoSelecao::default());
        assert_eq!(c.minimo, -1.25, "a parte sem números não mexe no mínimo");
    }

    #[test]
    fn o_que_sai_primeiro() {
        assert_eq!(partes_que_cabem(5, |n| n <= 5), 5);
        assert_eq!(partes_que_cabem(5, |n| n <= 4), 3, "sai o Mín. e o Máx. juntos");
        assert_eq!(partes_que_cabem(5, |n| n <= 2), 2, "depois a Média");
        assert_eq!(partes_que_cabem(5, |_| false), 2, "a contagem e a soma ficam sempre");
        assert_eq!(partes_que_cabem(1, |_| false), 1);
    }

    #[test]
    fn casas_dos_literais() {
        assert_eq!(casas_do_literal("17051"), 0);
        assert_eq!(casas_do_literal("12.50"), 2);
        assert_eq!(casas_do_literal("1e-7"), MAX_CASAS);
        assert_eq!(casas_do_literal("0.123456789"), MAX_CASAS);
    }
}
