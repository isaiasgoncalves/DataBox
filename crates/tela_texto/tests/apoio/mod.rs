//! Apoio dos testes da tela de texto e JSON: abrir um arquivo num palco (1440×900, a tela desenhada como a casca), esperar
//! a leitura, digitar, apertar teclas e conferir o disco.
//!
//! Criar, renderizar e desfazer um palco (um wgpu) passa por uma vez só ([`vez`]): vários ao mesmo tempo, nas threads
//! dos testes, às vezes derrubam o processo dentro do driver (0xC0000005); o resto dos testes segue em paralelo.

#![allow(dead_code)]

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use estado::apoio_testes::{Ambiente, NoPalco, Palco, esperar, palco_documento};
use interface::tema::Tema;
use tela_texto::{Modo, TelaTexto};

pub type Tela = NoPalco<TelaTexto>;
pub type H = Harness<'static, Palco<Tela>>;

static VEZ: Mutex<()> = Mutex::new(());

/// A vez de mexer no wgpu de um palco (criar, renderizar, desfazer), uma thread de cada vez.
pub fn vez() -> MutexGuard<'static, ()> {
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// O palco de um teste: usado como o próprio harness (`Deref`), desfeito na sua vez.
pub struct Teste {
    h: Option<H>,
    /// O ambiente (a pasta temporária do teste) vive até o fim do teste.
    pub amb: Ambiente,
    pub caminho: PathBuf,
}

impl Deref for Teste {
    type Target = H;
    fn deref(&self) -> &H {
        self.h.as_ref().expect("palco desfeito")
    }
}

impl DerefMut for Teste {
    fn deref_mut(&mut self) -> &mut H {
        self.h.as_mut().expect("palco desfeito")
    }
}

impl Drop for Teste {
    fn drop(&mut self) {
        let _vez = vez();
        drop(self.h.take());
    }
}

/// O tamanho das capturas (a viewport das pranchetas).
pub const TAMANHO: egui::Vec2 = vec2(1440.0, 900.0);

/// Grava `bytes` como `nome` numa pasta do ambiente e devolve o caminho.
pub fn gravar(amb: &Ambiente, nome: &str, bytes: &[u8]) -> PathBuf {
    let pasta = amb.pasta().join("arquivos");
    std::fs::create_dir_all(&pasta).expect("criar a pasta dos arquivos");
    let caminho = pasta.join(nome);
    std::fs::write(&caminho, bytes).expect("gravar o arquivo de teste");
    caminho
}

/// Abre o arquivo `nome` com `bytes` num palco claro e espera a leitura terminar (editor, leitor ou falha).
pub fn abrir(nome_do_teste: &str, nome: &str, bytes: &[u8]) -> Teste {
    abrir_com(nome_do_teste, nome, bytes, Tema::Claro, TAMANHO)
}

pub fn abrir_com(nome_do_teste: &str, nome: &str, bytes: &[u8], tema: Tema, tamanho: egui::Vec2) -> Teste {
    abrir_criando(nome_do_teste, nome, tema, tamanho, |caminho| std::fs::write(caminho, bytes).expect("gravar"))
}

/// Abre o arquivo `nome` que `criar` faz (ou não faz: o arquivo que sumiu) no caminho que recebe.
pub fn abrir_criando(
    nome_do_teste: &str,
    nome: &str,
    tema: Tema,
    tamanho: egui::Vec2,
    criar: impl FnOnce(&std::path::Path),
) -> Teste {
    let amb = Ambiente::vazio(nome_do_teste);
    let pasta = amb.pasta().join("arquivos");
    std::fs::create_dir_all(&pasta).expect("criar a pasta dos arquivos");
    let caminho = pasta.join(nome);
    criar(&caminho);
    let h = {
        let _vez = vez();
        palco_documento(&amb, TelaTexto::nova(caminho.clone()), tema, tamanho)
    };
    let mut t = Teste { h: Some(h), amb, caminho };
    esperar_a_leitura(&mut t);
    t
}

/// Roda quadros até a tela sair do "Lendo" e a tarefa da análise do JSON de abertura (já feita na leitura) estar
/// assentada.
pub fn esperar_a_leitura(t: &mut Teste) {
    esperar(&mut **t, |h| !matches!(h.state().tela.doc.modo(), Modo::Inicial | Modo::Lendo));
}

/// O texto que a tela mostra.
pub fn texto(t: &H) -> String {
    t.state().tela.doc.texto().unwrap_or_default().to_string()
}

/// Foca o campo de texto do arquivo `nome` e digita `texto` no cursor.
pub fn digitar(t: &mut H, nome: &str, texto: &str) {
    let rotulo = format!("Texto de {nome}");
    t.get_by_label(&rotulo).focus();
    t.run_steps(2);
    t.get_by_label(&rotulo).type_text(texto);
    t.run_steps(2);
}

/// Aperta uma tecla com os modificadores e roda alguns quadros.
pub fn tecla(t: &mut H, modificadores: Modifiers, tecla: Key) {
    t.key_press_modifiers(modificadores, tecla);
    t.run_steps(2);
}

/// Clica no botão da barra (ou da tela) com este rótulo e roda quadros.
pub fn clicar(t: &mut H, rotulo: &str) {
    t.get_by_label(rotulo).click();
    t.run_steps(3);
}

/// Roda quadros até `condicao` valer (as tarefas de fundo terminam em outras threads).
pub fn esperar_ate(t: &mut Teste, condicao: impl FnMut(&H) -> bool) {
    esperar(&mut **t, condicao);
}

/// Renderiza (na vez do wgpu) e grava a captura em `capturas/rust/`.
pub fn capturar(t: &mut H, nome: &str) -> PathBuf {
    t.run_steps(2);
    let imagem = {
        let _vez = vez();
        t.render().expect("renderizar com wgpu")
    };
    let destino = estado::apoio_testes::pasta_capturas().join(nome);
    imagem.save(&destino).expect("gravar png");
    destino
}

/// O JSON da prancheta 8 (24 linhas), com os cinco nomes da estrutura do documento.
pub const JSON_DA_PRANCHETA: &str = "{\n  \"versao\": 3,\n  \"tema\": \"escuro\",\n  \"idioma\": \"pt-BR\",\n  \"conexoes\": [\n    {\n      \"nome\": \"Produção\",\n      \"arquivo\": \"C:\\\\dados\\\\exemplo.db\",\n      \"tempo_limite\": 30,\n      \"somente_leitura\": false\n    },\n    {\n      \"nome\": \"Relatórios\",\n      \"arquivo\": \"D:\\\\relatorios\\\\financeiro.sqlite\",\n      \"tempo_limite\": 60,\n      \"somente_leitura\": true\n    }\n  ],\n  \"exportacao\": {\n    \"formato\": \"xlsx\",\n    \"separador\": \";\",\n    \"cabecalho\": true\n  }\n}";

/// O mesmo JSON sem a vírgula do fim da linha 13 (a prancheta 15, N1): o erro é na linha 14, coluna 7.
pub fn json_invalido_da_prancheta() -> String {
    JSON_DA_PRANCHETA.replace("\"nome\": \"Relatórios\",", "\"nome\": \"Relatórios\"")
}
