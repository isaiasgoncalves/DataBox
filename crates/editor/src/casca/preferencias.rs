//! A janela Preferências (prancheta 13, `Preferencias.dc.html`): Arquivo › Preferências (Ctrl+,), Exibir › Tema ›
//! "Preferências de aparência" (direto na Aparência) e o botão do pé da barra de ícones. É uma janela global, como a
//! Exportar: `.larga` (760px), o cabeçalho com a engrenagem de 20px, o corpo em grade 184 / 1fr (altura fixa de 432px,
//! revisão de UI de 19/10/2026: a janela não muda de tamanho entre as seções; a seção mais longa rola)
//! e o rodapé com "Restaurar o padrão" à esquerda e Cancelar e Salvar à direita.
//!
//! **As seções** (a navegação à esquerda, nós de [`No::lateral`] com o sólido de 20px; o `selected` do AccessKit marca a
//! que está à vista), só com opções que o app já sabe fazer:
//! - **Geral**: limpar a lista de "Abertos recentemente".
//! - **Aparência**: o tema (Seguir o Windows, Claro, Escuro), o idioma (Seguir o sistema, com o idioma que ele dá entre
//!   parênteses, e cada idioma de [`idioma::IDIOMAS`] com o nome no próprio idioma; prancheta "Preferências com o
//!   idioma"), as quatro "cores do código" (cartas com a amostra de três linhas no realce de verdade) e a fonte do
//!   código (a família entre as que existem na máquina e o tamanho).
//! - **Consulta SQL**: limpar o histórico de comandos.
//! - **Exportação**: as opções que a janela Exportar guarda (separador, decimal, cabeçalho, proteção de fórmulas,
//!   JSON indentado).
//!
//! A seção **Tabelas** da prancheta fica fora da lista: a Dados só tem o "N por página", que mora no endereço de cada
//! aba e não nas preferências, e nada mais ali é configurável. Ela volta quando ganhar uma opção de verdade. O
//! "abrir bancos só para leitura por padrão" do plano também não entrou: não há essa chave no arquivo de preferências.
//!
//! **Vale na hora**: ao abrir, a janela guarda o retrato do que valia (o tema, a
//! aparência do código em vigor e as opções de exportação). Cada mudança aplica na hora, por [`Efeito`]s que o editor
//! cumpre (o tema por `tema::aplicar` e pelo `SetTheme` do egui, o idioma por `idioma::definir`, as cores e o tamanho
//! por `codigo::definir`, a fonte numa `Promessa` que lê o arquivo, seguida de `fontes::trocar_mono`). Nada é gravado até o Salvar. Cancelar, Esc, o ×
//! e o clique no fundo reaplicam o retrato e fecham. O Salvar só liga quando algo difere do retrato e grava só o que
//! mudou (a chave `"aparencia"` só entra no arquivo depois de a pessoa escolher alguma coisa nela, e a `"idioma"` só
//! depois de escolher o idioma). As duas limpezas
//! (recentes e histórico) são ações do Salvar: o botão marca a limpeza, que o Cancelar desfaz. "Restaurar o padrão"
//! mexe só na seção visível.

use egui::{
    Align, Color32, FontId, Id, Layout, Rect, Response, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo,
    WidgetType, pos2, vec2,
};
use estado::{Estado, Promessa};
use idioma::t;
use interface::codigo::{self, CoresCodigo as CoresDoInterface};
use interface::fontes::{self, Peso};
use interface::icones::Icone;
use interface::pecas::arranjo::linha;
use interface::pecas::arvore::No;
use interface::pecas::botao::{Botao, pintor_de_fora};
use interface::pecas::campos::{Check, Select};
use interface::pecas::janela::Janela;
use interface::pecas::navegacao::{TamanhoTrilho, trilho};
use interface::pecas::texto::{Paragrafo, galley, galley_quebrado, h3};
use interface::pintura::{self, RAIO_PADRAO};
use interface::tema::{Tema, paleta, rgba};
use prefs::{CoresCodigo, Mudanca, MudancaAparencia, MudancaExportacao, TAMANHO_CODIGO_PADRAO};

/// A largura da navegação (`.pref-corpo{grid-template-columns:184px 1fr}`) e a altura do corpo, fixa (revisão de UI
/// de 19/10/2026).
pub(crate) const LARGURA_NAVEGACAO: f32 = 184.0;
pub(crate) const ALTURA_CORPO: f32 = 432.0;
/// O respiro da seção (`.pref-sec{padding:16px 22px 18px}`).
const RECUO_SECAO: f32 = 22.0;
const TOPO_SECAO: f32 = 16.0;
/// A linha da seção (`.pref-l`): a coluna do rótulo, o espaço até o controle e o `padding:14px 0`.
const COLUNA_ROTULO: f32 = 140.0;
const ESPACO_COLUNAS: f32 = 18.0;
const RESPIRO_LINHA: f32 = 14.0;
/// As cartas das cores do código (`.cc`): duas colunas de `gap:10px`; cada carta com `padding:6px 6px 8px`, a amostra
/// de 62px (três linhas de 16px com `padding:7px 9px`), `gap:7px` e o nome de 17px.
const ESPACO_CARTAS: f32 = 10.0;
const RECUO_CARTA: f32 = 6.0;
const ALTURA_AMOSTRA: f32 = 62.0;
const ALTURA_NOME_CARTA: f32 = 17.0;
/// A borda de 1px da carta entra na altura (`box-sizing:border-box` não vale aqui: o `padding` fica por dentro da borda).
const BORDA_CARTA: f32 = 1.0;
const DENTRO_CARTA: f32 = BORDA_CARTA + RECUO_CARTA;
const ALTURA_CARTA: f32 = BORDA_CARTA + RECUO_CARTA + ALTURA_AMOSTRA + 7.0 + ALTURA_NOME_CARTA + 8.0 + BORDA_CARTA;
/// O select do tamanho da fonte (`width:92px`) e o espaço entre os dois selects (`.pref-fonte{gap:8px}`).
const LARGURA_TAMANHO: f32 = 92.0;
/// Os tamanhos da fonte do código, em px (item 16 da revisão).
const TAMANHOS: [f32; 6] = [11.0, 12.0, 12.5, 13.0, 14.0, 16.0];

/// As três linhas da amostra de cada carta (`SELECT nome, limite` / `FROM "clientes"` / `WHERE limite > 1000;`, com os
/// verbos alinhados à direita como na prancheta).
const AMOSTRA: &str = "SELECT nome, limite\n  FROM \"clientes\"\n WHERE limite > 1000;";

const AJUDA_DECIMAL: &str =
    "Só para números com casas decimais: o Excel em português espera vírgula. Números inteiros não mudam.";
const AJUDA_FORMULAS: &str = "Um texto que começa com =, +, - ou @ seria lido pelo Excel como fórmula. Com a \
                              proteção, ele ganha um apóstrofo na frente e abre como texto.";
const SEPARADORES: [(char, &str); 3] =
    [(';', "Ponto e vírgula (Excel em português)"), (',', "Vírgula"), ('\t', "Tabulação")];
const DECIMAIS: [(char, &str); 2] = [(',', "Vírgula"), ('.', "Ponto")];

