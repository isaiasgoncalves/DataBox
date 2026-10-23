//! Renderiza o mostruário com o `egui_kittest` (wgpu) nos dois temas e grava as capturas em
//! `capturas/rust/` na raiz do projeto, para conferir contra as pranchetas de `design/canvas`.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use egui::UiBuilder;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use interface::mostruario::Mostruario;
use interface::tema::{self, Tema};
use interface::{fontes, pintura};

/// `capturas/rust/` na raiz do projeto (o crate fica em `crates/interface`).
fn pasta_capturas() -> PathBuf {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let pasta = raiz.join("capturas").join("rust");
    std::fs::create_dir_all(&pasta).expect("criar capturas/rust");
    pasta
}

fn quadro(ui: &mut egui::Ui, m: &mut Mostruario, depois: &mut dyn FnMut(&mut Mostruario)) {
    let ctx = ui.ctx().clone();
    tema::aplicar(&ctx, m.tema());
    // capturas estáveis: sem transição, sem animação contínua
    ctx.all_styles_mut(|s| {
        s.animation_time = 0.0;
        s.visuals.text_cursor.blink = false;
    });
    tema::definir_movimento_reduzido(&ctx, true);
    if !fontes::instaladas(&ctx) {
        ctx.request_repaint();
        return;
    }
    depois(m);
    // a página ocupa a janela inteira (o harness põe 8px de margem em volta do `Ui`)
    let tela = ctx.content_rect();
    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
    pagina.set_clip_rect(tela);
    pintura::pintar_fundo(pagina.painter(), tema::paleta(&pagina), tela);
    if m.casca_a_vista() {
        m.pagina_casca(&mut pagina);
    } else if m.icones_a_vista() {
        m.pagina_icones(&mut pagina);
    } else if m.codigo_a_vista() {
        m.pagina_codigo(&mut pagina);
    } else if m.contexto_a_vista() {
        m.pagina_contexto(&mut pagina);
    } else {
        m.pagina(&mut pagina);
    }
    m.sobreposicoes(&mut pagina);
}

fn capturar(tema_escolhido: Tema, nome: &str, altura: f32, preparar: fn(&mut Mostruario)) {
    capturar_com(tema_escolhido, nome, altura, preparar, |_| {});
}

/// Captura depois de uma interação (`agir` recebe o harness já pronto).
fn capturar_com(
    tema_escolhido: Tema,
    nome: &str,
    altura: f32,
    preparar: fn(&mut Mostruario),
    agir: fn(&mut Harness<'_, Mostruario>),
) {
    capturar_em(tema_escolhido, nome, egui::vec2(1440.0, altura), 1.0, preparar, agir);
}

/// Captura numa janela de `tamanho` pontos a `escala` pixels por ponto (a imagem tem `tamanho × escala` pixels).
fn capturar_em(
    tema_escolhido: Tema,
    nome: &str,
    tamanho: egui::Vec2,
    escala: f32,
    preparar: fn(&mut Mostruario),
    agir: fn(&mut Harness<'_, Mostruario>),
) {
    let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
    let mut preparar_uma_vez = Some(preparar);
    let mut harness = Harness::builder()
        .with_size(tamanho)
        .with_pixels_per_point(escala)
        .with_theme(tema_egui)
        .with_max_steps(12)
        .wgpu()
        .build_ui_state(
            move |ui, m: &mut Mostruario| {
                quadro(ui, m, &mut |m| {
                    if let Some(f) = preparar_uma_vez.take() {
                        f(m);
                    }
                })
            },
            Mostruario::new(tema_escolhido),
        );
    // as fontes do sistema (Consolas, Georgia, Segoe UI Symbol) chegam numa thread
    let inicio = std::time::Instant::now();
    while !fontes::do_sistema_carregadas(&harness.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        harness.step();
    }
    harness.run_steps(6);
    agir(&mut harness);
    harness.run_steps(3);
    let imagem = harness.render().expect("renderizar com wgpu");
    let destino = pasta_capturas().join(nome);
    imagem.save(&destino).expect("gravar png");
    assert_eq!((imagem.width(), imagem.height()), ((tamanho.x * escala) as u32, (tamanho.y * escala) as u32));
    // o mostruário tem as peças principais na árvore de acessibilidade
    let rotulo = if harness.state().casca_a_vista() {
        "Fechar a janela"
    } else if harness.state().icones_a_vista() {
        "Ver a casca"
    } else if harness.state().codigo_a_vista() {
        "Monokai"
    } else if harness.state().contexto_a_vista() {
        "Explorador"
    } else {
        "Salvar (.pri)"
    };
    let _ = harness.get_by_label(rotulo);
}

/// O menu de contexto aberto no clique direito de um arquivo (prancheta 15) e no da pasta, nos dois temas (capturas
/// `menu-de-contexto-{claro,escuro}` e `menu-de-contexto-pasta-{claro,escuro}`).
#[test]
fn menu_de_contexto_claro_e_escuro() {
    let pagina = |m: &mut Mostruario| m.mostrar_contexto(true);
    for (tema_escolhido, nome) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        capturar_em(
            tema_escolhido,
            &format!("menu-de-contexto-{nome}.png"),
            egui::vec2(960.0, 640.0),
            1.0,
            pagina,
            |h| {
                h.get_by_label("vendas_2025.csv").click_secondary();
                h.run_steps(3);
            },
        );
        capturar_em(
            tema_escolhido,
            &format!("menu-de-contexto-pasta-{nome}.png"),
            egui::vec2(960.0, 640.0),
            1.0,
            pagina,
            |h| {
                h.get_by_label("dados").focus();
                h.run_steps(2);
                h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F10);
                h.run_steps(4);
            },
        );
    }
}

