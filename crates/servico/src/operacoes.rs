//! As operações de mudança de estrutura, tipadas.

use comum::Resultado;
use ddl::ColunaDef;
use esquema::Plano;
use rusqlite::Connection;

/// Uma coluna de um índice novo: nome e se é decrescente.
pub type ColunaIndicePedida = esquema::ColunaIndice;

/// Uma mudança de estrutura (o `"operacao"` e os argumentos de `POST /api/esquema`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperacaoEsquema {
    CriarTabela {
        nome: String,
        colunas: Vec<ColunaDef>,
        restricoes: Vec<String>,
        sem_rowid: bool,
    },
    RenomearTabela {
        tabela: String,
        novo_nome: String,
    },
    ExcluirTabela {
        tabela: String,
    },
    AdicionarColuna {
        tabela: String,
        coluna: ColunaDef,
    },
    /// `coluna` é o nome atual; `nova` pode trazer outro nome (renomear).
    AlterarColuna {
        tabela: String,
        coluna: String,
        nova: ColunaDef,
    },
    ExcluirColuna {
        tabela: String,
        coluna: String,
    },
    ReordenarColunas {
        tabela: String,
        ordem: Vec<String>,
    },
    CriarIndice {
        nome: String,
        tabela: String,
        colunas: Vec<ColunaIndicePedida>,
        unico: bool,
        onde: Option<String>,
    },
    ExcluirIndice {
        nome: String,
    },
    /// `sql` é o SELECT da visão.
    CriarVisao {
        nome: String,
        sql: String,
    },
    ExcluirVisao {
        nome: String,
    },
    /// `sql` é o CREATE TRIGGER … BEGIN … END inteiro.
    CriarGatilho {
        sql: String,
    },
    ExcluirGatilho {
        nome: String,
    },
}

impl OperacaoEsquema {
    /// O texto da operação ("criar_tabela", "renomear_tabela"…).
    pub fn como_str(&self) -> &'static str {
        match self {
            OperacaoEsquema::CriarTabela { .. } => "criar_tabela",
            OperacaoEsquema::RenomearTabela { .. } => "renomear_tabela",
            OperacaoEsquema::ExcluirTabela { .. } => "excluir_tabela",
            OperacaoEsquema::AdicionarColuna { .. } => "adicionar_coluna",
            OperacaoEsquema::AlterarColuna { .. } => "alterar_coluna",
            OperacaoEsquema::ExcluirColuna { .. } => "excluir_coluna",
            OperacaoEsquema::ReordenarColunas { .. } => "reordenar_colunas",
            OperacaoEsquema::CriarIndice { .. } => "criar_indice",
            OperacaoEsquema::ExcluirIndice { .. } => "excluir_indice",
            OperacaoEsquema::CriarVisao { .. } => "criar_visao",
            OperacaoEsquema::ExcluirVisao { .. } => "excluir_visao",
            OperacaoEsquema::CriarGatilho { .. } => "criar_gatilho",
            OperacaoEsquema::ExcluirGatilho { .. } => "excluir_gatilho",
        }
    }
}

/// O `_montar_plano`: o `plano_*` da operação. Os erros do SQLite já chegam do crate esquema com a frase "Não
/// foi possível preparar a mudança de estrutura." na frente.
pub(crate) fn montar_plano(conn: &Connection, operacao: &OperacaoEsquema) -> Resultado<Plano> {
    match operacao {
        OperacaoEsquema::CriarTabela { nome, colunas, restricoes, sem_rowid } => {
            esquema::plano_criar_tabela(conn, nome, colunas, restricoes.as_slice(), *sem_rowid)
        }
        OperacaoEsquema::RenomearTabela { tabela, novo_nome } => {
            esquema::plano_renomear_tabela(conn, tabela, novo_nome)
        }
        OperacaoEsquema::ExcluirTabela { tabela } => esquema::plano_excluir_tabela(conn, tabela),
        OperacaoEsquema::AdicionarColuna { tabela, coluna } => esquema::plano_adicionar_coluna(conn, tabela, coluna),
        OperacaoEsquema::AlterarColuna { tabela, coluna, nova } => {
            esquema::plano_alterar_coluna(conn, tabela, coluna, nova)
        }
        OperacaoEsquema::ExcluirColuna { tabela, coluna } => esquema::plano_excluir_coluna(conn, tabela, coluna),
        OperacaoEsquema::ReordenarColunas { tabela, ordem } => {
            esquema::plano_reordenar_colunas(conn, tabela, ordem.as_slice())
        }
        OperacaoEsquema::CriarIndice { nome, tabela, colunas, unico, onde } => {
            esquema::plano_criar_indice(conn, nome, tabela, colunas, *unico, onde.as_deref())
        }
        OperacaoEsquema::ExcluirIndice { nome } => esquema::plano_excluir_indice(conn, nome),
        OperacaoEsquema::CriarVisao { nome, sql } => esquema::plano_criar_visao(conn, nome, sql),
        OperacaoEsquema::ExcluirVisao { nome } => esquema::plano_excluir_visao(conn, nome),
        OperacaoEsquema::CriarGatilho { sql } => esquema::plano_criar_gatilho(conn, sql),
        OperacaoEsquema::ExcluirGatilho { nome } => esquema::plano_excluir_gatilho(conn, nome),
    }
}
