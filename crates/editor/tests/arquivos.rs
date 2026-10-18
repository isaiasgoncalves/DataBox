//! A casca dos documentos editáveis, com um documento editável de mentira no
//! lugar dos editores de verdade (`Editor::com_editor_de_teste`): o ponto da aba alterada, a barra de status do
//! arquivo, Ctrl+S, Ctrl+Z e Ctrl+Y, o Arquivo › Salvar e a barra (Salvar, Desfazer, Refazer, Trazer colunas) pelas
//! ofertas, as origens de colunas no `Contexto`, o F5 que pergunta antes de descartar e a fila de perguntas ao fechar
//! (a aba, a pasta, a janela, também minimizada; a troca de banco não pergunta), com as três saídas da pergunta
//! "Salvar as alterações em…?" (Não salvar · Cancelar · Salvar), a gravação esperada e a que falha. As capturas da
//! pergunta vão para `capturas/rust/` (fechar-aba-alterada-{claro,escuro}).

mod apoio;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use apoio::*;
use comum::Resultado;
use editor::{Editor, Partida};
use egui::accesskit::Role;
use egui::{Key, Modifiers, Ui, ViewportCommand, ViewportId};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{
    Acao, Contexto, Documento, Estado, FonteDeColunas, Oferta, Ofertas, PastaAberta, Rota, Situacao, StatusDeArquivo,
    TipoDeFonte, Verbo,
};
use interface::pecas::toast::toast_atual;
use interface::tema::Tema;

// ---- o documento de mentira -----------------------------------------------------------------------------------

/// O que o teste vê e controla do documento de mentira (compartilhado com ele).
#[derive(Default)]
struct Painel {
    /// Quantas alterações não salvas há (0: salvo).
    alteracoes: usize,
    /// Uma gravação está em andamento.
    salvando: bool,
    /// A gravação não termina enquanto isto for verdade (o teste solta).
    segurar: bool,
    /// A próxima gravação falha com esta frase.
    falhar: Option<String>,
    /// O erro da última gravação, até a casca o tomar.
    erro: Option<String>,
    salvos: u32,
    atualizados: u32,
    desfeitos: u32,
    refeitos: u32,
    trazidos: u32,
    /// Quantas origens de colunas o `Contexto` trazia no último quadro.
    fontes_vistas: usize,
    /// O campo de texto do documento.
    texto: String,
}

type Compartilhado = Rc<RefCell<Painel>>;

/// Uma origem de colunas de mentira (a de cada documento de mentira).
struct FonteFalsa(String);

impl FonteDeColunas for FonteFalsa {
    fn rotulo(&self) -> String {
        self.0.clone()
    }
    fn tipo(&self) -> TipoDeFonte {
        TipoDeFonte::Csv
    }
    fn nomes(&self) -> Vec<String> {
        vec!["id".into()]
    }
    fn linhas(&self) -> Option<u64> {
        Some(0)
    }
    fn colunas(&self, _: &[usize], _: &mut dyn FnMut(u64, u64) -> Resultado<()>) -> Resultado<Vec<Vec<String>>> {
        Ok(vec![Vec::new()])
    }
}

/// O documento editável de mentira: diz que está alterado, oferece Salvar, Desfazer, Refazer, Atualizar e Trazer
/// colunas, grava "numa tarefa" (até o quadro seguinte, ou enquanto o teste segura) e pode falhar.
struct Falso {
    nome: String,
    p: Compartilhado,
    fonte: Arc<dyn FonteDeColunas>,
}

/// O id do campo de texto do documento de mentira.
fn id_do_campo(nome: &str) -> egui::Id {
    egui::Id::new(("campo-falso", nome))
}

impl Falso {
    /// A gravação "termina" (no próximo quadro em que o teste não a segura).
    fn andar(&self, egui: &egui::Context) {
        let mut p = self.p.borrow_mut();
        if !p.salvando || p.segurar {
            return;
        }
        p.salvando = false;
        match p.falhar.take() {
            Some(e) => p.erro = Some(e),
            None => {
                p.alteracoes = 0;
                p.salvos += 1;
            }
        }
        egui.request_repaint();
    }
}

