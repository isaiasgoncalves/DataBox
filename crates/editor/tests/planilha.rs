//! A planilha do Excel na casca (etapa 6, passo 7): o `.xlsx` abre na `tela_planilha` pela árvore (a ação do
//! Explorador), pelo caminho (o Ctrl+O e os recentes) e ao soltar; a planilha entra nas origens do "Trazer colunas"
//! (`Contexto::fontes`); a barra de status diz "Somente leitura"; e a volta do foco avisa do arquivo que mudou.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]
// sem o recurso `telas`, o Excel abre na ficha (o `pasta.rs` confere): nada daqui vale
#![cfg(feature = "telas")]

mod apoio;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use apoio::*;
use comum::{Resultado, Valor};
use editor::{Editor, Partida};
use egui::Ui;
use egui::accesskit::Role;
use estado::apoio_testes::Ambiente;
use estado::{Acao, Contexto, Documento, Rota};
use interface::tema::Tema;

/// Um `.xlsx` de verdade (o exportador do DataBox), com a planilha "dados": id, nome e valor em `n` linhas.
fn xlsx(amb: &Ambiente, nome: &str, n: i64) -> PathBuf {
    let pasta = amb.pasta().join("planilhas");
    std::fs::create_dir_all(&pasta).expect("a pasta");
    let destino = pasta.join(nome);
    let colunas = ["id".to_string(), "nome".to_string(), "valor".to_string()];
    let linhas: Vec<Resultado<Vec<Valor>>> = (1..=n)
        .map(|i| Ok(vec![Valor::Inteiro(i), Valor::Texto(format!("item {i}")), Valor::Real(i as f64 * 1.5)]))
        .collect();
    exportar::exportar_linhas(
        &colunas,
        linhas,
        exportar::Formato::Xlsx,
        &destino,
        &exportar::Opcoes::default(),
        "dados",
        exportar::Acompanhamento::default(),
    )
    .expect("exportar o xlsx");
    destino
}

/// Espera a planilha abrir (o chip "Somente leitura" na faixa e a aba da planilha "dados").
fn esperar_planilha(h: &mut H) {
    esperar(h, "a planilha abrir", |h| existe(h, Role::Tab, "dados, planilha 1 de 1"));
    assentar(h);
}

#[test]
fn abrir_pela_arvore_pelo_ctrl_o_pelos_recentes_e_ao_soltar() {
    let amb = Ambiente::vazio("planilha_abrir");
    let caminho = xlsx(&amb, "relatorio_q3.xlsx", 30);
    // a árvore (o Explorador emite a ação do arquivo de dados)
    let mut h = harness(&amb);
    h.state_mut().emitir(Acao::AbrirArquivoDeDados { caminho: caminho.clone() });
    h.run_steps(2);
    esperar_planilha(&mut h);
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho: c } if c.ends_with("relatorio_q3.xlsx")));
    assert!(abas(&h).contains(&("relatorio_q3.xlsx".to_string(), true)), "{:?}", abas(&h));
    assert!(tem_texto(&h, "Somente leitura"));
    assert!(!tem_texto(&h, "O editor de Excel ainda não está pronto"));
    // o caminho (o Ctrl+O e os recentes chegam aqui com o arquivo escolhido)
    let mut h = harness(&amb);
    h.state_mut().emitir(Acao::AbrirCaminho { caminho: caminho.clone(), somente_leitura: false });
    h.run_steps(2);
    esperar_planilha(&mut h);
    assert!(matches!(h.state().rota(), Rota::Arquivo { .. }));
    // ao soltar na janela
    let mut h = harness(&amb);
    h.input_mut().dropped_files.push(Arc::new(Solto(caminho.clone())));
    h.run_steps(2);
    esperar_planilha(&mut h);
    assert!(abas(&h).contains(&("relatorio_q3.xlsx".to_string(), true)));
    // o .xlsm também
    let xlsm = caminho.with_extension("xlsm");
    std::fs::copy(&caminho, &xlsm).expect("copiar");
    h.state_mut().emitir(Acao::AbrirArquivoDeDados { caminho: xlsm });
    h.run_steps(2);
    esperar(&mut h, "a aba do xlsm", |h| abas(h).iter().any(|(n, ativa)| n == "relatorio_q3.xlsm" && *ativa));
    esperar_planilha(&mut h);
    assert!(tem_texto(&h, "Macros não são executadas."));
}

#[derive(Debug)]
struct Solto(PathBuf);

impl egui::DroppedFile for Solto {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

/// Um CSV de mentira que anota as origens que o `Contexto` lhe dá.
struct Espiao(Rc<RefCell<Vec<String>>>);

impl Documento for Espiao {
    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        *self.0.borrow_mut() = ctx.fontes().iter().map(|f| f.rotulo()).collect();
        ui.label("O CSV");
    }
}