/// O clique direito numa célula da grade de exemplo: a `linha` e `x` a partir da borda esquerda (o retângulo da
/// "Grade" é o corpo, abaixo do cabeçalho).
fn clique_direito_na_grade(h: &mut Harness<'_, Mostruario>, linha: usize, x: f32) {
    let grade = h.get_by_label("Grade").rect();
    let pos = egui::pos2(grade.left() + x, grade.top() + interface::grade::ALTURA_LINHA * (linha as f32 + 0.5));
    h.hover_at(pos);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(3);
}

/// Um intervalo de 3 linhas × 2 colunas (nome e cidade) na grade de exemplo, com a primeira célula ativa.
fn intervalo_na_grade(h: &mut Harness<'_, Mostruario>) {
    let grade = h.get_by_label("Grade").rect();
    let pos = egui::pos2(grade.left() + 100.0, grade.top() + interface::grade::ALTURA_LINHA * 1.5);
    h.hover_at(pos);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    for tecla in [egui::Key::ArrowDown, egui::Key::ArrowDown, egui::Key::ArrowRight] {
        h.key_press_modifiers(egui::Modifiers::SHIFT, tecla);
        h.run_steps(1);
    }
}

/// O menu da célula (0.2.3) aberto pelo clique direito dentro de um intervalo, com o submenu "Copiar como" à vista,
/// e pelo Shift+F10 com o ponteiro no "Definir como NULL" desligado (a dica com o motivo), nos dois temas (capturas
/// `menu-da-celula-{claro,escuro}` e `menu-da-celula-desligado-{claro,escuro}`).
#[test]
fn menu_da_celula_claro_e_escuro() {
    let pagina = |m: &mut Mostruario| m.mostrar_contexto(true);
    for (tema_escolhido, nome) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        capturar_em(
            tema_escolhido,
            &format!("menu-da-celula-{nome}.png"),
            egui::vec2(960.0, 1000.0),
            1.0,
            pagina,
            |h| {
                intervalo_na_grade(h);
                clique_direito_na_grade(h, 2, 100.0);
                let menu = h.get_by_role(egui::accesskit::Role::Menu).accesskit_node().label();
                assert_eq!(menu.as_deref(), Some("Menu da seleção"), "dentro do intervalo, o menu age nele");
                h.get_by_label("Copiar como").hover();
                h.run_steps(3);
                let _ = h.get_by_label("Lista para IN (…)");
            },
        );
        capturar_em(
            tema_escolhido,
            &format!("menu-da-celula-desligado-{nome}.png"),
            egui::vec2(960.0, 1000.0),
            1.0,
            pagina,
            |h| {
                clique_direito_na_grade(h, 3, 100.0);
                h.key_press(egui::Key::Escape);
                h.run_steps(2);
                h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F10);
                h.run_steps(4);
                let menu = h.get_by_role(egui::accesskit::Role::Menu).accesskit_node().label();
                assert_eq!(menu.as_deref(), Some("Menu da célula"), "o Shift+F10 abre na célula ativa");
                h.get_by_label("Definir como NULL").hover();
                h.run_steps(6);
            },
        );
    }
}

/// Os cartões de ação do Início (prancheta 4) parados e com o ponteiro no primário, nos dois temas (capturas
/// `acoes-{claro,escuro}` e `acoes-hover-{claro,escuro}`).
#[test]
fn acoes_claro_e_escuro() {
    let pagina = |m: &mut Mostruario| m.mostrar_contexto(true);
    for (tema_escolhido, nome) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        capturar_em(tema_escolhido, &format!("acoes-{nome}.png"), egui::vec2(960.0, 640.0), 1.0, pagina, |_| {});
        capturar_em(tema_escolhido, &format!("acoes-hover-{nome}.png"), egui::vec2(960.0, 640.0), 1.0, pagina, |h| {
            h.get_by_label("Abrir pasta").hover();
            h.run_steps(3);
        });
    }
}

/// A peça de código nas quatro cores e o editor com os números, a linha com erro e o sublinhado, nos dois temas.
#[test]
fn codigo_claro_e_escuro() {
    let codigo = |m: &mut Mostruario| m.mostrar_codigo(true);
    capturar(Tema::Claro, "mostruario-codigo-claro.png", 960.0, codigo);
    capturar(Tema::Escuro, "mostruario-codigo-escuro.png", 960.0, codigo);
}

/// O JSON da prancheta 8 (24 linhas), com a linha 9 (a do cursor) com `--selh`.
const JSON_DA_PRANCHETA_8: &str = "{\n  \"versao\": 3,\n  \"tema\": \"escuro\",\n  \"idioma\": \"pt-BR\",\n  \"conexoes\": [\n    {\n      \"nome\": \"Produção\",\n      \"arquivo\": \"C:\\\\dados\\\\exemplo.db\",\n      \"tempo_limite\": 30,\n      \"somente_leitura\": false\n    },\n    {\n      \"nome\": \"Relatórios\",\n      \"arquivo\": \"D:\\\\relatorios\\\\financeiro.sqlite\",\n      \"tempo_limite\": 60,\n      \"somente_leitura\": true\n    }\n  ],\n  \"exportacao\": {\n    \"formato\": \"xlsx\",\n    \"separador\": \";\",\n    \"cabecalho\": true\n  }\n}";

