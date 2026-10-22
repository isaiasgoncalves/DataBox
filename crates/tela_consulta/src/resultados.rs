//! Os resultados da Consulta SQL (no formato da prancheta 3): montados uma vez
//! por resposta, com os textos e as grades prontos, e desenhados a cada quadro sem refazer nada. Cada comando
//! executado vira um item do seletor ("Comando 1 · 8 linhas", [`Card::parte`]) e mostra, quando escolhido, a grade,
//! o aviso da alteração ou o do erro ([`desenhar`]); o erro também vira a faixa de erro acima do seletor
//! ([`FaixaErro`]).

use banco::{MSG_INTERROMPIDA, ResultadoComando, ResultadoConsulta};
use comum::Celula;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, Id, Rect, ScrollArea, Sense, Ui, UiBuilder, vec2};
use estado::{OrigemJanelaExportar, PedidoJanelaExportar};
use interface::fontes;
use interface::formato::{formatar_ms, formatar_numero, plural};
use interface::grade::{
    AcaoCelula, ColunaGrade, GradeSomenteLeitura, Intervalo, OfertasCelula, OpcoesCopia, ResumoSelecao,
    frase_do_intervalo_copiado, menu_da_celula, pastilha_pelos_valores,
};
use interface::pecas::avisos::{Aviso, TomAviso, vazio};
use interface::pecas::documento::RECUO_PAGINA;
use interface::pecas::texto::{
    Paragrafo, Trecho, formato, mostrar_paragrafo_selecionavel, paragrafo_de_job, texto_rico,
};
use interface::pecas::toast::toast_concluido;
use interface::tema::paleta;

use crate::Chave;
use crate::texto::nome_para_exportar;

/// Um roteiro com milhares de INSERT viraria milhares de itens; depois deste número, só a contagem.
pub const MAX_CARDS: usize = 100;

pub const MSG_SEM_COMANDO: &str = "Não havia comando para executar: o texto só tem comentários ou ponto e vírgula.";
pub const MSG_SEM_LINHAS: &str = "A consulta não devolveu linhas.";
pub const MSG_PAROU: &str = "A execução parou neste comando; os anteriores já foram executados.";
const MSG_ERRO_PADRAO: &str = "O comando não pôde ser executado.";
/// A tabela do "Copiar como › INSERT" quando o comando não cita uma.
const TABELA_DO_INSERT: &str = "resultado";

/// Um resultado: um comando executado (um item do seletor) ou um aviso só.
pub(crate) enum Card {
    /// Um comando que devolveu linhas: a grade só leitura (ou o vazio) e o pedido da janela Exportar.
    Linhas {
        numero: usize,
        /// "8 linhas", "mais de 1.000 linhas" (a nota do seletor).
        contagem: String,
        /// "8 linhas · 1,6 ms" (o rodapé).
        hint: String,
        /// None com zero linhas.
        grade: Option<Box<GradeSomenteLeitura>>,
        pedido: PedidoJanelaExportar,
        /// A tabela do "Copiar como › INSERT": a do primeiro FROM do comando, ou "resultado".
        tabela: String,
        /// As contas do rodapé da seleção, guardadas para o intervalo em que foram feitas.
        resumo: Option<Box<(Intervalo, ResumoSelecao)>>,
    },
    /// "12 linhas alteradas · 4 ms" (ou "Comando executado") e o comando.
    Alteracao { numero: usize, texto: String, tempo: String, comando: BlocoComando },
    /// A frase em português, o original do SQLite no "i" (só quando a frase não o cita) e o comando.
    Erro { numero: usize, mensagem: String, detalhe: Option<String>, comando: BlocoComando, parou: bool },
    /// "Mais N comandos executados, que não aparecem aqui." (o item depois dos [`MAX_CARDS`] primeiros).
    Mais { resto: usize, texto: String },
    /// Um aviso só (o fim da transação, o roteiro sem comandos, a falha), no tom dele: o concluído, a informação
    /// ou o erro. Fica sem o seletor.
    Aviso { forte: Option<String>, texto: String, tom: TomAviso },
}

