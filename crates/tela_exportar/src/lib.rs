//! Janela Exportar: Excel, CSV ou JSON de uma tabela (inteira, só a busca e
//! os filtros, ou só as linhas marcadas), do resultado de um comando da Consulta SQL ou do banco inteiro, com
//! a barra de progresso e o "Interromper".
//!
//! # API (o contrato com a casca, §4.8 de ARQUITETURA.md)
//!
//! ```text
//! JanelaExportar::nova(pedido: PedidoJanelaExportar) -> JanelaExportar
//! JanelaExportar::mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto) -> bool   // false = fechou
//! JanelaExportar::exportando(&self) -> bool       // a casca pergunta antes de fechar o app no meio de uma exportação
//! ```
//!
//! Quem abre a janela (o Exportar da barra de ferramentas na tabela, na Consulta SQL e na Visão geral) emite
//! `Acao::AbrirExportacao(pedido)`; a casca
//! guarda a janela e chama `mostrar` a cada quadro até ele devolver `false`. Soltar a janela no meio de uma
//! exportação a interrompe: sem a janela não há como acompanhá-la.
//!
//! Também públicas: [`vista_progresso`]`(Option<&ProgressoInfo>, banco) -> `[`VistaProgresso`]` { fracao, tabela,
//! texto }` (os textos do bloco de progresso) e as frases [`SEM_TABELAS`],
//! [`MSG_INTERROMPIDA`] e [`PERGUNTA_FECHAR`].
//!
//! Para os testes (`#[doc(hidden)]`): [`JanelaExportar::com_dialogo_de_teste`] troca o diálogo de salvar do
//! Windows por uma função `(nome sugerido, formato) -> Option<(caminho, substituir)>` (`None` = a pessoa
//! cancelou; `substituir: false` simula um caminho que o diálogo não confirmou), e
//! [`JanelaExportar::progresso`] lê o andamento da exportação em curso.
//!
//! # O que a janela faz
//!
//! - Título "Exportar clientes" / "Exportar o banco" / "Exportar o resultado", com o sólido de 20 antes dele (a ficha da
//!   tabela, o cilindro do banco, a folha SQL do resultado; `Janela::icone`).
//! - **Formato**: o seletor segmentado Excel (.xlsx) · CSV · JSON (começa em Excel), cada parte com a folha do
//!   arquivo de 16 antes do rótulo (`ParteSeletor::icone`).
//! - **O que exportar** (tabela): "A tabela inteira (60 linhas)" · "Só o que a busca e os filtros mostram (12
//!   linhas)" (com busca ou filtro; sem o número quando a aba Dados ainda não contou) · "Só as linhas marcadas
//!   (3)" (com linhas marcadas); com busca ou filtro, a segunda vem escolhida. Banco: "Tabelas" com todas as
//!   tabelas de `estado.objetos` marcadas, em duas colunas (a ficha de 16 pelo tipo de cada uma entre a caixa e o
//!   nome), e "Marcar todas"/"Desmarcar todas" (`Botao::ghost` pequeno) à direita do título.
//! - **Opções** com o "i" do formato; CSV: Separador, Decimal, "Primeira linha com os nomes das colunas",
//!   "Proteger contra fórmulas do Excel"; JSON: "Indentado (mais fácil de ler)"; Excel: nenhuma. Os padrões vêm
//!   de `estado.prefs.exportacao` (`cabecalho` e `csv_proteger_formulas` nunca escolhidos valem marcados).
//! - Banco sem tabelas: só o vazio "O banco não tem tabelas para exportar. Crie uma com Nova tabela, na barra de
//!   ferramentas, ou pela Consulta SQL.", sem formato e sem botão.
//! - Rodapé: "Cancelar" (`Botao::ghost`: fecha sem exportar) e "Exportar" (`Botao::primario`, sem reticência) →
//!   diálogo de salvar (`estado::Dialogos::salvar_como` com o nome sugerido e a extensão do
//!   formato; o banco inteiro em CSV é um `.zip`) → `servico.exportar` numa tarefa de fundo, com um
//!   `Progresso::novo()` por pedido e `substituir: true` (o diálogo do Windows já perguntou).
//! - **Durante**: as opções ficam inertes e com 45% de opacidade (o `opacity:.45` da prancheta 5); o rodapé ganha à esquerda "Interromper"
//!   (perigo; vira "Interrompendo") e o rodapé da direita vira só o "Gerando arquivo" (desligado, com o arco
//!   girando); no fim do corpo, a barra (com total ou indeterminada) e os textos, lidos do `Progresso` a cada
//!   250 ms ("Preparando a exportação…", "Contando as linhas…", "12.340 de 1.000.000 linhas · 1%", "12.340
//!   linhas", "1.000.000 linhas · fechando o arquivo…"); no banco inteiro, "Tabela 3 de 7 · pedidos" acima da
//!   barra.
//! - Fechar no meio (✕, Esc, fundo) pergunta "Interromper a exportação? Nenhum arquivo é gravado." (título
//!   "Interromper a exportação", botão perigo "Interromper"): sim interrompe e fecha; desistir deixa a
//!   exportação correndo. Sem exportação, fecha sem perguntar.
//! - **Resultado**: "Arquivo salvo: **clientes.xlsx** · 60 linhas · 12 KB" (com "N tabelas" no banco inteiro),
//!   a lista dos avisos do formato e "Mostrar na pasta" / "Abrir o arquivo" (`servico.revelar` /
//!   `servico.abrir_arquivo` em tarefa; erro no toast); `Acao::RecarregarPrefs` (as opções viraram o padrão).
//!   Interrompida: "Exportação interrompida; nenhum arquivo foi gravado." e a janela volta ao estado inicial.
//!   Erro: o aviso de perigo. Um destino que já existe e não foi confirmado (`Codigo::DestinoExiste`) pergunta
//!   "Já existe x.csv. Substituir? O conteúdo atual do arquivo se perde." e refaz com `substituir`.
//!
//! # Detalhes
//!
//! - O andamento é lido do `Progresso` a cada 250 ms.
//! - "Primeira linha com os nomes das colunas" nasce com o `cabecalho` das preferências.
//! - Na tabela, a ordem da aba Dados vai junto em todo escopo (inteira, busca e filtros, marcadas) sempre que o
//!   pedido traz um `FiltroAtual`, mesmo só com a ordem (`busca` None e `filtros` vazio) (o `TabelaAberta::pedido_exportacao` do `estado` monta o `FiltroAtual` também
//!   quando só há a ordem); a opção "Só o que a busca…" aparece só com busca ou filtro.
//! - Com o 409 `DestinoExiste`, a barra some antes da pergunta "Já existe…". Com o diálogo do Windows (`substituir: true`) isso não acontece.
//!
//! # Testes
//!
//! `src/progresso.rs` e `src/lib.rs` (textos do progresso, rótulos, origem conforme o escolhido, o aviso do
//! arquivo salvo) e `tests/exportar.rs`, no `egui_kittest` com o banco de exemplo e o diálogo trocado pelo
//! gancho de teste (a lentidão vem de uma tabela
//! de um milhão de linhas exportada para o Excel; os testes com ela rodam um de cada vez, porque juntos deixam a
//! máquina tão carregada que a contagem do banco inteiro passa do prazo de 2 s). Capturas em `capturas/rust/`:
//! `exportar-tabela-claro.png`, `exportar-tabela-csv-claro.png`, `exportar-banco-claro.png` (com referência),
//! `exportar-tabela-json-claro.png`, `exportar-banco-marque-claro.png`, `exportar-banco-vazio-claro.png`,
//! `exportar-salvo-claro.png`, `exportar-avisos-claro.png`, `exportar-erro-claro.png`,
//! `exportar-progresso-claro.png`, `exportar-progresso-escuro.png`, `exportar-interrompida-claro.png`,
//! `exportar-banco-progresso-claro.png` e `exportar-resultado-progresso-claro.png`.

