//! Janela "Nova tabela": o nome, uma linha por coluna (nome, tipo
//! com a pastilha, padrão, chave primária, não aceita nulo, único e o × que tira a coluna) numa grade com os rótulos
//! em cima de cada campo, "+ Coluna", "Sem rowid" e a prévia do CREATE TABLE. Ao criar, recarrega os objetos e vai para
//! a estrutura da tabela nova.

use ddl::ColunaDef;
use egui::{Align, Id, Layout, Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use estado::{Aba, Acao, Contexto, Rota};
use interface::icones::{Icone, TipoColuna};
use interface::pecas::arranjo::linha;
use interface::pecas::botao::Botao;
use interface::pecas::campos::{Check, Entrada, EntradaSugestoes, campo, campos_duas_colunas};
use interface::pecas::janela::Janela;
use interface::pecas::texto::{galley_tit, h4};
use interface::tema::paleta;
use servico::OperacaoEsquema;

use crate::apoio::TIPOS;
use crate::estrutura::botao_cancelar;
use crate::mudanca::{Montagem, Mudanca};

/// Tipo com que nasce cada linha nova de coluna; sozinho, não conta como linha preenchida.
const TIPO_NOVA_LINHA: &str = "TEXT";

/// Uma linha de coluna da janela.
#[derive(Debug, Clone)]
pub(crate) struct LinhaColuna {
    id: u64,
    pub(crate) nome: String,
    pub(crate) tipo: String,
    pub(crate) padrao: String,
    pub(crate) pk: bool,
    pub(crate) nn: bool,
    pub(crate) un: bool,
}

impl LinhaColuna {
    /// Linha acrescentada e não usada: sem nome, sem padrão, sem regra e com o tipo que ela já traz (o
    /// `emBranco`).
    fn em_branco(&self) -> bool {
        let tipo = self.tipo.trim().to_uppercase();
        self.nome.trim().is_empty()
            && self.padrao.trim().is_empty()
            && (tipo.is_empty() || tipo == TIPO_NOVA_LINHA)
            && !self.pk
            && !self.nn
            && !self.un
    }
}

/// Os campos da janela.
pub(crate) struct CamposNovaTabela {
    id: Id,
    pub(crate) nome: String,
    pub(crate) sem_rowid: bool,
    pub(crate) linhas: Vec<LinhaColuna>,
    proximo: u64,
    /// A linha cujo nome recebe o foco no próximo quadro.
    focar: Option<u64>,
}

/// A janela "Nova tabela".
pub struct JanelaNovaTabela {
    pub(crate) campos: CamposNovaTabela,
    pub(crate) mudanca: Mudanca,
}

impl Default for JanelaNovaTabela {
    fn default() -> Self {
        JanelaNovaTabela::nova()
    }
}

impl JanelaNovaTabela {
    /// A janela nasce com uma coluna `id INTEGER` chave primária.
    pub fn nova() -> JanelaNovaTabela {
        let id = Id::new("tela_esquema::nova_tabela");
        let mut campos =
            CamposNovaTabela { id, nome: String::new(), sem_rowid: false, linhas: Vec::new(), proximo: 0, focar: None };
        campos.adicionar_linha("id", "INTEGER", true);
        JanelaNovaTabela { campos, mudanca: Mudanca::nova(id, "Digite o nome da tabela para ver o SQL.") }
    }

    /// Desenha a janela; false quando ela fechou (pelo ✕, Esc, clique no fundo, Cancelar ou depois de criar a tabela,
    /// quando também pede `Acao::RecarregarObjetos` e vai para a estrutura da tabela nova).
    pub fn mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto) -> bool {
        let JanelaNovaTabela { campos, mudanca } = self;
        if mudanca.passo(egui, ctx, || campos.montar()).is_some() {
            ctx.emitir(Acao::RecarregarObjetos);
            ctx.navegar(Rota::objeto_na_aba(campos.nome.trim(), false, Aba::Estrutura));
            return false;
        }
        let ocupado = mudanca.ocupada();
        let mut aplicar = false;
        let mut cancelar = false;
        // a ficha da tabela; com "Sem rowid" marcado, a chave anil da tabela sem rowid (D6)
        let icone = if campos.sem_rowid { Icone::TabelaSemRowid } else { Icone::Tabela };
        let r = Janela::new(("tela_esquema::janela_nova_tabela", campos.id), "Nova tabela")
            .larga(true)
            .icone(icone)
            .rodape_direita(|ui| {
                cancelar = botao_cancelar(ui, ocupado);
                if ui.add(Botao::primario("Criar tabela").habilitado(!ocupado)).clicked() {
                    aplicar = true;
                }
            })
            .mostrar(egui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                mudanca.desenhar_aviso(ui);
                let mudou = campos.desenhar(ui);
                mudanca.desenhar_previa(ui);
                mudou
            });
        if r.inner {
            mudanca.agendar(egui);
        }
        if (aplicar || r.enviou) && !ocupado {
            mudanca.aplicar(egui, ctx, campos.montar());
        }
        !((r.pediu_fechar || cancelar) && !ocupado)
    }
}

