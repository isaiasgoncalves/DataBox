//! O XML de uma aba (`xl/worksheets/sheetN.xml`) em fluxo: as linhas e as células uma a uma e, de passagem, as
//! informações da aba (dimensão, células mescladas, largura das colunas, painel congelado, filtro).
//!
//! Há dois modos de ler as linhas: o completo, que monta as células com valores tipados, e o rápido (sem
//! `Interno`), que só confere o número de cada linha e a última coluna, sem copiar texto: é o que o índice usa
//! para passar por milhões de células.

use std::io::Read;

use crate::datas::{valor_de_iso, valor_de_serie};
use crate::formato::Categoria;
use crate::pasta::{Interno, desescapar_x};
use crate::valores::{Celula, Intervalo, Linha, Valor, indice_da_coluna};
use crate::xml::{ErroXml, Evento, Leitor};

/// Largura (e se está oculta) de um trecho de colunas (`<col min max width hidden>`).
#[derive(Debug, Clone, PartialEq)]
pub struct InfoColuna {
    /// Primeira e última coluna do trecho (a partir de 0).
    pub primeira: u32,
    pub ultima: u32,
    /// Largura em caracteres do Excel.
    pub largura: Option<f64>,
    pub oculta: bool,
}

/// Linhas e colunas congeladas (`<pane state="frozen">`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Congelamento {
    pub linhas: u32,
    pub colunas: u32,
}

/// O que a aba diz sobre si mesma, fora das células.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetaFolha {
    /// `<dimension>`: o intervalo usado, como o programa que gravou declarou (pode estar errado ou ausente).
    pub dimensao: Option<Intervalo>,
    /// Células mescladas (`<mergeCells>`). Ficam no fim do XML: só vêm completas depois de
    /// [`crate::Planilha::indexar`] ou de ler a aba até o fim.
    pub mescladas: Vec<Intervalo>,
    pub colunas: Vec<InfoColuna>,
    pub congelado: Option<Congelamento>,
    /// Autofiltro (`<autoFilter ref>`, no fim do XML como as mescladas).
    pub filtro: Option<Intervalo>,
    /// Altura padrão das linhas, em pontos.
    pub altura_padrao: Option<f64>,
    /// Largura padrão das colunas, em caracteres.
    pub largura_padrao: Option<f64>,
}

impl MetaFolha {
    /// A mescla que cobre a célula (linha a partir de 1, coluna a partir de 0).
    pub fn mescla_em(&self, linha: u32, coluna: u32) -> Option<&Intervalo> {
        self.mescladas.iter().find(|m| m.contem(linha, coluna))
    }

    /// Largura da coluna (a partir de 0): a do trecho que a cobre, ou a padrão.
    pub fn largura_da_coluna(&self, coluna: u32) -> Option<f64> {
        self.colunas
            .iter()
            .find(|c| (c.primeira..=c.ultima).contains(&coluna))
            .and_then(|c| c.largura)
            .or(self.largura_padrao)
    }
}

/// Uma linha lida, com o lugar dela no fluxo (o deslocamento do `<row`).
pub(crate) struct LinhaLida {
    pub linha: Linha,
    pub deslocamento: u64,
    /// Última coluna que aparece nas células (a partir de 0), mesmo no modo rápido.
    pub ultima_coluna: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Numero,
    Compartilhada,
    TextoDeFormula,
    Embutida,
    Booleano,
    Erro,
    Data,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Alvo {
    Nenhum,
    V,
    F,
    T,
}

/// Os elementos que o modo rápido precisa ver: as linhas, as células (só a referência) e os que dizem algo
/// sobre a aba.
const MARCAS_DO_INDICE: &[&str] =
    &["row", "c", "sheetData", "dimension", "mergeCell", "col", "pane", "sheetFormatPr", "autoFilter"];

pub(crate) struct LeitorLinhas<R: Read> {
    pub xml: Leitor<R>,
    pub meta: MetaFolha,
    /// Número da última linha lida (as linhas têm de crescer: uma repetida ou fora de ordem vai para a seguinte).
    ultima_numero: u32,
    v: String,
    f: String,
    embutido: String,
}

fn atributo_numero<R: Read>(xml: &Leitor<R>, nome: &str) -> Option<u32> {
    xml.atributo(nome).and_then(|v| v.trim().parse().ok())
}

fn atributo_real<R: Read>(xml: &Leitor<R>, nome: &str) -> Option<f64> {
    xml.atributo(nome).and_then(|v| v.trim().parse::<f64>().ok()).filter(|x| x.is_finite())
}

fn atributo_bool<R: Read>(xml: &Leitor<R>, nome: &str) -> bool {
    xml.atributo(nome).is_some_and(|v| matches!(v.trim(), "1") || v.trim().eq_ignore_ascii_case("true"))
}

/// Letras da coluna no começo de uma referência ("C12" → 2), sem exigir o número da linha.
fn coluna_da_referencia(r: &str) -> Option<u32> {
    let r = r.trim().trim_start_matches('$');
    let fim = r.bytes().take_while(u8::is_ascii_alphabetic).count();
    indice_da_coluna(&r[..fim])
}

impl<R: Read> LeitorLinhas<R> {
    pub fn novo(fonte: R) -> Self {
        LeitorLinhas {
            xml: Leitor::novo(fonte),
            meta: MetaFolha::default(),
            ultima_numero: 0,
            v: String::new(),
            f: String::new(),
            embutido: String::new(),
        }
    }

