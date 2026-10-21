//! Grade de dados: o desenho de célula e de cabeçalho de coluna, o [`NucleoGrade`] (rolagem virtual, cabeçalho
//! grudado, larguras com alças, célula ativa e teclado, com ganchos por célula e por linha — a base da grade
//! editável da aba Dados) e a [`GradeSomenteLeitura`] da Consulta SQL, montada sobre o núcleo.
//!
//! Célula (`.grade td`, Main.dc.html e Dados.dc.html): 28px com o fio `--b` embaixo e à direita, `padding:0 10px`,
//! texto 12px em `--t` numa linha; NULL = `td.nulo` (itálico `--t3`); texto vazio = "vazio" no mesmo itálico;
//! BLOB = o `.chip` "BLOB · 2,3 KB" (dica com o começo em hexadecimal); texto cortado em 300 caracteres = o
//! começo + "…" em `--acct` 700 (dica "N caracteres no total"); `td.chave` em `--acct` 600
//! ([`pintar_celula_chave`]); números à direita e escritos como o `String(número)` do JavaScript
//! ([`crate::formato::real_js`]: `17051.0` → "17051"); texto que não cabe na coluna termina em "…". A dica da
//! célula é o texto inteiro (até 300 caracteres). A linha sob o ponteiro ganha o `--selh` (`tr:hover td`), a
//! marcada o `--sel` (`tr.mar td`, pelo [`DesenhoGrade::fundo_linha`]) e a célula ativa o contorno de 2px `--acc`
//! por dentro com o brilho interno `--brilho` (`td.ativa`, [`pintar_celula_ativa`]). A coluna do número da linha
//! (`td.lin`, [`NucleoGrade::com_coluna_numero`]) fica no fundo `--p` com o número em `--t3`
//! ([`pintar_numero_linha`]); a da caixa de marcar (`td.selc`, [`NucleoGrade::com_coluna_marcar`]) tem 32px, sem
//! padding e sem o fio à direita.
//!
//! Cabeçalho (`.grade th`): 30px, 11.5px / 600 em `--t2` sem caixa alta, `padding:0 10px`, fundo `Cabecalho` com
//! as riscas (a regra `.faixa,.grade th,.lista-t th` do fim do CSS), fio `--btb` embaixo e `--b` à direita; o tipo
//! declarado vai à direita do nome em 10px `--t3` (o `.tp`; a etapa 3 troca pela pastilha) e a seta da ordem
//! desenhada em `--acct` (a `.seta-ord` da revisão de UI de 19/10/2026, [`pintar_seta_ordem`]). Fica grudado no topo
//! ao rolar.
//!
//! O intervalo (0.2.2, [`Intervalo`]): arrastar de uma célula a outra, o Shift+clique e o Shift com as setas marcam um
//! retângulo de células a partir da ativa, com o fundo `--tint` e o contorno de 1px `--acc`; o Ctrl+C de quem chama o
//! leva em TSV ([`tsv_do_intervalo`]).
//!
//! O menu da célula (0.2.3, [`menu_da_celula`]): o clique direito numa célula (dentro do intervalo, o menu age nele;
//! fora, a célula vira a ativa) e o Shift+F10 com o foco na grade pedem o menu ([`RespostaNucleo::menu_celula`]); a
//! tela diz o que liga ([`OfertasCelula`]) e cumpre a [`AcaoCelula`] escolhida. O Ctrl+Shift+C com o foco na grade é
//! o "Copiar com cabeçalho" ([`RespostaNucleo::copiar_com_cabecalho`]); os formatos do "Copiar como" ficam no
//! [`tabular::colunas`] ([`texto_do_intervalo_como`]).
//!
//! A planilha (etapa 6) usa o mesmo núcleo com o painel congelado ([`NucleoGrade::com_congelado`]: linhas sob o
//! cabeçalho e colunas presas à esquerda, o fio de 2px `--b2` na emenda), as células mescladas
//! ([`NucleoGrade::com_mesclas`]), as larguras do arquivo ([`larguras_do_arquivo`]) e o clique no número da linha
//! ([`RespostaNucleo::clicou_numero_da_linha`]); sem pedir nada disso a grade é a de sempre.
//!
//! As funções de desenho recebem um `Painter` (com o recorte da célula) e a [`Paleta`]: nada de `Ui` por
//! célula, para a rolagem de 100 mil linhas não alocar por quadro mais que os textos visíveis.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use comum::Celula;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, CursorIcon, Event, EventFilter, FocusDirection, Galley, Id, Key, Painter, Rect, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use tabular::colunas::{FormatoCopia, OpcoesCopia, ValorCopia};

use crate::fontes::{self, Peso};
use crate::formato::{formatar_bytes, formatar_numero, real_js};
use crate::icones::{self, Icone, TipoColuna};
use crate::pecas::menu_contexto::consumir_shift_f10;
use crate::pecas::rotulos::{pintar_chip_em, tamanho_chip_em};
use crate::pecas::tabela::PADDING_TABELA;
use crate::pecas::texto::{formato, linha_de_base};
use crate::pecas::toast::toast_concluido;
use crate::pintura::{self, Preenchimento};
use crate::tema::{Paleta, paleta};

mod menu_celula;

pub use menu_celula::{
    AcaoCelula, LARGURA_MENU_DA_CELULA, OfertaCelula, OfertasCelula, menu_da_celula, menu_da_celula_aberto,
    texto_do_intervalo_como, tsv_do_intervalo_com_cabecalho, valor_de_copia,
};

/// Altura de uma linha (`.grade td{height:28px}`, com o fio de 1px embaixo).
pub const ALTURA_LINHA: f32 = 28.0;
/// Altura do cabeçalho (`.grade th{height:30px}`, com o fio `--btb` embaixo).
pub const ALTURA_CABECALHO: f32 = 30.0;
/// Altura do cabeçalho com o nome que ordena: a mesma (o `th` do Estúdio não tem o pontilhado embaixo do nome).
pub const ALTURA_CABECALHO_ABRE: f32 = ALTURA_CABECALHO;
/// Altura do cabeçalho com o tipo: a mesma (o tipo vai à direita do nome, não embaixo).
pub const ALTURA_CABECALHO_COM_TIPO: f32 = ALTURA_CABECALHO;
/// Altura do cabeçalho da aba Dados (o nome que ordena e o tipo): a mesma.
pub const ALTURA_CABECALHO_COM_TIPO_ABRE: f32 = ALTURA_CABECALHO;
/// Padding lateral das células e dos `th` da grade (`.grade th, .grade td{padding:0 10px}`); o conteúdo ainda
/// perde o fio de 1px da direita.
pub const PADDING_X: f32 = 10.0;
/// Padding da primeira e da última coluna da grade: no Estúdio, o mesmo das outras ([`PADDING_X`]).
pub const PADDING_BORDA: f32 = PADDING_X;
/// Largura da coluna da caixa de marcar (`.grade th.selc, .grade td.selc{width:32px;padding:0}`).
pub const LARGURA_COLUNA_MARCAR: f32 = 32.0;
/// Largura máxima de uma coluna ao abrir (`max-width:320px`).
pub const LARGURA_MAX_COLUNA: f32 = 320.0;
/// Largura mínima de uma coluna ao redimensionar.
pub const LARGURA_MIN_COLUNA: f32 = 48.0;
/// Quantas linhas do começo entram na medida da largura inicial das colunas.
const LINHAS_MEDIDAS: usize = 200;
/// Passo do PageUp/PageDown na aba Dados.
pub const PASSO_PAGINA_DADOS: usize = 10;

/// A calha da barra de rolagem (`::-webkit-scrollbar{width:8px}`): o espaço que a barra ocupa quando o conteúdo rola,
/// na grade e na página dos documentos.
pub const CALHA_ROLAGEM: f32 = 8.0;

/// A espessura do fio da emenda do painel congelado (`.xl tr.cl td{border-bottom:2px solid var(--b2)}` e
/// `.xl td.cc{border-right:2px ...}`, prancheta 17): o fio em `--b2`, por dentro da última linha e da última coluna
/// congeladas.
pub const LARGURA_EMENDA: f32 = 2.0;
/// Quanto do que se vê o painel congelado pode ocupar antes de ser ignorado naquele quadro (decisão 10 da etapa 6): mais
/// de 60% da altura (o cabeçalho e as linhas congeladas) ou da largura (as colunas presas) numa direção que rola e a
/// rolagem nunca prenderia: o congelamento daquela direção fica de lado.
pub const FRACAO_MAXIMA_CONGELADA: f32 = 0.6;
/// A maior mescla que se desenha, em linhas (uma mescla maior pinta cada célula como ela é).
pub const MAX_LINHAS_DA_MESCLA: usize = 1_000;
/// A maior mescla que se desenha, em colunas.
pub const MAX_COLUNAS_DA_MESCLA: usize = 100;

/// O dígito do Calibri 11 tem 7px a 96 dpi: a largura da coluna do arquivo é `caracteres × 7 + 5` px (o `+ 5` é a folga
/// do Excel).
pub const PX_POR_CARACTERE: f64 = 7.0;
/// A folga que o Excel soma à largura em caracteres.
pub const PX_DE_FOLGA: f64 = 5.0;
/// A menor coluna que vem do arquivo, em px.
pub const LARGURA_MIN_ARQUIVO: f32 = 24.0;
/// A maior coluna que vem do arquivo, em px.
pub const LARGURA_MAX_ARQUIVO: f32 = 640.0;
/// A largura padrão do Excel, em caracteres (8,43 → 64px).
pub const CARACTERES_PADRAO_ARQUIVO: f64 = 8.43;

/// A largura de uma coluna do arquivo, em px: `round(caracteres × 7 + 5)`, entre [`LARGURA_MIN_ARQUIVO`] (24) e
/// [`LARGURA_MAX_ARQUIVO`] (640). Um valor que não é número (ou negativo) cai na padrão
/// ([`CARACTERES_PADRAO_ARQUIVO`]).
pub fn largura_do_arquivo(caracteres: f64) -> f32 {
    let c = if caracteres.is_finite() && caracteres >= 0.0 { caracteres } else { CARACTERES_PADRAO_ARQUIVO };
    ((c * PX_POR_CARACTERE + PX_DE_FOLGA).round() as f32).clamp(LARGURA_MIN_ARQUIVO, LARGURA_MAX_ARQUIVO)
}

/// O que o arquivo diz de uma coluna: a largura em caracteres (`None` = a padrão) e se está oculta.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ColunaDoArquivo {
    pub caracteres: Option<f64>,
    pub oculta: bool,
}

/// As colunas visíveis do arquivo: as larguras em px (as colunas ocultas saem) e, para cada coluna que ficou, qual era a
/// dela no arquivo (a letra da coluna vem do índice do arquivo, não da posição na grade).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LargurasDoArquivo {
    pub larguras: Vec<f32>,
    pub do_arquivo: Vec<usize>,
}

/// As larguras do arquivo ([`largura_do_arquivo`]) sem as colunas ocultas, e o mapa coluna visível → coluna do arquivo.
pub fn larguras_do_arquivo(colunas: &[ColunaDoArquivo]) -> LargurasDoArquivo {
    let mut saida = LargurasDoArquivo::default();
    for (i, c) in colunas.iter().enumerate().filter(|(_, c)| !c.oculta) {
        saida.larguras.push(largura_do_arquivo(c.caracteres.unwrap_or(CARACTERES_PADRAO_ARQUIVO)));
        saida.do_arquivo.push(i);
    }
    saida
}

/// O painel congelado: as primeiras `linhas` linhas ficam sob o cabeçalho e as primeiras `colunas` colunas (de dados,
/// depois das colunas especiais) grudam à esquerda quando a grade rola ([`NucleoGrade::com_congelado`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Congelado {
    pub linhas: usize,
    pub colunas: usize,
}

/// Uma célula mesclada, em linhas e colunas da grade (a partir de 0, os dois cantos **incluídos**): o texto vem da célula
/// de cima à esquerda e se pinta no retângulo todo ([`DesenhoGrade::celula_mesclada`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mescla {
    pub linha0: usize,
    pub coluna0: usize,
    pub linha1: usize,
    pub coluna1: usize,
}

impl Mescla {
    pub fn new(linha0: usize, coluna0: usize, linha1: usize, coluna1: usize) -> Self {
        Mescla { linha0, coluna0, linha1, coluna1 }
    }

    /// Quantas linhas a mescla cobre.
    pub fn linhas(&self) -> usize {
        self.linha1.saturating_sub(self.linha0) + 1
    }

    /// Quantas colunas a mescla cobre.
    pub fn colunas(&self) -> usize {
        self.coluna1.saturating_sub(self.coluna0) + 1
    }

    /// A célula está dentro da mescla?
    pub fn contem(&self, linha: usize, coluna: usize) -> bool {
        (self.linha0..=self.linha1).contains(&linha) && (self.coluna0..=self.coluna1).contains(&coluna)
    }

    /// Dá para desenhar: dois cantos na ordem certa, mais de uma célula e dentro de
    /// [`MAX_LINHAS_DA_MESCLA`] × [`MAX_COLUNAS_DA_MESCLA`].
    fn desenhavel(&self) -> bool {
        self.linha1 >= self.linha0
            && self.coluna1 >= self.coluna0
            && (self.linha1 > self.linha0 || self.coluna1 > self.coluna0)
            && self.linhas() <= MAX_LINHAS_DA_MESCLA
            && self.colunas() <= MAX_COLUNAS_DA_MESCLA
    }
}

impl From<(usize, usize, usize, usize)> for Mescla {
    /// `(linha0, coluna0, linha1, coluna1)`.
    fn from((linha0, coluna0, linha1, coluna1): (usize, usize, usize, usize)) -> Self {
        Mescla { linha0, coluna0, linha1, coluna1 }
    }
}

/// As mesclas de uma grade, montadas uma vez: ordenadas pela linha inicial e com a maior altura guardada, para cada
/// quadro achar só as que cruzam a tela com uma busca binária e um passo curto para trás (sem alocar). As que não se
/// desenham (uma célula só, cantos trocados, mais de [`MAX_LINHAS_DA_MESCLA`] linhas ou [`MAX_COLUNAS_DA_MESCLA`]
/// colunas) ficam de fora.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndiceDeMesclas {
    mesclas: Vec<Mescla>,
    maior_altura: usize,
    descartadas: usize,
}

impl IndiceDeMesclas {
    pub fn nova(mesclas: impl IntoIterator<Item = Mescla>) -> Self {
        let (mut boas, mut descartadas) = (Vec::new(), 0);
        for m in mesclas {
            if m.desenhavel() {
                boas.push(m);
            } else if m.linha1 >= m.linha0 && m.coluna1 >= m.coluna0 && (m.linha1 > m.linha0 || m.coluna1 > m.coluna0) {
                descartadas += 1;
            }
        }
        boas.sort_by_key(|m| (m.linha0, m.coluna0));
        let maior_altura = boas.iter().map(Mescla::linhas).max().unwrap_or(0);
        IndiceDeMesclas { mesclas: boas, maior_altura, descartadas }
    }

    /// As mesclas que se desenham, ordenadas pela linha inicial.
    pub fn mesclas(&self) -> &[Mescla] {
        &self.mesclas
    }

    pub fn len(&self) -> usize {
        self.mesclas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mesclas.is_empty()
    }

    /// Quantas mesclas de mais de uma célula ficaram de fora por serem gigantes.
    pub fn descartadas(&self) -> usize {
        self.descartadas
    }

