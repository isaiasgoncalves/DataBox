# Revisão crítica do PO e do designer — 26/10/2026

Revisão independente, pedida pelo Isaías antes de a 0.3 pegar velocidade. Duas vozes: o **PO** (fala pelos cientistas de
dados, engenheiros de dados e desenvolvedores) e o **designer** (estética, fluxos, movimento e microdetalhes). Olha a
0.2.3 e a 0.2.4 em uso de verdade, os PRs abertos da onda 0 (#36, #37, #39) e os planos da 0.3. **Nada aqui muda código
nem prancheta**; os achados têm número (PO1…, D1…) para os PRs citarem.

Gravidade: **bloqueia** (impede a tarefa ou faz a pessoa ler um dado errado), **atrapalha** (funciona, mas cansa,
confunde ou obriga a contornar), **polimento** (acabamento). Esforço: **P** (um PR pequeno), **M** (um PR normal),
**G** (dois ou mais).

## (a) Resumo

1. A main na 0.2.4 é sólida no caminho feliz: o Ctrl+F acha "P099999" num CSV de 100 mil linhas na hora, o "Filtrar por este valor" vira etiqueta e o rodapé diz "filtradas de 100.000".
2. **Mais grave (PO1, bloqueia):** um `.xlsx` sem largura de coluna declarada (o que o `pandas.to_excel` grava) abre com todas as colunas em 8,43 caracteres e os números cortados com "…" ("47232…"); quem lê, lê errado.
3. **PO2, bloqueia:** no CSV, depois de qualquer filtro as colunas encolhem até a largura do cabeçalho e cortam números e datas ("44331,…", "04/01/…"). A causa está em `tela_csv/src/faixa.rs:110` (mede antes de a página chegar). Os Dados não têm o defeito.
4. **PO3, bloqueia a coerência da U1:** o mesmo CSV mostra o número em **três formatos** numa tela só: "589.0893" na grade, "Soma 1,514,780.7247" (formato americano) nas Propriedades e "Soma 4.890,9497" no rodapé. A U1 decidiu que o CSV "não muda", e é justamente nele que o problema ficou.
5. **D1, atrapalha muito:** com 7 abas a 1280 px, as abas viram "en…", "exp…", "Co…"; até "app.log" vira "app.l…". A regra de corte da U6 não chegou às abas.
6. **PO8, atrapalha:** a U1 não define o que acontece com **ids, anos e CEPs inteiros** ("2.025", "12.345") nem com as médias de 16 algarismos ("10.857,058571428572"). Precisa estar decidido antes da I1, porque a regra vai para o `interface::formato`.
7. PRs abertos: o #36 (U11) segue a regra ao pé da letra, mas o "Excluir a tabela" com a lixeira ensina o verbo errado da barra (D5). O #37 (Q1) insere SQL no clique simples, e o Ctrl+Z não desfaz essa inserção (PO7). O #39 tem pranchetas fortes (a chave mudada sem botão de aceitar é exemplar), com quatro acertos a fazer antes do merge (D7, D8, D9, PO11).
8. Planos: o escopo está bom. Faltam três coisas que o usuário vai sentir: **filtrar linhas num log** (PO5), o `.jsonl` e o `.sql` **invisíveis no Explorador** até o PR 10 (PO5), e **"colar criando linhas"** (PO12).
9. A tradução (P7) está bem definida no mecanismo. Falta o glossário dos termos sem tradução óbvia (Trazer colunas, Ficha da linha, Bancada, Gravação imediata) antes da varredura, e a ajuda da linha de comando no Linux ainda diz "DataBox.exe" (PO9).
10. Recomendação: um mini-lote de 6 PRs pequenos antes da varredura da tradução (seção e), todos de esforço P menos o das abas (M), e os ajustes dos #36, #37 e #39 dentro deles mesmos.

## Cobertura

- **App de verdade:** `cargo build --release -p editor` da main (`a09c9f3`, 0.2.4), rodado num Xvfb 1440 × 900 (janela 1280 × 800, tema
  escuro), dirigido por XTEST (clique, arrasto, teclado), com `XDG_*` numa pasta temporária. Arquivos gerados para a
  revisão: `vendas_2025.csv` (100 mil linhas, `;`, vírgula decimal, CEP com zero à esquerda, datas pt-BR, vazios),
  `exportacao_bi.csv` (vírgula, ponto decimal, ISO 8601, linha vazia no fim, como sai de um pipeline), `app.log`
  (20 mil linhas, 1,4 MB), `eventos.jsonl`, `config.json`, `relatorio_q3.xlsx` (1.205 linhas, sem `<cols>`, como o
  `openpyxl` grava) e o `exemplo.db` do `amostras`.
- **Jornadas:** o cientista de dados limpa e entende o `vendas_2025.csv`; o engenheiro confere a `exportacao_bi.csv` e
  a planilha; o desenvolvedor lê o log e o JSON e consulta o banco (Visão geral, Dados, seleção, menu da célula,
  filtro, Consulta).
- **PRs:** `gh pr diff` do #36 e do #37; as pranchetas do #39 desenhadas no Chrome sem janela, com o `{{classes}}`
  trocado, nos dois temas.
- **Fora:** o tema claro no app (só nas pranchetas), Windows, leitor de tela, a área de transferência (o Copiar como
  não foi lido de volta), o arrastar das abas e das docas, e os testes de tela do #37 (não rodei; o PR diz que passam).
- **Capturas:** em [`capturas-revisao-26-10/`](capturas-revisao-26-10/), numeradas como citadas abaixo.

---

## (b) A voz do PO

> *"A pessoa abre o arquivo para saber o que tem nele. Tudo o que faz ela ler um número errado, ou duvidar do número
> certo, vale mais do que qualquer recurso novo."*

### Bloqueia

**PO1 · O Excel corta os números na largura padrão — bloqueia · P**
- **Onde:** `tela_planilha` (`grade.rs:65`: a largura vem do arquivo ou 8,43 caracteres). [03](capturas-revisao-26-10/03-excel-colunas-na-largura-padrao.png)
- **O que acontece:** abra um `.xlsx` gravado pelo `pandas.to_excel`/`openpyxl` (sem `<cols>`). Todas as colunas
  ficam com ~64 px: "São P…", "2025-…", "47232…", "8969,…", com meia grade vazia à direita.
- **Por que importa:** o Excel nunca corta dígito: mostra `####` ou alarga. Aqui "47232…" se lê como outro número. O
  engenheiro que confere uma planilha gerada por um job precisa arrastar cada borda.
- **Proposta:** sem largura declarada (nem `defaultColWidth`), medir pelo conteúdo como o CSV (`medir_colunas`, até
  320 px). Em qualquer caso, **número não corta com "…"**: mostra `####` com o valor na dica, como o Excel.

**PO2 · O CSV filtrado encolhe as colunas e corta números e datas — bloqueia · P**
- **Onde:** `tela_csv`, `faixa.rs:104-115` (`trocar_vista` zera `self.pagina` e chama `medir_de_novo`; o
  `ajustar_larguras` do quadro seguinte mede sem página, só pelo cabeçalho, e ninguém mede de novo quando a página
  chega). [01](capturas-revisao-26-10/01-csv-filtro-encolhe-colunas.png) (em cima, antes; embaixo, depois)
- **Passos:** abra o `vendas_2025.csv` › clique direito numa célula de "produto" › Filtrar por este valor. As colunas
  viram "Cliente 2…", "1423…", "04/01/…", "44331,…", e Ctrl+Home não resolve. Pelo código, o Limpar filtros passa
  pelo mesmo `trocar_vista` (não conferido na tela).
- **Por que importa:** filtrar é o gesto principal do cientista de dados que limpa um CSV, e o resultado parece ter
  dados truncados.
- **Proposta:** medir quando a página da vista nova chega (o `medir` fica pendente enquanto `pagina` é `None`), com
  um teste de tela que filtra e confere a largura de uma coluna de números.

**PO3 · O mesmo CSV mostra o número em três formatos — bloqueia a coerência da U1 · P (regra) + P (código)**
- **Onde:** `tela_csv` (grade, Propriedades › Coluna, rodapé da seleção). [02](capturas-revisao-26-10/02-csv-tres-formatos-de-numero.png), [13](capturas-revisao-26-10/13-csv-coluna-maximo-e-soma.png)
- **O que acontece:** na `exportacao_bi.csv` (ponto decimal), a grade mostra "589.0893" (o texto, como decidido),
  Propriedades › Coluna dá "Mínimo 0.1768 · Soma **1,514,780.7247**" (milhar americano numa interface pt-BR) e o rodapé
  da seleção dá "Soma **4.890,9497**". Na coluna "#", "2.000"; ao lado, na coluna id, "2000". No `vendas_2025.csv`
  (vírgula), "Máximo 107979,72" sem milhar e "Soma 2.929.066.524,4" com milhar, três linhas abaixo.
- **Por que importa:** é o U1 que a 0.2.4 queria resolver, agora dentro do CSV, e piora no inglês: o CSV brasileiro
  vai mostrar "1056,30" na grade e "Sum 1,056.30" no rodapé. Com o SQL sobre CSV (P4), o mesmo dado aparece cru no CSV
  e formatado no resultado da Consulta.
- **Proposta (regra para o guia, antes da I1):** **o texto do arquivo só na grade e na edição; tudo o que o DataBox
  calcula ou conta (soma, média, mínimo, máximo, contagens, a coluna "#") sai no formato do idioma da interface**,
  também no CSV e no Excel. O decimal do arquivo só vale para *ler* o número. Mínimo e máximo passam a ser
  calculados (hoje são o texto da célula).

### Atrapalha

**PO4 · A linha vazia no fim do CSV vira um registro — atrapalha · P**
- **Onde:** `tabular`. [02](capturas-revisao-26-10/02-csv-tres-formatos-de-numero.png) (a linha 2.001 vazia)
- **O que acontece:** a `exportacao_bi.csv` termina com `\r\n\r\n` (o `csv.writer` do Python com `writerow([])`, e
  muitos geradores). O DataBox conta "2.001" linhas e mostra a 2.001 vazia; o `pandas.read_csv` e o `wc -l` menos o
  cabeçalho dão 2.000. A coluna revenue_usd ganha "Vazias 1".
- **Por que importa:** o engenheiro abre o arquivo para conferir a contagem da exportação. Um número que não bate com
  o pipeline faz ele desconfiar do arquivo, não do DataBox.
- **Proposta:** as linhas vazias **no fim** não são registros (o `skip_blank_lines` do pandas); a contagem diz
  "2.000" e a dica do rodapé, "e 1 linha vazia no fim". O Salvar grava os bytes de volta como estavam.

**PO5 · O desenvolvedor não tem como ver só as linhas de ERROR, e o `.jsonl` some do Explorador — atrapalha · P + M**
- **Onde:** `tela_texto`, `comum::arquivos` (`Texto => ["txt", "md", "log"]`). [05](capturas-revisao-26-10/05-log-faixa-e-busca.png)
- **O que acontece:** no `app.log`, o Ctrl+F acha "2 de 1.816" e vai de um em um; o Filtrar da barra fica desligado.
  O `eventos.jsonl` e um `.sql` da pasta **não aparecem** na árvore (só entram na conta da dica da pasta) até o PR 10.
- **Por que importa:** a pergunta do desenvolvedor diante de um log é "o que deu erro esta noite?". 1.816 Enters não
  respondem. E o arquivo `.jsonl` que o pipeline gravou "não existe" para o DataBox.
- **Proposta:** (a) **já**, antes da varredura: listar `.jsonl`/`.ndjson` e `.sql` como texto (P); o PR 10 depois os
  promove a tabela e a Consulta. (b) **Na 0.3, trilha D:** "Filtrar linhas" no texto (o Filtrar da barra liga, com
  "contém", "não contém" e "expressão regular"), também no leitor só leitura, com "1.816 de 20.000 linhas" no rodapé (M).

**PO6 · O limite de edição do texto é 0,5 MiB — atrapalha · P (decisão)**
- **Onde:** `texto::limites::LIMITE_EDICAO` (524.288). [05](capturas-revisao-26-10/05-log-faixa-e-busca.png)
- **O que acontece:** o `app.log` de 1,4 MB abre só para leitura. Um `package-lock.json` ou um GeoJSON de 1 a 2 MB
  também.
- **Por que importa:** para log, só leitura está certo. Para JSON de configuração, o desenvolvedor espera editar.
- **Proposta:** medir o editor com 2 e 4 MiB em release; se o egui aguentar, subir o limite do JSON. Senão, deixar
  registrado no guia por que é 0,5.

**PO7 · No #37 (Q1), um clique na árvore escreve no SQL e o Ctrl+Z não volta — atrapalha · P**
- **Onde:** PR #37, `tela_consulta/src/tabelas.rs` (o `r.resposta.clicked()` insere), e o "Limites conhecidos" do
  `lib.rs` ("O desfazer não alcança o que as Tabelas inserem").
- **O que acontece:** clicar no nome de uma tabela ou de uma coluna, para olhar ou abrir, já insere o nome no cursor
  do editor; desfazer é apagar à mão.
- **Por que importa:** quem escreve SQL clica na árvore o tempo todo para explorar. Um texto que entra sem querer,
  no meio de um `WHERE`, quebra a consulta, e a pessoa só percebe ao executar.
- **Proposta (PO):** manter o clique que insere (é rápido e o plano pede), mas **a inserção entra no desfazer do
  editor** (Ctrl+Z tira o que entrou). Ver a divergência 2, com a proposta do designer.

**PO8 · A U1 não diz o que fazer com ids, anos e médias longas — atrapalha · P (decisão antes da I1)**
- **Onde:** `design/README.md` (U1), `planos-0.3.md` (decisão 13). [09](capturas-revisao-26-10/09-consulta-media-sem-alinhamento.png), [02](capturas-revisao-26-10/02-csv-tres-formatos-de-numero.png)
- **O que acontece hoje e o que a U1 faria:** a Consulta `avg(limite)` devolve "10857.058571428572"; com a U1, "sem
  arredondar", vira "10.857,058571428572". Um INTEGER `ano` = 2025 vira "2.025"; um `id` 12345, "12.345"; um `cep`
  guardado como número, "1.310.100"; uma soma REAL dá "0,30000000000000004". A coluna "#" do CSV já faz isso ("2.000" ao
  lado do id "2000").
- **Por que importa:** quem trabalha com dados lê um separador de milhar num id ou num ano como dado errado, e copia
  o número da tela para o código.
- **Proposta:** (1) **sem milhar** em chave primária, chave estrangeira e na coluna "#" das grades; (2) REAL na
  representação mais curta que volta ao mesmo número (o que o Rust já faz; documentar que "sem arredondar" é isso);
  (3) os resultados calculados da Consulta (`avg`, divisões) também; (4) NaN, ±Infinito e notação científica
  (`1e-7`) escritos por extenso; (5) INTEGER acima de 2^53 sem passar por `f64`. Ver a divergência 3.

**PO9 · A tradução: falta o glossário dos termos difíceis e a ajuda do Linux ainda fala de Windows — atrapalha · P**
- **Onde:** `planos-0.3.md` §3.5 e decisão 14; `DataBox --help` no Linux.
- **O que acontece:** `--help` mostra "Uso: DataBox.exe" e "--criar-atalho: cria o atalho … no Menu Iniciar" no
  Linux; as opções são `--ajuda` e `--versao` (o `--help` funciona, mas a ajuda não o lista).
- **Por que importa:** o inglês é para quem não lê português; é ele quem vai digitar `--help` e `--version`. E a
  qualidade da I4 depende de decisões que um agente tradutor não deve tomar sozinho.
- **Proposta:** antes da I3, o `idioma/glossario.md` já com os termos sem tradução óbvia e a escolha do Isaías: Consulta
  SQL (Query? SQL query?), Propriedades (Properties? Inspector?), Visão geral, Trazer colunas, Ficha da linha, Bancada,
  Gravação imediata, Marcar as linhas, Somente leitura, "lista para IN (…)". Na ajuda: `-h, --help` e `--version` como
  nomes principais (`--ajuda` e `--versao` como sinônimos), o nome do executável real, e `--criar-atalho` só no
  Windows.

**PO10 · O arquivo remoto do S1 é só leitura, e o plano não diz como a pessoa sai com ele — atrapalha · P**
- **Onde:** `planos-ssh.md` (S1), prancheta `FuturoSsh` do #39 ("Salvar e a edição desligam com o motivo na dica").
- **O que acontece no plano:** o cientista de dados abre o `saida.csv` do servidor, quer limpar duas colunas e não
  pode. O Salvar como (PR 4) seria a saída natural (uma cópia local editável), mas nenhum dos dois planos o cita.
- **Proposta:** escrever no PR 14 que o Salvar como e o "Consultar com SQL" ficam ligados no remoto, e que a dica do
  Salvar desligado diz "Somente leitura nesta versão. Salvar como grava uma cópia no computador".

**PO11 · O gráfico escolhe a "primeira numérica", que costuma ser o id — atrapalha · P**
- **Onde:** `design/README.md` do #39 ("O app já escolhe as colunas ao abrir: a selecionada na grade e a primeira
  numérica").
- **O que acontece no plano:** na `clientes`, a primeira numérica é `id`. O gráfico de abertura seria "Soma de id por
  cidade". A prancheta mostra `limite` porque quem desenhou escolheu.
- **Proposta:** a escolha pula chave primária, chave estrangeira e INTEGER com todos os valores distintos; sem nenhuma
  sobrando, Contagem.

**PO12 · O plano da 0.3 não fecha o caminho "criar tabela e colar do Excel" — atrapalha · M**
- **Onde:** backlog da U5 (`revisao-ui-0.3.md`), `planos-0.2.3.md` A.5 ("Colar criando linhas … fora").
- **O que acontece:** o vazio com as colunas (U5) convida a colar; o Colar não cria linhas, e a 0.3 não o traz.
- **Proposta:** na 0.3, depois do PR 4: colar mais linhas do que há cria as linhas que faltam (Dados e CSV), numa
  transação e num Desfazer, com a pergunta acima de 100 células que já existe.

### Polimento

**PO13 · O `amostras` cria um banco chamado `--help` — polimento · P.** `cargo run -p amostras -- --help` grava
`--help` como arquivo SQLite na pasta. Recusar argumento que começa com `-`.

---

## (b) A voz do designer

> *"O Estúdio está bonito e coerente. O que falta é a disciplina dos estados que ninguém desenha: a décima aba, a
> coluna rolada, o número que não cabe."*

### Bloqueia

**D1 · As abas de documento cortam até ninguém saber qual é qual — bloqueia o reconhecimento · M**
- **Onde:** a fileira de abas da casca. [04](capturas-revisao-26-10/04-abas-cortadas.png)
- **Passos:** abra a pasta da revisão e os seis arquivos e uma Consulta, a 1280 × 800: "en…", "exp…", "rela…", "ap…",
  "con…", "exe…", "Co…". Com cinco abas, "vendas_202…" (dois anos de vendas abertos ficariam iguais).
- **Princípio:** reconhecer em vez de lembrar; a própria regra da U6 (o nome cortado no meio, guardando a extensão).
  O ícone de tipo ajuda, mas três abas de CSV têm o mesmo ícone.
- **Proposta:** largura mínima de ~120 px por aba (o ícone, ~10 letras e o ×); acima disso, a fileira rola na
  horizontal com a roda e com Ctrl+Tab, e o "Todas as abas" (o chevron, que já existe) lista todas; a aba ativa nunca
  é cortada; corte no meio com a extensão inteira ("vend…2025.csv"); nomes iguais ganham a pasta-mãe em `--t3`, como no
  VS Code.

### Atrapalha

**D2 · A coluna "#" e a caixa de marcar rolam para fora da vista — atrapalha · M**
- **Onde:** `NucleoGrade` (CSV, Dados, Excel, Consulta). [06](capturas-revisao-26-10/06-csv-busca-perde-o-numero-da-linha.png)
- **Passos:** no `vendas_2025.csv`, End numa célula ou Ctrl+F "P099999": a grade rola para a direita e a coluna "#"
  some; a pessoa não sabe mais em que linha está (a busca põe a célula achada na última linha visível, colada na borda).
- **Princípio:** visibilidade do estado do sistema; a convenção das planilhas (os cabeçalhos de linha não rolam).
- **Proposta:** "#" e a caixa de marcar fixas à esquerda (a rolagem horizontal só move as colunas de dados), com uma
  sombra curta de 6 px no fio quando há conteúdo por baixo; a busca centra a célula achada na vertical. Junto com a
  U2, que mexe no mesmo núcleo.

**D3 · O Caminho das Propriedades tem três desenhos — atrapalha · P**
- **Onde:** Propriedades › Arquivo. [07](capturas-revisao-26-10/07-caminho-em-tres-formas.png)
- **O que acontece:** no banco, "…/exemplo.db" numa linha com o Copiar; na tabela do mesmo banco, o caminho inteiro
  quebrado em três linhas, sem o Copiar; no CSV, "venda…2025.csv", que corta o nome que está escrito inteiro na aba.
  A coluna do valor tem ~95 px, então o "…/dados/vendas_2025.csv" que a U6 prometeu nunca aparece.
- **Princípio:** consistência; o mesmo dado, a mesma peça.
- **Proposta:** uma peça só (`interface::pecas::caminho`): o rótulo em cima e o caminho na largura inteira da seção,
  em até duas linhas, cortado do começo e com o nome inteiro, o Copiar na ponta. Em todas as Propriedades.

**D4 · O "Colunas" da Visão geral é um lápis — atrapalha · P**
- **Onde:** `editor/src/visao_geral.rs:582` (`Botao::icone(Icone::Editar)`). [08](capturas-revisao-26-10/08-visao-geral-colunas-com-lapis.png)
- **O que acontece:** passar o ponteiro na linha mostra um lápis; a dica diz "Colunas de itens_pedido".
- **Princípio:** correspondência com o mundo real: lápis é renomear ou editar. O #36 tira essa mesma silhueta do
  "Colunas" do menu Tabela.
- **Proposta:** o ícone da vista Colunas do segmentado das Propriedades (o mesmo destino).

**D5 · No #36, o "Excluir a tabela" leva a lixeira da barra, que nos Dados exclui linhas — atrapalha · P**
- **Onde:** PR #36, `casca/menus.rs` (Excluir a tabela e Nova tabela com silhueta).
- **O que acontece:** a U11 diz "ícone só quando o mesmo verbo tem botão na barra", e o menu Tabela só existe com uma
  tabela ativa, onde a lixeira da barra é "Excluir as linhas" e o "+" é "Nova linha". O menu ensina que a lixeira apaga
  a tabela.
- **Princípio:** consistência; o menu deve ensinar a barra, não um homônimo dela.
- **Proposta:** a regra fica "o mesmo verbo **sobre o mesmo objeto** na aba ativa": sai a silhueta do Excluir a
  tabela (ele já é vermelho) e a do Nova tabela quando a aba ativa é de Dados. Ver a divergência 5.

**D6 · O log tem uma faixa azul de duas linhas que não some, e a busca pinta só um achado — atrapalha · P**
- **Onde:** `tela_texto` (leitor de arquivo grande). [05](capturas-revisao-26-10/05-log-faixa-e-busca.png)
- **O que acontece:** "Este arquivo tem 1,4 MB e abre só para leitura…" com Abrir no programa padrão e Mostrar na
  pasta ocupa ~64 px, para sempre, enquanto o rodapé já tem o chip "SOMENTE LEITURA" e o menu Arquivo já tem o Abrir
  no programa padrão. A busca pinta só o achado atual: o ERROR da linha 25 fica sem cor, o da 32 pintado.
- **Princípio:** a regra U4 do guia (a faixa só para o que a aba e o rodapé não dizem); visibilidade.
- **Proposta:** o aviso numa linha só, com × (fechar guarda por arquivo), ou só o chip do rodapé com o motivo na dica.
  Na busca, todos os achados à vista com o realce fraco (`--selh`) e o atual com o forte, como no VS Code.

**D7 · No #39, duas fileiras de ícones de gráfico empilhadas — atrapalha · P (prancheta)**
- **Onde:** `FuturoGrafico`, cena da vista Gráfico. [10](capturas-revisao-26-10/10-grafico-duas-fileiras-de-icones.png)
- **O que acontece:** as seis vistas são ícones (a do Gráfico é `g_colunas`, barras em pé) e, logo embaixo, o Tipo
  são outros quatro ícones, o primeiro com barras deitadas. São dez ícones sem rótulo em 60 px de altura, dois deles
  quase iguais.
- **Princípio:** lei de Hick, reconhecer em vez de lembrar; Von Restorff perdido (nada se destaca).
- **Proposta:** as vistas continuam ícones (já é assim na doca estreita); o **Tipo** vira uma lista suspensa com o
  ícone e o nome ("Barras", "Linhas", "Histograma", "Dispersão"), que mede o texto e cabe nos 312 px da doca de 340.

**D8 · No #39, a dispersão desenha os eixos a partir do zero — atrapalha · P (prancheta e regra do PR 8)**
- **Onde:** `FuturoGrafico`, cena "Amostra". [11](capturas-revisao-26-10/11-grafico-dispersao-eixos-do-zero.png)
- **O que acontece:** temperatura 14–36 e umidade 40–95 desenhadas em 0–40 × 0–100: metade da área vazia e a nuvem
  achatada.
- **Princípio:** a razão dados/tinta; a convenção (pandas, Seaborn, Excel ajustam a dispersão e a linha aos dados).
- **Proposta:** a regra no README do #39: barras e histograma começam do zero; linhas e dispersão ajustam o eixo aos
  dados, com limites redondos (14 → 10, 95 → 100).

**D9 · No #39, o CSV remoto mostra números formatados, e o "Somente leitura" aparece três vezes — atrapalha · P**
- **Onde:** `FuturoSsh`, cena do Explorador remoto. [12](capturas-revisao-26-10/12-ssh-csv-com-numero-formatado.png)
- **O que acontece:** o `saida.csv` mostra "1.240", "88.410", "1,8", "—": um CSV de job ETL tem "1240", "1.8" e vazio.
  A prancheta contradiz a U1 (e mostra a confusão da PO3). O "Somente leitura" está no chip da faixa, na barra de
  status e nas Propriedades. A "Cópia local …/srv/etl/saida.csv" cortada fica igual ao caminho remoto.
- **Proposta:** a grade com o texto do arquivo; o chip só na faixa; a cópia local como "no cache do DataBox" com o
  Mostrar na pasta, e o caminho inteiro na dica.

### Polimento

**D10 · Os números da Consulta não alinham pela vírgula — polimento · M (junto da U1).** Em
[09](capturas-revisao-26-10/09-consulta-media-sem-alinhamento.png), "8471.9475", "11086.165" e
"10857.058571428572" alinham pela direita, e a vírgula anda. Com algarismos tabulares, completar à direita com espaço de
algarismo (U+2007) até as casas da maior da página, só na pintura (o Copiar leva o valor).

**D11 · A Soma do CSV perde o zero final — polimento · P.** "Soma 2.929.066.524,4" ao lado de "Mínimo 20,93"
([13](capturas-revisao-26-10/13-csv-coluna-maximo-e-soma.png)). As contas saem com as casas da coluna (o máximo de
casas lido), "2.929.066.524,40".

**D12 · A cena k do #39 (texto 40% mais longo) não mostra o que o texto dela diz — polimento · P.** A legenda diz
que Agregar e Ordem descem e que os botões passam a uma linha cada; na cena, os dois continuam lado a lado e o
"[Máiór priméiró·…" é cortado. Desenhar o estado que a regra promete, porque é ele que o pseudoidioma vai conferir.

**D13 · A dica da célula repete o texto que cabe inteiro — polimento · P.** Nos Dados, parar o ponteiro em
"nuno.10@example.test" mostra a dica com o mesmo texto. A dica só quando o texto foi cortado.

**D14 · A planilha com uma folha só mantém a faixa das planilhas — polimento · P.** 32 px com a pílula "Vendas" e as
setas desligadas ([03](capturas-revisao-26-10/03-excel-colunas-na-largura-padrao.png)); o rodapé já diz "Vendas". Com
uma planilha visível, a faixa some.

**D15 · No #39, o eixo da linha diz "R$" — polimento · P.** A unidade não vem dos dados; o título do eixo é o nome da
coluna (ou das séries, com a legenda).

---

## (c) O que ficou muito bom e deve ser mantido

- **Ctrl+F no arquivo inteiro:** "P099999" num CSV de 100 mil linhas em menos de 1 s, "1 de 1" na ponta da barra
  dos filtros, Esc volta.
- **Menu da célula:** "Filtrar por este valor" vira a etiqueta "cidade igual a Recife" e o rodapé "Linhas 1–8 de 8 ·
  filtradas de 60". Nos Dados, as larguras ficam estáveis (o defeito da PO2 é só do CSV).
- **Rodapé da seleção:** "Contagem 8 · Soma 64.935,52 · Média 8.116,94", com a célula NULL fora da contagem.
- **A detecção do CSV:** `;`, vírgula decimal, CRLF, data pt-BR (pastilha 31) e o CEP com zero à esquerda como texto,
  sem a pessoa fazer nada.
- **A migalha do JSON** sincronizada com a estrutura nas Propriedades ("raiz › conexoes › 0").
- **#39, a chave mudada:** sem botão de aceitar, as duas impressões, a linha do `known_hosts` e o `ssh-keygen -R`.
  É como deve ser e raro de ver.
- **#39, a amostra honesta:** a etiqueta âmbar no rodapé e "amostra" no subtítulo do gráfico, para o PNG não mentir.
- **#36:** testes que conferem ícone e ausência de ícone item a item, e o "Copiar o nome" achado fora da lista.

---

## (d) Divergências entre o PO e o designer

| # | Assunto | PO recomenda | Designer recomenda |
|---|---|---|---|
| 1 | **Fundo do PNG do gráfico** (dúvida 1 do #39) | **claro sempre**: o PNG vai para relatório, slide e e-mail, quase sempre brancos; um gráfico escuro colado num slide branco parece erro | **o tema da tela**, com "Copiar a imagem (fundo claro)" no menu do gráfico quando o tema é escuro: o que se vê é o que se leva |
| 2 | **Q1: o clique na árvore** (#37) | o clique simples insere (rápido, como o plano diz) e **entra no Ctrl+Z** | **alvos separados**: o clique seleciona e abre ou fecha (como o Explorador); o duplo clique e o botão ↵ da linha inserem. Duplo clique não compete com o simples (selecionar é pré-requisito), então não há atraso a pagar |
| 3 | **Milhar nos inteiros** (U1, PO8) | sem milhar em PK, FK, na "#" **e nas colunas com nome de id, ano, CEP** (`*_id`, `ano`, `year`, `cep`) | sem milhar **só em PK, FK e na "#"**, que o esquema declara; a heurística pelo nome é imprevisível (`anos_de_casa`) e muda com o idioma |
| 4 | **O aviso do arquivo grande** (D6) | manter um aviso visível no começo: quem tenta digitar e nada acontece precisa saber por quê | só o chip do rodapé, e o aviso surge na primeira tecla que tenta editar (um toast "Somente leitura: o arquivo tem 1,4 MB") |
| 5 | **Silhueta do Excluir a tabela** (#36, D5) | manter: é uma ação destrutiva e a lixeira ajuda a achá-la no menu | tirar: a lixeira na barra, nessa aba, é Excluir as linhas; o vermelho já marca o perigo |

As duas vozes concordam em todo o resto, inclusive na ordem da seção (e).

---

## (e) Itens que merecem PR

### Antes da 0.3.0 começar a varredura da tradução (I2/I3)

Tocam textos, larguras ou telas que a varredura vai varrer; feitos depois, brigam com ela.

| PR | Achados | Esforço |
|---|---|---|
| `fix(tela_csv)`: medir as colunas quando a página da vista nova chega, com teste de tela do filtro | PO2 | P |
| `fix(tela_planilha)`: largura pelo conteúdo quando o arquivo não declara; número não corta com "…" (`####` e a dica) | PO1 | P |
| `fix(tabular, tela_csv)`: linhas vazias no fim não contam; as contas da coluna no formato da interface, com as casas da coluna; Mín./Máx. calculados | PO3, PO4, D11 | P |
| `docs(design)`: as regras de número no guia (o texto do arquivo só na grade; o calculado no idioma; sem milhar em PK/FK/"#"; REAL na forma mais curta; NaN e científico) e o glossário inicial do `idioma` | PO3, PO8, PO9 | P |
| `feat(comum)`: `.jsonl`, `.ndjson` e `.sql` listados como texto no Explorador até o PR 10 | PO5a | P |
| `fix(editor, interface)`: abas com largura mínima, rolagem, corte no meio com a extensão e pasta-mãe nos nomes iguais | D1 | M |
| `fix(interface)`: a peça de Caminho das Propriedades; o ícone Colunas na Visão geral; a dica só na célula cortada | D3, D4, D13 | P |
| ajustes **dentro do #36** (silhueta do Excluir a tabela e do Nova tabela, se o Isaías ficar com o designer) | D5 | P |
| ajustes **dentro do #37** (inserção no Ctrl+Z, ou alvos separados) | PO7 | P |
| ajustes **dentro do #39** (Tipo em lista; eixos da dispersão; CSV remoto com o texto; a cena k; o "R$"; a escolha da coluna) | D7, D8, D9, D12, D15, PO11 | P |

### Dentro da 0.3

| Onde | Achados | Esforço |
|---|---|---|
| Junto da **U1** (onda 3): números alinhados pela vírgula | D10 | M |
| Junto da **U2** (onda 3, mexe no `NucleoGrade`): "#" e caixa de marcar fixas; a busca centra o achado | D2 | M |
| **Trilha D** (formatos): Filtrar linhas no texto e no leitor só leitura | PO5b | M |
| `tela_texto`: o aviso do arquivo grande numa linha (ou só o chip) e todos os achados da busca pintados | D6 | P |
| Medir o editor com 2 e 4 MiB e decidir o limite do JSON | PO6 | P |
| **PR 14** (S1): o Salvar como e o Consultar com SQL ligados no remoto; a dica do Salvar desligado | PO10 | P |
| Depois do **PR 4**: colar criando as linhas que faltam (Dados e CSV) | PO12 | M |
| A planilha de uma folha sem a faixa das planilhas | D14 | P |
| `amostras`: recusar argumento com `-` | PO13 | P |

**Fora deste relatório:** o CI do Linux do #38 e do #39 (PRs só de documento) falha no
`shift_f10_na_grade_abre_o_menu_da_celula_e_o_esc_volta_a_ela` (`editor/tests/foco.rs:192`), uma corrida que outra
sessão da onda 0 ("corrida-shift-f10") já está investigando.
