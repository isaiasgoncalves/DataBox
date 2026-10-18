//! Aba Índices e gatilhos e janela Novo índice (com a sugestão de índice), no
//! formato de documento: a faixa, a lista com a seleção, os gatilhos nas cores do código, o Novo índice e o Excluir da
//! barra e as Propriedades.

use egui_kittest::kittest::Queryable;
use estado::apoio_testes::{Ambiente, esperar};
use estado::{Acao, ParteTabela, Verbo};
use interface::pecas::toast::toast_atual;
use interface::tema::Tema;

use super::apoio::*;
use crate::AbaIndices;
use crate::indices::ROTULO_EXCLUIR_INDICE;

type Doc = NoPalcoDoc<AbaIndices>;

fn aba(h: &H<Doc>) -> &AbaIndices {
    &h.state().tela.doc.aba
}

fn recargas(h: &H<Doc>) -> usize {
    h.state().tela.doc.recargas
}

fn janela_indice(t: &Doc) -> Option<&crate::mudanca::Mudanca> {
    t.doc.aba.janela().map(|j| &j.mudanca)
}

/// O palco no claro, a 1440 × 900, com os índices já lidos.
fn palco(amb: &Ambiente, tabela: &str, ancora: &str) -> Teste<Doc> {
    let mut h = palco_doc_claro::<AbaIndices>(amb, tabela);
    esperar_texto(&mut h, ancora);
    h
}

/// Clica na linha do índice `nome` (no texto do nome, fora dos botões): seleciona.
fn clicar_linha<S>(h: &mut H<S>, nome: &str) {
    let rect = mirar(h, nome, |h| h.query_all_by_label(nome).next().map(|n| n.rect()));
    clicar_em(h, rect);
    h.run_steps(3);
}

#[test]
fn indices_automaticos_sem_excluir_e_condicao_do_parcial() {
    let amb = Ambiente::exemplo("indices_clientes");
    let mut h = palco(&amb, "clientes", "idx_clientes_nome_cidade");
    // a faixa: o título e a contagem; os índices na lista, sem o card
    assert!(tem(&h, "Índices") && tem(&h, "2 índices"));
    let auto = "sqlite_autoindex_clientes_1";
    assert_eq!(na_linha(&h, auto, "único"), 1);
    assert_eq!(na_linha(&h, auto, "de restrição UNIQUE"), 1);
    assert_eq!(na_linha(&h, auto, ROTULO_EXCLUIR_INDICE), 0);
    let parcial = "idx_clientes_nome_cidade";
    assert_eq!(na_linha(&h, parcial, "nome, cidade"), 1);
    assert_eq!(na_linha(&h, parcial, "ativo = 1"), 1);
    assert_eq!(na_linha(&h, parcial, ROTULO_EXCLUIR_INDICE), 1);
    assert!(!tem(&h, "+ Novo índice") && !tem(&h, "2 índices · só leitura"));
    // o título dos gatilhos e o vazio
    assert!(tem(&h, "Gatilhos"));
    assert!(tem(&h, "Nenhum gatilho nesta tabela. Gatilhos são criados pela Consulta SQL com CREATE TRIGGER."));
    // a linha de 32px das listas
    assert!(((y_de(&h, parcial) - y_de(&h, auto)).abs() - 32.0).abs() < 1.0);
    // a busca da barra fica desligada nesta parte
    let ofertas = {
        let d = &h.state().tela.doc;
        d.aba.ofertas(&d.tabela, &amb.estado)
    };
    assert!(!ofertas.ligado(Verbo::Buscar) && ofertas.ligado(Verbo::Novo));
    clicar_linha(&mut h, parcial);
    capturar(&mut h, "indices-clientes-claro.png");
}

