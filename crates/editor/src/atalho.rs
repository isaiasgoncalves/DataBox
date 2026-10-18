//! `DataBox.exe --criar-atalho`: cria o atalho "DataBox" na Área de
//! Trabalho (também a redirecionada para o OneDrive) e no Menu Iniciar › Programas, apontando para o próprio
//! executável, com o ícone dele (o `databox.ico` embutido pelo build.rs) e a pasta dele como pasta de trabalho.
//!
//! O atalho é gravado pelo `WScript.Shell` de um `powershell -NoProfile` filho (sem janela): não exige `unsafe` nem crate novo. O script vai em
//! `-EncodedCommand` (UTF-16 em base64): caminhos com acento, espaço ou aspas chegam intactos. Fora do Windows não
//! há atalho `.lnk` nem `WScript.Shell`: [`criar_atalhos`] devolve o erro.

use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;

/// O nome do arquivo do atalho.
pub const NOME_ATALHO: &str = "DataBox.lnk";
/// A descrição do atalho (a dica do Explorer), a mesma do Sobre.
pub const DESCRICAO: &str =
    "Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.";

/// Texto entre aspas simples do PowerShell (a aspa simples dobra).
fn aspas(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "''"))
}

/// O script que cria os atalhos. `pastas` None = a Área de Trabalho e o Menu Iniciar da pessoa.
pub fn script(exe: &Path, pastas: Option<&[PathBuf]>) -> String {
    let exe_txt = exe.display().to_string();
    let pasta_exe = exe.parent().map(|p| p.display().to_string()).unwrap_or_default();
    let lista = match pastas {
        Some(lista) => {
            let itens: Vec<String> = lista.iter().map(|p| aspas(&p.display().to_string())).collect();
            format!("@({})", itens.join(", "))
        }
        None => "@($shell.SpecialFolders('Desktop'), $shell.SpecialFolders('Programs'))".to_string(),
    };
    format!(
        "$ErrorActionPreference = 'Stop'\n\
         [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
         $shell = New-Object -ComObject WScript.Shell\n\
         foreach ($pasta in {lista}) {{\n\
         \x20 [void][System.IO.Directory]::CreateDirectory($pasta)\n\
         \x20 $lnk = [System.IO.Path]::Combine($pasta, {nome})\n\
         \x20 $a = $shell.CreateShortcut($lnk)\n\
         \x20 $a.TargetPath = {alvo}\n\
         \x20 $a.Arguments = ''\n\
         \x20 $a.WorkingDirectory = {trabalho}\n\
         \x20 $a.IconLocation = {icone}\n\
         \x20 $a.Description = {descricao}\n\
         \x20 $a.WindowStyle = 1\n\
         \x20 $a.Save()\n\
         \x20 [Console]::Out.WriteLine($lnk)\n\
         }}\n",
        nome = aspas(NOME_ATALHO),
        alvo = aspas(&exe_txt),
        trabalho = aspas(&pasta_exe),
        icone = aspas(&format!("{exe_txt},0")),
        descricao = aspas(DESCRICAO),
    )
}

/// Base64 (padrão, com `=`) do texto em UTF-16LE: o que o `-EncodedCommand` do PowerShell espera.
pub fn base64_utf16(texto: &str) -> String {
    const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = texto.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut saida = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for bloco in bytes.chunks(3) {
        let b = [bloco[0], bloco.get(1).copied().unwrap_or(0), bloco.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, deslocamento) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= bloco.len() {
                saida.push(char::from(ALFABETO[((n >> deslocamento) & 63) as usize]));
            } else {
                saida.push('=');
            }
        }
    }
    saida
}

