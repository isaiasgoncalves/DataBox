//! O leitor (arquivo acima de [`texto::limites::LIMITE_EDICAO`]): só leitura, sem `TextEdit`. Desenha por linha, só as
//! linhas que cabem na vista, com a calha dos números e as cores do JSON (uma cadeia nunca atravessa a linha, então
//! o léxico linha a linha é exato). O índice dos começos de linha e a análise do JSON saem da tarefa de leitura.

use std::ops::Range;
use std::sync::Arc;

use ::texto::Carregado;
use ::texto::json::ErroJson;
use egui::text::{LayoutJob, TextFormat};
use egui::{Rect, ScrollArea, Sense, Ui, pos2, vec2};
use interface::codigo::{self, Linguagem, trechos_json};
use interface::pecas::campos::LARGURA_CALHA;
use interface::tema::paleta;

use crate::carga::Lido;
use crate::json::EstadoJson;

/// O recuo do texto dos lados da calha e em cima (o `.cod{padding:0 14px}` dentro de um bloco com `padding-top:10px`).
const RECUO_X: f32 = 14.0;
const RECUO_Y: f32 = 10.0;
/// O número da margem se alinha à direita da calha com 12 de folga.
const FOLGA_DO_NUMERO: f32 = 12.0;
/// Quantos caracteres de uma linha o leitor desenha (uma linha de megabytes custaria o quadro; o resto vira "…").
pub(crate) const MAXIMO_DE_CARACTERES_DA_LINHA: usize = 4_000;

pub(crate) struct Leitor {
    pub texto: Arc<String>,
    pub carregado: Carregado,
    /// O byte onde cada linha começa.
    pub linhas: Vec<u32>,
    pub maior_linha: usize,
    pub caracteres: usize,
    pub json: EstadoJson,
    /// As linhas que o último quadro desenhou (os testes conferem que são só as da vista).
    pub desenhadas: Range<usize>,
    /// A linha (de 0) que a rolagem deve trazer à vista no próximo quadro.
    pub rolar_para: Option<usize>,
    /// O trecho (em bytes do texto) que a busca da barra achou, pintado com o fundo `--sel`, e se a rolagem vai até
    /// ele no próximo quadro.
    pub achado: Option<Range<usize>>,
    pub rolar_ao_achado: bool,
}

impl Leitor {
    pub(crate) fn novo(lido: Lido) -> Leitor {
        Leitor {
            texto: lido.texto,
            carregado: lido.carregado,
            linhas: lido.linhas,
            maior_linha: lido.maior_linha,
            caracteres: lido.caracteres,
            json: lido.json.map(EstadoJson::com_resposta).unwrap_or_default(),
            desenhadas: 0..0,
            rolar_para: None,
            achado: None,
            rolar_ao_achado: false,
        }
    }

    pub(crate) fn n_linhas(&self) -> usize {
        self.linhas.len()
    }

    /// O texto da linha `i`, sem a quebra.
    pub(crate) fn linha(&self, i: usize) -> &str {
        let Some(&inicio) = self.linhas.get(i) else {
            return "";
        };
        let inicio = inicio as usize;
        let fim = self.linhas.get(i + 1).map_or(self.texto.len(), |&n| n as usize);
        self.texto[inicio..fim].trim_end_matches('\n')
    }

    /// A linha (de 0) onde o byte está.
    pub(crate) fn linha_do_byte(&self, byte: usize) -> usize {
        self.linhas.partition_point(|&n| n as usize <= byte).saturating_sub(1)
    }

    /// O pedaço do trecho `bytes` que cai na linha `i`, em caracteres da linha (cortado no que o leitor desenha).
    fn colunas_do_trecho(&self, i: usize, bytes: &Range<usize>) -> Option<Range<usize>> {
        let inicio = *self.linhas.get(i)? as usize;
        let linha = self.linha(i);
        let fim = inicio + linha.len();
        if bytes.end <= inicio || bytes.start > fim || (bytes.start == fim && bytes.end > fim) {
            return None;
        }
        let a = bytes.start.max(inicio) - inicio;
        let b = bytes.end.min(fim) - inicio;
        let coluna =
            |byte: usize| linha.get(..byte).map_or(0, |t| t.chars().count()).min(MAXIMO_DE_CARACTERES_DA_LINHA);
        Some(coluna(a)..coluna(b))
    }

