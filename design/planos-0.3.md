# Planos da 0.3

Discussão do escopo da versão 0.3 do DataBox, escrita em 21/10/2026, logo depois da 0.2.2. Dois papéis conversam: o
**PO** (Product Owner, que fala pelos cientistas de dados, engenheiros de dados e desenvolvedores que usam o app) e o
**TL** (Tech Lead, que fala pela arquitetura, pelo risco, pela dívida técnica, pelos testes e pelo CI). No fim, um
plano conciliado e as decisões que ficam com o Isaías. Nada aqui está implementado.

**Decidido em 21/10/2026** (a lista completa na seção 5): (1) antes da 0.3 sai uma 0.2.3 só de refinamentos e correções — o menu da seleção, o
Ctrl+F nos arquivos, as contas do CSV em fluxo, o CI confiável e o `app.rs` fatiado —, descrita em
[`planos-0.2.3.md`](planos-0.2.3.md); (2) os gráficos no app (P6b) sobem para a 0.3.0; (3) a 0.3.0 traz a tradução
oficial para o inglês, com a interface pronta para receber outros idiomas (P7).

Fontes: o Roadmap e o "Na fila" do [`README.pt-BR.md`](../README.pt-BR.md), o [`CHANGELOG.md`](../CHANGELOG.md) da
0.1.0 à 0.2.2, o [`ARQUITETURA.md`](../ARQUITETURA.md), as revisões de UI de 15 e 19/10/2026 neste
[`README.md`](README.md), o [`RELEASE.md`](../RELEASE.md), os workflows em `.github/workflows/`, as últimas 40
execuções do CI e o código. Não há issues nem pull requests abertos: o uso real que existe é o das revisões do PO.

---

## 1. Onde estamos

- **Seis versões em quatro dias** (0.1.0 em 18/10, 0.2.2 em 21/10). A 0.2.x foi quase toda casca e acabamento: docas,
  uma aba por objeto, menus Banco e Tabela, barra própria no Linux, seleção de intervalo, abas móveis, Desfazer no
  banco. O motor de dados quase não mudou desde a 0.1.0.
- **O que o app faz com cada tipo de arquivo hoje:**

  | | Ver | Editar | Filtrar/ordenar | Ctrl+F | Exportar/converter | Estatística |
  |---|---|---|---|---|---|---|
  | SQLite | sim | sim | sim | sim | Excel, CSV, JSON | não |
  | CSV | sim | sim | sim | **não** | **não** | contas da célula ativa (vazias, mín., máx., soma) |
  | Excel | sim | não | sim | **não** | **não** (só "Trazer colunas" para um CSV) | não |
  | JSON/texto | sim | até 0,5 MiB | — | **não** | — | — |

- **A seleção de intervalo (0.2.2) não tem o que fazer além do Ctrl+C.** Nenhuma grade tem menu no clique direito
  da célula: nos Dados, no CSV, no Excel e no resultado da Consulta, o clique direito numa célula ou num intervalo
  não faz nada. Só os cabeçalhos do CSV e do Excel têm menu (`tela_csv::colunas`, `tela_planilha::menu`). Quem
  seleciona não descobre que pode copiar, e não tem como filtrar, limpar, excluir ou exportar o que selecionou
  (observado pelo Isaías no uso da 0.2.2, em 21/10/2026).
- **Código:** ~158 mil linhas em 21 crates (130 mil em `src/`, com os testes unitários; 28 mil em `tests/`), sem
  ciclos. Os maiores arquivos são `editor/src/app.rs` (4.479 linhas), `editor/tests/casca.rs` (4.979) e
  `interface/tests/pecas.rs` (5.439).
