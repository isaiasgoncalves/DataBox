# Changelog

As mudanças de cada versão do DataBox. O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e as versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

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

[Não lançado]: ../../compare/v0.1.1...HEAD
[0.1.1]: ../../compare/v0.1.0...v0.1.1
[0.1.0]: ../../releases/tag/v0.1.0
