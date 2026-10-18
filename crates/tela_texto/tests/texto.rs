//! O editor de texto no `egui_kittest`: abrir TXT, editar, salvar, desfazer e refazer, o leitor de
//! arquivo grande, as falhas (32 MiB, UTF-16), o Atualizar e o aviso de que o arquivo mudou no disco. A tela é desenhada
//! como a casca desenha (`palco_documento`: a barra com os verbos, o documento com a faixa, o conteúdo e o rodapé, e as
//! Propriedades à direita). As capturas `texto-*` vão para `capturas/rust/`.

mod apoio;

use apoio::*;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use estado::{Documento, Verbo};
use interface::pecas::toast::{icone_toast, toast_atual};
use interface::tema::Tema;
use tela_texto::{Modo, PedidoDeArquivo, TipoDeTexto};

fn ofertas(t: &H) -> estado::Ofertas {
    let amb_estado = &t.state().estado;
    t.state().tela.doc.ofertas(amb_estado)
}

fn grande(linhas: usize) -> Vec<u8> {
    let mut s = String::new();
    for i in 0..linhas {
        s.push_str(&format!("linha {i}: um texto comprido o bastante para encher o arquivo e passar do limite\n"));
    }
    s.into_bytes()
}

#[test]
fn abre_txt_e_mostra_o_texto() {
    let t = abrir("abre_txt", "notas.txt", "primeira linha\nsegunda linha com ação\n".as_bytes());
    let doc = &t.state().tela.doc;
    assert_eq!(doc.modo(), Modo::Editor);
    assert_eq!(doc.tipo(), TipoDeTexto::Texto);
    assert_eq!(doc.texto(), Some("primeira linha\nsegunda linha com ação\n"));
    assert!(!doc.alterado() && !doc.salvando());
    // a faixa só com a folha e o nome (sem estado e sem Formatar), o rodapé e as Propriedades do texto
    assert!(t.query_by_label("notas.txt").is_some());
    assert!(t.query_by_label("Texto de notas.txt").is_some());
    assert!(t.query_by_label("2 linhas").is_some());
    assert!(t.query_by_label("Linha 1, coluna 1").is_some());
    assert!(t.query_by_label("Formatar").is_none() && t.query_by_label("JSON válido").is_none());
    assert!(t.query_by_label("Seção Texto").is_some() && t.query_by_label("Seção Arquivo").is_some());
    assert!(t.query_by_label("UTF-8").is_some() && t.query_by_label("LF").is_some());
}

#[test]
fn texto_utf8_com_bom_volta_com_bom() {
    let mut original = vec![0xEF, 0xBB, 0xBF];
    original.extend_from_slice("ação\r\nsegunda\r\n".as_bytes());
    let mut t = abrir("bom", "com_bom.txt", &original);
    assert_eq!(t.state().tela.doc.texto(), Some("ação\nsegunda\n"), "o editor trabalha com \\n e sem o BOM");
    assert!(t.query_by_label("UTF-8 com BOM").is_some());
    digitar(&mut t, "com_bom.txt", "X");
    assert!(t.state().tela.doc.alterado());
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    let gravado = std::fs::read(&t.caminho).expect("ler o arquivo");
    let mut esperado = vec![0xEF, 0xBB, 0xBF];
    esperado.extend_from_slice("Xação\r\nsegunda\r\n".as_bytes());
    assert_eq!(gravado, esperado, "o BOM e o CRLF voltam");
}

#[test]
fn windows_1252_volta_em_windows_1252() {
    // "não" em Windows-1252: o ã é o byte E3
    let mut t = abrir("w1252", "antigo.txt", b"n\xE3o\r\nfim");
    assert_eq!(t.state().tela.doc.texto(), Some("não\nfim"));
    assert!(t.query_by_label("Windows-1252").is_some());
    digitar(&mut t, "antigo.txt", "é");
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    assert_eq!(std::fs::read(&t.caminho).expect("ler"), b"\xE9n\xE3o\r\nfim");
}

