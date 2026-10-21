# Planos da 0.2.3 — refinamentos e correções

Decidido em 21/10/2026: antes da 0.3, uma 0.2.3 só com **melhorias, refinamentos e correções** do que já existe,
sem recurso novo. Os recursos (perfil, Salvar como, SQL sobre CSV, gráficos…) ficam na
[`planos-0.3.md`](planos-0.3.md). Nada aqui está implementado.

| | Item | Tipo | Por quê |
|---|---|---|---|
| A | Menu de contexto da seleção e o rodapé da seleção | refinamento | a seleção da 0.2.2 parou no Ctrl+C |
| B | Ctrl+F no CSV, no Excel e no texto/JSON | correção | o Ctrl+F existe no app e não faz nada nesses documentos |
| C | Contas da coluna do CSV sem ler a coluna inteira na memória | correção | um CSV grande pode esgotar a memória |
| D | CI confiável no Windows | correção (infra) | a repetição dos testes esconde as falhas intermitentes |
| E | Fatiar o `editor/src/app.rs` e o `editor/tests/casca.rs` | dívida técnica | a 0.3 mexe nos dois; arquivos de ~5 mil linhas |

---

## A. O menu de contexto da seleção

A 0.2.2 ensinou a
selecionar um intervalo (arrastar, Shift+clique, Shift com as setas) e parou no Ctrl+C: o clique direito numa célula
ou num intervalo não faz nada em nenhuma grade, e só os cabeçalhos do CSV e do Excel têm menu
(`tela_csv::colunas`, `tela_planilha::menu`). O "Exportar a seleção…" e o "Perfil da seleção" ficam para a 0.3.

### A.1 O menu

- **Abre com** o clique direito numa célula (se ela está dentro do intervalo selecionado, o menu age sobre o
  intervalo; fora dele, a célula vira a ativa e o menu age só nela), com o Shift+F10 e com a tecla Menu, com o foco na
  grade (o menu nasce na célula ativa). Esc fecha e devolve o foco à grade. Vidro fosco, como os outros menus
  (`PainelMenu`), com as silhuetas da barra nos itens que já têm ícone.
- **Itens, nesta ordem e por grade** (o que não vale numa grade não aparece; o que vale mas não pode agora aparece
  desligado, com o motivo na dica):

| Item | Atalho | Dados | CSV | Excel | Consulta |
|---|---|---|---|---|---|
| Copiar | Ctrl+C | ✓ | ✓ | ✓ | ✓ |
| Copiar com cabeçalho | Ctrl+Shift+C¹ | ✓ | ✓ | ✓ | ✓ |
| Copiar como › CSV · JSON · Markdown · `INSERT` · lista para `IN (…)` · pandas | | ✓ | ✓ | ✓ | ✓ |
| — | | | | | |
| Colar | Ctrl+V | ✓ | ✓ | | |
| Limpar | Delete | ✓ | ✓ | | |
| Definir como NULL | | ✓ | | | |
| — | | | | | |
| Filtrar por este valor | | ✓ | ✓ | ✓ | |
| Excluir este valor do filtro | | ✓ | ✓ | ✓ | |
| — | | | | | |
| Marcar as linhas | | ✓ | ✓ | | |
| Abrir a ficha da linha | Enter² | ✓ | | | |
| Excluir as linhas | | ✓ | ✓ | | |

¹ O Ctrl+Shift+C hoje é o "Copiar caminho" do Explorador; na grade com foco, vale o da grade (decidido em 21/10/2026).
² O mesmo atalho que a ficha já tem nos Dados.

### A.2 O que cada item faz

- **Copiar:** o mesmo Ctrl+C da 0.2.2 (TSV, como o Excel; no CSV, lê na hora as linhas que não estavam lidas). O
  aviso diz o que foi copiado ("3 linhas × 2 colunas copiadas.").
