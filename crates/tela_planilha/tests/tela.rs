//! A tela da planilha (etapa 6, prancheta 17, cenas a, c e d): a grade com as letras, o número do Excel, as
//! larguras do arquivo, as mesclas, o painel congelado, a linha oculta, a seleção e a cópia; a faixa das planilhas, as
//! Propriedades, o rodapé, o status, os verbos e o arquivo que muda no disco. As capturas `planilha-*`.

mod apoio;

use apoio::*;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::{NodeT, Queryable};
use estado::{Documento, Situacao, Verbo};
use interface::grade::{Congelado, Mescla};
use interface::tema::{Tema, paleta_ctx};
use tela_planilha::{IGNORAR, LER_DE_NOVO, MSG_COPIADO, MSG_FORMULAS, MSG_MUDOU_NO_DISCO, Modo};

const LINHAS: u32 = 60;

fn abrir_relatorio(nome: &str) -> Teste {
    let mut t = abrir(&format!("pl_rel_{nome}"), "relatorio_q3.xlsx", &relatorio_q3(LINHAS));
    esperar_o_indice(&mut t);
    t.run_steps(3);
    t
}

/// Os textos pintados no último quadro: o texto, o canto, a largura, a cor e a fonte.
fn textos(h: &H) -> Vec<(String, Pos2, f32, egui::Color32, egui::FontId)> {
    h.output()
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                t.pos,
                t.galley.size().x,
                t.fallback_color,
                t.galley.job.sections.first().map(|s| s.format.font_id.clone()).unwrap_or_default(),
            )),
            _ => None,
        })
        .collect()
}

fn pintado<'a>(
    v: &'a [(String, Pos2, f32, egui::Color32, egui::FontId)],
    texto: &str,
) -> Option<&'a (String, Pos2, f32, egui::Color32, egui::FontId)> {
    v.iter().find(|x| x.0 == texto)
}

/// Um clique com o botão principal em `pos`, com os modificadores.
fn clicar_em(h: &mut H, pos: Pos2, modificadores: Modifiers) {
    h.event(Event::ModifiersChanged(modificadores));
    h.hover_at(pos);
    h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: modificadores });
    h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: modificadores });
    h.run_steps(2);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(1);
}

/// O centro do texto pintado `texto` (o primeiro).
fn centro_de(h: &H, texto: &str) -> Pos2 {
    let v = textos(h);
    let (_, pos, largura, ..) = pintado(&v, texto).unwrap_or_else(|| panic!("{texto} não foi pintado"));
    *pos + egui::vec2(largura / 2.0, 7.0)
}

/// O texto que foi para a área de transferência neste quadro.
fn copiado(h: &H) -> Option<String> {
    h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    })
}

/// Roda quadros até algo ir para a área de transferência.
fn esperar_copia(t: &mut Teste) -> String {
    let mut texto = None;
    esperar_ate(t, |h| {
        texto = texto.take().or_else(|| copiado(h));
        texto.is_some()
    });
    texto.unwrap_or_default()
}

#[test]
fn letras_de_coluna_e_numero_da_linha_do_excel() {
    let t = abrir_relatorio("letras");
    let v = textos(&t);
    for letra in ["A", "B", "C", "D", "E", "F"] {
        assert!(pintado(&v, letra).is_some(), "a letra {letra} no cabeçalho");
    }
    // o número da linha é o do Excel (a 1 é o título congelado)
    for n in ["1", "2", "3", "12"] {
        assert!(pintado(&v, n).is_some(), "o número {n}");
    }
    let g = tela(&t).grade().expect("a grade");
    assert_eq!(g.do_arquivo, [0, 1, 2, 3, 4, 5]);
    assert_eq!(g.nucleo.n_linhas(), LINHAS as usize);
}

