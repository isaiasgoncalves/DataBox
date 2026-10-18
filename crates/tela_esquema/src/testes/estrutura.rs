//! Aba Estrutura e janelas de coluna e de renomear (leitura, colunas e tabela), no formato de
//! documento da prancheta 6: a faixa, a lista com as pastilhas, a seleção, as ações em ícone, o "+N", a Nova coluna
//! e o Excluir da barra e as Propriedades.

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::{Ambiente, esperar};
use estado::{Aba, Acao, ParteTabela, Rota, Verbo};
use interface::pecas::toast::toast_atual;
use interface::tema::Tema;

use super::apoio::*;
use crate::AbaEstrutura;
use crate::estrutura::{DICA_DESCER, VAZIO_BUSCA_COLUNAS, rotulo_da_acao};

const N_CLIENTES: i64 = 60;
const N_EVENTOS: i64 = 5000;
const N_PRODUTOS: i64 = 10;

type Doc = NoPalcoDoc<AbaEstrutura>;

fn aba(h: &H<Doc>) -> &AbaEstrutura {
    &h.state().tela.doc.aba
}

fn recargas(h: &H<Doc>) -> usize {
    h.state().tela.doc.recargas
}

fn janela_coluna(t: &Doc) -> Option<&crate::mudanca::Mudanca> {
    t.doc.aba.janela_coluna().map(|j| &j.mudanca)
}

fn janela_renomear(t: &Doc) -> Option<&crate::mudanca::Mudanca> {
    t.doc.aba.janela_renomear().map(|j| &j.mudanca)
}

fn esperar_toast<S>(h: &mut H<S>) -> String {
    esperar(h, |h| toast_atual(&h.ctx).is_some());
    toast_atual(&h.ctx).unwrap_or_default()
}

/// O palco no claro, a 1440 × 900, com a lista já lida.
fn palco(amb: &Ambiente, tabela: &str, ancora: &str) -> Teste<Doc> {
    let mut h = palco_doc_claro::<AbaEstrutura>(amb, tabela);
    esperar_texto(&mut h, ancora);
    h
}

/// Clica na linha da coluna `nome`, fora do nome e dos botões (no meio da célula do Tipo): seleciona.
fn clicar_linha<S>(h: &mut H<S>, nome: &str) {
    let rect = mirar(h, nome, |h| {
        let n = h.query_all_by_label(nome).find(|n| n.accesskit_node().role() == Role::Button)?;
        let r = n.rect();
        Some(egui::Rect::from_center_size(egui::pos2(r.left() + 150.0 + 40.0, r.center().y), egui::vec2(4.0, 4.0)))
    });
    clicar_em(h, rect);
}

/// Os rótulos das etiquetas "+N" ("+2: não aceita nulo · → clientes.id").
fn rotulos_mais<S>(h: &H<S>) -> Vec<String> {
    h.query_all_by_label_contains("+")
        .filter_map(|n| {
            let a = n.accesskit_node();
            a.label().or(a.value())
        })
        .filter(|l| l.starts_with('+'))
        .collect()
}

#[test]
fn regras_que_nao_cabem_deixam_lugar_para_o_mais() {
    use crate::estrutura::regras_que_cabem;
    let mais = |_: usize| 20.0;
    // 3 itens de 50: 50 + 6 + 50 + 6 + 50 = 162
    assert_eq!(regras_que_cabem(&[50.0, 50.0, 50.0], 162.0, mais), 3);
    // não cabem todos: dois e o "+1" (50 + 6 + 50 + 6 + 20 = 132)
    assert_eq!(regras_que_cabem(&[50.0, 50.0, 50.0], 140.0, mais), 2);
    // um e o "+2"
    assert_eq!(regras_que_cabem(&[50.0, 50.0, 50.0], 100.0, mais), 1);
    // nem um: só o "+3"
    assert_eq!(regras_que_cabem(&[50.0, 50.0, 50.0], 30.0, mais), 0);
    assert_eq!(regras_que_cabem(&[], 10.0, mais), 0);
}

#[test]
fn padrao_cede_primeiro_para_as_regras_ficarem_com_170() {
    use crate::estrutura::largura_do_padrao;
    // larga: o Padrão fica com os 150 de sempre
    assert_eq!(largura_do_padrao(1100.0, true), 150.0);
    // 44 + 150 + 120 + 96 = 410 fixos: com 410 + 170 + 2 + 150 = 732 ele ainda tem os 150; abaixo disso cede
    assert_eq!(largura_do_padrao(732.0, true), 150.0);
    assert_eq!(largura_do_padrao(700.0, true), 118.0);
    // sem as ações (a visão e o banco só leitura), a coluna delas não conta
    assert_eq!(largura_do_padrao(700.0, false), 150.0);
    // estreita: o mínimo dele, e as Regras ficam com o resto
    assert_eq!(largura_do_padrao(400.0, true), 72.0);
}

