//! A única parte que fala com o `tabular`: a [`Tabela`] mora num
//! `Arc<Mutex<…>>` e só é tocada dentro de tarefas de fundo (`Promessa::rodar`). A grade pinta de cópias: o
//! [`Retrato`] (o que a tabela é agora: linhas, nomes, tipos, formato, alterações) e a [`Pagina`] (as linhas à
//! vista). Cada tarefa devolve uma [`Resposta`] com o retrato e a página já relidos, e a tela troca as cópias.
//!
//! As leituras longas (a coluna inteira do Copiar coluna, a chave do Trazer colunas, o filtro) não seguram o
//! `Mutex`: [`instantaneo`] tira, com ele travado por um instante, um leitor próprio do arquivo
//! (`Leitor::reabrir_para_leitura`) e uma cópia das alterações, e [`ler_colunas`] passa pelo arquivo com eles (o
//! `Leitor::varrer`). A grade continua lendo páginas enquanto isso.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use comum::{Codigo, ErroBanco, Resultado};
use tabular::csv::{ChaveLinha, Edicoes, Formato, Leitor, OpcoesAbertura, ResumoSalvar, Tabela};
use tabular::numero::TipoDetectado;

/// A tabela de um documento, compartilhada com as tarefas (e com a fonte do Trazer colunas de outra aba).
pub(crate) type TabelaCompartilhada = Arc<Mutex<Tabela>>;

/// Trava a tabela (um pânico numa tarefa não a deixa inutilizável: a tabela continua como estava).
pub(crate) fn travar(t: &TabelaCompartilhada) -> MutexGuard<'_, Tabela> {
    t.lock().unwrap_or_else(|e| e.into_inner())
}

/// Quantas linhas do começo a pastilha de tipo do cabeçalho examina (`tipos_das_colunas`).
pub(crate) const AMOSTRA_DOS_TIPOS: usize = 2000;

/// O erro de quem cancelou (o Cancelar do carregamento, da gravação, do cálculo da janela).
pub(crate) fn erro_cancelado() -> ErroBanco {
    ErroBanco::novo("Cancelado.").codigo(Codigo::Cancelado)
}

/// O progresso e o cancelamento de uma tarefa longa, vistos das duas threads.
#[derive(Debug, Default)]
pub(crate) struct Controle {
    feito: AtomicU64,
    total: AtomicU64,
    cancelar: AtomicBool,
}

impl Controle {
    pub(crate) fn novo() -> Arc<Controle> {
        Arc::new(Controle::default())
    }

    /// O `ao_progresso` do núcleo: guarda o andamento e, depois do Cancelar, devolve o erro que interrompe.
    pub(crate) fn avisar(&self, feito: u64, total: u64) -> Resultado<()> {
        self.feito.store(feito, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        if self.cancelar.load(Ordering::Relaxed) { Err(erro_cancelado()) } else { Ok(()) }
    }

    /// A fração feita, se a tarefa já disse o total.
    pub(crate) fn fracao(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.feito.load(Ordering::Relaxed) as f64 / total as f64).clamp(0.0, 1.0) as f32)
    }

    pub(crate) fn cancelar(&self) {
        self.cancelar.store(true, Ordering::Relaxed);
    }
}

/// O que a tabela é agora (uma cópia barata, sem as linhas): o que o cabeçalho, o rodapé, as Propriedades, as
/// ofertas e a barra de status mostram.
#[derive(Debug, Clone, PartialEq)]
pub struct Retrato {
    /// Linhas de dados (sem o cabeçalho).
    pub linhas: u64,
    pub nomes: Vec<String>,
    /// O tipo de cada coluna pelas primeiras [`AMOSTRA_DOS_TIPOS`] linhas (a pastilha do cabeçalho).
    pub tipos: Vec<TipoDetectado>,
    pub formato: Formato,
    /// Os passos do desfazer desde a abertura ou o último Salvar ("1 alteração não salva").
    pub alteracoes: usize,
    pub alterada: bool,
    pub pode_desfazer: bool,
    pub pode_refazer: bool,
    /// Os passos do Refazer (os rótulos do refazer andam em paralelo com eles).
    pub passos_a_refazer: usize,
    /// O tamanho e a data do arquivo quando foi lido (ou gravado) pela última vez.
    pub tamanho: u64,
    pub modificado: Option<SystemTime>,
    pub caminho: PathBuf,
}

