//! A leitura de uma planilha (decisão 12 da etapa 6): tudo o que fala com o crate `planilha`, sempre numa tarefa
//! ([`Promessa::rodar`]); a thread da interface nunca lê o arquivo.
//!
//! - **Duas `Planilha`**: a de páginas (o [`SessaoDaFolha::leitor`]) e a do índice, uma instância nova que roda
//!   `indexar` numa tarefa própria, com um `AtomicBool` de cancelamento e o `Progresso` publicado num `Arc<Mutex<_>>`.
//!   No fim, a que indexou (com os marcos, a dimensão real, as mesclas e o filtro) **substitui** a de páginas.
//! - **A de páginas vai e volta**: um pedido leva a `Planilha` para a tarefa e ela volta com a página (sem `Mutex` que
//!   a interface tenha de esperar). Um pedido por vez; os blocos são os do núcleo, de [`LINHAS_POR_BLOCO`] linhas
//!   alinhadas ao Excel (1–128, 129–256…), e a janela pede os da vista e um de cada lado.
//! - **Pedido velho**: cada mudança da janela é uma geração nova; a página que chega de uma geração antiga e não cruza
//!   mais a janela é descartada ([`SessaoDaFolha::descartados`]).
//! - **Leitura que falhou** (o XML cortado no meio): o erro fica na grade e os blocos daquele em diante não são pedidos
//!   de novo (os de antes, sim, se saírem da memória).

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use comum::{Codigo, ErroBanco, Resultado};
use egui::Context;
use estado::Promessa;
use planilha::{LINHAS_POR_BLOCO, Linha, MetaFolha, Pagina, Pasta, Planilha, Progresso};

/// Quantos blocos a interface guarda de uma planilha (2.048 linhas): os mais longe da janela saem primeiro.
pub const BLOCOS_GUARDADOS: usize = 16;
/// Quantos blocos seguidos um pedido lê de uma vez.
pub const BLOCOS_POR_PEDIDO: u32 = 3;
/// A barra "Lendo a planilha…" só aparece depois disto (planilha pequena não pisca a barra).
pub const ESPERA_DA_BARRA: Duration = Duration::from_millis(400);

/// O bloco (do núcleo) de uma linha do Excel (a partir de 1).
pub fn bloco_de(linha: u32) -> u32 {
    (linha.max(1) - 1) / LINHAS_POR_BLOCO
}

/// O que o índice descobriu, para a tela (o `planilha::Indice` sem os marcos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumoDoIndice {
    pub linhas_com_dados: u64,
    pub ultima_linha: u32,
    pub colunas: u32,
}

/// Uma página pedida que volta da tarefa com a `Planilha` que a leu.
struct Volta {
    planilha: Planilha,
    pagina: Resultado<Pagina>,
}

struct PedidoEmVoo {
    geracao: u64,
    blocos: RangeInclusive<u32>,
    promessa: Promessa<Volta>,
}

struct TarefaDoIndice {
    promessa: Promessa<Planilha>,
    cancelar: Arc<AtomicBool>,
    progresso: Arc<Mutex<Option<Progresso>>>,
    inicio: Instant,
}

/// O que mudou num passo da sessão (para a tela refazer a grade).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mudancas {
    /// A primeira página chegou (a planilha abriu).
    pub abriu: bool,
    /// Chegou uma página.
    pub pagina: bool,
    /// O índice terminou: o tamanho é o final e as mesclas e o filtro estão completos.
    pub indice: bool,
}

/// Em que pé está a abertura da planilha.
enum Abertura {
    Abrindo(Promessa<(Planilha, Pagina)>),
    Pronta,
    Falhou,
}