#[test]
fn selecionar_um_indice_mostra_a_secao_dele_e_liga_o_excluir() {
    let amb = Ambiente::exemplo("indices_selecao");
    let mut h = palco(&amb, "clientes", "idx_clientes_nome_cidade");
    assert!(aba(&h).selecionado().is_none() && !tem(&h, "Seção Índice"));
    assert!(!ligado_na_barra(&h, "Excluir"), "sem seleção, nada a excluir");
    clicar_linha(&mut h, "idx_clientes_nome_cidade");
    assert_eq!(aba(&h).selecionado(), Some("idx_clientes_nome_cidade"));
    assert!(dialogos(&h).is_empty(), "o clique na linha só seleciona");
    // a seção do índice: o nome, as colunas e a condição aparecem de novo (a lista e a ficha), e o Único e a origem
    assert!(tem(&h, "Seção Índice"));
    assert_eq!(contar(&h, "idx_clientes_nome_cidade"), 2);
    assert_eq!(contar(&h, "nome, cidade"), 2);
    assert_eq!(contar(&h, "ativo = 1"), 2);
    assert!(tem(&h, "sim") && tem(&h, "criado"));
    assert!(ligado_na_barra(&h, "Excluir"));
    let ofertas = {
        let d = &h.state().tela.doc;
        d.aba.ofertas(&d.tabela, &amb.estado)
    };
    assert_eq!(ofertas.dica(Verbo::Excluir), "Excluir o índice idx_clientes_nome_cidade");
    // um índice automático seleciona, mas não se exclui
    clicar_linha(&mut h, "sqlite_autoindex_clientes_1");
    assert!(tem(&h, "de restrição UNIQUE") && !ligado_na_barra(&h, "Excluir"));
}

#[test]
fn indices_de_pedidos_com_o_gatilho_aberto() {
    let amb = Ambiente::exemplo("indices_pedidos");
    let mut h = palco(&amb, "pedidos", "idx_pedidos_status_data");
    assert!(tem(&h, "status, feito_em DESC"));
    assert!(tem(&h, "1 gatilho") && !tem(&h, "1 gatilho · clique no nome para ver o SQL"));
    assert!(!tem(&h, "Copiar"), "o gatilho nasce fechado");
    clicar(&mut h, "trg_pedido_cancelado");
    esperar_texto(&mut h, "Copiar");
    clicar_linha(&mut h, "idx_pedidos_status_data");
    capturar(&mut h, "indices-pedidos-claro.png");
    clicar(&mut h, "Copiar");
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Copiado."));
}

/// O modelo do gatilho aberto não continua aberto numa aba nova.
#[test]
fn modelos_dos_gatilhos_nascem_fechados_em_cada_aba_nova() {
    let amb = Ambiente::exemplo("indices_modelo_fechado");
    let mut h = palco(&amb, "pedidos", "trg_pedido_cancelado");
    clicar(&mut h, "trg_pedido_cancelado");
    esperar_texto(&mut h, "Copiar");
    // a casca refaz a aba (a recarga depois de uma mudança, a volta à aba)
    h.state_mut().tela.doc.tabela.recarregar = true;
    esperar(&mut h, |h| recargas(h) == 1);
    h.run_steps(3);
    assert!(tem(&h, "trg_pedido_cancelado"));
    assert!(!tem(&h, "Copiar"), "o gatilho volta fechado");
}

#[test]
fn indices_de_pedidos_no_tema_escuro() {
    let amb = Ambiente::exemplo("indices_escuro");
    let mut h = palco_doc::<AbaIndices>(&amb, "pedidos", Tema::Escuro, egui::vec2(1440.0, 900.0));
    esperar_texto(&mut h, "trg_pedido_cancelado");
    clicar(&mut h, "trg_pedido_cancelado");
    esperar_texto(&mut h, "Copiar");
    clicar_linha(&mut h, "idx_pedidos_status_data");
    capturar(&mut h, "indices-pedidos-escuro.png");
}