impl Retrato {
    pub fn colunas(&self) -> usize {
        self.nomes.len()
    }
}

/// Um pedaço das linhas que a grade mostra: as linhas `inicio..inicio + linhas.len()` **da vista** (sem filtro nem
/// ordem, a vista é a própria tabela), com a posição de cada uma na tabela e as marcas de alteração (`td.alt`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pagina {
    pub inicio: u64,
    /// A posição na tabela (de 0) de cada linha da página: o número da linha na grade é ela mais 1.
    pub posicoes: Vec<u64>,
    pub linhas: Vec<Vec<String>>,
    pub marcas: Vec<Vec<bool>>,
}

impl Pagina {
    pub fn contem(&self, linha: u64) -> bool {
        linha >= self.inicio && linha < self.inicio + self.linhas.len() as u64
    }

    fn indice(&self, linha: u64) -> Option<usize> {
        self.contem(linha).then(|| (linha - self.inicio) as usize)
    }

    /// A linha `linha` da vista, se está na página.
    pub fn linha(&self, linha: u64) -> Option<&[String]> {
        self.indice(linha).and_then(|i| self.linhas.get(i)).map(Vec::as_slice)
    }

    pub fn celula(&self, linha: u64, coluna: usize) -> Option<&str> {
        self.linha(linha).and_then(|l| l.get(coluna)).map(String::as_str)
    }

    pub fn posicao(&self, linha: u64) -> Option<u64> {
        self.indice(linha).and_then(|i| self.posicoes.get(i)).copied()
    }

    pub fn alterada(&self, linha: u64, coluna: usize) -> bool {
        self.indice(linha).and_then(|i| self.marcas.get(i)).and_then(|m| m.get(coluna)).copied().unwrap_or(false)
    }
}

/// O pedaço da vista que a grade quer à vista: `quantidade` linhas a partir de `inicio`, e a vista (as posições
/// das linhas filtradas ou ordenadas; `None`: a tabela na ordem do arquivo).
#[derive(Debug, Clone, Default)]
pub(crate) struct Janela {
    pub inicio: u64,
    pub quantidade: usize,
    pub vista: Option<Arc<Vec<u64>>>,
}

/// Quantas linhas a página lida tem (a grade mostra umas 30; o resto é a folga para rolar sem esperar).
pub(crate) const LINHAS_DA_PAGINA: usize = 240;

impl Janela {
    pub(crate) fn inicial() -> Janela {
        Janela { inicio: 0, quantidade: LINHAS_DA_PAGINA, vista: None }
    }
}

/// O retrato da tabela agora (os tipos leem as primeiras linhas; o resto é cópia do que ela já sabe).
pub(crate) fn retrato(t: &mut Tabela) -> Resultado<Retrato> {
    let tipos = t.tipos_das_colunas(AMOSTRA_DOS_TIPOS)?;
    Ok(Retrato {
        linhas: t.linhas(),
        nomes: t.nomes(),
        tipos,
        formato: t.formato().clone(),
        alteracoes: t.alteracoes(),
        alterada: t.alterada(),
        pode_desfazer: t.pode_desfazer(),
        pode_refazer: t.pode_refazer(),
        passos_a_refazer: t.passos_a_refazer(),
        tamanho: t.leitor().tamanho(),
        modificado: t.leitor().modificado(),
        caminho: t.caminho().to_path_buf(),
    })
}

