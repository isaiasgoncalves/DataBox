//! A aba Dados com o `egui_kittest` contra o serviço de verdade: grade,
//! busca, filtros, ordenação, paginação, edição no lugar, janela da linha (NULL, padrão, BLOB), exclusão de
//! marcadas e visão só leitura. O que é gravado é conferido lendo o arquivo pelo serviço.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use amostras::{N_CLIENTES, N_EVENTOS};
use apoio::*;
use banco::Operador;
use comum::Celula;
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::{Ambiente, esperar};
use estado::{ConsultaDados, FiltroDigitado, OrigemJanelaExportar};
use interface::tema::Tema;
use tela_dados::Ganchos;

/// A contagem da faixa sem busca nem filtro ("60 linhas").
fn linhas(n: u64) -> String {
    interface::formato::plural(n, "linha", "linhas")
}

/// As linhas que passam com busca ou filtro, de quantas a tabela tem ("41 de 60 linhas", o `Retrato::total`).
fn linhas_de(n: u64, total: u64) -> String {
    format!("{} de {}", interface::formato::formatar_numero(n), linhas(total))
}

fn contar(h: &H, sql: &str) -> u64 {
    inteiro(&ler_um(h, sql)) as u64
}

fn contar_busca(h: &H, texto: &str, extra: &str) -> u64 {
    let colunas = ["id", "nome", "email", "cidade", "limite", "ativo", "criado_em"];
    let ou: Vec<String> = colunas.iter().map(|c| format!("CAST({c} AS TEXT) LIKE '%{texto}%'")).collect();
    contar(h, &format!("SELECT count(*) FROM clientes WHERE ({}){extra}", ou.join(" OR ")))
}

// ---- leitura: paginação, ordenação, busca e filtros -----------------------------------------------------

#[test]
fn paginacao_e_contagem_de_eventos() {
    let amb = Ambiente::exemplo("dados_paginacao");
    let mut h = abrir(&amb, "eventos");
    let r = retrato(&h);
    assert_eq!(r.total, linhas(N_EVENTOS as u64));
    assert_eq!(r.linhas, 100);
    assert_eq!(r.contagem_linhas, "Linhas 1–100 de 5.000");
    assert_eq!(r.pagina_atual, "Página 1 de 50");
    assert!(!r.anterior_habilitado);

    clicar_botao(&mut h, "Próxima página");
    esperar_que(&mut h, |r| r.contagem_linhas == "Linhas 101–200 de 5.000" && !r.carregando);
    assert_eq!(ultima_rota(&h).map(|c| c.pagina), Some(2), "o endereço acompanha (#/t/eventos/dados?pagina=2)");
    assert!(sem_entrada_no_historico(&h), "trocar de página não entra no histórico");
    assert_eq!(h.state().tela.doc.parte.rotulo_da_linha(0).as_deref(), Some("101"));

    escolher(&mut h, "Linhas por página", 0, "250 por página");
    esperar_que(&mut h, |r| r.pagina_atual == "Página 1 de 20" && r.linhas == 250);
    let rota = ultima_rota(&h).expect("rota");
    assert_eq!((rota.por_pagina, rota.pagina), (250, 1));
    clicar_botao(&mut h, "Próxima página");
    esperar_que(&mut h, |r| r.contagem_linhas == "Linhas 251–500 de 5.000");

    // a tela tem endereço: recarregar volta à mesma página, com o mesmo tamanho
    let mut h = recarregar(&amb, h);
    assert_eq!(retrato(&h).contagem_linhas, "Linhas 251–500 de 5.000");
    assert_eq!(h.state().tela.doc.parte.rotulo_da_linha(0).as_deref(), Some("251"));

    // página além da última (endereço antigo) cai na última que existe
    let mut tabela = amb.tabela("eventos");
    tabela.rota = tabela.rota.com_dados(ConsultaDados { pagina: 999, ..Default::default() });
    h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    esperar_que(&mut h, |r| r.pagina_atual == "Página 50 de 50");
    assert_eq!(ultima_rota(&h).map(|c| c.pagina), Some(50));
}

#[test]
fn ordenacao_no_cabecalho() {
    let amb = Ambiente::exemplo("dados_ordenacao");
    let mut h = abrir(&amb, "clientes");
    assert_eq!(h.state().tela.doc.parte.tipo_da_coluna(1).as_deref(), Some("TEXT"));
    let primeiro_asc = texto(&ler_um(&h, "SELECT nome FROM clientes ORDER BY nome ASC, rowid ASC LIMIT 1"));
    let primeiro_desc = texto(&ler_um(&h, "SELECT nome FROM clientes ORDER BY nome DESC, rowid ASC LIMIT 1"));

    clicar_botao(&mut h, "nome");
    esperar_grade(&mut h);
    let rota = ultima_rota(&h).expect("rota");
    assert_eq!((rota.ordem.as_deref(), rota.direcao), (Some("nome"), banco::Direcao::Asc));
    assert_eq!(celula(&h, 0, 1), primeiro_asc);

    clicar_botao(&mut h, "nome");
    esperar_grade(&mut h);
    assert_eq!(ultima_rota(&h).map(|r| r.direcao), Some(banco::Direcao::Desc));
    assert_eq!(celula(&h, 0, 1), primeiro_desc);

    let mut h = recarregar(&amb, h);
    assert_eq!(celula(&h, 0, 1), primeiro_desc);

    clicar_botao(&mut h, "nome");
    esperar_grade(&mut h);
    assert_eq!(ultima_rota(&h).and_then(|r| r.ordem), None);
    assert_eq!(celula(&h, 0, 0), "1");
}

#[test]
fn busca_e_filtros_no_endereco() {
    let amb = Ambiente::exemplo("dados_busca");
    let mut h = abrir(&amb, "clientes");
    let id_busca = campo_busca();

    // Ctrl+F leva à busca, também com a grade em foco
    clicar_celula(&mut h, 0, 1);
    assert!(grade_com_foco(&h));
    tecla_com(&mut h, CTRL, Key::F);
    assert!(com_foco(&h, id_busca), "Ctrl+F foca a busca");

    let n_recife = contar_busca(&h, "Recife", "");
    assert!(n_recife > 0 && n_recife < N_CLIENTES as u64);
    for letra in ["R", "e", "c", "i", "f", "e"] {
        digitar(&mut h, letra);
    }
    let total = N_CLIENTES as u64;
    esperar_que(&mut h, |r| r.total == linhas_de(n_recife, total) && !r.carregando);
    let r = retrato(&h);
    assert_eq!(r.contagem_linhas, format!("Linhas 1–{n_recife} de {n_recife}"), "a contagem só no rodapé (U3)");
    assert_eq!(r.filtradas, format!("filtradas de {total}"), "com busca, de quantas a tabela tem");
    assert!(h.query_by_label_contains(" de 60 linhas").is_none(), "a faixa não conta mais");
    assert_eq!(ultima_rota(&h).map(|r| r.busca), Some("Recife".to_string()));
    assert!(sem_entrada_no_historico(&h), "a busca também troca o endereço sem entrar no histórico");
    assert!(com_foco(&h, id_busca), "só a grade redesenha: a busca continua com o foco");

    condicao_pelo_popover(&mut h, "ativo", "igual a", Some("1"));
    let n_ativos = contar_busca(&h, "Recife", " AND ativo = 1");
    esperar_que(&mut h, |r| r.total == linhas_de(n_ativos, total) && !r.carregando);
    assert_eq!(ultima_rota(&h).map(|r| r.filtros.len()), Some(1));
    assert!(
        h.query_by_role_and_label(Role::Button, "Editar o filtro ativo igual a 1").is_some(),
        "a etiqueta da condição"
    );

    // operador sem valor esconde o campo do valor
    clicar_botao(&mut h, "Nova condição");
    escolher(&mut h, "Coluna da condição", 0, "limite");
    escolher(&mut h, "Operador da condição", 0, "não é nulo");
    h.run_steps(2);
    assert!(h.query_by_role_and_label(Role::TextInput, "Valor da condição").is_none());
    clicar_botao(&mut h, "Aplicar");
    let n_final = contar_busca(&h, "Recife", " AND ativo = 1 AND limite IS NOT NULL");
    esperar_que(&mut h, |r| r.total == linhas_de(n_final, total) && !r.carregando);
    assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro limite não é nulo").is_some());

    // o "Exportar…" do cabeçalho enxerga a busca e os filtros da aba
    match h.state().tela.doc.tabela.pedido_exportacao().origem {
        OrigemJanelaExportar::Tabela { filtro: Some(f), .. } => {
            assert_eq!(f.busca.as_deref(), Some("Recife"));
            assert_eq!(f.filtros.len(), 2);
            assert_eq!(f.linhas, Some(n_final), "o total já contado vai junto");
        }
        outra => panic!("{outra:?}"),
    }

    // recarregar volta à mesma busca e aos mesmos filtros
    let mut h = recarregar(&amb, h);
    assert_eq!(retrato(&h).filtros, 2);
    assert_eq!(retrato(&h).total, linhas_de(n_final, total));
    let rota = h.state().tela.doc.tabela.rota.dados().cloned().expect("rota");
    assert_eq!(rota.filtros[0], FiltroDigitado { coluna: "ativo".into(), op: Operador::Igual, valor: "1".into() });
    assert_eq!(rota.filtros[1].op, Operador::NaoNulo);

    clicar_botao(&mut h, "Limpar filtros");
    esperar_que(&mut h, |r| r.filtros == 0 && r.total == linhas_de(n_recife, total) && !r.carregando);
    assert_eq!(ultima_rota(&h).map(|r| r.filtros.len()), Some(0));

    let id_busca = campo_busca();
    preencher(&mut h, id_busca, "nada-assim@example.test");
    esperar_que(&mut h, |r| r.vazio.is_some() && !r.carregando);
    let r = retrato(&h);
    assert_eq!(r.vazio.as_deref(), Some("Nenhuma linha corresponde à busca ou aos filtros."));
    assert_eq!(r.contagem_linhas, "Nenhuma linha");
    assert_eq!(r.filtradas, format!("filtradas de {total}"));
    assert_eq!(r.total, linhas_de(0, total));
}

#[test]
fn celulas_nulo_vazio_e_nome_com_espaco() {
    let amb = Ambiente::exemplo("dados_celulas");
    let h = abrir(&amb, "Tabela com espaço");
    assert_eq!(h.state().tela.doc.parte.colunas_da_grade()[0], "coluna \"citada\"");
    assert!(h.query_by_role_and_label(Role::Button, "coluna \"citada\"").is_some());
    assert_eq!(retrato(&h).linhas, 4);
    assert_eq!(celula(&h, 0, 0), "olá, mundo");
    assert_eq!(celula(&h, 2, 0), "vazio");
    assert_eq!(celula(&h, 3, 0), "NULL");
    assert_eq!(h.state().tela.doc.parte.coluna_numerica(1), Some(true));
}

