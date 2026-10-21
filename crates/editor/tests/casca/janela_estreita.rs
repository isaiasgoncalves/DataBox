//! As janelas estreitas (960, 990 e 1024px) e os recentes que cabem nelas.

use super::*;

/// "dd/mm/aaaa hh:mm" (o espaço do meio pode ser o sem quebra)
fn e_data(texto: &str) -> bool {
    let c: Vec<char> = texto.chars().collect();
    c.len() == 16 && c[2] == '/' && c[5] == '/' && c[10].is_whitespace() && c[13] == ':'
}

/// As ações dos recentes cabem no card, a data fica numa linha só e a pasta continua visível.
/// `pasta_minima`: a largura que a pasta (encurtada) ainda tem, no mínimo.
fn conferir_recentes(h: &H, direita: f32, pasta_minima: f32) {
    let botoes: Vec<egui::Rect> = acoes_dos_recentes(h, "Mostrar ", " na pasta")
        .into_iter()
        .chain(acoes_dos_recentes(h, "Remover ", " da lista"))
        .collect();
    assert!(!botoes.is_empty());
    for r in &botoes {
        assert!(r.right() <= direita + 0.5, "a ação {r:?} passa da borda do card ({direita})");
    }
    let datas: Vec<egui::Rect> =
        h.query_all_by(|n| n.label().or_else(|| n.value()).is_some_and(|l| e_data(&l))).map(|n| n.rect()).collect();
    assert!(!datas.is_empty(), "{:?}", textos(h));
    for d in &datas {
        assert!(d.height() < 25.0 && d.right() <= direita + 0.5, "a data numa linha só, dentro do card: {d:?}");
    }
    for r in &h.state().estado().prefs.recentes {
        let pasta = h.query_all_by_label(&r.pasta).next().map(|n| n.rect());
        assert!(pasta.is_some_and(|p| p.width() >= pasta_minima), "a pasta de {} aparece: {pasta:?}", r.nome);
    }
}

/// Põe estes bancos (criados agora) nos recentes, com o aberto no topo, e espera a tela mostrar `total`.
fn mais_recentes(h: &mut H, amb: &Ambiente, nomes: &[&str], total: usize) {
    for nome in nomes {
        let c = amb.pasta().join(nome);
        criar_banco(&c, "CREATE TABLE t (id INTEGER PRIMARY KEY);");
        amb.servico.prefs().adicionar_recente(&c);
    }
    amb.servico.prefs().adicionar_recente(&amb.estado.banco.as_ref().expect("banco").caminho);
    h.state_mut().emitir(estado::Acao::RecarregarPrefs);
    esperar(h, "os recentes", |h| h.state().estado().prefs.recentes.len() == total);
    assentar(h);
}

