//! Passo 11 da etapa 5: editar o CSV (células, linhas, marcas, desfazer, salvar; prancheta 15).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use apoio::*;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Documento, Situacao, Verbo};
use interface::tema::Tema;
use tela_csv::Ganchos;

/// Começa a editar a célula com Enter, troca o texto todo por `texto` e aperta `fim`.
fn editar(h: &mut H, linha: usize, coluna: usize, texto: &str, fim: Key) {
    clicar_celula(h, linha, coluna);
    tecla(h, Key::Enter, Modifiers::NONE);
    assert!(doc(h).editando(), "a edição abriu");
    // o texto começa todo selecionado: digitar troca
    h.input_mut().events.push(Event::Text(texto.to_string()));
    h.step();
    h.run_steps(2);
    let shift = Modifiers::NONE;
    tecla(h, fim, shift);
    esperar_ociosa(h);
}

fn celula(h: &H, linha: u64, coluna: usize) -> String {
    doc(h).pagina().and_then(|p| p.celula(linha, coluna)).unwrap_or_default().to_string()
}

/// O que foi para a área de transferência nos próximos quadros.
fn copiado(h: &mut H) -> Option<String> {
    let inicio = Instant::now();
    while inicio.elapsed() < Duration::from_secs(10) {
        let achado = h.output().platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        });
        if achado.is_some() {
            return achado;
        }
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

fn ofertas(h: &H) -> estado::Ofertas {
    doc(h).ofertas(&h.state().estado)
}

#[test]
fn editar_celula_marca_alt_e_altera() {
    let amb = Ambiente::vazio("csv_editar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(30)));
    editar(&mut h, 6, 4, "21", Key::Enter);
    assert_eq!(celula(&h, 6, 4), "21");
    let p = doc(&h).pagina().unwrap();
    assert!(p.alterada(6, 4) && !p.alterada(6, 3), "só a célula digitada tem o td.alt");
    assert!(doc(&h).alterado());
    assert_eq!(doc(&h).alteracoes(), Some(1));
    assert!(h.query_by_label("· 1 alteração não salva").is_some());
    assert_eq!(doc(&h).status_do_arquivo(&h.state().estado).unwrap().situacao, Situacao::Alterado);
    // Enter grava e a grade fica com o foco na mesma célula
    assert_eq!(doc(&h).celula_ativa(), Some((6, 4)));
    assert!(!doc(&h).editando());
}

#[test]
fn editar_para_o_mesmo_texto_nao_altera() {
    let amb = Ambiente::vazio("csv_mesmo_texto");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    let antes = celula(&h, 2, 1);
    editar(&mut h, 2, 1, &antes, Key::Enter);
    assert!(!doc(&h).alterado(), "o mesmo texto não entra no desfazer");
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));
}

#[test]
fn enter_tab_e_esc_na_edicao() {
    let amb = Ambiente::vazio("csv_teclas");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    // Tab grava e anda para a célula seguinte
    editar(&mut h, 1, 1, "Oeste", Key::Tab);
    assert_eq!(celula(&h, 1, 1), "Oeste");
    assert_eq!(doc(&h).celula_ativa(), Some((1, 2)));
    // Esc desiste
    let antes = celula(&h, 3, 2);
    editar(&mut h, 3, 2, "Ninguém", Key::Escape);
    assert_eq!(celula(&h, 3, 2), antes);
    assert_eq!(doc(&h).alteracoes(), Some(1));
    // uma tecla de texto começa a edição com ela
    clicar_celula(&mut h, 4, 1);
    h.input_mut().events.push(Event::Text("N".into()));
    h.step();
    h.run_steps(2);
    assert!(doc(&h).editando());
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(celula(&h, 4, 1), "N");
}