#[test]
fn texto_longo_abre_a_janela() {
    let amb = Ambiente::exemplo("dados_texto_longo");
    let mut tabela = amb.tabela("pedidos");
    let filtro = FiltroDigitado { coluna: "observacao".into(), op: Operador::Comeca, valor: "Texto longo".into() };
    tabela.rota = tabela.rota.com_dados(ConsultaDados { filtros: vec![filtro], ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    let col = h.state().tela.doc.parte.colunas_da_grade().iter().position(|c| c == "observacao").expect("observacao");
    assert!(matches!(h.state().tela.doc.parte.valor_celula(0, col), Some(Celula::TextoCortado { .. })));
    duplo_clique_celula(&mut h, 0, col);
    esperar_que(&mut h, |r| r.janela.is_some());
    let aba = &h.state().tela.doc.parte;
    assert!(aba.texto_campo_janela("observacao").is_some_and(|t| t.chars().count() > 700));
    assert_eq!(aba.campo_janela_largo("observacao"), Some(true));
    assert!(retrato(&h).editando.is_none(), "não edita no lugar");
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.janela.is_none());
}

// ---- gravação ----------------------------------------------------------------------------------------------

#[test]
fn edicao_no_lugar_com_teclado() {
    let amb = Ambiente::exemplo("dados_edicao");
    let mut h = abrir(&amb, "clientes");
    duplo_clique_celula(&mut h, 0, 1);
    let r = retrato(&h);
    assert_eq!(r.editando.as_ref().map(|e| (e.0, e.1)), Some((0, 1)));
    preencher_edicao(&mut h, "Nome Editado");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert_eq!(celula(&h, 0, 1), "Nome Editado");
    assert!(h.state().tela.doc.parte.piscando(0, 1), "a célula pisca o .salvo");
    assert!(grade_com_foco(&h), "o foco volta para a célula");
    assert_eq!(texto(&ler_um(&h, "SELECT nome FROM clientes WHERE id = 1")), "Nome Editado");

    // seta para a direita, F2 edita, Esc desiste
    let email_original = texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 1"));
    tecla(&mut h, Key::ArrowRight);
    assert_eq!(retrato(&h).ativa, Some((0, 2)));
    tecla(&mut h, Key::F2);
    assert_eq!(retrato(&h).editando.as_ref().map(|e| (e.0, e.1)), Some((0, 2)));
    preencher_edicao(&mut h, "nao.grava@example.test");
    tecla(&mut h, Key::Escape);
    assert!(retrato(&h).editando.is_none());
    assert_eq!(celula(&h, 0, 2), email_original);
    assert!(grade_com_foco(&h));
    assert_eq!(texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 1")), email_original);

    // seta para baixo, Enter edita, Tab grava e vai para a próxima célula
    tecla(&mut h, Key::ArrowDown);
    assert!(grade_com_foco(&h), "as setas não tiram o foco da grade");
    tecla(&mut h, Key::Enter);
    preencher_edicao(&mut h, "novo.endereco@example.test");
    tecla(&mut h, Key::Tab);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0);
    assert_eq!(celula(&h, 1, 2), "novo.endereco@example.test");
    assert_eq!(retrato(&h).ativa, Some((1, 3)));
    assert!(grade_com_foco(&h));
    assert_eq!(texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 2")), "novo.endereco@example.test");

    // digitar numa célula ativa começa a edição, como numa planilha
    digitar(&mut h, "Curitiba");
    assert_eq!(retrato(&h).editando, Some((1, 3, "Curitiba".into())));
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert_eq!(celula(&h, 1, 3), "Curitiba");
    assert_eq!(texto(&ler_um(&h, "SELECT cidade FROM clientes WHERE id = 2")), "Curitiba");

    // erro do serviço (CHECK limite >= 0): toast e a célula volta ao valor de antes
    let antes = celula(&h, 1, 4);
    duplo_clique_celula(&mut h, 1, 4);
    preencher_edicao(&mut h, "-5");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0);
    h.run_steps(1);
    assert!(toast(&h).is_some_and(|t| !t.is_empty()), "o erro aparece no toast");
    assert_eq!(celula(&h, 1, 4), antes);
    assert!(retrato(&h).editando.is_none());
    assert_eq!(inteiro(&ler_um(&h, "SELECT limite >= 0 FROM clientes WHERE id = 2")), 1);
}

/// Troca o texto da célula em edição (seleciona tudo e digita).
fn preencher_edicao(h: &mut H, texto: &str) {
    tecla_com(h, CTRL, Key::A);
    digitar(h, texto);
}

#[test]
fn nova_linha_com_padrao_e_nulo() {
    let amb = Ambiente::exemplo("dados_nova_linha");
    let mut h = abrir(&amb, "clientes");
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.as_deref() == Some("Nova linha"));
    assert!(h.query_by_role_and_label(Role::Button, "Incluir linha").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Excluir linha").is_none());

    // id automático e colunas com padrão nascem com "Usar o padrão" marcado e o campo travado
    let aba = &h.state().tela.doc.parte;
    for coluna in ["id", "cidade", "ativo", "criado_em"] {
        assert_eq!(aba.caixas_campo_janela(coluna).and_then(|c| c.1), Some(true), "{coluna}");
    }
    assert_eq!(aba.caixas_campo_janela("nome"), Some((None, None)), "NOT NULL não oferece nulo");
    let cidade = h.get_by_role_and_label(Role::TextInput, "cidade");
    assert!(desabilitado(&cidade), "o campo travado pelo padrão");

    let (id_nome, id_email, id_limite) = (id_campo(&h, "nome"), id_campo(&h, "email"), id_campo(&h, "limite"));
    preencher(&mut h, id_nome, "Pessoa Inventada");
    preencher(&mut h, id_email, "pessoa.inventada@example.test");
    preencher(&mut h, id_limite, "123");
    clicar_caixa_do_campo(&mut h, "limite", "NULL");
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("limite").and_then(|c| c.0), Some(true));
    clicar_botao(&mut h, "Incluir linha");
    esperar_que(&mut h, |r| r.janela.is_none());
    assert_eq!(toast(&h).as_deref(), Some("Linha incluída."));
    assert!(h.state().tela.doc.tabela.recarregar, "a casca relê a tabela (a contagem do cabeçalho muda)");

    let linha = ler(
        &h,
        "SELECT id, nome, cidade, limite, ativo, criado_em IS NOT NULL FROM clientes \
         WHERE email = 'pessoa.inventada@example.test'",
    );
    assert_eq!(
        linha,
        vec![vec![
            Celula::Inteiro(N_CLIENTES + 1),
            Celula::Texto("Pessoa Inventada".into()),
            Celula::Texto("São Paulo".into()),
            Celula::Nulo,
            Celula::Inteiro(1),
            Celula::Inteiro(1),
        ]]
    );
    assert_eq!(inteiro(&ler_um(&h, "SELECT seq FROM sqlite_sequence WHERE name = 'clientes'")), N_CLIENTES + 1);

    // a tela é refeita pela casca: a aba nova mostra a contagem nova
    let mut h = recarregar(&amb, h);
    assert_eq!(retrato(&h).total, linhas(N_CLIENTES as u64 + 1));

    // a linha nova abre na janela; a caixa "NULL" grava NULL e "Usar o padrão" não existe na edição
    // a linha 61 fica abaixo da parte visível da grade: rola até ela
    h.state_mut().tela.doc.parte.rolar_ate(N_CLIENTES as usize);
    let rotulo = (N_CLIENTES + 1).to_string();
    let alvo = rotulo.clone();
    esperar(&mut h, move |h| h.query_by_role_and_label(Role::Button, &alvo).is_some());
    let abre = h.get_by_role_and_label(Role::Button, &rotulo).rect().center();
    let abre = egui::pos2(abre.x, abre.y);
    clicar(&mut h, abre);
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela, Some(format!("Linha {}", N_CLIENTES + 1)));
    assert!(h.query_by_label("Usar o padrão").is_none());
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("limite").and_then(|c| c.0), Some(true));
    clicar_caixa_do_campo(&mut h, "cidade", "NULL");
    clicar_botao(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela.is_none() && r.gravacoes_pendentes == 0);
    assert_eq!(ler_um(&h, &format!("SELECT cidade FROM clientes WHERE id = {}", N_CLIENTES + 1)), Celula::Nulo);

    // Duplicar parte dos valores da janela, com o id de novo automático
    clicar(&mut h, abre);
    esperar_que(&mut h, |r| r.janela.is_some());
    clicar_botao(&mut h, "Duplicar");
    esperar_que(&mut h, |r| r.janela.as_deref() == Some("Nova linha"));
    assert_eq!(h.state().tela.doc.parte.texto_campo_janela("nome").as_deref(), Some("Pessoa Inventada"));
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("id").and_then(|c| c.1), Some(true));
    let id_email = id_campo(&h, "email");
    preencher(&mut h, id_email, "pessoa.copia@example.test");
    clicar_botao(&mut h, "Incluir linha");
    esperar_que(&mut h, |r| r.janela.is_none());
    assert_eq!(
        ler(&h, "SELECT id, nome, cidade FROM clientes WHERE email = 'pessoa.copia@example.test'"),
        vec![vec![Celula::Inteiro(N_CLIENTES + 2), Celula::Texto("Pessoa Inventada".into()), Celula::Nulo]]
    );
}

#[test]
fn itens_pedido_chave_composta() {
    let amb = Ambiente::exemplo("dados_itens");
    let mut h = abrir(&amb, "itens_pedido");
    let abre = h.query_all_by_role_and_label(Role::Button, "1").next().expect("abridor da linha 1").rect().center();
    clicar(&mut h, egui::pos2(abre.x, abre.y));
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha 1"));
    let id_qtd = id_campo(&h, "quantidade");
    preencher(&mut h, id_qtd, "4");
    clicar_botao(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela.is_none());
    assert_eq!(toast(&h).as_deref(), Some("Linha salva."));
    let col_qtd = h.state().tela.doc.parte.colunas_da_grade().iter().position(|c| c == "quantidade").expect("coluna");
    assert_eq!(celula(&h, 0, col_qtd), "4");
    assert_eq!(inteiro(&ler_um(&h, "SELECT quantidade FROM itens_pedido WHERE rowid = 1")), 4);

    // vírgula decimal no lugar: o serviço converte pela afinidade NUMERIC
    let col_preco = h.state().tela.doc.parte.colunas_da_grade().iter().position(|c| c == "preco").expect("coluna");
    duplo_clique_celula(&mut h, 1, col_preco);
    preencher_edicao(&mut h, "12,5");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert_eq!(celula(&h, 1, col_preco), "12.5");
    assert_eq!(
        ler(&h, "SELECT preco, typeof(preco) FROM itens_pedido WHERE rowid = 2"),
        vec![vec![Celula::Real(12.5), Celula::Texto("real".into())]]
    );
}

#[test]
fn produtos_sem_rowid_blob_inteiro_grande_e_gerada() {
    let amb = Ambiente::exemplo("dados_produtos");
    let pasta = amb.pasta().to_path_buf();
    let mut h = abrir(&amb, "produtos");
    let aba = &h.state().tela.doc.parte;
    assert_eq!(aba.tem_coluna_abrir(), Some(true));
    assert_eq!(aba.rotulo_da_linha(0).as_deref(), Some("abrir"));
    // inteiro acima de 2^53 aparece exato
    assert_eq!(celula(&h, 3, 5), "9007199254740997");
    assert_eq!(celula(&h, 2, 4), "BLOB · 112 bytes");
    assert_eq!(aba.tipo_da_coluna(6).as_deref(), Some("INTEGER · gerada"));

    // inteiro grande editado no lugar grava exato
    duplo_clique_celula(&mut h, 3, 5);
    preencher_edicao(&mut h, "9007199254740999");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert_eq!(celula(&h, 3, 5), "9007199254740999");
    assert_eq!(inteiro(&ler_um(&h, "SELECT serie FROM produtos WHERE codigo = 'P004'")), 9007199254740999);

    // coluna gerada e BLOB não editam no lugar: abrem a janela
    duplo_clique_celula(&mut h, 0, 6);
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha P001"));
    assert!(h.state().tela.doc.parte.id_campo_janela("preco_centavos").is_some());
    // a gerada é um campo só leitura com o valor (`.campo.so-leitura`): o que se digita não entra
    let gerada = h.get_by_role_and_label(Role::TextInput, "preco_centavos");
    assert_eq!(gerada.value().as_deref(), Some("8618"));
    let c = gerada.rect().center();
    clicar(&mut h, egui::pos2(c.x, c.y));
    digitar(&mut h, "9");
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "preco_centavos").value().as_deref(), Some("8618"));
    assert!(h.query_all_by_label("8618").count() >= 1, "o valor gerado em texto (e o Valor das Propriedades)");
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.janela.is_none());

    duplo_clique_celula(&mut h, 2, 4);
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha P003"));
    assert!(h.query_by_label("112 bytes").is_some(), "a pílula do BLOB");

    // Salvar em arquivo (sem reticência) grava o conteúdo exato do BLOB
    let destino = pasta.join("foto_salva.bin");
    h.state_mut().tela.doc.parte.ganchos_mut().salvar_como = Some(destino.clone());
    clicar_botao(&mut h, "Salvar em arquivo");
    esperar_que(&mut h, |r| r.janela_aviso.is_some());
    assert_eq!(retrato(&h).janela_aviso.as_deref(), Some("Arquivo salvo: foto_salva.bin · 112 bytes"));
    let original = match ler_um(&h, "SELECT hex(foto) FROM produtos WHERE codigo = 'P003'") {
        Celula::Texto(t) => t,
        outro => panic!("{outro:?}"),
    };
    let salvo = std::fs::read(&destino).expect("arquivo salvo");
    assert_eq!(salvo.iter().map(|b| format!("{b:02X}")).collect::<String>(), original);

    // Substituir por arquivo manda o conteúdo do arquivo; o preço mexe na coluna gerada
    let novo = pasta.join("novo_conteudo.bin");
    let bytes: Vec<u8> = (0..=255u8).cycle().take(768).collect();
    std::fs::write(&novo, &bytes).expect("gravar");
    h.state_mut().tela.doc.parte.ganchos_mut().escolher_arquivo = Some(novo.clone());
    clicar_botao(&mut h, "Substituir por arquivo");
    assert!(h.query_by_label("Novo conteúdo: novo_conteudo.bin").is_some());
    let id_preco = id_campo(&h, "preco");
    preencher(&mut h, id_preco, "10");
    clicar_botao(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela.is_none() && r.gravacoes_pendentes == 0);
    assert_eq!(
        ler(&h, "SELECT length(foto), preco, preco_centavos FROM produtos WHERE codigo = 'P003'"),
        vec![vec![Celula::Inteiro(768), Celula::Inteiro(10), Celula::Inteiro(1000)]]
    );
    assert_eq!(celula(&h, 2, 4), "BLOB · 768 bytes");
    assert_eq!(celula(&h, 2, 6), "1000");
    assert!(h.state().tela.doc.parte.piscando(2, 6), "a coluna gerada que mudou pisca");
}

#[test]
fn excluir_marcadas_com_confirmacao() {
    let amb = Ambiente::exemplo("dados_excluir");
    let mut h = abrir(&amb, "eventos");
    for l in 0..3 {
        clicar_marca(&mut h, Some(l));
    }
    assert_eq!(retrato(&h).marcadas, 3);
    assert!(h.query_by_label("· 3 marcadas").is_some(), "no rodapé");
    assert_eq!(h.state().tela.doc.tabela.selecao.len(), 3, "o Exportar… oferece só as linhas marcadas");
    let caixa = h.get_by_role_and_label(Role::CheckBox, "Marcar todas as linhas desta página");
    assert_eq!(
        caixa.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::Mixed),
        "parte da página marcada: indeterminada"
    );

    // Delete na grade pergunta; recusado, nada sai
    clicar_celula(&mut h, 5, 1);
    tecla(&mut h, Key::Delete);
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Excluir 3 linhas de eventos? Isso não pode ser desfeito."));
    tecla(&mut h, Key::Escape);
    assert!(retrato(&h).confirmacao.is_none());
    assert_eq!(retrato(&h).marcadas, 3);
    assert_eq!(contar(&h, "SELECT count(*) FROM eventos"), N_EVENTOS as u64);

    assert_eq!(ofertas(&h).dica(estado::Verbo::Excluir), "Excluir as 3 linhas marcadas");
    clicar_botao(&mut h, "Excluir");
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Excluir 3 linhas de eventos? Isso não pode ser desfeito."));
    clicar_botao(&mut h, "Excluir");
    esperar_que(&mut h, |r| r.marcadas == 0 && !r.carregando && r.linhas > 0);
    assert_eq!(toast(&h).as_deref(), Some("3 linhas excluídas."));
    assert!(h.state().tela.doc.tabela.selecao.is_empty());
    assert_eq!(
        ler(&h, "SELECT count(*), min(rowid) FROM eventos"),
        vec![vec![Celula::Inteiro(N_EVENTOS - 3), Celula::Inteiro(4)]]
    );
    let mut h = recarregar(&amb, h);
    assert_eq!(retrato(&h).total, linhas(N_EVENTOS as u64 - 3));

    // a caixa do cabeçalho marca a página inteira
    let estado_da_caixa = |h: &H| {
        h.get_by_role_and_label(Role::CheckBox, "Marcar todas as linhas desta página").accesskit_node().toggled()
    };
    assert_eq!(estado_da_caixa(&h), Some(egui::accesskit::Toggled::False));
    clicar_marca(&mut h, None);
    assert!(h.query_by_label("· 100 marcadas").is_some());
    assert_eq!(estado_da_caixa(&h), Some(egui::accesskit::Toggled::True));
    clicar_marca(&mut h, None);
    assert_eq!(retrato(&h).marcadas, 0);
    assert_eq!(estado_da_caixa(&h), Some(egui::accesskit::Toggled::False));
    assert!(h.query_by_label_contains("marcada").is_none(), "o rodapé sem marcadas");
}

