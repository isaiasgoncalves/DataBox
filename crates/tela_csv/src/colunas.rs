//! As colunas do CSV (prancheta 16): a seleção pelo cabeçalho (Ctrl e Shift
//! somam), o menu do cabeçalho (clique direito e Shift+F10), a coluna provisória com o cabeçalho em edição (Nova
//! coluna e a cola: decisão 14), renomear (F2) e excluir, copiar colunas em TSV com o cabeçalho e colar por posição
//! (como nova coluna ou sobre uma coluna), com o aviso das quantidades diferentes e o "Substituir N valores".

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Event, Id, Key, Modifiers, Rect, Response, Sense, Ui, ViewportCommand};
use estado::Promessa;
use interface::formato::{formatar_numero, plural};
use interface::icones::Icone;
use interface::pecas::avisos::{AcaoAviso, TomAviso, aviso_com_acoes, aviso_tom};
use interface::pecas::menu_contexto::MenuContexto;
use interface::pecas::texto::Trecho;
use interface::pecas::toast::{toast, toast_atencao, toast_concluido};
use tabular::colunas::{
    ColunaColada, OrigemDoTexto, QuandoSobram, Quantidades, colunas_coladas, comparar_quantidades, escrever_tsv,
    ler_tsv, nome_generico, posicionar,
};

use crate::edicao::Mutacao;
use crate::sessao::{self, LIMITE_DA_COPIA};
use crate::tela::TelaCsv;
use crate::textos;

/// A dica dos dois Colar quando a ordem da tela não é a do arquivo (prancheta 16 b).
pub const DICA_COLAR_SEM_POSICAO: &str =
    "Tire o filtro e a ordenação para colar por posição, ou use Colar com correspondência.";
/// O espaço do aviso compacto da cola até a grade e até o alto do conteúdo.
const RECUO_AVISO_COLA: f32 = 12.0;

/// Uma coluna da grade: uma da tabela ou a provisória (a Nova coluna e a coluna colada antes de confirmar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColunaDaGrade {
    Dados(usize),
    Provisoria,
}

/// O que o cabeçalho em edição vai virar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AlvoCabecalho {
    /// A Nova coluna (Esc desiste: nada entra no desfazer).
    Nova { posicao: usize },
    /// A coluna colada (Esc mantém o nome sugerido): os valores já na medida das linhas.
    Colar { posicao: usize, valores: Arc<Vec<String>>, acrescentar: u64, vazias: usize },
    /// Renomear uma coluna que existe (Esc desiste).
    Renomear { coluna: usize },
}

/// O cabeçalho em edição (o `th-edit` da prancheta 16: o nome selecionado, raio 4).
#[derive(Debug, Clone)]
pub(crate) struct CabecalhoEmEdicao {
    pub alvo: AlvoCabecalho,
    pub nome: String,
    /// O nome sugerido (o que fica com o Esc da cola, ou com o nome vazio).
    pub sugerido: String,
    pub pedir_foco: bool,
    pub focada: bool,
}

impl CabecalhoEmEdicao {
    pub(crate) fn id(&self) -> Id {
        Id::new("tela_csv::th_edicao")
    }

    /// A posição da coluna provisória (a Nova e a colada), se há.
    pub(crate) fn provisoria(&self) -> Option<usize> {
        match &self.alvo {
            AlvoCabecalho::Nova { posicao } | AlvoCabecalho::Colar { posicao, .. } => Some(*posicao),
            AlvoCabecalho::Renomear { .. } => None,
        }
    }
}

/// Como terminou a edição do cabeçalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FimCabecalho {
    /// Enter, ou o clique fora (o campo perdeu o foco).
    Confirmar,
    Esc,
}

/// Onde uma cola vai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PedidoCola {
    /// Como nova coluna, nesta posição.
    Nova { posicao: usize },
    /// Sobre a coluna.
    Sobre { coluna: usize },
    /// Com correspondência: o Trazer colunas com a área de transferência como origem.
    Correspondencia,
}

/// O aviso compacto de topo da cola (prancheta 16 c).
#[derive(Debug, Clone)]
pub(crate) enum AvisoCola {
    /// "A coluna colada tem 1.204 valores e o arquivo, 1.180 linhas." Colar 1.180 · Acrescentar 24 linhas · Cancelar.
    Quantidades { pedido: PedidoCola, colunas: Vec<ColunaColada>, valores: usize, linhas: usize },
    /// "Substituir 1.180 valores de quantidade?" Substituir · Cancelar.
    Substituir { coluna: usize, valores: Arc<Vec<String>>, acrescentar: u64, n: usize },
}