#[test]
fn estrutura_faixa_e_lista_com_pastilhas() {
    let amb = Ambiente::exemplo("estrutura_clientes");
    let mut h = palco(&amb, "clientes", "criado_em");
    // a faixa: o título e a contagem
    assert!(tem(&h, "Colunas") && tem(&h, "7 colunas"));
    // o tipo escrito ao lado da pastilha, as pills de cada linha
    assert_eq!(na_linha(&h, "id", "INTEGER"), 1);
    assert_eq!(na_linha(&h, "id", "chave primária"), 1);
    assert_eq!(na_linha(&h, "id", "autoincremento"), 1);
    assert_eq!(na_linha(&h, "nome", "não aceita nulo"), 1);
    assert_eq!(na_linha(&h, "email", "único"), 1);
    assert_eq!(na_linha(&h, "cidade", "'São Paulo'"), 1);
    // a chave primária não se exclui, a primeira linha não sobe e a última não desce: só o lugar fica
    assert_eq!(na_linha(&h, "id", &rotulo_da_acao("Excluir", "id")), 0);
    assert_eq!(na_linha(&h, "id", &rotulo_da_acao("Subir", "id")), 0);
    assert_eq!(na_linha(&h, "id", &rotulo_da_acao("Descer", "id")), 1);
    assert_eq!(na_linha(&h, "criado_em", &rotulo_da_acao("Descer", "criado_em")), 0);
    assert_eq!(na_linha(&h, "nome", &rotulo_da_acao("Excluir", "nome")), 1);
    // o card Tabela saiu do corpo: Nome, Linhas, Sem rowid e Estrita estão na seção Tabela das Propriedades
    assert!(tem(&h, "Seção Tabela") && tem(&h, "Renomear a tabela"));
    assert!(!tem(&h, "Renomear…") && !tem(&h, "+ Nova coluna"));
    assert!(!tem(&h, "7 colunas · clique no nome para alterar"));
    // a linha das listas (`.lista-t td{height:32px}`)
    assert!((y_de(&h, "nome") - y_de(&h, "id") - 32.0).abs() < 1.0);
    // a coluna email selecionada, como na prancheta
    clicar_linha(&mut h, "email");
    h.run_steps(3);
    capturar(&mut h, "estrutura-clientes-claro.png");
    assert!(acoes(&h).is_empty());
}

#[test]
fn nova_coluna_pela_barra_e_o_mais_desligado() {
    let amb = Ambiente::exemplo("estrutura_nova_coluna_barra");
    let mut h = palco(&amb, "clientes", "criado_em");
    let ofertas = {
        let doc = &h.state().tela.doc;
        doc.aba.ofertas(&doc.tabela, &amb.estado)
    };
    // D2 (c): a Nova coluna liga e o "+" (Novo) fica desligado; sem coluna selecionada, o Excluir também
    assert!(ofertas.ligado(Verbo::NovaColuna) && !ofertas.ligado(Verbo::Novo));
    assert!(!ofertas.ligado(Verbo::Excluir) && ofertas.ligado(Verbo::Buscar));
    assert!(ligado_na_barra(&h, "Nova coluna") && !tem(&h, "Novo"));
    clicar(&mut h, "Nova coluna");
    esperar(&mut h, |h| dialogos(h) == ["Nova coluna"]);
    // só leitura: nada de Nova coluna
    let mut amb = Ambiente::exemplo("estrutura_nova_coluna_leitura");
    let caminho = amb.caminho_banco();
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só leitura");
    amb.recarregar();
    let doc = palco(&amb, "clientes", "criado_em");
    let d = &doc.state().tela.doc;
    assert!(!d.aba.ofertas(&d.tabela, &amb.estado).ligado(Verbo::NovaColuna));
}

#[test]
fn clicar_na_linha_seleciona_e_as_propriedades_seguem() {
    let amb = Ambiente::exemplo("estrutura_selecao");
    let mut h = palco(&amb, "clientes", "criado_em");
    assert!(aba(&h).selecionada().is_none() && !tem(&h, "Seção Coluna"));
    clicar_linha(&mut h, "email");
    h.run_steps(3);
    assert_eq!(aba(&h).selecionada(), Some("email"));
    assert!(dialogos(&h).is_empty(), "o clique na linha só seleciona");
    assert!(tem(&h, "Seção Coluna") && tem(&h, "Alterar a coluna"));
    // a ficha da coluna: o tipo, a regra e a ordenação (nenhuma)
    assert!(texto_com(&h, "TEXT").is_some() && tem(&h, "nenhuma"));
    // o Excluir da barra liga e diz a coluna
    assert!(ligado_na_barra(&h, "Excluir"));
    let d = &h.state().tela.doc;
    assert_eq!(d.aba.ofertas(&d.tabela, &amb.estado).dica(Verbo::Excluir), "Excluir a coluna email");
    // a chave primária não se exclui pela barra
    clicar_linha(&mut h, "id");
    h.run_steps(3);
    assert!(!ligado_na_barra(&h, "Excluir"));
    // o clique no nome seleciona e abre "Alterar a coluna"
    clicar(&mut h, "cidade");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    assert_eq!(aba(&h).selecionada(), Some("cidade"));
    // "Alterar a coluna" das Propriedades abre a mesma janela
    clicar_no_dialogo(&mut h, "Fechar");
    esperar(&mut h, |h| dialogos(h).is_empty());
    clicar(&mut h, "Alterar a coluna");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    assert!(aba(&h).janela_coluna().is_some());
}

#[test]
fn excluir_a_coluna_selecionada_pela_barra() {
    let amb = Ambiente::exemplo("estrutura_excluir_barra");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "momento");
    clicar_linha(&mut h, "dados");
    h.run_steps(3);
    clicar(&mut h, "Excluir");
    esperar(&mut h, |h| dialogos(h) == ["Excluir coluna"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("Excluir a coluna dados de eventos?") && frase.contains("não pode ser desfeito"), "{frase}");
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| recargas(h) == 1);
    assert_eq!(colunas_do_arquivo(&caminho, "eventos"), ["momento", "tipo", "valor"]);
    assert_eq!(contar_linhas(&caminho, "eventos"), N_EVENTOS);
}

