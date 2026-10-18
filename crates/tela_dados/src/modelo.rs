//! O que a aba guarda entre um quadro e outro: a página na tela ([`Dados`]), as colunas como a grade as mostra
//! ([`ColunaVista`]), as linhas marcadas ([`Marcadas`]), a célula em edição ([`Edicao`]) e o sinal que põe as
//! gravações da grade em fila ([`Sinal`]).

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use banco::{ColunaInfo, Pagina, TabelaInfo, TipoChave};
use comum::{Celula, Chave};
use interface::icones::TipoColuna;

use crate::regras::{afinidade_numerica, chave_texto, rotulo_linha};

/// Uma coluna da grade.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColunaVista {
    pub nome: String,
    /// Tipo declarado ("" sem tipo).
    pub tipo: String,
    /// O tipo por extenso, na dica da pastilha do `th`: "TEXT", "sem tipo", "INTEGER · chave primária", "TEXT ·
    /// único", "INTEGER · gerada".
    pub rotulo_tipo: String,
    /// A pastilha do `th` pelo tipo declarado.
    pub pastilha: TipoColuna,
    pub numerica: bool,
    pub gerada: bool,
    /// A coluna é o apelido do rowid (`id INTEGER PRIMARY KEY`): as células dela abrem a linha.
    pub abre: bool,
}

/// A coluna visível que é o apelido do rowid numa tabela com rowid (a própria coluna abre a linha).
pub(crate) fn coluna_que_abre(info: &TabelaInfo) -> Option<&ColunaInfo> {
    match info.chave {
        Some(TipoChave::Rowid { .. }) => info.colunas.iter().find(|c| c.rowid_alias && c.oculta != 1),
        _ => None,
    }
}

/// As colunas da página com o que a grade precisa saber de cada uma.
pub(crate) fn montar_colunas(info: &TabelaInfo, p: &Pagina) -> Vec<ColunaVista> {
    let abre = coluna_que_abre(info).map(|c| c.nome.as_str());
    p.colunas
        .iter()
        .enumerate()
        .map(|(i, nome)| {
            let tipo = p.tipos.get(i).cloned().unwrap_or_default();
            let afinidade = p.afinidades.get(i).copied();
            let meta = info.colunas.iter().find(|c| &c.nome == nome);
            let gerada = meta.is_some_and(|m| m.gerada);
            // sem tipo declarado (visão, FTS) a afinidade é BLOB: aí vale o que os valores da página são
            let mut valores = p.linhas.iter().filter_map(|l| l.valores.get(i)).filter(|v| !v.e_nulo()).peekable();
            let numerica = afinidade.is_some_and(afinidade_numerica)
                || (tipo.is_empty() && valores.peek().is_some() && valores.all(Celula::e_numero));
            let mut partes = vec![if tipo.is_empty() { "sem tipo" } else { tipo.as_str() }];
            let pk = meta.is_some_and(|m| m.pk > 0);
            if pk {
                partes.push("chave primária");
            }
            if meta.is_some_and(|m| m.unica) {
                partes.push("único");
            }
            if gerada {
                partes.push("gerada");
            }
            let rotulo_tipo = partes.join(" · ");
            let pastilha = TipoColuna::do_declarado(&tipo, pk, meta.is_some_and(|m| m.fk.is_some()));
            ColunaVista {
                nome: nome.clone(),
                tipo,
                rotulo_tipo,
                pastilha,
                numerica,
                gerada,
                abre: abre == Some(nome.as_str()),
            }
        })
        .collect()
}

