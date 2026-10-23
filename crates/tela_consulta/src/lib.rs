//! Consulta SQL (`#/sql`) no formato de documento (prancheta 3,
//! `design/canvas/Consulta.dc.html`).
//!
//! # API (o contrato com a casca, §4.8 de ARQUITETURA.md)
//!
//! ```text
//! TelaConsulta::nova(sql: Option<String>) -> TelaConsulta      // sql = o ?sql= da rota (pré-preenche o editor)
//! TelaConsulta::com_id(self, sal: u64) -> TelaConsulta         // ids próprios (o id da aba)
//! TelaConsulta::ui(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto)   // só o conteúdo (sem faixa nem rodapé)
//! TelaConsulta::receber_em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto)  // fora de vista
//! TelaConsulta::esquecer_transacao(&mut self)                  // outra Consulta mudou a transação
//! TelaConsulta::executar_agora(&mut self)                      // o Executar da barra: como o Ctrl+Enter
//! TelaConsulta::executando(&self) -> bool                       // um roteiro, o COMMIT ou o ROLLBACK em andamento
//! TelaConsulta::soltar_execucao(&mut self) -> Option<Promessa<ResultadoConsulta>>
//! TelaConsulta::interromper(&mut self, egui: &egui::Context, ctx: &mut Contexto)   // o Interromper (e o Esc)
//! impl estado::Documento for TelaConsulta   // anatomia (faixa, rodapé, rolagem própria) · avisos (a transação) ·
//!     faixa (o banco) · conteudo · rodape (linhas e tempo; a posição do cursor) · ofertas (Executar principal,
//!     Exportar, Buscar) · executar · busca ("Buscar no editor") · propriedades ("Execução" e "Tabelas") · status
//!     ("SQL") · em_segundo_plano (o receber)
//! ```
//!
//! **Várias Consultas abertas.** Cada aba cria a sua com [`TelaConsulta::com_id`] (o id da aba como sal): o texto, o
//! cursor, a seleção e o desfazer do editor, os resultados, as grades e as rolagens de uma não passam para a outra.
//! Sem o sal, os ids são os de sempre (`Id::new("consulta::editor")`, o [`TelaConsulta::id_editor`]); o de cada
//! instância é o [`TelaConsulta::id_do_editor`]. Todas falam com a mesma conexão: um `BEGIN` numa vale para as
//! outras.
//!
//! **Fora de vista.** A casca só desenha a aba ativa e, a cada quadro, chama
//! [`TelaConsulta::receber_em_segundo_plano`] nas Consultas escondidas: ela só toma as respostas prontas (a sondagem
//! e a execução), sem desenhar nada; parada, não pede quadro. A resposta de uma execução que seguiu numa aba
//! escondida emite `Acao::NotarTransacao` e (com dados ou estrutura mudados) `Acao::RecarregarObjetos` na hora, e a
//! casca relê também a aba à vista; os resultados ficam prontos para quando a aba voltar. Quando outra Consulta muda a
//! transação, a casca chama [`TelaConsulta::esquecer_transacao`] nas demais: a faixa da transação volta a seguir o
//! `BancoInfo` do estado, e não a última resposta desta tela.
//!
//! **Executar pela casca.** [`TelaConsulta::executar_agora`] é o Executar da barra de ferramentas: executa no
//! próximo conteúdo, como o Ctrl+Enter e com a mesma regra (com seleção, só o trecho; com a confirmação ou uma
//! janela aberta, o pedido se perde). A seleção é a do `TextEditState` do editor, e o egui 0.36 a reduz ao cursor no
//! primeiro quadro em que desenha o editor sem o foco (`owns_ime_events`, em `widgets/text_edit/builder.rs`): ela só
//! chega ao pedido se nenhum quadro desenhar o editor entre a perda do foco e o pedido. Pelo mesmo motivo, o trecho
//! que a busca da barra acha é pintado ([`interface::pecas::campos::AreaTexto::destacar`]) e não fica como seleção:
//! o foco está na busca.
//!
//! **Fechar a aba no meio de uma execução** (ou trocar de banco). Antes de soltar a `TelaConsulta` com
//! [`TelaConsulta::executando`], a casca chama [`TelaConsulta::soltar_execucao`] e guarda a promessa. Quando ela
//! chegar com `Ok(r)`, a casca emite `Acao::NotarTransacao(r.transacao_aberta)` e, quando o comando mudou dados ou
//! estrutura (ou fechou a transação), `Acao::RecarregarObjetos`; com `Err(e)`, só `ctx.reagir(&e)`. Trocar de aba
//! não solta nada: a Consulta escondida continua e recebe a resposta em segundo plano.
//!
//! Também públicas: as regras de texto ([`resumo_comando`],
//! [`nome_para_exportar`], [`inserir_no_cursor`]) e as constantes [`LIMITE`], [`MAX_SQL_NO_ENDERECO`],
//! [`ALTURA_EDITOR`], [`ALTURA_MIN_EDITOR`], [`DICA_BUSCA`], [`DICA_EDITOR`], [`PARA_COMECAR`], [`NADA_EXECUTADO`],
//! [`DICA_EXECUTAR`], [`DICA_EXECUTANDO`] e [`DICA_INTERROMPER`]. Para os testes (`#[doc(hidden)]`):
//! [`TelaConsulta::id_editor`] e [`TelaConsulta::id_do_editor`], [`TelaConsulta::texto`],
//! [`TelaConsulta::execucoes`] (quantas respostas já foram mostradas), [`TelaConsulta::area_editor`],
//! [`TelaConsulta::tipos_dos_cards`], [`TelaConsulta::escolhido`], [`TelaConsulta::grade`] e
//! [`TelaConsulta::transacao_aberta`].
//!
//! # O que a tela faz
//!
//! - **Faixa** (42px): o cilindro de 16px e o nome do banco em `--t2` ("Linha 9, coluna 25", do cursor do editor,
//!   fica na ponta direita do rodapé, em `--t3`, desde a U4 da revisão de UI de 22/10/2026). Antes dela, com a transação aberta, o `.aviso` de atenção de ponta a ponta com "Há
//!   uma transação aberta. As alterações só gravam no arquivo com COMMIT." e "Gravar (COMMIT)" / "Descartar
//!   (ROLLBACK)" (este pergunta antes, na confirmação "Descartar a transação"); o fim mostra "Transação gravada." /
//!   "Transação descartada." e recarrega os objetos. Erro do COMMIT/ROLLBACK vai para o toast de erro.
//! - **Editor** (232px; a borda de baixo se arrasta, com mínimo de 96px): o [`AreaTexto::codigo`] plano, encostado
//!   no documento, com a calha de 46px dos números, as cores do código das Preferências (`interface::codigo`), a
//!   linha do cursor em `--selh` e, depois de um erro, as linhas do comando que falhou em `--perigo` a 9% e o
//!   sublinhado ondulado no nome que a frase cita ("A tabela não existe: X.") quando ele aparece no comando (o melhor
//!   esforço). As marcas somem quando o texto muda. Tab recua, Shift+Tab tira o recuo, depois de um Esc o Tab sai.
//! - **Consulta vazia** (prancheta 15, revisão de UI de 19/10/2026): com o editor sem texto, por cima dele, a dica
//!   [`DICA_EDITOR`] em itálico, na fonte da interface e em `--t3` (para não ser confundida com código), e, 14px
//!   abaixo, o `.tit` "Para começar" e as sugestões (o `a.atalho` com a largura do texto, numa fileira que quebra):
//!   "SELECT * FROM clientes LIMIT 100", "SELECT count(*) FROM clientes" e "PRAGMA table_info(clientes)", com a
//!   primeira tabela do banco citada (sem tabelas, "SELECT sqlite_version()" e um CREATE TABLE). O clique insere o SQL
//!   no editor, com o cursor no fim e o foco nele. O Executar da barra fica desligado até haver texto, e o rodapé diz
//!   [`NADA_EXECUTADO`].
//! - **Executando** (N3, prancheta 15): o Executar da barra fica desligado, com o arco girando no lugar do triângulo e
//!   a dica "Executando"; o Interromper ao lado dele (`.ib.risco`, [`DICA_INTERROMPER`]), que tem o lugar dele na
//!   barra em toda aba, liga e fica vermelho; o rodapé diz
//!   "Executando · 1,4 s", contado ao vivo, e pede um quadro a cada 250 ms só enquanto executa (o arco do Executar,
//!   que gira no mesmo tempo, pede um a cada ~33 ms: o ritmo real da tela executando é o do arco, ~30 quadros por
//!   segundo; os 250 ms são o piso do rodapé quando o arco não gira, com o movimento reduzido); o Ctrl+Enter é
//!   ignorado. O Interromper e o Esc (sem janela aberta) chamam `Servico::interromper_sql_detalhado` numa tarefa de
//!   fundo (nunca na thread da interface); se a consulta ainda espera a vez na conexão (uma gravação ou uma
//!   exportação a segura), ela já fica marcada para sair sem executar e o toast de atenção diz
//!   [`MSG_ESPERANDO_O_BANCO`]. O comando cortado volta como o erro "A consulta foi interrompida." (a faixa de erro e o
//!   item "Comando 2 · interrompido" do seletor, sem o original do SQLite em inglês), os anteriores ficam e os
//!   seguintes não rodam. Dentro de um BEGIN, a faixa da transação segue a resposta (`transacao_aberta`): interromper
//!   um SELECT não desfaz a transação. Só o roteiro do editor é interrompível; o COMMIT e o ROLLBACK não.
//! - **Ctrl+Enter** executa (também com o foco fora do editor, desde que nenhuma janela esteja aberta); com
//!   seleção, só o trecho selecionado. Texto vazio (o Executar da barra já fica desligado): toast "Escreva um comando
//!   para executar.".
//! - O texto do editor vai para o endereço (`Acao::Substituir(Rota::Consulta{sql})`) 400 ms depois de parar de
//!   digitar e na hora de executar; acima de 50.000 caracteres, o endereço fica sem o texto.
//! - A execução roda em tarefa de fundo (`servico.sql(id, trecho, 1000)`), com o Executar da barra desligado. A
//!   resposta vira os resultados, montados uma vez (textos e grades prontos):
//!   - a **faixa de erro** (item 23): o sólido de erro, "Comando 3: {mensagem}" em `--perigo` (o original do SQLite
//!     no "i", só quando a frase não o cita) e, depois do primeiro comando, "A execução parou neste comando; os
//!     anteriores já foram executados." em `--t2`;
//!   - o **seletor** `.seg` (item 24) numa faixa: "Comando 1 · 8 linhas", "Comando 2 · 3 linhas
//!     alteradas", "Comando 3 · erro" (em `--perigo`), de partes iguais, rolando na horizontal com muitos; até 100
//!     comandos ([`resultados::MAX_CARDS`]) e o item "Mais 6 comandos" ("Mais 6 comandos executados, que não aparecem
//!     aqui.") e o erro do fim. Ao chegar, a tela mostra o primeiro com linhas (sem nenhum, o último);
//!   - embaixo, **um resultado por vez**: a grade só leitura de ponta a ponta, o vazio "A consulta não devolveu
//!     linhas.", ou o aviso da alteração ("3 linhas alteradas · 0,6 ms" e o comando) ou do erro, numa caixa centrada.
//!   - Um aviso sozinho, sem seletor: o fim da transação, "Não havia comando para executar…" e a falha da execução
//!     (esta, na faixa de erro).
//! - **Menu da célula** (0.2.3): o clique direito e o Shift+F10 na grade do resultado: Copiar, Copiar com cabeçalho e
//!   Copiar como (o `INSERT` com a tabela do primeiro FROM, ou "resultado"). Com mais de uma célula, as contas da
//!   seleção vão à direita do rodapé, pelo valor de cada célula.
//! - **Rodapé** (34px): o que o resultado escolhido diz ("8 linhas · 0,5 ms", "mais de 1.000 linhas · … · mostrando
//!   as primeiras 1.000", "3 linhas alteradas · 0,6 ms").
//! - **Barra**: o Executar principal ([`DICA_EXECUTAR`]); o Exportar "Exportar o resultado do comando 1" (desligado
//!   sem um resultado com linhas escolhido), que emite `Acao::AbrirExportacao` com a origem consulta e o nome
//!   "consulta_<tabela>"; a busca "Buscar no editor" (cada letra acha a primeira ocorrência a partir da achada, sem
//!   caixa nem acento, e o Enter vai à seguinte, com volta; o trecho fica pintado no fundo da seleção e o editor rola
//!   até ele).
//! - **Propriedades**: "Execução" (Comandos · Executados · Parou em, "Comando 3" em `--perigo` · Transação, "aberta"
//!   / "nenhuma aberta") e "Tabelas" (os nós com a ficha e o ↵ que inserem o nome citado no cursor, com um espaço
//!   antes quando precisa; o banco sem tabelas diz "O banco ainda não tem tabelas; crie a primeira com CREATE
//!   TABLE.").
//! - Depois de cada resposta: `Acao::NotarTransacao(transacao_aberta)` (a casca relê também o Histórico de
//!   consultas) e `Acao::RecarregarObjetos` quando `alterou_esquema` ou `alterou_dados`.
//! - Erros de tarefa passam por `ctx.reagir` (banco trocado e leitura substituída ficam em silêncio).
//!
//! # Leveza
//!
//! Os resultados guardam os textos e a [`interface::grade::GradeSomenteLeitura`] (rolagem virtual) prontos; a cada
//! quadro só se desenha. A posição do cursor é refeita só quando o cursor ou o texto mudam. Sem repintura contínua:
//! as tarefas acordam a interface ao terminar, e o adiamento do endereço pede um quadro só para quando vence.
//!
//! # Limites conhecidos
//!
//! - O desfazer (Ctrl+Z) não alcança o que as Tabelas das Propriedades inserem.
//! - Os resultados não se empilham em cards: um por vez, pelo seletor. O Histórico mora no painel da
//!   casca.
//! - A monoespaçada do editor e dos comandos sai na Consolas quando a Cascadia Mono só existe como fonte
//!   compartilhada de um pacote (a do Windows Terminal, em `C:\Program Files\WindowsApps`): o
//!   `interface::fontes` só procura nas pastas de fontes.
//!
//! # Testes
//!
//! `src/lib.rs` (os ids sem e com o sal), `src/texto.rs` e `src/resultados.rs` (as regras de texto e a montagem
//! dos resultados) e `tests/consulta.rs`, no `egui_kittest` com o banco de
//! exemplo, pelo `palco_documento` (a tela desenhada como a casca: a barra com os verbos e a busca, o documento com a
//! faixa e o rodapé e as Propriedades à direita), que grava em `capturas/rust/` as capturas `consulta-*`. Fora
//! daqui: a exportação do resultado até o arquivo (em `tela_exportar/tests/exportar.rs`; este crate só confere o
//! `Acao::AbrirExportacao`), o texto do balão do "i" (não entra na árvore de acessibilidade) e o recarregar da
//! página (o endereço é conferido pela `Acao::Substituir`).

