//! O passo a mais da faixa G: filtrar e ordenar o CSV (a faixa com as condições, a seta do cabeçalho, a vista numa
//! tarefa pelo `Leitor::varrer`; com filtro ou ordem, os Colar por posição desligados).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Documento, Verbo};
use interface::pecas::toast::toast_atual;
use interface::tema::Tema;
use tela_csv::OperadorCsv;

fn posicoes(h: &H) -> Vec<u64> {
    doc(h).pagina().map(|p| p.posicoes.clone()).unwrap_or_default()
}

fn coluna(h: &H, c: usize) -> Vec<String> {
    doc(h).pagina().unwrap().linhas.iter().map(|l| l.get(c).cloned().unwrap_or_default()).collect()
}

#[test]
fn filtro_pela_faixa_e_o_popover() {
    let amb = Ambiente::vazio("filtro_popover");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(20)));
    assert!(!doc(&h).anatomia().faixa, "sem a faixa: a barra dos filtros fica no alto (0.2.3, item F)");
    assert!(h.query_by_label("Filtros").is_some(), "a barra dos filtros, mesmo sem filtro");
    assert!(h.query_by_label_contains("filtradas de").is_none(), "sem filtro, o rodapé só conta as linhas");
    clicar_celula(&mut h, 0, 1);
    h.get_by_label("Filtrar").click();
    h.run_steps(3);
    assert!(h.query_by_label("Condição de filtro").is_some(), "o popover");
    h.get(by().role(Role::ComboBox).label("Operador da condição")).click();
    h.run_steps(2);
    h.get_by_label("igual a").click();
    h.run_steps(2);
    let valor = h.get(by().role(Role::TextInput).label("Valor da condição"));
    valor.focus();
    h.run_steps(1);
    h.get(by().role(Role::TextInput).label("Valor da condição")).type_text("Sul");
    h.run_steps(2);
    h.get_by_label("Aplicar").click();
    esperar_ociosa(&mut h);
    // regiao = Sul: as linhas 1, 6, 11 e 16 (a posição no arquivo continua no número da linha)
    assert_eq!(posicoes(&h), [0, 5, 10, 15]);
    assert!(coluna(&h, 1).iter().all(|r| r == "Sul"));
    assert_eq!(doc(&h).linhas_filtradas(), Some(4));
    assert!(h.query_by_label("Linhas 1–4 de 4").is_some(), "o rodapé: as linhas da vista");
    assert!(h.query_by_label("· filtradas de 20").is_some(), "e de quantas o arquivo tem, no rodapé (U3)");
    assert!(h.query_by_label("4 de 20 linhas").is_none(), "a barra dos filtros não conta mais");
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::Filtrar), "Filtrar (1 condição)");
    assert!(doc(&h).ofertas(&h.state().estado).get(Verbo::Filtrar).marcado);
}

#[test]
fn tirar_o_filtro_pelo_x_e_limpar() {
    let amb = Ambiente::vazio("filtro_tirar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(20)));
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Sul");
    doc_mut(&mut h).filtrar("quantidade", OperadorCsv::Maior, "5");
    esperar_ociosa(&mut h);
    assert_eq!(posicoes(&h), [5, 15]);
    h.get_by_label("Limpar filtros").click();
    esperar_ociosa(&mut h);
    assert!(doc(&h).condicoes().is_empty());
    assert_eq!(doc(&h).linhas_filtradas(), None);
    assert_eq!(posicoes(&h).len(), 20);
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Norte");
    esperar_ociosa(&mut h);
    h.get_by_label("Tirar o filtro regiao igual a Norte").click();
    esperar_ociosa(&mut h);
    assert_eq!(posicoes(&h).len(), 20);
}

#[test]
fn ordenar_pela_seta_do_cabecalho() {
    let amb = Ambiente::vazio("filtro_ordem");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    // a seta aparece com o ponteiro sobre o cabeçalho
    let pos = doc(&h).centro_cabecalho(4).unwrap();
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.run_steps(2);
    h.get_by_label("Ordenar por quantidade").click();
    esperar_ociosa(&mut h);
    let q: Vec<u32> = coluna(&h, 4).iter().map(|v| v.parse().unwrap()).collect();
    assert!(q.windows(2).all(|w| w[0] <= w[1]), "crescente pelo valor: {q:?}");
    h.get_by_label("Ordenar por quantidade").click();
    esperar_ociosa(&mut h);
    let q: Vec<u32> = coluna(&h, 4).iter().map(|v| v.parse().unwrap()).collect();
    assert!(q.windows(2).all(|w| w[0] >= w[1]), "decrescente: {q:?}");
    assert!(doc(&h).ordem().is_some_and(|o| o.decrescente));
    h.get_by_label("Ordenar por quantidade").click();
    esperar_ociosa(&mut h);
    assert!(doc(&h).ordem().is_none(), "a terceira vez tira a ordem");
    assert_eq!(posicoes(&h), (0..10).collect::<Vec<u64>>());
}

#[test]
fn editar_com_filtro_grava_na_linha_certa() {
    let amb = Ambiente::vazio("filtro_editar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(20)));
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Sul");
    esperar_ociosa(&mut h);
    // a segunda linha da vista é a posição 5 do arquivo
    clicar_celula(&mut h, 1, 2);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    digitar(&mut h, "Fulano");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Contem, "");
    h.get_by_label("Limpar filtros").click();
    esperar_ociosa(&mut h);
    assert_eq!(coluna(&h, 2)[5], "Fulano");
    assert_eq!(coluna(&h, 2)[1], "Bruno Brandão", "as outras ficam");
    // excluir com filtro: a vista é refeita sem a linha
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Sul");
    esperar_ociosa(&mut h);
    doc_mut(&mut h).marcar_linhas([10]);
    h.run_steps(2);
    let o = doc(&h).ofertas(&h.state().estado);
    assert!(o.ligado(Verbo::Excluir), "{:?}", o.get(Verbo::Excluir));
    h.get_by_label("Excluir").click();
    esperar_ociosa(&mut h);
    assert_eq!(posicoes(&h), [0, 5, 14], "a vista acompanha a exclusão");
}

#[test]
fn colar_por_posicao_desligado_com_filtro_ou_ordem() {
    let amb = Ambiente::vazio("filtro_colar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    assert!(doc(&h).por_posicao_permitida());
    doc_mut(&mut h).ordenar("quantidade");
    esperar_ociosa(&mut h);
    assert!(!doc(&h).por_posicao_permitida());
    let pos = doc(&h).centro_cabecalho(1).unwrap();
    clicar_direito_em(&mut h, pos);
    let item = item_do_menu(&h, "Colar como nova coluna");
    assert!(item.accesskit_node().is_disabled(), "o Colar por posição desligado");
    assert!(!item_do_menu(&h, "Colar com correspondência").accesskit_node().is_disabled(), "com correspondência vale");
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    // o Ctrl+V com a coluna escolhida explica
    clicar_cabecalho(&mut h, 1, Modifiers::NONE);
    colar(&mut h, "a\nb\n");
    assert_eq!(
        toast_atual(&h.ctx).as_deref(),
        Some("Tire o filtro e a ordenação para colar por posição, ou use Colar com correspondência.")
    );
    assert_eq!(doc(&h).retrato().unwrap().colunas(), 6);
}

/// A captura do fechamento das etapas 5 e 6, a 1280×800: o CSV filtrado (a barra com a condição e "filtradas de 1.180"
/// no rodapé) e ordenado pela quantidade.
#[test]
fn captura_do_csv_filtrado_do_fechamento() {
    let amb = Ambiente::vazio("filtro_captura_56");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(1180));
    let mut h = abrir_com(&amb, caminho, Tema::Escuro, egui::vec2(1280.0, 800.0));
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Sul");
    esperar_ociosa(&mut h);
    doc_mut(&mut h).ordenar("quantidade");
    esperar_ociosa(&mut h);
    capturar(&mut h, "etapa56-csv-filtrado-escuro.png");
}

#[test]
fn captura_do_csv_filtrado_contra_a_prancheta_16_b() {
    let amb = Ambiente::vazio("filtro_captura");
    let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1180)), Tema::Escuro, TAMANHO);
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Sul");
    esperar_ociosa(&mut h);
    let pos = doc(&h).centro_cabecalho(1).unwrap();
    clicar_direito_em(&mut h, pos);
    let item = item_do_menu(&h, "Colar como nova coluna").rect().center();
    h.input_mut().events.push(Event::PointerMoved(item));
    h.run_steps(4);
    capturar(&mut h, "csv-filtrado-escuro.png");
}

