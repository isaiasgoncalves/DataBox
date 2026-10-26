# Plano de execução da 0.3.0

Como a 0.3.0 vai ser construída, com o que já foi decidido em [`planos-0.3.md`](planos-0.3.md),
[`planos-ssh.md`](planos-ssh.md) e [`revisao-ui-0.3.md`](revisao-ui-0.3.md): em que ordem, o que corre ao mesmo tempo,
o que bloqueia o quê, as regras de operação e onde o Isaías entra. Escrito em 23/10/2026, com a main na 0.2.4. Nada
aqui está implementado.

Os números dos PRs (1 a 14) e os nomes I1 a I4, U1 e U2, S0 e S1 são os de `planos-0.3.md`, `revisao-ui-0.3.md` e
`planos-ssh.md`.

---

## 1. Ponto de partida

- A main está na **0.2.4** (publicada em 23/10/2026). Nada da 0.3.0 começou.
- Todas as decisões da 0.3.0 estão tomadas, inclusive a tradução (P7), os gráficos (P6b) e, desde 23/10/2026, a
  primeira etapa do SSH (S1), como a trilha E.
- As pranchetas `FuturoPerfil` e `FuturoGrafico` já existem no canvas; a do SSH não (a janela da conexão e a da
  impressão digital da chave).
- Falta um PR pequeno da U11: tirar as silhuetas de seis itens dos menus da barra (Novo banco, Colunas, Renomear a
  tabela, Propriedades em janela própria, Fazer cópia de segurança e as manutenções do Banco). Só o guia foi atualizado.

---

## 2. As ondas

Cada linha é uma onda; as sessões de uma onda correm ao mesmo tempo, em worktrees diferentes. No máximo **4 sessões**
ao mesmo tempo.

| Onda | PRs | Sessões | Entra quando |
|---|---|---|---|
| **0** | **U11** dos ícones; **Q1** (**PR 15**), as colunas na doca "Tabelas" da Consulta; revisão da prancheta dos gráficos (**PR 7**) e a do SSH; o documento do contrato de fonte de dados (0.4, sem código) | 3–4 | já. A U11 e a Q1 antes da varredura, porque mexem em menus e na `tela_consulta` |
| **1** | **I1**: o crate `idioma`, `t!`/`tn!`, o `.po`, o extrator no CI, o seletor nas Preferências, `interface::formato` | 1 | depois da U11 |
| **2** | **I2** núcleos ‖ **I3a** `tela_*` ‖ **I3b** `interface`/`estado`/`editor` | 3 | depois da I1; crates disjuntos |
| **2b** | **PR 1** `FonteDeLinhas` → **PR 2** gravadores de SQLite e de JSON Lines | 1 | depois que a I2 entrar; só toca crates de núcleo, então corre junto com a I3 |
| **3** | **U1** números formatados e "Como gravados" → **U2** rolagem contínua nos Dados; **I4** `en.po`, glossário, pseudoidioma e revisão; **S0** (PR 11), o spike do SSH | 4 | depois da I3 (U1 e U2 mexem em `tela_dados`). O S0 é um crate novo e não conflita com nada; pode começar já na onda 2b se houver sessão livre |
| **4** | as trilhas A a E (abaixo) | até 4 | depois das ondas 2b e 3 |
| **5** | README e capturas em português e em inglês, CHANGELOG, a lista "O que testar à mão" do `RELEASE.md`, a versão, a tag | 1–2 | depois da onda 4 |

O que **bloqueia** o quê:

- Todo recurso depende da I1 (texto novo já nasce com `t!` e a linha em inglês no `.po`).
- A varredura (I2, I3) briga com qualquer PR aberto no mesmo crate. Por isso os PRs de recurso que tocam texto só
  entram depois dela; os de núcleo sem texto (PR 1, PR 2, S0) correm junto.
- A I4 só adiciona arquivos (`en.po`, glossário, testes). Não bloqueia nenhum recurso: sai do caminho crítico.
- **Caminho crítico:** I1 → I2 → PR 1 → PR 2 → trilha B (Salvar como e a bancada).

### As trilhas da onda 4