/// Uma planilha aberta para leitura (veja o topo do módulo).
pub struct SessaoDaFolha {
    /// A aba na pasta (índice de `Pasta::folhas`).
    pub folha: usize,
    pub nome: String,
    pasta: Pasta,
    abertura: Abertura,
    /// A `Planilha` de páginas, quando está em casa (`None` enquanto um pedido a levou).
    leitor: Option<Planilha>,
    /// O que se sabe da aba: dimensão, colunas e painel logo; mesclas e filtro depois do índice.
    pub meta: MetaFolha,
    blocos: BTreeMap<u32, Vec<Linha>>,
    pedido: Option<PedidoEmVoo>,
    geracao: u64,
    janela: Option<RangeInclusive<u32>>,
    /// Páginas que chegaram de uma janela que já não é a de agora (descartadas).
    pub descartados: u64,
    /// Quantos pedidos de página foram feitos (os testes).
    pub pedidos: u64,
    indice: Option<TarefaDoIndice>,
    /// A `Planilha` indexada esperando a de páginas voltar de um pedido.
    indexada: Option<Planilha>,
    pub resumo: Option<ResumoDoIndice>,
    /// A pessoa cancelou o índice (a planilha fica navegável com o que já foi visto).
    pub indice_cancelado: bool,
    /// As linhas congeladas da grade (os blocos delas ficam sempre).
    fixas: Option<u32>,
    /// O maior número de linha e a maior coluna (+1) com valor que já se viu.
    ultima_vista: u32,
    colunas_vistas: u32,
    /// As linhas que o índice já percorreu, anotadas a cada passo: o maior de todos os desta sessão, para a rolagem não
    /// encolher quando um índice para (cancelado, a planilha no cache, o erro).
    progresso_visto: u32,
    /// O erro de leitura desta planilha (a frase da tela já pronta).
    pub erro: Option<String>,
    /// O primeiro bloco cuja leitura falhou: dele em diante nada mais é pedido (o arquivo não muda sozinho; tentar de
    /// novo a cada quadro só gastaria uma tarefa por quadro). O erro fica na grade.
    falhou_desde: Option<u32>,
}

impl SessaoDaFolha {
    /// Começa a abrir a aba `folha`: uma tarefa faz `pasta.planilha(folha)` e lê o primeiro bloco.
    pub fn abrir(egui: &Context, pasta: &Pasta, folha: usize) -> SessaoDaFolha {
        let nome = pasta.folhas().get(folha).map(|f| f.nome.clone()).unwrap_or_default();
        let p = pasta.clone();
        let promessa = Promessa::rodar(egui, move || {
            let mut planilha = p.planilha(folha)?;
            let pagina = planilha.pagina(1, LINHAS_POR_BLOCO)?;
            Ok((planilha, pagina))
        });
        SessaoDaFolha::com_abertura(pasta, folha, nome, Abertura::Abrindo(promessa))
    }

    /// A sessão de uma planilha já aberta (a abertura do arquivo leu o primeiro bloco junto).
    pub fn ja_aberta(pasta: &Pasta, folha: usize, planilha: Planilha, pagina: Pagina) -> SessaoDaFolha {
        let nome = planilha.nome().to_owned();
        let mut s = SessaoDaFolha::com_abertura(pasta, folha, nome, Abertura::Pronta);
        s.instalar(planilha, pagina);
        s
    }

    /// A sessão de uma planilha que não abriu (o erro já na frase da tela).
    pub fn falhou(pasta: &Pasta, folha: usize, frase: String) -> SessaoDaFolha {
        let nome = pasta.folhas().get(folha).map(|f| f.nome.clone()).unwrap_or_default();
        let mut s = SessaoDaFolha::com_abertura(pasta, folha, nome, Abertura::Falhou);
        s.erro = Some(frase);
        s
    }

    fn com_abertura(pasta: &Pasta, folha: usize, nome: String, abertura: Abertura) -> SessaoDaFolha {
        SessaoDaFolha {
            folha,
            nome,
            pasta: pasta.clone(),
            abertura,
            leitor: None,
            meta: MetaFolha::default(),
            blocos: BTreeMap::new(),
            pedido: None,
            geracao: 0,
            janela: None,
            descartados: 0,
            pedidos: 0,
            indice: None,
            indexada: None,
            resumo: None,
            indice_cancelado: false,
            fixas: None,
            ultima_vista: 0,
            colunas_vistas: 0,
            progresso_visto: 0,
            erro: None,
            falhou_desde: None,
        }
    }

