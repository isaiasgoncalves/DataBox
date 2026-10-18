//! Passo 12 da etapa 5: as colunas do CSV (seleção, menu do cabeçalho, nova, renomear, excluir, copiar e colar;
//! prancheta 16).

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

fn nomes(h: &H) -> Vec<String> {
    doc(h).retrato().unwrap().nomes.clone()
}

fn coluna(h: &H, c: usize) -> Vec<String> {
    let p = doc(h).pagina().unwrap();
    p.linhas.iter().map(|l| l.get(c).cloned().unwrap_or_default()).collect()
}

const ITENS_DO_MENU: [&str; 9] = [
    "Copiar coluna",
    "Colar como nova coluna",
    "Colar sobre esta coluna",
    "Colar com correspondência",
    "Trazer colunas de outro arquivo",
    "Renomear a coluna",
    "Nova coluna depois desta",
    "Excluir a coluna",
    "",
];

#[test]
fn clique_no_cabecalho_seleciona_a_coluna_e_ctrl_soma() {
    let amb = Ambiente::vazio("csv_sel_colunas");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    clicar_cabecalho(&mut h, 4, Modifiers::NONE);
    assert_eq!(doc(&h).colunas_selecionadas(), [4]);
    assert!(h.query_by_label("· coluna quantidade selecionada").is_some());
    clicar_cabecalho(&mut h, 5, Modifiers::CTRL);
    assert_eq!(doc(&h).colunas_selecionadas(), [4, 5]);
    assert!(h.query_by_label("· 2 colunas selecionadas").is_some());
    clicar_cabecalho(&mut h, 4, Modifiers::CTRL);
    assert_eq!(doc(&h).colunas_selecionadas(), [5], "Ctrl tira a que estava");
    clicar_cabecalho(&mut h, 2, Modifiers::SHIFT);
    // a âncora é a última do Ctrl (a 4, que saiu): o Shift vai dela até a 2
    assert_eq!(doc(&h).colunas_selecionadas(), [2, 3, 4], "Shift estende desde a âncora");
    // o clique numa célula desfaz a seleção de colunas
    clicar_celula(&mut h, 1, 1);
    assert!(doc(&h).colunas_selecionadas().is_empty());
}

#[test]
fn menu_do_cabecalho_tem_os_itens_da_prancheta_16_na_ordem() {
    let amb = Ambiente::vazio("csv_menu_itens");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    let pos = doc(&h).centro_cabecalho(5).unwrap();
    clicar_direito_em(&mut h, pos);
    assert_eq!(doc(&h).colunas_selecionadas(), [5], "o clique direito seleciona a coluna");
    let mut ys = Vec::new();
    for item in ITENS_DO_MENU.iter().filter(|i| !i.is_empty()) {
        let n = item_do_menu(&h, item);
        ys.push(n.rect().top());
    }
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "na ordem: {ys:?}");
    assert!(h.query_by_label("Renomear a coluna…").is_none(), "sem reticências");
}

#[test]
fn shift_f10_abre_o_menu_da_coluna() {
    let amb = Ambiente::vazio("csv_shift_f10");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    clicar_cabecalho(&mut h, 4, Modifiers::NONE);
    tecla(&mut h, Key::F10, Modifiers::SHIFT);
    h.run_steps(3);
    let item = item_do_menu(&h, "Copiar coluna");
    let th = doc(&h).centro_cabecalho(4).unwrap();
    assert!(item.rect().top() > th.y, "o menu abre embaixo do cabeçalho");
}

#[test]
fn copiar_coluna_vai_em_tsv_com_cabecalho() {
    let amb = Ambiente::vazio("csv_copiar_coluna");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(3)));
    clicar_cabecalho(&mut h, 4, Modifiers::NONE);
    h.input_mut().events.push(Event::Copy);
    h.step();
    assert_eq!(area_de_transferencia(&mut h).as_deref(), Some("quantidade\r\n1\r\n8\r\n15\r\n"));
    esperar_ociosa(&mut h);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Coluna quantidade copiada · 3 linhas"));
}

