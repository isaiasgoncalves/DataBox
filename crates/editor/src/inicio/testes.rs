//! O Início no palco (`estado::apoio_testes::palco_documento`, como a casca desenha um documento): o desenho da
//! prancheta 4 e as ações que cada gesto emite. Quem abre (a pasta, o caminho, o arquivo de dados) é a casca: aqui só
//! se confere o pedido. Os testes de ponta a ponta, com a casca, ficam em `tests/inicio.rs` e `tests/casca.rs`.

use std::path::PathBuf;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{Queryable, by};
use estado::apoio_testes::{Ambiente, NoPalco, Palco, assentar, esperar, palco_documento};
use interface::tema::Tema;

use super::*;

type H = Harness<'static, Palco<NoPalco<TelaInicio>>>;

/// O documento do Início na janela de 1280px (8 + a barra de ícones de 52 + 8 + o Explorador de 258 + 8, e 8 do
/// outro lado).
const LARGURA_DOCUMENTO: f32 = 1280.0 - 8.0 - 52.0 - 8.0 - 258.0 - 8.0 - 8.0;

fn montar(amb: &Ambiente) -> H {
    palco_documento(amb, TelaInicio::nova(None), Tema::Claro, egui::vec2(LARGURA_DOCUMENTO, 800.0))
}

fn rect(h: &H, papel: Role, rotulo: &str) -> egui::Rect {
    h.get(by().role(papel).label(rotulo)).rect()
}

fn clicar(h: &mut H, papel: Role, rotulo: &str) {
    h.get(by().role(papel).label(rotulo)).click();
    h.run_steps(2);
}

fn digitar_caminho(h: &mut H, texto: &str) {
    let campo = h.get_by_label(ROTULO_CAMINHO);
    campo.focus();
    campo.type_text(texto);
    h.run_steps(2);
}

/// Tira as ações emitidas até agora.
fn acoes(h: &mut H) -> Vec<Acao> {
    std::mem::take(&mut h.state_mut().acoes)
}

/// Algum nó tem o texto no rótulo ou no valor (o texto selecionável dos avisos vai em nós `TextRun`).
fn tem_texto(h: &H, texto: &str) -> bool {
    let alvo = texto.to_string();
    h.query_all_by(move |n| {
        n.label().is_some_and(|l| l.contains(alvo.as_str())) || n.value().is_some_and(|v| v.contains(alvo.as_str()))
    })
    .next()
    .is_some()
}

/// A borda de dentro do documento à esquerda: a borda de 1px e o `padding:36px` (o documento tem menos de 960px).
const ESQUERDA_DO_CONTEUDO: f32 = 1.0 + 36.0;

/// O caminho de um recente como as preferências o guardaram.
fn caminho_do_recente(h: &H, nome: &str) -> PathBuf {
    let r = h.state().estado.prefs.recentes.iter().find(|r| r.nome == nome).expect("o recente");
    PathBuf::from(&r.caminho)
}

/// Uma pasta de dados com uma pasta, um banco e dois arquivos de dados nos recentes (o mais novo primeiro: a pasta,
/// o banco, o CSV e o texto).
fn com_recentes_de_cada_tipo(nome: &str) -> Ambiente {
    let mut amb = Ambiente::com_pasta(nome, &[]);
    let dados = amb.pasta_de_dados();
    let prefs = amb.servico.prefs();
    prefs.adicionar_recente_de(dados.join("notas.txt"), TipoRecente::Arquivo);
    prefs.adicionar_recente_de(dados.join("clientes.csv"), TipoRecente::Arquivo);
    prefs.adicionar_recente(dados.join("exemplo.db"));
    prefs.adicionar_recente_de(&dados, TipoRecente::Pasta);
    amb.recarregar();
    amb
}

#[test]
fn caminho_digitado_sem_aspas_nem_espacos() {
    assert_eq!(limpar_caminho_digitado("  C:\\dados\\vendas.db  "), "C:\\dados\\vendas.db");
    assert_eq!(limpar_caminho_digitado("\"C:\\com espaço\\a.db\""), "C:\\com espaço\\a.db");
    assert_eq!(limpar_caminho_digitado("\""), "\"");
    assert_eq!(limpar_caminho_digitado("   "), "");
}