#![forbid(unsafe_code)]

mod resultados;
mod texto;

use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::time::Duration;

use banco::{Interrupcao, ResultadoComando, ResultadoConsulta};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{
    Align, CornerRadius, CursorIcon, Event, Id, Key, Layout, Margin, Rect, Sense, Stroke, Ui, UiBuilder, Vec2, pos2,
    vec2,
};
use estado::{Acao, Contexto, Promessa, Reacao, Rota};
use interface::codigo::citar as citar_nome;
use interface::fontes::{self, Peso};
use interface::formato::{formatar_numero, formatar_segundos};
use interface::grade::rodape_da_selecao;
use interface::icones::{self, EstadoIcone, Icone};
use interface::pecas::arranjo::em_linha;
use interface::pecas::arvore::No;
use interface::pecas::avisos::TomAviso;
use interface::pecas::botao::Botao;
use interface::pecas::campos::{AreaTexto, LARGURA_CALHA};
use interface::pecas::documento::{ALTURA_FAIXA, Documento as DocumentoPeca};
use interface::pecas::janela::{Confirmacao, confirmar};
use interface::pecas::navegacao::{ParteSeletor, seletor};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::{Trecho, atalho_justo, galley_tit_em, paragrafo, texto, texto_rico};
use interface::pecas::toast::{toast, toast_atencao, toast_erro};
use interface::pintura::{self, Preenchimento};
use interface::tema::paleta;

pub use texto::{inserir_no_cursor, nome_para_exportar, resumo_comando};

use resultados::Card;

/// A dica (e o rótulo) da busca da barra na Consulta SQL.
pub const DICA_BUSCA: &str = "Buscar no editor";
/// A dica do Executar da barra de ferramentas (a seleção fica no editor: com texto selecionado, o botão também
/// executa só o trecho).
pub const DICA_EXECUTAR: &str = "Executar (Ctrl+Enter); com texto selecionado, executa só o trecho";
/// A dica do editor vazio (Consulta vazia, prancheta 15): em itálico, na fonte da interface, para não ser confundida
/// com código.
pub const DICA_EDITOR: &str = "Escreva SQL aqui. Ctrl+Enter executa; vários comandos separados por ponto e vírgula.";
/// O rodapé antes da primeira execução (em `--t3`).
pub const NADA_EXECUTADO: &str = "Nada executado ainda";
/// O título das sugestões do editor vazio (o `.tit` em caixa alta).
pub const PARA_COMECAR: &str = "Para começar";
/// A dica do Executar enquanto executa (N3), e a do Interromper que aparece ao lado dele.
pub const DICA_EXECUTANDO: &str = "Executando";
pub const DICA_INTERROMPER: &str = "Interromper o comando em execução (Esc)";
/// O rodapé conta o tempo ao vivo: o quadro se renova a cada 250 ms, só enquanto executa (com o arco do Executar
/// girando, o quadro já vem a cada ~33 ms; os 250 ms valem sozinhos com o movimento reduzido, em que o arco para).
const REPINTURA_EXECUTANDO: Duration = Duration::from_millis(250);
/// A dica do Exportar sem um resultado com linhas escolhido.
const DICA_EXPORTAR: &str = "Exportar o resultado (execute um SELECT primeiro)";
/// Linhas de cada resultado.
pub const LIMITE: usize = 1000;
/// Acima disto o texto não vai para o endereço.
pub const MAX_SQL_NO_ENDERECO: usize = 50_000;
/// Espera depois da última tecla para guardar o texto no endereço (o `debounce` de 400 ms).
const ESPERA_ENDERECO: f64 = 0.4;
/// Altura inicial do editor (prancheta 3: `height:232px`) e a mínima ao arrastar a borda de baixo
/// (`min-height:96px`).
pub const ALTURA_EDITOR: f32 = 232.0;
pub const ALTURA_MIN_EDITOR: f32 = 96.0;
/// O que fica abaixo do editor, no mínimo, quando a pessoa o alarga: a faixa do seletor e um pouco do resultado.
const SOBRA_ABAIXO_DO_EDITOR: f32 = ALTURA_FAIXA + 60.0;
/// A borda de baixo do editor que se arrasta: 3px para cada lado do fio.
const ALCA: f32 = 3.0;
/// `.faixa{padding:0 10px 0 14px}` da faixa do seletor.
const RECUO_FAIXA: (f32, f32) = (14.0, 10.0);