/// A página na tela, com o total quando já se sabe.
#[derive(Debug, Clone)]
pub(crate) struct Dados {
    /// Identidade desta página: uma gravação ou abertura feita sobre outra página não mexe nesta.
    pub geracao: u64,
    pub pagina: Pagina,
    /// A busca e os filtros com que ela foi lida (a chave da contagem).
    pub chave_filtro: String,
    pub total: Option<u64>,
    pub paginas: Option<u64>,
    /// Tempo da contagem à parte (tabela grande), para a dica da busca lenta.
    pub tempo_contagem: Option<f64>,
    /// A contagem à parte falhou: o motivo (vai na dica do "linhas não contadas").
    pub contagem_falhou: Option<String>,
    /// A chave de cada linha como texto (para marcar e achar a linha), na ordem das linhas.
    pub chaves: Vec<Option<String>>,
    /// O texto do número de cada linha (o abridor: o rowid, ou "abrir"), pronto para desenhar.
    pub rotulos: Vec<String>,
    pub colunas: Vec<ColunaVista>,
}

impl Dados {
    pub fn nova(geracao: u64, pagina: Pagina, chave_filtro: String, info: &TabelaInfo) -> Dados {
        let chaves = pagina.linhas.iter().map(|l| l.chave.as_ref().map(chave_texto)).collect();
        let rotulos = pagina.linhas.iter().map(|l| rotulo_linha(l.chave.as_ref())).collect();
        let colunas = montar_colunas(info, &pagina);
        Dados {
            geracao,
            total: pagina.total,
            paginas: pagina.paginas,
            pagina,
            chave_filtro,
            tempo_contagem: None,
            contagem_falhou: None,
            chaves,
            rotulos,
            colunas,
        }
    }

    /// O texto do número da linha.
    pub fn rotulo(&self, linha: usize) -> &str {
        self.rotulos.get(linha).map_or("", String::as_str)
    }

    pub fn n_linhas(&self) -> usize {
        self.pagina.linhas.len()
    }

    pub fn celula(&self, linha: usize, coluna: usize) -> Option<&Celula> {
        self.pagina.linhas.get(linha)?.valores.get(coluna)
    }

    pub fn chave(&self, linha: usize) -> Option<&Chave> {
        self.pagina.linhas.get(linha)?.chave.as_ref()
    }

    /// O índice da linha com esta chave (texto), se estiver na página.
    pub fn linha_da_chave(&self, chave: &str) -> Option<usize> {
        self.chaves.iter().position(|c| c.as_deref() == Some(chave))
    }

    /// Completa o total (a contagem à parte chegou, ou a última página já disse quantas linhas há).
    pub fn completar_total(&mut self, total: u64) {
        self.total = Some(total);
        self.paginas = Some(crate::regras::paginas_de(total, self.pagina.por_pagina));
    }

    /// Troca a chave de uma linha (a gravação devolveu outra: a PK foi editada).
    pub fn trocar_chave(&mut self, linha: usize, chave: Chave) {
        if let (Some(l), Some(t), Some(r)) =
            (self.pagina.linhas.get_mut(linha), self.chaves.get_mut(linha), self.rotulos.get_mut(linha))
        {
            *t = Some(chave_texto(&chave));
            *r = rotulo_linha(Some(&chave));
            l.chave = Some(chave);
        }
    }
}

/// As linhas marcadas, na ordem em que foram marcadas, com busca, marcação e desmarcação em tempo constante pela
/// chave em texto (desmarcar a página inteira de uma vez não vira um quadrado do tamanho dela).
#[derive(Debug, Clone, Default)]
pub(crate) struct Marcadas {
    /// Cada marcada pela chave em texto: o número de ordem em que foi marcada e a chave.
    itens: HashMap<String, (u64, Chave)>,
    /// O número de ordem da próxima a marcar.
    proxima: u64,
}

impl Marcadas {
    pub fn contem(&self, chave: &str) -> bool {
        self.itens.contains_key(chave)
    }

    pub fn len(&self) -> usize {
        self.itens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.itens.is_empty()
    }

