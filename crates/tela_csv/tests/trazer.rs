//! Passo 13 da etapa 5: a janela "Trazer colunas" (prancheta 16 d e e).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use apoio::*;
use comum::Resultado;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Documento, FonteDeColunas, TipoDeFonte, Verbo};
use interface::pecas::toast::{rotulo_da_acao_do_toast, toast_atual};
use interface::tema::Tema;
use tabular::colunas::ChavesRepetidas;
use tela_csv::Ganchos;

const ESTOQUE: &str = "codigo;produto;regiao;deposito\n\
004718;Caderno universitário;Sudeste;Guarulhos\n\
004725;Caneta gel;;Recife\n\
004731;Mochila escolar;Sudeste;Contagem\n\
019044;Agenda 2025;Norte;Contagem\n\
005102;Estojo duplo;;Guarulhos\n\
005117;Marca-texto;Nordeste;Recife\n\
005123;Agenda 2026;Nordeste;Recife\n\
005130;Caderno brochura;Sudeste;Guarulhos\n\
005148;Lápis HB;Sudeste;Contagem\n\
005155;Borracha branca;Nordeste;Recife\n";

const CATALOGO: &str = "cod_produto;produto;categoria;preco;regiao;fornecedor\n\
004718;Caderno universitário;Cadernos;24,90;Sudeste;Tilibra\n\
004725;Caneta gel;Escrita;4,50;Sul;Bic\n\
004731;Mochila escolar;Mochilas;129,90;Sudeste;Sestini\n\
005102;Estojo duplo;Estojos;27,40;Centro-Oeste;Faber\n\
005117;Marca-texto;Escrita;6,90;Nordeste;Stabilo\n\
005117;Marca-texto amarelo;Destaque;7,10;Nordeste;Stabilo\n\
005123;Agenda 2026;Agendas;39,90;Nordeste;Tilibra\n\
005130;Caderno brochura;Cadernos;12,90;Sudeste;Tilibra\n\
005148;Lápis HB;Escrita;1,20;Sudeste;Faber\n\
005155;Borracha branca;Escrita;1,80;Nordeste;Faber\n";

/// O estoque aberto, com o catálogo como uma das abas (a fonte que a casca põe em `Contexto::fontes`).
fn preparar(nome: &str) -> (Ambiente, Teste) {
    let amb = Ambiente::vazio(nome);
    let catalogo = arquivo(&amb, "catalogo_2025.csv", CATALOGO);
    let mut h = abrir(&amb, arquivo(&amb, "estoque_2026.csv", ESTOQUE));
    let fonte = tela_csv::abrir_fonte(&catalogo).expect("o catálogo");
    h.state_mut().fontes.push(fonte);
    (amb, h)
}

/// Abre a janela pelo verbo da barra e casa codigo = cod_produto.
fn abrir_janela(h: &mut H) {
    h.get_by_label("Trazer colunas").click();
    esperar_ociosa(h);
    assert!(doc(h).trazer_aberta());
    doc_mut(h).trazer_definir_chaves(0, 0);
    esperar_ociosa(h);
}

fn coluna(h: &H, c: usize) -> Vec<String> {
    let p = doc(h).pagina().unwrap();
    p.linhas.iter().map(|l| l.get(c).cloned().unwrap_or_default()).collect()
}

/// O botão da barra (o das Propriedades tem o mesmo rótulo e fica à direita).
fn botao_da_barra<'a>(h: &'a H, rotulo: &'a str) -> egui_kittest::Node<'a> {
    let nos: Vec<_> = h.query_all_by_label(rotulo).filter(|n| n.rect().top() < 46.0).collect();
    nos.into_iter().next().expect("o botão da barra")
}

