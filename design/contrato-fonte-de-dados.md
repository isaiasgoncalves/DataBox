# Contrato de fonte de dados (para a 0.4)

O documento de decisão que a decisão 9 de [`planos-0.3.md`](planos-0.3.md) pediu antes de qualquer código de outro
banco: onde o DataBox hoje é do SQLite, que contrato o PostgreSQL (e os bancos seguintes do roadmap) vai implementar,
o que o PostgreSQL pede além do contrato, em que ordem a 0.4 ("Conectar") constrói isso e o que a 0.3 precisa deixar
pronto para não haver retrabalho. Escrito em 26/10/2026, com a main na 0.2.4, na onda 0 do
[`execucao-0.3.md`](execucao-0.3.md). **Nada aqui está implementado, e este documento não traz código**: os trechos
em `text` são o desenho do contrato, não uma API pronta.

Fontes: o código de `crates/comum`, `crates/banco`, `crates/esquema`, `crates/ddl`, `crates/servico` e
`crates/estado` (os caminhos e as linhas abaixo são os da main de 26/10/2026), o [`ARQUITETURA.md`](../ARQUITETURA.md)
(§2, §3, §4.1 a §4.3, §4.6 e §4.8), [`planos-0.3.md`](planos-0.3.md) (a linha "PostgreSQL" da tabela de viabilidade,
a decisão 9 e a seção da 0.4), [`planos-ssh.md`](planos-ssh.md) e a prancheta `FuturoConexao` do canvas.

**Em uma frase:** um crate novo, `fonte`, na base do grafo (só `comum`), com um trait obrigatório pequeno
(`FonteDeDados`: abrir, fechar, ler objetos, páginas, contagem, consulta livre com Interromper e leitura em fluxo) e
traits opcionais por capacidade (`FonteEditavel` primeiro), descritos para a interface por um `Capacidades` de dados
simples; o que só o SQLite tem (mudar estrutura, manutenção, cópia de segurança, a Definição SQL) continua do
SQLite e não entra no contrato.

---

## 1. Onde o contrato atual é do SQLite

### 1.1 Os pontos de acoplamento

O "contrato" de hoje é a API pública do `banco` (§4.3), usada pelo `servico` e, por tipos, pelo `estado` e pelas
telas. Cada linha é um ponto que o PostgreSQL não atravessa como está.