#![forbid(unsafe_code)]

mod corpo;
mod progresso;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use comum::Codigo;
use estado::{
    Acao, Contexto, Dialogos, Estado, FiltroAtual, FormatoArquivo, OrigemJanelaExportar, PedidoJanelaExportar,
    Promessa, Reacao,
};
use exportar::{Formato, Opcoes};
use interface::formato::{formatar_bytes, formatar_numero, nome_do_caminho, plural};
use interface::pecas::avisos::vazio;
use interface::pecas::botao::Botao;
use interface::pecas::janela::{Confirmacao, Janela, confirmar};
use interface::pecas::toast::toast_erro;
use servico::{FiltroExportacao, OrigemExportacao, PedidoExportacao, Progresso, ProgressoInfo, ResumoExportacao};

use corpo::{AvisoJanela, Escopo, Eventos, Formulario, ItemTabela, Tipo};
pub use progresso::{VistaProgresso, vista_progresso};

pub const SEM_TABELAS: &str =
    "O banco não tem tabelas para exportar. Crie uma com Nova tabela, na barra de ferramentas, ou pela Consulta SQL.";
pub const MSG_INTERROMPIDA: &str = "Exportação interrompida; nenhum arquivo foi gravado.";
pub const PERGUNTA_FECHAR: &str = "Interromper a exportação? Nenhum arquivo é gravado.";
const MSG_MARQUE: &str = "Marque ao menos uma tabela para exportar.";
/// De quanto em quanto tempo o andamento é lido.
const INTERVALO_PROGRESSO: f64 = 0.25;

