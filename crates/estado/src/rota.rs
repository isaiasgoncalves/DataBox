//! Cada tela tem endereço: [`Rota`] e o histórico de
//! voltar/avançar em [`Navegacao`].
//!
//! | Rota | Tela | Endereço |
//! |---|---|---|
//! | `Inicio` | Início: abrir, novo, recentes | `#/` |
//! | `Banco` | Visão geral do banco | `#/banco` |
//! | `Tabela { nome, aba, dados }` | tabela; aba Dados, Estrutura, Índices e gatilhos, Definição SQL | `#/t/:nome/:aba?…` |
//! | `Visao { nome, aba, dados }` | visão; aba Dados ou Definição SQL | `#/v/:nome/:aba?…` |
//! | `Consulta { sql }` | Consulta SQL (`sql` pré-preenche o editor) | `#/sql?sql=` |
//! | `Arquivo { caminho }` | ficha de um arquivo de dados (não exige banco) | `#/arquivo/<caminho>` |
//!
//! Sem banco aberto, toda rota que [`Rota::exige_banco`] volta para `Inicio` (quem aplica é a casca); o `Inicio` e o
//! `Arquivo` não exigem. A aba Dados
//! guarda na rota a página, o tamanho da página, a ordem, a busca e os filtros ([`ConsultaDados`]).
//!
//! A [`Navegacao`] é genérica: a casca de uma tela só usa `Navegacao<Rota>`; a de várias abas guarda
//! `Navegacao<Entrada>` (aba + endereço, em [`crate::Abas`]).

use std::path::{Path, PathBuf};

use banco::{Direcao, Filtro, Operador};

/// Parte da tela de tabela ou visão: Dados, Estrutura, Índices e gatilhos, Definição SQL. Daqui em diante
/// "aba" quer dizer aba de documento ([`crate::Abas`]); o campo `aba` das rotas é renomeado na etapa 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Parte {
    #[default]
    Dados,
    Estrutura,
    Indices,
    Definicao,
}

/// O nome antigo de [`Parte`], para o código de hoje continuar compilando.
pub type Aba = Parte;

impl Parte {
    /// As partes de uma tabela, na ordem da tela.
    pub const DA_TABELA: [Parte; 4] = [Parte::Dados, Parte::Estrutura, Parte::Indices, Parte::Definicao];
    /// As partes de uma visão.
    pub const DA_VISAO: [Parte; 2] = [Parte::Dados, Parte::Definicao];

    /// O nome no endereço: "dados", "estrutura", "indices", "definicao".
    pub fn chave(self) -> &'static str {
        match self {
            Parte::Dados => "dados",
            Parte::Estrutura => "estrutura",
            Parte::Indices => "indices",
            Parte::Definicao => "definicao",
        }
    }

    /// O rótulo da parte na tela.
    pub fn rotulo(self) -> &'static str {
        match self {
            Parte::Dados => "Dados",
            Parte::Estrutura => "Estrutura",
            Parte::Indices => "Índices e gatilhos",
            Parte::Definicao => "Definição SQL",
        }
    }

    /// Parte pela chave; desconhecida → None (a tela usa a padrão, Dados).
    pub fn de_chave(chave: &str) -> Option<Parte> {
        Parte::DA_TABELA.into_iter().find(|a| a.chave() == chave)
    }
}

/// Um filtro como a pessoa montou na barra da aba Dados: coluna, operador e o texto digitado.
#[derive(Debug, Clone, PartialEq)]
pub struct FiltroDigitado {
    pub coluna: String,
    pub op: Operador,
    pub valor: String,
}

impl FiltroDigitado {
    /// O filtro para o banco (o texto passa pela coerção da afinidade da coluna).
    pub fn para_banco(&self) -> Filtro {
        Filtro::texto(self.coluna.clone(), self.op, &self.valor)
    }
}