/// O `id_salt` do campo Nome de uma linha (para o foco depois de "+ Coluna" e de "×").
fn id_nome(janela: Id, linha: u64) -> Id {
    janela.with(("nome", linha))
}

/// A largura máxima do campo "Nome da tabela" (`max-width:340px`).
const LARGURA_NOME_TABELA: f32 = 340.0;
/// A altura dos campos da grade (`.campo.pq`) e das linhas.
const ALTURA_LINHA: f32 = interface::pecas::campos::ALTURA_CAMPO_PEQUENO;
/// O espaço entre as células da grade (`gap:8px`).
const ESPACO: f32 = 8.0;
/// A linha de um rótulo `.h` (11px × 1,3) e o cabeçalho de duas linhas.
const LINHA_ROTULO: f32 = 14.3;
const ALTURA_CABECALHO: f32 = 2.0 * LINHA_ROTULO;
/// As colunas de largura fixa da grade: Chave primária, Não aceita nulo, Único e o ×.
const FIXAS: [f32; 4] = [84.0, 80.0, 52.0, 28.0];
/// As colunas do Nome, do Tipo e do Padrão dividem o resto na razão `.95fr 1.15fr 1.4fr`.
const FRACOES: [f32; 3] = [0.95, 1.15, 1.4];

/// As colunas da grade (`minmax(0,.95fr) minmax(0,1.15fr) minmax(0,1.4fr) 84px 80px 52px 28px`, `gap:8px`).
struct Grade {
    xs: [f32; 7],
    larguras: [f32; 7],
}

impl Grade {
    fn medir(esquerda: f32, largura: f32) -> Grade {
        let resto = (largura - FIXAS.iter().sum::<f32>() - 6.0 * ESPACO).max(0.0);
        let fr = resto / FRACOES.iter().sum::<f32>();
        let larguras = [fr * FRACOES[0], fr * FRACOES[1], fr * FRACOES[2], FIXAS[0], FIXAS[1], FIXAS[2], FIXAS[3]];
        let mut xs = [0.0; 7];
        let mut x = esquerda;
        for (i, w) in larguras.iter().enumerate() {
            xs[i] = x;
            x += w + ESPACO;
        }
        Grade { xs, larguras }
    }

    fn celula(&self, i: usize, y: f32, altura: f32) -> Rect {
        Rect::from_min_size(pos2(self.xs[i], y), vec2(self.larguras[i], altura))
    }
}

impl CamposNovaTabela {
    fn adicionar_linha(&mut self, nome: &str, tipo: &str, pk: bool) -> u64 {
        let id = self.proximo;
        self.proximo += 1;
        self.linhas.push(LinhaColuna {
            id,
            nome: nome.to_string(),
            tipo: tipo.to_string(),
            padrao: String::new(),
            pk,
            nn: false,
            un: false,
        });
        id
    }

    /// O pedido (o `montar()` da Nova tabela).
    pub(crate) fn montar(&self) -> Montagem {
        let n = self.nome.trim();
        if n.is_empty() {
            return Montagem::Falta;
        }
        // uma linha com algo preenchido e sem nome não some em silêncio: a frase diz qual é
        if let Some(i) = self.linhas.iter().position(|l| !l.em_branco() && l.nome.trim().is_empty()) {
            return Montagem::Erro(format!("A coluna {} precisa de um nome (ou tire a linha com ✕).", i + 1));
        }
        let colunas: Vec<ColunaDef> = self
            .linhas
            .iter()
            .filter(|l| !l.em_branco())
            .map(|l| ColunaDef {
                nome: l.nome.trim().to_string(),
                tipo: l.tipo.trim().to_string(),
                pk: l.pk,
                nao_nulo: l.nn,
                unica: l.un,
                autoincremento: false,
                padrao: Some(l.padrao.trim()).filter(|p| !p.is_empty()).map(str::to_string),
                ..ColunaDef::default()
            })
            .collect();
        if colunas.is_empty() {
            return Montagem::Erro("A tabela precisa de ao menos uma coluna.".to_string());
        }
        Montagem::pedido(OperacaoEsquema::CriarTabela {
            nome: n.to_string(),
            colunas,
            restricoes: Vec::new(),
            sem_rowid: self.sem_rowid,
        })
    }

