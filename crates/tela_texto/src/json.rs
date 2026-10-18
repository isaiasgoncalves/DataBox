//! A validação do JSON: 250 ms depois da última tecla, numa tarefa, e a resposta de uma
//! versão velha do texto é descartada. Guarda o veredito mais novo, o erro com o lugar dele e a **última estrutura
//! válida** (a que a estrutura do documento mostra em `--t3` quando o JSON está inválido, N1).

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use ::texto::json::{Analise, Arvore, ErroJson, analisar, validar};
use ::texto::limites::LIMITE_ESTRUTURA;
use ::texto::posicao::caractere_do_byte;
use estado::Promessa;

/// Quanto se espera depois da última tecla para validar.
pub(crate) const ATRASO_DA_VALIDACAO: f64 = 0.25;

/// O que a tarefa de análise devolve: o texto analisado (com a versão dele) e o que se achou.
pub(crate) struct Resposta {
    pub versao: u64,
    pub texto: Arc<String>,
    /// A estrutura (None com erro, ou com o texto grande demais para ela: então só se validou).
    pub arvore: Option<Arvore>,
    pub erro: Option<ErroJson>,
}

impl Resposta {
    /// Analisa `texto` (a estrutura até [`LIMITE_ESTRUTURA`], só a validação acima).
    pub(crate) fn analisar(versao: u64, texto: Arc<String>) -> Resposta {
        if texto.len() as u64 > LIMITE_ESTRUTURA {
            let erro = validar(&texto);
            Resposta { versao, texto, arvore: None, erro }
        } else {
            let Analise { arvore, erro } = analisar(&texto);
            Resposta { versao, texto, arvore, erro }
        }
    }
}

/// A última estrutura válida e o texto de que ela veio (os nomes das chaves saem dele).
pub(crate) struct Valida {
    pub arvore: Arvore,
    pub texto: Arc<String>,
}

/// Como está o JSON: o círculo da faixa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Veredito {
    /// Ainda nenhuma análise terminou.
    Verificando,
    Valido,
    Invalido(ErroJson),
}

/// O lugar do erro no editor, pronto para as marcas: a linha (de 0) e o trecho em caracteres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarcaDoErro {
    pub linha: usize,
    pub caracteres: Range<usize>,
}

/// O estado da validação e da estrutura de um JSON.
#[derive(Default)]
pub(crate) struct EstadoJson {
    /// A última estrutura válida.
    pub valida: Option<Valida>,
    /// O erro da análise mais nova (None: válido, ou ainda sem análise).
    pub erro: Option<ErroJson>,
    /// A versão do texto da análise mais nova que chegou.
    pub versao_respondida: Option<u64>,
    /// O texto está válido, mas é grande demais para montar a estrutura.
    pub sem_estrutura: bool,
    /// Sobe a cada resposta aplicada (os caches que dependem da estrutura se refazem).
    pub geracao: u64,
    /// Quantas análises já foram pedidas (os testes contam).
    pub pedidas: usize,
    /// Quantas respostas velhas foram descartadas (os testes contam).
    pub descartadas: usize,
    pendente: Option<Promessa<Resposta>>,
    devido_em: Option<f64>,
    // a estrutura do documento
    /// Os nós abertos, pela chave do caminho ([`crate::estrutura::chave_do_no`]).
    pub abertos: HashSet<String>,
    /// Quantos filhos de cada nó aberto estão à vista (de 200 em 200).
    pub mostrados: HashMap<String, usize>,
    /// O nó ativo do quadro anterior: quando muda, os pais dele se abrem.
    pub ativo_anterior: Option<String>,
    /// A raiz já foi aberta uma vez (ela abre sozinha ao aparecer a primeira estrutura).
    pub raiz_iniciada: bool,
}

impl EstadoJson {
    /// O estado com a análise que a carga já fez ao ler o arquivo.
    pub(crate) fn com_resposta(resposta: Resposta) -> EstadoJson {
        let mut s = EstadoJson::default();
        s.aplicar(resposta);
        s
    }

    /// Guarda uma resposta que valeu: o veredito e, se o texto é válido, a estrutura nova.
    pub(crate) fn aplicar(&mut self, r: Resposta) {
        self.geracao += 1;
        self.versao_respondida = Some(r.versao);
        match r.erro {
            Some(erro) => self.erro = Some(erro),
            None => {
                self.erro = None;
                self.sem_estrutura = r.arvore.is_none();
                self.valida = r.arvore.map(|arvore| Valida { arvore, texto: r.texto });
            }
        }
    }

    /// O texto mudou: valida de novo `ATRASO_DA_VALIDACAO` depois de `agora`.
    pub(crate) fn agendar(&mut self, agora: f64) {
        self.devido_em = Some(agora + ATRASO_DA_VALIDACAO);
    }

    /// Há análise esperando a hora ou rodando.
    pub(crate) fn ocupado(&self) -> bool {
        self.pendente.is_some() || self.devido_em.is_some()
    }

