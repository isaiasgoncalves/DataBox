//! O menu Ajuda › Sobre o DataBox: as informações, as licenças, os links das pastas, o renderizador.

use super::*;

/// O link das preferências no Sobre: `%APPDATA%\DataBox` no Windows; nos outros sistemas, a pasta de verdade.
fn rotulo_preferencias(amb: &Ambiente) -> String {
    let pasta = amb.servico.prefs().arquivo().parent().map(|p| p.display().to_string()).unwrap_or_default();
    if cfg!(windows) { r"%APPDATA%\DataBox".to_string() } else { pasta }
}

/// O link do arquivo de log no Sobre: `%LOCALAPPDATA%\DataBox\databox.log` no Windows; nos outros, o caminho real.
fn rotulo_log() -> String {
    if cfg!(windows) {
        r"%LOCALAPPDATA%\DataBox\databox.log".to_string()
    } else {
        servico::arquivo_log().display().to_string()
    }
}

#[test]
fn ajuda_sobre_o_databox() {
    let amb = Ambiente::exemplo("sobre");
    let mut h = harness(&amb);
    abrir_menu(&mut h, "Ajuda");
    clicar_item(&mut h, "Sobre o DataBox");
    h.run_steps(2);
    assert!(h.state().janela_aberta());
    assert!(existe(&h, Role::Dialog, "Sobre o DataBox"));
    // o logo e o nome, a versão, a descrição e a ficha, sem Consolas
    let app = h.state().estado().app;
    let versao = format!("Versão {}", app.versao);
    assert_eq!(versao, "Versão 0.2.3", "a versão do aplicativo");
    let sqlite = format!("{}, embutido", app.sqlite_versao);
    let (preferencias, log) = (rotulo_preferencias(&amb), rotulo_log());
    for texto in [
        "DataBox",
        versao.as_str(),
        "Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.",
        "Compilação",
        "Renderizador",
        "OpenGL (glow)",
        "SQLite",
        sqlite.as_str(),
        "Sistema",
        "Licença",
        "MIT",
        "Preferências",
        preferencias.as_str(),
        "Arquivo de log",
        log.as_str(),
        "Desenvolvido por Isaías Gonçalves. Feito em Rust, com egui e SQLite.",
    ] {
        assert!(tem_texto_exato(&h, texto), "{texto} na janela Sobre: {:?}", textos(&h));
    }
    // o "Sistema" vem do `std::env::consts` e a "Compilação" é "data · hash" (ou "desconhecida")
    let sistema = if cfg!(windows) { "Windows · x64" } else { "" };
    if cfg!(all(windows, target_arch = "x86_64")) {
        assert!(tem_texto_exato(&h, sistema), "{:?}", textos(&h));
    }
    assert!(tem_texto(&h, " · ") || tem_texto_exato(&h, "desconhecida"));
    for botao in ["Licenças de terceiros", "Copiar informações", "Fechar"] {
        assert!(existe(&h, Role::Button, botao), "{botao}");
    }
    assert!(existe(&h, Role::Link, "MIT") && existe(&h, Role::Link, &preferencias));
    capturar(&mut h, "janela-sobre-claro.png");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
    assert!(botao_menu(&h, "Ajuda").is_focused(), "o foco volta ao botão Ajuda");
}

/// Os dois temas e as duas larguras da janela Sobre, para comparar com a prancheta 13.
#[test]
fn capturas_do_sobre() {
    for (tema, tamanho, nome) in [
        (Tema::Escuro, TAMANHO, "sobre-escuro.png"),
        (Tema::Claro, TAMANHO, "sobre-claro.png"),
        (Tema::Escuro, egui::vec2(960.0, 700.0), "sobre-escuro-960.png"),
        (Tema::Claro, egui::vec2(960.0, 700.0), "sobre-claro-960.png"),
    ] {
        let amb = Ambiente::exemplo("sobre_capturas");
        let mut h = harness_com(&amb, tema, tamanho, Partida::default(), |_| None);
        abrir_menu(&mut h, "Ajuda");
        clicar_item(&mut h, "Sobre o DataBox");
        h.run_steps(3);
        capturar(&mut h, nome);
    }
}