#[test]
fn banco_so_leitura_nao_mostra_acoes_de_estrutura() {
    let mut amb = Ambiente::exemplo("estrutura_so_leitura");
    let caminho = amb.caminho_banco();
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só leitura");
    amb.recarregar();
    let mut h = palco_aba::<AbaEstrutura>(&amb, "clientes", 1000.0);
    esperar_texto(&mut h, "criado_em");
    assert!(texto_com(&h, "aberto só para leitura").is_some());
    assert!(botoes(&h).is_empty(), "nenhum botão: {:?}", botoes(&h));
    assert!(!tem(&h, "Excluir a tabela"));
    let passo = |a: &str, b: &str| y_de(&h, b) - y_de(&h, a);
    assert!((passo("id", "nome") - 32.0).abs() < 1.0, "id → nome: {}", passo("id", "nome"));
    assert!((passo("cidade", "limite") - 32.0).abs() < 1.0, "cidade → limite: {}", passo("cidade", "limite"));
    capturar(&mut h, "estrutura-leitura-claro.png");
}

#[test]
fn estrutura_sem_edicao_nao_mostra_acoes() {
    let amb = Ambiente::exemplo("estrutura_sem_edicao");
    for (tabela, motivo) in [("notas", "Tabela virtual"), ("sqlite_sequence", "Tabela interna do SQLite")] {
        let mut h = palco_aba::<AbaEstrutura>(&amb, tabela, 900.0);
        esperar_texto(&mut h, motivo);
        assert!(botoes(&h).is_empty(), "{tabela}: {:?}", botoes(&h));
        if tabela == "notas" {
            // as colunas ocultas da tabela virtual, nas linhas de 32px
            let ys: Vec<f32> = h.query_all_by_label("oculta").map(|n| n.rect().center().y).collect();
            assert_eq!(ys.len(), 2, "{ys:?}");
            assert!((ys[1] - ys[0] - 32.0).abs() < 1.0, "{ys:?}");
            capturar(&mut h, "estrutura-notas-claro.png");
        }
    }
}

/// Numa janela de 1000px (o documento com 720px), as Regras que não cabem viram o "+N" com a lista na dica, e as
/// ações de cada linha continuam à vista, dentro do documento.
#[test]
fn estrutura_numa_janela_estreita_mostra_mais_n() {
    let amb = Ambiente::exemplo("estrutura_estreita");
    let mut h = palco_doc::<AbaEstrutura>(&amb, "clientes", Tema::Claro, egui::vec2(1000.0, 900.0));
    esperar_texto(&mut h, "criado_em");
    h.run_steps(3);
    capturar(&mut h, "estrutura-1000-claro.png");
    // as regras de id não cabem nos 134px da célula: o "+N" leva o autoincremento para a dica
    let mais = rotulos_mais(&h);
    assert!(mais.iter().any(|l| l.contains("autoincremento")), "o +N com a lista: {mais:?}");
    assert_eq!(na_linha(&h, "nome", "não aceita nulo"), 1, "a que cabe fica");
    // tudo numa linha: as linhas ficam nos 32px
    assert!((y_de(&h, "nome") - y_de(&h, "id") - 32.0).abs() < 1.0);
    let direita_do_doc = 1000.0 - 272.0 - 8.0;
    let excluir: Vec<egui::Rect> = h
        .query_all_by_label_contains("Excluir a coluna ")
        .filter(|n| n.accesskit_node().role() == Role::Button)
        .map(|n| n.rect())
        .collect();
    assert_eq!(excluir.len(), 6);
    assert!(excluir.iter().all(|r| r.right() <= direita_do_doc), "{excluir:?}");
}

#[test]
fn regras_que_nao_cabem_viram_mais_n() {
    let amb = Ambiente::exemplo("estrutura_mais_n");
    let mut h = palco_doc::<AbaEstrutura>(&amb, "pedidos", Tema::Claro, egui::vec2(960.0, 900.0));
    esperar_texto(&mut h, "observacao");
    h.run_steps(3);
    capturar(&mut h, "estrutura-pedidos-960-claro.png");
    // em pedidos, a 960px, a linha de cliente_id não cabe com a referência: o "+N" a leva para a dica
    let mais = rotulos_mais(&h);
    assert!(!mais.is_empty(), "algum +N");
    assert!(mais.iter().any(|l| l.contains("→ clientes.id") || l.contains("verificação")), "{mais:?}");
}

/// Numa janela larga, nada encolhe: as Regras ficam numa linha.
#[test]
fn estrutura_larga_nao_quebra() {
    let amb = Ambiente::exemplo("estrutura_larga");
    let mut h = palco(&amb, "pedidos", "observacao");
    h.run_steps(3);
    assert!((y_de(&h, "→ clientes.id") - y_de(&h, "cliente_id")).abs() < 1.0);
    assert!((y_de(&h, "verificação") - y_de(&h, "status")).abs() < 1.0);
    assert!(!tem(&h, "+1"));
    capturar(&mut h, "estrutura-pedidos-claro.png");
}

