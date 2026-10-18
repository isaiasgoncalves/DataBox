//! O que um caminho é para o DataBox: pasta, banco SQLite ou arquivo de dados (CSV, Excel, JSON, texto).
//!
//! Tudo aqui é **puro**: não abre arquivo nem lê pasta. Quem varre o disco é o `servico` (`servico::pasta`),
//! que usa estas peças para decidir, na ordem da decisão 1 da etapa 4:
//!
//! 1. [`palpite`]: pelo nome e pelo tamanho, sem tocar no disco. Extensão de dados conhecida vale sem ler;
//!    arquivo de 0 byte com extensão de banco é um banco vazio (o "Novo banco" cria assim); o resto, com 100 bytes
//!    ou mais, pede a leitura do cabeçalho.
//! 2. [`e_cabecalho_sqlite`]: os 16 primeiros bytes são `SQLite format 3\0`? A extensão mente (`.dat`, `.bak`,
//!    sem extensão) e o `Thumbs.db` do Windows não é SQLite.
//!
//! O resto é o que a árvore da pasta precisa: [`Natureza`], [`Entrada`] e [`Listagem`], a ordem das entradas
//! ([`comparar_nomes`], pastas, bancos e dados, cada grupo em ordem natural sem caixa) e a regra dos ocultos
//! ([`e_oculto`]: atributo oculto ou de sistema do Windows, os `~$` do Office e as pastas de ferramentas; um
//! ponto no começo do nome **não** esconde).

use std::cmp::Ordering;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// As extensões de banco (sem o ponto, em minúsculas).
pub const EXTENSOES_BANCO: [&str; 6] = ["db", "sqlite", "sqlite3", "db3", "s3db", "sl3"];
/// Os 16 primeiros bytes de todo arquivo SQLite.
pub const CABECALHO_SQLITE: [u8; 16] = *b"SQLite format 3\0";
/// Abaixo disto não há cabeçalho para ler (o cabeçalho do SQLite tem 100 bytes).
pub const TAMANHO_DO_CABECALHO: u64 = 100;
/// Entradas que a árvore mostra por pasta; o resto vira "mais N itens".
pub const LIMITE_POR_PASTA: usize = 500;
/// Entradas lidas de uma pasta antes de parar.
pub const LIMITE_DE_LEITURA: usize = 20_000;
/// Arquivos cujo cabeçalho se lê numa varredura.
pub const LIMITE_DE_CABECALHOS: usize = 2_000;
/// O tempo máximo de uma varredura de pasta.
pub const PRAZO_DA_VARREDURA: Duration = Duration::from_secs(5);

/// Pastas de ferramentas que a árvore não mostra (comparadas sem caixa, como o Windows).
pub const PASTAS_DE_FERRAMENTAS: [&str; 8] =
    [".git", ".svn", ".hg", ".vscode", ".idea", "__pycache__", "node_modules", ".venv"];
/// O começo do nome dos arquivos de bloqueio que o Office deixa ao lado de um arquivo aberto.
pub const PREFIXO_DE_BLOQUEIO: &str = "~$";
/// `FILE_ATTRIBUTE_HIDDEN` do Windows.
pub const ATRIBUTO_OCULTO: u32 = 0x2;
/// `FILE_ATTRIBUTE_SYSTEM` do Windows.
pub const ATRIBUTO_DE_SISTEMA: u32 = 0x4;

/// Os tipos de arquivo de dados que o app entende (além dos bancos).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TipoDeArquivo {
    Csv,
    Xlsx,
    Json,
    Texto,
}

impl TipoDeArquivo {
    pub const TODOS: [TipoDeArquivo; 4] =
        [TipoDeArquivo::Csv, TipoDeArquivo::Xlsx, TipoDeArquivo::Json, TipoDeArquivo::Texto];

    /// O tipo de uma extensão, sem o ponto e sem diferenciar caixa: `csv` `tsv` | `xlsx` | `json` | `txt` `md`
    /// `log`; o `.xlsm` (Excel com macros, o mesmo pacote) é Excel também. O `.xls`, o `.xlsb` e o `.sql` ficam de fora.
    pub fn da_extensao(ext: &str) -> Option<TipoDeArquivo> {
        let ext = ext.to_ascii_lowercase();
        TipoDeArquivo::TODOS.into_iter().find(|t| t.extensoes().contains(&ext.as_str()))
    }