/// A prancheta 4: o nome e a frase, os três cartões de 220px lado a lado (`gap:12px`), a 36px da borda do documento;
/// a linha do caminho (até 560px) com a caixa ao lado, na altura do campo; e o título dos recentes.
#[test]
fn inicio_pela_prancheta_4() {
    let amb = Ambiente::vazio("inicio_prancheta_4");
    let h = montar(&amb);
    assert!(h.query(by().role(Role::Label).label("DataBox")).is_some());
    assert!(tem_texto(&h, "Abra uma pasta para trabalhar com vários arquivos, ou um arquivo só"));
    let nome = rect(&h, Role::Label, "DataBox");
    let cartoes = ["Abrir pasta", "Abrir arquivo", "Novo banco"].map(|r| rect(&h, Role::Button, r));
    // o documento começa em x = 0 no palco
    assert_eq!(cartoes[0].left(), ESQUERDA_DO_CONTEUDO, "{cartoes:?}");
    for (i, c) in cartoes.iter().enumerate() {
        assert_eq!(c.width(), interface::pecas::acao::LARGURA_ACAO, "{c:?}");
        assert_eq!(c.top(), cartoes[0].top(), "na mesma linha: {cartoes:?}");
        if i > 0 {
            assert_eq!(c.left() - cartoes[i - 1].right(), 12.0, "{cartoes:?}");
        }
    }
    assert!(nome.bottom() < cartoes[0].top(), "{nome:?} {cartoes:?}");
    // a linha do caminho: o campo e o "Abrir" até 560px; a caixa ao lado, na altura do campo
    let campo = rect(&h, Role::TextInput, ROTULO_CAMINHO);
    let abrir = h.query_all(by().role(Role::Button).label("Abrir")).last().expect("o Abrir").rect();
    let caixa = rect(&h, Role::CheckBox, ROTULO_SO_LEITURA);
    assert_eq!(campo.left(), cartoes[0].left());
    assert!(abrir.right() - campo.left() <= 560.0 + 0.5, "{campo:?} {abrir:?}");
    assert_eq!(abrir.left() - campo.right(), 8.0);
    assert!((caixa.center().y - campo.center().y).abs() <= 1.0, "{caixa:?} {campo:?}");
    assert_eq!(caixa.left() - abrir.right(), 20.0, "o gap de 20px até a caixa");
    assert!(cartoes[0].bottom() < campo.top());
    let titulo = rect(&h, Role::Label, "Abertos recentemente");
    assert!(campo.bottom() < titulo.top());
    assert!(tem_texto(&h, "Ou digite o caminho"));
    // o Início não tem o cabeçalho "Início" com o "i" de antes, nem o card "Abrir um banco"
    assert!(!tem_texto(&h, "Abrir um banco"));
    assert!(h.query(by().role(Role::Button).label("Ajuda")).is_none());
}

#[test]
fn abrir_pasta_arquivo_e_novo_banco_pelos_cartoes() {
    let amb = Ambiente::vazio("inicio_cartoes");
    let mut h = montar(&amb);
    clicar(&mut h, Role::Button, "Abrir pasta");
    assert_eq!(acoes(&mut h), [Acao::EscolherEAbrirPasta]);
    clicar(&mut h, Role::Button, "Abrir arquivo");
    assert_eq!(acoes(&mut h), [Acao::EscolherEAbrir { somente_leitura: false }]);
    clicar(&mut h, Role::Button, "Novo banco");
    assert_eq!(acoes(&mut h), [Acao::NovoBanco]);
    // a caixa vale para o banco que o Abrir arquivo escolher
    clicar(&mut h, Role::CheckBox, ROTULO_SO_LEITURA);
    clicar(&mut h, Role::Button, "Abrir arquivo");
    assert_eq!(acoes(&mut h), [Acao::EscolherEAbrir { somente_leitura: true }]);
    // o cartão da pasta é o primário e mostra o atalho; o Novo banco não tem atalho (a mesma altura de todos)
    let pasta = rect(&h, Role::Button, "Abrir pasta");
    let novo = rect(&h, Role::Button, "Novo banco");
    assert_eq!(pasta.height(), novo.height());
    // o teclado chega aos cartões: Enter no que tem o foco
    h.get(by().role(Role::Button).label("Abrir pasta")).focus();
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(acoes(&mut h), [Acao::EscolherEAbrirPasta]);
}

