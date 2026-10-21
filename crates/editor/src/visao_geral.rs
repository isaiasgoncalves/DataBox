//! Visão geral do banco (`#/banco`) no formato de documento da prancheta 1
//! (`Main.dc.html`):
//!
//! - **faixa**: o cilindro de 20, o nome em 600/13, a pílula "só leitura" (com o motivo na dica) e o caminho em
//!   `--t3` semilight, com a reticência no meio quando não cabe;
//! - **conteúdo** (gap de 18): os contadores na dock (`interface::pecas::dock`, no singular com um: "1 Visão") e a
//!   lista de Tabelas (o título 13/600 e a `.lista-t` na moldura `--b2`), filtrada pela busca da barra ("Buscar
//!   tabelas", sem acento nem caixa);
//! - **Propriedades**: "Arquivo" (os nove campos, o Caminho numa linha com o Copiar caminho e o toast "Caminho
//!   copiado.") e "Manutenção" (as verificações, Otimizar, Compactar (VACUUM) e a cópia de segurança, só as
//!   verificações e a cópia em só leitura; o resultado vem no aviso flutuante, o toast, também com a aba escondida ou
//!   fechada: a cópia diz "Cópia salva: x.db · 119 MB" com a ação "Mostrar na pasta" nele e o caminho na dica). Sem o painel, as duas seções vão para
//!   o fim da página (a casca faz isso).
//! - **barra**: Nova tabela (desligada em só leitura), Atualizar e Exportar o banco (desligado sem tabelas nem
//!   visões).
//!
//! A tela desenha a partir do estado da casca (`estado.banco`, `estado.objetos`). Ao abrir, e de novo
//! depois de `reler`, ela pede à casca que releia o BancoInfo e os objetos (`RecarregarBanco` e `RecarregarObjetos`,
//! juntos); o Atualizar pede os dois na hora, sem refazer a
//! tela (a busca, o resultado e a manutenção em andamento ficam). O Explorador recebe a mesma lista.

use std::path::PathBuf;
use std::sync::Arc;

