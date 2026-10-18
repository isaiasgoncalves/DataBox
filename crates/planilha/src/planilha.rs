//! Leitura paginada de uma aba grande: um índice de posições de linha e um cache de páginas.
//!
//! O XML de uma aba não tem acesso direto (é um fluxo comprimido). O índice resolve isso com uma passada só,
//! rápida e sem copiar texto: a cada [`LINHAS_POR_MARCO`] linhas anota o número da linha e o deslocamento
//! (em bytes do XML descomprimido) do `<row`. Para mostrar uma página, a leitura salta ao marco mais próximo
//! antes dela (descartando bytes, sem interpretar) e só então lê as linhas de verdade. Um cursor continua de onde
//! parou: rolar para baixo página a página nunca relê o começo, só rolar para cima obriga a recomeçar do
//! início do fluxo (e o cache de blocos de [`LINHAS_POR_BLOCO`] linhas segura as páginas recentes).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use comum::Resultado;

use crate::erros;
use crate::fluxo::Fluxo;
use crate::folha::{LeitorLinhas, LinhaLida, MetaFolha};
use crate::pasta::{Folha, Pasta};
use crate::valores::{Linha, Pagina};

/// De quantas em quantas linhas (do arquivo) o índice anota uma posição.
pub const LINHAS_POR_MARCO: u32 = 128;
/// Tamanho do bloco do cache de páginas, em linhas (números de linha do Excel, alinhados: 1–128, 129–256…).
pub const LINHAS_POR_BLOCO: u32 = 128;
/// Quantos blocos o cache guarda por padrão (8.192 linhas).
pub const BLOCOS_NO_CACHE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Marco {
    /// Número (do Excel) da linha que começa neste deslocamento.
    linha: u32,
    deslocamento: u64,
}

/// O resultado da passada de indexação.
#[derive(Debug, Clone, PartialEq)]
pub struct Indice {
    marcos: Vec<Marco>,
    /// Quantas linhas o XML tem (as que existem; buracos de numeração não contam).
    pub linhas_com_dados: u64,
    /// Número da primeira e da última linha que existem (0 se a aba está vazia).
    pub primeira_linha: u32,
    pub ultima_linha: u32,
    /// Colunas usadas, de verdade: a última coluna de qualquer linha, mais um (0 se vazia).
    pub colunas: u32,
    /// Tamanho do XML descomprimido, em bytes.
    pub bytes_xml: u64,
    /// Quanto levou a passada.
    pub tempo: Duration,
}

impl Indice {
    /// O marco a partir do qual dá para ler a linha `alvo`: o último que não passa dela (ou o primeiro).
    fn marco_para(&self, alvo: u32) -> Option<Marco> {
        let n = self.marcos.partition_point(|m| m.linha <= alvo);
        self.marcos.get(n.saturating_sub(1)).copied()
    }

    /// Quantos marcos foram anotados.
    pub fn marcos(&self) -> usize {
        self.marcos.len()
    }
}

/// Andamento da indexação, para a barra de progresso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progresso {
    /// Bytes do XML já percorridos e total (descomprimido).
    pub bytes_lidos: u64,
    pub bytes_total: u64,
    pub linhas: u64,
}

/// O cursor aberto sobre o fluxo: o leitor, a linha já lida mas ainda não entregue e onde estamos.
struct Cursor {
    leitor: LeitorLinhas<Fluxo>,
    pendente: Option<LinhaLida>,
    /// Número da última linha tirada do fluxo e entregue (ou descartada). Linhas até aqui não voltam.
    ultimo_consumido: u32,
}

impl Cursor {
    /// Onde o fluxo está: o `<row` da linha pendente ou o ponto de leitura.
    fn posicao(&self) -> u64 {
        self.pendente.as_ref().map_or_else(|| self.leitor.xml.deslocamento(), |p| p.deslocamento)
    }
}