/// A cola sobre uma coluna esperando a leitura dela (para saber se há valores no caminho).
pub(crate) struct LeituraSobre {
    pub coluna: usize,
    pub valores: Arc<Vec<String>>,
    pub acrescentar: u64,
    pub promessa: Promessa<usize>,
}

/// O que o menu do cabeçalho pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcaoMenu {
    Copiar,
    ColarNova,
    ColarSobre,
    Renomear,
    NovaDepois,
    Excluir,
    Correspondencia,
    Trazer,
}

/// O último texto que o DataBox copiou (em qualquer aba): quando ele volta numa cola, a primeira linha é o cabeçalho.
fn id_ultimo_copiado() -> Id {
    Id::new("tela_csv::ultimo_copiado")
}

fn normalizar_texto(t: &str) -> String {
    t.replace("\r\n", "\n").trim_end_matches(['\n', '\r']).to_string()
}

/// Guarda o texto que o DataBox acabou de copiar.
pub(crate) fn lembrar_copia(ctx: &egui::Context, texto: &str) {
    let normal = Arc::new(normalizar_texto(texto));
    ctx.data_mut(|d| d.insert_temp(id_ultimo_copiado(), normal));
}

pub(crate) fn origem_do_texto(ctx: &egui::Context, texto: &str) -> OrigemDoTexto {
    let ultimo: Option<Arc<String>> = ctx.data(|d| d.get_temp(id_ultimo_copiado()));
    if ultimo.is_some_and(|u| *u == normalizar_texto(texto)) { OrigemDoTexto::Databox } else { OrigemDoTexto::Externo }
}

/// "Copiar coluna" / "Copiar 3 colunas".
pub(crate) fn rotulo_copiar(n: usize) -> String {
    if n > 1 { format!("Copiar {n} colunas") } else { "Copiar coluna".to_string() }
}

/// "Excluir a coluna" / "Excluir 3 colunas".
fn rotulo_excluir(n: usize) -> String {
    if n > 1 { format!("Excluir {n} colunas") } else { "Excluir a coluna".to_string() }
}

impl TelaCsv {
    /// A cola por posição vale: com filtro ou ordenação, a ordem da tela não é a do arquivo.
    pub fn por_posicao_permitida(&self) -> bool {
        !self.filtrando()
    }

    /// As colunas da grade, na ordem (com a provisória no lugar dela).
    pub(crate) fn montar_mapa(&self) -> Vec<ColunaDaGrade> {
        let n = self.retrato.as_ref().map_or(0, |r| r.colunas());
        let mut mapa: Vec<ColunaDaGrade> = (0..n).map(ColunaDaGrade::Dados).collect();
        if let Some(p) = self.cabecalho_edicao.as_ref().and_then(CabecalhoEmEdicao::provisoria) {
            mapa.insert(p.min(n), ColunaDaGrade::Provisoria);
        }
        mapa
    }

    /// A coluna da tabela que as ações de coluna alcançam sem menu: a selecionada (a primeira) ou a da célula ativa.
    pub(crate) fn coluna_em_foco(&self) -> Option<usize> {
        self.colunas_selecionadas.iter().next().copied().or_else(|| self.celula_ativa().map(|(_, c)| c))
    }

    /// As colunas que o Copiar e o Excluir alcançam a partir de `coluna`: a seleção, se a coluna está nela.
    fn alvo_das_colunas(&self, coluna: usize) -> Vec<usize> {
        if self.colunas_selecionadas.contains(&coluna) {
            self.colunas_selecionadas.iter().copied().collect()
        } else {
            vec![coluna]
        }
    }

    // ------------------------------------------------------------------ seleção

    /// O clique no `th` de uma coluna: seleciona (Ctrl soma ou tira, Shift estende desde a âncora).
    pub(crate) fn clicar_cabecalho(&mut self, coluna: usize, ctrl: bool, shift: bool) {
        match (ctrl, shift, self.ancora_coluna) {
            (_, true, Some(a)) => {
                let (de, ate) = (a.min(coluna), a.max(coluna));
                if !ctrl {
                    self.colunas_selecionadas.clear();
                }
                self.colunas_selecionadas.extend(de..=ate);
            }
            (true, _, _) => {
                if !self.colunas_selecionadas.remove(&coluna) {
                    self.colunas_selecionadas.insert(coluna);
                }
                self.ancora_coluna = Some(coluna);
            }
            _ => {
                self.colunas_selecionadas = BTreeSet::from([coluna]);
                self.ancora_coluna = Some(coluna);
            }
        }
        self.focar_grade = true;
    }

