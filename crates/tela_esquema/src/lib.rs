//! Estrutura, Índices e gatilhos, Definição SQL, Nova tabela e Novo índice.
//!
//! # API (o contrato com a casca, crate editor)
//!
//! ```text
//! AbaEstrutura::nova(&TabelaAberta) · ui(&mut self, ui, ctx: &mut Contexto, tabela: &mut TabelaAberta)
//! AbaIndices::nova(&TabelaAberta)   · ui(…mesmo…)
//! AbaDefinicao::nova(&TabelaAberta) · ui(…mesmo…)
//!     (as três com impl estado::ParteTabela: o conteudo é o corpo do ui, e o ui é o desenhar_parte_empilhada)
//! JanelaNovaTabela::nova() · mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto) -> bool   // false = fechou
//! JanelaNovoIndice::nova(tabela: String, colunas: Vec<String>) · mostrar(…mesmo…) -> bool
//!                  · criou() -> bool   // fechou pelo "Criar índice"   · tabela() -> &str
//! ```
//!
//! Também públicas (regras puras, úteis à casca e aos testes): [`texto_referencia`],
//! [`pode_editar`], [`motivo_sem_edicao`], [`condicao_indice`], [`formatar_create`], [`script_definicao`],
//! [`nome_sugerido`].
//!
//! # O que a casca precisa saber
//!
//! - A Estrutura está no formato de documento da prancheta 6 (passo 11 da etapa 3): `anatomia()` com a faixa
//!   ("Colunas · 7 colunas") e a página da casca; a lista de colunas vai de ponta a ponta
//!   (`interface::pecas::documento::de_ponta_a_ponta`), e as Chaves estrangeiras e as Restrições vêm depois, com o
//!   `padding` da página. A linha clicada fica selecionada (`tr.sel`); o nome seleciona e abre "Alterar a coluna".
//!   Ofertas: a Nova coluna (o Novo desligado, D2 c), o Excluir da coluna selecionada ("Excluir a coluna email",
//!   desligado na chave primária e na tabela de uma coluna só) e a busca "Buscar colunas". Propriedades: o
//!   `acoes_da_tabela` ("Renomear a tabela", no fim da seção Tabela) e o `propriedades` (a "Coluna" selecionada, com
//!   "Alterar a coluna", e a zona "Excluir a tabela", recolhida e em `--perigo`); escondido o painel, as mesmas seções
//!   servem para o fim do documento.
//! - Índices e gatilhos e Definição SQL estão no formato de documento (passo 12, pelo sistema visual, sem prancheta):
//!   - Índices: a faixa "Índices · 3 índices" e a página da casca. A lista dos índices vai de ponta a ponta, com a
//!     seleção da linha e o Excluir em ícone (só nos criados); os "Gatilhos" (título 13/600 com a contagem) vêm
//!     depois, com o SQL nas cores do código, "Copiar" e "Excluir". Ofertas: o "Novo índice", o Excluir do índice
//!     selecionado ("Excluir o índice idx_x", ligado só nos criados), sem busca. Propriedades: a seção "Índice" da
//!     linha selecionada (Nome, Colunas, Único, Origem e Condição).
//!   - Definição: a faixa "Comando de criação" com o "Copiar" na ponta e a rolagem própria (o texto, nas cores do
//!     código, rola por dentro). Ofertas: o Excluir da visão ("Excluir a visão v", desligado em só leitura), sem
//!     busca. Propriedades: "Abrir na Consulta SQL", com os dois comandos prontos (`estado::secao_abrir_na_consulta`,
//!     a mesma seção dos Dados).
//! - As janelas de coluna, de renomear, de novo índice da aba e as confirmações são das abas e se desenham dentro do
//!   conteúdo delas (são `egui::Modal`); as que as Propriedades e a barra abrem aparecem no quadro seguinte.
//! - As janelas de edição seguem as pranchetas 12b e 12c (passo 17): o cabeçalho com o sólido da tabela (a chave anil
//!   da tabela sem rowid, D6: `Icone::TabelaSemRowid`) e o sub com o nome dela; "Cancelar" antes do botão principal;
//!   a prévia ("SQL que será executado") com um comando por bloco, sem a linha em branco entre eles; e o aviso do
//!   plano em caixa compacta e centrada, até 560px. "Alterar a coluna {nome}" / "Nova coluna": Nome · Tipo (com a
//!   pastilha de 16px, que acompanha o tipo digitado), Valor padrão · Verificação (CHECK), Ordenação (COLLATE) ·
//!   Referência, e as caixas em fluxo (`gap:10px 18px`). "Nova tabela": a grade da prancheta 12c, com campos de 26px
//!   (`Entrada::pequeno`, `EntradaSugestoes::pequeno`), as três caixas sem texto centradas nas colunas de 84, 80 e 52px
//!   (o leitor de tela ouve "id: chave primária") e o × (`Icone::Fechar`) que tira a coluna.
//! - Depois de mudar a estrutura pela aba (coluna, reordenar, índice, gatilho), a aba põe
//!   `tabela.recarregar = true` e emite `Acao::RecarregarObjetos`: a casca relê a tabela e **refaz as abas**
//!   (`nova`). Renomear a tabela emite `RecarregarObjetos` + `Navegar(Rota::objeto_na_aba(novo, false,
//!   Aba::Estrutura))`; excluir a tabela ou a visão, `RecarregarObjetos` + `Navegar(Rota::Banco)`; os links da
//!   Definição SQL, `Navegar(Rota::consulta_com(sql))`.
//! - `JanelaNovaTabela`: ao criar, emite `RecarregarObjetos` + `Navegar(Rota::objeto_na_aba(nome, false,
//!   Aba::Estrutura))` e devolve false.
//! - `JanelaNovoIndice` (a que a casca abre por `Acao::AbrirNovoIndice`): lê as colunas da tabela pelo serviço
//!   no primeiro quadro (`carregando` "Lendo a tabela…" na lista enquanto isso); ao criar, emite
//!   `RecarregarObjetos`, devolve false e `criou()` passa a dizer sim (fechada pelo ✕, Esc ou fundo, não emite
//!   nada e `criou()` fica false). **A casca deve então reler a tabela aberta**, se for a `tabela()` (a dica de
//!   ordenação lenta da aba Dados some), como faz com `tabela.recarregar`.
//! - Um erro novo no alto de um card (mover ou excluir coluna, excluir índice, gatilho ou visão) ou no alto de uma
//!   janela faz a rolagem em volta (a página da casca, o corpo da janela) andar o mínimo para ele aparecer, uma
//!   vez : a casca não precisa fazer nada além de pôr as abas num
//!   `ScrollArea`.
//! - O que abre e fecha nas abas (a zona "Excluir a tabela", o SQL de cada gatilho) nasce fechado em cada aba nova
//!   (`nova`). A coluna selecionada também volta a nenhuma.
//! - Enquanto um "Aplicar" está a caminho, ✕/Esc/fundo não fecham a janela (a resposta chega em instantes).
//! - Toda mudança passa por `servico.esquema(banco_id, &OperacaoEsquema, previa)` numa tarefa de fundo; a
//!   prévia (`previa: true`) sai 300 ms depois da última mudança nos campos, e a resposta de uma prévia que já
//!   foi trocada por outra é descartada. Erros: `ctx.reagir(&e)` primeiro. Toast:
//!   `ctx.aviso_de_gravacao(transacao_aberta, descricao)`.
//!
//! # Peças próprias (o crate `interface` não tem a variante)
//!
//! Em `apoio.rs`: o `details.modelo` com o nome em negrito e as ações coladas no `pre` (os gatilhos: o `Modelo` do
//! kit usa texto normal e 8px entre o `pre` e os botões), o painel rebaixado (`.ref-detalhes`, `.lista-indice`), o
//! título da prévia (o `.tit` do `.blk`, 16px depois do bloco de cima), o fluxo que quebra linha com os
//! itens medidos (`.checks-coluna`), o `code` que quebra (restrições e condições longas), o título e a contagem da
//! faixa das partes (`titulo_da_faixa`) e o título das listas do documento com o espaço até a lista (`titulo_lista`
//! e a versão com a contagem, sobre o `pecas::texto::titulo_lista`). As listas usam a `pecas::tabela::moldura_lista`
//! da Visão geral, e o "Abrir na Consulta SQL" é o do `estado`. Na lista do Novo
//! índice, o nome da coluna é desenhado ao lado de uma `Check` sem texto, porque fica em 800 quando escolhida e
//! encurta com "…" (o `Check` do kit não tem nem uma coisa nem outra); clicar nele também marca.
//!
//! Em `estrutura.rs`, a lista de colunas tem as larguras da prancheta 6 (# 44, Nome 150, Tipo 120, Padrão 150 e as
//! ações 96; as Regras ficam com o resto) e nada quebra linha (item 18 da revisão): as Regras mostram as pills que
//! cabem e o "+N" (`rotulos::etiqueta_mais`) com as outras na dica, e o Padrão encurta com "…" e o texto inteiro na
//! dica.
//!
//! # Limites conhecidos
//!
//! - O nome sugerido do índice tira os acentos das letras latinas (Latin-1 e Latin Extended-A) por uma tabela,
//!   sem a normalização NFD completa: uma letra de outro alfabeto com marca vira "_".
//!
//! # Testes
//!
//! `src/testes/` (unitários do crate, para ver o estado das janelas): cada cenário das abas e janelas, e a
//! janela da sugestão de índice, com `egui_kittest` sobre o
//! banco de exemplo (`estado::apoio_testes`), conferindo o que aparece, as ações emitidas e o arquivo do banco
//! (lido por uma conexão própria). Os testes das três abas usam o `palco_doc` (a aba desenhada como a casca, pelo
//! `estado::apoio_testes::palco_documento`: a barra com os verbos e a busca, o documento com a faixa e as
//! Propriedades à direita, e a tabela relida quando a aba pede). A "casca" antiga dos testes põe a aba em x=260,
//! y=230 e relê a tabela quando a aba pede; outros palcos põem a
//! página num `ScrollArea` (para conferir que o erro novo fica à vista). Capturas em
//! `capturas/rust/`: `estrutura-clientes-claro.png`,
//! `estrutura-clientes-zona-risco-claro.png`, `janela-coluna-alterar-claro.png`,
//! `janela-coluna-reconstroi-claro.png`, `indices-pedidos-claro.png`, `novo-indice-claro.png`,
//! `definicao-clientes-claro.png`, `nova-tabela-claro.png`, `estrutura-leitura`, `estrutura-notas`
//! (tabela virtual), `estrutura-1000` e `estrutura-pedidos-960` (janelas de 1000 e 960px), `estrutura-pedidos`,
//! `estrutura-produtos`, `estrutura-itens-pedido`, `erro-no-card`, `confirmar-excluir-coluna`,
//! `confirmar-mover`, `nova-coluna`, `nova-coluna-erro-previa`, `janela-coluna-fk`, `janela-gerada`,
//! `renomear`, `renomear-previa`, `indices-clientes`, `indices-eventos`, `indices-leitura`,
//! `novo-indice-clientes`, `definicao-visao`, `nova-tabela-inicial`, `nova-tabela-erro-previa` e
//! `nova-tabela-erro-alto` (todas `-claro.png`) e `indices-pedidos-escuro.png`, e ainda
//! `estrutura-clientes-escuro.png`, `janela-coluna-alterar-escuro.png`, `erro-no-card-rolado-claro.png`,
//! `nova-tabela-rolada-claro.png` e `nova-tabela-erro-rolado-claro.png`.

#![forbid(unsafe_code)]

mod apoio;
mod definicao;
mod estrutura;
mod indices;
mod mudanca;
mod nova_tabela;

#[cfg(test)]
mod testes;

pub use apoio::{
    condicao_indice, formatar_create, motivo_sem_edicao, nome_sugerido, pode_editar, script_definicao, texto_referencia,
};
pub use definicao::AbaDefinicao;
pub use estrutura::AbaEstrutura;
pub use indices::{AbaIndices, JanelaNovoIndice};
pub use nova_tabela::JanelaNovaTabela;
