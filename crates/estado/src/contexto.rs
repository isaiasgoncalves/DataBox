//! O estado que a casca mantém ([`Estado`]) e o que cada tela recebe a cada quadro ([`Contexto`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use banco::{BancoInfo, ObjetosInfo};
use comum::arquivos::Listagem;
use comum::{Codigo, ErroBanco, Resultado};
use prefs::PrefsInfo;
use servico::{AppInfo, Servico};

use crate::acao::Acao;
use crate::dialogos::Dialogos;
use crate::fonte::FonteDeColunas;
use crate::promessa::Promessa;
use crate::rota::Rota;

/// Frase do toast de uma gravação feita dentro de uma transação aberta na Consulta SQL.
pub const MSG_SO_NA_TRANSACAO: &str = "Salvo só na transação aberta; vai para o arquivo com COMMIT.";

/// Acima disto (ou com TabelaInfo.linhas desconhecido), a busca e os filtros da aba Dados pedem a página sem
/// contar e contam à parte.
pub const LIMIAR_TABELA_GRANDE: u64 = 200_000;

/// O que a casca sabe do aplicativo, do banco aberto e das preferências.
#[derive(Debug, Clone)]
pub struct Estado {
    pub app: AppInfo,
    pub banco: Option<BancoInfo>,
    pub objetos: Option<ObjetosInfo>,
    /// A pasta aberta no Explorador, com o que já foi lido de cada nível (independe do banco aberto).
    pub pasta: Option<PastaAberta>,
    pub prefs: PrefsInfo,
    /// Última pasta usada num diálogo (preferências), para o próximo diálogo abrir nela.
    pub ultimo_diretorio: Option<PathBuf>,
}

/// A pasta aberta (Arquivo › Abrir pasta): a raiz e a leitura de cada nível que a árvore já abriu.
#[derive(Debug, Clone, PartialEq)]
pub struct PastaAberta {
    pub caminho: PathBuf,
    /// O nome da raiz para o título e a barra de status ("dados"); numa raiz de disco, o próprio caminho.
    pub nome: String,
    /// A chave do caminho (`prefs::chave_caminho`): quem lê a pasta devolve o resultado com ela, e a casca descarta
    /// o que chega de uma pasta que já não é a aberta.
    pub chave: String,
    /// A leitura de cada pasta (a raiz e as subpastas que abriram), pela chave do caminho da pasta (para as
    /// subpastas, a `chave` da `comum::arquivos::Entrada` delas).
    pub nos: HashMap<String, NoDaPasta>,
}

/// O que a árvore sabe de uma pasta.
#[derive(Debug, Clone, PartialEq)]
pub enum NoDaPasta {
    /// A leitura está em andamento (o ponto pulsante).
    Lendo,
    /// Lida: as entradas ordenadas e cortadas (a `Arc` deixa a árvore segurar a lista sem copiá-la).
    Pronto(Arc<Listagem>),
    /// A leitura falhou: a frase para mostrar com o "Tentar de novo".
    Falhou(String),
}

impl PastaAberta {
    /// A pasta recém-aberta, com a raiz ainda sem leitura (`Lendo`).
    pub fn nova(caminho: impl Into<PathBuf>) -> PastaAberta {
        let caminho = caminho.into();
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| caminho.to_string_lossy().into_owned());
        let chave = prefs::chave_caminho(&caminho);
        let nos = HashMap::from([(chave.clone(), NoDaPasta::Lendo)]);
        PastaAberta { caminho, nome, chave, nos }
    }

    /// A listagem pronta da pasta com esta chave de caminho (a raiz ou uma subpasta); None se ainda lê, falhou ou
    /// nunca foi pedida.
    pub fn listagem(&self, chave: &str) -> Option<&Listagem> {
        match self.nos.get(chave)? {
            NoDaPasta::Pronto(l) => Some(l),
            NoDaPasta::Lendo | NoDaPasta::Falhou(_) => None,
        }
    }

    /// O caminho está na pasta aberta, em qualquer profundidade (a própria raiz também conta)? Compara pela chave
    /// do caminho (no Windows, sem diferença de caixa nem de barra) e por pastas inteiras: `C:\dados2` não está em
    /// `C:\dados`.
    pub fn contem(&self, caminho: &Path) -> bool {
        // a chave usa o separador do sistema: `\` no Windows (a `/` vira `\`), `/` nos outros
        let separador = if cfg!(windows) { '\\' } else { '/' };
        let raiz = self.chave.trim_end_matches(separador);
        let outro = prefs::chave_caminho(caminho);
        match outro.strip_prefix(raiz) {
            Some(resto) => resto.is_empty() || resto.starts_with(separador),
            None => false,
        }
    }
}

