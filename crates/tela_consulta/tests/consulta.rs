//! A Consulta SQL no `egui_kittest` com o banco de exemplo, desenhada
//! como a casca desenha (`palco_documento`: a barra com os verbos e a busca, o documento com a faixa, o conteúdo e o
//! rodapé, e as Propriedades à direita): execução, seletor de resultados, erro, transação, atalhos, seleção,
//! endereço, busca e o pedido de exportação. As capturas vão para `capturas/rust/`.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Ui, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::{Ambiente, NoPalco, Palco, assentar, capturar, esperar, esperar_texto, palco_documento};
use estado::{Acao, Contexto, Documento, OrigemJanelaExportar, Promessa, Rota};
use interface::pecas::documento::Documento as DocumentoPeca;
use interface::tema::Tema;
use tela_consulta::{ALTURA_EDITOR, ALTURA_MIN_EDITOR, TelaConsulta};

type H = Harness<'static, Palco<NoPalco<TelaConsulta>>>;
type H2 = Harness<'static, Palco<NoPalco<Duas>>>;

/// O tamanho de sempre (a prancheta 3).
const TELA: Vec2 = vec2(1440.0, 900.0);

/// Um SELECT que leva perto de um segundo (para a execução ainda estar rodando quando o teste age).
const LENTA: &str = "WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 4000000) \
                     SELECT count(*) AS n FROM s;";

/// O roteiro da prancheta 3: um SELECT, um UPDATE e um SELECT numa tabela que não existe.
const ROTEIRO: &str = "SELECT id, nome, email, cidade, limite FROM clientes ORDER BY id LIMIT 8;\n\
                       UPDATE pedidos SET observacao = observacao WHERE id <= 3;\n\
                       SELECT * FROM tabela_que_nao_existe;\n";

/// Duas Consultas como duas abas da casca: cada uma com o seu [`TelaConsulta::com_id`]; só a `ativa` é desenhada, e
/// a outra só recebe em segundo plano.
struct Duas {
    telas: [TelaConsulta; 2],
    ativa: usize,
}

impl Duas {
    fn ativa(&mut self) -> &mut TelaConsulta {
        &mut self.telas[self.ativa]
    }
}

impl Documento for Duas {
    fn anatomia(&self) -> estado::Anatomia {
        self.telas[self.ativa].anatomia()
    }
    fn avisos(&mut self, doc: &mut DocumentoPeca, ui: &mut Ui, ctx: &mut Contexto) {
        self.ativa().avisos(doc, ui, ctx);
    }
    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ativa().faixa(ui, ctx);
    }
    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let egui = ui.ctx().clone();
        let ativa = self.ativa;
        for (i, tela) in self.telas.iter_mut().enumerate() {
            if i == ativa {
                tela.conteudo(ui, ctx);
            } else {
                tela.em_segundo_plano(&egui, ctx);
            }
        }
    }
    fn rodape(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ativa().rodape(ui, ctx);
    }
    fn ofertas(&self, estado: &estado::Estado) -> estado::Ofertas {
        self.telas[self.ativa].ofertas(estado)
    }
    fn executar(&mut self, verbo: estado::Verbo, egui: &egui::Context, ctx: &mut Contexto) {
        Documento::executar(self.ativa(), verbo, egui, ctx);
    }
    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        self.ativa().busca()
    }
    fn evento_busca(&mut self, evento: estado::EventoBusca, egui: &egui::Context, ctx: &mut Contexto) {
        self.ativa().evento_busca(evento, egui, ctx);
    }
    fn tem_propriedades(&self) -> bool {
        true
    }
    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ativa().propriedades(ui, ctx);
    }
}

/// A tela que os apoios leem: a do palco, ou a aba ativa das [`Duas`].
trait TemTela {
    fn tela(&self) -> &TelaConsulta;
}

impl TemTela for NoPalco<TelaConsulta> {
    fn tela(&self) -> &TelaConsulta {
        &self.doc
    }
}

impl TemTela for NoPalco<Duas> {
    fn tela(&self) -> &TelaConsulta {
        &self.doc.telas[self.doc.ativa]
    }
}

fn abrir(amb: &Ambiente, sql: Option<&str>, tema: Tema) -> H {
    abrir_em(amb, sql, tema, TELA)
}

fn abrir_em(amb: &Ambiente, sql: Option<&str>, tema: Tema, tamanho: Vec2) -> H {
    let tela = TelaConsulta::nova(sql.map(str::to_string));
    let mut h = palco_documento(amb, tela, tema, tamanho);
    esperar(&mut h, |h| h.query_by_label("Comando SQL").is_some());
    assentar(&mut h);
    h
}

/// As [`Duas`] no palco, com a primeira à vista.
fn abrir_duas(amb: &Ambiente, telas: [TelaConsulta; 2]) -> H2 {
    let mut h = palco_documento(amb, Duas { telas, ativa: 0 }, Tema::Claro, TELA);
    esperar(&mut h, |h| h.query_by_label("Comando SQL").is_some());
    assentar(&mut h);
    h
}

/// Põe a aba `i` das [`Duas`] à vista.
fn mostrar_aba(h: &mut H2, i: usize) {
    h.state_mut().tela.doc.ativa = i;
    h.run_steps(2);
    esperar(h, |h| h.query_by_label("Comando SQL").is_some());
}

/// Arrasta com o botão principal de `de` até `ate`, em alguns passos.
fn arrastar(h: &mut H, de: Pos2, ate: Pos2) {
    let botao = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.event(Event::PointerMoved(de));
    h.run_steps(1);
    h.event(botao(de, true));
    h.run_steps(1);
    for i in 1..=6 {
        h.event(Event::PointerMoved(de + (ate - de) * (i as f32 / 6.0)));
        h.run_steps(1);
    }
    h.event(botao(ate, false));
    h.run_steps(2);
}

/// Espera uma promessa (a execução que a casca pegou da tela).
fn esperar_promessa<T: Send + 'static>(p: &mut Promessa<T>) -> comum::Resultado<T> {
    let inicio = Instant::now();
    loop {
        if let Some(r) = p.tomar() {
            return r;
        }
        assert!(inicio.elapsed() < Duration::from_secs(15), "a execução não terminou");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// O texto de um nó (num rótulo, o AccessKit guarda o texto em value).
fn rotulo(n: &egui_kittest::Node<'_>) -> String {
    let no = n.accesskit_node();
    if no.role() == Role::Label { no.value() } else { no.label() }.unwrap_or_default()
}

fn execucoes<S: TemTela>(h: &Harness<'static, Palco<S>>) -> u64 {
    h.state().tela.tela().execucoes()
}

fn tela(h: &H) -> &TelaConsulta {
    &h.state().tela.doc
}

/// Troca o texto do editor (Ctrl+A e digitação; as quebras de linha vão como Enter).
fn escrever<S: TemTela>(h: &mut Harness<'static, Palco<S>>, sql: &str) {
    h.get_by_label("Comando SQL").click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(1);
    for (i, linha) in sql.split('\n').enumerate() {
        if i > 0 {
            h.key_press(Key::Enter);
        }
        if !linha.is_empty() {
            h.get_by_label("Comando SQL").type_text(linha);
        }
        h.run_steps(1);
    }
    h.run_steps(2);
    assert_eq!(h.state().tela.tela().texto(), sql);
}

/// Executa pelo Executar da barra (ou por Ctrl+Enter) e espera os resultados desta execução.
fn executar<S: TemTela>(h: &mut Harness<'static, Palco<S>>, sql: Option<&str>, atalho: bool) {
    if let Some(s) = sql {
        escrever(h, s);
    }
    let antes = execucoes(h);
    if atalho {
        h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    } else {
        h.get_by_role_and_label(Role::Button, "Executar").click();
    }
    esperar(h, move |h| h.state().tela.tela().execucoes() > antes);
}

/// Seleciona no editor `id` do caractere `inicio` até `fim`.
fn selecionar<S>(h: &mut Harness<'static, Palco<S>>, id: egui::Id, inicio: usize, fim: usize) {
    let mut estado = egui::text_edit::TextEditState::load(&h.ctx, id).unwrap_or_default();
    estado.cursor.set_char_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(inicio),
        egui::text::CCursor::new(fim),
    )));
    estado.store(&h.ctx, id);
    h.run_steps(2);
}