    /// As mesclas que cruzam alguma das `linhas` (sem alocar): a busca binária acha o fim (as que começam depois não
    /// cruzam) e o começo (as que começam antes de `linhas.start − maior altura` já acabaram), e o resto se confere.
    pub fn que_cruzam(&self, linhas: Range<usize>) -> impl Iterator<Item = &Mescla> + '_ {
        let fim = self.mesclas.partition_point(|m| m.linha0 < linhas.end);
        let inicio = self.mesclas.partition_point(|m| m.linha0 + self.maior_altura <= linhas.start).min(fim);
        self.mesclas[inicio..fim].iter().filter(move |m| m.linha1 >= linhas.start)
    }

    /// A mescla que cobre a célula, se houver.
    pub fn mescla_em(&self, linha: usize, coluna: usize) -> Option<&Mescla> {
        self.que_cruzam(linha..linha + 1).find(|m| m.contem(linha, coluna))
    }
}

/// A altura do cabeçalho: [`ALTURA_CABECALHO`] (30px) com ou sem o tipo e o nome que ordena — no Estúdio o tipo
/// fica ao lado do nome e o `th` que ordena não tem o pontilhado embaixo.
pub fn altura_cabecalho(_com_tipo: bool, _abre: bool) -> f32 {
    ALTURA_CABECALHO
}

/// Uma coluna da grade: o nome e, na aba Dados, o tipo declarado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColunaGrade {
    pub nome: String,
    pub tipo: Option<String>,
    /// A pastilha de tipo de 16px antes do nome (a Consulta SQL: pelo que os valores são).
    pub pastilha: Option<TipoColuna>,
}

impl ColunaGrade {
    pub fn new(nome: impl Into<String>) -> Self {
        ColunaGrade { nome: nome.into(), tipo: None, pastilha: None }
    }

    pub fn com_tipo(nome: impl Into<String>, tipo: impl Into<String>) -> Self {
        ColunaGrade { nome: nome.into(), tipo: Some(tipo.into()), pastilha: None }
    }

    /// Com a pastilha de tipo antes do nome (o tipo por extenso vai na dica).
    pub fn com_pastilha(mut self, pastilha: TipoColuna) -> Self {
        self.pastilha = Some(pastilha);
        self
    }
}

/// A pastilha de uma coluna de resultado pelos valores (a Consulta SQL não sabe o tipo declarado da coluna de
/// origem): `12` quando todos os valores são inteiros, `,5` quando são números e algum tem casas, `01` quando são
/// BLOB, e `Aa` (o padrão) em qualquer outro caso, também quando só há NULL.
pub fn pastilha_pelos_valores<'a>(valores: impl IntoIterator<Item = &'a Celula>) -> TipoColuna {
    let (mut inteiros, mut reais, mut blobs, mut outros) = (0_usize, 0_usize, 0_usize, 0_usize);
    for v in valores {
        match v {
            Celula::Nulo => {}
            Celula::Inteiro(_) => inteiros += 1,
            Celula::Real(_) => reais += 1,
            Celula::Blob { .. } => blobs += 1,
            _ => outros += 1,
        }
    }
    match (inteiros, reais, blobs, outros) {
        (0, 0, 0, _) => TipoColuna::Texto,
        (_, 0, 0, 0) => TipoColuna::Inteiro,
        (_, _, 0, 0) => TipoColuna::Real,
        (0, 0, _, 0) => TipoColuna::Blob,
        _ => TipoColuna::Texto,
    }
}

/// Ordenação de uma coluna (a seta desenhada em `--acct` no cabeçalho, [`pintar_seta_ordem`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordem {
    Crescente,
    Decrescente,
}

/// Coluna numérica (alinha à direita): todos os valores não nulos são números e há pelo menos um (a regra da
/// Consulta SQL).
pub fn coluna_numerica<'a>(valores: impl IntoIterator<Item = &'a Celula>) -> bool {
    let mut algum = false;
    for v in valores {
        match v {
            Celula::Nulo => {}
            v if v.e_numero() => algum = true,
            _ => return false,
        }
    }
    algum
}

/// Texto que a célula mostra (sem o "…" do corte): número (o real como o JavaScript escreve, [`real_js`]), texto
/// numa linha (quebras viram espaço, como no `white-space:nowrap`), "NULL", "vazio" ou o rótulo do BLOB.
pub fn texto_exibicao(c: &Celula) -> Cow<'_, str> {
    match c {
        Celula::Nulo => Cow::Borrowed("NULL"),
        Celula::Inteiro(i) => Cow::Owned(i.to_string()),
        Celula::Real(r) => Cow::Owned(real_js(*r)),
        Celula::Texto(s) if s.is_empty() => Cow::Borrowed("vazio"),
        Celula::Texto(s) => uma_linha(s),
        Celula::TextoCortado { inicio, .. } => uma_linha(inicio),
        Celula::Blob { tamanho, .. } => Cow::Owned(format!("BLOB · {}", formatar_bytes(*tamanho))),
    }
}

/// O texto cru da célula para o campo de edição no lugar e para a cópia: NULL e BLOB
/// → "", número como o `String(número)` do JavaScript (`17051.0` → "17051"), texto cortado → só o começo (quem
/// edita texto cortado abre a janela da linha, que lê o texto inteiro). A aba Dados usa esta função no
/// `Entrada::compacta` da edição no lugar.
pub fn texto_celula(c: &Celula) -> String {
    match c {
        Celula::Nulo | Celula::Blob { .. } => String::new(),
        Celula::Inteiro(i) => i.to_string(),
        Celula::Real(r) => real_js(*r),
        Celula::Texto(s) => s.clone(),
        Celula::TextoCortado { inicio, .. } => inicio.clone(),
    }
}

fn uma_linha(s: &str) -> Cow<'_, str> {
    if !s.contains(['\n', '\r', '\t']) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut espaco = false;
    for ch in s.chars() {
        if matches!(ch, '\n' | '\r' | '\t' | ' ') {
            if !espaco {
                out.push(' ');
            }
            espaco = true;
        } else {
            out.push(ch);
            espaco = false;
        }
    }
    Cow::Owned(out)
}

/// A dica (o `title`) da célula: o texto inteiro para texto e número; para o texto cortado, o começo e
/// "N caracteres no total"; para o BLOB, o começo em hexadecimal; NULL não tem dica.
pub fn dica_celula(c: &Celula) -> Option<String> {
    match c {
        Celula::Nulo => None,
        Celula::Texto(s) if s.is_empty() => Some("Texto vazio ('')".into()),
        Celula::Texto(s) => Some(s.clone()),
        Celula::Inteiro(_) | Celula::Real(_) => Some(texto_celula(c)),
        Celula::TextoCortado { inicio, tamanho } => {
            Some(format!("{inicio}…\n{} caracteres no total", formatar_numero(*tamanho)))
        }
        Celula::Blob { inicio, .. } if inicio.is_empty() => Some("Conteúdo binário".into()),
        Celula::Blob { inicio, .. } => Some(format!("Início em hexadecimal: {}", hex(inicio))),
    }
}

/// Bytes em hexadecimal minúsculo, sem separador.
pub fn hex(bytes: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from(D[usize::from(b >> 4)]));
        s.push(char::from(D[usize::from(b & 0x0F)]));
    }
    s
}

/// O texto de uma célula numa linha (`white-space:nowrap; text-overflow:ellipsis`): 12px em `--t` (`td.chave`:
/// `--acct` 600), o NULL e o "vazio" em itálico `--t3`, o texto cortado com o "…" em `--acct` 700. O BLOB não tem
/// texto (é o chip).
fn job_celula(c: &Celula, p: &Paleta, largura: f32, chave: bool) -> Option<LayoutJob> {
    let (corpo, cor) =
        if chave { (fontes::fonte(12.0, Peso::Medio), p.gold_text) } else { (fontes::fonte(12.0, Peso::Normal), p.fg) };
    let mut job = match c {
        Celula::Blob { .. } => return None,
        Celula::Nulo => {
            LayoutJob::single_section("NULL".to_owned(), formato(fontes::fonte(12.0, Peso::Italico), p.faint))
        }
        Celula::Texto(s) if s.is_empty() => {
            LayoutJob::single_section("vazio".to_owned(), formato(fontes::fonte(12.0, Peso::Italico), p.faint))
        }
        Celula::TextoCortado { inicio, .. } => {
            let mut j = LayoutJob::default();
            j.append(&uma_linha(inicio), 0.0, formato(corpo, cor));
            j.append("…", 1.0, formato(fontes::fonte(12.0, Peso::Pesado), p.gold_text));
            j
        }
        outro => LayoutJob::single_section(texto_exibicao(outro).into_owned(), formato(corpo, cor)),
    };
    job.wrap =
        TextWrapping {
            max_width: largura.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…')
        };
    job.break_on_newline = false;
    Some(job)
}

/// Largura natural do conteúdo de uma célula (sem padding).
pub fn largura_natural(painter: &Painter, p: &Paleta, c: &Celula) -> f32 {
    match c {
        Celula::Blob { .. } => tamanho_chip_em(painter, &texto_exibicao(c)).x,
        _ => job_celula(c, p, f32::INFINITY, false).map_or(0.0, |j| painter.layout_job(j).size().x),
    }
}

/// Pinta o conteúdo de uma célula no retângulo de conteúdo (já sem o padding), centrado na vertical; numa
/// coluna numérica, tudo à direita. O `painter` deve vir com o recorte da célula.
pub fn pintar_celula(painter: &Painter, p: &Paleta, conteudo: Rect, c: &Celula, numerica: bool) {
    pintar_celula_com(painter, p, conteudo, c, numerica, false);
}

/// O texto de uma célula de arquivo de texto (o CSV, prancheta 7): como o [`pintar_celula`] de um texto (12px em
/// `--t`, numa linha, "…" quando não cabe; quebras viram espaço), mas **sem o "vazio"**: o CSV não tem NULL, e um
/// "vazio" por célula afogaria a tabela, então o texto vazio não pinta nada. `numerica` alinha à direita
/// (`td.num`). O `painter` deve vir com o recorte da célula.
pub fn pintar_texto_de_celula(painter: &Painter, p: &Paleta, conteudo: Rect, texto: &str, numerica: bool) {
    if texto.is_empty() {
        return;
    }
    let mut job =
        LayoutJob::single_section(uma_linha(texto).into_owned(), formato(fontes::fonte(12.0, Peso::Normal), p.fg));
    job.wrap = TextWrapping {
        max_width: conteudo.width().max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    job.break_on_newline = false;
    let g = painter.layout_job(job);
    let x = if numerica { conteudo.right() - g.size().x } else { conteudo.left() };
    painter.galley(pos2(x, conteudo.center().y - g.size().y / 2.0), g, p.fg);
}

/// Onde o texto de uma célula fica no retângulo de conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alinhamento {
    #[default]
    Esquerda,
    /// `td.num` / `td.d`: números e datas.
    Direita,
    /// `td.c` / `td.err`: o booleano e o erro da planilha.
    Centro,
}

/// O texto de uma célula de arquivo (a planilha, prancheta 17) com o alinhamento e a cor à escolha: 12px numa linha com
/// "…" quando não cabe, quebras viram espaço, texto vazio não pinta nada. `cor` `None` é `--t`; `italico` é o
/// `tr.ocul td.lin` (o número da linha oculta). O `painter` deve vir com o recorte da célula.
pub fn pintar_texto_alinhado(
    painter: &Painter,
    p: &Paleta,
    conteudo: Rect,
    texto: &str,
    alinhamento: Alinhamento,
    cor: Option<Color32>,
    italico: bool,
) {
    if texto.is_empty() {
        return;
    }
    let cor = cor.unwrap_or(p.estudio.t);
    let peso = if italico { Peso::Italico } else { Peso::Normal };
    let mut job = LayoutJob::single_section(uma_linha(texto).into_owned(), formato(fontes::fonte(12.0, peso), cor));
    job.wrap = TextWrapping {
        max_width: conteudo.width().max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    job.break_on_newline = false;
    let g = painter.layout_job(job);
    let x = match alinhamento {
        Alinhamento::Esquerda => conteudo.left(),
        Alinhamento::Direita => conteudo.right() - g.size().x,
        Alinhamento::Centro => conteudo.center().x - g.size().x / 2.0,
    };
    painter.galley(pos2(x.max(conteudo.left()), conteudo.center().y - g.size().y / 2.0), g, cor);
}

/// A faixa de `td.alt` à esquerda da célula alterada (`box-shadow:inset 3px 0 0 --acct`).
pub const LARGURA_FAIXA_ALTERADA: f32 = 3.0;

/// A célula alterada e ainda não salva (`.grade td.alt{box-shadow:inset 3px 0 0 --acct;background:--tint}`): o fundo
/// `--tint` na `celula` inteira e a faixa de [`LARGURA_FAIXA_ALTERADA`] em `--acct` na borda esquerda. Pinte antes do
/// texto ([`pintar_texto_de_celula`]), com o `painter` recortado na célula (o `CelulaVisivel::recorte`).
pub fn pintar_celula_alterada(painter: &Painter, p: &Paleta, celula: Rect) {
    painter.rect_filled(celula, 0.0, p.estudio.tint);
    painter.rect_filled(
        Rect::from_min_size(celula.min, vec2(LARGURA_FAIXA_ALTERADA.min(celula.width()), celula.height())),
        0.0,
        p.estudio.acct,
    );
}

/// [`pintar_celula`] da coluna da chave (`.grade td.chave`: `--acct` 600; o NULL fica no itálico `--t3`).
pub fn pintar_celula_chave(painter: &Painter, p: &Paleta, conteudo: Rect, c: &Celula, numerica: bool) {
    pintar_celula_com(painter, p, conteudo, c, numerica, true);
}

fn pintar_celula_com(painter: &Painter, p: &Paleta, conteudo: Rect, c: &Celula, numerica: bool, chave: bool) {
    if let Celula::Blob { .. } = c {
        let rotulo = texto_exibicao(c);
        let t = tamanho_chip_em(painter, &rotulo);
        let x = if numerica { conteudo.right() - t.x } else { conteudo.left() };
        let r = Rect::from_min_size(pos2(x, conteudo.center().y - t.y / 2.0), t);
        pintar_chip_em(painter, p, r, &rotulo, false);
        return;
    }
    let Some(job) = job_celula(c, p, conteudo.width(), chave) else { return };
    let g = painter.layout_job(job);
    // `td.num`: a coluna inteira alinha à direita (o NULL e o "vazio" também)
    let x = if numerica { conteudo.right() - g.size().x } else { conteudo.left() };
    painter.galley(pos2(x, conteudo.center().y - g.size().y / 2.0), g, p.fg);
}

/// O número da linha (`.grade td.lin`): 12px em `--t3`, à direita do conteúdo e centrado na vertical. O fundo
/// `--p` da coluna é pintado pelo núcleo ([`NucleoGrade::com_coluna_numero`]).
pub fn pintar_numero_linha(painter: &Painter, p: &Paleta, conteudo: Rect, numero: &str) {
    let mut job =
        LayoutJob::single_section(numero.to_owned(), formato(fontes::fonte(12.0, Peso::Normal), p.estudio.t3));
    job.wrap = TextWrapping { max_width: conteudo.width().max(1.0), max_rows: 1, ..TextWrapping::default() };
    let g = painter.layout_job(job);
    painter.galley(pos2(conteudo.right() - g.size().x, conteudo.center().y - g.size().y / 2.0), g, p.estudio.t3);
}

/// Espaço entre o nome e o tipo no `th` (`.grade th .tp{margin-left:6px}`).
const ESPACO_TIPO: f32 = 6.0;

/// O nome de um `th` (11.5px / 600), numa linha.
fn galley_nome_th(painter: &Painter, nome: &str, cor_nome: Color32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(nome.to_owned(), formato(fontes::fonte(11.5, Peso::Medio), cor_nome));
    job.break_on_newline = false;
    painter.layout_job(job)
}

/// Lado da seta da ordem (`.seta-ord{width:9px;height:9px}`).
pub const LADO_SETA_ORDEM: f32 = 9.0;
/// Espaço entre o nome e a seta da ordem (`.seta-ord{margin-left:5px}`).
pub const ESPACO_SETA_ORDEM: f32 = 5.0;

/// A seta da ordem desenhada (a `.seta-ord` da revisão de UI de 19/10/2026, no lugar do glifo ▲/▼ da fonte): o
/// triângulo `M5 1.5 9.2 8.5H.8z` da `viewBox` de 10 esticado em `rect` (9px nas pranchetas), com a ponta para cima
/// na crescente e para baixo na decrescente, cheio em `cor` (`--acct`; a seta clicável do CSV usa `--t3` e `--t` no
/// hover antes de ordenar).
pub fn pintar_seta_ordem(painter: &Painter, rect: Rect, ordem: Ordem, cor: Color32) {
    let ponto = |x: f32, y: f32| {
        let y = if ordem == Ordem::Crescente { y } else { 10.0 - y };
        pos2(rect.left() + x * rect.width() / 10.0, rect.top() + y * rect.height() / 10.0)
    };
    let pontos = vec![ponto(5.0, 1.5), ponto(9.2, 8.5), ponto(0.8, 8.5)];
    painter.add(egui::Shape::convex_polygon(pontos, cor, Stroke::NONE));
}

/// O tipo de um `th` (o `.tp`: 10px / 400, `letter-spacing:.03em`).
fn galley_tipo_th(painter: &Painter, tipo: &str, cor: Color32) -> Arc<Galley> {
    let mut f = formato(fontes::fonte(10.0, Peso::Normal), cor);
    f.extra_letter_spacing = 0.3;
    let mut job = LayoutJob::single_section(tipo.to_owned(), f);
    job.break_on_newline = false;
    painter.layout_job(job)
}

/// Pinta o fundo do cabeçalho grudado (o degradê `Cabecalho`, opaco, com as riscas presas à janela) e o fio
/// `--btb` de baixo. Serve também ao `th` da [`crate::pecas::tabela::Tabela`].
pub fn pintar_fundo_cabecalho(painter: &Painter, p: &Paleta, rect: Rect) {
    pintura::preencher(painter, p, rect, 0.0, Preenchimento::Cabecalho);
    pintura::riscas(painter, p, rect, 0.0);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.estudio.btb));
}

