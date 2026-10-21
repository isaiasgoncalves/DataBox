//! A ponte entre as telas (egui) e o `servico`: estado do aplicativo, endereços com histórico, pedidos das
//! telas à casca, tarefas de fundo e diálogos do Windows.
//!
//! # API
//!
//! ```text
//! Promessa<T>                 tarefa de fundo: Promessa::rodar(ctx, || -> Resultado<T>) · tomar() -> Option<Resultado<T>>
//!                             (uma vez) · pendente() · Promessa::pronta(r). Pânico vira ErroBanco 500 MSG_INESPERADO.
//!                             tarefas_em_andamento() · esperar_tarefas(prazo) (o executável espera ao fechar).
//! Rota::{Inicio, Banco, Tabela{nome, aba, dados}, Visao{nome, aba, dados}, Consulta{sql}, Arquivo{caminho}}
//!     objeto(nome, e_visao) · objeto_na_aba · consulta_com(sql) · arquivo(caminho) · exige_banco (false no Início e
//!     no Arquivo) · nome_objeto · caminho_arquivo · aba · dados · com_dados(ConsultaDados) · endereco()
//!     (Arquivo: "#/arquivo/<caminho>")
//! Parte::{Dados, Estrutura, Indices, Definicao} · DA_TABELA · DA_VISAO · chave · rotulo · de_chave
//!     (Aba = Parte, o nome antigo; "aba" agora é a aba de documento)
//! ConsultaDados { pagina, por_pagina, ordem, direcao, busca, filtros: Vec<FiltroDigitado> } (Default: 1, 100)
//!     busca_opt · tem_busca_ou_filtro · filtros_banco · POR_PAGINA_OPCOES [50,100,250,500]
//! FiltroDigitado { coluna, op: banco::Operador, valor: String } · para_banco() -> banco::Filtro
//! Navegacao<T = Rota>         ir · substituir · voltar · avancar · pode_voltar · pode_avancar · reiniciar · atual
//!                             atual_mut · entradas_mut · reter(|&T| bool) (poda e funde vizinhas iguais)
//! Abas<T>                     as abas de documento com o histórico único (Navegacao<Entrada>), sem egui:
//!     nova(rota, criar) · abrir(rota, criar) -> Abertura::{Nova, Existente}(IdAba) · nova_consulta(sql, criar)
//!     focar(id) · substituir(id, rota) / atualizar(id, rota) -> Option<AbaAberta> (a fundida) ·
//!     fechar(id, criar_inicio) · fechar_onde(f, criar_inicio) · reiniciar(rota, criar) -> Vec<AbaAberta> ·
//!     reiniciar_menos(rota, criar, manter: Fn(&AbaAberta) -> bool) -> Vec<AbaAberta> (a troca de banco que mantém as
//!     de arquivo: devolve as fechadas, abre a rota ao fim ou foca a mantida de mesma chave, zera o histórico) ·
//!     voltar · avancar · pode_voltar · pode_avancar · vizinha(seguinte) -> Option<IdAba> (sem efeito; o
//!     Ctrl+Tab foca a que ela diz) · id_ativa · ativa · ativa_mut · get ·
//!     get_mut · achar(&ChaveAba) · iter · iter_mut · len        (criar: FnOnce(IdAba, &Rota) -> T) ·
//!     mais_recente_onde(Fn(&AbaAberta) -> bool) -> Option<IdAba> (a focada por último entre as que passam: o CSV de
//!     destino do "Trazer colunas desta planilha")
//! AbaAberta<T>                id() · chave() · rota() · numero_consulta() · pub conteudo: T
//! ChaveAba::{Inicio, VisaoGeral, Parte{nome, parte}, Arquivo(chave_caminho)} · de(&Rota) (None na Consulta)  ·
//!     Entrada { aba, rota }
//! Verbo::{Salvar, Novo, NovaColuna, TrazerColunas, Duplicar, Excluir, Desfazer, Refazer, Buscar, Filtrar, Atualizar,
//!     Executar, Interromper, Exportar}  TODOS (14, a ordem da barra; o TrazerColunas logo depois da NovaColuna,
//!     desligado em toda aba menos no editor de CSV) · rotulo · atalho · icone
//! Oferta { ligado, marcado, principal, ocupado, rotulo, dica } · ligada() · ligada_se · marcada_se · como_principal ·
//!     ocupada_se · com_rotulo · com_dica   (ocupado: o arco girando no lugar do ícone, o Executar da Consulta
//!     executando)
//! Ofertas                     o que a aba oferece de cada verbo (Default: tudo desligado): get · com(verbo,
//!                             oferta) · ligar(verbo) · ligado · rotulo(verbo) · dica(verbo) · iter
//! Acao                        pedidos à casca (Navegar, Atualizar, Substituir, Voltar, Avancar, NovaConsulta,
//!                             FocarAba, FecharAba, MoverAba, AbrirBanco, EscolherEAbrir, NovoBanco, FecharBanco,
//!                             RecarregarObjetos/Banco/Prefs, NotarTransacao, BancoTrocado, LembrarDiretorio,
//!                             TrocarTema, AbrirExportacao, AbrirNovaTabela, AbrirNovoIndice, EscolherCopia) e, da
//!                             etapa 4 (a casca as trata): EscolherEAbrirPasta · AbrirPasta{caminho} ·
//!                             AbrirCaminho{caminho, somente_leitura} (classifica: pasta, banco ou arquivo de dados) ·
//!                             AbrirArquivoDeDados{caminho} (registra o recente e vai para Rota::Arquivo) ·
//!                             ListarPasta{caminho} (uma subpasta da árvore) · AtualizarPasta (a raiz e as abertas) ·
//!                             FecharPasta (tira a árvore e fecha as abas de arquivo de dentro dela; o banco fica)
//! PedidoJanelaExportar { origem: OrigemJanelaExportar::{Tabela{nome, total, filtro, marcadas}, Consulta{sql}, Banco},
//!                        nome_sugerido }  ·  FiltroAtual { busca, filtros, ordem, direcao, linhas }
//! Estado { app, banco, objetos, pasta, prefs, ultimo_diretorio } · banco_id · tem_banco · tem_pasta ·
//!     transacao_aberta · somente_leitura
//! PastaAberta { caminho, nome, chave, nos: HashMap<String, NoDaPasta> } (a pasta aberta no Explorador; independe do
//!     banco aberto)  nova(caminho) (a raiz nasce Lendo) · listagem(chave) -> Option<&comum::arquivos::Listagem> (só a
//!     pronta) · contem(&Path) (a raiz e tudo abaixo dela, pela chave do caminho, por pastas inteiras)
//! NoDaPasta::{Lendo, Pronto(Arc<Listagem>), Falhou(frase)} (PartialEq pelo conteúdo: a casca compara a leitura nova
//!     com a de antes e, igual, não troca o estado; a Listagem da pasta vem sem a data das subpastas)
//! Contexto<'a> { servico, estado, frame, versao, trocando }  novo(…) · com_casca(versao, trocando) ·
//!     com_fontes(&[Arc<dyn FonteDeColunas>]) (opcional; sem ele, a lista é vazia) · fontes() · emitir ·
//!     navegar · banco_id · tarefa(ctx, |servico, id| …) · reagir(&erro) -> Reacao::{Ignorar, Mostrar} ·
//!     aviso_de_gravacao(transacao_aberta, frase) · dialogos()
//! Dialogos                    abrir_banco · abrir_arquivo (os filtros "Arquivos que o DataBox abre", "Bancos SQLite" e
//!                             "Todos os arquivos") · abrir_pasta (a Escolha guarda a pasta de cima) ·
//!                             salvar_como(nome, FormatoArquivo) · escolher_arquivo(titulo) -> Option<Escolha>
//!                             filtros_de_abrir_arquivo() (a lista de extensões do abrir_arquivo; as de banco vêm de
//!                             comum::arquivos::EXTENSOES_BANCO)
//! TabelaAberta { nome, e_visao, completa: servico::TabelaCompleta, rota, recarregar, selecao }
//!     info() · aba() · definir_contagem_filtrada(busca, filtros, n) · contagem_filtrada · pedido_exportacao()
//! MSG_SO_NA_TRANSACAO · LIMIAR_TABELA_GRANDE (200.000) · MSG_INESPERADO
//!
//! documento (etapa 3; o módulo `documento`, com os nomes também na raiz):
//! Anatomia { faixa, rodape, rolagem: Rolagem::{Pagina, Propria} } (Default: a página sem faixa nem rodapé) ·
//!     com_faixa · com_rodape · rolagem_propria          ALTURA_FAIXA (42) · ALTURA_RODAPE (34)
//! BuscaDaBarra<'a> { texto: &'a mut String, dica: Cow<'static, str> } · EventoBusca::{Mudou, Enter, Limpar}
//! trait Documento     anatomia · avisos(doc, ui, ctx) (os `.aviso` antes da faixa) · faixa(ui, ctx) · conteudo(ui,
//!                     ctx) (o único obrigatório) · rodape(ui, ctx) ·
//!                     ofertas(&Estado) -> Ofertas · executar(Verbo, &egui::Context, ctx) · busca() ->
//!                     Option<BuscaDaBarra> · evento_busca(EventoBusca, &egui::Context, ctx) · tem_propriedades ·
//!                     propriedades(ui, ctx) · status(&Estado) -> Option<String> · ocupada ·
//!                     em_segundo_plano(&egui::Context, ctx)
//!                     e, da etapa 5 (os documentos editáveis; todos com padrão, nada quebra): alterado() -> bool
//!                     (false) · alteracoes() -> Option<usize> (None: "Há alterações não salvas.") · salvando() (false) ·
//!                     tomar_erro_de_gravacao() -> Option<String> (uma vez) · status_do_arquivo(&Estado) ->
//!                     Option<StatusDeArquivo> · fonte_de_colunas() -> Option<Arc<dyn FonteDeColunas>> (None)
//! StatusDeArquivo { nome, situacao: Situacao::{Salvo, Alterado, ComErro, SoLeitura}, detalhes: Vec<String> }  novo ·
//!     com_detalhe · rotulo_da_situacao · detalhes_em_linha ("CSV · UTF-8 · CRLF")   Situacao::rotulo ("Salvo",
//!     "Alterado, não salvo", "Com erro", "Somente leitura")
//! InfoArquivo { caminho, tamanho: Option<u64>, modificado: Option<SystemTime> }
//! secao_arquivo(ui, ctx, &InfoArquivo, depois(ui)) · secao_arquivo_com(ui, ctx, &info, primeira, linhas(&mut Detalhes),
//!     depois(ui))  a seção "Arquivo" das Propriedades (Caminho com o Copiar caminho e o toast MSG_CAMINHO_COPIADO,
//!     Tamanho, Modificado em); `linhas` soma linhas à ficha (Codificação, Fim de linha), `depois` fica embaixo dela
//! FonteDeColunas (trait Send + Sync, em `fonte`)  rotulo · tipo() -> TipoDeFonte · nomes · linhas() -> Option<u64> ·
//!     colunas(&[usize], ao_progresso: &mut dyn FnMut(feito, total) -> Resultado<()>) -> Resultado<Vec<Vec<String>>>
//!     (lê disco: sempre numa tarefa; o erro do ao_progresso cancela) · primeira_linha_e_cabecalho() -> Option<bool>
//!     e com_primeira_linha_cabecalho(bool) -> Option<Arc<dyn FonteDeColunas>> (a caixa "Primeira linha é o
//!     cabeçalho" da planilha, etapa 6; `None` por padrão: quem não tem a escolha) · para_o_decimal(char) ->
//!     Option<Arc<dyn FonteDeColunas>> (os números com o decimal do CSV de destino: a planilha; `None` por padrão)
//! TipoDeFonte::{Csv, Planilha, Tabela, Consulta, AreaDeTransferencia}  TODOS · rotulo
//! trait ParteTabela  o mesmo, com a `&mut TabelaAberta` da tela de tabela em cada um (ofertas(&TabelaAberta,
//!                     &Estado)); sem tem_propriedades nem status (os da tela de tabela); mais
//!                     acoes_da_tabela(ui, ctx, &TabelaAberta), o fim da seção "Tabela" ("Renomear a tabela")
//! desenhar_parte_empilhada(parte, ui, ctx, tabela)   faixa, conteúdo e rodapé um embaixo do outro, sem a casca
//! propriedades_no_fim(ui, secoes)   sem o painel, nas telas de Rolagem::Pagina, a casca chama o `propriedades` da
//!     tela no fim da página, depois do fio e do .tit "Propriedades" (decisão 4; a tela não faz nada para isso)
//! secao_tabela(ui, tabela, depois) · tipo_da_tabela(info) · icone_da_tabela(tipo, sem_rowid) (a ficha de cada tipo,
//!     a mesma no Explorador, nas listas e nas janelas)
//! secao_abrir_na_consulta(ui, ctx, nome) (os dois a.atalho de consultas_da_tabela(nome), que navegam à Consulta)
//! busca_da_barra(ui, Option<BuscaDaBarra>, aba) -> Option<EventoBusca>   (a Entrada::busca de 250px; sem busca, a
//!     desligada) · id_busca_da_barra(aba) · focar_busca_da_barra(ctx, aba, texto) (o Ctrl+F, com o texto selecionado)
//! apoio_testes: Ambiente::com_pasta(nome, itens_a_mais) (sem banco aberto, com `pasta_de_dados()`: bancos de verdade,
//!     um sem extensão e um `.dat`, um `.db` que não é SQLite, CSV, XLSX, JSON, TXT, um PDF e subpastas) ·
//!     palco_documento(amb, doc, tema, tamanho) e palco_parte(amb, tabela, parte, tema, tamanho), que
//!     desenham como a casca (a barra com os verbos ligados e a busca, o .doc, as Propriedades à direita ou, com
//!     NoPalco::sem_painel, no fim da página) · abs(relativo) / abs_texto(relativo) (um caminho absoluto do
//!     sistema, `C:\…` ou `/…`, para os testes que não tocam no disco)
//! ```
//!
//! # Regras para as telas
//!
//! - A thread da interface nunca chama o serviço direto: sempre `ctx.tarefa(…)` e `Promessa::tomar` nos quadros
//!   seguintes. Enquanto espera, mostre `interface::pecas::avisos::carregando`.
//! - Erro de tarefa: `ctx.reagir(&e)` primeiro; só mostre quando devolver `Reacao::Mostrar`.
//! - Mudar o próprio endereço (página, busca, filtro, o texto da Consulta SQL): atualize a tela e emita
//!   `Acao::Substituir(rota)` (a casca não refaz a tela e o põe na aba de onde ele
//!   veio). `Acao::Atualizar` existe para quando a mudança deve virar uma entrada do histórico. Ir a outra tela:
//!   `ctx.navegar(rota)` (a casca abre a aba dela ou foca a que já está aberta, sem refazê-la).
//! - Depois de gravar: `ctx.aviso_de_gravacao(resposta.transacao_aberta, "Linha salva.")` para o toast; depois de
//!   mudar a estrutura, `Acao::RecarregarObjetos` (o Explorador mostra as tabelas novas, e as outras abas da mesma
//!   tabela releem quando aparecerem).
//! - A casca sabe de que aba veio cada ação: o `Substituir` e o `Atualizar` valem para a aba que os emitiu, mesmo
//!   que ela já tenha saído de vista. Uma tela escondida não é desenhada: recebe só o `Documento::em_segundo_plano`
//!   a cada quadro, para tomar as respostas que chegaram (a Consulta SQL, a gravação dos Dados, a manutenção da Visão
//!   geral) e dar o toast na hora, sem pedir quadro.
//! - O que várias telas de crates diferentes desenham igual mora aqui, no `documento`: a seção "Tabela" e o "Abrir na
//!   Consulta SQL" das Propriedades, a busca da barra e o ícone de cada tipo de tabela. Não copie para a tela.
//!
//! # Abas e verbos (a casca da etapa 2, com o contrato da etapa 3)
//!
//! - [`Abas`] guarda as abas abertas, na ordem da faixa, e o histórico único de voltar e avançar ([`Entrada`]: aba +
//!   endereço). Uma aba por [`ChaveAba`] (o Início, a Visão geral e cada parte de tabela ou visão); a Consulta SQL
//!   não tem chave, e as Consultas se numeram pelo menor número livre. Voltar e avançar só focam a aba da entrada.
//! - [`Verbo`] são os botões da barra de ferramentas, na ordem fixa de [`Verbo::TODOS`]; cada aba diz o que oferece
//!   de cada um em [`Ofertas`] (o padrão desliga tudo), com rótulo e dica próprios ("Nova tabela", "Exportar
//!   clientes"). Desde a etapa 3, quem diz as ofertas são as próprias telas, pelo trait [`Documento`] (e as partes
//!   de tabela pelo [`ParteTabela`]); o editor só despacha.