#[test]
fn janela_abre_pelo_botao_pelo_menu_pelo_atalho_e_pela_secao() {
    let (_amb, mut h) = preparar("trazer_abre");
    // o botão da barra, logo depois da Nova coluna
    assert!(doc(&h).ofertas(&h.state().estado).ligado(Verbo::TrazerColunas));
    botao_da_barra(&h, "Trazer colunas").click();
    esperar_ociosa(&mut h);
    assert!(h.query_by_label("para estoque_2026.csv").is_some(), "o título com o destino");
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    assert!(!doc(&h).trazer_aberta());
    // o menu do cabeçalho
    let pos = doc(&h).centro_cabecalho(0).unwrap();
    clicar_direito_em(&mut h, pos);
    item_do_menu(&h, "Trazer colunas de outro arquivo").click();
    esperar_ociosa(&mut h);
    assert!(doc(&h).trazer_aberta());
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    assert!(!doc(&h).trazer_aberta(), "Esc fecha");
    // a seção Coluna das Propriedades
    clicar_celula(&mut h, 0, 1);
    let nos: Vec<_> = h.query_all_by_label("Trazer colunas").filter(|n| n.rect().left() > TAMANHO.x - 290.0).collect();
    nos.into_iter().next().expect("o botão da seção Coluna").click();
    esperar_ociosa(&mut h);
    assert!(doc(&h).trazer_aberta());
    tecla(&mut h, Key::Escape, Modifiers::NONE);
    // o Ctrl+Shift+V: o Paste e a tecla V com o Shift, no mesmo quadro
    clicar_celula(&mut h, 0, 1);
    let m = Modifiers { shift: true, ctrl: true, command: true, ..Modifiers::NONE };
    h.input_mut().events.push(Event::Key {
        key: Key::V,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: m,
    });
    h.input_mut().events.push(Event::Paste("cod_produto\tpreco\n004718\t24,90\n".into()));
    h.step();
    esperar_ociosa(&mut h);
    assert!(doc(&h).trazer_aberta());
    assert_eq!(doc(&h).trazer_origem().map(|o| o.0).as_deref(), Some("Área de transferência"));
}

#[test]
fn fonte_csv_inclui_as_alteracoes_pendentes() {
    let amb = Ambiente::vazio("trazer_fonte_csv");
    let mut h = abrir(&amb, arquivo(&amb, "estoque_2026.csv", ESTOQUE));
    clicar_celula(&mut h, 1, 2);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    digitar(&mut h, "Sul");
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    let fonte = doc(&h).fonte_de_colunas().expect("o CSV oferece a fonte");
    assert_eq!(fonte.tipo(), TipoDeFonte::Csv);
    assert_eq!(fonte.rotulo(), "estoque_2026.csv");
    assert_eq!(fonte.linhas(), Some(10));
    let regiao = fonte.colunas(&[2], &mut |_, _| Ok(())).expect("ler").remove(0);
    assert_eq!(regiao[1], "Sul", "com a alteração ainda não salva");
    // a mesma Arc a cada quadro
    assert!(Arc::ptr_eq(&fonte, &doc(&h).fonte_de_colunas().unwrap()));
}

#[test]
fn fonte_area_de_transferencia() {
    let (_amb, mut h) = preparar("trazer_area");
    let pos = doc(&h).centro_cabecalho(0).unwrap();
    clicar_direito_em(&mut h, pos);
    item_do_menu(&h, "Colar com correspondência").click();
    h.run_steps(2);
    colar(&mut h, "cod_produto\tpreco\n004718\t24,90\n019044\t3,00\n");
    esperar_ociosa(&mut h);
    let (rotulo, nomes) = doc(&h).trazer_origem().expect("a origem");
    assert_eq!(rotulo, "Área de transferência");
    assert_eq!(nomes, ["cod_produto", "preco"]);
    doc_mut(&mut h).trazer_definir_chaves(0, 0);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((2, 10)));
}

