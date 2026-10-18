//! Janela Nova tabela (criar pela janela, cabeçalho alinhado, linha em branco e a frase da
//! coluna sem nome).

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use estado::apoio_testes::{Ambiente, esperar};
use estado::{Aba, Acao, Rota};
use interface::pecas::toast::toast_atual;

use super::apoio::*;
use crate::JanelaNovaTabela;

fn mudanca(t: &TelaJanela<JanelaNovaTabela>) -> Option<&crate::mudanca::Mudanca> {
    Some(&t.janela.mudanca)
}

#[test]
fn criar_tabela_pela_janela() {
    let amb = Ambiente::exemplo("nova_tabela_criar");
    let caminho = amb.caminho_banco();
    let mut h = palco_nova_tabela(&amb);
    esperar(&mut h, |h| dialogos(h) == ["Nova tabela"]);
    assert_eq!(valor(&h, "Nome da coluna 1"), "id");
    assert_eq!(valor(&h, "Tipo da coluna 1"), "INTEGER");
    assert!(h.state().tela.janela.campos.linhas[0].pk);
    assert!(!tem(&h, "Tirar a coluna id"), "o × fica escondido com uma linha só");
    esperar_previa(&mut h, mudanca, "-- Digite o nome da tabela para ver o SQL.");
    assert!(h.query_by_role_and_label(Role::TextInput, "Nome da tabela").is_some_and(|n| n.is_focused()));
    capturar(&mut h, "nova-tabela-inicial-claro.png");

    preencher(&mut h, "Nome da tabela", "fornecedores");
    clicar_no_dialogo(&mut h, "+ Coluna");
    assert_eq!(h.state().tela.janela.campos.linhas.len(), 2);
    assert!(h.query_by_role_and_label(Role::TextInput, "Nome da coluna 2").is_some_and(|n| n.is_focused()));
    h.get_by_role_and_label(Role::TextInput, "Nome da coluna 2").type_text("razao_social");
    h.run_steps(2);
    clicar_caixa(&mut h, "razao_social: não aceita nulo");
    clicar_no_dialogo(&mut h, "+ Coluna");
    h.get_by_role_and_label(Role::TextInput, "Nome da coluna 3").type_text("rascunho");
    h.run_steps(2);
    assert_eq!(h.query_all_by_label_contains("Tirar a coluna").count(), 3);
    clicar(&mut h, "Tirar a coluna rascunho");
    assert_eq!(h.state().tela.janela.campos.linhas.len(), 2);
    esperar_previa(&mut h, mudanca, "\"razao_social\" TEXT NOT NULL");
    {
        let m = &h.state().tela.janela.mudanca;
        assert!(m.texto.contains("CREATE TABLE \"fornecedores\""));
        assert!(!m.texto.contains("rascunho"));
        assert!(m.texto_do_slot().is_some_and(|s| s.0.contains("Criar a tabela fornecedores com 2 colunas")));
    }
    capturar(&mut h, "nova-tabela-claro.png");

    clicar_no_dialogo(&mut h, "Criar tabela");
    esperar(&mut h, |h| !h.state().tela.aberta);
    assert!(toast_atual(&h.ctx).is_some());
    assert!(acoes(&h).ends_with(&[
        Acao::RecarregarObjetos,
        Acao::Navegar(Rota::objeto_na_aba("fornecedores", false, Aba::Estrutura))
    ]));
    let sql = objeto(&caminho, "table", "fornecedores").expect("tabela criada");
    assert!(sql.contains("\"razao_social\" TEXT NOT NULL") && sql.contains("\"id\" INTEGER PRIMARY KEY"), "{sql}");
}

/// Clica na caixa com este rótulo ("razao_social: não aceita nulo").
fn clicar_caixa<S>(h: &mut H<S>, rotulo: &str) {
    let rect = mirar(h, rotulo, |h| h.query_by_role_and_label(Role::CheckBox, rotulo).map(|n| n.rect()));
    clicar_em(h, rect);
}