/// A seleção guardada do editor `id` (em ordem).
fn selecao<S>(h: &Harness<'static, Palco<S>>, id: egui::Id) -> Option<(usize, usize)> {
    egui::text_edit::TextEditState::load(&h.ctx, id).and_then(|s| s.cursor.char_range()).map(|r| {
        let [a, b] = r.sorted_cursors();
        (a.index.0, b.index.0)
    })
}

fn ler(amb: &Ambiente, sql: &str) -> Vec<Vec<comum::Celula>> {
    // uma conexão à parte: o que está no arquivo, fora da transação da tela
    let banco = banco::Banco::abrir(&amb.caminho_banco(), true, false).expect("abrir o banco à parte");
    let r = banco.consultar(sql, 100).expect("consultar");
    banco.fechar();
    match r.resultados.into_iter().next() {
        Some(banco::ResultadoComando::Linhas { linhas, .. }) => linhas,
        outro => panic!("esperava linhas: {outro:?}"),
    }
}

fn inteiro(c: &comum::Celula) -> i64 {
    match c {
        comum::Celula::Inteiro(n) => *n,
        outro => panic!("esperava inteiro: {outro:?}"),
    }
}

fn texto_da_celula(c: &comum::Celula) -> String {
    match c {
        comum::Celula::Texto(t) => t.clone(),
        outro => panic!("esperava texto: {outro:?}"),
    }
}

/// Grava a captura com o mouse fora da tela (nada em hover) e o foco fora do editor.
fn capturar_sem_hover(h: &mut H, nome: &str) {
    let id = tela(h).id_do_editor();
    h.ctx.memory_mut(|m| m.surrender_focus(id));
    h.event(egui::Event::PointerGone);
    assentar(h);
    capturar(h, nome);
}

/// A casca relê o estado (objetos, banco) depois de `RecarregarObjetos` / `NotarTransacao`.
fn casca_rele<S>(h: &mut Harness<'static, Palco<S>>, amb: &mut Ambiente) {
    amb.recarregar();
    h.state_mut().estado = amb.estado.clone();
    h.run_steps(2);
}

/// O texto do rodapé: o rótulo que começa por `inicio`.
fn rodape<S>(h: &Harness<'static, Palco<S>>, inicio: &str) -> Option<String> {
    h.query_all_by_label_contains(inicio).map(|n| rotulo(&n)).find(|t| t.starts_with(inicio))
}

// ------------------------------------------------------------------ faixa, editor, Propriedades

#[test]
fn tela_tem_faixa_editor_e_tabelas() {
    let amb = Ambiente::exemplo("consulta_faixa_editor");
    let h = abrir(&amb, None, Tema::Claro);
    // a faixa: o banco; a posição do cursor, no rodapé, à direita (U4 da revisão de UI de 22/10/2026)
    assert!(h.query_by_label("exemplo.db").is_some());
    let posicao = h.get_by_label("Linha 1, coluna 1").rect();
    assert!(posicao.top() > h.get_by_label("exemplo.db").rect().bottom() + 200.0, "a posição embaixo: {posicao:?}");
    // saíram o cabeçalho, o card Comando, o select do Histórico e as tags
    assert!(h.query_by_label("Consulta SQL").is_none());
    assert!(h.query_by_role_and_label(Role::ComboBox, "Histórico").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "clientes").is_none());
    // as Propriedades: Execução e Tabelas, com os nós que inserem
    assert!(h.query_by_label("Seção Execução").is_some());
    assert!(h.query_by_label("Seção Tabelas").is_some());
    for nome in ["clientes", "\"Tabela com espaço\"", "v_pedidos_por_cliente"] {
        assert!(h.query_by_role_and_label(Role::Link, &format!("Inserir {nome} no comando")).is_some(), "{nome}");
    }
    // a barra: a busca; com o editor vazio, o Executar fica desligado, e sem resultado, o Exportar (fora do palco)
    assert!(h.query_by_role_and_label(Role::Button, "Executar").is_none());
    assert!(h.query_by_label("Buscar no editor").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Exportar").is_none());
    // o foco nasce no editor, que encosta na faixa e tem 232px
    assert!(h.get_by_label("Comando SQL").is_focused());
    let area = tela(&h).area_editor();
    assert_eq!(area.height(), ALTURA_EDITOR);
    assert!((area.top() - h.get_by_label("exemplo.db").rect().bottom()).abs() < 20.0, "{area:?}");
}

#[test]
fn consulta_rodape_com_a_posicao() {
    let amb = Ambiente::exemplo("consulta_posicao");
    let mut h = abrir(&amb, None, Tema::Claro);
    escrever(&mut h, "SELECT 1;\nSELECT nome FROM clientes");
    assert!(h.query_by_label("Linha 2, coluna 26").is_some());
    h.key_press(Key::Home);
    h.run_steps(2);
    assert!(h.query_by_label("Linha 2, coluna 1").is_some());
    h.key_press(Key::ArrowUp);
    h.run_steps(2);
    assert!(h.query_by_label("Linha 1, coluna 1").is_some());
}

#[test]
fn editor_com_numeros_e_cores() {
    for (tema, nome) in [(Tema::Escuro, "consulta-editor-escuro.png"), (Tema::Claro, "consulta-editor-claro.png")] {
        let amb = Ambiente::exemplo("consulta_editor_cores");
        let sql = "SELECT id, nome FROM clientes -- os oito primeiros\nWHERE cidade = 'Recife' LIMIT 8;";
        let mut h = abrir(&amb, Some(sql), tema);
        // o editor plano ocupa a largura do documento, sem a moldura do campo
        let area = tela(&h).area_editor();
        let painel = h.get_by_label("Seção Execução").rect();
        assert!(area.right() < painel.left() && area.width() > 900.0, "{area:?}");
        capturar_sem_hover(&mut h, nome);
    }
}

#[test]
fn tabelas_das_propriedades_inserem_no_cursor() {
    let amb = Ambiente::exemplo("consulta_tabelas_inserem");
    let mut h = abrir(&amb, None, Tema::Claro);
    escrever(&mut h, "SELECT count(*) AS n FROM");
    h.get_by_role_and_label(Role::Link, "Inserir \"Tabela com espaço\" no comando").click();
    h.run_steps(3);
    // com o espaço antes, depois de uma palavra; o foco volta ao editor
    assert_eq!(tela(&h).texto(), "SELECT count(*) AS n FROM \"Tabela com espaço\"");
    assert!(h.get_by_label("Comando SQL").is_focused());
    executar(&mut h, None, true);
    let (colunas, linhas) = tela(&h).grade(0).expect("grade");
    assert_eq!((colunas, inteiro(&linhas[0][0])), (vec!["n"], 4));
    // a seleção é trocada pelo nome
    escrever(&mut h, "SELECT * FROM xyz");
    let id = tela(&h).id_do_editor();
    selecionar(&mut h, id, 14, 17);
    h.get_by_role_and_label(Role::Link, "Inserir clientes no comando").click();
    h.run_steps(3);
    assert_eq!(tela(&h).texto(), "SELECT * FROM clientes");
}

// ------------------------------------------------------------------ resultados

#[test]
fn select_mostra_grade_e_contagem() {
    let amb = Ambiente::exemplo("consulta_select");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some("SELECT id, nome, limite FROM clientes ORDER BY id"), false);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas"]);
    assert!(h.query_by_label("Comando 1 · 60 linhas").is_some());
    let (colunas, linhas) = tela(&h).grade(0).expect("grade");
    assert_eq!(colunas, ["id", "nome", "limite"]);
    assert_eq!(linhas.len(), 60);
    assert_eq!(inteiro(&linhas[0][0]), 1);
    // limite é NULL quando o id é múltiplo de 7
    assert_eq!(linhas[6][2], comum::Celula::Nulo);
    assert!(h.state().acoes.contains(&Acao::NotarTransacao(false)));
    // as contas da execução nas Propriedades
    assert!(h.query_by_label("nenhuma aberta").is_some());
}

