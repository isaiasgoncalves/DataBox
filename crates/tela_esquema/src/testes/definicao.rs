//! Aba Definição SQL (script, links para a Consulta SQL e "Excluir visão"), no formato de
//! documento: a faixa com o Copiar, o texto nas cores do código, o Excluir da visão na barra e os comandos prontos nas
//! Propriedades.

use estado::apoio_testes::{Ambiente, esperar};
use estado::{Acao, ParteTabela, Rota, Verbo};
use interface::pecas::toast::toast_atual;

use super::apoio::*;
use crate::AbaDefinicao;

type Doc = NoPalcoDoc<AbaDefinicao>;

fn script(h: &H<Doc>) -> String {
    h.state().tela.doc.aba.script().to_string()
}

fn ofertas(h: &H<Doc>, amb: &Ambiente) -> estado::Ofertas {
    let d = &h.state().tela.doc;
    d.aba.ofertas(&d.tabela, &amb.estado)
}

#[test]
fn definicao_mostra_script_e_links_para_a_consulta() {
    let amb = Ambiente::exemplo("definicao_pedidos");
    let mut h = palco_doc_claro::<AbaDefinicao>(&amb, "pedidos");
    esperar_texto(&mut h, "SELECT * FROM pedidos LIMIT 100");
    let script = script(&h);
    assert!(script.contains("CREATE TABLE pedidos ("), "{script}");
    assert!(script.contains("CREATE INDEX idx_pedidos_status_data ON pedidos (status, feito_em DESC);"), "{script}");
    assert!(script.contains("CREATE TRIGGER trg_pedido_cancelado"), "{script}");
    // a faixa: o título e o Copiar na ponta; o card "Abrir na Consulta SQL" virou a seção das Propriedades
    assert!(tem(&h, "Comando de criação") && tem(&h, "Copiar"));
    assert!(tem(&h, "Seção Abrir na Consulta SQL"));
    assert!(!tem(&h, "Excluir visão") && !ligado_na_barra(&h, "Excluir"));
    assert!(tem(&h, "SELECT * FROM pedidos LIMIT 100"));
    assert!(tem(&h, "SELECT count(*) FROM pedidos"));
    // o Copiar fica na ponta direita da faixa
    assert!(no(&h, "Copiar").rect().right() > 1440.0 - 272.0 - 8.0 - 60.0 - 20.0);
    clicar(&mut h, "SELECT * FROM pedidos LIMIT 100");
    assert_eq!(acoes(&h), [Acao::Navegar(Rota::consulta_com("SELECT * FROM pedidos LIMIT 100"))]);
    clicar(&mut h, "Copiar");
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Copiado."));

    let mut h = palco_doc_claro::<AbaDefinicao>(&amb, "Tabela com espaço");
    esperar_texto(&mut h, "SELECT * FROM \"Tabela com espaço\" LIMIT 100");
}

#[test]
fn definicao_de_clientes() {
    let amb = Ambiente::exemplo("definicao_clientes");
    let mut h = palco_doc_claro::<AbaDefinicao>(&amb, "clientes");
    esperar_texto(&mut h, "SELECT * FROM clientes LIMIT 100");
    let script = script(&h);
    assert!(
        script.contains("CREATE UNIQUE INDEX idx_clientes_nome_cidade ON clientes (nome, cidade) WHERE ativo = 1;")
    );
    // a busca da barra fica desligada; numa tabela, nada se exclui aqui
    let ofertas = ofertas(&h, &amb);
    assert!(!ofertas.ligado(Verbo::Buscar) && !ofertas.ligado(Verbo::Excluir));
    capturar(&mut h, "definicao-clientes-claro.png");
}

#[test]
fn definicao_da_visao_tem_excluir_visao() {
    let amb = Ambiente::exemplo("definicao_visao");
    let caminho = amb.caminho_banco();
    let mut h = palco_doc_claro::<AbaDefinicao>(&amb, "v_pedidos_por_cliente");
    esperar_texto(&mut h, "SELECT * FROM v_pedidos_por_cliente LIMIT 100");
    assert!(script(&h).contains("CREATE VIEW v_pedidos_por_cliente AS"));
    // o Excluir da visão é o da barra, com a dica que diz qual
    assert!(ligado_na_barra(&h, "Excluir"));
    assert_eq!(ofertas(&h, &amb).dica(Verbo::Excluir), "Excluir a visão v_pedidos_por_cliente");
    capturar(&mut h, "definicao-visao-claro.png");
    clicar(&mut h, "Excluir");
    esperar(&mut h, |h| dialogos(h) == ["Excluir visão"]);
    let frase = frase_da_confirmacao(&h).expect("frase");
    assert!(frase.contains("Excluir a visão v_pedidos_por_cliente?"), "{frase}");
    clicar_no_dialogo(&mut h, "Excluir");
    esperar(&mut h, |h| h.state().acoes.iter().any(|a| matches!(a, Acao::Navegar(_))));
    assert!(acoes(&h).ends_with(&[Acao::RecarregarObjetos, Acao::Navegar(Rota::Banco)]));
    assert!(objeto(&caminho, "view", "v_pedidos_por_cliente").is_none());
}

#[test]
fn visao_no_banco_so_leitura_nao_tem_excluir_visao() {
    let mut amb = Ambiente::exemplo("definicao_visao_leitura");
    let caminho = amb.caminho_banco();
    amb.servico.abrir_banco(&caminho, true, false).expect("abrir só leitura");
    amb.recarregar();
    let mut h = palco_doc_claro::<AbaDefinicao>(&amb, "v_pedidos_por_cliente");
    esperar_texto(&mut h, "Copiar");
    assert!(!ofertas(&h, &amb).ligado(Verbo::Excluir));
    assert!(!ligado_na_barra(&h, "Excluir"));
}

#[test]
fn tabela_interna_sem_comando_de_criacao() {
    let amb = Ambiente::exemplo("definicao_interna");
    let mut h = palco_doc_claro::<AbaDefinicao>(&amb, "sqlite_sequence");
    esperar_texto(&mut h, "SELECT * FROM sqlite_sequence LIMIT 100");
    // a sqlite_sequence tem o CREATE no sqlite_schema; as sombras do FTS também
    assert!(
        tem(&h, "Copiar") || tem(&h, "O SQLite não guarda o comando de criação deste objeto (é uma tabela interna).")
    );
}