const DICA_FECHAR: &str = "Fechar sem salvar (Esc): desfaz o que você mudou";
const DICA_CANCELAR: &str = "Volta ao que estava quando a janela abriu (Esc)";
const DICA_SALVAR: &str = "As mudanças já valem na hora; Salvar guarda para as próximas aberturas";
const DICA_SALVAR_DESLIGADO: &str = "Nenhuma mudança para salvar";
const DICA_RESTAURAR_APARENCIA: &str = "Restaura o padrão só desta seção (Aparência): seguir o Windows, o idioma do \
                                         sistema, cores DataBox, Consolas 12,5 px";
const DICA_RESTAURAR_EXPORTACAO: &str = "Restaura o padrão só desta seção (Exportação): separador ponto e vírgula, \
                                         decimal vírgula, primeira linha com os nomes, proteção de fórmulas e JSON \
                                         indentado";

/// A seção à vista. Tabelas (a quinta da prancheta) não está aqui de propósito: ver o topo do módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Secao {
    Geral,
    Aparencia,
    ConsultaSql,
    Exportacao,
}

impl Secao {
    /// As seções da navegação, na ordem da prancheta.
    pub(crate) const TODAS: [Secao; 4] = [Secao::Geral, Secao::Aparencia, Secao::ConsultaSql, Secao::Exportacao];

    /// O nome do nó e o título da seção.
    pub(crate) fn rotulo(self) -> &'static str {
        match self {
            Secao::Geral => "Geral",
            Secao::Aparencia => "Aparência",
            Secao::ConsultaSql => "Consulta SQL",
            Secao::Exportacao => "Exportação",
        }
    }

    /// O sólido de 20px do nó. A Consulta SQL usa a folha SQL (`Icone::Consulta`, a mesma que `Folha(Sql)` com o PNG de 20).
    fn icone(self) -> Icone {
        match self {
            Secao::Geral => Icone::Geral,
            Secao::Aparencia => Icone::Aparencia,
            Secao::ConsultaSql => Icone::Consulta,
            Secao::Exportacao => Icone::Exportacao,
        }
    }

    /// Tem um padrão a restaurar (o "Restaurar o padrão" do rodapé só aparece nessas).
    fn tem_padrao(self) -> bool {
        matches!(self, Secao::Aparencia | Secao::Exportacao)
    }

    fn dica_restaurar(self) -> &'static str {
        match self {
            Secao::Exportacao => DICA_RESTAURAR_EXPORTACAO,
            _ => DICA_RESTAURAR_APARENCIA,
        }
    }
}

/// Tudo o que a janela mexe, nos valores que valem (as opções de exportação sem os `None` de "nunca escolhido").
#[derive(Debug, Clone, PartialEq)]
struct Valores {
    tema: prefs::Tema,
    idioma: idioma::Escolha,
    cores: CoresCodigo,
    /// None = a família padrão (Consolas).
    familia: Option<String>,
    tamanho: f32,
    separador: char,
    decimal: char,
    cabecalho: bool,
    proteger_formulas: bool,
    indentado: bool,
}

/// A família como o arquivo a guarda: a padrão (Consolas) é None.
fn normalizar_familia(familia: Option<String>) -> Option<String> {
    familia.filter(|f| f != fontes::FAMILIA_MONO_PADRAO)
}

impl Valores {
    /// O que vale agora: o tema do editor, a aparência do código em vigor no contexto e a exportação das preferências.
    fn em_vigor(ctx: &egui::Context, tema: Tema, estado: &Estado) -> Valores {
        let a = codigo::atual(ctx);
        let e = &estado.prefs.exportacao;
        Valores {
            tema: crate::app::tema_das_prefs(tema),
            idioma: estado.prefs.idioma,
            cores: CoresCodigo::de_texto(a.cores.como_str()).unwrap_or_default(),
            familia: normalizar_familia(a.familia),
            tamanho: a.tamanho,
            separador: e.csv_separador,
            decimal: e.csv_decimal,
            cabecalho: e.cabecalho.unwrap_or(true),
            proteger_formulas: e.csv_proteger_formulas.unwrap_or(true),
            indentado: e.json_indentado,
        }
    }

    /// O padrão da Aparência: seguir o Windows, o idioma do sistema, cores DataBox, Consolas 12,5 px.
    fn restaurar_aparencia(&mut self) {
        self.tema = prefs::Tema::Sistema;
        self.idioma = idioma::Escolha::Sistema;
        self.cores = CoresCodigo::DataBox;
        self.familia = None;
        self.tamanho = TAMANHO_CODIGO_PADRAO;
    }

    /// O padrão da Exportação (o de `OpcoesExportacao::default`, com o cabeçalho e a proteção marcados).
    fn restaurar_exportacao(&mut self) {
        let padrao = prefs::OpcoesExportacao::default();
        self.separador = padrao.csv_separador;
        self.decimal = padrao.csv_decimal;
        self.cabecalho = true;
        self.proteger_formulas = true;
        self.indentado = padrao.json_indentado;
    }

    fn aparencia_igual(&self, outro: &Valores) -> bool {
        self.cores == outro.cores && self.familia == outro.familia && self.tamanho == outro.tamanho
    }

    fn exportacao_igual(&self, outro: &Valores) -> bool {
        self.separador == outro.separador
            && self.decimal == outro.decimal
            && self.cabecalho == outro.cabecalho
            && self.proteger_formulas == outro.proteger_formulas
            && self.indentado == outro.indentado
    }
}

/// Uma mudança a pôr em vigor agora (o editor a cumpre: ele é dono do tema, do contexto e da leitura da fonte).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Efeito {
    Tema(prefs::Tema),
    /// O idioma da interface (a escolha; o editor resolve "seguir o sistema" e chama o `idioma::definir`).
    Idioma(idioma::Escolha),
    Cores(CoresCodigo),
    /// None = a família padrão.
    Fonte(Option<String>),
    Tamanho(f32),
}

/// O que leva `de` a `para`, na ordem em que o editor aplica.
fn diferencas(de: &Valores, para: &Valores) -> Vec<Efeito> {
    let mut efeitos = Vec::new();
    if de.tema != para.tema {
        efeitos.push(Efeito::Tema(para.tema));
    }
    if de.idioma != para.idioma {
        efeitos.push(Efeito::Idioma(para.idioma));
    }
    if de.cores != para.cores {
        efeitos.push(Efeito::Cores(para.cores));
    }
    if de.familia != para.familia {
        efeitos.push(Efeito::Fonte(para.familia.clone()));
    }
    if de.tamanho != para.tamanho {
        efeitos.push(Efeito::Tamanho(para.tamanho));
    }
    efeitos
}

/// O que o Salvar grava.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Gravacao {
    /// Só o que mudou em relação ao retrato (a chave `"aparencia"` só vai se algo da aparência mudou).
    pub mudanca: Mudanca,
    pub limpar_recentes: bool,
    pub limpar_historico: bool,
}

/// Como a janela terminou.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Fim {
    /// Cancelar, Esc, × ou o clique no fundo: `reverter` leva o que vale agora de volta ao retrato da abertura.
    Cancelou {
        reverter: Vec<Efeito>,
    },
    Salvou(Gravacao),
}

