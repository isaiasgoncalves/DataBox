//! As larguras das docas (a da esquerda, do Explorador e do Histórico no padrão, e a da direita, das Propriedades; ver
//! [`crate::casca::docas`]) e como o corpo da janela as reparte: a pessoa arrasta a divisória de cada doca
//! (`interface::pecas::divisoria`), o que ela escolhe vale entre as sessões (a chave `"larguras_paineis"` das
//! preferências, com os nomes de antes: `"explorador"` é a da esquerda e `"propriedades"`, a da direita) e a janela,
//! quando fica estreita, encolhe as docas antes de esconder a da direita.
//!
//! - **Limites:** de [`MINIMA`] (200 pt) a [`FRACAO_MAXIMA`] (45%) da largura da janela, e o documento nunca fica
//!   abaixo de [`DOCUMENTO_MINIMO`] (560 pt, a base do `flex:999 1 560px` das pranchetas): o máximo de uma doca
//!   também desconta o que a outra ocupa.
//! - **Janela estreita:** a escolha da pessoa ([`Larguras`]) não muda; o que se desenha é a [`Disposicao`]. Se o
//!   documento ficaria abaixo do mínimo, as duas docas encolhem (cada uma na proporção do que tem acima de
//!   [`MINIMA`]); só quando nem as duas no mínimo cabem é que a da direita some (e a da esquerda, sozinha, ainda
//!   encolhe se precisar). Com as larguras padrão, a da direita some abaixo de ~992 pt de janela, e as duas começam a
//!   encolher abaixo de ~1122; com elas mais estreitas, ficam à vista por mais tempo.
//! - **Arrastar numa janela apertada:** o que a pessoa vê passa a ser a escolha dela (a largura efetiva da outra
//!   doca vira a guardada dela), senão a doca arrastada não acompanharia o ponteiro.

use interface::pecas::divisoria::LimitesLargura;

use crate::casca::propriedades;

/// As larguras padrão (`.doca{width:258px}` à esquerda e `aside{width:272px}` à direita, nas pranchetas).
pub(crate) const ESQUERDA_PADRAO: f32 = 258.0;
pub(crate) const DIREITA_PADRAO: f32 = propriedades::LARGURA;
/// A largura mínima de uma doca.
pub(crate) const MINIMA: f32 = 200.0;
/// O máximo de uma doca, em fração da largura da janela.
pub(crate) const FRACAO_MAXIMA: f32 = 0.45;
/// O documento não fica abaixo disto (senão a doca da direita some).
pub(crate) const DOCUMENTO_MINIMO: f32 = propriedades::DOCUMENTO_MINIMO;

/// As larguras que a pessoa escolheu (as padrão enquanto ela não arrastar): são só a escolha, o que se desenha é a
/// [`Disposicao`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Larguras {
    pub esquerda: f32,
    pub direita: f32,
}

impl Default for Larguras {
    fn default() -> Self {
        Larguras { esquerda: ESQUERDA_PADRAO, direita: DIREITA_PADRAO }
    }
}

impl From<prefs::LarguraPaineis> for Larguras {
    /// A que nunca foi escolhida (None) fica a padrão.
    fn from(p: prefs::LarguraPaineis) -> Self {
        Larguras {
            esquerda: p.explorador.unwrap_or(ESQUERDA_PADRAO),
            direita: p.propriedades.unwrap_or(DIREITA_PADRAO),
        }
    }
}

/// O espaço do corpo da janela (sem o `padding` dos lados), com o `gap` entre as colunas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Corpo {
    /// A largura da janela (a base dos 45%).
    pub janela: f32,
    /// A largura do corpo (a janela sem o `padding` dos lados).
    pub largura: f32,
    pub espaco: f32,
}

/// Como o corpo se reparte agora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Disposicao {
    /// As larguras que se desenham (a da doca que não está à vista é só a escolha, limitada).
    pub esquerda: f32,
    pub direita: f32,
    /// A doca da direita cabe (e a pessoa a quer e ela tem o que mostrar).
    pub com_direita: bool,
    /// Até onde cada divisória pode levar a doca agora.
    pub maximo_esquerda: f32,
    pub maximo_direita: f32,
}

impl Disposicao {
    /// Os limites da doca da esquerda para a divisória.
    pub(crate) fn limites_esquerda(&self) -> LimitesLargura {
        LimitesLargura { atual: self.esquerda, minima: MINIMA, maxima: self.maximo_esquerda, padrao: ESQUERDA_PADRAO }
    }

    /// Os limites da doca da direita para a divisória.
    pub(crate) fn limites_direita(&self) -> LimitesLargura {
        LimitesLargura { atual: self.direita, minima: MINIMA, maxima: self.maximo_direita, padrao: DIREITA_PADRAO }
    }
}