impl Documento for Falso {
    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.andar(ui.ctx());
        let mut p = self.p.borrow_mut();
        p.fontes_vistas = ctx.fontes().len();
        ui.label(format!("Conteúdo de {}", self.nome));
        ui.add(egui::TextEdit::singleline(&mut p.texto).id(id_do_campo(&self.nome)));
    }

    fn em_segundo_plano(&mut self, egui: &egui::Context, _ctx: &mut Contexto) {
        self.andar(egui);
    }

    fn ofertas(&self, _estado: &Estado) -> Ofertas {
        let p = self.p.borrow();
        let salvar = Oferta::ligada()
            .ligada_se(p.alteracoes > 0 && !p.salvando)
            .com_dica(format!("Salvar {} (Ctrl+S)", self.nome));
        Ofertas::default()
            .com(Verbo::Salvar, salvar)
            .com(Verbo::Desfazer, Oferta::ligada().ligada_se(!p.salvando))
            .com(Verbo::Refazer, Oferta::ligada().ligada_se(!p.salvando))
            .com(Verbo::Atualizar, Oferta::ligada())
            .com(Verbo::TrazerColunas, Oferta::ligada().com_dica(format!("Trazer colunas para {}", self.nome)))
    }

    fn executar(&mut self, verbo: Verbo, _egui: &egui::Context, _ctx: &mut Contexto) {
        let mut p = self.p.borrow_mut();
        match verbo {
            Verbo::Salvar => p.salvando = true,
            Verbo::Desfazer => p.desfeitos += 1,
            Verbo::Refazer => p.refeitos += 1,
            Verbo::Atualizar => {
                p.atualizados += 1;
                p.alteracoes = 0;
            }
            Verbo::TrazerColunas => p.trazidos += 1,
            _ => {}
        }
    }

    fn alterado(&self) -> bool {
        self.p.borrow().alteracoes > 0
    }

    fn alteracoes(&self) -> Option<usize> {
        Some(self.p.borrow().alteracoes)
    }

    fn salvando(&self) -> bool {
        self.p.borrow().salvando
    }

    fn tomar_erro_de_gravacao(&mut self) -> Option<String> {
        self.p.borrow_mut().erro.take()
    }

    fn status_do_arquivo(&self, _estado: &Estado) -> Option<StatusDeArquivo> {
        let situacao = if self.alterado() { Situacao::Alterado } else { Situacao::Salvo };
        Some(StatusDeArquivo::novo(&self.nome, situacao).com_detalhe("CSV").com_detalhe("UTF-8").com_detalhe("CRLF"))
    }

    fn fonte_de_colunas(&self) -> Option<Arc<dyn FonteDeColunas>> {
        Some(Arc::clone(&self.fonte))
    }
}

/// Os documentos de mentira de um teste, pelo nome do arquivo.
#[derive(Default, Clone)]
struct Falsos(Rc<RefCell<HashMap<String, Compartilhado>>>);

impl Falsos {
    /// O painel do arquivo `nome` (criado com `alteracoes` alterações).
    fn novo(&self, nome: &str, alteracoes: usize) -> Compartilhado {
        let p = Rc::new(RefCell::new(Painel { alteracoes, ..Painel::default() }));
        self.0.borrow_mut().insert(nome.to_string(), Rc::clone(&p));
        p
    }
}

/// O editor com os documentos de mentira no lugar dos editores (os arquivos sem painel abrem a tela de sempre).
fn harness_falsos(amb: &Ambiente, falsos: &Falsos, tema: Tema) -> H {
    let mapa = falsos.clone();
    let editor = Editor::novo(Arc::clone(&amb.servico), tema, Partida::default())
        .com_dialogo(|_| None)
        .com_editor_de_teste(move |caminho: &Path| {
            let nome = caminho.file_name()?.to_string_lossy().into_owned();
            let p = Rc::clone(mapa.0.borrow().get(&nome)?);
            let fonte: Arc<dyn FonteDeColunas> = Arc::new(FonteFalsa(nome.clone()));
            Some(Box::new(Falso { nome, p, fonte }) as Box<dyn Documento>)
        });
    harness_editor(editor, tema, TAMANHO)
}

/// A pasta dos arquivos de mentira (existe: a casca lê a pasta quando ela abre).
fn pasta_dos_arquivos(amb: &Ambiente) -> PathBuf {
    let p = amb.pasta().join("dados");
    std::fs::create_dir_all(&p).expect("a pasta");
    p
}

/// Abre a aba do arquivo (o caminho não precisa existir: o documento é de mentira).
fn abrir(h: &mut H, caminho: &Path) {
    h.state_mut().ir(Rota::arquivo(caminho));
    h.run_steps(3);
}

/// O título da pergunta ao fechar.
fn pergunta(nome: &str) -> String {
    format!("Salvar as alterações em {nome}?")
}

/// A pergunta sobre `nome` está aberta.
fn perguntando(h: &H, nome: &str) -> bool {
    existe(h, Role::Dialog, &pergunta(nome))
}

/// Espera a pergunta sobre `nome` aparecer.
fn esperar_pergunta(h: &mut H, nome: &str) {
    let alvo = nome.to_string();
    esperar(h, &format!("a pergunta sobre {nome}"), move |h| perguntando(h, &alvo));
}

/// Clica num botão da pergunta aberta (o "Salvar" da pergunta, não o da barra de ferramentas).
fn responder(h: &mut H, botao: &str) {
    clicar_ligado_ultimo(h, botao);
    h.run_steps(3);
}

/// Clica no último botão ligado com este rótulo (o da janela, que vem depois da barra na árvore).
fn clicar_ligado_ultimo(h: &mut H, rotulo: &str) {
    h.query_all(by().role(Role::Button).label(rotulo))
        .rfind(|n| !n.accesskit_node().is_disabled())
        .unwrap_or_else(|| panic!("nenhum botão ligado {rotulo:?}: {:?}", textos(h)))
        .click();
}