| Trilha | PRs | Depende de | Observação |
|---|---|---|---|
| **A · Entender** | PR 3 (perfil da coluna) → PR 6 (perfil da tabela e da seleção) | PR 1, U2 | o PR 3 substitui as contas do CSV; as Propriedades ganham a vista "Perfil" (Alt+5) |
| **B · Levar** | PR 4 (Salvar como e Salvar tudo) → PR 5 (a bancada e "Consultar com SQL") | PR 1, PR 2 | o PR 5 depende do 4 só pela janela de prévia dos tipos |
| **C · Ver** | PR 8 (gráficos, `egui_plot` 0.37) → PR 9 (Abrir no Python) | PR 1, PR 7 | o PR 9 reaproveita a escolha de colunas do 8 |
| **D · Formatos** | PR 10 (JSON Lines como tabela; `.sql` na Consulta) | PR 1 | pequeno: entra quando liberar uma sessão |
| **F · Consultar (Q2)** | PR 16 (o autocompletar no editor da Consulta) | PR 15, I3a | pequeno e sem dependência nova; não toca a casca. Entra um de cada vez com a parte `.sql` do PR 10, que também mexe na `tela_consulta` |
| **E · Conectar (S1)** | PR 11 (spike) → PR 12 (`remoto`) → PR 13 (pasta remota e cache) → PR 14 (a interface) | S0 passar; PR 14 depende também da I3 | só o PR 14 toca a casca |

Com 4 sessões e 6 trilhas, começam A, B, C e E (a de maior risco de build); a F e a D entram quando uma delas fechar, a F primeiro (a pessoa sente a diferença no dia a dia). Os PRs
11 a 13 são crates novos ou de núcleo e correm desde a onda 3.

### A casca em fila

Os PRs que mexem na casca (menus, verbos, a fila de salvar, o Explorador, `Rota`) correm em paralelo mas **entram um de
cada vez**, pela ordem em que o CI ficar verde; quem entra depois faz rebase, como no #16 e no #18 da 0.2.3. A ordem
prevista é 3 → 4 → 14 → 5 → 8 → 9 → 10; o PR 6 entra a qualquer hora depois do 3. O PR 14 sobe cedo porque mexe no
Explorador e em `Rota::Arquivo`, que o 4 e o 5 também leem.

O CHANGELOG ("Não lançado") é o outro ponto de conflito: quem entra por último resolve.

---

## 3. A trilha E, o SSH na 0.3.0

Só leitura. O que entra: conectar (as chaves e o agente que a pessoa já usa, ou a senha digitada, **sem** "Lembrar"),
conferir a chave da máquina (nova, igual, mudada), listar, trazer o arquivo para um cache local com progresso e
Interromper, e abrir o caminho do cache com os editores de sempre. O banco SQLite remoto abre a cópia em só leitura. O
que fica na 0.4: salvar de volta, "Lembrar" no cofre (`keyring`), os recentes, a pasta remota que volta, o banco
remoto editável e o "Enviar ao servidor".

| PR | O que é | Como se prova |
|---|---|---|
| **11** `spike(remoto)` | `russh` + `russh-sftp` compilam com a MinGW e no Ubuntu 22.04; o tamanho somado ao executável; listar um diretório e ler um arquivo de um servidor de teste | um relatório curto no `planos-ssh.md` |
| **12** `feat(remoto)` | o crate: conectar, `known_hosts`, listar, metadados, trazer com `parar` | testes com o `sshd` do Ubuntu num usuário temporário, no CI do Linux; no Windows, só os que não precisam de servidor |
| **13** `feat(servico, prefs)` | a pasta remota no `servico::pasta` (a mesma `Listagem`, os mesmos limites), o cache, as conexões no `prefs` sem senha | testes do `servico` contra o `remoto` de teste |
| **14** `feat(estado, editor)` | Arquivo › Conectar por SSH…, a janela da conexão e a da impressão digital, o Explorador remoto, a aba "saida.csv · servidor", o progresso | testes de tela (com o servidor falso do `remoto`) e a prancheta do SSH |

**O portão do spike:** se o S0 falhar no Windows, a trilha E sai da 0.3.0 sem atrasar o resto, ou usa o `ssh`/`sftp` do
sistema, se o Isaías preferir (plano B de `planos-ssh.md`). A 0.3.0 não espera pela trilha E.

**Um arquivo remoto reaproveita tudo:** como o editor lê o caminho do cache, o Perfil, o Salvar como, o SQL sobre CSV e
os gráficos funcionam num arquivo remoto sem trabalho extra. O PR 14 confere isso com um teste de tela.

