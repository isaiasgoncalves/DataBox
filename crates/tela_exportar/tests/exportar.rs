//! A janela Exportar no `egui_kittest` com o banco de exemplo: formato, o que
//! exportar, opções, o arquivo salvo, substituir só com confirmação, o banco sem tabelas, os avisos do Excel, a
//! proteção de fórmulas no CSV, a barra de progresso, "Interromper" e fechar no meio. O diálogo de salvar do
//! Windows é trocado pelo gancho de teste da janela. As capturas vão para `capturas/rust/`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, PoisonError};

use egui::accesskit::Role;
use egui::{Event, Key, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use estado::apoio_testes::{Ambiente, Palco, assentar, capturar, esperar, esperar_texto, palco};
use estado::{Acao, FormatoArquivo, OrigemJanelaExportar, PedidoJanelaExportar};
use interface::icones::{Icone, TipoArquivo};
use interface::tema::Tema;
use tela_exportar::{JanelaExportar, MSG_INTERROMPIDA, PERGUNTA_FECHAR, SEM_TABELAS};

/// A janela no palco; `None` depois que `mostrar` devolveu false.
struct Cena {
    janela: Option<JanelaExportar>,
}

type H = Harness<'static, Palco<Cena>>;

/// As respostas do diálogo, na ordem: Some((caminho, confirmado)) ou None (cancelou).
type Destinos = VecDeque<Option<(PathBuf, bool)>>;

/// O diálogo de salvar dos testes: guarda o que foi pedido e devolve os destinos da fila, na ordem.
#[derive(Clone, Default)]
struct Dialogo {
    pedidos: Rc<RefCell<Vec<(String, FormatoArquivo)>>>,
    destinos: Rc<RefCell<Destinos>>,
}

impl Dialogo {
    fn responder(&self, caminho: &Path, confirmado: bool) {
        self.destinos.borrow_mut().push_back(Some((caminho.to_path_buf(), confirmado)));
    }
}

fn abrir(amb: &Ambiente, pedido: PedidoJanelaExportar, tema: Tema) -> (H, Dialogo) {
    let dialogo = Dialogo::default();
    let d = dialogo.clone();
    let janela = JanelaExportar::nova(pedido).com_dialogo_de_teste(move |nome, formato| {
        d.pedidos.borrow_mut().push((nome.to_string(), formato));
        d.destinos.borrow_mut().pop_front().flatten()
    });
    let mut h = palco(amb, Cena { janela: Some(janela) }, tema, vec2(1440.0, 900.0), |ui, ctx, cena| {
        if let Some(j) = cena.janela.as_mut()
            && !j.mostrar(ui.ctx(), ctx)
        {
            cena.janela = None;
        }
    });
    esperar(&mut h, |h| h.query_by_role(Role::Dialog).is_some());
    assentar(&mut h);
    (h, dialogo)
}

/// O texto de um nó (num rótulo, o AccessKit guarda o texto em `value`).
fn rotulo(n: &egui_kittest::Node<'_>) -> String {
    let no = n.accesskit_node();
    if no.role() == Role::Label { no.value() } else { no.label() }.unwrap_or_default()
}

/// O texto do título da janela (o AccessKit guarda o do `Heading` em `value`).
fn rotulo_do_titulo(n: &egui_kittest::Node<'_>) -> String {
    n.accesskit_node().value().unwrap_or_default()
}

fn aberta(h: &H) -> bool {
    h.state().tela.janela.is_some()
}

fn exportando(h: &H) -> bool {
    h.state().tela.janela.as_ref().is_some_and(JanelaExportar::exportando)
}

fn clicar_exportar(h: &mut H) {
    h.get_by_role_and_label(Role::Button, "Exportar").click();
    h.run_steps(2);
}

/// Espera o aviso do arquivo salvo e devolve o texto dele.
fn esperar_salvo(h: &mut H) -> String {
    esperar_texto(h, "Arquivo salvo: ");
    rotulo(&h.get_by_label_contains("Arquivo salvo: "))
}

fn pedido_tabela(amb: &Ambiente, nome: &str, marcadas: usize) -> PedidoJanelaExportar {
    let mut tabela = amb.tabela(nome);
    tabela.selecao = (1..=marcadas as i64).map(comum::Chave::Rowid).collect();
    tabela.pedido_exportacao()
}

fn sql(amb: &Ambiente, comando: &str) {
    amb.servico.sql(&amb.estado.banco_id(), comando, 10).expect("executar o SQL do teste");
}

fn contar(amb: &Ambiente, consulta: &str) -> i64 {
    let r = amb.servico.sql(&amb.estado.banco_id(), consulta, 10).expect("contar");
    match r.resultados.into_iter().next() {
        Some(banco::ResultadoComando::Linhas { linhas, .. }) => match linhas.first().and_then(|l| l.first()) {
            Some(comum::Celula::Inteiro(n)) => *n,
            outro => panic!("esperava um inteiro: {outro:?}"),
        },
        outro => panic!("esperava linhas: {outro:?}"),
    }
}

/// Os testes com a tabela de um milhão de linhas, um de cada vez. Em paralelo, os quatro criam e exportam um
/// milhão de linhas ao mesmo tempo, e a máquina carregada faz a contagem do banco inteiro passar do
/// `servico::PRAZO_CONTAGEM_BANCO` (2 s): o total vem None, o texto fica sem a porcentagem e a espera não acaba.
static PESADOS: Mutex<()> = Mutex::new(());

/// Toma a vez entre os testes pesados (um teste que falhou com a trava não derruba os outros).
fn um_pesado_por_vez() -> MutexGuard<'static, ()> {
    PESADOS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Uma tabela de um milhão de linhas, para a exportação levar alguns segundos. Quem chama segura
/// [`um_pesado_por_vez`] até o fim do teste.
fn tabela_grande(amb: &mut Ambiente) {
    sql(amb, "CREATE TABLE grande (id INTEGER PRIMARY KEY, t0 TEXT, t1 TEXT, n0 INTEGER, r0 REAL)");
    sql(
        amb,
        "WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 1000000) \
         INSERT INTO grande SELECT i, 'texto ' || ((i * 7919) % 100003), 'outro ' || ((i * 104729) % 1000), \
         (i * 3) % 1000000, ((i * 1299709) % 100000) / 7.0 FROM s",
    );
    amb.recarregar();
}

fn pedido_grande() -> PedidoJanelaExportar {
    PedidoJanelaExportar {
        origem: OrigemJanelaExportar::Tabela {
            nome: "grande".into(),
            total: Some(1_000_000),
            filtro: None,
            marcadas: vec![],
        },
        nome_sugerido: "grande".into(),
    }
}

/// O texto do progresso com linhas já gravadas ("12.340 de 1.000.000 linhas · 1%").
fn progresso_andando(h: &H) -> Option<String> {
    let texto = rotulo(&h.query_by_label_contains(" de 1.000.000 linhas · ")?);
    (!texto.starts_with("0 ") && texto.ends_with('%')).then_some(texto)
}

/// A porcentagem do texto do progresso com total ("… linhas · 12%"), de qualquer total.
fn porcentagem(h: &H) -> Option<u64> {
    let texto = rotulo(&h.query_by_label_contains(" linhas · ")?);
    texto.strip_suffix('%')?.rsplit(" · ").next()?.parse().ok()
}

/// As linhas do texto do progresso sem total ("12.340 linhas", sem a porcentagem).
fn linhas_sem_total(h: &H) -> Option<u64> {
    let texto = rotulo(&h.query_by_label_contains(" linhas")?);
    texto.strip_suffix(" linhas")?.replace('.', "").parse().ok()
}

/// Espera a barra passar de `minimo`% (para a captura mostrar o preenchimento e a proporção).
fn esperar_porcentagem(h: &mut H, minimo: u64) {
    esperar(h, move |h| porcentagem(h).is_some_and(|p| p >= minimo));
}

/// Arquivos da pasta além dos esperados (temporários que a exportação deixou para trás).
fn sobras(pasta: &Path, esperados: &[&str]) -> Vec<String> {
    let mut nomes: Vec<String> = std::fs::read_dir(pasta)
        .map(|it| it.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    nomes.retain(|n| !esperados.contains(&n.as_str()));
    nomes.sort();
    nomes
}

fn pasta_de_saida(amb: &Ambiente) -> PathBuf {
    let saida = amb.pasta().join("saida");
    std::fs::create_dir_all(&saida).expect("criar a pasta de saída");
    saida
}

#[test]
fn tabela_mostra_formato_o_que_exportar_e_opcoes() {
    let amb = Ambiente::exemplo("exportar_tabela");
    let (mut h, _) = abrir(&amb, pedido_tabela(&amb, "clientes", 3), Tema::Claro);
    assert!(h.query_by_role_and_label(Role::Dialog, "Exportar clientes").is_some());
    for rotulo in ["Excel (.xlsx)", "CSV", "JSON", "A tabela inteira (60 linhas)", "Só as linhas marcadas (3)"] {
        assert!(h.query_by_label(rotulo).is_some(), "falta {rotulo}");
    }
    assert!(h.query_by_label_contains("Só o que a busca").is_none(), "sem busca nem filtro, sem essa opção");
    assert!(h.query_by_role_and_label(Role::ComboBox, "Separador").is_none(), "o Excel não tem opções");
    assert!(h.query_by_label("Opções").is_none(), "o Excel não tem o bloco das Opções (prancheta 5)");
    assert!(h.query_by_role_and_label(Role::Button, "Exportar").is_some());
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-tabela-claro.png");

    h.get_by_label("CSV").click();
    h.run_steps(3);
    for (papel, rotulo) in [
        (Role::Label, "Separador"),
        (Role::ComboBox, "Separador"),
        (Role::Label, "Decimal"),
        (Role::ComboBox, "Decimal"),
        (Role::CheckBox, "Primeira linha com os nomes das colunas"),
        (Role::CheckBox, "Proteger contra fórmulas do Excel"),
    ] {
        assert!(h.query_by_role_and_label(papel, rotulo).is_some(), "falta {rotulo}");
    }
    let sep = h.get_by_role_and_label(Role::ComboBox, "Separador");
    let dec = h.get_by_role_and_label(Role::ComboBox, "Decimal");
    assert!(h.query_by_label("Opções").is_some(), "o CSV tem o bloco das Opções");
    // as opções em linhas, o rótulo à esquerda (90px) e o campo no resto (`90px | 1fr`, `gap:8px 12px`)
    assert_eq!(sep.rect().left(), dec.rect().left(), "os dois campos na mesma coluna");
    assert_eq!(sep.rect().width(), dec.rect().width());
    assert!(dec.rect().top() > sep.rect().bottom(), "um em cada linha");
    let rotulo = h.get_by_role_and_label(Role::Label, "Separador").rect();
    assert!((sep.rect().left() - rotulo.left() - 102.0).abs() < 1.0, "{rotulo:?} {:?}", sep.rect());
    // o Formato vai de ponta a ponta do formulário (dentro da borda e do `padding:2px` do trilho), em três partes
    // iguais: Excel à esquerda, CSV no centro, JSON à direita
    let partes = ["Excel (.xlsx)", "CSV", "JSON"].map(|r| h.get_by_role_and_label(Role::Button, r).rect());
    let perto = |a: f32, b: f32| (a - b).abs() < 0.5;
    assert!(partes.iter().all(|r| perto(r.width(), partes[0].width())), "{partes:?}");
    let trilho_esq = rotulo.left();
    assert!(perto(partes[0].left() - 3.0, trilho_esq), "{partes:?} {rotulo:?}");
    assert!(perto(partes[2].right() + 3.0, dec.rect().right()), "{partes:?} {:?}", dec.rect());
    assert!(perto(partes[1].center().x, (trilho_esq + dec.rect().right()) / 2.0), "{partes:?}");
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-tabela-csv-claro.png");

    h.get_by_label("JSON").click();
    h.run_steps(3);
    assert!(h.query_by_label("Indentado (mais fácil de ler)").is_some());
    assert!(h.query_by_role_and_label(Role::ComboBox, "Separador").is_none());
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-tabela-json-claro.png");
}

/// Onde o PNG do ícone de `lado` pontos foi pintado dentro de `dentro` no último quadro (a malha com a textura dele).
fn icone_pintado_em(h: &H, icone: Icone, lado: f32, dentro: egui::Rect) -> Option<egui::Rect> {
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

#[test]
fn cabecalho_com_icone_e_formatos_com_arquivos() {
    let amb = Ambiente::exemplo("exportar_icones");
    let pedidos = [
        (pedido_tabela(&amb, "clientes", 0), "Exportar clientes", Icone::Tabela),
        (
            PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() },
            "Exportar o banco",
            Icone::Banco,
        ),
        (
            PedidoJanelaExportar {
                origem: OrigemJanelaExportar::Consulta { sql: "SELECT 1".into() },
                nome_sugerido: "consulta".into(),
            },
            "Exportar o resultado",
            Icone::Folha(TipoArquivo::Sql),
        ),
    ];
    for (pedido, titulo, icone) in pedidos {
        let (h, _) = abrir(&amb, pedido, Tema::Escuro);
        assert!(h.query_by_role_and_label(Role::Dialog, titulo).is_some());
        // o sólido de 20 no cabeçalho, com o título 10px depois dele, no meio da mesma linha
        let titulo_no = h.get_by_role(Role::Heading).rect();
        assert_eq!(rotulo_do_titulo(&h.get_by_role(Role::Heading)), titulo);
        let faixa = egui::Rect::from_min_max(
            pos2(titulo_no.left() - 40.0, titulo_no.top() - 20.0),
            pos2(titulo_no.left(), titulo_no.bottom() + 20.0),
        );
        let r = icone_pintado_em(&h, icone, 20.0, faixa).unwrap_or_else(|| panic!("o sólido de 20 em {titulo}"));
        assert!((titulo_no.left() - r.right() - 10.0).abs() < 1.5, "{r:?} {titulo_no:?}");
        assert!((r.center().y - titulo_no.center().y).abs() < 1.5, "{r:?} {titulo_no:?}");
        // as três partes do Formato, cada uma com o arquivo dela (16 pt) antes do rótulo
        for (rotulo, arquivo) in
            [("Excel (.xlsx)", TipoArquivo::Xlsx), ("CSV", TipoArquivo::Csv), ("JSON", TipoArquivo::Json)]
        {
            let parte = h.get_by_role_and_label(Role::Button, rotulo).rect();
            let r = icone_pintado_em(&h, Icone::Folha(arquivo), 16.0, parte)
                .unwrap_or_else(|| panic!("o arquivo de 16 em {rotulo}"));
            assert!(r.center().x < parte.center().x, "o ícone fica antes do rótulo: {r:?} {parte:?}");
        }
    }
}

#[test]
fn formatos_de_larguras_iguais_e_json_no_centro() {
    let amb = Ambiente::exemplo("exportar_formatos_iguais");
    let (mut h, _) = abrir(&amb, pedido_tabela(&amb, "clientes", 0), Tema::Claro);
    let perto = |a: f32, b: f32| (a - b).abs() < 0.75;
    for escolhido in ["Excel (.xlsx)", "CSV", "JSON"] {
        h.get_by_label(escolhido).click();
        h.run_steps(3);
        let partes = ["Excel (.xlsx)", "CSV", "JSON"].map(|r| h.get_by_role_and_label(Role::Button, r).rect());
        // o escolhido (600) não muda a largura de nenhuma parte: o ícone e o rótulo cabem sempre
        assert!(partes.iter().all(|r| perto(r.width(), partes[0].width())), "{escolhido}: {partes:?}");
        assert!(perto(partes[1].center().x - partes[0].center().x, partes[2].center().x - partes[1].center().x));
        // o ícone e o rótulo juntos ficam no meio da parte (o JSON não sai do centro)
        let (icone, rotulo) = (Icone::Folha(TipoArquivo::Json), "JSON");
        let r = icone_pintado_em(&h, icone, 16.0, partes[2]).unwrap_or_else(|| panic!("o arquivo JSON em {rotulo}"));
        assert!(r.left() > partes[2].left() + 14.0 && r.right() < partes[2].center().x, "{r:?} {:?}", partes[2]);
    }
}

#[test]
fn cancelar_fecha_sem_exportar() {
    let amb = Ambiente::exemplo("exportar_cancelar_janela");
    let (mut h, dialogo) = abrir(&amb, pedido_tabela(&amb, "clientes", 0), Tema::Claro);
    let cancelar = h.get_by_role_and_label(Role::Button, "Cancelar").rect();
    let exportar = h.get_by_role_and_label(Role::Button, "Exportar").rect();
    assert!(cancelar.right() < exportar.left(), "o Cancelar vem antes da ação principal");
    assert!(!h.get_by_role_and_label(Role::Button, "Cancelar").is_focused(), "o foco inicial é do formato");
    h.get_by_role_and_label(Role::Button, "Cancelar").click();
    h.run_steps(3);
    assert!(!aberta(&h), "o Cancelar fecha a janela");
    assert!(dialogo.pedidos.borrow().is_empty(), "sem exportar nem pedir o caminho");
    assert!(h.state().acoes.is_empty());
}

#[test]
fn desmarcar_todas_a_direita_do_titulo() {
    let amb = Ambiente::exemplo("exportar_desmarcar_direita");
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (h, _) = abrir(&amb, pedido, Tema::Claro);
    let titulo = h.get_by_label("Tabelas").rect();
    let botao = h.get_by_role_and_label(Role::Button, "Desmarcar todas").rect();
    assert!(botao.left() > titulo.right() + 100.0, "o botão fica à direita do título: {botao:?} {titulo:?}");
    assert!((botao.center().y - titulo.center().y).abs() < 2.0, "{botao:?} {titulo:?}");
    // a caixa rebaixada da lista termina na mesma borda do botão (menos o padding dele)
    let lista = h.get_by_role_and_label(Role::CheckBox, "clientes").rect();
    assert!(botao.right() > lista.right(), "{botao:?} {lista:?}");
}

#[test]
fn lista_de_tabelas_com_a_ficha_de_cada_tipo() {
    let amb = Ambiente::exemplo("exportar_fichas");
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (h, _) = abrir(&amb, pedido, Tema::Escuro);
    // a ficha comum, a da tabela sem rowid (anil) e a da virtual (lupa), de 16 pt entre a caixa e o nome
    for (nome, icone) in
        [("clientes", Icone::Tabela), ("produtos", Icone::TabelaSemRowid), ("notas", Icone::TabelaVirtual)]
    {
        let caixa = h.get_by_role_and_label(Role::CheckBox, nome).rect();
        let r = icone_pintado_em(&h, icone, 16.0, caixa).unwrap_or_else(|| panic!("a ficha de {nome}"));
        assert!((r.left() - (caixa.left() + 24.0)).abs() < 1.0, "{nome}: {r:?} {caixa:?}");
    }
}

/// As capturas da prancheta 5: a janela da tabela com busca e linhas marcadas em CSV, nos dois temas.
#[test]
fn capturas_da_prancheta_5_da_tabela() {
    for (tema, nome) in
        [(Tema::Escuro, "exportar-prancheta-tabela-escuro.png"), (Tema::Claro, "exportar-prancheta-tabela-claro.png")]
    {
        let amb = Ambiente::exemplo("exportar_prancheta_tabela");
        let mut tabela = amb.tabela("clientes");
        tabela.rota =
            tabela.rota.com_dados(estado::ConsultaDados { busca: "a".into(), ..estado::ConsultaDados::default() });
        tabela.definir_contagem_filtrada("a", &[], 41);
        tabela.selecao = (1..=2_i64).map(comum::Chave::Rowid).collect();
        let (mut h, _) = abrir(&amb, tabela.pedido_exportacao(), tema);
        h.get_by_label("CSV").click();
        h.run_steps(3);
        h.event(Event::PointerGone);
        capturar(&mut h, nome);
    }
}

#[test]
fn foco_inicial_no_formato_e_enter_nao_exporta() {
    let amb = Ambiente::exemplo("exportar_foco");
    let (mut h, dialogo) = abrir(&amb, pedido_tabela(&amb, "clientes", 0), Tema::Claro);
    // o foco nasce no formato pressionado, não no "Exportar" do rodapé
    assert!(h.get_by_label("Excel (.xlsx)").is_focused(), "o foco nasce no formato pressionado");
    assert!(!h.get_by_role_and_label(Role::Button, "Exportar").is_focused());
    // Enter e Espaço logo depois de abrir só reescolhem o Excel: o diálogo de salvar não abre
    for tecla in [Key::Enter, Key::Space] {
        h.key_press(tecla);
        h.run_steps(3);
    }
    assert!(dialogo.pedidos.borrow().is_empty(), "{:?}", dialogo.pedidos.borrow());
    assert!(!exportando(&h) && aberta(&h));
    assert!(h.query_by_role_and_label(Role::ComboBox, "Separador").is_none(), "continua no Excel");
    assert!(h.get_by_label("Excel (.xlsx)").is_focused());
}

#[test]
fn tabela_com_busca_oferece_o_que_a_busca_mostra() {
    let amb = Ambiente::exemplo("exportar_busca");
    let mut tabela = amb.tabela("clientes");
    tabela.rota =
        tabela.rota.com_dados(estado::ConsultaDados { busca: "ana".into(), ..estado::ConsultaDados::default() });
    tabela.definir_contagem_filtrada("ana", &[], 12);
    let (h, _) = abrir(&amb, tabela.pedido_exportacao(), Tema::Claro);
    let rotulo = "Só o que a busca e os filtros mostram (12 linhas)";
    let no = h.get_by_label(rotulo);
    assert_eq!(no.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "com busca, vem escolhida");
}

#[test]
fn banco_lista_as_tabelas_e_pede_ao_menos_uma() {
    let amb = Ambiente::exemplo("exportar_banco");
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (mut h, dialogo) = abrir(&amb, pedido, Tema::Claro);
    assert!(h.query_by_role_and_label(Role::Dialog, "Exportar o banco").is_some());
    let tabelas: Vec<String> =
        amb.estado.objetos.as_ref().map(|o| o.tabelas.iter().map(|t| t.nome.clone()).collect()).unwrap_or_default();
    assert!(tabelas.len() >= 7);
    for t in &tabelas {
        let no = h.get_by_role_and_label(Role::CheckBox, t);
        assert_eq!(no.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "{t} nasce marcada");
    }
    assert!(h.query_by_label("Tabelas").is_some());
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-banco-claro.png");

    h.get_by_role_and_label(Role::Button, "Desmarcar todas").click();
    h.run_steps(2);
    assert!(h.query_by_role_and_label(Role::Button, "Marcar todas").is_some());
    clicar_exportar(&mut h);
    assert!(h.query_by_label("Marque ao menos uma tabela para exportar.").is_some());
    assert!(dialogo.pedidos.borrow().is_empty(), "nem chega a pedir o caminho");
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-banco-marque-claro.png");
    // marcar uma tabela tira o aviso
    h.get_by_role_and_label(Role::CheckBox, "clientes").click();
    h.run_steps(2);
    assert!(h.query_by_label("Marque ao menos uma tabela para exportar.").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Marcar todas").is_some());
}

#[test]
fn nome_longo_encurta_na_lista_de_tabelas() {
    let mut amb = Ambiente::exemplo("exportar_nome_longo");
    let longo = "t_150_tabela_de_nome_razoavelmente_comprido_para_a_lista_da_exportacao";
    sql(&amb, &format!("CREATE TABLE {longo} (a)"));
    amb.recarregar();
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (h, _) = abrir(&amb, pedido, Tema::Claro);
    // o nome acessível é o inteiro
    let nomes: Vec<String> =
        h.query_all_by_role(Role::CheckBox).map(|n| rotulo(&n)).filter(|r| r.starts_with("t_150_tabela")).collect();
    assert_eq!(nomes, [longo]);
    // o que se pinta é o nome encurtado: a caixa (16px), o espaço (8px) e o nome cabem na coluna (~219px), bem
    // menos que o nome inteiro em 12,5px
    let no = h.get_by_role_and_label(Role::CheckBox, longo);
    assert!(no.rect().right() <= 960.0, "{:?}", no.rect());
    assert!(no.rect().width() <= 225.0, "{:?}", no.rect());
}

#[test]
fn banco_inteiro_em_csv_vira_zip() {
    let amb = Ambiente::exemplo("exportar_banco_zip");
    let saida = pasta_de_saida(&amb);
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (mut h, dialogo) = abrir(&amb, pedido, Tema::Claro);
    h.get_by_label("CSV").click();
    h.run_steps(2);
    let destino = saida.join("exemplo.zip");
    dialogo.responder(&destino, true);
    clicar_exportar(&mut h);
    let texto = esperar_salvo(&mut h);
    assert!(texto.starts_with("Arquivo salvo: exemplo.zip · ") && texto.contains(" tabelas · "), "{texto}");
    assert_eq!(dialogo.pedidos.borrow().as_slice(), [("exemplo.zip".to_string(), FormatoArquivo::Zip)]);
    assert!(std::fs::read(&destino).is_ok_and(|b| b.starts_with(b"PK")));
}

#[test]
fn resultado_da_consulta_para_excel() {
    let amb = Ambiente::exemplo("exportar_consulta");
    let saida = pasta_de_saida(&amb);
    let sql = "SELECT id, nome, email FROM clientes WHERE ativo = 1 ORDER BY id";
    let ativos = contar(&amb, "SELECT count(*) FROM clientes WHERE ativo = 1");
    let pedido = PedidoJanelaExportar {
        origem: OrigemJanelaExportar::Consulta { sql: sql.into() },
        nome_sugerido: "consulta_clientes".into(),
    };
    let (mut h, dialogo) = abrir(&amb, pedido, Tema::Claro);
    assert!(h.query_by_role_and_label(Role::Dialog, "Exportar o resultado").is_some());
    assert!(h.query_by_label("O que exportar").is_none(), "a consulta não tem o que escolher");
    let destino = saida.join("clientes_ativos.xlsx");
    dialogo.responder(&destino, true);
    clicar_exportar(&mut h);
    let texto = esperar_salvo(&mut h);
    assert!(texto.starts_with(&format!("Arquivo salvo: clientes_ativos.xlsx · {ativos} linhas · ")), "{texto}");
    assert_eq!(dialogo.pedidos.borrow().as_slice(), [("consulta_clientes.xlsx".to_string(), FormatoArquivo::Xlsx)]);
    assert!(std::fs::read(&destino).is_ok_and(|b| b.starts_with(b"PK")));
    assert!(h.query_by_role_and_label(Role::Button, "Mostrar na pasta").is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Abrir o arquivo").is_some());
    // as opções usadas viraram o padrão: a casca relê as preferências
    assert!(h.state().acoes.contains(&Acao::RecarregarPrefs));
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-salvo-claro.png");
}

#[test]
fn cancelar_o_dialogo_nao_exporta() {
    let amb = Ambiente::exemplo("exportar_cancelar_dialogo");
    let (mut h, dialogo) = abrir(&amb, pedido_tabela(&amb, "clientes", 0), Tema::Claro);
    dialogo.destinos.borrow_mut().push_back(None);
    clicar_exportar(&mut h);
    assert_eq!(dialogo.pedidos.borrow().as_slice(), [("clientes.xlsx".to_string(), FormatoArquivo::Xlsx)]);
    assert!(!exportando(&h));
    assert!(h.query_by_label_contains("Arquivo salvo").is_none());
}

#[test]
fn arquivo_que_ja_existe_pergunta_antes() {
    let amb = Ambiente::exemplo("exportar_existe");
    let saida = pasta_de_saida(&amb);
    let destino = saida.join("planilha_importante.xlsx");
    std::fs::write(&destino, b"conteudo que a pessoa editou no Excel").expect("gravar");
    let (mut h, dialogo) = abrir(&amb, pedido_tabela(&amb, "clientes", 0), Tema::Claro);
    // o caminho veio sem a confirmação do diálogo: o serviço recusa e a janela pergunta
    dialogo.responder(&destino, false);
    clicar_exportar(&mut h);
    let frase = "Já existe planilha_importante.xlsx. Substituir? O conteúdo atual do arquivo se perde.";
    esperar_texto(&mut h, frase);
    assert!(h.query_by_role_and_label(Role::Dialog, "Substituir o arquivo").is_some());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label_contains(frase).is_none());
    assert!(aberta(&h) && h.query_by_role_and_label(Role::Dialog, "Exportar clientes").is_some());
    assert!(h.query_by_label_contains("Arquivo salvo").is_none());
    assert_eq!(std::fs::read(&destino).unwrap_or_default(), b"conteudo que a pessoa editou no Excel");

    dialogo.responder(&destino, false);
    clicar_exportar(&mut h);
    esperar_texto(&mut h, frase);
    h.get_by_role_and_label(Role::Button, "Substituir").click();
    let texto = esperar_salvo(&mut h);
    assert!(texto.starts_with("Arquivo salvo: planilha_importante.xlsx · 60 linhas · "), "{texto}");
    assert!(std::fs::read(&destino).is_ok_and(|b| b.starts_with(b"PK")));
}

#[test]
fn banco_sem_tabelas_so_explica() {
    let mut amb = Ambiente::vazio("exportar_sem_tabelas");
    let caminho = amb.pasta().join("vazio.db");
    amb.servico.novo_banco(&caminho, false).expect("criar o banco vazio");
    amb.recarregar();
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "vazio".into() };
    let (mut h, _) = abrir(&amb, pedido, Tema::Claro);
    assert!(h.query_by_label(SEM_TABELAS).is_some());
    assert!(h.query_by_role_and_label(Role::Button, "Exportar").is_none());
    assert!(h.query_by_label("CSV").is_none() && h.query_by_label("Tabelas").is_none());
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-banco-vazio-claro.png");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!aberta(&h));
}

#[test]
fn excel_avisa_o_que_cortou_e_o_csv_protege_formulas() {
    let mut amb = Ambiente::exemplo("exportar_avisos");
    sql(&amb, "CREATE TABLE anotacoes (texto TEXT)");
    sql(&amb, &format!("INSERT INTO anotacoes VALUES ('=1+1'), ('{}')", "x".repeat(40_000)));
    amb.recarregar();
    let saida = pasta_de_saida(&amb);

    // Excel: o texto acima de 32.767 caracteres sai cortado, e a janela diz isso
    let (mut h, dialogo) = abrir(&amb, pedido_tabela(&amb, "anotacoes", 0), Tema::Claro);
    dialogo.responder(&saida.join("anotacoes.xlsx"), true);
    clicar_exportar(&mut h);
    esperar_salvo(&mut h);
    assert!(h.query_by_label_contains("o máximo do Excel").is_some(), "a lista dos avisos do formato");
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-avisos-claro.png");
    drop(h);

    // CSV: "Proteger contra fórmulas do Excel" vem ligado; desligado, o texto sai como está
    for (proteger, esperado) in [(true, "'=1+1"), (false, "=1+1")] {
        amb.recarregar();
        let (mut h, dialogo) = abrir(&amb, pedido_tabela(&amb, "anotacoes", 0), Tema::Claro);
        h.get_by_label("CSV").click();
        h.run_steps(2);
        let caixa = h.get_by_role_and_label(Role::CheckBox, "Proteger contra fórmulas do Excel");
        assert_eq!(caixa.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
        if !proteger {
            caixa.click();
            h.run_steps(2);
        }
        let destino = saida.join(format!("anotacoes_{proteger}.csv"));
        dialogo.responder(&destino, true);
        clicar_exportar(&mut h);
        esperar_salvo(&mut h);
        let conteudo = std::fs::read_to_string(&destino).unwrap_or_default();
        let linhas: Vec<&str> = conteudo.trim_start_matches('\u{feff}').lines().collect();
        assert_eq!(linhas.get(1).copied(), Some(esperado));
    }
}

#[test]
fn progresso_e_interromper_nao_grava_nada() {
    let _vez = um_pesado_por_vez();
    let mut amb = Ambiente::exemplo("exportar_progresso");
    tabela_grande(&mut amb);
    let saida = pasta_de_saida(&amb);
    let destino = saida.join("grande.xlsx");
    std::fs::write(&destino, b"planilha que a pessoa ja tinha").expect("gravar");
    let (mut h, dialogo) = abrir(&amb, pedido_grande(), Tema::Claro);
    dialogo.responder(&destino, true);
    clicar_exportar(&mut h);
    assert!(exportando(&h));
    esperar(&mut h, |h| progresso_andando(h).is_some());
    // o "Exportar" fica ocupado e o "Interromper" (destrutivo) entra à esquerda do rodapé
    let interromper = h.get_by_role_and_label(Role::Button, "Interromper");
    let gerando = h.get_by_role_and_label(Role::Button, "Gerando arquivo");
    assert!(interromper.rect().left() < gerando.rect().left());
    assert!(gerando.accesskit_node().is_disabled());
    // as opções ficam inertes
    assert!(h.get_by_label("A tabela inteira (1.000.000 linhas)").accesskit_node().is_disabled());
    assert!(h.query_by_role(Role::ProgressIndicator).is_some());
    esperar_porcentagem(&mut h, 10);
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-progresso-claro.png");

    h.get_by_role_and_label(Role::Button, "Interromper").click();
    esperar_texto(&mut h, MSG_INTERROMPIDA);
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-interrompida-claro.png");
    // a janela volta ao estado inicial: sem progresso, sem Interromper, com o Exportar de novo
    assert!(!exportando(&h));
    assert!(h.query_by_role_and_label(Role::Button, "Interromper").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Interrompendo").is_none());
    assert!(!h.get_by_role_and_label(Role::Button, "Exportar").accesskit_node().is_disabled());
    assert!(!h.get_by_label("A tabela inteira (1.000.000 linhas)").accesskit_node().is_disabled());
    assert_eq!(std::fs::read(&destino).unwrap_or_default(), b"planilha que a pessoa ja tinha");
    assert_eq!(sobras(&saida, &["grande.xlsx"]), Vec::<String>::new());

    // depois de interromper, exportar de novo grava normalmente
    h.get_by_label("CSV").click();
    h.run_steps(2);
    let novo = saida.join("grande_novo.csv");
    dialogo.responder(&novo, true);
    clicar_exportar(&mut h);
    let texto = esperar_salvo(&mut h);
    assert!(texto.starts_with("Arquivo salvo: grande_novo.csv · 1.000.000 linhas · "), "{texto}");
    let conteudo = std::fs::read_to_string(&novo).unwrap_or_default();
    assert_eq!(conteudo.lines().count(), 1_000_001);
}

#[test]
fn fechar_no_meio_pergunta_e_interrompe() {
    let _vez = um_pesado_por_vez();
    let mut amb = Ambiente::exemplo("exportar_fechar");
    tabela_grande(&mut amb);
    let saida = pasta_de_saida(&amb);
    let destino = saida.join("grande.xlsx");
    let (mut h, dialogo) = abrir(&amb, pedido_grande(), Tema::Escuro);
    dialogo.responder(&destino, true);
    clicar_exportar(&mut h);
    esperar_porcentagem(&mut h, 10);
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-progresso-escuro.png");

    // Esc pergunta; desistir da pergunta deixa a exportação correndo, com a janela aberta
    h.key_press(Key::Escape);
    esperar_texto(&mut h, PERGUNTA_FECHAR);
    assert!(h.query_by_role_and_label(Role::Dialog, "Interromper a exportação").is_some());
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label(PERGUNTA_FECHAR).is_none());
    assert!(aberta(&h) && exportando(&h));
    let antes = progresso_andando(&h);
    esperar(&mut h, move |h| progresso_andando(h).is_some_and(|t| Some(t) != antes));

    // o clique no fundo também pergunta
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos: pos2(40.0, 40.0),
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    esperar_texto(&mut h, PERGUNTA_FECHAR);
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(aberta(&h) && h.query_by_label(PERGUNTA_FECHAR).is_none());

    // o ✕ pergunta; aceitar interrompe e fecha a janela
    h.get_by_role_and_label(Role::Button, "Fechar").click();
    esperar_texto(&mut h, PERGUNTA_FECHAR);
    let botao_da_pergunta = h.get_all_by_role_and_label(Role::Button, "Interromper").last().map(|n| n.rect());
    let Some(rect) = botao_da_pergunta else { panic!("o botão Interromper da pergunta") };
    h.event(Event::PointerMoved(rect.center()));
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos: rect.center(),
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    esperar(&mut h, |h| !aberta(h));
    // a exportação para sozinha (a janela solta cancela o andamento) e não deixa nada
    let inicio = std::time::Instant::now();
    while !sobras(&saida, &[]).is_empty() && inicio.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(!destino.exists());
    assert_eq!(sobras(&saida, &[]), Vec::<String>::new());
}

#[test]
fn banco_inteiro_mostra_a_tabela_atual() {
    andamento_do_banco("exportar_banco_progresso", Tema::Claro, "exportar-banco-progresso-claro.png");
}

#[test]
fn andamento_do_banco_com_tabela_maiuscula() {
    andamento_do_banco("exportar_banco_progresso_escuro", Tema::Escuro, "exportar-banco-progresso-escuro.png");
}

/// O banco inteiro em CSV, com a tabela de um milhão de linhas: o "Tabela 3 de 8 · grande" (T maiúsculo, `--t2`)
/// acima da barra, e o texto das linhas abaixo dela.
fn andamento_do_banco(ambiente: &str, tema: Tema, captura: &str) {
    let _vez = um_pesado_por_vez();
    let mut amb = Ambiente::exemplo(ambiente);
    tabela_grande(&mut amb);
    let saida = pasta_de_saida(&amb);
    let n_tabelas = amb.estado.objetos.as_ref().map_or(0, |o| o.tabelas.len());
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (mut h, dialogo) = abrir(&amb, pedido, tema);
    h.get_by_label("CSV").click();
    h.run_steps(2);
    let destino = saida.join("banco.zip");
    dialogo.responder(&destino, true);
    clicar_exportar(&mut h);
    // a linha da tabela atual e as linhas andando: com o total, "… linhas · 12%"; se a contagem do banco inteiro
    // passar do prazo de 2 s (a máquina carregada por outros processos), o total fica None e o texto é só
    // "123.456 linhas", que também é um estado certo da janela
    let marca = format!(" de {n_tabelas} · ");
    esperar(&mut h, move |h| {
        h.query_by_label_contains(&marca).is_some_and(|n| rotulo(&n).starts_with("Tabela "))
            && (porcentagem(h).is_some_and(|p| p >= 10) || linhas_sem_total(h).is_some_and(|n| n >= 100_000))
    });
    let com_total = h.state().tela.janela.as_ref().and_then(JanelaExportar::progresso).and_then(|p| p.total);
    if com_total.is_none() {
        eprintln!("a contagem do banco inteiro passou do prazo: o progresso ficou sem o total");
    }
    // a linha da tabela atual ("Tabela 3 de 8 · grande", com T maiúsculo) acima da barra, e o texto das linhas abaixo
    // dela
    let marca = format!(" de {n_tabelas} · ");
    let tabela_atual = h.get_by_label_contains(&marca);
    assert!(rotulo(&tabela_atual).starts_with("Tabela "), "{}", rotulo(&tabela_atual));
    let (tabela, linhas) = (tabela_atual.rect(), h.get_by_label_contains(" linhas").rect());
    let barra = h.get_by_role(Role::ProgressIndicator).rect();
    assert!(tabela.bottom() <= barra.top() && barra.bottom() <= linhas.top(), "{tabela:?} {barra:?} {linhas:?}");
    h.event(Event::PointerGone);
    capturar(&mut h, captura);
    h.get_by_role_and_label(Role::Button, "Interromper").click();
    esperar_texto(&mut h, MSG_INTERROMPIDA);
    assert!(!destino.exists());
    assert_eq!(sobras(&saida, &[]), Vec::<String>::new());
}

#[test]
fn resultado_sem_total_tem_a_barra_indeterminada() {
    let _vez = um_pesado_por_vez();
    let mut amb = Ambiente::exemplo("exportar_indeterminada");
    tabela_grande(&mut amb);
    let saida = pasta_de_saida(&amb);
    let pedido = PedidoJanelaExportar {
        origem: OrigemJanelaExportar::Consulta { sql: "SELECT * FROM grande".into() },
        nome_sugerido: "consulta_grande".into(),
    };
    let (mut h, dialogo) = abrir(&amb, pedido, Tema::Claro);
    let destino = saida.join("consulta_grande.xlsx");
    dialogo.responder(&destino, true);
    clicar_exportar(&mut h);
    // a consulta não é contada: "12.340 linhas", sem o total nem a porcentagem, e a barra sem valor
    esperar(&mut h, |h| {
        h.query_by_label_contains(" linhas").is_some_and(|n| {
            let t = rotulo(&n);
            t.len() > 7 && !t.starts_with("0 ") && !t.contains(" de ") && !t.contains('·')
        })
    });
    let barra = h.get_by_role(Role::ProgressIndicator);
    assert_eq!(barra.accesskit_node().numeric_value(), None, "barra indeterminada");
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-resultado-progresso-claro.png");
    h.get_by_role_and_label(Role::Button, "Interromper").click();
    esperar_texto(&mut h, MSG_INTERROMPIDA);
    assert!(!destino.exists());
}

#[test]
fn zip_que_ja_existe_vira_aviso_de_perigo_sem_perguntar_de_novo() {
    let amb = Ambiente::exemplo("exportar_zip_existe");
    let saida = pasta_de_saida(&amb);
    let zip = saida.join("relatorio.zip");
    std::fs::write(&zip, b"zip que a pessoa ja tinha").expect("gravar");
    let pedido = PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: "exemplo".into() };
    let (mut h, dialogo) = abrir(&amb, pedido, Tema::Claro);
    h.get_by_label("CSV").click();
    h.run_steps(2);
    let frase = "O banco inteiro em CSV é gravado num .zip, e já existe relatorio.zip nesta pasta; escolha outro nome.";

    // o diálogo do Windows confirmou relatorio.csv, mas o arquivo gravado seria relatorio.zip, que já existe: o
    // 409 vem com `substituir` e não há o que perguntar
    dialogo.responder(&saida.join("relatorio.csv"), true);
    clicar_exportar(&mut h);
    esperar(&mut h, |h| !exportando(h));
    esperar_texto(&mut h, frase);
    assert!(h.query_by_role_and_label(Role::Dialog, "Substituir o arquivo").is_none(), "não pergunta");
    assert!(h.query_by_label_contains("Já existe relatorio.zip").is_none());
    h.event(Event::PointerGone);
    capturar(&mut h, "exportar-erro-claro.png");

    // sem a confirmação do diálogo: pergunta uma vez; aceita, o mesmo 409 vira o aviso (não pergunta de novo)
    dialogo.responder(&saida.join("relatorio.csv"), false);
    clicar_exportar(&mut h);
    esperar_texto(&mut h, "Já existe relatorio.zip. Substituir? O conteúdo atual do arquivo se perde.");
    h.get_by_role_and_label(Role::Button, "Substituir").click();
    esperar(&mut h, |h| !exportando(h) && h.query_by_label(frase).is_some());
    assentar(&mut h);
    assert!(h.query_by_role_and_label(Role::Dialog, "Substituir o arquivo").is_none(), "uma pergunta só");
    assert!(aberta(&h) && !exportando(&h));
    assert_eq!(std::fs::read(&zip).unwrap_or_default(), b"zip que a pessoa ja tinha");
    assert_eq!(sobras(&saida, &["relatorio.zip"]), Vec::<String>::new());
}

#[test]
fn ordem_da_aba_dados_vai_mesmo_sem_busca() {
    let amb = Ambiente::exemplo("exportar_ordem");
    let saida = pasta_de_saida(&amb);
    // a aba Dados ordenada por nome, decrescente, sem busca nem filtro e com duas linhas marcadas
    let pedido = PedidoJanelaExportar {
        origem: OrigemJanelaExportar::Tabela {
            nome: "clientes".into(),
            total: Some(60),
            filtro: Some(estado::FiltroAtual {
                busca: None,
                filtros: vec![],
                ordem: Some("nome".into()),
                direcao: banco::Direcao::Desc,
                linhas: None,
            }),
            marcadas: vec![comum::Chave::Rowid(1), comum::Chave::Rowid(2)],
        },
        nome_sugerido: "clientes".into(),
    };
    let esperado_tudo = textos(&amb, "SELECT nome FROM clientes ORDER BY nome DESC");
    let esperado_marcadas = textos(&amb, "SELECT nome FROM clientes WHERE id IN (1, 2) ORDER BY nome DESC");
    let (mut h, dialogo) = abrir(&amb, pedido, Tema::Claro);
    assert!(h.query_by_label_contains("Só o que a busca").is_none(), "sem busca nem filtro, sem essa opção");
    h.get_by_label("CSV").click();
    h.run_steps(2);
    for (opcao, arquivo, esperado) in [
        ("A tabela inteira (60 linhas)", "tudo.csv", esperado_tudo),
        ("Só as linhas marcadas (2)", "marcadas.csv", esperado_marcadas),
    ] {
        h.get_by_label(opcao).click();
        h.run_steps(2);
        let destino = saida.join(arquivo);
        dialogo.responder(&destino, true);
        clicar_exportar(&mut h);
        esperar(&mut h, |h| !exportando(h));
        esperar_salvo(&mut h);
        let conteudo = std::fs::read_to_string(&destino).unwrap_or_default();
        let nomes: Vec<String> = conteudo
            .trim_start_matches('\u{feff}')
            .lines()
            .skip(1)
            .map(|l| l.split(';').nth(1).unwrap_or_default().to_string())
            .collect();
        assert_eq!(nomes, esperado, "{opcao}");
    }
}

/// A primeira coluna (texto) das linhas de uma consulta.
fn textos(amb: &Ambiente, consulta: &str) -> Vec<String> {
    let r = amb.servico.sql(&amb.estado.banco_id(), consulta, 1000).expect("consultar");
    match r.resultados.into_iter().next() {
        Some(banco::ResultadoComando::Linhas { linhas, .. }) => linhas
            .into_iter()
            .map(|l| match l.into_iter().next() {
                Some(comum::Celula::Texto(t)) => t,
                outro => panic!("esperava texto: {outro:?}"),
            })
            .collect(),
        outro => panic!("esperava linhas: {outro:?}"),
    }
}

#[test]
fn dialogo_de_salvar_abre_na_pasta_do_banco() {
    let mut amb = Ambiente::exemplo("exportar_pasta_dialogo");
    let outra = amb.pasta().join("outra");
    // a última pasta usada (no diálogo de abrir, por outra tela) não manda no de salvar: vale a do banco
    amb.estado.ultimo_diretorio = Some(outra.clone());
    let pasta_banco = amb.estado.banco.as_ref().map(|b| b.pasta.clone());
    assert!(pasta_banco.is_some());
    assert_eq!(tela_exportar::pasta_do_dialogo(&amb.estado), pasta_banco);
    // sem banco, a última pasta usada
    let mut sem_banco = Ambiente::vazio("exportar_pasta_dialogo_sem_banco");
    assert_eq!(tela_exportar::pasta_do_dialogo(&sem_banco.estado), None);
    sem_banco.estado.ultimo_diretorio = Some(outra.clone());
    assert_eq!(tela_exportar::pasta_do_dialogo(&sem_banco.estado), Some(outra));
}

#[test]
fn janela_sem_exportacao_fecha_sem_perguntar() {
    let amb = Ambiente::exemplo("exportar_fecha");
    let (mut h, _) = abrir(&amb, pedido_tabela(&amb, "clientes", 0), Tema::Claro);
    assert!(h.query_by_role(Role::ProgressIndicator).is_none());
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!aberta(&h));
}