/// Pede para fechar a janela (como o ✕ do sistema) e devolve os comandos do quadro.
fn pedir_fechar(h: &mut H) -> Vec<ViewportCommand> {
    h.input_mut().viewports.get_mut(&ViewportId::ROOT).expect("viewport").events.push(egui::ViewportEvent::Close);
    h.step();
    h.output().viewport_output.get(&ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default()
}

/// Os comandos da janela dos próximos `n` quadros, juntos.
fn comandos_dos_quadros(h: &mut H, n: usize) -> Vec<ViewportCommand> {
    let mut todos = Vec::new();
    for _ in 0..n {
        h.step();
        todos.extend(h.output().viewport_output.get(&ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default());
    }
    todos
}

fn rotulos(h: &H) -> Vec<String> {
    rotulos_das_abas(h).0
}

// ---- a aba, a barra de status, os atalhos e as ofertas ----------------------------------------------------------

#[test]
fn aba_alterada_mostra_o_ponto() {
    let amb = Ambiente::vazio("aba_alterada_ponto");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    falsos.novo("salvo.csv", 0);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    let pasta = pasta_dos_arquivos(&amb);
    abrir(&mut h, &pasta.join("vendas.csv"));
    abrir(&mut h, &pasta.join("salvo.csv"));
    // o leitor de tela ouve ", alterada" na aba com alteração; a outra não
    assert!(existe(&h, Role::Tab, "vendas.csv, alterada"), "{:?}", textos(&h));
    assert!(existe(&h, Role::Tab, "salvo.csv") && !existe(&h, Role::Tab, "salvo.csv, alterada"));
    // salvo, o ponto some
    p.borrow_mut().alteracoes = 0;
    h.run_steps(2);
    assert!(existe(&h, Role::Tab, "vendas.csv") && !existe(&h, Role::Tab, "vendas.csv, alterada"));
}

#[test]
fn status_de_arquivo_troca_o_lado_esquerdo() {
    let amb = Ambiente::exemplo("status_de_arquivo");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 2);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    // a Visão geral: a barra do banco
    assert!(existe(&h, Role::Label, "Gravação imediata"));
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // a aba de arquivo: o nome e a situação à esquerda, os detalhes à direita; nada do banco nem do SQLite
    let status = rect_de(&h, Role::Label, "Alterado, não salvo");
    assert!(status.top() > documento(&h).bottom(), "na barra de status");
    for detalhe in ["CSV", "UTF-8", "CRLF"] {
        assert!(existe(&h, Role::Label, detalhe), "{detalhe}: {:?}", textos(&h));
    }
    assert!(!existe(&h, Role::Label, "Gravação imediata"), "o modo do banco sai");
    assert!(!tem_texto(&h, "SQLite 3."), "o SQLite sai");
    let nome = h
        .query_all(by().role(Role::Label).label("vendas.csv"))
        .find(|n| n.rect().top() >= status.top() - 1.0)
        .expect("o nome na barra");
    assert!(nome.rect().right() < status.left(), "o nome antes da situação");
    // salvo: "Salvo"
    p.borrow_mut().alteracoes = 0;
    h.run_steps(2);
    assert!(existe(&h, Role::Label, "Salvo") && !existe(&h, Role::Label, "Alterado, não salvo"));
    // de volta à Visão geral, a barra do banco
    h.state_mut().ir(Rota::Banco);
    h.run_steps(3);
    assert!(existe(&h, Role::Label, "Gravação imediata") && !existe(&h, Role::Label, "Salvo"));
}

#[test]
fn ctrl_s_salva_mesmo_com_campo_focado() {
    let amb = Ambiente::vazio("ctrl_s_salva");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // o campo de texto do documento com o foco: o Ctrl+S salva e o foco fica nele
    h.ctx.memory_mut(|m| m.request_focus(id_do_campo("vendas.csv")));
    h.run_steps(2);
    assert!(h.ctx.text_edit_focused());
    h.key_press_modifiers(CTRL, Key::S);
    h.run_steps(3);
    assert_eq!(p.borrow().salvos, 1, "salvou");
    assert!(h.ctx.memory(|m| m.has_focus(id_do_campo("vendas.csv"))), "o foco continua no campo");
    // salvo, o Salvar desliga e o Ctrl+S não faz nada
    h.key_press_modifiers(CTRL, Key::S);
    h.run_steps(3);
    assert_eq!(p.borrow().salvos, 1);
    // numa aba que não oferece o Salvar (o Início), também nada
    h.state_mut().ir(Rota::Inicio);
    h.run_steps(3);
    p.borrow_mut().alteracoes = 1;
    h.key_press_modifiers(CTRL, Key::S);
    h.run_steps(3);
    assert_eq!(p.borrow().salvos, 1);
}

#[test]
fn ctrl_z_e_ctrl_y_so_fora_de_campo_de_texto() {
    let amb = Ambiente::vazio("ctrl_z_y");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // sem campo focado: Ctrl+Z desfaz, Ctrl+Y e Ctrl+Shift+Z refazem
    h.key_press_modifiers(CTRL, Key::Z);
    h.run_steps(2);
    h.key_press_modifiers(CTRL, Key::Y);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers { shift: true, ..CTRL }, Key::Z);
    h.run_steps(2);
    assert_eq!((p.borrow().desfeitos, p.borrow().refeitos), (1, 2));
    // com o campo de texto focado, ficam com ele (o desfazer do egui)
    h.ctx.memory_mut(|m| m.request_focus(id_do_campo("vendas.csv")));
    h.run_steps(2);
    h.event(egui::Event::Text("abc".into()));
    h.run_steps(2);
    assert_eq!(p.borrow().texto, "abc", "o campo tem o foco");
    h.key_press_modifiers(CTRL, Key::Z);
    h.run_steps(2);
    h.key_press_modifiers(CTRL, Key::Y);
    h.run_steps(2);
    assert_eq!((p.borrow().desfeitos, p.borrow().refeitos), (1, 2), "o documento não recebeu");
    assert!(h.ctx.memory(|m| m.has_focus(id_do_campo("vendas.csv"))));
}