/// O que a janela devolve a cada quadro.
#[derive(Debug, Default)]
pub(crate) struct Saida {
    /// O que mudou neste quadro e vale na hora.
    pub efeitos: Vec<Efeito>,
    pub fim: Option<Fim>,
}

/// A janela Preferências.
pub(crate) struct JanelaPreferencias {
    secao: Secao,
    /// O que valia quando a janela abriu.
    retrato: Valores,
    /// O que está escolhido (e aplicado) agora.
    atual: Valores,
    /// As famílias do código que a máquina tem, na ordem das candidatas: a padrão sempre primeiro. Começa só com a
    /// padrão (e a em vigor); as outras chegam da leitura em `familias_pendentes`.
    familias: Vec<String>,
    /// A procura das famílias nas pastas de fontes (um `stat` por candidata), numa tarefa: a thread da interface não
    /// toca no disco.
    familias_pendentes: Option<Promessa<Vec<String>>>,
    limpar_recentes: bool,
    limpar_historico: bool,
}

/// O rótulo da família padrão do código na lista: "Padrão do sistema (Consolas)".
pub(crate) fn rotulo_da_familia_padrao() -> String {
    format!("Padrão do sistema ({})", fontes::FAMILIA_MONO_PADRAO)
}

/// A lista de famílias: a padrão primeiro, depois as achadas e, por último, a em vigor se não estiver entre elas.
fn lista_de_familias(achadas: &[String], em_vigor: Option<&String>) -> Vec<String> {
    let mut familias: Vec<String> = achadas.to_vec();
    familias.retain(|f| f != fontes::FAMILIA_MONO_PADRAO);
    familias.insert(0, fontes::FAMILIA_MONO_PADRAO.to_string());
    if let Some(atual) = em_vigor
        && !familias.contains(atual)
    {
        familias.push(atual.clone());
    }
    familias
}

impl JanelaPreferencias {
    /// Abre na `secao`, com o retrato do que vale agora. As famílias que a máquina tem vêm de uma tarefa (a lista
    /// começa com a padrão e a em vigor).
    pub(crate) fn nova(ctx: &egui::Context, tema: Tema, estado: &Estado, secao: Secao) -> JanelaPreferencias {
        let retrato = Valores::em_vigor(ctx, tema, estado);
        let familias = lista_de_familias(&[], retrato.familia.as_ref());
        let procura = Promessa::rodar(ctx, || {
            Ok(fontes::familias_mono_disponiveis().into_iter().map(|f| f.nome.to_string()).collect())
        });
        JanelaPreferencias {
            secao,
            atual: retrato.clone(),
            retrato,
            familias,
            familias_pendentes: Some(procura),
            limpar_recentes: false,
            limpar_historico: false,
        }
    }

    /// A procura das famílias ainda corre (para os testes esperarem).
    pub(crate) fn ocupada(&self) -> bool {
        self.familias_pendentes.is_some()
    }

    /// Toma a lista de famílias quando a tarefa termina (sem ela, ficam a padrão e a em vigor).
    fn tomar_familias(&mut self) {
        if let Some(r) = self.familias_pendentes.as_mut().and_then(Promessa::tomar) {
            self.familias_pendentes = None;
            if let Ok(achadas) = r {
                self.familias = lista_de_familias(&achadas, self.retrato.familia.as_ref());
            }
        }
    }

    /// A seção à vista (para os testes).
    pub(crate) fn secao(&self) -> Secao {
        self.secao
    }

    /// Algo difere do que valia na abertura (o Salvar liga).
    pub(crate) fn mudou(&self) -> bool {
        self.atual != self.retrato || self.limpar_recentes || self.limpar_historico
    }

    /// A fonte escolhida não pôde ser lida: a escolha volta para a que está em vigor (`familia`, None = a padrão).
    pub(crate) fn fonte_recusada(&mut self, familia: Option<String>) {
        self.atual.familia = normalizar_familia(familia);
    }

    /// O que o Salvar grava agora: só o que difere do retrato.
    pub(crate) fn gravacao(&self) -> Gravacao {
        let (r, a) = (&self.retrato, &self.atual);
        let tema = (r.tema != a.tema).then_some(a.tema);
        let idioma = (r.idioma != a.idioma).then_some(a.idioma);
        let aparencia = (!r.aparencia_igual(a)).then(|| MudancaAparencia {
            cores_codigo: Some(a.cores),
            fonte_codigo: Some(a.familia.clone()),
            tamanho_codigo: Some(a.tamanho),
        });
        let exportacao = (!r.exportacao_igual(a)).then(|| MudancaExportacao {
            cabecalho: (r.cabecalho != a.cabecalho).then_some(a.cabecalho),
            csv_separador: (r.separador != a.separador).then_some(a.separador),
            csv_decimal: (r.decimal != a.decimal).then_some(a.decimal),
            json_indentado: (r.indentado != a.indentado).then_some(a.indentado),
            csv_proteger_formulas: (r.proteger_formulas != a.proteger_formulas).then_some(a.proteger_formulas),
        });
        Gravacao {
            mudanca: Mudanca { tema, exportacao, aparencia, idioma, ..Mudanca::default() },
            limpar_recentes: self.limpar_recentes,
            limpar_historico: self.limpar_historico,
        }
    }

    /// Desenha a janela. As mudanças do quadro saem em [`Saida::efeitos`] (já valem na janela; o editor as põe em
    /// vigor), e [`Saida::fim`] diz quando ela terminou.
    pub(crate) fn mostrar(&mut self, ctx: &egui::Context, estado: &Estado) -> Saida {
        self.tomar_familias();
        let antes = self.atual.clone();
        let (mudou, secao) = (self.mudou(), self.secao);
        let (mut restaurar, mut cancelar, mut salvar) = (false, false, false);
        let resposta = Janela::new("preferencias", "Preferências")
            .icone(Icone::Preferencias)
            .larga(true)
            .corpo_cru(true)
            .dica_fechar(DICA_FECHAR)
            .rodape_esquerda(|ui| {
                if secao.tem_padrao() {
                    restaurar = ui.add(Botao::ghost("Restaurar o padrão").dica(secao.dica_restaurar())).clicked();
                }
            })
            .rodape_direita(|ui| {
                cancelar = ui.add(Botao::ghost("Cancelar").dica(DICA_CANCELAR)).clicked();
                let botao = ui.add(Botao::primario("Salvar").habilitado(mudou).dica(DICA_SALVAR));
                salvar = botao.on_disabled_hover_text(DICA_SALVAR_DESLIGADO).clicked();
            })
            .mostrar(ctx, |ui| self.corpo(ui, estado));
        if restaurar {
            match secao {
                Secao::Aparencia => self.atual.restaurar_aparencia(),
                Secao::Exportacao => self.atual.restaurar_exportacao(),
                Secao::Geral | Secao::ConsultaSql => {}
            }
        }
        let efeitos = diferencas(&antes, &self.atual);
        let fim = if salvar && self.mudou() {
            Some(Fim::Salvou(self.gravacao()))
        } else if cancelar || resposta.pediu_fechar {
            Some(Fim::Cancelou { reverter: diferencas(&self.atual, &self.retrato) })
        } else {
            None
        };
        Saida { efeitos, fim }
    }

