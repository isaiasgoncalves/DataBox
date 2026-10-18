//! A tabela de um CSV: o arquivo lido por páginas ([`Leitor`]) com as alterações da pessoa por cima
//! ([`Edicoes`]). É o que o editor tabular usa: pede uma página, edita células, insere e exclui linhas e
//! colunas, desfaz, refaz e salva. O arquivo só muda no [`Tabela::salvar`].

use std::collections::HashMap;
use std::path::Path;

use comum::{ErroBanco, Resultado};

use super::edicoes::{ChaveLinha, ColunaVirtual, Edicoes, IdColuna};
use super::formato::{Formato, OpcoesAbertura};
use super::gravacao::{ResumoSalvar, gravar};
use super::leitor::Leitor;
use crate::numero::{TipoDetectado, tipo_da_coluna};

fn sem_progresso(_: u64, _: u64) -> Resultado<()> {
    Ok(())
}

/// Um CSV aberto para edição.
#[derive(Debug)]
pub struct Tabela {
    leitor: Leitor,
    edicoes: Edicoes,
}

impl Tabela {
    pub fn abrir(caminho: &Path, opcoes: &OpcoesAbertura) -> Resultado<Tabela> {
        Tabela::abrir_com_progresso(caminho, opcoes, &mut sem_progresso)
    }

