//! O contrato das telas com a casca: o trait [`Documento`] (o que uma aba
//! de documento diz e faz) e o [`ParteTabela`] (as partes de uma tabela ou visão, que recebem a [`TabelaAberta`]
//! da tela de tabela).
//!
//! A casca desenha o documento da aba ativa em volta do que ele diz da própria [`Anatomia`]: a faixa de contexto
//! de 42px ([`Documento::faixa`]), o conteúdo, com a rolagem da casca ([`Rolagem::Pagina`]) ou a dele
//! ([`Rolagem::Propria`]), e o rodapé de 34px ([`Documento::rodape`]). A cada quadro, antes do conteúdo, ela pede
//! as [`Ofertas`] (os verbos da barra de ferramentas e do menu Editar), executa o verbo clicado
//! ([`Documento::executar`]) e amarra a busca da barra ao texto que o documento empresta
//! ([`Documento::busca`], [`EventoBusca`]); depois do conteúdo, as Propriedades ([`Documento::propriedades`]) e a
//! barra de status ([`Documento::status`]). As abas fora de vista não são desenhadas: recebem só o
//! [`Documento::em_segundo_plano`].
//!
//! **As Propriedades sem o painel**: com o painel escondido (a janela estreita, abaixo
//! de ~1052px com o Explorador, quando nem os dois painéis na largura mínima deixam 560px ao documento; ou o
//! Ctrl+Alt+B), as telas de rolagem de página ([`Rolagem::Pagina`]) recebem a
//! mesma chamada [`Documento::propriedades`] no fim da página, depois do conteúdo, dentro de
//! [`propriedades_no_fim`] (o fio e o `.tit` "Propriedades"). A tela não faz nada para isso: as seções são as
//! mesmas, só que na largura do documento. As de rolagem própria ([`Rolagem::Propria`]) ficam sem elas.
//!
//! O que é do ciclo de vida das abas (o título, o ícone, `serve_para`, `refeito`, `ao_fechar`, `marcar_para_reler`,
//! `esquecer_transacao`) continua no enum de documentos do editor, que guarda os tipos concretos.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use banco::{TabelaInfo, TipoObjeto};
use egui::{Align, Layout, Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_bytes_opt, formatar_data_opt, formatar_numero, formatar_numero_opt};
use interface::icones::Icone;
use interface::pecas::botao::{Botao, LADO_PEQUENO};
use interface::pecas::detalhes::Detalhes;
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::texto::{galley_caminho, y_na_linha};
use interface::pecas::toast::toast_concluido;

use crate::contexto::{Contexto, Estado};
use crate::fonte::FonteDeColunas;
use crate::tabela::TabelaAberta;
use crate::verbo::{Ofertas, Verbo};

/// A altura da faixa de contexto (`.faixa{height:42px}`) e a do rodapé (`.rodape{height:34px}`), as da peça
/// `interface::pecas::documento`, e quem rola o conteúdo ([`Rolagem::Pagina`]: a casca; [`Rolagem::Propria`]: o
/// próprio conteúdo, que recebe o retângulo inteiro, e a casca não intercepta as teclas de rolagem).
pub use interface::pecas::documento::{ALTURA_FAIXA, ALTURA_RODAPE, Rolagem};

/// A anatomia do documento: o que a casca desenha em volta do conteúdo. O padrão é a página sem faixa nem rodapé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Anatomia {
    pub faixa: bool,
    pub rodape: bool,
    pub rolagem: Rolagem,
}

impl Anatomia {
    /// Com a faixa de contexto de 42px.
    pub fn com_faixa(self) -> Anatomia {
        Anatomia { faixa: true, ..self }
    }

    /// Com o rodapé de 34px.
    pub fn com_rodape(self) -> Anatomia {
        Anatomia { rodape: true, ..self }
    }

    /// Com a rolagem própria ([`Rolagem::Propria`]).
    pub fn rolagem_propria(self) -> Anatomia {
        Anatomia { rolagem: Rolagem::Propria, ..self }
    }
}

/// O texto da busca da barra de ferramentas, emprestado pelo documento a cada quadro, com a dica do campo ("Buscar
/// em clientes"). A casca desenha o campo sobre o texto e avisa o documento do que aconteceu nele
/// ([`EventoBusca`]).
pub struct BuscaDaBarra<'a> {
    pub texto: &'a mut String,
    pub dica: Cow<'static, str>,
}

/// O que aconteceu na busca da barra neste quadro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventoBusca {
    /// O texto mudou (a pessoa digitou ou apagou).
    Mudou,
    /// Enter no campo.
    Enter,
    /// Esc no campo com texto: a casca já limpou o texto.
    Limpar,
}

/// Uma aba de documento (o Início, a Visão geral, a tela de tabela ou visão, a Consulta SQL). Só o
/// [`Documento::conteudo`] é obrigatório; o resto tem o padrão de quem não oferece nada.
pub trait Documento {
    /// A faixa, o rodapé e a rolagem (o padrão: a página, sem faixa nem rodapé).
    fn anatomia(&self) -> Anatomia {
        Anatomia::default()
    }

    /// Os avisos de ponta a ponta do documento, antes da faixa (o `.aviso` da prancheta 6, pela
    /// [`interface::pecas::documento::Documento::aviso`]): a transação aberta da Consulta SQL, com Gravar e
    /// Descartar. A casca desenha os dela (a troca de banco que demora) antes destes.
    fn avisos(&mut self, _doc: &mut interface::pecas::documento::Documento, _ui: &mut Ui, _ctx: &mut Contexto) {}

    /// A faixa de contexto (42px, um `Ui` em linha e centrado na altura): filtros, banco da consulta, título da
    /// lista.
    fn faixa(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}