#[test]
fn editar_marca_alterado_e_o_ponto() {
    let mut t = abrir("editar_marca", "notas.txt", b"abc\n");
    assert!(!ofertas(&t).ligado(Verbo::Salvar), "sem alteração, o Salvar está desligado");
    digitar(&mut t, "notas.txt", "x");
    let doc = &t.state().tela.doc;
    assert!(doc.alterado());
    assert_eq!(doc.alteracoes(), None, "o texto não conta as alterações");
    assert_eq!(doc.texto(), Some("xabc\n"));
    let status = doc.status_do_arquivo(&t.state().estado).expect("o status de arquivo");
    assert_eq!(status.situacao, estado::Situacao::Alterado);
    assert_eq!(status.nome, "notas.txt");
    assert_eq!(status.detalhes_em_linha(), "Texto · UTF-8 · LF");
    let o = ofertas(&t);
    assert!(o.ligado(Verbo::Salvar) && o.dica(Verbo::Salvar) == "Salvar notas.txt (Ctrl+S)");
    // o disco não mudou
    assert_eq!(std::fs::read(&t.caminho).expect("ler"), b"abc\n");
}

#[test]
fn salvar_grava_e_limpa_o_ponto() {
    let mut t = abrir("salvar_grava", "notas.txt", b"abc\n");
    digitar(&mut t, "notas.txt", "x");
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    assert_eq!(std::fs::read(&t.caminho).expect("ler"), b"xabc\n");
    assert_eq!(toast_atual(&t.ctx).as_deref(), Some("notas.txt salvo."));
    assert!(!t.state().tela.doc.alterado());
    let status = t.state().tela.doc.status_do_arquivo(&t.state().estado).expect("status");
    assert_eq!(status.situacao, estado::Situacao::Salvo);
    assert!(!ofertas(&t).ligado(Verbo::Salvar));
    // o tamanho e a data das Propriedades acompanham a gravação
    assert!(t.query_by_label("5 bytes").is_some());
}

#[test]
fn desfazer_ate_o_salvo_tira_o_ponto() {
    let mut t = abrir("desfazer_salvo", "notas.txt", b"abc\n");
    digitar(&mut t, "notas.txt", "x");
    assert!(t.state().tela.doc.alterado());
    clicar(&mut t, "Desfazer");
    assert_eq!(t.state().tela.doc.texto(), Some("abc\n"));
    assert!(!t.state().tela.doc.alterado(), "voltou ao texto salvo");
}

#[test]
fn desfazer_e_refazer_pelo_undoer() {
    let mut t = abrir("desfazer_refazer", "notas.txt", b"abc\n");
    assert!(!ofertas(&t).ligado(Verbo::Desfazer) && !ofertas(&t).ligado(Verbo::Refazer));
    digitar(&mut t, "notas.txt", "xyz");
    assert_eq!(texto(&t), "xyzabc\n");
    assert!(ofertas(&t).ligado(Verbo::Desfazer) && !ofertas(&t).ligado(Verbo::Refazer));
    // o Desfazer da barra
    clicar(&mut t, "Desfazer");
    assert_eq!(texto(&t), "abc\n");
    assert!(!ofertas(&t).ligado(Verbo::Desfazer) && ofertas(&t).ligado(Verbo::Refazer));
    clicar(&mut t, "Refazer");
    assert_eq!(texto(&t), "xyzabc\n");
    assert!(ofertas(&t).ligado(Verbo::Desfazer) && !ofertas(&t).ligado(Verbo::Refazer));
    // o Ctrl+Z e o Ctrl+Y com o foco no campo são do egui, pelo mesmo desfazer
    t.get_by_label("Texto de notas.txt").focus();
    t.run_steps(2);
    tecla(&mut t, Modifiers::COMMAND, Key::Z);
    assert_eq!(texto(&t), "abc\n", "{:?}", texto(&t));
    tecla(&mut t, Modifiers::COMMAND, Key::Y);
    assert_eq!(texto(&t), "xyzabc\n");
}

#[test]
fn oferta_de_desfazer_so_muda_quando_o_texto_muda() {
    let mut t = abrir("oferta_desfazer", "notas.txt", b"abc\n");
    let antes = t.state().tela.doc.recalculos_da_oferta();
    // quadros parados, e o cursor andando sem mudar o texto: nada é recalculado
    t.run_steps(6);
    t.get_by_label("Texto de notas.txt").focus();
    t.run_steps(2);
    tecla(&mut t, Modifiers::NONE, Key::ArrowRight);
    tecla(&mut t, Modifiers::NONE, Key::ArrowRight);
    assert_eq!(t.state().tela.doc.recalculos_da_oferta(), antes, "sem mudar o texto, a oferta não se refaz");
    assert!(!ofertas(&t).ligado(Verbo::Desfazer));
    // uma tecla que muda o texto: recalcula, e o Desfazer liga
    t.get_by_label("Texto de notas.txt").type_text("q");
    t.run_steps(2);
    let depois = t.state().tela.doc.recalculos_da_oferta();
    assert!(depois > antes && depois <= antes + 2, "{antes} -> {depois}");
    assert!(ofertas(&t).ligado(Verbo::Desfazer));
    t.run_steps(6);
    assert_eq!(t.state().tela.doc.recalculos_da_oferta(), depois, "quadros parados não recalculam");
}

