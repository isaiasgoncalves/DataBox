//! A linha de comando do DataBox.exe:
//!
//! ```text
//! DataBox.exe [caminho] [--somente-leitura] [--renderizador glow|wgpu] [--criar-atalho]
//! ```
//!
//! O `caminho` é uma pasta, um banco ou um arquivo de dados (quem classifica é a casca, pelo
//! `Servico::alvo_do_caminho`, como o caminho digitado no Início).
//!
//! O executável não tem console (`windows_subsystem = "windows"`): a ajuda, a versão e os erros de argumento
//! aparecem numa caixa de mensagem (quem mostra é o `main`).

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// A linha de uso (a primeira linha da ajuda e de todo erro de argumento). O `--renderizador glow|wgpu` continua
/// aceito, mas fica fora da ajuda: é a reserva que o próprio executável usa quando o OpenGL não abre.
pub const USO: &str = "Uso: DataBox.exe [caminho] [--somente-leitura] [--criar-atalho]";

/// O texto da ajuda (`-h`, `--ajuda`, `--help`); a descrição é a do Sobre.
pub const AJUDA: &str = "Uso: DataBox.exe [caminho] [--somente-leitura] [--criar-atalho]

Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.

Argumentos:
  caminho              pasta, banco SQLite ou arquivo de dados para abrir ao iniciar

Opções:
  --somente-leitura    abre o banco sem permitir gravação
  --criar-atalho       cria o atalho \"DataBox\" na Área de Trabalho e no Menu Iniciar e sai
  -h, --ajuda          mostra esta ajuda e sai
  --versao             mostra a versão e sai";

/// Como a janela é desenhada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Renderizador {
    /// OpenGL (o padrão: ~110 MB de memória).
    #[default]
    Glow,
    /// wgpu (DirectX 12/Vulkan): a reserva quando o OpenGL não abre.
    Wgpu,
}

impl Renderizador {
    /// "glow" | "wgpu".
    pub fn como_str(self) -> &'static str {
        match self {
            Renderizador::Glow => "glow",
            Renderizador::Wgpu => "wgpu",
        }
    }

    /// "glow" | "wgpu" (sem diferença de caixa); outro texto → None.
    pub fn de_texto(texto: &str) -> Option<Renderizador> {
        match texto.to_ascii_lowercase().as_str() {
            "glow" => Some(Renderizador::Glow),
            "wgpu" => Some(Renderizador::Wgpu),
            _ => None,
        }
    }
}

/// Os argumentos de uma execução normal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Argumentos {
    /// A pasta, o banco ou o arquivo de dados a abrir ao iniciar (absoluto).
    pub caminho: Option<PathBuf>,
    pub somente_leitura: bool,
    pub renderizador: Renderizador,
    /// Só cria os atalhos e sai.
    pub criar_atalho: bool,
}

/// O que a linha de comando pede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comando {
    Executar(Argumentos),
    /// `-h`, `--ajuda`, `--help`: mostra [`AJUDA`] e sai.
    Ajuda,
    /// `--versao`, `--version`: mostra "DataBox 0.1.1" e sai.
    Versao,
}

/// Argumento que não foi entendido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroArgumentos {
    /// A frase do problema, sem o uso ("opção desconhecida: --x").
    pub mensagem: String,
}

impl ErroArgumentos {
    /// O texto para a pessoa: o uso e a frase.
    pub fn texto(&self) -> String {
        format!("{USO}\nArgumentos inválidos: {}.", self.mensagem)
    }
}

fn erro(mensagem: impl Into<String>) -> ErroArgumentos {
    ErroArgumentos { mensagem: mensagem.into() }
}

/// Lê os argumentos (sem o nome do programa).
pub fn analisar(argv: &[OsString]) -> Result<Comando, ErroArgumentos> {
    let mut args = Argumentos::default();
    let mut caminho: Option<OsString> = None;
    let mut so_posicionais = false;
    let mut i = 0;
    while i < argv.len() {
        let bruto = &argv[i];
        i += 1;
        let texto = bruto.to_string_lossy();
        if so_posicionais || !texto.starts_with('-') || texto == "-" {
            if let Some(primeiro) = &caminho {
                return Err(erro(format!(
                    "só um caminho pode ser aberto ao iniciar (recebi {} e {})",
                    primeiro.to_string_lossy(),
                    texto
                )));
            }
            caminho = Some(bruto.clone());
            continue;
        }
        let (opcao, valor_junto) = match texto.split_once('=') {
            Some((o, v)) if o.starts_with("--") => (o.to_string(), Some(v.to_string())),
            _ => (texto.to_string(), None),
        };
        match opcao.as_str() {
            "--" => so_posicionais = true,
            "-h" | "--ajuda" | "--help" => return Ok(Comando::Ajuda),
            "--versao" | "--version" => return Ok(Comando::Versao),
            "--somente-leitura" => args.somente_leitura = true,
            "--criar-atalho" => args.criar_atalho = true,
            "--renderizador" => {
                let valor = match valor_junto {
                    Some(v) => v,
                    None => {
                        let proximo = argv.get(i).map(|v| v.to_string_lossy().to_string());
                        i += 1;
                        proximo.ok_or_else(|| erro("--renderizador precisa de um valor: glow ou wgpu"))?
                    }
                };
                args.renderizador = Renderizador::de_texto(&valor)
                    .ok_or_else(|| erro(format!("renderizador desconhecido: '{valor}' (use glow ou wgpu)")))?;
            }
            outra => return Err(erro(format!("opção desconhecida: {outra}"))),
        }
    }
    args.caminho = caminho.map(|b| absoluto(&b));
    Ok(Comando::Executar(args))
}

