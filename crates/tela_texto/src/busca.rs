//! A busca da barra no texto e no JSON (Ctrl+F, "Buscar em notas.txt"): no texto inteiro, no editor e no leitor só
//! leitura dos arquivos grandes (até 32 MiB), numa tarefa (a [`comum::texto::Agulha`] numa passada, sem copiar o texto
//! do leitor), sem diferenciar maiúsculas nem acentos. O trecho achado fica pintado como na Consulta SQL (no editor,
//! `AreaTexto::destacar`, com o cursor no fim dele; no leitor, o fundo `--sel` na linha) e a rolagem vai até ele; Enter
//! e Shift+Enter andam (com volta); a faixa diz "3 de 41" ("41+" enquanto conta); Esc limpa e volta ao cursor (ou à
//! linha) de antes. Cada tecla troca a busca (a de antes é cancelada); o texto que muda no editor a refaz sem mexer no
//! cursor de quem digita.

use std::ops::Range;
use std::sync::Arc;

use ::texto::posicao::{byte_do_caractere, caractere_do_byte};
use comum::texto::Agulha;
use egui::Context;
use estado::EventoBusca;
use estado::busca::BuscaEmFundo;

use crate::edicao::{colocar_cursor, cursor_do_editor};
use crate::{Corpo, TelaTexto};

/// Um achado: o trecho em bytes do texto.
type Achado = (usize, usize);

/// Para onde o Esc volta.
#[derive(Debug, Clone, Copy)]
enum Antes {
    /// O cursor do editor (em caracteres).
    Cursor(usize),
    /// A primeira linha à vista no leitor.
    Linha(usize),
}

/// O estado da busca da barra de um texto.
#[derive(Default)]
pub(crate) struct BuscaTexto {
    /// O texto do campo.
    pub texto: String,
    busca: Option<BuscaEmFundo<Achado>>,
    antes: Option<Antes>,
    /// A versão do texto do editor que a busca rodando viu.
    versao: u64,
    /// O trecho achado que o editor pinta (em caracteres), da versão do texto, e se a rolagem vai até ele.
    pub trecho: Option<(Range<usize>, u64)>,
    pub rolar: bool,
}

impl BuscaTexto {
    /// O que a faixa diz da busca ("3 de 41"), quando há uma.
    pub(crate) fn rotulo(&self) -> Option<String> {
        self.busca.as_ref().map(BuscaEmFundo::rotulo)
    }

    /// A busca ainda percorre o texto.
    pub(crate) fn ocupada(&self) -> bool {
        self.busca.as_ref().is_some_and(|b| !b.terminou())
    }
}

impl TelaTexto {
    /// A dica (e o rótulo) da busca da barra.
    pub(crate) fn dica_da_busca(&self) -> String {
        format!("Buscar em {}", self.nome)
    }

    /// O que a faixa diz da busca ("3 de 41", "41+", "Nenhum resultado"), quando há uma (para os testes).
    #[doc(hidden)]
    pub fn rotulo_da_busca(&self) -> Option<String> {
        self.busca.rotulo()
    }

    /// O trecho que a busca achou e o editor ou o leitor pintam, em bytes do texto (para os testes).
    #[doc(hidden)]
    pub fn trecho_achado(&self) -> Option<Range<usize>> {
        let (a, b) = self.busca.busca.as_ref()?.atual()?;
        Some(a..b)
    }

    /// De onde a busca parte: o achado da vez (a próxima tecla refina a mesma ocorrência), o cursor do editor ou a
    /// primeira linha à vista no leitor; e para onde o Esc volta.
    fn origem_da_busca(&self, egui: &Context) -> (usize, Antes) {
        let atual = self.busca.busca.as_ref().and_then(BuscaEmFundo::atual).map(|(a, _)| a);
        match &self.corpo {
            Corpo::Editor(e) => {
                let cursor = cursor_do_editor(egui, self.id_do_editor()).unwrap_or(0);
                (atual.unwrap_or_else(|| byte_do_caractere(&e.texto, cursor)), Antes::Cursor(cursor))
            }
            Corpo::Leitor(l) => {
                let linha = l.desenhadas.start;
                let byte = l.linhas.get(linha).map_or(0, |&b| b as usize);
                (atual.unwrap_or(byte), Antes::Linha(linha))
            }
            _ => (0, Antes::Cursor(0)),
        }
    }