/// O campo aceita qualquer caminho: a casca é que diz se é pasta, banco ou arquivo de dados (`AbrirCaminho`).
#[test]
fn campo_do_caminho_aceita_pasta_e_arquivo() {
    let amb = Ambiente::com_pasta("inicio_campo", &[]);
    let dados = amb.pasta_de_dados();
    let mut h = montar(&amb);
    digitar_caminho(&mut h, &format!("  \"{}\"  ", dados.display()));
    clicar(&mut h, Role::Button, "Abrir");
    assert_eq!(acoes(&mut h), [Acao::AbrirCaminho { caminho: dados.clone(), somente_leitura: false }]);
    // um arquivo, com a caixa marcada e o Enter no campo
    clicar(&mut h, Role::CheckBox, ROTULO_SO_LEITURA);
    let campo = h.get_by_label(ROTULO_CAMINHO);
    campo.focus();
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    let csv = dados.join("clientes.csv");
    digitar_caminho(&mut h, &csv.display().to_string());
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(acoes(&mut h), [Acao::AbrirCaminho { caminho: csv, somente_leitura: true }]);
}

#[test]
fn caminho_vazio_mostra_o_erro_e_o_erro_da_casca_volta_ao_campo() {
    let amb = Ambiente::vazio("inicio_vazio");
    let mut h = montar(&amb);
    clicar(&mut h, Role::Button, "Abrir");
    h.run_steps(1);
    assert!(tem_texto(&h, MSG_CAMINHO_VAZIO));
    assert!(acoes(&mut h).is_empty(), "nada a abrir");
    // o erro de uma abertura pedida daqui (a casca chama o mostrar_erro) aparece no mesmo lugar
    h.state_mut().tela.doc.mostrar_erro("O arquivo não existe (ou foi movido): x.db.".to_string());
    h.run_steps(2);
    assert!(tem_texto(&h, "O arquivo não existe (ou foi movido): x.db."));
    // e sai com o pedido seguinte
    clicar(&mut h, Role::Button, "Abrir pasta");
    assert!(!tem_texto(&h, "O arquivo não existe"));
}