/// O caminho absoluto, sem tocar no disco (o `os.path.abspath`).
fn absoluto(caminho: &OsStr) -> PathBuf {
    let p = PathBuf::from(caminho);
    std::path::absolute(&p).unwrap_or(p)
}

/// Os argumentos para relançar o próprio executável com outro renderizador (os mesmos de antes).
pub fn argumentos_para_relancar(args: &Argumentos, renderizador: Renderizador) -> Vec<OsString> {
    let mut saida = Vec::new();
    if args.somente_leitura {
        saida.push(OsString::from("--somente-leitura"));
    }
    saida.push(OsString::from("--renderizador"));
    saida.push(OsString::from(renderizador.como_str()));
    if let Some(b) = &args.caminho {
        saida.push(OsString::from("--"));
        saida.push(b.clone().into_os_string());
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn a(lista: &[&str]) -> Vec<OsString> {
        lista.iter().map(OsString::from).collect()
    }

    fn executar(lista: &[&str]) -> Argumentos {
        match analisar(&a(lista)) {
            Ok(Comando::Executar(args)) => args,
            outro => panic!("esperava Executar: {outro:?}"),
        }
    }

    #[test]
    fn argumentos_padrao() {
        let args = executar(&[]);
        assert_eq!(args, Argumentos::default());
        assert_eq!(args.renderizador, Renderizador::Glow);
    }

    #[test]
    fn argumentos_todos() {
        let vendas = estado::apoio_testes::abs_texto("dados/vendas.db");
        let args = executar(&[&vendas, "--somente-leitura", "--renderizador", "wgpu", "--criar-atalho"]);
        assert_eq!(args.caminho, Some(PathBuf::from(&vendas)));
        assert!(args.somente_leitura && args.criar_atalho);
        assert_eq!(args.renderizador, Renderizador::Wgpu);
        assert_eq!(executar(&["--renderizador=GLOW"]).renderizador, Renderizador::Glow);
    }

    #[test]
    fn caminho_relativo_vira_absoluto() {
        let pasta = executar(&["dados"]).caminho.expect("pasta");
        assert!(pasta.is_absolute() && pasta.ends_with("dados"), "{}", pasta.display());
        let args = executar(&["vendas.db"]);
        let banco = args.caminho.expect("banco");
        assert!(banco.is_absolute(), "{}", banco.display());
        assert!(banco.ends_with("vendas.db"));
    }

    #[test]
    fn argumentos_invalidos() {
        for lista in [
            &["--renderizador", "vulkan"][..],
            &["--renderizador"],
            &["a.db", "b.db"],
            &["--desconhecido"],
            &["--porta", "8765"],
        ] {
            let e = analisar(&a(lista)).expect_err("devia recusar");
            assert!(e.texto().starts_with("Uso: DataBox.exe"), "{}", e.texto());
            assert!(e.texto().contains("Argumentos inválidos: "), "{}", e.texto());
        }
        assert!(analisar(&a(&["--renderizador", "vulkan"])).expect_err("x").mensagem.contains("vulkan"));
    }

    #[test]
    fn ajuda_e_versao() {
        assert_eq!(analisar(&a(&["--ajuda"])), Ok(Comando::Ajuda));
        // o argumento é um caminho qualquer: pasta, banco ou arquivo de dados
        assert!(USO.contains("[caminho]") && !USO.contains("banco.db"));
        assert!(AJUDA.contains("pasta, banco SQLite ou arquivo de dados"));
        assert_eq!(analisar(&a(&["-h"])), Ok(Comando::Ajuda));
        assert_eq!(analisar(&a(&["--versao"])), Ok(Comando::Versao));
        assert!(AJUDA.starts_with(USO));
    }

    #[test]
    fn traco_duplo_aceita_banco_que_parece_opcao() {
        let args = executar(&["--", "-estranho.db"]);
        assert!(args.caminho.expect("banco").ends_with("-estranho.db"));
    }

    #[test]
    fn relancar_repete_os_argumentos_com_o_outro_renderizador() {
        let vendas = estado::apoio_testes::abs_texto("dados/vendas.db");
        let args = executar(&[&vendas, "--somente-leitura"]);
        let novos = argumentos_para_relancar(&args, Renderizador::Wgpu);
        assert_eq!(novos, a(&["--somente-leitura", "--renderizador", "wgpu", "--", &vendas]));
        match analisar(&novos) {
            Ok(Comando::Executar(de_novo)) => {
                assert_eq!(de_novo.caminho, args.caminho);
                assert!(de_novo.somente_leitura);
                assert_eq!(de_novo.renderizador, Renderizador::Wgpu);
            }
            outro => panic!("{outro:?}"),
        }
    }
}
