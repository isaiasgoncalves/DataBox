# Revisão de UI para a 0.3 — debate do designer com o PO

Escrita em 22/10/2026, logo depois da 0.2.3. Dois papéis olham a interface inteira e discutem o que melhorar antes e
durante a 0.3: o **designer** (UI/UX: fluxo, mecânica de interação, consistência e acabamento) e o **PO** (fala pelos
cientistas de dados, engenheiros de dados e desenvolvedores que abrem um arquivo de dados para entender e levar). No
fim, as decisões que ficam com o Isaías. **Nada aqui está implementado nem decidido.**

As propostas desenhadas estão na página **"Propostas 0.3"** do [canvas do DataBox](https://claude.ai/artifact/TCdgEPdRzEzWEmC6XTJVu9).
Cada achado tem um número (U1, U2…) que aparece nas pranchetas.

## Cobertura

- Capturas dos testes de tela da 0.2.3 (`cargo test -p editor --test casca -- cena_`, `pasta`, `capturas_editores`,
  `planilha`, `inicio`, `tela_csv`, `tela_consulta`), nos dois temas, em 1440 × 900, 1280 × 800 e 960 × 600, e as
  imagens do README (`docs/imagens/`).
- Telas: Início, Visão geral, Dados (com filtro, vazia, menu da célula), CSV, Excel, JSON, Consulta (com erro e vazia),
  menus Arquivo e da célula, Propriedades nas docas.
- **Fora:** o app aberto à mão (sem computer use nesta sessão: a mecânica de arrastar, o tempo das dicas e o foco do
  teclado foram vistos só nas capturas e no CHANGELOG), Windows de verdade, leitor de tela.

## O que está bem e não se mexe

Os dois concordam:

- **A barra de ferramentas universal**, com cada verbo no mesmo lugar em toda aba. No JSON, 11 dos 17 botões ficam
  desligados, e o PO perguntou se não seria melhor esconder. O designer defende o lugar fixo (memória espacial, lei de
  Jakob: é o que VS Code e Office fazem), e o PO aceitou: o desligado com a dica do motivo ensina o que existe.
- **A busca que diz onde busca** ("Buscar em clientes", "Buscar no editor", "Buscar tabelas") e a contagem "3 de 41".
- **O menu da célula** da 0.2.3: o item que não vale fica desligado com o motivo na dica, em vez de sumir.
- **O erro da Consulta**: diz o comando, o que já rodou e marca cada comando numa pílula. É a melhor tela do app.
- **A soma da seleção no rodapé**, que encolhe por prioridade quando não cabe.
- **Os estados vazios do Explorador e da Consulta**: dizem o que fazer e têm o botão (Abrir pasta) ou as sugestões.
- **O seletor Resumo · Colunas · Índices · SQL** que vira ícones na doca estreita, em vez de cortar com "…".

## Achados e debate

Em ordem de prioridade conciliada. Severidade na escala do designer (crítica, alta, média, baixa).

### U1 · Números em dois formatos na mesma tela — alta

**Hoje.** A grade dos Dados mostra o REAL como o SQLite devolve ("14542.56", ponto, sem milhar), e o rodapé da
seleção, logo embaixo, mostra a soma formatada ("Soma 49.656,08"). A Visão geral também formata ("5.000"). O CSV
mostra o texto do arquivo ("1056,30"), e o Excel, o formato da célula ("R$ 10.119,37").

**Designer.** É o achado que mais pesa: dois formatos para a mesma grandeza na mesma tela fazem a pessoa duvidar dos
dois (consistência; correspondência com o mundo real). Proposta: a grade formata INTEGER e REAL no idioma da
interface, com milhar, algarismos tabulares e alinhados à direita, e **sem arredondar** (todas as casas gravadas
aparecem). O valor cru volta na edição, na ficha da linha e no Copiar.

**PO.** Concorda com a direção, com duas condições. Primeiro, quem trabalha com dados precisa ver a precisão: nada de
"14.542,6" quando o banco tem "14542.56". Segundo, o CSV não muda: lá o dado **é** o texto, e o separador decimal é do
arquivo (já decidido na P7). E quer a saída para quem prefere o cru, porque vai colar o número num script Python.

**Conciliado.** Dados e resultado da Consulta formatam; CSV e Excel ficam como estão; o Copiar sempre leva o valor
cru; Preferências › Aparência ganha "Números na grade: Formatados (padrão) · Como gravados". Vai junto com a P7, que
já faz `interface::formato` seguir o idioma (decisão 13 da 0.3). Esforço: P.

### U2 · Os Dados paginam, as outras grades rolam — alta

**Hoje.** Os Dados têm "100 por página" e "Página 1 de 1" no rodapé. O CSV, o Excel e a Consulta rolam direto, com
"Linhas 1–20 de 1.204". Selecionar um intervalo, somar a seleção e copiar param na borda da página nos Dados.

**PO.** É atrito de verdade para o público: "selecionar a coluna inteira e copiar" ou "somar estas 300 linhas" não
funciona numa tabela de 5.000 linhas, e funciona num CSV de 1 milhão. A 0.3 traz o perfil e o Salvar como, que
pensam na tabela inteira; a página vira um conceito a mais que só os Dados têm.

**Designer.** Uma grade, um modelo (consistência; reconhecer em vez de lembrar). Rolagem contínua nos Dados, como no
CSV: a contagem "Linhas a–b de n" no rodapé, Ctrl+Home e Ctrl+End até as bordas, o seletor de página sai. A rolagem
mostra a linha que está sendo lida como no CSV grande, sem travar.

**Custo (do TL, a confirmar).** M: os Dados leem por `LIMIT/OFFSET` por página; a rolagem contínua precisa de uma
janela de linhas com cache, como o `tabular` faz no CSV, e de cuidado com tabelas sem rowid. **Para o Isaías:** na
0.3 (no marco M1, antes do perfil) ou depois.

### U3 · A contagem aparece duas vezes — média

**Hoje.** Com filtro, os Dados mostram "55 de 60 linhas" na ponta da barra de filtros e "Linhas 1–55 de 55" no
rodapé; o CSV, "236 de 1.180 linhas" em cima e "Linhas 1–24 de 236" embaixo. Com a tabela vazia, "0 linhas" em cima e
"Nenhuma linha" embaixo.

**Designer.** Uma informação, um lugar (minimalismo). Fica só no rodapé, que é onde as quatro grades contam:
"Linhas 1–24 de 236 · filtradas de 1.180". A ponta direita da barra de filtros fica com o "Limpar filtros" e, durante
a busca, com o "3 de 41" (como o CSV já faz).

**PO.** De acordo. Pede só que o "filtradas de" fique no rodapé mesmo com a grade rolada, porque é ele que avisa que
há filtro quando a barra está fora de vista. Esforço: P.

### U4 · A faixa do topo segue regras diferentes em cada documento — média

**Hoje.** A 0.2.3 tirou a faixa do CSV e do Excel porque ela repetia a aba. Mas o JSON ainda abre com "config.json"
na faixa (o nome já está na aba e nas Propriedades); a Consulta tem "Linha 3, coluna 37" **em cima**, e o JSON tem a
mesma informação **embaixo**; a Visão geral repete "exemplo.db" e o caminho inteiro, cortado no meio.

**Designer.** Proposta de regra para o guia: **a faixa de cima só existe para controle ou contexto que a aba não
diz** (filtros, a migalha do JSON, as fontes da consulta); **a posição do cursor e as contagens moram no rodapé**,
sempre à direita. Pela regra:
- **JSON:** a faixa vira a migalha do ponto onde o cursor está (raiz › conexoes › [0] › nome), com o sinal de válido
  e o Formatar na ponta. O TXT fica sem faixa, como o CSV.
- **Consulta:** "Linha, coluna" desce para o rodapé, à direita. A faixa vira a das **fontes**: o banco e, com a
  bancada da P4, os arquivos ligados ("exemplo.db + vendas_2025.csv, clientes.xlsx · + Arquivo"). É a peça que a P4
  precisa e que ainda não tem prancheta.
- **Visão geral:** a faixa sai; o caminho já está em Propriedades › Arquivo.

**PO.** Concorda com o JSON e a Consulta (a faixa das fontes é exatamente o que a P4 precisa mostrar). Discorda da
Visão geral: com dois bancos de nomes iguais em pastas diferentes, o caminho à vista evita editar o errado. O
designer aceitou trocar por algo que vale para todos: **a dica de toda aba de documento mostra o caminho inteiro**,
e a Visão geral fica sem faixa. Esforço: P (JSON, Visão geral, dicas), e o da Consulta vai no PR da P4.

### U5 · A tabela vazia esconde as colunas — média

**Hoje.** Uma tabela sem linhas mostra só "Nenhuma linha. Use Nova linha, na barra de ferramentas, para incluir a
primeira." no lugar da grade: o cabeçalho some, e a pessoa não vê quais colunas tem. O rodapé ainda tem o seletor de
página.

**Designer.** O cabeçalho fica, com nomes e pastilhas de tipo; o vazio vai no corpo, centrado, com a ação em botão
("Nova linha", o principal), em vez de mandar procurar na barra. Com filtro que não acha nada: "Nenhuma linha com
estes filtros" e o botão "Limpar filtros". Vale para os Dados, o CSV e o resultado vazio da Consulta.

**PO.** Ótimo, e levanta uma **lacuna**: quem cria uma tabela quer colar as linhas que copiou do Excel, e o Colar da
0.2.3 não cria linhas. O designer não desenha um "Colar" que não funcionaria; fica no backlog como história
("Como analista, quero colar linhas do Excel numa tabela para preenchê-la sem digitar"). Esforço: P.

### U6 · Caminhos e nomes cortados no lugar errado — baixa (ganho rápido)

**Hoje.** Nas Propriedades: "vendas_2025.c…" (corta a extensão), "/h…s/config.json", "/…1/exemplo.db". Nos
Abertos recentemente, a pasta corta no fim e esconde justamente a pasta-mãe.

**Designer.** O corte de caminho guarda sempre o nome inteiro e a pasta-mãe, e corta do começo:
"…/dados/vendas_2025.csv". Só se o nome sozinho não couber ele corta no meio do nome, guardando a extensão
("vendas_20…2025.csv"). O caminho inteiro fica na dica. Uma função só na `interface`. **PO:** de acordo. Esforço: P.

### U7 · A seta de ordenação longe do nome — baixa

**Hoje.** Na coluna numérica (alinhada à direita), a seta fica na ponta esquerda do cabeçalho, longe do nome.
**Designer:** a seta vai logo depois do nome, dos dois lados (proximidade). **PO:** sem objeção. Esforço: P.

### U8 · A Visão geral tem dois cliques por linha — baixa

**Hoje.** Cada linha tem o nome em azul (abre a tabela) e "Colunas" em azul na ponta (abre as Propriedades); a coluna
Tipo fica quase sempre vazia. **Designer:** a linha inteira abre a tabela (clique duplo e Enter, como o Explorador);
o "Colunas" vira um botão de ícone que aparece no hover e no foco; o tipo especial (VIRTUAL, SEM ROWID) vira etiqueta
ao lado do nome; a coluna Tipo sai e entra a de Índices. **PO:** quer manter um jeito de ir às colunas sem mouse, e o
foco no botão de ícone resolve. Esforço: P.

### U9 · Rodapé da planilha denso — baixa

**Hoje.** "Vendas › E6 · Somente leitura · 1.204 linhas · 6 colunas · A1:F1204 · 1 mesclada", logo abaixo da faixa
das planilhas. **Designer:** o intervalo e as mescladas já estão nas Propriedades; o rodapé fica com "Vendas › E6 ·
Somente leitura · 1.204 linhas · 6 colunas". **PO:** de acordo. Esforço: P.

### U10 · A Consulta vazia usa um terço da tela — baixa

**Hoje.** Antes da primeira execução, o editor ocupa a altura que teria com resultado, e o resto do documento fica
vazio. **Designer:** sem resultado, o editor ocupa a altura toda; o primeiro Ctrl+Enter abre a área do resultado.
**PO:** baixa prioridade, mas é a primeira coisa que quem abre uma Consulta vê. Esforço: P.

### U11 · Regras a registrar no guia (custo zero)

- **Ícone em item de menu só quando o mesmo verbo tem botão na barra de ferramentas** (Filtrar, Excluir, Abrir pasta,
  Salvar…): o menu ensina a barra. É quase o que já acontece no menu da célula e no Arquivo; falta escrever.
- **Contagens e posição no rodapé, à direita; controles e contexto na faixa de cima** (U4).
- **Toda aba de documento tem o caminho inteiro na dica** (U4).
- **Para a P7:** nenhum botão, etiqueta ou coluna com largura fixa pelo texto em português (os três botões grandes do
  Início, a busca de 250 px, a altura fixa das Preferências); o pseudoidioma da P7 confere isso nas capturas.

## O que a 0.3 traz que ainda não tem prancheta

O PO lembrou que as pranchetas do perfil (`FuturoPerfil`) e do gráfico (`FuturoGrafico`) já existem, mas não há
desenho para três superfícies decididas em 21/10/2026:

- **As fontes da Consulta com a bancada (P4)**: a faixa da U4, com "+ Arquivo" e o arquivo ligado como esquema
  `arquivos`. Desenhada na prancheta "Faixas e rodapés".
- **O Perfil da tabela (P1)**, decidido como aba de documento: uma linha por coluna (tipo, preenchidas, nulos,
  distintos, mín., máx., média, mais frequente e um histograma pequeno), com o "≈" acima de 100 mil valores.
  Desenhado na prancheta "Perfil da tabela".
- **Preferências › Idioma (P7)**: "Seguir o sistema · Português (Brasil) · English", ao lado do Tema. Desenhado na
  prancheta "Detalhes".

## Decisões para o Isaías

1. **U1 Números:** formatar INTEGER e REAL nos Dados e na Consulta (sem arredondar, cru no Copiar), com a opção
   "Como gravados" nas Preferências? Ou manter cru em todo lugar e deixar só o rodapé formatado?
2. **U2 Rolagem contínua nos Dados:** na 0.3 (M1, antes do perfil) ou depois da 0.3?
3. **Onde entram os ganhos rápidos** (U3, U5, U6, U7, U8, U9, U10, e a parte da U4 sem a Consulta): numa **0.2.4 de
   acabamento** antes da varredura da tradução (menos texto mexido depois do `t!`), ou dentro da 0.3?
4. **U4 Visão geral:** sem faixa e com o caminho na dica de toda aba (conciliado), ou manter a faixa com o caminho?
5. **U11:** registrar as quatro regras no guia (`design/README.md`)?
