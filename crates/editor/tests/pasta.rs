//! A pasta na casca (etapa 4): Arquivo › Abrir pasta pelo diálogo e sem ele, o título e os recentes, o caminho
//! classificado (pasta, banco ou arquivo de dados), a pasta que volta na abertura seguinte (e o banco não), a que
//! sumiu, o Fechar pasta, a troca de banco que mantém as abas de arquivo, a releitura na volta do foco e a linha de
//! comando. Tudo com o `Editor` inteiro sobre o `Ambiente::com_pasta` (uma pasta de dados de verdade).

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod apoio;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use apoio::*;
use editor::{Partida, PedidoDialogo};
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Acao, NoDaPasta, Rota};
use interface::pecas::toast::toast_atual;
use interface::tema::Tema;

/// O caminho como o serviço guarda o do banco aberto e o da pasta: absoluto, com os nomes curtos (8.3) resolvidos (a
/// pasta temporária vem como `C:\Users\CONCEI~1\…`), sem o `\\?\`.
fn real(caminho: &Path) -> PathBuf {
    let c = std::fs::canonicalize(caminho).expect("canonicalize");
    let texto = c.to_string_lossy().into_owned();
    PathBuf::from(texto.strip_prefix(r"\\?\").unwrap_or(&texto))
}

fn chave(caminho: &Path) -> String {
    prefs::chave_caminho(caminho)
}

/// A pasta aberta agora (o caminho dela), se houver.
fn pasta_aberta(h: &H) -> Option<PathBuf> {
    h.state().estado().pasta.as_ref().map(|p| p.caminho.clone())
}

/// A raiz da pasta aberta já foi lida.
fn raiz_lida(h: &H) -> bool {
    h.state().estado().pasta.as_ref().is_some_and(|p| p.listagem(&p.chave).is_some())
}

/// Emite a ação (sem aba de origem) e espera as tarefas terminarem.
fn emitir_e_assentar(h: &mut H, acao: Acao) {
    h.state_mut().emitir(acao);
    h.run_steps(2);
    assentar(h);
}

/// Abre a pasta de dados do ambiente pela casca e espera a raiz chegar.
fn abrir_dados(h: &mut H, amb: &Ambiente) {
    emitir_e_assentar(h, Acao::AbrirPasta { caminho: amb.pasta_de_dados() });
    assert!(raiz_lida(h), "a raiz lida: {:?}", h.state().estado().pasta);
}

/// Espera o toast com este texto.
fn esperar_toast(h: &mut H, texto: &str) {
    let alvo = texto.to_string();
    esperar(h, &format!("o toast {texto:?}"), move |h| toast_atual(&h.ctx).is_some_and(|t| t.contains(&alvo)));
}

/// As rotas de arquivo de dados abertas, pelo nome do arquivo.
fn fichas(h: &H) -> Vec<String> {
    h.state()
        .rotas_abertas()
        .iter()
        .filter_map(|r| r.caminho_arquivo().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()))
        .collect()
}

#[test]
fn abrir_pasta_pelo_dialogo_lembra_a_pasta_de_cima() {
    let amb = Ambiente::com_pasta("pasta_pelo_dialogo", &[]);
    let dados = amb.pasta_de_dados();
    let pedidos = Rc::new(RefCell::new(Vec::new()));
    let (anotar, escolhida) = (Rc::clone(&pedidos), dados.clone());
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, Partida::default(), move |p| {
        anotar.borrow_mut().push(p.clone());
        (*p == PedidoDialogo::AbrirPasta).then(|| escolhida.clone())
    });
    emitir_e_assentar(&mut h, Acao::EscolherEAbrirPasta);
    assert_eq!(pedidos.borrow().as_slice(), [PedidoDialogo::AbrirPasta]);
    assert_eq!(pasta_aberta(&h), Some(real(&dados)), "a pasta resolvida, como o banco");
    assert!(raiz_lida(&h));
    // a pasta de cima da escolhida é a próxima do diálogo (também nas preferências)
    assert_eq!(h.state().estado().ultimo_diretorio.as_deref(), Some(amb.pasta()));
    assert_eq!(amb.servico.prefs().ultimo_diretorio().map(PathBuf::from).as_deref(), Some(amb.pasta()));
    // cancelar não muda nada
    let mut h = harness_com(&amb, Tema::Claro, TAMANHO, Partida::default(), |_| None);
    let antes = h.state().estado().pasta.clone();
    emitir_e_assentar(&mut h, Acao::EscolherEAbrirPasta);
    assert_eq!(h.state().estado().pasta, antes);
    assert!(!h.state().janela_aberta());
}

/// O campo "Caminho da pasta" da janela "Pedir o caminho" (o rótulo também é o do `campo` em cima dele).
fn campo_da_pasta(h: &H) -> egui_kittest::Node<'_> {
    h.get(by().role(Role::TextInput).label("Caminho da pasta"))
}

fn escrever_na_pasta(h: &mut H, texto: &str) {
    let campo = campo_da_pasta(h);
    campo.focus();
    campo.type_text(texto);
    h.run_steps(2);
}

#[test]
fn abrir_pasta_sem_dialogo_usa_o_pedir_o_caminho() {
    let amb = Ambiente::com_pasta("pasta_sem_dialogo", &[]);
    let mut h = harness_sem_dialogo(&amb, TAMANHO);
    emitir_e_assentar(&mut h, Acao::EscolherEAbrirPasta);
    assert!(h.state().janela_aberta(), "a janela Pedir o caminho");
    assert!(tem_texto_exato(&h, "Abrir pasta"));
    // vazio: a frase da pasta
    campo_da_pasta(&h).focus();
    h.run_steps(1);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(tem_texto(&h, "Digite o caminho da pasta."), "{:?}", textos(&h));
    // a que não existe: o erro na janela, que continua aberta
    escrever_na_pasta(&mut h, &amb.pasta().join("nao_existe").display().to_string());
    h.key_press(Key::Enter);
    h.run_steps(2);
    esperar_texto(&mut h, "A pasta não existe (ou foi movida)");
    assert!(h.state().janela_aberta() && h.state().estado().pasta.is_none());
    capturar(&mut h, "pedir-caminho-pasta-claro.png");
    // a de verdade: abre e a janela fecha
    campo_da_pasta(&h).focus();
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    escrever_na_pasta(&mut h, &amb.pasta_de_dados().display().to_string());
    h.key_press(Key::Enter);
    h.run_steps(2);
    assentar(&mut h);
    assert_eq!(pasta_aberta(&h), Some(real(&amb.pasta_de_dados())));
    assert!(!h.state().janela_aberta());
}

