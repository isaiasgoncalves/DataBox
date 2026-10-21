//! Os ícones em PNG na casca (etapa 3, passo 8).

use super::*;

#[test]
fn docas_com_os_solidos_e_o_explorador_on() {
    use interface::icones::Icone;
    let amb = Ambiente::exemplo("docas_solidos");
    let mut h = harness(&amb);
    assentar(&mut h);
    assert!(painel_a_vista(&h, "Explorador"), "o Explorador à vista, a aba ativa da doca");
    assert!(!painel_a_vista(&h, "Histórico"));
    // a doca da esquerda (revisão de UI de 19/10/2026): as abas só ícone (`.aba.so-ic`, 38 × 34) com os sólidos de 16
    for (icone, rotulo) in [(Icone::Explorador, "Explorador"), (Icone::Historico, "Histórico")] {
        let aba = rect_de(&h, Role::Tab, rotulo);
        assert_eq!(aba.size(), egui::vec2(38.0, 34.0), "{rotulo}");
        let r = icone_pintado_em(&h, icone, 16.0, aba).unwrap_or_else(|| panic!("o sólido de 16 em {rotulo}"));
        assert!((r.center() - aba.center()).length() <= 0.75, "{rotulo}: {r:?} em {aba:?}");
    }
    assert!(aba_do_painel_ativa(&h, "Explorador") && !aba_do_painel_ativa(&h, "Histórico"));
    // a da direita, com o nome e o objeto
    assert!(aba_do_painel_ativa(&h, "Propriedades · exemplo.db"));
    // o Explorador pelo tipo do objeto: o banco, a tabela sem rowid (produtos) e a virtual (notas)
    let explorador = h.get(by().role(Role::Link).label("produtos")).rect();
    assert!(icone_pintado_em(&h, Icone::TabelaSemRowid, 16.0, explorador.expand2(egui::vec2(60.0, 0.0))).is_some());
    let notas = h.get(by().role(Role::Link).label("notas")).rect();
    assert!(icone_pintado_em(&h, Icone::TabelaVirtual, 16.0, notas.expand2(egui::vec2(60.0, 0.0))).is_some());
    assert!(interface::icones::textura_carregada(&h.ctx, Icone::Banco, 16.0).is_some());
    // a aba da tabela sem rowid leva a mesma ficha
    clicar_link(&mut h, "produtos");
    assentar(&mut h);
    let aba = h.query_all(by().role(Role::Tab)).find(|n| n.accesskit_node().label().is_some_and(|l| l == "produtos"));
    let aba = aba.expect("a aba de produtos").rect();
    assert!(icone_pintado_em(&h, Icone::TabelaSemRowid, 16.0, aba).is_some(), "a ficha anil na aba");
}

#[test]
fn menu_arquivo_com_as_silhuetas_e_os_recentes_com_os_solidos() {
    use interface::icones::Icone;
    let amb = Ambiente::exemplo("menu_icones");
    let estoque = amb.pasta().join("estoque_2026.db");
    criar_banco(&estoque, "CREATE TABLE itens (id INTEGER PRIMARY KEY);");
    amb.servico.prefs().adicionar_recente(&estoque);
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    // as silhuetas de 14 da barra de ferramentas, como nos menus Banco e Tabela (revisão de 20/10/2026), em linhas
    // da mesma altura; os sólidos de 24 de antes nem são carregados
    let altura = item_de_menu(&h, "Abrir pasta").rect().height();
    for rotulo in ["Abrir arquivo", "Novo banco", "Abertos recentemente", "Salvar", "Fechar aba", "Preferências"] {
        assert_eq!(item_de_menu(&h, rotulo).rect().height(), altura, "{rotulo}");
    }
    let mut x_da_silhueta = None;
    for (icone, rotulo) in
        [(Icone::Pasta, "Abrir pasta"), (Icone::Arquivo, "Abrir arquivo"), (Icone::NovoBanco, "Novo banco")]
    {
        let item = item_de_menu(&h, rotulo).rect();
        let r = icone_pintado_em(&h, icone, 14.0, item).unwrap_or_else(|| panic!("a silhueta de 14 em {rotulo}"));
        assert_eq!(r.width(), 14.0, "{rotulo}");
        let x = r.left() - item.left();
        assert_eq!(*x_da_silhueta.get_or_insert(x), x, "a mesma coluna em {rotulo}");
    }
    for solido in [Icone::PastaSolida, Icone::DocumentoSolido, Icone::NovoBancoSolido, Icone::Recentes] {
        assert!(interface::icones::textura_carregada(&h.ctx, solido, 24.0).is_none(), "{solido:?}");
    }
    // os recentes: o cilindro de 24 em linhas de 44
    abrir_submenu(&mut h, "Abertos recentemente");
    let recente = item_de_menu(&h, "estoque_2026.db").rect();
    assert_eq!(recente.height(), 44.0);
    let r = icone_pintado_em(&h, Icone::Banco, 24.0, recente).expect("o cilindro de 24 no recente");
    assert_eq!(r.left() - recente.left(), 8.0);
    capturar(&mut h, "menu-arquivo-silhuetas-claro.png");
}
