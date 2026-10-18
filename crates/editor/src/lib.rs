//! O executável **DataBox.exe**: a casca do aplicativo (a barra de título integrada aos menus, com os controles da
//! janela sem decoração, a barra de ferramentas com os verbos da aba, a barra de ícones, o Explorador ou o Histórico
//! de consultas, a faixa de abas de documento com o histórico único, o painel de Propriedades, a barra de status, o
//! aviso de transação aberta, as janelas globais e os toasts), o Início, a Visão geral do banco e o contêiner da tela
//! de tabela,
//! ligando as telas dos outros crates (tela_dados, tela_esquema, tela_consulta, tela_exportar). Ver o §5 do
//! ARQUITETURA.md.
//!
//! # A linha de comando (src/main.rs)
//!
//! ```text
//! DataBox.exe [caminho] [--somente-leitura] [--renderizador glow|wgpu] [--criar-atalho]
//!             [-h | --ajuda] [--versao]
//! ```
//!
//! Sem console (`windows_subsystem = "windows"`): ajuda, versão, erro de argumento e falha fatal aparecem numa
//! caixa de mensagem (rfd). Liga o registro em `servico::arquivo_log()` com o gancho de pânico, abre
//! `Preferencias::abrir(None)`; abre o `Servico`, lê o "Mostrar animações" do Windows
//! (`SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)`, o único `unsafe` do crate, no main.rs, sob
//! `cfg(windows)`; fora dele, sem movimento reduzido) para o `interface::tema::definir_movimento_reduzido`, e
//! abre a janela (título `comum::app::NOME`, 1280×800, mínimo 960×600, ícone `interface::marca::icone_janela`; no
//! Windows, sem a decoração do sistema, com a barra de título própria: `BARRA_PROPRIA` e `com_barra_propria`)
//! com o tema das preferências aplicado antes do primeiro quadro (e o `sync_window_theme` do egui ligado: a borda e
//! a sombra do DWM seguem o tema do app). O caminho da
//! linha de comando (pasta, banco ou arquivo de dados, pelo `Servico::alvo_do_caminho`) é aberto na primeira tarefa;
//! se não abrir, o aplicativo abre sem ele e o Início mostra o motivo. Renderizador glow (OpenGL) por padrão; se o eframe não abrir com ele, o executável se relança com
//! `--renderizador wgpu` e os mesmos argumentos, e sai. `--criar-atalho` cria os atalhos "DataBox" na Área de
//! Trabalho e no Menu Iniciar (para o próprio executável, com o ícone dele) e sai.
//!
//! O build.rs embute no executável, pelo windres, o `recursos/databox.rc`: o ícone (`databox.ico`, montado por
//! `design/ferramentas/ico.py` com os PNG do logo), a versão (DataBox 0.1.0) e o manifesto (`databox.manifest`:
//! controles comuns 6, DPI por monitor, caminhos longos). Com a variável `DATABOX_DIAGNOSTICO_REPINTURA` (ou a
//! antiga, `EDITOR_DIAGNOSTICO_REPINTURA`), o executável registra no log, por segundo, os quadros pintados e por
//! quê.
//!
//! # API pública
//!
//! ```text
//! Editor                         o aplicativo inteiro; impl eframe::App (fn ui chama desenhar com a janela;
//!                                fn logic chama logica)
//!   Editor::novo(servico: Arc<Servico>, tema: interface::tema::Tema, partida: Partida) -> Editor
//!       não toca no disco: o estado (e o banco da partida) chegam numa tarefa no primeiro quadro
//!   com_barra_propria(bool) -> Editor   a janela sem a decoração do sistema: os botões minimizar, maximizar e fechar,
//!       o arrasto e as bordas de redimensionar na barra de título (padrão: cfg!(windows))
//!   com_larguras(prefs::LarguraPaineis) -> Editor   as larguras do Explorador e das Propriedades da sessão de antes
//!       (`prefs.paineis()`, lidas antes da janela); as divisórias as mudam e a casca as guarda ao fim do gesto
//!   desenhar(&mut self, ui: &mut egui::Ui, frame: Option<&eframe::Frame>)
//!       um quadro inteiro na janela (`ui` = a janela toda, sem margem; frame None nos testes, sem diálogos)
//!   titulo() -> &str        "vendas.db · DataBox" (sem banco, "DataBox"); vai para a janela sozinho
//!   rota() -> &Rota · estado() -> &Estado · tema() -> Tema · carregado() -> bool · janela_aberta() -> bool
//!   #[doc(hidden)] com_dialogo(f: FnMut(&PedidoDialogo) -> Option<PathBuf>) -> Editor   (o diálogo nos testes)
//!   #[doc(hidden)] sem_dialogo_nativo() -> Editor   (como se o diálogo do Windows não abrisse: "Pedir o caminho")
//!   #[doc(hidden)] ocupado() -> bool        (há tarefas da casca, com a leitura do Histórico de consultas, ou do
//!                                            documento da aba ativa rodando)
//!   #[doc(hidden)] ir(rota)                 (abre a aba da rota ou foca a aberta, como um clique num link)
//!   #[doc(hidden)] emitir(Acao)             (uma ação sem aba de origem, aplicada no fim do próximo quadro)
//!   #[doc(hidden)] emitir_de(IdAba, Acao)   (uma ação como se o documento daquela aba a emitisse)
//!   #[doc(hidden)] abas_abertas() -> Vec<(String, bool)>   ("clientes · Dados" e se é a ativa, na ordem da faixa)
//!   #[doc(hidden)] id_aba_ativa() -> IdAba · rect_do_documento() -> Option<Rect> (o `.doc` do último quadro)
//!   #[doc(hidden)] propriedades_a_vista() -> bool · exportacao_aberta() -> Option<&PedidoJanelaExportar>
//!   #[doc(hidden)] larguras_desenhadas() -> Option<(f32, f32)> (Explorador e Propriedades do último quadro) ·
//!                  larguras_escolhidas() -> (f32, f32) (a escolha da pessoa, a que vai para as preferências)
//!   #[doc(hidden)] tabela_aberta() -> Option<&TabelaAberta> (a da aba ativa) · trocando() -> bool
//!   #[doc(hidden)] logica(&egui::Context)   (o corpo do App::logic: fechar com a janela minimizada ou encoberta)
//!   #[doc(hidden)] simular_pasta(ctx, Option<PastaAberta>) · acoes_do_explorador() -> Vec<Acao> (o passo 9) ·
//!                  leituras_de_pasta() -> u64 (as leituras de pasta pedidas) · rotas_abertas() -> Vec<Rota>
//!   #[doc(hidden)] com_editor_de_teste(f: Fn(&Path) -> Option<Box<dyn estado::Documento>>) -> Editor (o documento
//!                  de mentira no lugar do editor de um arquivo de dados) · fila_ao_fechar() -> bool (a fila de
//!                  perguntas ao fechar abas alteradas está andando)
//! Partida { caminho: Option<PathBuf>, somente_leitura: bool }   o caminho da linha de comando (Default: nenhum)
//! PedidoDialogo::{AbrirBanco (o "Abrir arquivo"), AbrirPasta, NovoBanco, CopiaDeSeguranca { nome_sugerido }}
//! tema_da_interface(prefs::Tema) -> Tema · tema_das_prefs(Tema) -> prefs::Tema       (pelo texto system|light|dark)
//! aparencia_da_interface(&prefs::Aparencia) -> interface::codigo::AparenciaCodigo
//! aplicar_aparencia(&egui::Context, &prefs::Aparencia)   (cores e tamanho do código na hora; a família não padrão
//!     é lida do disco na hora: o main.rs chama ao subir, ao lado do tema)
//! titulo_da_janela(pasta: Option<&str>, arquivo: Option<&str>) -> String   ("dados · DataBox"; sem pasta, o
//!     arquivo; sem nada, "DataBox") · menu_aberto(&egui::Context) -> bool   (algum dos quatro menus)
//! RegioesJanela { livre: Vec<Rect>, botoes: [Rect; 3], borda: f32 } · regioes_da_janela(&egui::Context)
//!     as regiões da barra própria no último quadro (a porta para um WM_NCHITTEST próprio)
//! linha_comando::{analisar(&[OsString]) -> Result<Comando, ErroArgumentos>, Comando::{Executar(Argumentos),
//!     Ajuda, Versao}, Argumentos { caminho, somente_leitura, renderizador, criar_atalho }, Renderizador::{Glow, Wgpu},
//!     ErroArgumentos { mensagem } · texto(), argumentos_para_relancar(&Argumentos, Renderizador), USO, AJUDA}
//! registro::{iniciar(PathBuf) -> bool, instalar_gancho_de_panico(), RegistroCompartilhado::novo(arquivo,
//!     max_bytes, copias) · escrever(linha) · com_adiamento, Registro (log::Log), formatar_linha, nivel_do_alvo,
//!     MAX_BYTES (1.000.000), COPIAS (2), ADIAR_ROTACAO (60 s)}
//! atalho::{criar_atalhos(exe, Option<&[PathBuf]>) -> Result<Vec<PathBuf>, String> (fora do Windows, Err),
//!     script, base64_utf16, NOME_ATALHO ("DataBox.lnk"), DESCRICAO}
//! ```
//!
//! # O que a casca faz a cada quadro ([`Editor::desenhar`])
//!
//! 1. Aplica o tema escolhido (sem custo quando não mudou) e espera as fontes; no primeiro quadro lança a tarefa
//!    da partida (abrir o banco da linha de comando e ler o estado: `servico.estado()`, os objetos e a última
//!    pasta).
//! 2. Recebe as tarefas: o estado, a troca de banco, as releituras (objetos, BancoInfo, preferências, tema), a ação
//!    do toast (o "Mostrar na pasta" da cópia de segurança) e as abas escondidas (`Documento::em_segundo_plano`: as
//!    Consultas, os Dados e a Visão geral tomam o que chegou e dão o toast na hora, sem desenhar).
//! 3. Atalhos (`casca::atalhos`, sem janela aberta, com os modificadores exatos): Ctrl+O e Ctrl+Shift+O (fecham o
//!    menu e abrem o diálogo do arquivo e o da pasta), Ctrl+F (a busca da barra, quando a aba tem busca), Ctrl+, (as Preferências), Alt+←/→ e os botões
//!    laterais do mouse (o histórico único, que atravessa as abas), Ctrl+W (fecha a
//!    aba), Ctrl+Tab e Ctrl+Shift+Tab (a aba seguinte e a anterior, com volta: `Abas::vizinha`) e F5 (o Atualizar
//!    da aba, só quando ela o oferece): a edição em curso grava agora e a ação vale no quadro seguinte; Ctrl+B e
//!    Ctrl+Alt+B mostram ou escondem o Explorador e as Propriedades na hora (o Ctrl+Alt+B só com o botão delas
//!    ligado, e fica com o campo de texto focado, por causa do AltGr). Fechar a janela com uma exportação em
//!    andamento ou com a transação aberta pergunta antes (`ViewportCommand::CancelClose` e a confirmação;
//!    "Descartar e continuar" fecha). Com a janela minimizada ou encoberta, o eframe não desenha quadro e só chama o
//!    `App::logic`, e as respostas que chegaram esperam um quadro: com uma exportação, a transação aberta ou uma
//!    Consulta que executa (também a execução solta), o `logica` cancela o fechar e restaura a janela
//!    (`Minimized(false)` e `Focus`); a decisão sai no fim do primeiro quadro dela, depois de as respostas
//!    acertarem a transação.
//! 4. Desenha o fundo e, de cima para baixo (as medidas das pranchetas): a barra de título de 30px
//!    (`casca::janela`: o logo, os menus Arquivo, Editar, Exibir e Ajuda e o título no centro; ver "Barra de título e
//!    menus"), a barra de ferramentas (`casca::ferramentas`: `margin:6px 8px 0`, 46px por fora, na ordem fixa Voltar
//!    · Avançar | Abrir pasta · Abrir arquivo · Salvar | Novo · Nova coluna · Duplicar · Excluir · Desfazer ·
//!    Refazer | busca · Filtrar · Atualizar | Executar · Interromper · Nova consulta · Exportar | Painel de
//!    propriedades; os verbos da aba vêm do `Documento::ofertas` e o clicado é executado no documento antes de ele
//!    se desenhar; o Interromper tem o lugar dele em toda aba e só liga com a Consulta executando; a busca é a do
//!    documento, `Documento::busca`; ela é desenhada antes da barra de ícones, que é a ordem do Tab), o corpo
//!    (`padding:8px; gap:8px`) e a barra de status. No corpo: a barra de ícones de 52px (`casca::icones`: Início,
//!    Explorador e Histórico de consultas, e as Preferências no pé), o painel da esquerda (`casca::explorador`, a
//!    árvore da pasta aberta, com o banco aberto no lugar dele ou na segunda raiz, ou só a do banco, com rolagem
//!    própria; o menu de contexto, o Shift+F10 e o Ctrl+Shift+C nos nós; o clique duplo numa pasta a deixa aberta, o →
//!    num banco fechado não troca de banco, e fechar a pasta esquece os nós abertos dela; ou `casca::historico`, os
//!    comandos já executados; a escolha fica só na sessão), a seção do documento (a faixa de abas de 35px, `casca::abas`: o clique foca, o × e o clique do meio
//!    fecham, o "+" abre uma Consulta, tudo no quadro seguinte; e o `.doc` embaixo, que forma uma superfície só com
//!    a aba ativa, pela peça `interface::pecas::documento`: no alto o aviso da troca de banco que passou de 0,3 s e o
//!    de transação, fora das Consultas, depois a faixa de 42px, o conteúdo, com a rolagem da casca, de cada aba, ou a
//!    própria, e o rodapé de 34px, conforme a `Anatomia` do documento) e as Propriedades (`casca::propriedades`;
//!    não aparecem no Início nem quando o documento ficaria com menos de 560px, sem mudar a escolha da pessoa;
//!    escondidas, nas telas de rolagem de página as seções vão para o fim do documento,
//!    `estado::propriedades_no_fim`). O painel da esquerda (258px) e as Propriedades (272px) mudam de largura pelas
//!    divisórias (`casca::larguras`, de 200 a 45% da janela; duplo clique volta à padrão), e as larguras escolhidas
//!    ficam nas preferências (`"larguras_paineis"`). A barra de status (`casca::status`, 26px com `margin:0 8px 8px`): o
//!    banco (sem ele, a pasta aberta com o caminho na dica; sem nada, "Nenhuma pasta aberta"), o modo de gravação
//!    com a frase na dica, o que a aba diz de si, o SQLite e a codificação (sem o tamanho
//!    do arquivo, que está nas Propriedades). Depois as janelas globais (Exportar, Nova tabela, Novo índice,
//!    Preferências, Atalhos do teclado, Sobre o DataBox, Pedir o caminho), a confirmação, o toast (acima da barra de
//!    status) e, por cima de tudo (também da modal), os controles da janela sem decoração. Toda camada que se
//!    sobrepõe usa o vidro fosco (`interface::fosco`, ligado no criador do app).
//! 5. Aplica as `estado::Acao`, cada uma com a aba que a emitiu: `Navegar` (sem banco, toda rota vira o Início;
//!    abre a aba da rota ou foca a aberta; a Consulta SQL sem comando foca a que abriu a transação, depois a mais
//!    recente), `Atualizar`/`Substituir` (só o endereço da aba de origem; se ele cai na chave de outra aba aberta,
//!    as duas viram uma), `Voltar`/`Avancar` (só focam), `NovaConsulta`, `FocarAba`, `FecharAba` (fechar a última
//!    abre o Início; a execução da Consulta que ainda corre segue numa tarefa da casca),
//!    `AbrirBanco`/`EscolherEAbrir`/`NovoBanco`/`FecharBanco` (uma troca de cada vez, só depois de o estado
//!    chegar; 409 `TransacaoAberta` → a confirmação "Descartar e continuar" e a mesma troca com
//!    `descartar_transacao`; no fim, o estado relido, todas as abas fechadas, inclusive as Consultas, e a Visão
//!    geral do banco novo, ou o Início), `RecarregarObjetos` (vindo de uma aba de tabela, as outras abas da mesma
//!    tabela releem; de outro lugar, todas as abas de tabela e a Visão geral, menos a de origem: as escondidas quando
//!    aparecerem, a ativa no fim do quadro seguinte, depois de a célula em edição gravar; a Visão geral só pede de
//!    novo o banco e os objetos, sem perder a manutenção em andamento; a lista nova fecha as abas das tabelas que
//!    sumiram, menos a ativa, que relê e mostra o card de inexistente; a execução solta só o emite quando o comando
//!    mudou dados ou estrutura ou fechou a transação), `RecarregarBanco/Prefs` (a resposta de um pedido
//!    feito para outro banco é descartada, com o erro), `NotarTransacao` (quando muda, as outras Consultas esquecem
//!    a última resposta delas e o BancoInfo é relido; vindo de uma Consulta, o Histórico de consultas à vista se
//!    relê), `BancoTrocado` (toast `MSG_BANCO_TROCADO`, fecha as janelas
//!    — no começo e no fim da releitura —, relê o estado: o mesmo arquivo mantém as abas e refaz cada documento
//!    pela rota dela, a Consulta com o texto; outro vai à Visão geral dele, nenhum ao Início; durante uma troca
//!    pedida aqui não faz nada — a troca traz o estado novo — e uma releitura pendente é descartada quando a troca
//!    termina), `LembrarDiretorio`, `TrocarTema` (aplica na hora e grava com `gravar_prefs`), as três janelas
//!    globais e as da pasta (abaixo, "A pasta"). Por fim, o título (também para a barra de tarefas e o Alt+Tab) e o foco devolvido ao botão "Ajuda"
//!    (quando a janela "Atalhos do teclado" ou a "Sobre o DataBox" fechou: só um quadro depois de a camada modal sumir
//!    o egui deixa o botão ter foco).
//!
//! A thread da interface nunca chama o serviço nem toca no disco: tudo vai por `estado::Promessa` (que acorda a
//! interface ao terminar); parada, a tela não pede quadros (só as animações das peças; uma troca de
//! banco em andamento pede um quadro aos 0,3 s e aos 2 s, para o card dela). As abas escondidas não são desenhadas
//! nem pedem quadro. Os diálogos do Windows são os do `estado::Dialogos` (rfd, modais, com a janela como dona).
//!
//! # A pasta (etapa 4)
//!
//! - `EscolherEAbrirPasta`: o diálogo "Abrir pasta" (`PedidoDialogo::AbrirPasta`; a pasta de cima vira a última usada)
//!   ou, sem ele, a janela "Pedir o caminho" ("Abrir pasta", "Caminho da pasta", `C:\pasta`). `AbrirPasta`:
//!   `Servico::abrir_pasta` numa tarefa (recente, `pasta_aberta`); a pasta de antes fica à vista até a nova chegar, e
//!   a raiz volta resolvida (sem nomes 8.3), como o caminho do banco aberto. `AbrirCaminho`: o
//!   `Servico::alvo_do_caminho` numa tarefa, que vira a pasta, o banco (com o `somente_leitura`) ou a ficha; um nome
//!   com extensão de banco que não se reconhece abre como banco (o erro do SQLite diz por quê). O Ctrl+O e a janela
//!   "Pedir o caminho" do Abrir também classificam. `AbrirArquivoDeDados`: o `registrar_arquivo` (recente) e a
//!   navegação para `Rota::Arquivo`. O erro vai à janela, ao Início (quando a ação veio dele) ou ao toast.
//! - `ListarPasta`: lê uma pasta da árvore numa tarefa (`Tarefa::Pasta`, com a chave da pasta aberta e a geração dela:
//!   a leitura de outra pasta, ou da mesma pedida antes de um Fechar pasta, é descartada); a já lida fica à vista e
//!   reaproveita os cabeçalhos; a mesma em andamento não é pedida de novo; a mesma listagem não muda o estado.
//!   `AtualizarPasta`: a raiz e as subpastas já lidas (também as que falharam). `FecharPasta`: tira a árvore, fecha as
//!   abas de arquivo de dentro dela e apaga a `pasta_aberta` (o banco continua); a abertura de pasta pedida antes dele
//!   chega e é desfeita (a chave sai de novo), e a ficha pedida de um arquivo de dentro dela não abre.
//! - Partida: a `pasta_aberta` das preferências volta (nunca o banco), com a leitura da raiz pedida sem esperar; a
//!   que sumiu fica com "Não foi possível ler a pasta." e o "Tentar de novo". A pasta da linha de comando vale por ela.
//! - Releitura automática (sem temporizador): na volta do foco da janela (`RawInput::focused`), no máximo uma a cada 2 s e
//!   nunca com uma leitura em andamento; no fim de uma troca de banco, quando uma janela global fecha e quando a cópia
//!   de segurança termina, sem o espaçamento (o app pode ter criado o arquivo), e com uma leitura em andamento ela
//!   espera (`reler_pendente`).
//! - A troca de banco fecha as abas do banco e mantém as de arquivo de dados (`Abas::reiniciar_menos`). Título:
//!   "dados · DataBox" com a pasta.
//! - Soltar na janela (`RawInput::dropped_files`): um item só abre, a primeira pasta ou, sem pasta, o
//!   primeiro que o app abre, classificado como o `AbrirCaminho` numa tarefa; com mais de um, o toast de atenção "Só um item abre
//!   por vez; abri dados."; com uma janela ou modal aberto, nada. Sem o vidro "Solte para abrir" (sem prancheta); o
//!   Windows não entrega o que vem de um Explorador elevado a um app comum (UIPI).
//!
//! # Trocas de banco e janelas globais
//!
//! - **Troca demorada**: abrir, criar e fechar esperam o banco atual soltar a conexão (Consulta SQL demorada,
//!   VACUUM, cópia, exportação com transação aberta). Passados 0,3 s, um card no alto do conteúdo mostra o
//!   `.carregando` "Abrindo o banco…" / "Criando o banco…" / "Fechando o banco…"; depois de 2 s, o texto diz que
//!   espera a operação em andamento terminar. Um segundo pedido de troca enquanto isso é ignorado.
//! - **Pedir o caminho** (`pedir_caminho.rs`): o rfd devolve `None` tanto no cancelar
//!   quanto quando o diálogo não abre; um `None` em menos de 150 ms não foi uma pessoa (`recursos::
//!   LIMIAR_SEM_DIALOGO`) e, daí em diante, a casca usa a janela "Abrir arquivo" / "Abrir pasta" / "Novo banco" (com novo.db na
//!   pasta do banco) / "Salvar como" (a cópia de segurança, com o nome sugerido na pasta do banco), campo
//!   "Caminho do arquivo" com o "i" e o botão primário "Abrir" / "Criar" / "Salvar". Vazio → "Digite o caminho do
//!   arquivo."; abrir e criar rodam com a janela aberta (o erro aparece nela; desistir da confirmação de
//!   transação a fecha); a cópia volta à Visão geral, que pergunta antes de substituir um arquivo que existe
//!   ("Já existe X. Substituir? O conteúdo atual do arquivo se perde.", `Codigo::DestinoExiste`).
//! - **Novo índice** (a janela que a dica de ordenação lenta da aba Dados abre): ao fechar depois de criar o
//!   índice (ela emitiu `RecarregarObjetos`), as abas daquela tabela releem (`TelaTabela::recarregar`; a ativa na
//!   hora) e a dica some.
//!
//! # Telas deste crate
//!
//! Cada tela (estas e a Consulta SQL) é o `Documento` de uma aba (`documento.rs`, guardado nas `estado::Abas`): uma
//! aba por parte de tabela ou visão ("clientes · Dados", "clientes · Estrutura"), uma Visão geral, um Início e
//! quantas Consultas SQL a pessoa abrir ("Consulta 1", "Consulta 2", numeradas pelo menor número livre). Cada
//! Consulta nasce com o id da aba como sal dos ids (`TelaConsulta::com_id`): o texto, o cursor e o desfazer não
//! passam de uma para outra. A execução que ainda corre quando a aba fecha segue numa tarefa da casca com o id do
//! banco para que foi pedida (numa troca de banco, o de antes: a resposta é descartada) e, no fim, ainda avisa a
//! transação e os objetos. Todas as Consultas usam a mesma conexão: um BEGIN numa vale para as outras.
//!
//! - **Início** (prancheta 4): os cartões "Abrir pasta", "Abrir arquivo" (diálogo) e "Novo banco", "Abrir bancos só
//!   para leitura", o caminho digitado, de pasta ou de arquivo (Enter ou "Abrir"; aspas e espaços em volta saem;
//!   vazio → "Digite o caminho completo da pasta ou do arquivo."), o erro de abertura numa `.notice.perigo` do card (o banco da linha de comando que não abriu:
//!   "Não foi possível abrir o banco X." + o motivo + "O DataBox abre sem ele.", separados por
//!   linhas em branco), o aviso "X está aberto. Abrir outro fecha este." + "Ir para o banco", e os recentes na
//!   tabela do kit (nome que abre; o que sumiu é um `strong` que não abre, na linha inativa com a pill "não
//!   encontrado"; pasta encurtada com a dica do caminho completo; tamanho, data e os botões de ícone "Mostrar na
//!   pasta" e "Remover da lista", os `.ib.pq` da prancheta do Início; o estado vazio). A coluna Pasta fica com a
//!   sobra; numa janela estreita, com a sobra de verdade (até o próprio padding, sem o título quando nem "PASTA"
//!   cabe), medida a partir das larguras das outras colunas no quadro anterior: as ações ficam dentro do card a
//!   960px, com o Explorador ao lado.
//! - **Visão geral** (prancheta 1, `visao_geral.rs`): desenha do `Estado` (o BancoInfo e os objetos que a casca já
//!   mantém; o Atualizar pede os dois de novo). Na faixa, o cilindro, o nome, o caminho com a reticência no meio e a
//!   pill "só leitura"; no conteúdo, os contadores na dock (`pecas::dock`: Tabelas, Visões, Índices, Gatilhos,
//!   Tamanho e Linhas, com os ícones `-grande` de 48pt na prateleira, no singular com 1, "Linhas (contadas)" com a
//!   dica "Tabelas grandes não são contadas ao abrir; a soma é das que foram contadas.") e a lista Tabelas (Nome com
//!   a ficha pelo tipo, Linhas, Colunas, Tipo e o "Estrutura"), filtrada pela busca da barra ("Buscar tabelas"). Na
//!   barra, "Nova tabela" (o Novo) e "Exportar o banco". Nas Propriedades, "Arquivo" (os nove campos, o Caminho com o
//!   Copiar caminho, toast "Caminho copiado.") e "Manutenção" (verificações, Otimizar, Compactar com confirmação e
//!   "De X para Y.", cópia de segurança pelo diálogo, que a casca abre com `Acao::EscolherCopia`, ou pela janela
//!   "Pedir o caminho"; só leitura: só as verificações e a cópia); o resultado vem num toast ("Cópia salva: x.db ·
//!   119 MB" com a ação "Mostrar na pasta"), também com a aba escondida (`em_segundo_plano`) ou fechada (a casca
//!   espera a execução no lugar da tela, `Documento::execucao_ao_fechar`).
//! - **Tabela/visão** (o contêiner, `tabela.rs`): lê `servico.tabela` ("Lendo a tabela…"), corrige o endereço se o
//!   objeto é do outro tipo ou a aba não existe nele (`Acao::Substituir`), monta a `TabelaAberta` e entrega a parte
//!   ao crate dela pelo `estado::ParteTabela` (a faixa, o conteúdo e o rodapé no documento da casca); junta o
//!   Atualizar e o Exportar ("Exportar clientes", `tabela.pedido_exportacao()` com o nome do arquivo limpo) às
//!   ofertas da parte; nas Propriedades, a seção "Tabela" (`estado::secao_tabela`) antes das da parte e o "Arquivo"
//!   curto depois; no status, "clientes · 60 linhas · 7 colunas" (+ "sem rowid"/"visão"; a contagem de tabela grande
//!   chega depois). `tabela.recarregar` relê a tabela e os objetos e refaz a parte; a tabela que não existe mais
//!   vira o card "A tabela X não existe mais neste banco." com o link para a visão geral logo embaixo. Uma leitura
//!   que volta `BancoTrocado` deixa a tela esperando (sem pedir de novo com o id velho) até a casca ressincronizar e
//!   refazê-la.
//!
//! - **Consulta SQL**: `TelaConsulta` (do crate dela) com o `sql` da rota.
//! - **Ficha do arquivo de dados** (`tela_arquivo.rs`, `Rota::Arquivo`; só no build sem o recurso `telas`): na faixa,
//!   a folha do tipo, o nome e o caminho; no conteúdo, o aviso "O editor de CSV ainda não está pronto. Enquanto isso,
//!   o arquivo abre no programa padrão do Windows." e os botões "Abrir no programa padrão" e "Mostrar na pasta" (a
//!   casca cumpre: `Servico::abrir_arquivo` e `revelar`, nos testes o gancho); o arquivo que sumiu mostra o erro e
//!   desliga os botões. Propriedades "Arquivo" (Tipo, Tamanho, Modificado em, Caminho com o Copiar caminho); status
//!   "CSV · 96 KB"; nenhum verbo. A troca de banco mantém a aba; o Fechar pasta fecha as de dentro da pasta.
//! - **Editores de arquivo** (etapa 5, `tela_arquivo::TelaArquivo`): a aba de arquivo escolhe a tela pelo
//!   tipo (CSV → `Csv`, JSON e texto → `Texto`, a `tela_texto::TelaTexto`; o resto → a ficha), pelos pontos de
//!   encaixe `editor_de_csv` (a ficha até o `tela_csv` chegar) e `editor_de_texto`. A casca liga tudo pelo trait
//!   `estado::Documento`: o ponto da aba alterada, a barra de status do arquivo (`status_do_arquivo`), Ctrl+S (também
//!   com campo de texto focado), Ctrl+Z e Ctrl+Y/Ctrl+Shift+Z (só sem campo de texto focado), o Salvar, o Desfazer,
//!   o Refazer e o Trazer colunas da barra e dos menus pelas ofertas, as origens de colunas das abas abertas no
//!   `Contexto::fontes` (montadas a cada quadro), o F5 (a tela pergunta "Descartar as alterações…?" ela mesma), os
//!   botões da ficha do editor que não abriu o arquivo e a **fila de perguntas ao fechar**:
//!   fechar a aba (×, clique do meio, Ctrl+W, Arquivo › Fechar aba), o Fechar pasta e o pedido de fechar a janela
//!   (também minimizada) perguntam "Salvar as alterações em {nome}?" (Não salvar · Cancelar · Salvar) por aba
//!   alterada que vai fechar, com a aba à frente; Salvar grava e espera (também a pergunta da própria tela, o
//!   "Salvar mesmo assim", pelo `pergunta_aberta`), o erro dá o toast e para tudo, Cancelar e Esc
//!   param tudo; os arquivos vêm antes da exportação e da transação. A troca de banco não pergunta (as abas de
//!   arquivo ficam).
//!
//! # Barra de título e menus
//!
//! A faixa de 30px (`casca::janela`) com o logo de 16px, os menus (`casca::menus`, sem reticências) e o título no
//! centro ("exemplo.db · DataBox"), esmaecido com a janela inativa:
//! - **Arquivo** (com os sólidos de 24): Abrir pasta (Ctrl+Shift+O), Abrir arquivo (Ctrl+O), Novo banco,
//!   "Abertos recentemente" (o submenu com os recentes que existem, menos o banco aberto e a pasta aberta, com o
//!   ícone de cada tipo, e o clique que abre pelo tipo; sem nenhum, desligado), Salvar (o verbo da aba), Fechar aba
//!   (Ctrl+W), Fechar banco, Fechar pasta (com uma pasta aberta) e
//!   Preferências (Ctrl+,: `casca\preferencias.rs`, prancheta 13, com Geral, Aparência, Consulta SQL e Exportação,
//!   que valem na hora e gravam no Salvar);
//! - **Editar**: os verbos da aba, das mesmas `Ofertas` da barra de ferramentas (desligado o que a aba não oferece; o
//!   texto é o da oferta, "Nova tabela"), executados no documento antes de ele se desenhar;
//! - **Exibir**: Explorador (Ctrl+B), Propriedades (Ctrl+Alt+B; desligado quando somem por falta de largura e no
//!   Início) e Histórico de consultas, marcados quando à vista, e Tema ▸ (Seguir o Windows, Claro, Escuro e
//!   "Preferências de aparência", que abre as Preferências na Aparência);
//! - **Ajuda**: Atalhos do teclado e Sobre o DataBox (casca\sobre.rs, prancheta 13: o logo de 96, a versão, a ficha com Compilação (o `DATABOX_COMPILACAO` do build.rs, do git), Renderizador (`Editor::com_renderizador`), SQLite, Sistema (`std::env::consts`), Licença, Preferências e Arquivo de log (links que abrem a pasta e o arquivo), e as janelas Licença (o LICENSE embutido) e Licenças de terceiros (a lista do Cargo.lock, também feita pelo build.rs); Copiar informações).
//!
//! Pelo teclado, com um menu aberto, ↑ e ↓ andam pelos itens ligados dele (com volta, sem sair do painel) e o Esc o
//! fecha e devolve o foco ao botão.
//!
//! Com a barra própria (Windows), os controles da janela ficam numa camada `Order::Tooltip`, acima da modal: o espaço
//! livre (o logo e o trecho do título) arrasta a janela e o duplo clique maximiza; os botões Minimizar, Maximizar (ou
//! Restaurar) e "Fechar a janela" (o pedido de fechar de sempre, com a pergunta da exportação e da transação); faixas
//! de 5px por dentro da borda redimensionam (não com a janela maximizada nem com um popup aberto, um menu ou uma
//! lista; as dos lados começam embaixo da barra de título, e o Fechar fica inteiro). O espaço livre e as faixas não são paradas do Tab nem nós de
//! acessibilidade. Sem o `WM_NCHITTEST` (o winit 0.30 não o trata; o winit direto e o subclassing ficaram de fora por
//! decisão de 16/10/2026), perdem-se o Snap Layouts ao passar o mouse no Maximizar e o menu do sistema no clique
//! direito do título.
//!
//! # Recurso `telas`
//!
//! Ligado por padrão: as telas e janelas vêm dos crates de tela. Desligado (`--no-default-features`), o módulo
//! `substitutos` (cópias dos esboços, com a mesma API do §4.8 de ARQUITETURA.md) toma o lugar deles; o
//! substituto da janela Novo índice cria de verdade `idx_<tabela>_<coluna>` pelo "Criar índice" (para o teste
//! da releitura da tabela valer com e sem o recurso).
//!
//! # Testes
//!
//! `src/` (unitários): linha de comando, registro (com outra instância segurando o arquivo), atalho (criado de
//! verdade numa pasta temporária e relido pelo WScript.Shell), textos (subtítulo, nome base, filtro sem
//! acento, o erro da partida, o card da troca, a pergunta de substituir, a janela "Pedir o caminho"), diálogos
//! (sem o nativo), o lugar da dica do tile, a tela de tabela (`BancoTrocado` não relança a leitura; recarregar), o
//! fechar com a janela minimizada (`logica`, pelo `egui::Context::run_logic`), os atalhos da casca (modificadores
//! exatos, AltGr, o Ctrl+B e o Ctrl+Alt+B que não tiram a edição do campo, o F5, o Ctrl+Alt+B que fica com o campo
//! de texto) e a janela de atalhos, os documentos (rota, título, o refeito, o que vai na barra de status, as ofertas
//! de cada um com a Nova coluna sempre desligada, o que cada verbo pede à casca), o ícone de cada verbo, os textos
//! e o tipo da tabela das Propriedades, as abas na casca (navegar abre ou foca, o `Substituir` na aba de origem, a
//! Consulta da transação, os objetos que sumiram), o Explorador (os pais da aba à vista que se abrem, o que volta ao
//! começo noutro banco), o Histórico (o resumo e a data), a barra de status (as três frases do modo, com e sem
//! banco), os painéis (o da esquerda e as Propriedades), os recentes do menu (sem o aberto nem o que sumiu), o
//! título da janela (pasta, arquivo, nada), os menus (o Editar com todos os verbos menos o Salvar, os temas do
//! Exibir) e a barra própria (`direcao_da_borda` nos lados, nos cantos, dentro e na quina do Fechar; as faixas dos
//! lados embaixo da barra de título; o trecho do título entre os menus e os botões), a releitura da ativa pedida por
//! outra aba, a execução solta que só recarrega quando mudou algo, o fechar minimizado com a resposta do BEGIN
//! ainda não tomada e os atalhos que não valem (o F5 sem o Atualizar, o Ctrl+Alt+B sem o botão).
//! `tests/foco.rs`: o Tab só em peças com nome e na ordem da casca (a barra de ferramentas antes da de ícones, os nós
//! do Explorador sem a seta no meio), o Tab com a Exportar aberta, as setas do Explorador e as do menu aberto, e o Esc
//! do menu que devolve o foco ao botão.
//! `tests/casca.rs`: o `Editor` inteiro num `egui_kittest::Harness` sobre o `estado::apoio_testes::Ambiente`,
//! com o diálogo falso (ou sem o nativo): a casca de ponta a ponta (inclusive o commit que
//! tira o aviso, o layout a 960×600 e 1024×700 medido dentro do `.doc`, com as Propriedades escondidas e de volta
//! sem o Explorador, a cópia com caminho longo a 1024px, o foco que volta ao menu e o fechar com a janela
//! minimizada, que pergunta no quadro seguinte), a barra de ferramentas (a ordem fixa e o mesmo lugar em toda aba,
//! a Nova coluna desligada com a dica, os verbos de cada aba, o Exportar com a origem certa, Nova tabela, Atualizar
//! e F5, Executar, Voltar e Avançar, os desligados que não respondem, só leitura), as Propriedades (a ficha de cada
//! aba, a transação, o botão e o Ctrl+Alt+B, o Início sem elas), a barra de ícones, o Explorador (partes,
//! marcada, Recolher tudo, Ctrl+B, o filtro com muitas tabelas, o banco sem o ponto e, com a transação aberta, o
//! Atenção com a dica "Transação aberta"), o Histórico (seleciona e abre uma Consulta), a barra
//! de status (a frase do modo lida na dica), a barra de título (Minimizar, Maximizar e Restaurar com a janela
//! maximizada, sem a borda, "Fechar a janela" com a pergunta da transação, o duplo clique no espaço livre e no logo,
//! o arrasto que só começa no espaço livre, o cursor e o `BeginResize` nas bordas e nos cantos, os controles que
//! respondem com a Exportar aberta, o título no centro e esmaecido com a janela inativa, a faixa sem a barra
//! própria), os menus (Arquivo com os itens da prancheta e o Fechar aba, Editar que espelha a barra e executa,
//! Exibir com os painéis e o Tema que manda o `SetTheme`, Ajuda com os Atalhos e o Sobre, o foco que volta ao
//! "Ajuda"), os Abertos recentemente do menu, os fluxos das abas (partes, fechar
//! pelo ×, pelo meio e pelo Ctrl+W, Ctrl+Tab, o histórico que pula a fechada, a rolagem de cada aba, três Consultas
//! com textos separados, a transação vista por todas as Consultas, fechar a Consulta que executa, fechar e reabrir
//! o banco, as tabelas renomeadas, excluídas e mudadas, a célula em edição que grava antes de a aba sair de vista,
//! a aba à vista que relê depois de uma Consulta escondida ou de uma execução solta e mostra o card da tabela que
//! sumiu, a Visão geral escondida que guarda a manutenção, o fechar minimizado logo depois de um BEGIN, o F5 sem o
//! Atualizar, o toast acima da barra de status, a direita do Fechar e a borda que sai com um menu aberto; as Consultas
//! que precisam ficar executando esperam um outro programa soltar o banco, sem depender da velocidade da máquina)
//! e as capturas em `capturas/rust/` com os nomes das de referência e, sem referência: inicio-erro,
//! inicio-erro-caminho, inicio-com-recentes-960/1024, inicio-com-3-recentes-960/1024, inicio-com-4-recentes-1024,
//! banco-visao-geral-960/1024 (e o fim da página, -fim, com as Propriedades), banco-visao-geral-960-escuro,
//! banco-visao-geral-sem-painel, banco-visao-geral-so-leitura, banco-integridade, banco-vacuum, banco-copia,
//! banco-copia-painel, propriedades-visao-geral-manutencao, confirmar-vacuum, confirmar-transacao, confirmar-substituir, menu-sem-banco,
//! menu-arquivo-aberto-escuro, menu-arquivo-recentes, menu-editar, menu-exibir-tema-escuro, janela-sobre,
//! barra-titulo-maximizada, barra-titulo-inativa, aviso-transacao-escuro,
//! tabela-inexistente, visao-dados, pedir-caminho, pedir-caminho-erro, troca-esperando, casca-abas,
//! casca-consulta-3, casca-consulta-barra, casca-visao-geral, casca-960-propriedades, casca-1024-propriedades,
//! barra-de-ferramentas, propriedades-visao-geral, propriedades-tabela, explorador-expandido,
//! explorador-com-muitas-tabelas, explorador-filtrado, explorador-rolado-ate-o-fim e historico (todas `-claro.png`,
//! menos as `-escuro`). As cenas das pranchetas (só com o recurso `telas`), para a comparação a olho com
//! `design/canvas`: casca-inicio-claro e casca-inicio-escuro (o Início com os recentes; no escuro, com o Arquivo ›
//! Abertos recentemente da prancheta 4), casca-visao-geral-escuro (três abas e o Exibir › Tema, prancheta 1),
//! casca-dados-abas-escuro (a aba de Dados com o filtro entre a Visão geral, a Estrutura e uma Consulta, prancheta 2),
//! casca-consulta-2-escuro (o roteiro de três comandos com o Histórico à esquerda, prancheta 3),
//! casca-transacao-status-claro (a Estrutura com a transação aberta, prancheta 6) e casca-960-claro. As renderizações
//! com wgpu passam uma de cada vez (uma trava no apoio dos testes): muitas juntas estouravam o prazo do wgpu.
//! `tests/diagnostico_repintura.rs`: parado não pede quadro, com várias abas e duas Consultas abertas, com o
//! Histórico à vista (vazio e com comandos) ou sem o painel da esquerda, com o Explorador numa parte à vista, com as
//! Visões abertas, recolhido, com o campo de filtro (mais de 12 tabelas), filtrado e rolado até o fim, com as
//! Propriedades escondidas, à vista ou sumidas por falta de largura (a 960px, no tema escuro), com a transação aberta
//! (o modo na barra de status, o aviso e as Propriedades da Consulta e da Visão geral), com o Arquivo › Abertos
//! recentemente, o Exibir › Tema e o Editar abertos, com as janelas Atalhos do teclado e Sobre o DataBox, e depois de
//! o ponteiro passar pela faixa de abas, pelo Explorador, pelo Histórico, pela barra de ícones, pela de ferramentas
//! (um botão ligado, um desligado com dica, o Painel), por uma seção das Propriedades, pela barra de status (o modo,
//! com a transação aberta) e pela barra de título (os botões da janela, um menu, o espaço livre e a borda de
//! redimensionar).
//!
//! # Pedidos à interface (registrados aqui; resolvidos dentro deste crate)
//!
//! - `pecas::tabela`: deixar a coluna que estica encolher abaixo de 60px (o `max-width:0` do `td.caminho-curto`) e
//!   deixar o `th` quebrar em duas linhas; hoje o Início mede
//!   as colunas e passa a Pasta como `fixa`.
//! - `pecas::navegacao::segmentado` e `pecas::tiles::Tile`: uma dica (`title`) por item.

#![forbid(unsafe_code)]

mod app;
pub mod atalho;
mod casca;
mod documento;
mod inicio;
pub mod linha_comando;
mod pedir_caminho;
mod recursos;
pub mod registro;
#[cfg(not(feature = "telas"))]
mod substitutos;
mod tabela;
mod tela_arquivo;
mod telas;
mod visao_geral;

pub use app::{Editor, Partida, aparencia_da_interface, aplicar_aparencia, tema_da_interface, tema_das_prefs};
pub use casca::janela::{RegioesJanela, regioes_da_janela};
pub use casca::{menu_aberto, titulo_da_janela};
pub use recursos::PedidoDialogo;

/// A janela fechou: corta a Consulta SQL que ainda roda no banco aberto (ou espera a vez nele), para as tarefas de
/// fundo terminarem logo. Sem isso, o `estado::esperar_tarefas` do `main` esperava o prazo inteiro por um comando
/// longo e o processo saía matando-o no meio. Devolve true quando havia uma consulta a cortar.
pub fn interromper_ao_sair(servico: &servico::Servico) -> bool {
    servico.interromper_sql_do_aberto()
}