    // ------------------------------------------------------------------ o corpo

    /// A grade 184 / 1fr: a navegação (com o fio da direita) e a seção, na altura fixa de 432px (a seção rola).
    fn corpo(&mut self, ui: &mut Ui, estado: &Estado) {
        let e = paleta(ui).estudio;
        let topo = ui.cursor().min;
        let largura = ui.available_width();
        let id = ui.id().with("preferencias");

        // a navegação: `padding:12px 0` e `gap:2px` entre os nós
        let rect_nav = Rect::from_min_size(topo, vec2(LARGURA_NAVEGACAO, f32::INFINITY));
        let mut nav = ui.new_child(UiBuilder::new().id_salt(id.with("nav")).max_rect(rect_nav));
        nav.spacing_mut().item_spacing = vec2(0.0, 2.0);
        nav.add_space(12.0 - 2.0);
        for secao in Secao::TODAS {
            let r = No::new(secao.rotulo()).icone(secao.icone()).lateral().ativo(secao == self.secao).mostrar(&mut nav);
            if r.resposta.clicked() {
                self.secao = secao;
            }
        }
        nav.add_space(12.0);

        // a seção: `padding:16px 22px 18px`, numa rolagem na altura fixa do corpo
        let x = topo.x + LARGURA_NAVEGACAO;
        let area = Rect::from_min_max(pos2(x, topo.y), pos2(topo.x + largura, topo.y + ALTURA_CORPO));
        let mut ui_area =
            ui.new_child(UiBuilder::new().id_salt(id.with("area")).max_rect(area).layout(Layout::top_down(Align::Min)));
        ui_area.set_clip_rect(area.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt(id.with(("rolagem", self.secao)))
            .auto_shrink([false, false])
            .max_height(ALTURA_CORPO)
            .show(&mut ui_area, |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: RECUO_SECAO as i8,
                        right: RECUO_SECAO as i8,
                        top: TOPO_SECAO as i8,
                        bottom: 18,
                    })
                    .show(ui, |secao| {
                        secao.set_width((area.width() - 2.0 * RECUO_SECAO).max(80.0));
                        secao.spacing_mut().item_spacing = vec2(8.0, 0.0);
                        match self.secao {
                            Secao::Geral => self.secao_geral(secao, estado),
                            Secao::Aparencia => self.secao_aparencia(secao),
                            Secao::ConsultaSql => self.secao_consulta(secao),
                            Secao::Exportacao => self.secao_exportacao(secao),
                        }
                    });
            });

        let altura = ALTURA_CORPO.max(nav.min_rect().height());
        let corpo = Rect::from_min_size(topo, vec2(largura, altura));
        // `.pref-nav{border-right:1px solid var(--vidrob)}`
        ui.painter().vline(x - 0.5, corpo.y_range(), Stroke::new(1.0, e.vidrob));
        ui.allocate_rect(corpo, Sense::hover());
    }

    /// O título da seção (`h3`: 14px / 600, `margin:0 0 2px`).
    fn titulo(ui: &mut Ui, texto: &str) {
        h3(ui, texto);
        ui.add_space(2.0);
    }

    fn secao_geral(&mut self, ui: &mut Ui, estado: &Estado) {
        Self::titulo(ui, Secao::Geral.rotulo());
        let n = estado.prefs.recentes.len();
        let situacao = contagem_de_recentes(&estado.prefs.recentes);
        let limpar = &mut self.limpar_recentes;
        linha_da_secao(
            ui,
            "recentes",
            "Abertos recentemente",
            Some("A lista do menu Arquivo e do Início."),
            &Opcoes { altura_controle: 28.0, ..Opcoes::ULTIMA },
            |ui| {
                acao_de_limpar(
                    ui,
                    limpar,
                    &Limpeza {
                        situacao: &situacao,
                        limpar: "Limpar a lista",
                        manter: "Manter a lista",
                        aviso: "A lista será limpa ao salvar.",
                        ha_o_que_limpar: n > 0,
                    },
                );
            },
        );
    }

    fn secao_consulta(&mut self, ui: &mut Ui) {
        Self::titulo(ui, Secao::ConsultaSql.rotulo());
        let limpar = &mut self.limpar_historico;
        linha_da_secao(
            ui,
            "historico",
            "Histórico de consultas",
            Some("As consultas executadas na Consulta SQL, de todos os bancos."),
            &Opcoes { altura_controle: 28.0, ..Opcoes::ULTIMA },
            |ui| {
                acao_de_limpar(
                    ui,
                    limpar,
                    &Limpeza {
                        situacao: "",
                        limpar: "Limpar o histórico",
                        manter: "Manter o histórico",
                        aviso: "O histórico será limpo ao salvar.",
                        ha_o_que_limpar: true,
                    },
                );
            },
        );
    }

    fn secao_aparencia(&mut self, ui: &mut Ui) {
        Self::titulo(ui, Secao::Aparencia.rotulo());
        let id = Id::new("preferencias::aparencia");

        // o tema: o trilho de ponta a ponta da coluna do controle
        let temas = super::menus::TEMAS;
        let rotulos = temas.map(|(_, r)| r);
        let escolhido = temas.iter().position(|(t, _)| crate::app::tema_das_prefs(*t) == self.atual.tema);
        let atual = &mut self.atual;
        linha_da_secao(
            ui,
            "tema",
            "Tema",
            Some("Cores da janela, dos painéis e dos documentos."),
            &Opcoes { altura_controle: 32.0, ..Opcoes::PADRAO },
            |ui| {
                let (clicado, _) = trilho(ui, id.with("tema"), &rotulos, escolhido, TamanhoTrilho::CheioJusto, true);
                if let Some((t, _)) = clicado.and_then(|i| temas.get(i)) {
                    atual.tema = crate::app::tema_das_prefs(*t);
                }
            },
        );

        // o idioma: a lista de ponta a ponta da coluna do controle (P7)
        let escolhas = rotulos_dos_idiomas(idioma::do_sistema());
        let opcoes_idioma: Vec<(idioma::Escolha, &str)> = escolhas.iter().map(|(e, r)| (*e, r.as_str())).collect();
        let mut escolha = atual.idioma;
        let ajuda_idioma = t!("Menus, janelas e mensagens; os números e as datas seguem o idioma.");
        linha_da_secao(
            ui,
            "idioma",
            &t!("Idioma"),
            Some(&ajuda_idioma),
            &Opcoes { altura_controle: 28.0, ..Opcoes::PADRAO },
            |ui| {
                let largura = ui.available_width();
                Select::new(id.with("idioma"), &mut escolha, &opcoes_idioma)
                    .rotulo(&t!("Idioma"))
                    .largura(largura)
                    .mostrar(ui);
            },
        );
        atual.idioma = escolha;

        // as quatro cartas das cores do código
        linha_da_secao(
            ui,
            "cores",
            "Cores do código",
            Some("Consulta SQL, JSON, texto e o SQL que as janelas mostram."),
            &Opcoes { alta: true, ..Opcoes::PADRAO },
            |ui| cartas(ui, &mut atual.cores),
        );

        // a fonte do código: a família e o tamanho
        let nomes = &self.familias;
        // a padrão (a primeira) diz que é a do sistema (revisão de UI de 19/10/2026): "Padrão do sistema (Consolas)"
        let padrao = rotulo_da_familia_padrao();
        let opcoes_familia: Vec<(usize, &str)> =
            nomes.iter().enumerate().map(|(i, n)| (i, if i == 0 { padrao.as_str() } else { n.as_str() })).collect();
        let mut familia = nomes.iter().position(|n| Some(n.as_str()) == atual.familia.as_deref()).unwrap_or(0);
        let rotulos_tamanho = rotulos_dos_tamanhos(atual.tamanho);
        let opcoes_tamanho: Vec<(f32, &str)> = rotulos_tamanho.iter().map(|(v, r)| (*v, r.as_str())).collect();
        let mut tamanho = atual.tamanho;
        linha_da_secao(
            ui,
            "fonte",
            "Fonte do código",
            Some("Também vale para os números das linhas."),
            &Opcoes { altura_controle: 28.0, ultima: true, ..Opcoes::PADRAO },
            |ui| {
                let largura = ui.available_width();
                linha(ui, 28.0, 8.0, |ui| {
                    let da_familia = (largura - 8.0 - LARGURA_TAMANHO).max(80.0);
                    Select::new(id.with("familia"), &mut familia, &opcoes_familia)
                        .rotulo("Família da fonte do código")
                        .largura(da_familia)
                        .mostrar(ui);
                    Select::new(id.with("tamanho"), &mut tamanho, &opcoes_tamanho)
                        .rotulo("Tamanho da fonte do código")
                        .largura(LARGURA_TAMANHO)
                        .mostrar(ui);
                });
            },
        );
        atual.familia = normalizar_familia(nomes.get(familia).cloned());
        atual.tamanho = tamanho;
    }

    fn secao_exportacao(&mut self, ui: &mut Ui) {
        Self::titulo(ui, Secao::Exportacao.rotulo());
        let id = Id::new("preferencias::exportacao");
        let a = &mut self.atual;
        let (mut separador, mut decimal) = (a.separador, a.decimal);
        linha_da_secao(ui, "separador", "Separador", None, &Opcoes { altura_controle: 28.0, ..Opcoes::PADRAO }, |ui| {
            Select::new(id.with("separador"), &mut separador, &SEPARADORES).rotulo("Separador").mostrar(ui);
        });
        linha_da_secao(
            ui,
            "decimal",
            "Decimal",
            Some(AJUDA_DECIMAL),
            &Opcoes { altura_controle: 28.0, ..Opcoes::PADRAO },
            |ui| {
                Select::new(id.with("decimal"), &mut decimal, &DECIMAIS).rotulo("Decimal").mostrar(ui);
            },
        );
        a.separador = separador;
        a.decimal = decimal;
        linha_com_caixa(ui, "cabecalho", &mut a.cabecalho, "CSV: primeira linha com os nomes das colunas", None, false);
        linha_com_caixa(
            ui,
            "formulas",
            &mut a.proteger_formulas,
            "Proteger contra fórmulas do Excel",
            Some(AJUDA_FORMULAS),
            false,
        );
        linha_com_caixa(ui, "indentado", &mut a.indentado, "JSON indentado (mais fácil de ler)", None, true);
    }
}