    /// O conteúdo do documento.
    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto);

    /// O rodapé (34px, um `Ui` em linha e centrado na altura): contagem e páginas, linhas e tempo.
    fn rodape(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}

    /// O que o documento oferece de cada verbo agora. Calculadas a cada quadro, sem ir ao serviço.
    fn ofertas(&self, _estado: &Estado) -> Ofertas {
        Ofertas::default()
    }

    /// Um verbo ligado (o clique na barra, o menu Editar, o atalho), antes do [`Documento::conteudo`] no mesmo
    /// quadro. A casca só chama com o verbo ligado nas [`Documento::ofertas`].
    fn executar(&mut self, _verbo: Verbo, _egui: &egui::Context, _ctx: &mut Contexto) {}

    /// O texto da busca da barra (None: a busca fica desligada).
    fn busca(&mut self) -> Option<BuscaDaBarra<'_>> {
        None
    }

    /// O que aconteceu na busca da barra neste quadro (antes do [`Documento::conteudo`]).
    fn evento_busca(&mut self, _evento: EventoBusca, _egui: &egui::Context, _ctx: &mut Contexto) {}

    /// O documento tem o painel de Propriedades (sem ele, o painel não aparece e o documento fica com a largura).
    fn tem_propriedades(&self) -> bool {
        false
    }

    /// As seções do painel de 272px, depois do conteúdo no mesmo quadro. Com o painel escondido e a
    /// [`Rolagem::Pagina`], a casca chama o mesmo no fim da página, na largura do documento, dentro de
    /// [`propriedades_no_fim`] (a decisão 4; veja o topo do módulo): as seções não sabem onde estão.
    fn propriedades(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}

    /// As Propriedades rolam pela casca (o painel põe as seções numa rolagem). Sem ela (false), o documento recebe o
    /// painel na altura dele e cuida da rolagem: a tabela, nas vistas Colunas, Índices e SQL (revisão de UI de
    /// 19/10/2026), põe o segmentado fixo em cima e o texto da Definição com a rolagem dele.
    fn propriedades_rolam(&self) -> bool {
        true
    }

    /// O que a aba diz de si na barra de status ("clientes · 60 linhas · 7 colunas", "SQL").
    fn status(&self, _estado: &Estado) -> Option<String> {
        None
    }

    /// Há tarefas do documento rodando (para os testes esperarem).
    fn ocupada(&self) -> bool {
        false
    }

    /// A cada quadro, com a aba fora de vista: as respostas que chegam, sem desenhar nada.
    fn em_segundo_plano(&mut self, _egui: &egui::Context, _ctx: &mut Contexto) {}

    // Os documentos editáveis: CSV e texto. Todos com o padrão de quem não
    // edita nada, então as telas que já existem não mudam.

    /// O documento tem alterações não salvas (o ponto de "alterado" da aba, a pergunta ao fechar). Barato: a casca
    /// chama a cada quadro, para cada aba.
    fn alterado(&self) -> bool {
        false
    }

    /// Quantas alterações não salvas há, para a pergunta "Há 3 alterações não salvas." e o rodapé ("· 3 alterações
    /// não salvas"). `None`: o documento não conta (o texto), e a pergunta diz só "Há alterações não salvas.".
    /// Só vale com [`Documento::alterado`] verdadeiro.
    fn alteracoes(&self) -> Option<usize> {
        None
    }

    /// Uma gravação está em andamento: a fila de perguntas ao fechar espera, e o documento ignora edição e desliga os
    /// verbos de edição enquanto isso.
    fn salvando(&self) -> bool {
        false
    }

    /// O erro da última gravação, uma vez (some depois de tomado): a fila de perguntas ao fechar o toma para parar
    /// tudo quando o Salvar falha, e o toast é dela.
    fn tomar_erro_de_gravacao(&mut self) -> Option<String> {
        None
    }

    /// O que a aba de arquivo diz de si na barra de status, no lugar do lado do banco: o ponto, o nome, a situação e
    /// os detalhes ("CSV · UTF-8 · CRLF · Separador ;"). `None`: a barra de status de sempre ([`Documento::status`]).
    fn status_do_arquivo(&self, _estado: &Estado) -> Option<StatusDeArquivo> {
        None
    }

    /// As colunas deste documento como origem do "Trazer colunas" (o CSV, e a planilha na etapa 6). `None`: o
    /// documento não tem tabela para oferecer. A casca junta as de todas as abas em [`Contexto::fontes`].
    fn fonte_de_colunas(&self) -> Option<Arc<dyn FonteDeColunas>> {
        None
    }
}

/// Como está o arquivo aberto (o ponto da barra de status: `--acct` alterado, `--ok` salvo, `--perigo` com erro).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Situacao {
    /// Igual ao que está no disco.
    Salvo,
    /// Com alterações que ainda não foram para o disco.
    Alterado,
    /// A última gravação falhou, ou o arquivo está com um erro que a pessoa precisa ver.
    ComErro,
    /// Aberto só para leitura (o leitor de arquivo grande): nada a salvar.
    SoLeitura,
}

impl Situacao {
    /// A frase ao lado do nome na barra de status ("Salvo", "Alterado, não salvo").
    pub fn rotulo(self) -> &'static str {
        match self {
            Situacao::Salvo => "Salvo",
            Situacao::Alterado => "Alterado, não salvo",
            Situacao::ComErro => "Com erro",
            Situacao::SoLeitura => "Somente leitura",
        }
    }
}

/// O que a aba de arquivo diz de si na barra de status ([`Documento::status_do_arquivo`]): à esquerda o ponto, o
/// nome e a [`Situacao`]; à direita os `detalhes` ("CSV", "UTF-8", "CRLF", "Separador ;").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusDeArquivo {
    /// O nome do arquivo ("vendas_2025.csv").
    pub nome: String,
    pub situacao: Situacao,
    /// Os detalhes à direita, um por item (a casca os junta com " · ").
    pub detalhes: Vec<String>,
}

impl StatusDeArquivo {
    /// O status de `nome` na `situacao`, sem detalhes.
    pub fn novo(nome: impl Into<String>, situacao: Situacao) -> StatusDeArquivo {
        StatusDeArquivo { nome: nome.into(), situacao, detalhes: Vec::new() }
    }

    /// Com mais um detalhe à direita.
    pub fn com_detalhe(mut self, detalhe: impl Into<String>) -> StatusDeArquivo {
        self.detalhes.push(detalhe.into());
        self
    }

    /// A situação em palavras, depois do nome ([`Situacao::rotulo`]).
    pub fn rotulo_da_situacao(&self) -> &'static str {
        self.situacao.rotulo()
    }

    /// Os detalhes numa linha só, como a barra os mostra ("CSV · UTF-8 · CRLF · Separador ;"); vazio sem detalhes.
    pub fn detalhes_em_linha(&self) -> String {
        self.detalhes.join(" · ")
    }
}

/// O que as Propriedades da aba de arquivo mostram do arquivo ([`secao_arquivo`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoArquivo {
    pub caminho: PathBuf,
    /// O tamanho em bytes, se se sabe ("—" quando não).
    pub tamanho: Option<u64>,
    /// A data de modificação, se se sabe ("—" quando não).
    pub modificado: Option<SystemTime>,
}