/// O que sobra do corpo para as docas, sem o documento mínimo: o corpo menos os `gap`s (um entre cada doca à vista e
/// o documento) e o documento.
fn orcamento(corpo: &Corpo, esquerda: bool, direita: bool) -> f32 {
    let gaps = usize::from(esquerda) + usize::from(direita);
    corpo.largura - gaps as f32 * corpo.espaco - DOCUMENTO_MINIMO
}

/// Encolhe as docas presentes até a soma caber no `orcamento`, cada um na proporção do que tem acima de
/// [`MINIMA`]. Devolve as larguras e se coube (nem tudo no mínimo cabendo, fica tudo no mínimo e devolve false).
fn encolher(largura: [f32; 2], presentes: [bool; 2], orcamento: f32) -> ([f32; 2], bool) {
    let soma: f32 = (0..2).filter(|&i| presentes[i]).map(|i| largura[i]).sum();
    if soma <= orcamento {
        return (largura, true);
    }
    let falta = soma - orcamento;
    let folga: f32 = (0..2).filter(|&i| presentes[i]).map(|i| largura[i] - MINIMA).sum();
    let mut saida = largura;
    if falta <= folga {
        for i in (0..2).filter(|&i| presentes[i]) {
            // para baixo: a soma nunca passa do orçamento por causa do arredondamento
            saida[i] = (largura[i] - falta * (largura[i] - MINIMA) / folga).floor().max(MINIMA);
        }
        (saida, true)
    } else {
        for i in (0..2).filter(|&i| presentes[i]) {
            saida[i] = MINIMA;
        }
        (saida, false)
    }
}