#[test]
fn abrir_pasta_poe_a_pasta_no_titulo_e_nos_recentes() {
    let amb = Ambiente::com_pasta("pasta_titulo", &[]);
    let mut h = harness(&amb);
    assert_eq!(h.state().titulo(), "DataBox");
    abrir_dados(&mut h, &amb);
    assert_eq!(h.state().titulo(), "dados · DataBox");
    let recentes = &h.state().estado().prefs.recentes;
    let pasta = recentes.iter().find(|r| r.nome == "dados").expect("o recente da pasta");
    assert_eq!(pasta.tipo, prefs::TipoRecente::Pasta);
    // a pasta aberta fica nas preferências, com o caminho resolvido
    assert_eq!(amb.servico.prefs().pasta_aberta().map(PathBuf::from), Some(real(&amb.pasta_de_dados())));
    // com um banco aberto, o título continua com a pasta
    emitir_e_assentar(
        &mut h,
        Acao::AbrirBanco { caminho: amb.pasta_de_dados().join("exemplo.db"), somente_leitura: false },
    );
    assert!(h.state().estado().tem_banco());
    assert_eq!(h.state().titulo(), "dados · DataBox");
    assert!(h.state().estado().pasta.is_some(), "a pasta sobrevive à troca de banco");
}

#[test]
fn abrir_caminho_classifica_pasta_banco_e_arquivo() {
    let amb = Ambiente::com_pasta("abrir_caminho", &[]);
    let dados = amb.pasta_de_dados();
    let mut h = harness(&amb);
    // a pasta
    emitir_e_assentar(&mut h, Acao::AbrirCaminho { caminho: dados.clone(), somente_leitura: false });
    assert_eq!(pasta_aberta(&h), Some(real(&dados)));
    assert!(!h.state().estado().tem_banco());
    // o arquivo de dados: a ficha (Rota::Arquivo), e o recente de tipo arquivo
    emitir_e_assentar(&mut h, Acao::AbrirCaminho { caminho: dados.join("clientes.csv"), somente_leitura: false });
    assert!(
        matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("clientes.csv")),
        "{:?}",
        h.state().rota()
    );
    let recentes = &h.state().estado().prefs.recentes;
    assert!(recentes.iter().any(|r| r.nome == "clientes.csv" && r.tipo == prefs::TipoRecente::Arquivo), "{recentes:?}");
    // o banco sem extensão de banco (pelo cabeçalho), só para leitura
    emitir_e_assentar(&mut h, Acao::AbrirCaminho { caminho: dados.join("copia.dat"), somente_leitura: true });
    let banco = h.state().estado().banco.clone().expect("o banco abriu");
    assert_eq!(banco.nome, "copia.dat");
    assert!(banco.somente_leitura);
    assert_eq!(h.state().rota(), &Rota::Banco);
    // um .db que não é SQLite: o erro do SQLite, num toast (e o banco aberto continua)
    h.state_mut().emitir(Acao::AbrirCaminho { caminho: dados.join("Thumbs.db"), somente_leitura: false });
    esperar_toast(&mut h, "SQLite");
    assentar(&mut h);
    assert_eq!(h.state().estado().banco.as_ref().map(|b| b.nome.as_str()), Some("copia.dat"));
}

#[test]
fn abrir_caminho_de_tipo_que_nao_abre_da_o_toast() {
    let amb = Ambiente::com_pasta("tipo_que_nao_abre", &[]);
    let pdf = amb.pasta_de_dados().join("relatorio.pdf");
    let mut h = harness(&amb);
    h.state_mut().emitir(Acao::AbrirCaminho { caminho: pdf.clone(), somente_leitura: false });
    esperar_toast(&mut h, "O DataBox ainda não abre este tipo de arquivo: relatorio.pdf.");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    // vindo do Início, o erro aparece nele
    let inicio = h.state().id_aba_ativa();
    h.state_mut().emitir_de(inicio, Acao::AbrirCaminho { caminho: pdf, somente_leitura: false });
    esperar_texto(&mut h, "O DataBox ainda não abre este tipo de arquivo: relatorio.pdf.");
    // a pasta que não existe, também do Início
    let fantasma = amb.pasta().join("sumida");
    h.state_mut().emitir_de(inicio, Acao::AbrirPasta { caminho: fantasma });
    esperar_texto(&mut h, "A pasta não existe (ou foi movida)");
    assert!(h.state().estado().pasta.is_none());
}

#[test]
fn a_pasta_volta_na_proxima_abertura_e_o_banco_nao() {
    let amb = Ambiente::com_pasta("pasta_volta", &[]);
    let dados = amb.pasta_de_dados();
    {
        let mut h = harness(&amb);
        abrir_dados(&mut h, &amb);
        emitir_e_assentar(&mut h, Acao::AbrirBanco { caminho: dados.join("exemplo.db"), somente_leitura: false });
        assert!(h.state().estado().tem_banco());
    }
    // a sessão seguinte: o serviço sem banco aberto, as mesmas preferências
    let id = amb.servico.estado().expect("estado").banco.map(|b| b.id).unwrap_or_default();
    amb.servico.fechar_banco(&id, true).expect("fechar o banco");
    let h = harness(&amb);
    assert_eq!(pasta_aberta(&h), Some(real(&dados)));
    assert!(raiz_lida(&h), "a leitura da raiz foi pedida ao iniciar");
    assert!(!h.state().estado().tem_banco(), "o banco não reabre sozinho");
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert_eq!(h.state().titulo(), "dados · DataBox");
    assert!(existe(&h, Role::Link, "dados, pasta"), "a raiz no Explorador: {:?}", textos(&h));
}