    /// As colunas selecionadas (da tabela).
    pub fn colunas_selecionadas(&self) -> Vec<usize> {
        self.colunas_selecionadas.iter().copied().collect()
    }

    /// Seleciona exatamente estas colunas (da tabela).
    pub fn selecionar_colunas(&mut self, colunas: impl IntoIterator<Item = usize>) {
        self.colunas_selecionadas = colunas.into_iter().collect();
        self.ancora_coluna = self.colunas_selecionadas.iter().next().copied();
    }

    // ------------------------------------------------------------------ o menu do cabeçalho

    /// O menu do cabeçalho de `coluna`, com o `dono` (o `th` dela, ou um vazio se ele saiu da vista).
    pub(crate) fn menu_do_cabecalho(&mut self, ectx: &egui::Context, dono: &Response, coluna: usize) {
        let nome = self.retrato.as_ref().and_then(|r| r.nomes.get(coluna)).cloned().unwrap_or_default();
        let n = self.alvo_das_colunas(coluna).len();
        let livre = self.livre_para_editar();
        let por_posicao = self.por_posicao_permitida();
        let renomear = self.renomear_permitido();
        let dica_renomear = textos::DICA_SEM_CABECALHO;
        let acao = MenuContexto::new(("tela_csv::menu", &self.id)).rotulo(&nome).mostrar(dono, |m| {
            let mut acao = None;
            if m.item(&rotulo_copiar(n), Some("Ctrl+C"), false).clicked() {
                acao = Some(AcaoMenu::Copiar);
            }
            if por_posicao && livre {
                if m.item("Colar como nova coluna", Some("Ctrl+V"), false).clicked() {
                    acao = Some(AcaoMenu::ColarNova);
                }
                if m.item("Colar sobre esta coluna", None, false).clicked() {
                    acao = Some(AcaoMenu::ColarSobre);
                }
            } else {
                let dica = if por_posicao { "" } else { DICA_COLAR_SEM_POSICAO };
                let r = m.item_desligado("Colar como nova coluna", Some("Ctrl+V"));
                if !dica.is_empty() {
                    r.on_hover_text(dica);
                }
                let r = m.item_desligado("Colar sobre esta coluna", None);
                if !dica.is_empty() {
                    r.on_hover_text(dica);
                }
            }
            if livre {
                if m.item("Colar com correspondência", Some("Ctrl+Shift+V"), false).clicked() {
                    acao = Some(AcaoMenu::Correspondencia);
                }
                if m.silhueta(Icone::TrazerColunas).item("Trazer colunas de outro arquivo", None, false).clicked() {
                    acao = Some(AcaoMenu::Trazer);
                }
            } else {
                m.item_desligado("Colar com correspondência", Some("Ctrl+Shift+V"));
                m.silhueta(Icone::TrazerColunas).item_desligado("Trazer colunas de outro arquivo", None);
            }
            if livre {
                if !renomear {
                    // sem cabeçalho, o nome não seria gravado
                    m.separador();
                    m.silhueta(Icone::Editar)
                        .item_desligado("Renomear a coluna", Some("F2"))
                        .on_hover_text(dica_renomear);
                } else if m.silhueta(Icone::Editar).item("Renomear a coluna", Some("F2"), true).clicked() {
                    acao = Some(AcaoMenu::Renomear);
                }
                if m.silhueta(Icone::NovaColuna).item("Nova coluna depois desta", None, false).clicked() {
                    acao = Some(AcaoMenu::NovaDepois);
                }
                if m.silhueta(Icone::Excluir).item_perigo(&rotulo_excluir(n), None, true).clicked() {
                    acao = Some(AcaoMenu::Excluir);
                }
            }
            acao
        });
        match acao.flatten() {
            Some(AcaoMenu::Copiar) => self.copiar_colunas(ectx, self.alvo_das_colunas(coluna)),
            Some(AcaoMenu::ColarNova) => self.pedir_cola(ectx, PedidoCola::Nova { posicao: coluna + 1 }),
            Some(AcaoMenu::ColarSobre) => self.pedir_cola(ectx, PedidoCola::Sobre { coluna }),
            Some(AcaoMenu::Renomear) => self.renomear(coluna),
            Some(AcaoMenu::NovaDepois) => self.nova_coluna(coluna + 1),
            Some(AcaoMenu::Excluir) => self.excluir_colunas(self.alvo_das_colunas(coluna)),
            Some(AcaoMenu::Correspondencia) => self.pedir_cola(ectx, PedidoCola::Correspondencia),
            Some(AcaoMenu::Trazer) => self.trazer_pedido = Some(None),
            None => {}
        }
    }

