//! O que uma tela pede à casca: navegar, abrir/fechar banco, recarregar o estado, abrir uma janela global.
//! As telas não mexem no estado da casca: emitem [`Acao`] pelo [`crate::Contexto`] e a casca aplica no fim do
//! quadro.

use std::path::PathBuf;

use banco::{Direcao, Filtro};
use comum::Chave;

use crate::abas::IdAba;
use crate::rota::Rota;

/// Pedido de uma tela à casca. A casca sabe de que aba veio cada ação: o `Substituir` e o `Atualizar` valem para a
/// aba que os emitiu, mesmo que ela já tenha saído de vista.
#[derive(Debug, Clone, PartialEq)]
pub enum Acao {
    /// Vai para outra tela: a casca abre a aba dessa rota ou foca a que já está aberta (sem refazê-la nem trocar o
    /// endereço dela), e o foco entra no histórico. A Consulta SQL sem comando foca a que abriu a transação, depois
    /// a usada mais recentemente; com um comando, abre sempre outra.
    Navegar(Rota),
    /// A tela já se atualizou e registra o novo endereço como uma entrada nova do histórico, na mesma aba, sem ser
    /// refeita. Voltar a essa entrada depois só foca a aba.
    Atualizar(Rota),
    /// Troca o endereço da aba sem histórico e sem refazer a tela: é o que a
    /// aba Dados (página, busca, filtros, ordem) e a Consulta SQL (o texto do editor) usam.
    Substituir(Rota),
    /// Foca a aba da entrada anterior do histórico único (que atravessa as abas), sem refazer nada.
    Voltar,
    /// Foca a aba da entrada seguinte do histórico.
    Avancar,
    /// Abre uma Consulta SQL nova (o "Nova consulta" da barra de ferramentas e da faixa de abas), com o comando no
    /// editor quando vem um (um item do Histórico de consultas).
    NovaConsulta { sql: Option<String> },
    /// Foca uma aba aberta (o clique na faixa de abas).
    FocarAba(IdAba),
    /// Fecha uma aba (o × dela, o clique do meio, Ctrl+W); None é a ativa.
    FecharAba(Option<IdAba>),
    /// Muda a aba de lugar na faixa (o arrasto, o Ctrl+Shift+PageUp e PageDown, o menu da aba), sem focá-la.
    MoverAba { id: IdAba, para: usize },

    /// Abre um banco (com transação aberta no atual, a casca pergunta se descarta antes).
    AbrirBanco { caminho: PathBuf, somente_leitura: bool },
    /// Diálogo do Windows para escolher o banco e abrir.
    EscolherEAbrir { somente_leitura: bool },
    /// Diálogo de salvar para criar um banco novo e abrir.
    NovoBanco,
    /// Fecha o banco aberto (pergunta antes com transação aberta) e volta ao Início.
    FecharBanco,

    /// Diálogo do Windows (ou a janela "Pedir o caminho", sem ele) para escolher uma pasta e abri-la: o Arquivo ›
    /// Abrir pasta, o Ctrl+Shift+O, o botão do Explorador e o cartão do Início. A casca lembra a pasta **de cima**
    /// da escolhida para o próximo diálogo ([`crate::Escolha`]) e emite [`Acao::AbrirPasta`].
    EscolherEAbrirPasta,
    /// Abre uma pasta: um recente, um nó da árvore, algo solto na janela, a linha de comando. A casca confere e lê a
    /// raiz numa tarefa (`Servico::abrir_pasta`), põe a pasta em [`crate::Estado::pasta`] (recentes, `pasta_aberta`
    /// e o título da janela) e relê a lista dela quando o foco volta. Erro: o toast, ou o Início quando veio dele.
    AbrirPasta { caminho: PathBuf },
    /// Abre qualquer caminho sem a tela saber o que ele é: a casca pergunta ao `Servico::alvo_do_caminho` e vira
    /// [`Acao::AbrirPasta`], [`Acao::AbrirBanco`] (com o `somente_leitura`) ou [`Acao::AbrirArquivoDeDados`]. O campo
    /// do Início, o Ctrl+O, a linha de comando e soltar na janela usam esta. Tipo que o app não abre: o toast "O
    /// DataBox ainda não abre este tipo de arquivo: x.pdf." (ou o erro do Início, quando veio dele).
    AbrirCaminho { caminho: PathBuf, somente_leitura: bool },
    /// Abre a ficha de um arquivo de dados: a casca registra o recente (tipo arquivo, `Servico::registrar_arquivo`) e
    /// navega para [`Rota::Arquivo`], que abre a aba ou foca a que já está aberta. Não precisa de banco.
    AbrirArquivoDeDados { caminho: PathBuf },
    /// Lê uma subpasta da árvore (a primeira vez que abre, o "Tentar de novo"): o resultado volta para
    /// [`crate::PastaAberta::nos`], com `Lendo` até chegar. A casca descarta a leitura se a pasta aberta mudou nesse
    /// meio-tempo.
    ListarPasta { caminho: PathBuf },
    /// Relê a raiz e as pastas abertas da árvore (o "Atualizar" da pasta, a volta do foco da janela, o fim de uma troca
    /// de banco ou de uma janela que cria arquivos). Sem pasta aberta, não faz nada.
    AtualizarPasta,
    /// Fecha a pasta: tira a árvore, apaga a `pasta_aberta` das preferências e fecha as abas de arquivo de dados de
    /// dentro dela (as de fora ficam). O banco aberto continua aberto. Os recentes ficam.
    FecharPasta,

