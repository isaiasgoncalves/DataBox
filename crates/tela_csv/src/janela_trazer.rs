//! A janela "Trazer colunas" (prancheta 16 d): 640px no vidro, três blocos que
//! se leem como uma frase. **De onde** (as abas com tabela, a área de transferência e "Outro arquivo", com "5.038
//! linhas · 9 colunas"), **Casar as linhas onde** (as duas chaves, sugeridas pelo nome ou pelos valores em comum
//! numa amostra; o medidor ao vivo "4.812 de 5.000 linhas têm par (96%)" com a barra de progresso do Exportar; as três
//! opções; o aviso das chaves repetidas com "Usar" e "Ver quais") e **Trazer** (as colunas da origem com a caixa, a
//! pastilha e o destino: "nova coluna [preco]" ou "atualizar [regiao] substituir / só as células vazias"), mais a
//! **Prévia** das 6 primeiras linhas. Rodapé: Cancelar · Trazer 3 colunas.
//!
//! Tudo o que lê roda em tarefas (a chave do destino fora do `Mutex`, as colunas da origem pela `FonteDeColunas`, o
//! `casar` numa thread); trocar uma opção refaz só o casamento, e trocar o "Usar" só a prévia.

use std::collections::HashMap;
use std::sync::Arc;

use comum::Resultado;
use egui::{
    Align, Color32, CursorIcon, Layout, Rect, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use estado::{FonteDeColunas, Promessa};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_numero, plural};
use interface::icones::{self, Icone, TipoArquivo};
use interface::pecas::avisos::Aviso;
use interface::pecas::botao::Botao;
use interface::pecas::campos::{ALTURA_CAMPO, Check, Entrada, Select};
use interface::pecas::janela::Janela;
use interface::pecas::progresso::barra_progresso;
use interface::pecas::texto::{galley, galley_tit, link, texto as texto_simples};
use interface::tema::paleta;
use tabular::colunas::{
    AMOSTRA_DA_SUGESTAO, Casamento, ChavesRepetidas, ModoAtualizacao, OpcoesCasamento, casar, nome_da_coluna_trazida,
    rotulo_da_origem, sugerir_chaves,
};
use tabular::numero::{TipoDetectado, tipo_da_coluna};

use crate::sessao::{self, Controle, TabelaCompartilhada};
use crate::textos;

/// A largura da janela (`.dlg.tc{width:640px}`).
const LARGURA: f32 = 640.0;
/// O `gap:14px` entre os blocos e o `gap:8px` dentro deles.
const ESPACO_BLOCOS: f32 = 14.0;
const ESPACO_BLOCO: f32 = 8.0;
/// A coluna do "=" entre as duas chaves (`grid-template-columns:1fr 20px 1fr; gap 8px`).
const LARGURA_IGUAL: f32 = 20.0;
/// As linhas da lista "Trazer" (`.lt{height:30px}`, `max-height:192px` com o padding de 4).
const ALTURA_LT: f32 = 30.0;
const ALTURA_LISTA: f32 = 192.0;
/// O campo do nome da coluna nova (`.dest input.campo.pq{width:128px}`) e o seletor do destino.
const LARGURA_NOME: f32 = 128.0;
const LARGURA_DESTINO: f32 = 124.0;
/// As linhas da grade das chaves: a do campo (28) e a da meta embaixo (16).
const ALTURA_META: f32 = 16.0;
const LARGURA_MODO: f32 = 150.0;
/// As 6 linhas da prévia (`td{height:24px}`, `th{height:28px}`).
const LINHAS_DA_PREVIA: usize = 6;
const ALTURA_LINHA_PREVIA: f32 = 24.0;
const ALTURA_CAB_PREVIA: f32 = 28.0;
/// Quantas chaves repetidas o "Ver quais" lista.
const MAX_REPETIDAS_LISTADAS: usize = 20;

/// Uma origem de colunas pelo endereço da `Arc` (a mesma aba dá a mesma `Arc` a cada quadro).
fn id_da_fonte(f: &Arc<dyn FonteDeColunas>) -> usize {
    Arc::as_ptr(f) as *const () as usize
}

/// O destino de uma coluna trazida.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DestinoTrazido {
    /// Uma coluna nova com o nome (editável).
    Nova { nome: String },
    /// Atualizar uma coluna que existe deste arquivo.
    Atualizar { coluna: usize, modo: ModoAtualizacao },
}

/// O que se pediu para uma coluna da origem.
#[derive(Debug, Clone)]
struct Escolha {
    marcada: bool,
    destino: DestinoTrazido,
}

/// O que identifica um casamento já feito (a troca de qualquer um refaz a conta).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ChaveCasamento {
    fonte: usize,
    destino: usize,
    origem: usize,
    opcoes: OpcoesCasamento,
    versao: u64,
}

/// O que a janela pediu à tela neste quadro.
pub(crate) enum PedidoDaJanela {
    Fechar,
    /// Trazer: as colunas (os valores da origem, uma por linha dela, e o destino), o casamento e o modo das repetidas.
    Trazer(PlanoTrazer),
    /// Abrir "Outro arquivo" (o diálogo é da tela, que tem o `Contexto`).
    OutroArquivo,
}

/// O plano do Trazer, que a mutação aplica num passo só.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanoTrazer {
    pub(crate) casamento: Arc<Casamento>,
    pub(crate) colunas: Vec<(Arc<Vec<String>>, DestinoTrazido)>,
    pub(crate) repetidas: ChavesRepetidas,
}

/// A leitura das colunas da origem: a fonte (pelo endereço), as colunas pedidas e a tarefa.
type LeituraDaOrigem = (usize, Vec<usize>, Promessa<Vec<Vec<String>>>);
/// A sugestão das chaves pelos valores (a coluna deste arquivo e a da origem).
type ParDeChaves = Option<(usize, usize)>;