use banco::{AcaoManutencao, BancoInfo, CopiaFeita, ObjetosInfo, ResultadoManutencao, TipoObjeto};
use comum::Codigo;
use egui::{Align, Layout, Rect, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use estado::{
    Aba, Acao, Anatomia, Contexto, Oferta, Ofertas, OrigemJanelaExportar, PedidoJanelaExportar, Promessa, Reacao, Rota,
    Verbo, icone_da_tabela,
};
use interface::fontes::{self, Peso};
use interface::formato::{
    UnidadeBytes, formatar_bytes, formatar_bytes_em, formatar_data_opt, formatar_numero, formatar_numero_opt,
    nome_do_caminho, partes_bytes, sem_acento,
};
use interface::icones::{self, Icone};
use interface::pecas::avisos::{carregando, vazio};
use interface::pecas::botao::Botao;
use interface::pecas::dock::{ItemDock, dock};
use interface::pecas::janela::{Confirmacao, confirmar};
use interface::pecas::painel::{Secao, ficha};
use interface::pecas::rotulos::{TipoPill, pill};
use interface::pecas::tabela::{ColunaTabela, Tabela, moldura_lista};
use interface::pecas::texto::{galley, galley_caminho, nome_abre, texto, titulo_lista, y_na_linha};
use interface::pecas::toast::{AcaoToast, toast_atencao, toast_concluido, toast_concluido_com_acao, toast_erro};
use interface::tema::paleta;

use crate::casca::explorador::icone_do_banco;
use crate::recursos::nome_base;

/// A dica do contador "Linhas (contadas)".
pub(crate) const DICA_LINHAS_CONTADAS: &str =
    "Tabelas grandes não são contadas ao abrir; a soma é das que foram contadas.";

/// A dica do Exportar da barra de ferramentas.
pub(crate) const DICA_EXPORTAR_BANCO: &str = "Exportar o banco";

/// O toast do Copiar caminho das Propriedades.
pub(crate) const MSG_CAMINHO_COPIADO: &str = "Caminho copiado.";

/// O pedido da janela Exportar do banco inteiro (o Exportar da barra de ferramentas), com o nome do arquivo sem a
/// extensão.
pub(crate) fn pedido_exportacao(b: &BancoInfo) -> PedidoJanelaExportar {
    PedidoJanelaExportar { origem: OrigemJanelaExportar::Banco, nome_sugerido: nome_base(&b.nome) }
}

/// A pergunta antes de substituir um arquivo que já existe (só acontece com o
/// caminho digitado, porque o diálogo do Windows já perguntou).
pub(crate) fn pergunta_substituir(detalhe: Option<&str>) -> String {
    let nome = detalhe.map(nome_do_caminho).filter(|n| !n.is_empty()).unwrap_or("o arquivo");
    format!("Já existe {nome}. Substituir? O conteúdo atual do arquivo se perde.")
}

/// A lista de Tabelas sem tabelas.
pub(crate) const VAZIO_TABELAS: &str =
    "O banco não tem tabelas. Use o botão Nova tabela da barra de ferramentas ou a Consulta SQL para criar a primeira.";
/// A lista de Tabelas quando a busca da barra não acha nenhuma.
pub(crate) const VAZIO_BUSCA_TABELAS: &str = "Nenhuma tabela corresponde à busca.";
/// A dica (e o rótulo) da busca da barra na Visão geral.
pub(crate) const DICA_BUSCA: &str = "Buscar tabelas";
/// A pergunta antes do VACUUM.
pub(crate) const PERGUNTA_VACUUM: &str =
    "Compactar reescreve o arquivo inteiro e pode demorar em bancos grandes. Continuar?";

/// O `gap:18px` entre os blocos do conteúdo e o título da lista (`h2` 13/600 com `margin-bottom:8px`).
const ESPACO_BLOCOS: f32 = 18.0;
const ESPACO_TITULO_LISTA: f32 = 8.0;
/// As colunas da lista de Tabelas (`th style="width:…"`): Linhas, Colunas, Tipo e a do botão "Colunas" (a vista das
/// Propriedades, revisão de UI de 19/10/2026). Com menos de [`NOME_MINIMO`] para o nome, as quatro ficam com a
/// largura do conteúdo (a lista não rola na janela estreita).
const LARGURAS_LISTA: [f32; 4] = [110.0, 100.0, 150.0, 130.0];
const NOME_MINIMO: f32 = 200.0;
/// O cilindro da faixa (`.sk.s20`) e a ficha de cada tabela (`.sk.s16`, com o `gap:8px` do `.link`).
const LADO_BANCO_FAIXA: f32 = 20.0;
const LADO_FICHA: f32 = 16.0;
const ESPACO_FICHA: f32 = 8.0;
/// `.det` da Manutenção: `padding:2px 8px 12px; gap:2px`.
const RECUO_MANUTENCAO: [f32; 3] = [2.0, 8.0, 12.0];
const ESPACO_MANUTENCAO: f32 = 2.0;
/// O espaço entre o Caminho e o Copiar caminho, e a linha da ficha (`.det`: 12px × 1,4).
const ESPACO_COPIAR: f32 = 4.0;
const ALTURA_LINHA_FICHA: f32 = 17.0;

/// Um botão de manutenção, com a silhueta (D7).
struct BotaoManutencao {
    acao: AcaoManutencao,
    rotulo: &'static str,
    icone: Icone,
    /// Grava no arquivo: não aparece em banco só leitura e, no fim, o BancoInfo é relido.
    escreve: bool,
}

const ACOES: [BotaoManutencao; 4] = [
    BotaoManutencao {
        acao: AcaoManutencao::Integridade,
        rotulo: "Verificar integridade",
        icone: Icone::VerificarIntegridade,
        escreve: false,
    },
    BotaoManutencao {
        acao: AcaoManutencao::Chaves,
        rotulo: "Verificar chaves estrangeiras",
        icone: Icone::VerificarChaves,
        escreve: false,
    },
    BotaoManutencao { acao: AcaoManutencao::Otimizar, rotulo: "Otimizar", icone: Icone::Otimizar, escreve: true },
    BotaoManutencao {
        acao: AcaoManutencao::Vacuum,
        rotulo: "Compactar (VACUUM)",
        icone: Icone::Compactar,
        escreve: true,
    },
];
/// O último botão da Manutenção (sem reticência).
const ROTULO_COPIA: &str = "Fazer cópia de segurança";

/// O que o toast de uma manutenção ou cópia que terminou diz.
pub(crate) enum Resultado {
    Mensagens { linhas: Vec<String>, de_para: Option<(String, String)>, perigo: bool },
    Copia { caminho: PathBuf, tamanho: String },
    Erro(String),
}

/// O rótulo da ação do toast da cópia de segurança.
pub(crate) const ROTULO_MOSTRAR_NA_PASTA: &str = "Mostrar na pasta";

pub(crate) enum Execucao {
    Manutencao(AcaoManutencao, Promessa<ResultadoManutencao>),
    /// A cópia para o caminho; `substituir` já foi confirmado (pelo diálogo do Windows ou pela pergunta).
    Copia {
        promessa: Promessa<CopiaFeita>,
        caminho: PathBuf,
        substituir: bool,
    },
}

impl Execucao {
    /// A execução que a casca guardou ao soltar a aba: a resposta, com o que fazer depois (o BancoInfo relido, se a
    /// manutenção gravou no arquivo). None enquanto não chegou.
    pub(crate) fn tomar_solta(&mut self) -> Option<comum::Resultado<(Resultado, bool)>> {
        match self {
            Execucao::Manutencao(acao, p) => {
                let acao = *acao;
                p.tomar().map(|r| r.map(|r| (resultado_da_manutencao(acao, &r), escreve(acao))))
            }
            Execucao::Copia { promessa, .. } => promessa.tomar().map(|r| r.map(|c| (resultado_da_copia(c), false))),
        }
    }
}

/// A ação grava no arquivo (o BancoInfo é relido no fim).
fn escreve(acao: AcaoManutencao) -> bool {
    ACOES.iter().any(|a| a.acao == acao && a.escreve)
}

/// O aviso de uma manutenção que terminou.
fn resultado_da_manutencao(acao: AcaoManutencao, r: &ResultadoManutencao) -> Resultado {
    let mut linhas = r.mensagens.clone();
    let de_para = (acao == AcaoManutencao::Vacuum).then(|| {
        let escala = escala_de(r.tamanho_antes, r.tamanho_depois);
        (bytes_na_escala(r.tamanho_antes, escala), bytes_na_escala(r.tamanho_depois, escala))
    });
    if linhas.is_empty() && de_para.is_none() {
        linhas.push(if r.ok { "Concluído, sem problemas." } else { "A verificação encontrou problemas." }.to_string());
    }
    Resultado::Mensagens { linhas, de_para, perigo: !r.ok }
}

/// O aviso de uma cópia de segurança feita.
fn resultado_da_copia(c: CopiaFeita) -> Resultado {
    Resultado::Copia { caminho: c.caminho, tamanho: formatar_bytes(c.tamanho) }
}

/// O resultado de uma manutenção ou cópia no aviso flutuante (o toast, no tom concluído, de atenção ou de erro),
/// com a tela à vista, escondida ou já fechada. A cópia diz "Cópia salva: noticias (cópia).db · 119 MB", com
/// "Mostrar na pasta" no próprio toast e o caminho inteiro na dica.
pub(crate) fn avisar_resultado(egui: &egui::Context, r: &Resultado) {
    match r {
        Resultado::Erro(m) => toast_erro(egui, m.clone()),
        Resultado::Mensagens { linhas, de_para, perigo } => {
            let mut texto = linhas.join(" ");
            if let Some((antes, depois)) = de_para {
                if !texto.is_empty() {
                    texto.push(' ');
                }
                texto.push_str(&format!("De {antes} para {depois}."));
            }
            if *perigo { toast_atencao(egui, texto) } else { toast_concluido(egui, texto) }
        }
        Resultado::Copia { caminho, tamanho } => {
            let inteiro = caminho.display().to_string();
            let nome = nome_do_caminho(&inteiro);
            toast_concluido_com_acao(
                egui,
                format!("Cópia salva: {nome} · {tamanho}"),
                Some(inteiro.clone()),
                AcaoToast { rotulo: ROTULO_MOSTRAR_NA_PASTA.to_string(), carga: inteiro },
            );
        }
    }
}

/// O rótulo do contador no singular com um ("1 Visão") e no plural com os outros (item 26 da revisão).
fn singular_ou_plural(n: u64, um: &str, varios: &str) -> String {
    if n == 1 { um } else { varios }.to_string()
}

/// Os textos da tela, refeitos só quando o estado muda.
struct Textos {
    versao: u64,
    caminho: String,
    /// Os contadores da dock (Tabelas, Visões, Índices, Gatilhos, Tamanho e Linhas).
    dock: Vec<ItemDock>,
    linhas_tabela: Vec<String>,
    colunas_tabela: Vec<String>,
    /// O nome de cada tabela sem acento e em minúsculas, para a busca da barra.
    chaves_tabelas: Vec<String>,
    tamanho_arquivo: String,
    modificado: String,
    tamanho_pagina: String,
    versao_usuario: String,
}

impl Textos {
    fn de(versao: u64, b: &BancoInfo, objetos: Option<&ObjetosInfo>) -> Textos {
        let tabelas = objetos.map(|o| o.tabelas.as_slice()).unwrap_or_default();
        let conhecidas: Vec<u64> = tabelas.iter().filter_map(|t| t.linhas).collect();
        let total_linhas: u64 = conhecidas.iter().sum();
        let contadas = conhecidas.len() < tabelas.len();
        let c = &b.contagem;
        let tamanho = partes_bytes(b.tamanho, None);
        let contador = |icone: Icone, n: u64, um: &str, varios: &str| {
            ItemDock::new(icone, formatar_numero(n), singular_ou_plural(n, um, varios))
        };
        let mut linhas = ItemDock::new(
            Icone::LinhasGrande,
            formatar_numero(total_linhas),
            if contadas {
                "Linhas (contadas)".to_string()
            } else {
                singular_ou_plural(total_linhas, "Linha", "Linhas")
            },
        );
        if contadas {
            linhas = linhas.dica(DICA_LINHAS_CONTADAS);
        }
        let mut tamanho_item = ItemDock::new(Icone::DiscoGrande, &tamanho.numero, "Tamanho");
        if !tamanho.unidade.is_empty() {
            tamanho_item = tamanho_item.unidade(tamanho.unidade);
        }
        Textos {
            versao,
            caminho: b.caminho.display().to_string(),
            dock: vec![
                contador(Icone::TabelaGrande, c.tabelas, "Tabela", "Tabelas"),
                contador(Icone::VisaoGrande, c.visoes, "Visão", "Visões"),
                contador(Icone::IndiceGrande, c.indices, "Índice", "Índices"),
                contador(Icone::GatilhoGrande, c.gatilhos, "Gatilho", "Gatilhos"),
                tamanho_item,
                linhas,
            ],
            linhas_tabela: tabelas.iter().map(|t| formatar_numero_opt(t.linhas)).collect(),
            colunas_tabela: tabelas.iter().map(|t| formatar_numero_opt(t.colunas)).collect(),
            chaves_tabelas: tabelas.iter().map(|t| sem_acento(&t.nome)).collect(),
            tamanho_arquivo: formatar_bytes(b.tamanho),
            modificado: formatar_data_opt(b.modificado_em.as_deref()),
            tamanho_pagina: formatar_bytes(b.tamanho_pagina),
            versao_usuario: formatar_numero(b.versao_usuario),
        }
    }
}

/// A Visão geral do banco.
pub(crate) struct TelaBanco {
    /// Já pediu à casca que releia o BancoInfo e os objetos.
    iniciada: bool,
    execucao: Option<Execucao>,
    confirmacao: Option<Confirmacao>,
    /// "Substituir o arquivo" (a cópia para um caminho digitado que já existe) e o caminho dela.
    confirmacao_substituir: Option<Confirmacao>,
    destino_substituir: Option<PathBuf>,
    /// O caminho da cópia que a casca devolveu (o do diálogo do Windows, que já perguntou antes de substituir, ou o
    /// digitado na janela "Pedir o caminho", que não perguntou), à espera do próximo quadro.
    copia_pedida: Option<(PathBuf, bool)>,
    textos: Option<Textos>,
    /// O texto da busca da barra ("Buscar tabelas"): filtra a lista de tabelas pelo nome, sem acento nem caixa.
    busca: String,
    /// O que o menu Banco pediu (uma manutenção ou a cópia), à espera do próximo quadro.
    pedido_do_menu: Option<PedidoDoMenu>,
}

/// O que o menu Banco pede à Visão geral (revisão de UI de 19/10/2026): o mesmo dos botões da Manutenção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PedidoDoMenu {
    Manutencao(AcaoManutencao),
    Copia,
}