/// O JSON da prancheta 15 (N1): falta a vírgula depois de `"somente_leitura"`, na linha 14.
const JSON_COM_ERRO: &str = "{\n  \"versao\": 3,\n  \"tema\": \"escuro\",\n  \"conexoes\": [\n    {\n      \"nome\": \"Produção\",\n      \"tempo_limite\": 30,\n      \"somente_leitura\": false\n    }\n  ],\n  \"exportacao\": {\n    \"formato\": \"xlsx\",\n    \"separador\": \";\"\n    \"arquivo\": \"saida.xlsx\"\n  }\n}";

/// O editor de código plano em JSON (a peça do passo 4 da etapa 5: as cores da chave, a linha atual em `--selh` e, no
/// inválido, a linha do erro com o fundo, o sublinhado e o número da margem em `--perigo`), nos dois temas (capturas
/// `editor-json-{claro,escuro}` e `editor-json-invalido-{claro,escuro}`), contra as pranchetas 8 e 15.
#[test]
fn editor_json_claro_e_escuro() {
    use interface::codigo::Linguagem;
    use interface::pecas::campos::AreaTexto;
    struct Estado {
        tema: Tema,
        texto: String,
        linha_atual: usize,
        erro: Option<(usize, std::ops::Range<usize>)>,
    }
    let casos = [
        ("editor-json", JSON_DA_PRANCHETA_8, 8, None),
        // o erro: a linha 14 (índice 13) e o `"arquivo"` da 15 (o símbolo que estraga), em caracteres
        (
            "editor-json-invalido",
            JSON_COM_ERRO,
            13,
            Some((13, {
                let b = JSON_COM_ERRO.find("\"arquivo\"").unwrap();
                let c = JSON_COM_ERRO[..b].chars().count();
                c..c + "\"arquivo\"".chars().count()
            })),
        ),
    ];
    for (nome, texto, linha_atual, erro) in casos {
        for (tema_escolhido, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
            let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
            let mut h = Harness::builder()
                .with_size(egui::vec2(760.0, 560.0))
                .with_pixels_per_point(1.0)
                .with_theme(tema_egui)
                .with_max_steps(12)
                .wgpu()
                .build_ui_state(
                    |ui, s: &mut Estado| {
                        let ctx = ui.ctx().clone();
                        tema::aplicar(&ctx, s.tema);
                        ctx.all_styles_mut(|st| {
                            st.animation_time = 0.0;
                            st.visuals.text_cursor.blink = false;
                        });
                        tema::definir_movimento_reduzido(&ctx, true);
                        if !fontes::instaladas(&ctx) {
                            ctx.request_repaint();
                            return;
                        }
                        let tela = ctx.content_rect();
                        let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                        pagina.set_clip_rect(tela);
                        let p = tema::paleta(&pagina);
                        pintura::pintar_fundo(pagina.painter(), p, tela);
                        let doc = tela.shrink(16.0);
                        pagina.painter().rect_filled(doc, pintura::RAIO_PADRAO, p.estudio.doc);
                        let mut filho = pagina.new_child(UiBuilder::new().id_salt("json").max_rect(doc.shrink(1.0)));
                        let mut area = AreaTexto::new(&mut s.texto)
                            .id_salt("editor-json")
                            .mono(true)
                            .codigo(true)
                            .plana(true)
                            .linguagem(Linguagem::Json)
                            .numeros_de_linha(true)
                            .marcar_linhas(s.linha_atual..s.linha_atual + 1, p.estudio.selh)
                            .altura(doc.height() - 2.0)
                            .largura(doc.width() - 2.0);
                        if let Some((linha, trecho)) = s.erro.clone() {
                            area = area
                                .marcar_linhas(linha..linha + 1, p.estudio.perigo.gamma_multiply(0.09))
                                .sublinhar(trecho)
                                .erro_na_linha(linha);
                        }
                        area.mostrar(&mut filho);
                    },
                    Estado { tema: tema_escolhido, texto: texto.into(), linha_atual, erro: erro.clone() },
                );
            let inicio = std::time::Instant::now();
            while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                h.step();
            }
            h.run_steps(6);
            let imagem = h.render().expect("renderizar com wgpu");
            imagem.save(pasta_capturas().join(format!("{nome}-{sufixo}.png"))).expect("gravar png");
        }
    }
}

/// As linhas do CSV da prancheta 7 (`vendas_2025.csv`): data, região, vendedor, produto, quantidade e valor.
const LINHAS_CSV: [[&str; 6]; 8] = [
    ["2025-01-02", "Sul", "Ana Coutinho", "Caderno universitário", "1", "12,90"],
    ["2025-01-03", "Sudeste", "Bruno Brandão", "Marca-texto", "8", "178,00"],
    ["2025-01-04", "Nordeste", "Carla Ibiapina", "Estojo duplo", "15", "474,00"],
    ["2025-01-05", "Norte", "João Esteves", "Agenda 2026", "22", "900,90"],
    ["2025-01-06", "Centro-Oeste", "Marina Lacerda", "Mochila escolar", "6", "301,80"],
    ["2025-01-07", "Sul", "Pedro Guimarães", "Caneta gel", "13", "775,45"],
    ["2025-01-08", "Sudeste", "Ana Coutinho", "Caderno universitário", "20", "258,00"],
    ["2025-01-09", "", "Bruno Brandão", "Marca-texto", "4", "89,00"],
];