#[test]
fn nova_linha_depois_da_ativa_e_no_fim_sem_ativa() {
    let amb = Ambiente::vazio("csv_nova_linha");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    // sem célula ativa: no fim
    h.get_by_label("Nova linha").click();
    esperar_ociosa(&mut h);
    let r = doc(&h).retrato().unwrap();
    assert_eq!(r.linhas, 6);
    assert_eq!(celula(&h, 5, 0), "");
    assert_eq!(doc(&h).celula_ativa().map(|(l, _)| l), Some(5), "a linha nova fica ativa");
    // com a célula ativa: logo depois dela
    clicar_celula(&mut h, 1, 2);
    h.get_by_label("Nova linha").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 7);
    assert_eq!(celula(&h, 2, 0), "");
    assert_eq!(celula(&h, 3, 0), "2025-01-04");
    assert_eq!(doc(&h).celula_ativa(), Some((2, 2)));
}

#[test]
fn duplicar_a_marcada_e_a_ativa() {
    let amb = Ambiente::vazio("csv_duplicar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(6)));
    clicar_celula(&mut h, 2, 0);
    assert_eq!(ofertas(&h).dica(Verbo::Duplicar), "Duplicar a linha 3");
    h.get_by_label("Duplicar").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 7);
    assert_eq!(doc(&h).pagina().unwrap().linha(3), doc(&h).pagina().unwrap().linha(2), "a cópia logo abaixo");
    // com marcas, as marcadas (e a dica diz quantas)
    doc_mut(&mut h).marcar_linhas([0, 5]);
    h.run_steps(2);
    assert_eq!(ofertas(&h).dica(Verbo::Duplicar), "Duplicar as 2 linhas marcadas");
    h.get_by_label("Duplicar").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 9);
    assert_eq!(celula(&h, 1, 0), "2025-01-02");
    assert_eq!(doc(&h).linhas_marcadas(), [0, 6], "as originais continuam marcadas");
}

#[test]
fn excluir_marcadas_e_a_ativa_com_a_dica_certa() {
    let amb = Ambiente::vazio("csv_excluir");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(8)));
    assert_eq!(ofertas(&h).dica(Verbo::Excluir), "Excluir");
    assert!(!ofertas(&h).ligado(Verbo::Excluir), "sem alvo, desligado");
    clicar_celula(&mut h, 6, 0);
    assert_eq!(ofertas(&h).dica(Verbo::Excluir), "Excluir a linha 7");
    // marcar pela caixa da linha
    let p = doc(&h).centro_marca(1).unwrap();
    clicar_em(&mut h, p, Modifiers::NONE);
    let p = doc(&h).centro_marca(3).unwrap();
    clicar_em(&mut h, p, Modifiers::NONE);
    assert_eq!(doc(&h).linhas_marcadas(), [1, 3]);
    assert_eq!(ofertas(&h).dica(Verbo::Excluir), "Excluir as 2 linhas marcadas");
    assert!(h.query_by_label("· 2 marcadas").is_some());
    h.get_by_label("Excluir").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 6);
    assert!(doc(&h).linhas_marcadas().is_empty());
    assert_eq!(celula(&h, 1, 0), "2025-01-04");
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer Excluir 2 linhas");
}

#[test]
fn marcas_acompanham_inserir_e_excluir() {
    let amb = Ambiente::vazio("csv_marcas");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(8)));
    doc_mut(&mut h).marcar_linhas([2, 5]);
    clicar_celula(&mut h, 0, 0);
    h.get_by_label("Nova linha").click();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).linhas_marcadas(), [3, 6], "a linha nova empurra as de baixo");
    // Delete com marcadas exclui as marcadas (a grade com o foco)
    clicar_celula(&mut h, 0, 0);
    tecla(&mut h, Key::Delete, Modifiers::NONE);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 7);
    // espaço marca a linha ativa
    tecla(&mut h, Key::Space, Modifiers::NONE);
    assert_eq!(doc(&h).linhas_marcadas(), [0]);
}

