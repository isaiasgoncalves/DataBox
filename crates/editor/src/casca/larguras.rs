//! As larguras dos painéis laterais (o Explorador ou o Histórico à esquerda, as Propriedades à direita) e como o corpo
//! da janela as reparte: a pessoa arrasta a divisória de cada painel (`interface::pecas::divisoria`), o que ela
//! escolhe vale entre as sessões (a chave `"larguras_paineis"` das preferências) e a janela, quando fica estreita, encolhe os
//! painéis antes de esconder as Propriedades.
//!
//! - **Limites:** de [`MINIMA`] (200 pt) a [`FRACAO_MAXIMA`] (45%) da largura da janela, e o documento nunca fica
//!   abaixo de [`DOCUMENTO_MINIMO`] (560 pt, a base do `flex:999 1 560px` das pranchetas): o máximo de um painel
//!   também desconta o que o outro ocupa.
//! - **Janela estreita:** a escolha da pessoa ([`Larguras`]) não muda; o que se desenha é a [`Disposicao`]. Se o
//!   documento ficaria abaixo do mínimo, os dois painéis encolhem (cada um na proporção do que tem acima de
//!   [`MINIMA`]); só quando nem os dois no mínimo cabem é que as Propriedades somem (e o Explorador, sozinho, ainda
//!   encolhe se precisar). Com as larguras padrão, as Propriedades somem abaixo de ~1052 pt de janela, não mais de
//!   ~1182; com elas mais estreitas, ficam à vista por mais tempo.
//! - **Arrastar numa janela apertada:** o que a pessoa vê passa a ser a escolha dela (a largura efetiva do outro
//!   painel vira a guardada dele), senão o painel arrastado não acompanharia o ponteiro.

use interface::pecas::divisoria::LimitesLargura;

use crate::casca::propriedades;

/// As larguras padrão (`aside.vidro{width:258px}` e `aside{width:272px}` das pranchetas).
pub(crate) const EXPLORADOR_PADRAO: f32 = 258.0;
pub(crate) const PROPRIEDADES_PADRAO: f32 = propriedades::LARGURA;
/// A largura mínima de um painel.
pub(crate) const MINIMA: f32 = 200.0;
/// O máximo de um painel, em fração da largura da janela.
pub(crate) const FRACAO_MAXIMA: f32 = 0.45;
/// O documento não fica abaixo disto (senão as Propriedades somem).
pub(crate) const DOCUMENTO_MINIMO: f32 = propriedades::DOCUMENTO_MINIMO;

/// As larguras que a pessoa escolheu (as padrão enquanto ela não arrastar): são só a escolha, o que se desenha é a
/// [`Disposicao`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Larguras {
    pub explorador: f32,
    pub propriedades: f32,
}

impl Default for Larguras {
    fn default() -> Self {
        Larguras { explorador: EXPLORADOR_PADRAO, propriedades: PROPRIEDADES_PADRAO }
    }
}

impl From<prefs::LarguraPaineis> for Larguras {
    /// A que nunca foi escolhida (None) fica a padrão.
    fn from(p: prefs::LarguraPaineis) -> Self {
        Larguras {
            explorador: p.explorador.unwrap_or(EXPLORADOR_PADRAO),
            propriedades: p.propriedades.unwrap_or(PROPRIEDADES_PADRAO),
        }
    }
}

/// O espaço do corpo da janela (da borda esquerda da barra de ícones à direita), com o que fica fora dos painéis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Corpo {
    /// A largura da janela (a base dos 45%).
    pub janela: f32,
    /// A largura do corpo (a janela sem o `padding` dos lados).
    pub largura: f32,
    /// A barra de ícones e o `gap` entre as colunas.
    pub icones: f32,
    pub espaco: f32,
}

/// Como o corpo se reparte agora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Disposicao {
    /// As larguras que se desenham (a do painel que não está à vista é só a escolha, limitada).
    pub explorador: f32,
    pub propriedades: f32,
    /// As Propriedades cabem (e a pessoa as quer e o documento as tem).
    pub com_propriedades: bool,
    /// Até onde cada divisória pode levar o painel agora.
    pub maximo_explorador: f32,
    pub maximo_propriedades: f32,
}

impl Disposicao {
    /// Os limites do Explorador para a divisória.
    pub(crate) fn limites_explorador(&self) -> LimitesLargura {
        LimitesLargura {
            atual: self.explorador,
            minima: MINIMA,
            maxima: self.maximo_explorador,
            padrao: EXPLORADOR_PADRAO,
        }
    }

