//! Passo 10 da etapa 5: o CSV aberto na grade (prancheta 7).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use apoio::*;
use egui::Modifiers;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Acao, Documento, Situacao};
use interface::tema::Tema;
use tabular::numero::TipoDetectado;
use tela_csv::Ganchos;

#[test]
fn abre_o_csv_da_prancheta_7_e_mostra_a_grade() {
    let amb = Ambiente::vazio("csv_abre");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(1204));
    let h = abrir(&amb, caminho);
    let r = doc(&h).retrato().expect("o retrato");
    assert_eq!(r.linhas, 1204);
    assert_eq!(r.nomes, ["data", "regiao", "vendedor", "produto", "quantidade", "valor"]);
    let p = doc(&h).pagina().expect("a página");
    assert_eq!(
        p.linha(0).expect("a primeira"),
        ["2025-01-02", "Sul", "Ana Coutinho", "Caderno universitário", "1", "12,90"]
    );
    assert_eq!(p.posicao(0), Some(0));
    assert!(h.query_by_label("Grade").is_some());
    let rodape = h.query_by_label_contains("de 1.204").expect("o rodapé").accesskit_node().value().unwrap_or_default();
    assert!(rodape.starts_with("Linhas 1–"), "o rodapé dos Dados: {rodape}");
    // sem alteração: nem o ponto de alterado nem a contagem no rodapé
    assert!(!doc(&h).alterado());
    assert!(h.query_by_label_contains("não salva").is_none());
}

/// A revisão de UI de 19/10/2026 (prancheta 7): a faixa com o nome e o formato em pílulas, a barra dos filtros (a
/// dos Dados) e o rodapé "Linhas a–b de n" com as linhas à vista.
#[test]
fn faixa_com_o_formato_barra_de_filtros_e_rodape_como_os_dados() {
    let amb = Ambiente::vazio("csv_faixa");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)));
    assert!(doc(&h).anatomia().faixa, "a faixa sempre, sem filtro");
    assert!(h.query_by_label("vendas_2025.csv").is_some(), "o nome na faixa");
    for pilula in ["Ponto e vírgula", "UTF-8", "CRLF"] {
        assert!(h.query_by_label(pilula).is_some(), "a pílula {pilula}");
    }
    assert!(h.query_by_label("Filtros").is_some(), "a barra dos filtros");
    assert!(h.query_by_label("Nova condição").is_some());
    assert!(h.query_by_label("Limpar filtros").is_none(), "sem condição, sem o Limpar");
    // a coluna de marcar: a caixa do cabeçalho e a de cada linha
    assert!(h.query(by().role(Role::CheckBox).label("Marcar todas as linhas")).is_some());
    let rodape = |h: &H| h.query_by_label_contains("de 1.204").map(|n| n.accesskit_node().value().unwrap_or_default());
    let antes = rodape(&h).expect("o rodapé");
    assert!(antes.starts_with("Linhas 1–"), "{antes}");
    // rolar até a linha 1101: o intervalo segue a grade
    doc_mut(&mut h).selecionar(Some((1100, 0)));
    esperar_ociosa(&mut h);
    let depois = rodape(&h).expect("o rodapé");
    let (a, b) = depois
        .trim_start_matches("Linhas ")
        .trim_end_matches(" de 1.204")
        .split_once('–')
        .map(|(a, b)| (a.replace('.', "").parse::<u64>().unwrap(), b.replace('.', "").parse::<u64>().unwrap()))
        .expect("Linhas a–b");
    assert!((2..=1101).contains(&a) && (1101..=1204).contains(&b), "{depois}");
    // filtrando, o total é o da vista
    doc_mut(&mut h).filtrar("regiao", tela_csv::OperadorCsv::Igual, "Sul");
    esperar_ociosa(&mut h);
    assert!(h.query_by_label("241 de 1.204 linhas").is_some(), "a contagem na barra dos filtros");
    assert!(h.query_by_label_contains("de 241").is_some(), "o rodapé conta a vista");
}