| # | Ponto | Onde (código) | Por que é do SQLite | No contrato |
|---|---|---|---|---|
| A1 | A conexão exposta | `Banco::com_conexao(f: FnOnce(&rusqlite::Connection))` em `crates/banco/src/banco.rs:290`; `Banco::conexao_leitura() -> Connection` em `:333` | entrega a conexão do rusqlite a quem chama; é por ela que o `esquema` e o `servico` trabalham | sai da API comum: fica só no `banco`, alcançada pelo SQLite que o pede (§2.7) |
| A2 | O `esquema` inteiro | todas as funções de `crates/esquema/src/lib.rs` recebem `&rusqlite::Connection`; leem `sqlite_schema` (`leitura.rs:36`), `PRAGMA table_list`, `table_xinfo` e `foreign_key_list` (`leitura.rs:74, 186, 295`); a reconstrução de 12 passos usa `PRAGMA foreign_keys`, `legacy_alter_table` e `sqlite_sequence` (`reconstrucao.rs:68–84`) | o plano de mudança de estrutura é o ALTER TABLE do SQLite e a receita da documentação dele | fica do SQLite; nenhuma capacidade "estrutura" no contrato da 0.4 (decisão 3) |
| A3 | O `ddl` | `crates/ddl/src/lib.rs` ("dialeto do SQLite"); `analisar_create_table` em `esquema/src/leitura.rs:95, 244` e **na tela**, em `crates/tela_dados/src/janela.rs:82` | lê o texto do `CREATE TABLE` guardado em `sqlite_schema`; o PostgreSQL não guarda esse texto | a ficha da linha (janela.rs) passa a ler das `ColunaInfo` (o que o `ddl` dá a mais vira campo opcional) |
| A4 | O serviço junta banco e esquema | `Servico::tabela` (`crates/servico/src/servico.rs:409–436`) chama `banco.com_conexao(ler_definicoes)`; `Servico::esquema` (`:561–583`) monta e executa o plano na mesma trava; `servico/src/operacoes.rs:6` importa `rusqlite::Connection`; `servico.rs:657` | a `TabelaCompleta` (as `definicoes` e `restricoes` do `ddl`) e o `OperacaoEsquema` são do SQLite | `TabelaCompleta.definicoes` fica `None` numa fonte sem estrutura editável (o caminho já existe: é o do aviso "Não foi possível entender a definição…") |
| A5 | Os valores sabem ler o SQLite | `crates/comum/Cargo.toml` depende do `rusqlite`; `Valor::de_ref(ValueRef)`, `Celula::de_ref`, `impl ToSql/FromSql for Valor` (`comum/src/valores.rs:82, 133, 146, 175`); `From<rusqlite::Error> for ErroBanco` e `traduzir_sqlite` (`comum/src/erro.rs:96, 196`); `From<ErroBanco> for rusqlite::Error` (`valores.rs:279`) | os tipos neutros moram num crate que depende do driver do SQLite | os tipos ficam onde estão; as conversões do rusqlite ficam no `comum` por enquanto (decisão 7). Pela regra de órfãos do Rust, o `banco` não pode implementar `ToSql` para o `Valor` do `comum`: tirar dali exige um tipo embrulho no `banco` |
| A6 | Afinidade | `comum::Afinidade` e `comum::afinidade` (as regras da seção 3.1 de datatype3 do SQLite, `valores.rs:25–62`); `ColunaInfo.afinidade`, `Pagina.afinidades`; `banco::coagir(&Entrada, Afinidade)`; usada na grade, na ficha e nos filtros (`tela_dados/src/{modelo,regras,janela,menu}.rs`, `tela_esquema/src/estrutura.rs`, `interface/src/icones`) | as cinco afinidades são do SQLite; o PostgreSQL tem tipos de verdade | a `Afinidade` continua como **classe de exibição** (alinhar à direita, ícone, coerção do texto digitado); o tipo nativo vai em `ColunaInfo.tipo` (já existe, é o texto declarado). Cada fonte diz a afinidade de cada tipo dela (§3.5) |
| A7 | A identidade da linha | `comum::Chave::Rowid(i64)`; `banco::TipoChave::Rowid { alias }`; `TabelaInfo.sem_rowid`; `ColunaInfo.rowid_alias` (`banco/src/tipos.rs`) | o rowid é do SQLite; o `ctid` do PostgreSQL muda a cada `UPDATE` e no `VACUUM FULL` | a `Chave::Pk` já é neutra; o PostgreSQL só usa ela. Tabela sem chave primária fica sem chave (`None`), como as WITHOUT ROWID com PK nula de hoje |
| A8 | Os tipos de objeto e o `BancoInfo` | `TipoObjeto::{Table, View, Virtual, Shadow}`, `OrigemIndice::{C, U, Pk}` (as letras do `PRAGMA index_list`), `TabelaInfo.estrita`; `BancoInfo { sqlite_versao, codificacao, tamanho_pagina, paginas, paginas_livres, modo_diario, versao_usuario, … }` (`banco/src/tipos.rs`); `AppInfo.sqlite_versao: rusqlite::version()` (`servico.rs:219`) | são o vocabulário do SQLite | o contrato tem um `InfoFonte` com o comum (nome, rótulo, só leitura e o motivo, transação aberta, contagem de objetos) e `detalhes: Vec<(rótulo, valor)>` para as Propriedades; o `BancoInfo` vira o detalhe do SQLite. `TipoObjeto` ganha `MaterializedView`, `Estrangeira` e `Particionada` quando o PostgreSQL chegar (decisão 5) |
| A9 | Um nome só, sem esquema | `Rota::Tabela { nome }` e `Rota::Visao { nome }` (`crates/estado/src/rota.rs`); `ChaveAba::Objeto { nome }`; todo método do `banco` recebe `nome: &str`; o SQL é montado como `main.<nome>` (`exportacao.rs:249`, `meta.rs`) | o SQLite tem um esquema `main` (e os anexados); o PostgreSQL tem `public`, `vendas`, `auditoria`… e o mesmo nome em dois deles | um `NomeObjeto { esquema: Option<String>, nome: String }` no contrato; `None` é o padrão da fonte (`main` no SQLite). A rota e a aba passam a carregar o esquema |
| A10 | Páginas, busca e filtros montados em SQL do SQLite | `PedidoPagina`, `Pagina`, `Filtro`, `Operador`, `Direcao` (`banco/src/tipos.rs`, `busca.rs`); o SQL em `meta.rs:297–410` (`select_sql`, `montar_ordem`, `pagina_sql` com `LIMIT ? OFFSET ?` e a CTE `editor_pagina` que traz o total na mesma leitura); a busca sem caixa por `GLOB` com as duas formas de cada letra (`busca.rs`, `glob_sem_caixa`) | os **tipos** são neutros; o **SQL** é do SQLite (o `GLOB`, o `CAST(? AS TEXT)`, a contagem junto da página) | os tipos vão para o `fonte` como estão; cada fonte monta o SQL dela. A semântica fica no contrato e ganha uma suíte de conformidade (§2.8): "contém" sem diferença de maiúsculas, nulos, vazio, a ordem estável |
| A11 | A contagem | `Banco::contagem` e `contar_linhas` (`dados.rs:155–189`); `LIMITE_CONTAGEM_BYTES` (256 MB, acima disso `linhas: None`) | o `count(*)` é barato no SQLite pequeno; num PostgreSQL de 50 milhões de linhas é uma varredura | `Contagem` ganha `exata: bool`; a fonte pode estimar (`pg_class.reltuples`) e a tela escreve "≈" (como o Perfil da tabela, decisão 4 da 0.3) |
| A12 | A consulta livre | `Banco::consultar` (`consulta.rs:138`): divide o roteiro por `dividir_comandos` (`sql.rs:155`, que chama o `sqlite3_complete` da FFI, `ffi.rs:19`), compara `PRAGMA schema_version` e `total_changes` para `alterou_esquema` e `alterou_dados`, e lê `is_autocommit` para `transacao_aberta`; os nomes das colunas pela FFI (`ffi.rs:24`); a `tela_consulta` chama `banco::dividir_comandos` direto (`tela_consulta/src/lib.rs:784`) | dividir comandos é do dialeto; os sinais de mudança são do SQLite | `ResultadoConsulta` vai para o contrato como está; `dividir_comandos` vira método da fonte (`dividir`), e a tela pergunta à fonte, não ao `banco` |
| A13 | O Interromper | `Banco::interromper_consulta_detalhado` (`consulta.rs:211`): `InterruptHandle::interrupt` com a marca sob o `Mutex`, o progress handler a cada 20.000 passos e a fila `consultas_na_fila` | o interrupt do SQLite age na conexão, no mesmo processo | o contrato guarda a **semântica** (`Interrupcao::{Nenhuma, EmAndamento, NaFila}`, nunca alcançar a próxima operação, o comando cortado com `MSG_INTERROMPIDA`); o PostgreSQL implementa com o pedido de cancelamento do protocolo, que abre outra conexão (§3.7) |
| A14 | A leitura substituída | `PedidoPagina.leitura = Some("grade:<tabela>")` e `contagem(…, leitura)`; o protocolo em `crates/banco/src/leitura.rs:51–120` (o interrupt só na leitura abandonada, a marca `em_curso`) | a interrupção é a do SQLite | fica no contrato a semântica (a mais nova de um grupo abandona a mais velha, que devolve `Codigo::Substituido`); a fonte escolhe como cortar |
| A15 | A leitura em fluxo para exportar | `Banco::iterar_tabela<R>` e `iterar_consulta<R>` (`exportacao.rs:209, 260`), com `consumir: impl FnOnce(&[String], &mut LinhasExportacao) -> Resultado<R>`; a conexão própria `mode=ro` ou a principal com `PRAGMA query_only` (`exportacao.rs:171–200`); `OrigemExportacao::Banco` (`servico/src/exportacao.rs:18`) | o iterador só vive dentro do `consumir` porque segura a trava e o comando preparado do rusqlite; e o método é **genérico** (não cabe num `dyn Trait`) | a forma de "callback" fica (ela é a que serve ao PostgreSQL também: o cursor vive dentro da transação), mas sem o genérico: `consumir: &mut dyn FnMut(…) -> Resultado<()>` (§2.3) |
| A16 | O Desfazer da grade | `Banco::desfazer_celula` e `desfazer_intervalo` (`gravacao.rs:337, 433`): grava o `antigo` só se a célula ainda tem o `gravado`, num `SAVEPOINT` (`gravacao.rs:49`); a tela monta o pedido (`tela_dados/src/menu.rs:243`, `para_desfazer`) | o `SAVEPOINT` dentro da transação aberta da Consulta e o "ainda tem o gravado" são feitos em SQL do SQLite | a semântica é neutra e vai para a `FonteEditavel` como está ("compare e troque" por célula, tudo ou nada); fica fora da 0.4, que é só leitura no PostgreSQL |
| A17 | Transação aberta compartilhada | `Banco::transacao_aberta`, `exigir_sem_transacao` e `Codigo::TransacaoAberta`; a Consulta abre o `BEGIN` na conexão principal, que a grade, a Estrutura e a exportação também usam (§4.3) | uma conexão só por banco, em autocommit, com a trava reentrante, é o desenho do SQLite embutido | capacidade `transacoes` no `Capacidades`; no PostgreSQL só leitura, desligada (a Consulta recusa `BEGIN`? ver decisão 6) |
| A18 | `trusted_schema` e a desconfiança do arquivo | `conectar` liga `PRAGMA trusted_schema = OFF` em toda conexão (`banco.rs:122–131`); `foreign_keys = ON`, `temp_store = MEMORY` | o perigo do SQLite é o **arquivo** que veio de qualquer lugar | o equivalente do PostgreSQL é outro: a **sessão** (só leitura, `statement_timeout`, `search_path` fixo para os comandos do próprio app) (§3.6). O contrato diz "a fonte abre desconfiada", não como |
| A19 | Só leitura | `Banco::abrir(…, somente_leitura, …)` com `mode=ro`, `motivo_somente_leitura` (arquivo protegido, `-journal` quente) | no SQLite, só leitura é garantido pelo modo de abrir o arquivo | no PostgreSQL, a sessão só leitura **não** é garantia contra o SQL da pessoa (§3.6); o contrato separa "a fonte é só leitura" (o app não oferece gravar) de "a fonte garante" |
| A20 | Manutenção e cópia | `Banco::manutencao` (`VACUUM`, `integrity_check`, `foreign_key_check`, `optimize`, `ANALYZE`; `manutencao.rs:123`) e `copia_de_seguranca` pela API de backup (`:180`); `Acao::ManutencaoDoBanco(banco::AcaoManutencao)` no `estado/src/acao.rs:100` | comandos e API do SQLite | ficam do SQLite (§2.7); o menu Banco desliga os itens pelo `Capacidades` |
| A21 | Um banco por vez, achado pelo caminho | `Servico { banco: Mutex<Option<Arc<Banco>>> }` (`servico.rs:32`), `banco_aberto(banco_id)` (`:138`); o histórico de SQL e os recentes pela chave do **caminho** (`prefs::registrar_sql(caminho_banco, …)`, `servico.rs:595`; `TipoRecente::{Banco, Pasta, Arquivo}`) | um banco é um arquivo | o `servico` guarda `Arc<dyn FonteDeDados>`; a chave do histórico e dos recentes vira um `EnderecoFonte` (o caminho, ou `postgres://usuario@maquina:porta/base`, sem senha); um `TipoRecente::Conexao` novo (o `prefs` já guarda as chaves desconhecidas, então uma versão antiga não perde o item) |
| A22 | Os tipos do `banco` na interface | `estado` importa `banco::{Direcao, Filtro, Operador}` (`rota.rs:22`), `TabelaInfo` (`tabela.rs:11`), `TipoObjeto` (`documento.rs:29`), `AcaoManutencao` (`acao.rs:100`); as telas importam `Pagina`, `PedidoPagina`, `ResultadoConsulta`, `Interrupcao`, `CelulaDesfeita`, `ColunaInfo`, `TipoChave`, `BancoInfo`, `ObjetosInfo`… (cerca de 35 usos em `tela_dados`, `tela_consulta`, `tela_esquema`, `tela_exportar` e `editor`) | o nome do crate é `banco` (SQLite) | os tipos mudam de crate (`fonte`) e o `banco` os reexporta (`pub use fonte::…`): nenhuma tela muda no PR que move. Depois, as telas trocam `banco::` por `fonte::` aos poucos |