#[test]
fn copiar_tres_colunas_muda_o_item() {
    let amb = Ambiente::vazio("csv_copiar_tres");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(3)));
    clicar_cabecalho(&mut h, 1, Modifiers::NONE);
    clicar_cabecalho(&mut h, 3, Modifiers::SHIFT);
    let pos = doc(&h).centro_cabecalho(2).unwrap();
    clicar_direito_em(&mut h, pos);
    assert!(h.query_by_label("Copiar 3 colunas").is_some());
    assert!(h.query_by_label("Excluir 3 colunas").is_some());
    item_do_menu(&h, "Copiar 3 colunas").click();
    h.step();
    let texto = area_de_transferencia(&mut h).expect("copiou");
    assert!(texto.starts_with("regiao\tvendedor\tproduto\r\nSul\tAna Coutinho\tCaderno universitário\r\n"), "{texto}");
}

#[test]
fn nova_coluna_provisoria_enter_confirma_e_esc_desiste() {
    let amb = Ambiente::vazio("csv_nova_coluna");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    h.get_by_label("Nova coluna").click();
    h.run_steps(3);
    assert!(h.query(by().role(Role::TextInput).label("Nome da coluna")).is_some(), "o cabeçalho em edição");
    assert!(!ofertas_ligadas(&h).contains(&Verbo::Novo), "com a provisória, o resto espera");
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h).len(), 6, "Esc desiste");
    assert!(!doc(&h).alterado());
    h.get_by_label("Nova coluna").click();
    h.run_steps(3);
    digitar(&mut h, "obs");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h).last().map(String::as_str), Some("obs"));
    assert_eq!(doc(&h).alteracoes(), Some(1));
    assert!(coluna(&h, 6).iter().all(String::is_empty), "a coluna nova, vazia, não marca as células");
    assert!(!doc(&h).pagina().unwrap().alterada(0, 6));
}

fn ofertas_ligadas(h: &H) -> Vec<Verbo> {
    doc(h).ofertas(&h.state().estado).iter().filter(|(_, o)| o.ligado).map(|(v, _)| v).collect()
}

#[test]
fn nova_coluna_depois_da_ativa_e_no_fim() {
    let amb = Ambiente::vazio("csv_nova_coluna_lugar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::NovaColuna), "Nova coluna no fim");
    clicar_celula(&mut h, 0, 1);
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::NovaColuna), "Nova coluna depois de regiao");
    h.get_by_label("Nova coluna").click();
    h.run_steps(3);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h)[2], "coluna_3");
    // o "+" do fim do cabeçalho insere no fim
    h.get_by_label("Nova coluna no fim").click();
    h.run_steps(3);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h).last().map(String::as_str), Some("coluna_8"));
}

#[test]
fn renomear_pelo_f2_e_pela_secao() {
    let amb = Ambiente::vazio("csv_renomear");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_cabecalho(&mut h, 1, Modifiers::NONE);
    tecla(&mut h, Key::F2, Modifiers::NONE);
    assert!(h.query(by().role(Role::TextInput).label("Nome da coluna")).is_some());
    digitar(&mut h, "zona");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h)[1], "zona");
    // pela seção Coluna das Propriedades
    clicar_celula(&mut h, 0, 2);
    h.get_by_label("Renomear a coluna").click();
    h.run_steps(3);
    digitar(&mut h, "pessoa");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h)[2], "pessoa");
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::Desfazer), "Desfazer Renomear a coluna");
}