#[test]
fn cabecalho_tem_pastilha_de_tipo_pelos_valores() {
    let amb = Ambiente::vazio("csv_pastilhas");
    let h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(40)));
    let tipos = &doc(&h).retrato().expect("o retrato").tipos;
    use TipoDetectado::*;
    assert_eq!(tipos, &[Data, Texto, Texto, Texto, Inteiro, Decimal]);
    // a dica da pastilha (o `title` da prancheta)
    assert!(h.query_by_label("número inteiro (detectado)").is_some());
    assert!(h.query_by_label("número decimal (detectado)").is_some());
    assert!(h.query_by_label("data (detectado)").is_some());
    assert!(h.query_all_by_label("texto").count() >= 3);
}

#[test]
fn numeros_a_direita_e_celula_vazia_em_branco() {
    let amb = Ambiente::vazio("csv_vazias");
    let h = abrir(&amb, arquivo(&amb, "t.csv", "nome;qtd\nAna;3\n;\nBia;12\n"));
    let p = doc(&h).pagina().expect("a página");
    assert_eq!(p.celula(1, 0), Some(""));
    assert_eq!(doc(&h).retrato().expect("retrato").tipos[1], TipoDetectado::Inteiro);
    // o CSV não tem NULL: a célula vazia não mostra "vazio"
    assert!(h.query_by_label("vazio").is_none());
}

#[test]
fn carregando_mostra_a_barra_so_depois_de_400_ms() {
    let amb = Ambiente::vazio("csv_carregando");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(50));
    let solto = Arc::new(AtomicBool::new(false));
    let s2 = Arc::clone(&solto);
    let ganchos = Ganchos {
        abrir: Some(Arc::new(move || {
            let inicio = Instant::now();
            while !s2.load(Ordering::SeqCst) && inicio.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(5));
            }
        })),
        ..Ganchos::default()
    };
    let mut h = palco(&amb, caminho, Tema::Claro, TAMANHO, ganchos);
    let desde = doc(&h).abrindo_desde().expect("a leitura começou no primeiro quadro");
    // o relógio do egui na mão: 0,3 s depois de começar, ainda nada; 0,5 s depois, o card
    h.input_mut().time = Some(desde + 0.3);
    h.run_steps(2);
    assert!(doc(&h).carregando());
    assert!(h.query_by_label_contains("Lendo vendas_2025.csv").is_none(), "antes dos 400 ms, nada pisca");
    h.input_mut().time = Some(desde + 0.5);
    h.run_steps(2);
    assert!(h.query_by_label_contains("Lendo vendas_2025.csv").is_some(), "depois dos 400 ms, o card");
    assert!(h.query_by_label("Cancelar").is_some());
    h.input_mut().time = None;
    solto.store(true, Ordering::SeqCst);
    esperar_ociosa(&mut h);
    assert!(doc(&h).aberta());
    assert!(h.query_by_label_contains("Lendo vendas_2025.csv").is_none());
}

#[test]
fn cancelar_o_carregamento_fecha_a_aba() {
    let amb = Ambiente::vazio("csv_cancelar");
    let caminho = arquivo(&amb, "vendas_2025.csv", vendas(50));
    let solto = Arc::new(AtomicBool::new(false));
    let s2 = Arc::clone(&solto);
    let ganchos = Ganchos {
        abrir: Some(Arc::new(move || {
            while !s2.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        })),
        ..Ganchos::default()
    };
    let mut h = palco(&amb, caminho, Tema::Claro, TAMANHO, ganchos);
    h.run_steps(4);
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    solto.store(true, Ordering::SeqCst);
    assert!(h.state().acoes.contains(&Acao::FecharAba(None)), "{:?}", h.state().acoes);
    esperar_ociosa(&mut h);
    assert!(!doc(&h).aberta(), "o cancelamento chega ao núcleo e nada abre");
}