fn abrir_o_sobre(h: &mut H) {
    abrir_menu(h, "Ajuda");
    clicar_item(h, "Sobre o DataBox");
    h.run_steps(2);
    assert!(existe(h, Role::Dialog, "Sobre o DataBox"));
}

/// O que foi copiado nos próximos quadros.
fn texto_copiado(h: &mut H) -> Option<String> {
    let mut copiado = None;
    for _ in 0..3 {
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

#[test]
fn copiar_informacoes() {
    let amb = Ambiente::exemplo("sobre_copiar");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    h.get(by().role(Role::Button).label("Copiar informações")).click();
    let copiado = texto_copiado(&mut h).expect("a ficha foi copiada");
    let app = h.state().estado().app;
    let linhas: Vec<&str> = copiado.lines().collect();
    assert_eq!(linhas[0], format!("DataBox {}", app.versao));
    assert!(linhas[1].starts_with("Compilação: "), "{copiado}");
    for esperada in [
        "Renderizador: OpenGL (glow)".to_string(),
        format!("SQLite: {}, embutido", app.sqlite_versao),
        "Licença: MIT".to_string(),
        r"Preferências: %APPDATA%\DataBox".to_string(),
        r"Arquivo de log: %LOCALAPPDATA%\DataBox\databox.log".to_string(),
    ] {
        assert!(linhas.contains(&esperada.as_str()), "{esperada}: {copiado}");
    }
    assert!(linhas.iter().any(|l| l.starts_with("Sistema: ")));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Informações copiadas."));
    assert!(existe(&h, Role::Dialog, "Sobre o DataBox"), "o Sobre continua aberto");
}

#[test]
fn licenca_mostra_o_texto_mit() {
    let amb = Ambiente::exemplo("sobre_licenca");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    clicar_link(&mut h, "MIT");
    h.run_steps(2);
    assert!(existe(&h, Role::Dialog, "Licença") && existe(&h, Role::Dialog, "Sobre o DataBox"));
    assert!(tem_texto(&h, "MIT License"), "{:?}", textos(&h));
    assert!(tem_texto(&h, "Permission is hereby granted, free of charge"), "{:?}", textos(&h));
    capturar(&mut h, "sobre-licenca-claro.png");
    // o Esc fecha só a de cima
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!existe(&h, Role::Dialog, "Licença") && existe(&h, Role::Dialog, "Sobre o DataBox"));
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().janela_aberta());
}

#[test]
fn licencas_de_terceiros_lista_as_bibliotecas() {
    let amb = Ambiente::exemplo("sobre_terceiros");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    clicar_botao(&mut h, "Licenças de terceiros");
    h.run_steps(2);
    assert!(existe(&h, Role::Dialog, "Licenças de terceiros"));
    for texto in
        ["Bibliotecas do projeto", "egui", "eframe", "egui_extras", "rusqlite", "chrono", "rfd", "zip", "image"]
    {
        assert!(tem_texto_exato(&h, texto), "{texto}: {:?}", textos(&h));
    }
    // as versões de verdade vêm do Cargo.lock; o egui e o eframe são os da 0.36
    assert!(tem_texto_exato(&h, "0.36.2"));
    assert!(tem_texto_exato(&h, "MIT OR Apache-2.0"));
    assert!(!tem_texto_exato(&h, "egui_kittest"), "as de teste ficam de fora");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!existe(&h, Role::Dialog, "Licenças de terceiros") && existe(&h, Role::Dialog, "Sobre o DataBox"));
}

/// A lista que o build.rs fez: "nome\tversão\tlicença\tdireta".
const TERCEIROS: &str = include_str!(concat!(env!("OUT_DIR"), "/terceiros.txt"));