- **CI:** das últimas 40 execuções, 12 falharam (algumas por clippy, de verdade). As de teste, nas recentes, são do
  Windows e de tela: o `casca_explorador_e_status` (o tamanho do arquivo abaixo da dobra das Propriedades, corrigido
  no #11), o WARP com `STATUS_ACCESS_VIOLATION` sob carga e o `interface --test mostruario`, que às vezes cai dentro
  do wgpu em paralelo. O passo de testes do Windows roda tudo e, se falhar, **repete tudo numa thread só**: a falha
  intermitente vira tempo de CI e fica escondida.
- **egui:** o projeto está no 0.36.2 (fixo); o 0.37.0 saiu em 15/10/2026. O `egui_plot` 0.37 ainda pede o egui
  0.36; o 0.38 já pede o 0.37.

---

## 2. A proposta do PO

> *"A 0.2 deixou a casa arrumada. A 0.3 tem que responder à pergunta que a pessoa faz quando abre um arquivo de dados:
> **o que tem aqui, e como eu levo isso para onde preciso?**"*

**Tema: "Entender e levar os dados".** A pessoa abre um CSV ou uma planilha que alguém mandou, quer saber em segundos
se presta (nulos, tipos, distribuição), achar um valor, e tirar dali no formato que a próxima ferramenta pede. Hoje
ela entra no DataBox e precisa sair dele para três dessas quatro coisas.

### Itens, em ordem de prioridade

**P0. Agir sobre a seleção: o menu de contexto da grade.** *(Decidido em 21/10/2026: sai antes, sozinho, na 0.2.3;
fica aqui pela descrição.)*
- *Problema:* a 0.2.2 ensinou a pessoa a selecionar um intervalo, mas parou no meio: o clique direito não faz nada, e
  o Ctrl+C só é achado por quem já conhece o Excel. É o gesto mais natural de uma grade, e hoje ele não responde.
- *Pronto quando:* o clique direito numa célula ou num intervalo (e o Shift+F10 ou a tecla Menu, com o foco na
  grade) abre um menu, nos Dados, no CSV, no Excel e no resultado da Consulta, com o que vale em cada um:
  - **Copiar** (Ctrl+C, como hoje), **Copiar com cabeçalho** e **Copiar como ›** CSV, JSON, Markdown, `INSERT` do
    SQL e lista para `IN (…)` (uma coluna só);
  - **Filtrar por este valor** e **Excluir este valor do filtro** (a célula ativa), que viram uma condição na barra
    de filtros de sempre;
  - nos editáveis (Dados e CSV): **Colar** (Ctrl+V), **Limpar** (Delete; nos Dados, o NULL onde a coluna aceita) e
    **Excluir as linhas** do intervalo, com o Desfazer de cada um;
  - **Marcar as linhas** do intervalo (Dados e CSV) e **Abrir a ficha da linha** (Dados);
  - o rodapé mostra, com mais de uma célula selecionada, "Contagem · Soma · Média" das numéricas, como o Excel.

  Depois, quando chegarem: **Exportar a seleção…** (com o Salvar como, P3) e **Perfil da seleção** (com o P1).

**P1. Perfil de colunas.**
- *Problema:* o cientista de dados abre `vendas.csv` e a primeira coisa que faz é `df.describe()` e
  `df["regiao"].value_counts()` num notebook. O DataBox mostra a grade, mas não responde "quantos nulos?", "quantos
  valores distintos?", "isso é mesmo um ID?". No CSV, as Propriedades já dão vazias, mínimo, máximo e soma da célula
  ativa: é o começo, sem distribuição e sem nada no banco nem no Excel.
- *Pronto quando:* para uma coluna de uma tabela do banco, de um CSV ou de uma planilha, as Propriedades mostram
  linhas, nulos/vazios (número e %), distintos, mínimo, máximo, média, mediana e desvio (numéricas), os 10 valores mais
  frequentes com a contagem, e um histograma pequeno (numéricas e datas). Um "Perfil da tabela" mostra todas as
  colunas numa grade só (uma linha por coluna), exportável. Um CSV de 1 milhão de linhas dá o perfil de uma coluna em
  poucos segundos, com progresso e Interromper, sem travar a interface.

**P2. Buscar dentro do arquivo (Ctrl+F no CSV, no Excel e no texto).**
- *Problema:* o Ctrl+F funciona nos Dados e na Consulta, mas no CSV, no Excel e no JSON não faz nada. Isso parece
  defeito, não falta de recurso. "Onde aparece o CPF 123…?" num CSV de 400 mil linhas não tem resposta.
- *Pronto quando:* Ctrl+F na barra busca no arquivo inteiro (não só na página carregada), com Enter/Shift+Enter para
  o próximo e o anterior, "3 de 41" na faixa, a célula achada selecionada e a grade rolada até ela; no texto e no JSON,
  o trecho pintado como na Consulta, também no leitor de arquivos grandes (até 32 MiB). Opções: diferenciar
  maiúsculas e acentos, só a coluna ativa.

**P3. Salvar como e converter (CSV, Excel, SQLite, JSON).**
- *Problema:* o Excel é só leitura e não sai do DataBox de jeito nenhum; o CSV também não vira Excel. O pedido mais
  comum do engenheiro de dados ("me manda isso em CSV", "joga essa planilha num banco") obriga a abrir outro programa.
- *Pronto quando:* Arquivo › Salvar como… (Ctrl+Shift+S) no CSV, na planilha (a aba ativa ou todas) e numa tabela
  do banco grava em CSV (com o separador e a codificação escolhidos), Excel, JSON, JSON Lines e **SQLite** (um banco
  novo ou uma tabela nova no banco aberto), com os tipos detectados virando os tipos das colunas (INTEGER, REAL, TEXT,
  datas em ISO), prévia das colunas e dos tipos, progresso e Interromper. Um milhão de linhas de CSV vira SQLite em
  menos de 15 s. "Salvar tudo" grava todos os documentos alterados.

**P4. SQL sobre CSV e planilha.**
- *Problema:* "quanto vendeu cada região?" num CSV é um `GROUP BY`. Hoje a pessoa importa no pandas ou no DuckDB.
- *Pronto quando:* no CSV ou na planilha, "Consultar com SQL" abre uma Consulta SQL em que o arquivo é uma tabela
  (`SELECT regiao, sum(valor) FROM vendas GROUP BY 1`), com o Histórico, o Interromper e o Exportar de sempre. Dá para
  juntar dois arquivos abertos num `JOIN`. O arquivo original nunca é alterado.

**P5. Mais formatos: JSON Lines como tabela e scripts `.sql`.**
- *Problema:* log de API e exportação de pipeline vêm em `.jsonl`; hoje abre como texto, uma linha gigante por
  registro. Um `.sql` da pasta não abre na Consulta.
- *Pronto quando:* `.jsonl`/`.ndjson` abre numa grade (uma coluna por chave de primeiro nível, os objetos aninhados
  como JSON na célula), só leitura, com filtro, ordenação, busca, perfil e Salvar como. Um `.sql` aparece no
  Explorador e abre numa Consulta SQL ligada ao arquivo: o Ctrl+S grava de volta no `.sql`.

**P6. Gráficos e "Abrir no Python".**
- *Problema:* depois do perfil, o próximo passo é ver: barras por categoria, a série no tempo.
- *Pronto quando:* "Abrir no Python" gera um script com pandas e Seaborn/Matplotlib que lê o arquivo (ou o
  resultado da consulta, gravado num CSV temporário) e roda no Python da máquina, ou só copia o script se não houver
  Python. E os gráficos desenhados no próprio DataBox (barras, linhas, histograma, dispersão), a partir de uma
  tabela, de um CSV, de uma planilha ou do resultado de uma consulta, numa aba própria, com Copiar a imagem e Salvar
  como PNG. *(Decidido em 21/10/2026: os dois entram na 0.3.0.)*

**P7. Interface em inglês, pronta para outros idiomas.** *(Pedido do Isaías em 21/10/2026.)*
- *Problema:* o DataBox é de código aberto e quer ser achado por quem trabalha com dados no mundo todo, mas a
  interface inteira é em português. O README já tem versão em inglês; o programa, não. Quem não lê português baixa,
  abre e desiste.
- *Pronto quando:*
  - Preferências › Idioma: "Seguir o sistema", "Português (Brasil)" e "English"; a troca vale na hora, sem reabrir
    (como o tema). Na primeira abertura, segue o idioma do sistema; um sistema em idioma que o DataBox não tem abre
    em inglês.
  - Todo texto que a pessoa vê sai traduzido: menus, barras, dicas, avisos, toasts, janelas, Propriedades, mensagens
    de erro (as frases do `ErroBanco`), os nomes de acessibilidade (AccessKit), a ajuda da linha de comando e os
    Atalhos do teclado. Os números e as datas seguem o idioma ("1.234,56" e "21/10/2026" no português; "1,234.56" e
    "2026-10-21" no inglês).
  - O que vem do arquivo da pessoa não muda: nomes de tabela, valores, o separador decimal do CSV (que continua
    detectado pelo arquivo).
  - Um idioma novo é um arquivo de catálogo novo, sem mexer no código: uma pessoa de fora traduz para o espanhol com
    uma ferramenta comum de tradução e abre um PR só com o arquivo. O CI diz que textos faltam em cada idioma.
  - O README em inglês ganha capturas da interface em inglês.
- *Fora:* traduzir o código, os comentários, o `ARQUITETURA.md`, o `CHANGELOG.md` e as pranchetas (continuam em
  português, como manda o `CONTRIBUTING.md`); idiomas da direita para a esquerda (árabe, hebraico).

### O que fica de fora da 0.3

- **PostgreSQL e outros bancos.** É o item mais pedido de qualquer ferramenta de dados, mas a 0.3 é sobre o arquivo
  que a pessoa já tem na máquina. Fica para a 0.4, que pode ter o tema "Conectar".
- DuckDB e Parquet, `.xls`/`.xlsb`, macOS.
- Editar texto acima de 0,5 MiB e logs acima de 32 MiB.
- Uma tabela do banco como destino do Trazer colunas (o Salvar como para SQLite cobre parte do caso).

---

## 3. A resposta do TL

> *"Concordo com o tema. Três dos quatro primeiros itens se apoiam em duas peças que ainda não existem: um leitor de
> linhas comum a todas as fontes e um gravador de SQLite. Construídas primeiro, o resto encaixa. E antes de qualquer
> recurso, o CI do Windows precisa parar de mentir."*

### 3.1 Base antes dos recursos (marco 0)

*(Decidido em 21/10/2026: os itens 1 e 2 vão na 0.2.3, junto com a busca nos arquivos; ver
[`planos-0.2.3.md`](planos-0.2.3.md).)*

1. **CI confiável.** Separar os testes de tela (kittest/wgpu) dos de núcleo. No Windows, os de tela rodam sempre com
   `--test-threads=1` (o WARP não aguenta vários wgpu ao mesmo tempo, e isso já é sabido) e os de núcleo em
   paralelo; sem a repetição de tudo, que esconde falha intermitente. Um teste que falhar duas vezes por motivo de
   ambiente vira `#[ignore]` com issue, não retentativa silenciosa. Medir o tempo dos dois jobs antes e depois.
2. **Fatiar o `editor/src/app.rs`** nos pontos que a 0.3 vai tocar: a fila de perguntas ao fechar (o "Salvar tudo"
   mexe nela), o despacho dos verbos e das ações de menu (Salvar como, Consultar com SQL, Abrir no Python). Só mover,
   sem mudar comportamento, num PR que só move. O mesmo para o `editor/tests/casca.rs`, dividido por área.
3. **Não subir o egui para o 0.37 na 0.3.0.** Toda captura de tela muda, e misturar isso com recurso novo deixa a
   revisão impossível. Subir num PR só dele, logo depois da 0.3.0 (marco 0.3.1), com as capturas conferidas.

### 3.2 Viabilidade de cada item

Tamanhos: **P** cabe num PR pequeno, **M** num PR normal, **G** em dois ou mais.

| Item | Custo | Crates | Comentário |
|---|---|---|---|
| P0 Menu da seleção | M | `interface::grade` (o menu e as ações), `tabular::colunas` (os formatos de cópia, ao lado do `escrever_tsv`, com o `comum::q`), as quatro telas | A grade já sabe o intervalo (`interface::grade::Intervalo`) e já copia em TSV; falta uma peça de menu da célula na `interface` que devolve a ação escolhida à tela, e cada tela diz o que liga (como as `Ofertas` dos verbos). Filtrar usa os filtros que já existem nos Dados, no CSV e no Excel. O Colar e o Excluir as linhas reaproveitam o que o CSV já faz; nos Dados, colar várias células são várias gravações: uma transação só, com um Desfazer só (o Desfazer do banco hoje desfaz uma célula). Não depende da fundação. **Vai na 0.2.3** (decisão de 21/10/2026). |
| Base: leitor de linhas comum | M | `comum` (trait), `tabular`, `planilha`, `banco` | Um `trait FonteDeLinhas` (colunas, tipos sugeridos, iterador de `Vec<Valor>`, `parar`) implementado pelo CSV, pela planilha e pela tabela/consulta do banco. Hoje cada um tem o seu iterador; o `exportar` só recebe linhas do banco. |
| Base: gravador de SQLite | M | `exportar` (ganha o `rusqlite`, que o workspace já tem) | `Formato::Sqlite` no `Escritor`: `CREATE TABLE` com as afinidades sugeridas, `INSERT` preparado em transações de 50 mil linhas, `PRAGMA journal_mode=OFF` e `synchronous=OFF` no arquivo novo. Mesmo contrato de cancelar e de temporário que os outros formatos. |
| P1 Perfil | M + M | crate novo `perfil` (só `comum`), `tela_csv`, `tela_dados`, `tela_planilha`, `estado` | O cálculo em fluxo sobre a `FonteDeLinhas`: contagens, mín./máx., média e desvio de Welford, distintos exatos até 100 mil e estimados acima (HyperLogLog, sem dependência), os 10 mais frequentes (contagem exata até o limite de memória, depois Space-Saving), histograma em duas passadas (mín./máx., depois as faixas). No banco, o SQL (`COUNT`, `COUNT(DISTINCT)`, `GROUP BY … LIMIT 10`) é mais rápido que ler tudo, mas tem o mesmo risco de demora: rodar numa conexão de leitura com `parar`. Substitui o `tela_csv::Estatistica`, que hoje junta a coluna inteira num `Vec<String>`. O histograma pequeno é desenhado com o pintor do egui, sem biblioteca. |
| P2 Busca | M | `tabular`, `planilha`, `texto`, `tela_csv`, `tela_planilha`, `tela_texto` | O CSV e a planilha precisam de `buscar(texto, a_partir_de, opções, parar)` que percorre o índice do arquivo sem montar as páginas; o resultado é a posição (linha, coluna). O texto já tem o realce da Consulta (`AreaTexto::destacar`) para reaproveitar. Risco baixo: o contrato `Documento::busca` já existe, as três telas só não o implementam. |
| P3 Salvar como | M | crate novo `conversao` (← `tabular`, `planilha`, `texto`, `exportar`), `tela_exportar`, `editor` | Com as duas peças da base, é a janela Exportar aberta a partir de um arquivo. A detecção de tipos do CSV (`tabular::numero`) vira a sugestão de afinidade. O "Salvar tudo" é a fila de salvar que já existe, sem a pergunta. |
| P4 SQL sobre CSV | M | `servico`, `tela_consulta`, `estado` | Ver 3.3: é importar com o gravador de SQLite e consultar. O motor de SQL continua o SQLite. |
| P5a JSON Lines | M | `texto` (o analisador já existe), tela só leitura sobre a grade da Consulta | Indexar uma linha por registro, achatar o primeiro nível. Só leitura e com o mesmo leitor em fluxo, entra de graça no Salvar como e no perfil. |
| P5b `.sql` na Consulta | P–M | `comum::arquivos`, `estado` (a `Rota::Consulta` ganha o arquivo), `tela_consulta` | O nó é fácil; o difícil é a Consulta sem banco aberto (hoje ela exige um). Com 3.3, ela abre sobre a bancada. |
| P6a Abrir no Python | P | `servico::arquivos` (achar `python`/`py`, rodar), `estado` | Gerar texto e rodar um processo. Detecção do Python e do pandas com prazo curto; se faltar o pandas, o script diz como instalar. |
| P6b Gráficos no app | G | `egui_plot` 0.37 (MIT/Apache, compatível com o egui 0.36), tela nova | Dependência nova, prancheta nova (os 21 ícones de gráfico já estão no canvas) e testes de captura novos. Não cabe na 0.3.0 sem atrasar o resto. |
| P7 Inglês e idiomas | G | crate novo `idioma` (sem dependências do projeto, na base do grafo), **todos** os crates com texto, `prefs`, `interface::formato` | Medido em 21/10/2026: ~2.300 textos para a pessoa em `src/` (~1.700 diferentes), quase metade no `interface` (465) e no `editor` (396), e também nos núcleos (`esquema` 210, `banco` 128): as frases de erro nascem lá. O mecanismo e a ordem estão em 3.5. |
| PostgreSQL | G+ | `banco`, `esquema`, `servico` e telas | O contrato inteiro é do `rusqlite`: `Banco::com_conexao(&Connection)`, o `esquema` recebe `&rusqlite::Connection`, o `ddl` lê o `CREATE TABLE` do SQLite. Antes do primeiro byte de Postgres vem o contrato de fonte de dados, mais TLS, cofre de senhas e um Postgres de teste no CI (no Windows não há contêiner Linux). Concordo que é 0.4. |

### 3.3 Onde o TL contesta o PO

1. **SQL sobre CSV esbarra no "um banco de cada vez".** O `servico` é a sessão de *um* banco, e a Consulta SQL é
   dela. Três caminhos:
   - **(a) Bancada** (recomendado): um banco SQLite temporário, só do app (`pasta_de_estado/bancada.db`, apagado ao
     fechar), em que cada arquivo vira uma tabela importada pelo gravador de SQLite. Com um banco aberto, a bancada
     entra por `ATTACH` como o esquema `arquivos` (`SELECT … FROM clientes JOIN arquivos.vendas …`); sem banco, a
     Consulta abre sobre ela. Custo: a importação inicial (1 milhão de linhas em ~10 s) e o espaço em disco.
   - (b) Tabela virtual do SQLite lendo o CSV direto (`rusqlite` com `vtab`): sem cópia, mas exige `unsafe impl
     VTab`, que o projeto proíbe fora da FFI, e cada consulta relê o arquivo inteiro.
   - (c) DuckDB: o melhor motor para isso, mas é C++ compilado pela MinGW, deixa o build vários minutos mais lento e
     o executável dezenas de MB maior. Fica para a "versão opcional" do roadmap.
2. **O Salvar como vem antes do SQL sobre CSV**, não depois: o SQL sobre CSV é o Salvar como para SQLite rodando
   por baixo. Inverter P3 e P4 na ordem de implementação, não na de prioridade.
3. **Perfil "exato sempre" não escala.** `COUNT(DISTINCT)` em todas as colunas de uma tabela de 10 milhões de linhas
   leva minutos. Proposta: o perfil de uma coluna é exato e sob demanda; o "Perfil da tabela" de todas as colunas usa
   os distintos estimados e diz "≈" quando estimou.
4. **Os gráficos no app saem da 0.3.0.** O "Abrir no Python" (P6a) é barato e entrega o gráfico hoje; os gráficos
   nativos (P6b) vão para uma 0.3.x, com prancheta antes. *(Vencido em 21/10/2026: o Isaías trouxe os gráficos
   para a 0.3.0; a prancheta continua vindo antes.)*
5. **Ordem na casca:** P2, P3 e P5b mexem em menus, verbos e na fila de salvar. Por isso o fatiamento do `app.rs` vem
   no marco 0, e os PRs que tocam a casca entram um de cada vez.

### 3.4 Riscos

| Risco | Prob. | Impacto | O que fazer |
|---|---|---|---|
| CI do Windows continua intermitente e trava os PRs da 0.3 | alta | alto | marco 0 antes de tudo; o Linux (lavapipe, mais estável) é a referência das capturas |
| Perfil ou importação lentos em arquivo grande deixam a interface "presa" | média | alto | tudo em `Promessa` com `parar` e progresso; metas de tempo com teste de desempenho em release (como o "1 milhão em 7 s" do Excel) |
| Tipos errados na conversão (CEP com zero à esquerda virando número, datas pt-BR) | alta | alto | a prévia mostra os tipos e deixa trocar; por padrão, coluna com zero à esquerda ou mistura de formatos fica TEXT |
| A bancada ocupa disco e fica órfã depois de uma queda | média | baixo | um arquivo por processo, apagado ao fechar e na próxima abertura; tamanho na dica |
| Dependência nova (`egui_plot`) prende a subida do egui | média | médio | o `egui_plot` 0.37 (egui 0.36) na 0.3.0; a subida para o egui 0.37 leva junto o `egui_plot` 0.38, no mesmo PR |
| A varredura da tradução toca todo crate e briga com todo PR aberto | alta | alto | fazer a varredura no começo da 0.3, crate a crate, antes dos recursos; depois dela, texto novo já nasce com `t!` (o CI recusa texto solto) |
| Texto em inglês que não cabe (botões, colunas fixas, a busca de 250 px, a altura fixa das Preferências) | média | médio | um pseudoidioma de teste ("[Ábrír pástá·····]", 40% mais longo) com capturas de cada tela; o inglês costuma ser mais curto, o alemão de amanhã não |
| Os 1.116 `label(…)` dos testes de tela procuram o texto em português | certa | médio | os testes fixam o português (o padrão do `apoio_testes`); um teste de fumaça por tela em inglês e no pseudoidioma |
| Linux/AppImage pouco testado em áreas de trabalho de verdade | média | médio | a lista "O que testar à mão" do `RELEASE.md` ganha os itens novos (diálogo do Salvar como pelo portal, rodar o Python) |

### 3.5 A tradução (P7): mecanismo e ordem

**Mecanismo (recomendação):** um crate próprio, `idioma`, no estilo do gettext: **a chave é o próprio texto em
português**. `t!("Abrir pasta")`, `t!("Gravado em {tabela} (linha {linha}, {coluna})", tabela = …, linha = …,
coluna = …)` e `tn!(n, "{n} linha", "{n} linhas")` para o plural. O português continua legível no código e os testes
não mudam; os catálogos são arquivos `.po` (o formato que Poedit, Weblate e Crowdin já leem) em `crates/idioma/idiomas/`,
embutidos no executável (`include_str!`), lidos por um analisador próprio pequeno (o subconjunto do `.po` que o projeto
usa, com os plurais). O idioma atual é um valor global (`idioma::atual()`), trocado pela casca; os núcleos (`banco`,
`esquema`, `servico`…) traduzem as frases de erro na hora de criá-las, sem conhecer egui.

- **Por que não o Fluent** (`fluent-bundle`, do Mozilla): melhor para gênero e plurais complicados, mas cada texto vira
  um identificador (`menu-arquivo-abrir-pasta`) e as ~1.700 frases do código teriam de ser reescritas e afastadas do
  código; o `.ftl` também é menos conhecido das ferramentas de tradução. O `rust-i18n` (YAML) fica no meio, com uma
  dependência a mais e a chave livre.
- **O risco do gettext:** mudar uma frase em português muda a chave e a tradução se perde. Mitigação: o extrator
  (`ferramentas/idioma.rs`, roda no CI) lista as chaves do código, compara com cada `.po` e falha com as que faltam
  no inglês; os outros idiomas só avisam (caem no inglês e, sem ele, no português).
- **Formatos:** `interface::formato` (`formatar_numero`, `formatar_data`, `formatar_bytes`, `plural`) passa a perguntar
  ao `idioma`; o `sem_acento` das buscas não muda.
- **Idioma do sistema:** o `sys-locale` (MIT/Apache, sem `unsafe` no nosso código; decidido em 21/10/2026).
- **Regra nova no `CONTRIBUTING.md`:** todo texto para a pessoa passa pelo `t!`; o clippy não pega isso, então o
  extrator também lista os literais com espaço e acento fora do `t!` nos crates de tela e falha o CI.

**Ordem:** a infraestrutura e a varredura vêm **no começo da 0.3**, antes dos recursos: cada PR de recurso que entra
antes dela é mais texto para varrer depois, e a varredura briga com qualquer PR aberto no mesmo crate.

- **I1** `feat(idioma)`: o crate, o `t!`/`tn!`, o `.po`, o extrator e o CI; o seletor nas Preferências e o
  `prefs` com a chave `"idioma"`; os formatos no `interface::formato`.
- **I2** varredura dos núcleos: `comum`, `ddl`, `esquema`, `banco`, `exportar`, `planilha`, `tabular`, `texto`,
  `prefs`, `servico` (as frases de erro).
- **I3** varredura das telas: `interface`, `estado`, `tela_*`, `editor` (pode ser dividida em dois PRs, os `tela_*`
  num e `interface`/`estado`/`editor` noutro, em sessões paralelas: crates diferentes).
- **I4** `idioma/en.po` completo, traduzido com o glossário (`idioma/glossario.md`), revisado por um segundo agente e
  conferido pelo Isaías nas capturas; o pseudoidioma; os testes de fumaça em inglês; o README em inglês com
  capturas em inglês.

O I2 e o I3 podem correr ao mesmo tempo (crates diferentes), os dois depois do I1. A 0.2.3 sai antes e não espera
nada disso; o fatiamento do `app.rs` na 0.2.3 também ajuda o I3.

### 3.6 PRs sugeridos (na ordem)

Os PRs de base (CI, `app.rs` fatiado), a busca nos arquivos e o menu da seleção estão na 0.2.3. Os da 0.3:

1. `feat(comum, tabular, planilha, banco)`: `FonteDeLinhas`.
2. `feat(exportar)`: `Formato::Sqlite` e `Formato::JsonLinhas`.
3. `feat(perfil)`: o crate e o perfil da coluna nas Propriedades (CSV, Dados, Excel); substitui as contas do CSV.
4. `feat(conversao)`: Salvar como e Salvar tudo, e o "Exportar a seleção…" no menu da 0.2.3.
5. `feat(servico, tela_consulta)`: a bancada e o "Consultar com SQL".
6. `feat(perfil)`: o Perfil da tabela (todas as colunas) e o "Perfil da seleção" no menu da 0.2.3.
7. `design`: prancheta dos gráficos (a prévia na vista "Gráfico" das Propriedades, a aba, a escolha das colunas e do
   tipo, os quatro tipos, os dois temas).
8. `feat(graficos)`: crate `tela_grafico` com o `egui_plot` 0.37: barras, linhas, histograma e dispersão sobre a
   `FonteDeLinhas` (amostra de até 100 mil pontos na dispersão, agregação no banco ou na bancada nas barras), Copiar
   a imagem e Salvar como PNG; a prévia nas Propriedades e o "Abrir em aba".
9. `feat`: Abrir no Python (o script com pandas e Seaborn/Matplotlib, a mesma escolha de colunas do gráfico).
10. `feat(texto)`: JSON Lines como tabela; `.sql` na Consulta.

Antes deles, os PRs da tradução, I1 a I4 (seção 3.5); os de recurso já nascem com o `t!` e com a linha em inglês no
`.po`. O 1 e o 2 vêm primeiro. O 10 só depende do 1 (o JSON Lines é mais uma `FonteDeLinhas`) e pode correr em paralelo com
o 3 e o 7. Depois, o 3 e o 7 podem correr em paralelo (crates e arquivos diferentes). O 4 e o 5 dependem
do 1 e do 2; o 5 depende do 4 só pela janela de prévia dos tipos. O 8 depende do 1 e do 7; o 9 reaproveita a escolha
de colunas do 8.

---

## 4. Plano conciliado

O PO aceita a ordem do TL (fundação primeiro, Salvar como antes do SQL sobre CSV). O TL aceita o Perfil como item
número um. Os dois tiraram da 0.3 o que é refinamento ou correção (agora a 0.2.3) e, por decisão do Isaías, trouxeram
os gráficos para a 0.3.0.

**Antes: 0.2.3 — refinamentos e correções** ([`planos-0.2.3.md`](planos-0.2.3.md)): o CI confiável, o `app.rs`
fatiado, o menu da seleção, o Ctrl+F nos arquivos e as contas do CSV em fluxo.

**Depois: 0.2.4 — acabamento da interface** (decidido em 22/10/2026, revisão de UI em
[`revisao-ui-0.3.md`](revisao-ui-0.3.md)): contagem num lugar só, faixa do topo com uma regra só, vazio com as colunas
à vista, caminhos cortados, seta da ordenação, Visão geral, rodapé da planilha e Consulta vazia, um PR por assunto,
**antes do M0** (a varredura da tradução mexe em todo texto; estes PRs mexem em texto e vêm antes).

**Versão 0.3.0 — "Entender, levar e ver os dados".**

| Marco | Conteúdo | PRs |
|---|---|---|
| **M0 · Idioma** | O crate `idioma`, a varredura de todos os crates, o inglês completo e revisado, o seletor nas Preferências; **os números da grade dos Dados e da Consulta no formato do idioma**, com "Como gravados" nas Preferências (U1 da revisão de UI de 22/10/2026) | I1 a I4 |
| **M1 · Fundação** | `FonteDeLinhas`; gravador de SQLite e de JSON Lines; **rolagem contínua nos Dados** (sem páginas, como o CSV; U2 da revisão de UI de 22/10/2026) | 1, 2 |
| **M2 · Entender** | Perfil da coluna, da tabela e da seleção | 3, 6 |
| **M3 · Levar** | Salvar como (CSV, Excel, JSON, JSON Lines, SQLite) e Salvar tudo; SQL sobre CSV e planilha pela bancada | 4, 5 |
| **M4 · Ver** | Gráficos no app (barras, linhas, histograma, dispersão); Abrir no Python | 7, 8, 9 |
| **M5 · Formatos** | JSON Lines como tabela; `.sql` na Consulta | 10 |
| **0.3.0** | M0 a M5, o CHANGELOG, o README (Roadmap e guias rápidos novos: "Converter uma planilha em banco", "Perguntar com SQL a um CSV", "Um gráfico de uma consulta") e capturas novas | |

**Depois da 0.3.0:**

- **0.3.1:** subir para o egui 0.37 e o `egui_plot` 0.38 (PR só disso).
- **0.4 — "Conectar":** abre pelo **SSH** (ver arquivos e bancos de outras máquinas; decidido em 21/10/2026, plano em
  [`planos-ssh.md`](planos-ssh.md), com o spike da biblioteca no fim da 0.3); depois, o contrato de fonte de dados
  (um documento de decisão escrito ainda durante a 0.3, sem código), PostgreSQL só leitura por padrão, com o túnel
  SSH, senhas no cofre do sistema.

**Fora da 0.3:** PostgreSQL e outros bancos, DuckDB e Parquet, `.xls`/`.xlsb`, outros idiomas além do inglês (a
porta fica aberta), macOS, editar texto acima de
0,5 MiB, logs acima de 32 MiB, Trazer colunas para uma tabela do banco.

---

## 5. Decisões para o Isaías (0.3)

Decididas em 21/10/2026, além destas: a 0.2.3 de refinamentos e correções antes da 0.3; os gráficos no app na
0.3.0.

1. ~~Corte da 0.3.0~~ **Decidido em 21/10/2026:** perfil, Salvar como, SQL sobre CSV, gráficos com Abrir no Python,
   JSON Lines e `.sql` na Consulta, tudo na 0.3.0; a 0.3.1 fica só com a subida do egui.
2. ~~SQL sobre CSV~~ **Decidido em 21/10/2026:** a bancada com `ATTACH`.
3. ~~Onde mora o perfil~~ **Decidido em 21/10/2026:** a vista "Perfil" nas Propriedades (Alt+5) para a coluna e uma
   aba de documento para o Perfil da tabela.
4. ~~Perfil exato ou estimado~~ **Decidido em 21/10/2026:** exato por coluna; estimado ("≈") no Perfil da tabela
   acima de 100 mil valores.
5. ~~CSV para SQLite~~ **Decidido em 21/10/2026:** banco novo ou tabela nova no banco aberto, esta com a prévia do
   SQL antes, como as outras mudanças de estrutura, e desligada no só leitura.
6. ~~Gráficos~~ **Decidido em 21/10/2026:** com o `egui_plot` 0.37 (dependência nova aprovada; sobe para a 0.38
   junto com o egui 0.37).
7. ~~Onde mora o gráfico~~ **Decidido em 21/10/2026:** os dois: uma prévia pequena numa vista "Gráfico" das
   Propriedades (que pode ser destacada em janela) e "Abrir em aba" para o gráfico grande, numa aba de documento com
   endereço, Voltar e Avançar.
8. ~~egui 0.37~~ **Decidido em 21/10/2026:** na 0.3.1, num PR só dele, com o `egui_plot` 0.38.
9. ~~PostgreSQL na 0.4~~ **Decidido em 21/10/2026:** na 0.4, com o documento do contrato de fonte de dados escrito
   ainda durante a 0.3.

**Da tradução (P7, pedida em 21/10/2026):**

10. ~~Mecanismo~~ **Decidido em 21/10/2026:** o crate próprio `idioma`, no estilo gettext, com o texto em português
    como chave e catálogos `.po`.
11. ~~Quando~~ **Decidido em 21/10/2026:** a infraestrutura e a varredura no começo da 0.3, antes dos recursos.
12. ~~Idioma na primeira abertura~~ **Decidido em 21/10/2026:** o do sistema; o inglês quando o DataBox não tem o do
    sistema.
13. ~~Números e datas~~ **Decidido em 21/10/2026:** seguem o idioma da interface.
14. ~~Revisão do inglês~~ **Decidido em 21/10/2026:** um agente traduz com um glossário fixo (`idioma/glossario.md`:
    table, query, row, column, filter… no estilo de DBeaver e VS Code), outro agente revisa, e o Isaías confere as
    telas principais pelas capturas em inglês.
15. ~~Idioma do sistema~~ **Decidido em 21/10/2026:** a dependência `sys-locale` (aprovada).

Todas as decisões da 0.3, inclusive as da tradução, estão tomadas.
