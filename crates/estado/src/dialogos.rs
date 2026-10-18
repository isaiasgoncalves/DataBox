//! Diálogos do Windows, pelo `rfd` (IFileDialog), com a janela do aplicativo
//! como dona: nascem na frente e prendem o foco.
//!
//! Rodam na thread da interface e bloqueiam até a pessoa escolher (como qualquer diálogo modal do Windows;
//! o laço do diálogo continua entregando as mensagens da janela). Depois de escolher, a tela emite
//! `Acao::LembrarDiretorio(pasta)` para o próximo diálogo abrir na mesma pasta — [`Dialogos`] devolve a pasta
//! junto com o caminho para isso.

use std::path::{Path, PathBuf};

use comum::arquivos::{EXTENSOES_BANCO, TipoDeArquivo};

/// Os bancos que o diálogo de abrir mostra; as extensões são as de `comum::arquivos`.
const ROTULO_BANCOS: &str = "Bancos SQLite (*.db;*.sqlite;*.sqlite3;*.db3;*.s3db;*.sl3)";
const ROTULO_TODOS: &str = "Todos os arquivos (*.*)";
/// O primeiro filtro do "Abrir arquivo": tudo o que o DataBox abre (bancos e arquivos de dados).
const ROTULO_QUE_ABRE: &str = "Arquivos que o DataBox abre";

/// Os filtros do diálogo "Abrir arquivo", na ordem em que aparecem: o que o DataBox abre (as extensões de banco e as
/// de todos os tipos de dados), os bancos SQLite e todos os arquivos. O primeiro é o selecionado.
pub fn filtros_de_abrir_arquivo() -> [(&'static str, Vec<&'static str>); 3] {
    let que_abre = EXTENSOES_BANCO
        .iter()
        .copied()
        .chain(TipoDeArquivo::TODOS.into_iter().flat_map(|t| t.extensoes().iter().copied()))
        .collect();
    [(ROTULO_QUE_ABRE, que_abre), (ROTULO_BANCOS, EXTENSOES_BANCO.to_vec()), (ROTULO_TODOS, vec!["*"])]
}

/// O que o diálogo de salvar vai gravar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoArquivo {
    Xlsx,
    Csv,
    Json,
    /// Banco inteiro em CSV: um .zip com um CSV por tabela.
    Zip,
    /// Banco SQLite (novo banco, cópia de segurança).
    Db,
    /// Qualquer arquivo (BLOB salvo em arquivo).
    Arquivo,
}

impl FormatoArquivo {
    /// (rótulo do filtro, extensões, título do diálogo).
    fn descricao(self) -> (&'static str, &'static [&'static str], &'static str) {
        match self {
            FormatoArquivo::Xlsx => ("Pasta de trabalho do Excel (*.xlsx)", &["xlsx"], "Exportar para o Excel"),
            FormatoArquivo::Csv => ("Arquivo CSV (*.csv)", &["csv"], "Exportar para CSV"),
            FormatoArquivo::Json => ("Arquivo JSON (*.json)", &["json"], "Exportar para JSON"),
            FormatoArquivo::Zip => {
                ("Pasta compactada com arquivos CSV (*.zip)", &["zip"], "Exportar para CSV (um arquivo por tabela)")
            }
            FormatoArquivo::Db => (ROTULO_BANCOS, &EXTENSOES_BANCO, "Salvar banco SQLite"),
            FormatoArquivo::Arquivo => (ROTULO_TODOS, &[], "Salvar em arquivo"),
        }
    }

    /// A extensão padrão (o `DefExt`): posta quando a pessoa digita o nome sem extensão.
    pub fn extensao(self) -> Option<&'static str> {
        self.descricao().1.first().copied()
    }

    /// Formato do arquivo exportado.
    pub fn de_exportacao(formato: exportar::Formato, banco_inteiro: bool) -> FormatoArquivo {
        match formato {
            exportar::Formato::Xlsx => FormatoArquivo::Xlsx,
            exportar::Formato::Json => FormatoArquivo::Json,
            exportar::Formato::Csv if banco_inteiro => FormatoArquivo::Zip,
            exportar::Formato::Csv => FormatoArquivo::Csv,
        }
    }
}

