//! As duas chamadas diretas à API em C do SQLite: o rusqlite não oferece `sqlite3_complete`, e o nome de uma
//! coluna com UTF-8 inválido (gravado por outro programa) faz o `Statement::column_name` dele entrar em pânico.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int};
use std::ptr;

use rusqlite::{Connection, ffi};

/// `sqlite3_complete`: o texto termina um comando SQL completo (respeita aspas, comentários e o BEGIN…END
/// de um gatilho)? Um NUL no meio corta o texto ali, como o SQLite faria lendo a string em C.
pub(crate) fn comando_completo(sql: &str) -> bool {
    let ate_o_nul = sql.split('\0').next().unwrap_or_default();
    let Ok(texto) = CString::new(ate_o_nul) else {
        return false;
    };
    // SAFETY: `texto` é uma string C válida, terminada em NUL, viva durante toda a chamada; sqlite3_complete
    // só lê o texto e não guarda o ponteiro.
    unsafe { ffi::sqlite3_complete(texto.as_ptr()) != 0 }
}

/// Nomes das colunas do resultado de `sql` (um comando só), com "�" no lugar de bytes inválidos. None quando
/// o SQLite não consegue preparar o comando.
pub(crate) fn nomes_das_colunas(conn: &Connection, sql: &str) -> Option<Vec<String>> {
    let tamanho = c_int::try_from(sql.len()).ok()?;
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    // SAFETY: `conn.handle()` é o ponteiro da conexão aberta, válido enquanto `conn` está emprestada (a função
    // inteira) e usado só nesta thread (quem chama segura a trava da conexão ou é dono dela). O texto vai com o
    // tamanho em bytes, sem precisar de NUL. Os ponteiros de nome são lidos (e copiados) antes de o comando
    // preparado ser finalizado, e ele é finalizado em todos os caminhos.
    unsafe {
        let db = conn.handle();
        let rc = ffi::sqlite3_prepare_v2(db, sql.as_ptr().cast::<c_char>(), tamanho, &mut stmt, ptr::null_mut());
        if rc != ffi::SQLITE_OK || stmt.is_null() {
            if !stmt.is_null() {
                ffi::sqlite3_finalize(stmt);
            }
            return None;
        }
        let n = ffi::sqlite3_column_count(stmt);
        let mut nomes = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
        for i in 0..n {
            let nome = ffi::sqlite3_column_name(stmt, i);
            nomes.push(if nome.is_null() {
                String::new()
            } else {
                CStr::from_ptr(nome).to_string_lossy().into_owned()
            });
        }
        ffi::sqlite3_finalize(stmt);
        Some(nomes)
    }
}