const MSG_ESCREVA: &str = "Escreva um comando para executar.";
/// O Interromper com a consulta ainda esperando a vez na conexão (o banco está com outra operação).
pub const MSG_ESPERANDO_O_BANCO: &str = "A consulta ainda espera o banco ficar livre.";
const MSG_TRANSACAO: &str = " As alterações só gravam no arquivo com COMMIT.";
const MSG_DESCARTAR: &str = "Descartar todas as alterações feitas desde o BEGIN? Nada delas vai para o arquivo, e \
                             isso não pode ser desfeito.";
const SEM_TABELAS: &str = "O banco ainda não tem tabelas; crie a primeira com CREATE TABLE.";
const FIM_GRAVADA: (&str, &str) = ("Transação gravada.", " As alterações feitas desde o BEGIN estão no arquivo.");
const FIM_DESCARTADA: (&str, &str) = ("Transação descartada.", " As alterações feitas desde o BEGIN foram desfeitas.");

/// O que está sendo executado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pedido {
    Executar,
    Gravar,
    Descartar,
}

/// O nome de um id da tela com o sal da instância ([`TelaConsulta::com_id`]). O hash é o do nome sozinho sem o
/// sal e o da tupla `(nome, sal)` com ele: sem o sal, os ids são os de sempre (`Id::new("consulta::editor")`).
#[derive(Debug, Clone, Copy)]
struct Chave {
    nome: &'static str,
    sal: Option<u64>,
}

impl Hash for Chave {
    fn hash<H: Hasher>(&self, estado: &mut H) {
        self.nome.hash(estado);
        if let Some(sal) = self.sal {
            sal.hash(estado);
        }
    }
}

/// O que a execução em andamento levou: o trecho executado, onde ele começa no editor (em caracteres) e quantos
/// comandos tem.
struct Execucao {
    pedido: Pedido,
    promessa: Promessa<ResultadoConsulta>,
    trecho: String,
    base: usize,
    comandos: usize,
    /// Quando começou (o relógio do egui), para o "Executando · 1,4 s" do rodapé.
    inicio: f64,
}

/// A seção "Execução" das Propriedades, da última resposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Contas {
    comandos: usize,
    executados: usize,
    parou_em: Option<usize>,
}

/// O que o editor marca depois de um erro: as linhas do comando e o nome sublinhado (em caracteres).
#[derive(Debug, Clone, PartialEq, Eq)]
struct MarcasErro {
    linhas: Range<usize>,
    sublinhado: Option<Range<usize>>,
}

/// A tela Consulta SQL.
pub struct TelaConsulta {
    /// O sal dos ids desta instância ([`TelaConsulta::com_id`]); None = os ids de sempre.
    sal: Option<u64>,
    sql: String,
    /// Quantas vezes o texto mudou (a posição do cursor e as marcas valem para uma versão).
    edicao: u64,
    iniciada: bool,
    /// O Executar da barra ([`TelaConsulta::executar_agora`]), atendido no próximo conteúdo.
    pedido_executar: bool,
    /// Quando o texto mudou pela última vez (para o endereço, 400 ms depois).
    editado_em: Option<f64>,
    execucao: Option<Execucao>,
    /// O pedido de interromper em andamento (`Servico::interromper_sql`, numa tarefa: ele nunca roda na thread da
    /// interface). `EmAndamento` cortou o comando, que volta pela resposta da execução; `NaFila` marcou o `sql()` que
    /// ainda espera a vez (ele sai interrompido, sem executar, quando o banco ficar livre: a tela avisa);
    /// `Nenhuma` diz que nenhum `sql()` existia ainda, e o botão pode ser clicado de novo.
    interrupcao: Option<Promessa<Interrupcao>>,
    /// A sondagem de partida (um roteiro vazio, que só diz se há transação aberta); some quando uma execução
    /// começa, porque a resposta dela vale mais.
    sondagem: Option<Promessa<bool>>,
    /// A altura do editor, que a pessoa muda pela borda de baixo, e onde ele ficou no último quadro.
    altura_editor: f32,
    area_editor: Rect,
    cards: Vec<Card>,
    /// O resultado à vista (no seletor).
    escolhido: usize,
    contas: Option<Contas>,
    marcas: Option<MarcasErro>,
    execucoes: u64,
    /// A transação aberta segundo a última resposta (antes dela, vale o `BancoInfo` do estado).
    transacao: Option<bool>,
    confirmacao: Option<Confirmacao>,
    /// (primeira tabela, sugestões do editor vazio), refeito só quando a primeira tabela muda.
    sugestoes: (Option<String>, Vec<String>),
    /// A posição do cursor no rodapé ("Linha 4, coluna 1"), com o cursor e a versão do texto de onde saiu.
    posicao: Option<((usize, u64), (usize, usize))>,
    /// Quantos caracteres tem o texto, com a versão e o tamanho em bytes de onde saiu (contar é O(n): só quando o
    /// texto muda, não a cada quadro).
    chars_do_texto: Option<(u64, usize, usize)>,
    /// A linha do cursor, com a versão, o tamanho em bytes e o cursor de onde saiu (O(n) também).
    linha_do_cursor: Option<((u64, usize, usize), usize)>,
    /// O texto da busca da barra ("Buscar no editor") e o trecho que ela achou no editor (em caracteres); `rolar`
    /// leva o editor até ele no próximo quadro.
    busca: String,
    achado: Option<(usize, usize)>,
    rolar_achado: bool,
}

impl TelaConsulta {
    pub fn nova(sql: Option<String>) -> TelaConsulta {
        TelaConsulta {
            sal: None,
            sql: sql.unwrap_or_default(),
            edicao: 0,
            iniciada: false,
            pedido_executar: false,
            editado_em: None,
            execucao: None,
            interrupcao: None,
            sondagem: None,
            altura_editor: ALTURA_EDITOR,
            area_editor: Rect::NOTHING,
            cards: Vec::new(),
            escolhido: 0,
            contas: None,
            marcas: None,
            execucoes: 0,
            transacao: None,
            confirmacao: None,
            sugestoes: (None, texto::sugestoes(None)),
            posicao: None,
            chars_do_texto: None,
            linha_do_cursor: None,
            busca: String::new(),
            achado: None,
            rolar_achado: false,
        }
    }

    /// Quantos caracteres tem o texto do editor, contados de novo só quando o texto muda (a versão e o tamanho em
    /// bytes dizem): o editor e a faixa perguntam a cada quadro.
    fn tamanho_do_texto(&mut self) -> usize {
        let chave = (self.edicao, self.sql.len());
        match self.chars_do_texto {
            Some((e, b, n)) if (e, b) == chave => n,
            _ => {
                let n = self.sql.chars().count();
                self.chars_do_texto = Some((chave.0, chave.1, n));
                n
            }
        }
    }

    /// A linha do cursor `cursor` (em caracteres), refeita só quando o texto ou o cursor mudam.
    fn linha_do_cursor(&mut self, cursor: usize) -> usize {
        let chave = (self.edicao, self.sql.len(), cursor);
        match self.linha_do_cursor {
            Some((c, linha)) if c == chave => linha,
            _ => {
                let linha = texto::linha_do_caractere(&self.sql, cursor);
                self.linha_do_cursor = Some((chave, linha));
                linha
            }
        }
    }

    /// Ids próprios para esta instância, com `sal` (o id da aba) em todos: o editor (texto, cursor, seleção e
    /// desfazer), o seletor, os resultados, as grades e as rolagens. Sem isso, duas Consultas abertas dividiriam
    /// esse estado (o Ctrl+Z de uma traria o texto da outra).
    pub fn com_id(mut self, sal: u64) -> Self {
        self.sal = Some(sal);
        self
    }

    /// O `TextEdit` do editor de SQL de uma tela sem [`TelaConsulta::com_id`].
    #[doc(hidden)]
    pub fn id_editor() -> Id {
        Id::new("consulta::editor")
    }

    /// O `TextEdit` do editor de SQL desta instância (o [`TelaConsulta::id_editor`] quando ela não tem sal).
    #[doc(hidden)]
    pub fn id_do_editor(&self) -> Id {
        self.id("consulta::editor")
    }