/// O toast do Copiar caminho da seção "Arquivo".
pub const MSG_CAMINHO_COPIADO: &str = "Caminho copiado.";

/// O que o menu Tabela pede à parte da tabela que cuida da estrutura (ver [`ParteTabela::pedido`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PedidoTabela {
    NovaColuna,
    Renomear,
    Excluir,
}

/// Uma parte de uma tabela ou visão (Dados, Estrutura, Índices e gatilhos, Definição SQL): o mesmo contrato do
/// [`Documento`], com a [`TabelaAberta`] que a tela de tabela carrega e empresta. A tela de tabela junta as ofertas
/// de toda tabela (Atualizar, Exportar) com as da parte e desenha a seção "Tabela" das Propriedades
/// ([`secao_tabela`]) antes das da parte.
pub trait ParteTabela {
    fn anatomia(&self) -> Anatomia {
        Anatomia::default()
    }

    fn faixa(&mut self, _ui: &mut Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {}

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto, tabela: &mut TabelaAberta);

    fn rodape(&mut self, _ui: &mut Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {}

    fn ofertas(&self, _tabela: &TabelaAberta, _estado: &Estado) -> Ofertas {
        Ofertas::default()
    }

    fn executar(&mut self, _verbo: Verbo, _egui: &egui::Context, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {}

    fn busca(&mut self) -> Option<BuscaDaBarra<'_>> {
        None
    }

    fn evento_busca(
        &mut self,
        _evento: EventoBusca,
        _egui: &egui::Context,
        _ctx: &mut Contexto,
        _tabela: &mut TabelaAberta,
    ) {
    }

    /// As ações da parte no fim da seção "Tabela" (o espaço `depois` do [`secao_tabela`]): o "Renomear a tabela" da
    /// Estrutura. A tela de tabela chama dentro da seção, só com ela aberta.
    fn acoes_da_tabela(&mut self, _ui: &mut Ui, _ctx: &mut Contexto, _tabela: &TabelaAberta) {}

    /// As seções da parte, depois da seção "Tabela", que é comum e desenhada pela tela de tabela. Sem o painel, nas
    /// partes de [`Rolagem::Pagina`], ficam no fim da página, como as do [`Documento::propriedades`].
    fn propriedades(&mut self, _ui: &mut Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {}

    /// Um pedido do menu Tabela (revisão de UI de 19/10/2026) à parte que cuida dele: a Estrutura abre a Nova coluna
    /// ou o Renomear e abre a zona "Excluir a tabela". A tela de tabela já pôs a vista da parte à vista.
    fn pedido(
        &mut self,
        _pedido: PedidoTabela,
        _egui: &egui::Context,
        _ctx: &mut Contexto,
        _tabela: &mut TabelaAberta,
    ) {
    }

    /// A coluna escolhida na parte (a linha selecionada da Estrutura), que acende na grade dos Dados.
    fn coluna_escolhida(&self) -> Option<&str> {
        None
    }

    /// Acende uma coluna na grade (os Dados: o `th.selcol` e o `td.selcol`); None apaga.
    fn destacar_coluna(&mut self, _coluna: Option<&str>) {}

    fn ocupada(&self) -> bool {
        false
    }

    fn em_segundo_plano(&mut self, _egui: &egui::Context, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {}
}

/// Um `Ui` em linha, centrado na altura, com `altura` pontos e a largura toda (a faixa e o rodapé fora da casca).
fn em_linha(ui: &mut Ui, altura: f32, desenhar: impl FnOnce(&mut Ui)) {
    let largura = ui.available_width();
    ui.allocate_ui_with_layout(vec2(largura, altura), Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_size(vec2(largura, altura));
        ui.spacing_mut().item_spacing.x = 8.0;
        desenhar(ui);
    });
}

/// Faixa, conteúdo e rodapé um embaixo do outro, sem a casca (a faixa com 42px e o rodapé com 34px, em linha). É o
/// `ui(…)` de sempre das partes (o contrato do ARQUITETURA §4.8 continua) e o que os testes das telas usam. Com a
/// rolagem própria, o conteúdo recebe a altura que sobra sem o rodapé (como na casca).
pub fn desenhar_parte_empilhada(
    parte: &mut dyn ParteTabela,
    ui: &mut Ui,
    ctx: &mut Contexto,
    tabela: &mut TabelaAberta,
) {
    let anatomia = parte.anatomia();
    if anatomia.faixa {
        em_linha(ui, ALTURA_FAIXA, |ui| parte.faixa(ui, ctx, tabela));
    }
    if anatomia.rolagem == Rolagem::Propria {
        let rodape = if anatomia.rodape { ALTURA_RODAPE } else { 0.0 };
        let altura = (ui.available_height() - rodape).max(0.0);
        let largura = ui.available_width();
        ui.allocate_ui_with_layout(vec2(largura, altura), Layout::top_down(Align::Min), |ui| {
            ui.set_min_size(vec2(largura, altura));
            ui.set_max_height(altura);
            parte.conteudo(ui, ctx, tabela);
        });
    } else {
        parte.conteudo(ui, ctx, tabela);
    }
    if anatomia.rodape {
        em_linha(ui, ALTURA_RODAPE, |ui| parte.rodape(ui, ctx, tabela));
    }
}

/// O espaço entre o conteúdo e as Propriedades no fim da página (o `gap:18px` do conteúdo das pranchetas) e entre o
/// fio e o `.tit`.
const ESPACO_ANTES_DO_FIM: f32 = 18.0;
const ESPACO_TITULO_DO_FIM: f32 = 10.0;

/// As Propriedades no fim da página (decisão 4; veja o topo do módulo), quando o painel não está à vista: o espaço,
/// o fio `--vidrob` de ponta a ponta, o `.tit` "Propriedades" e as `secoes` embaixo, sem espaço entre os itens (como
/// no painel). A casca e o `apoio_testes::palco_documento` chamam isto com o [`Documento::propriedades`].
pub fn propriedades_no_fim(ui: &mut Ui, secoes: impl FnOnce(&mut Ui)) {
    let cor = interface::tema::paleta(ui).estudio.vidrob;
    ui.add_space(ESPACO_ANTES_DO_FIM);
    let y = ui.cursor().min.y;
    ui.painter().hline(ui.max_rect().x_range(), y + 0.5, egui::Stroke::new(1.0, cor));
    ui.add_space(1.0 + ESPACO_TITULO_DO_FIM);
    interface::pecas::texto::h4(ui, "Propriedades");
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        secoes(ui);
    });
}

