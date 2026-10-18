//! A ficha da linha (`.dlg.larga`, prancheta 12a; a só leitura, prancheta 15): edição completa (texto curto e longo,
//! números, BLOB, coluna gerada, "NULL", "Usar o padrão"), linha nova, duplicar, excluir e a leitura (visão, banco só
//! leitura, linha sem chave).
//!
//! O corpo é a `.ficha`: uma linha `.fl` por coluna, em grade de 250 | 1fr | 64 com `gap:0 12px`, `padding:9px 2px` e o
//! fio `--b` embaixo. A coluna da esquerda tem a pastilha de tipo de 24 e o nome em 600, com a descrição das regras
//! embaixo (11px `--t3`, recuo de 32); a do meio, o valor; a da direita, a caixa "NULL" (só nas colunas que aceitam
//! nulo). Na linha nova, "Usar o padrão" fica dentro da célula do valor, à direita.
//!
//! A janela não chama o serviço nem abre diálogos: devolve o que a pessoa pediu ([`AcaoJanela`]) e a aba faz.

use std::path::PathBuf;

use banco::TabelaInfo;
use comum::{Afinidade, Celula, Chave, Entrada};
use egui::{Align, Color32, Layout, Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use interface::fontes::{self, Peso};
use interface::formato::{formatar_bytes, nome_do_caminho};
use interface::grade::{dica_celula, texto_celula};
use interface::icones::{self, Icone, TipoColuna};
use interface::pecas::avisos::{TomAviso, aviso, aviso_tom};
use interface::pecas::botao::{Botao, com_altura};
use interface::pecas::campos::{ALTURA_CAMPO, AreaTexto, Check, Entrada as CampoTexto};
use interface::pecas::janela::Janela;
use interface::pecas::rotulos::{TipoPill, chip, pill};
use interface::pecas::texto::{Trecho, galley, galley_cortado, galley_tit, hint};
use interface::tema::paleta;

use crate::grade::texto_selecionado_legivel;
use crate::regras::{TEXTO_CURTO, regras_coluna};

/// A coluna do nome (`250px`), a do NULL (`64px`, a da decisão 21) e o `gap:0 12px` entre as colunas.
const COLUNA_NOME: f32 = 250.0;
const COLUNA_NULO: f32 = 64.0;
const ESPACO_COLUNAS: f32 = 12.0;
/// O `padding:9px 2px` da linha.
const RECUO_X: f32 = 2.0;
const RECUO_Y: f32 = 9.0;
/// A pastilha de tipo de 24 e os 8px até o nome (`.fl-t{gap:8px}`); a descrição recua os 32 de baixo.
const LADO_PASTILHA: f32 = 24.0;
const ESPACO_PASTILHA: f32 = 8.0;
const RECUO_DESCRICAO: f32 = LADO_PASTILHA + ESPACO_PASTILHA;
/// A área de texto do valor longo (`62px`).
const ALTURA_AREA: f32 = 62.0;
/// O `btn pq` do BLOB.
const ALTURA_BOTAO_PQ: f32 = 26.0;
/// A caixa de 16 com os 8px até o texto, e a linha dela (o "NULL" e o "Usar o padrão").
const CAIXA_E_ESPACO: f32 = 16.0 + 8.0;
const ALTURA_CAIXA: f32 = 18.0;
const AJUDA_GERADA: &str = "Coluna gerada: o valor é calculado pelo banco e não se edita";

/// O que a janela sabe de uma coluna.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MetaColuna {
    pub nome: String,
    pub tipo: String,
    pub afinidade: Afinidade,
    pub nao_nulo: bool,
    pub rowid_alias: bool,
    pub autoincremento: bool,
    pub gerada: bool,
    pub padrao: Option<String>,
    /// A pastilha de tipo pelo tipo declarado.
    pub pastilha: TipoColuna,
    /// A descrição embaixo do nome: "INTEGER · chave primária · autoincremento".
    pub regras: String,
    /// A expressão da coluna gerada, lida do CREATE TABLE (None quando não é gerada ou não deu para ler).
    pub expressao: Option<String>,
}

/// A dica do valor de uma coluna gerada: com a expressão, "Coluna gerada: o valor sai de CAST(preco * 100 AS INTEGER)
/// e não se edita".
fn ajuda_gerada(meta: &MetaColuna) -> String {
    match &meta.expressao {
        Some(e) => format!("Coluna gerada: o valor sai de {e} e não se edita"),
        None => AJUDA_GERADA.to_string(),
    }
}

/// A expressão da coluna gerada `nome` no CREATE TABLE da tabela.
fn expressao_gerada(info: &TabelaInfo, nome: &str) -> Option<String> {
    let def = ddl::analisar_create_table(info.sql.as_deref()?).ok()?;
    let baixo = nome.to_lowercase();
    let coluna = def.colunas.into_iter().find(|c| c.nome.to_lowercase() == baixo)?;
    coluna.gerada.map(|g| g.expressao).filter(|e| !e.trim().is_empty())
}

impl MetaColuna {
    pub fn de(info: &TabelaInfo, nome: &str) -> MetaColuna {
        match info.colunas.iter().find(|c| c.nome == nome) {
            Some(c) => MetaColuna {
                expressao: if c.gerada { expressao_gerada(info, &c.nome) } else { None },
                nome: c.nome.clone(),
                tipo: c.tipo.clone(),
                afinidade: c.afinidade,
                nao_nulo: c.nao_nulo,
                rowid_alias: c.rowid_alias,
                autoincremento: c.autoincremento,
                gerada: c.gerada,
                padrao: c.padrao.clone(),
                pastilha: TipoColuna::do_declarado(&c.tipo, c.pk > 0, c.fk.is_some()),
                regras: regras_coluna(c),
            },
            None => MetaColuna {
                nome: nome.to_string(),
                tipo: String::new(),
                afinidade: Afinidade::Blob,
                nao_nulo: false,
                rowid_alias: false,
                autoincremento: false,
                gerada: false,
                padrao: None,
                pastilha: TipoColuna::do_declarado("", false, false),
                regras: "sem tipo".to_string(),
                expressao: None,
            },
        }
    }

    /// A coluna aceita a caixa "NULL" (`aceita_nulo`).
    fn aceita_nulo(&self) -> bool {
        !self.nao_nulo && !self.rowid_alias && !self.gerada
    }
}

/// O controle de um campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TipoCampo {
    Texto,
    Numero,
    /// Texto longo ou com quebra de linha (`textarea`).
    Area,
    Blob,
    /// Coluna gerada: o valor num campo só leitura.
    Gerada,
}