/// A janela aberta.
pub(crate) struct JanelaTrazer {
    pub fonte: Option<Arc<dyn FonteDeColunas>>,
    /// A área de transferência (quando se sabe o texto: o Ctrl+Shift+V, o Colar com correspondência).
    pub area: Option<Arc<dyn FonteDeColunas>>,
    /// O "Outro arquivo", depois de aberto.
    pub outro: Option<Arc<dyn FonteDeColunas>>,
    pub abrindo_outro: Option<Promessa<Arc<dyn FonteDeColunas>>>,
    nomes_origem: Vec<String>,
    fonte_dos_nomes: Option<usize>,
    pub chave_destino: Option<usize>,
    pub chave_origem: Option<usize>,
    pub opcoes: OpcoesCasamento,
    pub repetidas: ChavesRepetidas,
    escolhas: Vec<Escolha>,
    /// A coluna-chave deste arquivo (coluna, versão do documento, valores).
    destino: Option<(usize, u64, Arc<Vec<String>>)>,
    /// As colunas da origem já lidas (da fonte de agora).
    origem: HashMap<usize, Arc<Vec<String>>>,
    tipos_origem: HashMap<usize, TipoDetectado>,
    lendo_destino: Option<(usize, u64, Promessa<Vec<String>>)>,
    lendo_origem: Option<LeituraDaOrigem>,
    amostras: Option<(usize, Promessa<ParDeChaves>)>,
    sugerir: bool,
    casamento: Option<(ChaveCasamento, Arc<Casamento>)>,
    calculando: Option<(ChaveCasamento, Promessa<Casamento>)>,
    previa: Option<(u64, Vec<Vec<String>>)>,
    lendo_previa: Option<(u64, Promessa<Vec<Vec<String>>>)>,
    controle: Arc<Controle>,
    /// O cálculo foi cancelado (o medidor oferece "Calcular de novo").
    cancelado: bool,
    ver_quais: bool,
    /// O separador decimal deste CSV (o destino): a planilha escreve os números crus com ele.
    pub decimal: char,
}

impl JanelaTrazer {
    /// A origem como ela deve ser lida para este arquivo: a planilha com os números no decimal dele (decisão 16 da
    /// etapa 6); as outras como estão. A identidade da origem (o seletor) continua a da `fonte`.
    fn origem_no_decimal(&self, fonte: &Arc<dyn FonteDeColunas>) -> Arc<dyn FonteDeColunas> {
        fonte.para_o_decimal(self.decimal).unwrap_or_else(|| Arc::clone(fonte))
    }

    pub(crate) fn nova(fonte: Option<Arc<dyn FonteDeColunas>>, area: Option<Arc<dyn FonteDeColunas>>) -> JanelaTrazer {
        JanelaTrazer {
            fonte,
            area,
            outro: None,
            abrindo_outro: None,
            nomes_origem: Vec::new(),
            fonte_dos_nomes: None,
            chave_destino: None,
            chave_origem: None,
            opcoes: OpcoesCasamento::default(),
            repetidas: ChavesRepetidas::default(),
            escolhas: Vec::new(),
            destino: None,
            origem: HashMap::new(),
            tipos_origem: HashMap::new(),
            lendo_destino: None,
            lendo_origem: None,
            amostras: None,
            sugerir: true,
            casamento: None,
            calculando: None,
            previa: None,
            lendo_previa: None,
            controle: Controle::novo(),
            cancelado: false,
            ver_quais: false,
            decimal: ',',
        }
    }

    /// Há leitura ou cálculo em andamento.
    pub(crate) fn ocupada(&self) -> bool {
        self.lendo_destino.is_some()
            || self.lendo_origem.is_some()
            || self.amostras.is_some()
            || self.calculando.is_some()
            || self.lendo_previa.is_some()
            || self.abrindo_outro.is_some()
    }

    /// O casamento de agora (o medidor), se já foi feito para as escolhas de agora.
    pub(crate) fn casamento(&self) -> Option<&Casamento> {
        let chave = self.chave_atual(u64::MAX)?;
        self.casamento
            .as_ref()
            .filter(|(k, _)| ChaveCasamento { versao: k.versao, ..chave } == *k)
            .map(|(_, c)| c.as_ref())
    }

    fn chave_atual(&self, versao: u64) -> Option<ChaveCasamento> {
        Some(ChaveCasamento {
            fonte: id_da_fonte(self.fonte.as_ref()?),
            destino: self.chave_destino?,
            origem: self.chave_origem?,
            opcoes: self.opcoes,
            versao,
        })
    }

    /// As colunas marcadas (da origem).
    pub(crate) fn marcadas(&self) -> Vec<usize> {
        self.escolhas.iter().enumerate().filter(|(_, e)| e.marcada).map(|(i, _)| i).collect()
    }

    /// Marca ou desmarca uma coluna da origem.
    pub(crate) fn marcar(&mut self, coluna: usize, sim: bool) {
        if let Some(e) = self.escolhas.get_mut(coluna) {
            e.marcada = sim;
        }
    }

    pub(crate) fn definir_destino(&mut self, coluna: usize, destino: DestinoTrazido) {
        if let Some(e) = self.escolhas.get_mut(coluna) {
            e.destino = destino;
        }
    }

    pub(crate) fn destino_de(&self, coluna: usize) -> Option<&DestinoTrazido> {
        self.escolhas.get(coluna).map(|e| &e.destino)
    }

    pub(crate) fn nomes_origem(&self) -> &[String] {
        &self.nomes_origem
    }

    /// A troca de origem: os nomes, as escolhas e as chaves recomeçam (a sugestão roda de novo).
    fn trocar_de_fonte(&mut self, nomes_destino: &[String]) {
        let Some(f) = self.fonte.clone() else { return };
        self.nomes_origem = f.nomes();
        self.fonte_dos_nomes = Some(id_da_fonte(&f));
        let rotulo = rotulo_da_origem(&f.rotulo());
        self.escolhas = self
            .nomes_origem
            .iter()
            .map(|n| Escolha {
                marcada: false,
                destino: DestinoTrazido::Nova { nome: nome_da_coluna_trazida(n, &rotulo, nomes_destino) },
            })
            .collect();
        self.chave_origem = None;
        self.origem.clear();
        self.tipos_origem.clear();
        self.lendo_origem = None;
        self.amostras = None;
        self.casamento = None;
        self.calculando = None;
        self.sugerir = true;
        self.cancelado = false;
    }

