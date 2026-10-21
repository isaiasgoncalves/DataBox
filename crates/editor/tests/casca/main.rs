//! A casca inteira no `egui_kittest`: Início, abrir por caminho digitado e pelos recentes, a barra de título (os botões da janela, o
//! arrasto, o duplo clique, as bordas, o título) e os menus Arquivo (com os Abertos recentemente), Editar, Exibir
//! (com o tema) e Ajuda, a barra de ferramentas (ordem fixa, verbos de cada aba, F5), as Propriedades, a barra de ícones, o
//! Explorador, o Histórico de consultas e a barra de status, navegação, voltar/avançar,
//! transação aberta, fechar banco, banco trocado por outro lugar, as abas de documento (várias partes e Consultas,
//! fechar, Ctrl+Tab, o histórico único, a transação entre as Consultas, as tabelas que mudam ou somem) e as capturas
//! em `capturas/rust/`, com os nomes das de referência.
//!
//! Um alvo só (`--test casca`), com um módulo por área: [`abrir`] (o Início, os diálogos, o caminho, as trocas de
//! banco), [`visao_geral`], [`menus`], [`sobre`], [`barra_de_titulo`], [`explorador`] (com o Histórico, a barra de
//! status e a navegação), [`abas`], [`transacao`], [`ferramentas`] (com as Propriedades), [`janela_estreita`],
//! [`cenas`] (as das pranchetas), [`revisoes`], [`busca`], [`icones`], [`preferencias`] e [`acessibilidade`]. O apoio
//! que mais de uma área usa fica aqui; o comum a todos os testes da casca, em `tests/apoio`.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod abas;
mod abrir;
mod acessibilidade;
#[path = "../apoio/mod.rs"]
mod apoio;
mod barra_de_titulo;
mod busca;
mod cenas;
mod explorador;
mod ferramentas;
mod icones;
mod janela_estreita;
mod menus;
mod preferencias;
mod revisoes;
mod sobre;
mod transacao;
mod visao_geral;

use std::path::PathBuf;

use apoio::*;
use editor::{Partida, PedidoDialogo, menu_aberto};
use egui::accesskit::Role;
use egui::{Key, Modifiers, PointerButton, ViewportCommand, ViewportId};
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::Ambiente;
use estado::{Aba, Rota};
use interface::pecas::toast::{toast, toast_atual};
use interface::tema::Tema;

const TABELAS: [&str; 7] = ["clientes", "eventos", "itens_pedido", "notas", "pedidos", "produtos", "Tabela com espaço"];

fn comandos(h: &H) -> Vec<ViewportCommand> {
    h.output().viewport_output.get(&ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default()
}

/// Manda os eventos, um por quadro a 10ms um do outro (o passo do harness é de 0,25 s, e o duplo clique tem de
/// caber em 0,3 s), e junta os comandos da janela de todos esses quadros e de mais três, no passo normal (0,75 s
/// depois, um clique seguinte não forma um duplo nem um triplo com estes).
fn comandos_dos_eventos(h: &mut H, eventos: &[egui::Event]) -> Vec<ViewportCommand> {
    let mut todos = Vec::new();
    let inicio = h.ctx.input(|i| i.time);
    for (n, e) in eventos.iter().enumerate() {
        h.input_mut().time = Some(inicio + 0.01 * (n + 1) as f64);
        h.event(e.clone());
        h.step();
        todos.extend(comandos(h));
    }
    h.input_mut().time = None;
    for _ in 0..3 {
        h.step();
        todos.extend(comandos(h));
    }
    todos
}

fn mover(pos: egui::Pos2) -> egui::Event {
    egui::Event::PointerMoved(pos)
}

fn apertar(pos: egui::Pos2, apertado: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed: apertado, modifiers: Modifiers::NONE }
}

/// Um clique com o botão principal em `pos`, com os comandos da janela que ele mandou.
fn clicar_em(h: &mut H, pos: egui::Pos2) -> Vec<ViewportCommand> {
    comandos_dos_eventos(h, &[mover(pos), apertar(pos, true), apertar(pos, false)])
}

