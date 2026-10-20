//! O editor de JSON no `egui_kittest` (pranchetas 8 e 15, estado N1): o círculo de estado, a frase do
//! erro e o clique que leva a ele, a faixa de erro com "Ir para o erro", a linha do erro, o Formatar (que preserva a
//! ordem das chaves e é um passo do desfazer), a estrutura do documento com páginas de 200, a migalha e o rodapé que
//! acompanham o cursor, e o leitor com a validação feita uma vez. As capturas `json-*` e `json-invalido-*` vão para
//! `capturas/rust/`, para comparar com as pranchetas 8 e 15.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui_kittest::kittest::{NodeT, Queryable};
use estado::{Documento, Verbo};
use interface::pecas::toast::{toast_atual, toast_concluido};
use interface::tema::Tema;
use tela_texto::Modo;

const ERRO_DA_PRANCHETA: &str = "JSON inválido na linha 14, coluna 7: falta uma vírgula ou um } antes de \"arquivo\"";

fn ofertas(t: &H) -> estado::Ofertas {
    t.state().tela.doc.ofertas(&t.state().estado)
}

/// Põe o cursor do campo em `indice` (caracteres) e roda quadros.
fn cursor_em(t: &mut H, indice: usize) {
    let id = t.state().tela.doc.id_do_campo();
    let mut estado = TextEditState::load(&t.ctx, id).unwrap_or_default();
    estado.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(indice))));
    estado.store(&t.ctx, id);
    t.run_steps(2);
}

/// Põe o cursor logo depois da primeira ocorrência de `trecho` no texto da tela.
fn cursor_depois_de(t: &mut H, trecho: &str) {
    let indice = depois_de(t, trecho);
    cursor_em(t, indice);
}

/// O cursor do campo, em caracteres.
fn cursor(t: &H) -> usize {
    let id = t.state().tela.doc.id_do_campo();
    TextEditState::load(&t.ctx, id).and_then(|e| e.cursor.char_range()).map_or(0, |r| r.primary.index.0)
}

/// O índice (em caracteres) logo depois da primeira ocorrência de `trecho` no texto da tela.
fn depois_de(t: &H, trecho: &str) -> usize {
    let texto = t.state().tela.doc.texto().expect("o texto").to_string();
    let byte = texto.find(trecho).expect("o trecho") + trecho.len();
    texto[..byte].chars().count()
}

/// Espera a validação do JSON da tela (a leitura já traz a do arquivo; depois de digitar, vem 250 ms depois).
fn esperar_a_validacao(t: &mut Teste) {
    esperar_ate(t, |h| {
        let (pedidas, _) = h.state().tela.doc.contas_da_validacao();
        pedidas >= 1 && !h.state().tela.doc.ocupada()
    });
}

#[test]
fn analisa_o_json_da_prancheta_8_e_mostra_a_estrutura() {
    let t = abrir("json_prancheta", "config.json", JSON_DA_PRANCHETA.as_bytes());
    let doc = &t.state().tela.doc;
    assert_eq!(doc.modo(), Modo::Editor);
    assert!(doc.erro_de_json().is_none());
    // os cinco nomes da raiz (aberta sozinha); `conexoes` e `exportacao` abrem pela seta
    for nome in ["raiz", "versao", "tema", "idioma", "conexoes", "exportacao"] {
        assert!(t.query_by_label(nome).is_some(), "{nome}");
    }
    assert!(t.query_by_label("2 itens").is_none(), "a contagem é pintada, não é um nó");
}

#[test]
fn json_valido_mostra_o_circulo_verde_e_a_dica() {
    let t = abrir("json_valido", "config.json", JSON_DA_PRANCHETA.as_bytes());
    assert!(t.query_by_label("JSON válido").is_some());
    assert!(t.state().tela.doc.erro_de_json().is_none());
    // o Formatar está ligado
    assert!(!t.get_by_label("Formatar").accesskit_node().is_disabled());
}