/// O que dá o id do campo da busca da barra de uma aba (o `id_salt` da `Entrada`): cada aba tem o seu, com o
/// cursor e a seleção dela.
fn sal_da_busca(aba: u64) -> (&'static str, u64) {
    ("databox::busca_da_barra", aba)
}

/// O id do campo da busca da barra da aba `aba` (para o Ctrl+F levar o foco a ele).
pub fn id_busca_da_barra(aba: u64) -> egui::Id {
    egui::Id::new(sal_da_busca(aba))
}

/// O Ctrl+F: o foco na busca da barra da aba, com o texto todo selecionado (digitar troca a busca).
pub fn focar_busca_da_barra(ctx: &egui::Context, aba: u64, texto: &str) {
    let id = id_busca_da_barra(aba);
    let n = texto.chars().count();
    let mut estado = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    estado
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
    estado.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// A busca da barra de ferramentas da aba `aba` sobre o texto que o documento empresta: a `Entrada::busca` de 250px
/// com a lupa, a dica do documento ("Buscar em clientes", que é também o rótulo) e o `.kbd` Ctrl+F. Sem busca
/// (`None`), a desligada ("Buscar"). Devolve o que aconteceu nela neste quadro: o texto mudou, Enter, ou Esc com
/// texto (o `input[type=search]`: o texto já foi limpo). Depois do Enter e do Esc, o foco continua no campo.
pub fn busca_da_barra(ui: &mut Ui, busca: Option<BuscaDaBarra<'_>>, aba: u64) -> Option<EventoBusca> {
    use interface::pecas::campos::{Entrada, enter_no_campo};
    use interface::pecas::ferramentas::{LARGURA_BUSCA, busca_desligada};
    let Some(BuscaDaBarra { texto, dica }) = busca else {
        busca_desligada(ui);
        return None;
    };
    let id = id_busca_da_barra(aba);
    // o Esc já tirou o foco do campo no começo do quadro: vale o foco do quadro anterior
    let tinha_foco = ui.memory(|m| m.has_focus(id) || m.had_focus_last_frame(id));
    let esc = tinha_foco && ui.input(|i| i.key_pressed(egui::Key::Escape));
    let saida = Entrada::new(&mut *texto)
        .busca(true)
        .atalho("Ctrl+F")
        .dica(&dica)
        .rotulo(&dica)
        .largura(LARGURA_BUSCA)
        .id_salt(sal_da_busca(aba))
        .mostrar(ui);
    let r = saida.response.response;
    if enter_no_campo(ui, &r) {
        ui.memory_mut(|m| m.request_focus(id));
        Some(EventoBusca::Enter)
    } else if esc && !texto.is_empty() {
        texto.clear();
        ui.memory_mut(|m| m.request_focus(id));
        Some(EventoBusca::Limpar)
    } else if r.changed() {
        Some(EventoBusca::Mudou)
    } else {
        None
    }
}

/// O tipo da tabela em uma palavra: "tabela", "visão", "virtual" ou "sem rowid".
pub fn tipo_da_tabela(info: &TabelaInfo) -> &'static str {
    match info.tipo {
        TipoObjeto::View => "visão",
        TipoObjeto::Virtual => "virtual",
        _ if info.sem_rowid => "sem rowid",
        _ => "tabela",
    }
}

/// O ícone sólido de uma tabela pelo tipo dela, o mesmo em todo lugar (o Explorador, as abas, a Visão geral, as
/// Tabelas da Consulta, o Exportar e o cabeçalho das janelas): a visão, a virtual (FTS) com a lupa, a sem rowid anil
/// com a chave (D6) ou a ficha comum. Mora aqui, e não no `interface::icones`, porque o `interface` não conhece o
/// `banco`.
pub fn icone_da_tabela(tipo: TipoObjeto, sem_rowid: bool) -> Icone {
    match tipo {
        TipoObjeto::View => Icone::Visao,
        TipoObjeto::Virtual => Icone::TabelaVirtual,
        _ if sem_rowid => Icone::TabelaSemRowid,
        _ => Icone::Tabela,
    }
}

/// As linhas da ficha "Tabela" (ou "Visão"): Nome, Tipo, Linhas, Colunas, Índices, Gatilhos, Sem rowid e Estrita (a
/// visão não tem índices, rowid nem é estrita).
fn linhas_da_tabela(d: &mut Detalhes<'_>, tabela: &TabelaAberta) {
    let info = tabela.info();
    let sim = |v: bool| if v { "sim" } else { "não" };
    d.linha_texto("Nome", Some(&tabela.nome), "—");
    d.linha_texto("Tipo", Some(tipo_da_tabela(info)), "—");
    d.linha_texto("Linhas", Some(&formatar_numero_opt(info.linhas)), "—");
    d.linha_texto("Colunas", Some(&formatar_numero(info.colunas.len())), "—");
    if !tabela.e_visao {
        d.linha_texto("Índices", Some(&formatar_numero(info.indices.len())), "—");
    }
    d.linha_texto("Gatilhos", Some(&formatar_numero(info.gatilhos.len())), "—");
    if !tabela.e_visao {
        d.linha_texto("Sem rowid", Some(sim(info.sem_rowid)), "—");
        d.linha_texto("Estrita", Some(sim(info.estrita)), "—");
    }
}

/// A seção "Tabela" das Propriedades ("Visão" numa visão), a primeira do painel, com o espaço `depois` para as
/// ações da parte ("Renomear a tabela" na Estrutura).
pub fn secao_tabela(ui: &mut Ui, tabela: &TabelaAberta, depois: impl FnOnce(&mut Ui)) {
    let titulo = if tabela.e_visao { "Visão" } else { "Tabela" };
    let _ = Secao::new("propriedades::tabela", titulo).primeira(true).mostrar(ui, |ui| {
        ficha(ui, |d| linhas_da_tabela(d, tabela));
        depois(ui);
    });
}

/// O espaço entre o caminho e o botão Copiar caminho, e a altura da linha da ficha (`.det`: 12px × 1,4).
const ESPACO_COPIAR: f32 = 4.0;
const ALTURA_LINHA_FICHA: f32 = 17.0;

