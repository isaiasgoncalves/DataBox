//! A Visão geral: a faixa, os cartões e as tabelas, a só leitura, a manutenção nas Propriedades, a busca, o
//! caminho copiado e as seções no fim do documento sem o painel.

use super::*;

/// Os contadores da dock do exemplo: as visões e os gatilhos no singular (o exemplo tem um de cada).
const DOCK_DO_EXEMPLO: [&str; 4] = ["7 Tabelas", "1 Visão", "1 Gatilho", "Tamanho"];

#[test]
fn visao_geral_faixa_cartoes_e_tabelas() {
    let amb = Ambiente::exemplo("visao_geral");
    let copia = amb.pasta().join("copias").join("exemplo (cópia).db");
    std::fs::create_dir_all(copia.parent().expect("pasta")).expect("pasta");
    let destino = copia.clone();
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, Partida::default(), move |p| match p {
        PedidoDialogo::CopiaDeSeguranca { nome_sugerido } => {
            assert_eq!(nome_sugerido, "exemplo (cópia).db");
            Some(destino.clone())
        }
        _ => None,
    });
    assert_eq!(h.state().rota(), &Rota::Banco);
    // a faixa: o nome e o caminho inteiro (no leitor de tela), logo abaixo da faixa de abas
    let aberto = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string()).unwrap_or_default();
    let caminho = h.query_all_by_label(&aberto).map(|n| n.rect()).min_by(|a, b| a.top().total_cmp(&b.top()));
    let doc = documento(&h);
    assert!(caminho.is_some_and(|r| r.top() > doc.top() && r.bottom() < doc.top() + 42.0), "na faixa: {caminho:?}");
    // os contadores na dock, no singular com um, e a lista de Tabelas (sem o "7 tabelas" do título)
    for c in DOCK_DO_EXEMPLO {
        assert!(tem_texto(&h, c), "{c} na dock: {:?}", textos(&h));
    }
    assert!(tem_texto_exato(&h, "Tabelas") && !tem_texto(&h, "7 tabelas · clique"));
    for t in TABELAS {
        assert!(h.query(by().role(Role::Button).label(t)).is_some(), "{t} na tabela da visão geral");
    }
    // o botão de cada linha abre a vista Colunas da tabela (revisão de UI de 19/10/2026)
    assert_eq!(h.query_all(by().role(Role::Button).label("Colunas")).count(), 7);
    assert!(tem_texto(&h, "virtual") && tem_texto(&h, "sem rowid"));
    // o cabeçalho de antes saiu: a Nova tabela e o Exportar são os da barra
    assert!(h.query(by().role(Role::Button).label("Exportar o banco")).is_none());
    assert!(h.query(by().role(Role::Button).label("+ Nova tabela")).is_none());
    assert!(botao_ligado(&h, "Nova tabela") && botao_ligado(&h, "Exportar"));
    assert!(tem_texto(&h, "verificadas") || tem_texto(&h, "desligadas"));
    capturar(&mut h, "banco-visao-geral-claro.png");

    clicar_botao(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert!(tem_texto(&h, "O banco passou na verificação de integridade."), "{:?}", textos(&h));
    capturar(&mut h, "banco-integridade-claro.png");

    // VACUUM pergunta antes
    clicar_botao(&mut h, "Compactar (VACUUM)");
    esperar_texto(&mut h, "Compactar reescreve o arquivo inteiro");
    capturar(&mut h, "confirmar-vacuum-claro.png");
    clicar_botao(&mut h, "Compactar");
    assentar(&mut h);
    assert!(tem_texto(&h, "De "), "o \"de X para Y\" do VACUUM");
    capturar(&mut h, "banco-vacuum-claro.png");

    // cópia de segurança pelo diálogo (falso)
    clicar_botao(&mut h, "Fazer cópia de segurança");
    assentar(&mut h);
    esperar_texto(&mut h, "Cópia salva: ");
    assert!(copia.is_file());
    // o resultado é o toast: o nome do arquivo e o tamanho, o "Mostrar na pasta" no próprio toast e o caminho inteiro
    // só na dica (nada de caixa no painel)
    let nome = copia.file_name().and_then(|n| n.to_str()).expect("nome").to_string();
    let texto = toast_atual(&h.ctx).expect("o toast da cópia");
    assert!(
        texto.starts_with(&format!("Cópia salva: {nome} · "))
            && !texto.contains(copia.parent().unwrap().to_str().unwrap()),
        "{texto}"
    );
    assert_eq!(interface::pecas::toast::rotulo_da_acao_do_toast(&h.ctx).as_deref(), Some("Mostrar na pasta"));
    let dica = interface::pecas::toast::dica_do_toast(&h.ctx).expect("a dica");
    assert!(
        std::path::Path::new(&dica).ends_with(std::path::Path::new("copias").join(nome)),
        "o caminho inteiro na dica: {dica}"
    );
    assert!(h.query(by().role(Role::Button).label("Mostrar na pasta")).is_some());
    assert!(!tem_texto(&h, "Cópia salva em"), "sem a caixa de antes");
    capturar(&mut h, "banco-copia-claro.png");

    // o nome abre a tabela; "Colunas" abre a tabela com a vista Colunas nas Propriedades
    h.get(by().role(Role::Button).label("pedidos")).click();
    h.run_steps(2);
    assert_eq!(h.state().rota(), &Rota::objeto("pedidos", false));
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    h.query_all(by().role(Role::Button).label("Colunas")).next().expect("Colunas").click();
    h.run_steps(2);
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::objeto("clientes", false));
    assert_eq!(h.state().vista_da_tabela(), Some("Colunas"));
}