#[test]
fn menu_arquivo_salvar_vem_da_oferta() {
    let amb = Ambiente::vazio("menu_salvar_oferta");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    // no Início, o Salvar do menu e o da barra ficam desligados
    abrir_menu_arquivo(&mut h);
    assert!(!item_ligado(&h, "Salvar"));
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!botao_ligado(&h, "Salvar"));
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // no editor alterado, ligados, com a dica da oferta na barra
    assert!(botao_ligado(&h, "Salvar") && botao_ligado(&h, "Desfazer") && botao_ligado(&h, "Refazer"));
    assert!(ler_dica(&mut h, Role::Button, "Salvar").contains("Salvar vendas.csv (Ctrl+S)"));
    abrir_menu_arquivo(&mut h);
    assert!(item_ligado(&h, "Salvar"));
    clicar_item(&mut h, "Salvar");
    h.run_steps(3);
    assert_eq!(p.borrow().salvos, 1, "o item executa o verbo da aba");
    // a barra: o Desfazer e o Trazer colunas vão ao documento
    clicar_primeiro(&mut h, Role::Button, "Desfazer");
    clicar_primeiro(&mut h, Role::Button, "Trazer colunas");
    assert_eq!((p.borrow().desfeitos, p.borrow().trazidos), (1, 1));
    // salvo, o Salvar desliga nos dois
    assert!(!botao_ligado(&h, "Salvar"));
}

#[test]
fn trazer_colunas_tem_o_lugar_dele_na_barra() {
    let amb = Ambiente::exemplo("trazer_colunas_barra");
    let mut h = harness(&amb);
    // logo depois da Nova coluna e antes do Duplicar, desligado fora do editor de CSV, com a dica
    let x = |h: &H, r: &str| botao_da_barra(h, r).rect().left();
    assert!(x(&h, "Nova coluna") < x(&h, "Trazer colunas") && x(&h, "Trazer colunas") < x(&h, "Duplicar"));
    assert!(!botao_ligado(&h, "Trazer colunas"));
    // a dica do desligado repete o rótulo: com ela à vista, o texto aparece duas vezes
    assert_eq!(quantos(&h, "Trazer colunas"), 1);
    botao_da_barra(&h, "Trazer colunas").hover();
    h.run_steps(3);
    assert_eq!(quantos(&h, "Trazer colunas"), 2, "a dica do botão desligado");
}

#[test]
fn contexto_tem_as_fontes_das_abas_abertas() {
    let amb = Ambiente::vazio("contexto_fontes_abas");
    let falsos = Falsos::default();
    let a = falsos.novo("vendas.csv", 0);
    let b = falsos.novo("estoque.csv", 0);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    let pasta = pasta_dos_arquivos(&amb);
    abrir(&mut h, &pasta.join("vendas.csv"));
    assert_eq!(a.borrow().fontes_vistas, 1, "a própria aba");
    abrir(&mut h, &pasta.join("estoque.csv"));
    assert_eq!(b.borrow().fontes_vistas, 2, "as duas abas abertas, a cada quadro");
    // fechada uma, a lista diminui
    h.state_mut().emitir(Acao::FecharAba(None));
    h.run_steps(3);
    assert_eq!(a.borrow().fontes_vistas, 1);
}

/// O F5 e o Atualizar da barra vão ao documento, também alterado: quem pergunta "Descartar as alterações de…?"
/// é a própria tela (a `TelaTexto` pergunta; o documento de mentira não), sem uma segunda pergunta da
/// casca.
#[test]
fn f5_e_o_atualizar_vao_ao_documento_alterado() {
    let amb = Ambiente::vazio("f5_vai_ao_documento");
    let falsos = Falsos::default();
    let p = falsos.novo("config.json", 2);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("config.json"));
    h.key_press(Key::F5);
    h.run_steps(4);
    assert!(!h.state().janela_aberta(), "a casca não pergunta");
    assert_eq!((p.borrow().atualizados, p.borrow().alteracoes), (1, 0));
    p.borrow_mut().alteracoes = 1;
    clicar_primeiro(&mut h, Role::Button, "Atualizar");
    h.run_steps(2);
    assert!(!h.state().janela_aberta());
    assert_eq!(p.borrow().atualizados, 2);
}

// ---- a tela de texto de verdade -----------------------------------------------------------------------------------

/// Espera a aba de texto ler o arquivo e põe o cursor no fim do editor dela (o campo com o texto).
#[cfg(feature = "telas")]
fn editar_o_texto(h: &mut H, conteudo: &str, digitado: &str) {
    let alvo = conteudo.to_string();
    esperar(h, "o editor de texto", move |h| {
        h.query_all(by().role(Role::MultilineTextInput)).any(|n| n.value().is_some_and(|v| v.contains(alvo.as_str())))
    });
    let campo = h
        .query_all(by().role(Role::MultilineTextInput))
        .find(|n| n.value().is_some_and(|v| v.contains(conteudo)))
        .expect("o editor");
    campo.focus();
    h.run_steps(2);
    h.key_press_modifiers(CTRL, Key::End);
    h.event(egui::Event::Text(digitado.into()));
    h.run_steps(3);
}