/// O diálogo de salvar trocado nos testes.
type DialogoTeste = Box<dyn FnMut(&str, FormatoArquivo) -> Option<(PathBuf, bool)>>;

/// A exportação em andamento.
struct Exportacao {
    promessa: Promessa<ResumoExportacao>,
    progresso: Arc<Progresso>,
    /// O pedido (para refazer com `substituir` depois do 409 `DestinoExiste`).
    pedido: PedidoExportacao,
    lido_em: f64,
    vista: VistaProgresso,
    cancelando: bool,
}

/// O que a confirmação aberta pergunta.
enum Pergunta {
    Fechar,
    Substituir(PedidoExportacao),
}

/// A janela Exportar.
pub struct JanelaExportar {
    pedido: PedidoJanelaExportar,
    titulo: String,
    form: Formulario,
    /// As preferências e a lista de tabelas já foram lidas do estado.
    preparada: bool,
    sem_tabelas: bool,
    em_curso: Option<Exportacao>,
    aviso: Option<AvisoJanela>,
    /// O arquivo salvo (para "Mostrar na pasta" e "Abrir o arquivo").
    salvo: Option<PathBuf>,
    confirmacao: Option<Confirmacao>,
    pergunta: Option<Pergunta>,
    acao_arquivo: Option<Promessa<()>>,
    dialogo_teste: Option<DialogoTeste>,
}

impl JanelaExportar {
    pub fn nova(pedido: PedidoJanelaExportar) -> JanelaExportar {
        let (tipo, titulo) = match &pedido.origem {
            OrigemJanelaExportar::Tabela { nome, .. } => (Tipo::Tabela, format!("Exportar {nome}")),
            OrigemJanelaExportar::Consulta { .. } => (Tipo::Consulta, "Exportar o resultado".to_string()),
            OrigemJanelaExportar::Banco => (Tipo::Banco, "Exportar o banco".to_string()),
        };
        let mut escopo = Escopo::Tudo;
        let (mut rotulo_filtradas, mut rotulo_marcadas) = (None, None);
        let mut rotulo_tudo = "A tabela inteira".to_string();
        if let OrigemJanelaExportar::Tabela { total, filtro, marcadas, .. } = &pedido.origem {
            rotulo_tudo = com_contagem("A tabela inteira", *total);
            // um `FiltroAtual` só com a ordem (sem busca nem filtro) não oferece a opção da busca
            if let Some(f) = filtro.as_ref().filter(|f| tem_busca_ou_filtro(f)) {
                escopo = Escopo::Filtradas;
                rotulo_filtradas = Some(com_contagem("Só o que a busca e os filtros mostram", f.linhas));
            }
            if !marcadas.is_empty() {
                rotulo_marcadas = Some(format!("Só as linhas marcadas ({})", formatar_numero(marcadas.len())));
            }
        }
        let padrao = Opcoes::default();
        JanelaExportar {
            pedido,
            titulo,
            form: Formulario {
                tipo,
                formato: Formato::Xlsx,
                escopo,
                tabelas: Vec::new(),
                separador: padrao.csv_separador,
                decimal: padrao.csv_decimal,
                cabecalho: padrao.cabecalho,
                proteger_formulas: padrao.csv_proteger_formulas,
                indentado: padrao.json_indentado,
                rotulo_tudo,
                rotulo_filtradas,
                rotulo_marcadas,
            },
            preparada: false,
            sem_tabelas: false,
            em_curso: None,
            aviso: None,
            salvo: None,
            confirmacao: None,
            pergunta: None,
            acao_arquivo: None,
            dialogo_teste: None,
        }
    }

