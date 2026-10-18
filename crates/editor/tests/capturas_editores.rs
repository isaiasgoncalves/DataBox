//! As capturas do fechamento das etapas 5 e 6 na casca inteira, a 1280×800 (o tamanho inicial da janela): o editor
//! de CSV, a janela Trazer colunas, o JSON válido e com o erro, o editor de texto e a pergunta "Salvar as alterações
//! em…?". As do CSV filtrado e das planilhas do Excel saem dos palcos dos crates das telas (`tela_csv`,
//! `tela_planilha`), que têm os montadores de arquivo. Tudo em `capturas/rust/etapa56-*.png`.

#![cfg(feature = "telas")]

mod apoio;

use std::path::{Path, PathBuf};

use apoio::*;
use egui::Key;
use egui::accesskit::Role;
use estado::Rota;
use estado::apoio_testes::Ambiente;
use interface::tema::Tema;

/// O tamanho das capturas.
const TELA: egui::Vec2 = egui::vec2(1280.0, 800.0);

const REGIOES: [&str; 5] = ["Sul", "Sudeste", "Nordeste", "Norte", "Centro-Oeste"];
const VENDEDORES: [&str; 6] =
    ["Ana Coutinho", "Bruno Brandão", "Carla Ibiapina", "João Esteves", "Marina Lacerda", "Pedro Guimarães"];
const PRODUTOS: [&str; 6] =
    ["Caderno universitário", "Caneta gel", "Mochila escolar", "Agenda 2026", "Estojo duplo", "Marca-texto"];

/// O `vendas_2025.csv` da prancheta 7 (data, regiao, vendedor, produto, quantidade, valor; `;`, vírgula decimal, CRLF).
fn vendas(linhas: usize) -> String {
    let mut s = String::from("data;regiao;vendedor;produto;quantidade;valor\r\n");
    for i in 0..linhas {
        let q = ((i * 7) % 23) + 1;
        let v = format!("{:.2}", q as f64 * (12.9 + (i % 6) as f64 * 9.35)).replace('.', ",");
        let (mes, dia) = (1 + (i / 28) % 12, 1 + i % 28);
        s.push_str(&format!(
            "2025-{mes:02}-{dia:02};{};{};{};{q};{v}\r\n",
            REGIOES[i % 5],
            VENDEDORES[i % 6],
            PRODUTOS[(i * 5) % 6]
        ));
    }
    s
}

/// A pasta dos arquivos do teste, com o arquivo gravado.
fn gravar(amb: &Ambiente, nome: &str, conteudo: &str) -> PathBuf {
    let pasta = amb.pasta().join("dados");
    std::fs::create_dir_all(&pasta).expect("a pasta");
    let caminho = pasta.join(nome);
    std::fs::write(&caminho, conteudo).expect("o arquivo");
    caminho
}

fn editor(amb: &Ambiente, tema: Tema) -> H {
    harness_com(amb, tema, TELA, editor::Partida::default(), |_| None)
}

fn abrir(h: &mut H, caminho: &Path, pronto: &str) {
    h.state_mut().ir(Rota::arquivo(caminho));
    h.run_steps(2);
    let alvo = pronto.to_string();
    esperar(h, pronto, move |h| tem_texto(h, &alvo));
    assentar(h);
}

/// O JSON da prancheta 8 (as conexões), válido; e o mesmo com a vírgula faltando antes de "arquivo" (prancheta 15, N1).
const CONFIG: &str = r#"{
  "nome": "Painel de vendas",
  "versao": "2.4.1",
  "tema": "escuro",
  "conexoes": [
    {
      "nome": "principal",
      "arquivo": "C:\\dados\\vendas.db",
      "somente_leitura": false,
      "tempo_limite": 30
    },
    {
      "nome": "arquivo morto",
      "arquivo": "C:\\dados\\historico_2019.db",
      "somente_leitura": true,
      "tempo_limite": 60
    }
  ],
  "exportar": {
    "pasta": "C:\\Users\\ana\\Documentos",
    "formato": "xlsx",
    "abrir_ao_terminar": true
  },
  "atalhos": null
}
"#;