---

## 4. Regras de operação

Do que a 0.2.x e a 0.2.3 mostraram:

- **Uma sessão por PR**, em worktree próprio, coordenadas pela sessão do plano ("0.3: planos do PO e do TL"), que
  também faz o merge quando o CI fica verde.
- **O `/tmp` é um tmpfs de 7,6 GB e estourou** numa sessão da 0.2.3: nas sessões da 0.3, o `TMPDIR` dos testes vai para
  o disco, e os worktrees arquivados são limpos.
- **Portão de cada PR:** `cargo fmt --check`, `clippy -D warnings`, os testes do crate e os do editor com
  `--test-threads=1`, e o CI do Linux e do Windows verdes (o do Windows leva cerca de 22 min: abrir o PR cedo). O Linux
  é a referência das capturas.
- **Depois da I1:** texto novo nasce com `t!` e a linha em inglês no `.po`; o CI recusa texto solto.
- **Desempenho:** perfil e conversão ganham teste em release com as metas do plano (o perfil de uma coluna de 1 milhão
  de linhas em poucos segundos; 1 milhão de linhas de CSV em SQLite em menos de 15 s).
- **Tipos na conversão:** coluna com zero à esquerda ou mistura de formatos fica TEXT por padrão, e a prévia deixa
  trocar.
- **O egui não sobe na 0.3.0** (a 0.3.1 faz isso, num PR só, com o `egui_plot` 0.38).

---

## 5. Riscos que mandam na ordem

| Risco | O que a ordem faz |
|---|---|
| A varredura da tradução briga com todo PR aberto | a I1 e a varredura vêm antes dos recursos; os de núcleo sem texto correm junto |
| O CI do Windows continua intermitente | a 0.2.3 já deixou os testes de tela numa thread só; o Linux é a referência |
| `russh` não compila com a MinGW, ou pesa no executável | o spike (S0) vem cedo, antes de qualquer outro PR da trilha E |
| Tipos errados na conversão (CEP, datas pt-BR) | o PR 4 mostra os tipos e deixa trocar antes do PR 5 usar a mesma janela |
| A 0.3.0 é a maior até agora | marcos de verificação do Isaías (abaixo); a trilha E é a primeira a sair, se o spike falhar |

---

## 6. Onde o Isaías entra

1. **Fim da onda 3:** conferir o inglês nas capturas das telas principais (decisão 14 da 0.3) e a grade com os números
   formatados.
2. **Fim da trilha B:** testar o Salvar como e a bancada com um CSV de verdade no AppImage. Os tipos e os zeros à
   esquerda são o maior risco.
3. **Fim da trilha C:** conferir os gráficos nos dois temas.
4. **Fim da trilha F:** escrever consultas de verdade com o autocompletar: apelidos, `JOIN` com o `ON` sugerido, o Enter que só aceita depois das setas, os dois temas.
5. **Fim da trilha E:** conectar a um servidor de verdade, aceitar a chave, abrir um CSV e um banco; recusar uma chave
   mudada.
6. **Antes da tag:** rodar a lista "O que testar à mão" do `RELEASE.md`, com os itens novos (o diálogo do Salvar como
   pelo portal, rodar o Python, o SSH).

---

## 7. Depois da 0.3.0

- **0.3.1:** subir para o egui 0.37 e o `egui_plot` 0.38 (PR só disso, com as capturas conferidas).
- **0.3.2 — "Entender o banco":** o diagrama relacional em aba (Q3), gerado a partir do banco, com mover, ampliar, o segmentado Tabelas · Chaves · Colunas e a busca. Detalhe em `planos-0.3.md`, P8.
- **0.4 — "Conectar":** o S2 e o S3 do SSH (salvar de volta, "Lembrar", recentes, o banco remoto editável), depois o
  PostgreSQL só leitura, com o túnel SSH e as senhas no cofre do sistema. O contrato de fonte de dados, a ordem dos
  PRs (C1 a C3, o spike P0 e P1 a P4) e as decisões estão em
  [`contrato-fonte-de-dados.md`](contrato-fonte-de-dados.md); a seção 4.1 dele lista o que os PRs 1, 2, 5 e 12, a U2,
  a Q1/Q2 e a I2 desta versão precisam respeitar.