#[test]
fn max_undos_cai_com_o_tamanho() {
    use egui::text::{CCursor, CCursorRange};
    use egui::text_edit::TextEditState;
    let pontos = |t: &H| -> String {
        let id = t.state().tela.doc.id_do_campo();
        let estado = TextEditState::load(&t.ctx, id).expect("o estado do campo");
        let mut u = estado.undoer();
        let c = CCursorRange::one(CCursor::new(0));
        for i in 0..300 {
            u.add_undo(&(c, format!("{i}")));
        }
        format!("{u:?}")
    };
    // texto pequeno: o máximo, 100 passos
    let pequeno = abrir("undos_pequeno", "a.txt", b"abc\n");
    assert!(pontos(&pequeno).contains("undo count: 100"), "{}", pontos(&pequeno));
    // perto do limite da edição (0,5 MiB): 32 MiB / 500 KB
    let quase_meio_mib = vec![b'a'; 500_000];
    let medio = abrir("undos_medio", "b.txt", &quase_meio_mib);
    assert_eq!(medio.state().tela.doc.modo(), Modo::Editor);
    assert!(pontos(&medio).contains("undo count: 67"), "{}", pontos(&medio));
    assert_eq!(texto::limites::max_undos(500_000), 67);
}

#[test]
fn arquivo_acima_de_1_mib_abre_no_leitor_so_leitura() {
    let t = abrir("leitor_so_leitura", "grande.log", &grande(14_000));
    let doc = &t.state().tela.doc;
    assert_eq!(doc.modo(), Modo::Leitor);
    assert!(doc.texto().is_some_and(|x| x.len() > 1_048_576));
    // nada de campo de texto, e nenhum verbo de edição
    assert!(t.query_by_label("Texto de grande.log").is_none());
    let o = ofertas(&t);
    assert!(!o.ligado(Verbo::Salvar) && !o.ligado(Verbo::Desfazer) && !o.ligado(Verbo::Refazer));
    assert!(o.ligado(Verbo::Atualizar));
    assert!(!doc.alterado());
    let status = doc.status_do_arquivo(&t.state().estado).expect("status");
    assert_eq!(status.situacao, estado::Situacao::SoLeitura);
    // a faixa com a etiqueta e o aviso de topo com os botões da ficha
    assert!(t.query_by_label_contains("Somente leitura").is_some());
    assert!(t.query_by_label_contains("e abre só para leitura. Para editar, use um programa de texto.").is_some());
    assert!(t.query_by_label("Abrir no programa padrão").is_some() && t.query_by_label("Mostrar na pasta").is_some());
    assert!(t.query_by_label("14.000 linhas").is_some());
}

#[test]
fn botoes_do_leitor_pedem_o_programa_e_a_pasta() {
    let mut t = abrir("leitor_botoes", "grande.log", &grande(14_000));
    clicar(&mut t, "Abrir no programa padrão");
    let caminho = t.caminho.clone();
    assert_eq!(t.state_mut().tela.doc.tomar_pedido(), Some(PedidoDeArquivo::AbrirNoPadrao(caminho.clone())));
    assert_eq!(t.state_mut().tela.doc.tomar_pedido(), None, "o pedido é tomado uma vez");
    clicar(&mut t, "Mostrar na pasta");
    assert_eq!(t.state_mut().tela.doc.tomar_pedido(), Some(PedidoDeArquivo::MostrarNaPasta(caminho)));
}

#[test]
fn acima_de_32_mib_mostra_a_ficha_com_a_frase() {
    // 33 MiB (arquivo esparso: só o tamanho importa para a recusa)
    let mut t = abrir_criando("acima_de_32", "enorme.txt", Tema::Claro, TAMANHO, |caminho| {
        let f = std::fs::File::create(caminho).expect("criar");
        f.set_len(33 * 1_048_576).expect("tamanho");
    });
    assert_eq!(t.state().tela.doc.modo(), Modo::Falhou);
    assert_eq!(
        t.state().tela.doc.falha(),
        Some("Este arquivo é grande demais para o DataBox mostrar (33 MB). Abra no programa padrão.")
    );
    assert!(t.query_by_label_contains("grande demais para o DataBox mostrar (33 MB)").is_some());
    // os botões da ficha estão ligados e pedem o programa
    clicar(&mut t, "Abrir no programa padrão");
    assert!(matches!(t.state_mut().tela.doc.tomar_pedido(), Some(PedidoDeArquivo::AbrirNoPadrao(_))));
    assert!(!ofertas(&t).ligado(Verbo::Salvar));
}