#[test]
fn nova_tabela_cabecalho_alinhado_e_linha_em_branco() {
    let amb = Ambiente::exemplo("nova_tabela_em_branco");
    let caminho = amb.caminho_banco();
    let mut h = palco_nova_tabela(&amb);
    esperar(&mut h, |h| !dialogos(h).is_empty());
    // cada rótulo fica sobre o seu campo: Nome, Tipo e Padrão à esquerda dele; os três últimos, centrados na caixa
    let esquerda = |h: &H<TelaJanela<JanelaNovaTabela>>, rotulo: &str| no(h, rotulo).rect().left().round();
    assert_eq!(esquerda(&h, "Nome"), esquerda(&h, "Nome da coluna 1"));
    assert_eq!(esquerda(&h, "Tipo"), esquerda(&h, "Tipo da coluna 1"));
    assert_eq!(esquerda(&h, "Padrão"), esquerda(&h, "Valor padrão da coluna 1"));
    let centro = |h: &H<TelaJanela<JanelaNovaTabela>>, rotulo: &str| no(h, rotulo).rect().center().x;
    for (rotulo, caixa) in
        [("Chave primária", "id: chave primária"), ("Não aceita nulo", "id: não aceita nulo"), ("Único", "id: único")]
    {
        assert!(h.query_by_role_and_label(Role::CheckBox, caixa).is_some(), "{caixa}");
        assert!(
            (centro(&h, rotulo) - centro(&h, caixa)).abs() < 1.5,
            "{rotulo}: {} e {}",
            centro(&h, rotulo),
            centro(&h, caixa)
        );
    }
    // os campos de 26px e as linhas de 34px (`.campo.pq` e o `gap:8px`)
    assert_eq!(no(&h, "Nome da coluna 1").rect().height().round(), 26.0);

    // "+ Coluna" sem usar: a linha em branco (só com o tipo que ela já traz) fica de fora
    preencher(&mut h, "Nome da tabela", "com_linha_em_branco");
    clicar_no_dialogo(&mut h, "+ Coluna");
    esperar_previa(&mut h, mudanca, "CREATE TABLE \"com_linha_em_branco\"");
    assert!(h.state().tela.janela.mudanca.texto_do_slot().is_some_and(|(_, perigo)| !perigo));
    clicar_no_dialogo(&mut h, "Criar tabela");
    esperar(&mut h, |h| !h.state().tela.aberta);
    assert_eq!(colunas_do_arquivo(&caminho, "com_linha_em_branco"), ["id"]);

    // linha com algo preenchido e sem nome: a frase diz qual é, uma vez só (no alto da janela, não também na
    // prévia)
    let mut h = palco_nova_tabela(&amb);
    esperar(&mut h, |h| !dialogos(h).is_empty());
    preencher(&mut h, "Nome da tabela", "sem_nome_na_coluna");
    clicar_no_dialogo(&mut h, "+ Coluna");
    preencher(&mut h, "Valor padrão da coluna 2", "0");
    let frase = "A coluna 2 precisa de um nome (ou tire a linha com ✕).";
    esperar(&mut h, |h| h.state().tela.janela.mudanca.texto_do_slot() == Some((frase.to_string(), true)));
    assert!(tem(&h, frase));
    capturar(&mut h, "nova-tabela-erro-previa-claro.png");
    clicar_no_dialogo(&mut h, "Criar tabela");
    esperar(&mut h, |h| {
        let m = &h.state().tela.janela.mudanca;
        m.aviso == Some((frase.to_string(), true)) && m.texto_do_slot().is_none() && !m.previa_pendente()
    });
    assert_eq!(contar(&h, frase), 1);
    capturar(&mut h, "nova-tabela-erro-alto-claro.png");
    assert!(h.state().tela.aberta);
    assert!(objeto(&caminho, "table", "sem_nome_na_coluna").is_none());
}

#[test]
fn tirar_a_linha_poe_o_foco_na_vizinha() {
    let amb = Ambiente::exemplo("nova_tabela_tirar");
    let mut h = palco_nova_tabela(&amb);
    esperar(&mut h, |h| !dialogos(h).is_empty());
    clicar_no_dialogo(&mut h, "+ Coluna");
    clicar_no_dialogo(&mut h, "+ Coluna");
    assert_eq!(h.state().tela.janela.campos.linhas.len(), 3);
    clicar(&mut h, "Tirar a coluna 2");
    assert_eq!(h.state().tela.janela.campos.linhas.len(), 2);
    assert!(h.query_by_role_and_label(Role::TextInput, "Nome da coluna 2").is_some_and(|n| n.is_focused()));
    clicar(&mut h, "Tirar a coluna 2");
    assert_eq!(h.state().tela.janela.campos.linhas.len(), 1);
    assert!(!tem(&h, "Tirar a coluna id"));
    clicar_no_dialogo(&mut h, "Fechar");
    assert!(!h.state().tela.aberta);
}