    /// Troca o diálogo de salvar do Windows por `f(nome sugerido, formato) -> Some((caminho, substituir))` ou
    /// `None` (cancelou). Só para testes.
    #[doc(hidden)]
    pub fn com_dialogo_de_teste(
        mut self,
        f: impl FnMut(&str, FormatoArquivo) -> Option<(PathBuf, bool)> + 'static,
    ) -> JanelaExportar {
        self.dialogo_teste = Some(Box::new(f));
        self
    }

    /// O andamento da exportação em curso (só para testes).
    #[doc(hidden)]
    pub fn progresso(&self) -> Option<ProgressoInfo> {
        self.em_curso.as_ref().map(|e| e.progresso.instantaneo())
    }

    /// Há uma exportação em andamento.
    pub fn exportando(&self) -> bool {
        self.em_curso.is_some()
    }

    /// Desenha a janela; false quando ela fechou.
    pub fn mostrar(&mut self, egui: &egui::Context, ctx: &mut Contexto) -> bool {
        if !self.preparada {
            self.preparar(ctx);
        }
        self.receber(egui, ctx);
        self.ler_progresso(egui);
        let icone = self.form.tipo.icone();
        if self.sem_tabelas {
            let r = Janela::new("exportar", &self.titulo).icone(icone).mostrar(egui, |ui| {
                vazio(ui, SEM_TABELAS, false);
            });
            return !r.pediu_fechar;
        }

        let exportando = self.em_curso.is_some();
        let cancelando = self.em_curso.as_ref().is_some_and(|e| e.cancelando);
        let (mut exportar, mut interromper, mut cancelar_janela) = (false, false, false);
        let mut eventos = Eventos::default();
        let mut janela = Janela::new("exportar", &self.titulo).icone(icone);
        if exportando {
            // a ação destrutiva fica à esquerda do rodapé (§4.9)
            janela = janela.rodape_esquerda(|ui| {
                let rotulo = if cancelando { "Interrompendo" } else { "Interromper" };
                interromper = ui.add(Botao::perigo(rotulo).habilitado(!cancelando)).clicked();
            });
        }
        let form = &mut self.form;
        let aviso = self.aviso.as_ref();
        let vista = self.em_curso.as_ref().map(|e| &e.vista);
        let r = janela
            .rodape_direita(|ui| {
                // exportando, só o "Gerando arquivo" (desligado, com o arco girando); o "Interromper" fica à esquerda
                if exportando {
                    ui.add(Botao::primario("Gerando arquivo").ocupado(true).com_arco(true).habilitado(false));
                } else {
                    cancelar_janela = ui.add(Botao::ghost("Cancelar")).clicked();
                    exportar = ui.add(Botao::primario("Exportar")).clicked();
                }
            })
            .mostrar(egui, |ui| corpo::desenhar(ui, form, aviso, vista, &mut eventos));

        // marcar uma tabela tira o aviso
        if eventos.marcou {
            self.aviso = None;
        }
        if eventos.mostrar_na_pasta || eventos.abrir_arquivo {
            self.acao_no_arquivo(egui, ctx, eventos.abrir_arquivo);
        }
        if interromper {
            self.cancelar();
        }
        if exportar && !exportando {
            self.enviar(egui, ctx);
        }
        // "Cancelar" só existe sem exportação em curso: fecha sem exportar nada, como o ✕ e o Esc
        if (r.pediu_fechar || cancelar_janela) && self.confirmacao.is_none() {
            if self.em_curso.is_none() {
                return false;
            }
            self.perguntar(
                Confirmacao::new(PERGUNTA_FECHAR).titulo("Interromper a exportação").rotulo("Interromper").perigo(true),
                Pergunta::Fechar,
            );
        }
        if let Some(sim) = confirmar(egui, &mut self.confirmacao) {
            match self.pergunta.take() {
                Some(Pergunta::Fechar) if sim => {
                    // a exportação pode ter terminado enquanto a pergunta estava aberta: aí só fecha
                    self.cancelar();
                    return false;
                }
                Some(Pergunta::Substituir(pedido)) if sim => {
                    self.comecar(egui, ctx, PedidoExportacao { substituir: true, ..pedido });
                }
                _ => {}
            }
        }
        true
    }