/// O que há na lista de Abertos recentemente, pelo tipo: "Nenhum item na lista"; com um tipo só, "3 bancos na lista",
/// "1 pasta na lista"; com mais de um, o total e cada tipo que aparece ("5 itens na lista: 3 bancos, 1 pasta e 1 arquivo").
fn contagem_de_recentes(recentes: &[prefs::Recente]) -> String {
    use interface::formato::{formatar_numero, plural};
    use prefs::TipoRecente;
    let partes: Vec<String> = [
        (TipoRecente::Banco, "banco", "bancos"),
        (TipoRecente::Pasta, "pasta", "pastas"),
        (TipoRecente::Arquivo, "arquivo", "arquivos"),
    ]
    .into_iter()
    .filter_map(|(tipo, um, varios)| {
        let n = recentes.iter().filter(|r| r.tipo == tipo).count();
        (n > 0).then(|| plural(n as u64, um, varios))
    })
    .collect();
    match partes.as_slice() {
        [] => "Nenhum item na lista".to_string(),
        [so] => format!("{so} na lista"),
        [antes @ .., ultima] => {
            format!("{} itens na lista: {} e {ultima}", formatar_numero(recentes.len()), antes.join(", "))
        }
    }
}

/// As opções do Idioma: "Seguir o sistema (Português)", com o idioma que o sistema dá agora, e cada idioma pelo nome
/// no próprio idioma ("Português (Brasil)", "English"), que não se traduz.
pub(crate) fn rotulos_dos_idiomas(do_sistema: idioma::Idioma) -> Vec<(idioma::Escolha, String)> {
    idioma::Escolha::todas()
        .into_iter()
        .map(|e| {
            let rotulo = match e {
                idioma::Escolha::Sistema => {
                    t!("Seguir o sistema ({idioma})", idioma = do_sistema.nome_curto()).into_owned()
                }
                idioma::Escolha::Idioma(i) => i.nome().to_string(),
            };
            (e, rotulo)
        })
        .collect()
}

/// Os tamanhos da fonte com o texto de cada um ("12,5 px"); o tamanho de agora entra se não for um deles (um valor
/// do arquivo feito à mão).
fn rotulos_dos_tamanhos(atual: f32) -> Vec<(f32, String)> {
    let mut tamanhos = TAMANHOS.to_vec();
    if !tamanhos.contains(&atual) {
        tamanhos.push(atual);
        tamanhos.sort_by(f32::total_cmp);
    }
    tamanhos.into_iter().map(|t| (t, rotulo_do_tamanho(t))).collect()
}

/// "12 px", "12,5 px".
fn rotulo_do_tamanho(tamanho: f32) -> String {
    let numero =
        if tamanho.fract() == 0.0 { format!("{}", tamanho as i32) } else { format!("{tamanho}").replace('.', ",") };
    format!("{numero} px")
}

// ---------------------------------------------------------------------- as linhas da seção

/// Como uma linha da seção (`.pref-l`) se comporta.
struct Opcoes {
    /// `.pref-l.alto`: o rótulo e o controle começam no alto (as cartas); senão, ficam centrados um no outro.
    alta: bool,
    /// A altura do controle (para centrar o rótulo ou o controle na outra coluna): o trilho 32, os campos 28.
    altura_controle: f32,
    /// A última linha da seção não tem o fio de baixo (`.pref-l:last-child`).
    ultima: bool,
}

impl Opcoes {
    const PADRAO: Opcoes = Opcoes { alta: false, altura_controle: 28.0, ultima: false };
    const ULTIMA: Opcoes = Opcoes { alta: false, altura_controle: 28.0, ultima: true };
}

/// O texto de uma linha, alocado onde está e dito ao leitor de tela.
fn texto_da_linha(ui: &Ui, id: Id, pos: egui::Pos2, g: &Paragrafo, cor: Color32, rotulo: &str) {
    let resp = ui.interact(Rect::from_min_size(pos, g.tamanho()), id, Sense::hover());
    g.pintar(ui.painter(), pos, cor);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, rotulo));
}