    /// Como [`Tabela::abrir`], com o progresso da passada que monta o índice (ver [`Leitor::abrir_com_progresso`]).
    pub fn abrir_com_progresso(
        caminho: &Path,
        opcoes: &OpcoesAbertura,
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Tabela> {
        Ok(Tabela::de_leitor(Leitor::abrir_com_progresso(caminho, opcoes, ao_progresso)?))
    }

    pub fn de_leitor(leitor: Leitor) -> Tabela {
        let edicoes = Edicoes::nova(leitor.nomes(), leitor.linhas());
        Tabela { leitor, edicoes }
    }

    // ---- o que a tabela é ----------------------------------------------------------------------------

    pub fn leitor(&self) -> &Leitor {
        &self.leitor
    }

    pub fn edicoes(&self) -> &Edicoes {
        &self.edicoes
    }

    pub fn caminho(&self) -> &Path {
        self.leitor.caminho()
    }

    pub fn formato(&self) -> &Formato {
        self.leitor.formato()
    }

    /// Linhas de dados que a grade mostra (sem o cabeçalho).
    pub fn linhas(&self) -> u64 {
        self.edicoes.total_linhas()
    }

    pub fn colunas(&self) -> usize {
        self.edicoes.n_colunas()
    }

    /// As colunas que a grade mostra, com o nome de cada uma.
    pub fn colunas_virtuais(&self) -> &[ColunaVirtual] {
        self.edicoes.colunas()
    }

    pub fn nome_da_coluna(&self, coluna: usize) -> Option<&str> {
        self.edicoes.colunas().get(coluna).map(|c| c.nome.as_str())
    }

    pub fn nomes(&self) -> Vec<String> {
        self.edicoes.colunas().iter().map(|c| c.nome.clone()).collect()
    }

    // ---- leitura -------------------------------------------------------------------------------------

    /// Os textos de uma linha da grade, uma célula por coluna, com as alterações.
    fn montar(&self, chave: ChaveLinha, campos: Option<&[String]>) -> Vec<String> {
        let colunas = self.edicoes.colunas();
        let mut linha: Vec<String> = colunas
            .iter()
            .map(|c| match (c.origem, campos) {
                (Some(o), Some(campos)) => campos.get(o).cloned().unwrap_or_default(),
                _ => String::new(),
            })
            .collect();
        if self.edicoes.tem_celulas_alteradas() {
            for (id, valor) in self.edicoes.celulas_da_linha(chave) {
                if let Some(p) = colunas.iter().position(|c| c.id == id) {
                    linha[p] = valor.to_string();
                }
            }
        }
        linha
    }

    /// As linhas `inicio..inicio + quantidade` da grade (menos, se o arquivo acaba antes), cada uma com uma
    /// célula por coluna.
    pub fn pagina(&mut self, inicio: u64, quantidade: usize) -> Resultado<Vec<Vec<String>>> {
        let chaves = self.edicoes.chaves(inicio, quantidade);
        let mut saida = Vec::with_capacity(chaves.len());
        let mut i = 0;
        while i < chaves.len() {
            match chaves[i] {
                ChaveLinha::Nova(_) => {
                    saida.push(self.montar(chaves[i], None));
                    i += 1;
                }
                ChaveLinha::Base(r) => {
                    // as linhas do arquivo em sequência saem de uma leitura só
                    let mut j = i + 1;
                    while j < chaves.len() && chaves[j] == ChaveLinha::Base(r + (j - i) as u64) {
                        j += 1;
                    }
                    let lidas = self.leitor.linhas_de(r, j - i)?;
                    for (k, chave) in chaves[i..j].iter().enumerate() {
                        saida.push(self.montar(*chave, lidas.get(k).map(Vec::as_slice)));
                    }
                    i = j;
                }
            }
        }
        Ok(saida)
    }

    /// O texto de uma célula.
    pub fn celula(&mut self, linha: u64, coluna: usize) -> Resultado<String> {
        let pagina = self.pagina(linha, 1)?;
        let l =
            pagina.into_iter().next().ok_or_else(|| ErroBanco::novo(format!("A linha {} não existe.", linha + 1)))?;
        l.into_iter().nth(coluna).ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", coluna + 1)))
    }

    /// As colunas pedidas (pelo número na grade) de todas as linhas, com as alterações, numa passada pelo
    /// arquivo: a origem do Trazer colunas e a coluna-chave. O progresso e o cancelamento são os de
    /// [`Leitor::varrer`].
    pub fn colunas_de(
        &mut self,
        indices: &[usize],
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<Vec<Vec<String>>> {
        let colunas = self.edicoes.colunas();
        let mut origens = Vec::new();
        let mut posicao_na_origem = Vec::with_capacity(indices.len());
        for &c in indices {
            let coluna = colunas.get(c).ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", c + 1)))?;
            posicao_na_origem.push(coluna.origem.map(|o| {
                origens.push(o);
                origens.len() - 1
            }));
        }
        let ids: Vec<_> = indices.iter().map(|&c| colunas[c].id).collect();
        let mut base = if origens.is_empty() { Vec::new() } else { self.leitor.colunas_de(&origens, ao_progresso)? };
        let total = usize::try_from(self.edicoes.total_linhas()).unwrap_or(0);
        let mut saida: Vec<Vec<String>> = indices.iter().map(|_| Vec::with_capacity(total)).collect();
        let tem_alteracoes = self.edicoes.tem_celulas_alteradas();
        for chave in self.edicoes.percorrer_chaves() {
            for (k, coluna) in saida.iter_mut().enumerate() {
                let mut valor = match (chave, posicao_na_origem[k]) {
                    (ChaveLinha::Base(r), Some(p)) => {
                        usize::try_from(r).ok().and_then(|r| base[p].get_mut(r)).map(std::mem::take).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                if tem_alteracoes && let Some(v) = self.edicoes.celula_editada(chave, ids[k]) {
                    valor = v.to_string();
                }
                coluna.push(valor);
            }
        }
        Ok(saida)
    }

    /// O tipo detectado de cada coluna (a pastilha `12`, `,5`, `Aa`, `31` do cabeçalho da grade), pelas primeiras
    /// `amostra` linhas da grade, com as alterações e o decimal do arquivo.
    pub fn tipos_das_colunas(&mut self, amostra: usize) -> Resultado<Vec<TipoDetectado>> {
        let linhas = self.pagina(0, amostra)?;
        let decimal = self.formato().decimal;
        Ok((0..self.colunas())
            .map(|j| tipo_da_coluna(linhas.iter().map(|l| l.get(j).map_or("", String::as_str)), decimal))
            .collect())
    }

    // ---- alterações ----------------------------------------------------------------------------------

    /// O texto que a célula tem no arquivo (vazio numa linha ou coluna nova).
    fn texto_no_arquivo(&mut self, chave: ChaveLinha, origem: Option<usize>) -> Resultado<String> {
        match (chave, origem) {
            (ChaveLinha::Base(r), Some(o)) => {
                Ok(self.leitor.linha(r)?.and_then(|l| l.into_iter().nth(o)).unwrap_or_default())
            }
            _ => Ok(String::new()),
        }
    }

    /// Muda o texto de uma célula. Devolve `false` se o texto já era esse (nada mudou, e nada entra no
    /// desfazer). Voltar ao texto do arquivo apaga a alteração da célula.
    pub fn editar_celula(&mut self, linha: u64, coluna: usize, texto: &str) -> Resultado<bool> {
        let chave = self
            .edicoes
            .chave_da_linha(linha)
            .ok_or_else(|| ErroBanco::novo(format!("A linha {} não existe.", linha + 1)))?;
        let col = self
            .edicoes
            .colunas()
            .get(coluna)
            .ok_or_else(|| ErroBanco::novo(format!("A coluna {} não existe.", coluna + 1)))?
            .clone();
        let no_arquivo = self.texto_no_arquivo(chave, col.origem)?;
        let atual = self.edicoes.celula_editada(chave, col.id).map_or(no_arquivo.as_str(), |v| v);
        if atual == texto {
            return Ok(false);
        }
        let valor = if texto == no_arquivo { None } else { Some(texto.to_string()) };
        self.edicoes.definir_celula(linha, coluna, valor)?;
        Ok(true)
    }

    /// Insere uma linha vazia na posição dada da grade (`linhas()` põe no fim).
    pub fn inserir_linha(&mut self, posicao: u64) -> Resultado<()> {
        self.edicoes.inserir_linha(posicao).map(|_| ())
    }

    /// Acrescenta `quantidade` linhas vazias no fim, num passo só.
    pub fn acrescentar_linhas(&mut self, quantidade: u64) -> Resultado<()> {
        self.edicoes.inserir_linhas(self.edicoes.total_linhas(), quantidade).map(|_| ())
    }

    pub fn excluir_linhas(&mut self, linhas: &[u64]) -> Resultado<()> {
        self.edicoes.excluir_linhas(linhas)
    }

    /// Insere uma coluna vazia na posição dada (`colunas()` põe no fim).
    pub fn inserir_coluna(&mut self, posicao: usize, nome: &str) -> Resultado<()> {
        self.edicoes.inserir_coluna(posicao, nome.to_string()).map(|_| ())
    }

    pub fn excluir_colunas(&mut self, colunas: &[usize]) -> Resultado<()> {
        self.edicoes.excluir_colunas(colunas)
    }

    pub fn renomear_coluna(&mut self, coluna: usize, nome: &str) -> Resultado<()> {
        self.edicoes.renomear_coluna(coluna, nome.to_string())
    }

    /// Põe os `valores` numa coluna, por posição: o primeiro vai para a primeira linha da grade, e assim por
    /// diante; `None` deixa a célula como está. Valores a mais que linhas são ignorados.
    pub fn preencher_coluna(&mut self, coluna: usize, valores: &[Option<String>]) -> Resultado<()> {
        let itens = valores
            .iter()
            .enumerate()
            .take(usize::try_from(self.edicoes.total_linhas()).unwrap_or(usize::MAX))
            .filter_map(|(i, v)| v.as_ref().map(|v| (i as u64, coluna, Some(v.clone()))));
        self.edicoes.definir_celulas(itens.collect::<Vec<_>>())
    }

    /// Insere uma coluna com os `valores` (a linha 1 colada vai para a linha 1 da grade), num passo só.
    pub fn inserir_coluna_com_valores(&mut self, posicao: usize, nome: &str, valores: &[String]) -> Resultado<()> {
        self.em_um_passo(|t| {
            t.inserir_coluna(posicao, nome)?;
            let valores: Vec<Option<String>> =
                valores.iter().map(|v| if v.is_empty() { None } else { Some(v.clone()) }).collect();
            t.preencher_coluna(posicao, &valores)
        })
    }

    /// Faz tudo o que `f` fizer valer um passo só no desfazer; se `f` falhar, nada fica. Se já há um grupo
    /// aberto (uma chamada dentro da outra), as alterações entram nele.
    pub fn em_um_passo<T>(&mut self, f: impl FnOnce(&mut Tabela) -> Resultado<T>) -> Resultado<T> {
        let ja_aberto = self.edicoes.grupo_aberto();
        self.edicoes.abrir_grupo();
        let r = f(self);
        if !ja_aberto {
            match &r {
                Ok(_) => self.edicoes.fechar_grupo(),
                Err(_) => self.edicoes.descartar_grupo(),
            }
        }
        r
    }

    pub fn desfazer(&mut self) -> bool {
        self.edicoes.desfazer()
    }

    pub fn refazer(&mut self) -> bool {
        self.edicoes.refazer()
    }

    pub fn pode_desfazer(&self) -> bool {
        self.edicoes.pode_desfazer()
    }

    pub fn pode_refazer(&self) -> bool {
        self.edicoes.pode_refazer()
    }

    /// Há alterações que o arquivo ainda não tem (o ponto na aba).
    pub fn alterada(&self) -> bool {
        self.edicoes.alterada()
    }

    /// Quantas alterações a tabela tem desde a abertura ou o último Salvar: os passos do desfazer aplicados (o
    /// "1 alteração não salva"). Desfazer baixa a conta, refazer sobe, e `0` é o mesmo que `!alterada()`. Só
    /// lê as alterações, sem E/S.
    pub fn alteracoes(&self) -> usize {
        self.edicoes.passos_aplicados()
    }

    /// Quantos passos o Refazer tem (os desfeitos desde o último passo novo). Só lê as alterações, sem E/S.
    pub fn passos_a_refazer(&self) -> usize {
        self.edicoes.passos_desfeitos()
    }

    /// Para cada linha de `pagina(inicio, quantidade)` (o mesmo número de linhas), uma marca por coluna: `true`
    /// se a célula tem alteração (o `td.alt` da grade). Uma célula de linha nova ou de coluna nova só é marcada
    /// se alguém pôs um valor nela (vazia, nunca); uma célula do arquivo, se a pessoa a alterou (`editar_celula`
    /// voltando ao texto do arquivo apaga a marca). A marca é "a célula tem um valor guardado nas alterações",
    /// sem comparar com o disco (por isso `preencher_coluna` com o mesmo texto do arquivo também marca). Só lê
    /// as alterações, sem E/S.
    pub fn marcas_da_pagina(&self, inicio: u64, quantidade: usize) -> Vec<Vec<bool>> {
        let colunas = self.edicoes.colunas();
        let chaves = self.edicoes.chaves(inicio, quantidade);
        let mut marcas = vec![vec![false; colunas.len()]; chaves.len()];
        if !self.edicoes.tem_celulas_alteradas() {
            return marcas;
        }
        let posicoes: HashMap<IdColuna, usize> = colunas.iter().enumerate().map(|(p, c)| (c.id, p)).collect();
        for (linha, chave) in marcas.iter_mut().zip(chaves) {
            for (id, _) in self.edicoes.celulas_da_linha(chave) {
                // a célula de uma coluna que foi excluída continua guardada, mas não aparece
                if let Some(&p) = posicoes.get(&id) {
                    linha[p] = true;
                }
            }
        }
        marcas
    }

    // ---- formato -------------------------------------------------------------------------------------

    fn recusar_se_alterada(&self) -> Resultado<()> {
        if self.alterada() {
            return Err(ErroBanco::com_status(
                "Salve ou desfaça as alterações antes de mudar a forma de ler o arquivo.",
                409,
            ));
        }
        Ok(())
    }

    /// Liga ou desliga "a primeira linha é o cabeçalho". Só vale sem alterações pendentes: o que é linha e o
    /// que é nome muda.
    pub fn definir_cabecalho(&mut self, cabecalho: bool) -> Resultado<()> {
        self.recusar_se_alterada()?;
        self.leitor.definir_cabecalho(cabecalho)?;
        self.edicoes = Edicoes::nova(self.leitor.nomes(), self.leitor.linhas());
        Ok(())
    }

    /// Lê o arquivo de novo com outras opções (outro separador, outra codificação…). Só vale sem alterações
    /// pendentes. Com `OpcoesAbertura::default()`, relê com a detecção de sempre (é o "Recarregar").
    pub fn reabrir(
        &mut self,
        opcoes: &OpcoesAbertura,
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<()> {
        self.recusar_se_alterada()?;
        let leitor = Leitor::abrir_com_progresso(self.leitor.caminho(), opcoes, ao_progresso)?;
        *self = Tabela::de_leitor(leitor);
        Ok(())
    }

    // ---- gravação ------------------------------------------------------------------------------------

    /// Grava as alterações no próprio arquivo, no formato dele: lê o arquivo de origem e escreve num
    /// temporário ao lado, trocando no fim. Sem alterações, não faz nada. Depois de salvar, o desfazer
    /// recomeça (as alterações já são o arquivo). `ao_progresso(bytes, total)` acompanha e pode cancelar
    /// (o erro dele sobe como veio, e o arquivo fica como estava).
    pub fn salvar(&mut self, ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>) -> Resultado<ResumoSalvar> {
        if !self.alterada() {
            return Ok(ResumoSalvar {
                linhas: self.linhas(),
                bytes: self.leitor.tamanho(),
                linhas_regravadas: 0,
                tempo_ms: 0,
            });
        }
        let destino = self.leitor.caminho().to_path_buf();
        let formato = self.leitor.formato().clone();
        self.salvar_como(&destino, &formato, ao_progresso)
    }

    /// Grava num arquivo (que passa a ser o da tabela) no `formato` dado: com outra codificação, separador,
    /// quebra de linha ou estilo de aspas, todas as linhas são regravadas no formato novo.
    ///
    /// Um `Err` quer dizer que o destino ficou como estava (e as alterações também). Depois que o temporário toma o
    /// lugar do destino, as alterações já estão no disco: daí em diante nada devolve erro, e as alterações recomeçam
    /// do arquivo gravado mesmo se lê-lo de novo falhar (veja `leitor_do_gravado`).
    pub fn salvar_como(
        &mut self,
        destino: &Path,
        formato: &Formato,
        ao_progresso: &mut dyn FnMut(u64, u64) -> Resultado<()>,
    ) -> Resultado<ResumoSalvar> {
        let gravado = gravar(&mut self.leitor, &self.edicoes, destino, formato, ao_progresso)?;
        if let Some(leitor) = Tabela::leitor_do_gravado(destino, gravado.formato, gravado.indice) {
            self.leitor = leitor;
        }
        self.edicoes = Edicoes::nova(self.leitor.nomes(), self.leitor.linhas());
        Ok(gravado.resumo)
    }

    /// O leitor do arquivo que acabou de ser gravado: com o índice da gravação; se ele não abrir assim (o tamanho no
    /// disco não bate, o arquivo sumiu num instante), uma leitura nova com o formato gravado fixo; e se nem essa der
    /// certo, `None`: fica o leitor de antes, que vê o arquivo mudado no disco (`mudou_no_disco`, e as leituras
    /// recusam com 409) até alguém ler o arquivo de novo.
    fn leitor_do_gravado(destino: &Path, formato: Formato, indice: super::indice::Indice) -> Option<Leitor> {
        let opcoes = OpcoesAbertura {
            codificacao: Some(formato.codificacao),
            separador: Some(formato.separador),
            fim_de_linha: Some(formato.fim_de_linha),
            aspas: Some(formato.aspas),
            cabecalho: Some(formato.cabecalho),
            decimal: Some(formato.decimal),
        };
        let com_indice =
            if falha_simulada_do_indice() { None } else { Leitor::com_indice(destino, formato, indice).ok() };
        com_indice.or_else(|| Leitor::abrir(destino, &opcoes).ok())
    }
}

#[cfg(test)]
thread_local! {
    /// Os testes fazem o `Leitor::com_indice` depois da gravação falhar (não há como provocar isso de fora).
    pub(crate) static FALHAR_COM_INDICE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn falha_simulada_do_indice() -> bool {
    FALHAR_COM_INDICE.with(std::cell::Cell::get)
}

#[cfg(not(test))]
fn falha_simulada_do_indice() -> bool {
    false
}