- **Copiar com cabeçalho:** a primeira linha leva os nomes das colunas do intervalo.
- **Copiar como:**
  - *CSV:* vírgula e aspas pelas regras do RFC 4180 (o separador do arquivo, no CSV);
  - *JSON:* uma lista de objetos, `[{"nome": "Bruno", "limite": 13741.03}, …]`, com os números como números e o NULL
    como `null` (no CSV e no Excel, o tipo detectado da coluna decide);
  - *Markdown:* tabela com o cabeçalho e as barras escapadas;
  - *`INSERT`:* `INSERT INTO "clientes" ("nome", "email") VALUES (…), (…);` com os nomes citados por `comum::q`, um
    comando para até 500 linhas (no CSV e no Excel, o nome da tabela é o do arquivo ou da planilha);
  - *lista para `IN (…)`:* só com uma coluna no intervalo, `('Recife', 'Curitiba')` sem repetidos, números sem aspas;
  - *pandas:* `pd.DataFrame({"nome": ["Bruno", …], "limite": [13741.03, …]})`, com `None` no NULL, para colar num
    notebook (ideia do TL, aceita em 21/10/2026).

  Sem HTML: o egui só põe texto puro na área de transferência, e o HTML colado como texto não vira tabela no Word.
- **Colar (Dados e CSV):** cola o TSV da área de transferência a partir da célula ativa, por cima das células
  existentes; um valor só, com um intervalo selecionado, preenche o intervalo inteiro. Não cria linhas nem colunas
  (o colar como nova coluna do CSV continua no menu do cabeçalho). Se o que foi colado passa da última linha ou coluna,
  o aviso diz quantas ficaram de fora. Coluna que não se edita (gerada, rowid só leitura, banco só leitura) desliga o
  item com o motivo.
- **Limpar:** o vazio no CSV; nos Dados, o texto vazio nas colunas de texto e o NULL nas outras que aceitam NULL
  (coluna `NOT NULL` sem padrão desliga o item com o motivo). **Definir como NULL:** só nos Dados.
- **Filtrar por este valor / Excluir este valor do filtro:** pela célula ativa, viram uma condição "igual a" ou
  "diferente de" na barra de filtros de sempre (no NULL dos Dados, "é nulo" e "não é nulo"; no vazio do CSV, "vazio" e
  "não vazio"). A condição aparece na faixa como as outras e sai pelo × dela.
- **Marcar as linhas:** marca as linhas do intervalo (as caixas da primeira coluna), somando às já marcadas.
- **Excluir as linhas:** as linhas do intervalo, com a mesma pergunta e o mesmo caminho do Excluir da barra.
- **Abrir a ficha da linha:** a da linha da célula ativa.

### A.3 O rodapé da seleção

Com mais de uma célula selecionada, o rodapé do documento troca o lado direito por "Contagem 33 · Soma 98.231,40 ·
Média 8.930,13 · Mín. 154,03 · Máx. 17.810,34" (como a barra de status do Excel; decidido em 21/10/2026): a contagem é
das células não vazias; a soma, a média, o mínimo e o máximo, só das numéricas (o tipo da coluna, nos Dados; o detectado, no CSV e no Excel; o valor, na Consulta), no formato pt-BR. Onde não couber (a janela estreita, as Propriedades largas), sai primeiro o
Mín. e o Máx., depois a Média, e o que saiu vai para a dica do rodapé. Com
o intervalo lido só em parte (CSV de milhões de linhas), a conta roda em segundo plano e mostra "Somando…". Uma
célula só, o rodapé volta ao de sempre.

### A.4 Desfazer

Cada Colar, Limpar ou Definir como NULL é **uma alteração só** no Desfazer:

- **CSV:** `Edicoes::em_um_passo`, que já existe (o Trazer colunas usa).
- **Dados:** hoje o Desfazer do banco desfaz *uma* célula (`Banco::desfazer_celula`). O Colar e o Limpar de um
  intervalo gravam numa transação só (tudo ou nada) e o Desfazer grava de volta todas as células, cada uma só se ainda
  tem o valor gravado (a mesma regra de hoje, célula a célula); se alguma mudou depois, nada é desfeito e o aviso diz
  qual. O toast diz "Gravado em clientes (9 células)".