### 1.2 O que já é neutro

Boa parte do desenho já atravessa sem mudança, e é por isso que o contrato é um trabalho de M, não de G:

- **Os erros.** `ErroBanco { mensagem, detalhe, status, codigo }` e os `Codigo` estáveis (`TransacaoAberta`,
  `DestinoExiste`, `BancoTrocado`, `Substituido`, `Cancelado`) não citam o SQLite. Só as **frases** de `traduzir_sqlite`
  são dele; o PostgreSQL ganha o seu `traduzir_pg` (pelo SQLSTATE, §2.5).
- **Os valores.** `Valor`, `Celula`, `Entrada` e a `Chave::Pk` servem a qualquer banco (§2.5 e §3.5).
- **A forma das respostas.** `Pagina`, `LinhaPagina`, `LinhaCompleta`, `LinhaGravada`, `ObjetosInfo` (as listas de
  tabelas, visões, índices e gatilhos), `ColunaInfo` (menos `cid`, `oculta` e `rowid_alias`), `ChaveEstrangeira`,
  `ResultadoConsulta` e `ResultadoComando` descrevem o que a tela mostra, não como o SQLite guarda.
- **O cancelamento.** `Parar = dyn Fn() -> bool + Send + Sync` e o "callback devolve `Err` e cancela" do `exportar`.
- **O `exportar`.** O `Escritor` recebe linhas empurradas (`&[Valor]`) e não sabe de onde vêm.
- **O padrão de "fonte" do Trazer colunas.** `estado::fonte::FonteDeColunas` já é um trait `Send + Sync` com rótulo,
  tipo, nomes baratos e leitura cara numa tarefa: o mesmo jeito que o contrato propõe.
- **A thread da interface nunca toca o banco** (§3 item 6): tudo passa pela `Promessa`. Um banco de rede, lento e
  que cai, não muda essa regra; só a reforça.

### 1.3 O que o código contradiz nos planos e no `ARQUITETURA.md`

Para registrar no PR (nenhum destes muda uma decisão; o 1 e o 2 mudam a forma do PR 1 da 0.3):