#[test]
fn estrutura_de_pedidos_mostra_chave_estrangeira() {
    let amb = Ambiente::exemplo("estrutura_pedidos_fk");
    let mut h = palco(&amb, "pedidos", "cliente_id");
    // o mesmo texto nas Regras e na lista Chaves estrangeiras
    assert_eq!(contar(&h, "→ clientes.id"), 2);
    assert_eq!(na_linha(&h, "cliente_id", "→ clientes.id"), 1);
    assert!(tem(&h, "Chaves estrangeiras") && tem(&h, "CASCADE"));
    // a pastilha da chave estrangeira e a referência na ficha da coluna
    clicar_linha(&mut h, "cliente_id");
    h.run_steps(3);
    assert!(tem(&h, "Seção Coluna"));

    // a janela nasce com a referência como está: abrir e não mexer não muda nada
    clicar(&mut h, "cliente_id");
    esperar(&mut h, |h| aba(h).janela_coluna().is_some());
    {
        let j = aba(&h).janela_coluna().expect("janela");
        assert_eq!(j.campos.ref_tabela_nome(), "clientes");
        assert_eq!(j.campos.ref_coluna_nome(), "id");
        assert_eq!(j.campos.ref_excluir_nome(), "CASCADE");
    }
    esperar_previa(&mut h, janela_coluna, "Nada muda");
    esperar(&mut h, |h| aba(h).janela_coluna().is_some_and(|j| j.campos.colunas_ref.len() == 1 + 7));
    assert_eq!(aba(&h).janela_coluna().map(|j| j.campos.ref_coluna_nome()), Some("id"));
    assentar_previa(&mut h, janela_coluna);
    capturar(&mut h, "janela-coluna-fk-claro.png");
    preencher(&mut h, "Tipo", "BIGINT");
    esperar_previa(
        &mut h,
        janela_coluna,
        "\"cliente_id\" BIGINT NOT NULL REFERENCES \"clientes\" (\"id\") ON DELETE CASCADE",
    );
}

/// A zona "Excluir a tabela" aberta não continua aberta numa aba nova (depois de uma recarga ou de voltar à aba): o
/// nasce fechado.
#[test]
fn zona_de_risco_nasce_fechada_em_cada_aba_nova() {
    let amb = Ambiente::exemplo("estrutura_zona_fechada");
    let mut h = palco(&amb, "clientes", "criado_em");
    assert!(!tem(&h, "Digite o nome da tabela para confirmar"));
    clicar(&mut h, "Seção Excluir a tabela");
    esperar_texto(&mut h, "Digite o nome da tabela para confirmar");
    // a casca refaz a aba (a recarga depois de uma mudança, a volta à aba)
    h.state_mut().tela.doc.tabela.recarregar = true;
    esperar(&mut h, |h| recargas(h) == 1);
    h.run_steps(3);
    assert!(!tem(&h, "Digite o nome da tabela para confirmar"));
    clicar(&mut h, "Seção Excluir a tabela");
    esperar_texto(&mut h, "Digite o nome da tabela para confirmar");
}

/// O erro de uma ação numa linha de baixo aparece no alto da lista, e a página rola até ele (o
/// `slot.scrollIntoView({block:"nearest"})`).
#[test]
fn erro_na_lista_rola_a_pagina_ate_ele() {
    let amb = Ambiente::exemplo("estrutura_erro_rola");
    let tamanho = egui::vec2(1440.0, 420.0);
    let mut h = palco_aba_rolavel::<AbaEstrutura>(&amb, "pedidos", tamanho);
    esperar_texto(&mut h, "observacao");
    assentar(&mut h);
    let vista = vista_da_pagina(tamanho);
    rolar_com_roda(&mut h, vista.center(), -200.0, 1);
    assert!(no(&h, "Colunas").rect().bottom() < vista.top(), "a faixa saiu da vista");
    clicar_na_linha(&mut h, "observacao", &rotulo_da_acao("Excluir", "observacao"));
    esperar(&mut h, |h| dialogos(h) == ["Excluir coluna"]);
    clicar_no_dialogo(&mut h, "Excluir");
    let frase = "A coluna observacao é usada pelo gatilho trg_pedido_cancelado. Exclua o gatilho antes.";
    esperar_texto(&mut h, frase);
    assentar(&mut h);
    // o aviso entra entre a faixa e a lista; com a página rolada o mínimo para ele aparecer, a primeira linha da
    // lista fica à vista logo abaixo
    let primeira = no(&h, "id").rect();
    assert!(primeira.top() >= vista.top() && primeira.bottom() <= vista.bottom(), "{primeira:?}");
    assert_eq!(h.state().tela.recargas, 0);
    capturar(&mut h, "erro-no-card-rolado-claro.png");
}

#[test]
fn adicionar_coluna_nativa() {
    let amb = Ambiente::exemplo("estrutura_adicionar");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "clientes", "criado_em");
    clicar(&mut h, "Nova coluna");
    esperar(&mut h, |h| dialogos(h) == ["Nova coluna"]);
    assert!(!tem(&h, "Excluir coluna"));
    esperar_previa(&mut h, janela_coluna, "-- Digite o nome da coluna para ver o SQL.");
    capturar(&mut h, "nova-coluna-claro.png");
    preencher(&mut h, "Nome", "apelido");
    esperar_previa(&mut h, janela_coluna, "ALTER TABLE \"clientes\" ADD COLUMN \"apelido\" TEXT");
    let slot = aba(&h).janela_coluna().and_then(|j| j.mudanca.texto_do_slot()).expect("aviso");
    assert!(slot.0.contains("Adicionar a coluna apelido (TEXT) em clientes."), "{slot:?}");
    assert!(!slot.0.contains("reconstruída"));

    clicar_no_dialogo(&mut h, "Aplicar");
    esperar(&mut h, |h| recargas(h) == 1);
    assert!(dialogos(&h).is_empty());
    assert!(esperar_toast(&mut h).contains("apelido"));
    assert!(acoes(&h).contains(&Acao::RecarregarObjetos));
    esperar_texto(&mut h, "apelido");
    assert_eq!(colunas_do_arquivo(&caminho, "clientes").last().map(String::as_str), Some("apelido"));
    assert_eq!(contar_linhas(&caminho, "clientes"), N_CLIENTES);
}