#[test]
fn outro_arquivo_usa_o_dialogo() {
    let amb = Ambiente::vazio("trazer_outro");
    let catalogo = arquivo(&amb, "catalogo_2025.csv", CATALOGO);
    let ganchos = Ganchos { outro_arquivo: Some(catalogo), ..Ganchos::default() };
    let mut h = palco(&amb, arquivo(&amb, "estoque_2026.csv", ESTOQUE), Tema::Claro, TAMANHO, ganchos);
    esperar_ociosa(&mut h);
    botao_da_barra(&h, "Trazer colunas").click();
    esperar_ociosa(&mut h);
    // sem outra aba, a origem começa vazia, e o seletor oferece "Outro arquivo"
    h.get(by().role(Role::ComboBox).label("De onde")).click();
    h.run_steps(2);
    h.get_by_label("Outro arquivo").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_origem().map(|o| o.0).as_deref(), Some("catalogo_2025.csv"));
    assert!(h.query_by_label("10 linhas · 6 colunas").is_some());
}

#[test]
fn sugere_a_coluna_de_mesmo_nome() {
    let (_amb, mut h) = preparar("trazer_sugere");
    botao_da_barra(&h, "Trazer colunas").click();
    esperar_ociosa(&mut h);
    // "produto" existe dos dois lados: a sugestão pelo nome vem antes da dos valores em comum
    assert_eq!(doc(&h).trazer_chaves(), Some((Some(1), Some(1))));
    // sem nome em comum, o par com mais valores em comum numa amostra
    drop(h);
    let amb = Ambiente::vazio("trazer_sugere_valores");
    let origem = arquivo(&amb, "o.csv", "cod;nome\n004718;x\n004725;y\n");
    let mut h = abrir(&amb, arquivo(&amb, "estoque_2026.csv", ESTOQUE));
    h.state_mut().fontes.push(tela_csv::abrir_fonte(&origem).unwrap());
    botao_da_barra(&h, "Trazer colunas").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_chaves(), Some((Some(0), Some(0))));
}

#[test]
fn medidor_mostra_quantas_tem_par() {
    let (_amb, mut h) = preparar("trazer_medidor");
    abrir_janela(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((9, 10)));
    assert!(h.query_by_label("9 de 10 linhas têm par (90%)").is_some());
    let medidor = h.get(by().role(Role::ProgressIndicator).label("Linhas com par"));
    assert_eq!(medidor.accesskit_node().numeric_value(), Some(90.0));
}

#[test]
fn trocar_a_opcao_recalcula_o_medidor() {
    let amb = Ambiente::vazio("trazer_opcoes");
    let origem = arquivo(&amb, "o.csv", "cod;v\nABC;1\n  def ;2\n");
    let mut h = abrir(&amb, arquivo(&amb, "d.csv", "cod;x\nabc;1\ndef;2\n"));
    h.state_mut().fontes.push(tela_csv::abrir_fonte(&origem).unwrap());
    botao_da_barra(&h, "Trazer colunas").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((2, 2)), "maiúsculas e espaços ignorados de saída");
    h.get_by_label("Ignorar maiúsculas").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((1, 2)));
    h.get_by_label("Ignorar espaços nas pontas").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((0, 2)));
    assert!(
        h.query_all(by().role(Role::Button).label("Trazer colunas"))
            .last()
            .expect("o botão da janela")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn numeros_pelo_valor_desligado_protege_os_zeros() {
    let amb = Ambiente::vazio("trazer_zeros");
    let origem = arquivo(&amb, "o.csv", "cod;v\n007;sete\n");
    let mut h = abrir(&amb, arquivo(&amb, "d.csv", "cod;x\n7;a\n"));
    h.state_mut().fontes.push(tela_csv::abrir_fonte(&origem).unwrap());
    botao_da_barra(&h, "Trazer colunas").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((0, 1)), "007 e 7 não casam de saída");
    h.get_by_label("Números pelo valor").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((1, 1)));
}