    /// Liga o modo rápido do leitor: nada de texto, e só chegam as marcas que o índice usa.
    pub fn modo_rapido(&mut self) {
        self.xml.sem_texto(true);
        self.xml.filtrar(Some(MARCAS_DO_INDICE));
    }

    /// Retoma no deslocamento `deslocamento` (o `<row` de um marco do índice) e diz qual é a linha anterior.
    pub fn pular_para(&mut self, deslocamento: u64, linha_anterior: u32) -> Result<(), ErroXml> {
        self.xml.pular_para(deslocamento)?;
        self.ultima_numero = linha_anterior;
        Ok(())
    }

    /// Lê até o começo de `<sheetData>`: dimensão, colunas, painel, padrões.
    pub fn ler_cabecalho(&mut self) -> Result<(), ErroXml> {
        loop {
            match self.xml.avancar()? {
                Evento::Fim => return Ok(()),
                Evento::Abre => {
                    if self.xml.nome() == "sheetData" {
                        return Ok(());
                    }
                    self.elemento_da_aba();
                }
                _ => {}
            }
        }
    }

    /// Os elementos que dizem algo sobre a aba (só atributos).
    fn elemento_da_aba(&mut self) {
        let xml = &self.xml;
        match xml.nome() {
            "dimension" => {
                if let Some(i) = xml.atributo("ref").and_then(Intervalo::de_texto) {
                    self.meta.dimensao = Some(i);
                }
            }
            "mergeCell" => {
                if let Some(i) = xml.atributo("ref").and_then(Intervalo::de_texto) {
                    self.meta.mescladas.push(i);
                }
            }
            "col" => {
                let primeira = atributo_numero(xml, "min").unwrap_or(1).max(1) - 1;
                let ultima = atributo_numero(xml, "max").unwrap_or(primeira + 1).max(1) - 1;
                self.meta.colunas.push(InfoColuna {
                    primeira,
                    ultima: ultima.max(primeira),
                    largura: atributo_real(xml, "width"),
                    oculta: atributo_bool(xml, "hidden"),
                });
            }
            "pane" => {
                if matches!(xml.atributo("state"), Some("frozen" | "frozenSplit")) {
                    let n = |nome| atributo_real(xml, nome).map_or(0, |x| x.max(0.0) as u32);
                    self.meta.congelado = Some(Congelamento { linhas: n("ySplit"), colunas: n("xSplit") });
                }
            }
            "sheetFormatPr" => {
                self.meta.altura_padrao = atributo_real(xml, "defaultRowHeight");
                self.meta.largura_padrao = atributo_real(xml, "defaultColWidth");
            }
            "autoFilter" if self.meta.filtro.is_none() => {
                self.meta.filtro = xml.atributo("ref").and_then(Intervalo::de_texto);
            }
            _ => {}
        }
    }

    /// A próxima linha. Com `ctx`, as células vêm montadas; sem ele (modo rápido) a linha vem sem células e só
    /// com o número e a última coluna. `Ok(None)` no fim da aba.
    pub fn proxima(&mut self, ctx: Option<&Interno>) -> Result<Option<LinhaLida>, ErroXml> {
        loop {
            match self.xml.avancar()? {
                Evento::Fim => return Ok(None),
                Evento::Abre => {
                    if self.xml.nome() == "row" {
                        return self.linha(ctx).map(Some);
                    }
                    self.elemento_da_aba();
                }
                _ => {}
            }
        }
    }