/// O que um `th` mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Th<'a> {
    /// Nome da coluna (11.5px / 600 em `--t2`, sem caixa alta).
    pub nome: &'a str,
    /// Tipo declarado, à direita do nome em 10px `--t3` (o `.tp`; aba Dados); `None` na Consulta SQL.
    pub tipo: Option<&'a str>,
    /// Coluna numérica: alinha à direita.
    pub numerica: bool,
    /// Seta de ordenação depois do nome ([`pintar_seta_ordem`], em `--acct`).
    pub ordem: Option<Ordem>,
    /// O nome é clicável (o `th` que ordena). O desenho é o mesmo do `th` comum (o Estúdio não tem o pontilhado
    /// embaixo do nome); o hover vem por [`Th::realce`].
    pub clicavel: bool,
    /// Hover do nome clicável: o nome em `--t`.
    pub realce: bool,
    /// A pastilha de tipo de 16px antes do nome, com 6px até ele (`.grade th .sk.s16`, etapa 3: a aba Dados; o tipo
    /// por extenso vai na dica).
    pub pastilha: Option<TipoColuna>,
    /// A coluna está selecionada (`th.selcol`: o nome em `--t`; o fundo `--sel` do `th` e o `--tint` das células vêm
    /// do núcleo, pelo gancho [`DesenhoGrade::th_selecionada`]).
    pub selecionada: bool,
    /// O nome vai no meio do `th` (a letra da coluna da planilha, `.xl th{text-align:center}`); [`Th::numerica`] vale
    /// primeiro.
    pub centrada: bool,
}

impl<'a> Th<'a> {
    pub fn new(nome: &'a str) -> Self {
        Th {
            nome,
            tipo: None,
            numerica: false,
            ordem: None,
            clicavel: false,
            realce: false,
            pastilha: None,
            selecionada: false,
            centrada: false,
        }
    }
}

/// Lado da pastilha do `th` ([`Th::pastilha`]).
pub const LADO_PASTILHA_TH: f32 = 16.0;
/// O que a pastilha soma à largura do texto de um `th`: os 16px dela e os 6px até o nome.
pub const LARGURA_PASTILHA_TH: f32 = LADO_PASTILHA_TH + ESPACO_TIPO;

/// Pinta o texto de um `th` no retângulo de conteúdo (sem o padding e sem os fios): a pastilha, o nome, a seta da
/// ordem e o tipo numa linha, centrada na vertical; numa coluna numérica, à direita (`th.num`), sem passar da
/// esquerda do conteúdo (o que não cabe é cortado pelo recorte do `painter`, como o `overflow:hidden` do `th`).
/// Devolve onde a pastilha ficou (para a dica com o tipo).
pub fn pintar_cabecalho_coluna(painter: &Painter, p: &Paleta, conteudo: Rect, th: &Th<'_>) -> Option<Rect> {
    let e = &p.estudio;
    let cor = if th.realce || th.selecionada { e.t } else { e.t2 };
    let g = galley_nome_th(painter, th.nome, cor);
    let gt = th.tipo.map(|t| galley_tipo_th(painter, t, e.t3));
    let antes = if th.pastilha.is_some() { LARGURA_PASTILHA_TH } else { 0.0 };
    let seta = if th.ordem.is_some() { ESPACO_SETA_ORDEM + LADO_SETA_ORDEM } else { 0.0 };
    let largura = antes + g.size().x + seta + gt.as_ref().map_or(0.0, |gt| ESPACO_TIPO + gt.size().x);
    let x0 = if th.numerica {
        (conteudo.right() - largura).max(conteudo.left())
    } else if th.centrada {
        (conteudo.center().x - largura / 2.0).max(conteudo.left())
    } else {
        conteudo.left()
    };
    let x = x0 + antes;
    let y = conteudo.center().y - g.size().y / 2.0;
    // o tipo e a seta na linha de base do nome (`vertical-align:baseline`; a seta é o `svg` com `vertical-align:0`)
    let base = y + linha_de_base(&g);
    let x_tipo = x + g.size().x + seta + ESPACO_TIPO;
    let pastilha = th.pastilha.map(|tipo| {
        let r = Rect::from_center_size(
            pos2(x0 + LADO_PASTILHA_TH / 2.0, conteudo.center().y),
            vec2(LADO_PASTILHA_TH, LADO_PASTILHA_TH),
        );
        icones::pintar(painter, r, Icone::Tipo(tipo), Color32::WHITE);
        r
    });
    if let Some(ordem) = th.ordem {
        let r = Rect::from_min_size(
            pos2(x + g.size().x + ESPACO_SETA_ORDEM, base - LADO_SETA_ORDEM),
            vec2(LADO_SETA_ORDEM, LADO_SETA_ORDEM),
        );
        pintar_seta_ordem(painter, r, ordem, e.acct);
    }
    painter.galley(pos2(x, y), g, cor);
    if let Some(gt) = gt {
        let y_tipo = base - linha_de_base(&gt);
        painter.galley(pos2(x_tipo, y_tipo), gt, e.t3);
    }
    pastilha
}

/// Largura natural do texto de um `th` (sem padding): o nome e, com o tipo, os 6px e o tipo à direita dele (sem a
/// seta da ordem, que soma [`ESPACO_SETA_ORDEM`] e [`LADO_SETA_ORDEM`]).
pub fn largura_cabecalho(painter: &Painter, nome: &str, tipo: Option<&str>) -> f32 {
    let n = galley_nome_th(painter, nome, Color32::PLACEHOLDER).size().x;
    n + tipo.map_or(0.0, |t| ESPACO_TIPO + galley_tipo_th(painter, t, Color32::PLACEHOLDER).size().x)
}

/// A célula ativa (`.grade td.ativa{outline:2px solid --acc; outline-offset:-2px}`): só o contorno de 2px por dentro
/// do retângulo (a célula com os fios dela), sem o halo interno (revisão de 17/10/2026, como as pranchetas).
pub fn pintar_celula_ativa(painter: &Painter, p: &Paleta, celula: Rect) {
    painter.rect_stroke(celula, 0.0, Stroke::new(2.0, p.estudio.acc), StrokeKind::Inside);
}

/// Padding esquerdo e direito da coluna `c` de `n` na tabela simples ([`crate::pecas::tabela::Tabela`],
/// `.lista-t th, .lista-t td{padding:0 12px}`: o mesmo em todas as colunas), que as telas usam para prever as
/// larguras dela. A grade usa [`PADDING_X`] (10px) e o fio de 1px da direita.
pub fn padding_coluna(_c: usize, _n: usize) -> (f32, f32) {
    (PADDING_TABELA, PADDING_TABELA)
}

/// Padding esquerdo e direito do conteúdo de uma coluna da grade: o `padding:0 10px` e o fio da direita (a coluna
/// da caixa de marcar não tem nenhum dos dois).
fn padding_grade(marcar: bool) -> (f32, f32) {
    if marcar { (0.0, 0.0) } else { (PADDING_X, PADDING_X + 1.0) }
}

/// Largura inicial de cada coluna: o maior entre o cabeçalho e as primeiras 200 linhas, até 320px (com o padding
/// e o fio).
pub fn larguras_iniciais(painter: &Painter, p: &Paleta, colunas: &[ColunaGrade], linhas: &[Vec<Celula>]) -> Vec<f32> {
    colunas
        .iter()
        .enumerate()
        .map(|(c, col)| {
            let (esq, dir) = padding_grade(false);
            let limite = LARGURA_MAX_COLUNA - esq - dir;
            let antes = if col.pastilha.is_some() { LARGURA_PASTILHA_TH } else { 0.0 };
            let mut maior = antes + largura_cabecalho(painter, &col.nome, col.tipo.as_deref());
            for linha in linhas.iter().take(LINHAS_MEDIDAS) {
                if maior >= limite {
                    break;
                }
                if let Some(cel) = linha.get(c) {
                    maior = maior.max(largura_natural(painter, p, cel));
                }
            }
            (maior.min(limite) + esq + dir).ceil().max(LARGURA_MIN_COLUNA)
        })
        .collect()
}

// ------------------------------------------------------------------ núcleo

/// Um intervalo retangular de células ([`NucleoGrade::intervalo`]): as linhas `linha0..=linha1` e as colunas
/// `coluna0..=coluna1` da grade, com as pontas incluídas e na ordem da grade (a de cima à esquerda primeiro).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intervalo {
    pub linha0: usize,
    pub linha1: usize,
    pub coluna0: usize,
    pub coluna1: usize,
}

impl Intervalo {
    /// O retângulo entre duas células, em qualquer ordem.
    pub fn entre(a: (usize, usize), b: (usize, usize)) -> Intervalo {
        Intervalo { linha0: a.0.min(b.0), linha1: a.0.max(b.0), coluna0: a.1.min(b.1), coluna1: a.1.max(b.1) }
    }

    pub fn contem(&self, linha: usize, coluna: usize) -> bool {
        (self.linha0..=self.linha1).contains(&linha) && (self.coluna0..=self.coluna1).contains(&coluna)
    }

    pub fn linhas(&self) -> Range<usize> {
        self.linha0..self.linha1 + 1
    }

    pub fn colunas(&self) -> Range<usize> {
        self.coluna0..self.coluna1 + 1
    }

    /// Quantas células ele tem.
    pub fn celulas(&self) -> u64 {
        self.linhas().len() as u64 * self.colunas().len() as u64
    }

    /// Mais de uma célula.
    pub fn varias(&self) -> bool {
        self.linha0 != self.linha1 || self.coluna0 != self.coluna1
    }
}

/// O texto do intervalo para a área de transferência, como o Excel copia ([`tabular::colunas::escrever_tsv`]:
/// tabulação entre as colunas, `\r\n` depois de cada linha, as aspas onde o campo pede). `texto` dá o texto de
/// cada célula (linha e coluna da grade); a célula sem texto vai vazia.
pub fn tsv_do_intervalo(intervalo: Intervalo, mut texto: impl FnMut(usize, usize) -> Option<String>) -> String {
    let linhas: Vec<Vec<String>> =
        intervalo.linhas().map(|l| intervalo.colunas().map(|c| texto(l, c).unwrap_or_default()).collect()).collect();
    tabular::colunas::escrever_tsv(&linhas)
}

/// A frase do aviso depois de copiar um intervalo: "3 linhas × 2 colunas copiadas.", "Copiado." numa célula só.
pub fn frase_do_intervalo_copiado(intervalo: Intervalo) -> String {
    if !intervalo.varias() {
        return "Copiado.".to_string();
    }
    let (l, c) = (intervalo.linhas().len(), intervalo.colunas().len());
    let linhas = if l == 1 { "1 linha".to_string() } else { format!("{} linhas", formatar_numero(l as u64)) };
    let colunas = if c == 1 { "1 coluna".to_string() } else { format!("{} colunas", formatar_numero(c as u64)) };
    format!("{linhas} × {colunas} copiadas.")
}

/// Regras de teclado do [`NucleoGrade`] (valem com o corpo da grade focado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Navegacao {
    /// A grade só leitura da Consulta SQL: setas, PageUp/PageDown de uma tela, Home/End na linha,
    /// Ctrl+Home/Ctrl+End no começo e no fim da grade; Tab sai da grade.
    #[default]
    Consulta,
    /// A aba Dados: setas (a coluna passa para a linha seguinte ou anterior, sem dar a
    /// volta na grade), PageUp/PageDown de 10 linhas, Home/End na linha, Tab/Shift+Tab de célula em célula (na
    /// ponta, o foco sai da grade), Enter/F2 pedem a edição, Delete e espaço viram pedidos, e uma tecla de texto
    /// começa a edição com ela ([`RespostaNucleo::texto_digitado`]).
    Dados,
}

/// Uma célula visível, como o núcleo a entrega ao [`DesenhoGrade`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CelulaVisivel {
    pub linha: usize,
    pub coluna: usize,
    /// A célula inteira (do começo da coluna até o fio da direita, inclusive; sem o fio de baixo).
    pub celula: Rect,
    /// O conteúdo: a célula sem o `padding:0 10px` e sem o fio da direita (a da caixa de marcar é a célula toda).
    pub conteudo: Rect,
    /// O recorte da célula na parte visível da grade (use em `ui.painter().with_clip_rect(recorte)`).
    pub recorte: Rect,
    /// É a célula ativa (o contorno `--acc` e o brilho são pintados pelo núcleo, depois da linha).
    pub ativa: bool,
}

/// O `th` de uma coluna visível.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CabecalhoVisivel {
    pub coluna: usize,
    /// O `th` inteiro (com os fios).
    pub th: Rect,
    /// O conteúdo: o `th` sem o `padding:0 10px`, o fio da direita e o de baixo (o texto vai centrado na vertical).
    pub conteudo: Rect,
    /// O recorte do `th` na parte visível.
    pub recorte: Rect,
}