/// Um campo da janela de edição.
#[derive(Debug, Clone)]
pub(crate) struct CampoJanela {
    pub meta: MetaColuna,
    pub tipo: TipoCampo,
    /// O valor de antes (None numa linha nova).
    pub original: Option<Celula>,
    /// O que o campo mostra (o BLOB, a coluna gerada).
    pub valor: Celula,
    pub texto: String,
    /// A caixa "NULL" (só nas colunas que aceitam nulo).
    pub nulo: Option<bool>,
    /// A caixa "Usar o padrão" (só na linha nova, nas colunas com padrão ou automáticas).
    pub padrao: Option<bool>,
    /// O arquivo escolhido para o BLOB (grava ao salvar).
    pub arquivo: Option<PathBuf>,
    pub largo: bool,
    pub placeholder: Option<String>,
    /// Linha duplicada de uma com BLOB: o conteúdo não vem junto (dica embaixo).
    pub blob_nao_copiado: bool,
    /// O botão do BLOB: "Substituir por arquivo" (havia conteúdo) ou "Escolher arquivo".
    pub rotulo_trocar: &'static str,
}

impl CampoJanela {
    fn travado(&self) -> bool {
        self.nulo == Some(true) || self.padrao == Some(true)
    }

    fn id_controle(&self, serial: u64, i: usize) -> egui::Id {
        egui::Id::new(("tela_dados::campo", serial, i))
    }
}

/// Para que a janela foi aberta.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModoJanela {
    Editar { chave: Chave },
    Nova { duplicada: bool },
    Leitura { chave: Option<Chave> },
}

/// O aviso no alto do corpo (`j.aviso`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AvisoJanela {
    Texto {
        mensagem: String,
        perigo: bool,
    },
    /// A nota neutra da ficha só leitura: "exemplo.db está aberto só para leitura. Nada é gravado no arquivo."
    Nota(String),
    /// "Arquivo salvo: foto.bin · 112 bytes".
    ArquivoSalvo {
        nome: String,
        bytes: u64,
    },
}

/// O que a pessoa pediu na janela neste quadro.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AcaoJanela {
    /// ✕, Esc, Cancelar, Fechar ou clique no fundo.
    Fechar,
    /// "Salvar" / "Incluir linha" (ou Enter num campo): os valores que mudaram.
    Enviar(Vec<(String, Entrada)>),
    Excluir,
    /// Uma linha nova a partir do que está na janela agora.
    Duplicar(Vec<(String, Celula)>),
    /// "Salvar em arquivo" do BLOB desta coluna (com os primeiros bytes, para sugerir a extensão).
    SalvarBlob {
        coluna: String,
        inicio: Vec<u8>,
    },
    /// "Substituir por arquivo" / "Escolher arquivo" do campo `campo`.
    EscolherArquivo {
        campo: usize,
    },
    /// "Copiar a linha" da ficha só leitura: o texto da linha, uma coluna por linha.
    Copiar(String),
}

/// A ficha da linha aberta.
pub(crate) struct JanelaLinha {
    serial: u64,
    pub modo: ModoJanela,
    pub titulo: String,
    /// O nome da tabela, depois do título ("clientes"; "clientes · só leitura" na leitura).
    tabela: String,
    /// A ficha do cabeçalho: a da tabela ou a da visão.
    icone: Icone,
    pub campos: Vec<CampoJanela>,
    /// Leitura: rótulo · valor.
    pub leitura: Vec<(MetaColuna, Celula)>,
    /// Leitura: o texto de cada valor longo (o das áreas de texto), feito uma vez ao abrir (vazio nos outros); a
    /// área só leitura mostra este mesmo `String` a cada quadro, sem copiar o texto.
    textos_longos: Vec<String>,
    pub aviso: Option<AvisoJanela>,
    /// Gravando (os botões do rodapé ficam desabilitados).
    pub ocupado: bool,
    /// A coluna cujo campo recebe o foco ao abrir (a célula que a pessoa tentou editar).
    pub foco: Option<String>,
    quadros: u32,
    /// Pedir o foco neste campo no próximo quadro (desmarcou "NULL" ou "Usar o padrão").
    focar: Option<usize>,
}

fn icone_da_tabela(info: &TabelaInfo) -> Icone {
    estado::icone_da_tabela(info.tipo, info.sem_rowid)
}

impl JanelaLinha {
    /// A janela de edição (`Editar`) ou de linha nova (`Nova`), com os valores dados por coluna.
    pub fn edicao(
        serial: u64,
        info: &TabelaInfo,
        colunas: &[String],
        modo: ModoJanela,
        titulo: String,
        valores: &[(String, Celula)],
        foco: Option<String>,
    ) -> JanelaLinha {
        let (nova, duplicada) = match modo {
            ModoJanela::Nova { duplicada } => (true, duplicada),
            _ => (false, false),
        };
        let campos = colunas
            .iter()
            .map(|nome| {
                let meta = MetaColuna::de(info, nome);
                let guardado = valores.iter().find(|(n, _)| n == nome).map(|(_, v)| v.clone());
                montar_campo(meta, guardado, nova, duplicada)
            })
            .collect();
        JanelaLinha {
            serial,
            modo,
            titulo,
            tabela: info.nome.clone(),
            icone: icone_da_tabela(info),
            campos,
            leitura: Vec::new(),
            textos_longos: Vec::new(),
            aviso: None,
            ocupado: false,
            foco,
            quadros: 0,
            focar: None,
        }
    }

    /// A janela só de leitura (visão, banco só leitura, linha sem chave).
    pub fn leitura(
        serial: u64,
        info: &TabelaInfo,
        colunas: &[String],
        chave: Option<Chave>,
        titulo: String,
        valores: &[(String, Celula)],
        aviso: Option<AvisoJanela>,
    ) -> JanelaLinha {
        let leitura: Vec<(MetaColuna, Celula)> = colunas
            .iter()
            .map(|nome| {
                let v = valores.iter().find(|(n, _)| n == nome).map_or(Celula::Nulo, |(_, v)| v.clone());
                (MetaColuna::de(info, nome), v)
            })
            .collect();
        let textos_longos =
            leitura.iter().map(|(_, v)| if e_longo(v) { valor_em_texto(v) } else { String::new() }).collect();
        JanelaLinha {
            serial,
            modo: ModoJanela::Leitura { chave },
            titulo,
            tabela: info.nome.clone(),
            icone: icone_da_tabela(info),
            campos: Vec::new(),
            leitura,
            textos_longos,
            aviso,
            ocupado: false,
            foco: None,
            quadros: 0,
            focar: None,
        }
    }

    /// O número desta janela (cada abertura tem o seu: uma resposta que chega depois de a janela fechar não
    /// mexe na seguinte).
    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn nova(&self) -> bool {
        matches!(self.modo, ModoJanela::Nova { .. })
    }

    pub fn chave(&self) -> Option<&Chave> {
        match &self.modo {
            ModoJanela::Editar { chave } => Some(chave),
            ModoJanela::Leitura { chave } => chave.as_ref(),
            ModoJanela::Nova { .. } => None,
        }
    }

