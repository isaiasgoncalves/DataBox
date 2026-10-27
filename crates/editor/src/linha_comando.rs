//! A linha de comando do DataBox.exe:
//!
//! ```text
//! DataBox.exe [caminho] [--somente-leitura] [--moldura-do-sistema] [--renderizador glow|wgpu] [--criar-atalho]
//! ```
//!
//! O `caminho` é uma pasta, um banco ou um arquivo de dados (quem classifica é a casca, pelo
//! `Servico::alvo_do_caminho`, como o caminho digitado no Início).
//!
//! O executável não tem console (`windows_subsystem = "windows"`): a ajuda, a versão e os erros de argumento
//! aparecem numa caixa de mensagem (quem mostra é o `main`). Os três saem no idioma da interface (o das preferências
//! ou o do sistema, que o `main` define antes de mostrar), com o nome do executável que a pessoa chamou
//! ([`executavel`]).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use idioma::t;

/// O nome do executável quando o sistema não diz: "DataBox.exe" no Windows, "DataBox" fora.
pub const EXECUTAVEL_PADRAO: &str = if cfg!(windows) { "DataBox.exe" } else { "DataBox" };

/// O nome que a ajuda mostra: o do AppImage (`APPIMAGE`), senão o do próprio executável, senão
/// [`EXECUTAVEL_PADRAO`].
pub fn executavel(appimage: Option<&OsStr>, exe: Option<&Path>) -> String {
    appimage
        .map(Path::new)
        .or(exe)
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| EXECUTAVEL_PADRAO.to_string())
}

/// A linha de uso (a primeira linha da ajuda e de todo erro de argumento). O `--renderizador glow|wgpu` continua
/// aceito, mas fica fora da ajuda: é a reserva que o próprio executável usa quando o OpenGL não abre.
pub fn uso(executavel: &str) -> String {
    t!(
        "Uso: {executavel} [caminho] [--somente-leitura] [--moldura-do-sistema] [--criar-atalho]",
        executavel = executavel
    )
    .into_owned()
}

/// O texto da ajuda (`-h`, `--help`, `--ajuda`); a descrição é a do Sobre. Os nomes principais são os que quem lê
/// inglês espera (`--help`, `--version`, PO9 da revisão de 26/10/2026); `--ajuda` e `--versao` continuam valendo.
pub fn ajuda(executavel: &str) -> String {
    let atalho = if cfg!(windows) {
        t!("cria o atalho \"DataBox\" na Área de Trabalho e no Menu Iniciar e sai")
    } else {
        t!("cria o atalho \"DataBox\" no menu de aplicativos e sai")
    };
    let opcoes = [
        ("--somente-leitura", t!("abre o banco sem permitir gravação")),
        ("--moldura-do-sistema", t!("usa a barra de título e as bordas do sistema no lugar das do DataBox")),
        ("--criar-atalho", atalho),
        ("-h, --help", t!("mostra esta ajuda e sai (também --ajuda)")),
        ("--version", t!("mostra a versão e sai (também --versao)")),
    ];
    let argumento = t!("caminho");
    // a coluna das descrições: a opção mais longa (ou o "caminho" traduzido) e dois espaços
    let largura =
        opcoes.iter().map(|(o, _)| o.chars().count()).chain([argumento.chars().count()]).max().unwrap_or(0) + 2;
    let mut texto = format!(
        "{}\n\n{}\n\n{}\n  {:<largura$}{}\n\n{}",
        uso(executavel),
        t!("Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados."),
        t!("Argumentos:"),
        argumento,
        t!("pasta, banco SQLite ou arquivo de dados para abrir ao iniciar"),
        t!("Opções:"),
    );
    for (opcao, descricao) in opcoes {
        texto.push_str(&format!("\n  {opcao:<largura$}{descricao}"));
    }
    texto
}

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
    /// A janela com a decoração do sistema (a barra de título e as bordas dele), no lugar da barra própria: a saída
    /// para um gerenciador de janelas em que a barra própria não se comporta.
    pub moldura_do_sistema: bool,
}

/// O que a linha de comando pede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comando {
    Executar(Argumentos),
    /// `-h`, `--help`, `--ajuda`: mostra a [`ajuda`] e sai.
    Ajuda,
    /// `--versao`, `--version`: mostra "DataBox 0.2.4" e sai.
    Versao,
}

/// Argumento que não foi entendido. A frase sai no idioma de quando é mostrada: a leitura dos argumentos vem antes de
/// o `main` saber o idioma das preferências.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroArgumentos {
    /// Dois caminhos (o primeiro e o segundo).
    DoisCaminhos(String, String),
    RenderizadorSemValor,
    RenderizadorDesconhecido(String),
    OpcaoDesconhecida(String),
}

impl ErroArgumentos {
    /// A frase do problema, sem o uso ("opção desconhecida: --x").
    pub fn mensagem(&self) -> String {
        match self {
            ErroArgumentos::DoisCaminhos(a, b) => {
                t!("só um caminho pode ser aberto ao iniciar (recebi {a} e {b})", a = a, b = b)
            }
            ErroArgumentos::RenderizadorSemValor => t!("--renderizador precisa de um valor: glow ou wgpu"),
            ErroArgumentos::RenderizadorDesconhecido(v) => {
                t!("renderizador desconhecido: '{valor}' (use glow ou wgpu)", valor = v)
            }
            ErroArgumentos::OpcaoDesconhecida(o) => t!("opção desconhecida: {opcao}", opcao = o),
        }
        .into_owned()
    }