#[test]
fn json_invalido_mostra_a_frase_e_o_clique_leva_ao_erro() {
    let mut t = abrir("json_invalido", "config.json", json_invalido_da_prancheta().as_bytes());
    let doc = &t.state().tela.doc;
    let erro = doc.erro_de_json().expect("o erro da prancheta 15");
    assert_eq!((erro.linha, erro.coluna), (14, 7));
    assert_eq!(erro.to_string(), ERRO_DA_PRANCHETA);
    // N1: o círculo vira o ícone de erro, com a frase no rótulo (e na dica)
    assert!(t.query_by_label(ERRO_DA_PRANCHETA).is_some());
    assert!(t.query_by_label("JSON válido").is_none());
    // o clique leva o cursor ao erro: linha 14, coluna 7, e o rodapé diz
    t.get_by_label(ERRO_DA_PRANCHETA).click();
    t.run_steps(3);
    assert!(t.query_by_label("Linha 14, coluna 7").is_some(), "o cursor foi ao erro");
    let esperado = {
        let texto = texto(&t);
        let byte = texto.find("\"arquivo\": \"D:").expect("o símbolo do erro");
        texto[..byte].chars().count()
    };
    assert_eq!(cursor(&t), esperado);
}

/// A frase da faixa de erro da prancheta 15 (o lugar em `--perigo` e o motivo).
const FAIXA_DA_PRANCHETA: &str = "Linha 14, coluna 7: falta uma vírgula ou um } antes de \"arquivo\".";

#[test]
fn json_invalido_mostra_a_faixa_de_erro_e_ir_para_o_erro() {
    let mut t = abrir("json_faixa_erro", "config.json", json_invalido_da_prancheta().as_bytes());
    // a faixa de erro (`role="alert"`) com o lugar e o motivo, e o botão "Ir para o erro"
    assert!(t.query_by_label(FAIXA_DA_PRANCHETA).is_some());
    assert!(t.query_all_by_role(egui::accesskit::Role::Alert).count() == 1);
    let faixa = t.get_by_label(FAIXA_DA_PRANCHETA).rect();
    let botao = t.get_by_label(tela_texto::IR_PARA_O_ERRO).rect();
    assert!(botao.left() > faixa.right(), "o botão fica na ponta direita, depois da frase");
    assert!((botao.center().y - faixa.center().y).abs() < 1.5, "centrados na altura da linha: {botao:?} {faixa:?}");
    // o editor começa embaixo da faixa
    let editor = t.get_by_label("Texto de config.json").rect();
    assert!(editor.top() >= botao.bottom(), "{editor:?} {botao:?}");
    // "Ir para o erro" leva o cursor ao erro, como o círculo
    t.get_by_label(tela_texto::IR_PARA_O_ERRO).click();
    t.run_steps(3);
    assert!(t.query_by_label("Linha 14, coluna 7").is_some(), "o cursor foi ao erro");
    let esperado = {
        let texto = texto(&t);
        let byte = texto.find("\"arquivo\": \"D:").expect("o símbolo do erro");
        texto[..byte].chars().count()
    };
    assert_eq!(cursor(&t), esperado);
    // consertado o JSON, a faixa sai e o editor sobe
    digitar_no_cursor(&mut t, ", ");
    esperar_ate(&mut t, |h| h.state().tela.doc.erro_de_json().is_none());
    t.run_steps(2);
    assert!(t.query_by_label(tela_texto::IR_PARA_O_ERRO).is_none());
    assert_eq!(t.query_all_by_role(egui::accesskit::Role::Alert).count(), 0);
    assert!(t.get_by_label("Texto de config.json").rect().top() < editor.top());
}

#[test]
fn linha_do_erro_com_fundo_e_sublinhado() {
    let mut t = abrir("json_linha_erro", "config.json", json_invalido_da_prancheta().as_bytes());
    let (linha, caracteres) = t.state_mut().tela.doc.marca_do_erro().expect("a marca do erro");
    assert_eq!(linha, 13, "a linha 14, de 0");
    let texto = texto(&t);
    let marcado: String = texto.chars().skip(caracteres.start).take(caracteres.len()).collect();
    assert_eq!(marcado, "\"arquivo\"", "o sublinhado ondulado vai sobre o símbolo que estraga");
    // depois de editar, a marca volta com a validação do texto novo (a de antes não vale: o lugar dela pode ter se mexido)
    digitar(&mut t, "config.json", " ");
    esperar_a_validacao(&mut t);
    assert!(t.state_mut().tela.doc.marca_do_erro().is_some());
}