/// A unidade do "de X para Y" do VACUUM: a do maior dos dois tamanhos (os dois na mesma escala).
fn escala_de(antes: u64, depois: u64) -> Option<UnidadeBytes> {
    match partes_bytes(antes.max(depois), None).unidade {
        "KB" => Some(UnidadeBytes::Kb),
        "MB" => Some(UnidadeBytes::Mb),
        "GB" => Some(UnidadeBytes::Gb),
        "TB" => Some(UnidadeBytes::Tb),
        _ => None,
    }
}

fn bytes_na_escala(n: u64, escala: Option<UnidadeBytes>) -> String {
    match escala {
        Some(u) => formatar_bytes_em(n, u),
        None => formatar_bytes(n),
    }
}

impl TelaBanco {
    pub fn nova() -> TelaBanco {
        TelaBanco {
            iniciada: false,
            execucao: None,
            confirmacao: None,
            confirmacao_substituir: None,
            destino_substituir: None,
            copia_pedida: None,
            textos: None,
            busca: String::new(),
            pedido_do_menu: None,
        }
    }

    /// Um pedido do menu Banco: atendido no próximo quadro, como o clique no botão da Manutenção (o Compactar
    /// pergunta antes; com outra manutenção ou cópia correndo, o pedido é esquecido, como o botão desligado).
    pub(crate) fn pedir(&mut self, pedido: PedidoDoMenu) {
        if self.execucao.is_none() {
            self.pedido_do_menu = Some(pedido);
        }
    }

