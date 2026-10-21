//! A busca da barra.

use super::*;

#[test]
fn ctrl_f_vai_para_a_busca_da_barra() {
    let amb = Ambiente::exemplo("ctrl_f_busca");
    let mut h = harness(&amb);
    // a Visão geral: "Buscar tabelas"
    h.key_press_modifiers(CTRL, Key::F);
    h.run_steps(2);
    assert!(campo_da_busca(&h, "Buscar tabelas").is_focused(), "o Ctrl+F leva o foco à busca da barra");
    h.key_press(Key::Escape);
    h.run_steps(2);
    // o Buscar do menu Editar faz o mesmo
    abrir_menu(&mut h, "Editar");
    clicar_item(&mut h, "Buscar");
    h.run_steps(2);
    assert!(campo_da_busca(&h, "Buscar tabelas").is_focused(), "o Buscar do menu");
    // no Início, sem busca, o Ctrl+F não faz nada e a busca fica desligada
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    h.key_press_modifiers(CTRL, Key::F);
    h.run_steps(2);
    let busca = campo_da_busca(&h, "Buscar");
    assert!(busca.accesskit_node().is_disabled() && !busca.is_focused());
    if cfg!(feature = "telas") {
        // nos Dados, "Buscar em clientes", também com a grade em foco (o texto fica selecionado)
        h.state_mut().ir(Rota::objeto("clientes", false));
        assentar(&mut h);
        h.key_press_modifiers(CTRL, Key::F);
        h.run_steps(2);
        assert!(campo_da_busca(&h, "Buscar em clientes").is_focused());
    }
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn busca_da_barra_filtra_os_dados_e_vai_para_o_endereco() {
    let amb = Ambiente::exemplo("busca_dados");
    let mut h = harness(&amb);
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    digitar(&mut h, "Buscar em clientes", "Recife");
    esperar(&mut h, "a busca no endereço", |h| h.state().rota().dados().is_some_and(|d| d.busca == "Recife"));
    assentar(&mut h);
    assert!(h.state().tabela_aberta().is_some_and(|t| t.rota.dados().is_some_and(|d| d.busca == "Recife")));
    // o Esc com texto limpa e aplica na hora
    campo_da_busca(&h, "Buscar em clientes").focus();
    h.run_steps(1);
    h.key_press(Key::Escape);
    esperar(&mut h, "a busca limpa", |h| h.state().rota().dados().is_some_and(|d| d.busca.is_empty()));
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some(""));
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn cada_aba_guarda_a_propria_busca() {
    let amb = Ambiente::exemplo("busca_por_aba");
    let mut h = harness(&amb);
    // a Visão geral filtra a lista de tabelas pelo nome, sem acento nem caixa
    digitar(&mut h, "Buscar tabelas", "PRODU");
    h.run_steps(2);
    assert!(h.query(by().role(Role::Button).label("produtos")).is_some());
    assert!(h.query(by().role(Role::Button).label("clientes")).is_none(), "{:?}", textos(&h));
    // os Dados têm a busca deles
    clicar_link(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some(""));
    digitar(&mut h, "Buscar em clientes", "Recife");
    esperar(&mut h, "a busca no endereço", |h| h.state().rota().dados().is_some_and(|d| d.busca == "Recife"));
    // de volta à Visão geral, a dela continua lá; e a dos Dados também
    clicar_aba(&mut h, "exemplo.db · Visão geral");
    assentar(&mut h);
    assert_eq!(campo_da_busca(&h, "Buscar tabelas").value().as_deref(), Some("PRODU"));
    clicar_aba(&mut h, "clientes");
    assentar(&mut h);
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some("Recife"));
    // a vista Colunas das Propriedades não troca a busca da barra: continua a dos Dados
    vista_da_tabela(&mut h, 2);
    assert_eq!(campo_da_busca(&h, "Buscar em clientes").value().as_deref(), Some("Recife"));
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da busca da aba Dados de verdade (tela_dados)")]
fn busca_desligada_no_inicio_e_ligada_na_tabela_em_qualquer_vista() {
    let amb = Ambiente::exemplo("busca_desligada");
    let mut h = harness(&amb);
    let ligada = |h: &mut H, rotulo: &str| {
        let desligada = campo_da_busca(h, rotulo).accesskit_node().is_disabled();
        abrir_menu(h, "Editar");
        let no_menu = item_ligado(h, "Buscar");
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert_eq!(!desligada, no_menu, "a barra e o menu dizem o mesmo");
        !desligada
    };
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    assert!(!ligada(&mut h, "Buscar"), "a busca desligada no Início");
    // a tabela é sempre a grade dos Dados (revisão de UI de 19/10/2026): a busca fica, em qualquer vista das
    // Propriedades
    for (rota, rotulo) in [
        (Rota::objeto_na_aba("v_pedidos_por_cliente", true, Aba::Definicao), "Buscar em v_pedidos_por_cliente"),
        (Rota::objeto_na_aba("clientes", false, Aba::Indices), "Buscar em clientes"),
    ] {
        h.state_mut().ir(rota.clone());
        assentar(&mut h);
        assert!(ligada(&mut h, rotulo), "a busca ligada em {rota:?}");
    }
}
