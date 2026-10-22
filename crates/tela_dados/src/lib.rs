//! Aba Dados da tela de tabela ou visão.
//!
//! # API (o contrato com a casca, §4.8 de ARQUITETURA.md)
//!
//! ```text
//! AbaDados::nova(tabela: &TabelaAberta) -> AbaDados
//! AbaDados::ui(&mut self, ui: &mut egui::Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta)
//! impl estado::ParteTabela for AbaDados   // o contrato da etapa 3: anatomia (faixa, rodapé, rolagem própria) ·
//!                                         // faixa · conteudo · rodape · ofertas/executar · busca · propriedades ·
//!                                         // em_segundo_plano; o ui é o estado::desenhar_parte_empilhada
//! ```
//!
//! - `nova` não toca no serviço: guarda a consulta da rota (`Rota::Tabela`/`Rota::Visao` com `ConsultaDados`:
//!   página, por página, ordem, direção, busca, filtros), validada (ordem e filtros só de colunas
//!   visíveis, por página de 1 a 1000). A primeira página é pedida no primeiro `ui`.
//! - `ui` desenha a faixa, o conteúdo e o rodapé um embaixo do outro (na casca, a peça do documento os põe nos
//!   lugares dela) e as janelas da aba por cima (janela da linha, confirmações). A cada quadro:
//!   - mudou a página, o tamanho, a ordem, a busca ou os filtros: `tabela.rota` recebe a consulta nova e sai
//!     `Acao::Substituir(rota)` (a tela não é refeita e não entra no histórico: Alt+← sai da tabela em vez de percorrer cada página ou
//!     busca). Se a casca trocar `tabela.rota` por fora sem refazer a aba, a aba segue a rota nova;
//!   - `tabela.selecao` acompanha as linhas marcadas (o Exportar da barra oferece "Só as linhas marcadas") e `tabela.definir_contagem_filtrada(busca, filtros, n)` recebe o total já contado da busca e
//!     dos filtros (os da rota, só os completos);
//!   - incluir ou excluir linhas pede `tabela.recarregar = true` (a casca relê a tabela e os objetos e refaz as
//!     abas; o foco pendente na linha nova atravessa a troca pelo contexto do egui). Se a casca baixar o
//!     `recarregar` sem refazer a aba, ela mesma relê a página;
//!   - "Criar índice" da dica de ordenação lenta emite `Acao::AbrirNovoIndice { tabela, colunas: [coluna] }`;
//!   - as gravações avisam no toast com `ctx.aviso_de_gravacao` (que também emite `Acao::NotarTransacao`);
//!     erros de tarefa passam antes por `ctx.reagir` (substituída e banco trocado não aparecem).
//! - Fora de vista, a casca chama o `ParteTabela::em_segundo_plano`: as gravações, a exclusão e as tarefas da ficha
//!   que terminam com a aba escondida dão o toast e notam a transação na hora, sem esperar a aba voltar.
//! - O Ctrl+Z e o Editar › Desfazer são o `Verbo::Desfazer` da casca. O toast da gravação de uma célula traz a ação
//!   "Desfazer · Ctrl+Z" com a carga [`PREFIXO_ACAO_TOAST`]`…`: a casca a devolve ao documento (como a do CSV), que
//!   a toma com `acao_do_toast` no quadro dele.
//!
//! # O que a aba faz (TELAS.md › Aba Dados)
//!
//! - Formato de documento (etapa 3, prancheta 2; `ParteTabela::anatomia`: faixa, rodapé e rolagem própria). Faixa
//!   (as peças de `interface::pecas::filtros`, que o CSV também usa): "Filtros", uma etiqueta `chip_filtro` por condição ("ativo igual a 1": o clique abre a condição no popover, o ×
//!   a tira), "+ Condição" e, à direita, a contagem ("41 de 60 linhas", "60 linhas"; "contando…" e "linhas não
//!   contadas", com o motivo na dica, na tabela grande) e "Limpar filtros" (só com condição). O popover de vidro
//!   (`interface::pecas::popover`) tem Coluna, Operador (os 14 rótulos), Valor (fora nos operadores sem valor),
//!   Cancelar e Aplicar; Enter aplica e Esc desiste.
//! - Verbos da barra (`ParteTabela::ofertas`/`executar`): Nova linha; Duplicar com um alvo só (a marcada única ou a
//!   linha da célula ativa: lê a linha inteira e abre a "Nova linha" preenchida; com várias marcadas, desligado, "Duplicar
//!   (marque uma linha só)"); Excluir (as marcadas ou a linha da célula ativa, com a confirmação "Excluir N linhas de
//!   X? Isso não pode ser desfeito."); Filtrar (o popover numa condição nova; `.ib.on` com condição, "Filtrar (1
//!   condição)"). Visão ou banco só leitura: só Filtrar e Buscar. A tecla Delete exclui as marcadas (com a pergunta); sem marcadas, limpa o intervalo.
//! - Propriedades: "Coluna" da célula ativa (Tipo com a pastilha, Regras em pills, Padrão, Valor) e "Abrir na Consulta
//!   SQL" (`estado::secao_abrir_na_consulta`), depois da "Tabela" que a tela de tabela desenha.
//! - Busca: a da barra de ferramentas da casca ("Buscar em clientes", `ParteTabela::busca`; o Ctrl+F é da casca):
//!   aplica 300 ms depois da última tecla, Enter na hora, Esc limpa como o `input[type=search]`.
//! - Grade sobre `interface::grade::NucleoGrade` (`Navegacao::Dados`): caixas de marcar (a do cabeçalho
//!   indeterminada, `Toggled::Mixed`, com parte da página marcada) e coluna do número da linha (fixas e fora da
//!   navegação; a coluna apelido do rowid é o próprio abridor), `th` com nome e tipo que ordena asc → desc → sem
//!   ordem, células do `interface::grade`, célula ativa com o contorno de 2px em `--acc`, edição no lugar
//!   (`Entrada::compacta`, de 26px, o texto selecionado na cor do texto: duplo clique/Enter/F2 ou uma tecla de
//!   texto começam; Enter grava, Esc
//!   desiste, Tab grava e anda, perder o foco grava; pisca o `pintar_salvo`; BLOB, texto cortado, texto com
//!   quebra e coluna gerada abrem a janela), Delete com marcadas pergunta (sem marcadas, limpa), espaço marca, setas/PageUp/PageDown/
//!   Home/End movem. Começar a editar uma célula fora da vista rola a grade até ela; o campo só grava depois de
//!   ter aparecido com o foco (o que se digita enquanto a grade rola vai para ele). As colunas ficam com a
//!   largura do conteúdo (até 320px) e a sobra do documento é repartida entre as de dados na proporção do
//!   conteúdo (a tabela `min-width:100%` do navegador); a pessoa ainda pode arrastar as bordas. Sem linhas, o
//!   vazio compacto (6px em cima e embaixo).
//! - Gravações no lugar em fila: cada uma espera a anterior e sai com a chave que a fila deixou na própria linha
//!   (editar a PK e logo outra célula da mesma linha; a chave segue a linha, não a chave antiga); a releitura da página espera a fila. A resposta de uma gravação feita com Enter só
//!   devolve o foco à célula se a pessoa não seguiu adiante (outra célula, outra edição). Uma gravação que falha
//!   depois de a aba sumir avisa no toast ("O valor de X em Y não foi salvo: …").
//! - Desfazer no banco (prancheta 15, revisão de UI de 19/10/2026; `desfazer.rs`): a gravação de uma célula avisa
//!   no toast onde gravou, "Gravado em clientes (linha 3, email)", com "Desfazer · Ctrl+Z" por 6 s
//!   ([`PRAZO_TOAST`]; com a aba fora de vista, o toast sem a ação). O verbo Desfazer da barra (Ctrl+Z, Editar ›
//!   Desfazer) faz o mesmo enquanto não houver outra mudança, com a dica "Desfazer email da linha 3 (Ctrl+Z)":
//!   grava o valor antigo com um UPDATE (`Servico::desfazer_celula`), só se a célula ainda tem o que a edição
//!   gravou (senão 409, nada muda), e avisa "Edição desfeita em clientes (linha 3, email)"; a célula volta e pisca.
//!   A última edição é uma só para o aplicativo (outra edição a troca) e atravessa a aba refeita; não há Refazer.
//!   Excluir e incluir linhas, mudar a estrutura e uma transação da Consulta SQL que começou ou terminou depois
//!   da edição deixam o Desfazer desligado, com o porquê na dica ([`DICA_EXCLUIR`], [`DICA_INCLUIR`],
//!   [`DICA_ESTRUTURA`], [`DICA_TRANSACAO`]); salvar a linha pela ficha só tira a última edição. Visão e banco só
//!   leitura não oferecem o Desfazer; dentro de uma transação aberta, o UPDATE entra nela, como a edição.
//! - Menu da célula (0.2.3, `menu.rs`): o clique direito numa célula ou num intervalo e o Shift+F10: Copiar, Copiar
//!   com cabeçalho (também o Ctrl+Shift+C) e Copiar como (o `INSERT` com o nome da tabela), Filtrar por este valor e
//!   Excluir este valor do filtro (o NULL vira "é nulo", o texto vazio "vazio"; o BLOB e o texto cortado desligam),
//!   Marcar as linhas, Abrir a ficha da linha; Colar (também o Ctrl+V: o TSV a partir da célula de cima à esquerda do
//!   intervalo, por cima das células; um valor só preenche o intervalo; o que passa da última linha da página ou da
//!   última coluna fica de fora e o aviso diz quantas), Limpar (também o Delete sem linhas marcadas: o texto vazio nas
//!   colunas de texto, o NULL nas outras, o DEFAULT na `NOT NULL` que tem um), Definir como NULL e Excluir as linhas
//!   (a pergunta e o caminho do Excluir da barra). O Colar, o Limpar e o Definir como NULL gravam numa transação só
//!   (`Servico::atualizar_intervalo`, tudo ou nada), perguntam acima de 100 células ("Colar 120 células em
//!   clientes?"), avisam "Gravado em clientes (9 células)" com o "Desfazer · Ctrl+Z", e o Desfazer volta todas de uma
//!   vez (`Servico::desfazer_intervalo`; se uma mudou depois, nenhuma volta e o aviso diz qual), com as regras do
//!   Desfazer de uma célula. A coluna gerada, a chave do rowid, a `NOT NULL` (no NULL; no Limpar, só sem DEFAULT), a
//!   linha sem chave, o texto cortado e o BLOB (o Desfazer não os grava de volta), o banco só leitura e uma gravação a
//!   caminho desligam o item, com o motivo na dica (o Ctrl+V e o Delete dizem o motivo no toast). Com mais de uma
//!   célula, as contas da seleção vão no rodapé, antes das páginas.
//! - Rodapé (34px): "Linhas a–b de N" e "· N marcadas" · "N por página" (50/100/250/500, mais o da rota, 26px) · ‹
//!   "Página X de Y" › (`.ib.pq`); a dica de página lenta numa linha de informação acima dele (o ícone Informação e
//!   "Criar índice").
//! - Cabeçalho da grade: a pastilha de 16 pelo tipo declarado (`TipoColuna::do_declarado`), com o tipo por extenso
//!   na dica ("INTEGER · chave primária").
//! - Tabela grande (`TabelaInfo.linhas` None ou acima de `LIMIAR_TABELA_GRANDE`) com busca ou filtro: página com
//!   `contar=false` e a contagem à parte (`servico.contagem`), com "contando…"; a contagem de uma busca vale para
//!   as outras páginas dela; a última página (ou uma busca vazia) já diz o total. Leituras com os grupos
//!   `grade:<tabela>` e `contagem:<tabela>`; a abandonada (409 `substituido`) é ignorada, e refeita sem o grupo
//!   se ainda era a que valia.
//! - Ficha da linha (`Janela` larga, prancheta 12a; `janela.rs`): "Linha 12" / "Linha de pedido_id 7 · produto P003" /
//!   "Nova linha", com a ficha da tabela e o nome dela depois do título. O corpo é a `.ficha`, uma linha por coluna em
//!   grade 250 | 1fr | 64 (gap 12, padding 9 2, fio `--b`), com o cabeçalho "Coluna · Valor · NULL": na esquerda, a
//!   pastilha de tipo de 24 e o nome em 600 e embaixo as regras em 11px `--t3` ("INTEGER · chave primária ·
//!   autoincremento"); no meio, o valor (`Entrada`; números à direita; área de 62px no texto longo; BLOB com a
//!   pílula do tamanho e os `btn pq` "Salvar em arquivo" e "Substituir por arquivo" / "Escolher arquivo"; coluna
//!   gerada num campo só leitura); na direita, a caixa "NULL" (`aria-label` "email é NULL"), só onde a coluna aceita
//!   nulo. Marcado, o NULL deixa o campo normal, com "NULL" em itálico (`Entrada::fixo`). Na linha nova, "Usar o
//!   padrão" fica dentro da célula do valor, à direita. Os valores são crus, como na grade (`texto_celula`). Foco no
//!   primeiro campo de digitar (ou no da célula que a pessoa tentou editar); rodapé "Excluir linha" · "Duplicar" ·
//!   "Cancelar" · "Salvar" / "Cancelar" · "Incluir linha"; erro do serviço num aviso de perigo no alto.
//! - Só leitura (visão, banco só leitura e linha sem chave; prancheta 15): a mesma ficha com os campos só leitura
//!   (opacos, sem baixo-relevo), as caixas NULL desligadas (marcadas onde o valor é NULL), o BLOB só com "Salvar em
//!   arquivo", a nota compacta "exemplo.db está aberto só para leitura. Nada é gravado no arquivo." (`TomAviso::Nota`;
//!   o aviso de atenção na linha sem chave) e o rodapé "Copiar a linha" (uma coluna por linha, "nome: valor") ·
//!   "Fechar". O texto selecionado dos campos fica na cor do texto (o tema do `interface` o pinta em ouro, ilegível
//!   sobre o fundo da seleção).
//!
//! # Testes
//!
//! `tests/dados.rs`, `tests/volumes.rs` (tabelas grandes) e `tests/capturas.rs` (as capturas em
//! `capturas/rust/`), pelo `estado::apoio_testes`. Os ganchos `#[doc(hidden)]` ([`Ganchos`],
//! [`AbaDados::com_ganchos`], [`AbaDados::retrato`] e os métodos de inspeção) existem só para eles:
//! página lenta, contagem que espera, gravação lenta, os caminhos que os diálogos do
//! Windows devolveriam e a leitura do que a aba mostra. O `tests/apoio` cria, renderiza e desfaz cada palco
//! (um wgpu por harness) uma thread de cada vez: vários ao mesmo tempo às vezes derrubavam o processo dentro
//! do driver (0xC0000005).