#[test]
fn desfazer_e_refazer_limpam_as_marcas() {
    let amb = Ambiente::vazio("csv_desfazer");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(8)));
    editar(&mut h, 1, 1, "Oeste", Key::Enter);
    doc_mut(&mut h).marcar_linhas([4]);
    h.get_by_label("Desfazer").click();
    esperar_ociosa(&mut h);
    assert_eq!(celula(&h, 1, 1), "Sudeste");
    assert!(doc(&h).linhas_marcadas().is_empty());
    assert!(!doc(&h).alterado());
    assert_eq!(ofertas(&h).dica(Verbo::Refazer), "Refazer Editar a célula");
    doc_mut(&mut h).marcar_linhas([4]);
    h.get_by_label("Refazer").click();
    esperar_ociosa(&mut h);
    assert_eq!(celula(&h, 1, 1), "Oeste");
    assert!(doc(&h).linhas_marcadas().is_empty());
}

#[test]
fn dica_do_desfazer_diz_o_passo() {
    let amb = Ambiente::vazio("csv_dica_desfazer");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(8)));
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));
    editar(&mut h, 1, 1, "Oeste", Key::Enter);
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer Editar a célula");
    clicar_celula(&mut h, 3, 0);
    h.get_by_label("Nova linha").click();
    esperar_ociosa(&mut h);
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer Nova linha");
}

#[test]
fn rotulos_divergentes_caem_no_generico() {
    let amb = Ambiente::vazio("csv_rotulos");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(8)));
    editar(&mut h, 1, 1, "Oeste", Key::Enter);
    doc_mut(&mut h).rotulos_do_desfazer().push("Passo fantasma".into());
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer (Ctrl+Z)");
}

#[test]
fn salvar_grava_e_zera_as_alteracoes() {
    let amb = Ambiente::vazio("csv_salvar");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(5));
    let mut h = abrir(&amb, caminho.clone());
    assert!(!ofertas(&h).ligado(Verbo::Salvar));
    editar(&mut h, 0, 4, "99", Key::Enter);
    assert_eq!(ofertas(&h).dica(Verbo::Salvar), "Salvar vendas_2025.csv (Ctrl+S)");
    h.get_by_label("Salvar").click();
    esperar_ociosa(&mut h);
    let gravado = std::fs::read_to_string(&caminho).unwrap();
    assert!(gravado.contains("2025-01-02;Sul;Ana Coutinho;Caderno universitário;99;12,90\r\n"), "{gravado}");
    assert!(!doc(&h).alterado());
    assert_eq!(doc(&h).status_do_arquivo(&h.state().estado).unwrap().situacao, Situacao::Salvo);
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("vendas_2025.csv salvo."));
    assert!(!ofertas(&h).ligado(Verbo::Desfazer), "o desfazer recomeça depois de salvar");
    assert!(!doc(&h).pagina().unwrap().alterada(0, 4));
}

/// Um gancho que segura os trabalhos da fila enquanto `segurar` estiver ligado.
fn gancho_de_segurar() -> (Ganchos, Arc<AtomicBool>) {
    let segurar = Arc::new(AtomicBool::new(false));
    let s2 = Arc::clone(&segurar);
    let ganchos = Ganchos {
        trabalho: Some(Arc::new(move || {
            let inicio = Instant::now();
            while s2.load(Ordering::SeqCst) && inicio.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(5));
            }
        })),
        ..Ganchos::default()
    };
    (ganchos, segurar)
}

#[test]
fn salvar_longo_mostra_o_aviso_e_cancela() {
    let amb = Ambiente::vazio("csv_salvar_longo");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(5));
    let original = std::fs::read(&caminho).unwrap();
    let (ganchos, segurar) = gancho_de_segurar();
    let mut h = palco(&amb, caminho.clone(), Tema::Claro, TAMANHO, ganchos);
    esperar_ociosa(&mut h);
    editar(&mut h, 0, 4, "99", Key::Enter);
    segurar.store(true, Ordering::SeqCst);
    h.get_by_label("Salvar").click();
    h.run_steps(4);
    assert!(doc(&h).salvando());
    assert!(h.query_by_label_contains("Salvando vendas_2025.csv").is_some(), "o aviso depois de 0,4 s");
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    segurar.store(false, Ordering::SeqCst);
    esperar_ociosa(&mut h);
    assert_eq!(std::fs::read(&caminho).unwrap(), original, "o arquivo ficou como estava");
    assert!(doc(&h).alterado());
    assert!(doc_mut(&mut h).tomar_erro_de_gravacao().is_some(), "a fila de perguntas ao fechar para");
    assert!(!doc(&h).salvando());
}