#[test]
fn excluir_linha_pela_janela() {
    let amb = Ambiente::exemplo("dados_excluir_linha");
    let mut h = abrir(&amb, "eventos");
    let abre = h.get_by_role_and_label(Role::Button, "2").rect().center();
    clicar(&mut h, egui::pos2(abre.x, abre.y));
    esperar_que(&mut h, |r| r.janela.is_some());
    clicar_botao(&mut h, "Excluir linha");
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Excluir a linha 2 de eventos? Isso não pode ser desfeito."));
    clicar_botao(&mut h, "Excluir");
    esperar_que(&mut h, |r| r.janela.is_none() && r.confirmacao.is_none());
    esperar(&mut h, |h| toast(h).as_deref() == Some("Linha excluída."));
    assert_eq!(contar(&h, "SELECT count(*) FROM eventos WHERE rowid = 2"), 0);
}

#[test]
fn visao_so_leitura() {
    let amb = Ambiente::exemplo("dados_visao");
    let mut h = abrir(&amb, "v_pedidos_por_cliente");
    assert_eq!(retrato(&h).total, linhas(N_CLIENTES as u64));
    assert_eq!(h.state().tela.doc.parte.tem_caixas(), Some(false));
    // visão: só Filtrar e Buscar (a tela de tabela junta Atualizar e Exportar)
    let o = ofertas(&h);
    let ligados: Vec<estado::Verbo> = o.iter().filter(|(_, x)| x.ligado).map(|(v, _)| v).collect();
    assert_eq!(ligados, [estado::Verbo::Buscar, estado::Verbo::Filtrar]);
    assert!(h.query_by_role_and_label(Role::Button, "Nova linha").is_none());
    assert_eq!(h.state().tela.doc.parte.rotulo_da_linha(0).as_deref(), Some("abrir"));

    duplo_clique_celula(&mut h, 0, 1);
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha 1"));
    assert_eq!(h.state().tela.doc.parte.linhas_leitura_janela(), Some(4));
    // os campos são só leitura: o que se digita não entra
    let nome = h.get_by_role_and_label(Role::TextInput, "nome");
    let antes = nome.value();
    let c = nome.rect().center();
    clicar(&mut h, egui::pos2(c.x, c.y));
    digitar(&mut h, "xyz");
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "nome").value(), antes);
    assert!(h.query_by_role_and_label(Role::Button, "Salvar").is_none(), "só Copiar a linha e Fechar");
    assert!(h.query_all_by_role_and_label(Role::Button, "Fechar").count() >= 1);
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.janela.is_none());

    // Enter na célula ativa também abre a janela, sem editar no lugar
    clicar_celula(&mut h, 1, 1);
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha 2"));
    assert!(retrato(&h).editando.is_none());
}

#[test]
fn erro_de_unique_aparece_na_janela() {
    let amb = Ambiente::exemplo("dados_unique");
    let mut h = abrir(&amb, "clientes");
    let email_1 = texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 1"));
    let email_2 = texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 2"));
    let abre = h.get_by_role_and_label(Role::Button, "2").rect().center();
    clicar(&mut h, egui::pos2(abre.x, abre.y));
    esperar_que(&mut h, |r| r.janela.is_some());
    let id_email = id_campo(&h, "email");
    preencher(&mut h, id_email, &email_1);
    clicar_botao(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela_aviso.is_some());
    let r = retrato(&h);
    assert!(r.janela.is_some(), "a janela continua aberta");
    assert!(!r.janela_aviso.unwrap_or_default().is_empty());
    assert!(!desabilitado(&h.get_by_role_and_label(Role::Button, "Salvar")), "Salvar volta a valer");
    assert_eq!(texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 2")), email_2);
}

// ---- correções da revisão da interface ----------------------------------------------------------------------

#[test]
fn coluna_sem_tipo_grava_numero_como_numero() {
    let amb = Ambiente::exemplo("dados_sem_tipo");
    executar(&amb, "CREATE TABLE solta (a, b); INSERT INTO solta VALUES (5, 'x'), (7, 'y'), (10, 'z');");
    let mut h = abrir(&amb, "solta");
    duplo_clique_celula(&mut h, 0, 0);
    preencher_edicao(&mut h, "6");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert!(h.state().tela.doc.parte.piscando(0, 0));
    assert_eq!(
        ler(&h, "SELECT a, typeof(a) FROM solta WHERE rowid = 1"),
        vec![vec![Celula::Inteiro(6), Celula::Texto("integer".into())]]
    );
    assert_eq!(n_inteiros(&ler(&h, "SELECT a FROM solta ORDER BY a")), vec![6, 7, 10]);
}

#[test]
fn nova_linha_pelo_teclado_comeca_no_primeiro_campo_de_texto() {
    let amb = Ambiente::exemplo("dados_nova_teclado");
    let mut h = abrir(&amb, "clientes");
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.is_some());
    h.run_steps(2);
    // id é automático (caixa "Usar o padrão" marcada): o foco vai para o primeiro campo de digitar
    assert!(com_foco(&h, id_campo(&h, "nome")));
    digitar(&mut h, "Pessoa Digitada Direto");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.janela.is_none());
    assert_eq!(
        ler(&h, &format!("SELECT nome, cidade, ativo FROM clientes WHERE id = {}", N_CLIENTES + 1)),
        vec![vec![
            Celula::Texto("Pessoa Digitada Direto".into()),
            Celula::Texto("São Paulo".into()),
            Celula::Inteiro(1)
        ]]
    );
    // a tela é refeita (a contagem muda) e o foco volta para a grade
    let h = recarregar(&amb, h);
    assert!(grade_com_foco(&h), "o foco volta para a célula ativa da grade");
}

#[test]
fn casca_que_mantem_a_aba_ve_a_pagina_relida() {
    let amb = Ambiente::exemplo("dados_casca_mantem");
    let mut h = abrir(&amb, "clientes");
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.is_some());
    h.run_steps(2);
    digitar(&mut h, "Outra Pessoa");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.janela.is_none());
    assert!(h.state().tela.doc.tabela.recarregar, "pede à casca que releia a tabela");
    // a casca releu a tabela sem refazer a aba: a própria aba relê a página
    h.state_mut().tela.doc.tabela.recarregar = false;
    esperar_que(&mut h, |r| r.total == linhas(N_CLIENTES as u64 + 1));
    assert!(grade_com_foco(&h), "o foco vai para a linha nova");
    let ativa = retrato(&h).ativa.expect("célula ativa");
    assert_eq!(h.state().tela.doc.parte.rotulo_da_linha(ativa.0), Some((N_CLIENTES + 1).to_string()));
}

/// Segura cada gravação no lugar por um tempo, como numa máquina lenta: a próxima edição sai antes da resposta.
fn gravacoes_lentas(segundos: f64) -> Ganchos {
    Ganchos {
        gravacao: Some(Arc::new(move || std::thread::sleep(Duration::from_secs_f64(segundos)))),
        ..Ganchos::default()
    }
}

#[test]
fn valor_recusado_avisa_mesmo_depois_de_sair_da_tela() {
    let amb = Ambiente::exemplo("dados_sair_da_tela");
    let mut h = abrir_com(&amb, amb.tabela("clientes"), Tema::Claro, gravacoes_lentas(0.4));
    let antes = ler_um(&h, "SELECT limite FROM clientes WHERE id = 2");
    duplo_clique_celula(&mut h, 1, 4); // limite, com CHECK (limite >= 0)
    preencher_edicao(&mut h, "-9");
    // o clique fora grava (o blur), e a tela já é outra quando o erro volta
    let fora = egui::pos2(600.0, 860.0);
    let botao = |pressed| egui::Event::PointerButton {
        pos: fora,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.input_mut().events.extend([egui::Event::PointerMoved(fora), botao(true), botao(false)]);
    h.step();
    assert_eq!(retrato(&h).gravacoes_pendentes, 1, "a célula entrou na fila ao perder o foco");
    trocar_aba(&mut h, amb.tabela("eventos"));
    esperar(&mut h, |h| toast(h).is_some_and(|t| t.contains("O valor de limite em clientes não foi salvo")));
    assert_eq!(ler_um(&h, "SELECT limite FROM clientes WHERE id = 2"), antes);
}

#[test]
fn editar_a_chave_e_logo_outra_celula_da_mesma_linha() {
    let amb = Ambiente::exemplo("dados_chave_editada");
    // WITHOUT ROWID: a chave é o código
    executar(
        &amb,
        "CREATE TABLE codigos (k TEXT PRIMARY KEY, d TEXT) WITHOUT ROWID; \
         INSERT INTO codigos VALUES ('A1', 'um'), ('B2', 'dois');",
    );
    let mut h = abrir_com(&amb, amb.tabela("codigos"), Tema::Claro, gravacoes_lentas(0.3));
    let codigo = celula(&h, 0, 0);
    let novo = format!("{codigo}X");
    duplo_clique_celula(&mut h, 0, 0);
    preencher_edicao(&mut h, &novo);
    tecla(&mut h, Key::Tab); // grava a chave e vai para a descrição
    tecla(&mut h, Key::Enter);
    preencher_edicao(&mut h, "Descrição depois da chave");
    tecla(&mut h, Key::Enter); // sai antes da resposta da primeira: espera a vez e usa a chave nova
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0);
    assert_eq!(
        ler(&h, &format!("SELECT k, d FROM codigos WHERE k IN ('{codigo}', '{novo}')")),
        vec![vec![Celula::Texto(novo.clone()), Celula::Texto("Descrição depois da chave".into())]]
    );
    assert_eq!(celula(&h, 0, 1), "Descrição depois da chave");
    assert!(!toast(&h).unwrap_or_default().contains("não foi encontrada"));
}

#[test]
fn coluna_apelido_do_rowid_abre_a_linha() {
    let amb = Ambiente::exemplo("dados_apelido");
    let mut h = abrir(&amb, "clientes");
    // id INTEGER PRIMARY KEY é o próprio rowid: sem uma coluna "rowid" repetindo os mesmos números
    assert_eq!(h.state().tela.doc.parte.tem_coluna_abrir(), Some(false));
    let abre = h.get_by_role_and_label(Role::Button, "1").rect().center();
    clicar(&mut h, egui::pos2(abre.x, abre.y));
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha 1"));
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.janela.is_none());
    // duplo clique na célula abre a janela uma vez só, e o segundo clique não a fecha
    duplo_clique_celula(&mut h, 1, 0);
    esperar_que(&mut h, |r| r.janela.is_some());
    h.run_steps(2);
    assert_eq!(retrato(&h).janela.as_deref(), Some("Linha 2"));
    tecla(&mut h, Key::Escape);

    // sem apelido, a coluna à parte continua, com o rótulo "rowid"
    let h = abrir(&amb, "eventos");
    assert_eq!(h.state().tela.doc.parte.tem_coluna_abrir(), Some(true));
    assert_eq!(h.state().tela.doc.parte.rotulo_da_linha(0).as_deref(), Some("1"));
}

#[test]
fn coluna_chamada_proto_e_linha_sem_chave() {
    let amb = Ambiente::exemplo("dados_proto");
    executar(
        &amb,
        "CREATE TABLE esquisita (\"__proto__\" TEXT, \"constructor\" TEXT); \
         INSERT INTO esquisita VALUES ('a', 'b'); \
         CREATE TABLE sem_apelido (rowid TEXT, oid TEXT, _rowid_ TEXT, codigo TEXT PRIMARY KEY, nome TEXT); \
         INSERT INTO sem_apelido VALUES ('r', 'o', '_', NULL, 'sem chave'), ('r', 'o', '_', 'K1', 'com chave');",
    );
    let mut h = abrir(&amb, "esquisita");
    let abre = h.get_by_role_and_label(Role::Button, "1").rect().center();
    clicar(&mut h, egui::pos2(abre.x, abre.y));
    esperar_que(&mut h, |r| r.janela.is_some());
    let id = id_campo(&h, "__proto__");
    preencher(&mut h, id, "mudado");
    clicar_botao(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela.is_none());
    assert_eq!(toast(&h).as_deref(), Some("Linha salva."));
    assert_eq!(
        ler(&h, "SELECT \"__proto__\", \"constructor\" FROM esquisita"),
        vec![vec![Celula::Texto("mudado".into()), Celula::Texto("b".into())]]
    );
    // linha nova: os campos nascem vazios
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.is_some());
    assert_eq!(h.state().tela.doc.parte.texto_campo_janela("__proto__").as_deref(), Some(""));
    assert_eq!(h.state().tela.doc.parte.texto_campo_janela("constructor").as_deref(), Some(""));
    tecla(&mut h, Key::Escape);

    // todos os nomes do rowid tomados por colunas: a chave é a PK, e a linha com PK nula não tem chave
    let mut h = abrir(&amb, "sem_apelido");
    let l = (0..2).find(|&l| celula(&h, l, 4) == "sem chave").expect("linha sem chave");
    let caixa = h.get_all_by_role_and_label(Role::CheckBox, "Marcar a linha abrir").nth(l).expect("caixa");
    assert!(desabilitado(&caixa), "linha sem chave não se marca");
    duplo_clique_celula(&mut h, l, 4);
    esperar_que(&mut h, |r| r.janela.is_some());
    let r = retrato(&h);
    assert!(r.janela_aviso.is_some_and(|a| a.starts_with("Esta linha não tem chave")));
    assert_eq!(h.state().tela.doc.parte.linhas_leitura_janela(), Some(5), "só leitura, sem campos");
    assert!(h.query_by_role_and_label(Role::Button, "Salvar").is_none());
}