    /// Toma o que as tarefas entregaram e pede o que falta (a cada quadro).
    pub(crate) fn atualizar(
        &mut self,
        ectx: &egui::Context,
        tabela: &TabelaCompartilhada,
        nomes_destino: &[String],
        amostra_destino: &[Vec<String>],
        versao: u64,
    ) {
        if let Some(r) = self.abrindo_outro.as_mut().and_then(Promessa::tomar) {
            self.abrindo_outro = None;
            match r {
                Ok(f) => {
                    self.outro = Some(Arc::clone(&f));
                    self.fonte = Some(f);
                }
                Err(e) => interface::pecas::toast::toast_erro(ectx, e.mensagem),
            }
        }
        if self.fonte.as_ref().map(id_da_fonte) != self.fonte_dos_nomes {
            self.trocar_de_fonte(nomes_destino);
        }
        let Some(fonte) = self.fonte.clone() else { return };
        // as respostas
        if let Some((c, v, p)) = self.lendo_destino.as_mut()
            && let Some(r) = p.tomar()
        {
            let (c, v) = (*c, *v);
            self.lendo_destino = None;
            if let Ok(valores) = r {
                self.destino = Some((c, v, Arc::new(valores)));
            }
        }
        if let Some((f, cols, p)) = self.lendo_origem.as_mut()
            && let Some(r) = p.tomar()
        {
            let (f, cols) = (*f, std::mem::take(cols));
            self.lendo_origem = None;
            match r {
                Ok(valores) if Some(f) == self.fonte_dos_nomes => {
                    for (c, v) in cols.into_iter().zip(valores) {
                        self.tipos_origem
                            .insert(c, tipo_da_coluna(v.iter().take(AMOSTRA_DA_SUGESTAO).map(String::as_str), ','));
                        self.origem.insert(c, Arc::new(v));
                    }
                }
                Ok(_) => {}
                Err(e) if e.tem_codigo(comum::Codigo::Cancelado) => self.cancelado = true,
                Err(e) => interface::pecas::toast::toast_erro(ectx, e.mensagem),
            }
        }
        if let Some((f, p)) = self.amostras.as_mut()
            && let Some(r) = p.tomar()
        {
            let f = *f;
            self.amostras = None;
            if Some(f) == self.fonte_dos_nomes
                && let Ok(Some((d, o))) = r
            {
                self.chave_destino.get_or_insert(d);
                self.chave_origem.get_or_insert(o);
            }
        }
        if let Some((k, p)) = self.calculando.as_mut()
            && let Some(r) = p.tomar()
        {
            let k = *k;
            self.calculando = None;
            match r {
                Ok(c) => self.casamento = Some((k, Arc::new(c))),
                Err(e) if e.tem_codigo(comum::Codigo::Cancelado) => self.cancelado = true,
                Err(e) => interface::pecas::toast::toast_erro(ectx, e.mensagem),
            }
        }
        if let Some((v, p)) = self.lendo_previa.as_mut()
            && let Some(r) = p.tomar()
        {
            let v = *v;
            self.lendo_previa = None;
            if let Ok(l) = r {
                self.previa = Some((v, l));
            }
        }
        // a sugestão das chaves: pelo nome (sem ler nada) ou pelos valores em comum numa amostra
        if self.sugerir && !self.nomes_origem.is_empty() {
            self.sugerir = false;
            match sugerir_chaves(nomes_destino, &[], &self.nomes_origem, &[], &self.opcoes) {
                Some((d, o)) => {
                    self.chave_destino = Some(d);
                    self.chave_origem = Some(o);
                }
                None => {
                    let (fonte2, nomes_d, nomes_o) =
                        (self.origem_no_decimal(&fonte), nomes_destino.to_vec(), self.nomes_origem.clone());
                    let amostra_d: Vec<Vec<String>> = (0..nomes_destino.len())
                        .map(|c| {
                            amostra_destino
                                .iter()
                                .take(AMOSTRA_DA_SUGESTAO)
                                .map(|l| l.get(c).cloned().unwrap_or_default())
                                .collect()
                        })
                        .collect();
                    let opcoes = self.opcoes;
                    let controle = Arc::clone(&self.controle);
                    let p = Promessa::rodar(ectx, move || {
                        let todas: Vec<usize> = (0..nomes_o.len()).collect();
                        let mut amostra_o = fonte2.colunas(&todas, &mut |f, t| controle.avisar(f, t))?;
                        for c in &mut amostra_o {
                            c.truncate(AMOSTRA_DA_SUGESTAO);
                        }
                        Ok(sugerir_chaves(&nomes_d, &amostra_d, &nomes_o, &amostra_o, &opcoes))
                    });
                    self.amostras = Some((id_da_fonte(&fonte), p));
                }
            }
        }
        // a chave deste arquivo, fora do Mutex
        if let Some(d) = self.chave_destino
            && self.lendo_destino.is_none()
            && !self.destino.as_ref().is_some_and(|(c, v, _)| *c == d && *v == versao)
        {
            let t = Arc::clone(tabela);
            let controle = Arc::clone(&self.controle);
            let p = Promessa::rodar(ectx, move || {
                let mut v = sessao::ler_colunas(&t, &[d], &mut |f, tot| controle.avisar(f, tot))?;
                Ok(v.pop().unwrap_or_default())
            });
            self.lendo_destino = Some((d, versao, p));
        }
        // as colunas da origem que faltam: a chave e as marcadas (uma leitura por vez)
        if self.lendo_origem.is_none() && !self.cancelado {
            let mut faltam: Vec<usize> = self.chave_origem.into_iter().chain(self.marcadas()).collect();
            faltam.sort_unstable();
            faltam.dedup();
            faltam.retain(|c| !self.origem.contains_key(c));
            if !faltam.is_empty() {
                let (f2, cols) = (self.origem_no_decimal(&fonte), faltam.clone());
                let controle = Arc::clone(&self.controle);
                let p = Promessa::rodar(ectx, move || f2.colunas(&cols, &mut |f, t| controle.avisar(f, t)));
                self.lendo_origem = Some((id_da_fonte(&fonte), faltam, p));
            }
        }
        // o casamento das escolhas de agora
        if let Some(k) = self.chave_atual(versao)
            && self.calculando.is_none()
            && !self.cancelado
            && self.casamento.as_ref().is_none_or(|(c, _)| *c != k)
            && let (Some((dc, dv, dest)), Some(orig)) = (self.destino.as_ref(), self.origem.get(&k.origem))
            && *dc == k.destino
            && *dv == versao
        {
            let (dest, orig, opcoes) = (Arc::clone(dest), Arc::clone(orig), k.opcoes);
            let p = Promessa::rodar(ectx, move || Ok(casar(dest.as_slice(), orig.as_slice(), &opcoes)));
            self.calculando = Some((k, p));
        }
        // a prévia: as 6 primeiras linhas deste arquivo (o Mutex por um instante)
        if self.lendo_previa.is_none() && self.previa.as_ref().is_none_or(|(v, _)| *v != versao) {
            let t = Arc::clone(tabela);
            let p = Promessa::rodar(ectx, move || -> Resultado<Vec<Vec<String>>> {
                sessao::travar(&t).pagina(0, LINHAS_DA_PREVIA)
            });
            self.lendo_previa = Some((versao, p));
        }
    }

