//! Os objetos que o banco devolve: structs com campos públicos (ver o topo do `lib.rs`).

use std::path::PathBuf;

use comum::{Afinidade, Celula, Chave, ErroBanco, Resultado};

use crate::busca::Filtro;

/// Tipo de um objeto no PRAGMA table_list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoObjeto {
    Table,
    View,
    Virtual,
    /// Tabela sombra de uma tabela virtual (as `notas_data` etc. do FTS5).
    Shadow,
}

impl TipoObjeto {
    /// "table" | "view" | "virtual" | "shadow".
    pub fn como_str(self) -> &'static str {
        match self {
            TipoObjeto::Table => "table",
            TipoObjeto::View => "view",
            TipoObjeto::Virtual => "virtual",
            TipoObjeto::Shadow => "shadow",
        }
    }

    pub(crate) fn de_texto(texto: &str) -> TipoObjeto {
        match texto {
            "view" => TipoObjeto::View,
            "virtual" => TipoObjeto::Virtual,
            "shadow" => TipoObjeto::Shadow,
            _ => TipoObjeto::Table,
        }
    }
}

/// Como as linhas de uma tabela são identificadas (`{"tipo": "rowid", "alias"}` | `{"tipo": "pk", "colunas"}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TipoChave {
    /// O primeiro de rowid, oid e _rowid_ que não é nome de coluna.
    Rowid { alias: String },
    /// WITHOUT ROWID (ou os três apelidos do rowid são colunas): as colunas da chave primária, em ordem.
    Pk { colunas: Vec<String> },
}

/// Direção da ordenação da grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direcao {
    #[default]
    Asc,
    Desc,
}

impl Direcao {
    /// "asc" | "desc".
    pub fn como_str(self) -> &'static str {
        match self {
            Direcao::Asc => "asc",
            Direcao::Desc => "desc",
        }
    }

    pub(crate) fn sql(self) -> &'static str {
        match self {
            Direcao::Asc => "ASC",
            Direcao::Desc => "DESC",
        }
    }

    /// "asc"/"desc" sem diferença de caixa ("" vale asc); outro → 400 "Direção de ordenação inválida: x. Use
    /// asc ou desc.".
    pub fn de_texto(texto: &str) -> Resultado<Direcao> {
        let baixo = if texto.is_empty() { "asc".to_string() } else { texto.to_lowercase() };
        match baixo.as_str() {
            "asc" => Ok(Direcao::Asc),
            "desc" => Ok(Direcao::Desc),
            _ => Err(ErroBanco::novo(format!("Direção de ordenação inválida: {baixo}. Use asc ou desc."))),
        }
    }
}

/// `BancoInfo.contagem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContagemObjetos {
    pub tabelas: u64,
    pub visoes: u64,
    pub indices: u64,
    pub gatilhos: u64,
}

/// BancoInfo (§4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct BancoInfo {
    /// Identidade desta abertura (16 caracteres hexadecimais): um pedido feito para outro banco é recusado.
    pub id: String,
    pub caminho: PathBuf,
    pub nome: String,
    pub pasta: PathBuf,
    pub tamanho: u64,
    /// Data local da última modificação do arquivo, "2026-09-22T10:31:00".
    pub modificado_em: Option<String>,
    pub somente_leitura: bool,
    pub motivo_somente_leitura: Option<String>,
    /// BEGIN aberto na Consulta SQL: nada desde então está no arquivo (a casca avisa).
    pub transacao_aberta: bool,
    pub sqlite_versao: String,
    pub codificacao: String,
    pub tamanho_pagina: i64,
    pub paginas: i64,
    pub paginas_livres: i64,
    pub modo_diario: String,
    pub versao_usuario: i64,
    pub chaves_estrangeiras: bool,
    pub contagem: ContagemObjetos,
}

/// Uma tabela do usuário em ObjetosInfo. `linhas` é None quando o arquivo passa de 256 MB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabelaResumo {
    pub nome: String,
    /// Table ou Virtual.
    pub tipo: TipoObjeto,
    pub sem_rowid: bool,
    pub linhas: Option<u64>,
    /// Colunas visíveis (None se o SQLite não conseguiu ler).
    pub colunas: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisaoResumo {
    pub nome: String,
    /// None numa visão quebrada (aponta para uma tabela que não existe mais).
    pub colunas: Option<u64>,
}

