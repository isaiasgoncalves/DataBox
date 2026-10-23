//! **O design system do DataBox no visual Estúdio**, em egui 0.36: as peças das pranchetas aprovadas
//! (`design/canvas/*.dc.html`). É a base de todas as telas:
//! as telas **nunca** usam cor solta nem desenham peça própria quando há uma aqui.
//!
//! # Começo rápido
//!
//! ```ignore
//! // 1. Antes do primeiro quadro (no criador do app do eframe) — instala fontes, estilos e texturas:
//! interface::tema::aplicar(&cc.egui_ctx, Tema::Sistema);           // e de novo quando a pessoa trocar o tema
//! // 2. A cada quadro: o fundo da página, as peças, e no fim o toast.
//! interface::pintura::pintar_fundo(ui.painter(), interface::tema::paleta(ui), ui.ctx().content_rect());
//! Cartao::new("tabelas").titulo("Tabelas").hint("7 tabelas · clique no nome para abrir")
//!     .acoes(|ui| { ui.add(Botao::new("+ Nova tabela")); })
//!     .mostrar(ui, |c| { c.corpo_flush(|ui| { /* … */ }); });
//! interface::pecas::toast::desenhar_toast(ui.ctx(), f32::INFINITY);
//! ```
//!
//! **Fontes:** o egui só troca as fontes na passada seguinte e cai (pânico) ao desenhar com uma família
//! nomeada que ainda não existe. Chame [`tema::aplicar`] antes do primeiro quadro: na primeira chamada ele lê do
//! disco as fontes do sistema (a Segoe UI nos quatro pesos e na itálica, a Consolas ou, sem ela, a Cascadia Mono,
//! a Georgia do "i" e a Segoe UI Symbol; ~5 MB; a família do código escolhida nas Preferências entra depois, por
//! [`fontes::trocar_mono`]) com [`fontes::instalar`], a única leitura de disco na thread da interface. Se não der
//! (testes com `egui_kittest`), não desenhe enquanto [`fontes::instaladas`] for `false` (veja
//! `tests/mostruario.rs`); [`fontes::do_sistema_carregadas`] diz se elas já foram pedidas ao contexto.
//!
//! **Medidas:** 1px do CSS = 1 ponto do egui. Botões têm 28px (30px nas janelas) e campos 28px (busca 30px);
//! um raio único de 10 ([`pintura::RAIO_PADRAO`]) nos cartões, painéis, janelas, abas e campos, e pílula nos
//! botões, menus do topo, etiquetas, seletores e barra de progresso; janela 500px (larga 760px). Várias peças
//! medem a si mesmas no quadro anterior (ações à direita, colunas de mesma altura, rodapé) e pedem
//! `request_discard` quando a medida muda: o egui refaz a passada sem mostrar a errada. Parada, a tela não pede
//! repintura (só o ponto pulsante, a barra indeterminada e as transições de 200ms repintam).
//!
//! # Módulos e API pública
//!
//! - [`tema`]: [`tema::Paleta`] (os tokens das pranchetas nos dois temas, com os `color-mix` resolvidos: os campos
//!   básicos (`--k-*`) e os tokens novos em
//!   [`tema::Paleta::estudio`], um [`tema::Estudio`]),
//!   [`tema::CLARA`], [`tema::ESCURA`], [`tema::SombraCss`], [`tema::Tema`] `{Sistema, Claro, Escuro}` (`como_str`/`de_str` = `"system"|"light"|"dark"`, `rotulo`),
//!   [`tema::aplicar`]`(ctx, tema)`, [`tema::tema_aplicado`]`(ctx) -> Tema` ([`tema::Tema::Sistema`] quando nada
//!   foi aplicado), [`tema::paleta`]`(ui)` / [`tema::paleta_ctx`]`(ctx)`, [`tema::estilos`] (nomes dos
//!   `TextStyle`: h2, h3, corpo, hint, mono, eyebrow, botao, campo, numero) e [`tema::estilo`]`(nome)`,
//!   [`tema::estilo_completo`], [`tema::visuais`] (com a espessura do texto calibrada contra o Edge:
//!   [`tema::GAMA_TEXTO_CLARO`] / [`tema::GAMA_TEXTO_ESCURO`]), [`tema::definir_movimento_reduzido`] (o
//!   `prefers-reduced-motion`; o editor repassa o `SPI_GETCLIENTAREAANIMATION` do Windows) /
//!   [`tema::movimento_reduzido`]`(ui)` / [`tema::movimento_reduzido_ctx`]`(ctx)` (também valem com
//!   `animation_time` zero), [`tema::duracao_transicao`], [`tema::rgba`] / [`tema::rgb`].
//! - [`fontes`]: as fontes do sistema, lidas em tempo de execução e nunca embutidas: a Segoe UI nos pesos das
//!   pranchetas ([`fontes::Peso`]: `Leve` 350, `Normal` 400, `Medio` 600, `Pesado` 700, `Italico`; fora do Windows,
//!   Noto Sans ou DejaVu Sans), [`fontes::familia`], [`fontes::fonte`]`(tamanho, peso)`, [`fontes::fonte_mono`]
//!   (Consolas → Cascadia Mono → Hack, só nos editores de SQL e JSON e no SQL das janelas), [`fontes::fonte_ajuda`]
//!   (Georgia itálico negrito), [`fontes::definicoes`] (lê do disco), [`fontes::instalar`]`(ctx)` (chamado por
//!   [`tema::aplicar`]), [`fontes::do_sistema_carregadas`]`(ctx)`, [`fontes::instaladas`], [`fontes::fonte_valida`],
//!   [`fontes::metricas_verticais`]; a família do código: [`fontes::FAMILIA_MONO_PADRAO`] ("Consolas"),
//!   [`fontes::familias_mono_disponiveis`] / [`fontes::familias_mono_em`] (as candidatas Consolas, Cascadia Mono,
//!   Courier New e Lucida Console que existem; só `stat`) → [`fontes::FamiliaMono`] `{ nome, arquivo }`,
//!   [`fontes::ler_familia_mono`] (lê e confere; fora da thread da interface), [`fontes::trocar_mono`]`(ctx, bytes,
//!   nome) -> bool`, [`fontes::restaurar_mono`] e [`fontes::mono_escolhida`].
//! - [`codigo`]: o realce de SQL e de JSON e a aparência do código — [`codigo::PALAVRAS_RESERVADAS`] (as 147 do
//!   SQLite, movidas da Consulta SQL) e [`codigo::palavra_reservada`], o léxico [`codigo::trechos_sql`] →
//!   [`codigo::TrechoSql`] `{ bytes, classe: ClasseSql }`, [`codigo::realcar_sql`]`(texto, cores, fonte) ->
//!   LayoutJob`; o do JSON, [`codigo::trechos_json`] → [`codigo::TrechoJson`] `{ bytes, classe: ClasseJson }`
//!   (`Comum`, `Chave` = cadeia seguida de `:` na mesma linha, `Texto`, `Numero`, `Literal` = true/false/null,
//!   `Pontuacao`; vale para o JSON inválido e uma cadeia nunca atravessa a linha, então lexar linha a linha dá o
//!   mesmo que o texto todo) e [`codigo::realcar_json`]; [`codigo::Linguagem`] `{ Sql, Json, Texto }` com
//!   [`codigo::realcar`]`(texto, linguagem, cores, fonte)` (o cache do realce entra a linguagem na chave),
//!   [`codigo::CoresCodigo`] `{DataBox, AltoContraste, SolarizedEscuro, Monokai}` (`como_str`/`de_str`
//!   como no arquivo de preferências, `rotulo`, `nota`, `resolver(paleta)`) → [`codigo::CoresResolvidas`] `{ fundo,
//!   texto, kw, st, nu, pu, chave }` (os valores da prancheta 13; o DataBox segue o tema; a `chave` do JSON é a
//!   `--chave` do tema, Alto contraste `#E0B0FF`, Solarized `#8489D6`, Monokai `#A6E22E`, todas com 4,5:1 sobre o
//!   fundo da carta), [`codigo::AparenciaCodigo`]
//!   `{ cores, familia, tamanho }` com [`codigo::definir`] / [`codigo::atual`] / [`codigo::cores`] /
//!   [`codigo::fonte`] / [`codigo::altura_linha`] (no contexto; o tamanho também nos estilos `Monospace` e `mono`).
//! - [`icones`]: os ícones do app por nome — [`icones::Icone`] (as silhuetas, os sólidos do trilho, do menu Arquivo,
//!   do banco, dos objetos e das partes da tabela, `Tipo(`[`icones::TipoColuna`]`)` (com a `Lista`, a pastilha `[]` do
//!   JSON, que usa o PNG do `{}`), `Folha(`[`icones::TipoArquivo`]`)`
//!   e `Estado(`[`icones::EstadoIcone`]`)`, e os traços; `TODOS`, `nome()` = o rótulo padrão, `familia()` →
//!   [`icones::Familia`] `{Silhueta, Solido, Traco}`, `pngs()`), [`icones::pintar`]`(painter, rect, Icone, cor)` (o
//!   PNG embutido pelos pixels da tela, preso aos pixels, com o texto da pastilha e da folha por cima; a cor vale
//!   para as silhuetas e os traços), [`icones::pintar_apagado`] (o `grayscale` do item desligado),
//!   [`icones::pintar_arco`] (o arco do ocupado), [`icones::TipoColuna::do_declarado`]`(tipo, pk, fk)` e as funções
//!   puras [`icones::glifo_do_tipo`], [`icones::retangulo_do_rotulo`], [`icones::escolher_png`],
//!   [`icones::lado_do_png`] e [`icones::textura_carregada`] (os testes).
//! - [`marca`]: o logo do DataBox (a caixa de papelão com o banco e as ferramentas) em PNG, sempre o completo (de 16 a
//!   256px, sem versão simplificada), no tamanho certo para a escala da tela:
//!   [`marca::logo`]`(ui, lado) -> Response`, [`marca::pintar_logo`]`(ui: &Ui, rect)` (sem alocar),
//!   [`marca::icone_janela`] (para o `ViewportBuilder::with_icon`) e [`marca::decodificar_png`].
//! - [`pintura`]: [`pintura::preencher`] / [`pintura::forma_preenchida`] com [`pintura::Preenchimento`] (`Aqua` = o
//!   volume neutro `--bt1` → `--bt2`, `Primario` = `--volpri`, `Risco` = `--volrisco`, `Superficie` = `--c`, `Aba`,
//!   `Cabecalho`, `Liso`), [`pintura::riscas`] / [`pintura::riscas_com_canto`] (a textura de riscas a 135°, presa à
//!   janela), [`pintura::sombra_interna_baixo`] / [`pintura::sombra_interna_topo`] (o volume e o baixo-relevo),
//!   [`pintura::sombra`] / [`pintura::forma_sombra`] (o `box-shadow` externo: gaussiana do CSS pintada **só fora**
//!   da peça, como no navegador; as malhas ficam guardadas sem a cor, que pode animar), [`pintura::halo`] (o brilho
//!   do hover), [`pintura::anel_foco`], [`pintura::halo_foco`], [`pintura::foco_visivel`] (o `:focus-visible`: anel
//!   só com teclado), [`pintura::raio`] / [`pintura::canto`], [`pintura::clarear`], [`pintura::misturar`],
//!   [`pintura::RAIO_PADRAO`] (o raio único de 10 das superfícies e dos campos),
//!   [`pintura::pintar_fundo`] (o fundo liso da janela) e as peças de antes com as cores do Estúdio
//!   ([`pintura::anel_aqua`], [`pintura::anel_rebaixado`], [`pintura::brilho_topo`]).
//! - [`formato`]: [`formato::formatar_numero`] ("5.000", "1,5"), [`formato::formatar_numero_com_casas`]`(n, min, max)`
//!   ("98.231,40"), [`formato::formatar_bytes`]
//!   / [`formato::partes_bytes`] ("1,2 MB"), [`formato::formatar_bytes_em`], [`formato::formatar_data`]
//!   ("22/09/2026 10:31"), [`formato::formatar_ms`] ("320 ms", o `tempo()` da Consulta SQL),
//!   [`formato::formatar_segundos`] ("4,6 s", o `segundos()` da aba Dados), [`formato::plural`]
//!   ("60 linhas"), [`formato::contagem_linhas`] ("Linhas 1–41 de 41", o rodapé das grades),
//!   [`formato::contagem_filtrada`] ("41 de 60 linhas"), [`formato::filtradas_de`] ("filtradas de 60", no rodapé), [`formato::nome_do_caminho`], [`formato::real_js`] (o `String(número)` do JavaScript:
//!   `17051.0` → "17051", `1e-7` → "1e-7"); variantes `_opt` devolvem "—" para `None`; [`formato::sem_acento`] (a
//!   chave das buscas e dos filtros: sem acento e em minúsculas) e [`formato::letra_sem_acento`] (uma letra, sem
//!   mudar a caixa das outras).
//! - [`pecas`] — cada uma com o papel da classe do kit:
//!   - [`pecas::botao::Botao`] (`.btn` em pílula de 28px: `new` = neutro, `primario` = `.btn.pri`, `ouro` = o
//!     mesmo primário, `ghost` = `.btn.sutil`, `perigo` = `.btn.risco`, `ghost_perigo` = `.btn.sutil.perigo`,
//!     `redondo` = `.ib` de 30px ("‹", "›", "✕", "+" viram os ícones), `fechar` = o `.ib` com o ×, `icone(Icone)` =
//!     o `.ib` com um ícone da casca; `.pequeno()` = `.ib.pq` de 26px / `.btn.pq` de 24px, `.do_cabecalho()` = o
//!     `.cab .ib` de 20px com a silhueta de 12, `.rail()` = o `.rail` de
//!     36px da barra de ícones (ligado, com a barrinha na cor de destaque), `.principal()` = `.ib.pri`;
//!     `.com_icone(Icone)` = a silhueta de 14 antes do texto, `.lista()` = o `.btn.sutil.lista` (largura toda, 400,
//!     à esquerda); `.ocupado(true)` com o ponto do `.carregando` (no de ícone, o arco girando no lugar do
//!     ícone); `.habilitado(false)`; `.selecionado` = `.ib.on`, e o leitor de
//!     tela ouve o estado; `.dica`; `.rotulo`), [`pecas::botao::com_altura`]`(ui, ALTURA_JANELA, |ui| …)` (os 30px do
//!     `.dlg footer .btn`), e o
//!     volume para outras peças: [`pecas::botao::Volume`] (`neutro`, `primario`, `risco`, `sem_sombra_baixo`,
//!     `halo`), [`pecas::botao::pintar_volume`] com [`pecas::botao::EstadoVolume`], [`pecas::botao::realce_curvo`]
//!     / [`pecas::botao::forma_realce_curvo`] (o `inset 0 1px 0` que acompanha a curva) e
//!     [`pecas::botao::pintor_de_fora`] (o recorte dos halos).
//!   - [`pecas::cartao::Cartao`] (o "documento" `.doc`: fundo `--c`, borda `--vidrob`, raio 10; o título na faixa
//!     `.sec` de 32px — 42px com ações — e o fio `--vidrob`; `titulo`, `hint`, `extra`, `acoes`, `esticar`,
//!     `mostrar(ui, |c| …)` com [`pecas::cartao::CorpoCartao`]: `corpo`, `corpo_flush`, `corpo_avisos`,
//!     `corpo_com_margem`, `livre`, `rodape`, `rodape_dividido` — o `.rodape` de 34px no fundo `--p`, que no fim do
//!     cartão leva os cantos de baixo dele), [`pecas::cartao::espaco_entre_cards`].
//!   - [`pecas::rotulos`]: `pill(ui, texto, TipoPill::{Neutra, Ok, No, Wait, Acc, Mute})` (`.pill`: 10px / 700
//!     em caixa alta, 16px), `pill_clicavel`, `pintar_pill`, `pintar_pill_contorno` (só o contorno, sobre a
//!     seleção), `tag`, `tag_clicavel(ui, texto, TipoTag::{Ouro, Tracejada})` (`Ouro` = `--acct` sobre `--tint`),
//!     `chip(ui, texto, mute)` (`.chip` de 24px), `pintar_chip_em`,
//!     [`pecas::rotulos::chip_filtro`]`(ui, &[(texto, forte)], rótulo do ×) -> RespostaChip { resposta, fechar }`
//!     (a etiqueta de filtro com o × de 18px), `etiqueta_mais(ui, n, dica)` (o "+N" com a lista na dica).
//!   - [`pecas::avisos`]: `aviso(ui, &[Trecho], perigo) -> RespostaTexto` (o `.aviso` âmbar ou o `.erro`
//!     vermelho: faixa de raio 10 com o sólido de estado de 18px, fundo e fio na cor do estado, texto 12px `--t2`
//!     com o `strong` em `--t` ou `--perigo`; papel `status`; texto selecionável e copiável; `Trecho::Link`
//!     clicável, `Trecho::Ajuda` com o "i" mini), [`pecas::avisos::aviso_tom`]`(ui, &[Trecho],`
//!     [`pecas::avisos::TomAviso`]`::{Atencao, Erro, Concluido, Informacao})` (o âmbar só para o alerta de
//!     verdade; sucesso e resultado no concluído, `--ok`; prévia e informação neutra na informação, `--tint`),
//!     `aviso_texto -> Response`, `Aviso::new(perigo)` / `Aviso::com_tom(tom)` `.mostrar(ui, |ui, cores| …)`; a
//!     caixa tem a largura do texto, até 560px, e fica centrada ([`pecas::avisos::caixa_do_aviso`]), e só a faixa
//!     do topo do documento vai de ponta a ponta ([`pecas::avisos::aviso_faixa`], `Aviso::ponta_a_ponta`),
//!     `vazio(ui, texto, compacto)` (em `--t3`), `carregando(ui, "contando")` (o ponto pulsa em `--acc`),
//!     `pintar_ponto_pulsante`. O aviso que abre o corpo de um cartão ou de uma janela ganha
//!     [`pecas::avisos::ESPACO_APOS_AVISO`] (14px) embaixo: o `CorpoCartao` e a `Janela` marcam o corpo
//!     ([`pecas::avisos::marcar_corpo`], para quem monta um corpo próprio).
//!   - [`pecas::janela`]: [`pecas::janela::Janela`] (o `.dlg`: 500px, raio 10, no vidro `.flut`, cabeçalho de 50px
//!     e rodapé de 58px com os botões de 30px, o véu `--veu` atrás; modal com ✕/Esc/fundo em `pediu_fechar` — a
//!     tela decide; clique no fundo nos 400ms iniciais não conta; foco inicial no primeiro campo, senão no
//!     primário do rodapé; `rodape_esquerda`/`rodape_direita`; `larga`; `largura`; `icone` (o sólido de 20 antes
//!     do título); `sub` (o `--t3` depois dele); [`pecas::janela::RespostaJanela`]
//!     `{inner, pediu_fechar, enviou, abriu_agora, rect}` — `enviou` é o Enter num campo de uma linha do corpo, o
//!     submit implícito do `<form>`), [`pecas::janela::confirmar`]`(ctx, &mut Option<Confirmacao>) ->
//!     Option<bool>` e [`pecas::janela::Confirmacao`].
//!   - [`pecas::toast`]: `toast_concluido(ctx, msg)` (o ✓: "Linha salva.", "Copiado."), `toast_erro` (o ×: o
//!     erro de uma tarefa), `toast_atencao` (o que não foi feito) e `toast` (a informação neutra, com o "i"),
//!     `desenhar_toast(ctx, limite)` (uma vez por quadro; 22px acima da borda de baixo e nunca abaixo de `limite`,
//!     `f32::INFINITY` sem nada embaixo; o aviso flutuante no vidro `.flut` em pílula, 37px (`padding:9px 16px`),
//!     com o sólido de estado e o texto 12px `--t`; respeita `tema::movimento_reduzido_ctx`), `toast_atual`,
//!     `icone_toast`, `rect_toast` (onde ele ficou, para os testes), `opacidade_toast`, e
//!     `toast_concluido_com_acao(ctx, msg, dica, AcaoToast)` (com um botão pequeno na pílula, como o "Mostrar na
//!     pasta" da cópia de segurança) com `acao_do_toast(ctx) -> Option<carga>`, que a casca lê a cada quadro.
//!   - [`pecas::ajuda`]: `ajuda(ui, texto)` (o sólido de informação de 16px, azul com o "i" branco), `ajuda_mini`
//!     (14px), `Ajuda::new(&[Trecho])` (o balão no vidro `.flut`, da largura do texto até 440px / 320px), e os
//!     sólidos de estado para outras peças: [`pecas::ajuda::pintar_icone_estado`]`(painter, rect,
//!     IconeEstado)` (o [`icones::EstadoIcone`] em PNG: atenção, erro, concluído, informação, ajuda, bloqueado).
//!   - [`pecas::campos`]: `campo(ui, rotulo, ajuda, |ui| controle)` e `rotulo_campo` (rótulo `.tit`: 11px / 600
//!     em caixa alta, `--t3`), `campos_duas_colunas` (`par`, `um`, `largo`), [`pecas::campos::Entrada`] (`.campo`
//!     de 28px com os cantos de raio 10, [`pecas::campos::RAIO_CAMPO`], em baixo-relevo, com hover e foco de anel
//!     e halo azuis; `dica`, `monoespacada` — o
//!     `input.mono` da janela de esquema, que no Estúdio é a fonte da interface em peso 350 —, `busca` = `.busca`
//!     de 30px com a lupa, `atalho` = o `.kbd` no fim (o Ctrl+F da busca), `largura`, `senha`, `alinhar_direita`,
//!     `rotulo`, `habilitado`, `cor`, `pastilha(TipoColuna)` = a pastilha de 16 antes do texto, `somente_leitura` =
//!     o `.campo.so-leitura` (seleciona e copia, não muda, sem esmaecer), `compacta` = o campo da
//!     edição no lugar, 26px, raio 4, transparente até o hover/foco; `mostrar` → `TextEditOutput`),
//!     [`pecas::campos::EntradaSugestoes`] (input com `datalist`: lista filtrada embaixo, ↑/↓, Enter/clique
//!     escolhem sem enviar a janela, Esc fecha a lista), [`pecas::campos::AreaTexto`] (o baixo-relevo com raio 10;
//!     `mono`, `tamanho_fonte`, `altura`, `habilitado`, `codigo` = editor de SQL sem quebra, com rolagem nos dois
//!     eixos, as cores e a fonte do código e o Tab do `recuar()` da Consulta SQL — dois espaços ou o recuo das
//!     linhas, Shift+Tab tira, depois de um Esc o Tab sai do campo; [`pecas::campos::recuar`] é a regra pura; no
//!     modo código, `numeros_de_linha` (a calha de 46px), `marcar_linhas(linhas, cor)`, `sublinhar(caracteres)`,
//!     `cores`, `linguagem(`[`codigo::Linguagem`]`)` (SQL por padrão, JSON ou texto sem cores) e
//!     `erro_na_linha(linha)` (o número da calha em `--perigo`)), [`pecas::campos::Select`] (o
//!     `button.campo` com o mesmo raio 10 e a seta em `--t3`; setas trocam; `largura_auto`),
//!     [`pecas::campos::lista_opcoes`]
//!     (a lista no vidro dos menus, `.flut` com itens `.mi` de 28px e a opção escolhida destacada, para um `Popup`
//!     próprio), [`pecas::campos::pintar_vidro_flutuante`] (o `.flut` fosco de toda camada flutuante: sombra
//!     `--sh`, a foto desfocada do [`fosco`] embaixo do `--flut` a 78/80%, reflexo, borda e realce;
//!     [`pecas::campos::forma_vidro_fosco`] devolve a forma, para o vidro que só tem tamanho depois do conteúdo ir
//!     num lugar reservado antes dele; [`pecas::campos::forma_vidro_flutuante`] é o liso a 96%, sem a foto: só o das
//!     dicas e do balão de ajuda) e [`pecas::campos::pintar_item_menu`] (o
//!     `.mi:hover`), [`pecas::campos::pintar_salvo`]
//!     (o verde que a célula pisca por 1,4s depois de gravar), [`pecas::campos::enter_no_campo`], `check`,
//!     [`pecas::campos::Check`] (`indeterminado`) e `radio` (com o volume dos botões: a caixa quadrada de 16px com
//!     cantos de 5px, [`pecas::campos::RAIO_CAIXA`], e a opção no círculo de 16px; marcados no degradê do destaque
//!     com o ✓ ou o ponto brancos), `pedir_foco_inicial`.
//!   - [`pecas::navegacao`]: `trilho` / `segmentado(ui, id, itens, &mut valor, TamanhoTrilho::{Normal, Cheio,
//!     Compacto, CompactoCheio}) -> bool` (o `.seg`: trilho rebaixado em pílula com as partes todas da largura da
//!     maior — nos cheios, a linha inteira em partes iguais —, botões de 26px — 24px nos compactos — e o escolhido
//!     em volume com o texto em 600), `abas(ui, id, &[(chave, rótulo)], ativa) -> Option<chave>` (as abas de
//!     documento `.aba`: 34px numa faixa de [`pecas::navegacao::ALTURA_ABAS`], cantos de cima com raio 10, a ativa
//!     na superfície com textura), `trilha(ui, &[Passo]) -> Option<usize>` (o `.migalha`: `--t2` com o chevron em
//!     `--t3`), [`pecas::navegacao::Cabecalho`] (título 14px / 600 com o "i", `trilha`, `ajuda`, `sub` em `--t3`
//!     selecionável e copiável, `apos`, `acoes` à direita que descem juntas).
//!   - [`pecas::tiles::tiles`]`(ui, &[Tile])` (os números da Visão geral: auto-fit de 150px; caixas de borda
//!     `--b2` e raio 10 com o número 24/350, o sufixo em `--t3` e o rótulo 10.5/600 em caixa alta).
//!   - [`pecas::dock::dock`]`(ui, &[ItemDock]) -> RespostaDock` (os contadores da Visão geral na dock: os ícones
//!     `-grande` de 48pt em pé numa prateleira, com reflexo e sombra de contato, o número 24/300 e o `.tit`
//!     embaixo; seis colunas ou 3 + 3 pelo conteúdo; parada: sem a animação de hover, pedido de 17/10/2026).
//!   - [`pecas::popover::Popover`]`::new(id, ancora).largura(…).mostrar(ctx, |ui| …)` (o vidro fosco preso embaixo de
//!     uma âncora, por cima do documento; fecha com o Esc ou o clique fora: a condição de filtro dos Dados).
//!   - [`pecas::filtros`]: os filtros das grades, iguais nos Dados e no CSV (revisão de UI de 19/10/2026) —
//!     [`pecas::filtros::filtros`]`(ui, &[EtiquetaFiltro]) -> RespostaFiltros { abrir, tirar, limpar,
//!     nova_condicao }` ("Filtros", as etiquetas, "+ Condição" e "Limpar filtros"; a contagem fica no rodapé),
//!     [`pecas::filtros::filtros_com_direita`]`(…, |ui| …)` (o mesmo, com mais alguma coisa na ponta direita: a contagem
//!     da busca do CSV), [`pecas::filtros::barra_filtros`] (a `.barra-filtros` de 38px do CSV) e
//!     [`pecas::filtros::popover_condicao`]`(ctx, id, &mut CondicaoEmEdicao, colunas, operadores, sem_valor) ->
//!     Option<FimCondicao>` (Coluna, Operador, Valor, Cancelar e Aplicar).
//!   - [`pecas::divisoria::Divisoria`] (a alça dos painéis laterais da casca; ver "Mudanças de API").
//!   - [`pecas::detalhes::detalhes`]`(ui, |d| d.linha(…) / d.linha_texto(…) / d.titulo_grupo(…))` (o `.det`:
//!     rótulo `--t3` em 112px · valor `--t`, `gap:7px 10px`; o texto do `linha_texto` é selecionável;
//!     `titulo_grupo` é o `.sec` entre dois `.det`, o "Na grade de dados" da janela de atalhos).
//!   - [`pecas::tabela`]: [`pecas::tabela::Tabela`]`::new(id, &[ColunaTabela], n_linhas).mostrar(ui, |ui, linha,
//!     coluna| …)` — a tabela simples `.lista-t` (`th` de 30px em 11.5/600 `--t2` sobre o degradê `Cabecalho` com
//!     as riscas e o fio `--btb`; linhas de 32px — `altura_linha` para os 34/36px do Início e da Estrutura — com
//!     `padding:0 12px`, fio `--b` entre elas e hover `--selh`; `inativas` = `tr.inativo` (texto em `--t3`),
//!     `marcadas` = `tr.sel` (`--sel`); conteúdo mais alto estica a linha); [`pecas::tabela::ColunaTabela`]
//!     (`direita` = `td.num`/`td.acoes`, `estica` = a coluna que fica com a sobra e encurta com "…", `fixa`);
//!     [`pecas::tabela::moldura_lista`]`(ui, |ui| …)` (a moldura `--b2` de raio 10 das listas no corpo do
//!     documento, com os cantos recortados: a de Tabelas da Visão geral).
//!   - [`pecas::modelo::Modelo`] (`details.modelo`: a seta da árvore em `--t3`, fio entre os itens, `primeiro`,
//!     `aberto_inicial`; o estado fica no contexto).
//!   - [`pecas::progresso::barra_progresso`]`(ui, Some(fração) | None)` (a `.barra` de 8px: trilho rebaixado e o
//!     preenchimento com o volume do botão principal; indeterminada anima; parada com movimento reduzido).
//!   - [`pecas::texto_pre::TextoPre`] (caixa `--p` com borda `--b2` e raio 10; `sql()` é o `.cod`: a fonte do
//!     código, Consolas 12.5px em linhas de 20px por padrão, com as cores do código — fora do DataBox, a caixa no
//!     fundo delas; `altura_max`, `apagado`, `cores`; selecionável e copiável).
//!   - [`pecas::zona_risco::ZonaRisco`] (recolhida por padrão, no vermelho `--perigo`; `hint`, `no_card_tabela`).
//!   - [`pecas::texto`]: `h2` (20/600), `h3` (14/600), `h4` / `eyebrow` / `rotulo_caps` (o `.tit`: 11/600 em caixa
//!     alta, `--t3`), `hint`, `texto`, `galley_caminho` / `encurtar_caminho` / `encurtar_nome` (cortado do
//!     começo, com o nome e a pasta-mãe inteiros; o nome que não cabe, no meio, com a extensão), `texto_cortado` (uma linha com "…" e a dica do texto inteiro: o
//!     `td.caminho-curto`), `paragrafo`, `forte`, `link` e `nome_abre` (o `.link`: `--acct` 600), `codigo` (a fonte
//!     da interface em 350 sobre `--h`), `texto_rico(ui, &[Trecho::{Normal, Forte, Codigo, Fraco, Quebra, Link,
//!     Ajuda}], …) -> RespostaTexto` (`{resposta, link: Option<índice do trecho>}`; selecionável),
//!     `mostrar_paragrafo` / `mostrar_paragrafo_selecionavel`, e os construtores de `Galley` (`galley`,
//!     `galley_cortado`, `galley_quebrado` → [`pecas::texto::Paragrafo`] `{galley, deslocamento, faixas}`,
//!     `galley_caps` (700, o `.pill`), `galley_tit` (o `.tit`), `paragrafo_rico` / `paragrafo_rico_com` com
//!     [`pecas::texto::CoresTexto`], `interagir_paragrafo` para um parágrafo de trechos montado à mão,
//!     `retangulos_da_faixa`) e de posição (`y_na_linha`, `linha_de_base`, `linha_de_base_css`,
//!     `y_para_linha_de_base`).
//!   - [`pecas::casca`]: [`pecas::casca::Menu`] (o botão `.menu` de 24px, transparente até o hover e aberto com o
//!     volume neutro, e o painel no vidro `.flut` de 286px — `largura` para outro —: [`pecas::casca::PainelMenu`]
//!     com `cabecalho`, `item` (`.mi` de 28px) com atalho, `item_desligado` (`.mi.off`), `com_marcas` (a coluna
//!     `.ck`), `item_marcado` (o ✓), `item_opcao` (o ponto), `item_duplo` (o recente de 40px com o caminho),
//!     `submenu(texto, largura, |m| …)` (abre com o ponteiro em cima, 2px à direita, no mesmo vidro),
//!     `separador` (`.msep`), `titulo` (`.msep` + `.tit`), `livre`, `fechar`; pelo teclado, ↓ e ↑ do botão vão ao
//!     primeiro e ao último item ligado, nos itens andam pelos ligados com volta sem sair do painel, e o Esc fecha e
//!     devolve o foco ao botão). O [`pecas::casca::PainelMenu`] ganhou `com_silhuetas`, `silhueta(Icone)` e
//!     `item_perigo` (o `.mi.perigo`) para o menu de contexto.
//!   - [`pecas::menu_contexto::MenuContexto`] (`new(id).rotulo(nome).mostrar(&dono, |m| …)`: o `.menu-ctx` da
//!     prancheta 15, 250px no vidro fosco, itens de 28px com a silhueta de 14px; abre no clique direito do dono, no
//!     ponteiro, e no Shift+F10 com o foco nele (só com o Shift: o Alt+Shift+F10 e o Ctrl+Shift+F10 não), embaixo, com o foco no primeiro item; ↑ e ↓ andam pelos ligados, Enter
//!     ativa, Esc fecha e devolve o foco ao dono; `aberto` e `fechar` por `id_salt`). `mostrar_aberto(ctx, camada,
//!     id_dono, Option<Abertura { pos, pedir_foco }>, |m| …)` abre sem a resposta de um widget (o menu da célula).
//!   - o menu da célula (0.2.3): [`grade::menu_da_celula`]`(ui, &mut nucleo, &`[`grade::OfertasCelula`]`) ->
//!     Option<`[`grade::AcaoCelula`]`>`, chamado todo quadro depois do `NucleoGrade::mostrar`: abre no clique direito
//!     numa célula (dentro do intervalo, age nele; fora, a célula vira a ativa) e no Shift+F10 com o foco na grade
//!     (embaixo da célula ativa, com o foco no primeiro item); o Esc fecha e devolve o foco à grade; enquanto ele está
//!     aberto, as teclas não andam pela grade. Itens na ordem da A.1 do `design/planos-0.2.3.md` (Copiar · Copiar com
//!     cabeçalho · Copiar como › CSV, JSON, Markdown, INSERT, Lista para IN (…), pandas — Colar · Limpar · Definir
//!     como NULL — Filtrar por este valor · Excluir este valor do filtro — Marcar as linhas · Abrir a ficha da linha ·
//!     Excluir as linhas), o fio entre os grupos com algum item; `OfertasCelula::de_leitura(intervalo)` liga os de
//!     copiar (a lista para `IN` só com uma coluna) e `ligar`, `desligar(acao, motivo)` (o item desligado com o motivo
//!     na dica e no `description`), `ligar_se` e `com` o resto; o que não tem oferta não aparece. O atalho vai no texto
//!     e no `keyboard_shortcut` do AccessKit. A tecla Menu não chega ao egui 0.36. Para copiar:
//!     [`grade::tsv_do_intervalo_com_cabecalho`], [`grade::texto_do_intervalo_como`] (`FormatoCopia`, os nomes, cada
//!     célula como `tabular::colunas::ValorCopia`, `OpcoesCopia { separador, tabela }`) e [`grade::valor_de_copia`]
//!     (`Celula` → NULL, número, texto). A [`grade::GradeSomenteLeitura`] copia o Ctrl+Shift+C sozinha e tem
//!     `texto_copiado_com_cabecalho`, `texto_copiado_como(formato, &opcoes)` e `nucleo_mut` (para o menu). Para colar
//!     (nos Dados e no CSV): [`grade::colagem`]`(texto, intervalo, linhas, colunas)` (onde cada valor do TSV cai, a
//!     partir da célula de cima à esquerda; um valor só preenche o intervalo; o que passa da última linha ou coluna é
//!     contado em `de_fora`), [`grade::texto_colado`]`(ctx, &nucleo)` (o `Event::Paste` com o foco no corpo) e
//!     [`grade::frase_de_fora`]. O `grade` reexporta `FormatoCopia`, `OpcoesCopia` e `ValorCopia` do `tabular`.
//!   - o rodapé da seleção (0.2.3): [`grade::ResumoSelecao`] (`contagem`, `numeros`, `soma`, `minimo`, `maximo`,
//!     `casas`; `somar_celula(&Celula, numerica)`, `somar_do_csv(texto, numerica, decimal)`, `somar_numero(valor,
//!     casas)`, `somar_texto`, `juntar`, `partes`, `texto` = "Contagem 33 · Soma 98.231,40 · Média 8.930,13 · Mín.
//!     154,03 · Máx. 17.810,34"), [`grade::rodape_da_selecao`]`(ui, &resumo)` (12px `--t2` na largura que sobra; o que
//!     não cabe sai, primeiro o Mín. e o Máx., depois a Média, e vai para a dica) e [`grade::rodape_somando`] (o
//!     "Somando…" de [`grade::SOMANDO`], com o ponto pulsante) e [`grade::largura_minima_da_selecao`] (a contagem e a
//!     soma, ou o "Somando…": o mínimo que a tela que divide o rodapé deixa para as contas).
//!   - [`pecas::acao::Acao`] (`new(titulo, Icone).atalho("Ctrl+O").primaria(true).habilitado(..)`, `ui.add`: o `.acao`
//!     do Início, 220×69 com o círculo de 36 e a silhueta de 20, o nome em 13/600 e o atalho no `.kbd`; o primário
//!     usa o degradê do botão principal, o halo de 30px no hover e o texto branco).
//!   - As peças da casca nova (a anatomia do `design/README.md`; quem monta a janela é o editor):
//!     - [`pecas::barra_titulo`]: a faixa de 30px integrada aos menus — `marca(ui)` (o logo de 16px em 36×30),
//!       `botao_janela(ui, BotaoJanela, janela_ativa)` / `botao_janela_em(ui, id, rect, …)` (46×30, glifos de 10px
//!       com traço de 1px, o Fechar vermelho `#c42b1c` no hover; rótulos "Minimizar", "Maximizar", "Restaurar" e
//!       "Fechar a janela") e `titulo_centrado(ui, faixa, livre, texto, janela_ativa)` (em `--t3`, no centro da
//!       janela, cortado e encostado quando não cabe; esmaece com a janela inativa).
//!     - [`pecas::ferramentas`]: `barra_ferramentas(ui, |ui| esquerda, |ui| direita)` (o painel fixo de 44px com
//!       2px entre os botões), `separador` (`.sep`) e `busca_desligada` (a [`pecas::campos::Entrada::busca`] de
//!       250px com o Ctrl+F, desligada; a ligada é a `estado::busca_da_barra`).
//!     - [`pecas::abas_documento`]: `FaixaAbas::new(id, &[AbaDocumento]).ativa(..).nova(..).mostrar(ui) ->
//!       RespostaAbas { clicada, fechar, nova, mover, rect, juncao }` (abas de 34px com ícone, nome, sub e ×, e a
//!       chave estável de `com_chave`; papel `tab`; clique do meio fecha; arrastar muda de lugar (`mover: (de, para)`),
//!       e o menu da aba move e fecha; encolhem com "…" e rolam; `alterada(true)` põe o
//!       ponto de 7px `--acct` ([`pecas::abas_documento::LADO_PONTO`], o `.aba .mod`) no lugar do ×, que volta com o
//!       ponteiro ou o foco na aba, e o rótulo ganha ", alterada") e
//!       `pintar_documento(painter, paleta, rect, juncao)` (o `.doc` sem o fio de cima embaixo da aba ativa).
//!     - [`pecas::avisos::aviso_com_acoes`]`(ui, trechos, tom, &[`[`pecas::avisos::AcaoAviso`]`]) ->
//!       RespostaAcoes { resposta, clicada, link }` (etapa 5, passo 5): o aviso compacto com o texto e, embaixo, até três
//!       botões `.btn.pq` (`AcaoAviso::principal`, `neutra`, `sutil`, com `dica`; `MAXIMO_ACOES` = 3) — "Colar 1.180 ·
//!       Acrescentar 24 linhas · Cancelar" da prancheta 16.
//!     - [`pecas::documento`] (etapa 3, passo 5): `Documento::abrir(ui, rect, juncao, com_rodape)` pinta a moldura e,
//!       na ordem em que são chamados, `aviso(ui, tom, |ui, cores| …)` / `aviso_texto(ui, tom, trechos)` (o `.aviso`
//!       de ponta a ponta no alto, [`pecas::avisos::Aviso::faixa_do_documento`]), `faixa(ui, |ui| …)` (42px,
//!       [`pecas::documento::ALTURA_FAIXA`]), `pagina(ui, Pagina, |ui| …)` (a página que rola, `padding:18px 20px
//!       20px` debaixo da faixa e 20px sem ela) ou `propria(ui, |ui| …)` (o retângulo inteiro) e `rodape(ui, |ui| …)`
//!       (34px, [`pecas::documento::ALTURA_RODAPE`]); `Rolagem::{Pagina, Propria}` e `rect_conteudo()`.
//!     - [`pecas::arvore`]: o `.no` de 24px do Explorador, `No::new(texto).recuo(recuo(nível)).seta(Seta)` com
//!       `icone`, `contagem`, `ponto`, `forte`, `ativo`, `rotulo` e `dica`, e `.mostrar(ui) -> RespostaNo
//!       { resposta, alternou }` (papel `link` com `selected` e `expanded`; com o foco, → e ← abrem e fecham sem
//!       tirar o foco do nó; a seta não é parada do Tab); `RAIO_NO` (6) serve também aos itens das listas dos
//!       painéis.
//!     - [`pecas::painel`]: `pintar_painel_fixo` / `forma_painel_fixo` (o `.vidro` dos painéis fixos), `Painel`
//!       (o painel com o `.cab`, a faixa de 23px com a largura da linha da árvore e os quatro cantos de `RAIO_NO`,
//!       `RECUO_CAB` = borda + `MARGEM_NO`; o título de 10,5px em `--t2`, as ações `Botao::do_cabecalho` de 20px e o
//!       corpo) e `Secao` (o `.sec` de 32px que abre e fecha,
//!       com o rótulo "Seção <título>" para o leitor de tela).
//!     - [`pecas::barra_status`]: `barra_status(ui, &[ItemStatus], &[ItemStatus])` (26px; o ponto, a cor, o peso e
//!       a dica de cada item; sem espaço, o primeiro da direita encolhe com "…" antes de cobrir o primeiro da
//!       esquerda, e nada passa da barra).
//!   - [`pecas::arranjo`]: `linha_com_direita` (o `.spacer`), `colunas` / `colunas_iguais` (`.grid.g2`),
//!     `duas_colunas`, `fluxo` (flex-wrap), `linha` / `em_linha` (linha de altura fixa).
//! - [`grade`]: o desenho de célula e de `th` para as duas grades (`.grade` das pranchetas) —
//!   [`grade::pintar_celula`]`(painter, paleta, rect, &Celula, numérica)` (12px em `--t`; NULL e "vazio" em itálico
//!   `--t3`, chip de BLOB, "…" do texto cortado em `--acct`, números à direita, reais como o JavaScript escreve),
//!   [`grade::pintar_celula_chave`] (`td.chave`: `--acct` 600), [`grade::pintar_numero_linha`] (`td.lin`: `--t3`
//!   à direita), [`grade::pintar_cabecalho_coluna`] com [`grade::Th`] (nome em 11.5/600 `--t2`, `--t` com
//!   `realce`; a seta de ordem desenhada em `--acct`, a `.seta-ord` de [`grade::pintar_seta_ordem`]; o tipo à
//!   direita em 10px `--t3`, o `.tp`), [`grade::pintar_fundo_cabecalho`]
//!   (o degradê `Cabecalho` com as riscas e o fio `--btb`; serve também à tabela), [`grade::pintar_celula_ativa`]
//!   (contorno de 2px `--acc` e o brilho interno `--brilho`), [`grade::texto_exibicao`], [`grade::texto_celula`]
//!   (o `textoCelula`: o texto da edição no lugar e da cópia), [`grade::dica_celula`], [`grade::coluna_numerica`],
//!   [`grade::pintar_texto_de_celula`] (o texto de uma célula de arquivo de texto, o CSV: sem o "vazio" em itálico,
//!   o texto vazio não pinta nada), [`grade::pintar_celula_alterada`] (`td.alt`: fundo `--tint` e faixa de
//!   [`grade::LARGURA_FAIXA_ALTERADA`] 3px `--acct` à esquerda) e [`grade::Th::selecionada`] (`th.selcol`: o nome em
//!   `--t`),
//!   [`grade::larguras_iniciais`], as medidas (`ALTURA_LINHA` 28, [`grade::altura_cabecalho`] = 30 em todos os
//!   casos, [`grade::PADDING_X`] 10 e o fio da direita, [`grade::LARGURA_COLUNA_MARCAR`] 32,
//!   [`grade::padding_coluna`] = o `padding:0 12px` da tabela simples);
//!   - o [`grade::NucleoGrade`] — a base da grade da aba Dados: rolagem virtual nas duas direções, cabeçalho
//!     grudado, linhas de 28px com os fios `--b` embaixo e entre as colunas, hover `--selh`, larguras com alças
//!     (`com_fixas` para as colunas de marcar e de abrir), `com_coluna_marcar` (`td.selc`: sem padding e sem o fio
//!     à direita), `com_coluna_numero` (`td.lin`: o fundo `--p`), célula ativa (clique e teclado:
//!     [`grade::Navegacao`] `Consulta` ou `Dados`, `com_navegaveis` para pular as colunas que não são dados),
//!     `mover`, `na_ponta`, `focar`, `definir_linhas`; o intervalo de células ([`grade::Intervalo`], 0.2.2: a célula
//!     ativa é a âncora; o arrasto com a rolagem perto da borda, o Shift+clique e o Shift com as setas, PageUp/PageDown
//!     e Home/End o estendem, Esc e as teclas sem Shift o desfazem; fundo `--tint` e contorno `--acc`): `intervalo()`,
//!     `estender(celula)`, `recolher_intervalo()`, e [`grade::tsv_do_intervalo`] e
//!     [`grade::frase_do_intervalo_copiado`] para o Ctrl+C de quem chama; o conteúdo vem dos ganchos de
//!     [`grade::DesenhoGrade`]
//!     (`celula(ui, &CelulaVisivel)`, `cabecalho(ui, &CabecalhoVisivel)`, `fundo_linha` para a linha marcada —
//!     `--sel`, no lugar do hover —, `dica`, `th_selecionada(coluna)`: o núcleo pinta o `th` em `--sel` e as células
//!     da coluna em `--tint`) e [`grade::RespostaNucleo`] traz `clicou`, `editar` (duplo clique,
//!     Enter, F2), `texto_digitado` (começa a edição com a tecla), `copiar`, `copiar_com_cabecalho` (o Ctrl+Shift+C,
//!     que chega do egui-winit como `Event::Copy` com o Shift e é consumido pela grade com o foco), `menu_celula` (o
//!     menu da célula foi pedido), `excluir` (Delete), `espaco`,
//!     `selecao_mudou`, `intervalo_mudou`, `linha_sob_ponteiro`, `clicou_cabecalho` (`Some((coluna, `[`grade::Toques`]`
//!     { ctrl, shift }))` no clique no `th` de uma coluna navegável, onde o gancho não pôs um widget que ficou com o clique) e
//!     `menu_cabecalho` (o clique direito) e `clicou_numero_da_linha` (o clique no número da linha, que não é célula
//!     ativável). Etapa 6 (a planilha; tudo opcional, sem isso a grade é a de sempre): `com_congelado(linhas, colunas)`
//!     (as linhas ficam sob o cabeçalho e as colunas de dados, mais as especiais, grudam à esquerda, com o fio de 2px
//!     `--b2` na emenda; passando de 60% do que se vê numa direção que rola, o congelamento dela é ignorado no quadro:
//!     `congelado_efetivo`), `com_mesclas(Arc<`[`grade::IndiceDeMesclas`]`>)` (a célula de cima à esquerda pinta no
//!     retângulo todo pelo gancho `celula_mesclada(ui, &CelulaMesclada)`, as outras somem, o clique ativa a de cima;
//!     mescla de mais de 1.000 linhas ou 100 colunas não se desenha), `com_larguras_externas` (as do arquivo,
//!     [`grade::larguras_do_arquivo`]: `round(caracteres × 7 + 5)` entre 24 e 640, coluna oculta sai; a alça continua),
//!     `com_cabecalho_liso` (sem as riscas), [`grade::Th::centrada`] e [`grade::pintar_texto_alinhado`]; o Ctrl+PageUp
//!     e o Ctrl+PageDown ficam livres na `Consulta`. O exemplo vivo é o [`mostruario::planilha::PlanilhaExemplo`];
//!   - a [`grade::GradeSomenteLeitura`] da Consulta SQL, montada no núcleo (`new(id, colunas, linhas)`,
//!     `mostrar(ui, altura_max) -> RespostaGrade`; Ctrl+C copia o [`grade::texto_celula`] e mostra "Copiado.", ou o
//!     intervalo em TSV com "3 linhas × 2 colunas copiadas.", `texto_copiado()`).
//! - [`mostruario`]: [`mostruario::Mostruario`] — todas as peças numa tela (`ui(ui)` para uma janela;
//!   `pagina`/`sobreposicoes` para captura) e a casca nova inteira, só com as peças (`pagina_casca`; na janela,
//!   `mostrar_casca(true)` ou o "Ver a casca"). `examples/mostruario.rs` e `tests/mostruario.rs` gravam as
//!   capturas em `capturas/rust/`.
//! - [`fosco`]: o vidro fosco das camadas que se sobrepõem — [`fosco::ligar`] (no criador do app) faz o vidro
//!   ([`pecas::campos::forma_vidro_fosco`], [`pecas::campos::pintar_vidro_flutuante`] e as dicas do egui) pintar
//!   embaixo do `--flut` a foto da janela desfocada e saturada (o `blur(24px) saturate(150%)`); sem ele, o vidro liso.
//!
//! # Mudanças de API desta revisão (para quem já usava o crate)
//!
//! - Etapa 3, fechamento (a limpeza da revisão de código): saíram `Paleta::modal_opaco`, `Paleta::balao`,
//!   `Paleta::painel_menu` e `pecas::ajuda::cor_balao` (sem uso desde o vidro fosco) e o
//!   `pecas::campos::ALTURA_CAMPO_COMPACTO` (o compacto usa a [`pecas::campos::ALTURA_CAMPO_PEQUENO`], a mesma de
//!   26px). Entraram [`formato::sem_acento`] e [`formato::letra_sem_acento`] (as três cópias das telas saíram) e
//!   [`pecas::texto::titulo_lista`] (o título 13/600 das listas do documento). O [`icones::Icone::pngs`] acha a
//!   tabela pela chave num mapa, e o [`icones::pintar`] escolhe o PNG sem montar uma lista a cada quadro.
//! - Etapa 3, passo 8 (os ícones em PNG): `icones.rs` virou `icones/mod.rs` com o `png.rs` gerado; saíram
//!   `Icone::solido()`/`traco()` (agora [`icones::Icone::familia`]) e os desenhos provisórios do pintor (os
//!   sólidos de estado do [`pecas::ajuda`] também: [`pecas::ajuda::IconeEstado`] é o [`icones::EstadoIcone`]). O
//!   [`icones::Icone::Inicio`] é o sólido do trilho; a aba Início usa a silhueta [`icones::Icone::Casa`]. Entraram
//!   as variantes novas, `Botao::com_icone`/`lista`, o arco do ocupado, `Entrada::pastilha`/`somente_leitura`,
//!   `EntradaSugestoes::pastilha`, `etiqueta_mais`, `galley_caminho`, `Janela::icone`/`sub`/`largura` e o
//!   `PainelMenu::com_icones`/`icone` (o menu Arquivo com os sólidos de 24 em linhas de 36).
//! - Etapa 3, passo 3 (realce e aparência do código): o módulo [`codigo`]; em [`fontes`], a família do código
//!   trocável ([`fontes::trocar_mono`] e companhia) e a Consolas à frente da Cascadia Mono na cadeia
//!   monoespaçada; na [`pecas::campos::AreaTexto`], o modo código com as cores e a fonte do código (sem
//!   `tamanho_fonte`, o tamanho vem das Preferências) e `numeros_de_linha`, `marcar_linhas`, `sublinhar` e `cores`
//!   ([`pecas::campos::LARGURA_CALHA`]); no [`pecas::texto_pre::TextoPre::sql`], as cores e a fonte do código e
//!   `cores`; no mostruário, [`mostruario::Mostruario::pagina_codigo`], `mostrar_codigo` e `codigo_a_vista`.
//! - Etapa 3, passo 1 (tokens e pintura da revisão de 15/10/2026): entraram [`tema::Estudio::fbc`],
//!   [`tema::Estudio::fbc_hover`] (no lugar do `campo_hover`, que saiu), [`tema::Estudio::caixa_borda`] e
//!   [`tema::Estudio::ok_texto`]; [`pintura::paradas_primario`] (público, com o destaque a 25%),
//!   [`pecas::campos::cor_borda_campo`], [`pecas::rotulos::cores_pill`] (público),
//!   [`pecas::barra_status::RAIO_STATUS`], as cores do terciário que vira secundário
//!   ([`pecas::painel::cor_titulo_cab`], [`pecas::abas_documento::cor_sub_aba`], [`pecas::arvore::cor_contagem_no`],
//!   [`pecas::casca::cor_atalho_menu`]), [`pecas::texto::galley_tit_em`], o cabeçalho dos painéis, uma faixa de 23px
//!   ([`pecas::painel::ALTURA_CAB`], [`pecas::painel::TAMANHO_TITULO_CAB`]) com as bordas da linha da árvore
//!   ([`pecas::painel::RECUO_CAB`] = borda + [`pecas::arvore::MARGEM_NO`]) e [`pecas::arvore::RAIO_NO`] nos quatro
//!   cantos, com [`pecas::botao::Botao::do_cabecalho`]
//!   ([`pecas::botao::LADO_CABECALHO`]), o aviso em caixa ([`pecas::avisos::LARGURA_MAX_AVISO`],
//!   [`pecas::avisos::caixa_do_aviso`], [`pecas::avisos::aviso_faixa`], [`pecas::avisos::Aviso::ponta_a_ponta`]) e o
//!   toast de 37px com 16px dos dois lados.
//! - Etapa 2, as peças da casca nova (só acréscimos): o módulo [`icones`]; os módulos [`pecas::barra_titulo`],
//!   [`pecas::ferramentas`], [`pecas::abas_documento`], [`pecas::arvore`], [`pecas::painel`] e
//!   [`pecas::barra_status`]; [`pecas::botao::Botao::icone`], `pequeno`, `rail` e `principal` (com
//!   [`pecas::botao::LADO_PEQUENO`], [`pecas::botao::LADO_RAIL`] e [`pecas::botao::ALTURA_PEQUENO`]); o
//!   [`pecas::botao::Botao::selecionado`] passou a dizer o estado ao leitor de tela; [`pecas::casca::Menu::largura`]
//!   e, no [`pecas::casca::PainelMenu`], `separador` (agora público), `com_marcas`, `item_desligado`,
//!   `item_marcado`, `item_opcao`, `item_duplo` e `submenu`; [`pecas::campos::Entrada::atalho`], e a
//!   [`pecas::campos::Entrada::busca`] passou a ter a lupa; [`mostruario::Mostruario::pagina_casca`],
//!   `mostrar_casca` e `casca_a_vista`. No passo 12, o [`icones::Icone::NovaColuna`] (provisório: a coluna com o
//!   "+"), no lugar fixo da barra de ferramentas logo depois do Novo.
//! - Ajustes de 15/10/2026: entrou [`pintura::RAIO_PADRAO`] (10), o raio único das superfícies e dos campos —
//!   [`pecas::cartao::RAIO_CARD`] (cartão, documento e janela) passou de 14 para 10, e a lateral da casca e os
//!   painéis rebaixados das telas também; os campos ([`pecas::campos::Entrada`], busca, [`pecas::campos::Select`],
//!   [`pecas::campos::EntradaSugestoes`]) deixaram de ser pílula e têm o mesmo raio da [`pecas::campos::AreaTexto`]
//!   ([`pecas::campos::RAIO_CAMPO`]; o compacto continua com 4). As caixas de marcar passaram de círculo a quadrado
//!   de cantos de 5px ([`pecas::campos::RAIO_CAIXA`]); a opção continua redonda. No seletor segmentado as partes têm
//!   todas a largura da maior (medida em 600) e entrou [`pecas::navegacao::TamanhoTrilho::Cheio`] (o de 26px de ponta
//!   a ponta, o Formato do Exportar).
//! - Depois da junção com o `ajustes-raios` (16/10/2026): saíram de [`pecas::casca`] as peças da casca antiga, que
//!   só o mostruário usava — `topbar`, `fundo_topbar`, `RespostaTopbar`, `ALTURA_TOPBAR`, `lateral`,
//!   `LARGURA_LATERAL`, `grupo_lateral`, `titulo_grupo`, `item_lateral`, `EstadoItem`, `seletor_contexto`,
//!   `seletor_contexto_lista`, `lista_do_seletor_aberta` e `nota_lateral` (o mostruário mostra no lugar delas a
//!   barra de título, a barra de ferramentas e o Explorador da casca nova). [`pecas::avisos::RAIO_AVISO`] passou de
//!   8 para 10 ([`pintura::RAIO_PADRAO`]), o toast virou pílula ([`pecas::toast::RAIO_TOAST`], agora público), a
//!   barra de status ficou com o raio 10 e o `window_corner_radius` do [`tema::visuais`] passou de 14 a 10.
//! - Tokens, fontes e logo do DataBox: saíram a Avenir embutida (`fontes::AVENIR_LIGHT`, `AVENIR_ROMAN`,
//!   `AVENIR_OBLIQUE`, `AVENIR_MEDIUM`, `AVENIR_HEAVY`, `fontes::definicoes_embutidas`) e a carga em segundo plano
//!   (`fontes::carregar_do_sistema_em_segundo_plano`): [`tema::aplicar`] lê as fontes do sistema com
//!   [`fontes::instalar`] antes do primeiro quadro. Saíram os PNGs da marca antiga (`marca::LOGO_CLARO`,
//!   `LOGO_ESCURO`, `ICONE_AZUL`, `ICONE_OURO`, `PROPORCAO_LOGO`) e o módulo `tema::marca` (as cores `--cr-*`);
//!   [`marca::logo`] recebe o lado do quadrado (antes, a altura do logo horizontal). Entraram [`tema::Paleta::estudio`]
//!   ([`tema::Estudio`]), [`marca::decodificar_png`], [`fontes::instalar`], os
//!   `Preenchimento` `Risco`, `Aba` e `Cabecalho` e as primitivas [`pintura::riscas`],
//!   [`pintura::riscas_com_canto`], [`pintura::sombra_interna_baixo`], [`pintura::sombra_interna_topo`],
//!   [`pintura::halo`] e [`pintura::canto`]; [`tema::GAMA_TEXTO_CLARO`] e [`tema::GAMA_TEXTO_ESCURO`] passaram a
//!   0,8 e 0,6.
//! - Avisos e toast em tons: entraram [`pecas::avisos::TomAviso`], [`pecas::avisos::aviso_tom`] e
//!   [`pecas::avisos::Aviso::com_tom`] (`aviso(…, perigo)` e `Aviso::new(perigo)` continuam: atenção ou erro); o
//!   [`pecas::toast::toast`] passou a mostrar o sólido de informação, e entraram [`pecas::toast::toast_concluido`],
//!   [`pecas::toast::toast_atencao`], [`pecas::toast::toast_erro`] e [`pecas::toast::icone_toast`].
//! - O vidro num lugar reservado: entraram [`pecas::campos::forma_vidro_flutuante`] (depois do fosco, o liso das
//!   dicas; o das camadas é o [`pecas::campos::forma_vidro_fosco`]) e [`pecas::botao::forma_realce_curvo`]; a
//!   [`pecas::janela::Janela`] e o painel do [`pecas::casca::Menu`] pintam o vidro no mesmo quadro, embaixo do
//!   conteúdo, sem mexer na lista de formas do egui nem refazer a passada.
//! - `Entrada::mono` virou [`pecas::campos::Entrada::monoespacada`] (só o `input.mono` da janela de esquema: não
//!   use a opção no caminho do Início, no "Pedir o caminho" nem nos números da janela da linha). No Estúdio ela
//!   deixa de ser monoespaçada: é a fonte da interface em peso 350, o `.mono` das pranchetas.
//! - Campos do Estúdio: [`pecas::campos::ALTURA_CAMPO`] passou de 30 para 28px; entraram
//!   [`pecas::campos::Entrada::busca`] / [`pecas::campos::ALTURA_BUSCA`] e o vidro dos menus
//!   ([`pecas::campos::pintar_vidro_flutuante`], [`pecas::campos::pintar_item_menu`]).
//! - Navegação e menus do Estúdio: [`pecas::navegacao::abas`] ocupa a largura disponível (a faixa de 35px com o
//!   fio do documento embaixo, [`pecas::navegacao::ALTURA_ABAS`]); o seletor segmentado mede 32px (30px nos
//!   compactos) e não tem mais espaço entre os botões; o botão do [`pecas::casca::Menu`] passou a 24px
//!   ([`pecas::casca::ALTURA_MENU`]) e sem a seta, e o painel a 286px ([`pecas::casca::LARGURA_PAINEL_MENU`]); o
//!   "i" da ajuda passou de 18 para 16px; a barra de progresso, de 6 para 8px
//!   ([`pecas::progresso::ALTURA_BARRA`]); entraram [`pecas::ajuda::IconeEstado`] e
//!   [`pecas::ajuda::pintar_icone_estado`].
//! - Cartões, avisos, janelas e textos do Estúdio: a [`pecas::janela::Janela`] passou de 520 para 500px
//!   ([`pecas::janela::LARGURA_JANELA`], com o cabeçalho de [`pecas::janela::ALTURA_CABECALHO`] e o rodapé de
//!   [`pecas::janela::ALTURA_RODAPE`]); a ficha de [`pecas::detalhes`] tem o rótulo de 112px
//!   ([`pecas::detalhes::LARGURA_ROTULO`], antes 150) a [`pecas::detalhes::ESPACO_COLUNAS`] do valor, sem fios e
//!   sem empilhar abaixo de 480px; o título do [`pecas::cartao::Cartao`] mede 32px (42px com ações) e o rodapé
//!   34px; o aviso tem o sólido de estado e raio 8 ([`pecas::avisos::RAIO_AVISO`]); `link`, `nome_abre` e
//!   `Rotulo` passaram a 12px; `h4`, `eyebrow` e `rotulo_caps` desenham o `.tit` (o espaçamento pedido ao
//!   `rotulo_caps` fica sem efeito); `codigo` e o `Trecho::Codigo` usam a fonte da interface em 350; entraram
//!   [`pecas::texto::galley_tit`] e [`pecas::texto::linha_de_base`].
//! - Tabelas e grade do Estúdio: a [`pecas::tabela::Tabela`] tem o `th` de 30px ([`pecas::tabela::ALTURA_TH`],
//!   antes 38) e linhas de 32px ([`pecas::tabela::ALTURA_TD`]; `ALTURA_CONTEUDO_MIN` passou a 31; entrou
//!   [`pecas::tabela::Tabela::altura_linha`]) com `padding:0 12px` em todas as colunas ([`grade::padding_coluna`]
//!   devolve o padding da tabela: 12px, sem os 20px das pontas; [`pecas::tabela::PADDING_TABELA`]). A grade passou
//!   a linhas de 28px ([`grade::ALTURA_LINHA`], antes 35) e `th` de 30px em todos os casos (as quatro
//!   `ALTURA_CABECALHO*` e [`grade::altura_cabecalho`]: o tipo vai ao lado do nome e o `th` que ordena não tem mais o
//!   pontilhado), `padding:0 10px` ([`grade::PADDING_X`]; [`grade::PADDING_BORDA`] igual) com o fio de 1px à direita,
//!   o fio de baixo também na última linha; [`grade::CabecalhoVisivel::conteudo`] é o `th` inteiro sem o padding e
//!   os fios (o texto vai centrado); [`grade::largura_cabecalho`] soma o tipo ao lado do nome. Entraram
//!   [`grade::pintar_celula_chave`], [`grade::pintar_numero_linha`], [`grade::LARGURA_COLUNA_MARCAR`],
//!   [`grade::NucleoGrade::com_coluna_marcar`] e [`grade::NucleoGrade::com_coluna_numero`].
//! - [`pecas::avisos::aviso`] e [`pecas::texto::texto_rico`] devolvem [`pecas::texto::RespostaTexto`] (a resposta
//!   está em `.resposta`); [`pecas::texto::Trecho`] ganhou `Link` e `Ajuda`; [`pecas::texto::Paragrafo`] ganhou
//!   `faixas`; [`pecas::janela::RespostaJanela`] ganhou `enviou`.
//! - Os reais da grade, das dicas e da cópia saem como o JavaScript (`17051.0` → "17051"), não como o `repr`.
//! - `pintura::FATOR_DESFOQUE` saiu (a sombra agora é a gaussiana do CSS calculada na malha).
//! - [`tema::aplicar`] lê as fontes do sistema do disco na primeira chamada ([`fontes::instalar`], ~5 MB, antes do
//!   primeiro quadro), e não mais numa thread.
//! - Painéis laterais redimensionáveis: entrou [`pecas::divisoria::Divisoria`] (a alça do vão entre um painel e o
//!   documento: fio de 2px em `--acc` no hover, no arrasto e no foco, duplo clique, setas de 16pt, papel `Splitter`;
//!   com [`pecas::divisoria::LimitesLargura`], [`pecas::divisoria::LadoDoPainel`] e
//!   [`pecas::divisoria::SaidaDivisoria`]) e [`pecas::detalhes::largura_do_rotulo`] (a coluna do rótulo da ficha cede
//!   lugar ao valor numa ficha estreita).

#![forbid(unsafe_code)]

pub mod codigo;
pub mod fontes;
pub mod formato;
pub mod fosco;
pub mod grade;
pub mod icones;
pub mod marca;
pub mod mostruario;
pub mod pecas;
pub mod pintura;
pub mod tema;
