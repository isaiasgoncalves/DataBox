//! O editor: o texto que a pessoa edita (até [`texto::limites::LIMITE_EDICAO`]), o que é "alterado", o desfazer (o
//! `Undoer` do `TextEdit`, com o `max_undos` que cai com o tamanho), a posição do cursor e a gravação.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ::texto::json::{ErroJson, NoId, Recuo, recuo_do_texto, validar};
use ::texto::limites::max_undos;
use ::texto::posicao::{byte_do_caractere, contar_linhas, linha_e_coluna};
use ::texto::{Carregado, ResumoGravado, gravar, mudou_no_disco};
use comum::ErroBanco;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::util::undoer::{Settings, Undoer};
use egui::{Context, Id};
use estado::Promessa;

use crate::TipoDeTexto;
use crate::carga::Lido;
use crate::json::{EstadoJson, MarcaDoErro};

/// O que identifica um texto sem guardar uma cópia dele: o tamanho e o resumo (SipHash com chave fixa). O "alterado"
/// é o texto de agora diferir do salvo, então desfazer até o salvo tira o ponto.
pub(crate) type Marca = (usize, u64);

/// O intervalo mínimo entre duas contas da oferta do Desfazer e do Refazer durante a digitação (s).
pub(crate) const INTERVALO_DA_OFERTA: f64 = 0.25;

pub(crate) fn marca(texto: &str) -> Marca {
    let mut h = DefaultHasher::new();
    texto.hash(&mut h);
    (texto.len(), h.finish())
}

/// O que a tarefa de gravação devolve.
pub(crate) enum Saida {
    /// O arquivo já não é o que foi lido: nada foi gravado, a pessoa decide.
    Mudou,
    Gravado {
        resumo: ResumoGravado,
        termina_em_quebra: bool,
        /// O erro do JSON gravado (o toast "salvo com o erro da linha 14").
        erro: Option<ErroJson>,
    },
}

/// O que aconteceu quando a gravação terminou.
pub(crate) enum FimDaGravacao {
    PerguntarDisco,
    Gravado { erro: Option<ErroJson> },
    Falhou(ErroBanco),
}

pub(crate) struct Gravando {
    pub promessa: Promessa<Saida>,
    /// A marca do texto que foi para a tarefa.
    pub marca: Marca,
}

/// Contas do texto que o rodapé e as Propriedades usam, refeitas só quando o texto muda.
pub(crate) struct Medidas {
    pub versao: u64,
    pub linhas: usize,
    pub caracteres: usize,
    pub recuo: Option<Recuo>,
}

/// O cursor em linha e coluna e o caminho dele na estrutura, refeitos só quando o cursor, o texto ou a estrutura
/// mudam.
pub(crate) struct Posicao {
    pub versao: u64,
    pub geracao: u64,
    pub indice: usize,
    pub linha: usize,
    pub coluna: usize,
    /// O caminho do cursor na estrutura (da raiz ao mais interno), vazio sem estrutura.
    pub caminho: Vec<NoId>,
}

pub(crate) struct Editor {
    pub texto: String,
    pub tipo: TipoDeTexto,
    /// Tudo o que a gravação precisa para devolver o arquivo como estava (o texto fica em `texto`).
    pub carregado: Carregado,
    /// Sobe a cada mudança do texto.
    pub versao: u64,
    marca_atual: Marca,
    salvo: Marca,
    pub alterado: bool,
    pub pode_desfazer: bool,
    pub pode_refazer: bool,
    /// Quantas vezes a oferta do desfazer foi recalculada (os testes conferem que é só quando o texto muda).
    pub recalculos: usize,
    /// O relógio do egui da última conta da oferta, e se uma conta ficou para o fim do intervalo.
    ultimo_recalculo: f64,
    oferta_pendente: bool,
    pub gravando: Option<Gravando>,
    pub medidas: Option<Medidas>,
    pub posicao: Option<Posicao>,
    /// O cursor (índice de caractere) que o editor deve tomar no próximo quadro, com o foco e a rolagem até ele.
    pub ir_para: Option<usize>,
    pub json: EstadoJson,
    /// A marca do erro de JSON (versão do texto, geração da análise e a marca), refeita só quando uma das duas muda.
    marca_do_erro: Option<(u64, u64, Option<MarcaDoErro>)>,
}