/// Caminho escolhido num diálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Escolha {
    pub caminho: PathBuf,
    /// A pasta, para `Acao::LembrarDiretorio`.
    pub pasta: Option<PathBuf>,
}

impl Escolha {
    fn de(caminho: PathBuf) -> Escolha {
        let pasta = caminho.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf);
        Escolha { caminho, pasta }
    }
}

/// Os diálogos, com a janela dona e a pasta inicial.
pub struct Dialogos<'a> {
    dono: Option<&'a eframe::Frame>,
    pasta: Option<PathBuf>,
}

impl<'a> Dialogos<'a> {
    pub fn novo(dono: Option<&'a eframe::Frame>, pasta: Option<PathBuf>) -> Dialogos<'a> {
        Dialogos { dono, pasta }
    }

    fn base(&self, titulo: &str) -> rfd::FileDialog {
        let mut d = rfd::FileDialog::new().set_title(titulo);
        if let Some(dono) = self.dono {
            d = d.set_parent(dono);
        }
        if let Some(p) = self.pasta.as_ref().filter(|p| p.is_dir()) {
            d = d.set_directory(p);
        }
        d
    }

    /// "Abrir banco SQLite": bancos e todos os arquivos. None = a pessoa cancelou. O
    /// [`Dialogos::abrir_arquivo`] é o da etapa 4 (com os filtros de dados) e o substitui no Ctrl+O.
    pub fn abrir_banco(&self) -> Option<Escolha> {
        self.base("Abrir banco SQLite")
            .add_filter(ROTULO_BANCOS, &EXTENSOES_BANCO)
            .add_filter(ROTULO_TODOS, &["*"])
            .pick_file()
            .map(Escolha::de)
    }

    /// "Abrir arquivo": o que o DataBox abre (bancos, CSV, Excel, JSON e texto), os bancos SQLite e todos os arquivos
    /// ([`filtros_de_abrir_arquivo`]). None = a pessoa cancelou. Quem chama decide o que o arquivo é
    /// (`Acao::AbrirCaminho`).
    pub fn abrir_arquivo(&self) -> Option<Escolha> {
        let mut d = self.base("Abrir arquivo");
        for (rotulo, extensoes) in filtros_de_abrir_arquivo() {
            d = d.add_filter(rotulo, extensoes.as_slice());
        }
        d.pick_file().map(Escolha::de)
    }

    /// "Abrir pasta": o seletor de pasta do Windows, começando na última pasta usada. A [`Escolha::pasta`] é a pasta
    /// **de cima** da escolhida, para o próximo diálogo abrir ao lado dela. None = a pessoa cancelou.
    pub fn abrir_pasta(&self) -> Option<Escolha> {
        self.base("Abrir pasta").pick_folder().map(Escolha::de)
    }

    /// "Salvar como", com o nome sugerido e o filtro do formato; o Windows já pergunta antes de substituir um
    /// arquivo existente (quem chama passa `substituir: true` ao serviço). Nome sem extensão ganha a do formato.
    pub fn salvar_como(&self, nome_sugerido: &str, formato: FormatoArquivo) -> Option<Escolha> {
        let (rotulo, extensoes, titulo) = formato.descricao();
        let mut d = self.base(titulo).set_file_name(nome_sugerido);
        if extensoes.is_empty() {
            d = d.add_filter(ROTULO_TODOS, &["*"]);
        } else {
            d = d.add_filter(rotulo, extensoes).add_filter(ROTULO_TODOS, &["*"]);
        }
        let caminho = d.save_file()?;
        Some(Escolha::de(com_extensao(caminho, formato.extensao())))
    }

    /// "Escolher arquivo" (qualquer arquivo; o conteúdo de um BLOB).
    pub fn escolher_arquivo(&self, titulo: &str) -> Option<Escolha> {
        let titulo = if titulo.is_empty() { "Escolher arquivo" } else { titulo };
        self.base(titulo).add_filter(ROTULO_TODOS, &["*"]).pick_file().map(Escolha::de)
    }
}

/// Acrescenta a extensão padrão a um nome que veio sem nenhuma.
fn com_extensao(caminho: PathBuf, extensao: Option<&str>) -> PathBuf {
    match extensao {
        Some(ext) if caminho.extension().is_none() => {
            let mut s = caminho.into_os_string();
            s.push(".");
            s.push(ext);
            PathBuf::from(s)
        }
        _ => caminho,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn extensao_padrao_so_quando_falta() {
        assert_eq!(com_extensao(PathBuf::from(r"C:\x\planilha"), Some("xlsx")), PathBuf::from(r"C:\x\planilha.xlsx"));
        assert_eq!(com_extensao(PathBuf::from(r"C:\x\dados.csv"), Some("xlsx")), PathBuf::from(r"C:\x\dados.csv"));
        assert_eq!(com_extensao(PathBuf::from(r"C:\x\blob"), None), PathBuf::from(r"C:\x\blob"));
        assert_eq!(FormatoArquivo::de_exportacao(exportar::Formato::Csv, true), FormatoArquivo::Zip);
        assert_eq!(FormatoArquivo::Db.extensao(), Some("db"));
    }

    #[test]
    fn escolha_guarda_a_pasta() {
        let e = Escolha::de(PathBuf::from(r"C:\dados\vendas.db"));
        assert_eq!(e.pasta, Some(PathBuf::from(r"C:\dados")));
    }

    #[test]
    fn dialogo_de_abrir_pasta_e_o_de_arquivo_com_os_tres_filtros() {
        let [que_abre, bancos, todos] = filtros_de_abrir_arquivo();
        assert_eq!(que_abre.0, "Arquivos que o DataBox abre");
        assert_eq!(bancos.0, ROTULO_BANCOS);
        assert_eq!(todos, ("Todos os arquivos (*.*)", vec!["*"]));
        // os bancos são a lista do comum, e o primeiro filtro tem os bancos e todas as extensões de dados
        assert_eq!(bancos.1, EXTENSOES_BANCO.to_vec());
        for ext in EXTENSOES_BANCO {
            assert!(que_abre.1.contains(&ext), "falta {ext}");
        }
        for tipo in TipoDeArquivo::TODOS {
            for ext in tipo.extensoes() {
                assert!(que_abre.1.contains(ext), "falta {ext}");
            }
        }
        for ext in ["csv", "tsv", "xlsx", "xlsm", "json", "txt", "md", "log"] {
            assert!(que_abre.1.contains(&ext), "falta {ext}");
        }
        assert!(
            !que_abre.1.contains(&"xls")
                && !que_abre.1.contains(&"xlsb")
                && !que_abre.1.contains(&"sql")
                && !que_abre.1.contains(&"*")
        );
        assert_eq!(que_abre.1.len(), EXTENSOES_BANCO.len() + 8, "sem repetição");
        // o rótulo dos bancos diz as mesmas extensões da lista do comum
        for ext in EXTENSOES_BANCO {
            assert!(ROTULO_BANCOS.contains(&format!("*.{ext}")), "o rótulo não diz {ext}");
        }
        // o "Salvar banco" usa a mesma lista
        assert_eq!(FormatoArquivo::Db.descricao().1, &EXTENSOES_BANCO);
        // a pasta escolhida guarda a de cima para o próximo diálogo
        let e = Escolha::de(PathBuf::from(r"C:\dados\projeto"));
        assert_eq!(e.pasta, Some(PathBuf::from(r"C:\dados")));
        assert_eq!(Escolha::de(PathBuf::from(r"C:\")).pasta, None, "a raiz de um disco não tem pasta de cima");
    }

    #[test]
    fn filtros_de_abrir_arquivo_incluem_xlsm() {
        let [(_, que_abre), (_, bancos), _] = filtros_de_abrir_arquivo();
        assert!(que_abre.contains(&"xlsx") && que_abre.contains(&"xlsm"));
        assert!(!bancos.contains(&"xlsm"));
    }
}