#[test]
fn rodape_com_linhas_e_tempo() {
    let amb = Ambiente::exemplo("consulta_rodape");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some("SELECT * FROM eventos; SELECT * FROM clientes WHERE id < 0"), false);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas", "linhas"]);
    let r = rodape(&h, "mais de 1.000 linhas · ").expect("o rodapé do resultado cortado");
    assert!(r.ends_with(" ms · mostrando as primeiras 1.000"), "{r}");
    assert_eq!(tela(&h).grade(0).map(|(_, l)| l.len()), Some(1000));
    // o segundo, sem linhas: o cabeçalho continua, com o vazio embaixo dele (U5 da revisão de UI de 22/10/2026)
    h.get_by_label("Comando 2 · 0 linhas").click();
    h.run_steps(3);
    assert_eq!(tela(&h).escolhido(), 1);
    let frase = h.get_by_label("A consulta não devolveu linhas.").rect();
    assert!(frase.top() > 400.0, "o vazio embaixo do cabeçalho da grade, não no alto: {frase:?}");
    assert_eq!(tela(&h).grade(1).map(|(c, l)| (c.len(), l.len())), Some((7, 0)), "as colunas, sem linhas");
    capturar(&mut h, "consulta-sem-linhas-claro.png");
    assert!(!tela(&h).ofertas(&h.state().estado).ligado(estado::Verbo::Exportar), "nada a exportar sem linhas");
    assert!(rodape(&h, "0 linhas · ").is_some_and(|t| t.ends_with(" ms")));
}

#[test]
fn seletor_de_comandos_mostra_um_resultado_por_vez() {
    let amb = Ambiente::exemplo("consulta_seletor");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(
        &mut h,
        Some(
            "UPDATE pedidos SET observacao = 'conferido' WHERE id <= 3;\nSELECT id, observacao FROM pedidos WHERE id <= 3 ORDER BY id;",
        ),
        false,
    );
    assert_eq!(tela(&h).tipos_dos_cards(), ["alteracao", "linhas"]);
    // as duas partes do seletor; a tela abre no primeiro com linhas
    let partes: Vec<String> = h.query_all_by_role(Role::Tab).map(|n| rotulo(&n)).collect();
    assert_eq!(partes, ["Comando 1 · 3 linhas alteradas", "Comando 2 · 3 linhas"]);
    assert_eq!(tela(&h).escolhido(), 1);
    let (_, linhas) = tela(&h).grade(1).expect("grade");
    assert!(linhas.iter().all(|l| texto_da_celula(&l[1]) == "conferido"));
    // um resultado por vez: o da alteração, com o comando
    assert!(h.query_by_label("UPDATE pedidos SET observacao = 'conferido' WHERE id <= 3;").is_none());
    h.get_by_label("Comando 1 · 3 linhas alteradas").click();
    h.run_steps(3);
    assert_eq!(tela(&h).escolhido(), 0);
    assert!(h.query_by_label("UPDATE pedidos SET observacao = 'conferido' WHERE id <= 3;").is_some());
    assert!(rodape(&h, "3 linhas alteradas · ").is_some());
    assert_eq!(inteiro(&ler(&amb, "SELECT count(*) FROM pedidos WHERE observacao = 'conferido'")[0][0]), 3);
    // o Explorador relê as contagens
    assert!(h.state().acoes.contains(&Acao::RecarregarObjetos));
}

#[test]
fn faixa_de_erro_com_o_comando_e_a_linha_marcada() {
    let amb = Ambiente::exemplo("consulta_faixa_erro");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some(ROTEIRO.trim_end()), false);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas", "alteracao", "erro"]);
    assert!(h.query_by_label_contains("Comando 3: A tabela não existe: tabela_que_nao_existe.").is_some());
    assert!(h.query_by_label("A execução parou neste comando; os anteriores já foram executados.").is_some());
    // o texto do SQLite em inglês não fica na tela (a frase já cita o nome)
    assert!(h.query_by_label_contains("no such table").is_none());
    let partes: Vec<String> = h.query_all_by_role(Role::Tab).map(|n| rotulo(&n)).collect();
    assert_eq!(partes, ["Comando 1 · 8 linhas", "Comando 2 · 3 linhas alteradas", "Comando 3 · erro"]);
    // a linha do comando com erro (a 3ª) fica marcada, com o nome sublinhado
    let inicio = ROTEIRO.find("tabela_que_nao_existe").unwrap_or_default();
    let n = "tabela_que_nao_existe".len();
    assert_eq!(tela(&h).marcas_do_erro_no_editor(), Some((2..3, Some(inicio..inicio + n))));
    // as contas nas Propriedades
    assert!(h.query_by_label("Comando 3").is_some(), "o Parou em");
    // mexer no texto tira as marcas
    h.get_by_label("Comando SQL").click();
    h.run_steps(2);
    h.get_by_label("Comando SQL").type_text(" ");
    h.run_steps(2);
    assert_eq!(tela(&h).marcas_do_erro_no_editor(), None);
}

#[test]
fn erro_mostra_a_frase_em_portugues_e_o_original_no_balao() {
    let amb = Ambiente::exemplo("consulta_erro");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some("SELEC * FROM clientes;"), false);
    assert_eq!(tela(&h).tipos_dos_cards(), ["erro"]);
    assert!(h.query_by_label_contains("Comando 1: ").is_some());
    // o primeiro comando que falha não tem a nota dos anteriores
    assert!(h.query_by_label_contains("os anteriores já foram executados").is_none());
}

#[test]
fn erro_de_sintaxe_para_no_comando() {
    let amb = Ambiente::exemplo("consulta_sintaxe");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some("SELECT 1 AS um; SELEC * FROM clientes; SELECT 2 AS dois"), false);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas", "erro"]);
    assert_eq!(tela(&h).grade(0).map(|(c, _)| c), Some(vec!["um"]));
    // o comando com erro, no resultado dele
    h.get_by_label("Comando 2 · erro").click();
    h.run_steps(3);
    assert!(h.query_by_label("SELEC * FROM clientes;").is_some());
    // as marcas na mesma linha (o roteiro tem uma só)
    assert_eq!(tela(&h).marcas_do_erro_no_editor().map(|(l, _)| l), Some(0..1));
}

#[test]
fn create_table_aparece_nas_tabelas() {
    let mut amb = Ambiente::exemplo("consulta_create");
    let mut h = abrir(&amb, None, Tema::Claro);
    assert!(h.query_by_role_and_label(Role::Link, "Inserir anotacoes_rapidas no comando").is_none());
    executar(&mut h, Some("CREATE TABLE anotacoes_rapidas (id INTEGER PRIMARY KEY, texto TEXT NOT NULL)"), false);
    assert!(h.query_by_label("Comando 1 · executado").is_some());
    assert!(rodape(&h, "Comando executado · ").is_some());
    assert!(h.state().acoes.contains(&Acao::RecarregarObjetos));
    casca_rele(&mut h, &mut amb);
    assert!(h.query_by_role_and_label(Role::Link, "Inserir anotacoes_rapidas no comando").is_some());
}

#[test]
fn muitos_comandos_mostram_so_os_cem_primeiros() {
    let amb = Ambiente::exemplo("consulta_muitos");
    let mut roteiro = "UPDATE clientes SET cidade = cidade WHERE id = 1;\n".repeat(101);
    roteiro.push_str("SELECT * FROM tabela_que_nao_existe;");
    let mut h = abrir(&amb, Some(&roteiro), Tema::Claro);
    executar(&mut h, None, false);
    assentar(&mut h);
    let tipos = tela(&h).tipos_dos_cards();
    assert_eq!(tipos.len(), 102);
    assert_eq!(tipos[100..], ["mais", "erro"]);
    // a tela abre no último (sem linhas, o erro), que o seletor rola até mostrar
    assert_eq!(tela(&h).escolhido(), 101);
    let erro = h.get_by_label("Comando 102 · erro").rect();
    assert!(erro.left() >= 0.0 && erro.right() <= TELA.x, "{erro:?}");
    assert!(h.query_by_label_contains("Comando 102: A tabela não existe").is_some());
    h.get_by_label("Mais 2 comandos").click();
    h.run_steps(3);
    assert!(h.query_by_label("Mais 2 comandos executados, que não aparecem aqui.").is_some());
    capturar_sem_hover(&mut h, "consulta-mais-comandos-claro.png");
}