#[test]
fn arquivo_mudou_no_disco_pergunta_ao_salvar() {
    let amb = Ambiente::vazio("csv_mudou");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(5));
    let mut h = abrir(&amb, caminho.clone());
    editar(&mut h, 0, 4, "99", Key::Enter);
    // outro programa grava o arquivo: o núcleo regrava a partir do arquivo de origem, então nada pode ser gravado
    // por cima (as linhas de antes não existem mais); a tela diz e oferece ler de novo
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&caminho, vendas(9)).unwrap();
    h.get_by_label("Salvar").click();
    esperar_ociosa(&mut h);
    assert_eq!(std::fs::read_to_string(&caminho).unwrap(), vendas(9), "nada foi gravado");
    assert!(doc(&h).alterado());
    let erro = doc_mut(&mut h).tomar_erro_de_gravacao().expect("o erro da gravação");
    assert!(erro.starts_with("O arquivo mudou no disco desde que foi aberto"), "{erro}");
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx), Some(erro));
    assert_eq!(doc(&h).status_do_arquivo(&h.state().estado).unwrap().situacao, Situacao::ComErro);
    assert!(h.query_by_label("O arquivo mudou no disco.").is_some(), "o aviso de topo");
    h.get_by_label("Ler de novo").click();
    h.run_steps(2);
    h.get_by_label("Descartar").click();
    esperar_ociosa(&mut h);
    assert!(!doc(&h).alterado());
    assert_eq!(doc(&h).retrato().unwrap().linhas, 9);
}

#[test]
fn edicao_ignorada_enquanto_salva() {
    let amb = Ambiente::vazio("csv_ignora");
    let (ganchos, segurar) = gancho_de_segurar();
    let mut h = palco(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)), Tema::Claro, TAMANHO, ganchos);
    esperar_ociosa(&mut h);
    editar(&mut h, 0, 4, "99", Key::Enter);
    segurar.store(true, Ordering::SeqCst);
    h.get_by_label("Salvar").click();
    h.run_steps(2);
    clicar_celula(&mut h, 1, 1);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    assert!(!doc(&h).editando(), "enquanto grava, nada se edita");
    assert!(!ofertas(&h).ligado(Verbo::Novo) && !ofertas(&h).ligado(Verbo::Desfazer));
    segurar.store(false, Ordering::SeqCst);
    esperar_ociosa(&mut h);
    assert!(ofertas(&h).ligado(Verbo::Novo));
}

#[test]
fn delete_repetido_com_a_exclusao_na_fila_exclui_uma_linha_so() {
    let amb = Ambiente::vazio("csv_delete_repetido");
    let (ganchos, segurar) = gancho_de_segurar();
    let mut h = palco(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)), Tema::Claro, TAMANHO, ganchos);
    esperar_ociosa(&mut h);
    let (sexta, setima) = (celula(&h, 6, 0), celula(&h, 7, 0));
    doc_mut(&mut h).marcar_linhas([5]);
    clicar_celula(&mut h, 0, 0);
    // a máquina lenta: a exclusão fica na fila, e o segundo Delete (a tecla repetida) chega antes da resposta
    segurar.store(true, Ordering::SeqCst);
    tecla(&mut h, Key::Delete, Modifiers::NONE);
    tecla(&mut h, Key::Delete, Modifiers::NONE);
    assert!(!ofertas(&h).ligado(Verbo::Novo), "com a exclusão sem resposta, o que vai por posição espera");
    assert!(!ofertas(&h).ligado(Verbo::Duplicar) && !ofertas(&h).ligado(Verbo::Excluir));
    clicar_celula(&mut h, 2, 1);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    assert!(!doc(&h).editando(), "nem a célula abre (a posição dela é de antes da exclusão)");
    segurar.store(false, Ordering::SeqCst);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).retrato().unwrap().linhas, 9, "uma linha só saiu");
    assert_eq!(celula(&h, 5, 0), sexta, "a antiga linha 7 sobe e fica");
    assert_eq!(celula(&h, 6, 0), setima);
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer Excluir a linha");
    assert!(ofertas(&h).ligado(Verbo::Novo));
}