    /// Marca ou desmarca; devolve se mudou.
    pub fn definir(&mut self, texto: &str, chave: &Chave, sim: bool) -> bool {
        if sim {
            if self.itens.contains_key(texto) {
                return false;
            }
            self.itens.insert(texto.to_string(), (self.proxima, chave.clone()));
            self.proxima += 1;
            true
        } else {
            self.itens.remove(texto).is_some()
        }
    }

    /// A chave de uma linha marcada mudou (a PK foi editada): continua marcada com a nova, no mesmo lugar da ordem.
    pub fn trocar(&mut self, antiga: &str, nova: &Chave) {
        if let Some((ordem, _)) = self.itens.remove(antiga) {
            self.itens.insert(chave_texto(nova), (ordem, nova.clone()));
        }
    }

    pub fn limpar(&mut self) {
        self.itens.clear();
    }

    /// A primeira que foi marcada (a única, quando só há uma), sem copiar nada.
    pub fn primeira(&self) -> Option<&Chave> {
        self.itens.values().min_by_key(|(ordem, _)| *ordem).map(|(_, c)| c)
    }

    /// As chaves na ordem em que foram marcadas (uma cópia: só para quando a seleção muda ou a exclusão começa).
    pub fn chaves(&self) -> Vec<Chave> {
        let mut todas: Vec<&(u64, Chave)> = self.itens.values().collect();
        todas.sort_by_key(|(ordem, _)| *ordem);
        todas.into_iter().map(|(_, c)| c.clone()).collect()
    }
}

/// A célula em edição no lugar.
#[derive(Debug, Clone)]
pub(crate) struct Edicao {
    pub geracao: u64,
    pub linha: usize,
    /// Índice da coluna de dados.
    pub coluna: usize,
    pub texto: String,
    /// O valor de antes (volta com o Esc ou com um erro).
    pub original: Celula,
    pub numerica: bool,
    /// O nome para o leitor de tela ("nome da linha 3").
    pub rotulo: String,
    /// O campo já recebeu o foco (perder depois disso grava).
    pub focada: bool,
    /// Pedir o foco para o campo no quadro em que ele aparece.
    pub pedir_foco: bool,
}

impl Edicao {
    /// O id do campo (o mesmo `id_salt` da `Entrada`).
    pub fn id(&self) -> egui::Id {
        egui::Id::new(self.salt())
    }

    pub fn salt(&self) -> (&'static str, u64, usize, usize) {
        ("tela_dados::edicao", self.geracao, self.linha, self.coluna)
    }
}

/// Como uma edição no lugar terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FimEdicao {
    /// Enter (`foco`: a grade volta a ter o foco) ou o campo perdeu o foco.
    Salvar { foco: bool },
    /// Esc.
    Cancelar,
    /// Tab / Shift+Tab: grava e vai para a célula seguinte (ou anterior).
    Tab { voltando: bool },
}

/// Um sinal de "terminei" entre as gravações da fila: cada uma espera a anterior antes de ler a chave da linha.
#[derive(Debug, Default)]
pub(crate) struct Sinal {
    feito: Mutex<bool>,
    cv: Condvar,
}