#[test]
fn tab_na_ponta_da_grade_sai_dela() {
    let amb = Ambiente::exemplo("dados_tab_ponta");
    let mut tabela = amb.tabela("eventos");
    tabela.rota = tabela.rota.com_dados(ConsultaDados { por_pagina: 50, ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    let ultima_col = h.state().tela.doc.parte.colunas_da_grade().len() - 1;
    // a última célula da página: rola até ela pelo teclado
    clicar_celula(&mut h, 0, ultima_col);
    for _ in 0..5 {
        tecla(&mut h, Key::PageDown);
    }
    assert_eq!(retrato(&h).ativa, Some((49, ultima_col)));
    tecla(&mut h, Key::Enter);
    assert!(retrato(&h).editando.is_some());
    tecla(&mut h, Key::Tab);
    h.run_steps(2);
    // o Tab segue a página, em vez de voltar para a primeira célula da mesma linha
    assert!(retrato(&h).editando.is_none());
    assert!(!grade_com_foco(&h), "o foco saiu da grade");
    assert_eq!(retrato(&h).ativa, Some((49, ultima_col)));
}

#[test]
fn ctrl_home_e_ctrl_end_vao_a_primeira_e_a_ultima_linha() {
    let amb = Ambiente::exemplo("dados_ctrl_home_end");
    let mut tabela = amb.tabela("eventos");
    tabela.rota = tabela.rota.com_dados(ConsultaDados { por_pagina: 50, ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    clicar_celula(&mut h, 3, 2);
    tecla_com(&mut h, egui::Modifiers::COMMAND, Key::End);
    assert_eq!(retrato(&h).ativa, Some((49, 2)), "Ctrl+End: a última linha da página, na mesma coluna");
    tecla_com(&mut h, egui::Modifiers::COMMAND, Key::Home);
    assert_eq!(retrato(&h).ativa, Some((0, 2)), "Ctrl+Home: a primeira linha, na mesma coluna");
    tecla(&mut h, Key::End);
    assert_eq!(retrato(&h).ativa.map(|(l, _)| l), Some(0), "End sozinho continua na linha");
}

#[test]
fn hint_sem_linhas() {
    let amb = Ambiente::exemplo("dados_hint_vazio");
    let mut tabela = amb.tabela("clientes");
    tabela.rota =
        tabela.rota.com_dados(ConsultaDados { busca: "nada-assim@example.test".into(), ..Default::default() });
    let h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    assert_eq!(retrato(&h).total, linhas_de(0, N_CLIENTES as u64));
}

#[test]
fn esc_na_busca_limpa_e_enter_aplica_na_hora() {
    let amb = Ambiente::exemplo("dados_esc_busca");
    let mut tabela = amb.tabela("clientes");
    tabela.rota =
        tabela.rota.com_dados(ConsultaDados { busca: "nada-assim@example.test".into(), ..Default::default() });
    let mut h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    assert_eq!(retrato(&h).total, linhas_de(0, N_CLIENTES as u64));
    let id_busca = campo_busca();
    h.ctx.memory_mut(|m| m.request_focus(id_busca));
    h.step();
    h.step();
    // o Esc do campo de busca limpa a busca (como o `input[type=search]`)
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.total == linhas(N_CLIENTES as u64) && !r.carregando);
    assert_eq!(ultima_rota(&h).map(|c| c.busca), Some(String::new()));
    // Enter aplica sem esperar os 300 ms
    h.ctx.memory_mut(|m| m.request_focus(id_busca));
    h.step();
    h.input_mut().events.extend([
        egui::Event::Text("Recife".into()),
        egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    h.step();
    assert_eq!(ultima_rota(&h).map(|c| c.busca), Some("Recife".into()), "aplicada no mesmo quadro");
}

#[test]
fn pagina_que_falha_mostra_o_aviso_na_grade() {
    let amb = Ambiente::exemplo("dados_pagina_falha");
    let ganchos = Ganchos {
        pagina: Some(Arc::new(|pedido: &banco::PedidoPagina, _: &mut banco::Pagina| {
            if pedido.pagina > 1 {
                panic!("página quebrada de propósito");
            }
        })),
        ..Ganchos::default()
    };
    let mut h = abrir_com(&amb, amb.tabela("eventos"), Tema::Claro, ganchos);
    clicar_botao(&mut h, "Próxima página");
    esperar_que(&mut h, |r| r.erro.is_some());
    let r = retrato(&h);
    assert_eq!(r.total, "", "a faixa fica sem contagem; o aviso fica no conteúdo");
    assert_eq!(r.erro.as_deref(), Some(estado::MSG_INESPERADO));
    assert!(r.contagem_linhas.is_empty() && r.pagina_atual.is_empty(), "o rodapé fica vazio");
    assert!(h.query_by_label_contains(estado::MSG_INESPERADO).is_some(), "o aviso de perigo na grade");
    // voltar à página que lê some com o aviso
    let mut tabela = amb.tabela("eventos");
    tabela.rota = h.state().tela.doc.tabela.rota.com_dados(ConsultaDados::default());
    let h = abrir_com(&amb, tabela, Tema::Claro, Ganchos::default());
    assert!(retrato(&h).erro.is_none());
}

#[test]
fn tabela_vazia_mostra_o_estado_vazio() {
    let amb = Ambiente::exemplo("dados_tabela_vazia");
    executar(&amb, "CREATE TABLE vazia (id INTEGER PRIMARY KEY, nome TEXT);");
    let mut h = abrir(&amb, "vazia");
    let r = retrato(&h);
    assert_eq!(r.vazio.as_deref(), Some("Nenhuma linha nesta tabela."));
    assert_eq!(r.contagem_linhas, "Nenhuma linha");
    assert_eq!(r.total, "0 linhas");
    // o botão do vazio (o de baixo; o da barra é o mesmo verbo) abre a ficha da linha nova
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.is_some());
}

#[test]
fn filtro_sem_linhas_oferece_limpar_os_filtros() {
    let amb = Ambiente::exemplo("dados_filtro_vazio");
    let mut h = abrir(&amb, "clientes");
    condicao_pelo_popover(&mut h, "nome", "igual a", Some("ninguém com este nome"));
    esperar_que(&mut h, |r| r.vazio.is_some() && !r.carregando);
    assert_eq!(retrato(&h).vazio.as_deref(), Some("Nenhuma linha corresponde à busca ou aos filtros."));
    assert!(h.query_by_label("nome").is_some(), "o cabeçalho continua à vista");
    // dois "Limpar filtros": o da faixa e o do vazio, embaixo
    assert_eq!(h.query_all_by_role_and_label(Role::Button, "Limpar filtros").count(), 2);
    clicar_botao(&mut h, "Limpar filtros");
    esperar_que(&mut h, |r| r.filtros == 0 && r.vazio.is_none() && !r.carregando);
}

// ---- correções da segunda revisão ---------------------------------------------------------------------------

#[test]
fn tecla_com_a_celula_ativa_fora_da_vista_espera_a_grade_rolar() {
    let amb = Ambiente::exemplo("dados_fora_da_vista");
    let mut h = abrir(&amb, "clientes");
    let nome_antes = texto(&ler_um(&h, "SELECT nome FROM clientes WHERE id = 1"));
    clicar_celula(&mut h, 0, 1);
    assert!(grade_com_foco(&h));
    // a roda do mouse leva a célula ativa para fora da vista; o foco continua na grade
    let sobre = centro_celula(&h, 5, 3);
    rolar_com_a_roda(&mut h, sobre, 900.0);
    assert!(h.state().tela.doc.parte.centro_celula(0, 1).is_none(), "a célula ativa saiu da vista");
    assert!(grade_com_foco(&h));

    // a tecla começa a edição, mas nada é gravado sem o campo aparecer: a grade rola de volta até a célula
    digitar(&mut h, "Z");
    let r = retrato(&h);
    assert_eq!(r.editando, Some((0, 1, "Z".into())));
    assert_eq!(r.gravacoes_pendentes, 0, "nenhuma gravação sem o campo ter aparecido");
    assert_eq!(texto(&ler_um(&h, "SELECT nome FROM clientes WHERE id = 1")), nome_antes);
    esperar(&mut h, |h| h.state().tela.doc.parte.centro_celula(0, 1).is_some() && !grade_com_foco(h));
    assert_eq!(retrato(&h).editando, Some((0, 1, "Z".into())), "o campo aparece com a tecla digitada");
    digitar(&mut h, "Y");
    assert_eq!(retrato(&h).editando, Some((0, 1, "ZY".into())));
    assert_eq!(texto(&ler_um(&h, "SELECT nome FROM clientes WHERE id = 1")), nome_antes);
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert_eq!(texto(&ler_um(&h, "SELECT nome FROM clientes WHERE id = 1")), "ZY");

    // Enter com a célula ativa fora da vista abre a edição e ela fica aberta (não fecha no quadro seguinte)
    rolar_com_a_roda(&mut h, sobre, 900.0);
    assert!(h.state().tela.doc.parte.centro_celula(0, 1).is_none());
    tecla(&mut h, Key::Enter);
    h.run_steps(3);
    assert_eq!(retrato(&h).editando, Some((0, 1, "ZY".into())));
    esperar(&mut h, |h| h.state().tela.doc.parte.centro_celula(0, 1).is_some() && !grade_com_foco(h));
    tecla(&mut h, Key::Escape);
    assert!(retrato(&h).editando.is_none());
    assert_eq!(retrato(&h).gravacoes_pendentes, 0);
}

/// Uma tabela WITHOUT ROWID com a chave em `k`: as linhas A1 (um) e B2 (dois).
fn tabela_de_codigos(amb: &Ambiente) {
    executar(
        amb,
        "CREATE TABLE codigos (k TEXT PRIMARY KEY, d TEXT) WITHOUT ROWID; \
         INSERT INTO codigos VALUES ('A1', 'um'), ('B2', 'dois');",
    );
}

fn codigos(h: &H) -> Vec<(String, String)> {
    ler(h, "SELECT k, d FROM codigos ORDER BY k").into_iter().map(|l| (texto(&l[0]), texto(&l[1]))).collect()
}

#[test]
fn chave_que_volta_a_existir_nao_leva_a_gravacao_para_outra_linha() {
    let amb = Ambiente::exemplo("dados_chave_reusada");
    tabela_de_codigos(&amb);
    let mut h = abrir(&amb, "codigos");
    assert_eq!((celula(&h, 0, 0), celula(&h, 1, 0)), ("A1".into(), "B2".into()));
    // A1 → A1X numa linha, B2 → A1 na outra, e a descrição da que agora é A1
    for (l, c, novo) in [(0, 0, "A1X"), (1, 0, "A1"), (1, 1, "mudou")] {
        duplo_clique_celula(&mut h, l, c);
        preencher_edicao(&mut h, novo);
        tecla(&mut h, Key::Enter);
        esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    }
    assert_eq!(codigos(&h), vec![("A1".into(), "mudou".into()), ("A1X".into(), "um".into())]);
    assert_eq!(celula(&h, 1, 1), "mudou");
    assert_eq!(celula(&h, 0, 1), "um");
}

#[test]
fn chaves_trocadas_na_fila_seguem_cada_linha() {
    let amb = Ambiente::exemplo("dados_chave_reusada_fila");
    tabela_de_codigos(&amb);
    let mut h = abrir_com(&amb, amb.tabela("codigos"), Tema::Claro, gravacoes_lentas(0.3));
    // as três saem antes da primeira resposta: cada uma espera a anterior
    duplo_clique_celula(&mut h, 0, 0);
    preencher_edicao(&mut h, "A1X");
    tecla(&mut h, Key::Enter);
    tecla(&mut h, Key::ArrowDown);
    tecla(&mut h, Key::Enter);
    preencher_edicao(&mut h, "A1");
    tecla(&mut h, Key::Enter);
    tecla(&mut h, Key::ArrowRight);
    tecla(&mut h, Key::Enter);
    preencher_edicao(&mut h, "mudou");
    tecla(&mut h, Key::Enter);
    assert!(retrato(&h).gravacoes_pendentes >= 2, "as gravações estão na fila");
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0);
    assert_eq!(codigos(&h), vec![("A1".into(), "mudou".into()), ("A1X".into(), "um".into())]);
    assert!(!toast(&h).unwrap_or_default().contains("não foi encontrada"));
}

#[test]
fn resposta_de_gravacao_lenta_nao_tira_o_foco_da_edicao_seguinte() {
    let amb = Ambiente::exemplo("dados_resposta_lenta");
    let mut h = abrir_com(&amb, amb.tabela("clientes"), Tema::Claro, gravacoes_lentas(0.5));
    duplo_clique_celula(&mut h, 0, 1);
    preencher_edicao(&mut h, "Primeiro");
    tecla(&mut h, Key::Enter);
    assert_eq!(retrato(&h).gravacoes_pendentes, 1);
    // a pessoa segue antes da resposta: desce e digita na célula de baixo
    tecla(&mut h, Key::ArrowDown);
    digitar(&mut h, "xy");
    assert_eq!(retrato(&h).editando, Some((1, 1, "xy".into())));
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0);
    // a resposta da primeira chegou: a edição de baixo continua aberta e com o foco
    let r = retrato(&h);
    assert_eq!(r.editando, Some((1, 1, "xy".into())), "a resposta não fecha a edição seguinte");
    assert_eq!(r.ativa, Some((1, 1)), "a célula ativa não volta para a de cima");
    assert!(!grade_com_foco(&h), "o foco fica no campo");
    digitar(&mut h, "z");
    assert_eq!(retrato(&h).editando, Some((1, 1, "xyz".into())));
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert_eq!(
        ler(&h, "SELECT nome FROM clientes WHERE id IN (1, 2) ORDER BY id"),
        vec![vec![Celula::Texto("Primeiro".into())], vec![Celula::Texto("xyz".into())]]
    );
    assert!(grade_com_foco(&h));
    assert_eq!(retrato(&h).ativa, Some((1, 1)));
}

#[test]
fn chip_do_blob_na_janela_tem_a_dica_de_la() {
    let amb = Ambiente::exemplo("dados_dica_blob");
    let mut h = abrir(&amb, "produtos");
    dicas_sem_demora(&mut h);
    duplo_clique_celula(&mut h, 2, 4);
    esperar_que(&mut h, |r| r.janela.is_some());
    let chip = h.get_by_label("112 bytes").rect().center();
    pairar(&mut h, egui::pos2(chip.x, chip.y));
    let hex = match ler_um(&h, "SELECT lower(hex(substr(foto, 1, 4))) FROM produtos WHERE codigo = 'P003'") {
        Celula::Texto(t) => t,
        outro => panic!("{outro:?}"),
    };
    assert!(
        h.query_by_label_contains(&format!("Início em hexadecimal: {hex}")).is_some(),
        "a dica do chip começa pelos bytes do BLOB"
    );
}

use std::sync::Arc;
use std::time::Duration;

// ---- etapa 3: o documento (faixa, popover, rodapé, verbos, Propriedades) --------------------------------------

/// clientes com a condição "ativo igual a 1" já no endereço.
fn clientes_ativos(amb: &Ambiente) -> estado::TabelaAberta {
    let mut tabela = amb.tabela("clientes");
    let filtro = FiltroDigitado { coluna: "ativo".into(), op: Operador::Igual, valor: "1".into() };
    tabela.rota = tabela.rota.com_dados(ConsultaDados { filtros: vec![filtro], ..Default::default() });
    tabela
}

#[test]
fn faixa_mostra_as_condicoes_e_a_contagem() {
    let amb = Ambiente::exemplo("dados_faixa");
    let h = abrir_com(&amb, clientes_ativos(&amb), Tema::Claro, Ganchos::default());
    let n = contar(&h, "SELECT count(*) FROM clientes WHERE ativo = 1");
    assert_eq!(retrato(&h).total, linhas_de(n, N_CLIENTES as u64), "\"41 de 60 linhas\"");
    assert!(h.query_by_label("Filtros").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro ativo igual a 1").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Tirar o filtro ativo igual a 1").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Nova condição").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Limpar filtros").is_some());
    // sem condição, só "Filtros", "+ Condição" e a contagem
    let h = abrir(&amb, "clientes");
    assert_eq!(retrato(&h).total, "60 linhas");
    assert!(h.query_by_role_and_label(Role::Button, "Limpar filtros").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Tirar o filtro ativo igual a 1").is_none());
}

#[test]
fn condicao_pelo_popover_vira_etiqueta() {
    let amb = Ambiente::exemplo("dados_popover");
    let mut h = abrir(&amb, "clientes");
    condicao_pelo_popover(&mut h, "cidade", "igual a", Some("Recife"));
    let n = contar(&h, "SELECT count(*) FROM clientes WHERE cidade = 'Recife'");
    esperar_que(&mut h, |r| !r.carregando && !r.condicao_aberta && r.filtros == 1);
    assert_eq!(retrato(&h).total, linhas_de(n, N_CLIENTES as u64));
    assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro cidade igual a Recife").is_some());
    let rota = ultima_rota(&h).expect("rota");
    assert_eq!(
        rota.filtros,
        vec![FiltroDigitado { coluna: "cidade".into(), op: Operador::Igual, valor: "Recife".into() }]
    );

    // o clique na etiqueta abre a mesma condição no popover; trocar o valor e Enter aplica
    clicar_botao(&mut h, "Editar o filtro cidade igual a Recife");
    assert!(retrato(&h).condicao_aberta);
    let campo = h.get_by_role_and_label(Role::TextInput, "Valor da condição");
    assert_eq!(campo.value().as_deref(), Some("Recife"));
    let c = campo.rect().center();
    clicar(&mut h, egui::pos2(c.x, c.y));
    tecla_com(&mut h, CTRL, Key::A);
    digitar(&mut h, "Curitiba");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| !r.condicao_aberta && !r.carregando);
    assert_eq!(retrato(&h).filtros, 1, "trocou a condição, sem outra");
    assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro cidade igual a Curitiba").is_some());

    // Esc desiste sem mudar nada
    clicar_botao(&mut h, "Nova condição");
    assert!(retrato(&h).condicao_aberta);
    digitar(&mut h, "x");
    tecla(&mut h, Key::Escape);
    assert!(!retrato(&h).condicao_aberta);
    assert_eq!(retrato(&h).filtros, 1);
    // sem valor, o Aplicar fica desligado; o leitor de tela ouve o nome do popover
    clicar_botao(&mut h, "Nova condição");
    assert!(desabilitado(&h.get_by_role_and_label(Role::Button, "Aplicar")));
    assert!(h.query_by_role_and_label(Role::Dialog, "Condição de filtro").is_some());
    clicar_botao(&mut h, "Cancelar");
    assert!(!retrato(&h).condicao_aberta);
}