#[test]
fn sem_cabecalho_nao_renomeia_e_a_nova_coluna_entra_com_o_nome_generico() {
    let amb = Ambiente::vazio("csv_sem_cabecalho");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(4));
    let mut h = abrir(&amb, caminho.clone());
    h.get(by().role(Role::CheckBox).label("Primeira linha é o cabeçalho")).click();
    esperar_ociosa(&mut h);
    assert!(!doc(&h).retrato().unwrap().formato.cabecalho);
    assert_eq!(nomes(&h)[1], "coluna_2");
    // o F2 com a coluna selecionada não abre o nome (ele não seria gravado): a dica diz por quê
    clicar_cabecalho(&mut h, 1, Modifiers::NONE);
    tecla(&mut h, Key::F2, Modifiers::NONE);
    assert!(h.query(by().role(Role::TextInput).label("Nome da coluna")).is_none());
    let dica = "Ligue \"Primeira linha é o cabeçalho\" para dar nome às colunas.";
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some(dica));
    // o botão da seção Coluna, desligado
    let botao = h.query_all_by_label("Renomear a coluna").last().expect("o botão das Propriedades");
    assert!(botao.accesskit_node().is_disabled());
    // o item do menu do cabeçalho, desligado: clicar não abre nada
    let pos = doc(&h).centro_cabecalho(1).unwrap();
    clicar_direito_em(&mut h, pos);
    item_do_menu(&h, "Renomear a coluna").click();
    h.run_steps(3);
    assert!(h.query(by().role(Role::TextInput).label("Nome da coluna")).is_none());
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    // a Nova coluna entra direto, com o nome genérico (sem o cabeçalho em edição)
    h.get_by_label("Nova coluna").click();
    h.run_steps(3);
    assert!(h.query(by().role(Role::TextInput).label("Nome da coluna")).is_none());
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h).len(), 7);
    assert_eq!(doc(&h).alteracoes(), Some(1));
    h.get_by_label("Salvar").click();
    esperar_ociosa(&mut h);
    let gravado = std::fs::read_to_string(&caminho).unwrap();
    let primeira = gravado.lines().next().unwrap_or_default();
    assert_eq!(primeira.split(';').count(), 7, "a coluna nova também na primeira linha: {primeira}");
    assert!(primeira.starts_with("data;regiao;"), "a primeira linha continua sendo dados: {primeira}");
}

#[test]
fn excluir_coluna_entra_no_desfazer() {
    let amb = Ambiente::vazio("csv_excluir_coluna");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    let pos = doc(&h).centro_cabecalho(3).unwrap();
    clicar_direito_em(&mut h, pos);
    item_do_menu(&h, "Excluir a coluna").click();
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h), ["data", "regiao", "vendedor", "quantidade", "valor"]);
    assert!(doc(&h).colunas_selecionadas().is_empty());
    h.get_by_label("Desfazer").click();
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h).len(), 6);
    assert_eq!(coluna(&h, 3)[0], "Caderno universitário");
}

#[test]
fn colar_como_nova_coluna_um_passo_so() {
    let amb = Ambiente::vazio("csv_colar_nova");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_celula(&mut h, 0, 1);
    // texto de fora (o Excel): a primeira linha é texto sobre números, então é o nome
    colar(&mut h, "preco\r\n1,5\r\n2\r\n3\r\n4\r\n5\r\n");
    assert!(h.query(by().role(Role::TextInput).label("Nome da coluna")).is_some(), "o cabeçalho colado em edição");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h)[2], "preco");
    assert_eq!(coluna(&h, 2), ["1,5", "2", "3", "4", "5"]);
    assert_eq!(doc(&h).alteracoes(), Some(1), "uma alteração só");
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Coluna preco colada · 5 linhas"));
}

#[test]
fn colar_com_cabecalho_do_proprio_databox() {
    let amb = Ambiente::vazio("csv_colar_databox");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(4)));
    clicar_cabecalho(&mut h, 1, Modifiers::NONE);
    h.input_mut().events.push(Event::Copy);
    h.step();
    let texto = area_de_transferencia(&mut h).expect("copiou");
    esperar_ociosa(&mut h);
    // texto sobre texto: de fora seria coluna_N; do DataBox, a primeira linha é o nome
    colar(&mut h, &texto);
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h)[2], "regiao", "Esc mantém o nome sugerido");
    assert_eq!(coluna(&h, 2), ["Sul", "Sudeste", "Nordeste", "Norte"]);
    // o mesmo texto vindo de fora não tem cabeçalho
    let mut h2 = abrir(&amb, arquivo(&amb, "outro.csv", vendas(4)));
    clicar_celula(&mut h2, 0, 0);
    colar(&mut h2, "a\r\nb\r\nc\r\nd\r\n");
    tecla(&mut h2, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h2);
    assert_eq!(nomes(&h2)[1], "coluna_2");
}

#[test]
fn colar_com_quantidades_diferentes_mostra_o_aviso_e_acrescenta_linhas() {
    let amb = Ambiente::vazio("csv_colar_quantidades");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_celula(&mut h, 0, 5);
    colar(&mut h, "1\n2\n3\n4\n5\n6\n7\n");
    assert!(h.query_by_label("A coluna colada tem 7 valores e o arquivo, 5 linhas.").is_some());
    assert!(h.query_by_label("Colar 5").is_some());
    h.get_by_label("Acrescentar 2 linhas").click();
    h.run_steps(3);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 7);
    assert_eq!(coluna(&h, 6), ["1", "2", "3", "4", "5", "6", "7"]);
    assert_eq!(doc(&h).alteracoes(), Some(1), "acrescentar e colar num passo só");
    // Colar 5 corta os que sobram
    colar(&mut h, "x\ny\nz\nw\nv\nu\nt\ns\n");
    h.get_by_label("Colar 7").click();
    h.run_steps(3);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 7);
}