impl Sinal {
    pub fn esperar(&self) {
        let mut feito = self.feito.lock().unwrap_or_else(|e| e.into_inner());
        while !*feito {
            feito = self.cv.wait(feito).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn avisar(&self) {
        *self.feito.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cv.notify_all();
    }
}

/// Avisa o sinal ao sair (também num erro ou num pânico da tarefa): a fila nunca fica presa.
pub(crate) struct AvisarAoSair(pub Arc<Sinal>);

impl Drop for AvisarAoSair {
    fn drop(&mut self) {
        self.0.avisar();
    }
}

/// A chave nova de cada linha da página (geração, índice) que uma gravação da fila trocou e a aba ainda pode não
/// ter aplicado: a gravação seguinte da mesma linha usa essa (lida da própria linha na hora de gravar). É a linha que manda, não a chave antiga: trocar A1 → A1X numa linha e depois B2 →
/// A1 em outra não faz uma gravação da segunda ir parar na primeira. A aba esvazia o mapa quando a fila esvazia
/// (aí a página já tem todas as chaves novas).
pub(crate) type ChavesDaFila = Arc<Mutex<HashMap<(u64, usize), Chave>>>;

/// A chave com que a gravação da linha (geração, índice) sai: a que a fila deixou nela, ou a lida da página ao
/// entrar na fila.
pub(crate) fn chave_na_fila(chaves: &ChavesDaFila, geracao: u64, linha: usize, lida: Chave) -> Chave {
    let mapa = chaves.lock().unwrap_or_else(|e| e.into_inner());
    mapa.get(&(geracao, linha)).cloned().unwrap_or(lida)
}

/// Guarda a chave nova que uma gravação da fila deu à linha (geração, índice).
pub(crate) fn trocar_na_fila(chaves: &ChavesDaFila, geracao: u64, linha: usize, nova: Chave) {
    chaves.lock().unwrap_or_else(|e| e.into_inner()).insert((geracao, linha), nova);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn marcadas_mantem_a_ordem_e_trocam_de_chave() {
        let mut m = Marcadas::default();
        let (a, b) = (Chave::Rowid(1), Chave::Rowid(2));
        assert!(m.definir(&chave_texto(&a), &a, true));
        assert!(!m.definir(&chave_texto(&a), &a, true), "marcar de novo não muda");
        assert!(m.definir(&chave_texto(&b), &b, true));
        assert_eq!(m.chaves(), vec![a.clone(), b.clone()]);
        m.trocar(&chave_texto(&a), &Chave::Rowid(9));
        assert!(m.contem(&chave_texto(&Chave::Rowid(9))));
        assert!(!m.contem(&chave_texto(&a)));
        // a trocada continua no lugar dela na ordem
        assert_eq!(m.chaves(), vec![Chave::Rowid(9), b.clone()]);
        assert_eq!(m.primeira(), Some(&Chave::Rowid(9)));
        assert!(m.definir(&chave_texto(&b), &b, false));
        assert!(!m.definir(&chave_texto(&b), &b, false), "desmarcar de novo não muda");
        assert_eq!(m.len(), 1);
        assert_eq!(m.primeira(), Some(&Chave::Rowid(9)));
        // a que é marcada depois vai para o fim, mesmo depois de outras saírem
        assert!(m.definir(&chave_texto(&b), &b, true));
        assert_eq!(m.chaves(), vec![Chave::Rowid(9), b]);
    }

    #[test]
    fn chave_segue_a_linha_e_nao_a_chave_antiga() {
        let k = |t: &str| Chave::Pk(vec![("k".into(), comum::Valor::Texto(t.into()))]);
        let chaves: ChavesDaFila = Arc::default();
        assert_eq!(chave_na_fila(&chaves, 1, 0, k("A1")), k("A1"));
        // linha 0: A1 → A1X; linha 1: B2 → A1
        trocar_na_fila(&chaves, 1, 0, k("A1X"));
        assert_eq!(chave_na_fila(&chaves, 1, 0, k("A1")), k("A1X"));
        trocar_na_fila(&chaves, 1, 1, k("A1"));
        // a gravação seguinte da linha 1 vai para A1 (a própria linha), não segue A1 → A1X da outra
        assert_eq!(chave_na_fila(&chaves, 1, 1, k("B2")), k("A1"));
        assert_eq!(chave_na_fila(&chaves, 1, 1, k("A1")), k("A1"));
        // outra página (outra geração) não herda nada
        assert_eq!(chave_na_fila(&chaves, 2, 0, k("A1")), k("A1"));
    }

    #[test]
    fn sinal_libera_quem_espera() {
        let s = Arc::new(Sinal::default());
        let s2 = Arc::clone(&s);
        let t = std::thread::spawn(move || s2.esperar());
        drop(AvisarAoSair(Arc::clone(&s)));
        t.join().unwrap();
    }
}