    /// O nome de um id com o sal desta instância, para as peças que recebem um `id_salt`.
    fn chave(&self, nome: &'static str) -> Chave {
        Chave { nome, sal: self.sal }
    }

    /// `Id::new(nome)` sem sal; `Id::new((nome, sal))` com ele.
    fn id(&self, nome: &'static str) -> Id {
        Id::new(self.chave(nome))
    }

    /// Para a casca, a cada quadro, com a aba desta Consulta fora de vista: toma as respostas que chegaram (a
    /// sondagem e a execução) como o conteúdo faria, com as mesmas ações (`NotarTransacao`, `RecarregarObjetos`,
    /// `BancoTrocado`), sem desenhar nada. Os resultados ficam prontos para quando a aba voltar.
    pub fn receber_em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        self.receber(egui, ctx);
    }

    /// Para a casca, quando outra Consulta mudou a transação (todas usam a mesma conexão): a faixa da transação
    /// volta a seguir o `BancoInfo` do estado, e não a última resposta desta tela.
    pub fn esquecer_transacao(&mut self) {
        self.transacao = None;
    }

    /// O Executar da barra de ferramentas: executa no próximo conteúdo, como o Ctrl+Enter (com seleção no editor,
    /// só o trecho selecionado; texto vazio, o toast). Com a confirmação ou uma janela aberta, o pedido se perde,
    /// como o atalho.
    pub fn executar_agora(&mut self) {
        self.pedido_executar = true;
    }

    /// O texto do editor.
    #[doc(hidden)]
    pub fn texto(&self) -> &str {
        &self.sql
    }

    /// Quantas respostas já foram mostradas.
    #[doc(hidden)]
    pub fn execucoes(&self) -> u64 {
        self.execucoes
    }

    /// Há uma execução em andamento (um roteiro, o COMMIT ou o ROLLBACK). Parte do contrato com a casca: com ela,
    /// antes de soltar a tela, [`TelaConsulta::soltar_execucao`].
    pub fn executando(&self) -> bool {
        self.execucao.is_some()
    }

    /// Para a casca, ao soltar a tela (trocar de rota) com [`TelaConsulta::executando`]: a execução que ainda
    /// roda. Sem a tela, a resposta se perderia, e com ela o estado da transação (um `BEGIN` demorado, um
    /// "Gravar (COMMIT)"): a casca guarda a promessa e, quando ela chegar com `Ok(r)`, emite
    /// `Acao::NotarTransacao(r.transacao_aberta)` e, se o comando mudou dados ou estrutura (ou fechou a transação),
    /// `Acao::RecarregarObjetos`; com `Err(e)`, só `ctx.reagir(&e)`. A tela fica sem a execução.
    pub fn soltar_execucao(&mut self) -> Option<Promessa<ResultadoConsulta>> {
        self.interrupcao = None;
        self.execucao.take().map(|e| e.promessa)
    }

    /// Há um roteiro do editor em andamento (não o COMMIT nem o ROLLBACK), que o Interromper alcança.
    fn interrompivel(&self) -> bool {
        matches!(&self.execucao, Some(e) if e.pedido == Pedido::Executar)
    }

    /// O Interromper (o botão da barra e o Esc): pede ao serviço, numa tarefa, que corte o comando em andamento
    /// (`Servico::interromper_sql`). O comando cortado volta pela resposta da execução como o erro "A consulta foi
    /// interrompida.", e os seguintes não rodam. Sem execução, ou com um pedido já a caminho, não faz nada.
    pub fn interromper(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        if !self.interrompivel() || self.interrupcao.is_some() {
            return;
        }
        self.interrupcao = Some(ctx.tarefa(egui, |servico, id| servico.interromper_sql_detalhado(id)));
    }

    /// Onde o editor ficou no último quadro (a altura é 232px até a pessoa arrastar a borda de baixo).
    #[doc(hidden)]
    pub fn area_editor(&self) -> Rect {
        self.area_editor
    }

    /// O tipo de cada resultado, na ordem ("linhas", "alteracao", "erro", "mais", "aviso").
    #[doc(hidden)]
    pub fn tipos_dos_cards(&self) -> Vec<&'static str> {
        self.cards.iter().map(Card::tipo).collect()
    }

    /// O resultado à vista no seletor.
    #[doc(hidden)]
    pub fn escolhido(&self) -> usize {
        self.escolhido
    }

    /// O que o editor marca depois de um erro: as linhas do comando (do 0) e o nome sublinhado (em caracteres).
    #[doc(hidden)]
    pub fn marcas_do_erro_no_editor(&self) -> Option<(Range<usize>, Option<Range<usize>>)> {
        self.marcas.as_ref().map(|m| (m.linhas.clone(), m.sublinhado.clone()))
    }

    /// O trecho que a busca da barra achou e o editor pinta (em caracteres).
    #[doc(hidden)]
    pub fn achado(&self) -> Option<Range<usize>> {
        self.achado.filter(|_| !self.busca.trim().is_empty()).map(|(a, b)| a..b)
    }

    /// A tela mostra a faixa da transação aberta.
    #[doc(hidden)]
    pub fn transacao_aberta(&self, ctx: &Contexto) -> bool {
        self.transacao.unwrap_or_else(|| ctx.estado.transacao_aberta())
    }

    /// As colunas e as linhas da grade do resultado `i` (None se ele não for um resultado com linhas).
    #[doc(hidden)]
    pub fn grade(&self, i: usize) -> Option<(Vec<&str>, &[Vec<comum::Celula>])> {
        match self.cards.get(i)? {
            Card::Linhas { grade: Some(g), .. } => {
                Some((g.colunas().iter().map(|c| c.nome.as_str()).collect(), g.linhas()))
            }
            _ => None,
        }
    }

    /// A tela fora da casca: só o conteúdo (o editor e os resultados), sem a faixa, o rodapé e as Propriedades.
    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.conteudo_da_tela(ui, ctx);
    }

    /// O pedido da janela Exportar do resultado à vista, com o número do comando.
    fn exportavel(&self) -> Option<(usize, &estado::PedidoJanelaExportar)> {
        let card = self.cards.get(self.escolhido)?;
        Some((card.numero()?, card.pedido()?))
    }

    // ---------------------------------------------------------------- conteúdo

    /// O conteúdo do documento (o `estado::Documento::conteudo`): o retângulo inteiro, que a tela divide entre o
    /// editor, a faixa de erro, o seletor e o resultado.
    fn conteudo_da_tela(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let egui_ctx = ui.ctx().clone();
        let agora = ui.input(|i| i.time);
        self.receber(&egui_ctx, ctx);
        if !self.iniciada {
            self.iniciada = true;
            // a partida da web: um roteiro vazio não executa nada nem entra no histórico, mas a resposta diz se
            // ficou uma transação aberta de antes (de uma execução cuja resposta a tela anterior não viu, ou que o
            // estado da casca ainda não sabe)
            if ctx.estado.tem_banco() {
                self.sondagem =
                    Some(ctx.tarefa(&egui_ctx, |servico, id| servico.sql(id, "", 1).map(|r| r.transacao_aberta)));
            }
            // o foco nasce no editor, com o cursor no fim
            colocar_cursor(&egui_ctx, self.id_do_editor(), self.sql.chars().count());
            egui_ctx.memory_mut(|m| m.request_focus(self.id_do_editor()));
        }
        // Ctrl+Enter também vale com o foco fora do editor, mas não com uma janela aberta; o Executar da barra
        // (`executar_agora`) segue a mesma regra, e o pedido não sobrevive a ela
        let pedido = std::mem::take(&mut self.pedido_executar);
        if self.confirmacao.is_none() && !janela_aberta(&egui_ctx) && (consumir_ctrl_enter(&egui_ctx) || pedido) {
            self.executar(&egui_ctx, ctx);
        }
        // o Esc interrompe a execução (nunca com uma janela aberta, que é dona do Esc)
        if self.interrompivel() && self.confirmacao.is_none() && !janela_aberta(&egui_ctx) && consumir_esc(&egui_ctx) {
            self.interromper(&egui_ctx, ctx);
        }
        if let Some(t) = self.editado_em {
            let falta = t + ESPERA_ENDERECO - agora;
            if falta <= 0.0 {
                self.guardar_no_endereco(ctx);
            } else {
                egui_ctx.request_repaint_after(Duration::from_secs_f64(falta));
            }
        }

        let area = ui.max_rect();
        let maximo = (area.height() - SOBRA_ABAIXO_DO_EDITOR).max(ALTURA_MIN_EDITOR);
        let altura = self.altura_editor.min(maximo);
        let rect_editor = Rect::from_min_size(area.min, vec2(area.width(), altura));
        self.editor(ui, rect_editor, agora, ctx);
        alca_do_editor(ui, self.id("consulta::editor::alca"), rect_editor, &mut self.altura_editor, maximo);

        let mut topo = rect_editor.bottom();
        if let Some(faixa) = resultados::faixa_erro(&self.cards) {
            let r = Rect::from_min_max(pos2(area.left(), topo), area.max);
            let mut filho = ui.new_child(
                UiBuilder::new().id_salt(self.chave("consulta::erro")).max_rect(r).layout(Layout::top_down(Align::Min)),
            );
            filho.set_clip_rect(r.intersect(ui.clip_rect()));
            topo = faixa_de_erro(&mut filho, &faixa).bottom();
        }
        let com_seletor = self.cards.iter().any(|c| c.numero().is_some() || matches!(c, Card::Mais { .. }));
        if com_seletor {
            let r = Rect::from_min_size(pos2(area.left(), topo), vec2(area.width(), ALTURA_FAIXA));
            if let Some(i) = self.seletor(ui, r) {
                self.escolhido = i;
            }
            topo = r.bottom();
        }
        let sozinho_de_erro = resultados::faixa_erro(&self.cards).is_some_and(|f| f.numero.is_none());
        if !self.cards.is_empty() && !sozinho_de_erro {
            let r = Rect::from_min_max(pos2(area.left(), topo), area.max);
            let id = Id::new((self.chave("consulta::resultado"), (self.execucoes, self.escolhido)));
            let mut filho = ui.new_child(UiBuilder::new().id_salt(id).max_rect(r).layout(Layout::top_down(Align::Min)));
            filho.set_clip_rect(r.intersect(ui.clip_rect()));
            let i = self.escolhido.min(self.cards.len() - 1);
            if let Some(card) = self.cards.get_mut(i) {
                resultados::desenhar(&mut filho, card, id);
            }
        }
        if confirmar(&egui_ctx, &mut self.confirmacao) == Some(true) {
            self.finalizar_transacao(&egui_ctx, ctx, Pedido::Descartar);
        }
    }

    /// O editor plano no alto do documento, com as marcas do erro, a linha do cursor e o trecho achado.
    fn editor(&mut self, ui: &mut Ui, rect: Rect, agora: f64, ctx: &Contexto) {
        self.area_editor = rect;
        self.atualizar_sugestoes(ctx);
        let p = paleta(ui);
        let id = self.id_do_editor();
        let n = self.tamanho_do_texto();
        let (s, _) = selecao(ui.ctx(), id, n);
        let linha_cursor = self.linha_do_cursor(s);
        let achado = self.achado.filter(|_| !self.busca.trim().is_empty());
        let rolar = std::mem::take(&mut self.rolar_achado);
        let chave_editor = self.chave("consulta::editor");
        let mut filho = ui.new_child(
            UiBuilder::new().id_salt(self.chave("consulta::area")).max_rect(rect).layout(Layout::top_down(Align::Min)),
        );
        filho.set_clip_rect(rect.intersect(ui.clip_rect()));
        let mut editor = AreaTexto::new(&mut self.sql)
            .codigo(true)
            .plana(true)
            .numeros_de_linha(true)
            .altura(rect.height())
            .largura(rect.width())
            .id_salt(chave_editor)
            .marcar_linhas(linha_cursor..linha_cursor + 1, p.estudio.selh);
        if let Some(m) = &self.marcas {
            editor = editor.marcar_linhas(m.linhas.clone(), p.estudio.perigo.gamma_multiply(0.09));
            if let Some(nome) = &m.sublinhado {
                editor = editor.sublinhar(nome.clone());
            }
        }
        if let Some((inicio, fim)) = achado {
            editor = editor.destacar(inicio..fim, rolar);
        }
        let saida = editor.mostrar(&mut filho);
        let r = &saida.response.response;
        ui.ctx().accesskit_node_builder(r.id, |no| no.set_label("Comando SQL"));
        if r.changed() {
            self.editado_em = Some(agora);
            self.edicao += 1;
            self.marcas = None;
            self.achado = None;
        }
        // a Consulta vazia: a dica e as sugestões por cima do editor (depois dele, para o clique ser delas)
        if self.sql.is_empty()
            && let Some(i) = consulta_vazia(ui, rect, self.chave("consulta::vazia"), &self.sugestoes.1)
        {
            let sql = self.sugestoes.1[i].clone();
            let egui = ui.ctx().clone();
            self.inserir(&egui, &sql);
        }
    }

    /// A faixa do seletor de resultados (42px, a `.faixa` com o fio de cima): devolve o resultado clicado.
    fn seletor(&self, ui: &mut Ui, rect: Rect) -> Option<usize> {
        let p = paleta(ui);
        let painter = ui.painter();
        painter.add(pintura::forma_preenchida(ui.ctx(), p, rect, 0.0, Preenchimento::Cabecalho));
        pintura::riscas_com_canto(painter, p, rect, CornerRadius::ZERO);
        painter.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, p.estudio.b));
        painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.estudio.btb));
        let linha = Rect::from_min_max(
            pos2(rect.left() + RECUO_FAIXA.0, rect.top() + 1.0),
            pos2(rect.right() - RECUO_FAIXA.1, rect.bottom() - 1.0),
        );
        let mut filho = ui.new_child(
            UiBuilder::new()
                .id_salt(self.chave("consulta::seletor"))
                .max_rect(linha)
                .layout(Layout::left_to_right(Align::Center)),
        );
        filho.set_clip_rect(linha.intersect(ui.clip_rect()));
        let textos: Vec<(String, Option<&str>, bool)> = self.cards.iter().map(Card::parte).collect();
        let partes: Vec<ParteSeletor<'_>> = textos
            .iter()
            .filter(|(t, _, _)| !t.is_empty())
            .map(|(t, nota, perigo)| {
                let parte = ParteSeletor::new(t).perigo(*perigo);
                match nota {
                    Some(n) => parte.nota(n),
                    None => parte,
                }
            })
            .collect();
        let ativo = self.escolhido.min(partes.len().saturating_sub(1));
        seletor(&mut filho, (self.chave("consulta::seletor"), self.execucoes), "Resultados", &partes, ativo)
    }

    /// As sugestões do editor vazio, da primeira tabela do banco (refeitas só quando ela muda).
    fn atualizar_sugestoes(&mut self, ctx: &Contexto) {
        let primeira = ctx.estado.objetos.as_ref().and_then(|o| o.tabelas.first()).map(|t| t.nome.as_str());
        if self.sugestoes.0.as_deref() != primeira {
            self.sugestoes = (primeira.map(str::to_string), texto::sugestoes(primeira));
        }
    }

    /// Insere o nome (já citado) ou a sugestão no cursor do editor, com o foco de volta nele.
    fn inserir(&mut self, ctx: &egui::Context, nome: &str) {
        let id = self.id_do_editor();
        let (s, e) = selecao(ctx, id, self.sql.chars().count());
        let (novo, cursor) = inserir_no_cursor(&self.sql, s, e, nome);
        self.sql = novo;
        self.edicao += 1;
        self.marcas = None;
        self.achado = None;
        colocar_cursor(ctx, id, cursor);
        ctx.memory_mut(|m| m.request_focus(id));
        self.editado_em = Some(ctx.input(|i| i.time));
    }

    fn guardar_no_endereco(&mut self, ctx: &mut Contexto) {
        self.editado_em = None;
        let sql = (!self.sql.is_empty() && self.sql.chars().count() <= MAX_SQL_NO_ENDERECO).then(|| self.sql.clone());
        ctx.emitir(Acao::Substituir(Rota::Consulta { sql }));
    }

    /// "Linha 4, coluna 1": o cursor do editor, refeito só quando ele ou o texto mudam.
    fn posicao(&mut self, egui: &egui::Context) -> (usize, usize) {
        let n = self.tamanho_do_texto();
        let cursor = TextEditState::load(egui, self.id_do_editor())
            .and_then(|s| s.cursor.char_range())
            .map_or(n, |r| r.primary.index.0.min(n));
        let chave = (cursor, self.edicao);
        match self.posicao {
            Some((c, pos)) if c == chave => pos,
            _ => {
                let pos = texto::linha_e_coluna(&self.sql, cursor);
                self.posicao = Some((chave, pos));
                pos
            }
        }
    }

    // ---------------------------------------------------------------- execução

    fn executar(&mut self, egui_ctx: &egui::Context, ctx: &mut Contexto) {
        if self.execucao.is_some() {
            return;
        }
        let id = self.id_do_editor();
        let (s, e) = selecao(egui_ctx, id, self.sql.chars().count());
        let (trecho, base) = if s != e { (texto::trecho(&self.sql, s, e), s) } else { (self.sql.clone(), 0) };
        if trecho.trim().is_empty() {
            toast(egui_ctx, MSG_ESCREVA);
            egui_ctx.memory_mut(|m| m.request_focus(id));
            return;
        }
        self.guardar_no_endereco(ctx);
        let comandos = banco::dividir_comandos(&trecho).len();
        let enviado = trecho.clone();
        let promessa = ctx.tarefa(egui_ctx, move |servico, id| servico.sql(id, &enviado, LIMITE));
        let inicio = egui_ctx.input(|i| i.time);
        self.execucao = Some(Execucao { pedido: Pedido::Executar, promessa, trecho, base, comandos, inicio });
        self.sondagem = None;
    }

    fn finalizar_transacao(&mut self, egui_ctx: &egui::Context, ctx: &mut Contexto, pedido: Pedido) {
        if self.execucao.is_some() {
            return;
        }
        let comando = if pedido == Pedido::Gravar { "COMMIT" } else { "ROLLBACK" };
        let promessa = ctx.tarefa(egui_ctx, move |servico, id| servico.sql(id, comando, LIMITE));
        let inicio = egui_ctx.input(|i| i.time);
        self.execucao = Some(Execucao { pedido, promessa, trecho: comando.to_string(), base: 0, comandos: 1, inicio });
        self.sondagem = None;
    }

    /// Toma as respostas das tarefas que terminaram.
    fn receber(&mut self, egui_ctx: &egui::Context, ctx: &mut Contexto) {
        if let Some(r) = self.sondagem.as_mut().and_then(Promessa::tomar) {
            self.sondagem = None;
            match r {
                Ok(aberta) => {
                    ctx.emitir(Acao::NotarTransacao(aberta));
                    self.transacao = Some(aberta);
                }
                // sem essa informação, a faixa aparece na próxima execução
                Err(e) => {
                    let _ = ctx.reagir(&e);
                }
            }
        }
        if let Some(r) = self.interrupcao.as_mut().and_then(Promessa::tomar) {
            self.interrupcao = None;
            // banco trocado fica em silêncio; `Nenhuma` deixa o botão ligado para outro clique; a que ainda espera a
            // vez só sai quando o banco soltar a conexão (uma gravação, uma exportação, um VACUUM): a pessoa fica
            // sabendo, em vez de achar que o Interromper não fez nada
            match r {
                Ok(Interrupcao::NaFila) => toast_atencao(egui_ctx, MSG_ESPERANDO_O_BANCO),
                Ok(_) => {}
                Err(e) => {
                    let _ = ctx.reagir(&e);
                }
            }
        }
        let Some(r) = self.execucao.as_mut().and_then(|e| e.promessa.tomar()) else { return };
        let Some(execucao) = self.execucao.take() else { return };
        let pedido = execucao.pedido;
        match r {
            Ok(resposta) => {
                // a casca (aviso nas outras abas, modo na barra de status, pergunta ao fechar, o Histórico)
                // acompanha a transação
                ctx.emitir(Acao::NotarTransacao(resposta.transacao_aberta));
                self.transacao = Some(resposta.transacao_aberta);
                let recarregar = pedido != Pedido::Executar || resposta.alterou_esquema || resposta.alterou_dados;
                let fim = match pedido {
                    Pedido::Executar => None,
                    Pedido::Gravar => Some(FIM_GRAVADA),
                    Pedido::Descartar => Some(FIM_DESCARTADA),
                };
                if pedido == Pedido::Executar {
                    self.marcas = self.marcas_do_erro(&execucao, &resposta.resultados);
                    let erro = resposta.resultados.iter().position(|r| matches!(r, ResultadoComando::Erro { .. }));
                    self.contas = Some(Contas {
                        comandos: execucao.comandos.max(resposta.resultados.len()),
                        executados: erro.unwrap_or(resposta.resultados.len()),
                        parou_em: erro.map(|i| i + 1),
                    });
                } else {
                    self.marcas = None;
                }
                self.execucoes += 1;
                self.cards = resultados::montar(resposta, fim, self.execucoes, self.sal);
                self.escolhido = resultados::escolhido_de_partida(&self.cards);
                if recarregar {
                    ctx.emitir(Acao::RecarregarObjetos);
                }
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Ignorar {
                    return;
                }
                if pedido == Pedido::Executar {
                    self.execucoes += 1;
                    self.cards = vec![Card::aviso(e.mensagem, TomAviso::Erro)];
                    self.escolhido = 0;
                    self.marcas = None;
                } else {
                    toast_erro(egui_ctx, e.mensagem);
                }
            }
        }
    }

    /// As marcas do editor depois de um erro: as linhas do comando que falhou (achado no trecho executado, na ordem
    /// dos comandos) e o nome que a frase cita, quando ele aparece no comando. Só quando o editor ainda tem o trecho
    /// executado no mesmo lugar.
    fn marcas_do_erro(&self, execucao: &Execucao, resultados: &[ResultadoComando]) -> Option<MarcasErro> {
        let ResultadoComando::Erro { mensagem, .. } = resultados.last()? else { return None };
        let n = execucao.trecho.chars().count();
        let ainda_la: String = self.sql.chars().skip(execucao.base).take(n).collect();
        if ainda_la != execucao.trecho {
            return None;
        }
        let comandos = resultados.iter().map(|r| match r {
            ResultadoComando::Linhas { comando, .. }
            | ResultadoComando::Alteracao { comando, .. }
            | ResultadoComando::Erro { comando, .. } => comando.as_str(),
        });
        let posicoes = texto::posicoes_dos_comandos(&execucao.trecho, execucao.base, comandos);
        let trecho = posicoes.last()?.clone()?;
        let primeira = texto::linha_do_caractere(&self.sql, trecho.start);
        let ultima = texto::linha_do_caractere(&self.sql, trecho.end);
        let comando: String = self.sql.chars().skip(trecho.start).take(trecho.len()).collect();
        let sublinhado = texto::nome_citado(mensagem)
            .and_then(|nome| texto::trecho_do_nome(&comando, nome))
            .map(|r| trecho.start + r.start..trecho.start + r.end);
        Some(MarcasErro { linhas: primeira..ultima + 1, sublinhado })
    }

    // ---------------------------------------------------------------- transação

    /// A faixa da transação aberta, antes da faixa do documento (o `.aviso` de atenção de ponta a ponta).
    fn aviso_transacao(&mut self, doc: &mut DocumentoPeca, ui: &mut Ui, ctx: &mut Contexto) {
        let pedido = self.execucao.as_ref().map(|e| e.pedido);
        let ocupado = pedido.is_some();
        let mut gravar = false;
        let mut descartar = false;
        doc.aviso(ui, TomAviso::Atencao, |ui, cores| {
            let trechos = [Trecho::Forte("Há uma transação aberta."), Trecho::Normal(MSG_TRANSACAO)];
            texto_rico(ui, &trechos, 12.0, cores.texto, cores.forte);
            em_linha(ui, 8.0, |ui| {
                let b = Botao::primario("Gravar (COMMIT)").pequeno().ocupado(pedido == Some(Pedido::Gravar));
                gravar = ui.add(b.habilitado(!ocupado)).clicked();
                let b = Botao::perigo("Descartar (ROLLBACK)").pequeno().ocupado(pedido == Some(Pedido::Descartar));
                descartar = ui.add(b.habilitado(!ocupado)).clicked();
            });
        });
        let egui_ctx = ui.ctx().clone();
        if gravar {
            self.finalizar_transacao(&egui_ctx, ctx, Pedido::Gravar);
        }
        if descartar && self.execucao.is_none() {
            self.confirmacao =
                Some(Confirmacao::new(MSG_DESCARTAR).titulo("Descartar a transação").rotulo("Descartar").perigo(true));
        }
    }

    // ---------------------------------------------------------------- Propriedades

    /// "Execução": as contas da última resposta e a transação.
    fn secao_execucao(&self, ui: &mut Ui, ctx: &Contexto) {
        let perigo = paleta(ui).estudio.perigo;
        let aberta = self.transacao_aberta(ctx);
        let contas = self.contas;
        let _ = Secao::new(self.chave("propriedades::execucao"), "Execução").primeira(true).mostrar(ui, |ui| {
            ficha(ui, |d| {
                let numero = |n: Option<usize>| n.map(formatar_numero);
                d.linha_texto("Comandos", numero(contas.map(|c| c.comandos)).as_deref(), "—");
                d.linha_texto("Executados", numero(contas.map(|c| c.executados)).as_deref(), "—");
                match contas.and_then(|c| c.parou_em) {
                    Some(n) => {
                        d.linha("Parou em", |ui| texto(ui, &format!("Comando {n}"), 12.0, perigo));
                    }
                    None => d.linha_texto("Parou em", None, "—"),
                }
                d.linha_texto("Transação", Some(if aberta { "aberta" } else { "nenhuma aberta" }), "—");
            });
        });
    }

    /// "Tabelas": as tabelas e visões do banco, que inserem o nome citado no cursor do editor.
    fn secao_tabelas(&mut self, ui: &mut Ui, ctx: &Contexto) {
        let mut clicado = None;
        let objetos = ctx.estado.objetos.as_ref();
        let tabelas = objetos.map(|o| o.tabelas.as_slice()).unwrap_or_default();
        let visoes = objetos.map(|o| o.visoes.as_slice()).unwrap_or_default();
        let t3 = paleta(ui).estudio.t3;
        let _ = Secao::new(self.chave("propriedades::tabelas"), "Tabelas").mostrar(ui, |ui| {
            if tabelas.is_empty() && visoes.is_empty() {
                egui::Frame::NONE.inner_margin(Margin { left: 14, right: 14, top: 2, bottom: 12 }).show(ui, |ui| {
                    paragrafo(ui, SEM_TABELAS, 12.0, t3);
                });
                return;
            }
            let itens = tabelas
                .iter()
                .map(|t| (t.nome.as_str(), estado::icone_da_tabela(t.tipo, t.sem_rowid)))
                .chain(visoes.iter().map(|v| (v.nome.as_str(), Icone::Visao)));
            for (nome, icone) in itens {
                let dica = format!("Inserir {} no comando", citar_nome(nome));
                // o rótulo diz o que o clique faz (e não repete o nó do Explorador)
                let r = No::new(nome)
                    .rotulo(&dica)
                    .recuo(12.0)
                    .icone(icone)
                    .icone_a_direita(Icone::Inserir)
                    .dica(&dica)
                    .mostrar(ui);
                if r.resposta.clicked() {
                    clicado = Some(nome.to_string());
                }
            }
            // `padding:0 0 10px`
            ui.add_space(10.0);
        });
        if let Some(nome) = clicado {
            let egui = ui.ctx().clone();
            self.inserir(&egui, &citar_nome(&nome));
        }
    }
}