    fn perguntar(&mut self, confirmacao: Confirmacao, pergunta: Pergunta) {
        self.confirmacao = Some(confirmacao);
        self.pergunta = Some(pergunta);
    }

    /// Lê do estado o que a janela precisa (uma vez): as opções das preferências e as tabelas do banco.
    fn preparar(&mut self, ctx: &Contexto) {
        self.preparada = true;
        let prefs = &ctx.estado.prefs.exportacao;
        self.form.separador = prefs.csv_separador;
        self.form.decimal = prefs.csv_decimal;
        self.form.indentado = prefs.json_indentado;
        self.form.cabecalho = prefs.cabecalho.unwrap_or(true);
        self.form.proteger_formulas = prefs.csv_proteger_formulas.unwrap_or(true);
        if self.form.tipo == Tipo::Banco {
            let tabelas = ctx.estado.objetos.as_ref().map(|o| o.tabelas.as_slice()).unwrap_or_default();
            self.form.tabelas = tabelas
                .iter()
                .map(|t| ItemTabela::novo(t.nome.clone(), estado::icone_da_tabela(t.tipo, t.sem_rowid)))
                .collect();
            // banco sem tabelas: não há o que escolher nem o que gerar; a janela só explica
            self.sem_tabelas = self.form.tabelas.is_empty();
        }
    }

    /// Toma as respostas das tarefas que terminaram.
    fn receber(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        if let Some(r) = self.acao_arquivo.as_mut().and_then(Promessa::tomar) {
            self.acao_arquivo = None;
            if let Err(e) = r
                && ctx.reagir(&e) == Reacao::Mostrar
            {
                toast_erro(egui, e.mensagem);
            }
        }
        let Some(r) = self.em_curso.as_mut().and_then(|e| e.promessa.tomar()) else { return };
        let Some(exportacao) = self.em_curso.take() else { return };
        match r {
            Ok(resumo) => {
                self.aviso = Some(aviso_salvo(&resumo, self.form.tipo));
                self.salvo = Some(resumo.caminho);
                // as opções usadas viraram o padrão das preferências
                ctx.emitir(Acao::RecarregarPrefs);
            }
            Err(e) if e.tem_codigo(Codigo::Cancelado) => {
                self.aviso = Some(AvisoJanela::Texto { texto: MSG_INTERROMPIDA.to_string(), perigo: false });
            }
            Err(e) if e.tem_codigo(Codigo::DestinoExiste) && !exportacao.pedido.substituir => {
                // arquivo que já existe e que o diálogo não confirmou: pergunta (uma vez) e refaz com substituir.
                // Com `substituir` o 409 não é um "posso substituir?" (o .zip do banco inteiro em CSV que já existe
                // ao lado do .csv escolhido): ele vira o aviso de perigo.
                let nome = e
                    .detalhe
                    .as_deref()
                    .map(nome_do_caminho)
                    .filter(|n| !n.is_empty())
                    .unwrap_or("o arquivo")
                    .to_string();
                self.perguntar(
                    Confirmacao::new(format!("Já existe {nome}. Substituir? O conteúdo atual do arquivo se perde."))
                        .titulo("Substituir o arquivo")
                        .rotulo("Substituir")
                        .perigo(true),
                    Pergunta::Substituir(exportacao.pedido),
                );
            }
            Err(e) => {
                if ctx.reagir(&e) == Reacao::Mostrar {
                    self.aviso = Some(AvisoJanela::Texto { texto: e.mensagem, perigo: true });
                }
            }
        }
    }