#[test]
fn adicionar_coluna_unica_reconstroi() {
    let amb = Ambiente::exemplo("estrutura_unica");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "clientes", "criado_em");
    clicar(&mut h, "Nova coluna");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    preencher(&mut h, "Nome", "documento");
    clicar_no_dialogo(&mut h, "Único");
    esperar_previa(&mut h, janela_coluna, "CREATE TABLE \"_novo_clientes\"");
    {
        let m = janela_coluna(&h.state().tela).expect("janela");
        assert!(m.texto.contains("BEGIN;"));
        assert!(m.texto.contains("\"documento\" TEXT UNIQUE"));
        assert!(m.texto.contains("COMMIT;"));
        assert!(m.texto_do_slot().is_some_and(|s| s.0.contains("A tabela será reconstruída")));
    }
    clicar_no_dialogo(&mut h, "Aplicar");
    esperar(&mut h, |h| recargas(h) == 1);
    esperar_texto(&mut h, "documento");
    assert_eq!(na_linha(&h, "documento", "único"), 1);
    assert!(objeto(&caminho, "table", "clientes").is_some_and(|s| s.contains("\"documento\" TEXT UNIQUE")));
    assert_eq!(contar_linhas(&caminho, "clientes"), N_CLIENTES);
    assert!(objeto(&caminho, "index", "idx_clientes_nome_cidade").is_some());
    assert!(objeto(&caminho, "view", "v_pedidos_por_cliente").is_some());
    assert!(consultar(&caminho, "SELECT name FROM sqlite_schema WHERE name LIKE '_novo_%'").is_empty());
}

#[test]
fn previa_com_erro_nao_trava_a_janela() {
    let amb = Ambiente::exemplo("estrutura_previa_erro");
    let mut h = palco(&amb, "clientes", "criado_em");
    clicar(&mut h, "Nova coluna");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    preencher(&mut h, "Nome", "email");
    esperar(&mut h, |h| {
        janela_coluna(&h.state().tela)
            .and_then(|m| m.texto_do_slot())
            .is_some_and(|(t, perigo)| perigo && t.contains("já tem uma coluna chamada email"))
    });
    assentar_previa(&mut h, janela_coluna);
    capturar(&mut h, "nova-coluna-erro-previa-claro.png");
    preencher(&mut h, "Nome", "email_alternativo");
    esperar_previa(&mut h, janela_coluna, "ADD COLUMN \"email_alternativo\"");
    assert!(janela_coluna(&h.state().tela).and_then(|m| m.texto_do_slot()).is_some_and(|(_, perigo)| !perigo));

    // autoincremento só com INTEGER e chave primária
    assert!(desabilitada(&h, "Autoincremento"));
    preencher(&mut h, "Tipo", "INTEGER");
    h.key_press(egui::Key::Escape); // fecha a lista de sugestões
    h.run_steps(2);
    assert!(desabilitada(&h, "Autoincremento"));
    clicar_no_dialogo(&mut h, "Chave primária");
    assentar_previa(&mut h, janela_coluna);
    assert!(!desabilitada(&h, "Autoincremento"));
    clicar_no_dialogo(&mut h, "Autoincremento");
    assert!(marcada(&h, "Autoincremento"));
    preencher(&mut h, "Tipo", "TEXT");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(desabilitada(&h, "Autoincremento"));
    assert!(!marcada(&h, "Autoincremento"));
    clicar_no_dialogo(&mut h, "Fechar");
    assert!(dialogos(&h).is_empty());
    assert!(aba(&h).janela_coluna().is_none());
}

#[test]
fn alterar_tipo_de_produtos_preco() {
    let amb = Ambiente::exemplo("estrutura_preco");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "produtos", "preco");
    clicar(&mut h, "preco");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    assert_eq!(valor(&h, "Tipo"), "NUMERIC");
    assert_eq!(valor(&h, "Valor padrão"), "0");
    assert!(marcada(&h, "Não aceita nulo"));
    esperar_previa(&mut h, janela_coluna, "Nada muda");
    preencher(&mut h, "Tipo", "REAL");
    esperar_previa(&mut h, janela_coluna, "\"preco\" REAL NOT NULL DEFAULT 0");
    {
        let slot = janela_coluna(&h.state().tela).and_then(|m| m.texto_do_slot()).expect("aviso");
        assert!(slot.0.contains("tipo NUMERIC → REAL"), "{slot:?}");
        assert!(slot.0.contains("A tabela será reconstruída"), "{slot:?}");
    }
    clicar_no_dialogo(&mut h, "Aplicar");
    esperar(&mut h, |h| recargas(h) == 1);
    assert_eq!(na_linha(&h, "preco", "REAL"), 1);
    let tipos = consultar(&caminho, "SELECT name, type FROM pragma_table_xinfo('produtos')");
    assert!(tipos.iter().any(|l| l[0] == "preco" && l[1] == "REAL"));
    assert_eq!(contar_linhas(&caminho, "produtos"), N_PRODUTOS);
    assert!(objeto(&caminho, "table", "produtos").is_some_and(|s| s.to_uppercase().contains("WITHOUT ROWID")));
    assert_eq!(
        numero(&caminho, "SELECT count(*) FROM produtos WHERE preco_centavos = CAST(preco * 100 AS INTEGER)"),
        N_PRODUTOS
    );
}

