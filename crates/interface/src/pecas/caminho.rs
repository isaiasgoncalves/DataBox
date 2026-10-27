//! O Caminho de um arquivo ou banco (D3 da revisão de 26/10/2026): uma peça só para as Propriedades, a ficha e as
//! faixas, no lugar dos três desenhos de antes (uma linha com o Copiar no banco, o caminho inteiro quebrado em três
//! linhas na tabela, o nome cortado no CSV).
//!
//! - [`caminho`]: o valor do Caminho da ficha ([`crate::pecas::detalhes::Detalhes::linha_caminho`] põe o rótulo em
//!   cima, para o caminho ter a largura inteira da seção). Em até duas linhas de 12px / 350 em `--t`, quebradas entre
//!   as pastas; o que ainda não couber sai do começo ("…/dados/" em cima, "vendas_2025.csv" embaixo), e o nome fica
//!   inteiro (o nome que não cabe sozinho é cortado no meio, com a extensão). O `.ib.pq` Copiar caminho fica na ponta
//!   da última linha. O caminho inteiro vai na dica (quando cortado) e para o leitor de tela.
//! - [`caminho_na_faixa`]: a forma de uma linha em `--t3`, sem o Copiar, da faixa do topo do documento (o leitor só
//!   leitura e a planilha), com a mesma regra de corte.
//!
//! A regra do corte é a U6 (revisão de UI de 22/10/2026): [`linhas_do_caminho`], sobre o
//! [`crate::pecas::texto::encurtar_caminho`] e o [`crate::pecas::texto::encurtar_nome`].

use std::sync::Arc;

use egui::{Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};

use crate::fontes::{self, Peso};
use crate::icones::Icone;
use crate::pecas::botao::{Botao, LADO_PEQUENO};
use crate::pecas::texto::{encurtar_nome, galley, galley_caminho, galley_cortado, y_na_linha};
use crate::tema::paleta;

/// A altura de cada linha do caminho (`line-height:1.4` em 12px, a das fichas).
pub const ALTURA_LINHA: f32 = 17.0;
/// O espaço entre o fim do caminho e o Copiar.
const ESPACO_COPIAR: f32 = 4.0;
/// O rótulo do botão e a dica dele.
pub const ROTULO_COPIAR: &str = "Copiar caminho";

/// As linhas em que o caminho é mostrado, no máximo duas. `cabe` mede a primeira linha (a largura inteira) e
/// `cabe_ultima`, a última (sem o Copiar). Inteiro numa linha, se couber; senão, quebrado depois de um separador, com
/// a segunda linha terminando no nome; o que não couber nas duas sai do começo da primeira ("…/dados/"). O nome que
/// não cabe sozinho na última é cortado no meio, com a extensão.
pub fn linhas_do_caminho(
    caminho: &str,
    cabe: impl Fn(&str) -> bool,
    cabe_ultima: impl Fn(&str) -> bool,
) -> Vec<String> {
    if cabe_ultima(caminho) {
        return vec![caminho.to_owned()];
    }
    // os pedaços, cada um com o separador do fim: "/", "home/", "dados/", "vendas.csv"
    let mut pedacos: Vec<&str> = Vec::new();
    let mut inicio = 0;
    for (i, ch) in caminho.char_indices() {
        if ch == '/' || ch == '\\' {
            pedacos.push(&caminho[inicio..i + ch.len_utf8()]);
            inicio = i + ch.len_utf8();
        }
    }
    if inicio < caminho.len() {
        pedacos.push(&caminho[inicio..]);
    }
    let Some(&nome) = pedacos.last() else { return vec![caminho.to_owned()] };
    let sep = caminho.rfind(['\\', '/']).and_then(|i| caminho[i..].chars().next()).unwrap_or('/');
    // a última linha: o nome e as pastas mais perto dele que couberem
    let mut k = pedacos.len() - 1;
    let ultima = if cabe_ultima(nome) {
        while k > 0 && cabe_ultima(&pedacos[k - 1..].concat()) {
            k -= 1;
        }
        pedacos[k..].concat()
    } else {
        encurtar_nome(nome, &cabe_ultima).unwrap_or_else(|| nome.to_owned())
    };
    if k == 0 {
        return vec![ultima];
    }
    // a primeira: o resto, inteiro ou sem o começo
    let resto = &pedacos[..k];
    let inteira = cabe(&resto.concat()).then(|| resto.concat());
    let cauda = || (1..resto.len()).map(|j| format!("…{sep}{}", resto[j..].concat())).find(|s| cabe(s));
    // nem a pasta-mãe inteira cabe: ela cortada no meio ("…/tmp-a…9f3/")
    let mae = || {
        let pasta = resto.last()?.trim_end_matches(['/', '\\']);
        encurtar_nome(pasta, |x| cabe(&format!("…{sep}{x}{sep}"))).map(|x| format!("…{sep}{x}{sep}"))
    };
    match inteira.or_else(cauda).or_else(mae) {
        Some(primeira) => vec![primeira, ultima],
        None => vec![ultima],
    }
}