    /// Relê o andamento a cada 250 ms (sem repintar sem parar: só um quadro por leitura).
    fn ler_progresso(&mut self, egui: &egui::Context) {
        let banco = self.form.tipo == Tipo::Banco;
        let Some(e) = self.em_curso.as_mut() else { return };
        let agora = egui.input(|i| i.time);
        if agora - e.lido_em >= INTERVALO_PROGRESSO {
            e.lido_em = agora;
            e.vista = vista_progresso(Some(&e.progresso.instantaneo()), banco);
        }
        egui.request_repaint_after(Duration::from_secs_f64(INTERVALO_PROGRESSO));
    }

    /// O alvo da exportação, conforme o que está escolhido.
    fn origem(&self) -> OrigemExportacao {
        match &self.pedido.origem {
            OrigemJanelaExportar::Tabela { nome, filtro, marcadas, .. } => {
                let mut f = FiltroExportacao::default();
                if let Some(atual) = filtro {
                    f.ordem = atual.ordem.clone();
                    f.direcao = atual.direcao;
                }
                match self.form.escopo {
                    Escopo::Filtradas => {
                        if let Some(atual) = filtro {
                            f.busca = atual.busca.clone();
                            f.filtros = atual.filtros.clone();
                        }
                    }
                    Escopo::Marcadas => f.chaves = Some(marcadas.clone()),
                    Escopo::Tudo => {}
                }
                OrigemExportacao::Tabela { nome: nome.clone(), filtro: f }
            }
            OrigemJanelaExportar::Consulta { sql } => OrigemExportacao::Consulta { sql: sql.clone() },
            OrigemJanelaExportar::Banco => OrigemExportacao::Banco {
                tabelas: Some(self.form.tabelas.iter().filter(|t| t.marcado).map(|t| t.nome.clone()).collect()),
            },
        }
    }

    /// O "Exportar": confere, pede o caminho e começa.
    fn enviar(&mut self, egui: &egui::Context, ctx: &mut Contexto) {
        let origem = self.origem();
        if let OrigemExportacao::Banco { tabelas: Some(t) } = &origem
            && t.is_empty()
        {
            self.aviso = Some(AvisoJanela::Texto { texto: MSG_MARQUE.to_string(), perigo: true });
            return;
        }
        let f = &self.form;
        let opcoes = Opcoes {
            cabecalho: f.cabecalho,
            csv_separador: f.separador,
            csv_decimal: f.decimal,
            json_indentado: f.indentado,
            csv_proteger_formulas: f.proteger_formulas,
        };
        let banco = f.tipo == Tipo::Banco;
        let formato = f.formato;
        let formato_arquivo = FormatoArquivo::de_exportacao(formato, banco);
        let extensao = if formato == Formato::Csv && banco { "zip" } else { formato.como_str() };
        let nome = format!("{}.{extensao}", self.pedido.nome_sugerido);
        self.aviso = None;
        let escolha = match self.dialogo_teste.as_mut() {
            Some(dialogo) => dialogo(&nome, formato_arquivo),
            // o diálogo do Windows já pergunta antes de substituir um arquivo que existe
            None => Dialogos::novo(ctx.frame, pasta_do_dialogo(ctx.estado))
                .salvar_como(&nome, formato_arquivo)
                .map(|e| (e.caminho, true)),
        };
        let Some((caminho, substituir)) = escolha else { return };
        self.comecar(egui, ctx, PedidoExportacao { origem, formato, caminho, opcoes, substituir });
    }

    /// Começa a exportação numa tarefa de fundo, com um `Progresso` novo (uma "tarefa" por pedido).
    fn comecar(&mut self, egui: &egui::Context, ctx: &mut Contexto, pedido: PedidoExportacao) {
        let progresso = Progresso::novo();
        let do_servico = Arc::clone(&progresso);
        let para_tarefa = pedido.clone();
        let promessa = ctx.tarefa(egui, move |servico, id| servico.exportar(id, &para_tarefa, Some(do_servico)));
        self.aviso = None;
        self.em_curso = Some(Exportacao {
            promessa,
            progresso,
            pedido,
            lido_em: egui.input(|i| i.time),
            vista: VistaProgresso::default(),
            cancelando: false,
        });
    }