impl Card {
    /// Um aviso de texto simples.
    pub(crate) fn aviso(texto: impl Into<String>, tom: TomAviso) -> Card {
        Card::Aviso { forte: None, texto: texto.into(), tom }
    }

    /// O tipo do resultado para os testes ("linhas", "alteracao", "erro", "mais", "aviso").
    pub(crate) fn tipo(&self) -> &'static str {
        match self {
            Card::Linhas { .. } => "linhas",
            Card::Alteracao { .. } => "alteracao",
            Card::Erro { .. } => "erro",
            Card::Mais { .. } => "mais",
            Card::Aviso { .. } => "aviso",
        }
    }

    /// O número do comando ("Comando 3"), nos que são comandos.
    pub(crate) fn numero(&self) -> Option<usize> {
        match self {
            Card::Linhas { numero, .. } | Card::Alteracao { numero, .. } | Card::Erro { numero, .. } => Some(*numero),
            Card::Mais { .. } | Card::Aviso { .. } => None,
        }
    }

    /// O texto do item no seletor e a nota dele ("Comando 1", "8 linhas"; "Comando 3", "erro", em `--perigo`).
    pub(crate) fn parte(&self) -> (String, Option<&str>, bool) {
        match self {
            Card::Linhas { numero, contagem, .. } => (format!("Comando {numero}"), Some(contagem), false),
            Card::Alteracao { numero, texto, .. } => {
                let nota = if texto == "Comando executado" { "executado" } else { texto.as_str() };
                (format!("Comando {numero}"), Some(nota), false)
            }
            // a interrupção foi pedida pela pessoa: sem o `--perigo`
            Card::Erro { numero, mensagem, .. } if mensagem == MSG_INTERROMPIDA => {
                (format!("Comando {numero}"), Some("interrompido"), false)
            }
            Card::Erro { numero, .. } => (format!("Comando {numero}"), Some("erro"), true),
            Card::Mais { resto, .. } => (format!("Mais {}", plural(*resto as u64, "comando", "comandos")), None, false),
            Card::Aviso { .. } => (String::new(), None, false),
        }
    }

    /// O que o rodapé diz do item escolhido ("8 linhas · 0,5 ms", "3 linhas alteradas · 0,6 ms").
    pub(crate) fn rodape(&self) -> Option<String> {
        match self {
            Card::Linhas { hint, .. } => Some(hint.clone()),
            Card::Alteracao { texto, tempo, .. } => Some(format!("{texto} · {tempo}")),
            Card::Erro { .. } | Card::Mais { .. } | Card::Aviso { .. } => None,
        }
    }

    /// As contas do rodapé da seleção (A.3): com mais de uma célula selecionada na grade, a contagem e as contas pelo
    /// valor de cada célula; refeitas só quando o intervalo muda.
    pub(crate) fn resumo_da_selecao(&mut self) -> Option<ResumoSelecao> {
        let Card::Linhas { grade: Some(g), resumo, .. } = self else { return None };
        let iv = g.nucleo().intervalo().filter(Intervalo::varias)?;
        if let Some(guardado) = resumo
            && guardado.0 == iv
        {
            return Some(guardado.1);
        }
        let mut r = ResumoSelecao::default();
        for l in iv.linhas() {
            for c in iv.colunas() {
                if let Some(celula) = g.linhas().get(l).and_then(|linha| linha.get(c)) {
                    r.somar_celula(celula, true);
                }
            }
        }
        *resumo = Some(Box::new((iv, r)));
        Some(r)
    }

    /// O pedido da janela Exportar (só num resultado com linhas).
    pub(crate) fn pedido(&self) -> Option<&PedidoJanelaExportar> {
        match self {
            Card::Linhas { grade: Some(_), pedido, .. } => Some(pedido),
            _ => None,
        }
    }
}

