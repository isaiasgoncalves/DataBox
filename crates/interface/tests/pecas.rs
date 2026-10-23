//! Comportamento das peças com o `egui_kittest` (cliques, teclas, foco, tempo), sem renderizar.

// teste ou exemplo: unwrap/expect valem também nas funções de apoio (fora dos #[test])
#![allow(clippy::unwrap_used, clippy::expect_used)]

use comum::Celula;
use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, Ui};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable, by};
use interface::fontes;
use interface::grade::{ColunaGrade, GradeSomenteLeitura, RespostaGrade};
use interface::icones::{self, Icone};
use interface::pecas::ajuda::ajuda;
use interface::pecas::botao::Botao;
use interface::pecas::campos::{Entrada, Select, check};
use interface::pecas::janela::{Confirmacao, Janela, confirmar};
use interface::pecas::navegacao::{TamanhoTrilho, abas, segmentado};
use interface::pecas::toast::{desenhar_toast, toast, toast_atual};
use interface::pecas::zona_risco::ZonaRisco;
use interface::tema::{self, Tema};

/// Aplica o tema e espera as fontes (o primeiro quadro só instala); `false` = ainda não dá para desenhar.
fn pronto(ui: &mut Ui) -> bool {
    let ctx = ui.ctx().clone();
    tema::aplicar(&ctx, Tema::Claro);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    if !fontes::instaladas(&ctx) {
        ctx.request_repaint();
        return false;
    }
    true
}

fn harness<S>(estado: S, mut f: impl FnMut(&mut Ui, &mut S) + 'static) -> Harness<'static, S> {
    let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).with_max_steps(20).build_ui_state(
        move |ui, s: &mut S| {
            if pronto(ui) {
                f(ui, s);
            }
        },
        estado,
    );
    h.run_steps(2);
    h
}

/// Os cantos dos contornos pintados exatamente em `rect` no último quadro (a borda de 1px das peças: o
/// `RectShape` com traço, também dentro dos `Shape::Vec`).
fn cantos_do_contorno<S>(h: &Harness<'_, S>, rect: egui::Rect) -> Vec<egui::CornerRadius> {
    fn juntar(forma: &egui::Shape, rect: egui::Rect, cantos: &mut Vec<egui::CornerRadius>) {
        match forma {
            egui::Shape::Vec(formas) => formas.iter().for_each(|f| juntar(f, rect, cantos)),
            egui::Shape::Rect(r) if r.rect == rect && r.stroke.width > 0.0 => cantos.push(r.corner_radius),
            _ => {}
        }
    }
    let mut cantos = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, rect, &mut cantos);
    }
    cantos
}

/// Todos os contornos em `rect` têm os cantos `raio` (e há ao menos um).
fn contorno_com_raio<S>(h: &Harness<'_, S>, rect: egui::Rect, raio: u8, peca: &str) {
    let cantos = cantos_do_contorno(h, rect);
    assert!(!cantos.is_empty(), "{peca}: nenhum contorno em {rect:?}");
    assert!(cantos.iter().all(|c| *c == egui::CornerRadius::same(raio)), "{peca}: {cantos:?} (esperado {raio})");
}

#[derive(Default)]
struct Cliques {
    salvar: u32,
    parado: u32,
    ocupado: u32,
}

#[test]
fn botao_clica_e_desabilitado_ou_ocupado_nao() {
    let mut h = harness(Cliques::default(), |ui, s| {
        if ui.add(Botao::primario("Salvar")).clicked() {
            s.salvar += 1;
        }
        if ui.add(Botao::new("Parado").habilitado(false)).clicked() {
            s.parado += 1;
        }
        if ui.add(Botao::ouro("Gerando arquivo…").ocupado(true)).clicked() {
            s.ocupado += 1;
        }
    });
    h.get_by_label("Salvar").click();
    h.get_by_label("Parado").click();
    h.get_by_label("Gerando arquivo…").click();
    h.run_steps(2);
    assert_eq!((h.state().salvar, h.state().parado, h.state().ocupado), (1, 0, 0));
}

#[test]
fn botao_tem_28px_30px_nas_janelas_e_o_redondo_30px() {
    use interface::pecas::botao::{ALTURA_JANELA, com_altura};
    let mut h = harness((Vec::<egui::Rect>::new(), 0.0_f32), |ui, s| {
        s.0.clear();
        s.0.push(ui.add(Botao::new("Abrir")).rect);
        s.0.push(com_altura(ui, ALTURA_JANELA, |ui| ui.add(Botao::primario("Exportar")).rect));
        s.0.push(ui.add(Botao::redondo("‹").rotulo("Voltar")).rect);
        s.0.push(ui.add(Botao::ghost("Depois")).rect);
        let fonte = fontes::fonte(12.0, fontes::Peso::Medio);
        s.1 = interface::pecas::texto::galley(ui, "Abrir", fonte, egui::Color32::PLACEHOLDER).size().x;
    });
    h.run_steps(1);
    let (r, texto) = h.state().clone();
    // `.btn{height:28px}`, `.dlg footer .btn` 30px, `.ib` 30×30; fora do `com_altura` volta aos 28px
    assert_eq!(r.iter().map(|r| r.height()).collect::<Vec<_>>(), [28.0, 30.0, 30.0, 28.0]);
    assert_eq!(r[2].width(), 30.0);
    // `padding:0 15px` + a borda de 1px dos dois lados
    assert_eq!(r[0].width(), (texto + 32.0).ceil());
}

#[derive(Default)]
struct EstadoChip {
    fechou: u32,
    abriu: u32,
    altura: f32,
}

#[test]
fn chip_de_filtro_tira_o_filtro_pelo_x() {
    use interface::pecas::rotulos::{ALTURA_CHIP, chip_filtro};
    let mut h = harness(EstadoChip::default(), |ui, s| {
        let r = chip_filtro(ui, &[("ativo", true), ("igual a", false), ("1", true)], "Tirar o filtro");
        s.altura = r.resposta.rect.height();
        if r.fechar {
            s.fechou += 1;
        }
        if r.abrir {
            s.abriu += 1;
        }
    });
    assert!(h.query_by_label("Editar o filtro ativo igual a 1").is_some(), "o corpo diz o verbo e a condição");
    h.get_by_label("Tirar o filtro ativo igual a 1").click();
    h.run_steps(2);
    assert_eq!((h.state().fechou, h.state().abriu), (1, 0), "o × só tira");
    assert_eq!((h.state().altura, ALTURA_CHIP), (24.0, 24.0), "`.chip{{height:24px}}`");
    // o corpo é um botão: abre a condição para editar
    h.get_by_role_and_label(Role::Button, "Editar o filtro ativo igual a 1").click();
    h.run_steps(2);
    assert_eq!((h.state().fechou, h.state().abriu), (1, 1));
}

#[derive(Default)]
struct EstadoPopover {
    aberto: bool,
    fechou: u32,
    rect: Option<egui::Rect>,
    ancora: Option<egui::Rect>,
    opcao: usize,
}

#[test]
fn popover_fecha_com_esc_e_clique_fora_mas_nao_pela_lista_de_dentro() {
    use interface::pecas::popover::{DISTANCIA_POPOVER, Popover};
    let mut h = harness(EstadoPopover::default(), |ui, s| {
        let b = ui.add(Botao::new("Abrir"));
        s.ancora = Some(b.rect);
        if b.clicked() {
            s.aberto = true;
        }
        ui.add_space(400.0);
        ui.label("Fora");
        if !s.aberto {
            return;
        }
        let opcoes = [(0usize, "um"), (1, "dois"), (2, "três")];
        let ctx = ui.ctx().clone();
        let r = Popover::new("teste", b.rect).largura(260.0).mostrar(&ctx, |ui| {
            Select::new("dentro", &mut s.opcao, &opcoes).rotulo("Opção").mostrar(ui);
        });
        s.rect = Some(r.rect);
        if r.fechar {
            s.aberto = false;
            s.fechou += 1;
        }
    });
    h.get_by_label("Abrir").click();
    h.run_steps(3);
    assert!(h.state().aberto);
    let (rect, ancora) = (h.state().rect.unwrap(), h.state().ancora.unwrap());
    assert_eq!(rect.width(), 260.0);
    assert!((rect.top() - (ancora.bottom() + DISTANCIA_POPOVER)).abs() < 0.5, "preso embaixo da âncora: {rect:?}");
    // a lista do select de dentro: escolher fecha a lista, não o popover
    h.get_by_label("Opção").click();
    h.run_steps(2);
    h.get_by_label("três").click();
    h.run_steps(2);
    assert!(h.state().aberto, "a lista de dentro não fecha o popover");
    assert_eq!(h.state().opcao, 2);
    // Esc fecha
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(!h.state().aberto && h.state().fechou == 1);
    // o clique fora também (o que abriu não conta)
    h.get_by_label("Abrir").click();
    h.run_steps(3);
    assert!(h.state().aberto);
    h.get_by_label("Fora").click();
    h.run_steps(2);
    assert!(!h.state().aberto && h.state().fechou == 2);
}

#[test]
fn atalho_de_sql_e_um_link_da_largura_toda() {
    use interface::pecas::texto::atalho;
    let mut h = harness((0u32, None::<egui::Rect>), |ui, s| {
        ui.set_width(240.0);
        let r = atalho(ui, "SELECT * FROM clientes LIMIT 100");
        s.1 = Some(r.rect);
        if r.clicked() {
            s.0 += 1;
        }
    });
    let link = h.get_by_role_and_label(Role::Link, "SELECT * FROM clientes LIMIT 100");
    link.click();
    h.run_steps(2);
    assert_eq!(h.state().0, 1);
    let r = h.state().1.unwrap();
    // `padding:7px 10px` em volta da linha de 12px, com a borda de 1px
    assert!(r.width() >= 239.0 && r.height() >= 30.0 && r.height() <= 34.0, "{r:?}");
}

#[test]
fn atalho_justo_tem_a_largura_do_texto() {
    use interface::pecas::texto::{atalho, atalho_justo};
    let mut h = harness((None::<egui::Rect>, None::<egui::Rect>), |ui, s| {
        ui.set_width(600.0);
        s.0 = Some(atalho_justo(ui, "SELECT count(*) FROM clientes").rect);
        s.1 = Some(atalho(ui, "SELECT count(*) FROM clientes").rect);
    });
    h.run_steps(1);
    let (justo, cheio) = (h.state().0.unwrap(), h.state().1.unwrap());
    // a mesma altura; a largura do texto com o `padding:0 10px` e a borda, bem menos que a da linha
    assert_eq!(justo.height(), cheio.height());
    assert!(justo.width() < 300.0 && justo.width() > 150.0, "{justo:?}");
    assert!(cheio.width() >= 599.0, "{cheio:?}");
    assert_eq!(
        h.query_all_by_role_and_label(Role::Link, "SELECT count(*) FROM clientes").count(),
        2,
        "os dois são links"
    );
}

#[test]
fn cabecalho_com_a_pastilha_antes_do_nome() {
    use interface::grade::{LARGURA_PASTILHA_TH, Th, largura_cabecalho, pintar_cabecalho_coluna};
    use interface::icones::TipoColuna;
    let mut h = harness((None::<egui::Rect>, 0.0_f32), |ui, s| {
        let conteudo = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(200.0, 30.0));
        let p = interface::tema::paleta(ui);
        let th = Th { pastilha: Some(TipoColuna::Texto), ..Th::new("email") };
        s.0 = pintar_cabecalho_coluna(ui.painter(), p, conteudo, &th);
        s.1 = largura_cabecalho(ui.painter(), "email", None);
        // numa coluna numérica, os dois à direita
        let th = Th { pastilha: Some(TipoColuna::Inteiro), numerica: true, ..Th::new("ativo") };
        let direita = pintar_cabecalho_coluna(ui.painter(), p, conteudo.translate(egui::vec2(0.0, 40.0)), &th);
        let nome = largura_cabecalho(ui.painter(), "ativo", None);
        let esperado = conteudo.right() - nome - LARGURA_PASTILHA_TH;
        assert!(direita.is_some_and(|r| (r.left() - esperado).abs() < 0.5), "{direita:?}");
    });
    h.run_steps(1);
    let r = h.state().0.expect("a pastilha");
    assert_eq!(r.size(), egui::vec2(16.0, 16.0));
    assert_eq!(r.left(), 100.0, "a pastilha no começo do th");
    assert!((r.center().y - 115.0).abs() < 0.5, "no meio da altura");
    assert_eq!(LARGURA_PASTILHA_TH, 22.0, "16px e os 6px até o nome");
}

/// Os triângulos cheios pintados no último quadro (as setas da ordem: o `PathShape` fechado de 3 pontos), com a cor.
fn triangulos<S>(h: &Harness<'_, S>) -> Vec<(Vec<egui::Pos2>, egui::Color32)> {
    fn juntar(forma: &egui::Shape, v: &mut Vec<(Vec<egui::Pos2>, egui::Color32)>) {
        match forma {
            egui::Shape::Vec(formas) => formas.iter().for_each(|f| juntar(f, v)),
            egui::Shape::Path(p) if p.closed && p.points.len() == 3 => v.push((p.points.clone(), p.fill)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, &mut v);
    }
    v
}

#[test]
fn cabecalho_com_a_seta_da_ordem_desenhada() {
    use interface::grade::{ESPACO_SETA_ORDEM, LADO_SETA_ORDEM, Ordem, Th, largura_cabecalho, pintar_cabecalho_coluna};
    let mut h = harness((0.0_f32, egui::Color32::TRANSPARENT), |ui, s| {
        let conteudo = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(200.0, 30.0));
        let p = interface::tema::paleta(ui);
        s.0 = largura_cabecalho(ui.painter(), "id", None);
        s.1 = p.estudio.acct;
        let th = Th { ordem: Some(Ordem::Crescente), ..Th::new("id") };
        pintar_cabecalho_coluna(ui.painter(), p, conteudo, &th);
        let th = Th { ordem: Some(Ordem::Decrescente), ..Th::new("id") };
        pintar_cabecalho_coluna(ui.painter(), p, conteudo.translate(egui::vec2(0.0, 40.0)), &th);
    });
    h.run_steps(1);
    let (nome, acct) = *h.state();
    let setas = triangulos(&h);
    assert_eq!(setas.len(), 2, "a `.seta-ord` é um triângulo desenhado, não o glifo ▲ da fonte: {setas:?}");
    for (i, (pontos, cor)) in setas.iter().enumerate() {
        assert_eq!(*cor, acct, "em `--acct`");
        let (x0, x1) = pontos.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = pontos.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        // `margin-left:5px` depois do nome, 9px de lado (o triângulo ocupa 8,4 × 6,3 da caixa)
        let esquerda = 100.0 + nome + ESPACO_SETA_ORDEM;
        assert!((x0 - (esquerda + 0.72)).abs() < 0.5 && (x1 - x0 - 0.84 * LADO_SETA_ORDEM).abs() < 0.1, "{pontos:?}");
        assert!((y1 - y0 - 0.7 * LADO_SETA_ORDEM).abs() < 0.1, "{pontos:?}");
        let ponta = pontos.iter().find(|p| (p.x - (x0 + x1) / 2.0).abs() < 0.01).expect("a ponta no meio");
        if i == 0 {
            assert_eq!(ponta.y, y0, "crescente: a ponta para cima");
        } else {
            assert_eq!(ponta.y, y1, "decrescente: a ponta para baixo");
        }
    }
}

#[derive(Default)]
struct EstadoFiltros {
    etiquetas: Vec<(u64, &'static str)>,
    abrir: Vec<Option<u64>>,
    tirar: Vec<u64>,
    limpar: u32,
    barra: Option<egui::Rect>,
}

#[test]
fn filtros_da_grade_abrem_tiram_e_limpam() {
    use interface::pecas::filtros::{ALTURA_BARRA_FILTROS, ContagemFiltros, EtiquetaFiltro, barra_filtros, filtros};
    let mut h = harness(EstadoFiltros { etiquetas: vec![(7, "Sul")], ..Default::default() }, |ui, s| {
        let etiquetas: Vec<EtiquetaFiltro<'_>> = s
            .etiquetas
            .iter()
            .map(|&(id, v)| EtiquetaFiltro { id, coluna: "regiao", operador: "igual a", valor: Some(v) })
            .collect();
        let contagem =
            if etiquetas.is_empty() { ContagemFiltros::Nada } else { ContagemFiltros::Texto("4 de 20 linhas".into()) };
        ui.spacing_mut().item_spacing.y = 0.0;
        let topo = ui.cursor().top();
        let r = barra_filtros(ui, |ui| filtros(ui, &etiquetas, &contagem));
        s.barra = Some(egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), topo..=ui.cursor().top()));
        if let Some((alvo, _)) = r.abrir {
            s.abrir.push(alvo);
        }
        if let Some(id) = r.tirar {
            s.tirar.push(id);
            s.etiquetas.retain(|e| e.0 != id);
        }
        if r.limpar {
            s.limpar += 1;
            s.etiquetas.clear();
        }
    });
    assert!(h.query_by_label("Filtros").is_some());
    assert!(h.query_by_label("4 de 20 linhas").is_some(), "a contagem à direita");
    let barra = h.state().barra.unwrap();
    assert_eq!(barra.height(), ALTURA_BARRA_FILTROS, "`.barra-filtros{{height:38px}}`");
    h.get_by_label("Nova condição").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Editar o filtro regiao igual a Sul").click();
    h.run_steps(2);
    assert_eq!(h.state().abrir, [None, Some(7)], "o + abre uma nova; a etiqueta, a dela");
    h.get_by_label("Limpar filtros").click();
    h.run_steps(2);
    assert_eq!(h.state().limpar, 1);
    assert!(h.query_by_label("Limpar filtros").is_none(), "sem condição, sem o Limpar");
    assert!(h.query_by_label("4 de 20 linhas").is_none());
    h.state_mut().etiquetas = vec![(8, "Norte")];
    h.run_steps(2);
    h.get_by_label("Tirar o filtro regiao igual a Norte").click();
    h.run_steps(2);
    assert_eq!(h.state().tirar, [8]);
}

#[test]
fn popover_da_condicao_aplica_com_valor_e_desiste_com_esc() {
    use interface::pecas::filtros::{CondicaoEmEdicao, FimCondicao, popover_condicao};
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Op {
        Igual,
        Nulo,
    }
    let ops = [(Op::Igual, "igual a"), (Op::Nulo, "é nulo")];
    let c = CondicaoEmEdicao {
        alvo: None,
        coluna: 0,
        op: Op::Igual,
        valor: String::new(),
        ancora: egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(80.0, 26.0)),
        focar: true,
    };
    let mut h = harness((Some(c), Vec::<FimCondicao>::new()), move |ui, s| {
        let Some(c) = s.0.as_mut() else { return };
        let ctx = ui.ctx().clone();
        if let Some(fim) = popover_condicao(&ctx, "teste", c, &[(0, "regiao"), (1, "valor")], &ops, |o| *o == Op::Nulo)
        {
            s.1.push(fim);
        }
    });
    assert!(h.query_by_label("Condição de filtro").is_some());
    assert!(h.get_by_label("Aplicar").accesskit_node().is_disabled(), "sem valor, Aplicar desligado");
    h.get(by().role(Role::TextInput).label("Valor da condição")).type_text("Sul");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().1, [FimCondicao::Aplicar], "Enter no valor aplica");
    assert_eq!(h.state().0.as_ref().unwrap().valor, "Sul");
    // um operador sem valor tira o campo e liga o Aplicar
    h.state_mut().0.as_mut().unwrap().op = Op::Nulo;
    h.run_steps(2);
    assert!(h.query_by_label("Valor da condição").is_none());
    assert!(!h.get_by_label("Aplicar").accesskit_node().is_disabled());
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert_eq!(h.state().1.last(), Some(&FimCondicao::Desistir), "Esc desiste");
}

#[derive(Default)]
struct EstadoJanela {
    aberta: bool,
    nome: String,
    focado: bool,
    pedidos: u32,
    abriu_em_quadros: u32,
    rect: Option<egui::Rect>,
}

fn janela_de_teste(ui: &mut Ui, s: &mut EstadoJanela) {
    if !s.aberta {
        return;
    }
    let ctx = ui.ctx().clone();
    let nome = &mut s.nome;
    let mut focado = false;
    let r = Janela::new("teste", "Nova pessoa")
        .rodape_direita(|ui| {
            ui.add(Botao::primario("Criar"));
        })
        .mostrar(&ctx, |ui| {
            focado = Entrada::new(nome).rotulo("Nome").mostrar(ui).response.response.has_focus();
        });
    s.focado = focado;
    s.rect = Some(r.rect);
    if r.abriu_agora {
        s.abriu_em_quadros += 1;
    }
    if r.pediu_fechar {
        // a tela decide: aqui só conta (a exportação em andamento faria a pergunta)
        s.pedidos += 1;
    }
}

#[test]
fn janela_poe_o_foco_no_primeiro_campo() {
    let mut h = harness(EstadoJanela { aberta: true, ..Default::default() }, janela_de_teste);
    h.run_steps(3);
    assert!(h.state().focado, "o foco nasce no primeiro campo de digitar");
    assert_eq!(h.state().abriu_em_quadros, 1);
    h.get_by_label("Nome").type_text("Ana");
    h.run_steps(2);
    assert_eq!(h.state().nome, "Ana");
}

#[test]
fn janela_esc_e_x_pedem_para_fechar_e_a_tela_decide() {
    let mut h = harness(EstadoJanela { aberta: true, ..Default::default() }, janela_de_teste);
    h.run_steps(2);
    h.key_press(Key::Escape);
    h.run_steps(1);
    assert_eq!(h.state().pedidos, 1, "Esc pede para fechar");
    assert!(h.state().aberta, "quem fecha é a tela");
    h.get_by_label("Fechar").click();
    h.run_steps(1);
    assert_eq!(h.state().pedidos, 2, "o ✕ pede para fechar");
}

#[test]
fn janela_tem_as_medidas_do_dlg() {
    let mut h = harness(EstadoJanela { aberta: true, ..Default::default() }, janela_de_teste);
    h.run_steps(3);
    let janela = h.state().rect.unwrap_or(egui::Rect::NOTHING);
    let fechar = h.get_by_label("Fechar").rect();
    let criar = h.get_by_label("Criar").rect();
    // `.dlg{width:500px}`
    assert_eq!(janela.width(), 500.0);
    // `.dlg header{height:50px;padding:0 10px 0 20px}`: o ✕ no meio dos 49px de cima, 10px antes da borda
    assert_eq!(fechar.center().y - janela.top(), 1.0 + 49.0 / 2.0);
    assert_eq!(janela.right() - fechar.right(), 1.0 + 10.0);
    // `.dlg footer{height:58px;padding:0 14px 0 16px}` com os botões de 30px no meio dos 57px de baixo
    assert_eq!(criar.height(), 30.0);
    assert_eq!(janela.bottom() - criar.center().y, 1.0 + 57.0 / 2.0);
    assert_eq!(janela.right() - criar.right(), 1.0 + 14.0);
    // o vidro com o raio único dos cartões (`.dlg{border-radius:10px}`)
    contorno_com_raio(&h, janela, 10, "janela");
}

#[test]
fn clique_no_fundo_logo_ao_abrir_nao_fecha() {
    // passos de 50ms: o clique (mover, apertar, soltar) leva três passos
    let mut h =
        Harness::builder().with_size(egui::vec2(1000.0, 700.0)).with_step_dt(0.05).with_max_steps(40).build_ui_state(
            |ui, s: &mut EstadoJanela| {
                if pronto(ui) {
                    janela_de_teste(ui, s);
                }
            },
            EstadoJanela::default(),
        );
    h.run_steps(2);
    h.state_mut().aberta = true;
    h.step();
    // clique no fundo logo depois de abrir (o segundo clique de um duplo clique que abriu a janela)
    clicar_em(&h, egui::pos2(10.0, 10.0));
    h.step();
    assert_eq!(h.state().pedidos, 0, "nos primeiros 400ms o clique no fundo não conta");
    h.run_steps(10);
    clicar_em(&h, egui::pos2(10.0, 10.0));
    h.step();
    assert_eq!(h.state().pedidos, 1, "depois disso, o clique no fundo pede para fechar");
}

fn clicar_em<S>(h: &Harness<'_, S>, pos: egui::Pos2) {
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
}

/// A divisória de um painel lateral (`pecas::divisoria`): arrastar, duplo clique, setas, limites, cursor, fio e
/// acessibilidade.
mod divisoria {
    use super::*;
    use interface::pecas::divisoria::{Divisoria, LadoDoPainel, LimitesLargura, PASSO_TECLADO};

    const ROTULO: &str = "Largura do teste";
    /// O vão da divisória nos testes (8px de largura, como entre o painel e o documento).
    fn vao() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(300.0, 50.0), egui::pos2(308.0, 450.0))
    }

    struct Estado {
        lado: LadoDoPainel,
        largura: f32,
        mudancas: u32,
        concluidas: u32,
    }

    fn nova(lado: LadoDoPainel) -> Harness<'static, Estado> {
        harness(Estado { lado, largura: 258.0, mudancas: 0, concluidas: 0 }, |ui, s| {
            let limites = LimitesLargura { atual: s.largura, minima: 200.0, maxima: 400.0, padrao: 258.0 };
            let saida = Divisoria::new("teste", ROTULO, vao(), s.lado, limites).mostrar(ui);
            if saida.mudou {
                s.largura = saida.largura;
                s.mudancas += 1;
            }
            if saida.concluiu {
                s.concluidas += 1;
            }
        })
    }

    fn botao(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
    }

    /// Aperta em `de`, anda até `para` e (se `soltar`) solta.
    fn arrastar(h: &mut Harness<'static, Estado>, de: egui::Pos2, para: egui::Pos2, soltar: bool) {
        h.hover_at(de);
        h.run_steps(2);
        h.event(botao(de, true));
        h.run_steps(1);
        for t in 1..=4 {
            h.hover_at(de + (para - de) * (t as f32 / 4.0));
            h.run_steps(2);
        }
        if soltar {
            h.event(botao(para, false));
            h.run_steps(2);
        }
    }

    fn focar(h: &mut Harness<'static, Estado>) {
        h.get_by_label(ROTULO).focus();
        h.run_steps(2);
    }

    #[test]
    fn arrastar_muda_a_largura_ao_vivo_e_conclui_so_ao_soltar() {
        let mut h = nova(LadoDoPainel::Esquerda);
        let centro = vao().center();
        arrastar(&mut h, centro, centro + egui::vec2(25.0, 0.0), false);
        assert_eq!(h.state().largura, 283.0, "o painel da esquerda alarga para a direita, ao vivo");
        assert_eq!(h.state().concluidas, 0, "ainda apertado: nada a guardar");
        h.event(botao(centro + egui::vec2(25.0, 0.0), false));
        h.run_steps(2);
        assert_eq!((h.state().largura, h.state().concluidas), (283.0, 1));
        // voltar um pouco depois, arrastando para a esquerda
        arrastar(&mut h, centro, centro - egui::vec2(40.0, 0.0), true);
        // (o vão não anda neste teste: o painel é só o estado)
        assert_eq!(h.state().largura, 243.0);
        assert_eq!(h.state().concluidas, 2);
    }

    #[test]
    fn o_painel_da_direita_alarga_para_a_esquerda() {
        let mut h = nova(LadoDoPainel::Direita);
        let centro = vao().center();
        arrastar(&mut h, centro, centro - egui::vec2(30.0, 0.0), true);
        assert_eq!(h.state().largura, 288.0);
        arrastar(&mut h, centro, centro + egui::vec2(20.0, 0.0), true);
        assert_eq!(h.state().largura, 268.0);
    }

    #[test]
    fn a_largura_fica_entre_o_minimo_e_o_maximo_em_pontos_inteiros() {
        let mut h = nova(LadoDoPainel::Esquerda);
        let centro = vao().center();
        arrastar(&mut h, centro, centro + egui::vec2(900.0, 0.0), true);
        assert_eq!(h.state().largura, 400.0);
        arrastar(&mut h, centro, centro - egui::vec2(900.0, 0.0), true);
        assert_eq!(h.state().largura, 200.0);
        arrastar(&mut h, centro, centro + egui::vec2(33.4, 0.0), true);
        assert_eq!(h.state().largura, 233.0, "pontos inteiros");
    }

    #[test]
    fn duplo_clique_volta_a_padrao() {
        let mut h = nova(LadoDoPainel::Esquerda);
        let centro = vao().center();
        arrastar(&mut h, centro, centro + egui::vec2(60.0, 0.0), true);
        assert_eq!(h.state().largura, 318.0);
        let concluidas = h.state().concluidas;
        // dois cliques a 10 ms um do outro (o passo do harness é maior que o prazo do duplo clique)
        let inicio = h.ctx.input(|i| i.time);
        for (n, e) in [
            egui::Event::PointerMoved(centro),
            botao(centro, true),
            botao(centro, false),
            botao(centro, true),
            botao(centro, false),
        ]
        .into_iter()
        .enumerate()
        {
            h.input_mut().time = Some(inicio + 0.01 * (n + 1) as f64);
            h.event(e);
            h.step();
        }
        h.input_mut().time = None;
        h.run_steps(3);
        assert_eq!(h.state().largura, 258.0);
        assert_eq!(h.state().concluidas, concluidas + 1);
    }

    #[test]
    fn setas_home_end_e_enter_com_o_foco() {
        let mut h = nova(LadoDoPainel::Esquerda);
        h.remove_cursor();
        focar(&mut h);
        h.key_press(Key::ArrowRight);
        h.run_steps(2);
        assert_eq!(h.state().largura, 258.0 + PASSO_TECLADO);
        h.key_press(Key::ArrowLeft);
        h.key_press(Key::ArrowLeft);
        h.run_steps(2);
        assert_eq!(h.state().largura, 258.0 - PASSO_TECLADO);
        assert_eq!(
            h.state().concluidas,
            3,
            "cada toque conclui (um quadro por tecla no harness): é o passo que se guarda"
        );
        h.key_press(Key::Home);
        h.run_steps(2);
        assert_eq!(h.state().largura, 200.0);
        h.key_press(Key::ArrowLeft);
        h.run_steps(2);
        assert_eq!(h.state().largura, 200.0, "no mínimo a seta não passa");
        h.key_press(Key::End);
        h.run_steps(2);
        assert_eq!(h.state().largura, 400.0);
        h.key_press(Key::Enter);
        h.run_steps(2);
        assert_eq!(h.state().largura, 258.0, "o Enter volta ao padrão");
        // o foco ficou na divisória o tempo todo (as setas não o levaram ao vizinho)
        assert!(h.get_by_label(ROTULO).is_focused());
    }

    #[test]
    fn no_painel_da_direita_a_seta_para_a_esquerda_alarga() {
        let mut h = nova(LadoDoPainel::Direita);
        h.remove_cursor();
        focar(&mut h);
        h.key_press(Key::ArrowLeft);
        h.run_steps(2);
        assert_eq!(h.state().largura, 258.0 + PASSO_TECLADO);
        h.key_press(Key::ArrowRight);
        h.run_steps(2);
        assert_eq!(h.state().largura, 258.0);
    }

    #[test]
    fn o_mouse_nao_deixa_o_foco_na_divisoria() {
        let mut h = nova(LadoDoPainel::Esquerda);
        let centro = vao().center();
        arrastar(&mut h, centro, centro + egui::vec2(20.0, 0.0), true);
        assert!(!h.get_by_label(ROTULO).is_focused(), "sem o foco, a página rola pelo teclado e as setas não são dela");
    }

    #[test]
    fn o_papel_de_divisoria_com_valor_e_limites() {
        let mut h = nova(LadoDoPainel::Esquerda);
        let no = h.get(by().role(Role::Splitter).label(ROTULO));
        let n = no.accesskit_node();
        assert_eq!(
            (n.numeric_value(), n.min_numeric_value(), n.max_numeric_value()),
            (Some(258.0), Some(200.0), Some(400.0))
        );
        assert_eq!(n.value().as_deref(), Some("258 pontos"));
        assert_eq!(no.rect(), vao());
        assert!(n.data().supports_action(egui::accesskit::Action::Increment));
        assert!(n.data().supports_action(egui::accesskit::Action::Decrement));
        h.remove_cursor();
        focar(&mut h);
        h.key_press(Key::End);
        h.run_steps(2);
        let n = h.get(by().role(Role::Splitter).label(ROTULO));
        assert!(!n.accesskit_node().data().supports_action(egui::accesskit::Action::Increment), "no máximo");
    }

    /// O fio de 2px no meio do vão que o último quadro pintou, se pintou.
    fn fio<S>(h: &Harness<'_, S>) -> Option<(egui::Rect, egui::Color32)> {
        fn achar(forma: &egui::Shape) -> Option<(egui::Rect, egui::Color32)> {
            match forma {
                egui::Shape::Vec(formas) => formas.iter().find_map(achar),
                egui::Shape::Rect(r) if r.rect.width() == 2.0 && r.rect.height() > 300.0 => Some((r.rect, r.fill)),
                _ => None,
            }
        }
        h.output().shapes.iter().find_map(|c| achar(&c.shape))
    }

    #[test]
    fn cursor_e_fio_so_com_o_ponteiro_ou_o_foco() {
        let mut h = nova(LadoDoPainel::Esquerda);
        h.remove_cursor();
        h.run_steps(3);
        assert!(fio(&h).is_none(), "em repouso a divisória é invisível");
        assert_ne!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal);
        // o ponteiro em cima: o cursor de redimensionar e o fio de 2px no meio do vão, em destaque e translúcido
        h.hover_at(vao().center());
        h.run_steps(3);
        assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal);
        let (rect, cor) = fio(&h).expect("o fio com o ponteiro em cima");
        assert_eq!((rect.left(), rect.right()), (303.0, 305.0));
        assert_eq!(rect.center().y, vao().center().y);
        assert!(rect.height() < vao().height(), "o fio não encosta nas quinas");
        let acc = interface::tema::CLARA.estudio.acc;
        let [r, g, b, a] = cor.to_srgba_unmultiplied();
        let perto = |x: u8, y: u8| x.abs_diff(y) <= 6;
        assert!(perto(r, acc.r()) && perto(g, acc.g()) && perto(b, acc.b()), "na cor de destaque: {cor:?} {acc:?}");
        assert!(a > 0 && a < 128, "translúcido: {a}");
        // o ponteiro some: o fio também
        h.remove_cursor();
        h.run_steps(3);
        assert!(fio(&h).is_none());
        // o foco do teclado: o fio firme (mais opaco que no hover)
        focar(&mut h);
        h.key_press(Key::ArrowRight);
        h.run_steps(3);
        let (_, firme) = fio(&h).expect("o fio com o foco do teclado");
        assert!(firme.to_srgba_unmultiplied()[3] > a, "{firme:?} mais opaco que {cor:?}");
    }

    #[test]
    fn parada_nao_pede_repintura() {
        let mut h = nova(LadoDoPainel::Esquerda);
        h.hover_at(vao().center());
        h.run_steps(5);
        h.remove_cursor();
        // (o tempo do harness é o simulado: cada quadro avança 0,25 s e a transição do fio dura 0,2 s)
        h.run_steps(5);
        for _ in 0..5 {
            h.step();
            assert!(!h.ctx.has_requested_repaint(), "{:?}", h.ctx.repaint_causes());
        }
    }
}