/// Uma célula mesclada visível, como o núcleo a entrega ao [`DesenhoGrade::celula_mesclada`]: a de cima à esquerda, com
/// o retângulo da mescla inteira.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CelulaMesclada {
    /// A linha e a coluna da célula de cima à esquerda (onde está o texto).
    pub linha: usize,
    pub coluna: usize,
    /// A mescla inteira (do começo da primeira coluna até o fio da direita da última, sem o fio de baixo). Numa mescla
    /// que cruza a emenda do painel congelado, a parte que rola se junta à presa como se a grade estivesse no começo.
    pub celula: Rect,
    /// O conteúdo: a mescla sem o `padding:0 10px` e sem o fio da direita.
    pub conteudo: Rect,
    /// O recorte da mescla na parte da grade que está sendo pintada (use em `ui.painter().with_clip_rect(recorte)`: uma
    /// mescla que cruza a emenda do congelado é pintada uma vez por parte).
    pub recorte: Rect,
    /// A célula ativa está dentro da mescla (o contorno `--acc` de 2px na mescla toda é do núcleo).
    pub ativa: bool,
}

/// Os ganchos do [`NucleoGrade`]: o que cada célula e cada `th` mostram. O `ui` é o do conteúdo rolável: pinte
/// com `ui.painter().with_clip_rect(c.recorte)` ou ponha widgets (caixa de marcar, `nome_abre`, o
/// `Entrada::compacta` da edição) num filho em `c.conteudo` — **com id explícito** (por linha e coluna), porque
/// as células que aparecem mudam com a rolagem. Widgets nas células ganham do clique do corpo.
pub trait DesenhoGrade {
    /// O conteúdo de uma célula visível (o fundo da linha e os fios já estão pintados).
    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel);
    /// Uma célula mesclada ([`NucleoGrade::com_mesclas`]): o conteúdo da célula de cima à esquerda no retângulo da mescla
    /// inteira (as outras células dela não chamam o [`DesenhoGrade::celula`], e o fio de dentro não é pintado). Por
    /// padrão não pinta nada.
    fn celula_mesclada(&mut self, _ui: &mut Ui, _c: &CelulaMesclada) {}
    /// O `th` de uma coluna visível (o fundo `Cabecalho` e os fios já estão pintados; as alças vêm depois).
    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel);
    /// Fundo da linha inteira antes das células (a linha marcada da aba Dados, `tr.mar td`: `p.estudio.sel`). A
    /// linha com fundo próprio não ganha o `--selh` do hover (no CSS a regra da marcada vem depois).
    fn fundo_linha(&mut self, _linha: usize) -> Option<Color32> {
        None
    }
    /// A dica (o `title`) da célula sob o ponteiro.
    fn dica(&mut self, _linha: usize, _coluna: usize) -> Option<String> {
        None
    }
    /// A coluna está selecionada (`th.selcol` e `td.selcol`, prancheta 16)? O núcleo pinta o fundo `--sel` no `th` e o
    /// `--tint` nas células dela; o nome em `--t` é do [`Th::selecionada`], que o gancho do `th` liga.
    fn th_selecionada(&mut self, _coluna: usize) -> bool {
        false
    }
}

/// As teclas que estavam apertadas num clique (para somar colunas: Ctrl soma uma, Shift estende).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Toques {
    /// Ctrl (ou Cmd): soma ou tira uma coluna da seleção.
    pub ctrl: bool,
    /// Shift: estende a seleção até a coluna.
    pub shift: bool,
}

/// O que aconteceu no núcleo neste quadro.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RespostaNucleo {
    /// A célula ativa mudou (clique ou teclado).
    pub selecao_mudou: bool,
    /// O intervalo mudou ([`NucleoGrade::intervalo`]: o arrasto, o Shift+clique, o Shift com as setas).
    pub intervalo_mudou: bool,
    /// Clique simples numa célula (que passou a ser a ativa).
    pub clicou: Option<(usize, usize)>,
    /// Pedido de edição da célula: duplo clique, ou Enter/F2 na célula ativa.
    pub editar: Option<(usize, usize)>,
    /// Tecla de texto com a célula ativa ([`Navegacao::Dados`]): começa a edição com ela, como numa planilha.
    pub texto_digitado: Option<String>,
    /// Ctrl+C com a célula ativa (com um intervalo de várias, quem chama copia o [`NucleoGrade::intervalo`]).
    pub copiar: bool,
    /// Ctrl+Shift+C com a célula ativa: o intervalo com os nomes das colunas na primeira linha
    /// ([`tsv_do_intervalo_com_cabecalho`]).
    pub copiar_com_cabecalho: bool,
    /// O menu da célula foi pedido neste quadro (o clique direito numa célula ou o Shift+F10): ele abre no
    /// [`menu_da_celula`], que a tela chama todo quadro depois do [`NucleoGrade::mostrar`].
    pub menu_celula: bool,
    /// Delete com o corpo focado ([`Navegacao::Dados`]: excluir as marcadas).
    pub excluir: bool,
    /// Espaço com o corpo focado ([`Navegacao::Dados`]: marcar a linha ativa).
    pub espaco: bool,
    /// A linha sob o ponteiro (a que ganhou o hover).
    pub linha_sob_ponteiro: Option<usize>,
    /// Clique com o botão principal no `th` de uma coluna navegável (a que não é a da caixa de marcar nem a do número
    /// da linha), com as teclas apertadas: o CSV seleciona a coluna (Ctrl e Shift somam). Vale onde o gancho do `th`
    /// não pôs um widget que ficou com o clique (a seta da ordem da aba Dados).
    pub clicou_cabecalho: Option<(usize, Toques)>,
    /// Clique com o botão secundário no `th` de uma coluna navegável: o menu do cabeçalho.
    pub menu_cabecalho: Option<usize>,
    /// Clique com o botão principal no número de uma linha (a coluna de [`NucleoGrade::com_coluna_numero`]), com as teclas
    /// apertadas: a planilha seleciona a linha (Ctrl e Shift somam). O número não é uma célula ativável: o clique nele não
    /// é o [`RespostaNucleo::clicou`].
    pub clicou_numero_da_linha: Option<(usize, Toques)>,
}

/// O núcleo de uma grade com rolagem virtual: só as linhas e colunas visíveis são desenhadas (100 mil linhas
/// rolam sem custo), o cabeçalho fica grudado no topo, as colunas têm largura ajustável arrastando a borda do
/// `th` (as que não são fixas), a célula ativa anda com o teclado ([`Navegacao`]) e o clique, e a linha sob o
/// ponteiro ganha o `--selh`. Linhas de 28px com os fios `--b` embaixo e entre as colunas. O conteúdo vem dos
/// ganchos de [`DesenhoGrade`]. Guarde o núcleo junto com a página: ele é o estado (larguras, célula ativa).
#[derive(Debug, Clone)]
pub struct NucleoGrade {
    id: Id,
    n_linhas: usize,
    larguras: Vec<f32>,
    fixas: Vec<bool>,
    altura_cabecalho: f32,
    navegacao: Navegacao,
    navegaveis: Range<usize>,
    /// A coluna da caixa de marcar (`selc`) e a do número da linha (`lin`).
    coluna_marcar: Option<usize>,
    coluna_numero: Option<usize>,
    selecao: Option<(usize, usize)>,
    rolar_para_selecao: bool,
    /// A outra ponta do intervalo (a célula ativa é a âncora, onde ele começou); None: só a célula ativa.
    extremo: Option<(usize, usize)>,
    rolar_para_extremo: bool,
    /// O painel congelado pedido ([`NucleoGrade::com_congelado`]) e o que valeu no último quadro (a trava de 60%).
    congelado: Option<Congelado>,
    congelado_efetivo: Congelado,
    /// As mesclas ([`NucleoGrade::com_mesclas`]); compartilhadas porque o índice se monta fora da tela.
    mesclas: Option<Arc<IndiceDeMesclas>>,
    /// A menor largura que a alça deixa: 48px, ou 24px com as larguras do arquivo.
    largura_minima: f32,
    larguras_externas: bool,
    /// O cabeçalho sem as riscas (a planilha: `.xl th{background:transparent}` sobre o degradê da faixa).
    cabecalho_liso: bool,
    /// O Shift+F10 pediu o menu da célula; o [`NucleoGrade::mostrar`] põe o menu embaixo da célula ativa.
    menu_pelo_teclado: bool,
    /// O menu da célula pedido neste quadro, até o [`menu_da_celula`] abri-lo.
    pedido_menu: Option<menu_celula::PedidoMenu>,
}

/// O Ctrl+Shift+C com o foco na grade (só com esses modificadores: o AltGr, que chega como Ctrl+Alt, não conta). O
/// egui-winit entrega o Ctrl+Shift+C como `Event::Copy`, sem o `Event::Key`; os dois valem e são consumidos (na grade
/// com o foco, o atalho é o "Copiar com cabeçalho", não o "Copiar caminho" do Explorador).
fn consumir_copiar_com_cabecalho(ctx: &egui::Context) -> bool {
    let atalho = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    ctx.input_mut(|i| {
        let agora = i.modifiers;
        let e_o_atalho = |e: &Event| match e {
            Event::Key { key: Key::C, pressed: true, modifiers, .. } => modifiers.matches_exact(atalho),
            Event::Copy => agora.matches_exact(atalho),
            _ => false,
        };
        if !i.events.iter().any(&e_o_atalho) {
            return false;
        }
        i.events.retain(|e| !e_o_atalho(e));
        true
    })
}

