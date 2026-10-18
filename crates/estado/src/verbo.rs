//! Os verbos da barra de ferramentas e do menu Editar (Salvar, Novo, Excluir, Exportar…) e o que a aba ativa
//! oferece deles a cada quadro ([`Ofertas`]): ligado ou não, marcado (`.ib.on`), principal (`.ib.pri`) e os
//! textos próprios ("Nova tabela", "Exportar clientes"). A casca desenha e chama a aba; a aba executa.
//!
//! O rótulo de acessibilidade é o da oferta ou o do verbo; a dica é a da oferta ou o rótulo com o atalho
//! ("Atualizar (F5)"), como os `aria-label` e `title` das pranchetas.
//!
//! Cada verbo tem um lugar fixo na barra, ligado ou não: nenhum botão muda de posição ao trocar de aba. O
//! [`Verbo::NovaColuna`] fica logo depois do [`Verbo::Novo`] e, na etapa 2, nenhuma aba o oferece. Na etapa 3, na
//! Estrutura, ele liga e o "+" (o Novo) desliga; no editor de CSV (etapa 5), ele insere a coluna depois da
//! selecionada. O [`Verbo::TrazerColunas`] fica logo depois da Nova coluna (etapa 5): só o editor de CSV o liga.

use std::borrow::Cow;

/// Um verbo da barra de ferramentas, na ordem dela.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Verbo {
    Salvar,
    Novo,
    /// Uma coluna nova (o lugar fixo depois do Novo; desligado em todas as abas na etapa 2).
    NovaColuna,
    /// O Trazer colunas do editor de CSV (logo depois da Nova coluna; desligado em todas as outras abas).
    TrazerColunas,
    Duplicar,
    Excluir,
    Desfazer,
    Refazer,
    Buscar,
    Filtrar,
    Atualizar,
    Executar,
    /// O Interromper da Consulta (logo depois do Executar): desligado fora de uma execução e o `.ib.risco` vermelho
    /// enquanto a aba o oferece ligado (uma execução em andamento, prancheta 15, N3).
    Interromper,
    Exportar,
}

/// Quantos verbos existem.
const QUANTOS: usize = Verbo::TODOS.len();

impl Verbo {
    /// Todos, na ordem da barra (e da declaração: `TODOS[v as usize] == v`).
    pub const TODOS: [Verbo; 14] = [
        Verbo::Salvar,
        Verbo::Novo,
        Verbo::NovaColuna,
        Verbo::TrazerColunas,
        Verbo::Duplicar,
        Verbo::Excluir,
        Verbo::Desfazer,
        Verbo::Refazer,
        Verbo::Buscar,
        Verbo::Filtrar,
        Verbo::Atualizar,
        Verbo::Executar,
        Verbo::Interromper,
        Verbo::Exportar,
    ];

    /// O rótulo padrão ("Novo", "Exportar"); a oferta pode trocar ("Nova tabela").
    pub fn rotulo(self) -> &'static str {
        match self {
            Verbo::Salvar => "Salvar",
            Verbo::Novo => "Novo",
            Verbo::NovaColuna => "Nova coluna",
            Verbo::TrazerColunas => "Trazer colunas",
            Verbo::Duplicar => "Duplicar",
            Verbo::Excluir => "Excluir",
            Verbo::Desfazer => "Desfazer",
            Verbo::Refazer => "Refazer",
            Verbo::Buscar => "Buscar",
            Verbo::Filtrar => "Filtrar",
            Verbo::Atualizar => "Atualizar",
            Verbo::Executar => "Executar",
            Verbo::Interromper => "Interromper",
            Verbo::Exportar => "Exportar",
        }
    }

    /// O ícone do botão da barra (o nome do ícone é o rótulo do verbo).
    pub fn icone(self) -> interface::icones::Icone {
        use interface::icones::Icone;
        match self {
            Verbo::Salvar => Icone::Salvar,
            Verbo::Novo => Icone::Novo,
            Verbo::NovaColuna => Icone::NovaColuna,
            Verbo::TrazerColunas => Icone::TrazerColunas,
            Verbo::Duplicar => Icone::Duplicar,
            Verbo::Excluir => Icone::Excluir,
            Verbo::Desfazer => Icone::Desfazer,
            Verbo::Refazer => Icone::Refazer,
            Verbo::Buscar => Icone::Buscar,
            Verbo::Filtrar => Icone::Filtrar,
            Verbo::Atualizar => Icone::Atualizar,
            Verbo::Executar => Icone::Executar,
            Verbo::Interromper => Icone::Interromper,
            Verbo::Exportar => Icone::Exportar,
        }
    }

    /// O atalho do teclado, para a dica e o menu.
    pub fn atalho(self) -> Option<&'static str> {
        match self {
            Verbo::Salvar => Some("Ctrl+S"),
            Verbo::Desfazer => Some("Ctrl+Z"),
            Verbo::Refazer => Some("Ctrl+Y"),
            Verbo::Buscar => Some("Ctrl+F"),
            Verbo::Atualizar => Some("F5"),
            Verbo::Executar => Some("Ctrl+Enter"),
            Verbo::Interromper => Some("Esc"),
            Verbo::Novo
            | Verbo::NovaColuna
            | Verbo::TrazerColunas
            | Verbo::Duplicar
            | Verbo::Excluir
            | Verbo::Filtrar
            | Verbo::Exportar => None,
        }
    }
}

