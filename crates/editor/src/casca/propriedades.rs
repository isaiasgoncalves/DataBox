//! O painel de Propriedades (o `aside` da direita de `Main.dc.html`, `Dados.dc.html` e `Consulta.dc.html`): 272px
//! ([`LARGURA`], a padrão: a divisória à esquerda dele muda a largura, de 200 a 45% da janela; ver
//! [`crate::casca::larguras`]) com o cabeçalho `.cab` "Propriedades", sobre a peça `interface::pecas::painel` ([`Painel`]), com a
//! rolagem própria de cada aba e as seções que o documento à vista desenha (`estado::Documento::propriedades`):
//!
//! - **Visão geral**: "Arquivo" (os nove campos, o Caminho numa linha com o Copiar caminho) e "Manutenção" (as
//!   verificações, Otimizar, Compactar e a cópia de segurança, com o resultado embaixo);
//! - **tabela ou visão**: "Tabela" (ou "Visão", o `estado::secao_tabela`: Nome, Tipo, Linhas, Colunas, Índices,
//!   Gatilhos, Sem rowid e Estrita; a visão não tem índices, rowid nem é estrita), as seções da parte e o "Arquivo"
//!   curto (Caminho e Tamanho);
//! - **Consulta SQL**: "Execução" (Comandos, Executados, Parou em, com o "Comando 3" em `--perigo`, e Transação:
//!   "aberta" ou "nenhuma aberta") e "Tabelas" (as tabelas e visões do banco, com a ficha de cada uma e o Inserir,
//!   que põe o nome citado no cursor do editor).
//!
//! O Início não tem: o painel não é desenhado e o documento fica com a largura. Mostrar ou esconder (o botão da barra
//! de ferramentas, Ctrl+Alt+B) fica só na sessão; com o documento abaixo de [`DOCUMENTO_MINIMO`], o painel some sem
//! mudar a escolha da pessoa (`flex:999 1 560px` da seção do documento nas pranchetas). Com o painel escondido (pela
//! pessoa ou pela largura), as telas de rolagem de página mostram as mesmas seções no fim do documento
//! (`estado::propriedades_no_fim`, chamado no `app.rs`). As seções abrem e fecham no clique do título.

use egui::{ScrollArea, Ui};
use estado::{Contexto, IdAba};
use interface::pecas::painel::Painel;

use crate::documento::Documento;

/// A largura padrão do painel (`aside{width:272px}`); a pessoa a muda arrastando a divisória
/// ([`crate::casca::larguras`]).
pub(crate) const LARGURA: f32 = 272.0;
/// Abaixo desta largura do documento, o painel some, depois de os painéis encolherem até o mínimo (a base do
/// `flex:999 1 560px` da seção do documento).
pub(crate) const DOCUMENTO_MINIMO: f32 = 560.0;

/// O painel inteiro no espaço de `ui`, com as seções do documento e a rolagem própria de cada aba.
pub(crate) fn painel(ui: &mut Ui, documento: &mut Documento, aba: IdAba, ctx: &mut Contexto) {
    Painel::new("propriedades", "Propriedades").mostrar(ui, |ui| {
        ScrollArea::vertical().id_salt(("propriedades", aba)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            documento.doc().propriedades(ui, ctx);
        });
    });
}