1. **A `FonteDeLinhas` do PR 1 com um iterador dono não cabe no banco.** O plano (`planos-0.3.md`, §3.2, "Base: leitor
   de linhas comum") a descreve com "iterador de `Vec<Valor>`" implementado também pela tabela e pela consulta do banco.
   No código, o iterador do banco só vive dentro do `consumir` de `iterar_tabela` (`exportacao.rs:209`), porque segura
   a trava e o comando preparado (`Rows<'stmt>` empresta o `Statement`). Um iterador que sai da função exigiria uma
   thread com um canal ou um tipo autorreferente. A saída é o trait da `FonteDeLinhas` ter a forma de callback (§4.1).
2. **O `comum` depende do `rusqlite`** (`crates/comum/Cargo.toml`), e o §2 do `ARQUITETURA.md` não diz. Não é ciclo,
   mas o plano põe a `FonteDeLinhas` no `comum`: ela não pode usar nenhum tipo do rusqlite na assinatura.
3. **A tela da ficha da linha lê o `CREATE TABLE`** (`tela_dados/src/janela.rs:82`, `ddl::analisar_create_table`).
   A linha "PostgreSQL" da viabilidade cita o `ddl` como acoplamento do `esquema`, mas há um na tela também.
4. **O bloco de código do §4.3 do `ARQUITETURA.md` está atrás do `lib.rs` do `banco`** em três pontos: diz
   `Filtro { …, valor: String }` (o código tem `valor: Entrada`, `busca.rs:92`, como o próprio §4 diz mais acima), `parar:
   Option<&Parar>` (o código recebe `Option<Arc<Parar>>`) e `contar_para_exportar(…) -> Option<u64>` (o código devolve
   `Resultado<Option<u64>>`). Vale o `lib.rs`, como o `ARQUITETURA.md` manda; o bloco deve ser corrigido num PR de
   documentação (fora deste, que não mexe no `ARQUITETURA.md`).
5. **Não existe `Capacidades` no código.** O nome é proposta deste documento (§2.4), no molde das `Ofertas` dos verbos.

---

## 2. O contrato proposto

### 2.1 Princípios

1. **O contrato cobre o que duas fontes de verdade fazem, não o que uma faria.** Na 0.4 são o SQLite e o PostgreSQL só
   leitura. O que só o SQLite faz (a reconstrução de tabela, o `VACUUM`, a cópia pela API de backup, a Definição SQL
   tirada de `sqlite_schema`) continua dele, sem um método no contrato que o PostgreSQL responderia com 501.
2. **Semântica no contrato, mecânica na fonte.** O contrato diz "a leitura mais nova de um grupo abandona a mais velha,
   que devolve `Substituido`"; o SQLite faz com o interrupt e o PostgreSQL com o cancelamento ou só descartando a
   resposta. Cada regra do §4.3 vira uma frase do contrato e um teste da suíte de conformidade (§2.8).
3. **Sem tipo do driver na assinatura.** Nada de `rusqlite::` ou `postgres::` no `fonte`. Os erros chegam já
   `ErroBanco`, os valores já `Valor`/`Celula`.
4. **Tudo bloqueante e `Send + Sync`, chamado de uma `Promessa`.** O PostgreSQL vai ter um runtime por baixo (§3.1),
   mas, como no `remoto`, ninguém fora do crate dele vê `tokio`.
5. **A interface pergunta o que a fonte faz uma vez, por dados** (`Capacidades`), e não descobre pelo erro.

### 2.2 As alternativas

| | (a) Um trait só | (b) Traits por capacidade + `Capacidades` | (c) Enum de bancos |
|---|---|---|---|
| **Forma** | `trait FonteDeDados` com tudo (ler, editar, estrutura, manutenção), os métodos opcionais com padrão que devolve 501 "indisponível" | um trait obrigatório pequeno (`FonteDeDados`) e traits opcionais (`FonteEditavel`; depois o que precisar), alcançados por `fn edicao(&self) -> Option<&dyn FonteEditavel>`; um `Capacidades` com o que a interface liga e desliga | `enum Fonte { Sqlite(Arc<Banco>), Postgres(Arc<BancoPg>) }` no `servico`, e cada método do serviço faz um `match` |
| **Prós** | o mais simples de escrever; um `Arc<dyn …>` só; a interface não muda | o compilador diz o que a fonte tem: o PostgreSQL não implementa `FonteEditavel` e pronto; a interface lê o `Capacidades` uma vez (como as `Ofertas`); o que é só do SQLite fica do SQLite; um banco novo é um crate novo que implementa os traits, sem tocar nos outros | sem `dyn` e sem problema de "object safety": os métodos genéricos (`iterar_tabela<R>`) ficam como estão; o `match` exaustivo acha o que falta em cada banco; o refatoramento de hoje é o menor |
| **Contras** | o PostgreSQL "tem" `atualizar` que responde 501: a interface só descobre fazendo; o `esquema` com `&Connection` não cabe, e o trait vira o `Banco` de hoje com outro nome; cada banco novo arrasta tudo | mais traits e o `Option<&dyn …>` no caminho; os métodos genéricos viram `&mut dyn FnMut` (uma alocação e uma chamada indireta por leitura, não por linha); o downcast (`Any`) para chegar no SQLite é uma porta que precisa de disciplina | cada banco novo do roadmap (MySQL/MariaDB, SQL Server, DuckDB) soma um braço em ~35 métodos do `servico`; não há um contrato escrito para testar uma fonte isolada (a suíte de conformidade testa o enum, não o banco); o `servico` passa a saber de todos os drivers |
| **Custo** | M agora, G depois (a cada banco) | M+ agora (o crate `fonte`, mover os tipos, o `impl` para o `Banco`, o `servico` com `dyn`), P–M a cada banco | P–M agora, M a cada banco |

**Recomendação: (b).** O (c) é o mais barato com dois bancos, mas o roadmap do README já promete quatro, e o custo
dele cresce com cada um em cima do arquivo mais disputado do núcleo (`servico.rs`). O (a) junta os defeitos: a
interface erra para descobrir e o PostgreSQL carrega a forma do SQLite. O (b) custa um PR a mais agora e deixa cada
banco seguinte num crate próprio, testado pela mesma suíte. O preço do (b) que dá para ver no código é o fim dos
métodos genéricos: `iterar_tabela<R>` e `com_conexao<R>` não cabem num `dyn Trait`. O `com_conexao` sai do contrato
(fica do SQLite); a leitura em fluxo troca o `R` por um `FnMut` que guarda o resultado no lado de quem chama.

### 2.3 Os traits

O desenho, sem ser código pronto (os nomes seguem os do `banco` para o PR que move ser só mover):

```text
crate fonte (← comum)

trait FonteDeDados: Send + Sync + Any
    // quem é
    id() -> &str                          16 hex desta abertura (como Banco::id)
    endereco() -> &EnderecoFonte          o caminho, ou postgres://usuario@maquina:porta/base (sem senha): a chave do
                                          histórico de SQL, dos recentes e da aba
    tipo() -> TipoDeBanco                 Sqlite | Postgres (o ícone, o emblema do design/README.md)
    rotulo() -> String                    "exemplo.db", "vendas em servidor"
    capacidades() -> Capacidades          barato, sem rede: a interface chama a cada quadro
    somente_leitura() -> bool · motivo_somente_leitura() -> Option<&str>
    fechar()                              idempotente; usar depois → 409

    // ler o catálogo
    info() -> Resultado<InfoFonte>        o comum + detalhes: Vec<(String, String)> para as Propriedades
    objetos() -> Resultado<ObjetosInfo>
    tabela_info(&NomeObjeto) -> Resultado<TabelaInfo>

    // ler as linhas
    linhas(&NomeObjeto, &PedidoPagina) -> Resultado<Pagina>
    contagem(&NomeObjeto, busca, &[Filtro], leitura: Option<&str>) -> Resultado<Contagem>   Contagem.exata
    linha(&NomeObjeto, &Chave) -> Resultado<LinhaCompleta>
    ler_blob(&NomeObjeto, &Chave, coluna) -> Resultado<Option<Vec<u8>>>

    // consulta livre
    dividir(sql) -> Vec<String>           os comandos de topo, no dialeto da fonte
    consultar(sql, limite) -> Resultado<ResultadoConsulta>
    interromper_consulta() -> Interrupcao   de qualquer thread, sem esperar a trava; a semântica do §4.3
    consulta_em_andamento() -> bool
    transacao_aberta() -> bool            sempre false sem a capacidade `transacoes`

    // leitura em fluxo (exportar, perfil, gráficos, Salvar como, a bancada)
    ler_tabela(&NomeObjeto, &FiltroExportacao, parar: Option<Arc<Parar>>,
               consumir: &mut dyn FnMut(&[ColunaDeLinhas], &mut LinhasExportacao) -> Resultado<()>) -> Resultado<()>
    ler_consulta(sql, parar, consumir) -> Resultado<()>
    contar_para_ler(&NomeObjeto, &FiltroExportacao, parar, prazo) -> Resultado<Option<u64>>

    // capacidades opcionais
    edicao() -> Option<&dyn FonteEditavel>     padrão None

trait FonteEditavel: Send + Sync          (o SQLite; o PostgreSQL só quando sair do só leitura)
    inserir · atualizar · atualizar_intervalo · excluir · salvar_blob
    desfazer_celula · desfazer_intervalo       "compare e troque": grava o antigo só se ainda há o gravado; tudo ou nada
    exigir_sem_transacao(acao) -> Resultado<()>

struct NomeObjeto { esquema: Option<String>, nome: String }      None = o padrão da fonte (main, public)
struct ColunaDeLinhas { nome: String, tipo: String, afinidade: Afinidade }   o que a FonteDeLinhas da 0.3 também diz
enum EnderecoFonte { Arquivo(PathBuf), Rede { esquema_url, usuario, maquina, porta, base } }
```

O que cada parte resolve, frente à tabela do §1.1:

- **Abrir não está no trait.** Cada fonte abre do seu jeito (`Banco::abrir(caminho, somente_leitura, criar)`;
  `BancoPg::conectar(&PerfilConexao, senha, túnel)`), e o `servico` guarda o resultado como `Arc<dyn FonteDeDados>`.
  Abrir é o único lugar em que o `servico` sabe que banco é.
- **`ler_tabela` e `ler_consulta` mantêm a forma de callback** do `iterar_*` (o iterador só vive dentro do
  `consumir`), que é a certa para os dois bancos: no SQLite segura a trava e o comando preparado; no PostgreSQL, o
  cursor do lado do servidor vive dentro de uma transação. O `R` genérico vira `()`, e quem chama guarda o resultado
  numa variável do lado de fora (o `Resumo` do `Escritor`, o perfil).
- **A `FonteDeLinhas` da 0.3 sai daqui de graça**: uma tabela ou uma consulta de qualquer `FonteDeDados` é uma
  `FonteDeLinhas` por um adaptador de dez linhas no `fonte` (§4.1).

### 2.4 `Capacidades`

Dados simples, no molde das `Ofertas` dos verbos: a interface lê e liga ou desliga, sem tentar e errar.

```text
struct Capacidades {
    editar: bool                 // Nova linha, Duplicar, Excluir, Colar, Limpar, Desfazer nos Dados (= edicao().is_some())
    transacoes: bool             // BEGIN na Consulta e o aviso de transação da casca
    estrutura: bool              // Estrutura editável, Nova tabela, Novo índice (só o SQLite, §2.7)
    definicao_sql: bool          // a parte "Definição SQL"
    manutencao: bool             // o menu Banco (integridade, VACUUM, otimizar) e a cópia de segurança
    contagem_exata_barata: bool  // falso: a grade pede a contagem estimada primeiro e a exata sob demanda
    esquemas: bool               // a árvore do Explorador agrupa por esquema; a rota leva o esquema
    busca_sem_acento: bool       // falso no PostgreSQL sem `unaccent`: a dica da busca diz "diferencia acentos"
    cancelar: bool               // o Interromper liga
    garante_somente_leitura: bool // verdadeiro no SQLite mode=ro; falso no PostgreSQL com sessão só leitura (§3.6)
}
```

O `Capacidades` é a única coisa que as telas novas precisam aprender. Hoje elas decidem por `TabelaInfo.editavel`,
`esquema_editavel` e `Banco::somente_leitura` (por exemplo `tela_esquema::apoio::pode_editar`); esses campos continuam
(dizem respeito à tabela) e o `Capacidades` diz respeito à fonte. A regra: o verbo liga só se as duas deixam.

### 2.5 Como os erros e os valores atravessam

**Erros.** Continua `ErroBanco` com `status` e `codigo`. Cada fonte traduz o erro do driver dela na saída, nunca
depois: o `banco` com o `traduzir_sqlite` de hoje; o PostgreSQL com um `traduzir_pg` pelo SQLSTATE, que é estável
(`23505` violação de unicidade → 409; `42P01` tabela inexistente → 404; `42501` sem permissão → 403; `25006`
transação só leitura → 403 "A conexão é só leitura."; `57014` cancelado → `MSG_INTERROMPIDA`; `08xxx` conexão → 503
com frase de rede; `28P01` senha → 401). Dois status novos no §3 do `ARQUITETURA.md`, que hoje vai até 501: **401**
(a credencial foi recusada: a interface pede a senha de novo) e **503** (a máquina não respondeu: a interface oferece
reconectar). Dois `Codigo` novos: `Desconectado` (a conexão caiu; a tela mostra "Reconectar") e `CredencialRecusada`.
As frases vão pelo `t!` desde a I2 (a varredura dos núcleos), como as do SQLite.

**Valores.** `Valor` é o que está no banco, exato; `Celula` é o que a grade mostra; `Chave` é a identidade; `Entrada` é
o que a pessoa manda gravar. Nenhum muda de forma na 0.4:

- O PostgreSQL tem mais tipos que as seis variantes do `Valor`. O que não é inteiro, real, texto ou bytes chega como
  **texto** na forma de saída do próprio PostgreSQL (data em ISO, `numeric` com todos os dígitos, `uuid`, `jsonb`,
  vetores), e a `Afinidade` da coluna diz como mostrar (§3.5). Isso evita perder precisão (`numeric(38,10)` não cabe
  num `f64`) e evita uma variante nova que todos os `match` do app teriam de aprender agora.
- `Celula` não muda. `Celula::TextoCortado` e `Celula::Blob { tamanho, inicio }` servem como estão; o PostgreSQL corta
  no servidor (`left(col, 301)` e `octet_length`) para não trazer um `text` de 1 GB para mostrar 300 caracteres.
- `Chave` usa só `Chave::Pk(Vec<(String, Valor)>)` no PostgreSQL. O `WHERE` compara cada coluna com o parâmetro
  convertido para o tipo dela (`coluna = $1::uuid`), para usar o índice.
- `Entrada` só aparece com a `FonteEditavel`. A coerção (`banco::coagir`) é do SQLite (as regras de afinidade e o
  número pt-BR); no PostgreSQL, o texto vai como parâmetro tipado pelo próprio servidor (`$1::date`) e o erro de
  conversão (`22007`, `22P02`) vira 400 com a coluna.

### 2.6 Onde mora no grafo

```text
comum ← fonte ← banco (SQLite) ──┐
          ↑  ← banco_pg ─────────┤← servico ← estado ← tela_* ← editor
          │       (← comum, sem remoto) ↑
          │                      remoto (0.3, ← comum) ──┘   o servico abre o túnel e passa a porta ao banco_pg
          └── exportar, tabular, planilha, texto (a FonteDeLinhas, §4.1)
```

- **`fonte`** (crate novo, só `comum`): os traits, `Capacidades`, `NomeObjeto`, `EnderecoFonte`, e os tipos neutros
  que hoje moram no `banco` (`TabelaInfo`, `ColunaInfo`, `ObjetosInfo`, `Pagina`, `PedidoPagina`, `Filtro`, `Operador`,
  `Direcao`, `Contagem`, `ResultadoConsulta`, `ResultadoComando`, `Interrupcao`, `LinhaCompleta`, `LinhaGravada`,
  `CelulaDesfeita`, `FiltroExportacao`, `Parar`, `MSG_INTERROMPIDA`, `MSG_SUBSTITUIDA`). Nenhuma dependência de fora.
- **`banco`** (o SQLite, o nome fica) ← `fonte`: `impl FonteDeDados for Banco` e `impl FonteEditavel for Banco`,
  e reexporta os tipos que saíram (`pub use fonte::{…}`), para o `estado` e as telas compilarem sem mudança.
- **`banco_pg`** (crate novo; não `postgres`, que é o nome do driver) ← `fonte`: o PostgreSQL. Não depende do
  `remoto`: recebe o destino já resolvido (`maquina:porta` direto, ou `127.0.0.1:porta` do túnel com o nome
  verdadeiro para conferir o certificado; §3.4).
- **`servico`** ← `fonte`, `banco`, `banco_pg`, `esquema`, `remoto`: abre a fonte certa, abre o túnel quando a
  conexão pede, guarda `Arc<dyn FonteDeDados>` e faz o downcast para o SQLite onde precisa (§2.7).
- **`estado`** e as telas passam a importar de `fonte` aos poucos; nenhuma conhece `banco_pg`.

Sem ciclo: o `fonte` não conhece ninguém além do `comum`; o `banco` e o `banco_pg` não se conhecem; só o `servico`
conhece os dois.

### 2.7 O que fica do SQLite

A Estrutura (o `esquema`, o `ddl`, a `TabelaCompleta.definicoes`), a Definição SQL, as manutenções, a cópia de
segurança e o "Novo banco" continuam do SQLite, alcançados pelo `servico` com um downcast:
`fonte.as_any().downcast_ref::<banco::Banco>()` (é para isso o `Any` no trait). Quem não é SQLite responde 501 no
`servico` (`Codigo` nenhum: a interface nem chega a pedir, porque o `Capacidades` desliga). O `com_conexao` fica no
`Banco`, sem passar pelo trait.

Por que não uma capacidade "estrutura" no contrato já: o PostgreSQL da 0.4 é só leitura, e um `OperacaoEsquema`
neutro desenhado sem o segundo banco que o use é o trait gordo de novo. Quando um banco além do SQLite precisar de
estrutura editável, o `OperacaoEsquema` (que já é tipado, `servico/src/operacoes.rs`) vira a entrada de uma
`FonteDeEstrutura`, e cada banco monta o plano dele.

### 2.8 A suíte de conformidade

Um módulo de testes no `fonte` (`fonte::conformidade`, atrás de um recurso `conformidade`) com as regras do §4.3 do
`ARQUITETURA.md` escritas contra o trait: a página com busca, filtros e ordem estável; o NULL na ordem; a contagem
igual às linhas; a leitura substituída devolvendo `Substituido`; o Interromper que nunca alcança a próxima operação; o
`ler_tabela` com `parar`; `Chave` que volta a mesma linha. O `banco` roda a suíte sobre o `exemplo.db` das `amostras`;
o `banco_pg` sobre o mesmo conteúdo carregado num PostgreSQL de teste (§3.9). É a prova de que o contrato é um
contrato, e o que um banco novo do roadmap tem de passar.

---

## 3. O que o PostgreSQL pede além do contrato

### 3.1 O driver

| | `postgres` (síncrono) | `tokio-postgres` (assíncrono) | `sqlx` | `libpq` (`pq-sys`) |
|---|---|---|---|---|
| Licença | MIT/Apache | MIT/Apache | MIT/Apache | PostgreSQL (C) |
| O que é | uma casca bloqueante sobre o `tokio-postgres`, com um runtime de uma thread dentro | o protocolo em Rust puro, sobre `tokio` | consultas assíncronas, vários bancos, macros | a biblioteca oficial em C |
| Prós | API do jeito do `banco` (bloqueante, numa `Promessa`); cursor (`query_raw`, `RowIter`), `CancelToken`, `simple_query` com o texto de cada coluna | o controle fino: `connect_raw` sobre qualquer fluxo (o canal do túnel do `russh`, sem porta local), o `CancelToken` com `cancel_query_raw` | um driver para o PostgreSQL e o MySQL do roadmap | tudo do PostgreSQL (GSSAPI, `service`, `.pgpass`) |
| Contras | o runtime próprio não é o do `remoto` (dois runtimes em threads de fundo); `connect_raw` não existe na casca | o `banco_pg` precisa do seu runtime numa thread, como o `remoto` | pesado, macros que pedem um banco na compilação (a menos que se use só a API dinâmica), muito mais do que o app usa | compilar C com a MinGW: o risco de build que o projeto evita desde o começo; no Linux, a `libpq` da distribuição |
| `tokio` | sim (por baixo) | sim | sim | não |

**Sem decidir o driver (decisão 1):** o candidato natural é o `tokio-postgres` usado do mesmo jeito que o `remoto`
usa o `russh` (um runtime numa thread de fundo, nenhum `tokio` fora do crate), porque o túnel pelo canal do SSH e o
cancelamento pelo mesmo túnel pedem o `connect_raw` e o `cancel_query_raw`. O `postgres` síncrono é o plano B se o
túnel for por uma porta local. O `sqlx` e a `libpq` ficam de fora pelos motivos da tabela. O custo de build com a
MinGW é o mesmo risco do SSH: o `tokio` já vem com o `russh` na 0.3, e o que soma é o TLS (§3.2).

### 3.2 TLS

O PostgreSQL de produção pede TLS (`sslmode=require` no mínimo; `verify-full` no certo). Opções:

- **`rustls`** (pelo `tokio-postgres-rustls` ou um conector próprio de poucas linhas): Rust, sem OpenSSL. O provedor
  padrão do `rustls` é o `aws-lc-rs`, que pede CMake e NASM no Windows (o mesmo problema anotado para o `russh` em
  [`planos-ssh.md`](planos-ssh.md), §2.1): usar o provedor `ring`, que compila com o `cc` e a MinGW. Os certificados da
  máquina pelo `rustls-native-certs` (o repositório do Windows e o `/etc/ssl` do Linux), e um arquivo de CA próprio
  (`sslrootcert`) para o servidor da empresa.
- **`native-tls`** (`postgres-native-tls`): o SChannel no Windows, mas o **OpenSSL** no Linux, que o AppImage teria de
  levar ou achar na distribuição. Contra a regra de "sem C de fora".

Recomendação a confirmar no spike: `rustls` com `ring`, e a escolha do `sslmode` na janela da conexão (`disable`,
`require`, `verify-ca`, `verify-full`; o padrão é `verify-full`, e `require` diz na dica que não confere quem
responde). Certificado de cliente (`sslcert`/`sslkey`) fica para depois.

### 3.3 O cofre de senhas

O `keyring` (MIT/Apache) já está aprovado para o "Lembrar" do SSH (decisão 4 de `planos-ssh.md`), que é do S2 da
0.4. O PostgreSQL usa a mesma peça e as mesmas regras: senha só com "Lembrar" marcado, nunca no
`preferencias.json`, no log ou no endereço (`EnderecoFonte` não tem senha); "Esquecer esta conexão" apaga a senha
guardada. A peça que guarda e lê (um módulo `cofre` no `prefs` ou no `servico`) deve nascer **uma vez**, no S2, com a
chave pelo endereço (`databox:ssh:isaias@servidor`, `databox:pg:isaias@servidor:5432/vendas`). O PostgreSQL só a usa.
No Linux, o Secret Service pelo D-Bus pode não existir (uma máquina sem chaveiro): o "Lembrar" fica desligado com a
dica, e a senha é pedida a cada conexão.

### 3.4 O túnel SSH

A conexão comum a um PostgreSQL de produção é pelo SSH (o banco só aceita conexão de dentro do servidor). O `remoto`
da 0.3 já sabe conectar, conferir a chave da máquina e usar o agente; falta abrir um canal `direct-tcpip` até
`localhost:5432` do lado de lá. Dois jeitos:

- **Pelo canal, sem porta local:** o `remoto` entrega o canal como um fluxo, e o `tokio-postgres` conecta por
  `connect_raw` sobre ele. Nada escuta na máquina da pessoa. O cancelamento (§3.7) abre outro canal pelo mesmo túnel.
- **Por uma porta local:** o `remoto` escuta em `127.0.0.1:<porta livre>` e repassa. Funciona com qualquer driver,
  mas qualquer programa da máquina pode usar a porta enquanto ela existe.

Nos dois, o TLS por dentro do túnel confere o certificado pelo **nome verdadeiro** do servidor de banco, não por
`127.0.0.1`. A janela da conexão (a prancheta `FuturoConexao`) ganha "Conectar por SSH" com uma conexão SSH salva do
S2. Recomendação: pelo canal, se o spike provar.

### 3.5 Os tipos do PostgreSQL

| Tipo do PostgreSQL | `Valor` / `Celula` | `Afinidade` (exibição) | Observação |
|---|---|---|---|
| `int2`, `int4`, `int8`, `oid` | `Inteiro` | Integer | |
| `float4`, `float8` | `Real` | Real | `NaN` e `Infinity` já existem na `Celula::Real` |
| `numeric`, `decimal` | `Texto` (todos os dígitos) | Numeric (à direita) | um `f64` perderia dígitos; exportar para o Excel como número é a decisão 4 |
| `money` | `Texto` | Numeric | a forma depende do `lc_monetary` do servidor |
| `bool` | `Texto` (`true`/`false`) | Text | decisão 4 |
| `text`, `varchar`, `char`, `name`, `citext` | `Texto` (cortado no servidor) | Text | |
| `bytea` | `Blob` (o começo e o tamanho na grade) | Blob | |
| `date`, `time`, `timetz`, `timestamp`, `timestamptz`, `interval` | `Texto` em ISO 8601 | Text | a sessão com `DateStyle = ISO` e o `TimeZone` da máquina da pessoa |
| `uuid`, `inet`, `cidr`, `macaddr` | `Texto` | Text | |
| `json`, `jsonb`, `xml` | `Texto` | Text | depois: "Abrir no visualizador de JSON" |
| vetores, compostos, intervalos (`int4range`), `enum`, domínios, PostGIS | `Texto` na forma de saída do PostgreSQL | a do tipo base, quando houver; senão Text | o domínio herda a afinidade do tipo base |
| NULL | `Nulo` | | |

O jeito mais simples de ler é pedir o texto de cada coluna (o protocolo de texto) e converter para `Inteiro`/`Real`
só os tipos numéricos de base; o `bytea` em binário. O spike mede se isso é rápido o bastante para a grade e para a
exportação de 1 milhão de linhas.

### 3.6 Só leitura por padrão

A janela da conexão traz "Só leitura (recomendado)" marcado (prancheta `FuturoConexao`). O que isso garante e o que
não garante, dito na tela:

- **O que o app faz:** a sessão abre com `default_transaction_read_only = on`; toda leitura do app roda numa transação
  `READ ONLY` com `statement_timeout` (30 s na grade, nenhum na exportação, que tem o Interromper); a grade não
  oferece editar; a Consulta roda cada roteiro dentro de um `BEGIN READ ONLY … ROLLBACK`.
- **O que não dá para garantir só pelo app:** o SQL que a pessoa digita pode desligar a sessão só leitura (`SET
  default_transaction_read_only = off`, ou um `COMMIT` seguido de um `BEGIN READ WRITE` no meio do roteiro). Dividir o
  roteiro e recusar palavras (`SET`, `COMMIT`, `BEGIN`) é uma peneira, não uma garantia: há `DO`, funções que gravam,
  `CALL`. **A única garantia é um usuário do banco sem permissão de gravar.** A faixa diz "Só leitura nesta conexão;
  para garantir, use um usuário sem permissão de gravar" e o `Capacidades.garante_somente_leitura` é falso.
- **O mesmo cuidado do `trusted_schema`:** os comandos do próprio app citam o esquema de tudo (`pg_catalog.`, o
  esquema da tabela) e fixam o `search_path` na sessão, para uma função com o mesmo nome num esquema da pessoa não
  rodar no lugar da do sistema.

### 3.7 Interromper e leitura substituída

O PostgreSQL cancela por um **pedido de cancelamento**: uma conexão nova à mesma máquina e porta, com a chave secreta
que o servidor deu ao abrir (o `CancelToken` do driver). Consequências para o contrato:

- O cancelamento atravessa o túnel e o TLS **de novo** (abre outra conexão). O spike prova isso.
- O servidor cancela "o que estiver rodando agora" naquela conexão: a mesma corrida que o §4.3 resolve no SQLite com a
  marca sob o `Mutex`. O `banco_pg` usa a mesma ideia: a marca do `consultar` em posse da conexão, e o pedido só sai
  com a marca presente; um cancelamento que chegue entre dois comandos é absorvido (o próximo comando confere a marca
  antes de começar).
- A leitura substituída da grade pode ficar mais simples: a grade usa uma **conexão de leitura própria** (o
  PostgreSQL aguenta várias), e a leitura abandonada é cancelada ou só tem a resposta jogada fora. O `Substituido`
  continua o que a tela vê.

### 3.8 O resto que o PostgreSQL traz

- **Esquemas.** A árvore do Explorador agrupa as tabelas por esquema (o `public` aberto, os outros fechados), a rota e
  a aba levam o esquema (A9), e o título da aba mostra `vendas.pedidos` quando não é o padrão.
- **Várias bases na mesma máquina.** Uma conexão é a uma base; trocar de base é conectar de novo. A lista de bases
  (`pg_database`) aparece na janela da conexão, não na árvore.
- **A conexão cai.** Uma rede cai no meio de uma leitura; o SQLite nunca. O `banco_pg` devolve `Codigo::Desconectado`,
  a faixa oferece "Reconectar", e nenhuma tela trava esperando (o `connect_timeout` de 10 s e o `keepalive` do TCP).
- **Um banco por vez continua**, e "abrir outro" fecha o anterior, como hoje. Várias conexões abertas ao mesmo tempo
  (um SQLite e um PostgreSQL lado a lado) é outra conversa, que o `servico` de um banco por vez (`servico.rs:32`) não
  tem; fica fora da 0.4.
- **O histórico de SQL e os recentes** pela chave do `EnderecoFonte` (A21).
- **A bancada da 0.3 (SQL sobre CSV)** é um SQLite: num PostgreSQL aberto, ela não entra por `ATTACH`. A Consulta sobre
  CSV continua abrindo sobre a bancada, sem juntar com o PostgreSQL (decisão 8).

### 3.9 Um PostgreSQL de teste no CI

- **Linux:** as máquinas `ubuntu-*` do GitHub aceitam um contêiner de serviço (`services: postgres:17`), e também
  trazem um PostgreSQL instalado, desligado. O contêiner fixa a versão; é o caminho.
- **Windows:** não há contêiner Linux. As imagens `windows-*` do GitHub trazem um PostgreSQL instalado com o serviço
  desligado (a conferir no spike: a versão, as variáveis `PGBIN`/`PGROOT`, o tempo para ligar). Se servir, o CI do
  Windows liga o serviço e roda a suíte de conformidade lá também. Se não servir, os testes do `banco_pg` que precisam
  de servidor rodam só no Linux (como os do `sshd` do `remoto`), e no Windows só os que não precisam (os tipos, as
  frases do SQLSTATE, a montagem do SQL); a conexão real entra na lista "O que testar à mão" do `RELEASE.md`.
- **O conteúdo:** um roteiro (`amostras/dados/exemplo_pg.sql`) que cria no PostgreSQL o mesmo conteúdo do
  `exemplo.db` mais o que só o PostgreSQL tem (esquemas, `numeric`, `jsonb`, `uuid`, vetores, uma visão
  materializada, uma tabela sem chave primária), para a suíte de conformidade comparar os dois.
- **Versões:** a mais nova e a mais velha que o PostgreSQL ainda mantém (hoje, a 17 e a 13). Duas versões no Linux,
  uma no Windows.

### 3.10 O spike: o que ele precisa provar

Um PR só (`spike(banco_pg)`), sem tela, com um relatório curto neste documento, como o S0 do SSH. Ele decide o driver
(decisão 1) provando, nos dois sistemas:

1. **Compila** com a MinGW da WinLibs e no Ubuntu 22.04, com o TLS escolhido (o `ring`, sem CMake nem NASM), e
   **quanto soma** ao executável e ao tempo de build limpo e incremental do CI.
2. **Conecta** com senha (SCRAM-SHA-256 e MD5), com TLS em `require` e em `verify-full` (contra um certificado próprio
   e contra os certificados da máquina), e falha com frase certa em senha errada, máquina que não responde e
   certificado que não confere.
3. **Pelo túnel do `remoto`** (o canal, ou a porta local): conecta, confere o certificado pelo nome verdadeiro e
   **cancela** uma consulta longa (`SELECT pg_sleep(60)`) pelo mesmo túnel.
4. **Lê em fluxo** 1 milhão de linhas de uma tabela com 10 colunas variadas sem passar de memória constante, com
   `parar` cortando no meio, e mede o tempo contra a meta da exportação do SQLite.
5. **Os tipos** da tabela do §3.5 chegam como a tabela diz (texto ou binário: o que for mais rápido e exato).
6. **Só leitura**: a sessão recusa um `INSERT` com `25006`, e o relatório anota o que passa (o `SET … off`), para a
   frase da faixa.
7. **Um runtime só:** o `tokio` do `banco_pg` e o do `remoto` convivem (dois runtimes em threads de fundo, ou um
   compartilhado) sem travar e sem `tokio` fora dos dois crates.
8. **O CI:** um PostgreSQL de teste no Linux (contêiner) e o do Windows (o serviço da imagem), ou o relatório diz por
   que o Windows fica sem servidor.

---

## 4. Ordem e PRs

### 4.1 O que a 0.3 precisa deixar pronto (para não refazer na 0.4)

| PR da 0.3 | O que precisa caber no contrato | Custo de fazer já |
|---|---|---|
| **PR 1** `FonteDeLinhas` | a forma de **callback**: `ler(parar, consumir: &mut dyn FnMut(&[ColunaDeLinhas], &mut dyn Iterator<Item = Resultado<Vec<Valor>>>) -> Resultado<()>)`, não um iterador dono (§1.3, item 1); as colunas com `nome`, `tipo` (o texto do banco ou o detectado no CSV) e uma classe neutra; nenhum tipo do rusqlite na assinatura (§1.3, item 2); o `banco` implementa por um adaptador sobre `iterar_tabela`/`iterar_consulta`, sem mexer neles | zero: é escolher a forma |
| **PR 2** gravador de SQLite | o `CREATE TABLE` da tabela nova recebe as colunas pela classe neutra da `FonteDeLinhas` (Inteiro, Real, Texto, Data…) e escolhe a afinidade do SQLite **dentro** do `exportar`; assim um `numeric` do PostgreSQL vira `NUMERIC` (ou `TEXT`) no Salvar como sem código novo | zero |
| **U2** rolagem contínua nos Dados | o pedido de linhas como **janela** (início, quantidade) com a ordem estável, e a contagem que pode chegar depois (ou estimada); se a U2 mudar o `PedidoPagina`, o contrato herda o dela | baixo: é como a U2 já tende a ser feita |
| **Q1/Q2** colunas na doca e autocompletar | o cache de colunas da Consulta lê `ObjetosInfo`/`TabelaInfo` (já neutros), não o `PRAGMA` direto; o autocompletar trata `esquema.tabela` (o tokenizador do `ddl` já separa o ponto) | baixo |
| **I2** varredura dos núcleos | as frases de erro do `banco` com `t!`, sem citar "SQLite" onde a frase vale para qualquer banco ("A tabela não existe", não "O SQLite não achou a tabela") | baixo, durante a varredura |
| **PR 12** `remoto` | a API deixa abrir um canal `direct-tcpip` depois (a sessão guardada, não só o SFTP), mesmo que a 0.3 não use | baixo: não fechar a porta |
| **PR 5** a bancada | a bancada é uma `FonteDeDados` SQLite como outra qualquer (o `Banco` de sempre), sem um caminho próprio no `servico` | baixo |

Nenhum destes acrescenta trabalho à 0.3: são escolhas de forma. Se o PR 1 sair com um iterador dono, o custo de
consertar na 0.4 é o PR 1 de novo (as quatro implementações e o perfil, os gráficos e o Salvar como que o usam).

### 4.2 Os PRs da 0.4

O PostgreSQL vem depois do S2 e do S3 do SSH (decisão 1 de `planos-ssh.md`), mas os PRs C1 a C3 do contrato não
dependem do SSH e podem correr em paralelo com o S2, porque mexem em crates de núcleo diferentes.

| PR | O que é | Depende de | Toca a casca |
|---|---|---|---|
| **C1** `refactor(fonte)` | o crate `fonte` com os tipos neutros movidos do `banco`; o `banco` reexporta; nenhum comportamento muda, nenhuma tela muda | a 0.3.0 | não |
| **C2** `refactor(banco, servico)` | os traits `FonteDeDados` e `FonteEditavel` e o `Capacidades`; `impl` para o `Banco`; o `servico` guarda `Arc<dyn FonteDeDados>` e faz o downcast para a Estrutura, a manutenção e a cópia; `dividir` no lugar do `banco::dividir_comandos` da `tela_consulta`; a suíte de conformidade passando no SQLite | C1 | não |
| **C3** `refactor(estado, telas)` | `NomeObjeto` na rota e na aba (o esquema, `None` em tudo de hoje); as telas ligam os verbos pelo `Capacidades`; a ficha da linha sem o `ddl` (A3); os recentes e o histórico pelo `EnderecoFonte` (com a migração do caminho de hoje) | C2, a I3 | sim (a rota e o Explorador) |
| **P0** `spike(banco_pg)` | §3.10 | o S0 e o PR 12 (para o túnel) | não |
| **P1** `feat(banco_pg)` | conectar (senha, TLS, `sslmode`), o catálogo (esquemas, tabelas, visões, visões materializadas, colunas, chaves, índices), páginas com busca e filtros, contagem estimada e exata, a consulta livre com Interromper, a leitura em fluxo; só leitura; a suíte de conformidade no CI | C2, P0 | não |
| **P2** `feat(servico, prefs)` | as conexões no `prefs` (sem senha), a senha no cofre do S2, o túnel pelo `remoto`, o `TipoRecente::Conexao` | P1, o S2 (o cofre) | não |
| **P3** `feat(estado, editor)` | Arquivo › Conectar a um banco… (a prancheta `FuturoConexao`, revista antes), a árvore com os esquemas, a faixa "Só leitura nesta conexão…", "Reconectar", o emblema do PostgreSQL | P2, C3 | sim |
| **P4** `docs` | o README (o guia "Abrir um PostgreSQL pelo SSH"), o `ARQUITETURA.md` (o §4 do `fonte` e do `banco_pg`, os status 401 e 503), o CHANGELOG, a lista "O que testar à mão" | P3 | não |

A ordem na casca: C3 entra antes do P3 e depois dos PRs do S2/S3 que mexem no Explorador e na `Rota::Arquivo`, um de
cada vez, como na 0.3. O caminho crítico é C1 → C2 → P1 → P2 → P3; o P0 corre ao lado do C1.

**O portão do spike:** se o P0 falhar no Windows (o TLS ou o driver não compilam com a MinGW), o C1 a C3 entram mesmo
assim (eles valem por si: tiram o SQLite da interface e deixam a bancada e o resto testáveis por contrato), e o
PostgreSQL espera um driver que compile.

---

## 5. Decisões para o Isaías

1. **Driver do PostgreSQL.** Recomendação: decidir pelo spike (P0), com o `tokio-postgres` num runtime próprio (como o
   `remoto`) como candidato, o `postgres` síncrono como plano B se o túnel for por porta local, e sem `sqlx` nem
   `libpq`. *Dependência nova a aprovar depois do spike*, como foi com o `russh`.
2. **A forma do contrato.** Recomendação: (b), os traits por capacidade com o `Capacidades` (§2.2), num crate novo
   `fonte`; não o trait gordo (a) nem o enum de bancos (c).
3. **A Estrutura no contrato.** Recomendação: não na 0.4. A Estrutura, a Definição SQL, as manutenções e a cópia de
   segurança continuam do SQLite, alcançadas por downcast; uma `FonteDeEstrutura` só quando um segundo banco editável
   precisar.
4. **Os tipos sem par no `Valor` (`numeric`, `bool`, datas).** Recomendação: chegam como texto na forma do PostgreSQL,
   com a afinidade certa para alinhar; nenhuma variante nova no `Valor` na 0.4. No exportar para o Excel, o `numeric`
   vai como número quando couber num `f64` sem perder dígitos e como texto quando não couber; o `bool` vai como
   `VERDADEIRO`/`FALSO` do Excel. Datas como data do Excel ficam para quando o `Valor` ganhar a variante (a planilha
   já tem `Data`, `Hora` e `DataHora` no `planilha::Valor`).
5. **Esquemas e os tipos novos de objeto.** Recomendação: o `NomeObjeto` com o esquema já no C3 (todo o SQLite com
   `None`); a árvore agrupa por esquema só no PostgreSQL; as visões materializadas, as tabelas estrangeiras e as
   particionadas aparecem como visão (só leitura, com o tipo nas Propriedades).
6. **O que a Consulta faz no PostgreSQL só leitura.** Recomendação: cada roteiro dentro de um `BEGIN READ ONLY …
   ROLLBACK` feito pelo app, sem transação aberta pela pessoa (o `BEGIN` e o `COMMIT` digitados dão a frase "Nesta
   conexão só leitura, a Consulta não abre transações."), e a faixa que diz que só um usuário sem permissão de gravar
   garante (§3.6). Desligar o "Só leitura" na janela da conexão é para a versão que editar.
7. **O `rusqlite` no `comum`.** Recomendação: deixar como está na 0.4 (não é ciclo, e tirar dali exige um tipo
   embrulho no `banco` por causa da regra de órfãos); o `fonte` não usa nada dele. Se um dia sair um executável sem o
   SQLite, as conversões vão para um recurso do `comum`.
8. **A bancada (SQL sobre CSV) com um PostgreSQL aberto.** Recomendação: fora da 0.4. A Consulta sobre arquivos abre
   na bancada, sem `JOIN` com o PostgreSQL; juntar os dois é trazer a tabela do PostgreSQL para a bancada pelo Salvar
   como, que já existe.
9. **O PostgreSQL no CI do Windows.** Recomendação: tentar o serviço que a imagem do GitHub já traz (o spike confere);
   se não servir, o Linux é a referência do `banco_pg`, como é do `remoto`, e a conexão real vai para o "O que testar
   à mão".
10. **Quando escrever C1 e C2.** Recomendação: no começo da 0.4, em paralelo com o S2 do SSH (crates diferentes), e
    não na 0.3: a 0.3 já tem a varredura da tradução mexendo nos mesmos crates. O que a 0.3 faz é só a forma certa do
    PR 1 e do PR 2 (§4.1).