    /// O Enter, o Shift+Enter, a tecla no campo e o Esc da busca da barra.
    pub(crate) fn evento_da_busca(&mut self, evento: EventoBusca, egui: &Context) {
        match evento {
            EventoBusca::Mudou => {
                if self.busca.texto.trim().is_empty() {
                    self.limpar_achado();
                    self.busca.busca = None;
                    self.busca.antes = None;
                    return;
                }
                let (origem, antes) = self.origem_da_busca(egui);
                self.busca.antes.get_or_insert(antes);
                self.comecar_busca(egui, Some(origem));
            }
            EventoBusca::Enter | EventoBusca::Anterior => {
                let adiante = evento == EventoBusca::Enter;
                let (origem, antes) = self.origem_da_busca(egui);
                match self.busca.busca.as_mut() {
                    Some(b) => {
                        b.definir_origem((origem, origem));
                        if let Some(alvo) = b.seguinte(adiante) {
                            self.ir_ao_achado(egui, alvo);
                        }
                    }
                    None if !self.busca.texto.trim().is_empty() => {
                        self.busca.antes.get_or_insert(antes);
                        self.comecar_busca(egui, Some(origem));
                    }
                    None => {}
                }
            }
            EventoBusca::Limpar => {
                self.busca.busca = None;
                self.limpar_achado();
                let id = self.id_do_editor();
                match (self.busca.antes.take(), &mut self.corpo) {
                    (Some(Antes::Cursor(c)), Corpo::Editor(_)) => colocar_cursor(egui, id, c),
                    (Some(Antes::Linha(l)), Corpo::Leitor(leitor)) => leitor.rolar_para = Some(l),
                    _ => {}
                }
            }
        }
        egui.request_repaint();
    }

    /// Tira a pintura do trecho achado (no editor e no leitor).
    fn limpar_achado(&mut self) {
        self.busca.trecho = None;
        if let Corpo::Leitor(l) = &mut self.corpo {
            l.achado = None;
        }
    }

    /// Começa (ou recomeça) a busca do texto do campo; com `origem`, vai ao primeiro achado a partir dela.
    fn comecar_busca(&mut self, egui: &Context, origem: Option<usize>) {
        let (texto, versao): (Arc<String>, u64) = match &self.corpo {
            Corpo::Editor(e) => (Arc::new(e.texto.clone()), e.versao),
            Corpo::Leitor(l) => (Arc::clone(&l.texto), 0),
            _ => {
                self.busca.busca = None;
                return;
            }
        };
        let Some(agulha) = Agulha::nova(self.busca.texto.trim()) else {
            self.busca.busca = None;
            return;
        };
        self.busca.versao = versao;
        self.busca.busca = Some(BuscaEmFundo::nova(egui, origem.map(|o| (o, o)), true, move |coletor| {
            for r in agulha.achados(&texto) {
                if !coletor.achou((r.start, r.end)) {
                    break;
                }
            }
            Ok(())
        }));
    }

    /// A cada quadro: refaz a busca se o texto do editor mudou (sem mexer no cursor de quem digita), e vai ao achado
    /// que chegou.
    pub(crate) fn passo_da_busca(&mut self, egui: &Context) {
        if self.busca.busca.is_none() {
            return;
        }
        if let Corpo::Editor(e) = &self.corpo
            && e.versao != self.busca.versao
        {
            self.limpar_achado();
            self.comecar_busca(egui, None);
        }
        if let Some(alvo) = self.busca.busca.as_mut().and_then(|b| b.passo(egui)) {
            self.ir_ao_achado(egui, alvo);
        }
    }

    /// O trecho achado fica pintado e à vista: no editor, com o cursor no fim dele; no leitor, rolando até a linha.
    fn ir_ao_achado(&mut self, egui: &Context, (inicio, fim): Achado) {
        let id = self.id_do_editor();
        match &mut self.corpo {
            Corpo::Editor(e) => {
                let (a, b) = (caractere_do_byte(&e.texto, inicio), caractere_do_byte(&e.texto, fim));
                self.busca.trecho = Some((a..b, e.versao));
                self.busca.rolar = true;
                colocar_cursor(egui, id, b);
            }
            Corpo::Leitor(l) => {
                l.achado = Some(inicio..fim);
                l.rolar_ao_achado = true;
            }
            _ => {}
        }
        egui.request_repaint();
    }
}