    /// Cancela as leituras e o cálculo em andamento (o "Cancelar" do medidor).
    pub(crate) fn cancelar_calculo(&mut self) {
        self.controle.cancelar();
        self.controle = Controle::novo();
        self.cancelado = true;
    }

    // ------------------------------------------------------------------ desenho

    /// Desenha a janela. `fontes` são as abas com tabela (sem esta), na ordem das abas.
    pub(crate) fn mostrar(
        &mut self,
        ectx: &egui::Context,
        nome_destino: &str,
        nomes_destino: &[String],
        linhas_destino: u64,
        fontes: &[Arc<dyn FonteDeColunas>],
    ) -> Option<PedidoDaJanela> {
        let mut pedido = None;
        let n_marcadas = self.marcadas().len();
        let com_par = self.casamento().map(Casamento::com_par);
        let pode = n_marcadas > 0 && com_par.is_some_and(|n| n > 0) && !self.ocupada();
        let rotulo_trazer = crate::trazer::rotulo_trazer(n_marcadas);
        let sub = format!("para {nome_destino}");
        let mut clicou_trazer = false;
        let mut clicou_cancelar = false;
        let r = Janela::new("tela_csv::trazer", "Trazer colunas")
            .largura(LARGURA)
            .icone(Icone::Folha(TipoArquivo::Csv))
            .sub(&sub)
            .rodape_direita(|ui| {
                if ui.add(Botao::ghost("Cancelar").dica("Esc")).clicked() {
                    clicou_cancelar = true;
                }
                if ui.add(Botao::primario(&rotulo_trazer).habilitado(pode)).clicked() {
                    clicou_trazer = true;
                }
            })
            .mostrar(ectx, |ui| {
                ui.spacing_mut().item_spacing.y = ESPACO_BLOCOS;
                pedido = self.bloco_de_onde(ui, fontes);
                self.bloco_casar(ui, nomes_destino, linhas_destino);
                self.bloco_trazer(ui, nomes_destino);
                self.bloco_previa(ui, nome_destino, nomes_destino);
            });
        if r.pediu_fechar || clicou_cancelar {
            return Some(PedidoDaJanela::Fechar);
        }
        if clicou_trazer && pode {
            return self.plano().map(PedidoDaJanela::Trazer);
        }
        pedido
    }

    /// O plano do Trazer com as escolhas de agora.
    pub(crate) fn plano(&self) -> Option<PlanoTrazer> {
        let chave = self.chave_atual(0)?;
        let casamento = self.casamento.as_ref().filter(|(k, _)| ChaveCasamento { versao: k.versao, ..chave } == *k)?;
        let colunas = self
            .marcadas()
            .into_iter()
            .map(|c| Some((Arc::clone(self.origem.get(&c)?), self.escolhas[c].destino.clone())))
            .collect::<Option<Vec<_>>>()?;
        Some(PlanoTrazer { casamento: Arc::clone(&casamento.1), colunas, repetidas: self.repetidas })
    }

    fn bloco_de_onde(&mut self, ui: &mut Ui, fontes: &[Arc<dyn FonteDeColunas>]) -> Option<PedidoDaJanela> {
        let mut pedido = None;
        bloco(ui, "De onde", None, |ui| {
            // as abas, a área de transferência e o "Outro arquivo" (o último índice)
            let mut lista: Vec<Arc<dyn FonteDeColunas>> = fontes.to_vec();
            for extra in [self.area.clone(), self.outro.clone()].into_iter().flatten() {
                if !lista.iter().any(|f| id_da_fonte(f) == id_da_fonte(&extra)) {
                    lista.push(extra);
                }
            }
            let rotulos: Vec<String> = lista.iter().map(|f| f.rotulo()).collect();
            let mut opcoes: Vec<(usize, &str)> = rotulos.iter().map(String::as_str).enumerate().collect();
            let outro = lista.len();
            opcoes.push((outro, "Outro arquivo"));
            let atual = self.fonte.as_ref().and_then(|f| lista.iter().position(|g| id_da_fonte(g) == id_da_fonte(f)));
            // sem origem, nada escolhido (o "Outro arquivo" também é uma troca)
            let mut escolhida = atual.unwrap_or(usize::MAX);
            let meta = self.fonte.as_ref().map(|f| {
                let linhas = f.linhas().map_or_else(|| "—".to_string(), |n| plural(n, "linha", "linhas"));
                format!("{linhas} · {}", plural(f.nomes().len() as u64, "coluna", "colunas"))
            });
            em_duas_colunas(
                ui,
                ALTURA_CAMPO,
                |ui| {
                    let largura = ui.available_width();
                    let mudou = Select::new("tela_csv::trazer::fonte", &mut escolhida, &opcoes)
                        .largura(largura)
                        .rotulo("De onde")
                        .mostrar(ui)
                        .changed();
                    if mudou {
                        if escolhida == outro {
                            pedido = Some(PedidoDaJanela::OutroArquivo);
                        } else if let Some(f) = lista.get(escolhida) {
                            self.fonte = Some(Arc::clone(f));
                        }
                    }
                },
                None,
                |ui| {
                    if self.abrindo_outro.is_some() {
                        interface::pecas::avisos::carregando(ui, "Lendo o arquivo…");
                    } else if let Some(m) = &meta {
                        meta_t3(ui, m);
                    }
                },
            );
        });
        pedido
    }