#[derive(Default)]
struct EstadoConfirmar {
    pergunta: Option<Confirmacao>,
    resposta: Option<bool>,
}

#[test]
fn confirmar_sim_pelo_botao_nao_pelo_esc() {
    let mut h = harness(EstadoConfirmar::default(), |ui, s| {
        let ctx = ui.ctx().clone();
        if let Some(r) = confirmar(&ctx, &mut s.pergunta) {
            s.resposta = Some(r);
        }
    });
    h.state_mut().pergunta =
        Some(Confirmacao::new("Excluir as 3 linhas marcadas?").titulo("Excluir linhas").rotulo("Excluir").perigo(true));
    h.run_steps(2);
    h.get_by_label("Excluir").click();
    h.run_steps(2);
    assert_eq!(h.state().resposta, Some(true));
    assert!(h.state().pergunta.is_none());

    h.state_mut().resposta = None;
    h.state_mut().pergunta = Some(Confirmacao::new("Continuar?"));
    h.run_steps(2);
    assert!(h.query_by_label("Continuar").is_some(), "o botão padrão é \"Continuar\"");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert_eq!(h.state().resposta, Some(false));
}

#[derive(Default)]
struct EstadoDecisao {
    pergunta: Option<Confirmacao>,
    decisao: Option<interface::pecas::janela::Decisao>,
}

#[test]
fn confirmar_tem_cancelar_antes_da_acao_e_a_segunda_saida_a_esquerda() {
    use interface::pecas::janela::{Decisao, confirmar_decisao};
    let mut h = harness(EstadoDecisao::default(), |ui, s| {
        let ctx = ui.ctx().clone();
        if let Some(d) = confirmar_decisao(&ctx, &mut s.pergunta) {
            s.decisao = Some(d);
        }
    });
    // "Cancelar" (ghost, antes da ação) tem o mesmo efeito do ✕: desistir
    h.state_mut().pergunta =
        Some(Confirmacao::new("Excluir a coluna?").titulo("Excluir coluna").rotulo("Excluir").perigo(true));
    h.run_steps(2);
    let (cancelar, acao) = (h.get_by_label("Cancelar").rect(), h.get_by_label("Excluir").rect());
    assert!(cancelar.right() < acao.left(), "Cancelar vem antes da ação: {cancelar:?} {acao:?}");
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    assert_eq!(h.state().decisao, Some(Decisao::Cancelou));
    assert!(h.state().pergunta.is_none());
    // a segunda saída ("Não salvar") fica à esquerda do rodapé, e só vale o que ela é
    h.state_mut().decisao = None;
    h.state_mut().pergunta = Some(
        Confirmacao::new("Há 1 alteração não salva.")
            .titulo("Salvar as alterações em vendas_2025.csv?")
            .rotulo("Salvar")
            .descartar("Não salvar"),
    );
    h.run_steps(2);
    let (descartar, cancelar, salvar) =
        (h.get_by_label("Não salvar").rect(), h.get_by_label("Cancelar").rect(), h.get_by_label("Salvar").rect());
    assert!(descartar.right() < cancelar.left() && cancelar.right() < salvar.left(), "{descartar:?} {cancelar:?}");
    h.get_by_label("Não salvar").click();
    h.run_steps(2);
    assert_eq!(h.state().decisao, Some(Decisao::Descartou));
    h.state_mut().decisao = None;
    h.state_mut().pergunta =
        Some(Confirmacao::new("Há 1 alteração não salva.").rotulo("Salvar").descartar("Não salvar"));
    h.run_steps(2);
    h.get_by_label("Salvar").click();
    h.run_steps(2);
    assert_eq!(h.state().decisao, Some(Decisao::Confirmou));
}

#[test]
fn janela_nao_mostra_o_anel_de_foco_ao_abrir_so_depois_de_uma_tecla() {
    use interface::pecas::janela::Janela;
    // a tecla que abriu a janela ainda está nos eventos do quadro em que ela abre: sem anel; uma tecla depois, com
    let mut h = harness((false, false), |ui, s| {
        let ctx = ui.ctx().clone();
        if s.0 {
            Janela::new("anel", "Janela").mostrar(&ctx, |ui| {
                ui.label("corpo");
            });
        }
        s.1 = interface::pintura::foco_visivel(&ctx);
    });
    h.key_press(Key::Enter);
    h.state_mut().0 = true;
    h.run_steps(3);
    assert!(!h.state().1, "o foco dado ao abrir não ganha o anel");
    h.key_press(Key::Tab);
    h.step();
    assert!(h.state().1, "depois de uma tecla na janela, o anel volta");
}

#[derive(Default)]
struct EstadoCampos {
    perfil: u8,
    marcado: bool,
    mudou: u32,
}

#[test]
fn select_troca_pelas_setas_e_pela_lista_e_check_alterna() {
    let mut h = harness(EstadoCampos::default(), |ui, s| {
        if Select::new("perfil", &mut s.perfil, &[(0, "gestor"), (1, "rh"), (2, "admin")])
            .rotulo("Perfil")
            .mostrar(ui)
            .changed()
        {
            s.mudou += 1;
        }
        check(ui, &mut s.marcado, "Abrir só para leitura");
    });
    h.get_by_label("Perfil").focus();
    h.run_steps(1);
    h.key_press(Key::ArrowDown);
    h.run_steps(1);
    assert_eq!(h.state().perfil, 1, "seta para baixo escolhe a próxima");
    h.get_by_label("Perfil").click();
    h.run_steps(2);
    h.get_by_label("admin").click();
    h.run_steps(2);
    assert_eq!(h.state().perfil, 2, "a lista escolhe a opção clicada");
    assert_eq!(h.state().mudou, 2);
    h.get_by_label("Abrir só para leitura").click();
    h.run_steps(1);
    assert!(h.state().marcado);
}

#[derive(Default)]
struct MedidasCampos {
    alturas: Vec<f32>,
    larguras_caixas: Vec<f32>,
    textos: Vec<f32>,
    perfil: u8,
    marcado: bool,
}

#[test]
fn campos_tem_28px_busca_30px_e_caixas_de_16px() {
    use interface::pecas::campos::{ALTURA_BUSCA, ALTURA_CAMPO, radio};
    let mut h = harness(MedidasCampos::default(), |ui, s| {
        let (mut a, mut b) = (String::new(), String::new());
        s.alturas = vec![
            ui.add(Entrada::new(&mut a).rotulo("Nome")).rect.height(),
            ui.add(Entrada::new(&mut b).busca(true).rotulo("Buscar")).rect.height(),
            Select::new("perfil", &mut s.perfil, &[(0, "gestor"), (1, "rh"), (2, "admin")])
                .rotulo("Perfil")
                .mostrar(ui)
                .rect
                .height(),
        ];
        let fonte = fontes::fonte(12.0, fontes::Peso::Normal);
        let medir =
            |t: &str| interface::pecas::texto::galley(ui, t, fonte.clone(), egui::Color32::PLACEHOLDER).size().x;
        s.textos = vec![medir("Abrir só para leitura"), medir("A tabela inteira")];
        s.larguras_caixas = vec![
            check(ui, &mut s.marcado, "Abrir só para leitura").rect.width(),
            radio(ui, true, "A tabela inteira").rect.width(),
        ];
    });
    h.run_steps(1);
    let s = h.state();
    // `.campo{height:28px}`, `.busca{height:30px}`, o `button.campo` do select com os mesmos 28px
    assert_eq!(s.alturas, [ALTURA_CAMPO, ALTURA_BUSCA, ALTURA_CAMPO]);
    assert_eq!((ALTURA_CAMPO, ALTURA_BUSCA), (28.0, 30.0));
    // `input[type=checkbox]` e `[type=radio]` de 16px, `.op{gap:8px}` e o texto de 12px
    assert_eq!(s.larguras_caixas[0], (16.0 + 8.0 + s.textos[0]).ceil());
    assert_eq!(s.larguras_caixas[1], (16.0 + 8.0 + s.textos[1]).ceil());
    // a lista abre no vidro dos menus, com os itens `.mi` de 28px
    h.get_by_label("Perfil").click();
    h.run_steps(2);
    assert_eq!(h.get_by_label("admin").rect().height(), 28.0, "`.mi{{height:28px}}`");
}

#[derive(Default)]
struct Formas {
    /// (peça, onde ficou, o raio esperado nos cantos do contorno)
    pecas: Vec<(&'static str, egui::Rect, u8)>,
    perfil: u8,
    textos: [String; 4],
}

#[test]
fn raio_unico_nas_superficies_e_campos_caixas_quadradas_e_pilula_nos_botoes() {
    use interface::pecas::campos::{AreaTexto, Check, radio};
    use interface::pecas::cartao::Cartao;
    use interface::pecas::navegacao::trilho;
    let mut h = harness(Formas::default(), |ui, s| {
        let mut pecas = Vec::new();
        let r = Cartao::new("formas").titulo("Formas").mostrar(ui, |c| {
            c.corpo(|ui| {
                let [a, b, c, d] = &mut s.textos;
                // os campos com o raio único de 10, também na busca de 30px, no select e na área de texto
                pecas.push(("campo", ui.add(Entrada::new(a).rotulo("Nome")).rect, 10));
                pecas.push(("busca", ui.add(Entrada::new(b).busca(true).rotulo("Buscar")).rect, 10));
                let select = Select::new("perfil", &mut s.perfil, &[(0, "gestor"), (1, "rh")]).mostrar(ui).rect;
                pecas.push(("select", select, 10));
                pecas.push(("área de texto", ui.add(AreaTexto::new(c)).rect, 10));
                // o campo da edição no lugar continua com 4
                pecas.push(("campo compacto", ui.add(Entrada::new(d).compacta().largura(140.0)).rect, 4));
                // caixas de marcar quadradas de 16px com cantos de 5px (sem texto, a caixa é a resposta inteira)
                let (mut sim, mut nao, mut parte) = (true, false, false);
                pecas.push(("marcada", Check::new(&mut sim, "").mostrar(ui).rect, 5));
                pecas.push(("desmarcada", Check::new(&mut nao, "").mostrar(ui).rect, 5));
                pecas.push(("indeterminada", Check::new(&mut parte, "").indeterminado(true).mostrar(ui).rect, 5));
                // a opção continua redonda: a caixa de 16px à esquerda, no meio da linha do texto
                let o = radio(ui, true, "A tabela inteira").rect;
                let opcao = egui::Rect::from_min_size(egui::pos2(o.left(), o.center().y - 8.0), egui::vec2(16.0, 16.0));
                pecas.push(("opção", opcao, 8));
                // botões e o trilho do segmentado continuam em pílula (meia altura)
                pecas.push(("botão", ui.add(Botao::new("Salvar")).rect, 14));
                let t = trilho(ui, egui::Id::new("t"), &["Excel", "CSV"], Some(0), TamanhoTrilho::Normal, true);
                pecas.push(("trilho do segmentado", t.1.rect, 16));
            });
        });
        // o cartão (documento) com o raio único
        pecas.push(("cartão", r.response.rect, 10));
        s.pecas = pecas;
    });
    h.run_steps(1);
    let pecas = h.state().pecas.clone();
    assert_eq!(pecas.len(), 12);
    for (peca, rect, raio) in pecas {
        if raio == 5 {
            assert_eq!(rect.size(), egui::vec2(16.0, 16.0), "{peca}");
        }
        contorno_com_raio(&h, rect, raio, peca);
    }
}

#[derive(Default)]
struct FormasCasca {
    pecas: Vec<(&'static str, egui::Rect, u8)>,
    toast: Option<egui::Rect>,
}

#[test]
fn aviso_painel_e_status_com_o_raio_unico_e_toast_em_pilula() {
    use interface::pecas::avisos::Aviso;
    use interface::pecas::barra_status::{ItemStatus, barra_status};
    use interface::pecas::painel::Painel;
    use interface::pecas::toast::rect_toast;
    let mut h = harness(FormasCasca::default(), |ui, s| {
        let mut pecas = Vec::new();
        // o aviso dentro de cartões e janelas: o raio único (decisão de 16/10/2026; antes, 8)
        let aviso = Aviso::new(false).mostrar(ui, |ui, _| {
            ui.label("Há uma transação aberta na Consulta 1.");
        });
        pecas.push(("aviso", aviso.rect, 10));
        // a barra de status de 26px com 10, e não a pílula da meia altura
        let status = barra_status(ui, &[ItemStatus::new("exemplo.db")], &[ItemStatus::new("UTF-8")]);
        pecas.push(("barra de status", status.rect, 10));
        // o painel fixo com o cabeçalho (o Explorador)
        let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(258.0, 200.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r));
        Painel::new("explorador", "Explorador").mostrar(&mut filho, |_| {});
        pecas.push(("painel", r, 10));
        s.pecas = pecas;
        desenhar_toast(ui.ctx(), f32::INFINITY);
        s.toast = rect_toast(ui.ctx());
    });
    toast(&h.ctx, "Copiado.");
    h.run_steps(2);
    let pecas = h.state().pecas.clone();
    assert_eq!(pecas.len(), 3);
    for (peca, rect, raio) in pecas {
        contorno_com_raio(&h, rect, raio, peca);
    }
    // o toast em pílula: o contorno do vidro de 37px (`padding:9px 16px`) com a meia altura nos cantos
    let alvo = h.state().toast.expect("o toast à vista");
    let mut cantos = Vec::new();
    for c in &h.output().shapes {
        if let egui::Shape::Vec(formas) = &c.shape {
            for f in formas {
                if let egui::Shape::Rect(r) = f
                    && r.stroke.width > 0.0
                    && (r.rect.min - alvo.min).length() < 1.0
                    && (r.rect.size() - alvo.size()).length() < 0.01
                {
                    cantos.push(r.corner_radius);
                }
            }
        }
    }
    assert_eq!(alvo.height(), 37.0);
    assert_eq!(cantos, vec![egui::CornerRadius::same(19)], "toast em pílula");
}

/// O toast com ação (`.toast.com-acao`): o botão pequeno dentro da pílula, a dica com o caminho inteiro, a ação que
/// sai uma vez só e o toast que some ao clicar; dura mais que o toast sem ação.
#[test]
fn toast_com_acao_tem_o_botao_dentro_e_devolve_a_carga_uma_vez() {
    use interface::pecas::toast::{
        AcaoToast, DURACAO_TOAST, DURACAO_TOAST_COM_ACAO, acao_do_toast, dica_do_toast, rect_toast,
        rotulo_da_acao_do_toast, toast_concluido_com_acao,
    };
    let mut h = harness(None::<egui::Rect>, |ui, s| {
        desenhar_toast(ui.ctx(), f32::INFINITY);
        *s = rect_toast(ui.ctx());
    });
    let caminho = r"C:\dados\copias\noticias (cópia).db";
    toast_concluido_com_acao(
        &h.ctx,
        "Cópia salva: noticias (cópia).db · 119 MB",
        Some(caminho.to_string()),
        AcaoToast { rotulo: "Mostrar na pasta".into(), carga: caminho.into() },
    );
    h.run_steps(2);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Cópia salva: noticias (cópia).db · 119 MB"));
    assert_eq!(rotulo_da_acao_do_toast(&h.ctx).as_deref(), Some("Mostrar na pasta"));
    assert_eq!(dica_do_toast(&h.ctx).as_deref(), Some(caminho));
    // o botão cabe dentro da pílula, do mesmo tamanho de sempre
    let pilula = h.state().expect("o toast à vista");
    let botao = h.get(by().role(Role::Button).label("Mostrar na pasta")).rect();
    assert!(pilula.contains_rect(botao), "{botao:?} fora de {pilula:?}");
    assert_eq!((pilula.height(), botao.height()), (37.0, 24.0));
    assert!(acao_do_toast(&h.ctx).is_none(), "sem clique, nada");
    // dura mais que o sem ação (7 s contra 2,6 s)
    const { assert!(DURACAO_TOAST_COM_ACAO > DURACAO_TOAST) };
    // o clique devolve a carga uma vez só, e o toast some
    h.get(by().role(Role::Button).label("Mostrar na pasta")).click();
    h.run_steps(2);
    assert_eq!(acao_do_toast(&h.ctx).as_deref(), Some(caminho));
    assert!(acao_do_toast(&h.ctx).is_none(), "a chamada consome");
    assert!(toast_atual(&h.ctx).is_none(), "cumprida a ação, o toast sai");
}

/// O toast com ação por outro prazo (o "Desfazer · Ctrl+Z" da gravação no banco, 6 s na prancheta 15): à vista
/// até o prazo e fora depois da saída.
#[test]
fn toast_com_acao_por_um_prazo() {
    use interface::pecas::toast::{AcaoToast, rotulo_da_acao_do_toast, toast_concluido_com_acao_por};
    let mut h = harness((), |ui, _| desenhar_toast(ui.ctx(), f32::INFINITY));
    h.input_mut().time = Some(0.0);
    h.step();
    toast_concluido_com_acao_por(
        &h.ctx,
        "Gravado em clientes (linha 3, email)",
        AcaoToast { rotulo: "Desfazer · Ctrl+Z".into(), carga: "desfazer".into() },
        6.0,
    );
    h.input_mut().time = Some(5.9);
    h.step();
    assert_eq!(rotulo_da_acao_do_toast(&h.ctx).as_deref(), Some("Desfazer · Ctrl+Z"));
    h.input_mut().time = Some(6.5);
    h.step();
    assert!(toast_atual(&h.ctx).is_none(), "passado o prazo e a saída, o toast some");
    h.input_mut().time = None;
}

#[test]
fn toast_tem_os_recuos_das_pranchetas() {
    use interface::pecas::toast::rect_toast;
    // `.toast{padding:9px 16px;gap:8px}` na pílula (pranchetas 8 e 15): 16px dos dois lados, por dentro da borda de
    // 1px, o sólido de 16px e o texto
    let mut h = harness((), |ui, _| desenhar_toast(ui.ctx(), f32::INFINITY));
    toast(&h.ctx, "Copiado.");
    h.run_steps(2);
    let rect = rect_toast(&h.ctx).expect("o toast à vista");
    let texto = h.ctx.fonts_mut(|f| {
        f.layout_no_wrap("Copiado.".into(), fontes::fonte(12.0, fontes::Peso::Normal), egui::Color32::WHITE).size().x
    });
    assert_eq!(rect.width(), (1.0 + 16.0 + 16.0 + 8.0 + texto + 16.0 + 1.0).ceil());
    assert_eq!(rect.height(), 1.0 + 9.0 + 17.0 + 9.0 + 1.0, "a linha de 16,8px arredondada");
}

#[test]
fn degrade_principal_para_em_25() {
    use interface::pintura::{cor_no_degrade, paradas_primario};
    // `--volpri: linear-gradient(… 72% branco, var(--acc) 25%, … 80% preto)` (revisão, item 6): a cor de destaque
    // já a um quarto da altura, e não mais na metade
    for p in [&tema::CLARA, &tema::ESCURA] {
        let paradas = paradas_primario(p);
        assert_eq!(cor_no_degrade(&paradas, 0.25), p.estudio.acc);
        assert_eq!(cor_no_degrade(&paradas, 0.0), p.estudio.pri_topo);
        assert_eq!(cor_no_degrade(&paradas, 1.0), p.estudio.pri_base);
        assert_ne!(cor_no_degrade(&paradas, 0.5), p.estudio.acc, "na metade já escurece");
    }
}

#[test]
fn campos_com_a_borda_fbc() {
    use interface::pecas::campos::cor_borda_campo;
    use interface::tema::rgba;
    // `--fbc` (revisão, item 2): .40 de branco no escuro e .46 de preto no claro; o hover é o
    // `color-mix(in oklab, --acc 45%, --fbc)` resolvido pelo tokens_oklab.py, e o foco, o destaque
    let (escuro, claro) = (&tema::ESCURA.estudio, &tema::CLARA.estudio);
    assert_eq!(cor_borda_campo(escuro, 0.0, 0.0), rgba(255, 255, 255, 0.40));
    assert_eq!(cor_borda_campo(claro, 0.0, 0.0), rgba(0, 0, 0, 0.46));
    assert_eq!(cor_borda_campo(escuro, 1.0, 0.0), rgba(114, 160, 235, 0.67));
    assert_eq!(cor_borda_campo(claro, 1.0, 0.0), rgba(19, 58, 120, 0.703));
    for e in [escuro, claro] {
        assert_eq!(cor_borda_campo(e, 0.0, 1.0), e.acc);
        assert_eq!(cor_borda_campo(e, 1.0, 1.0), e.acc);
        // o `--fb` continua nos trilhos, nas barras e nos campos desligados
        assert_ne!(e.fbc, e.fb);
    }
}

#[test]
fn pill_ok_escurece_no_claro() {
    use interface::pecas::rotulos::{TipoPill, cores_pill};
    // `.est.claro .pill.ok{color:#0b6a41}` (revisão, item 30: 4,78:1); no escuro, o `--ok` de sempre
    assert_eq!(cores_pill(&tema::CLARA, TipoPill::Ok).0, tema::rgb(0x0B, 0x6A, 0x41));
    assert_eq!(cores_pill(&tema::ESCURA, TipoPill::Ok).0, tema::ESCURA.estudio.ok);
}

#[test]
fn raio_da_barra_de_status_e_o_padrao() {
    use interface::pecas::barra_status::{ItemStatus, RAIO_STATUS, barra_status};
    // `border-radius:10px` (revisão, item 5): com os 13 da meia altura, a barra virava pílula
    assert_eq!(RAIO_STATUS, interface::pintura::RAIO_PADRAO);
    let mut h = harness(None, |ui, s: &mut Option<egui::Rect>| {
        *s = Some(barra_status(ui, &[ItemStatus::new("exemplo.db")], &[ItemStatus::new("UTF-8")]).rect);
    });
    h.run_steps(1);
    let rect = h.state().expect("a barra");
    contorno_com_raio(&h, rect, RAIO_STATUS as u8, "barra de status");
}

#[test]
fn terciario_vira_secundario_no_relevo_e_na_selecao() {
    use interface::pecas::abas_documento::cor_sub_aba;
    use interface::pecas::arvore::cor_contagem_no;
    use interface::pecas::casca::cor_atalho_menu;
    use interface::pecas::painel::cor_titulo_cab;
    // `.cab .tit, .aba.on .sub, .no.on .q, .mi:hover .at, .mi.on .at {color:var(--t2)}` (revisão, item 1)
    for p in [&tema::CLARA, &tema::ESCURA] {
        let (t2, t3) = (p.estudio.t2, p.estudio.t3);
        assert_eq!(cor_titulo_cab(p), t2);
        assert_eq!((cor_sub_aba(p, true), cor_sub_aba(p, false)), (t2, t3));
        assert_eq!((cor_contagem_no(p, true), cor_contagem_no(p, false)), (t2, t3));
        assert_eq!((cor_atalho_menu(p, true), cor_atalho_menu(p, false)), (t2, t3));
    }
}

#[derive(Default)]
struct Avisos {
    esquerda: f32,
    disponivel: f32,
    curto: Option<egui::Rect>,
    longo: Option<egui::Rect>,
    faixa: Option<egui::Rect>,
    estreito: Option<egui::Rect>,
}

#[test]
fn aviso_em_caixa_tem_a_largura_do_texto_e_fica_centrado() {
    use interface::pecas::avisos::{LARGURA_MAX_AVISO, TomAviso, aviso_faixa, aviso_tom, caixa_do_aviso};
    use interface::pecas::texto::Trecho;
    const LONGO: &str = "A tabela será reconstruída: os dados são copiados para uma tabela nova com a definição \
                         alterada, e os índices, gatilhos e visões ligados a ela são recriados.";
    let mut h = harness(Avisos::default(), |ui, s| {
        s.esquerda = ui.cursor().min.x;
        s.disponivel = ui.available_width();
        let curto = aviso_tom(ui, &[Trecho::Normal("Copiado.")], TomAviso::Concluido);
        s.curto = Some(curto.resposta.rect);
        s.longo = Some(aviso_tom(ui, &[Trecho::Normal(LONGO)], TomAviso::Atencao).resposta.rect);
        s.faixa = Some(aviso_faixa(ui, &[Trecho::Normal("Há uma transação aberta.")], TomAviso::Atencao).resposta.rect);
        let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(300.0, 200.0));
        let mut filho =
            ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
        s.estreito = Some(aviso_tom(&mut filho, &[Trecho::Normal(LONGO)], TomAviso::Atencao).resposta.rect);
    });
    h.run_steps(2);
    let s = h.state();
    let (curto, longo, faixa, estreito) = (s.curto.unwrap(), s.longo.unwrap(), s.faixa.unwrap(), s.estreito.unwrap());
    let meio = s.esquerda + s.disponivel / 2.0;
    // a frase curta: a caixa com a largura dela (`width:fit-content`), no meio (`margin-inline:auto`)
    assert!(curto.width() < 200.0, "{curto:?}");
    assert!((curto.center().x - meio).abs() <= 0.5, "{curto:?} no meio de {}", s.disponivel);
    // a comprida: até 560px, no meio, e o texto quebra
    assert_eq!(longo.width(), LARGURA_MAX_AVISO);
    assert!((longo.center().x - meio).abs() <= 0.5);
    assert!(longo.height() > curto.height(), "o texto quebrou");
    // a faixa do topo do documento continua de ponta a ponta
    assert_eq!(faixa.width(), s.disponivel);
    // num espaço estreito, a largura toda
    assert_eq!(estreito.width(), 300.0);
    // a conta
    // `padding:9px 12px` + o fio, o sólido de 24 e o `gap:12px`
    assert_eq!(caixa_do_aviso(900.0, 100.0, false), (13.0 + 24.0 + 12.0 + 100.0 + 13.0, (900.0 - 162.0) / 2.0));
    assert_eq!(caixa_do_aviso(900.0, 1000.0, false), (560.0, 170.0));
    assert_eq!(caixa_do_aviso(300.0, 1000.0, false), (300.0, 0.0));
    assert_eq!(caixa_do_aviso(900.0, 100.0, true), (900.0, 0.0));
}

#[test]
fn avisos_cheios_tem_a_mesma_largura_e_o_link_vai_na_ponta_direita() {
    use interface::pecas::avisos::{Aviso, CoresAviso, TomAviso, texto_com_link_a_direita};
    use interface::pecas::texto::{Trecho, texto_rico};
    let mut h = harness(Vec::<egui::Rect>::new(), |ui, s| {
        s.clear();
        for texto in ["Curto.", "Um aviso bem mais comprido que o outro, para a largura do conteúdo ser outra mesmo."]
        {
            let r = Aviso::com_tom(TomAviso::Erro).cheia(true).mostrar(ui, |ui, cores: CoresAviso| {
                texto_rico(ui, &[Trecho::Normal(texto)], 12.5, cores.texto, cores.forte);
            });
            s.push(r.rect);
        }
        // a faixa do documento com o link à direita (prancheta 6)
        let faixa = Aviso::com_tom(TomAviso::Atencao).ponta_a_ponta(true).mostrar(ui, |ui, cores| {
            texto_com_link_a_direita(ui, &[Trecho::Forte("Há uma transação aberta.")], "Ir para a Consulta 1", cores);
        });
        s.push(faixa.rect);
    });
    h.run_steps(3);
    let r = h.state().clone();
    assert_eq!(r[0].width(), r[1].width(), "a mesma largura, qualquer que seja o texto");
    assert_eq!(r[0].width(), 560.0);
    assert_eq!(r[0].left(), r[1].left(), "o sólido na mesma coluna");
    let link = h.get(by().role(Role::Link).label("Ir para a Consulta 1")).rect();
    // `padding:9px 12px` + a borda: o link encosta nos 13px da direita da caixa
    assert!((r[2].right() - 13.0 - link.right()).abs() < 1.5, "{:?} {link:?}", r[2]);
    assert!(link.left() > r[2].center().x, "o link está na metade da direita");
}

#[test]
fn tabela_com_recuo_lateral_soma_as_pontas() {
    use interface::pecas::tabela::{ColunaTabela, Tabela};
    let mut h = harness((0.0_f32, 0.0_f32), |ui, s| {
        let origem = ui.cursor().min.x;
        let colunas = [ColunaTabela::new("Nome"), ColunaTabela::new("Origem")];
        Tabela::new("recuo", &colunas, 1).recuo_lateral(3.0).mostrar(ui, |ui, _, c| {
            if c == 0 {
                s.0 = ui.max_rect().left() - origem;
            } else {
                s.1 = ui.max_rect().right();
            }
        });
        let direita = ui.min_rect().right();
        s.1 = direita - s.1;
    });
    h.run_steps(3);
    // `.lista-t th{padding:0 12px}` e os 3px a mais, na esquerda da primeira coluna e na direita da última
    assert_eq!(h.state().0, 12.0 + 3.0);
    assert!((h.state().1 - 15.0).abs() < 0.5, "{}", h.state().1);
}

#[derive(Default)]
struct CabecalhoPainel {
    painel: Option<egui::Rect>,
    botao: Option<egui::Rect>,
}

#[test]
fn cabecalho_do_painel_tem_23px_com_botoes_de_20() {
    use interface::pecas::arvore::MARGEM_NO;
    use interface::pecas::painel::{ALTURA_CAB, Painel, RECUO_CAB, TAMANHO_TITULO_CAB};
    // `.cab{height:23px;margin:6px 6px 0;padding:0 4px 0 12px}`, `.cab .ib{width:20px;height:20px}`, `.cab .tit{font-size:10.5px}`
    // (pedido de 16/10/2026: 0,6 dos 38 de antes)
    let mut h = harness(CabecalhoPainel::default(), |ui, s| {
        let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(258.0, 200.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r));
        let mut botao = None;
        Painel::new("explorador", "Explorador")
            .acoes(|ui| botao = Some(ui.add(Botao::icone(Icone::Recolher).do_cabecalho().dica("Recolher tudo")).rect))
            .mostrar(&mut filho, |_| {});
        s.painel = Some(r);
        s.botao = botao;
    });
    h.run_steps(1);
    let (painel, botao) = (h.state().painel.unwrap(), h.state().botao.unwrap());
    assert_eq!((ALTURA_CAB, TAMANHO_TITULO_CAB), (23.0, 10.5));
    assert_eq!(botao.size(), egui::vec2(20.0, 20.0));
    // a faixa tem as bordas da linha da árvore: a borda do painel mais a margem do nó, nos lados e em cima
    assert_eq!(RECUO_CAB, 1.0 + MARGEM_NO);
    let faixa_esq = painel.left() + RECUO_CAB;
    let faixa_dir = painel.right() - RECUO_CAB;
    let faixa_topo = painel.top() + RECUO_CAB;
    // o botão no meio da faixa, a 4px da direita dela
    assert_eq!(botao.right(), faixa_dir - 4.0);
    assert_eq!(botao.center().y, faixa_topo + ALTURA_CAB / 2.0);
    // o título a 12px da esquerda da faixa
    let titulo = h.get_by_label("Explorador").rect();
    assert_eq!(titulo.left(), faixa_esq + 12.0);
}

#[derive(Default)]
struct FaixaEArvore {
    painel: Option<egui::Rect>,
    no: Option<egui::Rect>,
}

#[test]
fn faixa_do_cabecalho_tem_as_bordas_da_linha_da_arvore() {
    use interface::pecas::arvore::No;
    use interface::pecas::painel::{ALTURA_CAB, Painel, RECUO_CAB};
    let mut h = harness(FaixaEArvore::default(), |ui, s| {
        let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(258.0, 200.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r));
        let mut no = None;
        Painel::new("explorador", "Explorador").mostrar(&mut filho, |ui| {
            no = Some(No::new("clientes").ativo(true).mostrar(ui).resposta.rect);
        });
        s.painel = Some(r);
        s.no = no;
    });
    h.run_steps(1);
    let (painel, no) = (h.state().painel.unwrap(), h.state().no.unwrap());
    // a linha selecionada começa e termina onde a faixa começa e termina
    assert_eq!(no.left(), painel.left() + RECUO_CAB);
    assert_eq!(no.right(), painel.right() - RECUO_CAB);
    assert_eq!(no.top(), painel.top() + RECUO_CAB + ALTURA_CAB);
}

#[derive(Default)]
struct Segmentados {
    trilhos: Vec<egui::Rect>,
    /// a largura de "Excel (.xlsx)" em 600
    excel: f32,
}

#[test]
fn seletor_segmentado_tem_partes_iguais_e_o_cheio_vai_de_ponta_a_ponta() {
    use interface::pecas::navegacao::trilho;
    let mut h = harness(Segmentados::default(), |ui, s| {
        let fonte = fontes::fonte(12.0, fontes::Peso::Medio);
        s.excel = interface::pecas::texto::galley(ui, "Excel (.xlsx)", fonte, egui::Color32::PLACEHOLDER).size().x;
        s.trilhos.clear();
        let normal = ["Excel (.xlsx)", "CSV", "JSON"];
        s.trilhos.push(trilho(ui, egui::Id::new("normal"), &normal, Some(1), TamanhoTrilho::Normal, true).1.rect);
        // o cheio numa linha de 460px (o corpo da janela Exportar)
        let linha = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(460.0, 40.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(linha));
        let cheio = ["Sistema", "Claro", "Escuro"];
        let r = trilho(&mut filho, egui::Id::new("cheio"), &cheio, Some(0), TamanhoTrilho::Cheio, true).1.rect;
        s.trilhos.push(r);
    });
    h.run_steps(1);
    let (trilho_normal, trilho_cheio) = (h.state().trilhos[0], h.state().trilhos[1]);
    let excel_600 = h.state().excel;
    let partes = |rotulos: [&str; 3]| rotulos.map(|r| h.get_by_label(r).rect());
    let perto = |a: f32, b: f32| (a - b).abs() < 0.05;
    // todas as partes com a largura da maior (a do Excel), encostadas, dentro da borda e do `padding:2px`
    let normal = partes(["Excel (.xlsx)", "CSV", "JSON"]);
    assert!(normal.iter().all(|r| perto(r.width(), normal[0].width())), "{normal:?}");
    assert!(perto(normal[1].left(), normal[0].right()) && perto(normal[2].left(), normal[1].right()), "{normal:?}");
    assert!(perto(trilho_normal.width(), 3.0 * normal[0].width() + 6.0), "{trilho_normal:?}");
    // a largura não muda com a escolha: a parte é medida no peso do escolhido (600), com o `padding:0 14px` e a
    // borda de 1px, mesmo com o Excel desmarcado
    assert!(perto(normal[0].width(), (excel_600 + 2.0 * 14.0 + 2.0).ceil()), "{normal:?} {excel_600}");
    // cheio: de ponta a ponta da linha, em três partes iguais, com a do meio no centro
    let cheio = partes(["Sistema", "Claro", "Escuro"]);
    assert_eq!(trilho_cheio.width(), 460.0, "de ponta a ponta");
    assert!(cheio.iter().all(|r| perto(r.width(), (460.0 - 6.0) / 3.0)), "{cheio:?}");
    assert!(perto(cheio[0].left(), trilho_cheio.left() + 3.0) && perto(cheio[2].right(), trilho_cheio.right() - 3.0));
    assert!(perto(cheio[1].center().x, trilho_cheio.center().x), "{cheio:?}");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aba {
    Dados,
    Estrutura,
}

#[test]
fn abas_e_seletor_segmentado() {
    let mut h = harness((Aba::Dados, Tema::Sistema), |ui, s| {
        if let Some(a) = abas(ui, "abas", &[(Aba::Dados, "Dados"), (Aba::Estrutura, "Estrutura")], s.0) {
            s.0 = a;
        }
        segmentado(
            ui,
            "tema",
            &[(Tema::Sistema, "Sistema"), (Tema::Claro, "Claro"), (Tema::Escuro, "Escuro")],
            &mut s.1,
            TamanhoTrilho::Compacto,
        );
    });
    h.get_by_label("Estrutura").click();
    h.get_by_label("Escuro").click();
    h.run_steps(2);
    assert_eq!(*h.state(), (Aba::Estrutura, Tema::Escuro));
}

#[test]
fn toast_some_sozinho_depois_de_2_6_s() {
    let mut h = harness((), |ui, _| {
        desenhar_toast(ui.ctx(), f32::INFINITY);
    });
    toast(&h.ctx, "Copiado.");
    h.run_steps(1);
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Copiado."));
    let status = h.get_by_role(egui::accesskit::Role::Status);
    assert_eq!(status.value().as_deref(), Some("Copiado."), "o toast aparece na árvore com o papel status");
    h.run_steps(12); // 3 s
    assert_eq!(toast_atual(&h.ctx), None);
}

#[test]
fn toast_fica_acima_do_limite_de_baixo() {
    use interface::pecas::toast::rect_toast;
    // o editor passa a borda de baixo do cartão (o alto da barra de status menos o espaço entre os painéis): o toast
    // fica 24px acima dela
    let mut h = harness((), |ui, _| desenhar_toast(ui.ctx(), 500.0));
    toast(&h.ctx, "Copiado.");
    h.run_steps(2);
    assert_eq!(rect_toast(&h.ctx).map(|r| r.bottom()), Some(500.0 - 24.0));
    // sem nada embaixo, 24px acima da borda da janela (e um limite abaixo dela não o desce mais)
    for (limite, esperado) in [(f32::INFINITY, 700.0 - 24.0), (690.0, 690.0 - 24.0), (900.0, 700.0 - 24.0)] {
        let mut h = harness((), move |ui, _| desenhar_toast(ui.ctx(), limite));
        toast(&h.ctx, "Copiado.");
        h.run_steps(2);
        assert_eq!(rect_toast(&h.ctx).map(|r| r.bottom()), Some(esperado), "{limite}");
    }
}

#[test]
fn zona_de_risco_nasce_fechada_e_abre_no_clique() {
    let mut h = harness(false, |ui, aberta| {
        ZonaRisco::new("z", "Excluir a tabela").mostrar(ui, |ui| {
            ui.add(Botao::perigo("Excluir tabela"));
        });
        *aberta = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new(("databox::zona_risco", "z")))).unwrap_or(false);
    });
    assert!(h.query_by_label("Excluir tabela").is_none(), "fechada por padrão");
    h.get_by_label("Excluir a tabela").click();
    h.run_steps(2);
    assert!(h.query_by_label("Excluir tabela").is_some());
    assert!(*h.state());
}

