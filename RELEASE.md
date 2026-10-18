# Como publicar o DataBox

Duas partes: pôr o repositório no GitHub (uma vez) e publicar cada versão (o `DataBox.exe` para download).

## 1. Pôr o repositório no GitHub (uma vez)

1. Crie o repositório em <https://github.com/new>: nome `databox`, **Public**, **sem** README, `.gitignore` nem
   licença (o projeto já tem os três).
2. No PowerShell, na pasta do projeto (troque `SEU-USUARIO`):

   ```powershell
   git remote add origin https://github.com/SEU-USUARIO/databox.git
   git push -u origin main
   ```

   Com o [GitHub CLI](https://cli.github.com) dá para fazer os dois passos de uma vez:
   `gh repo create databox --public --source . --push`.
3. Na página do repositório, em **About** (a engrenagem), ponha a descrição ("Caixa de ferramentas de dados para
   Windows: SQLite, CSV, JSON e Excel") e os tópicos (`rust`, `egui`, `sqlite`, `csv`, `data-tools`, `windows`).
4. Em **Actions**, confira que o workflow **CI** rodou e passou (ele compila, roda o clippy, o fmt e os testes a cada
   push na `main` e em cada pull request).

## 2. Publicar uma versão

Uma "release" do GitHub é uma página ligada a uma **tag** do git (por exemplo `v0.1.0`), com notas e arquivos para
baixar. O workflow [`release.yml`](.github/workflows/release.yml) faz tudo sozinho quando uma tag `v*` chega ao
GitHub: compila o `DataBox.exe` otimizado e cria a página com o executável, um `.zip` (executável, licença e
`THIRD-PARTY-NOTICES.txt`, os avisos de terceiros que o `build.rs` do editor gera) e o
`SHA256SUMS.txt`.

### Passo a passo

1. **Atualize a versão** em `Cargo.toml` (`[workspace.package] version`) e em `crates/editor/recursos/databox.rc`
   (`FILEVERSION`, `PRODUCTVERSION` e os textos `FileVersion`/`ProductVersion`). Para a 0.1.0, já está feito.
2. **Atualize o [`CHANGELOG.md`](CHANGELOG.md):** mova o que está em "Não lançado" para uma seção nova com a versão e
   a data.
3. Rode os testes (`.\cargo.ps1 test --workspace`) e faça o commit (`git commit -am "chore: versão 0.2.0"`).
4. **Crie a tag e envie:**

   ```powershell
   git tag v0.1.0
   git push origin main
   git push origin v0.1.0
   ```

5. Acompanhe em **Actions › Release** (uns 15 minutos). No fim, a versão aparece em **Releases**, e o link
   `../../releases/latest` do README passa a baixar ela.

A tag precisa bater com a versão do `Cargo.toml` (`v0.1.0` ↔ `0.1.0`); se não bater, o workflow para antes de
publicar. Se algo der errado depois de criada, apague a versão na página de Releases e a tag
(`git push --delete origin v0.1.0` e `git tag -d v0.1.0`), corrija e repita.

### Sem o workflow (à mão)

```powershell
.\cargo.ps1 build --release -p editor
gh release create v0.1.0 target\release\DataBox.exe --title "DataBox v0.1.0" --notes-file CHANGELOG.md
```

Ou, pelo site: **Releases › Draft a new release**, escolha ou crie a tag, escreva as notas, arraste o `DataBox.exe` e
clique em **Publish release**.

## Sobre o aviso do Windows (SmartScreen)

O executável não é assinado digitalmente, então o Windows pode mostrar "O Windows protegeu o computador" nos
primeiros downloads; a pessoa clica em **Mais informações › Executar assim mesmo**. O aviso diminui conforme o
arquivo ganha reputação. Para eliminá-lo, é preciso assinar o executável com um certificado de assinatura de código
(pago; para projetos de código aberto existe, por exemplo, o programa gratuito da SignPath Foundation).

## Versionamento

O projeto segue o [Versionamento Semântico](https://semver.org/lang/pt-BR/): enquanto estiver em `0.x`, versões
`0.(x+1).0` trazem recursos novos e podem mudar comportamento; `0.x.(y+1)` são correções.
