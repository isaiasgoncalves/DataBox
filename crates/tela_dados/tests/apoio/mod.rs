//! Apoio dos testes da aba Dados: abrir a aba num palco (1440×900, o card com a largura do conteúdo da versão
//! web), esperar a página, clicar em células, digitar, apertar teclas e ler o arquivo pelo serviço.
//!
//! Cada palco do `egui_kittest` cria o seu wgpu (instância com todos os back-ends, inclusive o GL do Windows,
//! que abre uma janela e um contexto escondidos). Criar ou desfazer vários ao mesmo tempo, nas threads dos
//! testes, às vezes derruba o processo dentro do driver (0xC0000005, uma vez em cada quatro ou cinco rodadas
//! nesta máquina; nunca com `--test-threads=1`). Por isso criar, renderizar e desfazer um palco passam por uma
//! vez só ([`vez`]); o resto dos testes continua em paralelo.

#![allow(dead_code)]

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use comum::Celula;
use egui::accesskit::Role;
use egui::{Event, Id, Key, Modifiers, PointerButton, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::{Ambiente, NoPalco, Palco, ParteNoPalco, esperar, id_busca_do_palco, palco_parte};
use estado::{Acao, ConsultaDados, TabelaAberta};
use interface::tema::Tema;
use tela_dados::{AbaDados, Ganchos, Retrato};

pub type Tela = NoPalco<ParteNoPalco<AbaDados>>;
pub type H = Harness<'static, Palco<Tela>>;

static VEZ: Mutex<()> = Mutex::new(());

/// A vez de mexer no wgpu de um palco (criar, renderizar, desfazer), uma thread de cada vez.
pub fn vez() -> MutexGuard<'static, ()> {
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// O palco de um teste: usado como o próprio harness (`Deref`), desfeito na sua vez.
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

/// Renderiza (na vez do wgpu) e grava a captura em `capturas/rust/` (o `capturar` do `estado::apoio_testes`).
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

/// Tamanho do palco: a viewport das capturas de referência (1440×900). O palco desenha a aba como a casca
/// (`estado::apoio_testes::palco_parte`): em cima a barra com os verbos ligados ("Nova linha", "Duplicar",
/// "Excluir", "Filtrar") e a busca da barra; embaixo o documento com a faixa, a grade e o rodapé, e as Propriedades
/// à direita. As janelas ficam no centro.
pub const TAMANHO: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// A aba de uma tabela do banco de exemplo, já com a primeira página na tela.
pub fn abrir(amb: &Ambiente, nome: &str) -> Teste {
    abrir_com(amb, amb.tabela(nome), Tema::Claro, Ganchos::default())
}

/// A aba de uma tabela já carregada (com a rota, a `TabelaInfo` e os ganchos que o teste quiser).
pub fn abrir_com(amb: &Ambiente, tabela: TabelaAberta, tema: Tema, ganchos: Ganchos) -> Teste {
    abrir_no_tamanho(amb, tabela, tema, ganchos, TAMANHO)
}

/// [`abrir_com`] numa janela de outro tamanho (a janela estreita de 960px).
pub fn abrir_no_tamanho(
    amb: &Ambiente,
    tabela: TabelaAberta,
    tema: Tema,
    ganchos: Ganchos,
    tamanho: egui::Vec2,
) -> Teste {
    let aba = AbaDados::nova(&tabela).com_ganchos(ganchos);
    let h = {
        let _vez = vez();
        palco_parte(amb, tabela, aba, tema, tamanho)
    };
    let mut h = Teste(Some(h));
    esperar_grade(&mut h);
    h
}
/// As dicas (`title`) aparecem logo no hover, para o teste as ler.
pub fn dicas_sem_demora(h: &mut H) {
    h.ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
}

/// Leva o ponteiro até um ponto (sem clicar) e deixa parado alguns quadros.
pub fn pairar(h: &mut H, pos: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.step();
    h.run_steps(4);
}

/// Rola a grade com a roda do mouse (`linhas` positivas descem), com o ponteiro sobre ela, até a rolagem parar.
pub fn rolar_com_a_roda(h: &mut H, sobre: Pos2, pontos: f32) {
    h.input_mut().events.extend([
        Event::PointerMoved(sobre),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -pontos),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.step();
    h.run_steps(30);
}

pub fn retrato(h: &H) -> Retrato {
    let doc = &h.state().tela.doc;
    doc.parte.retrato(&doc.tabela)
}

/// As ofertas da aba agora (os verbos da barra, com as dicas).
pub fn ofertas(h: &H) -> estado::Ofertas {
    use estado::ParteTabela;
    let s = h.state();
    s.tela.doc.parte.ofertas(&s.tela.doc.tabela, &s.estado)
}

/// Espera a página chegar (sem leitura nem gravação pendente).
pub fn esperar_grade(h: &mut H) {
    esperar(h, |h| {
        let r = retrato(h);
        !r.carregando && r.gravacoes_pendentes == 0 && (r.linhas > 0 || r.vazio.is_some() || r.erro.is_some())
    });
}

/// Espera o retrato satisfazer a condição.
pub fn esperar_que(h: &mut H, condicao: impl Fn(&Retrato) -> bool) {
    esperar(h, move |h| condicao(&retrato(h)));
}

/// Roda quadros enquanto houver tarefa pendente e mais alguns (para o desenho acompanhar).
pub fn assentar(h: &mut H) {
    estado::apoio_testes::assentar(h);
}

fn botao(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Tira o mouse de cima da tela antes de uma captura (nada em hover, como nas de referência).
pub fn afastar_mouse(h: &mut H) {
    h.input_mut().events.push(Event::PointerGone);
    h.step();
    h.run_steps(3);
}

/// Um clique (mover, apertar e soltar no mesmo quadro) e dois quadros.
pub fn clicar(h: &mut H, pos: Pos2) {
    h.input_mut().events.extend([Event::PointerMoved(pos), botao(pos, true), botao(pos, false)]);
    h.step();
    h.step();
}

/// Duplo clique (dois cliques no mesmo quadro) e dois quadros.
pub fn duplo_clique(h: &mut H, pos: Pos2) {
    h.input_mut().events.extend([
        Event::PointerMoved(pos),
        botao(pos, true),
        botao(pos, false),
        botao(pos, true),
        botao(pos, false),
    ]);
    h.step();
    h.step();
}

pub fn centro_celula(h: &H, linha: usize, coluna: usize) -> Pos2 {
    h.state()
        .tela
        .doc
        .parte
        .centro_celula(linha, coluna)
        .unwrap_or_else(|| panic!("a célula {linha},{coluna} não está à vista"))
}

pub fn clicar_celula(h: &mut H, linha: usize, coluna: usize) {
    let pos = centro_celula(h, linha, coluna);
    clicar(h, pos);
}

pub fn duplo_clique_celula(h: &mut H, linha: usize, coluna: usize) {
    let pos = centro_celula(h, linha, coluna);
    duplo_clique(h, pos);
}

/// Marca a caixa de uma linha (`None`: a do cabeçalho, que marca a página).
pub fn clicar_marca(h: &mut H, linha: Option<usize>) {
    let pos = h.state().tela.doc.parte.centro_marca(linha).expect("caixa de marcar");
    clicar(h, pos);
}

/// Uma tecla (apertar e soltar no mesmo quadro).
pub fn tecla(h: &mut H, key: Key) {
    tecla_com(h, Modifiers::NONE, key);
}

pub fn tecla_com(h: &mut H, modifiers: Modifiers, key: Key) {
    h.input_mut().events.extend([
        Event::ModifiersChanged(modifiers),
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
        Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
    ]);
    h.step();
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.step();
}

/// O nó está desabilitado.
pub fn desabilitado(n: &egui_kittest::Node<'_>) -> bool {
    n.accesskit_node().is_disabled()
}

/// Ctrl (o "command" do Windows também).
pub const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
pub const SHIFT: Modifiers = Modifiers::SHIFT;

/// Texto digitado no que tem o foco.
pub fn digitar(h: &mut H, texto: &str) {
    h.input_mut().events.push(Event::Text(texto.to_string()));
    h.step();
    h.step();
}

/// Põe o foco num campo, seleciona tudo e digita por cima (o `fill` do Playwright).
pub fn preencher(h: &mut H, id: Id, texto: &str) {
    h.ctx.memory_mut(|m| m.request_focus(id));
    h.step();
    tecla_com(h, CTRL, Key::A);
    if texto.is_empty() {
        tecla(h, Key::Backspace);
    } else {
        digitar(h, texto);
    }
}

/// O campo de uma coluna na janela da linha.
pub fn id_campo(h: &H, coluna: &str) -> Id {
    h.state().tela.doc.parte.id_campo_janela(coluna).unwrap_or_else(|| panic!("o campo {coluna} não está na janela"))
}

/// Clica na caixa da linha de uma coluna da ficha: "NULL" (a da coluna da direita, "limite é NULL") ou "Usar o padrão"
/// (dentro da célula do valor, à direita do campo).
pub fn clicar_caixa_do_campo(h: &mut H, coluna: &str, rotulo: &str) {
    let alvo = if rotulo == "NULL" {
        h.get_by_role_and_label(Role::CheckBox, &format!("{coluna} é NULL")).rect().center()
    } else {
        let campo = h.get_by_role_and_label(Role::TextInput, coluna).rect();
        h.query_all_by_label(rotulo)
            .filter(|n| (n.rect().center().y - campo.center().y).abs() < 30.0 && n.rect().left() >= campo.right() - 1.0)
            .map(|n| n.rect().center())
            .next()
            .unwrap_or_else(|| panic!("a caixa {rotulo} de {coluna}"))
    };
    clicar(h, egui::pos2(alvo.x, alvo.y));
}
/// Escolhe uma opção de um select (o `n`-ésimo com aquele rótulo): abre a lista e clica na opção que aparece
/// embaixo dele.
pub fn escolher(h: &mut H, rotulo: &str, n: usize, opcao: &str) {
    let sel = h.query_all_by_role_and_label(Role::ComboBox, rotulo).nth(n).expect("select").rect();
    clicar(h, egui::pos2(sel.center().x, sel.center().y));
    // a lista mostra 10 opções; as de baixo aparecem rolando a lista com a roda do mouse
    for _ in 0..4 {
        let alvo = h
            .query_all_by_label(opcao)
            // a lista abre embaixo do select, ou em cima dele quando não cabe (o do rodapé)
            .filter(|o| {
                let r = o.rect();
                (r.top() >= sel.bottom() - 1.0 || r.bottom() <= sel.top() + 1.0) && (r.left() - sel.left()).abs() < 12.0
            })
            .map(|o| o.rect().center())
            .next();
        if let Some(alvo) = alvo {
            clicar(h, egui::pos2(alvo.x, alvo.y));
            return;
        }
        let sobre_a_lista = egui::pos2(sel.center().x, sel.bottom() + 100.0);
        h.input_mut().events.extend([
            Event::PointerMoved(sobre_a_lista),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -96.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.step();
        h.step();
    }
    panic!("a opção {opcao}");
}

/// Clica num botão pelo rótulo (exato). Com mais de um (o Duplicar e o Excluir da barra e os da janela ou da
/// confirmação), o mais baixo: as janelas ficam abaixo da barra de ferramentas.
pub fn clicar_botao(h: &mut H, rotulo: &str) {
    let centro = h
        .query_all_by_role_and_label(Role::Button, rotulo)
        .map(|n| n.rect())
        .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
        .unwrap_or_else(|| panic!("o botão {rotulo}"))
        .center();
    clicar(h, egui::pos2(centro.x, centro.y));
}

/// Uma condição nova pelo popover da faixa: "+ Condição", a coluna, o operador, o valor (nos operadores que têm) e
/// Aplicar.
pub fn condicao_pelo_popover(h: &mut H, coluna: &str, operador: &str, valor: Option<&str>) {
    clicar_botao(h, "Nova condição");
    assert!(retrato(h).condicao_aberta, "o popover abriu");
    escolher(h, "Coluna da condição", 0, coluna);
    escolher(h, "Operador da condição", 0, operador);
    if let Some(v) = valor {
        let campo = h.get_by_role_and_label(Role::TextInput, "Valor da condição").rect().center();
        clicar(h, egui::pos2(campo.x, campo.y));
        digitar(h, v);
    }
    clicar_botao(h, "Aplicar");
}

/// A última rota que a aba registrou (`Acao::Substituir`: sem entrada no histórico).
pub fn ultima_rota(h: &H) -> Option<ConsultaDados> {
    h.state().acoes.iter().rev().find_map(|a| match a {
        Acao::Substituir(r) => r.dados().cloned(),
        _ => None,
    })
}

/// A aba nunca pediu uma entrada no histórico (`Acao::Atualizar`).
pub fn sem_entrada_no_historico(h: &H) -> bool {
    !h.state().acoes.iter().any(|a| matches!(a, Acao::Atualizar(_)))
}

/// A primeira coluna das linhas de uma consulta feita pelo serviço.
pub fn ler(h: &H, sql: &str) -> Vec<Vec<Celula>> {
    let s = &h.state();
    let id = s.estado.banco_id();
    let r = s.servico.sql(&id, sql, 1000).expect("consulta de conferência");
    for c in r.resultados {
        if let banco::ResultadoComando::Linhas { linhas, .. } = c {
            return linhas;
        }
    }
    Vec::new()
}

/// Um valor só.
pub fn ler_um(h: &H, sql: &str) -> Celula {
    ler(h, sql).into_iter().next().and_then(|l| l.into_iter().next()).unwrap_or(Celula::Nulo)
}

pub fn inteiro(c: &Celula) -> i64 {
    match c {
        Celula::Inteiro(i) => *i,
        outro => panic!("esperava inteiro, veio {outro:?}"),
    }
}

/// Executa SQL direto (prepara tabelas "por fora", como outro programa faria).
pub fn executar(amb: &Ambiente, sql: &str) {
    let id = amb.estado.banco_id();
    let r = amb.servico.sql(&id, sql, 10).expect("executar");
    for c in r.resultados {
        if let banco::ResultadoComando::Erro { mensagem, .. } = c {
            panic!("{mensagem}");
        }
    }
}

/// O texto do toast agora.
pub fn toast(h: &H) -> Option<String> {
    interface::pecas::toast::toast_atual(&h.ctx)
}

/// O texto de uma célula (linha, coluna de dados).
pub fn celula(h: &H, linha: usize, coluna: usize) -> String {
    h.state().tela.doc.parte.texto_celula(linha, coluna).unwrap_or_default()
}

/// A grade tem o foco do teclado.
pub fn grade_com_foco(h: &H) -> bool {
    h.state().tela.doc.parte.id_grade().is_some_and(|id| h.ctx.memory(|m| m.has_focus(id)))
}

/// O campo da busca da barra (a da casca, à esquerda da aba).
pub fn campo_busca() -> Id {
    id_busca_do_palco()
}

/// Algum widget com este id tem o foco.
pub fn com_foco(h: &H, id: Id) -> bool {
    h.ctx.memory(|m| m.has_focus(id))
}

/// A aba refeita a partir da rota atual, com a tabela relida (o que a casca faz com `tabela.recarregar`), no mesmo contexto do egui.
pub fn recarregar(amb: &Ambiente, h: Teste) -> Teste {
    recarregar_com(amb, h, Ganchos::default())
}

/// [`recarregar`] com ganchos.
pub fn recarregar_com(amb: &Ambiente, mut h: Teste, ganchos: Ganchos) -> Teste {
    let (nome, rota) = {
        let t = &h.state().tela.doc.tabela;
        (t.nome.clone(), t.rota.clone())
    };
    let mut nova = amb.tabela(&nome);
    nova.rota = rota;
    let aba = AbaDados::nova(&nova).com_ganchos(ganchos);
    h.state_mut().tela.doc = ParteNoPalco { parte: aba, tabela: nova };
    esperar_grade(&mut h);
    h
}

/// Troca a aba do palco por outra (a tela é outra; a velha é desfeita).
pub fn trocar_aba(h: &mut H, tabela: TabelaAberta) {
    let parte = AbaDados::nova(&tabela);
    let velha = std::mem::replace(&mut h.state_mut().tela.doc, ParteNoPalco { parte, tabela });
    drop(velha);
}

pub fn n_inteiros(v: &[Vec<Celula>]) -> Vec<i64> {
    v.iter().map(|l| l.first().map_or(0, inteiro)).collect()
}

/// O texto de uma célula de consulta.
pub fn texto(c: &Celula) -> String {
    interface::grade::texto_celula(c)
}

pub fn vec2_tamanho() -> egui::Vec2 {
    vec2(1200.0, 900.0)
}