#[test]
fn formatar_desliga_com_erro() {
    let mut t = abrir("formatar_erro", "config.json", json_invalido_da_prancheta().as_bytes());
    assert!(t.get_by_label("Formatar").accesskit_node().is_disabled(), "N1: o Formatar fica desligado");
    // consertar o JSON liga o Formatar de novo
    let doc_texto = texto(&t);
    let falta = doc_texto.find("\"Relatórios\"").expect("o trecho") + "\"Relatórios\"".len();
    cursor_em(&mut t, doc_texto[..falta].chars().count());
    digitar_no_cursor(&mut t, ",");
    esperar_ate(&mut t, |h| h.state().tela.doc.erro_de_json().is_none());
    assert!(!t.get_by_label("Formatar").accesskit_node().is_disabled());
    assert!(t.query_by_label("JSON válido").is_some());
}

/// Digita no campo que já tem o foco e o cursor (sem refocar, que o levaria ao começo).
fn digitar_no_cursor(t: &mut H, texto: &str) {
    t.get_by_label("Texto de config.json").focus();
    t.run_steps(2);
    t.get_by_label("Texto de config.json").type_text(texto);
    t.run_steps(2);
}

#[test]
fn formatar_e_um_passo_no_desfazer() {
    let compacto = "{\"b\":1,\"a\":[1,2],\"c\":{\"d\":null}}";
    let mut t = abrir("formatar_passo", "dados.json", compacto.as_bytes());
    clicar(&mut t, "Formatar");
    let formatado = texto(&t);
    assert_eq!(formatado, "{\n  \"b\": 1,\n  \"a\": [\n    1,\n    2\n  ],\n  \"c\": {\n    \"d\": null\n  }\n}");
    assert!(t.state().tela.doc.alterado());
    assert!(ofertas(&t).ligado(Verbo::Desfazer));
    // um Desfazer volta ao texto de antes, inteiro; um Refazer, ao formatado
    clicar(&mut t, "Desfazer");
    assert_eq!(texto(&t), compacto);
    assert!(!t.state().tela.doc.alterado(), "o compacto é o texto salvo");
    clicar(&mut t, "Refazer");
    assert_eq!(texto(&t), formatado);
}

#[test]
fn formatar_depois_de_desfazer_esvazia_o_refazer() {
    // o Refazer que sobrava de um Desfazer não pode trazer de volta, por cima do Formatar, o passo desfeito
    let compacto = "{\"b\":1,\"a\":[1,2]}";
    let mut t = abrir("formatar_refazer", "dados.json", compacto.as_bytes());
    clicar(&mut t, "Formatar");
    let formatado = texto(&t);
    clicar(&mut t, "Desfazer");
    assert_eq!(texto(&t), compacto);
    assert!(ofertas(&t).ligado(Verbo::Refazer));
    clicar(&mut t, "Formatar");
    assert_eq!(texto(&t), formatado);
    assert!(!ofertas(&t).ligado(Verbo::Refazer), "o Formatar é um passo novo: o refazer se esvazia");
    clicar(&mut t, "Desfazer");
    assert_eq!(texto(&t), compacto);
}

#[test]
fn formatar_que_passaria_do_limite_de_edicao_e_recusado() {
    // 450 KB de lista compacta viram mais de 1 MB formatados: o editor recusa e o texto fica como estava
    let mut compacto = String::from("[1");
    while compacto.len() < 450 * 1024 {
        compacto.push_str(",1");
    }
    compacto.push(']');
    let mut t = abrir("formatar_grande", "lista.json", compacto.as_bytes());
    assert_eq!(t.state().tela.doc.modo(), Modo::Editor);
    clicar(&mut t, "Formatar");
    assert_eq!(texto(&t), compacto);
    assert!(!t.state().tela.doc.alterado());
    assert_eq!(toast_atual(&t.ctx).as_deref(), Some(tela_texto::MSG_FORMATADO_GRANDE_DEMAIS));
}