impl Editor {
    pub(crate) fn novo(lido: Lido, tipo: TipoDeTexto) -> Editor {
        let texto = std::sync::Arc::unwrap_or_clone(lido.texto);
        let m = marca(&texto);
        Editor {
            texto,
            tipo,
            carregado: lido.carregado,
            versao: 0,
            marca_atual: m,
            salvo: m,
            alterado: false,
            pode_desfazer: false,
            pode_refazer: false,
            recalculos: 0,
            ultimo_recalculo: f64::NEG_INFINITY,
            oferta_pendente: false,
            gravando: None,
            medidas: None,
            posicao: None,
            ir_para: None,
            json: lido.json.map(EstadoJson::com_resposta).unwrap_or_default(),
            marca_do_erro: None,
        }
    }

    /// O texto mudou (digitação, desfazer, Formatar): a versão sobe, o "alterado" se refaz e, no JSON, a validação
    /// é agendada.
    pub(crate) fn mudou(&mut self, agora: f64) {
        self.versao += 1;
        self.marca_atual = marca(&self.texto);
        self.alterado = self.marca_atual != self.salvo;
        self.medidas = None;
        if self.tipo == TipoDeTexto::Json {
            self.json.agendar(agora);
        }
    }

    pub(crate) fn medidas(&mut self) -> &Medidas {
        if self.medidas.as_ref().is_none_or(|m| m.versao != self.versao) {
            self.medidas = Some(Medidas {
                versao: self.versao,
                linhas: contar_linhas(&self.texto),
                caracteres: self.texto.chars().count(),
                recuo: recuo_do_texto(&self.texto),
            });
        }
        self.medidas.as_ref().expect("acabou de ser feita")
    }

    /// O cursor em linha e coluna e o caminho dele na estrutura; `indice` é o cursor em caracteres.
    pub(crate) fn posicao(&mut self, indice: usize) -> &Posicao {
        let geracao = self.json.geracao;
        let valido = self
            .posicao
            .as_ref()
            .is_some_and(|p| p.versao == self.versao && p.geracao == geracao && p.indice == indice);
        if !valido {
            let byte = byte_do_caractere(&self.texto, indice);
            let (linha, coluna) = linha_e_coluna(&self.texto, byte);
            let caminho = crate::estrutura::caminho_do_cursor(&self.json, &self.texto, self.versao, byte);
            self.posicao = Some(Posicao { versao: self.versao, geracao, indice, linha, coluna, caminho });
        }
        self.posicao.as_ref().expect("acabou de ser feita")
    }

    /// Onde marcar o erro de JSON no texto de agora: só quando a validação é a do texto de agora (senão o lugar do erro
    /// pode ter se mexido). A conta dos caracteres é O(n): refeita só quando o texto ou a análise mudam.
    pub(crate) fn marca_do_erro(&mut self) -> Option<MarcaDoErro> {
        if self.json.erro.is_none() || !self.json.em_dia(self.versao) {
            return None;
        }
        let chave = (self.versao, self.json.geracao);
        if self.marca_do_erro.as_ref().is_none_or(|(v, g, _)| (*v, *g) != chave) {
            self.marca_do_erro = Some((chave.0, chave.1, self.json.marca_do_erro(&self.texto)));
        }
        self.marca_do_erro.as_ref().and_then(|(_, _, m)| m.clone())
    }

    /// O cursor está na quantidade de caracteres do texto (o fim, se o egui ainda não guardou um cursor).
    pub(crate) fn caracteres(&mut self) -> usize {
        self.medidas().caracteres
    }

    // ------------------------------------------------------------------------------------------ desfazer

