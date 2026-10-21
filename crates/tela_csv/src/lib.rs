//! O editor tabular de CSV do DataBox (etapa 5, prancheta 7 `Csv.dc.html`; os estados da prancheta 15 e o "copiar,
//! colar e trazer colunas" da prancheta 16). Um `estado::Documento` que a casca põe no lugar da ficha do arquivo
//! (`Rota::Arquivo`).
//!
//! # API (o contrato com a casca)
//!
//! ```text
//! TelaCsv::nova(caminho) -> TelaCsv          o arquivo é lido numa tarefa no primeiro quadro (nada de disco aqui)
//! impl estado::Documento for TelaCsv         anatomia (faixa, rodapé, rolagem própria) · avisos · faixa · conteudo ·
//!                                            rodape ·
//!                                            ofertas/executar · tem_propriedades/propriedades · status ·
//!                                            status_do_arquivo · ocupada · em_segundo_plano · alterado · alteracoes ·
//!                                            salvando · tomar_erro_de_gravacao · fonte_de_colunas
//!   nome() · caminho() · aberta() · carregando() · erro_ao_abrir() -> Option<&str> (a casca troca pela ficha) ·
//!   retrato() -> Option<&Retrato> · pagina() -> Option<&Pagina> · celula_ativa() · selecionar(..) ·
//!   linhas_marcadas() · marcar_linhas(..) · colunas_selecionadas() · selecionar_colunas(..) · editando() ·
//!   trazer_aberta() · por_posicao_permitida() ·
//!   trazer_colunas_de(rótulo)   a janela Trazer colunas com a origem de outra aba já escolhida (o "Trazer colunas
//!                               desta planilha" do menu do cabeçalho de uma planilha: a casca foca o CSV e chama)
//! abrir_fonte(&caminho) -> Resultado<Arc<dyn FonteDeColunas>>   um CSV do disco como origem (numa tarefa)
//! PREFIXO_ACAO_TOAST ("tela_csv:")   a carga da ação do toast do documento (o "Marcar as linhas sem par")
//!
//! O que a casca (faixa E) precisa fazer: o Ctrl+S, o Ctrl+Z e o Ctrl+Y são os verbos Salvar, Desfazer e Refazer
//! (`Documento::executar`); a fila de perguntas ao fechar usa `alterado`, `alteracoes`, `salvando` e
//! `tomar_erro_de_gravacao` (`alterado` conta também a célula e o cabeçalho em edição e a alteração ainda na fila, e o
//! Salvar os grava antes); `fontes` no `Contexto` de cada aba (a lista das `fonte_de_colunas`); e a casca **não**
//! deve consumir a ação do toast cuja carga começa com [`PREFIXO_ACAO_TOAST`] (o `cumprir_acao_do_toast` de hoje
//! consome toda ação e a trata como um caminho): ela é do documento, que a toma com `acao_do_toast` no quadro dele.
//! Retrato { linhas, nomes, tipos, formato, alteracoes, alterada, pode_desfazer, pode_refazer, passos_a_refazer,
//!           tamanho, modificado, caminho }       a cópia do que a tabela é, que a grade e as Propriedades desenham
//! Pagina { inicio, posicoes, linhas, marcas }   as linhas em mãos (as da vista, com a posição de cada uma)
//! ESPERA_DO_CARD (0,4 s)    o card "Lendo vendas.csv… 43%" com a barra e o Cancelar só aparece depois disto
//! ```
//!
//! # Como funciona
//!
//! - **A tabela fica numa tarefa**: `Arc<Mutex<tabular::csv::Tabela>>`, e toda chamada que lê disco ou
//!   altera vai numa fila do documento, uma de cada vez (`sessao::Trabalho`). Cada tarefa devolve o [`Retrato`] e a
//!   [`Pagina`] já relidos; a grade pinta dessas cópias e nunca espera o `Mutex`. As leituras de coluna inteira (as
//!   estatísticas da seção Coluna) tiram um instantâneo (um leitor próprio do arquivo e a cópia das alterações) e
//!   leem fora do `Mutex`, pelo `Leitor::varrer`.
//! - **Abrir:** `Tabela::abrir_com_progresso`; o card com a barra de progresso e o Cancelar (que fecha a aba,
//!   `Acao::FecharAba(None)`) só depois de [`ESPERA_DO_CARD`]. Falhou (UTF-16, bytes nulos, arquivo sumido): o aviso
//!   de perigo com a frase do núcleo no alto do documento, e [`TelaCsv::erro_ao_abrir`] diz a frase.
//! - **A grade** (`interface::grade::NucleoGrade`, `Navegacao::Dados`): a caixa de marcar (32), o número da linha
//!   (`#`, 48; a posição na tabela), as colunas com a pastilha do tipo detectado nas primeiras 2.000 linhas (a dica
//!   "número inteiro (detectado)"), o `th.nova-col` com o "+" no fim. Números à direita, vazias em branco, alteradas
//!   com o `td.alt`. A rolagem é de todas as linhas: só a página à vista (240 linhas) fica em mãos, e a que falta é
//!   pedida quando a grade rola.
//! - **Faixa e rodapé** (revisão de UI de 19/10/2026, como os Dados): na faixa, o nome do arquivo e o formato em
//!   pílulas ("Ponto e vírgula", "UTF-8", "CRLF"); embaixo dela, sempre, a barra dos filtros; a seta da ordem é a
//!   `.seta-ord` desenhada (`interface::grade::pintar_seta_ordem`). Rodapé: "Linhas 1–26 de 1.204" (as linhas à vista
//!   de quantas a vista tem, o `interface::formato::contagem_linhas` dos Dados) e, em `--acct`, "· 1 alteração não
//!   salva".
//! - **Propriedades:** Formato (os quatro seletores e a caixa do cabeçalho; mudar um relê o arquivo com ele fixo;
//!   desligados com alteração pendente, com a dica), Arquivo (`estado::secao_arquivo`) e Coluna {nome} da célula ativa
//!   (Tipo detectado, Vazias e, nas numéricas, Mínimo, Máximo e Soma; nas de texto, Maior texto).
//! - **Status:** `status_do_arquivo` com "Salvo" ou "Alterado, não salvo" e "CSV · UTF-8 · CRLF · Separador ;".
//! - **Editar** (passo 11): a célula no lugar (`Entrada::compacta`; duplo clique, Enter, F2 ou uma tecla de texto;
//!   Enter grava, Tab grava e anda, Esc desiste, perder o foco grava; o texto fica na célula até a resposta). Nova
//!   linha (depois da ativa; sem ativa, no fim), Duplicar e Excluir (as marcadas ou a linha da ativa; a dica diz o
//!   alvo: "Excluir as 2 linhas marcadas", "Duplicar a linha 7"), Delete exclui as marcadas, espaço marca a ativa.
//!   As marcas são posições: acompanham inserir, duplicar e excluir; desfazer, refazer e reler limpam.
//!   O desfazer leva rótulos ("Desfazer Excluir 2 linhas"); fora de passo com o núcleo, o genérico. Salvar
//!   (`Tabela::salvar`, com o aviso "Salvando vendas_2025.csv… 43%" e o Cancelar depois de 0,4 s; o toast
//!   "vendas_2025.csv salvo."; o erro em `tomar_erro_de_gravacao` e o status "Com erro"). Com o arquivo mudado por
//!   outro programa, o núcleo não tem como regravar (ele copia as linhas que não mudaram do arquivo de origem): nada é
//!   gravado, o toast e o erro dizem, e o aviso de topo oferece "Ler de novo". Enquanto grava, nada se edita e os verbos
//!   de edição ficam desligados. Com uma alteração que mexe nas posições (inserir, excluir, duplicar, desfazer, ler de
//!   novo) ainda sem resposta, o que é pedido por posição espera (o Delete repetido não exclui a linha de baixo); a
//!   célula aberta grava antes de outra alteração entrar na fila. Ctrl+C: o intervalo de células em TSV (as linhas
//!   fora da página lidas numa tarefa), as linhas marcadas em TSV com o cabeçalho (numa tarefa) ou a célula ativa. Sem "Primeira linha é o cabeçalho", as colunas não têm nome no arquivo: Renomear
//!   fica desligado (com a dica) e a Nova coluna entra com o nome genérico.
//! - **Colunas** (passo 12, prancheta 16): o clique no cabeçalho seleciona a coluna (Ctrl soma ou tira, Shift
//!   estende; o clique numa célula desfaz), "· coluna valor selecionada" no rodapé. O menu do cabeçalho (clique
//!   direito e Shift+F10, a peça `MenuContexto`): Copiar coluna ("Copiar 3 colunas") · Colar como nova coluna ·
//!   Colar sobre esta coluna · Colar com correspondência · Trazer colunas de outro arquivo · Renomear a coluna (F2) ·
//!   Nova coluna depois desta · Excluir a coluna. A Nova coluna (o verbo, depois da coluna da ativa: "Nova coluna
//!   depois de regiao"; o "+" no fim) e a cola entram como coluna **provisória** com o cabeçalho em edição (o
//!   `.th-edit`, o nome selecionado): Enter confirma, Esc desiste (a cola mantém o nome sugerido), e o núcleo só é
//!   tocado no fim, num passo só. Copiar coluna: TSV com o cabeçalho (numa tarefa, fora do `Mutex`), o toast "Coluna
//!   valor copiada · 1.204 linhas"; o DataBox lembra o último texto que copiou (a cola dele tem cabeçalho). Colar
//!   (Ctrl+V, ou o item do menu, que pede o `RequestPaste`): `ler_tsv` + `colunas_coladas` + `posicionar`; com mais
//!   valores que linhas, o aviso "A coluna colada tem 1.204 valores e o arquivo, 1.180 linhas." com Colar 1.180 ·
//!   Acrescentar 24 linhas · Cancelar; com menos, "24 linhas ficaram vazias"; sobre uma coluna com valores,
//!   "Substituir 1.180 valores de quantidade?". Os Colar por posição ficam desligados com filtro ou ordem
//!   ([`TelaCsv::por_posicao_permitida`], com a dica da prancheta 16 b).
//! - **Trazer colunas** (passo 13, prancheta 16 d e e): a janela de 640px (o verbo da barra, o menu do cabeçalho, o
//!   Ctrl+Shift+V, que chega como o `Event::Paste` e a tecla V com o Shift, e a seção Coluna). De onde: as fontes das
//!   outras abas (`Contexto::fontes`, sem esta), a área de transferência (quando se sabe o texto) e "Outro arquivo"
//!   (o diálogo; [`abrir_fonte`] numa tarefa). As chaves sugeridas pelo nome ou pelos valores em comum numa amostra;
//!   o medidor "4.812 de 5.000 linhas têm par (96%)" com a barra do Exportar, recalculado a cada troca (a chave deste
//!   arquivo fora do `Mutex`, as da origem pela `FonteDeColunas`, o `casar` numa tarefa, com "Cancelar o cálculo");
//!   as três opções; o aviso das chaves repetidas com "Usar" e "Ver quais"; a lista (nova coluna com o nome
//!   editável, `preco (catalogo)` quando já existe, ou atualizar substituindo ou só nas vazias); a prévia das 6
//!   primeiras linhas (a chave em `--acct`, o `--tint` só no que vem, o "—" sem par). Trazer é um passo só
//!   ("Desfazer Trazer 3 colunas") e dá o toast "3 colunas trazidas · 188 linhas sem par" com "Marcar as linhas sem
//!   par" (a carga começa com [`PREFIXO_ACAO_TOAST`]). `Documento::fonte_de_colunas` devolve a fonte deste CSV (a
//!   mesma `Arc`, com as alterações pendentes).
//! - **Filtrar e ordenar** (o passo a mais; prancheta 16 b): a barra dos filtros (as peças da aba Dados,
//!   `interface::pecas::filtros`: "Filtros", uma etiqueta por condição com o popover de Coluna, Operador e Valor, "+
//!   Condição" e, filtrando, "236 de 1.180 linhas" e "Limpar filtros"; o verbo Filtrar abre o popover) e a seta do cabeçalho ordena (crescente →
//!   decrescente → sem ordem; números pelo valor, texto sem diferença de caixa, vazias no fim). A **vista** (as
//!   posições que passam, na ordem) é calculada numa tarefa com um instantâneo e uma passada pelo arquivo
//!   (`Leitor::varrer`) e refeita a cada alteração; o número da linha continua a posição no arquivo, e os Colar por
//!   posição ficam desligados (o Ctrl+V explica com a dica da prancheta).
//! - **Atualizar (F5):** relê; com alterações, pergunta "Descartar as alterações de {nome} e ler o arquivo de novo?".
//!   A volta do foco confere se o arquivo mudou no disco (o aviso de topo "O arquivo mudou no disco." com Ler de novo
//!   e Ignorar).
//!
//! # Testes
//!
//! `tests/abrir.rs`, `editar.rs`, `colunas.rs`, `trazer.rs` e `filtrar.rs` (um por passo) pelo
//! `estado::apoio_testes::palco_documento` (a barra com os verbos ligados, o documento e as
//! Propriedades, como a casca), um wgpu por vez (`tests/apoio`); as capturas vão para `capturas/rust/`.

#![forbid(unsafe_code)]

mod colunas;
mod edicao;
mod faixa;
mod filtro;
mod fonte_csv;
mod grade;
mod janela_trazer;
mod pintor;
mod propriedades;
mod sessao;
mod tela;
mod textos;
mod trazer;
mod verbos;

pub use filtro::{Condicao, OPERADORES, OperadorCsv, Ordem};
pub use propriedades::{Estatistica, calcular_estatistica};
pub use sessao::{Pagina, Retrato};
pub use tela::{ESPERA_DO_CARD, Ganchos, TelaCsv};
pub use trazer::{MARCAR_SEM_PAR, PREFIXO_ACAO_TOAST, abrir_fonte};