    /// As extensões do tipo (sem o ponto, em minúsculas): o que alimenta o filtro do diálogo "Abrir arquivo".
    pub fn extensoes(self) -> &'static [&'static str] {
        match self {
            TipoDeArquivo::Csv => &["csv", "tsv"],
            TipoDeArquivo::Xlsx => &["xlsx", "xlsm"],
            TipoDeArquivo::Json => &["json"],
            TipoDeArquivo::Texto => &["txt", "md", "log"],
        }
    }

    /// A etiqueta do ícone: "CSV", "XLSX", "JSON", "TXT".
    pub fn rotulo(self) -> &'static str {
        match self {
            TipoDeArquivo::Csv => "CSV",
            TipoDeArquivo::Xlsx => "XLSX",
            TipoDeArquivo::Json => "JSON",
            TipoDeArquivo::Texto => "TXT",
        }
    }

    /// O nome para as frases: "CSV", "Excel", "JSON", "texto".
    pub fn nome(self) -> &'static str {
        match self {
            TipoDeArquivo::Csv => "CSV",
            TipoDeArquivo::Xlsx => "Excel",
            TipoDeArquivo::Json => "JSON",
            TipoDeArquivo::Texto => "texto",
        }
    }
}

/// O que um caminho é.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Natureza {
    Pasta,
    Banco,
    Dados(TipoDeArquivo),
}

impl Natureza {
    pub fn e_pasta(self) -> bool {
        self == Natureza::Pasta
    }

    /// A posição do grupo na árvore: pastas (0), bancos (1), arquivos de dados (2).
    pub fn grupo(self) -> u8 {
        match self {
            Natureza::Pasta => 0,
            Natureza::Banco => 1,
            Natureza::Dados(_) => 2,
        }
    }
}

/// O primeiro palpite pelo nome e pelo tamanho, antes de ler o disco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palpite {
    /// Extensão de dados conhecida: vale sem ler nada.
    Dados(TipoDeArquivo),
    /// 0 byte com extensão de banco: um banco que ainda não foi gravado.
    BancoVazio,
    /// Pode ser um banco: falta ler os 16 primeiros bytes ([`e_cabecalho_sqlite`]).
    LerCabecalho,
    /// Não é nada que o app abra (pequeno demais para ter o cabeçalho do SQLite).
    Nenhuma,
}

/// A regra da decisão 1: extensão de dados conhecida vale sem ler; arquivo de 0 byte com extensão de banco é
/// banco vazio; o resto, com [`TAMANHO_DO_CABECALHO`] bytes ou mais, tem o cabeçalho lido.
pub fn palpite(nome: &str, tamanho: u64) -> Palpite {
    let ext = extensao_minuscula(nome);
    if let Some(tipo) = ext.as_deref().and_then(TipoDeArquivo::da_extensao) {
        return Palpite::Dados(tipo);
    }
    if tamanho == 0 {
        return if ext.as_deref().is_some_and(e_extensao_de_banco) { Palpite::BancoVazio } else { Palpite::Nenhuma };
    }
    if tamanho >= TAMANHO_DO_CABECALHO { Palpite::LerCabecalho } else { Palpite::Nenhuma }
}