    /// Os valores que vão no pedido (`coletar`): só o que mudou; na linha nova, tudo menos o padrão.
    pub fn coletar(&self) -> Vec<(String, Entrada)> {
        let nova = self.nova();
        self.campos.iter().filter_map(|f| ler_campo(f, nova).map(|e| (f.meta.nome.clone(), e))).collect()
    }

    /// Os valores para a linha duplicada: o que está na janela agora (também o que ainda não foi salvo).
    pub fn base_duplicada(&self) -> Vec<(String, Celula)> {
        self.campos
            .iter()
            .filter(|f| f.tipo != TipoCampo::Gerada)
            .map(|f| {
                let v = if f.nulo == Some(true) {
                    Celula::Nulo
                } else if f.tipo == TipoCampo::Blob {
                    f.original.clone().unwrap_or(Celula::Nulo)
                } else {
                    Celula::Texto(f.texto.clone())
                };
                (f.meta.nome.clone(), v)
            })
            .collect()
    }

    /// O texto do "Copiar a linha": uma coluna por linha, "nome: valor" (NULL como "NULL", o BLOB pelo tamanho).
    pub fn texto_da_linha(&self) -> String {
        self.leitura
            .iter()
            .map(|(meta, valor)| format!("{}: {}", meta.nome, valor_em_texto(valor)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// O arquivo escolhido para o BLOB de um campo: "Novo conteúdo: …" e as caixas desmarcadas.
    pub fn escolher_arquivo(&mut self, campo: usize, caminho: PathBuf) {
        if let Some(f) = self.campos.get_mut(campo) {
            f.arquivo = Some(caminho);
            if f.nulo == Some(true) {
                f.nulo = Some(false);
            }
            if f.padrao == Some(true) {
                f.padrao = Some(false);
            }
        }
    }

    /// Desenha a janela; devolve o que a pessoa pediu.
    pub fn mostrar(&mut self, ctx: &egui::Context) -> Option<AcaoJanela> {
        let mut acao: Option<AcaoJanela> = None;
        let titulo = self.titulo.clone();
        let leitura = matches!(self.modo, ModoJanela::Leitura { .. });
        let sub = if leitura { format!("{} · só leitura", self.tabela) } else { self.tabela.clone() };
        let id = ("tela_dados::linha", self.serial);
        let janela = Janela::new(id, &titulo).larga(true).icone(self.icone).sub(&sub);
        let resposta = match self.modo.clone() {
            ModoJanela::Leitura { chave } => {
                let (mut copiar, mut fechar) = (false, false);
                let r = janela
                    .rodape_direita(|ui| {
                        if ui.add(Botao::new("Copiar a linha")).clicked() {
                            copiar = true;
                        }
                        if ui.add(Botao::primario("Fechar")).clicked() {
                            fechar = true;
                        }
                    })
                    .mostrar(ctx, |ui| self.corpo_leitura(ui, chave.is_some()));
                if copiar {
                    acao = Some(AcaoJanela::Copiar(self.texto_da_linha()));
                } else if fechar {
                    acao = Some(AcaoJanela::Fechar);
                }
                r
            }
            ModoJanela::Nova { .. } => {
                let ocupado = self.ocupado;
                let (mut enviar, mut cancelar) = (false, false);
                let r = janela
                    .rodape_direita(|ui| {
                        if ui.add(Botao::ghost("Cancelar").habilitado(!ocupado)).clicked() {
                            cancelar = true;
                        }
                        if ui.add(Botao::primario("Incluir linha").habilitado(!ocupado)).clicked() {
                            enviar = true;
                        }
                    })
                    .mostrar(ctx, |ui| self.corpo_edicao(ui));
                if cancelar {
                    acao = Some(AcaoJanela::Fechar);
                } else if enviar {
                    acao = Some(AcaoJanela::Enviar(self.coletar()));
                }
                r
            }
            ModoJanela::Editar { .. } => {
                let ocupado = self.ocupado;
                let (mut enviar, mut excluir, mut duplicar, mut cancelar) = (false, false, false, false);
                let r = janela
                    .rodape_esquerda(|ui| {
                        if ui.add(Botao::ghost_perigo("Excluir linha").habilitado(!ocupado)).clicked() {
                            excluir = true;
                        }
                        if ui.add(Botao::new("Duplicar").habilitado(!ocupado)).clicked() {
                            duplicar = true;
                        }
                    })
                    .rodape_direita(|ui| {
                        if ui.add(Botao::ghost("Cancelar").habilitado(!ocupado)).clicked() {
                            cancelar = true;
                        }
                        if ui.add(Botao::primario("Salvar").habilitado(!ocupado)).clicked() {
                            enviar = true;
                        }
                    })
                    .mostrar(ctx, |ui| self.corpo_edicao(ui));
                if excluir {
                    acao = Some(AcaoJanela::Excluir);
                } else if duplicar {
                    acao = Some(AcaoJanela::Duplicar(self.base_duplicada()));
                } else if cancelar {
                    acao = Some(AcaoJanela::Fechar);
                } else if enviar {
                    acao = Some(AcaoJanela::Enviar(self.coletar()));
                }
                r
            }
        };
        self.quadros = self.quadros.saturating_add(1);
        let corpo = resposta.inner;
        if acao.is_none() {
            acao = corpo;
        }
        if acao.is_none() && resposta.enviou && !self.ocupado && !matches!(self.modo, ModoJanela::Leitura { .. }) {
            acao = Some(AcaoJanela::Enviar(self.coletar()));
        }
        if resposta.pediu_fechar {
            acao = Some(AcaoJanela::Fechar);
        }
        acao
    }

    fn mostrar_aviso(&self, ui: &mut Ui) {
        match &self.aviso {
            None => {}
            Some(AvisoJanela::Texto { mensagem, perigo }) => {
                aviso(ui, &[Trecho::Normal(mensagem)], *perigo);
            }
            Some(AvisoJanela::Nota(mensagem)) => {
                aviso_tom(ui, &[Trecho::Normal(mensagem)], TomAviso::Nota);
            }
            Some(AvisoJanela::ArquivoSalvo { nome, bytes }) => {
                let tamanho = formatar_bytes(*bytes);
                aviso_tom(
                    ui,
                    &[
                        Trecho::Normal("Arquivo salvo: "),
                        Trecho::Forte(nome),
                        Trecho::Normal(" · "),
                        Trecho::Normal(&tamanho),
                    ],
                    TomAviso::Concluido,
                );
            }
        }
    }

    fn corpo_edicao(&mut self, ui: &mut Ui) -> Option<AcaoJanela> {
        texto_selecionado_legivel(ui);
        self.mostrar_aviso(ui);
        let primeiro_quadro = self.quadros == 0;
        // o foco pedido ao abrir: a célula que a pessoa tentou editar, se for um campo de digitar habilitado
        let foco_inicial = if primeiro_quadro {
            self.foco.as_ref().and_then(|c| {
                self.campos.iter().position(|f| {
                    &f.meta.nome == c
                        && matches!(f.tipo, TipoCampo::Texto | TipoCampo::Numero | TipoCampo::Area)
                        && !f.travado()
                })
            })
        } else {
            None
        };
        let focar = self.focar.take().or(foco_inicial);
        let serial = self.serial;
        let chave = self.chave().is_some();
        let nova = self.nova();
        let campos = &mut self.campos;
        let mut acao: Option<AcaoInterna> = None;
        ficha(ui, true, |ui| {
            let ultimo = campos.len().saturating_sub(1);
            for (i, f) in campos.iter_mut().enumerate() {
                let alturas = Alturas {
                    nome: altura_do_nome(ui, &f.meta.regras),
                    valor: altura_do_valor(f),
                    nulo: if f.nulo.is_some() { ALTURA_CAIXA } else { 0.0 },
                };
                linha_da_ficha(ui, (serial, i), alturas, i == ultimo, |nome, valor, nulo| {
                    celula_do_nome(nome, &f.meta);
                    valor_do_campo(valor, f, serial, i, nova, chave, focar, &mut acao);
                    celula_do_nulo(nulo, f, i, &mut acao);
                });
            }
        });
        match acao {
            Some(AcaoInterna::Publica(a)) => Some(a),
            Some(AcaoInterna::Focar(i)) => {
                // desmarcar "NULL" ou "Usar o padrão" devolve o foco ao campo (no quadro seguinte, já habilitado)
                self.focar = Some(i);
                ui.ctx().request_repaint();
                None
            }
            None => None,
        }
    }

    fn corpo_leitura(&mut self, ui: &mut Ui, tem_chave: bool) -> Option<AcaoJanela> {
        texto_selecionado_legivel(ui);
        self.mostrar_aviso(ui);
        let serial = self.serial;
        let mut acao = None;
        let leitura = &self.leitura;
        let longos = &mut self.textos_longos;
        ficha(ui, true, |ui| {
            let ultimo = leitura.len().saturating_sub(1);
            for (i, ((meta, valor), longo)) in leitura.iter().zip(longos.iter_mut()).enumerate() {
                let alturas = Alturas {
                    nome: altura_do_nome(ui, &meta.regras),
                    valor: altura_do_valor_leitura(valor),
                    nulo: if meta.aceita_nulo() { ALTURA_CAIXA } else { 0.0 },
                };
                linha_da_ficha(ui, (serial, i), alturas, i == ultimo, |nome, ui_valor, nulo| {
                    celula_do_nome(nome, meta);
                    if let Some(a) = valor_de_leitura(ui_valor, meta, valor, longo, serial, i, tem_chave) {
                        acao = Some(a);
                    }
                    if meta.aceita_nulo() {
                        // o NULL não se marca na leitura: a caixa fica desligada, marcada onde o valor é NULL
                        let mut marcado = valor.e_nulo();
                        nulo.with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.add_enabled_ui(false, |ui| caixa_nulo(ui, &mut marcado, &meta.nome));
                        });
                    }
                });
            }
        });
        acao
    }
}

/// O valor de uma célula em texto, para o "Copiar a linha".
fn valor_em_texto(valor: &Celula) -> String {
    match valor {
        Celula::Nulo => "NULL".to_string(),
        Celula::Blob { tamanho, .. } => format!("BLOB · {}", formatar_bytes(*tamanho)),
        Celula::TextoCortado { inicio, .. } => format!("{inicio}…"),
        c => texto_celula(c),
    }
}

// ---------------------------------------------------------------------------------------------------- a ficha

/// As alturas do que cada coluna desenha (para centrar as três na linha).
#[derive(Clone, Copy)]
struct Alturas {
    nome: f32,
    valor: f32,
    nulo: f32,
}

/// O corpo `.ficha` (`margin:-6px 0`): o cabeçalho "Coluna | Valor | NULL" e as linhas, coladas (sem espaço entre
/// elas), com a largura toda do corpo da janela.
fn ficha(ui: &mut Ui, com_nulo: bool, linhas: impl FnOnce(&mut Ui)) {
    let largura = ui.available_width();
    let origem = ui.cursor().min - vec2(0.0, 6.0);
    let mut filho = ui.new_child(
        UiBuilder::new()
            .id_salt("tela_dados::ficha")
            .max_rect(Rect::from_min_size(origem, vec2(largura, f32::INFINITY)))
            .layout(Layout::top_down(Align::Min)),
    );
    filho.spacing_mut().item_spacing = vec2(0.0, 0.0);
    cabecalho_da_ficha(&mut filho, com_nulo);
    linhas(&mut filho);
    let altura = filho.min_rect().height();
    // a margem de -6px também embaixo
    ui.allocate_space(vec2(largura, (altura - 12.0).max(0.0)));
}

/// As colunas de uma linha: onde começa e quanto mede o valor, e onde fica o NULL.
fn colunas(esquerda: f32, largura: f32) -> [(f32, f32); 3] {
    let x_nome = esquerda + RECUO_X;
    let x_valor = x_nome + COLUNA_NOME + ESPACO_COLUNAS;
    let w_valor = (largura - 2.0 * RECUO_X - COLUNA_NOME - COLUNA_NULO - 2.0 * ESPACO_COLUNAS).max(40.0);
    let x_nulo = x_valor + w_valor + ESPACO_COLUNAS;
    [(x_nome, COLUNA_NOME), (x_valor, w_valor), (x_nulo, COLUNA_NULO)]
}

/// O `.fl-cab`: "Coluna", "Valor" e "NULL" (11px / 600, caixa alta, `--t3`) com o fio `--vidrob` embaixo.
fn cabecalho_da_ficha(ui: &mut Ui, com_nulo: bool) {
    let p = paleta(ui);
    let e = &p.estudio;
    let largura = ui.available_width();
    let topo = ui.cursor().min;
    let [(x_nome, _), (x_valor, _), (x_nulo, w_nulo)] = colunas(topo.x, largura);
    let g = galley_tit(ui, "Coluna", e.t3);
    let altura = g.size().y + 6.0 + 1.0;
    ui.painter().galley(pos2(x_nome, topo.y), g, e.t3);
    let g = galley_tit(ui, "Valor", e.t3);
    ui.painter().galley(pos2(x_valor, topo.y), g, e.t3);
    if com_nulo {
        let g = galley_tit(ui, "NULL", e.t3);
        ui.painter().galley(pos2(x_nulo + (w_nulo - g.size().x) / 2.0, topo.y), g, e.t3);
    }
    let rect = Rect::from_min_size(topo, vec2(largura, altura));
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, e.vidrob));
    ui.allocate_rect(rect, Sense::hover());
}