impl NucleoGrade {
    /// `larguras`: uma por coluna (veja [`larguras_iniciais`]); todas ajustáveis e navegáveis, cabeçalho de 30px,
    /// teclado da Consulta SQL.
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, n_linhas: usize, larguras: Vec<f32>) -> Self {
        let n = larguras.len();
        NucleoGrade {
            id: Id::new(("databox::grade", id_salt)),
            n_linhas,
            fixas: vec![false; n],
            larguras,
            altura_cabecalho: ALTURA_CABECALHO,
            navegacao: Navegacao::Consulta,
            navegaveis: 0..n,
            coluna_marcar: None,
            coluna_numero: None,
            selecao: None,
            rolar_para_selecao: false,
            extremo: None,
            rolar_para_extremo: false,
            congelado: None,
            congelado_efetivo: Congelado::default(),
            mesclas: None,
            largura_minima: LARGURA_MIN_COLUNA,
            larguras_externas: false,
            cabecalho_liso: false,
            menu_pelo_teclado: false,
            pedido_menu: None,
        }
    }

    /// O cabeçalho só com o degradê da faixa, sem as riscas (o `th` da planilha, prancheta 17:
    /// `.xl th{background:transparent}`).
    pub fn com_cabecalho_liso(mut self) -> Self {
        self.cabecalho_liso = true;
        self
    }

    /// As larguras que vêm de fora (as do arquivo, [`larguras_do_arquivo`]), no lugar das iniciais pela medida do texto
    /// ([`larguras_iniciais`]): a alça de redimensionar continua, e a coluna pode ir até [`LARGURA_MIN_ARQUIVO`] (24px).
    pub fn com_larguras_externas(mut self, larguras: Vec<f32>) -> Self {
        self.definir_larguras(larguras);
        self.largura_minima = LARGURA_MIN_ARQUIVO;
        self.larguras_externas = true;
        self
    }

    /// As larguras vieram de fora ([`NucleoGrade::com_larguras_externas`])?
    pub fn larguras_externas(&self) -> bool {
        self.larguras_externas
    }

    /// O painel congelado (decisão 10 da etapa 6): as primeiras `linhas` linhas ficam sob o cabeçalho, e as primeiras
    /// `colunas` colunas de dados (as navegáveis: depois da caixa de marcar e do número da linha, que ficam presos junto)
    /// grudam à esquerda quando a grade rola, com o fio de [`LARGURA_EMENDA`] (2px) em `--b2` na emenda. Se a área
    /// congelada ocupa mais de [`FRACAO_MAXIMA_CONGELADA`] (60%) da altura ou da largura que se vê, o congelamento daquela
    /// direção é ignorado naquele quadro, e a rolagem nunca prende. Com `(0, 0)` só as colunas especiais ficam presas,
    /// sem fio. Sem esta chamada, nada muda: as grades da aba Dados, do CSV e da Consulta rolam inteiras.
    pub fn com_congelado(mut self, linhas: usize, colunas: usize) -> Self {
        self.congelado = Some(Congelado { linhas, colunas });
        self
    }

    /// Troca (ou tira) o painel congelado.
    pub fn definir_congelado(&mut self, congelado: Option<Congelado>) {
        self.congelado = congelado;
    }

    /// O painel congelado pedido.
    pub fn congelado(&self) -> Option<Congelado> {
        self.congelado
    }

    /// O painel congelado que valeu no último quadro: `linhas` e `colunas` são as linhas congeladas e as colunas presas
    /// (as especiais contam), ou zero onde a trava de 60% ignorou o congelamento.
    pub fn congelado_efetivo(&self) -> Congelado {
        self.congelado_efetivo
    }

    /// As células mescladas, no [`IndiceDeMesclas`]: a de cima à esquerda pinta o conteúdo no retângulo todo
    /// ([`DesenhoGrade::celula_mesclada`]), as outras somem (sem o fundo e sem o fio de dentro), o clique em qualquer
    /// uma ativa a de cima à esquerda e a ativa contorna a mescla inteira.
    pub fn com_mesclas(mut self, mesclas: Arc<IndiceDeMesclas>) -> Self {
        self.mesclas = Some(mesclas);
        self
    }

    /// Troca (ou tira) as mesclas: o índice de uma planilha só fica completo depois da leitura.
    pub fn definir_mesclas(&mut self, mesclas: Option<Arc<IndiceDeMesclas>>) {
        self.mesclas = mesclas;
    }

    pub fn mesclas(&self) -> Option<&Arc<IndiceDeMesclas>> {
        self.mesclas.as_ref()
    }

    /// A coluna da caixa de marcar (`.grade th.selc, td.selc`: [`LARGURA_COLUNA_MARCAR`], sem padding — o
    /// `conteudo` é a célula toda, para centrar a caixa — e sem o fio à direita). Fica sem alça; tire-a da
    /// navegação com [`NucleoGrade::com_navegaveis`].
    pub fn com_coluna_marcar(mut self, coluna: usize) -> Self {
        self.coluna_marcar = Some(coluna);
        self.com_fixas(&[coluna])
    }

    /// A coluna do número da linha (`.grade td.lin`): o núcleo pinta o fundo `--p` dela (o hover e a linha marcada
    /// ficam no lugar dele); o número vem do gancho, com [`pintar_numero_linha`]. Fica sem alça.
    pub fn com_coluna_numero(mut self, coluna: usize) -> Self {
        self.coluna_numero = Some(coluna);
        self.com_fixas(&[coluna])
    }

    /// As regras de teclado.
    pub fn com_navegacao(mut self, navegacao: Navegacao) -> Self {
        self.navegacao = navegacao;
        self
    }

    /// A altura do cabeçalho ([`altura_cabecalho`]).
    pub fn com_altura_cabecalho(mut self, altura: f32) -> Self {
        self.altura_cabecalho = altura;
        self
    }

    /// As colunas em que a célula ativa pode ficar (na aba Dados, as de dados: sem a da caixa de marcar e a do
    /// número da linha). A navegação pula as outras.
    pub fn com_navegaveis(mut self, colunas: Range<usize>) -> Self {
        let n = self.larguras.len();
        self.navegaveis = colunas.start.min(n)..colunas.end.min(n);
        self
    }

    /// Colunas sem alça de redimensionar (a da caixa de marcar e a do número da linha).
    pub fn com_fixas(mut self, colunas: &[usize]) -> Self {
        for &c in colunas {
            if let Some(f) = self.fixas.get_mut(c) {
                *f = true;
            }
        }
        self
    }

    pub fn id(&self) -> Id {
        self.id
    }

    /// Id do corpo da grade (o que recebe o foco do teclado).
    pub fn id_corpo(&self) -> Id {
        self.id.with("corpo")
    }

    pub fn n_linhas(&self) -> usize {
        self.n_linhas
    }

    pub fn n_colunas(&self) -> usize {
        self.larguras.len()
    }

    /// Troca o número de linhas (outra página): a célula ativa fica, se ainda existir.
    pub fn definir_linhas(&mut self, n_linhas: usize) {
        self.n_linhas = n_linhas;
        self.selecao = self.selecao.filter(|&(l, _)| l < n_linhas);
        self.extremo = self.extremo.filter(|&(l, _)| l < n_linhas && self.selecao.is_some());
    }

    pub fn larguras(&self) -> &[f32] {
        &self.larguras
    }

    /// Troca todas as larguras (o número de colunas também; as novas colunas são ajustáveis e navegáveis).
    pub fn definir_larguras(&mut self, larguras: Vec<f32>) {
        let n = larguras.len();
        self.fixas.resize(n, false);
        if self.navegaveis.end > n || self.navegaveis.is_empty() {
            self.navegaveis = self.navegaveis.start.min(n)..n;
        }
        self.larguras = larguras;
        self.selecao = self.selecao.filter(|&(_, c)| c < n);
        self.extremo = self.extremo.filter(|&(_, c)| c < n && self.selecao.is_some());
    }

    /// Troca a largura de uma coluna (mínimo 48px; 24px com as larguras do arquivo).
    pub fn definir_largura(&mut self, coluna: usize, largura: f32) {
        if let Some(l) = self.larguras.get_mut(coluna) {
            *l = largura.max(self.largura_minima);
        }
    }

    pub fn altura_do_cabecalho(&self) -> f32 {
        self.altura_cabecalho
    }

    /// A célula ativa (linha, coluna).
    pub fn selecao(&self) -> Option<(usize, usize)> {
        self.selecao
    }

    /// Escolhe a célula ativa (e rola até ela), sem intervalo. Fora da grade, ou numa coluna que não é navegável, não
    /// escolhe.
    pub fn selecionar(&mut self, celula: Option<(usize, usize)>) {
        self.selecao = celula.filter(|&(l, c)| l < self.n_linhas && self.navegaveis.contains(&c));
        self.rolar_para_selecao = self.selecao.is_some();
        self.extremo = None;
    }

    /// O intervalo entre a célula ativa (onde ele começou) e a outra ponta: o arrasto com o mouse, o Shift+clique e o
    /// Shift com as setas o estendem; sem nada disso, é a própria célula ativa. None sem célula ativa.
    pub fn intervalo(&self) -> Option<Intervalo> {
        let a = self.selecao?;
        Some(Intervalo::entre(a, self.extremo.unwrap_or(a)))
    }

    /// Desfaz o intervalo (fica só a célula ativa), sem rolar. Devolve se havia um.
    pub fn recolher_intervalo(&mut self) -> bool {
        self.extremo.take().is_some()
    }

    /// Estende o intervalo da célula ativa até `celula` (presa à grade e às colunas navegáveis); sem célula ativa, ela
    /// passa a ser a ativa. Devolve se mudou.
    pub fn estender(&mut self, celula: (usize, usize)) -> bool {
        if self.n_linhas == 0 || self.navegaveis.is_empty() {
            return false;
        }
        let l = celula.0.min(self.n_linhas - 1);
        let c = celula.1.clamp(self.navegaveis.start, self.navegaveis.end - 1);
        let Some(a) = self.selecao else {
            self.selecionar(Some((l, c)));
            return true;
        };
        let novo = (a != (l, c)).then_some((l, c));
        let mudou = novo != self.extremo;
        self.extremo = novo;
        self.rolar_para_extremo |= mudou;
        mudou
    }

    /// Passa o foco do teclado para o corpo da grade (depois de gravar a edição no lugar com Enter).
    pub fn focar(&self, ui: &Ui) {
        ui.memory_mut(|m| m.request_focus(self.id_corpo()));
    }

    /// A célula está na ponta da grade para o Tab (a última com Tab, a primeira com Shift+Tab)?
    pub fn na_ponta(&self, voltando: bool) -> bool {
        let (l, c) = self.selecao.unwrap_or((0, self.navegaveis.start));
        if voltando {
            l == 0 && c <= self.navegaveis.start
        } else {
            l + 1 >= self.n_linhas && c + 1 >= self.navegaveis.end
        }
    }

    /// Move a célula ativa: o passo na coluna passa para a linha seguinte (ou
    /// anterior), mas não dá a volta na grade. Devolve se mudou.
    pub fn mover(&mut self, dl: isize, dc: isize) -> bool {
        if self.n_linhas == 0 || self.navegaveis.is_empty() {
            return false;
        }
        let (c0, c1) = (self.navegaveis.start as isize, self.navegaveis.end as isize);
        let (l, c) = self.selecao.unwrap_or((0, self.navegaveis.start));
        let (mut l, mut c) = (l as isize, c as isize + dc);
        let ultima = self.n_linhas as isize - 1;
        if c >= c1 {
            if l >= ultima {
                return false;
            }
            c = c0;
            l += 1;
        }
        if c < c0 {
            if l <= 0 {
                return false;
            }
            c = c1 - 1;
            l -= 1;
        }
        l = (l + dl).clamp(0, ultima);
        let nova = Some((l as usize, c as usize));
        let mudou = nova != self.selecao || self.extremo.is_some();
        self.selecao = nova;
        self.extremo = None;
        self.rolar_para_selecao = true;
        mudou
    }

    fn teclado(&mut self, ui: &Ui, altura_max: f32, resposta: &mut RespostaNucleo) {
        let n_lin = self.n_linhas;
        if self.navegaveis.is_empty() || n_lin == 0 {
            return;
        }
        let dados = self.navegacao == Navegacao::Dados;
        let id_corpo = self.id_corpo();
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id_corpo,
                // com um intervalo, o Esc é da grade (desfaz o intervalo) e não tira o foco dela
                EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    tab: dados,
                    escape: self.extremo.is_some(),
                },
            )
        });
        let (c0, c1) = (self.navegaveis.start, self.navegaveis.end - 1);
        if consumir_shift_f10(ui.ctx()) {
            // o menu nasce na célula ativa (sem ela, na primeira)
            if self.selecao.is_none() {
                self.selecao = Some((0, c0));
                self.rolar_para_selecao = true;
            }
            self.menu_pelo_teclado = true;
        }
        if consumir_copiar_com_cabecalho(ui.ctx()) {
            resposta.copiar_com_cabecalho = true;
        }
        let pagina = if dados {
            PASSO_PAGINA_DADOS
        } else {
            ((altura_max - self.altura_cabecalho) / ALTURA_LINHA).floor().max(1.0) as usize
        };
        let eventos: Vec<Event> = ui.input(|i| i.events.clone());
        let antes = self.selecao;
        let extremo_antes = self.extremo;
        let mut esc = false;
        for e in &eventos {
            match e {
                Event::Key { key, pressed: true, modifiers, .. } => {
                    if modifiers.alt || modifiers.mac_cmd {
                        continue;
                    }
                    let (l, c) = self.selecao.unwrap_or((0, c0));
                    // cada tecla navegável move; as outras não desfazem o que as anteriores moveram
                    let mut nova: Option<(usize, usize)> = None;
                    let ctrl = modifiers.ctrl || modifiers.command;
                    // com o Shift, as setas, o PageUp/PageDown e o Home/End estendem o intervalo a partir da outra
                    // ponta (com o Ctrl, até a borda da grade); o Shift+Tab continua sendo o Tab de volta
                    if modifiers.shift
                        && let Some(alvo) = self.alvo_do_shift(*key, ctrl, pagina, (c0, c1))
                    {
                        self.estender(alvo);
                        continue;
                    }
                    if *key == Key::Escape && self.extremo.is_some() {
                        self.extremo = None;
                        esc = true;
                        continue;
                    }
                    if dados {
                        match key {
                            Key::ArrowDown if !ctrl => {
                                self.mover(1, 0);
                            }
                            Key::ArrowUp if !ctrl => {
                                self.mover(-1, 0);
                            }
                            Key::ArrowRight if !ctrl => {
                                self.mover(0, 1);
                            }
                            Key::ArrowLeft if !ctrl => {
                                self.mover(0, -1);
                            }
                            Key::PageDown if !ctrl => {
                                self.mover(pagina as isize, 0);
                            }
                            Key::PageUp if !ctrl => {
                                self.mover(-(pagina as isize), 0);
                            }
                            Key::Home if !ctrl => nova = Some((l, c0)),
                            Key::End if !ctrl => nova = Some((l, c1)),
                            Key::Home => nova = Some((0, c)),
                            Key::End => nova = Some((n_lin.saturating_sub(1), c)),
                            Key::Tab if !ctrl => {
                                if self.na_ponta(modifiers.shift) {
                                    // na ponta, o Tab deixa o foco sair da grade
                                    let direcao =
                                        if modifiers.shift { FocusDirection::Previous } else { FocusDirection::Next };
                                    ui.memory_mut(|m| m.move_focus(direcao));
                                } else {
                                    self.mover(0, if modifiers.shift { -1 } else { 1 });
                                }
                            }
                            Key::Enter | Key::F2 => resposta.editar = Some((l, c)),
                            Key::Delete => resposta.excluir = true,
                            Key::Space if !ctrl => resposta.espaco = true,
                            _ => {}
                        }
                    } else {
                        match key {
                            Key::ArrowDown => nova = Some(((l + 1).min(n_lin - 1), c)),
                            Key::ArrowUp => nova = Some((l.saturating_sub(1), c)),
                            Key::ArrowRight => nova = Some((l, (c + 1).min(c1))),
                            Key::ArrowLeft => nova = Some((l, c.saturating_sub(1).max(c0))),
                            // Ctrl+PageUp/PageDown ficam livres: a planilha troca de folha com eles
                            Key::PageDown if !ctrl => nova = Some(((l + pagina).min(n_lin - 1), c)),
                            Key::PageUp if !ctrl => nova = Some((l.saturating_sub(pagina), c)),
                            Key::Home if modifiers.command => nova = Some((0, c)),
                            Key::End if modifiers.command => nova = Some((n_lin - 1, c)),
                            Key::Home => nova = Some((l, c0)),
                            Key::End => nova = Some((l, c1)),
                            _ => {}
                        }
                    }
                    if let Some(n) = nova {
                        self.selecao = Some(n);
                        self.extremo = None;
                        self.rolar_para_selecao = true;
                    }
                }
                Event::Text(t) if dados && t != " " && !t.is_empty() && self.selecao.is_some() => {
                    resposta.texto_digitado.get_or_insert_with(String::new).push_str(t);
                }
                Event::Copy => resposta.copiar = true,
                _ => {}
            }
        }
        if self.selecao != antes {
            resposta.selecao_mudou = true;
        }
        if self.extremo != extremo_antes {
            resposta.intervalo_mudou = true;
        }
        if esc {
            // o Esc que desfez o intervalo não vai a mais ninguém
            ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        }
    }

    /// Onde o Shift com uma tecla leva a outra ponta do intervalo (presa à grade depois, pelo
    /// [`NucleoGrade::estender`]); None numa tecla que não estende.
    fn alvo_do_shift(&self, tecla: Key, ctrl: bool, pagina: usize, (c0, c1): (usize, usize)) -> Option<(usize, usize)> {
        let (l, c) = self.extremo.or(self.selecao).unwrap_or((0, c0));
        let ultima = self.n_linhas.saturating_sub(1);
        Some(match tecla {
            Key::ArrowDown if ctrl => (ultima, c),
            Key::ArrowDown => (l + 1, c),
            Key::ArrowUp if ctrl => (0, c),
            Key::ArrowUp => (l.saturating_sub(1), c),
            Key::ArrowRight if ctrl => (l, c1),
            Key::ArrowRight => (l, c + 1),
            Key::ArrowLeft if ctrl => (l, c0),
            Key::ArrowLeft => (l, c.saturating_sub(1)),
            Key::PageDown if !ctrl => (l + pagina, c),
            Key::PageUp if !ctrl => (l.saturating_sub(pagina), c),
            Key::Home if ctrl => (0, c0),
            Key::Home => (l, c0),
            Key::End if ctrl => (ultima, c1),
            Key::End => (l, c1),
            _ => return None,
        })
    }

    /// Desenha a grade na largura disponível, com no máximo `altura_max` de altura (encolhe quando há poucas
    /// linhas). O espaço que sobra à direita das colunas fica vazio (a coluna "sobra" do kit).
    ///
    /// Com o painel congelado ([`NucleoGrade::com_congelado`]) a grade se pinta em até quatro partes, cada uma com o seu
    /// recorte: as linhas que rolam × as colunas que rolam, as que rolam × as presas, as congeladas × as que rolam e as
    /// congeladas × as presas. Cada linha tem um y só e cada coluna um x só (o das presas e o das congeladas não mexe
    /// com a rolagem); só o recorte muda. Sem painel congelado e sem mesclas é uma parte só, a de sempre.
    pub fn mostrar(&mut self, ui: &mut Ui, altura_max: f32, desenho: &mut dyn DesenhoGrade) -> RespostaNucleo {
        let mut resposta = RespostaNucleo::default();
        let p = paleta(ui);
        self.pedido_menu = None;
        // com o menu da célula aberto, as teclas são dele (as setas andam pelos itens, o Esc fecha)
        if ui.memory(|m| m.has_focus(self.id_corpo())) && !menu_da_celula_aberto(ui.ctx(), self) {
            self.teclado(ui, altura_max, &mut resposta);
        }
        let larguras = self.larguras.clone();
        let fixas = self.fixas.clone();
        let navegaveis = self.navegaveis.clone();
        let (coluna_marcar, coluna_numero) = (self.coluna_marcar, self.coluna_numero);
        let n_col = larguras.len();
        let n_lin = self.n_linhas;
        let alt_cab = self.altura_cabecalho;
        let largura_dados: f32 = larguras.iter().sum();
        let altura_total = alt_cab + n_lin as f32 * ALTURA_LINHA;
        let id = self.id;
        let id_corpo = self.id_corpo();
        let selecao = self.selecao;
        let rolar = std::mem::take(&mut self.rolar_para_selecao);
        let rolar_extremo = std::mem::take(&mut self.rolar_para_extremo);
        // o intervalo de várias células (o de uma só é a própria célula ativa, com o contorno de sempre)
        let intervalo = selecao.map(|a| Intervalo::entre(a, self.extremo.unwrap_or(a))).filter(Intervalo::varias);
        let alvo_da_rolagem = if rolar_extremo { self.extremo } else { selecao.filter(|_| rolar) };
        let mut arrasto_ate: Option<(usize, usize)> = None;
        let congelado = self.congelado;
        let mesclas = self.mesclas.clone();
        let cabecalho_liso = self.cabecalho_liso;
        let mut congelado_efetivo = Congelado::default();
        let mut nova_largura: Option<(usize, f32)> = None;
        let mut clique: Option<(usize, usize)> = None;
        let mut clique_numero: Option<(usize, Toques)> = None;
        let mut clique_cabecalho: Option<(usize, Toques)> = None;
        let mut menu_cabecalho: Option<usize> = None;
        let menu_pelo_teclado = std::mem::take(&mut self.menu_pelo_teclado);
        let mut pedido_menu: Option<menu_celula::PedidoMenu> = None;
        let selecionadas: Vec<bool> = (0..n_col).map(|c| desenho.th_selecionada(c)).collect();

        // a calha de 8px da barra de rolagem: quando a grade rola, o conteúdo não passa por baixo da barra (a 960 a
        // última coluna ficava coberta)
        let calha_antes = ui.spacing().scroll.floating_allocated_width;
        ui.spacing_mut().scroll.floating_allocated_width = CALHA_ROLAGEM;
        let saida = ScrollArea::both()
            .id_salt(id.with("rolagem"))
            .max_height(altura_max)
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                let largura_conteudo = largura_dados.max(ui.available_width());
                let (conteudo, _) = ui.allocate_exact_size(vec2(largura_conteudo, altura_total), Sense::hover());
                let origem = conteudo.min;
                let visivel = viewport.translate(origem.to_vec2());

                // uma posição por coluna e mais uma no fim (`xs` tem sempre `n_col + 1` itens)
                let mut xs = Vec::with_capacity(n_col + 1);
                let mut x = origem.x;
                for l in &larguras {
                    xs.push(x);
                    x += l;
                }
                xs.push(x);

                // o painel congelado, com a trava de 60% por direção (só vale numa direção que rola)
                let especiais = navegaveis.start.min(n_col);
                let (linhas_pedidas, colunas_pedidas) = match congelado {
                    Some(c) => (c.linhas.min(n_lin), (especiais + c.colunas).min(n_col)),
                    None => (0, 0),
                };
                let rola_y = altura_total > viewport.height() + 0.5;
                let rola_x = largura_conteudo > viewport.width() + 0.5;
                let nf = if rola_y
                    && alt_cab + linhas_pedidas as f32 * ALTURA_LINHA > FRACAO_MAXIMA_CONGELADA * viewport.height()
                {
                    0
                } else {
                    linhas_pedidas
                };
                let k = if rola_x && xs[colunas_pedidas] - origem.x > FRACAO_MAXIMA_CONGELADA * viewport.width() {
                    0
                } else {
                    colunas_pedidas
                };
                congelado_efetivo = Congelado { linhas: nf, colunas: k };
                // onde acaba a parte presa: à direita das colunas presas e embaixo das linhas congeladas
                let w_presa = xs[k] - origem.x;
                let x_presa = visivel.left() + w_presa;
                let y_presa = visivel.top() + alt_cab + nf as f32 * ALTURA_LINHA;
                // onde cada coluna e cada linha está na tela: as presas e as congeladas ficam no lugar
                let esq = |c: usize| if c < k { visivel.left() + xs[c] - origem.x } else { xs[c] };
                let dir = |c: usize| if c < k { visivel.left() + xs[c + 1] - origem.x } else { xs[c + 1] };
                let y_de = |l: usize| {
                    if l < nf {
                        visivel.top() + alt_cab + l as f32 * ALTURA_LINHA
                    } else {
                        origem.y + alt_cab + l as f32 * ALTURA_LINHA
                    }
                };
                let col_visiveis: Vec<usize> =
                    (0..n_col).filter(|&c| c < k || (xs[c + 1] >= x_presa && xs[c] <= visivel.right())).collect();
                let n_presas = col_visiveis.partition_point(|&c| c < k);
                let (cols_presas, cols_rolam) = col_visiveis.split_at(n_presas);

                // linhas que rolam e visíveis (as congeladas são sempre 0..nf)
                let primeira = (((viewport.min.y - alt_cab) / ALTURA_LINHA).floor().max(0.0) as usize + nf).min(n_lin);
                let ultima = ((((viewport.max.y - alt_cab) / ALTURA_LINHA).ceil().max(0.0)) as usize + 1).min(n_lin);
                let linhas_rolam = primeira..ultima;

                // as mesclas à vista, dentro da grade (uma lista pequena: só as que cruzam a tela)
                let mut mesclas_visiveis: Vec<Mescla> = Vec::new();
                if let Some(ix) = mesclas.as_ref().filter(|_| n_col > 0 && n_lin > 0) {
                    let dentro = |m: &Mescla| -> Option<Mescla> {
                        (m.linha0 < n_lin && m.coluna0 < n_col).then(|| Mescla {
                            linha1: m.linha1.min(n_lin - 1),
                            coluna1: m.coluna1.min(n_col - 1),
                            ..*m
                        })
                    };
                    if nf > 0 {
                        mesclas_visiveis.extend(ix.que_cruzam(0..nf).filter_map(dentro));
                    }
                    mesclas_visiveis
                        .extend(ix.que_cruzam(linhas_rolam.clone()).filter(|m| m.linha0 >= nf).filter_map(dentro));
                }
                // a mescla inteira: o canto de cima à esquerda até o de baixo à direita; a que cruza a emenda do
                // congelado junta a parte que rola à presa (os dois lados se pintam, cada um no seu recorte)
                let uniao = |m: &Mescla| -> Rect {
                    let x0 = esq(m.coluna0);
                    let x1 = if m.coluna0 < k && m.coluna1 >= k { dir(m.coluna1).max(x_presa) } else { dir(m.coluna1) };
                    let y0 = y_de(m.linha0);
                    let y1 = y_de(m.linha1) + ALTURA_LINHA;
                    let y1 = if m.linha0 < nf && m.linha1 >= nf { y1.max(y_presa) } else { y1 };
                    Rect::from_min_max(pos2(x0, y0), pos2(x1.max(x0), y1.max(y0)))
                };
                // a mescla que cobre a célula passa a ser a célula de cima à esquerda dela
                let canto_da_mescla = |l: usize, c: usize| -> (usize, usize) {
                    mesclas.as_ref().and_then(|m| m.mescla_em(l, c)).map_or((l, c), |m| (m.linha0, m.coluna0))
                };

                // o corpo (abaixo do cabeçalho grudado) é uma interação só
                let corpo_tela = Rect::from_min_max(pos2(visivel.left(), visivel.top() + alt_cab), visivel.max);
                let corpo = ui.interact(corpo_tela, id_corpo, Sense::click_and_drag());
                let celula_em = |pos: egui::Pos2| -> Option<(usize, usize)> {
                    let l = if nf > 0 && pos.y < y_presa {
                        ((pos.y - visivel.top() - alt_cab) / ALTURA_LINHA).floor()
                    } else {
                        ((pos.y - origem.y - alt_cab) / ALTURA_LINHA).floor()
                    };
                    if l < 0.0 || l as usize >= n_lin {
                        return None;
                    }
                    let candidatas = if k > 0 && pos.x < x_presa { 0..k } else { k..n_col };
                    let c = candidatas.into_iter().find(|&c| pos.x >= esq(c) && pos.x < dir(c))?;
                    Some((l as usize, c))
                };
                // a célula mais perto do ponteiro, presa à grade (o arrasto que passa da borda continua na última)
                let celula_perto = |pos: egui::Pos2| -> Option<(usize, usize)> {
                    if n_lin == 0 || navegaveis.is_empty() {
                        return None;
                    }
                    let y = pos.y.clamp(corpo_tela.top(), corpo_tela.bottom() - 1.0);
                    let x = pos.x.clamp(visivel.left(), visivel.right() - 1.0);
                    let (l, c) = celula_em(pos2(x, y)).unwrap_or_else(|| {
                        let l = if y >= origem.y + alt_cab + n_lin as f32 * ALTURA_LINHA { n_lin - 1 } else { 0 };
                        let c = if x >= xs[n_col] { navegaveis.end - 1 } else { navegaveis.start };
                        (l, c)
                    });
                    Some((l, c.clamp(navegaveis.start, navegaveis.end - 1)))
                };
                let sob_ponteiro = corpo.hover_pos().and_then(celula_em);
                resposta.linha_sob_ponteiro = sob_ponteiro.map(|(l, _)| l);
                let shift = ui.input(|i| i.modifiers.shift);
                // o arrasto de uma célula a outra marca o intervalo (a que começou fica a ativa); perto da borda, a
                // grade rola sozinha
                let id_arrasto = id.with("arrasto-do-intervalo");
                if corpo.drag_started() {
                    corpo.request_focus();
                    let inicio = ui.input(|i| i.pointer.press_origin()).and_then(celula_em);
                    let mut valido = false;
                    if let Some((l, c)) = inicio.filter(|&(_, c)| coluna_numero != Some(c)) {
                        let (l, c) = canto_da_mescla(l, c);
                        if navegaveis.contains(&c) {
                            // com o Shift, o arrasto estende a partir da célula ativa
                            valido = true;
                            if !(shift && selecao.is_some()) {
                                clique = Some((l, c));
                            }
                        }
                    }
                    ui.data_mut(|d| d.insert_temp(id_arrasto, valido));
                }
                let arrasto_valido = ui.data(|d| d.get_temp::<bool>(id_arrasto)).unwrap_or(false);
                if corpo.dragged()
                    && arrasto_valido
                    && let Some(pos) = corpo.interact_pointer_pos()
                {
                    arrasto_ate = celula_perto(pos);
                    let borda = ALTURA_LINHA;
                    let passo = |fora: f32| (fora / borda).clamp(0.0, 3.0) * 8.0;
                    let dy = passo(pos.y - (corpo_tela.bottom() - borda)) - passo(corpo_tela.top() + borda - pos.y);
                    let dx = passo(pos.x - (visivel.right() - borda)) - passo(visivel.left() + borda - pos.x);
                    if dx != 0.0 || dy != 0.0 {
                        ui.scroll_with_delta(vec2(-dx, -dy));
                        ui.ctx().request_repaint();
                    }
                }
                if corpo.clicked() || corpo.double_clicked() {
                    corpo.request_focus();
                    // o Shift+clique estende o intervalo da célula ativa até a clicada
                    let estende = shift && selecao.is_some() && !corpo.double_clicked();
                    if estende {
                        arrasto_ate =
                            corpo.interact_pointer_pos().and_then(celula_em).map(|(l, c)| canto_da_mescla(l, c));
                    } else if let Some((l, c)) = corpo.interact_pointer_pos().and_then(celula_em) {
                        if coluna_numero == Some(c) {
                            // o número da linha não é célula: o clique pede a seleção da linha
                            if !corpo.double_clicked() {
                                let m = ui.input(|i| i.modifiers);
                                clique_numero = Some((l, Toques { ctrl: m.ctrl || m.command, shift: m.shift }));
                            }
                        } else {
                            let (l, c) = canto_da_mescla(l, c);
                            if navegaveis.contains(&c) {
                                clique = Some((l, c));
                                if corpo.double_clicked() {
                                    resposta.editar = Some((l, c));
                                }
                            }
                        }
                    }
                }

                // o clique direito numa célula pede o menu: dentro do intervalo, ele age no intervalo; fora, a célula
                // vira a ativa
                if corpo.secondary_clicked()
                    && let Some(pos) = corpo.interact_pointer_pos()
                    && let Some((l, c)) = celula_em(pos).filter(|&(_, c)| coluna_numero != Some(c))
                {
                    let (l, c) = canto_da_mescla(l, c);
                    if navegaveis.contains(&c) {
                        corpo.request_focus();
                        if !intervalo.is_some_and(|iv| iv.contem(l, c)) {
                            clique = Some((l, c));
                        }
                        pedido_menu = Some(menu_celula::PedidoMenu { pos, teclado: false });
                    }
                }
                // o Shift+F10: o menu embaixo da célula ativa, um pouco para dentro (preso à parte visível)
                if menu_pelo_teclado && let Some((l, c)) = selecao {
                    let canto = pos2(esq(c) + menu_celula::RECUO_TECLADO, y_de(l) + ALTURA_LINHA);
                    let pos = pos2(
                        canto.x.clamp(corpo_tela.left(), corpo_tela.right()),
                        canto.y.clamp(corpo_tela.top(), corpo_tela.bottom()),
                    );
                    pedido_menu = Some(menu_celula::PedidoMenu { pos, teclado: true });
                }

                let fio = Stroke::new(1.0, p.estudio.b);
                // o fio da direita (`border-right:1px solid --b`), menos na coluna da caixa de marcar
                let com_fio = |c: usize| coluna_marcar != Some(c);
                // uma parte da grade: as `linhas` e as `colunas` dela, no `clip`; `presas` e `congeladas` dizem de que lado
                // da emenda ela está (para saber que mesclas pintam aqui)
                let pintar_parte = |ui: &mut Ui,
                                    desenho: &mut dyn DesenhoGrade,
                                    linhas: Range<usize>,
                                    colunas: &[usize],
                                    clip: Rect,
                                    presas: bool,
                                    congeladas: bool| {
                    if clip.width() <= 0.0 || clip.height() <= 0.0 {
                        return;
                    }
                    let pintor = ui.painter_at(clip);
                    let mut buracos: Vec<(f32, f32)> = Vec::new();
                    let mut na_linha: Vec<Mescla> = Vec::new();
                    for l in linhas {
                        let y = y_de(l);
                        let faixa =
                            Rect::from_min_max(pos2(visivel.left(), y), pos2(visivel.right(), y + ALTURA_LINHA));
                        na_linha.clear();
                        na_linha
                            .extend(mesclas_visiveis.iter().filter(|m| (m.linha0..=m.linha1).contains(&l)).copied());
                        let fundo = desenho.fundo_linha(l);
                        let sob = sob_ponteiro.is_some_and(|(lh, _)| lh == l);
                        match fundo {
                            Some(cor) => {
                                pintor.rect_filled(faixa, 0.0, cor);
                            }
                            None if sob => {
                                pintor.rect_filled(faixa, 0.0, p.estudio.selh);
                            }
                            None => {
                                // `td.lin`: o fundo `--p` (o da linha sob o ponteiro ou marcada fica no lugar dele)
                                if let Some(cn) = coluna_numero.filter(|&cn| cn < n_col && colunas.contains(&cn)) {
                                    let r = Rect::from_min_max(pos2(esq(cn), y), pos2(dir(cn), y + ALTURA_LINHA));
                                    pintor.rect_filled(r, 0.0, p.estudio.faixa);
                                }
                            }
                        }
                        // `td.selcol{background:--tint}`: a coluna selecionada, por baixo dos fios e do texto
                        for &c in colunas.iter().filter(|&&c| selecionadas[c]) {
                            let direita = if com_fio(c) { dir(c) - 1.0 } else { dir(c) };
                            let celula = Rect::from_min_max(pos2(esq(c), y), pos2(direita, y + ALTURA_LINHA - 1.0));
                            pintor.rect_filled(celula, 0.0, p.estudio.tint);
                        }
                        // o intervalo de várias células: o `--tint` da coluna selecionada, embaixo dos fios e do texto
                        if let Some(iv) = intervalo.filter(|iv| iv.linhas().contains(&l)) {
                            for &c in colunas.iter().filter(|&&c| iv.colunas().contains(&c)) {
                                let direita = if com_fio(c) { dir(c) - 1.0 } else { dir(c) };
                                let celula = Rect::from_min_max(pos2(esq(c), y), pos2(direita, y + ALTURA_LINHA - 1.0));
                                pintor.rect_filled(celula, 0.0, p.estudio.tint);
                            }
                        }
                        // `.grade td{border-bottom:1px solid --b}` em todas as linhas, e o da direita sem cruzar com ele;
                        // dentro de uma mescla nenhum dos dois passa (só o da borda dela)
                        let y_fio = faixa.bottom() - 0.5;
                        buracos.clear();
                        buracos.extend(na_linha.iter().filter(|m| l < m.linha1).map(|m| {
                            let u = uniao(m);
                            (u.left(), u.right())
                        }));
                        if buracos.is_empty() {
                            pintor.hline(faixa.x_range(), y_fio, fio);
                        } else {
                            for (a, b) in trechos_livres(faixa.left(), faixa.right(), &mut buracos) {
                                pintor.hline(a..=b, y_fio, fio);
                            }
                        }
                        for &c in colunas
                            .iter()
                            .filter(|&&c| com_fio(c) && !na_linha.iter().any(|m| (m.coluna0..m.coluna1).contains(&c)))
                        {
                            pintor.vline(dir(c) - 0.5, y..=y + ALTURA_LINHA - 1.0, fio);
                        }
                        for &c in colunas {
                            if na_linha.iter().any(|m| (m.coluna0..=m.coluna1).contains(&c)) {
                                continue;
                            }
                            let (esq_p, dir_p) = padding_grade(!com_fio(c));
                            let (x0, x1) = (esq(c), dir(c));
                            let celula = Rect::from_min_max(pos2(x0, y), pos2(x1, y + ALTURA_LINHA - 1.0));
                            let dados_celula = CelulaVisivel {
                                linha: l,
                                coluna: c,
                                celula,
                                conteudo: Rect::from_min_max(
                                    pos2(x0 + esq_p, y),
                                    pos2((x1 - dir_p).max(x0 + esq_p), y + ALTURA_LINHA - 1.0),
                                ),
                                recorte: celula.intersect(clip),
                                ativa: selecao == Some((l, c)),
                            };
                            desenho.celula(ui, &dados_celula);
                        }
                        // o contorno da ativa cobre os fios dela (`outline-offset:-2px` sobre a caixa com as bordas)
                        if let Some((la, ca)) = selecao
                            && la == l
                            && colunas.contains(&ca)
                            && !na_linha.iter().any(|m| m.contem(la, ca))
                        {
                            let celula = Rect::from_min_max(pos2(esq(ca), y), pos2(dir(ca), y + ALTURA_LINHA));
                            pintar_celula_ativa(&pintor, p, celula);
                        }
                    }
                    // o contorno de 1px `--acc` do intervalo, por cima dos fios
                    if let Some(iv) = intervalo {
                        let r = Rect::from_min_max(
                            pos2(esq(iv.coluna0), y_de(iv.linha0)),
                            pos2(dir(iv.coluna1), y_de(iv.linha1) + ALTURA_LINHA),
                        );
                        pintor.rect_stroke(r, 0.0, Stroke::new(1.0, p.estudio.acc), StrokeKind::Inside);
                    }
                    // as mescladas desta parte: o conteúdo da célula de cima à esquerda no retângulo todo
                    for m in &mesclas_visiveis {
                        let desta_parte = (if presas { m.coluna0 < k } else { m.coluna1 >= k })
                            && (if congeladas { m.linha0 < nf } else { m.linha1 >= nf });
                        if !desta_parte {
                            continue;
                        }
                        let u = uniao(m);
                        let recorte = u.intersect(clip);
                        if recorte.width() <= 0.0 || recorte.height() <= 0.0 {
                            continue;
                        }
                        let ativa = selecao.is_some_and(|(sl, sc)| m.contem(sl, sc));
                        let celula = Rect::from_min_max(u.min, pos2(u.right(), (u.bottom() - 1.0).max(u.top())));
                        let conteudo = Rect::from_min_max(
                            pos2(u.left() + PADDING_X, u.top()),
                            pos2((u.right() - PADDING_X - 1.0).max(u.left() + PADDING_X), celula.bottom()),
                        );
                        desenho.celula_mesclada(
                            ui,
                            &CelulaMesclada { linha: m.linha0, coluna: m.coluna0, celula, conteudo, recorte, ativa },
                        );
                        if ativa {
                            pintar_celula_ativa(&pintor, p, u);
                        }
                    }
                };
                let corpo_esq = if k > 0 { x_presa } else { visivel.left() };
                let corpo_topo = if nf > 0 { y_presa } else { visivel.top() };
                let sob_cabecalho = visivel.top() + alt_cab;
                pintar_parte(
                    ui,
                    desenho,
                    linhas_rolam.clone(),
                    cols_rolam,
                    Rect::from_min_max(pos2(corpo_esq, corpo_topo), visivel.max),
                    false,
                    false,
                );
                if k > 0 {
                    let parte = Rect::from_min_max(pos2(visivel.left(), corpo_topo), pos2(x_presa, visivel.bottom()));
                    pintar_parte(ui, desenho, linhas_rolam.clone(), cols_presas, parte, true, false);
                }
                if nf > 0 {
                    let parte = Rect::from_min_max(pos2(corpo_esq, sob_cabecalho), pos2(visivel.right(), y_presa));
                    pintar_parte(ui, desenho, 0..nf, cols_rolam, parte, false, true);
                    if k > 0 {
                        let parte = Rect::from_min_max(pos2(visivel.left(), sob_cabecalho), pos2(x_presa, y_presa));
                        pintar_parte(ui, desenho, 0..nf, cols_presas, parte, true, true);
                    }
                }

                // dica da célula sob o ponteiro (o `title`); na mescla, a da célula de cima à esquerda
                if let Some((l, c)) = sob_ponteiro {
                    let (l, c) = canto_da_mescla(l, c);
                    if let Some(d) = desenho.dica(l, c) {
                        let curta: String = d.chars().take(320).collect();
                        corpo.clone().on_hover_text_at_pointer(curta);
                    }
                }

                // cabeçalho grudado no topo do que aparece: as colunas que rolam por baixo das presas
                let pintor = ui.painter_at(visivel);
                let cab = Rect::from_min_size(visivel.min, vec2(visivel.width(), alt_cab));
                if cabecalho_liso {
                    pintura::preencher(&pintor, p, cab, 0.0, Preenchimento::Cabecalho);
                    pintor.hline(cab.x_range(), cab.bottom() - 0.5, Stroke::new(1.0, p.estudio.btb));
                } else {
                    pintar_fundo_cabecalho(&pintor, p, cab);
                }
                let cab_rolam = Rect::from_min_max(pos2(corpo_esq, cab.top()), pos2(visivel.right(), cab.bottom()));
                let cab_presas = Rect::from_min_max(pos2(visivel.left(), cab.top()), pos2(x_presa, cab.bottom()));
                for (colunas, clip) in [(cols_rolam, cab_rolam), (cols_presas, cab_presas)] {
                    let pintor = ui.painter_at(clip);
                    for &c in colunas {
                        let (esq_p, dir_p) = padding_grade(!com_fio(c));
                        let (x0, x1) = (esq(c), dir(c));
                        let th = Rect::from_min_max(pos2(x0, cab.top()), pos2(x1, cab.bottom()));
                        if selecionadas[c] {
                            // `th.selcol{background:--sel}`: no lugar do fundo do `th` (o fio de baixo e o da direita ficam)
                            let direita = if com_fio(c) { th.right() - 1.0 } else { th.right() };
                            let miolo = Rect::from_min_max(th.min, pos2(direita, th.bottom() - 1.0));
                            pintor.rect_filled(miolo, 0.0, p.estudio.doc);
                            pintor.rect_filled(miolo, 0.0, p.estudio.sel);
                        }
                        if com_fio(c) {
                            pintor.vline(x1 - 0.5, cab.top()..=cab.bottom() - 1.0, fio);
                        }
                        let h = CabecalhoVisivel {
                            coluna: c,
                            th,
                            conteudo: Rect::from_min_max(
                                pos2(x0 + esq_p, cab.top()),
                                pos2((x1 - dir_p).max(x0 + esq_p), cab.bottom() - 1.0),
                            ),
                            recorte: th.intersect(clip),
                        };
                        // o clique no `th` (por baixo do que o gancho puser: a seta da ordem, a alça de redimensionar)
                        if navegaveis.contains(&c) {
                            let r_th = ui.interact(h.recorte, id.with(("th", c)), Sense::CLICK);
                            if r_th.clicked() {
                                let m = ui.input(|i| i.modifiers);
                                clique_cabecalho = Some((c, Toques { ctrl: m.ctrl || m.command, shift: m.shift }));
                            }
                            if r_th.secondary_clicked() {
                                menu_cabecalho = Some(c);
                            }
                        }
                        desenho.cabecalho(ui, &h);
                        if fixas.get(c).copied().unwrap_or(false) {
                            continue;
                        }
                        // alça de redimensionar na borda direita da coluna
                        let alca = Rect::from_center_size(pos2(x1, cab.center().y), vec2(8.0, alt_cab)).intersect(clip);
                        let r_alca = ui.interact(alca, id.with(("alca", c)), Sense::drag());
                        if r_alca.hovered() || r_alca.dragged() {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                            pintor.vline(x1 - 0.5, cab.top()..=cab.bottom() - 1.0, Stroke::new(1.0, p.estudio.acc));
                        }
                        if r_alca.dragged()
                            && let Some(ponteiro) = ui.ctx().pointer_interact_pos()
                        {
                            nova_largura = Some((c, ponteiro.x - x0));
                        }
                    }
                }

                // o fio da emenda do congelado (`--b2`, 2px, por dentro da última linha e da última coluna congeladas); a
                // mescla que cruza a emenda não é cortada por ele
                let emenda = p.estudio.b2;
                if nf > 0 {
                    let mut buracos: Vec<(f32, f32)> = mesclas_visiveis
                        .iter()
                        .filter(|m| m.linha0 < nf && m.linha1 >= nf)
                        .map(|m| {
                            let u = uniao(m);
                            (u.left(), u.right())
                        })
                        .collect();
                    for (a, b) in trechos_livres(visivel.left(), visivel.right(), &mut buracos) {
                        let r = Rect::from_min_max(pos2(a, y_presa - LARGURA_EMENDA), pos2(b, y_presa));
                        pintor.rect_filled(r, 0.0, emenda);
                    }
                }
                if k > especiais {
                    let mut buracos: Vec<(f32, f32)> = mesclas_visiveis
                        .iter()
                        .filter(|m| m.coluna0 < k && m.coluna1 >= k)
                        .map(|m| {
                            let u = uniao(m);
                            (u.top(), u.bottom())
                        })
                        .collect();
                    for (a, b) in trechos_livres(cab.top(), visivel.bottom(), &mut buracos) {
                        let r = Rect::from_min_max(pos2(x_presa - LARGURA_EMENDA, a), pos2(x_presa, b));
                        pintor.rect_filled(r, 0.0, emenda);
                    }
                }

                // rolar até a célula ativa, deixando de fora o cabeçalho e o que está congelado
                if let Some((l, c)) = alvo_da_rolagem
                    && c < n_col
                {
                    let (x_ini, x_fim) =
                        if c < k { (visivel.left(), visivel.right()) } else { (xs[c] - w_presa, xs[c + 1]) };
                    let (y_ini, y_fim) = if l < nf {
                        (visivel.top(), visivel.bottom())
                    } else {
                        let y = origem.y + alt_cab + l as f32 * ALTURA_LINHA;
                        (y - alt_cab - nf as f32 * ALTURA_LINHA, y + ALTURA_LINHA)
                    };
                    ui.scroll_to_rect(Rect::from_min_max(pos2(x_ini, y_ini), pos2(x_fim, y_fim)), None);
                }
                corpo
            });
        ui.spacing_mut().scroll.floating_allocated_width = calha_antes;

        self.congelado_efetivo = congelado_efetivo;
        if let Some((c, w)) = nova_largura {
            self.definir_largura(c, w);
        }
        if let Some(nova) = clique {
            if Some(nova) != self.selecao {
                resposta.selecao_mudou = true;
            }
            if self.extremo.take().is_some() {
                resposta.intervalo_mudou = true;
            }
            self.selecao = Some(nova);
            resposta.clicou = Some(nova);
        }
        if let Some(ate) = arrasto_ate
            && self.estender(ate)
        {
            resposta.intervalo_mudou = true;
            // o arrasto já está com a ponta à vista: não rola até ela
            self.rolar_para_extremo = false;
        }
        resposta.clicou_numero_da_linha = clique_numero;
        resposta.clicou_cabecalho = clique_cabecalho;
        resposta.menu_cabecalho = menu_cabecalho;
        if self.selecao.is_none() {
            resposta.copiar = false;
            resposta.copiar_com_cabecalho = false;
        }
        if pedido_menu.is_some() && self.selecao.is_some() {
            self.pedido_menu = pedido_menu;
            resposta.menu_celula = true;
        }
        let corpo = saida.inner;
        corpo.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Grade"));
        resposta
    }
}