#[test]
fn ajuda_abre_o_balao_no_hover() {
    let mut h = harness((), |ui, _| {
        ajuda(ui, "Resumo do arquivo, as tabelas que ele tem e as ações de manutenção.");
    });
    h.get_by_label("Sobre esta tela").hover();
    h.run_steps(2);
    let tem_balao = h.ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Tooltip));
    assert!(tem_balao, "o balão abre no hover");
}

struct EstadoGrade {
    grade: GradeSomenteLeitura,
    ultima: RespostaGrade,
}

fn grade_de_teste(n: usize) -> EstadoGrade {
    let colunas = vec![ColunaGrade::new("id"), ColunaGrade::new("nome"), ColunaGrade::new("valor")];
    let linhas = (0..n)
        .map(|i| {
            vec![
                Celula::Inteiro(i as i64 + 1),
                Celula::Texto(format!("Nome {i}")),
                if i % 7 == 0 { Celula::Nulo } else { Celula::Real(i as f64 * 1.5) },
            ]
        })
        .collect();
    EstadoGrade { grade: GradeSomenteLeitura::new("g", colunas, linhas), ultima: RespostaGrade::default() }
}

#[test]
fn grade_seleciona_move_com_setas_e_copia() {
    let mut h = harness(grade_de_teste(50), |ui, s| {
        let r = s.grade.mostrar(ui, 400.0);
        if r.copiado.is_some() || r.selecao_mudou {
            s.ultima = r;
        }
        desenhar_toast(ui.ctx(), f32::INFINITY);
    });
    // clique na segunda linha, coluna "nome" (o harness põe 8px de margem em volta)
    let larguras = h.state().grade.larguras().map(<[f32]>::to_vec).unwrap_or_default();
    let x = 8.0 + larguras[0] + 10.0;
    let y = 8.0 + interface::grade::ALTURA_CABECALHO + interface::grade::ALTURA_LINHA * 1.5;
    clicar_em(&h, egui::pos2(x, y));
    h.run_steps(2);
    assert_eq!(h.state().grade.selecao(), Some((1, 1)));
    h.key_press(Key::ArrowDown);
    h.run_steps(1);
    h.key_press(Key::ArrowRight);
    h.run_steps(1);
    assert_eq!(h.state().grade.selecao(), Some((2, 2)));
    h.event(egui::Event::Copy);
    h.run_steps(1);
    // o real copiado como o `String(número)` do JavaScript (4.5 → "4.5", 3.0 → "3")
    assert_eq!(h.state().ultima.copiado.as_deref(), Some("3"));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("Copiado."));
}

#[test]
fn grade_seleciona_um_intervalo_arrastando_e_com_o_shift_e_copia_como_o_excel() {
    let mut h = harness(grade_de_teste(50), |ui, s| {
        let r = s.grade.mostrar(ui, 400.0);
        if r.copiado.is_some() || r.selecao_mudou {
            s.ultima = r;
        }
        desenhar_toast(ui.ctx(), f32::INFINITY);
    });
    let larguras = h.state().grade.larguras().map(<[f32]>::to_vec).unwrap_or_default();
    let centro = |l: usize, c: usize| {
        let x = 8.0 + larguras[..c].iter().sum::<f32>() + larguras[c] / 2.0;
        let y = 8.0 + interface::grade::ALTURA_CABECALHO + interface::grade::ALTURA_LINHA * (l as f32 + 0.5);
        egui::pos2(x, y)
    };
    // arrastar da linha 1, coluna "id", até a linha 3, coluna "nome": a célula onde começou fica a ativa
    let (de, ate) = (centro(1, 0), centro(3, 1));
    h.hover_at(de);
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: de,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_steps(1);
    for t in 1..=5 {
        h.hover_at(de + (ate - de) * (t as f32 / 5.0));
        h.run_steps(1);
    }
    h.event(egui::Event::PointerButton {
        pos: ate,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run_steps(2);
    let g = &h.state().grade;
    assert_eq!(g.selecao(), Some((1, 0)));
    let iv = g.nucleo().intervalo().expect("o intervalo");
    assert_eq!((iv.linha0, iv.linha1, iv.coluna0, iv.coluna1), (1, 3, 0, 1));
    // o Ctrl+C leva o intervalo com tabulações e `\r\n`, como o Excel
    h.event(egui::Event::Copy);
    h.run_steps(1);
    assert_eq!(h.state().ultima.copiado.as_deref(), Some("2\tNome 1\r\n3\tNome 2\r\n4\tNome 3\r\n"));
    assert_eq!(toast_atual(&h.ctx).as_deref(), Some("3 linhas × 2 colunas copiadas."));
    // Shift+→ estende até a coluna "valor"; a linha 7 é NULL (vai vazia)
    h.event(tecla(Key::ArrowRight, Modifiers::SHIFT));
    h.run_steps(1);
    let iv = h.state().grade.nucleo().intervalo().expect("o intervalo");
    assert_eq!((iv.coluna0, iv.coluna1), (0, 2));
    // Esc volta à célula ativa; uma seta sem o Shift também desfaz o intervalo
    h.key_press(Key::Escape);
    h.run_steps(1);
    assert!(!h.state().grade.nucleo().intervalo().expect("a ativa").varias());
    h.event(tecla(Key::ArrowDown, Modifiers::SHIFT));
    h.event(tecla(Key::ArrowDown, Modifiers::SHIFT));
    h.run_steps(1);
    let iv = h.state().grade.nucleo().intervalo().expect("o intervalo");
    assert_eq!((iv.linha0, iv.linha1), (1, 3));
    h.key_press(Key::ArrowDown);
    h.run_steps(1);
    assert_eq!(h.state().grade.selecao(), Some((2, 0)));
    assert!(!h.state().grade.nucleo().intervalo().expect("a ativa").varias());
    // Shift+clique estende da ativa até a clicada
    let (x, y) = (centro(6, 2).x, centro(6, 2).y);
    h.event(egui::Event::ModifiersChanged(Modifiers::SHIFT));
    h.hover_at(egui::pos2(x, y));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: egui::pos2(x, y),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    h.event(egui::Event::PointerButton {
        pos: egui::pos2(x, y),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    h.run_steps(2);
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    let iv = h.state().grade.nucleo().intervalo().expect("o intervalo");
    assert_eq!((iv.linha0, iv.linha1, iv.coluna0, iv.coluna1), (2, 6, 0, 2));
    assert_eq!(h.state().grade.selecao(), Some((2, 0)), "a ativa não muda");
}

fn tecla(key: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

#[test]
fn grade_move_com_varias_teclas_no_mesmo_quadro() {
    let mut h = harness(grade_de_teste(50), |ui, s| {
        let r = s.grade.mostrar(ui, 400.0);
        if r.selecao_mudou {
            s.ultima = r;
        }
    });
    let larguras = h.state().grade.larguras().map(<[f32]>::to_vec).unwrap_or_default();
    let x = 8.0 + larguras[0] + 10.0;
    let y = 8.0 + interface::grade::ALTURA_CABECALHO + interface::grade::ALTURA_LINHA * 0.5;
    clicar_em(&h, egui::pos2(x, y));
    h.run_steps(2);
    assert_eq!(h.state().grade.selecao(), Some((0, 1)));
    // uma seta e, no mesmo quadro, uma tecla que não navega (o C que acompanha o Ctrl+C, por exemplo); o
    // `h.event` daria um quadro para cada evento
    h.input_mut().events.extend([tecla(Key::ArrowDown, Modifiers::NONE), tecla(Key::C, Modifiers::CTRL)]);
    h.step();
    assert_eq!(h.state().grade.selecao(), Some((1, 1)), "a tecla que não navega não desfaz a seta");
    assert!(h.state().ultima.selecao_mudou);
}

/// O que o núcleo entregou aos ganchos num quadro.
#[derive(Default)]
struct Registro {
    celulas: Vec<interface::grade::CelulaVisivel>,
    ths: Vec<interface::grade::CabecalhoVisivel>,
}

impl interface::grade::DesenhoGrade for Registro {
    fn celula(&mut self, _ui: &mut Ui, c: &interface::grade::CelulaVisivel) {
        self.celulas.push(*c);
    }
    fn cabecalho(&mut self, _ui: &mut Ui, h: &interface::grade::CabecalhoVisivel) {
        self.ths.push(*h);
    }
}

struct EstadoMedidasGrade {
    nucleo: interface::grade::NucleoGrade,
    registro: Registro,
}

#[test]
fn nucleo_entrega_celulas_de_28px_com_o_padding_do_estudio() {
    use interface::grade::{ALTURA_CABECALHO, ALTURA_LINHA, LARGURA_COLUNA_MARCAR, NucleoGrade};
    let estado = EstadoMedidasGrade {
        nucleo: NucleoGrade::new("medidas", 2, vec![LARGURA_COLUNA_MARCAR, 100.0, 80.0]).com_coluna_marcar(0),
        registro: Registro::default(),
    };
    let mut h = harness(estado, |ui, s| {
        s.registro = Registro::default();
        s.nucleo.mostrar(ui, 300.0, &mut s.registro);
    });
    h.run_steps(2);
    let r = &h.state().registro;
    let th = |c: usize| r.ths.iter().find(|t| t.coluna == c).copied().expect("th");
    let td = |l: usize, c: usize| r.celulas.iter().find(|t| t.linha == l && t.coluna == c).copied().expect("td");
    // `.grade th{height:30px;padding:0 10px}` com o fio `--btb` embaixo e o `--b` à direita
    let t1 = th(1);
    assert_eq!(t1.th.height(), ALTURA_CABECALHO);
    assert_eq!((t1.conteudo.left() - t1.th.left(), t1.th.right() - t1.conteudo.right()), (10.0, 11.0));
    assert_eq!((t1.conteudo.top(), t1.conteudo.bottom()), (t1.th.top(), t1.th.bottom() - 1.0));
    // `.grade td{height:28px;padding:0 10px}`: a célula sem o fio de baixo, o conteúdo sem o padding e o fio da direita
    let c1 = td(0, 1);
    assert_eq!(c1.celula.top(), t1.th.bottom());
    assert_eq!((c1.celula.width(), c1.celula.height()), (100.0, ALTURA_LINHA - 1.0));
    assert_eq!((c1.conteudo.left() - c1.celula.left(), c1.celula.right() - c1.conteudo.right()), (10.0, 11.0));
    assert_eq!(td(1, 1).celula.top() - c1.celula.top(), ALTURA_LINHA);
    // `td.selc`/`th.selc`: 32px, sem padding (a caixa centra na célula toda) e sem o fio da direita
    let m = td(0, 0);
    assert_eq!(m.conteudo, m.celula);
    assert_eq!(m.celula.width(), 32.0);
    assert_eq!(th(0).conteudo.width(), 32.0);
}

/// Grade da aba Dados montada no núcleo: colunas 0 (marcar) e 1 (número da linha) fixas e fora da navegação.
struct EstadoNucleo {
    nucleo: interface::grade::NucleoGrade,
    editar: Option<(usize, usize)>,
    digitado: Option<String>,
    excluir: bool,
    espaco: bool,
}

struct DesenhoVazio;

impl interface::grade::DesenhoGrade for DesenhoVazio {
    fn celula(&mut self, _ui: &mut Ui, _c: &interface::grade::CelulaVisivel) {}
    fn cabecalho(&mut self, _ui: &mut Ui, _h: &interface::grade::CabecalhoVisivel) {}
}

fn nucleo_de_dados() -> EstadoNucleo {
    use interface::grade::{Navegacao, NucleoGrade};
    EstadoNucleo {
        nucleo: NucleoGrade::new("dados", 3, vec![40.0, 50.0, 120.0, 120.0])
            .com_navegacao(Navegacao::Dados)
            .com_navegaveis(2..4)
            .com_fixas(&[0, 1])
            .com_altura_cabecalho(interface::grade::ALTURA_CABECALHO_COM_TIPO_ABRE),
        editar: None,
        digitado: None,
        excluir: false,
        espaco: false,
    }
}

#[test]
fn nucleo_da_aba_dados_pede_edicao_e_anda_com_tab() {
    // passos de 20ms (o padrão do harness é 250ms por evento, e o duplo clique precisa de menos de 300ms)
    let mut h =
        Harness::builder().with_size(egui::vec2(1000.0, 700.0)).with_step_dt(0.02).with_max_steps(20).build_ui_state(
            |ui, s: &mut EstadoNucleo| {
                if !pronto(ui) {
                    return;
                }
                let r = s.nucleo.mostrar(ui, 400.0, &mut DesenhoVazio);
                if r.editar.is_some() {
                    s.editar = r.editar;
                }
                if r.texto_digitado.is_some() {
                    s.digitado = r.texto_digitado;
                }
                s.excluir |= r.excluir;
                s.espaco |= r.espaco;
                ui.add(egui::Button::new("depois da grade"));
            },
            nucleo_de_dados(),
        );
    h.run_steps(2);
    let y = 8.0 + interface::grade::ALTURA_CABECALHO_COM_TIPO_ABRE + interface::grade::ALTURA_LINHA * 0.5;
    // clique na coluna do número da linha: não vira célula ativa
    clicar_em(&h, egui::pos2(8.0 + 60.0, y));
    h.run_steps(2);
    assert_eq!(h.state().nucleo.selecao(), None);
    // tempo para o clique acima não virar um clique triplo com os dois de baixo
    h.run_steps(60);
    // duplo clique numa célula de dados pede a edição
    let pos = egui::pos2(8.0 + 40.0 + 50.0 + 30.0, y);
    clicar_em(&h, pos);
    h.step();
    clicar_em(&h, pos);
    h.run_steps(2);
    assert_eq!(h.state().nucleo.selecao(), Some((0, 2)));
    assert_eq!(h.state().editar, Some((0, 2)), "duplo clique edita");
    h.state_mut().editar = None;
    h.key_press(Key::F2);
    h.run_steps(1);
    assert_eq!(h.state().editar, Some((0, 2)), "F2 edita a célula ativa");
    // digitar começa a edição com a tecla
    h.event(egui::Event::Text("x".into()));
    h.run_steps(1);
    assert_eq!(h.state().digitado.as_deref(), Some("x"));
    h.key_press(Key::Delete);
    h.key_press(Key::Space);
    h.run_steps(1);
    assert!(h.state().excluir && h.state().espaco);
    // Tab de célula em célula; da última coluna passa para a linha seguinte
    h.key_press(Key::Tab);
    h.run_steps(1);
    assert_eq!(h.state().nucleo.selecao(), Some((0, 3)));
    h.key_press(Key::Tab);
    h.run_steps(1);
    assert_eq!(h.state().nucleo.selecao(), Some((1, 2)));
    // PageDown anda 10 linhas (para na última)
    h.key_press(Key::PageDown);
    h.run_steps(1);
    assert_eq!(h.state().nucleo.selecao(), Some((2, 2)));
    // na última célula, o Tab deixa o foco sair da grade
    h.key_press(Key::End);
    h.run_steps(1);
    assert_eq!(h.state().nucleo.selecao(), Some((2, 3)));
    h.key_press(Key::Tab);
    h.run_steps(2);
    let id_corpo = h.state().nucleo.id_corpo();
    assert!(!h.ctx.memory(|m| m.has_focus(id_corpo)), "o foco saiu da grade");
    assert_eq!(h.state().nucleo.selecao(), Some((2, 3)));
}

#[derive(Default)]
struct EstadoEditor {
    sql: String,
    outro: String,
}

fn editor_sql(ui: &mut Ui, s: &mut EstadoEditor) {
    interface::pecas::campos::AreaTexto::new(&mut s.sql)
        .id_salt("editor")
        .mono(true)
        .codigo(true)
        .altura(120.0)
        .mostrar(ui);
    ui.add(Entrada::new(&mut s.outro).rotulo("Outro campo"));
}

#[test]
fn editor_de_sql_recua_com_dois_espacos_e_libera_o_tab_depois_do_esc() {
    let mut h = harness(EstadoEditor { sql: "SELECT 1".into(), ..Default::default() }, editor_sql);
    let id = egui::Id::new("editor");
    h.ctx.memory_mut(|m| m.request_focus(id));
    h.run_steps(2);
    h.key_press(Key::Tab);
    h.run_steps(1);
    assert_eq!(h.state().sql, "SELECT 1  ", "Tab põe dois espaços no cursor, nunca \\t");
    assert!(h.ctx.memory(|m| m.has_focus(id)), "o Tab fica no editor");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    h.run_steps(1);
    assert_eq!(h.state().sql, "SELECT 1  ", "a linha não começa com recuo: Shift+Tab não muda nada");
    // Esc libera o próximo Tab para mover o foco (o Esc não tira o foco do editor)
    h.key_press(Key::Escape);
    h.run_steps(1);
    assert!(h.ctx.memory(|m| m.has_focus(id)), "o Esc não tira o foco do editor");
    h.key_press(Key::Tab);
    h.run_steps(2);
    assert_eq!(h.state().sql, "SELECT 1  ", "o Tab liberado não recua");
    assert!(!h.ctx.memory(|m| m.has_focus(id)), "o Tab liberado move o foco");
}

/// Todas as formas do último quadro, com as de dentro dos `Shape::Vec`.
fn formas<S>(h: &Harness<'_, S>) -> Vec<egui::Shape> {
    fn juntar(forma: &egui::Shape, todas: &mut Vec<egui::Shape>) {
        match forma {
            egui::Shape::Vec(v) => v.iter().for_each(|f| juntar(f, todas)),
            outra => todas.push(outra.clone()),
        }
    }
    let mut todas = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, &mut todas);
    }
    todas
}

struct EstadoCalha {
    sql: String,
    rect: egui::Rect,
}

#[test]
fn calha_de_46_px_com_os_numeros() {
    use interface::codigo::{self, AparenciaCodigo, CoresCodigo};
    use interface::pecas::campos::{AreaTexto, LARGURA_CALHA};
    let sql = "SELECT 1;\nSELECT 'dois';\n-- três".to_string();
    let mut h = harness(EstadoCalha { sql, rect: egui::Rect::NOTHING }, |ui, s| {
        let saida = AreaTexto::new(&mut s.sql)
            .id_salt("calha")
            .mono(true)
            .codigo(true)
            .numeros_de_linha(true)
            .altura(120.0)
            .mostrar(ui);
        s.rect = saida.response.response.rect;
    });
    h.run_steps(1);
    let campo = h.state().rect;
    let p = tema::paleta_ctx(&h.ctx);
    let todas = formas(&h);
    // a calha: de dentro da borda até 46px, no `--p`
    let calha: Vec<egui::Rect> = todas
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Rect(r) if r.fill == p.estudio.faixa && r.rect.left() == campo.left() + 1.0 => Some(r.rect),
            _ => None,
        })
        .collect();
    assert_eq!(calha.len(), 1, "{calha:?}");
    assert_eq!(calha[0].right(), campo.left() + LARGURA_CALHA);
    assert_eq!(LARGURA_CALHA, 46.0);
    // os números 1, 2 e 3 dentro da calha, alinhados à direita com 12px de folga, em `--t3`
    let textos: Vec<(String, egui::Pos2, f32, egui::Color32)> = todas
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.pos, t.galley.size().x, t.fallback_color)),
            _ => None,
        })
        .collect();
    for n in ["1", "2", "3"] {
        let (_, pos, largura, cor) = textos.iter().find(|(t, ..)| t == n).unwrap_or_else(|| panic!("sem o {n}"));
        assert!((pos.x + largura - (campo.left() + LARGURA_CALHA - 12.0)).abs() < 0.5, "{n}: {pos:?} + {largura}");
        assert_eq!(*cor, p.estudio.t3);
    }
    // o texto começa depois da calha, com as cores do código: SELECT em `kw`, o texto em `st`, o comentário em `t3`
    let (_, pos, ..) = textos.iter().find(|(t, ..)| t.starts_with("SELECT 1;")).expect("o texto do editor");
    assert!(pos.x >= campo.left() + LARGURA_CALHA, "{pos:?}");
    let galley = todas
        .iter()
        .find_map(|f| match f {
            egui::Shape::Text(t) if t.galley.text().starts_with("SELECT 1;") => Some(t.galley.clone()),
            _ => None,
        })
        .unwrap();
    let cor_de = |trecho: &str| {
        let b = galley.job.text.find(trecho).unwrap();
        galley.job.sections.iter().find(|s| s.byte_range.start.0 <= b && b < s.byte_range.end.0).unwrap().format.color
    };
    assert_eq!(cor_de("SELECT"), p.estudio.kw);
    assert_eq!(cor_de("'dois'"), p.estudio.st);
    assert_eq!(cor_de("-- três"), p.estudio.t3);
    assert_eq!(cor_de("1"), p.estudio.nu);

    // com o Monokai, o fundo do editor e as cores mudam na hora
    codigo::definir(&h.ctx, AparenciaCodigo { cores: CoresCodigo::Monokai, ..Default::default() });
    h.run_steps(2);
    let monokai = CoresCodigo::Monokai.resolver(p);
    let todas = formas(&h);
    assert!(
        todas
            .iter()
            .any(|f| matches!(f, egui::Shape::Rect(r) if r.fill == monokai.fundo && r.rect.left() == campo.left())),
        "o fundo do Monokai no campo"
    );
    let galley = todas
        .iter()
        .find_map(|f| match f {
            egui::Shape::Text(t) if t.galley.text().starts_with("SELECT 1;") => Some(t.galley.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(galley.job.sections[0].format.color, monokai.kw);
    codigo::definir(&h.ctx, AparenciaCodigo::default());
}

/// O editor em modo JSON (`.linguagem(Json)`): cada classe na cor da carta em vigor (a chave em `--chave`), o texto
/// que a pessoa digita muda as cores sem cache velho, e com `Texto` o editor não leva cor nenhuma.
#[test]
fn area_de_texto_com_linguagem_json() {
    use interface::codigo::{self, AparenciaCodigo, CoresCodigo, Linguagem};
    use interface::pecas::campos::AreaTexto;
    struct E {
        texto: String,
        linguagem: Linguagem,
    }
    let json = "{\n  \"nome\": \"Produção\",\n  \"n\": 30,\n  \"ok\": true\n}".to_string();
    let mut h = harness(E { texto: json.clone(), linguagem: Linguagem::Json }, |ui, s| {
        AreaTexto::new(&mut s.texto)
            .id_salt("json")
            .mono(true)
            .codigo(true)
            .linguagem(s.linguagem)
            .numeros_de_linha(true)
            .altura(140.0)
            .mostrar(ui);
    });
    h.run_steps(1);
    let galley_do_editor = |h: &Harness<'_, E>| {
        formas(h)
            .into_iter()
            .find_map(|f| match f {
                egui::Shape::Text(t) if t.galley.text().starts_with('{') => Some(t.galley.clone()),
                _ => None,
            })
            .expect("o texto do editor")
    };
    let cor_de = |galley: &egui::Galley, trecho: &str| {
        let b = galley.job.text.find(trecho).unwrap();
        galley.job.sections.iter().find(|s| s.byte_range.start.0 <= b && b < s.byte_range.end.0).unwrap().format.color
    };
    let p = tema::paleta_ctx(&h.ctx);
    let es = &p.estudio;
    let galley = galley_do_editor(&h);
    assert_eq!(cor_de(&galley, "\"nome\""), es.chave);
    assert_eq!(cor_de(&galley, "\"Produção\""), es.st);
    assert_eq!(cor_de(&galley, "30"), es.nu);
    assert_eq!(cor_de(&galley, "true"), es.kw);
    assert_eq!(cor_de(&galley, "{"), es.t3);
    assert_ne!(es.chave, es.st);

    // o texto muda: o realce acompanha (o JSON inválido também leva cor)
    h.state_mut().texto = "{\n  \"nome\" \"Produção\"\n  \"n\": 30,".into();
    h.run_steps(2);
    let galley = galley_do_editor(&h);
    assert_eq!(cor_de(&galley, "\"nome\""), es.st, "sem os dois pontos, é texto");
    assert_eq!(cor_de(&galley, "\"n\""), es.chave);

    // com o Monokai, a chave é a do Monokai
    h.state_mut().texto = json;
    codigo::definir(&h.ctx, AparenciaCodigo { cores: CoresCodigo::Monokai, ..Default::default() });
    h.run_steps(2);
    let monokai = CoresCodigo::Monokai.resolver(p);
    let galley = galley_do_editor(&h);
    assert_eq!(cor_de(&galley, "\"nome\""), monokai.chave);
    assert_eq!(cor_de(&galley, "true"), monokai.kw);
    codigo::definir(&h.ctx, AparenciaCodigo::default());

    // texto simples: uma cor só
    h.state_mut().linguagem = Linguagem::Texto;
    h.run_steps(2);
    let galley = galley_do_editor(&h);
    assert_eq!(galley.job.sections.len(), 1);
    assert_eq!(cor_de(&galley, "\"nome\""), es.t);
}

/// `erro_na_linha`: só o número da margem da linha do erro sai em `--perigo`; os outros ficam em `--t3`.
#[test]
fn numero_da_margem_do_erro_em_perigo() {
    use interface::codigo::Linguagem;
    use interface::pecas::campos::AreaTexto;
    struct E {
        texto: String,
        erro: Option<usize>,
    }
    let texto = "{\n  \"a\": 1\n  \"b\": 2\n}".to_string();
    let mut h = harness(E { texto, erro: Some(2) }, |ui, s| {
        let mut a = AreaTexto::new(&mut s.texto)
            .id_salt("erro")
            .mono(true)
            .codigo(true)
            .linguagem(Linguagem::Json)
            .numeros_de_linha(true)
            .altura(120.0);
        if let Some(l) = s.erro {
            a = a.erro_na_linha(l);
        }
        a.mostrar(ui);
    });
    h.run_steps(1);
    let p = tema::paleta_ctx(&h.ctx);
    let cor_do_numero = |h: &Harness<'_, E>, n: &str| {
        formas(h)
            .into_iter()
            .find_map(|f| match f {
                egui::Shape::Text(t) if t.galley.text() == n => Some(t.fallback_color),
                _ => None,
            })
            .unwrap_or_else(|| panic!("sem o número {n}"))
    };
    assert_eq!(cor_do_numero(&h, "3"), p.estudio.perigo, "a linha 3 (índice 2) é a do erro");
    for n in ["1", "2", "4"] {
        assert_eq!(cor_do_numero(&h, n), p.estudio.t3, "o {n} não é a linha do erro");
    }
    // sem erro, todos em `--t3`
    h.state_mut().erro = None;
    h.run_steps(2);
    for n in ["1", "2", "3", "4"] {
        assert_eq!(cor_do_numero(&h, n), p.estudio.t3, "{n} sem erro");
    }
}

struct EstadoPlana {
    sql: String,
    rect: egui::Rect,
    destacar: Option<std::ops::Range<usize>>,
}

/// O editor plano da Consulta (prancheta 3): sem borda nem raio, com a calha encostada na esquerda e o texto a 14px
/// dela; o trecho destacado no fundo da seleção e trazido para a vista.
#[test]
fn editor_plano_encosta_e_destaca_o_trecho() {
    use interface::pecas::campos::{AreaTexto, LARGURA_CALHA};
    let mut sql = "SELECT 1;\n".repeat(40);
    sql.push_str("SELECT 'agulha';");
    let mut h = harness(EstadoPlana { sql, rect: egui::Rect::NOTHING, destacar: None }, |ui, s| {
        let mut a = AreaTexto::new(&mut s.sql).id_salt("plana").codigo(true).plana(true).numeros_de_linha(true);
        a = a.altura(120.0);
        if let Some(t) = s.destacar.clone() {
            a = a.destacar(t, true);
        }
        let saida = a.mostrar(ui);
        s.rect = saida.response.response.rect;
    });
    h.run_steps(1);
    let campo = h.state().rect;
    let p = tema::paleta_ctx(&h.ctx);
    let todas = formas(&h);
    // sem o contorno do campo: nenhum traço de borda em volta dele
    let contornos =
        todas.iter().filter(|f| matches!(f, egui::Shape::Rect(r) if r.stroke.width > 0.0 && r.rect == campo)).count();
    assert_eq!(contornos, 0);
    // a calha encosta na esquerda, sem os cantos, e vai até 46px
    let calha = todas.iter().find_map(|f| match f {
        egui::Shape::Rect(r) if r.fill == p.estudio.faixa && r.rect.left() == campo.left() => Some(r.clone()),
        _ => None,
    });
    let calha = calha.expect("a calha");
    assert_eq!((calha.rect.right(), calha.corner_radius), (campo.left() + LARGURA_CALHA, egui::CornerRadius::ZERO));
    // o texto a 14px da calha
    let texto = todas.iter().find_map(|f| match f {
        egui::Shape::Text(t) if t.galley.text().starts_with("SELECT 1;") => Some(t.pos),
        _ => None,
    });
    assert!((texto.expect("o texto").x - (campo.left() + LARGURA_CALHA + 14.0)).abs() < 0.5);
    // o destaque da "agulha", lá embaixo: o fundo `--sel` e a rolagem até ele
    let inicio = 40 * 10 + 8;
    h.state_mut().destacar = Some(inicio..inicio + 6);
    // a rolagem do egui anda animada
    h.run_steps(20);
    let destaque = formas(&h).into_iter().find_map(|f| match f {
        egui::Shape::Rect(r) if r.fill == p.estudio.sel => Some(r.rect),
        _ => None,
    });
    let destaque = destaque.unwrap_or_else(|| {
        let rects: Vec<_> = formas(&h)
            .into_iter()
            .filter_map(|f| match f {
                egui::Shape::Rect(r) => Some((r.rect, r.fill)),
                _ => None,
            })
            .collect();
        panic!("o trecho destacado: {:?} {rects:?}", p.estudio.sel)
    });
    assert!(campo.contains_rect(destaque), "rolou até o trecho: {destaque:?} em {campo:?}");
    assert!(destaque.width() > 30.0, "{destaque:?}");
}

#[derive(Default)]
struct EstadoSeletor {
    ativo: usize,
}

/// O seletor de resultados da Consulta: partes iguais com a nota, o leitor de tela ouve `tab`s, e rola na
/// horizontal até a escolhida quando não cabe.
#[test]
fn seletor_de_resultados_tem_partes_iguais_e_rola_ate_a_escolhida() {
    use interface::pecas::navegacao::{ParteSeletor, seletor};
    let textos: Vec<String> = (1..=30).map(|n| format!("Comando {n}")).collect();
    let mut h = harness(EstadoSeletor::default(), move |ui, s| {
        let partes: Vec<ParteSeletor<'_>> = textos
            .iter()
            .enumerate()
            .map(|(i, t)| ParteSeletor::new(t).nota(if i == 29 { "erro" } else { "8 linhas" }).perigo(i == 29))
            .collect();
        let r = ui.allocate_ui(egui::vec2(600.0, 40.0), |ui| seletor(ui, "teste", "Resultados", &partes, s.ativo));
        if let Some(i) = r.inner {
            s.ativo = i;
        }
    });
    h.run_steps(2);
    let tabs: Vec<egui::Rect> = h.query_all(by().role(Role::Tab)).map(|n| n.rect()).collect();
    assert_eq!(tabs.len(), 30);
    assert!(tabs.windows(2).all(|w| (w[0].width() - w[1].width()).abs() < 0.01), "partes iguais");
    assert_eq!(h.get(by().role(Role::Tab).label("Comando 1 · 8 linhas")).accesskit_node().is_selected(), Some(true));
    assert!(h.query(by().role(Role::TabList).label("Resultados")).is_some());
    // escolher a última (fora da vista): a rolagem vai até ela
    h.state_mut().ativo = 29;
    // a rolagem do egui anda animada
    h.run_steps(20);
    let ultima = h.get(by().role(Role::Tab).label("Comando 30 · erro")).rect();
    assert!(ultima.right() <= 610.0 && ultima.left() >= 0.0, "{ultima:?}");
    h.get(by().role(Role::Tab).label("Comando 29 · 8 linhas")).click();
    h.run_steps(2);
    assert_eq!(h.state().ativo, 28);
}

#[derive(Default)]
struct EstadoNo {
    cliques: u32,
}

