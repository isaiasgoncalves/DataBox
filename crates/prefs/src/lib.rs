//! Preferências do DataBox, num JSON por pessoa: `%APPDATA%\DataBox\preferencias.json` (tema, opções de
//! exportação, última pasta, recentes (bancos, pastas e arquivos de dados), histórico da Consulta SQL de cada banco,
//! a aparência do código, as larguras dos painéis e a pasta aberta).
//!
//! # API pública
//!
//! ```text
//! pub struct Preferencias                       // Send + Sync; use por Arc<Preferencias>
//! impl Preferencias {
//!     pub fn abrir(arquivo: Option<PathBuf>) -> Preferencias   // None → arquivo_padrao(); não toca no disco
//!     pub fn arquivo(&self) -> &Path            // o preferencias.json
//!     pub fn arquivo_trava(&self) -> PathBuf    // o preferencias.json.lock (arquivo do app: não exportar por cima)
//!     pub fn tema(&self) -> Tema
//!     pub fn exportacao(&self) -> OpcoesExportacao
//!     pub fn aparencia(&self) -> Aparencia      // o padrão enquanto a pessoa não escolher
//!     pub fn paineis(&self) -> LarguraPaineis   // as larguras dos painéis laterais; None = a padrão da casca
//!     pub fn pasta_aberta(&self) -> Option<String>   // a pasta que estava aberta; None = nenhuma
//!     pub fn ultimo_diretorio(&self) -> Option<String>
//!     pub fn atualizar(&self, mudanca: Mudanca) -> Resultado<()>   // ErroBanco 400 com a frase
//!     pub fn recentes(&self) -> Vec<Recente>    // até 15 (bancos, pastas, arquivos), o mais recente primeiro; consulta os arquivos agora
//!     pub fn adicionar_recente(&self, caminho: impl AsRef<Path>)   // = adicionar_recente_de(caminho, Banco)
//!     pub fn adicionar_recente_de(&self, caminho: impl AsRef<Path>, tipo: TipoRecente)   // leva as chaves desconhecidas
//!     pub fn remover_recente(&self, caminho: impl AsRef<Path>)
//!     pub fn limpar_recentes(&self)             // esvazia a lista (o Limpar a lista das Preferências)
//!     pub fn historico_sql(&self, caminho_banco: impl AsRef<Path>) -> Vec<ComandoSql>   // até 30, mais recente primeiro
//!     pub fn registrar_sql(&self, caminho_banco: impl AsRef<Path>, sql: &str)
//!     pub fn limpar_historico_sql(&self)        // esvazia o histórico de todos os bancos
//!     pub fn resumo(&self) -> PrefsInfo         // {tema, recentes, exportacao, aparencia}
//! }
//! pub enum Tema { Sistema, Claro, Escuro }      // como_str()/Display: "system" | "light" | "dark"; Default = Sistema
//!     Tema::TODOS; Tema::de_texto(&str) -> Resultado<Tema> (e FromStr):
//!     "Tema desconhecido: 'roxo'. Use system, light ou dark." (400)
//! pub struct OpcoesExportacao { csv_separador: char, csv_decimal: char, json_indentado: bool,
//!     cabecalho: Option<bool>, csv_proteger_formulas: Option<bool> }  // Default = ';' ',' true None None
//!     // None = a pessoa nunca escolheu (a janela Exportar usa true nos dois)
//! pub struct MudancaExportacao { cabecalho, csv_separador, csv_decimal, json_indentado, csv_proteger_formulas }
//!     // todos Option (None = não mexe); From<OpcoesExportacao>
//! pub enum CoresCodigo { DataBox, AltoContraste, SolarizedEscuro, Monokai }   // Default = DataBox
//!     como_str()/Display: "databox" | "alto_contraste" | "solarized_escuro" | "monokai"; CoresCodigo::TODAS;
//!     CoresCodigo::de_texto(&str) -> Resultado<CoresCodigo> (e FromStr): "Cores do código desconhecidas: 'x'. …" (400)
//! pub struct Aparencia { cores_codigo: CoresCodigo, fonte_codigo: Option<String>, tamanho_codigo: f32 }
//!     // Default = DataBox, None (Consolas), 12,5 px
//! pub struct MudancaAparencia { cores_codigo, fonte_codigo: Option<Option<String>>, tamanho_codigo }
//!     // todos Option (None = não mexe); From<Aparencia>
//! pub const TAMANHO_CODIGO_PADRAO = 12.5, TAMANHO_CODIGO_MINIMO = 8.0, TAMANHO_CODIGO_MAXIMO = 24.0
//! pub struct LarguraPaineis { explorador: Option<f32>, propriedades: Option<f32> }   // pontos inteiros; Default = None, None
//! pub struct MudancaLarguraPaineis { explorador, propriedades }   // todos Option (None = não mexe)
//! pub const LARGURA_PAINEL_MINIMA = 120.0, LARGURA_PAINEL_MAXIMA = 2000.0   // a sanidade do arquivo, não o limite da casca
//! pub struct Mudanca { tema: Option<Tema>, exportacao: Option<MudancaExportacao>,
//!     ultimo_diretorio: Option<Option<PathBuf>>,    // None = não mexe; Some(None) = apaga
//!     aparencia: Option<MudancaAparencia>, paineis: Option<MudancaLarguraPaineis>,
//!     pasta_aberta: Option<Option<PathBuf>> }       // None = não mexe; Some(None) = apaga a chave
//!     Mudanca::tema(t) / ::exportacao(m) / ::ultimo_diretorio(p) / ::aparencia(m) / ::paineis(m) / ::pasta_aberta(p)
//!     e Default
//! pub enum TipoRecente { Banco, Pasta, Arquivo }  // como_str()/Display: "banco" | "pasta" | "arquivo"; Default = Banco
//!     TipoRecente::TODOS; TipoRecente::de_texto(&str) -> Resultado<TipoRecente> (e FromStr):
//!     "Tipo de recente desconhecido: 'x'. Use banco, pasta ou arquivo." (400)
//! pub struct Recente { caminho, nome, pasta: String, tipo: TipoRecente, existe: bool, tamanho: Option<u64>,
//!     modificado_em: Option<String>, aberto_em: Option<String> }  // datas ISO locais "2026-10-07T14:03:05"
//!     // existe: pasta → "é uma pasta que respondeu a tempo"; banco e arquivo → "é arquivo comum que respondeu";
//!     // tamanho: só do arquivo (None na pasta)
//! pub struct ComandoSql { sql: String, quando: Option<String>, resultado: Option<ResultadoSql>, comandos: Option<u32> }
//! pub enum ResultadoSql { Linhas { quantidade: u64, mais: bool }, Alteradas(u64), Executado,
//!     Erro { numero: u32, mensagem: String }, Interrompida { numero: u32 } }
//!     // ResultadoSql::erro(numero, &str) corta a mensagem
//! pub struct PrefsInfo { tema: Tema, recentes: Vec<Recente>, exportacao: OpcoesExportacao, aparencia: Aparencia }
//! pub fn arquivo_padrao() -> PathBuf            // %APPDATA%\DataBox\preferencias.json (sem APPDATA, a pasta pessoal)
//! pub fn caminho_exibido(p) -> String           // absoluto e normalizado, com a caixa como veio
//! pub fn chave_caminho(p) -> String             // a chave de comparação de caminhos (sem caixa nem barra)
//! pub const MAX_RECENTES = 15, MAX_COMANDOS = 30, MAX_BANCOS_NO_HISTORICO = 50,
//!     MAX_CARACTERES_COMANDO = 100_000, TEMPO_MAXIMO_TRAVA = 3 s, TEMPO_MAXIMO_CONSULTA_ARQUIVOS = 2 s
//! pub const SEPARADORES_CSV = [';', ',', '\t'], SEPARADORES_DECIMAIS = [',', '.']
//! ```
//!
//! # Regras
//!
//! - Cada mudança relê o arquivo, aplica a mudança e grava de novo sob uma trava entre processos
//!   (`preferencias.json.lock` ao lado): duas instâncias não apagam os recentes nem o histórico uma da outra. Sem a
//!   trava em 3 s, segue sem ela. Duas instâncias gravando juntas se revezam: quem espera "bate" (muda a data do
//!   `.lock`) e quem acabou de gravar, se viu a batida, espera 50 ms antes de travar de novo (sem isso, uma rajada
//!   de mudanças de uma deixava a outra de fora até o fim ou até os 3 s; ver `trava.rs`). Sem ninguém esperando,
//!   nada atrasa.
//! - Se o arquivo existe mas não pode ser lido na hora de mudar (outro programa com ele aberto), a mudança fica
//!   só em memória em vez de gravar por cima dados velhos.
//! - Gravação atômica: temporário na mesma pasta (`preferencias.json.XXXXXXXX.tmp`) + troca; o temporário
//!   some se algo falhar. Disco que recusa → aviso no log e a mudança vale em memória até fechar; enquanto o
//!   arquivo não existir, as mudanças seguintes partem do que está em memória, não do padrão.
//! - Arquivo que não é JSON (ou não é um objeto) é renomeado para `preferencias.json.corrompido` e tudo
//!   recomeça do padrão; campos com valor estranho voltam ao padrão sem alarde. Aceita o BOM do Bloco de Notas.
//!   O leitor é tolerante (`json.rs`): `NaN`, `Infinity`, número fora da faixa e qualquer profundidade de
//!   aninhamento são aceitos; inteiro com mais de 4300 algarismos é recusado; escape de surrogate solto vira
//!   U+FFFD.
//! - A leitura fica em memória enquanto o arquivo não muda (data de modificação e tamanho); mudou por fora →
//!   relê na próxima consulta.
//! - Caminhos são comparados normalizados ([`chave_caminho`]: sem diferença de caixa nem de barra), mas
//!   guardados e devolvidos como a pessoa os conhece ([`caminho_exibido`]).
//! - [`Preferencias::recentes`] consulta os arquivos em paralelo com prazo total de 2 s (uma unidade de rede
//!   desconectada não segura a tela); o que não responder a tempo conta como inexistente. `modificado_em` é a
//!   data de modificação local, arredondada ao microssegundo, com as regras de horário de verão da época (as do
//!   Windows, como o Explorer).
//! - `registrar_sql` guarda o texto aparado, ignora texto vazio e comando com mais de 100 000 caracteres;
//!   repetido só sobe para o topo, com o resultado novo. Guarda também as execuções que falharam (desde a 0.2.0):
//!   cada item leva `"resultado"` (`{"tipo": "linhas"|"alteradas"|"executado"|"erro"|"interrompida", …}`, a mensagem do erro
//!   cortada em 300 caracteres) e `"comandos"`; os dois são opcionais, e um item da 0.1.x (sem eles) abre como
//!   antes, sem pílula. Um resultado estranho fica de fora sem alarde. O histórico guarda no máximo 50 bancos; o usado há mais tempo sai primeiro.
//! - `atualizar` recusa (ErroBanco 400, nada gravado): separador de CSV fora de `; , \t` ("O separador do CSV
//!   precisa ser ponto e vírgula, vírgula ou tabulação."), decimal fora de `, .` ("O separador decimal precisa
//!   ser vírgula ou ponto."), última pasta vazia ("A última pasta precisa ser um caminho."), tamanho do código
//!   fora de 8 a 24 px ou NaN ("O tamanho da fonte do código precisa ficar entre 8 e 24 px."), pasta aberta
//!   vazia ("A pasta aberta precisa ser um caminho.").
//! - **A aparência do código** fica na chave `"aparencia"`, depois de `"historico_sql"`:
//!   `{"cores_codigo": "monokai", "fonte_codigo": null, "tamanho_codigo": 12.5}` (`null` = Consolas; o tamanho
//!   em centésimos). A chave só é gravada depois que a pessoa escolhe (a primeira mudança com `aparencia`, mesmo
//!   que repita o padrão). Lida com valor estranho, cada campo volta ao padrão (cores desconhecidas → DataBox;
//!   família vazia ou que não é texto → Consolas; tamanho fora de 8 a 24 px → 12,5); uma `"aparencia"` que não é
//!   objeto conta como não escolhida. As chaves dela que esta versão não conhece são guardadas.
//! - Nenhum método entra em pânico nem devolve erro por causa do disco; problemas de disco vão para o `log`
//!   (`log::warn!`). Todos tocam no disco (no mínimo um `stat`): chame fora da thread da interface.
//!
//! # Chaves que esta versão não conhece
//!
//! - As chaves desconhecidas do arquivo e de cada recente (como o `"tipo"` de um recente, que fica entre elas) são
//!   guardadas e regravadas depois das conhecidas, na ordem lida. O formato não muda (`"versao"` continua 1). Os
//!   valores voltam como o leitor os guarda (inteiro fora do `i64` vira real; surrogate solto, U+FFFD); um valor
//!   que chega ao fundo que o leitor guarda (64 níveis) fica de fora: regravado, sairia cortado.
//! - **Os recentes com tipo.** Cada recente é um banco, uma pasta ou um arquivo de dados ([`TipoRecente`]). O
//!   banco **não leva a chave**. A pasta e o arquivo gravam `"tipo": "pasta"` / `"tipo": "arquivo"` logo depois de
//!   `"aberto_em"` (`{"caminho": "C:\\Projetos", "aberto_em": "2026-10-17T09:00:00", "tipo": "pasta"}`). Sem
//!   `"tipo"` é banco, **exceto** se o caminho é uma pasta no disco: aí vale como pasta (um `"tipo": "banco"`
//!   escrito é respeitado). [`Preferencias::recentes`] não devolve o recente com `"tipo"` de texto desconhecido,
//!   ou que não é texto (de um DataBox mais novo): ele continua no arquivo e conta nos 15.
//! - **A pasta aberta** fica na chave `"pasta_aberta"`, depois de `"larguras_paineis"`: o caminho em texto
//!   (`"pasta_aberta": "C:\\Projetos"`). Só existe enquanto há pasta aberta: [`Mudanca::pasta_aberta`] a grava ao
//!   abrir e a apaga ao fechar. Valor que não é texto, ou texto vazio, conta como nenhuma pasta (e some na próxima
//!   gravação).
//! - **As larguras dos painéis** (as divisórias do Explorador e das Propriedades) ficam na chave
//!   `"larguras_paineis"`, depois da `"aparencia"`: `{"explorador": 300, "propriedades": 272}` (pontos inteiros;
//!   cada uma só existe depois que a pessoa a muda). A chave só é gravada na primeira mudança com `paineis`; sem
//!   ela, a casca usa as larguras padrão. Lida com valor estranho (que não é número, ou fora de 120 a 2000), a
//!   largura volta à padrão; um `"larguras_paineis"` que não é objeto conta como não escolhido. As chaves dele que
//!   esta versão não conhece também são guardadas.
//! - [`Preferencias::adicionar_recente_de`] leva as chaves desconhecidas do item que já estava lá com a mesma chave
//!   de caminho, menos um `"tipo"` que contradiz o novo (o banco só mantém um `"tipo": "banco"` que já estava).

