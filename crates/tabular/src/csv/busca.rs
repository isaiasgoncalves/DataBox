//! A busca da barra (Ctrl+F) no CSV inteiro: [`Tabela::buscar`] acha a próxima célula com o termo a partir de uma
//! posição, e [`Tabela::achar_todos`] entrega todas, na ordem da grade. Sem diferenciar maiúsculas nem acentos
//! ([`comum::texto::Agulha`]), com as alterações pendentes por cima do arquivo ([`Edicoes`]). O arquivo é lido pelo
//! índice, trecho a trecho, sem montar páginas e sem guardar as linhas: a memória não cresce com o arquivo.
//!
//! A busca não segura a tabela: [`Tabela::copia_para_buscar`] tira um leitor próprio do arquivo e a cópia das
//! alterações ([`CopiaParaBuscar`]), que buscam numa tarefa enquanto a grade continua lendo páginas.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use comum::texto::Agulha;
use comum::{ErroBanco, Resultado};

use super::analise::{BLOCO, Campo, Varredura, decodificar_campo};
use super::arquivos::{CONTEXTO_LER, erro_io};
use super::edicoes::{ChaveLinha, Edicoes, Trecho};
use super::leitor::Leitor;
use super::tabela::Tabela;

/// De quantas em quantas linhas a busca pergunta se deve parar.
const LINHAS_POR_CONSULTA: u64 = 1024;

/// O que recebe cada linha lida: o número dela, o buffer e os campos.
type QuandoUmaLinha<'a> = dyn FnMut(u64, &[u8], &[Campo]) -> bool + 'a;

/// O que a busca precisa de uma tabela, sem segurá-la: um leitor próprio do arquivo (o mesmo índice) e a cópia das
/// alterações de agora.
#[derive(Debug)]
pub struct CopiaParaBuscar {
    leitor: Leitor,
    edicoes: Edicoes,
}

impl Tabela {
    /// Um leitor próprio do arquivo e a cópia das alterações, para buscar numa tarefa sem segurar a tabela.
    pub fn copia_para_buscar(&self) -> Resultado<CopiaParaBuscar> {
        Ok(CopiaParaBuscar { leitor: self.leitor().reabrir_para_leitura()?, edicoes: self.edicoes().clone() })
    }

    /// A primeira célula com `texto` (sem diferenciar maiúsculas nem acentos) a partir de `a_partir_de` (linha da
    /// grade e coluna, inclusive), na ordem da grade: a linha, e nela da esquerda para a direita. Sem volta ao começo:
    /// `None` se não há nenhuma até o fim (ou se `parar` pediu). `texto` vazio não acha nada.
    pub fn buscar(
        &self,
        texto: &str,
        a_partir_de: (u64, usize),
        parar: &dyn Fn() -> bool,
    ) -> Resultado<Option<(u64, usize)>> {
        buscar(self.leitor(), self.edicoes(), texto, a_partir_de, parar)
    }

    /// Todas as células com `texto`, na ordem da grade: `ao_achar(linha, coluna)` recebe cada uma e devolve se
    /// continua. Devolve `true` se percorreu a tabela até o fim (`false`: `parar` ou `ao_achar` interromperam).
    pub fn achar_todos(
        &self,
        texto: &str,
        parar: &dyn Fn() -> bool,
        ao_achar: &mut dyn FnMut(u64, usize) -> bool,
    ) -> Resultado<bool> {
        achar_todos(self.leitor(), self.edicoes(), texto, parar, ao_achar)
    }
}

impl CopiaParaBuscar {
    /// As linhas da grade na cópia.
    pub fn linhas(&self) -> u64 {
        self.edicoes.total_linhas()
    }

    /// O mesmo que [`Tabela::buscar`], na cópia.
    pub fn buscar(
        &self,
        texto: &str,
        a_partir_de: (u64, usize),
        parar: &dyn Fn() -> bool,
    ) -> Resultado<Option<(u64, usize)>> {
        buscar(&self.leitor, &self.edicoes, texto, a_partir_de, parar)
    }

    /// O mesmo que [`Tabela::achar_todos`], na cópia.
    pub fn achar_todos(
        &self,
        texto: &str,
        parar: &dyn Fn() -> bool,
        ao_achar: &mut dyn FnMut(u64, usize) -> bool,
    ) -> Resultado<bool> {
        achar_todos(&self.leitor, &self.edicoes, texto, parar, ao_achar)
    }
}

fn buscar(
    leitor: &Leitor,
    edicoes: &Edicoes,
    texto: &str,
    (linha, coluna): (u64, usize),
    parar: &dyn Fn() -> bool,
) -> Resultado<Option<(u64, usize)>> {
    let Some(agulha) = Agulha::nova(texto) else { return Ok(None) };
    let mut achada = None;
    percorrer(leitor, edicoes, &agulha, linha, parar, &mut |l, c| {
        if l == linha && c < coluna {
            return true;
        }
        achada = Some((l, c));
        false
    })?;
    Ok(achada)
}

