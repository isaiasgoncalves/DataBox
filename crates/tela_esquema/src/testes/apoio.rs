//! Apoio dos testes: palcos das abas (com a casca imitada: a aba no lugar do conteúdo da página, x=260 e
//! y=230 como nas capturas de referência, e a tabela relida quando a aba pede) e das janelas globais,
//! consultas pela árvore de acessibilidade e leituras do arquivo do banco.
//!
//! Cada palco do `egui_kittest` cria o seu wgpu. Criar, renderizar ou desfazer vários ao mesmo tempo, nas threads
//! dos testes, às vezes derruba o processo dentro do driver (0xC0000005; o mesmo que o apoio da aba Dados
//! registra): essas três coisas passam por uma vez só ([`vez`]); o resto dos testes continua em paralelo.

use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use egui::accesskit::Role;
use egui::{Key, Modifiers, Rect, UiBuilder, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::{Ambiente, NoPalco, Palco, esperar, palco, palco_documento};
use estado::{Aba, Acao, Contexto, Documento, ParteTabela, Rota, TabelaAberta};
use interface::tema::Tema;

use crate::{AbaDefinicao, AbaEstrutura, AbaIndices, JanelaNovaTabela, JanelaNovoIndice};

pub(crate) type H<S> = Harness<'static, Palco<S>>;

static VEZ: Mutex<()> = Mutex::new(());

/// A vez de mexer no wgpu de um palco (criar, renderizar, desfazer), uma thread de cada vez.
pub(crate) fn vez() -> MutexGuard<'static, ()> {
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// O palco de um teste: usado como o próprio harness (`Deref`), criado e desfeito na sua vez.
pub(crate) struct Teste<S: 'static>(Option<H<S>>);

impl<S: 'static> Deref for Teste<S> {
    type Target = H<S>;
    fn deref(&self) -> &H<S> {
        self.0.as_ref().expect("palco desfeito")
    }
}

impl<S: 'static> DerefMut for Teste<S> {
    fn deref_mut(&mut self) -> &mut H<S> {
        self.0.as_mut().expect("palco desfeito")
    }
}

impl<S: 'static> Drop for Teste<S> {
    fn drop(&mut self) {
        let _vez = vez();
        drop(self.0.take());
    }
}

/// O `palco` do `estado::apoio_testes`, criado na vez do wgpu.
fn palco_na_vez<S: 'static>(
    amb: &Ambiente,
    tela: S,
    tema: Tema,
    tamanho: egui::Vec2,
    desenhar: impl FnMut(&mut egui::Ui, &mut Contexto, &mut S) + 'static,
) -> Teste<S> {
    let _vez = vez();
    Teste(Some(palco(amb, tela, tema, tamanho, desenhar)))
}

/// As três abas têm o mesmo par nova/ui.
pub(crate) trait AbaDeTeste: 'static {
    const ABA: Aba;
    fn criar(tabela: &TabelaAberta) -> Self;
    fn desenhar(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta);
}

impl AbaDeTeste for AbaEstrutura {
    const ABA: Aba = Aba::Estrutura;
    fn criar(tabela: &TabelaAberta) -> Self {
        AbaEstrutura::nova(tabela)
    }
    fn desenhar(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.ui(ui, ctx, tabela);
    }
}

impl AbaDeTeste for AbaIndices {
    const ABA: Aba = Aba::Indices;
    fn criar(tabela: &TabelaAberta) -> Self {
        AbaIndices::nova(tabela)
    }
    fn desenhar(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.ui(ui, ctx, tabela);
    }
}

impl AbaDeTeste for AbaDefinicao {
    const ABA: Aba = Aba::Definicao;
    fn criar(tabela: &TabelaAberta) -> Self {
        AbaDefinicao::nova(tabela)
    }
    fn desenhar(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta) {
        self.ui(ui, ctx, tabela);
    }
}

/// Uma aba com a tabela dela, como a casca guarda.
pub(crate) struct TelaAba<A> {
    pub(crate) aba: A,
    pub(crate) tabela: TabelaAberta,
    /// Quantas vezes a "casca" releu a tabela e refez a aba.
    pub(crate) recargas: usize,
}