/// O Caminho na ficha (ver o topo do módulo), na largura de `ui`. Devolve se o Copiar caminho foi clicado (quem chama
/// copia e dá o toast).
pub fn caminho(ui: &mut Ui, caminho: &str) -> bool {
    let e = &paleta(ui).estudio;
    let fonte = fontes::fonte(12.0, Peso::Leve);
    let largura = ui.available_width();
    let ultima_cabe = (largura - LADO_PEQUENO - ESPACO_COPIAR).max(0.0);
    let medir = |s: &str| galley(ui, s, fonte.clone(), e.t).size().x;
    let linhas = linhas_do_caminho(caminho, |s| medir(s) <= largura + 0.5, |s| medir(s) <= ultima_cabe + 0.5);
    // o que nem cortado coube (uma pasta só, enorme) sai com "…" no fim
    let n = linhas.len();
    let galleys: Vec<_> = linhas
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let cabe = if i + 1 == n { ultima_cabe } else { largura };
            if medir(l) <= cabe + 0.5 {
                galley(ui, l, fonte.clone(), e.t)
            } else {
                galley_cortado(ui, l, fonte.clone(), e.t, cabe)
            }
        })
        .collect();
    let encurtado = linhas.concat() != caminho;
    let topo = ui.cursor().min;
    let largura_texto = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let altura = ALTURA_LINHA * galleys.len() as f32;
    let (rect, resp) = ui.allocate_exact_size(vec2(largura_texto.min(largura), altura), Sense::hover());
    for (i, g) in galleys.iter().enumerate() {
        let y = y_na_linha(topo.y + ALTURA_LINHA * i as f32, ALTURA_LINHA, g);
        ui.painter().galley(pos2(rect.left(), y), Arc::clone(g), e.t);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, caminho));
    if encurtado {
        resp.on_hover_text(caminho);
    }
    // o Copiar na ponta da última linha, centrado nela, sem esticar a linha
    let fim = galleys.last().map_or(0.0, |g| g.size().x);
    let centro = pos2(rect.left() + fim + ESPACO_COPIAR + LADO_PEQUENO / 2.0, topo.y + altura - ALTURA_LINHA / 2.0);
    let mut ui_botao = ui.new_child(
        UiBuilder::new()
            .id_salt(("databox::caminho", caminho))
            .max_rect(Rect::from_center_size(centro, vec2(LADO_PEQUENO, LADO_PEQUENO))),
    );
    ui_botao.add(Botao::icone(Icone::CopiarCaminho).pequeno().rotulo(ROTULO_COPIAR).dica(ROTULO_COPIAR)).clicked()
}

/// O caminho numa linha em `--t3` (a faixa do topo do documento), em `largura`, cortado como o da ficha; o caminho
/// inteiro na dica quando cortado.
pub fn caminho_na_faixa(ui: &mut Ui, caminho: &str, largura: f32) -> egui::Response {
    let e = &paleta(ui).estudio;
    let g = galley_caminho(ui, caminho, fontes::fonte(12.0, Peso::Leve), e.t3, largura);
    let encurtado = g.text() != caminho;
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, e.t3);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, caminho));
    if encurtado { resp.on_hover_text(caminho) } else { resp }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ate(n: usize) -> impl Fn(&str) -> bool {
        move |s: &str| s.chars().count() <= n
    }

    #[test]
    fn inteiro_quando_cabe_e_em_duas_linhas_quebrado_nas_pastas() {
        let c = "/home/isaias/dados/vendas_2025.csv";
        assert_eq!(linhas_do_caminho(c, ate(40), ate(36)), [c]);
        // as pastas de cima em cima, o nome e as mais perto dele embaixo
        let duas = |a: &str, b: &str| vec![a.to_string(), b.to_string()];
        assert_eq!(linhas_do_caminho(c, ate(26), ate(22)), duas("/home/isaias/", "dados/vendas_2025.csv"));
        // o que não cabe sai do começo da primeira
        assert_eq!(linhas_do_caminho(c, ate(10), ate(16)), duas("…/dados/", "vendas_2025.csv"));
        // o nome que não cabe sozinho: no meio, com a extensão
        assert_eq!(linhas_do_caminho(c, ate(30), ate(10)), duas("/home/isaias/dados/", "ven…25.csv"));
        // a pasta-mãe que não cabe inteira: cortada no meio, sem perder a linha
        let t = "/tmp/abcdefghijklmnopqrstuvwxyz/exemplo.db";
        assert_eq!(linhas_do_caminho(t, ate(14), ate(12)), duas("…/abcde…vwxyz/", "exemplo.db"));
        // no Windows, com a unidade
        let w = r"C:\Users\ana\dados\vendas.db";
        assert_eq!(linhas_do_caminho(w, ate(30), ate(15)), duas(r"C:\Users\ana\", r"dados\vendas.db"));
    }
}
