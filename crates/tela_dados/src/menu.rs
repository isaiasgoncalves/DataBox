//! O menu da célula na grade dos Dados (0.2.3, `design/planos-0.2.3.md`, A.1, A.2 e A.4): o que ele oferece, a cópia nos
//! formatos (o Copiar, o Copiar com cabeçalho e o Copiar como, com a célula gravando agora no texto digitado), a
//! condição do "Filtrar por este valor" e do "Excluir este valor do filtro", o lote do Colar, do Limpar e do Definir
//! como NULL (o que cada célula grava, ou por que nada pode ser gravado); e as contas do rodapé da seleção (A.3).

use banco::{CelulaDesfeita, Operador, TabelaInfo};
use comum::{Afinidade, Celula, Chave, Entrada};
use interface::formato::plural;
use interface::grade::{
    AcaoCelula, Intervalo, OfertasCelula, OpcoesCopia, ResumoSelecao, ValorCopia, frase_do_intervalo_copiado,
    texto_celula, texto_do_intervalo_como, tsv_do_intervalo, tsv_do_intervalo_com_cabecalho, valor_de_copia,
};
use interface::pecas::toast::toast_concluido;

use crate::grade::Pendente;
use crate::modelo::{ColunaVista, Dados};
use crate::regras::texto_edicao;

/// O motivo do filtro desligado num BLOB.
pub const MOTIVO_FILTRO_BLOB: &str = "Um valor binário não vira filtro.";
/// O motivo do filtro desligado num texto cortado (a página só traz o começo dele).
pub const MOTIVO_FILTRO_CORTADO: &str = "O texto é longo demais para virar filtro.";
/// O motivo dos itens de escrita num banco aberto só para leitura.
pub const MOTIVO_SO_LEITURA: &str = "O banco foi aberto só para leitura; nada pode ser gravado nele.";
/// O motivo dos itens de escrita com uma gravação a caminho do banco.
pub const MOTIVO_OCUPADO: &str = "Espere a gravação anterior terminar.";
/// O motivo do Colar, do Limpar e do Definir como NULL com um texto longo ou um BLOB no intervalo: o Desfazer não os
/// grava de volta.
pub const MOTIVO_NAO_VOLTA: &str =
    "Um valor do intervalo é longo demais (ou binário) para voltar no Desfazer; mude-o pela ficha da linha.";
/// O motivo do Colar, do Limpar e do Definir como NULL com uma linha sem chave no intervalo.
pub const MOTIVO_SEM_CHAVE: &str =
    "Uma linha do intervalo não tem chave (a chave primária é NULL); edite-a pela Consulta SQL.";
/// Acima disto, o Colar, o Limpar e o Definir como NULL perguntam antes ("Colar 120 células em clientes?").
pub const LIMITE_SEM_PERGUNTA: usize = 100;
/// O aviso de um Colar ou Limpar que não mudaria nenhuma célula.
pub const NADA_MUDOU: &str = "Nada mudou: as células já tinham esses valores.";

/// O que decide, numa coluna de dados, se ela recebe o Colar, o Limpar e o Definir como NULL.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RegraColuna {
    pub nome: String,
    pub gerada: bool,
    /// O apelido do rowid (a célula abre a ficha; o NULL nela trocaria a chave da linha).
    pub abre: bool,
    /// Coluna de texto (afinidade TEXT): o Limpar grava o texto vazio.
    pub texto: bool,
    pub nao_nulo: bool,
    /// O DEFAULT como está no esquema ("1", "'São Paulo'", "CURRENT_TIMESTAMP").
    pub padrao: Option<String>,
}

impl RegraColuna {
    pub(crate) fn de(info: &TabelaInfo, col: &ColunaVista) -> RegraColuna {
        let meta = info.colunas.iter().find(|m| m.nome == col.nome);
        RegraColuna {
            nome: col.nome.clone(),
            gerada: col.gerada,
            abre: col.abre,
            texto: meta.is_some_and(|m| m.afinidade == Afinidade::Text),
            nao_nulo: meta.is_some_and(|m| m.nao_nulo),
            padrao: meta.and_then(|m| m.padrao.clone()),
        }
    }

