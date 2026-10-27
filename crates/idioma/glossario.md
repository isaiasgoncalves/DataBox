# Glossário do inglês

Os termos fixos da tradução do DataBox para o inglês (decisão 14 de [`design/planos-0.3.md`](../../design/planos-0.3.md)):
quem traduz o `en.po` usa estes e quem revisa confere por eles. O estilo é o das ferramentas que o público já usa
(DBeaver, DB Browser for SQLite, VS Code, Excel): o termo comum, em minúsculas no meio da frase, e frases curtas no
imperativo nos botões e nos itens de menu ("Open folder", não "Opens the folder").

Este arquivo nasce na I1 com o que já está decidido; a I4 (o `en.po` completo) o completa. Um termo marcado
**proposta** espera a palavra do Isaías (PO9 da [revisão de 26/10/2026](../../design/revisao-po-designer-26-10.md)):
até lá, o tradutor usa a proposta e não inventa outra.

## Os termos de banco e de grade (decididos)

| Português | Inglês | Observação |
|---|---|---|
| tabela | table | |
| consulta | query | "Consulta SQL" (a aba e a ferramenta): **SQL query** no título e no menu; "query" no meio da frase. |
| linha | row | Nunca "line" para dados; "line" só no texto e no editor de código ("Linha 3, coluna 5" → "Ln 3, Col 5", como o VS Code). |
| coluna | column | No editor de texto, a posição do cursor também é "Col". |
| filtro / filtrar | filter | "Limpar filtros" → "Clear filters". |
| banco (de dados) | database | Nunca "bank". "Abrir banco" → "Open database". |
| visão (do SQL) | view | |
| índice | index | Plural "indexes" (como o SQLite e o DBeaver), não "indices". |
| gatilho | trigger | |
| chave primária / estrangeira | primary key / foreign key | Siglas PK e FK iguais. |
| célula | cell | |
| planilha (do Excel) | sheet | O arquivo é "workbook" só quando a frase precisa distinguir; no geral, "Excel file". |
| pasta | folder | |
| arquivo | file | |
| gravar / salvar | save | "Salvar" e "Gravar" viram os dois "Save". |
| desfazer / refazer | undo / redo | |
| exportar | export | |

## Os termos difíceis (PO9)

| Português | Inglês proposto | Por quê |
|---|---|---|
| Trazer colunas | **Bring in columns** (proposta) | É o PROCV/merge do DataBox: traz colunas de outro arquivo casando por uma chave. "Merge" e "Join" prometem uma junção completa de tabelas, que não é o que acontece (só entram colunas, as linhas do arquivo aberto ficam); "Lookup columns" lembra a função do Excel, mas soa como busca. "Bring in" diz o gesto, e o "from…" da janela diz de onde. |
| Ficha da linha | **Row details** (proposta) | A janela com todos os campos de uma linha, um embaixo do outro. "Record form" é o termo do Access, pouco conhecido de quem usa DBeaver; "Row details" é o que o DBeaver e o DataGrip chamam de "record view"/"details" e se entende sem explicação. |
| Bancada | **Workbench** (proposta) | O banco temporário em que os arquivos (CSV, Excel, JSON) viram tabelas para o "Consultar com SQL". "Workbench" é o termo de ferramentas de dados (MySQL Workbench) para o lugar de trabalho; "Scratch database" é exato, mas técnico demais para a interface (fica para a documentação). |
| Gravação imediata | **Instant save** (proposta) | A edição na grade grava no banco na hora, sem um Salvar (com Desfazer). "Auto-save" sugere um salvamento de tempos em tempos, que não é o caso; "Instant save" diz que cada mudança já está no arquivo. |
| Consulta SQL | **SQL query** (proposta) | Ver a tabela acima; "Query" sozinho no título da aba fica ambíguo com a consulta do filtro. |
| Propriedades | **Properties** (proposta) | O painel da direita, como no Visual Studio e no VS Code. "Inspector" é o do Xcode e do navegador, menos conhecido aqui. |
| Visão geral | **Overview** (proposta) | |
| Marcar as linhas | **Check rows** (proposta) | As caixas de marcar da grade; "Select rows" já é o da seleção azul, que é outra coisa. |
| Somente leitura | **Read-only** | Termo fixo do Windows e do SQLite. |
| lista para IN (…) | **IN (…) list** (proposta) | O "Copiar como": mantém o `IN` do SQL à vista. |

## Os nomes que não se traduzem

- Os nomes dos idiomas no seletor das Preferências ficam no próprio idioma: "Português (Brasil)", "English".
- Os nomes do SQL (`SELECT`, `INTEGER`, `NULL`, `PRAGMA`), os formatos (CSV, JSON, JSON Lines, Excel, Markdown) e
  os nomes de tabela, coluna e arquivo da pessoa.
- As opções da linha de comando (`--somente-leitura`, `--criar-atalho`…) são nomes fixos: a ajuda em inglês explica
  cada uma, sem trocar o nome. `-h, --help` e `--version` já são os principais.