    /// O círculo da faixa.
    pub(crate) fn veredito(&self) -> Veredito {
        match (&self.erro, self.versao_respondida) {
            (Some(e), _) => Veredito::Invalido(e.clone()),
            (None, Some(_)) => Veredito::Valido,
            (None, None) => Veredito::Verificando,
        }
    }

    /// A análise mais nova é a do texto desta `versao`.
    pub(crate) fn em_dia(&self, versao: u64) -> bool {
        self.versao_respondida == Some(versao)
    }

    /// O que acontece a cada quadro: toma a resposta que chegou (e a descarta se o texto já é outro) e, passado o
    /// atraso, pede a análise do texto de agora (uma de cada vez).
    pub(crate) fn passo(&mut self, egui: &egui::Context, agora: f64, versao: u64, texto: &str) {
        if let Some(p) = self.pendente.as_mut()
            && let Some(resultado) = p.tomar()
        {
            self.pendente = None;
            match resultado {
                Ok(r) if r.versao == versao => self.aplicar(r),
                Ok(_) => self.descartadas += 1,
                Err(_) => {}
            }
        }
        let Some(devido) = self.devido_em else {
            return;
        };
        if agora < devido {
            egui.request_repaint_after(Duration::from_secs_f64((devido - agora).max(0.0)));
        } else if self.pendente.is_none() {
            self.devido_em = None;
            self.pedidas += 1;
            let copia = texto.to_string();
            self.pendente = Some(Promessa::rodar(egui, move || Ok(Resposta::analisar(versao, Arc::new(copia)))));
        } else {
            egui.request_repaint_after(Duration::from_millis(40));
        }
    }

    /// Onde marcar o erro no texto `texto` (o analisado): a linha dele e o símbolo que estraga.
    pub(crate) fn marca_do_erro(&self, texto: &str) -> Option<MarcaDoErro> {
        let e = self.erro.as_ref()?;
        let inicio = e.byte.min(texto.len());
        let fim = fim_do_simbolo(texto, inicio);
        let (a, b) = if fim > inicio { (inicio, fim) } else { trecho_anterior(texto, inicio) };
        Some(MarcaDoErro {
            linha: e.linha.saturating_sub(1),
            caracteres: caractere_do_byte(texto, a)..caractere_do_byte(texto, b),
        })
    }
}

/// O fim (em bytes) do símbolo que começa em `inicio`: a cadeia até a aspa de fechamento (ou o fim da linha), a
/// palavra ou o número, ou o caractere solto. Vazio no fim do texto.
pub(crate) fn fim_do_simbolo(texto: &str, inicio: usize) -> usize {
    let Some(c) = texto.get(inicio..).and_then(|r| r.chars().next()) else {
        return inicio;
    };
    let b = texto.as_bytes();
    match c {
        '"' => {
            let mut i = inicio + 1;
            while i < b.len() {
                match b[i] {
                    b'"' => return i + 1,
                    b'\n' => return i,
                    b'\\' if b.get(i + 1).is_some_and(|c| *c != b'\n') => i += 2,
                    _ => i += 1,
                }
            }
            b.len()
        }
        c if c.is_alphanumeric() || matches!(c, '-' | '+' | '.' | '_') => {
            let resto = &texto[inicio..];
            let n = resto
                .char_indices()
                .find(|&(_, c)| !(c.is_alphanumeric() || matches!(c, '-' | '+' | '.' | '_')))
                .map_or(resto.len(), |(n, _)| n);
            inicio + n
        }
        c => inicio + c.len_utf8(),
    }
}

