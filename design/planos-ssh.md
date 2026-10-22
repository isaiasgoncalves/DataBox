# Planos do SSH — ver arquivos de outras máquinas

Pedido do Isaías em 21/10/2026: "integração com SSH, para ver arquivos que estão em outros dispositivos", em uma das
próximas versões. Este documento traz a proposta do PO, a resposta do TL e as decisões. Nada aqui está implementado.

---

## 1. O PO

> *"O dado de quem trabalha com dados raramente está no notebook. Está no servidor de treino, na máquina do
> laboratório, na VM do pipeline. Hoje a pessoa faz `scp`, abre, edita, faz `scp` de volta e torce para não ter
> sobrescrito nada."*

**Problema:** o engenheiro de dados quer olhar o `saida.csv` que o job da noite gravou em `/srv/etl/`; o cientista de
dados quer conferir o `experimentos.db` da GPU do laboratório; o desenvolvedor quer ler o `.log` da VM de teste. Hoje
o DataBox só enxerga o disco local, e eles saem dele para o terminal.

**Pronto quando:**

- **Conectar:** Arquivo › Conectar por SSH… pede um destino (`isaias@servidor`, `servidor:2222` ou um `Host` do
  `~/.ssh/config`, listados para escolher) e a pasta inicial. Conecta com as chaves e o agente que a pessoa já usa no
  terminal (`~/.ssh/id_*`, o ssh-agent no Linux, o agente do OpenSSH do Windows ou o Pageant); senha e frase da chave,
  só quando pedidas, e guardadas no cofre do sistema só se a pessoa marcar "Lembrar".
- **Navegar:** a pasta remota aparece no Explorador como uma raiz a mais (`isaias@servidor:/srv/etl`), com o mesmo
  jeito da pasta local: subpastas, bancos pelo cabeçalho, CSV, Excel, JSON e texto, os mesmos limites (500 itens,
  prazo). O ícone diz que é remota; a dica diz a máquina e a latência.
- **Abrir:** um clique abre o arquivo com o mesmo editor do arquivo local. A aba diz que é remoto
  ("saida.csv · servidor"). Arquivo grande mostra o progresso de trazer, com Interromper.
- **Salvar de volta:** CSV, JSON e texto salvam na máquina remota (Ctrl+S), com a mesma regra do local: se o arquivo
  mudou lá desde que abriu, nada é gravado e o aviso oferece Ler de novo.
- **Banco remoto:** abre uma **cópia local** com tudo que o DataBox faz num banco: Dados, edição, Consulta SQL,
  estrutura, exportar. A faixa diz que é uma cópia ("cópia de experimentos.db de servidor, 21/10 14:32") e se há
  mudanças a enviar. **Enviar ao servidor** sobe o banco inteiro só se, lá, o arquivo não mudou desde a cópia e não
  há `-wal` nem `-journal` (ninguém está gravando); senão, nada é enviado e o aviso explica. **Atualizar** traz de
  novo (pergunta antes, se houver mudanças a enviar). Ao fechar com mudanças não enviadas, pergunta, como o Salvar.
  *(Decidido em 21/10/2026: o Isaías quer edição e consultas no banco remoto.)*
- **Segurança à vista:** na primeira conexão a uma máquina, a impressão digital da chave dela aparece para a pessoa
  conferir (como o `ssh` faz); se mudar depois, a conexão é recusada com o aviso, nunca aceita em silêncio.
- **Lembrar:** as conexões usadas entram nos recentes do Início, e a pasta remota aberta volta na próxima abertura
  (pedindo a senha de novo, se for o caso).

**Fora da primeira versão:** editar um banco SQLite remoto **ao vivo** (no servidor, com outros programas gravando
ao mesmo tempo: exige o `sqlite3` de lá ou um ajudante do DataBox no servidor); abrir um arquivo de vários GB sem trazê-lo
inteiro (ver 2.4); outros protocolos (SMB, FTP, S3, WebDAV); terminal SSH dentro do app; rodar comandos remotos.

---

## 2. O TL

