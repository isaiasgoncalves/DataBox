//! A tela da planilha abrindo e lendo (etapa 6, passo 5): a primeira página na hora, o índice em segundo plano, as duas
//! `Planilha`, o cache de três, os pedidos de página e os estados de erro.

mod apoio;

use apoio::*;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use interface::tema::Tema;
use tela_planilha::{ABRIR_NO_PADRAO, EM_MEMORIA, MSG_SEM_PLANILHA_A_VISTA, MSG_SEM_PLANILHAS, Modo, PedidoDeArquivo};

/// Uma planilha de `n` linhas, com a `<dimension>` declarada (a grade já nasce com o tamanho dela).
fn grande(n: u32) -> Vec<u8> {
    let x = Extras { dimensao: Some(format!("A1:B{n}")), ..Extras::default() };
    Montador::novo().aba_com("Lançamentos", &linhas_simples(n), &x).aba("Contas", &linhas_simples(10)).bytes()
}

/// A mesma, sem `<dimension>`: o tamanho vem do que se vê e do índice.
fn grande_sem_dimensao(n: u32) -> Vec<u8> {
    Montador::novo().aba("Lançamentos", &linhas_simples(n)).aba("Contas", &linhas_simples(10)).bytes()
}

#[test]
fn abre_o_xlsx_e_mostra_a_primeira_planilha_visivel() {
    let bytes = Montador::novo()
        .aba("Escondida", &linhas_simples(3))
        .estado("hidden")
        .aba("Resumo", &linhas_simples(3))
        .aba("Vendas", &linha_xml(1, &[t("Região"), t("Produto")], false))
        .ativa(2)
        .bytes();
    let t = abrir("pl_abre", "relatorio_q3.xlsx", &bytes);
    let tela = tela(&t);
    assert_eq!(tela.modo(), Modo::Pronta);
    // a aba ativa do arquivo, que está à vista
    assert_eq!((tela.folha_ativa(), tela.nome_da_folha()), (Some(2), Some("Vendas")));
    let s = tela.sessao().expect("a planilha da tela");
    let l1 = s.linha(1).expect("a linha 1 já chegou");
    assert_eq!(l1.celula(0).map(|c| c.valor.clone()), Some(planilha::Valor::Texto("Região".into())));
    // a faixa tem só as visíveis, com a ativa marcada
    let nomes: Vec<String> = tela.folhas_na_faixa().into_iter().map(|f| f.nome).collect();
    assert_eq!(nomes, ["Resumo", "Vendas"]);
    let vendas = t.get_by_role_and_label(Role::Tab, "Vendas, planilha 2 de 2");
    assert_eq!(vendas.accesskit_node().is_selected(), Some(true));
    // a faixa de contexto: o nome, a planilha e o "Somente leitura"
    assert!(t.query_by_label("relatorio_q3.xlsx").is_some());
    assert!(t.query_by_label("Somente leitura").is_some());
}

#[test]
fn primeira_pagina_antes_do_indice() {
    let mut t =
        abrir_com("pl_primeira", "grande.xlsx", &grande(60_000), Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    t.run_steps(3);
    let s = tela(&t).sessao().expect("a planilha");
    // a grade já tem as 128 primeiras linhas, e o índice nem começou
    assert!(s.linha(1).is_some() && s.linha(128).is_some());
    assert!(!s.indexada() && !s.indexando());
    assert_eq!(s.blocos_guardados()[0], 0);
}

#[test]
fn indice_em_segundo_plano_com_progresso() {
    let mut t = abrir_com("pl_indice", "grande.xlsx", &grande(60_000), Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    let egui = t.ctx.clone();
    tela_mut(&mut t).iniciar_indice(&egui);
    assert!(tela(&t).sessao().expect("a planilha").indexando());
    // o progresso chega pelo `Arc` (a cada 8.192 linhas) e o índice termina sozinho
    let mut viu_progresso = false;
    esperar_ate(&mut t, |h| {
        let s = tela(h).sessao().expect("a planilha");
        viu_progresso |= s.linhas_do_progresso() > 0 || s.indexada();
        s.indexada()
    });
    assert!(viu_progresso);
    let r = tela(&t).sessao().and_then(|s| s.resumo).expect("o resumo do índice");
    assert_eq!((r.ultima_linha, r.colunas, r.linhas_com_dados), (60_000, 2, 60_000));
}

#[test]
fn indice_termina_e_troca_a_leitura() {
    let mut t = abrir("pl_troca", "grande.xlsx", &grande(20_000));
    esperar_o_indice(&mut t);
    // a `Planilha` que indexou (com os marcos) é a que serve as páginas agora
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.leitor_indexado() && !s.pedindo()));
    // e ir longe lê pelo marco: o bloco da linha 19.000 chega
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((18_999, 1)));
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.linha(19_000).is_some()));
    let l = tela(&t).sessao().and_then(|s| s.linha(19_000)).cloned().expect("a linha");
    assert_eq!(l.celula(0).map(|c| c.valor.clone()), Some(planilha::Valor::Numero(19_000.0)));
}