/// Uma linha `.pref-l`: a coluna do rótulo (`b` em 600 e o `span` de 11px `--t3`) com 140px, 18px de espaço e o
/// controle em `1fr`; `padding:14px 0` e o fio `--b` embaixo.
fn linha_da_secao(
    ui: &mut Ui,
    id: &str,
    titulo: &str,
    ajuda: Option<&str>,
    opcoes: &Opcoes,
    controle: impl FnOnce(&mut Ui),
) {
    let e = paleta(ui).estudio;
    let topo = ui.cursor().min;
    let largura = ui.available_width();
    let id_linha = ui.id().with(("pref-l", id));

    // o rótulo, medido antes (decide a altura da linha)
    let fonte_titulo = fontes::fonte(12.0, Peso::Medio);
    let altura_titulo = 17.0;
    let ajuda_quebrada =
        ajuda.map(|a| galley_quebrado(ui, a, fontes::fonte(11.0, Peso::Normal), e.t3, COLUNA_ROTULO, 15.0 / 11.0));
    let altura_rotulo = altura_titulo + ajuda_quebrada.as_ref().map_or(0.0, |g| 3.0 + g.tamanho().y);

    // o controle, no `1fr`; a linha alta o põe no alto, a outra o centra pela altura dele
    let largura_controle = (largura - COLUNA_ROTULO - ESPACO_COLUNAS).max(40.0);
    let x_controle = topo.x + COLUNA_ROTULO + ESPACO_COLUNAS;
    let miolo_previsto = altura_rotulo.max(opcoes.altura_controle);
    let y_controle =
        topo.y + RESPIRO_LINHA + if opcoes.alta { 0.0 } else { (miolo_previsto - opcoes.altura_controle) / 2.0 };
    let mut ui_controle = ui.new_child(
        UiBuilder::new()
            .id_salt(id_linha.with("controle"))
            .max_rect(Rect::from_min_size(pos2(x_controle, y_controle), vec2(largura_controle, f32::INFINITY)))
            .layout(Layout::top_down(Align::Min)),
    );
    ui_controle.spacing_mut().item_spacing = vec2(8.0, 0.0);
    controle(&mut ui_controle);
    let altura_controle = ui_controle.min_rect().height().max(0.0);
    let miolo = if opcoes.alta { altura_rotulo.max(altura_controle) } else { miolo_previsto.max(altura_controle) };

    // o rótulo
    let y_rotulo = topo.y + RESPIRO_LINHA + if opcoes.alta { 0.0 } else { (miolo - altura_rotulo) / 2.0 };
    let g_titulo = galley(ui, titulo, fonte_titulo, e.t);
    let pos_titulo = pos2(topo.x, y_rotulo + (altura_titulo - g_titulo.size().y) / 2.0);
    let resp = ui.interact(Rect::from_min_size(pos_titulo, g_titulo.size()), id_linha.with("titulo"), Sense::hover());
    ui.painter().galley(pos_titulo, g_titulo, e.t);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, titulo));
    if let (Some(g), Some(a)) = (&ajuda_quebrada, ajuda) {
        texto_da_linha(ui, id_linha.with("ajuda"), pos2(topo.x, y_rotulo + altura_titulo + 3.0), g, e.t3, a);
    }

    let total = RESPIRO_LINHA + miolo + RESPIRO_LINHA + if opcoes.ultima { 0.0 } else { 1.0 };
    if !opcoes.ultima {
        ui.painter().hline(topo.x..=topo.x + largura, topo.y + total - 0.5, Stroke::new(1.0, e.b));
    }
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, total)), Sense::hover());
}

/// Uma linha de uma caixa de marcar só (`.pref-l` de uma coluna): a caixa com o texto e, embaixo, a explicação em
/// 11px `--t3` recuada até o texto.
fn linha_com_caixa(ui: &mut Ui, id: &str, marcado: &mut bool, texto: &str, ajuda: Option<&str>, ultima: bool) {
    let e = paleta(ui).estudio;
    let topo = ui.cursor().min;
    let largura = ui.available_width();
    ui.add_space(RESPIRO_LINHA);
    Check::new(marcado, texto).mostrar(ui);
    if let Some(a) = ajuda {
        // o recuo da caixa de 16px e o `gap:8px` até o texto
        let recuo = 24.0;
        ui.add_space(3.0);
        let g =
            galley_quebrado(ui, a, fontes::fonte(11.0, Peso::Normal), e.t3, (largura - recuo).max(40.0), 15.0 / 11.0);
        let pos = pos2(topo.x + recuo, ui.cursor().min.y);
        let id_ajuda = ui.id().with(("pref-ajuda", id));
        texto_da_linha(ui, id_ajuda, pos, &g, e.t3, a);
        ui.allocate_space(vec2(largura, g.tamanho().y));
    }
    ui.add_space(RESPIRO_LINHA);
    let fim = ui.cursor().min.y;
    if !ultima {
        ui.painter().hline(topo.x..=topo.x + largura, fim + 0.5, Stroke::new(1.0, e.b));
        ui.add_space(1.0);
    }
}

/// O botão de uma limpeza que vale no Salvar, com o que ela diz e o que desfaz.
struct Limpeza<'a> {
    /// O que há agora ("3 bancos na lista"); vazio se não há o que contar.
    situacao: &'a str,
    limpar: &'a str,
    manter: &'a str,
    aviso: &'a str,
    ha_o_que_limpar: bool,
}

/// O controle de uma limpeza: o botão que a marca e, marcada, o aviso de que acontece ao salvar com o botão que a
/// desmarca.
fn acao_de_limpar(ui: &mut Ui, marcada: &mut bool, l: &Limpeza<'_>) {
    let e = paleta(ui).estudio;
    linha(ui, 28.0, 10.0, |ui| {
        if *marcada {
            texto_em_linha(ui, l.aviso, e.t2);
            if ui.add(Botao::ghost(l.manter)).clicked() {
                *marcada = false;
            }
        } else {
            if ui.add(Botao::new(l.limpar).habilitado(l.ha_o_que_limpar)).clicked() {
                *marcada = true;
            }
            if !l.situacao.is_empty() {
                texto_em_linha(ui, l.situacao, e.t3);
            }
        }
    });
}

/// Um texto de 12px numa linha.
fn texto_em_linha(ui: &mut Ui, texto: &str, cor: Color32) -> Response {
    interface::pecas::texto::texto(ui, texto, 12.0, cor)
}

// ---------------------------------------------------------------------- as cartas das cores do código

/// As quatro cartas (`.cc`, `role=radiogroup`): duas colunas com `gap:10px`, cada uma com a amostra no realce de
/// verdade e o nome. A escolhida ganha a borda e o brilho do destaque.
fn cartas(ui: &mut Ui, escolhida: &mut CoresCodigo) {
    let topo = ui.cursor().min;
    let largura = ui.available_width();
    let coluna = ((largura - ESPACO_CARTAS) / 2.0).max(60.0);
    let linhas = CoresDoInterface::TODAS.len().div_ceil(2);
    for (i, cores) in CoresDoInterface::TODAS.into_iter().enumerate() {
        let (l, c) = ((i / 2) as f32, (i % 2) as f32);
        let rect = Rect::from_min_size(
            pos2(topo.x + c * (coluna + ESPACO_CARTAS), topo.y + l * (ALTURA_CARTA + ESPACO_CARTAS)),
            vec2(coluna, ALTURA_CARTA),
        );
        let valor = CoresCodigo::de_texto(cores.como_str()).unwrap_or_default();
        if carta(ui, rect, cores, *escolhida == valor).clicked() {
            *escolhida = valor;
        }
    }
    let altura = linhas as f32 * ALTURA_CARTA + (linhas - 1) as f32 * ESPACO_CARTAS;
    ui.allocate_rect(Rect::from_min_size(topo, vec2(largura, altura)), Sense::hover());
}