#[test]
fn arquivo_que_sumiu_mostra_o_erro_e_desliga_os_botoes() {
    let mut t = abrir_criando("sumiu", "nao_existe.txt", Tema::Claro, TAMANHO, |_| {});
    assert_eq!(t.state().tela.doc.modo(), Modo::Falhou);
    assert!(t.state().tela.doc.falha().is_some_and(|f| f.contains("O arquivo não existe")));
    clicar(&mut t, "Abrir no programa padrão");
    assert_eq!(t.state_mut().tela.doc.tomar_pedido(), None, "o botão está desligado");
}

#[test]
fn sem_cores_acima_de_200_kb_diz_no_rodape() {
    // um JSON válido de uns 250 KB: o editor abre (abaixo de 1 MiB), mas sem as cores
    let mut json = String::from("[\n");
    for i in 0..5_000 {
        json.push_str(&format!("  {{\"id\": {i}, \"nome\": \"item numero {i} com texto de enchimento\"}},\n"));
    }
    json.push_str("  null\n]\n");
    assert!(json.len() > 200_000 && json.len() < 1_048_576);
    let t = abrir("sem_cores", "grande.json", json.as_bytes());
    assert_eq!(t.state().tela.doc.modo(), Modo::Editor);
    assert!(t.query_by_label("· Sem cores acima de 200 KB").is_some());
    // o JSON pequeno não diz
    let t = abrir("com_cores", "pequeno.json", b"{\"a\": 1}\n");
    assert!(t.query_by_label("· Sem cores acima de 200 KB").is_none());
}

#[test]
fn o_leitor_so_desenha_as_linhas_visiveis() {
    let t = abrir("leitor_visiveis", "grande.log", &grande(14_000));
    let desenhadas = t.state().tela.doc.linhas_desenhadas();
    assert!(desenhadas.start == 0 && !desenhadas.is_empty(), "{desenhadas:?}");
    // 900 de altura, 20 por linha: umas 45 linhas (nunca as 14.000)
    assert!(desenhadas.len() < 80, "{desenhadas:?}");
}

#[test]
fn utf16_mostra_a_frase() {
    let mut utf16 = vec![0xFF, 0xFE];
    for u in "texto em utf16\n".encode_utf16() {
        utf16.extend_from_slice(&u.to_le_bytes());
    }
    let t = abrir("utf16", "dois_bytes.txt", &utf16);
    assert_eq!(t.state().tela.doc.modo(), Modo::Falhou);
    assert_eq!(
        t.state().tela.doc.falha(),
        Some(
            "O arquivo está em UTF-16, que o DataBox ainda não abre como texto. Salve-o como UTF-8 no programa de origem."
        )
    );
    assert!(t.query_by_label_contains("O arquivo está em UTF-16").is_some());
}

#[test]
fn atualizar_relê_e_troca_o_texto() {
    let mut t = abrir("atualizar", "notas.txt", b"antes\n");
    assert_eq!(texto(&t), "antes\n");
    std::fs::write(&t.caminho, "depois, com mais coisa\n").expect("regravar");
    // sem alteração pendente, o Atualizar lê direto (sem pergunta)
    clicar(&mut t, "Atualizar");
    esperar_ate(&mut t, |h| h.state().tela.doc.texto() == Some("depois, com mais coisa\n"));
    assert!(t.query_by_label("Descartar").is_none());
    assert!(!t.state().tela.doc.alterado());
}

#[test]
fn atualizar_com_alteracao_pergunta_antes_de_descartar() {
    let mut t = abrir("atualizar_pergunta", "notas.txt", b"antes\n");
    digitar(&mut t, "notas.txt", "x");
    std::fs::write(&t.caminho, "no disco\n").expect("regravar");
    clicar(&mut t, "Atualizar");
    assert!(t.query_by_label_contains("Descartar as alterações de notas.txt e ler o arquivo de novo?").is_some());
    // Cancelar: o texto editado continua
    clicar(&mut t, "Cancelar");
    assert_eq!(texto(&t), "xantes\n");
    assert!(t.state().tela.doc.alterado());
    // Descartar: lê o arquivo do disco
    clicar(&mut t, "Atualizar");
    clicar(&mut t, "Descartar");
    esperar_ate(&mut t, |h| h.state().tela.doc.texto() == Some("no disco\n"));
    assert!(!t.state().tela.doc.alterado());
    assert!(!ofertas(&t).ligado(Verbo::Desfazer), "o desfazer recomeça com o arquivo novo");
}