#[test]
fn banco_so_leitura_nao_mostra_acoes_de_indices() {
    let mut amb = Ambiente::exemplo("indices_so_leitura");
    let caminho = amb.caminho_banco();
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só leitura");
    amb.recarregar();
    let mut h = palco(&amb, "pedidos", "idx_pedidos_status_data");
    assert!(tem(&h, "idx_pedidos_cliente"));
    assert!(tem(&h, "2 índices"));
    assert!(texto_com(&h, "aberto só para leitura").is_some(), "o motivo no aviso");
    let ofertas = {
        let d = &h.state().tela.doc;
        d.aba.ofertas(&d.tabela, &amb.estado)
    };
    assert!(!ofertas.ligado(Verbo::Novo) && !ofertas.ligado(Verbo::Excluir));
    assert!(!tem(&h, ROTULO_EXCLUIR_INDICE), "nenhum Excluir nas linhas");
    capturar(&mut h, "indices-leitura-claro.png");
    clicar(&mut h, "trg_pedido_cancelado");
    esperar_texto(&mut h, "Copiar");
    assert_eq!(contar(&h, "Copiar"), 1);
    assert_eq!(na_linha(&h, "Copiar", "Excluir"), 0, "o gatilho só se copia");
}

#[test]
fn indices_sem_edicao_nao_mostram_novo_indice() {
    let amb = Ambiente::exemplo("indices_sem_edicao");
    for (tabela, motivo) in [("notas", "Tabela virtual"), ("sqlite_sequence", "Tabela interna do SQLite")] {
        let mut h = palco_doc_claro::<AbaIndices>(&amb, tabela);
        esperar_texto(&mut h, motivo);
        let d = &h.state().tela.doc;
        assert!(!d.aba.ofertas(&d.tabela, &amb.estado).ligado(Verbo::Novo), "{tabela}");
    }
}

#[test]
fn criar_e_excluir_indice() {
    let amb = Ambiente::exemplo("indices_criar_excluir");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "eventos", "Nenhum índice nesta tabela.");
    capturar(&mut h, "indices-eventos-claro.png");
    // o Novo índice é o da barra
    assert!(ligado_na_barra(&h, "Novo índice"));
    clicar(&mut h, "Novo índice");
    esperar(&mut h, |h| dialogos(h) == ["Novo índice"]);
    esperar_previa(&mut h, janela_indice, "-- Marque ao menos uma coluna");
    clicar_no_dialogo(&mut h, "tipo");
    clicar_no_dialogo(&mut h, "momento");
    {
        let j = aba(&h).janela().expect("janela");
        let linhas = j.campos.linhas.as_ref().expect("colunas");
        let ordem = |nome: &str| linhas.iter().find(|l| l.nome == nome).map(|l| l.ordem.clone()).unwrap_or_default();
        assert_eq!(ordem("tipo"), "1ª");
        assert_eq!(ordem("momento"), "2ª");
        assert_eq!(ordem("valor"), "");
    }
    assert!(!tem(&h, "Ordem de valor"), "o ASC/DESC só aparece nas escolhidas");
    // DESC no momento: abre a lista e escolhe
    clicar_no_dialogo(&mut h, "Ordem de momento");
    clicar_no_dialogo(&mut h, "DESC (decrescente)");
    assert_eq!(valor(&h, "Nome"), "idx_eventos_tipo_momento");
    esperar_previa(
        &mut h,
        janela_indice,
        "CREATE INDEX \"idx_eventos_tipo_momento\" ON \"eventos\" (\"tipo\", \"momento\" DESC)",
    );

    clicar_no_dialogo(&mut h, "Criar índice");
    esperar(&mut h, |h| recargas(h) == 1);
    assert!(dialogos(&h).is_empty());
    assert!(acoes(&h).contains(&Acao::RecarregarObjetos));
    esperar_texto(&mut h, "idx_eventos_tipo_momento");
    assert_eq!(na_linha(&h, "idx_eventos_tipo_momento", "tipo, momento DESC"), 1);
    assert!(objeto(&caminho, "index", "idx_eventos_tipo_momento").is_some_and(|s| s.contains("\"momento\" DESC")));

    // o Excluir da linha (o ícone de lixeira)
    clicar_na_linha(&mut h, "idx_eventos_tipo_momento", ROTULO_EXCLUIR_INDICE);
    esperar(&mut h, |h| dialogos(h) == ["Excluir índice"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("Excluir o índice idx_eventos_tipo_momento?"), "{frase}");
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| recargas(h) == 2);
    assert!(!tem(&h, "idx_eventos_tipo_momento"));
    assert!(objeto(&caminho, "index", "idx_eventos_tipo_momento").is_none());
}