/// Uma carta (`.cc-c`): fundo `--f`, borda `--fb` (no hover, puxa para o destaque; escolhida, a borda e o anel de 1px
/// no destaque com o brilho de 14px), a amostra (`.cc-p`) e o nome em 600 com a nota em 400 `--t3` (`.cc-n small`).
fn carta(ui: &mut Ui, rect: Rect, cores: CoresDoInterface, escolhida: bool) -> Response {
    let p = paleta(ui);
    let e = &p.estudio;
    let id = ui.id().with(("pref-carta", cores.como_str()));
    let resp = ui.interact(rect, id, Sense::click());
    if ui.is_rect_visible(rect.expand(16.0)) {
        let pintor = ui.painter();
        let hover = resp.hovered();
        let borda = if escolhida {
            e.acc
        } else if hover {
            pintura::misturar(e.fb, e.acc, 0.45)
        } else {
            e.fb
        };
        if escolhida {
            pintura::halo(&pintor_de_fora(ui, rect), rect, RAIO_PADRAO, 14.0, e.brilho);
        }
        pintor.rect_filled(rect, RAIO_PADRAO, e.f);
        pintor.rect_stroke(rect, RAIO_PADRAO, Stroke::new(1.0, borda), StrokeKind::Inside);
        if escolhida {
            pintor.rect_stroke(rect, RAIO_PADRAO, Stroke::new(1.0, e.acc), StrokeKind::Outside);
        }
        if resp.has_focus() && pintura::foco_visivel(ui.ctx()) {
            pintura::anel_foco(&pintor_de_fora(ui, rect), p, rect, RAIO_PADRAO);
        }

        // a amostra: o fundo do esquema, o anel de 1px e a sombra interna de cima, e as três linhas no realce
        let resolvidas = cores.resolver(p);
        let amostra = Rect::from_min_size(
            rect.min + vec2(DENTRO_CARTA, DENTRO_CARTA),
            vec2(rect.width() - 2.0 * DENTRO_CARTA, ALTURA_AMOSTRA),
        );
        pintor.rect_filled(amostra, 6.0, resolvidas.fundo);
        pintura::sombra_interna_topo(pintor, amostra, 6.0, 2.0, rgba(0, 0, 0, 0.3));
        pintor.rect_stroke(amostra, 6.0, Stroke::new(1.0, rgba(0, 0, 0, 0.28)), StrokeKind::Inside);
        let mut job = codigo::realcar_sql(AMOSTRA, &resolvidas, FontId::monospace(11.0));
        for secao in &mut job.sections {
            secao.format.line_height = Some(16.0);
        }
        let g = ui.fonts_mut(|f| f.layout_job(job));
        pintor.with_clip_rect(amostra).galley(amostra.min + vec2(9.0, 7.0), g, resolvidas.texto);

        // o nome e a nota
        let y = amostra.bottom() + 7.0;
        let nome = galley(ui, cores.rotulo(), fontes::fonte(12.0, Peso::Medio), e.t);
        let x = rect.left() + DENTRO_CARTA + 3.0;
        let largura_nome = nome.size().x;
        pintor.galley(pos2(x, y + (ALTURA_NOME_CARTA - nome.size().y) / 2.0), nome, e.t);
        if !cores.nota().is_empty() {
            let restante = (rect.right() - DENTRO_CARTA - 3.0 - (x + largura_nome + 6.0)).max(0.0);
            let nota = interface::pecas::texto::galley_cortado(
                ui,
                cores.nota(),
                fontes::fonte(11.0, Peso::Normal),
                e.t3,
                restante,
            );
            pintor.galley(
                pos2(x + largura_nome + 6.0, y + (ALTURA_NOME_CARTA - nota.size().y) / 2.0 + 0.5),
                nota,
                e.t3,
            );
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let rotulo = cores.rotulo();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, ui.is_enabled(), escolhida, rotulo));
    resp
}

#[cfg(test)]
mod testes {
    use super::*;

    fn valores() -> Valores {
        Valores {
            tema: prefs::Tema::Sistema,
            idioma: idioma::Escolha::Sistema,
            cores: CoresCodigo::DataBox,
            familia: None,
            tamanho: TAMANHO_CODIGO_PADRAO,
            separador: ';',
            decimal: ',',
            cabecalho: true,
            proteger_formulas: true,
            indentado: true,
        }
    }

    fn janela() -> JanelaPreferencias {
        JanelaPreferencias {
            secao: Secao::Geral,
            retrato: valores(),
            atual: valores(),
            familias: vec!["Consolas".into(), "Courier New".into()],
            familias_pendentes: None,
            limpar_recentes: false,
            limpar_historico: false,
        }
    }

    #[test]
    fn so_quatro_secoes_e_tabelas_fica_de_fora() {
        let rotulos: Vec<&str> = Secao::TODAS.iter().map(|s| s.rotulo()).collect();
        assert_eq!(rotulos, ["Geral", "Aparência", "Consulta SQL", "Exportação"]);
        assert!(Secao::Aparencia.tem_padrao() && Secao::Exportacao.tem_padrao());
        assert!(!Secao::Geral.tem_padrao() && !Secao::ConsultaSql.tem_padrao());
    }

    #[test]
    fn sem_mudanca_o_salvar_fica_desligado_e_nada_vai_para_gravar() {
        let j = janela();
        assert!(!j.mudou());
        assert_eq!(j.gravacao().mudanca, Mudanca::default());
    }

    #[test]
    fn so_o_que_mudou_vai_para_o_arquivo() {
        let mut j = janela();
        j.atual.tema = prefs::Tema::Escuro;
        let g = j.gravacao();
        assert_eq!(g.mudanca.tema, Some(prefs::Tema::Escuro));
        assert!(g.mudanca.aparencia.is_none(), "a chave aparencia só entra depois de a pessoa escolher nela");
        assert!(g.mudanca.exportacao.is_none());

        let mut j = janela();
        j.atual.idioma = idioma::Escolha::Idioma(idioma::INGLES);
        let g = j.gravacao();
        assert_eq!(g.mudanca.idioma, Some(idioma::Escolha::Idioma(idioma::INGLES)));
        assert_eq!((g.mudanca.tema, g.mudanca.aparencia), (None, None), "o idioma não leva a aparência");

        let mut j = janela();
        j.atual.tamanho = 14.0;
        let g = j.gravacao();
        assert_eq!(g.mudanca.tema, None);
        assert_eq!(g.mudanca.idioma, None, "a chave idioma só entra depois da escolha");
        let aparencia = g.mudanca.aparencia.expect("aparência");
        assert_eq!(aparencia.tamanho_codigo, Some(14.0));
        assert_eq!(aparencia.fonte_codigo, Some(None), "a família padrão grava como None");

        let mut j = janela();
        j.atual.separador = ',';
        j.atual.indentado = false;
        let exportacao = j.gravacao().mudanca.exportacao.expect("exportação");
        assert_eq!(exportacao.csv_separador, Some(','));
        assert_eq!(exportacao.json_indentado, Some(false));
        assert_eq!(
            (exportacao.csv_decimal, exportacao.cabecalho, exportacao.csv_proteger_formulas),
            (None, None, None)
        );
    }