#[test]
fn largura_do_arquivo_vira_pixel_pela_formula() {
    let t = abrir_relatorio("larguras");
    let g = tela(&t).grade().expect("a grade");
    // round(caracteres × 7 + 5): 18 → 131, 30 → 215, 14 → 103, 16 → 117; a coluna do número tem 56
    assert_eq!(g.nucleo.larguras(), [56.0, 131.0, 215.0, 103.0, 131.0, 103.0, 117.0]);
}

#[test]
fn coluna_oculta_some_e_a_secao_diz() {
    let x = Extras {
        colunas: vec![(1, 1, 10.0, false), (3, 3, 10.0, true)],
        dimensao: Some("A1:D3".into()),
        ..Extras::default()
    };
    let linhas: String =
        (1..=3).map(|n| linha_xml(n, &[C::Num(1.0), C::Num(2.0), C::Num(3.0), C::Num(4.0)], false)).collect();
    let mut t = abrir("pl_oculta", "ocultas.xlsx", &Montador::novo().aba_com("P", &linhas, &x).bytes());
    esperar_o_indice(&mut t);
    t.run_steps(3);
    let g = tela(&t).grade().expect("a grade");
    assert_eq!(g.do_arquivo, [0, 1, 3], "a C some");
    assert_eq!(g.colunas_ocultas(), 1);
    let v = textos(&t);
    assert!(pintado(&v, "D").is_some() && pintado(&v, "C").is_none());
    // a seção Planilha diz quantas colunas estão ocultas (e não "nenhuma")
    assert!(t.query_by_label("nenhuma").is_none());
}

#[test]
fn numeros_a_direita_texto_a_esquerda_e_erro_em_perigo() {
    let t = abrir_relatorio("alinhamento");
    let e = &paleta_ctx(&t.ctx).estudio;
    let v = textos(&t);
    let erro = pintado(&v, "#DIV/0!").expect("o erro pintado");
    assert_eq!(erro.3, e.perigo, "o erro em --perigo");
    // o texto do cabeçalho da linha 2 fica à esquerda da coluna (10px de padding); 109% (E3) encosta à direita da E
    // (103px, 11 de padding e fio) e VERDADEIRO (F3) fica no meio da F (117px)
    let (pct, atingido) = (pintado(&v, "109%").expect("109%"), pintado(&v, "Atingido").expect("Atingido"));
    let coluna_e = atingido.1.x - 10.0;
    assert!((pct.1.x + pct.2 - (coluna_e + 103.0 - 11.0)).abs() < 1.5, "109% encostado à direita da E");
    let (verdadeiro, meta) =
        (pintado(&v, "VERDADEIRO").expect("VERDADEIRO"), pintado(&v, "Meta batida").expect("Meta"));
    let meio_f = meta.1.x - 10.0 + (117.0 - 1.0) / 2.0;
    assert!((verdadeiro.1.x + verdadeiro.2 / 2.0 - meio_f).abs() < 1.5, "o booleano no centro");
}

#[test]
fn linha_oculta_aparece_esmaecida() {
    let t = abrir_relatorio("oculta");
    let e = &paleta_ctx(&t.ctx).estudio;
    let s = tela(&t).sessao().expect("a planilha");
    assert!(s.linha(9).is_some_and(|l| l.oculta));
    let v = textos(&t);
    // a linha 9 (Nordeste, k = 7) em --t3, e o número dela em itálico
    let nove = v.iter().find(|x| x.0 == "9").expect("o número 9");
    assert_eq!(nove.4, interface::fontes::fonte(12.0, interface::fontes::Peso::Italico));
    let y = nove.1.y;
    let da_linha: Vec<_> = v.iter().filter(|x| (x.1.y - y).abs() < 1.0 && x.0 != "9").collect();
    assert!(!da_linha.is_empty() && da_linha.iter().all(|x| x.3 == e.t3), "{da_linha:?}");
}

