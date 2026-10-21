//! Os textos e a acessibilidade que cruzam as telas (etapa 3, passo 20).

use super::*;

/// Os botões da árvore de acessibilidade cujo nome termina em reticências (o "…" ou os três pontos): o nome de um
/// botão diz o que ele faz; a reticência de "Abrir arquivo…" prometia um diálogo que o clique já abre sem ela.
fn botoes_com_reticencias(h: &H) -> Vec<String> {
    h.query_all(by().role(Role::Button))
        .filter_map(|n| n.accesskit_node().label())
        .filter(|rotulo| {
            let rotulo = rotulo.trim_end();
            rotulo.ends_with('…') || rotulo.ends_with("...")
        })
        .collect()
}

/// Falha com o lugar e os botões, se algum botão à vista termina em reticências.
fn sem_reticencias(h: &H, onde: &str) {
    let achados = botoes_com_reticencias(h);
    assert!(achados.is_empty(), "botões com reticências {onde}: {achados:?}");
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa das telas de verdade")]
fn nenhum_botao_com_reticencias() {
    let amb = Ambiente::exemplo("sem_reticencias");
    let mut h = harness(&amb);
    // o Início, com banco aberto e sem
    h.state_mut().ir(Rota::Inicio);
    assentar(&mut h);
    sem_reticencias(&h, "no Início");
    for rotulo in ["Abrir arquivo", "Novo banco"] {
        assert!(existe(&h, Role::Button, rotulo), "{rotulo} no Início: {:?}", textos(&h));
    }
    // a Visão geral e as quatro vistas de uma tabela
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    sem_reticencias(&h, "na Visão geral");
    for (parte, nome) in [
        (Aba::Dados, "Dados"),
        (Aba::Estrutura, "Estrutura"),
        (Aba::Indices, "Índices e gatilhos"),
        (Aba::Definicao, "Definição SQL"),
    ] {
        h.state_mut().ir(Rota::objeto_na_aba("clientes", false, parte));
        assentar(&mut h);
        sem_reticencias(&h, &format!("na vista {nome}"));
    }
    // a Consulta SQL, com um resultado e o Histórico
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    executar(&mut h, "SELECT id, nome FROM clientes LIMIT 3");
    sem_reticencias(&h, "na Consulta SQL");
    // os menus
    for menu in ["Arquivo", "Editar", "Exibir", "Banco", "Ajuda"] {
        abrir_menu(&mut h, menu);
        sem_reticencias(&h, &format!("no menu {menu}"));
        h.key_press(Key::Escape);
        h.run_steps(3);
    }
    // as janelas: a barra (Exportar, Nova linha, Nova coluna), a Nova tabela, os Atalhos, o Sobre e as Preferências
    h.state_mut().ir(Rota::objeto_na_aba("clientes", false, Aba::Dados));
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Exportar");
    assentar(&mut h);
    assert!(h.state().janela_aberta(), "a janela Exportar");
    sem_reticencias(&h, "na janela Exportar");
    fechar_janela(&mut h);
    // o menu Tabela (com a tabela à vista) e a Nova coluna dele: um diálogo da Estrutura, nas Propriedades (não conta
    // em `janela_aberta`)
    abrir_menu(&mut h, "Tabela");
    sem_reticencias(&h, "no menu Tabela");
    clicar_item(&mut h, "Nova coluna");
    esperar(&mut h, "o diálogo Nova coluna", |h| existe(h, Role::Dialog, "Nova coluna"));
    sem_reticencias(&h, "na janela Nova coluna");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!existe(&h, Role::Dialog, "Nova coluna"));
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova tabela");
    assentar(&mut h);
    assert!(h.state().janela_aberta(), "a janela Nova tabela");
    sem_reticencias(&h, "na janela Nova tabela");
    fechar_janela(&mut h);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    assert!(h.state().janela_aberta());
    sem_reticencias(&h, "na janela Atalhos do teclado");
    fechar_janela(&mut h);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Sobre o DataBox");
    assert!(h.state().janela_aberta());
    sem_reticencias(&h, "na janela Sobre");
    clicar_botao(&mut h, "Licenças de terceiros");
    sem_reticencias(&h, "na janela Licenças de terceiros");
    h.key_press(Key::Escape);
    h.run_steps(3);
    if h.state().janela_aberta() {
        fechar_janela(&mut h);
    }
    abrir_preferencias(&mut h);
    assert!(preferencias_abertas(&h));
    sem_reticencias(&h, "nas Preferências");
    fechar_janela(&mut h);
}