/// O momento em ISO 8601 com `Z` ("2026-10-18T13:05:00Z"): o `formatar_data` do `interface` o converte para a hora
/// local. Sem biblioteca de datas aqui (o algoritmo de dias civis de Howard Hinnant); antes de 1970, `None`.
fn iso_utc(quando: SystemTime) -> Option<String> {
    let segundos = i64::try_from(quando.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()?;
    let (dias, resto) = (segundos.div_euclid(86_400), segundos.rem_euclid(86_400));
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let dia_da_era = z.rem_euclid(146_097);
    let ano_da_era = (dia_da_era - dia_da_era / 1_460 + dia_da_era / 36_524 - dia_da_era / 146_096) / 365;
    let dia_do_ano = dia_da_era - (365 * ano_da_era + ano_da_era / 4 - ano_da_era / 100);
    let mp = (5 * dia_do_ano + 2) / 153;
    let dia = dia_do_ano - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = ano_da_era + era * 400 + i64::from(mes <= 2);
    let (h, m, s) = (resto / 3_600, resto % 3_600 / 60, resto % 60);
    Some(format!("{ano:04}-{mes:02}-{dia:02}T{h:02}:{m:02}:{s:02}Z"))
}

/// O Caminho da ficha numa linha, com a reticência no meio quando não cabe (o `dd.caminho`: `.ini` cortado, `.fim`
/// inteiro; o caminho inteiro na dica e para o leitor de tela), e o `.ib.pq` Copiar caminho. Devolve se ele foi
/// clicado.
fn linha_do_caminho(ui: &mut Ui, caminho: &str) -> bool {
    let e = &interface::tema::paleta(ui).estudio;
    let largura = (ui.available_width() - LADO_PEQUENO - ESPACO_COPIAR).max(0.0);
    let g = galley_caminho(ui, caminho, fontes::fonte(12.0, Peso::Leve), e.t, largura);
    let encurtado = g.text() != caminho;
    // a linha de 17pt das outras da ficha; o botão de 26 centrado nela, logo depois do caminho, sem a esticar
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x, ALTURA_LINHA_FICHA), Sense::hover());
    ui.painter().galley(pos2(rect.left(), y_na_linha(rect.top(), ALTURA_LINHA_FICHA, &g)), Arc::clone(&g), e.t);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, caminho));
    if encurtado {
        resp.on_hover_text(caminho);
    }
    let centro = pos2(rect.right() + ESPACO_COPIAR + LADO_PEQUENO / 2.0, rect.center().y);
    let mut ui_botao =
        ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(centro, vec2(LADO_PEQUENO, LADO_PEQUENO))));
    ui_botao.add(Botao::icone(Icone::CopiarCaminho).pequeno().dica("Copiar caminho")).clicked()
}

/// A seção "Arquivo" das Propriedades dos editores (CSV, texto, JSON) e da ficha: Caminho (com o Copiar caminho e o
/// toast [`MSG_CAMINHO_COPIADO`]), Tamanho e Modificado em, e o espaço `depois` para o que o editor soma embaixo da
/// ficha. Não é a primeira do painel (o CSV tem o Formato antes, o JSON a Estrutura); para a primeira, use
/// [`secao_arquivo_com`]. Para somar linhas à própria ficha (Codificação, Fim de linha), também.
pub fn secao_arquivo(ui: &mut Ui, ctx: &mut Contexto, info: &InfoArquivo, depois: impl FnOnce(&mut Ui)) {
    secao_arquivo_com(ui, ctx, info, false, |_| {}, depois);
}

