//! `DataBox.exe --criar-atalho`: cria o atalho "DataBox" na Área de
//! Trabalho (também a redirecionada para o OneDrive) e no Menu Iniciar › Programas, apontando para o próprio
//! executável, com o ícone dele (o `databox.ico` embutido pelo build.rs) e a pasta dele como pasta de trabalho.
//!
//! O atalho é gravado pelo `WScript.Shell` de um `powershell -NoProfile` filho (sem janela): não exige `unsafe` nem crate novo. O script vai em
//! `-EncodedCommand` (UTF-16 em base64): caminhos com acento, espaço ou aspas chegam intactos.
//!
//! Fora do Windows (Linux e outros desktops freedesktop), o atalho é o `databox.desktop` em
//! `$XDG_DATA_HOME/applications` (sem a variável, `~/.local/share/applications`), que o menu de aplicativos lê, com o
//! logo de 256 px gravado em `$XDG_DATA_HOME/icons/hicolor/256x256/apps/databox.png` ([`gravar_desktop`]). Rodando
//! de um AppImage, o atalho aponta para o `.AppImage` ([`executavel_do_pacote`]).

use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;

/// O nome do arquivo do atalho.
pub const NOME_ATALHO: &str = "DataBox.lnk";
/// O nome do atalho fora do Windows (a entrada do menu de aplicativos).
pub const NOME_DESKTOP: &str = "databox.desktop";
/// O nome do ícone gravado ao lado do `.desktop`.
pub const NOME_ICONE: &str = "databox.png";
/// A descrição do atalho (a dica do Explorer), a mesma do Sobre.
pub const DESCRICAO: &str =
    "Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.";

/// O executável que o atalho e o "abrir de novo" usam. Dentro de um AppImage é o próprio `.AppImage` (a variável
/// `APPIMAGE`, caminho absoluto de um arquivo que existe): o executável de dentro fica numa montagem temporária
/// (`/tmp/.mount_…`) que some quando o programa fecha. Fora dele, `atual` (o `current_exe`).
pub fn executavel_do_pacote(appimage: Option<std::ffi::OsString>, atual: PathBuf) -> PathBuf {
    match appimage.map(PathBuf::from) {
        Some(pacote) if pacote.is_absolute() && pacote.is_file() => pacote,
        _ => atual,
    }
}

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

/// Fora do Windows, o `databox.desktop` no menu de aplicativos (`pastas` None) ou em cada pasta pedida, com o
/// ícone; devolve os `.desktop` criados.
#[cfg(not(windows))]
pub fn criar_atalhos(exe: &Path, pastas: Option<&[PathBuf]>) -> Result<Vec<PathBuf>, String> {
    let dados = comum::app::base_ou(std::env::var_os("XDG_DATA_HOME"), || {
        comum::app::pasta_pessoal().join(".local").join("share")
    });
    match pastas {
        Some(lista) => lista.iter().map(|p| gravar_desktop(exe, p, p)).collect(),
        None => {
            let icones = dados.join("icons").join("hicolor").join("256x256").join("apps");
            Ok(vec![gravar_desktop(exe, &dados.join("applications"), &icones)?])
        }
    }
}

/// O texto de um valor do `Exec` do `.desktop`: entre aspas duplas, com `"`, `` ` ``, `$` e `\` precedidos de
/// barra, essas barras dobradas de novo (o arquivo também usa `\` como escape) e o `%` dobrado (é o dos `%f`).
fn exec_entre_aspas(texto: &str) -> String {
    let mut saida = String::from("\"");
    for c in texto.chars() {
        match c {
            '"' | '`' | '$' => {
                saida.push_str(r"\\");
                saida.push(c);
            }
            '\\' => saida.push_str(r"\\\\"),
            '%' => saida.push_str("%%"),
            _ => saida.push(c),
        }
    }
    saida.push('"');
    saida
}

