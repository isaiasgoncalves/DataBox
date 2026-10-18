//! A planilha como origem (etapa 6, prancheta 17, cenas e e f): a `FontePlanilha` do "Trazer colunas" pela
//! regra da decisão 16 (número cru com a vírgula do destino, datas pelo exibir, booleano e erro como texto), o cabeçalho
//! pela heurística, e o menu do cabeçalho (Copiar coluna · Trazer colunas desta planilha).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;

use apoio::*;
use comum::Resultado;
use egui::{Event, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::{NodeT, Queryable};
use estado::{Documento, FonteDeColunas, TipoDeFonte};
use tela_planilha::{FontePlanilha, MSG_ABRA_UM_CSV, PedidoDeArquivo, TRAZER_DESTA_PLANILHA};

const LINHAS: u32 = 60;

/// A origem da planilha da tela (a que a casca põe no `Contexto::fontes`).
fn fonte(t: &Teste) -> Arc<dyn FonteDeColunas> {
    tela(t).fonte_de_colunas().expect("a planilha oferece a origem")
}

fn abrir_relatorio(nome: &str) -> Teste {
    let mut t = abrir(&format!("pl_fonte_{nome}"), "relatorio_q3.xlsx", &relatorio_q3(LINHAS));
    esperar_o_indice(&mut t);
    t.run_steps(3);
    t
}

/// As colunas `quais` da origem, sem progresso.
fn ler(f: &dyn FonteDeColunas, quais: &[usize]) -> Vec<Vec<String>> {
    f.colunas(quais, &mut |_, _| Ok(())).expect("ler as colunas")
}

/// A Vendas sem a linha do título: o cabeçalho na 1 (Região, Produto…) e os dados embaixo.
fn vendas_sem_titulo() -> FontePlanilha {
    let pasta = planilha::Pasta::abrir(gravar_temporario("vendas.xlsx", &relatorio_q3(LINHAS))).expect("a pasta");
    FontePlanilha::nova(pasta, 1, (0..6).collect(), vec![String::new(); 6], Some(LINHAS), false)
}

/// Grava os bytes numa pasta temporária do teste (fora de um palco).
fn gravar_temporario(nome: &str, bytes: &[u8]) -> std::path::PathBuf {
    let pasta = std::env::temp_dir().join(format!("tela_planilha_fonte_{}_{nome}", std::process::id()));
    std::fs::create_dir_all(&pasta).expect("a pasta");
    let c = pasta.join(nome);
    std::fs::write(&c, bytes).expect("gravar");
    c
}

#[test]
fn fonte_de_planilha_com_cabecalho_ligado_pela_heuristica() {
    // a primeira linha de textos sobre números e datas: é cabeçalho, e os nomes vêm dela
    let mut linhas = linha_xml(1, &[t("data"), t("valor"), t("pago")], false);
    for n in 2..=20 {
        linhas.push_str(&linha_xml(
            n,
            &[C::NumEstilo(46_204.0 + f64::from(n), 1), C::Num(f64::from(n) * 1.5), C::Bool(n % 2 == 0)],
            false,
        ));
    }
    let bytes = Montador::novo().aba("Pagamentos", &linhas).estilos(&[], &[0, 14]).bytes();
    let mut t = abrir("pl_fonte_cabecalho", "pagamentos.xlsx", &bytes);
    esperar_o_indice(&mut t);
    t.run_steps(3);
    let f = fonte(&t);
    assert_eq!((f.rotulo().as_str(), f.tipo()), ("pagamentos.xlsx › Pagamentos", TipoDeFonte::Planilha));
    assert_eq!(f.primeira_linha_e_cabecalho(), Some(true));
    assert_eq!(f.nomes(), ["data", "valor", "pago"]);
    assert_eq!(f.linhas(), Some(19), "sem a linha do cabeçalho");
    // desligada a caixa, as colunas se chamam pelas letras e a primeira linha entra nos dados
    let sem = f.com_primeira_linha_cabecalho(false).expect("a planilha tem a caixa");
    assert_eq!(sem.nomes(), ["A", "B", "C"]);
    assert_eq!(sem.linhas(), Some(20));
    assert_eq!(ler(sem.as_ref(), &[1])[0][0], "valor");
    // só números: não é cabeçalho
    let numeros: String = (1..=10).map(|n| linha_xml(n, &[C::Num(f64::from(n)), C::Num(2.0)], false)).collect();
    let mut t = abrir("pl_fonte_sem_cabecalho", "numeros.xlsx", &Montador::novo().aba("N", &numeros).bytes());
    esperar_o_indice(&mut t);
    t.run_steps(3);
    assert_eq!(fonte(&t).primeira_linha_e_cabecalho(), Some(false));
    assert_eq!(fonte(&t).nomes(), ["A", "B"]);
}

#[test]
fn numero_vai_cru_com_a_virgula_do_destino() {
    let f = vendas_sem_titulo();
    let receita = &ler(&f, &[3])[0];
    // a linha 3 (R$ 10.119,37 na grade) vai crua, com a vírgula; o percentual também (0,98 e não 98%)
    assert_eq!(receita[2], "10119,37");
    assert_eq!(ler(&f, &[4])[0][5], "0,98");
    let ponto = f.clone().com_decimal('.');
    assert_eq!(ler(&ponto, &[3])[0][2], "10119.37");
    // pelo trait, como a janela do CSV com o ponto decimal lê: o mesmo número, com o ponto
    let pelo_trait = FonteDeColunas::para_o_decimal(&f, '.').expect("a planilha aceita o decimal do destino");
    assert_eq!(ler(pelo_trait.as_ref(), &[3])[0][2], "10119.37");
    assert_eq!(pelo_trait.rotulo(), f.rotulo(), "a mesma origem");
}

#[test]
fn data_vai_pelo_exibir() {
    let f = vendas_sem_titulo();
    let datas = &ler(&f, &[2])[0];
    assert_eq!((datas[2].as_str(), datas[3].as_str()), ("01/07/2026", "02/07/2026"));
}

#[test]
fn booleano_e_erro_vao_como_texto() {
    let f = vendas_sem_titulo();
    let v = ler(&f, &[4, 5]);
    assert_eq!(v[0][11], "#DIV/0!", "E12");
    assert_eq!((v[1][2].as_str(), v[1][3].as_str()), ("VERDADEIRO", "FALSO"));
    // a coluna que não existe é erro, não pânico
    assert!(f.colunas(&[9], &mut |_, _| Ok(())).is_err());
}

#[test]
fn trazer_colunas_de_planilha_para_csv_de_ponta_a_ponta() {
    // o que a janela do CSV faz com a origem: os nomes, as linhas e duas colunas lidas em fluxo, com o progresso e o
    // cancelamento pelo erro do progresso
    let t = abrir_relatorio("ponta_a_ponta");
    let f = fonte(&t);
    let nomes = f.nomes();
    assert_eq!(nomes.len(), 6);
    let mut passos = Vec::new();
    let v = f
        .colunas(&[0, 3], &mut |feito, total| {
            passos.push((feito, total));
            Ok(())
        })
        .expect("as colunas");
    let n = f.linhas().expect("o total") as usize;
    assert!(v.iter().all(|c| c.len() == n), "uma célula por linha");
    assert_eq!(passos.last().map(|p| p.0), Some(n as u64));
    let cancelar = &mut |_: u64, _: u64| -> Resultado<()> { Err(comum::ErroBanco::novo("Cancelado.")) };
    assert!(f.colunas(&[0], cancelar).is_err(), "o erro do progresso cancela");
}

/// Um clique com o botão secundário em `pos`.
fn clique_direito(h: &mut H, pos: Pos2) {
    h.hover_at(pos);
    h.event(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
    h.event(Event::PointerButton { pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

/// O centro da letra `letra` pintada no cabeçalho.
fn centro_da_letra(h: &H, letra: &str) -> Pos2 {
    h.output()
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == letra => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a letra {letra}"))
}

#[test]
fn menu_do_cabecalho_da_planilha_tem_os_dois_itens() {
    let mut t = abrir_relatorio("menu");
    let pos = centro_da_letra(&t, "D");
    clique_direito(&mut t, pos);
    assert!(t.query_by_label("Copiar coluna").is_some(), "o Copiar coluna");
    assert!(t.query_by_label(TRAZER_DESTA_PLANILHA).is_some(), "o Trazer colunas desta planilha");
    // o clique direito escolheu a coluna
    assert_eq!(tela(&t).grade().map(|g| g.colunas_selecionadas.iter().copied().collect::<Vec<_>>()), Some(vec![3]));
}

/// Uma origem CSV de mentira (a de um CSV aberto em outra aba).
struct CsvAberto;

impl FonteDeColunas for CsvAberto {
    fn rotulo(&self) -> String {
        "vendas_2025.csv".into()
    }
    fn tipo(&self) -> TipoDeFonte {
        TipoDeFonte::Csv
    }
    fn nomes(&self) -> Vec<String> {
        vec!["regiao".into()]
    }
    fn linhas(&self) -> Option<u64> {
        Some(0)
    }
    fn colunas(&self, _: &[usize], _: &mut dyn FnMut(u64, u64) -> Resultado<()>) -> Resultado<Vec<Vec<String>>> {
        Ok(vec![Vec::new()])
    }
}

#[test]
fn trazer_colunas_desta_planilha_desligado_sem_csv_aberto() {
    let mut t = abrir_relatorio("trazer");
    let pos = centro_da_letra(&t, "B");
    clique_direito(&mut t, pos);
    let item = t.get_by_label(TRAZER_DESTA_PLANILHA);
    assert!(item.accesskit_node().is_disabled(), "sem CSV aberto, desligado");
    item.hover();
    let mut dica = false;
    for _ in 0..60 {
        t.step();
        dica |= t.query_by_label(MSG_ABRA_UM_CSV).is_some();
        if dica {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(dica, "a dica diz o que fazer");
    t.key_press(egui::Key::Escape);
    t.run_steps(2);
    // com um CSV aberto em outra aba, o item liga e pede à casca a janela com esta origem
    t.state_mut().fontes = vec![Arc::new(CsvAberto)];
    let pos = centro_da_letra(&t, "B");
    clique_direito(&mut t, pos);
    let item = t.get_by_label(TRAZER_DESTA_PLANILHA);
    assert!(!item.accesskit_node().is_disabled());
    item.click();
    t.run_steps(2);
    assert_eq!(
        tela_mut(&mut t).tomar_pedido(),
        Some(PedidoDeArquivo::TrazerColunas("relatorio_q3.xlsx › Vendas".to_owned()))
    );
}

#[test]
fn copiar_coluna_da_planilha_cola_no_csv() {
    let mut t = abrir_relatorio("copiar_coluna");
    let pos = centro_da_letra(&t, "C");
    clique_direito(&mut t, pos);
    t.get_by_label("Copiar coluna").click();
    let mut texto = None;
    esperar_ate(&mut t, |h| {
        texto = texto.take().or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
        });
        texto.is_some()
    });
    // o que o colar do CSV lê: uma coluna, uma linha por linha da planilha, as datas como se veem
    let tabela = tabular::colunas::ler_tsv(&texto.unwrap_or_default());
    assert_eq!(tabela.len(), LINHAS as usize);
    let coluna = tabular::colunas::coluna_do_texto(&tabela, 0);
    assert_eq!((coluna[1].as_str(), coluna[2].as_str()), ("Fechado em", "01/07/2026"));
}

#[test]
fn captura_do_menu_do_cabecalho() {
    let mut t = abrir_com(
        "pl_fonte_cap_menu",
        "relatorio_q3.xlsx",
        &relatorio_q3(1204),
        interface::tema::Tema::Escuro,
        TAMANHO,
        |t| t,
    );
    esperar_o_indice(&mut t);
    t.run_steps(3);
    let pos = centro_da_letra(&t, "D");
    clique_direito(&mut t, pos);
    capturar(&mut t, "planilha-menu-escuro.png");
}