    /// A digitação mudou o texto: a oferta do Desfazer e do Refazer se refaz no máximo a cada
    /// [`INTERVALO_DA_OFERTA`] (cada conta copia o `Undoer` inteiro, até 64 fotografias de 0,5 MiB). No meio-tempo vale
    /// o que a tecla faz no egui: há o que desfazer, e o refazer se esvaziou. O quadro pedido para o fim do intervalo
    /// faz a conta exata.
    pub(crate) fn oferta_apos_digitar(&mut self, egui: &Context, id: Id, agora: f64) {
        if agora - self.ultimo_recalculo >= INTERVALO_DA_OFERTA {
            self.recalcular_oferta(egui, id);
        } else {
            self.pode_desfazer = true;
            self.pode_refazer = false;
            if !self.oferta_pendente {
                self.oferta_pendente = true;
                egui.request_repaint_after(std::time::Duration::from_secs_f64(INTERVALO_DA_OFERTA));
            }
        }
    }

    /// A conta exata que ficou para o fim do intervalo (a cada quadro; não faz nada sem conta pendente).
    pub(crate) fn oferta_pendente(&mut self, egui: &Context, id: Id, agora: f64) {
        if !self.oferta_pendente {
            return;
        }
        if agora - self.ultimo_recalculo >= INTERVALO_DA_OFERTA {
            self.recalcular_oferta(egui, id);
        } else {
            let falta = INTERVALO_DA_OFERTA - (agora - self.ultimo_recalculo);
            egui.request_repaint_after(std::time::Duration::from_secs_f64(falta.max(0.01)));
        }
    }

    /// Recalcula o que o Desfazer e o Refazer oferecem (uma cópia do texto, só quando ele muda).
    pub(crate) fn recalcular_oferta(&mut self, egui: &Context, id: Id) {
        self.recalculos += 1;
        self.oferta_pendente = false;
        self.ultimo_recalculo = egui.input(|i| i.time);
        let n = self.caracteres();
        let Some(estado) = TextEditState::load(egui, id) else {
            self.pode_desfazer = false;
            self.pode_refazer = false;
            return;
        };
        let atual = (cursor_ou_fim(&estado, n), self.texto.clone());
        let undoer = estado.undoer();
        self.pode_desfazer = undoer.has_undo(&atual);
        self.pode_refazer = undoer.has_redo(&atual);
    }

    /// Desfaz ou refaz um passo pelo `Undoer` do campo (o que o Ctrl+Z faz com o foco no campo). Devolve se o texto
    /// mudou.
    pub(crate) fn desfazer_ou_refazer(&mut self, egui: &Context, id: Id, refazer: bool, agora: f64) -> bool {
        let n = self.caracteres();
        let mut estado = TextEditState::load(egui, id).unwrap_or_default();
        let atual = (cursor_ou_fim(&estado, n), self.texto.clone());
        let mut undoer = estado.undoer();
        let passo = if refazer { undoer.redo(&atual) } else { undoer.undo(&atual) }.cloned();
        estado.set_undoer(undoer);
        let mudou = match passo {
            Some((cursor, texto)) => {
                self.texto = texto;
                estado.cursor.set_char_range(Some(cursor));
                true
            }
            None => false,
        };
        estado.store(egui, id);
        if mudou {
            self.mudou(agora);
        }
        self.recalcular_oferta(egui, id);
        mudou
    }

    /// Troca o texto todo por `novo` como **um** passo do desfazer (o Formatar): o texto de antes entra no `Undoer`
    /// como ponto e o de depois também, então um Ctrl+Z volta ao de antes.
    pub(crate) fn substituir_como_um_passo(&mut self, egui: &Context, id: Id, novo: String, agora: f64) {
        let n = self.caracteres();
        let mut estado = TextEditState::load(egui, id).unwrap_or_default();
        let antigo_cursor = cursor_ou_fim(&estado, n);
        let indice = antigo_cursor.primary.index.0;
        let novo_indice = cursor_apos_formatar(&self.texto, indice, &novo);
        let novo_cursor = CCursorRange::one(CCursor::new(novo_indice));
        let mut undoer = estado.undoer();
        undoer.add_undo(&(antigo_cursor, self.texto.clone()));
        // o `add_undo` não esvazia o refazer (só o `feed_state` faz isso, quando o estado mudou): sem isto, um Refazer
        // depois do Formatar traria de volta o passo desfeito antes, sem a formatação
        let depois = (novo_cursor, novo.clone());
        undoer.feed_state(agora, &depois);
        undoer.add_undo(&depois);
        estado.set_undoer(undoer);
        estado.cursor.set_char_range(Some(novo_cursor));
        estado.store(egui, id);
        self.texto = novo;
        self.mudou(agora);
        self.recalcular_oferta(egui, id);
    }