#[test]
fn formatar_preserva_a_ordem_na_tela() {
    let original = "{\"zeta\": 1.0, \"alfa\": 1e5, \"meio\": \"a\\u00e7\\n\", \"zeta\": [ ], \"v\": -0}";
    let mut t = abrir("formatar_ordem", "ordem.json", original.as_bytes());
    clicar(&mut t, "Formatar");
    let f = texto(&t);
    // a ordem das chaves (inclusive a repetida), o texto dos números e o das cadeias ficam como estão
    let ordem: Vec<usize> =
        ["\"zeta\": 1.0", "\"alfa\": 1e5", "\"meio\": \"a\\u00e7\\n\"", "\"zeta\": []", "\"v\": -0"]
            .iter()
            .map(|s| f.find(s).unwrap_or_else(|| panic!("falta {s} em {f}")))
            .collect();
    assert!(ordem.windows(2).all(|p| p[0] < p[1]), "{f}");
    // salvar grava o formatado, com o fim de linha do arquivo
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    // o arquivo não tinha quebra nenhuma: o fim de linha é o padrão, CRLF (`texto::arquivo`), e o editor trabalha com `\n`
    assert_eq!(std::fs::read_to_string(&t.caminho).expect("ler").replace("\r\n", "\n"), f);
}

#[test]
fn formatar_json_ja_formatado_diz_e_nao_mexe() {
    let mut t = abrir("formatar_igual", "a.json", b"{\n  \"a\": 1\n}\n");
    clicar(&mut t, "Formatar");
    assert_eq!(toast_atual(&t.ctx).as_deref(), Some("O JSON já está formatado."));
    assert!(!t.state().tela.doc.alterado());
}

#[test]
fn estrutura_mostra_a_ultima_versao_valida_em_t3() {
    let mut t = abrir("estrutura_velha", "config.json", JSON_DA_PRANCHETA.as_bytes());
    assert!(!t.state().tela.doc.estrutura_e_a_ultima_valida());
    // estraga o JSON: a estrutura continua a da última versão válida
    let texto_atual = texto(&t);
    let depois_da_chave = texto_atual.find("\"versao\"").expect("chave") + "\"versao\"".len();
    cursor_em(&mut t, texto_atual[..depois_da_chave].chars().count());
    digitar_no_cursor(&mut t, "x");
    esperar_ate(&mut t, |h| h.state().tela.doc.erro_de_json().is_some());
    assert!(t.state().tela.doc.estrutura_e_a_ultima_valida());
    assert!(
        t.query_by_label("raiz").is_some() && t.query_by_label("conexoes").is_some(),
        "a árvore velha continua à vista"
    );
}

#[test]
fn estrutura_com_mais_de_200_filhos_tem_mais_n_itens() {
    let itens: Vec<String> = (0..250).map(|i| i.to_string()).collect();
    let json = format!("[{}]", itens.join(", "));
    let mut t = abrir("estrutura_paginas", "lista.json", json.as_bytes());
    // a raiz mostra 200 itens e a linha "mais 50 itens"
    assert!(t.query_by_label("199").is_some() && t.query_by_label("200").is_none());
    assert!(t.query_by_label("mais 50 itens em raiz").is_some());
    assert!(t.query_by_label("250 itens").is_none(), "a contagem é do nó, não da linha");
    // o clique mostra os 50 seguintes e a linha some
    t.get_by_label("mais 50 itens em raiz").scroll_to_me();
    t.run_steps(3);
    clicar(&mut t, "mais 50 itens em raiz");
    assert!(t.query_by_label("249").is_some());
    assert!(t.query_by_label("mais 50 itens em raiz").is_none());
}

#[test]
fn clique_no_no_leva_o_cursor() {
    let mut t = abrir("clique_no_no", "config.json", JSON_DA_PRANCHETA.as_bytes());
    // abre os pais do nó que o cursor acha: é preciso o `conexoes` e o `0` abertos
    cursor_depois_de(&mut t, "\"tempo_limite\": 3");
    assert!(t.query_by_label("conexoes › 0 › tempo_limite").is_some());
    cursor_em(&mut t, 0);
    t.get_by_label("conexoes › 0 › tempo_limite").click();
    t.run_steps(3);
    // o cursor vai ao começo do valor: linha 9, coluna 23 (depois de `      "tempo_limite": `)
    assert!(t.query_by_label("Linha 9, coluna 23").is_some());
    let inicio_do_valor = depois_de(&t, "\"tempo_limite\": ");
    assert_eq!(cursor(&t), inicio_do_valor);
}

/// Há na faixa (o alto do documento) um nó de acessibilidade com este rótulo?
fn na_faixa(t: &H, rotulo: &str) -> bool {
    t.query_all_by_label(rotulo).any(|n| n.rect().top() < 100.0)
}