/// Tamanhos de página que a aba Dados oferece.
pub const POR_PAGINA_OPCOES: [u64; 4] = [50, 100, 250, 500];
/// Tamanho de página padrão.
pub const POR_PAGINA_PADRAO: u64 = 100;

/// O estado da aba Dados que mora no endereço (a query `pagina, por_pagina, ordem, direcao, busca, filtros`).
#[derive(Debug, Clone, PartialEq)]
pub struct ConsultaDados {
    pub pagina: u64,
    pub por_pagina: u64,
    pub ordem: Option<String>,
    pub direcao: Direcao,
    pub busca: String,
    pub filtros: Vec<FiltroDigitado>,
}

impl Default for ConsultaDados {
    fn default() -> Self {
        ConsultaDados {
            pagina: 1,
            por_pagina: POR_PAGINA_PADRAO,
            ordem: None,
            direcao: Direcao::default(),
            busca: String::new(),
            filtros: Vec::new(),
        }
    }
}

impl ConsultaDados {
    /// A busca, se houver texto (espaços nas pontas contam como texto, como lá; só o vazio não conta).
    pub fn busca_opt(&self) -> Option<&str> {
        (!self.busca.is_empty()).then_some(self.busca.as_str())
    }

    pub fn tem_busca_ou_filtro(&self) -> bool {
        !self.busca.is_empty() || !self.filtros.is_empty()
    }

    /// Os filtros prontos para o banco.
    pub fn filtros_banco(&self) -> Vec<Filtro> {
        self.filtros.iter().map(FiltroDigitado::para_banco).collect()
    }
}

/// Endereço de uma tela.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Rota {
    #[default]
    Inicio,
    Banco,
    Tabela {
        nome: String,
        aba: Parte,
        dados: ConsultaDados,
    },
    Visao {
        nome: String,
        aba: Parte,
        dados: ConsultaDados,
    },
    Consulta {
        sql: Option<String>,
    },
    /// A ficha de um arquivo de dados (CSV, Excel, JSON, texto) aberto pela pasta ou por um caminho. Não depende do
    /// banco aberto; o mesmo arquivo, com qualquer grafia do caminho, cai na mesma aba ([`crate::ChaveAba::Arquivo`]).
    Arquivo {
        caminho: PathBuf,
    },
}

impl Rota {
    /// A rota da ficha de um arquivo de dados.
    pub fn arquivo(caminho: impl Into<PathBuf>) -> Rota {
        Rota::Arquivo { caminho: caminho.into() }
    }

    /// A tabela (ou visão) na parte Dados, primeira página.
    pub fn objeto(nome: impl Into<String>, e_visao: bool) -> Rota {
        Rota::objeto_na_aba(nome, e_visao, Parte::Dados)
    }

    /// A tabela (ou visão) numa parte; uma parte que a visão não tem vira Dados.
    pub fn objeto_na_aba(nome: impl Into<String>, e_visao: bool, aba: Parte) -> Rota {
        let nome = nome.into();
        if e_visao {
            let aba = if Parte::DA_VISAO.contains(&aba) { aba } else { Parte::Dados };
            Rota::Visao { nome, aba, dados: ConsultaDados::default() }
        } else {
            Rota::Tabela { nome, aba, dados: ConsultaDados::default() }
        }
    }

    /// Consulta SQL já com um comando no editor.
    pub fn consulta_com(sql: impl Into<String>) -> Rota {
        Rota::Consulta { sql: Some(sql.into()) }
    }

    /// A tela só existe com um banco aberto (o Início e a ficha de um arquivo de dados existem sem ele).
    pub fn exige_banco(&self) -> bool {
        !matches!(self, Rota::Inicio | Rota::Arquivo { .. })
    }

    /// O caminho do arquivo da ficha; None nas outras rotas.
    pub fn caminho_arquivo(&self) -> Option<&Path> {
        match self {
            Rota::Arquivo { caminho } => Some(caminho),
            _ => None,
        }
    }