    /// Pede para a exportação parar (o botão vira "Interrompendo…"); a tarefa volta com `Codigo::Cancelado`.
    fn cancelar(&mut self) {
        if let Some(e) = self.em_curso.as_mut()
            && !e.cancelando
        {
            e.cancelando = true;
            e.progresso.cancelar();
        }
    }

    fn acao_no_arquivo(&mut self, egui: &egui::Context, ctx: &Contexto, abrir: bool) {
        let Some(caminho) = self.salvo.clone() else { return };
        self.acao_arquivo =
            Some(ctx.tarefa(
                egui,
                move |servico, _| {
                    if abrir { servico.abrir_arquivo(&caminho) } else { servico.revelar(&caminho) }
                },
            ));
    }
}

impl Drop for JanelaExportar {
    /// Fechada no meio da exportação: sem a janela não há como acompanhá-la, então ela para.
    fn drop(&mut self) {
        if let Some(e) = &self.em_curso {
            e.progresso.cancelar();
        }
    }
}

/// A pasta em que o diálogo de salvar abre: a do banco aberto; sem banco, a última usada (o item 1 dos
/// diálogos na documentação do `servico`). O
/// `Contexto::dialogos()` do estado começa na última pasta usada, que é a regra do diálogo de abrir.
#[doc(hidden)]
pub fn pasta_do_dialogo(estado: &Estado) -> Option<PathBuf> {
    estado.banco.as_ref().map(|b| b.pasta.clone()).or_else(|| estado.ultimo_diretorio.clone())
}

/// A aba Dados tem busca ou filtro; sem eles, o `FiltroAtual` só leva a ordem.
fn tem_busca_ou_filtro(f: &FiltroAtual) -> bool {
    f.busca.as_deref().is_some_and(|b| !b.is_empty()) || !f.filtros.is_empty()
}

/// "A tabela inteira (60 linhas)"; sem contagem, só o texto.
fn com_contagem(texto: &str, n: Option<u64>) -> String {
    match n {
        Some(n) => format!("{texto} ({})", plural(n, "linha", "linhas")),
        None => texto.to_string(),
    }
}