#[test]
fn tirar_a_etiqueta_tira_o_filtro() {
    let amb = Ambiente::exemplo("dados_tirar_etiqueta");
    let mut h = abrir_com(&amb, clientes_ativos(&amb), Tema::Claro, Ganchos::default());
    clicar_botao(&mut h, "Tirar o filtro ativo igual a 1");
    esperar_que(&mut h, |r| r.filtros == 0 && !r.carregando);
    assert_eq!(retrato(&h).total, "60 linhas");
    assert_eq!(ultima_rota(&h).map(|c| c.filtros.len()), Some(0));
    assert!(h.query_by_role_and_label(Role::Button, "Editar o filtro ativo igual a 1").is_none());
}

#[test]
fn limpar_filtros_pela_faixa() {
    let amb = Ambiente::exemplo("dados_limpar_faixa");
    let mut h = abrir_com(&amb, clientes_ativos(&amb), Tema::Claro, Ganchos::default());
    condicao_pelo_popover(&mut h, "limite", "não é nulo", None);
    esperar_que(&mut h, |r| r.filtros == 2 && !r.carregando);
    clicar_botao(&mut h, "Limpar filtros");
    esperar_que(&mut h, |r| r.filtros == 0 && !r.carregando);
    assert_eq!(retrato(&h).total, "60 linhas");
    assert!(h.query_by_role_and_label(Role::Button, "Limpar filtros").is_none());
}

#[test]
fn rodape_com_linhas_e_marcadas() {
    let amb = Ambiente::exemplo("dados_rodape");
    let mut h = abrir(&amb, "clientes");
    assert!(h.query_by_label("Linhas 1–60 de 60").is_some());
    assert!(h.query_by_label("Página 1 de 1").is_some());
    let por_pagina = h.get_by_role_and_label(Role::ComboBox, "Linhas por página").rect();
    assert_eq!(por_pagina.height(), 26.0, "o select de 26px do rodapé");
    let anterior = h.get_by_role_and_label(Role::Button, "Página anterior");
    assert!(desabilitado(&anterior));
    assert_eq!(anterior.rect().width(), 26.0, "o .ib.pq");
    clicar_marca(&mut h, Some(0));
    clicar_marca(&mut h, Some(2));
    assert!(h.query_by_label("· 2 marcadas").is_some());
    // o rodapé fica embaixo do documento, com 34px
    let r = h.get_by_label("Linhas 1–60 de 60").rect();
    assert!(r.bottom() > TAMANHO.y - 34.0, "{r:?}");
}

#[test]
fn nova_linha_pela_barra() {
    let amb = Ambiente::exemplo("dados_nova_barra");
    let mut h = abrir(&amb, "clientes");
    assert_eq!(ofertas(&h).rotulo(estado::Verbo::Novo), "Nova linha");
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.as_deref() == Some("Nova linha"));
}

#[test]
fn duplicar_a_linha_da_celula_ativa() {
    let amb = Ambiente::exemplo("dados_duplicar");
    let mut h = abrir(&amb, "clientes");
    clicar_celula(&mut h, 2, 1);
    assert_eq!(ofertas(&h).dica(estado::Verbo::Duplicar), "Duplicar a linha 3");
    clicar_botao(&mut h, "Duplicar");
    esperar_que(&mut h, |r| r.janela.as_deref() == Some("Nova linha"));
    let nome = texto(&ler_um(&h, "SELECT nome FROM clientes WHERE id = 3"));
    let aba = &h.state().tela.doc.parte;
    assert_eq!(aba.texto_campo_janela("nome"), Some(nome));
    assert_eq!(aba.caixas_campo_janela("id").and_then(|c| c.1), Some(true), "o id volta a ser automático");
    // com uma marcada só, ela é o alvo
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.janela.is_none());
    clicar_marca(&mut h, Some(5));
    assert_eq!(ofertas(&h).dica(estado::Verbo::Duplicar), "Duplicar a linha 6");
}

#[test]
fn duplicar_desligado_com_varias_marcadas() {
    let amb = Ambiente::exemplo("dados_duplicar_varias");
    let mut h = abrir(&amb, "clientes");
    clicar_marca(&mut h, Some(0));
    clicar_marca(&mut h, Some(1));
    let o = ofertas(&h);
    assert!(!o.ligado(estado::Verbo::Duplicar));
    assert_eq!(o.dica(estado::Verbo::Duplicar), "Duplicar (marque uma linha só)");
    assert!(h.query_by_role_and_label(Role::Button, "Duplicar").is_none(), "a barra do palco só mostra os ligados");
}

#[test]
fn excluir_a_linha_da_celula_ativa_pergunta() {
    let amb = Ambiente::exemplo("dados_excluir_ativa");
    let mut h = abrir(&amb, "eventos");
    clicar_celula(&mut h, 1, 1);
    assert_eq!(ofertas(&h).dica(estado::Verbo::Excluir), "Excluir a linha 2");
    clicar_botao(&mut h, "Excluir");
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Excluir 1 linha de eventos? Isso não pode ser desfeito."));
    clicar_botao(&mut h, "Excluir");
    esperar(&mut h, |h| toast(h).as_deref() == Some("1 linha excluída."));
    assert_eq!(contar(&h, "SELECT count(*) FROM eventos WHERE rowid = 2"), 0);
}

#[test]
fn excluir_as_marcadas_pela_barra() {
    let amb = Ambiente::exemplo("dados_excluir_barra");
    let mut h = abrir(&amb, "eventos");
    clicar_marca(&mut h, Some(0));
    clicar_marca(&mut h, Some(1));
    // com marcadas, o alvo são elas (não a célula ativa)
    clicar_celula(&mut h, 7, 1);
    assert_eq!(ofertas(&h).dica(estado::Verbo::Excluir), "Excluir as 2 linhas marcadas");
    clicar_botao(&mut h, "Excluir");
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Excluir 2 linhas de eventos? Isso não pode ser desfeito."));
    clicar_botao(&mut h, "Excluir");
    esperar_que(&mut h, |r| r.marcadas == 0 && r.confirmacao.is_none());
    esperar(&mut h, |h| contar(h, "SELECT count(*) FROM eventos WHERE rowid IN (1, 2, 8)") == 1);
}

#[test]
fn filtrar_marcado_com_condicao() {
    let amb = Ambiente::exemplo("dados_filtrar");
    let mut h = abrir(&amb, "clientes");
    let o = ofertas(&h);
    assert!(o.ligado(estado::Verbo::Filtrar) && !o.get(estado::Verbo::Filtrar).marcado);
    assert_eq!(o.dica(estado::Verbo::Filtrar), "Filtrar");
    // o Filtrar da barra abre o popover numa condição nova
    clicar_botao(&mut h, "Filtrar");
    assert!(retrato(&h).condicao_aberta);
    tecla(&mut h, Key::Escape);
    let h = abrir_com(&amb, clientes_ativos(&amb), Tema::Claro, Ganchos::default());
    let o = ofertas(&h);
    assert!(o.get(estado::Verbo::Filtrar).marcado, "o .ib.on com condição");
    assert_eq!(o.dica(estado::Verbo::Filtrar), "Filtrar (1 condição)");
}

#[test]
fn propriedades_seguem_a_celula_ativa() {
    let amb = Ambiente::exemplo("dados_propriedades");
    let mut h = abrir(&amb, "clientes");
    assert!(h.query_by_label("Seção Tabela").is_some());
    clicar_celula(&mut h, 2, 2);
    h.run_steps(2);
    let email = texto(&ler_um(&h, "SELECT email FROM clientes WHERE id = 3"));
    assert!(h.query_by_label("Seção Coluna").is_some());
    assert!(h.query_by_label(&email).is_some(), "o Valor da célula ativa");
    // o Tipo das Propriedades, além da pastilha do cabeçalho de cada coluna TEXT
    let n_text = h.query_all_by_label("TEXT").count();
    clicar_celula(&mut h, 2, 4);
    h.run_steps(2);
    assert_eq!(h.query_all_by_label("TEXT").count(), n_text - 1, "a coluna limite é REAL");
    assert_eq!(h.query_all_by_label("REAL").count(), 2, "a pastilha de limite e o Tipo");
    clicar_celula(&mut h, 2, 0);
    h.run_steps(2);
    assert!(h.query_by_label("CHAVE PRIMÁRIA").is_some() || h.query_by_label("chave primária").is_some());
}