#[test]
fn planilha_pequena_nao_mostra_a_barra() {
    let mut t = abrir("pl_pequena", "pequena.xlsx", &Montador::novo().aba("A", &linhas_simples(50)).bytes());
    for _ in 0..30 {
        t.step();
        assert!(t.query_by_label_contains("Lendo a planilha").is_none(), "a barra não pisca na planilha pequena");
        assert!(tela(&t).sessao().is_some_and(|s| s.fracao_da_barra().is_none()));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    esperar_o_indice(&mut t);
    assert!(t.query_by_label_contains("Lendo a planilha").is_none());
}

#[test]
fn cancelar_o_indice_deixa_a_planilha_navegavel() {
    let mut t =
        abrir_com("pl_cancelar", "grande.xlsx", &grande(60_000), Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    let egui = t.ctx.clone();
    // o índice começa e é cancelado antes do primeiro marco de 8.192 linhas: a tarefa para nele
    tela_mut(&mut t).iniciar_indice(&egui);
    tela_mut(&mut t).cancelar_indice(true);
    t.run_steps(3);
    let s = tela(&t).sessao().expect("a planilha");
    assert!(s.indice_cancelado && !s.indexando() && !s.indexada());
    // a planilha continua navegável: ir longe lê o bloco (do começo, sem os marcos)

    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((40_000, 1)));
    t.run_steps(2);
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.linha(40_001).is_some()));
    assert!(!tela(&t).sessao().expect("a planilha").indexando(), "o índice cancelado não volta sozinho");
}

#[test]
fn trocar_de_planilha_cancela_o_indice_da_anterior() {
    let mut t = abrir_com("pl_trocar", "grande.xlsx", &grande(60_000), Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    let egui = t.ctx.clone();
    tela_mut(&mut t).iniciar_indice(&egui);
    assert!(tela(&t).sessao().is_some_and(|s| s.indexando()));
    tela_mut(&mut t).escolher_folha(&egui, 1);
    t.run_steps(2);
    assert_eq!(tela(&t).nome_da_folha(), Some("Contas"));
    // a que saiu foi para o cache, sem índice rodando
    assert_eq!(tela(&t).folhas_no_cache(), [0]);
    esperar_a_abertura(&mut t);
    assert_eq!(tela(&t).sessao().map(|s| s.folha), Some(1));
}

#[test]
fn voltar_a_planilha_indexada_e_instantaneo() {
    let bytes = Montador::novo().aba("A", &linhas_simples(300)).aba("B", &linhas_simples(20)).bytes();
    let mut t = abrir("pl_voltar", "duas.xlsx", &bytes);
    esperar_o_indice(&mut t);
    let egui = t.ctx.clone();
    tela_mut(&mut t).escolher_folha(&egui, 1);
    esperar_a_abertura(&mut t);
    tela_mut(&mut t).escolher_folha(&egui, 0);
    // no mesmo quadro: a planilha voltou pronta e indexada, sem tarefa nenhuma
    let s = tela(&t).sessao().expect("a planilha");
    assert_eq!(s.folha, 0);
    assert!(s.pronta() && s.indexada() && s.linha(1).is_some());
    assert!(!s.abrindo());
}

#[test]
fn cache_de_tres_planilhas() {
    let mut m = Montador::novo();
    for nome in ["A", "B", "C", "D"] {
        m = m.aba(nome, &linhas_simples(5));
    }
    let mut t = abrir("pl_cache", "quatro.xlsx", &m.bytes());
    let egui = t.ctx.clone();
    for folha in [1, 2, 3] {
        tela_mut(&mut t).escolher_folha(&egui, folha);
        esperar_a_abertura(&mut t);
    }
    // a da tela e as duas últimas que saíram: 3 abertas, a primeira saiu do cache
    assert_eq!(EM_MEMORIA, 3);
    assert_eq!(tela(&t).folha_ativa(), Some(3));
    assert_eq!(tela(&t).folhas_no_cache(), [2, 1]);
}

#[test]
fn voltar_a_mais_antiga_com_o_cache_cheio_nao_le_de_novo() {
    let mut m = Montador::novo();
    for nome in ["A", "B", "C"] {
        m = m.aba(nome, &linhas_simples(300));
    }
    let mut t = abrir("pl_cache_volta", "tres.xlsx", &m.bytes());
    esperar_o_indice(&mut t);
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| !s.pedindo()));
    let pedidos_da_a = tela(&t).sessao().map(|s| s.pedidos);
    let egui = t.ctx.clone();
    for folha in [1, 2] {
        tela_mut(&mut t).escolher_folha(&egui, folha);
        esperar_a_abertura(&mut t);
        esperar_o_indice(&mut t);
        esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| !s.pedindo()));
    }
    // A → B → C: o cache está cheio, com a A como a mais antiga
    assert_eq!(tela(&t).folhas_no_cache(), [1, 0]);
    tela_mut(&mut t).escolher_folha(&egui, 0);
    // no mesmo quadro: a A voltou do cache (não foi ela que saiu para a C entrar), pronta e indexada
    let s = tela(&t).sessao().expect("a planilha");
    assert_eq!(s.folha, 0);
    assert!(s.pronta() && s.indexada() && !s.abrindo() && !s.indexando());
    assert_eq!(tela(&t).folhas_no_cache(), [2, 1], "quem sai do cache é a B, a mais antiga depois da A");
    t.run_steps(3);
    let s = tela(&t).sessao().expect("a planilha");
    assert_eq!(Some(s.pedidos), pedidos_da_a, "nenhuma página pedida de novo");
    assert!(!s.indexando() && !s.pedindo());
}