#[test]
fn visao_geral_escuro() {
    let amb = Ambiente::exemplo("visao_geral_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, TAMANHO, Partida::default(), |_| None);
    assert!(tem_texto(&h, "7 Tabelas"));
    capturar(&mut h, "banco-visao-geral-escuro.png");
    // a 960, com o Explorador: as Propriedades vão para o fim do documento
    let amb = Ambiente::exemplo("visao_geral_960_escuro");
    let mut h = harness_com(&amb, Tema::Escuro, egui::vec2(960.0, 600.0), Partida::default(), |_| None);
    assert!(!h.state().propriedades_a_vista());
    capturar(&mut h, "banco-visao-geral-960-escuro.png");
}

/// "1 Visão", "1 Gatilho": o rótulo do contador no singular com um, no plural com os outros (item 26).
#[test]
fn cartoes_no_singular_com_um() {
    let amb = Ambiente::exemplo("singular");
    let mut h = harness(&amb);
    for (um, varios) in [("1 Visão", "1 Visões"), ("1 Gatilho", "1 Gatilhos"), ("7 Tabelas", "7 Tabela ")] {
        assert!(tem_texto_exato(&h, um) && !tem_texto(&h, varios), "{um}: {:?}", textos(&h));
    }
    // uma visão a mais: "2 Visões"
    amb.servico.sql(&amb.estado.banco_id(), "CREATE VIEW v_dois AS SELECT 1;", 10).expect("criar a visão");
    h.state_mut().emitir(estado::Acao::RecarregarObjetos);
    h.state_mut().emitir(estado::Acao::RecarregarBanco);
    esperar(&mut h, "a visão nova", |h| tem_texto_exato(h, "2 Visões"));
}

#[test]
fn visao_geral_so_leitura_so_tem_verificacoes_e_copia() {
    let amb = Ambiente::vazio("so_leitura");
    let caminho = amostras::criar_banco_exemplo(amb.pasta().join("exemplo.db")).expect("exemplo");
    let partida = Partida { caminho: Some(caminho), somente_leitura: true };
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, partida, |_| None);
    // a pílula "só leitura" na faixa, logo depois do nome
    assert!(tem_texto(&h, "só leitura"));
    // (e outra na linha "Gravação" das Propriedades, revisão de 20/10/2026)
    let doc = documento(&h);
    let pilulas: Vec<egui::Rect> = h.query_all(by().label("só leitura")).map(|n| n.rect()).collect();
    assert_eq!(pilulas.len(), 2, "{pilulas:?}");
    let pilula = pilulas.iter().find(|r| doc.contains_rect(**r)).copied().expect("a pílula da faixa");
    assert!(pilula.top() > doc.top() && pilula.bottom() < doc.top() + 42.0, "{pilula:?}");
    // a Manutenção nas Propriedades: só as verificações e a cópia
    let t = textos_das_propriedades(&h);
    for item in
        ["Seção Manutenção", "Verificar integridade", "Verificar chaves estrangeiras", "Fazer cópia de segurança"]
    {
        assert!(t.iter().any(|x| x == item), "{item} nas Propriedades: {t:?}");
    }
    assert!(h.query(by().role(Role::Button).label("Otimizar")).is_none());
    assert!(h.query(by().role(Role::Button).label("Compactar (VACUUM)")).is_none());
    assert!(h.query(by().role(Role::Button).label("+ Nova tabela")).is_none());
    assert!(h.query(by().role(Role::Link).label("+ Nova tabela")).is_none());
    let dica = ler_dica(&mut h, Role::Label, "Só leitura");
    assert!(dica.contains("exemplo.db está aberto só para leitura. Nada é gravado no arquivo."), "{dica:?}");
    capturar(&mut h, "banco-visao-geral-so-leitura-claro.png");

    // no menu Editar, a Nova tabela fica desligada, como na barra
    abrir_menu(&mut h, "Editar");
    let desligadas: Vec<bool> =
        h.query_all(by().role(Role::Button).label("Nova tabela")).map(|n| n.accesskit_node().is_disabled()).collect();
    assert_eq!(desligadas, [true, true], "o item do menu e o botão da barra");
}

