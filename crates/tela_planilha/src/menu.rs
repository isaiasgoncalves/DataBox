//! O menu do cabeçalho da planilha (prancheta 17, cena f): o clique direito no `th` (ou o Shift+F10 com a grade focada e
//! uma coluna escolhida) abre o menu de contexto de sempre com **Copiar coluna** (Ctrl+C; "Copiar 3 colunas") e **Trazer
//! colunas desta planilha**, que só liga com um CSV aberto como destino (senão, desligado com a dica "Abra um CSV para
//! trazer colunas."). Não há Colar nem Renomear: a planilha é só leitura.

use egui::{Response, Ui};
use interface::formato::formatar_numero;
use interface::icones::Icone;
use interface::pecas::menu_contexto::MenuContexto;

use crate::grade::Grade;

/// Os itens e a dica do item desligado.
pub const COPIAR_COLUNA: &str = "Copiar coluna";
pub const TRAZER_DESTA_PLANILHA: &str = "Trazer colunas desta planilha";
pub const MSG_ABRA_UM_CSV: &str = "Abra um CSV para trazer colunas.";

/// O que o menu pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedidoDoMenu {
    /// Copiar as colunas escolhidas (o mesmo do Ctrl+C).
    Copiar,
    /// Abrir o "Trazer colunas" do CSV com esta planilha como origem.
    Trazer,
}

/// "Copiar coluna" ou "Copiar 3 colunas".
pub fn rotulo_de_copiar(n: usize) -> String {
    if n <= 1 { COPIAR_COLUNA.to_owned() } else { format!("Copiar {} colunas", formatar_numero(n)) }
}

/// O menu do cabeçalho (chame a cada quadro, depois da grade). `aberto_no_th`: o clique direito deste quadro, na coluna
/// `c` da grade (escolhe a coluna se ela ainda não estava entre as escolhidas); `dono`: quem recebe o foco de volta (o
/// corpo da grade; o Shift+F10 vale com ele focado e alguma coluna escolhida), pedido só quando o menu precisa dele (ler a
/// resposta de um widget num quadro de clique em outro lugar tira o foco dele). `ha_csv`: há um CSV aberto para ser o
/// destino.
pub fn menu_do_cabecalho(
    ui: &Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug + Clone,
    grade: &mut Grade,
    aberto_no_th: Option<(usize, Response)>,
    dono: impl FnOnce() -> Option<Response>,
    ha_csv: bool,
) -> Option<PedidoDoMenu> {
    let aberto = MenuContexto::aberto(ui.ctx(), id_salt.clone());
    let dono = match aberto_no_th {
        Some((c, th)) => {
            let ja = grade.coluna_do_arquivo(c).is_some_and(|a| grade.colunas_selecionadas.contains(&a));
            if !ja {
                grade.clicar_coluna(c, interface::grade::Toques::default());
            }
            th
        }
        // pelo teclado: o Shift+F10 com o corpo da grade focado e alguma coluna escolhida (sem ele, o menu nem é montado)
        None if aberto || (!grade.colunas_selecionadas.is_empty() && shift_f10(ui)) => dono()?,
        None => return None,
    };
    let n = grade.colunas_selecionadas.len();
    let mut pedido = None;
    let _ = MenuContexto::new(id_salt).rotulo("Coluna").mostrar(&dono, |m| {
        if m.item(&rotulo_de_copiar(n), Some("Ctrl+C"), false).clicked() {
            pedido = Some(PedidoDoMenu::Copiar);
        }
        if ha_csv {
            if m.silhueta(Icone::TrazerColunas).item(TRAZER_DESTA_PLANILHA, None, false).clicked() {
                pedido = Some(PedidoDoMenu::Trazer);
            }
        } else {
            let r = m.silhueta(Icone::TrazerColunas).item_desligado(TRAZER_DESTA_PLANILHA, None);
            let _ = r.on_hover_text(MSG_ABRA_UM_CSV);
        }
    });
    pedido
}

/// O Shift+F10 (só com o Shift) foi apertado neste quadro.
fn shift_f10(ui: &Ui) -> bool {
    ui.input(|i| {
        i.events.iter().any(|e| {
            matches!(e, egui::Event::Key { key: egui::Key::F10, pressed: true, modifiers, .. }
                if modifiers.matches_exact(egui::Modifiers::SHIFT))
        })
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn rotulo_de_copiar_no_plural() {
        assert_eq!(rotulo_de_copiar(1), "Copiar coluna");
        assert_eq!(rotulo_de_copiar(3), "Copiar 3 colunas");
    }
}