#![forbid(unsafe_code)]

mod caminhos;
mod json;
mod trava;

#[cfg(test)]
mod testes;

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::TimeZone;
use comum::{ErroBanco, Resultado};

pub use caminhos::{caminho_exibido, chave_caminho};
use json::Json;
use trava::TravaEntreProcessos;

pub const MAX_RECENTES: usize = 15;
pub const MAX_COMANDOS: usize = 30;
pub const MAX_BANCOS_NO_HISTORICO: usize = 50;
pub const MAX_CARACTERES_COMANDO: usize = 100_000;
/// A mensagem de erro guardada no histórico é cortada aqui.
pub const MAX_CARACTERES_MENSAGEM: usize = 300;
/// Quanto esperar a outra instância terminar de gravar.
pub const TEMPO_MAXIMO_TRAVA: Duration = Duration::from_secs(3);
/// Prazo total para consultar os arquivos dos recentes.
pub const TEMPO_MAXIMO_CONSULTA_ARQUIVOS: Duration = Duration::from_secs(2);

/// Os separadores de CSV aceitos: ponto e vírgula, vírgula e tabulação.
pub const SEPARADORES_CSV: [char; 3] = [';', ',', '\t'];
/// Os separadores decimais aceitos: vírgula e ponto.
pub const SEPARADORES_DECIMAIS: [char; 2] = [',', '.'];

const ESPERA_LEITURA: Duration = Duration::from_millis(50);
const ESPERA_TROCA: Duration = Duration::from_millis(50);

// ---- tipos públicos ---------------------------------------------------------------------------------

/// Aparência: seguir o sistema, clara ou escura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tema {
    #[default]
    Sistema,
    Claro,
    Escuro,
}

impl Tema {
    pub const TODOS: [Tema; 3] = [Tema::Sistema, Tema::Claro, Tema::Escuro];

    /// O texto do arquivo: "system", "light" ou "dark".
    pub fn como_str(self) -> &'static str {
        match self {
            Tema::Sistema => "system",
            Tema::Claro => "light",
            Tema::Escuro => "dark",
        }
    }

    fn reconhecer(texto: &str) -> Option<Tema> {
        Tema::TODOS.into_iter().find(|t| t.como_str() == texto)
    }

    /// "system" | "light" | "dark" → [`Tema`]; outro texto → ErroBanco 400.
    pub fn de_texto(texto: &str) -> Resultado<Tema> {
        Tema::reconhecer(texto).ok_or_else(|| {
            ErroBanco::novo(format!("Tema desconhecido: {}. Use system, light ou dark.", caminhos::citar(texto)))
        })
    }
}

impl FromStr for Tema {
    type Err = ErroBanco;

    fn from_str(texto: &str) -> Resultado<Tema> {
        Tema::de_texto(texto)
    }
}

impl fmt::Display for Tema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

/// As opções de exportação guardadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcoesExportacao {
    pub csv_separador: char,
    pub csv_decimal: char,
    pub json_indentado: bool,
    /// None: nunca escolhido (a exportação usa cabeçalho).
    pub cabecalho: Option<bool>,
    /// None: nunca escolhido (a janela Exportar deixa marcado).
    pub csv_proteger_formulas: Option<bool>,
}

impl Default for OpcoesExportacao {
    /// O padrão: `;`, `,` e JSON indentado.
    fn default() -> Self {
        OpcoesExportacao {
            csv_separador: ';',
            csv_decimal: ',',
            json_indentado: true,
            cabecalho: None,
            csv_proteger_formulas: None,
        }
    }
}

/// As cores do código (Consulta SQL, JSON, texto e o SQL que as janelas mostram).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CoresCodigo {
    /// As do tema do DataBox, claro ou escuro.
    #[default]
    DataBox,
    AltoContraste,
    SolarizedEscuro,
    Monokai,
}

impl CoresCodigo {
    pub const TODAS: [CoresCodigo; 4] =
        [CoresCodigo::DataBox, CoresCodigo::AltoContraste, CoresCodigo::SolarizedEscuro, CoresCodigo::Monokai];

    /// O texto do arquivo: "databox", "alto_contraste", "solarized_escuro" ou "monokai".
    pub fn como_str(self) -> &'static str {
        match self {
            CoresCodigo::DataBox => "databox",
            CoresCodigo::AltoContraste => "alto_contraste",
            CoresCodigo::SolarizedEscuro => "solarized_escuro",
            CoresCodigo::Monokai => "monokai",
        }
    }

    fn reconhecer(texto: &str) -> Option<CoresCodigo> {
        CoresCodigo::TODAS.into_iter().find(|c| c.como_str() == texto)
    }

    /// "databox" | "alto_contraste" | "solarized_escuro" | "monokai" → [`CoresCodigo`]; outro texto → ErroBanco 400.
    pub fn de_texto(texto: &str) -> Resultado<CoresCodigo> {
        CoresCodigo::reconhecer(texto).ok_or_else(|| {
            ErroBanco::novo(format!(
                "Cores do código desconhecidas: {}. Use databox, alto_contraste, solarized_escuro ou monokai.",
                caminhos::citar(texto)
            ))
        })
    }
}

impl FromStr for CoresCodigo {
    type Err = ErroBanco;

    fn from_str(texto: &str) -> Resultado<CoresCodigo> {
        CoresCodigo::de_texto(texto)
    }
}

impl fmt::Display for CoresCodigo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

/// O tamanho da fonte do código quando a pessoa não escolheu outro (item 16 da revisão).
pub const TAMANHO_CODIGO_PADRAO: f32 = 12.5;
/// A faixa aceita para o tamanho da fonte do código, em px.
pub const TAMANHO_CODIGO_MINIMO: f32 = 8.0;
pub const TAMANHO_CODIGO_MAXIMO: f32 = 24.0;

/// O tamanho está na faixa de [`TAMANHO_CODIGO_MINIMO`] a [`TAMANHO_CODIGO_MAXIMO`] (e não é NaN).
fn tamanho_codigo_valido(tamanho: f64) -> bool {
    (f64::from(TAMANHO_CODIGO_MINIMO)..=f64::from(TAMANHO_CODIGO_MAXIMO)).contains(&tamanho)
}

