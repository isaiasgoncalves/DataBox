# DataBox — arquitetura e contratos

O DataBox é um **executável nativo** em Rust (`DataBox.exe`), com a interface desenhada direto na GPU com **egui**:
uma caixa de ferramentas de dados que abre bancos SQLite, pastas, CSV, JSON, textos e planilhas do Excel. Este
documento é o contrato entre os crates: as decisões, a toolchain, as convenções e o que cada crate oferece. A API de
verdade de cada crate está documentada no topo do `lib.rs` dele. Quem muda um contrato muda este arquivo no mesmo
commit. O guia visual (o visual Estúdio) está em [`design/README.md`](design/README.md).

---

## 0. Decisões

| Decisão | Por quê |
|---|---|
| Interface em **egui 0.36** (eframe), uma janela nativa | Um `.exe` só, sem navegador nem servidor; desenhada na GPU, abre na hora e rola grades grandes sem travar. |
| Renderizador **glow (OpenGL)** por padrão; **wgpu** de reserva | Medido nesta máquina (Radeon 760M, release): glow ~110 MB de memória, wgpu ~380 MB (~300 MB só com DX12), os dois sem CPU parado. Se o OpenGL não abrir (máquina virtual sem driver), o app se relança com `--renderizador wgpu`. Os testes de captura (`egui_kittest`) usam wgpu, sem janela. |
| **Sem servidor nem navegador.** A interface chama o crate `servico` direto, em threads de fundo | Um processo só, sem portas abertas nem sessão para proteger. |
| O `servico` concentra as regras do banco aberto e devolve erros tipados (`ErroBanco` com `status` e `codigo`) | A interface reage a códigos estáveis (`transacao_aberta`, `destino_existe`, `banco_trocado`, `substituido`, `cancelado`), não ao texto da frase. |
| SQLite **embutido** (rusqlite `bundled`, 3.53) | Nada para instalar; mesma versão em toda máquina. |
| Excel (SpreadsheetML) gravado à mão em fluxo e comprimido pelo crate `zip`, sem `rust_xlsxwriter` (ver `exportar` no §4); CSV e JSON à mão | Memória constante e rápido: 1 milhão de linhas em Excel em ~7 s. |
| Diálogos nativos com **rfd** (IFileDialog do Windows), modais, na thread da interface, com a janela do app como dona | Nascem na frente e prendem o foco; o laço do diálogo segue entregando as mensagens da janela. Se o diálogo não abrir (uma resposta vazia em menos de 150 ms), a casca usa a janela "Pedir o caminho" para abrir, criar e copiar o banco. |
| Preferências num JSON por pessoa (`%APPDATA%\DataBox\preferencias.json`, registro em `%LOCALAPPDATA%\DataBox\databox.log`), com trava entre processos e gravação atômica (§4.5) | Duas janelas abertas não apagam o que a outra gravou; um arquivo estranho não derruba o app. |
| Visual **Estúdio** em código (crate `interface`): os tokens claro e escuro das pranchetas, as fontes do Windows lidas em tempo de execução (Segoe UI, Consolas ou Cascadia Mono), o logo próprio, os ícones em PNG rasterizados dos SVG e as peças (botões, campos, cartão, menus, abas, árvore, painéis, janela, toast…) | O egui não usa CSS nem desfoca o que está atrás: o vidro fosco das camadas que se sobrepõem é feito com a foto da própria janela, desfocada embaixo do `--flut` a 78/80% (`interface::fosco`, §4.7), com o liso a 96% de reserva (`design/README.md`). Nada de terceiros embutido (fontes nem marcas). |
| Cada tela é o **documento de uma aba** e tem endereço (`Rota`); Voltar e Avançar andam num **histórico único** (aba + endereço), com Alt+←/→ e os botões laterais do mouse | As abas deixam várias partes de tabela e várias Consultas abertas juntas, e o Voltar funciona como num navegador. |
| Janela **sem a decoração do Windows**, com a barra de título própria integrada aos menus (só no Windows; `BARRA_PROPRIA` no main.rs) | Uma faixa só de 30 px para o logo, os menus, o título e os botões da janela. Fica dentro do eframe e do winit 0.30, sem `unsafe`: sem o `WM_NCHITTEST`, perdem-se o Snap Layouts no Maximizar e o menu do sistema no clique direito do título (§5). O winit como dependência direta e o subclassing da janela ficaram de fora por decisão de 16/10/2026. |
| Toolchain **GNU** (MinGW-w64 da WinLibs) e não MSVC | Não exige Visual Studio nem admin. Ver §1. |

---

## 1. Toolchain e build

- Rust estável (rustup, host `x86_64-pc-windows-gnu`) em `%USERPROFILE%\.cargo` e `.rustup`; GCC 16 da WinLibs
  (MSVCRT, sem LLVM) em `%LOCALAPPDATA%\Programs\winlibs-gcc16\mingw64`.
- **Sempre pelo `cargo.ps1`** da raiz (põe as ferramentas no PATH e liga o intermediário de link):
  `.\cargo.ps1 test -p banco`, `.\cargo.ps1 build --release`. Ele roda de qualquer pasta.
- `ferramentas\vincular.rs`: o gcc da WinLibs usa página de código UTF-8 e o ld e o dlltool usam a ANSI (1252);
  um caminho com acento (`C:\Users\Conceição`) não chega certo a eles. O intermediário troca cada caminho
  com acento pelo nome curto 8.3 (ASCII) antes de chamar o gcc (como linker) e o dlltool (uma cópia chamada
  `dlltool.exe`, à frente no PATH). O `cargo.ps1` o compila na primeira vez e quando o fonte muda.
- Caminhos de compilação longos estouram o limite de 260 caracteres do Windows: a pasta `target` fica na raiz do
  projeto e o projeto não deve ficar numa pasta funda.
- Dependências compiladas com `opt-level = 2` e sem depuração mesmo em `dev` (SQLite e egui rápidos nos
  testes); os crates do projeto ficam com `debug = "line-tables-only"`.
- **Vários agentes ao mesmo tempo:** cada um compila na sua própria pasta (`--target-dir target\ag-<nome>`) para
  não esperar a trava do cargo, e **nenhum mexe em `Cargo.toml`/`Cargo.lock`** (os manifestos já declaram as
  dependências de cada crate; se faltar uma, ela só pode vir de `[workspace.dependencies]` e o relatório diz).
- Formatação: `rustfmt.toml` (largura 120). Lint: `.\cargo.ps1 clippy -p <crate> --target-dir target\ag-<nome>`
  sem avisos novos.

---

## 2. Crates

```
databox/
├─ Cargo.toml · Cargo.lock · cargo.ps1 · compilar.bat · rustfmt.toml · .gitignore · LICENSE
├─ README.md · README.pt-BR.md · ARQUITETURA.md (este) · CHANGELOG.md · CONTRIBUTING.md · RELEASE.md
├─ .github/workflows/             CI (compila, testa, clippy, fmt) e release (o DataBox.exe na página da versão)
├─ ferramentas/vincular.rs        intermediário de link (ver §1)
├─ design/                        guia visual (README.md), canvas, logo, ícones em SVG e geradores em Python
└─ crates/
   ├─ comum/          ErroBanco, Codigo, q(), afinidade(), traduzir_*; Valor, Celula, Chave, Entrada;
   │                  app (o nome DataBox e as pastas por pessoa); arquivos (os tipos de arquivo, o cabeçalho do
   │                  SQLite, os ocultos, a ordem natural e a Listagem de uma pasta, sem disco)
   ├─ amostras/       bancos de exemplo para testes (dados/exemplo.db, gerados uma vez com semente fixa)
   ├─ ddl/            tokenizador SQL, analisador de CREATE TABLE, ColunaDef
   ├─ esquema/        planos de mudança de estrutura (ALTER nativo e reconstrução)
   ├─ banco/          conexão, introspecção, páginas, busca/filtros, CRUD, consulta
   ├─ exportar/       Excel, CSV, JSON, .zip
   ├─ planilha/       leitura de .xlsx e .xlsm (só leitura): abas, células, índice e páginas
   ├─ tabular/        CSV: ler por páginas com o índice, as alterações por cima com desfazer, gravar atômico no
   │                  formato do arquivo; e o motor de copiar, colar e trazer colunas (casar)
   ├─ texto/          arquivos de texto e JSON: codificação e fim de linha que voltam como eram, gravação atômica,
   │                  os limites de tamanho e o analisador de JSON próprio (sem recursão, com posições)
   ├─ prefs/          preferências, recentes, histórico de SQL
   ├─ servico/        regras do banco aberto: sessão do banco, destinos, exportação acompanhada, a pasta
   ├─ interface/      visual Estúdio em egui: tema, fontes do Windows, logo, ícones em PNG, realce de SQL
   │                  (codigo), vidro fosco (fosco), peças (também as da casca: abas de documento, documento,
   │                  árvore, painel, divisória, dock, popover, barras de título, de ferramentas e de status),
   │                  formatação pt-BR
   ├─ estado/         estado do app, Rota, as abas de documento com o histórico único, os verbos da barra de
   │                  ferramentas, Acao, Promessa (tarefa de fundo), diálogos e o contrato das telas com a casca
   │                  (documento: os traits Documento e ParteTabela)
   ├─ tela_dados/     aba Dados: grade, busca, filtros, edição, janela da linha
   ├─ tela_esquema/   Estrutura, Índices e gatilhos, Definição SQL, Nova tabela, Novo índice
   ├─ tela_consulta/  Consulta SQL (várias ao mesmo tempo, com ids próprios)
   ├─ tela_exportar/  janela Exportar, com progresso e Interromper
   ├─ tela_csv/       o editor tabular de CSV (prancheta 7): grade paginada, editar, marcar, colunas, filtrar e
   │                  ordenar, copiar, colar e a janela Trazer colunas (pranchetas 15 e 16)
   ├─ tela_texto/     o editor de texto e de JSON (prancheta 8): números de linha, cores, validação, Formatar,
   │                  estrutura do documento; o leitor só leitura acima de 0,5 MiB
   ├─ tela_planilha/  a planilha do Excel só leitura (prancheta 17): faixa das planilhas, grade com letras,
   │                  mescladas e painéis congelados, índice em segundo plano, origem do Trazer colunas
   └─ editor/         o executável DataBox.exe (o `[[bin]]` do pacote editor): a casca (src/casca: barra de
                      título e janela sem decoração, menus, barra de ferramentas, barra de ícones, Explorador,
                      Histórico de consultas, faixa de abas, Propriedades, larguras dos painéis, barra de status,
                      atalhos, aviso de transação, Preferências, Sobre), os documentos das abas
                      (src/documento.rs), Início, Visão geral, o contêiner da tabela, linha de comando, recursos
                      (recursos/databox.rc: ícone, versão e manifesto; build.rs: a compilação e as licenças de
                      terceiros), registro (log)
```

Dependências (sem ciclos): `comum` ← `ddl` ← `esquema`; `comum` ← `banco`, `exportar`, `prefs`, `planilha`,
`tabular` ← `texto`; todos ← `servico` ← `estado` ← `tela_*` ← `editor`; `interface` (só `comum` e egui) ← `estado`,
`tela_*`, `editor`. As telas de arquivo falam com o núcleo delas direto, em tarefas (`tela_csv` → `tabular`,
`tela_texto` → `texto`, `tela_planilha` → `planilha`), sem passar pelo `servico`, que é a sessão de um banco.

