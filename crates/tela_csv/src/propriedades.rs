//! As Propriedades do CSV (prancheta 7): **Formato** (Separador, Decimal, Codificação, Fim de linha e a caixa
//! "Primeira linha é o cabeçalho"; todas dizem como **ler** o arquivo, decisão 6, e ficam desligadas com
//! alteração pendente), **Arquivo** (a `estado::secao_arquivo`) e **Coluna {nome}** da célula ativa (o tipo
//! detectado com a pastilha e as estatísticas, calculadas numa tarefa).

use egui::{Align, Layout, Sense, Ui, vec2};
use estado::{Contexto, InfoArquivo, secao_arquivo};
use interface::formato::formatar_numero;
use interface::icones::{self, Icone};
use interface::pecas::campos::{ALTURA_CAMPO, Check, Select};
use interface::pecas::detalhes::Detalhes;
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::texto as texto_simples;
use interface::tema::paleta;
use tabular::csv::{Codificacao, FimDeLinha, OpcoesAbertura, SEPARADORES};
use tabular::numero::{TipoDetectado, analisar_numero};

use crate::sessao::Retrato;
use crate::textos::{self, DICA_FORMATO_TRAVADO, PRIMEIRA_LINHA_CABECALHO};

/// O `.form` da seção Formato: `grid-template-columns:88px 1fr; gap:6px 10px; padding:2px 14px 10px`.
const LARGURA_ROTULO_FORM: f32 = 88.0;
const ESPACO_FORM_X: f32 = 10.0;
const ESPACO_FORM_Y: f32 = 6.0;
const RECUO_FORM_X: f32 = 14.0;
const RECUO_FORM_TOPO: f32 = 2.0;
const RECUO_FORM_BAIXO: f32 = 10.0;
/// O `label.op` da caixa: `padding:0 14px 14px`.
const RECUO_CAIXA_BAIXO: f32 = 14.0;

/// O que a pessoa pediu nas Propriedades neste quadro.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PedidoPropriedades {
    /// Ler o arquivo de novo com estas opções (um dos seletores mudou).
    Reabrir(OpcoesAbertura),
    Cabecalho(bool),
}

/// Uma linha do `.form`: o rótulo em `--t3` à esquerda (88px) e o controle no resto.
fn linha_do_form<R>(ui: &mut Ui, rotulo: &str, controle: impl FnOnce(&mut Ui) -> R) -> R {
    let largura = ui.available_width();
    ui.allocate_ui_with_layout(vec2(largura, ALTURA_CAMPO), Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let t3 = paleta(ui).estudio.t3;
        let (r, _) = ui.allocate_exact_size(vec2(LARGURA_ROTULO_FORM, ALTURA_CAMPO), Sense::hover());
        let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
        texto_simples(&mut filho, rotulo, 12.0, t3);
        ui.add_space(ESPACO_FORM_X);
        controle(ui)
    })
    .inner
}

/// As opções de um seletor com o valor atual garantido na lista (um separador fora dos quatro de sempre).
fn opcoes_com<T: PartialEq + Clone>(
    mut lista: Vec<(T, String)>,
    atual: &T,
    rotulo: impl Fn(&T) -> String,
) -> Vec<(T, String)> {
    if !lista.iter().any(|(v, _)| v == atual) {
        lista.push((atual.clone(), rotulo(atual)));
    }
    lista
}

/// Um seletor do Formato (desligado com alteração pendente, com a dica da decisão 6). Devolve o valor escolhido,
/// quando mudou.
fn seletor<T: PartialEq + Clone>(
    ui: &mut Ui,
    id: (&str, &str),
    rotulo: &str,
    atual: &T,
    opcoes: &[(T, String)],
    travado: bool,
) -> Option<T> {
    let mut valor = atual.clone();
    let opcoes: Vec<(T, &str)> = opcoes.iter().map(|(v, t)| (v.clone(), t.as_str())).collect();
    let largura = ui.available_width();
    let r = ui
        .scope(|ui| {
            if travado {
                ui.disable();
            }
            Select::new(id, &mut valor, &opcoes).largura(largura).rotulo(rotulo).mostrar(ui)
        })
        .inner;
    r.on_disabled_hover_text(DICA_FORMATO_TRAVADO);
    (valor != *atual).then_some(valor)
}

