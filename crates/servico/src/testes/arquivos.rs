//! Sem os diálogos (que ficam na interface, com rfd): a montagem
//! dos comandos de "Mostrar na pasta" e "Abrir", sem iniciar programa nenhum.
//!
//! Arquivo sem programa associado: pelo Explorador de Arquivos o Windows mostra a janela "Abrir com" em vez de um
//! erro.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::{Pasta, erro_com};
use crate::arquivos::{montar_abrir, montar_revelar};

#[test]
fn revelar_monta_a_linha_do_explorer() {
    let pasta = Pasta::nova("explorer");
    let arquivo = pasta.join("Exportações do mês").join("relatório final.xlsx");
    fs::create_dir(arquivo.parent().unwrap()).unwrap();
    fs::write(&arquivo, b"x").unwrap();
    let comando = montar_revelar(Some(OsStr::new(r"C:\Windows")), &arquivo).unwrap();
    assert_eq!(comando.linha(), format!("\"C:\\Windows\\explorer.exe\" /select,\"{}\"", arquivo.display()));
    assert_eq!(comando.programa, Path::new(r"C:\Windows\explorer.exe"));
    // sem a variável SystemRoot, o Windows de sempre
    assert_eq!(montar_revelar(None, &arquivo).unwrap().programa, Path::new(r"C:\Windows\explorer.exe"));
}

#[test]
fn revelar_arquivo_inexistente_e_404() {
    let pasta = Pasta::nova("explorer-404");
    erro_com(montar_revelar(None, &pasta.join("sumiu.db")), 404);
    assert_eq!(erro_com(montar_revelar(None, Path::new("")), 400).mensagem, "Nenhum arquivo foi indicado.");
}

#[test]
fn abrir_com_programa_padrao() {
    let pasta = Pasta::nova("abrir");
    let arquivo = pasta.join("clientes.csv");
    fs::write(&arquivo, "id;nome\n").unwrap();
    let comando = montar_abrir(Some(OsStr::new(r"D:\Win")), &arquivo).unwrap();
    assert_eq!(comando.linha(), format!("\"D:\\Win\\explorer.exe\" \"{}\"", arquivo.display()));
}

#[test]
fn abrir_com_programa_padrao_erros() {
    let pasta = Pasta::nova("abrir-404");
    let e = erro_com(montar_abrir(None, &pasta.join("sumiu.csv")), 404);
    assert_eq!(e.mensagem, format!("O arquivo não existe (ou foi movido): {}.", pasta.join("sumiu.csv").display()));
}