#[test]
fn exportar_o_resultado_do_comando_selecionado() {
    let amb = Ambiente::exemplo("consulta_exportar");
    let mut h = abrir(&amb, None, Tema::Claro);
    let sql = "SELECT id, nome, email FROM clientes WHERE ativo = 1 ORDER BY id";
    executar(&mut h, Some(&format!("UPDATE clientes SET cidade = cidade WHERE id = 1; {sql}")), false);
    let ofertas = tela(&h).ofertas(&h.state().estado);
    assert_eq!(ofertas.dica(estado::Verbo::Exportar), "Exportar o resultado do comando 2");
    h.state_mut().acoes.clear();
    h.get_by_role_and_label(Role::Button, "Exportar").click();
    h.run_steps(2);
    let Some(Acao::AbrirExportacao(pedido)) = h.state().acoes.first().cloned() else {
        panic!("esperava AbrirExportacao: {:?}", h.state().acoes)
    };
    assert_eq!(pedido.nome_sugerido, "consulta_clientes");
    assert_eq!(pedido.origem, OrigemJanelaExportar::Consulta { sql: sql.to_string() });
    // com a alteração à vista, o Exportar desliga
    h.get_by_label("Comando 1 · 1 linha alterada").click();
    h.run_steps(3);
    assert!(h.query_by_role_and_label(Role::Button, "Exportar").is_none());
    assert!(!tela(&h).ofertas(&h.state().estado).ligado(estado::Verbo::Exportar));
}

#[test]
fn texto_vazio_avisa_no_toast() {
    let amb = Ambiente::exemplo("consulta_vazio");
    let mut h = abrir(&amb, None, Tema::Claro);
    // a Consulta vazia (prancheta 15): o Executar da barra fica desligado até haver texto (fora do palco); o
    // Ctrl+Enter avisa
    assert!(h.query_by_role_and_label(Role::Button, "Executar").is_none());
    assert!(!tela(&h).ofertas(&h.state().estado).ligado(estado::Verbo::Executar));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run_steps(2);
    assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some("Escreva um comando para executar."));
    assert_eq!(execucoes(&h), 0);
    // só espaços: ainda desligado
    escrever(&mut h, "   ");
    assert!(!tela(&h).ofertas(&h.state().estado).ligado(estado::Verbo::Executar));
    // só comentários: o servidor não acha comando (um aviso só, sem o seletor)
    executar(&mut h, Some("-- nada aqui"), false);
    assert!(
        h.query_by_label("Não havia comando para executar: o texto só tem comentários ou ponto e vírgula.").is_some()
    );
    assert_eq!(h.query_all_by_role(Role::Tab).count(), 0);
}

/// A Consulta vazia (prancheta 15): a dica em itálico e as sugestões "Para começar" com a primeira tabela; o clique
/// numa sugestão põe o SQL no editor (a dica e as sugestões somem) e liga o Executar.
#[test]
fn consulta_vazia_tem_a_dica_e_sugestoes_que_inserem_sql() {
    let amb = Ambiente::exemplo("consulta_vazia");
    let mut h = abrir(&amb, None, Tema::Claro);
    assert!(h.query_by_label(tela_consulta::DICA_EDITOR).is_some());
    assert!(h.query_by_label(tela_consulta::PARA_COMECAR).is_some());
    assert!(h.query_by_label(tela_consulta::NADA_EXECUTADO).is_some(), "o rodapé antes da primeira execução");
    let sugestoes =
        ["SELECT * FROM clientes LIMIT 100", "SELECT count(*) FROM clientes", "PRAGMA table_info(clientes)"];
    for s in sugestoes {
        assert!(h.query_by_role_and_label(Role::Link, s).is_some(), "{s}");
    }
    // as sugestões ficam no editor, na fileira de baixo da dica, depois da calha
    let editor = tela(&h).area_editor();
    let primeira = h.get_by_role_and_label(Role::Link, sugestoes[0]).rect();
    assert!(editor.contains_rect(primeira), "{primeira:?} fora de {editor:?}");
    assert!(primeira.left() >= editor.left() + 46.0 + 14.0 - 0.5);
    // o clique insere o SQL, com o cursor no fim e o foco no editor
    h.get_by_role_and_label(Role::Link, sugestoes[1]).click();
    h.run_steps(3);
    assert_eq!(tela(&h).texto(), sugestoes[1]);
    assert!(h.query_by_label(tela_consulta::DICA_EDITOR).is_none());
    assert!(h.query_by_role_and_label(Role::Link, sugestoes[0]).is_none());
    assert!(h.ctx.memory(|m| m.has_focus(tela(&h).id_do_editor())));
    assert!(h.query_by_label(&format!("Linha 1, coluna {}", sugestoes[1].len() + 1)).is_some());
    assert!(tela(&h).ofertas(&h.state().estado).ligado(estado::Verbo::Executar));
    // e executa como qualquer texto
    executar(&mut h, None, true);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas"]);
    assert!(h.query_by_label(tela_consulta::NADA_EXECUTADO).is_none());
}

#[test]
fn capturas_da_consulta_vazia() {
    for (tema, nome) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let amb = Ambiente::exemplo(&format!("consulta_vazia_captura_{nome}"));
        let mut h = abrir(&amb, None, tema);
        capturar_sem_hover(&mut h, &format!("consulta-vazia-{nome}.png"));
    }
}

// ------------------------------------------------------------------ atalhos, seleção, endereço, busca

#[test]
fn ctrl_enter_e_tab() {
    let amb = Ambiente::exemplo("consulta_atalhos");
    let mut h = abrir(&amb, None, Tema::Claro);
    escrever(&mut h, "SELECT count(*) AS n FROM clientes");
    h.key_press(Key::Home);
    h.run_steps(1);
    h.key_press(Key::Tab);
    h.run_steps(2);
    assert_eq!(tela(&h).texto(), "  SELECT count(*) AS n FROM clientes");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    h.run_steps(2);
    assert_eq!(tela(&h).texto(), "SELECT count(*) AS n FROM clientes");
    executar(&mut h, None, true);
    let (colunas, linhas) = tela(&h).grade(0).expect("grade");
    assert_eq!((colunas, inteiro(&linhas[0][0])), (vec!["n"], 60));
}

#[test]
fn executa_so_a_selecao() {
    let amb = Ambiente::exemplo("consulta_selecao");
    let mut h = abrir(&amb, None, Tema::Claro);
    let texto = "SELECT 1 AS primeiro;\nSELECT 2 AS segundo;\nSELECT 3 AS terceiro;";
    escrever(&mut h, texto);
    let inicio = texto.find("SELECT 2").unwrap_or_default();
    let fim = texto.find("\nSELECT 3").unwrap_or_default();
    let id = tela(&h).id_do_editor();
    selecionar(&mut h, id, inicio, fim);
    assert_eq!(selecao(&h, id), Some((inicio, fim)));
    executar(&mut h, None, true);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas"]);
    let (colunas, linhas) = tela(&h).grade(0).expect("grade");
    assert_eq!((colunas, inteiro(&linhas[0][0])), (vec!["segundo"], 2));
    // o editor não perde o resto do texto
    assert_eq!(tela(&h).texto(), texto);
}

#[test]
fn sql_do_endereco_preenche_e_o_texto_vai_para_o_endereco() {
    let amb = Ambiente::exemplo("consulta_endereco");
    let mut h = abrir(&amb, Some("SELECT 42 AS resposta"), Tema::Claro);
    assert_eq!(tela(&h).texto(), "SELECT 42 AS resposta");
    executar(&mut h, None, false);
    assert_eq!(tela(&h).grade(0).map(|(_, l)| inteiro(&l[0][0])), Some(42));
    assert!(h.state().acoes.contains(&Acao::Substituir(Rota::consulta_com("SELECT 42 AS resposta"))));
    escrever(&mut h, "SELECT 'olá' AS saudacao");
    // 400 ms depois de parar de digitar (cada passo do harness anda 250 ms)
    h.run_steps(3);
    assert_eq!(h.state().acoes.last(), Some(&Acao::Substituir(Rota::consulta_com("SELECT 'olá' AS saudacao"))));
}