/// O Excluir da barra apaga o índice selecionado (com a mesma confirmação da linha).
#[test]
fn excluir_o_indice_selecionado_pela_barra() {
    let amb = Ambiente::exemplo("indices_excluir_barra");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "pedidos", "idx_pedidos_cliente");
    clicar_linha(&mut h, "idx_pedidos_cliente");
    clicar(&mut h, "Excluir");
    esperar(&mut h, |h| dialogos(h) == ["Excluir índice"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("Excluir o índice idx_pedidos_cliente?"), "{frase}");
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| recargas(h) == 1);
    assert!(objeto(&caminho, "index", "idx_pedidos_cliente").is_none());
    assert!(objeto(&caminho, "index", "idx_pedidos_status_data").is_some());
}

#[test]
fn nome_digitado_nao_e_trocado_pela_sugestao() {
    let amb = Ambiente::exemplo("indices_nome_digitado");
    let mut h = palco(&amb, "eventos", "Nenhum índice nesta tabela.");
    clicar(&mut h, "Novo índice");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    assert_eq!(valor(&h, "Nome"), "idx_eventos");
    preencher(&mut h, "Nome", "meu_indice");
    assentar_previa(&mut h, janela_indice);
    clicar_no_dialogo(&mut h, "valor");
    assert_eq!(valor(&h, "Nome"), "meu_indice");
    assert!(aba(&h).janela().is_some_and(|j| j.campos.nome_editado));
    // apagado, volta a seguir as colunas
    preencher(&mut h, "Nome", "");
    assentar_previa(&mut h, janela_indice);
    clicar_no_dialogo(&mut h, "tipo");
    assert_eq!(valor(&h, "Nome"), "idx_eventos_valor_tipo");
}

#[test]
fn excluir_gatilho() {
    let amb = Ambiente::exemplo("indices_excluir_gatilho");
    let caminho = amb.caminho_banco();
    let mut h = palco(&amb, "pedidos", "trg_pedido_cancelado");
    clicar(&mut h, "trg_pedido_cancelado");
    esperar_texto(&mut h, "Copiar");
    let sql = aba(&h).sql_gatilhos().first().cloned().unwrap_or_default();
    assert!(sql.contains("AFTER UPDATE OF status ON pedidos"), "{sql}");
    assert!(sql.ends_with(';'));
    clicar_na_linha(&mut h, "Copiar", "Excluir");
    esperar(&mut h, |h| dialogos(h) == ["Excluir gatilho"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("Excluir o gatilho trg_pedido_cancelado?"), "{frase}");
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| recargas(h) == 1);
    esperar_texto(&mut h, "Nenhum gatilho nesta tabela.");
    assert!(objeto(&caminho, "trigger", "trg_pedido_cancelado").is_none());
}

#[test]
fn novo_indice_de_pedidos() {
    let amb = Ambiente::exemplo("indices_novo_pedidos");
    let mut h = palco(&amb, "pedidos", "idx_pedidos_status_data");
    clicar(&mut h, "Novo índice");
    esperar(&mut h, |h| !dialogos(h).is_empty());
    clicar_no_dialogo(&mut h, "status");
    clicar_no_dialogo(&mut h, "feito_em");
    clicar_no_dialogo(&mut h, "Ordem de feito_em");
    clicar_no_dialogo(&mut h, "DESC (decrescente)");
    esperar_previa(
        &mut h,
        janela_indice,
        "CREATE INDEX \"idx_pedidos_status_feito_em\" ON \"pedidos\" (\"status\", \"feito_em\" DESC)",
    );
    capturar(&mut h, "novo-indice-claro.png");
}