    /// A tela aceita edição agora: sem gravar, sem cabeçalho em edição e sem trabalho que mexe nas posições esperando a
    /// resposta (`posicoes_pendentes`: a página e as marcas em mãos ainda são de antes dele). A célula já aberta grava
    /// mesmo assim (`terminar_edicao` não pergunta: a posição dela é anterior a tudo o que está na fila).
    pub(crate) fn livre_para_editar(&self) -> bool {
        self.livre_para_salvar() && !self.posicoes_pendentes()
    }

    /// Sem gravar e sem cabeçalho em edição: o Salvar, o Desfazer, o Refazer e o Atualizar, que não levam posições,
    /// podem entrar na fila atrás de uma alteração.
    pub(crate) fn livre_para_salvar(&self) -> bool {
        !self.salvando_agora() && self.cabecalho_edicao.is_none()
    }

    /// O nome de uma coluna só fica no arquivo com cabeçalho (sem ele, a gravação não escreve nomes).
    pub(crate) fn renomear_permitido(&self) -> bool {
        self.retrato.as_ref().is_some_and(|r| r.formato.cabecalho)
    }

    // ------------------------------------------------------------------ nova, renomear, excluir

    /// A coluna provisória da Nova coluna na posição, com o cabeçalho em edição. Sem cabeçalho no arquivo, o nome não
    /// seria gravado: a coluna entra direto, com o nome genérico.
    pub(crate) fn nova_coluna(&mut self, posicao: usize) {
        if !self.livre_para_editar() {
            return;
        }
        let nomes = self.retrato.as_ref().map(|r| r.nomes.clone()).unwrap_or_default();
        let posicao = posicao.min(nomes.len());
        let nome = nome_generico(posicao + 1, &nomes);
        if !self.renomear_permitido() {
            self.mutar(Mutacao::NovaColuna { posicao, nome });
            self.focar_grade = true;
            return;
        }
        self.abrir_cabecalho(AlvoCabecalho::Nova { posicao }, nome);
    }

    /// Renomear: o cabeçalho da coluna em edição, com o nome dela (sem cabeçalho no arquivo, desligado).
    pub(crate) fn renomear(&mut self, coluna: usize) {
        if !self.livre_para_editar() || !self.renomear_permitido() {
            return;
        }
        let Some(nome) = self.retrato.as_ref().and_then(|r| r.nomes.get(coluna)).cloned() else { return };
        self.abrir_cabecalho(AlvoCabecalho::Renomear { coluna }, nome);
    }

    fn abrir_cabecalho(&mut self, alvo: AlvoCabecalho, nome: String) {
        // a célula em edição grava antes (a coluna provisória muda as posições das colunas da grade)
        if let Some(ed) = self.edicao.take() {
            self.gravar_celula(ed);
        }
        self.cabecalho_edicao =
            Some(CabecalhoEmEdicao { alvo, sugerido: nome.clone(), nome, pedir_foco: true, focada: false });
        self.selecionar_nome_do_cabecalho = true;
    }

    pub(crate) fn excluir_colunas(&mut self, colunas: Vec<usize>) {
        if colunas.is_empty() || !self.livre_para_editar() {
            return;
        }
        self.colunas_selecionadas.clear();
        self.mutar(Mutacao::ExcluirColunas { colunas });
    }