### A.5 Fora do menu na 0.2.3

- **Exportar a seleção…** (vem com o Salvar como da 0.3) e **Perfil da seleção** (vem com o Perfil da 0.3).
- Colar criando linhas ou colunas novas; Preencher para baixo (Ctrl+D); copiar e colar entre o DataBox e ele mesmo
  guardando os tipos.
- Menu de contexto no texto e no JSON (o editor já tem o do egui).

### A.6 Técnica (TL)

- **Peça comum na `interface`:** `grade::menu_da_celula(ui, alvo, ofertas) -> Option<AcaoCelula>`. A grade só abre o
  menu e devolve a ação; cada tela diz o que liga (`OfertasCelula`, como as `Ofertas` dos verbos, com o motivo do
  desligado) e cumpre a ação. O clique direito e o Shift+F10 entram no `NucleoGrade`, que já sabe o intervalo
  (`grade::Intervalo`), para as quatro grades reagirem igual.
- **Formatos de cópia** no `tabular::colunas`, ao lado do `escrever_tsv` (o `interface` já depende dele só para isso):
  `escrever_csv`, `escrever_json`, `escrever_markdown`, `escrever_insert(tabela, …)` (com o `comum::q`),
  `escrever_lista_in`, `escrever_pandas`. Sem tela, testados em unidade (aspas, quebra de linha dentro da célula, NULL, números pt-BR do
  CSV com vírgula decimal).
- **Banco:** `Banco::atualizar_intervalo(tabela, &[(Chave, coluna, Entrada)]) -> Resultado<Vec<LinhaGravada>>` numa
  transação (ou num `SAVEPOINT`, se a Consulta deixou uma aberta) e `desfazer_intervalo(…)`, irmão do
  `desfazer_celula`. As regras do Desfazer (desliga depois de excluir, de mudar a estrutura, de uma transação da
  Consulta) continuam valendo.
- **Filtros:** os Dados usam `banco::Filtro` (14 operadores, com `Nulo`/`NaoNulo`), o CSV o `OperadorCsv` (12, com
  `Vazio`/`NaoVazio`) e o Excel o filtro dele; um `Filtro::igual_a(coluna, celula)` em cada um.
- **Soma da seleção:** `interface::formatar_numero` para o pt-BR; no CSV, os valores pelo `tabular::numero` com o
  decimal do arquivo; leitura de fundo com `Promessa` e `parar` quando o intervalo não está todo na página.
- **Acessibilidade:** o menu no AccessKit como os outros (`Role::Menu`, itens com o atalho no nome); teste em
  `editor/tests/foco.rs` para o Shift+F10 e o Esc.
- **Crates tocados:** `interface`, `tabular`, `banco`, `tela_dados`, `tela_csv`, `tela_planilha`, `tela_consulta`.
  Nenhum no `editor/src/app.rs`, nenhuma dependência nova, nenhum contrato entre crates quebrado (o
  `ARQUITETURA.md` ganha a peça do menu e as duas funções do `banco`).