#[test]
fn pasta_que_sumiu_mostra_o_erro_e_guarda_a_chave() {
    let amb = Ambiente::com_pasta("pasta_sumiu", &["rede/a.txt"]);
    let rede = amb.pasta_de_dados().join("rede");
    amb.servico.abrir_pasta(&rede).expect("abrir a pasta");
    let guardada = amb.servico.prefs().pasta_aberta().expect("a pasta aberta guardada");
    let fora = amb.pasta().join("rede-fora");
    std::fs::rename(&rede, &fora).expect("tirar a pasta do lugar");
    let mut h = harness(&amb);
    let p = h.state().estado().pasta.clone().expect("a pasta continua aberta");
    assert!(matches!(p.nos.get(&p.chave), Some(NoDaPasta::Falhou(_))), "{:?}", p.nos);
    assert!(tem_texto_exato(&h, "Não foi possível ler a pasta."), "{:?}", textos(&h));
    assert_eq!(amb.servico.prefs().pasta_aberta(), Some(guardada), "a chave fica");
    capturar(&mut h, "explorador-pasta-sumiu-claro.png");
    // a pasta volta e o "Tentar de novo" a lê
    std::fs::rename(&fora, &rede).expect("pôr a pasta de volta");
    clicar_botao(&mut h, "Tentar de novo");
    assentar(&mut h);
    assert!(raiz_lida(&h));
    assert!(!tem_texto_exato(&h, "Não foi possível ler a pasta."));
}

#[test]
fn fechar_pasta_apaga_a_chave_fecha_as_abas_de_dentro_e_mantem_o_banco() {
    let amb = Ambiente::com_pasta("fechar_pasta", &[]);
    let dados = amb.pasta_de_dados();
    let fora = amb.pasta().join("fora.csv");
    std::fs::write(&fora, "a;b\n1;2\n").expect("o csv de fora");
    let mut h = harness(&amb);
    abrir_dados(&mut h, &amb);
    emitir_e_assentar(&mut h, Acao::AbrirBanco { caminho: dados.join("exemplo.db"), somente_leitura: false });
    emitir_e_assentar(&mut h, Acao::AbrirArquivoDeDados { caminho: dados.join("clientes.csv") });
    emitir_e_assentar(&mut h, Acao::AbrirArquivoDeDados { caminho: fora.clone() });
    assert_eq!(fichas(&h), ["clientes.csv", "fora.csv"]);
    emitir_e_assentar(&mut h, Acao::FecharPasta);
    assert!(h.state().estado().pasta.is_none());
    assert_eq!(amb.servico.prefs().pasta_aberta(), None, "a chave saiu das preferências");
    assert_eq!(fichas(&h), ["fora.csv"], "a ficha de fora da pasta fica");
    assert!(h.state().estado().tem_banco(), "o banco continua aberto");
    assert_eq!(h.state().titulo(), "exemplo.db · DataBox");
    // os recentes ficam
    assert!(h.state().estado().prefs.recentes.iter().any(|r| r.tipo == prefs::TipoRecente::Pasta));
    // sem pasta, o Fechar pasta não faz nada
    emitir_e_assentar(&mut h, Acao::FecharPasta);
    assert_eq!(fichas(&h), ["fora.csv"]);
}

/// Fechar pasta com outra pasta ainda abrindo (uma pasta de rede lenta): o fechar vale, a abertura que chega depois
/// não reabre a árvore, e a chave `pasta_aberta` que ela gravou sai de novo (a próxima abertura do app não a traz).
#[test]
fn fechar_pasta_com_outra_abrindo_desfaz_a_abertura() {
    let amb = Ambiente::com_pasta("fechar_com_outra_abrindo", &[]);
    let mut h = harness(&amb);
    abrir_dados(&mut h, &amb);
    h.state_mut().emitir(Acao::AbrirPasta { caminho: amb.pasta_de_dados().join("sub") });
    h.state_mut().emitir(Acao::FecharPasta);
    h.run_steps(2);
    assentar(&mut h);
    assert!(h.state().estado().pasta.is_none(), "{:?}", pasta_aberta(&h));
    assert_eq!(amb.servico.prefs().pasta_aberta(), None);
    assert_eq!(h.state().titulo(), "DataBox");
    // depois disso, abrir de novo funciona
    abrir_dados(&mut h, &amb);
    assert_eq!(pasta_aberta(&h).map(|p| chave(&p)), Some(chave(&real(&amb.pasta_de_dados()))));
}

/// A ficha pedida de um arquivo da pasta e, antes de ela abrir, o Fechar pasta: a ficha não abre (o Fechar pasta fecha
/// as de dentro dela), e o recente fica.
#[test]
fn ficha_pedida_antes_do_fechar_pasta_nao_abre() {
    let amb = Ambiente::com_pasta("ficha_antes_do_fechar", &[]);
    let mut h = harness(&amb);
    abrir_dados(&mut h, &amb);
    let csv = real(&amb.pasta_de_dados()).join("clientes.csv");
    h.state_mut().emitir(Acao::AbrirArquivoDeDados { caminho: csv.clone() });
    h.state_mut().emitir(Acao::FecharPasta);
    h.run_steps(2);
    assentar(&mut h);
    assert!(fichas(&h).is_empty(), "{:?}", fichas(&h));
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert!(h.state().estado().prefs.recentes.iter().any(|r| chave(Path::new(&r.caminho)) == chave(&csv)));
}