O contrato entre as telas e a casca mora no `estado::documento` (§4.8): as telas, cada uma no crate dela,
implementam o trait `Documento` (ou o `ParteTabela`, as partes de uma tabela), e a casca do `editor` só despacha.
Como o `estado` conhece o `banco` e o `interface`, é ele que guarda o que as telas de crates diferentes repetiriam: a
seção "Tabela" das Propriedades, o "Abrir na Consulta SQL", a busca da barra e o ícone de cada tipo de tabela.

---

## 3. Convenções (valem para todo crate)

1. **Português** nos nomes, comentários, documentação e testes (frases inteiras). As mensagens para a pessoa são
   frases completas em português do Brasil.
2. Textos de um conjunto fechado (`"table" | "view"`, operadores de filtro, ações de manutenção, formato) viram
   `enum` com `como_str()` devolvendo o texto estável; ausente vira `Option`.
3. **Erros** sempre como `comum::ErroBanco`, com `status` (400 regra, 403 só leitura, 404 não existe, 409 conflito,
   500 inesperado, 501 indisponível) e, quando a interface precisa reagir, um `codigo` estável.
4. **Valores** pelos tipos de `comum`: o que está no arquivo é `Valor`; o que a grade mostra é `Celula`
   (texto cortado em `LIMITE_TEXTO` caracteres, BLOB resumido); identidade de linha é `Chave`; o que a pessoa
   manda gravar é `Entrada` (a coerção pela afinidade da coluna é `banco::coagir`). Inteiros são `i64`, texto
   inválido é `Valor::TextoBruto`.
5. **Nada de pânico com dado da pessoa ou do arquivo**: sem `unwrap`/`expect`/índice que possa estourar fora
   de testes; erro vira `ErroBanco`. `Mutex` envenenado não derruba o app (recupere o conteúdo).
6. **Concorrência.** `Banco`, `Preferencias` e `Servico` são `Send + Sync` e usados por `Arc` de várias threads.
   A thread da interface **nunca** toca no SQLite nem no disco: tudo vai por `estado::Promessa` (thread de
   fundo que acorda a interface ao terminar).
7. **Testes** usam `amostras` para o banco de exemplo e uma pasta temporária própria (`std::env::temp_dir()` +
   nome único; apague no fim). Nunca o `%APPDATA%` real.
8. `#![forbid(unsafe_code)]` em todo crate, exceto onde a FFI do SQLite for indispensável (`sqlite3_complete`
   em `banco::dividir_comandos`, por exemplo), com comentário `// SAFETY:`.
9. As telas nunca usam cor solta nem desenham peça própria quando o crate `interface` já tem uma.

---

## 4. Contratos por crate

**A API de verdade de cada crate está documentada no topo do `lib.rs` dele, e vale ela.** O que segue é o que as
outras partes precisam saber:

- `banco`: `parar` é `Option<Arc<Parar>>`; `contar_para_exportar(…) -> Resultado<Option<u64>>`;
  `Filtro { coluna, op: Operador, valor: Entrada }` (com `Filtro::texto(coluna, op, "digitado")`);
  `LinhaGravada.linha: Option<Vec<Celula>>`; o iterador da exportação é `LinhasExportacao`; um erro no meio da
  leitura (inclusive pelo `parar`) chega traduzido como "Não foi possível ler os dados para exportar." e quem
  cancelou troca por `Codigo::Cancelado`.
- `esquema`: o plano da prévia não é guardado; para executar, o serviço monta o plano de novo e executa na
  mesma chamada, sob a mesma trava (`Banco::com_conexao`). `plano_criar_tabela` recebe as restrições como
  fatia tipada (`&[String]` ou `&[&str]`).
- `exportar`: grava o `.xlsx` (SpreadsheetML) em fluxo pelo crate `zip`, sem `rust_xlsxwriter` (conferido no
  Excel 16 e no openpyxl). Cancelar = um callback devolver `ErroBanco` 409 `Codigo::Cancelado`, que volta igual.
- `prefs`: todo getter pode tocar no disco (e `recentes()` abre threads): chame fora da thread da interface e
  guarde em cache. `modificado_em` usa as regras de horário de verão da época (como o Explorer).
- `interface`: `tema::aplicar` antes do primeiro quadro; a grade da aba Dados se monta sobre
  `grade::NucleoGrade`; movimento reduzido do Windows vem do editor por `tema::definir_movimento_reduzido`.

### 4.1 `ddl`

`Token`, `tokenizar`, `significativos`, `desfazer_aspas`, `analisar_create_table → DefTabela`,
`DefColuna`, `montar_coluna(&ColunaDef) → String`, `montar_create_table`, `colunas_da_expressao`,
`colunas_da_restricao`, `colunas_gravadas_pelo_gatilho`. `ColunaDef` mora aqui:

```rust
pub struct ColunaDef { pub nome: String, pub tipo: String, pub nao_nulo: bool, pub unica: bool, pub pk: bool,
    pub autoincremento: bool, pub padrao: Option<String>, pub verificacao: Option<String>,
    pub colacao: Option<String>, pub referencia: Option<Referencia>, pub gerada: Option<Gerada>,
    pub extra: Option<String> }
pub struct Referencia { pub tabela: String, pub coluna: Option<String>, pub ao_excluir: Option<String>, pub ao_atualizar: Option<String> }
pub struct Gerada { pub expressao: String, pub armazenada: bool }
```

### 4.2 `esquema`

Todas as funções recebem `&rusqlite::Connection` (autocommit) e devolvem `Resultado<Plano>`; nada é
executado até `executar(conn, &plano)`. `Plano { descricao, comandos, reconstroi, avisos }` + `sql()`
(o `ja_havia` fica privado). `definicoes_tabela(conn, tabela) → Definicoes { colunas: Vec<ColunaDef>,
restricoes: Vec<String>, sem_rowid, estrita }`. Os ensaios deixam o arquivo byte a byte igual (BEGIN …
ROLLBACK; SAVEPOINT dentro de transação aberta).

### 4.3 `banco`

```rust
pub struct Banco { … }   // Send + Sync; a conexão principal fica atrás de uma trava interna
impl Banco {
    pub fn abrir(caminho: &Path, somente_leitura: bool, criar: bool) -> Resultado<Banco>;
    pub fn caminho(&self) -> &Path;  pub fn nome(&self) -> &str;  pub fn id(&self) -> &str;
    pub fn somente_leitura(&self) -> bool;  pub fn motivo_somente_leitura(&self) -> Option<&str>;
    pub fn transacao_aberta(&self) -> bool;  pub fn exigir_sem_transacao(&self, acao: &str) -> Resultado<()>;
    pub fn e_arquivo_do_banco(&self, caminho: &Path) -> bool;  pub fn fechar(&self);
    pub fn com_conexao<R>(&self, f: impl FnOnce(&Connection) -> Resultado<R>) -> Resultado<R>; // para o esquema e o serviço
    pub fn conexao_leitura(&self) -> Resultado<Connection>;
    pub fn info(&self) -> Resultado<BancoInfo>;          pub fn objetos(&self) -> Resultado<ObjetosInfo>;
    pub fn contagem(&self, tabela: &str, busca: Option<&str>, filtros: &[Filtro], leitura: Option<&str>) -> Resultado<Contagem>;
    pub fn tabela_info(&self, nome: &str) -> Resultado<TabelaInfo>;
    pub fn linhas(&self, nome: &str, pedido: &PedidoPagina) -> Resultado<Pagina>;
    pub fn linha(&self, nome: &str, chave: &Chave) -> Resultado<LinhaCompleta>;
    pub fn inserir(&self, nome: &str, valores: &[(String, Entrada)]) -> Resultado<LinhaGravada>;
    pub fn atualizar(&self, nome: &str, chave: &Chave, valores: &[(String, Entrada)]) -> Resultado<LinhaGravada>;
    pub fn excluir(&self, nome: &str, chaves: &[Chave]) -> Resultado<usize>;
    pub fn ler_blob(&self, nome: &str, chave: &Chave, coluna: &str) -> Resultado<Option<Vec<u8>>>;
    pub fn consultar(&self, sql: &str, limite: usize) -> Resultado<ResultadoConsulta>;
    pub fn interromper_consulta(&self) -> bool;   // corta o `consultar` em andamento ou na fila
    pub fn interromper_consulta_detalhado(&self) -> Interrupcao;   // o mesmo: Nenhuma | EmAndamento | NaFila
    // leitura em fluxo para a exportação: o iterador só vive dentro do `consumir` (a trava e o comando
    // preparado ficam vivos enquanto ele roda)
    pub fn iterar_tabela<R>(&self, nome: &str, filtro: &FiltroExportacao, parar: Option<&Parar>,
        consumir: impl FnOnce(&[String], &mut dyn Iterator<Item = Resultado<Vec<Valor>>>) -> Resultado<R>) -> Resultado<R>;
    pub fn iterar_consulta<R>(&self, sql: &str, parar: Option<&Parar>, consumir: …) -> Resultado<R>;
    pub fn contar_para_exportar(&self, nome: &str, filtro: &FiltroExportacao, parar: Option<&Parar>, prazo: Option<Duration>) -> Option<u64>;
    pub fn manutencao(&self, acao: AcaoManutencao) -> Resultado<ResultadoManutencao>;
    pub fn copia_de_seguranca(&self, destino: &Path) -> Resultado<CopiaFeita>;
}
pub type Parar = dyn Fn() -> bool + Send + Sync;
pub struct Filtro { pub coluna: String, pub op: Operador, pub valor: String }   // 14 operadores
pub fn dividir_comandos(sql: &str) -> Vec<String>;
pub fn coagir(entrada: &Entrada, afinidade: Afinidade) -> Resultado<Coagido>;  // Coagido = Valor | Padrao | Manter
```

`leitura: Some("grade:<tabela>")` / `Some("contagem:<tabela>")` liga a **leitura substituída**: a
mais nova de um grupo abandona a mais velha (interrupt do SQLite só nela), que devolve `Codigo::Substituido`.
`Pagina.linhas[i].chave: Option<Chave>`, `valores: Vec<Celula>`; `LinhaCompleta.valores` traz o texto inteiro
(`limite None`) e o BLOB resumido. `ResultadoConsulta { resultados: Vec<ResultadoComando>, alterou_esquema,
alterou_dados, transacao_aberta }`, com `ResultadoComando::{Linhas{…}, Alteracao{…}, Erro{…}}`.

**Interromper a Consulta SQL.** `interromper_consulta()` é chamado de outra thread e
nunca espera a trava da conexão (no serviço, `Servico::interromper_sql(banco_id) → Resultado<bool>` e
`interromper_sql_detalhado(banco_id) → Resultado<Interrupcao>`, que só passam pelo `banco_aberto`; e
`interromper_sql_do_aberto()`, que o executável chama ao fechar a janela, `editor::interromper_ao_sair`). Cada
`consultar` cria a marca dele (`Arc<AtomicBool>`) e, enquanto espera a vez na conexão, a deixa na fila
(`consultas_na_fila`, na ordem de chegada; um guarda a tira ao sair). De posse da conexão (dentro do `com_conexao`),
ele passa a marca da fila para `parada_consulta: Mutex<Option<Arc<AtomicBool>>>` com um progress handler que a
confere a cada 20.000 passos do SQLite, confere a marca antes de cada comando e desliga os dois num `Drop`, sob o
mesmo `Mutex`, antes de soltar a conexão. `interromper_consulta_detalhado` devolve o que alcançou
(`banco::Interrupcao`):