/// O clique no meio do nó com este papel e rótulo, com os comandos da janela.
fn clicar_no(h: &mut H, papel: Role, rotulo: &str) -> Vec<ViewportCommand> {
    let centro = rect_de(h, papel, rotulo).center();
    clicar_em(h, centro)
}

/// O título com este texto na barra de título (os 30px de cima), se estiver lá.
fn titulo_na_barra(h: &H, texto: &str) -> Option<egui::Rect> {
    h.query_all(by().role(Role::Label).label(texto)).map(|n| n.rect()).find(|r| r.bottom() <= 30.0)
}

/// Lê um texto do banco por um serviço à parte (só leitura).
fn ler_texto(caminho: &std::path::Path, sql: &str) -> String {
    let pasta = caminho.parent().expect("pasta").join(format!("leitura-{}", std::process::id()));
    std::fs::create_dir_all(&pasta).expect("pasta");
    let s = servico::Servico::novo(prefs::Preferencias::abrir(Some(pasta.join("p.json"))));
    let info = s.abrir_banco(caminho, true, false).expect("abrir só leitura");
    let r = s.sql(&info.id, sql, 10).expect("consulta");
    let texto = match r.resultados.first() {
        Some(banco::ResultadoComando::Linhas { linhas, .. }) => {
            linhas.first().and_then(|l| l.first()).map(|c| format!("{c:?}")).unwrap_or_default()
        }
        outro => panic!("{outro:?}"),
    };
    let _ = s.fechar_banco(&info.id, true);
    let _ = std::fs::remove_dir_all(&pasta);
    texto
}

/// O campo do caminho do Início (pasta, banco ou arquivo de dados).
const CAMPO_DO_INICIO: &str = "Caminho da pasta ou do arquivo";

fn s(textos: &[&str]) -> Vec<String> {
    textos.iter().map(|t| t.to_string()).collect()
}

/// Executa o texto do editor da Consulta à vista (Ctrl+Enter) e espera a resposta.
fn executar(h: &mut H, sql: &str) {
    let campo = h.get_by_label("Comando SQL");
    campo.focus();
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::A);
    h.run_steps(1);
    h.get_by_label("Comando SQL").type_text(sql);
    h.run_steps(1);
    h.key_press_modifiers(CTRL, Key::Enter);
    h.run_steps(2);
    assentar(h);
}

/// Onde ficam as ações dos recentes com este rótulo ("Remover " … " da lista", com o nome do arquivo no meio).
fn acoes_dos_recentes(h: &H, comeco: &str, fim: &str) -> Vec<egui::Rect> {
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with(comeco) && l.ends_with(fim))).map(|n| n.rect()).collect()
}

// As capturas do fechamento da etapa 2 montam, com o banco de exemplo, as cenas das pranchetas do canvas, para a
// comparação a olho: o Início com os recentes e o menu Arquivo (prancheta 4), a Visão geral com três abas e o
// Exibir › Tema (1), a aba de Dados com o filtro (2), a Consulta com o Histórico (3), a Estrutura com a transação
// aberta (6) e a janela de 960px. O que as pranchetas têm e a etapa 2 não (a pasta aberta no Explorador, os
// ícones rasterizados, as telas no formato de documento) fica como está. Com os substitutos (sem o recurso
// `telas`), as cenas não fazem sentido e não sobrescrevem as capturas.

/// Passa o ponteiro sobre o canto do banco no Explorador onde fica o ícone de estado (16px, a 10px da direita do nó) e
/// devolve a dica que apareceu; tira o ponteiro no fim.
fn dica_do_estado_do_banco(h: &mut H) -> String {
    h.remove_cursor();
    h.run_steps(6);
    let antes = textos(h);
    let no = rect_de(h, Role::Link, "exemplo.db");
    h.hover_at(egui::pos2(no.right() - 10.0 - 8.0, no.center().y));
    h.run_steps(3);
    let mut depois = textos(h);
    for t in &antes {
        if let Some(i) = depois.iter().position(|d| d == t) {
            depois.remove(i);
        }
    }
    h.remove_cursor();
    h.run_steps(2);
    depois.join("\n")
}