// ------------------------------------------------------------------ auxiliares

/// A faixa de erro (`.erro`, item 23: `padding:8px 14px; gap:12px; align-items:center`, o `--perigo` a 10% e o fio
/// de baixo a 30%): o sólido de erro de 24px centrado na altura do texto, "Comando 3: {mensagem}" em 600 `--perigo`
/// (com o "i" do original do SQLite) e, embaixo, em `--t2`, a frase de que a execução parou ali. Devolve onde ela
/// ficou.
fn faixa_de_erro(ui: &mut Ui, faixa: &resultados::FaixaErro<'_>) -> Rect {
    const RECUO: Vec2 = vec2(14.0, 8.0);
    const LADO_ICONE: f32 = 24.0;
    const GAP: f32 = 12.0;
    let e = paleta(ui).estudio;
    // a interrupção pedida pela pessoa sai no tom de atenção (`--aviso` a 12% e o fio a 35%, o sólido de atenção e
    // a frase em `--t`), como os outros avisos
    let (cor, forte, fundo, fio, estado) = if faixa.interrompido {
        (e.aviso, e.t, e.aviso.gamma_multiply(0.12), e.aviso.gamma_multiply(0.35), EstadoIcone::Atencao)
    } else {
        (e.perigo, e.perigo, e.perigo.gamma_multiply(0.10), e.perigo.gamma_multiply(0.30), EstadoIcone::Erro)
    };
    let frase = match faixa.numero {
        Some(n) => format!("Comando {n}: {}", faixa.mensagem),
        None => faixa.mensagem.to_string(),
    };
    let topo = ui.cursor().min;
    let largura = ui.available_width();
    let lugar_fundo = ui.painter().add(egui::Shape::Noop);
    let x_texto = RECUO.x + LADO_ICONE + GAP;
    let interno = Rect::from_min_size(
        topo + vec2(x_texto, RECUO.y),
        vec2((largura - x_texto - RECUO.x).max(10.0), f32::INFINITY),
    );
    let mut filho = ui.new_child(UiBuilder::new().max_rect(interno).layout(Layout::top_down(Align::Min)));
    // `margin-top:2px` da segunda linha
    filho.spacing_mut().item_spacing.y = 2.0;
    resultados::frase_do_erro(&mut filho, &frase, faixa.detalhe, e.t2, forte);
    if faixa.parou {
        texto(&mut filho, resultados::MSG_PAROU, 12.0, e.t2);
    }
    let altura = filho.min_rect().height().max(LADO_ICONE) + 2.0 * RECUO.y;
    let rect = Rect::from_min_size(topo, vec2(largura, altura));
    ui.painter().set(lugar_fundo, egui::Shape::rect_filled(rect, 0.0, fundo));
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, fio));
    let icone = Rect::from_min_size(
        pos2(rect.left() + RECUO.x, rect.center().y - LADO_ICONE / 2.0),
        vec2(LADO_ICONE, LADO_ICONE),
    );
    icones::pintar(ui.painter(), icone, Icone::Estado(estado), cor);
    ui.allocate_rect(rect, Sense::hover()).rect
}