    fn instalar(&mut self, planilha: Planilha, pagina: Pagina) {
        self.meta = planilha.meta().clone();
        self.leitor = Some(planilha);
        self.guardar_pagina(pagina);
        self.abertura = Abertura::Pronta;
    }

    /// A primeira página já chegou.
    pub fn pronta(&self) -> bool {
        matches!(self.abertura, Abertura::Pronta)
    }

    /// Ainda abrindo (a primeira página não chegou).
    pub fn abrindo(&self) -> bool {
        matches!(self.abertura, Abertura::Abrindo(_))
    }

    /// Há tarefa rodando (abrir, página, índice).
    pub fn ocupada(&self) -> bool {
        self.abrindo() || self.pedido.is_some() || self.indice.is_some()
    }

    /// Há um pedido de página em voo.
    pub fn pedindo(&self) -> bool {
        self.pedido.is_some()
    }

    /// O índice está rodando.
    pub fn indexando(&self) -> bool {
        self.indice.is_some()
    }

    /// A `Planilha` que serve as páginas é a que indexou (os testes).
    #[doc(hidden)]
    pub fn leitor_indexado(&self) -> bool {
        self.leitor.as_ref().is_some_and(|p| p.indice().is_some())
    }

    /// O índice já terminou.
    pub fn indexada(&self) -> bool {
        self.resumo.is_some()
    }

    /// O andamento do índice, de 0 a 1, depois de [`ESPERA_DA_BARRA`] (antes disso, `None`: a barra não aparece).
    pub fn fracao_da_barra(&self) -> Option<f32> {
        let t = self.indice.as_ref()?;
        if t.inicio.elapsed() < ESPERA_DA_BARRA {
            return None;
        }
        Some(self.fracao_do_indice().unwrap_or(0.0))
    }

    /// O andamento do índice, de 0 a 1 (sem a espera da barra).
    pub fn fracao_do_indice(&self) -> Option<f32> {
        let t = self.indice.as_ref()?;
        let p = (*t.progresso.lock().unwrap_or_else(|e| e.into_inner()))?;
        Some(if p.bytes_total == 0 {
            0.0
        } else {
            (p.bytes_lidos as f64 / p.bytes_total as f64).clamp(0.0, 1.0) as f32
        })
    }

    /// Quantas linhas o índice já percorreu.
    pub fn linhas_do_progresso(&self) -> u64 {
        self.indice
            .as_ref()
            .and_then(|t| *t.progresso.lock().unwrap_or_else(|e| e.into_inner()))
            .map_or(0, |p| p.linhas)
    }

    /// Quanto tempo a barra espera ainda para aparecer (para a tela pedir uma repintura).
    pub fn espera_da_barra(&self) -> Option<Duration> {
        let t = self.indice.as_ref()?;
        ESPERA_DA_BARRA.checked_sub(t.inicio.elapsed())
    }

    /// (linhas, colunas) da planilha: depois do índice, as reais; antes, o maior entre a dimensão declarada, o que já se
    /// viu e as linhas que o índice já percorreu (a rolagem cresce com o progresso e não encolhe quando o índice para:
    /// cancelado ou com a planilha indo para o cache, ela fica navegável com o que já foi visto).
    pub fn tamanho(&self) -> (u32, u32) {
        if let Some(r) = self.resumo {
            return (r.ultima_linha, r.colunas.max(self.colunas_vistas));
        }
        let (dl, dc) = self.meta.dimensao.map_or((0, 0), |d| (d.linha_final, d.coluna_final + 1));
        (dl.max(self.ultima_vista).max(self.progresso_visto), dc.max(self.colunas_vistas))
    }

    /// Guarda o que o índice de agora já percorreu (a cada passo e antes de ele parar): o tamanho só muda num passo,
    /// junto com a grade, e nunca para menos.
    fn guardar_progresso(&mut self) {
        let agora = u32::try_from(self.linhas_do_progresso()).unwrap_or(u32::MAX);
        self.progresso_visto = self.progresso_visto.max(agora);
    }