/// A faixa de erro acima do seletor (item 23): "Comando 3: {mensagem}", o original do SQLite no "i" e, depois do
/// primeiro comando, a frase de que a execução parou ali.
pub(crate) struct FaixaErro<'a> {
    pub numero: Option<usize>,
    pub mensagem: &'a str,
    pub detalhe: Option<&'a str>,
    pub parou: bool,
    /// O comando foi interrompido pela pessoa ("A consulta foi interrompida."): a faixa sai no tom de atenção, não
    /// no de erro.
    pub interrompido: bool,
}

/// A faixa de erro dos resultados: a do comando com erro (o último) ou a da falha da execução (um aviso de erro
/// sozinho).
pub(crate) fn faixa_erro(cards: &[Card]) -> Option<FaixaErro<'_>> {
    cards.iter().rev().find_map(|c| match c {
        Card::Erro { numero, mensagem, detalhe, parou, .. } => Some(FaixaErro {
            numero: Some(*numero),
            mensagem,
            detalhe: detalhe.as_deref(),
            parou: *parou,
            interrompido: mensagem == MSG_INTERROMPIDA,
        }),
        Card::Aviso { forte: None, texto, tom: TomAviso::Erro } if cards.len() == 1 => {
            Some(FaixaErro { numero: None, mensagem: texto, detalhe: None, parou: false, interrompido: false })
        }
        _ => None,
    })
}

/// O item que a tela mostra ao chegar a resposta: o primeiro com linhas (o SELECT de um roteiro); sem nenhum, o
/// último (o erro, a última alteração).
pub(crate) fn escolhido_de_partida(cards: &[Card]) -> usize {
    cards.iter().position(|c| matches!(c, Card::Linhas { .. })).unwrap_or(cards.len().saturating_sub(1))
}

/// Monta os resultados de uma resposta (`mensagem_final`: o "Transação gravada." do COMMIT e do ROLLBACK, que
/// substitui os resultados quando nada deu erro). `execucao` deixa o estado das grades de uma execução longe
/// do da seguinte, e `sal` (o da tela, [`crate::TelaConsulta::com_id`]) o de uma Consulta longe do da outra.
pub(crate) fn montar(
    r: ResultadoConsulta,
    mensagem_final: Option<(&str, &str)>,
    execucao: u64,
    sal: Option<u64>,
) -> Vec<Card> {
    let grade = Chave { nome: "consulta::grade", sal };
    let teve_erro = r.resultados.iter().any(|x| matches!(x, ResultadoComando::Erro { .. }));
    if let Some((forte, texto)) = mensagem_final
        && !teve_erro
    {
        return vec![Card::Aviso {
            forte: Some(forte.to_string()),
            texto: texto.to_string(),
            tom: TomAviso::Concluido,
        }];
    }
    if r.resultados.is_empty() {
        return vec![Card::aviso(MSG_SEM_COMANDO, TomAviso::Informacao)];
    }
    let total = r.resultados.len();
    let mut resultados = r.resultados.into_iter();
    let mut cards = Vec::with_capacity(total.min(MAX_CARDS) + 2);
    for (i, res) in resultados.by_ref().take(MAX_CARDS).enumerate() {
        cards.push(card(res, i, (grade, execucao)));
    }
    if total > MAX_CARDS {
        let mut erro = None;
        for res in resultados {
            if erro.is_none() && matches!(res, ResultadoComando::Erro { .. }) {
                erro = Some(res);
            }
        }
        let resto = total - MAX_CARDS;
        let texto = format!(
            "Mais {}, que não {} aqui.",
            plural(resto as u64, "comando executado", "comandos executados"),
            if resto == 1 { "aparece" } else { "aparecem" }
        );
        cards.push(Card::Mais { resto, texto });
        if let Some(e) = erro {
            // o roteiro para no primeiro erro: ele é o último comando
            cards.push(card(e, total - 1, (grade, execucao)));
        }
    }
    cards
}