    /// O texto para a pessoa: o uso e a frase.
    pub fn texto(&self, executavel: &str) -> String {
        format!("{}\n{}", uso(executavel), t!("Argumentos inválidos: {mensagem}.", mensagem = self.mensagem()))
    }
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
                return Err(ErroArgumentos::DoisCaminhos(primeiro.to_string_lossy().into_owned(), texto.into_owned()));
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
            "--moldura-do-sistema" => args.moldura_do_sistema = true,
            "--renderizador" => {
                let valor = match valor_junto {
                    Some(v) => v,
                    None => {
                        let proximo = argv.get(i).map(|v| v.to_string_lossy().to_string());
                        i += 1;
                        proximo.ok_or(ErroArgumentos::RenderizadorSemValor)?
                    }
                };
                args.renderizador =
                    Renderizador::de_texto(&valor).ok_or(ErroArgumentos::RenderizadorDesconhecido(valor))?;
            }
            outra => return Err(ErroArgumentos::OpcaoDesconhecida(outra.to_string())),
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
    if args.moldura_do_sistema {
        saida.push(OsString::from("--moldura-do-sistema"));
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
        assert!(args.somente_leitura && args.criar_atalho && !args.moldura_do_sistema);
        assert_eq!(args.renderizador, Renderizador::Wgpu);
        assert!(executar(&["--moldura-do-sistema"]).moldura_do_sistema);
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
            let texto = idioma::com_idioma(idioma::PORTUGUES, || e.texto("DataBox.exe"));
            assert!(texto.starts_with("Uso: DataBox.exe"), "{texto}");
            assert!(texto.contains("Argumentos inválidos: "), "{texto}");
        }
        idioma::com_idioma(idioma::PORTUGUES, || {
            assert!(analisar(&a(&["--renderizador", "vulkan"])).expect_err("x").mensagem().contains("vulkan"));
            assert_eq!(
                analisar(&a(&["a.db", "b.db"])).expect_err("x").mensagem(),
                "só um caminho pode ser aberto ao iniciar (recebi a.db e b.db)"
            );
        });
    }

    #[test]
    fn ajuda_e_versao() {
        assert_eq!(analisar(&a(&["--ajuda"])), Ok(Comando::Ajuda));
        assert_eq!(analisar(&a(&["--help"])), Ok(Comando::Ajuda));
        assert_eq!(analisar(&a(&["-h"])), Ok(Comando::Ajuda));
        assert_eq!(analisar(&a(&["--versao"])), Ok(Comando::Versao));
        assert_eq!(analisar(&a(&["--version"])), Ok(Comando::Versao));
        idioma::com_idioma(idioma::PORTUGUES, || {
            // o argumento é um caminho qualquer: pasta, banco ou arquivo de dados
            let uso = uso("DataBox");
            assert!(uso.starts_with("Uso: DataBox [caminho]") && !uso.contains("banco.db"), "{uso}");
            let ajuda = ajuda("DataBox");
            assert!(ajuda.starts_with(&uso));
            assert!(ajuda.contains("pasta, banco SQLite ou arquivo de dados"));
            // os nomes principais, com os sinônimos na descrição
            assert!(ajuda.contains("\n  -h, --help ") && ajuda.contains("(também --ajuda)"), "{ajuda}");
            assert!(ajuda.contains("\n  --version ") && ajuda.contains("(também --versao)"), "{ajuda}");
            // as descrições (e a do caminho) começam todas na mesma coluna
            let colunas: Vec<Option<usize>> = [
                "pasta, banco",
                "abre o banco",
                "usa a barra",
                "cria o atalho",
                "mostra esta ajuda",
                "mostra a versão",
            ]
            .iter()
            .map(|d| ajuda.lines().find_map(|l| l.find(d).map(|i| l[..i].chars().count())))
            .collect();
            assert!(
                colunas[0].is_some() && colunas.windows(2).all(|w| w[0] == w[1]),
                "{colunas:?}
{ajuda}"
            );
        });
    }

    #[test]
    fn ajuda_e_erros_no_ingles() {
        idioma::com_idioma(idioma::INGLES, || {
            let ajuda = ajuda("DataBox");
            assert!(ajuda.starts_with("Usage: DataBox [path] [--somente-leitura]"), "{ajuda}");
            assert!(ajuda.contains("Options:") && ajuda.contains("shows this help and exits"), "{ajuda}");
            assert!(!ajuda.contains("Opções"), "{ajuda}");
            let e = analisar(&a(&["--desconhecido"])).expect_err("x");
            assert_eq!(
                e.texto("DataBox.exe").lines().nth(1),
                Some("Invalid arguments: unknown option: --desconhecido.")
            );
        });
    }

    #[test]
    fn nome_do_executavel() {
        let exe = PathBuf::from("/opt/databox/DataBox");
        assert_eq!(executavel(None, Some(&exe)), "DataBox");
        let appimage = OsString::from("/home/ana/Baixados/DataBox-0.3.0-x86_64.AppImage");
        assert_eq!(executavel(Some(&appimage), Some(&exe)), "DataBox-0.3.0-x86_64.AppImage");
        assert_eq!(executavel(None, None), EXECUTAVEL_PADRAO);
        assert_eq!(executavel(Some(OsStr::new("")), None), EXECUTAVEL_PADRAO);
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
        // a moldura do sistema vai junto
        let com_moldura = Argumentos { moldura_do_sistema: true, ..Argumentos::default() };
        assert_eq!(
            argumentos_para_relancar(&com_moldura, Renderizador::Wgpu),
            a(&["--moldura-do-sistema", "--renderizador", "wgpu"])
        );
    }
}