    #[test]
    fn limpezas_ligam_o_salvar_e_vao_na_gravacao() {
        let mut j = janela();
        j.limpar_historico = true;
        assert!(j.mudou());
        let g = j.gravacao();
        assert!(g.limpar_historico && !g.limpar_recentes);
        assert_eq!(g.mudanca, Mudanca::default(), "limpar não mexe no resto das preferências");
    }

    #[test]
    fn diferencas_na_ordem_e_cancelar_volta_ao_retrato() {
        let mut j = janela();
        j.atual.tema = prefs::Tema::Claro;
        j.atual.idioma = idioma::Escolha::Idioma(idioma::PORTUGUES);
        j.atual.cores = CoresCodigo::Monokai;
        j.atual.familia = Some("Courier New".into());
        j.atual.tamanho = 16.0;
        assert_eq!(
            diferencas(&j.retrato, &j.atual),
            [
                Efeito::Tema(prefs::Tema::Claro),
                Efeito::Idioma(idioma::Escolha::Idioma(idioma::PORTUGUES)),
                Efeito::Cores(CoresCodigo::Monokai),
                Efeito::Fonte(Some("Courier New".into())),
                Efeito::Tamanho(16.0)
            ]
        );
        assert_eq!(
            diferencas(&j.atual, &j.retrato),
            [
                Efeito::Tema(prefs::Tema::Sistema),
                Efeito::Idioma(idioma::Escolha::Sistema),
                Efeito::Cores(CoresCodigo::DataBox),
                Efeito::Fonte(None),
                Efeito::Tamanho(12.5)
            ]
        );
        assert!(diferencas(&j.retrato, &j.retrato).is_empty());
    }

    #[test]
    fn restaurar_so_mexe_na_secao() {
        let mut v = valores();
        v.tema = prefs::Tema::Escuro;
        v.idioma = idioma::Escolha::Idioma(idioma::INGLES);
        v.tamanho = 16.0;
        v.separador = '\t';
        v.cabecalho = false;
        v.restaurar_aparencia();
        assert_eq!((v.tema, v.idioma, v.tamanho), (prefs::Tema::Sistema, idioma::Escolha::Sistema, 12.5));
        assert_eq!((v.separador, v.cabecalho), ('\t', false), "a exportação ficou");
        v.restaurar_exportacao();
        assert_eq!(v, valores());
    }

    #[test]
    fn idiomas_seguir_o_sistema_e_os_nomes_no_proprio_idioma() {
        let rotulos = |sistema| -> Vec<String> { rotulos_dos_idiomas(sistema).into_iter().map(|(_, r)| r).collect() };
        idioma::com_idioma(idioma::PORTUGUES, || {
            assert_eq!(rotulos(idioma::PORTUGUES), ["Seguir o sistema (Português)", "Português (Brasil)", "English"]);
            assert_eq!(rotulos(idioma::INGLES)[0], "Seguir o sistema (English)");
        });
        idioma::com_idioma(idioma::INGLES, || {
            assert_eq!(rotulos(idioma::PORTUGUES), ["Follow the system (Português)", "Português (Brasil)", "English"]);
        });
        let escolhas: Vec<idioma::Escolha> =
            rotulos_dos_idiomas(idioma::PORTUGUES).into_iter().map(|(e, _)| e).collect();
        assert_eq!(escolhas, idioma::Escolha::todas());
    }

    #[test]
    fn consolas_e_a_familia_padrao_e_vira_none() {
        assert_eq!(normalizar_familia(Some("Consolas".into())), None);
        assert_eq!(normalizar_familia(Some("Cascadia Mono".into())), Some("Cascadia Mono".into()));
        assert_eq!(normalizar_familia(None), None);
    }

    #[test]
    fn fonte_recusada_volta_a_escolha() {
        let mut j = janela();
        j.atual.familia = Some("Courier New".into());
        j.fonte_recusada(None);
        assert_eq!(j.atual.familia, None);
    }

    #[test]
    fn tamanhos_com_virgula_e_o_de_fora_da_lista_entra() {
        assert_eq!(rotulo_do_tamanho(12.0), "12 px");
        assert_eq!(rotulo_do_tamanho(12.5), "12,5 px");
        let comuns: Vec<String> = rotulos_dos_tamanhos(12.5).into_iter().map(|(_, r)| r).collect();
        assert_eq!(comuns, ["11 px", "12 px", "12,5 px", "13 px", "14 px", "16 px"]);
        let com_extra: Vec<f32> = rotulos_dos_tamanhos(15.0).into_iter().map(|(v, _)| v).collect();
        assert_eq!(com_extra, [11.0, 12.0, 12.5, 13.0, 14.0, 15.0, 16.0]);
    }

    fn recentes_de(tipos: &[prefs::TipoRecente]) -> Vec<prefs::Recente> {
        tipos
            .iter()
            .enumerate()
            .map(|(i, &tipo)| prefs::Recente {
                caminho: format!("C:\\dados\\item{i}"),
                nome: format!("item{i}"),
                pasta: "C:\\dados".to_string(),
                tipo,
                existe: true,
                tamanho: None,
                modificado_em: None,
                aberto_em: None,
            })
            .collect()
    }

    #[test]
    fn limpar_a_lista_conta_itens() {
        use prefs::TipoRecente::{Arquivo, Banco, Pasta};
        assert_eq!(contagem_de_recentes(&[]), "Nenhum item na lista");
        // só bancos: a frase de antes
        assert_eq!(contagem_de_recentes(&recentes_de(&[Banco])), "1 banco na lista");
        assert_eq!(contagem_de_recentes(&recentes_de(&[Banco, Banco, Banco])), "3 bancos na lista");
        assert_eq!(contagem_de_recentes(&recentes_de(&[Pasta])), "1 pasta na lista");
        assert_eq!(contagem_de_recentes(&recentes_de(&[Arquivo, Arquivo])), "2 arquivos na lista");
        // mais de um tipo: o total e cada tipo que aparece, na ordem banco, pasta, arquivo
        assert_eq!(contagem_de_recentes(&recentes_de(&[Pasta, Banco])), "2 itens na lista: 1 banco e 1 pasta");
        assert_eq!(
            contagem_de_recentes(&recentes_de(&[Banco, Arquivo, Pasta, Banco, Banco])),
            "5 itens na lista: 3 bancos, 1 pasta e 1 arquivo"
        );
        assert_eq!(
            contagem_de_recentes(&recentes_de(&[Arquivo, Banco, Arquivo])),
            "3 itens na lista: 1 banco e 2 arquivos"
        );
    }
}
