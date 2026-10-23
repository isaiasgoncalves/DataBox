//! Capturas da aba Dados em `capturas/rust/`, no palco que desenha como a casca (a barra com os verbos e a busca,
//! o documento com a faixa, a grade e o rodapé, e as Propriedades): a grade de clientes (claro e escuro), a edição
//! no lugar, a busca com filtro e linhas marcadas, produtos (sem rowid, BLOB, gerada), a janela da linha (clientes,
//! nova, produtos com BLOB) e a confirmação de excluir; a grade vazia, a de uma visão, a página que não pôde ser
//! lida, a janela de uma linha sem chave e a com erro do serviço; a faixa com as etiquetas, o popover da condição
//! (a parada 2, sem prancheta: claro e escuro) e a janela de 960px. O mouse fica fora da tela (nada em hover).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use apoio::*;
use banco::Operador;
use egui::accesskit::Role;
use egui::pos2;
use egui_kittest::kittest::Queryable;
use estado::apoio_testes::Ambiente;
use estado::{ConsultaDados, FiltroDigitado};
use interface::tema::Tema;
use tela_dados::Ganchos;

fn clicar_rotulo(h: &mut H, rotulo: &str) {
    let c = h.get_by_role_and_label(Role::Button, rotulo).rect().center();
    clicar(h, pos2(c.x, c.y));
}

#[test]
fn grade_de_clientes_claro_e_escuro() {
    let amb = Ambiente::exemplo("captura_clientes");
    for (tema, nome) in [(Tema::Claro, "dados-clientes-claro.png"), (Tema::Escuro, "dados-clientes-escuro.png")] {
        let mut h = abrir_com(&amb, amb.tabela("clientes"), tema, Ganchos::default());
        afastar_mouse(&mut h);
        capturar(&mut h, nome);
    }
}

#[test]
fn edicao_no_lugar() {
    let amb = Ambiente::exemplo("captura_edicao");
    let mut h = abrir(&amb, "clientes");
    duplo_clique_celula(&mut h, 0, 1);
    afastar_mouse(&mut h);
    assert!(retrato(&h).editando.is_some());
    capturar(&mut h, "dados-edicao-celula-claro.png");
}