impl Estado {
    /// O id do banco aberto, que vai em todo pedido ao serviço ("" sem banco).
    pub fn banco_id(&self) -> String {
        self.banco.as_ref().map(|b| b.id.clone()).unwrap_or_default()
    }

    pub fn tem_banco(&self) -> bool {
        self.banco.is_some()
    }

    /// Há uma pasta aberta no Explorador.
    pub fn tem_pasta(&self) -> bool {
        self.pasta.is_some()
    }

    /// Há um BEGIN aberto na Consulta SQL.
    pub fn transacao_aberta(&self) -> bool {
        self.banco.as_ref().is_some_and(|b| b.transacao_aberta)
    }

    /// O banco aberto é só leitura.
    pub fn somente_leitura(&self) -> bool {
        self.banco.as_ref().is_some_and(|b| b.somente_leitura)
    }
}

/// O que fazer com um erro que voltou de uma tarefa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reacao {
    /// Leitura substituída por outra mais nova, ou banco trocado (a casca já foi avisada): ignore em silêncio.
    Ignorar,
    /// Mostre a mensagem (toast ou `.notice.perigo`, conforme a tela).
    Mostrar,
}

/// O que cada tela recebe a cada quadro: o serviço, o estado e o canal de [`Acao`] para a casca.
pub struct Contexto<'a> {
    pub servico: &'a Arc<Servico>,
    pub estado: &'a Estado,
    /// A janela do aplicativo (dona dos diálogos do Windows); None nos testes sem eframe.
    pub frame: Option<&'a eframe::Frame>,
    /// Muda sempre que o estado (banco, objetos, preferências) muda: os textos que a tela guarda são refeitos
    /// quando ela muda. 0 fora da casca (nos palcos dos testes).
    pub versao: u64,
    /// Há uma troca de banco em andamento (abrir, criar ou fechar): o Início desliga os botões dele.
    pub trocando: bool,
    acoes: &'a mut Vec<Acao>,
    /// As origens de colunas das abas abertas (a casca monta a lista a cada quadro); vazia fora dela.
    fontes: &'a [Arc<dyn FonteDeColunas>],
}