#[test]
fn abrir_na_consulta_pelas_propriedades() {
    let amb = Ambiente::exemplo("dados_abrir_consulta");
    let mut h = abrir(&amb, "clientes");
    assert!(h.query_by_role_and_label(Role::Link, "SELECT count(*) FROM clientes").is_some());
    let c = h.get_by_role_and_label(Role::Link, "SELECT * FROM clientes LIMIT 100").rect().center();
    clicar(&mut h, egui::pos2(c.x, c.y));
    assert!(
        h.state().acoes.iter().any(|a| matches!(a, estado::Acao::Navegar(r)
            if *r == estado::Rota::consulta_com("SELECT * FROM clientes LIMIT 100"))),
        "{:?}",
        h.state().acoes
    );
}

#[test]
fn pastilha_no_cabecalho_e_o_tipo_na_dica() {
    let amb = Ambiente::exemplo("dados_pastilha");
    let mut h = abrir(&amb, "clientes");
    dicas_sem_demora(&mut h);
    assert_eq!(h.state().tela.doc.parte.tipo_da_coluna(0).as_deref(), Some("INTEGER · chave primária"));
    let pastilha = h.get_by_label("INTEGER · chave primária").rect();
    assert_eq!(pastilha.size(), egui::vec2(16.0, 16.0), "a pastilha de 16");
    let nome = h.get_by_role_and_label(Role::Button, "id").rect();
    assert!((nome.left() - pastilha.right() - 6.0).abs() < 1.0, "6px até o nome: {pastilha:?} {nome:?}");
    pairar(&mut h, pastilha.center());
    assert!(h.query_all_by_label("INTEGER · chave primária").count() >= 2, "o tipo por extenso na dica");
}

// ---- etapa 3: a ficha da linha (prancheta 12a e a só leitura, 15) ---------------------------------------------

/// Abre a ficha de uma linha de `produtos` pelo duplo clique na coluna gerada (que nunca edita no lugar).
fn abrir_ficha_de_produto(h: &mut H, linha: usize, titulo: &str) {
    duplo_clique_celula(h, linha, 6);
    esperar_que(h, |r| r.janela.is_some());
    assert_eq!(retrato(h).janela.as_deref(), Some(titulo));
}

fn valor_do_campo(h: &H, coluna: &str) -> Option<String> {
    h.get_by_role_and_label(Role::TextInput, coluna).value()
}

#[test]
fn ficha_com_pastilhas_e_descricoes() {
    let amb = Ambiente::exemplo("dados_ficha_pastilhas");
    let mut h = abrir(&amb, "produtos");
    abrir_ficha_de_produto(&mut h, 2, "Linha P003");
    // o cabeçalho: a ficha, o título e a tabela
    assert!(h.query_all_by_label("produtos").count() >= 1, "o sub com a tabela");
    // a coluna do nome: a linha da pastilha de 24 e, embaixo, a descrição recuada 32
    let nome = h.get_by_role_and_label(Role::Label, "codigo").rect();
    let descricao = h
        .query_all_by_role_and_label(Role::Label, "TEXT · chave primária")
        .map(|n| n.rect())
        .find(|r| r.width() > 100.0)
        .expect("a descrição");
    assert_eq!(nome.height(), 24.0, "a pastilha de 24");
    assert_eq!(nome.width(), 250.0, "a coluna do nome");
    assert!((descricao.left() - nome.left() - 32.0).abs() < 0.5, "o recuo de 32: {nome:?} {descricao:?}");
    assert!(descricao.top() > nome.bottom(), "a descrição fica embaixo do nome");
    assert!(h.query_all_by_label("INTEGER · gerada (VIRTUAL)").count() >= 1, "a gerada diz qual é");
    // as três colunas: o valor começa 12px depois do nome e a do NULL tem 64
    let campo = h.get_by_role_and_label(Role::TextInput, "codigo").rect();
    assert!((campo.left() - nome.right() - 12.0).abs() < 0.5, "{nome:?} {campo:?}");
    let caixa = h.get_by_role_and_label(Role::CheckBox, "estoque é NULL").rect();
    assert!(caixa.left() > campo.right(), "a caixa NULL é a coluna da direita");
    // o Cancelar e o Salvar no rodapé, o Excluir linha e o Duplicar à esquerda
    for botao in ["Excluir linha", "Duplicar", "Cancelar", "Salvar"] {
        assert!(h.query_all_by_role_and_label(Role::Button, botao).count() >= 1, "{botao}");
    }
    let (cancelar, salvar) = (
        h.get_by_role_and_label(Role::Button, "Cancelar").rect(),
        h.get_by_role_and_label(Role::Button, "Salvar").rect(),
    );
    assert!(cancelar.right() < salvar.left(), "Cancelar antes do Salvar");
    clicar_botao(&mut h, "Cancelar");
    esperar_que(&mut h, |r| r.janela.is_none());
}

#[test]
fn ficha_null_marcado_mostra_null_sem_esmaecer() {
    let amb = Ambiente::exemplo("dados_ficha_null");
    executar(&amb, "UPDATE produtos SET serie = NULL WHERE codigo = 'P003'");
    let mut h = abrir(&amb, "produtos");
    abrir_ficha_de_produto(&mut h, 2, "Linha P003");
    // a série de P003 ficou NULL: a caixa marcada, o campo normal (sem o 70% do campo desligado) e vazio, com "NULL"
    // em itálico no lugar do valor
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("serie").and_then(|c| c.0), Some(true));
    assert_eq!(valor_do_campo(&h, "serie").as_deref(), Some(""));
    let caixa = h.get_by_role_and_label(Role::CheckBox, "serie é NULL");
    assert_eq!(caixa.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    // desmarcar devolve o campo, que recebe o foco, e o valor digitado grava
    clicar_caixa_do_campo(&mut h, "serie", "NULL");
    h.run_steps(3);
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("serie").and_then(|c| c.0), Some(false));
    digitar(&mut h, "77");
    assert_eq!(valor_do_campo(&h, "serie").as_deref(), Some("77"));
    clicar_botao(&mut h, "Salvar");
    esperar_que(&mut h, |r| r.janela.is_none() && r.gravacoes_pendentes == 0);
    assert_eq!(inteiro(&ler_um(&h, "SELECT serie FROM produtos WHERE codigo = 'P003'")), 77);
}

#[test]
fn ficha_campo_so_leitura_sem_esmaecer() {
    let amb = Ambiente::exemplo("dados_ficha_gerada");
    let mut h = abrir(&amb, "produtos");
    abrir_ficha_de_produto(&mut h, 0, "Linha P001");
    let gerada = h.get_by_role_and_label(Role::TextInput, "preco_centavos");
    let valor = gerada.value();
    assert!(valor.as_deref().is_some_and(|v| !v.is_empty()), "o valor calculado");
    assert!(!desabilitado(&gerada), "só leitura não é o campo desligado");
    // a gerada não tem a caixa NULL
    assert!(h.query_by_role_and_label(Role::CheckBox, "preco_centavos é NULL").is_none());
    let c = gerada.rect().center();
    clicar(&mut h, egui::pos2(c.x, c.y));
    digitar(&mut h, "123");
    assert_eq!(valor_do_campo(&h, "preco_centavos"), valor);
}

#[test]
fn ficha_blob_salvar_e_substituir() {
    let amb = Ambiente::exemplo("dados_ficha_blob");
    let mut h = abrir(&amb, "produtos");
    abrir_ficha_de_produto(&mut h, 2, "Linha P003");
    assert!(h.query_by_label("112 bytes").is_some(), "a pílula com o tamanho");
    let (salvar, trocar) = (
        h.get_by_role_and_label(Role::Button, "Salvar em arquivo").rect(),
        h.get_by_role_and_label(Role::Button, "Substituir por arquivo").rect(),
    );
    assert_eq!((salvar.height(), trocar.height()), (26.0, 26.0), "os btn pq de 26px da janela");
    assert!(h.query_by_role_and_label(Role::CheckBox, "foto é NULL").is_some());
    assert!(
        h.query_all_by_role(Role::Button).all(|b| !b.accesskit_node().label().is_some_and(|l| l.ends_with('…'))),
        "nenhum botão com reticências"
    );
    tecla(&mut h, Key::Escape);
    esperar_que(&mut h, |r| r.janela.is_none());
    // sem BLOB, o botão é o "Escolher arquivo"
    abrir_ficha_de_produto(&mut h, 4, "Linha P005");
    assert!(h.query_by_role_and_label(Role::Button, "Escolher arquivo").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Salvar em arquivo").is_none(), "sem BLOB, nada a salvar");
}

#[test]
fn ficha_valores_crus_como_na_grade() {
    let amb = Ambiente::exemplo("dados_ficha_crus");
    executar(&amb, "CREATE TABLE medidas (id INTEGER PRIMARY KEY, valor REAL, nota TEXT);");
    executar(&amb, "INSERT INTO medidas VALUES (1, 2500.0, 'a'), (2, 18.9, 'b');");
    let mut h = abrir(&amb, "medidas");
    assert_eq!(celula(&h, 0, 1), "2500", "a grade");
    clicar_botao(&mut h, "1");
    esperar_que(&mut h, |r| r.janela.is_some());
    // 2500.0 vira "2500" na ficha, como na grade: ninguém formata em pt-BR
    assert_eq!(valor_do_campo(&h, "valor").as_deref(), Some("2500"));
    let campo = h.get_by_role_and_label(Role::TextInput, "valor").rect();
    let nota = h.get_by_role_and_label(Role::TextInput, "nota").rect();
    assert!((campo.width() - nota.width()).abs() < 0.5, "o número mantém a largura da coluna");
}

#[test]
fn ficha_so_leitura_da_visao() {
    let amb = Ambiente::exemplo("dados_ficha_visao");
    let mut h = abrir(&amb, "v_pedidos_por_cliente");
    duplo_clique_celula(&mut h, 0, 1);
    esperar_que(&mut h, |r| r.janela.is_some());
    assert!(h.query_by_label("v_pedidos_por_cliente · só leitura").is_some(), "o sub diz só leitura");
    // os campos opacos, sem bevel: só leitura; a coluna NULL fica, com as caixas desligadas
    let campo = h.get_by_role_and_label(Role::TextInput, "nome");
    assert!(!desabilitado(&campo), "só leitura não esmaece");
    let caixa = h.get_by_role_and_label(Role::CheckBox, "nome é NULL");
    assert!(desabilitado(&caixa), "o NULL não se marca");
    // o rodapé: Copiar a linha e Fechar
    assert!(h.query_by_role_and_label(Role::Button, "Copiar a linha").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Excluir linha").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Cancelar").is_none());
    clicar_botao(&mut h, "Copiar a linha");
    assert_eq!(toast(&h).as_deref(), Some("Copiado."));
    assert!(retrato(&h).janela.is_some(), "copiar não fecha");
    clicar_botao(&mut h, "Fechar");
    esperar_que(&mut h, |r| r.janela.is_none());
}

#[test]
fn ficha_so_leitura_banco_so_leitura() {
    // o exemplo, com a série de P003 em NULL, reaberto só para leitura
    let mut amb = Ambiente::exemplo("dados_ficha_banco_so_leitura");
    executar(&amb, "UPDATE produtos SET serie = NULL WHERE codigo = 'P003'");
    let caminho = amb.caminho_banco();
    amb.servico.fechar_banco(&amb.estado.banco_id(), true).expect("fechar");
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só para leitura");
    amb.recarregar();
    let mut h = abrir(&amb, "produtos");
    abrir_ficha_de_produto(&mut h, 2, "Linha P003");
    // a nota compacta e centrada, e a ficha só leitura
    let frase = "exemplo.db está aberto só para leitura. Nada é gravado no arquivo.";
    assert_eq!(retrato(&h).janela_aviso.as_deref(), Some(frase));
    assert!(h.query_by_label("produtos · só leitura").is_some());
    // o BLOB fica só com o Salvar em arquivo
    assert!(h.query_by_role_and_label(Role::Button, "Salvar em arquivo").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Substituir por arquivo").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Escolher arquivo").is_none());
    assert!(desabilitado(&h.get_by_role_and_label(Role::CheckBox, "estoque é NULL")));
    // o NULL marcado da série aparece como "NULL" no campo, com a caixa marcada e desligada
    let serie = h.get_by_role_and_label(Role::CheckBox, "serie é NULL");
    assert_eq!(serie.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    assert!(desabilitado(&serie));
    assert_eq!(valor_do_campo(&h, "serie").as_deref(), Some(""));
}

#[test]
fn nova_linha_usa_o_padrao_na_celula() {
    let amb = Ambiente::exemplo("dados_ficha_padrao");
    let mut h = abrir(&amb, "clientes");
    clicar_botao(&mut h, "Nova linha");
    esperar_que(&mut h, |r| r.janela.as_deref() == Some("Nova linha"));
    // "Usar o padrão" fica dentro da célula do valor, à direita do campo, na mesma linha
    let cidade = h.get_by_role_and_label(Role::TextInput, "cidade").rect();
    let nome = h.get_by_role_and_label(Role::TextInput, "nome").rect();
    let usar: Vec<egui::Rect> = h
        .query_all_by_label("Usar o padrão")
        .map(|n| n.rect())
        .filter(|r| (r.center().y - cidade.center().y).abs() < 4.0)
        .collect();
    assert_eq!(usar.len(), 1, "uma caixa na linha de cidade: {usar:?}");
    assert!(usar[0].left() >= cidade.right(), "à direita do campo: {:?} {cidade:?}", usar[0]);
    assert!(usar[0].right() <= nome.right() + 0.5, "dentro da célula do valor");
    assert!(cidade.width() < nome.width(), "o campo cede o lugar à caixa");
    // a coluna do NULL continua à direita da célula do valor
    let caixa_nulo = h.get_by_role_and_label(Role::CheckBox, "cidade é NULL").rect();
    assert!(caixa_nulo.left() > usar[0].right());
    // o campo travado mostra o padrão como dica e o desmarcar libera
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("cidade").and_then(|c| c.1), Some(true));
    clicar_caixa_do_campo(&mut h, "cidade", "Usar o padrão");
    h.run_steps(3);
    assert_eq!(h.state().tela.doc.parte.caixas_campo_janela("cidade").and_then(|c| c.1), Some(false));
}