    fn linha(&mut self, ctx: Option<&Interno>) -> Result<LinhaLida, ErroXml> {
        let deslocamento = self.xml.inicio_do_evento();
        let numero = match atributo_numero(&self.xml, "r") {
            Some(r) if r > self.ultima_numero => r,
            _ => self.ultima_numero + 1,
        };
        self.ultima_numero = numero;
        let mut linha = Linha {
            numero,
            oculta: atributo_bool(&self.xml, "hidden"),
            altura: atributo_real(&self.xml, "ht").map(|h| h as f32),
            celulas: Vec::new(),
        };
        // modo rápido: a linha inteira de uma vez, sem olhar as células
        if ctx.is_none() && self.xml.pular_linha()? {
            let ultima_coluna = coluna_da_referencia(self.xml.ultima_referencia());
            return Ok(LinhaLida { linha, deslocamento, ultima_coluna });
        }
        let mut ultima: Option<u32> = None;
        let mut desordenada = false;
        loop {
            match self.xml.avancar()? {
                Evento::Fim => break,
                Evento::Fecha if self.xml.nome() == "row" => break,
                Evento::Abre if self.xml.nome() == "c" => {
                    let coluna = match self.xml.atributo("r").and_then(coluna_da_referencia) {
                        Some(c) => c,
                        None => ultima.map_or(0, |u| u + 1),
                    };
                    if ultima.is_some_and(|u| coluna <= u) {
                        desordenada = true;
                    }
                    ultima = Some(ultima.map_or(coluna, |u| u.max(coluna)));
                    // sem `ctx` (modo rápido) o resto da célula nem é lido: o leitor já pula o que está dentro
                    if let Some(ctx) = ctx {
                        let celula = self.celula(coluna, ctx)?;
                        linha.celulas.push(celula);
                    }
                }
                _ => {}
            }
        }
        if desordenada {
            linha.celulas.sort_by_key(|c| c.coluna);
        }
        Ok(LinhaLida { linha, deslocamento, ultima_coluna: ultima })
    }

    /// A célula recém-aberta (`<c>`), até o `</c>`.
    fn celula(&mut self, coluna: u32, ctx: &Interno) -> Result<Celula, ErroXml> {
        let tipo = match self.xml.atributo("t") {
            None | Some("n") => Tipo::Numero,
            Some("s") => Tipo::Compartilhada,
            Some("str") => Tipo::TextoDeFormula,
            Some("inlineStr") => Tipo::Embutida,
            Some("b") => Tipo::Booleano,
            Some("e") => Tipo::Erro,
            Some("d") => Tipo::Data,
            Some(_) => Tipo::TextoDeFormula,
        };
        let estilo = atributo_numero(&self.xml, "s").unwrap_or(0);
        self.v.clear();
        self.f.clear();
        self.embutido.clear();
        let mut alvo = Alvo::Nenhum;
        let mut fonetico = 0usize;
        loop {
            match self.xml.avancar()? {
                Evento::Fim => break,
                Evento::Abre => match self.xml.nome() {
                    "v" => alvo = Alvo::V,
                    "f" => alvo = Alvo::F,
                    "t" if fonetico == 0 => alvo = Alvo::T,
                    "rPh" => fonetico += 1,
                    _ => {}
                },
                Evento::Texto => match alvo {
                    Alvo::V => self.v.push_str(self.xml.texto()),
                    Alvo::F => self.f.push_str(self.xml.texto()),
                    Alvo::T => self.embutido.push_str(self.xml.texto()),
                    Alvo::Nenhum => {}
                },
                Evento::Fecha => match self.xml.nome() {
                    "v" | "f" | "t" => alvo = Alvo::Nenhum,
                    "rPh" => fonetico = fonetico.saturating_sub(1),
                    "c" => break,
                    _ => {}
                },
            }
        }
        let valor = self.valor(tipo, estilo, ctx);
        let formula = if self.f.is_empty() { None } else { Some(self.f.clone()) };
        Ok(Celula { coluna, valor, estilo, formula })
    }

    fn valor(&self, tipo: Tipo, estilo: u32, ctx: &Interno) -> Valor {
        let v = self.v.trim();
        match tipo {
            Tipo::Numero => {
                if v.is_empty() {
                    return Valor::Vazio;
                }
                let Ok(x) = v.parse::<f64>() else {
                    return Valor::Texto(self.v.clone());
                };
                match ctx.formato(estilo).categoria() {
                    Categoria::Data => valor_de_serie(x, true, false, ctx.sistema),
                    Categoria::Hora => valor_de_serie(x, false, true, ctx.sistema),
                    Categoria::DataHora => valor_de_serie(x, false, false, ctx.sistema),
                    _ => Valor::Numero(x),
                }
            }
            Tipo::Compartilhada => match v.parse::<usize>().ok().and_then(|i| ctx.textos.get(i)) {
                Some(t) => Valor::Texto(t.to_string()),
                None => Valor::Vazio,
            },
            Tipo::TextoDeFormula => Valor::Texto(desescapar_x(&self.v).into_owned()),
            Tipo::Embutida => {
                if self.embutido.is_empty() && !self.v.is_empty() {
                    Valor::Texto(desescapar_x(&self.v).into_owned())
                } else {
                    Valor::Texto(desescapar_x(&self.embutido).into_owned())
                }
            }
            Tipo::Booleano => Valor::Booleano(v == "1" || v.eq_ignore_ascii_case("true")),
            Tipo::Erro => Valor::Erro(v.to_string()),
            Tipo::Data => valor_de_iso(v).unwrap_or_else(|| Valor::Texto(self.v.clone())),
        }
    }
}