#[test]
fn mescla_aparece_depois_do_indice() {
    let mut t =
        abrir_com("pl_mescla", "r.xlsx", &relatorio_q3(LINHAS), Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    t.run_steps(2);
    // antes do índice, a mescla (que fica no fim do XML) ainda não existe
    assert!(tela(&t).grade().expect("a grade").nucleo.mesclas().is_none());
    let egui = t.ctx.clone();
    tela_mut(&mut t).iniciar_indice(&egui);
    esperar_o_indice(&mut t);
    t.run_steps(2);
    let m = tela(&t).grade().and_then(|g| g.nucleo.mesclas().cloned()).expect("as mesclas");
    assert_eq!(m.mesclas(), [Mescla::new(0, 1, 0, 6)]);
    assert!(textos(&t).iter().any(|x| x.0 == TITULO), "o título pintado no retângulo da mescla");
}

#[test]
fn congelado_do_arquivo_vira_o_do_nucleo() {
    let t = abrir_relatorio("congelado");
    let g = tela(&t).grade().expect("a grade");
    assert_eq!(g.nucleo.congelado(), Some(Congelado { linhas: 1, colunas: 1 }));
    // a coluna do número e a A presas (2), e a linha 1
    assert_eq!(g.nucleo.congelado_efetivo(), Congelado { linhas: 1, colunas: 2 });
}

#[test]
fn dica_traz_a_formula() {
    let mut t = abrir_relatorio("dica");
    let pos = centro_de(&t, "98%");
    t.hover_at(pos);
    let mut achou = false;
    for _ in 0..80 {
        t.step();
        if t.query_by_label_contains("= D6/Resumo!C3").is_some() {
            achou = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(achou, "a dica da E6 com a fórmula");
}

#[test]
fn celula_ativa_e_migalha_acompanham() {
    let mut t = abrir_relatorio("migalha");
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((5, 5)));
    t.run_steps(3);
    let na_faixa = |t: &Teste, rotulo: &str| t.query_all_by_label(rotulo).any(|n| n.rect().top() < 120.0);
    assert!(na_faixa(&t, "E6"), "a migalha diz a célula");
    // na mescla, o intervalo dela
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((0, 1)));
    t.run_steps(3);
    assert!(na_faixa(&t, "A1:F1"));
}

#[test]
fn ctrl_c_copia_a_celula_pelo_exibir() {
    let mut t = abrir_relatorio("copia_celula");
    let pos = centro_de(&t, "R$ 10.119,37");
    clicar_em(&mut t, pos, Modifiers::NONE);
    assert_eq!(tela(&t).grade().and_then(|g| g.celula_ativa()), Some((3, 3)));
    t.event(Event::Copy);
    let texto = esperar_copia(&mut t);
    assert_eq!(texto, "R$ 10.119,37", "a célula leva o texto como se vê");
    assert_eq!(interface::pecas::toast::toast_atual(&t.ctx).as_deref(), Some(MSG_COPIADO));
}

#[test]
fn ctrl_c_copia_a_coluna_em_tsv() {
    let mut t = abrir_relatorio("copia_coluna");
    let pos = centro_de(&t, "D");
    clicar_em(&mut t, pos, Modifiers::NONE);
    assert_eq!(tela(&t).grade().map(|g| g.colunas_selecionadas.iter().copied().collect::<Vec<_>>()), Some(vec![3]));
    t.event(Event::Copy);
    let texto = esperar_copia(&mut t);
    let linhas: Vec<&str> = texto.split("\r\n").collect();
    // todas as linhas do Excel (sem cabeçalho de verdade: a 2 vai como texto), os números crus com a vírgula
    assert_eq!(linhas.len(), LINHAS as usize + 1, "uma por linha e o fim");
    assert_eq!(linhas[0], "");
    assert_eq!(linhas[1], "Receita");
    assert_eq!(linhas[2], "10119,37");
    t.run_steps(2);
    let toast = interface::pecas::toast::toast_atual(&t.ctx).unwrap_or_default();
    assert_eq!(toast, "Coluna D copiada · 60 linhas");
}

#[test]
fn copiar_coluna_antes_do_indice_leva_a_coluna_inteira() {
    // a `<dimension>` diz só A1, e o índice não terminou: a cópia lê até o fim do XML, nem 1 nem 128 linhas
    let x = Extras { dimensao: Some("A1".into()), ..Extras::default() };
    let bytes = Montador::novo().aba_com("Dados", &linhas_simples(300), &x).bytes();
    let mut t = abrir_com("pl_copia_sem_indice", "dados.xlsx", &bytes, Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    t.run_steps(3);
    assert!(!tela(&t).sessao().expect("a planilha").indexada());
    let pos = centro_de(&t, "A");
    clicar_em(&mut t, pos, Modifiers::NONE);
    assert_eq!(tela(&t).grade().map(|g| g.colunas_selecionadas.iter().copied().collect::<Vec<_>>()), Some(vec![0]));
    t.event(Event::Copy);
    let texto = esperar_copia(&mut t);
    let linhas: Vec<&str> = texto.split("\r\n").collect();
    assert_eq!(linhas.len(), 300 + 1, "uma por linha e o fim");
    assert_eq!((linhas[0], linhas[299]), ("1", "300"));
    t.run_steps(2);
    let toast = interface::pecas::toast::toast_atual(&t.ctx).unwrap_or_default();
    assert_eq!(toast, "Coluna A copiada · 300 linhas");
}

#[test]
fn numero_da_linha_fica_preso_sem_painel_congelado() {
    // 40 colunas de texto (64 px cada) e nenhum `<pane>` no arquivo: a grade rola para o lado
    let linhas: String =
        (1..=20).map(|n| linha_xml(n, &(0..40).map(|c| t(&format!("v{c}"))).collect::<Vec<_>>(), false)).collect();
    let mut h = abrir("pl_numero_preso", "largo.xlsx", &Montador::novo().aba("Largo", &linhas).bytes());
    esperar_o_indice(&mut h);
    h.run_steps(3);
    let antes = pintado(&textos(&h), "7").map(|x| x.1.x).expect("o número 7");
    assert!(pintado(&textos(&h), "v0").is_some());
    // a última coluna: a grade rola até ela
    tela_mut(&mut h).grade_mut().expect("a grade").nucleo.selecionar(Some((6, 40)));
    h.run_steps(4);
    let v = textos(&h);
    assert!(pintado(&v, "v39").is_some() && pintado(&v, "v0").is_none(), "a grade rolou para a direita");
    let depois = pintado(&v, "7").map(|x| x.1.x).expect("o número 7 continua à vista");
    assert!((depois - antes).abs() < 1.0, "a coluna do número fica à esquerda: {antes} → {depois}");
    // só a coluna do número presa (sem linha nem coluna do arquivo congelada, sem emenda)
    let g = tela(&h).grade().expect("a grade");
    assert_eq!(g.nucleo.congelado_efetivo(), Congelado { linhas: 0, colunas: 1 });
}

#[test]
fn selecionar_linha_e_coluna_com_toques() {
    let mut t = abrir_relatorio("toques");
    let pos = centro_de(&t, "B");
    clicar_em(&mut t, pos, Modifiers::NONE);
    let pos = centro_de(&t, "D");
    clicar_em(&mut t, pos, Modifiers::CTRL);
    let cols = |t: &Teste| tela(t).grade().map(|g| g.colunas_selecionadas.iter().copied().collect::<Vec<_>>());
    assert_eq!(cols(&t), Some(vec![1, 3]));
    let pos = centro_de(&t, "F");
    clicar_em(&mut t, pos, Modifiers::SHIFT);
    assert_eq!(cols(&t), Some(vec![3, 4, 5]), "Shift estende da última clicada (a D)");
    assert!(t.query_by_label("· 3 colunas selecionadas").is_some());
    // o número da linha escolhe a linha (e tira as colunas)
    let pos = centro_de(&t, "5");
    clicar_em(&mut t, pos, Modifiers::NONE);
    let lins = tela(&t).grade().map(|g| g.linhas_selecionadas.iter().copied().collect::<Vec<_>>());
    assert_eq!(lins, Some(vec![5]));
    assert_eq!(cols(&t), Some(vec![]));
    assert!(t.query_by_label("· linha 5 selecionada").is_some());
}

#[test]
fn ctrl_page_down_troca_a_planilha() {
    let mut t = abrir_relatorio("ctrl_pgdn");
    let pos = centro_de(&t, "Sudeste");
    clicar_em(&mut t, pos, Modifiers::NONE);
    t.key_press_modifiers(Modifiers::COMMAND, Key::PageDown);
    t.run_steps(2);
    assert_eq!(tela(&t).nome_da_folha(), Some("Regiões"));
    t.key_press_modifiers(Modifiers::COMMAND, Key::PageUp);
    t.run_steps(2);
    t.key_press_modifiers(Modifiers::COMMAND, Key::PageUp);
    t.run_steps(2);
    assert_eq!(tela(&t).nome_da_folha(), Some("Resumo"));
}

#[test]
fn faixa_das_planilhas_lista_as_visiveis_e_so_elas() {
    let t = abrir_relatorio("faixa");
    let abas: Vec<String> =
        t.query_all(egui_kittest::kittest::by().role(Role::Tab)).filter_map(|n| n.accesskit_node().label()).collect();
    assert_eq!(abas, ["Resumo, planilha 1 de 3", "Vendas, planilha 2 de 3", "Regiões, planilha 3 de 3"]);
}

#[test]
fn caixa_das_ocultas_mostra_as_ocultas_em_t3() {
    let mut t = abrir_relatorio("caixa");
    t.get_by_label("Mostrar planilhas ocultas").click();
    t.run_steps(3);
    assert!(tela(&t).mostrando_ocultas());
    assert!(t.query_by_role_and_label(Role::Tab, "Custos, planilha 4 de 4, oculta").is_some());
    let e = &paleta_ctx(&t.ctx).estudio;
    let v = textos(&t);
    assert_eq!(pintado(&v, "Custos").map(|x| x.3), Some(e.t3));
    // a da tela continua a mesma
    assert_eq!(tela(&t).nome_da_folha(), Some("Vendas"));
}

#[test]
fn graficos_nao_aparecem_e_a_secao_conta() {
    let t = abrir_relatorio("graficos");
    assert!(t.query_by_label("1 gráfico (não aparece)").is_some());
    assert!(t.query_by_role_and_label(Role::Tab, "Gráfico1, planilha 4 de 4").is_none());
    assert!(t.query_by_label("4 · 1 oculta").is_some());
}

#[test]
fn propriedades_arquivo_planilha_pasta_e_celula() {
    let mut t = abrir_relatorio("propriedades");
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((5, 5)));
    t.run_steps(3);
    for secao in ["Seção Arquivo", "Seção Planilha", "Seção Pasta de trabalho", "Seção Célula"] {
        assert!(t.query_by_label(secao).is_some(), "{secao}");
    }
    for valor in ["A1:F60", "1 linha e 1 coluna", "A2:F60", "1900", "Percentual", "número", "0,98", "= D6/Resumo!C3"] {
        assert!(t.query_by_label(valor).is_some(), "o valor {valor}");
    }
    assert!(t.query_by_label_contains(MSG_FORMULAS).is_some());
    // Copiar a fórmula
    t.get_by_label("Copiar a fórmula").click();
    assert_eq!(esperar_copia(&mut t), "=D6/Resumo!C3");
}

#[test]
fn rodape_conta_e_mostra_a_barra_depois_de_400_ms() {
    let t = abrir_relatorio("rodape");
    assert!(t.query_by_label("60 linhas · 6 colunas · A1:F60").is_some());
    assert!(t.query_by_label("· 1 mesclada").is_some());
    drop(t);
    // uma planilha grande: a barra não aparece antes de 400 ms
    let bytes = Montador::novo().aba("Lançamentos", &linhas_simples(200_000)).bytes();
    let mut t = abrir_com("pl_barra", "grande.xlsx", &bytes, Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    let egui = t.ctx.clone();
    let inicio = std::time::Instant::now();
    tela_mut(&mut t).iniciar_indice(&egui);
    t.run_steps(2);
    if inicio.elapsed() < tela_planilha::ESPERA_DA_BARRA {
        assert!(t.query_by_label_contains("Lendo a planilha…").is_none(), "nada antes de 400 ms");
    }
    std::thread::sleep(tela_planilha::ESPERA_DA_BARRA);
    t.run_steps(2);
    if tela(&t).sessao().is_some_and(|s| s.indexando()) {
        assert!(t.query_by_label_contains("Lendo a planilha…").is_some(), "a barra depois de 400 ms");
        assert!(t.query_by_label("Cancelar").is_some());
        t.get_by_label("Cancelar").click();
        t.run_steps(2);
        assert!(tela(&t).sessao().is_some_and(|s| s.indice_cancelado && !s.indexando()));
        assert!(t.query_by_label_contains("Lendo a planilha…").is_none());
    }
}

#[test]
fn status_de_arquivo_somente_leitura() {
    let t = abrir_relatorio("status");
    let s = tela(&t).status_do_arquivo(&t.state().estado).expect("o status");
    assert_eq!(s.situacao, Situacao::SoLeitura);
    assert_eq!(s.nome, "relatorio_q3.xlsx");
    assert_eq!(s.detalhes, ["XLSX", "Vendas"]);
}

#[test]
fn verbos_so_atualizar_e_o_resto_desligado() {
    let t = abrir_relatorio("verbos");
    let o = tela(&t).ofertas(&t.state().estado);
    assert!(o.ligado(Verbo::Atualizar));
    assert_eq!(o.dica(Verbo::Atualizar), "Atualizar relatorio_q3.xlsx (F5)");
    for v in [
        Verbo::Salvar,
        Verbo::Novo,
        Verbo::NovaColuna,
        Verbo::TrazerColunas,
        Verbo::Duplicar,
        Verbo::Excluir,
        Verbo::Desfazer,
        Verbo::Refazer,
        Verbo::Buscar,
        Verbo::Filtrar,
        Verbo::Exportar,
    ] {
        assert!(!o.ligado(v), "{v:?} desligado");
    }
}

#[test]
fn alterado_e_sempre_falso_e_fechar_nao_pergunta() {
    let mut t = abrir_relatorio("alterado");
    assert!(!tela(&t).alterado() && tela(&t).alteracoes().is_none() && !tela(&t).salvando());
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((5, 5)));
    t.run_steps(2);
    assert!(!tela(&t).alterado());
    assert!(tela_mut(&mut t).tomar_erro_de_gravacao().is_none());
}

#[test]
fn atualizar_volta_a_mesma_planilha_e_a_mesma_linha() {
    let mut t = abrir_relatorio("atualizar");
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((40, 3)));
    t.run_steps(2);
    t.get_by_label("Atualizar").click();
    t.run_steps(2);
    esperar_a_abertura(&mut t);
    t.run_steps(2);
    assert_eq!(tela(&t).modo(), Modo::Pronta);
    assert_eq!(tela(&t).nome_da_folha(), Some("Vendas"));
    assert_eq!(tela(&t).grade().and_then(|g| g.nucleo.selecao()), Some((40, 3)));
}