/// A grade do CSV nas peças do passo 5 da etapa 5: texto de célula sem "vazio" (a região da última linha está vazia), a
/// célula alterada (`td.alt`), a coluna `regiao` selecionada (`th.selcol` e `td.selcol`) e a célula ativa, nos dois
/// temas (capturas `grade-csv-{claro,escuro}`, contra as pranchetas 7 e 16).
#[test]
fn grade_csv_claro_e_escuro() {
    use interface::grade::{
        CabecalhoVisivel, CelulaVisivel, DesenhoGrade, Navegacao, NucleoGrade, Th, pintar_cabecalho_coluna,
        pintar_celula_alterada, pintar_numero_linha, pintar_texto_de_celula,
    };
    use interface::icones::TipoColuna;
    const NOMES: [&str; 6] = ["data", "regiao", "vendedor", "produto", "quantidade", "valor"];
    const TIPOS: [TipoColuna; 6] = [
        TipoColuna::Data,
        TipoColuna::Texto,
        TipoColuna::Texto,
        TipoColuna::Texto,
        TipoColuna::Inteiro,
        TipoColuna::Real,
    ];
    struct Desenho;
    impl DesenhoGrade for Desenho {
        fn celula(&mut self, ui: &mut egui::Ui, c: &CelulaVisivel) {
            let p = tema::paleta(ui);
            let pintor = ui.painter().with_clip_rect(c.recorte);
            if c.coluna == 0 {
                pintar_numero_linha(&pintor, p, c.conteudo, &(c.linha + 1).to_string());
                return;
            }
            if (c.linha, c.coluna) == (2, 6) {
                pintar_celula_alterada(&pintor, p, c.celula);
            }
            let texto = LINHAS_CSV[c.linha][c.coluna - 1];
            pintar_texto_de_celula(&pintor, p, c.conteudo, texto, c.coluna >= 5);
        }

        fn cabecalho(&mut self, ui: &mut egui::Ui, h: &CabecalhoVisivel) {
            let p = tema::paleta(ui);
            let pintor = ui.painter().with_clip_rect(h.recorte);
            if h.coluna == 0 {
                pintar_cabecalho_coluna(&pintor, p, h.conteudo, &Th { numerica: true, ..Th::new("#") });
                return;
            }
            let i = h.coluna - 1;
            let th = Th {
                pastilha: Some(TIPOS[i]),
                numerica: i >= 4,
                selecionada: self.th_selecionada(h.coluna),
                ..Th::new(NOMES[i])
            };
            pintar_cabecalho_coluna(&pintor, p, h.conteudo, &th);
        }

        fn th_selecionada(&mut self, coluna: usize) -> bool {
            coluna == 2
        }
    }
    struct Estado {
        tema: Tema,
        nucleo: NucleoGrade,
    }
    for (tema_escolhido, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
        let mut nucleo =
            NucleoGrade::new("csv", LINHAS_CSV.len(), vec![48.0, 110.0, 110.0, 130.0, 160.0, 110.0, 110.0])
                .com_navegacao(Navegacao::Dados)
                .com_navegaveis(1..7)
                .com_coluna_numero(0);
        nucleo.selecionar(Some((6, 5)));
        let mut h = Harness::builder()
            .with_size(egui::vec2(900.0, 340.0))
            .with_pixels_per_point(1.0)
            .with_theme(tema_egui)
            .with_max_steps(12)
            .wgpu()
            .build_ui_state(
                |ui, s: &mut Estado| {
                    let ctx = ui.ctx().clone();
                    tema::aplicar(&ctx, s.tema);
                    tema::definir_movimento_reduzido(&ctx, true);
                    if !fontes::instaladas(&ctx) {
                        ctx.request_repaint();
                        return;
                    }
                    let tela = ctx.content_rect();
                    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                    pagina.set_clip_rect(tela);
                    let p = tema::paleta(&pagina);
                    pintura::pintar_fundo(pagina.painter(), p, tela);
                    let doc = tela.shrink(16.0);
                    pagina.painter().rect_filled(doc, pintura::RAIO_PADRAO, p.estudio.doc);
                    let mut filho = pagina.new_child(UiBuilder::new().id_salt("csv").max_rect(doc.shrink(1.0)));
                    s.nucleo.mostrar(&mut filho, doc.height() - 2.0, &mut Desenho);
                },
                Estado { tema: tema_escolhido, nucleo },
            );
        let inicio = std::time::Instant::now();
        while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            h.step();
        }
        h.run_steps(6);
        let imagem = h.render().expect("renderizar com wgpu");
        imagem.save(pasta_capturas().join(format!("grade-csv-{sufixo}.png"))).expect("gravar png");
    }
}

