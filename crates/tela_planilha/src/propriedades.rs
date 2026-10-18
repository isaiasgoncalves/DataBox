//! As seções das Propriedades da planilha (decisão 19 da etapa 6; prancheta 17, cena a): **Planilha {nome}** e
//! **Célula {A1}**. A "Arquivo" é a do `estado` (`secao_arquivo_com`) e a "Pasta de trabalho", com a caixa das ocultas,
//! fica na tela (ela troca a faixa).

use egui::{Id, Ui};
use interface::formato::formatar_numero;
use interface::icones::Icone;
use interface::pecas::botao::Botao;
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::{paragrafo, texto};
use interface::tema::paleta;
use planilha::{Categoria, Celula, Estado, Pasta, Valor};

use crate::fonte::{DECIMAL_PADRAO, texto_para_levar};

/// A nota das fórmulas.
pub const MSG_FORMULAS: &str = "Fórmulas não são recalculadas; o valor é o último que o Excel calculou.";
/// A nota do filtro do Excel.
pub const MSG_FILTRO: &str = "(as linhas que ele esconde aparecem esmaecidas)";
/// O botão da seção Célula.
pub const COPIAR_A_FORMULA: &str = "Copiar a fórmula";
/// A nota do `.xlsm` na seção Arquivo.
pub const MSG_MACROS: &str = "Macros não são executadas.";

/// O que a seção Planilha mostra, já em palavras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosDaPlanilha {
    pub nome: String,
    pub estado: &'static str,
    pub dimensao: String,
    pub linhas: String,
    pub colunas: String,
    pub mescladas: String,
    pub congelado: String,
    pub filtro: Option<String>,
    pub colunas_ocultas: String,
}

/// "visível", "oculta" ou "muito oculta".
pub fn estado_em_palavras(estado: Estado) -> &'static str {
    match estado {
        Estado::Visivel => "visível",
        Estado::Oculta => "oculta",
        Estado::MuitoOculta => "muito oculta",
    }
}

/// "1 linha e 1 coluna", "2 linhas", "3 colunas" ou "nenhum".
pub fn congelado_em_palavras(linhas: u32, colunas: u32) -> String {
    let l = (linhas > 0)
        .then(|| format!("{} {}", formatar_numero(linhas as usize), if linhas == 1 { "linha" } else { "linhas" }));
    let c = (colunas > 0)
        .then(|| format!("{} {}", formatar_numero(colunas as usize), if colunas == 1 { "coluna" } else { "colunas" }));
    match (l, c) {
        (Some(l), Some(c)) => format!("{l} e {c}"),
        (Some(x), None) | (None, Some(x)) => x,
        (None, None) => "nenhum".to_owned(),
    }
}

/// A categoria do formato em palavras ("Geral", "Percentual"…).
pub fn categoria_em_palavras(c: Categoria) -> &'static str {
    match c {
        Categoria::Geral => "Geral",
        Categoria::Numero => "Número",
        Categoria::Percentual => "Percentual",
        Categoria::Cientifico => "Científico",
        Categoria::Fracao => "Fração",
        Categoria::Data => "Data",
        Categoria::Hora => "Hora",
        Categoria::DataHora => "Data e hora",
        Categoria::Duracao => "Duração",
        Categoria::Texto => "Texto",
    }
}

/// O tipo do valor em palavras ("texto", "número", "data", "booleano", "erro", "vazio").
pub fn tipo_em_palavras(valor: &Valor) -> &'static str {
    match valor {
        Valor::Vazio => "vazio",
        Valor::Texto(_) => "texto",
        Valor::Numero(_) => "número",
        Valor::Booleano(_) => "booleano",
        Valor::Erro(_) => "erro",
        Valor::Data(_) => "data",
        Valor::Hora(_) => "hora",
        Valor::DataHora(_) => "data e hora",
    }
}