#[test]
fn arquivo_que_mudou_mostra_o_aviso() {
    let mut t = abrir_relatorio("mudou");
    t.input_mut().focused = false;
    t.run_steps(2);
    std::fs::write(&t.caminho, relatorio_q3(LINHAS + 5)).expect("regravar");
    t.input_mut().focused = true;
    t.run_steps(1);
    estado::apoio_testes::esperar_texto(&mut *t, MSG_MUDOU_NO_DISCO);
    assert!(tela(&t).mudou_no_disco());
    assert!(t.query_by_label(IGNORAR).is_some());
    t.get_by_label(LER_DE_NOVO).click();
    t.run_steps(2);
    esperar_o_indice(&mut t);
    t.run_steps(2);
    assert!(!tela(&t).mudou_no_disco());
    assert_eq!(tela(&t).grade().map(|g| g.nucleo.n_linhas()), Some(LINHAS as usize + 5));
}

// ------------------------------------------------------------------ capturas

#[test]
fn capturas_da_planilha() {
    for (tema, nome) in [(Tema::Claro, "planilha-claro.png"), (Tema::Escuro, "planilha-escuro.png")] {
        let mut t =
            abrir_com(&format!("pl_cap_{nome}"), "relatorio_q3.xlsx", &relatorio_q3(1204), tema, TAMANHO, |t| t);
        esperar_o_indice(&mut t);
        let egui = t.ctx.clone();
        tela_mut(&mut t).mostrar_ocultas(&egui, true);
        tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((5, 5)));
        t.run_steps(4);
        capturar(&mut t, nome);
    }
}