/// Lê a página da janela: sem vista, um pedaço contínuo da tabela; com vista, as posições dela, juntando as que
/// seguem uma à outra numa leitura só.
pub(crate) fn ler_pagina(t: &mut Tabela, janela: &Janela) -> Resultado<Pagina> {
    let posicoes: Vec<u64> = match &janela.vista {
        None => {
            let fim = (janela.inicio + janela.quantidade as u64).min(t.linhas());
            (janela.inicio.min(fim)..fim).collect()
        }
        Some(v) => {
            let inicio = usize::try_from(janela.inicio).unwrap_or(usize::MAX).min(v.len());
            let fim = inicio.saturating_add(janela.quantidade).min(v.len());
            v[inicio..fim].iter().copied().filter(|&p| p < t.linhas()).collect()
        }
    };
    let mut linhas = Vec::with_capacity(posicoes.len());
    let mut marcas = Vec::with_capacity(posicoes.len());
    let mut i = 0;
    while i < posicoes.len() {
        let mut j = i + 1;
        while j < posicoes.len() && posicoes[j] == posicoes[i] + (j - i) as u64 {
            j += 1;
        }
        let n = j - i;
        linhas.extend(t.pagina(posicoes[i], n)?);
        marcas.extend(t.marcas_da_pagina(posicoes[i], n));
        i = j;
    }
    let n = linhas.len().min(marcas.len());
    linhas.truncate(n);
    marcas.truncate(n);
    let mut posicoes = posicoes;
    posicoes.truncate(n);
    Ok(Pagina { inicio: janela.inicio, posicoes, linhas, marcas })
}

/// O que a tarefa de um documento faz na tabela (uma por vez, na ordem: a fila do documento).
#[derive(Debug, Clone)]
pub(crate) enum Trabalho {
    /// Só reler a página da janela (a grade rolou).
    Pagina,
    /// Ler de novo com outras opções (Separador, Decimal, Codificação, Fim de linha): só sem alterações.
    Reabrir(OpcoesAbertura),
    /// "Primeira linha é o cabeçalho": só sem alterações.
    Cabecalho(bool),
    /// Ler o arquivo de novo descartando as alterações (o Atualizar com a pergunta, o "Ler de novo").
    Recarregar(OpcoesAbertura),
    /// O arquivo mudou no disco desde que foi lido? (a volta do foco)
    ConferirDisco,
    /// Uma alteração (um passo no desfazer, ou nenhum se nada mudou).
    Mutacao(crate::edicao::Mutacao),
    /// Gravar no arquivo (antes confere se outro programa mudou o arquivo: aí não grava).
    Salvar,
    /// As linhas inteiras destas posições (o Ctrl+C das linhas marcadas), com o cabeçalho na frente.
    LerLinhas(Vec<u64>),
    /// Um intervalo da grade (o Ctrl+C de um intervalo que passa da página em mãos): destas posições, só estas
    /// colunas da tabela, sem o cabeçalho (None: a coluna provisória, que vai vazia).
    LerIntervalo { posicoes: Vec<u64>, colunas: Vec<Option<usize>> },
}

/// O que a tarefa fez, além de reler o retrato e a página.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Efeito {
    Nada,
    /// A resposta do [`Trabalho::ConferirDisco`].
    Disco(bool),
    /// A mutação mudou a tabela (`false`: o texto já era esse, nada entrou no desfazer).
    Alterou(bool),
    Salvo(ResumoSalvar),
    /// O Salvar não gravou: o arquivo mudou no disco (o núcleo não tem mais as linhas de antes para regravar).
    MudouNoDisco,
    /// As linhas pedidas (a primeira é o cabeçalho).
    Linhas(Vec<Vec<String>>),
    /// As células do intervalo pedido, linha a linha.
    Intervalo(Vec<Vec<String>>),
}

/// Quantas células uma cópia leva, no máximo.
pub(crate) const LIMITE_DA_COPIA: u64 = 5_000_000;

/// A resposta de uma tarefa: o retrato e a página relidos (nada, quando o arquivo mudou no disco) e o efeito.
#[derive(Debug, Clone)]
pub(crate) struct Resposta {
    pub retrato: Option<Retrato>,
    pub pagina: Option<Pagina>,
    pub efeito: Efeito,
}

