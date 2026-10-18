//! O modelo das alterações de um CSV: o arquivo continua como está no disco, e o que a pessoa mudou fica à
//! parte, em estruturas pequenas, até o Salvar. Nada aqui lê ou grava arquivo.
//!
//! - **Colunas:** a lista das colunas que a grade mostra, cada uma com um [`IdColuna`] que não muda (as do
//!   arquivo guardam o número da coluna de origem; as novas, não). Inserir, excluir e renomear mexem só nela.
//! - **Linhas:** uma lista de trechos, cada um com um intervalo de linhas do arquivo ou uma linha nova.
//!   Excluir parte um trecho em dois; inserir põe uma linha nova entre eles. Um arquivo de 1 milhão de linhas
//!   com 3 exclusões tem 4 trechos, não 1 milhão de entradas.
//! - **Células:** só as que mudaram, por (linha, coluna). As de uma linha nova ou de uma coluna nova que ficaram
//!   vazias não têm entrada.
//! - **Desfazer e refazer:** cada operação guarda o que desfaz (uma fotografia das listas que mudaram e o valor
//!   antigo de cada célula tocada); aplicar um desses "remendos" devolve o remendo inverso, e é assim que o
//!   refazer existe. Operações compostas (colar uma coluna, trazer colunas) entram num grupo e valem um passo só.

use std::collections::BTreeMap;
use std::mem;

use comum::{ErroBanco, Resultado};

/// A identidade de uma coluna, que não muda com inserções e exclusões.
pub type IdColuna = u32;

/// Uma coluna que a grade mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColunaVirtual {
    pub id: IdColuna,
    pub nome: String,
    /// O número da coluna no arquivo; `None` numa coluna criada aqui.
    pub origem: Option<usize>,
}

/// A identidade de uma linha: uma do arquivo (pelo número, de 0, sem o cabeçalho) ou uma criada aqui. A ordem
/// põe as do arquivo na frente, o que ajuda a percorrer as células na ordem do arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChaveLinha {
    Base(u64),
    Nova(u64),
}

/// Um pedaço da lista de linhas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trecho {
    /// As linhas `de..ate` do arquivo, nessa ordem.
    Base { de: u64, ate: u64 },
    /// Uma linha criada aqui.
    Nova(u64),
}

impl Trecho {
    fn tamanho(&self) -> u64 {
        match self {
            Trecho::Base { de, ate } => ate - de,
            Trecho::Nova(_) => 1,
        }
    }
}

type MapaDeCelulas<V> = BTreeMap<(ChaveLinha, IdColuna), V>;

/// O que desfaz uma operação.
#[derive(Debug, Clone, Default)]
struct Passo {
    colunas: Option<Vec<ColunaVirtual>>,
    trechos: Option<Vec<Trecho>>,
    /// O valor antigo de cada célula tocada (`None`: não tinha entrada).
    celulas: MapaDeCelulas<Option<String>>,
}

impl Passo {
    fn vazio(&self) -> bool {
        self.colunas.is_none() && self.trechos.is_none() && self.celulas.is_empty()
    }

    /// Junta um passo posterior a este: vale o estado mais antigo de cada parte.
    fn juntar(&mut self, depois: Passo) {
        if self.colunas.is_none() {
            self.colunas = depois.colunas;
        }
        if self.trechos.is_none() {
            self.trechos = depois.trechos;
        }
        for (chave, antigo) in depois.celulas {
            self.celulas.entry(chave).or_insert(antigo);
        }
    }
}

fn linha_inexistente(linha: u64) -> ErroBanco {
    ErroBanco::novo(format!("A linha {} não existe.", linha.saturating_add(1)))
}

fn coluna_inexistente(coluna: usize) -> ErroBanco {
    ErroBanco::novo(format!("A coluna {} não existe.", coluna.saturating_add(1)))
}

/// As alterações de uma tabela em relação ao arquivo de onde ela veio.
#[derive(Debug, Clone)]
pub struct Edicoes {
    colunas: Vec<ColunaVirtual>,
    trechos: Vec<Trecho>,
    celulas: MapaDeCelulas<String>,
    colunas_base: usize,
    linhas_base: u64,
    proximo_id_coluna: IdColuna,
    proximo_id_linha: u64,
    desfazer: Vec<Passo>,
    refazer: Vec<Passo>,
    grupo: Option<Passo>,
}

impl Edicoes {
    /// Sem alterações: as colunas com os `nomes` dados e as `linhas_base` linhas do arquivo.
    pub fn nova(nomes: &[String], linhas_base: u64) -> Edicoes {
        let colunas: Vec<ColunaVirtual> = nomes
            .iter()
            .enumerate()
            .map(|(i, n)| ColunaVirtual {
                id: IdColuna::try_from(i).unwrap_or(IdColuna::MAX),
                nome: n.clone(),
                origem: Some(i),
            })
            .collect();
        Edicoes {
            colunas_base: colunas.len(),
            proximo_id_coluna: IdColuna::try_from(colunas.len()).unwrap_or(IdColuna::MAX),
            colunas,
            trechos: if linhas_base > 0 { vec![Trecho::Base { de: 0, ate: linhas_base }] } else { Vec::new() },
            celulas: BTreeMap::new(),
            linhas_base,
            proximo_id_linha: 0,
            desfazer: Vec::new(),
            refazer: Vec::new(),
            grupo: None,
        }
    }