/// As capturas do fechamento das etapas 5 e 6, a 1280×800: a planilha com o título mesclado, a linha 1 e a coluna A
/// congeladas e a oculta "Custos" na faixa (a caixa ligada), nos dois temas; e o card do erro (o arquivo com senha).
#[test]
fn capturas_do_fechamento() {
    let tela_pequena = egui::vec2(1280.0, 800.0);
    for (tema, nome) in [(Tema::Escuro, "etapa56-planilha-escuro.png"), (Tema::Claro, "etapa56-planilha-claro.png")] {
        let mut t =
            abrir_com(&format!("pl_cap56_{nome}"), "relatorio_q3.xlsx", &relatorio_q3(1204), tema, tela_pequena, |t| t);
        esperar_o_indice(&mut t);
        let egui = t.ctx.clone();
        tela_mut(&mut t).mostrar_ocultas(&egui, true);
        tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((5, 5)));
        t.run_steps(4);
        capturar(&mut t, nome);
    }
    let mut t = abrir_com(
        "pl_cap56_erro",
        "folha_pagamento.xlsx",
        &ole_com(&["Root Entry", "EncryptionInfo", "EncryptedPackage"]),
        Tema::Escuro,
        tela_pequena,
        |t| t,
    );
    capturar(&mut t, "etapa56-planilha-erro-escuro.png");
}