#[test]
fn rotulos_das_caixas_dizem_a_coluna() {
    let amb = Ambiente::exemplo("dados_ficha_rotulos");
    let mut h = abrir(&amb, "clientes");
    clicar_botao(&mut h, "1");
    esperar_que(&mut h, |r| r.janela.is_some());
    // "limite é NULL", "cidade é NULL"…: o nome da coluna no leitor de tela, com o "NULL" na tela
    assert!(h.query_by_role_and_label(Role::CheckBox, "limite é NULL").is_some());
    assert!(h.query_by_role_and_label(Role::CheckBox, "cidade é NULL").is_some());
    assert_eq!(h.query_all_by_role_and_label(Role::CheckBox, "NULL").count(), 0, "nenhuma caixa só com NULL");
    // a PK e as colunas NOT NULL não têm a caixa
    assert!(h.query_by_role_and_label(Role::CheckBox, "id é NULL").is_none());
    assert!(h.query_by_role_and_label(Role::CheckBox, "nome é NULL").is_none());
}

/// Edita a cidade do cliente 1 pela grade (com o valor novo) e espera a gravação.
fn editar_cidade(h: &mut H, valor: &str) {
    duplo_clique_celula(h, 0, 3);
    preencher_edicao(h, valor);
    tecla(h, Key::Enter);
    esperar_que(h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    h.run_steps(1);
}

fn cidade_do_1(h: &H) -> String {
    texto(&ler_um(h, "SELECT cidade FROM clientes WHERE id = 1"))
}

/// Desfazer no banco (prancheta 15, revisão de UI de 19/10/2026): o toast da gravação diz onde gravou e oferece
/// "Desfazer · Ctrl+Z"; o Desfazer da barra (o Ctrl+Z da casca) e o do toast gravam o valor de antes; a aba refeita
/// continua podendo desfazer; a célula mudada depois, a transação aberta depois, a estrutura mudada e a exclusão
/// desligam o Desfazer (com o porquê na dica).
#[test]
fn desfazer_a_ultima_edicao_de_celula() {
    use estado::Verbo;
    use interface::pecas::toast::rotulo_da_acao_do_toast;
    let amb = Ambiente::exemplo("dados_desfazer");
    let mut h = abrir(&amb, "clientes");
    assert!(!ofertas(&h).ligado(Verbo::Desfazer), "nada gravado, nada a desfazer");
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer (Ctrl+Z)");
    let original = cidade_do_1(&h);

    editar_cidade(&mut h, "Natal");
    assert_eq!(toast(&h).as_deref(), Some("Gravado em clientes (linha 1, cidade)"));
    assert_eq!(rotulo_da_acao_do_toast(&h.ctx).as_deref(), Some(tela_dados::ROTULO_ACAO));
    assert!(ofertas(&h).ligado(Verbo::Desfazer));
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer cidade da linha 1 (Ctrl+Z)");

    // o Desfazer da barra grava o valor de antes, a célula volta e pisca; não há Refazer
    clicar_botao(&mut h, "Desfazer");
    esperar_que(&mut h, |r| !r.desfazendo);
    h.run_steps(1);
    assert_eq!(cidade_do_1(&h), original);
    assert_eq!(celula(&h, 0, 3), original);
    assert!(h.state().tela.doc.parte.piscando(0, 3));
    assert_eq!(toast(&h).as_deref(), Some("Edição desfeita em clientes (linha 1, cidade)"));
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));

    // o botão do toast faz o mesmo, também depois de a aba ser refeita (o F5)
    editar_cidade(&mut h, "Recife");
    clicar_botao(&mut h, tela_dados::ROTULO_ACAO);
    esperar_que(&mut h, |r| !r.desfazendo);
    assert_eq!(cidade_do_1(&h), original);
    editar_cidade(&mut h, "Recife");
    let mut h = recarregar(&amb, h);
    assert!(ofertas(&h).ligado(Verbo::Desfazer), "a última edição atravessa a aba refeita");
    clicar_botao(&mut h, "Desfazer");
    esperar_que(&mut h, |r| !r.desfazendo);
    assert_eq!(cidade_do_1(&h), original);

    // a célula mudou depois da edição (a Consulta SQL, outro programa): nada volta
    editar_cidade(&mut h, "Natal");
    executar(&amb, "UPDATE clientes SET cidade = 'Manaus' WHERE id = 1");
    clicar_botao(&mut h, "Desfazer");
    esperar_que(&mut h, |r| !r.desfazendo);
    h.run_steps(1);
    assert_eq!(toast(&h).as_deref(), Some(banco::MSG_CELULA_MUDOU));
    assert_eq!(cidade_do_1(&h), "Manaus");

    // uma transação da Consulta SQL que começou depois da edição desliga o Desfazer
    editar_cidade(&mut h, "Belém");
    executar(&amb, "BEGIN");
    let marcar_transacao = |h: &mut H, aberta: bool| {
        h.state_mut().estado.banco.as_mut().expect("banco aberto").transacao_aberta = aberta;
        h.run_steps(1);
    };
    marcar_transacao(&mut h, true);
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), tela_dados::DICA_TRANSACAO);
    // dentro dela, a edição e o Desfazer entram na transação
    editar_cidade(&mut h, "Recife");
    assert!(ofertas(&h).ligado(Verbo::Desfazer));
    clicar_botao(&mut h, "Desfazer");
    esperar_que(&mut h, |r| !r.desfazendo);
    assert_eq!(cidade_do_1(&h), "Belém");
    executar(&amb, "ROLLBACK");
    marcar_transacao(&mut h, false);

    // mudar a estrutura não se desfaz: a tela relida desliga o Desfazer
    let mut h = recarregar(&amb, h);
    editar_cidade(&mut h, "Natal");
    executar(&amb, "ALTER TABLE clientes ADD COLUMN apelido TEXT");
    let mut h = recarregar(&amb, h);
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), tela_dados::DICA_ESTRUTURA);

    // banco só leitura: sem Desfazer
    editar_cidade(&mut h, "Recife");
    assert!(ofertas(&h).ligado(Verbo::Desfazer));
    h.state_mut().estado.banco.as_mut().expect("banco aberto").somente_leitura = true;
    h.run_steps(1);
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));
    h.state_mut().estado.banco.as_mut().expect("banco aberto").somente_leitura = false;

    // excluir linhas também não se desfaz: o Desfazer desliga e diz por quê
    trocar_aba(&mut h, amb.tabela("eventos"));
    esperar_grade(&mut h);
    assert!(!ofertas(&h).ligado(Verbo::Desfazer), "a última edição é de outra tabela");
    duplo_clique_celula(&mut h, 0, 1);
    preencher_edicao(&mut h, "teste");
    tecla(&mut h, Key::Enter);
    esperar_que(&mut h, |r| r.gravacoes_pendentes == 0 && r.editando.is_none());
    assert!(ofertas(&h).ligado(Verbo::Desfazer));
    clicar_marca(&mut h, Some(2));
    clicar_botao(&mut h, "Excluir");
    clicar_botao(&mut h, "Excluir");
    esperar_que(&mut h, |r| r.marcadas == 0 && !r.carregando && r.linhas > 0);
    assert!(!ofertas(&h).ligado(Verbo::Desfazer));
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), tela_dados::DICA_EXCLUIR);
}

/// O que foi para a área de transferência neste quadro.
fn copiado_agora(h: &H) -> Option<String> {
    h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    })
}

#[test]
fn ctrl_c_copia_a_celula_ativa_e_o_intervalo() {
    let amb = Ambiente::exemplo("dados_copiar_intervalo");
    let mut h = abrir(&amb, "eventos");
    // a célula ativa sozinha: o texto dela
    clicar_celula(&mut h, 0, 1);
    h.input_mut().events.push(Event::Copy);
    h.step();
    assert_eq!(copiado_agora(&h).as_deref(), Some("login"));
    // Shift com as setas: duas linhas × duas colunas, como o Excel copia
    clicar_celula(&mut h, 0, 0);
    tecla_com(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    tecla_com(&mut h, Modifiers::SHIFT, Key::ArrowRight);
    h.input_mut().events.push(Event::Copy);
    h.step();
    assert_eq!(copiado_agora(&h).as_deref(), Some("2026-09-01T00:00:00\tlogin\r\n2026-09-02T01:01:13\tconsulta\r\n"));
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("2 linhas × 2 colunas copiadas."));
}

// ---- o menu da célula (0.2.3) ------------------------------------------------------------------------------

/// O clique direito (mover, apertar e soltar o botão secundário) e alguns quadros.
fn clique_direito(h: &mut H, linha: usize, coluna: usize) {
    let pos = centro_celula(h, linha, coluna);
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.step();
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run_steps(3);
}

fn menu_aberto(h: &H) -> Option<String> {
    h.query_by_role(Role::Menu).and_then(|m| m.accesskit_node().label())
}

/// Escolhe um item do menu aberto (no submenu "Copiar como", passa por ele antes). Devolve o que foi para a área de
/// transferência nos quadros seguintes.
fn escolher_no_menu(h: &mut H, item: &str, submenu: bool) -> Option<String> {
    if submenu {
        h.get_by_label("Copiar como").hover();
        h.run_steps(3);
    }
    h.get_by_label(item).click();
    let mut copiado = None;
    for _ in 0..3 {
        h.step();
        copiado = copiado.or_else(|| copiado_agora(h));
    }
    copiado
}

/// As contas do rodapé da seleção (o texto inteiro, que o leitor de tela ouve; num rótulo, o AccessKit o guarda em
/// value).
fn rodape_da_selecao(h: &H) -> Option<String> {
    h.query_by_label_contains("Contagem ").and_then(|n| n.accesskit_node().value())
}

#[test]
fn menu_da_celula_copia_filtra_marca_e_abre_a_ficha() {
    let amb = Ambiente::exemplo("dados_menu_da_celula");
    let mut h = abrir(&amb, "clientes");
    // três linhas da coluna nome
    clicar_celula(&mut h, 0, 1);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    assert_eq!(rodape_da_selecao(&h).as_deref(), Some("Contagem 3"), "nome é texto: só a contagem");

    // o clique direito dentro do intervalo age nele
    clique_direito(&mut h, 1, 1);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da seleção"));
    for item in ["Colar", "Limpar", "Excluir as linhas"] {
        assert!(!desabilitado(&h.get_by_label(item)), "{item} ligado");
    }
    // nome é NOT NULL: o Limpar grava o texto vazio, o NULL não vale
    let n = h.get_by_label("Definir como NULL");
    assert!(desabilitado(&n));
    assert_eq!(n.accesskit_node().description().as_deref(), Some("A coluna nome não aceita NULL."));
    let copiado = escolher_no_menu(&mut h, "JSON", true);
    let nomes = ler(&h, "SELECT nome FROM clientes ORDER BY id LIMIT 3");
    let esperado = format!(
        "[\n{}\n]\n",
        nomes.iter().map(|l| format!("  {{\"nome\": \"{}\"}}", texto(&l[0]))).collect::<Vec<_>>().join(",\n")
    );
    assert_eq!(copiado.as_deref(), Some(esperado.as_str()), "Copiar como › JSON");
    assert!(menu_aberto(&h).is_none() && grade_com_foco(&h), "escolher fecha e devolve o foco");

    // Copiar com cabeçalho, pelo menu e pelo Ctrl+Shift+C
    let primeiro = texto(&nomes[0][0]);
    clique_direito(&mut h, 1, 1);
    let copiado = escolher_no_menu(&mut h, "Copiar com cabeçalho", false);
    assert!(copiado.is_some_and(|t| t.starts_with(&format!("nome\r\n{primeiro}\r\n"))));
    h.input_mut().events.push(Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: CTRL | SHIFT,
    });
    h.step();
    assert!(copiado_agora(&h).is_some_and(|t| t.starts_with("nome\r\n")), "o Ctrl+Shift+C");

    // as contas de uma coluna numérica
    clicar_celula(&mut h, 0, 4);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    let limites = ler(&h, "SELECT limite FROM clientes ORDER BY id LIMIT 2");
    let mut resumo = interface::grade::ResumoSelecao::default();
    for l in &limites {
        resumo.somar_celula(&l[0], true);
    }
    assert_eq!(rodape_da_selecao(&h), Some(resumo.texto()));
    assert!(resumo.texto().contains("Soma "), "{}", resumo.texto());

    // fora do intervalo, a célula vira a ativa; o Filtrar cria a condição na faixa
    clique_direito(&mut h, 4, 3);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da célula"));
    assert_eq!(retrato(&h).ativa, Some((4, 3)));
    let cidade = celula(&h, 4, 3);
    escolher_no_menu(&mut h, "Filtrar por este valor", false);
    esperar_que(&mut h, |r| r.filtros == 1 && !r.carregando);
    let filtro = ultima_rota(&h).and_then(|r| r.filtros.first().cloned()).expect("a condição");
    assert_eq!(
        (filtro.coluna.as_str(), filtro.op, filtro.valor.as_str()),
        ("cidade", Operador::Igual, cidade.as_str())
    );
    let rotulo = format!("Editar o filtro cidade igual a {cidade}");
    assert!(h.query_by_role_and_label(Role::Button, &rotulo).is_some(), "o chip da condição");
    assert!((0..retrato(&h).linhas).all(|l| celula(&h, l, 3) == cidade));

    // Excluir este valor do filtro: "diferente de"
    clique_direito(&mut h, 0, 1);
    let nome = celula(&h, 0, 1);
    escolher_no_menu(&mut h, "Excluir este valor do filtro", false);
    esperar_que(&mut h, |r| r.filtros == 2 && !r.carregando);
    let filtro = ultima_rota(&h).and_then(|r| r.filtros.get(1).cloned()).expect("a segunda condição");
    assert_eq!((filtro.op, filtro.valor.as_str()), (Operador::Diferente, nome.as_str()));

    // Marcar as linhas: as do intervalo, somando às marcadas
    clicar_celula(&mut h, 0, 1);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    clique_direito(&mut h, 0, 1);
    escolher_no_menu(&mut h, "Marcar as linhas", false);
    assert_eq!(retrato(&h).marcadas, 2);

    // o Shift+F10 abre na ativa, com o foco no primeiro item; Abrir a ficha da linha
    clicar_celula(&mut h, 1, 2);
    tecla_com(&mut h, SHIFT, Key::F10);
    assert!(menu_aberto(&h).is_some(), "o Shift+F10 abre o menu");
    assert!(h.get_by_label("Copiar").accesskit_node().is_focused());
    tecla(&mut h, Key::Escape);
    assert!(menu_aberto(&h).is_none() && grade_com_foco(&h), "o Esc fecha e devolve o foco");
    tecla_com(&mut h, SHIFT, Key::F10);
    escolher_no_menu(&mut h, "Abrir a ficha da linha", false);
    esperar_que(&mut h, |r| r.janela.is_some());
}

