//! O menu da célula (0.2.3, `design/planos-0.2.3.md`, A.1): o [`MenuContexto`] (vidro fosco de 280, itens de 28 do
//! [`PainelMenu`]) que abre no clique direito de uma célula e no Shift+F10 com o foco na grade, com os itens de copiar,
//! colar, limpar, filtrar, marcar e excluir. A grade só abre o menu e devolve a [`AcaoCelula`]; cada tela diz o que
//! liga ([`OfertasCelula`], com o motivo do desligado na dica) e cumpre a ação.
//!
//! A tecla Menu do teclado não chega ao egui 0.36 (o egui-winit não traduz o `NamedKey::ContextMenu`): pelo teclado,
//! o menu abre com o Shift+F10.
//!
//! ```ignore
//! let r = nucleo.mostrar(ui, altura, &mut desenho);
//! let ofertas = OfertasCelula::de_leitura(nucleo.intervalo()?).ligar(AcaoCelula::FiltrarPorValor);
//! match menu_da_celula(ui, &mut nucleo, &ofertas) {
//!     Some(AcaoCelula::Copiar) => …,
//!     Some(AcaoCelula::CopiarComo(formato)) => ui.ctx().copy_text(texto_do_intervalo_como(formato, …)),
//!     …
//! }
//! ```

use std::borrow::Cow;

use comum::Celula;
use egui::{Context, Pos2, Ui};
use tabular::colunas::{FormatoCopia, OpcoesCopia, ValorCopia, escrever_como, escrever_tsv_com_cabecalho};

use super::{Intervalo, NucleoGrade, texto_celula};
use crate::formato::real_js;
use crate::icones::Icone;
use crate::pecas::casca::PainelMenu;
use crate::pecas::menu_contexto::{Abertura, MenuContexto};

/// Pelo teclado, o menu abre embaixo da célula ativa, este tanto à direita da borda esquerda dela.
pub(super) const RECUO_TECLADO: f32 = 8.0;
/// A largura do menu: os 250 do menu de contexto não cabem o "Copiar com cabeçalho" com o Ctrl+Shift+C.
pub const LARGURA_MENU_DA_CELULA: f32 = 280.0;
/// A largura do submenu "Copiar como".
const LARGURA_COPIAR_COMO: f32 = 200.0;

/// O menu pedido num quadro ([`NucleoGrade::mostrar`] → [`menu_da_celula`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PedidoMenu {
    /// O canto de cima à esquerda do menu: o ponteiro, ou embaixo da célula ativa pelo teclado.
    pub(super) pos: Pos2,
    /// Pelo Shift+F10: o foco vai ao primeiro item.
    pub(super) teclado: bool,
}

/// O que a pessoa escolheu no menu da célula. Age no intervalo ([`NucleoGrade::intervalo`]; uma célula só quando não
/// há intervalo), ou na célula ativa nos itens de filtro e na ficha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcaoCelula {
    /// Ctrl+C: o TSV do intervalo, como o Excel copia.
    Copiar,
    /// Ctrl+Shift+C: o TSV com os nomes das colunas na primeira linha.
    CopiarComCabecalho,
    /// O submenu "Copiar como" ([`texto_do_intervalo_como`]).
    CopiarComo(FormatoCopia),
    /// Ctrl+V: o TSV da área de transferência a partir da célula ativa.
    Colar,
    /// Delete: o vazio (ou o NULL, onde a coluna aceita).
    Limpar,
    DefinirNulo,
    /// Uma condição "igual a" pela célula ativa na barra de filtros.
    FiltrarPorValor,
    /// Uma condição "diferente de" pela célula ativa.
    ExcluirValorDoFiltro,
    /// Marca as linhas do intervalo (somando às já marcadas).
    MarcarLinhas,
    /// A ficha da linha da célula ativa (Enter, nos Dados).
    AbrirFicha,
    /// Exclui as linhas do intervalo, com a pergunta do Excluir da barra.
    ExcluirLinhas,
}