#[test]
fn capturas_do_csv_e_do_trazer_colunas() {
    for (tema, sufixo) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let amb = Ambiente::vazio(&format!("capturas_csv_{sufixo}"));
        let precos = gravar(
            &amb,
            "precos_2025.csv",
            "produto;preco;fornecedor\r\nCaderno universitário;18,90;Tilibra\r\nCaneta gel;4,50;Pilot\r\nMochila escolar;129,00;Sestini\r\nAgenda 2026;39,90;Tilibra\r\nEstojo duplo;24,90;Faber\r\n",
        );
        let csv = gravar(&amb, "vendas_2025.csv", &vendas(1204));
        let mut h = editor(&amb, tema);
        abrir(&mut h, &precos, "5 linhas · 3 colunas");
        abrir(&mut h, &csv, "1.204 linhas · 6 colunas");
        capturar(&mut h, &format!("etapa56-csv-{sufixo}.png"));
        if tema == Tema::Escuro {
            clicar_botao(&mut h, "Trazer colunas");
            esperar(&mut h, "a janela", |h| tem_texto(h, "para vendas_2025.csv"));
            assentar(&mut h);
            h.remove_cursor();
            h.run_steps(2);
            capturar(&mut h, "etapa56-trazer-colunas-escuro.png");
            h.key_press(Key::Escape);
            h.run_steps(2);
        }
    }
}

#[test]
fn capturas_do_json_e_do_texto() {
    for (tema, sufixo) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let amb = Ambiente::vazio(&format!("capturas_json_{sufixo}"));
        let config = gravar(&amb, "config.json", CONFIG);
        let mut h = editor(&amb, tema);
        abrir(&mut h, &config, "JSON válido");
        capturar(&mut h, &format!("etapa56-json-{sufixo}.png"));
        let quebrado = gravar(
            &amb,
            "config_com_erro.json",
            &CONFIG.replacen(
                "\"somente_leitura\": true,\n      \"tempo_limite\": 60",
                "\"somente_leitura\": true\n      \"tempo_limite\": 60",
                1,
            ),
        );
        h.state_mut().ir(Rota::arquivo(&quebrado));
        h.run_steps(2);
        esperar(&mut h, "o erro", |h| tem_texto(h, "JSON inválido na linha"));
        assentar(&mut h);
        capturar(&mut h, &format!("etapa56-json-invalido-{sufixo}.png"));
        let notas = gravar(
            &amb,
            "leia-me.txt",
            "DataBox — notas da versão 0.1.0\r\n\r\nAbre bancos SQLite, CSV, JSON, texto e planilhas do Excel.\r\n\r\n- Ctrl+S salva o arquivo aberto.\r\n- Ctrl+Z e Ctrl+Y desfazem e refazem.\r\n- Arraste um arquivo para a janela para abri-lo.\r\n",
        );
        h.state_mut().ir(Rota::arquivo(&notas));
        h.run_steps(2);
        esperar(&mut h, "o texto", |h| tem_texto(h, "leia-me.txt"));
        assentar(&mut h);
        capturar(&mut h, &format!("etapa56-texto-{sufixo}.png"));
    }
}

#[test]
fn captura_da_pergunta_ao_fechar() {
    for (tema, sufixo) in [(Tema::Escuro, "escuro"), (Tema::Claro, "claro")] {
        let amb = Ambiente::vazio(&format!("capturas_fechar_{sufixo}"));
        let csv = gravar(&amb, "vendas_2025.csv", &vendas(40));
        let mut h = editor(&amb, tema);
        abrir(&mut h, &csv, "40 linhas · 6 colunas");
        // a primeira célula de dados (abaixo do cabeçalho, depois da caixa de marcar e do número da linha)
        let grade = {
            use egui_kittest::kittest::{Queryable, by};
            h.get(by().label("Grade")).rect()
        };
        let pos = egui::pos2(grade.left() + 32.0 + 48.0 + 20.0, grade.top() + 14.0);
        h.event(egui::Event::PointerMoved(pos));
        for pressed in [true, false] {
            let button = egui::PointerButton::Primary;
            h.event(egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.run_steps(2);
        h.key_press(Key::Enter);
        h.run_steps(2);
        h.event(egui::Event::Text("2025-12-31".into()));
        h.key_press(Key::Enter);
        esperar(&mut h, "a alteração", |h| tem_texto(h, "1 alteração não salva"));
        h.key_press_modifiers(CTRL, Key::W);
        esperar(&mut h, "a pergunta", |h| existe(h, Role::Dialog, "Salvar as alterações em vendas_2025.csv?"));
        h.remove_cursor();
        h.run_steps(2);
        capturar(&mut h, &format!("etapa56-fechar-alterado-{sufixo}.png"));
    }
}