#[test]
#[cfg_attr(not(feature = "telas"), ignore = "precisa da Consulta SQL de verdade (tela_consulta)")]
fn aviso_de_transacao_cita_a_consulta() {
    let amb = Ambiente::exemplo("aviso_cita_consulta");
    let mut h = harness(&amb);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    clicar_primeiro(&mut h, Role::Button, "Nova consulta");
    assentar(&mut h);
    // a transação abre na Consulta 2 (a que está à vista)
    executar(&mut h, "BEGIN; UPDATE clientes SET cidade = 'Teresina' WHERE id = 1;");
    esperar(&mut h, "a transação aberta", |h| h.state().estado().transacao_aberta());
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta 2."), "{:?}", textos(&h));
    assert!(!tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    assert!(existe(&h, Role::Link, "Ir para a Consulta 2"));
    assert!(!existe(&h, Role::Link, "Ir para a Consulta SQL"));
    clicar_link(&mut h, "Ir para a Consulta 2");
    h.run_steps(2);
    assert_eq!(rotulos_das_abas(&h).1, "Consulta 2", "o link foca a Consulta da transação");
}

#[test]
fn aviso_de_transacao_sem_consulta_diz_consulta_sql() {
    let amb = Ambiente::exemplo("aviso_sem_consulta");
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    assert!(tem_texto(&h, "Há uma transação aberta na Consulta SQL."));
    assert!(existe(&h, Role::Link, "Ir para a Consulta SQL"));
}

#[test]
fn dica_do_status_igual_ao_aviso() {
    let amb = Ambiente::exemplo("dica_igual_ao_aviso");
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::objeto("clientes", false));
    assentar(&mut h);
    // o aviso: o começo em destaque e o resto; a dica do modo é as duas partes numa frase só
    let frase = frase_da_transacao("Consulta SQL");
    let (inicio, resto) = frase.split_once(". ").expect("duas partes");
    assert!(tem_texto(&h, &format!("{inicio}.")) && tem_texto(&h, resto), "{:?}", textos(&h));
    assert!(ler_dica(&mut h, Role::Label, "Transação aberta").contains(&frase));
}