    /// Por que a coluna não recebe o lote (None: recebe).
    fn motivo(&self, tipo: TipoLote) -> Option<String> {
        let n = &self.nome;
        if self.gerada {
            return Some(format!("A coluna {n} é gerada; o valor dela vem da fórmula."));
        }
        if self.abre {
            return Some(format!("A coluna {n} é a chave da linha; mude-a pela ficha."));
        }
        match tipo {
            TipoLote::Limpar if !self.texto && self.nao_nulo && self.padrao.is_none() => {
                Some(format!("A coluna {n} não aceita NULL e não tem valor padrão."))
            }
            TipoLote::Nulo if self.nao_nulo => Some(format!("A coluna {n} não aceita NULL.")),
            _ => None,
        }
    }

    /// O que o Limpar grava nela: o texto vazio na de texto, o NULL onde cabe, o valor padrão na `NOT NULL`.
    fn vazio(&self) -> Entrada {
        if self.texto {
            Entrada::TextoLiteral(String::new())
        } else if !self.nao_nulo {
            Entrada::Nulo
        } else {
            Entrada::Padrao
        }
    }
}

/// Se a grade grava agora.
pub(crate) enum Escrita {
    /// Não grava (visão, tabela sem chave): os itens de escrita não aparecem.
    Nao,
    /// Grava, mas não agora (banco só leitura, gravação a caminho): desligados com o motivo.
    AgoraNao(&'static str),
    /// Grava: as colunas e as células do intervalo decidem.
    Sim(EscritaDoIntervalo),
}

/// O que o intervalo tem que desliga um item de escrita.
pub(crate) struct EscritaDoIntervalo {
    pub colunas: Vec<RegraColuna>,
    /// Um texto cortado ou um BLOB no intervalo.
    pub nao_volta: bool,
    /// Uma linha sem chave no intervalo.
    pub sem_chave: bool,
}

impl EscritaDoIntervalo {
    /// O intervalo (colunas do núcleo; a primeira de dados é `primeira`) sobre a página.
    pub(crate) fn de(iv: Intervalo, primeira: usize, dados: &Dados, info: &TabelaInfo) -> EscritaDoIntervalo {
        let colunas: Vec<usize> = iv.colunas().filter_map(|c| c.checked_sub(primeira)).collect();
        let nao_volta = iv.linhas().any(|l| {
            colunas
                .iter()
                .any(|&c| matches!(dados.celula(l, c), Some(Celula::TextoCortado { .. } | Celula::Blob { .. })))
        });
        EscritaDoIntervalo {
            colunas: colunas.iter().filter_map(|&c| dados.colunas.get(c)).map(|c| RegraColuna::de(info, c)).collect(),
            nao_volta,
            sem_chave: iv.linhas().any(|l| dados.chave(l).is_none()),
        }
    }

    fn motivo(&self, tipo: TipoLote) -> Option<String> {
        if let Some(m) = self.colunas.iter().find_map(|c| c.motivo(tipo)) {
            return Some(m);
        }
        if self.sem_chave {
            return Some(MOTIVO_SEM_CHAVE.to_string());
        }
        self.nao_volta.then(|| MOTIVO_NAO_VOLTA.to_string())
    }
}

/// O que o menu oferece nos Dados: os itens de copiar; os de escrita (numa grade que grava: Colar, Limpar e Definir
/// como NULL pelas colunas do intervalo, Excluir as linhas); o filtro pela célula ativa (desligado num BLOB e num texto
/// cortado); marcar as linhas (com as caixas de marcar) e a ficha da linha.
pub(crate) fn ofertas(iv: Intervalo, ativa: Option<&Celula>, escrita: &Escrita, tem_marca: bool) -> OfertasCelula {
    let mut o = OfertasCelula::de_leitura(iv);
    let lotes = [
        (AcaoCelula::Colar, TipoLote::Colar),
        (AcaoCelula::Limpar, TipoLote::Limpar),
        (AcaoCelula::DefinirNulo, TipoLote::Nulo),
    ];
    match escrita {
        Escrita::Nao => {}
        Escrita::AgoraNao(motivo) => {
            for acao in [AcaoCelula::Colar, AcaoCelula::Limpar, AcaoCelula::DefinirNulo, AcaoCelula::ExcluirLinhas] {
                o = o.desligar(acao, *motivo);
            }
        }
        Escrita::Sim(e) => {
            for (acao, tipo) in lotes {
                o = match e.motivo(tipo) {
                    Some(m) => o.desligar(acao, m),
                    None => o.ligar(acao),
                };
            }
            o = o.ligar(AcaoCelula::ExcluirLinhas);
        }
    }
    for acao in [AcaoCelula::FiltrarPorValor, AcaoCelula::ExcluirValorDoFiltro] {
        o = match ativa {
            Some(Celula::Blob { .. }) => o.desligar(acao, MOTIVO_FILTRO_BLOB),
            Some(Celula::TextoCortado { .. }) => o.desligar(acao, MOTIVO_FILTRO_CORTADO),
            _ => o.ligar(acao),
        };
    }
    if tem_marca {
        o = o.ligar(AcaoCelula::MarcarLinhas);
    }
    o.ligar(AcaoCelula::AbrirFicha)
}

/// O Colar, o Limpar e o Definir como NULL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TipoLote {
    Colar,
    Limpar,
    Nulo,
}