/// A `TelaTexto` de verdade (o recurso `telas`) na fila ao fechar: a pergunta sem contagem, o Salvar que grava no
/// disco e fecha a aba.
#[cfg(feature = "telas")]
#[test]
fn tela_de_texto_salva_ao_fechar() {
    let amb = Ambiente::vazio("tela_texto_salva_ao_fechar");
    let caminho = pasta_dos_arquivos(&amb).join("notas.txt");
    std::fs::write(&caminho, "primeira linha\n").expect("o arquivo");
    let mut h = harness(&amb);
    abrir(&mut h, &caminho);
    editar_o_texto(&mut h, "primeira linha", "fim");
    assert!(existe(&h, Role::Tab, "notas.txt, alterada"), "{:?}", textos(&h));
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "notas.txt");
    assert!(tem_texto(&h, "Há alterações não salvas. Se não salvar, elas se perdem."), "{:?}", textos(&h));
    responder(&mut h, "Salvar");
    esperar(&mut h, "a aba fechar", |h| !rotulos(h).iter().any(|r| r == "notas.txt"));
    let gravado = std::fs::read_to_string(&caminho).expect("ler");
    assert!(gravado.contains("fim"), "{gravado:?}");
    assert!(!h.state().fila_ao_fechar());
}

/// O arquivo mudou no disco: o Salvar da fila para na pergunta da tela ("Salvar mesmo assim"), que a fila espera com
/// a aba à vista; respondida, a aba fecha. Cancelada, a fila para e a aba fica.
#[cfg(feature = "telas")]
#[test]
fn fila_espera_a_pergunta_da_tela_de_texto() {
    let amb = Ambiente::vazio("fila_espera_pergunta_texto");
    let pasta = pasta_dos_arquivos(&amb);
    let caminho = pasta.join("config.txt");
    std::fs::write(&caminho, "a = 1\n").expect("o arquivo");
    let mut h = harness(&amb);
    abrir(&mut h, &caminho);
    editar_o_texto(&mut h, "a = 1", "b = 2");
    // outro programa grava no arquivo
    std::fs::write(&caminho, "a = 1\nc = 3 de outro programa\n").expect("mudar no disco");
    // a primeira vez, Cancelar na pergunta da tela: a fila para, a aba fica
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "config.txt");
    responder(&mut h, "Salvar");
    esperar_texto(&mut h, "Salvar mesmo assim substitui o que está lá.");
    assert!(h.state().fila_ao_fechar(), "a fila espera a resposta");
    responder(&mut h, "Cancelar");
    esperar(&mut h, "a fila parar", |h| !h.state().fila_ao_fechar());
    assert!(rotulos(&h).iter().any(|r| r == "config.txt"), "a aba fica: {:?}", rotulos(&h));
    // a segunda, Salvar mesmo assim: grava e fecha
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "config.txt");
    responder(&mut h, "Salvar");
    esperar_texto(&mut h, "Salvar mesmo assim substitui o que está lá.");
    clicar_ligado_ultimo(&mut h, "Salvar mesmo assim");
    esperar(&mut h, "a aba fechar", |h| !rotulos(h).iter().any(|r| r == "config.txt"));
    assert!(std::fs::read_to_string(&caminho).expect("ler").contains("b = 2"));
}

// ---- a tela de CSV de verdade -------------------------------------------------------------------------------------

/// Espera a aba de CSV abrir o arquivo e começa a editar a primeira célula de dados (o clique nela e o Enter), com
/// o texto todo trocado por `digitado` (o campo continua aberto).
#[cfg(feature = "telas")]
fn editar_a_primeira_celula(h: &mut H, rodape: &str, digitado: &str) {
    let alvo = rodape.to_string();
    esperar(h, "o CSV na grade", move |h| tem_texto(h, &alvo));
    let grade = h.get(by().label("Grade")).rect();
    // o corpo da grade (abaixo do cabeçalho), depois da caixa de marcar (32) e do número da linha (48)
    let pos = egui::pos2(grade.left() + 32.0 + 48.0 + 20.0, grade.top() + 14.0);
    h.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        let button = egui::PointerButton::Primary;
        h.event(egui::Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    h.event(egui::Event::Text(digitado.into()));
    h.run_steps(3);
}

/// A `TelaCsv` de verdade na fila ao fechar: a pergunta com a contagem ("Há 1 alteração não salva."), o Salvar que
/// grava no disco e fecha a aba.
#[cfg(feature = "telas")]
#[test]
fn tela_de_csv_salva_ao_fechar() {
    let amb = Ambiente::vazio("tela_csv_salva_ao_fechar");
    let caminho = pasta_dos_arquivos(&amb).join("vendas.csv");
    std::fs::write(&caminho, "regiao;qtd\nSul;1\nNorte;2\n").expect("o arquivo");
    let mut h = harness(&amb);
    abrir(&mut h, &caminho);
    editar_a_primeira_celula(&mut h, "2 linhas · 2 colunas", "Oeste");
    h.key_press(Key::Enter);
    esperar(&mut h, "a alteração", |h| existe(h, Role::Tab, "vendas.csv, alterada"));
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "vendas.csv");
    assert!(tem_texto(&h, "Há 1 alteração não salva. Se não salvar, ela se perde."), "{:?}", textos(&h));
    responder(&mut h, "Salvar");
    esperar(&mut h, "a aba fechar", |h| !rotulos(h).iter().any(|r| r == "vendas.csv"));
    assert_eq!(std::fs::read_to_string(&caminho).expect("ler"), "regiao;qtd\nOeste;1\nNorte;2\n");
    assert!(!h.state().fila_ao_fechar());
}