    /// Desenha os campos; devolve se algo mudou.
    fn desenhar(&mut self, ui: &mut Ui) -> bool {
        let mut mudou = false;
        let mut mais_coluna = false;
        let mut remover: Option<usize> = None;
        let id = self.id;
        {
            let CamposNovaTabela { nome, sem_rowid, linhas, focar, .. } = self;
            campos_duas_colunas(ui, |c| {
                c.largo(|ui| {
                    campo(ui, "Nome da tabela", None, |ui| {
                        let largura = ui.available_width().min(LARGURA_NOME_TABELA);
                        mudou |= ui
                            .add(
                                Entrada::new(nome)
                                    .dica("nome_da_tabela")
                                    .rotulo("Nome da tabela")
                                    .id_salt(id.with("nome"))
                                    .largura(largura),
                            )
                            .changed();
                    });
                });
                c.largo(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // 16px depois do campo do nome (`gap:16px`: os 14px da linha de campos e mais 2)
                    ui.add_space(2.0);
                    // `.blk{gap:8px}`: o título "Colunas" e a grade
                    h4(ui, "Colunas");
                    ui.add_space(ESPACO);
                    let (m, r) = grade_colunas(ui, id, linhas, focar);
                    mudou |= m;
                    remover = r;
                    // `margin-top:6px` depois dos 8px do `.blk`
                    ui.add_space(ESPACO + 6.0);
                    // "+ Coluna" (`btn pq`) e a caixa "Sem rowid", `gap:16px`
                    linha(ui, 24.0, 16.0, |ui| {
                        if ui.add(Botao::new("+ Coluna").pequeno()).clicked() {
                            mais_coluna = true;
                        }
                        ui.spacing_mut().item_spacing.x = 8.0;
                        // a explicação vai na dica (a prancheta 12c não tem o "i")
                        let r = Check::new(sem_rowid, "Sem rowid (WITHOUT ROWID)").mostrar(ui);
                        mudou |= r.changed();
                        r.on_hover_text(
                            "A tabela passa a ser guardada pela chave primária, sem o número interno de linha (rowid). \
                             Precisa de chave primária e costuma valer só para tabelas com chave de texto ou composta.",
                        );
                    });
                });
            });
        }
        if mais_coluna {
            let nova = self.adicionar_linha("", TIPO_NOVA_LINHA, false);
            self.focar = Some(nova);
            mudou = true;
        }
        if let Some(i) = remover
            && self.linhas.len() > 1
            && i < self.linhas.len()
        {
            self.linhas.remove(i);
            let alvo = i.min(self.linhas.len() - 1);
            self.focar = self.linhas.get(alvo).map(|l| l.id);
            mudou = true;
        }
        mudou
    }
}

/// Um rótulo `.h` da grade (11px / 600, caixa alta, `--t3`, linhas de 1,3): em uma ou duas linhas, na altura do
/// cabeçalho e, nos três últimos, centrado sobre a coluna.
fn rotulo_da_grade(ui: &mut Ui, id: Id, celula: Rect, linhas: &[&str], centrado: bool) {
    let cor = paleta(ui).estudio.t3;
    let mut filho = ui.new_child(UiBuilder::new().id_salt(id).max_rect(celula).layout(Layout::top_down(Align::Min)));
    let galeys: Vec<_> = linhas.iter().map(|l| galley_tit(&filho, l, cor)).collect();
    let largura = galeys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max);
    let bloco = linhas.len() as f32 * LINHA_ROTULO;
    let topo = celula.top() + (celula.height() - bloco) / 2.0;
    let x = if centrado { celula.center().x - largura / 2.0 } else { celula.left() };
    let area = Rect::from_min_size(pos2(x, topo), vec2(largura, bloco));
    let resp = filho.allocate_rect(area, Sense::hover());
    let texto = linhas.join(" ");
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &texto));
    for (k, g) in galeys.into_iter().enumerate() {
        let y = area.top() + k as f32 * LINHA_ROTULO + (LINHA_ROTULO - g.size().y) / 2.0;
        let x = if centrado { celula.center().x - g.size().x / 2.0 } else { x };
        filho.painter().galley(pos2(x, y), g, cor);
    }
}