#[test]
fn arquivo_que_mudou_no_disco_pergunta_ao_salvar() {
    let mut t = abrir("mudou_no_disco", "notas.txt", b"antes\n");
    digitar(&mut t, "notas.txt", "x");
    // outro programa regrava o arquivo (o tamanho muda)
    std::fs::write(&t.caminho, "o trabalho de outro programa\n").expect("regravar");
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| h.query_by_label_contains("O arquivo mudou no disco desde que foi aberto.").is_some());
    assert!(t.state().tela.doc.pergunta_aberta() && !t.state().tela.doc.salvando());
    assert_eq!(
        std::fs::read_to_string(&t.caminho).expect("ler"),
        "o trabalho de outro programa\n",
        "nada foi gravado ainda"
    );
    // Cancelar deixa tudo como está
    clicar(&mut t, "Cancelar");
    assert!(t.state().tela.doc.alterado() && !t.state().tela.doc.pergunta_aberta());
    assert_eq!(std::fs::read_to_string(&t.caminho).expect("ler"), "o trabalho de outro programa\n");
    // Salvar mesmo assim substitui
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| h.query_by_label("Salvar mesmo assim").is_some());
    clicar(&mut t, "Salvar mesmo assim");
    esperar_ate(&mut t, |h| !h.state().tela.doc.salvando() && !h.state().tela.doc.alterado());
    assert_eq!(std::fs::read_to_string(&t.caminho).expect("ler"), "xantes\n");
}

#[test]
fn salvar_que_falha_guarda_o_erro_uma_vez_e_mantem_o_ponto() {
    // um caractere que o Windows-1252 não tem: a gravação recusa e nada é gravado
    let mut t = abrir("salvar_falha", "antigo.txt", b"n\xE3o\n");
    digitar(&mut t, "antigo.txt", "€€");
    // "€" existe em Windows-1252 (0x80); um ideograma não
    t.get_by_label("Texto de antigo.txt").type_text("日");
    t.run_steps(2);
    clicar(&mut t, "Salvar");
    esperar_ate(&mut t, |h| {
        h.state().tela.doc.status_do_arquivo(&h.state().estado).is_some_and(|s| s.situacao == estado::Situacao::ComErro)
    });
    assert_eq!(std::fs::read(&t.caminho).expect("ler"), b"n\xE3o\n", "nada foi gravado");
    assert!(t.state().tela.doc.alterado());
    let erro = t.state_mut().tela.doc.tomar_erro_de_gravacao().expect("o erro");
    assert!(erro.contains("não existe na codificação Windows-1252"), "{erro}");
    assert_eq!(t.state_mut().tela.doc.tomar_erro_de_gravacao(), None, "o erro vem uma vez só");
    assert_eq!(icone_toast(&t.ctx), Some(interface::icones::EstadoIcone::Erro));
}

#[test]
fn capturas_do_texto() {
    let mut t = abrir_com("captura_texto", "leia-me.txt", "Leia-me do projeto\n\nEste arquivo é só texto, sem cores.\nO editor mostra os números das linhas e a posição do cursor.\n".as_bytes(), Tema::Escuro, TAMANHO);
    digitar(&mut t, "leia-me.txt", "");
    let pasta = capturar(&mut t, "texto-escuro.png");
    assert!(pasta.is_file());
    let mut t = abrir_com(
        "captura_texto_claro",
        "leia-me.txt",
        "Leia-me do projeto\n\nEste arquivo é só texto, sem cores.\n".as_bytes(),
        Tema::Claro,
        TAMANHO,
    );
    assert!(capturar(&mut t, "texto-claro.png").is_file());
    let mut t = abrir_com("captura_leitor", "relatorio.log", &grande(14_000), Tema::Escuro, TAMANHO);
    assert!(capturar(&mut t, "texto-leitor-escuro.png").is_file());
    let mut t = abrir_com("captura_vazio", "vazio.txt", b"", Tema::Claro, TAMANHO);
    assert_eq!(t.state().tela.doc.modo(), Modo::Editor);
    assert!(capturar(&mut t, "texto-vazio-claro.png").is_file());
}