/// O Ctrl+S com a célula ainda em edição grava a célula antes e salva o arquivo (o "Salvo" volta à aba).
#[cfg(feature = "telas")]
#[test]
fn ctrl_s_no_csv_grava_a_celula_em_edicao() {
    let amb = Ambiente::vazio("ctrl_s_no_csv");
    let caminho = pasta_dos_arquivos(&amb).join("estoque.csv");
    std::fs::write(&caminho, "codigo;qtd\r\nA1;5\r\nB2;7\r\n").expect("o arquivo");
    let mut h = harness(&amb);
    abrir(&mut h, &caminho);
    editar_a_primeira_celula(&mut h, "2 linhas · 2 colunas", "Z9");
    h.key_press_modifiers(CTRL, Key::S);
    esperar(&mut h, "o arquivo gravado", |_| {
        std::fs::read_to_string(&caminho).is_ok_and(|t| t.starts_with("codigo;qtd\r\nZ9;5"))
    });
    esperar(&mut h, "a aba salva", |h| existe(h, Role::Tab, "estoque.csv"));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("estoque.csv salvo."));
}

// ---- a fila de perguntas ao fechar ------------------------------------------------------------------------------

#[test]
fn pergunta_ao_fechar_aba_alterada_tem_tres_saidas() {
    for (tema, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let amb = Ambiente::vazio(&format!("pergunta_tres_saidas_{sufixo}"));
        let falsos = Falsos::default();
        falsos.novo("vendas_2025.csv", 1);
        let mut h = harness_falsos(&amb, &falsos, tema);
        abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas_2025.csv"));
        // o Ctrl+W numa aba alterada: a pergunta, e a aba ainda aberta
        h.key_press_modifiers(CTRL, Key::W);
        esperar_pergunta(&mut h, "vendas_2025.csv");
        assert!(rotulos(&h).iter().any(|r| r == "vendas_2025.csv"));
        assert!(tem_texto(&h, "Há 1 alteração não salva. Se não salvar, ela se perde."), "{:?}", textos(&h));
        // as três saídas, na ordem da prancheta 15 (N2): Não salvar à esquerda, Cancelar e Salvar à direita
        let x = |h: &H, r: &str| {
            h.query_all(by().role(Role::Button).label(r)).last().unwrap_or_else(|| panic!("{r}")).rect().left()
        };
        assert!(x(&h, "Não salvar") < x(&h, "Cancelar") && x(&h, "Cancelar") < x(&h, "Salvar"));
        assert!(h.state().fila_ao_fechar());
        capturar(&mut h, &format!("fechar-aba-alterada-{sufixo}.png"));
    }
    // o plural e o texto sem contagem
    let amb = Ambiente::vazio("pergunta_plural");
    let falsos = Falsos::default();
    falsos.novo("estoque.csv", 1_204);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("estoque.csv"));
    fechar_aba(&mut h, "estoque.csv, alterada");
    esperar_pergunta(&mut h, "estoque.csv");
    assert!(tem_texto(&h, "Há 1.204 alterações não salvas. Se não salvar, elas se perdem."), "{:?}", textos(&h));
}

#[test]
fn nao_salvar_fecha_a_aba() {
    let amb = Ambiente::vazio("nao_salvar_fecha");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 3);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // o × da aba (o rótulo dela tem ", alterada")
    fechar_aba(&mut h, "vendas.csv, alterada");
    esperar_pergunta(&mut h, "vendas.csv");
    responder(&mut h, "Não salvar");
    assentar(&mut h);
    assert!(!rotulos(&h).iter().any(|r| r.starts_with("vendas.csv")), "{:?}", rotulos(&h));
    assert_eq!(p.borrow().salvos, 0, "nada gravou");
    assert!(!h.state().fila_ao_fechar() && !h.state().janela_aberta());
    // uma aba sem alteração fecha sem perguntar
    let q = falsos.novo("limpo.csv", 0);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("limpo.csv"));
    h.state_mut().emitir(Acao::FecharAba(None));
    h.run_steps(3);
    assert!(!h.state().janela_aberta() && !rotulos(&h).iter().any(|r| r == "limpo.csv"));
    assert_eq!(q.borrow().salvos, 0);
}

#[test]
fn esc_na_pergunta_cancela() {
    let amb = Ambiente::vazio("esc_cancela");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "vendas.csv");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!perguntando(&h, "vendas.csv") && !h.state().fila_ao_fechar());
    assert!(rotulos(&h).iter().any(|r| r == "vendas.csv"), "a aba fica");
    assert_eq!((p.borrow().salvos, p.borrow().alteracoes), (0, 1));
}