/// Uma linha `.fl`: as três células, centradas na vertical pelas alturas que o chamador sabe, e o fio `--b`
/// embaixo (menos na última). A linha cresce se o que foi desenhado passar das alturas.
fn linha_da_ficha(
    ui: &mut Ui,
    id: (u64, usize),
    alturas: Alturas,
    ultima: bool,
    celulas: impl FnOnce(&mut Ui, &mut Ui, &mut Ui),
) {
    let p = paleta(ui);
    let largura = ui.available_width();
    let topo = ui.cursor().min;
    let interno = alturas.nome.max(alturas.valor).max(alturas.nulo);
    let [(x_nome, w_nome), (x_valor, w_valor), (x_nulo, w_nulo)] = colunas(topo.x, largura);
    let mut filho = |sufixo: u8, x: f32, w: f32, altura: f32| {
        let y = topo.y + RECUO_Y + (interno - altura) / 2.0;
        ui.new_child(
            UiBuilder::new()
                .id_salt(("tela_dados::celula", id, sufixo))
                .max_rect(Rect::from_min_size(pos2(x, y), vec2(w, f32::INFINITY)))
                .layout(Layout::top_down(Align::Min)),
        )
    };
    let mut u_nome = filho(0, x_nome, w_nome, alturas.nome);
    let mut u_valor = filho(1, x_valor, w_valor, alturas.valor);
    let mut u_nulo = filho(2, x_nulo, w_nulo, alturas.nulo);
    celulas(&mut u_nome, &mut u_valor, &mut u_nulo);
    let fim =
        [&u_nome, &u_valor, &u_nulo].iter().map(|u| u.min_rect().bottom()).fold(topo.y + RECUO_Y + interno, f32::max)
            + RECUO_Y;
    let rect = Rect::from_min_max(topo, pos2(topo.x + largura, fim));
    if !ultima {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, p.estudio.b));
    }
    ui.allocate_rect(rect, Sense::hover());
}