/// `i`: a posição do comando no roteiro (do 0); `grade`: a chave das grades desta tela e o número da execução.
fn card(res: ResultadoComando, i: usize, grade: (Chave, u64)) -> Card {
    let numero = i + 1;
    match res {
        ResultadoComando::Linhas { comando, colunas, linhas, truncado, tempo_ms } => {
            card_linhas(comando, colunas, linhas, truncado, tempo_ms, numero, grade)
        }
        ResultadoComando::Alteracao { comando, linhas_afetadas, tempo_ms } => {
            let texto = match linhas_afetadas {
                None => "Comando executado".to_string(),
                Some(n) => {
                    format!("{} {}", plural(n, "linha", "linhas"), if n == 1 { "alterada" } else { "alteradas" })
                }
            };
            Card::Alteracao { numero, texto, tempo: formatar_ms(tempo_ms), comando: BlocoComando::novo(&comando) }
        }
        ResultadoComando::Erro { comando, mensagem, detalhe } => {
            // a tela fica com a frase em português; o texto do SQLite (em inglês) vai para o "i", e só quando a
            // frase não o cita (a interrompida não tem o que explicar)
            let detalhe = detalhe.filter(|d| !mensagem.contains(d.trim()) && mensagem != MSG_INTERROMPIDA);
            let mensagem = if mensagem.is_empty() { MSG_ERRO_PADRAO.to_string() } else { mensagem };
            Card::Erro { numero, mensagem, detalhe, comando: BlocoComando::novo(&comando), parou: i > 0 }
        }
    }
}

fn card_linhas(
    comando: String,
    colunas: Vec<String>,
    linhas: Vec<Vec<Celula>>,
    truncado: bool,
    tempo_ms: f64,
    numero: usize,
    (grade, execucao): (Chave, u64),
) -> Card {
    let n = linhas.len() as u64;
    let contagem = plural(n, "linha", "linhas");
    let contagem = if truncado { format!("mais de {contagem}") } else { contagem };
    let mut hint = format!("{contagem} · {}", formatar_ms(tempo_ms));
    if truncado {
        hint.push_str(&format!(" · mostrando as primeiras {}", formatar_numero(n)));
    }
    let nome_sugerido = nome_para_exportar(&comando);
    let tabela = nome_sugerido.strip_prefix("consulta_").unwrap_or(TABELA_DO_INSERT).to_string();
    let pedido =
        PedidoJanelaExportar { origem: OrigemJanelaExportar::Consulta { sql: comando.clone() }, nome_sugerido };
    let grade = (!linhas.is_empty()).then(|| {
        // a pastilha de tipo no cabeçalho: pelo que os valores de cada coluna são (a Consulta não sabe o tipo
        // declarado da coluna de origem); `Aa` quando não dá para saber
        let colunas = colunas
            .into_iter()
            .enumerate()
            .map(|(c, nome)| {
                let pastilha = pastilha_pelos_valores(linhas.iter().filter_map(|l| l.get(c)));
                ColunaGrade::new(nome).com_pastilha(pastilha)
            })
            .collect();
        Box::new(GradeSomenteLeitura::new((grade, execucao, numero), colunas, linhas))
    });
    Card::Linhas { numero, contagem, hint, grade, pedido, tabela, resumo: None }
}

/// Desenha o resultado escolhido no espaço que sobra (`id`: o da tela, da execução e do item): a grade de ponta a
/// ponta, preenchendo a altura; o vazio; ou o aviso (a alteração, o erro, os comandos que não aparecem, o fim da
/// transação) numa caixa centrada, com o recuo da página.
pub(crate) fn desenhar(ui: &mut Ui, card: &mut Card, id: Id) {
    let altura = ui.available_height();
    match card {
        Card::Linhas { grade: Some(g), tabela, .. } => {
            g.mostrar(ui, altura);
            menu_da_grade(ui, g, tabela);
        }
        Card::Linhas { grade: None, .. } => {
            vazio(ui, MSG_SEM_LINHAS, false);
        }
        outro => {
            let area = ui.available_rect_before_wrap().shrink(RECUO_PAGINA);
            let mut filho = ui.new_child(UiBuilder::new().id_salt(id).max_rect(area));
            filho.set_clip_rect(area.expand(RECUO_PAGINA).intersect(ui.clip_rect()));
            ScrollArea::vertical().id_salt(id.with("rolagem")).auto_shrink([false, true]).show(&mut filho, |ui| {
                aviso_do_resultado(ui, outro, id);
            });
        }
    }
}