/// A extensão do nome em minúsculas, sem o ponto (`.gitignore` não tem extensão, como no `Path`).
fn extensao_minuscula(nome: &str) -> Option<String> {
    Path::new(nome).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

/// A extensão (sem o ponto, sem caixa) é de banco?
pub fn e_extensao_de_banco(ext: &str) -> bool {
    EXTENSOES_BANCO.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// O começo do arquivo (pelo menos 16 bytes) é o cabeçalho do SQLite? Com menos de 16 bytes, não.
pub fn e_cabecalho_sqlite(inicio: &[u8]) -> bool {
    inicio.len() >= CABECALHO_SQLITE.len() && inicio[..CABECALHO_SQLITE.len()] == CABECALHO_SQLITE
}

// ---- A ordem dos nomes ----

/// A letra sem caixa e sem acento (o que o português usa), para comparar como o Explorador do Windows.
fn dobrar(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase().map(|m| match m {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        outra => outra,
    })
}

/// Compara duas sequências de dígitos como números (sem os zeros à esquerda, sem estourar em número grande).
fn comparar_numeros(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// A ordem natural sem caixa e sem acento: `dados2` antes de `dados10`, `Dados` junto de `dados`, `é` junto de
/// `e`. Só devolve `Equal` para textos idênticos (o desempate é o texto como veio), então a ordem é total.
pub fn comparar_nomes(a: &str, b: &str) -> Ordering {
    let mut ia = a.chars().flat_map(dobrar).peekable();
    let mut ib = b.chars().flat_map(dobrar).peekable();
    loop {
        match (ia.peek().copied(), ib.peek().copied()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(d) = ia.next_if(char::is_ascii_digit) {
                    na.push(d);
                }
                let mut nb = String::new();
                while let Some(d) = ib.next_if(char::is_ascii_digit) {
                    nb.push(d);
                }
                match comparar_numeros(&na, &nb) {
                    Ordering::Equal => {}
                    outra => return outra,
                }
            }
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => {
                    ia.next();
                    ib.next();
                }
                outra => return outra,
            },
        }
    }
    a.cmp(b)
}

// ---- Os ocultos ----

/// É uma das pastas de ferramentas (`.git`, `node_modules`…)? Sem diferenciar caixa.
pub fn e_pasta_de_ferramenta(nome: &str) -> bool {
    PASTAS_DE_FERRAMENTAS.iter().any(|p| p.eq_ignore_ascii_case(nome))
}

/// É o arquivo de bloqueio do Office (`~$planilha.xlsx`)?
pub fn e_arquivo_de_bloqueio(nome: &str) -> bool {
    nome.starts_with(PREFIXO_DE_BLOQUEIO)
}

/// Os atributos do Windows pedem para esconder (oculto ou de sistema)?
pub fn atributos_escondem(atributos: u32) -> bool {
    atributos & (ATRIBUTO_OCULTO | ATRIBUTO_DE_SISTEMA) != 0
}

/// Os atributos do Windows de um arquivo ou pasta (`file_attributes`, sem `unsafe`). Fora do Windows não há
/// atributo e o resultado é 0.
#[cfg(windows)]
pub fn atributos_de(meta: &Metadata) -> u32 {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes()
}

/// Os atributos do Windows de um arquivo ou pasta. Fora do Windows não há atributo e o resultado é 0.
#[cfg(not(windows))]
pub fn atributos_de(_meta: &Metadata) -> u32 {
    0
}

/// A regra dos ocultos: atributo oculto ou de sistema do Windows, `~$` de bloqueio do Office e, só
/// para pastas, as de ferramentas. Um nome com ponto no começo **não** esconde por si só: no Windows isso não é
/// "oculto" (há bancos guardados em `C:\Users\…\.coletor_noticias`).
pub fn e_oculto(nome: &str, e_pasta: bool, atributos: u32) -> bool {
    atributos_escondem(atributos) || e_arquivo_de_bloqueio(nome) || (e_pasta && e_pasta_de_ferramenta(nome))
}

// ---- A listagem de uma pasta ----

/// Uma linha da árvore: uma pasta, um banco ou um arquivo de dados dentro da pasta lida.
#[derive(Clone, Debug, PartialEq)]
pub struct Entrada {
    pub nome: String,
    pub caminho: PathBuf,
    /// A chave do caminho (`prefs::chave_caminho`), calculada uma vez por quem lê a pasta.
    pub chave: String,
    pub natureza: Natureza,
    /// Só de arquivo (`None` na pasta).
    pub tamanho: Option<u64>,
    /// Só de arquivo: a data de uma subpasta, vista da pasta de cima, muda no NTFS quando alguém a abre, e a
    /// releitura acharia mudança onde não há.
    pub modificado: Option<SystemTime>,
}

/// Pastas primeiro, depois bancos, depois arquivos de dados; cada grupo em ordem natural sem caixa.
pub fn comparar_entradas(a: &Entrada, b: &Entrada) -> Ordering {
    a.natureza.grupo().cmp(&b.natureza.grupo()).then_with(|| comparar_nomes(&a.nome, &b.nome))
}

/// O resultado de ler uma pasta.
#[derive(Clone, Debug, PartialEq)]
pub struct Listagem {
    pub pasta: PathBuf,
    /// Ordenadas (pastas, bancos, dados) e cortadas em [`LIMITE_POR_PASTA`].
    pub entradas: Vec<Entrada>,
    /// Arquivos de outros tipos (e ocultos) que não aparecem.
    pub ocultas: usize,
    /// Entradas que o limite cortou ("mais 37 itens").
    pub a_mais: usize,
    /// A leitura parou por prazo ou limite: pode faltar coisa.
    pub parcial: bool,
}

impl Listagem {
    /// Uma listagem sem nada (pasta vazia).
    pub fn vazia(pasta: PathBuf) -> Listagem {
        Listagem { pasta, entradas: Vec::new(), ocultas: 0, a_mais: 0, parcial: false }
    }

    /// Ordena as entradas ([`comparar_entradas`]), corta em `limite` e conta o que sobrou em `a_mais`.
    /// A varredura passa [`LIMITE_POR_PASTA`]; o parâmetro existe para os testes poderem reduzi-lo.
    pub fn montar(
        pasta: PathBuf,
        mut entradas: Vec<Entrada>,
        ocultas: usize,
        parcial: bool,
        limite: usize,
    ) -> Listagem {
        entradas.sort_by(comparar_entradas);
        let a_mais = entradas.len().saturating_sub(limite);
        entradas.truncate(limite);
        Listagem { pasta, entradas, ocultas, a_mais, parcial }
    }

    /// A entrada com esta chave de caminho.
    pub fn entrada(&self, chave: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.chave == chave)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn entrada(nome: &str, natureza: Natureza) -> Entrada {
        Entrada {
            nome: nome.to_string(),
            caminho: PathBuf::from("pasta").join(nome),
            chave: nome.to_lowercase(),
            natureza,
            tamanho: None,
            modificado: None,
        }
    }

    fn nomes(l: &Listagem) -> Vec<&str> {
        l.entradas.iter().map(|e| e.nome.as_str()).collect()
    }

    #[test]
    fn extensoes_de_dados_sem_caixa() {
        for (ext, tipo) in [
            ("csv", TipoDeArquivo::Csv),
            ("TSV", TipoDeArquivo::Csv),
            ("Xlsx", TipoDeArquivo::Xlsx),
            ("xlsm", TipoDeArquivo::Xlsx),
            ("XLSM", TipoDeArquivo::Xlsx),
            ("json", TipoDeArquivo::Json),
            ("txt", TipoDeArquivo::Texto),
            ("md", TipoDeArquivo::Texto),
            ("LOG", TipoDeArquivo::Texto),
        ] {
            assert_eq!(TipoDeArquivo::da_extensao(ext), Some(tipo), "{ext}");
        }
        for fora in ["xls", "xlsb", "sql", "pdf", "db", "", "csv.bak", ".csv"] {
            assert_eq!(TipoDeArquivo::da_extensao(fora), None, "{fora}");
        }
    }

    #[test]
    fn xlsm_e_um_arquivo_de_planilha() {
        assert_eq!(TipoDeArquivo::Xlsx.extensoes(), ["xlsx", "xlsm"]);
        assert_eq!(palpite("macros.xlsm", 5_000), Palpite::Dados(TipoDeArquivo::Xlsx));
        assert_eq!(palpite("MACROS.XLSM", 0), Palpite::Dados(TipoDeArquivo::Xlsx), "vale sem ler o disco");
        assert_eq!(palpite("antigo.xls", 5_000), Palpite::LerCabecalho, "o .xls continua fora");
        assert_eq!(palpite("binario.xlsb", 5_000), Palpite::LerCabecalho, "o .xlsb continua fora");
        // o arquivo de bloqueio do Office de um .xlsm continua oculto
        assert!(e_oculto("~$macros.xlsm", false, 0));
    }

    #[test]
    fn toda_extensao_listada_volta_ao_seu_tipo() {
        for tipo in TipoDeArquivo::TODOS {
            assert!(!tipo.extensoes().is_empty());
            for ext in tipo.extensoes() {
                assert_eq!(TipoDeArquivo::da_extensao(ext), Some(tipo), "{ext}");
            }
        }
    }

    #[test]
    fn rotulo_e_nome_de_cada_tipo() {
        let tabela = [
            (TipoDeArquivo::Csv, "CSV", "CSV"),
            (TipoDeArquivo::Xlsx, "XLSX", "Excel"),
            (TipoDeArquivo::Json, "JSON", "JSON"),
            (TipoDeArquivo::Texto, "TXT", "texto"),
        ];
        for (tipo, rotulo, nome) in tabela {
            assert_eq!(tipo.rotulo(), rotulo);
            assert_eq!(tipo.nome(), nome);
        }
        assert_eq!(TipoDeArquivo::TODOS.map(|t| t.rotulo()), ["CSV", "XLSX", "JSON", "TXT"]);
    }

    #[test]
    fn cabecalho_sqlite_so_com_os_16_bytes() {
        let mut inicio = CABECALHO_SQLITE.to_vec();
        assert!(e_cabecalho_sqlite(&inicio));
        inicio.extend_from_slice(&[0u8; 84]);
        assert!(e_cabecalho_sqlite(&inicio), "o que vem depois dos 16 bytes não importa");
        assert!(!e_cabecalho_sqlite(&CABECALHO_SQLITE[..15]), "15 bytes não bastam");
        assert!(!e_cabecalho_sqlite(b"SQLite format 3 "), "o último byte é o zero");
        assert!(!e_cabecalho_sqlite(b"sqlite format 3\0"), "a caixa conta");
        assert!(!e_cabecalho_sqlite(b""));
        assert!(!e_cabecalho_sqlite(b"PK\x03\x04 um xlsx"));
    }

    #[test]
    fn palpite_extensao_de_dados_nao_le() {
        assert_eq!(palpite("vendas.csv", 5_000), Palpite::Dados(TipoDeArquivo::Csv));
        assert_eq!(palpite("VENDAS.XLSX", 1), Palpite::Dados(TipoDeArquivo::Xlsx));
        assert_eq!(palpite("notas.md", 0), Palpite::Dados(TipoDeArquivo::Texto), "vazio também abre");
        assert_eq!(palpite("dados.json", 99), Palpite::Dados(TipoDeArquivo::Json));
    }

    #[test]
    fn palpite_zero_bytes_com_extensao_de_banco_e_banco_vazio() {
        for ext in EXTENSOES_BANCO {
            assert_eq!(palpite(&format!("novo.{ext}"), 0), Palpite::BancoVazio, "{ext}");
        }
        assert_eq!(palpite("NOVO.DB", 0), Palpite::BancoVazio);
        assert_eq!(palpite("novo.dat", 0), Palpite::Nenhuma, "0 byte sem extensão de banco não é banco");
        assert_eq!(palpite("novo", 0), Palpite::Nenhuma);
        assert_eq!(palpite("relatorio.pdf", 0), Palpite::Nenhuma);
    }

    #[test]
    fn palpite_demais_arquivos_com_cem_bytes_leem_o_cabecalho() {
        assert_eq!(palpite("estoque.db", 100), Palpite::LerCabecalho);
        assert_eq!(palpite("estoque.db", 8_192), Palpite::LerCabecalho);
        assert_eq!(palpite("copia.dat", 100), Palpite::LerCabecalho);
        assert_eq!(palpite("copia.bak", 4_096), Palpite::LerCabecalho);
        assert_eq!(palpite("sem_extensao", 100), Palpite::LerCabecalho);
        assert_eq!(palpite("Thumbs.db", 24_576), Palpite::LerCabecalho, "o cabeçalho decide");
        assert_eq!(palpite("estoque.db-wal", 32_992), Palpite::LerCabecalho, "o cabeçalho do wal é outro");
    }

    #[test]
    fn palpite_menos_de_cem_bytes_nao_e_banco() {
        for tamanho in [1, 16, 99] {
            assert_eq!(palpite("estoque.db", tamanho), Palpite::Nenhuma, "{tamanho}");
            assert_eq!(palpite("copia.dat", tamanho), Palpite::Nenhuma, "{tamanho}");
        }
    }

    #[test]
    fn nomes_em_ordem_natural_sem_caixa() {
        let ordenar = |mut v: Vec<&'static str>| {
            v.sort_by(|a, b| comparar_nomes(a, b));
            v
        };
        // Números por valor, não por texto.
        assert_eq!(ordenar(vec!["dados10", "dados2", "dados1"]), ["dados1", "dados2", "dados10"]);
        assert_eq!(comparar_nomes("dados2", "Dados10"), Ordering::Less);
        assert_eq!(comparar_nomes("DADOS2.db", "dados10.db"), Ordering::Less);
        // Zeros à esquerda não mudam o valor; número enorme não estoura.
        assert_eq!(comparar_nomes("a007", "a10"), Ordering::Less);
        assert_eq!(comparar_nomes("n99999999999999999999999", "n100000000000000000000000"), Ordering::Less);
        // A caixa não separa; o acento fica junto da letra.
        assert_eq!(ordenar(vec!["b", "A", "a", "B"]), ["A", "a", "B", "b"]);
        assert_eq!(ordenar(vec!["zebra", "árvore", "Banco", "ação"]), ["ação", "árvore", "Banco", "zebra"]);
        assert_eq!(comparar_nomes("é", "e1"), Ordering::Less);
        // Sem número: letra a letra, o mais curto primeiro.
        assert_eq!(comparar_nomes("dados", "dados2"), Ordering::Less);
        assert_eq!(comparar_nomes("estoque", "estoque_2026"), Ordering::Less);
        assert_eq!(comparar_nomes("", "a"), Ordering::Less);
        // Só é igual quem é idêntico: a ordem é total e estável.
        assert_eq!(comparar_nomes("dados", "dados"), Ordering::Equal);
        assert_ne!(comparar_nomes("Dados", "dados"), Ordering::Equal);
        assert_ne!(comparar_nomes("a01", "a1"), Ordering::Equal);
        assert_eq!(comparar_nomes("Dados", "dados"), comparar_nomes("dados", "Dados").reverse(), "antissimétrica");
    }

    #[test]
    fn ponto_no_comeco_nao_esconde() {
        assert!(!e_oculto(".coletor_noticias", true, 0));
        assert!(!e_oculto(".env", false, 0));
        assert!(!e_oculto(".gitignore", false, 0));
        assert!(!e_oculto(".github", true, 0), "só as pastas da lista");
    }

    #[test]
    fn oculta_arquivo_de_bloqueio_do_office() {
        assert!(e_oculto("~$planilha.xlsx", false, 0));
        assert!(e_oculto("~$", false, 0));
        assert!(!e_oculto("~planilha.xlsx", false, 0), "o til sozinho não basta");
        assert!(!e_oculto("planilha~$.xlsx", false, 0));
    }

    #[test]
    fn oculta_so_as_pastas_de_ferramentas_da_lista() {
        for nome in PASTAS_DE_FERRAMENTAS {
            assert!(e_oculto(nome, true, 0), "{nome}");
            assert!(!e_oculto(nome, false, 0), "{nome} arquivo comum não é pasta de ferramenta");
        }
        assert!(e_oculto(".GIT", true, 0), "o Windows não diferencia a caixa");
        assert!(e_oculto("Node_Modules", true, 0));
        assert!(!e_oculto("node_modules2", true, 0));
        assert!(!e_oculto("dados", true, 0));
        assert_eq!(PASTAS_DE_FERRAMENTAS.len(), 8);
    }

    #[test]
    fn oculta_pelo_atributo_oculto_ou_de_sistema() {
        const ARQUIVO: u32 = 0x20;
        const PASTA: u32 = 0x10;
        assert!(!e_oculto("dados.db", false, ARQUIVO));
        assert!(!e_oculto("dados", true, PASTA));
        assert!(e_oculto("dados.db", false, ARQUIVO | ATRIBUTO_OCULTO));
        assert!(e_oculto("dados.db", false, ARQUIVO | ATRIBUTO_DE_SISTEMA));
        assert!(e_oculto("System Volume Information", true, PASTA | ATRIBUTO_OCULTO | ATRIBUTO_DE_SISTEMA));
        assert!(!atributos_escondem(0));
    }

    #[cfg(windows)]
    #[test]
    fn atributos_de_le_o_atributo_oculto_do_windows() {
        let pasta = std::env::temp_dir().join(format!("databox-arquivos-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let comum = pasta.join("comum.txt");
        let escondido = pasta.join("escondido.txt");
        std::fs::write(&comum, b"a").unwrap();
        std::fs::write(&escondido, b"b").unwrap();
        let ok = std::process::Command::new("attrib").arg("+h").arg(&escondido).status().is_ok_and(|s| s.success());
        let a_comum = atributos_de(&std::fs::metadata(&comum).unwrap());
        let a_escondido = atributos_de(&std::fs::metadata(&escondido).unwrap());
        let a_pasta = atributos_de(&std::fs::metadata(&pasta).unwrap());
        let _ = std::fs::remove_dir_all(&pasta);
        assert!(ok, "o attrib deveria ter funcionado");
        assert!(!atributos_escondem(a_comum));
        assert!(!atributos_escondem(a_pasta));
        assert!(atributos_escondem(a_escondido));
        assert!(e_oculto("escondido.txt", false, a_escondido));
        assert!(!e_oculto("comum.txt", false, a_comum));
    }

    #[test]
    fn entradas_ordenam_pastas_bancos_e_dados() {
        use Natureza::*;
        let l = Listagem::montar(
            PathBuf::from("pasta"),
            vec![
                entrada("notas.txt", Dados(TipoDeArquivo::Texto)),
                entrada("dados10.db", Banco),
                entrada("Vendas", Pasta),
                entrada("dados2.db", Banco),
                entrada("a.csv", Dados(TipoDeArquivo::Csv)),
                entrada("arquivo", Pasta),
            ],
            0,
            false,
            LIMITE_POR_PASTA,
        );
        assert_eq!(nomes(&l), ["arquivo", "Vendas", "dados2.db", "dados10.db", "a.csv", "notas.txt"]);
        assert_eq!(l.a_mais, 0);
    }

    #[test]
    fn montar_corta_no_limite_e_conta_o_que_faltou() {
        let muitas: Vec<Entrada> = (0..7).map(|i| entrada(&format!("d{i}.db"), Natureza::Banco)).collect();
        let l = Listagem::montar(PathBuf::from("p"), muitas, 3, true, 4);
        assert_eq!(nomes(&l), ["d0.db", "d1.db", "d2.db", "d3.db"]);
        assert_eq!((l.a_mais, l.ocultas, l.parcial), (3, 3, true));
        let justa: Vec<Entrada> = (0..4).map(|i| entrada(&format!("d{i}.db"), Natureza::Banco)).collect();
        assert_eq!(Listagem::montar(PathBuf::from("p"), justa, 0, false, 4).a_mais, 0);
        assert_eq!(LIMITE_POR_PASTA, 500);
    }

    #[test]
    fn listagem_vazia_e_busca_pela_chave() {
        let v = Listagem::vazia(PathBuf::from("p"));
        assert!(v.entradas.is_empty());
        assert_eq!((v.ocultas, v.a_mais, v.parcial), (0, 0, false));
        let l = Listagem::montar(PathBuf::from("p"), vec![entrada("Dados.db", Natureza::Banco)], 0, false, 10);
        assert_eq!(l.entrada("dados.db").map(|e| e.nome.as_str()), Some("Dados.db"));
        assert!(l.entrada("outro.db").is_none());
    }

    #[test]
    fn natureza_e_grupo() {
        assert!(Natureza::Pasta.e_pasta());
        assert!(!Natureza::Banco.e_pasta());
        assert!(Natureza::Pasta.grupo() < Natureza::Banco.grupo());
        assert!(Natureza::Banco.grupo() < Natureza::Dados(TipoDeArquivo::Csv).grupo());
    }

    #[test]
    fn extensoes_de_banco_sem_caixa() {
        for ext in EXTENSOES_BANCO {
            assert!(e_extensao_de_banco(ext));
            assert!(e_extensao_de_banco(&ext.to_uppercase()));
        }
        assert!(!e_extensao_de_banco("csv"));
        assert!(!e_extensao_de_banco(""));
    }
}