/// A Consulta vazia (prancheta 15): por cima do editor sem texto, onde o texto começaria (depois da calha, a 14px
/// dela e a 10px do alto), a dica [`DICA_EDITOR`] em itálico 12px `--t3`, centrada na altura da linha 1 do código
/// (a do número "1" da calha), e, 14px abaixo, o `.tit` "Para começar" (10,5px) e as sugestões em [`atalho_justo`]
/// numa fileira que quebra (`gap:8px`). Devolve a sugestão clicada.
fn consulta_vazia(ui: &mut Ui, rect: Rect, chave: Chave, sugestoes: &[String]) -> Option<usize> {
    const RECUO: Vec2 = vec2(14.0, 10.0);
    let t3 = paleta(ui).estudio.t3;
    let area = Rect::from_min_max(
        pos2(rect.left() + LARGURA_CALHA + RECUO.x, rect.top() + RECUO.y),
        pos2(rect.right() - RECUO.x, rect.bottom()),
    );
    let mut filho = ui.new_child(UiBuilder::new().id_salt(chave).max_rect(area).layout(Layout::top_down(Align::Min)));
    filho.set_clip_rect(rect.intersect(ui.clip_rect()));
    filho.spacing_mut().item_spacing = vec2(8.0, 8.0);
    let linha = interface::codigo::altura_linha(interface::codigo::fonte(ui.ctx()).size);
    let g = filho.fonts_mut(|f| {
        f.layout(DICA_EDITOR.to_string(), fontes::fonte(12.0, Peso::Italico), t3, area.width().max(1.0))
    });
    let (r, resp) = filho.allocate_exact_size(vec2(g.size().x, g.size().y.max(linha)), Sense::hover());
    filho.painter().galley(pos2(r.left(), r.top() + ((linha - g.size().y) / 2.0).max(0.0)), g, t3);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, DICA_EDITOR));
    // `gap:14px` entre a dica e o bloco das sugestões
    filho.add_space(14.0 - 8.0);
    let g = galley_tit_em(&filho, PARA_COMECAR, 10.5, t3);
    let (r, resp) = filho.allocate_exact_size(g.size(), Sense::hover());
    filho.painter().galley(r.min, g, t3);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, PARA_COMECAR));
    let mut clicada = None;
    filho.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (i, sql) in sugestoes.iter().enumerate() {
            if atalho_justo(ui, sql).clicked() {
                clicada = Some(i);
            }
        }
    });
    clicada
}

