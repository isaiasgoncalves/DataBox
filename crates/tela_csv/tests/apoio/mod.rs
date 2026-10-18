//! Apoio dos testes do editor de CSV: o CSV da prancheta 7, o palco do documento (como a casca o desenha), esperar a
//! tela ficar ociosa e as capturas. Criar, renderizar e desfazer um palco passam por uma vez só ([`vez`]): vários
//! wgpu ao mesmo tempo às vezes derrubam o processo dentro do driver (o mesmo cuidado do `tela_dados`).

#![allow(dead_code)]

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use estado::Documento;
use estado::apoio_testes::{Ambiente, NoPalco, Palco, palco_documento};
use interface::tema::Tema;
use tela_csv::{Ganchos, TelaCsv};

pub type H = Harness<'static, Palco<NoPalco<TelaCsv>>>;

static VEZ: Mutex<()> = Mutex::new(());

pub fn vez() -> MutexGuard<'static, ()> {
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// O palco de um teste: usado como o próprio harness, desfeito na sua vez.
pub struct Teste(Option<H>);

impl Deref for Teste {
    type Target = H;
    fn deref(&self) -> &H {
        self.0.as_ref().expect("palco desfeito")
    }
}

impl DerefMut for Teste {
    fn deref_mut(&mut self) -> &mut H {
        self.0.as_mut().expect("palco desfeito")
    }
}

impl Drop for Teste {
    fn drop(&mut self) {
        let _vez = vez();
        drop(self.0.take());
    }
}

pub const TAMANHO: egui::Vec2 = egui::vec2(1440.0, 900.0);

const REGIOES: [&str; 5] = ["Sul", "Sudeste", "Nordeste", "Norte", "Centro-Oeste"];
const VENDEDORES: [&str; 6] =
    ["Ana Coutinho", "Bruno Brandão", "Carla Ibiapina", "João Esteves", "Marina Lacerda", "Pedro Guimarães"];
const PRODUTOS: [&str; 6] =
    ["Caderno universitário", "Caneta gel", "Mochila escolar", "Agenda 2026", "Estojo duplo", "Marca-texto"];

/// A data `dias` depois de 2025-01-02 (sem biblioteca de datas: o calendário de 2025 e 2026 basta).
fn data(dias: usize) -> String {
    const MESES: [usize; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let (mut ano, mut mes, mut dia) = (2025, 0usize, 2 + dias);
    loop {
        let n = if mes == 1 && ano % 4 == 0 { 29 } else { MESES[mes] };
        if dia <= n {
            break;
        }
        dia -= n;
        mes += 1;
        if mes == 12 {
            mes = 0;
            ano += 1;
        }
    }
    format!("{ano}-{:02}-{dia:02}", mes + 1)
}

/// O `vendas_2025.csv` da prancheta 7: `linhas` linhas de data, regiao, vendedor, produto, quantidade e valor, com
/// `;` e a vírgula decimal (o gerador da prancheta), CRLF e UTF-8.
pub fn vendas(linhas: usize) -> String {
    let mut s = String::from("data;regiao;vendedor;produto;quantidade;valor\r\n");
    for i in 0..linhas {
        let q = ((i * 7) % 23) + 1;
        let v = format!("{:.2}", q as f64 * (12.9 + (i % 6) as f64 * 9.35)).replace('.', ",");
        s.push_str(&format!(
            "{};{};{};{};{q};{v}\r\n",
            data(i),
            REGIOES[i % 5],
            VENDEDORES[i % 6],
            PRODUTOS[(i * 5) % 6]
        ));
    }
    s
}

/// Grava `conteudo` em `nome` na pasta do teste.
pub fn arquivo(amb: &Ambiente, nome: &str, conteudo: impl AsRef<[u8]>) -> PathBuf {
    let caminho = amb.pasta().join(nome);
    std::fs::write(&caminho, conteudo).expect("gravar o CSV do teste");
    caminho
}

/// O palco com o documento de `caminho`, sem esperar nada.
pub fn palco(amb: &Ambiente, caminho: PathBuf, tema: Tema, tamanho: egui::Vec2, ganchos: Ganchos) -> Teste {
    let doc = TelaCsv::nova(caminho).com_ganchos(ganchos);
    let h = {
        let _vez = vez();
        palco_documento(amb, doc, tema, tamanho)
    };
    Teste(Some(h))
}

/// O documento de `caminho` já aberto e ocioso (1440×900, claro).
pub fn abrir(amb: &Ambiente, caminho: PathBuf) -> Teste {
    abrir_com(amb, caminho, Tema::Claro, TAMANHO)
}

pub fn abrir_com(amb: &Ambiente, caminho: PathBuf, tema: Tema, tamanho: egui::Vec2) -> Teste {
    let mut h = palco(amb, caminho, tema, tamanho, Ganchos::default());
    esperar_ociosa(&mut h);
    assert!(doc(&h).aberta(), "não abriu: {:?}", doc(&h).erro_ao_abrir());
    h
}

pub fn doc(h: &H) -> &TelaCsv {
    &h.state().tela.doc
}

pub fn doc_mut(h: &mut H) -> &mut TelaCsv {
    &mut h.state_mut().tela.doc
}

/// Roda quadros até o documento não ter tarefa nenhuma (e mais alguns, para a grade pedir a página e desenhar).
pub fn esperar_ociosa(h: &mut H) {
    let inicio = Instant::now();
    let mut quietos = 0;
    loop {
        h.step();
        if doc(h).ocupada() {
            quietos = 0;
        } else {
            quietos += 1;
            if quietos >= 3 {
                h.run_steps(2);
                if !doc(h).ocupada() {
                    return;
                }
            }
        }
        assert!(inicio.elapsed() < Duration::from_secs(20), "o documento não ficou ocioso em 20 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Clica num ponto (com os modificadores).
pub fn clicar_em(h: &mut H, pos: Pos2, modifiers: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(modifiers));
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers });
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers });
    h.step();
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.step();
    h.run_steps(2);
}