#[test]
fn no_com_icone_a_direita_responde_ao_clique() {
    use interface::pecas::arvore::No;
    let mut h = harness(EstadoNo::default(), |ui, s| {
        ui.set_max_width(260.0);
        let r = No::new("clientes")
            .rotulo("Inserir clientes no comando")
            .icone(Icone::Tabela)
            .icone_a_direita(Icone::Inserir)
            .mostrar(ui);
        if r.resposta.clicked() {
            s.cliques += 1;
        }
    });
    h.run_steps(1);
    let no = h.get(by().role(Role::Link).label("Inserir clientes no comando")).rect();
    // o ↵ de 13px à direita, a 10px da borda (o `padding-right` do nó)
    let silhueta = malhas_com_textura(&h)
        .into_iter()
        .map(|(_, r)| r)
        .find(|r| (13.0..=14.0).contains(&r.width()) && r.right() > no.center().x);
    let silhueta = silhueta.unwrap_or_else(|| panic!("a silhueta à direita: {:?} {no:?}", malhas_com_textura(&h)));
    // (o PNG de 14px, preso aos pixels, no meio da caixa de 13)
    assert!((silhueta.center().x - (no.right() - 10.0 - 6.5)).abs() < 1.0, "{silhueta:?} em {no:?}");
    h.get(by().role(Role::Link).label("Inserir clientes no comando")).click();
    h.run_steps(1);
    assert_eq!(h.state().cliques, 1);
}

#[derive(Default)]
struct EstadoEnvio {
    nome: String,
    obs: String,
    enviou: u32,
}

fn janela_com_envio(ui: &mut Ui, s: &mut EstadoEnvio) {
    let ctx = ui.ctx().clone();
    let (nome, obs) = (&mut s.nome, &mut s.obs);
    let r = Janela::new("envio", "Renomear tabela")
        .rodape_direita(|ui| {
            ui.add(Botao::primario("Renomear"));
        })
        .mostrar(&ctx, |ui| {
            ui.add(Entrada::new(nome).rotulo("Novo nome"));
            ui.add(interface::pecas::campos::AreaTexto::new(obs).id_salt("obs"));
        });
    if r.enviou {
        s.enviou += 1;
    }
}

#[test]
fn enter_no_campo_envia_a_janela_e_na_area_de_texto_nao() {
    let mut h = harness(EstadoEnvio::default(), janela_com_envio);
    h.run_steps(2);
    h.get_by_label("Novo nome").type_text("clientes_2024");
    h.run_steps(1);
    h.key_press(Key::Enter);
    h.run_steps(1);
    assert_eq!(h.state().enviou, 1, "Enter no campo de uma linha envia");
    assert_eq!(h.state().nome, "clientes_2024");
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("obs")));
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(1);
    assert_eq!(h.state().enviou, 1, "Enter na área de texto é quebra de linha");
    assert_eq!(h.state().obs, "\n");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    h.run_steps(1);
    assert_eq!(h.state().enviou, 1);
}

#[derive(Default)]
struct EstadoSugestoes {
    tipo: String,
    enviou: bool,
}

#[test]
fn entrada_com_sugestoes_escolhe_pelas_setas_sem_enviar_a_janela() {
    use interface::pecas::campos::EntradaSugestoes;
    const TIPOS: [&str; 4] = ["INTEGER", "TEXT", "REAL", "BLOB"];
    let mut h = harness(EstadoSugestoes::default(), |ui, s| {
        let ctx = ui.ctx().clone();
        let tipo = &mut s.tipo;
        let r = Janela::new("coluna", "Nova coluna").mostrar(&ctx, |ui| {
            EntradaSugestoes::new("tipo", tipo, &TIPOS).rotulo("Tipo").mostrar(ui);
        });
        s.enviou |= r.enviou;
    });
    h.run_steps(2);
    h.get_by_label("Tipo").type_text("ex");
    h.run_steps(2);
    assert!(h.query_by_label("TEXT").is_some(), "a lista mostra o que contém o texto");
    assert!(h.query_by_label("INTEGER").is_none());
    h.key_press(Key::ArrowDown);
    h.run_steps(1);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().tipo, "TEXT");
    assert!(!h.state().enviou, "o Enter que escolhe a sugestão não envia a janela");
    assert!(h.query_by_label("REAL").is_none(), "a lista fechou");
}

#[test]
fn toast_respeita_o_movimento_reduzido_do_tema() {
    // com animação ligada no estilo, só o indicador do tema (o do Windows) diz para não animar
    let mut h = Harness::builder().with_size(egui::vec2(800.0, 600.0)).with_step_dt(0.016).build_ui(|ui| {
        let ctx = ui.ctx().clone();
        tema::aplicar(&ctx, Tema::Claro);
        if fontes::instaladas(&ctx) {
            desenhar_toast(&ctx, f32::INFINITY);
        }
    });
    h.run_steps(2);
    assert!(h.ctx.global_style().animation_time > 0.0);
    let fundo_do_toast = |h: &Harness<'_>| {
        h.ctx.memory(|m| m.area_rect(egui::Id::new("databox::toast").with("area"))).map(|r| r.bottom())
    };
    // com animação: dois quadros (33ms) depois, o toast ainda está subindo
    toast(&h.ctx, "Salvo.");
    h.run_steps(2);
    let animando = fundo_do_toast(&h).unwrap_or(0.0);
    assert!(animando > h.ctx.content_rect().bottom() - 24.0 + 5.0, "{animando}");
    // com o movimento reduzido do tema: sem a subida de 14px, já no lugar
    tema::definir_movimento_reduzido(&h.ctx, true);
    toast(&h.ctx, "Salvo de novo.");
    h.run_steps(2);
    assert_eq!(fundo_do_toast(&h), Some(h.ctx.content_rect().bottom() - 24.0));
    assert_eq!(interface::pecas::toast::opacidade_toast(0.01, true), 1.0);
    assert!(interface::pecas::toast::opacidade_toast(0.01, false) < 0.1);
}

#[test]
fn toast_mostra_o_solido_do_estado_da_mensagem() {
    use interface::pecas::ajuda::IconeEstado;
    use interface::pecas::toast::{icone_toast, toast_atencao, toast_concluido, toast_erro};
    let ctx = egui::Context::default();
    assert_eq!(icone_toast(&ctx), None);
    // o canal de mensagens das telas leva o estado: um erro nunca sai com o ✓ de concluído
    type Mostrar = fn(&egui::Context, &str);
    let casos: [(Mostrar, &str, IconeEstado); 4] = [
        (|c, m| toast(c, m), "Nada mudou.", IconeEstado::Informacao),
        (|c, m| toast_concluido(c, m), "Linha salva.", IconeEstado::Concluido),
        (|c, m| toast_atencao(c, m), "O banco aberto mudou; nada foi feito.", IconeEstado::Atencao),
        (|c, m| toast_erro(c, m), "O valor não foi salvo.", IconeEstado::Erro),
    ];
    for (mostrar, mensagem, icone) in casos {
        mostrar(&ctx, mensagem);
        assert_eq!(toast_atual(&ctx).as_deref(), Some(mensagem));
        assert_eq!(icone_toast(&ctx), Some(icone), "{mensagem}");
    }
}

#[test]
fn avisos_de_concluido_e_de_informacao_tem_a_medida_do_aviso() {
    use interface::pecas::avisos::{TomAviso, aviso_tom};
    use interface::pecas::texto::Trecho;
    let mut h = harness(Vec::<f32>::new(), |ui, s| {
        s.clear();
        for tom in [TomAviso::Atencao, TomAviso::Concluido, TomAviso::Informacao] {
            s.push(aviso_tom(ui, &[Trecho::Normal("Arquivo salvo: clientes.xlsx")], tom).resposta.rect.height());
        }
    });
    h.run_steps(1);
    // `.aviso-j{padding:9px 12px}` + a borda de 1px e a linha de 17px (o sólido de 24px manda)
    assert_eq!(*h.state(), vec![24.0 + 2.0 * 10.0; 3]);
}

#[derive(Default)]
struct EstadoAvisos {
    link: Option<usize>,
    alturas: Vec<f32>,
}

#[test]
fn aviso_com_link_clicavel_e_espaco_de_14px_no_alto_do_corpo() {
    use interface::pecas::avisos::aviso;
    use interface::pecas::cartao::Cartao;
    use interface::pecas::texto::Trecho;
    let mut h = harness(EstadoAvisos::default(), |ui, s| {
        let mut rects = Vec::new();
        Cartao::new("transacao").mostrar(ui, |c| {
            c.corpo_avisos(|ui| {
                let r = aviso(
                    ui,
                    &[
                        Trecho::Forte("Há uma transação aberta na Consulta SQL."),
                        Trecho::Normal(" O que for salvo agora só vai para o arquivo com COMMIT. "),
                        Trecho::Link("Ir para a Consulta SQL"),
                    ],
                    false,
                );
                if r.link.is_some() {
                    s.link = r.link;
                }
                rects.push(r.resposta.rect);
                let r2 = aviso(ui, &[Trecho::Normal("Segundo aviso.")], true);
                rects.push(r2.resposta.rect);
                rects.push(ui.min_rect());
            });
        });
        s.alturas = vec![rects[1].top() - rects[0].bottom(), rects[2].bottom() - rects[1].bottom()];
    });
    h.run_steps(2);
    let alturas = h.state().alturas.clone();
    assert_eq!(alturas[0], 14.0, "do primeiro aviso ao segundo: 14px");
    assert_eq!(alturas[1], 0.0, "o segundo não ganha espaço embaixo");
    h.get_by_label("Ir para a Consulta SQL").click();
    h.run_steps(2);
    assert_eq!(h.state().link, Some(2), "o índice do trecho do link");
}

#[derive(Default)]
struct EstadoSelecao {
    copiado: Option<String>,
    aviso: Option<egui::Rect>,
}

#[test]
fn texto_do_aviso_pode_ser_selecionado_e_copiado() {
    use interface::pecas::avisos::aviso;
    use interface::pecas::texto::Trecho;
    let mut h = harness(EstadoSelecao::default(), |ui, s| {
        s.aviso = Some(aviso(ui, &[Trecho::Normal("no such table: clientes_x")], true).resposta.rect);
    });
    h.run_steps(2);
    // arrasta do começo ao fim do texto: o texto do `.erro`, depois da borda de 1px, do `padding:9px 12px` e do
    // sólido de 24px com `gap:12px` (49px), no meio da linha (a caixa tem a largura do texto e fica no meio, e o
    // texto fica centrado na altura do sólido)
    let r = h.state().aviso.expect("o aviso");
    let (inicio, fim) = (egui::pos2(r.left() + 49.0, r.center().y), egui::pos2(r.right() + 40.0, r.center().y));
    h.hover_at(inicio);
    h.event(egui::Event::PointerButton {
        pos: inicio,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.hover_at(fim);
    h.step();
    h.event(egui::Event::PointerButton {
        pos: fim,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.event(egui::Event::Copy);
    h.step();
    let copiado = h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    });
    h.state_mut().copiado = copiado;
    assert_eq!(h.state().copiado.as_deref(), Some("no such table: clientes_x"));
}

#[test]
fn modelo_abre_e_fecha_no_clique() {
    use interface::pecas::modelo::Modelo;
    let mut h = harness(false, |ui, aberto| {
        let r = Modelo::new("gatilho", "trg_pedidos_total").primeiro(true).mostrar(ui, |ui| {
            ui.add(Botao::new("Copiar"));
        });
        *aberto = r.aberto;
    });
    assert!(h.query_by_label("Copiar").is_none(), "fechado por padrão");
    h.get_by_label("trg_pedidos_total").click();
    h.run_steps(2);
    assert!(*h.state());
    assert!(h.query_by_label("Copiar").is_some());
    h.get_by_label("trg_pedidos_total").click();
    h.run_steps(2);
    assert!(!*h.state());
}

#[derive(Default)]
struct EstadoTabela {
    alturas: Vec<f32>,
    clicado: Option<usize>,
}

#[test]
fn tabela_simples_mede_as_linhas_e_aceita_conteudo_livre() {
    use interface::pecas::tabela::{ColunaTabela, Tabela};
    use interface::pecas::texto::{nome_abre, texto};
    let mut h = harness(EstadoTabela::default(), |ui, s| {
        let colunas =
            [ColunaTabela::new("Nome"), ColunaTabela::new("Linhas").direita(), ColunaTabela::new("").direita()];
        let mut alturas = Vec::new();
        let mut ultimo_y = None;
        let r = Tabela::new("tabelas", &colunas, 4).inativas(|l| l == 3).mostrar(ui, |ui, l, c| {
            let p = tema::paleta(ui);
            match c {
                0 => {
                    if nome_abre(ui, ["clientes", "pedidos", "notas", "sumiu"][l], l == 3).clicked() {
                        s.clicado = Some(l);
                    }
                    let y = ui.max_rect().top();
                    if ultimo_y != Some(y) {
                        alturas.push(y);
                        ultimo_y = Some(y);
                    }
                }
                1 => {
                    texto(ui, ["60", "5.000", "2", "—"][l], 12.0, p.fg);
                }
                _ => match l {
                    1 => {
                        ui.add(Botao::ghost("Estrutura"));
                    }
                    // conteúdo mais alto que a linha
                    2 => {
                        ui.allocate_exact_size(egui::vec2(10.0, 40.0), egui::Sense::hover());
                    }
                    _ => {}
                },
            }
        });
        alturas.push(r.rect.bottom());
        s.alturas = alturas;
    });
    h.run_steps(3);
    let y = h.state().alturas.clone();
    // `.lista-t td{height:32px;padding:0 12px}` com o fio de 1px embaixo: o conteúdo começa no topo da linha
    // (centrado nos 31px); o botão (`.btn`: 28px) cabe na linha; conteúdo de 40px a estica (40 + o fio); a última
    // linha não tem o fio (`tr:last-child td{border-bottom:0}`) e mede 32px
    assert_eq!(y[1] - y[0], 32.0, "linha só com texto: {y:?}");
    assert_eq!(y[2] - y[1], 32.0, "linha com botão: {y:?}");
    assert_eq!(y[3] - y[2], 41.0, "linha com conteúdo de 40px: {y:?}");
    assert_eq!(y[4] - y[3], 32.0, "última linha: {y:?}");
    h.get_by_label("pedidos").click();
    h.run_steps(1);
    assert_eq!(h.state().clicado, Some(1));
}

#[derive(Default)]
struct EstadoTabelaAlta {
    topos: Vec<f32>,
    fim: f32,
}

#[test]
fn tabela_com_linhas_mais_altas_e_cabecalho_de_30px() {
    use interface::pecas::tabela::{ALTURA_TH, ColunaTabela, Tabela};
    use interface::pecas::texto::texto;
    let mut h = harness(EstadoTabelaAlta::default(), |ui, s| {
        let colunas = [ColunaTabela::new("Nome"), ColunaTabela::new("Tamanho").direita()];
        let inicio = ui.cursor().top();
        let mut topos = Vec::new();
        let r = Tabela::new("recentes", &colunas, 2).altura_linha(34.0).mostrar(ui, |ui, l, c| {
            if c == 0 {
                topos.push(ui.max_rect().top() - inicio);
            }
            texto(ui, ["exemplo.db", "500 KB"][c], 12.0, tema::paleta(ui).fg);
            let _ = l;
        });
        s.topos = topos;
        s.fim = r.rect.bottom() - inicio;
    });
    h.run_steps(3);
    let s = h.state();
    // `.lista-t th{height:30px}`; as linhas da lista do Início com `td{height:34px}` (a última sem o fio)
    assert_eq!(ALTURA_TH, 30.0);
    assert_eq!(s.topos, vec![30.0, 64.0], "{:?}", s.topos);
    assert_eq!(s.fim, 98.0);
}

#[derive(Default)]
struct EstadoAjudaNoTexto {
    fim_do_texto: f32,
    icone: Option<egui::Rect>,
}

#[test]
fn ajuda_mini_fica_5px_depois_do_trecho_anterior() {
    use interface::pecas::avisos::aviso;
    use interface::pecas::texto::{CoresTexto, Trecho, paragrafo_rico_com, retangulos_da_faixa};
    let mut h = harness(EstadoAjudaNoTexto::default(), |ui, s| {
        let trechos = [
            Trecho::Forte("A tabela clientes_x não existe."),
            Trecho::Normal(" "),
            Trecho::Ajuda(&[Trecho::Forte("Mensagem original do SQLite: "), Trecho::Codigo("no such table")]),
            Trecho::Quebra,
            Trecho::Codigo("SELECT * FROM clientes_x"),
        ];
        // onde termina o espaço antes do "i" (no mesmo lugar do texto do `.erro`: 12px em linhas de 1,4, começando
        // 49px adentro da caixa — a borda, os 12px de padding, o sólido de 24px e o `gap:12px` — e com os 13px da
        // direita, numa caixa de até 560px)
        let p = tema::paleta(ui);
        let cores = CoresTexto::new(p, p.estudio.t2, p.estudio.perigo);
        let largura = ui.available_width().min(560.0) - 49.0 - 13.0;
        let g = paragrafo_rico_com(ui, &trechos, 12.0, &cores, largura, 1.4, &[]);
        let espaco = retangulos_da_faixa(&g.galley, &g.faixas[1]);
        let caixa = aviso(ui, &trechos, true).resposta.rect;
        s.fim_do_texto = caixa.left() + 49.0 + espaco.first().map_or(0.0, |(r, _)| r.right());
    });
    h.run_steps(2);
    let icone = h.get_by_label("Ajuda").rect();
    h.state_mut().icone = Some(icone);
    let s = h.state();
    let icone = s.icone.unwrap_or(egui::Rect::NOTHING);
    assert!((icone.left() - (s.fim_do_texto + 5.0)).abs() < 1.0, "{icone:?} depois de {}", s.fim_do_texto);
    assert_eq!(icone.width(), 14.0);
}

#[derive(Default)]
struct EstadoCompacta {
    obs: String,
    altura: f32,
    pisca: Vec<bool>,
    salvo_em: Option<f64>,
}

#[test]
fn campo_compacto_da_celula_e_o_salvo_que_pisca() {
    use interface::pecas::campos::{ALTURA_CAMPO_PEQUENO, DURACAO_SALVO, pintar_salvo};
    let mut h = Harness::builder().with_size(egui::vec2(600.0, 300.0)).with_step_dt(0.5).build_ui_state(
        |ui, s: &mut EstadoCompacta| {
            if !pronto(ui) {
                return;
            }
            let r = Entrada::new(&mut s.obs).compacta().largura(140.0).rotulo("Observação").mostrar(ui);
            s.altura = r.response.response.rect.height();
            // com animação (o `pronto` desliga): a célula pisca por 1,4s
            ui.ctx().all_styles_mut(|e| e.animation_time = 0.2);
            let p = tema::paleta(ui);
            let agora = ui.input(|i| i.time);
            let desde = *s.salvo_em.get_or_insert(agora);
            s.pisca.push(pintar_salvo(ui.painter(), p, r.response.response.rect, 7.0, desde));
        },
        EstadoCompacta::default(),
    );
    h.run_steps(6);
    assert_eq!(h.state().altura, ALTURA_CAMPO_PEQUENO);
    assert_eq!(ALTURA_CAMPO_PEQUENO, 26.0, "linha de 16px + padding 4px + borda");
    // passos de 0,5s a partir de t = 0: pisca nos primeiros quadros (t < 1,4s), depois não
    let pisca = h.state().pisca.clone();
    assert!(pisca.iter().any(|&p| p), "{pisca:?}");
    assert!(!pisca.last().copied().unwrap_or(true), "depois de {DURACAO_SALVO}s não pisca mais: {pisca:?}");
}

#[test]
fn titulo_de_grupo_da_ficha_tem_as_medidas_do_det() {
    use interface::pecas::detalhes::{ESPACO_COLUNAS, LARGURA_ROTULO, detalhes};
    let mut h = harness((), |ui, _| {
        detalhes(ui, |d| {
            d.linha_texto("Ctrl+O", Some("Abrir um banco"), "—");
            d.linha_texto("Ctrl+F", Some("Buscar"), "—");
            d.titulo_grupo("Na grade de dados");
            d.linha_texto("Setas", Some("Mover a célula ativa"), "—");
        });
    });
    h.run_steps(2);
    let primeira = h.get_by_label("Abrir um banco").rect();
    let antes = h.get_by_label("Buscar").rect();
    let titulo = h.get_by_label("Na grade de dados").rect();
    let depois = h.get_by_label("Mover a célula ativa").rect();
    // `.det{grid-template-columns:112px minmax(0,1fr);gap:7px 10px}`: o valor começa 122px depois do rótulo e as
    // linhas de 17px (12px × 1,4) ficam a 7px uma da outra
    assert_eq!((LARGURA_ROTULO, ESPACO_COLUNAS), (112.0, 10.0));
    assert_eq!(primeira.left() - titulo.left(), 112.0 + 10.0);
    assert_eq!((primeira.height(), antes.top() - primeira.bottom()), (17.0, 7.0));
    // o `.sec` depois do `.det`: os 14px de baixo do `.det` e o fio `--vidrob` em cima
    assert_eq!(titulo.top() - antes.bottom(), 14.0 + 1.0);
    assert_eq!(titulo.height(), 32.0, "`.sec{{height:32px}}`");
    // a linha seguinte começa com o `padding-top:2px` do `.det`
    assert_eq!(depois.top() - titulo.bottom(), 2.0);
}

#[test]
fn ficha_estreita_cede_o_rotulo_e_a_estreitissima_empilha() {
    use interface::pecas::detalhes::detalhes;
    use interface::pecas::rotulos::{TipoPill, pill};
    // as larguras da ficha das Propriedades: o painel de 272px (242), um pouco mais estreito (212) e o mínimo (170)
    for (largura, rotulo_largura, empilhada) in [(242.0, 112.0, false), (212.0, 92.0, false), (170.0, 0.0, true)] {
        let mut h = harness((), move |ui, _| {
            let r = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(largura, 400.0));
            let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r));
            detalhes(&mut filho, |d| {
                d.linha("Regras", |ui| {
                    pill(ui, "chave primária", TipoPill::Ok);
                });
                d.linha_texto("Tamanho", Some("500 KB"), "—");
            });
        });
        h.run_steps(2);
        let pilula = h.get_by_label("chave primária").rect();
        let valor = h.get_by_label("500 KB").rect();
        assert!(pilula.right() <= 100.0 + largura + 0.5, "a pill passa da ficha de {largura}: {pilula:?}");
        // (o rótulo é pintado direto, sem nó: a primeira linha começa em y = 50, com 17px de altura)
        if empilhada {
            // o valor vai embaixo do rótulo, na largura toda da ficha
            assert_eq!((pilula.left(), valor.left()), (100.0, 100.0), "{largura}");
            assert!(pilula.top() >= 50.0 + 17.0, "{pilula:?}");
            assert!(valor.top() >= pilula.bottom() + 7.0 + 17.0, "{pilula:?} {valor:?}");
        } else {
            assert_eq!(pilula.left() - 100.0, rotulo_largura + 10.0, "{largura}");
            assert!(pilula.top() < 50.0 + 17.0 && valor.top() >= pilula.bottom(), "lado a lado: {pilula:?}");
            assert_eq!(valor.left() - 100.0, rotulo_largura + 10.0);
        }
    }
}

#[derive(Default)]
struct MedidasDocumento {
    cabecalho: Vec<f32>,
    rodape: f32,
    aviso: f32,
}

#[test]
fn cartao_aviso_e_numeros_tem_as_medidas_das_pranchetas() {
    use interface::pecas::avisos::aviso_texto;
    use interface::pecas::cartao::Cartao;
    use interface::pecas::tiles::{Tile, tiles};
    let mut h = harness(MedidasDocumento::default(), |ui, s| {
        s.cabecalho.clear();
        for com_acoes in [false, true] {
            let mut corpo_topo = 0.0;
            let mut cartao = Cartao::new(("medidas", com_acoes)).titulo("Tabelas").hint("7 tabelas");
            if com_acoes {
                cartao = cartao.acoes(|ui| {
                    ui.add(Botao::new("Nova tabela"));
                });
            }
            let r = cartao.mostrar(ui, |c| {
                c.corpo_flush(|ui| corpo_topo = ui.max_rect().top());
                if com_acoes {
                    c.rodape(|ui| {
                        interface::pecas::texto::hint(ui, "Ordem alfabética");
                    });
                }
            });
            // a borda de 1px e a faixa `.sec` com o fio de baixo
            s.cabecalho.push(corpo_topo - r.response.rect.top() - 1.0);
            if com_acoes {
                s.rodape = r.response.rect.bottom() - 1.0 - corpo_topo;
            }
        }
        s.aviso = aviso_texto(ui, "Há uma transação aberta na Consulta SQL.", false).rect.height();
        tiles(ui, &[Tile::new("7", "Tabelas"), Tile::new("500", "Tamanho").sufixo("KB")]);
    });
    h.run_steps(3);
    let s = h.state();
    // `.sec{height:32px}` (42px com ações, a `.faixa`) + o fio `--vidrob`
    assert_eq!(s.cabecalho, [32.0 + 1.0, 42.0 + 1.0]);
    // `.rodape{height:34px}` (com o fio de cima), logo depois do corpo vazio
    assert_eq!(s.rodape, 34.0);
    // `.aviso-j`: a borda, `padding:9px` e o sólido de 24px (mais alto que a linha de 17px)
    assert_eq!(s.aviso, 1.0 + 9.0 + 24.0 + 9.0 + 1.0);
    let numero = h.get_by_label("7  Tabelas").rect();
    // a caixa dos números: `padding:12px 16px` + a borda, a linha de 26,4px, 4px e o rótulo de 14,7px
    assert!((numero.height() - (2.0 * 13.0 + 26.4 + 4.0 + 14.7)).abs() < 0.01, "{numero:?}");
}

#[test]
fn botao_do_menu_tem_a_largura_do_navegador() {
    use interface::pecas::casca::Menu;
    use interface::pecas::texto::galley;
    let mut h = harness((egui::Vec2::ZERO, 0.0_f32), |ui, s| {
        let texto = galley(ui, "Arquivo", fontes::fonte(12.0, fontes::Peso::Normal), egui::Color32::PLACEHOLDER);
        let (r, _) = Menu::new("arquivo", "Arquivo").mostrar(ui, |m| {
            m.item("Abrir pasta", Some("Ctrl+Shift+O"), false);
        });
        *s = (r.rect.size(), texto.size().x);
    });
    h.run_steps(1);
    let (tamanho, texto) = *h.state();
    // `.menu{height:24px;padding:0 10px;border:1px}`, texto 12px / 400 e sem a seta do `summary.btn` antigo
    assert_eq!(tamanho, egui::vec2((texto + 22.0).ceil(), 24.0));
    // aberto: o painel no vidro (`.flut`, 286px) com os itens `.mi` de 28px
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Arquivo").click();
    h.run_steps(2);
    assert_eq!(h.get_by_label("Abrir pasta").rect().height(), 28.0, "`.mi{{height:28px}}`");
    assert_eq!(h.get_by_label("Abrir pasta").rect().width(), 286.0 - 12.0, "286px menos a borda e o `padding:5px`");
}

#[derive(Default)]
struct MedidasNavegacao {
    seg: Vec<f32>,
    abas: f32,
    barra: f32,
    ajuda: Vec<f32>,
    tema: Tema,
}

#[test]
fn seg_abas_barra_e_ajuda_tem_as_medidas_das_pranchetas() {
    use interface::pecas::ajuda::{ajuda, ajuda_mini};
    use interface::pecas::navegacao::ALTURA_ABAS;
    use interface::pecas::progresso::barra_progresso;
    let mut h = harness(MedidasNavegacao::default(), |ui, s| {
        let antes = ui.cursor().min.y;
        segmentado(ui, "n", &[(Tema::Sistema, "Sistema"), (Tema::Claro, "Claro")], &mut s.tema, TamanhoTrilho::Normal);
        let meio = ui.cursor().min.y;
        segmentado(
            ui,
            "c",
            &[(Tema::Sistema, "Sistema"), (Tema::Claro, "Claro")],
            &mut s.tema,
            TamanhoTrilho::Compacto,
        );
        let depois = ui.cursor().min.y;
        // a altura de cada peça: quanto o cursor desceu, menos o espaço entre os itens
        let item = ui.spacing().item_spacing.y;
        s.seg = vec![meio - antes - item, depois - meio - item];
        let topo_abas = ui.cursor().min.y;
        abas(ui, "abas", &[(Aba::Dados, "Dados"), (Aba::Estrutura, "Estrutura")], Aba::Dados);
        s.abas = ui.cursor().min.y - topo_abas - item;
        s.barra = barra_progresso(ui, Some(0.5)).rect.height();
        s.ajuda = vec![ajuda(ui, "Explicação.").rect.width(), ajuda_mini(ui, "Explicação.").rect.width()];
    });
    h.run_steps(1);
    let s = h.state();
    // `.seg`: botões de 26px (24px no compacto) + `padding:2px` + a borda de 1px
    assert_eq!(s.seg, [26.0 + 6.0, 24.0 + 6.0]);
    // a faixa das abas: 35px, com as abas de 34px encostadas embaixo
    assert_eq!((s.abas, ALTURA_ABAS), (35.0, 35.0));
    assert_eq!(h.get_by_label("Dados").rect().height(), 34.0, "`.aba{{height:34px}}`");
    // `.barra{height:8px}`; o "i" de 16px (14px no mini)
    assert_eq!(s.barra, 8.0);
    assert_eq!(s.ajuda, [16.0, 14.0]);
}

#[test]
fn fontes_do_sistema_entram_na_primeira_aplicacao() {
    let mut h = harness((), |_, _| {});
    assert!(fontes::do_sistema_carregadas(&h.ctx), "o tema::aplicar instalou as fontes do sistema na hora");
    h.run_steps(2);
    assert!(fontes::instaladas(&h.ctx));
    let fontes_windows =
        std::path::Path::new(&std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    if fontes_windows.join("consola.ttf").exists() {
        let mono = h.ctx.fonts(|f| f.definitions().families.get(&egui::FontFamily::Monospace).cloned());
        assert_eq!(mono.and_then(|m| m.first().cloned()).as_deref(), Some("Interface mono"));
    }
    if fontes_windows.join("segoeui.ttf").exists() {
        let prop = h.ctx.fonts(|f| f.definitions().families.get(&egui::FontFamily::Proportional).cloned());
        assert_eq!(prop.and_then(|m| m.first().cloned()).as_deref(), Some("Interface 400"));
    }
}

#[test]
fn mostruario_parado_nao_repinta_nem_refaz_passadas() {
    let mut h = Harness::builder().with_size(egui::vec2(1440.0, 2700.0)).with_max_steps(20).build_ui_state(
        |ui, m: &mut interface::mostruario::Mostruario| {
            if pronto(ui) {
                tema::definir_movimento_reduzido(ui.ctx(), true);
                m.pagina(ui);
                m.sobreposicoes(ui);
            }
        },
        interface::mostruario::Mostruario::new(Tema::Claro),
    );
    h.run_steps(8);
    h.step();
    let saida = h.output();
    assert!(
        saida.platform_output.request_discard_reasons.is_empty(),
        "medidas estáveis: {:?}",
        saida.platform_output.request_discard_reasons
    );
    let atraso = saida.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay);
    assert_eq!(atraso, Some(std::time::Duration::MAX), "sem evento, a tela não pede repintura");
}

#[test]
fn grade_de_100_mil_linhas_desenha_so_o_que_aparece() {
    let mut h = harness(grade_de_teste(100_000), |ui, s| {
        s.grade.mostrar(ui, 600.0);
    });
    h.run_steps(1);
    let formas = h.output().shapes.len();
    assert!(formas < 2_000, "só as linhas visíveis viram formas: {formas}");
    let inicio = std::time::Instant::now();
    h.run_steps(30);
    let por_quadro = inicio.elapsed() / 30;
    eprintln!("100 mil linhas: {formas} formas, {por_quadro:?} por quadro");
    assert!(por_quadro < std::time::Duration::from_millis(100), "quadro lento demais: {por_quadro:?}");
}

// ------------------------------------------------------------------ peças da casca (etapa 2)

fn ligado<S>(h: &Harness<'_, S>, papel: Role, rotulo: &str) -> Option<Toggled> {
    h.get(by().role(papel).label(rotulo)).accesskit_node().toggled()
}

#[test]
fn todos_os_icones_pintam_em_14_16_18_e_24() {
    let mut h = harness((), |ui, _| {
        ui.horizontal_wrapped(|ui| {
            for lado in [14.0, 16.0, 18.0, 24.0] {
                for icone in Icone::TODOS {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(lado, lado), egui::Sense::hover());
                    icones::pintar(ui.painter(), r, icone, tema::paleta(ui).estudio.t2);
                }
            }
        });
        // um retângulo vazio não pinta nada (nem cai)
        icones::pintar(ui.painter(), egui::Rect::NOTHING, Icone::Banco, egui::Color32::WHITE);
    });
    h.run_steps(1);
    assert!(h.output().shapes.len() > 4 * Icone::TODOS.len(), "cada ícone pinta pelo menos uma forma");
}

#[derive(Default)]
struct BotoesCasca {
    rects: Vec<egui::Rect>,
    texto: f32,
    painel: bool,
}

#[test]
fn botoes_de_icone_medem_30_26_e_36_e_dizem_se_estao_ligados() {
    let mut h = harness(BotoesCasca::default(), |ui, s| {
        s.rects.clear();
        s.rects.push(ui.add(Botao::icone(Icone::Salvar).dica("Salvar (Ctrl+S)")).rect);
        s.rects.push(ui.add(Botao::icone(Icone::Mais).pequeno().rotulo("Nova consulta")).rect);
        s.rects.push(ui.add(Botao::icone(Icone::Explorador).rail().selecionado(true)).rect);
        s.rects.push(ui.add(Botao::new("Estrutura").pequeno()).rect);
        if ui.add(Botao::icone(Icone::Painel).selecionado(s.painel)).clicked() {
            s.painel = !s.painel;
        }
        ui.add(Botao::icone(Icone::Executar).principal());
        let fonte = fontes::fonte(12.0, fontes::Peso::Medio);
        s.texto = interface::pecas::texto::galley(ui, "Estrutura", fonte, egui::Color32::PLACEHOLDER).size().x;
    });
    h.run_steps(1);
    let s = h.state();
    // `.ib` 30px, `.ib.pq` 26px, `.rail` 36px; o `.btn.pq` tem 24px e `padding:0 10px`
    let tamanhos: Vec<egui::Vec2> = s.rects.iter().map(|r| r.size()).collect();
    assert_eq!(&tamanhos[..3], [egui::vec2(30.0, 30.0), egui::vec2(26.0, 26.0), egui::vec2(36.0, 36.0)]);
    assert_eq!(tamanhos[3], egui::vec2((s.texto + 22.0).ceil(), 24.0));
    // o rótulo padrão é o nome do ícone; o de alternar diz se está ligado
    assert!(h.query(by().role(Role::Button).label("Salvar")).is_some());
    assert!(h.query(by().role(Role::Button).label("Executar")).is_some());
    assert_eq!(ligado(&h, Role::Button, "Explorador"), Some(Toggled::True));
    assert_eq!(ligado(&h, Role::Button, "Salvar"), None, "botão comum não é de alternar");
    assert_eq!(ligado(&h, Role::Button, "Painel de propriedades"), Some(Toggled::False));
    h.get_by_label("Painel de propriedades").click();
    h.run_steps(2);
    assert_eq!(ligado(&h, Role::Button, "Painel de propriedades"), Some(Toggled::True));
}

#[derive(Default)]
struct EstadoAbas {
    largura: f32,
    ativa: usize,
    fechadas: Vec<usize>,
    novas: u32,
    altura: f32,
    juncao: Option<egui::Rangef>,
    movidas: Vec<(usize, usize)>,
}