/// A seção Formato. `opcoes` são as que a pessoa já escolheu (o que fica `None` segue a detecção); mudar um
/// seletor pede a leitura com ele fixo.
pub(crate) fn secao_formato(
    ui: &mut Ui,
    id: &str,
    retrato: &Retrato,
    opcoes: &OpcoesAbertura,
    ocupada: bool,
) -> Option<PedidoPropriedades> {
    let f = &retrato.formato;
    let travado = retrato.alterada || ocupada;
    let mut pedido = None;
    let _ = Secao::new(("propriedades::formato", id), "Formato").primeira(true).mostrar(ui, |ui| {
        let largura = ui.available_width();
        let dentro = egui::Rect::from_min_size(
            ui.cursor().min + vec2(RECUO_FORM_X, RECUO_FORM_TOPO),
            vec2((largura - 2.0 * RECUO_FORM_X).max(0.0), 0.0),
        );
        let mut form = ui.new_child(egui::UiBuilder::new().max_rect(dentro).layout(Layout::top_down(Align::Min)));
        form.spacing_mut().item_spacing.y = ESPACO_FORM_Y;
        let separadores =
            opcoes_com(SEPARADORES.iter().map(|&c| (c, textos::rotulo_do_separador(c))).collect(), &f.separador, |c| {
                textos::rotulo_do_separador(*c)
            });
        if let Some(c) = linha_do_form(&mut form, "Separador", |ui| {
            seletor(ui, (id, "separador"), "Separador", &f.separador, &separadores, travado)
        }) {
            pedido = Some(PedidoPropriedades::Reabrir(OpcoesAbertura { separador: Some(c), ..opcoes.clone() }));
        }
        let decimais: Vec<(char, String)> =
            [',', '.'].iter().map(|&c| (c, textos::rotulo_do_decimal(c).to_string())).collect();
        if let Some(c) = linha_do_form(&mut form, "Decimal", |ui| {
            seletor(ui, (id, "decimal"), "Decimal", &f.decimal, &decimais, travado)
        }) {
            pedido = Some(PedidoPropriedades::Reabrir(OpcoesAbertura { decimal: Some(c), ..opcoes.clone() }));
        }
        let codificacoes: Vec<(Codificacao, String)> =
            Codificacao::TODAS.iter().map(|&c| (c, c.rotulo().to_string())).collect();
        if let Some(c) = linha_do_form(&mut form, "Codificação", |ui| {
            seletor(ui, (id, "codificacao"), "Codificação", &f.codificacao, &codificacoes, travado)
        }) {
            pedido = Some(PedidoPropriedades::Reabrir(OpcoesAbertura { codificacao: Some(c), ..opcoes.clone() }));
        }
        let fins: Vec<(FimDeLinha, String)> = FimDeLinha::TODOS.iter().map(|&c| (c, c.rotulo().to_string())).collect();
        if let Some(c) = linha_do_form(&mut form, "Fim de linha", |ui| {
            seletor(ui, (id, "fim_de_linha"), "Fim de linha", &f.fim_de_linha, &fins, travado)
        }) {
            pedido = Some(PedidoPropriedades::Reabrir(OpcoesAbertura { fim_de_linha: Some(c), ..opcoes.clone() }));
        }
        let altura = form.min_rect().height() + RECUO_FORM_TOPO + RECUO_FORM_BAIXO;
        ui.allocate_space(vec2(largura, altura));
        // a caixa "Primeira linha é o cabeçalho" (`label.op`, padding 0 14 14)
        let mut cabecalho = f.cabecalho;
        ui.horizontal(|ui| {
            ui.add_space(RECUO_FORM_X);
            let r = ui
                .scope(|ui| {
                    if travado {
                        ui.disable();
                    }
                    Check::new(&mut cabecalho, PRIMEIRA_LINHA_CABECALHO).mostrar(ui)
                })
                .inner;
            r.on_disabled_hover_text(DICA_FORMATO_TRAVADO);
        });
        ui.add_space(RECUO_CAIXA_BAIXO);
        if cabecalho != f.cabecalho {
            pedido = Some(PedidoPropriedades::Cabecalho(cabecalho));
        }
    });
    pedido
}