/// A janela que a casca abre pela sugestão de índice da aba Dados: a coluna já marcada e o
/// nome sugerido; criado o índice, a janela fecha, pede a releitura dos objetos e diz que criou: a casca relê a
/// tabela aberta, que passa a ter o índice (a dica de ordenação lenta some).
#[test]
fn janela_da_sugestao_de_indice_ja_vem_com_a_coluna() {
    let amb = Ambiente::exemplo("indices_sugestao");
    let caminho = amb.caminho_banco();
    let mut h = palco_novo_indice(&amb, "clientes", &["cidade"], true);
    esperar(&mut h, |h| janela_da_casca(h).campos.linhas.is_some());
    h.run_steps(2);
    assert_eq!(dialogos(&h), ["Novo índice"]);
    assert!(marcada(&h, "cidade"));
    assert!(!marcada(&h, "nome"));
    assert_eq!(valor(&h, "Nome"), "idx_clientes_cidade");
    assert!(!janela_da_casca(&h).criou());
    assert_eq!(janela_da_casca(&h).tabela(), "clientes");
    esperar(&mut h, |h| janela_da_casca(h).mudanca.texto.contains("CREATE INDEX \"idx_clientes_cidade\""));
    capturar(&mut h, "novo-indice-clientes-claro.png");
    clicar_no_dialogo(&mut h, "Criar índice");
    esperar(&mut h, |h| !janela_aberta(h));
    assert_eq!(acoes(&h).last(), Some(&Acao::RecarregarObjetos));
    assert!(toast_atual(&h.ctx).is_some());
    assert!(objeto(&caminho, "index", "idx_clientes_cidade").is_some());
    assert_eq!(h.state().tela.recargas, 1, "a casca releu a tabela aberta");
    let tabela = h.state().tela.tabela.as_ref().expect("tabela aberta");
    assert!(tabela.info().indices.iter().any(|i| i.nome == "idx_clientes_cidade"));
}

/// Fechar a janela sem criar não relê nada.
#[test]
fn janela_da_sugestao_fechada_sem_criar_nao_rele_a_tabela() {
    let amb = Ambiente::exemplo("indices_sugestao_fechada");
    let mut h = palco_novo_indice(&amb, "clientes", &["cidade"], true);
    esperar(&mut h, |h| janela_da_casca(h).campos.linhas.is_some());
    assentar_previa(&mut h, |t: &CascaNovoIndice| t.janela.as_ref().map(|j| &j.mudanca));
    clicar_no_dialogo(&mut h, "Fechar");
    assert!(!janela_aberta(&h));
    assert_eq!(h.state().tela.recargas, 0);
    assert!(acoes(&h).is_empty());
}

#[test]
fn janela_de_tabela_que_nao_existe_avisa_no_alto() {
    let amb = Ambiente::exemplo("indices_tabela_inexistente");
    let mut h = palco_novo_indice(&amb, "nao_existe", &["x"], false);
    esperar(&mut h, |h| janela_da_casca(h).mudanca.aviso.as_ref().is_some_and(|(_, perigo)| *perigo));
    assert!(janela_da_casca(&h).campos.linhas.as_ref().is_some_and(Vec::is_empty));
    assert!(!tem(&h, "Lendo a tabela…"), "sem as colunas, a lista não fica carregando para sempre");
    clicar_no_dialogo(&mut h, "Criar índice");
    assert_eq!(
        janela_da_casca(&h).mudanca.aviso,
        Some(("Marque ao menos uma coluna para ver o SQL.".to_string(), true))
    );
    clicar_no_dialogo(&mut h, "Fechar");
    assert!(!janela_aberta(&h));
    assert!(acoes(&h).is_empty());
}