    /// Os limites das Propriedades para a divisória.
    pub(crate) fn limites_propriedades(&self) -> LimitesLargura {
        LimitesLargura {
            atual: self.propriedades,
            minima: MINIMA,
            maxima: self.maximo_propriedades,
            padrao: PROPRIEDADES_PADRAO,
        }
    }
}

/// O que sobra do corpo para os painéis, sem o documento mínimo: o corpo menos a barra de ícones, os `gap`s (um
/// depois da barra, um depois de cada painel à vista) e o documento.
fn orcamento(corpo: &Corpo, explorador: bool, propriedades: bool) -> f32 {
    let gaps = 1 + usize::from(explorador) + usize::from(propriedades);
    corpo.largura - corpo.icones - gaps as f32 * corpo.espaco - DOCUMENTO_MINIMO
}

/// Encolhe os painéis presentes até a soma caber no `orcamento`, cada um na proporção do que tem acima de
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

/// Reparte o corpo: `com_explorador` é o painel da esquerda à vista (o Explorador ou o Histórico) e
/// `quer_propriedades`, as Propriedades pedidas pela pessoa para um documento que as tem.
pub(crate) fn dispor(corpo: &Corpo, escolha: Larguras, com_explorador: bool, quer_propriedades: bool) -> Disposicao {
    let teto = (corpo.janela * FRACAO_MAXIMA).floor().max(MINIMA);
    let limitar = |v: f32| v.round().clamp(MINIMA, teto);
    let quer = [limitar(escolha.explorador), limitar(escolha.propriedades)];

    let (mut largura, cabe) =
        encolher(quer, [com_explorador, quer_propriedades], orcamento(corpo, com_explorador, quer_propriedades));
    if quer_propriedades && !cabe {
        // nem os dois no mínimo cabem: as Propriedades somem e o Explorador fica sozinho
        let sozinho = encolher(quer, [com_explorador, false], orcamento(corpo, com_explorador, false));
        largura = [sozinho.0[0], quer[1]];
    }
    let com_propriedades = quer_propriedades && cabe;
    let (explorador, propriedades) = (largura[0], largura[1]);

    // até onde cada divisória leva o painel: o teto, e o que o documento mínimo deixa depois do outro painel à vista
    let orc = orcamento(corpo, com_explorador, com_propriedades);
    let maximo_explorador = teto.min(orc - if com_propriedades { propriedades } else { 0.0 }).max(MINIMA);
    let maximo_propriedades = teto.min(orc - if com_explorador { explorador } else { 0.0 }).max(MINIMA);
    Disposicao { explorador, propriedades, com_propriedades, maximo_explorador, maximo_propriedades }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Uma janela de `largura` (8px de `padding`, a barra de ícones de 52px e os `gap`s de 8px).
    fn janela(largura: f32) -> Corpo {
        Corpo { janela: largura, largura: largura - 16.0, icones: 52.0, espaco: 8.0 }
    }

    #[test]
    fn padrao_em_janela_larga_fica_como_sempre() {
        let d = dispor(&janela(1440.0), Larguras::default(), true, true);
        assert_eq!((d.explorador, d.propriedades, d.com_propriedades), (258.0, 272.0, true));
        // o máximo de cada um: 45% da janela (648) ou o que o documento mínimo deixa depois do outro painel
        // (1424 − 52 − 24 − 560 = 788 menos o outro)
        assert_eq!(d.maximo_explorador, 516.0);
        assert_eq!(d.maximo_propriedades, 530.0);
        assert_eq!(d.limites_explorador().padrao, 258.0);
        assert_eq!(d.limites_propriedades().padrao, 272.0);
    }

    #[test]
    fn o_maximo_e_45_por_cento_da_janela_quando_o_documento_sobra() {
        let d = dispor(&janela(2560.0), Larguras::default(), true, true);
        assert_eq!((d.maximo_explorador, d.maximo_propriedades), (1152.0, 1152.0));
        // sem as Propriedades, o limite do Explorador não desconta o painel que não existe
        let d = dispor(&janela(1200.0), Larguras::default(), true, false);
        assert_eq!(d.maximo_explorador, 540.0);
        let d = dispor(&janela(1200.0), Larguras::default(), false, true);
        assert_eq!(d.maximo_propriedades, 540.0);
    }

    #[test]
    fn a_escolha_e_limitada_ao_minimo_e_ao_maximo() {
        let d = dispor(&janela(1440.0), Larguras { explorador: 50.0, propriedades: 9000.0 }, true, true);
        assert_eq!(d.explorador, 200.0);
        // 9000 vira o teto (648) e depois encolhe para caber com o outro e o documento mínimo: 788 − 200 = 588
        assert_eq!(d.propriedades, 588.0);
        assert!(d.com_propriedades);
    }

    #[test]
    fn janela_estreita_encolhe_os_paineis_antes_de_esconder_as_propriedades() {
        // com os dois nas larguras padrão, as Propriedades sumiam abaixo de 1182; agora cabem até 1052
        for largura in [1100.0, 1060.0, 1053.0] {
            let d = dispor(&janela(largura), Larguras::default(), true, true);
            assert!(d.com_propriedades, "{largura}");
            assert!(d.explorador >= MINIMA && d.propriedades >= MINIMA, "{largura}: {d:?}");
            // o documento é o que sobra: nunca abaixo do mínimo
            let documento = largura - 16.0 - 52.0 - 24.0 - d.explorador - d.propriedades;
            assert!(documento >= DOCUMENTO_MINIMO, "{largura}: {documento}");
        }
        let d = dispor(&janela(1100.0), Larguras::default(), true, true);
        // encolhem na proporção do que têm acima do mínimo (58 e 72)
        assert!(d.explorador < 258.0 && d.propriedades < 272.0, "{d:?}");
        // no mínimo exato, os dois no mínimo
        let d = dispor(&janela(1052.0), Larguras::default(), true, true);
        assert_eq!((d.explorador, d.propriedades, d.com_propriedades), (200.0, 200.0, true));
        // um pixel a menos: as Propriedades somem e o Explorador volta a ter o que pediu
        let d = dispor(&janela(1051.0), Larguras::default(), true, true);
        assert!(!d.com_propriedades);
        assert_eq!(d.explorador, 258.0);
    }

    #[test]
    fn propriedades_mais_estreitas_deixam_o_explorador_inteiro_por_mais_tempo() {
        let estreitas = Larguras { explorador: 258.0, propriedades: 200.0 };
        // com as de 200, o Explorador só encolhe abaixo de 1110 de janela (com as padrão, abaixo de 1182)
        let d = dispor(&janela(1110.0), estreitas, true, true);
        assert_eq!((d.explorador, d.propriedades, d.com_propriedades), (258.0, 200.0, true));
        let d = dispor(&janela(1109.0), estreitas, true, true);
        assert!(d.com_propriedades && d.propriedades == 200.0 && d.explorador < 258.0, "{d:?}");
        let d = dispor(&janela(1110.0), Larguras::default(), true, true);
        assert!(d.com_propriedades && d.explorador < 258.0 && d.propriedades < 272.0, "{d:?}");
        let d = dispor(&janela(1182.0), Larguras::default(), true, true);
        assert_eq!((d.explorador, d.propriedades), (258.0, 272.0));
        // com o Explorador escondido, as Propriedades cabem inteiras muito antes (a 960, como nos testes de janela
        // estreita) e ainda encolhem um pouco abaixo disso
        let d = dispor(&janela(960.0), Larguras::default(), false, true);
        assert_eq!((d.propriedades, d.com_propriedades), (272.0, true));
        let d = dispor(&janela(900.0), Larguras::default(), false, true);
        assert_eq!((d.propriedades, d.com_propriedades), (256.0, true));
    }

    #[test]
    fn explorador_sozinho_encolhe_se_for_preciso() {
        // 800 de janela: o corpo de 784 deixa 784 − 52 − 16 − 560 = 156 abaixo do mínimo: fica no mínimo
        let d = dispor(&janela(800.0), Larguras::default(), true, false);
        assert_eq!(d.explorador, 200.0);
        assert!(!d.com_propriedades);
        // sem nenhum painel, nada a encolher
        let d = dispor(&janela(800.0), Larguras::default(), false, false);
        assert!(!d.com_propriedades);
    }

    #[test]
    fn sem_propriedades_pedidas_nao_ha_o_que_esconder() {
        let d = dispor(&janela(1440.0), Larguras::default(), true, false);
        assert!(!d.com_propriedades);
        assert_eq!(d.explorador, 258.0);
    }

    #[test]
    fn as_prefs_viram_a_escolha_e_o_que_falta_fica_padrao() {
        let l = Larguras::from(prefs::LarguraPaineis { explorador: Some(300.0), propriedades: None });
        assert_eq!(l, Larguras { explorador: 300.0, propriedades: 272.0 });
        assert_eq!(Larguras::from(prefs::LarguraPaineis::default()), Larguras::default());
    }
}