/// Roda um trabalho na tabela (dentro da tarefa) e relê o que a grade mostra.
pub(crate) fn executar(
    tabela: &TabelaCompartilhada,
    trabalho: Trabalho,
    janela: &Janela,
    controle: &Controle,
) -> Resultado<Resposta> {
    let mut t = travar(tabela);
    let mut progresso = |f, total| controle.avisar(f, total);
    // os que não mudam a tabela não releem o retrato (os tipos leem as primeiras 2.000 linhas a cada vez): a tela
    // fica com o que tem
    let mesmo_retrato = matches!(
        trabalho,
        Trabalho::Pagina | Trabalho::ConferirDisco | Trabalho::LerLinhas(_) | Trabalho::LerIntervalo { .. }
    );
    let efeito = match trabalho {
        Trabalho::Pagina => Efeito::Nada,
        Trabalho::Reabrir(opcoes) => {
            t.reabrir(&opcoes, &mut progresso)?;
            Efeito::Nada
        }
        Trabalho::Cabecalho(sim) => {
            t.definir_cabecalho(sim)?;
            Efeito::Nada
        }
        Trabalho::Recarregar(opcoes) => {
            let caminho = t.caminho().to_path_buf();
            *t = Tabela::abrir_com_progresso(&caminho, &opcoes, &mut progresso)?;
            Efeito::Nada
        }
        Trabalho::ConferirDisco => Efeito::Disco(t.leitor().mudou_no_disco()),
        Trabalho::Mutacao(m) => Efeito::Alterou(crate::edicao::aplicar(&mut t, m)?),
        Trabalho::Salvar => {
            if t.alterada() && t.leitor().mudou_no_disco() {
                Efeito::MudouNoDisco
            } else {
                Efeito::Salvo(t.salvar(&mut progresso)?)
            }
        }
        Trabalho::LerLinhas(posicoes) => {
            let mut linhas = Vec::with_capacity(posicoes.len() + 1);
            linhas.push(t.nomes());
            for (i, &p) in posicoes.iter().enumerate() {
                if let Some(l) = t.pagina(p, 1)?.into_iter().next() {
                    linhas.push(l);
                }
                controle.avisar(i as u64, posicoes.len() as u64)?;
            }
            Efeito::Linhas(linhas)
        }
        Trabalho::LerIntervalo { posicoes, colunas } => {
            let mut linhas = Vec::with_capacity(posicoes.len());
            for (i, &p) in posicoes.iter().enumerate() {
                let linha = t.pagina(p, 1)?.into_iter().next().unwrap_or_default();
                linhas
                    .push(colunas.iter().map(|c| c.and_then(|c| linha.get(c).cloned()).unwrap_or_default()).collect());
                controle.avisar(i as u64, posicoes.len() as u64)?;
            }
            Efeito::Intervalo(linhas)
        }
    };
    // o arquivo mudou por fora: as leituras recusam (409) até reler, e a tela fica com o que já mostrava
    if matches!(efeito, Efeito::MudouNoDisco | Efeito::Disco(true)) {
        return Ok(Resposta { retrato: None, pagina: None, efeito });
    }
    let retrato = if mesmo_retrato { None } else { Some(retrato(&mut t)?) };
    let pagina = ler_pagina(&mut t, janela)?;
    Ok(Resposta { retrato, pagina: Some(pagina), efeito })
}

/// Um leitor próprio do arquivo e a cópia das alterações: para ler colunas inteiras sem segurar a tabela.
pub(crate) struct Instantaneo {
    leitor: Leitor,
    edicoes: Edicoes,
}

/// Tira o [`Instantaneo`] da tabela (com o `Mutex` travado só aqui).
pub(crate) fn instantaneo(tabela: &TabelaCompartilhada) -> Resultado<Instantaneo> {
    let t = travar(tabela);
    Ok(Instantaneo { leitor: t.leitor().reabrir_para_leitura()?, edicoes: t.edicoes().clone() })
}

impl Instantaneo {
    /// As linhas da tabela no instantâneo.
    pub(crate) fn linhas(&self) -> u64 {
        self.edicoes.total_linhas()
    }