fn achar_todos(
    leitor: &Leitor,
    edicoes: &Edicoes,
    texto: &str,
    parar: &dyn Fn() -> bool,
    ao_achar: &mut dyn FnMut(u64, usize) -> bool,
) -> Resultado<bool> {
    let Some(agulha) = Agulha::nova(texto) else { return Ok(true) };
    percorrer(leitor, edicoes, &agulha, 0, parar, ao_achar)
}

/// Percorre as linhas da grade a partir de `desde`, trecho a trecho (as do arquivo lidas pelo índice, as novas só das
/// alterações), e entrega cada célula com o termo. Devolve se chegou ao fim.
fn percorrer(
    leitor: &Leitor,
    edicoes: &Edicoes,
    agulha: &Agulha,
    desde: u64,
    parar: &dyn Fn() -> bool,
    ao_achar: &mut dyn FnMut(u64, usize) -> bool,
) -> Resultado<bool> {
    let colunas = edicoes.colunas();
    let alteradas = edicoes.tem_celulas_alteradas();
    let mut arquivo: Option<File> = None;
    let mut posicao = 0u64;
    for trecho in edicoes.trechos() {
        match *trecho {
            Trecho::Nova(id) => {
                if posicao >= desde {
                    let chave = ChaveLinha::Nova(id);
                    for (c, coluna) in colunas.iter().enumerate() {
                        let valor = edicoes.celula_editada(chave, coluna.id).unwrap_or_default();
                        if agulha.em(valor) && !ao_achar(posicao, c) {
                            return Ok(false);
                        }
                    }
                }
                posicao += 1;
            }
            Trecho::Base { de, ate } => {
                let pular = desde.saturating_sub(posicao).min(ate - de);
                let primeira = posicao + pular;
                posicao += ate - de;
                if de + pular >= ate {
                    continue;
                }
                let f = match arquivo.as_mut() {
                    Some(f) => f,
                    None => arquivo.insert(abrir(leitor)?),
                };
                let mut continuar = true;
                varrer_trecho(leitor, f, de + pular, ate, parar, &mut |r, buf, campos| {
                    let linha = primeira + (r - de - pular);
                    let chave = ChaveLinha::Base(r);
                    for (c, coluna) in colunas.iter().enumerate() {
                        let editada = if alteradas { edicoes.celula_editada(chave, coluna.id) } else { None };
                        let casou = match (editada, coluna.origem) {
                            (Some(v), _) => agulha.em(v),
                            (None, Some(o)) => campos.get(o).is_some_and(|&campo| {
                                agulha.em(&decodificar_campo(buf, campo, leitor.formato().codificacao))
                            }),
                            (None, None) => false,
                        };
                        if casou && !ao_achar(linha, c) {
                            continuar = false;
                            return false;
                        }
                    }
                    true
                })?;
                if !continuar || parar() {
                    return Ok(false);
                }
            }
        }
    }
    Ok(!parar())
}

/// O arquivo da tabela, aberto de novo, conferindo que outro programa não o mudou depois do índice.
fn abrir(leitor: &Leitor) -> Resultado<File> {
    let caminho = leitor.caminho();
    let io = |e: std::io::Error| erro_io(&e, Some(caminho), CONTEXTO_LER);
    let arquivo = File::open(caminho).map_err(io)?;
    if arquivo.metadata().map_err(io)?.len() != leitor.indice().tamanho {
        return Err(ErroBanco::com_status(
            "O arquivo foi alterado por outro programa depois de aberto. Abra-o de novo para ver o que há nele.",
            409,
        ));
    }
    Ok(arquivo)
}