    /// Desenha o leitor em `rect`: a calha parada na rolagem horizontal, as linhas visíveis e, no JSON inválido, a
    /// linha do erro com o fundo e o número em `--perigo`.
    pub(crate) fn desenhar(&mut self, ui: &mut Ui, sal: egui::Id, rect: Rect, linguagem: Linguagem) {
        let p = paleta(ui);
        let e = &p.estudio;
        let aparencia = codigo::atual(ui.ctx());
        let cores = aparencia.cores.resolver(p);
        let fonte = codigo::fonte(ui.ctx());
        let altura = codigo::altura_linha(fonte.size);
        let largura_do_char = ui.fonts_mut(|f| f.glyph_width(&fonte, '0'));
        let n = self.n_linhas();
        let colunas = self.maior_linha.min(MAXIMO_DE_CARACTERES_DA_LINHA + 1);
        let largura_conteudo = LARGURA_CALHA + RECUO_X * 2.0 + colunas as f32 * largura_do_char;
        let altura_conteudo = RECUO_Y * 2.0 + n as f32 * altura;
        let erro: Option<ErroJson> = self.json.erro.clone();
        let rolar = self.rolar_para.take();
        // o trecho achado pela busca: a linha e as colunas dele, e o retângulo aproximado (a fonte é monoespaçada) para
        // a rolagem levá-lo à vista também na horizontal
        let achado = self.achado.clone();
        let rolar_achado = match (std::mem::take(&mut self.rolar_ao_achado), &achado) {
            (true, Some(a)) => {
                let i = self.linha_do_byte(a.start);
                self.colunas_do_trecho(i, a).map(|colunas| (i, colunas))
            }
            _ => None,
        };

        ui.painter().rect_filled(rect, 0.0, cores.fundo);
        let mut origem_y = rect.top();
        let mut visiveis = 0..0;
        let area = ScrollArea::both().id_salt(("databox::leitor", sal)).auto_shrink([false, false]);
        let _ = area.show_viewport(&mut ui.new_child(egui::UiBuilder::new().max_rect(rect)), |ui, vista| {
            let (conteudo, _) = ui.allocate_exact_size(
                vec2(largura_conteudo.max(rect.width()), altura_conteudo.max(rect.height())),
                Sense::hover(),
            );
            origem_y = conteudo.top();
            let primeira = (((vista.min.y - RECUO_Y) / altura).floor().max(0.0)) as usize;
            let ultima = ((((vista.max.y - RECUO_Y) / altura).ceil().clamp(0.0, n as f32)) as usize + 1).min(n);
            visiveis = primeira.min(n)..ultima;
            if let Some(linha) = rolar {
                let topo = conteudo.top() + RECUO_Y + linha as f32 * altura;
                let alvo = Rect::from_min_size(pos2(conteudo.left(), topo), vec2(rect.width(), altura));
                ui.scroll_to_rect(alvo, Some(egui::Align::Center));
            }
            if let Some((linha, colunas)) = &rolar_achado {
                let topo = conteudo.top() + RECUO_Y + *linha as f32 * altura;
                let x = conteudo.left() + LARGURA_CALHA + RECUO_X + colunas.start as f32 * largura_do_char;
                let largura = (colunas.end - colunas.start).max(1) as f32 * largura_do_char;
                let alvo = Rect::from_min_size(pos2(x, topo), vec2(largura, altura))
                    .expand2(vec2(LARGURA_CALHA + RECUO_X, 0.0));
                ui.scroll_to_rect(alvo, Some(egui::Align::Center));
            }
            let pintor = ui.painter().clone();
            for i in visiveis.clone() {
                let topo = conteudo.top() + RECUO_Y + i as f32 * altura;
                if erro.as_ref().is_some_and(|e| e.linha.saturating_sub(1) == i) {
                    let faixa = Rect::from_min_size(
                        pos2(conteudo.left() + LARGURA_CALHA, topo),
                        vec2(conteudo.width() - LARGURA_CALHA, altura),
                    );
                    pintor.rect_filled(faixa, 0.0, e.perigo.gamma_multiply(0.09));
                }
                let job = job_da_linha(self.linha(i), linguagem, &cores, &fonte, altura);
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                let origem = pos2(conteudo.left() + LARGURA_CALHA + RECUO_X, topo);
                if let Some(colunas) = achado.as_ref().and_then(|a| self.colunas_do_trecho(i, a)) {
                    let x0 = galley.pos_from_cursor(egui::text::CCursor::new(colunas.start)).min.x;
                    let x1 = galley.pos_from_cursor(egui::text::CCursor::new(colunas.end)).min.x;
                    let r =
                        Rect::from_min_max(pos2(origem.x + x0, topo), pos2(origem.x + x1.max(x0 + 2.0), topo + altura));
                    pintor.rect_filled(r, 2.0, e.sel);
                }
                pintor.galley(origem, galley, cores.texto);
            }
        });
        self.desenhadas = visiveis.clone();
        // a calha: por cima das linhas, parada, acompanhando só a rolagem vertical
        let calha = Rect::from_min_size(rect.min, vec2(LARGURA_CALHA, rect.height()));
        let pintor = ui.painter().with_clip_rect(calha.intersect(ui.clip_rect()));
        pintor.rect_filled(calha, 0.0, e.faixa);
        pintor.vline(calha.right() - 0.5, calha.y_range(), egui::Stroke::new(1.0, e.b));
        for i in visiveis {
            let topo = origem_y + RECUO_Y + i as f32 * altura;
            let cor = if erro.as_ref().is_some_and(|e| e.linha.saturating_sub(1) == i) { e.perigo } else { e.t3 };
            let mut job = LayoutJob::simple_singleline((i + 1).to_string(), fonte.clone(), cor);
            for s in &mut job.sections {
                s.format.line_height = Some(altura);
            }
            let numero = ui.fonts_mut(|f| f.layout_job(job));
            let x = calha.right() - FOLGA_DO_NUMERO - numero.size().x;
            pintor.galley(pos2(x, topo), numero, cor);
        }
    }
}

