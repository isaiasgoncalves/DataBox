//! As respostas do serviço que não existem nos outros crates.

use std::path::PathBuf;

use banco::{BancoInfo, TabelaInfo};
use ddl::ColunaDef;
use exportar::Formato;
use prefs::PrefsInfo;

/// `GET /api/estado` → `app` (sem `dialogos_nativos` e `instancia`, que não existem sem a página).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppInfo {
    /// "DataBox".
    pub nome: &'static str,
    /// A versão do aplicativo ([`crate::versao`]).
    pub versao: &'static str,
    /// A versão do SQLite embutido ("3.53.0").
    pub sqlite_versao: &'static str,
}

/// `GET /api/estado`.
#[derive(Debug, Clone, PartialEq)]
pub struct EstadoApp {
    pub app: AppInfo,
    /// None sem banco aberto.
    pub banco: Option<BancoInfo>,
    pub prefs: PrefsInfo,
}

/// `GET /api/tabela`: a [`TabelaInfo`] do banco com as definições do esquema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabelaCompleta {
    /// Com `esquema_editavel` false quando a definição não pôde ser entendida (ver `aviso_esquema`).
    pub info: TabelaInfo,
    /// As colunas como o analisador de CREATE TABLE as entende; None em visão, tabela virtual, sombra, tabela
    /// do sistema ou definição que o analisador não entende.
    pub definicoes: Option<Vec<ColunaDef>>,
    /// Restrições de tabela ("PRIMARY KEY (pedido_id, produto)"); None nos mesmos casos.
    pub restricoes: Option<Vec<String>>,
    /// Só quando a tabela teria a estrutura editável e o analisador não entendeu a definição: "Não foi
    /// possível entender a definição desta tabela, então a estrutura não pode ser mudada por aqui (use a
    /// Consulta SQL). <motivo>".
    pub aviso_esquema: Option<String>,
}

/// Resposta de uma gravação (inserir, atualizar, excluir, mudança de estrutura).
#[derive(Debug, Clone, PartialEq)]
pub struct Gravado<T> {
    pub resultado: T,
    /// A gravação ficou dentro de uma transação aberta na Consulta SQL: só vai para o arquivo com o COMMIT
    /// .
    pub transacao_aberta: bool,
}

/// `POST /api/tabela/blob/salvar` → `{"caminho", "bytes"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobSalvo {
    pub caminho: PathBuf,
    pub bytes: u64,
}

/// `POST /api/esquema` → `{"previa", "descricao", "sql", "comandos", "reconstroi", "avisos"}` (+ `transacao_aberta`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespostaEsquema {
    /// true: só o plano; false: a mudança já foi executada.
    pub previa: bool,
    pub descricao: String,
    /// O SQL que será (ou foi) executado: comandos unidos por ";\n\n" e terminados em ";".
    pub sql: String,
    pub comandos: Vec<String>,
    /// A mudança reconstrói a tabela (cópia das linhas para uma tabela nova).
    pub reconstroi: bool,
    pub avisos: Vec<String>,
    /// Executada (`previa` false) dentro de uma transação aberta na Consulta SQL.
    pub transacao_aberta: bool,
}

/// `POST /api/exportar` → `{"caminho", "nome", "formato", "linhas", "bytes", "tabelas", "tempo_ms", "avisos"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoExportacao {
    /// O arquivo gravado de fato (o banco inteiro em CSV vira .zip).
    pub caminho: PathBuf,
    /// O nome dele ("clientes.xlsx").
    pub nome: String,
    pub formato: Formato,
    pub linhas: u64,
    pub bytes: u64,
    pub tabelas: usize,
    pub tempo_ms: u64,
    /// Frases: células que o Excel não comportou como estavam (texto cortado, BLOB grande, real como texto).
    pub avisos: Vec<String>,
}