#[test]
fn chaves_repetidas_mostra_o_aviso_e_os_tres_modos() {
    let (_amb, mut h) = preparar("trazer_repetidas");
    abrir_janela(&mut h);
    assert!(h.query_by_label("1 valor de cod_produto aparece mais de uma vez em catalogo_2025.csv.").is_some());
    h.get_by_label("Ver quais").click();
    h.run_steps(2);
    assert!(h.query_by_label("005117 · 2 vezes").is_some());
    doc_mut(&mut h).trazer_marcar(2, true);
    doc_mut(&mut h).trazer_repetidas(ChavesRepetidas::TodasJuntas);
    esperar_ociosa(&mut h);
    h.get_by_label("Trazer 1 coluna").click();
    esperar_ociosa(&mut h);
    assert_eq!(coluna(&h, 4)[5], "Escrita; Destaque", "todas, juntas com \"; \"");
}

#[test]
fn lista_marca_e_marca_todas() {
    let (_amb, mut h) = preparar("trazer_lista");
    abrir_janela(&mut h);
    assert!(h.query_by_label("0 de 5 colunas").is_some());
    h.get(by().role(Role::CheckBox).label("Trazer preco")).click();
    h.run_steps(2);
    assert!(h.query_by_label("1 de 5 colunas").is_some());
    h.get_by_label("Marcar todas").click();
    h.run_steps(2);
    assert!(h.query_by_label("5 de 5 colunas").is_some());
    h.get_by_label("Marcar todas").click();
    h.run_steps(2);
    assert!(h.query_by_label("0 de 5 colunas").is_some(), "com todas marcadas, desmarca");
}

#[test]
fn destino_novo_com_nome_editavel_e_sugestao_com_a_origem() {
    let (_amb, mut h) = preparar("trazer_destino");
    abrir_janela(&mut h);
    // "produto" e "regiao" já existem neste arquivo: o nome sugerido leva o rótulo da origem
    assert_eq!(doc(&h).trazer_nome_novo(1).as_deref(), Some("produto (catalogo)"));
    assert_eq!(doc(&h).trazer_nome_novo(3).as_deref(), Some("preco"));
    h.get(by().role(Role::CheckBox).label("Trazer preco")).click();
    h.run_steps(2);
    let campo = h.get(by().role(Role::TextInput).label("Nome da coluna nova para preco"));
    campo.focus();
    h.run_steps(1);
    h.get(by().role(Role::TextInput).label("Nome da coluna nova para preco")).type_text("_novo");
    h.run_steps(2);
    assert_eq!(doc(&h).trazer_nome_novo(3).as_deref(), Some("preco_novo"));
    h.get_by_label("Trazer 1 coluna").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().nomes.last().map(String::as_str), Some("preco_novo"));
}

#[test]
fn atualizar_coluna_so_celulas_vazias() {
    let (_amb, mut h) = preparar("trazer_atualizar");
    abrir_janela(&mut h);
    doc_mut(&mut h).trazer_marcar(4, true);
    doc_mut(&mut h).trazer_atualizar(4, 2, true);
    esperar_ociosa(&mut h);
    h.get_by_label("Trazer 1 coluna").click();
    esperar_ociosa(&mut h);
    // as vazias (linhas 2 e 5) recebem; as outras ficam como estavam (a linha 2 era Sul no catálogo)
    assert_eq!(
        coluna(&h, 2),
        [
            "Sudeste",
            "Sul",
            "Sudeste",
            "Norte",
            "Centro-Oeste",
            "Nordeste",
            "Nordeste",
            "Sudeste",
            "Sudeste",
            "Nordeste"
        ]
    );
    assert_eq!(doc(&h).retrato().unwrap().colunas(), 4, "atualizar não cria coluna");
    let p = doc(&h).pagina().unwrap();
    assert!(p.alterada(1, 2) && !p.alterada(0, 2), "só as que mudaram ganham o td.alt");
}