impl<'a> Contexto<'a> {
    pub fn novo(
        servico: &'a Arc<Servico>,
        estado: &'a Estado,
        frame: Option<&'a eframe::Frame>,
        acoes: &'a mut Vec<Acao>,
    ) -> Contexto<'a> {
        Contexto { servico, estado, frame, versao: 0, trocando: false, acoes, fontes: &[] }
    }

    /// O contexto com as origens de colunas das abas abertas (o "Trazer colunas" as lista). Opcional: sem isto, a
    /// lista é vazia.
    pub fn com_fontes(mut self, fontes: &'a [Arc<dyn FonteDeColunas>]) -> Self {
        self.fontes = fontes;
        self
    }

    /// As origens de colunas das abas abertas, na ordem das abas (o documento que está desenhando também entra: a
    /// janela decide se o tira da lista).
    pub fn fontes(&self) -> &[Arc<dyn FonteDeColunas>] {
        self.fontes
    }

    /// O contexto com o que só a casca sabe: a versão do estado e se há uma troca de banco em andamento.
    pub fn com_casca(mut self, versao: u64, trocando: bool) -> Self {
        self.versao = versao;
        self.trocando = trocando;
        self
    }

    /// Pede algo à casca (aplicado no fim do quadro).
    pub fn emitir(&mut self, acao: Acao) {
        self.acoes.push(acao);
    }

    pub fn navegar(&mut self, rota: Rota) {
        self.emitir(Acao::Navegar(rota));
    }

    /// O id do banco aberto, para os pedidos ao serviço.
    pub fn banco_id(&self) -> String {
        self.estado.banco_id()
    }

    /// Roda uma chamada ao serviço numa thread de fundo, com o serviço e o id do banco aberto agora.
    pub fn tarefa<T: Send + 'static>(
        &self,
        ctx: &egui::Context,
        f: impl FnOnce(&Servico, &str) -> Resultado<T> + Send + 'static,
    ) -> Promessa<T> {
        let servico = Arc::clone(self.servico);
        let id = self.banco_id();
        Promessa::rodar(ctx, move || f(&servico, &id))
    }

    /// Decide o que fazer com um erro de tarefa: `Substituido` e `BancoTrocado` são ignorados (o segundo avisa
    /// a casca, que relê o estado e redesenha); o resto a tela mostra.
    pub fn reagir(&mut self, erro: &ErroBanco) -> Reacao {
        if erro.tem_codigo(Codigo::Substituido) {
            return Reacao::Ignorar;
        }
        if erro.tem_codigo(Codigo::BancoTrocado) {
            self.emitir(Acao::BancoTrocado);
            return Reacao::Ignorar;
        }
        Reacao::Mostrar
    }

    /// A frase do toast de uma gravação: a de sempre, ou [`MSG_SO_NA_TRANSACAO`] quando ela ficou numa
    /// transação aberta. Também avisa a casca do estado da transação.
    pub fn aviso_de_gravacao(&mut self, transacao_aberta: bool, frase: &str) -> String {
        self.emitir(Acao::NotarTransacao(transacao_aberta));
        if transacao_aberta { MSG_SO_NA_TRANSACAO.to_string() } else { frase.to_string() }
    }

    /// Os diálogos do Windows (abrir, salvar, escolher arquivo), donos da janela do aplicativo.
    pub fn dialogos(&self) -> Dialogos<'_> {
        let pasta_banco = self.estado.banco.as_ref().map(|b| b.pasta.clone());
        Dialogos::novo(self.frame, self.estado.ultimo_diretorio.clone().or(pasta_banco))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[cfg(windows)]
    use comum::arquivos::{Entrada, Natureza, TipoDeArquivo};

    #[cfg(windows)]
    fn entrada(pasta: &Path, nome: &str, natureza: Natureza) -> Entrada {
        let caminho = pasta.join(nome);
        Entrada {
            nome: nome.to_string(),
            chave: prefs::chave_caminho(&caminho),
            caminho,
            natureza,
            tamanho: None,
            modificado: None,
        }
    }

    #[test]
    fn contexto_fontes_comeca_vazio_e_aceita_a_lista() {
        use crate::fonte::{FonteFalsa, TipoDeFonte};
        let amb = crate::apoio_testes::Ambiente::vazio("contexto_fontes");
        let mut acoes = Vec::new();
        // sem `com_fontes` (os palcos e as telas que não usam), a lista é vazia
        let c = Contexto::novo(&amb.servico, &amb.estado, None, &mut acoes);
        assert!(c.fontes().is_empty());
        // com a lista da casca: na ordem das abas, e `com_casca` não a perde
        let lista: Vec<Arc<dyn FonteDeColunas>> = vec![
            Arc::new(FonteFalsa { rotulo: "vendas.csv".into(), tipo: TipoDeFonte::Csv, linhas: 4 }),
            Arc::new(FonteFalsa { rotulo: "estoque.csv".into(), tipo: TipoDeFonte::Csv, linhas: 9 }),
        ];
        let c = Contexto::novo(&amb.servico, &amb.estado, None, &mut acoes).com_fontes(&lista).com_casca(7, false);
        let rotulos: Vec<String> = c.fontes().iter().map(|f| f.rotulo()).collect();
        assert_eq!(rotulos, ["vendas.csv", "estoque.csv"]);
        assert_eq!(c.fontes()[1].linhas(), Some(9));
        assert_eq!(c.versao, 7);
    }

    #[test]
    #[cfg(not(windows))]
    fn pasta_aberta_contem_o_caminho_dentro_dela_fora_do_windows() {
        let pasta = PastaAberta::nova("/dados/Projeto");
        assert_eq!((pasta.nome.as_str(), pasta.chave.as_str()), ("Projeto", "/dados/Projeto"));
        assert!(pasta.contem(Path::new("/dados/Projeto/vendas.db")));
        assert!(pasta.contem(Path::new("/dados/Projeto/sub/fundo/a.csv")), "em qualquer profundidade");
        assert!(pasta.contem(Path::new("/dados/Projeto/sub/../a.csv")), "o caminho é normalizado");
        assert!(pasta.contem(Path::new("/dados/Projeto")), "a própria raiz conta");
        assert!(pasta.contem(Path::new("/dados/Projeto/")));
        assert!(!pasta.contem(Path::new("/dados/projeto/a.csv")), "a caixa diferencia");
        assert!(!pasta.contem(Path::new("/dados/Projeto2/a.csv")));
        assert!(!pasta.contem(Path::new("/dados/Projeto/../a.csv")));
        assert!(!pasta.contem(Path::new("/dados")));
        let raiz = PastaAberta::nova("/");
        assert!(raiz.contem(Path::new("/a.csv")) && raiz.contem(Path::new("/x/y.db")));
    }

    #[test]
    #[cfg(windows)]
    fn pasta_aberta_contem_o_caminho_dentro_dela() {
        let pasta = PastaAberta::nova(r"C:\Dados\Projeto");
        assert_eq!((pasta.nome.as_str(), pasta.chave.as_str()), ("Projeto", r"c:\dados\projeto"));
        assert!(pasta.contem(Path::new(r"C:\Dados\Projeto\vendas.db")));
        assert!(pasta.contem(Path::new(r"C:\Dados\Projeto\sub\fundo\a.csv")), "em qualquer profundidade");
        assert!(pasta.contem(Path::new("c:/DADOS/projeto/Vendas.DB")), "sem diferença de caixa nem de barra");
        assert!(pasta.contem(Path::new(r"C:\Dados\Projeto\sub\..\a.csv")), "o caminho é normalizado");
        assert!(pasta.contem(Path::new(r"C:\Dados\Projeto")), "a própria raiz conta");
        assert!(pasta.contem(Path::new(r"C:\Dados\Projeto\")));
        // só por pastas inteiras, e nada de fora
        assert!(!pasta.contem(Path::new(r"C:\Dados\Projeto2\a.csv")));
        assert!(!pasta.contem(Path::new(r"C:\Dados\ProjetoVelho.db")));
        assert!(!pasta.contem(Path::new(r"C:\Dados\outro.db")));
        assert!(!pasta.contem(Path::new(r"C:\Dados")));
        assert!(!pasta.contem(Path::new(r"D:\Dados\Projeto\a.csv")));
        assert!(!pasta.contem(Path::new(r"C:\Dados\Projeto\..\a.csv")));
        // a raiz de um disco: o nome é o próprio caminho e tudo no disco está dentro
        let disco = PastaAberta::nova(r"C:\");
        assert_eq!(disco.nome, r"C:\");
        assert!(disco.contem(Path::new(r"C:\a.csv")) && disco.contem(Path::new(r"C:\x\y.db")));
        assert!(!disco.contem(Path::new(r"D:\a.csv")));
    }

    #[test]
    #[cfg(windows)]
    fn pasta_aberta_guarda_cada_no_pela_chave() {
        let mut pasta = PastaAberta::nova(r"C:\Dados\Projeto");
        let raiz = pasta.chave.clone();
        assert_eq!(pasta.nos.get(&raiz), Some(&NoDaPasta::Lendo), "a raiz nasce lendo");
        assert!(pasta.listagem(&raiz).is_none(), "ainda sem listagem");
        assert!(pasta.listagem("outra").is_none());

        let sub = entrada(&pasta.caminho, "Sub", Natureza::Pasta);
        let csv = entrada(&pasta.caminho, "vendas.csv", Natureza::Dados(TipoDeArquivo::Csv));
        let listagem = Listagem::montar(pasta.caminho.clone(), vec![csv.clone(), sub.clone()], 2, false, 500);
        pasta.nos.insert(raiz.clone(), NoDaPasta::Pronto(Arc::new(listagem)));
        let lida = pasta.listagem(&raiz).expect("a raiz está pronta");
        assert_eq!(lida.entradas.iter().map(|e| e.nome.as_str()).collect::<Vec<_>>(), ["Sub", "vendas.csv"]);
        assert_eq!(lida.ocultas, 2);
        // cada nível tem o seu: a subpasta é achada pela chave da entrada dela, sem misturar com a raiz
        pasta.nos.insert(sub.chave.clone(), NoDaPasta::Lendo);
        assert!(pasta.listagem(&sub.chave).is_none());
        pasta.nos.insert(sub.chave.clone(), NoDaPasta::Falhou("Não foi possível ler a pasta.".into()));
        assert!(pasta.listagem(&sub.chave).is_none(), "falhou: não há lista");
        assert_eq!(pasta.nos.get(&sub.chave), Some(&NoDaPasta::Falhou("Não foi possível ler a pasta.".into())));
        let interna =
            Listagem::montar(sub.caminho.clone(), vec![entrada(&sub.caminho, "a.txt", Natureza::Banco)], 0, true, 500);
        pasta.nos.insert(sub.chave.clone(), NoDaPasta::Pronto(Arc::new(interna)));
        assert_eq!(pasta.listagem(&sub.chave).map(|l| (l.entradas.len(), l.parcial)), Some((1, true)));
        assert_eq!(pasta.listagem(&raiz).map(|l| l.entradas.len()), Some(2), "a raiz continua como estava");
        assert_eq!(pasta.nos.len(), 2);
        // a mesma subpasta com outra grafia tem a mesma chave
        assert_eq!(prefs::chave_caminho(r"c:/dados/PROJETO/sub"), sub.chave);
        // clonar o estado não copia as listas (é a mesma Arc)
        let copia = pasta.clone();
        assert_eq!(copia, pasta);
    }
}