#[test]
fn recentes_com_icone_por_tipo_e_pasta_na_coluna_tamanho() {
    let amb = com_recentes_de_cada_tipo("inicio_recentes_tipo");
    let recentes = &amb.estado.prefs.recentes;
    let tipos: Vec<(&str, TipoRecente)> = recentes.iter().map(|r| (r.nome.as_str(), r.tipo)).collect();
    assert_eq!(
        tipos,
        [
            ("dados", TipoRecente::Pasta),
            ("exemplo.db", TipoRecente::Banco),
            ("clientes.csv", TipoRecente::Arquivo),
            ("notas.txt", TipoRecente::Arquivo)
        ]
    );
    let icones: Vec<Icone> = recentes.iter().map(icone_do_recente).collect();
    assert_eq!(
        icones,
        [Icone::PastaSolida, Icone::Banco, Icone::Folha(TipoArquivo::Csv), Icone::Folha(TipoArquivo::Txt)]
    );
    let h = montar(&amb);
    for nome in ["dados", "exemplo.db", "clientes.csv", "notas.txt"] {
        assert!(h.query(by().role(Role::Button).label(nome)).is_some(), "{nome} abre pelo nome");
    }
    // "pasta" na coluna Tamanho, só na linha da pasta; os arquivos com o tamanho deles
    let pastas: Vec<egui::Rect> = h.query_all(by().role(Role::Label).label("pasta")).map(|n| n.rect()).collect();
    assert_eq!(pastas.len(), 1, "{pastas:?}");
    let linha_da_pasta = rect(&h, Role::Button, "dados");
    assert!((pastas[0].center().y - linha_da_pasta.center().y).abs() <= 1.0);
    let tamanho_csv = interface::formato::formatar_bytes_opt(recentes[2].tamanho);
    let csv = rect(&h, Role::Button, "clientes.csv");
    // o ícone, o gap e o nome inteiro (sem "…")
    assert!(csv.width() > 16.0 + 8.0 + 50.0, "{csv:?}");
    let celula = h
        .query_all(by().role(Role::Label).label(tamanho_csv.as_str()))
        .map(|n| n.rect())
        .find(|r| (r.center().y - csv.center().y).abs() <= 1.0);
    assert!(celula.is_some_and(|c| c.left() > pastas[0].left() - 80.0), "o tamanho do CSV na linha dele");
    // as ações de cada linha, com o nome no rótulo (a dica é só o verbo)
    for nome in ["dados", "exemplo.db", "clientes.csv", "notas.txt"] {
        assert!(h.query(by().role(Role::Button).label(format!("Mostrar {nome} na pasta").as_str())).is_some());
        assert!(h.query(by().role(Role::Button).label(format!("Remover {nome} da lista").as_str())).is_some());
    }
    // os dois botões da linha (26px, sem espaço entre eles), o último a 6px da borda da moldura (1px)
    let mostrar = rect(&h, Role::Button, "Mostrar dados na pasta");
    let remover = rect(&h, Role::Button, "Remover dados da lista");
    assert_eq!((mostrar.width(), remover.width()), (26.0, 26.0));
    assert_eq!(remover.left(), mostrar.right());
    let moldura_direita = LARGURA_DOCUMENTO - ESQUERDA_DO_CONTEUDO;
    assert_eq!(remover.right(), moldura_direita - 1.0 - 6.0, "{remover:?}");
}

#[test]
fn clique_no_recente_abre_pelo_tipo() {
    let amb = com_recentes_de_cada_tipo("inicio_recentes_clique");
    let mut h = montar(&amb);
    let (dados, banco, csv) =
        (caminho_do_recente(&h, "dados"), caminho_do_recente(&h, "exemplo.db"), caminho_do_recente(&h, "clientes.csv"));
    clicar(&mut h, Role::Button, "dados");
    assert_eq!(acoes(&mut h), [Acao::AbrirPasta { caminho: dados }]);
    clicar(&mut h, Role::Button, "exemplo.db");
    assert_eq!(acoes(&mut h), [Acao::AbrirBanco { caminho: banco.clone(), somente_leitura: false }]);
    clicar(&mut h, Role::Button, "clientes.csv");
    assert_eq!(acoes(&mut h), [Acao::AbrirArquivoDeDados { caminho: csv }]);
    // a caixa "Abrir bancos só para leitura" vale para o banco
    clicar(&mut h, Role::CheckBox, ROTULO_SO_LEITURA);
    clicar(&mut h, Role::Button, "exemplo.db");
    assert_eq!(acoes(&mut h), [Acao::AbrirBanco { caminho: banco, somente_leitura: true }]);
}