fn abas_de_teste(ui: &mut Ui, s: &mut EstadoAbas) {
    use interface::pecas::abas_documento::{AbaDocumento, FaixaAbas};
    let abas = [
        AbaDocumento::new(Icone::Banco, "exemplo.db").sub("Visão geral"),
        AbaDocumento::new(Icone::Tabela, "clientes").sub("Dados"),
        AbaDocumento::new(Icone::Consulta, "Consulta 1"),
    ];
    let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(s.largura, 40.0));
    let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    let r = FaixaAbas::new("teste", &abas).ativa(Some(s.ativa)).nova(true).mostrar(&mut filho);
    if let Some(i) = r.clicada {
        s.ativa = i;
    }
    if let Some(i) = r.fechar {
        s.fechadas.push(i);
    }
    if r.nova {
        s.novas += 1;
    }
    s.altura = r.rect.height();
    s.juncao = r.juncao;
    s.movidas.extend(r.mover);
}

#[test]
fn faixa_de_abas_foca_fecha_e_abre_nova() {
    let mut h = harness(EstadoAbas { largura: 900.0, ..Default::default() }, abas_de_teste);
    h.run_steps(1);
    assert_eq!(h.state().altura, 35.0, "a faixa de 35px");
    let aba = h.get(by().role(Role::Tab).label("clientes · Dados"));
    assert_eq!(aba.rect().height(), 34.0, "`.aba{{height:34px}}`");
    assert_eq!(aba.accesskit_node().toggled(), Some(Toggled::False));
    aba.click();
    h.run_steps(2);
    assert_eq!(h.state().ativa, 1);
    assert_eq!(ligado(&h, Role::Tab, "clientes · Dados"), Some(Toggled::True));
    // a junção com o documento é o trecho da aba ativa
    let ativa = h.get(by().role(Role::Tab).label("clientes · Dados")).rect();
    assert_eq!(h.state().juncao, Some(egui::Rangef::new(ativa.left(), ativa.right())));
    // o × (rótulo único) e o clique do meio fecham; o "+" abre
    h.get_by_label("Fechar a aba Consulta 1").click();
    h.run_steps(2);
    h.get(by().role(Role::Tab).label("exemplo.db · Visão geral")).click_button(egui::PointerButton::Middle);
    h.run_steps(2);
    assert_eq!(h.state().fechadas, [2, 0]);
    assert_eq!(h.state().ativa, 1, "fechar não foca");
    // o "+" diz para o leitor de tela o mesmo que a dica
    h.get_by_label("Nova consulta SQL").click();
    h.run_steps(2);
    assert_eq!(h.state().novas, 1);
}

#[test]
fn faixa_de_abas_muda_a_aba_de_lugar_pelo_arrasto_e_pelo_menu() {
    let mut h = harness(EstadoAbas { largura: 900.0, ..Default::default() }, abas_de_teste);
    h.run_steps(1);
    let centro = |h: &Harness<'_, EstadoAbas>, rotulo: &str| h.get(by().role(Role::Tab).label(rotulo)).rect().center();
    let arrastar = |h: &mut Harness<'_, EstadoAbas>, de: egui::Pos2, para: egui::Pos2, esc: bool| {
        h.hover_at(de);
        h.run_steps(2);
        h.event(egui::Event::PointerButton {
            pos: de,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
        for t in 1..=6 {
            h.hover_at(de + (para - de) * (t as f32 / 6.0));
            h.run_steps(2);
        }
        if esc {
            h.key_press(egui::Key::Escape);
            h.run_steps(1);
        }
        h.event(egui::Event::PointerButton {
            pos: para,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
    };
    // a Consulta 1 arrastada até antes do meio da primeira cai na frente
    let de = centro(&h, "Consulta 1");
    let para = egui::pos2(centro(&h, "exemplo.db · Visão geral").x - 20.0, de.y);
    arrastar(&mut h, de, para, false);
    assert_eq!(h.state().movidas, [(2, 0)]);
    assert_eq!(h.state().ativa, 0, "arrastar não foca nem clica");
    // um arrasto curto, que não passa do meio da vizinha, não muda nada; o Esc cancela
    let de = centro(&h, "exemplo.db · Visão geral");
    arrastar(&mut h, de, de + egui::vec2(20.0, 0.0), false);
    let longe = egui::pos2(centro(&h, "Consulta 1").x + 30.0, de.y);
    arrastar(&mut h, de, longe, true);
    assert_eq!(h.state().movidas.len(), 1);
    // o menu da aba: a primeira não vai mais para a esquerda
    h.get(by().role(Role::Tab).label("exemplo.db · Visão geral")).click_button(egui::PointerButton::Secondary);
    h.run_steps(2);
    assert!(h.get_by_label("Mover à esquerda").accesskit_node().is_disabled());
    h.get_by_label("Mover à direita").click();
    h.run_steps(2);
    assert_eq!(h.state().movidas, [(2, 0), (0, 1)]);
}

#[test]
fn faixa_de_abas_encolhe_os_nomes_e_rola_ate_a_ativa() {
    // 500px: os textos encolhem e as três cabem inteiras
    let mut h = harness(EstadoAbas { largura: 500.0, ..Default::default() }, abas_de_teste);
    h.run_steps(1);
    let fim = |h: &Harness<'_, EstadoAbas>| h.get(by().role(Role::Tab).label("Consulta 1")).rect().right();
    // a primeira aba encosta na quina (`padding-left:0`, revisão de UI de 19/10/2026); à direita, os 10px e o "+"
    let x_faixa = h.get(by().role(Role::Tab).label("exemplo.db · Visão geral")).rect().left();
    assert!(fim(&h) <= x_faixa + 500.0 - 10.0 - 32.0 + 0.5, "{}", fim(&h));
    // 260px: nem encolhidas elas cabem; a faixa rola e a ativa (a última) fica à vista
    let mut h = harness(EstadoAbas { largura: 260.0, ativa: 2, ..Default::default() }, abas_de_teste);
    h.run_steps(2);
    let area = egui::Rangef::new(x_faixa, x_faixa + 260.0 - 10.0 - 32.0);
    let ultima = h.get(by().role(Role::Tab).label("Consulta 1")).rect();
    assert!(ultima.left() >= area.min - 0.5 && ultima.right() <= area.max + 0.5, "{ultima:?} {area:?}");
    let primeira = h.query(by().role(Role::Tab).label("exemplo.db · Visão geral"));
    assert!(primeira.is_none_or(|n| n.rect().left() >= area.min - 0.5), "a primeira some pela esquerda");
}

#[derive(Default)]
struct EstadoArvore {
    aberto: bool,
    cliques: u32,
}

#[test]
fn no_da_arvore_alterna_pela_seta_e_abre_pelo_nome() {
    use interface::pecas::arvore::{ALTURA_NO, No, Seta, recuo};
    let mut h = harness(EstadoArvore::default(), |ui, s| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let seta = if s.aberto { Seta::Aberta } else { Seta::Fechada };
        let r = No::new("clientes").recuo(recuo(2)).seta(seta).icone(Icone::Tabela).contagem("60").mostrar(ui);
        if r.alternou {
            s.aberto = !s.aberto;
        }
        if r.resposta.clicked() {
            s.cliques += 1;
        }
        if s.aberto {
            No::new("Dados").rotulo("clientes · Dados").recuo(recuo(3)).seta(Seta::Vazia).ativo(true).mostrar(ui);
        }
    });
    h.run_steps(1);
    let no = h.get(by().role(Role::Link).label("clientes")).rect();
    assert_eq!(no.height(), ALTURA_NO);
    // a seta: `margin:0 6px` + o recuo do nível; o clique nela só abre
    clicar_em(&h, egui::pos2(no.left() + recuo(2) + 5.0, no.center().y));
    h.run_steps(2);
    assert_eq!((h.state().aberto, h.state().cliques), (true, 0));
    assert_eq!(ligado(&h, Role::Link, "clientes · Dados"), Some(Toggled::True), "a parte ativa");
    // o clique no nome é do nó
    h.get(by().role(Role::Link).label("clientes")).click();
    h.run_steps(2);
    assert_eq!((h.state().aberto, h.state().cliques), (true, 1));
    // com o foco, ← fecha
    h.get(by().role(Role::Link).label("clientes")).focus();
    h.run_steps(1);
    h.key_press(Key::ArrowLeft);
    h.run_steps(2);
    assert!(!h.state().aberto);
}

#[derive(Default)]
struct EstadoPaineis {
    secao: Option<egui::Rect>,
    aberta: bool,
    status: Option<egui::Rect>,
}

#[test]
fn painel_secao_e_barra_de_status_tem_as_medidas_e_a_dica() {
    use interface::pecas::barra_status::{ALTURA_STATUS, ItemStatus, barra_status};
    use interface::pecas::painel::{ALTURA_SECAO, Painel, Secao};
    const FRASE: &str = "Você está editando exemplo.db. Tudo o que for salvo grava no arquivo na hora.";
    let mut h = harness(EstadoPaineis::default(), |ui, s| {
        ui.ctx().all_styles_mut(|st| st.interaction.tooltip_delay = 0.0);
        let topo = ui.cursor().min;
        let r = egui::Rect::from_min_size(topo, egui::vec2(272.0, 300.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r));
        Painel::new("p", "Propriedades").mostrar(&mut filho, |ui| {
            let (resp, corpo) = Secao::new("s", "Tabela").primeira(true).mostrar(ui, |_| ());
            s.secao = Some(resp.rect);
            s.aberta = corpo.is_some();
        });
        let rs = egui::Rect::from_min_size(egui::pos2(topo.x, r.bottom() + 10.0), egui::vec2(700.0, 40.0));
        let mut ui_status = ui.new_child(egui::UiBuilder::new().max_rect(rs));
        let e = tema::paleta(ui).estudio;
        let esquerda = [ItemStatus::new("exemplo.db").ponto(e.ok), ItemStatus::new("Gravação imediata").dica(FRASE)];
        s.status = Some(barra_status(&mut ui_status, &esquerda, &[ItemStatus::new("SQLite 3.53.2")]).rect);
    });
    h.run_steps(1);
    let (secao, status) = (h.state().secao.expect("a seção"), h.state().status.expect("a barra de status"));
    // a faixa `.cab` de 23px, a 7px do alto (a borda e a margem); a seção de 32px logo depois dela
    assert_eq!((secao.height(), ALTURA_SECAO), (32.0, 32.0));
    assert_eq!(secao.top() - h.get_by_label("Propriedades").rect().center().y, 23.0 / 2.0);
    assert!(h.state().aberta);
    assert_eq!(status.height(), ALTURA_STATUS);
    // a seção fecha no clique; o rótulo dela é próprio ("Seção Tabela"), sem repetir o de um menu
    assert!(h.query_by_label("Tabela").is_none());
    h.get(by().role(Role::Button).label("Seção Tabela")).click();
    h.run_steps(2);
    assert!(!h.state().aberta);
    // a frase do modo é a dica dele
    h.get_by_label("Gravação imediata").hover();
    h.run_steps(3);
    assert!(h.query_by_label(FRASE).is_some(), "a dica com a frase inteira");
}

/// A barra e onde ficaram os itens da esquerda e da direita.
type RectsStatus = Option<(egui::Rect, Vec<egui::Rect>, Vec<egui::Rect>)>;

#[test]
fn barra_de_status_estreita_corta_o_documento_e_nao_cobre_o_banco() {
    // antes, a direita era desenhada inteira: um nome de tabela comprido numa janela estreita cobria o nome do banco
    use interface::pecas::barra_status::{ItemStatus, barra_status};
    let longo = format!("{} · 1 linha · 1 coluna", "uma_tabela_com_um_nome_bem_comprido_".repeat(4));
    let mut h = harness(RectsStatus::None, move |ui, s| {
        let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(520.0, 40.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r));
        let e = tema::paleta(ui).estudio;
        let esquerda = [ItemStatus::new("exemplo.db").ponto(e.ok), ItemStatus::new("Gravação imediata")];
        let direita = [ItemStatus::new(&longo), ItemStatus::new("SQLite 3.53.2"), ItemStatus::new("UTF-8")];
        let resp = barra_status(&mut filho, &esquerda, &direita);
        let rects = |rs: &[egui::Response]| rs.iter().map(|r| r.rect).collect::<Vec<_>>();
        *s = Some((resp.rect, rects(&resp.esquerda), rects(&resp.direita)));
    });
    h.run_steps(1);
    let (barra, esquerda, direita) = h.state().clone().expect("a barra");
    // o nome do banco inteiro, e o item do documento cortado depois dele, com o recuo de 14px entre os dois
    assert!(direita[0].left() >= esquerda[0].right() + 14.0 - 0.5, "{esquerda:?} {direita:?}");
    assert!(direita[0].width() > 0.0 && direita[0].width() < 400.0, "cortado com …: {direita:?}");
    // os outros da direita ficam inteiros, e tudo dentro da barra
    assert!(direita[1].width() > 50.0 && direita[2].width() > 20.0, "{direita:?}");
    for r in esquerda.iter().take(1).chain(&direita) {
        assert!(r.left() >= barra.left() - 0.5 && r.right() <= barra.right() + 0.5, "{r:?} fora de {barra:?}");
    }
    // o leitor de tela continua ouvindo o texto inteiro
    assert!(
        h.query_by_label(&format!("{} · 1 linha · 1 coluna", "uma_tabela_com_um_nome_bem_comprido_".repeat(4)))
            .is_some()
    );
}

#[derive(Default)]
struct EstadoTitulo {
    clicados: Vec<&'static str>,
    /// o título e a faixa
    rects: Vec<egui::Rect>,
    botoes: Vec<egui::Rect>,
}

#[test]
fn barra_de_titulo_botoes_de_46_e_titulo_no_centro() {
    use interface::pecas::barra_titulo::{
        ALTURA_TITULO, BotaoJanela, LARGURA_MARCA, botao_janela, marca, titulo_centrado,
    };
    let mut h = harness(EstadoTitulo::default(), |ui, s| {
        let faixa = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(900.0, ALTURA_TITULO));
        s.rects = vec![faixa];
        let mut linha = ui
            .new_child(egui::UiBuilder::new().max_rect(faixa).layout(egui::Layout::left_to_right(egui::Align::Center)));
        linha.spacing_mut().item_spacing.x = 0.0;
        let logo = marca(&mut linha);
        assert_eq!(logo.rect.size(), egui::vec2(LARGURA_MARCA, ALTURA_TITULO));
        let mut direita = ui
            .new_child(egui::UiBuilder::new().max_rect(faixa).layout(egui::Layout::right_to_left(egui::Align::Center)));
        direita.spacing_mut().item_spacing.x = 0.0;
        s.botoes.clear();
        for b in [BotaoJanela::Fechar, BotaoJanela::Restaurar, BotaoJanela::Minimizar] {
            let r = botao_janela(&mut direita, b, false);
            s.botoes.push(r.rect);
            if r.clicked() {
                s.clicados.push(b.rotulo());
            }
        }
        let livre = egui::Rangef::new(logo.rect.right(), faixa.right() - 3.0 * 46.0);
        s.rects.push(titulo_centrado(ui, faixa, livre, "exemplo.db · DataBox", false));
    });
    h.run_steps(1);
    let s = h.state();
    assert!(s.botoes.iter().all(|r| r.size() == egui::vec2(46.0, 30.0)));
    assert!((s.rects[1].center().x - s.rects[0].center().x).abs() < 0.5, "no centro da faixa");
    assert!(h.query_by_label("exemplo.db · DataBox").is_some());
    assert!(h.query(by().role(Role::Button).label("Restaurar")).is_some());
    h.get(by().role(Role::Button).label("Fechar a janela")).click();
    h.run_steps(2);
    assert_eq!(h.state().clicados, ["Fechar a janela"]);
}

#[derive(Default)]
struct EstadoMenuCasca {
    explorador: bool,
    tema: usize,
    desligado: u32,
    recente: u32,
}

#[test]
fn menu_com_marcas_desligados_e_submenus() {
    use interface::pecas::casca::Menu;
    let mut h = harness(EstadoMenuCasca { explorador: true, ..Default::default() }, |ui, s| {
        Menu::new("exibir", "Exibir").largura(262.0).mostrar(ui, |m| {
            m.com_marcas();
            if m.item_marcado("Explorador", Some("Ctrl+B"), s.explorador).clicked() {
                s.explorador = !s.explorador;
            }
            if m.item_desligado("Salvar", Some("Ctrl+S")).clicked() {
                s.desligado += 1;
            }
            m.separador();
            m.submenu("Tema", 210.0, |m| {
                for (i, nome) in [comum::app::SEGUIR_O_SISTEMA, "Claro", "Escuro"].into_iter().enumerate() {
                    if m.item_opcao(nome, s.tema == i).clicked() {
                        s.tema = i;
                    }
                }
            });
            m.submenu("Abertos recentemente", 300.0, |m| {
                if m.item_duplo(Some(Icone::Banco), "vendas.db", "C:\\dados").clicked() {
                    s.recente += 1;
                }
            });
        });
    });
    let abrir = |h: &mut Harness<'_, EstadoMenuCasca>| {
        h.get(by().role(Role::Button).label("Exibir")).click();
        h.run_steps(2);
    };
    abrir(&mut h);
    assert_eq!(ligado(&h, Role::CheckBox, "Explorador"), Some(Toggled::True));
    assert_eq!(h.get_by_label("Explorador").rect().width(), 262.0 - 12.0, "a largura do Exibir");
    // o desligado não responde e não fecha o menu
    h.get_by_label("Salvar").click();
    h.run_steps(2);
    assert_eq!(h.state().desligado, 0);
    assert!(h.get_by_label("Salvar").accesskit_node().is_disabled());
    // o submenu abre com o ponteiro em cima, com o primeiro item na altura dele
    h.get_by_label("Tema").hover();
    h.run_steps(3);
    let tema = h.get_by_label("Tema").rect();
    let seguir = h.get(by().role(Role::RadioButton).label(comum::app::SEGUIR_O_SISTEMA)).rect();
    assert_eq!(seguir.top(), tema.top());
    assert_eq!(seguir.left() - tema.right(), 2.0 + 6.0, "2px de folga, a borda e o `padding:5px`");
    assert_eq!(ligado(&h, Role::RadioButton, comum::app::SEGUIR_O_SISTEMA), Some(Toggled::True));
    // ir para o outro submenu fecha este e abre aquele
    h.get_by_label("Abertos recentemente").hover();
    h.run_steps(4);
    assert!(h.query_by_label("Claro").is_none());
    assert_eq!(h.get_by_label("vendas.db").rect().height(), 40.0, "o item de duas linhas");
    // clicar num item do submenu fecha o menu inteiro
    h.get_by_label("vendas.db").click();
    h.run_steps(2);
    assert_eq!(h.state().recente, 1);
    assert!(h.query_by_label("Explorador").is_none(), "o menu fechou");
    // e escolher o tema também
    abrir(&mut h);
    h.get_by_label("Tema").hover();
    h.run_steps(3);
    h.get_by_label("Escuro").click();
    h.run_steps(2);
    assert_eq!(h.state().tema, 2);
    assert!(h.query_by_label("Explorador").is_none());
    // marcar fecha o menu e troca o estado
    abrir(&mut h);
    h.get_by_label("Explorador").click();
    h.run_steps(2);
    assert!(!h.state().explorador);
    assert!(h.query_by_label("Salvar").is_none());
}

#[test]
fn busca_da_barra_e_separador_tem_as_medidas_das_pranchetas() {
    use interface::pecas::ferramentas::{ALTURA_BARRA, LARGURA_BUSCA, barra_ferramentas, busca_desligada, separador};
    let mut h = harness(Vec::<egui::Rect>::new(), |ui, s| {
        s.clear();
        let mut busca = egui::Rect::NOTHING;
        let mut sep = egui::Rect::NOTHING;
        let barra = barra_ferramentas(
            ui,
            |ui| {
                ui.add(Botao::icone(Icone::Voltar));
                sep = separador(ui).rect;
                busca = busca_desligada(ui).rect;
            },
            |ui| {
                ui.add(Botao::icone(Icone::Painel));
            },
        );
        s.extend([barra, sep, busca]);
    });
    h.run_steps(3);
    let r = h.state().clone();
    assert_eq!(r[0].height(), ALTURA_BARRA);
    // `.sep`: 1×22px com `margin:0 6px`
    assert_eq!(r[1].size(), egui::vec2(13.0, 22.0));
    assert_eq!(r[2].size(), egui::vec2(LARGURA_BUSCA, 30.0));
    let busca = h.get(by().role(Role::TextInput).label("Buscar"));
    assert!(busca.accesskit_node().is_disabled(), "desligada até a etapa 3");
    // o Painel encosta à direita (`padding:0 8px` dentro da borda de 1px)
    let painel = h.get_by_label("Painel de propriedades").rect();
    assert_eq!(r[0].right() - painel.right(), 9.0);
    assert_eq!(ALTURA_BARRA, 46.0, "o `header.vidro` das pranchetas: a linha de 44px e a borda de 1px dos dois lados");
}

#[test]
fn casca_do_mostruario_parada_nao_repinta() {
    let mut m = interface::mostruario::Mostruario::new(Tema::Claro);
    m.mostrar_casca(true);
    let mut h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_max_steps(20).build_ui_state(
        |ui, m: &mut interface::mostruario::Mostruario| {
            if pronto(ui) {
                tema::definir_movimento_reduzido(ui.ctx(), true);
                let tela = ui.ctx().content_rect();
                let mut pagina = ui.new_child(egui::UiBuilder::new().max_rect(tela));
                m.pagina_casca(&mut pagina);
                m.sobreposicoes(&mut pagina);
            }
        },
        m,
    );
    h.run_steps(8);
    // o ponteiro passa pelo Fechar, por uma aba e por um nó da árvore e sai
    for (papel, rotulo) in
        [(Role::Button, "Fechar a janela"), (Role::Tab, "clientes · Estrutura"), (Role::Link, "eventos")]
    {
        h.get(by().role(papel).label(rotulo)).hover();
        h.run_steps(2);
    }
    h.hover_at(egui::pos2(700.0, 600.0));
    h.run_steps(30);
    h.step();
    let saida = h.output();
    assert!(
        saida.platform_output.request_discard_reasons.is_empty(),
        "medidas estáveis: {:?}",
        saida.platform_output.request_discard_reasons
    );
    let atraso = saida.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay);
    assert_eq!(atraso, Some(std::time::Duration::MAX), "sem evento, a casca não pede repintura");
}

// ------------------------------------------------------------------ ícones em PNG e as peças do passo 8 (etapa 3)

/// As malhas com textura pintadas no último quadro (os PNG dos ícones), com o retângulo de cada uma.
fn malhas_com_textura<S>(h: &Harness<'_, S>) -> Vec<(egui::TextureId, egui::Rect)> {
    fn juntar(forma: &egui::Shape, malhas: &mut Vec<(egui::TextureId, egui::Rect)>) {
        match forma {
            egui::Shape::Vec(formas) => formas.iter().for_each(|f| juntar(f, malhas)),
            egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => {
                malhas.push((m.texture_id, m.calc_bounds()));
            }
            _ => {}
        }
    }
    let mut malhas = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, &mut malhas);
    }
    malhas
}

#[test]
fn icones_vem_dos_png_presos_aos_pixels() {
    use egui::{pos2, vec2};
    for escala in [1.0_f32, 1.25, 1.5, 2.0] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(300.0, 200.0))
            .with_pixels_per_point(escala)
            .with_max_steps(20)
            .build_ui_state(
                |ui, _: &mut ()| {
                    if pronto(ui) {
                        // os cantos fora dos pixels: o ícone vai para o pixel mais perto
                        let r = |x: f32| egui::Rect::from_min_size(pos2(x + 0.3, 10.3), vec2(16.0, 16.0));
                        icones::pintar(ui.painter(), r(10.0), Icone::Salvar, egui::Color32::WHITE);
                        icones::pintar(ui.painter(), r(40.0), Icone::Dados, egui::Color32::WHITE);
                        icones::pintar(ui.painter(), r(70.0), Icone::Fechar, egui::Color32::WHITE);
                    }
                },
                (),
            );
        h.run_steps(2);
        let malhas = malhas_com_textura(&h);
        assert_eq!(malhas.len(), 2, "a {escala}: só as silhuetas e os sólidos são PNG; o × é traço");
        for icone in [Icone::Salvar, Icone::Dados] {
            let textura = icones::textura_carregada(&h.ctx, icone, 16.0).expect("textura carregada");
            let (_, r) = malhas.iter().find(|(t, _)| *t == textura).expect("pintado com a textura do lado certo");
            let lado_png = icones::lado_do_png(icone, 16.0, escala).expect("png") as f32;
            let px = r.width() * escala;
            if (lado_png - 16.0 * escala).abs() <= 1.0 {
                assert!((px - lado_png).abs() < 0.01, "{icone:?} a {escala}: {px} px, PNG de {lado_png}");
            } else {
                // o sólido sem o 1,25×: o de 24 reduzido a 20 px
                assert!((px - (16.0 * escala).round()).abs() < 0.01, "{icone:?} a {escala}: {px} px");
            }
            let x = r.min.x * escala;
            assert!((x - x.round()).abs() < 0.01, "{icone:?} a {escala}: canto em {x} px");
        }
    }
}

#[test]
fn pastilhas_e_folhas_levam_o_texto_por_cima() {
    let mut h = harness((), |ui, _| {
        ui.horizontal(|ui| {
            for icone in [
                Icone::Tipo(icones::TipoColuna::Inteiro),
                Icone::Folha(icones::TipoArquivo::Csv),
                Icone::Folha(icones::TipoArquivo::Txt),
            ] {
                for lado in [16.0, 24.0, 32.0] {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(lado, lado), egui::Sense::hover());
                    icones::pintar(ui.painter(), r, icone, egui::Color32::WHITE);
                }
            }
        });
    });
    h.run_steps(1);
    let mut textos = Vec::new();
    for c in &h.output().shapes {
        if let egui::Shape::Text(t) = &c.shape {
            textos.push(t.galley.text().to_owned());
        }
    }
    // o texto e a sombra: dois de cada; o glifo pelo lado em pontos ("12" até 24, como a prancheta 12; "123" acima)
    for texto in ["12", "123", "CSV", "TXT"] {
        assert!(textos.iter().filter(|t| *t == texto).count() >= 2, "{texto}: {textos:?}");
    }
}

struct BotoesPasso8 {
    com_icone: egui::Rect,
    lista: egui::Rect,
    texto: f32,
    ocupado: u32,
}

#[test]
fn botao_com_icone_lista_e_ocupado_com_o_arco() {
    let estado = BotoesPasso8 { com_icone: egui::Rect::NOTHING, lista: egui::Rect::NOTHING, texto: 0.0, ocupado: 0 };
    let mut h = harness(estado, |ui, s| {
        s.com_icone = ui.add(Botao::new("Exportar").com_icone(Icone::Exportar)).rect;
        s.lista = ui
            .allocate_ui(egui::vec2(260.0, 28.0), |ui| {
                ui.add(Botao::ghost("Verificar integridade").com_icone(Icone::VerificarIntegridade).lista())
            })
            .inner
            .rect;
        if ui.add(Botao::icone(Icone::Executar).principal().ocupado(true).dica("Executando")).clicked() {
            s.ocupado += 1;
        }
        let fonte = fontes::fonte(12.0, fontes::Peso::Medio);
        s.texto = interface::pecas::texto::galley(ui, "Exportar", fonte, egui::Color32::PLACEHOLDER).size().x;
    });
    h.run_steps(1);
    let s = h.state();
    // a silhueta de 14 e o `gap:6px` antes do texto, com o `padding:0 15px` e a borda
    assert_eq!(s.com_icone.size(), egui::vec2((s.texto + 14.0 + 6.0 + 32.0).ceil(), 28.0));
    // a lista ocupa a largura toda, com 28px
    assert_eq!(s.lista.size(), egui::vec2(260.0, 28.0));
    // o ocupado não clica, o leitor de tela ouve desligado, e o triângulo dá lugar ao arco
    let executar = h.get(by().role(Role::Button).label("Executar"));
    assert!(executar.accesskit_node().is_disabled());
    executar.click();
    h.run_steps(2);
    assert_eq!(h.state().ocupado, 0);
    assert!(icones::textura_carregada(&h.ctx, Icone::Executar, 16.0).is_none(), "o arco no lugar do ícone");
    assert!(icones::textura_carregada(&h.ctx, Icone::Exportar, 14.0).is_some());
    assert!(icones::textura_carregada(&h.ctx, Icone::VerificarIntegridade, 14.0).is_some());
}

struct CamposPasso8 {
    caminho: String,
    tipo: String,
    campo_tipo: egui::Rect,
}

#[test]
fn entrada_so_leitura_nao_muda_e_a_pastilha_vem_antes_do_texto() {
    let estado =
        CamposPasso8 { caminho: "C:\\dados\\exemplo.db".into(), tipo: String::new(), campo_tipo: egui::Rect::NOTHING };
    let mut h = harness(estado, |ui, s| {
        ui.add(Entrada::new(&mut s.caminho).somente_leitura(true).rotulo("Caminho"));
        let pastilha = icones::TipoColuna::do_declarado(&s.tipo, false, false);
        s.campo_tipo = ui.add(Entrada::new(&mut s.tipo).pastilha(pastilha).rotulo("Tipo")).rect;
    });
    let caminho = h.get_by_label("Caminho");
    caminho.click();
    h.run_steps(1);
    h.get_by_label("Caminho").type_text("xyz");
    h.run_steps(2);
    assert_eq!(h.state().caminho, "C:\\dados\\exemplo.db", "só leitura: digitar não muda");
    // a pastilha de 16 a 13px da borda, e o texto depois dela
    // sem tipo, o ∅
    let textura = icones::textura_carregada(&h.ctx, Icone::Tipo(icones::TipoColuna::Nulo), 16.0).expect("pastilha");
    let malhas = malhas_com_textura(&h);
    let (_, r) = malhas.iter().find(|(t, _)| *t == textura).expect("a pastilha pintada");
    let campo = h.state().campo_tipo;
    assert_eq!((r.left() - campo.left(), r.width()), (13.0, 16.0));
    // digitar no Tipo troca a pastilha no quadro seguinte
    h.get_by_label("Tipo").click();
    h.run_steps(1);
    h.get_by_label("Tipo").type_text("INTEGER");
    h.run_steps(2);
    assert_eq!(h.state().tipo, "INTEGER");
    assert!(icones::textura_carregada(&h.ctx, Icone::Tipo(icones::TipoColuna::Inteiro), 16.0).is_some());
}

struct CamposFicha {
    fixo: String,
    area: String,
    marcado: bool,
    nota: egui::Rect,
}

#[test]
fn ficha_campo_fixo_area_so_leitura_check_com_rotulo_e_nota() {
    use interface::pecas::avisos::{TomAviso, aviso_tom};
    use interface::pecas::campos::{AreaTexto, Check};
    use interface::pecas::texto::Trecho;
    let estado =
        CamposFicha { fixo: String::new(), area: "primeira\nsegunda".into(), marcado: true, nota: egui::Rect::NOTHING };
    let mut h = harness(estado, |ui, s| {
        ui.add(Entrada::new(&mut s.fixo).fixo(true).dica("NULL").dica_italica(true).rotulo("serie"));
        AreaTexto::new(&mut s.area).somente_leitura(true).id_salt("area").mostrar(ui);
        Check::new(&mut s.marcado, "NULL").rotulo("serie é NULL").mostrar(ui);
        s.nota =
            aviso_tom(ui, &[Trecho::Normal("exemplo.db está aberto só para leitura.")], TomAviso::Nota).resposta.rect;
    });
    h.run_steps(2);
    // o campo fixo (o NULL marcado) não edita e continua habilitado: sem o esmaecimento do desligado
    let campo = h.get_by_label("serie");
    assert!(!campo.accesskit_node().is_disabled());
    campo.click();
    h.run_steps(1);
    h.get_by_label("serie").type_text("x");
    h.run_steps(2);
    assert_eq!(h.state().fixo, "", "o campo fixo não muda");
    // a área só leitura seleciona e copia, mas não muda
    let c = h.get_by_role(Role::MultilineTextInput).rect().center();
    h.event(egui::Event::PointerMoved(egui::pos2(c.x, c.y)));
    h.get_by_role(Role::MultilineTextInput).click();
    h.run_steps(1);
    h.get_by_role(Role::MultilineTextInput).type_text("zzz");
    h.run_steps(2);
    assert_eq!(h.state().area, "primeira\nsegunda", "a área só leitura não muda");
    // a caixa diz a coluna ao leitor de tela e continua com o NULL na tela
    let caixa = h.get_by_role_and_label(Role::CheckBox, "serie é NULL");
    assert_eq!(caixa.accesskit_node().toggled(), Some(Toggled::True));
    assert!(h.query_by_role_and_label(Role::CheckBox, "NULL").is_none());
    // a nota tem a altura de um aviso (o sólido de 24 e os 10 de cima e de baixo)
    assert_eq!(h.state().nota.height(), 24.0 + 2.0 * 10.0);
}
#[test]
fn etiqueta_mais_tem_a_lista_na_dica() {
    let mut h = harness((), |ui, _| {
        interface::pecas::rotulos::etiqueta_mais(ui, 3, "UNIQUE · CHECK · COLLATE NOCASE");
    });
    h.run_steps(1);
    let no = h.get_by_label("+3: UNIQUE · CHECK · COLLATE NOCASE");
    assert_eq!(no.rect().height(), 16.0, "a pill de 16px");
}

#[test]
fn caminho_corta_do_comeco_e_guarda_o_nome_e_a_pasta_mae() {
    use interface::pecas::texto::{encurtar_caminho, encurtar_nome};
    let caminho = "C:\\Users\\ana\\dados\\arquivo\\2026\\vendas.db";
    let cabe = |n: usize| move |s: &str| s.chars().count() <= n;
    assert_eq!(encurtar_caminho(caminho, cabe(200)).as_deref(), Some(caminho), "cabe inteiro");
    // as pastas mais perto do nome, inteiras, quantas couberem (U6 da revisão de UI de 22/10/2026)
    assert_eq!(encurtar_caminho(caminho, cabe(24)).as_deref(), Some("…\\arquivo\\2026\\vendas.db"));
    assert_eq!(encurtar_caminho(caminho, cabe(16)).as_deref(), Some("…\\2026\\vendas.db"));
    assert_eq!(encurtar_caminho(caminho, cabe(11)).as_deref(), Some("…\\vendas.db"));
    assert_eq!(encurtar_caminho("/home/ana/dados/vendas.csv", cabe(18)).as_deref(), Some("…/dados/vendas.csv"));
    assert_eq!(encurtar_caminho(caminho, cabe(5)), None, "nem o nome cabe");
    assert_eq!(encurtar_caminho("vendas.db", cabe(3)), None, "sem pasta");
    // o nome que não cabe: cortado no meio, com a extensão inteira
    assert_eq!(encurtar_nome("vendas_2025.csv", cabe(200)).as_deref(), Some("vendas_2025.csv"));
    let nome = encurtar_nome("vendas_2025.csv", cabe(10)).expect("cabe cortado");
    assert!(nome.ends_with(".csv") && nome.contains('…') && nome.starts_with("ven"), "{nome}");
    assert_eq!(nome.chars().count(), 10, "o maior que cabe: {nome}");
    assert_eq!(encurtar_nome("vendas_2025.csv", cabe(4)), None, "nem ….csv cabe");
    assert_eq!(encurtar_nome("LEIAME", cabe(4)).as_deref(), Some("LE…E"), "sem extensão, no meio");
}

