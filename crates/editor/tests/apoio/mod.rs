//! Apoio dos testes da casca: o [`Editor`] inteiro num `egui_kittest::Harness` (wgpu, 1px = 1 ponto, sem
//! animações), sobre o `Ambiente` do estado (pasta temporária, preferências próprias, banco de exemplo), e as
//! capturas em `capturas/rust/`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use editor::{Editor, Partida, PedidoDialogo};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable, by};
use estado::apoio_testes::{Ambiente, pasta_capturas};
use interface::fontes;
use interface::tema::{self, Tema};

pub type H = Harness<'static, Editor>;

pub const TAMANHO: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// O Ctrl do Windows como o egui-winit entrega: `ctrl` e `command` juntos.
pub const CTRL: egui::Modifiers =
    egui::Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

fn quadro(ui: &mut egui::Ui, e: &mut Editor) {
    let ctx = ui.ctx().clone();
    ctx.all_styles_mut(|s| {
        s.animation_time = 0.0;
        s.visuals.text_cursor.blink = false;
        // as dicas (`title`) aparecem logo no hover, para os testes as lerem
        s.interaction.tooltip_delay = 0.0;
    });
    tema::definir_movimento_reduzido(&ctx, true);
    e.desenhar(ui, None);
}

/// O editor sobre o serviço do ambiente, com o diálogo falso, já com o estado carregado e as fontes do sistema.
pub fn harness_com(
    amb: &Ambiente,
    tema_escolhido: Tema,
    tamanho: egui::Vec2,
    partida: Partida,
    dialogo: impl FnMut(&PedidoDialogo) -> Option<PathBuf> + 'static,
) -> H {
    let editor = Editor::novo(Arc::clone(&amb.servico), tema_escolhido, partida).com_dialogo(dialogo);
    harness_editor(editor, tema_escolhido, tamanho)
}

/// O editor sem o diálogo do Windows (a janela "Pedir o caminho" no lugar dele), tema claro.
pub fn harness_sem_dialogo(amb: &Ambiente, tamanho: egui::Vec2) -> H {
    let editor = Editor::novo(Arc::clone(&amb.servico), Tema::Claro, Partida::default()).sem_dialogo_nativo();
    harness_editor(editor, Tema::Claro, tamanho)
}

/// Um editor já montado num harness, esperando as fontes e o estado.
pub fn harness_editor(editor: Editor, tema_escolhido: Tema, tamanho: egui::Vec2) -> H {
    harness_editor_com_passo(editor, tema_escolhido, tamanho, 0.25)
}

/// O [`harness_editor`] com outro passo de tempo por quadro (o padrão do kittest é 0,25 s: um clique duplo, com os
/// quadros do ponteiro entre os dois cliques, passaria do tempo dele).
pub fn harness_editor_com_passo(editor: Editor, tema_escolhido: Tema, tamanho: egui::Vec2, passo: f32) -> H {
    let tema_egui = if tema_escolhido == Tema::Escuro { egui::Theme::Dark } else { egui::Theme::Light };
    let mut h = Harness::builder()
        .with_step_dt(passo)
        .with_size(tamanho)
        .with_pixels_per_point(1.0)
        .with_theme(tema_egui)
        .with_max_steps(64)
        .wgpu()
        .build_ui_state(quadro, editor);
    // "Sistema" segue o tema que o harness diz ser o do sistema (sem ele, o egui cairia no escuro)
    h.ctx.options_mut(|o| o.fallback_theme = tema_egui);
    let inicio = Instant::now();
    while !fontes::do_sistema_carregadas(&h.ctx) && inicio.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(5));
        h.step();
    }
    esperar(&mut h, "o estado carregar", |h| h.state().carregado() && !h.state().ocupado());
    h
}

/// O editor com o tema claro, 1440×900, sem banco na linha de comando e sem diálogo (cancela).
pub fn harness(amb: &Ambiente) -> H {
    harness_com(amb, Tema::Claro, TAMANHO, Partida::default(), |_| None)
}