/// O nome da família guardado: sem espaços nas pontas; vazio vira None (a padrão, Consolas).
fn nome_de_fonte(nome: &str) -> Option<String> {
    let limpo = caminhos::tirar_espacos(nome);
    (!limpo.is_empty()).then(|| limpo.to_string())
}

/// A aparência do código: cores, família e tamanho da fonte.
#[derive(Debug, Clone, PartialEq)]
pub struct Aparencia {
    pub cores_codigo: CoresCodigo,
    /// O nome da família monoespaçada ("Cascadia Mono"); None = a padrão (Consolas).
    pub fonte_codigo: Option<String>,
    /// Em px, de [`TAMANHO_CODIGO_MINIMO`] a [`TAMANHO_CODIGO_MAXIMO`].
    pub tamanho_codigo: f32,
}

impl Default for Aparencia {
    /// Cores DataBox, Consolas, 12,5 px.
    fn default() -> Self {
        Aparencia { cores_codigo: CoresCodigo::DataBox, fonte_codigo: None, tamanho_codigo: TAMANHO_CODIGO_PADRAO }
    }
}

/// A aparência a mesclar na guardada; None = não mexe.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MudancaAparencia {
    pub cores_codigo: Option<CoresCodigo>,
    /// Some(None) volta à família padrão.
    pub fonte_codigo: Option<Option<String>>,
    pub tamanho_codigo: Option<f32>,
}

impl From<Aparencia> for MudancaAparencia {
    fn from(a: Aparencia) -> Self {
        MudancaAparencia {
            cores_codigo: Some(a.cores_codigo),
            fonte_codigo: Some(a.fonte_codigo),
            tamanho_codigo: Some(a.tamanho_codigo),
        }
    }
}

/// A faixa aceita para a largura de um painel lateral (o Explorador, as Propriedades), em pontos. É só a sanidade do
/// arquivo: os limites de verdade (mínimo, 45% da janela, o documento) são da casca.
pub const LARGURA_PAINEL_MINIMA: f32 = 120.0;
pub const LARGURA_PAINEL_MAXIMA: f32 = 2000.0;

/// A largura é um número da faixa de [`LARGURA_PAINEL_MINIMA`] a [`LARGURA_PAINEL_MAXIMA`] (e não é NaN).
fn largura_painel_valida(largura: f64) -> bool {
    (f64::from(LARGURA_PAINEL_MINIMA)..=f64::from(LARGURA_PAINEL_MAXIMA)).contains(&largura)
}

/// As larguras dos painéis laterais que a pessoa escolheu arrastando as divisórias (em pontos, inteiros); None =
/// nunca escolheu, a casca usa a padrão.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LarguraPaineis {
    /// A coluna da esquerda (o Explorador e o Histórico de consultas, que dividem o mesmo lugar).
    pub explorador: Option<f32>,
    /// O painel de Propriedades, à direita.
    pub propriedades: Option<f32>,
}

/// As larguras a mesclar nas guardadas; None = não mexe.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MudancaLarguraPaineis {
    pub explorador: Option<f32>,
    pub propriedades: Option<f32>,
}

/// Opções de exportação a mesclar nas guardadas; None = não mexe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MudancaExportacao {
    pub cabecalho: Option<bool>,
    pub csv_separador: Option<char>,
    pub csv_decimal: Option<char>,
    pub json_indentado: Option<bool>,
    pub csv_proteger_formulas: Option<bool>,
}

impl From<OpcoesExportacao> for MudancaExportacao {
    fn from(o: OpcoesExportacao) -> Self {
        MudancaExportacao {
            cabecalho: o.cabecalho,
            csv_separador: Some(o.csv_separador),
            csv_decimal: Some(o.csv_decimal),
            json_indentado: Some(o.json_indentado),
            csv_proteger_formulas: o.csv_proteger_formulas,
        }
    }
}

/// O que `atualizar` muda; None em cada campo = deixa como está.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mudanca {
    pub tema: Option<Tema>,
    pub exportacao: Option<MudancaExportacao>,
    /// Some(None) apaga a última pasta; um arquivo vira a pasta dele.
    pub ultimo_diretorio: Option<Option<PathBuf>>,
    /// Some grava a chave `"aparencia"` (ver [`Preferencias::atualizar`]).
    pub aparencia: Option<MudancaAparencia>,
    /// Some grava a chave `"larguras_paineis"` (ver [`Preferencias::atualizar`]).
    pub paineis: Option<MudancaLarguraPaineis>,
    /// Some(Some(p)) grava a chave `"pasta_aberta"` (a pasta que acabou de abrir); Some(None) a apaga (o Fechar
    /// pasta); None não mexe.
    pub pasta_aberta: Option<Option<PathBuf>>,
}

impl Mudanca {
    pub fn pasta_aberta(pasta: Option<PathBuf>) -> Mudanca {
        Mudanca { pasta_aberta: Some(pasta), ..Mudanca::default() }
    }

    pub fn paineis(paineis: MudancaLarguraPaineis) -> Mudanca {
        Mudanca { paineis: Some(paineis), ..Mudanca::default() }
    }

    pub fn tema(tema: Tema) -> Mudanca {
        Mudanca { tema: Some(tema), ..Mudanca::default() }
    }

    pub fn exportacao(exportacao: MudancaExportacao) -> Mudanca {
        Mudanca { exportacao: Some(exportacao), ..Mudanca::default() }
    }

    pub fn aparencia(aparencia: MudancaAparencia) -> Mudanca {
        Mudanca { aparencia: Some(aparencia), ..Mudanca::default() }
    }

    pub fn ultimo_diretorio(pasta: Option<PathBuf>) -> Mudanca {
        Mudanca { ultimo_diretorio: Some(pasta), ..Mudanca::default() }
    }
}

/// O que um recente é: um banco, uma pasta ou um arquivo de dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TipoRecente {
    /// O padrão: sem a chave `"tipo"` no arquivo.
    #[default]
    Banco,
    Pasta,
    Arquivo,
}

impl TipoRecente {
    pub const TODOS: [TipoRecente; 3] = [TipoRecente::Banco, TipoRecente::Pasta, TipoRecente::Arquivo];

    /// O texto do `"tipo"` no arquivo: "banco", "pasta" ou "arquivo".
    pub fn como_str(self) -> &'static str {
        match self {
            TipoRecente::Banco => "banco",
            TipoRecente::Pasta => "pasta",
            TipoRecente::Arquivo => "arquivo",
        }
    }

    fn reconhecer(texto: &str) -> Option<TipoRecente> {
        TipoRecente::TODOS.into_iter().find(|t| t.como_str() == texto)
    }

    /// "banco" | "pasta" | "arquivo" → [`TipoRecente`]; outro texto → ErroBanco 400.
    pub fn de_texto(texto: &str) -> Resultado<TipoRecente> {
        TipoRecente::reconhecer(texto).ok_or_else(|| {
            ErroBanco::novo(format!(
                "Tipo de recente desconhecido: {}. Use banco, pasta ou arquivo.",
                caminhos::citar(texto)
            ))
        })
    }
}

impl FromStr for TipoRecente {
    type Err = ErroBanco;

    fn from_str(texto: &str) -> Resultado<TipoRecente> {
        TipoRecente::de_texto(texto)
    }
}

impl fmt::Display for TipoRecente {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

/// Um banco, pasta ou arquivo aberto recentemente, com o estado do caminho consultado agora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recente {
    /// Como a pessoa o conhece (absoluto, caixa original).
    pub caminho: String,
    /// `os.path.basename(caminho)`.
    pub nome: String,
    /// `os.path.dirname(caminho)`.
    pub pasta: String,
    /// O que é. Sem `"tipo"` no arquivo é banco, a não ser que o caminho seja uma pasta no disco.
    pub tipo: TipoRecente,
    /// Pasta: é uma pasta que respondeu a tempo. Banco e arquivo: é um arquivo comum que respondeu a tempo.
    pub existe: bool,
    /// O tamanho do arquivo; None na pasta (e no que não existe).
    pub tamanho: Option<u64>,
    /// Data de modificação, ISO local sem fração ("2026-10-07T14:03:05").
    pub modificado_em: Option<String>,
    /// Quando foi aberto pela última vez, ISO local.
    pub aberto_em: Option<String>,
}

/// Um comando do histórico da Consulta SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoSql {
    pub sql: String,
    /// ISO local sem fração.
    pub quando: Option<String>,
    /// O que a última execução deu (None nos comandos guardados antes da 0.2.0, que só guardava os que deram certo).
    pub resultado: Option<ResultadoSql>,
    /// Quantos comandos o roteiro executou (até o que parou, se parou); None como no `resultado`.
    pub comandos: Option<u32>,
}

/// O que uma execução da Consulta SQL deu: a pílula do Histórico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoSql {
    /// O último comando devolveu linhas; `mais`: a lista parou no limite e havia mais.
    Linhas { quantidade: u64, mais: bool },
    /// O último comando alterou linhas (INSERT, UPDATE, DELETE, REPLACE).
    Alteradas(u64),
    /// O último comando rodou sem contagem (CREATE, PRAGMA sem resultado, BEGIN…).
    Executado,
    /// O roteiro parou no comando `numero` (do 1); os anteriores já rodaram. `mensagem`: a frase do erro, cortada
    /// em [`MAX_CARACTERES_MENSAGEM`].
    Erro { numero: u32, mensagem: String },
    /// A pessoa interrompeu o roteiro no comando `numero` (do 1); os anteriores já rodaram.
    Interrompida { numero: u32 },
}

impl ResultadoSql {
    /// O erro, com a mensagem cortada em [`MAX_CARACTERES_MENSAGEM`] (com "…").
    pub fn erro(numero: u32, mensagem: &str) -> ResultadoSql {
        let mensagem = caminhos::tirar_espacos(mensagem);
        let mensagem = if mensagem.chars().count() > MAX_CARACTERES_MENSAGEM {
            let mut curta: String = mensagem.chars().take(MAX_CARACTERES_MENSAGEM - 1).collect();
            curta.push('…');
            curta
        } else {
            mensagem.to_string()
        };
        ResultadoSql::Erro { numero, mensagem }
    }

