# Changelog

As mudanças de cada versão do DataBox. O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e as versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Alterado

- **A faixa do topo segue uma regra só** (revisão de UI de 22/10/2026, U4): ela existe para o que a aba não diz, e a
  posição do cursor e as contagens moram no rodapé, à direita.
  - **JSON:** a faixa não repete mais o nome do arquivo; mostra a migalha do ponto do cursor a partir da raiz
    ("raiz › conexoes › 0 › arquivo"), com o sinal de válido e o Formatar.
  - **Texto (TXT, MD, LOG):** sem faixa. A contagem da busca ("3 de 41") e, no arquivo grande, o "Somente leitura"
    vão para o rodapé (também no JSON).
  - **Consulta SQL:** "Linha 3, coluna 37" desce para a ponta direita do rodapé, como no JSON; a faixa fica com o
    banco.
  - **Visão geral:** sem faixa. O caminho do banco continua nas Propriedades, e o "só leitura", na Gravação delas.
- **A dica de cada aba de documento mostra o caminho inteiro** do arquivo ou do banco (nas abas da Visão geral, das
  tabelas e das Consultas).
- **A contagem das linhas fica só no rodapé** (revisão de UI de 22/10/2026, U3): com busca ou filtro, os Dados e o
  CSV mostravam "236 de 1.180 linhas" na barra dos filtros e "Linhas 1–24 de 236" no rodapé. Agora o rodapé diz tudo,
  "Linhas 1–24 de 236 · filtradas de 1.180" (no CSV, "contando…" enquanto a vista é calculada), e a barra dos filtros
  fica com as condições e o "Limpar filtros".
- **O vazio da grade mostra as colunas** (revisão de UI de 22/10/2026, U5): uma tabela sem linhas, um CSV só com o
  cabeçalho, um filtro que não acha nada e uma consulta que não devolve linhas mantêm o cabeçalho da grade (os nomes
  e as pastilhas de tipo), com a frase embaixo dele. A ação vai num botão, em vez de mandar procurar na barra:
  **Nova linha** na tabela e no CSV que gravam ("Nenhuma linha nesta tabela.", "Nenhuma linha neste arquivo.") e
  **Limpar filtros** quando o filtro não acha nada.
- **Caminhos cortados guardam o nome e a pasta-mãe** (revisão de UI de 22/10/2026, U6): quando não cabem, o Caminho
  das Propriedades, da ficha e das faixas, e a pasta dos Abertos recentemente no Início, perdem o começo, não o meio:
  "…/dados/vendas_2025.csv" no lugar de "/h…s/vendas_2025.csv", com as pastas mais perto do arquivo inteiras. O nome
  que não cabe sozinho é cortado no meio, com a extensão inteira ("vendas_20…25.csv"), e não mais no fim
  ("vendas_2025.c…"). O caminho inteiro continua na dica.

## [0.2.3] - 2026-10-22

### Adicionado

- **Clique direito numa célula ou num intervalo** nos Dados, no CSV, na planilha do Excel e no resultado da Consulta
  SQL (também com o Shift+F10, com o foco na grade; fora do intervalo, a célula vira a ativa e o menu age só nela):
  **Copiar**, **Copiar com cabeçalho** (Ctrl+Shift+C com o foco na grade; no Explorador continua o Copiar caminho) e
  **Copiar como** CSV, JSON, Markdown, `INSERT`, lista para `IN (…)` ou pandas (no CSV e no Excel, o `INSERT` leva o
  nome do arquivo ou da planilha); **Filtrar por este valor** e **Excluir este valor do filtro** nos Dados e no CSV
  (o NULL vira "é nulo", o vazio do CSV vira "é vazio"); **Marcar as linhas** nos Dados e no CSV; **Abrir a ficha da
  linha** nos Dados. O item que não vale ali fica desligado, com o motivo na dica.