#[test]
fn busca_filtro_e_marcadas() {
    let amb = Ambiente::exemplo("captura_busca");
    let mut tabela = amb.tabela("clientes");
    let filtro = FiltroDigitado { coluna: "limite".into(), op: Operador::Maior, valor: "1000".into() };
    tabela.rota =
        tabela.rota.com_dados(ConsultaDados { busca: "ana".into(), filtros: vec![filtro], ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    assert_eq!(retrato(&h).total, "2 de 60 linhas");
    clicar_marca(&mut h, None);
    afastar_mouse(&mut h);
    assert_eq!(retrato(&h).marcadas, 2);
    capturar(&mut h, "dados-busca-filtro-marcadas-claro.png");
}

#[test]
fn produtos() {
    let amb = Ambiente::exemplo("captura_produtos");
    let mut h = abrir(&amb, "produtos");
    afastar_mouse(&mut h);
    capturar(&mut h, "dados-produtos-claro.png");
}

#[test]
fn janela_da_linha_de_clientes() {
    let amb = Ambiente::exemplo("captura_janela_clientes");
    let mut h = abrir(&amb, "clientes");
    clicar_rotulo(&mut h, "7");
    esperar_que(&mut h, |r| r.janela.is_some());
    afastar_mouse(&mut h);
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha 7"));
    capturar(&mut h, "janela-linha-clientes-claro.png");
}

#[test]
fn janela_da_linha_nova() {
    let amb = Ambiente::exemplo("captura_janela_nova");
    let mut h = abrir(&amb, "clientes");
    clicar_rotulo(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.is_some());
    afastar_mouse(&mut h);
    capturar(&mut h, "janela-linha-nova-claro.png");
}

/// A ficha de P003 da prancheta 12a: a série em NULL (marcada) e a gerada só leitura.
fn ficha_de_p003(amb: &Ambiente, tema: Tema) -> Teste {
    let mut tabela = amb.tabela("produtos");
    tabela.rota = tabela.rota.com_dados(ConsultaDados::default());
    let mut h = abrir_com(amb, tabela, tema, Ganchos::default());
    let abre = h.query_all_by_role_and_label(Role::Button, "abrir").nth(2).expect("abrir P003").rect().center();
    clicar(&mut h, pos2(abre.x, abre.y));
    esperar_que(&mut h, |r| r.janela.is_some());
    afastar_mouse(&mut h);
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha P003"));
    h
}

#[test]
fn janela_da_linha_de_produtos_com_blob() {
    let amb = Ambiente::exemplo("captura_janela_produtos");
    executar(&amb, "UPDATE produtos SET serie = NULL WHERE codigo = 'P003'");
    for (tema, nome) in
        [(Tema::Claro, "janela-linha-produtos-blob-claro.png"), (Tema::Escuro, "janela-linha-produtos-blob-escuro.png")]
    {
        let mut h = ficha_de_p003(&amb, tema);
        capturar(&mut h, nome);
    }
}

#[test]
fn janela_da_linha_so_leitura() {
    // a prancheta 15 ("Ficha da linha só leitura"): o banco aberto só para leitura, a nota compacta no alto
    let mut amb = Ambiente::exemplo("captura_janela_so_leitura");
    executar(&amb, "UPDATE produtos SET serie = NULL WHERE codigo = 'P003'");
    let caminho = amb.caminho_banco();
    amb.servico.fechar_banco(&amb.estado.banco_id(), true).expect("fechar");
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só para leitura");
    amb.recarregar();
    for (tema, nome) in
        [(Tema::Claro, "janela-linha-so-leitura-claro.png"), (Tema::Escuro, "janela-linha-so-leitura-escuro.png")]
    {
        let mut h = ficha_de_p003(&amb, tema);
        capturar(&mut h, nome);
    }
}

#[test]
fn janela_da_linha_de_uma_visao() {
    // a ficha de leitura (rótulo · valor) da visão
    let amb = Ambiente::exemplo("captura_janela_visao");
    let mut h = abrir(&amb, "v_pedidos_por_cliente");
    duplo_clique_celula(&mut h, 0, 1);
    esperar_que(&mut h, |r| r.janela.is_some());
    afastar_mouse(&mut h);
    capturar(&mut h, "janela-linha-visao-claro.png");
}

#[test]
fn grade_vazia() {
    // a referência é a grade de extra_03 em lateral-com-muitas-tabelas-claro.png, com as medidas do Estúdio
    let amb = Ambiente::exemplo("captura_vazia");
    executar(&amb, "CREATE TABLE extra_03 (a);");
    let mut h = abrir(&amb, "extra_03");
    afastar_mouse(&mut h);
    // o cabeçalho continua à vista, e o vazio vai embaixo dele com o botão (U5 da revisão de UI de 22/10/2026)
    let frase = h.get_by_label("Nenhuma linha nesta tabela.").rect();
    let cabecalho = h.query_all_by_label("a").map(|n| n.rect()).find(|r| r.bottom() <= frase.top());
    assert!(cabecalho.is_some(), "o cabeçalho da coluna a, acima do vazio");
    let botao =
        h.query_all_by_role_and_label(Role::Button, "Nova linha").map(|n| n.rect()).find(|r| r.top() > frase.bottom());
    assert!(botao.is_some(), "o Nova linha embaixo da frase");
    assert!(h.query_by_label("Nenhuma linha").is_some() && h.query_by_label("Página 1 de 1").is_some());
    capturar(&mut h, "dados-vazio-claro.png");
}

#[test]
fn grade_de_uma_visao() {
    // a grade só leitura (sem caixas, coluna "abrir")
    let amb = Ambiente::exemplo("captura_visao");
    let mut h = abrir(&amb, "v_pedidos_por_cliente");
    afastar_mouse(&mut h);
    capturar(&mut h, "dados-visao-claro.png");
}

#[test]
fn pagina_que_nao_pode_ser_lida() {
    // o aviso de perigo no lugar da grade e o rodapé vazio
    let amb = Ambiente::exemplo("captura_erro");
    let ganchos = Ganchos {
        pagina: Some(std::sync::Arc::new(|pedido: &banco::PedidoPagina, _: &mut banco::Pagina| {
            if pedido.pagina > 1 {
                panic!("página quebrada de propósito");
            }
        })),
        ..Ganchos::default()
    };
    let mut h = abrir_com(&amb, amb.tabela("eventos"), Tema::Claro, ganchos);
    clicar_rotulo(&mut h, "Próxima página");
    esperar_que(&mut h, |r| r.erro.is_some());
    afastar_mouse(&mut h);
    capturar(&mut h, "dados-erro-claro.png");
}

#[test]
fn janela_de_linha_sem_chave() {
    // a ficha de leitura com o aviso "Esta linha não tem chave…" no alto
    let amb = Ambiente::exemplo("captura_sem_chave");
    executar(
        &amb,
        "CREATE TABLE sem_apelido (rowid TEXT, oid TEXT, _rowid_ TEXT, codigo TEXT PRIMARY KEY, nome TEXT); \
         INSERT INTO sem_apelido VALUES ('r', 'o', '_', NULL, 'sem chave'), ('r', 'o', '_', 'K1', 'com chave');",
    );
    let mut h = abrir(&amb, "sem_apelido");
    let l = (0..2).find(|&l| celula(&h, l, 4) == "sem chave").expect("linha sem chave");
    duplo_clique_celula(&mut h, l, 4);
    esperar_que(&mut h, |r| r.janela_aviso.is_some());
    afastar_mouse(&mut h);
    capturar(&mut h, "janela-linha-sem-chave-claro.png");
}

#[test]
fn janela_de_linha_com_erro() {
    // o erro do serviço num aviso de perigo no alto do corpo, a janela continua aberta
    let amb = Ambiente::exemplo("captura_janela_erro");
    let mut h = abrir(&amb, "clientes");
    let email_1 = texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 1"));
    clicar_rotulo(&mut h, "2");
    esperar_que(&mut h, |r| r.janela.is_some());
    let id_email = id_campo(&h, "email");
    preencher(&mut h, id_email, &email_1);
    clicar_rotulo(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela_aviso.is_some());
    afastar_mouse(&mut h);
    capturar(&mut h, "janela-linha-erro-claro.png");
}

fn com_dois_filtros(amb: &Ambiente) -> estado::TabelaAberta {
    let mut tabela = amb.tabela("clientes");
    let filtros = vec![
        FiltroDigitado { coluna: "limite".into(), op: Operador::Maior, valor: "1000".into() },
        FiltroDigitado { coluna: "cidade".into(), op: Operador::NaoNulo, valor: String::new() },
    ];
    tabela.rota = tabela.rota.com_dados(ConsultaDados { filtros, ..Default::default() });
    tabela
}

#[test]
fn faixa_com_as_etiquetas() {
    let amb = Ambiente::exemplo("captura_faixa");
    for (tema, nome) in [(Tema::Claro, "dados-faixa-claro.png"), (Tema::Escuro, "dados-faixa-escuro.png")] {
        let mut h = abrir_com(&amb, com_dois_filtros(&amb), tema, Ganchos::default());
        afastar_mouse(&mut h);
        assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro limite maior que 1000").is_some());
        assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro cidade não é nulo").is_some());
        capturar(&mut h, nome);
    }
}

#[test]
fn popover_da_condicao() {
    // a parada 2 do plano (sem prancheta): a condição da etiqueta aberta no vidro, nos dois temas
    let amb = Ambiente::exemplo("captura_popover");
    for (tema, nome) in [(Tema::Claro, "dados-popover-claro.png"), (Tema::Escuro, "dados-popover-escuro.png")] {
        let mut h = abrir_com(&amb, com_dois_filtros(&amb), tema, Ganchos::default());
        clicar_rotulo(&mut h, "Editar o filtro limite maior que 1000");
        assert!(retrato(&h).condicao_aberta);
        afastar_mouse(&mut h);
        capturar(&mut h, nome);
        // a condição nova, pelo "+ Condição"
        tecla(&mut h, egui::Key::Escape);
        clicar_rotulo(&mut h, "Nova condição");
        afastar_mouse(&mut h);
        capturar(&mut h, &nome.replace("popover", "popover-nova"));
    }
}

#[test]
fn janela_estreita_de_960() {
    let amb = Ambiente::exemplo("captura_960");
    let mut h =
        abrir_no_tamanho(&amb, com_dois_filtros(&amb), Tema::Claro, Ganchos::default(), egui::vec2(960.0, 700.0));
    afastar_mouse(&mut h);
    let limpar = h.get_by_role_and_label(Role::Button, "Limpar filtros").rect();
    assert!(limpar.right() <= 960.0, "o Limpar filtros cabe na faixa: {limpar:?}");
    capturar(&mut h, "dados-960-claro.png");
}

#[test]
fn confirmar_excluir() {
    let amb = Ambiente::exemplo("captura_confirmar");
    let mut h = abrir(&amb, "clientes");
    for l in 0..3 {
        clicar_marca(&mut h, Some(l));
    }
    clicar_rotulo(&mut h, "Excluir");
    afastar_mouse(&mut h);
    assert!(retrato(&h).confirmacao.is_some());
    capturar(&mut h, "confirmar-excluir-claro.png");
}

#[test]
fn menu_da_celula_claro_e_escuro() {
    let amb = Ambiente::exemplo("captura_menu_da_celula");
    for (tema, nome) in
        [(Tema::Claro, "dados-menu-da-celula-claro.png"), (Tema::Escuro, "dados-menu-da-celula-escuro.png")]
    {
        let mut h = abrir_com(&amb, amb.tabela("clientes"), tema, Ganchos::default());
        clicar_celula(&mut h, 1, 4);
        for _ in 0..3 {
            tecla_com(&mut h, SHIFT, egui::Key::ArrowDown);
        }
        let pos = centro_celula(&h, 2, 4);
        h.input_mut().events.push(egui::Event::PointerMoved(pos));
        h.step();
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run_steps(3);
        h.get_by_label("Colar").hover();
        h.run_steps(30);
        capturar(&mut h, nome);
    }
}