/// A grade das colunas: o cabeçalho de rótulos e uma linha por coluna. Devolve (algo mudou, linha a tirar).
fn grade_colunas(ui: &mut Ui, id: Id, linhas: &mut [LinhaColuna], focar: &mut Option<u64>) -> (bool, Option<usize>) {
    let largura = ui.available_width();
    let topo = ui.cursor().min;
    let grade = Grade::medir(topo.x, largura);
    let mut mudou = false;
    let mut remover = None;

    let rotulos: [(&[&str], bool); 6] = [
        (&["Nome"], false),
        (&["Tipo"], false),
        (&["Padrão"], false),
        (&["Chave", "primária"], true),
        (&["Não aceita", "nulo"], true),
        (&["Único"], true),
    ];
    for (i, (linhas_do_rotulo, centrado)) in rotulos.iter().enumerate() {
        let celula = grade.celula(i, topo.y, ALTURA_CABECALHO);
        rotulo_da_grade(ui, id.with(("cab", i)), celula, linhas_do_rotulo, *centrado);
    }
    let mut y = topo.y + ALTURA_CABECALHO;
    let uma_so = linhas.len() == 1;
    for (k, l) in linhas.iter_mut().enumerate() {
        y += ESPACO;
        let n = k + 1;
        let id_linha = id.with(("linha", l.id));
        let mut em = |i: usize, desenhar: &mut dyn FnMut(&mut Ui)| {
            let rect = grade.celula(i, y, ALTURA_LINHA);
            let mut filho = ui.new_child(
                UiBuilder::new().id_salt(id_linha.with(i)).max_rect(rect).layout(Layout::left_to_right(Align::Center)),
            );
            desenhar(&mut filho);
        };
        let rotulo_nome = format!("Nome da coluna {n}");
        let rotulo_tipo = format!("Tipo da coluna {n}");
        let rotulo_padrao = format!("Valor padrão da coluna {n}");
        // o leitor de tela ouve o nome digitado ou, sem nome, o número da coluna
        let nome_da_linha = l.nome.trim();
        let (quem_marca, quem_tira) = if nome_da_linha.is_empty() {
            (format!("coluna {n}"), n.to_string())
        } else {
            (nome_da_linha.to_string(), nome_da_linha.to_string())
        };
        em(0, &mut |ui| {
            let r = ui.add(Entrada::new(&mut l.nome).pequeno().rotulo(&rotulo_nome).id_salt(id_nome(id, l.id)));
            mudou |= r.changed();
            // o foco vai para o nome da linha nova (ou da vizinha da que saiu) quando ela já está na tela
            if *focar == Some(l.id) {
                r.request_focus();
                *focar = None;
            }
        });
        em(1, &mut |ui| {
            // a pastilha de 16px acompanha o tipo digitado (a decisão 17, só pelo texto do tipo)
            let pastilha = TipoColuna::do_declarado(&l.tipo, false, false);
            mudou |= EntradaSugestoes::new(id_linha.with("tipo"), &mut l.tipo, &TIPOS)
                .pastilha(pastilha)
                .pequeno()
                .monoespacada(true)
                .rotulo(&rotulo_tipo)
                .mostrar(ui)
                .changed();
        });
        em(2, &mut |ui| {
            mudou |= ui
                .add(Entrada::new(&mut l.padrao).pequeno().monoespacada(true).dica("padrão").rotulo(&rotulo_padrao))
                .changed();
        });
        // as três caixas, sem texto, centradas na coluna (`.ck`); o leitor de tela ouve "id: chave primária"
        let caixas: [(usize, &str); 3] = [(3, "chave primária"), (4, "não aceita nulo"), (5, "único")];
        for (coluna, regra) in caixas {
            em(coluna, &mut |ui| {
                let marcado = match coluna {
                    3 => &mut l.pk,
                    4 => &mut l.nn,
                    _ => &mut l.un,
                };
                let rotulo = format!("{quem_marca}: {regra}");
                ui.add_space(((ui.available_width() - 16.0) / 2.0).max(0.0));
                let r = Check::new(marcado, "").mostrar(ui);
                let estado = *marcado;
                r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, estado, &rotulo));
                mudou |= r.changed();
            });
        }
        if !uma_so {
            em(6, &mut |ui| {
                let rotulo = format!("Tirar a coluna {quem_tira}");
                if ui.add(Botao::icone(Icone::Fechar).pequeno().rotulo(&rotulo).dica(&rotulo)).clicked() {
                    remover = Some(k);
                }
            });
        }
        y += ALTURA_LINHA;
    }
    ui.allocate_rect(Rect::from_min_max(topo, pos2(topo.x + largura, y)), Sense::hover());
    (mudou, remover)
}