    /// A linha do Excel (a partir de 1), se o bloco dela já chegou (as que não existem no arquivo vêm vazias).
    pub fn linha(&self, numero: u32) -> Option<&Linha> {
        let bloco = self.blocos.get(&bloco_de(numero))?;
        let i = ((numero.max(1) - 1) % LINHAS_POR_BLOCO) as usize;
        bloco.get(i).filter(|l| l.numero == numero)
    }

    /// Os blocos guardados (os testes).
    pub fn blocos_guardados(&self) -> Vec<u32> {
        self.blocos.keys().copied().collect()
    }

    /// A planilha que a sessão lê (a mesma `Pasta` da tela).
    pub fn pasta(&self) -> &Pasta {
        &self.pasta
    }

    fn guardar_pagina(&mut self, pagina: Pagina) {
        let mut por_bloco: BTreeMap<u32, Vec<Linha>> = BTreeMap::new();
        for l in pagina.linhas {
            if !l.sem_valores() {
                self.ultima_vista = self.ultima_vista.max(l.numero);
            }
            if let Some(c) = l.celulas.iter().rev().find(|c| !c.valor.vazio()).map(|c| c.coluna) {
                self.colunas_vistas = self.colunas_vistas.max(c + 1);
            }
            por_bloco.entry(bloco_de(l.numero)).or_default().push(l);
        }
        // a página do núcleo não tem buracos: cada bloco vem inteiro (o último da planilha, até a última linha)
        for (b, linhas) in por_bloco {
            self.blocos.insert(b, linhas);
        }
        self.aparar();
    }

    /// Fica com os [`BLOCOS_GUARDADOS`] mais perto da janela.
    fn aparar(&mut self) {
        if self.blocos.len() <= BLOCOS_GUARDADOS {
            return;
        }
        let centro = self.janela.as_ref().map_or(0, |j| (j.start() + j.end()) / 2);
        // os blocos das linhas congeladas ficam sempre (elas estão sempre à vista)
        let fixos = self.fixas.map(bloco_de);
        let mut chaves: Vec<u32> = self.blocos.keys().copied().filter(|b| fixos.is_none_or(|f| *b > f)).collect();
        let guardar = BLOCOS_GUARDADOS.saturating_sub(self.blocos.len() - chaves.len());
        chaves.sort_by_key(|&b| (b.abs_diff(centro), b));
        for b in chaves.into_iter().skip(guardar) {
            self.blocos.remove(&b);
        }
    }

    /// Um passo, a cada quadro (barato quando nada chegou): a abertura, a página e o índice que terminaram.
    pub fn passo(&mut self, egui: &Context) -> Mudancas {
        let mut m = Mudancas::default();
        self.guardar_progresso();
        if let Abertura::Abrindo(p) = &mut self.abertura
            && let Some(r) = p.tomar()
        {
            match r {
                Ok((planilha, pagina)) => {
                    self.instalar(planilha, pagina);
                    m.abriu = true;
                }
                Err(e) => {
                    self.erro = Some(frase_da_folha(&self.nome, &e));
                    self.abertura = Abertura::Falhou;
                }
            }
            egui.request_repaint();
        }
        if let Some(r) = self.pedido.as_mut().and_then(|p| p.promessa.tomar())
            && let Some(pedido) = self.pedido.take()
        {
            match r {
                Ok(Volta { planilha, pagina }) => {
                    self.devolver(planilha);
                    match pagina {
                        Ok(pagina) => {
                            let velho = pedido.geracao != self.geracao
                                && self.janela.as_ref().is_some_and(|j| {
                                    pedido.blocos.end() < j.start() || pedido.blocos.start() > j.end()
                                });
                            if velho {
                                self.descartados += 1;
                            } else {
                                self.guardar_pagina(pagina);
                                m.pagina = true;
                            }
                        }
                        Err(e) => self.falhar_desde(*pedido.blocos.start(), &e),
                    }
                }
                // a tarefa caiu (pânico): a planilha de páginas se perdeu; abre outra no próximo pedido (de um bloco
                // antes deste)
                Err(e) => self.falhar_desde(*pedido.blocos.start(), &e),
            }
            egui.request_repaint();
        }
        if let Some(t) = &mut self.indice
            && let Some(r) = t.promessa.tomar()
        {
            self.guardar_progresso();
            self.indice = None;
            match r {
                Ok(indexada) => {
                    if let Some(i) = indexada.indice() {
                        self.resumo = Some(ResumoDoIndice {
                            linhas_com_dados: i.linhas_com_dados,
                            ultima_linha: i.ultima_linha,
                            colunas: i.colunas,
                        });
                    }
                    self.meta = indexada.meta().clone();
                    if self.leitor.is_some() {
                        let velha = self.leitor.replace(indexada);
                        soltar(velha);
                    } else {
                        self.indexada = Some(indexada);
                    }
                    m.indice = true;
                }
                Err(e) if e.tem_codigo(Codigo::Cancelado) => {}
                Err(e) => self.erro = Some(frase_da_folha(&self.nome, &e)),
            }
            egui.request_repaint();
        }
        m
    }