#[test]
fn migalha_acompanha_o_cursor() {
    let mut t = abrir("migalha", "config.json", JSON_DA_PRANCHETA.as_bytes());
    // no começo do documento (na raiz) só há o nome do arquivo
    assert!(na_faixa(&t, "config.json"));
    assert!(!na_faixa(&t, "conexoes"));
    // com o cursor no `30` do tempo_limite: conexoes › 0 › tempo_limite
    cursor_depois_de(&mut t, "\"tempo_limite\": 30");
    for nome in ["config.json", "conexoes", "0", "tempo_limite"] {
        assert!(na_faixa(&t, nome), "{nome}");
    }
    // com o cursor no nome do segundo item: conexoes › 1 › nome
    cursor_depois_de(&mut t, "\"Relatórios\"");
    for nome in ["conexoes", "1", "nome"] {
        assert!(na_faixa(&t, nome), "{nome}");
    }
    assert!(!na_faixa(&t, "tempo_limite") && !na_faixa(&t, "0"));
    // e a estrutura acompanha: o nó do cursor se abre
    assert!(t.query_by_label("conexoes › 1 › nome").is_some());
}

#[test]
fn rodape_diz_linhas_espacos_e_posicao() {
    let mut t = abrir("rodape_json", "config.json", JSON_DA_PRANCHETA.as_bytes());
    cursor_depois_de(&mut t, "\"tempo_limite\": 30");
    assert!(t.query_by_label("24 linhas").is_some());
    assert!(t.query_by_label("· Espaços: 2").is_some());
    assert!(t.query_by_label("Linha 9, coluna 25").is_some(), "a posição da prancheta 8");
    // com tabulação e sem recuo
    let t2 = abrir("rodape_tabs", "t.json", b"{\n\t\"a\": 1\n}\n");
    assert!(t2.query_by_label("· Tabulação").is_some());
    let t3 = abrir("rodape_sem_recuo", "s.json", b"{\"a\": 1}\n");
    assert!(t3.query_by_label("· Sem recuo").is_some());
}

#[test]
fn validacao_espera_250_ms() {
    let mut t = abrir("validacao_espera", "config.json", JSON_DA_PRANCHETA.as_bytes());
    assert_eq!(t.state().tela.doc.contas_da_validacao().0, 0, "a leitura já trouxe a análise do arquivo");
    digitar(&mut t, "config.json", " ");
    // o `digitar` roda quadros de 250 ms: a análise foi pedida uma vez só, depois da pausa
    esperar_a_validacao(&mut t);
    let (pedidas, descartadas) = t.state().tela.doc.contas_da_validacao();
    assert_eq!(pedidas, 1);
    assert_eq!(descartadas, 0);
}

#[test]
fn salvar_com_erro_da_o_toast_com_a_linha() {
    let mut t = abrir("salvar_com_erro", "config.json", json_invalido_da_prancheta().as_bytes());
    digitar(&mut t, "config.json", " ");
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    // N1: salvar continua permitido, e o aviso diz que salvou com erro
    assert_eq!(toast_atual(&t.ctx).as_deref(), Some("config.json salvo com o erro da linha 14."));
    assert!(std::fs::read_to_string(&t.caminho).expect("ler").starts_with(" {"));
    // o JSON válido salva sem a ressalva
    let mut t = abrir("salvar_sem_erro", "config.json", JSON_DA_PRANCHETA.as_bytes());
    digitar(&mut t, "config.json", " ");
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    assert_eq!(toast_atual(&t.ctx).as_deref(), Some("config.json salvo."));
}

#[test]
fn txt_nao_tem_formatar_nem_estrutura() {
    let t = abrir("txt_sem_json", "leia-me.txt", b"{\"isto\": nao e json}\n");
    assert!(t.query_by_label("Formatar").is_none());
    assert!(t.query_by_label("JSON válido").is_none());
    assert!(t.query_by_label("Seção Estrutura do documento").is_none());
    assert!(t.query_by_label("Seção Texto").is_some());
    assert!(t.state().tela.doc.erro_de_json().is_none());
    assert!(t.query_by_label_contains("Espaços").is_none(), "o texto não tem o recuo no rodapé");
}