/// O menu da célula da grade (A.1): só os itens de copiar, que valem no resultado de uma consulta.
fn menu_da_grade(ui: &Ui, g: &mut GradeSomenteLeitura, tabela: &str) {
    let Some(iv) = g.nucleo().intervalo() else { return };
    let ofertas = OfertasCelula::de_leitura(iv);
    let Some(acao) = menu_da_celula(ui, g.nucleo_mut(), &ofertas) else { return };
    let texto = match acao {
        AcaoCelula::Copiar => g.texto_copiado(),
        AcaoCelula::CopiarComCabecalho => g.texto_copiado_com_cabecalho(),
        AcaoCelula::CopiarComo(formato) => g.texto_copiado_como(formato, &OpcoesCopia { separador: ',', tabela }),
        _ => None,
    };
    if let Some(t) = texto {
        ui.ctx().copy_text(t);
        toast_concluido(ui.ctx(), frase_do_intervalo_copiado(iv));
    }
}

/// O aviso de um resultado que não é grade.
fn aviso_do_resultado(ui: &mut Ui, card: &mut Card, id: Id) {
    match card {
        Card::Alteracao { texto, tempo, comando, .. } => {
            Aviso::com_tom(TomAviso::Concluido).mostrar(ui, |ui, cores| {
                let trechos = [Trecho::Forte(texto), Trecho::Normal(" · "), Trecho::Normal(tempo)];
                texto_rico(ui, &trechos, 12.5, cores.texto, cores.forte);
                comando.mostrar(ui, id.with("comando"));
            });
        }
        Card::Erro { mensagem, detalhe, comando, parou, .. } => {
            // a interrupção pedida pela pessoa não é um erro: o tom de atenção
            let tom = if mensagem == MSG_INTERROMPIDA { TomAviso::Atencao } else { TomAviso::Erro };
            Aviso::com_tom(tom).mostrar(ui, |ui, cores| {
                frase_do_erro(ui, mensagem, detalhe.as_deref(), cores.texto, cores.forte);
                comando.mostrar(ui, id.with("comando"));
                if *parou {
                    texto_rico(ui, &[Trecho::Fraco(MSG_PAROU)], 11.5, cores.texto, cores.forte);
                }
            });
        }
        Card::Mais { texto, .. } => {
            Aviso::com_tom(TomAviso::Informacao).mostrar(ui, |ui, cores| {
                texto_rico(ui, &[Trecho::Normal(texto)], 12.5, cores.texto, cores.forte);
            });
        }
        Card::Aviso { forte, texto, tom } => {
            Aviso::com_tom(*tom).mostrar(ui, |ui, cores| match forte {
                Some(f) => {
                    texto_rico(ui, &[Trecho::Forte(f), Trecho::Normal(texto)], 12.5, cores.texto, cores.forte);
                }
                None => {
                    texto_rico(ui, &[Trecho::Normal(texto)], 12.5, cores.texto, cores.forte);
                }
            });
        }
        Card::Linhas { .. } => {}
    }
}

/// A frase do erro em `forte` (o `--perigo`), com o original do SQLite na dica da frase quando há (sem o "i" ao lado:
/// o erro das pranchetas não o tem).
pub(crate) fn frase_do_erro(ui: &mut Ui, mensagem: &str, detalhe: Option<&str>, cor: Color32, forte: Color32) {
    match detalhe {
        Some(d) => {
            let r = texto_rico(ui, &[Trecho::Forte(mensagem)], 12.0, cor, forte);
            r.resposta.on_hover_text(format!("Mensagem original do SQLite: {d}"));
        }
        None => {
            texto_rico(ui, &[Trecho::Forte(mensagem)], 12.0, cor, forte);
        }
    }
}