/// A seleção do editor `id` (em caracteres, em ordem); sem estado guardado, o cursor no fim.
fn selecao(ctx: &egui::Context, id: Id, n: usize) -> (usize, usize) {
    TextEditState::load(ctx, id).and_then(|s| s.cursor.char_range()).map_or((n, n), |r| {
        let [a, b] = r.sorted_cursors();
        (a.index.0.min(n), b.index.0.min(n))
    })
}

/// Põe o cursor do editor `id` em `pos` (sem seleção).
fn colocar_cursor(ctx: &egui::Context, id: Id, pos: usize) {
    let mut estado = TextEditState::load(ctx, id).unwrap_or_default();
    estado.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
    estado.store(ctx, id);
}

/// A borda de baixo do editor (`rect`), que se arrasta para mudar a altura dele: no mínimo [`ALTURA_MIN_EDITOR`] e
/// no máximo `maximo` (o que deixa o seletor e um pouco do resultado à vista).
fn alca_do_editor(ui: &mut Ui, id: Id, rect: Rect, altura: &mut f32, maximo: f32) {
    let borda = Rect::from_min_max(pos2(rect.left(), rect.bottom() - ALCA), pos2(rect.right(), rect.bottom() + ALCA));
    let r = ui.interact(borda, id, Sense::drag());
    if r.drag_started() {
        ui.ctx().data_mut(|d| d.insert_temp(id, rect.height()));
    }
    if r.dragged() {
        // a altura do começo mais o caminho do ponteiro desde onde foi apertado (somar os deslocamentos de cada
        // quadro perderia o passo abaixo do mínimo, e a borda deixaria de acompanhar o ponteiro)
        let inicio: Option<f32> = ui.ctx().data(|d| d.get_temp(id));
        let origem = ui.input(|i| i.pointer.press_origin());
        if let (Some(inicio), Some(origem), Some(agora)) = (inicio, origem, r.interact_pointer_pos()) {
            *altura = (inicio + agora.y - origem.y).clamp(ALTURA_MIN_EDITOR, maximo.max(ALTURA_MIN_EDITOR));
        }
    }
    if r.hovered() || r.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }
}

/// Alguma janela (modal) está aberta: o Ctrl+Enter é dela.
fn janela_aberta(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.top_modal_layer().is_some())
}

/// Ctrl+Enter (sem Alt) neste quadro; o evento é consumido.
fn consumir_ctrl_enter(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| {
        let antes = i.events.len();
        i.events.retain(|e| {
            !matches!(e, Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                if modifiers.command && !modifiers.alt)
        });
        i.events.len() != antes
    })
}

/// Esc (sem modificadores) neste quadro; o evento é consumido.
fn consumir_esc(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
}

/// A Consulta SQL como documento da casca (prancheta 3): faixa, rodapé e rolagem própria; o Executar principal, o
/// Exportar do resultado à vista e a busca "Buscar no editor" na barra; "SQL" na barra de status; "Execução" e
/// "Tabelas" nas Propriedades. A execução não conta como tarefa (`ocupada` é false): ela pode demorar quanto o
/// comando quiser, e a casca não espera por ela.
impl estado::Documento for TelaConsulta {
    fn anatomia(&self) -> estado::Anatomia {
        estado::Anatomia::default().com_faixa().com_rodape().rolagem_propria()
    }