/// Outro programa com o banco preso para gravação (`BEGIN IMMEDIATE` numa conexão à parte): os comandos que gravam
/// esperam por ele (até os 5 s do busy_timeout) até [`OutroPrograma::soltar`]. Deixa uma Consulta executando pelo
/// tempo que o teste quiser, sem depender da velocidade da máquina.
struct OutroPrograma {
    servico: servico::Servico,
    id: String,
    pasta: PathBuf,
}

impl OutroPrograma {
    fn prender(caminho: &std::path::Path) -> OutroPrograma {
        let pasta = caminho.parent().expect("pasta").join(format!("outro-programa-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let servico = servico::Servico::novo(prefs::Preferencias::abrir(Some(pasta.join("p.json"))));
        let id = servico.abrir_banco(caminho, false, false).expect("abrir").id;
        servico.sql(&id, "BEGIN IMMEDIATE;", 10).expect("prender o banco");
        OutroPrograma { servico, id, pasta }
    }

    /// Solta o banco: o comando que esperava segue.
    fn soltar(self) {}
}

impl Drop for OutroPrograma {
    fn drop(&mut self) {
        let _ = self.servico.sql(&self.id, "ROLLBACK;", 10);
        let _ = self.servico.fechar_banco(&self.id, true);
        let _ = std::fs::remove_dir_all(&self.pasta);
    }
}

/// Digita o comando na Consulta à vista e executa: ele fica esperando o [`OutroPrograma`] soltar o banco.
fn executando(h: &mut H, sql: &str) {
    digitar(h, "Comando SQL", sql);
    h.key_press_modifiers(CTRL, Key::Enter);
    h.run_steps(2);
    // o rodapé da Consulta conta o tempo
    esperar_texto(h, "Executando");
}

/// O campo da busca da barra pelo rótulo (a dica da aba: "Buscar tabelas", "Buscar em clientes"; "Buscar" desligada).
fn campo_da_busca<'h>(h: &'h H, rotulo: &'h str) -> egui_kittest::Node<'h> {
    h.get(by().role(Role::TextInput).label(rotulo))
}

/// Onde o PNG do ícone de `lado` pontos foi pintado dentro de `dentro` no último quadro (a malha com a textura dele).
fn icone_pintado_em(h: &H, icone: interface::icones::Icone, lado: f32, dentro: egui::Rect) -> Option<egui::Rect> {
    fn juntar(forma: &egui::Shape, textura: egui::TextureId, achados: &mut Vec<egui::Rect>) {
        match forma {
            egui::Shape::Vec(formas) => formas.iter().for_each(|f| juntar(f, textura, achados)),
            egui::Shape::Mesh(m) if m.texture_id == textura => achados.push(m.calc_bounds()),
            _ => {}
        }
    }
    let textura = interface::icones::textura_carregada(&h.ctx, icone, lado)?;
    let mut achados = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, textura, &mut achados);
    }
    achados.into_iter().find(|r| dentro.expand(0.5).contains_rect(*r))
}

/// Arquivo › Preferências.
fn abrir_preferencias(h: &mut H) {
    abrir_menu_arquivo(h);
    clicar_item(h, "Preferências");
    h.run_steps(3);
    // as famílias de fonte do código chegam de uma tarefa (a thread da interface não toca no disco)
    assentar(h);
}

fn preferencias_abertas(h: &H) -> bool {
    h.state().preferencias_aberta().is_some()
}

/// A frase do aviso de transação aberta e da dica do modo na barra de status, com o nome da Consulta.
fn frase_da_transacao(consulta: &str) -> String {
    format!(
        "Há uma transação aberta na {consulta}. O que for salvo agora só vai para o arquivo depois do COMMIT; fechar o \
         banco descarta tudo desde o BEGIN."
    )
}

/// Fecha a janela aberta com o Esc.
fn fechar_janela(h: &mut H) {
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
}