/// Onde o conteúdo da página começa nas capturas de referência (depois da lateral e das abas da tabela).
pub(crate) const X_CONTEUDO: f32 = 260.0;
pub(crate) const Y_CONTEUDO: f32 = 230.0;
pub(crate) const LARGURA_CONTEUDO: f32 = 1160.0;

/// Palco de uma aba da tabela `nome`, na largura 1440 e na altura dada.
pub(crate) fn palco_aba<A: AbaDeTeste>(amb: &Ambiente, nome: &str, altura: f32) -> Teste<TelaAba<A>> {
    palco_aba_tema(amb, nome, altura, Tema::Claro)
}

/// [`palco_aba`] num tema.
pub(crate) fn palco_aba_tema<A: AbaDeTeste>(amb: &Ambiente, nome: &str, altura: f32, tema: Tema) -> Teste<TelaAba<A>> {
    palco_aba_com(amb, nome, vec2(1440.0, altura), tema, false)
}

/// Palco de uma aba numa janela de `tamanho`, com a página que rola (o `ScrollArea` vertical da casca, de y=230 até o fim da janela).
pub(crate) fn palco_aba_rolavel<A: AbaDeTeste>(amb: &Ambiente, nome: &str, tamanho: egui::Vec2) -> Teste<TelaAba<A>> {
    palco_aba_com(amb, nome, tamanho, Tema::Claro, true)
}

/// O lugar da página nos palcos de [`palco_aba_rolavel`] (para conferir o que está à vista).
pub(crate) fn vista_da_pagina(tamanho: egui::Vec2) -> Rect {
    Rect::from_min_max(pos2(X_CONTEUDO, Y_CONTEUDO), pos2(tamanho.x - 20.0, tamanho.y))
}

