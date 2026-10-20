//! Onde mora cada painel da casca (revisão de UI de 19/10/2026, pranchetas 1, 2, 2b, 3 e 6): o Explorador, o
//! Histórico de consultas e as Propriedades moram cada um num de três lugares, a doca da esquerda, a da direita ou
//! uma janela própria do sistema ([`Lugar`]). A barra de ícones (o trilho) saiu: com só três painéis, ela repetia
//! as abas da doca. Os atalhos dela ficaram no Início (os botões "Painéis") e nos menus (Exibir › Início, Explorador,
//! Propriedades e Histórico de consultas; Arquivo › Preferências).
//!
//! - **As docas** mostram uma aba por painel que mora nelas, na ordem Explorador, Histórico, Propriedades; a aba à
//!   vista é a de [`Paineis::ativo`]. A da esquerda tem as abas só ícone e a da direita, com o nome
//!   ([`interface::pecas::doca::FormaAbas`]): o painel adota a forma da doca onde está. As Propriedades só têm aba
//!   quando o documento à vista as tem (o Início não tem); uma doca sem aba nenhuma some.
//! - **Mostrar e esconder** (Ctrl+B, Ctrl+Alt+B, o Exibir, o botão da barra de ferramentas, os botões do Início):
//!   [`Paineis::alternar`] põe o painel à vista na doca dele (a doca aparece com a aba dele) ou, se ele já está à
//!   vista, esconde a doca inteira; o painel numa janela própria abre ou fecha a janela.
//! - **Mudar de lugar** ([`Paineis::mover`]): arrastar a aba para a outra doca prende o painel lá; soltar fora da
//!   janela, ou o "Destacar em janela", abre a janela própria; arrastar a aba da janela de volta (ou a silhueta
//!   espelhada dela) prende de novo, na doca de onde ele saiu ([`Paineis::prender`]). Onde cada um mora fica nas
//!   preferências (`"lugares_paineis"`): o que está à vista, não.

use interface::icones::Icone;
use prefs::{LugarPainel, LugaresPaineis};

/// Um painel da casca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Painel {
    Explorador,
    Historico,
    Propriedades,
}

impl Painel {
    /// Todos, na ordem das abas de uma doca.
    pub(crate) const TODOS: [Painel; 3] = [Painel::Explorador, Painel::Historico, Painel::Propriedades];

    fn indice(self) -> usize {
        self as usize
    }

    /// O nome na aba, na dica e no título da janela própria.
    pub(crate) fn nome(self) -> &'static str {
        match self {
            Painel::Explorador => "Explorador",
            Painel::Historico => "Histórico",
            Painel::Propriedades => "Propriedades",
        }
    }

    /// O ícone da aba (os sólidos do trilho de antes; o das Propriedades é a silhueta do botão da barra).
    pub(crate) fn icone(self) -> Icone {
        match self {
            Painel::Explorador => Icone::Explorador,
            Painel::Historico => Icone::Historico,
            Painel::Propriedades => Icone::Painel,
        }
    }

    /// Onde ele mora enquanto a pessoa não o muda: o Explorador e o Histórico à esquerda, as Propriedades à direita.
    fn lugar_padrao(self) -> Lugar {
        match self {
            Painel::Explorador | Painel::Historico => Lugar::Doca(Lado::Esquerda),
            Painel::Propriedades => Lugar::Doca(Lado::Direita),
        }
    }
}

impl From<estado::PainelCasca> for Painel {
    fn from(p: estado::PainelCasca) -> Self {
        match p {
            estado::PainelCasca::Explorador => Painel::Explorador,
            estado::PainelCasca::Historico => Painel::Historico,
            estado::PainelCasca::Propriedades => Painel::Propriedades,
        }
    }
}

/// Uma das duas docas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Lado {
    Esquerda,
    Direita,
}

impl Lado {
    fn indice(self) -> usize {
        self as usize
    }

    /// O nome da fileira de abas para o leitor de tela.
    pub(crate) fn rotulo(self) -> &'static str {
        match self {
            Lado::Esquerda => "Painéis à esquerda",
            Lado::Direita => "Painéis à direita",
        }
    }
}

/// Onde mora um painel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Lugar {
    Doca(Lado),
    Janela,
}

impl Lugar {
    fn de_prefs(l: LugarPainel) -> Lugar {
        match l {
            LugarPainel::Esquerda => Lugar::Doca(Lado::Esquerda),
            LugarPainel::Direita => Lugar::Doca(Lado::Direita),
            LugarPainel::Janela => Lugar::Janela,
        }
    }