impl TipoLote {
    /// O nome da ação (o título da pergunta e a dica do Desfazer).
    pub(crate) fn acao(self) -> &'static str {
        match self {
            TipoLote::Colar => "Colar",
            TipoLote::Limpar => "Limpar",
            TipoLote::Nulo => "Definir como NULL",
        }
    }

    /// A pergunta acima de [`LIMITE_SEM_PERGUNTA`] células: "Colar 120 células em clientes?".
    pub(crate) fn pergunta(self, n: usize, tabela: &str) -> String {
        let celulas = plural(n, "célula", "células");
        match self {
            TipoLote::Colar => format!("Colar {celulas} em {tabela}?"),
            TipoLote::Limpar => format!("Limpar {celulas} em {tabela}?"),
            TipoLote::Nulo => format!("Definir {celulas} de {tabela} como NULL?"),
        }
    }
}

/// Uma célula do lote: onde está na página, a chave da linha, o que grava e o valor de antes (para o Desfazer).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CelulaDoLote {
    pub linha: usize,
    /// A coluna de dados.
    pub coluna: usize,
    pub chave: Chave,
    pub nome: String,
    pub entrada: Entrada,
    pub antigo: Celula,
}

/// O que um Colar, Limpar ou Definir como NULL grava, numa transação só.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Lote {
    pub tipo: TipoLote,
    /// A página em que as células foram lidas.
    pub geracao: u64,
    /// Só as que mudam.
    pub celulas: Vec<CelulaDoLote>,
    /// Quantas a cola deixou de fora (passavam da última linha ou coluna).
    pub de_fora: u64,
}

impl Lote {
    /// O que vai para o `Servico::atualizar_intervalo`.
    pub(crate) fn pedido(&self) -> Vec<(Chave, String, Entrada)> {
        self.celulas.iter().map(|c| (c.chave.clone(), c.nome.clone(), c.entrada.clone())).collect()
    }

    /// O registro do Desfazer, com o que o banco releu de cada célula (None: uma linha não voltou relida, e o lote não
    /// se desfaz).
    pub(crate) fn para_desfazer(&self, gravadas: &[banco::LinhaGravada]) -> Option<Vec<CelulaDesfeita>> {
        if gravadas.len() != self.celulas.len() {
            return None;
        }
        self.celulas
            .iter()
            .zip(gravadas)
            .map(|(c, g)| {
                Some(CelulaDesfeita {
                    chave: g.chave.clone()?,
                    coluna: c.nome.clone(),
                    gravado: g.linha.as_ref()?.get(c.coluna)?.clone(),
                    antigo: c.antigo.clone(),
                })
            })
            .collect()
    }
}