/// O centro do botão mais embaixo com este rótulo (o do vazio, abaixo do da barra dos filtros ou da barra de
/// ferramentas).
fn botao_de_baixo(h: &H, rotulo: &str) -> egui::Pos2 {
    h.query_all_by_role_and_label(Role::Button, rotulo)
        .map(|n| n.rect())
        .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
        .unwrap_or_else(|| panic!("o botão {rotulo}"))
        .center()
}

#[test]
fn filtro_sem_linhas_mostra_o_vazio_com_limpar_filtros() {
    let amb = Ambiente::vazio("filtro_vazio");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(20)));
    doc_mut(&mut h).filtrar("regiao", OperadorCsv::Igual, "Antártida");
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).linhas_filtradas(), Some(0));
    // o cabeçalho continua à vista, e o vazio vai embaixo dele (U5 da revisão de UI de 22/10/2026)
    let frase = h.get_by_label("Nenhuma linha com estes filtros.").rect();
    assert!(h.query_all_by_label("regiao").any(|n| n.rect().bottom() <= frase.top()), "o cabeçalho acima");
    assert_eq!(h.query_all_by_role_and_label(Role::Button, "Limpar filtros").count(), 2, "o da barra e o do vazio");
    let centro = botao_de_baixo(&h, "Limpar filtros");
    h.input_mut().events.push(Event::PointerMoved(centro));
    h.input_mut().events.push(Event::PointerButton {
        pos: centro,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.input_mut().events.push(Event::PointerButton {
        pos: centro,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    esperar_ociosa(&mut h);
    assert!(h.query_by_label("Nenhuma linha com estes filtros.").is_none());
    assert_eq!(posicoes(&h).len(), 20, "sem filtro, as 20 linhas");
}

#[test]
fn arquivo_so_com_o_cabecalho_oferece_nova_linha() {
    let amb = Ambiente::vazio("so_cabecalho");
    let mut h = abrir(&amb, arquivo(&amb, "vazio.csv", "nome;valor\n"));
    esperar_ociosa(&mut h);
    let frase = h.get_by_label("Nenhuma linha neste arquivo.").rect();
    assert!(h.query_all_by_label("nome").any(|n| n.rect().bottom() <= frase.top()), "o cabeçalho acima");
    let centro = botao_de_baixo(&h, "Nova linha");
    assert!(centro.y > frase.bottom(), "o Nova linha do vazio, embaixo da frase");
    capturar(&mut h, "csv-vazio-claro.png");
    h.input_mut().events.push(Event::PointerMoved(centro));
    h.input_mut().events.push(Event::PointerButton {
        pos: centro,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.input_mut().events.push(Event::PointerButton {
        pos: centro,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    esperar_ociosa(&mut h);
    assert!(h.query_by_label("Nenhuma linha neste arquivo.").is_none(), "a linha nova entrou");
    assert!(doc(&h).alterado());
}