/// Uma aba aberta para leitura paginada.
///
/// ```text
/// let pasta = Pasta::abrir("dados.xlsx")?;
/// let mut aba = pasta.planilha(0)?;
/// aba.indexar(|p| { barra(p.bytes_lidos, p.bytes_total); true })?;   // true continua, false interrompe
/// let (linhas, colunas) = aba.tamanho();
/// let pagina = aba.pagina(1, 100)?;                                  // linhas 1 a 100
/// ```
pub struct Planilha {
    pasta: Pasta,
    folha: usize,
    meta: MetaFolha,
    indice: Option<Indice>,
    cursor: Option<Cursor>,
    /// Blocos do cache, do mais recente ao mais antigo: (número do bloco, as linhas que existem nele).
    cache: Vec<(u32, Vec<Linha>)>,
    capacidade: usize,
    /// Quantas vezes o fluxo foi aberto de novo (testes e diagnóstico).
    aberturas: u32,
}

impl Planilha {
    pub(crate) fn nova(pasta: &Pasta, folha: usize) -> Resultado<Planilha> {
        let f = pasta.folha_com_celulas(folha)?;
        let mut leitor = LeitorLinhas::novo(abrir_fluxo(pasta, f));
        leitor.ler_cabecalho().map_err(|e| erros::da_leitura(e, &f.parte))?;
        Ok(Planilha {
            pasta: pasta.clone(),
            folha,
            meta: leitor.meta.clone(),
            indice: None,
            cursor: None,
            cache: Vec::new(),
            capacidade: BLOCOS_NO_CACHE,
            aberturas: 0,
        })
    }

    pub fn pasta(&self) -> &Pasta {
        &self.pasta
    }

    /// A aba na pasta (índice a partir de 0).
    pub fn indice_da_folha(&self) -> usize {
        self.folha
    }

    // `Planilha::nova` só nasce de uma aba que `folha_com_celulas` conferiu, e a lista de abas da pasta não muda
    // depois de aberta.
    #[allow(clippy::indexing_slicing)]
    pub fn folha(&self) -> &Folha {
        &self.pasta.interno.folhas[self.folha]
    }

    pub fn nome(&self) -> &str {
        &self.folha().nome
    }

    /// Informações da aba. Dimensão, colunas e painel vêm logo; células mescladas e filtro só depois de
    /// [`Planilha::indexar`] (ficam no fim do XML).
    pub fn meta(&self) -> &MetaFolha {
        &self.meta
    }

    /// O índice, se [`Planilha::indexar`] já rodou.
    pub fn indice(&self) -> Option<&Indice> {
        self.indice.as_ref()
    }

    /// (linhas, colunas) da aba: as reais, depois de indexar; antes, as da dimensão declarada (ou 0).
    pub fn tamanho(&self) -> (u32, u32) {
        match &self.indice {
            Some(i) => (i.ultima_linha, i.colunas),
            None => self.meta.dimensao.map_or((0, 0), |d| (d.linha_final, d.coluna_final + 1)),
        }
    }

    /// Quantos blocos o cache guarda (mínimo 1; cresce sozinho para caber uma página grande).
    pub fn definir_cache(&mut self, blocos: usize) {
        self.capacidade = blocos.max(1);
        self.cache.truncate(self.capacidade);
    }

    /// Larga o fluxo aberto (a linha de execução que descomprime) e o cache; o índice fica. Útil para a aba
    /// que saiu da tela.
    pub fn liberar(&mut self) {
        self.cursor = None;
        self.cache.clear();
    }

    /// Quantas vezes o fluxo do XML foi aberto do início (cada recomeço custa descomprimir até o ponto).
    pub fn aberturas_do_fluxo(&self) -> u32 {
        self.aberturas
    }