    /// O cabeçalho em edição terminou: a Nova e o Renomear confirmam com Enter e desistem com o Esc; a
    /// cola confirma sempre (o Esc mantém o nome sugerido).
    pub(crate) fn terminar_cabecalho(&mut self, fim: FimCabecalho) {
        let Some(ed) = self.cabecalho_edicao.take() else { return };
        let nome =
            if ed.nome.trim().is_empty() || fim == FimCabecalho::Esc { ed.sugerido.clone() } else { ed.nome.clone() };
        match ed.alvo {
            AlvoCabecalho::Nova { posicao } => {
                if fim == FimCabecalho::Confirmar {
                    self.mutar(Mutacao::NovaColuna { posicao, nome });
                }
            }
            AlvoCabecalho::Colar { posicao, valores, acrescentar, .. } => {
                let colunas = Arc::new(vec![(nome, valores.as_ref().clone())]);
                self.mutar(Mutacao::ColarColunas { posicao, colunas, acrescentar });
            }
            AlvoCabecalho::Renomear { coluna } => {
                if fim == FimCabecalho::Confirmar && !ed.nome.trim().is_empty() && ed.nome != ed.sugerido {
                    self.mutar(Mutacao::Renomear { coluna, nome: ed.nome });
                }
            }
        }
        self.focar_grade = true;
    }

    // ------------------------------------------------------------------ copiar

    /// Copia as colunas inteiras (numa tarefa, fora do `Mutex`) em TSV com o cabeçalho na primeira linha.
    pub(crate) fn copiar_colunas(&mut self, ectx: &egui::Context, colunas: Vec<usize>) {
        let (Some(tabela), Some(r)) = (self.tabela.clone(), self.retrato.as_ref()) else { return };
        if colunas.is_empty() || self.copia.is_some() {
            return;
        }
        let celulas = r.linhas.max(1) * colunas.len() as u64;
        if celulas > LIMITE_DA_COPIA {
            toast_atencao(ectx, textos::grande_demais_para_copiar(celulas));
            return;
        }
        let nomes: Vec<String> = colunas.iter().filter_map(|&c| r.nomes.get(c).cloned()).collect();
        let n = colunas.len();
        let promessa = Promessa::rodar(ectx, move || {
            let valores = sessao::ler_colunas(&tabela, &colunas, &mut |_, _| Ok(()))?;
            let linhas = valores.first().map_or(0, Vec::len);
            let mut tabela_tsv: Vec<Vec<&str>> = Vec::with_capacity(linhas + 1);
            tabela_tsv.push(nomes.iter().map(String::as_str).collect());
            for i in 0..linhas {
                tabela_tsv.push(valores.iter().map(|c| c[i].as_str()).collect());
            }
            let toast = if n == 1 {
                format!("Coluna {} copiada · {}", nomes[0], plural(linhas as u64, "linha", "linhas"))
            } else {
                format!("{n} colunas copiadas · {}", plural(linhas as u64, "linha", "linhas"))
            };
            Ok((escrever_tsv(&tabela_tsv), toast))
        });
        self.copia = Some(promessa);
    }

    /// A cópia de colunas que terminou: a área de transferência e o toast.
    pub(crate) fn receber_copia(&mut self, ectx: &egui::Context) {
        let Some(r) = self.copia.as_mut().and_then(Promessa::tomar) else { return };
        self.copia = None;
        match r {
            Ok((texto, aviso)) => {
                lembrar_copia(ectx, &texto);
                ectx.copy_text(texto);
                toast_concluido(ectx, aviso);
            }
            Err(e) => interface::pecas::toast::toast_erro(ectx, e.mensagem),
        }
    }

    // ------------------------------------------------------------------ colar

    /// O Colar do menu: o texto vem no `Event::Paste` do quadro seguinte (o `RequestPaste` é o Ctrl+V do sistema).
    fn pedir_cola(&mut self, ectx: &egui::Context, pedido: PedidoCola) {
        self.cola_pedida = Some(pedido);
        ectx.send_viewport_cmd(ViewportCommand::RequestPaste);
    }

    /// Onde o Ctrl+V cola: depois da última coluna selecionada, ou da coluna da célula ativa, ou no fim.
    fn cola_do_teclado(&self) -> PedidoCola {
        let n = self.retrato.as_ref().map_or(0, |r| r.colunas());
        let depois =
            self.colunas_selecionadas.iter().next_back().copied().or_else(|| self.celula_ativa().map(|(_, c)| c));
        PedidoCola::Nova { posicao: depois.map_or(n, |c| c + 1) }
    }