/// A planilha do Excel em miniatura (prancheta 17, cena a) nos dois temas: a linha 1 (o título A1:F1 mesclado) e a coluna
/// A congeladas, o fio de 2px `--b2` na emenda, as larguras do arquivo e a célula ativa E6 (capturas
/// `grade-congelada-{claro,escuro}`); rolada para o fim, com o painel congelado no lugar e o título ainda por cima
/// (`grade-congelada-rolada-{claro,escuro}`); e numa janela estreita, rolada para a direita e para baixo, com a
/// mescla do título cruzando a emenda (`grade-congelada-estreita-{claro,escuro}`).
#[test]
fn grade_congelada_claro_e_escuro() {
    use interface::mostruario::planilha::PlanilhaExemplo;
    struct Estado {
        tema: Tema,
        planilha: PlanilhaExemplo,
    }
    for (tema_escolhido, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        // (nome, janela, a célula para onde a grade rola antes da captura)
        let variantes = [
            ("grade-congelada", egui::vec2(1100.0, 560.0), None),
            ("grade-congelada-rolada", egui::vec2(1100.0, 560.0), Some((52, 4))),
            ("grade-congelada-estreita", egui::vec2(560.0, 420.0), Some((30, 6))),
        ];
        for (nome, janela, rolar_para) in variantes {
            let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
            let mut h = Harness::builder()
                .with_size(janela)
                .with_pixels_per_point(1.0)
                .with_theme(tema_egui)
                .with_max_steps(12)
                .wgpu()
                .build_ui_state(
                    |ui, s: &mut Estado| {
                        let ctx = ui.ctx().clone();
                        tema::aplicar(&ctx, s.tema);
                        tema::definir_movimento_reduzido(&ctx, true);
                        if !fontes::instaladas(&ctx) {
                            ctx.request_repaint();
                            return;
                        }
                        let tela = ctx.content_rect();
                        let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                        pagina.set_clip_rect(tela);
                        let p = tema::paleta(&pagina);
                        pintura::pintar_fundo(pagina.painter(), p, tela);
                        let doc = tela.shrink(16.0);
                        pagina.painter().rect_filled(doc, pintura::RAIO_PADRAO, p.estudio.doc);
                        let mut filho =
                            pagina.new_child(UiBuilder::new().id_salt("planilha").max_rect(doc.shrink(1.0)));
                        s.planilha.mostrar(&mut filho, doc.height() - 2.0);
                    },
                    Estado { tema: tema_escolhido, planilha: PlanilhaExemplo::new("planilha") },
                );
            let inicio = std::time::Instant::now();
            while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                h.step();
            }
            h.run_steps(6);
            if let Some(celula) = rolar_para {
                h.state_mut().planilha.nucleo_mut().selecionar(Some(celula));
                h.run_steps(8);
            }
            let imagem = h.render().expect("renderizar com wgpu");
            imagem.save(pasta_capturas().join(format!("{nome}-{sufixo}.png"))).expect("gravar png");
        }
    }
}

/// As abas com a de CSV alterada (o ponto em `--acct` no lugar do ×) e com o ponteiro nela (o × volta), nos dois temas
/// (capturas `abas-alterada-{claro,escuro}` e `abas-alterada-hover-{claro,escuro}`), contra as pranchetas 7 e 15.
#[test]
fn abas_alterada_claro_e_escuro() {
    use interface::icones::{Icone, TipoArquivo};
    use interface::pecas::abas_documento::{AbaDocumento, FaixaAbas, pintar_documento};
    for (tema_escolhido, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        for com_hover in [false, true] {
            let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
            let mut h = Harness::builder()
                .with_size(egui::vec2(700.0, 120.0))
                .with_pixels_per_point(1.0)
                .with_theme(tema_egui)
                .with_max_steps(12)
                .wgpu()
                .build_ui_state(
                    |ui, t: &mut Tema| {
                        let ctx = ui.ctx().clone();
                        tema::aplicar(&ctx, *t);
                        ctx.all_styles_mut(|s| s.animation_time = 0.0);
                        tema::definir_movimento_reduzido(&ctx, true);
                        if !fontes::instaladas(&ctx) {
                            ctx.request_repaint();
                            return;
                        }
                        let tela = ctx.content_rect();
                        let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                        pagina.set_clip_rect(tela);
                        pintura::pintar_fundo(pagina.painter(), tema::paleta(&pagina), tela);
                        let abas = [
                            AbaDocumento::new(Icone::Banco, "exemplo.db").sub("Visão geral"),
                            AbaDocumento::new(Icone::Tabela, "clientes").sub("Dados"),
                            AbaDocumento::new(Icone::Folha(TipoArquivo::Csv), "vendas_2025.csv").alterada(true),
                            AbaDocumento::new(Icone::Folha(TipoArquivo::Json), "config.json"),
                        ];
                        let faixa = egui::Rect::from_min_size(
                            tela.min + egui::vec2(16.0, 16.0),
                            egui::vec2(tela.width() - 32.0, 35.0),
                        );
                        let mut filho = pagina.new_child(UiBuilder::new().id_salt("abas").max_rect(faixa));
                        let r = FaixaAbas::new("alterada", &abas).ativa(Some(1)).mostrar(&mut filho);
                        let doc = egui::Rect::from_min_max(
                            egui::pos2(faixa.left(), faixa.bottom() - 1.0),
                            egui::pos2(faixa.right(), faixa.bottom() + 40.0),
                        );
                        pintar_documento(pagina.painter(), tema::paleta(&pagina), doc, r.juncao);
                    },
                    tema_escolhido,
                );
            let inicio = std::time::Instant::now();
            while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                h.step();
            }
            h.run_steps(6);
            let nome = if com_hover {
                h.get_by_role_and_label(egui::accesskit::Role::Tab, "vendas_2025.csv, alterada").hover();
                h.run_steps(3);
                format!("abas-alterada-hover-{sufixo}.png")
            } else {
                format!("abas-alterada-{sufixo}.png")
            };
            let imagem = h.render().expect("renderizar com wgpu");
            imagem.save(pasta_capturas().join(nome)).expect("gravar png");
        }
    }
}