/// Com o corpo da janela rolado até a prévia, o erro do "Criar tabela" aparece no alto do corpo e o corpo rola até
/// ele (o `avisoEl.scrollIntoView({block:"nearest"})`): quem rolou não fica sem retorno.
#[test]
fn erro_no_alto_com_o_corpo_rolado_aparece() {
    let amb = Ambiente::exemplo("nova_tabela_rolada");
    let mut h = palco_nova_tabela(&amb);
    esperar(&mut h, |h| !dialogos(h).is_empty());
    preencher(&mut h, "Nome da tabela", "muitas");
    for _ in 0..7 {
        clicar_no_dialogo(&mut h, "+ Coluna");
    }
    assert_eq!(h.state().tela.janela.campos.linhas.len(), 8);
    preencher(&mut h, "Valor padrão da coluna 2", "0");
    let frase = "A coluna 2 precisa de um nome (ou tire a linha com ✕).";
    esperar(&mut h, |h| h.state().tela.janela.mudanca.texto_do_slot() == Some((frase.to_string(), true)));
    // numa tela mais baixa, o corpo da janela rola
    h.set_size(egui::vec2(1440.0, 600.0));
    assentar(&mut h);
    // o corpo rola até o fim (a prévia)
    let campo = h.get_by_role_and_label(Role::TextInput, "Valor padrão da coluna 2").rect();
    rolar_com_roda(&mut h, campo.center(), -400.0, 10);
    // o corpo vai do fim do cabeçalho (o ✕ e 15px) ao começo do rodapé ("Criar tabela" e 14px acima dele)
    let (fechar, criar) = (no(&h, "Fechar").rect(), no(&h, "Criar tabela").rect());
    let corpo =
        egui::Rect::from_x_y_ranges(fechar.left()..=fechar.right(), fechar.bottom() + 15.0..=criar.top() - 14.0);
    assert!(no(&h, "Nome da tabela").rect().bottom() < corpo.top(), "o alto do corpo saiu da vista");
    capturar(&mut h, "nova-tabela-rolada-claro.png");
    clicar_no_dialogo(&mut h, "Criar tabela");
    esperar(&mut h, |h| h.state().tela.janela.mudanca.aviso == Some((frase.to_string(), true)));
    assentar(&mut h);
    // o texto do aviso não tem retângulo na árvore de acessibilidade: confere pelo que vem logo depois dele (o
    // aviso abre o corpo; o campo Nome da tabela vem 14px abaixo, com o rótulo): rolado o mínimo para o aviso
    // aparecer, o campo volta à vista logo abaixo dele
    let campo = no(&h, "Nome da tabela").rect();
    assert!(campo.top() >= corpo.top() + 40.0 && campo.bottom() <= corpo.top() + 140.0, "{campo:?} em {corpo:?}");
    assert!(h.state().tela.aberta);
    capturar(&mut h, "nova-tabela-erro-rolado-claro.png");
}

/// A janela no tema escuro, com a chave marcada em "Sem rowid" (o ícone do cabeçalho vira a chave anil, D6) e uma
/// coluna a mais: a captura da prancheta 12c no escuro.
#[test]
fn nova_tabela_no_tema_escuro() {
    let amb = Ambiente::exemplo("nova_tabela_escura");
    let mut h = palco_nova_tabela_tema(&amb, interface::tema::Tema::Escuro);
    esperar(&mut h, |h| dialogos(h) == ["Nova tabela"]);
    preencher(&mut h, "Nome da tabela", "contratos");
    clicar_no_dialogo(&mut h, "+ Coluna");
    h.get_by_role_and_label(Role::TextInput, "Nome da coluna 2").type_text("valor");
    h.run_steps(2);
    preencher(&mut h, "Tipo da coluna 2", "REAL");
    h.key_press(egui::Key::Escape); // fecha a lista de sugestões
    h.run_steps(2);
    clicar_caixa(&mut h, "Sem rowid (WITHOUT ROWID)");
    esperar_previa(&mut h, mudanca, "WITHOUT ROWID");
    capturar(&mut h, "nova-tabela-escuro.png");
}