/// O texto de uma linha com as cores da `linguagem`, cortado em [`MAXIMO_DE_CARACTERES_DA_LINHA`] caracteres (com "…").
fn job_da_linha(
    linha: &str,
    linguagem: Linguagem,
    cores: &codigo::CoresResolvidas,
    fonte: &egui::FontId,
    altura: f32,
) -> LayoutJob {
    let (corte, cortada) = match linha.char_indices().nth(MAXIMO_DE_CARACTERES_DA_LINHA) {
        Some((i, _)) => (&linha[..i], true),
        None => (linha, false),
    };
    let mut job = LayoutJob { text: String::new(), ..Default::default() };
    job.wrap.max_width = f32::INFINITY;
    let formato = |cor| {
        let mut f = TextFormat::simple(fonte.clone(), cor);
        f.line_height = Some(altura);
        f
    };
    match linguagem {
        Linguagem::Json => {
            for t in trechos_json(corte) {
                job.append(&corte[t.bytes], 0.0, formato(cores.da_classe_json(t.classe)));
            }
        }
        _ => job.append(corte, 0.0, formato(cores.texto)),
    }
    if cortada {
        job.append("…", 0.0, formato(cores.pu));
    }
    if job.text.is_empty() {
        job.append("", 0.0, formato(cores.texto));
    }
    job
}

#[cfg(test)]
mod testes {
    use super::*;

    fn leitor(texto: &str) -> Leitor {
        let (linhas, maior_linha) = crate::carga::indice_de_linhas(texto);
        Leitor {
            texto: Arc::new(texto.to_string()),
            carregado: ::texto::arquivo::decodificar(b"", None).expect("vazio"),
            linhas,
            maior_linha,
            caracteres: texto.chars().count(),
            json: EstadoJson::default(),
            desenhadas: 0..0,
            rolar_para: None,
            achado: None,
            rolar_ao_achado: false,
        }
    }

    #[test]
    fn linha_e_sem_quebra_e_o_byte_acha_a_linha() {
        let l = leitor("ab\ncde\n\nf");
        assert_eq!(l.n_linhas(), 4);
        assert_eq!([l.linha(0), l.linha(1), l.linha(2), l.linha(3), l.linha(9)], ["ab", "cde", "", "f", ""]);
        assert_eq!(
            [l.linha_do_byte(0), l.linha_do_byte(2), l.linha_do_byte(3), l.linha_do_byte(7), l.linha_do_byte(99)],
            [0, 0, 1, 2, 3]
        );
    }

    #[test]
    fn o_trecho_achado_cai_nas_colunas_da_linha() {
        let l = leitor("ab\nação x\n\nfim");
        // "ção" está nos bytes 4..9 do texto (na linha 1, as colunas 1..4)
        assert_eq!(l.colunas_do_trecho(1, &(4..9)), Some(1..4));
        assert_eq!(l.colunas_do_trecho(0, &(4..9)), None);
        assert_eq!(l.colunas_do_trecho(3, &(13..16)), Some(0..3));
        assert_eq!(l.colunas_do_trecho(2, &(4..9)), None);
    }

    #[test]
    fn linha_comprida_e_cortada_com_reticencia() {
        let cores = codigo::CoresCodigo::DataBox.resolver(interface::tema::paleta_ctx(&egui::Context::default()));
        let fonte = egui::FontId::monospace(12.5);
        let longa = "x".repeat(MAXIMO_DE_CARACTERES_DA_LINHA + 500);
        let job = job_da_linha(&longa, Linguagem::Texto, &cores, &fonte, 20.0);
        assert_eq!(job.text.chars().count(), MAXIMO_DE_CARACTERES_DA_LINHA + 1);
        assert!(job.text.ends_with('…'));
        let curta = job_da_linha("ab", Linguagem::Json, &cores, &fonte, 20.0);
        assert_eq!(curta.text, "ab");
        let vazia = job_da_linha("", Linguagem::Json, &cores, &fonte, 20.0);
        assert_eq!(vazia.text, "");
    }
}