/// A releitura de quando o próprio app mexe na pasta (o fim de uma troca de banco: o Novo banco cria o arquivo) não
/// se perde por causa da volta do foco logo antes: nem pelo espaçamento de 2 s, nem pela leitura que ainda corre (ela
/// espera e sai depois).
#[test]
fn releitura_do_fim_da_troca_nao_se_perde_logo_depois_da_volta_do_foco() {
    let amb = Ambiente::com_pasta("releitura_da_troca", &[]);
    let mut h = harness(&amb);
    abrir_dados(&mut h, &amb);
    let banco = amb.pasta_de_dados().join("exemplo.db");
    emitir_e_assentar(&mut h, Acao::AbrirBanco { caminho: banco, somente_leitura: false });
    let antes = h.state().leituras_de_pasta();
    // a volta do foco relê a raiz; logo em seguida (a leitura ainda corre, e não passaram 2 s), a troca termina
    h.input_mut().focused = false;
    h.step();
    h.input_mut().focused = true;
    h.step();
    assert_eq!(h.state().leituras_de_pasta(), antes + 1);
    std::fs::write(amb.pasta_de_dados().join("novo.csv"), "x\n1\n").expect("o csv novo");
    h.state_mut().emitir(Acao::FecharBanco);
    h.run_steps(2);
    assentar(&mut h);
    assert_eq!(h.state().leituras_de_pasta(), antes + 2, "a raiz relida de novo depois da troca");
    let p = h.state().estado().pasta.clone().expect("pasta");
    assert!(p.listagem(&p.chave).expect("raiz").entradas.iter().any(|e| e.nome == "novo.csv"));
}

#[test]
fn troca_de_banco_mantem_as_abas_de_arquivo() {
    let amb = Ambiente::com_pasta("troca_mantem_fichas", &[]);
    let dados = amb.pasta_de_dados();
    let mut h = harness(&amb);
    emitir_e_assentar(&mut h, Acao::AbrirArquivoDeDados { caminho: dados.join("notas.txt") });
    emitir_e_assentar(&mut h, Acao::AbrirBanco { caminho: dados.join("exemplo.db"), somente_leitura: false });
    h.state_mut().ir(Rota::objeto("clientes", false));
    h.run_steps(2);
    assentar(&mut h);
    // outro banco: as abas do banco de antes fecham, a ficha fica
    emitir_e_assentar(&mut h, Acao::AbrirBanco { caminho: dados.join("estoque_2026.db"), somente_leitura: false });
    let rotas = h.state().rotas_abertas();
    assert_eq!(fichas(&h), ["notas.txt"], "{rotas:?}");
    assert!(rotas.contains(&Rota::Banco) && !rotas.iter().any(|r| r.nome_objeto() == Some("clientes")), "{rotas:?}");
    assert_eq!(h.state().rota(), &Rota::Banco);
    // fechar o banco: o Início, e a ficha continua
    emitir_e_assentar(&mut h, Acao::FecharBanco);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert_eq!(fichas(&h), ["notas.txt"]);
}

#[test]
fn volta_do_foco_rele_a_pasta_uma_vez_a_cada_2s_e_nao_durante_uma_leitura() {
    let amb = Ambiente::com_pasta("foco_rele", &[]);
    let mut h = harness(&amb);
    // sem pasta, a volta do foco não lê nada
    let alternar_o_foco = |h: &mut H| {
        h.input_mut().focused = false;
        h.step();
        h.input_mut().focused = true;
        h.step();
    };
    alternar_o_foco(&mut h);
    assert_eq!(h.state().leituras_de_pasta(), 0);
    abrir_dados(&mut h, &amb);
    let antes = h.state().leituras_de_pasta();
    // a primeira volta relê a raiz (e só ela: nenhuma subpasta foi lida)
    alternar_o_foco(&mut h);
    let quando = h.ctx.input(|i| i.time);
    assert_eq!(h.state().leituras_de_pasta(), antes + 1);
    // outra logo em seguida: com a leitura em andamento ou antes dos 2 s, nada
    alternar_o_foco(&mut h);
    assert_eq!(h.state().leituras_de_pasta(), antes + 1);
    assentar(&mut h);
    // (a volta do foco cai dois quadros depois, 0,5 s adiante)
    if h.ctx.input(|i| i.time) + 0.5 - quando < 2.0 {
        alternar_o_foco(&mut h);
        assert_eq!(h.state().leituras_de_pasta(), antes + 1, "antes dos 2 s");
    }
    // passados os 2 s (cada quadro do harness avança 0,25 s), a volta relê de novo
    while h.ctx.input(|i| i.time) - quando < 2.1 {
        h.step();
    }
    alternar_o_foco(&mut h);
    assert_eq!(h.state().leituras_de_pasta(), antes + 2);
    assentar(&mut h);
    // um arquivo novo na pasta aparece na releitura
    std::fs::write(amb.pasta_de_dados().join("novo.csv"), "x\n1\n").expect("o csv novo");
    let quando = h.ctx.input(|i| i.time);
    while h.ctx.input(|i| i.time) - quando < 2.1 {
        h.step();
    }
    alternar_o_foco(&mut h);
    assentar(&mut h);
    let p = h.state().estado().pasta.clone().expect("pasta");
    let raiz = p.listagem(&p.chave).expect("raiz");
    assert!(raiz.entradas.iter().any(|e| e.nome == "novo.csv"), "{:?}", raiz.entradas);
    assert_eq!(chave(&raiz.pasta), p.chave);
}

#[test]
fn linha_de_comando_abre_pasta_banco_ou_arquivo() {
    let amb = Ambiente::com_pasta("linha_comando_pasta", &[]);
    let dados = amb.pasta_de_dados();
    // a pasta
    let h =
        harness_com(&amb, Tema::Claro, TAMANHO, Partida { caminho: Some(dados.clone()), ..Partida::default() }, |_| {
            None
        });
    assert_eq!(pasta_aberta(&h), Some(real(&dados)));
    assert!(raiz_lida(&h) && !h.state().estado().tem_banco());
    assert_eq!(h.state().titulo(), "dados · DataBox");
    drop(h);
    // o arquivo de dados: a ficha
    let csv = dados.join("clientes.csv");
    let h = harness_com(&amb, Tema::Claro, TAMANHO, Partida { caminho: Some(csv), ..Partida::default() }, |_| None);
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("clientes.csv")));
    drop(h);
    // o banco sem extensão de banco, só para leitura (a pasta da sessão anterior também volta)
    let banco = dados.join("sem_extensao");
    let partida = Partida { caminho: Some(banco), somente_leitura: true };
    let h = harness_com(&amb, Tema::Claro, TAMANHO, partida, |_| None);
    assert_eq!(
        h.state().estado().banco.as_ref().map(|b| (b.nome.as_str(), b.somente_leitura)),
        Some(("sem_extensao", true))
    );
    assert_eq!(h.state().rota(), &Rota::Banco);
    assert_eq!(pasta_aberta(&h), Some(real(&dados)), "a pasta da sessão anterior");
}