    fn para_prefs(self) -> LugarPainel {
        match self {
            Lugar::Doca(Lado::Esquerda) => LugarPainel::Esquerda,
            Lugar::Doca(Lado::Direita) => LugarPainel::Direita,
            Lugar::Janela => LugarPainel::Janela,
        }
    }
}

/// Onde mora cada painel e o que está à vista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Paineis {
    lugar: [Lugar; 3],
    /// A doca de onde cada painel saiu para a janela própria (para onde o "Prender de volta" o leva).
    origem: [Lado; 3],
    /// A aba à vista em cada doca (a que a pessoa escolheu por último).
    ativo: [Painel; 2],
    /// Cada doca à vista (o Ctrl+B esconde a da esquerda).
    aberta: [bool; 2],
    /// Cada painel que mora em janela própria está com a janela aberta.
    janela_aberta: [bool; 3],
}

impl Default for Paineis {
    fn default() -> Self {
        Paineis {
            lugar: Painel::TODOS.map(Painel::lugar_padrao),
            origem: [Lado::Esquerda, Lado::Esquerda, Lado::Direita],
            ativo: [Painel::Explorador, Painel::Propriedades],
            aberta: [true, true],
            janela_aberta: [false; 3],
        }
    }
}

impl Paineis {
    /// Os lugares guardados nas preferências (o que não foi escolhido fica no padrão). Os painéis em janela própria
    /// abrem com a janela aberta.
    pub(crate) fn de_prefs(l: LugaresPaineis) -> Paineis {
        let mut p = Paineis::default();
        for (painel, guardado) in Painel::TODOS.into_iter().zip([l.explorador, l.historico, l.propriedades]) {
            if let Some(g) = guardado {
                p.lugar[painel.indice()] = Lugar::de_prefs(g);
                if let Lugar::Doca(lado) = Lugar::de_prefs(g) {
                    p.origem[painel.indice()] = lado;
                }
                p.janela_aberta[painel.indice()] = g == LugarPainel::Janela;
            }
        }
        p
    }

    /// Os lugares para as preferências (todos: a mudança é mesclada à guardada).
    pub(crate) fn para_prefs(&self) -> LugaresPaineis {
        let l = |p: Painel| Some(self.lugar[p.indice()].para_prefs());
        LugaresPaineis {
            explorador: l(Painel::Explorador),
            historico: l(Painel::Historico),
            propriedades: l(Painel::Propriedades),
        }
    }

    pub(crate) fn lugar(&self, painel: Painel) -> Lugar {
        self.lugar[painel.indice()]
    }

    /// As abas de uma doca, na ordem; sem as Propriedades quando o documento à vista não as tem.
    pub(crate) fn abas(&self, lado: Lado, tem_propriedades: bool) -> Vec<Painel> {
        Painel::TODOS
            .into_iter()
            .filter(|&p| self.lugar(p) == Lugar::Doca(lado) && (tem_propriedades || p != Painel::Propriedades))
            .collect()
    }

    /// A aba à vista numa doca: a escolhida, se ela está lá; senão, a primeira.
    pub(crate) fn ativo(&self, lado: Lado, tem_propriedades: bool) -> Option<Painel> {
        let abas = self.abas(lado, tem_propriedades);
        let escolhido = self.ativo[lado.indice()];
        if abas.contains(&escolhido) { Some(escolhido) } else { abas.first().copied() }
    }

    /// A doca aparece: a pessoa a quer e ela tem alguma aba.
    pub(crate) fn doca_a_vista(&self, lado: Lado, tem_propriedades: bool) -> bool {
        self.aberta[lado.indice()] && !self.abas(lado, tem_propriedades).is_empty()
    }

    /// O painel está à vista: a aba ativa de uma doca à vista, ou a janela própria aberta.
    pub(crate) fn a_vista(&self, painel: Painel, tem_propriedades: bool) -> bool {
        match self.lugar(painel) {
            Lugar::Doca(lado) => {
                self.doca_a_vista(lado, tem_propriedades) && self.ativo(lado, tem_propriedades) == Some(painel)
            }
            Lugar::Janela => {
                self.janela_aberta[painel.indice()] && (tem_propriedades || painel != Painel::Propriedades)
            }
        }
    }

    /// A janela própria do painel está aberta (ele mora nela e a pessoa não a fechou).
    pub(crate) fn janela_aberta(&self, painel: Painel) -> bool {
        self.lugar(painel) == Lugar::Janela && self.janela_aberta[painel.indice()]
    }