/// A altura da célula do nome: a pastilha, os 2px e a descrição (11px).
fn altura_do_nome(ui: &Ui, regras: &str) -> f32 {
    let g = galley(ui, regras, fontes::fonte(11.0, Peso::Normal), Color32::WHITE);
    LADO_PASTILHA + 2.0 + g.size().y
}

/// A altura do que a célula do valor desenha (a de uma linha só; o BLOB que quebra, e a área que cresce, esticam a
/// linha).
fn altura_do_valor(f: &CampoJanela) -> f32 {
    match f.tipo {
        TipoCampo::Area if f.nulo != Some(true) => ALTURA_AREA,
        TipoCampo::Blob => ALTURA_BOTAO_PQ,
        _ => ALTURA_CAMPO,
    }
}

fn altura_do_valor_leitura(valor: &Celula) -> f32 {
    match valor {
        Celula::Blob { .. } => ALTURA_BOTAO_PQ,
        v if e_longo(v) => ALTURA_AREA,
        _ => ALTURA_CAMPO,
    }
}

/// O texto passa dos [`TEXTO_CURTO`] caracteres ou tem quebra de linha (na leitura vira a área de texto).
fn e_longo(valor: &Celula) -> bool {
    match valor {
        Celula::Nulo | Celula::Blob { .. } | Celula::Inteiro(_) | Celula::Real(_) => false,
        Celula::TextoCortado { .. } => true,
        v => {
            let t = texto_celula(v);
            t.chars().count() > TEXTO_CURTO || t.contains(['\r', '\n'])
        }
    }
}

/// A coluna do nome: a pastilha de tipo de 24 e o nome em 600, e embaixo a descrição das regras em `--t3`, 11px, com
/// o recuo de 32 (cortada com "…", com o texto inteiro na dica).
fn celula_do_nome(ui: &mut Ui, meta: &MetaColuna) {
    let p = paleta(ui);
    let e = &p.estudio;
    ui.spacing_mut().item_spacing.y = 2.0;
    let largura = ui.available_width();
    let (linha, resp_nome) = ui.allocate_exact_size(vec2(largura, LADO_PASTILHA), Sense::hover());
    let pastilha = Rect::from_min_size(linha.min, vec2(LADO_PASTILHA, LADO_PASTILHA));
    icones::pintar(ui.painter(), pastilha, Icone::Tipo(meta.pastilha), Color32::WHITE);
    let g =
        galley_cortado(ui, &meta.nome, fontes::fonte(13.0, Peso::Medio), p.fg, (largura - RECUO_DESCRICAO).max(10.0));
    ui.painter().galley(pos2(linha.left() + RECUO_DESCRICAO, linha.center().y - g.size().y / 2.0), g, p.fg);
    resp_nome.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &meta.nome));
    let fonte = fontes::fonte(11.0, Peso::Normal);
    let disponivel = (largura - RECUO_DESCRICAO).max(10.0);
    let cortada = galley(ui, &meta.regras, fonte.clone(), e.t3).size().x > disponivel + 0.5;
    let g = galley_cortado(ui, &meta.regras, fonte, e.t3, disponivel);
    let (rect, _) = ui.allocate_exact_size(vec2(largura, g.size().y), Sense::hover());
    // a resposta cobre só o texto (recuado de 32), com a dica quando ele foi cortado
    let rect_texto = Rect::from_min_size(pos2(rect.left() + RECUO_DESCRICAO, rect.top()), g.size());
    ui.painter().galley(rect_texto.min, g, e.t3);
    let resp = ui.interact(rect_texto, ui.id().with(("tela_dados::descricao", &meta.nome)), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &meta.regras));
    if cortada {
        resp.on_hover_text(&meta.regras);
    }
}

/// A caixa "NULL" (11px `--t3`), com o nome para o leitor de tela "email é NULL".
fn caixa_nulo(ui: &mut Ui, marcado: &mut bool, coluna: &str) -> egui::Response {
    let rotulo = format!("{coluna} é NULL");
    let t3 = paleta(ui).estudio.t3;
    Check::new(marcado, "NULL").tamanho_texto(11.0).cor_texto(t3).rotulo(&rotulo).mostrar(ui)
}

/// A coluna do NULL: a caixa, centrada, só nas colunas que aceitam nulo.
fn celula_do_nulo(ui: &mut Ui, f: &mut CampoJanela, i: usize, acao: &mut Option<AcaoInterna>) {
    let Some(mut nulo) = f.nulo else { return };
    let r = ui.with_layout(Layout::top_down(Align::Center), |ui| caixa_nulo(ui, &mut nulo, &f.meta.nome)).inner;
    if r.changed() {
        f.nulo = Some(nulo);
        if nulo && f.padrao.is_some() {
            f.padrao = Some(false);
        }
        if !nulo {
            *acao = Some(AcaoInterna::Focar(i));
        }
    }
}

