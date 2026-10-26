# DataBox — guia visual (visual Estúdio)

O guia da interface do DataBox: um visual moderno e sóbrio, inspirado em apps profissionais de criação, com tema
claro e escuro, vidro fosco nas camadas que se sobrepõem e ícones sólidos com volume. As pranchetas desta pasta são a
referência visual; o crate `interface` é a implementação em egui.

- **Situação:** todas as pranchetas estão implementadas no DataBox 0.1.0. O que cada prancheta virou está em
  "Implementação no Rust", abaixo; os próximos passos estão no [`README.md`](../README.md) da raiz.

## O que tem nesta pasta

| Pasta | Conteúdo |
|---|---|
| `canvas/` | As 21 pranchetas do canvas (mais a `LogoEditavel.dc.html`, fonte do logo, fora dele) (`*.dc.html`) e o `canvas.json`, publicados no [canvas do DataBox](https://claude.ai/artifact/TCdgEPdRzEzWEmC6XTJVu9) em quatro páginas: Fundamentos, Telas, Janelas e estados, Telas futuras. São páginas do formato de canvas de design: só abrem completas dentro do canvas (dependem do `support.js` dele), mas o HTML, o CSS (tokens e componentes) e o SVG de cada ícone estão inteiros e servem de fonte para a implementação. |
| `icones/` | Os ícones do app em SVG autônomo: `<nome>.svg` (o desenho de até 24 px) e `<nome>-grande.svg` (o de 48), e as silhuetas da barra em `sil/`. São a fonte dos PNG de `crates/interface/assets/icones` (ver `icones_app.py`, abaixo). |
| `logo/` | `databox.svg` (o logo, sempre o completo) e `databox-simples.svg` (a versão simplificada, **fora de uso** desde 17/10/2026, só como registro), em viewBox 512 e sem dependências. Em `fragmentos/` ficam os pedaços que os geradores embutem nas pranchetas. |
| `ferramentas/` | Os geradores em Python que montam o logo, os ícones e as pranchetas que ainda se refazem. Só biblioteca padrão. |

### As pranchetas

Organizadas em páginas do canvas (`pages` no `canvas.json`). A antiga 14 (`TrilhoOpcoes.dc.html`, as opções do trilho)
saiu em 19/10/2026 junto com o trilho. A `LogoEditavel.dc.html` (o logo em camadas soltas) saiu do canvas em 20/10/2026, mas
fica na pasta: é a fonte que o `logo_final.py` lê.

| Página | Arquivo | O que mostra |
|---|---|---|
| Fundamentos | `Regras.dc.html` | Anatomia da janela, cores (tokens), medidas e tipografia, e os padrões com exemplos de sim e não |
| Fundamentos | `Componentes.dc.html` | Botões, botões de ícone, seleção, campos, etiquetas, peças de dados, abas, menu, grade e mensagens |
| Fundamentos | `Icones.dc.html` | A biblioteca: ícones sólidos, silhuetas da barra e glifos de traço. Todo ícone de uma prancheta está nela |
| Fundamentos | `Logo.dc.html` | O logo (só o completo, todos os tamanhos) |
| Telas | `Main.dc.html` | Visão geral do banco, docas com abas, menu Exibir aberto (escuro) |
| Telas | `Inicio.dc.html` | Início sem pasta, botões "Painéis", menu Arquivo aberto (escuro) |
| Telas | `Dados.dc.html` | Tabela clientes: grade com filtro, colunas nas Propriedades, menu Tabela aberto (escuro) |
| Telas | `DadosClaro.dc.html` | clientes com as Propriedades em janela própria, transação aberta (claro) |
| Telas | `Consulta.dc.html` | Consulta 1 com o Histórico na doca e o menu Banco aberto (escuro) |
| Telas | `Csv.dc.html` · `Json.dc.html` | Editor tabular de CSV (escuro) e editor de texto com JSON (claro) |
| Telas | `Planilha.dc.html` | A planilha do Excel, só leitura, e os seus estados (escuro) |
| Janelas e estados | `Exportar.dc.html` · `Janelas.dc.html` · `Preferencias.dc.html` | Exportar; a ficha da linha, Alterar a coluna e Nova tabela; Preferências e Sobre |
| Janelas e estados | `Estados.dc.html` | Consulta executando e vazia, JSON inválido, fechar aba alterada, coluna nova e linhas marcadas no CSV, Histórico vazio, Copiar caminho, Desfazer no banco, ficha só leitura |
| Janelas e estados | `Colunas.dc.html` · `DadosArrastar.dc.html` | Copiar, colar e trazer colunas; arrastar a aba de um painel |
| Telas futuras | `FuturoPerfil.dc.html` · `FuturoConexao.dc.html` | Do roadmap: perfil de colunas nas Propriedades, conexão ao PostgreSQL |
| Telas futuras | `FuturoGrafico.dc.html` | Os gráficos da 0.3 (PR 7), em onze cenas: a vista Gráfico nas Propriedades, o gráfico em aba, os quatro tipos, vazio, carregando, a amostra, Copiar/Salvar/Python e o texto 40% mais longo (`grafico.py`) |
| Telas futuras | `FuturoSsh.dc.html` | O SSH da 0.3.0 (S1, só leitura), em sete cenas: o menu, a janela Conectar por SSH, a impressão digital da chave, o Explorador remoto com a aba "saida.csv · servidor", trazer com Interromper, o banco remoto como cópia e a conexão que cai (`ssh.py`) |

Pranchetas da onda 0 da 0.3, em 26/10/2026 (`FuturoGrafico` refeita sobre as decisões 6 e 7 da 0.3; `FuturoSsh`
nova, sobre o S1 de [`planos-ssh.md`](planos-ssh.md)). O que elas decidem, ainda **não implementado**:

- **Gráficos (PR 7 → PR 8 e 9):**
  - a prancheta de 13/10/2026 (um "Tabela · Gráfico" no resultado da Consulta, com rótulos de 130 px e uma legenda de
    uso) saiu inteira;
  - **vista "Gráfico"** nas Propriedades (Alt+6, depois do Perfil), para tabela, visão, CSV, planilha e o resultado da
    Consulta (segue o comando escolhido). De cima para baixo: o Tipo (os sólidos `g_barras`, `g_linhas`,
    `g_histograma`, `g_dispersao` da biblioteca num segmentado só de ícones, o nome do escolhido à direita do rótulo),
    as colunas do tipo (com a pastilha), a prévia (a largura da doca, 184 px, sem títulos de eixo), a contagem
    ("9 barras · 57 de 60 linhas"), "Abrir em aba" (o principal) e, embaixo, Copiar a imagem, Salvar como PNG e o
    Abrir no Python (sutil, com a explicação na dica). O app já escolhe as colunas ao abrir (a selecionada na grade e
    a primeira numérica);
  - **seis vistas não cabem com nome na doca:** o segmentado das vistas passa a só ícones na doca (Perfil com a Caixa,
    `g_caixa`; Gráfico com as Colunas, `g_colunas`; as outras com os sólidos de hoje) e por extenso na janela própria;
    a doca alarga para 340 px nas vistas Perfil e Gráfico;
  - **a aba do gráfico:** "Gráfico de clientes", com o sólido do tipo e o caminho na dica; endereço próprio (Voltar e
    Avançar passam por ela); fechar não pergunta. **Sem faixa**: o tipo e as colunas ficam nas Propriedades (a mesma
    vista, sem a prévia e sem o "Abrir em aba"); o título e a fonte vão **dentro** do gráfico, porque a imagem e o PNG
    precisam deles. Na barra, Atualizar lê a fonte de novo e Exportar salva o PNG (Ctrl+Shift+S, o Salvar como da
    aba); o rodapé conta à esquerda e mostra o ponto sob o cursor à direita (U4);
  - **os tipos:** barras **deitadas** (o nome da categoria inteiro à esquerda, em qualquer idioma; até 30, as outras
    numa barra "Outras (n)"); linhas com até três séries (`--s1` o destaque, `--s2` verde-azulado e `--s3` âmbar, nos
    dois temas; a legenda a partir de duas); histograma com as barras encostadas (1 px) e Faixas Automático/10/20/50;
    dispersão a 55% de opacidade. Os rótulos e eixos medem o texto, sem coluna fixa;
  - **estados:** sem coluna de números, a caixa de nota oferece "Contar por categoria"; escolha faltando diz o que
    falta e desliga as ações; carregando, a prévia fica a 35% com o arco e a aba mostra andamento e Interromper (como a
    N3); **amostra** acima de 100 mil pontos na dispersão, sorteada com semente fixa, com a etiqueta âmbar no rodapé e
    "amostra" no subtítulo do gráfico (a imagem não mente);
  - **o menu do gráfico** (clique direito): Copiar a imagem (Ctrl+C), Salvar como PNG (a silhueta do Exportar),
    Abrir no Python e Restaurar o zoom. Copiar leva o PNG a 2× com o fundo do tema da tela; sem Python, o script vai
    para a área de transferência e o aviso oferece vê-lo;
  - **o texto 40% mais longo** (pseudoidioma da P7) está na cena k: nada de largura pelo português; Agregar e Ordem
    descem um embaixo do outro, os botões passam a uma linha cada e o campo corta com "…" e a dica.
- **SSH (S1, PR 14):**
  - **Arquivo › Conectar por SSH** (sem reticências, pela regra dos menus; sem silhueta, pela U11), logo depois de
    Abrir arquivo, com Ctrl+Shift+K; o Explorador sem pasta oferece Abrir pasta e Conectar por SSH;
  - **a janela** (480 px, modal): Destino com as sugestões dos `Host` do `~/.ssh/config` (o nome e, embaixo, usuário,
    endereço e porta), Usuário e Porta na mesma linha, Pasta inicial ("a pasta de casa" vazia). A coluna dos rótulos
    mede o rótulo mais longo. Nenhum campo de senha de saída: Senha ou Frase da chave (com o nome do arquivo) só quando
    a máquina pede, e o principal vira "Entrar"; **sem "Lembrar"**. Conectando, os campos desligam e o principal gira;
    os erros vão na caixa de aviso, o da senha no campo;
  - **a impressão digital:** máquina nova mostra a SHA-256 em mono (seleciona inteira, Copiar), o tipo da chave e
    "Lembrar esta máquina" (desmarcado, grava no `known_hosts`), com "Confiar e conectar"; **chave mudada já chega
    recusada**: sem botão de aceitar, as duas impressões (a guardada riscada, a nova em `--perigo`), a linha do
    `known_hosts` e o `ssh-keygen -R`, com Copiar os detalhes e Fechar;
  - **o Explorador remoto:** a raiz `isaias@servidor:/srv/etl` com o sólido novo **Máquina remota** (`a_maquina`,
    seção Aplicativo da biblioteca) e o ponto verde da conexão; a dica tem a máquina, o endereço, o tempo conectado,
    a latência e a chave. Dentro, o Explorador de sempre;
  - **a aba remota:** "saida.csv" e, em `--t3`, " · servidor", com o endereço inteiro e a hora da cópia na dica. Tudo
    que é remoto abre **só leitura** na 0.3.0: a faixa leva o chip Somente leitura, de onde veio a cópia e "Trazer de
    novo" (o Atualizar, F5); Salvar e a edição desligam com o motivo na dica. As Propriedades contam a máquina, o
    caminho lá, as datas de lá e da cópia, e a cópia local. A barra de status mostra a conexão;
  - **trazer:** andamento na própria aba depois de 400 ms (MB, velocidade, tempo que falta) com Interromper; acima de
    1 GB pergunta antes; interrompido não deixa nada no cache;
  - **o banco remoto** abre a cópia como o banco só leitura de hoje (o cilindro cinza com cadeado); Editar e "Enviar
    ao servidor" não aparecem, nem desligados (são da 0.4);
  - **a raiz remota** tem Atualizar, Copiar caminho (no formato do `scp`) e Desconectar; quando a conexão cai, a raiz
    fica em `--t3` e a caixa de aviso oferece Reconectar, sem perder as abas.

Revisão de UI de 22/10/2026 (debate do designer com o PO, depois da 0.2.3; o debate inteiro, os achados U1 a U11 e
as capturas de antes estão em [`revisao-ui-0.3.md`](revisao-ui-0.3.md) e na página "Propostas 0.3" do canvas).
Decidido pelo Isaías em 22/10/2026, ainda **não implementado**:

- **Números na grade (U1):** os Dados e o resultado da Consulta mostram INTEGER e REAL no formato do idioma da
  interface (milhar, vírgula, algarismos tabulares), **sem arredondar**: todas as casas gravadas aparecem. A edição,
  a ficha da linha e o Copiar levam o valor gravado ("14542.56"). Preferências › Aparência ganha "Números na grade:
  Formatados (padrão) · Como gravados". O CSV e o Excel não mudam (lá o dado é o texto do arquivo e o formato da
  célula). Vai junto com a P7, que faz o `interface::formato` seguir o idioma. O que o app **calcula** sobre o CSV
  e o Excel (soma, média, mínimo, máximo, contagens, a coluna "#") segue o idioma, e ids, anos e CEPs saem sem milhar:
  ver [Números](#números-regras-de-26102026).
- **Rolagem contínua nos Dados (U2):** sai o "100 por página" e o "Página 1 de 1"; os Dados rolam como o CSV, o Excel
  e a Consulta, com "Linhas a–b de n" no rodapé e Ctrl+Home/Ctrl+End até as bordas. Na 0.3, no marco M1, antes do
  perfil.
- **Uma 0.2.4 de acabamento**, antes da varredura da tradução, com um PR pequeno por assunto:
  - **contagem num lugar só (U3):** no rodapé, "Linhas 1–24 de 236 · filtradas de 1.180"; a barra de filtros perde a
    contagem e guarda na ponta o "Limpar filtros" e, na busca, o "3 de 41";
  - **faixa do topo com uma regra só (U4):** a faixa existe só para controle ou contexto que a aba não diz (filtros,
    a migalha do JSON, as fontes da Consulta); a posição do cursor e as contagens moram no rodapé, à direita. O JSON
    troca o nome do arquivo pela migalha do ponto do cursor (raiz › conexoes › [0] › arquivo), com o sinal de válido e
    o Formatar; o TXT e a Visão geral ficam sem faixa; a Consulta leva "Linha, coluna" para o rodapé. **Toda aba de
    documento mostra o caminho inteiro na dica.** A faixa das fontes da Consulta (banco e arquivos da bancada, com
    "+ Arquivo") vem com a P4, na 0.3;
  - **vazio com as colunas à vista (U5):** a tabela sem linhas mantém o cabeçalho; o corpo diz "Nenhuma linha em
    extra_03" com o botão principal "Nova linha"; o filtro que não acha nada diz "Nenhuma linha com estes filtros"
    com "Limpar filtros". Vale para os Dados, o CSV e o resultado vazio da Consulta;
  - **caminhos cortados (U6):** guardam o nome inteiro e a pasta-mãe e cortam do começo ("…/dados/vendas_2025.csv");
    só um nome que não cabe sozinho corta no meio, guardando a extensão. O caminho inteiro na dica;
  - **seta da ordenação (U7)** logo depois do nome da coluna, também nas numéricas;
  - **Visão geral (U8):** a linha inteira abre a tabela (duplo clique e Enter); "Colunas" vira botão de ícone no hover
    e no foco; VIRTUAL e SEM ROWID viram etiqueta ao lado do nome; a coluna Tipo sai e entra a de Índices;
  - **rodapé da planilha (U9):** "Vendas › E6 · Somente leitura · 1.204 linhas · 6 colunas" (o intervalo e as
    mescladas ficam nas Propriedades);
  - **Consulta vazia (U10):** sem resultado, o editor ocupa a altura toda; a primeira execução abre o resultado.
- **Regras do guia (U11)**, nas seções abaixo:
  - **ícone em item de menu só quando o mesmo verbo tem botão na barra de ferramentas** (Abrir pasta, Abrir arquivo,
    Salvar, Novo, Excluir, Filtrar, Nova consulta, Exportar…): o menu ensina a barra. Vale para os menus da barra e os
    de contexto, e revê a de 20/10/2026 (silhueta em todo item que a biblioteca tem). Saem os ícones de Novo banco,
    Colunas, Renomear a tabela, Propriedades em janela própria, Fazer cópia de segurança e as manutenções do menu
    Banco (e o Copiar o nome do menu Tabela, que também não tem botão); o texto deles fica alinhado com os outros.
    Implementado na 0.3.0;
  - **a faixa de cima é para controle e contexto que a aba não diz; contagens e a posição do cursor ficam no rodapé,
    à direita** (U4);
  - **toda aba de documento tem o caminho inteiro na dica** (U4);
  - **nenhum botão, etiqueta, campo ou coluna com largura fixa pelo texto em português** (os três botões grandes do
    Início, a busca de 250 px, a altura fixa das Preferências): a largura sai do texto, e o pseudoidioma da P7 confere
    nas capturas.

Revisão de UI de 19/10/2026 (uso do app pelo PO), já nas pranchetas; cada prancheta tocada ganhou blocos de CSS
"revisão de UI de 19/10/2026" e de docas no fim do `<style>`:

- **Painéis em docas** (1, 2, 2b, 3, 4, 6, 7, 8, 15, 17). A prancheta 14 (opções do trilho) fica como registro. Explorador, Histórico e Propriedades são painéis; cada um
  mora num de três lugares: a doca da esquerda, a da direita ou uma janela própria do sistema. A doca troca o cabeçalho
  fino (`.cab`) por uma fileira de abas no estilo das de documento (`.aba`, `.abas-doca`), na mesma altura delas. As
  abas da esquerda (Explorador, Histórico) são só ícone (`.aba.so-ic`), com o nome na dica; a das Propriedades leva o
  nome e o objeto ("Propriedades · exemplo.db"). As ações do painel e a silhueta "Destacar em janela" ficam na ponta
  direita da fileira.
- **Arrastar a aba de um painel** (2b): sobre uma doca, prende lá; fora da janela, abre janela própria; Esc cancela. A
  posição de cada painel fica guardada.
- **Janela própria** (6): acabamento das janelas do app (`.dlg`), com a aba do painel no cabeçalho (arrastá-la de volta
  prende), a silhueta espelhada (prender de volta) e Fechar; segmentado na faixa; rodapé com as ações. Janela do sistema,
  não modal. Na largura de janela o conteúdo usa a forma larga (a tabela de colunas); na doca, a compacta (a lista).
- **Uma aba por objeto** (1, 2, 3, 6, 7, 8): o título da aba é só o objeto. As vistas da tabela (Resumo, Colunas,
  Índices, SQL) ficam num segmentado no topo das Propriedades; o documento da tabela é só a grade, e a coluna escolhida
  nas Propriedades acende na grade. "Todas as abas" (chevron) na ponta da fileira de documentos.
- **Menus Banco e Tabela** na barra de menus, entre Exibir e Ajuda, quando houver banco aberto e tabela ativa (Banco
  aberto na 3, Tabela na 2): no Banco, Visão geral, Nova consulta, Nova tabela, verificações, Otimizar, VACUUM, cópia de
  segurança, Exportar, só leitura e Fechar; na Tabela, as vistas (Alt+2…4), Abrir na Consulta, Exportar, Copiar o
  nome, Propriedades em janela própria, Nova linha/coluna/índice, Renomear e Excluir.
- **Sem trilho, por enquanto**: com só três painéis, o trilho repetia as abas da doca. Ele sai de todas as telas e
  volta quando houver mais recursos. Os atalhos dele ficam no Início (botões "Painéis": Explorador, Histórico de
  consultas, Preferências) e nos menus (Exibir › Início, Alt+Home; Exibir › Explorador, Propriedades, Histórico;
  Arquivo › Preferências).
- **Abas de uma doca têm uma forma só**: todas só ícone (doca estreita, como a da esquerda) ou todas com nome (doca
  larga, como a da direita); um painel preso numa doca adota a forma dela. Um fio (`.sep-doca`) separa as abas das
  ações do painel.
- **Explorador**: a tabela é uma folha (sem os filhos Dados, Estrutura, Índices, Definição SQL); as vistas ficam nas
  Propriedades e no menu Tabela. Na Visão geral, o botão da linha é "Colunas".
- **Janela própria**: Subir, Descer e Excluir aparecem só na coluna escolhida; a prancheta 6 mostra a janela fora da
  janela principal, sobre a área de trabalho, para não parecer modal.
- **Grade** (2, 7): seta de ordenação desenhada (`.seta-ord`); o CSV ganha a faixa com o formato em pílulas, a barra
  de filtros, a coluna de marcar e o rodapé "Linhas a–b de n", como os Dados.
- **Consulta** (3, 15): o Histórico mostra o resultado de cada execução em pílula, inclusive as falhas; estado novo
  "Consulta vazia", com a dica em itálico e sugestões que inserem SQL.
- **JSON inválido** (15): o motivo vira a faixa de erro (`.erro`, a da Consulta) com "Ir para o erro".
- **Desfazer no banco** (15): desfaz a última edição de célula, com aviso e Ctrl+Z.
- **Preferências e Sobre** (13): corpo de altura fixa (432 px); fonte do código "Padrão do sistema"; caminhos do Sobre
  com corte e Copiar caminho (`.cam`).

Pedidos de 15/10/2026 que valem para a implementação, além das pranchetas 12 a 14:

- **Avisos** (faixa no topo do documento e aviso dentro das janelas): triângulo de 24 px, centrado na altura do texto.
- **Avisos e notas em caixa** (dentro de janelas, fichas e documentos; não a faixa de ponta a ponta do topo do
  documento): a caixa tem a largura do texto, até 560 px, e fica centrada. Em janelas e documentos largos ela fica
  menor e no meio, com margem dos dois lados; num espaço estreito ela ocupa a largura toda e o texto quebra.
  É o padrão para mensagens assim.
- **Cabeçalho dos painéis** (Explorador, Propriedades, Histórico, `.cab`): 23 px de altura (0,6 dos 38 de antes), com
  botões de ícone de 20 px e título de 10,5 px. Vale para todas as telas.
  Desde 17/10/2026 o cabeçalho é uma faixa com a largura da linha selecionada da árvore: margem de 6 px dos lados e em
  cima (7 px da beirada, com a borda de 1 px do painel; o `margin:0 6px` do `.no`), os quatro cantos de 6 px (o raio do
  `.no`) e o fio `--btb` de 1 px em volta toda, no lugar do fio de baixo; o corpo começa logo embaixo e, na árvore,
  o respiro de 6 px de cima dá o mesmo vão até a primeira linha. No Rust, `RECUO_CAB` = borda + `MARGEM_NO`
  (`pecas/painel.rs`, `pecas/arvore.rs`). Vale para todas as interfaces; `ferramentas/cabecalho_na_linha.py`.
- **Outras decisões** (já no app e nas pranchetas):
  - o cabeçalho dos painéis na largura da linha da árvore (acima);
  - o vidro em toda camada que se sobrepõe, **fosco** a 78% (escuro) e 80% (claro), com o liso a 96% de reserva
    (ver "Superfícies, volume e efeitos");
  - a dock dos contadores da Visão geral **sem animação**: o visual fica, a subida de 3 px no hover sai;
  - **só o logo completo**, em todos os tamanhos (ver "Logo");
  - os **painéis laterais redimensionáveis** (ver "Anatomia da janela");
  - a janela abre com **1280 × 800** (mínimo 960 × 600), largo o
    bastante para as Propriedades ficarem à vista;
  - a versão do aplicativo é **0.2.4**: no Sobre, no recurso de versão do `DataBox.exe` e no workspace.
- **CSV:** botão Nova coluna logo depois de Nova linha (coluna com as divisas e um + à direita), que insere depois da
  coluna selecionada; um + no fim do cabeçalho da grade insere no fim. A coluna # tem 48 px e alarga com os algarismos.
- **JSON:** o estado do documento é só o ícone (concluído ou erro), com a dica ao passar o mouse ou com o foco.
- **Exibir > Tema** continua como atalho e ganha "Preferências de aparência"; o trilho tem Preferências no pé.
- **Cores do código:** DataBox (segue o tema; o padrão), Alto contraste, Solarized escuro e Monokai.
- **Ícones de arquivo:** a etiqueta colorida grande (40 × 21) até 24 px e a menor (36 × 16) de 32 px para cima.
- **Menu Arquivo:** ícones sólidos de 24 px (prancheta 4), decidido em 16/10/2026. **Revisto em 20/10/2026:** todos os
  menus da barra usam as silhuetas de 14 px (item de menu é ação, como na barra de ferramentas); os sólidos ficam para
  os objetos, como os arquivos dos Abertos recentemente. Abertos recentemente e Preferências ficam sem silhueta até
  haver o relógio e a engrenagem na biblioteca. A prancheta 4 ainda mostra os sólidos. **Revisto em 22/10/2026
  (U11):** a silhueta só vai no item cujo verbo tem botão na barra de ferramentas.

Da revisão crítica de 15/10/2026, aplicada às pranchetas (as que se refazem inteiras saem de `janelas.py`,
`preferencias.py` e `estados_novos.py`):

- **Preferências valem na hora**, como Exibir › Tema. Cancelar, Esc e o × voltam ao estado de quando a janela abriu;
  Salvar fica desligado até alguma coisa mudar; "Restaurar o padrão" vale só para a seção visível (a dica diz qual).
- **Fechar a aba alterada**, abrir outra pasta ou fechar o app pergunta "Salvar as alterações em vendas_2025.csv?",
  com Salvar, Não salvar e Cancelar (Esc). Na aba alterada, o × aparece no lugar do ponto no hover e no foco.
- **Histórico** (N5): o clique seleciona; o duplo clique ou Enter abre o SQL numa Consulta nova; o
  item corta em duas linhas, com o SQL inteiro na dica. Vazio, mostra "Nenhuma consulta ainda." e, em `--t3`, "As
  consultas executadas aparecem aqui.".
- **Consulta executando** (N3): Executar fica desligado com o arco girando (dica "Executando") e,
  ao lado, o Interromper (botão de risco, Esc) liga; ele tem o lugar dele na barra em toda aba, desligado fora da
  execução, para nada andar quando ele liga. O rodapé conta o tempo ("Executando · 1,4 s").
- **CSV:** marca várias linhas como os Dados; renomeia e exclui coluna pelo menu do cabeçalho e pela seção da coluna
  nas Propriedades; a coluna nova entra depois da coluna da célula ativa, como "coluna_7", com o cabeçalho já em edição.
- **Copiar caminho** no menu do item no Explorador e na seção Arquivo das Propriedades.
- **Ficha da linha só leitura** (Excel e banco só leitura): campos só leitura, NULL que não se marca, BLOB que só se
  salva, rodapé com Copiar a linha e Fechar.

## Números (regras de 26/10/2026)

Decididas pelo Isaías em 26/10/2026, a partir da [revisão do PO e do designer](revisao-po-designer-26-10.md) (PO3,
PO4, PO8, D11). Completam a U1 e a decisão 13 do [`planos-0.3.md`](planos-0.3.md); quem implementa a I1 e a trilha de
formatos confere as telas por esta lista. **Ainda não implementado**: o texto abaixo é o alvo, e a PR de código de cada
linha cita o item.

1. **De onde vem o número decide como ele aparece.**
   - **Lido do arquivo** (a célula do CSV e do Excel, na grade e na edição): o texto do arquivo, como está
     ("1056,30", "589.0893", "R$ 10.119,37"). O separador decimal do arquivo só serve para *ler* o número.
   - **Gravado no banco** (Dados, resultado da Consulta): INTEGER e REAL no idioma da interface (U1), ver 3 e 4.
   - **Calculado pelo app** (Soma, Média, Mínimo, Máximo e Contagem do rodapé da seleção e das Propriedades; a coluna
     "#"; as contagens de linhas): sempre no idioma da interface, em qualquer tipo de arquivo, **também no CSV e no
     Excel**. Português: "1.234,56". Inglês: "1,234.56". Um CSV com ponto decimal mostra "589.0893" na grade e "589,0893" no Mínimo em português: é
     de propósito (a grade é o arquivo, a conta é do app), e vale também com a interface em inglês e o arquivo em vírgula.
     Dois formatos *calculados* na mesma tela, nunca.
2. **Casas decimais.** A coluna tem as *casas* do maior número de casas que um valor dela tem (lido do texto, com os
   zeros finais: "20,90" tem duas). Limite: 15.
   - **Soma, Mínimo e Máximo** saem com as casas da coluna, **zeros finais mantidos** ("2.929.066.524,40", nunca
     "…524,4"). A soma é arredondada nessas casas (tira o ruído do ponto flutuante: 0,1 + 0,2 → "0,3"). Mínimo e
     máximo são calculados sobre o valor numérico, não copiados da célula.
   - **Média** (e qualquer divisão): as casas da coluna, **no mínimo 2** ("10.857,06", não "10.857,058571428572").
   - **Contagem** e **Vazias**: inteiros, sem casas.
   - Uma coluna de inteiros tem 0 casas: Soma, Mínimo e Máximo saem sem vírgula; só a Média tem 2.
3. **REAL gravado** (Dados e Consulta, sem calcular nada): a representação mais curta que volta ao **mesmo** número
   (o que o Rust faz com `{}`), sem arredondar e **sem notação científica**: 14542.56 → "14.542,56";
   1e-7 → "0,0000001"; 17051.0 → "17.051". "Sem arredondar" da U1 quer dizer isto. Consequência assumida: um
   `avg()` da Consulta mostra o que o SQLite devolveu ("10.857,058571428572"); quem quer duas casas escreve
   `round(avg(x), 2)`. O app não adivinha casas numa coluna que a consulta inventou.
4. **INTEGER** nunca passa por `f64`: 2^63 − 1 sai exato.
5. **Sem separador de milhar** (os algarismos colados: "2025", "12345", "01310100") em:
   - **chave primária e chave estrangeira** (o esquema declara; vale nos Dados e onde a Consulta traz a coluna com a
     origem conhecida);
   - a **coluna "#"** das grades (e o "Ir para a linha"): é identificação, não quantidade;
   - **colunas cujo nome indique identificador, ano ou CEP.** A heurística é a mais simples que funciona: o nome,
     sem acento e em minúsculas, é partido em palavras por `_`, `-`, espaço e a virada de caixa (`clienteId`), e a
     coluna é de identificação se **uma palavra inteira** for `id`, `ano`, `year`, `cep` ou `zip`
     (`id`, `cliente_id`, `id_pedido`, `ano`, `ano_fiscal`, `CEP`). Vale no CSV (que não tem esquema), no Excel e na
     Consulta, e só em coluna de inteiros. O valor não decide: "1234" pode ser quantidade.
   - **Limite da heurística:** só pega o que o *nome* diz. `anos_de_casa` e `identidade` ficam com milhar (a palavra
     inteira não bate, de propósito); um id chamado `matricula`, `pedido`, `sku` ou `numero` ganha milhar; um
     `qtd_por_id` perde o milhar que deveria ter. Nenhum desses erros muda o valor, só a leitura, e o Copiar leva
     sempre o número cru. A lista de palavras cresce quando aparecer caso real, não por adivinhação.
   - O que não for desses casos leva milhar normalmente (U1).
6. **NaN, infinito e científica.** O app escreve por extenso: "NaN", "Infinito" e "−Infinito" (em inglês "Infinity" e
   "−Infinity"), nunca "inf" ou "NaN" cru de biblioteca. Notação científica não sai de conta nem de REAL gravado
   (regra 3): "1,5E3" no arquivo vira, numa conta, "1.500". Texto do arquivo que não é número (inclui "NaN", "inf",
   "1e") é texto: fica de fora das contas e não conta como número.
7. **Alinhamento:** número alinhado à direita, algarismos tabulares; D10 (alinhar pela vírgula na Consulta) vem junto da
   U1.

**Como conferir** (cada regra vira um teste): a mesma coluna do CSV com ponto decimal mostra "589.0893" na grade e o
Mínimo/Máximo/Soma no formato da interface nas Propriedades e no rodapé; a Soma de "20,93" e "1,5" é "22,43"; a de
"2.000,10" e "3" é "2.003,10"; a coluna `id` e a "#" dão "2000" nas duas; `avg` de duas casas dá "10.857,06".

## Escopo e comportamento

- **Nome:** DataBox no título da janela, no executável (`DataBox.exe`), nos atalhos e na pasta de preferências
  (`%APPDATA%\DataBox`, com o registro em `%LOCALAPPDATA%\DataBox`).
- **Fontes do Windows, nada embutido:** Segoe UI Variable/Segoe UI na interface, Segoe Fluent Icons onde
  couber e Consolas só dentro dos editores de SQL e JSON.
- **Abrir pastas** (Arquivo › Abrir pasta, Ctrl+Shift+O): o Explorador mostra a árvore com os bancos e os
  arquivos que o app entende, cada tipo com o seu ícone.
  - **Bancos:** SQLite expande em Tabelas, Visões e Sistema. Os bancos da pasta abrem **um de cada vez**:
    abrir outro fecha as abas do anterior e pergunta antes se houver transação aberta.
  - **CSV:** editor tabular. Edita no lugar, inclui e exclui linhas, desfaz e salva com Ctrl+S; um ponto na aba
    indica alteração não salva.
  - **Excel:** abre na mesma grade, com as planilhas em abas embaixo, **só para leitura** (ver, buscar, filtrar,
    exportar). Gravar por cima de um .xlsx perderia fórmulas e formatação.
  - **JSON e texto:** editor com números de linha, cores no JSON, validação, Formatar e a estrutura do documento
    no painel Propriedades.
  - CSV, Excel, JSON e texto podem ficar abertos juntos, sem limite.
  - **O que a árvore lista**: pastas, bancos (reconhecidos pelos 16 primeiros bytes,
    `SQLite format 3`, e não pela extensão: o `Thumbs.db` do Windows fica de fora, um `.dat` que é SQLite entra; o
    arquivo de 0 byte com extensão de banco também é banco), `.csv`/`.tsv`, `.xlsx`, `.json` e `.txt`/`.md`/`.log`,
    nessa ordem e em ordem natural (`dados2` antes de `dados10`). Os ocultos do Windows, os `~$` do Office e as pastas
    de ferramentas (`.git`, `.svn`, `.hg`, `.vscode`, `.idea`, `__pycache__`, `node_modules`, `.venv`) não aparecem; o
    ponto no começo do nome não esconde. Os arquivos de outros tipos só entram na conta da dica da pasta.
  - **Limites:** uma pasta de cada vez, lida quando abre na árvore; 500 itens por pasta (o resto vira a linha "mais N
    itens"), 20.000 entradas lidas, 2.000 cabeçalhos e 5 s por leitura; a leitura cortada avisa na dica da pasta. Um
    arquivo que o OneDrive guarda só na nuvem não é aberto para ler o cabeçalho.
  - **Releitura** (sem vigia do sistema nem temporizador): o Atualizar do menu de contexto, a volta do foco da janela
    (no máximo uma vez a cada 2 s) e o fim do que o próprio app faz na pasta (o Novo banco, a Exportar, a cópia de
    segurança). Parado, nada é relido e nada repinta.
  - **A ficha** (até as etapas 5 e 6): o arquivo de dados abre numa aba que diz o que ele é (tipo, tamanho, data,
    caminho) e o abre no programa padrão do Windows ou na pasta dele. A pasta aberta volta na próxima abertura do app
    (o banco não); fechar a pasta fecha as fichas de dentro dela e mantém o banco.
- **Só no canvas, sem implementar por enquanto:** os 21 ícones de gráficos e visualizações e os de MySQL/MariaDB,
  PostgreSQL, DuckDB e MongoDB. Ficaram prontos para quando esses recursos vierem. Seis dos de gráficos (barras,
  linhas, histograma, dispersão, colunas e caixa) vão para o app com o PR 8 da 0.3 (a prancheta `FuturoGrafico`).

## Anatomia da janela

De cima para baixo:

1. **Barra de título integrada aos menus**, numa faixa só de 30 px na cor do fundo da janela: logo completo em
   16 px, menus Arquivo, Editar, Exibir, Banco e Tabela (quando há banco e tabela ativos) e Ajuda, título centralizado em `--t3` ("pasta · DataBox") e os botões
   minimizar, maximizar e fechar (46 px cada). A janela não usa a barra do Windows; o espaço livre arrasta a janela
   e o duplo clique maximiza. Decidido em 14/10/2026.
2. **Barra de ferramentas** num cabeçalho de vidro logo abaixo, com a busca à direita (Ctrl+F).
3. **Corpo:**
   - **Doca da esquerda** (Explorador e Histórico, abas só ícone). Sem barra de ícones desde 19/10/2026.
   - **Documento com abas** no centro, uma aba por objeto.
   - **Doca da direita** (Propriedades, aba com nome e objeto).
4. **Barra de status.**

- **Painéis laterais redimensionáveis** (desde 17/10/2026).
  O Explorador (que divide a coluna com o Histórico) e as Propriedades têm uma divisória no vão de 8 px entre o
  painel e o documento, **invisível em repouso**: sem barra nova. Com o ponteiro em cima, no arrasto e no foco do
  teclado, um fio de 2 px em `--acc` aparece no meio do vão (translúcido no hover, firme no arrasto e no foco), com
  o cursor de redimensionar. Arrastar muda a largura ao vivo, o duplo clique volta ao padrão (258 e 272 px) e, com o
  foco, as setas movem 16 px (Home e End vão ao mínimo e ao máximo, Enter volta ao padrão). Limites: **200 px** no
  mínimo, **45% da janela** no máximo e o documento nunca abaixo de **560 px**. Na janela estreita os dois painéis
  encolhem até 200 px antes de as Propriedades sumirem (~1052 px com as larguras padrão). A faixa `.cab`, a árvore
  e as fichas seguem a largura do painel. A escolha fica nas preferências (`larguras_paineis`) e volta na próxima
  abertura.

- **Barra de ferramentas universal.** É sempre a mesma e na mesma ordem: navegação · abrir pasta/arquivo e
  salvar · novo, nova coluna, duplicar, excluir, desfazer, refazer · buscar, filtrar, atualizar · executar,
  interromper, nova consulta e Exportar · painel. Cada aba decide o que cada verbo faz e desliga o que não se aplica,
  sem mudar nada de lugar. Por exemplo, o "+" é Nova linha nos Dados e Nova tabela na Visão geral; na Estrutura, liga
  a Nova coluna, que tem o lugar fixo logo depois do "+" e o mesmo ícone do CSV, e o "+" desliga (decisão D2, opção c). No CSV, a Nova coluna insere depois da coluna selecionada.
  - **Duplicar e Excluir** (item 19 da revisão): o Excluir age nas linhas marcadas e,
    sem marcas, na linha da célula ativa ("Excluir as 2 linhas marcadas", "Excluir a linha 7"). O Duplicar só tem um
    alvo, a marcada única ou a linha da célula ativa, e abre a ficha "Nova linha" já preenchida; com várias marcadas,
    ele desliga, com a dica "Duplicar (marque uma linha só)" (em lote, uma chave
    primária de texto bateria em UNIQUE e o BLOB não seria copiado). A dica sempre diz o alvo.
  - **Exportar** é um botão de ícone como os outros. A dica diz o que vai ser exportado ("Exportar clientes",
    "Exportar o banco"), e o botão fica desligado quando não há o que exportar.
- **Abas de documento:** "exemplo.db", "clientes", "Consulta 1", "vendas_2025.csv", "config.json": uma por objeto
  (19/10/2026). As vistas da tabela ficam nas Propriedades e no menu Tabela; no Explorador, a tabela é uma folha.
  - A aba ativa e a faixa logo abaixo dela formam **uma superfície só**, com a textura e o degradê dos botões. Os
    cabeçalhos de grade e de painel usam a mesma textura.
  - A aba ativa **não** tem barrinha colorida em cima.
- **Todo documento tem a mesma anatomia:**
  - em cima, a faixa de contexto, só para controle ou contexto que a aba não diz: filtros como etiquetas, a migalha
    do JSON, as fontes da consulta (U4, 22/10/2026: o nome do arquivo e o caminho não entram, porque já estão na aba,
    na dica dela e nas Propriedades; sem nada disso, o documento fica sem faixa);
  - no meio, o conteúdo;
  - embaixo, o rodapé: as contagens à esquerda ("Linhas 1–24 de 236 · filtradas de 1.180") e, à direita, a posição
    do cursor, a contagem da busca e as contas da seleção.
- **Abas:** a dica de toda aba de documento é o caminho inteiro do arquivo ou do banco (U4).
- **Menus:** Exibir › Tema (Seguir o Windows, Claro, Escuro) e mostrar Explorador, Propriedades e Histórico;
  Ajuda › Atalhos do teclado; Exibir › Início (Alt+Home).
- **Formato do CSV**: resumo no rodapé do documento, depois da contagem de linhas (separador, codificação, fim de linha; até a 0.2.2, em pílulas numa faixa que a 0.2.3 tirou); o detalhe e a troca ficam nas Propriedades: seção "Formato" (Separador,
  Decimal, Codificação, Fim de linha, "Primeira linha é o cabeçalho") e seção "Arquivo" (caminho, tamanho,
  modificado em).
- **Textos:**
  - Nenhuma reticência em botões nem em itens de menu.
  - Nenhuma largura fixa pelo texto em português (U11, 22/10/2026): botões, etiquetas, campos e colunas medem o
    texto, para o inglês e os outros idiomas da P7 caberem.
  - Nada de legendas de uso redundantes, como "duplo clique numa célula para editar": ficam só as contagens, e as
    explicações vão para as dicas.
  - Caminhos, tipos, andamento da exportação e histórico de SQL usam a fonte da interface em peso semilight, não
    monoespaçada.
- **Grades:** a coluna de seleção tem 32 px, sem linha divisória. No cabeçalho, o tipo de cada coluna vira uma
  pastilha (ver Ícones), e o tipo escrito vai para a dica.

## Superfícies, volume e efeitos

- **Fundo liso**, sem brilhos.
- **Painéis fixos sólidos**, um tom acima do fundo: barra de menus e ferramentas, docas (Explorador,
  Propriedades) e barra de status. Têm realce de 1 px em cima e sombra curta.
- **Vidro só onde há sobreposição, e em toda sobreposição:** todo elemento que fica por cima de outro tem o efeito
  de transparência da barra de menus (pedido de 17/10/2026): menus e submenus, menus de contexto, listas de opções e
  sugestões, popovers, dicas, janelas e o aviso flutuante. Nas pranchetas é a classe `.flut` (o `--flut` com o
  `--desfoque`, `blur(24px) saturate(150%)`); no app, o `forma_vidro_fosco` / `pintar_vidro_flutuante`. O fundo é 78%
  (escuro) e 80% (claro) opaco (eram 88% e 90%; mais transparente a pedido de 17/10/2026, `vidro_transparente.py`),
  com borda clara e realce de 1 px.
  - **Fosco no app** (pedido de 17/10/2026: "elegante, mas legível", com a transparência em 78%): o egui não desfoca
    o que está atrás, então o app tira a foto da própria janela quando a camada aparece, desfoca com o mesmo desvio
    de 24 px, satura a 150% e pinta embaixo do `--flut` (`interface::fosco`). O que está atrás vira manchas de cor,
    sem texto legível, e o texto da camada fica nítido. A camada aparece quando a foto chega (dois quadros); enquanto
    está aberta, o fundo fica como estava na foto (a troca de tema e a mudança de tamanho da janela refazem a foto).
    Sem a foto (testes de tela, janela minimizada, a foto que não chegou em 0,5 s), o vidro é o liso: o mesmo `--flut`
    a 96%, com o que está atrás só esmaecido, porque a 78/80% sem desfoque ele apareceria nítido. As dicas (as do
    egui e o balão de ajuda) ficam sempre no liso a 96%: uma foto de tela cheia a cada dica custaria mais do que a
    dica vale (revisão de código de 17/10/2026).
  - Nas pranchetas, a 78% o `blur(24px)` já apaga o texto de trás dos menus (conferido no Chrome em 17/10/2026):
    o desfoque ficou como estava.
- As camadas flutuantes ficam sempre acima da barra de abas.
- **Botões:**
  - Em pílula; os só de ícone são redondos.
  - Volume:
    - degradê de cima para baixo;
    - realce claro na borda de cima e sombra interna embaixo;
    - sombra projetada;
    - textura de riscas finas a 135°, bem discreta.
  - No hover, um halo na cor de destaque (vermelho nos de perigo). Ao clicar, o botão afunda.
  - O principal usa o degradê da cor de destaque, com a parada do meio em 25% (o branco do topo não apaga as
    letras); o de risco (Interromper, Excluir) é vermelho.
- **Campos de texto, busca, listas suspensas, campo com sugestões e área de texto:**
  - Retângulos de cantos arredondados com raio 10, o mesmo dos documentos e painéis (deixaram de ser pílula em
    15/10/2026). O campo compacto da edição no lugar da grade tem raio 4.
  - Baixo-relevo: sombra interna em cima, luz embaixo.
  - Borda com o token próprio `--fbc` (3:1 contra o fundo); campos desligados continuam com `--fb`.
  - No hover, a borda puxa para a cor de destaque; no foco, ganham anel e brilho.
  - **Só leitura** (coluna gerada, ficha só leitura): texto em `--t2` opaco, fundo `--p`, sem a sombra interna e com a
    borda `--fb`. Nada de esmaecer com opacidade. O NULL vai num campo normal, em `--t3` itálico.
- **Seletor segmentado** (o Formato do Exportar: Excel · CSV · JSON, o tema): trilho e opções em pílula, com as
  opções de larguras iguais (cada uma com a largura da maior). Num formulário ele vai de ponta a ponta da linha:
  Excel à esquerda, CSV no centro, JSON à direita.
- **Caixas de marcar e opções:**
  - A caixa de marcar (também a da grade e a de marcar a página) é quadrada, de 16 px com cantos de 5 px; a
    opção única é redonda. As duas com o volume dos botões.
  - Desmarcadas, borda `rgba(255,255,255,.35)` no escuro e `rgba(0,0,0,.5)` no claro (3:1 contra o fundo).
  - Marcadas, usam o degradê da cor de destaque com ✓ (caixa) ou um ponto (opção) em branco.
  - No hover, brilho como nos botões.
- **Barra de progresso:** trilho rebaixado; o preenchimento tem a textura, o volume e o brilho do botão principal.
- **Item em hover ou selecionado** (menus, Explorador, Histórico, etiquetas de filtro): realce de 1 px e brilho
  fraco.

## Tokens

Os valores exatos estão no `<style>` de cada prancheta (blocos `.est` e `.est.claro`; o último bloco `.est` da
folha, com `--painel`, é o que vale para as superfícies). Os principais:

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--acc` | `#2c6fdc` | `#2c6fdc` | cor de destaque (fixa no azul; as variantes verde `#0d7f74`, violeta `#6b4fd8` e laranja `#c2540a` ficaram só nas pranchetas) |
| `--fundo0` | `#111214` | `#e3e5e9` | fundo da janela |
| `--painel` | `#1e1f22` | `#f4f5f7` | painéis Explorador e Propriedades |
| `--c` | `#191a1d` | `#ffffff` | documento |
| `--p` | `#212226` | `#f6f7f9` | faixas e rodapés do documento |
| `--t` · `--t2` · `--t3` | `#ececee` · `#b9bbc0` · `#9597a0` | `#1b1c1f` · `#45474d` · `#62656c` | texto principal, secundário, terciário |
| `--bt1` → `--bt2` | `#3b3c40` → `#26272a` | `#ffffff` → `#e6e7ea` | degradê dos botões neutros, abas ativas e cabeçalhos |
| `--btb` · `--btluz` | `rgba(0,0,0,.6)` · `rgba(255,255,255,.13)` | `rgba(0,0,0,.18)` · `rgba(255,255,255,.95)` | borda e realce de 1 px dos botões |
| `--flut` | `rgba(30,31,35,.78)` | `rgba(255,255,255,.8)` | vidro de toda camada que se sobrepõe (menus, dicas, janelas, avisos) |
| `--fb` · `--fbc` | `rgba(255,255,255,.14)` · `rgba(255,255,255,.40)` | `rgba(0,0,0,.16)` · `rgba(0,0,0,.46)` | borda de trilhos, barras e prévias · borda dos campos |
| `--perigo` · `--ok` · `--aviso` | `#ff8a80` · `#5fd09a` · `#f2b65e` | `#c4262e` · `#0f7a4c` · `#8f5400` | estados (o texto de ok em pílula, no claro, é `#0b6a41`) |
| `--kw` · `--nu` · `--st` · `--chave` | `#8ab4f8` · `#f2b65e` · `#7fd1a8` · `#c9a0ff` | `#1f5fbf` · `#a35200` · `#0f7a4c` · `#7a3fc4` | cores de sintaxe (palavra-chave, número, texto, chave) |

- **Raios** (raio único de 10 px desde 15/10/2026; no Rust é uma constante só,
  `interface::pintura::RAIO_PADRAO`):
  - 10 px nos documentos, painéis, cartões, janelas, abas e campos de texto, busca e listas (retângulos
    arredondados, não pílulas);
  - 12 px no vidro dos menus; 6 px nos itens de menu e da árvore; 5 px nas caixas de marcar (as opções únicas
    são redondas);
  - 10 px também na barra de status, nos atalhos de SQL e nos avisos dentro das janelas;
  - pílula (999 px) nos botões, menus do topo, etiquetas, seletores segmentados (partes de larguras iguais),
    barra de progresso e aviso flutuante (toast).
- **Texto:** 12 px na interface. Títulos de seção em 11 px, peso 600, maiúsculas, espaçamento de 0,06 em e
  cor `--t3`. Números com algarismos tabulares. Sobre o cabeçalho em relevo, a aba ativa e a seleção, o terciário
  passa a `--t2` (o `--t3` cai abaixo de 4,5:1 nesses fundos).

## Ícones

São três famílias, desenhadas na grade de 48 e usadas em 48, 24 e 16 px.

1. **Silhuetas cheias**, para ferramentas e ações: barra de ferramentas, fileira de abas das docas
   e ações nas Propriedades.
   - Pasta, arquivo e duplicar usam dois tons.
   - Salvar, Nova linha, Excluir, Buscar e Painel têm recortes vazados.
2. **Sólidos esqueumórficos**, para objetos e estados:
   - degradê de cima para baixo;
   - realce branco em cima (`icRealce`), que termina antes do glifo ou do texto da etiqueta, e sombra embaixo (`icFundo`);
   - contorno `#000` com 30% (papel) a 35% de opacidade e largura 1 no desenho de 48;
   - até 24 px, o contorno fica mais firme para segurar as bordas no claro: o corpo com 2,4 de largura e 35%, a
     dobra do papel com 1,6 e 25% (revisão de 15/10/2026, item 13). Em `icones/`, `<nome>.svg` é esse desenho
     pequeno e `<nome>-grande.svg` o de 48;
   - sombra curta (`icSombra`: 1 px para baixo, desfoque 1,1, 40%).
3. **Glifos de traço**, só para o que é simples: setas de expandir, fechar (×), "+", os botões da janela e o
   indicador de carregamento.

Os sólidos, por grupo:

- **Arquivos.** Folha com dobra e uma faixa colorida com a extensão, legível também em 16 px; extensões de 4 letras
  usam letra menor.
  - Tipos: SQL `#2c6fdc`, CSV `#27854c`, TSV `#20826b`, XLSX `#1d7443`, JSON `#9c6c0c`, XML `#b95a25`, YAML
    `#b0447a`, PARQ `#1d8181`, TXT `#6f7783`, MD `#45505f`, LOG `#86684a`, ZIP `#927010`. Seis delas foram
    escurecidas no mesmo tom em 16/10/2026 para o texto branco passar de ~4,6:1 (eram CSV `#2e9e5b`, JSON `#c2860f`,
    XML `#d0652a`, PARQ `#1f8a8a`, ZIP `#b38a14`); o logo, a tabela virtual e os gráficos mantêm as cores antigas.
  - A classe `l4` (letra menor) vai nas etiquetas de 4 letras (XLSX, JSON, YAML, PARQ), e só nelas.
  - A pasta é um ícone próprio.
- **Bancos.** Cilindro metálico com duas faixas na cor do banco.
  - Cada faixa é a **área preenchida entre dois arcos com a mesma curvatura da tampa**, recortada pelo corpo. O
    contorno do corpo é desenhado uma vez só, depois das faixas.
  - Nunca use um traço grosso de arco no lugar da faixa: ele vaza pelas laterais e parece um anel por fora do
    cilindro. Foi reprovado duas vezes.
  - O SQLite leva a pena; o banco só leitura é cinza com um cadeado âmbar.
  - Os outros bancos levam um emblema no lugar da pena: golfinho no MySQL, elefante no PostgreSQL, pato no DuckDB e
    folha no MongoDB.
- **Objetos do banco:**
  - tabela: ficha com cabeçalho colorido `#2c6fdc`;
  - sem rowid: anil `#4f5fc9`, com uma chave (branca num círculo anil, no lugar e no formato do olho da visão);
  - visão: violeta `#7a52c7`, com um olho;
  - tabela virtual (FTS): `#1f8a8a`, com uma lupa;
  - do sistema: cinza `#6b7280`, com uma engrenagem;
  - índice: barras ordenadas;
  - gatilho: moeda âmbar com um raio.
- **Partes da tabela**, também em ícone sólido: Dados (grade), Estrutura, Índices e gatilhos (índice com raio) e
  Definição SQL (folha SQL).
- **Tipos de coluna.** Pastilhas com relevo e um glifo:
  - INTEGER `123`/`12` em `#2c6fdc`;
  - REAL (e NUMERIC) `1,5`/`,5` em `#1d8181`;
  - TEXT `Aa` em `#5f6b7a`;
  - BLOB `01` em `#7a52c7`;
  - data `31` em `#c8443b`;
  - booleano `✓` em `#27854c`;
  - JSON `{}` em `#9c6c0c`;
  - chave primária `PK` em `#927010`;
  - chave estrangeira `FK` em `#4a6fd1`;
  - NULL `∅` em `#6f757f`.
  - A pastilha segue o tipo declarado: `12` para INTEGER e `Aa` para TEXT, mesmo quando os valores são 0/1 ou datas;
    `✓` e `31` só quando o tipo declarado é BOOLEAN ou DATE/DATETIME. No CSV, que não declara tipo, vale o detectado.
  - Na ficha da linha (prancheta 12) a pastilha tem 24 px; nos campos de tipo e nos cabeçalhos, 16.
- **Estados:**
  - atenção: triângulo âmbar com "!";
  - erro: círculo vermelho com ×;
  - concluído: verde com ✓;
  - informação: azul com i;
  - ajuda: com "?";
  - bloqueado: cadeado de frente, com corpo de latão mais largo que alto, filete claro por dentro da borda, arco
    de aço tubular e fechadura gravada.
- **Máquina remota** (a raiz do SSH no Explorador, 26/10/2026): duas gavetas de servidor empilhadas, de metal claro,
  com a frente escura, o led verde e as frestas; no desenho de 48, o halo do led, os parafusos e três frestas; até
  24 px, o contorno firme e duas frestas. Na seção Aplicativo da biblioteca; ainda sem PNG no app.
- **Gráficos (só canvas):** colunas, colunas empilhadas, barras, linhas, área, pizza, rosca, dispersão,
  bolhas, histograma, caixa, mapa de calor, treemap, cascata, funil, radar, medidor, indicador, tabela
  dinâmica, mapa e painel.

## Logo

Caixa de papelão aberta, em isométrico, com as ferramentas saindo dela, mas ainda dentro: o banco de dados e o
lápis inclinados, a planilha, o gráfico de barras ao fundo e a lupa, com luz âmbar por trás. O acabamento é o mesmo
dos ícones: cilindro metálico com faixas de destaque, folha com cabeçalho, contornos escuros suaves e sombra curta
em cada peça.

- **Construção:**
  - As peças saem do `ferramentas/logo.py`, em projeção isométrica `P(x,y,z) = (cos30°·(x−y), ½(x+y) − z)`
    no viewBox 512.
  - A composição final é a posicionada à mão na prancheta 11 (`LogoEditavel.dc.html`): cada camada com
    posição, giro e escala próprios.
  - O `ferramentas/logo_final.py` converte as camadas em `<g transform>` e gera o `logo/databox.svg`, exatamente
    como o navegador desenha a prancheta.
  - Ordem das camadas, de trás para a frente: luz, abas de trás, raios, interior da caixa, gráfico, planilha,
    banco, lápis, lupa e frente da caixa. As peças que saem da caixa ficam entre o interior e a frente, então a
    frente as esconde quando descem.
- **Só o logo completo, em todos os tamanhos** (desde 17/10/2026): a versão simplificada
  (`logo/databox-simples.svg`, só a caixa e o banco, que valia de 32 px para baixo) **não é mais usada** em lugar
  nenhum, nem no app, nem no `.ico`, nem nas pranchetas. Os PNG pequenos (16, 20, 24, 32 e 40 px) saem do
  `databox.svg` desenhado direto no lado pedido, e não de uma redução do grande. O arquivo e o `simples.svgfrag`
  ficam só como registro (e para o `logo_so_completo.py` reconhecer o que trocar).
- **Onde usar:**
  - barra de título: 16 px;
  - barra de tarefas (o ícone da janela): 48 px, que o sistema reduz;
  - Início: completo, 60 px, ao lado do título "DataBox";
  - ícone do executável (.ico): 16, 20, 24, 32, 48, 64, 96, 128 e 256 px,
    montado por `ferramentas/ico.py` com os PNG de `crates/interface/assets/logo`.
- **Para mexer de novo no logo:**
  1. Ajuste as camadas na prancheta 11.
  2. Se o canvas tiver posto uma camada dentro de outra (acontece ao arrastar uma por cima da outra), rode
     `reordenar_logo.py`.
  3. Rode `logo_final.py`.

## Ferramentas

Python 3, só biblioteca padrão. Os geradores leem e gravam em `../canvas` e `../logo`. Rode-os de qualquer pasta.

Ficaram aqui só os geradores que ainda servem (refazem uma prancheta, o logo, os ícones ou o `.ico`, ou são
idempotentes).

| Script | O que faz | Situação |
|---|---|---|
| `logo.py` | Gera as peças do logo e os fragmentos (a versão simplificada continua sendo gerada, **histórica**: não é mais usada). Também refaz `Logo.dc.html` com a composição gerada, então rode `logo_final.py` logo depois. | vivo |
| `logo_editavel.py` | Monta `LogoEditavel.dc.html` com as camadas do `logo.py`. **Apaga os ajustes feitos à mão.** | vivo (cuidado) |
| `reordenar_logo.py` | Desaninha as camadas do logo editável, somando as posições dos pais, e as põe na ordem certa. | vivo |
| `logo_final.py` | Logo final a partir do logo editável: `Logo.dc.html`, o logo grande do Início e `logo/databox.svg` (o `databox-simples.svg`, histórico, continua sendo gravado). | vivo |
| `logo_so_completo.py` | Só o logo completo (17/10/2026): troca o logo simplificado das cópias pequenas (barra de título, 16 px; cabeçalho do Sobre, 20 px) das pranchetas pelo completo (`final.svgfrag`), no mesmo tamanho, e põe o filtro `lgSombraPeca` onde faltar. Idempotente (pula a prancheta sem logo simplificado) e recusa logo de conteúdo desconhecido sem gravar nada. `Logo.dc.html` fica com o `logo_final.py`. | vivo (idempotente) |
| `cadeado.py` | Ícone "Bloqueado" na prancheta 9. Com `--previa`, grava uma página para conferir o ícone ao lado dos outros estados. | vivo |
| `ico.py` | O ícone do executável, `crates/editor/recursos/databox.ico`, com os PNG do logo de `crates/interface/assets/logo` (uma entrada PNG por tamanho). Rode de novo quando o logo mudar; com `--conferir`, só lista o que o .ico tem. | vivo |
| `janelas.py` | Monta a prancheta 12 (`Janelas.dc.html`) com a folha de estilo, os ícones e o fundo do `Exportar.dc.html` e o triângulo de atenção do `DadosClaro.dc.html`. Já leva a revisão de 15/10/2026 (ficha de produtos P003 com pastilhas de 24 px, controles de verdade, plano do SQL, cores do SQL). Refaz a prancheta inteira: ajustes feitos à mão nela se perdem. | vivo |
| `tokens_oklab.py` | Resolve os `color-mix(in oklab, …)` das pranchetas em cores fixas, para a `Paleta` do Rust. | vivo |
| `rasterizar/` | Rasteriza SVG em PNG pelo navegador embutido ou pelo Chrome sem janela: `servidor.py` serve `rasterizar.html`, que desenha cada SVG num canvas e devolve o PNG por POST; `logo.json` lista os tamanhos do logo (só o completo, de 16 a 256 px) e `icones.json` (gerado pelo `icones_app.py`) os dos ícones do app. | vivo |
| `icones_app.py` | Os ícones do app em Rust: as silhuetas das pranchetas em branco em `icones/sil/`, os sólidos que faltavam em `icones/` (pelos moldes da biblioteca e o `exportar.py` da skill, com o desenho pequeno e o `-grande`) e a lista `rasterizar/icones.json`, com os PNG de `crates/interface/assets/icones` em 1×, 1,5× e 2× (mais 1,25× nas silhuetas). Recusa silhueta com dois desenhos e arquivo sem a marca dele. Os lugares que mostram o desenho `-grande` (a lista `GRANDES`: a dock da Visão geral) ganham PNG à parte, `<pasta>/<nome>-g-<px>.png`. `--conferir` só confere (SVG, lista, tamanho e canto transparente dos PNG, PNG sobrando); `--tabela` gera `crates/interface/src/icones/png.rs`. A receita do Chrome sem janela está na docstring. | vivo |
| `icones_resumo.py` | Os seis contadores da Visão geral (17/10/2026) numa dock: o desenho `-grande` de cada sólido (tabela, visão, índice, gatilho e os novos disco e linhas, tirados dos moldes da biblioteca) a 48 px, em pé sobre uma prateleira de raio 10 com sombra de contato e reflexo fraco, o número e o rótulo centrados embaixo, parados no hover (a subida de 3 px saiu a pedido de 17/10/2026) e duas prateleiras de três abaixo de 700 px de largura; o CSS fica entre os comentários "contadores na dock" da prancheta 1. Na primeira rodada eram sólidos de 16 px ao lado do rótulo. Também põe as formas `a_disco`/`a_discoG` e `a_linhas`/`a_linhasG` na seção Aplicativo da prancheta 9 (desenho de até 24 px e de 48). A marca `data-icones="resumo"` faz refazer os contadores a partir da biblioteca (aceita a faixa antiga, a de 16 px e a dock, guardando número e rótulo); recusa contador fora do formato, rótulo fora da ordem, forma mudada à mão, CSS com um comentário só e `url(#…)` sem definição. Rode o `icones_app.py` depois: os PNG da dock são `solidos/<nome>-g-48/72/96.png`. | vivo (idempotente) |
| `preferencias.py` | Monta a prancheta 13 (`Preferencias.dc.html`) com a folha de estilo e o fundo do `Exportar.dc.html` e o logo do `Inicio.dc.html`. Já leva a revisão de 15/10/2026 (Salvar desligado até mudar algo, 12,5 px, "Arquivo de log", contorno de 20 px). Refaz a prancheta inteira. | vivo |
| `estados_novos.py` | Monta a prancheta 15 (`Estados.dc.html`), os estados a implementar (N1 a N5 da revisão e os recursos da D9), com a folha do `Dados.dc.html` e as regras e peças das outras telas. Põe a entrada no `canvas.json`. Refaz a prancheta inteira. | vivo |
| `colunas.py` | Monta a prancheta 16 (`Colunas.dc.html`, 17/10/2026) com a folha, as definições e o script do `Estados.dc.html`, as regras de formulário do `Janelas.dc.html` e as peças das outras pranchetas; guarda a silhueta `trazer_colunas` na biblioteca (prancheta 9) se ela não estiver lá e põe a prancheta no `canvas.json` (x 0, y 8300). Refaz a prancheta inteira; recusa um `Colunas.dc.html` sem a marca dele, um ladrilho `trazer_colunas` diferente e um lugar ocupado no canvas. Depois dele, o `icones_app.py` leva a silhueta ao app. | vivo (idempotente) |
| `planilha.py` | Monta a prancheta 17 (`Planilha.dc.html`, 17/10/2026) com a folha, as definições e o script do `Colunas.dc.html` (que já traz os do `Estados.dc.html` e as regras de formulário) e as peças das outras pranchetas, sem ícone novo. Seis cenas com título e legenda, todas com o relatorio_q3.xlsx: a planilha aberta, muitas planilhas, a planilha grande lendo, os erros e a tela sem planilha à vista, o Trazer colunas com origem numa planilha e o menu do cabeçalho. A faixa das planilhas (`.folhas`, 32 px, planilhas em pílula de 24 px) e a grade com letras de coluna, mescla e painel congelado são as peças novas, para `pecas::folhas` e o `NucleoGrade`. Põe a prancheta no `canvas.json` (x 1560, y 8300). Refaz a prancheta inteira; recusa um `Planilha.dc.html` sem a marca dele e um lugar ocupado no canvas. Com `--conferir`, só confere o que já foi gravado (marca, lugar, as seis cenas, definições `url(#…)` e as peças da biblioteca). | vivo (idempotente) |
| `cabecalho_fino.py`, `cabecalho_na_linha.py` | O `.cab` dos painéis em duas camadas de regra no fim do `<style>`: `cabecalho_fino.py` (16/10/2026: 23 px de altura, botões de 20 px, título de 10,5 px) e, depois dele, `cabecalho_na_linha.py` (17/10/2026: faixa na largura da linha da árvore, margem de 6 px, cantos de 6 px, fio `--btb` em volta). Uma marca no `<style>` faz cada um pular a prancheta já ajustada; o segundo recusa a que ainda não tem a regra do primeiro. As pranchetas do logo ficam de fora. | vivo (idempotentes) |
| `propostas_03.py` | As pranchetas da página "Propostas 0.3" (`Propostas03*.dc.html`, 22/10/2026), da revisão em [`revisao-ui-0.3.md`](revisao-ui-0.3.md): cada uma sai de uma prancheta original (Dados, CSV, JSON, Consulta, Visão geral, `FuturoPerfil`, Preferências), com os ícones e as peças dela, e muda só o que a proposta muda; as etiquetas âmbar apontam os achados. A `Propostas03Hoje` (as capturas de antes) não sai dele. Refaz as pranchetas inteiras e recusa a original que mudou a ponto de um trecho esperado sumir; não mexe no `canvas.json`. | vivo (idempotente) |
| `pecas.py` | As peças comuns de `grafico.py` e `ssh.py` (26/10/2026): a folha, as definições e o script da prancheta 17 com as regras do segmentado do Dados; os botões e os ícones tirados das pranchetas pelo `aria-label`, pelo texto ou pela forma da biblioteca (`g_barras`, `st_info`…) e de `icones/`; as cenas, os campos, os itens de menu e os avisos; e o `gravar`, que guarda a página e o lugar da prancheta no `canvas.json` e recusa um lugar ocupado. Não grava nada sozinho. | vivo (biblioteca) |
| `grafico.py` | Refaz `FuturoGrafico.dc.html` (PR 7 da 0.3) com onze cenas e os gráficos em SVG desenhados por ele, com as cores dos tokens. A primeira vez pede `--substituir` (a prancheta à mão de 13/10/2026 não tinha a marca); `--medir` só imprime a página. | vivo (idempotente) |
| `ssh.py` | Monta `FuturoSsh.dc.html` (S1 do SSH) com sete cenas e põe a Máquina remota (`a_maquina`, `a_maquinaG`) na seção Aplicativo da biblioteca, se faltar; recusa a biblioteca com só uma das duas formas ou com a forma mudada à mão. Depois dele, o `icones_app.py` leva o ícone ao app (no PR 14). | vivo (idempotente) |
| `vidro_transparente.py` | O `--flut` mais transparente (17/10/2026): 78% no escuro e 80% no claro, no lugar de 88% e 90%, em toda prancheta que o define. Pula a que já tem os valores novos e recusa um `--flut` desconhecido. | vivo (idempotente) |

## Implementação no Rust

**Cada prancheta no app:**

| # | Prancheta | No app |
|---|---|---|
| 1 a 6 | Visão geral, Dados, Consulta, Início, Exportar, Estrutura com transação | feitas |
| 7 | Editor tabular de CSV | feita (`tela_csv`): grade paginada com a pastilha de tipo, `td.alt`, rodapé, Propriedades Formato, Arquivo e Coluna, barra de status do arquivo |
| 8 | Editor de texto com JSON | feita (`tela_texto`): números, cores, o círculo de estado, Formatar, estrutura e migalha; o TXT sem a validação |
| 9 a 11 | Ícones e logo | feitas |
| 12 e 13 | Janelas pesadas, Preferências e Sobre | feitas |
| 14 | Opções do trilho | decidida (B), feita |
| 15 | Estados | feita: N1 (JSON inválido), N2 (fechar a aba alterada, com a fila ao fechar a pasta e a janela), N3, N4 (a coluna nova no CSV), N5, D9 (linhas marcadas, renomear e excluir coluna, Copiar caminho); **fora:** a ficha da linha só leitura para planilhas (o banco só leitura já tem) |
| 16 | Copiar, colar e trazer colunas | feita, com o CSV filtrado (b) de verdade: filtrar e ordenar entraram |
| 17 | A planilha do Excel | feita (`tela_planilha`): cenas a (aberta, com mescla, congelado e oculta), b (muitas planilhas com a lista), c (grande lendo), d (erros e sem planilha à vista), e (Trazer colunas com a planilha) e f (menu do cabeçalho) |

**Como o visual vira código:**

- **Fontes do Windows**, carregadas de `C:\Windows\Fonts` em tempo de execução; nada embutido.
- **Tema:** os tokens claro/escuro acima e as variantes de destaque (`interface::tema`); as superfícies e o volume dos
  botões, campos e caixas feitos com o pintor do egui (degradês por malha, realces e sombras em camadas).
- **Ícones:** PNG rasterizados dos SVG desta pasta antes da compilação, pelo Chrome sem janela
  (`ferramentas/rasterizar/` e `icones_app.py`), e embutidos no `interface` com a tabela gerada `icones/png.rs`.
  Rasterizar em tempo de execução pediria uma biblioteca a mais (a feature `svg` do `egui_extras`, que traz o
  `resvg`). O `.ico` do executável sai dos PNG do logo (`ferramentas/ico.py`).
- **Documentos sem biblioteca a mais:** o editor de CSV (núcleo próprio, `tabular`), o de JSON e texto (analisador
  próprio, `texto::json`, que guarda a ordem das chaves) e o Excel `.xlsx`/`.xlsm` só leitura (leitor próprio sobre o
  crate `zip`, `planilha`).
- **Limites:** o texto se edita até 0,5 MiB (`texto::limites::LIMITE_EDICAO`); acima, o leitor só leitura até 32 MiB,
  com as cores por linha. O CSV não tem "Salvar mesmo assim": com o arquivo mudado por outro programa, nada é gravado e
  o aviso de topo oferece "Ler de novo".
- **Acessibilidade:** AccessKit com os mesmos papéis das pranchetas (aba, botão de alternar, cabeçalho ordenado, nome
  em cada caixa).
- **Testes:** as capturas de referência são as pranchetas; os testes de tela gravam em `capturas/rust/`. Teste o
  executável de verdade com `APPDATA` e `LOCALAPPDATA` apontando para uma pasta temporária, para não mexer nos
  recentes reais.