/// O texto de um nó: o rótulo ou, nos textos soltos, o valor.
fn texto_do_no(n: &egui_kittest::Node<'_>) -> String {
    n.accesskit_node().label().or_else(|| n.accesskit_node().value()).unwrap_or_default()
}

/// Os rótulos de texto da lista de terceiros que estão na árvore (nome, retângulo), do alto da lista para baixo.
fn linhas_da_lista(h: &H, topo: f32, base: f32) -> Vec<(String, egui::Rect)> {
    // só os textos da própria lista (atrás da janela há o Sobre e o resto do programa, também na árvore)
    let da_lista: std::collections::HashSet<&str> = TERCEIROS
        .lines()
        .flat_map(|l| l.split('\t').take(3))
        .chain(["Bibliotecas do projeto", "Dependências delas", "não informada"])
        .collect();
    let mut v: Vec<(String, egui::Rect)> = h
        .query_all_by(|n| n.role() == Role::Label && n.bounding_box().is_some())
        .map(|n| (texto_do_no(&n), n.rect()))
        .filter(|(t, r)| {
            da_lista.contains(t.as_str())
                && r.height() <= 24.0
                && r.width() > 0.0
                && r.center().y > topo
                && r.center().y < base
        })
        .collect();
    v.sort_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y).then(a.1.left().total_cmp(&b.1.left())));
    v
}

#[test]
fn licencas_de_terceiros_sem_cortes_e_rolam_ate_o_fim() {
    let ultima = TERCEIROS
        .lines()
        .rfind(|l| l.split('\t').nth(3) == Some("0"))
        .and_then(|l| l.split('\t').next())
        .expect("há dependências indiretas");
    for (tema, sufixo) in [(Tema::Claro, "claro"), (Tema::Escuro, "escuro")] {
        let amb = Ambiente::exemplo("sobre_terceiros_rolagem");
        let mut h = harness_com(&amb, tema, TAMANHO, Partida::default(), |_| None);
        abrir_o_sobre(&mut h);
        clicar_botao(&mut h, "Licenças de terceiros");
        h.run_steps(3);
        // a janela tem 620 px de largura, centrada (o nó do diálogo não traz retângulo)
        let janela_direita = TAMANHO.x / 2.0 + 310.0;
        let titulo = h.get_by_label("Bibliotecas do projeto").rect();
        let (topo, base) = (titulo.top() - 1.0, titulo.top() + 381.0);
        // a lista inteira cabe na janela, sem uma coluna por cima da outra
        let confere = |h: &H, onde: &str| {
            let linhas = linhas_da_lista(h, topo, base);
            assert!(linhas.len() > 12, "{onde}: {linhas:?}");
            for (texto, r) in &linhas {
                assert!(r.right() <= janela_direita - 20.0, "{onde}: {texto} passa do corpo: {r:?} {janela_direita}");
            }
            for par in linhas.windows(2) {
                let ((a, ra), (b, rb)) = (&par[0], &par[1]);
                if (ra.center().y - rb.center().y).abs() < 1.0 {
                    assert!(ra.right() <= rb.left(), "{onde}: {a} invade {b}: {ra:?} {rb:?}");
                }
            }
        };
        confere(&h, "no alto");
        assert!(tem_texto_exato(&h, "Bibliotecas do projeto"));
        assert!(!tem_texto_exato(&h, ultima), "a última ainda não devia estar à vista");
        capturar(&mut h, &format!("sobre-terceiros-{sufixo}.png"));

        // a roda do mouse sobre a lista, até o fim
        let sobre_a_lista = titulo.left_top() + egui::vec2(60.0, 60.0);
        h.hover_at(sobre_a_lista);
        let mut dica_conferida = false;
        for passo in 0..120 {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -150.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            });
            h.run_steps(2);
            // em todo o caminho, nada passa da borda nem fica por cima do vizinho
            if passo % 6 == 0 {
                confere(&h, &format!("rolando, passo {passo}"));
            }
            // a licença cortada com "…" guarda o texto inteiro na dica
            if !dica_conferida {
                let cortada = linhas_da_lista(&h, topo, base).into_iter().find(|(t, r)| {
                    r.left() > 760.0 && t.len() > 40 && r.right() > janela_direita - 40.0 && r.center().y < base - 12.0
                });
                if let Some((texto, _)) = cortada {
                    // a rolagem é suavizada: espera parar e acha a linha de novo antes de pôr o ponteiro nela
                    h.run_steps(10);
                    let Some((_, r)) = linhas_da_lista(&h, topo, base).into_iter().find(|(t, _)| *t == texto) else {
                        continue;
                    };
                    h.hover_at(r.center());
                    h.run_steps(3);
                    let n = textos(&h).iter().filter(|t| **t == texto).count();
                    assert!(n >= 2, "a licença cortada ({texto}) devia ter dica com o texto inteiro: {n}");
                    dica_conferida = true;
                    h.hover_at(sobre_a_lista);
                    h.run_steps(2);
                }
            }
        }
        h.run_steps(3);
        if TERCEIROS.lines().any(|l| l.split('\t').nth(2).is_some_and(|x| x.len() > 45)) {
            assert!(dica_conferida, "nenhuma licença longa foi cortada à vista");
        }
        assert!(tem_texto_exato(&h, ultima), "a última ({ultima}) não foi desenhada: {:?}", textos(&h));
        let r = h
            .query_all_by(|n| n.label().as_deref() == Some(ultima) || n.value().as_deref() == Some(ultima))
            .map(|n| n.rect())
            .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
            .expect("a última");
        assert!(r.top() >= topo && r.bottom() <= base, "a última linha está à vista: {r:?} em {topo}..{base}");
        // sem espaço vazio embaixo: a última linha encosta no fim da área
        assert!(base - r.bottom() < 30.0, "linhas faltando no fim: sobram {} px", base - r.bottom());
        confere(&h, "no fim");
        capturar(&mut h, &format!("sobre-terceiros-fim-{sufixo}.png"));
    }
}