#[test]
fn captura_da_planilha_grande() {
    let x = Extras { dimensao: Some("A1:B200000".into()), ..Extras::default() };
    let bytes = Montador::novo()
        .aba_com("Lançamentos", &linhas_simples(200_000), &x)
        .aba("Contas", &linhas_simples(5))
        .aba("Plano de contas", &linhas_simples(5))
        .bytes();
    let mut t =
        abrir_com("pl_cap_grande", "movimentos_2025.xlsx", &bytes, Tema::Escuro, TAMANHO, |t| t.com_indice_manual());
    // a vista longe (as linhas sem página a 55%) e o índice lendo
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((150_000, 1)));
    t.run_steps(3);
    let egui = t.ctx.clone();
    tela_mut(&mut t).iniciar_indice(&egui);
    std::thread::sleep(tela_planilha::ESPERA_DA_BARRA);
    t.run_steps(2);
    capturar(&mut t, "planilha-grande-escuro.png");
}

#[test]
fn captura_dos_erros() {
    let mut t = abrir_com(
        "pl_cap_erros",
        "folha_pagamento.xlsx",
        &ole_com(&["Root Entry", "EncryptionInfo", "EncryptedPackage"]),
        Tema::Escuro,
        TAMANHO,
        |t| t,
    );
    capturar(&mut t, "planilha-erros-escuro.png");
    drop(t);
    let bytes = Montador::novo().aba("Cotações", &linhas_simples(3)).estado("hidden").bytes();
    let mut t = abrir_com("pl_cap_ocultas", "cotacoes.xlsx", &bytes, Tema::Escuro, TAMANHO, |t| t);
    capturar(&mut t, "planilha-sem-planilha-escuro.png");
}