- **Colar, Limpar, Definir como NULL e Excluir as linhas** no mesmo menu, nos Dados e no CSV. **Colar** (Ctrl+V): o
  que foi copiado (do Excel, de outra grade) cai a partir da célula ativa, por cima das células; um valor só preenche
  o intervalo selecionado; não cria linhas nem colunas, e o aviso diz quantas células ficaram de fora (no CSV, com uma
  coluna escolhida, o Ctrl+V continua colando como nova coluna). **Limpar** (Delete, sem linhas marcadas): o vazio no
  CSV; nos Dados, o texto vazio nas colunas de texto, o NULL nas outras e o valor padrão na que não aceita NULL.
  **Definir como NULL**, só nos Dados. **Excluir as linhas** do intervalo, com a mesma pergunta do Excluir da barra.
  Nos Dados, o Colar e o Limpar gravam numa transação só (tudo ou nada), perguntam antes acima de 100 células ("Colar
  120 células em clientes?"), avisam "Gravado em clientes (9 células)" e **se desfazem de uma vez** com o Ctrl+Z (se
  uma célula mudou depois, nada é desfeito e o aviso diz qual); no CSV, são um passo só no desfazer. Coluna gerada,
  banco só leitura e coluna que não aceita NULL desligam o item, com o motivo na dica.
- **O rodapé soma a seleção:** com mais de uma célula selecionada, "Contagem 33 · Soma 98.231,40 · Média 8.930,13 ·
  Mín. 154,03 · Máx. 17.810,34", como a barra de status do Excel (a contagem das células não vazias; as contas, das
  numéricas). Onde não cabe, saem o mínimo e o máximo, depois a média, e vão para a dica. No CSV grande, a conta roda
  em segundo plano com "Somando…".

### Alterado

- **O CSV não tem mais a faixa de cima:** o nome do arquivo já está na aba e nas Propriedades, e o formato (separador,
  codificação e fim de linha) foi para o rodapé, ao lado da contagem de linhas ("· Ponto e vírgula · UTF-8 · CRLF").
  A barra dos filtros sobe para o alto do documento, como nos Dados, e a contagem da busca ("3 de 41") fica na ponta
  direita dela. A grade ganha uma linha.
- **A planilha do Excel também não tem mais a faixa de cima:** a planilha e a célula ativa ("Vendas › B7") e o
  "Somente leitura" vão para o rodapé, antes da contagem de linhas e colunas, e a contagem da busca fica na ponta
  direita dele. Só a ficha do arquivo que não abriu (com senha, `.xls`) mantém a faixa, com o nome e o caminho. O
  texto e o JSON continuam com a faixa (o Formatar, o sinal de válido e o caminho do JSON).
- **Arquivo › Abrir no programa padrão**, ligado com o Excel, o CSV, o texto/JSON e a ficha de um arquivo (antes, só
  um botão na faixa do Excel e na ficha).

### Corrigido

- **Ctrl+F no CSV, na planilha do Excel e no texto/JSON:** a busca da barra ficava desligada nesses documentos. Agora
  ela procura no **arquivo inteiro**, não só no que está à vista (num CSV de 400 mil linhas, "onde aparece este CPF?"
  tem resposta), sem diferenciar maiúsculas nem acentos. A célula achada vira a ativa e a grade rola até ela (no texto,
  o trecho fica pintado); Enter e Shift+Enter vão ao próximo e ao anterior; a contagem diz "3 de 41" ("41+" enquanto
  conta); Esc limpa e volta à célula de antes. No CSV, conta as alterações ainda não salvas e só as linhas que o
  filtro mostra; no Excel, deixa de fora as colunas ocultas; no texto, vale também para os arquivos grandes, abertos
  só para leitura (até 32 MiB).
- **As contas da coluna do CSV não leem mais a coluna inteira para a memória:** a seção Coluna das Propriedades
  (vazias, mínimo, máximo, soma, maior texto) conta numa passada pelo arquivo, um valor por vez, com a memória
  constante num CSV de milhões de linhas (antes, centenas de MB a cada clique numa coluna de textos longos). Mostra
  "Calculando…" enquanto conta, e trocar de coluna ou alterar a tabela cancela a conta velha.
- **Desenvolvimento:** no CI do Windows, os testes de tela rodam num passo próprio, numa thread só e sem repetição (em
  paralelo, o renderizador de software do Windows caía com `STATUS_ACCESS_VIOLATION`).

## [0.2.2] - 2026-10-21

### Adicionado

- **Selecionar um intervalo de células** nos Dados, no CSV, na planilha do Excel e no resultado da Consulta SQL:
  arraste de uma célula a outra (a grade rola sozinha perto da borda), use Shift+clique ou Shift com as setas,
  PageUp/PageDown e Home/End (com Ctrl, até a borda). Ctrl+C copia o intervalo como o Excel, com tabulações, para colar
  numa planilha; no CSV, as linhas que ainda não estavam lidas são lidas na hora. Esc volta à célula ativa.