- **Riscos:** o Colar nos Dados é escrita em lote no banco da pessoa: transação, prévia no aviso ("Colar 120 células
  em clientes?" acima de 100 células) e o Desfazer. O CI novo (D) entra antes; os testes de tela do menu rodam no
  passo de tela, numa thread.

#### Testes do menu

- Unidade: os seis formatos de cópia; `atualizar_intervalo` e `desfazer_intervalo` (tudo ou nada, célula mudada por
  fora, dentro de uma transação aberta); a soma pt-BR.
- Tela (`kittest`), em cada grade: clique direito dentro e fora do intervalo, Shift+F10, itens desligados com a dica,
  Copiar como lido de volta da área de transferência, Filtrar por este valor criando o chip, Colar e Ctrl+Z, o rodapé
  da seleção. Capturas do menu nos dois temas para conferir com a prancheta.

#### Design

Uma prancheta (ou cena nova na 17) para o menu da célula: os itens, os separadores, o submenu "Copiar como", o
desligado com a dica e o rodapé da seleção, nos dois temas, antes do PR do menu nas telas.

---

## B. Ctrl+F no CSV, no Excel e no texto

- *Problema:* a busca da barra (Ctrl+F) funciona nos Dados, na Visão geral, na Estrutura e na Consulta, mas o CSV, o
  Excel e o editor de texto/JSON não implementam o `Documento::busca`: o campo da barra fica desligado e o Ctrl+F não faz nada. Num CSV de
  400 mil linhas, "onde aparece este CPF?" não tem resposta.
- *Pronto quando:* Ctrl+F busca no arquivo inteiro, não só na página carregada; Enter e Shift+Enter vão ao próximo e
  ao anterior; a faixa diz "3 de 41" (ou "41+" enquanto conta); no CSV e no Excel, a célula achada vira a ativa e a
  grade rola até ela; no texto e no JSON, o trecho fica pintado como na Consulta (`AreaTexto::destacar`), também no
  leitor só leitura de arquivos grandes (até 32 MiB). Sem diferenciar maiúsculas nem acentos, como os filtros
  (`interface::sem_acento`). Esc limpa e volta à célula de antes.
- *Técnica:* `tabular::csv::Tabela::buscar(texto, a_partir_de, parar) -> Option<(linha, coluna)>` percorre o arquivo
  pelo índice sem montar páginas, as alterações pendentes por cima (`Edicoes`); `planilha::Planilha::buscar` sobre o
  iterador em fluxo; no texto, sobre a `String` carregada. Tudo em `Promessa`, cancelado a cada tecla (a leitura
  substituída, como na grade do banco). A busca respeita os filtros à vista (só nas linhas que o filtro mostra).
- *Testes:* unidade no `tabular` e no `planilha` (acento, aspas, campo com quebra de linha, alteração pendente);
  tela em cada um (Enter, Shift+Enter, "3 de 41", rolar até a célula).
- *Crates:* `tabular`, `planilha`, `tela_csv`, `tela_planilha`, `tela_texto`. Sem mudança no contrato do
  `Documento`.

---

## C. As contas da coluna do CSV em fluxo

- *Problema:* as Propriedades do CSV mostram as contas da coluna da célula ativa (vazias, mínimo, máximo, soma, maior
  texto). Para isso, `tela_csv::tela::estatistica` lê **a coluna inteira num `Vec<String>`**
  (`sessao::ler_colunas`) e só depois conta. Num CSV de milhões de linhas com textos longos, isso são centenas de MB
  por clique numa coluna.
- *Pronto quando:* as mesmas contas, com os mesmos números, calculadas numa passada sem guardar os valores
  (memória constante), com "Calculando…" e cancelamento ao trocar de coluna. Um teste de memória como o do
  `exportar/tests/memoria.rs`.
- *Técnica:* um `calcular_estatistica` que recebe um iterador; `tabular` ganha a leitura de uma coluna em fluxo. É
  também o começo do crate `perfil` da 0.3, sem antecipá-lo: aqui só as contas que já existem.

---

## D. CI confiável

- *Problema:* das últimas 40 execuções do CI, 12 falharam; as de teste, no Windows e em telas: o WARP com
  `STATUS_ACCESS_VIOLATION` quando vários testes criam o wgpu ao mesmo tempo, o `interface --test mostruario` caindo
  dentro do wgpu em paralelo, e testes que dependiam da dobra de um painel (o `casca_explorador_e_status`, corrigido no
  #11). O passo de testes do Windows roda tudo em paralelo e, se falhar, repete tudo numa thread só: cada falha
  intermitente custa uma rodada inteira e fica escondida.
- *Pronto quando:* no Windows, os testes de núcleo rodam em paralelo e os de tela (os que usam `egui_kittest`) num
  passo próprio com `--test-threads=1`, sem repetição; o Linux continua em paralelo (o lavapipe aguenta) e é a
  referência das capturas. Dez execuções seguidas sem falha intermitente. O tempo dos dois jobs medido antes e depois,
  no PR. O `CONTRIBUTING.md` e o README trocam a frase do "rode de novo com `--test-threads=1`" pela regra nova.
- *Técnica:* separar os alvos de tela por nome (`--test casca`, `--test pecas`…) ou por uma feature `telas` nos
  crates `tela_*`, `interface` e `editor`; um teste que cair por ambiente duas vezes vira `#[ignore]` com issue aberta,
  nunca retentativa silenciosa.

---

## E. Fatiar o `app.rs` e o `casca.rs`

- *Problema:* `editor/src/app.rs` tem 4.479 linhas e `editor/tests/casca.rs`, 4.979. A 0.3 mexe na fila de perguntas
  ao fechar (Salvar tudo), no despacho dos verbos e das ações de menu (Salvar como, Consultar com SQL, gráficos), e
  dois PRs ao mesmo tempo nesses arquivos brigam.
- *Pronto quando:* o `app.rs` vira módulos por assunto (a fila de salvar, os verbos e as ações de menu, os toasts, o
  ciclo da janela), e o `casca.rs` vira arquivos de teste por área (Explorador, abas, docas, menus, Propriedades),
  **só movendo código**: o diff não muda comportamento, os mesmos testes passam, nenhuma captura muda.
- *Técnica:* um PR só disso, sem nada junto, revisado com `git diff --color-moved`.

---

## Ordem e PRs da 0.2.3

1. `ci`: testes de tela num passo próprio, Windows numa thread, sem repetição (D). Primeiro, para os outros PRs já
   rodarem no CI novo.
2. `refactor(editor)`: fatiar `app.rs` e `tests/casca.rs` (E). Pode correr junto com o 3 e o 4 (não tocam o editor).
3. `feat(tabular, interface)`: formatos de cópia e o menu da célula na grade, com o mostruário (A).
4. `fix(tela_csv, tabular)`: as contas da coluna em fluxo (C).
5. `feat(tela_*)`: o menu nas quatro grades, só os itens de leitura (copiar, filtrar, marcar, ficha) e o rodapé da
   seleção (A). Depende do 3.
6. `feat(banco, tela_dados, tela_csv)`: Colar, Limpar, Definir como NULL e Excluir as linhas, com o Desfazer de uma
   vez (A). Depende do 5.
7. `fix(tabular, planilha, tela_csv, tela_planilha, tela_texto)`: Ctrl+F nos arquivos (B). Toca as mesmas telas que o
   5 e o 6: depois deles.
8. `chore`: versão 0.2.3 (CHANGELOG, README: o clique direito no guia "Explorar um banco" e a busca nos arquivos;
   capturas; a lista "O que testar à mão" do `RELEASE.md` com o menu e o Shift+F10 no Linux).

---

---

## Decisões para o Isaías (0.2.3)

1. ~~Formatos do "Copiar como"~~ **Decidido em 21/10/2026:** CSV, JSON, Markdown, `INSERT`, `IN (…)` e pandas.
2. ~~Colar e Limpar nos Dados na 0.2.3~~ **Decidido em 21/10/2026:** entram, com transação, Desfazer de uma vez e
   confirmação acima de 100 células.
3. ~~Ctrl+Shift+C~~ **Decidido em 21/10/2026:** "Copiar com cabeçalho" na grade com foco; no Explorador, continua o
   "Copiar caminho".
4. ~~Rodapé da seleção~~ **Decidido em 21/10/2026:** Contagem · Soma · Média · Mín. · Máx.
5. ~~Ctrl+F nos arquivos na 0.2.3~~ **Decidido em 21/10/2026:** entra como correção, no arquivo inteiro.
6. ~~CI~~ **Decidido em 21/10/2026:** testes de tela do Windows num passo próprio, numa thread, sem repetição; o
   Linux é a referência das capturas.
7. ~~Fatiar o `app.rs`~~ **Decidido em 21/10/2026:** na 0.2.3, num PR só de mover código.

Todas as decisões da 0.2.3 estão tomadas.