/// "Arquivo salvo: **clientes.xlsx** · 60 linhas · 12 KB" (com as tabelas no banco inteiro).
fn aviso_salvo(r: &ResumoExportacao, tipo: Tipo) -> AvisoJanela {
    let mut detalhes = Vec::with_capacity(3);
    if tipo == Tipo::Banco || r.tabelas > 1 {
        detalhes.push(plural(r.tabelas, "tabela", "tabelas"));
    }
    detalhes.push(plural(r.linhas, "linha", "linhas"));
    detalhes.push(formatar_bytes(r.bytes));
    AvisoJanela::Salvo { nome: r.nome.clone(), detalhes: detalhes.join(" · "), avisos: r.avisos.clone() }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido_tabela(filtro: bool, marcadas: usize, linhas_filtro: Option<u64>) -> PedidoJanelaExportar {
        PedidoJanelaExportar {
            origem: OrigemJanelaExportar::Tabela {
                nome: "clientes".into(),
                total: Some(60),
                filtro: filtro.then(|| FiltroAtual {
                    busca: Some("ana".into()),
                    filtros: vec![],
                    ordem: Some("nome".into()),
                    direcao: banco::Direcao::Desc,
                    linhas: linhas_filtro,
                }),
                marcadas: (0..marcadas as i64).map(comum::Chave::Rowid).collect(),
            },
            nome_sugerido: "clientes".into(),
        }
    }

    #[test]
    fn rotulos_e_escopo_da_tabela() {
        let j = JanelaExportar::nova(pedido_tabela(false, 3, None));
        assert_eq!(j.titulo, "Exportar clientes");
        assert_eq!(j.form.rotulo_tudo, "A tabela inteira (60 linhas)");
        assert_eq!(j.form.rotulo_filtradas, None);
        assert_eq!(j.form.rotulo_marcadas.as_deref(), Some("Só as linhas marcadas (3)"));
        assert_eq!(j.form.escopo, Escopo::Tudo);

        let j = JanelaExportar::nova(pedido_tabela(true, 0, Some(12)));
        assert_eq!(j.form.rotulo_filtradas.as_deref(), Some("Só o que a busca e os filtros mostram (12 linhas)"));
        assert_eq!(j.form.escopo, Escopo::Filtradas, "com busca, a opção da busca vem escolhida");
        // numa tabela grande ainda sem a contagem, a opção fica sem o número
        let j = JanelaExportar::nova(pedido_tabela(true, 0, None));
        assert_eq!(j.form.rotulo_filtradas.as_deref(), Some("Só o que a busca e os filtros mostram"));
    }

    #[test]
    fn origem_conforme_o_escopo() {
        let mut j = JanelaExportar::nova(pedido_tabela(true, 2, Some(12)));
        let OrigemExportacao::Tabela { filtro, .. } = j.origem() else { panic!("tabela") };
        assert_eq!(filtro.busca.as_deref(), Some("ana"));
        assert_eq!((filtro.ordem.as_deref(), filtro.direcao), (Some("nome"), banco::Direcao::Desc));
        assert_eq!(filtro.chaves, None);
        j.form.escopo = Escopo::Marcadas;
        let OrigemExportacao::Tabela { filtro, .. } = j.origem() else { panic!("tabela") };
        assert_eq!((filtro.busca, filtro.chaves.map(|c| c.len())), (None, Some(2)));
        j.form.escopo = Escopo::Tudo;
        let OrigemExportacao::Tabela { filtro, .. } = j.origem() else { panic!("tabela") };
        assert!(filtro.busca.is_none() && filtro.chaves.is_none() && filtro.ordem.as_deref() == Some("nome"));
    }

    #[test]
    fn ordem_sem_busca_vai_em_todos_os_escopos() {
        // a aba Dados ordenada por uma coluna, sem busca nem filtro: o `FiltroAtual` só com a ordem
        let mut pedido = pedido_tabela(true, 2, None);
        if let OrigemJanelaExportar::Tabela { filtro: Some(f), .. } = &mut pedido.origem {
            f.busca = None;
        }
        let mut j = JanelaExportar::nova(pedido);
        assert_eq!(j.form.rotulo_filtradas, None, "sem busca nem filtro, sem a opção da busca");
        assert_eq!(j.form.escopo, Escopo::Tudo);
        for escopo in [Escopo::Tudo, Escopo::Marcadas] {
            j.form.escopo = escopo;
            let OrigemExportacao::Tabela { filtro, .. } = j.origem() else { panic!("tabela") };
            assert_eq!((filtro.ordem.as_deref(), filtro.direcao), (Some("nome"), banco::Direcao::Desc), "{escopo:?}");
            assert!(filtro.busca.is_none() && filtro.filtros.is_empty());
        }
    }

    #[test]
    fn titulos_do_banco_e_do_resultado() {
        let banco = JanelaExportar::nova(PedidoJanelaExportar {
            origem: OrigemJanelaExportar::Banco,
            nome_sugerido: "vendas".into(),
        });
        assert_eq!(banco.titulo, "Exportar o banco");
        let consulta = JanelaExportar::nova(PedidoJanelaExportar {
            origem: OrigemJanelaExportar::Consulta { sql: "SELECT 1".into() },
            nome_sugerido: "consulta".into(),
        });
        assert_eq!(consulta.titulo, "Exportar o resultado");
        assert!(!consulta.exportando());
    }

    #[test]
    fn aviso_do_arquivo_salvo() {
        let r = ResumoExportacao {
            caminho: PathBuf::from(r"C:\x\clientes.xlsx"),
            nome: "clientes.xlsx".into(),
            formato: Formato::Xlsx,
            linhas: 60,
            bytes: 12_000,
            tabelas: 1,
            tempo_ms: 5,
            avisos: vec![],
        };
        let AvisoJanela::Salvo { detalhes, .. } = aviso_salvo(&r, Tipo::Tabela) else { panic!("salvo") };
        assert_eq!(detalhes, "60 linhas · 12 KB");
        let AvisoJanela::Salvo { detalhes, .. } = aviso_salvo(&ResumoExportacao { tabelas: 7, ..r }, Tipo::Banco)
        else {
            panic!("salvo")
        };
        assert_eq!(detalhes, "7 tabelas · 60 linhas · 12 KB");
    }
}