    fn bloco_casar(&mut self, ui: &mut Ui, nomes_destino: &[String], linhas_destino: u64) {
        let rotulo_origem = self.fonte.as_ref().map(|f| f.rotulo()).unwrap_or_default();
        bloco(ui, "Casar as linhas onde", None, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let opcoes_d: Vec<(usize, &str)> = nomes_destino.iter().map(String::as_str).enumerate().collect();
            let opcoes_o: Vec<(usize, &str)> = self.nomes_origem.iter().map(String::as_str).enumerate().collect();
            let mut d = self.chave_destino.unwrap_or(usize::MAX);
            let mut o = self.chave_origem.unwrap_or(usize::MAX);
            let rotulo_d = "Coluna deste arquivo".to_string();
            let rotulo_o = format!("Coluna de {rotulo_origem}");
            em_duas_colunas(
                ui,
                ALTURA_CAMPO,
                |ui| {
                    let w = ui.available_width();
                    Select::new("tela_csv::trazer::chave_d", &mut d, &opcoes_d)
                        .largura(w)
                        .rotulo(&rotulo_d)
                        .mostrar(ui);
                },
                Some("="),
                |ui| {
                    let w = ui.available_width();
                    Select::new("tela_csv::trazer::chave_o", &mut o, &opcoes_o)
                        .largura(w)
                        .rotulo(&rotulo_o)
                        .mostrar(ui);
                },
            );
            if d != usize::MAX && Some(d) != self.chave_destino {
                self.chave_destino = Some(d);
                self.cancelado = false;
            }
            if o != usize::MAX && Some(o) != self.chave_origem {
                self.chave_origem = Some(o);
                self.cancelado = false;
            }
            let de_origem = format!("de {rotulo_origem}");
            em_duas_colunas(ui, ALTURA_META, |ui| meta_t3(ui, "deste arquivo"), None, |ui| meta_t3(ui, &de_origem));
            ui.add_space(4.0);
            self.medidor(ui, linhas_destino);
            ui.add_space(4.0);
            self.opcoes_de_casamento(ui);
            if let Some(c) = self.casamento()
                && !c.repetidas.is_empty()
            {
                ui.add_space(4.0);
                let n = c.repetidas.len();
                let lista: Vec<(String, usize)> = c
                    .repetidas
                    .iter()
                    .take(MAX_REPETIDAS_LISTADAS)
                    .map(|r| (r.chave.clone(), r.linhas.len()))
                    .collect();
                let chave = self.chave_origem.and_then(|o| self.nomes_origem.get(o)).cloned().unwrap_or_default();
                self.aviso_repetidas(ui, n, &chave, &rotulo_origem, &lista);
            }
        });
    }

    /// O medidor: "4.812 de 5.000 linhas têm par (96%)" e a barra de progresso (a do Exportar) de ponta a ponta;
    /// enquanto lê ou calcula, a barra anda sozinha e o Cancelar para.
    fn medidor(&mut self, ui: &mut Ui, linhas_destino: u64) {
        let p = paleta(ui);
        let casamento = self.casamento().map(|c| (c.com_par(), c.total(), c.percentual()));
        let calculando = self.lendo_destino.is_some() || self.lendo_origem.is_some() || self.calculando.is_some();
        let mut cancelar = false;
        let mut de_novo = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            match casamento {
                Some((com, total, pct)) if !calculando => {
                    let forte =
                        format!("{} de {} linhas têm par", formatar_numero(com as u64), formatar_numero(total as u64));
                    let g = galley(ui, &forte, fontes::fonte(12.0, Peso::Medio), p.estudio.t);
                    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                    ui.painter().galley(r.min, g, p.estudio.t);
                    let rotulo = format!("{forte} ({pct}%)");
                    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &rotulo));
                    texto_simples(ui, &format!("({pct}%)"), 12.0, p.estudio.t2);
                }
                _ if self.cancelado => {
                    texto_simples(ui, "Cálculo cancelado.", 12.0, p.estudio.t2);
                    if link(ui, "Calcular de novo").clicked() {
                        de_novo = true;
                    }
                }
                _ if self.chave_destino.is_none() || self.chave_origem.is_none() => {
                    texto_simples(ui, "Escolha as duas colunas que casam as linhas.", 12.0, p.estudio.t2);
                }
                _ => {
                    texto_simples(
                        ui,
                        &format!("Casando as {} linhas…", formatar_numero(linhas_destino)),
                        12.0,
                        p.estudio.t2,
                    );
                    if link(ui, "Cancelar o cálculo").clicked() {
                        cancelar = true;
                    }
                }
            }
        });
        let fracao = match casamento {
            Some((com, total, _)) if !calculando && total > 0 => Some(com as f32 / total as f32),
            Some(_) if !calculando => Some(0.0),
            _ if calculando => None,
            _ => Some(0.0),
        };
        let r = barra_progresso(ui, fracao);
        let (com, total) = casamento.map_or((0, 0), |(c, t, _)| (c, t));
        r.widget_info(|| {
            let mut info = WidgetInfo::labeled(WidgetType::ProgressIndicator, true, "Linhas com par");
            info.value = Some(if total > 0 { com as f64 * 100.0 / total as f64 } else { 0.0 });
            info
        });
        if cancelar {
            self.cancelar_calculo();
        }
        if de_novo {
            self.cancelado = false;
        }
    }

    fn opcoes_de_casamento(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            let mut o = self.opcoes;
            etiqueta_de_ligar(ui, "Ignorar maiúsculas", &mut o.ignorar_maiusculas);
            etiqueta_de_ligar(ui, "Ignorar espaços nas pontas", &mut o.ignorar_espacos);
            etiqueta_de_ligar(ui, "Números pelo valor", &mut o.numeros_pelo_valor);
            if o != self.opcoes {
                self.opcoes = o;
                self.cancelado = false;
            }
        });
    }

    fn aviso_repetidas(&mut self, ui: &mut Ui, n: usize, chave: &str, rotulo: &str, lista: &[(String, usize)]) {
        let frase = if n == 1 {
            format!("1 valor de {chave} aparece mais de uma vez em {rotulo}.")
        } else {
            format!("{} valores de {chave} aparecem mais de uma vez em {rotulo}.", formatar_numero(n as u64))
        };
        let opcoes = [
            (ChavesRepetidas::Primeira, "a primeira"),
            (ChavesRepetidas::Ultima, "a última"),
            (ChavesRepetidas::TodasJuntas, "todas, juntas com \"; \""),
        ];
        let mut modo = self.repetidas;
        let mut ver = false;
        let ver_quais = self.ver_quais;
        Aviso::new(false).mostrar(ui, |ui, cores| {
            texto_simples(ui, &frase, 12.0, cores.texto);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                texto_simples(ui, "Usar:", 12.0, cores.texto);
                Select::new("tela_csv::trazer::repetidas", &mut modo, &opcoes)
                    .largura_auto()
                    .pequeno()
                    .rotulo("Qual usar das repetidas")
                    .mostrar(ui);
                ui.add_space(6.0);
                if link(ui, if ver_quais { "Esconder" } else { "Ver quais" }).clicked() {
                    ver = true;
                }
            });
            if ver_quais {
                for (k, n) in lista {
                    texto_simples(ui, &format!("{k} · {n} vezes"), 11.5, cores.texto);
                }
            }
        });
        self.repetidas = modo;
        if ver {
            self.ver_quais = !self.ver_quais;
        }
    }

    fn bloco_trazer(&mut self, ui: &mut Ui, nomes_destino: &[String]) {
        let n = self.escolhas.len().saturating_sub(usize::from(self.chave_origem.is_some()));
        let marcadas = self.marcadas().iter().filter(|c| Some(**c) != self.chave_origem).count();
        let meta = format!("{} de {}", formatar_numero(marcadas as u64), plural(n as u64, "coluna", "colunas"));
        let mut marcar_todas = false;
        bloco(ui, "Trazer", Some((&meta, &mut marcar_todas)), |ui| {
            let p = paleta(ui);
            let largura = ui.available_width();
            let altura = ((n as f32) * (ALTURA_LT + 1.0) + 8.0).clamp(ALTURA_LT + 8.0, ALTURA_LISTA);
            let (caixa, _) = ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
            ui.painter().rect_filled(caixa, 10.0, p.estudio.f);
            ui.painter().rect_stroke(caixa, 10.0, Stroke::new(1.0, p.estudio.fb), StrokeKind::Inside);
            let mut dentro =
                ui.new_child(egui::UiBuilder::new().max_rect(caixa.shrink(4.0)).layout(Layout::top_down(Align::Min)));
            dentro.set_clip_rect(caixa.shrink(1.0).intersect(ui.clip_rect()));
            egui::ScrollArea::vertical().id_salt("tela_csv::trazer::lista").auto_shrink([false, false]).show(
                &mut dentro,
                |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for c in 0..self.escolhas.len() {
                        if Some(c) == self.chave_origem {
                            continue;
                        }
                        self.linha_da_lista(ui, c, nomes_destino);
                    }
                },
            );
        });
        if marcar_todas {
            let chave = self.chave_origem;
            let todas = self.escolhas.iter().enumerate().all(|(i, e)| e.marcada || Some(i) == chave);
            for (i, e) in self.escolhas.iter_mut().enumerate() {
                e.marcada = !todas && Some(i) != chave;
            }
        }
    }

    fn linha_da_lista(&mut self, ui: &mut Ui, c: usize, nomes_destino: &[String]) {
        let p = paleta(ui);
        let largura = ui.available_width();
        let nome = self.nomes_origem.get(c).cloned().unwrap_or_default();
        let tipo = self.tipos_origem.get(&c).copied().unwrap_or(TipoDetectado::Texto);
        let Some(escolha) = self.escolhas.get_mut(c) else { return };
        let (rect, _) = ui.allocate_exact_size(vec2(largura, ALTURA_LT), Sense::hover());
        if escolha.marcada {
            ui.painter().rect_filled(rect, 6.0, p.estudio.selh);
        }
        let mut linha = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("tela_csv::trazer::lt", c))
                .max_rect(Rect::from_min_max(
                    pos2(rect.left() + 10.0, rect.top()),
                    pos2(rect.right() - 12.0, rect.bottom()),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        linha.spacing_mut().item_spacing.x = 8.0;
        let rotulo_caixa = format!("Trazer {nome}");
        let mut m = escolha.marcada;
        let r = Check::new(&mut m, "").rotulo(&rotulo_caixa).mostrar(&mut linha);
        if r.changed() {
            escolha.marcada = m;
        }
        let (rp, _) = linha.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        icones::pintar(linha.painter(), rp, Icone::Tipo(textos::pastilha(tipo)), Color32::WHITE);
        let cor = if escolha.marcada { p.estudio.t } else { p.estudio.t2 };
        let peso = if escolha.marcada { Peso::Medio } else { Peso::Normal };
        let g = galley(&linha, &nome, fontes::fonte(12.0, peso), cor);
        let (rn, _) = linha.allocate_exact_size(g.size(), Sense::hover());
        linha.painter().galley(rn.min, g, cor);
        if !escolha.marcada {
            return;
        }
        // o destino, à direita
        linha.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let atualizar = matches!(escolha.destino, DestinoTrazido::Atualizar { .. });
            match &mut escolha.destino {
                DestinoTrazido::Nova { nome: novo } => {
                    let rotulo = format!("Nome da coluna nova para {nome}");
                    Entrada::new(novo)
                        .pequeno()
                        .largura(LARGURA_NOME)
                        .rotulo(&rotulo)
                        .id_salt(("tela_csv::trazer::nome", c))
                        .mostrar(ui);
                }
                DestinoTrazido::Atualizar { coluna, modo } => {
                    let modos = [
                        (ModoAtualizacao::Substituir, "substituir"),
                        (ModoAtualizacao::SoCelulasVazias, "só as células vazias"),
                    ];
                    let rotulo_modo =
                        format!("Como atualizar {}", nomes_destino.get(*coluna).map_or("", String::as_str));
                    Select::new(("tela_csv::trazer::modo", c), modo, &modos)
                        .largura(LARGURA_MODO)
                        .pequeno()
                        .rotulo(&rotulo_modo)
                        .mostrar(ui);
                    let cols: Vec<(usize, &str)> = nomes_destino.iter().map(String::as_str).enumerate().collect();
                    let rotulo_col = format!("Coluna a atualizar com {nome}");
                    Select::new(("tela_csv::trazer::col", c), coluna, &cols)
                        .largura_auto()
                        .pequeno()
                        .rotulo(&rotulo_col)
                        .mostrar(ui);
                }
            }
            let mut tipo_destino = usize::from(atualizar);
            let opcoes = [(0usize, "nova coluna"), (1usize, "atualizar")];
            let rotulo_dest = format!("Destino de {nome}");
            if Select::new(("tela_csv::trazer::destino", c), &mut tipo_destino, &opcoes)
                .largura(LARGURA_DESTINO)
                .pequeno()
                .rotulo(&rotulo_dest)
                .mostrar(ui)
                .changed()
            {
                escolha.destino = if tipo_destino == 1 {
                    // atualizar a coluna de mesmo nome, se há
                    let coluna =
                        nomes_destino.iter().position(|n| n.trim().eq_ignore_ascii_case(nome.trim())).unwrap_or(0);
                    DestinoTrazido::Atualizar { coluna, modo: ModoAtualizacao::Substituir }
                } else {
                    let rotulo = rotulo_da_origem(&self.fonte.as_ref().map(|f| f.rotulo()).unwrap_or_default());
                    DestinoTrazido::Nova { nome: nome_da_coluna_trazida(&nome, &rotulo, nomes_destino) }
                };
            }
        });
    }

    fn bloco_previa(&mut self, ui: &mut Ui, nome_destino: &str, nomes_destino: &[String]) {
        let meta = format!("as {} primeiras linhas de {nome_destino}", LINHAS_DA_PREVIA);
        let Some((_, linhas)) = self.previa.clone() else { return };
        let Some(d) = self.chave_destino else { return };
        bloco(ui, "Prévia", Some((&meta, &mut false)), |ui| {
            let p = paleta(ui);
            let casamento = self.casamento().cloned();
            // as colunas: #, a chave, a primeira outra deste arquivo e as trazidas
            let outra = (0..nomes_destino.len()).find(|&c| c != d);
            struct Col {
                nome: String,
                valores: Vec<(String, bool, bool)>,
                numerica: bool,
                chave: bool,
                trazida: bool,
            }
            let valor = |l: &Vec<String>, c: usize| l.get(c).cloned().unwrap_or_default();
            let mut cols: Vec<Col> = vec![Col {
                nome: nomes_destino.get(d).cloned().unwrap_or_default(),
                valores: linhas.iter().map(|l| (valor(l, d), false, false)).collect(),
                numerica: false,
                chave: true,
                trazida: false,
            }];
            if let Some(o) = outra {
                cols.push(Col {
                    nome: nomes_destino[o].clone(),
                    valores: linhas.iter().map(|l| (valor(l, o), false, false)).collect(),
                    numerica: false,
                    chave: false,
                    trazida: false,
                });
            }
            for c in self.marcadas() {
                let (Some(orig), Some(e)) = (self.origem.get(&c), self.escolhas.get(c)) else { continue };
                let tipo = self.tipos_origem.get(&c).copied().unwrap_or(TipoDetectado::Texto);
                let trazidos: Vec<Option<String>> = (0..linhas.len())
                    .map(|i| casamento.as_ref().and_then(|k| k.trazer_valor(i, orig.as_slice(), self.repetidas)))
                    .collect();
                match &e.destino {
                    DestinoTrazido::Nova { nome } => cols.push(Col {
                        nome: nome.clone(),
                        valores: trazidos
                            .iter()
                            .map(|t| match t {
                                Some(v) => (v.clone(), true, false),
                                None => ("—".to_string(), false, true),
                            })
                            .collect(),
                        numerica: textos::numerica(tipo),
                        chave: false,
                        trazida: true,
                    }),
                    DestinoTrazido::Atualizar { coluna, modo } => {
                        let atual: Vec<String> = linhas.iter().map(|l| valor(l, *coluna)).collect();
                        let a = tabular::colunas::atualizar_coluna(&atual, &trazidos, *modo);
                        if let Some(existente) = cols.iter_mut().find(|k| !k.chave && k.nome == nomes_destino[*coluna])
                        {
                            existente.trazida = true;
                            for i in &a.alteradas {
                                existente.valores[*i] = (a.valores[*i].clone(), true, false);
                            }
                        } else {
                            cols.push(Col {
                                nome: nomes_destino.get(*coluna).cloned().unwrap_or_default(),
                                valores: a
                                    .valores
                                    .iter()
                                    .enumerate()
                                    .map(|(i, v)| (v.clone(), a.alteradas.contains(&i), false))
                                    .collect(),
                                numerica: false,
                                chave: false,
                                trazida: true,
                            });
                        }
                    }
                }
            }
            let largura = ui.available_width();
            let altura = ALTURA_CAB_PREVIA + ALTURA_LINHA_PREVIA * linhas.len() as f32;
            let (caixa, resp) = ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Prévia"));
            let pintor = ui.painter().with_clip_rect(caixa.intersect(ui.clip_rect()));
            pintor.rect_filled(caixa, 10.0, p.estudio.doc);
            let w_num = 40.0;
            let w_col = ((largura - w_num) / cols.len().max(1) as f32).floor();
            let fio = Stroke::new(1.0, p.estudio.b);
            let cab = Rect::from_min_size(caixa.min, vec2(largura, ALTURA_CAB_PREVIA));
            interface::grade::pintar_fundo_cabecalho(&pintor, p, cab);
            let numero = interface::grade::Th { numerica: true, ..interface::grade::Th::new("#") };
            let mut p3 = *p;
            p3.estudio.t2 = p3.estudio.t3;
            interface::grade::pintar_cabecalho_coluna(
                &pintor,
                &p3,
                Rect::from_min_size(cab.min + vec2(8.0, 0.0), vec2(w_num - 16.0, ALTURA_CAB_PREVIA - 1.0)),
                &numero,
            );
            for (k, col) in cols.iter().enumerate() {
                let x = caixa.left() + w_num + k as f32 * w_col;
                let th = Rect::from_min_size(pos2(x, caixa.top()), vec2(w_col, ALTURA_CAB_PREVIA - 1.0));
                if col.trazida {
                    // `th.tr`: o destaque a 26% sobre o cabeçalho
                    pintor.rect_filled(th, 0.0, p.estudio.acc.gamma_multiply(0.26));
                }
                pintor.vline(x - 0.5, caixa.top()..=caixa.bottom(), fio);
                let conteudo = th.shrink2(vec2(10.0, 0.0));
                let t = interface::grade::Th { numerica: col.numerica, ..interface::grade::Th::new(&col.nome) };
                interface::grade::pintar_cabecalho_coluna(&pintor.with_clip_rect(th), p, conteudo, &t);
            }
            for (i, _) in linhas.iter().enumerate() {
                let y = caixa.top() + ALTURA_CAB_PREVIA + i as f32 * ALTURA_LINHA_PREVIA;
                if i > 0 {
                    pintor.hline(caixa.x_range(), y - 0.5, fio);
                }
                let r_num = Rect::from_min_size(pos2(caixa.left() + 8.0, y), vec2(w_num - 16.0, ALTURA_LINHA_PREVIA));
                interface::grade::pintar_numero_linha(&pintor, p, r_num, &(i + 1).to_string());
                for (k, col) in cols.iter().enumerate() {
                    let x = caixa.left() + w_num + k as f32 * w_col;
                    let cel = Rect::from_min_size(pos2(x, y), vec2(w_col, ALTURA_LINHA_PREVIA - 1.0));
                    let Some((texto, tingida, sem_par)) = col.valores.get(i) else { continue };
                    if *tingida {
                        pintor.rect_filled(cel, 0.0, p.estudio.tint);
                    }
                    let mut pc = *p;
                    if col.chave {
                        pc.fg = p.estudio.acct;
                    }
                    if *sem_par {
                        pc.fg = p.estudio.t3;
                        let g = galley(ui, texto, fontes::fonte(12.0, Peso::Normal), pc.fg);
                        pintor.galley(
                            pos2(cel.center().x - g.size().x / 2.0, cel.center().y - g.size().y / 2.0),
                            g,
                            pc.fg,
                        );
                        continue;
                    }
                    interface::grade::pintar_texto_de_celula(
                        &pintor.with_clip_rect(cel),
                        &pc,
                        cel.shrink2(vec2(10.0, 0.0)),
                        texto,
                        col.numerica,
                    );
                }
            }
            pintor.rect_stroke(caixa, 10.0, Stroke::new(1.0, p.estudio.fb), StrokeKind::Inside);
        });
    }
}