- Ctrl+C na aba Dados copia a célula ativa (antes não fazia nada fora da janela da linha).
- **Mover as abas de documento:** arraste uma aba na faixa para mudar o lugar dela (um fio na cor de destaque marca
  onde ela cai; Esc cancela). O clique direito na aba abre o menu com "Mover à esquerda", "Mover à direita" e "Fechar
  aba"; Ctrl+Shift+PageUp e Ctrl+Shift+PageDown movem a aba ativa.
- **Exibir › Barra de status**, para mostrar ou esconder a barra; a escolha fica guardada.
- **Gravação** na seção Arquivo das Propriedades (do banco e da tabela): "imediata", "transação aberta" ou "só
  leitura", com a explicação na dica.
- `--moldura-do-sistema`: abre a janela com a barra de título e as bordas do sistema.

### Alterado

- **Linux:** a janela usa a barra de título do DataBox, como no Windows: os botões Minimizar, Maximizar e Fechar
  ficam na linha dos menus, sem a barra do GNOME em cima (e sem o título repetido).
- **A barra de status fica escondida por padrão:** o que ela mostrava já está nas Propriedades (o modo de gravação, a
  versão do SQLite, a codificação), nos rodapés dos documentos e nas abas. Sem ela, o aviso que aparece embaixo fica
  logo acima do rodapé do documento.
- **Menus com o mesmo tipo de ícone:** o Arquivo troca os ícones coloridos pelas silhuetas da barra de ferramentas,
  como os menus Banco e Tabela, e as manutenções do Banco ganham as silhuetas que já tinham nas Propriedades.
- Na doca mais estreita, o seletor Resumo · Colunas · Índices · SQL das Propriedades mostra só os ícones, com o nome
  na dica, em vez de cortar os nomes com "…".

### Corrigido

- **Linux:** o DataBox segue o tamanho do texto do GNOME (Acessibilidade › Texto grande, ou o fator de escala das
  fontes no Ajustes): com 125%, a interface inteira e a janela inicial crescem 25%, e ele deixa de parecer menor que
  os outros programas na mesma tela.
- O canto de cima à esquerda do documento, com a primeira aba ativa, tinha um recorte: a faixa de cima do documento
  continuava arredondada embaixo da aba reta.

## [0.2.1] - 2026-10-20

### Adicionado

- **Desfazer no banco:** na aba Dados, a última edição de célula pode ser desfeita. O aviso da gravação diz onde
  gravou ("Gravado em clientes (linha 3, email)") e oferece Desfazer por 6 s; Ctrl+Z e Editar › Desfazer fazem o
  mesmo enquanto não houver outra mudança. O valor de antes é gravado com um UPDATE, só se a célula não mudou depois.

### Alterado