    // ------------------------------------------------------------------------------------------ gravar

    /// Pede a gravação numa tarefa (o texto de agora, copiado). Com `forcar`, grava mesmo que o arquivo tenha mudado
    /// no disco. Sem efeito se já há uma gravação.
    pub(crate) fn gravar(&mut self, egui: &Context, caminho: &std::path::Path, forcar: bool) {
        if self.gravando.is_some() {
            return;
        }
        let texto = self.texto.clone();
        let m = self.marca_atual;
        let original = self.carregado.clone();
        let caminho = caminho.to_path_buf();
        let json = self.tipo == TipoDeTexto::Json;
        let promessa = Promessa::rodar(egui, move || {
            if !forcar && mudou_no_disco(&caminho, &original) {
                return Ok(Saida::Mudou);
            }
            let resumo = gravar(&caminho, &texto, &original)?;
            let erro = if json { validar(&texto) } else { None };
            Ok(Saida::Gravado { resumo, termina_em_quebra: texto.ends_with('\n'), erro })
        });
        self.gravando = Some(Gravando { promessa, marca: m });
    }

    /// Toma o fim da gravação, se terminou.
    pub(crate) fn fim_da_gravacao(&mut self) -> Option<FimDaGravacao> {
        let g = self.gravando.as_mut()?;
        let resultado = g.promessa.tomar()?;
        let marca_gravada = g.marca;
        self.gravando = None;
        Some(match resultado {
            Ok(Saida::Mudou) => FimDaGravacao::PerguntarDisco,
            Ok(Saida::Gravado { resumo, termina_em_quebra, erro }) => {
                self.carregado.bytes = resumo.bytes;
                self.carregado.modificado = resumo.modificado;
                self.carregado.termina_em_quebra = termina_em_quebra;
                self.carregado.fim_misto = false;
                self.salvo = marca_gravada;
                self.alterado = self.marca_atual != self.salvo;
                FimDaGravacao::Gravado { erro }
            }
            Err(e) => FimDaGravacao::Falhou(e),
        })
    }
}

/// O cursor do `TextEdit` guardado em `estado`, ou o fim do texto de `n` caracteres.
fn cursor_ou_fim(estado: &TextEditState, n: usize) -> CCursorRange {
    estado.cursor.char_range().unwrap_or_else(|| CCursorRange::one(CCursor::new(n)))
}

/// O cursor do editor `id` (índice de caractere), se o egui já guardou um.
pub(crate) fn cursor_do_editor(egui: &Context, id: Id) -> Option<usize> {
    TextEditState::load(egui, id).and_then(|e| e.cursor.char_range()).map(|r| r.primary.index.0)
}

/// Põe o cursor do editor `id` em `indice` (sem seleção).
pub(crate) fn colocar_cursor(egui: &Context, id: Id, indice: usize) {
    let mut estado = TextEditState::load(egui, id).unwrap_or_default();
    estado.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(indice))));
    estado.store(egui, id);
}

/// O estado do campo como de um texto recém-aberto: cursor no começo e desfazer vazio, com o `max_undos` do tamanho
/// do texto (`clamp(32 MiB / tamanho, 10, 100)`).
pub(crate) fn preparar_campo(egui: &Context, id: Id, tamanho: usize) {
    let mut estado = TextEditState::default();
    estado.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(0))));
    estado.set_undoer(Undoer::with_settings(Settings { max_undos: max_undos(tamanho as u64), ..Settings::default() }));
    estado.store(egui, id);
}