/// A seção Arquivo (Caminho com o Copiar caminho, Tamanho, Modificado em).
pub(crate) fn secao_do_arquivo(ui: &mut Ui, ctx: &mut Contexto, retrato: &Retrato) {
    let info = InfoArquivo {
        caminho: retrato.caminho.clone(),
        tamanho: Some(retrato.tamanho),
        modificado: retrato.modificado,
    };
    secao_arquivo(ui, ctx, &info, |_| {});
}

/// As estatísticas de uma coluna (a seção Coluna).
#[derive(Debug, Clone, PartialEq)]
pub struct Estatistica {
    pub tipo: TipoDetectado,
    pub vazias: u64,
    pub minimo: Option<String>,
    pub maximo: Option<String>,
    pub soma: Option<String>,
    /// Caracteres do maior texto (nas colunas de texto).
    pub maior_texto: Option<usize>,
}

/// As contas de uma coluna feitas numa passada, um valor por vez, sem guardar os valores (memória constante numa
/// coluna de milhões de linhas): Vazias sempre; nas numéricas, Mínimo, Máximo (o texto da célula, como está no
/// arquivo) e Soma (no formato do arquivo); nas de data, Mínimo e Máximo pela ordem do texto; nas de texto, o Maior
/// texto. [`calcular_estatistica`] faz o mesmo sobre um iterador.
#[derive(Debug, Clone)]
pub struct Contas {
    tipo: TipoDetectado,
    decimal: char,
    vazias: u64,
    soma: f64,
    casas: usize,
    /// O menor e o maior número com o texto da célula (nas numéricas).
    menor: Option<(f64, String)>,
    maior: Option<(f64, String)>,
    /// A menor e a maior data pela ordem do texto.
    primeira: Option<String>,
    ultima: Option<String>,
    maior_texto: Option<usize>,
}

impl Contas {
    pub fn nova(tipo: TipoDetectado, decimal: char) -> Contas {
        Contas {
            tipo,
            decimal,
            vazias: 0,
            soma: 0.0,
            casas: 0,
            menor: None,
            maior: None,
            primeira: None,
            ultima: None,
            maior_texto: None,
        }
    }

    /// Conta mais um valor da coluna.
    pub fn somar(&mut self, valor: &str) {
        let aparado = valor.trim();
        if aparado.is_empty() {
            self.vazias += 1;
        }
        match self.tipo {
            TipoDetectado::Inteiro | TipoDetectado::Decimal => {
                let Some(n) = analisar_numero(valor, self.decimal) else { return };
                self.soma += n;
                if let Some((_, f)) = aparado.split_once(self.decimal) {
                    self.casas = self.casas.max(f.chars().take_while(char::is_ascii_digit).count());
                }
                if self.menor.as_ref().is_none_or(|(m, _)| n < *m) {
                    self.menor = Some((n, aparado.to_string()));
                }
                if self.maior.as_ref().is_none_or(|(m, _)| n > *m) {
                    self.maior = Some((n, aparado.to_string()));
                }
            }
            TipoDetectado::Data => {
                if aparado.is_empty() {
                    return;
                }
                if self.primeira.as_deref().is_none_or(|p| aparado < p) {
                    self.primeira = Some(aparado.to_string());
                }
                if self.ultima.as_deref().is_none_or(|u| aparado > u) {
                    self.ultima = Some(aparado.to_string());
                }
            }
            _ => {
                let n = valor.chars().count();
                self.maior_texto = Some(self.maior_texto.map_or(n, |m| m.max(n)));
            }
        }
    }

