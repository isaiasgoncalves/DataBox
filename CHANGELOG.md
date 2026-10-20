# Changelog

As mudanças de cada versão do DataBox. O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e as versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

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

[Não lançado]: ../../compare/v0.2.0...HEAD
[0.2.0]: ../../compare/v0.1.2...v0.2.0
[0.1.2]: ../../compare/v0.1.1...v0.1.2
[0.1.1]: ../../compare/v0.1.0...v0.1.1
[0.1.0]: ../../releases/tag/v0.1.0