#[test]
fn utf16_e_arquivo_sumido_mostram_a_ficha() {
    let amb = Ambiente::vazio("csv_falhas");
    let mut bytes = vec![0xFF, 0xFE];
    for u in "a;b\r\n1;2\r\n".encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    let mut h = palco(&amb, arquivo(&amb, "u16.csv", bytes), Tema::Claro, TAMANHO, Ganchos::default());
    esperar_ociosa(&mut h);
    let erro = doc(&h).erro_ao_abrir().expect("UTF-16 não abre").to_string();
    assert!(erro.contains("UTF-16"), "{erro}");
    assert!(h.query_by_label_contains("UTF-16").is_some(), "a frase no documento");
    drop(h);
    let mut h = palco(&amb, amb.pasta().join("sumiu.csv"), Tema::Claro, TAMANHO, Ganchos::default());
    esperar_ociosa(&mut h);
    let erro = doc(&h).erro_ao_abrir().expect("o arquivo sumido não abre").to_string();
    assert!(h.query_by_label_contains(&erro).is_some(), "{erro}");
    assert!(doc(&h).ofertas(&h.state().estado).iter().all(|(_, o)| !o.ligado), "nada a oferecer");
}

#[test]
fn propriedades_formato_arquivo_e_coluna() {
    let amb = Ambiente::vazio("csv_propriedades");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(30)));
    assert!(h.query_by_label("Seção Formato").is_some());
    assert!(h.query_by_label("Seção Arquivo").is_some());
    for rotulo in ["Separador", "Decimal", "Codificação", "Fim de linha"] {
        assert!(h.query(by().role(Role::ComboBox).label(rotulo)).is_some(), "o seletor {rotulo}");
    }
    let f = &doc(&h).retrato().unwrap().formato;
    assert_eq!(
        (f.separador, f.decimal, f.codificacao.rotulo(), f.fim_de_linha.rotulo()),
        (';', ',', "UTF-8", "CRLF (Windows)")
    );
    assert!(h.query_by_label("Primeira linha é o cabeçalho").is_some());
    // sem célula ativa, não há a seção Coluna
    assert!(h.query_by_label_contains("Seção Coluna").is_none());
    clicar_celula(&mut h, 2, 1);
    esperar_ociosa(&mut h);
    assert!(h.query_by_label_contains("Seção Coluna").is_some());
    // as três pastilhas de texto do cabeçalho e o tipo detectado da coluna regiao
    assert_eq!(h.query_all_by_label("texto").count(), 4);
    assert_eq!(doc(&h).celula_ativa(), Some((2, 1)));
}

#[test]
fn estatistica_da_coluna_numerica() {
    let amb = Ambiente::vazio("csv_estatistica");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(18)));
    clicar_celula(&mut h, 6, 4);
    esperar_ociosa(&mut h);
    // quantidade: ((i * 7) % 23) + 1 nas 18 linhas
    let q: Vec<u64> = (0..18).map(|i| ((i * 7) % 23) + 1).collect();
    let soma: u64 = q.iter().sum();
    let (min, max) = (q.iter().min().unwrap(), q.iter().max().unwrap());
    assert!(h.query_by_label(&interface::formato::formatar_numero(soma)).is_some(), "a soma {soma}");
    assert!(h.query_by_label(&min.to_string()).is_some() && h.query_by_label(&max.to_string()).is_some());
    assert!(h.query_by_label("número inteiro").is_some(), "o tipo detectado");
}

#[test]
fn trocar_de_coluna_mostra_as_contas_da_nova() {
    let amb = Ambiente::vazio("csv_estatistica_troca");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(18)));
    clicar_celula(&mut h, 6, 4);
    // sem esperar as contas da quantidade (a conta dela, se ainda corre, é cancelada), a coluna regiao
    clicar_celula(&mut h, 2, 1);
    esperar_ociosa(&mut h);
    assert!(h.query_by_label("12 caracteres").is_some(), "o maior texto da regiao (Centro-Oeste)");
    assert!(h.query_by_label("Calculando…").is_none(), "a conta terminou");
}