/// O aviso compacto com ações da prancheta 16 ("A coluna colada tem 1.204 valores e o arquivo, 1.180 linhas." com
/// Colar 1.180 · Acrescentar 24 linhas · Cancelar), nos dois temas (capturas `aviso-acoes-{claro,escuro}`).
#[test]
fn aviso_com_acoes_claro_e_escuro() {
    use interface::pecas::avisos::{AcaoAviso, TomAviso, aviso_com_acoes};
    use interface::pecas::texto::Trecho;
    for (tema_escolhido, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = Harness::builder()
            .with_size(egui::vec2(720.0, 160.0))
            .with_pixels_per_point(1.0)
            .with_theme(tema_egui)
            .with_max_steps(12)
            .wgpu()
            .build_ui_state(
                |ui, t: &mut Tema| {
                    let ctx = ui.ctx().clone();
                    tema::aplicar(&ctx, *t);
                    tema::definir_movimento_reduzido(&ctx, true);
                    if !fontes::instaladas(&ctx) {
                        ctx.request_repaint();
                        return;
                    }
                    let tela = ctx.content_rect();
                    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                    pagina.set_clip_rect(tela);
                    let p = tema::paleta(&pagina);
                    pintura::pintar_fundo(pagina.painter(), p, tela);
                    let doc = tela.shrink(16.0);
                    pagina.painter().rect_filled(doc, pintura::RAIO_PADRAO, p.estudio.doc);
                    let mut filho = pagina.new_child(UiBuilder::new().id_salt("aviso").max_rect(doc.shrink(14.0)));
                    aviso_com_acoes(
                        &mut filho,
                        &[Trecho::Normal("A coluna colada tem 1.204 valores e o arquivo, 1.180 linhas.")],
                        TomAviso::Atencao,
                        &[
                            AcaoAviso::principal("Colar 1.180"),
                            AcaoAviso::neutra("Acrescentar 24 linhas"),
                            AcaoAviso::sutil("Cancelar").dica("Esc"),
                        ],
                    );
                },
                tema_escolhido,
            );
        let inicio = std::time::Instant::now();
        while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            h.step();
        }
        h.run_steps(6);
        let imagem = h.render().expect("renderizar com wgpu");
        imagem.save(pasta_capturas().join(format!("aviso-acoes-{sufixo}.png"))).expect("gravar png");
    }
}
#[test]
fn mostruario_claro_e_escuro() {
    capturar(Tema::Claro, "mostruario-claro.png", 3900.0, |_| {});
    capturar(Tema::Escuro, "mostruario-escuro.png", 3900.0, |_| {});
}

/// A casca nova inteira (1440×900, como as pranchetas), nos dois temas, e com os menus e submenus abertos: o
/// Exibir › Tema da prancheta Main e o Arquivo › Abertos recentemente da prancheta Inicio.
#[test]
fn casca_claro_e_escuro() {
    let casca = |m: &mut Mostruario| m.mostrar_casca(true);
    capturar_com(Tema::Escuro, "mostruario-casca-escuro.png", 900.0, casca, |_| {});
    capturar_com(Tema::Claro, "mostruario-casca-claro.png", 900.0, casca, |_| {});
    capturar_com(Tema::Escuro, "mostruario-casca-menu-escuro.png", 900.0, casca, |h| {
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Exibir").click();
        h.run_steps(2);
        h.get_by_label("Tema").hover();
        h.run_steps(2);
    });
    capturar_com(Tema::Claro, "mostruario-casca-recentes-claro.png", 900.0, casca, |h| {
        // o menu Arquivo é o único "Arquivo" (a seção das Propriedades é a "Seção Arquivo")
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Arquivo").click();
        h.run_steps(2);
        h.get_by_label("Abertos recentemente").hover();
        h.run_steps(2);
    });
}

#[test]
fn janela_e_confirmacao() {
    capturar(Tema::Claro, "mostruario-janela-claro.png", 900.0, |m| m.abrir_janela());
    capturar(Tema::Escuro, "mostruario-janela-escuro.png", 900.0, |m| m.abrir_janela());
    capturar(Tema::Claro, "mostruario-confirmar-claro.png", 900.0, |m| m.abrir_confirmacao());
}

#[test]
fn menu_lista_balao_e_toast() {
    capturar_com(
        Tema::Claro,
        "mostruario-menu-claro.png",
        900.0,
        |_| {},
        |h| h.get_by_role_and_label(egui::accesskit::Role::Button, "Arquivo").click(),
    );
    capturar_com(
        Tema::Escuro,
        "mostruario-menu-escuro.png",
        900.0,
        |_| {},
        |h| h.get_by_role_and_label(egui::accesskit::Role::Button, "Arquivo").click(),
    );
    capturar_com(
        Tema::Claro,
        "mostruario-select-claro.png",
        1100.0,
        |_| {},
        |h| h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "rh").click(),
    );
    capturar_com(
        Tema::Claro,
        "mostruario-balao-claro.png",
        900.0,
        |_| {},
        |h| h.get_by_label("Sobre esta tela").hover(),
    );
    capturar_com(
        Tema::Escuro,
        "mostruario-toast-escuro.png",
        900.0,
        |_| {},
        |h| h.get_by_label("Mostrar toast").click_accesskit(),
    );
}

/// O que a peça do documento mediu no último quadro.
#[derive(Default)]
struct MedidasDocumento {
    tema: Option<Tema>,
    moldura: Option<egui::Rect>,
    depois_do_aviso: f32,
    depois_da_faixa: f32,
    fim_do_conteudo: f32,
}