    /// As contas de todos os valores somados.
    pub fn estatistica(self) -> Estatistica {
        let mut e = Estatistica {
            tipo: self.tipo,
            vazias: self.vazias,
            minimo: None,
            maximo: None,
            soma: None,
            maior_texto: self.maior_texto,
        };
        match self.tipo {
            TipoDetectado::Inteiro | TipoDetectado::Decimal => {
                e.soma =
                    self.menor.is_some().then(|| textos::numero_no_formato(self.soma, self.decimal, self.casas.min(6)));
                e.minimo = self.menor.map(|(_, t)| t);
                e.maximo = self.maior.map(|(_, t)| t);
            }
            TipoDetectado::Data => {
                e.minimo = self.primeira;
                e.maximo = self.ultima;
            }
            _ => {}
        }
        e
    }
}

/// Calcula as estatísticas da coluna numa passada pelos `valores` (as [`Contas`]), sem guardá-los.
pub fn calcular_estatistica<S: AsRef<str>>(
    valores: impl IntoIterator<Item = S>,
    tipo: TipoDetectado,
    decimal: char,
) -> Estatistica {
    let mut contas = Contas::nova(tipo, decimal);
    for v in valores {
        contas.somar(v.as_ref());
    }
    contas.estatistica()
}

/// A seção "Coluna {nome}" (só com uma coluna escolhida): o tipo detectado com a pastilha e as estatísticas (ou
/// "Calculando…" enquanto a tarefa conta, `calculando`; "—" se a conta falhou). `acoes` desenha os botões embaixo
/// da ficha.
pub(crate) fn secao_coluna(
    ui: &mut Ui,
    id: &str,
    nome: &str,
    tipo: TipoDetectado,
    estatistica: Option<&Estatistica>,
    calculando: bool,
    acoes: impl FnOnce(&mut Ui),
) {
    let _ = Secao::new(("propriedades::coluna", id), "Coluna").extra(nome).mostrar(ui, |ui| {
        ficha(ui, |f: &mut Detalhes<'_>| {
            f.linha("Tipo detectado", |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icones::pintar(ui.painter(), r, Icone::Tipo(textos::pastilha(tipo)), egui::Color32::WHITE);
                    let t = paleta(ui).estudio.t;
                    texto_simples(ui, textos::rotulo_do_tipo(tipo), 12.0, t);
                });
            });
            // sem as contas: "Calculando…" na primeira linha e "…" nas outras (ou "—", se a conta falhou)
            let pendente = if calculando { "…" } else { "—" };
            let vazias = estatistica.map(|e| formatar_numero(e.vazias));
            f.linha_texto("Vazias", vazias.as_deref(), if calculando { textos::CALCULANDO } else { pendente });
            match tipo {
                TipoDetectado::Inteiro | TipoDetectado::Decimal => {
                    let e = estatistica;
                    f.linha_texto("Mínimo", e.map(|e| e.minimo.as_deref().unwrap_or("—")), pendente);
                    f.linha_texto("Máximo", e.map(|e| e.maximo.as_deref().unwrap_or("—")), pendente);
                    f.linha_texto("Soma", e.map(|e| e.soma.as_deref().unwrap_or("—")), pendente);
                }
                TipoDetectado::Data => {
                    let e = estatistica;
                    f.linha_texto("Mínimo", e.map(|e| e.minimo.as_deref().unwrap_or("—")), pendente);
                    f.linha_texto("Máximo", e.map(|e| e.maximo.as_deref().unwrap_or("—")), pendente);
                }
                _ => {
                    let maior = estatistica.map(|e| match e.maior_texto {
                        Some(1) => "1 caractere".to_string(),
                        Some(n) => format!("{} caracteres", formatar_numero(n as u64)),
                        None => "—".to_string(),
                    });
                    f.linha_texto("Maior texto", maior.as_deref(), pendente);
                }
            }
        });
        acoes(ui);
    });
}