#[test]
fn celula_aberta_grava_antes_da_alteracao_seguinte() {
    let amb = Ambiente::vazio("csv_celula_antes");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(6)));
    clicar_celula(&mut h, 2, 1);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    h.input_mut().events.push(Event::Text("Oeste".into()));
    h.step();
    h.run_steps(2);
    assert!(doc(&h).editando());
    // o Duplicar com a célula ainda aberta: ela grava primeiro, e a cópia já sai com o texto novo
    h.get_by_label("Duplicar").click();
    esperar_ociosa(&mut h);
    assert!(!doc(&h).editando());
    assert_eq!(doc(&h).retrato().unwrap().linhas, 7);
    assert_eq!(celula(&h, 2, 1), "Oeste");
    assert_eq!(celula(&h, 3, 1), "Oeste", "a cópia tem o texto da célula que estava em edição");
    assert_eq!(celula(&h, 4, 1), "Norte");
}

#[test]
fn fechar_com_a_celula_em_edicao_conta_como_alterado() {
    let amb = Ambiente::vazio("csv_fechar_editando");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(5));
    let (ganchos, segurar) = gancho_de_segurar();
    let mut h = palco(&amb, caminho.clone(), Tema::Claro, TAMANHO, ganchos);
    esperar_ociosa(&mut h);
    assert!(!doc(&h).alterado());
    // a célula aberta com outro texto, num arquivo sem alteração: o Ctrl+W tem de perguntar
    clicar_celula(&mut h, 1, 1);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    h.input_mut().events.push(Event::Text("Oeste".into()));
    h.step();
    h.run_steps(2);
    assert!(doc(&h).editando());
    assert!(doc(&h).alterado(), "a célula em edição com outro texto conta");
    assert_eq!(doc(&h).alteracoes(), Some(1));
    // gravada e ainda na fila (a tarefa segura): continua contando
    segurar.store(true, Ordering::SeqCst);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    assert!(!doc(&h).editando());
    assert!(!doc(&h).retrato().unwrap().alterada, "o núcleo ainda não respondeu");
    assert!(doc(&h).alterado(), "a alteração na fila conta");
    assert_eq!(doc(&h).alteracoes(), Some(1));
    assert!(ofertas(&h).ligado(Verbo::Salvar), "o Salvar da pergunta grava depois dela");
    segurar.store(false, Ordering::SeqCst);
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).alteracoes(), Some(1));
    // o Salvar (o da pergunta ao fechar) com a célula aberta grava a célula e o arquivo
    clicar_celula(&mut h, 2, 1);
    tecla(&mut h, Key::Enter, Modifiers::NONE);
    h.input_mut().events.push(Event::Text("Leste".into()));
    h.step();
    h.run_steps(2);
    h.get_by_label("Salvar").click();
    esperar_ociosa(&mut h);
    assert!(!doc(&h).alterado());
    let gravado = std::fs::read_to_string(&caminho).unwrap();
    assert!(gravado.contains(";Oeste;") && gravado.contains(";Leste;"), "{gravado}");
}