/// O último símbolo antes de `byte` (para o erro "o documento termina antes…", que aponta o fim do texto): o caractere
/// que não é espaço mais perto, ou nada.
fn trecho_anterior(texto: &str, byte: usize) -> (usize, usize) {
    let antes = &texto[..byte];
    match antes.char_indices().rev().find(|(_, c)| !c.is_whitespace()) {
        Some((i, c)) => (i, i + c.len_utf8()),
        None => (byte, byte),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn fim_do_simbolo_pega_cadeia_palavra_numero_e_caractere() {
        let t = "{\n  \"arquivo\": 12.5e3, true x\n}";
        let i = t.find("\"arquivo\"").unwrap();
        assert_eq!(&t[i..fim_do_simbolo(t, i)], "\"arquivo\"");
        let n = t.find("12.5e3").unwrap();
        assert_eq!(&t[n..fim_do_simbolo(t, n)], "12.5e3");
        let v = t.find("true").unwrap();
        assert_eq!(&t[v..fim_do_simbolo(t, v)], "true");
        let d = t.find(':').unwrap();
        assert_eq!(&t[d..fim_do_simbolo(t, d)], ":");
        // cadeia sem fecho para na quebra; com escape não fecha na aspa escapada
        assert_eq!(fim_do_simbolo("\"abc\ndef", 0), 4);
        assert_eq!(fim_do_simbolo("\"a\\\"b\" x", 0), 6);
        assert_eq!(fim_do_simbolo("ab", 2), 2);
    }

    #[test]
    fn marca_do_erro_aponta_a_linha_e_o_simbolo_em_caracteres() {
        let t = "{\n  \"a\": 1\n  \"çb\": 2\n}";
        let analise = analisar(t);
        let mut s = EstadoJson::default();
        s.aplicar(Resposta { versao: 1, texto: Arc::new(t.to_string()), arvore: analise.arvore, erro: analise.erro });
        let m = s.marca_do_erro(t).expect("o erro");
        assert_eq!(m.linha, 2);
        let tokens: Vec<char> = t.chars().collect();
        let marcado: String = tokens[m.caracteres.clone()].iter().collect();
        assert_eq!(marcado, "\"çb\"");
    }

    fn resposta(versao: u64, texto: &str) -> Resposta {
        Resposta::analisar(versao, Arc::new(texto.to_string()))
    }

    #[test]
    fn validacao_espera_250_ms_e_pede_uma_analise_so() {
        let ctx = egui::Context::default();
        let mut s = EstadoJson::default();
        assert!(!s.ocupado());
        s.agendar(10.0);
        assert!(s.ocupado());
        // antes dos 250 ms: nada pedido
        s.passo(&ctx, 10.1, 1, "[1]");
        assert_eq!(s.pedidas, 0);
        s.passo(&ctx, 10.249, 1, "[1]");
        assert_eq!(s.pedidas, 0, "249 ms ainda não");
        // passados os 250 ms: uma análise, e só uma por mais quadros que haja
        s.passo(&ctx, 10.25, 1, "[1]");
        assert_eq!(s.pedidas, 1);
        s.passo(&ctx, 10.3, 1, "[1]");
        assert_eq!(s.pedidas, 1);
        // a resposta chega e vale
        let inicio = std::time::Instant::now();
        while s.versao_respondida.is_none() {
            s.passo(&ctx, 11.0, 1, "[1]");
            assert!(inicio.elapsed().as_secs() < 10, "a análise não terminou");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(s.veredito(), Veredito::Valido);
        assert!(s.valida.is_some() && !s.ocupado());
        assert_eq!(s.descartadas, 0);
    }

    #[test]
    fn resposta_de_versao_velha_e_descartada() {
        let ctx = egui::Context::default();
        // a análise da versão 1 chega quando o texto já é a versão 2: não vale
        let mut s = EstadoJson { pendente: Some(Promessa::pronta(Ok(resposta(1, "[1]")))), ..EstadoJson::default() };
        s.passo(&ctx, 0.0, 2, "[1, 2");
        assert_eq!((s.descartadas, s.versao_respondida, s.geracao), (1, None, 0));
        assert!(s.valida.is_none() && s.erro.is_none());
        // a da versão 2 vale (e traz o erro)
        s.pendente = Some(Promessa::pronta(Ok(resposta(2, "[1, 2"))));
        s.passo(&ctx, 0.0, 2, "[1, 2");
        assert_eq!((s.descartadas, s.versao_respondida, s.geracao), (1, Some(2), 1));
        assert!(matches!(s.veredito(), Veredito::Invalido(_)));
        // o erro não apaga a última estrutura válida: a versão 3 é válida e a 4, inválida de novo
        s.pendente = Some(Promessa::pronta(Ok(resposta(3, "[1, 2]"))));
        s.passo(&ctx, 0.0, 3, "[1, 2]");
        assert!(s.valida.is_some() && s.erro.is_none());
        s.pendente = Some(Promessa::pronta(Ok(resposta(4, "[1, 2"))));
        s.passo(&ctx, 0.0, 4, "[1, 2");
        assert!(s.valida.is_some(), "a última versão válida fica");
        assert!(s.erro.is_some() && s.em_dia(4) && !s.em_dia(3));
    }

    #[test]
    fn so_a_validacao_do_texto_de_agora_marca_o_erro() {
        let mut s = EstadoJson::default();
        let t = "{\"a\" 1}";
        let a = analisar(t);
        s.aplicar(Resposta { versao: 3, texto: Arc::new(t.to_string()), arvore: None, erro: a.erro });
        assert!(s.em_dia(3) && !s.em_dia(4), "o texto da versão 4 ainda não foi validado");
        assert!(s.marca_do_erro(t).is_some());
    }

    #[test]
    fn texto_grande_so_valida_e_nao_monta_a_estrutura() {
        let grande = format!("[{}0]", "0,".repeat(LIMITE_ESTRUTURA as usize / 2));
        assert!(grande.len() as u64 > LIMITE_ESTRUTURA);
        let r = Resposta::analisar(0, Arc::new(grande));
        assert!(r.arvore.is_none() && r.erro.is_none());
        let s = EstadoJson::com_resposta(r);
        assert!(s.sem_estrutura && s.valida.is_none());
        assert_eq!(s.veredito(), Veredito::Valido);
    }

    #[test]
    fn erro_no_fim_do_texto_marca_o_ultimo_simbolo() {
        let t = "{\"a\": 1";
        let analise = analisar(t);
        let mut s = EstadoJson::default();
        s.aplicar(Resposta { versao: 1, texto: Arc::new(t.to_string()), arvore: None, erro: analise.erro });
        let m = s.marca_do_erro(t).expect("o erro");
        assert_eq!(m.caracteres, 6..7);
    }
}