impl AcaoCelula {
    /// O texto do item no menu (o "Copiar como" leva o nome do formato, [`FormatoCopia::rotulo`]).
    pub fn rotulo(self) -> &'static str {
        match self {
            AcaoCelula::Copiar => "Copiar",
            AcaoCelula::CopiarComCabecalho => "Copiar com cabeçalho",
            AcaoCelula::CopiarComo(formato) => formato.rotulo(),
            AcaoCelula::Colar => "Colar",
            AcaoCelula::Limpar => "Limpar",
            AcaoCelula::DefinirNulo => "Definir como NULL",
            AcaoCelula::FiltrarPorValor => "Filtrar por este valor",
            AcaoCelula::ExcluirValorDoFiltro => "Excluir este valor do filtro",
            AcaoCelula::MarcarLinhas => "Marcar as linhas",
            AcaoCelula::AbrirFicha => "Abrir a ficha da linha",
            AcaoCelula::ExcluirLinhas => "Excluir as linhas",
        }
    }

    /// O atalho escrito à direita do item (e no `keyboard_shortcut` do AccessKit).
    pub fn atalho(self) -> Option<&'static str> {
        match self {
            AcaoCelula::Copiar => Some("Ctrl+C"),
            AcaoCelula::CopiarComCabecalho => Some("Ctrl+Shift+C"),
            AcaoCelula::Colar => Some("Ctrl+V"),
            AcaoCelula::Limpar => Some("Delete"),
            AcaoCelula::AbrirFicha => Some("Enter"),
            _ => None,
        }
    }

    /// A silhueta da barra, nos itens que já têm ícone nela.
    fn silhueta(self) -> Option<Icone> {
        match self {
            AcaoCelula::FiltrarPorValor => Some(Icone::Filtrar),
            AcaoCelula::ExcluirLinhas => Some(Icone::Excluir),
            _ => None,
        }
    }
}

/// O que a tela oferece de um item do menu da célula.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OfertaCelula {
    /// Não vale nesta grade: o item não aparece.
    #[default]
    Oculta,
    Ligada,
    /// Vale, mas não agora: o item aparece desligado, com o motivo na dica ("A coluna id é gerada.").
    Desligada(Cow<'static, str>),
}

/// O que a tela oferece no menu da célula, item por item (como as `Ofertas` dos verbos). O padrão esconde tudo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OfertasCelula {
    itens: Vec<(AcaoCelula, OfertaCelula)>,
}

/// A frase do "Lista para IN (…)" desligado.
pub const MOTIVO_LISTA_IN: &str = "Selecione uma coluna só.";
/// A dica dos itens de escrita (Colar, Limpar, Definir como NULL, Excluir as linhas) enquanto eles não chegam.
pub const MOTIVO_PROXIMA_ETAPA: &str = "Chega na próxima etapa da 0.2.3.";

impl OfertasCelula {
    /// Os itens de copiar, que valem nas quatro grades: Copiar, Copiar com cabeçalho e os seis formatos do "Copiar
    /// como" (a lista para `IN (…)` desligada quando o `intervalo` tem mais de uma coluna).
    pub fn de_leitura(intervalo: Intervalo) -> Self {
        let mut ofertas = OfertasCelula::default().ligar(AcaoCelula::Copiar).ligar(AcaoCelula::CopiarComCabecalho);
        for formato in FormatoCopia::TODOS {
            ofertas = ofertas.ligar(AcaoCelula::CopiarComo(formato));
        }
        if intervalo.colunas().len() > 1 {
            ofertas = ofertas.desligar(AcaoCelula::CopiarComo(FormatoCopia::ListaIn), MOTIVO_LISTA_IN);
        }
        ofertas
    }

    /// Com os itens de escrita que valem na grade (Colar, Limpar, Definir como NULL, Excluir as linhas) desligados,
    /// com a dica [`MOTIVO_PROXIMA_ETAPA`]: eles chegam na próxima etapa da 0.2.3.
    pub fn com_escrita_por_vir(mut self, acoes: &[AcaoCelula]) -> Self {
        for &acao in acoes {
            self = self.desligar(acao, MOTIVO_PROXIMA_ETAPA);
        }
        self
    }

    /// Com esta oferta para o item.
    pub fn com(mut self, acao: AcaoCelula, oferta: OfertaCelula) -> Self {
        match self.itens.iter_mut().find(|(a, _)| *a == acao) {
            Some((_, o)) => *o = oferta,
            None => self.itens.push((acao, oferta)),
        }
        self
    }

    /// Com o item ligado.
    pub fn ligar(self, acao: AcaoCelula) -> Self {
        self.com(acao, OfertaCelula::Ligada)
    }

    /// Com o item desligado, com o `motivo` na dica.
    pub fn desligar(self, acao: AcaoCelula, motivo: impl Into<Cow<'static, str>>) -> Self {
        self.com(acao, OfertaCelula::Desligada(motivo.into()))
    }