#[test]
fn trocar_o_separador_reabre_o_arquivo() {
    let amb = Ambiente::vazio("csv_separador");
    let mut h = abrir(&amb, arquivo(&amb, "t.csv", "a;b,c\n1;2,3\n4;5,6\n"));
    assert_eq!(doc(&h).retrato().unwrap().nomes, ["a", "b,c"]);
    h.get(by().role(Role::ComboBox).label("Separador")).click();
    h.run_steps(2);
    // a opção da lista aberta (o seletor do Decimal também diz "Vírgula": a lista vem por último)
    h.query_all_by_label("Vírgula").last().expect("a opção Vírgula").click();
    esperar_ociosa(&mut h);
    let r = doc(&h).retrato().unwrap();
    assert_eq!(r.formato.separador, ',');
    assert_eq!(r.nomes, ["a;b", "c"]);
}

#[test]
fn status_de_arquivo_do_csv() {
    let amb = Ambiente::vazio("csv_status");
    let h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(5)));
    let s = doc(&h).status_do_arquivo(&h.state().estado).expect("o status do arquivo");
    assert_eq!(s.nome, "vendas_2025.csv");
    assert_eq!(s.situacao, Situacao::Salvo);
    assert_eq!(s.detalhes_em_linha(), "CSV · UTF-8 · CRLF · Separador ;");
}

#[test]
fn rolar_pede_a_pagina_que_falta() {
    let amb = Ambiente::vazio("csv_rolar");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)));
    assert!(!doc(&h).pagina().unwrap().contem(1100));
    doc_mut(&mut h).selecionar(Some((1100, 0)));
    esperar_ociosa(&mut h);
    let p = doc(&h).pagina().unwrap();
    assert!(p.contem(1100), "a página da linha 1101 chegou ({}..{})", p.inicio, p.inicio + p.linhas.len() as u64);
    assert_eq!(p.posicao(1100), Some(1100));
    assert!(doc(&h).centro_celula(1100, 0).is_some(), "a grade rolou até ela");
}

#[test]
fn a_pagina_de_um_milhao_de_linhas_nao_espera_o_mutex() {
    let amb = Ambiente::vazio("csv_grande");
    let mut h = abrir(&amb, arquivo(&amb, "vendas_2025.csv", vendas(120_000)));
    let tabela = doc(&h).tabela_compartilhada().expect("a tabela");
    // outra thread segura a tabela (uma gravação longa): a grade continua desenhando do retrato
    let trava = tabela.lock().expect("travar");
    doc_mut(&mut h).selecionar(Some((90_000, 2)));
    let inicio = Instant::now();
    h.run_steps(6);
    assert!(inicio.elapsed() < Duration::from_secs(5), "os quadros não esperam o Mutex");
    assert!(doc(&h).ocupada(), "a página nova espera a tabela");
    drop(trava);
    esperar_ociosa(&mut h);
    assert!(doc(&h).pagina().unwrap().contem(90_000));
}

#[test]
fn capturas_do_csv_contra_a_prancheta_7() {
    let amb = Ambiente::vazio("csv_capturas");
    for (tema, nome) in [(Tema::Escuro, "csv-escuro.png"), (Tema::Claro, "csv-claro.png")] {
        let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)), tema, TAMANHO);
        clicar_celula(&mut h, 6, 4);
        esperar_ociosa(&mut h);
        capturar(&mut h, nome);
    }
    let mut h = abrir_com(&amb, arquivo(&amb, "vendas_2025.csv", vendas(1204)), Tema::Escuro, egui::vec2(960.0, 700.0));
    let pos = doc(&h).centro_celula(2, 1).unwrap();
    clicar_em(&mut h, pos, Modifiers::NONE);
    esperar_ociosa(&mut h);
    capturar(&mut h, "csv-960-escuro.png");
}