    /// Há tarefas desta tela rodando.
    pub fn ocupada(&self) -> bool {
        self.execucao.is_some() || self.copia_pedida.is_some()
    }

    /// Outra aba mudou o banco: o BancoInfo e os objetos são pedidos de novo à casca na próxima vez que a tela se
    /// desenhar, sem refazer a tela. A manutenção ou a cópia em andamento continua (o resultado dela aparece quando
    /// chegar) e o que está à vista fica até a leitura nova chegar.
    pub fn reler(&mut self) {
        self.iniciada = false;
    }

    /// O caminho da cópia que a casca devolveu: a cópia começa no próximo quadro. `substituir` vem do diálogo do
    /// Windows, que já perguntou; o caminho digitado na janela "Pedir o caminho" vem sem (o serviço confere se o
    /// arquivo existe e a tela pergunta antes).
    pub fn copiar_para(&mut self, caminho: PathBuf, substituir: bool) {
        self.copia_pedida = Some((caminho, substituir));
    }

    /// Os textos do estado de agora (refeitos quando a versão do estado muda).
    fn atualizar_textos(&mut self, ctx: &Contexto) {
        let Some(b) = ctx.estado.banco.as_ref() else { return };
        if self.textos.as_ref().is_none_or(|t| t.versao != ctx.versao) {
            self.textos = Some(Textos::de(ctx.versao, b, ctx.estado.objetos.as_ref()));
        }
    }