    /// `{"tipo": "linhas", "quantidade": 9}`, `{"tipo": "alteradas", "quantidade": 3}`, `{"tipo": "executado"}`,
    /// `{"tipo": "erro", "comando": 3, "mensagem": "…"}`, `{"tipo": "interrompida", "comando": 1}`; `"mais": true`
    /// só nas linhas cortadas.
    fn para_json(&self) -> Json {
        let tipo = |t: &str| ("tipo".to_string(), Json::Texto(t.to_string()));
        let quantidade = |n: u64| ("quantidade".to_string(), Json::Inteiro(i64::try_from(n).unwrap_or(i64::MAX)));
        Json::Objeto(match self {
            ResultadoSql::Linhas { quantidade: n, mais } => {
                let mut pares = vec![tipo("linhas"), quantidade(*n)];
                if *mais {
                    pares.push(("mais".to_string(), Json::Bool(true)));
                }
                pares
            }
            ResultadoSql::Alteradas(n) => vec![tipo("alteradas"), quantidade(*n)],
            ResultadoSql::Executado => vec![tipo("executado")],
            ResultadoSql::Erro { numero, mensagem } => vec![
                tipo("erro"),
                ("comando".to_string(), Json::Inteiro(i64::from(*numero))),
                ("mensagem".to_string(), Json::Texto(mensagem.clone())),
            ],
            ResultadoSql::Interrompida { numero } => {
                vec![tipo("interrompida"), ("comando".to_string(), Json::Inteiro(i64::from(*numero)))]
            }
        })
    }

    /// O contrário de [`ResultadoSql::para_json`]; o que não fizer sentido é None (o item fica sem resultado).
    fn de_json(valor: &Json) -> Option<ResultadoSql> {
        let quantidade = || match valor.campo("quantidade") {
            Some(Json::Inteiro(n)) => u64::try_from(*n).ok(),
            _ => None,
        };
        let numero = || match valor.campo("comando") {
            Some(Json::Inteiro(n)) => u32::try_from(*n).ok().filter(|n| *n > 0),
            _ => None,
        };
        match valor.campo("tipo").and_then(Json::como_texto)? {
            "linhas" => Some(ResultadoSql::Linhas {
                quantidade: quantidade()?,
                mais: matches!(valor.campo("mais"), Some(Json::Bool(true))),
            }),
            "alteradas" => Some(ResultadoSql::Alteradas(quantidade()?)),
            "executado" => Some(ResultadoSql::Executado),
            "erro" => {
                let mensagem = valor.campo("mensagem").and_then(Json::como_texto).unwrap_or_default();
                Some(ResultadoSql::erro(numero()?, mensagem))
            }
            "interrompida" => Some(ResultadoSql::Interrompida { numero: numero()? }),
            _ => None,
        }
    }
}

/// O resumo para a interface (com a aparência do código).
#[derive(Debug, Clone, PartialEq)]
pub struct PrefsInfo {
    pub tema: Tema,
    pub recentes: Vec<Recente>,
    pub exportacao: OpcoesExportacao,
    pub aparencia: Aparencia,
}

/// O nome do arquivo, o mesmo nas duas versões.
const NOME_ARQUIVO: &str = "preferencias.json";

/// `%APPDATA%\DataBox\preferencias.json` (sem APPDATA, a pasta pessoal; fora do Windows, a pasta de
/// configuração do XDG): ver `comum::app::pasta_de_configuracao`.
pub fn arquivo_padrao() -> PathBuf {
    comum::app::pasta_de_configuracao().join(comum::app::PASTA).join(NOME_ARQUIVO)
}

/// O [`arquivo_padrao`] com o APPDATA e a pasta pessoal vindos de fora (os testes não mexem nas variáveis do
/// processo).
#[cfg(test)]
fn arquivo_padrao_de(appdata: Option<std::ffi::OsString>, casa: impl FnOnce() -> PathBuf) -> PathBuf {
    comum::app::base_ou(appdata, casa).join(comum::app::PASTA).join(NOME_ARQUIVO)
}

// ---- dados em memória -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extra {
    Cabecalho,
    CsvProtegerFormulas,
}

/// As opções e a ordem em que as opcionais entraram no dicionário (para gravar sempre na mesma ordem).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Exportacao {
    opcoes: OpcoesExportacao,
    ordem_extras: Vec<Extra>,
}

impl Exportacao {
    fn definir_extra(&mut self, extra: Extra, valor: bool) {
        match extra {
            Extra::Cabecalho => self.opcoes.cabecalho = Some(valor),
            Extra::CsvProtegerFormulas => self.opcoes.csv_proteger_formulas = Some(valor),
        }
        if !self.ordem_extras.contains(&extra) {
            self.ordem_extras.push(extra);
        }
    }

    /// `dados["exportacao"].update(opcoes)`, na ordem em que o servidor manda as opções.
    fn aplicar(&mut self, m: &MudancaExportacao) {
        if let Some(v) = m.cabecalho {
            self.definir_extra(Extra::Cabecalho, v);
        }
        if let Some(v) = m.csv_separador {
            self.opcoes.csv_separador = v;
        }
        if let Some(v) = m.csv_decimal {
            self.opcoes.csv_decimal = v;
        }
        if let Some(v) = m.json_indentado {
            self.opcoes.json_indentado = v;
        }
        if let Some(v) = m.csv_proteger_formulas {
            self.definir_extra(Extra::CsvProtegerFormulas, v);
        }
    }

    /// Aproveita uma opção lida do arquivo se ela for válida.
    fn aproveitar(&mut self, nome: &str, valor: &Json) {
        let um_de = |permitidos: &[char]| {
            valor.como_texto().and_then(|t| {
                let mut c = t.chars();
                match (c.next(), c.next()) {
                    (Some(x), None) if permitidos.contains(&x) => Some(x),
                    _ => None,
                }
            })
        };
        match (nome, valor) {
            ("csv_separador", _) => {
                if let Some(c) = um_de(&SEPARADORES_CSV) {
                    self.opcoes.csv_separador = c;
                }
            }
            ("csv_decimal", _) => {
                if let Some(c) = um_de(&SEPARADORES_DECIMAIS) {
                    self.opcoes.csv_decimal = c;
                }
            }
            ("json_indentado", Json::Bool(b)) => self.opcoes.json_indentado = *b,
            ("cabecalho", Json::Bool(b)) => self.definir_extra(Extra::Cabecalho, *b),
            ("csv_proteger_formulas", Json::Bool(b)) => self.definir_extra(Extra::CsvProtegerFormulas, *b),
            _ => {}
        }
    }

    fn para_json(&self) -> Json {
        let o = &self.opcoes;
        let mut pares = vec![
            ("csv_separador".to_string(), Json::Texto(o.csv_separador.to_string())),
            ("csv_decimal".to_string(), Json::Texto(o.csv_decimal.to_string())),
            ("json_indentado".to_string(), Json::Bool(o.json_indentado)),
        ];
        for extra in &self.ordem_extras {
            let (nome, valor) = match extra {
                Extra::Cabecalho => ("cabecalho", o.cabecalho),
                Extra::CsvProtegerFormulas => ("csv_proteger_formulas", o.csv_proteger_formulas),
            };
            if let Some(v) = valor {
                pares.push((nome.to_string(), Json::Bool(v)));
            }
        }
        Json::Objeto(pares)
    }
}

/// Pares de um objeto do arquivo que esta versão não conhece, na ordem lida.
type Extras = Vec<(String, Json)>;

/// As chaves do arquivo que esta versão conhece; as outras ficam em [`Dados::extras`].
const CHAVES_DO_ARQUIVO: [&str; 9] = [
    "versao",
    "tema",
    "exportacao",
    "ultimo_diretorio",
    "recentes",
    "historico_sql",
    "aparencia",
    "larguras_paineis",
    "pasta_aberta",
];
/// As chaves de cada recente que esta versão conhece; as outras ficam em [`RecenteGuardado::extras`].
const CHAVES_DO_RECENTE: [&str; 2] = ["caminho", "aberto_em"];
/// As chaves da `"aparencia"` que esta versão conhece; as outras ficam em [`AparenciaGuardada::extras`].
const CHAVES_DA_APARENCIA: [&str; 3] = ["cores_codigo", "fonte_codigo", "tamanho_codigo"];
/// As chaves dos `"larguras_paineis"` que esta versão conhece; as outras ficam em [`PaineisGuardados::extras`].
const CHAVES_DOS_PAINEIS: [&str; 2] = ["explorador", "propriedades"];
/// A chave com que o DataBox diz o que é cada recente; sem ela, é um banco (ou uma pasta, se o caminho é uma pasta
/// no disco). Fica entre as chaves desconhecidas do item, logo depois de `"aberto_em"`.
const CHAVE_TIPO: &str = "tipo";

/// Em que nível do arquivo ficam os contêineres pais dos extras: o objeto de fora é o 1º; a `"aparencia"`, o 2º;
/// cada recente, o 3º.
const NIVEL_DO_ARQUIVO: usize = 1;
const NIVEL_DA_APARENCIA: usize = 2;
const NIVEL_DOS_PAINEIS: usize = 2;
const NIVEL_DO_RECENTE: usize = 3;

/// A aparência escolhida, com as chaves dela que esta versão não conhece.
#[derive(Debug, Clone, PartialEq)]
struct AparenciaGuardada {
    aparencia: Aparencia,
    extras: Extras,
}

impl AparenciaGuardada {
    /// Mescla `m`; o tamanho e a família já vêm conferidos por [`Preferencias::atualizar`].
    fn aplicar(&mut self, m: &MudancaAparencia) {
        let a = &mut self.aparencia;
        if let Some(c) = m.cores_codigo {
            a.cores_codigo = c;
        }
        if let Some(f) = &m.fonte_codigo {
            a.fonte_codigo = f.as_deref().and_then(nome_de_fonte);
        }
        if let Some(t) = m.tamanho_codigo {
            a.tamanho_codigo = t;
        }
    }

    /// Aproveita o que der do objeto lido; campo estranho volta ao padrão.
    fn de_json(objeto: &Json) -> AparenciaGuardada {
        let mut a = Aparencia::default();
        if let Some(c) = objeto.campo("cores_codigo").and_then(Json::como_texto).and_then(CoresCodigo::reconhecer) {
            a.cores_codigo = c;
        }
        a.fonte_codigo = objeto.campo("fonte_codigo").and_then(Json::como_texto).and_then(nome_de_fonte);
        let tamanho = match objeto.campo("tamanho_codigo") {
            Some(Json::Inteiro(n)) => Some(*n as f64),
            Some(Json::Real(r)) => Some(*r),
            _ => None,
        };
        if let Some(t) = tamanho.filter(|t| tamanho_codigo_valido(*t)) {
            a.tamanho_codigo = t as f32;
        }
        AparenciaGuardada { aparencia: a, extras: extras_de(objeto, &CHAVES_DA_APARENCIA, NIVEL_DA_APARENCIA) }
    }