    /// A doca de onde o painel saiu para a janela própria (onde o "Prender de volta" o põe).
    pub(crate) fn origem(&self, painel: Painel) -> Lado {
        self.origem[painel.indice()]
    }

    /// Põe o painel à vista no lugar dele.
    pub(crate) fn mostrar(&mut self, painel: Painel) {
        match self.lugar(painel) {
            Lugar::Doca(lado) => {
                self.aberta[lado.indice()] = true;
                self.ativo[lado.indice()] = painel;
            }
            Lugar::Janela => self.janela_aberta[painel.indice()] = true,
        }
    }

    /// Mostra o painel ou, se ele já está à vista, esconde a doca dele (ou fecha a janela dele).
    pub(crate) fn alternar(&mut self, painel: Painel) {
        let lugar = self.lugar(painel);
        let a_vista = match lugar {
            Lugar::Doca(lado) => self.aberta[lado.indice()] && self.ativo(lado, true) == Some(painel),
            Lugar::Janela => self.janela_aberta[painel.indice()],
        };
        match (lugar, a_vista) {
            (Lugar::Doca(lado), true) => self.aberta[lado.indice()] = false,
            (Lugar::Janela, true) => self.janela_aberta[painel.indice()] = false,
            (_, false) => self.mostrar(painel),
        }
    }

    /// A aba clicada numa doca passa a ser a da vista.
    pub(crate) fn focar(&mut self, painel: Painel) {
        if let Lugar::Doca(lado) = self.lugar(painel) {
            self.ativo[lado.indice()] = painel;
        }
    }

    /// Fecha a janela própria (o × dela, prancheta 6): o painel volta escondido para a doca de onde saiu (a aba dele
    /// fica lá, sem ser a da vista; se for a única aba da doca, a doca fica escondida). Devolve se o lugar mudou.
    pub(crate) fn fechar_janela(&mut self, painel: Painel) -> bool {
        if self.lugar(painel) != Lugar::Janela {
            return false;
        }
        let lado = self.origem[painel.indice()];
        self.janela_aberta[painel.indice()] = false;
        self.lugar[painel.indice()] = Lugar::Doca(lado);
        let outros: Vec<Painel> = self.abas(lado, true).into_iter().filter(|&p| p != painel).collect();
        match outros.first() {
            Some(&outro) if self.ativo[lado.indice()] == painel => self.ativo[lado.indice()] = outro,
            Some(_) => {}
            None => self.aberta[lado.indice()] = false,
        }
        true
    }

    /// Muda o painel de lugar, já à vista no lugar novo. Devolve se mudou (para gravar nas preferências).
    pub(crate) fn mover(&mut self, painel: Painel, destino: Lugar) -> bool {
        let antes = self.lugar(painel);
        if let Lugar::Doca(lado) = antes
            && destino == Lugar::Janela
        {
            self.origem[painel.indice()] = lado;
        }
        self.lugar[painel.indice()] = destino;
        self.mostrar(painel);
        antes != destino
    }