- Revisão de UI de 19/10/2026 (o uso do app pelo PO):
  - **Painéis em docas.** O Explorador, o Histórico e as Propriedades moram cada um numa doca (à esquerda ou à
    direita) ou numa janela própria do sistema. A doca tem uma fileira de abas no estilo das de documento (só ícone à
    esquerda, com o nome à direita: "Propriedades · clientes"), as ações do painel na ponta e o "Destacar em janela".
    Arraste a aba de um painel até a outra doca para prendê-lo lá, ou para fora da janela para abrir a janela própria
    (Esc cancela); a janela própria tem "Prender de volta" e "Fechar" (o painel volta escondido para a doca). O lugar
    de cada painel fica guardado.
  - **Sem a barra de ícones (o trilho):** com só três painéis, ela repetia as abas da doca. O Início ganhou os botões
    "Painéis" (Explorador, Histórico de consultas, Preferências) e o Exibir ganhou "Início" (Alt+Home).
  - **Uma aba por objeto.** A aba de uma tabela é só o nome dela ("clientes") e mostra a grade; a Estrutura, os
    Índices e gatilhos e a Definição SQL viraram vistas das Propriedades, num segmentado no topo do painel (Resumo,
    Colunas, Índices, SQL; Alt+1 a Alt+4). Na doca, as colunas vão numa lista compacta; na janela própria, na tabela
    de sempre. A coluna escolhida em Colunas acende na grade. No Explorador, a tabela é uma folha (sem os filhos), e
    na Visão geral o botão da linha virou "Colunas".
  - **Menus Banco e Tabela** na barra de menus, com banco aberto e tabela à vista: as manutenções, a cópia de
    segurança, a exportação, abrir só para leitura e fechar o banco; as vistas, Abrir na Consulta SQL, Exportar,
    Copiar o nome, Propriedades em janela própria, Nova linha, Nova coluna, Novo índice, Renomear e Excluir a tabela.
  - **"Todas as abas"** na ponta da faixa de abas, com o menu de todas as abas abertas; a primeira aba encosta na
    quina do documento.
  - **Preferências** com o corpo de altura fixa (a janela não muda de tamanho entre as seções) e a fonte do código
    padrão como "Padrão do sistema (Consolas)". No **Sobre**, os caminhos das preferências e do log são cortados com
    "…" e têm o Copiar caminho ao lado.

  - **Consulta SQL vazia:** a dica fica em itálico, na fonte da interface, e as sugestões "Para começar" (ler as
    primeiras linhas, contar e ver as colunas da primeira tabela) inserem o SQL no editor; o Executar fica desligado
    até haver texto, e o rodapé diz "Nada executado ainda".
  - **JSON inválido:** o motivo aparece na faixa de erro embaixo da faixa do documento, com a linha, a coluna e "Ir
    para o erro", que leva o cursor ao erro (também no leitor de arquivos grandes); o Formatar desligado diz "Corrija
    o erro para formatar".
  - **Grade:** a seta de ordenação do cabeçalho agora é desenhada (um triângulo na cor de destaque), e não mais um
    caractere da fonte, nos Dados e no CSV.
  - **CSV** com o formato da aba Dados: a faixa com o nome do arquivo e o formato em pílulas (separador, codificação e
    fim de linha), a barra de filtros sempre à vista embaixo dela e o rodapé "Linhas 1–26 de 1.204", no lugar de
    "1.204 linhas · 6 colunas".
- O Desfazer da aba Dados fica desligado, com o motivo na dica, depois de excluir ou incluir linhas, de mudar a
  estrutura ou quando uma transação da Consulta SQL começou ou terminou depois da edição (antes o Ctrl+Z não fazia
  nada e o menu continuava oferecendo).

## [0.2.0] - 2026-10-20

### Alterado

- O Histórico de consultas guarda também as execuções que falharam ou foram interrompidas: a consulta que falhou é
  justamente a que se quer recuperar. Cada item mostra, embaixo do SQL, uma pílula com o resultado ("9 linhas",
  "3 alteradas", "Parou no 3", "Erro", "Interrompida") e o horário ("10:42", "ontem · 18:20"), com quantos comandos
  rodaram quando o roteiro tem mais de um; a dica traz a mensagem do erro. No item selecionado, a pílula de erro
  fica só no contorno. O `preferencias.json` da 0.1.x continua abrindo (os itens antigos ficam sem pílula).

### Corrigido

- Linux: as setas e os triângulos (ordenação da grade, "Alt+← e Alt+→" nos Atalhos do teclado) apareciam como
  quadrados vazios. A fonte de símbolos agora tem reservas fora do Windows (DejaVu Sans, Noto Sans Symbols2).
- Um arquivo de texto sem nenhuma quebra de linha mostrava "Fim de linha: CRLF" fora do Windows; agora vale o fim de
  linha do sistema (CRLF no Windows, LF nos outros).
- Linux: as dicas dos campos de caminho (Início, Abrir arquivo, Novo banco, Salvar como) mostravam `C:\pasta`;
  agora seguem o jeito de escrever caminho do sistema.
- Ctrl+Home e Ctrl+End na grade dos Dados e do CSV vão à primeira e à última linha (antes não faziam nada).

## [0.1.2] - 2026-10-19

### Adicionado

- Versão para Linux (experimental): o `.tar.gz` de cada versão traz o binário `DataBox`. "Abrir no programa padrão"
  e "Mostrar na pasta" usam o `xdg-open`, e `DataBox --criar-atalho` põe o DataBox no menu de aplicativos (um
  `databox.desktop` com o logo). O CI roda fmt, clippy e os testes também no Ubuntu.