    /// Nome da tabela ou visão da rota.
    pub fn nome_objeto(&self) -> Option<&str> {
        match self {
            Rota::Tabela { nome, .. } | Rota::Visao { nome, .. } => Some(nome),
            _ => None,
        }
    }

    pub fn aba(&self) -> Option<Parte> {
        match self {
            Rota::Tabela { aba, .. } | Rota::Visao { aba, .. } => Some(*aba),
            _ => None,
        }
    }

    pub fn dados(&self) -> Option<&ConsultaDados> {
        match self {
            Rota::Tabela { dados, .. } | Rota::Visao { dados, .. } => Some(dados),
            _ => None,
        }
    }

    /// A mesma rota com outro estado da aba Dados (página, busca, filtros…).
    pub fn com_dados(&self, novos: ConsultaDados) -> Rota {
        match self {
            Rota::Tabela { nome, aba, .. } => Rota::Tabela { nome: nome.clone(), aba: *aba, dados: novos },
            Rota::Visao { nome, aba, .. } => Rota::Visao { nome: nome.clone(), aba: *aba, dados: novos },
            outra => outra.clone(),
        }
    }

    /// O endereço em texto (para registro e testes): "#/t/clientes/dados?pagina=2".
    pub fn endereco(&self) -> String {
        match self {
            Rota::Inicio => "#/".into(),
            Rota::Banco => "#/banco".into(),
            Rota::Consulta { sql: None } => "#/sql".into(),
            Rota::Consulta { sql: Some(s) } => format!("#/sql?sql={s}"),
            Rota::Arquivo { caminho } => format!("#/arquivo/{}", caminho.to_string_lossy()),
            Rota::Tabela { nome, aba, dados } | Rota::Visao { nome, aba, dados } => {
                let letra = if matches!(self, Rota::Tabela { .. }) { "t" } else { "v" };
                let mut partes = Vec::new();
                if dados.pagina != 1 {
                    partes.push(format!("pagina={}", dados.pagina));
                }
                if dados.por_pagina != POR_PAGINA_PADRAO {
                    partes.push(format!("por_pagina={}", dados.por_pagina));
                }
                if let Some(o) = &dados.ordem {
                    partes.push(format!("ordem={o}&direcao={}", dados.direcao.como_str()));
                }
                if !dados.busca.is_empty() {
                    partes.push(format!("busca={}", dados.busca));
                }
                if !dados.filtros.is_empty() {
                    partes.push(format!("filtros={}", dados.filtros.len()));
                }
                let query = if partes.is_empty() { String::new() } else { format!("?{}", partes.join("&")) };
                format!("#/{letra}/{nome}/{}{query}", aba.chave())
            }
        }
    }
}

/// Quantas entradas o histórico guarda para trás.
pub(crate) const LIMITE_HISTORICO: usize = 100;

/// Endereço atual com o histórico de voltar e avançar (Alt+←/→ e os botões laterais do mouse). `T` é a
/// [`Rota`] (uma tela só) ou a [`crate::Entrada`] (aba + rota) das [`crate::Abas`].
#[derive(Debug, Clone, Default)]
pub struct Navegacao<T = Rota> {
    atras: Vec<T>,
    atual: T,
    frente: Vec<T>,
}

impl<T: PartialEq> Navegacao<T> {
    pub fn nova(atual: T) -> Navegacao<T> {
        Navegacao { atras: Vec::new(), atual, frente: Vec::new() }
    }

    pub fn atual(&self) -> &T {
        &self.atual
    }

    /// A entrada atual, para corrigir em lugar (como [`Navegacao::substituir`]).
    pub fn atual_mut(&mut self) -> &mut T {
        &mut self.atual
    }