/// Os trechos de `inicio..fim` que os `buracos` (intervalos em qualquer ordem) não cobrem: o fio que passa em volta de uma
/// mescla e o fio da emenda do congelado que não corta a que a cruza.
fn trechos_livres(inicio: f32, fim: f32, buracos: &mut [(f32, f32)]) -> Vec<(f32, f32)> {
    buracos.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut trechos = Vec::new();
    let mut x = inicio;
    for &(a, b) in buracos.iter() {
        if a > x {
            trechos.push((x, a.min(fim)));
        }
        x = x.max(b);
        if x >= fim {
            break;
        }
    }
    if x < fim {
        trechos.push((x, fim));
    }
    trechos
}

// ------------------------------------------------------------------ grade só leitura

/// O que aconteceu na grade neste quadro.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RespostaGrade {
    /// Texto copiado com Ctrl+C (o toast "Copiado." já foi mostrado).
    pub copiado: Option<String>,
    /// A célula escolhida mudou.
    pub selecao_mudou: bool,
    /// O menu da célula foi pedido ([`RespostaNucleo::menu_celula`]): a tela chama o [`menu_da_celula`] com o
    /// [`GradeSomenteLeitura::nucleo_mut`].
    pub menu_celula: bool,
}

/// Os ganchos da grade só leitura: as células de `comum::Celula` e o `th` com o nome.
struct DesenhoSomenteLeitura<'a> {
    colunas: &'a [ColunaGrade],
    linhas: &'a [Vec<Celula>],
    numericas: &'a [bool],
}