    // ---- leitura -------------------------------------------------------------------------------------

    pub fn colunas(&self) -> &[ColunaVirtual] {
        &self.colunas
    }

    pub fn n_colunas(&self) -> usize {
        self.colunas.len()
    }

    pub fn trechos(&self) -> &[Trecho] {
        &self.trechos
    }

    /// Quantas linhas a grade mostra.
    pub fn total_linhas(&self) -> u64 {
        self.trechos.iter().map(Trecho::tamanho).sum()
    }

    /// O valor que a pessoa pôs numa célula, se pôs.
    pub fn celula_editada(&self, chave: ChaveLinha, coluna: IdColuna) -> Option<&str> {
        self.celulas.get(&(chave, coluna)).map(String::as_str)
    }

    /// As células alteradas, na ordem das linhas (as do arquivo primeiro) e das colunas.
    pub fn celulas(&self) -> impl Iterator<Item = (ChaveLinha, IdColuna, &str)> + '_ {
        self.celulas.iter().map(|((l, c), v)| (*l, *c, v.as_str()))
    }

    /// As células alteradas de uma linha, na ordem das colunas.
    pub fn celulas_da_linha(&self, chave: ChaveLinha) -> impl Iterator<Item = (IdColuna, &str)> + '_ {
        self.celulas.range((chave, IdColuna::MIN)..=(chave, IdColuna::MAX)).map(|((_, c), v)| (*c, v.as_str()))
    }

    pub fn tem_celulas_alteradas(&self) -> bool {
        !self.celulas.is_empty()
    }

    /// A identidade da linha que está na posição `linha` da grade.
    pub fn chave_da_linha(&self, linha: u64) -> Option<ChaveLinha> {
        let mut acumulado = 0;
        for t in &self.trechos {
            let tamanho = t.tamanho();
            if linha < acumulado + tamanho {
                return Some(match t {
                    Trecho::Base { de, .. } => ChaveLinha::Base(de + (linha - acumulado)),
                    Trecho::Nova(id) => ChaveLinha::Nova(*id),
                });
            }
            acumulado += tamanho;
        }
        None
    }

    /// A identidade de cada uma das `linhas` (posições da grade, em qualquer ordem): uma passada só pelos trechos,
    /// em vez de uma [`Edicoes::chave_da_linha`] por linha (que percorre os trechos a cada vez; depois de muitas
    /// exclusões salteadas, são muitos).
    fn chaves_das_linhas(&self, linhas: &[u64]) -> Vec<Option<ChaveLinha>> {
        let mut ordem: Vec<usize> = (0..linhas.len()).collect();
        ordem.sort_unstable_by_key(|&i| linhas[i]);
        let mut saida = vec![None; linhas.len()];
        let mut trechos = self.trechos.iter();
        let mut atual = trechos.next();
        let mut acumulado = 0u64;
        for i in ordem {
            let linha = linhas[i];
            while let Some(t) = atual {
                if linha < acumulado + t.tamanho() {
                    break;
                }
                acumulado += t.tamanho();
                atual = trechos.next();
            }
            let Some(t) = atual else { break };
            saida[i] = Some(match t {
                Trecho::Base { de, .. } => ChaveLinha::Base(de + (linha - acumulado)),
                Trecho::Nova(id) => ChaveLinha::Nova(*id),
            });
        }
        saida
    }

    /// As identidades das linhas `inicio..inicio + quantidade` da grade.
    pub fn chaves(&self, inicio: u64, quantidade: usize) -> Vec<ChaveLinha> {
        let mut saida = Vec::with_capacity(quantidade.min(usize::try_from(self.total_linhas()).unwrap_or(usize::MAX)));
        if quantidade == 0 {
            return saida;
        }
        let mut acumulado = 0u64;
        for t in &self.trechos {
            let tamanho = t.tamanho();
            if acumulado + tamanho > inicio {
                let dentro = inicio.saturating_sub(acumulado);
                match t {
                    Trecho::Base { de, ate } => {
                        let mut r = de + dentro;
                        while r < *ate && saida.len() < quantidade {
                            saida.push(ChaveLinha::Base(r));
                            r += 1;
                        }
                    }
                    Trecho::Nova(id) => saida.push(ChaveLinha::Nova(*id)),
                }
                if saida.len() >= quantidade {
                    break;
                }
            }
            acumulado += tamanho;
        }
        saida
    }

    /// Todas as linhas da grade, na ordem.
    pub fn percorrer_chaves(&self) -> Box<dyn Iterator<Item = ChaveLinha> + '_> {
        Box::new(self.trechos.iter().flat_map(|t| -> Box<dyn Iterator<Item = ChaveLinha>> {
            match t {
                Trecho::Base { de, ate } => Box::new((*de..*ate).map(ChaveLinha::Base)),
                Trecho::Nova(id) => Box::new(std::iter::once(ChaveLinha::Nova(*id))),
            }
        }))
    }

    /// As colunas são exatamente as do arquivo, na mesma ordem (só os nomes podem ter mudado).
    pub fn colunas_como_no_arquivo(&self) -> bool {
        self.colunas.len() == self.colunas_base && self.colunas.iter().enumerate().all(|(i, c)| c.origem == Some(i))
    }

    /// As linhas são exatamente as do arquivo, na mesma ordem.
    pub fn linhas_como_no_arquivo(&self) -> bool {
        match self.trechos.as_slice() {
            [] => self.linhas_base == 0,
            [Trecho::Base { de: 0, ate }] => *ate == self.linhas_base,
            _ => false,
        }
    }

    /// Há alguma alteração para salvar.
    pub fn alterada(&self) -> bool {
        !self.desfazer.is_empty() || self.grupo.as_ref().is_some_and(|g| !g.vazio())
    }

    /// Quantos passos do desfazer estão aplicados (os que o Desfazer percorreria), contando o grupo aberto, se
    /// ele já mudou alguma coisa, como um passo. Zero quando, e só quando, [`Edicoes::alterada`] é falso.
    pub fn passos_aplicados(&self) -> usize {
        self.desfazer.len() + usize::from(self.grupo.as_ref().is_some_and(|g| !g.vazio()))
    }

    /// Quantos passos o Refazer percorreria.
    pub fn passos_desfeitos(&self) -> usize {
        self.refazer.len()
    }

    pub fn pode_desfazer(&self) -> bool {
        !self.desfazer.is_empty()
    }

    pub fn pode_refazer(&self) -> bool {
        !self.refazer.is_empty()
    }

    // ---- desfazer e grupos ---------------------------------------------------------------------------

    fn registrar(&mut self, passo: Passo) {
        if passo.vazio() {
            return;
        }
        match self.grupo.as_mut() {
            Some(g) => g.juntar(passo),
            None => {
                self.desfazer.push(passo);
                self.refazer.clear();
            }
        }
    }

    /// Aplica um remendo e devolve o que o desfaz.
    fn aplicar(&mut self, passo: Passo) -> Passo {
        let mut inverso = Passo::default();
        if let Some(c) = passo.colunas {
            inverso.colunas = Some(mem::replace(&mut self.colunas, c));
        }
        if let Some(t) = passo.trechos {
            inverso.trechos = Some(mem::replace(&mut self.trechos, t));
        }
        for (chave, valor) in passo.celulas {
            let antigo = match valor {
                Some(v) => self.celulas.insert(chave, v),
                None => self.celulas.remove(&chave),
            };
            inverso.celulas.insert(chave, antigo);
        }
        inverso
    }

    /// Desfaz a última operação (ou grupo); `false` se não há o que desfazer.
    pub fn desfazer(&mut self) -> bool {
        if self.grupo.is_some() {
            return false;
        }
        let Some(passo) = self.desfazer.pop() else {
            return false;
        };
        let inverso = self.aplicar(passo);
        self.refazer.push(inverso);
        true
    }

    pub fn refazer(&mut self) -> bool {
        if self.grupo.is_some() {
            return false;
        }
        let Some(passo) = self.refazer.pop() else {
            return false;
        };
        let inverso = self.aplicar(passo);
        self.desfazer.push(inverso);
        true
    }

    /// Há um grupo aberto.
    pub fn grupo_aberto(&self) -> bool {
        self.grupo.is_some()
    }

    /// Começa um grupo: tudo o que for feito até [`Edicoes::fechar_grupo`] vale um passo só no desfazer.
    pub fn abrir_grupo(&mut self) {
        if self.grupo.is_none() {
            self.grupo = Some(Passo::default());
        }
    }

    /// Fecha o grupo: o que foi feito vira um passo no desfazer (nenhum, se nada mudou).
    pub fn fechar_grupo(&mut self) {
        if let Some(g) = self.grupo.take() {
            self.registrar(g);
        }
    }

    /// Fecha o grupo desfazendo tudo o que ele fez (uma operação do grupo falhou no meio).
    pub fn descartar_grupo(&mut self) {
        if let Some(g) = self.grupo.take() {
            self.aplicar(g);
        }
    }

    // ---- células -------------------------------------------------------------------------------------

    fn id_da_coluna(&self, coluna: usize) -> Resultado<IdColuna> {
        self.colunas.get(coluna).map(|c| c.id).ok_or_else(|| coluna_inexistente(coluna))
    }

    /// Muda uma célula; `None` volta ao valor do arquivo (ou a vazio, numa linha ou coluna nova).
    pub fn definir_celula(&mut self, linha: u64, coluna: usize, valor: Option<String>) -> Resultado<()> {
        self.definir_celulas([(linha, coluna, valor)])
    }

    /// Muda várias células num passo só. Tudo ou nada: se uma posição não existe, nada muda.
    pub fn definir_celulas(&mut self, itens: impl IntoIterator<Item = (u64, usize, Option<String>)>) -> Resultado<()> {
        let itens: Vec<(u64, usize, Option<String>)> = itens.into_iter().collect();
        let linhas: Vec<u64> = itens.iter().map(|(l, _, _)| *l).collect();
        let chaves = self.chaves_das_linhas(&linhas);
        let mut alvos = Vec::with_capacity(itens.len());
        for ((linha, coluna, valor), chave) in itens.into_iter().zip(chaves) {
            let chave = chave.ok_or_else(|| linha_inexistente(linha))?;
            alvos.push(((chave, self.id_da_coluna(coluna)?), valor));
        }
        let mut passo = Passo::default();
        for (chave, valor) in alvos {
            let antigo = self.celulas.get(&chave).cloned();
            if antigo == valor {
                continue;
            }
            match valor {
                Some(v) => self.celulas.insert(chave, v),
                None => self.celulas.remove(&chave),
            };
            passo.celulas.entry(chave).or_insert(antigo);
        }
        self.registrar(passo);
        Ok(())
    }

    // ---- linhas --------------------------------------------------------------------------------------

    /// Insere uma linha nova na posição `posicao` da grade (`total_linhas()` põe no fim).
    pub fn inserir_linha(&mut self, posicao: u64) -> Resultado<ChaveLinha> {
        let chaves = self.inserir_linhas(posicao, 1)?;
        chaves.into_iter().next().ok_or_else(|| ErroBanco::com_status("Nenhuma linha foi inserida.", 500))
    }

    /// Insere `quantidade` linhas novas a partir da posição `posicao`, num passo só.
    pub fn inserir_linhas(&mut self, posicao: u64, quantidade: u64) -> Resultado<Vec<ChaveLinha>> {
        if posicao > self.total_linhas() {
            return Err(linha_inexistente(posicao));
        }
        if quantidade == 0 {
            return Ok(Vec::new());
        }
        let antes = self.trechos.clone();
        let mut chaves = Vec::new();
        for i in 0..quantidade {
            let id = self.proximo_id_linha;
            self.proximo_id_linha += 1;
            Self::inserir_trecho(&mut self.trechos, posicao + i, id);
            chaves.push(ChaveLinha::Nova(id));
        }
        self.registrar(Passo { trechos: Some(antes), ..Passo::default() });
        Ok(chaves)
    }

    fn inserir_trecho(trechos: &mut Vec<Trecho>, posicao: u64, id: u64) {
        let mut acumulado = 0;
        for i in 0..trechos.len() {
            let tamanho = trechos[i].tamanho();
            if posicao == acumulado {
                trechos.insert(i, Trecho::Nova(id));
                return;
            }
            if posicao < acumulado + tamanho
                && let Trecho::Base { de, ate } = trechos[i].clone()
            {
                let corte = de + (posicao - acumulado);
                trechos.splice(
                    i..=i,
                    [Trecho::Base { de, ate: corte }, Trecho::Nova(id), Trecho::Base { de: corte, ate }],
                );
                return;
            }
            acumulado += tamanho;
        }
        trechos.push(Trecho::Nova(id));
    }

    /// Exclui as linhas das posições dadas (da grade), num passo só.
    pub fn excluir_linhas(&mut self, linhas: &[u64]) -> Resultado<()> {
        let mut alvo: Vec<u64> = linhas.to_vec();
        alvo.sort_unstable();
        alvo.dedup();
        let Some(&maior) = alvo.last() else {
            return Ok(());
        };
        if maior >= self.total_linhas() {
            return Err(linha_inexistente(maior));
        }
        let mut novos = Vec::with_capacity(self.trechos.len() + alvo.len());
        let mut acumulado = 0u64;
        let mut k = 0;
        for t in &self.trechos {
            let tamanho = t.tamanho();
            let fim = acumulado + tamanho;
            match t {
                Trecho::Nova(id) => {
                    if alvo.get(k) == Some(&acumulado) {
                        k += 1;
                    } else {
                        novos.push(Trecho::Nova(*id));
                    }
                }
                Trecho::Base { de, ate } => {
                    let mut cursor = *de;
                    while let Some(&p) = alvo.get(k).filter(|&&p| p < fim) {
                        let r = de + (p - acumulado);
                        if r > cursor {
                            novos.push(Trecho::Base { de: cursor, ate: r });
                        }
                        cursor = r + 1;
                        k += 1;
                    }
                    if cursor < *ate {
                        novos.push(Trecho::Base { de: cursor, ate: *ate });
                    }
                }
            }
            acumulado = fim;
        }
        let antes = mem::replace(&mut self.trechos, novos);
        self.registrar(Passo { trechos: Some(antes), ..Passo::default() });
        Ok(())
    }

    // ---- colunas -------------------------------------------------------------------------------------

    /// Insere uma coluna nova, vazia, na posição `posicao` (`n_colunas()` põe no fim).
    pub fn inserir_coluna(&mut self, posicao: usize, nome: String) -> Resultado<IdColuna> {
        if posicao > self.colunas.len() {
            return Err(coluna_inexistente(posicao));
        }
        let id = self.proximo_id_coluna;
        self.proximo_id_coluna += 1;
        let antes = self.colunas.clone();
        self.colunas.insert(posicao, ColunaVirtual { id, nome, origem: None });
        self.registrar(Passo { colunas: Some(antes), ..Passo::default() });
        Ok(id)
    }

    /// Exclui as colunas das posições dadas, num passo só. A última coluna não se exclui.
    pub fn excluir_colunas(&mut self, colunas: &[usize]) -> Resultado<()> {
        let mut alvo: Vec<usize> = colunas.to_vec();
        alvo.sort_unstable();
        alvo.dedup();
        let Some(&maior) = alvo.last() else {
            return Ok(());
        };
        if maior >= self.colunas.len() {
            return Err(coluna_inexistente(maior));
        }
        if alvo.len() >= self.colunas.len() {
            return Err(ErroBanco::novo("O arquivo precisa ficar com pelo menos uma coluna."));
        }
        let antes = self.colunas.clone();
        let mut i = 0;
        self.colunas.retain(|_| {
            let fica = alvo.binary_search(&i).is_err();
            i += 1;
            fica
        });
        self.registrar(Passo { colunas: Some(antes), ..Passo::default() });
        Ok(())
    }

    pub fn renomear_coluna(&mut self, coluna: usize, nome: String) -> Resultado<()> {
        let atual = self.colunas.get(coluna).ok_or_else(|| coluna_inexistente(coluna))?;
        if atual.nome == nome {
            return Ok(());
        }
        let antes = self.colunas.clone();
        self.colunas[coluna].nome = nome;
        self.registrar(Passo { colunas: Some(antes), ..Passo::default() });
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn nomes(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("c{i}")).collect()
    }

    fn edicoes() -> Edicoes {
        Edicoes::nova(&nomes(3), 10)
    }

    fn base(de: u64, ate: u64) -> Trecho {
        Trecho::Base { de, ate }
    }

    fn chaves(e: &Edicoes) -> Vec<ChaveLinha> {
        e.percorrer_chaves().collect()
    }

    fn b(r: u64) -> ChaveLinha {
        ChaveLinha::Base(r)
    }

    #[test]
    fn comeca_sem_alteracao() {
        let e = edicoes();
        assert_eq!(e.total_linhas(), 10);
        assert_eq!(e.n_colunas(), 3);
        assert!(!e.alterada() && !e.pode_desfazer() && !e.pode_refazer());
        assert!(e.colunas_como_no_arquivo() && e.linhas_como_no_arquivo());
        assert_eq!(e.trechos(), &[base(0, 10)]);
        assert_eq!(e.chave_da_linha(0), Some(b(0)));
        assert_eq!(e.chave_da_linha(9), Some(b(9)));
        assert_eq!(e.chave_da_linha(10), None);
        let vazia = Edicoes::nova(&nomes(2), 0);
        assert!(vazia.trechos().is_empty() && vazia.linhas_como_no_arquivo() && vazia.total_linhas() == 0);
    }

    #[test]
    fn celulas_editam_e_voltam_ao_valor_do_arquivo() {
        let mut e = edicoes();
        e.definir_celula(2, 1, Some("novo".into())).unwrap();
        let id = e.colunas()[1].id;
        assert_eq!(e.celula_editada(b(2), id), Some("novo"));
        assert!(e.alterada() && e.tem_celulas_alteradas());
        assert_eq!(e.celulas().collect::<Vec<_>>(), vec![(b(2), id, "novo")]);
        e.definir_celula(2, 1, Some("novo".into())).unwrap();
        assert_eq!(e.desfazer.len(), 1, "o mesmo valor de novo não é outro passo");
        e.definir_celula(2, 1, None).unwrap();
        assert_eq!(e.celula_editada(b(2), id), None);
        assert_eq!(e.desfazer.len(), 2);
        e.definir_celula(5, 0, None).unwrap();
        assert_eq!(e.desfazer.len(), 2, "tirar o que não existe não é passo");
        assert!(e.definir_celula(10, 0, Some("x".into())).is_err());
        assert!(e.definir_celula(0, 3, Some("x".into())).is_err());
    }

    #[test]
    fn celulas_da_linha_em_ordem_de_coluna() {
        let mut e = edicoes();
        e.definir_celula(1, 2, Some("c".into())).unwrap();
        e.definir_celula(1, 0, Some("a".into())).unwrap();
        e.definir_celula(2, 1, Some("outra".into())).unwrap();
        assert_eq!(e.celulas_da_linha(b(1)).collect::<Vec<_>>(), vec![(0, "a"), (2, "c")]);
        assert_eq!(e.celulas_da_linha(b(0)).count(), 0);
    }

    #[test]
    fn definir_celulas_e_tudo_ou_nada() {
        let mut e = edicoes();
        let r = e.definir_celulas([(0, 0, Some("a".into())), (99, 0, Some("b".into()))]);
        assert!(r.is_err());
        assert!(!e.alterada() && !e.tem_celulas_alteradas());
        e.definir_celulas([(0, 0, Some("a".into())), (1, 1, Some("b".into()))]).unwrap();
        assert_eq!(e.desfazer.len(), 1);
        assert!(e.desfazer());
        assert!(!e.tem_celulas_alteradas());
    }

    #[test]
    fn chaves_das_linhas_numa_passada_batem_com_uma_por_uma() {
        let mut e = edicoes();
        // exclusões salteadas e linhas novas no meio: muitos trechos
        e.excluir_linhas(&[1, 3, 5]).unwrap();
        e.inserir_linha(2).unwrap();
        e.inserir_linha(0).unwrap();
        let linhas = [7, 0, 3, 3, 99, 2, e.total_linhas() - 1, e.total_linhas()];
        let uma_por_uma: Vec<Option<ChaveLinha>> = linhas.iter().map(|&l| e.chave_da_linha(l)).collect();
        assert_eq!(e.chaves_das_linhas(&linhas), uma_por_uma);
        assert_eq!(uma_por_uma[4], None);
        assert_eq!(uma_por_uma[7], None);
    }

    #[test]
    fn inserir_linha_no_meio_no_comeco_e_no_fim() {
        let mut e = edicoes();
        let k = e.inserir_linha(4).unwrap();
        assert_eq!(k, ChaveLinha::Nova(0));
        assert_eq!(e.trechos(), &[base(0, 4), Trecho::Nova(0), base(4, 10)]);
        assert_eq!(e.total_linhas(), 11);
        assert_eq!(e.chave_da_linha(4), Some(ChaveLinha::Nova(0)));
        assert_eq!(e.chave_da_linha(5), Some(b(4)));
        e.inserir_linha(0).unwrap();
        e.inserir_linha(e.total_linhas()).unwrap();
        assert_eq!(e.chave_da_linha(0), Some(ChaveLinha::Nova(1)));
        assert_eq!(e.chave_da_linha(12), Some(ChaveLinha::Nova(2)));
        assert_eq!(e.total_linhas(), 13);
        assert!(e.inserir_linha(14).is_err());
        assert!(!e.linhas_como_no_arquivo());
        // no limite entre dois trechos e logo antes de uma linha nova
        e.inserir_linha(5).unwrap();
        assert_eq!(e.chave_da_linha(5), Some(ChaveLinha::Nova(3)));
        assert_eq!(e.chave_da_linha(6), Some(ChaveLinha::Nova(0)));
    }

    #[test]
    fn inserir_varias_linhas_de_uma_vez() {
        let mut e = edicoes();
        let ks = e.inserir_linhas(10, 3).unwrap();
        assert_eq!(ks, vec![ChaveLinha::Nova(0), ChaveLinha::Nova(1), ChaveLinha::Nova(2)]);
        assert_eq!(e.total_linhas(), 13);
        assert_eq!(e.desfazer.len(), 1);
        assert_eq!(e.chaves(9, 4), vec![b(9), ChaveLinha::Nova(0), ChaveLinha::Nova(1), ChaveLinha::Nova(2)]);
        assert!(e.inserir_linhas(3, 0).unwrap().is_empty());
        e.desfazer();
        assert_eq!(e.total_linhas(), 10);
    }

    #[test]
    fn excluir_linhas_parte_os_trechos() {
        let mut e = edicoes();
        e.excluir_linhas(&[0, 3, 4, 9]).unwrap();
        assert_eq!(e.trechos(), &[base(1, 3), base(5, 9)]);
        assert_eq!(e.total_linhas(), 6);
        assert_eq!(chaves(&e), vec![b(1), b(2), b(5), b(6), b(7), b(8)]);
        assert_eq!(e.desfazer.len(), 1);
        assert!(e.desfazer());
        assert_eq!(e.trechos(), &[base(0, 10)]);
        assert!(e.excluir_linhas(&[10]).is_err());
        assert!(e.excluir_linhas(&[]).is_ok());
        assert_eq!(e.desfazer.len(), 0, "nada a excluir não é passo");
    }

    #[test]
    fn excluir_tudo_e_desordenado_com_repetidas() {
        let mut e = edicoes();
        e.excluir_linhas(&[5, 1, 5, 1, 8]).unwrap();
        assert_eq!(chaves(&e), vec![b(0), b(2), b(3), b(4), b(6), b(7), b(9)]);
        let todas: Vec<u64> = (0..e.total_linhas()).collect();
        e.excluir_linhas(&todas).unwrap();
        assert_eq!(e.total_linhas(), 0);
        assert!(e.trechos().is_empty());
    }

    #[test]
    fn excluir_linha_nova_e_misturado() {
        let mut e = edicoes();
        e.inserir_linha(2).unwrap(); // posição 2 = Nova(0)
        e.inserir_linha(7).unwrap(); // posição 7 = Nova(1)
        assert_eq!(e.chave_da_linha(2), Some(ChaveLinha::Nova(0)));
        assert_eq!(e.chave_da_linha(7), Some(ChaveLinha::Nova(1)));
        e.excluir_linhas(&[2, 3, 7]).unwrap();
        assert_eq!(e.total_linhas(), 9);
        assert_eq!(chaves(&e), vec![b(0), b(1), b(3), b(4), b(5), b(6), b(7), b(8), b(9)]);
    }

    #[test]
    fn chaves_de_um_intervalo() {
        let mut e = edicoes();
        e.excluir_linhas(&[2]).unwrap();
        e.inserir_linha(5).unwrap();
        let todas = chaves(&e);
        assert_eq!(todas.len(), 10);
        for inicio in 0..10u64 {
            for q in [0usize, 1, 3, 20] {
                let esperado: Vec<ChaveLinha> = todas.iter().skip(inicio as usize).take(q).copied().collect();
                assert_eq!(e.chaves(inicio, q), esperado, "inicio {inicio}, quantidade {q}");
            }
        }
        assert!(e.chaves(10, 5).is_empty());
    }

    #[test]
    fn colunas_inserir_excluir_renomear() {
        let mut e = edicoes();
        let id = e.inserir_coluna(1, "nova".into()).unwrap();
        assert_eq!(id, 3);
        let nomes: Vec<&str> = e.colunas().iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["c0", "nova", "c1", "c2"]);
        assert_eq!(e.colunas()[1].origem, None);
        assert_eq!(e.colunas()[2].origem, Some(1));
        assert!(!e.colunas_como_no_arquivo());
        e.excluir_colunas(&[0, 2]).unwrap();
        let nomes: Vec<&str> = e.colunas().iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["nova", "c2"]);
        e.renomear_coluna(1, "ultima".into()).unwrap();
        assert_eq!(e.colunas()[1].nome, "ultima");
        assert_eq!(e.colunas()[1].id, 2, "renomear não muda a identidade");
        e.renomear_coluna(1, "ultima".into()).unwrap();
        assert_eq!(e.desfazer.len(), 3, "o mesmo nome de novo não é passo");
        assert!(e.inserir_coluna(5, "x".into()).is_err());
        assert!(e.excluir_colunas(&[2]).is_err());
        assert!(e.renomear_coluna(2, "x".into()).is_err());
    }

    #[test]
    fn nao_exclui_a_ultima_coluna() {
        let mut e = Edicoes::nova(&nomes(2), 1);
        assert!(e.excluir_colunas(&[0, 1]).unwrap_err().mensagem.contains("pelo menos uma coluna"));
        e.excluir_colunas(&[0]).unwrap();
        assert!(e.excluir_colunas(&[0]).is_err());
        assert_eq!(e.n_colunas(), 1);
    }

    #[test]
    fn renomear_so_o_nome_mantem_as_colunas_do_arquivo() {
        let mut e = edicoes();
        e.renomear_coluna(0, "outro".into()).unwrap();
        assert!(e.colunas_como_no_arquivo());
    }

    #[test]
    fn desfazer_e_refazer_em_cadeia() {
        let mut e = edicoes();
        e.definir_celula(0, 0, Some("a".into())).unwrap();
        e.inserir_linha(3).unwrap();
        e.inserir_coluna(0, "k".into()).unwrap();
        e.excluir_linhas(&[0]).unwrap();
        e.renomear_coluna(1, "z".into()).unwrap();
        let depois = (
            e.colunas().to_vec(),
            e.trechos().to_vec(),
            e.celulas().map(|(l, c, v)| (l, c, v.to_string())).collect::<Vec<_>>(),
        );
        for _ in 0..5 {
            assert!(e.desfazer());
        }
        assert!(!e.desfazer());
        assert!(!e.alterada());
        assert!(e.colunas_como_no_arquivo() && e.linhas_como_no_arquivo() && !e.tem_celulas_alteradas());
        for _ in 0..5 {
            assert!(e.refazer());
        }
        assert!(!e.refazer());
        assert!(e.alterada());
        let agora = (
            e.colunas().to_vec(),
            e.trechos().to_vec(),
            e.celulas().map(|(l, c, v)| (l, c, v.to_string())).collect::<Vec<_>>(),
        );
        assert_eq!(agora, depois);
    }

    #[test]
    fn operacao_nova_limpa_o_refazer() {
        let mut e = edicoes();
        e.definir_celula(0, 0, Some("a".into())).unwrap();
        e.desfazer();
        assert!(e.pode_refazer());
        e.definir_celula(0, 0, Some("b".into())).unwrap();
        assert!(!e.pode_refazer());
    }

    #[test]
    fn grupo_vale_um_passo_so() {
        let mut e = edicoes();
        e.definir_celula(0, 0, Some("antes".into())).unwrap();
        e.abrir_grupo();
        let id = e.inserir_coluna(3, "valor".into()).unwrap();
        e.definir_celulas([(0, 3, Some("1".into())), (1, 3, Some("2".into())), (0, 0, Some("mudou".into()))]).unwrap();
        e.inserir_linha(10).unwrap();
        e.renomear_coluna(3, "preco".into()).unwrap();
        assert!(e.alterada());
        assert!(!e.desfazer(), "com o grupo aberto não se desfaz");
        e.fechar_grupo();
        assert_eq!(e.desfazer.len(), 2);
        assert_eq!(e.celula_editada(b(1), id), Some("2"));
        assert!(e.desfazer());
        assert_eq!(e.n_colunas(), 3);
        assert_eq!(e.total_linhas(), 10);
        assert_eq!(e.celula_editada(b(0), 0), Some("antes"), "o valor de antes do grupo voltou");
        assert_eq!(e.celula_editada(b(1), id), None);
        assert!(e.refazer());
        assert_eq!(e.n_colunas(), 4);
        assert_eq!(e.colunas()[3].nome, "preco");
        assert_eq!(e.celula_editada(b(0), 0), Some("mudou"));
        assert_eq!(e.total_linhas(), 11);
    }

    #[test]
    fn passos_aplicados_conta_o_grupo_aberto_como_um() {
        let mut e = edicoes();
        assert_eq!(e.passos_aplicados(), 0);
        e.definir_celula(0, 0, Some("a".into())).unwrap();
        e.inserir_linha(0).unwrap();
        assert_eq!(e.passos_aplicados(), 2);
        e.abrir_grupo();
        assert_eq!(e.passos_aplicados(), 2, "grupo aberto e vazio não conta");
        e.inserir_coluna(0, "k".into()).unwrap();
        e.definir_celula(1, 1, Some("b".into())).unwrap();
        assert_eq!(e.passos_aplicados(), 3, "o grupo aberto vale um passo, não dois");
        assert_eq!(e.passos_aplicados() > 0, e.alterada());
        e.fechar_grupo();
        assert_eq!(e.passos_aplicados(), 3);
        assert!(e.desfazer());
        assert_eq!(e.passos_aplicados(), 2);
        e.abrir_grupo();
        e.definir_celula(0, 0, Some("c".into())).unwrap();
        e.descartar_grupo();
        assert_eq!(e.passos_aplicados(), 2, "grupo descartado não conta");
        assert!(e.desfazer() && e.desfazer());
        assert_eq!(e.passos_aplicados(), 0);
        assert!(!e.alterada());
    }

    #[test]
    fn grupo_vazio_nao_vira_passo_e_descartar_desfaz_tudo() {
        let mut e = edicoes();
        e.abrir_grupo();
        e.fechar_grupo();
        assert!(!e.alterada());
        e.abrir_grupo();
        e.inserir_coluna(0, "x".into()).unwrap();
        e.definir_celula(0, 0, Some("v".into())).unwrap();
        e.excluir_linhas(&[1, 2]).unwrap();
        assert!(e.alterada());
        e.descartar_grupo();
        assert!(!e.alterada() && !e.pode_desfazer());
        assert!(e.colunas_como_no_arquivo() && e.linhas_como_no_arquivo() && !e.tem_celulas_alteradas());
    }

    #[test]
    fn ids_nao_se_repetem_depois_de_desfazer() {
        let mut e = edicoes();
        let a = e.inserir_coluna(0, "a".into()).unwrap();
        e.desfazer();
        let b2 = e.inserir_coluna(0, "b".into()).unwrap();
        assert_ne!(a, b2);
        let l1 = e.inserir_linha(0).unwrap();
        e.desfazer();
        let l2 = e.inserir_linha(0).unwrap();
        assert_ne!(l1, l2);
    }

    #[test]
    fn edicao_de_linha_nova_e_de_coluna_nova() {
        let mut e = edicoes();
        e.inserir_linha(0).unwrap();
        let id = e.inserir_coluna(3, "extra".into()).unwrap();
        e.definir_celula(0, 3, Some("x".into())).unwrap();
        assert_eq!(e.celula_editada(ChaveLinha::Nova(0), id), Some("x"));
        e.definir_celula(1, 3, Some("y".into())).unwrap();
        assert_eq!(e.celula_editada(b(0), id), Some("y"));
        let ordem: Vec<_> = e.celulas().map(|(l, _, _)| l).collect();
        assert_eq!(ordem, vec![b(0), ChaveLinha::Nova(0)], "as do arquivo vêm primeiro");
    }
}