#[test]
fn leitor_mostra_o_estado_do_json_uma_vez_ao_abrir() {
    // um JSON válido de uns 1,2 MB: o leitor, com o círculo verde e a estrutura
    let mut json = String::from("{\"itens\": [\n");
    for i in 0..14_000 {
        json.push_str(&format!(
            "  {{\"id\": {i}, \"nome\": \"um texto comprido para encher o arquivo e passar do limite\"}},\n"
        ));
    }
    json.push_str("  null\n], \"fim\": true}\n");
    let t = abrir("leitor_json_valido", "grande.json", json.as_bytes());
    assert_eq!(t.state().tela.doc.modo(), Modo::Leitor);
    assert!(t.query_by_label("JSON válido").is_some());
    assert!(t.query_by_label_contains("Somente leitura").is_some());
    assert!(t.query_by_label("Formatar").is_none(), "o leitor tem a etiqueta no lugar do Formatar");
    assert!(t.query_by_label("raiz").is_some(), "a estrutura sai da análise feita ao abrir");
    assert_eq!(t.state().tela.doc.contas_da_validacao(), (0, 0), "nenhuma análise nova depois de abrir");
    // o mesmo arquivo com o erro: o círculo de erro, também feito uma vez
    let quebrado = json.replace("\"fim\": true", "\"fim\" true");
    let t = abrir("leitor_json_invalido", "quebrado.json", quebrado.as_bytes());
    assert_eq!(t.state().tela.doc.modo(), Modo::Leitor);
    let erro = t.state().tela.doc.erro_de_json().expect("o erro").to_string();
    assert!(
        erro.starts_with("JSON inválido na linha 14003, coluna 10: faltam os dois pontos depois de \"fim\""),
        "{erro}"
    );
    assert!(t.query_by_label(&erro).is_some());
    assert_eq!(t.state().tela.doc.contas_da_validacao(), (0, 0));
    // o clique no círculo de erro rola o leitor até a linha do erro (a 14003, bem abaixo da vista)
    let mut t = t;
    assert!(!t.state().tela.doc.linhas_desenhadas().contains(&14_002));
    t.get_by_label(&erro).click();
    t.run_steps(4);
    assert!(t.state().tela.doc.linhas_desenhadas().contains(&14_002), "{:?}", t.state().tela.doc.linhas_desenhadas());
    // o leitor também tem a faixa de erro, e o "Ir para o erro" rola até a linha
    let mut t = abrir("leitor_json_faixa", "quebrado.json", quebrado.as_bytes());
    assert!(t.query_by_label("Linha 14003, coluna 10: faltam os dois pontos depois de \"fim\".").is_some());
    assert!(!t.state().tela.doc.linhas_desenhadas().contains(&14_002));
    t.get_by_label(tela_texto::IR_PARA_O_ERRO).click();
    t.run_steps(4);
    assert!(t.state().tela.doc.linhas_desenhadas().contains(&14_002), "{:?}", t.state().tela.doc.linhas_desenhadas());
}

#[test]
fn capturas_do_json() {
    for (tema, nome) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let mut t = abrir_com(&format!("cap_json_{nome}"), "config.json", JSON_DA_PRANCHETA.as_bytes(), tema, TAMANHO);
        cursor_depois_de(&mut t, "\"tempo_limite\": 30");
        toast_concluido(&t.ctx, "config.json salvo.");
        assert!(capturar(&mut t, &format!("json-{nome}.png")).is_file());
    }
    // N1: o JSON válido que a pessoa estraga (tira a vírgula da linha 13); o clique no círculo leva ao erro
    for (tema, nome) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let mut t =
            abrir_com(&format!("cap_json_invalido_{nome}"), "config.json", JSON_DA_PRANCHETA.as_bytes(), tema, TAMANHO);
        cursor_depois_de(&mut t, "\"nome\": \"Relatórios\",");
        t.get_by_label("Texto de config.json").focus();
        t.run_steps(2);
        tecla(&mut t, egui::Modifiers::NONE, egui::Key::Backspace);
        esperar_ate(&mut t, |h| h.state().tela.doc.erro_de_json().is_some());
        t.get_by_label(ERRO_DA_PRANCHETA).click();
        t.run_steps(3);
        interface::pecas::toast::toast_atencao(&t.ctx, "config.json salvo com o erro da linha 14.");
        assert!(capturar(&mut t, &format!("json-invalido-{nome}.png")).is_file());
    }
}