#[test]
fn colar_com_menos_valores_so_informa() {
    let amb = Ambiente::vazio("csv_colar_menos");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_celula(&mut h, 0, 5);
    colar(&mut h, "1\n2\n3\n");
    assert!(h.query_by_label("2 linhas ficaram vazias").is_some());
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(coluna(&h, 6), ["1", "2", "3", "", ""]);
    assert!(h.query_by_label("2 linhas ficaram vazias").is_none());
}

#[test]
fn colar_sobre_pergunta_substituir() {
    let amb = Ambiente::vazio("csv_colar_sobre");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    let pos = doc(&h).centro_cabecalho(4).unwrap();
    clicar_direito_em(&mut h, pos);
    item_do_menu(&h, "Colar sobre esta coluna").click();
    h.run_steps(2);
    // o RequestPaste do menu: o texto vem no Paste seguinte
    colar(&mut h, "9\n9\n9\n9\n9\n");
    esperar_ociosa(&mut h);
    assert!(h.query_by_label("Substituir 5 valores de quantidade?").is_some());
    h.get_by_label("Substituir").click();
    esperar_ociosa(&mut h);
    assert_eq!(coluna(&h, 4), ["9", "9", "9", "9", "9"]);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Coluna quantidade colada · 5 linhas"));
}

#[test]
fn cola_e_desfazer_voltam_o_arquivo() {
    let amb = Ambiente::vazio("csv_cola_desfazer");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_celula(&mut h, 0, 5);
    colar(&mut h, "1\n2\n3\n4\n5\n6\n");
    h.get_by_label("Acrescentar 1 linha").click();
    h.run_steps(3);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::Desfazer), "Desfazer Colar a coluna");
    h.get_by_label("Desfazer").click();
    esperar_ociosa(&mut h);
    assert_eq!(nomes(&h).len(), 6);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 5);
    assert!(!doc(&h).alterado());
}

#[test]
fn selecao_grande_demais_nao_copia() {
    let amb = Ambiente::vazio("csv_grande_demais");
    let cabecalho: Vec<String> = (1..=20).map(|i| format!("c{i}")).collect();
    let linha = vec!["1"; 20].join(";");
    let mut s = cabecalho.join(";");
    s.push('\n');
    for _ in 0..260_000 {
        s.push_str(&linha);
        s.push('\n');
    }
    let mut h = abrir(&amb, arquivo(&amb, "largo.csv", s));
    clicar_cabecalho(&mut h, 0, Modifiers::NONE);
    clicar_cabecalho(&mut h, 4, Modifiers::SHIFT);
    // 5 colunas × 260.000 linhas cabem; todas as 20 não
    let tudo: Vec<usize> = (0..20).collect();
    doc_mut(&mut h).selecionar_colunas(tudo);
    h.input_mut().events.push(Event::Copy);
    h.step();
    h.run_steps(2);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("A seleção é grande demais para copiar (5.200.000 células)."));
}

#[test]
fn capturas_do_menu_e_do_aviso_contra_a_prancheta_16() {
    let amb = Ambiente::vazio("csv_capturas_colunas");
    let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)), Tema::Escuro, TAMANHO);
    let pos = doc(&h).centro_cabecalho(5).unwrap();
    clicar_direito_em(&mut h, pos);
    capturar(&mut h, "menu-cabecalho-escuro.png");
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    clicar_celula(&mut h, 0, 1);
    let valores: String = (0..1210).map(|i| format!("{},{:02}\n", 10 + i % 300, i % 100)).collect();
    colar(&mut h, &format!("valor\n{valores}"));
    capturar(&mut h, "colar-aviso-escuro.png");
    h.get_by_label("Colar 1.204").click();
    h.run_steps(3);
    capturar(&mut h, "colar-cabecalho-em-edicao-escuro.png");
}