/// Um bloco da janela: o `.tit` em caixa alta e, ao lado, a meta em `--t3` e o "Marcar todas" (quando há).
fn bloco(ui: &mut Ui, titulo: &str, cabecalho: Option<(&str, &mut bool)>, conteudo: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = ESPACO_BLOCO;
        let p = paleta(ui);
        let largura = ui.available_width();
        ui.allocate_ui_with_layout(vec2(largura, 0.0), Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let g = galley_tit(ui, titulo, p.estudio.t3);
            let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
            ui.painter().galley(r.min, g, p.estudio.t3);
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, titulo));
            if let Some((meta, marcar)) = cabecalho {
                meta_t3(ui, meta);
                if titulo == "Trazer" {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Botao::ghost("Marcar todas").pequeno()).clicked() {
                            *marcar = true;
                        }
                    });
                }
            }
        });
        conteudo(ui);
    });
}

/// A meta em `--t3` (11,5px): "5.038 linhas · 9 colunas", "deste arquivo".
fn meta_t3(ui: &mut Ui, texto: &str) {
    let t3 = paleta(ui).estudio.t3;
    texto_simples(ui, texto, 11.5, t3);
}

/// A grade `1fr 20px 1fr` (gap 8) das linhas de chaves e de origem: uma linha de `altura` com as três colunas centradas
/// nela (`align-items:center`).
fn em_duas_colunas(
    ui: &mut Ui,
    altura: f32,
    esquerda: impl FnOnce(&mut Ui),
    meio: Option<&str>,
    direita: impl FnOnce(&mut Ui),
) {
    let largura = ui.available_width();
    let coluna = ((largura - LARGURA_IGUAL - 16.0) / 2.0).floor();
    let (linha, _) = ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
    let celula = |x: f32, w: f32| Rect::from_min_size(pos2(x, linha.top()), vec2(w, linha.height()));
    let layout = Layout::left_to_right(Align::Center);
    let mut esq = ui.new_child(egui::UiBuilder::new().max_rect(celula(linha.left(), coluna)).layout(layout));
    esquerda(&mut esq);
    let r = celula(linha.left() + coluna + 8.0, LARGURA_IGUAL);
    if let Some(m) = meio {
        let p = paleta(ui);
        let g = galley(ui, m, fontes::fonte(12.0, Peso::Medio), p.estudio.t2);
        ui.painter().galley(pos2(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, p.estudio.t2);
    }
    let mut dir = ui.new_child(
        egui::UiBuilder::new().max_rect(celula(linha.left() + coluna + 16.0 + LARGURA_IGUAL, coluna)).layout(layout),
    );
    direita(&mut dir);
}
/// A etiqueta de ligar e desligar (`.tg`: 24px em pílula, `padding:0 11px`; ligada, o ✓ em `--acct` e o `--tint`).
fn etiqueta_de_ligar(ui: &mut Ui, texto: &str, ligado: &mut bool) {
    let p = paleta(ui);
    let e = &p.estudio;
    let fonte = fontes::fonte(12.0, Peso::Normal);
    let g = galley(ui, texto, fonte, if *ligado { e.t } else { e.t2 });
    let icone = if *ligado { 12.0 + 6.0 - 2.0 } else { 0.0 };
    let tamanho = vec2((g.size().x + icone + 22.0).ceil(), 24.0);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::click());
    if resp.clicked() {
        *ligado = !*ligado;
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let raio = rect.height() / 2.0;
        if *ligado {
            painter.rect_filled(rect, raio, e.tint);
            painter.rect_stroke(rect, raio, Stroke::new(1.0, e.acc.gamma_multiply(0.45)), StrokeKind::Inside);
        } else {
            painter.rect_stroke(rect, raio, Stroke::new(1.0, e.fbc), StrokeKind::Inside);
        }
        if resp.hovered() {
            interface::pintura::halo(painter, rect, raio, 12.0, e.brilho);
        }
        let mut x = rect.left() + 11.0;
        if *ligado {
            let r = Rect::from_min_size(pos2(x - 2.0, rect.center().y - 6.0), vec2(12.0, 12.0));
            icones::pintar(painter, r, Icone::Marcado, e.acct);
            x += 12.0 + 4.0;
        }
        let cor = if *ligado || resp.hovered() { e.t } else { e.t2 };
        painter.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, cor);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let marcado = *ligado;
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, marcado, texto));
}