    /// Prende de volta o painel da janela própria na doca de onde ele saiu.
    pub(crate) fn prender(&mut self, painel: Painel) -> bool {
        let lado = self.origem[painel.indice()];
        self.mover(painel, Lugar::Doca(lado))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn padrao_explorador_a_esquerda_e_propriedades_a_direita() {
        let p = Paineis::default();
        assert_eq!(p.abas(Lado::Esquerda, true), [Painel::Explorador, Painel::Historico]);
        assert_eq!(p.abas(Lado::Direita, true), [Painel::Propriedades]);
        assert_eq!(p.ativo(Lado::Esquerda, true), Some(Painel::Explorador));
        assert!(p.a_vista(Painel::Explorador, true) && p.a_vista(Painel::Propriedades, true));
        assert!(!p.a_vista(Painel::Historico, true));
        // o Início (sem Propriedades): a doca da direita some sem mudar a escolha
        assert!(!p.doca_a_vista(Lado::Direita, false));
        assert!(p.doca_a_vista(Lado::Direita, true));
    }

    #[test]
    fn alternar_mostra_na_doca_ou_esconde_a_doca() {
        let mut p = Paineis::default();
        // o Histórico vira a aba da vista; de novo, a doca some; o Explorador a traz de volta com ele
        p.alternar(Painel::Historico);
        assert_eq!(p.ativo(Lado::Esquerda, true), Some(Painel::Historico));
        p.alternar(Painel::Historico);
        assert!(!p.doca_a_vista(Lado::Esquerda, true));
        p.alternar(Painel::Explorador);
        assert!(p.a_vista(Painel::Explorador, true));
        p.alternar(Painel::Explorador);
        assert!(!p.a_vista(Painel::Explorador, true) && !p.a_vista(Painel::Historico, true));
        // as Propriedades vão e voltam sem mexer na esquerda
        p.alternar(Painel::Propriedades);
        assert!(!p.a_vista(Painel::Propriedades, true));
        p.alternar(Painel::Propriedades);
        assert!(p.a_vista(Painel::Propriedades, true));
        assert!(!p.doca_a_vista(Lado::Esquerda, true));
    }

    #[test]
    fn mover_entre_docas_e_para_a_janela_e_prender_de_volta() {
        let mut p = Paineis::default();
        // o Histórico vai para a direita: a esquerda fica com o Explorador e a direita ganha a aba dele, à vista
        assert!(p.mover(Painel::Historico, Lugar::Doca(Lado::Direita)));
        assert_eq!(p.abas(Lado::Direita, true), [Painel::Historico, Painel::Propriedades]);
        assert_eq!(p.ativo(Lado::Direita, true), Some(Painel::Historico));
        // no Início, a direita fica só com ele
        assert_eq!(p.abas(Lado::Direita, false), [Painel::Historico]);
        assert!(p.doca_a_vista(Lado::Direita, false));
        // as Propriedades vão para a janela própria e voltam para a direita
        assert!(p.mover(Painel::Propriedades, Lugar::Janela));
        assert!(p.janela_aberta(Painel::Propriedades) && p.a_vista(Painel::Propriedades, true));
        assert_eq!(p.abas(Lado::Direita, true), [Painel::Historico]);
        assert!(p.prender(Painel::Propriedades));
        assert_eq!(p.lugar(Painel::Propriedades), Lugar::Doca(Lado::Direita));
        assert_eq!(p.ativo(Lado::Direita, true), Some(Painel::Propriedades));
        // mover para onde já está não muda nada a gravar
        assert!(!p.mover(Painel::Explorador, Lugar::Doca(Lado::Esquerda)));
    }

    #[test]
    fn os_lugares_vao_e_voltam_das_preferencias() {
        let mut p = Paineis::default();
        p.mover(Painel::Explorador, Lugar::Janela);
        p.mover(Painel::Historico, Lugar::Doca(Lado::Direita));
        let guardados = p.para_prefs();
        assert_eq!(guardados.explorador, Some(LugarPainel::Janela));
        assert_eq!(guardados.historico, Some(LugarPainel::Direita));
        assert_eq!(guardados.propriedades, Some(LugarPainel::Direita));
        let lido = Paineis::de_prefs(guardados);
        assert_eq!(lido.lugar(Painel::Explorador), Lugar::Janela);
        assert!(lido.janela_aberta(Painel::Explorador));
        assert_eq!(lido.abas(Lado::Direita, true), [Painel::Historico, Painel::Propriedades]);
        // a esquerda ficou sem nada: some
        assert!(!lido.doca_a_vista(Lado::Esquerda, true));
        assert_eq!(Paineis::de_prefs(LugaresPaineis::default()), Paineis::default());
    }

    #[test]
    fn fechar_a_janela_devolve_o_painel_escondido_para_a_doca() {
        let mut p = Paineis::default();
        // as Propriedades sozinhas à direita: voltam, e a doca fica escondida até a pessoa pedir
        p.mover(Painel::Propriedades, Lugar::Janela);
        assert!(p.fechar_janela(Painel::Propriedades));
        assert_eq!(p.lugar(Painel::Propriedades), Lugar::Doca(Lado::Direita));
        assert!(!p.a_vista(Painel::Propriedades, true) && !p.doca_a_vista(Lado::Direita, true));
        p.alternar(Painel::Propriedades);
        assert!(p.a_vista(Painel::Propriedades, true));
        // o Histórico volta para a esquerda sem tirar o Explorador da vista
        p.mover(Painel::Historico, Lugar::Janela);
        assert!(p.a_vista(Painel::Explorador, true));
        assert!(p.fechar_janela(Painel::Historico));
        assert_eq!(p.abas(Lado::Esquerda, true), [Painel::Explorador, Painel::Historico]);
        assert!(p.a_vista(Painel::Explorador, true) && !p.a_vista(Painel::Historico, true));
        // fechar de novo não muda nada
        assert!(!p.fechar_janela(Painel::Historico));
    }
}