#![forbid(unsafe_code)]

mod aba;
mod desfazer;
mod grade;
mod janela;
mod menu;
mod modelo;
mod regras;

use std::path::PathBuf;
use std::sync::Arc;

use banco::{Pagina, PedidoPagina};

pub use aba::{AbaDados, Retrato};
pub use desfazer::{
    DICA_ESTRUTURA, DICA_EXCLUIR, DICA_INCLUIR, DICA_TRANSACAO, PRAZO_TOAST, PREFIXO_ACAO_TOAST, ROTULO_ACAO,
    frase_desfeito, frase_desfeito_lote, frase_gravado, frase_gravado_lote,
};

/// Gancho de teste sobre cada página lida (e o pedido dela).
#[doc(hidden)]
pub type GanchoPagina = Arc<dyn Fn(&PedidoPagina, &mut Pagina) + Send + Sync>;
/// Gancho de teste antes de cada contagem à parte (a busca e o grupo da leitura).
#[doc(hidden)]
pub type GanchoContagem = Arc<dyn Fn(Option<&str>, Option<&str>) + Send + Sync>;
/// Gancho de teste sem argumentos.
#[doc(hidden)]
pub type Gancho = Arc<dyn Fn() + Send + Sync>;

/// Ganchos para os testes: mexer na página lida (um
/// `tempo_ms` alto), segurar a contagem à parte (esperar um sinal) e responder no lugar dos diálogos do Windows.
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct Ganchos {
    /// Chamado na thread da tarefa com cada página lida (e o pedido dela).
    pub pagina: Option<GanchoPagina>,
    /// Chamado na thread da tarefa quando uma leitura da grade ou da contagem volta abandonada por outra mais
    /// nova do mesmo grupo (409 `substituido`).
    pub abandonada: Option<Gancho>,
    /// Chamado na thread da tarefa antes de cada contagem à parte, com a busca e o grupo da leitura.
    pub contagem: Option<GanchoContagem>,
    /// Chamado na thread da tarefa antes de cada gravação no lugar (uma máquina lenta: a próxima edição sai antes
    /// da resposta).
    pub gravacao: Option<Gancho>,
    /// O caminho que o diálogo "Salvar como" devolveria.
    pub salvar_como: Option<PathBuf>,
    /// O caminho que o diálogo "Escolher arquivo" devolveria.
    pub escolher_arquivo: Option<PathBuf>,
}