fn palco_aba_com<A: AbaDeTeste>(
    amb: &Ambiente,
    nome: &str,
    tamanho: egui::Vec2,
    tema: Tema,
    rolavel: bool,
) -> Teste<TelaAba<A>> {
    let mut tabela = amb.tabela(nome);
    tabela.rota = Rota::objeto_na_aba(nome, tabela.e_visao, A::ABA);
    let tela = TelaAba { aba: A::criar(&tabela), tabela, recargas: 0 };
    palco_na_vez(amb, tela, tema, tamanho, move |ui, ctx, tela| {
        // a página acompanha o tamanho da janela (o `set_size` dos testes)
        let tamanho = ui.ctx().content_rect().size();
        let largura = (tamanho.x - X_CONTEUDO - 20.0).min(LARGURA_CONTEUDO);
        if tela.tabela.recarregar {
            // o que a casca faz: relê a tabela e refaz as abas
            let id = ctx.banco_id();
            if let Ok(completa) = ctx.servico.tabela(&id, &tela.tabela.nome) {
                let rota = tela.tabela.rota.clone();
                tela.tabela = TabelaAberta::nova(tela.tabela.nome.clone(), tela.tabela.e_visao, completa, rota);
                tela.aba = A::criar(&tela.tabela);
                tela.recargas += 1;
            }
            tela.tabela.recarregar = false;
        }
        if rolavel {
            let vista = vista_da_pagina(tamanho);
            let mut filho =
                ui.new_child(UiBuilder::new().max_rect(vista).layout(egui::Layout::top_down(egui::Align::Min)));
            egui::ScrollArea::vertical().id_salt("pagina").auto_shrink([false, false]).show(&mut filho, |ui| {
                ui.set_width(largura);
                tela.aba.desenhar(ui, ctx, &mut tela.tabela);
            });
        } else {
            let rect = Rect::from_min_size(pos2(X_CONTEUDO, Y_CONTEUDO), vec2(largura, f32::INFINITY));
            let mut filho =
                ui.new_child(UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
            tela.aba.desenhar(&mut filho, ctx, &mut tela.tabela);
        }
    })
}

/// Uma aba com a tabela dela como documento da casca (o que a tela de tabela faz: a faixa, o conteúdo e o rodapé da
/// parte, a seção "Tabela" com as ações da parte e as Propriedades dela), relendo a tabela quando a aba pede.
pub(crate) struct DocAba<A> {
    pub(crate) aba: A,
    pub(crate) tabela: TabelaAberta,
    /// Quantas vezes a "casca" releu a tabela e refez a aba.
    pub(crate) recargas: usize,
}

impl<A: AbaDeTeste + ParteTabela> DocAba<A> {
    /// O que a casca faz quando a aba pede (`tabela.recarregar`): relê a tabela e refaz a aba.
    fn reler(&mut self, ctx: &Contexto) {
        if !self.tabela.recarregar {
            return;
        }
        let id = ctx.banco_id();
        if let Ok(completa) = ctx.servico.tabela(&id, &self.tabela.nome) {
            let rota = self.tabela.rota.clone();
            self.tabela = TabelaAberta::nova(self.tabela.nome.clone(), self.tabela.e_visao, completa, rota);
            self.aba = A::criar(&self.tabela);
            self.recargas += 1;
        }
        self.tabela.recarregar = false;
    }
}

impl<A: AbaDeTeste + ParteTabela> Documento for DocAba<A> {
    fn anatomia(&self) -> estado::Anatomia {
        self.aba.anatomia()
    }
    fn faixa(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        self.reler(ctx);
        self.aba.faixa(ui, ctx, &mut self.tabela);
    }
    fn conteudo(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        self.reler(ctx);
        self.aba.conteudo(ui, ctx, &mut self.tabela);
    }
    fn rodape(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        self.aba.rodape(ui, ctx, &mut self.tabela);
    }
    fn ofertas(&self, estado: &estado::Estado) -> estado::Ofertas {
        self.aba.ofertas(&self.tabela, estado)
    }
    fn executar(&mut self, verbo: estado::Verbo, egui: &egui::Context, ctx: &mut Contexto) {
        self.aba.executar(verbo, egui, ctx, &mut self.tabela);
    }
    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        self.aba.busca()
    }
    fn evento_busca(&mut self, evento: estado::EventoBusca, egui: &egui::Context, ctx: &mut Contexto) {
        self.aba.evento_busca(evento, egui, ctx, &mut self.tabela);
    }
    fn tem_propriedades(&self) -> bool {
        true
    }
    fn propriedades(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto) {
        let (aba, tabela) = (&mut self.aba, &self.tabela);
        estado::secao_tabela(ui, tabela, |ui| aba.acoes_da_tabela(ui, ctx, tabela));
        self.aba.propriedades(ui, ctx, &mut self.tabela);
    }
    fn ocupada(&self) -> bool {
        self.aba.ocupada()
    }
}

/// O estado de um palco de documento ([`palco_doc`]): `h.state().tela.doc` é a [`DocAba`].
pub(crate) type NoPalcoDoc<A> = NoPalco<DocAba<A>>;

/// Palco de uma aba da tabela `nome` desenhada como a casca (`estado::apoio_testes::palco_documento`: a barra com os
/// verbos ligados e a busca, o documento com a faixa e o painel de Propriedades à direita), no tema e no tamanho
/// dados.
pub(crate) fn palco_doc<A: AbaDeTeste + ParteTabela>(
    amb: &Ambiente,
    nome: &str,
    tema: Tema,
    tamanho: egui::Vec2,
) -> Teste<NoPalcoDoc<A>> {
    let mut tabela = amb.tabela(nome);
    tabela.rota = Rota::objeto_na_aba(nome, tabela.e_visao, A::ABA);
    let doc = DocAba { aba: A::criar(&tabela), tabela, recargas: 0 };
    let _vez = vez();
    Teste(Some(palco_documento(amb, doc, tema, tamanho)))
}

/// [`palco_doc`] no claro, a 1440 × 900.
pub(crate) fn palco_doc_claro<A: AbaDeTeste + ParteTabela>(amb: &Ambiente, nome: &str) -> Teste<NoPalcoDoc<A>> {
    palco_doc(amb, nome, Tema::Claro, vec2(1440.0, 900.0))
}

/// Uma janela global aberta (enquanto `aberta`).
pub(crate) struct TelaJanela<J> {
    pub(crate) janela: J,
    pub(crate) aberta: bool,
}