/// Uma tabela do sistema (sqlite_*) ou sombra de tabela virtual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SistemaResumo {
    pub nome: String,
    pub tipo: TipoObjeto,
    pub linhas: Option<u64>,
    pub colunas: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndiceResumo {
    pub nome: String,
    pub tabela: String,
    pub unico: bool,
    /// Índice criado pelo SQLite (UNIQUE, PRIMARY KEY), sem SQL.
    pub automatico: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatilhoResumo {
    pub nome: String,
    pub tabela: String,
}

/// ObjetosInfo (§4.2): "tabelas" exclui as sombras de tabela virtual e as sqlite_*, que vão em "sistema".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjetosInfo {
    pub tabelas: Vec<TabelaResumo>,
    pub visoes: Vec<VisaoResumo>,
    pub sistema: Vec<SistemaResumo>,
    pub indices: Vec<IndiceResumo>,
    pub gatilhos: Vec<GatilhoResumo>,
}

/// `{"linhas", "tempo_ms"}` da contagem da grade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contagem {
    pub linhas: u64,
    /// Tempo das consultas no SQLite, em ms, com uma casa.
    pub tempo_ms: f64,
}

/// A chave estrangeira de uma coluna (`ColunaInfo.fk`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FkColuna {
    pub tabela: String,
    pub coluna: Option<String>,
    /// "NO ACTION", "CASCADE", "SET NULL"… como o SQLite escreve.
    pub ao_excluir: String,
    pub ao_atualizar: String,
}

/// Coluna de TabelaInfo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColunaInfo {
    pub cid: i64,
    pub nome: String,
    /// Tipo declarado ("" sem tipo).
    pub tipo: String,
    pub afinidade: Afinidade,
    pub nao_nulo: bool,
    /// Expressão do DEFAULT como está no esquema ("'São Paulo'", "CURRENT_TIMESTAMP").
    pub padrao: Option<String>,
    /// Posição na chave primária (0 = fora dela).
    pub pk: i64,
    /// 0 normal, 1 oculta (tabela virtual), 2 gerada VIRTUAL, 3 gerada STORED.
    pub oculta: i64,
    pub gerada: bool,
    pub autoincremento: bool,
    pub rowid_alias: bool,
    /// Tem um índice UNIQUE só dela.
    pub unica: bool,
    pub fk: Option<FkColuna>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChaveEstrangeira {
    pub id: i64,
    pub colunas: Vec<String>,
    pub tabela: String,
    /// None: REFERENCES sem coluna, e a tabela referenciada não tem chave primária para completar.
    pub colunas_ref: Vec<Option<String>>,
    pub ao_excluir: String,
    pub ao_atualizar: String,
}

/// Origem de um índice: "c" (CREATE INDEX), "u" (UNIQUE) ou "pk" (PRIMARY KEY).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrigemIndice {
    C,
    U,
    Pk,
}