    /// Passa pela aba toda uma vez, sem copiar texto: anota as posições, a última linha, as colunas usadas, as
    /// células mescladas e o filtro. `progresso` é chamado a cada 8.192 linhas e devolve `false` para
    /// interromper (erro 409 `Codigo::Cancelado`).
    pub fn indexar(&mut self, mut progresso: impl FnMut(&Progresso) -> bool) -> Resultado<&Indice> {
        let inicio = Instant::now();
        let f = self.pasta.folha_com_celulas(self.folha)?;
        let parte = f.parte.clone();
        let bytes_total = f.tamanho_xml;
        let mut leitor = LeitorLinhas::novo(abrir_fluxo(&self.pasta, f));
        leitor.modo_rapido();
        let mut indice = Indice {
            marcos: Vec::new(),
            linhas_com_dados: 0,
            primeira_linha: 0,
            ultima_linha: 0,
            colunas: 0,
            bytes_xml: 0,
            tempo: Duration::ZERO,
        };
        while let Some(lida) = leitor.proxima(None).map_err(|e| erros::da_leitura(e, &parte))? {
            if indice.linhas_com_dados.is_multiple_of(u64::from(LINHAS_POR_MARCO)) {
                indice.marcos.push(Marco { linha: lida.linha.numero, deslocamento: lida.deslocamento });
            }
            if indice.linhas_com_dados == 0 {
                indice.primeira_linha = lida.linha.numero;
            }
            indice.ultima_linha = lida.linha.numero;
            indice.linhas_com_dados += 1;
            if let Some(c) = lida.ultima_coluna {
                indice.colunas = indice.colunas.max(c + 1);
            }
            if indice.linhas_com_dados.is_multiple_of(8192) {
                let p = Progresso {
                    bytes_lidos: lida.deslocamento,
                    bytes_total: bytes_total.max(lida.deslocamento),
                    linhas: indice.linhas_com_dados,
                };
                if !progresso(&p) {
                    return Err(erros::interrompido());
                }
            }
        }
        indice.bytes_xml = leitor.xml.deslocamento();
        indice.tempo = inicio.elapsed();
        let final_ =
            Progresso { bytes_lidos: indice.bytes_xml, bytes_total: indice.bytes_xml, linhas: indice.linhas_com_dados };
        progresso(&final_);
        self.meta = leitor.meta;
        // o cursor e o cache de antes não sabiam do índice: recomeça limpo
        self.cursor = None;
        self.cache.clear();
        Ok(self.indice.insert(indice))
    }

    /// As linhas `primeira_linha` a `primeira_linha + quantidade - 1` (números do Excel, a partir de 1), sem
    /// buracos: linha que não existe no XML vem vazia. Com o índice feito, a página é cortada na última linha
    /// da aba; sem ele, tem sempre `quantidade` linhas.
    pub fn pagina(&mut self, primeira_linha: u32, quantidade: u32) -> Resultado<Pagina> {
        let primeira = primeira_linha.max(1);
        let mut ultima = primeira.saturating_add(quantidade).saturating_sub(1);
        if let Some(i) = &self.indice {
            ultima = ultima.min(i.ultima_linha);
        }
        if quantidade == 0 || ultima < primeira {
            return Ok(Pagina { primeira_linha: primeira, linhas: Vec::new() });
        }
        let (b_ini, b_fim) = (bloco_de(primeira), bloco_de(ultima));
        let n_blocos = (b_fim - b_ini + 1) as usize;
        self.capacidade = self.capacidade.max(n_blocos);

        // os blocos que já estão sobem para a frente; os que faltam são lidos em trechos contínuos
        let faltam: Vec<u32> = (b_ini..=b_fim).filter(|&b| !self.tocar(b)).collect();
        for trecho in faltam.chunk_by(|a, b| *b == a + 1) {
            if let (Some(&de), Some(&ate)) = (trecho.first(), trecho.last()) {
                self.carregar(de, ate)?;
            }
        }

        let mut linhas = Vec::with_capacity((ultima - primeira + 1) as usize);
        let mut proxima = primeira;
        for b in b_ini..=b_fim {
            let Some((_, existentes)) = self.cache.iter().find(|(n, _)| *n == b) else {
                continue;
            };
            for l in existentes.iter().filter(|l| l.numero >= primeira && l.numero <= ultima) {
                while proxima < l.numero {
                    linhas.push(Linha::vazia(proxima));
                    proxima += 1;
                }
                linhas.push(l.clone());
                proxima = l.numero + 1;
            }
        }
        while proxima <= ultima {
            linhas.push(Linha::vazia(proxima));
            proxima += 1;
        }
        Ok(Pagina { primeira_linha: primeira, linhas })
    }

    /// Marca o bloco como recém-usado; diz se estava no cache.
    fn tocar(&mut self, bloco: u32) -> bool {
        match self.cache.iter().position(|(n, _)| *n == bloco) {
            Some(p) => {
                let item = self.cache.remove(p);
                self.cache.insert(0, item);
                true
            }
            None => false,
        }
    }

