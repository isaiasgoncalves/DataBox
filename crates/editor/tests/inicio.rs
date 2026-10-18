//! O Início da prancheta 4 com a casca inteira (o `Editor` no `egui_kittest`), na janela padrão de 1280×800: os
//! recentes de cada tipo (a pasta, os bancos, o CSV e o que sumiu), as capturas `inicio-{claro,escuro}.png` e a
//! contagem por tipo em Preferências › Geral. O desenho e as ações de cada gesto, no palco, ficam nos testes do
//! módulo (`src/inicio/testes.rs`).

mod apoio;

use apoio::*;
use editor::Partida;
use egui::accesskit::Role;
use egui_kittest::kittest::{Queryable, by};
use estado::Rota;
use estado::apoio_testes::Ambiente;
use interface::tema::Tema;
use prefs::TipoRecente;

/// A janela padrão (main.rs, `TAMANHO_INICIAL`).
const JANELA_PADRAO: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Os recentes da prancheta 4, criados agora (do mais novo ao mais velho): a pasta `dados`, o `exemplo.db` e o
/// `vendas_2025.csv` dentro dela, o `financeiro.sqlite` e o `arquivo_antigo.db`, que some do disco.
fn recentes_da_prancheta(amb: &Ambiente) {
    let dados = amb.pasta().join("dados");
    std::fs::create_dir_all(&dados).expect("criar a pasta dados");
    let antigo = amb.pasta().join("arquivo_antigo.db");
    let financeiro = amb.pasta().join("financeiro.sqlite");
    criar_banco(&antigo, "CREATE TABLE t (id INTEGER PRIMARY KEY);");
    criar_banco(&financeiro, "CREATE TABLE lancamentos (id INTEGER PRIMARY KEY, valor REAL);");
    let csv = dados.join("vendas_2025.csv");
    let linhas: String = (1..=400).map(|i| format!("{i};produto {i};{},50\n", i * 3)).collect();
    std::fs::write(&csv, format!("id;produto;valor\n{linhas}")).expect("gravar o CSV");
    let exemplo = amostras::criar_banco_exemplo(dados.join("exemplo.db")).expect("exemplo.db");
    let prefs = amb.servico.prefs();
    prefs.adicionar_recente(&antigo);
    prefs.adicionar_recente(&financeiro);
    prefs.adicionar_recente_de(&csv, TipoRecente::Arquivo);
    prefs.adicionar_recente(&exemplo);
    prefs.adicionar_recente_de(&dados, TipoRecente::Pasta);
    std::fs::remove_file(&antigo).expect("apagar o antigo");
}

/// O Início sem banco nem pasta, com os recentes da prancheta, na janela padrão.
fn cena(nome: &str, tema: Tema) -> (Ambiente, H) {
    let amb = Ambiente::vazio(nome);
    recentes_da_prancheta(&amb);
    let h = harness_com(&amb, tema, JANELA_PADRAO, Partida::default(), |_| None);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    (amb, h)
}

fn conferir_a_prancheta(h: &H) {
    // a cabeça e os três cartões, o primeiro é o Abrir pasta
    assert!(existe(h, Role::Label, "DataBox"), "{:?}", textos(h));
    assert!(tem_texto(h, "Abra uma pasta para trabalhar com vários arquivos, ou um arquivo só"));
    let doc = documento(h);
    let cartoes: Vec<egui::Rect> = ["Abrir pasta", "Abrir arquivo", "Novo banco"]
        .iter()
        .map(|r| {
            h.query_all(by().role(Role::Button).label(r))
                .map(|n| n.rect())
                .find(|c| c.width() == interface::pecas::acao::LARGURA_ACAO && doc.contains_rect(*c))
                .unwrap_or_else(|| panic!("o cartão {r}: {:?}", textos(h)))
        })
        .collect();
    assert_eq!(cartoes[0].left(), doc.left() + 1.0 + 36.0, "{cartoes:?} {doc:?}");
    assert!(cartoes.windows(2).all(|p| p[0].top() == p[1].top() && p[1].left() - p[0].right() == 12.0));
    // o caminho e a caixa
    assert!(existe(h, Role::TextInput, "Caminho da pasta ou do arquivo"));
    assert!(existe(h, Role::CheckBox, "Abrir bancos só para leitura"));
    // os recentes: cada nome que existe abre; "pasta" na coluna Tamanho; o que sumiu com a pill
    for nome in ["dados", "exemplo.db", "vendas_2025.csv", "financeiro.sqlite"] {
        assert!(
            h.query_all(by().role(Role::Button).label(nome)).any(|n| doc.contains_rect(n.rect())),
            "{nome} abre pelo nome: {:?}",
            textos(h)
        );
    }
    assert!(existe(h, Role::Label, "arquivo_antigo.db"));
    assert!(existe(h, Role::Label, "pasta"));
    assert!(tem_texto(h, "não encontrado"));
    // a tabela inteira cabe na janela padrão, sem rolar
    let ultima = rect_de(h, Role::Button, "Remover arquivo_antigo.db da lista");
    assert!(ultima.bottom() < doc.bottom() && ultima.right() <= doc.right() - 1.0 - 36.0, "{ultima:?} {doc:?}");
}

#[test]
fn inicio_pela_prancheta_4_claro() {
    let (_amb, mut h) = cena("inicio_prancheta_claro", Tema::Claro);
    conferir_a_prancheta(&h);
    capturar(&mut h, "inicio-claro.png");
}

#[test]
fn inicio_pela_prancheta_4_escuro() {
    let (_amb, mut h) = cena("inicio_prancheta_escuro", Tema::Escuro);
    conferir_a_prancheta(&h);
    capturar(&mut h, "inicio-escuro.png");
}

/// Preferências › Geral conta os itens da lista de Abertos recentemente pelo tipo.
#[test]
fn limpar_a_lista_conta_itens() {
    let (amb, mut h) = cena("inicio_limpar_conta", Tema::Claro);
    abrir_menu_arquivo(&mut h);
    clicar_item(&mut h, "Preferências");
    assentar(&mut h);
    // o arquivo_antigo.db sumiu, mas continua na lista (e conta como banco)
    assert!(tem_texto(&h, "5 itens na lista: 3 bancos, 1 pasta e 1 arquivo"), "{:?}", textos(&h));
    clicar_botao(&mut h, "Limpar a lista");
    clicar_ligado(&mut h, "Salvar");
    assentar(&mut h);
    assert!(amb.servico.prefs().recentes().is_empty(), "os três tipos saem");
    abrir_menu_arquivo(&mut h);
    clicar_item(&mut h, "Preferências");
    assentar(&mut h);
    assert!(tem_texto(&h, "Nenhum item na lista"), "{:?}", textos(&h));
}

/// O clique num banco dos recentes abre o banco pela casca (o `AbrirBanco` de sempre).
#[test]
fn clique_no_banco_dos_recentes_abre_o_banco() {
    let (_amb, mut h) = cena("inicio_recente_banco", Tema::Claro);
    clicar_botao(&mut h, "financeiro.sqlite");
    esperar(&mut h, "o financeiro abrir", |h| {
        h.state().estado().banco.as_ref().is_some_and(|b| b.nome == "financeiro.sqlite")
    });
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Banco);
}