impl DesenhoGrade for DesenhoSomenteLeitura<'_> {
    fn celula(&mut self, ui: &mut Ui, c: &CelulaVisivel) {
        if let Some(v) = self.linhas.get(c.linha).and_then(|l| l.get(c.coluna)) {
            let recorte = ui.painter().with_clip_rect(c.recorte);
            let numerica = self.numericas.get(c.coluna).copied().unwrap_or(false);
            pintar_celula(&recorte, paleta(ui), c.conteudo, v, numerica);
        }
    }

    fn cabecalho(&mut self, ui: &mut Ui, h: &CabecalhoVisivel) {
        let Some(col) = self.colunas.get(h.coluna) else { return };
        let th = Th {
            tipo: col.tipo.as_deref(),
            numerica: self.numericas.get(h.coluna).copied().unwrap_or(false),
            pastilha: col.pastilha,
            ..Th::new(&col.nome)
        };
        let pastilha = pintar_cabecalho_coluna(&ui.painter().with_clip_rect(h.recorte), paleta(ui), h.conteudo, &th);
        // o que os valores são, na dica da pastilha (como a biblioteca de ícones diz)
        if let (Some(r), Some(tipo)) = (pastilha, col.pastilha) {
            let dica = match tipo {
                TipoColuna::Inteiro => "Número inteiro",
                TipoColuna::Real => "Número decimal",
                TipoColuna::Blob => "Binário",
                TipoColuna::Lista => "Lista",
                _ => "Texto",
            };
            let resp = ui.interact(r.intersect(h.recorte), ui.id().with(("grade::pastilha", h.coluna)), Sense::hover());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, dica));
            resp.on_hover_text(dica);
        }
    }

    fn dica(&mut self, linha: usize, coluna: usize) -> Option<String> {
        self.linhas.get(linha).and_then(|l| l.get(coluna)).and_then(dica_celula)
    }
}