#[test]
fn leitura_que_falhou_nao_e_pedida_a_cada_quadro() {
    // 200 linhas e o XML cortado no meio da 201: o primeiro bloco abre, o segundo (129–256) falha
    let xml = format!(
        "<?xml version=\"1.0\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
         <sheetData>{}<row r=\"201\"><c r=\"A201\"><v>2",
        linhas_simples(200)
    );
    let bytes = Montador::novo().aba_xml("Cortada", &xml).bytes();
    let mut t = abrir_com("pl_cortada", "cortada.xlsx", &bytes, Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.erro.is_some()));
    let s = tela(&t).sessao().expect("a planilha");
    assert!(s.pronta() && s.linha(1).is_some(), "o primeiro bloco chegou");
    for _ in 0..50 {
        t.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let s = tela(&t).sessao().expect("a planilha");
    assert!(s.pedidos <= 2, "a leitura que falhou não é pedida de novo a cada quadro: {} pedidos", s.pedidos);
    assert!(!s.pedindo());
    // o erro continua na grade
    assert!(t.query_by_label_contains("Não foi possível ler a planilha Cortada").is_some());
    // e a tela parada não pede quadros
    let mut causas = Vec::new();
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        t.step();
        if t.ctx.has_requested_repaint() {
            causas.extend(t.ctx.repaint_causes().iter().map(|c| format!("{}:{} {}", c.file, c.line, c.reason)));
        }
    }
    assert!(causas.is_empty(), "{causas:?}");
}

#[test]
fn cancelar_ou_trocar_nao_encolhe_a_rolagem() {
    // sem `<dimension>`: antes do índice, o tamanho vem do que se viu e do que o índice já percorreu
    let bytes =
        Montador::novo().aba("Lançamentos", &linhas_simples(200_000)).aba("Contas", &linhas_simples(10)).bytes();
    let mut t = abrir_com("pl_nao_encolhe", "grande.xlsx", &bytes, Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    let egui = t.ctx.clone();
    let linhas = |t: &Teste| tela(t).sessao().map_or(0, |s| s.tamanho().0);
    let na_grade = |t: &Teste| tela(t).grade().map_or(0, |g| g.nucleo.n_linhas() as u32);
    tela_mut(&mut t).iniciar_indice(&egui);
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.linhas_do_progresso() > 128));
    assert!(!tela(&t).sessao().expect("a planilha").indexada(), "o índice ainda corre");
    let antes = linhas(&t);
    assert!(antes > 128, "{antes}");
    // o Cancelar: a planilha fica navegável com o que já foi visto
    tela_mut(&mut t).cancelar_indice(true);
    t.run_steps(3);
    assert!(linhas(&t) >= antes, "{} < {antes}", linhas(&t));
    assert_eq!(na_grade(&t), linhas(&t));
    // trocar de planilha no meio de outro índice: a que volta do cache tem o tamanho de quando saiu
    tela_mut(&mut t).iniciar_indice(&egui);
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.linhas_do_progresso() > 128));
    let durante = linhas(&t);
    tela_mut(&mut t).escolher_folha(&egui, 1);
    esperar_a_abertura(&mut t);
    tela_mut(&mut t).escolher_folha(&egui, 0);
    t.run_steps(2);
    assert_eq!(tela(&t).folha_ativa(), Some(0));
    assert!(!tela(&t).sessao().expect("a planilha").indexada());
    assert!(linhas(&t) >= durante.max(antes), "{} < {durante}", linhas(&t));
    assert_eq!(na_grade(&t), linhas(&t));
}

