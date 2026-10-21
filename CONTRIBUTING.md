# Contribuindo com o DataBox

Obrigado pelo interesse! Issues com bugs, ideias e pull requests são bem-vindos.

> **English speakers:** issues and pull requests in English are welcome too. The code base itself (identifiers,
> comments, UI strings) is written in Brazilian Portuguese; please follow that in code changes.

## Antes de começar

- Para mudanças grandes (um formato novo, uma ferramenta nova, mudança de arquitetura), abra uma issue primeiro para
  combinar o caminho.
- Leia o [`ARQUITETURA.md`](ARQUITETURA.md) (o contrato entre os crates e as convenções) e o topo do `lib.rs` do crate
  que você vai mexer: é lá que a API de cada um está documentada.
- Mudanças visuais seguem o [`design/README.md`](design/README.md) e as pranchetas de `design/canvas`.

## Preparar o ambiente

Siga "Compilar a partir do código" no [`README.pt-BR.md`](README.pt-BR.md#compilar-a-partir-do-código): Rust com a
toolchain GNU e o MinGW da WinLibs, e **sempre** `.\cargo.ps1` no lugar de `cargo`.

## Convenções

- **Português** nos nomes, comentários, documentação, testes e textos da interface (frases inteiras).
- Erros como `comum::ErroBanco`, com frase pronta para a pessoa; nada de `unwrap`/`expect` com dado da pessoa ou do
  arquivo fora dos testes.
- A thread da interface nunca toca no disco nem no SQLite: use `estado::Promessa` (tarefa de fundo).
- As telas não usam cor solta nem desenham peça própria quando o crate `interface` já tem uma.
- `#![forbid(unsafe_code)]` em todo crate, salvo FFI indispensável com comentário `// SAFETY:`.
- **Dependências novas:** proponha na issue antes. As dependências diretas são MIT e/ou Apache-2.0, com versões
  fixas em `[workspace.dependencies]` do `Cargo.toml` da raiz.
- Quem muda um contrato entre crates atualiza o `ARQUITETURA.md` no mesmo pull request; mudanças que a pessoa vê entram
  no [`CHANGELOG.md`](CHANGELOG.md), em "Não lançado".

## Antes de abrir o pull request

```powershell
.\cargo.ps1 fmt --all
.\cargo.ps1 clippy --workspace --all-targets '--' -D warnings
.\cargo.ps1 test --workspace --exclude editor --exclude interface --exclude 'tela_*'
.\cargo.ps1 test -p editor -p interface -p 'tela_*' '--' --test-threads=1
.\cargo.ps1 test -p editor --no-default-features '--' --test-threads=1
```

O CI roda os mesmos passos no Windows: os testes de núcleo em paralelo e os de tela (os crates `editor`, `interface` e
`tela_*`, que criam o wgpu) numa thread só, porque o renderizador de software do Windows (WARP) cai com
`STATUS_ACCESS_VIOLATION` (0xC0000005) quando vários testes o criam ao mesmo tempo. No Linux tudo roda em paralelo
(`cargo test --workspace`), e as capturas de lá são a referência. O CI não repete testes: um teste que cair duas
vezes por causa do ambiente, e não do código, ganha `#[ignore]` com uma issue aberta, nunca uma retentativa.

## Mensagens de commit

Curtas, no imperativo, em português ou inglês, no formato [Conventional Commits](https://www.conventionalcommits.org/)
quando fizer sentido (`feat(tela_csv): busca dentro do arquivo`, `fix(banco): …`).

## Licença

Ao contribuir, você concorda que a sua contribuição é licenciada sob a [licença MIT](LICENSE) do projeto.