- `EmAndamento`: um `consultar` tem a conexão; a marca dele é ligada e o `InterruptHandle::interrupt` chamado **só
  com a marca presente e segurando o `Mutex`**. Assim o interrupt nunca alcança a próxima operação na conexão
  (grade, Estrutura, exportação, outra consulta); um pedido que chegue com nenhum comando rodando o SQLite zera ao
  começar o próximo, e o progress handler cobre esse instante.
- `NaFila`: nenhum tem a conexão (ela está com uma gravação, uma exportação, um VACUUM), mas um espera a vez: só a
  marca do mais antigo da fila é ligada, sem interrupt (a conexão não é dele), e ele sai com o primeiro comando
  interrompido, sem executar nada, quando a pegar. A Consulta SQL avisa num toast de atenção ("A consulta ainda
  espera o banco ficar livre.").
- `Nenhuma`: nenhum `consultar` em andamento nem na fila; nada é feito (`interromper_consulta` devolve false só
  aqui).

O comando cortado volta como `ResultadoComando::Erro` com `MSG_INTERROMPIDA` ("A consulta foi interrompida.";
`detalhe` com o erro do SQLite quando cortado no meio, `None` quando a marca chegou antes de ele começar), os
seguintes não rodam e o roteiro não entra no histórico (tem erro). Os anteriores ficam como estão; um
INSERT/UPDATE/DELETE cortado dentro de um BEGIN pode levar o SQLite a desfazer a transação inteira
(`transacao_aberta` diz como ficou). Na tela, o Interromper é o verbo `Verbo::Interromper` da barra (o `.ib.risco`
logo depois do Executar, no mesmo lugar em toda aba, ligado só com a Consulta executando) e o Esc.

### 4.4 `exportar`

Gravação **empurrada** (quem tem as linhas chama o `Escritor`):

```rust
pub enum Formato { Xlsx, Csv, Json }
pub struct Opcoes { pub cabecalho: bool, pub csv_separador: char, pub csv_decimal: char, pub json_indentado: bool, pub csv_proteger_formulas: bool }
pub struct Resumo { pub caminho: PathBuf, pub formato: Formato, pub linhas: u64, pub bytes: u64, pub tabelas: usize, pub tempo_ms: u64, pub avisos: Vec<String> }
pub struct Escritor { … }   // grava num temporário ao lado do destino; `concluir` troca no fim; Drop sem concluir apaga
impl Escritor {
    pub fn novo(formato, destino: &Path, opcoes, varias: bool, meta: Option<Meta>, acomp: Acompanhamento) -> Resultado<Escritor>;
    pub fn comecar_tabela(&mut self, nome: &str, colunas: &[String]) -> Resultado<()>;
    pub fn linha(&mut self, valores: &[Valor]) -> Resultado<()>;
    pub fn terminar_tabela(&mut self) -> Resultado<()>;
    pub fn concluir(self) -> Resultado<Resumo>;
}
pub struct Acompanhamento { ao_progresso, ao_tabela, ao_finalizar }  // callbacks que podem devolver Err para cancelar
pub fn exportar_linhas(…) -> Resultado<Resumo>;  pub fn exportar_varias(…) -> Resultado<Resumo>;   // atalhos sobre o Escritor
pub fn destino_final(formato, destino, varias) -> PathBuf;  pub fn nome_planilha(nome, usados) -> String;
```

Um callback que devolve `Err` interrompe: o temporário é apagado, o destino fica como estava e o erro sobe
**como veio** (é assim que a exportação é cancelada).

### 4.4b `planilha` (só leitura)

Núcleo sem tela (o contrato inteiro está no topo do `lib.rs`): `Pasta::abrir(caminho)` lê só as partes pequenas
(abas, textos compartilhados, estilos) e é `Clone + Send + Sync`; `pasta.planilha(i)` dá a `Planilha` de uma aba, com
`indexar(|&Progresso| -> bool)` (uma passada sem copiar texto: marcos de posição de linha, última linha, colunas,
células mescladas), `tamanho()`, `meta()` e `pagina(primeira_linha, quantidade) -> Pagina` (linhas do Excel sem
buracos, com cache de blocos e cursor que continua de onde parou); `pasta.linhas(i)` é o iterador em fluxo. As
células saem tipadas (`Valor::{Vazio, Texto, Numero, Booleano, Erro, Data, Hora, DataHora}`; data/hora pelo formato
do estilo, sistemas 1900 e 1904) e `pasta.exibir(&celula)` escreve como o Excel (pt-BR). `.xls`, `.xlsb` e arquivo com
senha dão erro em português (`ErroBanco`). O XML é lido por um leitor "pull" próprio, em fluxo (`xml.rs`), e o zip é
descomprimido numa linha de execução à parte (`fluxo.rs`).

### 4.4c `tabular` (o CSV e o Trazer colunas)

Núcleo sem tela (o contrato inteiro no topo do `lib.rs`). `tabular::csv::Tabela::abrir(_com_progresso)(caminho,
&OpcoesAbertura)` detecta o formato (codificação UTF-8, UTF-8 com BOM ou Windows-1252 pela tabela própria; separador,
decimal, fim de linha, aspas e cabeçalho) e indexa o arquivo numa passada (um ponto a cada 64 registros); `pagina`,
`colunas_de` e as alterações (`editar_celula`, `inserir_linha`, `excluir_linhas`, `inserir_coluna(_com_valores)`,
`renomear_coluna`, `excluir_colunas`, `preencher_coluna`) vivem em `Edicoes`, por cima do arquivo, com `desfazer`/
`refazer`, `em_um_passo` (uma alteração só no desfazer) e, `alteracoes()` (os passos desde a abertura ou o
Salvar) e `marcas_da_pagina` (o `td.alt`). `salvar(ao_progresso)` grava num temporário ao lado, `sync_all` e `rename`
(200 ms de insistência), copiando byte a byte o que não mudou e escrevendo o que mudou no formato do arquivo; o erro do
`ao_progresso` cancela e apaga o temporário. Se o arquivo mudou no disco desde a abertura (tamanho ou data), a gravação
recusa: o núcleo copia as linhas intactas do arquivo de origem e não tem como regravar por cima do de outro programa
(por isso o CSV não tem "Salvar mesmo assim", decisão de 17/10/2026). `reabrir(&OpcoesAbertura)` (o Formato das
Propriedades) só sem alterações pendentes. `tabular::colunas` é o motor do copiar, colar e trazer colunas
(`ler_tsv`/`escrever_tsv`, `colunas_coladas`, `posicionar`, `casar` com as três opções de normalização, `sugerir_chaves`,
`atualizar_coluna`, `nome_da_coluna_trazida`); `tabular::numero`, o número e o tipo de um texto de célula.

### 4.4d `texto` (arquivos de texto e JSON)

`texto::arquivo::carregar(caminho) -> Carregado { texto (só `\n`), codificacao, fim_de_linha, fim_misto,
termina_em_quebra, bytes, modificado }` (as regras de codificação do `tabular`; UTF-16 e bytes nulos com a frase pronta;
acima de `LIMITE_LEITURA`, 32 MiB, recusado), `gravar(caminho, texto, &Carregado)` (o fim de linha dominante, o BOM e a
quebra final voltam como eram; caractere que o Windows-1252 não tem recusa com a linha, antes de tocar no disco) e
`gravar_atomico` (temporário, `sync_all`, `rename`; um link simbólico grava o arquivo para onde aponta), `mudou_no_disco`.
`texto::limites`: `LIMITE_EDICAO` 512 KiB (decisão de 18/10/2026, pela medição do `TextEdit`: ~50 ms por tecla a 0,5 MiB
e 150 a 200 ms a 1 MiB), `LIMITE_ESTRUTURA` 8 MiB, `max_undos(tamanho) = clamp(32 MiB / tamanho, 10, 100)`.
`texto::json`: o analisador próprio (RFC 8259 estrito, com pilha explícita: 100.000 níveis sem estourar), a árvore numa
arena com as posições de cada valor (`caminho_em(byte)`, a migalha), as frases de erro em português com linha e coluna
(16 motivos), `validar` (só o veredito, sem árvore) e `formatar(texto, Recuo)` (só espaços e quebras mudam: a ordem das
chaves, os números e as cadeias ficam). `texto::posicao`: linha e coluna em caracteres a partir do byte.

### 4.5 `prefs`

`Preferencias::abrir(arquivo: Option<PathBuf>)` (padrão `arquivo_padrao()`, `%APPDATA%\DataBox\preferencias.json`),
`tema()` (`Tema::{Sistema, Claro, Escuro}` ↔ `"system"|"light"|"dark"`), `exportacao()`, `ultimo_diretorio()`,
`recentes() → Vec<Recente>`, `adicionar_recente`, `remover_recente`, `limpar_recentes`, `historico_sql`, `registrar_sql`, `limpar_historico_sql`,
`atualizar(…)`, `resumo() → PrefsInfo`, mais `aparencia()` e `paineis()` (abaixo). JSON com `"versao": 1`, gravação
atômica, trava entre processos (`preferencias.json.lock`), arquivo corrompido recomeça do padrão, arquivo ilegível na
hora → a mudança fica só em memória. Duas instâncias do DataBox gravando juntas **se revezam** na trava: quem espera
"bate" a cada tentativa, mudando a data de modificação do `.lock` (a trava de faixa do Windows não impede), e quem
acabou de gravar, se viu a batida, espera 50 ms antes de travar de novo. Sem isso, uma rajada de mudanças de uma
deixava a outra de fora até o fim (ou até os 3 s do prazo, quando ela grava sem a trava); sem ninguém esperando,
nada atrasa. As pastas por pessoa vêm de `comum::app` (`NOME`, `PASTA`, `pasta_de_configuracao`, `pasta_de_estado`;
o específico do Windows fica em `cfg(windows)`). As chaves que esta versão não conhece (no arquivo e em cada recente)
são guardadas e regravadas depois das conhecidas, na ordem lida, sem mudar o formato. `adicionar_recente_de(caminho,
tipo)` leva as chaves desconhecidas do item de antes, menos um `"tipo"` que contradiz o novo; `adicionar_recente(c)`
é com `TipoRecente::Banco`.

**Recentes com tipo e a pasta aberta:** `Recente.tipo: TipoRecente::{Banco, Pasta, Arquivo}`
(`"banco"|"pasta"|"arquivo"`). O banco **não leva a chave `"tipo"`**; a pasta e o arquivo gravam `"tipo"` logo depois
de `"aberto_em"`. Sem `"tipo"` é banco, **exceto** se o caminho é uma pasta no disco; um `"tipo": "banco"` escrito é
respeitado. `recentes()` devolve os três tipos (`existe` da pasta = é pasta que respondeu a tempo; do banco e do
arquivo = é arquivo comum) e esconde, guardados, os de `"tipo"` desconhecido; `limpar_recentes` tira os três.
`pasta_aberta() → Option<String>` e `Mudanca.pasta_aberta: Option<Option<PathBuf>>` (None não mexe, `Some(None)`
apaga, vazio → 400 "A pasta aberta precisa ser um caminho."): a chave `"pasta_aberta"` vem depois de
`"larguras_paineis"` e **só existe enquanto há pasta aberta** (apagada no Fechar pasta).

**Aparência do código:** `aparencia() → Aparencia { cores_codigo: CoresCodigo::{DataBox, AltoContraste,
SolarizedEscuro, Monokai}, fonte_codigo: Option<String> /* None = Consolas */, tamanho_codigo: f32 }` (padrão
DataBox, Consolas, 12,5 px), `PrefsInfo.aparencia`, `Mudanca.aparencia: Option<MudancaAparencia>` (mesclada como a
exportação; tamanho fora de 8 a 24 px → 400 "O tamanho da fonte do código precisa ficar entre 8 e 24 px."). No
arquivo, a chave `"aparencia"` vem depois de `"historico_sql"`, `{"cores_codigo": "monokai", "fonte_codigo": null,
"tamanho_codigo": 12.5}`, e **só é gravada depois que a pessoa escolhe** (`aparencia_padrao_nao_vai_para_o_arquivo`).
Lida com valor estranho, cada campo volta ao padrão. `Servico::gravar_preferencias(Mudanca) → PrefsInfo` é o Salvar
das Preferências (tema, aparência e exportação de uma vez); o `gravar_prefs(tema, exportacao)` continua. A janela
Preferências (`editor/casca/preferencias.rs`) aplica cada mudança na hora, grava só o que difere do retrato da
abertura (a chave `aparencia` só entra depois de a pessoa escolher nela) e, no Salvar, esvazia os recentes e o
histórico se marcados.

**Larguras dos painéis laterais:** `paineis() → LarguraPaineis { explorador: Option<f32>, propriedades: Option<f32> }`
(None = a padrão da casca, 258 e 272 pt), `Mudanca.paineis: Option<MudancaLarguraPaineis>` (mesclada; fora de 120 a
2000 pt → 400 "A largura do painel precisa ficar entre 120 e 2000 pt."). No arquivo, a chave `"larguras_paineis"` vem
depois da `"aparencia"`, `{"explorador": 300, "propriedades": 272}` (pontos inteiros, só as que a pessoa mudou), e
**só é gravada na primeira mudança**. O `main.rs` lê `prefs.paineis()` e passa ao `Editor::com_larguras`; a casca
grava ao fim do gesto da divisória.

### 4.6 `servico`

`Servico::novo(prefs)`; os métodos recebem o `banco_id` que a tela tem (diferente do aberto →
`Codigo::BancoTrocado`). Abrir/novo/fechar com transação
aberta → `Codigo::TransacaoAberta` (a não ser com `descartar_transacao`); destinos de arquivo conferidos (o
próprio banco e `-journal/-wal/-shm`, os arquivos do app, `Codigo::DestinoExiste` sem `substituir`);
`tabela()` junta `TabelaInfo` com `definicoes`/`restricoes` do esquema; `esquema(operacao, previa)`; `sql()`
registra o histórico; `interromper_sql()` corta o `sql()` em andamento sem esperar a conexão (§4.3); `exportar(pedido, progresso: Option<Arc<Progresso>>)` com as fases `contando →
gravando → finalizando → concluida | cancelada | erro`, `Progresso::cancelar()` e leitura do andamento sem
nunca esperar a trava do banco.

**A pasta:** `servico::pasta`, todo o acesso a disco da árvore, sempre de uma tarefa de fundo e
sem nunca tocar na conexão do banco aberto. `alvo_do_caminho(caminho)` diz o que o caminho é (`Natureza`: `Pasta`,
`Banco` pelo cabeçalho `SQLite format 3\0` ou 0 byte com extensão de banco, `Dados(tipo)`; inexistente → 404, tipo que o
app não abre → 400 "O DataBox ainda não abre este tipo de arquivo: x."); `abrir_pasta` valida, lê a raiz, entra nos
recentes (tipo Pasta) e grava `pasta_aberta`; `listar_pasta(caminho, anterior)` lê um nível (pastas, bancos e dados em
ordem natural, ocultos do Windows, `~$` e pastas de ferramentas fora, atalhos de pasta seguidos, corte em 500, 20.000
entradas, 5 s e 2.000 cabeçalhos, com `parcial` quando algo passa disso e o cabeçalho dos bancos reaproveitado do
`anterior` se o tamanho e a data não mudaram); `fechar_pasta` apaga `pasta_aberta`; `registrar_arquivo` põe o arquivo
de dados nos recentes (tipo Arquivo) e `info_arquivo` diz o que a ficha mostra. O serviço não guarda a pasta aberta:
ela é uma noção da interface.

- **Caminhos**: `abrir_pasta` e `info_arquivo` devolvem o caminho resolvido como o do banco aberto (`Banco::abrir`):
  absoluto, com os nomes curtos 8.3 (`C:\Users\CONCEI~1\…`) e os links resolvidos, sem o `\\?\`; a casca compara a raiz,
  as entradas, o banco e a ficha pela `prefs::chave_caminho` desses caminhos. O `listar_pasta` mantém o caminho como
  veio (uma subpasta que é atalho continua o atalho): um banco aberto **por dentro** de um atalho de pasta tem o
  caminho do destino e aparece como a segunda raiz do Explorador (limitação conhecida).
- **Datas**: a entrada de uma pasta vem sem data (`modificado: None`, como o tamanho): o NTFS só atualiza a data de
  uma subpasta, na leitura da pasta de cima, depois que alguém a abre, e a releitura da volta do foco acharia mudança
  onde não há. Com isso, a releitura que não achou nada novo devolve uma `Listagem` igual à de antes, e a casca não
  troca o estado nem repinta.

### 4.7 `interface`

A API inteira está no topo do `interface/src/lib.rs`; aqui fica o que as outras partes precisam saber.

`tema`: paletas clara e escura com os tokens das pranchetas do visual Estúdio (antes, os de `tokens.css`),
`aplicar(ctx, tema)`, fontes (as do Windows, lidas em tempo de execução; a monoespaçada só nos editores de código e
no SQL das janelas: o `.mono` das pranchetas é a fonte da interface em 350). Peças com o mesmo papel das classes do
kit: botões (`normal`, `primario`, `ouro`, `ghost`, `perigo`, `ghost perigo`, `redondo`, `icone`, com `com_icone`,
`lista` e `ocupado`), `cartao`, `pill`, `tag`, `aviso`/`aviso_tom` (a caixa com a largura do texto até 560 pt,
centrada), `vazio`, `carregando`, `janela` (modal: cabeçalho com ícone, título, sub e ✕, corpo, rodapé; ✕/Esc/fundo
passam por um "pode fechar?"), `confirmar`, `toast`, `ajuda` (balão), `campo`, `check`, `radio`, `select`, `abas`,
`trilha`, `cabecalho`, `tiles`, `track` (seletor segmentado), `detalhes` (rótulo · valor), `barra_progresso`,
`texto_pre`, `zona_risco`, célula da grade (`Celula` → NULL, chip de BLOB, "…", números à direita em fonte tabular)
e uma grade só leitura com cabeçalho fixo e rolagem virtual (a da Consulta SQL). Formatação pt-BR: `formatar_bytes`,
`partes_bytes`, `formatar_numero`, `formatar_data`, `formatar_ms`, `plural`, e `sem_acento` (a chave das buscas e
dos filtros, a única). Um **mostruário** (exemplo e teste com captura em PNG pelo `egui_kittest`) mostra cada peça
nos dois temas. As peças da casca: `abas_documento`, `arvore`, `painel`, `barra_status`, `barra_titulo`,
`ferramentas` e os itens do `PainelMenu` (desligado, marcado, submenu, item de duas linhas, com ícones).

**Ícones:** `icones::Icone` (as silhuetas da barra de ferramentas e dos botões, os sólidos do
trilho, do menu Arquivo, do banco, dos objetos e das partes, os `-grande` de 48 da dock, `Tipo(TipoColuna)` — a
pastilha pelo tipo declarado, `TipoColuna::do_declarado(tipo, pk, fk)` —, `Folha(TipoArquivo)` e
`Estado(EstadoIcone)`, e os traços desenhados no pintor) são os PNG embutidos de `assets/icones` (1×, 1,5× e 2×, mais
1,25× nas silhuetas), com a tabela `icones/png.rs` gerada por `design/ferramentas/icones_app.py --tabela` e indexada
pela chave. O PNG sai pelos pixels da tela (o menor que cobre, `escolher_png`) e o desenho pelos pontos (o glifo da
pastilha, `glifo_do_tipo`); o texto da pastilha e da folha vai por cima. `icones::pintar`, `pintar_apagado` (o
`grayscale` do item desligado) e `pintar_arco` (o ocupado); as texturas ficam num cache por chave, lado e cinza.

**Documento:** `pecas::documento` é a peça do `.doc` (fundo `--c`, borda `--vidrob`, raio 10),
com API sequencial: `Documento::abrir(ui, rect, juncao, com_rodape)` pinta a moldura e, na ordem, `aviso`/
`aviso_texto` (o `.aviso` de ponta a ponta no alto: a troca de banco, a transação), `faixa` (42 pt, `ALTURA_FAIXA`),
`pagina(ui, Pagina, …)` (a página que rola, `padding:18px 20px 20px`, com a rolagem guardada por aba e o
PageUp/PageDown/Home/End da casca) ou `propria(ui, …)` (o retângulo inteiro, para a grade e o editor), e `rodape`
(34 pt, `ALTURA_RODAPE`). `de_ponta_a_ponta` tira o recuo da página para uma lista que vai de borda a borda.

**Vidro fosco (revisão de 17/10/2026):** `fosco::ligar(ctx, aviso)` (no criador do app) faz toda camada
que se sobrepõe — menus e submenus, popovers, a lista dos selects, as janelas e o toast — pintar embaixo do `--flut`
(78% no escuro, 80% no claro) a foto da própria janela, tirada pelo `ViewportCommand::Screenshot`, reduzida, desfocada
(a gaussiana de 24 pt do `blur(24px)`) e saturada a 150%. A peça usa `pecas::campos::forma_vidro_fosco(painter, p,
rect, raio)` (ou `pintar_vidro_flutuante`); o fundo serve enquanto alguma camada estiver aberta e mais 2 s depois de
fechar tudo. Sem o fosco ligado (os testes de tela), com a foto que não chegou em 0,5 s ou que veio duas vezes de
outro tamanho, o vidro é o `--flut` liso a 96% (`ALFA_VIDRO_LISO`). As dicas (as do egui e o balão de ajuda) ficam
sempre no liso, `forma_vidro_flutuante`: uma foto de tela cheia a cada dica custaria mais que a dica vale. Parado,
nada repinta; só a espera da foto pede quadros (dois, em geral).

**Outras peças:** `pecas::divisoria::Divisoria` (a alça invisível no vão de 8 pt entre um painel lateral e o
documento: o fio de 2 pt em `--acc` no hover, no arrasto e no foco, o cursor de redimensionar, duplo clique ou Enter
volta à padrão, as setas em passos de 16 pt, Home e End aos limites, o papel `Splitter`; devolve a largura e
`concluiu`, a hora de guardar); `pecas::dock::dock(ui, &[ItemDock])` (os contadores da Visão geral: os ícones
`-grande` de 48 pt em pé numa prateleira, com sombra de contato e reflexo, o número 24/300 e o `.tit`; seis colunas ou
3 + 3 pelo conteúdo; parada, sem animação de hover, pedido de 17/10/2026); `pecas::popover::Popover` (o
vidro fosco preso embaixo de uma âncora, com o conteúdo de quem chama, por cima do documento; fecha com o Esc ou o
clique fora; a lista de um select de dentro fecha antes); `toast_concluido_com_acao(ctx, mensagem, dica, AcaoToast)`
(o toast com um botão pequeno dentro, como o "Mostrar na pasta" da cópia de segurança; o clique sai em
`acao_do_toast(ctx)`, que a casca lê a cada quadro, e o toast some); `texto::atalho` (o `a.atalho` das Propriedades),
`texto::titulo_lista` (o título 13/600 das listas do documento), `texto::galley_caminho` (a reticência no meio da
pasta), `rotulos::chip_filtro` e `etiqueta_mais`, `Entrada::pastilha`/`somente_leitura`/`fixo`/`pequeno`, e
`Janela::icone`/`sub`/`largura`.

**Código:** `interface::codigo` tem as 147 palavras reservadas do SQLite (saíram do
`tela_consulta`, que passa a usar a lista daqui), um léxico próprio (`trechos_sql`: palavras-chave, textos,
identificadores citados, números, comentários `--` e `/* */`, pontuação; nunca perde um byte) e `realcar_sql`, sem
depender do `ddl` (o `interface` continua só com `comum` e o egui). As quatro "cores do código" (`CoresCodigo`,
espelho do `prefs::CoresCodigo`) resolvem para `CoresResolvidas { fundo, texto, kw, st, nu, pu }` com os valores da
prancheta 13; a DataBox segue o tema (`--c`, `--t`, `--kw`, `--st`, `--nu`, `--t3`). A `AparenciaCodigo { cores,
familia, tamanho }` mora no contexto (`codigo::definir`/`atual`) e vale na hora para o `AreaTexto::codigo` (o
editor da Consulta; com `numeros_de_linha`, `marcar_linhas` e `sublinhar` para a Consulta) e o `TextoPre::sql` (o SQL
das janelas, dos gatilhos e da definição). A família vem de `fontes::familias_mono_disponiveis` (Consolas, Cascadia
Mono, Courier New, Lucida Console, só as que existem) e troca por `fontes::trocar_mono` (`Context::set_fonts` sobre
as definições do `instalar`, com a escolhida à frente da cadeia monoespaçada); o arquivo é lido por quem chama. A
Consolas 12,5 px é o padrão. O `editor::aplicar_aparencia` aplica a aparência das preferências ao subir, ao lado
do tema (main.rs, antes do primeiro quadro).

### 4.8 `estado`, telas e `editor`

`estado` (API no topo do `lib.rs`): `Promessa` (tarefa de fundo), `Rota`, `Parte` (Dados, Estrutura, Índices e
gatilhos, Definição SQL; `Aba` ficou como apelido, e "aba" agora é a aba de documento), `ConsultaDados`,
`Navegacao<T = Rota>` (com `atual_mut` e `reter`, que poda e funde as entradas vizinhas iguais), `Abas<T>` (as abas
de documento com o histórico único, sem egui), `Verbo`/`Oferta`/`Ofertas` (os verbos da barra de ferramentas),
`Acao` (pedidos das telas à casca), `Estado`, `Contexto` (o que cada tela recebe a cada quadro: serviço, estado, a
janela dona dos diálogos e o canal de ações), `Dialogos` (rfd), `TabelaAberta` (a tabela carregada pela casca e
entregue às abas) e, com o recurso `apoio-testes`, `apoio_testes::{Ambiente, palco, esperar, esperar_texto,
capturar}` para testar telas com o `egui_kittest` e gravar capturas em `capturas/rust/`.

**Abas** (`estado::Abas<T>`; a casca guarda um `Abas<Documento>`):
- `IdAba` (nunca se repete), `ChaveAba::de(&Rota)`: o Início, a Visão geral e cada parte de tabela ou visão pelo
  nome (`Parte { nome, parte }`, sem o tipo: a tela corrige o tipo por `Substituir`) e a ficha de um arquivo de dados
  (`Arquivo(chave)`, a `prefs::chave_caminho`: o mesmo arquivo cai na mesma aba; `Rota::Arquivo` não exige banco); a
  Consulta SQL não tem chave.
  `Entrada { aba, rota }` é a entrada do histórico único; `AbaAberta { id, chave, rota, numero_consulta, conteudo }`.
- `abrir(rota, criar) -> Abertura::{Nova, Existente}`: com a chave de uma aba aberta, foca essa aba **sem trocar o
  endereço dela** (a página, a busca e os filtros ficam); `Consulta { sql: Some }` sempre abre outra; `Consulta {
  sql: None }` foca a usada mais recentemente (ou cria). `nova_consulta` numera pelo menor número livre (fechada a 2,
  a próxima é a 2).
- `substituir(id, rota)` é o `replaceState` daquela aba (no histórico, também a entrada atual, se for dela, e as
  entradas dela com o endereço de antes); `atualizar(id, rota)` muda a rota e, se a aba é a ativa, põe uma entrada
  nova, sem refazer nada. Se a chave nova é a de outra aba aberta, as duas viram uma e a fechada volta para a casca.
- `fechar(id, criar_inicio)` devolve o documento (para a casca soltar uma execução em andamento) e poda o histórico;
  a nova ativa é a da entrada anterior, senão a vizinha da direita, depois a da esquerda; fechar a última abre o
  Início, e o Início sozinho não fecha. `fechar_onde`, `reiniciar` (a troca de banco: fecha todas, inclusive as
  Consultas, e zera o histórico; `reiniciar_menos(rota, criar, manter)` faz o mesmo mantendo as abas de arquivo de
  dados), `voltar`/`avancar` (só focam a aba da entrada: nunca refazem),
  `vizinha(seguinte) -> Option<IdAba>` (a seguinte ou a anterior na faixa, com volta, sem mudar nada: o Ctrl+Tab
  foca a que ela diz), `ativa`, `get`, `achar(&ChaveAba)`, `iter`.

**Ações** (`estado::Acao`; a casca marca cada ação com a aba que a emitiu e aplica no fim do quadro):
- `Navegar(rota)` quer dizer **abrir ou focar** a aba dessa rota (as telas que já navegavam não mudaram); a Consulta
  SQL sem comando foca a que abriu a transação, depois a mais recente. Sem banco, toda rota vira o Início.
- `Atualizar(rota)`: entrada nova no histórico, na mesma aba, sem refazer a tela (voltar a ela só foca a aba).
  `Substituir(rota)`: o endereço da aba de origem, sem histórico. As duas valem para a aba que as emitiu, mesmo que
  ela já tenha saído de vista.
- `Voltar` e `Avancar` andam no histórico único; novas: `NovaConsulta { sql }` (o "Nova consulta" da barra e da
  faixa; com o comando, um item do Histórico), `FocarAba(IdAba)` (o clique na faixa) e `FecharAba(Option<IdAba>)`
  (o ×, o clique do meio e o Ctrl+W; None é a ativa).
- A pasta: `EscolherEAbrirPasta`, `AbrirPasta { caminho }`, `AbrirCaminho { caminho, somente_leitura }` (a casca
  classifica: pasta, banco ou arquivo de dados), `AbrirArquivoDeDados { caminho }` (vai para `Rota::Arquivo`),
  `ListarPasta { caminho }` (uma subpasta da árvore), `AtualizarPasta` e `FecharPasta` (fecha as abas de arquivo de
  dentro dela; o banco fica). O resultado das leituras mora em `Estado.pasta: Option<PastaAberta>` (`nos` por chave de
  caminho: `Lendo`, `Pronto(Arc<Listagem>)` ou `Falhou(frase)`), independente do banco aberto.

**Verbos** (`estado::Verbo`, na ordem fixa da barra, `Verbo::TODOS`: Salvar, Novo, Nova coluna, Duplicar, Excluir,
Desfazer, Refazer, Buscar, Filtrar, Atualizar, Executar, Interromper, Exportar; `rotulo()`, `atalho()` e `icone()`):
cada aba diz o que oferece num `Ofertas` (o padrão desliga tudo), com uma `Oferta { ligado, marcado, principal,
ocupado, rotulo, dica }` por verbo ("Nova tabela", "Exportar clientes"). Quem decide são as próprias
telas (`Documento::ofertas` e `Documento::executar`, abaixo); o editor só despacha. O menu Editar sai das mesmas
`Ofertas` (menos o Salvar e o Interromper). Todo verbo tem o lugar dele na barra, ligado ou não; o Interromper (passo
14, prancheta 15 N3) fica logo depois do Executar e só liga, como `.ib.risco`, com a Consulta executando, e a
`Oferta.ocupado` põe o arco girando no Executar, que fica desligado; o Esc também interrompe.

**O contrato das telas** (`estado::documento`; a API completa no topo do
`estado/src/lib.rs`). Cada aba é um documento que implementa o trait `estado::Documento`, e as quatro partes de uma
tabela ou visão implementam o `estado::ParteTabela`, o mesmo contrato com a `&mut TabelaAberta` que a tela de tabela
carrega e empresta:

```text
trait Documento     anatomia() -> Anatomia { faixa, rodape, rolagem: Rolagem::{Pagina, Propria} }
                    avisos(doc, ui, ctx)   os .aviso de ponta a ponta antes da faixa (a transação da Consulta)
                    faixa(ui, ctx) · conteudo(ui, ctx) (o único obrigatório) · rodape(ui, ctx)
                    ofertas(&Estado) -> Ofertas · executar(Verbo, &egui::Context, ctx)   (antes do conteúdo)
                    busca() -> Option<BuscaDaBarra { texto: &mut String, dica }> · evento_busca(EventoBusca::{Mudou,
                        Enter, Limpar}, …)
                    tem_propriedades() · propriedades(ui, ctx) · status(&Estado) -> Option<String> · ocupada()
                    em_segundo_plano(&egui::Context, ctx)   (a cada quadro, com a aba fora de vista)
trait ParteTabela   o mesmo, com a TabelaAberta em cada um (ofertas(&TabelaAberta, &Estado)); sem tem_propriedades
                    nem status (os da tela de tabela); mais acoes_da_tabela(ui, ctx, &TabelaAberta), o fim da seção
                    "Tabela" (o "Renomear a tabela" da Estrutura)
desenhar_parte_empilhada(parte, ui, ctx, tabela)   faixa, conteúdo e rodapé um embaixo do outro, sem a casca (o
                    `ui` de sempre das partes e o que os testes usam)
secao_tabela(ui, tabela, depois) · secao_abrir_na_consulta(ui, ctx, nome) · consultas_da_tabela(nome)
tipo_da_tabela(info) · icone_da_tabela(tipo, sem_rowid) · propriedades_no_fim(ui, secoes)
busca_da_barra(ui, Option<BuscaDaBarra>, aba) · id_busca_da_barra(aba) · focar_busca_da_barra(ctx, aba, texto)
```

Quem implementa: o Início, a Visão geral e a tela de tabela (no `editor`), a `TelaConsulta` (`tela_consulta`) e, como
`ParteTabela`, `AbaDados` (`tela_dados`), `AbaEstrutura`, `AbaIndices` e `AbaDefinicao` (`tela_esquema`). O enum
`editor::Documento` continua dono do ciclo de vida das abas (o título, o ícone, `serve_para`, `refeito`,
`ao_fechar`, `execucao_ao_fechar`, `marcar_para_reler`, `esquecer_transacao`) e despacha o resto por `doc()` /
`doc_ref()`. A tela de tabela passa à parte a anatomia, a faixa, o conteúdo e o rodapé, junta o Atualizar e o Exportar
às ofertas dela e desenha, nas Propriedades, a seção "Tabela" (com as `acoes_da_tabela` no fim) antes das seções da
parte e o "Arquivo" curto depois. O `Recursos` e a `Saida` do editor saíram: as telas pedem tudo por `Acao` (com o
`EscolherCopia`, o diálogo da cópia de segurança, que a casca abre e devolve à Visão geral), e o `Contexto` ganhou
`versao` e `trocando` (`com_casca`). Com o painel de Propriedades escondido (a janela estreita ou o Ctrl+Alt+B), as
telas de rolagem de página recebem o mesmo `propriedades` no fim da página, depois do fio e do `.tit` "Propriedades"
(`estado::propriedades_no_fim`); a tela não faz nada para isso, e as de rolagem própria (Dados, Consulta)
ficam sem elas.

O que cada tela diz (a anatomia, a busca da barra e as Propriedades; os textos estão no topo do `lib.rs` de cada crate):

| Tela | Anatomia | Busca da barra | Propriedades |
|---|---|---|---|
| Início | página | desligada | nenhuma (o painel não aparece) |
| Visão geral | faixa, página | "Buscar tabelas" | Arquivo (com o Copiar caminho), Manutenção |
| Dados | faixa, rodapé, própria | "Buscar em clientes" | Tabela, Coluna (a célula ativa), Abrir na Consulta SQL |
| Estrutura | faixa, página | "Buscar colunas" | Tabela (com Renomear), Coluna (com Alterar), Excluir a tabela |
| Índices e gatilhos | faixa, página | desligada | Tabela, Índice (o selecionado) |
| Definição SQL | faixa, própria | desligada | Tabela, Abrir na Consulta SQL |
| Consulta SQL | faixa, rodapé, própria | "Buscar no editor" | Execução, Tabelas (com o Inserir) |
| CSV (`tela_csv`) | rodapé, própria (a faixa "Filtros" só com condição) | desligada | Formato, Arquivo, Coluna {nome} |
| Texto e JSON (`tela_texto`) | faixa, rodapé, própria | desligada | Estrutura do documento (JSON) ou Texto, Arquivo |
| Planilha (`tela_planilha`) | faixa, rodapé, própria (a faixa das planilhas entre a grade e o rodapé) | desligada | Arquivo, Planilha, Pasta de trabalho, Célula {A1} |

**Os documentos de arquivo (etapas 5 e 6).** O trait ganhou, todos com padrão: `alterado()` (o ponto `--acct` da aba,
que vira × no hover), `alteracoes()` (a contagem da pergunta ao fechar), `salvando()`, `tomar_erro_de_gravacao()`,
`status_do_arquivo(&Estado) -> Option<StatusDeArquivo>` (a barra de status do arquivo: o ponto, o nome, "Salvo",
"Alterado, não salvo", "Com erro" ou "Somente leitura", e à direita "CSV · UTF-8 · CRLF · Separador ;") e
`fonte_de_colunas() -> Option<Arc<dyn FonteDeColunas>>` (a origem do Trazer colunas). `estado::fonte::FonteDeColunas`
(`Send + Sync`: rótulo, tipo, nomes e linhas baratos; `colunas(quais, ao_progresso)` lê disco, sempre numa tarefa; a
caixa "Primeira linha é o cabeçalho" da planilha por `com_primeira_linha_cabecalho`); a casca junta as das abas abertas
a cada quadro (uma `Arc` por aba) e as entrega em `Contexto::fontes()`. `estado::secao_arquivo` é a seção "Arquivo"
das Propriedades de todos (Caminho com o Copiar caminho, Tamanho, Modificado em, e as linhas de cada tela). O
`Verbo::TrazerColunas` entrou logo depois da Nova coluna (`Verbo::TODOS` com 14); só o CSV o liga.

A `editor::tela_arquivo::TelaArquivo` escolhe a tela pelo tipo do arquivo (`Rota::Arquivo` não mudou): CSV → `TelaCsv`,
JSON, TXT, MD e LOG → `TelaTexto`, `.xlsx` e `.xlsm` → `TelaPlanilha`; sem o recurso `telas`, a ficha do arquivo.
Além do trait, a casca usa de cada uma o `tomar_pedido` (os botões "Abrir no programa padrão" e "Mostrar na pasta" do
erro; da planilha, também o "Trazer colunas desta planilha"), o `pergunta_aberta` (o "Salvar mesmo assim" do texto) e,
do CSV, `trazer_colunas_de(rótulo)`.

As janelas que a casca abre ficam fora do trait, com o par de sempre (o `editor` liga os crates com o recurso `telas`,
padrão; sem ele, compila com os substitutos de `editor/src/substitutos.rs`, que implementam os mesmos traits):

```text
tela_esquema   JanelaNovaTabela::nova() · JanelaNovoIndice::nova(tabela, colunas)
                   · mostrar(&mut self, &egui::Context, &mut Contexto) -> bool   (false = fechou)
tela_consulta  TelaConsulta::nova(sql) · com_id(sal) (o id da aba: ids do egui próprios) · executar_agora() ·
                   executando() · soltar_execucao() -> Option<Promessa<ResultadoConsulta>> (ao fechar a aba) ·
                   esquecer_transacao() · interromper(&egui::Context, &mut Contexto) (numa tarefa, §4.3)
tela_exportar  JanelaExportar::nova(PedidoJanelaExportar) · mostrar(…) -> bool · exportando() -> bool
```

A Consulta mostra um resultado por vez pelo seletor `interface::pecas::navegacao::seletor`, e o trecho que a busca da
barra acha fica pintado (`AreaTexto::destacar`), porque o egui 0.36 reduz a seleção do editor ao cursor quando ele é
desenhado sem o foco. Os Dados editam as condições de filtro em etiquetas `chip_filtro` na faixa, num
`interface::pecas::popover::Popover`, e oferecem Nova linha, Duplicar (um alvo só), Excluir e Filtrar; com a aba
escondida (`em_segundo_plano`), as gravações e as tarefas da ficha que terminam dão o toast e notam a transação na
hora. A ficha da linha (`tela_dados::janela`) segue a prancheta 12a: `.ficha` em grade 250 | 1fr | 64 com a pastilha
de 24, as regras embaixo do nome, o valor, a caixa NULL (`aria-label` `coluna é NULL`) e o "Usar o padrão" dentro da
célula do valor; a só leitura (D9) mantém a coluna NULL desligada e leva a nota `TomAviso::Nota` e o rodapé "Copiar a
linha" · "Fechar".

---

## 5. Interface

- **Casca** (`editor/src/casca/`, a anatomia de `design/README.md`), de cima para baixo:
  - a **barra de título** de 30 px (`janela.rs`): o logo de 16 px, os menus (`menus.rs`, sem reticências) e o
    título no centro, esmaecido com a janela inativa; com a barra própria, os botões da janela (ver "Janela sem
    decoração");
  - a **barra de ferramentas** (`ferramentas.rs`, 46 px por fora): Voltar · Avançar | Abrir pasta
    (Ctrl+Shift+O) · Abrir arquivo (Ctrl+O) · Salvar | Novo · Nova coluna · Duplicar · Excluir · Desfazer · Refazer | a **busca da
    barra** (a do documento, `estado::Documento::busca`, com a dica dele: "Buscar tabelas", "Buscar em clientes",
    "Buscar colunas", "Buscar no editor"; desligada no Início, nos Índices e gatilhos e na Definição SQL; o Ctrl+F
    leva o foco a ela com o texto selecionado; mudança, Enter e Esc viram `EventoBusca`) · Filtrar · Atualizar |
    Executar · Interromper · Nova consulta · Exportar | o Painel de propriedades. Os verbos da aba vêm das `Ofertas`
    (§4.8): nada muda de lugar ao trocar de aba nem no meio de uma execução (o Interromper fica desligado e só liga,
    vermelho, com a Consulta executando), o que a aba não oferece fica desligado com a dica. O clique executa no
    documento antes de ele se desenhar;
  - o **corpo** (`padding:8px; gap:8px`): a barra de ícones de 52 px (`icones.rs`: Início, Explorador e Histórico de
    consultas, e no pé as Preferências), o painel da esquerda (o Explorador, `explorador.rs`: a pasta aberta, abaixo; sem
    ela, a árvore do banco aberto com Tabelas, Visões e Sistema, as fichas pelo tipo de cada tabela, as partes de cada uma, a parte da aba à vista
    marcada, o Atenção de 16 no banco com a transação aberta, Recolher tudo e o "Filtrar tabelas" acima de 12, e o
    "Tentar de novo" quando a leitura dos objetos falha; ou o Histórico de consultas, `historico.rs`: o clique
    seleciona, com o SQL inteiro na dica, e o duplo clique ou o Enter abrem uma Consulta com o comando), a faixa de
    abas de 35 px (`abas.rs`: o clique foca, o × e o clique do meio fecham, o "+" abre uma Consulta) com o
    **documento** da aba ativa embaixo e as **Propriedades** (`propriedades.rs`: as seções que o documento desenha,
    §4.8, com os rótulos "Seção Arquivo", "Seção Tabela"… para o leitor de tela; não aparecem no Início nem quando o
    documento ficaria com menos de 560 px, sem mudar a escolha da pessoa: o botão, o item do Exibir e o Ctrl+Alt+B
    ficam desligados). Mostrar ou esconder os painéis fica só na sessão;
  - a **pasta no Explorador** (pranchetas 1, 4 e 15): a raiz no nível 8 (aberta sozinha; o Recolher tudo a
    deixa aberta), as subpastas que abrem sob demanda (a primeira abertura emite `Acao::ListarPasta`, com
    "Carregando…" até a leitura chegar; a falha mostra o motivo e "Tentar de novo"; a vazia, "Nenhum arquivo que o
    DataBox abra."), os bancos (o aberto no lugar dele, em 600, com a subárvore de sempre um nível abaixo; os outros
    abrem pelo clique ou pelo Enter, com a troca de sempre, e o → neles não troca de banco), os arquivos de dados (a
    folha CSV, XLSX, JSON ou TXT; o clique abre a ficha) e a linha "mais N itens" depois de 500, sem foco. O banco
    aberto de fora da pasta é a segunda raiz, no alto; um banco ou uma ficha de dentro dela abre os pais. A dica da
    pasta diz o caminho, os arquivos que ela esconde e se a leitura parou antes do fim. O clique duplo numa pasta abre
    e deixa aberta. O id de cada nó leva a chave da pasta (`push_id`), e o banco aberto dentro dela, o dele. Sem pasta
    e sem banco, o vazio da prancheta 4 ("Nenhuma pasta aberta." e o botão "Abrir pasta"); fechar a pasta esquece os
    nós abertos dela;
  - o **menu de contexto** (`interface::pecas::menu_contexto`, prancheta 15: vidro fosco de 250, itens de 28): no
    clique direito ou no Shift+F10 (só ele, sem Alt nem Ctrl) com o foco no nó; arquivo e banco, Abrir · Mostrar na
    pasta · Copiar caminho (Ctrl+Shift+C); pasta, Atualizar · Mostrar na pasta · Copiar caminho; a raiz, também Fechar
    pasta, depois de um fio. ↑ e ↓ andam pelos itens, o Esc fecha e devolve o foco ao nó; o Ctrl+Shift+C com o foco no
    nó copia sem abrir o menu;
  - a **ficha do arquivo de dados** (`tela_arquivo.rs`, `Rota::Arquivo { caminho }`, `ChaveAba::Arquivo` pela chave do
    caminho): a faixa com a folha e o caminho, o aviso de que o editor ainda não existe, "Abrir no programa padrão"
    (primário) e "Mostrar na pasta", a seção Arquivo das Propriedades e o status "CSV · 96 KB"; nenhum verbo ligado. As
    trocas de banco mantêm as fichas (`Abas::reiniciar_menos`), e o Fechar pasta fecha as de dentro dela. Desde as
    etapas 5 e 6, a ficha só aparece sem o recurso `telas` e para o que não tem tela (os editores, abaixo);
  - os **editores de arquivo** (etapas 5 e 6, §4.8): o CSV, o texto e o JSON e a planilha do Excel no lugar da ficha. A
    aba alterada leva o ponto `--acct` ("vendas.csv, alterada" para o leitor de tela), que vira × no hover e no foco; a
    barra de status troca o banco pelo `StatusDeArquivo` da aba; Arquivo › Salvar (Ctrl+S) e a barra vêm das ofertas.
    **Ctrl+S** vale com um campo de texto focado (a célula em edição do CSV grava antes, por conta da tela) e não tira
    o foco; **Ctrl+Z**, **Ctrl+Y** e Ctrl+Shift+Z viram Desfazer e Refazer só sem campo de texto focado (com o
    `TextEdit` focado, o desfazer é o do egui, o mesmo `Undoer` que o texto usa); o F5 é o Atualizar da aba, e a tela
    pergunta antes de descartar ("Descartar as alterações de {nome} e ler o arquivo de novo?");
  - a **fila de perguntas ao fechar** (`FilaDeSalvar` no `app.rs`): o Ctrl+W, o ×, o clique
    do meio, o Arquivo › Fechar aba, o Fechar pasta e o pedido de fechar a janela (também minimizada) juntam as abas
    alteradas que vão fechar e perguntam uma a uma, com a aba à frente: "Salvar as alterações em {nome}?" com "Há 1
    alteração não salva. Se não salvar, ela se perde." e **Não salvar · Cancelar · Salvar** (Esc cancela). Salvar roda o
    verbo e espera a gravação (`salvando`); o erro dela dá o toast e para tudo; Cancelar para tudo; uma pergunta da
    própria tela no meio (o "Salvar mesmo assim" do texto) espera a resposta. Antes de cumprir o destino, a fila confere
    de novo as abas dele (a que mudou enquanto uma gravação era esperada também pergunta); a pergunta que some sem
    resposta (o banco trocado por fora fecha as janelas) para a fila em vez de valer como "Não salvar"; o Fechar pasta
    lembra a chave da pasta e não fecha outra que abriu no meio; o fechar da janela no meio de outra fila a transforma
    na do fechar a janela. As perguntas dos arquivos vêm antes das da exportação e da transação. A troca de banco não
    pergunta (as abas de arquivo ficam);
  - o **Trazer colunas desta planilha** (o menu do cabeçalho da planilha): a casca foca o CSV ativo, ou o usado mais
    recentemente (`Abas::mais_recente_onde`), e pede a ele a janela Trazer colunas com a planilha já escolhida como
    origem (`TelaCsv::trazer_colunas_de`); sem CSV aberto (lido), o item fica desligado com "Abra um CSV para trazer
    colunas.";
  - a **faixa das planilhas** (`interface::pecas::folhas`, 32 px entre a grade e o rodapé da planilha; prancheta 17):
    as pílulas de 24 px (a ativa em volume, as ocultas em `--t3` e itálico só com "Mostrar planilhas ocultas"), as setas
    ‹ › e a roda do mouse rolam, a lista "Planilhas" quando não cabem; `tablist` com "Vendas, planilha 2 de 4"; ←/→ e
    Enter na faixa, Ctrl+PageUp/PageDown de dentro da grade;
  - a **releitura da pasta** (sem vigia do sistema nem temporizador): o Atualizar do menu, a volta do foco
    da janela (no máximo uma a cada 2 s e nunca com uma leitura em andamento; a raiz e as subpastas lidas, também as
    que falharam) e, sem esse espaçamento, o fim de uma troca de banco (o Novo banco), o fim de uma janela (a Exportar)
    e a cópia de segurança; a releitura pedida com uma leitura em andamento espera ela terminar. A leitura que não
    achou nada novo não troca o estado. Cada leitura volta com a chave da pasta e a geração dela: a de uma pasta que já
    não é a aberta (outra pasta, o Fechar pasta, mesmo que a mesma volte depois) é descartada, e a abertura de pasta
    pedida antes de um Fechar pasta não reabre a árvore;
  - **soltar na janela** (sem o vidro "Solte para abrir"): o `RawInput::dropped_files` vira o
    `AbrirCaminho` do primeiro item (a primeira pasta; sem pasta, o primeiro que o app abre), classificado numa
    tarefa; com mais de um, o toast "Só um item abre por vez; abri dados."; com uma janela ou um modal aberto, nada
    abre;
  - o **documento** (a peça `interface::pecas::documento`, na `Anatomia` dele): no alto, o aviso da troca de banco
    que passou de 0,3 s e, fora das Consultas, o de transação aberta (de ponta a ponta, com "Ir para a Consulta 1");
    a **faixa** de contexto de 42 px; o conteúdo, na página que a casca rola (`padding:18px 20px 20px`, a rolagem de
    cada aba guardada, PageUp/PageDown/Espaço/Home/End quando nada tem o foco) ou com a rolagem própria (a grade dos
    Dados, o editor da Consulta, a Definição); e o **rodapé** de 34 px (as páginas dos Dados, as linhas e o tempo da
    Consulta). Com as Propriedades escondidas, nas telas de página as mesmas seções vêm no fim do documento, depois
    do fio e do `.tit` "Propriedades" (`estado::propriedades_no_fim`);
  - **painéis redimensionáveis** (`larguras.rs`): a `Divisoria` no vão de cada painel
    lateral muda a largura (o Explorador e o Histórico dividem a da esquerda, 258 pt por padrão; as Propriedades, 272
    pt), de 200 pt a 45% da janela, sem deixar o documento abaixo de 560 pt; o duplo clique ou o Enter voltam à
    padrão, as setas andam 16 pt. Numa janela estreita, os dois painéis encolhem antes de as Propriedades sumirem
    (agora abaixo de ~1052 pt, não mais ~1182). A escolha vale entre as sessões (a chave `"larguras_paineis"`,
    §4.5), gravada ao fim do gesto;
  - a **barra de status** (`status.rs`, 26 px): sem banco, o nome da pasta aberta (o ponto `--t3`, a dica com o
    caminho) ou "Nenhuma pasta aberta"; com banco, o banco com o ponto (`--ok`; `--aviso` com a transação aberta), o
    modo de gravação ("Gravação imediata", "Transação aberta" ou "Só leitura", com a frase do aviso de transação na
    dica) e, à direita, o que a aba diz de si ("clientes · 60 linhas · 7 colunas", "SQL"), o SQLite e a codificação
    (ele está na seção Arquivo das Propriedades). Numa janela
    estreita, o nome do banco fica inteiro e o que a aba diz de si encolhe com "…"; nada passa da barra.
  - Por cima: o toast (acima da barra de status; com ação, como o "Mostrar na pasta" da cópia de segurança) e as
    janelas globais (Exportar, Nova tabela, Novo índice, Confirmar, Pedir o caminho, Preferências, Atalhos do teclado
    e Sobre o DataBox), todas no vidro fosco (§4.7).
  - **Preferências** (`preferencias.rs`, prancheta 13; Arquivo › Preferências, Ctrl+,, Exibir › Tema › "Preferências
    de aparência" e o botão do pé da barra de ícones): a janela larga com a navegação à esquerda (Geral, Aparência,
    Consulta SQL e Exportação; a seção Tabelas fica fora enquanto não houver opção de verdade). Geral limpa os Abertos
    recentemente; Aparência tem o tema, as quatro cores do código e a fonte do código (a família entre as que existem e
    o tamanho); Consulta SQL limpa o histórico; Exportação, as opções que a janela Exportar guarda. Cada mudança vale na
    hora; o Salvar grava só o que difere do retrato da abertura (`Servico::gravar_preferencias`), e Cancelar, Esc, o × e
    o fundo reaplicam o retrato. "Restaurar o padrão" mexe só na seção à vista.
  - **Sobre o DataBox** (`sobre.rs`, prancheta 13, 440 pt): o logo de 96, "Versão 0.1.0" e a ficha (Compilação, do
    `DATABOX_COMPILACAO` do build.rs; Renderizador; SQLite; Sistema; Licença, o link "MIT"; Preferências e Arquivo de
    log, links com o caminho de verdade na dica), e no rodapé "Licenças de terceiros" (a lista do Cargo.lock, feita
    pelo build.rs), "Copiar informações" (a ficha em texto, com os `%APPDATA%`/`%LOCALAPPDATA%` no lugar do nome da
    pessoa) e "Fechar".
  - **Foco do teclado**: o Tab segue a ordem da tela (a barra de título, a de ferramentas, a de ícones, os painéis,
    as divisórias, o documento e os botões da janela) e só para em peças com nome; o espaço livre da barra de título,
    as faixas de redimensionar e a seta dos nós do Explorador não são paradas. No Explorador, → e ← abrem e fecham o
    nó com o foco nele; nos menus, ver abaixo.
- **Menus**: Arquivo (com os sólidos de 24: Abrir pasta (Ctrl+Shift+O), Abrir arquivo, Novo banco, Abertos
  recentemente (o ícone de cada tipo, sem o banco aberto nem a pasta aberta), Salvar, Fechar aba, Fechar banco, Fechar pasta (ligado com a pasta aberta), Preferências); Editar (os verbos da aba, das mesmas
  `Ofertas` da barra); Exibir (Explorador, Propriedades e Histórico de consultas, marcados quando à vista, e Tema ▸
  Seguir o Windows/Claro/Escuro e "Preferências de aparência"; Propriedades desligado no Início e quando o painel some
  por falta de largura); Ajuda (Atalhos do teclado e Sobre o DataBox, acima). Pelo teclado, com o menu aberto: do botão, ↓ vai ao primeiro item
  ligado e ↑ ao último; nos itens, ↑ e ↓ andam só pelos ligados, com volta, sem sair do painel; o Esc fecha o menu
  e devolve o foco ao botão.
- **Abas de documento e rotas**: as rotas são `Rota::{Inicio, Banco, Tabela{nome, aba, dados},
  Visao{nome, aba, dados}, Consulta{sql}, Arquivo{caminho}}` (a última, não exige banco), e a aba Dados guarda página, por página, ordem, direção, busca e filtros
  na rota. Cada aba guarda a rota dela e o documento (`editor/src/documento.rs`, o enum `Documento`: Início, Visão
  geral, tabela ou visão, Consulta SQL), que só é desenhado quando ela é a ativa; as Consultas escondidas só tomam
  as respostas que chegam (`em_segundo_plano`). Rótulos: "exemplo.db · Visão geral", "clientes · Dados", "Consulta
  2". Fechar a aba de uma Consulta que executa solta a execução numa tarefa da casca (com o id do banco para que foi
  pedida), que ainda avisa a transação e os objetos.
  - **Transação com várias Consultas**: todas usam a mesma conexão; a fonte da verdade é o `BancoInfo` do estado, não
    a aba à vista. Quando um `NotarTransacao` muda o valor, as outras Consultas esquecem a última resposta delas
    (`esquecer_transacao`); o aviso diz qual abriu ("Há uma transação aberta na Consulta 1.", "na Consulta SQL"
    quando a aba que a abriu já fechou), e "Ir para a Consulta 1" foca essa aba.
  - **Abas da mesma tabela**: mudar a estrutura numa parte relê as outras partes da mesma tabela; um
    `RecarregarObjetos` de outro lugar (uma Consulta, escondida ou não, a execução solta, as janelas) relê todas as
    abas de tabela e a Visão geral, menos a de origem. As escondidas releem quando aparecerem; a ativa relê no fim do
    quadro seguinte, depois de a célula em edição gravar (a edição termina na hora). A Visão geral só pede de novo o
    BancoInfo e os objetos, sem perder a manutenção ou a cópia em andamento. A execução solta só emite o
    `RecarregarObjetos` quando o comando mudou dados ou estrutura (ou fechou a transação). As abas das tabelas que
    sumiram fecham, menos a ativa, que relê e mostra o card de inexistente.
  - **Trocas de banco**: abrir, criar e fechar fecham as abas do banco (inclusive as Consultas; as fichas de arquivo
    de dados ficam) e soltam as execuções com
    o id do banco **de antes** (a resposta é descartada na chegada); abre a Visão geral do banco novo, ou o Início. O
    banco trocado por fora no mesmo arquivo mantém as abas e refaz cada documento pela rota (a Consulta com o
    texto); outro arquivo vai à Visão geral dele, e nenhum, ao Início.
- **Título**: `titulo_da_janela(pasta, arquivo)` dá "dados · DataBox" com uma pasta aberta (mesmo com um banco), sem
  ela "vendas.db · DataBox" e sem nada "DataBox"; vai para a barra de título e também para o sistema (`ViewportCommand::Title`: a
  barra de tarefas e o Alt+Tab).
- **Tarefas de fundo**: toda chamada ao `servico` vira uma `Promessa` (thread + aviso de repintura); a tela
  mostra `carregando` enquanto espera e ignora em silêncio `Codigo::Substituido` e respostas de um banco que
  já não é o aberto. Ao fechar a janela, o executável corta a Consulta SQL que ainda roda (ou espera a vez) no banco
  aberto (`editor::interromper_ao_sair`) e espera até 5 s as tarefas em andamento (`estado::esperar_tarefas`): uma
  gravação em voo vai até o fim e uma exportação interrompida apaga o temporário.
- **Fechar a janela** com uma exportação em andamento ou com a transação aberta pergunta antes
  (`ViewportCommand::CancelClose` e a confirmação do `pedido_de_fechar`). Com a janela minimizada ou encoberta, o
  eframe 0.36 não desenha quadro e só chama o `App::logic`; sem ele, fechar pela barra de tarefas encerrava sem
  perguntar. E as respostas que chegaram nesse meio-tempo (o BEGIN de uma Consulta, a execução solta) só são
  tomadas num quadro: o estado da casca pode estar atrasado. Com uma exportação, a transação aberta, uma Consulta
  que executa (também com a resposta pronta) ou uma execução solta, o `Editor::logica` cancela esse pedido, restaura
  a janela (`Minimized(false)` e `Focus`) e liga o `fechar_pendente`, decidido no fim do primeiro quadro da janela
  restaurada, depois do `aplicar` (`decidir_fechar_pendente`): com o motivo, a pergunta; sem ele, a janela fecha,
  como a pessoa pediu. Com a janela à vista, o `logica` não faz nada.
- **Janela sem decoração** (só no Windows, `BARRA_PROPRIA` no main.rs; o código em `editor/src/casca/janela.rs`): a
  barra de título é a faixa de 30 px do DataBox (logo, menus Arquivo/Editar/Exibir/Ajuda, o título "arquivo ·
  DataBox" no centro e os botões Minimizar, Maximizar/Restaurar e "Fechar a janela"). Os controles ficam numa camada
  `Order::Tooltip`, acima da modal: com a Exportar ou uma confirmação aberta, ainda dá para mover, minimizar e
  fechar (o `ViewportCommand::Close` volta como o pedido de fechar de sempre, com a pergunta). O espaço livre
  arrasta (`StartDrag` no começo do arrasto) e o duplo clique maximiza; faixas de 5 px por dentro da borda
  redimensionam (`BeginResize` no aperto; não existem com a janela maximizada nem com um popup aberto, um menu ou
  uma lista, que pode encostar na borda; as da esquerda e da direita começam embaixo da faixa de 30 px, e o Fechar
  fica inteiro, com o canto de cima à direita na faixa de cima); o espaço livre e as faixas só têm clique e arrasto,
  sem o foco (não entram no Tab nem na acessibilidade); o egui manda o `SetTheme` a cada mudança do tema (a borda e
  a sombra do DWM seguem o tema do app). As regiões vão para o `ctx.data`
  (`editor::regioes_da_janela`). **O que se perde**, porque o winit 0.30 não trata o `WM_NCHITTEST` e as duas saídas
  (o winit como dependência direta, para o menu do sistema, e o subclassing da janela, para o Snap Layouts) ficaram
  de fora por decisão de 16/10/2026: o Snap Layouts ao passar o mouse no Maximizar e o menu do sistema no clique
  direito do título (o Win+Z e o Alt+Espaço devem continuar), e as bordas invisíveis de ~8 px por fora. O Aero Snap,
  arrastar a maximizada para restaurar, Win+setas e Alt+F4 continuam (o laço é o do sistema).
- **Endereço**: a tela que muda o próprio estado (página, busca, filtros, texto da Consulta) emite
  `Acao::Substituir` (sem entrada no histórico). A casca sabe de que aba veio cada ação (as de cada documento chegam
  marcadas com o `IdAba` dele) e aplica o `Substituir` e o `Atualizar` nessa aba, mesmo que ela já tenha saído de
  vista; as ações da casca, das janelas e dos testes chegam sem aba. Isso substitui a regra antiga de descartar o
  `Substituir` que chegava depois de uma navegação.
- **Leveza**: parada, nenhuma tela pede repintura (o teste `editor/tests/diagnostico_repintura.rs` garante; só o
  cursor que pisca num campo com foco agenda as piscadas): com várias abas e duas Consultas abertas, com cada painel
  em cada estado (o Explorador recolhido, filtrado e rolado; o Histórico com comandos; as Propriedades de cada aba,
  escondidas ou sumidas a 960 px), com a transação aberta, com um menu e o submenu dele abertos, com as janelas da
  Ajuda e depois de o ponteiro passar pela casca toda; com a pasta aberta (a raiz, uma subpasta e uma
  vazia, o menu de contexto aberto, a ficha de um arquivo, o Início com os recentes de cada tipo e o vazio dos três
  cartões) e depois da releitura da volta do foco, que não deixa quadro pendente; desde as etapas 5 e 6, com os
  editores (o CSV parado, com uma célula em edição, com o menu do cabeçalho e com a janela Trazer colunas depois de
  casar; o JSON válido, com o erro e no leitor de um arquivo grande; o texto) e com a planilha (depois de o índice
  terminar, a faixa das planilhas com o ponteiro, rolada e com a lista aberta, o menu do cabeçalho). As abas escondidas não são desenhadas nem pedem quadro. Com
  `DATABOX_DIAGNOSTICO_REPINTURA=1` (o nome antigo, `EDITOR_DIAGNOSTICO_REPINTURA`, continua valendo), o executável
  registra no log, por segundo, quantos quadros pintou, por quê e que eventos de entrada chegaram.
- **Atalhos**: Ctrl+O, Ctrl+Enter (na Consulta SQL), Ctrl+F (a busca da barra, quando a aba tem busca), Esc,
  setas/Enter/F2/Esc/Tab/Delete na grade, mais os da casca (`editor/src/casca/atalhos.rs`): Alt+←/→ e os botões
  laterais do mouse (o histórico único, que atravessa as abas), Ctrl+W (fechar a aba), Ctrl+Tab e Ctrl+Shift+Tab (a
  aba seguinte e a anterior na faixa, com volta: `Abas::vizinha`), F5 (o Atualizar da aba, só com o Atualizar ligado
  nela; senão, a tecla fica com quem tem o foco e a edição em curso não termina), Ctrl+B (Explorador) e Ctrl+Alt+B
  (Propriedades, só com o botão delas ligado: no Início e na janela estreita não faz nada; com um campo de texto
  focado, fica com o campo, porque o AltGr+B escreve em outros teclados) e Ctrl+, (as Preferências); Ctrl+Shift+O (Abrir pasta) e, só com o foco num nó do Explorador,
  Shift+F10 (o menu de contexto) e Ctrl+Shift+C (copiar o caminho); das etapas 5 e 6, Ctrl+S (Salvar), Ctrl+Z e Ctrl+Y
  (Desfazer e Refazer fora de campo de texto), Ctrl+Shift+V (Colar com correspondência no CSV) e Ctrl+PageUp/PageDown
  (a planilha anterior e a seguinte). Todos só com os modificadores exatos (o AltGr do ABNT2 chega como Ctrl+Alt: não fecha a aba nem abre o
  diálogo) e nunca com uma janela ou modal aberta; os que trocam de aba terminam antes a edição de texto em curso
  (a célula grava) e valem no quadro seguinte. PageUp/PageDown/Espaço/Home/End rolam o documento ativo quando nada
  tem o foco. A janela "Atalhos do teclado" (Ajuda) lista todos.
- **Nome DataBox** num lugar só, `comum::app::NOME` (o `servico::NOME_APP` e o `AppInfo.nome` são ele): título
  da janela "vendas.db · DataBox" (sem banco, "DataBox"), ícone da janela `interface::marca::icone_janela` (o logo
  completo de 48 px). O executável é o `[[bin]] DataBox` do pacote `editor`, com o ícone, a versão (DataBox
  0.1.0; CompanyName "Isaías Gonçalves" e LegalCopyright "© 2026 Isaías Gonçalves — licença MIT") e o manifesto embutidos de `crates/editor/recursos/databox.rc`; o
  `databox.ico` sai de `design/ferramentas/ico.py` (só o logo completo, de 16 a 256 px: a versão simplificada deixou de ser usada em 17/10/2026).
  `--criar-atalho` cria o "DataBox.lnk"; a confirmação
  de fechar com a exportação em andamento se chama "Fechar o DataBox"; o Excel exportado leva
  `<dc:creator>DataBox</dc:creator>` e os arquivos de trabalho do .zip, o prefixo `DataBox-exportar-`.

---

## 6. Testes

- `.\cargo.ps1 test --workspace` na raiz do repositório (com `APPDATA` numa pasta temporária, para não mexer nas
  preferências reais). Unitários em cada crate (§3 item 7).
- Interface: `egui_kittest` (consulta pela árvore de acessibilidade, cliques, digitação) e capturas PNG em
  `capturas/rust/` (ignorada pelo git) para conferir o visual contra as pranchetas de `design/canvas`: as cenas `casca-*` de `editor/tests/casca.rs` montam as
  das pranchetas com o banco de exemplo. O foco do teclado (as paradas do Tab e a ordem delas, as setas do
  Explorador e dos menus, o Esc do menu) fica em `editor/tests/foco.rs`.