    fn copiar(&mut self, egui: &egui::Context, ctx: &Contexto, caminho: PathBuf, substituir: bool) {
        let destino = caminho.clone();
        let promessa = ctx.tarefa(egui, move |s, id| s.copia(id, &destino, substituir));
        self.execucao = Some(Execucao::Copia { promessa, caminho, substituir });
    }

    fn manutencao(&mut self, egui: &egui::Context, ctx: &Contexto, acao: AcaoManutencao) {
        self.execucao = Some(Execucao::Manutencao(acao, ctx.tarefa(egui, move |s, id| s.manutencao(id, acao))));
    }

    /// A manutenção ou a cópia ainda em andamento, para a casca terminar de esperar por ela quando a aba sai
    /// (fechou, ou a casca refez o documento): a resposta vira um toast em vez de se perder com a tela.
    pub(crate) fn soltar_execucao(&mut self) -> Option<Execucao> {
        self.execucao.take()
    }

    fn receber(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        match self.execucao.take() {
            Some(Execucao::Manutencao(acao, mut p)) => match p.tomar() {
                None => self.execucao = Some(Execucao::Manutencao(acao, p)),
                Some(Ok(r)) => {
                    avisar_resultado(egui, &resultado_da_manutencao(acao, &r));
                    if escreve(acao) {
                        // tamanho e contagens mudam: a casca relê o BancoInfo
                        ctx.emitir(Acao::RecarregarBanco);
                    }
                }
                Some(Err(e)) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        avisar_resultado(egui, &Resultado::Erro(e.mensagem));
                    }
                }
            },
            Some(Execucao::Copia { mut promessa, caminho, substituir }) => match promessa.tomar() {
                None => self.execucao = Some(Execucao::Copia { promessa, caminho, substituir }),
                Some(Ok(c)) => {
                    avisar_resultado(egui, &resultado_da_copia(c));
                    // a cópia criou um arquivo, talvez na pasta aberta: a árvore do Explorador relê (sem pasta, nada)
                    ctx.emitir(Acao::AtualizarPasta);
                }
                // o arquivo digitado já existe: pergunta antes de substituir
                Some(Err(e)) if !substituir && e.tem_codigo(Codigo::DestinoExiste) => {
                    self.confirmacao_substituir = Some(
                        Confirmacao::new(pergunta_substituir(e.detalhe.as_deref()))
                            .titulo("Substituir o arquivo")
                            .rotulo("Substituir")
                            .perigo(true),
                    );
                    self.destino_substituir = Some(caminho);
                }
                Some(Err(e)) => {
                    if ctx.reagir(&e) == Reacao::Mostrar {
                        avisar_resultado(egui, &Resultado::Erro(e.mensagem));
                    }
                }
            },
            None => {}
        }
    }

    /// As respostas, a cópia que a casca devolveu e as perguntas (Compactar, Substituir), antes do conteúdo.
    fn antes_do_conteudo(&mut self, ui: &Ui, ctx: &mut Contexto) {
        if !self.iniciada {
            // a casca relê os dois e redesenha (a tela mostra o estado de agora até lá)
            self.iniciada = true;
            ctx.emitir(Acao::RecarregarBanco);
            ctx.emitir(Acao::RecarregarObjetos);
        }
        self.receber(ui.ctx(), ctx);
        if let Some((caminho, substituir)) = self.copia_pedida.take()
            && self.execucao.is_none()
        {
            self.copiar(ui.ctx(), ctx, caminho, substituir);
        }
        match confirmar(ui.ctx(), &mut self.confirmacao_substituir) {
            Some(true) => {
                if let Some(caminho) = self.destino_substituir.take() {
                    self.copiar(ui.ctx(), ctx, caminho, true);
                }
            }
            Some(false) => {
                // desistiu de substituir: nada aparece
                self.destino_substituir = None;
            }
            None => {}
        }
        if confirmar(ui.ctx(), &mut self.confirmacao) == Some(true) {
            self.manutencao(ui.ctx(), ctx, AcaoManutencao::Vacuum);
        }
        match self.pedido_do_menu.take() {
            Some(PedidoDoMenu::Manutencao(AcaoManutencao::Vacuum)) if self.execucao.is_none() => {
                self.confirmacao =
                    Some(Confirmacao::new(PERGUNTA_VACUUM).titulo("Compactar (VACUUM)").rotulo("Compactar"));
                ui.ctx().request_repaint();
            }
            Some(PedidoDoMenu::Manutencao(acao)) if self.execucao.is_none() => self.manutencao(ui.ctx(), ctx, acao),
            Some(PedidoDoMenu::Copia) if self.execucao.is_none() => {
                if let Some(b) = &ctx.estado.banco {
                    ctx.emitir(Acao::EscolherCopia { nome_sugerido: format!("{} (cópia).db", nome_base(&b.nome)) });
                }
            }
            _ => {}
        }
    }

    /// O conteúdo do documento: a dock e a lista de Tabelas.
    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.antes_do_conteudo(ui, ctx);
        if ctx.estado.banco.is_none() {
            return;
        }
        self.atualizar_textos(ctx);
        let Some(textos) = self.textos.as_ref() else { return };
        let estado = ctx.estado;

        dock(ui, &textos.dock);
        ui.add_space(ESPACO_BLOCOS);

        // as tabelas (as que a busca da barra acha)
        titulo_lista(ui, "Tabelas");
        ui.add_space(ESPACO_TITULO_LISTA);
        let tabelas = estado.objetos.as_ref().map(|o| o.tabelas.as_slice()).unwrap_or_default();
        let termo = sem_acento(self.busca.trim());
        let visiveis: Vec<usize> = (0..tabelas.len())
            .filter(|&i| termo.is_empty() || textos.chaves_tabelas.get(i).is_some_and(|c| c.contains(&termo)))
            .collect();
        let mut ir: Option<Rota> = None;
        moldura_lista(ui, |ui| {
            if visiveis.is_empty() {
                let texto_vazio = if tabelas.is_empty() { VAZIO_TABELAS } else { VAZIO_BUSCA_TABELAS };
                vazio(ui, texto_vazio, false);
                return;
            }
            let fixas = ui.available_width() >= LARGURAS_LISTA.iter().sum::<f32>() + NOME_MINIMO;
            let largura = |c: ColunaTabela<'static>, i: usize| if fixas { c.fixa(LARGURAS_LISTA[i]) } else { c };
            let colunas = [
                ColunaTabela::new("Nome"),
                largura(ColunaTabela::new("Linhas").direita(), 0),
                largura(ColunaTabela::new("Colunas").direita(), 1),
                largura(ColunaTabela::new("Tipo"), 2),
                largura(ColunaTabela::new("").direita(), 3),
            ];
            Tabela::new("tabelas-banco", &colunas, visiveis.len()).mostrar(ui, |ui, l, coluna| {
                let Some(&l) = visiveis.get(l) else { return };
                let Some(t) = tabelas.get(l) else { return };
                let e = &paleta(ui).estudio;
                match coluna {
                    0 => {
                        let (r, _) = ui.allocate_exact_size(vec2(LADO_FICHA, LADO_FICHA), Sense::hover());
                        icones::pintar(ui.painter(), r, icone_da_tabela(t.tipo, t.sem_rowid), e.t2);
                        ui.add_space(ESPACO_FICHA - ui.spacing().item_spacing.x);
                        if nome_abre(ui, &t.nome, false).on_hover_text(&t.nome).clicked() {
                            ir = Some(Rota::objeto(t.nome.clone(), false));
                        }
                    }
                    1 => {
                        texto(ui, textos.linhas_tabela.get(l).map_or("—", String::as_str), 12.0, e.t);
                    }
                    2 => {
                        texto(ui, textos.colunas_tabela.get(l).map_or("—", String::as_str), 12.0, e.t);
                    }
                    3 => {
                        if t.tipo == TipoObjeto::Virtual {
                            pill(ui, "virtual", TipoPill::Mute);
                        }
                        if t.sem_rowid {
                            pill(ui, "sem rowid", TipoPill::Mute);
                        }
                    }
                    _ => {
                        if ui.add(Botao::ghost("Colunas").pequeno()).clicked() {
                            ir = Some(Rota::objeto_na_aba(t.nome.clone(), false, Aba::Estrutura));
                        }
                    }
                }
            });
        });
        if let Some(rota) = ir {
            ctx.navegar(rota);
        }
    }

    /// As seções "Arquivo" e "Manutenção" (no painel ou no fim da página).
    fn secoes(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let Some(b) = ctx.estado.banco.as_ref() else { return };
        self.atualizar_textos(ctx);
        let leitura = b.somente_leitura;
        let TelaBanco { textos, execucao, confirmacao, .. } = self;
        let Some(t) = textos.as_ref() else { return };

        let mut copiar_caminho = false;
        let _ = Secao::new("propriedades::arquivo", "Arquivo").primeira(true).mostrar(ui, |ui| {
            ficha(ui, |d| {
                d.linha("Caminho", |ui| copiar_caminho = linha_do_caminho(ui, &t.caminho));
                crate::casca::status::linha_da_gravacao(d, b);
                d.linha_texto("Tamanho", Some(&t.tamanho_arquivo), "—");
                d.linha_texto("Modificado em", Some(&t.modificado), "—");
                d.linha_texto("Versão do SQLite", Some(&b.sqlite_versao), "—");
                d.linha_texto("Codificação", Some(&b.codificacao), "—");
                d.linha_texto("Tamanho de página", Some(&t.tamanho_pagina), "—");
                d.linha_texto("Modo de diário", Some(&b.modo_diario), "—");
                d.linha_texto("Versão do usuário", Some(&t.versao_usuario), "—");
                d.linha("Chaves estrangeiras", |ui| {
                    if b.chaves_estrangeiras {
                        pill(ui, "verificadas", TipoPill::Ok)
                    } else {
                        pill(ui, "desligadas", TipoPill::Mute)
                    }
                });
            });
        });
        if copiar_caminho {
            ui.ctx().copy_text(t.caminho.clone());
            toast_concluido(ui.ctx(), MSG_CAMINHO_COPIADO);
        }

        let mut clicou: Option<AcaoManutencao> = None;
        let mut copia = false;
        let ocupado = execucao.is_some();
        let _ = Secao::new("propriedades::manutencao", "Manutenção").mostrar(ui, |ui| {
            com_recuo(ui, RECUO_MANUTENCAO, |ui| {
                ui.spacing_mut().item_spacing.y = ESPACO_MANUTENCAO;
                for a in &ACOES {
                    if a.escreve && leitura {
                        continue;
                    }
                    let botao = Botao::new(a.rotulo).lista().com_icone(a.icone).habilitado(!ocupado);
                    if ui.add(botao).clicked() {
                        clicou = Some(a.acao);
                    }
                }
                let botao = Botao::new(ROTULO_COPIA).lista().com_icone(Icone::CopiaSeguranca).habilitado(!ocupado);
                if ui.add(botao).clicked() {
                    copia = true;
                }
                let texto_execucao = match execucao {
                    Some(Execucao::Manutencao(AcaoManutencao::Vacuum, _)) => Some("Compactando…"),
                    Some(Execucao::Manutencao(..)) => Some("Executando…"),
                    Some(Execucao::Copia { .. }) => Some("Copiando…"),
                    None => None,
                };
                // o resultado não aparece aqui: vem num toast, também com a aba escondida
                if let Some(texto) = texto_execucao {
                    ui.add_space(8.0);
                    carregando(ui, texto);
                }
            });
        });

        if let Some(acao) = clicou {
            if acao == AcaoManutencao::Vacuum {
                *confirmacao = Some(Confirmacao::new(PERGUNTA_VACUUM).titulo("Compactar (VACUUM)").rotulo("Compactar"));
            } else {
                *execucao = Some(Execucao::Manutencao(acao, ctx.tarefa(ui.ctx(), move |s, id| s.manutencao(id, acao))));
            }
            ui.ctx().request_repaint();
        }
        if copia {
            // o diálogo é da casca, que devolve o caminho por `copiar_para`
            ctx.emitir(Acao::EscolherCopia { nome_sugerido: format!("{} (cópia).db", nome_base(&b.nome)) });
        }
    }
}