/// A peça do `.doc` com o aviso de ponta a ponta no alto, a faixa de 42px, a página e o rodapé de 34px, nos dois
/// temas (capturas `documento-faixa-aviso-rodape-{claro,escuro}`).
#[test]
fn documento_faixa_aviso_rodape() {
    use interface::pecas::avisos::TomAviso;
    use interface::pecas::documento::{ALTURA_FAIXA, ALTURA_RODAPE, Documento, Pagina};
    use interface::pecas::texto::{Trecho, hint, texto};
    for (tema_escolhido, nome) in [
        (Tema::Claro, "documento-faixa-aviso-rodape-claro.png"),
        (Tema::Escuro, "documento-faixa-aviso-rodape-escuro.png"),
    ] {
        let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = Harness::builder()
            .with_size(egui::vec2(900.0, 420.0))
            .with_pixels_per_point(1.0)
            .with_theme(tema_egui)
            .with_max_steps(12)
            .wgpu()
            .build_ui_state(
                |ui, m: &mut MedidasDocumento| {
                    let ctx = ui.ctx().clone();
                    let tema_atual = m.tema.unwrap_or_default();
                    tema::aplicar(&ctx, tema_atual);
                    tema::definir_movimento_reduzido(&ctx, true);
                    if !fontes::instaladas(&ctx) {
                        ctx.request_repaint();
                        return;
                    }
                    let tela = ctx.content_rect();
                    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                    pagina.set_clip_rect(tela);
                    pintura::pintar_fundo(pagina.painter(), tema::paleta(&pagina), tela);
                    let rect = tela.shrink(16.0);
                    let mut d = Documento::abrir(&pagina, rect, None, true);
                    let _ = d.aviso_texto(
                        &mut pagina,
                        TomAviso::Atencao,
                        &[
                            Trecho::Forte("Há uma transação aberta na Consulta SQL."),
                            Trecho::Normal(" O que for salvo agora só vai para o arquivo com COMMIT. "),
                            Trecho::Link("Ir para a Consulta SQL"),
                        ],
                    );
                    m.depois_do_aviso = d.rect_conteudo().top();
                    d.faixa(&mut pagina, |ui| {
                        texto(ui, "Filtros", 12.0, tema::paleta(ui).estudio.t2);
                        hint(ui, "Limpar filtros");
                    });
                    m.depois_da_faixa = d.rect_conteudo().top();
                    m.fim_do_conteudo = d.rect_conteudo().bottom();
                    d.pagina(&mut pagina, Pagina::new("documento"), |ui| {
                        texto(ui, "O conteúdo do documento rola aqui dentro.", 13.0, tema::paleta(ui).fg);
                    });
                    let _ = d.rodape(&mut pagina, |ui| {
                        ui.label("Linhas 1–41 de 41 · filtradas de 60");
                    });
                    m.moldura = Some(d.rect());
                },
                MedidasDocumento { tema: Some(tema_escolhido), ..Default::default() },
            );
        let inicio = std::time::Instant::now();
        while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            h.step();
        }
        h.run_steps(4);
        let m = h.state();
        let moldura = m.moldura.expect("a moldura");
        assert_eq!(m.depois_da_faixa - m.depois_do_aviso, ALTURA_FAIXA);
        assert_eq!(moldura.bottom() - 1.0 - m.fim_do_conteudo, ALTURA_RODAPE);
        assert!(m.depois_do_aviso > moldura.top() + 1.0 + 30.0, "o aviso ocupa o alto: {}", m.depois_do_aviso);
        let _ = h.get_by_label("Ir para a Consulta SQL");
        let _ = h.get_by_label("Linhas 1–41 de 41 · filtradas de 60");
        let imagem = h.render().expect("renderizar com wgpu");
        imagem.save(pasta_capturas().join(nome)).expect("gravar png");
    }
}

/// A dock da Visão geral nos dois temas (capturas `dock-{claro,escuro}`): em cima, as seis colunas na largura do
/// documento a 1440 (776pt), com a ficha em hover (subida 3pt); embaixo, a de 578pt (a 960) com "Linhas
/// (contadas)", que quebra em 3 + 3.
#[test]
fn dock_claro_e_escuro() {
    use interface::icones::Icone;
    use interface::pecas::dock::{ItemDock, dock};
    fn itens(linhas: &str) -> Vec<ItemDock> {
        vec![
            ItemDock::new(Icone::TabelaGrande, "7", "Tabelas"),
            ItemDock::new(Icone::VisaoGrande, "1", "Visão"),
            ItemDock::new(Icone::IndiceGrande, "5", "Índices"),
            ItemDock::new(Icone::GatilhoGrande, "1", "Gatilho"),
            ItemDock::new(Icone::DiscoGrande, "500", "Tamanho").unidade("KB"),
            ItemDock::new(Icone::LinhasGrande, "5.772", linhas),
        ]
    }
    for (tema_escolhido, nome) in [(Tema::Claro, "dock-claro.png"), (Tema::Escuro, "dock-escuro.png")] {
        let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = Harness::builder()
            .with_size(egui::vec2(840.0, 440.0))
            .with_pixels_per_point(1.0)
            .with_theme(tema_egui)
            .with_max_steps(12)
            .wgpu()
            .build_ui_state(
                |ui, t: &mut Tema| {
                    let ctx = ui.ctx().clone();
                    tema::aplicar(&ctx, *t);
                    tema::definir_movimento_reduzido(&ctx, true);
                    if !fontes::instaladas(&ctx) {
                        ctx.request_repaint();
                        return;
                    }
                    let tela = ctx.content_rect();
                    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                    pagina.set_clip_rect(tela);
                    let p = tema::paleta(&pagina);
                    pintura::pintar_fundo(pagina.painter(), p, tela);
                    // o fundo do documento (`--c`), onde a dock fica
                    let doc = tela.shrink(12.0);
                    pagina.painter().rect_filled(doc, pintura::RAIO_PADRAO, p.estudio.doc);
                    let largo = egui::Rect::from_min_size(doc.min + egui::vec2(20.0, 20.0), egui::vec2(776.0, 120.0));
                    let mut ui_largo = pagina.new_child(UiBuilder::new().id_salt("larga").max_rect(largo));
                    dock(&mut ui_largo, &itens("Linhas"));
                    let estreito =
                        egui::Rect::from_min_size(doc.min + egui::vec2(20.0, 170.0), egui::vec2(578.0, 240.0));
                    let mut ui_estreito = pagina.new_child(UiBuilder::new().id_salt("estreita").max_rect(estreito));
                    dock(&mut ui_estreito, &itens("Linhas (contadas)"));
                },
                tema_escolhido,
            );
        let inicio = std::time::Instant::now();
        while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            h.step();
        }
        h.run_steps(4);
        // a ficha da dock de cima em hover
        h.query_all_by_label("7 Tabelas").next().expect("a ficha").hover();
        h.run_steps(3);
        let imagem = h.render().expect("renderizar com wgpu");
        imagem.save(pasta_capturas().join(nome)).expect("gravar png");
    }
}