- **AppImage** para Linux em cada versão (`DataBox-vX.Y.Z-x86_64.AppImage`): um arquivo só, sem instalar, compilado
  no Ubuntu 22.04 para rodar em mais distribuições. Aberto de um AppImage, o `--criar-atalho` aponta para o próprio
  `.AppImage`. O `ferramentas/appimage.sh` monta o pacote na máquina de quem compila, e rodar o CI à mão (Run
  workflow) também gera um, para testar antes de publicar.

### Corrigido

- Linux: os caminhos absolutos (`/home/…`) não viram mais relativos à pasta do programa (o `/` era trocado por
  `\`), então bancos, pastas e arquivos abrem pelo caminho; o `~` usa o `HOME`, e a caixa das letras diferencia
  arquivos, como no sistema. Fechar a pasta fecha as abas dos arquivos de dentro, o Explorador acha o banco aberto
  na árvore, e o caminho sugerido em "Novo banco" e "Salvar como" usa `/`.

### Alterado

- Fora do Windows, o tema "Seguir o Windows" se chama "Seguir o sistema", e as frases falam no "programa padrão do
  sistema".

## [0.1.1] - 2026-10-18

### Adicionado

- `THIRD-PARTY-NOTICES.txt` no `.zip` de cada versão: as bibliotecas de terceiros com a licença de cada uma, os
  textos de licença que vêm nelas e as fontes padrão do egui embutidas no programa (SIL OFL 1.1 e Ubuntu Font
  Licence).

### Segurança

- Planilhas do Excel com textos compartilhados acima de 512 MB na memória (como um zip bomb no `sharedStrings.xml`)
  dão erro em português em vez de esgotar a memória.
- Bancos abertos com `trusted_schema` desligado: visões, gatilhos e padrões gravados no arquivo só usam funções
  inofensivas do SQLite.

### Alterado

- Os leitores de arquivo (CSV, texto/JSON, `CREATE TABLE` e Excel) não indexam mais sem conferir o tamanho: um arquivo
  malformado dá erro em vez de derrubar o programa (travado pelo clippy, `indexing_slicing`).

## [0.1.0] - 2026-10-18

Primeira versão pública.

### Adicionado

- **Bancos SQLite:** Visão geral; aba Dados com grade, busca, filtros, ordenação, edição no lugar, linhas marcadas e a
  ficha da linha (BLOB, NULL, colunas geradas); Estrutura, Índices e gatilhos e Definição SQL, com a prévia do SQL
  antes de cada mudança; Nova tabela, Novo índice e Alterar a coluna (com reconstrução segura da tabela); Consulta SQL
  com várias consultas ao mesmo tempo, Histórico e Interromper; manutenção e cópia de segurança; abrir só para leitura.
- **Exportar** para Excel (`.xlsx`), CSV e JSON, em fluxo e com progresso: uma tabela, a busca e os filtros, as linhas
  marcadas, o resultado de uma consulta ou o banco inteiro.
- **Pastas:** o Explorador com os bancos e os arquivos de dados da pasta, menu de contexto e releitura ao voltar o foco.
- **CSV:** editor de milhões de linhas sem carregar tudo, com tipo detectado por coluna, edição, desfazer, filtrar e
  ordenar, copiar e colar colunas e **Trazer colunas** de outro arquivo por uma chave em comum.
- **JSON e texto:** editor com números de linha, cores, validação com linha e coluna, Formatar e estrutura do
  documento; leitor só leitura para arquivos grandes.
- **Excel** (`.xlsx`, `.xlsm`) só leitura, com planilhas ocultas, mescladas e painéis congelados.
- Interface própria em egui (visual Estúdio): tema claro e escuro, vidro fosco, abas de documento com histórico único,
  painéis redimensionáveis, Preferências, atalhos do teclado e acessibilidade (AccessKit).
- Executável único, `DataBox.exe`, sem instalação; `--criar-atalho` para os atalhos do Windows.

[Não lançado]: ../../compare/v0.2.3...HEAD
[0.2.3]: ../../compare/v0.2.2...v0.2.3
[0.2.2]: ../../compare/v0.2.1...v0.2.2
[0.2.1]: ../../compare/v0.2.0...v0.2.1
[0.2.0]: ../../compare/v0.1.2...v0.2.0
[0.1.2]: ../../compare/v0.1.1...v0.1.2
[0.1.1]: ../../compare/v0.1.0...v0.1.1
[0.1.0]: ../../releases/tag/v0.1.0