/// O item do menu de contexto com o rótulo (o botão de mesmo nome das Propriedades fica de fora: ele mora no painel
/// da direita).
pub fn item_do_menu<'a>(h: &'a H, rotulo: &'a str) -> egui_kittest::Node<'a> {
    use egui_kittest::kittest::Queryable;
    let nos: Vec<egui_kittest::Node<'a>> =
        h.query_all_by_label(rotulo).filter(|n| n.rect().left() < TAMANHO.x - 290.0).collect();
    nos.into_iter().last().unwrap_or_else(|| panic!("o item {rotulo} no menu"))
}

/// Clica com o botão direito num ponto.
pub fn clicar_direito_em(h: &mut H, pos: Pos2) {
    let m = Modifiers::NONE;
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.input_mut().events.push(Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: m,
    });
    h.input_mut().events.push(Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: false,
        modifiers: m,
    });
    h.step();
    h.run_steps(3);
}

/// Clica no cabeçalho de uma coluna (com os modificadores).
pub fn clicar_cabecalho(h: &mut H, coluna: usize, modifiers: Modifiers) {
    let pos = doc(h).centro_cabecalho(coluna).expect("o cabeçalho à vista");
    clicar_em(h, pos, modifiers);
}

/// Cola `texto` como o Ctrl+V do sistema (o `Event::Paste`), com a grade focada.
pub fn colar(h: &mut H, texto: &str) {
    h.input_mut().events.push(Event::Paste(texto.to_string()));
    h.step();
    h.run_steps(2);
}

/// O que foi para a área de transferência nos próximos quadros (até 10 s).
pub fn area_de_transferencia(h: &mut H) -> Option<String> {
    let inicio = Instant::now();
    while inicio.elapsed() < Duration::from_secs(10) {
        let achado = h.output().platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        });
        if achado.is_some() {
            return achado;
        }
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

/// Digita `texto` (no campo com o foco).
pub fn digitar(h: &mut H, texto: &str) {
    h.input_mut().events.push(Event::Text(texto.to_string()));
    h.step();
    h.run_steps(2);
}

/// Clica na célula (linha da vista, coluna de dados) à vista.
pub fn clicar_celula(h: &mut H, linha: usize, coluna: usize) {
    let pos = doc(h).centro_celula(linha, coluna).expect("a célula à vista");
    clicar_em(h, pos, Modifiers::NONE);
}

/// Aperta uma tecla (com os modificadores).
pub fn tecla(h: &mut H, key: Key, modifiers: Modifiers) {
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.step();
    h.run_steps(2);
}

/// Renderiza (na vez do wgpu) e grava a captura em `capturas/rust/`.
pub fn capturar(h: &mut H, nome: &str) -> PathBuf {
    h.run_steps(2);
    let imagem = {
        let _vez = vez();
        h.render().expect("renderizar com wgpu")
    };
    let destino = estado::apoio_testes::pasta_capturas().join(nome);
    imagem.save(&destino).expect("gravar png");
    destino
}