#[test]
fn janela_com_icone_e_sub_no_cabecalho() {
    let mut h = harness(None, |ui, rect: &mut Option<egui::Rect>| {
        let r = Janela::new("passo8", "Alterar a coluna cidade")
            .icone(Icone::Tabela)
            .sub("clientes")
            .largura(440.0)
            .mostrar(ui.ctx(), |ui| {
                ui.label("corpo");
            });
        *rect = Some(r.rect);
    });
    h.run_steps(2);
    let titulo = h.get(by().role(Role::Heading).value("Alterar a coluna cidade")).rect();
    let sub = h.get(by().value("clientes")).rect();
    assert!(sub.left() >= titulo.right() + 7.5, "o sub 8px depois do título: {titulo:?} {sub:?}");
    let textura = icones::textura_carregada(&h.ctx, Icone::Tabela, 20.0).expect("a ficha de 20");
    let malhas = malhas_com_textura(&h);
    let (_, r) = malhas.iter().find(|(t, _)| *t == textura).expect("a ficha pintada");
    assert_eq!(r.width(), 20.0);
    assert!((titulo.left() - r.right() - 10.0).abs() < 0.5, "10px até o título: {r:?} {titulo:?}");
    assert_eq!(h.state().map(|r| r.width()), Some(440.0));
}

#[test]
fn menu_com_icones_tem_linhas_de_36_e_os_solidos_de_24() {
    use interface::pecas::casca::Menu;
    let mut h = harness((), |ui, _| {
        ui.horizontal(|ui| {
            let _ = Menu::new("passo8", "Arquivo").largura(300.0).mostrar(ui, |p| {
                p.com_icones();
                p.icone(Icone::PastaSolida).item_desligado("Abrir pasta", Some("Ctrl+Shift+O"));
                p.icone(Icone::DocumentoSolido).item("Abrir arquivo", Some("Ctrl+O"), false);
                p.item("Fechar aba", Some("Ctrl+W"), false);
            });
        });
    });
    h.get(by().role(Role::Button).label("Arquivo")).click();
    h.run_steps(2);
    let abrir = h.get_by_label("Abrir arquivo").rect();
    let fechar = h.get_by_label("Fechar aba").rect();
    assert_eq!((abrir.height(), fechar.height()), (36.0, 36.0));
    let malhas = malhas_com_textura(&h);
    let documento = icones::textura_carregada(&h.ctx, Icone::DocumentoSolido, 24.0).expect("a folha de 24");
    let (_, r) = malhas.iter().find(|(t, _)| *t == documento).expect("pintada");
    assert_eq!((r.width(), r.left() - abrir.left()), (24.0, 8.0), "`.mi-ic` de 24 com `padding-left:8px`");
    // o desligado leva o ícone em cinza (outra textura) e mais fraco
    assert!(icones::textura_carregada(&h.ctx, Icone::PastaSolida, 24.0).is_none(), "a pasta só em cinza");
    assert_eq!(malhas.len(), 2);
}

// ---- o menu de contexto (passo 6 da etapa 4) --------------------------------------------------------------------

#[derive(Default)]
struct EstadoContexto {
    abrir: u32,
    copiar: u32,
    desligado: u32,
    fechar_pasta: u32,
}

/// Um dono (o botão "dono") com o menu da prancheta 15: Abrir, Mostrar na pasta, Copiar caminho, um item desligado
/// e, depois de um fio, o de perigo.
fn harness_do_menu_de_contexto() -> Harness<'static, EstadoContexto> {
    use interface::pecas::menu_contexto::MenuContexto;
    harness(EstadoContexto::default(), |ui, s| {
        let dono = ui.add(egui::Button::new("dono"));
        MenuContexto::new("teste").rotulo("vendas_2025.csv").mostrar(&dono, |m| {
            if m.item("Abrir", None, false).clicked() {
                s.abrir += 1;
            }
            m.silhueta(Icone::MostrarNaPasta).item("Mostrar na pasta", None, false);
            if m.silhueta(Icone::CopiarCaminho).item("Copiar caminho", Some("Ctrl+Shift+C"), false).clicked() {
                s.copiar += 1;
            }
            if m.silhueta(Icone::Excluir).item_desligado("Renomear", None).clicked() {
                s.desligado += 1;
            }
            m.separador();
            if m.silhueta(Icone::Excluir).item_perigo("Fechar pasta", None, false).clicked() {
                s.fechar_pasta += 1;
            }
        });
    })
}

#[test]
fn menu_de_contexto_abre_no_clique_direito_e_no_shift_f10() {
    let mut h = harness_do_menu_de_contexto();
    assert!(h.query_by_label("Abrir").is_none(), "nasce fechado");
    // o clique esquerdo no dono não abre
    h.get_by_label("dono").click();
    h.run_steps(3);
    assert!(h.query_by_label("Abrir").is_none());
    // o clique direito abre com o canto de cima à esquerda do vidro no ponteiro
    let dono = h.get_by_label("dono").rect();
    h.get_by_label("dono").click_secondary();
    h.run_steps(3);
    let abrir = h.get_by_label("Abrir").rect();
    assert!(abrir.left() - 6.0 >= dono.left() && abrir.left() - 6.0 <= dono.right(), "{abrir:?} {dono:?}");
    assert!(abrir.top() - 6.0 >= dono.top() && abrir.top() - 6.0 <= dono.bottom(), "{abrir:?} {dono:?}");
    // o menu tem o papel e o nome da prancheta (`role="menu" aria-label`)
    let _ = h.get(by().role(Role::Menu).label("vendas_2025.csv"));
    // clicar num item faz a ação e fecha
    h.get_by_label("Abrir").click();
    h.run_steps(3);
    assert_eq!(h.state().abrir, 1);
    assert!(h.query_by_label("Abrir").is_none(), "fechou");
    // o Shift+F10 com o foco no dono abre embaixo dele, com o foco no primeiro item
    h.get_by_label("dono").focus();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(4);
    let abrir = h.get_by_label("Abrir");
    assert!((abrir.rect().top() - 6.0 - dono.bottom()).abs() < 0.5, "embaixo do dono: {:?}", abrir.rect());
    assert!(abrir.is_focused(), "o foco vai ao primeiro item");
    // sem o foco no dono, o Shift+F10 não abre; sem o Shift, também não
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label("Abrir").is_none());
    h.key_press(Key::F10);
    h.run_steps(3);
    assert!(h.query_by_label("Abrir").is_none(), "o F10 sozinho não abre");
    // só com os modificadores exatos: o Alt+Shift+F10 e o Ctrl+Shift+F10 não abrem
    for outros in [Modifiers::ALT, Modifiers::CTRL] {
        h.get_by_label("dono").focus();
        h.run_steps(2);
        h.key_press_modifiers(Modifiers::SHIFT | outros, Key::F10);
        h.run_steps(3);
        assert!(h.query_by_label("Abrir").is_none(), "{outros:?}+Shift+F10 não abre");
    }
}

#[test]
fn menu_de_contexto_tem_250_de_largura_e_itens_de_28() {
    let mut h = harness_do_menu_de_contexto();
    h.get_by_label("dono").click_secondary();
    h.run_steps(3);
    let (abrir, copiar, fechar) = (
        h.get_by_label("Abrir").rect(),
        h.get_by_label("Copiar caminho").rect(),
        h.get_by_label("Fechar pasta").rect(),
    );
    // `.menu-ctx{width:250px;padding:5px}` com a borda de 1px: os itens ocupam 250 - 12
    assert_eq!((abrir.width(), copiar.width(), fechar.width()), (238.0, 238.0, 238.0));
    assert_eq!((abrir.height(), copiar.height(), fechar.height()), (28.0, 28.0, 28.0), "`.mi{{height:28px}}`");
    assert_eq!(copiar.top() - abrir.top(), 2.0 * 28.0, "itens colados");
    // o fio `.msep` (1px com `margin:5px 6px`) separa o item de perigo
    let desligado = h.get_by_label("Renomear").rect();
    assert_eq!(fechar.top() - desligado.bottom(), 5.0 + 1.0 + 5.0);
    // o texto de todos começa depois da coluna da silhueta: `padding-left:12px`, 14px de silhueta e `gap:10px`; o
    // "Abrir", sem silhueta, guarda o espaço dela
    let mut textos = std::collections::HashMap::new();
    for c in &h.output().shapes {
        if let egui::Shape::Text(t) = &c.shape {
            textos.insert(t.galley.text().to_owned(), t.pos);
        }
    }
    for (texto, item) in [("Abrir", abrir), ("Copiar caminho", copiar), ("Fechar pasta", fechar)] {
        let pos = textos.get(texto).unwrap_or_else(|| panic!("{texto}: {:?}", textos.keys()));
        assert_eq!(pos.x - item.left(), 12.0 + 14.0 + 10.0, "{texto}");
    }
    // o atalho `.at` na ponta direita, com `padding-right:10px`
    let atalho = textos.get("Ctrl+Shift+C").expect("o atalho");
    assert!(
        copiar.right() - atalho.x < 10.0 + 80.0 && copiar.right() - atalho.x > 10.0 + 40.0,
        "{atalho:?} {copiar:?}"
    );
    // a silhueta de 14 antes do texto, em `--t2`; a do item de perigo, em `--perigo`
    let malhas = |ctx: &egui::Context, h: &Harness<'_, EstadoContexto>, icone: Icone| {
        let textura = icones::textura_carregada(ctx, icone, 14.0).expect("a silhueta de 14");
        let mut achadas = Vec::new();
        fn juntar(f: &egui::Shape, t: egui::TextureId, achadas: &mut Vec<(egui::Rect, egui::Color32)>) {
            match f {
                egui::Shape::Vec(v) => v.iter().for_each(|f| juntar(f, t, achadas)),
                egui::Shape::Mesh(m) if m.texture_id == t => achadas.push((m.calc_bounds(), m.vertices[0].color)),
                _ => {}
            }
        }
        for c in &h.output().shapes {
            juntar(&c.shape, textura, &mut achadas);
        }
        achadas
    };
    let p = tema::paleta_ctx(&h.ctx);
    let copiar_sil = malhas(&h.ctx, &h, Icone::CopiarCaminho);
    assert_eq!(copiar_sil.len(), 1);
    assert_eq!(copiar_sil[0].0.size(), egui::vec2(14.0, 14.0));
    assert_eq!(copiar_sil[0].0.left() - copiar.left(), 12.0);
    assert_eq!(copiar_sil[0].1, p.estudio.t2);
    // o Excluir pinta duas: a do desligado (`--t2` a 60%) e a do perigo
    let excluir = malhas(&h.ctx, &h, Icone::Excluir);
    assert_eq!(excluir.len(), 2);
    assert!(excluir.iter().any(|(r, c)| r.top() >= fechar.top() && *c == p.estudio.perigo), "{excluir:?}");
    assert!(excluir.iter().any(|(r, c)| r.top() < fechar.top() && *c != p.estudio.perigo), "{excluir:?}");
}

#[test]
fn setas_andam_pelos_itens_e_esc_fecha_e_devolve_o_foco() {
    let mut h = harness_do_menu_de_contexto();
    h.get_by_label("dono").focus();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(4);
    // os itens ligados: Abrir, Mostrar na pasta, Copiar caminho, Fechar pasta (o "Renomear" desligado não conta)
    assert!(h.get_by_label("Abrir").is_focused());
    h.key_press(Key::ArrowDown);
    h.run_steps(3);
    assert!(h.get_by_label("Mostrar na pasta").is_focused());
    h.key_press(Key::ArrowDown);
    h.run_steps(3);
    assert!(h.get_by_label("Copiar caminho").is_focused());
    h.key_press(Key::ArrowDown);
    h.run_steps(3);
    assert!(h.get_by_label("Fechar pasta").is_focused(), "pula o desligado");
    // com volta: depois do último, o primeiro; antes do primeiro, o último
    h.key_press(Key::ArrowDown);
    h.run_steps(3);
    assert!(h.get_by_label("Abrir").is_focused());
    h.key_press(Key::ArrowUp);
    h.run_steps(3);
    assert!(h.get_by_label("Fechar pasta").is_focused());
    // o Enter ativa o item com o foco, fecha o menu e devolve o foco ao dono
    h.key_press(Key::Enter);
    h.run_steps(4);
    assert_eq!(h.state().fechar_pasta, 1);
    assert!(h.query_by_label("Abrir").is_none());
    assert!(h.get_by_label("dono").is_focused(), "o foco volta ao dono");
    // o Esc fecha e devolve o foco, sem ativar nada
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_steps(4);
    h.key_press(Key::ArrowDown);
    h.run_steps(3);
    assert!(h.get_by_label("Mostrar na pasta").is_focused());
    h.key_press(Key::Escape);
    h.run_steps(4);
    assert!(h.query_by_label("Abrir").is_none(), "o Esc fechou");
    assert!(h.get_by_label("dono").is_focused(), "o foco volta ao dono");
    assert_eq!((h.state().abrir, h.state().copiar, h.state().fechar_pasta), (0, 0, 1));
    // aberto pelo clique direito (nenhum item com o foco): ↓ vai ao primeiro e ↑ ao último
    h.get_by_label("dono").click_secondary();
    h.run_steps(3);
    h.key_press(Key::ArrowUp);
    h.run_steps(3);
    assert!(h.get_by_label("Fechar pasta").is_focused(), "↑ sem foco vai ao último");
    h.key_press(Key::Escape);
    h.run_steps(3);
    h.get_by_label("dono").click_secondary();
    h.run_steps(3);
    h.key_press(Key::ArrowDown);
    h.run_steps(3);
    assert!(h.get_by_label("Abrir").is_focused(), "↓ sem foco vai ao primeiro");
}

#[test]
fn item_desligado_nao_clica() {
    let mut h = harness_do_menu_de_contexto();
    h.get_by_label("dono").click_secondary();
    h.run_steps(3);
    assert!(h.get_by_label("Renomear").accesskit_node().is_disabled());
    h.get_by_label("Renomear").click();
    h.run_steps(3);
    assert_eq!(h.state().desligado, 0);
    assert!(h.query_by_label("Abrir").is_some(), "o desligado não fecha o menu");
    // um clique fora fecha (num canto vazio: o menu abre onde o dono foi clicado e, com outras fontes, cobre o ponto)
    clicar_em(&h, egui::pos2(990.0, 690.0));
    h.run_steps(3);
    assert!(h.query_by_label("Abrir").is_none());
}

// ---- o cartão de ação (passo 7 da etapa 4) ----------------------------------------------------------------------

/// As formas do último quadro, com os `Shape::Vec` abertos.
fn formas_pintadas<S>(h: &Harness<'_, S>) -> Vec<egui::Shape> {
    fn juntar(f: &egui::Shape, formas: &mut Vec<egui::Shape>) {
        match f {
            egui::Shape::Vec(v) => v.iter().for_each(|f| juntar(f, formas)),
            outra => formas.push(outra.clone()),
        }
    }
    let mut formas = Vec::new();
    for c in &h.output().shapes {
        juntar(&c.shape, &mut formas);
    }
    formas
}

struct EstadoAcoes {
    cliques: [u32; 4],
    botoes: [egui::Rect; 2],
}

fn harness_das_acoes() -> Harness<'static, EstadoAcoes> {
    use interface::pecas::acao::Acao;
    harness(EstadoAcoes { cliques: [0; 4], botoes: [egui::Rect::NOTHING; 2] }, |ui, s| {
        ui.horizontal(|ui| {
            if ui.add(Acao::new("Abrir pasta", Icone::Pasta).atalho("Ctrl+Shift+O").primaria(true)).clicked() {
                s.cliques[0] += 1;
            }
            if ui.add(Acao::new("Abrir arquivo", Icone::Arquivo).atalho("Ctrl+O")).clicked() {
                s.cliques[1] += 1;
            }
            if ui.add(Acao::new("Novo banco", Icone::NovoBanco)).clicked() {
                s.cliques[2] += 1;
            }
            if ui.add(Acao::new("Desligado", Icone::Arquivo).habilitado(false)).clicked() {
                s.cliques[3] += 1;
            }
        });
        ui.horizontal(|ui| {
            s.botoes = [ui.add(Botao::primario("Principal")).rect, ui.add(Botao::new("Neutro")).rect];
        });
    })
}

#[test]
fn acao_tem_220_de_largura_e_o_primario_usa_o_degrade() {
    use interface::pecas::acao::{ALTURA_ACAO, LARGURA_ACAO};
    let h = harness_das_acoes();
    let rects: Vec<egui::Rect> =
        ["Abrir pasta", "Abrir arquivo", "Novo banco", "Desligado"].map(|n| h.get_by_label(n).rect()).into();
    // `.acao{width:220px}`; todos com a altura do cartão com atalho (a borda, `padding:14px` e 39px de conteúdo)
    assert_eq!(LARGURA_ACAO, 220.0);
    assert_eq!(ALTURA_ACAO, 1.0 + 14.0 + 17.0 + 4.0 + 18.0 + 14.0 + 1.0);
    for r in &rects {
        assert_eq!(r.size(), egui::vec2(220.0, ALTURA_ACAO), "{r:?}");
    }
    // o primário usa o preenchimento do `Botao::primario` (o degradê do destaque) e os outros, o do `Botao::new`
    let formas = formas_pintadas(&h);
    let textura = |rect: egui::Rect| {
        formas
            .iter()
            .find_map(|f| match f {
                egui::Shape::Rect(r) if r.rect == rect && r.fill_texture_id() != egui::TextureId::default() => {
                    Some(r.fill_texture_id())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("sem preenchimento em {rect:?}"))
    };
    let s = h.state();
    assert_eq!(textura(rects[0]), textura(s.botoes[0]), "o primário tem o degradê do botão principal");
    assert_eq!(textura(rects[1]), textura(s.botoes[1]), "o neutro tem o volume neutro");
    assert_eq!(textura(rects[2]), textura(rects[1]));
    assert_ne!(textura(rects[0]), textura(rects[1]));
    // o nome em 13 / 600: branco no primário, `--t` nos outros
    let p = tema::paleta_ctx(&h.ctx);
    let cor_do_texto = |texto: &str| {
        formas
            .iter()
            .find_map(|f| match f {
                egui::Shape::Text(t) if t.galley.text() == texto => Some(t.fallback_color),
                _ => None,
            })
            .unwrap_or_else(|| panic!("sem o texto {texto}"))
    };
    assert_eq!(cor_do_texto("Abrir pasta"), egui::Color32::WHITE);
    assert_eq!(cor_do_texto("Abrir arquivo"), p.estudio.t);
    // o círculo do ícone: `--tint` (o primário, branco a 18%)
    let circulos: Vec<(egui::Pos2, egui::Color32)> = formas
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Circle(c) if c.radius == 18.0 => Some((c.center, c.fill)),
            _ => None,
        })
        .collect();
    assert_eq!(circulos.len(), 4);
    assert_eq!(circulos[0].1, interface::tema::rgba(255, 255, 255, 0.18));
    assert_eq!(circulos[1].1, p.estudio.tint);
    // 14 de `padding` e 1 de borda até o círculo de 36, centrado na altura
    assert_eq!(circulos[0].0, egui::pos2(rects[0].left() + 15.0 + 18.0, rects[0].center().y));
}

#[test]
fn acao_com_atalho_mostra_o_kbd() {
    let h = harness_das_acoes();
    let formas = formas_pintadas(&h);
    let p = tema::paleta_ctx(&h.ctx);
    let texto = |t: &str| {
        formas.iter().find_map(|f| match f {
            egui::Shape::Text(s) if s.galley.text() == t => Some((s.pos, s.galley.size(), s.fallback_color)),
            _ => None,
        })
    };
    // o atalho em 10,5 `--t3` (branco no primário); o cartão sem atalho não tem etiqueta
    let (_, _, cor) = texto("Ctrl+O").expect("o atalho do Abrir arquivo");
    assert_eq!(cor, p.estudio.t3);
    assert_eq!(texto("Ctrl+Shift+O").expect("o atalho do primário").2, egui::Color32::WHITE);
    // a etiqueta: 18 de altura, borda de 1px de raio 4 (`--b2`; no primário, branco a 40%), `padding:0 4px`
    let caixas: Vec<(egui::Rect, egui::epaint::RectShape)> = formas
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Rect(r) if r.rect.height() == 18.0 && r.stroke.width == 1.0 => Some((r.rect, r.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(caixas.len(), 2, "só os dois com atalho têm etiqueta: {caixas:?}");
    assert_eq!(caixas[0].1.stroke.color, interface::tema::rgba(255, 255, 255, 0.4));
    assert_eq!(caixas[1].1.stroke.color, p.estudio.b2);
    assert_eq!(caixas[1].1.corner_radius, egui::CornerRadius::same(4));
    let (pos, tamanho, _) = texto("Ctrl+O").expect("atalho");
    assert_eq!(caixas[1].0.width(), (tamanho.x + 8.0 + 2.0).ceil());
    assert!((pos.x - caixas[1].0.left() - (caixas[1].0.width() - tamanho.x) / 2.0).abs() < 0.5);
    // o texto do cartão começa depois do círculo de 36 e do `gap:12px`, e a etiqueta fica 4px abaixo do nome
    let cartao = h.get_by_label("Abrir arquivo").rect();
    assert_eq!(caixas[1].0.left() - cartao.left(), 15.0 + 36.0 + 12.0);
    let (nome, nome_tam, _) = texto("Abrir arquivo").expect("o nome");
    assert_eq!(nome.x, caixas[1].0.left());
    assert!(caixas[1].0.top() - (nome.y + nome_tam.y) >= 4.0 - 2.0 && caixas[1].0.top() - nome.y < 17.0 + 4.0 + 1.0);
}

#[test]
fn acao_clica_com_enter() {
    let mut h = harness_das_acoes();
    h.get_by_label("Abrir arquivo").focus();
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().cliques, [0, 1, 0, 0], "o Enter no cartão com o foco");
    h.get_by_label("Novo banco").click();
    h.run_steps(2);
    h.get_by_label("Abrir pasta").click();
    h.run_steps(2);
    assert_eq!(h.state().cliques, [1, 1, 1, 0]);
    // o desligado não clica e o leitor de tela o ouve desligado
    assert!(h.get_by_label("Desligado").accesskit_node().is_disabled());
    h.get_by_label("Desligado").click();
    h.run_steps(2);
    assert_eq!(h.state().cliques[3], 0);
}

// ---- a dock da Visão geral (passo 9) ----------------------------------------------------------------------------

struct EstadoDock {
    largura: f32,
    itens: Vec<interface::pecas::dock::ItemDock>,
    colunas: usize,
    celulas: Vec<egui::Rect>,
}

/// Os seis contadores da prancheta 1, com o rótulo das linhas pedido.
fn itens_da_dock(linhas: &str) -> Vec<interface::pecas::dock::ItemDock> {
    use interface::pecas::dock::ItemDock;
    vec![
        ItemDock::new(Icone::TabelaGrande, "7", "Tabelas"),
        ItemDock::new(Icone::VisaoGrande, "1", "Visão"),
        ItemDock::new(Icone::IndiceGrande, "5", "Índices"),
        ItemDock::new(Icone::GatilhoGrande, "1", "Gatilho"),
        ItemDock::new(Icone::DiscoGrande, "500", "Tamanho").unidade("KB"),
        ItemDock::new(Icone::LinhasGrande, "5.772", linhas).dica("Tabelas grandes não são contadas ao abrir."),
    ]
}

/// A dock numa coluna de `largura` pontos.
fn dock_em(largura: f32, itens: Vec<interface::pecas::dock::ItemDock>) -> Harness<'static, EstadoDock> {
    let estado = EstadoDock { largura, itens, colunas: 0, celulas: Vec::new() };
    harness(estado, |ui, s| {
        // as dicas logo no hover
        ui.ctx().all_styles_mut(|st| st.interaction.tooltip_delay = 0.0);
        let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(s.largura, 400.0));
        let mut coluna =
            ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)));
        let resposta = interface::pecas::dock::dock(&mut coluna, &s.itens);
        s.colunas = resposta.colunas;
        s.celulas = resposta.celulas.iter().map(|c| c.rect).collect();
    })
}

#[test]
fn dock_quebra_em_tres_quando_nao_cabe() {
    use interface::pecas::dock::{ALTURA_CELULA, ESPACO_PRATELEIRAS};
    // o documento a 1440 (776pt por dentro): seis colunas iguais numa prateleira
    let h = dock_em(776.0, itens_da_dock("Linhas"));
    assert_eq!(h.state().colunas, 6);
    let c = &h.state().celulas;
    assert!(c.iter().all(|r| r.top() == c[0].top() && (r.width() - 776.0 / 6.0).abs() < 0.01), "{c:?}");
    assert_eq!(c[0].height(), ALTURA_CELULA);
    let _ = h.get_by_label("7 Tabelas");
    let _ = h.get_by_label("500 KB Tamanho");
    // a 960 (578pt): ainda cabem os textos desta dock; com "Linhas (contadas)", não cabe mais e vira 3 + 3
    assert_eq!(dock_em(578.0, itens_da_dock("Linhas")).state().colunas, 6);
    let h = dock_em(578.0, itens_da_dock("Linhas (contadas)"));
    assert_eq!(h.state().colunas, 3);
    let c = &h.state().celulas;
    assert_eq!(c[3].top() - c[0].top(), ALTURA_CELULA + ESPACO_PRATELEIRAS, "a segunda prateleira 14pt abaixo");
    assert_eq!((c[3].left(), c[5].right()), (c[0].left(), c[2].right()));
    // e numa dock estreita
    assert_eq!(dock_em(320.0, itens_da_dock("Linhas")).state().colunas, 3);
}

#[test]
fn dock_poe_os_icones_no_mesmo_pe() {
    use interface::pecas::dock::{LADO_ICONE, LINHA_DO_PE, pe_do_icone};
    let mut h = dock_em(776.0, itens_da_dock("Linhas"));
    let icones_da_dock = [
        Icone::TabelaGrande,
        Icone::VisaoGrande,
        Icone::IndiceGrande,
        Icone::GatilhoGrande,
        Icone::DiscoGrande,
        Icone::LinhasGrande,
    ];
    let conferir = |h: &Harness<'_, EstadoDock>| {
        let malhas = malhas_com_textura(h);
        for (i, icone) in icones_da_dock.into_iter().enumerate() {
            let celula = h.state().celulas[i];
            let textura = icones::textura_carregada(&h.ctx, icone, LADO_ICONE).expect("o PNG de 48");
            let desta: Vec<egui::Rect> = malhas.iter().filter(|(t, _)| *t == textura).map(|(_, r)| *r).collect();
            let icone_r = desta.iter().find(|r| r.height() == LADO_ICONE).expect("o ícone de 48");
            let reflexo = desta.iter().find(|r| r.height() < 10.0).expect("o reflexo");
            // o topo descido pelo pé de cada desenho: todos em pé na linha de 44,5pt; o reflexo começa nela
            assert_eq!(icone_r.top() - celula.top(), pe_do_icone(icone), "{icone:?}");
            assert_eq!(reflexo.top() - celula.top(), LINHA_DO_PE, "{icone:?}");
            assert_eq!(reflexo.height(), 8.0);
            assert!((icone_r.center().x - celula.center().x).abs() < 0.51, "centrado: {icone_r:?} {celula:?}");
        }
    };
    conferir(&h);
    // no hover nada se move (sem a animação, pedido de 17/10/2026), e a dica aparece só onde há
    h.get_by_label("7 Tabelas").hover();
    h.run_steps(2);
    conferir(&h);
    h.get_by_label("5.772 Linhas").hover();
    h.run_steps(3);
    assert!(h.query_by_label("Tabelas grandes não são contadas ao abrir.").is_some(), "a dica das linhas");
}

#[derive(Default)]
struct EstadoTabelaClicavel {
    clicada: Option<usize>,
    botao: u32,
}

/// A tabela clicável (a seleção da Estrutura e dos Índices): o clique na linha, fora dos controles, vira
/// `linha_clicada`; o botão da linha fica com o clique dele.
#[test]
fn tabela_clicavel_diz_a_linha_clicada_e_o_botao_fica_com_o_dele() {
    use interface::pecas::tabela::{ColunaTabela, Tabela};
    use interface::pecas::texto::texto;
    let mut h = harness(EstadoTabelaClicavel::default(), |ui, s| {
        let colunas = [ColunaTabela::new("Nome"), ColunaTabela::new("").direita().fixa(120.0)];
        let r = Tabela::new("clicavel", &colunas, 3).clicavel().marcadas(|l| l == 1).mostrar(ui, |ui, l, c| {
            if c == 0 {
                texto(ui, &format!("linha {l}"), 12.0, egui::Color32::BLACK);
            } else if ui.add(Botao::new(&format!("Ação {l}"))).clicked() {
                s.botao += 1;
            }
        });
        if r.linha_clicada.is_some() {
            s.clicada = r.linha_clicada;
        }
    });
    let pos = h.get_by_label("linha 2").rect().center();
    clicar_em(&h, pos);
    h.run_steps(2);
    assert_eq!(h.state().clicada, Some(2));
    h.state_mut().clicada = None;
    let pos = h.get_by_label("Ação 0").rect().center();
    clicar_em(&h, pos);
    h.run_steps(2);
    assert_eq!((h.state().botao, h.state().clicada), (1, None));
}

/// A seção de perigo recolhida (o "Excluir a tabela" das Propriedades): nasce fechada e abre no clique.
#[test]
fn secao_recolhida_nasce_fechada_e_abre_no_clique() {
    use interface::pecas::painel::Secao;
    use interface::pecas::texto::texto;
    let mut h = harness((), |ui, _| {
        let secao = Secao::new("zona", "Excluir a tabela").perigo(true).recolhida(true);
        let _ = secao.mostrar(ui, |ui| texto(ui, "o corpo", 12.0, egui::Color32::BLACK));
    });
    assert!(h.query_by_label("o corpo").is_none());
    h.get_by_label("Seção Excluir a tabela").click();
    h.run_steps(2);
    assert!(h.query_by_label("o corpo").is_some());
}

#[derive(Default)]
struct EstadoPontaAPonta {
    doc: Option<egui::Rect>,
    ponta: Option<egui::Rect>,
    depois: Option<egui::Rect>,
    fora: Option<egui::Rect>,
}

/// Na página do documento, o `de_ponta_a_ponta` tira o `padding` dos lados e, no alto, o de cima (a lista de colunas
/// da Estrutura encosta na faixa e nas bordas); o que vem depois volta à coluna. Fora da página, fica no lugar.
#[test]
fn de_ponta_a_ponta_tira_o_padding_da_pagina() {
    use interface::pecas::documento::{Documento, Pagina, RECUO_PAGINA, de_ponta_a_ponta};
    let mut h = harness(EstadoPontaAPonta::default(), |ui, s| {
        let barra = |ui: &mut Ui, altura: f32| {
            ui.allocate_exact_size(egui::vec2(ui.available_width(), altura), egui::Sense::hover()).0
        };
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(600.0, 400.0));
        let mut d = Documento::abrir(ui, rect, None, false);
        d.faixa(ui, |_| {});
        s.doc = Some(d.rect_conteudo());
        d.pagina(ui, Pagina::new("ponta_a_ponta"), |ui| {
            s.ponta = Some(de_ponta_a_ponta(ui, |ui| barra(ui, 50.0)));
            s.depois = Some(barra(ui, 10.0));
        });
        let fora = egui::Rect::from_min_size(egui::pos2(650.0, 10.0), egui::vec2(200.0, 100.0));
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(fora));
        s.fora = Some(de_ponta_a_ponta(&mut filho, |ui| barra(ui, 20.0)));
    });
    h.run_steps(2);
    let s = h.state();
    let (doc, ponta, depois, fora) = (s.doc.unwrap(), s.ponta.unwrap(), s.depois.unwrap(), s.fora.unwrap());
    assert_eq!((ponta.left(), ponta.right(), ponta.top()), (doc.left(), doc.right(), doc.top()), "{ponta:?} {doc:?}");
    assert_eq!((depois.left(), depois.top()), (doc.left() + RECUO_PAGINA, ponta.bottom()));
    assert_eq!((fora.left(), fora.top()), (650.0, 10.0));
}

#[derive(Default)]
struct EstadoSemFim {
    depois_do_ponta: Option<egui::Rect>,
    coluna_depois: Option<egui::Rect>,
}

/// C6: um conteúdo que enche a altura toda (a grade de rolagem própria desenhada numa página que rola, no quadro em
/// que a tela acabou de ser lida) não leva a coluna nem a área de rolagem ao infinito: o `de_ponta_a_ponta` e a
/// página terminam onde o conteúdo começou, e tudo o que vem depois tem posição finita.
#[test]
fn conteudo_de_altura_infinita_nao_leva_a_pagina_ao_infinito() {
    use interface::pecas::documento::{Documento, Pagina, de_ponta_a_ponta};
    let mut h = harness(EstadoSemFim::default(), |ui, s| {
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(600.0, 400.0));
        let mut d = Documento::abrir(ui, rect, None, false);
        d.pagina(ui, Pagina::new("sem_fim"), |ui| {
            // enche a altura que o `Ui` diz ter: infinita, como a coluna da página
            de_ponta_a_ponta(ui, |ui| {
                let r = ui.max_rect();
                ui.allocate_rect(
                    egui::Rect::from_min_size(r.min, egui::vec2(r.width(), f32::INFINITY)),
                    egui::Sense::hover(),
                );
            });
            s.depois_do_ponta = Some(ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover()).0);
            s.coluna_depois = Some(ui.min_rect());
        });
    });
    h.run_steps(3);
    let s = h.state();
    let (depois, coluna) = (s.depois_do_ponta.unwrap(), s.coluna_depois.unwrap());
    assert!(depois.is_finite() && depois.top().is_finite(), "{depois:?}");
    assert!(coluna.bottom().is_finite(), "a coluna da página tem fim: {coluna:?}");
}

#[derive(Default)]
struct EstadoCamposPequenos {
    nome: String,
    tipo: String,
    padrao: String,
    perfil: i32,
    alturas: Vec<f32>,
}

/// O `.campo.pq` (as linhas de coluna da Nova tabela): a entrada, a entrada com sugestões e o select ficam com 26px, e
/// os de sempre com 28px.
#[test]
fn campos_pequenos_tem_26px_e_os_de_sempre_28px() {
    use interface::pecas::campos::{ALTURA_CAMPO, ALTURA_CAMPO_PEQUENO, EntradaSugestoes};
    let mut h = harness(EstadoCamposPequenos::default(), |ui, s| {
        let (nome, tipo, padrao) = (&mut s.nome, &mut s.tipo, &mut s.padrao);
        let sugestoes = ["INTEGER", "TEXT"];
        s.alturas = vec![
            ui.add(Entrada::new(nome).pequeno().rotulo("Nome")).rect.height(),
            EntradaSugestoes::new("tipo", tipo, &sugestoes).pequeno().rotulo("Tipo").mostrar(ui).rect.height(),
            ui.add(Entrada::new(padrao).rotulo("Padrão")).rect.height(),
            Select::new("perfil", &mut s.perfil, &[(0, "gestor"), (1, "rh")]).pequeno().mostrar(ui).rect.height(),
            Select::new("perfil2", &mut s.perfil, &[(0, "gestor"), (1, "rh")]).mostrar(ui).rect.height(),
        ];
    });
    h.run_steps(2);
    let a = &h.state().alturas;
    assert_eq!(a[..], [ALTURA_CAMPO_PEQUENO, ALTURA_CAMPO_PEQUENO, ALTURA_CAMPO, ALTURA_CAMPO_PEQUENO, ALTURA_CAMPO]);
}