#[test]
fn explorador_sem_ponto_verde_e_com_atencao_na_transacao() {
    let amb = Ambiente::exemplo("explorador_atencao");
    let id = amb.estado.banco_id();
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    // aberto sem transação: nenhum ponto nem ícone de estado no nó do banco (o "Aberto" saiu)
    assert!(!existe(&h, Role::Image, "Transação aberta"));
    assert!(!dica_do_estado_do_banco(&mut h).contains("Aberto"));
    // com a transação aberta: o ícone Atenção de 16px, com o rótulo "Transação aberta" e, na dica, o aviso inteiro
    amb.servico.sql(&id, "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(estado::Acao::NotarTransacao(true));
    assentar(&mut h);
    let no = rect_de(&h, Role::Link, "exemplo.db");
    assert!(no.contains_rect(rect_de(&h, Role::Image, "Transação aberta")), "o ícone fica dentro do nó do banco");
    let atencao = interface::icones::Icone::Estado(interface::icones::EstadoIcone::Atencao);
    let pintado = icone_pintado_em(&h, atencao, 16.0, no).expect("o ícone Atenção pintado no nó do banco");
    assert_eq!((pintado.width(), pintado.height()), (16.0, 16.0));
    assert_eq!(dica_do_estado_do_banco(&mut h), frase_da_transacao("Consulta SQL"));
    // o COMMIT tira o ícone
    amb.servico.sql(&id, "COMMIT;", 10).expect("commit");
    h.state_mut().emitir(estado::Acao::NotarTransacao(false));
    assentar(&mut h);
    assert!(!existe(&h, Role::Image, "Transação aberta"));
}

#[test]
fn botao_marcado_diz_que_esta_pressionado() {
    use egui::accesskit::Toggled;
    let amb = Ambiente::exemplo("botao_pressionado");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    let estado_de = |h: &H, rotulo: &str| h.get(by().role(Role::Button).label(rotulo)).accesskit_node().toggled();
    // o botão Painel de propriedades da barra de ferramentas (`.ib.on`): pressionado com as Propriedades à vista
    assert_eq!(estado_de(&h, "Painel de propriedades"), Some(Toggled::True));
    clicar_botao(&mut h, "Painel de propriedades");
    assert_eq!(estado_de(&h, "Painel de propriedades"), Some(Toggled::False), "escondido: não pressionado");
    clicar_botao(&mut h, "Painel de propriedades");
    assert_eq!(estado_de(&h, "Painel de propriedades"), Some(Toggled::True));
}

#[test]
fn abas_das_docas_dizem_qual_esta_a_vista() {
    let amb = Ambiente::exemplo("docas_a_vista");
    let mut h = harness(&amb);
    h.state_mut().ir(Rota::Banco);
    assentar(&mut h);
    // a Visão geral à vista, com o Explorador: a aba dele é a escolhida; a do Histórico não
    assert!(aba_do_painel_ativa(&h, "Explorador") && !aba_do_painel_ativa(&h, "Histórico"));
    // o clique na aba do Histórico o põe à vista na mesma doca
    clicar_aba_do_painel(&mut h, "Histórico");
    assentar(&mut h);
    assert!(aba_do_painel_ativa(&h, "Histórico") && !aba_do_painel_ativa(&h, "Explorador"));
    assert!(painel_a_vista(&h, "Histórico") && !painel_a_vista(&h, "Explorador"));
    // pelo Exibir, de volta ao Explorador
    exibir(&mut h, "Explorador");
    assert!(aba_do_painel_ativa(&h, "Explorador"));
}

#[test]
fn barra_de_status_escondida_pelas_preferencias_e_mostrada_pelo_exibir() {
    let amb = Ambiente::exemplo("casca_sem_status");
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_barra_status(amb.servico.prefs().barra_status());
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    // escondida por padrão (revisão de 20/10/2026): o modo de gravação fica nas Propriedades, com a frase na dica
    assert!(!h.state().barra_status_a_vista());
    assert!(!existe(&h, Role::Label, "Gravação imediata"), "sem a barra");
    assert!(textos_das_propriedades(&h).iter().any(|t| t == "imediata"), "{:?}", textos_das_propriedades(&h));
    let dica = ler_dica(&mut h, Role::Label, "imediata");
    assert!(dica.contains("Tudo o que for salvo grava no arquivo na hora."), "{dica:?}");
    // Exibir › Barra de status: aparece, e a escolha fica nas preferências
    abrir_menu(&mut h, "Exibir");
    clicar_item(&mut h, "Barra de status");
    assert!(h.state().barra_status_a_vista());
    assert!(existe(&h, Role::Label, "Gravação imediata"));
    esperar(&mut h, "a preferência gravar", |_| amb.servico.prefs().barra_status());
    // e some de novo
    abrir_menu(&mut h, "Exibir");
    clicar_item(&mut h, "Barra de status");
    assert!(!existe(&h, Role::Label, "Gravação imediata"));
    esperar(&mut h, "a preferência gravar", |_| !amb.servico.prefs().barra_status());
}