/// O que decide se o parágrafo pronto de um [`BlocoComando`] ainda vale: a largura, a escala da tela e a fonte
/// monoespaçada (a Hack de reserva até as fontes do sistema chegarem; medida pela largura do "M" e pela altura
/// da linha, que mudam com ela).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ChaveLayout {
    largura: f32,
    pontos_por_pixel: f32,
    glifo: f32,
    linha: f32,
}

/// `code.comando-sql`: bloco monoespaçado de 11.5px / 1.5 com o fundo `--k-trackBg`, raio 7, `padding:6px 10px`,
/// quebrando em qualquer ponto e com rolagem acima de 9,6em; o texto pode ser selecionado e copiado.
///
/// O texto é montado uma vez por largura (o `LayoutJob` copiaria e o cache de galleys do egui passaria o texto
/// inteiro por hash a cada quadro: num roteiro com um INSERT de alguns MB, a cada movimento do mouse), e o bloco
/// fora da tela só reserva a altura guardada.
pub(crate) struct BlocoComando {
    /// O comando sem espaços nas pontas.
    texto: String,
    pronto: Option<(ChaveLayout, Paragrafo)>,
}

impl BlocoComando {
    const ALTURA_LINHA: f32 = 17.25;
    const ALTURA_MAX: f32 = 11.5 * 9.6;

    pub(crate) fn novo(comando: &str) -> BlocoComando {
        BlocoComando { texto: comando.trim().to_string(), pronto: None }
    }

    /// O comando sem espaços nas pontas.
    #[cfg(test)]
    pub(crate) fn texto(&self) -> &str {
        &self.texto
    }

    /// O parágrafo na largura do texto (`largura` menos o `padding`), montado de novo só quando a chave muda. A
    /// cor fica para a hora de pintar (o tema pode mudar sem refazer o texto).
    fn paragrafo(&mut self, ui: &Ui, largura: f32) -> &Paragrafo {
        let fonte = fontes::fonte_mono(11.5);
        let chave = ui.fonts_mut(|f| ChaveLayout {
            largura,
            pontos_por_pixel: ui.pixels_per_point(),
            glifo: f.glyph_width(&fonte, 'M'),
            linha: f.row_height(&fonte),
        });
        if self.pronto.as_ref().is_some_and(|(c, _)| *c != chave) {
            self.pronto = None;
        }
        let texto = &self.texto;
        let (_, g) = self.pronto.get_or_insert_with(|| {
            let mut f = formato(fonte.clone(), Color32::PLACEHOLDER);
            f.line_height = Some(Self::ALTURA_LINHA);
            let mut job = LayoutJob::single_section(texto.clone(), f);
            job.wrap = TextWrapping { max_width: largura.max(1.0), ..TextWrapping::default() };
            (chave, paragrafo_de_job(ui, job, &fonte, Self::ALTURA_LINHA))
        });
        g
    }