/// Lê as linhas de dados `primeira..ate` do arquivo, uma passada a partir do ponto do índice mais próximo, e entrega
/// cada uma (o número da linha, o buffer e os campos dela) até `quando_uma_linha` devolver `false` ou `parar` pedir.
fn varrer_trecho(
    leitor: &Leitor,
    arquivo: &mut File,
    primeira: u64,
    ate: u64,
    parar: &dyn Fn() -> bool,
    quando_uma_linha: &mut QuandoUmaLinha<'_>,
) -> Resultado<()> {
    let cabecalho = u64::from(leitor.formato().cabecalho);
    let Some((deslocamento, pular)) = leitor.indice().onde_comeca(primeira + cabecalho) else { return Ok(()) };
    let caminho = leitor.caminho();
    let io = |e: std::io::Error| erro_io(&e, Some(caminho), CONTEXTO_LER);
    arquivo.seek(SeekFrom::Start(deslocamento)).map_err(io)?;
    let fim = leitor.indice().onde_acaba(ate.saturating_sub(1) + cabecalho);
    let trecho = fim.saturating_sub(deslocamento);
    let capacidade = usize::try_from(trecho.min(BLOCO as u64)).unwrap_or(BLOCO);
    let mut v =
        Varredura::nova(arquivo.by_ref().take(trecho), deslocamento, leitor.formato().sep(), true, false, capacidade);
    v.pular(pular).map_err(io)?;
    for r in primeira..ate {
        if (r - primeira).is_multiple_of(LINHAS_POR_CONSULTA) && r > primeira && parar() {
            return Ok(());
        }
        if !v.seguinte().map_err(io)? || !quando_uma_linha(r, v.buffer(), v.campos()) {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::csv::OpcoesAbertura;
    use crate::testes_apoio::Pasta;

    fn abrir_tabela(pasta: &Pasta, conteudo: &str) -> Tabela {
        let caminho = pasta.arquivo("t.csv", conteudo.as_bytes());
        Tabela::abrir(&caminho, &OpcoesAbertura::default()).unwrap()
    }

    fn nunca() -> bool {
        false
    }

    fn todos(t: &Tabela, texto: &str) -> Vec<(u64, usize)> {
        let mut v = Vec::new();
        assert!(
            t.achar_todos(texto, &nunca, &mut |l, c| {
                v.push((l, c));
                true
            })
            .unwrap()
        );
        v
    }

    #[test]
    fn acha_sem_caixa_nem_acento_dentro_de_aspas_e_de_quebra_de_linha() {
        let pasta = Pasta::nova("busca-aspas");
        let t = abrir_tabela(
            &pasta,
            "nome;cidade;nota\nAna;\"São Paulo; SP\";\"linha um\nlinha SAO dois\"\nBia;Recife;7\nCaio;sao paulo;\"Sã\"\n",
        );
        assert_eq!(todos(&t, "sao"), [(0, 1), (0, 2), (2, 1)]);
        assert_eq!(todos(&t, "SÃO PAULO; sp"), [(0, 1)]);
        // o separador dentro das aspas é texto da célula; o cabeçalho não entra na busca
        assert_eq!(todos(&t, "cidade"), []);
        assert_eq!(todos(&t, ""), []);
        assert_eq!(t.buscar("sao", (0, 2), &nunca).unwrap(), Some((0, 2)));
        assert_eq!(t.buscar("sao", (0, 3), &nunca).unwrap(), Some((2, 1)));
        assert_eq!(t.buscar("sao", (2, 2), &nunca).unwrap(), None);
    }

    #[test]
    fn as_alteracoes_pendentes_valem_por_cima_do_arquivo() {
        let pasta = Pasta::nova("busca-alteracoes");
        let mut t = abrir_tabela(&pasta, "a;b\nx;Recife\ny;Olinda\nz;Recife\n");
        t.editar_celula(0, 1, "Natal").unwrap();
        t.inserir_linha(1).unwrap();
        t.editar_celula(1, 0, "recife novo").unwrap();
        t.inserir_coluna(2, "c").unwrap();
        t.editar_celula(2, 2, "RECIFE").unwrap();
        t.excluir_linhas(&[3]).unwrap();
        // a grade: Natal · nova "recife novo" · Olinda (com "RECIFE" na coluna nova); a última linha saiu
        assert_eq!(todos(&t, "recife"), [(1, 0), (2, 2)]);
        assert_eq!(t.buscar("recife", (1, 1), &nunca).unwrap(), Some((2, 2)));
        // a cópia busca o mesmo, sem a tabela
        let copia = t.copia_para_buscar().unwrap();
        assert_eq!(copia.linhas(), 3);
        assert_eq!(copia.buscar("olinda", (0, 0), &nunca).unwrap(), Some((2, 1)));
    }

    #[test]
    fn percorre_trechos_depois_de_excluir_e_para_quando_pedem() {
        let pasta = Pasta::nova("busca-trechos");
        let mut conteudo = String::from("n;v\n");
        for i in 0..3000 {
            conteudo.push_str(&format!("{i};{}\n", if i % 1000 == 7 { "alvo" } else { "x" }));
        }
        let mut t = abrir_tabela(&pasta, &conteudo);
        t.excluir_linhas(&[5, 6, 1500]).unwrap();
        // as linhas 7, 1007 e 2007 do arquivo estão nas posições 5, 1005 e 2004 da grade
        assert_eq!(todos(&t, "ALVO"), [(5, 1), (1005, 1), (2004, 1)]);
        assert_eq!(t.buscar("alvo", (6, 0), &nunca).unwrap(), Some((1005, 1)));
        assert_eq!(t.buscar("alvo", (2004, 1), &nunca).unwrap(), Some((2004, 1)));
        let mut n = 0;
        assert!(
            !t.achar_todos("alvo", &nunca, &mut |_, _| {
                n += 1;
                false
            })
            .unwrap()
        );
        assert_eq!(n, 1);
        assert!(!t.achar_todos("alvo", &|| true, &mut |_, _| true).unwrap());
    }

    #[test]
    fn arquivo_mudado_por_fora_recusa() {
        let pasta = Pasta::nova("busca-mudou");
        let t = abrir_tabela(&pasta, "a\n1\n");
        std::fs::write(t.caminho(), "a\n1\n2\n").unwrap();
        assert_eq!(t.buscar("1", (0, 0), &nunca).unwrap_err().status, 409);
    }
}