    fn para_json(&self) -> Json {
        let a = &self.aparencia;
        // o f32 em centésimos: 12,3 sai "12.3", não "12.300000190734863"
        let tamanho = (f64::from(a.tamanho_codigo) * 100.0).round() / 100.0;
        let mut pares = vec![
            ("cores_codigo".to_string(), Json::Texto(a.cores_codigo.como_str().to_string())),
            ("fonte_codigo".to_string(), texto_ou_nulo(&a.fonte_codigo)),
            ("tamanho_codigo".to_string(), Json::Real(tamanho)),
        ];
        pares.extend(self.extras.iter().cloned());
        Json::Objeto(pares)
    }
}

/// As larguras escolhidas, com as chaves dos `"larguras_paineis"` que esta versão não conhece.
#[derive(Debug, Clone, PartialEq)]
struct PaineisGuardados {
    larguras: LarguraPaineis,
    extras: Extras,
}

impl PaineisGuardados {
    /// Mescla `m`; as larguras já vêm conferidas por [`Preferencias::atualizar`] e saem em pontos inteiros.
    fn aplicar(&mut self, m: &MudancaLarguraPaineis) {
        if let Some(l) = m.explorador {
            self.larguras.explorador = Some(l.round());
        }
        if let Some(l) = m.propriedades {
            self.larguras.propriedades = Some(l.round());
        }
    }

    /// Aproveita o que der do objeto lido; largura estranha (que não é número, ou fora da faixa) volta à padrão.
    fn de_json(objeto: &Json) -> PaineisGuardados {
        let largura = |chave: &str| {
            let valor = match objeto.campo(chave) {
                Some(Json::Inteiro(n)) => Some(*n as f64),
                Some(Json::Real(r)) => Some(*r),
                _ => None,
            };
            valor.filter(|v| largura_painel_valida(*v)).map(|v| (v as f32).round())
        };
        PaineisGuardados {
            larguras: LarguraPaineis { explorador: largura("explorador"), propriedades: largura("propriedades") },
            extras: extras_de(objeto, &CHAVES_DOS_PAINEIS, NIVEL_DOS_PAINEIS),
        }
    }

    /// Só as larguras escolhidas entram, em pontos inteiros.
    fn para_json(&self) -> Json {
        let l = &self.larguras;
        let mut pares = Vec::new();
        for (chave, largura) in [("explorador", l.explorador), ("propriedades", l.propriedades)] {
            if let Some(v) = largura {
                pares.push((chave.to_string(), Json::Inteiro(v.round() as i64)));
            }
        }
        pares.extend(self.extras.iter().cloned());
        Json::Objeto(pares)
    }
}

/// Os pares de `objeto` fora de `conhecidas`, na ordem lida. Um valor que chega ao fundo que o leitor guarda
/// ([`json::PROFUNDIDADE_GUARDADA`]) fica de fora: regravado, sairia cortado.
fn extras_de(objeto: &Json, conhecidas: &[&str], nivel: usize) -> Extras {
    let Json::Objeto(pares) = objeto else {
        return Extras::new();
    };
    pares
        .iter()
        .filter(|(chave, valor)| {
            !conhecidas.contains(&chave.as_str()) && nivel + valor.profundidade() < json::PROFUNDIDADE_GUARDADA
        })
        .cloned()
        .collect()
}

/// O que a chave `"tipo"` de um recente diz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoLido {
    /// Sem a chave (o formato de sempre): banco, ou pasta se o caminho for uma pasta no disco.
    Ausente,
    Conhecido(TipoRecente),
    /// Texto que não conhecemos, ou que não é texto: o item fica escondido e guardado.
    Estranho,
}

#[derive(Debug, Clone, PartialEq)]
struct RecenteGuardado {
    caminho: String,
    aberto_em: Option<String>,
    /// As chaves que esta versão não conhece, e o `"tipo"` da pasta e do arquivo (o banco não o leva).
    extras: Extras,
}

impl RecenteGuardado {
    fn tipo_lido(&self) -> TipoLido {
        match self.extras.iter().find(|(chave, _)| chave == CHAVE_TIPO) {
            None => TipoLido::Ausente,
            Some((_, valor)) => {
                valor.como_texto().and_then(TipoRecente::reconhecer).map_or(TipoLido::Estranho, TipoLido::Conhecido)
            }
        }
    }