/// Cria os atalhos para `exe`; devolve os arquivos `.lnk` criados ou a mensagem de erro.
#[cfg(windows)]
pub fn criar_atalhos(exe: &Path, pastas: Option<&[PathBuf]>) -> Result<Vec<PathBuf>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let codificado = base64_utf16(&script(exe, pastas));
    let mut comando = Command::new("powershell.exe");
    comando.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &codificado]);
    comando.creation_flags(CREATE_NO_WINDOW);
    let saida = comando.output().map_err(|e| format!("Não foi possível executar o PowerShell: {e}"))?;
    if !saida.status.success() {
        let erro = String::from_utf8_lossy(&saida.stderr).trim().to_string();
        return Err(format!("Não foi possível criar o atalho. {erro}").trim().to_string());
    }
    let texto = String::from_utf8_lossy(&saida.stdout);
    Ok(texto.lines().map(str::trim).filter(|l| !l.is_empty()).map(PathBuf::from).collect())
}

/// Fora do Windows não há atalho `.lnk`: só o erro.
#[cfg(not(windows))]
pub fn criar_atalhos(_exe: &Path, _pastas: Option<&[PathBuf]>) -> Result<Vec<PathBuf>, String> {
    Err("O atalho só pode ser criado no Windows.".to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn base64_como_o_padrao() {
        // "ab" em UTF-16LE = 61 00 62 00
        assert_eq!(base64_utf16("ab"), "YQBiAA==");
        assert_eq!(base64_utf16("a"), "YQA=");
        assert_eq!(base64_utf16("abc"), "YQBiAGMA");
        assert_eq!(base64_utf16(""), "");
    }

    #[test]
    fn script_protege_as_aspas() {
        let s = script(Path::new(r"C:\Programas d'Ana\DataBox.exe"), Some(&[PathBuf::from(r"C:\Área")]));
        assert!(s.contains(r"$a.TargetPath = 'C:\Programas d''Ana\DataBox.exe'"), "{s}");
        assert!(s.contains(r"$a.IconLocation = 'C:\Programas d''Ana\DataBox.exe,0'"), "{s}");
        assert!(s.contains(r"@('C:\Área')"), "{s}");
        assert!(s.contains("'DataBox.lnk'"), "{s}");
        let padrao = script(Path::new(r"C:\x\DataBox.exe"), None);
        assert!(padrao.contains("SpecialFolders('Desktop')") && padrao.contains("SpecialFolders('Programs')"));
    }

    #[cfg(windows)]
    #[test]
    fn criar_atalho_numa_pasta_temporaria() {
        let pasta = std::env::temp_dir().join(format!("editor-atalho-{}", std::process::id())).join("Área de teste");
        let _ = std::fs::remove_dir_all(&pasta);
        let exe = std::env::current_exe().expect("exe");
        let criados = criar_atalhos(&exe, Some(std::slice::from_ref(&pasta))).expect("criar atalho");
        let lnk = pasta.join(NOME_ATALHO);
        assert_eq!(criados, vec![lnk.clone()]);
        assert!(lnk.is_file());
        // relê o atalho pelo mesmo objeto COM
        let ler = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
             $a = (New-Object -ComObject WScript.Shell).CreateShortcut({})\n\
             [Console]::Out.WriteLine($a.TargetPath)\n[Console]::Out.WriteLine($a.IconLocation)\n\
             [Console]::Out.WriteLine($a.WorkingDirectory)",
            aspas(&lnk.display().to_string())
        );
        let saida = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &base64_utf16(&ler)])
            .output()
            .expect("powershell");
        let texto = String::from_utf8_lossy(&saida.stdout).to_string();
        let linhas: Vec<&str> = texto.lines().collect();
        assert_eq!(linhas.first().map(|l| l.to_lowercase()), Some(exe.display().to_string().to_lowercase()));
        assert_eq!(linhas.get(1).map(|l| l.to_lowercase()), Some(format!("{},0", exe.display()).to_lowercase()));
        assert_eq!(
            linhas.get(2).map(|l| l.to_lowercase()),
            exe.parent().map(|p| p.display().to_string().to_lowercase())
        );
        let _ = std::fs::remove_dir_all(pasta.parent().unwrap_or(&pasta));
    }
}