/// O lote das células `alvos` (linha e coluna de dados; o texto, no Colar): cada uma com a entrada do `tipo`, sem as
/// que já têm o valor. Uma célula que não pode receber o lote (a coluna gerada, a chave do rowid, a `NOT NULL`, o texto
/// longo ou o BLOB, a linha sem chave) recusa o lote inteiro, com o motivo.
pub(crate) fn montar_lote(
    tipo: TipoLote,
    alvos: impl IntoIterator<Item = (usize, usize, Option<String>)>,
    de_fora: u64,
    dados: &Dados,
    info: &TabelaInfo,
) -> Result<Lote, String> {
    let regras: Vec<RegraColuna> = dados.colunas.iter().map(|c| RegraColuna::de(info, c)).collect();
    let mut celulas = Vec::new();
    for (l, c, texto) in alvos {
        let (Some(regra), Some(antigo)) = (regras.get(c), dados.celula(l, c)) else { continue };
        if let Some(m) = regra.motivo(tipo) {
            return Err(m);
        }
        if matches!(antigo, Celula::TextoCortado { .. } | Celula::Blob { .. }) {
            return Err(MOTIVO_NAO_VOLTA.to_string());
        }
        let chave = dados.chave(l).cloned().ok_or_else(|| MOTIVO_SEM_CHAVE.to_string())?;
        let entrada = match tipo {
            TipoLote::Colar => {
                let t = texto.unwrap_or_default();
                let igual = match antigo {
                    Celula::Nulo => t.is_empty(),
                    o => t == texto_edicao(o),
                };
                if igual {
                    continue;
                }
                Entrada::Texto(t)
            }
            TipoLote::Limpar => {
                let e = regra.vazio();
                let ja = match &e {
                    Entrada::TextoLiteral(t) => matches!(antigo, Celula::Texto(a) if a == t),
                    Entrada::Nulo => antigo.e_nulo(),
                    // o DEFAULT que é um número escrito: a célula que já o tem fica
                    _ => regra.padrao.as_deref().is_some_and(|p| antigo.e_numero() && p.trim() == texto_celula(antigo)),
                };
                if ja {
                    continue;
                }
                e
            }
            TipoLote::Nulo => {
                if antigo.e_nulo() {
                    continue;
                }
                Entrada::Nulo
            }
        };
        celulas.push(CelulaDoLote {
            linha: l,
            coluna: c,
            chave,
            nome: regra.nome.clone(),
            entrada,
            antigo: antigo.clone(),
        });
    }
    Ok(Lote { tipo, geracao: dados.geracao, celulas, de_fora })
}

/// A condição do "Filtrar por este valor" (`excluir` false) ou do "Excluir este valor do filtro": "igual a" e
/// "diferente de" pelo texto da célula; no NULL, "é nulo" e "não é nulo"; no texto vazio, "vazio" e "não vazio". None
/// num BLOB e num texto cortado.
pub(crate) fn condicao_do_valor(celula: &Celula, excluir: bool) -> Option<(Operador, String)> {
    let par = |sim, nao| if excluir { nao } else { sim };
    match celula {
        Celula::Blob { .. } | Celula::TextoCortado { .. } => None,
        Celula::Nulo => Some((par(Operador::Nulo, Operador::NaoNulo), String::new())),
        Celula::Texto(t) if t.is_empty() => Some((par(Operador::Vazio, Operador::NaoVazio), String::new())),
        _ => Some((par(Operador::Igual, Operador::Diferente), texto_celula(celula))),
    }
}

/// As células da página para copiar: a que está gravando agora vai com o texto digitado.
pub(crate) struct FonteCopia<'a> {
    pub dados: &'a Dados,
    pub pendentes: &'a [Pendente],
    /// O índice, no núcleo, da primeira coluna de dados.
    pub primeira: usize,
    /// A tabela do `INSERT`.
    pub tabela: &'a str,
}