    /// Todas as entradas, da mais antiga à mais nova (as de trás, a atual e as da frente), para corrigir em
    /// lugar.
    pub fn entradas_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.atras.iter_mut().chain(std::iter::once(&mut self.atual)).chain(self.frente.iter_mut().rev())
    }

    /// Vai para uma rota nova (a anterior entra no voltar; o avançar se perde). A mesma rota não empilha nada.
    pub fn ir(&mut self, rota: T) {
        if rota == self.atual {
            return;
        }
        let anterior = std::mem::replace(&mut self.atual, rota);
        self.atras.push(anterior);
        if self.atras.len() > LIMITE_HISTORICO {
            self.atras.remove(0);
        }
        self.frente.clear();
    }

    /// Troca a rota atual sem criar entrada no histórico (o `history.replaceState`).
    pub fn substituir(&mut self, rota: T) {
        self.atual = rota;
    }

    /// Tira de trás e da frente as entradas que não passam em `manter` e funde as vizinhas iguais (A, B, A sem
    /// o B vira A). Se a atual não passar, a anterior que ficou vira a atual. Devolve false quando a atual não
    /// passou e não havia nenhuma anterior: ela fica como estava, e quem chamou a substitui.
    pub fn reter(&mut self, mut manter: impl FnMut(&T) -> bool) -> bool {
        self.atras.retain(&mut manter);
        self.frente.retain(&mut manter);
        let ficou = manter(&self.atual)
            || match self.atras.pop() {
                Some(anterior) => {
                    self.atual = anterior;
                    true
                }
                None => false,
            };
        self.atras.dedup();
        self.frente.dedup();
        while self.atras.last() == Some(&self.atual) {
            self.atras.pop();
        }
        while self.frente.last() == Some(&self.atual) {
            self.frente.pop();
        }
        ficou
    }

    pub fn pode_voltar(&self) -> bool {
        !self.atras.is_empty()
    }

    pub fn pode_avancar(&self) -> bool {
        !self.frente.is_empty()
    }

    /// Volta uma rota; false se não havia para onde.
    pub fn voltar(&mut self) -> bool {
        match self.atras.pop() {
            Some(r) => {
                let atual = std::mem::replace(&mut self.atual, r);
                self.frente.push(atual);
                true
            }
            None => false,
        }
    }

    /// Avança uma rota; false se não havia para onde.
    pub fn avancar(&mut self) -> bool {
        match self.frente.pop() {
            Some(r) => {
                let atual = std::mem::replace(&mut self.atual, r);
                self.atras.push(atual);
                true
            }
            None => false,
        }
    }

    /// Recomeça o histórico numa rota (ao abrir, criar ou fechar um banco: as rotas antigas são de outro banco).
    pub fn reiniciar(&mut self, rota: T) {
        self.atras.clear();
        self.frente.clear();
        self.atual = rota;
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::abas::{Entrada, IdAba};

    #[test]
    fn historico_voltar_avancar() {
        let mut n = Navegacao::nova(Rota::Inicio);
        n.ir(Rota::Banco);
        n.ir(Rota::objeto("clientes", false));
        n.ir(Rota::objeto("clientes", false)); // a mesma não empilha
        assert!(n.voltar());
        assert_eq!(n.atual(), &Rota::Banco);
        assert!(n.voltar());
        assert_eq!(n.atual(), &Rota::Inicio);
        assert!(!n.voltar());
        assert!(n.avancar());
        n.ir(Rota::Consulta { sql: None });
        assert!(!n.pode_avancar());
        assert!(n.voltar());
        assert_eq!(n.atual(), &Rota::Banco);
    }

    #[test]
    fn visao_so_tem_dados_e_definicao() {
        assert_eq!(Rota::objeto_na_aba("v", true, Aba::Estrutura).aba(), Some(Aba::Dados));
        assert_eq!(Rota::objeto_na_aba("v", true, Aba::Definicao).aba(), Some(Aba::Definicao));
        assert_eq!(Aba::de_chave("indices"), Some(Aba::Indices));
        assert_eq!(Aba::de_chave("x"), None);
    }

    #[test]
    fn rota_arquivo_nao_exige_banco_e_tem_chave_pelo_caminho() {
        let rota = Rota::arquivo(r"C:\Dados\vendas.csv");
        assert!(!rota.exige_banco());
        assert!(!Rota::Inicio.exige_banco() && Rota::Banco.exige_banco() && Rota::Consulta { sql: None }.exige_banco());
        assert_eq!(rota.endereco(), r"#/arquivo/C:\Dados\vendas.csv");
        assert_eq!(rota.nome_objeto(), None);
        assert_eq!(rota.aba(), None);
        assert_eq!(rota.dados(), None);
        assert_eq!(rota.caminho_arquivo(), Some(std::path::Path::new(r"C:\Dados\vendas.csv")));
        assert_eq!(Rota::Banco.caminho_arquivo(), None);
        // a chave é a do caminho: sem diferença de caixa nem de barra
        let chave = crate::abas::ChaveAba::de(&rota);
        assert_eq!(chave, Some(crate::abas::ChaveAba::Arquivo(prefs::chave_caminho(r"C:\Dados\vendas.csv"))));
        assert_eq!(chave, crate::abas::ChaveAba::de(&Rota::arquivo("c:/dados/VENDAS.CSV")));
        assert_ne!(chave, crate::abas::ChaveAba::de(&Rota::arquivo(r"C:\Dados\outro.csv")));
        // com a rota de um arquivo no histórico, a comparação é pelo caminho como veio
        assert_ne!(rota, Rota::arquivo("c:/dados/VENDAS.CSV"));
    }

    #[test]
    fn reter_poda_e_funde_vizinhos() {
        // A, B, A, C com a atual no segundo A: sem o B, os dois A viram um
        let mut n = Navegacao::nova("A");
        n.ir("B");
        n.ir("A");
        n.ir("C");
        assert!(n.voltar());
        assert!(n.reter(|e| *e != "B"));
        assert_eq!(n.atual(), &"A");
        assert!(!n.pode_voltar(), "o A de trás se fundiu com o atual");
        assert!(n.avancar());
        assert_eq!(n.atual(), &"C");

        // a atual que sai dá lugar à anterior que ficou (e as vizinhas iguais se fundem)
        let mut n = Navegacao::nova(1);
        for i in [2, 3, 2, 4] {
            n.ir(i);
        }
        assert!(n.reter(|&e| e != 4 && e != 3));
        assert_eq!(n.atual(), &2);
        assert!(n.voltar());
        assert_eq!(n.atual(), &1);
        assert!(!n.pode_voltar());

        // sem nenhuma anterior que fique, a atual fica como estava e a resposta é false
        let mut n = Navegacao::nova(1);
        n.ir(2);
        assert!(n.voltar());
        assert!(!n.reter(|&e| e != 1));
        assert_eq!(n.atual(), &1);
        assert!(n.pode_avancar(), "a da frente fica");
    }

    #[test]
    fn navegacao_de_entradas() {
        let entrada = |aba, rota| Entrada { aba: IdAba(aba), rota };
        let clientes = Rota::objeto("clientes", false);
        let mut n = Navegacao::nova(entrada(1, Rota::Banco));
        n.ir(entrada(2, clientes.clone()));
        n.ir(entrada(2, clientes.clone())); // a mesma não empilha
        n.ir(entrada(3, Rota::Consulta { sql: None }));
        assert!(n.voltar());
        n.atual_mut().rota = clientes.com_dados(ConsultaDados { pagina: 2, ..Default::default() });
        let abas: Vec<u64> = n.entradas_mut().map(|e| e.aba.0).collect();
        assert_eq!(abas, [1, 2, 3], "da mais antiga à mais nova");
        assert_eq!(n.atual().rota.dados().map(|d| d.pagina), Some(2));
        assert_eq!(n.atual().rota.endereco(), "#/t/clientes/dados?pagina=2");
        assert_ne!(n.atual().rota, clientes, "a página conta na comparação");
        assert!(n.voltar());
        assert_eq!(n.atual(), &entrada(1, Rota::Banco));
        assert!(!n.pode_voltar());
    }
}