#[test]
fn renomear_coluna() {
    let amb = Ambiente::exemplo("estrutura_renomear_coluna");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "clientes", "cidade");
    clicar(&mut h, "cidade");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    preencher(&mut h, "Nome", "municipio");
    esperar_previa(&mut h, janela_coluna, "ALTER TABLE \"clientes\" RENAME COLUMN \"cidade\" TO \"municipio\"");
    assert!(
        janela_coluna(&h.state().tela).and_then(|m| m.texto_do_slot()).is_some_and(|s| !s.0.contains("reconstruída"))
    );
    capturar(&mut h, "janela-coluna-alterar-claro.png");
    clicar_no_dialogo(&mut h, "Aplicar");
    esperar(&mut h, |h| recargas(h) == 1);
    esperar_texto(&mut h, "municipio");
    let nomes = colunas_do_arquivo(&caminho, "clientes");
    assert!(nomes.contains(&"municipio".to_string()) && !nomes.contains(&"cidade".to_string()));
    assert!(objeto(&caminho, "index", "idx_clientes_nome_cidade").is_some_and(|s| s.contains("municipio")));
}

#[test]
fn janela_coluna_reconstroi() {
    let amb = Ambiente::exemplo("estrutura_reconstroi");
    let mut h = palco(&amb, "clientes", "limite");
    clicar(&mut h, "limite");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    preencher(&mut h, "Tipo", "NUMERIC");
    esperar_previa(&mut h, janela_coluna, "CREATE TABLE \"_novo_clientes\"");
    // a lista de sugestões fecha com um clique fora do campo
    clicar_no_texto(&mut h, "SQL que será executado");
    capturar(&mut h, "janela-coluna-reconstroi-claro.png");
}

/// A janela da prancheta 12b: o título com o nome da coluna, a tabela no sub, a "Ordenação (COLLATE)" (não mais
/// "Colação") ao lado da Referência, a prévia com um comando por bloco e o Cancelar que fecha.
#[test]
fn janela_de_coluna_pela_prancheta_12b() {
    let amb = Ambiente::exemplo("estrutura_janela_12b");
    let mut h = palco(&amb, "clientes", "limite");
    clicar(&mut h, "limite");
    esperar(&mut h, |h| dialogos(h) == ["Alterar a coluna limite"]);
    assert!(tem(&h, "clientes"), "o sub com o nome da tabela");
    assert!(h.query_by_role_and_label(Role::ComboBox, "Ordenação (COLLATE)").is_some());
    assert!(!tem(&h, "Colação") && tem(&h, "Referência") && tem(&h, "Excluir coluna"));
    // a Ordenação e a Referência na mesma linha, em duas colunas
    let (ordem, ref_) = (no(&h, "Ordenação (COLLATE)").rect(), no(&h, "Referência").rect());
    assert!((ordem.center().y - ref_.center().y).abs() < 1.0 && ref_.left() > ordem.right(), "{ordem:?} {ref_:?}");
    // a pastilha acompanha o tipo digitado (o campo fica com a altura de sempre)
    preencher(&mut h, "Tipo", "NUMERIC");
    esperar_previa(&mut h, janela_coluna, "CREATE TABLE \"_novo_clientes\"");
    {
        let m = janela_coluna(&h.state().tela).expect("janela");
        assert!(!m.texto.contains(";\n\n"), "um comando por bloco, sem linha em branco: {}", m.texto);
        assert!(m.texto.contains("COMMIT;"));
    }
    clicar_no_dialogo(&mut h, "Cancelar");
    esperar(&mut h, |h| dialogos(h).is_empty());
    assert!(aba(&h).janela_coluna().is_none());
    assert_eq!(recargas(&h), 0, "Cancelar não muda nada");
}

#[test]
fn subir_e_descer_pelos_botoes_de_icone() {
    let amb = Ambiente::exemplo("estrutura_reordenar");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "momento");
    // a dica diz que a tabela é reconstruída
    assert!(DICA_DESCER.contains("reconstrói a tabela"));
    clicar_na_linha(&mut h, "momento", &rotulo_da_acao("Descer", "momento"));
    esperar(&mut h, |h| dialogos(h) == ["Mudar a ordem das colunas"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("reconstrói a tabela eventos"), "{frase}");
    clicar_no_dialogo(&mut h, "Reconstruir a tabela");
    esperar(&mut h, |h| recargas(h) == 1);
    assert_eq!(colunas_do_arquivo(&caminho, "eventos"), ["tipo", "momento", "valor", "dados"]);
    assert_eq!(contar_linhas(&caminho, "eventos"), N_EVENTOS);
    assert!(y_de(&h, "tipo") < y_de(&h, "momento"));
    // e de volta pelo Subir
    clicar_na_linha(&mut h, "momento", &rotulo_da_acao("Subir", "momento"));
    esperar(&mut h, |h| dialogos(h) == ["Mudar a ordem das colunas"]);
    clicar_no_dialogo(&mut h, "Reconstruir a tabela");
    esperar(&mut h, |h| recargas(h) == 2);
    assert_eq!(colunas_do_arquivo(&caminho, "eventos"), ["momento", "tipo", "valor", "dados"]);
}