    /// Um dos três tipos que esta versão mostra e que o Limpar a lista tira (sem a chave, é banco).
    fn tipo_conhecido(&self) -> bool {
        self.tipo_lido() != TipoLido::Estranho
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Dados {
    tema: Tema,
    exportacao: Exportacao,
    ultimo_diretorio: Option<String>,
    recentes: Vec<RecenteGuardado>,
    /// Na ordem de uso: o primeiro é o banco usado há mais tempo.
    historico_sql: Vec<(String, Vec<ComandoSql>)>,
    /// None enquanto a pessoa não escolher: sem a chave no arquivo.
    aparencia: Option<AparenciaGuardada>,
    /// None enquanto a pessoa não arrastar uma divisória: sem a chave no arquivo.
    paineis: Option<PaineisGuardados>,
    /// O caminho da pasta aberta; None enquanto não há pasta aberta (sem a chave no arquivo).
    pasta_aberta: Option<String>,
    /// As chaves do arquivo que esta versão não conhece (gravadas por uma versão mais nova).
    extras: Extras,
}

fn texto_ou_nulo(t: &Option<String>) -> Json {
    t.as_ref().map_or(Json::Nulo, |s| Json::Texto(s.clone()))
}

impl Dados {
    /// As preferências padrão.
    fn padrao() -> Dados {
        Dados {
            tema: Tema::Sistema,
            exportacao: Exportacao::default(),
            ultimo_diretorio: None,
            recentes: Vec::new(),
            historico_sql: Vec::new(),
            aparencia: None,
            paineis: None,
            pasta_aberta: None,
            extras: Extras::new(),
        }
    }

    /// O objeto gravado, sempre na mesma ordem de chaves; os extras vêm depois das chaves conhecidas.
    fn para_json(&self) -> Json {
        let recentes = self
            .recentes
            .iter()
            .map(|r| {
                let mut pares = vec![
                    ("caminho".to_string(), Json::Texto(r.caminho.clone())),
                    ("aberto_em".to_string(), texto_ou_nulo(&r.aberto_em)),
                ];
                pares.extend(r.extras.iter().cloned());
                Json::Objeto(pares)
            })
            .collect();
        let historico = self
            .historico_sql
            .iter()
            .map(|(chave, itens)| {
                let itens = itens
                    .iter()
                    .map(|i| {
                        let mut pares = vec![
                            ("sql".to_string(), Json::Texto(i.sql.clone())),
                            ("quando".to_string(), texto_ou_nulo(&i.quando)),
                        ];
                        if let Some(resultado) = &i.resultado {
                            pares.push(("resultado".to_string(), resultado.para_json()));
                        }
                        if let Some(n) = i.comandos {
                            pares.push(("comandos".to_string(), Json::Inteiro(i64::from(n))));
                        }
                        Json::Objeto(pares)
                    })
                    .collect();
                (chave.clone(), Json::Lista(itens))
            })
            .collect();
        let mut pares = vec![
            ("versao".to_string(), Json::Inteiro(1)),
            ("tema".to_string(), Json::Texto(self.tema.como_str().to_string())),
            ("exportacao".to_string(), self.exportacao.para_json()),
            ("ultimo_diretorio".to_string(), texto_ou_nulo(&self.ultimo_diretorio)),
            ("recentes".to_string(), Json::Lista(recentes)),
            ("historico_sql".to_string(), Json::Objeto(historico)),
        ];
        if let Some(aparencia) = &self.aparencia {
            pares.push(("aparencia".to_string(), aparencia.para_json()));
        }
        if let Some(paineis) = &self.paineis {
            pares.push(("larguras_paineis".to_string(), paineis.para_json()));
        }
        if let Some(pasta) = &self.pasta_aberta {
            pares.push(("pasta_aberta".to_string(), Json::Texto(pasta.clone())));
        }
        pares.extend(self.extras.iter().cloned());
        Json::Objeto(pares)
    }

    /// O texto do arquivo, byte a byte o do `json.dump(dados, f, ensure_ascii=False, indent=2)`.
    fn serializar(&self) -> String {
        json::escrever(&self.para_json())
    }
}

/// Aproveita o que der de um JSON lido do disco; o que não fizer sentido volta ao padrão.
/// Guarda as chaves que esta versão não conhece (no arquivo e em cada recente).
fn sanear(bruto: &Json, max_bancos: usize) -> Result<Dados, String> {
    if !matches!(bruto, Json::Objeto(_)) {
        return Err("o arquivo de preferências não contém um objeto JSON".to_string());
    }
    let mut dados = Dados::padrao();
    dados.extras = extras_de(bruto, &CHAVES_DO_ARQUIVO, NIVEL_DO_ARQUIVO);
    if let Some(tema) = bruto.campo("tema").and_then(Json::como_texto).and_then(Tema::reconhecer) {
        dados.tema = tema;
    }
    if let Some(Json::Objeto(pares)) = bruto.campo("exportacao") {
        for (nome, valor) in pares {
            dados.exportacao.aproveitar(nome, valor);
        }
    }
    if let Some(ultimo) = bruto.campo("ultimo_diretorio").and_then(Json::como_texto)
        && !caminhos::tirar_espacos(ultimo).is_empty()
    {
        dados.ultimo_diretorio = Some(ultimo.to_string());
    }

    if let Some(Json::Lista(itens)) = bruto.campo("recentes") {
        let mut vistos = std::collections::HashSet::new();
        for item in itens {
            let Some(caminho) = item.campo("caminho").and_then(Json::como_texto) else {
                continue;
            };
            if caminhos::tirar_espacos(caminho).is_empty() || !vistos.insert(chave_caminho(caminho)) {
                continue;
            }
            let aberto_em = item.campo("aberto_em").and_then(Json::como_texto).map(str::to_string);
            let extras = extras_de(item, &CHAVES_DO_RECENTE, NIVEL_DO_RECENTE);
            dados.recentes.push(RecenteGuardado { caminho: caminho.to_string(), aberto_em, extras });
        }
        dados.recentes.truncate(MAX_RECENTES);
    }

    if let Some(Json::Objeto(bancos)) = bruto.campo("historico_sql") {
        for (chave, itens) in bancos {
            let Json::Lista(itens) = itens else {
                continue;
            };
            let mut limpos: Vec<ComandoSql> = Vec::new();
            let mut textos = std::collections::HashSet::new();
            for item in itens {
                let Some(sql) = item.campo("sql").and_then(Json::como_texto) else {
                    continue;
                };
                if caminhos::tirar_espacos(sql).is_empty() || !textos.insert(sql) {
                    continue;
                }
                let quando = item.campo("quando").and_then(Json::como_texto).map(str::to_string);
                let resultado = item.campo("resultado").and_then(ResultadoSql::de_json);
                let comandos = match item.campo("comandos") {
                    Some(Json::Inteiro(n)) => u32::try_from(*n).ok().filter(|n| *n > 0),
                    _ => None,
                };
                limpos.push(ComandoSql { sql: sql.to_string(), quando, resultado, comandos });
            }
            if !limpos.is_empty() {
                limpos.truncate(MAX_COMANDOS);
                dados.historico_sql.push((chave.clone(), limpos));
            }
        }
        if dados.historico_sql.len() > max_bancos {
            let sobra = dados.historico_sql.len() - max_bancos;
            dados.historico_sql.drain(..sobra);
        }
    }

    // uma "aparencia" que não é objeto conta como não escolhida
    if let Some(objeto @ Json::Objeto(_)) = bruto.campo("aparencia") {
        dados.aparencia = Some(AparenciaGuardada::de_json(objeto));
    }
    // idem os "larguras_paineis"
    if let Some(objeto @ Json::Objeto(_)) = bruto.campo("larguras_paineis") {
        dados.paineis = Some(PaineisGuardados::de_json(objeto));
    }
    // uma "pasta_aberta" que não é texto, ou é vazia, conta como nenhuma pasta aberta
    if let Some(pasta) = bruto.campo("pasta_aberta").and_then(Json::como_texto)
        && !caminhos::tirar_espacos(pasta).is_empty()
    {
        dados.pasta_aberta = Some(pasta.to_string());
    }
    Ok(dados)
}

// ---- disco ------------------------------------------------------------------------------------------

/// As operações de disco que os testes substituem para simular falhas.
pub(crate) trait Disco: Send + Sync {
    fn ler(&self, caminho: &Path) -> io::Result<Vec<u8>> {
        fs::read(caminho)
    }

    fn escrever(&self, arquivo: &mut File, bytes: &[u8]) -> io::Result<()> {
        arquivo.write_all(bytes)
    }

    /// `os.replace`: troca `destino` por `origem`.
    fn trocar(&self, origem: &Path, destino: &Path) -> io::Result<()> {
        fs::rename(origem, destino)
    }
}

struct DiscoReal;

impl Disco for DiscoReal {}

/// Erro em que vale tentar a troca de novo (acesso negado, arquivo em uso, trava).
fn e_permissao(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// (tamanho, modificado_em) de um arquivo comum ou de uma pasta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EstadoArquivo {
    /// O tamanho do arquivo; 0 numa pasta (que não o mostra).
    tamanho: u64,
    modificado_em: Option<String>,
    /// É uma pasta (senão, um arquivo comum).
    e_pasta: bool,
}

type ConsultaArquivo = Arc<dyn Fn(&str) -> Option<EstadoArquivo> + Send + Sync>;

/// O estado do arquivo, que também responde pela pasta: None se não existe ou não é arquivo comum nem
/// pasta.
pub(crate) fn estado_do_arquivo(caminho: &str) -> Option<EstadoArquivo> {
    let meta = fs::metadata(caminho).ok()?;
    let e_pasta = meta.is_dir();
    if !e_pasta && !meta.is_file() {
        return None;
    }
    Some(EstadoArquivo {
        tamanho: if e_pasta { 0 } else { meta.len() },
        modificado_em: meta.modified().ok().and_then(iso_de_mtime),
        e_pasta,
    })
}

/// A data de modificação local em ISO, até os segundos (`2026-10-07T14:03:05`), com a hora em ponto flutuante
/// arredondada ao microssegundo (meio para o par) antes de cortar.
fn iso_de_mtime(t: SystemTime) -> Option<String> {
    let (segundos, nanos): (i128, i128) = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => (i128::from(d.as_secs()), i128::from(d.subsec_nanos())),
        Err(e) => {
            let total = -(e.duration().as_nanos() as i128);
            (total.div_euclid(1_000_000_000), total.rem_euclid(1_000_000_000))
        }
    };
    let d = segundos as f64 + nanos as f64 * 1e-9;
    let inteiro = d.trunc();
    let fracao = arredondar_meio_par((d - inteiro) * 1e6);
    let mut s = inteiro;
    if fracao >= 1e6 {
        s += 1.0;
    } else if fracao < 0.0 {
        s -= 1.0;
    }
    let local = chrono::Local.timestamp_opt(s as i64, 0).earliest()?;
    Some(local.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn arredondar_meio_par(x: f64) -> f64 {
    let r = x.round();
    if (x - r).abs() == 0.5 { 2.0 * (x / 2.0).round() } else { r }
}

/// `datetime.now().isoformat(timespec="seconds")`.
fn agora() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Consulta os arquivos em paralelo, com prazo total. O que não responder a
/// tempo fica None; a thread dele termina sozinha depois.
fn consultar_arquivos(
    caminhos: &[String],
    tempo_maximo: Duration,
    consulta: &ConsultaArquivo,
) -> Vec<Option<EstadoArquivo>> {
    let mut resultados: Vec<Option<EstadoArquivo>> = vec![None; caminhos.len()];
    let (tx, rx) = mpsc::channel();
    let mut pendentes = 0usize;
    for (indice, caminho) in caminhos.iter().enumerate() {
        let tx = tx.clone();
        let consulta = Arc::clone(consulta);
        let caminho = caminho.clone();
        let iniciou = thread::Builder::new().name("prefs-recente".to_string()).spawn(move || {
            let _ = tx.send((indice, consulta(&caminho)));
        });
        if iniciou.is_ok() {
            pendentes += 1;
        }
    }
    drop(tx);
    let fim = Instant::now() + tempo_maximo;
    while pendentes > 0 {
        match rx.recv_timeout(fim.saturating_duration_since(Instant::now())) {
            Ok((indice, estado)) => {
                if let Some(lugar) = resultados.get_mut(indice) {
                    *lugar = estado;
                }
                pendentes -= 1;
            }
            Err(_) => break,
        }
    }
    resultados
}

/// Um sufixo de 8 caracteres para o temporário.
fn sufixo_aleatorio() -> String {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(CONTADOR.fetch_add(1, Ordering::Relaxed));
    h.write_u32(std::process::id());
    h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let mut n = h.finish();
    const LETRAS: &[u8; 37] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut s = String::with_capacity(8);
    for _ in 0..8 {
        s.push(char::from(LETRAS.get((n % 37) as usize).copied().unwrap_or(b'x')));
        n /= 37;
    }
    s
}

/// `caminho` com `sufixo` colado no nome (`preferencias.json` → `preferencias.json.lock`).
fn com_sufixo(caminho: &Path, sufixo: &str) -> PathBuf {
    let mut nome = caminho.as_os_str().to_owned();
    nome.push(sufixo);
    PathBuf::from(nome)
}

// ---- Preferencias -----------------------------------------------------------------------------------

/// O arquivo existe, mas não pôde ser lido agora (outro programa com ele aberto).
struct Ocupado;

type Assinatura = (Option<SystemTime>, u64);

#[derive(Default)]
struct Estado {
    dados: Option<Dados>,
    /// A do arquivo na última leitura ou gravação; None = ele não existia.
    assinatura: Option<Assinatura>,
}

impl Estado {
    /// O que vale com o arquivo ausente: se ele também não existia na última leitura, o que está em memória (uma
    /// mudança que o disco recusou); se ele sumiu desde então, o padrão.
    fn sem_arquivo(&self) -> Dados {
        match (&self.dados, self.assinatura) {
            (Some(dados), None) => dados.clone(),
            _ => Dados::padrao(),
        }
    }
}

/// Preferências de uma pessoa, seguras para uso por várias threads (`Arc<Preferencias>`).
pub struct Preferencias {
    arquivo: PathBuf,
    estado: Mutex<Estado>,
    disco: Arc<dyn Disco>,
    consulta: ConsultaArquivo,
    max_bancos_no_historico: usize,
    tempo_maximo_consulta: Duration,
    /// Antes desta hora, a próxima mudança não tenta a trava: outra instância bateu enquanto esta gravava (ver
    /// `trava.rs`, "A vez").
    vez: Mutex<Option<Instant>>,
}

impl fmt::Debug for Preferencias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Preferencias").field("arquivo", &self.arquivo).finish_non_exhaustive()
    }
}

impl Preferencias {
    /// As preferências em `arquivo` (None → [`arquivo_padrao`]). Não toca no disco: o arquivo é lido na
    /// primeira consulta e só é criado na primeira mudança.
    pub fn abrir(arquivo: Option<PathBuf>) -> Preferencias {
        Preferencias::com_disco(arquivo.unwrap_or_else(arquivo_padrao), Arc::new(DiscoReal))
    }

    pub(crate) fn com_disco(arquivo: PathBuf, disco: Arc<dyn Disco>) -> Preferencias {
        Preferencias {
            arquivo,
            estado: Mutex::new(Estado::default()),
            disco,
            consulta: Arc::new(estado_do_arquivo),
            max_bancos_no_historico: MAX_BANCOS_NO_HISTORICO,
            tempo_maximo_consulta: TEMPO_MAXIMO_CONSULTA_ARQUIVOS,
            vez: Mutex::new(None),
        }
    }

    /// O `preferencias.json`.
    pub fn arquivo(&self) -> &Path {
        &self.arquivo
    }

    /// O `preferencias.json.lock` ao lado (fica, vazio, de propósito).
    pub fn arquivo_trava(&self) -> PathBuf {
        self.com_sufixo(".lock")
    }

    fn com_sufixo(&self, sufixo: &str) -> PathBuf {
        com_sufixo(&self.arquivo, sufixo)
    }

    /// A trava interna; um pânico em outra thread não a deixa inutilizável.
    fn travar_estado(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- leitura e gravação ---------------------------------------------------------------------

    fn assinatura_do_arquivo(&self) -> Option<Assinatura> {
        let meta = fs::metadata(&self.arquivo).ok()?;
        Some((meta.modified().ok(), meta.len()))
    }

    fn dados_atuais<'a>(&self, est: &'a mut Estado) -> &'a Dados {
        let assinatura = self.assinatura_do_arquivo();
        if est.dados.is_none() || assinatura != est.assinatura {
            let novos = match self.ler(est.dados.as_ref(), false) {
                Ok(Some(dados)) => dados,
                Ok(None) => est.sem_arquivo(),
                Err(Ocupado) => est.dados.clone().unwrap_or_else(Dados::padrao),
            };
            est.dados = Some(novos);
            est.assinatura = self.assinatura_do_arquivo();
        }
        est.dados.get_or_insert_with(Dados::padrao)
    }