    /// Ligado se `ligado`; senão desligado com o `motivo`.
    pub fn ligar_se(self, acao: AcaoCelula, ligado: bool, motivo: impl Into<Cow<'static, str>>) -> Self {
        if ligado { self.ligar(acao) } else { self.desligar(acao, motivo) }
    }

    pub fn get(&self, acao: AcaoCelula) -> &OfertaCelula {
        static OCULTA: OfertaCelula = OfertaCelula::Oculta;
        self.itens.iter().find(|(a, _)| *a == acao).map_or(&OCULTA, |(_, o)| o)
    }

    pub fn ligado(&self, acao: AcaoCelula) -> bool {
        *self.get(acao) == OfertaCelula::Ligada
    }

    fn visivel(&self, acao: AcaoCelula) -> bool {
        *self.get(acao) != OfertaCelula::Oculta
    }
}

/// Uma entrada do menu: um item ou o submenu "Copiar como".
#[derive(Clone, Copy)]
enum Entrada {
    Item(AcaoCelula),
    CopiarComo,
}

/// Os grupos do menu, na ordem da tabela da A.1; o separador vai entre dois grupos com algum item à vista.
const GRUPOS: [&[Entrada]; 4] = [
    &[Entrada::Item(AcaoCelula::Copiar), Entrada::Item(AcaoCelula::CopiarComCabecalho), Entrada::CopiarComo],
    &[Entrada::Item(AcaoCelula::Colar), Entrada::Item(AcaoCelula::Limpar), Entrada::Item(AcaoCelula::DefinirNulo)],
    &[Entrada::Item(AcaoCelula::FiltrarPorValor), Entrada::Item(AcaoCelula::ExcluirValorDoFiltro)],
    &[
        Entrada::Item(AcaoCelula::MarcarLinhas),
        Entrada::Item(AcaoCelula::AbrirFicha),
        Entrada::Item(AcaoCelula::ExcluirLinhas),
    ],
];

impl Entrada {
    fn visivel(self, ofertas: &OfertasCelula) -> bool {
        match self {
            Entrada::Item(acao) => ofertas.visivel(acao),
            Entrada::CopiarComo => FormatoCopia::TODOS.into_iter().any(|f| ofertas.visivel(AcaoCelula::CopiarComo(f))),
        }
    }
}

/// O sal do [`MenuContexto`] do menu da célula de um núcleo.
fn id_salt(nucleo: &NucleoGrade) -> (&'static str, egui::Id) {
    ("grade::menu_da_celula", nucleo.id())
}

/// O menu da célula do `nucleo` está aberto?
pub fn menu_da_celula_aberto(ctx: &Context, nucleo: &NucleoGrade) -> bool {
    MenuContexto::aberto(ctx, id_salt(nucleo))
}

/// O menu da célula: chame todo quadro, logo depois do [`NucleoGrade::mostrar`]. Abre no quadro em que o núcleo pediu
/// ([`super::RespostaNucleo::menu_celula`]: o clique direito numa célula, o Shift+F10 com o foco na grade), com os itens
/// que as `ofertas` mostram; o Esc fecha e devolve o foco à grade, e escolher um item também. Devolve a ação escolhida.
pub fn menu_da_celula(ui: &Ui, nucleo: &mut NucleoGrade, ofertas: &OfertasCelula) -> Option<AcaoCelula> {
    let abertura = nucleo.pedido_menu.take().map(|p| Abertura { pos: p.pos, pedir_foco: p.teclado });
    let rotulo = if nucleo.intervalo().is_some_and(|iv| iv.varias()) { "Menu da seleção" } else { "Menu da célula" };
    let acao = MenuContexto::new(id_salt(nucleo))
        .rotulo(rotulo)
        .largura(LARGURA_MENU_DA_CELULA)
        .mostrar_aberto(ui.ctx(), ui.layer_id(), nucleo.id_corpo(), abertura, |m| itens(m, ofertas))
        .flatten();
    if acao.is_some() {
        // escolhido pelo ponteiro ou pelo teclado, o foco volta à grade (as setas e o Ctrl+C seguem nela)
        nucleo.focar(ui);
    }
    acao
}