    pub(crate) fn mostrar(&mut self, ui: &mut Ui, id: Id) {
        let p = paleta(ui);
        let largura = ui.available_width();
        let g = self.paragrafo(ui, (largura - 20.0).max(1.0));
        let altura_texto = g.tamanho().y.max(Self::ALTURA_LINHA);
        let visivel = altura_texto.min(Self::ALTURA_MAX);
        let (rect, _) = ui.allocate_exact_size(vec2(largura, visivel + 12.0), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return;
        }
        // o `Paragrafo` é um `Arc` do galley: a cópia é barata
        let g = g.clone();
        ui.painter().rect_filled(rect, 7.0, p.track_bg);
        let dentro = Rect::from_min_size(rect.min + vec2(10.0, 6.0), vec2(largura - 20.0, visivel));
        let mut filho = ui.new_child(UiBuilder::new().id_salt(id).max_rect(dentro));
        if altura_texto > Self::ALTURA_MAX {
            ScrollArea::vertical()
                .id_salt(id.with("rolagem"))
                .max_height(Self::ALTURA_MAX)
                .auto_shrink([false, true])
                .show(&mut filho, |ui| {
                    mostrar_paragrafo_selecionavel(ui, &g, &self.texto);
                });
        } else {
            mostrar_paragrafo_selecionavel(&mut filho, &g, &self.texto);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn consulta(resultados: Vec<ResultadoComando>) -> ResultadoConsulta {
        ResultadoConsulta { resultados, alterou_esquema: false, alterou_dados: false, transacao_aberta: false }
    }

    fn linhas(n: usize, truncado: bool) -> ResultadoComando {
        ResultadoComando::Linhas {
            comando: "SELECT * FROM clientes ".into(),
            colunas: vec!["id".into()],
            linhas: (0..n).map(|i| vec![Celula::Inteiro(i as i64)]).collect(),
            truncado,
            tempo_ms: 1.6,
        }
    }

    #[test]
    fn hint_das_linhas() {
        let cards = montar(consulta(vec![linhas(8, false), linhas(1000, true), linhas(0, false)]), None, 1, None);
        let hints: Vec<(String, Option<String>, Option<String>, bool)> = cards
            .iter()
            .map(|c| {
                let (texto, nota, _) = c.parte();
                (texto, nota.map(str::to_string), c.rodape(), c.pedido().is_some())
            })
            .collect();
        let s = |t: &str| Some(t.to_string());
        assert_eq!(
            hints,
            vec![
                ("Comando 1".into(), s("8 linhas"), s("8 linhas · 1,6 ms"), true),
                (
                    "Comando 2".into(),
                    s("mais de 1.000 linhas"),
                    s("mais de 1.000 linhas · 1,6 ms · mostrando as primeiras 1.000"),
                    true
                ),
                ("Comando 3".into(), s("0 linhas"), s("0 linhas · 1,6 ms"), false),
            ]
        );
        let Some(Card::Linhas { pedido, .. }) = cards.first() else { panic!("resultado com linhas") };
        assert_eq!(pedido.nome_sugerido, "consulta_clientes");
        assert_eq!(pedido.origem, OrigemJanelaExportar::Consulta { sql: "SELECT * FROM clientes ".into() });
        assert_eq!(escolhido_de_partida(&cards), 0);
        assert!(faixa_erro(&cards).is_none());
    }

    #[test]
    fn alteracao_e_erro() {
        let cards = montar(
            consulta(vec![
                ResultadoComando::Alteracao {
                    comando: " UPDATE t SET a = 1;".into(),
                    linhas_afetadas: Some(1),
                    tempo_ms: 0.6,
                },
                ResultadoComando::Alteracao {
                    comando: "CREATE TABLE t (a)".into(),
                    linhas_afetadas: None,
                    tempo_ms: 4.0,
                },
                ResultadoComando::Erro {
                    comando: "SELECT * FROM x".into(),
                    mensagem: "A tabela não existe: x.".into(),
                    detalhe: Some("no such table: x".into()),
                },
            ]),
            None,
            1,
            None,
        );
        assert!(matches!(&cards[0], Card::Alteracao { numero: 1, texto, tempo, comando }
            if texto == "1 linha alterada" && tempo == "0,6 ms" && comando.texto() == "UPDATE t SET a = 1;"));
        assert_eq!(cards[0].rodape().as_deref(), Some("1 linha alterada · 0,6 ms"));
        assert_eq!(cards[1].parte().1, Some("executado"));
        assert!(
            matches!(&cards[2], Card::Erro { numero: 3, detalhe: Some(d), parou: true, .. } if d == "no such table: x")
        );
        assert_eq!(cards[2].parte(), ("Comando 3".into(), Some("erro"), true));
        // sem linhas, a tela abre no último (o erro), e a faixa de erro é a dele
        assert_eq!(escolhido_de_partida(&cards), 2);
        let faixa = faixa_erro(&cards).expect("a faixa de erro");
        assert_eq!((faixa.numero, faixa.mensagem, faixa.parou), (Some(3), "A tabela não existe: x.", true));
    }

    #[test]
    fn interrompido_sem_o_original() {
        let cards = montar(
            consulta(vec![ResultadoComando::Erro {
                comando: "SELECT 1".into(),
                mensagem: MSG_INTERROMPIDA.into(),
                detalhe: Some("interrupted".into()),
            }]),
            None,
            1,
            None,
        );
        assert!(matches!(&cards[0], Card::Erro { detalhe: None, parou: false, .. }));
        // pedida pela pessoa: no tom de atenção, sem o `--perigo` no seletor
        assert_eq!(cards[0].parte(), ("Comando 1".into(), Some("interrompido"), false));
        assert!(faixa_erro(&cards).is_some_and(|f| f.interrompido));
    }

    #[test]
    fn um_comando_a_mais_no_singular() {
        let lista: Vec<ResultadoComando> = (0..=MAX_CARDS)
            .map(|_| ResultadoComando::Alteracao { comando: "INSERT".into(), linhas_afetadas: Some(1), tempo_ms: 0.0 })
            .collect();
        let cards = montar(consulta(lista), None, 1, None);
        assert!(matches!(&cards[MAX_CARDS], Card::Mais { resto: 1, texto }
            if texto == "Mais 1 comando executado, que não aparece aqui."));
    }

    #[test]
    fn erro_que_ja_cita_o_original_nao_repete() {
        let cards = montar(
            consulta(vec![ResultadoComando::Erro {
                comando: "SELEC".into(),
                mensagem: "Erro de sintaxe: near \"SELEC\": syntax error".into(),
                detalhe: Some("near \"SELEC\": syntax error".into()),
            }]),
            None,
            1,
            None,
        );
        assert!(matches!(&cards[0], Card::Erro { detalhe: None, parou: false, .. }));
    }

    #[test]
    fn sem_comando_e_mensagem_final() {
        assert!(
            matches!(&montar(consulta(vec![]), None, 1, None)[0], Card::Aviso { texto, .. } if texto == MSG_SEM_COMANDO)
        );
        let fim = montar(consulta(vec![]), Some(("Transação gravada.", " As alterações…")), 1, None);
        assert!(
            matches!(&fim[0], Card::Aviso { forte: Some(f), tom: TomAviso::Concluido, .. } if f == "Transação gravada.")
        );
        // a falha da execução vira a faixa de erro sem o comando
        let falha = [Card::aviso("O banco foi fechado.", TomAviso::Erro)];
        assert!(faixa_erro(&falha).is_some_and(|f| f.numero.is_none()));
    }

    #[test]
    fn muitos_comandos_viram_contagem_e_o_erro_do_fim_aparece() {
        let mut lista: Vec<ResultadoComando> = (0..105)
            .map(|_| ResultadoComando::Alteracao { comando: "INSERT".into(), linhas_afetadas: Some(1), tempo_ms: 0.0 })
            .collect();
        lista.push(ResultadoComando::Erro { comando: "X".into(), mensagem: "Falhou.".into(), detalhe: None });
        let cards = montar(consulta(lista), None, 1, None);
        assert_eq!(cards.len(), MAX_CARDS + 2);
        assert!(matches!(&cards[MAX_CARDS], Card::Mais { resto: 6, texto }
            if texto == "Mais 6 comandos executados, que não aparecem aqui."));
        assert_eq!(cards[MAX_CARDS].parte().0, "Mais 6 comandos");
        assert!(matches!(&cards[MAX_CARDS + 1], Card::Erro { numero: 106, parou: true, .. }));
    }
}