#[test]
fn excluir_coluna_livre() {
    let amb = Ambiente::exemplo("estrutura_excluir_coluna");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "dados");
    clicar_na_linha(&mut h, "dados", &rotulo_da_acao("Excluir", "dados"));
    esperar(&mut h, |h| dialogos(h) == ["Excluir coluna"]);
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| recargas(h) == 1);
    assert!(!tem(&h, "dados"));
    assert_eq!(colunas_do_arquivo(&caminho, "eventos"), ["momento", "tipo", "valor"]);
    assert_eq!(contar_linhas(&caminho, "eventos"), N_EVENTOS);
}

#[test]
fn excluir_coluna_indexada_mostra_o_erro_do_servidor() {
    let amb = Ambiente::exemplo("estrutura_coluna_indexada");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "pedidos", "cliente_id");
    clicar_na_linha(&mut h, "cliente_id", &rotulo_da_acao("Excluir", "cliente_id"));
    esperar(&mut h, |h| !dialogos(h).is_empty());
    capturar(&mut h, "confirmar-excluir-coluna-claro.png");
    clicar_no_dialogo(&mut h, "Excluir");
    esperar_texto(&mut h, "A coluna cliente_id é usada pelo índice idx_pedidos_cliente. Exclua o índice antes.");
    assert!(tem(&h, "cliente_id"));
    assert!(colunas_do_arquivo(&caminho, "pedidos").contains(&"cliente_id".to_string()));
    assert_eq!(recargas(&h), 0);
    capturar(&mut h, "erro-no-card-claro.png");

    // a confirmação de mover (fechada sem mover)
    clicar_na_linha(&mut h, "status", &rotulo_da_acao("Subir", "status"));
    esperar(&mut h, |h| dialogos(h) == ["Mudar a ordem das colunas"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("Mover a coluna status para cima reconstrói a tabela pedidos"), "{frase}");
    capturar(&mut h, "confirmar-mover-claro.png");
    clicar_no_dialogo(&mut h, "Fechar");
    esperar(&mut h, |h| dialogos(h).is_empty());
    assert_eq!(recargas(&h), 0);

    // pela janela da coluna, o erro aparece no alto da janela, que continua aberta
    clicar(&mut h, "cliente_id");
    esperar(&mut h, |h| dialogos(h).len() == 1);
    let titulo = dialogos(&h)[0].clone();
    clicar_no_dialogo(&mut h, "Excluir coluna");
    esperar(&mut h, |h| dialogos(h).len() == 2);
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| {
        aba(h).janela_coluna().is_some_and(|j| {
            j.mudanca.aviso.as_ref().is_some_and(|(m, perigo)| *perigo && m.contains("idx_pedidos_cliente"))
        })
    });
    assert_eq!(dialogos(&h), [titulo]);
}

#[test]
fn renomear_e_excluir_a_tabela_pelas_propriedades() {
    let amb = Ambiente::exemplo("estrutura_renomear_tabela");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "momento");
    clicar(&mut h, "Renomear a tabela");
    esperar(&mut h, |h| dialogos(h) == ["Renomear tabela"]);
    assert_eq!(valor(&h, "Novo nome"), "eventos");
    esperar_previa(&mut h, janela_renomear, "-- Digite o nome novo");
    capturar(&mut h, "renomear-claro.png");
    // o nome atual nasce selecionado: digitar troca o nome inteiro
    h.get_by_role_and_label(Role::TextInput, "Novo nome").type_text("registros");
    h.run_steps(2);
    assert_eq!(valor(&h, "Novo nome"), "registros");
    esperar_previa(&mut h, janela_renomear, "ALTER TABLE \"eventos\" RENAME TO \"registros\"");
    capturar(&mut h, "renomear-previa-claro.png");
    clicar_no_dialogo(&mut h, "Renomear");
    esperar(&mut h, |h| h.state().acoes.iter().any(|a| matches!(a, Acao::Navegar(_))));
    assert!(dialogos(&h).is_empty());
    assert_eq!(
        acoes(&h),
        [
            Acao::NotarTransacao(false),
            Acao::RecarregarObjetos,
            Acao::Navegar(Rota::objeto_na_aba("registros", false, Aba::Estrutura))
        ]
    );
    assert!(objeto(&caminho, "table", "eventos").is_none());
    assert_eq!(contar_linhas(&caminho, "registros"), N_EVENTOS);

    // a zona "Excluir a tabela" das Propriedades, recolhida, pede o nome
    let amb = Ambiente::exemplo("estrutura_excluir_tabela");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "momento");
    clicar(&mut h, "Seção Excluir a tabela");
    esperar_texto(&mut h, "Digite o nome da tabela para confirmar");
    assert!(desabilitada(&h, "Excluir tabela"));
    capturar(&mut h, "estrutura-clientes-zona-risco-claro.png");
    preencher(&mut h, "Digite o nome da tabela para confirmar", "Eventos");
    assert!(desabilitada(&h, "Excluir tabela"));
    preencher(&mut h, "Digite o nome da tabela para confirmar", "eventos");
    assert!(!desabilitada(&h, "Excluir tabela"));
    clicar(&mut h, "Excluir tabela");
    esperar(&mut h, |h| h.state().acoes.iter().any(|a| matches!(a, Acao::Navegar(_))));
    assert!(acoes(&h).ends_with(&[Acao::RecarregarObjetos, Acao::Navegar(Rota::Banco)]));
    assert!(objeto(&caminho, "table", "eventos").is_none());
}