> *"Cabe bem no desenho: o Explorador já é uma lista de entradas vinda de um serviço, e os editores já trabalham
> sobre um caminho local. O SSH entra como uma origem nova de arquivos, que traz o arquivo para um cache local e
> devolve o caminho. O que pede cuidado é a biblioteca de SSH, que não pode quebrar o build com a MinGW, e a
> segurança."*

### 2.1 A biblioteca

| Opção | Licença | Prós | Contras |
|---|---|---|---|
| **`russh` + `russh-sftp`** (Rust puro) | Apache-2.0 | sem C; agente e chaves do OpenSSH; mantidas (0.64, de 10/2026) | trazem o `tokio` (um runtime numa thread de fundo, como o `Promessa` já faz com threads); a criptografia padrão (`aws-lc-rs`) pede CMake e NASM no Windows: usar a da `ring` ou conferir no build com a MinGW |
| `ssh2` (libssh2 em C) | MIT/Apache | API síncrona e madura | compila libssh2 e, no Linux, OpenSSL; com a MinGW, é o risco de build que o projeto evita desde o começo |
| Chamar o `ssh`/`sftp` do sistema | — | zero dependência; usa o `~/.ssh/config`, o agente e o `known_hosts` da pessoa sem escrever nada disso | o Windows 10/11 traz o OpenSSH, mas nem sempre ligado; ler a saída do `sftp` é frágil; senha exige terminal |

**Recomendação:** `russh` + `russh-sftp`, depois de um **spike** de um PR só: compila com a MinGW e no Ubuntu 22.04,
mede o tamanho que soma ao executável, lista um diretório e lê um arquivo de um servidor de teste. Se o spike falhar
no Windows, o plano B é o `ssh`/`sftp` do sistema.

### 2.2 Onde entra nos crates

- **`remoto`** (crate novo, ← `comum`): a sessão SSH/SFTP (conectar, `listar`, `metadados`, `trazer` com progresso e
  `parar`, `enviar` atômico — temporário remoto, `fsync` e `rename` —, a conferência do `known_hosts`), num runtime
  próprio numa thread de fundo. Nenhum outro crate vê `tokio` nem `russh`.
- **`servico::pasta`** ganha a pasta remota: a mesma `Listagem` do local, com os mesmos limites; o cabeçalho dos
  bancos é lido pelos 16 primeiros bytes, como hoje.
- **O cache:** `pasta_de_estado/remoto/<máquina>/<caminho>`, com o tamanho e a data remotos guardados; abrir de novo
  sem mudança lá não traz de novo. Os editores (`tabular`, `texto`, `planilha`, `banco`) recebem o caminho do cache e
  **não mudam**.
- **O salvar de volta:** depois do salvar local de sempre, o `remoto` confere se o remoto ainda tem o tamanho e a data
  da abertura e só então envia. É a regra do CSV (`tabular` recusa gravar sobre o que outro programa mudou) aplicada
  ao outro lado.
- **`prefs`:** as conexões (destino, porta, usuário, pasta, impressão digital aceita), sem nenhuma senha. As senhas,
  com "Lembrar", no cofre do sistema pelo `keyring` (MIT/Apache; o Gerenciador de Credenciais do Windows, o Secret
  Service no Linux), a mesma peça que a 0.4 usaria para o PostgreSQL.
- **`estado`/`editor`:** a `Rota::Arquivo` ganha a origem (local ou remota); a aba e o Explorador mostram a máquina.

**O banco remoto:** o SQLite não pode ser aberto direto pela rede (o travamento dele não funciona sobre SFTP, e o
banco corrompe). Por isso a cópia: o `banco` abre o arquivo do cache como qualquer banco local, e o `remoto` envia o
arquivo inteiro de volta (temporário remoto e `rename`, que é atômico no mesmo sistema de arquivos) depois de
conferir o tamanho, a data e a ausência de `-wal`/`-journal` lá. Antes de enviar, um `PRAGMA wal_checkpoint(TRUNCATE)`
e o `journal_mode` original devolvido, para o arquivo enviado ser o banco inteiro. O risco que fica — outro programa
começar a gravar lá entre a conferência e o `rename` — é dito na dica do Enviar.