    /// A leitura a partir do bloco `bloco` falhou: o erro vai para a grade e os blocos dele em diante não são mais
    /// pedidos.
    fn falhar_desde(&mut self, bloco: u32, erro: &ErroBanco) {
        self.erro = Some(frase_da_folha(&self.nome, erro));
        self.falhou_desde = Some(self.falhou_desde.map_or(bloco, |f| f.min(bloco)));
    }

    /// A `Planilha` de páginas volta para casa (ou a indexada toma o lugar dela).
    fn devolver(&mut self, planilha: Planilha) {
        match self.indexada.take() {
            Some(indexada) => {
                soltar(Some(planilha));
                self.leitor = Some(indexada);
            }
            None => self.leitor = Some(planilha),
        }
    }

    /// Começa o índice numa tarefa, com uma `Planilha` nova (a de páginas continua servindo).
    pub fn iniciar_indice(&mut self, egui: &Context) {
        if self.indice.is_some() || self.resumo.is_some() || !self.pronta() {
            return;
        }
        let cancelar = Arc::new(AtomicBool::new(false));
        let progresso = Arc::new(Mutex::new(None));
        let (c, pr, pasta, folha) = (Arc::clone(&cancelar), Arc::clone(&progresso), self.pasta.clone(), self.folha);
        let ctx = egui.clone();
        let promessa = Promessa::rodar(egui, move || {
            let mut p = pasta.planilha(folha)?;
            p.indexar(|andamento| {
                *pr.lock().unwrap_or_else(|e| e.into_inner()) = Some(*andamento);
                ctx.request_repaint();
                !c.load(Ordering::Relaxed)
            })?;
            Ok(p)
        });
        self.indice_cancelado = false;
        self.indice = Some(TarefaDoIndice { promessa, cancelar, progresso, inicio: Instant::now() });
    }

    /// Interrompe o índice (a tarefa termina no próximo marco de 8.192 linhas). `pela_pessoa`: o Cancelar do rodapé (a
    /// planilha não volta a indexar sozinha).
    pub fn cancelar_indice(&mut self, pela_pessoa: bool) {
        self.guardar_progresso();
        if let Some(t) = self.indice.take() {
            t.cancelar.store(true, Ordering::Relaxed);
            // a tarefa termina sozinha; a resposta dela (o 409) não interessa mais
            drop(t);
            if pela_pessoa {
                self.indice_cancelado = true;
            }
        }
    }

    /// A planilha sai da tela (vai para o cache): o índice para, e a `Planilha` de páginas larga o fluxo e o cache dela.
    pub fn liberar(&mut self) {
        self.cancelar_indice(false);
        if let Some(l) = &mut self.leitor {
            l.liberar();
        }
    }