/// A seção "Planilha {nome}".
pub fn secao_planilha(ui: &mut Ui, sal: Id, d: &DadosDaPlanilha) {
    let _ = Secao::new(("propriedades::planilha", sal), "Planilha").extra(&d.nome).mostrar(ui, |ui| {
        ficha(ui, |f| {
            f.linha_texto("Nome", Some(&d.nome), "—");
            f.linha_texto("Estado", Some(d.estado), "—");
            f.linha_texto("Dimensão", Some(&d.dimensao), "—");
            f.linha_texto("Linhas", Some(&d.linhas), "—");
            f.linha_texto("Colunas", Some(&d.colunas), "—");
            f.linha_texto("Mescladas", Some(&d.mescladas), "—");
            f.linha_texto("Congelado", Some(&d.congelado), "—");
            if let Some(filtro) = &d.filtro {
                f.linha("Filtro do Excel", |ui| {
                    let e = &paleta(ui).estudio;
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        texto(ui, filtro, 12.0, e.t);
                        paragrafo(ui, MSG_FILTRO, 12.0, e.t3);
                    });
                });
            }
            f.linha_texto("Colunas ocultas", Some(&d.colunas_ocultas), "—");
        });
    });
}

/// A seção "Célula {A1}" da célula ativa: Referência, Tipo, Valor, Formato e, com fórmula, a Fórmula, o "Copiar a
/// fórmula" e a nota. `referencia` é a da célula (ou o intervalo da mescla). Devolve a fórmula a copiar, se o botão foi
/// clicado.
pub fn secao_celula(ui: &mut Ui, sal: Id, pasta: &Pasta, referencia: &str, celula: Option<&Celula>) -> Option<String> {
    let vazia = Celula::default();
    let c = celula.unwrap_or(&vazia);
    let tipo = tipo_em_palavras(&c.valor);
    let valor = texto_para_levar(pasta, c, DECIMAL_PADRAO);
    let formato = categoria_em_palavras(pasta.formato(c.estilo).categoria());
    let formula = c.formula.as_ref().map(|f| format!("= {f}"));
    let mut copiar = None;
    let _ = Secao::new(("propriedades::celula", sal), "Célula").extra(referencia).mostrar(ui, |ui| {
        ficha(ui, |f| {
            f.linha_texto("Referência", Some(referencia), "—");
            f.linha_texto("Tipo", Some(tipo), "—");
            f.linha_texto("Valor", (!valor.is_empty()).then_some(valor.as_str()), "—");
            f.linha_texto("Formato", Some(formato), "—");
            if let Some(fx) = &formula {
                f.linha_texto("Fórmula", Some(fx), "—");
            }
        });
        if let Some(fx) = &c.formula {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let largura = ui.available_width() - 8.0;
                ui.set_max_width(largura);
                if ui.add(Botao::ghost(COPIAR_A_FORMULA).com_icone(Icone::CopiarCaminho).lista()).clicked() {
                    copiar = Some(format!("={fx}"));
                }
            });
            let e = &paleta(ui).estudio;
            ui.add_space(6.0);
            // `padding:0 14px 14px`: o parágrafo na largura do painel, sem passar das bordas
            let r = egui::Rect::from_min_max(
                ui.cursor().min + egui::vec2(14.0, 0.0),
                egui::pos2(ui.max_rect().right() - 14.0, f32::INFINITY),
            );
            let mut dentro =
                ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
            paragrafo(&mut dentro, MSG_FORMULAS, 11.0, e.t3);
            let altura = dentro.min_rect().height() + 14.0;
            ui.allocate_space(egui::vec2(ui.available_width(), altura));
        }
    });
    copiar
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn palavras_das_propriedades() {
        assert_eq!(congelado_em_palavras(1, 1), "1 linha e 1 coluna");
        assert_eq!(congelado_em_palavras(2, 0), "2 linhas");
        assert_eq!(congelado_em_palavras(0, 3), "3 colunas");
        assert_eq!(congelado_em_palavras(0, 0), "nenhum");
        assert_eq!(estado_em_palavras(Estado::Oculta), "oculta");
        assert_eq!(categoria_em_palavras(Categoria::Percentual), "Percentual");
        assert_eq!(tipo_em_palavras(&Valor::Numero(0.98)), "número");
    }
}