#[test]
fn menu_da_celula_filtra_o_null() {
    let amb = Ambiente::exemplo("dados_menu_null");
    executar(&amb, "UPDATE clientes SET cidade = NULL WHERE id = 1");
    let mut h = abrir(&amb, "clientes");
    // o NULL vira "é nulo"
    clique_direito(&mut h, 0, 3);
    escolher_no_menu(&mut h, "Filtrar por este valor", false);
    esperar_que(&mut h, |r| r.filtros == 1 && !r.carregando);
    let filtro = ultima_rota(&h).and_then(|r| r.filtros.first().cloned()).expect("a condição");
    assert_eq!(filtro.op, Operador::Nulo);
    assert_eq!(retrato(&h).linhas, 1);
}

// ---- os itens de escrita do menu da célula (0.2.3, A.2 e A.4) ------------------------------------------------

/// O Ctrl+V com o foco na grade: o texto chega no `Event::Paste`.
fn colar(h: &mut H, texto: &str) {
    h.input_mut().events.push(Event::Paste(texto.to_string()));
    h.step();
    h.run_steps(2);
    esperar_grade(h);
    h.run_steps(1);
}

fn cidades_e_limites(h: &H, n: usize) -> Vec<(String, String)> {
    ler(h, &format!("SELECT cidade, limite FROM clientes ORDER BY id LIMIT {n}"))
        .iter()
        .map(|l| (texto(&l[0]), texto(&l[1])))
        .collect()
}

/// O Colar por cima das células, um passo só no Desfazer; o valor só preenche o intervalo; o que passa da última
/// linha ou coluna fica de fora e o aviso diz quantas.
#[test]
fn colar_e_desfazer_de_uma_vez() {
    use estado::Verbo;
    let amb = Ambiente::exemplo("dados_colar");
    let mut h = abrir(&amb, "clientes");
    let antes = cidades_e_limites(&h, 2);
    clicar_celula(&mut h, 0, 3);
    colar(&mut h, "Natal\t10,5\r\nBelém\t20\r\n");
    assert_eq!(
        cidades_e_limites(&h, 2),
        [("Natal".to_string(), "10.5".to_string()), ("Belém".to_string(), "20".to_string())]
    );
    assert_eq!(toast(&h).as_deref(), Some("Gravado em clientes (4 células)"));
    assert_eq!(celula(&h, 0, 3), "Natal", "a página leva o que o banco releu");
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), "Desfazer Colar em 4 células (Ctrl+Z)");

    clicar_botao(&mut h, "Desfazer");
    esperar_que(&mut h, |r| !r.desfazendo);
    h.run_steps(1);
    assert_eq!(cidades_e_limites(&h, 2), antes);
    assert_eq!(toast(&h).as_deref(), Some("Edição desfeita em clientes (4 células)"));
    assert_eq!(celula(&h, 1, 3), antes[1].0);

    // um valor só com um intervalo: preenche o intervalo inteiro
    clicar_celula(&mut h, 0, 3);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    colar(&mut h, "Natal\r\n");
    let n = contar(&h, "SELECT count(*) FROM (SELECT cidade FROM clientes ORDER BY id LIMIT 3) WHERE cidade = 'Natal'");
    assert_eq!(n, 3);

    // a última linha da página e a última coluna: três células ficam de fora
    let ultima = retrato(&h).linhas - 1;
    tecla_com(&mut h, CTRL, Key::End);
    tecla(&mut h, Key::End);
    assert_eq!(retrato(&h).ativa, Some((ultima, 6)));
    colar(&mut h, "2026-01-01\tx\r\ny\tz\r\n");
    let t = toast(&h).unwrap_or_default();
    assert!(t.starts_with("Gravado em clientes (linha"), "{t}");
    assert!(t.ends_with("3 células ficaram de fora: passavam da última linha ou coluna."), "{t}");

    // a célula mudada depois por fora: nada volta, e o aviso diz qual
    for _ in 0..6 {
        tecla(&mut h, Key::PageUp);
    }
    tecla(&mut h, Key::Home);
    tecla(&mut h, Key::ArrowRight);
    tecla(&mut h, Key::ArrowRight);
    tecla(&mut h, Key::ArrowRight);
    assert_eq!(retrato(&h).ativa, Some((0, 3)));
    colar(&mut h, "Fortaleza\r\nManaus\r\n");
    executar(&amb, "UPDATE clientes SET cidade = 'Outra' WHERE id = 2");
    clicar_botao(&mut h, "Desfazer");
    esperar_que(&mut h, |r| !r.desfazendo);
    h.run_steps(1);
    assert_eq!(toast(&h).as_deref(), Some("A célula cidade da linha 2 mudou depois; nada foi desfeito."));
    assert_eq!(texto(&ler_um(&h, "SELECT cidade FROM clientes WHERE id = 1")), "Fortaleza");
}

/// O Limpar (o Delete sem marcadas): o texto vazio na de texto, o NULL na que aceita, o DEFAULT na NOT NULL; o
/// Definir como NULL pelo menu.
#[test]
fn limpar_e_definir_como_null() {
    let amb = Ambiente::exemplo("dados_limpar");
    executar(&amb, "UPDATE clientes SET ativo = 0 WHERE id = 1");
    let mut h = abrir(&amb, "clientes");
    clicar_celula(&mut h, 0, 3);
    tecla_com(&mut h, SHIFT, Key::ArrowRight);
    tecla_com(&mut h, SHIFT, Key::ArrowRight);
    tecla(&mut h, Key::Delete);
    esperar_grade(&mut h);
    let l = &ler(&h, "SELECT cidade, limite, ativo FROM clientes WHERE id = 1")[0];
    assert_eq!(
        (l[0].clone(), l[1].clone(), l[2].clone()),
        (Celula::Texto(String::new()), Celula::Nulo, Celula::Inteiro(1))
    );
    assert_eq!(toast(&h).as_deref(), Some("Gravado em clientes (3 células)"));

    // de novo: nada muda
    tecla(&mut h, Key::Delete);
    h.run_steps(2);
    assert_eq!(toast(&h).as_deref(), Some("Nada mudou: as células já tinham esses valores."));

    // Definir como NULL na cidade de duas linhas
    clicar_celula(&mut h, 1, 3);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    clique_direito(&mut h, 1, 3);
    escolher_no_menu(&mut h, "Definir como NULL", false);
    esperar_grade(&mut h);
    assert_eq!(contar(&h, "SELECT count(*) FROM clientes WHERE id IN (2, 3) AND cidade IS NULL"), 2);
    assert_eq!(celula(&h, 1, 3), "NULL");
}

/// Acima de 100 células, o lote pergunta antes; o Cancelar não grava nada.
#[test]
fn colar_acima_de_cem_celulas_pergunta() {
    let amb = Ambiente::exemplo("dados_colar_pergunta");
    let mut h = abrir(&amb, "eventos");
    clicar_celula(&mut h, 0, 1);
    tecla_com(&mut h, SHIFT, Key::ArrowRight);
    tecla_com(&mut h, CTRL | SHIFT, Key::ArrowDown);
    colar(&mut h, "x");
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Colar 200 células em eventos?"));
    clicar_botao(&mut h, "Cancelar");
    h.run_steps(2);
    assert_eq!(contar(&h, "SELECT count(*) FROM eventos WHERE tipo = 'x'"), 0);
    // o intervalo continua; o foco volta à grade
    let id = h.state().tela.doc.parte.id_grade().unwrap();
    h.ctx.memory_mut(|m| m.request_focus(id));
    h.run_steps(2);
    colar(&mut h, "x");
    clicar_botao(&mut h, "Colar");
    esperar_grade(&mut h);
    assert_eq!(contar(&h, "SELECT count(*) FROM eventos WHERE tipo = 'x'"), 100);
    assert_eq!(toast(&h).as_deref(), Some("Gravado em eventos (200 células)"));
}

/// O Excluir as linhas do menu: a pergunta e o caminho do Excluir da barra.
#[test]
fn excluir_as_linhas_do_intervalo() {
    use estado::Verbo;
    let amb = Ambiente::exemplo("dados_excluir_intervalo");
    let mut h = abrir(&amb, "clientes");
    clicar_celula(&mut h, 0, 1);
    tecla_com(&mut h, SHIFT, Key::ArrowDown);
    clique_direito(&mut h, 0, 1);
    escolher_no_menu(&mut h, "Excluir as linhas", false);
    assert_eq!(retrato(&h).confirmacao.as_deref(), Some("Excluir 2 linhas de clientes? Isso não pode ser desfeito."));
    clicar_botao(&mut h, "Excluir");
    // a exclusão volta e a página é relida: só então o Desfazer sabe que ela não se desfaz
    esperar(&mut h, |h| contar(h, "SELECT count(*) FROM clientes") == N_CLIENTES as u64 - 2);
    esperar_que(&mut h, |r| r.confirmacao.is_none() && !r.carregando && r.linhas > 0);
    assentar(&mut h);
    assert_eq!(contar(&h, "SELECT count(*) FROM clientes"), N_CLIENTES as u64 - 2);
    assert_eq!(ofertas(&h).dica(Verbo::Desfazer), tela_dados::DICA_EXCLUIR);
}

/// Os itens de escrita desligados, com o motivo: a coluna gerada, o banco só leitura; o Ctrl+V explica no aviso.
#[test]
fn itens_de_escrita_desligados_com_o_motivo() {
    let mut amb = Ambiente::exemplo("dados_escrita_desligada");
    let mut h = abrir(&amb, "produtos");
    let gerada = h.state().tela.doc.parte.colunas_da_grade().iter().position(|c| c == "preco_centavos").unwrap();
    clique_direito(&mut h, 0, gerada);
    let motivo = "A coluna preco_centavos é gerada; o valor dela vem da fórmula.";
    for item in ["Colar", "Limpar", "Definir como NULL"] {
        let n = h.get_by_label(item);
        assert!(desabilitado(&n), "{item}");
        assert_eq!(n.accesskit_node().description().as_deref(), Some(motivo));
    }
    assert!(!desabilitado(&h.get_by_label("Excluir as linhas")));
    tecla(&mut h, Key::Escape);
    colar(&mut h, "1");
    assert_eq!(toast(&h).as_deref(), Some(motivo));

    let caminho = amb.caminho_banco();
    amb.servico.fechar_banco(&amb.estado.banco_id(), true).expect("fechar");
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só para leitura");
    amb.recarregar();
    let mut h = abrir(&amb, "clientes");
    clique_direito(&mut h, 0, 1);
    for item in ["Colar", "Limpar", "Definir como NULL", "Excluir as linhas"] {
        let n = h.get_by_label(item);
        assert!(desabilitado(&n), "{item}");
        assert_eq!(
            n.accesskit_node().description().as_deref(),
            Some("O banco foi aberto só para leitura; nada pode ser gravado nele.")
        );
    }
}

/// D13 da revisão de 26/10/2026: a dica da célula só aparece quando o texto foi cortado com "…"; o que cabe inteiro
/// não ganha um balão que repete o que já está escrito.
#[test]
fn dica_da_celula_so_no_texto_cortado() {
    let amb = Ambiente::exemplo("dados_dica_cortada");
    let longo = "Maria Aparecida dos Santos Albuquerque de Vasconcelos e Silva Figueiredo Cavalcanti Monteiro";
    executar(&amb, &format!("UPDATE clientes SET nome = '{longo}' WHERE id = 1"));
    let mut h = abrir(&amb, "clientes");
    dicas_sem_demora(&mut h);
    assert_eq!(celula(&h, 0, 1), longo);
    let curto = celula(&h, 1, 1);
    // o nome curto, inteiro na célula: sem dica
    let p = centro_celula(&h, 1, 1);
    pairar(&mut h, p);
    assert!(h.query_by_label(&curto).is_none(), "a dica repetia \"{curto}\"");
    // o nome longo, cortado: a dica com o texto inteiro
    let p = centro_celula(&h, 0, 1);
    pairar(&mut h, p);
    assert!(h.query_by_label(longo).is_some(), "a dica do texto cortado");
}