    /// As colunas pedidas (pelo número na grade) de todas as linhas, na ordem da tabela, com as alterações: o
    /// mesmo que `Tabela::colunas_de`, numa passada só pelo arquivo (`Leitor::varrer`).
    pub(crate) fn colunas(
        &mut self,
        indices: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let colunas = self.edicoes.colunas();
        let mut origens = Vec::new();
        let mut posicao_na_origem = Vec::with_capacity(indices.len());
        let mut ids = Vec::with_capacity(indices.len());
        for &c in indices {
            let coluna = colunas.get(c).ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", c + 1)))?;
            ids.push(coluna.id);
            posicao_na_origem.push(coluna.origem.map(|o| {
                origens.push(o);
                origens.len() - 1
            }));
        }
        let mut base = if origens.is_empty() { Vec::new() } else { self.leitor.colunas_de(&origens, ao_progresso)? };
        let total = usize::try_from(self.edicoes.total_linhas()).unwrap_or(0);
        let mut saida: Vec<Vec<String>> = indices.iter().map(|_| Vec::with_capacity(total)).collect();
        let tem_alteracoes = self.edicoes.tem_celulas_alteradas();
        for chave in self.edicoes.percorrer_chaves() {
            for (k, coluna) in saida.iter_mut().enumerate() {
                let mut valor = match (chave, posicao_na_origem[k]) {
                    (ChaveLinha::Base(r), Some(p)) => {
                        usize::try_from(r).ok().and_then(|r| base[p].get_mut(r)).map(std::mem::take).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                if tem_alteracoes && let Some(v) = self.edicoes.celula_editada(chave, ids[k]) {
                    valor = v.to_string();
                }
                coluna.push(valor);
            }
        }
        Ok(saida)
    }

    /// Os valores da coluna `c` (pelo número na grade), um por vez, com as alterações, sem juntá-los: o
    /// `tabular::csv::percorrer_coluna` com o leitor e as alterações do instantâneo.
    pub(crate) fn percorrer_coluna(
        &mut self,
        c: usize,
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
        quando_um_valor: impl FnMut(&str) -> Resultado<bool>,
    ) -> Resultado<()> {
        tabular::csv::percorrer_coluna(&mut self.leitor, &self.edicoes, c, ao_progresso, quando_um_valor)
    }
}

/// [`Instantaneo::colunas`] de uma tabela compartilhada: o `Mutex` fica travado só para tirar o instantâneo.
pub(crate) fn ler_colunas(
    tabela: &TabelaCompartilhada,
    indices: &[usize],
    ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
) -> Resultado<Vec<Vec<String>>> {
    instantaneo(tabela)?.colunas(indices, ao_progresso)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tabela(conteudo: &str, nome: &str) -> (PathBuf, TabelaCompartilhada) {
        let pasta = std::env::temp_dir().join(format!("tela_csv-sessao-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let caminho = pasta.join("t.csv");
        std::fs::write(&caminho, conteudo).expect("gravar");
        let t = Tabela::abrir(&caminho, &OpcoesAbertura::default()).expect("abrir");
        (caminho, Arc::new(Mutex::new(t)))
    }

    #[test]
    fn pagina_com_vista_junta_as_posicoes_seguidas() {
        let (_c, t) = tabela("a;b\n1;x\n2;y\n3;z\n4;w\n5;v\n", "vista");
        let vista = Arc::new(vec![4, 0, 1, 2]);
        let janela = Janela { inicio: 1, quantidade: 10, vista: Some(vista) };
        let p = ler_pagina(&mut travar(&t), &janela).expect("página");
        assert_eq!(p.posicoes, [0, 1, 2]);
        assert_eq!(p.linhas, [vec!["1", "x"], vec!["2", "y"], vec!["3", "z"]]);
        assert_eq!(p.celula(1, 1), Some("x"));
        assert_eq!(p.posicao(3), Some(2));
        assert!(!p.contem(0) && !p.contem(4));
    }

    #[test]
    fn colunas_do_instantaneo_trazem_as_alteracoes_sem_segurar_a_tabela() {
        let (_c, t) = tabela("a;b\n1;x\n2;y\n", "instantaneo");
        {
            let mut g = travar(&t);
            g.editar_celula(1, 1, "Y").expect("editar");
            g.inserir_linha(0).expect("inserir");
        }
        let mut inst = instantaneo(&t).expect("instantâneo");
        // a tabela está livre durante a leitura
        drop(travar(&t));
        let cols = inst.colunas(&[1, 0], &mut |_, _| Ok(())).expect("ler");
        assert_eq!(cols, [vec!["", "x", "Y"], vec!["", "1", "2"]]);
    }

    #[test]
    fn controle_cancela_pelo_progresso() {
        let c = Controle::novo();
        assert!(c.avisar(5, 10).is_ok());
        assert_eq!(c.fracao(), Some(0.5));
        c.cancelar();
        assert!(c.avisar(6, 10).expect_err("cancelado").tem_codigo(Codigo::Cancelado));
    }
}