    /// Os bytes de `caminho`, pelo disco, com até 3 tentativas: antivírus ou outra instância com o arquivo aberto
    /// costumam soltá-lo em milissegundos. Arquivo que não existe volta na hora (`NotFound`).
    fn ler_bytes(&self, caminho: &Path) -> io::Result<Vec<u8>> {
        let mut tentativa = 0;
        loop {
            match self.disco.ler(caminho) {
                Ok(bytes) => return Ok(bytes),
                Err(e) if e.kind() == io::ErrorKind::NotFound || tentativa == 2 => return Err(e),
                Err(_) => {
                    tentativa += 1;
                    thread::sleep(ESPERA_LEITURA);
                }
            }
        }
    }

    /// Lê e saneia o arquivo; `Ok(None)`: ele não existe (o que vale fica com quem chama, por
    /// [`Estado::sem_arquivo`]). Sem conseguir ler: `estrito` devolve [`Ocupado`]; senão, fica o que havia.
    fn ler(&self, atual: Option<&Dados>, estrito: bool) -> Result<Option<Dados>, Ocupado> {
        let conteudo = match self.ler_bytes(&self.arquivo) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                log::warn!("Não foi possível ler as preferências em {}: {}", self.arquivo.display(), e);
                if estrito {
                    return Err(Ocupado);
                }
                return Ok(Some(atual.cloned().unwrap_or_else(Dados::padrao)));
            }
        };
        match json::ler(&conteudo).and_then(|bruto| sanear(&bruto, self.max_bancos_no_historico)) {
            Ok(dados) => Ok(Some(dados)),
            Err(motivo) => {
                self.separar_corrompido(&motivo);
                Ok(Some(Dados::padrao()))
            }
        }
    }

    fn separar_corrompido(&self, motivo: &str) {
        let destino = self.com_sufixo(".corrompido");
        match self.disco.trocar(&self.arquivo, &destino) {
            Ok(()) => log::warn!(
                "Preferências ilegíveis ({}); o arquivo foi guardado como {} e tudo recomeça do padrão.",
                motivo,
                destino.display()
            ),
            Err(e) => log::warn!("Preferências ilegíveis ({}) e não foi possível renomeá-las: {}", motivo, e),
        }
    }

    /// Põe `dados` em memória e grava; devolve se o disco aceitou.
    fn gravar(&self, est: &mut Estado, dados: Dados) -> bool {
        // Mesmo que o disco recuse, a mudança fica valendo em memória até o aplicativo fechar.
        let texto = dados.serializar();
        est.dados = Some(dados);
        match self.gravar_no_disco(&texto) {
            Ok(()) => {
                est.assinatura = self.assinatura_do_arquivo();
                true
            }
            Err(e) => {
                log::warn!("Não foi possível gravar as preferências em {}: {}", self.arquivo.display(), e);
                false
            }
        }
    }

    fn criar_temporario(&self, pasta: &Path) -> io::Result<(PathBuf, File)> {
        let nome = self.arquivo.file_name().map_or_else(|| "preferencias.json".into(), |n| n.to_string_lossy());
        for _ in 0..100 {
            let caminho = pasta.join(format!("{nome}.{}.tmp", sufixo_aleatorio()));
            match OpenOptions::new().write(true).create_new(true).open(&caminho) {
                Ok(arquivo) => return Ok((caminho, arquivo)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "não foi possível criar um arquivo temporário"))
    }

    /// Grava num temporário na mesma pasta e troca; o temporário some se algo falhar.
    fn gravar_no_disco(&self, texto: &str) -> io::Result<()> {
        let pasta = self.arquivo.parent().unwrap_or(Path::new(""));
        fs::create_dir_all(pasta)?;
        let (temporario, mut arquivo) = self.criar_temporario(pasta)?;
        let escrito = self.disco.escrever(&mut arquivo, texto.as_bytes()).and_then(|()| arquivo.sync_all());
        drop(arquivo);
        let resultado = escrito.and_then(|()| self.trocar(&temporario));
        if resultado.is_err() {
            let _ = fs::remove_file(&temporario);
        }
        resultado
    }

    fn trocar(&self, temporario: &Path) -> io::Result<()> {
        let mut tentativa = 0;
        loop {
            match self.disco.trocar(temporario, &self.arquivo) {
                Ok(()) => return Ok(()),
                // No Windows a troca falha enquanto outro processo lê o destino; costuma passar em milissegundos.
                Err(e) if tentativa < 4 && e_permissao(&e) => {
                    tentativa += 1;
                    thread::sleep(ESPERA_TROCA);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Aplica `mudanca` aos dados relidos do arquivo e grava; `mudanca` devolve false quando nada mudou.
    fn alterar(&self, mudanca: impl FnOnce(&mut Dados) -> bool) {
        // a vez da outra instância, que bateu enquanto esta gravava a mudança anterior (fora da trava interna: as
        // leituras desta instância não esperam)
        let vez = self.vez.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(vez) = vez {
            thread::sleep(vez.saturating_duration_since(Instant::now()));
        }
        let mut est = self.travar_estado();
        let trava = TravaEntreProcessos::adquirir(&self.arquivo_trava(), TEMPO_MAXIMO_TRAVA);
        self.alterar_sob_a_trava(&mut est, mudanca);
        if trava.soltar() {
            *self.vez.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now() + trava::CORTESIA);
        }
    }

    /// O corpo de [`Preferencias::alterar`], com a trava entre processos já tomada (ou o prazo vencido).
    fn alterar_sob_a_trava(&self, est: &mut Estado, mudanca: impl FnOnce(&mut Dados) -> bool) {
        // sob a trava, sempre relê: a outra instância pode ter gravado há um instante
        match self.ler(est.dados.as_ref(), true) {
            Err(Ocupado) => {
                // gravar agora poria por cima o que a outra instância gravou; a mudança fica só em memória
                let mut dados = est.dados.clone().unwrap_or_else(Dados::padrao);
                if mudanca(&mut dados) {
                    est.dados = Some(dados);
                }
            }
            Ok(lidos) => {
                let atuais = lidos.unwrap_or_else(|| est.sem_arquivo());
                est.assinatura = self.assinatura_do_arquivo();
                let mut dados = atuais.clone();
                est.dados = Some(atuais);
                if mudanca(&mut dados) {
                    self.gravar(est, dados);
                }
            }
        }
    }

    // ---- campos simples -------------------------------------------------------------------------

    pub fn tema(&self) -> Tema {
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est).tema
    }

    /// Uma cópia das opções guardadas.
    pub fn exportacao(&self) -> OpcoesExportacao {
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est).exportacao.opcoes
    }

    pub fn ultimo_diretorio(&self) -> Option<String> {
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est).ultimo_diretorio.clone()
    }

    /// A aparência do código guardada; o padrão (DataBox, Consolas, 12,5 px) enquanto a pessoa não escolher.
    pub fn aparencia(&self) -> Aparencia {
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est).aparencia.as_ref().map(|a| a.aparencia.clone()).unwrap_or_default()
    }

    /// As larguras dos painéis laterais guardadas; sem nenhuma escolhida, tudo None (a casca usa as padrão).
    pub fn paineis(&self) -> LarguraPaineis {
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est).paineis.as_ref().map(|p| p.larguras).unwrap_or_default()
    }

    /// A pasta que estava aberta (o caminho como foi gravado); None se nenhuma. Não confere o disco: a pasta pode
    /// ter sumido (disco de rede fora), e quem a reabre é quem mostra o erro.
    pub fn pasta_aberta(&self) -> Option<String> {
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est).pasta_aberta.clone()
    }

    /// Muda tema, exportação, última pasta, aparência, larguras dos painéis e/ou pasta aberta (None em cada campo
    /// deixa como está).
    ///
    /// `exportacao`, `aparencia` e `paineis` são mescladas às guardadas. Separador fora de [`SEPARADORES_CSV`] /
    /// [`SEPARADORES_DECIMAIS`], pasta vazia, tamanho do código fora de [`TAMANHO_CODIGO_MINIMO`] a
    /// [`TAMANHO_CODIGO_MAXIMO`] ou largura de painel fora de [`LARGURA_PAINEL_MINIMA`] a
    /// [`LARGURA_PAINEL_MAXIMA`] → ErroBanco 400 com a frase para a tela, sem gravar nada. Um arquivo em
    /// `ultimo_diretorio` vira a pasta dele; uma família só de espaços volta à padrão; a largura vai para o ponto
    /// inteiro mais perto. Grava mesmo que nada tenha mudado. A chave `"aparencia"` só entra no arquivo na
    /// primeira mudança com `aparencia` (mesmo que ela repita o padrão), e a `"larguras_paineis"` na primeira com `paineis`:
    /// antes disso, o arquivo fica sem elas. A `"pasta_aberta"` só existe enquanto há pasta
    /// aberta: `pasta_aberta: Some(Some(p))` a grava (o caminho absoluto, sem conferir o disco) e `Some(None)` a
    /// apaga; caminho vazio → ErroBanco 400 ("A pasta aberta precisa ser um caminho."), sem gravar nada.
    pub fn atualizar(&self, mudanca: Mudanca) -> Resultado<()> {
        let Mudanca { tema, exportacao, ultimo_diretorio, aparencia, paineis, pasta_aberta } = mudanca;
        if let Some(t) = aparencia.as_ref().and_then(|m| m.tamanho_codigo)
            && !tamanho_codigo_valido(f64::from(t))
        {
            return Err(ErroBanco::novo("O tamanho da fonte do código precisa ficar entre 8 e 24 px.")
                .detalhe(format!("tamanho_codigo={t}")));
        }
        if let Some(m) = &paineis {
            for (nome, largura) in [("explorador", m.explorador), ("propriedades", m.propriedades)] {
                if let Some(l) = largura
                    && !largura_painel_valida(f64::from(l))
                {
                    return Err(ErroBanco::novo("A largura do painel precisa ficar entre 120 e 2000 pt.")
                        .detalhe(format!("paineis.{nome}={l}")));
                }
            }
        }
        if let Some(m) = &exportacao {
            if let Some(c) = m.csv_separador
                && !SEPARADORES_CSV.contains(&c)
            {
                return Err(ErroBanco::novo("O separador do CSV precisa ser ponto e vírgula, vírgula ou tabulação.")
                    .detalhe(format!("csv_separador={}", caminhos::citar(&c.to_string()))));
            }
            if let Some(c) = m.csv_decimal
                && !SEPARADORES_DECIMAIS.contains(&c)
            {
                return Err(ErroBanco::novo("O separador decimal precisa ser vírgula ou ponto.")
                    .detalhe(format!("csv_decimal={}", caminhos::citar(&c.to_string()))));
            }
        }
        let diretorio = match ultimo_diretorio {
            None => None,
            Some(None) => Some(None),
            Some(Some(pasta)) => {
                let texto = pasta.to_string_lossy();
                if caminhos::tirar_espacos(&texto).is_empty() {
                    return Err(ErroBanco::novo("A última pasta precisa ser um caminho."));
                }
                let mut exibido = caminhos::abspath(&texto);
                if fs::metadata(&exibido).is_ok_and(|m| m.is_file()) {
                    exibido = caminhos::pasta_de(&exibido);
                }
                Some(Some(exibido))
            }
        };
        let pasta_aberta = match pasta_aberta {
            None => None,
            Some(None) => Some(None),
            Some(Some(pasta)) => {
                let texto = pasta.to_string_lossy();
                if caminhos::tirar_espacos(&texto).is_empty() {
                    return Err(ErroBanco::novo("A pasta aberta precisa ser um caminho."));
                }
                Some(Some(caminhos::abspath(&texto)))
            }
        };
        self.alterar(|dados| {
            if let Some(t) = tema {
                dados.tema = t;
            }
            if let Some(m) = &exportacao {
                dados.exportacao.aplicar(m);
            }
            if let Some(d) = diretorio {
                dados.ultimo_diretorio = d;
            }
            if let Some(m) = &aparencia {
                dados
                    .aparencia
                    .get_or_insert_with(|| AparenciaGuardada { aparencia: Aparencia::default(), extras: Extras::new() })
                    .aplicar(m);
            }
            if let Some(m) = &paineis {
                dados
                    .paineis
                    .get_or_insert_with(|| PaineisGuardados {
                        larguras: LarguraPaineis::default(),
                        extras: Extras::new(),
                    })
                    .aplicar(m);
            }
            if let Some(p) = pasta_aberta {
                dados.pasta_aberta = p;
            }
            true
        });
        Ok(())
    }

    // ---- recentes -------------------------------------------------------------------------------

    /// Bancos, pastas e arquivos de dados abertos recentemente, o mais recente primeiro, com o estado do caminho
    /// consultado agora (o `tipo` de cada um: ver [`TipoRecente`]; sem `"tipo"` no arquivo é banco, a não ser que o
    /// caminho seja uma pasta no disco). Os recentes de `"tipo"` desconhecido (gravados por uma versão mais nova) não
    /// vêm, mas continuam no arquivo e contam nos 15.
    pub fn recentes(&self) -> Vec<Recente> {
        let itens: Vec<(RecenteGuardado, Option<TipoRecente>)> = {
            let mut est = self.travar_estado();
            self.dados_atuais(&mut est)
                .recentes
                .iter()
                .filter_map(|r| match r.tipo_lido() {
                    TipoLido::Ausente => Some((r.clone(), None)),
                    TipoLido::Conhecido(tipo) => Some((r.clone(), Some(tipo))),
                    TipoLido::Estranho => None,
                })
                .collect()
        };
        let caminhos: Vec<String> = itens.iter().map(|(r, _)| r.caminho.clone()).collect();
        let estados = consultar_arquivos(&caminhos, self.tempo_maximo_consulta, &self.consulta);
        itens
            .into_iter()
            .zip(estados)
            .map(|((item, declarado), estado)| {
                // sem "tipo": a pasta no disco vale como pasta; o que não respondeu a tempo, como banco (o de sempre)
                let tipo = declarado.unwrap_or_else(|| {
                    if estado.as_ref().is_some_and(|e| e.e_pasta) { TipoRecente::Pasta } else { TipoRecente::Banco }
                });
                // existe: a pasta é pasta; o banco e o arquivo, arquivo comum (uma pasta com nome de banco não conta)
                let estado = estado.filter(|e| e.e_pasta == (tipo == TipoRecente::Pasta));
                Recente {
                    nome: caminhos::nome_base(&item.caminho).to_string(),
                    pasta: caminhos::pasta_de(&item.caminho),
                    tipo,
                    existe: estado.is_some(),
                    tamanho: estado.as_ref().filter(|e| !e.e_pasta).map(|e| e.tamanho),
                    modificado_em: estado.and_then(|e| e.modificado_em),
                    aberto_em: item.aberto_em,
                    caminho: item.caminho,
                }
            })
            .collect()
    }

    /// Põe o banco no topo dos recentes (se já estava, sobe), com a hora de agora: [`Preferencias::adicionar_recente_de`]
    /// com [`TipoRecente::Banco`].
    pub fn adicionar_recente(&self, caminho: impl AsRef<Path>) {
        self.adicionar_recente_de(caminho, TipoRecente::Banco);
    }

    /// Põe o caminho no topo dos recentes (se já estava, sobe), com a hora de agora e o `tipo` dado. O banco não
    /// grava a chave `"tipo"`; a pasta e o arquivo gravam
    /// `"tipo"` logo depois de `"aberto_em"`. Se já estava, leva as chaves que esta versão não conhece, menos um
    /// `"tipo"` que contradiz o novo (um `"tipo": "banco"` que já estava fica).
    pub fn adicionar_recente_de(&self, caminho: impl AsRef<Path>, tipo: TipoRecente) {
        let exibido = caminho_exibido(caminho);
        let chave = caminhos::normcase(&exibido);
        self.alterar(|dados| {
            let (mesmos, outros): (Vec<_>, Vec<_>) =
                std::mem::take(&mut dados.recentes).into_iter().partition(|r| chave_caminho(&r.caminho) == chave);
            let mut extras = mesmos.into_iter().next().map(|r| r.extras).unwrap_or_default();
            // o que o item já dizia de si e continua valendo (o mesmo tipo) fica onde estava
            let ja_dizia = |valor: &Json| valor.como_texto().and_then(TipoRecente::reconhecer) == Some(tipo);
            let tinha_o_mesmo = extras.iter().any(|(nome, valor)| nome == CHAVE_TIPO && ja_dizia(valor));
            extras.retain(|(nome, valor)| nome != CHAVE_TIPO || ja_dizia(valor));
            if tipo != TipoRecente::Banco && !tinha_o_mesmo {
                extras.insert(0, (CHAVE_TIPO.to_string(), Json::Texto(tipo.como_str().to_string())));
            }
            dados.recentes = outros;
            dados.recentes.insert(0, RecenteGuardado { caminho: exibido, aberto_em: Some(agora()), extras });
            dados.recentes.truncate(MAX_RECENTES);
            true
        });
    }

    /// Tira o banco dos recentes (comparação normalizada); não estar lá não é erro.
    pub fn remover_recente(&self, caminho: impl AsRef<Path>) {
        let chave = chave_caminho(caminho);
        self.alterar(|dados| {
            let antes = dados.recentes.len();
            dados.recentes.retain(|r| chave_caminho(&r.caminho) != chave);
            dados.recentes.len() != antes
        });
    }

    /// Esvazia a lista de recentes (o "Limpar a lista" das Preferências): tira os três tipos conhecidos, bancos,
    /// pastas e arquivos. Os recentes de tipo desconhecido (de uma versão mais nova) ficam no arquivo, como em
    /// [`Preferencias::recentes`]; sem nenhum recente conhecido na lista, nada é gravado.
    pub fn limpar_recentes(&self) {
        self.alterar(|dados| {
            let antes = dados.recentes.len();
            dados.recentes.retain(|r| !r.tipo_conhecido());
            dados.recentes.len() != antes
        });
    }

    // ---- histórico da Consulta SQL --------------------------------------------------------------

    /// Esvazia o histórico de todos os bancos (o "Limpar o histórico" das Preferências); sem nenhum comando
    /// guardado, nada é gravado.
    pub fn limpar_historico_sql(&self) {
        self.alterar(|dados| {
            let havia = !dados.historico_sql.is_empty();
            dados.historico_sql.clear();
            havia
        });
    }

    /// Comandos já executados nesse banco, o mais recente primeiro.
    pub fn historico_sql(&self, caminho_banco: impl AsRef<Path>) -> Vec<ComandoSql> {
        let chave = chave_caminho(caminho_banco);
        let mut est = self.travar_estado();
        self.dados_atuais(&mut est)
            .historico_sql
            .iter()
            .find(|(k, _)| *k == chave)
            .map(|(_, itens)| itens.clone())
            .unwrap_or_default()
    }

    /// Guarda o comando no topo do histórico do banco, com o que a execução deu (inclusive o erro: a consulta que
    /// falhou é a que a pessoa mais quer recuperar) e quantos comandos rodaram; um comando repetido só sobe para o
    /// topo, com o resultado novo.
    ///
    /// Comandos vazios não entram, nem os gigantes (um script colado de megabytes incharia o arquivo).
    pub fn registrar_sql(
        &self,
        caminho_banco: impl AsRef<Path>,
        sql: &str,
        resultado: Option<ResultadoSql>,
        comandos: Option<u32>,
    ) {
        let texto = caminhos::tirar_espacos(sql);
        if texto.is_empty() || texto.chars().count() > MAX_CARACTERES_COMANDO {
            return;
        }
        let chave = chave_caminho(caminho_banco);
        let max_bancos = self.max_bancos_no_historico;
        self.alterar(|dados| {
            let historico = &mut dados.historico_sql;
            let mut itens = match historico.iter().position(|(k, _)| *k == chave) {
                Some(i) => historico.remove(i).1,
                None => Vec::new(),
            };
            itens.retain(|i| i.sql != texto);
            let item = ComandoSql { sql: texto.to_string(), quando: Some(agora()), resultado, comandos };
            itens.insert(0, item);
            itens.truncate(MAX_COMANDOS);
            // Reinserido no fim: a ordem é a do uso, e o banco mais antigo sai primeiro.
            historico.push((chave, itens));
            if historico.len() > max_bancos {
                let sobra = historico.len() - max_bancos;
                historico.drain(..sobra);
            }
            true
        });
    }

    // ---- para a interface -----------------------------------------------------------------------

    /// {tema, recentes, exportacao}, mais a
    /// aparência do código.
    pub fn resumo(&self) -> PrefsInfo {
        PrefsInfo {
            tema: self.tema(),
            recentes: self.recentes(),
            exportacao: self.exportacao(),
            aparencia: self.aparencia(),
        }
    }
}