/// Os botões da seção Coluna (`padding:0 8px 10px; gap:2px`, os `.btn.sutil.lista`).
const RECUO_ACOES_X: f32 = 8.0;
const RECUO_ACOES_BAIXO: f32 = 10.0;
const ESPACO_ACOES: f32 = 2.0;

/// O que os botões da seção Coluna pediram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcaoColuna {
    Trazer,
    Copiar,
    Renomear,
    Excluir,
}

/// Os botões embaixo da ficha da seção Coluna: Trazer colunas, Copiar coluna, Renomear a coluna e Excluir a coluna.
/// Sem cabeçalho no arquivo (`renomear` falso), o nome não teria onde ficar: Renomear desligado, com a dica.
pub(crate) fn acoes_da_coluna(ui: &mut Ui, livre: bool, renomear: bool) -> Option<AcaoColuna> {
    use interface::pecas::botao::{Botao, TipoBotao};
    let mut acao = None;
    let largura = ui.available_width();
    let r = egui::Rect::from_min_size(
        ui.cursor().min + vec2(RECUO_ACOES_X, 0.0),
        vec2((largura - 2.0 * RECUO_ACOES_X).max(0.0), 0.0),
    );
    let mut dentro = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
    dentro.spacing_mut().item_spacing.y = ESPACO_ACOES;
    if dentro.add(Botao::ghost("Trazer colunas").com_icone(Icone::TrazerColunas).lista().habilitado(livre)).clicked() {
        acao = Some(AcaoColuna::Trazer);
    }
    if dentro.add(Botao::ghost("Copiar coluna").com_icone(Icone::Duplicar).lista()).clicked() {
        acao = Some(AcaoColuna::Copiar);
    }
    let r =
        dentro.add(Botao::ghost("Renomear a coluna").com_icone(Icone::Editar).lista().habilitado(livre && renomear));
    let r = if renomear { r } else { r.on_disabled_hover_text(textos::DICA_SEM_CABECALHO) };
    if r.clicked() {
        acao = Some(AcaoColuna::Renomear);
    }
    if dentro
        .add(
            Botao::new("Excluir a coluna")
                .com_icone(Icone::Excluir)
                .lista()
                .tipo(TipoBotao::GhostPerigo)
                .habilitado(livre),
        )
        .clicked()
    {
        acao = Some(AcaoColuna::Excluir);
    }
    let altura = dentro.min_rect().height() + RECUO_ACOES_BAIXO;
    ui.allocate_space(vec2(largura, altura));
    acao
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn estatistica_da_coluna_numerica() {
        let e = calcular_estatistica(["1", "23", "", "4"], TipoDetectado::Inteiro, ',');
        assert_eq!(e.vazias, 1);
        assert_eq!((e.minimo.as_deref(), e.maximo.as_deref(), e.soma.as_deref()), (Some("1"), Some("23"), Some("28")));
        let d = calcular_estatistica(["12,90", "1.056,30", "4,5"], TipoDetectado::Decimal, ',');
        assert_eq!(d.minimo.as_deref(), Some("4,5"));
        assert_eq!(d.maximo.as_deref(), Some("1.056,30"));
        assert_eq!(d.soma.as_deref(), Some("1.073,7"));
    }

    #[test]
    fn estatistica_de_texto_e_de_data() {
        let t = calcular_estatistica(["Sul", "Centro-Oeste", " "], TipoDetectado::Texto, ',');
        assert_eq!((t.vazias, t.maior_texto), (1, Some(12)));
        let d = calcular_estatistica(["2025-01-03", "2025-01-02", ""], TipoDetectado::Data, ',');
        assert_eq!((d.minimo.as_deref(), d.maximo.as_deref()), (Some("2025-01-02"), Some("2025-01-03")));
    }
}