/// Grade só leitura (a da Consulta SQL), montada sobre o [`NucleoGrade`]: cabeçalho grudado, rolagem virtual
/// (só as linhas e colunas visíveis são desenhadas), colunas de largura ajustável (arraste a borda do
/// cabeçalho), célula ativa (clique; setas, Home/End, Ctrl+Home/Ctrl+End, PageUp/PageDown movem) e cópia
/// (Ctrl+C copia a célula ativa, como o [`texto_celula`]). Guarde a grade junto com o resultado: ela é o estado
/// (larguras, seleção).
pub struct GradeSomenteLeitura {
    colunas: Vec<ColunaGrade>,
    linhas: Vec<Vec<Celula>>,
    numericas: Vec<bool>,
    nucleo: NucleoGrade,
    larguras_prontas: bool,
}

impl GradeSomenteLeitura {
    /// Monta a grade (calcula uma vez quais colunas são numéricas).
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        colunas: Vec<ColunaGrade>,
        linhas: Vec<Vec<Celula>>,
    ) -> Self {
        let numericas = (0..colunas.len()).map(|c| coluna_numerica(linhas.iter().filter_map(|l| l.get(c)))).collect();
        let com_tipo = colunas.iter().any(|c| c.tipo.is_some());
        let nucleo = NucleoGrade::new(id_salt, linhas.len(), vec![LARGURA_MIN_COLUNA; colunas.len()])
            .com_altura_cabecalho(altura_cabecalho(com_tipo, false));
        GradeSomenteLeitura { colunas, linhas, numericas, nucleo, larguras_prontas: false }
    }

    /// Usa as larguras que vêm de fora (uma por coluna, as do arquivo: [`larguras_do_arquivo`]) no lugar das iniciais
    /// pela medida do texto: [`larguras_iniciais`] nunca é chamada ([`NucleoGrade::com_larguras_externas`]).
    pub fn com_larguras_externas(mut self, larguras: Vec<f32>) -> Self {
        self.nucleo = self.nucleo.com_larguras_externas(larguras);
        self.larguras_prontas = true;
        self
    }

    pub fn colunas(&self) -> &[ColunaGrade] {
        &self.colunas
    }

    pub fn linhas(&self) -> &[Vec<Celula>] {
        &self.linhas
    }

    pub fn numericas(&self) -> &[bool] {
        &self.numericas
    }

    /// Id do corpo da grade (o que recebe o foco do teclado).
    pub fn id_corpo(&self) -> Id {
        self.nucleo.id_corpo()
    }

    /// A célula ativa (linha, coluna).
    pub fn selecao(&self) -> Option<(usize, usize)> {
        self.nucleo.selecao()
    }

    /// Escolhe a célula ativa (e rola até ela).
    pub fn selecionar(&mut self, celula: Option<(usize, usize)>) {
        self.nucleo.selecionar(celula);
    }

    /// O texto da célula ativa para copiar ([`texto_celula`]: NULL e BLOB → "", real como o JavaScript, texto
    /// cortado → o começo).
    pub fn texto_selecionado(&self) -> Option<String> {
        let (l, c) = self.nucleo.selecao()?;
        self.linhas.get(l)?.get(c).map(texto_celula)
    }

    /// O que o Ctrl+C leva: o intervalo de várias células como o Excel copia ([`tsv_do_intervalo`]), ou o texto da
    /// célula ativa ([`GradeSomenteLeitura::texto_selecionado`]).
    pub fn texto_copiado(&self) -> Option<String> {
        let iv = self.nucleo.intervalo()?;
        if !iv.varias() {
            return self.texto_selecionado();
        }
        Some(tsv_do_intervalo(iv, |l, c| self.linhas.get(l)?.get(c).map(texto_celula)))
    }

    /// O que o Ctrl+Shift+C leva: o intervalo (ou a célula ativa) com os nomes das colunas na primeira linha
    /// ([`tsv_do_intervalo_com_cabecalho`]).
    pub fn texto_copiado_com_cabecalho(&self) -> Option<String> {
        let iv = self.nucleo.intervalo()?;
        let nome = |c: usize| self.colunas.get(c).map(|col| col.nome.clone()).unwrap_or_default();
        Some(tsv_do_intervalo_com_cabecalho(iv, nome, |l, c| self.linhas.get(l)?.get(c).map(texto_celula)))
    }

    /// O intervalo (ou a célula ativa) no `formato` do "Copiar como" ([`texto_do_intervalo_como`]).
    pub fn texto_copiado_como(&self, formato: FormatoCopia, opcoes: &OpcoesCopia<'_>) -> Option<String> {
        let iv = self.nucleo.intervalo()?;
        let nome = |c: usize| self.colunas.get(c).map(|col| col.nome.clone()).unwrap_or_default();
        let valor = |l: usize, c: usize| {
            self.linhas.get(l).and_then(|linha| linha.get(c)).map_or(ValorCopia::Nulo, valor_de_copia)
        };
        Some(texto_do_intervalo_como(formato, iv, nome, valor, opcoes))
    }

    /// O núcleo, para o [`menu_da_celula`].
    pub fn nucleo_mut(&mut self) -> &mut NucleoGrade {
        &mut self.nucleo
    }

    /// As larguras das colunas (depois do primeiro quadro).
    pub fn larguras(&self) -> Option<&[f32]> {
        self.larguras_prontas.then(|| self.nucleo.larguras())
    }

    /// Troca a largura de uma coluna (mínimo 48px).
    pub fn definir_largura(&mut self, coluna: usize, largura: f32) {
        self.nucleo.definir_largura(coluna, largura);
    }

    /// O núcleo (rolagem, larguras, célula ativa).
    pub fn nucleo(&self) -> &NucleoGrade {
        &self.nucleo
    }

    /// Desenha a grade na largura disponível, com no máximo `altura_max` de altura (encolhe quando há poucas
    /// linhas). As colunas que sobram à direita ficam com o espaço que resta (a coluna "sobra" do kit).
    pub fn mostrar(&mut self, ui: &mut Ui, altura_max: f32) -> RespostaGrade {
        if !self.larguras_prontas {
            let iniciais = larguras_iniciais(ui.painter(), paleta(ui), &self.colunas, &self.linhas);
            self.nucleo.definir_larguras(iniciais);
            self.larguras_prontas = true;
        }
        let mut desenho =
            DesenhoSomenteLeitura { colunas: &self.colunas, linhas: &self.linhas, numericas: &self.numericas };
        let r = self.nucleo.mostrar(ui, altura_max, &mut desenho);
        let mut resposta = RespostaGrade { copiado: None, selecao_mudou: r.selecao_mudou, menu_celula: r.menu_celula };
        let copiado = if r.copiar {
            self.texto_copiado()
        } else if r.copiar_com_cabecalho {
            self.texto_copiado_com_cabecalho()
        } else {
            None
        };
        if let Some(t) = copiado {
            ui.ctx().copy_text(t.clone());
            let frase = self.nucleo.intervalo().map_or_else(|| "Copiado.".to_string(), frase_do_intervalo_copiado);
            toast_concluido(ui.ctx(), frase);
            resposta.copiado = Some(t);
        }
        resposta
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn coluna_numerica_como_na_consulta() {
        assert!(coluna_numerica(&[Celula::Inteiro(1), Celula::Nulo, Celula::Real(2.5)]));
        assert!(!coluna_numerica(&[Celula::Inteiro(1), Celula::Texto("a".into())]));
        assert!(!coluna_numerica(&[Celula::Nulo, Celula::Nulo]), "só NULL não é numérica");
        assert!(!coluna_numerica(&[]));
    }

    #[test]
    fn textos_da_celula() {
        assert_eq!(texto_exibicao(&Celula::Nulo), "NULL");
        assert_eq!(texto_exibicao(&Celula::Texto(String::new())), "vazio");
        assert_eq!(texto_exibicao(&Celula::Inteiro(-5)), "-5");
        // o real como o `String(número)` do JavaScript (o limite de clientes no banco de exemplo é 17051.0)
        assert_eq!(texto_exibicao(&Celula::Real(10.0)), "10");
        assert_eq!(texto_exibicao(&Celula::Real(17051.0)), "17051");
        assert_eq!(texto_exibicao(&Celula::Real(1e-5)), "0.00001");
        assert_eq!(texto_exibicao(&Celula::Real(1e16)), "10000000000000000");
        assert_eq!(texto_exibicao(&Celula::Real(1.5e300)), "1.5e+300");
        assert_eq!(texto_exibicao(&Celula::Texto("a\nb\t c".into())), "a b c");
        assert_eq!(texto_exibicao(&Celula::Blob { tamanho: 2355, inicio: vec![] }), "BLOB · 2,3 KB");
        assert_eq!(texto_exibicao(&Celula::TextoCortado { inicio: "abc".into(), tamanho: 4000 }), "abc");
    }

    #[test]
    fn texto_cru_da_celula_como_o_texto_celula() {
        assert_eq!(texto_celula(&Celula::Nulo), "");
        assert_eq!(texto_celula(&Celula::Blob { tamanho: 3, inicio: vec![1, 2, 3] }), "");
        assert_eq!(texto_celula(&Celula::Inteiro(7)), "7");
        assert_eq!(texto_celula(&Celula::Real(17051.0)), "17051");
        assert_eq!(texto_celula(&Celula::Real(2.5)), "2.5");
        assert_eq!(texto_celula(&Celula::Texto("a\nb".into())), "a\nb");
        assert_eq!(texto_celula(&Celula::TextoCortado { inicio: "abc".into(), tamanho: 900 }), "abc");
    }

    #[test]
    fn dicas_da_celula() {
        assert_eq!(dica_celula(&Celula::Nulo), None);
        assert_eq!(dica_celula(&Celula::Texto(String::new())).as_deref(), Some("Texto vazio ('')"));
        assert_eq!(
            dica_celula(&Celula::Blob { tamanho: 3, inicio: vec![0x00, 0xAB, 0x10] }).as_deref(),
            Some("Início em hexadecimal: 00ab10")
        );
        assert_eq!(dica_celula(&Celula::Blob { tamanho: 0, inicio: vec![] }).as_deref(), Some("Conteúdo binário"));
        assert_eq!(
            dica_celula(&Celula::TextoCortado { inicio: "abc".into(), tamanho: 5000 }).as_deref(),
            Some("abc…\n5.000 caracteres no total")
        );
        assert_eq!(dica_celula(&Celula::Real(2.5)).as_deref(), Some("2.5"));
        assert_eq!(dica_celula(&Celula::Real(1500.0)).as_deref(), Some("1500"));
    }

    #[test]
    fn medidas_da_grade_do_estudio() {
        // `.grade th{height:30px}` com ou sem o tipo (que vai ao lado do nome) e com o nome que ordena
        for (com_tipo, abre) in [(false, false), (false, true), (true, false), (true, true)] {
            assert_eq!(altura_cabecalho(com_tipo, abre), 30.0);
        }
        // `.grade td{height:28px;padding:0 10px}` e o fio da direita; a `td.selc` sem padding nem fio
        assert_eq!(ALTURA_LINHA, 28.0);
        assert_eq!(padding_grade(false), (10.0, 11.0));
        assert_eq!(padding_grade(true), (0.0, 0.0));
        assert_eq!(LARGURA_COLUNA_MARCAR, 32.0);
        // a tabela simples: `.lista-t th, .lista-t td{padding:0 12px}` em todas as colunas
        assert_eq!(padding_coluna(0, 3), (12.0, 12.0));
        assert_eq!(padding_coluna(2, 3), (12.0, 12.0));
    }

    #[test]
    fn pastilha_pelos_valores_da_consulta() {
        use TipoColuna::{Blob, Inteiro, Real, Texto};
        let blob = Celula::Blob { tamanho: 3, inicio: vec![1, 2, 3] };
        let t = |s: &str| Celula::Texto(s.into());
        assert_eq!(pastilha_pelos_valores(&[Celula::Inteiro(1), Celula::Nulo, Celula::Inteiro(7)]), Inteiro);
        assert_eq!(pastilha_pelos_valores(&[Celula::Inteiro(1), Celula::Real(2.5)]), Real);
        assert_eq!(pastilha_pelos_valores(&[Celula::Real(2.5)]), Real);
        assert_eq!(pastilha_pelos_valores(&[blob.clone(), Celula::Nulo]), Blob);
        assert_eq!(pastilha_pelos_valores(&[t("a"), Celula::Inteiro(1)]), Texto, "mistura vira texto");
        assert_eq!(pastilha_pelos_valores(&[Celula::Inteiro(1), blob]), Texto);
        assert_eq!(pastilha_pelos_valores(&[Celula::Nulo, Celula::Nulo]), Texto, "só NULL: o padrão");
        assert_eq!(pastilha_pelos_valores(&[]), Texto);
    }

    #[test]
    fn grade_calcula_numericas_e_texto_selecionado() {
        let mut g = GradeSomenteLeitura::new(
            "t",
            vec![ColunaGrade::new("id"), ColunaGrade::new("nome")],
            vec![vec![Celula::Inteiro(1), Celula::Texto("Ana".into())], vec![Celula::Inteiro(2), Celula::Nulo]],
        );
        assert_eq!(g.numericas(), &[true, false]);
        assert_eq!(g.texto_selecionado(), None);
        g.selecionar(Some((0, 1)));
        assert_eq!(g.texto_selecionado().as_deref(), Some("Ana"));
        g.selecionar(Some((1, 1)));
        assert_eq!(g.texto_selecionado().as_deref(), Some(""), "NULL copia vazio");
        g.selecionar(Some((9, 9)));
        assert_eq!(g.selecao(), None, "fora da grade não seleciona");
    }

    #[test]
    fn mover_como_na_aba_dados() {
        // 3 linhas; colunas 0 (marcar) e 1 (número da linha) fora da navegação; dados em 2..5
        let mut n = NucleoGrade::new("m", 3, vec![30.0, 40.0, 100.0, 100.0, 100.0])
            .com_navegacao(Navegacao::Dados)
            .com_navegaveis(2..5)
            .com_fixas(&[0, 1]);
        n.selecionar(Some((0, 1)));
        assert_eq!(n.selecao(), None, "a coluna do número da linha não é navegável");
        n.selecionar(Some((0, 4)));
        assert!(n.mover(0, 1), "a coluna passa para a linha seguinte");
        assert_eq!(n.selecao(), Some((1, 2)));
        assert!(n.mover(0, -1), "e volta para a anterior");
        assert_eq!(n.selecao(), Some((0, 4)));
        n.selecionar(Some((0, 2)));
        assert!(!n.mover(0, -1), "não dá a volta na grade");
        assert!(n.na_ponta(true));
        n.selecionar(Some((2, 4)));
        assert!(!n.mover(0, 1));
        assert!(n.na_ponta(false));
        assert!(n.mover(-10, 0), "PageUp para na primeira linha");
        assert_eq!(n.selecao(), Some((0, 4)));
        n.definir_linhas(0);
        assert_eq!(n.selecao(), None);
    }
}