/// A busca da barra ("Buscar no editor"): cada letra acha a primeira ocorrência a partir da achada, sem caixa nem
/// acento, e o Enter vai à seguinte (com volta). O trecho achado fica pintado no editor (o foco está na busca, e o
/// egui reduz a seleção do editor sem o foco ao cursor), e o cursor vai para o fim dele.
#[test]
fn buscar_no_editor_seleciona_a_proxima() {
    let amb = Ambiente::exemplo("consulta_busca_da_barra");
    let sql = "SELECT nome FROM clientes WHERE cidade = 'São Paulo';\nselect * from Clientes;";
    let mut h = abrir(&amb, Some(sql), Tema::Claro);
    let id = tela(&h).id_do_editor();
    let campo = h.get_by_label("Buscar no editor");
    campo.focus();
    campo.type_text("CLIENTES");
    h.run_steps(3);
    assert_eq!(tela(&h).achado(), Some(17..25), "a primeira, sem caixa");
    assert_eq!(selecao(&h, id).map(|(_, b)| b), Some(25), "o cursor vai para o fim dela");
    assert!(h.query_by_label("Linha 1, coluna 26").is_some());
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert_eq!(tela(&h).achado(), Some(68..76), "o Enter vai à seguinte");
    assert!(h.query_by_label("Linha 2, coluna 23").is_some());
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert_eq!(tela(&h).achado(), Some(17..25), "e volta ao começo");
    assert!(h.get_by_label("Buscar no editor").is_focused(), "o foco fica na busca");
    assert_eq!(tela(&h).texto(), sql, "a busca não mexe no texto");
    capturar_sem_hover(&mut h, "consulta-busca-claro.png");
    // sem ocorrência, nada fica pintado; o Esc limpa
    let campo = h.get_by_label("Buscar no editor");
    campo.focus();
    campo.type_text("x");
    h.run_steps(3);
    assert_eq!(tela(&h).achado(), None);
}

/// Um editor longo: o trecho achado lá embaixo vem para a vista.
#[test]
fn busca_rola_o_editor_ate_o_achado() {
    let amb = Ambiente::exemplo("consulta_busca_rola");
    let mut sql = "SELECT 1;\n".repeat(60);
    sql.push_str("SELECT 'agulha';");
    let mut h = abrir(&amb, Some(&sql), Tema::Claro);
    // o cursor nasce no fim: volta ao começo, com o editor rolado para o alto
    let id = tela(&h).id_do_editor();
    selecionar(&mut h, id, 0, 0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Home);
    h.run_steps(3);
    assert!(h.query_by_label("Linha 1, coluna 1").is_some());
    let campo = h.get_by_label("Buscar no editor");
    campo.focus();
    campo.type_text("agulha");
    h.run_steps(4);
    assert_eq!(tela(&h).achado(), Some(608..614));
    // a última linha (61) aparece na faixa, e o editor rolou até ela (a captura mostra o trecho pintado)
    assert!(h.query_by_label("Linha 61, coluna 15").is_some());
    capturar_sem_hover(&mut h, "consulta-busca-rola-claro.png");
}

// ------------------------------------------------------------------ transação

#[test]
fn transacao_na_faixa_com_commit_e_rollback() {
    let mut amb = Ambiente::exemplo("consulta_rollback");
    let original = texto_da_celula(&ler(&amb, "SELECT cidade FROM clientes WHERE id = 1")[0][0]);
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some("BEGIN; UPDATE clientes SET cidade = 'Cidade de Teste' WHERE id = 1;"), false);
    assert!(h.query_by_label("Há uma transação aberta. As alterações só gravam no arquivo com COMMIT.").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Descartar (ROLLBACK)").is_some());
    assert!(h.state().acoes.contains(&Acao::NotarTransacao(true)));
    // a faixa da transação fica antes da faixa do banco
    let aviso = h.get_by_role_and_label(Role::Button, "Gravar (COMMIT)").rect();
    assert!(aviso.bottom() < h.get_by_label("exemplo.db").rect().top(), "{aviso:?}");
    // a Execução diz que há transação
    assert!(h.query_by_label("aberta").is_some());
    // nada foi para o arquivo ainda
    assert_eq!(texto_da_celula(&ler(&amb, "SELECT cidade FROM clientes WHERE id = 1")[0][0]), original);

    // a transação continua aberta ao sair e voltar à tela (a casca releu o banco)
    casca_rele(&mut h, &mut amb);
    drop(h);
    let mut h = abrir(&amb, None, Tema::Claro);
    assert!(h.query_by_role_and_label(Role::Button, "Descartar (ROLLBACK)").is_some());

    // a confirmação é a janela do kit
    let antes = execucoes(&h);
    h.get_by_role_and_label(Role::Button, "Descartar (ROLLBACK)").click();
    esperar_texto(&mut h, "Descartar todas as alterações feitas desde o BEGIN?");
    assert!(h.query_by_role_and_label(Role::Dialog, "Descartar a transação").is_some());
    h.get_by_role_and_label(Role::Button, "Descartar").click();
    esperar(&mut h, move |h| h.state().tela.doc.execucoes() > antes);
    assert!(h.query_by_role_and_label(Role::Button, "Descartar (ROLLBACK)").is_none());
    assert!(h.query_by_label("Transação descartada. As alterações feitas desde o BEGIN foram desfeitas.").is_some());
    assert_eq!(h.state().acoes.last(), Some(&Acao::RecarregarObjetos));
    assert_eq!(texto_da_celula(&ler(&amb, "SELECT cidade FROM clientes WHERE id = 1")[0][0]), original);
}

#[test]
fn transacao_com_commit_grava() {
    let amb = Ambiente::exemplo("consulta_commit");
    let mut h = abrir(&amb, None, Tema::Claro);
    executar(&mut h, Some("BEGIN;\nDELETE FROM eventos WHERE tipo = 'erro';"), false);
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
    assert_eq!(inteiro(&ler(&amb, "SELECT count(*) FROM eventos WHERE tipo = 'erro'")[0][0]), 1250);
    capturar_sem_hover(&mut h, "consulta-transacao-aberta-claro.png");
    let antes = execucoes(&h);
    h.get_by_role_and_label(Role::Button, "Gravar (COMMIT)").click();
    esperar(&mut h, move |h| h.state().tela.doc.execucoes() > antes);
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_none());
    assert!(h.query_by_label("Transação gravada. As alterações feitas desde o BEGIN estão no arquivo.").is_some());
    assert_eq!(inteiro(&ler(&amb, "SELECT count(*) FROM eventos WHERE tipo = 'erro'")[0][0]), 0);
    assert!(h.state().acoes.contains(&Acao::NotarTransacao(false)));
    assert_eq!(h.state().acoes.last(), Some(&Acao::RecarregarObjetos));
}

#[test]
fn transacao_que_o_estado_nao_viu_aparece_pela_sondagem() {
    let mut amb = Ambiente::exemplo("consulta_sondagem");
    // um BEGIN cuja resposta a casca não viu: o estado diz que não há transação aberta
    let id = amb.estado.banco_id();
    amb.servico.sql(&id, "BEGIN; UPDATE clientes SET cidade = 'Sondagem' WHERE id = 1;", 10).expect("begin");
    assert!(!amb.estado.transacao_aberta());
    let mut h = abrir(&amb, None, Tema::Claro);
    esperar_texto(&mut h, "Há uma transação aberta.");
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
    assert!(h.state().acoes.contains(&Acao::NotarTransacao(true)), "a casca fica sabendo");
    assert_eq!(execucoes(&h), 0, "a sondagem não vira resultado");
    drop(h);

    // o contrário: o estado acha que há transação, mas ela já acabou (um COMMIT cuja resposta se perdeu)
    amb.servico.sql(&id, "COMMIT", 10).expect("commit");
    if let Some(b) = amb.estado.banco.as_mut() {
        b.transacao_aberta = true;
    }
    let mut h = abrir(&amb, None, Tema::Claro);
    esperar(&mut h, |h| h.state().acoes.contains(&Acao::NotarTransacao(false)));
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_none());
    assert!(h.query_by_label_contains("Há uma transação aberta.").is_none());
}