#[test]
fn ordenacao_no_lugar_de_colacao() {
    let amb = Ambiente::exemplo("estrutura_gerada");
    let mut h = palco(&amb, "produtos", "preco_centavos");
    assert_eq!(na_linha(&h, "preco_centavos", "gerada"), 1);
    assert_eq!(na_linha(&h, "descricao", "ordenação NOCASE"), 1);
    assert!(!tem(&h, "colação NOCASE"));
    // na ficha da coluna, a Ordenação
    clicar_linha(&mut h, "descricao");
    h.run_steps(3);
    assert!(tem(&h, "Seção Coluna") && tem(&h, "NOCASE"), "a Ordenação da coluna na ficha");
    capturar(&mut h, "estrutura-produtos-claro.png");
    clicar(&mut h, "preco_centavos");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    assert_eq!(valor(&h, "Expressão calculada"), "CAST(preco * 100 AS INTEGER)");
    assert!(!marcada(&h, "Armazenada (STORED)"));
    assert!(tem(&h, "Verificação (CHECK)"));
    assert!(!tem(&h, "Valor padrão"));
    esperar_previa(&mut h, janela_coluna, "Nada muda");
    capturar(&mut h, "janela-gerada-claro.png");
}

#[test]
fn buscar_colunas_filtra_a_lista() {
    let amb = Ambiente::exemplo("estrutura_busca");
    let mut h = palco(&amb, "clientes", "criado_em");
    let campo = h.get_by_label("Buscar colunas");
    campo.focus();
    campo.type_text("MAIL");
    h.run_steps(3);
    assert!(tem(&h, "email") && !tem(&h, "cidade"), "a busca sem caixa acha email");
    h.get_by_label("Buscar colunas").type_text("xyz");
    h.run_steps(3);
    assert!(tem(&h, VAZIO_BUSCA_COLUNAS));
}

#[test]
fn referencia_escolhida_carrega_as_colunas_da_outra_tabela() {
    let amb = Ambiente::exemplo("estrutura_referencia");
    let mut h = palco(&amb, "eventos", "momento");
    clicar(&mut h, "Nova coluna");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    assert!(!tem(&h, "Coluna referenciada"), "o bloco da referência só aparece com uma tabela escolhida");
    preencher(&mut h, "Nome", "cliente");
    assentar_previa(&mut h, janela_coluna);
    clicar_no_dialogo(&mut h, "Referência");
    clicar_no_dialogo(&mut h, "clientes");
    esperar(&mut h, |h| aba(h).janela_coluna().is_some_and(|j| j.campos.colunas_ref.len() == 1 + 7));
    assert!(tem(&h, "Coluna referenciada"));
    esperar_previa(&mut h, janela_coluna, "\"cliente\" TEXT REFERENCES \"clientes\"");
    clicar_no_dialogo(&mut h, "Ao excluir");
    clicar_no_dialogo(&mut h, "CASCADE");
    esperar_previa(&mut h, janela_coluna, "ON DELETE CASCADE");
}

#[test]
fn estrutura_de_itens_pedido_mostra_restricoes_e_chaves() {
    let amb = Ambiente::exemplo("estrutura_itens");
    let mut h = palco_doc::<AbaEstrutura>(&amb, "itens_pedido", Tema::Claro, egui::vec2(1440.0, 1000.0));
    esperar_texto(&mut h, "Restrições da tabela");
    assert!(tem(&h, "PRIMARY KEY (pedido_id, produto)"));
    assert!(tem(&h, "Chaves estrangeiras"));
    assert!(tem(&h, "→ pedidos.id") && tem(&h, "→ produtos.codigo"));
    // as duas colunas da chave primária composta não se excluem
    assert_eq!(na_linha(&h, "pedido_id", &rotulo_da_acao("Excluir", "pedido_id")), 0);
    assert_eq!(na_linha(&h, "quantidade", &rotulo_da_acao("Excluir", "quantidade")), 1);
    capturar(&mut h, "estrutura-itens-pedido-claro.png");
}

#[test]
fn enter_no_campo_aplica() {
    let amb = Ambiente::exemplo("estrutura_enter");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "momento");
    clicar(&mut h, "Nova coluna");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    h.get_by_role_and_label(Role::TextInput, "Nome").type_text("extra");
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    esperar(&mut h, |h| recargas(h) == 1);
    assert!(colunas_do_arquivo(&caminho, "eventos").contains(&"extra".to_string()));
}

#[test]
fn estrutura_no_tema_escuro() {
    let amb = Ambiente::exemplo("estrutura_escuro");
    let mut h = palco_doc::<AbaEstrutura>(&amb, "clientes", Tema::Escuro, egui::vec2(1440.0, 900.0));
    esperar_texto(&mut h, "criado_em");
    clicar_linha(&mut h, "email");
    h.run_steps(3);
    capturar(&mut h, "estrutura-clientes-escuro.png");
    clicar(&mut h, "cidade");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    esperar_previa(&mut h, janela_coluna, "Nada muda");
    capturar(&mut h, "janela-coluna-alterar-escuro.png");
}