### 2.3 Segurança

- Chave da máquina conferida contra o `~/.ssh/known_hosts` da pessoa; máquina nova pergunta com a impressão digital
  SHA-256; chave mudada recusa. Gravar no `known_hosts` só com a pessoa marcando "Lembrar esta máquina".
- Nada de senha no `preferencias.json`, no log ou na URL; o log diz a máquina, nunca a credencial.
- O cache fica na pasta da pessoa, com as permissões dela; "Esquecer esta conexão" apaga o cache e a senha guardada.
- Abrir arquivo remoto nunca executa nada lá: só SFTP (listar, ler, gravar), sem `exec`.

### 2.4 Arquivo grande

A primeira versão traz o arquivo inteiro (com progresso e Interromper) e avisa acima de 1 GB. Ler direto da máquina
remota por partes (o SFTP lê por posição, e o `tabular` já lê por páginas com um índice) exige que os leitores leiam
de um `Read + Seek` em vez de um caminho: é uma mudança no `tabular`, no `texto` e no `planilha` que vale uma versão
própria, depois.

### 2.5 Testes

Um servidor SFTP de teste no CI do Linux (o `sshd` do Ubuntu num usuário temporário, ou um contêiner), com os testes
do `remoto` e um teste de tela abrindo um CSV remoto; no Windows, só os testes que não precisam de servidor. A
conferência da chave da máquina tem teste próprio (nova, igual, mudada).

### 2.6 Custo e lugar

**G**: o spike (P), o `remoto` com o cache (M), o Explorador e as abas (M), salvar de volta (M), segurança e
recentes (M). Não cabe na 0.3, que já tem a tradução, o perfil, o Salvar como, o SQL sobre CSV e os gráficos.

**Proposta conciliada:** abrir a **0.4 — "Conectar"** pelo SSH, antes do PostgreSQL. O SSH reaproveita todos os
editores e entrega primeiro; o PostgreSQL vem depois, usando o mesmo cofre de senhas e, de quebra, o **túnel SSH**
(o banco que só aceita conexão de dentro do servidor), que é o jeito comum de chegar num PostgreSQL de produção. O
spike da biblioteca pode correr no fim da 0.3, para a 0.4 começar sem a dúvida do build.

| Marco da 0.4 | Conteúdo |
|---|---|
| S0 | Spike da biblioteca (no fim da 0.3) |
| S1 | `remoto`, conectar, chave da máquina, Explorador remoto, abrir pelo cache (só leitura) |
| S2 | Salvar de volta (CSV, JSON, texto), recentes, pasta remota que volta, "Esquecer esta conexão" |
| S3 | O banco remoto como cópia local editável: Enviar ao servidor com a conferência (sem mudança lá, sem `-wal`/`-journal`), Atualizar, a pergunta ao fechar |
| P1 em diante | PostgreSQL (o contrato de fonte de dados), com o túnel SSH do S1 |

---

## 3. Decisões para o Isaías

1. ~~Lugar~~ **Decidido em 21/10/2026:** o SSH abre a 0.4, antes do PostgreSQL; o spike da biblioteca corre no fim
   da 0.3.
2. ~~Biblioteca~~ **Decidido em 21/10/2026:** `russh` + `russh-sftp` (dependências aprovadas), com o spike antes; se
   ele falhar no Windows, o plano B é o `ssh`/`sftp` do sistema.
3. ~~Escrita remota~~ **Decidido em 21/10/2026:** a primeira versão já salva de volta CSV, JSON e texto, com a
   conferência de conflito.
4. ~~Senhas~~ **Decidido em 21/10/2026:** no cofre do sistema pelo `keyring` (dependência aprovada), só com
   "Lembrar" marcado pela pessoa.
5. ~~Banco SQLite remoto~~ **Decidido em 21/10/2026:** cópia local editável, com consultas, e Enviar ao servidor com a
   conferência; o modo ao vivo fica para depois.

Todas as decisões do SSH estão tomadas.