// ---- a ficha do arquivo de dados (passo 10) ---------------------------------------------------------------------

/// O editor com a pasta de dados, a ficha do `nome` à vista e o gancho que recebe o "Abrir no programa padrão" e o
/// "Mostrar na pasta" (no lugar do Windows).
fn harness_da_ficha(amb: &Ambiente, nome: &str, tema: Tema) -> (H, Rc<RefCell<Vec<PathBuf>>>) {
    let abertos = Rc::new(RefCell::new(Vec::new()));
    let anotar = Rc::clone(&abertos);
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), tema, Partida::default())
        .com_dialogo(|_| None)
        .com_abrir_de_teste(move |c| anotar.borrow_mut().push(c.to_path_buf()));
    let mut h = harness_editor(editor, tema, TAMANHO);
    emitir_e_assentar(&mut h, Acao::AbrirArquivoDeDados { caminho: amb.pasta_de_dados().join(nome) });
    (h, abertos)
}

#[test]
fn ficha_do_arquivo_mostra_tipo_tamanho_data_e_caminho() {
    let amb = Ambiente::com_pasta("ficha_mostra", &[]);
    // sem as telas, o Excel abre na ficha; com elas, na planilha (o `.xlsx` de teste é só o começo de um zip: o erro
    // dela fica no lugar da grade, com a mesma seção Arquivo e os mesmos dois botões)
    let xlsx = real(&amb.pasta_de_dados().join("planilha.xlsx"));
    let (mut h, _) = harness_da_ficha(&amb, "planilha.xlsx", Tema::Claro);
    assert_eq!(h.state().rota(), &Rota::arquivo(&xlsx));
    let frase = if cfg!(feature = "telas") {
        "O arquivo não é uma planilha válida".to_string()
    } else {
        format!(
            "O editor de Excel ainda não está pronto. Enquanto isso, o arquivo abre no {}.",
            comum::app::PROGRAMA_PADRAO
        )
    };
    assert!(tem_texto(&h, &frase), "{:?}", textos(&h));
    // a faixa: o nome e o caminho
    assert!(existe(&h, Role::Label, "planilha.xlsx") && existe(&h, Role::Label, &xlsx.display().to_string()));
    let meta = std::fs::metadata(&xlsx).expect("meta");
    let tamanho = interface::formato::formatar_bytes(meta.len());
    let quando = chrono::DateTime::<chrono::Local>::from(meta.modified().expect("data")).format("%d/%m/%Y %H:%M");
    assert!(existe(&h, Role::Button, "Seção Arquivo"), "{:?}", textos(&h));
    // os valores da seção Arquivo (os rótulos dela são pintados): o tamanho e a data; a ficha também o tipo e o caminho
    let mut esperados = vec![tamanho.clone(), quando.to_string()];
    if !cfg!(feature = "telas") {
        esperados.extend(["XLSX".to_string(), xlsx.display().to_string()]);
    }
    for esperado in &esperados {
        assert!(tem_texto_exato(&h, esperado), "{esperado:?} nas Propriedades: {:?}", textos(&h));
    }
    assert!(existe(&h, Role::Button, "Abrir no programa padrão") && existe(&h, Role::Button, "Mostrar na pasta"));
    capturar(&mut h, "ficha-arquivo-claro.png");
    let (mut h, _) = harness_da_ficha(&amb, "planilha.xlsx", Tema::Escuro);
    // com as telas, o Excel abre na planilha (este .xlsx de teste é só o começo de um zip: a ficha diz o motivo)
    let excel = if cfg!(feature = "telas") {
        "O arquivo não é uma planilha válida"
    } else {
        "O editor de Excel ainda não está pronto."
    };
    assert!(tem_texto(&h, excel));
    capturar(&mut h, "ficha-arquivo-escuro.png");
}

#[test]
fn abrir_no_programa_padrao_chama_o_gancho() {
    let amb = Ambiente::com_pasta("ficha_abrir", &[]);
    // o Excel que não abre (e a ficha, sem as telas) tem os mesmos dois botões
    let json = real(&amb.pasta_de_dados().join("planilha.xlsx"));
    let (mut h, abertos) = harness_da_ficha(&amb, "planilha.xlsx", Tema::Claro);
    clicar_botao(&mut h, "Abrir no programa padrão");
    assert_eq!(abertos.borrow().as_slice(), std::slice::from_ref(&json));
    clicar_botao(&mut h, "Mostrar na pasta");
    assert_eq!(abertos.borrow().as_slice(), [json.clone(), json]);
}

#[test]
fn ficha_de_arquivo_que_sumiu_desliga_os_botoes() {
    let amb = Ambiente::com_pasta("ficha_sumiu", &[]);
    let (mut h, abertos) = harness_da_ficha(&amb, "planilha.xlsx", Tema::Claro);
    let sumido = amb.pasta_de_dados().join("sumido.xlsx");
    h.state_mut().ir(Rota::arquivo(&sumido));
    h.run_steps(2);
    assentar(&mut h);
    esperar_texto(&mut h, "O arquivo não existe (ou foi movido)");
    for botao in ["Abrir no programa padrão", "Mostrar na pasta"] {
        let n = h.query_all(by().role(Role::Button).label(botao)).next().expect(botao);
        assert!(n.accesskit_node().is_disabled(), "{botao} desligado");
    }
    clicar_primeiro(&mut h, Role::Button, "Abrir no programa padrão");
    assert!(abertos.borrow().is_empty());
    capturar(&mut h, "ficha-arquivo-sumiu-claro.png");
}

#[test]
fn copiar_caminho_da_ficha() {
    let amb = Ambiente::com_pasta("ficha_copiar", &[]);
    let csv = real(&amb.pasta_de_dados().join("clientes.csv"));
    let (mut h, _) = harness_da_ficha(&amb, "clientes.csv", Tema::Claro);
    h.query_all(by().role(Role::Button).label("Copiar caminho")).next().expect("o Copiar caminho").click();
    h.step();
    let copiado = h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    });
    assert_eq!(copiado, Some(csv.display().to_string()));
    h.run_steps(2);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Caminho copiado."));
}