pub(crate) fn palco_nova_tabela(amb: &Ambiente) -> Teste<TelaJanela<JanelaNovaTabela>> {
    palco_nova_tabela_tema(amb, Tema::Claro)
}

/// [`palco_nova_tabela`] num tema.
pub(crate) fn palco_nova_tabela_tema(amb: &Ambiente, tema: Tema) -> Teste<TelaJanela<JanelaNovaTabela>> {
    let tela = TelaJanela { janela: JanelaNovaTabela::nova(), aberta: true };
    palco_na_vez(amb, tela, tema, vec2(1440.0, 900.0), |ui, ctx, tela| {
        if tela.aberta {
            let egui = ui.ctx().clone();
            tela.aberta = tela.janela.mostrar(&egui, ctx);
        }
    })
}

/// A janela Novo índice aberta pela casca (a sugestão de índice da aba Dados), com a tabela aberta atrás dela,
/// como a casca guarda.
pub(crate) struct CascaNovoIndice {
    pub(crate) janela: Option<JanelaNovoIndice>,
    /// A tabela aberta na tela de trás (a aba Dados dela), se houver.
    pub(crate) tabela: Option<TabelaAberta>,
    /// Quantas vezes a "casca" releu a tabela aberta.
    pub(crate) recargas: usize,
}

/// A janela da casca para `tabela`, com `colunas` já marcadas; com `aberta`, a tabela também está aberta atrás.
pub(crate) fn palco_novo_indice(
    amb: &Ambiente,
    tabela: &str,
    colunas: &[&str],
    aberta: bool,
) -> Teste<CascaNovoIndice> {
    let janela = JanelaNovoIndice::nova(tabela.to_string(), colunas.iter().map(|c| c.to_string()).collect());
    let tabela = aberta.then(|| {
        let mut t = amb.tabela(tabela);
        t.rota = Rota::objeto_na_aba(tabela, false, Aba::Dados);
        t
    });
    let tela = CascaNovoIndice { janela: Some(janela), tabela, recargas: 0 };
    palco_na_vez(amb, tela, Tema::Claro, vec2(1440.0, 900.0), |ui, ctx, tela| {
        let egui = ui.ctx().clone();
        let Some(j) = tela.janela.as_mut() else { return };
        if !j.mostrar(&egui, ctx) {
            // o combinado com a casca: criado o índice da tabela aberta, ela relê a tabela (a dica some)
            if let Some(t) = tela.tabela.as_mut().filter(|t| j.criou() && j.tabela() == t.nome) {
                let id = ctx.banco_id();
                if let Ok(completa) = ctx.servico.tabela(&id, &t.nome) {
                    let rota = t.rota.clone();
                    *t = TabelaAberta::nova(t.nome.clone(), false, completa, rota);
                    tela.recargas += 1;
                }
            }
            tela.janela = None;
        }
    })
}

/// A janela da casca (aberta).
pub(crate) fn janela_da_casca(h: &H<CascaNovoIndice>) -> &JanelaNovoIndice {
    h.state().tela.janela.as_ref().expect("a janela está aberta")
}

/// Gira a roda do mouse sobre `pos` (`delta` < 0 desce o conteúdo), em passos, e espera a rolagem parar.
pub(crate) fn rolar_com_roda<S>(h: &mut H<S>, pos: egui::Pos2, delta: f32, vezes: usize) {
    h.hover_at(pos);
    for _ in 0..vezes {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, delta),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(2);
    }
    assentar(h);
}