// ------------------------------------------------------------------------------------------------------ valores

/// `span.nulo` ("NULL" em itálico, `--t3`).
fn nulo(ui: &mut Ui) {
    let p = paleta(ui);
    let mut f = interface::pecas::texto::formato(fontes::fonte(11.5, Peso::Italico), p.faint);
    f.extra_letter_spacing = 0.23;
    let g = ui.fonts_mut(|fs| fs.layout_job(egui::text::LayoutJob::single_section("NULL".to_owned(), f)));
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x, 19.5), Sense::hover());
    ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, p.faint);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, "NULL"));
}

/// A pílula do BLOB (`span.pill` "112 bytes", a prancheta 12a), com a dica: "Início em hexadecimal: …" (ou
/// "Conteúdo binário").
fn pilula_blob(ui: &mut Ui, valor: &Celula) {
    let Celula::Blob { tamanho, .. } = valor else { return };
    let r = pill(ui, &formatar_bytes(*tamanho), TipoPill::Neutra);
    if r.hovered()
        && let Some(dica) = dica_celula(valor)
    {
        r.on_hover_text(dica);
    }
}

/// O que um campo pediu (a aba recebe as públicas; o foco a janela resolve).
enum AcaoInterna {
    Publica(AcaoJanela),
    Focar(usize),
}

/// A coluna do valor de um campo de edição: o controle e, na linha nova, o "Usar o padrão" dentro da célula, à
/// direita.
#[allow(clippy::too_many_arguments)]
fn valor_do_campo(
    ui: &mut Ui,
    f: &mut CampoJanela,
    serial: u64,
    i: usize,
    nova: bool,
    tem_chave: bool,
    focar: Option<usize>,
    acao: &mut Option<AcaoInterna>,
) {
    let p = paleta(ui);
    let id = f.id_controle(serial, i);
    let nome = f.meta.nome.clone();
    let padrao_marcado = f.padrao == Some(true);
    let nulo_marcado = f.nulo == Some(true);
    let largura = ui.available_width();
    // "Usar o padrão": a caixa de 16, os 8px e o texto, e 8px até o controle
    let largura_padrao = f.padrao.map_or(0.0, |_| {
        CAIXA_E_ESPACO + galley(ui, "Usar o padrão", fontes::fonte(12.0, Peso::Normal), p.muted).size().x + 8.0
    });
    let largura_controle = (largura - largura_padrao).max(40.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        match f.tipo {
            TipoCampo::Texto | TipoCampo::Numero | TipoCampo::Area if nulo_marcado => {
                // o NULL marcado: o campo fica normal (sem esmaecer) e mostra "NULL" em itálico
                let mut vazio = String::new();
                ui.add(
                    CampoTexto::new(&mut vazio)
                        .fixo(true)
                        .dica("NULL")
                        .dica_italica(true)
                        .id_salt(("tela_dados::campo", serial, i))
                        .rotulo(&nome)
                        .largura(largura_controle),
                );
            }
            TipoCampo::Texto | TipoCampo::Numero => {
                let mut e = CampoTexto::new(&mut f.texto)
                    .habilitado(!padrao_marcado)
                    .id_salt(("tela_dados::campo", serial, i))
                    .rotulo(&nome)
                    .largura(largura_controle);
                if let Some(d) = &f.placeholder {
                    e = e.dica(d);
                }
                ui.add(e);
            }
            TipoCampo::Area => {
                AreaTexto::new(&mut f.texto)
                    .habilitado(!padrao_marcado)
                    .id_salt(("tela_dados::campo", serial, i))
                    .altura(ALTURA_AREA)
                    .largura(largura_controle)
                    .mostrar(ui);
            }
            TipoCampo::Gerada => {
                let (mut texto, dica) = match &f.valor {
                    _ if nova => (String::new(), "Calculada pelo banco ao incluir a linha."),
                    Celula::Nulo => (String::new(), "NULL"),
                    Celula::Texto(s) if s.is_empty() => (String::new(), "vazio"),
                    v => (valor_em_texto(v), ""),
                };
                let r = ui.add(
                    CampoTexto::new(&mut texto)
                        .somente_leitura(true)
                        .dica(dica)
                        .dica_italica(!nova)
                        .id_salt(("tela_dados::campo", serial, i))
                        .rotulo(&nome)
                        .largura(largura_controle),
                );
                r.on_hover_ui(|ui| {
                    ui.label(ajuda_gerada(&f.meta));
                });
            }
            TipoCampo::Blob => {
                ui.scope(|ui| {
                    ui.set_max_width(largura_controle);
                    blob_do_campo(ui, f, i, nova, tem_chave, acao);
                });
            }
        }
        if focar == Some(i) && !f.travado() {
            ui.memory_mut(|m| m.request_focus(id));
        }
        if let Some(mut padrao) = f.padrao {
            let r = Check::new(&mut padrao, "Usar o padrão").tamanho_texto(12.0).cor_texto(p.muted).mostrar(ui);
            if r.changed() {
                f.padrao = Some(padrao);
                if padrao && f.nulo.is_some() {
                    f.nulo = Some(false);
                }
                if !padrao {
                    *acao = Some(AcaoInterna::Focar(i));
                }
            }
        }
    });
    if f.blob_nao_copiado {
        ui.add_space(4.0);
        hint(ui, "O conteúdo binário não é copiado; escolha um arquivo se precisar dele.");
    }
}

/// O BLOB: o resumo (o chip, o "NULL" ou o arquivo novo) e os botões `btn pq` "Salvar em arquivo" e "Substituir por
/// arquivo" (o "Escolher arquivo" quando não há BLOB), quebrando em linhas quando não cabem.
fn blob_do_campo(
    ui: &mut Ui,
    f: &mut CampoJanela,
    i: usize,
    nova: bool,
    tem_chave: bool,
    acao: &mut Option<AcaoInterna>,
) {
    let nome = f.meta.nome.clone();
    let (mut clicou_salvar, mut clicou_trocar) = (false, false);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        // o resumo (chip, NULL ou o arquivo novo); só o "Usar o padrão" o esmaece
        ui.scope(|ui| {
            if f.padrao == Some(true) {
                ui.multiply_opacity(0.5);
            }
            match &f.arquivo {
                Some(caminho) => {
                    let texto = caminho.to_string_lossy();
                    let rotulo = format!("Novo conteúdo: {}", nome_do_caminho(&texto));
                    chip(ui, &rotulo, false).on_hover_text(texto.as_ref());
                    hint(ui, "grava ao salvar");
                }
                None if f.nulo == Some(true) => nulo(ui),
                None => match &f.valor {
                    Celula::Blob { .. } => pilula_blob(ui, &f.valor),
                    _ => nulo(ui),
                },
            }
        });
        let tem_blob = matches!(f.original, Some(Celula::Blob { .. }));
        // os `btn pq` de 26px da janela
        com_altura(ui, ALTURA_BOTAO_PQ, |ui| {
            if !nova && tem_blob && tem_chave && ui.add(Botao::new("Salvar em arquivo")).clicked() {
                clicou_salvar = true;
            }
            if ui.add(Botao::new(f.rotulo_trocar).habilitado(f.padrao != Some(true))).clicked() {
                clicou_trocar = true;
            }
        });
    });
    if clicou_salvar {
        let inicio = match &f.original {
            Some(Celula::Blob { inicio, .. }) => inicio.clone(),
            _ => Vec::new(),
        };
        *acao = Some(AcaoInterna::Publica(AcaoJanela::SalvarBlob { coluna: nome, inicio }));
    } else if clicou_trocar {
        *acao = Some(AcaoInterna::Publica(AcaoJanela::EscolherArquivo { campo: i }));
    }
}