#![forbid(unsafe_code)]

mod abas;
mod acao;
#[cfg(any(test, feature = "apoio-testes"))]
pub mod apoio_testes;
mod contexto;
mod dialogos;
pub mod documento;
mod fonte;
mod promessa;
mod rota;
mod tabela;
mod verbo;

pub use abas::{AbaAberta, Abas, Abertura, ChaveAba, Entrada, IdAba};
pub use acao::{Acao, FiltroAtual, OrigemJanelaExportar, PainelCasca, PedidoJanelaExportar};
pub use contexto::{Contexto, Estado, LIMIAR_TABELA_GRANDE, MSG_SO_NA_TRANSACAO, NoDaPasta, PastaAberta, Reacao};
pub use dialogos::{Dialogos, Escolha, FormatoArquivo, filtros_de_abrir_arquivo};
pub use documento::{
    Anatomia, BuscaDaBarra, Documento, EventoBusca, InfoArquivo, MSG_CAMINHO_COPIADO, ParteTabela, PedidoTabela,
    Rolagem, Situacao, StatusDeArquivo, busca_da_barra, consultas_da_tabela, desenhar_parte_empilhada,
    focar_busca_da_barra, icone_da_tabela, id_busca_da_barra, propriedades_no_fim, secao_abrir_na_consulta,
    secao_arquivo, secao_arquivo_com, secao_tabela, tipo_da_tabela,
};
pub use fonte::{FonteDeColunas, TipoDeFonte};
pub use promessa::{MSG_INESPERADO, Promessa, esperar_tarefas, tarefas_em_andamento};
pub use rota::{Aba, ConsultaDados, FiltroDigitado, Navegacao, POR_PAGINA_OPCOES, POR_PAGINA_PADRAO, Parte, Rota};
pub use tabela::TabelaAberta;
pub use verbo::{Oferta, Ofertas, Verbo};