/// Um `Ui` com o recuo `[cima, lados, baixo]` (o `padding` de um bloco das Propriedades).
fn com_recuo(ui: &mut Ui, [cima, lados, baixo]: [f32; 3], conteudo: impl FnOnce(&mut Ui)) {
    let topo = ui.cursor().min;
    let r = Rect::from_min_max(topo + vec2(lados, cima), pos2(ui.max_rect().right() - lados, f32::INFINITY));
    let mut dentro = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
    conteudo(&mut dentro);
    let altura = dentro.min_rect().height().max(0.0) + cima + baixo;
    ui.allocate_space(vec2(ui.available_width(), altura));
}

/// O Caminho das Propriedades numa linha, com a reticência no meio quando não cabe (o `dd.caminho`: `.ini` cortado,
/// `.fim` inteiro; o caminho inteiro na dica e para o leitor de tela), e o `.ib.pq` Copiar caminho. Devolve se ele
/// foi clicado. A ficha do arquivo de dados usa a mesma.
pub(crate) fn linha_do_caminho(ui: &mut Ui, caminho: &str) -> bool {
    let e = &paleta(ui).estudio;
    let lado = interface::pecas::botao::LADO_PEQUENO;
    let largura_valor = ui.available_width();
    let largura = (largura_valor - lado - ESPACO_COPIAR).max(0.0);
    let g = galley_caminho(ui, caminho, fontes::fonte(12.0, Peso::Leve), e.t, largura);
    let encurtado = g.text() != caminho;
    // a linha de 17pt das outras da ficha; o botão de 26 centrado nela, logo depois do caminho, sem a esticar
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x, ALTURA_LINHA_FICHA), Sense::hover());
    ui.painter().galley(pos2(rect.left(), y_na_linha(rect.top(), ALTURA_LINHA_FICHA, &g)), Arc::clone(&g), e.t);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, caminho));
    if encurtado {
        resp.on_hover_text(caminho);
    }
    let centro = pos2(rect.right() + ESPACO_COPIAR + lado / 2.0, rect.center().y);
    let mut ui_botao = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(centro, vec2(lado, lado))));
    ui_botao.add(Botao::icone(Icone::CopiarCaminho).pequeno().dica("Copiar caminho")).clicked()
}