/// O conteúdo do `databox.desktop` para `exe`, com o ícone `icone` (caminho absoluto). Caminho com quebra de linha
/// não cabe no arquivo: erro.
pub fn conteudo_desktop(exe: &Path, icone: &Path) -> Result<String, String> {
    let exe_txt = exe.display().to_string();
    let icone_txt = icone.display().to_string();
    let pasta_exe = exe.parent().map(|p| p.display().to_string()).unwrap_or_default();
    if [&exe_txt, &icone_txt, &pasta_exe].iter().any(|t| t.contains(['\n', '\r'])) {
        return Err("Não foi possível criar o atalho: o caminho do programa tem uma quebra de linha.".to_string());
    }
    let tipos = [
        "application/vnd.sqlite3",
        "application/x-sqlite3",
        "text/csv",
        "text/tab-separated-values",
        "application/json",
        "text/plain",
        "text/markdown",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.ms-excel.sheet.macroEnabled.12",
    ];
    let linhas = [
        "[Desktop Entry]".to_string(),
        "Type=Application".to_string(),
        "Version=1.5".to_string(),
        format!("Name={}", comum::app::NOME),
        format!("Comment={DESCRICAO}"),
        format!("Exec={} %f", exec_entre_aspas(&exe_txt)),
        format!("Path={pasta_exe}"),
        format!("Icon={icone_txt}"),
        "Terminal=false".to_string(),
        "Categories=Development;Database;Utility;".to_string(),
        format!("MimeType={};", tipos.join(";")),
        format!("StartupWMClass={}", comum::app::NOME),
    ];
    Ok(linhas.join("\n") + "\n")
}

/// Grava o `databox.desktop` em `pasta` e o logo em `pasta_icone` (criando as duas); devolve o `.desktop`.
pub fn gravar_desktop(exe: &Path, pasta: &Path, pasta_icone: &Path) -> Result<PathBuf, String> {
    let falha = |e: std::io::Error| format!("Não foi possível criar o atalho: {e}");
    let icone = pasta_icone.join(NOME_ICONE);
    let conteudo = conteudo_desktop(exe, &icone)?;
    std::fs::create_dir_all(pasta).map_err(falha)?;
    std::fs::create_dir_all(pasta_icone).map_err(falha)?;
    std::fs::write(&icone, interface::marca::logo_png_maior()).map_err(falha)?;
    let desktop = pasta.join(NOME_DESKTOP);
    std::fs::write(&desktop, conteudo).map_err(falha)?;
    Ok(desktop)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn executavel_do_pacote_so_com_appimage_que_existe() {
        let atual = PathBuf::from("atual");
        assert_eq!(executavel_do_pacote(None, atual.clone()), atual);
        // relativo ou inexistente: fica o executável de agora
        assert_eq!(executavel_do_pacote(Some("DataBox.AppImage".into()), atual.clone()), atual);
        let pasta = std::env::temp_dir().join(format!("databox-appimage-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let pacote = pasta.join("DataBox-x86_64.AppImage");
        assert_eq!(executavel_do_pacote(Some(pacote.clone().into()), atual.clone()), atual);
        std::fs::write(&pacote, b"").expect("pacote");
        assert_eq!(executavel_do_pacote(Some(pacote.clone().into()), atual), pacote);
        std::fs::remove_dir_all(&pasta).ok();
    }

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

    #[test]
    fn desktop_protege_o_caminho() {
        let s = conteudo_desktop(Path::new("/opt/Dados d'Ana/$x/100%/DataBox"), Path::new("/i/databox.png")).unwrap();
        assert!(s.starts_with("[Desktop Entry]\nType=Application\n"), "{s}");
        assert!(s.contains(r#"Exec="/opt/Dados d'Ana/\\$x/100%%/DataBox" %f"#), "{s}");
        assert!(s.contains("\nIcon=/i/databox.png\n"), "{s}");
        assert!(s.contains("\nStartupWMClass=DataBox\n"), "{s}");
        assert!(s.contains("text/csv;") && s.contains("spreadsheetml.sheet;"), "{s}");
        assert_eq!(exec_entre_aspas(r#"a"b\c`"#), r#""a\\"b\\\\c\\`""#);
        assert!(conteudo_desktop(Path::new("/a\nb"), Path::new("/i.png")).is_err());
    }

    #[test]
    fn gravar_desktop_numa_pasta_temporaria() {
        let pasta = std::env::temp_dir().join(format!("editor-desktop-{}", std::process::id())).join("Área");
        let _ = std::fs::remove_dir_all(&pasta);
        let icones = pasta.join("icones");
        let desktop = gravar_desktop(Path::new("/opt/DataBox/DataBox"), &pasta, &icones).unwrap();
        assert_eq!(desktop, pasta.join(NOME_DESKTOP));
        let texto = std::fs::read_to_string(&desktop).unwrap();
        assert!(texto.contains(&format!("Icon={}", icones.join(NOME_ICONE).display())), "{texto}");
        let png = std::fs::read(icones.join(NOME_ICONE)).unwrap();
        assert!(png.starts_with(b"\x89PNG"), "o ícone é o PNG do logo");
        let _ = std::fs::remove_dir_all(pasta.parent().unwrap_or(&pasta));
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