impl FonteCopia<'_> {
    fn pendente(&self, l: usize, c: usize) -> Option<&str> {
        let p = self.pendentes.iter().rev().find(|g| g.geracao == self.dados.geracao && g.linha == l && g.coluna == c);
        p.map(|g| g.texto.as_str())
    }

    /// O texto de uma célula (coluna do núcleo), como o Ctrl+C leva.
    fn texto(&self, l: usize, c: usize) -> Option<String> {
        let c = c.checked_sub(self.primeira)?;
        self.pendente(l, c).map(str::to_string).or_else(|| self.dados.celula(l, c).map(texto_celula))
    }

    /// A célula para os formatos do "Copiar como" (a que grava agora: número, se a coluna é numérica e o texto é um).
    fn valor(&self, l: usize, c: usize) -> ValorCopia<'static> {
        let Some(c) = c.checked_sub(self.primeira) else { return ValorCopia::Nulo };
        if let Some(t) = self.pendente(l, c) {
            let numerica = self.dados.colunas.get(c).is_some_and(|col| col.numerica);
            return match t.trim().parse::<f64>() {
                Ok(n) if numerica && n.is_finite() => ValorCopia::numero(t.trim().to_string()),
                _ => ValorCopia::texto(t.to_string()),
            };
        }
        self.dados.celula(l, c).map_or(ValorCopia::Nulo, valor_de_copia)
    }

    fn nome(&self, c: usize) -> String {
        c.checked_sub(self.primeira)
            .and_then(|c| self.dados.colunas.get(c))
            .map(|col| col.nome.clone())
            .unwrap_or_default()
    }

    /// O texto que a ação de copiar leva do intervalo (o Copiar de uma célula só: o texto dela, sem o TSV).
    pub(crate) fn texto_da_acao(&self, acao: AcaoCelula, iv: Intervalo) -> Option<String> {
        Some(match acao {
            AcaoCelula::Copiar if iv.varias() => tsv_do_intervalo(iv, |l, c| self.texto(l, c)),
            AcaoCelula::Copiar => self.texto(iv.linha0, iv.coluna0).unwrap_or_default(),
            AcaoCelula::CopiarComCabecalho => {
                tsv_do_intervalo_com_cabecalho(iv, |c| self.nome(c), |l, c| self.texto(l, c))
            }
            AcaoCelula::CopiarComo(formato) => {
                let opcoes = OpcoesCopia { separador: ',', tabela: self.tabela };
                texto_do_intervalo_como(formato, iv, |c| self.nome(c), |l, c| self.valor(l, c), &opcoes)
            }
            _ => return None,
        })
    }

    /// Copia o intervalo como a ação pede e avisa no toast ("3 linhas × 2 colunas copiadas.").
    pub(crate) fn copiar(&self, ctx: &egui::Context, acao: AcaoCelula, iv: Intervalo) {
        if let Some(texto) = self.texto_da_acao(acao, iv) {
            ctx.copy_text(texto);
            toast_concluido(ctx, frase_do_intervalo_copiado(iv));
        }
    }

    /// As contas do rodapé da seleção: o tipo da coluna decide o que é número.
    pub(crate) fn resumo(&self, iv: Intervalo) -> ResumoSelecao {
        let mut r = ResumoSelecao::default();
        for l in iv.linhas() {
            for c in iv.colunas() {
                let Some(dc) = c.checked_sub(self.primeira) else { continue };
                let numerica = self.dados.colunas.get(dc).is_some_and(|col| col.numerica);
                match self.pendente(l, dc) {
                    Some(t) if numerica => match t.trim().parse::<f64>() {
                        Ok(n) => r.somar_numero(n, casas(t)),
                        Err(_) => r.somar_texto(t),
                    },
                    Some(t) => r.somar_texto(t),
                    None => {
                        if let Some(celula) = self.dados.celula(l, dc) {
                            r.somar_celula(celula, numerica);
                        }
                    }
                }
            }
        }
        r
    }
}