/// Onde fica o cursor depois de reescrever o texto só nos espaços: depois do mesmo número de caracteres que não são
/// espaço que havia antes dele.
pub(crate) fn cursor_apos_formatar(antigo: &str, indice: usize, novo: &str) -> usize {
    if indice >= antigo.chars().count() {
        return novo.chars().count();
    }
    let alvo = antigo.chars().take(indice).filter(|c| !c.is_whitespace()).count();
    let mut vistos = 0;
    for (i, c) in novo.chars().enumerate() {
        if vistos == alvo {
            return i;
        }
        if !c.is_whitespace() {
            vistos += 1;
        }
    }
    novo.chars().count()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn editor(texto: &str, tipo: TipoDeTexto) -> Editor {
        let texto = std::sync::Arc::new(texto.to_string());
        let lido = Lido {
            carregado: ::texto::arquivo::decodificar(b"", None).expect("vazio"),
            grande: false,
            linhas: Vec::new(),
            maior_linha: 0,
            caracteres: texto.chars().count(),
            json: (tipo == TipoDeTexto::Json)
                .then(|| crate::json::Resposta::analisar(0, std::sync::Arc::clone(&texto))),
            texto,
        };
        Editor::novo(lido, tipo)
    }

    #[test]
    fn alterado_e_o_texto_diferir_do_salvo_e_a_marca_do_erro_so_vale_em_dia() {
        let mut e = editor("{\"a\" 1}", TipoDeTexto::Json);
        assert!(!e.alterado && e.versao == 0);
        assert!(e.marca_do_erro().is_some(), "o erro da análise do texto de agora");
        // muda o texto: a versão sobe, o erro deixa de valer até a análise nova, e a validação é agendada
        e.texto.push('x');
        e.mudou(1.0);
        assert!(e.alterado && e.versao == 1 && e.json.ocupado());
        assert!(e.marca_do_erro().is_none(), "a análise é a da versão 0");
        // voltar ao texto de antes tira o "alterado" (o texto salvo é o do começo)
        e.texto.pop();
        e.mudou(1.1);
        assert!(!e.alterado);
        // a análise da versão de agora faz a marca voltar
        let r = crate::json::Resposta::analisar(e.versao, std::sync::Arc::new(e.texto.clone()));
        e.json.aplicar(r);
        assert!(e.marca_do_erro().is_some());
        // texto simples não agenda validação nenhuma
        let mut t = editor("abc", TipoDeTexto::Texto);
        t.texto.push('d');
        t.mudou(0.0);
        assert!(t.alterado && !t.json.ocupado());
    }

    #[test]
    fn medidas_so_se_refazem_quando_o_texto_muda() {
        let mut e = editor("a\n  b\n", TipoDeTexto::Texto);
        let m = e.medidas();
        assert_eq!((m.linhas, m.caracteres, m.recuo), (2, 6, Some(Recuo::Espacos(2))));
        e.texto.push('c');
        // sem `mudou`, a conta antiga vale (é o contrato: quem mexe no texto avisa)
        assert_eq!(e.medidas().caracteres, 6);
        e.mudou(0.0);
        assert_eq!((e.medidas().linhas, e.medidas().caracteres), (3, 7));
    }

    #[test]
    fn marca_distingue_textos_e_so_depende_do_conteudo() {
        assert_eq!(marca("abc"), marca(&String::from("abc")));
        assert_ne!(marca("abc"), marca("abd"));
        assert_ne!(marca("abc"), marca("abc "));
        assert_eq!(marca("").0, 0);
    }

    #[test]
    fn cursor_acompanha_o_formatar_pelos_caracteres_que_nao_sao_espaco() {
        let antigo = "{\"a\":  1,\"b\":2}";
        let novo = "{\n  \"a\": 1,\n  \"b\": 2\n}";
        // depois do `1`
        let i = antigo.find('1').unwrap() + 1;
        let j = cursor_apos_formatar(antigo, i, novo);
        assert_eq!(&novo[..j], "{\n  \"a\": 1");
        // no começo e no fim
        assert_eq!(cursor_apos_formatar(antigo, 0, novo), 0);
        assert_eq!(cursor_apos_formatar(antigo, antigo.chars().count(), novo), novo.chars().count());
    }
}