#[test]
fn sair_no_meio_da_execucao_nao_perde_a_transacao() {
    let amb = Ambiente::exemplo("consulta_sair_no_meio");
    let roteiro = format!("BEGIN; UPDATE clientes SET cidade = 'Lenta' WHERE id = 1; {LENTA}");

    // a casca troca de tela no meio: pega a execução que ainda roda e espera a resposta dela
    let mut h = abrir(&amb, Some(&roteiro), Tema::Claro);
    h.get_by_role_and_label(Role::Button, "Executar").click();
    h.run_steps(1);
    assert!(tela(&h).executando());
    let promessa = h.state_mut().tela.doc.soltar_execucao();
    assert!(!tela(&h).executando());
    drop(h);
    let resposta = esperar_promessa(&mut promessa.expect("a execução em andamento")).expect("a execução");
    assert!(resposta.transacao_aberta);
    amb.servico.sql(&amb.estado.banco_id(), "ROLLBACK", 10).expect("rollback");

    // sem a casca pegar a resposta: ao voltar à Consulta (o estado ainda diz que não há transação), a sondagem
    // espera a execução que ainda roda e mostra a faixa
    let mut h = abrir(&amb, Some(&roteiro), Tema::Claro);
    h.get_by_role_and_label(Role::Button, "Executar").click();
    h.run_steps(1);
    assert!(tela(&h).executando());
    drop(h);
    assert!(!amb.estado.transacao_aberta());
    let mut h = abrir(&amb, None, Tema::Claro);
    esperar_texto(&mut h, "Há uma transação aberta.");
    assert!(h.state().acoes.contains(&Acao::NotarTransacao(true)));
}

// ------------------------------------------------------------------ várias Consultas

#[test]
fn duas_consultas_com_ids_proprios_nao_dividem_texto_cursor_nem_desfazer() {
    let amb = Ambiente::exemplo("consulta_duas_ids");
    let telas = [TelaConsulta::nova(None).com_id(1), TelaConsulta::nova(None).com_id(2)];
    let id_1 = telas[0].id_do_editor();
    assert_ne!(id_1, telas[1].id_do_editor());
    assert_eq!(TelaConsulta::nova(None).id_do_editor(), TelaConsulta::id_editor(), "sem o sal, o id de sempre");
    let mut h = abrir_duas(&amb, telas);

    // o cursor guardado no estado do editor `id` (o egui reduz a seleção ao cursor quando desenha o editor sem o
    // foco, e o cursor fica)
    let cursor = |h: &H2, id| {
        egui::text_edit::TextEditState::load(&h.ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0)
    };

    // na 1: um texto que entra no desfazer (o egui guarda o estado parado há 1 s; cada passo anda 250 ms) e uma
    // seleção que termina no caractere 8
    escrever(&mut h, "SELECT 1 AS um");
    h.run_steps(6);
    selecionar(&mut h, id_1, 0, 8);
    assert_eq!(selecao(&h, id_1), Some((0, 8)));

    // a 2 nasce com o editor dela: vazio, sem seleção e com o foco
    mostrar_aba(&mut h, 1);
    let id_2 = h.state().tela.doc.telas[1].id_do_editor();
    assert_eq!(h.state().tela.doc.telas[1].texto(), "");
    assert!(selecao(&h, id_2).is_none_or(|(a, b)| a == b));
    assert!(h.get_by_label("Comando SQL").is_focused());
    escrever(&mut h, "SELECT 2 AS dois");
    h.run_steps(6);

    // o Ctrl+Z da 2 volta ao texto dela (vazio), nunca ao da 1
    for _ in 0..3 {
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(2);
        let texto = h.state().tela.doc.telas[1].texto();
        assert!(!texto.contains("AS um"), "o desfazer da 2 trouxe o texto da 1: {texto:?}");
    }
    assert_eq!(h.state().tela.doc.telas[1].texto(), "");

    // a 1 continua com o texto e o cursor dela: o da 2 não passou para lá
    mostrar_aba(&mut h, 0);
    assert_eq!(h.state().tela.doc.telas[0].texto(), "SELECT 1 AS um");
    assert_eq!(cursor(&h, id_1), Some(8), "o cursor da 1 mudou com a 2");
}

#[test]
fn receber_em_segundo_plano_entrega_e_avisa_a_transacao() {
    let amb = Ambiente::exemplo("consulta_segundo_plano");
    let roteiro = format!("BEGIN; UPDATE clientes SET cidade = 'Fundo' WHERE id = 1; {LENTA}");
    let telas = [TelaConsulta::nova(None).com_id(1), TelaConsulta::nova(Some(roteiro)).com_id(2)];
    let mut h = abrir_duas(&amb, telas);

    // a 2 aparece, começa a execução e sai de vista no meio dela; a 1, já pronta, volta à vista
    mostrar_aba(&mut h, 1);
    h.get_by_role_and_label(Role::Button, "Executar").click();
    h.run_steps(1);
    assert!(h.state().tela.doc.telas[1].executando());
    h.state_mut().acoes.clear();
    mostrar_aba(&mut h, 0);

    // a resposta chega com a aba escondida: a casca fica sabendo da transação e relê os objetos na hora, e os
    // resultados ficam prontos
    esperar(&mut h, |h| !h.state().tela.doc.telas[1].executando());
    let acoes = &h.state().acoes;
    assert!(acoes.contains(&Acao::NotarTransacao(true)), "{acoes:?}");
    assert!(acoes.contains(&Acao::RecarregarObjetos), "{acoes:?}");
    assert_eq!(h.state().tela.doc.telas[1].execucoes(), 1);
    assert_eq!(h.state().tela.doc.telas[1].tipos_dos_cards(), ["alteracao", "alteracao", "linhas"]);
    assert!(h.query_by_label("Comando 3 · 1 linha").is_none(), "a aba escondida não desenha");

    // de volta à 2: os resultados e a faixa da transação
    mostrar_aba(&mut h, 1);
    assert!(h.query_by_label("Comando 3 · 1 linha").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
}

#[test]
fn esquecer_transacao_volta_ao_estado_da_casca() {
    let mut amb = Ambiente::exemplo("consulta_esquecer_transacao");
    let telas = [TelaConsulta::nova(None).com_id(1), TelaConsulta::nova(None).com_id(2)];
    let mut h = abrir_duas(&amb, telas);
    executar(&mut h, Some("BEGIN; UPDATE clientes SET cidade = 'Esquecer' WHERE id = 1;"), false);
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());

    // a 2 usa a mesma conexão (a sondagem dela vê a transação) e grava
    mostrar_aba(&mut h, 1);
    esperar(&mut h, |h| h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
    h.get_by_role_and_label(Role::Button, "Gravar (COMMIT)").click();
    esperar(&mut h, |h| h.state().tela.doc.telas[1].execucoes() > 0);
    assert!(h.query_by_label_contains("Transação gravada.").is_some());
    casca_rele(&mut h, &mut amb);
    assert!(!h.state().estado.transacao_aberta());

    // a 1 ainda segue a última resposta dela: sem a casca avisar, a faixa continua
    mostrar_aba(&mut h, 0);
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
    h.state_mut().tela.doc.telas[0].esquecer_transacao();
    h.run_steps(2);
    assert!(h.query_by_label_contains("Há uma transação aberta.").is_none());

    // daí em diante ela segue o estado da casca: um BEGIN feito fora dela traz a faixa de volta
    amb.servico.sql(&amb.estado.banco_id(), "BEGIN", 1).expect("begin");
    casca_rele(&mut h, &mut amb);
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
}

#[test]
fn executar_agora_executa_como_o_ctrl_enter() {
    let amb = Ambiente::exemplo("consulta_executar_agora");
    let texto = "SELECT 1 AS primeiro;\nSELECT 2 AS segundo;";
    let mut h = abrir(&amb, Some(texto), Tema::Claro);

    // sem seleção: o texto todo, que vai para o endereço
    h.state_mut().tela.doc.executar_agora();
    esperar(&mut h, |h| h.state().tela.doc.execucoes() == 1);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas", "linhas"]);
    assert!(h.state().acoes.contains(&Acao::Substituir(Rota::consulta_com(texto))));

    // com seleção no editor, só o trecho
    let id = tela(&h).id_do_editor();
    let inicio = texto.find("SELECT 2").unwrap_or_default();
    selecionar(&mut h, id, inicio, texto.chars().count());
    h.state_mut().tela.doc.executar_agora();
    esperar(&mut h, |h| h.state().tela.doc.execucoes() == 2);
    let (colunas, linhas) = tela(&h).grade(0).expect("grade");
    assert_eq!((colunas, inteiro(&linhas[0][0])), (vec!["segundo"], 2));
    assert_eq!(tela(&h).texto(), texto, "o editor não perde o resto do texto");

    // com a confirmação aberta o pedido se perde, como o Ctrl+Enter, e não volta quando ela fecha
    executar(&mut h, Some("BEGIN"), false);
    h.get_by_role_and_label(Role::Button, "Descartar (ROLLBACK)").click();
    esperar(&mut h, |h| h.query_by_role_and_label(Role::Dialog, "Descartar a transação").is_some());
    let antes = execucoes(&h);
    h.state_mut().tela.doc.executar_agora();
    h.run_steps(3);
    assert!(!tela(&h).executando());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_role_and_label(Role::Dialog, "Descartar a transação").is_none());
    assentar(&mut h);
    assert!(!tela(&h).executando());
    assert_eq!(execucoes(&h), antes);
}