/// Reparte o corpo: `com_esquerda` é a doca da esquerda à vista e `quer_direita`, a da direita pedida pela pessoa e
/// com alguma aba (as Propriedades de um documento que as tem, ou outro painel que mora nela).
pub(crate) fn dispor(corpo: &Corpo, escolha: Larguras, com_esquerda: bool, quer_direita: bool) -> Disposicao {
    let teto = (corpo.janela * FRACAO_MAXIMA).floor().max(MINIMA);
    let limitar = |v: f32| v.round().clamp(MINIMA, teto);
    let quer = [limitar(escolha.esquerda), limitar(escolha.direita)];

    let (mut largura, cabe) =
        encolher(quer, [com_esquerda, quer_direita], orcamento(corpo, com_esquerda, quer_direita));
    if quer_direita && !cabe {
        // nem as duas no mínimo cabem: a da direita some e a da esquerda fica sozinha
        let sozinha = encolher(quer, [com_esquerda, false], orcamento(corpo, com_esquerda, false));
        largura = [sozinha.0[0], quer[1]];
    }
    let com_direita = quer_direita && cabe;
    let (esquerda, direita) = (largura[0], largura[1]);

    // até onde cada divisória leva a doca: o teto, e o que o documento mínimo deixa depois da outra doca à vista
    let orc = orcamento(corpo, com_esquerda, com_direita);
    let maximo_esquerda = teto.min(orc - if com_direita { direita } else { 0.0 }).max(MINIMA);
    let maximo_direita = teto.min(orc - if com_esquerda { esquerda } else { 0.0 }).max(MINIMA);
    Disposicao { esquerda, direita, com_direita, maximo_esquerda, maximo_direita }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Uma janela de `largura` (8px de `padding` e os `gap`s de 8px).
    fn janela(largura: f32) -> Corpo {
        Corpo { janela: largura, largura: largura - 16.0, espaco: 8.0 }
    }

    #[test]
    fn padrao_em_janela_larga_fica_como_sempre() {
        let d = dispor(&janela(1440.0), Larguras::default(), true, true);
        assert_eq!((d.esquerda, d.direita, d.com_direita), (258.0, 272.0, true));
        // o máximo de cada uma: 45% da janela (648) ou o que o documento mínimo deixa depois da outra doca
        // (1424 − 16 − 560 = 848 menos a outra)
        assert_eq!(d.maximo_esquerda, 576.0);
        assert_eq!(d.maximo_direita, 590.0);
        assert_eq!(d.limites_esquerda().padrao, 258.0);
        assert_eq!(d.limites_direita().padrao, 272.0);
    }

    #[test]
    fn o_maximo_e_45_por_cento_da_janela_quando_o_documento_sobra() {
        let d = dispor(&janela(2560.0), Larguras::default(), true, true);
        assert_eq!((d.maximo_esquerda, d.maximo_direita), (1152.0, 1152.0));
        // sem a da direita, o limite da esquerda não desconta a doca que não existe
        let d = dispor(&janela(1200.0), Larguras::default(), true, false);
        assert_eq!(d.maximo_esquerda, 540.0);
        let d = dispor(&janela(1200.0), Larguras::default(), false, true);
        assert_eq!(d.maximo_direita, 540.0);
    }

    #[test]
    fn a_escolha_e_limitada_ao_minimo_e_ao_maximo() {
        let d = dispor(&janela(1440.0), Larguras { esquerda: 50.0, direita: 9000.0 }, true, true);
        assert_eq!(d.esquerda, 200.0);
        // 9000 vira o teto (648), que ainda cabe com a outra no mínimo e o documento mínimo (848 − 200)
        assert_eq!(d.direita, 648.0);
        assert!(d.com_direita);
        let d = dispor(&janela(1300.0), Larguras { esquerda: 50.0, direita: 9000.0 }, true, true);
        // 1284 − 16 − 560 − 200 = 508: o teto (585) encolhe até caber
        assert_eq!(d.direita, 508.0);
    }

    #[test]
    fn janela_estreita_encolhe_as_docas_antes_de_esconder_a_da_direita() {
        // com as duas nas larguras padrão, elas encolhem abaixo de 1122 e a da direita cabe até 992
        for largura in [1100.0, 1040.0, 993.0] {
            let d = dispor(&janela(largura), Larguras::default(), true, true);
            assert!(d.com_direita, "{largura}");
            assert!(d.esquerda >= MINIMA && d.direita >= MINIMA, "{largura}: {d:?}");
            // o documento é o que sobra: nunca abaixo do mínimo
            let documento = largura - 16.0 - 16.0 - d.esquerda - d.direita;
            assert!(documento >= DOCUMENTO_MINIMO, "{largura}: {documento}");
        }
        let d = dispor(&janela(1100.0), Larguras::default(), true, true);
        // encolhem na proporção do que têm acima do mínimo (58 e 72)
        assert!(d.esquerda < 258.0 && d.direita < 272.0, "{d:?}");
        // no mínimo exato, as duas no mínimo
        let d = dispor(&janela(992.0), Larguras::default(), true, true);
        assert_eq!((d.esquerda, d.direita, d.com_direita), (200.0, 200.0, true));
        // um pixel a menos: a da direita some e a da esquerda volta a ter o que pediu
        let d = dispor(&janela(991.0), Larguras::default(), true, true);
        assert!(!d.com_direita);
        assert_eq!(d.esquerda, 258.0);
    }

    #[test]
    fn direita_mais_estreita_deixa_a_esquerda_inteira_por_mais_tempo() {
        let estreitas = Larguras { esquerda: 258.0, direita: 200.0 };
        // com a de 200, a da esquerda só encolhe abaixo de 1050 de janela (com as padrão, abaixo de 1122)
        let d = dispor(&janela(1050.0), estreitas, true, true);
        assert_eq!((d.esquerda, d.direita, d.com_direita), (258.0, 200.0, true));
        let d = dispor(&janela(1049.0), estreitas, true, true);
        assert!(d.com_direita && d.direita == 200.0 && d.esquerda < 258.0, "{d:?}");
        let d = dispor(&janela(1050.0), Larguras::default(), true, true);
        assert!(d.com_direita && d.esquerda < 258.0 && d.direita < 272.0, "{d:?}");
        let d = dispor(&janela(1122.0), Larguras::default(), true, true);
        assert_eq!((d.esquerda, d.direita), (258.0, 272.0));
        // com a esquerda escondida, a da direita cabe inteira muito antes (a 960, como nos testes de janela
        // estreita) e ainda encolhe um pouco abaixo disso
        let d = dispor(&janela(960.0), Larguras::default(), false, true);
        assert_eq!((d.direita, d.com_direita), (272.0, true));
        let d = dispor(&janela(840.0), Larguras::default(), false, true);
        assert_eq!((d.direita, d.com_direita), (256.0, true));
    }

    #[test]
    fn esquerda_sozinha_encolhe_se_for_preciso() {
        // 760 de janela: o corpo de 744 deixa 744 − 8 − 560 = 176, abaixo do mínimo: fica no mínimo
        let d = dispor(&janela(760.0), Larguras::default(), true, false);
        assert_eq!(d.esquerda, 200.0);
        assert!(!d.com_direita);
        // sem nenhuma doca, nada a encolher
        let d = dispor(&janela(760.0), Larguras::default(), false, false);
        assert!(!d.com_direita);
    }

    #[test]
    fn sem_a_da_direita_pedida_nao_ha_o_que_esconder() {
        let d = dispor(&janela(1440.0), Larguras::default(), true, false);
        assert!(!d.com_direita);
        assert_eq!(d.esquerda, 258.0);
    }

    #[test]
    fn as_prefs_viram_a_escolha_e_o_que_falta_fica_padrao() {
        let l = Larguras::from(prefs::LarguraPaineis { explorador: Some(300.0), propriedades: None });
        assert_eq!(l, Larguras { esquerda: 300.0, direita: 272.0 });
        assert_eq!(Larguras::from(prefs::LarguraPaineis::default()), Larguras::default());
    }
}