    /// Lê os blocos `b_ini..=b_fim` e os põe no cache.
    fn carregar(&mut self, b_ini: u32, b_fim: u32) -> Resultado<()> {
        let de = b_ini * LINHAS_POR_BLOCO + 1;
        let ate = (b_fim + 1).saturating_mul(LINHAS_POR_BLOCO).saturating_add(1);
        let lidas = self.ler_faixa(de, ate)?;
        let mut por_bloco: HashMap<u32, Vec<Linha>> = (b_ini..=b_fim).map(|b| (b, Vec::new())).collect();
        for l in lidas {
            if let Some(v) = por_bloco.get_mut(&bloco_de(l.numero)) {
                v.push(l);
            }
        }
        for b in b_ini..=b_fim {
            let linhas = por_bloco.remove(&b).unwrap_or_default();
            self.cache.insert(0, (b, linhas));
        }
        self.cache.truncate(self.capacidade);
        Ok(())
    }

    /// As linhas com `de <= numero < ate`, lidas do fluxo (posicionando o cursor).
    fn ler_faixa(&mut self, de: u32, ate: u32) -> Resultado<Vec<Linha>> {
        let parte = self.folha().parte.clone();
        self.posicionar(de)?;
        let interno = self.pasta.interno.clone();
        let mut saida = Vec::new();
        let resultado = (|| -> Result<(), crate::xml::ErroXml> {
            let Some(cursor) = self.cursor.as_mut() else {
                return Ok(());
            };
            loop {
                let lida = match cursor.pendente.take() {
                    Some(p) => p,
                    None => match cursor.leitor.proxima(Some(&interno))? {
                        Some(l) => l,
                        None => return Ok(()),
                    },
                };
                if lida.linha.numero >= ate {
                    cursor.pendente = Some(lida);
                    return Ok(());
                }
                cursor.ultimo_consumido = lida.linha.numero;
                if lida.linha.numero >= de {
                    saida.push(lida.linha);
                }
            }
        })();
        match resultado {
            Ok(()) => Ok(saida),
            Err(e) => {
                self.cursor = None;
                Err(erros::da_leitura(e, &parte))
            }
        }
    }

    /// Deixa o cursor num ponto de onde a linha `alvo` ainda não passou: continua o fluxo aberto se der
    /// (saltando para um marco adiante, se houver), senão abre de novo.
    fn posicionar(&mut self, alvo: u32) -> Resultado<()> {
        let marco = self.indice.as_ref().and_then(|i| i.marco_para(alvo));
        let parte = self.folha().parte.clone();
        if let Some(cursor) = self.cursor.as_mut()
            && alvo > cursor.ultimo_consumido
        {
            if let Some(m) = marco
                && m.linha > cursor.ultimo_consumido
                && m.deslocamento > cursor.posicao()
            {
                cursor.pendente = None;
                let r = cursor.leitor.pular_para(m.deslocamento, m.linha.saturating_sub(1));
                match r {
                    Ok(()) => cursor.ultimo_consumido = m.linha.saturating_sub(1),
                    Err(e) => {
                        self.cursor = None;
                        return Err(erros::da_leitura(e, &parte));
                    }
                }
            }
            return Ok(());
        }
        // recomeço
        let f = self.pasta.folha_com_celulas(self.folha)?;
        let mut leitor = LeitorLinhas::novo(abrir_fluxo(&self.pasta, f));
        let mut ultimo_consumido = 0;
        if let Some(m) = marco {
            leitor.pular_para(m.deslocamento, m.linha.saturating_sub(1)).map_err(|e| erros::da_leitura(e, &parte))?;
            ultimo_consumido = m.linha.saturating_sub(1);
        }
        self.aberturas += 1;
        self.cursor = Some(Cursor { leitor, pendente: None, ultimo_consumido });
        Ok(())
    }
}

fn abrir_fluxo(pasta: &Pasta, folha: &Folha) -> Fluxo {
    Fluxo::abrir(pasta.interno.caminho.clone(), folha.parte.clone())
}

fn bloco_de(linha: u32) -> u32 {
    (linha.max(1) - 1) / LINHAS_POR_BLOCO
}