fn itens(m: &mut PainelMenu<'_>, ofertas: &OfertasCelula) -> Option<AcaoCelula> {
    let mut escolha = None;
    let mut algum_antes = false;
    for grupo in GRUPOS {
        if !grupo.iter().any(|e| e.visivel(ofertas)) {
            continue;
        }
        if algum_antes {
            m.separador();
        }
        algum_antes = true;
        for &entrada in grupo.iter().filter(|e| e.visivel(ofertas)) {
            let escolhida = match entrada {
                Entrada::Item(acao) => item(m, acao, ofertas),
                Entrada::CopiarComo => m
                    .submenu("Copiar como", LARGURA_COPIAR_COMO, |m| {
                        let mut escolha = None;
                        for formato in FormatoCopia::TODOS {
                            escolha = escolha.or(item(m, AcaoCelula::CopiarComo(formato), ofertas));
                        }
                        escolha
                    })
                    .flatten(),
            };
            escolha = escolha.or(escolhida);
        }
    }
    escolha
}

/// Um item conforme a oferta (o oculto não aparece). Devolve a ação, se foi escolhida.
fn item(m: &mut PainelMenu<'_>, acao: AcaoCelula, ofertas: &OfertasCelula) -> Option<AcaoCelula> {
    let (texto, atalho) = (acao.rotulo(), acao.atalho());
    let oferta = ofertas.get(acao);
    if *oferta == OfertaCelula::Oculta {
        return None;
    }
    if let Some(icone) = acao.silhueta() {
        m.silhueta(icone);
    }
    let resposta = match oferta {
        OfertaCelula::Oculta => return None,
        OfertaCelula::Ligada if acao == AcaoCelula::ExcluirLinhas => m.item_perigo(texto, atalho, false),
        OfertaCelula::Ligada => m.item(texto, atalho, false),
        OfertaCelula::Desligada(motivo) => {
            let r = m.item_desligado(texto, atalho);
            r.ctx.accesskit_node_builder(r.id, |no| no.set_description(motivo.to_string()));
            r.on_hover_text(motivo.as_ref())
        }
    };
    if let Some(atalho) = atalho {
        resposta.ctx.accesskit_node_builder(resposta.id, |no| no.set_keyboard_shortcut(atalho));
    }
    resposta.clicked().then_some(acao)
}

// ------------------------------------------------------------------ cópia

/// A célula do banco para os formatos de cópia: o NULL como NULL, o número como número (o real como o JavaScript
/// escreve, [`real_js`]; infinito e NaN, que o JSON não tem, como texto) e o resto como o [`texto_celula`] (o BLOB
/// vazio, o texto cortado só com o começo).
pub fn valor_de_copia(c: &Celula) -> ValorCopia<'static> {
    match c {
        Celula::Nulo => ValorCopia::Nulo,
        Celula::Inteiro(i) => ValorCopia::numero(i.to_string()),
        Celula::Real(r) if r.is_finite() => ValorCopia::numero(real_js(*r)),
        _ => ValorCopia::texto(texto_celula(c)),
    }
}

/// O TSV do intervalo com os nomes das colunas na primeira linha (o "Copiar com cabeçalho", Ctrl+Shift+C): `nome` dá
/// o nome de cada coluna da grade e `texto` o texto de cada célula, como no [`super::tsv_do_intervalo`].
pub fn tsv_do_intervalo_com_cabecalho(
    intervalo: Intervalo,
    nome: impl Fn(usize) -> String,
    mut texto: impl FnMut(usize, usize) -> Option<String>,
) -> String {
    let nomes: Vec<String> = intervalo.colunas().map(nome).collect();
    let linhas: Vec<Vec<String>> =
        intervalo.linhas().map(|l| intervalo.colunas().map(|c| texto(l, c).unwrap_or_default()).collect()).collect();
    escrever_tsv_com_cabecalho(&nomes, &linhas)
}