#[test]
fn pedido_velho_e_descartado() {
    let mut t = abrir_com("pl_velho", "grande.xlsx", &grande(80_000), Tema::Claro, TAMANHO, |t| t.com_indice_manual());
    // sem o índice, ir à linha 70.000 lê do começo (demora): enquanto isso, a vista volta para o topo

    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((70_000, 1)));
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.pedindo()));
    tela_mut(&mut t).grade_mut().expect("a grade").nucleo.selecionar(Some((0, 1)));
    t.run_steps(2);
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| !s.pedindo()));
    let s = tela(&t).sessao().expect("a planilha");
    assert!(s.descartados >= 1, "a página da linha 70.000 chegou com a vista no topo e foi descartada");
    assert!(s.linha(70_001).is_none());
    assert!(s.linha(1).is_some());
}

#[test]
fn rolagem_cresce_com_o_progresso() {
    let mut t = abrir("pl_rolagem", "grande.xlsx", &grande_sem_dimensao(60_000));
    // sem `<dimension>`, a grade começa com o que já viu (o primeiro bloco) e termina com a planilha inteira
    let mut maior_antes = 0;
    esperar_ate(&mut t, |h| {
        let s = tela(h).sessao().expect("a planilha");
        let linhas = tela(h).grade().map_or(0, |g| g.nucleo.n_linhas());
        if !s.indexada() {
            maior_antes = maior_antes.max(linhas);
            assert!(linhas >= 128, "{linhas}");
            assert_eq!(linhas as u32, s.tamanho().0, "a grade acompanha o tamanho");
        }
        s.indexada()
    });
    t.run_steps(2);
    assert_eq!(tela(&t).grade().map(|g| g.nucleo.n_linhas()), Some(60_000));
    assert!(maior_antes <= 60_000);
}

#[test]
fn senha_mostra_a_frase() {
    let t = abrir("pl_senha", "folha_pagamento.xlsx", &ole_com(&["Root Entry", "EncryptionInfo", "EncryptedPackage"]));
    assert_eq!(tela(&t).modo(), Modo::Falhou);
    // a frase do núcleo, que diz como tirar a senha
    let frase = tela(&t).falha().expect("a frase");
    assert!(frase.starts_with("A planilha está protegida por senha"), "{frase}");
    assert!(t.query_by_label_contains("protegida por senha").is_some());
    assert!(!t.get_by_label(ABRIR_NO_PADRAO).accesskit_node().is_disabled());
}

#[test]
fn formato_antigo_mostra_a_frase_e_o_botao() {
    let mut t = abrir("pl_xls", "orcamento_2009.xlsx", &ole_com(&["Root Entry", "Workbook"]));
    let frase = tela(&t).falha().expect("a frase");
    assert!(frase.contains("formato antigo do Excel"), "{frase}");
    t.get_by_label(ABRIR_NO_PADRAO).click();
    t.run_steps(2);
    let caminho = t.caminho.clone();
    assert_eq!(tela_mut(&mut t).tomar_pedido(), Some(PedidoDeArquivo::AbrirNoPadrao(caminho)));
    assert!(t.query_by_label("Mostrar na pasta").is_some());
}