/// A [`secao_arquivo`] completa: `primeira` tira o fio de cima (a seção abre o painel, como na ficha), `linhas` soma
/// linhas à ficha depois de Modificado em ("Codificação", "Fim de linha": `d.linha_texto(…)`) e `depois` desenha
/// embaixo dela, ainda dentro da seção.
pub fn secao_arquivo_com(
    ui: &mut Ui,
    _ctx: &mut Contexto,
    info: &InfoArquivo,
    primeira: bool,
    linhas: impl FnOnce(&mut Detalhes<'_>),
    depois: impl FnOnce(&mut Ui),
) {
    let caminho = info.caminho.display().to_string();
    let tamanho = formatar_bytes_opt(info.tamanho);
    let modificado = formatar_data_opt(info.modificado.and_then(iso_utc).as_deref());
    let mut copiar = false;
    let _ = Secao::new("propriedades::arquivo", "Arquivo").primeira(primeira).mostrar(ui, |ui| {
        ficha(ui, |d| {
            d.linha("Caminho", |ui| copiar = linha_do_caminho(ui, &caminho));
            d.linha_texto("Tamanho", Some(&tamanho), "—");
            d.linha_texto("Modificado em", Some(&modificado), "—");
            linhas(d);
        });
        depois(ui);
    });
    if copiar {
        ui.ctx().copy_text(caminho);
        toast_concluido(ui.ctx(), MSG_CAMINHO_COPIADO);
    }
}

/// Os dois atalhos de SQL de uma tabela ou visão: "SELECT * FROM clientes LIMIT 100" e "SELECT count(*) FROM
/// clientes" (o nome citado quando precisa, inclusive palavra reservada: `"order"`; sem `;` no fim). É a única
/// fonte desses comandos: a Visão geral, a Estrutura, a Definição e os Dados abrem a Consulta com eles.
pub fn consultas_da_tabela(nome: &str) -> [String; 2] {
    let n = interface::codigo::citar(nome);
    [format!("SELECT * FROM {n} LIMIT 100"), format!("SELECT count(*) FROM {n}")]
}

/// A seção "Abrir na Consulta SQL" das Propriedades (`Dados.dc.html`): os dois atalhos de
/// [`consultas_da_tabela`] um embaixo do outro (`padding:2px 14px 14px; gap:6px`); o clique abre a Consulta SQL com
/// o comando (`Rota::consulta_com`).
pub fn secao_abrir_na_consulta(ui: &mut Ui, ctx: &mut Contexto, nome: &str) {
    /// O `padding:2px 14px 14px` e o `gap:6px` da seção.
    const RECUO_TOPO: f32 = 2.0;
    const RECUO_LADOS: f32 = 14.0;
    const RECUO_BAIXO: f32 = 14.0;
    const ESPACO_ATALHOS: f32 = 6.0;
    let consultas = consultas_da_tabela(nome);
    let mut escolhida = None;
    let _ = Secao::new(("propriedades::abrir_na_consulta", nome), "Abrir na Consulta SQL").mostrar(ui, |ui| {
        let largura = ui.available_width();
        let r = egui::Rect::from_min_size(
            ui.cursor().min + vec2(RECUO_LADOS, RECUO_TOPO),
            vec2((largura - 2.0 * RECUO_LADOS).max(0.0), 0.0),
        );
        let mut dentro = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
        dentro.spacing_mut().item_spacing.y = ESPACO_ATALHOS;
        for sql in &consultas {
            if interface::pecas::texto::atalho(&mut dentro, sql).clicked() {
                escolhida = Some(sql.clone());
            }
        }
        let altura = dentro.min_rect().height() + RECUO_TOPO + RECUO_BAIXO;
        ui.allocate_space(vec2(largura, altura));
    });
    if let Some(sql) = escolhida {
        ctx.navegar(crate::Rota::consulta_com(sql));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn atalhos_da_consulta_citam_palavra_reservada_e_nome_estranho() {
        assert_eq!(
            consultas_da_tabela("clientes"),
            ["SELECT * FROM clientes LIMIT 100".to_string(), "SELECT count(*) FROM clientes".to_string()]
        );
        // uma tabela chamada `order` (ou `group`): sem as aspas o SQL não roda
        assert_eq!(consultas_da_tabela("order")[0], "SELECT * FROM \"order\" LIMIT 100");
        assert_eq!(consultas_da_tabela("Group")[1], "SELECT count(*) FROM \"Group\"");
        assert_eq!(consultas_da_tabela("Tabela com espaço")[0], "SELECT * FROM \"Tabela com espaço\" LIMIT 100");
    }

    struct Vazio;

    impl Documento for Vazio {
        fn conteudo(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}
    }

    /// Uma parte que anota a ordem em que foi desenhada.
    #[derive(Default)]
    struct Anotada {
        ordem: Vec<&'static str>,
        anatomia: Anatomia,
    }

    impl ParteTabela for Anotada {
        fn anatomia(&self) -> Anatomia {
            self.anatomia
        }
        fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {
            assert!((ui.max_rect().height() - ALTURA_FAIXA).abs() < 0.5, "a faixa com 42px");
            self.ordem.push("faixa");
        }
        fn conteudo(&mut self, _ui: &mut Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {
            self.ordem.push("conteudo");
        }
        fn rodape(&mut self, ui: &mut Ui, _ctx: &mut Contexto, _tabela: &mut TabelaAberta) {
            assert!((ui.max_rect().height() - ALTURA_RODAPE).abs() < 0.5, "o rodapé com 34px");
            self.ordem.push("rodape");
        }
    }

    #[test]
    fn anatomia_padrao_e_pagina_sem_faixa_nem_rodape() {
        let a = Anatomia::default();
        assert_eq!(a, Anatomia { faixa: false, rodape: false, rolagem: Rolagem::Pagina });
        assert_eq!(Vazio.anatomia(), a);
        let toda = a.com_faixa().com_rodape().rolagem_propria();
        assert_eq!(toda, Anatomia { faixa: true, rodape: true, rolagem: Rolagem::Propria });
    }

    #[test]
    fn documento_padrao_nao_oferece_nada() {
        let mut d = Vazio;
        let amb = crate::apoio_testes::Ambiente::vazio("documento_padrao");
        assert_eq!(d.ofertas(&amb.estado), Ofertas::default());
        assert!(d.busca().is_none());
        assert!(!d.tem_propriedades() && !d.ocupada());
        assert_eq!(d.status(&amb.estado), None);
    }

    #[test]
    fn trait_documento_novos_metodos_tem_padrao() {
        let mut d = Vazio;
        let amb = crate::apoio_testes::Ambiente::vazio("documento_padrao_arquivo");
        // quem não edita nada: sem alteração, sem contagem, sem gravação, sem erro, sem status próprio
        assert!(!d.alterado());
        assert_eq!(d.alteracoes(), None);
        assert!(!d.salvando());
        assert_eq!(d.tomar_erro_de_gravacao(), None);
        assert_eq!(d.status_do_arquivo(&amb.estado), None);
        assert!(d.fonte_de_colunas().is_none());
    }

    /// Um documento editável de mentira: diz que está alterado, que grava e que deu erro, uma vez só.
    struct Editavel {
        alteracoes: usize,
        erro: Option<String>,
    }

    impl Documento for Editavel {
        fn conteudo(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}
        fn alterado(&self) -> bool {
            self.alteracoes > 0
        }
        fn alteracoes(&self) -> Option<usize> {
            Some(self.alteracoes)
        }
        fn salvando(&self) -> bool {
            true
        }
        fn tomar_erro_de_gravacao(&mut self) -> Option<String> {
            self.erro.take()
        }
        fn status_do_arquivo(&self, _estado: &Estado) -> Option<StatusDeArquivo> {
            let situacao = if self.alteracoes > 0 { Situacao::Alterado } else { Situacao::Salvo };
            Some(StatusDeArquivo::novo("vendas_2025.csv", situacao).com_detalhe("CSV").com_detalhe("UTF-8"))
        }
    }

    #[test]
    fn fonte_de_colunas_do_documento_padrao_e_none() {
        use crate::fonte::{FonteFalsa, TipoDeFonte};
        /// Um documento com tabela: oferece a fonte dele, a mesma `Arc` a cada chamada.
        struct ComTabela(Arc<dyn FonteDeColunas>);
        impl Documento for ComTabela {
            fn conteudo(&mut self, _ui: &mut Ui, _ctx: &mut Contexto) {}
            fn fonte_de_colunas(&self) -> Option<Arc<dyn FonteDeColunas>> {
                Some(Arc::clone(&self.0))
            }
        }
        assert!(Vazio.fonte_de_colunas().is_none());
        let fonte: Arc<dyn FonteDeColunas> =
            Arc::new(FonteFalsa { rotulo: "vendas.csv".into(), tipo: TipoDeFonte::Csv, linhas: 2 });
        let doc = ComTabela(fonte);
        // a casca monta a lista das abas com uma `Arc` por aba: sem copiar a tabela
        let lista: Vec<Arc<dyn FonteDeColunas>> = [&doc, &doc].iter().filter_map(|d| d.fonte_de_colunas()).collect();
        assert_eq!(lista.len(), 2);
        assert!(Arc::ptr_eq(&lista[0], &lista[1]));
        assert_eq!(lista[0].rotulo(), "vendas.csv");
    }

    #[test]
    fn ofertas_de_um_documento_sem_trazer_colunas_desligam_o_botao() {
        let amb = crate::apoio_testes::Ambiente::vazio("ofertas_sem_trazer");
        // o documento padrão e os que oferecem outros verbos deixam o lugar do Trazer colunas desligado
        let o = Vazio.ofertas(&amb.estado);
        assert!(!o.ligado(Verbo::TrazerColunas));
        assert_eq!(o.rotulo(Verbo::TrazerColunas), "Trazer colunas");
        assert_eq!(o.dica(Verbo::TrazerColunas), "Trazer colunas");
        let o = Completo::default().ofertas(&amb.estado);
        assert!(o.ligado(Verbo::Novo) && !o.ligado(Verbo::TrazerColunas));
        assert!(!Ofertas::default().get(Verbo::TrazerColunas).ligado);
    }

    #[test]
    fn documento_editavel_responde_pelos_metodos_novos() {
        let amb = crate::apoio_testes::Ambiente::vazio("documento_editavel");
        let mut d = Editavel { alteracoes: 3, erro: Some("Sem espaço no disco.".into()) };
        assert!(d.alterado() && d.salvando());
        assert_eq!(d.alteracoes(), Some(3));
        // o erro vem uma vez só
        assert_eq!(d.tomar_erro_de_gravacao().as_deref(), Some("Sem espaço no disco."));
        assert_eq!(d.tomar_erro_de_gravacao(), None);
        let status = d.status_do_arquivo(&amb.estado).expect("o status do arquivo");
        assert_eq!(status.situacao, Situacao::Alterado);
        d.alteracoes = 0;
        assert!(!d.alterado());
        assert_eq!(d.status_do_arquivo(&amb.estado).map(|s| s.situacao), Some(Situacao::Salvo));
    }

    #[test]
    fn status_de_arquivo_por_situacao() {
        let rotulos: Vec<&str> =
            [Situacao::Salvo, Situacao::Alterado, Situacao::ComErro, Situacao::SoLeitura].map(Situacao::rotulo).into();
        assert_eq!(rotulos, ["Salvo", "Alterado, não salvo", "Com erro", "Somente leitura"]);
        let s = StatusDeArquivo::novo("vendas_2025.csv", Situacao::Alterado)
            .com_detalhe("CSV")
            .com_detalhe("UTF-8")
            .com_detalhe("CRLF")
            .com_detalhe("Separador ;");
        assert_eq!(s.nome, "vendas_2025.csv");
        assert_eq!(s.rotulo_da_situacao(), "Alterado, não salvo");
        assert_eq!(s.detalhes, ["CSV", "UTF-8", "CRLF", "Separador ;"]);
        assert_eq!(s.detalhes_em_linha(), "CSV · UTF-8 · CRLF · Separador ;");
        // sem detalhes, a linha vem vazia
        let so_nome = StatusDeArquivo::novo("notas.txt", Situacao::Salvo);
        assert_eq!(so_nome.detalhes_em_linha(), "");
        assert_eq!(so_nome, StatusDeArquivo { nome: "notas.txt".into(), situacao: Situacao::Salvo, detalhes: vec![] });
    }

    #[test]
    fn iso_utc_confere_datas_conhecidas() {
        let em = |s: u64| iso_utc(UNIX_EPOCH + std::time::Duration::from_secs(s));
        assert_eq!(em(0).as_deref(), Some("1970-01-01T00:00:00Z"));
        assert_eq!(em(1_000_000_000).as_deref(), Some("2001-09-09T01:46:40Z"));
        // o dia bissexto de 2000 e o de 2024, e a virada do ano
        assert_eq!(em(951_782_400).as_deref(), Some("2000-02-29T00:00:00Z"));
        assert_eq!(em(1_709_164_800).as_deref(), Some("2024-02-29T00:00:00Z"));
        assert_eq!(em(1_767_225_599).as_deref(), Some("2025-12-31T23:59:59Z"));
        assert_eq!(em(1_760_000_000).as_deref(), Some("2025-10-09T08:53:20Z"));
        // antes de 1970 não há data
        assert_eq!(iso_utc(UNIX_EPOCH - std::time::Duration::from_secs(1)), None);
    }

    /// Um documento cujas Propriedades são só a seção "Arquivo" (com uma linha a mais na ficha e um texto embaixo).
    struct SoArquivo {
        info: InfoArquivo,
        primeira: bool,
    }

    impl Documento for SoArquivo {
        fn conteudo(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
            ui.label("O conteúdo");
        }
        fn tem_propriedades(&self) -> bool {
            true
        }
        fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
            let linhas = |d: &mut Detalhes<'_>| d.linha_texto("Codificação", Some("UTF-8"), "—");
            secao_arquivo_com(ui, ctx, &self.info, self.primeira, linhas, |ui| {
                ui.label("Depois da ficha");
            });
        }
    }

    #[test]
    fn secao_arquivo_mostra_caminho_tamanho_data_e_copia() {
        use egui_kittest::kittest::Queryable;
        let amb = crate::apoio_testes::Ambiente::vazio("secao_arquivo");
        let caminho = PathBuf::from(r"C:\dados\vendas_2025.csv");
        let info = InfoArquivo {
            caminho: caminho.clone(),
            tamanho: Some(1_536),
            // 2001-09-09 01:46 UTC: em qualquer fuso é 08 ou 09 de setembro de 2001
            modificado: Some(UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000)),
        };
        let tema = interface::tema::Tema::Claro;
        let doc = SoArquivo { info, primeira: false };
        let mut h = crate::apoio_testes::palco_documento(&amb, doc, tema, vec2(1100.0, 600.0));
        h.run_steps(2);
        // o título da seção, os valores das três linhas padrão (os rótulos são só pintados), a linha que o editor somou
        // e o que veio depois da ficha
        assert!(h.query_by_label("Seção Arquivo").is_some());
        assert!(h.query_by_label("1,5 KB").is_some(), "o tamanho formatado");
        assert!(h.query_by_label_contains("/09/2001").is_some(), "a data de modificação");
        assert!(h.query_by_label("UTF-8").is_some());
        assert!(h.query_by_label("Depois da ficha").is_some());
        let mostrado = caminho.display().to_string();
        assert!(h.query_by_label(&mostrado).is_some(), "o caminho inteiro (cabe no painel)");
        // o Copiar caminho copia o caminho inteiro e dá o toast
        h.get_by_label("Copiar caminho").click();
        let mut copiado = None;
        for _ in 0..3 {
            h.step();
            copiado = copiado.or_else(|| {
                h.output().platform_output.commands.iter().find_map(|c| match c {
                    egui::OutputCommand::CopyText(t) => Some(t.clone()),
                    _ => None,
                })
            });
        }
        assert_eq!(copiado.as_deref(), Some(mostrado.as_str()));
        assert_eq!(interface::pecas::toast::toast_atual(&h.ctx).as_deref(), Some(MSG_CAMINHO_COPIADO));
    }

    #[test]
    fn secao_arquivo_sem_tamanho_nem_data_mostra_traco() {
        use egui_kittest::kittest::Queryable;
        let amb = crate::apoio_testes::Ambiente::vazio("secao_arquivo_sem_dados");
        let info = InfoArquivo { caminho: PathBuf::from(r"C:\dados\novo.json"), tamanho: None, modificado: None };
        let doc = SoArquivo { info, primeira: true };
        let mut h = crate::apoio_testes::palco_documento(&amb, doc, interface::tema::Tema::Escuro, vec2(1100.0, 600.0));
        h.run_steps(2);
        // Tamanho e Modificado em: o vazio "—" nas duas linhas
        assert_eq!(h.query_all_by_label("—").count(), 2);
    }

    #[test]
    fn parte_empilhada_desenha_faixa_conteudo_e_rodape() {
        let amb = crate::apoio_testes::Ambiente::exemplo("parte_empilhada");
        let mut tabela = amb.tabela("clientes");
        let ctx = egui::Context::default();
        let mut desenhar = |parte: &mut Anotada| {
            let mut acoes = Vec::new();
            let saida = ctx.run_ui(egui::RawInput::default(), |ui| {
                let mut c = Contexto::novo(&amb.servico, &amb.estado, None, &mut acoes);
                desenhar_parte_empilhada(parte, ui, &mut c, &mut tabela);
            });
            saida.drop_without_applying_deltas();
        };
        let mut so_conteudo = Anotada::default();
        desenhar(&mut so_conteudo);
        assert_eq!(so_conteudo.ordem, ["conteudo"]);
        let mut toda = Anotada { anatomia: Anatomia::default().com_faixa().com_rodape(), ..Anotada::default() };
        desenhar(&mut toda);
        assert_eq!(toda.ordem, ["faixa", "conteudo", "rodape"]);
    }

    #[test]
    fn tipo_da_tabela_em_uma_palavra() {
        let amb = crate::apoio_testes::Ambiente::exemplo("tipo_da_tabela");
        let tipo = |nome: &str| tipo_da_tabela(amb.tabela(nome).info());
        assert_eq!(tipo("clientes"), "tabela");
        assert_eq!(tipo("produtos"), "sem rowid");
        assert_eq!(tipo("notas"), "virtual");
        assert_eq!(tipo("v_pedidos_por_cliente"), "visão");
    }

    /// Um documento com a anatomia toda, um verbo, a busca e as Propriedades, que anota o que recebeu.
    #[derive(Default)]
    struct Completo {
        busca: String,
        eventos: Vec<EventoBusca>,
        executados: Vec<Verbo>,
        partes: Vec<egui::Rect>,
    }

    impl Documento for Completo {
        fn anatomia(&self) -> Anatomia {
            Anatomia::default().com_faixa().com_rodape()
        }
        fn faixa(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
            self.partes.push(ui.max_rect());
            ui.label("Filtros");
        }
        fn conteudo(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
            ui.label("O conteúdo");
        }
        fn rodape(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
            self.partes.push(ui.max_rect());
            ui.label("Linhas 1–41 de 41");
        }
        fn ofertas(&self, _estado: &Estado) -> Ofertas {
            Ofertas::default().com(Verbo::Novo, crate::Oferta::ligada().com_rotulo("Nova linha"))
        }
        fn executar(&mut self, verbo: Verbo, _egui: &egui::Context, _ctx: &mut Contexto) {
            self.executados.push(verbo);
        }
        fn busca(&mut self) -> Option<BuscaDaBarra<'_>> {
            Some(BuscaDaBarra { texto: &mut self.busca, dica: "Buscar em clientes".into() })
        }
        fn evento_busca(&mut self, evento: EventoBusca, _egui: &egui::Context, _ctx: &mut Contexto) {
            self.eventos.push(evento);
        }
        fn tem_propriedades(&self) -> bool {
            true
        }
        fn propriedades(&mut self, ui: &mut Ui, _ctx: &mut Contexto) {
            let _ = Secao::new("teste", "Coluna").primeira(true).mostrar(ui, |_| {});
        }
    }

    #[test]
    fn palco_desenha_a_barra_o_documento_e_as_propriedades() {
        use egui::Key;
        use egui_kittest::kittest::Queryable;
        let amb = crate::apoio_testes::Ambiente::vazio("palco_documento");
        let tema = interface::tema::Tema::Claro;
        let mut h = crate::apoio_testes::palco_documento(&amb, Completo::default(), tema, vec2(1100.0, 600.0));
        // o verbo pelo rótulo da oferta, executado antes do conteúdo
        h.get_by_label("Nova linha").click();
        h.run_steps(2);
        assert_eq!(h.state().tela.doc.executados, [Verbo::Novo]);
        // a busca pela dica: digitar, Enter e Esc (que limpa)
        let campo = h.get_by_label("Buscar em clientes");
        campo.focus();
        campo.type_text("Recife");
        h.run_steps(2);
        assert_eq!(h.state().tela.doc.busca, "Recife");
        h.key_press(Key::Enter);
        h.run_steps(2);
        h.key_press(Key::Escape);
        h.run_steps(2);
        let eventos = &h.state().tela.doc.eventos;
        assert!(eventos.contains(&EventoBusca::Mudou), "{eventos:?}");
        assert!(eventos.contains(&EventoBusca::Enter) && eventos.ends_with(&[EventoBusca::Limpar]), "{eventos:?}");
        assert!(h.state().tela.doc.busca.is_empty());
        // a faixa de 42px (sem o fio), o rodapé de 34px (sem o fio) e as Propriedades à direita
        let partes = &h.state().tela.doc.partes;
        let (faixa, rodape) = (partes[partes.len() - 2], partes[partes.len() - 1]);
        assert_eq!((faixa.height(), rodape.height()), (ALTURA_FAIXA - 1.0, ALTURA_RODAPE - 1.0));
        assert!(h.get_by_label("Seção Coluna").rect().left() > faixa.right());
        // sem o painel, as Propriedades vão para o fim da página
        h.state_mut().tela.sem_painel();
        h.run_steps(2);
        assert!(h.get_by_label("Seção Coluna").rect().top() > h.get_by_label("O conteúdo").rect().bottom());
    }
}