/// A faixa: o cilindro, o nome, a pílula "só leitura" e o caminho com a reticência no meio.
fn faixa_do_banco(ui: &mut Ui, b: &BancoInfo, caminho: &str) {
    let e = &paleta(ui).estudio;
    let (r, _) = ui.allocate_exact_size(vec2(LADO_BANCO_FAIXA, LADO_BANCO_FAIXA), Sense::hover());
    icones::pintar(ui.painter(), r, icone_do_banco(b), e.t2);
    let g = galley(ui, &b.nome, fontes::fonte(13.0, Peso::Medio), e.t);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, e.t);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &b.nome));
    if b.somente_leitura {
        let r = pill(ui, "só leitura", TipoPill::Wait);
        if let Some(motivo) = &b.motivo_somente_leitura {
            r.on_hover_text(motivo);
        }
    }
    let g = galley_caminho(ui, caminho, fontes::fonte(12.0, Peso::Leve), e.t3, ui.available_width());
    let encurtado = g.text() != caminho;
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, e.t3);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, caminho));
    if encurtado {
        resp.on_hover_text(caminho);
    }
}

/// A Visão geral como documento da casca (veja o topo do módulo).
impl estado::Documento for TelaBanco {
    fn anatomia(&self) -> Anatomia {
        Anatomia::default().com_faixa()
    }