struct PartesComIcone {
    trilho: egui::Rect,
    texto: f32,
    check: egui::Rect,
    check_com_icone: egui::Rect,
    ocupado: egui::Rect,
    ocupado_com_arco: egui::Rect,
}

#[test]
fn icone_nas_partes_do_trilho_na_caixa_e_o_arco_no_botao_de_texto() {
    use interface::icones::TipoArquivo;
    use interface::pecas::campos::Check;
    use interface::pecas::navegacao::{ParteSeletor, trilho_com_partes};
    let (mut a, mut b) = (true, true);
    let nada = egui::Rect::NOTHING;
    let estado = PartesComIcone {
        trilho: nada,
        texto: 0.0,
        check: nada,
        check_com_icone: nada,
        ocupado: nada,
        ocupado_com_arco: nada,
    };
    let mut h = harness(estado, move |ui, s| {
        let partes = [
            ParteSeletor::new("Excel").icone(Icone::Folha(TipoArquivo::Xlsx)),
            ParteSeletor::new("CSV").icone(Icone::Folha(TipoArquivo::Csv)),
        ];
        s.trilho =
            trilho_com_partes(ui, egui::Id::new("com_icone"), &partes, Some(0), TamanhoTrilho::Normal, true).1.rect;
        let fonte = fontes::fonte(12.0, fontes::Peso::Medio);
        s.texto = interface::pecas::texto::galley(ui, "Excel", fonte, egui::Color32::PLACEHOLDER).size().x;
        s.check = Check::new(&mut a, "clientes").mostrar(ui).rect;
        s.check_com_icone = Check::new(&mut b, "clientes").icone(Icone::Tabela).mostrar(ui).rect;
        s.ocupado = ui.add(Botao::primario("Gerando arquivo").ocupado(true)).rect;
        s.ocupado_com_arco = ui.add(Botao::primario("Gerando arquivo").ocupado(true).com_arco(true)).rect;
    });
    h.run_steps(2);
    let s = h.state();
    // o ícone de 16 e o `gap:6px` entram na largura da parte: texto (600) + ícone + gap + `padding:0 14px` + borda
    let parte_esperada = (s.texto + 16.0 + 6.0 + 28.0 + 2.0).ceil();
    assert_eq!(s.trilho.width(), 2.0 * parte_esperada + 6.0, "{:?}", s.trilho);
    let excel = h.get_by_label("Excel").rect();
    let csv = h.get_by_label("CSV").rect();
    assert_eq!(excel.width(), csv.width(), "partes iguais");
    for (icone, parte) in [(TipoArquivo::Xlsx, excel), (TipoArquivo::Csv, csv)] {
        let textura = icones::textura_carregada(&h.ctx, Icone::Folha(icone), 16.0).expect("a folha de 16");
        let malhas = malhas_com_textura(&h);
        let (_, r) = malhas.iter().find(|(t, m)| *t == textura && parte.contains_rect(*m)).expect("a folha na parte");
        assert_eq!(r.size(), egui::vec2(16.0, 16.0));
        assert!(r.right() < parte.center().x, "o ícone fica antes do rótulo");
    }
    // a caixa de marcar ganha 8px + 16px + o ícone entre a caixa e o texto
    assert_eq!(s.check_com_icone.width() - s.check.width(), 8.0 + 16.0);
    assert!(icones::textura_carregada(&h.ctx, Icone::Tabela, 16.0).is_some());
    // o ocupado do botão de texto: o ponto de 6px ou o arco de 14px, os dois com `gap:6px`
    assert_eq!(s.ocupado_com_arco.width() - s.ocupado.width(), 14.0 - 6.0);
}

// ---------------------------------------------------------------- etapa 5, passo 5: grade, abas e avisos

/// Um clique de `botao` em `pos` com as teclas `mods` apertadas (o `Response::clicked` lê as de `input().modifiers`).
fn clicar_com<S>(h: &mut Harness<'_, S>, pos: egui::Pos2, botao: egui::PointerButton, mods: Modifiers) {
    h.hover_at(pos);
    h.event(egui::Event::ModifiersChanged(mods));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button: botao, pressed, modifiers: mods });
    }
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
}

struct EstadoTexto {
    textos: Vec<(&'static str, bool)>,
    conteudo: egui::Rect,
}

#[test]
fn texto_de_celula_vazia_nao_mostra_vazio() {
    use interface::grade::pintar_texto_de_celula;
    let mut h = harness(
        EstadoTexto { textos: vec![("", false), ("a\nb", false), ("42", true)], conteudo: egui::Rect::NOTHING },
        |ui, s| {
            let p = tema::paleta(ui);
            for (i, (t, numerica)) in s.textos.iter().enumerate() {
                let conteudo =
                    egui::Rect::from_min_size(egui::pos2(20.0, 20.0 + 28.0 * i as f32), egui::vec2(200.0, 27.0));
                s.conteudo = conteudo;
                pintar_texto_de_celula(ui.painter(), p, conteudo, t, *numerica);
            }
        },
    );
    h.run_steps(1);
    let textos: Vec<(String, egui::Pos2, f32, egui::Color32)> = formas(&h)
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.pos, t.galley.size().x, t.fallback_color)),
            _ => None,
        })
        .collect();
    let p = tema::paleta_ctx(&h.ctx);
    assert!(textos.iter().all(|(t, ..)| t != "vazio"), "o CSV não tem o \"vazio\" em itálico: {textos:?}");
    assert_eq!(textos.len(), 2, "o texto vazio não pinta nada: {textos:?}");
    // as quebras viram espaço (uma linha só) e o texto é `--t`
    let (_, pos, _, cor) = textos.iter().find(|(t, ..)| t == "a b").expect("o texto numa linha");
    assert_eq!((pos.x, *cor), (20.0, p.fg));
    // a coluna numérica alinha à direita
    let (_, pos, largura, _) = textos.iter().find(|(t, ..)| t == "42").expect("o número");
    assert!((pos.x + largura - 220.0).abs() < 0.5, "{pos:?} + {largura}");
}

#[test]
fn celula_alterada_tem_faixa_e_tint() {
    use interface::grade::{LARGURA_FAIXA_ALTERADA, pintar_celula_alterada};
    let celula = egui::Rect::from_min_size(egui::pos2(30.0, 40.0), egui::vec2(120.0, 27.0));
    let mut h = harness(celula, |ui, c| pintar_celula_alterada(ui.painter(), tema::paleta(ui), *c));
    h.run_steps(1);
    let e = &tema::paleta_ctx(&h.ctx).estudio;
    let retangulos: Vec<(egui::Rect, egui::Color32)> = formas(&h)
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Rect(r) => Some((r.rect, r.fill)),
            _ => None,
        })
        .collect();
    assert_eq!(LARGURA_FAIXA_ALTERADA, 3.0);
    assert!(retangulos.contains(&(celula, e.tint)), "o fundo `--tint` na célula toda: {retangulos:?}");
    let faixa = egui::Rect::from_min_size(celula.min, egui::vec2(3.0, celula.height()));
    assert!(retangulos.contains(&(faixa, e.acct)), "a faixa de 3px em `--acct` à esquerda: {retangulos:?}");
    // a faixa vem depois do fundo (por cima)
    let ordem = |alvo: (egui::Rect, egui::Color32)| retangulos.iter().position(|r| *r == alvo).unwrap();
    assert!(ordem((faixa, e.acct)) > ordem((celula, e.tint)));
}

/// Um núcleo como o do CSV: caixa de marcar, número da linha e três colunas de dados; o gancho do `th` pinta o nome e
/// diz quais colunas estão selecionadas.
struct DesenhoSelecao {
    selecionadas: Vec<usize>,
}

const NOMES_CSV: [&str; 5] = ["", "#", "data", "regiao", "valor"];

impl interface::grade::DesenhoGrade for DesenhoSelecao {
    fn celula(&mut self, _ui: &mut Ui, _c: &interface::grade::CelulaVisivel) {}

    fn cabecalho(&mut self, ui: &mut Ui, h: &interface::grade::CabecalhoVisivel) {
        if h.coluna >= 2 {
            let th = interface::grade::Th {
                selecionada: self.selecionadas.contains(&h.coluna),
                ..interface::grade::Th::new(NOMES_CSV[h.coluna])
            };
            interface::grade::pintar_cabecalho_coluna(
                &ui.painter().with_clip_rect(h.recorte),
                tema::paleta(ui),
                h.conteudo,
                &th,
            );
        }
    }

    fn th_selecionada(&mut self, coluna: usize) -> bool {
        self.selecionadas.contains(&coluna)
    }
}

struct EstadoCabecalho {
    nucleo: interface::grade::NucleoGrade,
    desenho: DesenhoSelecao,
    resposta: interface::grade::RespostaNucleo,
    clicou: Vec<(usize, interface::grade::Toques)>,
    menus: Vec<usize>,
}

fn nucleo_de_csv(selecionadas: Vec<usize>) -> EstadoCabecalho {
    use interface::grade::{Navegacao, NucleoGrade};
    EstadoCabecalho {
        nucleo: NucleoGrade::new("csv", 3, vec![32.0, 48.0, 100.0, 100.0, 100.0])
            .com_navegacao(Navegacao::Dados)
            .com_navegaveis(2..5)
            .com_coluna_marcar(0)
            .com_coluna_numero(1),
        desenho: DesenhoSelecao { selecionadas },
        resposta: Default::default(),
        clicou: Vec::new(),
        menus: Vec::new(),
    }
}

fn harness_do_cabecalho(selecionadas: Vec<usize>) -> Harness<'static, EstadoCabecalho> {
    harness(nucleo_de_csv(selecionadas), |ui, s| {
        let r = s.nucleo.mostrar(ui, 300.0, &mut s.desenho);
        if let Some(c) = r.clicou_cabecalho {
            s.clicou.push(c);
        }
        if let Some(c) = r.menu_cabecalho {
            s.menus.push(c);
        }
        s.resposta = r;
    })
}

#[test]
fn cabecalho_selecionado_pinta_a_coluna() {
    use interface::grade::{ALTURA_CABECALHO, ALTURA_LINHA};
    let h = harness_do_cabecalho(vec![3]);
    let e = &tema::paleta_ctx(&h.ctx).estudio;
    let retangulos: Vec<(egui::Rect, egui::Color32)> = formas(&h)
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Rect(r) => Some((r.rect, r.fill)),
            _ => None,
        })
        .collect();
    // a coluna 3 começa em 8 (a margem do harness) + 32 + 48 + 100
    let x = 8.0 + 32.0 + 48.0 + 100.0;
    // o `th` em `--sel` (por cima do fundo do documento), sem o fio de baixo nem o da direita
    let th = egui::Rect::from_min_max(egui::pos2(x, 8.0), egui::pos2(x + 99.0, 8.0 + ALTURA_CABECALHO - 1.0));
    assert!(retangulos.contains(&(th, e.sel)), "o `th` em `--sel`: {retangulos:?}");
    // as três linhas em `--tint`, só nessa coluna
    let tint: Vec<egui::Rect> = retangulos.iter().filter(|(_, c)| *c == e.tint).map(|(r, _)| *r).collect();
    assert_eq!(tint.len(), 3, "{tint:?}");
    for (i, r) in tint.iter().enumerate() {
        let topo = 8.0 + ALTURA_CABECALHO + ALTURA_LINHA * i as f32;
        assert_eq!(*r, egui::Rect::from_min_max(egui::pos2(x, topo), egui::pos2(x + 99.0, topo + ALTURA_LINHA - 1.0)));
    }
    // o nome da coluna selecionada em `--t`; os outros em `--t2`
    let cor_do_nome = |nome: &str| {
        formas(&h)
            .iter()
            .find_map(|f| match f {
                egui::Shape::Text(t) if t.galley.text() == nome => Some(t.galley.job.sections[0].format.color),
                _ => None,
            })
            .unwrap_or_else(|| panic!("sem o nome {nome}"))
    };
    assert_eq!(cor_do_nome("regiao"), e.t);
    assert_eq!(cor_do_nome("valor"), e.t2);
    assert_eq!(cor_do_nome("data"), e.t2);
    // sem seleção, nada disso
    let h = harness_do_cabecalho(vec![]);
    assert!(!formas(&h).iter().any(|f| matches!(f, egui::Shape::Rect(r) if r.fill == e.sel || r.fill == e.tint)));
}

#[test]
fn clique_no_cabecalho_devolve_os_toques() {
    use interface::grade::Toques;
    let mut h = harness_do_cabecalho(vec![]);
    let y = 8.0 + 15.0;
    let x_da = |c: usize| 8.0 + [0.0, 32.0, 80.0, 180.0, 280.0][c] + 40.0;
    clicar_com(&mut h, egui::pos2(x_da(3), y), egui::PointerButton::Primary, Modifiers::NONE);
    h.run_steps(60);
    clicar_com(&mut h, egui::pos2(x_da(4), y), egui::PointerButton::Primary, Modifiers::CTRL);
    h.run_steps(60);
    clicar_com(&mut h, egui::pos2(x_da(2), y), egui::PointerButton::Primary, Modifiers::SHIFT);
    h.run_steps(60);
    assert_eq!(
        h.state().clicou,
        [
            (3, Toques { ctrl: false, shift: false }),
            (4, Toques { ctrl: true, shift: false }),
            (2, Toques { ctrl: false, shift: true }),
        ]
    );
    // a caixa de marcar e o número da linha não selecionam coluna
    h.state_mut().clicou.clear();
    clicar_com(&mut h, egui::pos2(x_da(0) - 24.0, y), egui::PointerButton::Primary, Modifiers::NONE);
    h.run_steps(60);
    clicar_com(&mut h, egui::pos2(x_da(1), y), egui::PointerButton::Primary, Modifiers::NONE);
    h.run_steps(2);
    assert!(h.state().clicou.is_empty(), "{:?}", h.state().clicou);
    // o clique no corpo não é clique no cabeçalho
    h.run_steps(60);
    clicar_com(&mut h, egui::pos2(x_da(3), y + 60.0), egui::PointerButton::Primary, Modifiers::NONE);
    h.run_steps(2);
    assert!(h.state().clicou.is_empty());
}

#[test]
fn clique_direito_no_cabecalho_pede_o_menu() {
    let mut h = harness_do_cabecalho(vec![]);
    let y = 8.0 + 15.0;
    clicar_com(&mut h, egui::pos2(8.0 + 180.0 + 40.0, y), egui::PointerButton::Secondary, Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(h.state().menus, [3]);
    assert!(h.state().clicou.is_empty(), "o botão direito não é o clique que seleciona");
    // na caixa de marcar e no número da linha, não há menu de coluna
    h.run_steps(60);
    clicar_com(&mut h, egui::pos2(8.0 + 32.0 + 20.0, y), egui::PointerButton::Secondary, Modifiers::NONE);
    h.run_steps(2);
    assert_eq!(h.state().menus, [3]);
}

#[derive(Default)]
struct EstadoAbaAlterada {
    alterada: bool,
    fechadas: Vec<usize>,
}

fn abas_com_alterada(ui: &mut Ui, s: &mut EstadoAbaAlterada) {
    use interface::pecas::abas_documento::{AbaDocumento, FaixaAbas};
    let abas = [
        AbaDocumento::new(Icone::Banco, "exemplo.db").sub("Visão geral"),
        AbaDocumento::new(Icone::Folha(icones::TipoArquivo::Csv), "vendas_2025.csv").alterada(s.alterada),
    ];
    let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(700.0, 40.0));
    let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    let r = FaixaAbas::new("alterada", &abas).ativa(Some(0)).mostrar(&mut filho);
    if let Some(i) = r.fechar {
        s.fechadas.push(i);
    }
}

/// Os círculos cheios de `cor` com o `raio` no último quadro.
fn circulos<S>(h: &Harness<'_, S>, cor: egui::Color32, raio: f32) -> Vec<egui::Pos2> {
    formas(h)
        .iter()
        .filter_map(|f| match f {
            egui::Shape::Circle(c) if c.fill == cor && (c.radius - raio).abs() < 0.01 => Some(c.center),
            _ => None,
        })
        .collect()
}

#[test]
fn aba_alterada_mostra_o_ponto_e_vira_x_no_hover() {
    use interface::pecas::abas_documento::LADO_PONTO;
    let mut h = harness(EstadoAbaAlterada { alterada: true, ..Default::default() }, abas_com_alterada);
    h.run_steps(2);
    let acct = tema::paleta_ctx(&h.ctx).estudio.acct;
    assert_eq!(LADO_PONTO, 7.0);
    let aba = h.get(by().role(Role::Tab).label("vendas_2025.csv, alterada")).rect();
    // o ponto de 7px em `--acct` no lugar do ×: o centro do × (6px da borda direita e 1 de borda)
    let pontos = circulos(&h, acct, 3.5);
    assert_eq!(pontos.len(), 1, "um ponto: {pontos:?}");
    assert!((pontos[0].x - (aba.right() - 1.0 - 6.0 - 10.0)).abs() < 0.5 && (pontos[0].y - aba.center().y).abs() < 0.5);
    // a aba sem alteração não tem ponto (a outra aba, a ativa, nem a alterada que não está sob o ponteiro)
    // o ponteiro na aba: o ponto some e o × volta (o círculo do hover do × fica no `--h` quando o ponteiro está nele)
    h.get(by().role(Role::Tab).label("vendas_2025.csv, alterada")).hover();
    h.run_steps(3);
    assert!(circulos(&h, acct, 3.5).is_empty(), "no hover da aba o ponto vira ×");
    // fora da aba, o ponto volta
    h.hover_at(egui::pos2(600.0, 300.0));
    h.run_steps(3);
    assert_eq!(circulos(&h, acct, 3.5).len(), 1, "o ponto volta quando o ponteiro sai");
    // o × continua sendo o botão de fechar (com o ponteiro na aba), com o nome dela
    h.get_by_label("Fechar a aba vendas_2025.csv, alterada").click();
    h.run_steps(2);
    assert_eq!(h.state().fechadas, [1]);
    // o foco do teclado na aba também mostra o ×
    h.state_mut().fechadas.clear();
    h.hover_at(egui::pos2(600.0, 300.0));
    h.get(by().role(Role::Tab).label("vendas_2025.csv, alterada")).focus();
    h.run_steps(3);
    assert!(circulos(&h, acct, 3.5).is_empty(), "no foco da aba o ponto vira ×");
    // sem alteração, nunca há ponto
    h.state_mut().alterada = false;
    h.hover_at(egui::pos2(600.0, 300.0));
    h.run_steps(3);
    assert!(circulos(&h, acct, 3.5).is_empty());
}

#[test]
fn aba_alterada_tem_rotulo_com_alterada() {
    use interface::pecas::abas_documento::AbaDocumento;
    let csv = AbaDocumento::new(Icone::Folha(icones::TipoArquivo::Csv), "vendas_2025.csv");
    assert_eq!(csv.rotulo(), "vendas_2025.csv");
    assert_eq!(csv.alterada(true).rotulo(), "vendas_2025.csv, alterada");
    assert_eq!(
        AbaDocumento::new(Icone::Tabela, "clientes").sub("Dados").alterada(true).rotulo(),
        "clientes · Dados, alterada"
    );
    let mut h = harness(EstadoAbaAlterada { alterada: true, ..Default::default() }, abas_com_alterada);
    h.run_steps(2);
    assert!(h.query(by().role(Role::Tab).label("vendas_2025.csv, alterada")).is_some());
    assert!(h.query(by().role(Role::Button).label("Fechar a aba vendas_2025.csv, alterada")).is_some());
    h.state_mut().alterada = false;
    h.run_steps(2);
    assert!(h.query(by().role(Role::Tab).label("vendas_2025.csv")).is_some());
}

#[derive(Default)]
struct EstadoAvisoAcoes {
    clicada: Option<usize>,
    caixa: Option<egui::Rect>,
    quantas: usize,
}

#[test]
fn aviso_com_acoes_tem_ate_tres_botoes() {
    use interface::pecas::avisos::{AcaoAviso, MAXIMO_ACOES, TomAviso, aviso_com_acoes};
    use interface::pecas::texto::Trecho;
    assert_eq!(MAXIMO_ACOES, 3);
    let mut h = harness(EstadoAvisoAcoes { quantas: 3, ..Default::default() }, |ui, s| {
        let todas = [
            AcaoAviso::principal("Colar 1.180"),
            AcaoAviso::neutra("Acrescentar 24 linhas"),
            AcaoAviso::sutil("Cancelar").dica("Esc"),
        ];
        let r = aviso_com_acoes(
            ui,
            &[Trecho::Normal("A coluna colada tem 1.204 valores e o arquivo, 1.180 linhas.")],
            TomAviso::Atencao,
            &todas[..s.quantas],
        );
        s.caixa = Some(r.resposta.rect);
        if r.clicada.is_some() {
            s.clicada = r.clicada;
        }
    });
    h.run_steps(3);
    let botao = |h: &Harness<'_, EstadoAvisoAcoes>, rotulo: &str| h.get(by().role(Role::Button).label(rotulo)).rect();
    let (colar, acrescentar, cancelar) =
        (botao(&h, "Colar 1.180"), botao(&h, "Acrescentar 24 linhas"), botao(&h, "Cancelar"));
    // os três na mesma linha, em ordem, com 6px entre eles, de 24px de altura, dentro da caixa
    let caixa = h.state().caixa.expect("a caixa");
    assert!(colar.top() == acrescentar.top() && acrescentar.top() == cancelar.top());
    assert_eq!(colar.height(), 24.0, "`.btn.pq`");
    assert!(
        (acrescentar.left() - colar.right() - 6.0).abs() < 0.5
            && (cancelar.left() - acrescentar.right() - 6.0).abs() < 0.5
    );
    assert!(caixa.contains_rect(colar) && caixa.contains_rect(cancelar), "{caixa:?}");
    // embaixo do texto
    assert!(
        colar.top() > caixa.top() + 10.0 + 17.0,
        "os botões vão embaixo do texto de uma linha: {caixa:?} {colar:?}"
    );
    // o clique devolve o índice
    h.get_by_label("Acrescentar 24 linhas").click();
    h.run_steps(2);
    assert_eq!(h.state().clicada, Some(1));
    h.state_mut().clicada = None;
    h.get_by_label("Cancelar").click();
    h.run_steps(2);
    assert_eq!(h.state().clicada, Some(2));
    // com uma só ação, só ela
    h.state_mut().quantas = 1;
    h.run_steps(3);
    assert!(h.query(by().role(Role::Button).label("Colar 1.180")).is_some());
    assert!(h.query(by().role(Role::Button).label("Cancelar")).is_none());
    // sem ação, só o aviso
    h.state_mut().quantas = 0;
    h.run_steps(3);
    assert!(h.query(by().role(Role::Button).label("Colar 1.180")).is_none());
}

/// Grade (etapa 6, passo 3): painel congelado, células mescladas, larguras do arquivo e o clique no número da linha.
mod congelado_e_mesclas {
    use super::*;
    use interface::grade::{
        ALTURA_CABECALHO, ALTURA_LINHA, CabecalhoVisivel, CelulaMesclada, CelulaVisivel, ColunaDoArquivo, DesenhoGrade,
        IndiceDeMesclas, Mescla, Navegacao, NucleoGrade, RespostaNucleo, Toques, largura_do_arquivo,
        larguras_do_arquivo,
    };

    /// O que o núcleo entregou aos ganchos num quadro, com as mescladas.
    #[derive(Default)]
    struct Registro {
        celulas: Vec<CelulaVisivel>,
        mescladas: Vec<CelulaMesclada>,
        ths: Vec<CabecalhoVisivel>,
    }

    impl DesenhoGrade for Registro {
        fn celula(&mut self, _ui: &mut Ui, c: &CelulaVisivel) {
            self.celulas.push(*c);
        }
        fn celula_mesclada(&mut self, _ui: &mut Ui, c: &CelulaMesclada) {
            self.mescladas.push(*c);
        }
        fn cabecalho(&mut self, _ui: &mut Ui, h: &CabecalhoVisivel) {
            self.ths.push(*h);
        }
    }

    struct Estado {
        nucleo: NucleoGrade,
        registro: Registro,
        resposta: RespostaNucleo,
        altura: f32,
        numeros: Vec<(usize, Toques)>,
        maior_quadro: usize,
    }