    fn avisos(&mut self, doc: &mut DocumentoPeca, ui: &mut Ui, ctx: &mut Contexto) {
        if self.transacao_aberta(ctx) {
            self.aviso_transacao(doc, ui, ctx);
        }
    }

    /// O cilindro e o nome do banco (a posição do cursor foi para o rodapé, U4 da revisão de UI de 22/10/2026).
    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let e = paleta(ui).estudio;
        let banco = ctx.estado.banco.as_ref();
        let icone = if banco.is_some_and(|b| b.somente_leitura) { Icone::BancoSoLeitura } else { Icone::Banco };
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        icones::pintar(ui.painter(), r, icone, e.t2);
        if let Some(b) = banco {
            texto(ui, &b.nome, 12.0, e.t2);
        }
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.conteudo_da_tela(ui, ctx);
    }

    /// O que o resultado à vista diz: "8 linhas · 0,5 ms"; executando, "Executando · 1,4 s", contado ao vivo (o
    /// quadro se renova a cada 250 ms só enquanto executa; com o arco do Executar girando, a cada ~33 ms); antes da
    /// primeira execução, [`NADA_EXECUTADO`] em `--t3`. Na ponta direita, a posição do cursor ("Linha 3, coluna
    /// 37", como no JSON) e, à esquerda dela, as contas da seleção.
    fn rodape(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
        let t2 = paleta(ui).estudio.t2;
        let t3 = paleta(ui).estudio.t3;
        let (linha, coluna) = self.posicao(ui.ctx());
        let posicao = format!("Linha {linha}, coluna {coluna}");
        if let Some(e) = self.execucao.as_ref().filter(|e| e.pedido == Pedido::Executar) {
            let agora = ui.input(|i| i.time);
            let decorrido = formatar_segundos(((agora - e.inicio).max(0.0) * 1000.0).floor());
            texto(ui, &format!("Executando · {decorrido}"), 12.0, t2);
            ui.ctx().request_repaint_after(REPINTURA_EXECUTANDO);
        } else if let Some(texto_rodape) = self.cards.get(self.escolhido).and_then(Card::rodape) {
            texto(ui, &texto_rodape, 12.0, t2);
        } else if self.cards.is_empty() && self.execucao.is_none() {
            texto(ui, NADA_EXECUTADO, 12.0, t3);
        }
        // com mais de uma célula selecionada, as contas da seleção à esquerda da posição (A.3)
        let executando = self.execucao.as_ref().is_some_and(|e| e.pedido == Pedido::Executar);
        let resumo = if executando { None } else { self.cards.get_mut(self.escolhido).and_then(Card::resumo_da_selecao) };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            texto(ui, &posicao, 12.0, t3);
            if let Some(resumo) = &resumo {
                ui.add_space(12.0);
                rodape_da_selecao(ui, resumo);
            }
        });
    }

    fn ofertas(&self, _estado: &estado::Estado) -> estado::Ofertas {
        let executar = if self.interrompivel() {
            // N3: desligado, com o arco girando no lugar do triângulo e a dica "Executando"
            estado::Oferta::default().ocupada_se(true).como_principal().com_dica(DICA_EXECUTANDO)
        } else {
            // a Consulta vazia: desligado até haver texto (prancheta 15)
            let com_texto = !self.sql.trim().is_empty();
            estado::Oferta::ligada().ligada_se(!self.executando() && com_texto).como_principal().com_dica(DICA_EXECUTAR)
        };
        let interromper = if self.interrompivel() {
            estado::Oferta::ligada().com_dica(DICA_INTERROMPER)
        } else {
            estado::Oferta::default()
        };
        let exportar = match self.exportavel() {
            Some((n, _)) => estado::Oferta::ligada().com_dica(format!("Exportar o resultado do comando {n}")),
            None => estado::Oferta::default().com_dica(DICA_EXPORTAR),
        };
        estado::Ofertas::default()
            .com(estado::Verbo::Executar, executar)
            .com(estado::Verbo::Interromper, interromper)
            .com(estado::Verbo::Exportar, exportar)
            .ligar(estado::Verbo::Buscar)
    }

    /// "Buscar no editor": cada mudança acha a primeira ocorrência a partir da que estava achada (ou do cursor), sem
    /// caixa nem acento; o Enter vai à seguinte, com volta ao começo. O trecho achado fica pintado no editor (que rola
    /// até ele), e o cursor do editor vai para o fim dele.
    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        Some(estado::BuscaDaBarra { texto: &mut self.busca, dica: DICA_BUSCA.into() })
    }

    fn evento_busca(&mut self, evento: estado::EventoBusca, egui: &egui::Context, _ctx: &mut Contexto) {
        let id = self.id_do_editor();
        let n = self.sql.chars().count();
        let (s, e) = selecao(egui, id, n);
        let desde = match (evento, self.achado) {
            (estado::EventoBusca::Limpar, _) => {
                self.achado = None;
                return;
            }
            (estado::EventoBusca::Enter | estado::EventoBusca::Anterior, Some((_, fim))) => fim,
            (estado::EventoBusca::Enter | estado::EventoBusca::Anterior, None) => e,
            (estado::EventoBusca::Mudou, Some((inicio, _))) => inicio,
            (estado::EventoBusca::Mudou, None) => s,
        };
        self.achado = texto::achar_sem_acento(&self.sql, self.busca.trim(), desde);
        if let Some((_, fim)) = self.achado {
            self.rolar_achado = true;
            colocar_cursor(egui, id, fim);
        }
    }

    fn executar(&mut self, verbo: estado::Verbo, egui: &egui::Context, ctx: &mut Contexto) {
        match verbo {
            // como o Ctrl+Enter: com texto selecionado no editor, só o trecho
            estado::Verbo::Executar => self.executar_agora(),
            estado::Verbo::Interromper => self.interromper(egui, ctx),
            estado::Verbo::Exportar => {
                if let Some((_, pedido)) = self.exportavel() {
                    ctx.emitir(Acao::AbrirExportacao(pedido.clone()));
                }
            }
            _ => {}
        }
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.secao_execucao(ui, ctx);
        self.secao_tabelas(ui, ctx);
    }

    fn status(&self, _estado: &estado::Estado) -> Option<String> {
        Some("SQL".into())
    }

    fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        self.receber_em_segundo_plano(egui, ctx);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sem_sal_os_ids_sao_os_de_sempre_e_com_sal_cada_tela_tem_os_seus() {
        let sem = TelaConsulta::nova(None);
        assert_eq!(sem.id_do_editor(), TelaConsulta::id_editor());
        assert_eq!(sem.id("consulta::editor::alca"), Id::new("consulta::editor::alca"));
        // dentro de tuplas também (as grades e os resultados)
        assert_eq!(
            Id::new((sem.chave("consulta::grade"), 3_u64, 1_usize)),
            Id::new(("consulta::grade", 3_u64, 1_usize))
        );
        let um = TelaConsulta::nova(None).com_id(1);
        let dois = TelaConsulta::nova(None).com_id(2);
        assert_eq!(um.id_do_editor(), Id::new(("consulta::editor", 1_u64)));
        assert_ne!(um.id_do_editor(), dois.id_do_editor());
        assert_ne!(um.id_do_editor(), TelaConsulta::id_editor());
        assert_ne!(um.id("consulta::seletor"), dois.id("consulta::seletor"));
    }

    /// C5: o tamanho do texto e a linha do cursor (O(n)) são lidos do cache enquanto a versão, o tamanho em bytes e o
    /// cursor não mudam, e refeitos quando mudam.
    #[test]
    fn tamanho_do_texto_e_linha_do_cursor_so_se_refazem_quando_mudam() {
        let mut t = TelaConsulta::nova(Some("SELECT 1;\nSELECT ação;\n".to_string()));
        assert_eq!(t.tamanho_do_texto(), 23);
        assert_eq!(t.chars_do_texto, Some((0, t.sql.len(), 23)));
        // sem mudar nada, o valor guardado serve (um valor guardado de propósito errado prova que não recontou)
        t.chars_do_texto = Some((0, t.sql.len(), 999));
        assert_eq!(t.tamanho_do_texto(), 999);
        // o texto mudou (a versão sobe): conta de novo
        t.sql.push('x');
        t.edicao += 1;
        assert_eq!(t.tamanho_do_texto(), 24);
        assert_eq!(t.linha_do_cursor(0), 0);
        assert_eq!(t.linha_do_cursor(12), 1);
        t.linha_do_cursor = Some(((t.edicao, t.sql.len(), 12), 77));
        assert_eq!(t.linha_do_cursor(12), 77, "o cursor e o texto são os mesmos: do cache");
        assert_eq!(t.linha_do_cursor(3), 0, "outro cursor: refaz");
        // o tamanho em bytes muda sem a versão (o recuo do Tab): também refaz
        t.sql.push_str("yy");
        assert_eq!(t.tamanho_do_texto(), 26);
    }

    #[test]
    fn anatomia_com_faixa_rodape_e_rolagem_propria() {
        use estado::Documento;
        let a = TelaConsulta::nova(None).anatomia();
        assert!(a.faixa && a.rodape && a.rolagem == estado::Rolagem::Propria);
    }
}
