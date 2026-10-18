//! Bancos de exemplo para os testes e para ver a aplicação funcionando.
//!
//! Os arquivos em `dados/` foram gerados uma vez, com semente fixa, e ficam no repositório: os testes contam com
//! exatamente os mesmos dados e números (as constantes abaixo). Não os regenere sem atualizar os testes.
//!
//! Conteúdo: clientes (AUTOINCREMENT, UNIQUE, CHECK, DEFAULT), produtos (WITHOUT ROWID, coluna gerada,
//! BLOB, inteiro acima de 2^53), pedidos (FK com CASCADE, CHECK), itens_pedido (chave composta), eventos
//! (5.000 linhas sem chave), "Tabela com espaço" (nomes com espaço e aspas), índices (um parcial), a visão
//! v_pedidos_por_cliente, o gatilho trg_pedido_cancelado e, no `exemplo.db`, a tabela virtual FTS5 `notas`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const N_CLIENTES: i64 = 60;
pub const N_PRODUTOS: i64 = 10;
pub const N_PEDIDOS: i64 = 240;
pub const N_EVENTOS: i64 = 5000;
/// Linhas de itens_pedido (sorteadas pela semente fixa).
pub const N_ITENS_PEDIDO: i64 = 456;

const EXEMPLO: &[u8] = include_bytes!("../dados/exemplo.db");
const EXEMPLO_SEM_FTS: &[u8] = include_bytes!("../dados/exemplo_sem_fts.db");

fn gravar(caminho: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    if let Some(pasta) = caminho.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(pasta)?;
    }
    fs::write(caminho, bytes)?;
    Ok(caminho.to_path_buf())
}

/// Cria (sobrescrevendo) o banco de exemplo, com a tabela virtual FTS5 `notas`.
pub fn criar_banco_exemplo(caminho: impl AsRef<Path>) -> io::Result<PathBuf> {
    gravar(caminho.as_ref(), EXEMPLO)
}

/// O mesmo banco de exemplo, sem a tabela FTS5.
pub fn criar_banco_exemplo_sem_fts(caminho: impl AsRef<Path>) -> io::Result<PathBuf> {
    gravar(caminho.as_ref(), EXEMPLO_SEM_FTS)
}

/// Cria (sobrescrevendo) um arquivo de banco vazio (0 bytes, como o `sqlite3.connect(...).close()`).
pub fn criar_banco_vazio(caminho: impl AsRef<Path>) -> io::Result<PathBuf> {
    let caminho = caminho.as_ref();
    if caminho.exists() {
        fs::remove_file(caminho)?;
    }
    rusqlite::Connection::open(caminho).map_err(io::Error::other)?;
    Ok(caminho.to_path_buf())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contagens_do_exemplo() {
        let pasta = std::env::temp_dir().join(format!("amostras-teste-{}", std::process::id()));
        let caminho = criar_banco_exemplo(pasta.join("exemplo.db")).unwrap();
        let conn = rusqlite::Connection::open(&caminho).unwrap();
        let contar =
            |t: &str| -> i64 { conn.query_row(&format!("select count(*) from {t}"), [], |r| r.get(0)).unwrap() };
        assert_eq!(contar("clientes"), N_CLIENTES);
        assert_eq!(contar("produtos"), N_PRODUTOS);
        assert_eq!(contar("pedidos"), N_PEDIDOS);
        assert_eq!(contar("eventos"), N_EVENTOS);
        assert_eq!(contar("itens_pedido"), N_ITENS_PEDIDO);
        assert_eq!(contar("notas"), 2);
        drop(conn);
        let vazio = criar_banco_vazio(pasta.join("vazio.db")).unwrap();
        assert!(vazio.exists());
        fs::remove_dir_all(pasta).unwrap();
    }
}