impl OrigemIndice {
    pub fn como_str(self) -> &'static str {
        match self {
            OrigemIndice::C => "c",
            OrigemIndice::U => "u",
            OrigemIndice::Pk => "pk",
        }
    }

    pub(crate) fn de_texto(texto: &str) -> OrigemIndice {
        match texto {
            "u" => OrigemIndice::U,
            "pk" => OrigemIndice::Pk,
            _ => OrigemIndice::C,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColunaIndice {
    /// Nome da coluna, "(expressão)" ou "rowid".
    pub nome: String,
    pub desc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndiceInfo {
    pub nome: String,
    pub unico: bool,
    pub origem: OrigemIndice,
    pub parcial: bool,
    pub colunas: Vec<ColunaIndice>,
    /// None nos índices automáticos.
    pub sql: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatilhoInfo {
    pub nome: String,
    pub sql: Option<String>,
}

/// TabelaInfo (§4.3) sem `definicoes`/`restricoes`, que o serviço junta a partir do crate esquema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabelaInfo {
    pub nome: String,
    pub tipo: TipoObjeto,
    pub sistema: bool,
    pub sem_rowid: bool,
    pub estrita: bool,
    /// Dados: false em visão, virtual sem suporte, sistema (exceto sqlite_sequence), somente leitura.
    pub editavel: bool,
    /// Estrutura: false em visão, virtual, sombra, sistema, somente leitura.
    pub esquema_editavel: bool,
    pub chave: Option<TipoChave>,
    pub colunas: Vec<ColunaInfo>,
    pub chaves_estrangeiras: Vec<ChaveEstrangeira>,
    pub indices: Vec<IndiceInfo>,
    pub gatilhos: Vec<GatilhoInfo>,
    pub sql: Option<String>,
    /// None quando o arquivo passa de 256 MB (ou o SQLite não conseguiu contar).
    pub linhas: Option<u64>,
}

/// O pedido de uma página da grade (os parâmetros de `GET /api/tabela/linhas`).
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoPagina {
    /// A partir de 1 (0 vale 1; limitada a 2^31).
    pub pagina: u64,
    /// 1 a 1000.
    pub por_pagina: u64,
    pub ordem: Option<String>,
    pub direcao: Direcao,
    pub busca: Option<String>,
    pub filtros: Vec<Filtro>,
    /// false: nada é contado (`total`/`paginas` None), e a consulta pode parar cedo.
    pub contar: bool,
    /// Grupo da leitura substituída (`"grade:<tabela>"`): uma leitura mais nova do mesmo grupo abandona esta.
    pub leitura: Option<String>,
}

impl Default for PedidoPagina {
    fn default() -> Self {
        PedidoPagina {
            pagina: 1,
            por_pagina: 100,
            ordem: None,
            direcao: Direcao::Asc,
            busca: None,
            filtros: Vec::new(),
            contar: true,
            leitura: None,
        }
    }
}

/// Uma linha da grade: a identidade (None em visão ou PK nula) e as células, na ordem das colunas.
#[derive(Debug, Clone, PartialEq)]
pub struct LinhaPagina {
    pub chave: Option<Chave>,
    pub valores: Vec<Celula>,
}

/// Pagina (§4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Pagina {
    pub colunas: Vec<String>,
    pub tipos: Vec<String>,
    pub afinidades: Vec<Afinidade>,
    pub linhas: Vec<LinhaPagina>,
    /// None com `contar: false`.
    pub total: Option<u64>,
    pub pagina: u64,
    pub por_pagina: u64,
    /// teto(total / por_pagina), no mínimo 1; None com `contar: false`.
    pub paginas: Option<u64>,
    /// Há linhas depois desta página (sempre).
    pub tem_mais: bool,
    /// Nome real da coluna de ordem.
    pub ordem: Option<String>,
    pub direcao: Direcao,
    /// Tempo das consultas da página no SQLite, em ms, com uma casa (sempre).
    pub tempo_ms: f64,
}

/// Uma linha inteira, com o texto sem cortar (`GET /api/tabela/linha`).
#[derive(Debug, Clone, PartialEq)]
pub struct LinhaCompleta {
    pub chave: Option<Chave>,
    /// (coluna, célula) na ordem das colunas visíveis; texto inteiro, BLOB resumido.
    pub valores: Vec<(String, Celula)>,
}

impl LinhaCompleta {
    /// A célula de uma coluna pelo nome exato.
    pub fn valor(&self, coluna: &str) -> Option<&Celula> {
        self.valores.iter().find(|(n, _)| n == coluna).map(|(_, c)| c)
    }
}

/// Resposta de inserir/atualizar: a chave (nova, se a PK mudou) e a linha relida do banco (com DEFAULT e
/// gatilhos aplicados). `linha` None quando a tabela não tem identidade ou um gatilho apagou a linha.
#[derive(Debug, Clone, PartialEq)]
pub struct LinhaGravada {
    pub chave: Option<Chave>,
    pub linha: Option<Vec<Celula>>,
}

/// Resultado de um comando da Consulta SQL.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoComando {
    /// Comando que devolve linhas (SELECT, PRAGMA com resultado, … RETURNING).
    Linhas { comando: String, colunas: Vec<String>, linhas: Vec<Vec<Celula>>, truncado: bool, tempo_ms: f64 },
    /// Comando sem resultado; `linhas_afetadas` só em INSERT/UPDATE/DELETE/REPLACE.
    Alteracao { comando: String, linhas_afetadas: Option<u64>, tempo_ms: f64 },
    /// O roteiro parou aqui; os comandos anteriores já gravaram.
    Erro { comando: String, mensagem: String, detalhe: Option<String> },
}

impl ResultadoComando {
    /// "linhas" | "alteracao" | "erro".
    pub fn tipo(&self) -> &'static str {
        match self {
            ResultadoComando::Linhas { .. } => "linhas",
            ResultadoComando::Alteracao { .. } => "alteracao",
            ResultadoComando::Erro { .. } => "erro",
        }
    }

    pub fn comando(&self) -> &str {
        match self {
            ResultadoComando::Linhas { comando, .. }
            | ResultadoComando::Alteracao { comando, .. }
            | ResultadoComando::Erro { comando, .. } => comando,
        }
    }
}

/// Resultado da consulta livre (§4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoConsulta {
    pub resultados: Vec<ResultadoComando>,
    pub alterou_esquema: bool,
    pub alterou_dados: bool,
    pub transacao_aberta: bool,
}