#[test]
fn salvar_espera_a_gravacao_e_fecha() {
    let amb = Ambiente::vazio("salvar_espera");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 2);
    p.borrow_mut().segurar = true;
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "vendas.csv");
    responder(&mut h, "Salvar");
    // a gravação em andamento: a aba espera aberta, sem outra pergunta
    h.run_steps(5);
    assert!(p.borrow().salvando && h.state().fila_ao_fechar());
    assert!(rotulos(&h).iter().any(|r| r.starts_with("vendas.csv")), "{:?}", rotulos(&h));
    assert!(!h.state().janela_aberta());
    // terminou: a aba fecha
    p.borrow_mut().segurar = false;
    esperar(&mut h, "a aba fechar", |h| !rotulos(h).iter().any(|r| r.starts_with("vendas.csv")));
    assert_eq!(p.borrow().salvos, 1);
    assert!(!h.state().fila_ao_fechar());
}

/// O fechar da janela pergunta de a.csv; enquanto o Salvar dele grava, b.csv (que não estava alterado) muda: antes de
/// fechar, a fila confere de novo e pergunta de b.csv também (nada fecha calado).
#[test]
fn aba_alterada_durante_a_gravacao_tambem_pergunta() {
    let amb = Ambiente::vazio("alterada_durante");
    let falsos = Falsos::default();
    let a = falsos.novo("a.csv", 1);
    let b = falsos.novo("b.csv", 0);
    a.borrow_mut().segurar = true;
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("a.csv"));
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("b.csv"));
    let _ = pedir_fechar(&mut h);
    esperar_pergunta(&mut h, "a.csv");
    responder(&mut h, "Salvar");
    h.run_steps(3);
    b.borrow_mut().alteracoes = 1;
    a.borrow_mut().segurar = false;
    esperar_pergunta(&mut h, "b.csv");
    assert!(h.state().fila_ao_fechar());
    responder(&mut h, "Cancelar");
    assert!(!h.state().fila_ao_fechar());
    assert!(rotulos(&h).iter().any(|r| r.starts_with("b.csv")), "a aba alterada continua aberta");
}

#[test]
fn salvar_com_erro_mantem_a_aba_e_da_o_toast() {
    let amb = Ambiente::vazio("salvar_com_erro");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    let erro = "Não foi possível gravar vendas.csv: o disco está cheio.";
    p.borrow_mut().falhar = Some(erro.to_string());
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    h.key_press_modifiers(CTRL, Key::W);
    esperar_pergunta(&mut h, "vendas.csv");
    responder(&mut h, "Salvar");
    h.run_steps(4);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some(erro));
    assert!(rotulos(&h).iter().any(|r| r == "vendas.csv"), "a aba fica: {:?}", rotulos(&h));
    assert!(!h.state().fila_ao_fechar() && !h.state().janela_aberta(), "tudo para");
    assert_eq!((p.borrow().salvos, p.borrow().alteracoes), (0, 1));
}