#[test]
fn trazer_desligado_sem_coluna_ou_com_zero_por_cento() {
    let (_amb, mut h) = preparar("trazer_desligado");
    abrir_janela(&mut h);
    assert!(
        h.query_all(by().role(Role::Button).label("Trazer colunas"))
            .last()
            .expect("o botão da janela")
            .accesskit_node()
            .is_disabled()
    );
    doc_mut(&mut h).trazer_marcar(3, true);
    h.run_steps(2);
    assert!(!h.get(by().role(Role::Button).label("Trazer 1 coluna")).accesskit_node().is_disabled());
    // casar por colunas sem nada em comum: 0%
    doc_mut(&mut h).trazer_definir_chaves(3, 5);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((0, 10)));
    assert!(h.get(by().role(Role::Button).label("Trazer 1 coluna")).accesskit_node().is_disabled());
}

#[test]
fn trazer_e_um_passo_so_e_o_toast_conta_sem_par() {
    let (_amb, mut h) = preparar("trazer_passo");
    abrir_janela(&mut h);
    doc_mut(&mut h).trazer_marcar(2, true);
    doc_mut(&mut h).trazer_marcar(3, true);
    esperar_ociosa(&mut h);
    h.get_by_label("Trazer 2 colunas").click();
    esperar_ociosa(&mut h);
    assert!(!doc(&h).trazer_aberta());
    let r = doc(&h).retrato().unwrap();
    assert_eq!(r.nomes, ["codigo", "produto", "regiao", "deposito", "categoria", "preco"]);
    assert_eq!(r.alteracoes, 1, "uma alteração só");
    assert_eq!(coluna(&h, 5)[0], "24,90");
    assert_eq!(coluna(&h, 5)[3], "", "a linha sem par fica vazia");
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("2 colunas trazidas · 1 linha sem par"));
    assert_eq!(rotulo_da_acao_do_toast(&h.ctx).as_deref(), Some("Marcar as linhas sem par"));
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::Desfazer), "Desfazer Trazer 2 colunas");
    botao_da_barra(&h, "Desfazer").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().colunas(), 4);
}

#[test]
fn marcar_as_linhas_sem_par_marca_na_grade() {
    let (_amb, mut h) = preparar("trazer_marcar_sem_par");
    abrir_janela(&mut h);
    doc_mut(&mut h).trazer_marcar(3, true);
    esperar_ociosa(&mut h);
    h.get_by_label("Trazer 1 coluna").click();
    esperar_ociosa(&mut h);
    h.get(by().role(Role::Button).label("Marcar as linhas sem par")).click();
    h.run_steps(3);
    assert_eq!(doc(&h).linhas_marcadas(), [3], "a 019044 não tem par");
    assert!(h.query_by_label("· 1 marcada").is_some());
}

#[test]
fn linhas_sem_par_andam_com_as_alteracoes_seguintes() {
    let (_amb, mut h) = preparar("trazer_sem_par_andam");
    abrir_janela(&mut h);
    doc_mut(&mut h).trazer_marcar(3, true);
    esperar_ociosa(&mut h);
    h.get_by_label("Trazer 1 coluna").click();
    esperar_ociosa(&mut h);
    // a primeira linha sai antes de marcar as sem par: a 019044 (era a 3) sobe para a 2
    doc_mut(&mut h).marcar_linhas([0]);
    h.run_steps(2);
    botao_da_barra(&h, "Excluir").click();
    esperar_ociosa(&mut h);
    assert!(doc(&h).linhas_marcadas().is_empty());
    h.get(by().role(Role::Button).label("Marcar as linhas sem par")).click();
    h.run_steps(3);
    assert_eq!(doc(&h).linhas_marcadas(), [2], "a 019044 continua sem par, uma linha acima");
    assert_eq!(coluna(&h, 0)[2], "019044");
}