/// A Manutenção nas Propriedades: os cinco `Botao::lista` da largura do painel, na ordem da prancheta, sem
/// reticência.
#[test]
fn manutencao_nas_propriedades() {
    let amb = Ambiente::exemplo("manutencao_propriedades");
    let mut h = harness(&amb);
    assert!(h.state().propriedades_a_vista());
    let t = textos_das_propriedades(&h);
    let ordem = [
        "Seção Arquivo",
        "Seção Manutenção",
        "Verificar integridade",
        "Verificar chaves estrangeiras",
        "Otimizar",
        "Compactar (VACUUM)",
        "Fazer cópia de segurança",
    ];
    let posicoes: Vec<usize> = ordem
        .iter()
        .map(|item| t.iter().position(|x| x == item).unwrap_or_else(|| panic!("{item} nas Propriedades: {t:?}")))
        .collect();
    assert!(posicoes.windows(2).all(|p| p[0] < p[1]), "na ordem: {t:?}");
    let doc = documento(&h);
    for rotulo in &ordem[2..] {
        let r = rect_de(&h, Role::Button, rotulo);
        // o painel tem 272px; o botão, a largura toda menos o `padding:0 8px` e a borda
        assert!(r.left() > doc.right() && (r.width() - (272.0 - 2.0 - 16.0)).abs() < 1.0, "{rotulo}: {r:?}");
    }
    assert!(!tem_texto(&h, "Fazer cópia de segurança…"));
    capturar(&mut h, "propriedades-visao-geral-manutencao-claro.png");
}

/// "Buscar tabelas" filtra a lista pelo nome, sem acento nem caixa; sem nenhuma, a frase do vazio.
#[test]
fn buscar_tabelas_filtra_a_lista() {
    let amb = Ambiente::exemplo("buscar_tabelas");
    let mut h = harness(&amb);
    let na_lista = |h: &H| -> Vec<&str> {
        TABELAS.iter().copied().filter(|t| h.query(by().role(Role::Button).label(t)).is_some()).collect()
    };
    assert_eq!(na_lista(&h), TABELAS);
    digitar(&mut h, "Buscar tabelas", "PEDIDO");
    assert_eq!(na_lista(&h), ["itens_pedido", "pedidos"]);
    // sem acento: "espaco" acha "Tabela com espaço"
    campo_da_busca(&h, "Buscar tabelas").focus();
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    campo_da_busca(&h, "Buscar tabelas").type_text("espaco");
    h.run_steps(2);
    assert_eq!(na_lista(&h), ["Tabela com espaço"]);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    campo_da_busca(&h, "Buscar tabelas").type_text("zzz");
    h.run_steps(2);
    assert!(na_lista(&h).is_empty());
    assert!(tem_texto(&h, "Nenhuma tabela corresponde à busca."), "{:?}", textos(&h));
    // o Esc limpa e a lista volta
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert_eq!(na_lista(&h), TABELAS);
}

