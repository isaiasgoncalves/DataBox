//! Mede a exportação de ponta a ponta (ler do SQLite e gravar o arquivo) de uma tabela de 1 milhão de linhas,
//! nos três formatos, pelo mesmo caminho que a janela Exportar usa (`Servico::exportar`).
//!
//! ```text
//! .\cargo.ps1 run --release -p servico --example medir_exportacao
//! ```

use std::path::PathBuf;
use std::time::Instant;

use servico::{FiltroExportacao, Formato, Opcoes, OrigemExportacao, PedidoExportacao, Servico};

const LINHAS: u64 = 1_000_000;

fn main() {
    let pasta = std::env::temp_dir().join(format!("medir-exportacao-{}", std::process::id()));
    std::fs::create_dir_all(&pasta).expect("pasta");
    let banco = pasta.join("grande.db");
    {
        let conn = rusqlite::Connection::open(&banco).expect("criar banco");
        conn.execute_batch(&format!(
            "CREATE TABLE eventos (id INTEGER PRIMARY KEY, momento TEXT, tipo TEXT, valor REAL, dados TEXT);
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {LINHAS})
             INSERT INTO eventos (momento, tipo, valor, dados)
             SELECT printf('2026-09-%02dT%02d:%02d:00', i % 28 + 1, i % 24, i % 60),
                    CASE i % 4 WHEN 0 THEN 'login' WHEN 1 THEN 'consulta' WHEN 2 THEN 'exportacao' ELSE 'erro' END,
                    (i % 10000) / 7.0, '{{\"origem\": \"teste\", \"n\": ' || i || '}}' FROM n;"
        ))
        .expect("preencher");
    }
    let prefs = prefs::Preferencias::abrir(Some(pasta.join("preferencias.json")));
    let servico = Servico::novo(prefs).com_arquivo_log(pasta.join("editor.log"));
    let info = servico.abrir_banco(&banco, false, false).expect("abrir");
    let tamanho = std::fs::metadata(&banco).map(|m| m.len()).unwrap_or(0);
    println!("banco: {LINHAS} linhas, {:.1} MB", tamanho as f64 / 1e6);
    for (formato, ext) in [(Formato::Csv, "csv"), (Formato::Json, "json"), (Formato::Xlsx, "xlsx")] {
        let destino: PathBuf = pasta.join(format!("eventos.{ext}"));
        let pedido = PedidoExportacao {
            origem: OrigemExportacao::Tabela { nome: "eventos".into(), filtro: FiltroExportacao::default() },
            formato,
            caminho: destino.clone(),
            opcoes: Opcoes::default(),
            substituir: true,
        };
        let inicio = Instant::now();
        let resumo = servico.exportar(&info.id, &pedido, None).expect("exportar");
        println!(
            "{ext:>4}: {:>6.1} s · {} linhas · {:.1} MB",
            inicio.elapsed().as_secs_f64(),
            resumo.linhas,
            resumo.bytes as f64 / 1e6
        );
    }
    let _ = servico.fechar_banco(&info.id, true);
    let _ = std::fs::remove_dir_all(&pasta);
}