/// O intervalo no `formato` do "Copiar como" ([`tabular::colunas::escrever_como`]): `nome` dá o nome de cada coluna da
/// grade e `valor` cada célula ([`valor_de_copia`] nos Dados e na Consulta; [`ValorCopia::do_csv`] no CSV e no Excel).
pub fn texto_do_intervalo_como<'a>(
    formato: FormatoCopia,
    intervalo: Intervalo,
    nome: impl Fn(usize) -> String,
    mut valor: impl FnMut(usize, usize) -> ValorCopia<'a>,
    opcoes: &OpcoesCopia<'_>,
) -> String {
    let nomes: Vec<String> = intervalo.colunas().map(nome).collect();
    let linhas: Vec<Vec<ValorCopia<'a>>> =
        intervalo.linhas().map(|l| intervalo.colunas().map(|c| valor(l, c)).collect()).collect();
    escrever_como(formato, &nomes, &linhas, opcoes)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ofertas_de_leitura() {
        let uma = Intervalo::entre((0, 1), (3, 1));
        let o = OfertasCelula::de_leitura(uma);
        assert!(o.ligado(AcaoCelula::Copiar) && o.ligado(AcaoCelula::CopiarComCabecalho));
        assert!(FormatoCopia::TODOS.iter().all(|&f| o.ligado(AcaoCelula::CopiarComo(f))));
        assert_eq!(*o.get(AcaoCelula::Colar), OfertaCelula::Oculta, "o padrão esconde");
        let duas = OfertasCelula::de_leitura(Intervalo::entre((0, 1), (0, 2)));
        assert_eq!(
            *duas.get(AcaoCelula::CopiarComo(FormatoCopia::ListaIn)),
            OfertaCelula::Desligada(MOTIVO_LISTA_IN.into())
        );
        let o = o
            .ligar_se(AcaoCelula::Colar, false, "O banco está aberto só para leitura.")
            .ligar(AcaoCelula::Colar)
            .desligar(AcaoCelula::Limpar, "A coluna id é gerada.");
        assert!(o.ligado(AcaoCelula::Colar), "a última oferta vale");
        assert_eq!(*o.get(AcaoCelula::Limpar), OfertaCelula::Desligada("A coluna id é gerada.".into()));
    }

    #[test]
    fn grupos_visiveis() {
        let o = OfertasCelula::default().ligar(AcaoCelula::CopiarComo(FormatoCopia::Json));
        assert!(Entrada::CopiarComo.visivel(&o), "um formato basta para o submenu");
        assert!(!Entrada::Item(AcaoCelula::Copiar).visivel(&o));
        assert!(!Entrada::CopiarComo.visivel(&OfertasCelula::default()));
        assert_eq!(GRUPOS.iter().map(|g| g.len()).sum::<usize>(), 11, "os itens da A.1 (o submenu conta um)");
    }

    #[test]
    fn rotulos_e_atalhos() {
        assert_eq!(AcaoCelula::CopiarComCabecalho.atalho(), Some("Ctrl+Shift+C"));
        assert_eq!(AcaoCelula::Limpar.atalho(), Some("Delete"));
        assert_eq!(AcaoCelula::CopiarComo(FormatoCopia::ListaIn).rotulo(), "Lista para IN (…)");
        assert_eq!(AcaoCelula::DefinirNulo.atalho(), None);
    }

    #[test]
    fn valores_do_banco() {
        assert_eq!(valor_de_copia(&Celula::Nulo), ValorCopia::Nulo);
        assert_eq!(valor_de_copia(&Celula::Inteiro(-5)), ValorCopia::numero("-5"));
        assert_eq!(valor_de_copia(&Celula::Real(17051.0)), ValorCopia::numero("17051"));
        assert_eq!(valor_de_copia(&Celula::Real(f64::NAN)), ValorCopia::texto("NaN"));
        assert_eq!(valor_de_copia(&Celula::Texto("a\nb".into())), ValorCopia::texto("a\nb"));
        assert_eq!(valor_de_copia(&Celula::Blob { tamanho: 3, inicio: vec![1, 2, 3] }), ValorCopia::texto(""));
    }

    #[test]
    fn copiar_o_intervalo() {
        let celulas = [["id", "nome", "limite"], ["1", "Ana", "10,5"], ["2", "Bia", ""]];
        let nome = |c: usize| celulas[0][c].to_string();
        let iv = Intervalo::entre((1, 1), (2, 2));
        assert_eq!(
            tsv_do_intervalo_com_cabecalho(iv, nome, |l, c| Some(celulas[l][c].to_string())),
            "nome\tlimite\r\nAna\t10,5\r\nBia\t\r\n"
        );
        let valor = |l: usize, c: usize| ValorCopia::do_csv(celulas[l][c], c == 2, ',');
        let opcoes = OpcoesCopia { separador: ';', tabela: "clientes" };
        assert_eq!(
            texto_do_intervalo_como(FormatoCopia::Json, iv, nome, valor, &opcoes),
            "[\n  {\"nome\": \"Ana\", \"limite\": 10.5},\n  {\"nome\": \"Bia\", \"limite\": null}\n]\n"
        );
        assert_eq!(
            texto_do_intervalo_como(FormatoCopia::Csv, iv, nome, valor, &opcoes),
            "nome;limite\r\nAna;10,5\r\nBia;\r\n"
        );
        let coluna = Intervalo::entre((1, 1), (2, 1));
        assert_eq!(texto_do_intervalo_como(FormatoCopia::ListaIn, coluna, nome, valor, &opcoes), "('Ana', 'Bia')");
    }
}