// ------------------------------------------------------------------ interromper

/// Um SELECT que nunca termina sozinho (a CTE recursiva de 2 bilhões de linhas): só o Interromper o encerra.
const INFINITA: &str = "WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 2000000000) \
                        SELECT count(*) AS n FROM s;";

/// Põe `sql` no editor, aperta o Executar da barra e espera a execução ficar em andamento (os quadros do
/// harness só andam enquanto o teste manda).
fn comecar_a_executar(h: &mut H, amb: &Ambiente, sql: &str) {
    escrever(h, sql);
    h.get_by_role_and_label(Role::Button, "Executar").click();
    h.run_steps(2);
    assert!(tela(h).executando());
    // Um pedido de parada que chega antes de a execução pegar a conexão vale (C7): ela sairia interrompida sem rodar
    // nem o primeiro comando. Os testes que querem o começo do roteiro rodando esperam a execução pegar a conexão
    // (e um instante para os comandos curtos do começo).
    let id = amb.estado.banco_id();
    let inicio = Instant::now();
    while !amb.servico.sql_em_andamento(&id).unwrap_or(false) {
        assert!(inicio.elapsed() < Duration::from_secs(15), "a execução não pegou a conexão");
        h.run_steps(1);
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(150));
}

/// Interrompe (pelo botão, ou pelo Esc) até a execução acabar. Um pedido que chega antes de a execução existir no
/// banco não alcança nada (`Nenhuma`: o pedido não fica guardado); por isso o pedido se repete, como a pessoa
/// clicaria de novo.
fn interromper_ate_acabar(h: &mut H, com_esc: bool) {
    let inicio = Instant::now();
    while tela(h).executando() {
        assert!(inicio.elapsed() < Duration::from_secs(15), "a execução não foi interrompida");
        if com_esc {
            h.key_press(Key::Escape);
        } else if let Some(botao) = h.query_by_role_and_label(Role::Button, "Interromper") {
            botao.click();
        }
        h.run_steps(2);
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// N3 (prancheta 15): o Executar fica desligado com o arco, o Interromper aparece ao lado, o rodapé conta o tempo,
/// o Ctrl+Enter é ignorado, e o Interromper devolve o comando como interrompido.
#[test]
fn executando_mostra_o_tempo_e_interromper() {
    let amb = Ambiente::exemplo("consulta_executando");
    let mut h = abrir(&amb, None, Tema::Escuro);
    // antes de executar, nada de Interromper
    assert!(h.query_by_role_and_label(Role::Button, "Interromper").is_none());
    comecar_a_executar(&mut h, &amb, INFINITA);

    // o rodapé conta o tempo ao vivo ("Executando · 1,4 s")
    let contagem = rodape(&h, "Executando · ").expect("o rodapé executando");
    assert!(contagem.ends_with(" s"), "{contagem}");
    // o Executar desligado (a dica diz "Executando") e o Interromper à direita dele
    let executar = h.get_by_role_and_label(Role::Button, "Executar");
    assert!(executar.accesskit_node().is_disabled());
    let executar = executar.rect();
    let interromper = h.get_by_role_and_label(Role::Button, "Interromper").rect();
    assert!(interromper.left() > executar.right() - 1.0 && (interromper.center().y - executar.center().y).abs() < 1.0);
    assert_eq!(
        tela(&h).ofertas(&h.state().estado).dica(estado::Verbo::Interromper),
        "Interromper o comando em execução (Esc)"
    );
    assert_eq!(tela(&h).ofertas(&h.state().estado).dica(estado::Verbo::Executar), "Executando");
    capturar_sem_hover(&mut h, "consulta-executando-escuro.png");

    // o Ctrl+Enter é ignorado: a mesma execução segue e nada novo começa
    let antes = execucoes(&h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run_steps(3);
    assert!(tela(&h).executando());
    assert_eq!(execucoes(&h), antes);

    // o Interromper corta o comando; a faixa de erro diz o que houve e o rodapé deixa de contar
    interromper_ate_acabar(&mut h, false);
    assentar(&mut h);
    assert_eq!(execucoes(&h), antes + 1);
    assert_eq!(tela(&h).tipos_dos_cards(), ["erro"]);
    assert!(h.query_by_label_contains("Comando 1: A consulta foi interrompida.").is_some());
    assert!(rodape(&h, "Executando").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Interromper").is_none());
    assert!(!h.get_by_role_and_label(Role::Button, "Executar").accesskit_node().is_disabled());
    capturar_sem_hover(&mut h, "consulta-interrompida-escuro.png");
}

/// O comando interrompido aparece no seletor ("interrompido"), os anteriores ficam, o seguinte não roda e o texto
/// do SQLite em inglês não vai para a tela. O Esc também interrompe.
#[test]
fn interrompida_mostra_o_comando_interrompido() {
    let amb = Ambiente::exemplo("consulta_interrompida");
    let mut h = abrir(&amb, None, Tema::Claro);
    comecar_a_executar(&mut h, &amb, &format!("SELECT 1 AS um; {INFINITA} SELECT 3 AS tres;"));
    interromper_ate_acabar(&mut h, true);
    assentar(&mut h);
    assert_eq!(tela(&h).tipos_dos_cards(), ["linhas", "erro"]);
    let partes: Vec<String> = h.query_all_by_role(Role::Tab).map(|n| rotulo(&n)).collect();
    assert_eq!(partes, ["Comando 1 · 1 linha", "Comando 2 · interrompido"]);
    assert!(h.query_by_label_contains("Comando 2: A consulta foi interrompida.").is_some());
    assert!(h.query_by_label_contains("interrupted").is_none());
    assert_eq!(tela(&h).grade(0).map(|(c, _)| c), Some(vec!["um"]));
    // a Execução diz onde parou
    assert!(h.query_by_label("Comando 2").is_some(), "o Parou em");
    // a conexão segue boa: o próximo roteiro roda
    executar(&mut h, Some("SELECT 5 AS cinco"), false);
    assert_eq!(tela(&h).grade(0).map(|(_, l)| inteiro(&l[0][0])), Some(5));
}

/// Interromper um SELECT dentro de um BEGIN não desfaz a transação: a faixa segue a resposta.
#[test]
fn interromper_dentro_da_transacao_mantem_a_faixa() {
    let amb = Ambiente::exemplo("consulta_interrompida_transacao");
    let mut h = abrir(&amb, None, Tema::Claro);
    comecar_a_executar(&mut h, &amb, &format!("BEGIN; UPDATE clientes SET cidade = 'Parada' WHERE id = 1; {INFINITA}"));
    interromper_ate_acabar(&mut h, false);
    assentar(&mut h);
    // a faixa de erro e o resultado do comando dizem a frase
    assert!(h.query_all_by_label_contains("A consulta foi interrompida.").count() >= 1);
    assert_eq!(tela(&h).tipos_dos_cards(), ["alteracao", "alteracao", "erro"]);
    assert!(h.state().acoes.contains(&Acao::NotarTransacao(true)));
    assert!(h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_some());
    // limpa a transação para não vazar para outros testes do mesmo arquivo
    h.get_by_role_and_label(Role::Button, "Gravar (COMMIT)").click();
    esperar(&mut h, |h| h.query_by_role_and_label(Role::Button, "Gravar (COMMIT)").is_none());
}

// ------------------------------------------------------------------ editor

#[test]
fn editor_muda_de_altura_pela_borda_de_baixo() {
    let amb = Ambiente::exemplo("consulta_alca");
    let mut h = abrir(&amb, Some("SELECT 1;"), Tema::Claro);
    executar(&mut h, None, false);
    let area = tela(&h).area_editor();
    assert_eq!(area.height(), ALTURA_EDITOR);
    let seletor = h.get_by_label("Comando 1 · 1 linha").rect().top();
    // a borda de baixo desce 100px: o editor e o seletor acompanham
    let borda = egui::pos2(area.center().x, area.bottom());
    arrastar(&mut h, borda, borda + vec2(0.0, 100.0));
    let area = tela(&h).area_editor();
    assert!((area.height() - (ALTURA_EDITOR + 100.0)).abs() < 0.5, "{area:?}");
    let desceu = h.get_by_label("Comando 1 · 1 linha").rect().top() - seletor;
    assert!((desceu - 100.0).abs() < 0.5, "{desceu}");
    // para cima, não passa do mínimo de 96px
    let borda = egui::pos2(area.center().x, area.bottom());
    arrastar(&mut h, borda, borda - vec2(0.0, 600.0));
    assert_eq!(tela(&h).area_editor().height(), ALTURA_MIN_EDITOR);
    // para baixo, deixa o seletor e um pouco do resultado à vista
    let area = tela(&h).area_editor();
    let borda = egui::pos2(area.center().x, area.bottom());
    arrastar(&mut h, borda, borda + vec2(0.0, 2000.0));
    let r = h.get_by_label("Comando 1 · 1 linha").rect();
    assert!(r.bottom() < TELA.y - 34.0 - 40.0, "{r:?}");
    // arrastar a borda não mexe no texto
    assert_eq!(tela(&h).texto(), "SELECT 1;");
}

// ------------------------------------------------------------------ capturas

#[test]
fn capturas_da_prancheta_3() {
    for (tema, nome) in
        [(Tema::Escuro, "consulta-resultados-escuro.png"), (Tema::Claro, "consulta-resultados-claro.png")]
    {
        let amb = Ambiente::exemplo("consulta_captura");
        let mut h = abrir(&amb, None, tema);
        executar(&mut h, Some(ROTEIRO), false);
        assert_eq!(tela(&h).tipos_dos_cards(), ["linhas", "alteracao", "erro"]);
        capturar_sem_hover(&mut h, nome);
    }
    // a 960, o seletor e a grade cabem na largura que sobra
    let amb = Ambiente::exemplo("consulta_captura_960");
    let mut h = abrir_em(&amb, None, Tema::Claro, vec2(960.0, 900.0));
    executar(&mut h, Some(ROTEIRO), false);
    let erro = h.get_by_label("Comando 3 · erro").rect();
    assert!(erro.right() <= 960.0, "{erro:?}");
    capturar_sem_hover(&mut h, "consulta-resultados-960-claro.png");
}

#[test]
fn banco_sem_tabelas_mostra_o_aviso_nas_propriedades() {
    let mut amb = Ambiente::vazio("consulta_sem_tabelas");
    let caminho = amb.pasta().join("vazio.db");
    amb.servico.novo_banco(&caminho, false).expect("criar o banco vazio");
    amb.recarregar();
    let mut h = abrir(&amb, None, Tema::Claro);
    assert!(h.query_by_label("O banco ainda não tem tabelas; crie a primeira com CREATE TABLE.").is_some());
    capturar_sem_hover(&mut h, "consulta-sem-tabelas-claro.png");
}

// ------------------------------------------------------------------ o menu da célula (0.2.3)

/// O centro do texto pintado `texto` na grade (o primeiro).
fn centro_do_texto(h: &H, texto: &str) -> Pos2 {
    h.output()
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == texto => Some(t.pos + vec2(t.galley.size().x / 2.0, 7.0)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{texto} não foi pintado"))
}

fn botao_em(h: &mut H, pos: Pos2, button: PointerButton) {
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

fn menu_aberto(h: &H) -> Option<String> {
    h.query_by_role(Role::Menu).and_then(|m| m.accesskit_node().label())
}

/// Escolhe um item do menu (no submenu "Copiar como", passa por ele antes) e devolve o que foi copiado.
fn escolher_no_menu(h: &mut H, item: &str, submenu: bool) -> Option<String> {
    if submenu {
        h.get_by_label("Copiar como").hover();
        h.run_steps(3);
    }
    h.get_by_label(item).click();
    let mut copiado = None;
    for _ in 0..4 {
        h.step();
        copiado = copiado.or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
        });
    }
    copiado
}

/// Executa o SELECT dos testes do menu e seleciona `linhas` × 2 a partir do nome da primeira linha.
fn selecionar_nome_e_limite(h: &mut H, linhas: usize) -> Vec<Vec<comum::Celula>> {
    executar(h, Some("SELECT nome, limite FROM clientes ORDER BY id LIMIT 8"), false);
    assentar(h);
    let dados = tela(h).grade(0).expect("a grade").1.to_vec();
    let pos = centro_do_texto(h, &texto_da_celula(&dados[0][0]));
    botao_em(h, pos, PointerButton::Primary);
    for _ in 1..linhas {
        h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
        h.run_steps(1);
    }
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    h.run_steps(2);
    dados
}

#[test]
fn menu_da_celula_copia_e_o_rodape_soma() {
    let amb = Ambiente::exemplo("consulta_menu_da_celula");
    let mut h = abrir(&amb, None, Tema::Claro);
    let dados = selecionar_nome_e_limite(&mut h, 3);
    let mut resumo = interface::grade::ResumoSelecao::default();
    for linha in &dados[..3] {
        for c in linha {
            resumo.somar_celula(c, true);
        }
    }
    assert!(resumo.texto().starts_with("Contagem 6 · Soma "), "{}", resumo.texto());
    assert_eq!(rodape(&h, "Contagem "), Some(resumo.texto()), "o valor decide o que é número");
    assert!(rodape(&h, "8 linhas · ").is_some(), "o rodapé do resultado continua à esquerda");

    // dentro do intervalo: só os itens de copiar
    let pos = centro_do_texto(&h, &texto_da_celula(&dados[1][0]));
    botao_em(&mut h, pos, PointerButton::Secondary);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da seleção"));
    for item in ["Colar", "Limpar", "Filtrar por este valor", "Marcar as linhas", "Excluir as linhas"] {
        assert!(h.query_by_label(item).is_none(), "a Consulta não tem {item}");
    }
    let insert = escolher_no_menu(&mut h, "INSERT", true).expect("copiou");
    let primeiro = texto_da_celula(&dados[0][0]).replace('\'', "''");
    assert!(
        insert.starts_with(&format!("INSERT INTO \"clientes\" (\"nome\", \"limite\") VALUES\n  ('{primeiro}', ")),
        "a tabela do FROM: {insert}"
    );
    assert_eq!(insert.matches("\n  (").count(), 3);
    assert!(menu_aberto(&h).is_none());

    // Copiar com cabeçalho
    botao_em(&mut h, pos, PointerButton::Secondary);
    let tsv = escolher_no_menu(&mut h, "Copiar com cabeçalho", false).expect("copiou");
    assert!(tsv.starts_with(&format!("nome\tlimite\r\n{}\t", texto_da_celula(&dados[0][0]))), "{tsv}");

    // o Shift+F10 abre na ativa, com o foco no primeiro item; o Esc fecha e devolve o foco à grade
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(3);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da seleção"));
    assert!(h.get_by_label("Copiar").accesskit_node().is_focused());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(menu_aberto(&h).is_none());

    // fora do intervalo: a célula vira a ativa, e uma célula só tira as contas do rodapé
    let fora = centro_do_texto(&h, &texto_da_celula(&dados[6][0]));
    botao_em(&mut h, fora, PointerButton::Secondary);
    assert_eq!(menu_aberto(&h).as_deref(), Some("Menu da célula"));
    let copiado = escolher_no_menu(&mut h, "Copiar", false);
    assert_eq!(copiado, Some(texto_da_celula(&dados[6][0])));
    assert_eq!(rodape(&h, "Contagem "), None);
}

#[test]
fn capturas_do_menu_da_celula() {
    let amb = Ambiente::exemplo("consulta_menu_capturas");
    for (tema, nome) in
        [(Tema::Claro, "consulta-menu-da-celula-claro.png"), (Tema::Escuro, "consulta-menu-da-celula-escuro.png")]
    {
        let mut h = abrir(&amb, None, tema);
        let dados = selecionar_nome_e_limite(&mut h, 4);
        let pos = centro_do_texto(&h, &texto_da_celula(&dados[1][0]));
        botao_em(&mut h, pos, PointerButton::Secondary);
        h.get_by_label("Copiar como").hover();
        h.run_steps(4);
        capturar(&mut h, nome);
    }
}