/// O que a aba oferece de um verbo. O padrão é desligado, sem textos próprios.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Oferta {
    pub ligado: bool,
    /// O estado "ligado" do botão (`.ib.on`): o filtro com condição, por exemplo.
    pub marcado: bool,
    /// O botão principal da aba (`.ib.pri`): o Executar da Consulta.
    pub principal: bool,
    /// O botão está trabalhando (o arco girando no lugar do ícone, `.ib.ocupado`): o Executar da Consulta
    /// executando, que fica desligado.
    pub ocupado: bool,
    /// O rótulo de acessibilidade e do menu no lugar do do verbo ("Nova tabela").
    pub rotulo: Option<Cow<'static, str>>,
    /// A dica no lugar da padrão ("Exportar clientes").
    pub dica: Option<Cow<'static, str>>,
}

impl Oferta {
    pub fn ligada() -> Oferta {
        Oferta { ligado: true, ..Oferta::default() }
    }

    pub fn ligada_se(self, ligado: bool) -> Oferta {
        Oferta { ligado, ..self }
    }

    pub fn marcada_se(self, marcado: bool) -> Oferta {
        Oferta { marcado, ..self }
    }

    pub fn como_principal(self) -> Oferta {
        Oferta { principal: true, ..self }
    }

    pub fn ocupada_se(self, ocupado: bool) -> Oferta {
        Oferta { ocupado, ..self }
    }

    pub fn com_rotulo(self, rotulo: impl Into<Cow<'static, str>>) -> Oferta {
        Oferta { rotulo: Some(rotulo.into()), ..self }
    }

    pub fn com_dica(self, dica: impl Into<Cow<'static, str>>) -> Oferta {
        Oferta { dica: Some(dica.into()), ..self }
    }
}

/// A oferta de um verbo que (por engano) ficasse fora de [`Verbo::TODOS`]: desligado.
static DESLIGADA: Oferta =
    Oferta { ligado: false, marcado: false, principal: false, ocupado: false, rotulo: None, dica: None };

/// O que a aba oferece de cada verbo. O padrão desliga tudo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ofertas([Oferta; QUANTOS]);

impl Ofertas {
    pub fn get(&self, verbo: Verbo) -> &Oferta {
        self.0.get(verbo as usize).unwrap_or(&DESLIGADA)
    }

    /// Com esta oferta para o verbo.
    pub fn com(mut self, verbo: Verbo, oferta: Oferta) -> Ofertas {
        if let Some(o) = self.0.get_mut(verbo as usize) {
            *o = oferta;
        }
        self
    }

    /// Com o verbo ligado (os textos ficam os que estavam).
    pub fn ligar(mut self, verbo: Verbo) -> Ofertas {
        if let Some(o) = self.0.get_mut(verbo as usize) {
            o.ligado = true;
        }
        self
    }

    pub fn ligado(&self, verbo: Verbo) -> bool {
        self.get(verbo).ligado
    }