#[test]
fn recente_que_sumiu_nao_abre() {
    let mut amb = Ambiente::vazio("inicio_recente_sumiu");
    let antigo = amb.pasta().join("arquivo_antigo.db");
    let pasta_velha = amb.pasta().join("pasta_velha");
    std::fs::write(&antigo, b"").expect("gravar");
    std::fs::create_dir_all(&pasta_velha).expect("criar a pasta");
    amb.servico.prefs().adicionar_recente(&antigo);
    amb.servico.prefs().adicionar_recente_de(&pasta_velha, TipoRecente::Pasta);
    std::fs::remove_file(&antigo).expect("apagar");
    std::fs::remove_dir(&pasta_velha).expect("apagar a pasta");
    amb.recarregar();
    assert!(amb.estado.prefs.recentes.iter().all(|r| !r.existe));
    let mut h = montar(&amb);
    for nome in ["arquivo_antigo.db", "pasta_velha"] {
        // o nome é um texto, não um botão: o clique não pede nada
        assert!(h.query(by().role(Role::Button).label(nome)).is_none(), "{nome}");
        clicar(&mut h, Role::Label, nome);
        assert!(acoes(&mut h).is_empty(), "{nome} não abre");
        // sem o Mostrar na pasta; o Remover da lista continua
        assert!(h.query(by().role(Role::Button).label(format!("Mostrar {nome} na pasta").as_str())).is_none());
        assert!(h.query(by().role(Role::Button).label(format!("Remover {nome} da lista").as_str())).is_some());
    }
    // a pill no lugar do tamanho (também no da pasta, que não diz "pasta")
    assert_eq!(h.query_all(by().label("não encontrado")).count(), 2);
    assert!(h.query(by().role(Role::Label).label("pasta")).is_none());
}

#[test]
fn remover_da_lista_tira_o_recente_e_pede_a_releitura() {
    let amb = com_recentes_de_cada_tipo("inicio_remover");
    let mut h = montar(&amb);
    clicar(&mut h, Role::Button, "Remover dados da lista");
    esperar(&mut h, |h| !h.state().acoes.is_empty());
    assert_eq!(acoes(&mut h), [Acao::RecarregarPrefs]);
    let restantes: Vec<String> = amb.servico.prefs().recentes().into_iter().map(|r| r.nome).collect();
    assert_eq!(restantes, ["exemplo.db", "clientes.csv", "notas.txt"]);
    assentar(&mut h);
}

#[test]
fn vazio_dos_recentes_novo() {
    let amb = Ambiente::vazio("inicio_sem_recentes");
    let h = montar(&amb);
    assert!(tem_texto(&h, VAZIO_RECENTES), "{VAZIO_RECENTES}");
    assert!(VAZIO_RECENTES.contains("As pastas, os bancos e os arquivos que você abrir"));
}

/// Com um banco aberto, o aviso de antes ("exemplo.db está aberto. Abrir outro fecha este.") e o link para ele.
#[test]
fn banco_aberto_avisa_e_leva_ao_banco() {
    let amb = Ambiente::exemplo("inicio_banco_aberto");
    let mut h = montar(&amb);
    assert!(tem_texto(&h, "está aberto. Abrir outro fecha este."));
    clicar(&mut h, Role::Link, "Ir para o banco");
    assert_eq!(acoes(&mut h), [Acao::Navegar(Rota::Banco)]);
}

#[test]
fn folha_pela_extensao_do_arquivo() {
    for (caminho, tipo) in [
        ("C:\\d\\a.csv", TipoArquivo::Csv),
        ("C:\\d\\a.TSV", TipoArquivo::Csv),
        ("C:\\d\\a.xlsx", TipoArquivo::Xlsx),
        ("C:\\d\\a.json", TipoArquivo::Json),
        ("C:\\d\\leia-me.md", TipoArquivo::Txt),
        ("C:\\d\\sem_extensao", TipoArquivo::Txt),
    ] {
        assert_eq!(folha_do_arquivo(caminho), tipo, "{caminho}");
    }
}

/// A raiz de uma unidade ou de um compartilhamento aberta como pasta não tem nome: fica o
/// caminho inteiro, no Início, nas ações da linha e no menu.
#[test]
fn recente_da_raiz_da_unidade_mostra_o_caminho() {
    let recente = |caminho: &str, nome: &str| prefs::Recente {
        caminho: caminho.to_string(),
        nome: nome.to_string(),
        pasta: caminho.to_string(),
        tipo: prefs::TipoRecente::Pasta,
        existe: true,
        tamanho: None,
        modificado_em: None,
        aberto_em: None,
    };
    assert_eq!(rotulo_do_recente(&recente("C:\\", "")), "C:\\");
    assert_eq!(rotulo_do_recente(&recente("\\\\srv\\dados", "")), "\\\\srv\\dados");
    assert_eq!(rotulo_do_recente(&recente("C:\\dados", "dados")), "dados");
}