/// A coluna do valor na ficha só leitura (`.campo.so-leitura`: fundo `--p`, texto `--t2` opaco, sem baixo-relevo);
/// devolve o "Salvar em arquivo" do BLOB, quando clicado.
fn valor_de_leitura(
    ui: &mut Ui,
    meta: &MetaColuna,
    valor: &Celula,
    texto_longo: &mut String,
    serial: u64,
    i: usize,
    tem_chave: bool,
) -> Option<AcaoJanela> {
    let largura = ui.available_width();
    let id = ("tela_dados::leitura", serial, i);
    match valor {
        Celula::Blob { .. } => {
            let mut clicou = false;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                pilula_blob(ui, valor);
                if tem_chave && com_altura(ui, ALTURA_BOTAO_PQ, |ui| ui.add(Botao::new("Salvar em arquivo"))).clicked()
                {
                    clicou = true;
                }
            });
            clicou.then(|| {
                let inicio = match valor {
                    Celula::Blob { inicio, .. } => inicio.clone(),
                    _ => Vec::new(),
                };
                AcaoJanela::SalvarBlob { coluna: meta.nome.clone(), inicio }
            })
        }
        v if e_longo(v) => {
            // o texto já está pronto na janela (feito ao abrir): nenhuma cópia por quadro
            AreaTexto::new(texto_longo)
                .somente_leitura(true)
                .altura(ALTURA_AREA)
                .id_salt(id)
                .largura(largura)
                .mostrar(ui);
            None
        }
        v => {
            let (mut texto, dica) = match v {
                Celula::Nulo => (String::new(), "NULL"),
                Celula::Texto(s) if s.is_empty() => (String::new(), "vazio"),
                outro => (texto_celula(outro), ""),
            };
            // os números ficam à esquerda, como o resto da ficha (prancheta 12)
            let r = ui.add(
                CampoTexto::new(&mut texto)
                    .somente_leitura(true)
                    .dica(dica)
                    .dica_italica(true)
                    .id_salt(id)
                    .rotulo(&meta.nome)
                    .largura(largura),
            );
            if meta.gerada {
                r.on_hover_ui(|ui| {
                    ui.label(ajuda_gerada(meta));
                });
            }
            None
        }
    }
}

/// Monta um campo como `janelaEdicao` (tipo do controle, texto, placeholder, caixas e o estado delas).
fn montar_campo(meta: MetaColuna, guardado: Option<Celula>, nova: bool, duplicada: bool) -> CampoJanela {
    let valor = guardado.unwrap_or(Celula::Nulo);
    let original = if nova { None } else { Some(valor.clone()) };
    let tem_padrao = meta.padrao.is_some();
    let automatico = meta.autoincremento || meta.rowid_alias;
    let aceita_nulo = meta.aceita_nulo();
    let numerico = meta.afinidade.numerica();
    let e_blob = matches!(valor, Celula::Blob { .. });
    let tipo_blob = meta.tipo.to_ascii_uppercase().contains("BLOB");
    let mut f = CampoJanela {
        tipo: TipoCampo::Texto,
        original,
        valor: valor.clone(),
        texto: String::new(),
        nulo: None,
        padrao: None,
        arquivo: None,
        largo: false,
        placeholder: None,
        blob_nao_copiado: false,
        rotulo_trocar: if e_blob { "Substituir por arquivo" } else { "Escolher arquivo" },
        meta,
    };
    if f.meta.gerada {
        f.tipo = TipoCampo::Gerada;
    } else if e_blob || (tipo_blob && valor.e_nulo()) {
        f.tipo = TipoCampo::Blob;
        f.largo = true;
        if nova && duplicada && e_blob {
            // o conteúdo binário não é copiado
            f.valor = Celula::Nulo;
            f.blob_nao_copiado = true;
        }
    } else {
        let texto = texto_celula(&valor);
        let longo = !numerico && (texto.chars().count() > TEXTO_CURTO || texto.contains(['\r', '\n']));
        if longo {
            f.largo = true;
            f.tipo = TipoCampo::Area;
        } else {
            f.tipo = if numerico { TipoCampo::Numero } else { TipoCampo::Texto };
        }
        f.texto = texto;
        if nova && (tem_padrao || automatico) {
            f.placeholder = Some(if automatico {
                "automático".to_string()
            } else {
                format!("padrão: {}", f.meta.padrao.as_deref().unwrap_or(""))
            });
        }
    }
    if nova && !f.meta.gerada && (tem_padrao || automatico) {
        f.padrao = Some(!duplicada || automatico);
    }
    if aceita_nulo {
        f.nulo = Some(valor.e_nulo() && f.padrao != Some(true) && !(nova && !duplicada));
    }
    f
}