    fn harness_da_grade(nucleo: NucleoGrade, altura: f32) -> Harness<'static, Estado> {
        let estado = Estado {
            nucleo,
            registro: Registro::default(),
            resposta: RespostaNucleo::default(),
            altura,
            numeros: Vec::new(),
            maior_quadro: 0,
        };
        harness(estado, |ui, s| {
            s.registro = Registro::default();
            s.resposta = s.nucleo.mostrar(ui, s.altura, &mut s.registro);
            if let Some(n) = s.resposta.clicou_numero_da_linha {
                s.numeros.push(n);
            }
            s.maior_quadro = s.maior_quadro.max(s.registro.celulas.len());
        })
    }

    fn celula(h: &Harness<'_, Estado>, linha: usize, coluna: usize) -> Option<CelulaVisivel> {
        h.state().registro.celulas.iter().find(|c| c.linha == linha && c.coluna == coluna).copied()
    }

    /// Os segmentos de reta (`vline` e `hline`) do último quadro.
    fn segmentos(h: &Harness<'_, Estado>) -> Vec<(egui::Pos2, egui::Pos2)> {
        formas(h)
            .iter()
            .filter_map(|f| match f {
                egui::Shape::LineSegment { points, .. } => Some((points[0], points[1])),
                _ => None,
            })
            .collect()
    }

    const X0: f32 = 8.0;
    const Y0: f32 = 8.0 + ALTURA_CABECALHO;

    #[test]
    fn congelado_gruda_as_linhas_sob_o_cabecalho() {
        let mut h = harness_da_grade(NucleoGrade::new("cong-l", 200, vec![100.0; 4]).com_congelado(2, 0), 400.0);
        let topos = |h: &Harness<'_, Estado>| [0, 1].map(|l| celula(h, l, 0).expect("linha congelada").celula.top());
        assert_eq!(topos(&h), [Y0, Y0 + ALTURA_LINHA]);
        assert_eq!(h.state().nucleo.congelado_efetivo().linhas, 2);
        h.state_mut().nucleo.selecionar(Some((199, 1)));
        h.run_steps(8);
        // as duas linhas ficam onde estavam, sob o cabeçalho
        assert_eq!(topos(&h), [Y0, Y0 + ALTURA_LINHA]);
        assert_eq!(h.state().registro.celulas.iter().filter(|c| c.linha == 0).count(), 4, "uma vez por coluna");
        // a grade rolou até o fim e nada do que rola passa por cima das congeladas
        let r = &h.state().registro;
        assert!(r.celulas.iter().any(|c| c.linha == 199));
        let base = Y0 + 2.0 * ALTURA_LINHA;
        for c in r.celulas.iter().filter(|c| c.linha >= 2) {
            assert!(c.recorte.top() >= base - 0.01, "a linha {} invade as congeladas: {:?}", c.linha, c.recorte);
        }
        // o cabeçalho continua no topo
        assert!(r.ths.iter().all(|t| t.th.top() == 8.0));
    }

    #[test]
    fn congelado_gruda_as_colunas_a_esquerda() {
        let mut h = harness_da_grade(NucleoGrade::new("cong-c", 20, vec![120.0; 14]).com_congelado(0, 2), 400.0);
        assert_eq!(h.state().nucleo.congelado_efetivo().colunas, 2);
        h.state_mut().nucleo.selecionar(Some((0, 13)));
        h.run_steps(8);
        // as duas primeiras ficam no lugar; a última coluna apareceu à direita delas
        assert_eq!(celula(&h, 0, 0).expect("presa").celula.left(), X0);
        assert_eq!(celula(&h, 0, 1).expect("presa").celula.left(), X0 + 120.0);
        let r = &h.state().registro;
        assert!(r.celulas.iter().any(|c| c.coluna == 13), "rolou até a última coluna");
        for c in r.celulas.iter().filter(|c| c.coluna >= 2) {
            assert!(c.recorte.left() >= X0 + 240.0 - 0.01, "a coluna {} invade as presas: {:?}", c.coluna, c.recorte);
        }
        // os `th` das presas também, e os das outras passam por baixo delas
        let th = |c: usize| r.ths.iter().find(|t| t.coluna == c).expect("th").th;
        assert_eq!((th(0).left(), th(1).left()), (X0, X0 + 120.0));
        assert!(r.ths.iter().filter(|t| t.coluna >= 2).all(|t| t.recorte.left() >= X0 + 240.0 - 0.01));
    }

    #[test]
    fn congelado_prende_tambem_a_coluna_do_numero() {
        // a coluna do número da linha (0) e a primeira de dados (1) ficam; o `th` da emenda é a 2
        let nucleo = NucleoGrade::new(
            "cong-n",
            20,
            vec![
                56.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            ],
        )
        .com_coluna_numero(0)
        .com_navegaveis(1..15)
        .com_congelado(0, 1);
        let mut h = harness_da_grade(nucleo, 400.0);
        assert_eq!(h.state().nucleo.congelado_efetivo().colunas, 2, "a do número e a primeira de dados");
        h.state_mut().nucleo.selecionar(Some((0, 14)));
        h.run_steps(8);
        assert_eq!(celula(&h, 0, 0).expect("número").celula.left(), X0);
        assert_eq!(celula(&h, 0, 1).expect("presa").celula.left(), X0 + 56.0);
        assert!(
            h.state().registro.celulas.iter().filter(|c| c.coluna >= 2).all(|c| c.recorte.left() >= X0 + 156.0 - 0.01)
        );
    }

    #[test]
    fn congelado_com_mais_de_60_por_cento_e_ignorado() {
        // altura 300: o cabeçalho e 8 linhas (30 + 224) passam de 60% (180) → ignora; 3 linhas (114) valem
        let mut h = harness_da_grade(NucleoGrade::new("cong-60", 200, vec![100.0; 4]).com_congelado(8, 0), 300.0);
        assert_eq!(h.state().nucleo.congelado_efetivo().linhas, 0, "ignorado");
        h.state_mut().nucleo.selecionar(Some((199, 1)));
        h.run_steps(8);
        assert!(celula(&h, 0, 0).is_none(), "a linha 0 rolou para fora: a rolagem não prende");
        let h = harness_da_grade(NucleoGrade::new("cong-60b", 200, vec![100.0; 4]).com_congelado(3, 0), 300.0);
        assert_eq!(h.state().nucleo.congelado_efetivo().linhas, 3);
        // as colunas: 10 × 100 passam de 60% da largura (uns 590); 5 × 100 não
        let h = harness_da_grade(NucleoGrade::new("cong-60c", 20, vec![100.0; 14]).com_congelado(0, 10), 400.0);
        assert_eq!(h.state().nucleo.congelado_efetivo().colunas, 0, "ignorado");
        let h = harness_da_grade(NucleoGrade::new("cong-60d", 20, vec![100.0; 14]).com_congelado(0, 5), 400.0);
        assert_eq!(h.state().nucleo.congelado_efetivo().colunas, 5);
    }

    #[test]
    fn sem_congelado_a_grade_rola_inteira() {
        let mut h = harness_da_grade(NucleoGrade::new("cong-sem", 200, vec![100.0; 4]), 400.0);
        assert_eq!(h.state().nucleo.congelado(), None);
        assert_eq!(h.state().nucleo.congelado_efetivo(), Default::default());
        assert_eq!(celula(&h, 0, 0).expect("linha 0").celula.top(), Y0);
        h.state_mut().nucleo.selecionar(Some((199, 1)));
        h.run_steps(8);
        assert!(celula(&h, 0, 0).is_none(), "sem congelado, a linha 0 rola para fora");
        assert!(celula(&h, 199, 0).is_some());
    }

    fn nucleo_com_mesclas(mesclas: &[(usize, usize, usize, usize)]) -> NucleoGrade {
        let indice = IndiceDeMesclas::nova(mesclas.iter().copied().map(Mescla::from));
        NucleoGrade::new("mescla", 10, vec![50.0, 60.0, 70.0, 80.0, 90.0]).com_mesclas(std::sync::Arc::new(indice))
    }

    #[test]
    fn mescla_pinta_o_texto_no_retangulo_todo() {
        // A1:C1 do Excel, sem a primeira coluna: (0,1) a (0,3); e duas linhas por duas colunas em (2,0) a (3,1)
        let mut h = harness_da_grade(nucleo_com_mesclas(&[(0, 1, 0, 3), (2, 0, 3, 1)]), 400.0);
        let r = &h.state().registro;
        assert_eq!(r.mescladas.len(), 2);
        let m = r.mescladas.iter().find(|m| (m.linha, m.coluna) == (0, 1)).expect("a de cima");
        // da esquerda da coluna 1 até o fio da direita da 3 (60 + 70 + 80), sem o fio de baixo
        assert_eq!(
            m.celula,
            egui::Rect::from_min_size(egui::pos2(X0 + 50.0, Y0), egui::vec2(210.0, ALTURA_LINHA - 1.0))
        );
        // o conteúdo: sem o padding de 10px e sem o fio da direita
        assert_eq!((m.conteudo.left() - m.celula.left(), m.celula.right() - m.conteudo.right()), (10.0, 11.0));
        assert_eq!((m.conteudo.top(), m.conteudo.bottom()), (m.celula.top(), m.celula.bottom()));
        assert!(!m.ativa);
        let alta = r.mescladas.iter().find(|m| (m.linha, m.coluna) == (2, 0)).expect("a de duas linhas");
        assert_eq!((alta.celula.width(), alta.celula.height()), (110.0, 2.0 * ALTURA_LINHA - 1.0));
        // as outras células da mescla não são entregues; as de fora são
        for (l, c) in [(0, 1), (0, 2), (0, 3), (2, 0), (2, 1), (3, 0), (3, 1)] {
            assert!(celula(&h, l, c).is_none(), "({l}, {c}) está dentro de uma mescla");
        }
        for (l, c) in [(0, 0), (0, 4), (1, 1), (2, 2), (4, 0)] {
            assert!(celula(&h, l, c).is_some(), "({l}, {c}) é uma célula comum");
        }
        // os fios de dentro não se pintam: a linha 2 (dentro da mescla alta) não tem o fio de baixo nas colunas 0 e 1,
        // e o fio da direita da coluna 0 não passa pelas linhas 2 e 3
        let segs = segmentos(&h);
        let fio_baixo_linha2 = Y0 + 3.0 * ALTURA_LINHA - 0.5;
        let cobre_x = |x0: f32, x1: f32| {
            segs.iter().any(|(a, b)| a.y == fio_baixo_linha2 && b.y == a.y && a.x <= x0 && b.x >= x1)
        };
        assert!(!cobre_x(X0 + 10.0, X0 + 100.0), "o fio de baixo da linha 2 passa por dentro da mescla");
        assert!(cobre_x(X0 + 120.0, X0 + 400.0), "e segue nas colunas de fora");
        let fio_dir_col0 = X0 + 50.0 - 0.5;
        let fio_vertical = |linha: usize| {
            let y = Y0 + linha as f32 * ALTURA_LINHA + 5.0;
            segs.iter().any(|(a, b)| a.x == fio_dir_col0 && b.x == a.x && a.y <= y && b.y >= y)
        };
        assert!(fio_vertical(1) && fio_vertical(4), "fora da mescla o fio vertical existe");
        assert!(!fio_vertical(2) && !fio_vertical(3), "dentro dela não");
        // a ativa dentro da mescla contorna a mescla inteira
        h.state_mut().nucleo.selecionar(Some((0, 1)));
        h.run_steps(2);
        assert!(h.state().registro.mescladas.iter().any(|m| (m.linha, m.coluna) == (0, 1) && m.ativa));
        let contorno = formas(&h)
            .iter()
            .any(|f| matches!(f, egui::Shape::Rect(r) if r.stroke.width == 2.0 && r.rect.width() == 210.0 + 0.0));
        assert!(contorno, "o contorno de 2px na mescla toda");
    }

    #[test]
    fn clicar_na_mescla_ativa_a_de_cima() {
        let mut h = harness_da_grade(nucleo_com_mesclas(&[(1, 1, 2, 2)]), 400.0);
        // o centro da célula (2, 2): coluna 2 começa em 50 + 60, linha 2 em 2 × 28
        let (x, y) = (X0 + 50.0 + 60.0 + 35.0, Y0 + 2.0 * ALTURA_LINHA + 14.0);
        clicar_com(&mut h, egui::pos2(x, y), egui::PointerButton::Primary, Modifiers::NONE);
        h.run_steps(3);
        assert_eq!(h.state().nucleo.selecao(), Some((1, 1)), "a de cima à esquerda");
        // sem mescla, o clique ativa a própria célula
        h.run_steps(60);
        clicar_com(
            &mut h,
            egui::pos2(X0 + 25.0, Y0 + 4.0 * ALTURA_LINHA + 14.0),
            egui::PointerButton::Primary,
            Modifiers::NONE,
        );
        h.run_steps(3);
        assert_eq!(h.state().nucleo.selecao(), Some((4, 0)));
    }

    #[test]
    fn mescla_gigante_nao_se_desenha() {
        let indice = IndiceDeMesclas::nova([
            Mescla::new(0, 0, 1000, 1), // 1.001 linhas: não
            Mescla::new(0, 0, 999, 1),  // 1.000 linhas: sim
            Mescla::new(0, 0, 0, 99),   // 100 colunas: sim
            Mescla::new(0, 0, 0, 100),  // 101 colunas: não
            Mescla::new(5, 0, 5, 0),    // uma célula só: não é mescla
            Mescla::new(3, 3, 1, 1),    // os cantos trocados: não
        ]);
        assert_eq!((indice.len(), indice.descartadas()), (2, 2));
        // numa grade com a gigante, cada célula aparece como é e nada é pintado como mescla
        let nucleo = NucleoGrade::new("gigante", 10, vec![50.0; 5])
            .com_mesclas(std::sync::Arc::new(IndiceDeMesclas::nova([Mescla::new(0, 0, 1000, 1)])));
        let h = harness_da_grade(nucleo, 400.0);
        assert!(h.state().registro.mescladas.is_empty());
        assert!(celula(&h, 0, 0).is_some() && celula(&h, 0, 1).is_some() && celula(&h, 3, 1).is_some());
    }

    #[test]
    fn indice_de_mesclas_acha_so_as_que_cruzam_a_tela() {
        let mut v: Vec<Mescla> = (0..30_000).step_by(3).map(|r| Mescla::new(r, 0, r + 1, 1)).collect();
        v.push(Mescla::new(10_000, 2, 10_499, 3));
        let indice = IndiceDeMesclas::nova(v);
        for janela in
            [0..10, 9_998..10_004, 10_400..10_410, 10_499..10_500, 10_500..10_501, 29_990..30_100, 50_000..50_010]
        {
            let achadas: Vec<Mescla> = indice.que_cruzam(janela.clone()).copied().collect();
            let esperadas: Vec<Mescla> = indice
                .mesclas()
                .iter()
                .filter(|m| m.linha0 < janela.end && m.linha1 >= janela.start)
                .copied()
                .collect();
            assert_eq!(achadas, esperadas, "{janela:?}");
        }
        // no meio da alta só ela (e as de duas linhas que começam dentro dela)
        let alta = Mescla::new(10_000, 2, 10_499, 3);
        assert!(indice.que_cruzam(10_400..10_410).any(|m| *m == alta));
        assert_eq!(indice.que_cruzam(50_000..50_010).count(), 0);
        assert_eq!(indice.mescla_em(10_200, 3), Some(&alta));
        assert_eq!(indice.mescla_em(10_200, 4), None);
        assert_eq!(indice.mescla_em(1, 1), Some(&Mescla::new(0, 0, 1, 1)));
        assert_eq!(indice.mescla_em(2, 0), None, "a linha 2 é a folga entre duas mesclas");
    }

    #[test]
    fn larguras_externas_nao_usam_larguras_iniciais() {
        let colunas = vec![ColunaGrade::new("uma coluna de nome bem comprido"), ColunaGrade::new("b")];
        let linhas = vec![vec![Celula::Texto("x".repeat(80)), Celula::Inteiro(1)]];
        let comuns = GradeSomenteLeitura::new("sem-ext", colunas.clone(), linhas.clone());
        let externas = GradeSomenteLeitura::new("ext", colunas, linhas).com_larguras_externas(vec![64.0, 30.0]);
        let mut h = harness((comuns, externas), |ui, s| {
            s.0.mostrar(ui, 100.0);
            s.1.mostrar(ui, 100.0);
        });
        h.run_steps(2);
        let (comuns, externas) = h.state();
        // as iniciais medem o texto (e passam de 64); as do arquivo ficam como vieram
        assert!(comuns.larguras().expect("medidas")[0] > 64.0);
        assert_eq!(externas.larguras(), Some(&[64.0, 30.0][..]));
        assert!(externas.nucleo().larguras_externas() && !comuns.nucleo().larguras_externas());
        // a alça continua, e o mínimo é 24 (o de uma coluna estreita do arquivo), não 48
        let mut externas = h.state().1.nucleo().clone();
        externas.definir_largura(1, 10.0);
        assert_eq!(externas.larguras()[1], 24.0);
        externas.definir_largura(1, 90.0);
        assert_eq!(externas.larguras()[1], 90.0);
    }

    #[test]
    fn largura_do_arquivo_pela_formula_e_coluna_oculta_some() {
        // round(caracteres × 7 + 5), de 24 a 640; a padrão (8,43) dá 64
        assert_eq!(largura_do_arquivo(8.43), 64.0);
        assert_eq!(largura_do_arquivo(18.0), 131.0);
        assert_eq!(largura_do_arquivo(30.0), 215.0);
        assert_eq!(largura_do_arquivo(0.0), 24.0, "5px passam a 24");
        assert_eq!(largura_do_arquivo(255.0), 640.0, "o teto");
        assert_eq!(largura_do_arquivo(f64::NAN), 64.0, "sem número, a padrão");
        assert_eq!(largura_do_arquivo(-3.0), 64.0);
        let saida = larguras_do_arquivo(&[
            ColunaDoArquivo { caracteres: Some(18.0), oculta: false },
            ColunaDoArquivo { caracteres: Some(30.0), oculta: true },
            ColunaDoArquivo { caracteres: None, oculta: false },
            ColunaDoArquivo { caracteres: Some(14.0), oculta: false },
        ]);
        assert_eq!(saida.larguras, [131.0, 64.0, 103.0], "a oculta sai e a sem largura é a padrão");
        assert_eq!(saida.do_arquivo, [0, 2, 3], "cada coluna que ficou lembra a dela no arquivo");
    }

    #[test]
    fn clique_no_numero_da_linha_devolve_os_toques() {
        // a caixa de marcar (0), o número da linha (1) e as colunas de dados (2 a 4)
        let nucleo = NucleoGrade::new("numero", 5, vec![32.0, 48.0, 100.0, 100.0, 100.0])
            .com_navegacao(Navegacao::Dados)
            .com_navegaveis(2..5)
            .com_coluna_marcar(0)
            .com_coluna_numero(1);
        let mut h = harness_da_grade(nucleo, 300.0);
        let no_numero = |linha: usize| egui::pos2(X0 + 32.0 + 24.0, Y0 + linha as f32 * ALTURA_LINHA + 14.0);
        clicar_com(&mut h, no_numero(1), egui::PointerButton::Primary, Modifiers::NONE);
        h.run_steps(60);
        clicar_com(&mut h, no_numero(2), egui::PointerButton::Primary, Modifiers::CTRL);
        h.run_steps(60);
        clicar_com(&mut h, no_numero(0), egui::PointerButton::Primary, Modifiers::SHIFT);
        h.run_steps(60);
        assert_eq!(
            h.state().numeros,
            [
                (1, Toques { ctrl: false, shift: false }),
                (2, Toques { ctrl: true, shift: false }),
                (0, Toques { ctrl: false, shift: true }),
            ]
        );
        // o número não é uma célula ativável
        assert_eq!(h.state().nucleo.selecao(), None);
        // o clique numa célula de dados é o de sempre, e não é clique no número
        h.state_mut().numeros.clear();
        clicar_com(&mut h, egui::pos2(X0 + 80.0 + 50.0, Y0 + 14.0), egui::PointerButton::Primary, Modifiers::NONE);
        h.run_steps(3);
        assert!(h.state().numeros.is_empty());
        assert_eq!(h.state().nucleo.selecao(), Some((0, 2)));
    }

    #[test]
    fn ctrl_page_up_e_down_ficam_livres() {
        let mut h = harness_da_grade(NucleoGrade::new("ctrl-pg", 100, vec![100.0; 3]), 400.0);
        clicar_com(&mut h, egui::pos2(X0 + 50.0, Y0 + 14.0), egui::PointerButton::Primary, Modifiers::NONE);
        h.run_steps(3);
        assert_eq!(h.state().nucleo.selecao(), Some((0, 0)));
        // PageDown anda uma tela (13 linhas com 400px)...
        h.key_press(Key::PageDown);
        h.run_steps(2);
        assert_eq!(h.state().nucleo.selecao(), Some((13, 0)));
        // ...mas Ctrl+PageDown e Ctrl+PageUp não são da grade: a planilha usa para trocar de folha
        h.key_press_modifiers(Modifiers::CTRL, Key::PageDown);
        h.run_steps(2);
        assert_eq!(h.state().nucleo.selecao(), Some((13, 0)), "Ctrl+PageDown fica livre");
        h.key_press_modifiers(Modifiers::CTRL, Key::PageUp);
        h.run_steps(2);
        assert_eq!(h.state().nucleo.selecao(), Some((13, 0)), "Ctrl+PageUp fica livre");
        h.key_press(Key::PageUp);
        h.run_steps(2);
        assert_eq!(h.state().nucleo.selecao(), Some((0, 0)));
    }

    #[test]
    fn um_milhao_de_linhas_com_congelado_nao_aloca_por_quadro() {
        let nucleo = NucleoGrade::new("milhao", 1_000_000, vec![100.0; 6]).com_congelado(2, 1).com_mesclas(
            std::sync::Arc::new(IndiceDeMesclas::nova([Mescla::new(0, 0, 0, 5), Mescla::new(500_000, 1, 500_001, 2)])),
        );
        let mut h = harness_da_grade(nucleo, 400.0);
        // o que se entrega por quadro é só o que cabe na tela (uns 15 linhas e as 2 congeladas), não cresce com as linhas
        let limite = (2 + (400.0_f32 / ALTURA_LINHA).ceil() as usize + 3) * 6;
        assert!(h.state().registro.celulas.len() <= limite, "{} células", h.state().registro.celulas.len());
        for destino in [(999_999, 3), (500_000, 1), (3, 4), (999_999, 0)] {
            h.state_mut().nucleo.selecionar(Some(destino));
            h.run_steps(6);
            let n = h.state().registro.celulas.len();
            assert!(n <= limite, "{n} células depois de ir a {destino:?}");
            assert!(celula(&h, destino.0, destino.1).is_some() || h.state().registro.mescladas.iter().any(|m| m.ativa));
            assert_eq!(h.state().nucleo.congelado_efetivo().linhas, 2);
        }
        assert!(h.state().maior_quadro <= limite, "o maior quadro entregou {} células", h.state().maior_quadro);
        // as duas congeladas seguem no topo mesmo no fim de um milhão de linhas
        h.state_mut().nucleo.selecionar(Some((999_999, 3)));
        h.run_steps(6);
        assert_eq!(celula(&h, 1, 0).expect("congelada").celula.top(), Y0 + ALTURA_LINHA);
    }
}

/// A faixa das planilhas (etapa 6, prancheta 17, cenas a e b).
mod faixa_das_planilhas {
    use super::*;
    use interface::pecas::folhas::{
        ALTURA_FAIXA_FOLHAS, ALTURA_PILULA, CARACTERES_MAXIMOS, EstadoDaFolha, FaixaDeFolhas, Folha, ROLAR_DIREITA,
        ROTULO_FAIXA, RespostaFolhas,
    };

    const QUATRO: [(&str, EstadoDaFolha); 4] = [
        ("Resumo", EstadoDaFolha::Visivel),
        ("Vendas", EstadoDaFolha::Visivel),
        ("Regiões", EstadoDaFolha::Visivel),
        ("Custos", EstadoDaFolha::Oculta),
    ];

    struct Faixa {
        nomes: Vec<(String, EstadoDaFolha)>,
        ativa: usize,
        largura: f32,
        escolhidas: Vec<usize>,
        resposta: Option<RespostaFolhas>,
    }

    fn faixa(nomes: &[(&str, EstadoDaFolha)], largura: f32) -> Harness<'static, Faixa> {
        let estado = Faixa {
            nomes: nomes.iter().map(|(n, e)| (n.to_string(), *e)).collect(),
            ativa: 1,
            largura,
            escolhidas: Vec::new(),
            resposta: None,
        };
        harness(estado, |ui, s| {
            let folhas: Vec<Folha<'_>> = s.nomes.iter().map(|(n, e)| Folha::new(n).estado(*e)).collect();
            let rect = egui::Rect::from_min_size(egui::pos2(20.0, 300.0), egui::vec2(s.largura, ALTURA_FAIXA_FOLHAS));
            let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            let r = FaixaDeFolhas::new("teste", &folhas).ativa(Some(s.ativa)).mostrar(&mut filho);
            if let Some(i) = r.escolhida {
                s.escolhidas.push(i);
                s.ativa = i;
            }
            s.resposta = Some(r);
        })
    }

    fn muitas() -> Vec<(&'static str, EstadoDaFolha)> {
        let nomes = [
            "Resumo",
            "Vendas",
            "Regiões",
            "Metas",
            "Custos",
            "Comissões",
            "Estoque",
            "Devoluções",
            "Parâmetros",
            "Câmbio",
            "Calendário fiscal",
            "Notas e fontes",
        ];
        nomes
            .iter()
            .map(|n| (*n, if *n == "Custos" { EstadoDaFolha::Oculta } else { EstadoDaFolha::Visivel }))
            .collect()
    }

    /// O texto pintado `texto`: a cor e a fonte da primeira seção.
    fn texto_pintado(h: &Harness<'_, Faixa>, texto: &str) -> Option<(egui::Color32, egui::FontId)> {
        h.output().shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == texto => {
                Some((t.fallback_color, t.galley.job.sections[0].format.font_id.clone()))
            }
            _ => None,
        })
    }

    fn aba<'h>(h: &'h Harness<'_, Faixa>, rotulo: &'h str) -> egui_kittest::Node<'h> {
        h.get(by().role(Role::Tab).label(rotulo))
    }

    #[test]
    fn faixa_de_folhas_tem_32_de_altura_e_pilulas_de_24() {
        let h = faixa(&QUATRO, 700.0);
        assert_eq!(h.state().resposta.expect("a faixa").rect.height(), ALTURA_FAIXA_FOLHAS);
        assert_eq!(ALTURA_FAIXA_FOLHAS, 32.0);
        let vendas = aba(&h, "Vendas, planilha 2 de 4").rect();
        assert_eq!(vendas.height(), ALTURA_PILULA);
        assert_eq!(ALTURA_PILULA, 24.0);
        // centradas na faixa, depois dos dois botões redondos de 26 e do `gap:4px`
        assert_eq!(vendas.center().y, 300.0 + ALTURA_FAIXA_FOLHAS / 2.0);
        let resumo = aba(&h, "Resumo, planilha 1 de 4").rect();
        assert_eq!(resumo.left(), 20.0 + 8.0 + 26.0 + 4.0 + 26.0 + 4.0);
        assert_eq!(vendas.left(), resumo.right() + 4.0);
    }

    #[test]
    fn a_ativa_tem_o_volume_e_a_inativa_e_t2() {
        let h = faixa(&QUATRO, 700.0);
        let e = &tema::paleta_ctx(&h.ctx).estudio;
        let vendas = aba(&h, "Vendas, planilha 2 de 4").rect();
        let resumo = aba(&h, "Resumo, planilha 1 de 4").rect();
        // o volume tem a borda `--btb` em volta da pílula; a inativa não tem contorno nenhum
        assert!(!cantos_do_contorno(&h, vendas).is_empty(), "a ativa no volume");
        assert!(cantos_do_contorno(&h, resumo).is_empty(), "a inativa é transparente");
        let (cor_ativa, fonte_ativa) = texto_pintado(&h, "Vendas").expect("Vendas pintado");
        let (cor_inativa, fonte_inativa) = texto_pintado(&h, "Resumo").expect("Resumo pintado");
        assert_eq!((cor_ativa, cor_inativa), (e.t, e.t2));
        assert_eq!(fonte_ativa, fontes::fonte(12.0, fontes::Peso::Medio), "a ativa em 600");
        assert_eq!(fonte_inativa, fontes::fonte(12.0, fontes::Peso::Normal));
    }

    #[test]
    fn nome_longo_corta_com_reticencia_e_a_dica_diz_tudo() {
        let longo = "Lançamentos contábeis de outubro de 2026";
        let mut h = faixa(&[("Resumo", EstadoDaFolha::Visivel), (longo, EstadoDaFolha::Visivel)], 900.0);
        let curto: String = longo.chars().take(CARACTERES_MAXIMOS - 1).chain(['…']).collect();
        assert!(texto_pintado(&h, &curto).is_some(), "a pílula mostra {curto}");
        assert!(texto_pintado(&h, longo).is_none());
        // o leitor de tela e a dica dizem o nome inteiro
        let rotulo = format!("{longo}, planilha 2 de 2");
        aba(&h, &rotulo).hover();
        for _ in 0..60 {
            h.step();
            if h.query_by_label(longo).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(h.query_by_label(longo).is_some(), "a dica com o nome inteiro");
    }

    #[test]
    fn setas_rolam_a_faixa_e_a_roda_tambem() {
        let mut h = faixa(&muitas(), 420.0);
        h.state_mut().ativa = 0;
        h.run_steps(3);
        let x_resumo = || aba(&h, "Resumo, planilha 1 de 12").rect().left();
        let antes = x_resumo();
        // no começo, a seta da esquerda está desligada
        assert!(h.get_by_label("Rolar as planilhas para a esquerda").accesskit_node().is_disabled());
        h.get_by_label(ROLAR_DIREITA).click();
        h.run_steps(3);
        let x_resumo = || aba(&h, "Resumo, planilha 1 de 12").rect().left();
        let depois = x_resumo();
        assert!(depois < antes - 10.0, "a seta rolou: {antes} → {depois}");
        assert!(!h.get_by_label("Rolar as planilhas para a esquerda").accesskit_node().is_disabled());
        // a roda, com o ponteiro na faixa, rola mais
        h.hover_at(egui::pos2(300.0, 300.0 + ALTURA_FAIXA_FOLHAS / 2.0));
        h.run_steps(1);
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(20);
        let rodou = aba(&h, "Resumo, planilha 1 de 12").rect().left();
        assert!(rodou < depois - 10.0, "a roda rolou: {depois} → {rodou}");
    }

    #[test]
    fn lista_aparece_so_quando_nao_cabe() {
        let h = faixa(&QUATRO, 700.0);
        assert!(!h.state().resposta.expect("a faixa").lista);
        assert!(h.query_by_role_and_label(Role::Button, ROTULO_FAIXA).is_none(), "sem a lista quando tudo cabe");
        let mut h = faixa(&muitas(), 420.0);
        assert!(h.state().resposta.expect("a faixa").lista);
        h.get_by_role_and_label(Role::Button, ROTULO_FAIXA).click();
        h.run_steps(3);
        // a lista tem todas, com a ativa marcada; escolher uma fecha e troca
        assert_eq!(h.get(by().role(Role::CheckBox).label("Vendas")).accesskit_node().toggled(), Some(Toggled::True));
        h.get(by().role(Role::CheckBox).label("Câmbio")).click();
        h.run_steps(3);
        assert_eq!(h.state().escolhidas, [9]);
        assert!(h.query_by_role_and_label(Role::CheckBox, "Câmbio").is_none(), "a lista fechou");
        // a escolhida entrou à vista
        let notas = aba(&h, "Câmbio, planilha 10 de 12").rect();
        assert!(notas.right() <= 20.0 + 420.0 - 8.0 - 26.0 - 4.0 + 0.5, "{notas:?}");
    }

    #[test]
    fn papel_tab_com_selected_e_rotulo_de_leitor_de_tela() {
        let h = faixa(&QUATRO, 700.0);
        assert_eq!(h.get(by().role(Role::TabList)).accesskit_node().label().as_deref(), Some(ROTULO_FAIXA));
        assert_eq!(aba(&h, "Vendas, planilha 2 de 4").accesskit_node().is_selected(), Some(true));
        assert_eq!(aba(&h, "Resumo, planilha 1 de 4").accesskit_node().is_selected(), Some(false));
        assert!(h.query_by_role_and_label(Role::Tab, "Custos, planilha 4 de 4, oculta").is_some());
        assert_eq!(h.query_all(by().role(Role::Tab)).count(), 4);
    }

    #[test]
    fn setas_e_enter_ativam_a_folha() {
        let mut h = faixa(&QUATRO, 700.0);
        aba(&h, "Vendas, planilha 2 de 4").focus();
        h.run_steps(2);
        h.key_press(Key::ArrowRight);
        h.run_steps(2);
        assert!(aba(&h, "Regiões, planilha 3 de 4").is_focused(), "→ leva o foco à seguinte");
        assert!(h.state().escolhidas.is_empty(), "andar não ativa");
        h.key_press(Key::Enter);
        h.run_steps(2);
        assert_eq!(h.state().escolhidas, [2]);
        h.key_press(Key::ArrowLeft);
        h.run_steps(2);
        h.key_press(Key::ArrowLeft);
        h.run_steps(2);
        assert!(aba(&h, "Resumo, planilha 1 de 4").is_focused());
        h.key_press(Key::Space);
        h.run_steps(2);
        assert_eq!(h.state().escolhidas, [2, 0]);
        // ← na primeira não sai dela
        h.key_press(Key::ArrowLeft);
        h.run_steps(2);
        assert!(aba(&h, "Resumo, planilha 1 de 4").is_focused());
    }

    #[test]
    fn oculta_em_t3_e_italico() {
        let h = faixa(&QUATRO, 700.0);
        let e = &tema::paleta_ctx(&h.ctx).estudio;
        let (cor, fonte) = texto_pintado(&h, "Custos").expect("Custos pintada");
        assert_eq!(cor, e.t3);
        assert_eq!(fonte, fontes::fonte(12.0, fontes::Peso::Italico));
    }
}

/// O menu da célula (0.2.3, `grade::menu_da_celula`) sobre a grade só leitura: o clique direito dentro e fora do
/// intervalo, o Shift+F10, o Esc, os itens desligados com o motivo, o submenu "Copiar como" e o Ctrl+Shift+C.
mod menu_da_celula {
    use super::*;
    use interface::grade::menu_da_celula;
    use interface::grade::{ALTURA_CABECALHO, ALTURA_LINHA, AcaoCelula, LARGURA_MENU_DA_CELULA, OfertasCelula};
    use interface::grade::{OfertaCelula, menu_da_celula_aberto};
    use tabular::colunas::{FormatoCopia, OpcoesCopia};

    struct Estado {
        grade: GradeSomenteLeitura,
        acao: Option<AcaoCelula>,
        pedidos: usize,
        copiado: Option<String>,
    }

    fn montar() -> Harness<'static, Estado> {
        let EstadoGrade { grade, .. } = grade_de_teste(50);
        let estado = Estado { grade, acao: None, pedidos: 0, copiado: None };
        harness(estado, |ui, s| {
            let r = s.grade.mostrar(ui, 400.0);
            if r.menu_celula {
                s.pedidos += 1;
            }
            if r.copiado.is_some() {
                s.copiado = r.copiado;
            }
            let ofertas = s
                .grade
                .nucleo()
                .intervalo()
                .map(OfertasCelula::de_leitura)
                .unwrap_or_default()
                .desligar(AcaoCelula::Colar, "A Consulta é só leitura.")
                .ligar(AcaoCelula::FiltrarPorValor)
                .ligar(AcaoCelula::ExcluirLinhas);
            if let Some(acao) = menu_da_celula(ui, s.grade.nucleo_mut(), &ofertas) {
                s.acao = Some(acao);
            }
        })
    }

    fn centro(h: &Harness<'_, Estado>, l: usize, c: usize) -> egui::Pos2 {
        let larguras = h.state().grade.larguras().map(<[f32]>::to_vec).unwrap_or_default();
        let x = 8.0 + larguras[..c].iter().sum::<f32>() + larguras[c] / 2.0;
        let y = 8.0 + ALTURA_CABECALHO + ALTURA_LINHA * (l as f32 + 0.5);
        egui::pos2(x, y)
    }

    fn clique_direito(h: &mut Harness<'_, Estado>, pos: egui::Pos2) {
        h.hover_at(pos);
        h.run_steps(1);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.run_steps(3);
    }

    fn aberto(h: &Harness<'_, Estado>) -> bool {
        menu_da_celula_aberto(&h.ctx, h.state().grade.nucleo())
    }

    fn foco_na_grade(h: &Harness<'_, Estado>) -> bool {
        let id = h.state().grade.id_corpo();
        h.ctx.memory(|m| m.has_focus(id))
    }

    #[test]
    fn clique_direito_dentro_do_intervalo_age_nele_e_fora_troca_a_ativa() {
        let mut h = montar();
        let inicio = centro(&h, 1, 0);
        clicar_em(&h, inicio);
        h.run_steps(2);
        h.event(tecla(Key::ArrowDown, Modifiers::SHIFT));
        h.event(tecla(Key::ArrowDown, Modifiers::SHIFT));
        h.event(tecla(Key::ArrowRight, Modifiers::SHIFT));
        h.run_steps(2);
        let antes = h.state().grade.nucleo().intervalo().expect("o intervalo");
        assert_eq!((antes.linha0, antes.linha1, antes.coluna0, antes.coluna1), (1, 3, 0, 1));

        // dentro do intervalo: o menu abre e o intervalo fica
        let dentro = centro(&h, 2, 1);
        clique_direito(&mut h, dentro);
        assert!(aberto(&h), "o clique direito abre o menu");
        assert_eq!(h.state().pedidos, 1);
        assert_eq!(h.state().grade.nucleo().intervalo(), Some(antes), "dentro do intervalo, ele fica");
        assert_eq!(h.state().grade.selecao(), Some((1, 0)), "e a ativa também");
        assert_eq!(h.get_by_role(Role::Menu).accesskit_node().label().as_deref(), Some("Menu da seleção"));
        h.get_by_label("Copiar").click();
        h.run_steps(3);
        assert_eq!(h.state().acao, Some(AcaoCelula::Copiar));
        assert!(!aberto(&h), "escolher fecha");
        assert!(foco_na_grade(&h), "e devolve o foco à grade");

        // fora do intervalo: a célula vira a ativa e o intervalo some
        let fora = centro(&h, 6, 2);
        clique_direito(&mut h, fora);
        assert!(aberto(&h));
        assert_eq!(h.state().grade.selecao(), Some((6, 2)));
        assert!(!h.state().grade.nucleo().intervalo().expect("a ativa").varias());
        assert_eq!(h.get_by_role(Role::Menu).accesskit_node().label().as_deref(), Some("Menu da célula"));

        // o Esc fecha e devolve o foco à grade, que continua com a mesma célula
        h.key_press(Key::Escape);
        h.run_steps(3);
        assert!(!aberto(&h));
        assert!(foco_na_grade(&h), "o Esc devolve o foco à grade");
        assert_eq!(h.state().grade.selecao(), Some((6, 2)));
    }

    #[test]
    fn shift_f10_abre_na_ativa_com_o_foco_no_primeiro_item() {
        let mut h = montar();
        let pos = centro(&h, 4, 1);
        clicar_em(&h, pos);
        h.run_steps(2);
        h.event(tecla(Key::F10, Modifiers::SHIFT));
        h.run_steps(4);
        assert!(aberto(&h), "o Shift+F10 abre o menu");
        assert!(h.get_by_label("Copiar").accesskit_node().is_focused(), "o foco vai ao primeiro item");
        // embaixo da célula ativa
        let menu = h.get_by_role(Role::Menu).rect();
        assert!((menu.top() - (pos.y + ALTURA_LINHA / 2.0)).abs() < 8.0, "o menu nasce embaixo da célula: {menu:?}");
        // as setas andam pelos itens, não pela grade
        h.key_press(Key::ArrowDown);
        h.run_steps(2);
        assert!(h.get_by_label("Copiar com cabeçalho").accesskit_node().is_focused());
        assert_eq!(h.state().grade.selecao(), Some((4, 1)), "a grade não andou");
        h.key_press(Key::Enter);
        h.run_steps(3);
        assert_eq!(h.state().acao, Some(AcaoCelula::CopiarComCabecalho));
        assert!(!aberto(&h));
        assert!(foco_na_grade(&h), "ativar pelo teclado devolve o foco à grade");
        // sem foco na grade, o Shift+F10 não abre
        let id = h.state().grade.id_corpo();
        h.ctx.memory_mut(|m| m.surrender_focus(id));
        h.run_steps(1);
        h.event(tecla(Key::F10, Modifiers::SHIFT));
        h.run_steps(3);
        assert!(!aberto(&h));
    }

    #[test]
    fn itens_desligados_ocultos_e_atalhos() {
        let mut h = montar();
        let pos = centro(&h, 0, 1);
        clique_direito(&mut h, pos);
        let largura = h.get_by_role(Role::Menu).rect().width();
        assert!((largura - LARGURA_MENU_DA_CELULA).abs() < 1.0, "280 de largura: {largura}");
        let colar = h.get_by_label("Colar");
        assert!(colar.accesskit_node().is_disabled(), "o desligado aparece desligado");
        assert_eq!(colar.accesskit_node().description().as_deref(), Some("A Consulta é só leitura."));
        assert_eq!(colar.accesskit_node().data().keyboard_shortcut(), Some("Ctrl+V"));
        assert_eq!(
            h.get_by_label("Copiar com cabeçalho").accesskit_node().data().keyboard_shortcut(),
            Some("Ctrl+Shift+C")
        );
        assert!(h.query_by_label("Limpar").is_none(), "o que a tela não oferece não aparece");
        assert!(h.query_by_label("Definir como NULL").is_none());
        assert!(h.query_by_label("Abrir a ficha da linha").is_none());
        // clicar no desligado não faz nada nem fecha
        h.get_by_label("Colar").click();
        h.run_steps(2);
        assert_eq!(h.state().acao, None);
        assert!(aberto(&h));
        h.get_by_label("Excluir as linhas").click();
        h.run_steps(2);
        assert_eq!(h.state().acao, Some(AcaoCelula::ExcluirLinhas));
        assert_eq!(*OfertasCelula::default().get(AcaoCelula::Limpar), OfertaCelula::Oculta);
    }

    #[test]
    fn copiar_como_pelo_submenu() {
        let mut h = montar();
        let inicio = centro(&h, 1, 1);
        clicar_em(&h, inicio);
        h.run_steps(2);
        h.event(tecla(Key::ArrowDown, Modifiers::SHIFT));
        h.run_steps(2);
        let pos = centro(&h, 1, 1);
        clique_direito(&mut h, pos);
        h.get_by_label("Copiar como").hover();
        h.run_steps(3);
        assert!(!h.get_by_label("CSV").accesskit_node().is_disabled());
        assert!(!h.get_by_label("Lista para IN (…)").accesskit_node().is_disabled(), "uma coluna só");
        h.get_by_label("Lista para IN (…)").click();
        h.run_steps(3);
        assert_eq!(h.state().acao, Some(AcaoCelula::CopiarComo(FormatoCopia::ListaIn)));
        assert!(!aberto(&h), "o item do submenu fecha o menu inteiro");
        let opcoes = OpcoesCopia { separador: ',', tabela: "consulta" };
        let grade = &h.state().grade;
        assert_eq!(grade.texto_copiado_como(FormatoCopia::ListaIn, &opcoes).as_deref(), Some("('Nome 1', 'Nome 2')"));
        assert_eq!(
            grade.texto_copiado_como(FormatoCopia::Insert, &opcoes).as_deref(),
            Some("INSERT INTO \"consulta\" (\"nome\") VALUES\n  ('Nome 1'),\n  ('Nome 2');\n")
        );

        // com duas colunas, a lista para IN fica desligada com o motivo
        h.event(tecla(Key::ArrowRight, Modifiers::SHIFT));
        h.run_steps(2);
        clique_direito(&mut h, pos);
        h.get_by_label("Copiar como").hover();
        h.run_steps(3);
        let lista = h.get_by_label("Lista para IN (…)");
        assert!(lista.accesskit_node().is_disabled());
        assert_eq!(lista.accesskit_node().description().as_deref(), Some("Selecione uma coluna só."));
        assert_eq!(
            h.state().grade.texto_copiado_como(FormatoCopia::Json, &opcoes).as_deref(),
            Some("[\n  {\"nome\": \"Nome 1\", \"valor\": 1.5},\n  {\"nome\": \"Nome 2\", \"valor\": 3}\n]\n")
        );
    }

    #[test]
    fn ctrl_shift_c_copia_com_o_cabecalho() {
        let mut h = montar();
        let pos = centro(&h, 1, 1);
        clicar_em(&h, pos);
        h.run_steps(2);
        h.event(tecla(Key::ArrowRight, Modifiers::SHIFT));
        h.run_steps(1);
        // o egui-winit entrega o Ctrl+Shift+C como `Event::Copy` com os modificadores apertados
        h.event_modifiers(egui::Event::Copy, Modifiers::COMMAND | Modifiers::SHIFT);
        h.run_steps(2);
        assert_eq!(h.state().copiado.as_deref(), Some("nome\tvalor\r\nNome 1\t1.5\r\n"));
        // e o `Event::Key` também vale (outros backends, o kittest)
        h.state_mut().copiado = None;
        h.key_press(Key::ArrowDown);
        h.run_steps(1);
        h.event(tecla(Key::C, Modifiers::COMMAND | Modifiers::SHIFT));
        h.run_steps(2);
        assert_eq!(h.state().copiado.as_deref(), Some("nome\r\nNome 2\r\n"));
        // o Ctrl+C sem o Shift continua o de sempre
        h.event(egui::Event::Copy);
        h.run_steps(2);
        assert_eq!(h.state().copiado.as_deref(), Some("Nome 2"));
    }
}