fn layout_em_janela_estreita(largura: f32, altura: f32, sufixo: &str) {
    let amb = Ambiente::exemplo(&format!("estreita_{sufixo}"));
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(largura, altura), Partida::default(), |_| None);
    assert_eq!(h.state().rota(), &Rota::Banco);

    // o corpo: 8 + a doca da esquerda (258) + 8 e o documento até 8px da borda; a da direita (as Propriedades, 272 +
    // 8), nem encolhidas as duas até 200, deixaria o documento com menos de 560px e some, com o botão desligado e a
    // dica de alargar
    let doc = documento(&h);
    assert_eq!((doc.left(), doc.right()), (8.0 + 258.0 + 8.0, largura - 8.0), "{doc:?}");
    assert!(!h.state().propriedades_a_vista());
    assert!(!botao_ligado(&h, "Painel de propriedades"));
    let dica = ler_dica(&mut h, Role::Button, "Painel de propriedades");
    assert!(dica.contains("Painel de propriedades (alargue a janela para ver)"), "{dica:?}");
    let direita = direita_do_card(&h);
    assert_eq!(direita, largura - 8.0 - 1.0 - 20.0 - 1.0);
    // a barra de ferramentas inteira cabe, com o Painel de propriedades encostado à direita
    let painel = rect_de(&h, Role::Button, "Painel de propriedades");
    assert!(painel.right() <= largura - 8.0 - 8.0 + 0.5, "{painel:?}");
    assert!(botao_da_barra(&h, "Exportar").rect().right() < painel.left());

    // Visão geral: a lista cabe na moldura, sem rolar na horizontal, e as seções das Propriedades vão para o fim do
    // documento, depois da lista, na largura dele
    for b in h.query_all_by_label("Colunas") {
        assert!(b.rect().right() <= direita + 0.5, "{:?}", b.rect());
    }
    let ultima = rect_de(&h, Role::Button, "Tabela com espaço");
    assert!(ultima.right() <= direita, "o nome inteiro à vista: {ultima:?}");
    let arquivo = h.get_by_label("Seção Arquivo").rect();
    let manutencao = h.get_by_label("Seção Manutenção").rect();
    assert!(ultima.bottom() < arquivo.top() && arquivo.top() < manutencao.top(), "{ultima:?} {arquivo:?}");
    assert!(arquivo.left() >= doc.left() && arquivo.right() <= doc.right(), "{arquivo:?}");
    assert_eq!(h.query_all_by_label("Verificar integridade").count(), 1);
    capturar(&mut h, &format!("banco-visao-geral-{sufixo}-claro.png"));
    // rolada até o fim: a Manutenção à vista
    h.key_press(Key::End);
    h.run_steps(4);
    capturar(&mut h, &format!("banco-visao-geral-{sufixo}-fim-claro.png"));
    h.key_press(Key::Home);
    h.run_steps(4);

    // Início: as ações dos recentes cabem no card e a data não quebra (só o exemplo.db nos recentes)
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    conferir_recentes(&h, direita, 30.0);
    capturar(&mut h, &format!("inicio-com-recentes-{sufixo}-claro.png"));

    // com nomes mais longos (o caso de "Remover da li" cortado): a pasta encurta até o próprio padding, e as
    // ações continuam dentro do card
    mais_recentes(&mut h, &amb, &["financeiro.sqlite", "estoque_2026.db"], 3);
    conferir_recentes(&h, direita, if largura < 1000.0 { 0.0 } else { 20.0 });
    capturar(&mut h, &format!("inicio-com-3-recentes-{sufixo}-claro.png"));

    // sem o Explorador, as Propriedades cabem ao lado da Visão geral (o documento passa dos 560px); de volta, somem
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(3);
    assert!(h.state().propriedades_a_vista());
    let doc = documento(&h);
    assert_eq!((doc.left(), doc.right()), (8.0, largura - 8.0 - 272.0 - 8.0), "{doc:?}");
    assert!(doc.width() >= 560.0);
    assert!(ligado(&h, Role::Button, "Painel de propriedades"));
    capturar(&mut h, &format!("casca-{sufixo}-propriedades-claro.png"));
    h.key_press_modifiers(CTRL, Key::B);
    h.run_steps(3);
    assert!(!h.state().propriedades_a_vista());
}

#[test]
fn layout_em_janela_estreita_960() {
    layout_em_janela_estreita(960.0, 600.0, "960");
}

#[test]
fn layout_em_janela_estreita_990() {
    layout_em_janela_estreita(990.0, 700.0, "990");
}

/// A 1024px (sem o trilho, desde a revisão de UI de 19/10/2026), as duas docas cabem encolhidas, na proporção do que
/// cada uma tem acima de 200, e o documento fica com pelo menos 560px.
#[test]
fn layout_em_janela_de_1024_encolhe_as_docas() {
    let amb = Ambiente::exemplo("estreita_1024");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1024.0, 700.0), Partida::default(), |_| None);
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert!(h.state().propriedades_a_vista());
    assert!(botao_ligado(&h, "Painel de propriedades"));
    let (esquerda, direita) = h.state().larguras_desenhadas().expect("as larguras");
    assert!(esquerda < 258.0 && direita < 272.0 && esquerda >= 200.0 && direita >= 200.0, "{esquerda} {direita}");
    let doc = documento(&h);
    assert_eq!(doc.left(), 8.0 + esquerda + 8.0, "{doc:?}");
    assert_eq!(doc.right(), 1024.0 - 8.0 - direita - 8.0, "{doc:?}");
    assert!(doc.width() >= 560.0, "{doc:?}");
    capturar(&mut h, "casca-1024-docas-claro.png");
}