/// As casas decimais de um número digitado ("12.50" → 2).
fn casas(texto: &str) -> u8 {
    texto.trim().split_once('.').map_or(0, |(_, f)| u8::try_from(f.len()).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod testes {
    use super::*;
    use interface::grade::OfertaCelula;

    #[test]
    fn condicoes_pelo_valor() {
        assert_eq!(condicao_do_valor(&Celula::Nulo, false), Some((Operador::Nulo, String::new())));
        assert_eq!(condicao_do_valor(&Celula::Nulo, true), Some((Operador::NaoNulo, String::new())));
        assert_eq!(condicao_do_valor(&Celula::Texto(String::new()), true), Some((Operador::NaoVazio, String::new())));
        assert_eq!(condicao_do_valor(&Celula::Real(17051.0), false), Some((Operador::Igual, "17051".into())));
        assert_eq!(condicao_do_valor(&Celula::Texto("Ana".into()), true), Some((Operador::Diferente, "Ana".into())));
        assert_eq!(condicao_do_valor(&Celula::Blob { tamanho: 1, inicio: vec![0] }, false), None);
    }

    fn regra(nome: &str) -> RegraColuna {
        RegraColuna { nome: nome.into(), gerada: false, abre: false, texto: false, nao_nulo: false, padrao: None }
    }

    fn sim(colunas: Vec<RegraColuna>) -> Escrita {
        Escrita::Sim(EscritaDoIntervalo { colunas, nao_volta: false, sem_chave: false })
    }

    #[test]
    fn ofertas_dos_dados() {
        let iv = Intervalo::entre((0, 2), (1, 3));
        let o = ofertas(iv, Some(&Celula::Inteiro(1)), &sim(vec![regra("cidade")]), true);
        assert!(o.ligado(AcaoCelula::FiltrarPorValor) && o.ligado(AcaoCelula::MarcarLinhas));
        assert!(o.ligado(AcaoCelula::AbrirFicha));
        for acao in [AcaoCelula::Colar, AcaoCelula::Limpar, AcaoCelula::DefinirNulo, AcaoCelula::ExcluirLinhas] {
            assert!(o.ligado(acao), "{acao:?}");
        }
        let ro = ofertas(iv, None, &Escrita::AgoraNao(MOTIVO_SO_LEITURA), true);
        assert_eq!(*ro.get(AcaoCelula::ExcluirLinhas), OfertaCelula::Desligada(MOTIVO_SO_LEITURA.into()));
        let visao = ofertas(iv, Some(&Celula::Blob { tamanho: 1, inicio: vec![0] }), &Escrita::Nao, false);
        assert_eq!(*visao.get(AcaoCelula::Colar), OfertaCelula::Oculta, "a visão não grava");
        assert_eq!(*visao.get(AcaoCelula::MarcarLinhas), OfertaCelula::Oculta);
        assert_eq!(*visao.get(AcaoCelula::FiltrarPorValor), OfertaCelula::Desligada(MOTIVO_FILTRO_BLOB.into()));
    }

    #[test]
    fn colunas_que_nao_recebem_o_lote() {
        let iv = Intervalo::entre((0, 2), (1, 3));
        let motivo = |e: &Escrita, acao| match ofertas(iv, None, e, false).get(acao) {
            OfertaCelula::Desligada(m) => Some(m.to_string()),
            _ => None,
        };
        let gerada = RegraColuna { gerada: true, ..regra("preco_centavos") };
        let e = sim(vec![regra("cidade"), gerada]);
        let m = "A coluna preco_centavos é gerada; o valor dela vem da fórmula.";
        assert_eq!(motivo(&e, AcaoCelula::Colar).as_deref(), Some(m));
        assert_eq!(motivo(&e, AcaoCelula::ExcluirLinhas), None, "excluir as linhas não olha as colunas");
        // NOT NULL: o Limpar grava o texto vazio na de texto e o padrão na que tem; o NULL não vale
        let ativo = RegraColuna { nao_nulo: true, padrao: Some("1".into()), ..regra("ativo") };
        let nome = RegraColuna { nao_nulo: true, texto: true, ..regra("nome") };
        let e = sim(vec![ativo.clone(), nome]);
        assert_eq!(motivo(&e, AcaoCelula::Limpar), None);
        assert_eq!(motivo(&e, AcaoCelula::DefinirNulo).as_deref(), Some("A coluna ativo não aceita NULL."));
        assert_eq!((ativo.vazio(), regra("x").vazio()), (Entrada::Padrao, Entrada::Nulo));
        let sem_padrao = sim(vec![RegraColuna { nao_nulo: true, ..regra("qtd") }]);
        assert_eq!(
            motivo(&sem_padrao, AcaoCelula::Limpar).as_deref(),
            Some("A coluna qtd não aceita NULL e não tem valor padrão.")
        );
        let longo = Escrita::Sim(EscritaDoIntervalo { colunas: vec![regra("x")], nao_volta: true, sem_chave: false });
        assert_eq!(motivo(&longo, AcaoCelula::Colar).as_deref(), Some(MOTIVO_NAO_VOLTA));
        assert_eq!(TipoLote::Colar.pergunta(120, "clientes"), "Colar 120 células em clientes?");
        assert_eq!(TipoLote::Nulo.pergunta(101, "clientes"), "Definir 101 células de clientes como NULL?");
    }
}
