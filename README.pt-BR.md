# DataBox

[English](README.md) · **Português**

O **DataBox** é uma caixa de ferramentas de dados, gratuita e de código aberto, para desenvolvedores, cientistas e
engenheiros de dados. É um programa de mesa para Windows que abre, mostra e edita os arquivos do dia a dia: bancos
**SQLite**, pastas inteiras com os arquivos de dados delas, **CSV**, **JSON** e textos, e **planilhas do Excel** (só
leitura). É um executável só, `DataBox.exe`, sem instalação, sem runtime, sem navegador e sem servidor: a interface é
desenhada direto na placa de vídeo com [egui](https://github.com/emilk/egui), em Rust.

Começou como um leitor e editor de SQLite e está crescendo para virar uma caixa de ferramentas com várias funções
para quem trabalha com dados. Versão atual: **0.1.1**.

| Tipo | Extensões | Como abre |
|---|---|---|
| Banco SQLite | `.db`, `.sqlite` (e qualquer arquivo que seja SQLite por dentro) | edição completa; ou só leitura, se pedir |
| Pasta | qualquer pasta | o Explorador mostra os bancos e os arquivos de dados dela |
| CSV | `.csv`, `.tsv` | editor de tabela |
| JSON e texto | `.json`, `.txt`, `.md`, `.log` | editor de texto (só leitura acima de 0,5 MiB) |
| Planilha do Excel | `.xlsx`, `.xlsm` | só leitura |

**Sumário:** [Baixar](#baixar) · [Recursos](#recursos) · [Primeiro uso](#primeiro-uso) ·
[Preferências e log](#onde-ficam-as-preferências-e-o-log) · [Compilar](#compilar-a-partir-do-código) ·
[Mapa do código](#mapa-do-código) · [Próximos passos](#próximos-passos) · [Contribuir](#contribuir) ·
[Licença](#licença)

---

## Baixar

Baixe o `DataBox.exe` da última versão na página [Releases](../../releases/latest) e dê um duplo clique. Não precisa
instalar nada. Windows 10 ou 11, 64 bits.

> O executável não é assinado digitalmente; na primeira vez, o Windows SmartScreen pode avisar ("O Windows protegeu o
> computador"). Clique em **Mais informações › Executar assim mesmo**. Se preferir, compile você mesmo (abaixo).

**Linux (experimental):** baixe o `DataBox-…-x86_64.AppImage`, dê permissão de execução e abra (duplo clique, ou
`chmod +x DataBox-*.AppImage && ./DataBox-*.AppImage`). Não precisa instalar nada; `--criar-atalho` põe o DataBox no
menu de aplicativos apontando para o AppImage. Se a distribuição não tiver FUSE, rode com `--appimage-extract-and-run`.

---

## Recursos

### Em todo o programa

- **Abas de documento:** cada coisa aberta fica numa aba ("clientes · Dados", "Consulta 1", "vendas.csv"…), com
  **Voltar e Avançar** num histórico só, que atravessa as abas (Alt+← e Alt+→, ou os botões laterais do mouse).
- **Janela própria:** barra de título com os menus Arquivo, Editar, Exibir e Ajuda; barra de ferramentas que é
  sempre a mesma (cada aba liga só os botões que fazem sentido nela); Explorador e Histórico de consultas à esquerda;
  Propriedades à direita; barra de status embaixo.
- **Painéis redimensionáveis:** arraste a divisória entre o painel e o documento; duplo clique volta à largura padrão.
  A largura fica guardada para a próxima vez.
- **Vidro fosco** nos menus, nas listas e nas janelas, que ficam por cima do resto.
- **Tema** Claro, Escuro ou Seguir o Windows (Exibir › Tema).
- **Preferências** (Arquivo › Preferências, Ctrl+,): tema, cores e fonte do código, opções de exportação, limpar os
  recentes e o histórico de SQL. As mudanças aparecem na hora; só ficam gravadas no **Salvar**.
- **Busca da barra** (Ctrl+F) e a lista completa de atalhos em Ajuda › Atalhos do teclado.
- **Arrastar para a janela:** solte uma pasta, um banco ou um arquivo de dados na janela para abrir.
- **Ao fechar** uma aba, a pasta ou o programa com alterações não salvas, ele pergunta "Salvar as alterações em
  vendas.csv?" (Salvar · Não salvar · Cancelar).

### Pasta

- Arquivo › Abrir pasta (Ctrl+Shift+O). O Explorador mostra as subpastas, os bancos (reconhecidos pelo conteúdo do
  arquivo, não pela extensão), os CSV, as planilhas, os JSON e os textos.
- Ficam de fora os arquivos ocultos do Windows, os de bloqueio do Office (`~$planilha.xlsx`), as pastas de
  ferramentas (`.git`, `node_modules`…) e os de outros tipos (a dica da pasta diz quantos). Até 500 itens por pasta.
- Clique direito (ou Shift+F10) num item: Abrir, Atualizar, Mostrar na pasta, Copiar caminho (Ctrl+Shift+C), Fechar
  pasta. A árvore relê a pasta quando a janela volta a ter o foco.
- A pasta aberta ao fechar o DataBox abre de novo na próxima vez.

### Banco SQLite

- **Visão geral** do banco, com os contadores de tabelas, visões, índices e gatilhos.
- **Dados:** grade com busca, filtros, ordenação, edição no lugar, linhas marcadas, a ficha da linha (com BLOB, NULL
  e colunas geradas), Nova linha, Duplicar e Excluir.
- **Estrutura**, **Índices e gatilhos** e **Definição SQL** de cada tabela; Nova tabela, Novo índice e Alterar a
  coluna, sempre mostrando antes o SQL que vai rodar.
- **Consulta SQL:** quantas consultas quiser ao mesmo tempo, com cores no código, Histórico de consultas e o botão
  **Interromper** (Esc) para parar uma consulta longa.
- **Exportar** para Excel, CSV ou JSON (uma tabela, só o que a busca e os filtros mostram, as linhas marcadas, o
  resultado de uma consulta ou o banco inteiro), com barra de progresso.
- Abrir **só para leitura**, para olhar sem risco de mudar nada.

### CSV

- A grade mostra o arquivo inteiro sem carregar tudo na memória (um milhão de linhas abre sem esperar), com o tipo
  detectado de cada coluna no cabeçalho.
- Editar célula (duplo clique, Enter ou F2), Nova linha, Nova coluna, Duplicar, Excluir, Desfazer e Refazer.
- **Filtrar** com condições, como na aba Dados de um banco, e **ordenar** pela seta do cabeçalho.
- Copiar e colar colunas (Ctrl+C, Ctrl+V; Ctrl+Shift+V cola casando pelas chaves); renomear e excluir colunas pelo
  menu do cabeçalho.
- **Trazer colunas:** casa as linhas deste CSV com as de outro arquivo (outro CSV aberto, uma planilha do Excel, a
  área de transferência ou um arquivo do disco) por uma coluna em comum e traz as colunas escolhidas, com prévia e
  numa alteração só (desfaz de uma vez).
- O **Salvar** grava no mesmo formato do arquivo (codificação, separador, fim de linha) sem tocar no que não mudou.
  Se outro programa mudou o arquivo depois que ele abriu, nada é gravado e o aviso oferece **Ler de novo**.
- As Propriedades mostram o formato (e deixam trocar como o arquivo é lido) e as contas da coluna da célula ativa.

### JSON e texto

- Números de linha; no JSON, cores, o sinal de válido ou de erro (com a linha, a coluna e o motivo; o clique leva
  até ele), o botão **Formatar** (que não muda a ordem das chaves nem os números) e a estrutura do documento nas
  Propriedades.
- **Limite de edição de 0,5 MiB** (512 KiB, em [`crates/texto/src/limites.rs`](crates/texto/src/limites.rs)): acima
  disso, o arquivo abre só para leitura, até 32 MiB. A codificação, o fim de linha e a quebra no fim voltam como eram.

### Excel

- Só leitura. As planilhas ficam em abas embaixo (as ocultas aparecem com "Mostrar planilhas ocultas", nas
  Propriedades), com as larguras de coluna, as células mescladas e os painéis congelados do arquivo.
- Planilhas grandes mostram a primeira página na hora e terminam de ler em segundo plano.
- Ctrl+C copia a célula ou a coluna; o clique direito no cabeçalho oferece **Trazer colunas desta planilha** para o
  CSV aberto. Ctrl+PageUp e Ctrl+PageDown trocam de planilha.
- Arquivo com senha ou no formato antigo (`.xls`) diz o que fazer e oferece **Abrir no programa padrão**.

### Atalhos principais

| Atalho | O que faz |
|---|---|
| Ctrl+Shift+O / Ctrl+O | Abrir pasta / Abrir arquivo |
| Ctrl+S | Salvar |
| Ctrl+Z / Ctrl+Y | Desfazer / Refazer |
| F5 | Ler o arquivo de novo (pergunta antes, se houver alterações) |
| Ctrl+F | Buscar |
| Ctrl+, | Preferências |
| Ctrl+B / Ctrl+Alt+B | Mostrar ou esconder o Explorador / as Propriedades |
| Alt+← / Alt+→ | Voltar / Avançar |

---

## Primeiro uso

1. **Abra o programa.** Dê um duplo clique em `DataBox.exe`. Pode guardar o arquivo onde preferir: ele não precisa de
   nada ao lado e não instala nada.
2. **Abra uma pasta ou um arquivo.** Na tela de Início, clique em **Abrir pasta** e escolha a pasta onde ficam os seus
   bancos e planilhas; o Explorador mostra o que ela tem, e um clique abre o item. Para um arquivo só, use **Abrir
   arquivo**. Também dá para digitar o caminho no Início ou arrastar a pasta ou o arquivo para a janela. Para só olhar
   um banco, sem risco de mudar nada, marque **Abrir bancos só para leitura**.
3. **Crie os atalhos** na Área de Trabalho e no Menu Iniciar (uma vez só): abra o PowerShell na pasta do arquivo e
   rode `.\DataBox.exe --criar-atalho`. Depois, se quiser, fixe o DataBox na barra de tarefas.
4. **Algo deu errado?** Abra uma [issue](../../issues) com o arquivo de log (veja abaixo) e conte o que estava fazendo.

### Linha de comando

```text
DataBox.exe [caminho] [--somente-leitura] [--renderizador glow|wgpu] [--criar-atalho] [-h | --ajuda] [--versao]
```

- `DataBox.exe C:\dados` abre a pasta; `DataBox.exe C:\dados\vendas.db --somente-leitura` abre o banco só para
  leitura. Também serve para o "Abrir com…" do Windows.
- `--criar-atalho` cria os atalhos "DataBox" na Área de Trabalho e no Menu Iniciar e sai.
- `--renderizador wgpu` força o outro modo de desenho. Normalmente não é preciso: se a máquina não abrir OpenGL
  (máquina virtual sem driver de vídeo), o programa se abre de novo sozinho com o wgpu.

---

## Onde ficam as preferências e o log

| O quê | Onde |
|---|---|
| Preferências (tema, recentes, histórico de SQL, exportação, larguras dos painéis, pasta aberta) | `%APPDATA%\DataBox\preferencias.json` |
| Registro de problemas (log) | `%LOCALAPPDATA%\DataBox\databox.log` |

Em Ajuda › Sobre o DataBox, os links abrem essas duas pastas. Apagar o `preferencias.json` volta tudo ao padrão.

---

## Compilar a partir do código

### Ferramentas (uma vez, sem administrador)

- **Rust** estável pelo [rustup](https://rustup.rs), com a toolchain **GNU** (não precisa do Visual Studio; o build
  com a toolchain MSVC ainda não é suportado):

  ```powershell
  rustup-init.exe --default-host x86_64-pc-windows-gnu --profile minimal -c clippy -c rustfmt
  ```

- **MinGW-w64 da [WinLibs](https://winlibs.com)** (GCC 16, MSVCRT, sem LLVM), extraído em
  `%LOCALAPPDATA%\Programs\winlibs-gcc16` (a pasta `mingw64` fica dentro dela). Em outro lugar, aponte a variável
  `DATABOX_MINGW` para a pasta `bin` dele.
- Opcional, para os geradores de `design/ferramentas`: Python 3 (só biblioteca padrão) e, para rasterizar ícones, o
  Chrome.

### Compilar e rodar: sempre pelo `cargo.ps1`

Use **`.\cargo.ps1 …` no lugar de `cargo …`**, com os mesmos argumentos (o `--` vai entre aspas, `'--'`: sem elas o
PowerShell o engole). Ele põe o Rust e o MinGW no PATH e liga um
intermediário de link ([`ferramentas/vincular.rs`](ferramentas/vincular.rs)) que troca caminhos com acento pelo nome
curto 8.3 do Windows: sem isso, numa pasta de usuário com acento (`C:\Users\Conceição`), o `ld` da MinGW quebra com
"cannot find…".

```powershell
.\cargo.ps1 run -p amostras '--' exemplo.db     # cria um banco de exemplo na raiz
.\cargo.ps1 run -p editor '--' exemplo.db       # abre o DataBox com ele
.\cargo.ps1 build --release -p editor         # versão otimizada em target\release\DataBox.exe
```

Ou dê um duplo clique em [`compilar.bat`](compilar.bat): compila a versão otimizada e copia para **`dist\DataBox.exe`**.

Caminhos com mais de 260 caracteres quebram a compilação no Windows: deixe o projeto numa pasta rasa.

### Linux (experimental)

No Linux é o `cargo` direto, sem o `cargo.ps1`. No Debian/Ubuntu, instale uma vez os pacotes de compilação:

```bash
sudo apt-get install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
cargo build --release -p editor          # target/release/DataBox
./target/release/DataBox --criar-atalho  # põe o DataBox no menu de aplicativos (~/.local/share/applications)
ferramentas/appimage.sh                  # monta dist/DataBox-<versão>-x86_64.AppImage (precisa do curl)
```

Para rodar, ele usa os diálogos de arquivo da área de trabalho (`xdg-desktop-portal`), o `xdg-open` (pacote
`xdg-utils`) e as fontes Noto Sans ou DejaVu. As preferências ficam em `~/.config/DataBox` e o log em
`~/.local/state/DataBox`. A janela usa a moldura do sistema (a barra de título própria é só do Windows). O build do
Linux passa no CI, mas ainda foi pouco testado em áreas de trabalho de verdade: conte nas issues como foi.

### Testar

```powershell
.\cargo.ps1 test --workspace                               # tudo
.\cargo.ps1 test -p banco nome_do_teste                    # um teste só
.\cargo.ps1 test -p editor --no-default-features           # o editor sem as telas
.\cargo.ps1 clippy --workspace --all-targets '--' -D warnings # nenhum aviso
.\cargo.ps1 fmt --all '--' --check                           # formatação (largura 120)
```

- Os testes de tela renderizam cada tela sem abrir janela, com o `egui_kittest`, e gravam capturas PNG em
  `capturas\rust\` (fora do git) para comparar com as pranchetas de `design/canvas`.
- Se um teste de tela cair com `STATUS_ACCESS_VIOLATION` (0xC0000005), é o renderizador de software do Windows
  (WARP) sob carga, não defeito do código: rode o mesmo alvo de novo com `'--' --test-threads=1`.
- Os testes nunca usam o `%APPDATA%` real. Para testar o executável à mão sem sujar os seus recentes, aponte
  `APPDATA` e `LOCALAPPDATA` para uma pasta temporária antes de abrir.

### Publicar uma versão

Veja [`RELEASE.md`](RELEASE.md).

---

## Mapa do código

O workspace tem um crate por pasta em `crates/`, sem ciclos de dependência:

| Crate | O que faz |
|---|---|
| `comum` | Peças usadas por todos: o erro `ErroBanco`, `q()` para citar nomes SQL, os valores (`Valor`, `Celula`, `Chave`, `Entrada`), o nome e as pastas por pessoa, os tipos de arquivo de uma pasta. |
| `amostras` | Bancos de exemplo para os testes e para ver o app funcionando (`dados/exemplo.db`). |
| `ddl` | Tokenizador de SQL e analisador de `CREATE TABLE`, com o caminho de volta (montar o SQL de novo sem perder nada). |
| `esquema` | Planos de mudança de estrutura (tabelas, colunas, índices, visões e gatilhos), com a prévia do SQL antes de executar. |
| `banco` | Acesso ao arquivo SQLite: conexão, introspecção, páginas da grade com busca e filtros, gravação, consulta livre, leitura em fluxo para exportar. |
| `exportar` | Exportação em fluxo para Excel (`.xlsx`, gravado à mão), CSV e JSON. |
| `planilha` | Leitura de `.xlsx` e `.xlsm`, com um leitor XML próprio sobre o crate `zip`. |
| `tabular` | O núcleo do CSV (ler por páginas, alterações com desfazer, gravar no formato do arquivo) e o motor do Trazer colunas. |
| `texto` | Ler e gravar TXT, MD, LOG e JSON (codificação e fim de linha preservados), limites de tamanho e o analisador de JSON próprio. |
| `prefs` | As preferências em `preferencias.json`, os recentes e o histórico de SQL. |
| `servico` | As regras do banco aberto, dos destinos de arquivo, da exportação acompanhada e da pasta. |
| `interface` | O design system em egui: tema claro e escuro, fontes do Windows, logo, ícones, realce de código, vidro fosco e as peças. |
| `estado` | A ponte entre as telas e o `servico`: rotas e abas com o histórico único, os verbos da barra, as tarefas de fundo (`Promessa`), os diálogos e o contrato das telas (`Documento`, `ParteTabela`). |
| `tela_dados`, `tela_esquema`, `tela_consulta`, `tela_exportar` | As telas do banco: Dados, Estrutura/Índices/Definição SQL, Consulta SQL, Exportar. |
| `tela_csv`, `tela_texto`, `tela_planilha` | Os editores de CSV, de texto/JSON e o leitor de Excel. |
| `editor` | O executável `DataBox.exe`: a casca (janela, menus, barras, Explorador, abas, Propriedades, status, Preferências, Sobre), o Início, a Visão geral, a linha de comando e o log. |

Os detalhes estão no [`ARQUITETURA.md`](ARQUITETURA.md) (o contrato técnico) e no
[`design/README.md`](design/README.md) (o guia visual). A API de cada crate está documentada no topo do `lib.rs` dele.

---

## Próximos passos

O DataBox quer ser uma caixa de ferramentas de dados, não só um editor de SQLite. Ideias na fila (sugestões são bem
vindas nas [issues](../../issues)):

- outros bancos: DuckDB, PostgreSQL, MySQL/MariaDB e MongoDB (os ícones já existem no canvas);
- gráficos a partir de uma tabela, de uma consulta ou de um CSV;
- Parquet e outros formatos colunares;
- busca dentro do CSV e do texto; editar texto acima de 0,5 MiB;
- **Salvar como** com conversão de formato (por exemplo, CSV para Excel) e **Salvar tudo**;
- uma tabela do banco como destino do Trazer colunas;
- os formatos `.xls` e `.xlsb` do Excel;
- versões para Linux e macOS (o código já separa o que é do Windows).

---

## Contribuir

Contribuições são bem-vindas. Leia o [`CONTRIBUTING.md`](CONTRIBUTING.md): ele explica as convenções (o código, os
comentários e as mensagens são em português), como rodar os testes e como abrir um pull request.

---

## Licença

[MIT](LICENSE). A interface usa as fontes do próprio Windows (Segoe UI e Consolas), lidas do sistema, e o logo próprio.
As dependências diretas são MIT e/ou Apache-2.0; entre as indiretas há outras licenças permissivas (Unicode-3.0,
BSL-1.0, Zlib, ISC, MPL-2.0), e o executável leva as fontes padrão do egui (licenças SIL OFL 1.1 e Ubuntu Font
Licence), usadas como reserva. A lista das bibliotecas, com a licença de cada uma, está no próprio programa (Ajuda ›
Sobre › Licenças de terceiros) e, com os textos das licenças, no `THIRD-PARTY-NOTICES.txt` do `.zip` de cada versão.