/// Roda quadros por meio segundo (a rolagem animada e as prévias agendadas terminam).
pub(crate) fn assentar<S>(h: &mut H<S>) {
    let inicio = std::time::Instant::now();
    while inicio.elapsed() < std::time::Duration::from_millis(500) {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h.run_steps(3);
}

/// A janela da casca ainda está aberta?
pub(crate) fn janela_aberta(h: &H<CascaNovoIndice>) -> bool {
    h.state().tela.janela.is_some()
}

// ------------------------------------------------------------------ consultas

/// Roda quadros até aparecer um nó cujo rótulo contém `texto` (o `esperar_texto` do apoio, aceitando vários).
pub(crate) fn esperar_texto<S>(h: &mut H<S>, texto: &str) {
    let alvo = texto.to_string();
    esperar(h, move |h| h.query_all_by_label_contains(&alvo).next().is_some());
}

/// Grava a captura com o ponteiro fora da janela (nada em hover, como nas capturas de referência).
pub(crate) fn capturar<S>(h: &mut H<S>, nome: &str) {
    h.event(egui::Event::PointerGone);
    h.run_steps(3);
    let _vez = vez();
    estado::apoio_testes::capturar(h, nome);
}

/// Há um nó com exatamente este rótulo?
pub(crate) fn tem<S>(h: &H<S>, rotulo: &str) -> bool {
    h.query_all_by_label(rotulo).next().is_some()
}

/// Quantos nós têm exatamente este rótulo.
pub(crate) fn contar<S>(h: &H<S>, rotulo: &str) -> usize {
    h.query_all_by_label(rotulo).count()
}

/// Quantos botões há na tela.
pub(crate) fn botoes<S>(h: &H<S>) -> Vec<String> {
    h.query_all_by_role(Role::Button).filter_map(|n| n.accesskit_node().label()).collect()
}

/// O botão da barra com este rótulo está ligado?
pub(crate) fn ligado_na_barra<S>(h: &H<S>, rotulo: &str) -> bool {
    h.query_all_by_label(rotulo).any(|n| n.accesskit_node().role() == Role::Button && !n.accesskit_node().is_disabled())
}

/// O texto de um rótulo que contém `trecho`.
pub(crate) fn texto_com<S>(h: &H<S>, trecho: &str) -> Option<String> {
    h.query_all_by_label_contains(trecho).next().and_then(|n| {
        let a = n.accesskit_node();
        if a.role() == Role::Label { a.value() } else { a.label() }
    })
}

/// O nó com este rótulo, preferindo o que não é texto (o botão `nome-abre` ao `code` com o mesmo nome).
pub(crate) fn no<'t, S>(h: &'t H<S>, rotulo: &'t str) -> egui_kittest::Node<'t> {
    let mut todos: Vec<_> = h.query_all_by_label(rotulo).collect();
    let i = todos.iter().position(|n| n.accesskit_node().role() != Role::Label).unwrap_or(0);
    assert!(!todos.is_empty(), "nenhum nó {rotulo:?}");
    todos.swap_remove(i)
}

/// O centro (y) do nó com este rótulo.
pub(crate) fn y_de<S>(h: &H<S>, rotulo: &str) -> f32 {
    no(h, rotulo).rect().center().y
}

/// Os botões com este rótulo que estão na linha do nó `ancora` (mesma altura).
pub(crate) fn na_linha<S>(h: &H<S>, ancora: &str, rotulo: &str) -> usize {
    let y = y_de(h, ancora);
    h.query_all_by_label(rotulo).filter(|n| (n.rect().center().y - y).abs() < 12.0).count()
}

/// Quadros seguidos com o alvo parado antes de clicar: mais que a espera da prévia (300 ms = 18 quadros), para
/// a resposta dela (que muda a altura do aviso e recentra a janela) não chegar no meio do clique.
const QUADROS_PARADO: usize = 24;

/// Roda quadros até o retângulo que `achar` devolve ficar parado por [`QUADROS_PARADO`] quadros; devolve ele.
pub(crate) fn mirar<S>(h: &mut H<S>, descricao: &str, achar: impl Fn(&H<S>) -> Option<egui::Rect>) -> egui::Rect {
    let inicio = std::time::Instant::now();
    let mut ultimo: Option<egui::Rect> = None;
    let mut parado = 0;
    loop {
        h.step();
        let r = achar(h);
        match (r, ultimo) {
            (Some(a), Some(b)) if a == b => {
                parado += 1;
                if parado >= QUADROS_PARADO {
                    return a;
                }
            }
            _ => {
                parado = 0;
                ultimo = r;
            }
        }
        assert!(inicio.elapsed() < std::time::Duration::from_secs(15), "{descricao}: o alvo não apareceu ou não parou");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Clica (mover, apertar, soltar) no meio do retângulo.
pub(crate) fn clicar_em<S>(h: &mut H<S>, rect: egui::Rect) {
    let pos = rect.center();
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run_steps(2);
}

/// Clica no botão com este rótulo que está na linha do nó `ancora`.
pub(crate) fn clicar_na_linha<S>(h: &mut H<S>, ancora: &str, rotulo: &str) {
    let rect = mirar(h, rotulo, |h| {
        let y = h.query_all_by_label(ancora).next()?.rect().center().y;
        h.query_all_by_label(rotulo).find(|n| (n.rect().center().y - y).abs() < 12.0).map(|n| n.rect())
    });
    clicar_em(h, rect);
}

/// Clica no nó com este rótulo (o botão, se também houver um texto igual).
pub(crate) fn clicar<S>(h: &mut H<S>, rotulo: &str) {
    let rect = mirar(h, rotulo, |h| {
        let todos: Vec<_> = h.query_all_by_label(rotulo).collect();
        let n = todos.iter().find(|n| n.accesskit_node().role() != Role::Label).or(todos.first())?;
        Some(n.rect())
    });
    clicar_em(h, rect);
}
/// A camada de cima: os widgets da última área desenhada (a janela aberta por último; sem janela, a tela). Na
/// árvore de acessibilidade do egui, cada área é um contêiner filho da raiz, na ordem em que foi desenhada; o
/// nó `Dialog` da janela é uma folha à parte.
pub(crate) fn camada_de_cima<'t, S>(h: &'t H<S>) -> egui_kittest::Node<'t> {
    let raiz = h.queryable_node();
    raiz.children().filter(|n| n.accesskit_node().role() != Role::Dialog).last().unwrap_or(raiz)
}

/// Clica no widget com este rótulo que está na janela mais de cima que o tem (a última área desenhada que o
/// tem: a lista aberta de um campo com sugestões não atrapalha).
pub(crate) fn clicar_no_dialogo<S>(h: &mut H<S>, rotulo: &str) {
    let rect = mirar(h, rotulo, |h| {
        let raiz = h.queryable_node();
        let camadas: Vec<_> = raiz.children().filter(|n| n.accesskit_node().role() != Role::Dialog).collect();
        camadas
            .into_iter()
            .rev()
            .find_map(|camada| camada.query_all_by_label(rotulo).find(|n| n.accesskit_node().role() != Role::Label))
            .map(|n| n.rect())
    });
    clicar_em(h, rect);
}

/// Clica num texto (um rótulo que não faz nada: tira o foco do campo e fecha a lista de sugestões).
pub(crate) fn clicar_no_texto<S>(h: &mut H<S>, texto: &str) {
    let rect = mirar(h, texto, |h| {
        h.query_all_by_label(texto)
            .rfind(|n| matches!(n.accesskit_node().role(), Role::Label | Role::Heading))
            .map(|n| n.rect())
    });
    clicar_em(h, rect);
}
/// Os títulos das janelas abertas.
pub(crate) fn dialogos<S>(h: &H<S>) -> Vec<String> {
    h.query_all_by_role(Role::Dialog).filter_map(|n| n.accesskit_node().label()).collect()
}

/// A frase da confirmação aberta (o `p.frase-confirmar`, o texto mais longo da janela de cima).
pub(crate) fn frase_da_confirmacao<S>(h: &H<S>) -> Option<String> {
    let camada = camada_de_cima(h);
    camada.query_all_by_role(Role::Label).filter_map(|n| n.accesskit_node().value()).max_by_key(String::len)
}

/// Troca o texto do campo com este rótulo (o `fill` do Playwright): clica, seleciona tudo e digita.
pub(crate) fn preencher<S>(h: &mut H<S>, rotulo: &str, texto: &str) {
    let rect = mirar(h, rotulo, |h| h.query_by_role_and_label(Role::TextInput, rotulo).map(|n| n.rect()));
    clicar_em(h, rect);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(1);
    if texto.is_empty() {
        h.key_press(Key::Backspace);
    } else {
        h.get_by_role_and_label(Role::TextInput, rotulo).type_text(texto);
    }
    h.run_steps(2);
}

/// O valor do campo de texto com este rótulo.
pub(crate) fn valor<S>(h: &H<S>, rotulo: &str) -> String {
    h.get_by_role_and_label(Role::TextInput, rotulo).value().unwrap_or_default()
}

/// A caixa com este rótulo está marcada?
pub(crate) fn marcada<S>(h: &H<S>, rotulo: &str) -> bool {
    h.get_by_role_and_label(Role::CheckBox, rotulo).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

/// A caixa com este rótulo está desabilitada?
pub(crate) fn desabilitada<S>(h: &H<S>, rotulo: &str) -> bool {
    h.get_by_label(rotulo).accesskit_node().is_disabled()
}

/// Espera o texto da prévia conter `trecho` e não estar mais desatualizada.
pub(crate) fn esperar_previa<S>(h: &mut H<S>, mudanca: impl Fn(&S) -> Option<&crate::mudanca::Mudanca>, trecho: &str) {
    let alvo = trecho.to_string();
    esperar(h, move |h| {
        mudanca(&h.state().tela).is_some_and(|m| m.texto.contains(&alvo) && !m.desatualizada && !m.previa_pendente())
    });
}

/// Espera a prévia agendada chegar (a janela se recentra quando o aviso muda de altura: clicar no meio disso
/// erra o alvo).
pub(crate) fn assentar_previa<S>(h: &mut H<S>, mudanca: impl Fn(&S) -> Option<&crate::mudanca::Mudanca>) {
    esperar(h, move |h| mudanca(&h.state().tela).is_some_and(|m| !m.previa_pendente()));
    h.run_steps(3);
}

/// As ações emitidas até agora.
pub(crate) fn acoes<S>(h: &H<S>) -> Vec<Acao> {
    h.state().acoes.clone()
}

// ------------------------------------------------------------------ o arquivo do banco

/// Lê o arquivo do banco por uma conexão própria, só leitura: as
/// colunas de `sql` são textos (NULL vira "").
pub(crate) fn consultar(caminho: &Path, sql: &str) -> Vec<Vec<String>> {
    let banco = banco::Banco::abrir(caminho, true, false).expect("abrir o banco");
    let r = banco.com_conexao(|conn| {
        let erro = |e: &dyn std::fmt::Display| comum::ErroBanco::novo(format!("{sql}: {e}"));
        let mut st = conn.prepare(sql).map_err(|e| erro(&e))?;
        let n = st.column_count();
        let linhas = st
            .query_map([], |r| (0..n).map(|i| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default)).collect())
            .map_err(|e| erro(&e))?;
        linhas.collect::<Result<Vec<Vec<String>>, _>>().map_err(|e| erro(&e))
    });
    banco.fechar();
    r.expect("consultar o arquivo")
}

/// Um número do arquivo do banco (`SELECT count(*) …`).
pub(crate) fn numero(caminho: &Path, sql: &str) -> i64 {
    let banco = banco::Banco::abrir(caminho, true, false).expect("abrir o banco");
    let r = banco.com_conexao(|conn| {
        conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(|e| comum::ErroBanco::novo(format!("{sql}: {e}")))
    });
    banco.fechar();
    r.expect("consultar o arquivo")
}

pub(crate) fn colunas_do_arquivo(caminho: &Path, tabela: &str) -> Vec<String> {
    consultar(caminho, &format!("SELECT name FROM pragma_table_xinfo('{tabela}')"))
        .into_iter()
        .map(|l| l[0].clone())
        .collect()
}

pub(crate) fn objeto(caminho: &Path, tipo: &str, nome: &str) -> Option<String> {
    consultar(caminho, &format!("SELECT sql FROM sqlite_schema WHERE type = '{tipo}' AND name = '{nome}'"))
        .into_iter()
        .next()
        .map(|l| l[0].clone())
}

pub(crate) fn contar_linhas(caminho: &Path, tabela: &str) -> i64 {
    numero(caminho, &format!("SELECT count(*) FROM \"{tabela}\""))
}