#[test]
fn fechar_com_o_cabecalho_em_edicao_conta_como_alterado() {
    let amb = Ambiente::vazio("csv_fechar_cabecalho");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(3));
    let mut h = abrir(&amb, caminho.clone());
    h.get_by_label("Nova coluna").click();
    h.run_steps(3);
    assert!(doc(&h).alterado(), "a Nova coluna ainda sem nome confirmado conta");
    assert_eq!(doc(&h).alteracoes(), Some(1));
    // o Salvar da pergunta ao fechar (a casca chama o verbo direto) confirma o cabeçalho antes de gravar
    let ectx = h.ctx.clone();
    let p = h.state_mut();
    let mut acoes = Vec::new();
    let mut c = estado::Contexto::novo(&p.servico, &p.estado, None, &mut acoes);
    p.tela.doc.executar(Verbo::Salvar, &ectx, &mut c);
    esperar_ociosa(&mut h);
    assert!(!doc(&h).alterado());
    let primeira = std::fs::read_to_string(&caminho).unwrap().lines().next().unwrap_or_default().to_string();
    assert_eq!(primeira, "data;regiao;vendedor;produto;quantidade;valor;coluna_7");
}

#[test]
fn ler_paginas_e_conferir_o_disco_nao_mudam_a_versao() {
    let amb = Ambiente::vazio("csv_versao");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(2000)));
    let v = doc(&h).versao();
    // rolar até o fim (outras páginas) e a volta do foco (a conferência do disco)
    doc_mut(&mut h).selecionar(Some((1990, 0)));
    h.run_steps(4);
    esperar_ociosa(&mut h);
    assert!(doc(&h).pagina().unwrap().contem(1990), "a página do fim chegou");
    h.input_mut().focused = false;
    h.step();
    h.input_mut().focused = true;
    h.step();
    esperar_ociosa(&mut h);
    assert_eq!(doc(&h).versao(), v, "nada mudou no conteúdo");
    // uma alteração anda a versão (a vista do filtro e as estatísticas se refazem)
    editar(&mut h, 1990, 1, "Oeste", Key::Enter);
    assert_eq!(doc(&h).versao(), v + 1);
}

#[test]
fn refazer_fora_de_passo_cai_no_generico() {
    let amb = Ambiente::vazio("csv_refazer_generico");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(8)));
    editar(&mut h, 1, 1, "Oeste", Key::Enter);
    h.get_by_label("Desfazer").click();
    esperar_ociosa(&mut h);
    assert_eq!(ofertas(&h).dica(Verbo::Refazer), "Refazer Editar a célula");
    doc_mut(&mut h).rotulos_do_refazer().push("Passo fantasma".into());
    assert_eq!(ofertas(&h).dica(Verbo::Refazer), "Refazer (Ctrl+Y)");
}

#[test]
fn ctrl_c_copia_a_celula_ativa() {
    let amb = Ambiente::vazio("csv_copiar_celula");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    clicar_celula(&mut h, 2, 3);
    h.input_mut().events.push(Event::Copy);
    h.step();
    assert_eq!(copiado(&mut h).as_deref(), Some("Estojo duplo"));
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("Copiado."));
}

#[test]
fn ctrl_c_copia_o_intervalo_em_tsv_mesmo_alem_da_pagina() {
    let amb = Ambiente::vazio("csv_copiar_intervalo");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(3000)));
    // Shift com as setas estende da célula ativa: duas linhas × duas colunas, na página em mãos
    clicar_celula(&mut h, 1, 1);
    tecla(&mut h, Key::ArrowDown, Modifiers::SHIFT);
    tecla(&mut h, Key::ArrowRight, Modifiers::SHIFT);
    h.input_mut().events.push(Event::Copy);
    h.step();
    let texto = copiado(&mut h).expect("copiou");
    let esperado =
        format!("{}\t{}\r\n{}\t{}\r\n", celula(&h, 1, 1), celula(&h, 1, 2), celula(&h, 2, 1), celula(&h, 2, 2));
    assert_eq!(texto, esperado);
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("2 linhas × 2 colunas copiadas."));
    // até o fim (Ctrl+Shift+End): as linhas fora da página são lidas numa tarefa
    clicar_celula(&mut h, 0, 0);
    let primeira = celula(&h, 0, 0);
    tecla(&mut h, Key::End, Modifiers::COMMAND | Modifiers::SHIFT);
    h.input_mut().events.push(Event::Copy);
    h.step();
    let texto = copiado(&mut h).expect("copiou");
    assert_eq!(texto.matches("\r\n").count(), 3000, "as 3.000 linhas, sem o cabeçalho");
    assert!(texto.starts_with(&format!("{primeira}\t")), "{:?}", &texto[..40]);
    esperar_ociosa(&mut h);
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("3.000 linhas × 6 colunas copiadas."));
}