#[test]
fn cancelar_para_a_fila_toda() {
    let amb = Ambiente::vazio("cancelar_fila");
    let falsos = Falsos::default();
    let a = falsos.novo("a.csv", 1);
    let b = falsos.novo("b.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    let pasta = pasta_dos_arquivos(&amb);
    abrir(&mut h, &pasta.join("a.csv"));
    abrir(&mut h, &pasta.join("b.csv"));
    // fechar a janela: a pergunta da primeira (a aba dela vem à frente); Cancelar para tudo
    assert!(pedir_fechar(&mut h).contains(&ViewportCommand::CancelClose));
    esperar_pergunta(&mut h, "a.csv");
    responder(&mut h, "Cancelar");
    h.run_steps(3);
    assert!(!perguntando(&h, "b.csv") && !h.state().fila_ao_fechar());
    assert_eq!(rotulos(&h).iter().filter(|r| r.ends_with(".csv")).count(), 2);
    assert!(!comandos_dos_quadros(&mut h, 3).contains(&ViewportCommand::Close), "a janela fica");
    assert_eq!((a.borrow().salvos, b.borrow().salvos), (0, 0));
}

#[test]
fn fechar_pasta_pergunta_uma_por_arquivo_alterado_de_dentro() {
    let amb = Ambiente::vazio("fechar_pasta_pergunta");
    let falsos = Falsos::default();
    let a = falsos.novo("a.csv", 1);
    falsos.novo("limpo.csv", 0);
    let b = falsos.novo("b.json", 1);
    let fora = falsos.novo("fora.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    let pasta = pasta_dos_arquivos(&amb);
    let outra = amb.pasta().join("outra");
    let ctx = h.ctx.clone();
    h.state_mut().simular_pasta(&ctx, Some(PastaAberta::nova(pasta.clone())));
    h.run_steps(2);
    for nome in ["a.csv", "limpo.csv", "b.json"] {
        abrir(&mut h, &pasta.join(nome));
    }
    abrir(&mut h, &outra.join("fora.csv"));
    // Fechar pasta: uma pergunta por arquivo alterado de dentro dela, na ordem das abas (o de fora não)
    abrir_menu_arquivo(&mut h);
    clicar_item(&mut h, "Fechar pasta");
    esperar_pergunta(&mut h, "a.csv");
    assert!(h.state().estado().pasta.is_some(), "a pasta espera as respostas");
    assert_eq!(rotulos_das_abas(&h).1, "a.csv", "a aba da pergunta vem à frente");
    responder(&mut h, "Salvar");
    esperar_pergunta(&mut h, "b.json");
    assert_eq!(a.borrow().salvos, 1);
    responder(&mut h, "Não salvar");
    esperar(&mut h, "a pasta fechar", |h| h.state().estado().pasta.is_none());
    assentar(&mut h);
    // as de dentro fecharam (também a limpa); a de fora fica, sem pergunta
    let abertas = rotulos(&h);
    assert!(
        !abertas.iter().any(|r| r.starts_with("a.csv") || r.starts_with("b.json") || r == "limpo.csv"),
        "{abertas:?}"
    );
    assert!(abertas.iter().any(|r| r.starts_with("fora.csv")), "{abertas:?}");
    assert!(!perguntando(&h, "fora.csv"));
    assert_eq!((b.borrow().salvos, fora.borrow().salvos), (0, 0));
}

#[test]
fn fechar_a_janela_pergunta_dos_arquivos_antes_da_transacao() {
    let amb = Ambiente::exemplo("fechar_janela_arquivos_antes");
    let falsos = Falsos::default();
    falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // a transação aberta por fora
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN;", 10).expect("begin");
    h.state_mut().emitir(Acao::RecarregarBanco);
    assentar(&mut h);
    assert!(h.state().estado().transacao_aberta());
    // primeiro o arquivo
    assert!(pedir_fechar(&mut h).contains(&ViewportCommand::CancelClose));
    esperar_pergunta(&mut h, "vendas.csv");
    assert!(!tem_texto(&h, "Fechar o DataBox descarta a transação aberta"));
    responder(&mut h, "Não salvar");
    // depois a transação, que fecha
    esperar_texto(&mut h, "Fechar o DataBox descarta a transação aberta");
    clicar_ligado_ultimo(&mut h, "Descartar e fechar");
    assert!(comandos_dos_quadros(&mut h, 4).contains(&ViewportCommand::Close), "confirmado, a janela fecha");
    // sem transação nem exportação, a fila fecha a janela sozinha
    let amb = Ambiente::vazio("fechar_janela_so_arquivos");
    let falsos = Falsos::default();
    falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    assert!(pedir_fechar(&mut h).contains(&ViewportCommand::CancelClose));
    esperar_pergunta(&mut h, "vendas.csv");
    clicar_ligado_ultimo(&mut h, "Não salvar");
    assert!(comandos_dos_quadros(&mut h, 4).contains(&ViewportCommand::Close));
    // o pedido que chega depois (o eframe devolve o Close como pedido de fechar) não pergunta de novo
    assert!(!pedir_fechar(&mut h).contains(&ViewportCommand::CancelClose));
}

#[test]
fn pedido_de_fechar_minimizado_tambem_pergunta() {
    let amb = Ambiente::vazio("fechar_minimizado_arquivo");
    let falsos = Falsos::default();
    falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // minimizada, o eframe só chama o `App::logic`: com a aba alterada, cancela e restaura a janela
    let mut raw = egui::RawInput::default();
    let info = raw.viewports.entry(ViewportId::ROOT).or_default();
    info.minimized = Some(true);
    info.events.push(egui::ViewportEvent::Close);
    let ctx = h.ctx.clone();
    let saida = ctx.run_logic(&raw, |c| h.state_mut().logica(c));
    let comandos = saida.viewport_commands.get(&ViewportId::ROOT).cloned().unwrap_or_default();
    assert!(comandos.contains(&ViewportCommand::CancelClose), "{comandos:?}");
    assert!(comandos.contains(&ViewportCommand::Minimized(false)));
    // o primeiro quadro da janela restaurada pergunta do arquivo
    esperar_pergunta(&mut h, "vendas.csv");
}

#[test]
fn troca_de_banco_nao_pergunta_do_arquivo() {
    let amb = Ambiente::exemplo("troca_sem_pergunta");
    let falsos = Falsos::default();
    let p = falsos.novo("vendas.csv", 1);
    let mut h = harness_falsos(&amb, &falsos, Tema::Claro);
    abrir(&mut h, &pasta_dos_arquivos(&amb).join("vendas.csv"));
    // fechar o banco fecha as abas dele e mantém as de arquivo, sem perguntar
    h.state_mut().emitir(Acao::FecharBanco);
    esperar(&mut h, "o banco fechar", |h| !h.state().estado().tem_banco());
    assentar(&mut h);
    assert!(!h.state().janela_aberta() && !h.state().fila_ao_fechar());
    assert!(rotulos(&h).iter().any(|r| r == "vendas.csv"), "{:?}", rotulos(&h));
    assert_eq!(p.borrow().alteracoes, 1, "a alteração continua");
    // e abrir outro banco também não
    h.state_mut().emitir(Acao::AbrirBanco { caminho: amb.caminho_banco(), somente_leitura: false });
    esperar(&mut h, "o banco abrir", |h| h.state().estado().tem_banco());
    assentar(&mut h);
    assert!(!h.state().janela_aberta());
    assert!(rotulos(&h).iter().any(|r| r == "vendas.csv"));
}