/// Roda quadros até `condicao` valer (as tarefas terminam em outras threads); falha em 15 s.
pub fn esperar(h: &mut H, o_que: &str, mut condicao: impl FnMut(&H) -> bool) {
    let inicio = Instant::now();
    loop {
        h.step();
        if condicao(h) {
            h.run_steps(3);
            return;
        }
        assert!(inicio.elapsed() < Duration::from_secs(15), "esperei 15 s por {o_que}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Espera as tarefas da casca e das telas terminarem.
pub fn assentar(h: &mut H) {
    esperar(h, "as tarefas terminarem", |h| !h.state().ocupado());
}

/// Quantos nós têm este rótulo (exato).
pub fn quantos(h: &H, rotulo: &str) -> usize {
    h.query_all_by_label(rotulo).count()
}

/// Algum nó contém este texto no rótulo ou no valor (o texto selecionável dos avisos e do `p.sub` vai em
/// nós `TextRun`, uma linha por nó).
pub fn tem_texto(h: &H, texto: &str) -> bool {
    let alvo = texto.to_string();
    h.query_all_by(move |n| {
        n.label().is_some_and(|l| l.contains(alvo.as_str())) || n.value().is_some_and(|v| v.contains(alvo.as_str()))
    })
    .next()
    .is_some()
}

/// Algum nó tem exatamente este texto no rótulo ou no valor.
pub fn tem_texto_exato(h: &H, texto: &str) -> bool {
    let alvo = texto.to_string();
    h.query_all_by(move |n| n.label().as_deref() == Some(alvo.as_str()) || n.value().as_deref() == Some(alvo.as_str()))
        .next()
        .is_some()
}

/// Todos os rótulos e valores da árvore (para a mensagem de um teste que falhou).
pub fn textos(h: &H) -> Vec<String> {
    h.query_all_by(|_| true)
        .filter_map(|n| n.accesskit_node().label().or_else(|| n.accesskit_node().value()))
        .filter(|t| !t.trim().is_empty())
        .collect()
}

/// Digita num campo pelo rótulo (o foco primeiro, como um clique nele).
pub fn digitar(h: &mut H, rotulo: &str, texto: &str) {
    let campo = h.get_by_label(rotulo);
    campo.focus();
    campo.type_text(texto);
    h.run_steps(2);
}

/// Espera aparecer um nó que contém o texto.
pub fn esperar_texto(h: &mut H, texto: &str) {
    let alvo = texto.to_string();
    esperar(h, &format!("o texto {texto:?}"), move |h| tem_texto(h, &alvo));
}

/// Clica no único nó com este papel e rótulo.
pub fn clicar(h: &mut H, papel: Role, rotulo: &str) {
    h.get(by().role(papel).label(rotulo)).click();
    h.run_steps(2);
}

/// Clica no botão com este rótulo.
pub fn clicar_botao(h: &mut H, rotulo: &str) {
    clicar(h, Role::Button, rotulo);
}

/// Clica no link com este rótulo (os nós do Explorador, os links das telas).
pub fn clicar_link(h: &mut H, rotulo: &str) {
    clicar(h, Role::Link, rotulo);
}

/// Há ao menos um nó com este papel e rótulo (para os rótulos que aparecem em mais de um lugar, como o nome do
/// banco).
pub fn existe(h: &H, papel: Role, rotulo: &str) -> bool {
    h.query_all(by().role(papel).label(rotulo)).next().is_some()
}

/// O nó com este papel e rótulo está ligado (o botão da barra de ícones, o nó marcado do Explorador).
pub fn ligado(h: &H, papel: Role, rotulo: &str) -> bool {
    h.get(by().role(papel).label(rotulo)).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

/// Passa o ponteiro sobre o nó com este papel e rótulo e devolve o texto da dica que apareceu (os textos que
/// surgiram na árvore com o ponteiro e somem quando ele sai, um por linha); tira o ponteiro no fim. O que chegou no
/// meio sem ser a dica (a página da grade que terminou de ler, "60 linhas" e as linhas) continua depois e não conta.
pub fn ler_dica(h: &mut H, papel: Role, rotulo: &str) -> String {
    h.remove_cursor();
    h.run_steps(2);
    let antes = textos(h);
    h.get(by().role(papel).label(rotulo)).hover();
    h.run_steps(3);
    let com_o_ponteiro = textos(h);
    h.remove_cursor();
    h.run_steps(2);
    let depois = textos(h);
    let novos: Vec<String> = com_o_ponteiro.into_iter().filter(|t| !antes.contains(t) && !depois.contains(t)).collect();
    novos.join("\n")
}

/// Clica no primeiro nó com este papel e rótulo (para os rótulos que aparecem em mais de um lugar: o da barra de
/// ferramentas vem antes do "+" da faixa de abas e do botão do Explorador vazio).
pub fn clicar_primeiro(h: &mut H, papel: Role, rotulo: &str) {
    h.query_all(by().role(papel).label(rotulo)).next().unwrap_or_else(|| panic!("nenhum {papel:?} {rotulo:?}")).click();
    h.run_steps(2);
}

/// Clica no primeiro botão ligado com este rótulo (o "Salvar" de uma janela, com o "Salvar" desligado da barra de
/// ferramentas por trás).
pub fn clicar_ligado(h: &mut H, rotulo: &str) {
    h.query_all(by().role(Role::Button).label(rotulo))
        .find(|n| !n.accesskit_node().is_disabled())
        .unwrap_or_else(|| panic!("nenhum botão ligado {rotulo:?}"))
        .click();
    h.run_steps(2);
}

/// Abre um menu da barra de título ("Arquivo", "Editar", "Exibir", "Ajuda"): o primeiro botão com o nome (os menus
/// vêm antes de tudo na árvore; a janela Exportar também tem um botão "Ajuda", o "i" dela). A seção "Arquivo" das
/// Propriedades não se confunde com o menu: o rótulo dela é "Seção Arquivo".
pub fn abrir_menu(h: &mut H, nome: &str) {
    clicar_primeiro(h, Role::Button, nome);
}

/// Abre o menu Arquivo.
pub fn abrir_menu_arquivo(h: &mut H) {
    abrir_menu(h, "Arquivo");
}

/// O botão de um menu da barra de título (o primeiro com o nome, como em [`abrir_menu`]).
pub fn botao_menu<'h>(h: &'h H, nome: &'h str) -> egui_kittest::Node<'h> {
    h.query_all(by().role(Role::Button).label(nome)).next().unwrap_or_else(|| panic!("o botão {nome}"))
}

/// O item do menu aberto com este rótulo (um botão, uma caixa de marcar ou uma opção): os itens têm a largura do
/// painel (de 198 a 288px), e a barra de ferramentas tem botões de ícone de 30px com os mesmos rótulos ("Abrir
/// arquivo", "Salvar", "Atualizar").
pub fn item_de_menu<'h>(h: &'h H, rotulo: &'h str) -> egui_kittest::Node<'h> {
    h.query_all_by_label(rotulo)
        .find(|n| {
            let papel = n.accesskit_node().role();
            matches!(papel, Role::Button | Role::CheckBox | Role::RadioButton)
                && n.rect().width() > 150.0
                && n.rect().top() > 30.0
        })
        .unwrap_or_else(|| panic!("o item de menu {rotulo:?}: {:?}", textos(h)))
}

/// O item do menu aberto está ligado.
pub fn item_ligado(h: &H, rotulo: &str) -> bool {
    !item_de_menu(h, rotulo).accesskit_node().is_disabled()
}

/// Clica no item do menu aberto.
pub fn clicar_item(h: &mut H, rotulo: &str) {
    item_de_menu(h, rotulo).click();
    h.run_steps(2);
}

/// Abre o submenu de um menu aberto (o "Tema" do Exibir, os "Abertos recentemente" do Arquivo), passando o ponteiro
/// sobre ele.
pub fn abrir_submenu(h: &mut H, rotulo: &str) {
    h.get(by().role(Role::Button).label(rotulo)).hover();
    h.run_steps(3);
}

/// O botão da barra de ferramentas com este rótulo (o primeiro com ele: a barra vem antes do resto da casca).
pub fn botao_da_barra<'h>(h: &'h H, rotulo: &'h str) -> egui_kittest::Node<'h> {
    h.query_all(by().role(Role::Button).label(rotulo)).next().unwrap_or_else(|| panic!("nenhum botão {rotulo:?}"))
}

/// O botão da barra de ferramentas está ligado (não `disabled`).
pub fn botao_ligado(h: &H, rotulo: &str) -> bool {
    !botao_da_barra(h, rotulo).accesskit_node().is_disabled()
}

/// Onde está o documento (o `.doc`) da aba ativa.
pub fn documento(h: &H) -> egui::Rect {
    h.state().rect_do_documento().expect("o documento à vista")
}

/// Os textos que estão no painel de Propriedades (à direita do documento, na altura do corpo: entre a barra de
/// ferramentas e a de status), de cima para baixo.
pub fn textos_das_propriedades(h: &H) -> Vec<String> {
    let doc = documento(h);
    // o corpo começa no alto da faixa de abas, 34px acima do documento
    let topo_do_corpo = doc.top() - 34.0;
    let mut nos: Vec<(f32, f32, String)> = h
        .query_all_by(|_| true)
        .filter_map(|n| {
            let texto = n.accesskit_node().label().or_else(|| n.accesskit_node().value())?;
            let r = n.rect();
            let no_painel = r.left() > doc.right() && r.top() >= topo_do_corpo && r.bottom() <= doc.bottom();
            (no_painel && !texto.trim().is_empty()).then_some((r.top(), r.left(), texto))
        })
        .collect();
    nos.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    nos.into_iter().map(|(_, _, t)| t).collect()
}

/// Clica na aba de documento com este rótulo ("clientes · Dados"); a troca vale no quadro seguinte ao do clique.
pub fn clicar_aba(h: &mut H, rotulo: &str) {
    clicar(h, Role::Tab, rotulo);
    h.run_steps(1);
}

/// Clica no × da aba com este rótulo ("Fechar a aba clientes · Dados").
pub fn fechar_aba(h: &mut H, rotulo: &str) {
    clicar_botao(h, &format!("Fechar a aba {rotulo}"));
    h.run_steps(1);
}

/// As abas abertas, na ordem da faixa: o rótulo e se é a ativa.
pub fn abas(h: &H) -> Vec<(String, bool)> {
    h.state().abas_abertas()
}

/// Os rótulos das abas abertas, na ordem da faixa, e o da ativa.
pub fn rotulos_das_abas(h: &H) -> (Vec<String>, String) {
    let abas = abas(h);
    let ativa = abas.iter().find(|(_, a)| *a).map(|(r, _)| r.clone()).unwrap_or_default();
    (abas.into_iter().map(|(r, _)| r).collect(), ativa)
}

/// Onde está o nó com este papel e rótulo.
pub fn rect_de(h: &H, papel: Role, rotulo: &str) -> egui::Rect {
    h.get(by().role(papel).label(rotulo)).rect()
}

/// Espera o nó com este papel e rótulo ficar no mesmo lugar por alguns quadros seguidos (uma janela que mudou
/// de altura se recentra no quadro seguinte: o clique vai onde o alvo está, não onde estava); falha em 15 s.
pub fn esperar_parado(h: &mut H, papel: Role, rotulo: &str) {
    let mut antes = None;
    let mut parado = 0;
    esperar(h, &format!("{rotulo:?} parar no lugar"), |h| {
        let agora = h.query(by().role(papel).label(rotulo)).map(|n| n.rect());
        parado = if agora.is_some() && agora == antes { parado + 1 } else { 0 };
        antes = agora;
        parado >= 5
    });
}

/// A borda de dentro de um card do documento à vista, à direita: o fim do `.doc`, menos a borda dele (1px), o
/// `padding:20px` e a borda do card (1px).
pub fn direita_do_card(h: &H) -> f32 {
    documento(h).right() - 1.0 - 20.0 - 1.0
}

/// Uma renderização com wgpu de cada vez: dezenas de testes renderizando juntos (e os de outros crates na mesma
/// placa) estouravam o prazo do wgpu ("PollError: The requested Wait timed out").
static RENDERIZANDO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Renderiza o quadro atual com wgpu (uma renderização de cada vez), como está: com o ponteiro onde estiver.
pub fn renderizar(h: &mut H) -> image::RgbaImage {
    let _vez = RENDERIZANDO.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    h.render().expect("renderizar com wgpu")
}

/// Renderiza e grava `capturas/rust/<nome>`.
pub fn capturar(h: &mut H, nome: &str) -> PathBuf {
    // como nas de referência: o ponteiro fora da janela (nada em hover)
    h.remove_cursor();
    h.run_steps(3);
    let imagem = renderizar(h);
    let destino = pasta_capturas().join(nome);
    imagem.save(&destino).expect("gravar png");
    destino
}

/// Cria um banco SQLite pequeno com o SQL dado (pelo serviço de um ambiente descartável).
pub fn criar_banco(caminho: &Path, sql: &str) {
    let prefs = prefs::Preferencias::abrir(Some(caminho.with_extension("prefs.json")));
    let s = servico::Servico::novo(prefs);
    let info = s.novo_banco(caminho, false).expect("criar banco");
    let r = s.sql(&info.id, sql, 10).expect("executar sql");
    for resultado in &r.resultados {
        if let banco::ResultadoComando::Erro { mensagem, .. } = resultado {
            panic!("sql de preparação falhou: {mensagem}");
        }
    }
    s.fechar_banco(&info.id, true).expect("fechar");
    let _ = std::fs::remove_file(caminho.with_extension("prefs.json"));
    let _ = std::fs::remove_file(caminho.with_extension("prefs.json.lock"));
}

// ---- docas (revisão de UI de 19/10/2026) ---------------------------------------------------------------------------

/// O Alt do teclado.
pub const ALT: egui::Modifiers =
    egui::Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };

/// O painel ("Explorador", "Histórico", "Propriedades") está à vista: a aba ativa de uma doca à vista, ou a janela
/// própria aberta.
pub fn painel_a_vista(h: &H, nome: &str) -> bool {
    h.state().painel_a_vista(nome)
}

/// Clica na aba de um painel numa doca ("Explorador", "Histórico", "Propriedades · clientes").
pub fn clicar_aba_do_painel(h: &mut H, rotulo: &str) {
    clicar(h, Role::Tab, rotulo);
    h.run_steps(2);
}

/// A aba de painel com este rótulo existe numa doca e é a ativa dela.
pub fn aba_do_painel_ativa(h: &H, rotulo: &str) -> bool {
    h.query_all(by().role(Role::Tab).label(rotulo)).any(|n| n.accesskit_node().is_selected() == Some(true))
}

/// Vai ao Início (Alt+Home, o Exibir › Início).
pub fn ir_ao_inicio(h: &mut H) {
    h.key_press_modifiers(ALT, egui::Key::Home);
    h.run_steps(2);
    assentar(h);
}

/// Põe uma vista nas Propriedades da tabela à vista (Alt+1 a Alt+4: Resumo, Colunas, Índices, SQL).
pub fn vista_da_tabela(h: &mut H, n: u8) {
    let tecla = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4][usize::from(n - 1)];
    h.key_press_modifiers(ALT, tecla);
    h.run_steps(2);
    assentar(h);
}

/// Mostra ou esconde um painel pelo Exibir ("Explorador", "Propriedades", "Histórico de consultas").
pub fn exibir(h: &mut H, item: &str) {
    abrir_menu(h, "Exibir");
    clicar(h, Role::CheckBox, item);
    h.run_steps(2);
}