#[test]
fn aba_da_ficha_com_a_folha_e_o_nome() {
    let amb = Ambiente::com_pasta("ficha_aba", &[]);
    let (mut h, _) = harness_da_ficha(&amb, "clientes.csv", Tema::Claro);
    assert!(abas(&h).contains(&("clientes.csv".to_string(), true)), "{:?}", abas(&h));
    assert!(existe(&h, Role::Tab, "clientes.csv"));
    // o mesmo arquivo de novo foca a aba aberta
    emitir_e_assentar(&mut h, Acao::AbrirArquivoDeDados { caminho: amb.pasta_de_dados().join("CLIENTES.csv") });
    assert_eq!(abas(&h).iter().filter(|(r, _)| r.eq_ignore_ascii_case("clientes.csv")).count(), 1);
    // o Explorador sem pasta nem banco continua; a aba se fecha pelo ×
    fechar_aba(&mut h, "clientes.csv");
    assert!(!abas(&h).iter().any(|(r, _)| r == "clientes.csv"));
}

#[test]
fn status_da_ficha_diz_tipo_e_tamanho() {
    let amb = Ambiente::com_pasta("ficha_status", &[]);
    let (h, _) = harness_da_ficha(&amb, "planilha.xlsx", Tema::Claro);
    let tamanho = std::fs::metadata(amb.pasta_de_dados().join("planilha.xlsx")).expect("meta").len();
    let esperado = format!("XLSX · {}", interface::formato::formatar_bytes(tamanho));
    assert!(existe(&h, Role::Label, &esperado), "{esperado:?}: {:?}", textos(&h));
}

#[test]
fn a_ficha_nao_oferece_verbo_nenhum() {
    let amb = Ambiente::com_pasta("ficha_verbos", &[]);
    let (h, _) = harness_da_ficha(&amb, "planilha.xlsx", Tema::Claro);
    for verbo in ["Salvar", "Novo", "Nova coluna", "Excluir", "Desfazer", "Executar", "Exportar"] {
        assert!(!botao_ligado(&h, verbo), "{verbo} desligado na ficha");
    }
    // a ficha não relê nada; com as telas, o Excel abre na planilha, que oferece o Atualizar (ler o arquivo de novo,
    // decisão 20 da etapa 6), também com o erro à vista
    assert_eq!(botao_ligado(&h, "Atualizar"), cfg!(feature = "telas"));
    // o painel de Propriedades existe (o botão dele está ligado)
    assert!(h.state().propriedades_a_vista());
}

// ---- menus, atalho, barra e status (passo 11) -------------------------------------------------------------------

/// O editor com o diálogo que escolhe a pasta de dados (e anota os pedidos).
fn harness_que_escolhe_a_pasta(amb: &Ambiente) -> (H, Rc<RefCell<Vec<PedidoDialogo>>>) {
    let pedidos = Rc::new(RefCell::new(Vec::new()));
    let (anotar, dados) = (Rc::clone(&pedidos), amb.pasta_de_dados());
    let h = harness_com(amb, Tema::Claro, TAMANHO, Partida::default(), move |p| {
        anotar.borrow_mut().push(p.clone());
        (*p == PedidoDialogo::AbrirPasta).then(|| dados.clone())
    });
    (h, pedidos)
}

#[test]
fn menu_arquivo_abre_a_pasta_e_fecha_a_pasta() {
    let amb = Ambiente::com_pasta("menu_pasta", &[]);
    let (mut h, pedidos) = harness_que_escolhe_a_pasta(&amb);
    abrir_menu_arquivo(&mut h);
    assert!(item_ligado(&h, "Abrir pasta") && !item_ligado(&h, "Fechar pasta"), "sem pasta, o Fechar desligado");
    clicar_item(&mut h, "Abrir pasta");
    assentar(&mut h);
    assert_eq!(pedidos.borrow().as_slice(), [PedidoDialogo::AbrirPasta]);
    assert_eq!(pasta_aberta(&h), Some(real(&amb.pasta_de_dados())));
    abrir_menu_arquivo(&mut h);
    assert!(item_ligado(&h, "Fechar pasta"));
    capturar(&mut h, "menu-arquivo-pasta-claro.png");
    clicar_item(&mut h, "Fechar pasta");
    assentar(&mut h);
    assert!(h.state().estado().pasta.is_none());
    assert_eq!(h.state().titulo(), "DataBox");
}

#[test]
fn ctrl_shift_o_abre_a_pasta() {
    let amb = Ambiente::com_pasta("ctrl_shift_o", &[]);
    let (mut h, pedidos) = harness_que_escolhe_a_pasta(&amb);
    let ctrl_shift = egui::Modifiers { shift: true, ..CTRL };
    // o AltGr (Ctrl+Alt) com o Shift não conta
    h.key_press_modifiers(egui::Modifiers { alt: true, ..ctrl_shift }, Key::O);
    h.run_steps(3);
    assert!(pedidos.borrow().is_empty());
    h.key_press_modifiers(ctrl_shift, Key::O);
    h.run_steps(3);
    assentar(&mut h);
    assert_eq!(pedidos.borrow().as_slice(), [PedidoDialogo::AbrirPasta], "o da pasta, não o do arquivo");
    assert_eq!(pasta_aberta(&h), Some(real(&amb.pasta_de_dados())));
    // o Ctrl+O continua o do arquivo
    h.key_press_modifiers(CTRL, Key::O);
    h.run_steps(3);
    assentar(&mut h);
    assert_eq!(pedidos.borrow().last(), Some(&PedidoDialogo::AbrirBanco));
}