    /// O rótulo de acessibilidade e do menu: o da oferta ou o do verbo.
    pub fn rotulo(&self, verbo: Verbo) -> Cow<'static, str> {
        self.get(verbo).rotulo.clone().unwrap_or(Cow::Borrowed(verbo.rotulo()))
    }

    /// A dica: a da oferta ou o rótulo com o atalho ("Atualizar (F5)", "Nova tabela").
    pub fn dica(&self, verbo: Verbo) -> Cow<'static, str> {
        if let Some(dica) = &self.get(verbo).dica {
            return dica.clone();
        }
        let rotulo = self.rotulo(verbo);
        match verbo.atalho() {
            Some(atalho) => Cow::Owned(format!("{rotulo} ({atalho})")),
            None => rotulo,
        }
    }

    /// Os verbos com as ofertas, na ordem da barra.
    pub fn iter(&self) -> impl Iterator<Item = (Verbo, &Oferta)> {
        Verbo::TODOS.into_iter().zip(self.0.iter())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn padrao_desliga_tudo() {
        let o = Ofertas::default();
        assert!(o.iter().all(|(_, oferta)| *oferta == Oferta::default()));
        assert!(Verbo::TODOS.iter().all(|&v| !o.ligado(v)));
        assert_eq!(o.iter().count(), Verbo::TODOS.len());
        for (i, v) in Verbo::TODOS.into_iter().enumerate() {
            assert_eq!(v as usize, i, "TODOS na ordem da declaração");
        }

        let o = Ofertas::default()
            .ligar(Verbo::Atualizar)
            .com(Verbo::Novo, Oferta::default().com_rotulo("Nova tabela"))
            .com(Verbo::Executar, Oferta::ligada().como_principal().marcada_se(true))
            .com(Verbo::Exportar, Oferta::ligada().com_dica(format!("Exportar {}", "clientes")).ligada_se(false));
        let ligados: Vec<Verbo> = o.iter().filter(|(_, x)| x.ligado).map(|(v, _)| v).collect();
        assert_eq!(ligados, [Verbo::Atualizar, Verbo::Executar]);
        assert!(!o.ligado(Verbo::Novo), "o rótulo próprio não liga");
        assert!(o.get(Verbo::Executar).principal && o.get(Verbo::Executar).marcado);
        assert_eq!(o.dica(Verbo::Exportar), "Exportar clientes");
    }

    #[test]
    fn rotulos_e_atalhos() {
        let o = Ofertas::default().com(Verbo::Novo, Oferta::ligada().com_rotulo("Nova tabela"));
        // os `aria-label` e `title` das pranchetas
        assert_eq!(o.rotulo(Verbo::Novo), "Nova tabela");
        assert_eq!(o.dica(Verbo::Novo), "Nova tabela");
        assert_eq!(o.rotulo(Verbo::Atualizar), "Atualizar");
        assert_eq!(o.dica(Verbo::Atualizar), "Atualizar (F5)");
        assert_eq!(o.dica(Verbo::Salvar), "Salvar (Ctrl+S)");
        assert_eq!(o.dica(Verbo::Desfazer), "Desfazer (Ctrl+Z)");
        assert_eq!(o.dica(Verbo::Refazer), "Refazer (Ctrl+Y)");
        assert_eq!(o.dica(Verbo::Executar), "Executar (Ctrl+Enter)");
        assert_eq!(o.dica(Verbo::Filtrar), "Filtrar");
        assert_eq!(o.dica(Verbo::Exportar), "Exportar");
        assert_eq!(o.dica(Verbo::NovaColuna), "Nova coluna");
        assert_eq!(Verbo::NovaColuna.atalho(), None);
        assert_eq!(Verbo::Buscar.atalho(), Some("Ctrl+F"));
        // a ordem da barra: a Nova coluna no lugar fixo logo depois do Novo
        let rotulos: Vec<&str> = Verbo::TODOS.iter().map(|v| v.rotulo()).collect();
        assert_eq!(
            rotulos,
            [
                "Salvar",
                "Novo",
                "Nova coluna",
                "Trazer colunas",
                "Duplicar",
                "Excluir",
                "Desfazer",
                "Refazer",
                "Buscar",
                "Filtrar",
                "Atualizar",
                "Executar",
                "Interromper",
                "Exportar"
            ]
        );
        assert_eq!(Verbo::Interromper.atalho(), Some("Esc"));
        assert_eq!(Verbo::Interromper.icone(), interface::icones::Icone::Interromper);
    }

    #[test]
    fn verbo_trazer_colunas_na_ordem_da_barra() {
        // 14 verbos, na ordem da declaração
        assert_eq!(Verbo::TODOS.len(), 14);
        for v in Verbo::TODOS {
            assert_eq!(Verbo::TODOS[v as usize], v, "{v:?}");
        }
        // logo depois da Nova coluna e antes do Duplicar
        let lugar = |v: Verbo| Verbo::TODOS.iter().position(|&x| x == v).expect("na lista");
        assert_eq!(lugar(Verbo::TrazerColunas), lugar(Verbo::NovaColuna) + 1);
        assert_eq!(lugar(Verbo::Duplicar), lugar(Verbo::TrazerColunas) + 1);
        // rótulo, ícone (a silhueta própria, com o mesmo nome do verbo) e dica sem atalho
        assert_eq!(Verbo::TrazerColunas.rotulo(), "Trazer colunas");
        assert_eq!(Verbo::TrazerColunas.icone(), interface::icones::Icone::TrazerColunas);
        assert_eq!(Verbo::TrazerColunas.icone().nome(), Verbo::TrazerColunas.rotulo());
        assert_eq!(Verbo::TrazerColunas.atalho(), None);
        let o = Ofertas::default();
        assert_eq!(o.dica(Verbo::TrazerColunas), "Trazer colunas");
        // a Ofertas guarda o verbo novo no próprio lugar, sem mexer nos vizinhos
        let o = o.com(Verbo::TrazerColunas, Oferta::ligada().com_dica("Trazer colunas para vendas.csv"));
        let ligados: Vec<Verbo> = o.iter().filter(|(_, x)| x.ligado).map(|(v, _)| v).collect();
        assert_eq!(ligados, [Verbo::TrazerColunas]);
        assert_eq!(o.dica(Verbo::TrazerColunas), "Trazer colunas para vendas.csv");
        assert!(!o.ligado(Verbo::NovaColuna) && !o.ligado(Verbo::Duplicar));
    }
}