/// Parada 1: os ícones (a página dos ícones e a casca) a 100%, 150% e 200% nos dois temas, para conferir a nitidez
/// (o PNG pelos pixels, preso aos pixels) e o desenho pelos pontos.
#[test]
fn icones_a_100_150_e_200() {
    for (tema_escolhido, nome_tema) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        for (escala, nome_escala) in [(1.0, "100"), (1.5, "150"), (2.0, "200")] {
            capturar_em(
                tema_escolhido,
                &format!("mostruario-icones-{nome_tema}-{nome_escala}.png"),
                egui::vec2(1100.0, 900.0),
                escala,
                |m| m.mostrar_icones(true),
                |_| {},
            );
            capturar_em(
                tema_escolhido,
                &format!("mostruario-casca-{nome_tema}-{nome_escala}.png"),
                egui::vec2(1440.0, 900.0),
                escala,
                |m| m.mostrar_casca(true),
                |_| {},
            );
        }
    }
    // o menu Arquivo com os sólidos de 24 e os recentes, a 150%
    capturar_em(
        Tema::Escuro,
        "mostruario-casca-arquivo-escuro-150.png",
        egui::vec2(1440.0, 900.0),
        1.5,
        |m| m.mostrar_casca(true),
        |h| {
            h.get_by_role_and_label(egui::accesskit::Role::Button, "Arquivo").click();
            h.run_steps(2);
            h.get_by_label("Abertos recentemente").hover();
            h.run_steps(2);
        },
    );
}

/// A faixa das planilhas (prancheta 17, cenas a e b) nos dois temas: quatro planilhas com a Vendas ativa e a Custos oculta
/// (capturas `folhas-{claro,escuro}`); e numa janela estreita, doze planilhas rolando, com ‹ › ligados e a lista aberta
/// (`folhas-estreita-escuro`).
#[test]
fn folhas_claro_e_escuro() {
    use interface::mostruario::planilha::PlanilhaExemplo;
    use interface::pecas::folhas::ROTULO_FAIXA;
    struct Estado {
        tema: Tema,
        planilha: PlanilhaExemplo,
        muitas: bool,
    }
    let variantes = [
        (Tema::Claro, "folhas-claro", false, egui::vec2(700.0, 120.0)),
        (Tema::Escuro, "folhas-escuro", false, egui::vec2(700.0, 120.0)),
        (Tema::Escuro, "folhas-estreita-escuro", true, egui::vec2(700.0, 440.0)),
    ];
    for (tema_escolhido, nome, muitas, janela) in variantes {
        let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = Harness::builder()
            .with_size(janela)
            .with_pixels_per_point(1.0)
            .with_theme(tema_egui)
            .with_max_steps(12)
            .wgpu()
            .build_ui_state(
                |ui, s: &mut Estado| {
                    let ctx = ui.ctx().clone();
                    tema::aplicar(&ctx, s.tema);
                    ctx.all_styles_mut(|st| st.animation_time = 0.0);
                    tema::definir_movimento_reduzido(&ctx, true);
                    if !fontes::instaladas(&ctx) {
                        ctx.request_repaint();
                        return;
                    }
                    let tela = ctx.content_rect();
                    let mut pagina = ui.new_child(UiBuilder::new().max_rect(tela));
                    pagina.set_clip_rect(tela);
                    let p = tema::paleta(&pagina);
                    pintura::pintar_fundo(pagina.painter(), p, tela);
                    // o documento com a faixa no pé (a grade fica acima, fora da captura)
                    let doc = egui::Rect::from_min_max(
                        egui::pos2(16.0, tela.bottom() - 16.0 - 40.0 - 34.0),
                        egui::pos2(tela.right() - 16.0, tela.bottom() - 16.0),
                    );
                    pagina.painter().rect_filled(doc, pintura::RAIO_PADRAO, p.estudio.doc);
                    let faixa =
                        egui::Rect::from_min_size(doc.min + egui::vec2(1.0, 8.0), egui::vec2(doc.width() - 2.0, 32.0));
                    let mut filho = pagina.new_child(UiBuilder::new().id_salt("folhas").max_rect(faixa));
                    s.planilha.mostrar_folhas(&mut filho, s.muitas);
                },
                Estado { tema: tema_escolhido, planilha: PlanilhaExemplo::new("planilha"), muitas },
            );
        let inicio = std::time::Instant::now();
        while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            h.step();
        }
        h.run_steps(6);
        if muitas {
            h.get_by_label("Rolar as planilhas para a direita").click();
            h.run_steps(3);
            h.get_by_role_and_label(egui::accesskit::Role::Button, ROTULO_FAIXA).click();
            h.run_steps(4);
        }
        let imagem = h.render().expect("renderizar com wgpu");
        imagem.save(pasta_capturas().join(format!("{nome}.png"))).expect("gravar png");
    }
}