#[test]
fn barra_de_ferramentas_abre_a_pasta() {
    let amb = Ambiente::com_pasta("barra_pasta", &[]);
    let (mut h, pedidos) = harness_que_escolhe_a_pasta(&amb);
    assert!(botao_ligado(&h, "Abrir pasta"));
    // a dica do da barra (o primeiro: o Explorador vazio também tem um "Abrir pasta")
    h.query_all(by().role(Role::Button).label("Abrir pasta")).next().expect("o da barra").hover();
    h.run_steps(3);
    assert!(tem_texto_exato(&h, "Abrir pasta (Ctrl+Shift+O)"), "{:?}", textos(&h));
    h.remove_cursor();
    h.run_steps(2);
    clicar_primeiro(&mut h, Role::Button, "Abrir pasta");
    h.run_steps(2);
    assentar(&mut h);
    assert_eq!(pedidos.borrow().as_slice(), [PedidoDialogo::AbrirPasta]);
    assert!(raiz_lida(&h));
}

#[test]
fn abertos_recentemente_mostra_o_icone_de_cada_tipo() {
    let amb = Ambiente::com_pasta("menu_recentes_tipos", &[]);
    let dados = amb.pasta_de_dados();
    let outra = amb.pasta().join("outra");
    std::fs::create_dir_all(&outra).expect("outra pasta");
    // como o serviço guarda: os caminhos resolvidos (sem os nomes curtos 8.3 da pasta temporária)
    let (dados, outra) = (real(&dados), real(&outra));
    for (c, tipo) in [
        (dados.join("estoque_2026.db"), prefs::TipoRecente::Banco),
        (dados.join("clientes.csv"), prefs::TipoRecente::Arquivo),
        (outra.clone(), prefs::TipoRecente::Pasta),
    ] {
        amb.servico.prefs().adicionar_recente_de(&c, tipo);
    }
    let mut h = harness(&amb);
    abrir_menu_arquivo(&mut h);
    abrir_submenu(&mut h, "Abertos recentemente");
    for nome in ["estoque_2026.db", "clientes.csv", "outra"] {
        assert_eq!(item_de_menu(&h, nome).rect().height(), 44.0, "{nome}");
    }
    capturar(&mut h, "menu-arquivo-recentes-tipos-claro.png");
    // o clique abre pelo tipo: a pasta
    clicar_item(&mut h, "outra");
    assentar(&mut h);
    assert_eq!(pasta_aberta(&h), Some(real(&outra)));
    // a pasta aberta sai do submenu; o arquivo abre a ficha
    abrir_menu_arquivo(&mut h);
    abrir_submenu(&mut h, "Abertos recentemente");
    assert!(h.query_all_by_label("outra").all(|n| n.rect().height() != 44.0), "a pasta aberta não aparece");
    clicar_item(&mut h, "clientes.csv");
    assentar(&mut h);
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("clientes.csv")));
    // e o banco, a troca
    abrir_menu_arquivo(&mut h);
    abrir_submenu(&mut h, "Abertos recentemente");
    clicar_item(&mut h, "estoque_2026.db");
    assentar(&mut h);
    assert_eq!(h.state().estado().banco.as_ref().map(|b| b.nome.as_str()), Some("estoque_2026.db"));
}

#[test]
fn status_com_pasta_e_sem_banco() {
    let amb = Ambiente::com_pasta("status_pasta", &[]);
    let mut h = harness(&amb);
    abrir_dados(&mut h, &amb);
    assert!(existe(&h, Role::Label, "dados"), "{:?}", textos(&h));
    assert!(!existe(&h, Role::Label, "Nenhuma pasta aberta") && !existe(&h, Role::Label, "Gravação imediata"));
    let dica = ler_dica(&mut h, Role::Label, "dados");
    assert_eq!(dica, real(&amb.pasta_de_dados()).display().to_string());
    capturar(&mut h, "status-pasta-claro.png");
    // com banco, o banco e o modo, como antes
    emitir_e_assentar(
        &mut h,
        Acao::AbrirBanco { caminho: amb.pasta_de_dados().join("exemplo.db"), somente_leitura: false },
    );
    assert!(existe(&h, Role::Label, "Gravação imediata"));
}

#[test]
fn status_sem_nada_diz_nenhuma_pasta_aberta() {
    let amb = Ambiente::vazio("status_sem_nada");
    let h = harness(&amb);
    assert!(existe(&h, Role::Label, "Nenhuma pasta aberta"), "{:?}", textos(&h));
    assert!(!existe(&h, Role::Label, "Nenhum banco aberto"));
}

#[test]
fn titulo_com_pasta() {
    let amb = Ambiente::com_pasta("titulo_pasta", &[]);
    let mut h = harness(&amb);
    abrir_dados(&mut h, &amb);
    assert_eq!(h.state().titulo(), "dados · DataBox");
    emitir_e_assentar(&mut h, Acao::FecharPasta);
    assert_eq!(h.state().titulo(), "DataBox");
}

#[test]
fn janela_de_atalhos_com_os_novos() {
    let amb = Ambiente::vazio("atalhos_novos");
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    h.run_steps(2);
    for frase in ["Abrir uma pasta", "Abrir um arquivo", "Copiar o caminho do item do Explorador"] {
        assert!(tem_texto(&h, frase), "{frase}: {:?}", textos(&h));
    }
}

// ---- soltar na janela (passo 13) --------------------------------------------------------------------------------

/// Um arquivo solto na janela, como o egui-winit entrega (só o caminho importa).
#[derive(Debug)]
struct Solto(PathBuf);