    /// As linhas do Excel `visiveis` estão na tela: pede o bloco que falta mais perto delas (com um de cada lado), se a
    /// `Planilha` de páginas estiver em casa.
    pub fn pedir(&mut self, egui: &Context, visiveis: RangeInclusive<u32>, congeladas: u32) {
        if !self.pronta() {
            return;
        }
        self.fixas = (congeladas > 0).then_some(congeladas);
        let (de, ate) = (bloco_de(*visiveis.start()), bloco_de(*visiveis.end()));
        let ultimo = self.resumo.map(|r| bloco_de(r.ultima_linha.max(1)));
        let janela = de.saturating_sub(1)..=ultimo.map_or(ate + 1, |u| (ate + 1).min(u.max(ate)));
        if self.janela.as_ref() != Some(&janela) {
            self.janela = Some(janela.clone());
            self.geracao += 1;
        }
        if self.pedido.is_some() {
            return;
        }
        // da vista para fora: os da vista primeiro, depois os vizinhos
        let fixos = self.fixas.map_or(0..=0, |f| 0..=bloco_de(f));
        let mut faltam: Vec<u32> = if self.fixas.is_some() { fixos.clone().collect() } else { Vec::new() };
        faltam.extend(janela.clone());
        // nem os que já estão, nem os que vêm depois de uma leitura que falhou
        faltam.retain(|b| !self.blocos.contains_key(b) && self.falhou_desde.is_none_or(|f| *b < f));
        faltam.dedup();
        if faltam.is_empty() {
            return;
        }
        faltam
            .sort_by_key(|&b| if (de..=ate).contains(&b) || fixos.contains(&b) { (0, b) } else { (1, b.abs_diff(de)) });
        let primeiro = faltam[0];
        let mut fim = primeiro;
        while fim + 1 - primeiro < BLOCOS_POR_PEDIDO && faltam.contains(&(fim + 1)) {
            fim += 1;
        }
        let planilha = match self.leitor.take() {
            Some(p) => p,
            None => match self.pasta.planilha(self.folha) {
                // a de páginas se perdeu numa tarefa que caiu: uma nova (abrir é só o cabeçalho do XML)
                Ok(p) => p,
                // nem abrir deu certo: nada mais é pedido (senão a thread da interface tentaria a cada quadro)
                Err(e) => {
                    self.falhar_desde(0, &e);
                    return;
                }
            },
        };
        let (linha, quantidade) = (primeiro * LINHAS_POR_BLOCO + 1, (fim - primeiro + 1) * LINHAS_POR_BLOCO);
        let promessa = Promessa::rodar(egui, move || {
            let mut planilha = planilha;
            let pagina = planilha.pagina(linha, quantidade);
            Ok(Volta { planilha, pagina })
        });
        self.pedidos += 1;
        self.pedido = Some(PedidoEmVoo { geracao: self.geracao, blocos: primeiro..=fim, promessa });
    }
}

impl Drop for SessaoDaFolha {
    fn drop(&mut self) {
        self.cancelar_indice(false);
        soltar(self.leitor.take());
        soltar(self.indexada.take());
    }
}

/// Larga uma `Planilha` numa thread à parte: o fluxo dela espera a thread de descompressão terminar.
fn soltar(planilha: Option<Planilha>) {
    if let Some(p) = planilha {
        let _ = std::thread::Builder::new().name("planilha-soltar".into()).spawn(move || drop(p));
    }
}

/// "Não foi possível ler a planilha Vendas: …" (o erro de uma planilha só, dentro da grade dela).
pub fn frase_da_folha(nome: &str, erro: &ErroBanco) -> String {
    let motivo = erro.mensagem.trim_end_matches('.');
    let mut chars = motivo.chars();
    let motivo = match chars.next() {
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };
    format!("Não foi possível ler a planilha {nome}: {motivo}.")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn blocos_alinhados_ao_excel() {
        assert_eq!(bloco_de(1), 0);
        assert_eq!(bloco_de(128), 0);
        assert_eq!(bloco_de(129), 1);
        assert_eq!(bloco_de(0), 0);
    }

    #[test]
    fn frase_de_uma_planilha_que_nao_abriu() {
        let e = ErroBanco::novo(
            "A planilha está corrompida: o conteúdo de «xl/worksheets/sheet2.xml» não é um XML válido.",
        );
        assert_eq!(
            frase_da_folha("Custos", &e),
            "Não foi possível ler a planilha Custos: a planilha está corrompida: o conteúdo de «xl/worksheets/sheet2.xml» \
             não é um XML válido."
        );
    }
}