#[test]
fn trazer_sem_mudar_nada_nao_deixa_rotulo() {
    let (_amb, mut h) = preparar("trazer_sem_mudar");
    // as regiões vazias do estoque vêm do catálogo (só nas vazias): a primeira vez muda
    for vez in 0..2 {
        botao_da_barra(&h, "Trazer colunas").click();
        esperar_ociosa(&mut h);
        doc_mut(&mut h).trazer_definir_chaves(0, 0);
        esperar_ociosa(&mut h);
        doc_mut(&mut h).trazer_marcar(4, true);
        doc_mut(&mut h).trazer_atualizar(4, 2, true);
        esperar_ociosa(&mut h);
        h.get_by_label("Trazer 1 coluna").click();
        esperar_ociosa(&mut h);
        assert_eq!(doc(&h).retrato().unwrap().alteracoes, 1, "na vez {vez}");
    }
    // a segunda não achou vazia com par: nada entrou no desfazer, e a tela diz
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Nada mudou: as células já tinham esses valores."));
    assert_eq!(doc_mut(&mut h).rotulos_do_desfazer().len(), 1, "um rótulo só, como o núcleo");
    assert_eq!(doc(&h).ofertas(&h.state().estado).dica(Verbo::Desfazer), "Desfazer Trazer 1 coluna");
    assert_eq!(coluna(&h, 2)[1], "Sul");
}

/// Uma origem lenta: as colunas só saem quando `solto` liga (ou quando o progresso cancela).
struct FonteLenta {
    solto: Arc<AtomicBool>,
}

impl FonteDeColunas for FonteLenta {
    fn rotulo(&self) -> String {
        "lenta.csv".into()
    }
    fn tipo(&self) -> TipoDeFonte {
        TipoDeFonte::Csv
    }
    fn nomes(&self) -> Vec<String> {
        vec!["codigo".into(), "v".into()]
    }
    fn linhas(&self) -> Option<u64> {
        Some(1)
    }
    fn colunas(
        &self,
        quais: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let inicio = Instant::now();
        while !self.solto.load(Ordering::SeqCst) && inicio.elapsed() < Duration::from_secs(20) {
            ao_progresso(0, 1)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(quais.iter().map(|_| vec!["004718".to_string()]).collect())
    }
}

#[test]
fn cancelar_o_calculo_pela_janela_e_a_janela_nao_trava() {
    let amb = Ambiente::vazio("trazer_cancelar");
    let mut h = abrir(&amb, arquivo(&amb, "estoque_2026.csv", ESTOQUE));
    let solto = Arc::new(AtomicBool::new(false));
    h.state_mut().fontes.push(Arc::new(FonteLenta { solto: Arc::clone(&solto) }));
    botao_da_barra(&h, "Trazer colunas").click();
    let inicio = Instant::now();
    h.run_steps(6);
    assert!(inicio.elapsed() < Duration::from_secs(5), "a janela desenha enquanto a origem lê");
    assert!(h.query_by_label_contains("Casando as 10 linhas").is_some());
    h.get_by_label("Cancelar o cálculo").click();
    h.run_steps(3);
    assert!(h.query_by_label("Cálculo cancelado.").is_some());
    solto.store(true, Ordering::SeqCst);
    h.get_by_label("Calcular de novo").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).trazer_medidor(), Some((1, 10)));
}

#[test]
fn capturas_do_trazer_contra_a_prancheta_16() {
    let amb = Ambiente::vazio("trazer_capturas");
    let catalogo = arquivo(&amb, "catalogo_2025.csv", CATALOGO);
    let mut h = abrir_com(&amb, arquivo(&amb, "estoque_2026.csv", ESTOQUE), Tema::Escuro, TAMANHO);
    h.state_mut().fontes.push(tela_csv::abrir_fonte(&catalogo).unwrap());
    abrir_janela(&mut h);
    doc_mut(&mut h).trazer_marcar(2, true);
    doc_mut(&mut h).trazer_marcar(3, true);
    doc_mut(&mut h).trazer_marcar(4, true);
    doc_mut(&mut h).trazer_atualizar(4, 2, true);
    esperar_ociosa(&mut h);
    capturar(&mut h, "trazer-colunas-escuro.png");
    h.get_by_label("Trazer 3 colunas").click();
    esperar_ociosa(&mut h);
    capturar(&mut h, "trazer-depois-escuro.png");
}