impl egui::DroppedFile for Solto {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

/// Solta os caminhos na janela (num quadro só, como o `RawInput::dropped_files`) e espera as tarefas.
fn soltar(h: &mut H, caminhos: &[PathBuf]) {
    for c in caminhos {
        h.input_mut().dropped_files.push(std::sync::Arc::new(Solto(c.clone())));
    }
    h.run_steps(2);
    assentar(h);
}

#[test]
fn soltar_pasta_abre_a_pasta() {
    let amb = Ambiente::com_pasta("soltar_pasta", &[]);
    let mut h = harness(&amb);
    soltar(&mut h, &[amb.pasta_de_dados()]);
    assert_eq!(pasta_aberta(&h), Some(real(&amb.pasta_de_dados())));
    assert!(raiz_lida(&h));
    assert!(toast_atual(&h.ctx).is_none(), "um item só: sem aviso");
}

#[test]
fn soltar_banco_abre_o_banco() {
    let amb = Ambiente::com_pasta("soltar_banco", &[]);
    let mut h = harness(&amb);
    soltar(&mut h, &[amb.pasta_de_dados().join("sem_extensao")]);
    assert_eq!(h.state().estado().banco.as_ref().map(|b| b.nome.as_str()), Some("sem_extensao"));
    assert_eq!(h.state().rota(), &Rota::Banco);
}

#[test]
fn soltar_arquivo_de_dados_abre_a_ficha() {
    let amb = Ambiente::com_pasta("soltar_arquivo", &[]);
    let mut h = harness(&amb);
    soltar(&mut h, &[amb.pasta_de_dados().join("planilha.xlsx")]);
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("planilha.xlsx")));
    assert!(abas(&h).contains(&("planilha.xlsx".to_string(), true)));
}

#[test]
fn soltar_varios_abre_o_primeiro_com_aviso() {
    let amb = Ambiente::com_pasta("soltar_varios", &[]);
    let dados = amb.pasta_de_dados();
    let mut h = harness(&amb);
    // a pasta vem primeiro, mesmo solta depois de um arquivo
    h.input_mut().dropped_files.push(std::sync::Arc::new(Solto(dados.join("clientes.csv"))));
    h.input_mut().dropped_files.push(std::sync::Arc::new(Solto(dados.clone())));
    h.run_steps(2);
    esperar_toast(&mut h, "Só um item abre por vez; abri dados.");
    assentar(&mut h);
    assert_eq!(pasta_aberta(&h), Some(real(&dados)));
    assert_eq!(h.state().rota(), &Rota::Inicio, "o arquivo não abriu");
    capturar(&mut h, "soltar-varios-claro.png");
    // sem pasta entre eles, o primeiro
    soltar(&mut h, &[dados.join("notas.txt"), dados.join("dados.json")]);
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("notas.txt")));
    assert_eq!(fichas(&h), ["notas.txt"]);
    // um tipo que o app não abre, solto antes, não esconde o que abre
    soltar(&mut h, &[dados.join("relatorio.pdf"), dados.join("dados.json")]);
    assert!(matches!(h.state().rota(), Rota::Arquivo { caminho } if caminho.ends_with("dados.json")));
    assert_eq!(fichas(&h), ["notas.txt", "dados.json"]);
}

#[test]
fn soltar_tipo_desconhecido_da_o_toast() {
    let amb = Ambiente::com_pasta("soltar_pdf", &[]);
    let mut h = harness(&amb);
    h.input_mut().dropped_files.push(std::sync::Arc::new(Solto(amb.pasta_de_dados().join("relatorio.pdf"))));
    h.run_steps(2);
    esperar_toast(&mut h, "O DataBox ainda não abre este tipo de arquivo: relatorio.pdf.");
    assentar(&mut h);
    assert_eq!(h.state().rota(), &Rota::Inicio);
    assert!(h.state().estado().pasta.is_none() && !h.state().estado().tem_banco());
}

#[test]
fn soltar_com_modal_aberto_nao_abre() {
    let amb = Ambiente::com_pasta("soltar_modal", &[]);
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Atalhos do teclado");
    h.run_steps(2);
    assert!(h.state().janela_aberta());
    soltar(&mut h, &[amb.pasta_de_dados()]);
    assert!(h.state().estado().pasta.is_none(), "com a janela aberta, nada abre");
    // fechada a janela, soltar de novo abre
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    soltar(&mut h, &[amb.pasta_de_dados()]);
    assert_eq!(pasta_aberta(&h), Some(real(&amb.pasta_de_dados())));
}

// ---- as capturas da parada da etapa 4 (o fechamento) ---------------------------------------------------------------

/// As capturas de conferência (a 1280×800, nos dois temas, com a pasta de dados aberta
/// pela linha de comando e o `exemplo.db` de dentro dela aberto pela árvore): o Explorador com a pasta e uma subpasta
/// abertas (prancheta 1), o menu de contexto de um arquivo (prancheta 15), a ficha do arquivo de dados e o Início com
/// os recentes de cada tipo (prancheta 4). Ficam em `capturas/rust/etapa4-*.png`.
#[test]
fn capturas_da_parada_da_etapa_4() {
    for (tema, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let amb = Ambiente::com_pasta(&format!("capturas_etapa4_{sufixo}"), &[]);
        let dados = amb.pasta_de_dados();
        let partida = Partida { caminho: Some(dados.clone()), ..Partida::default() };
        let mut h = harness_com(&amb, tema, egui::vec2(1280.0, 800.0), partida, |_| None);
        assentar(&mut h);
        assert!(raiz_lida(&h));
        emitir_e_assentar(&mut h, Acao::AbrirBanco { caminho: dados.join("exemplo.db"), somente_leitura: false });
        clicar_link(&mut h, "sub, pasta");
        assentar(&mut h);
        assert!(existe(&h, Role::Link, "interno.csv"));
        capturar(&mut h, &format!("etapa4-explorador-pasta-{sufixo}.png"));
        // o menu de contexto de um arquivo de dados
        h.get(by().role(Role::Link).label("clientes.csv")).click_secondary();
        h.run_steps(3);
        assert!(h.query(by().role(Role::Menu).label("clientes.csv")).is_some(), "o menu de contexto");
        capturar(&mut h, &format!("etapa4-menu-de-contexto-{sufixo}.png"));
        h.key_press(Key::Escape);
        h.run_steps(3);
        // a ficha do arquivo
        emitir_e_assentar(&mut h, Acao::AbrirArquivoDeDados { caminho: dados.join("clientes.csv") });
        assert!(matches!(h.state().rota(), Rota::Arquivo { .. }));
        capturar(&mut h, &format!("etapa4-ficha-{sufixo}.png"));
        // o Início, com a pasta, o banco e o arquivo nos recentes
        h.state_mut().ir(Rota::Inicio);
        assentar(&mut h);
        capturar(&mut h, &format!("etapa4-inicio-{sufixo}.png"));
    }
}
