//! Sem os diálogos (que ficam na interface, com rfd): a montagem
//! dos comandos de "Mostrar na pasta" e "Abrir", sem iniciar programa nenhum.
//!
//! Arquivo sem programa associado: pelo Explorador de Arquivos o Windows mostra a janela "Abrir com" em vez de um
//! erro.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::{Pasta, erro_com};
use crate::arquivos::{montar_abrir, montar_abrir_xdg, montar_revelar, montar_revelar_xdg};

#[cfg(windows)]
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

#[cfg(windows)]
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

#[test]
fn fora_do_windows_o_xdg_open_abre_o_arquivo_e_a_pasta() {
    let pasta = Pasta::nova("xdg");
    let arquivo = pasta.join("relatório final.csv");
    fs::write(&arquivo, "id\n").unwrap();
    let abrir = montar_abrir_xdg(&arquivo).unwrap();
    assert_eq!(abrir.programa, Path::new("xdg-open"));
    assert_eq!(Path::new(&abrir.argumentos), std::path::absolute(&arquivo).unwrap());
    // o "Mostrar na pasta" abre a pasta do arquivo (o xdg-open não seleciona); numa pasta, ela mesma
    let revelar = montar_revelar_xdg(&arquivo).unwrap();
    assert_eq!(Path::new(&revelar.argumentos), std::path::absolute(&arquivo).unwrap().parent().unwrap());
    let da_pasta = montar_revelar_xdg(&pasta.join(".")).unwrap();
    assert!(Path::new(&da_pasta.argumentos).is_dir());
    erro_com(montar_abrir_xdg(&pasta.join("sumiu.csv")), 404);
    erro_com(montar_revelar_xdg(Path::new("")), 400);
}

#[cfg(not(windows))]
#[test]
fn fora_do_windows_a_linha_mostra_o_caminho_entre_aspas() {
    let pasta = Pasta::nova("xdg-linha");
    let arquivo = pasta.join("a b.csv");
    fs::write(&arquivo, "id\n").unwrap();
    let comando = crate::arquivos::comando_abrir(&arquivo).unwrap();
    assert_eq!(comando.linha(), format!("xdg-open \"{}\"", std::path::absolute(&arquivo).unwrap().display()));
}