    fn faixa(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        let Some(b) = ctx.estado.banco.as_ref() else { return };
        self.atualizar_textos(ctx);
        let caminho = self.textos.as_ref().map_or_else(|| b.caminho.display().to_string(), |t| t.caminho.clone());
        faixa_do_banco(ui, b, &caminho);
    }

    fn conteudo(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.ui(ui, ctx);
    }

    fn ofertas(&self, estado: &estado::Estado) -> Ofertas {
        let objetos = estado.objetos.as_ref().is_some_and(|o| !o.tabelas.is_empty() || !o.visoes.is_empty());
        Ofertas::default()
            .com(Verbo::Novo, Oferta::ligada().ligada_se(!estado.somente_leitura()).com_rotulo("Nova tabela"))
            .com(Verbo::Atualizar, Oferta::ligada().ligada_se(!TelaBanco::ocupada(self)))
            .com(Verbo::Exportar, Oferta::ligada().ligada_se(objetos).com_dica(DICA_EXPORTAR_BANCO))
            .ligar(Verbo::Buscar)
    }

    fn executar(&mut self, verbo: Verbo, _egui: &egui::Context, ctx: &mut Contexto) {
        match verbo {
            Verbo::Novo => ctx.emitir(Acao::AbrirNovaTabela),
            // a casca relê o BancoInfo e os objetos (a lista do Explorador também), sem refazer a tela: a busca, o
            // resultado da manutenção e o que está à vista ficam até a leitura nova chegar
            Verbo::Atualizar => {
                self.iniciada = true;
                ctx.emitir(Acao::RecarregarBanco);
                ctx.emitir(Acao::RecarregarObjetos);
            }
            Verbo::Exportar => {
                if let Some(b) = ctx.estado.banco.as_ref() {
                    ctx.emitir(Acao::AbrirExportacao(pedido_exportacao(b)));
                }
            }
            _ => {}
        }
    }

    fn tem_propriedades(&self) -> bool {
        true
    }

    /// "Arquivo" e "Manutenção".
    fn propriedades(&mut self, ui: &mut Ui, ctx: &mut Contexto) {
        self.secoes(ui, ctx);
    }

    fn ocupada(&self) -> bool {
        TelaBanco::ocupada(self)
    }

    /// Com a aba fora de vista, a manutenção ou a cópia que estava correndo termina do mesmo jeito: o resultado é o
    /// toast de sempre, na hora (ele nunca foi um aviso do painel). A cópia que a casca devolveu com a aba escondida
    /// também começa.
    fn em_segundo_plano(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        if let Some((caminho, substituir)) = self.copia_pedida.take() {
            if self.execucao.is_none() {
                self.copiar(egui, ctx, caminho, substituir);
            } else {
                self.copia_pedida = Some((caminho, substituir));
            }
        }
        if self.execucao.is_some() {
            self.receber(egui, ctx);
        }
    }

    /// "Buscar tabelas": a lista filtra na hora, a cada tecla.
    fn busca(&mut self) -> Option<estado::BuscaDaBarra<'_>> {
        Some(estado::BuscaDaBarra { texto: &mut self.busca, dica: DICA_BUSCA.into() })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pergunta_antes_de_substituir() {
        assert_eq!(
            pergunta_substituir(Some(r"C:\dados\copia.db")),
            "Já existe copia.db. Substituir? O conteúdo atual do arquivo se perde."
        );
        assert_eq!(pergunta_substituir(None), "Já existe o arquivo. Substituir? O conteúdo atual do arquivo se perde.");
    }

    #[test]
    fn de_para_na_mesma_escala() {
        let escala = escala_de(1_258_291, 943_718);
        assert_eq!(escala, Some(UnidadeBytes::Mb));
        assert_eq!(bytes_na_escala(1_258_291, escala), "1,2 MB");
        assert_eq!(bytes_na_escala(943_718, escala), "0,9 MB");
        assert_eq!(escala_de(500, 300), None);
        assert_eq!(bytes_na_escala(500, None), "500 bytes");
    }

    #[test]
    fn contadores_no_singular_com_um() {
        assert_eq!(singular_ou_plural(1, "Visão", "Visões"), "Visão");
        assert_eq!(singular_ou_plural(0, "Visão", "Visões"), "Visões");
        assert_eq!(singular_ou_plural(7, "Tabela", "Tabelas"), "Tabelas");
    }
}