    /// Os `Event::Paste` do quadro que são para a grade (com o foco nela ou num cabeçalho, ou pedidos pelo menu).
    pub(crate) fn tomar_colas(&mut self, ectx: &egui::Context, focos: &[Id]) {
        if self.edicao.is_some() || self.cabecalho_edicao.is_some() {
            return;
        }
        let textos_colados: Vec<String> = ectx.input(|i| {
            i.events.iter().filter_map(|e| if let Event::Paste(t) = e { Some(t.clone()) } else { None }).collect()
        });
        let Some(texto) = textos_colados.into_iter().next_back() else { return };
        // o Ctrl+Shift+V chega como o Paste e a tecla V com o Shift no mesmo quadro
        let com_shift = ectx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, Event::Key { key: Key::V, pressed: true, modifiers, .. } if modifiers.shift))
        });
        let focado = ectx.memory(|m| m.focused());
        let pedido = match self.cola_pedida.take() {
            Some(p) => p,
            None if focado.is_some_and(|f| focos.contains(&f)) => self.cola_do_teclado(),
            None => return,
        };
        if com_shift || pedido == PedidoCola::Correspondencia {
            self.trazer_pedido = Some(Some(texto));
            return;
        }
        if !self.por_posicao_permitida() {
            toast_atencao(ectx, DICA_COLAR_SEM_POSICAO);
            return;
        }
        self.colar(ectx, &texto, pedido);
    }

    /// Cola `texto` (TSV, como o Excel copia) como pedido.
    pub(crate) fn colar(&mut self, ectx: &egui::Context, texto: &str, pedido: PedidoCola) {
        if !self.livre_para_editar() {
            return;
        }
        let Some(r) = self.retrato.as_ref() else { return };
        let linhas_tsv = ler_tsv(texto);
        let colunas = colunas_coladas(&linhas_tsv, origem_do_texto(ectx, texto), r.formato.decimal);
        let valores = colunas.iter().map(|c| c.valores.len()).max().unwrap_or(0);
        if colunas.is_empty() || valores == 0 {
            toast(ectx, "Não há texto em tabela para colar.");
            return;
        }
        let linhas = usize::try_from(r.linhas).unwrap_or(usize::MAX);
        match comparar_quantidades(valores, linhas) {
            Quantidades::SobramValores { .. } if linhas > 0 => {
                self.aviso_cola = Some(AvisoCola::Quantidades { pedido, colunas, valores, linhas });
            }
            // um arquivo vazio ganha as linhas que a cola trouxer
            _ => self.seguir_cola(ectx, pedido, colunas, QuandoSobram::Acrescentar),
        }
    }

    /// A cola depois das quantidades resolvidas.
    fn seguir_cola(
        &mut self,
        ectx: &egui::Context,
        pedido: PedidoCola,
        colunas: Vec<ColunaColada>,
        sobram: QuandoSobram,
    ) {
        let Some(r) = self.retrato.as_ref() else { return };
        let linhas = usize::try_from(r.linhas).unwrap_or(usize::MAX);
        let nomes = r.nomes.clone();
        let mut posicionadas = colunas.iter().map(|c| (c.nome.clone(), posicionar(&c.valores, linhas, sobram)));
        match pedido {
            PedidoCola::Nova { posicao } if colunas.len() == 1 => {
                let Some((nome, p)) = posicionadas.next() else { return };
                let posicao = posicao.min(nomes.len());
                let nome = nome.unwrap_or_else(|| nome_generico(posicao + 1, &nomes));
                if !self.renomear_permitido() {
                    // sem cabeçalho, o nome não seria gravado: nada de pedir um nome
                    let colunas = Arc::new(vec![(nome, p.valores)]);
                    let acrescentar = p.linhas_a_acrescentar as u64;
                    self.mutar(Mutacao::ColarColunas { posicao, colunas, acrescentar });
                    return;
                }
                let alvo = AlvoCabecalho::Colar {
                    posicao,
                    valores: Arc::new(p.valores),
                    acrescentar: p.linhas_a_acrescentar as u64,
                    vazias: p.linhas_vazias,
                };
                self.abrir_cabecalho(alvo, nome);
            }
            PedidoCola::Nova { posicao } => {
                let posicao = posicao.min(nomes.len());
                let mut usados = nomes.clone();
                let mut acrescentar = 0;
                let mut lista = Vec::new();
                for (k, (nome, p)) in posicionadas.enumerate() {
                    let nome = nome.unwrap_or_else(|| nome_generico(posicao + k + 1, &usados));
                    usados.push(nome.clone());
                    acrescentar = acrescentar.max(p.linhas_a_acrescentar as u64);
                    lista.push((nome, p.valores));
                }
                // as colunas mais curtas que a mais longa ficam vazias nas linhas acrescentadas
                self.mutar(Mutacao::ColarColunas { posicao, colunas: Arc::new(lista), acrescentar });
            }
            PedidoCola::Correspondencia => {}
            PedidoCola::Sobre { coluna } => {
                let Some((_, p)) = posicionadas.next() else { return };
                let (Some(tabela), valores) = (self.tabela.clone(), Arc::new(p.valores)) else { return };
                let promessa = Promessa::rodar(ectx, move || {
                    let atual = sessao::ler_colunas(&tabela, &[coluna], &mut |_, _| Ok(()))?;
                    Ok(atual.first().map_or(0, |c| c.iter().filter(|v| !v.trim().is_empty()).count()))
                });
                self.leitura_sobre =
                    Some(LeituraSobre { coluna, valores, acrescentar: p.linhas_a_acrescentar as u64, promessa });
            }
        }
    }

    /// A leitura da coluna da cola por cima: sem valores no caminho, cola; com eles, pergunta.
    pub(crate) fn receber_leitura_sobre(&mut self) {
        let Some(l) = self.leitura_sobre.as_mut() else { return };
        let Some(r) = l.promessa.tomar() else { return };
        let Some(l) = self.leitura_sobre.take() else { return };
        match r {
            Ok(0) => {
                self.mutar(Mutacao::ColarSobre { coluna: l.coluna, valores: l.valores, acrescentar: l.acrescentar });
            }
            Ok(n) => {
                self.aviso_cola =
                    Some(AvisoCola::Substituir { coluna: l.coluna, valores: l.valores, acrescentar: l.acrescentar, n })
            }
            Err(_) => {}
        }
    }

    /// O aviso compacto da cola no alto do conteúdo (e o informativo "24 linhas ficaram vazias" com a coluna colada
    /// ainda em edição). Devolve a altura que ocupou.
    pub(crate) fn desenhar_aviso_cola(&mut self, ui: &mut Ui) -> bool {
        let ectx = ui.ctx().clone();
        let vazias = match self.cabecalho_edicao.as_ref().map(|e| &e.alvo) {
            Some(AlvoCabecalho::Colar { vazias, .. }) if *vazias > 0 => Some(*vazias),
            _ => None,
        };
        let Some(aviso) = self.aviso_cola.clone() else {
            if let Some(n) = vazias {
                ui.add_space(RECUO_AVISO_COLA);
                let frase = format!("{} ficaram vazias", plural(n as u64, "linha", "linhas"));
                aviso_tom(ui, &[Trecho::Normal(&frase)], TomAviso::Informacao);
                ui.add_space(RECUO_AVISO_COLA);
                return true;
            }
            return false;
        };
        ui.add_space(RECUO_AVISO_COLA);
        let clicada = match &aviso {
            AvisoCola::Quantidades { valores, linhas, .. } => {
                let frase = format!(
                    "A coluna colada tem {} valores e o arquivo, {} linhas.",
                    formatar_numero(*valores as u64),
                    formatar_numero(*linhas as u64)
                );
                let colar = format!("Colar {}", formatar_numero(*linhas as u64));
                let acrescentar = format!("Acrescentar {}", plural((valores - linhas) as u64, "linha", "linhas"));
                let acoes = [
                    AcaoAviso::principal(&colar),
                    AcaoAviso::neutra(&acrescentar),
                    AcaoAviso::sutil("Cancelar").dica("Esc"),
                ];
                aviso_com_acoes(ui, &[Trecho::Normal(&frase)], TomAviso::Atencao, &acoes).clicada
            }
            AvisoCola::Substituir { coluna, n, .. } => {
                let nome = self.retrato.as_ref().and_then(|r| r.nomes.get(*coluna)).cloned().unwrap_or_default();
                let frase = format!("Substituir {} valores de {nome}?", formatar_numero(*n as u64));
                let acoes = [AcaoAviso::principal("Substituir"), AcaoAviso::sutil("Cancelar").dica("Esc")];
                aviso_com_acoes(ui, &[Trecho::Normal(&frase)], TomAviso::Atencao, &acoes)
                    .clicada
                    .map(|i| if i == 1 { 2 } else { i })
            }
        };
        ui.add_space(RECUO_AVISO_COLA);
        let esc = ectx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let escolha = if esc { Some(2) } else { clicada };
        if let Some(i) = escolha {
            self.aviso_cola = None;
            match (aviso, i) {
                (AvisoCola::Quantidades { pedido, colunas, .. }, 0) => {
                    self.seguir_cola(&ectx, pedido, colunas, QuandoSobram::Cortar)
                }
                (AvisoCola::Quantidades { pedido, colunas, .. }, 1) => {
                    self.seguir_cola(&ectx, pedido, colunas, QuandoSobram::Acrescentar)
                }
                (AvisoCola::Substituir { coluna, valores, acrescentar, .. }, 0) => {
                    self.mutar(Mutacao::ColarSobre { coluna, valores, acrescentar });
                }
                _ => {}
            }
        }
        true
    }

    /// O toast da cola que entrou: "Coluna valor colada · 1.180 linhas".
    pub(crate) fn toast_da_cola(&self, ectx: &egui::Context, m: &Mutacao) {
        let (nome, linhas) = match m {
            Mutacao::ColarColunas { colunas, .. } if colunas.len() == 1 => (colunas[0].0.clone(), colunas[0].1.len()),
            Mutacao::ColarColunas { colunas, .. } => {
                let linhas = colunas.iter().map(|c| c.1.len()).max().unwrap_or(0);
                toast_concluido(
                    ectx,
                    format!("{} colunas coladas · {}", colunas.len(), plural(linhas as u64, "linha", "linhas")),
                );
                return;
            }
            Mutacao::ColarSobre { coluna, valores, .. } => {
                (self.retrato.as_ref().and_then(|r| r.nomes.get(*coluna)).cloned().unwrap_or_default(), valores.len())
            }
            _ => return,
        };
        toast_concluido(ectx, format!("Coluna {nome} colada · {}", plural(linhas as u64, "linha", "linhas")));
    }

    // ------------------------------------------------------------------ o teclado das colunas

    /// As teclas das colunas com a grade focada (antes do núcleo vê-las): F2 com uma coluna selecionada renomeia, e o
    /// Shift+F10 abre o menu da coluna selecionada (ou da célula ativa), ancorado no cabeçalho dela.
    pub(crate) fn teclas_das_colunas(&mut self, ectx: &egui::Context, id_corpo: Id) {
        if !ectx.memory(|m| m.has_focus(id_corpo)) {
            return;
        }
        if let Some(c) = self.colunas_selecionadas.iter().next().copied()
            && ectx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F2))
        {
            if self.renomear_permitido() {
                self.renomear(c);
            } else {
                toast(ectx, textos::DICA_SEM_CABECALHO);
            }
        }
        if ectx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F10))
            && let Some(c) = self.coluna_em_foco()
        {
            self.menu_teclado = Some(c);
            ectx.memory_mut(|m| m.request_focus(id_do_th(&self.id, c)));
        }
    }

    /// O dono do menu neste quadro: o `th` clicado com o botão direito, o da coluna do menu aberto ou o que recebeu o
    /// foco pelo Shift+F10 (que reaparece aqui como a tecla que o menu espera).
    pub(crate) fn mostrar_menu(&mut self, ui: &mut Ui, ths: &[(usize, Response)], menu_th: Option<usize>) {
        let ectx = ui.ctx().clone();
        if let Some(c) = menu_th {
            if !self.colunas_selecionadas.contains(&c) {
                self.colunas_selecionadas = BTreeSet::from([c]);
                self.ancora_coluna = Some(c);
            }
            self.menu_coluna = Some(c);
        }
        if let Some(c) = self.menu_teclado
            && ths.iter().any(|(k, r)| *k == c && r.has_focus())
        {
            self.menu_teclado = None;
            self.menu_coluna = Some(c);
            ectx.input_mut(|i| {
                i.events.push(Event::Key {
                    key: Key::F10,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::SHIFT,
                })
            });
        }
        let Some(c) = self.menu_coluna else { return };
        let aberto = MenuContexto::aberto(&ectx, ("tela_csv::menu", &self.id));
        let dono = ths.iter().find(|(k, _)| *k == c).map(|(_, r)| r.clone());
        let dono = match dono {
            Some(r) => r,
            None if aberto => {
                ui.interact(Rect::NOTHING, Id::new(("tela_csv::menu_sem_dono", &self.id)), Sense::hover())
            }
            None => return,
        };
        self.menu_do_cabecalho(&ectx, &dono, c);
    }
}

/// O id do `th` de uma coluna (o dono do menu e o alvo do foco do Shift+F10).
pub(crate) fn id_do_th(doc: &str, coluna: usize) -> Id {
    Id::new(("tela_csv::th", doc, coluna))
}