#[test]
fn fonte_de_planilha_aparece_no_seletor_do_trazer_colunas() {
    let amb = Ambiente::vazio("planilha_fonte");
    let planilha = xlsx(&amb, "relatorio_q3.xlsx", 12);
    let csv = amb.pasta().join("planilhas").join("vendas_2025.csv");
    std::fs::write(&csv, "regiao;valor\nSul;1\n").expect("o csv");
    let vistas = Rc::new(RefCell::new(Vec::new()));
    let anotar = Rc::clone(&vistas);
    let editor = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(|_| None)
        .com_editor_de_teste(move |caminho: &Path| {
            let e_csv = caminho.extension().is_some_and(|e| e == "csv");
            e_csv.then(|| Box::new(Espiao(Rc::clone(&anotar))) as Box<dyn Documento>)
        });
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    h.state_mut().ir(Rota::arquivo(&planilha));
    h.run_steps(2);
    esperar_planilha(&mut h);
    h.state_mut().ir(Rota::arquivo(&csv));
    h.run_steps(3);
    // a planilha da outra aba está entre as origens, com o rótulo "arquivo › planilha"
    esperar(&mut h, "as origens", |_| vistas.borrow().iter().any(|r| r == "relatorio_q3.xlsx › dados"));
    assert!(vistas.borrow().iter().any(|r| r == "relatorio_q3.xlsx › dados"), "{:?}", vistas.borrow());
}

/// O centro da letra `letra` pintada no cabeçalho da planilha.
fn centro_da_letra(h: &H, letra: &str) -> egui::Pos2 {
    h.output()
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == letra => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a letra {letra}"))
}

fn clique_direito(h: &mut H, pos: egui::Pos2) {
    use egui::{Event, Modifiers, PointerButton};
    h.hover_at(pos);
    h.event(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
    h.event(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

#[test]
fn trazer_colunas_desta_planilha_abre_a_janela_do_csv_com_a_origem() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let amb = Ambiente::vazio("planilha_trazer_desta");
    let planilha = xlsx(&amb, "relatorio_q3.xlsx", 12);
    let csv = amb.pasta().join("planilhas").join("vendas_2025.csv");
    std::fs::write(&csv, "id;regiao\n1;Sul\n2;Norte\n").expect("o csv");
    let mut h = harness(&amb);
    // sem CSV aberto, o item fica desligado (a dica diz o que fazer)
    h.state_mut().ir(Rota::arquivo(&planilha));
    h.run_steps(2);
    esperar_planilha(&mut h);
    let pos = centro_da_letra(&h, "B");
    clique_direito(&mut h, pos);
    assert!(h.get_by_label("Trazer colunas desta planilha").accesskit_node().is_disabled());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    // as abas ficam outro.csv, vendas_2025.csv e a planilha: a primeira origem de vendas_2025.csv seria o outro.csv
    let outro = amb.pasta().join("planilhas").join("outro.csv");
    std::fs::write(&outro, "id;nome\n1;a\n").expect("o outro csv");
    h.state_mut().ir(Rota::arquivo(&outro));
    h.run_steps(2);
    esperar(&mut h, "o outro CSV abrir", |h| tem_texto(h, "Linhas 1–1 de 1"));
    fechar_aba(&mut h, "relatorio_q3.xlsx");
    assentar(&mut h);
    h.state_mut().ir(Rota::arquivo(&csv));
    h.run_steps(2);
    esperar(&mut h, "o CSV abrir", |h| tem_texto(h, "Linhas 1–2 de 2"));
    // com o CSV aberto (lido) em outra aba, o item liga e leva à janela do CSV usado por último, com a planilha
    // escolhida
    h.state_mut().ir(Rota::arquivo(&planilha));
    h.run_steps(2);
    esperar_planilha(&mut h);
    let pos = centro_da_letra(&h, "B");
    clique_direito(&mut h, pos);
    let item = h.get_by_label("Trazer colunas desta planilha");
    assert!(!item.accesskit_node().is_disabled(), "com um CSV aberto, ligado");
    item.click();
    esperar(&mut h, "a janela Trazer colunas", |h| tem_texto(h, "para vendas_2025.csv"));
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("vendas_2025.csv")));
    assert!(tem_texto(&h, "relatorio_q3.xlsx › dados"), "a origem já escolhida: {:?}", textos(&h));
}

#[test]
fn status_da_planilha_diz_somente_leitura() {
    let amb = Ambiente::vazio("planilha_status");
    let planilha = xlsx(&amb, "relatorio_q3.xlsx", 5);
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::arquivo(&planilha));
    h.run_steps(2);
    esperar_planilha(&mut h);
    // a barra de status do arquivo: o nome, "Somente leitura" e, à direita, "XLSX · dados"
    assert!(tem_texto_exato(&h, "XLSX") && tem_texto_exato(&h, "dados"), "{:?}", textos(&h));
    assert!(quantos(&h, "Somente leitura") >= 2, "o chip da faixa e a barra de status: {:?}", textos(&h));
    // só o Atualizar está ligado na barra
    assert!(botao_ligado(&h, "Atualizar"));
    for verbo in ["Salvar", "Novo", "Excluir", "Desfazer", "Trazer colunas"] {
        assert!(!botao_ligado(&h, verbo), "{verbo} desligado na planilha");
    }
}

#[test]
fn arquivo_mudou_e_a_volta_do_foco_avisa() {
    let amb = Ambiente::vazio("planilha_mudou");
    let planilha = xlsx(&amb, "relatorio_q3.xlsx", 5);
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::arquivo(&planilha));
    h.run_steps(2);
    esperar_planilha(&mut h);
    h.input_mut().focused = false;
    h.run_steps(2);
    let _ = xlsx(&amb, "relatorio_q3.xlsx", 50);
    h.input_mut().focused = true;
    h.run_steps(2);
    esperar_texto(&mut h, "O arquivo mudou no disco.");
    clicar_botao(&mut h, "Ler de novo");
    assentar(&mut h);
    assert!(!tem_texto(&h, "O arquivo mudou no disco."));
}