#[test]
fn links_das_pastas_revelam() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let amb = Ambiente::exemplo("sobre_links");
    let abertos: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let anotados = Rc::clone(&abertos);
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(|_| None)
        .com_abrir_de_teste(move |p| anotados.borrow_mut().push(p.to_path_buf()));
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    abrir_o_sobre(&mut h);
    clicar_link(&mut h, &rotulo_preferencias(&amb));
    clicar_link(&mut h, &rotulo_log());
    let pasta = amb.servico.prefs().arquivo().parent().map(std::path::Path::to_path_buf).expect("a pasta");
    assert_eq!(abertos.borrow().as_slice(), [pasta, servico::arquivo_log()]);
}

#[test]
fn arquivo_de_log_no_rotulo() {
    let amb = Ambiente::exemplo("sobre_log");
    let mut h = harness(&amb);
    abrir_o_sobre(&mut h);
    // o rótulo é "Arquivo de log" (não "Registro"), e a dica do link leva o caminho de verdade
    assert!(tem_texto_exato(&h, "Arquivo de log") && !tem_texto_exato(&h, "Registro"));
    h.get(by().role(Role::Link).label(&rotulo_log())).hover();
    h.run_steps(4);
    let log = servico::arquivo_log().display().to_string();
    assert!(tem_texto(&h, &format!("Abrir o arquivo: {log}")), "{:?}", textos(&h));
}

#[test]
fn sobre_mostra_o_renderizador_que_abriu_a_janela() {
    let amb = Ambiente::exemplo("sobre_wgpu");
    let editor = editor::Editor::novo(std::sync::Arc::clone(&amb.servico), Tema::Claro, Partida::default())
        .com_dialogo(|_| None)
        .com_renderizador(editor::linha_comando::Renderizador::Wgpu);
    let mut h = harness_editor(editor, Tema::Claro, TAMANHO);
    abrir_o_sobre(&mut h);
    assert!(tem_texto_exato(&h, "wgpu") && !tem_texto_exato(&h, "OpenGL (glow)"));
}