#[test]
fn arquivo_estragado_mostra_o_motivo() {
    // o pacote diz onde está a pasta de trabalho, mas a parte não está lá
    let partes = vec![
        (
            "[Content_Types].xml".to_string(),
            "<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>"
                .to_string(),
        ),
        (
            "_rels/.rels".to_string(),
            "<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
                .to_string(),
        ),
    ];
    let t = abrir("pl_estragado", "importacao_out.xlsx", &zip_de(&partes));
    let frase = tela(&t).falha().expect("a frase");
    assert!(frase.starts_with("O arquivo não é uma planilha válida: "), "{frase}");
    assert!(t.query_by_label_contains("não é uma planilha válida").is_some());
}

#[test]
fn arquivo_que_sumiu_desliga_os_botoes() {
    let amb = estado::apoio_testes::Ambiente::vazio("pl_sumiu");
    let caminho = inexistente(&amb, "sumiu.xlsx");
    let t = abrir_caminho(amb, caminho, Tema::Claro, TAMANHO, |t| t);
    let frase = tela(&t).falha().expect("a frase");
    assert!(frase.starts_with("O arquivo não existe (ou foi movido): "), "{frase}");
    assert!(t.get_by_label(ABRIR_NO_PADRAO).accesskit_node().is_disabled());
}

#[test]
fn sem_planilha_a_vista_abre_sem_grade() {
    let bytes = Montador::novo()
        .aba("Cotações", &linhas_simples(3))
        .estado("hidden")
        .aba("Taxas", &linhas_simples(3))
        .estado("veryHidden")
        .bytes();
    let mut t = abrir("pl_ocultas", "cotacoes.xlsx", &bytes);
    assert_eq!(tela(&t).modo(), Modo::Pronta);
    assert!(tela(&t).sessao().is_none(), "sem grade");
    assert!(t.query_by_label(MSG_SEM_PLANILHA_A_VISTA).is_some());
    assert!(t.query_by_role(Role::Tab).is_none(), "a faixa vazia");
    assert!(t.query_by_label("3 planilhas · todas ocultas").is_none());
    assert!(t.query_by_label("2 planilhas · todas ocultas").is_some());
    // a caixa das Propriedades mostra as ocultas (a muito oculta também) e abre a primeira
    t.get_by_label("Mostrar planilhas ocultas").click();
    t.run_steps(2);
    esperar_a_abertura(&mut t);
    assert_eq!(tela(&t).nome_da_folha(), Some("Cotações"));
    assert!(t.query_by_role_and_label(Role::Tab, "Taxas, planilha 2 de 2, muito oculta").is_some());
    let _ = MSG_SEM_PLANILHAS;
}

#[test]
fn erro_de_uma_planilha_nao_derruba_as_outras() {
    let bytes = Montador::novo()
        .aba("Boa", &linhas_simples(5))
        .aba_xml("Estragada", "<?xml version=\"1.0\"?><worksheet><sheetData><row r=\"1\"><c r=\"A1\"><v>1</v>")
        .bytes();
    let mut t = abrir("pl_uma_estragada", "duas.xlsx", &bytes);
    let egui = t.ctx.clone();
    tela_mut(&mut t).escolher_folha(&egui, 1);
    esperar_ate(&mut t, |h| tela(h).sessao().is_some_and(|s| s.erro.is_some()));
    t.run_steps(2);
    assert!(t.query_by_label_contains("Não foi possível ler a planilha Estragada").is_some());
    // a outra continua
    tela_mut(&mut t).escolher_folha(&egui, 0);
    t.run_steps(2);
    let s = tela(&t).sessao().expect("a planilha");
    assert!(s.erro.is_none() && s.linha(5).is_some());
}

/// Um arquivo de um milhão de linhas abre na hora (a primeira página) e o índice termina em segundo plano. Lento no
/// build de desenvolvimento: `cargo test -p tela_planilha --release -- --ignored`.
#[test]
#[ignore]
fn um_arquivo_de_um_milhao_de_linhas_abre_sem_travar() {
    let inicio = std::time::Instant::now();
    let bytes = grande(1_000_000);
    eprintln!("montado em {:?}", inicio.elapsed());
    let inicio = std::time::Instant::now();
    let mut t = abrir("pl_milhao", "milhao.xlsx", &bytes);
    eprintln!("aberto em {:?}", inicio.elapsed());
    assert!(inicio.elapsed() < std::time::Duration::from_secs(3), "a primeira página na hora");
    esperar_o_indice(&mut t);
    assert_eq!(tela(&t).sessao().and_then(|s| s.resumo).map(|r| r.ultima_linha), Some(1_000_000));
}