/// A entrada de gravação de um campo (`lerCampo`); `None` = não mudou (fica fora do pedido). Numa linha nova,
/// campo vazio de coluna que aceita nulo grava NULL.
pub(crate) fn ler_campo(f: &CampoJanela, nova: bool) -> Option<Entrada> {
    if f.tipo == TipoCampo::Gerada {
        return None;
    }
    if f.padrao == Some(true) {
        return nova.then_some(Entrada::Padrao);
    }
    if f.nulo == Some(true) {
        let ja_era_nulo = f.original.as_ref().is_none_or(Celula::e_nulo);
        return if !nova && ja_era_nulo { None } else { Some(Entrada::Nulo) };
    }
    if f.tipo == TipoCampo::Blob {
        return f.arquivo.clone().map(Entrada::Arquivo);
    }
    let v = &f.texto;
    if nova {
        return Some(if v.is_empty() && f.nulo.is_some() { Entrada::Nulo } else { Entrada::Texto(v.clone()) });
    }
    match &f.original {
        None | Some(Celula::Nulo) => (!(v.is_empty() && f.nulo.is_none())).then(|| Entrada::Texto(v.clone())),
        Some(o) => (*v != texto_celula(o)).then(|| Entrada::Texto(v.clone())),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn meta(nome: &str, afinidade: Afinidade) -> MetaColuna {
        MetaColuna {
            nome: nome.into(),
            tipo: afinidade.como_str().into(),
            afinidade,
            nao_nulo: false,
            rowid_alias: false,
            autoincremento: false,
            gerada: false,
            padrao: None,
            pastilha: TipoColuna::Texto,
            regras: String::new(),
            expressao: None,
        }
    }

    #[test]
    fn dica_da_coluna_gerada_cita_a_expressao() {
        let mut m = meta("centavos", Afinidade::Integer);
        m.gerada = true;
        assert_eq!(ajuda_gerada(&m), AJUDA_GERADA);
        m.expressao = Some("CAST(preco * 100 AS INTEGER)".into());
        assert_eq!(ajuda_gerada(&m), "Coluna gerada: o valor sai de CAST(preco * 100 AS INTEGER) e não se edita");
    }

    #[test]
    fn campo_novo_com_padrao_nasce_marcado_e_grava_o_padrao() {
        let mut m = meta("cidade", Afinidade::Text);
        m.padrao = Some("'São Paulo'".into());
        let f = montar_campo(m, None, true, false);
        assert_eq!(f.padrao, Some(true));
        assert_eq!(f.nulo, Some(false));
        assert_eq!(f.placeholder.as_deref(), Some("padrão: 'São Paulo'"));
        assert_eq!(ler_campo(&f, true), Some(Entrada::Padrao));
    }

    #[test]
    fn campo_que_nao_mudou_fica_fora_do_pedido() {
        let f = montar_campo(meta("nome", Afinidade::Text), Some(Celula::Texto("Ana".into())), false, false);
        assert_eq!(ler_campo(&f, false), None);
        let mut f2 = f.clone();
        f2.texto = "Bia".into();
        assert_eq!(ler_campo(&f2, false), Some(Entrada::Texto("Bia".into())));
        f2.nulo = Some(true);
        assert_eq!(ler_campo(&f2, false), Some(Entrada::Nulo));
        // já era NULL e continua: nada a mandar
        let f3 = montar_campo(meta("limite", Afinidade::Real), Some(Celula::Nulo), false, false);
        assert_eq!(f3.nulo, Some(true));
        assert_eq!(ler_campo(&f3, false), None);
    }

    #[test]
    fn linha_nova_vazia_grava_null_onde_aceita() {
        let f = montar_campo(meta("email", Afinidade::Text), None, true, false);
        assert_eq!(f.nulo, Some(false), "na linha nova o NULL nasce desmarcado");
        assert_eq!(ler_campo(&f, true), Some(Entrada::Nulo));
        let mut m = meta("nome", Afinidade::Text);
        m.nao_nulo = true;
        let f = montar_campo(m, None, true, false);
        assert_eq!(f.nulo, None, "NOT NULL não oferece nulo");
        assert_eq!(ler_campo(&f, true), Some(Entrada::Texto(String::new())));
    }

    #[test]
    fn texto_longo_vira_area_e_blob_vira_blob() {
        let longo = "x".repeat(81);
        let f = montar_campo(meta("obs", Afinidade::Text), Some(Celula::Texto(longo)), false, false);
        assert_eq!((f.tipo, f.largo), (TipoCampo::Area, true));
        let f = montar_campo(meta("n", Afinidade::Integer), Some(Celula::Inteiro(5)), false, false);
        assert_eq!(f.tipo, TipoCampo::Numero);
        let mut m = meta("foto", Afinidade::Blob);
        m.tipo = "BLOB".into();
        let f = montar_campo(m.clone(), Some(Celula::Nulo), false, false);
        assert_eq!(f.tipo, TipoCampo::Blob);
        let dup = montar_campo(m, Some(Celula::Blob { tamanho: 3, inicio: vec![1, 2, 3] }), true, true);
        assert!(dup.blob_nao_copiado);
        assert_eq!(dup.valor, Celula::Nulo);
        assert_eq!(dup.rotulo_trocar, "Substituir por arquivo");
        // sem arquivo escolhido, a coluna fica fora do INSERT
        assert_eq!(ler_campo(&dup, true), None);
    }

    #[test]
    fn duplicada_mantem_o_automatico_marcado() {
        let mut m = meta("id", Afinidade::Integer);
        m.rowid_alias = true;
        m.autoincremento = true;
        let f = montar_campo(m, Some(Celula::Texto("61".into())), true, true);
        assert_eq!(f.padrao, Some(true));
        assert_eq!(f.placeholder.as_deref(), Some("automático"));
        let mut m = meta("cidade", Afinidade::Text);
        m.padrao = Some("'São Paulo'".into());
        let f = montar_campo(m, Some(Celula::Nulo), true, true);
        assert_eq!(f.padrao, Some(false), "duplicada: o padrão só fica marcado no automático");
        assert_eq!(f.nulo, Some(true), "o NULL da linha de origem vem junto");
    }

    #[test]
    fn as_tres_colunas_da_ficha() {
        // o corpo da janela larga: 758 - 40 = 718 de largura
        let [(x_nome, w_nome), (x_valor, w_valor), (x_nulo, w_nulo)] = colunas(0.0, 718.0);
        assert_eq!((x_nome, w_nome), (2.0, 250.0));
        assert_eq!(x_valor, 264.0);
        assert_eq!(w_nulo, 64.0);
        assert!((x_nulo + w_nulo + RECUO_X - 718.0).abs() < 0.01, "as colunas fecham a largura");
        assert!((w_valor - (718.0 - 4.0 - 250.0 - 64.0 - 24.0)).abs() < 0.01);
    }

    #[test]
    fn texto_da_linha_copiada() {
        let mut m = meta("foto", Afinidade::Blob);
        m.tipo = "BLOB".into();
        let leitura = vec![
            (meta("codigo", Afinidade::Text), Celula::Texto("P003".into())),
            (m, Celula::Blob { tamanho: 112, inicio: vec![] }),
            (meta("serie", Afinidade::Integer), Celula::Nulo),
        ];
        let j = JanelaLinha {
            serial: 1,
            modo: ModoJanela::Leitura { chave: None },
            titulo: "Linha P003".into(),
            tabela: "produtos".into(),
            icone: Icone::Tabela,
            campos: Vec::new(),
            leitura,
            textos_longos: Vec::new(),
            aviso: None,
            ocupado: false,
            foco: None,
            quadros: 0,
            focar: None,
        };
        assert_eq!(j.texto_da_linha(), "codigo: P003\nfoto: BLOB · 112 bytes\nserie: NULL");
    }
}