    /// Relê a lista de tabelas, visões, índices e gatilhos (a lateral e a visão geral redesenham).
    RecarregarObjetos,
    /// Relê o BancoInfo (tamanho, transação aberta…).
    RecarregarBanco,
    /// Relê as preferências (recentes, tema, opções de exportação).
    RecarregarPrefs,
    /// O servidor disse se há BEGIN aberto (resposta de gravação ou da Consulta SQL); a casca relê o banco se mudou.
    NotarTransacao(bool),
    /// Uma tarefa recebeu `Codigo::BancoTrocado`: a casca relê o estado, fecha as janelas e redesenha.
    BancoTrocado,
    /// Guarda a pasta usada num diálogo (as preferências lembram a última).
    LembrarDiretorio(PathBuf),
    /// Troca o tema e grava nas preferências.
    TrocarTema(prefs::Tema),
    /// Põe um painel da casca à vista onde ele mora (a doca, a janela própria): os botões "Painéis" do Início.
    MostrarPainel(PainelCasca),
    /// Leva um painel da casca para uma janela própria (o Tabela › Propriedades em janela própria).
    DestacarPainel(PainelCasca),
    /// Abre a janela Preferências (o botão "Preferências" do Início).
    AbrirPreferencias,
    /// Uma manutenção do banco aberto pedida pelo menu Banco (Verificar integridade, Verificar chaves estrangeiras,
    /// Otimizar, Compactar): a casca põe a Visão geral à vista e ela executa, como no botão das Propriedades dela (o
    /// Compactar pergunta antes).
    ManutencaoDoBanco(banco::AcaoManutencao),
    /// A cópia de segurança pedida pelo menu Banco: a Visão geral à vista, e o diálogo de salvar dela.
    CopiaDeSeguranca,

    /// Abre a janela Exportar.
    AbrirExportacao(PedidoJanelaExportar),
    /// Abre a janela Nova tabela.
    AbrirNovaTabela,
    /// Abre a janela Novo índice da tabela, com estas colunas já marcadas (a sugestão de índice da aba Dados).
    AbrirNovoIndice { tabela: String, colunas: Vec<String> },
    /// O diálogo "Salvar banco SQLite" da cópia de segurança, na pasta do banco aberto, com este nome ("vendas
    /// (cópia).db"). O caminho escolhido volta à Visão geral, que faz a cópia; sem o diálogo do Windows, a casca abre
    /// a janela "Pedir o caminho".
    EscolherCopia { nome_sugerido: String },
}

/// Um painel da casca (o Explorador, o Histórico de consultas, as Propriedades), para as telas pedirem que ele
/// apareça.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PainelCasca {
    Explorador,
    Historico,
    Propriedades,
}

/// O que a janela Exportar recebe de quem a abre (o `abrirExportacao({origem, nomeSugerido, contagens})`).
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoJanelaExportar {
    pub origem: OrigemJanelaExportar,
    /// Nome sugerido no diálogo de salvar, sem extensão ("clientes", "vendas", "resultado").
    pub nome_sugerido: String,
}

/// De onde vêm as linhas a exportar.
#[derive(Debug, Clone, PartialEq)]
pub enum OrigemJanelaExportar {
    /// Uma tabela ou visão. A janela oferece "A tabela inteira (N linhas)", "Só o que a busca e os filtros
    /// mostram (M linhas)" (com `filtro`) e "Só as linhas marcadas (K)" (com `marcadas`).
    Tabela {
        nome: String,
        /// Linhas da tabela inteira, se conhecidas (TabelaInfo.linhas).
        total: Option<u64>,
        /// A busca, os filtros e a ordem que a aba Dados mostra, se houver busca ou filtro.
        filtro: Option<FiltroAtual>,
        /// Chaves das linhas marcadas na aba Dados (vazio = nenhuma).
        marcadas: Vec<Chave>,
    },
    /// O resultado de um comando da Consulta SQL.
    Consulta { sql: String },
    /// O banco inteiro (a janela lista as tabelas para escolher).
    Banco,
}

/// A busca e os filtros que a aba Dados está mostrando.
#[derive(Debug, Clone, PartialEq)]
pub struct FiltroAtual {
    pub busca: Option<String>,
    pub filtros: Vec<Filtro>,
    pub ordem: Option<String>,
    pub direcao: Direcao,
    /// Quantas linhas a busca e os filtros acham, se a aba Dados já contou (numa tabela grande ainda sem
    /// contagem, None: a janela abre na hora e a opção fica sem o número).
    pub linhas: Option<u64>,
}