#[test]
fn recentes_com_o_que_sumiu_cabem_em_1024() {
    let amb = Ambiente::exemplo("estreita_sumiu");
    let mut h = harness_com(&amb, Tema::Claro, egui::vec2(1024.0, 700.0), Partida::default(), |_| None);
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    // o que não existe mais leva a pill "não encontrado" à coluna Tamanho
    mais_recentes(&mut h, &amb, &["arquivo_antigo.db", "financeiro.sqlite", "estoque_2026.db"], 4);
    std::fs::remove_file(amb.pasta().join("arquivo_antigo.db")).expect("apagar o antigo");
    h.state_mut().emitir(estado::Acao::RecarregarPrefs);
    esperar(&mut h, "o antigo some", |h| h.state().estado().prefs.recentes.iter().any(|r| !r.existe));
    assentar(&mut h);
    // o Início não tem Propriedades: o documento vai até 8px da borda
    assert_eq!(documento(&h).right(), 1024.0 - 8.0);
    conferir_recentes(&h, direita_do_card(&h), 0.0);
    capturar(&mut h, "inicio-com-4-recentes-1024-claro.png");
}

/// A cópia de segurança com um caminho comprido: o resultado é o toast ("Cópia salva: x.db · 119 MB", sem a pasta),
/// com "Mostrar na pasta" nele e o caminho inteiro só na dica; nada vai para o painel. O clique na ação revela o
/// arquivo (aqui, o gancho dos testes no lugar do Explorador de Arquivos) e o toast sai.
#[test]
fn copia_de_seguranca_vira_toast_com_a_acao_e_o_caminho_na_dica() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let amb = Ambiente::exemplo("copia_longa");
    let pasta = amb.pasta().join("CopiasDeSegurancaDoFinanceiro2026").join("subpasta_com_nome_comprido");
    std::fs::create_dir_all(&pasta).expect("pasta");
    let destino = pasta.join("copia_de_seguranca_do_exemplo.db");
    let escolhido = destino.clone();
    let reveladas: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let anotadas = Rc::clone(&reveladas);
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(move |p| matches!(p, PedidoDialogo::CopiaDeSeguranca { .. }).then(|| escolhido.clone()))
        .com_abrir_de_teste(move |p| anotadas.borrow_mut().push(p.to_path_buf()));
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    clicar_botao(&mut h, "Fazer cópia de segurança");
    assentar(&mut h);
    assert!(destino.is_file());
    let texto = toast_atual(&h.ctx).expect("o toast da cópia");
    let nome = "copia_de_seguranca_do_exemplo.db";
    assert!(texto.starts_with(&format!("Cópia salva: {nome} · ")), "{texto}");
    assert!(!texto.contains("CopiasDeSeguranca"), "só o nome do arquivo no toast: {texto}");
    // (o serviço devolve o caminho por extenso, e o do teste pode estar na forma curta 8.3 da pasta temporária)
    let dica = interface::pecas::toast::dica_do_toast(&h.ctx).expect("a dica");
    let fim = ["CopiasDeSegurancaDoFinanceiro2026", "subpasta_com_nome_comprido", "copia_de_seguranca_do_exemplo.db"];
    assert!(
        std::path::Path::new(&dica).ends_with(fim.iter().collect::<PathBuf>()),
        "o caminho inteiro na dica: {dica}"
    );
    // nada de caixa no painel: nem o caminho, nem o botão dentro do painel de Propriedades
    assert!(!tem_texto(&h, "Cópia salva em"));
    assert!(!textos_das_propriedades(&h).iter().any(|t| t.contains("Cópia salva") || t == "Mostrar na pasta"));
    // o toast cabe na janela, centrado
    let r = interface::pecas::toast::rect_toast(&h.ctx).expect("onde ficou o toast");
    assert!(r.left() > 0.0 && r.right() < TAMANHO.x, "{r:?}");
    capturar(&mut h, "banco-copia-toast-claro.png");
    // o clique em "Mostrar na pasta" revela o arquivo e o toast sai
    h.get(by().role(Role::Button).label("Mostrar na pasta")).click();
    h.run_steps(3);
    let reveladas = reveladas.borrow();
    assert_eq!(reveladas.len(), 1);
    assert_eq!(reveladas[0].display().to_string(), dica, "o caminho da dica é o que se revela");
    assert!(toast_atual(&h.ctx).is_none(), "cumprida a ação, o toast sai");
}