/// O Copiar caminho do Arquivo copia o caminho inteiro e dá o toast "Caminho copiado.".
#[test]
fn copiar_caminho_copia_e_avisa() {
    let amb = Ambiente::exemplo("copiar_caminho");
    let mut h = harness(&amb);
    let caminho = h.state().estado().banco.as_ref().map(|b| b.caminho.display().to_string()).unwrap_or_default();
    // o Caminho numa linha, com o botão à direita dele, nas Propriedades
    let botao = rect_de(&h, Role::Button, "Copiar caminho");
    assert!(botao.left() > documento(&h).right(), "{botao:?}");
    h.get(by().role(Role::Button).label("Copiar caminho")).click();
    let mut copiado = None;
    for _ in 0..3 {
        h.step();
        copiado = copiado.or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
        });
    }
    assert_eq!(copiado.as_deref(), Some(caminho.as_str()));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Caminho copiado."));
}

/// Decisão 4: com o painel escondido (Ctrl+Alt+B), as seções das Propriedades vão para o fim do documento, depois da
/// lista e do `.tit` "Propriedades"; de volta, ficam só no painel.
#[test]
fn sem_o_painel_as_secoes_vao_para_o_fim_do_documento() {
    let amb = Ambiente::exemplo("secoes_no_fim");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1440.0, 1300.0), Partida::default(), |_| None);
    let painel = Modifiers { alt: true, ..CTRL };
    h.key_press_modifiers(painel, Key::B);
    h.run_steps(3);
    assert!(!h.state().propriedades_a_vista());
    let doc = documento(&h);
    assert_eq!(doc.right(), 1440.0 - 8.0, "o documento fica com a largura");
    let ultima = rect_de(&h, Role::Button, "Tabela com espaço");
    let titulo = h.get(by().role(Role::Label).label("Propriedades")).rect();
    let arquivo = h.get_by_label("Seção Arquivo").rect();
    let manutencao = h.get_by_label("Seção Manutenção").rect();
    assert!(ultima.bottom() < titulo.top() && titulo.bottom() <= arquivo.top(), "{ultima:?} {titulo:?} {arquivo:?}");
    assert!(arquivo.top() < manutencao.top());
    for r in [arquivo, manutencao, rect_de(&h, Role::Button, "Verificar integridade")] {
        assert!(r.left() >= doc.left() && r.right() <= doc.right(), "dentro do documento: {r:?}");
    }
    // as ações funcionam no fim também
    clicar_botao(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert!(tem_texto(&h, "O banco passou na verificação de integridade."), "{:?}", textos(&h));
    capturar(&mut h, "banco-visao-geral-sem-painel-claro.png");
    // de volta ao painel: nada no fim
    h.key_press_modifiers(painel, Key::B);
    h.run_steps(3);
    assert!(h.state().propriedades_a_vista());
    assert!(h.get_by_label("Seção Arquivo").rect().left() > documento(&h).right());
    assert!(
        h.query(by().role(Role::Label).label("Propriedades")).is_none_or(|n| n.rect().left() > documento(&h).right())
    );
}

/// Decisão 3: o Atualizar pede à casca o BancoInfo e os objetos, sem refazer a Visão geral (a busca e o resultado da
/// manutenção ficam).
#[test]
fn atualizar_rele_banco_e_objetos_pela_casca() {
    let amb = Ambiente::exemplo("atualizar_visao_geral");
    let mut h = harness(&amb);
    clicar_botao(&mut h, "Verificar integridade");
    assentar(&mut h);
    assert!(tem_texto(&h, "O banco passou na verificação de integridade."));
    digitar(&mut h, "Buscar tabelas", "a");
    // uma tabela nova por fora (sem a casca saber)
    amb.servico.sql(&amb.estado.banco_id(), "CREATE TABLE zebra (a);", 10).expect("criar");
    assert!(tem_texto(&h, "7 Tabelas") && h.query(by().role(Role::Button).label("zebra")).is_none());
    h.remove_cursor();
    clicar_primeiro(&mut h, Role::Button, "Atualizar");
    esperar(&mut h, "a lista nova", |h| h.query(by().role(Role::Button).label("zebra")).is_some());
    assert!(tem_texto(&h, "8 Tabelas"), "a dock relida: {:?}", textos(&h));
    assert!(existe(&h, Role::Link, "zebra"), "o Explorador recebe a mesma lista");
    assert_eq!(campo_da_busca(&h, "Buscar tabelas").value().as_deref(), Some("a"), "a busca fica");
}