#[test]
fn ctrl_c_com_marcas_copia_as_linhas_em_tsv() {
    let amb = Ambiente::vazio("csv_copiar_linhas");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    doc_mut(&mut h).marcar_linhas([0, 2]);
    clicar_celula(&mut h, 4, 0);
    h.input_mut().events.push(Event::Copy);
    h.step();
    let texto = copiado(&mut h).expect("copiou");
    assert_eq!(
        texto,
        "data\tregiao\tvendedor\tproduto\tquantidade\tvalor\r\n\
         2025-01-02\tSul\tAna Coutinho\tCaderno universitário\t1\t12,90\r\n\
         2025-01-04\tNordeste\tCarla Ibiapina\tEstojo duplo\t15\t474,00\r\n"
    );
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("2 linhas copiadas."));
}

#[test]
fn rodape_conta_alteracoes_e_marcadas() {
    let amb = Ambiente::vazio("csv_rodape");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(10)));
    editar(&mut h, 0, 1, "A", Key::Enter);
    editar(&mut h, 1, 1, "B", Key::Enter);
    editar(&mut h, 2, 1, "C", Key::Enter);
    doc_mut(&mut h).marcar_linhas([4, 5]);
    h.run_steps(2);
    assert!(h.query_by_label("Linhas 1–10 de 10").is_some());
    assert!(h.query_by_label("· 3 alterações não salvas").is_some());
    assert!(h.query_by_label("· 2 marcadas").is_some());
}

#[test]
fn atualizar_com_alteracao_pergunta() {
    let amb = Ambiente::vazio("csv_atualizar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    editar(&mut h, 0, 1, "Oeste", Key::Enter);
    h.get_by_label("Atualizar").click();
    h.run_steps(2);
    assert!(h.query_by_label_contains("Descartar as alterações de vendas_2025.csv e ler o arquivo de novo?").is_some());
    h.get_by_label("Descartar").click();
    esperar_ociosa(&mut h);
    assert!(!doc(&h).alterado());
    assert_eq!(celula(&h, 0, 1), "Sul");
}

#[test]
fn formato_desligado_com_alteracao_pendente() {
    let amb = Ambiente::vazio("csv_formato_travado");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    let separador = |h: &H| h.get(by().role(Role::ComboBox).label("Separador")).accesskit_node().is_disabled();
    assert!(!separador(&h));
    editar(&mut h, 0, 1, "Oeste", Key::Enter);
    assert!(separador(&h), "com alteração pendente, o Formato não muda");
    assert!(h.get(by().role(Role::CheckBox).label("Primeira linha é o cabeçalho")).accesskit_node().is_disabled());
}

#[test]
fn capturas_editado_e_marcadas_contra_as_pranchetas_7_e_15() {
    let amb = Ambiente::vazio("csv_capturas_edicao");
    let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)), Tema::Escuro, TAMANHO);
    editar(&mut h, 6, 4, "21", Key::Enter);
    // a prancheta 7: a célula 7 × quantidade alterada e ativa, "1 alteração não salva"
    capturar(&mut h, "csv-alterado-escuro.png");
    drop(h);
    let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)), Tema::Escuro, TAMANHO);
    doc_mut(&mut h).marcar_linhas([3, 4]);
    clicar_celula(&mut h, 4, 0);
    capturar(&mut h, "csv-marcadas-escuro.png");
}
